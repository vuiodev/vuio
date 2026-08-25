//! AC-3 / E-AC-3 frame error-detection — CRC-16 (poly 0x8005).
//!
//! ATSC A/52:2018 §7.10.1 specifies a 16-bit CRC computed by a linear
//! feedback shift register over the generator polynomial
//! `x^16 + x^15 + x^2 + 1`, i.e. binary `1_1000_0000_0000_0101` (the
//! leading `1` is the implicit `x^16`, so the feedback mask is
//! `0x8005`). Two CRC fields appear in every AC-3 syncframe:
//!
//! * **`crc1`** — second 16-bit word, covers the first 5/8 of the
//!   syncframe (excluding the syncword). §5.4.1.2 + §7.10.1.
//! * **`crc2`** — last 16-bit word, covers the entire syncframe
//!   excluding the syncword. §5.4.5.2 + §7.10.1.
//!
//! E-AC-3 (Annex E) syncframes carry only `crc2`; the spec elides
//! `crc1` because the variable-length frame body removes the
//! 5/8-checkpoint utility (§E.1.2 / Table E1.2).
//!
//! Per §7.10.1 the spec's reference check is **residue-based**:
//! shift the post-syncword data through the LFSR (with the stored
//! CRC bytes included), and the register must read zero at the
//! end. Validated empirically against the FFmpeg-produced
//! `tests/fixtures/sine440_stereo.ac3` corpus — every syncframe
//! satisfies `residue([2..frame_end]) == 0` AND
//! `residue([2..crc1_end]) == 0`. The verifier below implements
//! that residue check.
//!
//! Our own AC-3 and E-AC-3 encoders emit both CRC words in the
//! spec's reference form: `crc1` is solved via
//! `ac3_crc_solve_prefix_with` (a closed form over GF(2)) so the
//! LFSR reaches zero at the 5/8 boundary, and `crc2` is computed
//! in **augmented** form (`ac3_crc_update(0, body || [0, 0])`) so
//! the LFSR reaches zero at frame end. The augmented form follows
//! the standard CRC codeword property `data·x^16 + r(x) ≡ 0 mod
//! g(x)` — see encoder `emit_*_packet` for the placement.
//!
//! Both checks are bit-exact CRC-16 over poly 0x8005, MSB-first.

/// The CRC-16 LFSR feedback mask `1000_0000_0000_0101` (without the
/// implicit `x^16` term) — see §7.10.1.
pub(crate) const AC3_CRC_POLY: u16 = 0x8005;

/// Byte-wise CRC-16 table for `AC3_CRC_POLY`, MSB-first.
///
/// `CRC_TAB[h]` is eight input-free LFSR steps applied to `h << 8` — the
/// feedback the register's high byte generates as it shifts out.
///
/// Note this register is not the textbook MSB-first arrangement. §7.10.1's
/// recurrence shifts the *data bit into the low bit*
/// (`c' = (c << 1) | bit`, then feedback), so a byte enters at the bottom
/// rather than being XORed into the table index:
///
/// ```text
///   textbook:  c' = (c << 8) ^ TAB[(c >> 8) ^ byte]
///   here:      c' = (c << 8) ^ TAB[c >> 8] ^ byte
/// ```
///
/// Both absorb a byte per lookup instead of eight shift-and-branch steps;
/// using the wrong one silently produces a bitstream no decoder accepts,
/// which is what `table_crc_matches_the_bitwise_lfsr` guards against.
/// Built at compile time, so it costs no initialisation and lives in
/// `.rodata`.
const CRC_TAB: [u16; 256] = {
    let mut tab = [0u16; 256];
    let mut b = 0usize;
    while b < 256 {
        let mut crc = (b as u16) << 8;
        let mut i = 0;
        while i < 8 {
            crc = if crc & 0x8000 != 0 {
                (crc << 1) ^ AC3_CRC_POLY
            } else {
                crc << 1
            };
            i += 1;
        }
        tab[b] = crc;
        b += 1;
    }
    tab
};

/// Plain MSB-first CRC-16 over a byte slice using `AC3_CRC_POLY`.
///
/// Bit order within each byte is MSB-first to match the AC-3 bitstream
/// orientation (§5.3 transmission rule). Driven a byte at a time through
/// [`CRC_TAB`]; see [`ac3_crc_update_bitwise`] for the one-bit-at-a-time
/// definition this is equivalent to, which the tests check it against.
///
/// `init` lets callers chain partial updates — most call-sites use
/// `init = 0` so the LFSR starts cleared per §7.10.1.
#[inline]
pub(crate) fn ac3_crc_update(init: u16, data: &[u8]) -> u16 {
    let mut crc = init;
    for &b in data {
        crc = (crc << 8) ^ CRC_TAB[(crc >> 8) as usize] ^ b as u16;
    }
    crc
}

/// The literal §7.10.1 LFSR, one bit at a time.
///
/// Kept as the definition [`ac3_crc_update`] is tested against — the table
/// version is an optimisation of exactly this, and a table built wrong would
/// otherwise be invisible until a decoder rejected our frames.
#[cfg(test)]
pub(crate) fn ac3_crc_update_bitwise(init: u16, data: &[u8]) -> u16 {
    let mut crc: u32 = init as u32;
    for &b in data {
        for i in (0..8).rev() {
            let bit = ((b >> i) & 1) as u32;
            let top = (crc >> 15) & 1;
            crc = ((crc << 1) & 0xFFFF) | bit;
            if top != 0 {
                crc ^= AC3_CRC_POLY as u32;
            }
        }
    }
    crc as u16
}

/// Carry-less multiply of two GF(2) polynomials modulo `poly`.
///
/// `a` and `b` are degree-15 polynomials held in a `u16`, with the
/// implicit `x^16` term supplied by `poly` (= `AC3_CRC_POLY`). Used to
/// undo the shift that `crc1`'s position in the frame imposes — see
/// [`ac3_crc_solve_prefix`].
fn mul_poly(mut a: u16, mut b: u16, poly: u16) -> u16 {
    let mut acc: u16 = 0;
    while a != 0 {
        if a & 1 != 0 {
            acc ^= b;
        }
        a >>= 1;
        // b *= x, reducing mod poly when it overflows degree 15.
        let carry = b & 0x8000;
        b <<= 1;
        if carry != 0 {
            b ^= poly;
        }
    }
    acc
}

/// `a^n mod poly`, by square-and-multiply over GF(2)[x].
fn pow_poly(a: u16, mut n: usize, poly: u16) -> u16 {
    let mut r: u16 = 1;
    let mut base = a;
    while n != 0 {
        if n & 1 != 0 {
            r = mul_poly(r, base, poly);
        }
        base = mul_poly(base, base, poly);
        n >>= 1;
    }
    r
}

/// The multiplier that moves a residue back across `region_len` bytes.
///
/// `crc1` sits at the *front* of the region it protects, so the encoder needs
/// the value which, after being shifted through the remaining
/// `8*region_len - 16` bit positions, cancels the region's residue. That
/// multiplier is `(x^-1)^(8*region_len - 16)`.
///
/// It depends only on the region length, so an encoder emitting many frames of
/// one size computes it once at construction rather than per frame.
pub(crate) fn ac3_crc_prefix_multiplier(region_len: usize) -> u16 {
    debug_assert!(region_len >= 2);
    pow_poly(X_INVERSE, 8 * region_len - 16, AC3_CRC_POLY)
}

/// `x^-1` modulo the generator, in the same implicit-`x^16` representation as
/// [`AC3_CRC_POLY`].
///
/// From `x^16 + x^15 + x^2 + 1 = 0` we get `1 = x·(x^15 + x^14 + x)`, so
/// `x^-1 = x^15 + x^14 + x = 0xC002`. Equivalently it is the *full* 17-bit
/// generator shifted right by one — which is why libavcodec writes it as
/// `CRC16_POLY >> 1`, its `CRC16_POLY` being 0x18005 where ours drops the
/// implicit top bit. Dropping that bit here would give 0x4002 and a silently
/// wrong `crc1`.
const X_INVERSE: u16 = (AC3_CRC_POLY >> 1) | 0x8000;

/// Find a 16-bit value for the *first* 2 bytes of `region` such that
/// the running CRC of the entire region ends at zero. Used by the
/// encoder for `crc1`, where the CRC field sits at the *start* of
/// the covered area and must therefore be solved for rather than
/// derived from a trailing residue (§7.10.1 last paragraph: "crc1
/// is generated by encoders such that the CRC calculation will
/// produce zero at the 5/8 point in the syncframe").
///
/// Linear-algebra approach: the CRC is linear over GF(2), so we
/// build a 16×16 matrix whose columns are the CRC contributions of
/// each basis bit set in the first two bytes (vs. an all-zero
/// prefix), compute the residue of the region with the prefix
/// zeroed, then Gaussian-eliminate for the prefix bits that cancel
/// the residue.
///
/// The region must be at least 2 bytes (the 16-bit CRC field).
/// Convenience wrapper that derives the multiplier from `region`. Only tests
/// use it — the encoder holds a multiplier computed once at construction, and
/// deriving one per frame would give back part of what this replaced.
#[cfg(test)]
pub(crate) fn ac3_crc_solve_prefix(region: &[u8]) -> u16 {
    ac3_crc_solve_prefix_with(region, ac3_crc_prefix_multiplier(region.len()))
}

/// [`ac3_crc_solve_prefix`] with the length-dependent multiplier supplied.
///
/// The encoder emits thousands of frames of one fixed size, so it computes
/// the multiplier once at construction and calls this.
///
/// Closed form rather than a linear solve. The CRC is linear over GF(2), so
/// with the prefix zeroed the region has some residue `R`, and a prefix value
/// `X` contributes `X · x^(8·len - 16)` on top of it. Setting the total to
/// zero gives `X = R · (x^-1)^(8·len - 16)` — one multiply, where the
/// Gaussian elimination this replaced cost seventeen passes over the whole
/// region (one for `R`, sixteen to build the basis) plus sixteen heap
/// allocations, per frame. This is what libavcodec's `ac3_encode_frame` does.
pub(crate) fn ac3_crc_solve_prefix_with(region: &[u8], multiplier: u16) -> u16 {
    assert!(region.len() >= 2);
    // R = CRC(zeroed-prefix || rest-of-region), without materialising the
    // zeroed copy: feed two zero bytes, then the region's tail.
    let r = ac3_crc_update(ac3_crc_update(0, &[0, 0]), &region[2..]);
    mul_poly(r, multiplier, AC3_CRC_POLY)
}

/// Solve `A · x = b` over GF(2) where `A` is a 16×16 matrix
/// represented as 16 column vectors (each a `u16` with bit `j` =
/// row `j`), and `b` and `x` are 16-bit vectors. The matrix is
/// expected to be invertible (it is, for AC-3 CRC-16 over any
/// region ≥ 2 bytes).
///
/// Post-solve, the column index `i` in `x` maps to prefix bit
/// `15 - i` (MSB-first), because `ac3_crc_solve_prefix` builds
/// column `i` from `prefix_val = 1 << (15 - i)`. The returned
/// `u16` is the reassembled prefix word.
///
/// This was how `crc1` was solved for until the closed form in
/// [`ac3_crc_solve_prefix_with`] replaced it. Retained as the test oracle
/// that the closed form is checked against, via `gauss_solve_prefix`.
#[cfg(test)]
fn gauss_gf2_16(cols: &[u16; 16], b: u16) -> u16 {
    // Build an augmented matrix as rows: each row is 17 bits (16 cols + 1 b).
    let mut rows = [0u32; 16];
    for row in 0..16 {
        let bit = 1u16 << row;
        let mut r = 0u32;
        for c in 0..16 {
            if cols[c] & bit != 0 {
                r |= 1 << c;
            }
        }
        if b & bit != 0 {
            r |= 1 << 16;
        }
        rows[row] = r;
    }
    // Forward elimination.
    for col in 0..16 {
        let mut pivot = None;
        for r in col..16 {
            if rows[r] & (1 << col) != 0 {
                pivot = Some(r);
                break;
            }
        }
        let pivot = match pivot {
            Some(p) => p,
            None => continue, // singular column, leave as-is
        };
        rows.swap(col, pivot);
        for r in 0..16 {
            if r != col && rows[r] & (1 << col) != 0 {
                rows[r] ^= rows[col];
            }
        }
    }
    // Read x from the augment column (LSB-first across columns).
    let mut x = 0u16;
    for r in 0..16 {
        if rows[r] & (1 << 16) != 0 {
            x |= 1 << r;
        }
    }
    // Re-order from column-index space to prefix-bit space (MSB-first).
    let mut prefix = 0u16;
    for i in 0..16 {
        if x & (1 << i) != 0 {
            prefix |= 1 << (15 - i);
        }
    }
    prefix
}

/// Compute the 5/8-frame boundary used by `crc1` (§7.10.1).
///
/// The spec writes the calculation in 16-bit-word units:
///
/// ```text
///   5/8_framesize = (framesize >> 1) + (framesize >> 3)
/// ```
///
/// where `framesize` is in words. Returns the byte offset such that
/// `syncframe[2..byte_offset]` is the region covered by `crc1`.
///
/// Returns `None` if `frame_bytes` is too small to hold a 5-byte
/// syncinfo + the implied 5/8 region (i.e. `frame_bytes < 4`); valid
/// AC-3 frames per Table 5.18 are at least 128 bytes so this guard
/// only fires on truncated input.
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub fn crc1_boundary_bytes(frame_bytes: usize) -> Option<usize> {
    if frame_bytes < 4 {
        return None;
    }
    let frame_words = frame_bytes / 2;
    let five_eighths_words = (frame_words >> 1) + (frame_words >> 3);
    Some(five_eighths_words * 2)
}

/// Per-frame CRC validation outcome.
///
/// `crc1_ok` and `crc2_ok` are reported independently so a caller
/// can implement either of the §6.1.2 / §7.10.1 strategies:
/// "accept on either CRC valid", "require both", or "drop frames
/// failing crc2". `None` means the field was not checked because
/// it doesn't apply to this syncframe (E-AC-3 has no `crc1`, so a
/// `verify_eac3_syncframe` always reports `crc1_ok = None`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CrcStatus {
    /// `Some(true)` if the LFSR residue over the first 5/8 of the
    /// syncframe (post-syncword) is zero, `Some(false)` if non-zero.
    /// `None` when not checked (E-AC-3).
    pub crc1_ok: Option<bool>,
    /// `Some(true)` if the LFSR residue over the whole syncframe
    /// (post-syncword) is zero. `Some(false)` if non-zero.
    pub crc2_ok: Option<bool>,
}

impl CrcStatus {
    /// True when every checked field passed. For an AC-3 syncframe
    /// this means `crc1_ok == Some(true) && crc2_ok == Some(true)`;
    /// for E-AC-3, just `crc2_ok == Some(true)`. An unchecked field
    /// (`None`) is treated as a pass, matching the §6.1.2 "accept on
    /// either CRC" leniency.
    pub fn all_ok(&self) -> bool {
        let c1 = self.crc1_ok.unwrap_or(true);
        let c2 = self.crc2_ok.unwrap_or(true);
        c1 && c2
    }
}

/// Verify both CRC words in an AC-3 syncframe per §7.10.1.
///
/// `syncframe` must start with the 0x0B77 syncword and span exactly
/// `frame_bytes` bytes (the full §5.4.1.4 Table 5.18 syncframe).
///
/// Both checks are **residue-form**: the LFSR is reset to zero, the
/// data bits are shifted through (with the stored CRC fields
/// included), and the register must read zero at the end.
///
/// * `crc1` covers `syncframe[2..crc1_end]` — the first 5/8 of the
///   post-syncword bytes, including the crc1 field itself.
/// * `crc2` covers `syncframe[2..frame_bytes]` — the entire post-
///   syncword region, including both crc fields.
///
/// Returns a [`CrcStatus`] populated with the two checks. A
/// truncated `syncframe` (shorter than `frame_bytes`, or shorter
/// than 4 bytes) reports both as failed.
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub fn verify_ac3_syncframe(syncframe: &[u8], frame_bytes: usize) -> CrcStatus {
    if syncframe.len() < frame_bytes || frame_bytes < 4 {
        return CrcStatus {
            crc1_ok: Some(false),
            crc2_ok: Some(false),
        };
    }
    let crc1_end = match crc1_boundary_bytes(frame_bytes) {
        Some(v) if v >= 4 && v <= frame_bytes => v,
        _ => {
            return CrcStatus {
                crc1_ok: Some(false),
                crc2_ok: Some(false),
            }
        }
    };
    let crc1_residue = ac3_crc_update(0, &syncframe[2..crc1_end]);
    let crc2_residue = ac3_crc_update(0, &syncframe[2..frame_bytes]);
    CrcStatus {
        crc1_ok: Some(crc1_residue == 0),
        crc2_ok: Some(crc2_residue == 0),
    }
}

/// Verify the `crc2` word in an E-AC-3 syncframe per §E.1.2 /
/// §7.10.1.
///
/// E-AC-3 has no `crc1`; the field is omitted from the syncframe
/// to make room for the variable-bitrate `frmsiz` word. `crc2` is
/// still the last 16-bit word and still computed with the same
/// poly 0x8005 LFSR over the post-syncword bytes (residue-form per
/// §7.10.1) — the verifier shifts every byte of the post-syncword
/// region (including the trailing crc2 field) through the LFSR and
/// expects the register to read zero.
///
/// `syncframe` must start with the 0x0B77 syncword and span at
/// least `frame_bytes`. The reported `crc1_ok` is `None` because
/// E-AC-3 carries no `crc1` field.
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub fn verify_eac3_syncframe(syncframe: &[u8], frame_bytes: usize) -> CrcStatus {
    if syncframe.len() < frame_bytes || frame_bytes < 4 {
        return CrcStatus {
            crc1_ok: None,
            crc2_ok: Some(false),
        };
    }
    let crc2_residue = ac3_crc_update(0, &syncframe[2..frame_bytes]);
    CrcStatus {
        crc1_ok: None,
        crc2_ok: Some(crc2_residue == 0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Spec sanity: a single zero byte shifted through the cleared
    /// LFSR leaves the register at zero (no `1` bits → no XOR).
    #[test]
    fn zero_data_yields_zero_register() {
        assert_eq!(ac3_crc_update(0, &[0u8; 8]), 0);
    }

    /// Spec sanity: the LFSR is non-trivial — a single high bit in
    /// the leading byte produces a non-zero register at end-of-byte.
    #[test]
    fn single_high_bit_propagates() {
        assert_ne!(ac3_crc_update(0, &[0x80, 0x00]), 0);
    }

    /// Algebraic property: CRC is linear over GF(2). For any two
    /// equal-length byte slices `a`, `b`, `crc(a XOR b) == crc(a)
    /// XOR crc(b)`. This proves the bit-shifter respects the
    /// `gauss_gf2_16` solver's assumption.
    #[test]
    fn crc_is_gf2_linear() {
        let a = [0x12u8, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0];
        let b = [0xa5u8, 0x5a, 0x33, 0xcc, 0x0f, 0xf0, 0x99, 0x66];
        let mut x = [0u8; 8];
        for i in 0..8 {
            x[i] = a[i] ^ b[i];
        }
        let ca = ac3_crc_update(0, &a);
        let cb = ac3_crc_update(0, &b);
        let cx = ac3_crc_update(0, &x);
        assert_eq!(ca ^ cb, cx);
    }

    /// Solver round-trip: place the solved 16-bit value into the
    /// first 2 bytes of a region and the running CRC over the full
    /// region must be zero. Mirrors the encoder's `crc1` debug
    /// assertion (encoder.rs).
    #[test]
    fn solver_drives_residue_to_zero() {
        let mut region = vec![0u8; 80];
        // Fill the tail with arbitrary content so the residue with a
        // zero prefix is non-zero.
        for (i, byte) in region.iter_mut().enumerate().skip(2) {
            *byte = ((i * 17 + 3) & 0xFF) as u8;
        }
        let x = ac3_crc_solve_prefix(&region);
        region[0] = (x >> 8) as u8;
        region[1] = (x & 0xFF) as u8;
        assert_eq!(ac3_crc_update(0, &region), 0);
    }

    /// §7.10.1 example boundary calculation. For a 768-byte
    /// (= 384-word) syncframe at 48 kHz / 192 kbps (frmsizecod=20),
    /// 5/8_framesize = (384>>1) + (384>>3) = 192 + 48 = 240 words
    /// = 480 bytes (Table 7.34).
    #[test]
    fn crc1_boundary_matches_table_7_34_48k_192kbps() {
        assert_eq!(crc1_boundary_bytes(768), Some(480));
    }

    /// §7.10.1 example boundary calculation. 256-byte (= 128-word)
    /// syncframe → (128>>1) + (128>>3) = 64 + 16 = 80 words = 160 B.
    #[test]
    fn crc1_boundary_minimal_frame() {
        assert_eq!(crc1_boundary_bytes(256), Some(160));
    }

    /// Truncated-frame guard: shorter than the 4-byte minimum (the
    /// syncword + the first byte of `crc1`) yields `None`.
    #[test]
    fn crc1_boundary_rejects_truncated() {
        assert_eq!(crc1_boundary_bytes(0), None);
        assert_eq!(crc1_boundary_bytes(3), None);
        assert_eq!(crc1_boundary_bytes(4), Some(2)); // boundary land at field start
    }

    /// CrcStatus::all_ok semantics — None fields are treated as
    /// pass, matching the spec's "accept on either CRC valid"
    /// leniency.
    #[test]
    fn crc_status_all_ok_treats_none_as_pass() {
        let s = CrcStatus {
            crc1_ok: None,
            crc2_ok: Some(true),
        };
        assert!(s.all_ok());
        let s = CrcStatus {
            crc1_ok: Some(true),
            crc2_ok: Some(true),
        };
        assert!(s.all_ok());
        let s = CrcStatus {
            crc1_ok: Some(false),
            crc2_ok: Some(true),
        };
        assert!(!s.all_ok());
        let s = CrcStatus {
            crc1_ok: Some(true),
            crc2_ok: Some(false),
        };
        assert!(!s.all_ok());
    }

    /// Truncated AC-3 syncframe reports both checks as failed
    /// rather than panicking on the slice index.
    #[test]
    fn verify_ac3_truncated_buffer_reports_failure() {
        let buf = vec![0x0B, 0x77, 0x00, 0x00];
        let s = verify_ac3_syncframe(&buf, 768);
        assert_eq!(s.crc1_ok, Some(false));
        assert_eq!(s.crc2_ok, Some(false));
        assert!(!s.all_ok());
    }

    /// Truncated E-AC-3 syncframe reports `crc2_ok = Some(false)`
    /// and `crc1_ok = None` (no field exists).
    #[test]
    fn verify_eac3_truncated_buffer_reports_failure() {
        let buf = vec![0x0B, 0x77];
        let s = verify_eac3_syncframe(&buf, 768);
        assert_eq!(s.crc1_ok, None);
        assert_eq!(s.crc2_ok, Some(false));
    }

    /// Synthetic CRC-clean frame: a 256-byte buffer where the
    /// crc1 prefix has been solved so the LFSR is zero at the
    /// 5/8 point, and the trailing 2 bytes are filled with the
    /// *augmented* CRC (`ac3_crc_update(0, data || [0, 0])`) so
    /// the LFSR residue is zero at the end of the whole frame
    /// too. This mirrors what a §7.10.1-compliant encoder
    /// writes for `crc2`.
    #[test]
    fn synthetic_frame_passes_both_checks() {
        let frame_bytes = 256usize;
        let mut frame = vec![0u8; frame_bytes];
        frame[0] = 0x0B;
        frame[1] = 0x77;
        // Pad the body with non-trivial content so neither residue
        // is trivially zero from the data.
        for i in 5..(frame_bytes - 2) {
            frame[i] = ((i * 31 + 7) & 0xFF) as u8;
        }
        // Solve crc1 over bytes 2..crc1_end so the running CRC is
        // zero at the 5/8 point.
        let crc1_end = crc1_boundary_bytes(frame_bytes).unwrap();
        let x = ac3_crc_solve_prefix(&frame[2..crc1_end]);
        frame[2] = (x >> 8) as u8;
        frame[3] = (x & 0xFF) as u8;
        debug_assert_eq!(ac3_crc_update(0, &frame[2..crc1_end]), 0);
        // crc2 = augmented CRC of the post-syncword region minus
        // the trailing 2 bytes. Appending two zero bytes to the
        // payload flushes the register through 16 more shifts;
        // the resulting state equals `payload·x^16 mod g(x)`. When
        // that value is then written back into the trailing 2
        // bytes, shifting it through transitions the register to
        // zero (`(payload·x^16 + crc2) mod g = 0`).
        let mut padded = Vec::with_capacity(frame_bytes - 2 + 2);
        padded.extend_from_slice(&frame[2..(frame_bytes - 2)]);
        padded.extend_from_slice(&[0, 0]);
        let crc2_val = ac3_crc_update(0, &padded);
        frame[frame_bytes - 2] = (crc2_val >> 8) as u8;
        frame[frame_bytes - 1] = (crc2_val & 0xFF) as u8;

        let status = verify_ac3_syncframe(&frame, frame_bytes);
        assert_eq!(status.crc1_ok, Some(true), "crc1 residue should be zero");
        assert_eq!(status.crc2_ok, Some(true), "crc2 residue should be zero");
        assert!(status.all_ok());
    }

    /// Flipping any single bit in the post-syncword region of a
    /// CRC-clean frame breaks at least one of the two checks. This
    /// is the central error-detection property of §7.10.1 ("CRC
    /// check is reliable to 0.0015 percent").
    #[test]
    fn single_bit_flip_breaks_verification() {
        let frame_bytes = 256usize;
        let mut frame = vec![0u8; frame_bytes];
        frame[0] = 0x0B;
        frame[1] = 0x77;
        for i in 5..(frame_bytes - 2) {
            frame[i] = ((i * 31 + 7) & 0xFF) as u8;
        }
        let crc1_end = crc1_boundary_bytes(frame_bytes).unwrap();
        let x = ac3_crc_solve_prefix(&frame[2..crc1_end]);
        frame[2] = (x >> 8) as u8;
        frame[3] = (x & 0xFF) as u8;
        // Augmented-form crc2 (see `synthetic_frame_passes_both_checks`).
        let mut padded = Vec::with_capacity(frame_bytes - 2 + 2);
        padded.extend_from_slice(&frame[2..(frame_bytes - 2)]);
        padded.extend_from_slice(&[0, 0]);
        let crc2_val = ac3_crc_update(0, &padded);
        frame[frame_bytes - 2] = (crc2_val >> 8) as u8;
        frame[frame_bytes - 1] = (crc2_val & 0xFF) as u8;
        // Baseline: clean frame validates.
        assert!(verify_ac3_syncframe(&frame, frame_bytes).all_ok());
        // Flip a single bit deep in the body. Either crc1 (if the
        // flip is in the 5/8 region) or crc2 (if past it) must fail
        // — and a flip in the 5/8 region also fails crc2 because the
        // running register can't recover at the 5/8 boundary.
        let pos = 17;
        frame[pos] ^= 0x01;
        let status = verify_ac3_syncframe(&frame, frame_bytes);
        assert!(
            !status.all_ok(),
            "single-bit flip should fail at least one CRC"
        );
    }

    /// E-AC-3 path: a frame with `crc2` set to the augmented CRC
    /// (`ac3_crc_update(0, payload || [0, 0])`) verifies cleanly
    /// as a residue check.
    #[test]
    fn eac3_synthetic_frame_passes_crc2() {
        let frame_bytes = 384usize;
        let mut frame = vec![0u8; frame_bytes];
        frame[0] = 0x0B;
        frame[1] = 0x77;
        for i in 2..(frame_bytes - 2) {
            frame[i] = ((i * 23 + 11) & 0xFF) as u8;
        }
        let mut padded = Vec::with_capacity(frame_bytes - 2 + 2);
        padded.extend_from_slice(&frame[2..(frame_bytes - 2)]);
        padded.extend_from_slice(&[0, 0]);
        let crc2_val = ac3_crc_update(0, &padded);
        frame[frame_bytes - 2] = (crc2_val >> 8) as u8;
        frame[frame_bytes - 1] = (crc2_val & 0xFF) as u8;
        let status = verify_eac3_syncframe(&frame, frame_bytes);
        assert_eq!(status.crc1_ok, None);
        assert_eq!(status.crc2_ok, Some(true));
        assert!(status.all_ok());
    }

    /// E-AC-3 flip: corrupting the body breaks the crc2 residue.
    #[test]
    fn eac3_bit_flip_breaks_crc2() {
        let frame_bytes = 384usize;
        let mut frame = vec![0u8; frame_bytes];
        frame[0] = 0x0B;
        frame[1] = 0x77;
        for i in 2..(frame_bytes - 2) {
            frame[i] = ((i * 23 + 11) & 0xFF) as u8;
        }
        let mut padded = Vec::with_capacity(frame_bytes - 2 + 2);
        padded.extend_from_slice(&frame[2..(frame_bytes - 2)]);
        padded.extend_from_slice(&[0, 0]);
        let crc2_val = ac3_crc_update(0, &padded);
        frame[frame_bytes - 2] = (crc2_val >> 8) as u8;
        frame[frame_bytes - 1] = (crc2_val & 0xFF) as u8;
        assert!(verify_eac3_syncframe(&frame, frame_bytes).all_ok());
        frame[100] ^= 0x10;
        assert!(!verify_eac3_syncframe(&frame, frame_bytes).all_ok());
    }

    /// The pre-optimisation `crc1` solver: build the 16 basis columns by
    /// running a full CRC per perturbed prefix, then Gaussian-eliminate.
    fn gauss_solve_prefix(region: &[u8]) -> u16 {
        let mut zeroed = region.to_vec();
        zeroed[0] = 0;
        zeroed[1] = 0;
        let r = ac3_crc_update(0, &zeroed);
        let mut cols = [0u16; 16];
        for i in 0..16 {
            let prefix_val: u16 = 1 << (15 - i);
            let mut buf = vec![0u8; region.len()];
            buf[0] = (prefix_val >> 8) as u8;
            buf[1] = (prefix_val & 0xFF) as u8;
            cols[i] = ac3_crc_update(0, &buf);
        }
        gauss_gf2_16(&cols, r)
    }

    fn pseudorandom(len: usize, seed: u64) -> Vec<u8> {
        let mut x = seed | 1;
        (0..len)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                (x >> 24) as u8
            })
            .collect()
    }

    #[test]
    fn table_crc_matches_the_bitwise_lfsr() {
        for seed in 1..=8u64 {
            for len in [2usize, 3, 7, 64, 255, 800, 1598, 2558] {
                let data = pseudorandom(len, seed * 0x9E37_79B9);
                assert_eq!(
                    ac3_crc_update(0, &data),
                    ac3_crc_update_bitwise(0, &data),
                    "len={len} seed={seed}"
                );
                // Chained updates must agree with a single pass, which is what
                // the encoder relies on when it resumes at the 5/8 boundary.
                let (head, tail) = data.split_at(len / 2);
                assert_eq!(
                    ac3_crc_update(ac3_crc_update(0, head), tail),
                    ac3_crc_update_bitwise(0, &data),
                    "chained len={len}"
                );
            }
        }
    }

    #[test]
    fn closed_form_crc1_matches_the_gaussian_solver() {
        // Frame sizes that occur in practice: the 5/8 regions of 128..3840-byte
        // syncframes, plus the degenerate minimum.
        for frame_bytes in [128usize, 384, 640, 1280, 1792, 2560, 3840] {
            let boundary = crc1_boundary_bytes(frame_bytes).unwrap();
            let region_len = boundary - 2;
            for seed in 1..=4u64 {
                let mut region = pseudorandom(region_len, seed * 0x2545_F491);
                let closed = ac3_crc_solve_prefix(&region);
                assert_eq!(
                    closed,
                    gauss_solve_prefix(&region),
                    "frame_bytes={frame_bytes} seed={seed}"
                );
                // And the property both are solving for: with the solved value
                // in place, the region's residue is zero.
                region[0] = (closed >> 8) as u8;
                region[1] = (closed & 0xFF) as u8;
                assert_eq!(ac3_crc_update(0, &region), 0, "residue not cancelled");
            }
        }
    }

    #[test]
    fn prefix_multiplier_is_reusable_across_frames_of_one_size() {
        let region_len = crc1_boundary_bytes(2560).unwrap() - 2;
        let mult = ac3_crc_prefix_multiplier(region_len);
        for seed in 1..=6u64 {
            let region = pseudorandom(region_len, seed * 0x85EB_CA6B);
            assert_eq!(
                ac3_crc_solve_prefix_with(&region, mult),
                ac3_crc_solve_prefix(&region)
            );
        }
    }

    #[test]
    fn mul_poly_and_pow_poly_agree_with_repeated_multiplication() {
        let a = X_INVERSE;
        let mut acc: u16 = 1;
        for n in 0..40usize {
            assert_eq!(pow_poly(a, n, AC3_CRC_POLY), acc, "n={n}");
            acc = mul_poly(acc, a, AC3_CRC_POLY);
        }
        // x · x^-1 = 1, which is the identity the crc1 derivation rests on.
        // Note `AC3_CRC_POLY >> 1` is *not* x^-1 here — it drops the implicit
        // x^16 term — and using it would make crc1 silently wrong.
        assert_eq!(mul_poly(2, X_INVERSE, AC3_CRC_POLY), 1);
        assert_ne!(mul_poly(2, AC3_CRC_POLY >> 1, AC3_CRC_POLY), 1);
    }
}
