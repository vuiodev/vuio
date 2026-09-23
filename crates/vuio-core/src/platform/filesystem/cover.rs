//! Cover-only readers. Never hand a whole container to a metadata parser whose
//! allocation limits are advisory. Lengths are checked before allocating, audio
//! is skipped by seeking, and the total metadata read has a fixed budget.

use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;

type Cover = (String, Vec<u8>);
const TAG_OVERHEAD: usize = 64 * 1024;

pub(crate) fn extract_embedded_cover(path: &Path, max_bytes: usize) -> Option<Cover> {
    let file = std::fs::File::open(path).ok()?;
    if !file.metadata().ok()?.is_file() {
        return None;
    }
    CoverReader::new(std::io::BufReader::new(file), max_bytes)
        .ok()?
        .cover()
        .ok()
        .flatten()
}

struct CoverReader<R> {
    input: R,
    end: u64,
    remaining: usize,
    max_bytes: usize,
    max_tag: usize,
}

fn invalid() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "invalid or oversized cover metadata",
    )
}

impl<R: Read + Seek> CoverReader<R> {
    fn new(mut input: R, max_bytes: usize) -> io::Result<Self> {
        let end = input.seek(SeekFrom::End(0))?;
        input.seek(SeekFrom::Start(0))?;
        // Vorbis pictures are base64 encoded. The allowance is for encoding and
        // tag headers, not an exemption from the decoded image's size limit.
        let max_tag = max_bytes
            .checked_add(max_bytes / 3)
            .and_then(|size| size.checked_add(TAG_OVERHEAD))
            .ok_or_else(invalid)?;
        let remaining = max_tag.checked_mul(2).ok_or_else(invalid)?;
        Ok(Self {
            input,
            end,
            remaining,
            max_bytes,
            max_tag,
        })
    }

    fn read(&mut self, bytes: &mut [u8]) -> io::Result<()> {
        self.remaining = self
            .remaining
            .checked_sub(bytes.len())
            .ok_or_else(invalid)?;
        self.input.read_exact(bytes)
    }

    fn header<const N: usize>(&mut self) -> io::Result<[u8; N]> {
        let mut bytes = [0; N];
        self.read(&mut bytes)?;
        Ok(bytes)
    }

    fn seek(&mut self, position: u64) -> io::Result<()> {
        if position > self.end {
            return Err(invalid());
        }
        self.input.seek(SeekFrom::Start(position))?;
        Ok(())
    }

    fn block(&mut self, len: u64) -> io::Result<Vec<u8>> {
        let len = usize::try_from(len).map_err(|_| invalid())?;
        if len > self.max_tag
            || len > self.remaining
            || len as u64 > self.end.saturating_sub(self.input.stream_position()?)
        {
            return Err(invalid());
        }
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(len).map_err(|_| invalid())?;
        bytes.resize(len, 0);
        self.read(&mut bytes)?;
        Ok(bytes)
    }

    fn cover(&mut self) -> io::Result<Option<Cover>> {
        let header = self.header::<12>()?;
        self.seek(0)?;
        let cover = if &header[..3] == b"ID3" {
            self.leading_id3()?
        } else if &header[..4] == b"fLaC" {
            self.flac(0)?
        } else if (&header[..4] == b"FORM" && matches!(&header[8..], b"AIFF" | b"AIFC"))
            || (&header[..4] == b"RIFF" && &header[8..] == b"WAVE")
        {
            self.iff()?
        } else if &header[..4] == b"OggS" {
            self.ogg()?
        } else if matches!(&header[4..8], b"ftyp" | b"moov" | b"mdat" | b"free") {
            self.mp4(self.end, 0, false)?
        } else if header[..4] == [0x1a, 0x45, 0xdf, 0xa3] {
            self.ebml(self.end, 0)?
        } else {
            None
        };
        if cover.is_some() {
            return Ok(cover);
        }
        self.ape()
    }

    fn leading_id3(&mut self) -> io::Result<Option<Cover>> {
        let mut cover = None;
        loop {
            cover = self.id3(self.end)?.or(cover);
            let position = self.input.stream_position()?;
            if self.end - position < 4 {
                return Ok(cover);
            }
            let next = self.header::<4>()?;
            self.seek(position)?;
            if &next[..3] == b"ID3" {
                continue;
            }
            // Some FLAC files also carry a leading ID3 tag. The native picture
            // (or a later ID3 revision) takes precedence, as with the tag probe.
            if &next == b"fLaC" {
                cover = self.flac(position)?.or(cover);
            }
            return Ok(cover);
        }
    }

    fn id3(&mut self, container_end: u64) -> io::Result<Option<Cover>> {
        let header = self.header::<10>()?;
        if &header[..3] != b"ID3" || !(2..=4).contains(&header[3]) {
            return Ok(None);
        }
        let len = synchsafe(&header[6..]).ok_or_else(invalid)?;
        let tag_end = self
            .input
            .stream_position()?
            .checked_add(len as u64)
            .and_then(|end| {
                end.checked_add(if header[3] == 4 && header[5] & 0x10 != 0 {
                    10
                } else {
                    0
                })
            })
            .ok_or_else(invalid)?;
        if tag_end > container_end {
            return Err(invalid());
        }
        let mut tag = self.block(len as u64)?;
        self.seek(tag_end)?; // v2.4 may carry a ten-byte footer
                             // v2.2's compression and compressed/encrypted frames are deliberately
                             // unsupported: expanding them would defeat the metadata budget.
        if header[3] == 2 && header[5] & 0x40 != 0 {
            return Ok(None);
        }
        if header[3] < 4 && header[5] & 0x80 != 0 {
            unsynchronize(&mut tag);
        }
        let mut bytes = tag.as_slice();
        if header[3] >= 3 && header[5] & 0x40 != 0 {
            let len = if header[3] == 4 {
                synchsafe(take(&mut bytes, 4).ok_or_else(invalid)?).and_then(|n| n.checked_sub(4))
            } else {
                be32(&mut bytes).map(|n| n as usize)
            }
            .ok_or_else(invalid)?;
            take(&mut bytes, len).ok_or_else(invalid)?;
        }
        let frame_header = if header[3] == 2 { 6 } else { 10 };
        while bytes.len() >= frame_header && bytes[0] != 0 {
            let frame = take(&mut bytes, frame_header).ok_or_else(invalid)?;
            let len = if header[3] == 2 {
                u32::from_be_bytes([0, frame[3], frame[4], frame[5]]) as usize
            } else if header[3] == 4 {
                synchsafe(&frame[4..8]).ok_or_else(invalid)?
            } else {
                u32::from_be_bytes(frame[4..8].try_into().unwrap()) as usize
            };
            let data = take(&mut bytes, len).ok_or_else(invalid)?;
            let legacy = header[3] == 2;
            if (legacy && &frame[..3] == b"PIC") || (!legacy && &frame[..4] == b"APIC") {
                let flags = if legacy { 0 } else { frame[9] };
                if (header[3] == 3 && flags & 0xc0 != 0) || (header[3] == 4 && flags & 0x0c != 0) {
                    return Ok(None);
                }
                let mut data = std::borrow::Cow::Borrowed(data);
                if header[3] == 4 && (flags & 2 != 0 || header[5] & 0x80 != 0) {
                    unsynchronize(data.to_mut());
                }
                let mut picture = data.as_ref();
                if (header[3] == 3 && flags & 0x20 != 0) || (header[3] == 4 && flags & 0x40 != 0) {
                    take(&mut picture, 1).ok_or_else(invalid)?;
                }
                if header[3] == 4 && flags & 1 != 0 {
                    take(&mut picture, 4).ok_or_else(invalid)?;
                }
                return Ok(id3_picture(picture, legacy, self.max_bytes));
            }
        }
        Ok(None)
    }

    fn flac(&mut self, start: u64) -> io::Result<Option<Cover>> {
        self.seek(start + 4)?;
        loop {
            let header = self.header::<4>()?;
            let len = u32::from_be_bytes([0, header[1], header[2], header[3]]) as u64;
            if header[0] & 0x7f == 6 {
                return Ok(flac_picture(&self.block(len)?, self.max_bytes));
            }
            let end = self
                .input
                .stream_position()?
                .checked_add(len)
                .ok_or_else(invalid)?;
            self.seek(end)?;
            if header[0] & 0x80 != 0 {
                return Ok(None);
            }
        }
    }

    fn iff(&mut self) -> io::Result<Option<Cover>> {
        let header = self.header::<12>()?;
        let little = &header[..4] == b"RIFF";
        let number = |bytes: [u8; 4]| {
            if little {
                u32::from_le_bytes(bytes)
            } else {
                u32::from_be_bytes(bytes)
            }
        };
        let end = (number(header[4..8].try_into().unwrap()) as u64 + 8).min(self.end);
        let mut cover = None;
        while self.input.stream_position()?.saturating_add(8) <= end {
            let header = self.header::<8>()?;
            let len = number(header[4..].try_into().unwrap()) as u64;
            let next = self
                .input
                .stream_position()?
                .checked_add(len)
                .ok_or_else(invalid)?;
            if next > end {
                return Err(invalid());
            }
            if header[..4].eq_ignore_ascii_case(b"ID3 ") {
                cover = self.id3(next)?;
            }
            self.seek(next.checked_add(len % 2).ok_or_else(invalid)?)?;
        }
        Ok(cover)
    }

    fn mp4(&mut self, end: u64, depth: u8, in_cover: bool) -> io::Result<Option<Cover>> {
        if depth > 8 {
            return Err(invalid());
        }
        while self.input.stream_position()?.saturating_add(8) <= end {
            let start = self.input.stream_position()?;
            let header = self.header::<8>()?;
            let len = u32::from_be_bytes(header[..4].try_into().unwrap());
            let size = match len {
                0 => end - start,
                1 => u64::from_be_bytes(self.header::<8>()?),
                _ => len as u64,
            };
            let next = start.checked_add(size).ok_or_else(invalid)?;
            let position = self.input.stream_position()?;
            if next > end || next < position {
                return Err(invalid());
            }
            match &header[4..] {
                b"moov" | b"udta" | b"meta" | b"ilst" | b"covr" => {
                    if &header[4..] == b"meta" {
                        if next - position < 4 {
                            return Err(invalid());
                        }
                        self.header::<4>()?;
                    }
                    if let Some(cover) = self.mp4(next, depth + 1, &header[4..] == b"covr")? {
                        return Ok(Some(cover));
                    }
                }
                b"data" if in_cover => {
                    if next - position < 8 {
                        return Err(invalid());
                    }
                    let data_header = self.header::<8>()?;
                    let len = next - position - 8;
                    if len > self.max_bytes as u64 {
                        return Err(invalid());
                    }
                    let data = self.block(len)?;
                    let mime = match data_header[3] {
                        12 => "image/gif",
                        14 => "image/png",
                        27 => "image/bmp",
                        _ => "image/jpeg",
                    };
                    return Ok(Some((mime.to_owned(), data)));
                }
                _ => {}
            }
            self.seek(next)?;
        }
        Ok(None)
    }

    fn ogg(&mut self) -> io::Result<Option<Cover>> {
        let mut serial = None;
        let mut packet = Vec::new();
        loop {
            let header = self.header::<27>()?;
            if &header[..4] != b"OggS" || header[4] != 0 {
                return Err(invalid());
            }
            let stream = u32::from_le_bytes(header[14..18].try_into().unwrap());
            let selected = *serial.get_or_insert(stream) == stream;
            let mut lacing = [0u8; 255];
            let lacing = &mut lacing[..header[26] as usize];
            self.read(lacing)?;
            for &len in lacing.iter() {
                if !selected {
                    let position = self.input.stream_position()?;
                    self.seek(position + len as u64)?;
                    continue;
                }
                if packet.len() + len as usize > self.max_tag {
                    return Err(invalid());
                }
                let mut part = [0u8; 255];
                self.read(&mut part[..len as usize])?;
                let needed = packet.len() + len as usize;
                if needed > packet.capacity() {
                    let capacity = needed
                        .max(packet.capacity().saturating_mul(2))
                        .min(self.max_tag);
                    packet
                        .try_reserve_exact(capacity - packet.len())
                        .map_err(|_| invalid())?;
                }
                packet.extend_from_slice(&part[..len as usize]);
                if len < 255 {
                    if let Some(comments) = packet
                        .strip_prefix(b"\x03vorbis")
                        .or_else(|| packet.strip_prefix(b"OpusTags"))
                    {
                        return Ok(vorbis_picture(comments, self.max_bytes));
                    }
                    // Ogg FLAC carries native metadata blocks after its mapping header.
                    if packet.first().is_some_and(|byte| byte & 0x7f == 6) && packet.len() >= 4 {
                        return Ok(flac_picture(&packet[4..], self.max_bytes));
                    }
                    packet.clear();
                }
            }
            if header[5] & 4 != 0 {
                return Ok(None);
            }
        }
    }

    fn ape(&mut self) -> io::Result<Option<Cover>> {
        if self.end < 32 {
            return Ok(None);
        }
        let mut end = self.end;
        // APEv2 may precede a trailing ID3v1 tag.
        if end >= 128 {
            self.seek(end - 128)?;
            if &self.header::<3>()? == b"TAG" {
                end -= 128;
            }
        }
        if end < 32 {
            return Ok(None);
        }
        self.seek(end - 32)?;
        let footer = self.header::<32>()?;
        if &footer[..8] != b"APETAGEX" {
            return Ok(None);
        }
        let len = u32::from_le_bytes(footer[12..16].try_into().unwrap()) as u64;
        if len < 32 || len > end {
            return Err(invalid());
        }
        self.seek(end - len)?;
        let tag = self.block(len - 32)?;
        let mut bytes = tag.as_slice();
        while bytes.len() >= 8 {
            let len = le32(&mut bytes).ok_or_else(invalid)? as usize;
            let flags = le32(&mut bytes).ok_or_else(invalid)?;
            let name = terminated(&mut bytes, 1).ok_or_else(invalid)?;
            let mut value = take(&mut bytes, len).ok_or_else(invalid)?;
            if flags & 6 == 2
                && name
                    .get(..11)
                    .is_some_and(|name| name.eq_ignore_ascii_case(b"Cover Art ("))
            {
                terminated(&mut value, 1).ok_or_else(invalid)?;
                return Ok(picture("image/jpeg", value, self.max_bytes));
            }
        }
        Ok(None)
    }

    fn vint(&mut self, id: bool) -> io::Result<u64> {
        let first = self.header::<1>()?[0];
        let width = first.leading_zeros() as usize + 1;
        if width > if id { 4 } else { 8 } {
            return Err(invalid());
        }
        let mut value = u64::from(if id {
            first
        } else {
            first & ((0xffu16 >> width) as u8)
        });
        for _ in 1..width {
            value = (value << 8) | u64::from(self.header::<1>()?[0]);
        }
        if !id && value == (1u64 << (7 * width)) - 1 {
            return Ok(u64::MAX);
        }
        Ok(value)
    }

    fn ebml(&mut self, end: u64, depth: u8) -> io::Result<Option<Cover>> {
        if depth > 4 {
            return Err(invalid());
        }
        let mut mime = String::new();
        let mut data = None;
        while self.input.stream_position()? < end {
            let id = self.vint(true)?;
            let len = self.vint(false)?;
            let start = self.input.stream_position()?;
            let next = if len == u64::MAX && id == 0x18538067 {
                end
            } else {
                start.checked_add(len).ok_or_else(invalid)?
            };
            if next > end {
                return Err(invalid());
            }
            match id {
                0x18538067 | 0x1941a469 | 0x61a7 => {
                    if let Some(cover) = self.ebml(next, depth + 1)? {
                        return Ok(Some(cover));
                    }
                }
                0x4660 if depth == 3 => {
                    if len > 256 {
                        return Err(invalid());
                    }
                    mime = String::from_utf8(self.block(len)?).map_err(|_| invalid())?;
                }
                0x465c if depth == 3 => {
                    if len > self.max_bytes as u64 {
                        return Err(invalid());
                    }
                    data = Some(self.block(len)?);
                }
                _ => {}
            }
            self.seek(next)?;
        }
        Ok(data
            .filter(|_| mime.starts_with("image/"))
            .map(|data| (mime, data)))
    }
}

fn take<'a>(bytes: &mut &'a [u8], len: usize) -> Option<&'a [u8]> {
    let (head, tail) = bytes.split_at_checked(len)?;
    *bytes = tail;
    Some(head)
}

fn be32(bytes: &mut &[u8]) -> Option<u32> {
    Some(u32::from_be_bytes(take(bytes, 4)?.try_into().ok()?))
}
fn le32(bytes: &mut &[u8]) -> Option<u32> {
    Some(u32::from_le_bytes(take(bytes, 4)?.try_into().ok()?))
}

fn synchsafe(bytes: &[u8]) -> Option<usize> {
    bytes.iter().try_fold(0usize, |value, &byte| {
        (byte < 128).then_some((value << 7) | byte as usize)
    })
}

fn terminated<'a>(bytes: &mut &'a [u8], width: usize) -> Option<&'a [u8]> {
    let end = bytes
        .chunks_exact(width)
        .position(|chunk| chunk.iter().all(|byte| *byte == 0))?
        * width;
    let value = take(bytes, end)?;
    take(bytes, width)?;
    Some(value)
}

fn unsynchronize(bytes: &mut Vec<u8>) {
    let mut previous = 0;
    bytes.retain(|&byte| {
        let keep = !(previous == 0xff && byte == 0);
        previous = byte;
        keep
    });
}

fn picture(mime: &str, bytes: &[u8], max: usize) -> Option<Cover> {
    if bytes.is_empty() || bytes.len() > max || mime == "-->" {
        return None;
    }
    let mime = if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        "image/png"
    } else if bytes.starts_with(b"\xff\xd8\xff") {
        "image/jpeg"
    } else if mime.starts_with("image/") && mime.len() <= 256 {
        mime
    } else {
        "image/jpeg"
    };
    Some((mime.to_owned(), bytes.to_vec()))
}

fn id3_picture(mut bytes: &[u8], legacy: bool, max: usize) -> Option<Cover> {
    let encoding = take(&mut bytes, 1)?[0];
    let mime = if legacy {
        match take(&mut bytes, 3)? {
            b"PNG" => "image/png",
            b"JPG" => "image/jpeg",
            _ => return None,
        }
    } else {
        std::str::from_utf8(terminated(&mut bytes, 1)?).ok()?
    };
    take(&mut bytes, 1)?; // picture type
    let width = match encoding {
        0 | 3 => 1,
        1 | 2 => 2,
        _ => return None,
    };
    terminated(&mut bytes, width)?;
    picture(mime, bytes, max)
}

fn flac_picture(mut bytes: &[u8], max: usize) -> Option<Cover> {
    take(&mut bytes, 4)?; // picture type
    let len = be32(&mut bytes)? as usize;
    let mime = std::str::from_utf8(take(&mut bytes, len)?).ok()?;
    let len = be32(&mut bytes)? as usize;
    take(&mut bytes, len)?; // description
    take(&mut bytes, 16)?; // dimensions, depth, palette
    let len = be32(&mut bytes)? as usize;
    picture(mime, take(&mut bytes, len)?, max)
}

fn vorbis_picture(mut bytes: &[u8], max: usize) -> Option<Cover> {
    use base64::Engine;
    let len = le32(&mut bytes)? as usize;
    take(&mut bytes, len)?; // vendor
    let count = le32(&mut bytes)?;
    for _ in 0..count {
        let len = le32(&mut bytes)? as usize;
        let comment = take(&mut bytes, len)?;
        let Some(split) = comment.iter().position(|&byte| byte == b'=') else {
            continue;
        };
        let (name, value) = comment.split_at(split);
        if name.eq_ignore_ascii_case(b"METADATA_BLOCK_PICTURE")
            || name.eq_ignore_ascii_case(b"COVERART")
        {
            let data = base64::engine::general_purpose::STANDARD
                .decode(&value[1..])
                .ok()?;
            return if name.eq_ignore_ascii_case(b"COVERART") {
                picture("image/jpeg", &data, max)
            } else {
                flac_picture(&data, max)
            };
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn id3(image: &[u8], version: u8) -> Vec<u8> {
        let mut payload = b"\0image/png\0\x03\0".to_vec();
        payload.extend_from_slice(image);
        let mut frame = b"APIC".to_vec();
        frame.extend_from_slice(&if version == 4 {
            sync_bytes(payload.len())
        } else {
            (payload.len() as u32).to_be_bytes()
        });
        frame.extend_from_slice(&[0, 0]);
        frame.extend_from_slice(&payload);
        let mut tag = vec![b'I', b'D', b'3', version, 0, 0];
        tag.extend_from_slice(&sync_bytes(frame.len()));
        tag.extend(frame);
        tag
    }

    fn sync_bytes(size: usize) -> [u8; 4] {
        [
            ((size >> 21) & 127) as u8,
            ((size >> 14) & 127) as u8,
            ((size >> 7) & 127) as u8,
            (size & 127) as u8,
        ]
    }

    fn extract(bytes: Vec<u8>, max: usize) -> Option<Cover> {
        CoverReader::new(Cursor::new(bytes), max)
            .unwrap()
            .cover()
            .ok()
            .flatten()
    }

    fn flac_block(image: &[u8]) -> Vec<u8> {
        let mut block = 3u32.to_be_bytes().to_vec();
        block.extend_from_slice(&9u32.to_be_bytes());
        block.extend_from_slice(b"image/png");
        block.extend_from_slice(&[0; 20]); // empty description and dimensions
        block.extend_from_slice(&(image.len() as u32).to_be_bytes());
        block.extend_from_slice(image);
        block
    }

    #[test]
    fn id3_versions_and_flac_respect_the_exact_image_limit() {
        let image = vec![0xab; 1024];
        for version in [3, 4] {
            assert_eq!(extract(id3(&image, version), 1024).unwrap().1, image);
            assert!(extract(id3(&image, version), 1023).is_none());
        }
        let block = flac_block(&image);
        let mut flac = b"fLaC".to_vec();
        flac.push(0x86);
        flac.extend_from_slice(&(block.len() as u32).to_be_bytes()[1..]);
        flac.extend(block);
        assert_eq!(extract(flac.clone(), 1024).unwrap().1, image);
        let mut prefixed = id3(b"older picture", 4);
        prefixed[5] = 0x10;
        prefixed.extend_from_slice(b"3DI\x04\0\0\0\0\0\0");
        prefixed.extend_from_slice(&flac);
        assert_eq!(extract(prefixed, 1024).unwrap().1, image);
        assert!(extract(flac, 1023).is_none());
    }

    // Exposes a large declared file without storing its payload. A read into
    // that payload panics, so the test proves rejection precedes reading it.
    struct HeaderOnly {
        bytes: Cursor<Vec<u8>>,
        file_len: u64,
    }
    impl Read for HeaderOnly {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            assert!(
                self.bytes.position() + buf.len() as u64 <= self.bytes.get_ref().len() as u64,
                "oversized payload was requested"
            );
            self.bytes.read(buf)
        }
    }
    impl Seek for HeaderOnly {
        fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
            match position {
                SeekFrom::End(delta) => self.bytes.seek(SeekFrom::Start(
                    self.file_len.checked_add_signed(delta).unwrap(),
                )),
                position => self.bytes.seek(position),
            }
        }
    }

    #[test]
    fn oversized_id3_is_rejected_before_its_payload_is_read() {
        let len = 256 * 1024;
        let mut header = b"ID3\x03\0\0".to_vec();
        header.extend_from_slice(&sync_bytes(len));
        header.extend_from_slice(b"AP"); // format sniffing needs twelve bytes
        let input = HeaderOnly {
            bytes: Cursor::new(header),
            file_len: len as u64 + 10,
        };
        let mut reader = CoverReader::new(input, 1024).unwrap();
        assert_eq!(
            reader.cover().unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
        assert_eq!(reader.input.bytes.position(), 10);
    }

    #[test]
    fn oversized_flac_and_mp4_are_rejected_before_their_payloads_are_read() {
        let len = 256 * 1024;
        let mut flac = b"fLaC\x86".to_vec();
        flac.extend_from_slice(&(len as u32).to_be_bytes()[1..]);
        flac.extend_from_slice(&[0; 4]); // format sniffing
        let input = HeaderOnly {
            bytes: Cursor::new(flac),
            file_len: len + 8,
        };
        assert!(CoverReader::new(input, 1024).unwrap().cover().is_err());

        let mut mp4 = atom(b"ftyp", b"M4A \0\0\0\0");
        mp4.extend_from_slice(&((len + 24) as u32).to_be_bytes());
        mp4.extend_from_slice(b"covr");
        mp4.extend_from_slice(&((len + 16) as u32).to_be_bytes());
        mp4.extend_from_slice(b"data\0\0\0\x0e\0\0\0\0");
        let input = HeaderOnly {
            file_len: len + mp4.len() as u64,
            bytes: Cursor::new(mp4),
        };
        assert!(CoverReader::new(input, 1024).unwrap().cover().is_err());
    }

    #[test]
    fn id3_unsynchronization_and_legacy_pictures_are_supported() {
        let original = b"\xff\xe0\xff\0\x12";
        let mut encoded = Vec::new();
        for &byte in original {
            encoded.push(byte);
            if byte == 0xff {
                encoded.push(0);
            }
        }
        let mut tag = id3(&encoded, 4);
        tag[19] = 2; // frame-level unsynchronization
        assert_eq!(extract(tag, 1024).unwrap().1, original);

        let raw = id3(original, 3);
        let mut tag = raw[..10].to_vec();
        tag[5] = 0x80;
        for &byte in &raw[10..] {
            tag.push(byte);
            if byte == 0xff {
                tag.push(0);
            }
        }
        let len = sync_bytes(tag.len() - 10);
        tag[6..10].copy_from_slice(&len);
        assert_eq!(extract(tag, 1024).unwrap().1, original);

        let mut payload = b"\0PNG\x03\0".to_vec();
        payload.extend_from_slice(original);
        let mut tag = b"ID3\x02\0\0".to_vec();
        tag.extend_from_slice(&sync_bytes(payload.len() + 6));
        tag.extend_from_slice(b"PIC");
        tag.extend_from_slice(&(payload.len() as u32).to_be_bytes()[1..]);
        tag.extend(payload);
        assert_eq!(extract(tag, 1024).unwrap().1, original);
    }

    fn atom(name: &[u8; 4], data: &[u8]) -> Vec<u8> {
        let mut bytes = ((data.len() + 8) as u32).to_be_bytes().to_vec();
        bytes.extend_from_slice(name);
        bytes.extend_from_slice(data);
        bytes
    }

    #[test]
    fn mp4_seeks_past_audio_and_checks_the_cover_before_reading() {
        let image = vec![0xab; 128];
        let mut data = vec![0, 0, 0, 14, 0, 0, 0, 0];
        data.extend_from_slice(&image);
        let ilst = atom(b"ilst", &atom(b"covr", &atom(b"data", &data)));
        let mut meta = vec![0; 4];
        meta.extend(ilst);
        let mut file = atom(b"ftyp", b"M4A \0\0\0\0");
        file.extend(atom(b"mdat", &[0; 128 * 1024]));
        file.extend(atom(b"moov", &atom(b"udta", &atom(b"meta", &meta))));
        assert_eq!(extract(file.clone(), 128).unwrap().1, image);
        assert!(extract(file, 127).is_none());
    }

    #[test]
    fn opus_and_vorbis_base64_pictures_are_bounded() {
        use base64::Engine;
        let image = vec![0xab; 1024];
        let comment = format!(
            "METADATA_BLOCK_PICTURE={}",
            base64::engine::general_purpose::STANDARD.encode(flac_block(&image))
        );
        for prefix in [b"OpusTags".as_slice(), b"\x03vorbis".as_slice()] {
            let mut packet = prefix.to_vec();
            packet.extend_from_slice(&0u32.to_le_bytes());
            packet.extend_from_slice(&1u32.to_le_bytes());
            packet.extend_from_slice(&(comment.len() as u32).to_le_bytes());
            packet.extend_from_slice(comment.as_bytes());
            let mut page = b"OggS".to_vec();
            page.extend_from_slice(&[0; 22]);
            let count = packet.len() / 255 + 1;
            page.push(count as u8);
            page.extend(std::iter::repeat_n(255u8, count - 1));
            page.push((packet.len() % 255) as u8);
            page.extend(packet);
            assert_eq!(extract(page.clone(), 1024).unwrap().1, image);
            assert!(extract(page, 1023).is_none());
        }
    }

    #[test]
    fn malformed_lengths_and_metadata_bursts_are_rejected() {
        let mut malformed = id3(b"small", 3);
        malformed[14..18].copy_from_slice(&u32::MAX.to_be_bytes());
        assert!(extract(malformed, 1024).is_none());
        // Repeated small tags cannot evade the aggregate read budget.
        let tag = id3(&[0; 1024], 3);
        let mut chunks = Vec::new();
        for _ in 0..256 {
            chunks.extend_from_slice(b"ID3 ");
            chunks.extend_from_slice(&(tag.len() as u32).to_be_bytes());
            chunks.extend_from_slice(&tag);
            if tag.len() % 2 == 1 {
                chunks.push(0);
            }
        }
        let mut file = b"FORM".to_vec();
        file.extend_from_slice(&((chunks.len() + 4) as u32).to_be_bytes());
        file.extend_from_slice(b"AIFF");
        file.extend(chunks);
        assert!(extract(file, 1024).is_none());
    }

    #[test]
    fn ape_cover_and_matroska_attachment_are_supported() {
        let image = b"some picture";
        let mut ape = vec![0; 12];
        let mut item = ((image.len() + 1) as u32).to_le_bytes().to_vec();
        item.extend_from_slice(&2u32.to_le_bytes());
        item.extend_from_slice(b"Cover Art (Front)\0\0");
        item.extend_from_slice(image);
        let mut footer = b"APETAGEX".to_vec();
        footer.extend_from_slice(&2000u32.to_le_bytes());
        footer.extend_from_slice(&((item.len() + 32) as u32).to_le_bytes());
        footer.extend_from_slice(&1u32.to_le_bytes());
        footer.extend_from_slice(&[0; 12]);
        ape.extend(item);
        ape.extend(footer);
        assert_eq!(extract(ape, image.len()).unwrap().1, image);

        fn element(id: &[u8], data: &[u8]) -> Vec<u8> {
            assert!(data.len() < 127);
            let mut bytes = id.to_vec();
            bytes.push(0x80 | data.len() as u8);
            bytes.extend_from_slice(data);
            bytes
        }
        let mut attachment = element(&[0x46, 0x60], b"image/png");
        attachment.extend(element(&[0x46, 0x5c], image));
        let attached = element(&[0x61, 0xa7], &attachment);
        let attachments = element(&[0x19, 0x41, 0xa4, 0x69], &attached);
        let mut mkv = element(&[0x1a, 0x45, 0xdf, 0xa3], &[]);
        mkv.extend_from_slice(&[
            0x18, 0x53, 0x80, 0x67, 0x01, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        ]);
        mkv.extend(attachments);
        assert_eq!(extract(mkv, image.len()).unwrap().1, image);
    }
}
