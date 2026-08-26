//! Reading a Matroska's own index, to find out where its keyframes are.
//!
//! An HLS segment has to open on a random-access point, or a player that starts
//! there has nothing to decode the first picture against. Films do not oblige by
//! putting one every four seconds: a Blu-ray remux runs eight to twelve seconds
//! between them, so a four-second grid asks for boundaries that mostly are not
//! keyframes, and a segmenter that rounds forward to the next one hands the
//! player a stretch of film several segments further on than it asked for. The
//! picture stops within a few seconds of pressing play.
//!
//! The fix needs the keyframe times, and the file already knows them: that is
//! precisely what the `Cues` element is. This reads it — a few hundred kilobytes
//! at a position the `SeekHead` gives directly — rather than demuxing the track
//! to find out, which for a thirty-gigabyte film means reading thirty gigabytes.
//!
//! Symphonia parses the same element internally and seeks by it, but exposes
//! neither the cue list nor a seek that lands on one (its `Coarse` seek resolves
//! to the nearest *block*, which is how the segmenter came to be asking for
//! non-keyframe boundaries in the first place). So the element is read here. It
//! is a flat list, and nothing below cares about the rest of the container.

use anyhow::{Context, Result};
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;

/// EBML element ids, as the four documented classes of variable-length integer
/// with their length marker left in place — which is how they appear on the
/// wire, and why they are written a byte at a time here.
mod id {
    pub const EBML: u32 = 0x1A_45_DF_A3;
    pub const SEGMENT: u32 = 0x18_53_80_67;
    pub const SEEK_HEAD: u32 = 0x11_4D_9B_74;
    pub const SEEK: u32 = 0x4D_BB;
    pub const SEEK_ID: u32 = 0x53_AB;
    pub const SEEK_POSITION: u32 = 0x53_AC;
    pub const INFO: u32 = 0x15_49_A9_66;
    pub const TIMESTAMP_SCALE: u32 = 0x2A_D7_B1;
    pub const CUES: u32 = 0x1C_53_BB_6B;
    pub const CUE_POINT: u32 = 0xBB;
    pub const CUE_TIME: u32 = 0xB3;
    pub const CUE_TRACK_POSITIONS: u32 = 0xB7;
    pub const CUE_TRACK: u32 = 0xF7;
}

/// The default `TimestampScale`, in nanoseconds per tick: Matroska's cue and
/// cluster timestamps are in these, and almost every file uses milliseconds.
const DEFAULT_TIMESTAMP_SCALE: u64 = 1_000_000;

/// Top-level elements to walk past while looking for `Cues` without a
/// `SeekHead` to follow.
///
/// A film is a few thousand clusters, and walking them costs one header read
/// each — cheap, but not unbounded: a file whose element sizes are nonsense
/// must not turn a playlist request into a scan of the whole disk.
const MAX_TOP_LEVEL_ELEMENTS: usize = 100_000;

/// The most of one element this will read into memory. A two-and-a-half hour
/// film indexes itself in a few hundred kilobytes; this is room for an order of
/// magnitude more without letting a corrupt size become an allocation.
const MAX_ELEMENT_BYTES: u64 = 64 * 1024 * 1024;

/// The times, in milliseconds, at which `track_number` has a cue point.
///
/// For a video track those are its keyframes — that is what a cue point is for.
/// An empty result is not an error: plenty of files carry no index, or index
/// only some of their tracks, and the caller falls back to a fixed grid.
pub fn cue_times_ms(path: &Path, track_number: u64) -> Result<Vec<u64>> {
    let file = File::open(path)
        .with_context(|| format!("opening {} to read its cue index", path.display()))?;
    let mut reader = Reader {
        end: file.metadata().map(|m| m.len()).unwrap_or(u64::MAX),
        inner: BufReader::with_capacity(64 * 1024, file),
    };

    let segment = reader.find_segment()?;
    let (scale, cues) = reader.find_cues(&segment)?;
    let Some(cues) = cues else {
        return Ok(Vec::new());
    };

    // One contiguous read, then all of the walking happens in memory. Seeking
    // per element instead would be a few tens of thousands of seeks on a film,
    // each throwing away the read buffer it just filled.
    let body = reader.read_body(&cues)?;
    let mut times = cue_times(&body, track_number);
    // Cue points are meant to be written in order, and are not always. Anything
    // downstream treats these as a timeline, so make them one.
    times.sort_unstable();
    times.dedup();
    // A tick is a millisecond in every file anyone ships, but the header is
    // allowed to say otherwise and then the numbers mean something else.
    if scale != DEFAULT_TIMESTAMP_SCALE {
        for time in &mut times {
            *time = time.saturating_mul(scale) / 1_000_000;
        }
    }
    Ok(times)
}

/// Every cue point in `cues` belonging to `track_number`, in Matroska ticks.
fn cue_times(cues: &[u8], track_number: u64) -> Vec<u64> {
    let mut times = Vec::new();
    let mut points = Cursor::new(cues);
    while let Some((id, point)) = points.read_element() {
        if id != id::CUE_POINT {
            continue;
        }
        let mut time: Option<u64> = None;
        let mut wanted = false;
        let mut fields = Cursor::new(point);
        while let Some((field, body)) = fields.read_element() {
            match field {
                id::CUE_TIME => time = Some(uint(body)),
                // One cue point can index several tracks at the same instant,
                // so this is a search rather than a read.
                id::CUE_TRACK_POSITIONS => {
                    let mut positions = Cursor::new(body);
                    while let Some((inner, at)) = positions.read_element() {
                        if inner == id::CUE_TRACK && uint(at) == track_number {
                            wanted = true;
                        }
                    }
                }
                _ => {}
            }
        }
        if let (Some(time), true) = (time, wanted) {
            times.push(time);
        }
    }
    times
}

/// An EBML element's body read as an unsigned integer, big-endian.
fn uint(bytes: &[u8]) -> u64 {
    bytes
        .iter()
        .take(8)
        .fold(0u64, |acc, b| (acc << 8) | u64::from(*b))
}

/// A walk over the elements of one element's body, in memory.
struct Cursor<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Cursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, at: 0 }
    }

    /// Read one variable-length integer, returning it and its width.
    ///
    /// `keep_marker` distinguishes the two uses EBML puts these to: an element
    /// id is the bytes as they stand, marker included, while a size is the
    /// value with the marker stripped.
    fn read_vint(&mut self, keep_marker: bool) -> Option<(u64, u32)> {
        let lead = *self.bytes.get(self.at)?;
        if lead == 0 {
            // Five bytes or more: legal for a size, never used, and not worth
            // carrying a slow path for.
            return None;
        }
        let width = lead.leading_zeros() + 1;
        // At the widest, every bit of the first byte is marker.
        let value_bits = if width >= 8 { 0 } else { 0xFFu8 >> width };
        let mut value = if keep_marker {
            u64::from(lead)
        } else {
            u64::from(lead & value_bits)
        };
        for offset in 1..width as usize {
            value = (value << 8) | u64::from(*self.bytes.get(self.at + offset)?);
        }
        self.at += width as usize;
        Some((value, width))
    }

    /// The next element's id and body, or `None` at the end — or on anything
    /// malformed, because a truncated index is a missing index rather than a
    /// reason to fail a request.
    fn read_element(&mut self) -> Option<(u32, &'a [u8])> {
        let (id, _) = self.read_vint(true)?;
        let (size, _) = self.read_vint(false)?;
        let size = usize::try_from(size).ok()?;
        let end = self.at.checked_add(size)?.min(self.bytes.len());
        let body = self.bytes.get(self.at..end)?;
        self.at = end;
        Some((u32::try_from(id).ok()?, body))
    }
}

/// A `Segment` element's extent, which every `SeekHead` position is relative to.
struct Span {
    start: u64,
    end: u64,
}

struct Reader {
    inner: BufReader<File>,
    /// Length of the file, and so the end of any element that declares itself
    /// unknown-length (which a Segment written by a live muxer does).
    end: u64,
}

impl Reader {
    fn position(&mut self) -> Result<u64> {
        Ok(self.inner.stream_position()?)
    }

    fn seek_to(&mut self, at: u64) -> Result<()> {
        self.inner.seek(SeekFrom::Start(at))?;
        Ok(())
    }

    /// Read one variable-length integer, returning it and its width.
    ///
    /// `keep_marker` distinguishes the two uses EBML puts these to: an element
    /// id is the bytes as they stand, marker included, while a size is the
    /// value with the marker stripped.
    fn read_vint(&mut self, keep_marker: bool) -> Result<Option<(u64, u32)>> {
        let mut first = [0u8; 1];
        if self.inner.read_exact(&mut first).is_err() {
            return Ok(None);
        }
        let lead = first[0];
        if lead == 0 {
            // Five bytes or more: legal for a size, never used, and not worth
            // carrying a slow path for.
            return Ok(None);
        }
        let width = lead.leading_zeros() + 1;
        // At the widest, every bit of the first byte is marker.
        let value_bits = if width >= 8 { 0 } else { 0xFFu8 >> width };
        let mut value = if keep_marker {
            u64::from(lead)
        } else {
            u64::from(lead & value_bits)
        };
        for _ in 1..width {
            let mut next = [0u8; 1];
            if self.inner.read_exact(&mut next).is_err() {
                return Ok(None);
            }
            value = (value << 8) | u64::from(next[0]);
        }
        Ok(Some((value, width)))
    }

    /// Read an element header: its id, and the extent of its body.
    ///
    /// `None` at the end of the enclosing element, or on anything malformed —
    /// a truncated index is a missing index, not a reason to fail a request.
    fn read_header(&mut self, limit: u64) -> Result<Option<(u32, Span)>> {
        if self.position()? >= limit {
            return Ok(None);
        }
        let Some((id, _)) = self.read_vint(true)? else {
            return Ok(None);
        };
        let Some((size, width)) = self.read_vint(false)? else {
            return Ok(None);
        };
        let body = self.position()?;
        // A size whose every value bit is set means "unknown", which only a
        // Segment or a Cluster uses and which then runs to the end of its
        // parent.
        let unknown = size == (1u64 << (7 * width)) - 1;
        let end = if unknown {
            limit
        } else {
            body.saturating_add(size).min(limit)
        };
        Ok(Some((u32::try_from(id).unwrap_or(0), Span { start: body, end })))
    }

    /// Read an element's body into memory.
    ///
    /// Capped, because the size is a number in the file and a corrupt one must
    /// not become an allocation. A film's index is a few hundred kilobytes.
    fn read_body(&mut self, span: &Span) -> Result<Vec<u8>> {
        let len = (span.end.saturating_sub(span.start)).min(MAX_ELEMENT_BYTES) as usize;
        let mut body = vec![0u8; len];
        self.seek_to(span.start)?;
        self.inner.read_exact(&mut body)?;
        Ok(body)
    }

    /// Find the `Segment`, which everything else lives inside.
    fn find_segment(&mut self) -> Result<Span> {
        self.seek_to(0)?;
        let file_end = self.end;
        while let Some((id, span)) = self.read_header(file_end)? {
            match id {
                id::SEGMENT => return Ok(span),
                // The EBML header, and anything else preceding the Segment.
                id::EBML => self.seek_to(span.end)?,
                _ => self.seek_to(span.end)?,
            }
        }
        anyhow::bail!("no Matroska Segment element")
    }

    /// Locate `Cues` and read the `TimestampScale`, following the `SeekHead`
    /// where there is one and walking the top level where there is not.
    fn find_cues(&mut self, segment: &Span) -> Result<(u64, Option<Span>)> {
        let mut scale = DEFAULT_TIMESTAMP_SCALE;
        let mut pointer: Option<u64> = None;
        let mut info_seen = false;

        self.seek_to(segment.start)?;
        let mut seen = 0usize;
        while let Some((id, span)) = self.read_header(segment.end)? {
            seen += 1;
            if seen > MAX_TOP_LEVEL_ELEMENTS {
                break;
            }
            match id {
                id::SEEK_HEAD => pointer = self.read_seek_head(&span, segment)?.or(pointer),
                id::INFO => {
                    info_seen = true;
                    scale = self.read_timestamp_scale(&span)?.unwrap_or(scale);
                }
                id::CUES => return Ok((scale, Some(span))),
                _ => {}
            }
            // Following the pointer is the whole point of a `SeekHead`: it
            // turns a walk over every cluster of a thirty-gigabyte film into
            // two reads. Not taken before `Info`, because the scale those cue
            // times are in is only known once it has been read — and `Info`
            // always precedes the clusters.
            if info_seen {
                if let Some(at) = pointer.take() {
                    if let Some(cues) = self.cues_at(at, segment)? {
                        return Ok((scale, Some(cues)));
                    }
                    // The pointer did not lead to `Cues`, so it is worth
                    // nothing; carry on walking from where the walk had got to.
                }
            }
            self.seek_to(span.end)?;
        }
        Ok((scale, None))
    }

    /// The element at `at`, if that is where `Cues` turns out to be.
    fn cues_at(&mut self, at: u64, segment: &Span) -> Result<Option<Span>> {
        if at <= segment.start || at >= segment.end {
            return Ok(None);
        }
        self.seek_to(at)?;
        Ok(match self.read_header(segment.end)? {
            Some((id::CUES, span)) => Some(span),
            _ => None,
        })
    }

    /// The position a `SeekHead` gives for `Cues`, as an absolute file offset.
    fn read_seek_head(&mut self, span: &Span, segment: &Span) -> Result<Option<u64>> {
        let body = self.read_body(span)?;
        let mut entries = Cursor::new(&body);
        let mut found = None;
        while let Some((id, entry)) = entries.read_element() {
            if id != id::SEEK {
                continue;
            }
            let mut fields = Cursor::new(entry);
            let mut target: Option<u32> = None;
            let mut at: Option<u64> = None;
            while let Some((field, value)) = fields.read_element() {
                match field {
                    id::SEEK_ID => target = u32::try_from(uint(value)).ok(),
                    id::SEEK_POSITION => at = Some(uint(value)),
                    _ => {}
                }
            }
            if target == Some(id::CUES) {
                // Seek positions are relative to the Segment's first byte.
                found = at.map(|at| segment.start.saturating_add(at));
            }
        }
        Ok(found)
    }

    /// The `TimestampScale` inside an `Info` element.
    fn read_timestamp_scale(&mut self, span: &Span) -> Result<Option<u64>> {
        let body = self.read_body(span)?;
        let mut fields = Cursor::new(&body);
        let mut scale = None;
        while let Some((id, value)) = fields.read_element() {
            if id == id::TIMESTAMP_SCALE {
                scale = Some(uint(value));
            }
        }
        Ok(scale.filter(|s| *s > 0))
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    /// Write an EBML element: id (already marker-bearing), then an eight-byte
    /// size, then the body.
    fn element(id: u32, body: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        let id_bytes = id.to_be_bytes();
        let lead = id_bytes.iter().position(|b| *b != 0).unwrap_or(3);
        out.extend_from_slice(&id_bytes[lead..]);
        // The eight-byte size form: 0x01 then seven length bytes.
        out.push(0x01);
        out.extend_from_slice(&(body.len() as u64).to_be_bytes()[1..]);
        out.extend_from_slice(body);
        out
    }

    fn uint(id: u32, value: u64) -> Vec<u8> {
        element(id, &value.to_be_bytes())
    }

    /// A minimal but real Matroska skeleton: a header, then a Segment holding
    /// Info, a Cues list, and nothing else that matters here.
    fn skeleton(scale: u64, points: &[(u64, u64)]) -> Vec<u8> {
        let mut cues = Vec::new();
        for (time, track) in points {
            let mut point = uint(id::CUE_TIME, *time);
            point.extend_from_slice(&element(
                id::CUE_TRACK_POSITIONS,
                &uint(id::CUE_TRACK, *track),
            ));
            cues.extend_from_slice(&element(id::CUE_POINT, &point));
        }
        let mut segment = element(id::INFO, &uint(id::TIMESTAMP_SCALE, scale));
        segment.extend_from_slice(&element(id::CUES, &cues));

        let mut file = element(id::EBML, &[0u8; 4]);
        file.extend_from_slice(&element(id::SEGMENT, &segment));
        file
    }

    fn read(bytes: &[u8], track: u64) -> Vec<u64> {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Film.mkv");
        std::fs::write(&path, bytes).unwrap();
        cue_times_ms(&path, track).unwrap()
    }

    #[test]
    fn a_tracks_own_cue_points_come_back_in_milliseconds() {
        let file = skeleton(1_000_000, &[(0, 1), (2_002, 1), (12_429, 1), (22_814, 1)]);
        assert_eq!(read(&file, 1), vec![0, 2_002, 12_429, 22_814]);
    }

    /// A film indexes its subtitle tracks too, and those cue points are not
    /// keyframes of anything the segmenter is cutting.
    #[test]
    fn another_tracks_cue_points_are_not_this_ones() {
        let file = skeleton(1_000_000, &[(0, 1), (500, 7), (2_002, 1), (900, 7)]);
        assert_eq!(read(&file, 1), vec![0, 2_002]);
        assert_eq!(read(&file, 7), vec![500, 900]);
        assert!(read(&file, 3).is_empty());
    }

    /// The scale is a header field, not a constant, and the times mean nothing
    /// without it.
    #[test]
    fn a_non_default_timestamp_scale_is_applied() {
        // Ten-microsecond ticks: a hundred of them to the millisecond.
        let file = skeleton(10_000, &[(0, 1), (200_200, 1), (1_242_900, 1)]);
        assert_eq!(read(&file, 1), vec![0, 2_002, 12_429]);
    }

    /// Plenty of files carry no index at all. That is a fixed grid's problem to
    /// solve, not an error to fail a playlist request with.
    #[test]
    fn a_file_with_no_cues_reads_as_no_cues_rather_than_failing() {
        let mut file = element(id::EBML, &[0u8; 4]);
        file.extend_from_slice(&element(
            id::SEGMENT,
            &element(id::INFO, &uint(id::TIMESTAMP_SCALE, 1_000_000)),
        ));
        assert!(read(&file, 1).is_empty());
    }

    #[test]
    fn cue_points_written_out_of_order_still_read_as_a_timeline() {
        let file = skeleton(1_000_000, &[(12_429, 1), (0, 1), (2_002, 1), (2_002, 1)]);
        assert_eq!(read(&file, 1), vec![0, 2_002, 12_429]);
    }
}
