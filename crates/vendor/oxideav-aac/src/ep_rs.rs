//! §1.8.4.7 shortened Reed-Solomon codes of the MPEG-4
//! error-protection tool.
//!
//! `SRS(255−l, 255−2k−l)` over GF(2⁸) built on the primitive
//! polynomial `m(x) = x⁸ + x⁴ + x³ + x² + 1` (the Table 1.62 α-power
//! listing is exactly the antilog table of that polynomial — pinned
//! by tests against printed rows). The generator is
//! `g(x) = (x−α)(x−α²)…(x−α^2k)`; a class longer than `255−2k` octets
//! splits into parts (`l_i = 255−2k` except the zero-padded last),
//! each part's parity is `p(x) = x^2k·u(x) mod g(x)` with the
//! **lowest-order coefficient as the first octet** (§1.8.4.7), and
//! all parities are appended after the class data (Figure 1.11).
//!
//! Decoding runs the standard algebraic chain over the spec's field:
//! syndromes `S_j = r(α^j)`, Berlekamp-Massey for the error locator,
//! Chien search, Forney evaluation — correcting up to `k` byte errors
//! per part; an uncorrectable part surfaces
//! [`Error::EpFrameInvalid`].

use crate::{Error, Result};

/// GF(2⁸) tables for `m(x) = x⁸ + x⁴ + x³ + x² + 1` (0x11D).
struct Gf {
    exp: [u8; 512],
    log: [u8; 256],
}

fn gf() -> &'static Gf {
    use std::sync::OnceLock;
    static GF: OnceLock<Gf> = OnceLock::new();
    GF.get_or_init(|| {
        let mut exp = [0u8; 512];
        let mut log = [0u8; 256];
        let mut v: u16 = 1;
        #[allow(clippy::needless_range_loop)]
        for i in 0..255 {
            exp[i] = v as u8;
            log[v as usize] = i as u8;
            v <<= 1;
            if v & 0x100 != 0 {
                v ^= 0x11D;
            }
        }
        for i in 255..512 {
            exp[i] = exp[i - 255];
        }
        Gf { exp, log }
    })
}

#[inline]
fn gf_mul(a: u8, b: u8) -> u8 {
    if a == 0 || b == 0 {
        return 0;
    }
    let g = gf();
    g.exp[usize::from(g.log[usize::from(a)]) + usize::from(g.log[usize::from(b)])]
}

#[inline]
fn gf_inv(a: u8) -> Result<u8> {
    if a == 0 {
        return Err(Error::EpFrameInvalid);
    }
    let g = gf();
    Ok(g.exp[255 - usize::from(g.log[usize::from(a)])])
}

/// α^i (`0 <= i`), the Table 1.62 antilog.
pub fn alpha_pow(i: usize) -> u8 {
    gf().exp[i % 255]
}

/// The §1.8.4.7 generator polynomial `g(x) = ∏_{i=1..2k} (x − α^i)`,
/// lowest-order coefficient first, length `2k + 1` (monic).
fn generator(two_k: usize) -> Vec<u8> {
    let mut g = vec![0u8; two_k + 1];
    g[0] = 1;
    let mut deg = 0usize;
    for i in 1..=two_k {
        let a = alpha_pow(i);
        // g = g * (x + α^i)  (− == + in GF(2^8)).
        deg += 1;
        for j in (1..=deg).rev() {
            g[j] = g[j - 1] ^ gf_mul(g[j], a);
        }
        g[0] = gf_mul(g[0], a);
    }
    g
}

/// Parity octets (`2k`, lowest order first) for one part `u` of at
/// most `255 − 2k` octets: `p(x) = x^2k · u(x) mod g(x)` with the
/// first octet of `u` as the lowest-order coefficient (§1.8.4.7).
fn part_parity(part: &[u8], two_k: usize) -> Vec<u8> {
    let g = generator(two_k);
    // Work highest-order-first for the long division: u(x)·x^2k has
    // coefficients [0; 2k] ++ part (lowest first). Highest order is
    // the LAST octet of `part`.
    let mut rem = vec![0u8; two_k]; // remainder, highest order at [0]
    for &coeff in part.iter().rev() {
        let factor = rem[0] ^ coeff;
        // Shift left by one (multiply by x) and subtract factor·g.
        for i in 0..two_k {
            let next = if i + 1 < two_k { rem[i + 1] } else { 0 };
            rem[i] = next ^ gf_mul(factor, g[two_k - 1 - i]);
        }
    }
    // rem[0] is the highest-order remainder coefficient; the wire
    // wants lowest order first.
    rem.reverse();
    rem
}

/// §1.8.4.7 part split of a class of `len` octets under `2k` parity
/// octets per part: every part is `255 − 2k` long except the last
/// (`len mod (255 − 2k)`, zero-padded for the computation).
fn part_lengths(len: usize, two_k: usize) -> Result<Vec<usize>> {
    let cap = 255 - two_k;
    if cap == 0 || len == 0 {
        return Err(Error::EpFrameInvalid);
    }
    let n = len.div_ceil(cap);
    let mut parts = Vec::with_capacity(n);
    for i in 0..n {
        if i + 1 < n {
            parts.push(cap);
        } else {
            let last = len - cap * (n - 1);
            parts.push(last);
        }
    }
    Ok(parts)
}

/// SRS-encode a class: returns the parity octets to append after the
/// class data (all parts' parities in part order, Figure 1.11).
///
/// `k` is the per-codeword correction capability (`class_rate` for
/// `fec_type == 1 / 2`); `k == 0` yields no parity.
pub fn srs_encode(class_data: &[u8], k: usize) -> Result<Vec<u8>> {
    if k == 0 {
        return Ok(Vec::new());
    }
    let two_k = 2 * k;
    if two_k >= 255 {
        return Err(Error::EpConfigInvalid);
    }
    let parts = part_lengths(class_data.len(), two_k)?;
    let cap = 255 - two_k;
    let mut out = Vec::with_capacity(two_k * parts.len());
    let mut pos = 0usize;
    for (i, &plen) in parts.iter().enumerate() {
        let mut part = class_data[pos..pos + plen].to_vec();
        pos += plen;
        if i + 1 == parts.len() && plen < cap {
            // §1.8.4.7: zero-pad the short last part for the
            // computation only.
            part.resize(cap, 0);
        }
        out.extend_from_slice(&part_parity(&part, two_k));
    }
    Ok(out)
}

/// SRS-decode a class in place: `class_data` are the received data
/// octets, `parity` the received parity octets ([`srs_encode`]
/// layout). Corrects up to `k` byte errors per part (errors in the
/// parity octets included); an uncorrectable part is
/// [`Error::EpFrameInvalid`].
pub fn srs_decode(class_data: &mut [u8], parity: &[u8], k: usize) -> Result<()> {
    if k == 0 {
        return Ok(());
    }
    let two_k = 2 * k;
    if two_k >= 255 {
        return Err(Error::EpConfigInvalid);
    }
    let parts = part_lengths(class_data.len(), two_k)?;
    if parity.len() != two_k * parts.len() {
        return Err(Error::EpFrameInvalid);
    }
    let cap = 255 - two_k;
    let mut pos = 0usize;
    for (i, &plen) in parts.iter().enumerate() {
        // Codeword c(x): parity (lowest orders 0..2k) then data
        // (orders 2k..). Build lowest-order-first.
        let mut cw = vec![0u8; 255];
        cw[..two_k].copy_from_slice(&parity[i * two_k..(i + 1) * two_k]);
        let part = &class_data[pos..pos + plen];
        for (j, &b) in part.iter().enumerate() {
            cw[two_k + j] = b;
        }
        // (zero padding of a short last part occupies the top orders
        // implicitly.)
        let corrected = rs_correct(&mut cw, k)?;
        let _ = corrected;
        // Verify the padding stayed zero (errors located there would
        // mean a miscorrection for a conforming stream).
        for j in plen..cap {
            if cw[two_k + j] != 0 {
                return Err(Error::EpFrameInvalid);
            }
        }
        class_data[pos..pos + plen].copy_from_slice(&cw[two_k..two_k + plen]);
        pos += plen;
    }
    Ok(())
}

/// Correct one 255-octet codeword (lowest-order coefficient first) in
/// place; returns the number of corrected byte errors.
fn rs_correct(cw: &mut [u8], k: usize) -> Result<usize> {
    let two_k = 2 * k;
    // Syndromes S_j = c(α^j), j = 1..=2k.
    let mut synd = vec![0u8; two_k];
    let mut any = false;
    for (j, s) in synd.iter_mut().enumerate() {
        let a = alpha_pow(j + 1);
        let mut acc = 0u8;
        // Horner from the highest order down.
        for &c in cw.iter().rev() {
            acc = gf_mul(acc, a) ^ c;
        }
        *s = acc;
        any |= acc != 0;
    }
    if !any {
        return Ok(0);
    }

    // Berlekamp-Massey for the error locator Λ(x) (lowest order
    // first, Λ(0) = 1).
    let mut lambda = vec![0u8; two_k + 1];
    let mut prev = vec![0u8; two_k + 1];
    lambda[0] = 1;
    prev[0] = 1;
    let mut l = 0usize;
    let mut m = 1usize;
    let mut b = 1u8;
    for n in 0..two_k {
        // Discrepancy.
        let mut delta = synd[n];
        for i in 1..=l {
            delta ^= gf_mul(lambda[i], synd[n - i]);
        }
        if delta == 0 {
            m += 1;
        } else if 2 * l <= n {
            let t = lambda.clone();
            let coef = gf_mul(delta, gf_inv(b)?);
            for i in 0..=two_k {
                if i >= m && prev[i - m] != 0 {
                    lambda[i] ^= gf_mul(coef, prev[i - m]);
                }
            }
            prev = t;
            l = n + 1 - l;
            b = delta;
            m = 1;
        } else {
            let coef = gf_mul(delta, gf_inv(b)?);
            for i in 0..=two_k {
                if i >= m && prev[i - m] != 0 {
                    lambda[i] ^= gf_mul(coef, prev[i - m]);
                }
            }
            m += 1;
        }
    }
    if l > k {
        return Err(Error::EpFrameInvalid);
    }

    // Chien search: error at position p iff Λ(α^{-p}) == 0.
    let mut err_pos = Vec::with_capacity(l);
    for p in 0..255usize {
        let x = alpha_pow((255 - p) % 255); // α^{-p}
        let mut acc = 0u8;
        for i in (0..=l).rev() {
            acc = gf_mul(acc, x) ^ lambda[i];
        }
        if acc == 0 {
            err_pos.push(p);
        }
    }
    if err_pos.len() != l {
        return Err(Error::EpFrameInvalid);
    }

    // Forney: error magnitudes from the evaluator
    // Ω(x) = S(x)·Λ(x) mod x^{2k}.
    let mut omega = vec![0u8; two_k];
    for i in 0..two_k {
        let mut acc = 0u8;
        for j in 0..=i.min(l) {
            if lambda[j] != 0 && i >= j {
                acc ^= gf_mul(lambda[j], synd[i - j]);
            }
        }
        omega[i] = acc;
    }
    // Λ'(x): formal derivative (odd-power terms). Forney with the
    // first syndrome at j = 1: e_p = Ω(X_p⁻¹) / Λ'(X_p⁻¹).
    for &p in &err_pos {
        let x_inv = alpha_pow((255 - p) % 255);
        // Ω(x_inv), Horner highest order down.
        let mut om = 0u8;
        for i in (0..two_k).rev() {
            om = gf_mul(om, x_inv) ^ omega[i];
        }
        // Λ'(x_inv) = Σ_{i odd, i <= l} Λ_i · x_inv^{i−1}.
        let mut dl = 0u8;
        for i in (1..=l).step_by(2) {
            dl ^= gf_mul(lambda[i], gf_pow(x_inv, i - 1));
        }
        if dl == 0 {
            return Err(Error::EpFrameInvalid);
        }
        let magnitude = gf_mul(om, gf_inv(dl)?);
        cw[p] ^= magnitude;
    }

    // Re-verify.
    for j in 1..=two_k {
        let a = alpha_pow(j);
        let mut acc = 0u8;
        for &c in cw.iter().rev() {
            acc = gf_mul(acc, a) ^ c;
        }
        if acc != 0 {
            return Err(Error::EpFrameInvalid);
        }
    }
    Ok(l)
}

/// `x^i` in GF(2⁸).
fn gf_pow(x: u8, i: usize) -> u8 {
    let mut acc = 1u8;
    for _ in 0..i {
        acc = gf_mul(acc, x);
    }
    acc
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Spot-check the generated antilog table against printed rows of
    /// Table 1.62.
    #[test]
    fn alpha_table_matches_table_1_62() {
        assert_eq!(alpha_pow(0), 0b0000_0001);
        assert_eq!(alpha_pow(1), 0b0000_0010);
        assert_eq!(alpha_pow(8), 0b0001_1101);
        assert_eq!(alpha_pow(63), 0b1010_0001);
        assert_eq!(alpha_pow(64), 0b0101_1111);
        assert_eq!(alpha_pow(127), 0b1100_1100);
        assert_eq!(alpha_pow(128), 0b1000_0101);
        assert_eq!(alpha_pow(175), 0b1111_1111);
        assert_eq!(alpha_pow(191), 0b0100_0001);
        assert_eq!(alpha_pow(254), 0b1000_1110);
    }

    fn prand_bytes(n: usize, mut seed: u32) -> Vec<u8> {
        let mut v = Vec::with_capacity(n);
        for _ in 0..n {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            v.push((seed >> 16) as u8);
        }
        v
    }

    #[test]
    fn srs_roundtrip_clean() {
        for (len, k) in [(10usize, 2usize), (100, 4), (300, 8), (251, 2), (600, 1)] {
            let data = prand_bytes(len, 0xA5A5 ^ len as u32);
            let parity = srs_encode(&data, k).unwrap();
            let n_parts = len.div_ceil(255 - 2 * k);
            assert_eq!(parity.len(), 2 * k * n_parts, "len {len} k {k}");
            let mut rx = data.clone();
            srs_decode(&mut rx, &parity, k).unwrap();
            assert_eq!(rx, data, "len {len} k {k}");
        }
    }

    #[test]
    fn srs_corrects_byte_errors() {
        let data = prand_bytes(120, 0x5EED);
        let k = 4;
        let parity = srs_encode(&data, k).unwrap();
        // Up to k errors in the data part.
        let mut rx = data.clone();
        rx[3] ^= 0x41;
        rx[57] ^= 0xFF;
        rx[100] ^= 0x01;
        rx[119] ^= 0x80;
        srs_decode(&mut rx, &parity, k).unwrap();
        assert_eq!(rx, data);

        // Errors in the parity octets are located and ignored for the
        // data reconstruction.
        let mut rx = data.clone();
        let mut bad_parity = parity.clone();
        bad_parity[0] ^= 0x10;
        bad_parity[5] ^= 0x22;
        srs_decode(&mut rx, &bad_parity, k).unwrap();
        assert_eq!(rx, data);

        // k + 1 errors are uncorrectable.
        let mut rx = data.clone();
        for (i, b) in rx.iter_mut().enumerate().take(k + 1) {
            *b ^= 0x11 + i as u8;
        }
        assert!(srs_decode(&mut rx, &parity, k).is_err());
    }

    #[test]
    fn srs_multi_part_correction() {
        // 300 octets with k = 8 → parts of 239 + 61; errors in both
        // parts correct independently.
        let data = prand_bytes(300, 0x77);
        let k = 8;
        let parity = srs_encode(&data, k).unwrap();
        let mut rx = data.clone();
        for &p in &[0usize, 100, 238, 239, 250, 299] {
            rx[p] ^= 0x5A;
        }
        srs_decode(&mut rx, &parity, k).unwrap();
        assert_eq!(rx, data);
    }
}
