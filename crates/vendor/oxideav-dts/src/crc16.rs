//! Annex B (normative) "CRC Algorithm" — the single CRC-16 every DTS
//! Coherent Acoustics check word uses.
//!
//! Transcribed from ETSI TS 102 114 V1.3.1 Annex B (printed p.181),
//! staged as `docs/audio/dts/dts-crc16.md`. Annex B names the
//! algorithm "CRC-CCITT" and fixes every parameter:
//!
//! | Parameter | Value |
//! |-----------|-------|
//! | Width | 16 bits |
//! | Generator polynomial | `G(x) = x¹⁶ + x¹² + x⁵ + 1` (`0x1021`) |
//! | Initial value | `0xFFFF` ("initialized to the value of 0xFFFF before checksum computation commences") |
//! | Bit order | MSB-first (un-reflected) |
//! | Final XOR | none |
//!
//! This parameter set is commonly catalogued as **CRC-16/CCITT-FALSE**.
//! The bitstream zero-pads every protected region to a byte boundary
//! before its CRC field (the §5.x `ByteAlign…` fields) precisely so a
//! byte-wise table implementation can run — this module provides that
//! table form ([`DTS_CRC16_TABLE`], derived purely from the
//! polynomial) plus the incremental update entry point for callers
//! that checksum a region in pieces.
//!
//! ## Where the check words live
//!
//! Every DTS CRC field uses this one algorithm; each field's coverage
//! span runs from the first byte of the protected region up to the
//! byte immediately preceding the CRC field, inclusive
//! (`docs/audio/dts/dts-crc16.md` "Where CRC-16 is applied"):
//!
//! * Core substream (§5): `HCRC` / `AHCRC` / `SICRC` / `OCRC` when
//!   `CPF == 1` — **extracted but not tested** (the spec states "The
//!   CRC value test shall not be applied" for these core fields; they
//!   are informational placeholders).
//! * `nAUXCRC16` (§5.6/§5.7.1) — genuinely verified over the aux data
//!   from `bAUXTimeStampFlag` to the byte before the CRC
//!   ([`crate::AuxData::crc_valid`]).
//! * `nRev2AUXCRC16` (§5.7.2) — genuinely verified over
//!   `nRev2AUXDataByteSize − 2` bytes starting at the size field
//!   ([`crate::Rev2AuxChunk::crc_valid`]).
//! * Extension substreams (§6, e.g. `nCRC16HeaderX96`) — reference
//!   the same Annex-B algorithm; used during sync detection to reject
//!   false sync words.
//!
//! This module is feature-independent (no `oxideav-core` dep), so it
//! is available under both the default and `--no-default-features`
//! build modes.

/// The Annex B generator polynomial in normal (MSB-first) form:
/// `G(x) = x¹⁶ + x¹² + x⁵ + 1`.
pub const DTS_CRC16_POLY: u16 = 0x1021;

/// The Annex B initial register value: "The CRC16 is initialized to
/// the value of 0xFFFF before checksum computation commences."
pub const DTS_CRC16_INIT: u16 = 0xFFFF;

/// Build one row of the byte-wise CRC table: the register evolution
/// of a single byte `i` shifted through the MSB-first polynomial
/// division. Pure data derived from [`DTS_CRC16_POLY`].
const fn table_entry(i: u16) -> u16 {
    let mut crc = i << 8;
    let mut b = 0;
    while b < 8 {
        crc = if crc & 0x8000 != 0 {
            (crc << 1) ^ DTS_CRC16_POLY
        } else {
            crc << 1
        };
        b += 1;
    }
    crc
}

const fn build_table() -> [u16; 256] {
    let mut table = [0u16; 256];
    let mut i = 0;
    while i < 256 {
        table[i] = table_entry(i as u16);
        i += 1;
    }
    table
}

/// The 256-entry byte-wise lookup table (MSB-first, seeded from
/// [`DTS_CRC16_POLY`]) — the "fast table-based CRC16 calculation" form
/// the spec's byte-alignment fields exist to enable. Generated at
/// compile time from the polynomial alone.
pub static DTS_CRC16_TABLE: [u16; 256] = build_table();

/// Fold `bytes` into a running Annex B CRC-16 register value.
///
/// Start `crc` at [`DTS_CRC16_INIT`] (or use [`dts_crc16`] for the
/// one-shot form); feed successive slices of the protected region in
/// order. No reflection and no final XOR are applied — the returned
/// register value **is** the check word the bitstream carries.
#[must_use]
pub fn dts_crc16_update(crc: u16, bytes: &[u8]) -> u16 {
    let mut crc = crc;
    for &byte in bytes {
        let idx = ((crc >> 8) ^ u16::from(byte)) & 0xFF;
        crc = (crc << 8) ^ DTS_CRC16_TABLE[idx as usize];
    }
    crc
}

/// Compute the Annex B CRC-16 (CRC-CCITT: polynomial `0x1021`, initial
/// value `0xFFFF`, MSB-first, no reflection, no final XOR) over a
/// byte-aligned protected region.
///
/// The result equals the 16-bit check word the DTS bitstream stores
/// immediately after the region; a receiver verifies by recomputing
/// over the same span and comparing (`docs/audio/dts/dts-crc16.md`).
#[must_use]
pub fn dts_crc16(bytes: &[u8]) -> u16 {
    dts_crc16_update(DTS_CRC16_INIT, bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Bit-at-a-time reference implementation straight from the Annex B
    /// parameters (the doc's "reference implementation sketch"),
    /// independent of the table generation above.
    fn bitwise_reference(bytes: &[u8]) -> u16 {
        let mut crc = DTS_CRC16_INIT;
        for &byte in bytes {
            crc ^= u16::from(byte) << 8;
            for _ in 0..8 {
                crc = if crc & 0x8000 != 0 {
                    (crc << 1) ^ DTS_CRC16_POLY
                } else {
                    crc << 1
                };
            }
        }
        crc
    }

    /// An empty region leaves the register at the initial value: no
    /// bytes, no division steps.
    #[test]
    fn empty_input_is_init_value() {
        assert_eq!(dts_crc16(&[]), DTS_CRC16_INIT);
    }

    /// The catalogued CRC-16/CCITT-FALSE check value: the ASCII bytes
    /// `"123456789"` produce `0x29B1` under poly `0x1021` / init
    /// `0xFFFF` / no reflection / no final XOR — the standard
    /// check-value row for the parameter set Annex B specifies.
    #[test]
    fn catalog_check_value() {
        assert_eq!(dts_crc16(b"123456789"), 0x29B1);
        assert_eq!(bitwise_reference(b"123456789"), 0x29B1);
    }

    /// Table row 0 is zero (no set bits, no reduction) and row 1 is the
    /// polynomial itself shifted into place.
    #[test]
    fn table_anchor_rows() {
        assert_eq!(DTS_CRC16_TABLE[0], 0);
        assert_eq!(DTS_CRC16_TABLE[1], DTS_CRC16_POLY);
    }

    /// The byte-wise table form agrees with the bit-at-a-time Annex B
    /// reference over a deterministic pseudo-random buffer.
    #[test]
    fn table_form_matches_bitwise_reference() {
        // xorshift-style deterministic byte stream, no external data.
        let mut state = 0x1234_5678_u32;
        let bytes: Vec<u8> = (0..4096)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                (state >> 24) as u8
            })
            .collect();
        for len in [0usize, 1, 2, 3, 15, 16, 17, 255, 256, 4096] {
            assert_eq!(
                dts_crc16(&bytes[..len]),
                bitwise_reference(&bytes[..len]),
                "length {len}"
            );
        }
    }

    /// Incremental folding over arbitrary splits equals the one-shot
    /// computation (the ByteAlign'd regions are checksummed bytewise,
    /// so any byte-boundary split must be transparent).
    #[test]
    fn incremental_update_matches_one_shot() {
        let bytes: Vec<u8> = (0u16..777)
            .map(|i| (i.wrapping_mul(31) >> 3) as u8)
            .collect();
        let whole = dts_crc16(&bytes);
        for split in [0usize, 1, 76, 400, 776, 777] {
            let partial = dts_crc16_update(DTS_CRC16_INIT, &bytes[..split]);
            assert_eq!(dts_crc16_update(partial, &bytes[split..]), whole);
        }
    }

    /// A single-bit corruption anywhere in the region changes the check
    /// word (CRC-16 detects all single-bit errors by construction —
    /// G(x) has more than one term).
    #[test]
    fn detects_single_bit_flips() {
        let bytes: Vec<u8> = (0..64u8).collect();
        let reference = dts_crc16(&bytes);
        for byte in 0..bytes.len() {
            for bit in 0..8 {
                let mut corrupted = bytes.clone();
                corrupted[byte] ^= 1 << bit;
                assert_ne!(
                    dts_crc16(&corrupted),
                    reference,
                    "flip at byte {byte} bit {bit} went undetected"
                );
            }
        }
    }
}
