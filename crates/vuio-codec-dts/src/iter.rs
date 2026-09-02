//! Multi-frame iteration over a DTS Core byte stream.
//!
//! Round 6 (2026-05-25) adds two demuxer-friendly helpers on top of
//! the single-frame [`crate::parse_frame_header`] /
//! [`crate::parse_frame_header_14bit`] entry points:
//!
//! - [`find_next_sync`] — scan a byte buffer for the next DTS sync
//!   sequence starting at an arbitrary offset, returning both the
//!   offset and the [`crate::SyncWordEncoding`] that matched. Useful
//!   for callers that need to resynchronise after lost bytes, drop
//!   leading container padding, or walk a raw `.dts` file frame by
//!   frame.
//! - [`FrameIterator`] / [`iter_frames`] — walk a byte buffer one
//!   frame at a time. Each successful step parses the frame's header,
//!   reports its byte range, and advances by
//!   [`crate::DtsFrameHeader::frame_size_bytes`] (for raw 16-bit
//!   encodings; see the function docs for the 14-bit advance rule).
//!
//! Round 159 (2026-05-27) adds an error-tolerant counterpart to
//! [`FrameIterator`]:
//!
//! - [`FrameIteratorResync`] / [`iter_frames_resync`] — when a
//!   candidate sync turns out to be a false positive (random payload
//!   bytes that happened to match a 4-byte sync sequence and whose
//!   subsequent header bits fail the structural NBLKS / FSIZE bounds,
//!   or whose declared `frame_size_bytes` overruns end-of-buffer),
//!   surface a [`ResyncEvent`] reporting the discarded offset + cause
//!   and continue scanning from `offset + 1` instead of terminating.
//!   Useful for stream-integrity tooling that needs to walk a
//!   partially-corrupted `.dts` stream past a malformed-syncword
//!   patch.
//!
//! Neither helper depends on the [`oxideav-core`] integration; both
//! are available in the `--no-default-features` build alongside the
//! standalone parsers.
//!
//! ## What stays out of scope
//!
//! - Container-stream parsing: a raw `.dts` file is a concatenation
//!   of self-delimited Core frames, which is the only shape these
//!   helpers walk. AVI / MP4 / Matroska sample carriage stays in
//!   their respective container crates; the helpers here operate on
//!   raw codec bytes only.
//! - PCM sample emission. The iterator surfaces parsed
//!   [`crate::DtsFrameHeader`] records and the raw frame byte slice;
//!   subband / QMF / Huffman decoding remains gated on the spec
//!   tables in the docs gaps.
//! - 14-bit byte-advance for [`FrameIterator`]. The iterator only
//!   advances on the **raw 16-bit** sync variants because the
//!   `frame_size_bytes` field is documented as the byte length of
//!   the unpacked stream — for 14-bit-packed containers the
//!   corresponding container-byte advance would be
//!   `frame_size_bytes * 8 / 14` rounded up to the next even byte,
//!   which the wiki snapshot does **not** spell out. The 14-bit
//!   single-frame [`crate::parse_frame_header_14bit`] entry point
//!   remains available for callers that have already partitioned
//!   their 14-bit input into frame-sized slices. See `README.md`'s
//!   round-6 docs gap #7.

use crate::header::{detect_sync, parse_frame_header};
use crate::{DtsFrameHeader, Error, Result, SyncWordEncoding, parse_frame_header_14bit};

/// Result of a [`find_next_sync`] lookup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyncMatch {
    /// Absolute byte offset (within the input slice passed to
    /// [`find_next_sync`]) where the sync sequence starts.
    pub offset: usize,
    /// Which of the four documented sync encodings was found.
    pub encoding: SyncWordEncoding,
}

impl SyncMatch {
    /// Byte length of the sync sequence at [`Self::offset`],
    /// delegated to [`SyncWordEncoding::sync_byte_length`].
    ///
    /// Equivalent to `self.encoding.sync_byte_length()`. Provided as a
    /// thin accessor so the common pattern "advance the cursor past
    /// the matched sync" reads naturally —
    /// `cursor = sync_match.offset + sync_match.sync_byte_length()`
    /// — without spelling the field access out.
    #[inline]
    pub fn sync_byte_length(&self) -> usize {
        self.encoding.sync_byte_length()
    }

    /// Half-open byte range of the sync sequence at this match
    /// (`offset..offset + sync_byte_length()`).
    ///
    /// Provided so callers that want to highlight the matched bytes
    /// (e.g. for stream-integrity tooling that emits a per-sync
    /// hex-window report) can slice the input directly:
    /// `&bytes[sync_match.sync_byte_range()]`.
    #[inline]
    pub fn sync_byte_range(&self) -> core::ops::Range<usize> {
        self.offset..self.offset + self.sync_byte_length()
    }
}

/// First-byte gate for the four documented DTS sync sequences.
///
/// All four sync words begin with a distinct first byte that does
/// not appear in mid-sync positions of any of the other three:
///
/// | Encoding                  | First byte | Source                            |
/// | ------------------------- | ---------- | --------------------------------- |
/// | `RawBigEndian`            | `0x7F`     | wiki snapshot `7F FE 80 01`       |
/// | `RawLittleEndian`         | `0xFE`     | wiki snapshot `FE 7F 01 80`       |
/// | `FourteenBitBigEndian`    | `0x1F`     | wiki snapshot `1F FF E8 00 07 Fx` |
/// | `FourteenBitLittleEndian` | `0xFF`     | wiki snapshot `FF 1F 00 E8 Fx 07` |
///
/// `find_next_sync` uses this as a one-byte filter to skip the
/// 4-byte raw-sync check + 6-byte 14-bit-sync check on positions
/// whose first byte cannot start any documented sync. On random
/// payload bytes this short-circuits ~98.4% of positions (4 of 256
/// possible first bytes match) before any multi-byte comparison
/// fires.
#[inline]
fn is_sync_first_byte_candidate(b: u8) -> bool {
    // Equivalent to `matches!(b, 0x7F | 0xFE | 0x1F | 0xFF)`. Written
    // as a single bitmask check so a release-mode compile lowers to a
    // bt / cmp pair rather than a four-way branch.
    matches!(b, 0x7F | 0xFE | 0x1F | 0xFF)
}

/// Scan `bytes[start..]` for the next DTS Core sync sequence.
///
/// Returns the byte offset (in the original `bytes` slice, not in
/// `bytes[start..]`) and the matched [`SyncWordEncoding`] of the
/// first sync found at or after `start`, or `None` if no sync
/// appears before end-of-buffer.
///
/// All four documented sync sequences are accepted:
///
/// - `7F FE 80 01` — raw big-endian (4 bytes).
/// - `FE 7F 01 80` — raw little-endian (4 bytes).
/// - `1F FF E8 00 07 Fx` — 14-bit packed big-endian (6 bytes).
/// - `FF 1F 00 E8 Fx 07` — 14-bit packed little-endian (6 bytes).
///
/// The 14-bit variants are matched via the lower-14-bit payloads of
/// the first three containers (`0x1FFF`, `0x2800`,
/// top-4-of-`0x07F?`), matching the same widened detection rule the
/// single-frame parser uses (see [`detect_sync`] in `header.rs`).
///
/// The scan is `O(n)`: every byte is visited at most twice (once for
/// the 4-byte raw-sync check, once for the 6-byte 14-bit-sync
/// check). Calling [`find_next_sync`] in a loop with the previous
/// `offset + 1` is the standard resync pattern.
///
/// Round 165 (2026-05-27) added a one-byte first-byte gate
/// ([`is_sync_first_byte_candidate`]) before the multi-byte
/// [`detect_sync`] call so positions whose first byte cannot start
/// any documented sync (252 of 256 possible bytes) skip the
/// multi-byte comparison entirely. The walk order, returned offset,
/// and matched encoding are unchanged from the round-6 implementation
/// — round 165 also adds a `find_next_sync_matches_pre_optimization_reference`
/// equivalence test to prove that every byte sequence the old loop
/// would accept the new loop also accepts (and at the same offset
/// with the same encoding tag).
pub fn find_next_sync(bytes: &[u8], start: usize) -> Option<SyncMatch> {
    if start >= bytes.len() {
        return None;
    }
    let mut i = start;
    // We need at least 4 bytes for the shortest sync (raw 16-bit) and
    // 6 bytes for the longest (14-bit packed). Stop the scan at the
    // last position that could still hold a raw sync.
    let last = bytes.len();
    while i + 4 <= last {
        // First-byte gate: 252 of 256 possible bytes fail this and
        // skip the multi-byte detect_sync call. The walk order is
        // preserved (every position is still visited in order) so
        // the returned offset / encoding are identical to the
        // pre-round-165 implementation.
        if !is_sync_first_byte_candidate(bytes[i]) {
            i += 1;
            continue;
        }
        if let Ok(enc) = detect_sync(&bytes[i..]) {
            return Some(SyncMatch {
                offset: i,
                encoding: enc,
            });
        }
        i += 1;
    }
    None
}

/// One step of a [`FrameIterator`] over a raw-16-bit DTS Core
/// stream.
#[derive(Debug, Clone, Copy)]
pub struct FrameView<'a> {
    /// Parsed header.
    pub header: DtsFrameHeader,
    /// Absolute byte offset of the frame's first sync byte within
    /// the input passed to [`iter_frames`].
    pub offset: usize,
    /// Byte length of the frame (from
    /// [`DtsFrameHeader::frame_size_bytes`]). Always 95..=16384.
    pub len: usize,
    /// Borrowed frame bytes (`bytes[offset..offset + len]`).
    pub data: &'a [u8],
}

impl<'a> FrameView<'a> {
    /// SUBFRAMES region of the frame: the bytes that follow the
    /// fully-decoded frame-sync header.
    ///
    /// Equivalent to `&self.data[self.header.header_byte_length()..]`.
    /// The wiki snapshot (`docs/audio/dts/wiki/DTS.wiki`) marks this
    /// region as `'''TODO'''`; subband / QMF / Huffman / VQ decoding
    /// remains gated on the §5.3.1 value tables and the §5.4 polyphase
    /// filterbank landing in `docs/`. The helper is exposed so
    /// downstream code (re-muxers, payload-CRC validators, future
    /// subframe decoders) can carve out the region without recomputing
    /// the header boundary.
    ///
    /// Always non-empty for well-formed frames: the smallest documented
    /// frame size is 95 B and the largest header window is 15 B
    /// (`crc_present == true`), so at least 80 B of SUBFRAMES region
    /// is guaranteed.
    pub fn payload(&self) -> &'a [u8] {
        &self.data[self.header.header_byte_length()..]
    }

    /// Locate and parse the frame's §5.7.1 Auxiliary Data chunk (the
    /// optional end-of-frame time stamp + dynamic downmix
    /// coefficients), returning `Ok(None)` when the frame carries no
    /// DWORD-aligned `nSYNCAUX` word. Composition of
    /// [`crate::parse_aux_data`] over the frame's own bytes + header.
    pub fn aux_data(&self) -> crate::Result<Option<crate::AuxData>> {
        crate::parse_aux_data(self.data, &self.header)
    }

    /// Locate and parse the frame's §5.7.2 Rev2 Auxiliary Data Chunk
    /// (embedded-ES downmix scale + broadcast DRC / dialnorm),
    /// returning `Ok(None)` when the frame carries no DWORD-aligned
    /// `nSYNCRev2AUX` word. Composition of [`crate::parse_rev2_aux`]
    /// over the frame's own bytes + header.
    pub fn rev2_aux(&self) -> crate::Result<Option<crate::Rev2AuxChunk>> {
        crate::parse_rev2_aux(self.data, &self.header)
    }
}

/// Iterator that walks a raw-16-bit DTS Core byte buffer frame by
/// frame.
///
/// Each [`Iterator::next`] step:
/// 1. Calls [`find_next_sync`] from the current cursor to handle any
///    leading garbage / inter-frame padding the source may have
///    introduced. The iterator does NOT assume `bytes[0]` is a sync
///    byte; it scans for one.
/// 2. Calls [`parse_frame_header`] at the located offset. A parse
///    failure (no sync within reach, truncated header, NBLKS or
///    FSIZE out of range) is returned as the next item's `Err`
///    variant; subsequent calls then return `None`.
/// 3. On success, yields a [`FrameView`] borrowing the frame's
///    bytes from the input and advances the cursor by
///    `header.frame_size_bytes` so the following step parses the
///    next sync.
///
/// The iterator only accepts raw 16-bit encodings (`RawBigEndian` /
/// `RawLittleEndian`). When [`find_next_sync`] locates a 14-bit
/// sync, the iterator yields a single [`Error::UnsupportedFourteenBit`]
/// item and then terminates. Callers walking 14-bit container
/// streams must externally partition the input into frame-sized
/// slices and feed each slice to [`crate::parse_frame_header_14bit`]
/// directly. See `README.md`'s round-6 docs gap #7 for the
/// container-byte-advance rule.
#[derive(Debug)]
pub struct FrameIterator<'a> {
    bytes: &'a [u8],
    cursor: usize,
    done: bool,
}

impl<'a> FrameIterator<'a> {
    /// Construct an iterator positioned at byte 0 of `bytes`.
    pub fn new(bytes: &'a [u8]) -> Self {
        FrameIterator {
            bytes,
            cursor: 0,
            done: false,
        }
    }

    /// Current cursor offset. After a successful step the cursor
    /// points at the next frame's expected sync byte (or one past
    /// end-of-buffer if the previous frame was the last). After a
    /// failure the cursor stays at the point of failure.
    pub fn cursor(&self) -> usize {
        self.cursor
    }
}

impl<'a> Iterator for FrameIterator<'a> {
    type Item = Result<FrameView<'a>>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }
        let sync_match = find_next_sync(self.bytes, self.cursor)?;
        match sync_match.encoding {
            SyncWordEncoding::FourteenBitBigEndian | SyncWordEncoding::FourteenBitLittleEndian => {
                // 14-bit container streams need an out-of-band
                // container-byte advance rule which the wiki snapshot
                // does not enumerate. Surface the limitation and
                // terminate so the caller switches to the
                // single-frame 14-bit entry point.
                self.done = true;
                return Some(Err(Error::UnsupportedFourteenBit));
            }
            _ => {}
        }
        let off = sync_match.offset;
        let parse_result = parse_frame_header(&self.bytes[off..]);
        let hdr = match parse_result {
            Ok(h) => h,
            Err(e) => {
                self.done = true;
                self.cursor = off;
                return Some(Err(e));
            }
        };
        let len = hdr.frame_size_bytes as usize;
        if off + len > self.bytes.len() {
            // Header says the frame extends past end-of-buffer. We
            // still surface the header (the caller may want to know
            // the truncation occurred at this offset with this
            // size), but mark the iterator done.
            self.done = true;
            self.cursor = off;
            return Some(Err(Error::UnexpectedEof));
        }
        let view = FrameView {
            header: hdr,
            offset: off,
            len,
            data: &self.bytes[off..off + len],
        };
        self.cursor = off + len;
        Some(Ok(view))
    }
}

/// Convenience constructor — equivalent to [`FrameIterator::new`].
pub fn iter_frames(bytes: &[u8]) -> FrameIterator<'_> {
    FrameIterator::new(bytes)
}

/// Reason a [`FrameIteratorResync`] step discarded a candidate sync
/// position and resumed scanning one byte further.
///
/// Surfaced through [`ResyncEvent::cause`]; callers can route on the
/// variant (e.g. log truncated tails differently from false-positive
/// syncs mid-buffer).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResyncCause {
    /// The header bits that follow the sync failed one of the
    /// structural bounds the spec mandates (NBLKS &lt; 5 → [`Error::BlockCountOutOfRange`],
    /// frame size &lt; 95 → [`Error::FrameSizeOutOfRange`]) — strong
    /// evidence the sync was a coincidental byte sequence inside a
    /// previous frame's payload rather than the start of a real
    /// frame.
    StructuralBoundFailed(Error),
    /// The candidate sync sat too close to end-of-buffer for the
    /// header bit-width to fit — the parser returned
    /// [`Error::UnexpectedEof`] while reading the header itself.
    HeaderEof,
    /// The header parsed successfully but its declared
    /// `frame_size_bytes` runs past end-of-buffer. The fail-fast
    /// [`FrameIterator`] reports this as
    /// [`Error::UnexpectedEof`] and terminates; the resync iterator
    /// treats the candidate as a false-positive sync and resumes
    /// scanning at `offset + 1`. A genuine truncated tail will
    /// produce one or more `FrameLengthOverrunsBuffer` events near
    /// the end of the input and no further successful frames.
    FrameLengthOverrunsBuffer {
        /// `header.frame_size_bytes` at the discarded position.
        declared_len: u16,
    },
    /// A 14-bit sync was encountered. The fail-fast iterator yields
    /// [`Error::UnsupportedFourteenBit`] and terminates; the resync
    /// iterator instead surfaces this event and continues scanning
    /// past the 14-bit sync, so a stream with intermixed encodings
    /// (e.g. a raw-16-bit stream with a few stray 14-bit-shaped byte
    /// sequences in payload) still walks to completion.
    FourteenBitSyncSkipped,
}

/// One step of [`FrameIteratorResync`] when a candidate sync turned
/// out to be a false positive.
///
/// `offset` and `encoding` come from the underlying
/// [`find_next_sync`] match; `cause` documents which check rejected
/// the candidate (see [`ResyncCause`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResyncEvent {
    /// Absolute byte offset of the discarded sync candidate within
    /// the iterator's input.
    pub offset: usize,
    /// Sync encoding that matched at `offset`.
    pub encoding: SyncWordEncoding,
    /// Why the candidate was rejected.
    pub cause: ResyncCause,
}

/// Error-tolerant counterpart to [`FrameIterator`].
///
/// The fail-fast [`FrameIterator`] surfaces a structural-bound
/// failure at a candidate sync (NBLKS &lt; 5, FSIZE &lt; 95) or a
/// header-truncation by yielding the parser's [`Error`] and
/// terminating — appropriate for callers walking a known-good raw
/// `.dts` stream where any malformed sync is a hard fault.
///
/// `FrameIteratorResync` instead treats those cases as **evidence
/// the candidate sync was a false positive** (a coincidental
/// 4-byte sequence inside another frame's payload that matched the
/// 32-bit syncword), yielding a [`ResyncEvent`] documenting the
/// offset + cause and advancing the cursor by one byte so the scan
/// resumes past the spurious match. This lets stream-integrity
/// tooling walk a partially-corrupted stream past malformed-sync
/// patches and recover frames after the damage.
///
/// Iteration logic per step:
/// 1. Find the next sync at or after the current cursor via
///    [`find_next_sync`]. If none, the iterator ends (yields
///    `None`).
/// 2. If the sync is a 14-bit variant: yield
///    [`ResyncCause::FourteenBitSyncSkipped`] and advance the
///    cursor to `offset + 1` (rather than terminating like
///    [`FrameIterator`] does).
/// 3. Parse the header at the matched offset. On structural
///    failure (`BlockCountOutOfRange` / `FrameSizeOutOfRange` /
///    `UnexpectedEof` while reading the header bits) yield a
///    [`ResyncEvent`] with the appropriate [`ResyncCause`] and
///    advance the cursor to `offset + 1`.
/// 4. If the header parses but `frame_size_bytes` overruns
///    end-of-buffer, yield
///    [`ResyncCause::FrameLengthOverrunsBuffer`] with the declared
///    length and advance the cursor to `offset + 1`. A genuine
///    truncated tail will therefore emit one or more overrun
///    events and then iteration ends naturally when no further
///    sync is found.
/// 5. Otherwise yield `Ok(FrameView)` and advance the cursor by
///    `frame_size_bytes`.
///
/// This iterator is only meaningful for the raw 16-bit encodings
/// (the [`FrameIterator`] docs spell out the 14-bit container-byte
/// advance gap). 14-bit syncs are skipped per step (2) above
/// rather than walked, so a raw-16-bit stream that contains stray
/// 14-bit-shaped byte sequences in payload still walks past them.
#[derive(Debug)]
pub struct FrameIteratorResync<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> FrameIteratorResync<'a> {
    /// Construct a resync iterator positioned at byte 0 of `bytes`.
    pub fn new(bytes: &'a [u8]) -> Self {
        FrameIteratorResync { bytes, cursor: 0 }
    }

    /// Current cursor offset (advances on every yielded step,
    /// whether `Ok` or `Err`).
    pub fn cursor(&self) -> usize {
        self.cursor
    }
}

impl<'a> Iterator for FrameIteratorResync<'a> {
    type Item = core::result::Result<FrameView<'a>, ResyncEvent>;

    fn next(&mut self) -> Option<Self::Item> {
        let sync_match = find_next_sync(self.bytes, self.cursor)?;
        let off = sync_match.offset;
        if matches!(
            sync_match.encoding,
            SyncWordEncoding::FourteenBitBigEndian | SyncWordEncoding::FourteenBitLittleEndian
        ) {
            self.cursor = off + 1;
            return Some(Err(ResyncEvent {
                offset: off,
                encoding: sync_match.encoding,
                cause: ResyncCause::FourteenBitSyncSkipped,
            }));
        }
        match parse_frame_header(&self.bytes[off..]) {
            Ok(hdr) => {
                let len = hdr.frame_size_bytes as usize;
                if off + len > self.bytes.len() {
                    self.cursor = off + 1;
                    return Some(Err(ResyncEvent {
                        offset: off,
                        encoding: sync_match.encoding,
                        cause: ResyncCause::FrameLengthOverrunsBuffer {
                            declared_len: hdr.frame_size_bytes,
                        },
                    }));
                }
                let view = FrameView {
                    header: hdr,
                    offset: off,
                    len,
                    data: &self.bytes[off..off + len],
                };
                self.cursor = off + len;
                Some(Ok(view))
            }
            Err(e) => {
                let cause = match e {
                    Error::UnexpectedEof => ResyncCause::HeaderEof,
                    Error::BlockCountOutOfRange { .. } | Error::FrameSizeOutOfRange { .. } => {
                        ResyncCause::StructuralBoundFailed(e)
                    }
                    // `find_next_sync` already filtered out `NoSync`;
                    // the 14-bit branch is handled above so
                    // `UnsupportedFourteenBit` can't fire here;
                    // `FieldOutOfRange` is encoder-only. Treat any
                    // other variant as a structural false-positive
                    // so the iterator stays total over future enum
                    // extensions.
                    _ => ResyncCause::StructuralBoundFailed(e),
                };
                self.cursor = off + 1;
                Some(Err(ResyncEvent {
                    offset: off,
                    encoding: sync_match.encoding,
                    cause,
                }))
            }
        }
    }
}

/// Convenience constructor — equivalent to
/// [`FrameIteratorResync::new`].
///
/// See [`FrameIteratorResync`] for the resync vs fail-fast
/// trade-off and the per-step yield contract.
pub fn iter_frames_resync(bytes: &[u8]) -> FrameIteratorResync<'_> {
    FrameIteratorResync::new(bytes)
}

/// Scan an entire byte buffer and return every documented DTS sync
/// occurrence it contains.
///
/// This is the bulk-scan counterpart to [`find_next_sync`]: instead of
/// returning the first hit at or after a cursor, it walks the buffer
/// from start to end and collects every position where one of the four
/// documented sync sequences appears. Useful for tooling that needs to
/// validate stream integrity, count frames without parsing them, or
/// build an index of resync points up front.
///
/// The scan honours the same matching rules as [`find_next_sync`]:
///
/// - `7F FE 80 01` — raw big-endian (4 bytes).
/// - `FE 7F 01 80` — raw little-endian (4 bytes).
/// - `1F FF E8 00 07 Fx` — 14-bit packed big-endian (6 bytes, matched
///   on the lower 14 bits of each container per [`detect_sync`]).
/// - `FF 1F 00 E8 Fx 07` — 14-bit packed little-endian (6 bytes,
///   matched on the lower 14 bits of each container).
///
/// Each yielded [`SyncMatch`] reports the absolute byte offset of the
/// first sync byte plus the matched encoding. Overlapping matches are
/// not possible because the four sync sequences differ in their first
/// two bytes (`7F`/`FE`/`1F`/`FF`), but the scan still advances
/// one byte at a time so adjacent sync occurrences (one ending and the
/// next starting on consecutive bytes) are both reported.
///
/// The scan is `O(n)` — each byte is visited at most twice by
/// [`detect_sync`] (one 4-byte raw check, one 6-byte 14-bit check).
/// Callers that only need the first sync should prefer
/// [`find_next_sync`] to avoid materialising the result vector.
///
/// ## Example
///
/// ```
/// use vuio_codec_dts::{find_all_syncs, SyncWordEncoding};
///
/// let mut buf = vec![0u8; 32];
/// buf[0..4].copy_from_slice(&[0x7F, 0xFE, 0x80, 0x01]);
/// buf[8..12].copy_from_slice(&[0xFE, 0x7F, 0x01, 0x80]);
///
/// let matches = find_all_syncs(&buf);
/// assert_eq!(matches.len(), 2);
/// assert_eq!(matches[0].offset, 0);
/// assert_eq!(matches[0].encoding, SyncWordEncoding::RawBigEndian);
/// assert_eq!(matches[1].offset, 8);
/// assert_eq!(matches[1].encoding, SyncWordEncoding::RawLittleEndian);
/// ```
pub fn find_all_syncs(bytes: &[u8]) -> Vec<SyncMatch> {
    let mut out = Vec::new();
    let mut cursor = 0usize;
    while let Some(m) = find_next_sync(bytes, cursor) {
        out.push(m);
        // Advance by one byte so consecutive (non-overlapping) syncs
        // are both reported. The four documented sync prefixes start
        // with `7F`/`FE`/`1F`/`FF`, all distinct, so no two sync
        // sequences can overlap; a one-byte step is therefore both
        // sufficient and minimal.
        cursor = m.offset + 1;
    }
    out
}

/// Lazy streaming iterator over every DTS sync sequence in a byte
/// buffer.
///
/// Same matching rules and walk order as [`find_all_syncs`] — the
/// only difference is that this iterator does **not** materialise the
/// full result vector. It walks the buffer one [`find_next_sync`]
/// hop at a time, yielding each [`SyncMatch`] as it is found and
/// stopping when [`find_next_sync`] returns `None`.
///
/// Useful when the caller only needs to:
///
/// - act on syncs in order (e.g. stream-integrity tooling that prints
///   each resync point to a log as it walks),
/// - stop early after the first N matches (`iter_syncs(bytes).take(8)`),
/// - or chain a filter / sniff through standard
///   [`Iterator`] combinators (e.g.
///   `iter_syncs(bytes).filter(|m| m.encoding.is_raw_16bit())`)
///
/// without paying the upfront allocation that [`find_all_syncs`]
/// incurs. For a workload that consumes every match anyway,
/// `find_all_syncs(bytes)` and `iter_syncs(bytes).collect()` produce
/// the same `Vec<SyncMatch>` at the same `O(n)` cost — pick the bulk
/// helper when the result is needed as a slice, the iterator when the
/// caller is fine with element-by-element consumption.
///
/// The iterator is non-overlapping: after yielding a match at
/// `offset`, scanning resumes at `offset + 1`, identical to
/// [`find_all_syncs`]. The four documented sync prefixes have
/// distinct first bytes (`7F` / `FE` / `1F` / `FF`), so no two real
/// sync sequences can overlap — a one-byte step is both sufficient
/// and minimal. Adjacent syncs (one ending and the next starting on
/// consecutive bytes) are both reported.
///
/// ## Example
///
/// ```
/// use vuio_codec_dts::{iter_syncs, SyncWordEncoding};
///
/// let mut buf = vec![0u8; 32];
/// buf[0..4].copy_from_slice(&[0x7F, 0xFE, 0x80, 0x01]);
/// buf[8..12].copy_from_slice(&[0xFE, 0x7F, 0x01, 0x80]);
///
/// let mut it = iter_syncs(&buf);
/// let first = it.next().unwrap();
/// assert_eq!(first.offset, 0);
/// assert_eq!(first.encoding, SyncWordEncoding::RawBigEndian);
/// let second = it.next().unwrap();
/// assert_eq!(second.offset, 8);
/// assert_eq!(second.encoding, SyncWordEncoding::RawLittleEndian);
/// assert!(it.next().is_none());
/// ```
#[derive(Debug)]
pub struct SyncIterator<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> SyncIterator<'a> {
    /// Construct an iterator positioned at byte 0 of `bytes`.
    pub fn new(bytes: &'a [u8]) -> Self {
        SyncIterator { bytes, cursor: 0 }
    }

    /// Current scan cursor. After yielding a match at `offset` the
    /// cursor advances to `offset + 1`; after exhausting the input it
    /// rests at the byte position [`find_next_sync`] gave up at.
    pub fn cursor(&self) -> usize {
        self.cursor
    }
}

impl<'a> Iterator for SyncIterator<'a> {
    type Item = SyncMatch;

    fn next(&mut self) -> Option<Self::Item> {
        let m = find_next_sync(self.bytes, self.cursor)?;
        // Same one-byte advance as `find_all_syncs`: the four sync
        // prefixes have distinct first bytes so non-overlapping
        // matches at adjacent offsets are still both reported.
        self.cursor = m.offset + 1;
        Some(m)
    }
}

/// Convenience constructor — equivalent to [`SyncIterator::new`].
///
/// See [`SyncIterator`] for the streaming vs bulk-scan trade-off
/// against [`find_all_syncs`].
pub fn iter_syncs(bytes: &[u8]) -> SyncIterator<'_> {
    SyncIterator::new(bytes)
}

// ---------------------------------------------------------------------
// Round 192 — 14-bit container-byte frame iterator (`iter_frames_14bit`)
//
// The fail-fast [`FrameIterator`] from round 6 walks raw-16-bit streams
// only; a 14-bit sync at the cursor yields `Error::UnsupportedFourteenBit`
// and terminates because the iterator's `frame_size_bytes` advance only
// makes sense in the unpacked domain. Round 189 added the analytical
// half — [`DtsFrameHeader::frame_size_container_bytes`] — which converts
// the unpacked-domain frame size to a container-domain byte count for
// the 14-bit encodings. Round 192 wires that accessor into a dedicated
// 14-bit frame iterator: each step calls [`parse_frame_header_14bit`]
// at the matched container offset (which internally unpacks just enough
// containers to read the 13/15-byte unpacked header window), then steps
// the cursor by `frame_size_container_bytes(encoding)` container bytes.
//
// We deliberately introduce a separate [`FrameView14`] type rather than
// reusing [`FrameView`] because the semantics of `len` differ: in the
// 14-bit case `len` is a container-byte advance derived from the
// 14-bit `frame_size_container_bytes` formula (not the raw
// `header.frame_size_bytes` field), and `data` borrows the
// container-byte window — not the unpacked logical bytes. Sharing the
// `FrameView` type would require silently overloading `len`'s meaning,
// which is exactly the kind of footgun the wiki/`docs/` ambiguity asks
// us to avoid.
// ---------------------------------------------------------------------

/// One step of a [`FrameIterator14`] over a 14-bit-packed DTS Core
/// container stream.
///
/// Field semantics differ from [`FrameView`] in two specific ways:
///
/// - `len` is the **container-byte** advance produced by
///   [`DtsFrameHeader::frame_size_container_bytes`] — not the
///   `header.frame_size_bytes` field (which is the unpacked logical
///   byte count). The cursor steps by `len` container bytes to land on
///   the next sync.
/// - `data` is the container-byte window of the frame (`bytes[offset..
///   offset + len]`), not the unpacked logical bytes. Callers that
///   want to decode the SUBFRAMES region must run
///   [`crate::unpack_14bit_to_16bit`] on `data` first.
#[derive(Debug, Clone, Copy)]
pub struct FrameView14<'a> {
    /// Parsed header. `header.sync_word_encoding` is one of
    /// [`SyncWordEncoding::FourteenBitBigEndian`] /
    /// [`SyncWordEncoding::FourteenBitLittleEndian`] (the iterator
    /// only walks 14-bit streams).
    pub header: DtsFrameHeader,
    /// Absolute container-byte offset of the frame's first sync byte
    /// within the input passed to [`iter_frames_14bit`].
    pub offset: usize,
    /// Container-byte length of the frame, equal to
    /// `header.frame_size_container_bytes(header.sync_word_encoding)`.
    /// Always even (per ETSI §6.1.3.1's 28-bit / two-container-word
    /// boundary invariant) and strictly greater than the unpacked
    /// `header.frame_size_bytes` count (because 14 logical bits per
    /// 16 container bits scales the span up by 16/14 ≈ 1.143).
    pub len: usize,
    /// Borrowed container bytes (`bytes[offset..offset + len]`).
    pub data: &'a [u8],
}

/// Iterator that walks a 14-bit-packed DTS Core container stream
/// frame by frame.
///
/// Each [`Iterator::next`] step:
/// 1. Calls [`find_next_sync`] from the current cursor and accepts
///    only 14-bit syncs ([`SyncWordEncoding::FourteenBitBigEndian`] /
///    [`SyncWordEncoding::FourteenBitLittleEndian`]). A raw 16-bit
///    sync at the cursor yields a single [`Error::UnsupportedRaw16Bit`]
///    item (the symmetric counterpart to the round-6 [`FrameIterator`]
///    behaviour on 14-bit syncs) and the iterator terminates.
/// 2. Calls [`parse_frame_header_14bit`] at the located offset. The
///    parser internally unpacks the first ~18 container bytes (=
///    9 containers = 126 payload bits ≥ the 120-bit worst-case header
///    window) and reads the 13 or 15-byte unpacked header window. A
///    parse failure (truncated header, NBLKS or FSIZE out of range)
///    is returned as the next item's `Err` variant; subsequent calls
///    then return `None`.
/// 3. On success, computes the container-byte advance via
///    [`DtsFrameHeader::frame_size_container_bytes`] (the round-189
///    analytical formula:
///    `2 * ceil(frame_size_bytes * 8 / 14)` for the 14-bit
///    encodings), yields a [`FrameView14`] borrowing the frame's
///    container bytes from the input, and advances the cursor by
///    that count so the following step parses the next sync.
///
/// Just like [`FrameIterator`], this iterator resyncs after leading
/// garbage by calling [`find_next_sync`] up-front rather than
/// assuming `bytes[0]` is the first sync byte. A stream with
/// intermixed BE / LE 14-bit syncs walks correctly because each step
/// re-reads the matched encoding from the [`SyncMatch`].
///
/// See [`FrameIterator`] for the raw-16-bit counterpart and the
/// round-6 docs gap #7 for the formula's derivation.
#[derive(Debug)]
pub struct FrameIterator14<'a> {
    bytes: &'a [u8],
    cursor: usize,
    done: bool,
}

impl<'a> FrameIterator14<'a> {
    /// Construct an iterator positioned at byte 0 of `bytes`.
    pub fn new(bytes: &'a [u8]) -> Self {
        FrameIterator14 {
            bytes,
            cursor: 0,
            done: false,
        }
    }

    /// Current container-byte cursor. After a successful step the
    /// cursor points at the next frame's expected sync byte (or one
    /// past end-of-buffer if the previous frame was the last). After
    /// a failure the cursor stays at the point of failure.
    pub fn cursor(&self) -> usize {
        self.cursor
    }
}

impl<'a> Iterator for FrameIterator14<'a> {
    type Item = Result<FrameView14<'a>>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }
        let sync_match = find_next_sync(self.bytes, self.cursor)?;
        match sync_match.encoding {
            SyncWordEncoding::FourteenBitBigEndian | SyncWordEncoding::FourteenBitLittleEndian => {}
            // Symmetric counterpart to the round-6 `iter_frames`
            // behaviour on 14-bit syncs: this iterator only walks
            // 14-bit container streams, so a raw 16-bit sync is
            // out-of-domain. Surface the mismatch and terminate.
            _ => {
                self.done = true;
                return Some(Err(Error::UnsupportedRaw16Bit));
            }
        }
        let off = sync_match.offset;
        let enc = sync_match.encoding;
        let parse_result = parse_frame_header_14bit(&self.bytes[off..]);
        let hdr = match parse_result {
            Ok(h) => h,
            Err(e) => {
                self.done = true;
                self.cursor = off;
                return Some(Err(e));
            }
        };
        // Round-189 analytical formula: container-byte advance for the
        // 14-bit encodings is `2 * ceil(frame_size_bytes * 8 / 14)`.
        let len = hdr.frame_size_container_bytes(enc) as usize;
        if off + len > self.bytes.len() {
            self.done = true;
            self.cursor = off;
            return Some(Err(Error::UnexpectedEof));
        }
        let view = FrameView14 {
            header: hdr,
            offset: off,
            len,
            data: &self.bytes[off..off + len],
        };
        self.cursor = off + len;
        Some(Ok(view))
    }
}

/// Convenience constructor — equivalent to [`FrameIterator14::new`].
///
/// Walks a 14-bit-packed DTS Core container stream (BE or LE — each
/// frame's encoding is re-read from the matched sync, so mixed-encoding
/// inputs walk correctly). For raw 16-bit streams use [`iter_frames`].
pub fn iter_frames_14bit(bytes: &[u8]) -> FrameIterator14<'_> {
    FrameIterator14::new(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FrameType;

    const RAW_BE_SYNC: [u8; 4] = [0x7F, 0xFE, 0x80, 0x01];
    const RAW_LE_SYNC: [u8; 4] = [0xFE, 0x7F, 0x01, 0x80];

    #[test]
    fn find_next_sync_at_offset_zero() {
        let mut buf = vec![0u8; 16];
        buf[..4].copy_from_slice(&RAW_BE_SYNC);
        let m = find_next_sync(&buf, 0).unwrap();
        assert_eq!(m.offset, 0);
        assert_eq!(m.encoding, SyncWordEncoding::RawBigEndian);
    }

    #[test]
    fn find_next_sync_skips_leading_garbage() {
        let mut buf = vec![0xAAu8; 32];
        buf[7..11].copy_from_slice(&RAW_BE_SYNC);
        let m = find_next_sync(&buf, 0).unwrap();
        assert_eq!(m.offset, 7);
        assert_eq!(m.encoding, SyncWordEncoding::RawBigEndian);
    }

    #[test]
    fn find_next_sync_le_variant() {
        let mut buf = vec![0u8; 16];
        buf[3..7].copy_from_slice(&RAW_LE_SYNC);
        let m = find_next_sync(&buf, 0).unwrap();
        assert_eq!(m.offset, 3);
        assert_eq!(m.encoding, SyncWordEncoding::RawLittleEndian);
    }

    #[test]
    fn find_next_sync_14bit_be() {
        let mut buf = vec![0u8; 16];
        buf[5..11].copy_from_slice(&[0x1F, 0xFF, 0xE8, 0x00, 0x07, 0xFA]);
        let m = find_next_sync(&buf, 0).unwrap();
        assert_eq!(m.offset, 5);
        assert_eq!(m.encoding, SyncWordEncoding::FourteenBitBigEndian);
    }

    #[test]
    fn find_next_sync_14bit_le() {
        let mut buf = vec![0u8; 16];
        buf[5..11].copy_from_slice(&[0xFF, 0x1F, 0x00, 0xE8, 0xF3, 0x07]);
        let m = find_next_sync(&buf, 0).unwrap();
        assert_eq!(m.offset, 5);
        assert_eq!(m.encoding, SyncWordEncoding::FourteenBitLittleEndian);
    }

    #[test]
    fn find_next_sync_honours_start_offset() {
        // Two syncs in the buffer; start past the first.
        let mut buf = vec![0xAAu8; 32];
        buf[2..6].copy_from_slice(&RAW_BE_SYNC);
        buf[20..24].copy_from_slice(&RAW_BE_SYNC);
        let m = find_next_sync(&buf, 7).unwrap();
        assert_eq!(m.offset, 20);
    }

    #[test]
    fn find_next_sync_returns_none_when_absent() {
        let buf = vec![0xAAu8; 64];
        assert_eq!(find_next_sync(&buf, 0), None);
    }

    #[test]
    fn find_next_sync_returns_none_when_start_past_end() {
        let buf = vec![0u8; 16];
        assert_eq!(find_next_sync(&buf, 100), None);
    }

    #[test]
    fn find_next_sync_returns_none_when_only_partial_sync_at_tail() {
        // Three bytes of an in-progress raw sync (`7F FE 80`) but no
        // fourth byte — the scanner walks up to length-4, finds
        // nothing, and returns None.
        let mut buf = vec![0xAAu8; 8];
        buf[5..8].copy_from_slice(&[0x7F, 0xFE, 0x80]);
        assert_eq!(find_next_sync(&buf, 0), None);
    }

    // ---------------------------------------------------------------
    // Round 138 — FrameView::payload()
    // ---------------------------------------------------------------

    /// Hand-build a 95-byte raw-BE termination frame (NBLKS=5,
    /// FSIZE=95, crc_present=0) padded with zeros for the SUBFRAMES
    /// region; then confirm `FrameView::payload()` returns exactly
    /// `frame.len - header_byte_length()` bytes (= 95 - 13 = 82).
    #[test]
    fn frame_view_payload_slice_length_matches_header_boundary_no_crc() {
        // The base 13-byte header window is followed by 82 bytes of
        // SUBFRAMES region we fill with a distinctive 0xCD pattern
        // so the slice content can be checked too.
        let mut buf = vec![0u8; 95];
        // Sync.
        buf[0..4].copy_from_slice(&[0x7F, 0xFE, 0x80, 0x01]);
        // FTYPE=0 (termination, MSB), SHORT=0, CRC_PRESENT=0,
        // NBLKS=5 (7 bits), FSIZE-1=94 (14 bits = frame_size 95),
        // AMODE=0 (6 bits), SFREQ=0 (4 bits), RATE=0 (5 bits) +
        // 13 zero trailing bits + 16 zero post-CRC bits = 75 bits
        // after the sync = 13 bytes total.
        // Easier: just call build_be_header equivalent through
        // parse_frame_header on a synthesised buffer.
        // Bytes layout (after sync):
        //  byte 4: FTYPE(1)=0 SHORT(5)=0 CRC_PRESENT(1)=0 NBLKS_hi(1)=0 -> 0b00000000 = 0x00
        //  byte 5: NBLKS_lo(6)=000101 FSIZE_hi(2)=00 -> 0b00010100 = 0x14
        //  byte 6: FSIZE_mid(8)=00010111 -> 0x17  (FSIZE-1 = 94 = 0b00_00000101_1110, hi 2 bits=00, mid 8=00010111? — recompute below)
        // Rather than hand-bit-fiddle, sidestep the layout: use
        // `parse_frame_header` on a 95-byte buffer we synthesise via
        // the header.rs test helper indirectly by re-deriving the
        // bit layout here. Simpler: directly construct via a tiny
        // local builder.
        fn push(bv: &mut Vec<bool>, value: u32, width: u32) {
            for i in (0..width).rev() {
                bv.push(((value >> i) & 1) == 1);
            }
        }
        let mut bv: Vec<bool> = Vec::new();
        push(&mut bv, 0x7FFE_8001, 32);
        push(&mut bv, 0, 1); // ftype = termination
        push(&mut bv, 0, 5); // sample_count_m1
        push(&mut bv, 0, 1); // crc_present
        push(&mut bv, 5, 7); // nblks
        push(&mut bv, 94, 14); // fsize-1 = 94 -> frame_size = 95
        push(&mut bv, 0, 6); // amode
        push(&mut bv, 0, 4); // sfreq
        push(&mut bv, 0, 5); // rate
        push(&mut bv, 0, 13); // trailing flags
        push(&mut bv, 0, 16); // post-CRC window
        while !bv.len().is_multiple_of(8) {
            bv.push(false);
        }
        // Convert bit-vector to bytes (MSB-first).
        for (i, chunk) in bv.chunks(8).enumerate() {
            let mut b: u8 = 0;
            for (k, bit) in chunk.iter().enumerate() {
                if *bit {
                    b |= 1 << (7 - k);
                }
            }
            buf[i] = b;
        }
        // Fill the SUBFRAMES region (bytes 13..95) with a pattern so
        // the payload-slice contents can be verified.
        for byte in buf.iter_mut().skip(13) {
            *byte = 0xCD;
        }

        let mut it = iter_frames(&buf);
        let view = it.next().expect("frame must yield").expect("must parse");
        assert_eq!(view.offset, 0);
        assert_eq!(view.len, 95);
        assert_eq!(view.header.frame_size_bytes, 95);
        assert!(!view.header.crc_present);
        assert_eq!(view.header.header_byte_length(), 13);

        let payload = view.payload();
        assert_eq!(payload.len(), 95 - 13);
        assert!(payload.iter().all(|&b| b == 0xCD));
        // No more frames in the buffer.
        assert!(it.next().is_none());
    }

    /// `FrameView::payload()` shifts by 2 bytes (15 vs 13) when
    /// `crc_present == 1` because the optional HEADER_CRC slot
    /// extends the header window from 104 to 120 bits.
    #[test]
    fn frame_view_payload_offset_shifts_when_crc_present() {
        // Build a 95-byte crc-present termination frame.
        let mut buf = vec![0u8; 95];
        fn push(bv: &mut Vec<bool>, value: u32, width: u32) {
            for i in (0..width).rev() {
                bv.push(((value >> i) & 1) == 1);
            }
        }
        let mut bv: Vec<bool> = Vec::new();
        push(&mut bv, 0x7FFE_8001, 32);
        push(&mut bv, 0, 1);
        push(&mut bv, 0, 5);
        push(&mut bv, 1, 1); // crc_present = true
        push(&mut bv, 5, 7);
        push(&mut bv, 94, 14);
        push(&mut bv, 0, 6);
        push(&mut bv, 0, 4);
        push(&mut bv, 0, 5);
        push(&mut bv, 0, 13);
        push(&mut bv, 0xABCD, 16); // header_crc
        push(&mut bv, 0, 16); // post-CRC window
        while !bv.len().is_multiple_of(8) {
            bv.push(false);
        }
        for (i, chunk) in bv.chunks(8).enumerate() {
            let mut b: u8 = 0;
            for (k, bit) in chunk.iter().enumerate() {
                if *bit {
                    b |= 1 << (7 - k);
                }
            }
            buf[i] = b;
        }
        // Fill bytes 15..95 with a distinct pattern.
        for byte in buf.iter_mut().skip(15) {
            *byte = 0xEF;
        }

        let view = iter_frames(&buf).next().unwrap().unwrap();
        assert!(view.header.crc_present);
        assert_eq!(view.header.header_byte_length(), 15);
        let payload = view.payload();
        assert_eq!(payload.len(), 95 - 15);
        assert!(payload.iter().all(|&b| b == 0xEF));
    }

    // ---------------------------------------------------------------
    // Round 151 — find_all_syncs() bulk-scan helper.
    //
    // Counterpart to find_next_sync() that returns every sync match in
    // a buffer, useful for stream-integrity tooling that needs to know
    // about every resync point up front rather than walking one at a
    // time.
    // ---------------------------------------------------------------

    #[test]
    fn find_all_syncs_empty_buffer_returns_empty_vec() {
        let buf: [u8; 0] = [];
        assert!(find_all_syncs(&buf).is_empty());
    }

    #[test]
    fn find_all_syncs_no_sync_returns_empty_vec() {
        let buf = vec![0xAAu8; 64];
        assert!(find_all_syncs(&buf).is_empty());
    }

    #[test]
    fn find_all_syncs_returns_single_match_for_single_sync() {
        let mut buf = vec![0u8; 16];
        buf[0..4].copy_from_slice(&RAW_BE_SYNC);
        let matches = find_all_syncs(&buf);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].offset, 0);
        assert_eq!(matches[0].encoding, SyncWordEncoding::RawBigEndian);
    }

    #[test]
    fn find_all_syncs_mixed_raw_be_and_le_in_one_buffer() {
        // Stream pattern: BE at 0, LE at 8, BE at 16, LE at 24.
        let mut buf = vec![0xAAu8; 32];
        buf[0..4].copy_from_slice(&RAW_BE_SYNC);
        buf[8..12].copy_from_slice(&RAW_LE_SYNC);
        buf[16..20].copy_from_slice(&RAW_BE_SYNC);
        buf[24..28].copy_from_slice(&RAW_LE_SYNC);
        let matches = find_all_syncs(&buf);
        assert_eq!(matches.len(), 4);
        assert_eq!(matches[0].offset, 0);
        assert_eq!(matches[0].encoding, SyncWordEncoding::RawBigEndian);
        assert_eq!(matches[1].offset, 8);
        assert_eq!(matches[1].encoding, SyncWordEncoding::RawLittleEndian);
        assert_eq!(matches[2].offset, 16);
        assert_eq!(matches[2].encoding, SyncWordEncoding::RawBigEndian);
        assert_eq!(matches[3].offset, 24);
        assert_eq!(matches[3].encoding, SyncWordEncoding::RawLittleEndian);
    }

    #[test]
    fn find_all_syncs_with_all_four_encodings() {
        // Pack one of each documented sync encoding into a single
        // buffer; find_all_syncs must report all four.
        let mut buf = vec![0xAAu8; 48];
        buf[0..4].copy_from_slice(&RAW_BE_SYNC);
        buf[8..12].copy_from_slice(&RAW_LE_SYNC);
        buf[16..22].copy_from_slice(&[0x1F, 0xFF, 0xE8, 0x00, 0x07, 0xFA]);
        buf[24..30].copy_from_slice(&[0xFF, 0x1F, 0x00, 0xE8, 0xF3, 0x07]);
        let matches = find_all_syncs(&buf);
        assert_eq!(matches.len(), 4);
        assert_eq!(matches[0].encoding, SyncWordEncoding::RawBigEndian);
        assert_eq!(matches[1].encoding, SyncWordEncoding::RawLittleEndian);
        assert_eq!(matches[2].encoding, SyncWordEncoding::FourteenBitBigEndian);
        assert_eq!(
            matches[3].encoding,
            SyncWordEncoding::FourteenBitLittleEndian
        );
    }

    #[test]
    fn find_all_syncs_consecutive_back_to_back_frames() {
        // Two raw-BE syncs adjacent at offsets 0 and 4 — no gap.
        let mut buf = vec![0u8; 16];
        buf[0..4].copy_from_slice(&RAW_BE_SYNC);
        buf[4..8].copy_from_slice(&RAW_BE_SYNC);
        let matches = find_all_syncs(&buf);
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].offset, 0);
        assert_eq!(matches[1].offset, 4);
    }

    #[test]
    fn find_all_syncs_walks_full_buffer_independent_of_starting_garbage() {
        // 5 bytes of garbage, then BE sync at 5, more garbage, LE sync
        // at 20, trailing tail of garbage.
        let mut buf = vec![0xAAu8; 32];
        buf[5..9].copy_from_slice(&RAW_BE_SYNC);
        buf[20..24].copy_from_slice(&RAW_LE_SYNC);
        let matches = find_all_syncs(&buf);
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].offset, 5);
        assert_eq!(matches[1].offset, 20);
    }

    /// Cross-check parity with `find_next_sync` looped from `offset+1`
    /// — `find_all_syncs` must agree with the explicit loop on every
    /// match.
    #[test]
    fn find_all_syncs_matches_find_next_sync_loop() {
        let mut buf = vec![0xAAu8; 64];
        buf[3..7].copy_from_slice(&RAW_BE_SYNC);
        buf[15..19].copy_from_slice(&RAW_LE_SYNC);
        buf[40..44].copy_from_slice(&RAW_BE_SYNC);

        // find_next_sync loop reference.
        let mut reference: Vec<SyncMatch> = Vec::new();
        let mut cursor = 0usize;
        while let Some(m) = find_next_sync(&buf, cursor) {
            reference.push(m);
            cursor = m.offset + 1;
        }

        let bulk = find_all_syncs(&buf);
        assert_eq!(bulk, reference);
    }

    // ---------------------------------------------------------------
    // Round 151 — iter_frames coverage for raw-LE streams.
    //
    // The iterator's `frame_size_bytes`-based advance is documented as
    // applying to "raw 16-bit encodings" (both RawBigEndian AND
    // RawLittleEndian — the FSIZE field is the byte length of the
    // raw 16-bit-per-word on-wire stream, which is byte-equivalent
    // between BE and LE because the LE form is just a pairwise
    // byte-swap of the BE form). The existing
    // `multi_frame_iter.rs::iter_frames_walks_all_five_frames_in_fixture`
    // test exercises the BE path against the ffmpeg fixture; the
    // raw-LE path is exercised here by byte-swapping that fixture
    // (synthesised inline so the iterator's LE walk is covered without
    // adding a new fixture file).
    // ---------------------------------------------------------------

    /// Build a multi-frame raw-LE byte buffer by byte-swapping a
    /// raw-BE frame buffer pairwise. The frame layout (count, sizes,
    /// SUBFRAMES contents) is preserved because raw-LE is defined by
    /// the wiki as the 16-bit-word-swapped form of raw-BE.
    fn build_raw_le_two_frame_stream() -> Vec<u8> {
        // Two back-to-back 96-byte termination frames (NBLKS=5,
        // FSIZE=96). We hand-build the BE form via the same bit-
        // table the parser consumes and then word-swap to LE.
        let mut be = Vec::with_capacity(2 * 96);
        for _frame in 0..2 {
            let mut bv: Vec<bool> = Vec::new();
            fn push(bv: &mut Vec<bool>, value: u32, width: u32) {
                for i in (0..width).rev() {
                    bv.push(((value >> i) & 1) == 1);
                }
            }
            push(&mut bv, 0x7FFE_8001, 32);
            push(&mut bv, 0, 1); // ftype = termination
            push(&mut bv, 0, 5); // sample_count_m1
            push(&mut bv, 0, 1); // crc_present
            push(&mut bv, 5, 7); // nblks
            push(&mut bv, 95, 14); // fsize-1 = 95 -> frame_size = 96
            push(&mut bv, 0, 6); // amode
            push(&mut bv, 0, 4); // sfreq
            push(&mut bv, 0, 5); // rate
            push(&mut bv, 0, 13); // trailing flags
            push(&mut bv, 0, 16); // post-CRC window
            while !bv.len().is_multiple_of(8) {
                bv.push(false);
            }
            // Convert MSB-first bit-vector to bytes.
            let mut frame_bytes = vec![0u8; 96];
            for (i, chunk) in bv.chunks(8).enumerate() {
                let mut b: u8 = 0;
                for (k, bit) in chunk.iter().enumerate() {
                    if *bit {
                        b |= 1 << (7 - k);
                    }
                }
                frame_bytes[i] = b;
            }
            // Fill bytes 13..96 (SUBFRAMES region) with a distinct
            // per-frame pattern so payload checks can differentiate.
            for byte in frame_bytes.iter_mut().skip(13) {
                *byte = 0xCD;
            }
            be.extend_from_slice(&frame_bytes);
        }
        // Word-swap each 16-bit pair to produce the raw-LE form.
        for pair in be.as_chunks_mut::<2>().0 {
            pair.swap(0, 1);
        }
        be
    }

    #[test]
    fn iter_frames_walks_raw_le_two_frame_stream() {
        let buf = build_raw_le_two_frame_stream();
        // Sanity: starts with the canonical raw-LE sync.
        assert_eq!(&buf[..4], &[0xFE, 0x7F, 0x01, 0x80]);
        // Second frame's sync at offset 96.
        assert_eq!(&buf[96..100], &[0xFE, 0x7F, 0x01, 0x80]);

        let frames: Vec<_> = iter_frames(&buf)
            .collect::<core::result::Result<Vec<_>, _>>()
            .expect("raw-LE multi-frame stream must walk");
        assert_eq!(frames.len(), 2);

        for (i, frame) in frames.iter().enumerate() {
            assert_eq!(
                frame.header.sync_word_encoding,
                SyncWordEncoding::RawLittleEndian,
                "frame {i} encoding",
            );
            assert_eq!(frame.offset, i * 96);
            assert_eq!(frame.len, 96);
            assert_eq!(frame.header.frame_size_bytes, 96);
            assert_eq!(frame.header.blocks_per_frame, 5);
            assert_eq!(frame.header.frame_type, FrameType::Termination);
            assert!(!frame.header.crc_present);
            // The byte-length accessor returns the unpacked-bitstream
            // (raw-BE) header byte count regardless of the on-wire
            // sync encoding — 13 bytes when `crc_present == 0`.
            assert_eq!(frame.header.header_byte_length(), 13);
        }
    }

    /// The raw-LE walk is robust to leading garbage in the same way
    /// the raw-BE walk is — `find_next_sync` skips past unrelated
    /// bytes and lands on the first LE sync.
    #[test]
    fn iter_frames_raw_le_handles_leading_garbage() {
        let stream = build_raw_le_two_frame_stream();
        let mut prefixed = Vec::with_capacity(7 + stream.len());
        prefixed.extend_from_slice(&[0xAA; 7]);
        prefixed.extend_from_slice(&stream);

        let frames: Vec<_> = iter_frames(&prefixed)
            .collect::<core::result::Result<Vec<_>, _>>()
            .expect("garbage-prefixed raw-LE stream must still walk");
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].offset, 7);
        assert_eq!(frames[1].offset, 7 + 96);
        for f in &frames {
            assert_eq!(
                f.header.sync_word_encoding,
                SyncWordEncoding::RawLittleEndian
            );
        }
    }

    // -----------------------------------------------------------------
    // Round 159 — FrameIteratorResync.
    //
    // Error-tolerant counterpart to FrameIterator: a candidate sync
    // whose subsequent header bits fail the structural NBLKS / FSIZE
    // bounds (or whose declared frame_size_bytes overruns
    // end-of-buffer) is treated as a false-positive sync rather than a
    // hard fault; the iterator yields a ResyncEvent and continues
    // scanning from offset + 1. Useful for walking partially-corrupted
    // streams past malformed-sync patches.
    // -----------------------------------------------------------------

    /// Bit-vector helper used by the synthetic-frame builders below.
    /// Pushes the low `width` bits of `value` MSB-first into `bv`.
    fn push_bits(bv: &mut Vec<bool>, value: u32, width: u32) {
        for i in (0..width).rev() {
            bv.push(((value >> i) & 1) == 1);
        }
    }

    /// Pack an MSB-first bool vector into bytes, panicking if the
    /// length is not a multiple of 8.
    fn bits_to_bytes(bv: &[bool]) -> Vec<u8> {
        assert_eq!(bv.len() % 8, 0, "bit count must be multiple of 8");
        let mut out = vec![0u8; bv.len() / 8];
        for (i, chunk) in bv.chunks(8).enumerate() {
            let mut b: u8 = 0;
            for (k, bit) in chunk.iter().enumerate() {
                if *bit {
                    b |= 1 << (7 - k);
                }
            }
            out[i] = b;
        }
        out
    }

    /// Build a minimum-size (95-byte) termination raw-BE frame whose
    /// SUBFRAMES region is filled with `fill`. Header fields chosen so
    /// the parser accepts the frame.
    fn build_min_frame_be(fill: u8) -> Vec<u8> {
        let mut bv: Vec<bool> = Vec::new();
        push_bits(&mut bv, 0x7FFE_8001, 32);
        push_bits(&mut bv, 0, 1); // ftype = termination
        push_bits(&mut bv, 0, 5); // sample_count_m1
        push_bits(&mut bv, 0, 1); // crc_present
        push_bits(&mut bv, 5, 7); // nblks = 5
        push_bits(&mut bv, 94, 14); // fsize-1 = 94 → frame_size = 95
        push_bits(&mut bv, 0, 6); // amode
        push_bits(&mut bv, 0, 4); // sfreq
        push_bits(&mut bv, 0, 5); // rate
        push_bits(&mut bv, 0, 13); // trailing flags
        push_bits(&mut bv, 0, 16); // post-CRC window
        while !bv.len().is_multiple_of(8) {
            bv.push(false);
        }
        let mut buf = bits_to_bytes(&bv);
        buf.resize(95, fill);
        for byte in buf.iter_mut().skip(13) {
            *byte = fill;
        }
        buf
    }

    /// On a well-formed stream the resync walker is byte-for-byte
    /// equivalent to the fail-fast iterator: every step yields `Ok`
    /// and the frame views match.
    #[test]
    fn resync_walks_clean_stream_identically_to_fail_fast() {
        // Build a 3-frame raw-BE stream by concatenating three
        // build_min_frame_be calls. Each frame is 95 B → 3 × 95 = 285 B.
        let mut stream = Vec::with_capacity(3 * 95);
        for fill in [0x11u8, 0x22, 0x33] {
            stream.extend_from_slice(&build_min_frame_be(fill));
        }
        assert_eq!(stream.len(), 285);

        let strict: Vec<FrameView<'_>> = iter_frames(&stream)
            .collect::<core::result::Result<Vec<_>, _>>()
            .expect("clean stream must walk");
        let resync: Vec<FrameView<'_>> = iter_frames_resync(&stream)
            .collect::<core::result::Result<Vec<_>, ResyncEvent>>()
            .expect("clean stream must walk through resync iter too");
        assert_eq!(strict.len(), 3);
        assert_eq!(resync.len(), 3);
        for (a, b) in strict.iter().zip(resync.iter()) {
            assert_eq!(a.offset, b.offset);
            assert_eq!(a.len, b.len);
            assert_eq!(a.header, b.header);
        }
    }

    /// A coincidental 4-byte raw-BE sync pattern in the SUBFRAMES
    /// region of a preceding frame triggers a false-positive sync at
    /// that offset. The fail-fast iterator can't see it (it advances
    /// by frame_size_bytes), but a stand-alone sync planted at an
    /// arbitrary offset followed by zero bytes will fail
    /// structural-bound checks: the 7 NBLKS bits decode as 0 → after
    /// the +1 increment that is < 5 → BlockCountOutOfRange. The
    /// resync iterator must surface that as a `StructuralBoundFailed`
    /// ResyncEvent and continue, recovering the real frame that
    /// follows.
    #[test]
    fn resync_skips_false_positive_sync_in_garbage_and_recovers_real_frame() {
        // Layout: [garbage with embedded false sync 0..32] +
        // [real frame at offset 32, 95 B].
        let real = build_min_frame_be(0xCC);
        let mut buf = vec![0xAAu8; 32];
        // Plant the canonical raw-BE sync at offset 8, followed by all
        // zeros for the next ~11 bytes — the parser will read
        // NBLKS == 0 → BlockCountOutOfRange.
        buf[8..12].copy_from_slice(&RAW_BE_SYNC);
        for byte in buf.iter_mut().take(32).skip(12) {
            *byte = 0;
        }
        buf.extend_from_slice(&real);
        // Sanity: fail-fast iter_frames terminates with an error at
        // the false sync without recovering the real frame.
        let mut strict = iter_frames(&buf);
        match strict.next() {
            Some(Err(Error::BlockCountOutOfRange { .. })) => {}
            other => panic!("fail-fast must surface BlockCountOutOfRange, got {other:?}"),
        }
        assert!(strict.next().is_none(), "fail-fast iterator terminates");

        // Resync iterator: one Err event for the false sync, then the
        // real frame, then end.
        let mut it = iter_frames_resync(&buf);
        let first = it.next().expect("must yield event");
        match first {
            Err(ResyncEvent {
                offset: 8,
                encoding: SyncWordEncoding::RawBigEndian,
                cause: ResyncCause::StructuralBoundFailed(Error::BlockCountOutOfRange { .. }),
            }) => {}
            other => panic!("expected StructuralBoundFailed at offset 8, got {other:?}"),
        }
        let second = it.next().expect("must yield real frame");
        let view = second.expect("real frame must parse");
        assert_eq!(view.offset, 32);
        assert_eq!(view.len, 95);
        assert_eq!(
            view.header.sync_word_encoding,
            SyncWordEncoding::RawBigEndian
        );
        assert_eq!(view.header.frame_size_bytes, 95);
        assert!(it.next().is_none());
    }

    /// A false-positive sync at an offset whose declared frame size
    /// overruns end-of-buffer must surface as
    /// `FrameLengthOverrunsBuffer`, not terminate the iterator. After
    /// the event the scan resumes past the spurious sync and finds the
    /// next real frame (or ends naturally if none).
    #[test]
    fn resync_overrun_event_lets_iterator_continue() {
        // Build a buffer: real frame at offset 0 (95 B); then plant a
        // raw-BE sync at offset 96 with a header declaring a huge
        // frame_size (overruns end-of-buffer); then the genuine next
        // frame at offset 96 + 5 = 101.
        //
        // To engineer the "header parses OK but length overruns" case
        // we use a real 95-byte termination frame but place it inside
        // a buffer that is exactly 96+95 = 191 B starting at offset 96
        // — but if we plant the well-formed frame at offset 96, the
        // resync iterator would correctly walk it. So instead we plant
        // a HAND-CRAFTED header at offset 96 with FSIZE that exceeds
        // remaining buffer space.

        let real = build_min_frame_be(0x11);
        let mut buf = real.clone(); // frame at offset 0..95.
        // 1 byte of garbage so the false sync is well-separated.
        buf.push(0xAA);
        let false_sync_off = buf.len(); // == 96
        // Craft a header with FSIZE that overruns the remaining buffer.
        let total_after_false_sync = 50; // we'll make remaining = 50.
        // Declared frame_size = 200 > 50.
        let mut bv: Vec<bool> = Vec::new();
        push_bits(&mut bv, 0x7FFE_8001, 32);
        push_bits(&mut bv, 0, 1);
        push_bits(&mut bv, 0, 5);
        push_bits(&mut bv, 0, 1);
        push_bits(&mut bv, 5, 7); // nblks = 5
        push_bits(&mut bv, 199, 14); // fsize-1 = 199 → frame_size = 200
        push_bits(&mut bv, 0, 6);
        push_bits(&mut bv, 0, 4);
        push_bits(&mut bv, 0, 5);
        push_bits(&mut bv, 0, 13);
        push_bits(&mut bv, 0, 16);
        while !bv.len().is_multiple_of(8) {
            bv.push(false);
        }
        let hdr_bytes = bits_to_bytes(&bv);
        buf.extend_from_slice(&hdr_bytes);
        // Pad with garbage until we have `total_after_false_sync`
        // bytes after the false sync (so the header is fully readable
        // — 13 B — but the declared 200-byte frame overruns).
        while buf.len() - false_sync_off < total_after_false_sync {
            buf.push(0xBB);
        }
        assert_eq!(buf.len() - false_sync_off, total_after_false_sync);

        // Walk with the resync iterator.
        let events: Vec<_> = iter_frames_resync(&buf).collect();
        // Expect: Ok(real frame at 0), Err(FrameLengthOverrunsBuffer at
        // 96, declared_len=200), then end (find_next_sync from 97
        // finds nothing — the canonical sync was only planted at 96).
        assert_eq!(events.len(), 2);
        let frame = events[0].as_ref().unwrap();
        assert_eq!(frame.offset, 0);
        assert_eq!(frame.len, 95);

        match events[1] {
            Err(ResyncEvent {
                offset: 96,
                encoding: SyncWordEncoding::RawBigEndian,
                cause: ResyncCause::FrameLengthOverrunsBuffer { declared_len: 200 },
            }) => {}
            ref other => panic!("expected overrun event at 96 with len 200, got {other:?}"),
        }
    }

    /// A 14-bit sync encountered by the resync iterator is reported
    /// via `FourteenBitSyncSkipped` rather than terminating the walk.
    /// The iterator continues past the 14-bit sync so subsequent raw
    /// frames are still recovered.
    #[test]
    fn resync_skips_fourteen_bit_sync_and_keeps_walking() {
        let real = build_min_frame_be(0xDD);
        let mut buf = Vec::new();
        // 14-bit-BE sync at offset 0 (6 bytes, lower-14-bits match
        // 0x1FFF / 0x2800 / 0x07Fx).
        buf.extend_from_slice(&[0x1F, 0xFF, 0xE8, 0x00, 0x07, 0xFA]);
        // 2 bytes of garbage so the canonical raw-BE sync starts on a
        // byte boundary the scanner will reach without colliding.
        buf.extend_from_slice(&[0xAA, 0xAA]);
        let real_off = buf.len();
        buf.extend_from_slice(&real);

        let mut it = iter_frames_resync(&buf);
        match it.next() {
            Some(Err(ResyncEvent {
                offset: 0,
                encoding: SyncWordEncoding::FourteenBitBigEndian,
                cause: ResyncCause::FourteenBitSyncSkipped,
            })) => {}
            other => panic!("expected 14-bit-BE skip at offset 0, got {other:?}"),
        }
        // Next step recovers the real raw-BE frame.
        let view = it.next().expect("must yield frame").expect("must parse");
        assert_eq!(view.offset, real_off);
        assert_eq!(
            view.header.sync_word_encoding,
            SyncWordEncoding::RawBigEndian
        );
        assert!(it.next().is_none());
    }

    /// Cursor progresses correctly across mixed events: a real frame
    /// advances by frame_size_bytes; a ResyncEvent advances by exactly
    /// one byte.
    #[test]
    fn resync_cursor_advances_one_byte_on_event_and_frame_size_on_ok() {
        let real = build_min_frame_be(0x44);
        let mut buf = vec![0xAAu8; 4];
        buf[0..4].copy_from_slice(&RAW_BE_SYNC); // false sync at 0.
        // Pad so the false-sync header reads zeros → BlockCountOutOfRange.
        buf.resize(20, 0);
        // Real frame at offset 20.
        let real_off = buf.len();
        buf.extend_from_slice(&real);

        let mut it = iter_frames_resync(&buf);
        // Step 1: false sync at 0; cursor must advance to 1.
        let _ = it.next();
        assert_eq!(it.cursor(), 1);
        // Step 2: real frame at 20; cursor advances by frame_size_bytes.
        let _ = it.next();
        assert_eq!(it.cursor(), real_off + 95);
        // Step 3: no more syncs.
        assert!(it.next().is_none());
    }

    /// An empty buffer yields nothing.
    #[test]
    fn resync_empty_buffer_yields_none() {
        let buf: [u8; 0] = [];
        assert!(iter_frames_resync(&buf).next().is_none());
    }

    /// A buffer with no sync at all yields nothing.
    #[test]
    fn resync_no_sync_yields_none() {
        let buf = vec![0xAAu8; 64];
        assert!(iter_frames_resync(&buf).next().is_none());
    }

    /// Multiple consecutive false-positive raw-BE sync sequences are
    /// each reported (in order) and the iterator finally terminates
    /// when no more syncs exist.
    #[test]
    fn resync_multiple_consecutive_false_positives_each_reported() {
        let mut buf = vec![0u8; 64];
        // Three false sync sequences at offsets 0, 16, 32 with all
        // zeros following → each fails NBLKS bound.
        for &off in &[0usize, 16, 32] {
            buf[off..off + 4].copy_from_slice(&RAW_BE_SYNC);
        }
        let events: Vec<_> = iter_frames_resync(&buf).collect();
        assert_eq!(events.len(), 3);
        for (i, &expected_off) in [0usize, 16, 32].iter().enumerate() {
            match events[i] {
                Err(ResyncEvent {
                    offset,
                    encoding: SyncWordEncoding::RawBigEndian,
                    cause: ResyncCause::StructuralBoundFailed(Error::BlockCountOutOfRange { .. }),
                }) if offset == expected_off => {}
                ref other => panic!("event {i} mismatch: {other:?}"),
            }
        }
    }

    /// A genuine truncated tail surfaces as a single
    /// `FrameLengthOverrunsBuffer` event with the declared length;
    /// the iterator then ends because no subsequent sync exists in
    /// the truncated buffer.
    #[test]
    fn resync_truncated_tail_surfaces_overrun_event_then_ends() {
        // Build a valid-header buffer whose declared frame_size_bytes
        // is 95 but the buffer holds only 50 bytes total.
        let real = build_min_frame_be(0xEE);
        let truncated = &real[..50];
        let events: Vec<_> = iter_frames_resync(truncated).collect();
        assert_eq!(events.len(), 1);
        match events[0] {
            Err(ResyncEvent {
                offset: 0,
                encoding: SyncWordEncoding::RawBigEndian,
                cause: ResyncCause::FrameLengthOverrunsBuffer { declared_len: 95 },
            }) => {}
            ref other => panic!("expected overrun event, got {other:?}"),
        }
    }

    /// A header that's truncated mid-bits (sync present, body too
    /// short to read all 104 header bits) yields `HeaderEof`. The
    /// iterator then ends because no subsequent sync follows.
    #[test]
    fn resync_truncated_header_surfaces_header_eof() {
        // Sync + 4 bytes (so the parser sees 8 bytes total — far less
        // than the 13-byte minimum header window).
        let buf = [0x7F, 0xFE, 0x80, 0x01, 0x00, 0x00, 0x00, 0x00];
        let events: Vec<_> = iter_frames_resync(&buf).collect();
        assert_eq!(events.len(), 1);
        match events[0] {
            Err(ResyncEvent {
                offset: 0,
                encoding: SyncWordEncoding::RawBigEndian,
                cause: ResyncCause::HeaderEof,
            }) => {}
            ref other => panic!("expected HeaderEof, got {other:?}"),
        }
    }

    /// The convenience constructor `iter_frames_resync` is equivalent
    /// to `FrameIteratorResync::new`.
    #[test]
    fn iter_frames_resync_matches_struct_new() {
        let real = build_min_frame_be(0x55);
        let a: Vec<_> = iter_frames_resync(&real).collect();
        let b: Vec<_> = FrameIteratorResync::new(&real).collect();
        assert_eq!(a.len(), b.len());
        for (x, y) in a.iter().zip(b.iter()) {
            // FrameView lifetimes are anonymous; compare via fields.
            match (x, y) {
                (Ok(fx), Ok(fy)) => {
                    assert_eq!(fx.offset, fy.offset);
                    assert_eq!(fx.len, fy.len);
                    assert_eq!(fx.header, fy.header);
                }
                (Err(ex), Err(ey)) => assert_eq!(ex, ey),
                _ => panic!("variants differ"),
            }
        }
    }

    // ----------------------------------------------------------------
    // Round 165 — find_next_sync / find_all_syncs first-byte gate.
    //
    // The round-165 optimisation gates the multi-byte detect_sync()
    // check behind a one-byte filter
    // (`is_sync_first_byte_candidate`): only bytes 0x7F / 0xFE / 0x1F
    // / 0xFF can start one of the four documented sync sequences.
    // The tests below verify (a) the filter accepts exactly those
    // four bytes, (b) the optimised scan agrees with a brute-force
    // pre-round-165-style reference on every input, including
    // pathological payloads packed with first-byte-candidate bytes
    // whose multi-byte continuation is non-sync, and (c) the
    // optimisation does not change the documented walk order or
    // returned offsets.
    // ----------------------------------------------------------------

    /// Brute-force, pre-round-165-style reference scanner. Walks
    /// every position 1-by-1 and calls `detect_sync` without a
    /// first-byte gate. Used as the equivalence oracle for the
    /// optimised `find_next_sync`.
    fn reference_find_next_sync(bytes: &[u8], start: usize) -> Option<SyncMatch> {
        use crate::header::detect_sync;
        if start >= bytes.len() {
            return None;
        }
        let mut i = start;
        while i + 4 <= bytes.len() {
            if let Ok(enc) = detect_sync(&bytes[i..]) {
                return Some(SyncMatch {
                    offset: i,
                    encoding: enc,
                });
            }
            i += 1;
        }
        None
    }

    fn reference_find_all_syncs(bytes: &[u8]) -> Vec<SyncMatch> {
        let mut out = Vec::new();
        let mut cursor = 0usize;
        while let Some(m) = reference_find_next_sync(bytes, cursor) {
            out.push(m);
            cursor = m.offset + 1;
        }
        out
    }

    /// The filter accepts exactly the four first bytes of the four
    /// documented sync sequences. Every other byte (252 of 256)
    /// must short-circuit.
    #[test]
    fn first_byte_candidate_accepts_exactly_four_bytes() {
        let mut accepted: Vec<u8> = Vec::new();
        for b in 0u16..=255 {
            if is_sync_first_byte_candidate(b as u8) {
                accepted.push(b as u8);
            }
        }
        accepted.sort();
        assert_eq!(accepted, vec![0x1F, 0x7F, 0xFE, 0xFF]);
    }

    /// First-byte gate must accept the actual first byte of every
    /// documented sync prefix. (Belt-and-braces; the previous test
    /// asserts the same thing inversely, but spelling it out per
    /// encoding makes regressions easier to read.)
    #[test]
    fn first_byte_candidate_accepts_documented_sync_prefixes() {
        assert!(is_sync_first_byte_candidate(0x7F)); // raw BE
        assert!(is_sync_first_byte_candidate(0xFE)); // raw LE
        assert!(is_sync_first_byte_candidate(0x1F)); // 14-bit BE
        assert!(is_sync_first_byte_candidate(0xFF)); // 14-bit LE
        // Spot-check a few adjacent bytes that look similar but are
        // explicitly NOT sync prefixes.
        assert!(!is_sync_first_byte_candidate(0x7E));
        assert!(!is_sync_first_byte_candidate(0xFD));
        assert!(!is_sync_first_byte_candidate(0x80));
        assert!(!is_sync_first_byte_candidate(0x00));
    }

    /// Optimised `find_next_sync` agrees with the pre-round-165
    /// reference on every position of a buffer densely packed with
    /// first-byte candidates whose multi-byte continuation is
    /// deliberately non-sync. The first-byte gate must NOT smuggle
    /// in false-positive matches: positions where bytes[i] is one
    /// of {0x7F, 0xFE, 0x1F, 0xFF} but the following 3-5 bytes do
    /// not match the full sync sequence must still return `None`
    /// (or the next genuine sync further along).
    #[test]
    fn find_next_sync_matches_pre_optimization_reference_on_candidate_dense_payload() {
        // Build a 256 B buffer where every fourth byte is a sync
        // first-byte candidate but the continuation never matches.
        let mut buf = vec![0u8; 256];
        let candidates = [0x7Fu8, 0xFE, 0x1F, 0xFF];
        for (i, b) in buf.iter_mut().enumerate() {
            if i % 4 == 0 {
                *b = candidates[(i / 4) % 4];
            } else {
                // Bytes 1..4 deliberately != real continuation bytes.
                *b = 0x55;
            }
        }
        // Reference and optimised must both return None.
        assert_eq!(find_next_sync(&buf, 0), None);
        assert_eq!(reference_find_next_sync(&buf, 0), None);

        // Now embed a real raw-BE sync at offset 100 and confirm
        // both implementations find it at the same offset with the
        // same encoding tag.
        buf[100..104].copy_from_slice(&RAW_BE_SYNC);
        let opt = find_next_sync(&buf, 0).unwrap();
        let r = reference_find_next_sync(&buf, 0).unwrap();
        assert_eq!(opt, r);
        assert_eq!(opt.offset, 100);
        assert_eq!(opt.encoding, SyncWordEncoding::RawBigEndian);
    }

    /// Cross-check the optimised `find_next_sync` against the
    /// reference on a deterministic pseudo-random buffer. The LCG
    /// seed is fixed so the test is reproducible.
    #[test]
    fn find_next_sync_matches_reference_on_pseudo_random_buffer() {
        // 4 KB linear-congruential pseudo-random payload (Knuth's
        // MMIX-friendly multiplier).
        let mut buf = vec![0u8; 4096];
        let mut state: u64 = 0xDEAD_BEEF_CAFE_BABE;
        for b in buf.iter_mut() {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            *b = (state >> 32) as u8;
        }
        // Sweep every possible start offset and verify per-call agreement.
        for start in 0..buf.len() {
            assert_eq!(
                find_next_sync(&buf, start),
                reference_find_next_sync(&buf, start),
                "find_next_sync diverged at start={start}"
            );
        }
    }

    /// Cross-check `find_all_syncs` against the brute-force
    /// reference on the same pseudo-random buffer plus several
    /// embedded real syncs. Both must agree on the full list of
    /// (offset, encoding) pairs.
    #[test]
    fn find_all_syncs_matches_reference_on_random_buffer_with_embedded_syncs() {
        let mut buf = vec![0u8; 4096];
        let mut state: u64 = 0x0123_4567_89AB_CDEF;
        for b in buf.iter_mut() {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            *b = (state >> 32) as u8;
        }
        // Embed real syncs at known positions across all four
        // encodings.
        buf[100..104].copy_from_slice(&RAW_BE_SYNC);
        buf[500..504].copy_from_slice(&RAW_LE_SYNC);
        // 14-bit BE prefix per wiki: `1F FF E8 00 07 F?`.
        buf[1000..1006].copy_from_slice(&[0x1F, 0xFF, 0xE8, 0x00, 0x07, 0xF0]);
        // 14-bit LE prefix per wiki: `FF 1F 00 E8 F? 07`.
        buf[2000..2006].copy_from_slice(&[0xFF, 0x1F, 0x00, 0xE8, 0xF0, 0x07]);

        let opt = find_all_syncs(&buf);
        let r = reference_find_all_syncs(&buf);
        assert_eq!(opt, r, "find_all_syncs diverged from reference");
        // Sanity: the four embedded syncs are recovered with the
        // right encodings.
        let pairs: Vec<(usize, SyncWordEncoding)> =
            opt.iter().map(|m| (m.offset, m.encoding)).collect();
        assert!(pairs.contains(&(100, SyncWordEncoding::RawBigEndian)));
        assert!(pairs.contains(&(500, SyncWordEncoding::RawLittleEndian)));
        assert!(pairs.contains(&(1000, SyncWordEncoding::FourteenBitBigEndian)));
        assert!(pairs.contains(&(2000, SyncWordEncoding::FourteenBitLittleEndian)));
    }

    /// First-byte gate must not change the answer when the input is
    /// densely packed with `0xFF` bytes (a common payload pattern in
    /// silent-encoded audio): every position has a first-byte
    /// candidate but very few have a valid sync continuation.
    #[test]
    fn find_next_sync_handles_all_ones_payload_with_one_embedded_sync() {
        let mut buf = vec![0xFFu8; 256];
        // Embed a real raw-LE sync at offset 50.
        buf[50..54].copy_from_slice(&RAW_LE_SYNC);
        let opt = find_next_sync(&buf, 0).unwrap();
        let r = reference_find_next_sync(&buf, 0).unwrap();
        assert_eq!(opt, r);
        assert_eq!(opt.offset, 50);
        assert_eq!(opt.encoding, SyncWordEncoding::RawLittleEndian);
    }

    /// All-zero payload (no first-byte candidates anywhere) returns
    /// `None` from both implementations after a single full pass.
    /// Confirms the gate's early-exit path doesn't infinite-loop or
    /// skip end-of-buffer bookkeeping.
    #[test]
    fn find_next_sync_handles_all_zero_payload() {
        let buf = vec![0u8; 4096];
        assert_eq!(find_next_sync(&buf, 0), None);
        assert_eq!(reference_find_next_sync(&buf, 0), None);
    }

    /// Sweep `start` across every offset of a moderately-sized
    /// buffer containing two real syncs. The optimised and
    /// reference scanners must agree on the result for every start.
    #[test]
    fn find_next_sync_start_sweep_matches_reference_with_two_real_syncs() {
        let mut buf = vec![0xAAu8; 200];
        buf[20..24].copy_from_slice(&RAW_BE_SYNC);
        buf[100..104].copy_from_slice(&RAW_LE_SYNC);
        for start in 0..buf.len() {
            assert_eq!(
                find_next_sync(&buf, start),
                reference_find_next_sync(&buf, start),
                "divergence at start={start}"
            );
        }
    }

    // ---------------------------------------------------------------
    // Round 179 — SyncWordEncoding::sync_byte_length /
    //             SyncMatch::sync_byte_length / sync_byte_range /
    //             SyncIterator + iter_syncs
    // ---------------------------------------------------------------

    /// Wiki sync table directly enumerates the four sync sequences.
    /// Length 4 for the two raw encodings (`7F FE 80 01` /
    /// `FE 7F 01 80`); length 6 for the two 14-bit-packed encodings
    /// (`1F FF E8 00 07 Fx` / `FF 1F 00 E8 Fx 07`).
    #[test]
    fn sync_word_encoding_byte_length_matches_wiki_sync_table() {
        assert_eq!(SyncWordEncoding::RawBigEndian.sync_byte_length(), 4);
        assert_eq!(SyncWordEncoding::RawLittleEndian.sync_byte_length(), 4);
        assert_eq!(SyncWordEncoding::FourteenBitBigEndian.sync_byte_length(), 6);
        assert_eq!(
            SyncWordEncoding::FourteenBitLittleEndian.sync_byte_length(),
            6
        );
    }

    /// `is_raw_16bit` accepts exactly the two raw encodings;
    /// `is_14bit_packed` accepts exactly the other two. The two
    /// predicates are mutually exclusive and jointly exhaustive over
    /// the documented sync encodings.
    #[test]
    fn sync_word_encoding_raw_vs_packed_predicates_partition_the_enum() {
        for enc in [
            SyncWordEncoding::RawBigEndian,
            SyncWordEncoding::RawLittleEndian,
            SyncWordEncoding::FourteenBitBigEndian,
            SyncWordEncoding::FourteenBitLittleEndian,
        ] {
            assert_ne!(
                enc.is_raw_16bit(),
                enc.is_14bit_packed(),
                "exactly one predicate must hold for {enc:?}"
            );
        }
        assert!(SyncWordEncoding::RawBigEndian.is_raw_16bit());
        assert!(SyncWordEncoding::RawLittleEndian.is_raw_16bit());
        assert!(!SyncWordEncoding::FourteenBitBigEndian.is_raw_16bit());
        assert!(!SyncWordEncoding::FourteenBitLittleEndian.is_raw_16bit());
        assert!(SyncWordEncoding::FourteenBitBigEndian.is_14bit_packed());
        assert!(SyncWordEncoding::FourteenBitLittleEndian.is_14bit_packed());
    }

    /// `SyncMatch::sync_byte_length` delegates to the encoding; the
    /// resulting half-open range carries the expected number of
    /// bytes for each of the four documented encodings.
    #[test]
    fn sync_match_sync_byte_range_carries_wiki_documented_byte_count() {
        let cases = [
            (SyncWordEncoding::RawBigEndian, 4usize),
            (SyncWordEncoding::RawLittleEndian, 4),
            (SyncWordEncoding::FourteenBitBigEndian, 6),
            (SyncWordEncoding::FourteenBitLittleEndian, 6),
        ];
        for (enc, expected_len) in cases {
            let m = SyncMatch {
                offset: 17,
                encoding: enc,
            };
            assert_eq!(m.sync_byte_length(), expected_len);
            let r = m.sync_byte_range();
            assert_eq!(r.start, 17);
            assert_eq!(r.end, 17 + expected_len);
        }
    }

    /// `sync_byte_range` lets the caller slice the matched bytes
    /// straight out of the input. For a raw-BE sync at offset 0
    /// that slice is `7F FE 80 01`.
    #[test]
    fn sync_match_sync_byte_range_slices_raw_be_sync_bytes() {
        let mut buf = vec![0u8; 16];
        buf[..4].copy_from_slice(&RAW_BE_SYNC);
        let m = find_next_sync(&buf, 0).unwrap();
        assert_eq!(&buf[m.sync_byte_range()], &RAW_BE_SYNC);
    }

    /// `sync_byte_range` for a 14-bit-BE sync at offset 5 reproduces
    /// the wiki's `1F FF E8 00 07 F0` prefix.
    #[test]
    fn sync_match_sync_byte_range_slices_14bit_be_sync_bytes() {
        let mut buf = vec![0u8; 32];
        buf[5..11].copy_from_slice(&[0x1F, 0xFF, 0xE8, 0x00, 0x07, 0xF0]);
        let m = find_next_sync(&buf, 0).unwrap();
        assert_eq!(m.encoding, SyncWordEncoding::FourteenBitBigEndian);
        assert_eq!(m.sync_byte_range(), 5..11);
        assert_eq!(
            &buf[m.sync_byte_range()],
            &[0x1F, 0xFF, 0xE8, 0x00, 0x07, 0xF0]
        );
    }

    /// `iter_syncs` and `find_all_syncs` must agree element-by-element
    /// on a buffer that contains all four documented sync encodings.
    /// This is the streaming/bulk equivalence contract — collect()
    /// of the iterator equals the vector returned by the bulk helper.
    #[test]
    fn iter_syncs_collects_to_same_vec_as_find_all_syncs_on_mixed_encoding_buffer() {
        let mut buf = vec![0u8; 64];
        buf[2..6].copy_from_slice(&RAW_BE_SYNC);
        buf[12..16].copy_from_slice(&RAW_LE_SYNC);
        buf[24..30].copy_from_slice(&[0x1F, 0xFF, 0xE8, 0x00, 0x07, 0xF5]);
        buf[40..46].copy_from_slice(&[0xFF, 0x1F, 0x00, 0xE8, 0xF7, 0x07]);
        let bulk = find_all_syncs(&buf);
        let streamed: Vec<SyncMatch> = iter_syncs(&buf).collect();
        assert_eq!(bulk, streamed);
        assert_eq!(streamed.len(), 4);
        assert_eq!(streamed[0].encoding, SyncWordEncoding::RawBigEndian);
        assert_eq!(streamed[1].encoding, SyncWordEncoding::RawLittleEndian);
        assert_eq!(streamed[2].encoding, SyncWordEncoding::FourteenBitBigEndian);
        assert_eq!(
            streamed[3].encoding,
            SyncWordEncoding::FourteenBitLittleEndian
        );
    }

    /// Streaming + bulk agree on an empty-result buffer (no syncs).
    /// The iterator yields `None` on the first `next()` call.
    #[test]
    fn iter_syncs_returns_none_on_buffer_without_any_syncs() {
        let buf = vec![0xAAu8; 256];
        let mut it = iter_syncs(&buf);
        assert!(it.next().is_none());
        assert_eq!(find_all_syncs(&buf), Vec::<SyncMatch>::new());
    }

    /// `take(N)` correctly limits the iterator without forcing a
    /// full scan — the next call after the take window stops yielding
    /// even if more syncs exist downstream.
    #[test]
    fn iter_syncs_take_window_stops_after_n_matches() {
        let mut buf = vec![0u8; 256];
        // Plant five raw-BE syncs at 10, 30, 50, 70, 90.
        for off in [10usize, 30, 50, 70, 90] {
            buf[off..off + 4].copy_from_slice(&RAW_BE_SYNC);
        }
        let first_three: Vec<SyncMatch> = iter_syncs(&buf).take(3).collect();
        assert_eq!(first_three.len(), 3);
        assert_eq!(first_three[0].offset, 10);
        assert_eq!(first_three[1].offset, 30);
        assert_eq!(first_three[2].offset, 50);
    }

    /// `filter` combinator: select only the raw-16-bit syncs from a
    /// mixed-encoding buffer using `SyncWordEncoding::is_raw_16bit`.
    #[test]
    fn iter_syncs_filter_by_is_raw_16bit_excludes_14bit_matches() {
        let mut buf = vec![0u8; 64];
        buf[0..4].copy_from_slice(&RAW_BE_SYNC);
        buf[10..16].copy_from_slice(&[0x1F, 0xFF, 0xE8, 0x00, 0x07, 0xF1]);
        buf[20..24].copy_from_slice(&RAW_LE_SYNC);
        buf[30..36].copy_from_slice(&[0xFF, 0x1F, 0x00, 0xE8, 0xF2, 0x07]);
        let raws: Vec<SyncMatch> = iter_syncs(&buf)
            .filter(|m| m.encoding.is_raw_16bit())
            .collect();
        assert_eq!(raws.len(), 2);
        assert!(raws.iter().all(|m| m.encoding.is_raw_16bit()));
        assert_eq!(raws[0].offset, 0);
        assert_eq!(raws[1].offset, 20);
    }

    /// `SyncIterator::cursor()` exposes the scan position. After
    /// yielding the only match in the buffer, the cursor advances to
    /// `offset + 1`; after the iterator is exhausted, it sits at the
    /// position `find_next_sync` gave up at.
    #[test]
    fn sync_iterator_cursor_reflects_scan_position() {
        let mut buf = vec![0u8; 32];
        buf[10..14].copy_from_slice(&RAW_BE_SYNC);
        let mut it = iter_syncs(&buf);
        assert_eq!(it.cursor(), 0);
        let m = it.next().unwrap();
        assert_eq!(m.offset, 10);
        // After yielding the match at offset 10, the cursor advanced
        // by one so the next scan starts at offset 11 (the
        // non-overlapping resume position documented for
        // find_all_syncs).
        assert_eq!(it.cursor(), 11);
        assert!(it.next().is_none());
    }

    /// `iter_syncs` agrees with the reference `find_all_syncs` on a
    /// 4 KB pseudo-random buffer with four embedded real syncs (one
    /// of each encoding). Equivalence is checked element-by-element
    /// so the streaming iterator inherits the bulk helper's
    /// reference-validation coverage.
    #[test]
    fn iter_syncs_matches_reference_on_pseudo_random_buffer_with_embedded_syncs() {
        let mut buf = vec![0u8; 4096];
        let mut state: u64 = 0x4242_4242_4242_4242;
        for b in buf.iter_mut() {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            *b = (state >> 32) as u8;
        }
        buf[200..204].copy_from_slice(&RAW_BE_SYNC);
        buf[800..804].copy_from_slice(&RAW_LE_SYNC);
        buf[1500..1506].copy_from_slice(&[0x1F, 0xFF, 0xE8, 0x00, 0x07, 0xF2]);
        buf[3000..3006].copy_from_slice(&[0xFF, 0x1F, 0x00, 0xE8, 0xF4, 0x07]);
        let streamed: Vec<SyncMatch> = iter_syncs(&buf).collect();
        let reference = reference_find_all_syncs(&buf);
        assert_eq!(streamed, reference);
    }

    // ---------------------------------------------------------------
    // Round 192 — iter_frames_14bit (14-bit container-stream walker)
    //
    // Each test builds a raw-BE frame (95 bytes, the minimum FSIZE+1
    // the spec allows), packs it through `pack_16bit_to_14bit` into a
    // 14-bit-packed container buffer, and exercises the iterator
    // against the resulting bytes. The container-byte advance is
    // verified to equal `frame_size_container_bytes(encoding)`
    // (= 110 bytes for FSIZE+1 = 95: ceil(95 * 8 / 14) * 2 = 110).
    // ---------------------------------------------------------------

    use crate::{FourteenBitByteOrder, pack_16bit_to_14bit};

    /// Build a 95-byte raw-BE single-frame buffer: FTYPE=0
    /// (termination), SHORT=0, CRC_PRESENT=0, NBLKS=5, FSIZE-1=94 (=>
    /// frame_size_bytes = 95), all other fields zero. Mirrors the
    /// hand-built buffers used by the round-138 payload tests above.
    fn build_minimum_raw_be_frame() -> Vec<u8> {
        fn push(bv: &mut Vec<bool>, value: u32, width: u32) {
            for i in (0..width).rev() {
                bv.push(((value >> i) & 1) == 1);
            }
        }
        let mut bv: Vec<bool> = Vec::new();
        push(&mut bv, 0x7FFE_8001, 32);
        push(&mut bv, 0, 1); // ftype = termination
        push(&mut bv, 0, 5);
        push(&mut bv, 0, 1); // crc_present
        push(&mut bv, 5, 7); // nblks
        push(&mut bv, 94, 14); // fsize-1 = 94 -> frame_size = 95
        push(&mut bv, 0, 6); // amode
        push(&mut bv, 0, 4); // sfreq
        push(&mut bv, 0, 5); // rate
        push(&mut bv, 0, 13);
        push(&mut bv, 0, 16); // post-CRC window
        while !bv.len().is_multiple_of(8) {
            bv.push(false);
        }
        let mut buf = vec![0u8; 95];
        for (i, chunk) in bv.chunks(8).enumerate() {
            let mut b: u8 = 0;
            for (k, bit) in chunk.iter().enumerate() {
                if *bit {
                    b |= 1 << (7 - k);
                }
            }
            buf[i] = b;
        }
        // Fill SUBFRAMES region with a distinctive pattern so the
        // container-domain frame slice can be cross-checked through
        // a round-trip unpack later.
        for byte in buf.iter_mut().skip(13) {
            *byte = 0xC1;
        }
        buf
    }

    /// `iter_frames_14bit` parses a single 14-bit-BE frame and
    /// reports the round-189 container-byte advance.
    #[test]
    fn iter_frames_14bit_walks_single_be_frame() {
        let raw = build_minimum_raw_be_frame();
        let (packed, _bits) = pack_16bit_to_14bit(&raw, FourteenBitByteOrder::BigEndian);
        // 95 logical bytes => ceil(95 * 8 / 14) = 55 container words
        // => 110 container bytes.
        assert_eq!(packed.len(), 110);
        let mut it = iter_frames_14bit(&packed);
        let view = it.next().expect("frame must yield").expect("must parse");
        assert_eq!(view.offset, 0);
        assert_eq!(view.len, 110);
        assert_eq!(view.data.len(), 110);
        assert_eq!(
            view.header.sync_word_encoding,
            SyncWordEncoding::FourteenBitBigEndian
        );
        assert_eq!(view.header.frame_size_bytes, 95);
        assert_eq!(view.header.blocks_per_frame, 5);
        assert_eq!(view.header.frame_type, FrameType::Termination);
        // No further frames.
        assert!(it.next().is_none());
    }

    /// Same single-frame round-trip via the LE container order.
    #[test]
    fn iter_frames_14bit_walks_single_le_frame() {
        let raw = build_minimum_raw_be_frame();
        let (packed, _bits) = pack_16bit_to_14bit(&raw, FourteenBitByteOrder::LittleEndian);
        assert_eq!(packed.len(), 110);
        let view = iter_frames_14bit(&packed)
            .next()
            .expect("frame must yield")
            .expect("must parse");
        assert_eq!(view.offset, 0);
        assert_eq!(view.len, 110);
        assert_eq!(
            view.header.sync_word_encoding,
            SyncWordEncoding::FourteenBitLittleEndian
        );
        assert_eq!(view.header.frame_size_bytes, 95);
    }

    /// Two back-to-back 14-bit-BE frames: the iterator must advance
    /// by exactly `frame_size_container_bytes(enc) = 110` between
    /// frames and yield both.
    #[test]
    fn iter_frames_14bit_walks_two_back_to_back_be_frames() {
        let raw = build_minimum_raw_be_frame();
        let (packed_one, _) = pack_16bit_to_14bit(&raw, FourteenBitByteOrder::BigEndian);
        // Concatenate: two back-to-back container-packed frames.
        let mut stream = Vec::new();
        stream.extend_from_slice(&packed_one);
        stream.extend_from_slice(&packed_one);
        assert_eq!(stream.len(), 220);

        let frames: Vec<_> = iter_frames_14bit(&stream)
            .collect::<Result<Vec<_>>>()
            .expect("both frames must parse");
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].offset, 0);
        assert_eq!(frames[0].len, 110);
        assert_eq!(frames[1].offset, 110);
        assert_eq!(frames[1].len, 110);
        // Same encoding through both steps.
        assert_eq!(
            frames[1].header.sync_word_encoding,
            SyncWordEncoding::FourteenBitBigEndian
        );
    }

    /// Leading garbage before the first 14-bit sync must resync
    /// rather than terminate, matching `iter_frames`'s contract for
    /// raw streams.
    #[test]
    fn iter_frames_14bit_handles_leading_garbage_before_first_sync() {
        let raw = build_minimum_raw_be_frame();
        let (packed, _) = pack_16bit_to_14bit(&raw, FourteenBitByteOrder::BigEndian);
        let mut buf: Vec<u8> = vec![0xAA; 17];
        buf.extend_from_slice(&packed);
        let view = iter_frames_14bit(&buf)
            .next()
            .expect("first frame must yield")
            .expect("must parse after resync");
        assert_eq!(view.offset, 17);
        assert_eq!(view.len, 110);
    }

    /// A raw 16-bit sync at the cursor is out-of-domain for this
    /// iterator: it yields `Error::UnsupportedRaw16Bit` and
    /// terminates, mirroring the round-6 `iter_frames` behaviour on
    /// 14-bit syncs in the other direction.
    #[test]
    fn iter_frames_14bit_rejects_raw_16bit_sync() {
        let mut buf = vec![0u8; 32];
        buf[0..4].copy_from_slice(&RAW_BE_SYNC);
        let mut it = iter_frames_14bit(&buf);
        match it.next() {
            Some(Err(Error::UnsupportedRaw16Bit)) => {}
            other => panic!("expected UnsupportedRaw16Bit, got {other:?}"),
        }
        assert!(it.next().is_none(), "iterator terminates after rejection");
    }

    /// An empty buffer yields no frames.
    #[test]
    fn iter_frames_14bit_empty_buffer_yields_nothing() {
        let buf: [u8; 0] = [];
        let mut it = iter_frames_14bit(&buf);
        assert!(it.next().is_none());
    }

    /// A buffer that contains no sync at all yields no frames.
    #[test]
    fn iter_frames_14bit_no_sync_yields_nothing() {
        let buf = vec![0xAAu8; 256];
        let mut it = iter_frames_14bit(&buf);
        assert!(it.next().is_none());
    }

    /// A truncated 14-bit-BE frame where the declared container span
    /// runs past end-of-buffer must report `Error::UnexpectedEof` on
    /// the truncation, just like `iter_frames` for raw-16-bit.
    #[test]
    fn iter_frames_14bit_truncated_tail_reports_eof() {
        let raw = build_minimum_raw_be_frame();
        let (packed, _) = pack_16bit_to_14bit(&raw, FourteenBitByteOrder::BigEndian);
        // Truncate to 100 container bytes (< the 110 the header
        // declares for FSIZE+1 = 95).
        let truncated = &packed[..100];
        let mut it = iter_frames_14bit(truncated);
        match it.next() {
            Some(Err(Error::UnexpectedEof)) => {}
            other => panic!("expected UnexpectedEof on truncation, got {other:?}"),
        }
    }

    /// Cross-check: feeding the iterator's `data` slice (the
    /// container-byte window) back into [`parse_frame_header_14bit`]
    /// recovers the same header. This proves the iterator's window
    /// is correctly sized for the parser's input contract.
    #[test]
    fn iter_frames_14bit_data_slice_round_trips_through_parser() {
        let raw = build_minimum_raw_be_frame();
        let (packed, _) = pack_16bit_to_14bit(&raw, FourteenBitByteOrder::BigEndian);
        let view = iter_frames_14bit(&packed).next().unwrap().unwrap();
        let reparsed = parse_frame_header_14bit(view.data).unwrap();
        assert_eq!(reparsed, view.header);
    }

    /// `FrameIterator14::cursor()` advances by exactly
    /// `frame_size_container_bytes` after a successful step (BE
    /// container).
    #[test]
    fn iter_frames_14bit_cursor_advances_by_container_byte_count() {
        let raw = build_minimum_raw_be_frame();
        let (packed, _) = pack_16bit_to_14bit(&raw, FourteenBitByteOrder::BigEndian);
        let mut stream = Vec::new();
        stream.extend_from_slice(&packed);
        stream.extend_from_slice(&packed);

        let mut it = iter_frames_14bit(&stream);
        assert_eq!(it.cursor(), 0);
        it.next().unwrap().unwrap();
        assert_eq!(it.cursor(), 110);
        it.next().unwrap().unwrap();
        assert_eq!(it.cursor(), 220);
        assert!(it.next().is_none());
    }
}
