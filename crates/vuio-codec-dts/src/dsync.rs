//! DTS Coherent Acoustics — §5.5 Table 5-29 `DSYNC` subsubframe
//! synchronization check word.
//!
//! Source: ETSI TS 102 114 V1.3.1 (2011-08), staged PDF at
//! `docs/audio/dts/etsi-ts-102114-dts-coherent-acoustics.pdf`
//! (Table 5-29 pseudocode on PDF p.32, prose on PDF p.33).
//!
//! The final step of the §5.5 per-subsubframe `Audio Data` walk is a
//! conditional 16-bit synchronization word read. Transcribed verbatim
//! from the staged Table 5-29 pseudocode (PDF p.32):
//!
//! ```text
//! // Check for DSYNC
//! if ( (nSubSubFrame==(nSSC-1)) || (ASPF==1) ) {
//!   DSYNC = ExtractBits(16);
//!   if ( DSYNC != 0xffff )
//!     printf("DSYNC error at end of subsubframe #%d", nSubSubFrame);
//! }
//! ```
//!
//! and the §5.5 prose (PDF p.33, "AUDIO (Audio data)"):
//!
//! > "At end of each subsubframe there may be a synchronization check
//! > word `DSYNC = 0xffff` depending on the flag `ASPF` in the frame
//! > header, but there must be at least a DSYNC at the end of each
//! > subframe."
//!
//! Two facts fix the gating completely:
//!
//! * **End of subframe** (`nSubSubFrame == nSSC - 1`, the last
//!   subsubframe of the audio subframe): a DSYNC word is *always*
//!   present here regardless of `ASPF`. This is the "at least a DSYNC
//!   at the end of each subframe" guarantee.
//! * **`ASPF == 1`** (the §5.3.2 "Audio Sync-Word Insertion Flag" of
//!   the frame header, [`crate::DtsFrameHeader::aspf`]): a DSYNC word
//!   is present after *every* subsubframe, not only the last.
//!
//! When neither condition holds, no DSYNC word is read and the bit
//! cursor stays on the next subsubframe's first audio field.
//!
//! This module exposes the gating predicate
//! ([`dsync_present`]), the expected word
//! ([`DSYNC_WORD`]), and the bit-stream reader
//! ([`decode_dsync_at`]) that reads the 16-bit field and verifies it
//! against `0xffff`. It composes the round-281 `aspf` header field
//! with the round-249 [`crate::SubsubframeCount::n_ssc`] count to
//! drive the §5.5 walker's trailer step.

use crate::bitreader::BitReader;
use crate::{Error, Result};

/// The §5.5 `DSYNC` synchronization check word value, `0xffff`
/// (PDF p.32: `if ( DSYNC != 0xffff )`). A correctly framed
/// subsubframe trailer carries exactly this 16-bit pattern.
pub const DSYNC_WORD: u16 = 0xffff;

/// Wire width of the `DSYNC` field, in bits: `ExtractBits(16)`
/// (PDF p.32).
pub const DSYNC_WIRE_BITS: u32 = 16;

/// The §5.5 gating predicate for the per-subsubframe `DSYNC` trailer
/// (PDF p.32): is a 16-bit `DSYNC` word present after subsubframe
/// `n_subsubframe`?
///
/// Transcribes the spec's `if ( (nSubSubFrame==(nSSC-1)) ||
/// (ASPF==1) )` condition exactly:
///
/// * `n_subsubframe` — the zero-based index of the subsubframe whose
///   audio data was just unpacked (`nSubSubFrame`, the §5.5 loop
///   variable `for (nSubSubFrame=0; nSubSubFrame<nSSC; …)`).
/// * `n_ssc` — the subsubframe count `nSSC = SSC + 1` from
///   [`crate::SubsubframeCount::n_ssc`] (`1..=4`).
/// * `aspf` — the §5.3.2 Audio Sync-Word Insertion Flag,
///   [`crate::DtsFrameHeader::aspf`].
///
/// Returns `true` when `n_subsubframe` is the last subsubframe of the
/// subframe (`n_subsubframe + 1 == n_ssc`, the "at least a DSYNC at
/// the end of each subframe" guarantee) or when `aspf` is set (a DSYNC
/// after every subsubframe). The last-subsubframe check is written as
/// `n_subsubframe + 1 == n_ssc` rather than `n_subsubframe == n_ssc - 1`
/// to avoid an underflow if a caller passes the degenerate `n_ssc == 0`
/// (the field is `SSC + 1 >= 1` on the wire, so this only guards
/// against a malformed caller, never a real bit stream).
#[must_use]
pub fn dsync_present(n_subsubframe: u8, n_ssc: u8, aspf: bool) -> bool {
    aspf || (n_ssc != 0 && n_subsubframe + 1 == n_ssc)
}

/// Read and verify the §5.5 `DSYNC` synchronization check word at
/// `bit_offset` in `bytes` (PDF p.32), returning the number of bits
/// consumed (always [`DSYNC_WIRE_BITS`] = 16) on success.
///
/// The bit offset is measured from the MSB of `bytes[0]`, matching the
/// MSB-first convention used elsewhere in this crate. The 16-bit field
/// is read big-endian (`ExtractBits(16)`) and compared against
/// [`DSYNC_WORD`] (`0xffff`).
///
/// Errors:
///
/// * [`Error::UnexpectedEof`] when fewer than 16 bits remain after
///   `bit_offset`.
/// * [`Error::DsyncMismatch`] when the 16 bits read are not `0xffff`.
///   The spec text only `printf`s a diagnostic at this point and keeps
///   decoding; this API surfaces the mismatch as a recoverable typed
///   error carrying the bad word and the subsubframe index so the
///   caller can choose whether to treat it as fatal (it is the only
///   in-band integrity check the core profile provides for the audio
///   data array).
///
/// `n_subsubframe` is the zero-based subsubframe index this trailer
/// follows; it is threaded only into the [`Error::DsyncMismatch`]
/// diagnostic to mirror the spec's `"DSYNC error at end of subsubframe
/// #%d"` message. The caller is responsible for having already checked
/// [`dsync_present`] — this reader unconditionally consumes 16 bits.
pub fn decode_dsync_at(bytes: &[u8], bit_offset: usize, n_subsubframe: u8) -> Result<usize> {
    let byte_offset = bit_offset / 8;
    let intra_byte = bit_offset % 8;
    let mut br = BitReader::from_byte_offset(bytes, byte_offset);
    if intra_byte > 0 {
        br.read_bits(intra_byte as u32)?;
    }
    let word = br.read_bits(DSYNC_WIRE_BITS)? as u16;
    if word != DSYNC_WORD {
        return Err(Error::DsyncMismatch {
            found: word,
            n_subsubframe,
        });
    }
    Ok(DSYNC_WIRE_BITS as usize)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dsync_word_constants() {
        assert_eq!(DSYNC_WORD, 0xffff);
        assert_eq!(DSYNC_WIRE_BITS, 16);
    }

    #[test]
    fn present_only_at_last_subsubframe_when_aspf_clear() {
        // ASPF == 0: a DSYNC follows only the last subsubframe of the
        // subframe (n_subsubframe + 1 == n_ssc).
        // nSSC = 4 → subsubframes 0,1,2,3; only 3 carries a DSYNC.
        assert!(!dsync_present(0, 4, false));
        assert!(!dsync_present(1, 4, false));
        assert!(!dsync_present(2, 4, false));
        assert!(dsync_present(3, 4, false));
    }

    #[test]
    fn present_after_every_subsubframe_when_aspf_set() {
        // ASPF == 1: a DSYNC follows every subsubframe.
        for n_ssc in 1u8..=4 {
            for n in 0..n_ssc {
                assert!(
                    dsync_present(n, n_ssc, true),
                    "ASPF set: DSYNC must follow subsubframe {n} of {n_ssc}"
                );
            }
        }
    }

    #[test]
    fn single_subsubframe_subframe_always_has_dsync() {
        // nSSC = 1 → the lone subsubframe (index 0) is also the last,
        // so it always carries a DSYNC even with ASPF clear.
        assert!(dsync_present(0, 1, false));
        assert!(dsync_present(0, 1, true));
    }

    #[test]
    fn full_gating_matrix_matches_spec_condition() {
        // Exhaustively cross-check dsync_present against the spec's
        // `(nSubSubFrame == nSSC-1) || (ASPF==1)` for every
        // (n_subsubframe, n_ssc, aspf) the wire can produce
        // (nSSC in 1..=4, n_subsubframe in 0..nSSC).
        for n_ssc in 1u8..=4 {
            for n in 0..n_ssc {
                for &aspf in &[false, true] {
                    let want = aspf || (n + 1 == n_ssc);
                    assert_eq!(
                        dsync_present(n, n_ssc, aspf),
                        want,
                        "n={n} n_ssc={n_ssc} aspf={aspf}"
                    );
                }
            }
        }
    }

    #[test]
    fn degenerate_zero_nssc_does_not_underflow() {
        // A malformed caller passing n_ssc == 0 (never produced by the
        // wire, since SSC + 1 >= 1) must not panic on the
        // `n_subsubframe + 1 == n_ssc` check. With ASPF clear it
        // reports no DSYNC; with ASPF set it still reports present.
        assert!(!dsync_present(0, 0, false));
        assert!(dsync_present(0, 0, true));
    }

    #[test]
    fn decode_valid_word_byte_aligned() {
        // 0xffff at bit 0.
        let bytes = [0xff, 0xff];
        assert_eq!(decode_dsync_at(&bytes, 0, 3).unwrap(), 16);
    }

    #[test]
    fn decode_valid_word_non_aligned() {
        // 5 leading filler bits, then 0xffff, then trailing pad.
        // bits: 00000 1111111111111111 000
        // byte0 = 0b00000_111 = 0x07
        // byte1 = 0b11111111 = 0xff
        // byte2 = 0b11111_000 = 0xf8
        let bytes = [0x07, 0xff, 0xf8];
        assert_eq!(decode_dsync_at(&bytes, 5, 0).unwrap(), 16);
    }

    #[test]
    fn decode_valid_word_crossing_byte_boundary() {
        // bit_offset 4: nibble of filler then 0xffff straddling 3 bytes.
        // byte0 low nibble = 1111, byte1 = 0xff, byte2 high nibble = 1111
        let bytes = [0x0f, 0xff, 0xf0];
        assert_eq!(decode_dsync_at(&bytes, 4, 1).unwrap(), 16);
    }

    #[test]
    fn decode_mismatch_surfaces_bad_word_and_index() {
        // 0xfffe is not the sync word.
        let bytes = [0xff, 0xfe];
        let err = decode_dsync_at(&bytes, 0, 2).unwrap_err();
        assert_eq!(
            err,
            Error::DsyncMismatch {
                found: 0xfffe,
                n_subsubframe: 2,
            }
        );
    }

    #[test]
    fn decode_zero_word_is_mismatch() {
        let bytes = [0x00, 0x00];
        assert_eq!(
            decode_dsync_at(&bytes, 0, 0).unwrap_err(),
            Error::DsyncMismatch {
                found: 0x0000,
                n_subsubframe: 0,
            }
        );
    }

    #[test]
    fn decode_eof_when_fewer_than_16_bits_remain() {
        // Only 8 bits available.
        let bytes = [0xff];
        assert_eq!(
            decode_dsync_at(&bytes, 0, 0).unwrap_err(),
            Error::UnexpectedEof
        );
    }

    #[test]
    fn decode_eof_when_offset_leaves_too_few_bits() {
        // 16 bits total, offset 1 leaves only 15.
        let bytes = [0xff, 0xff];
        assert_eq!(
            decode_dsync_at(&bytes, 1, 0).unwrap_err(),
            Error::UnexpectedEof
        );
    }

    #[test]
    fn decode_consumes_exactly_sixteen_bits() {
        // A valid word followed by a distinct trailing byte; the
        // reader reports 16 bits consumed (the trailing byte is not
        // touched). Confirmed indirectly by reading a second field
        // immediately after via a fresh offset.
        let bytes = [0xff, 0xff, 0xab];
        let consumed = decode_dsync_at(&bytes, 0, 0).unwrap();
        assert_eq!(consumed, 16);
        // The next field begins at bit 16; reading it must see 0xab.
        let mut br = BitReader::from_byte_offset(&bytes, 16 / 8);
        assert_eq!(br.read_bits(8).unwrap(), 0xab);
    }
}
