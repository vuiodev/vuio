//! §1.8.4.6 SRCPC convolutional FEC and the §1.8.4.3 in-band-header
//! block codes of the MPEG-4 error-protection tool.
//!
//! ## SRCPC (§1.8.4.6)
//!
//! A systematic recursive convolutional code of rate 1/4 (Figure
//! 1.10) punctured to 8/8..8/32 (Table 1.61). Per input bit `u` with
//! state `(m1, m2, m3, m4)` and feedback `d = m4 ⊕ m2 ⊕ m1`:
//!
//! ```text
//! v1 = u
//! v2 = m3 ⊕ m2 ⊕ m1 ⊕ u
//! v3 = m3 ⊕ m1 ⊕ u
//! v4 = m3 ⊕ m2 ⊕ u
//! next state: (u ⊕ d, m1, m2, m3)
//! ```
//!
//! Puncturing runs with period 8: bit `7 − (t mod 8)` of `Pr(i)`
//! decides whether `v(i+1)` at time `t` is emitted; surviving bits go
//! out in `v1..v4` order per time step. `Pr(0) == 0xFF` for every
//! rate, so the code stays systematic. Termination (§1.8.4.6.2)
//! appends four tail input bits `u = d` driving the state to zero
//! (the Table 1.60 tail-bit listing is the closed form of exactly
//! that rule — pinned by a test).
//!
//! Decoding is hard-decision Viterbi over the 16-state trellis
//! (§1.8.4.6.4); an error-free stream round-trips exactly, and up to
//! the code's correction capability transmission errors are repaired.
//!
//! ## In-band header FEC (§1.8.4.3, Table 1.59)
//!
//! The `choice_of_pred` / `class_attrib()` header parts are protected
//! by a length-selected block code: 3× repetition (1–2 bits),
//! BCH(7,4) (3–4), BCH(15,7) (5–7), Golay(23,12) (8–12), BCH(31,16)
//! (13–16), or — for 17+ bits — CRC4 + terminated SRCPC 8/16. The
//! parity of the polynomial codes is `R(x)` of
//! `M(x)·x^deg(G) = Q(x)G(x) + R(x)` with the §1.8.4.3 generators;
//! decode-side correction is bounded-distance (exhaustive syndrome
//! search up to the code's design correction capability).

use crate::crc::{crc_bits, CrcPoly};
use crate::{Error, Result};

/// Number of tail input bits appended by §1.8.4.6.2 termination.
pub const SRCPC_TAIL_BITS: usize = 4;

/// The nine-step per-output-line puncture progression of Table 1.61
/// (`00, 80, 88, A8, AA, EA, EE, FE, FF`): entry `j` keeps `j` of the
/// eight period positions.
const PUNCTURE_STEPS: [u8; 9] = [0x00, 0x80, 0x88, 0xA8, 0xAA, 0xEA, 0xEE, 0xFE, 0xFF];

/// The Table 1.61 puncture pattern `[Pr(0), Pr(1), Pr(2), Pr(3)]` for
/// `class_rate` 0..=24 (rate 8/8 .. 8/32).
pub fn puncture_pattern(class_rate: u8) -> Result<[u8; 4]> {
    if class_rate > 24 {
        return Err(Error::EpConfigInvalid);
    }
    let extra = usize::from(class_rate);
    Ok([
        0xFF,
        PUNCTURE_STEPS[extra.min(8)],
        PUNCTURE_STEPS[extra.saturating_sub(8).min(8)],
        PUNCTURE_STEPS[extra.saturating_sub(16).min(8)],
    ])
}

/// Number of coded bits the SRCPC emits for `n_info` information bits
/// at `class_rate` (0..=24), with or without the four termination
/// tail steps.
pub fn srcpc_coded_len(n_info: usize, class_rate: u8, terminated: bool) -> Result<usize> {
    let p = puncture_pattern(class_rate)?;
    let steps = n_info + if terminated { SRCPC_TAIL_BITS } else { 0 };
    let per_period: usize = p.iter().map(|&b| b.count_ones() as usize).sum();
    let full = steps / 8;
    let mut len = full * per_period;
    for t in (full * 8)..steps {
        for &line in &p {
            if line & (0x80 >> (t % 8)) != 0 {
                len += 1;
            }
        }
    }
    Ok(len)
}

/// The §1.8.4.6.1 encoder state `(m1, m2, m3, m4)` packed as bits
/// 0..=3 of a nibble.
#[inline]
fn step(state: u8, u: bool) -> (u8, [bool; 4]) {
    let m1 = state & 1 != 0;
    let m2 = state & 2 != 0;
    let m3 = state & 4 != 0;
    let m4 = state & 8 != 0;
    let d = m4 ^ m2 ^ m1;
    let v = [u, m3 ^ m2 ^ m1 ^ u, m3 ^ m1 ^ u, m3 ^ m2 ^ u];
    let next = (u8::from(u ^ d)) | (state << 1) & 0b1110;
    (next, v)
}

/// Feedback bit `d` for a state (drives the §1.8.4.6.2 tail inputs).
#[inline]
fn feedback(state: u8) -> bool {
    let m1 = state & 1 != 0;
    let m2 = state & 2 != 0;
    let m4 = state & 8 != 0;
    m4 ^ m2 ^ m1
}

/// SRCPC-encode `info` at `class_rate` (0..=24 ⇒ 8/8..8/32),
/// optionally terminated. The encoder always starts from the all-zero
/// state (§1.8.4.6.1).
pub fn srcpc_encode(info: &[bool], class_rate: u8, terminated: bool) -> Result<Vec<bool>> {
    let p = puncture_pattern(class_rate)?;
    let mut out = Vec::with_capacity(srcpc_coded_len(info.len(), class_rate, terminated)?);
    let mut state = 0u8;
    let mut t = 0usize;
    let emit = |state: &mut u8, u: bool, t: usize, out: &mut Vec<bool>| {
        let (next, v) = step(*state, u);
        *state = next;
        for (i, &line) in p.iter().enumerate() {
            if line & (0x80 >> (t % 8)) != 0 {
                out.push(v[i]);
            }
        }
    };
    for &u in info {
        emit(&mut state, u, t, &mut out);
        t += 1;
    }
    if terminated {
        for _ in 0..SRCPC_TAIL_BITS {
            let u = feedback(state);
            emit(&mut state, u, t, &mut out);
            t += 1;
        }
        debug_assert_eq!(state, 0, "termination must return to state 0");
    }
    Ok(out)
}

/// Hard-decision Viterbi decode of an SRCPC stream (§1.8.4.6.4):
/// recovers `n_info` information bits from `coded`, correcting
/// transmission errors up to the punctured code's capability.
///
/// `coded.len()` must equal
/// [`srcpc_coded_len`]`(n_info, class_rate, terminated)`.
pub fn srcpc_decode(
    coded: &[bool],
    n_info: usize,
    class_rate: u8,
    terminated: bool,
) -> Result<Vec<bool>> {
    let p = puncture_pattern(class_rate)?;
    let steps = n_info + if terminated { SRCPC_TAIL_BITS } else { 0 };
    if coded.len() != srcpc_coded_len(n_info, class_rate, terminated)? {
        return Err(Error::EpFrameInvalid);
    }

    const INF: u32 = u32::MAX / 2;
    let mut metric = [INF; 16];
    metric[0] = 0;
    // survivors[t][s] = (previous state, input bit) — tail steps have
    // a forced input, still recorded uniformly.
    let mut survivors: Vec<[(u8, bool); 16]> = Vec::with_capacity(steps);

    let mut pos = 0usize;
    for t in 0..steps {
        // The emitted lines at this step.
        let mut lines: [bool; 4] = [false; 4];
        let mut n_lines = 0usize;
        for (i, &line) in p.iter().enumerate() {
            lines[i] = line & (0x80 >> (t % 8)) != 0;
            if lines[i] {
                n_lines += 1;
            }
        }
        let received = &coded[pos..pos + n_lines];
        pos += n_lines;

        let mut next_metric = [INF; 16];
        let mut surv = [(0u8, false); 16];
        for s in 0u8..16 {
            if metric[usize::from(s)] >= INF {
                continue;
            }
            let inputs: &[bool] = if t >= n_info {
                // Termination steps: the input is forced to d(state).
                if feedback(s) {
                    &[true]
                } else {
                    &[false]
                }
            } else {
                &[false, true]
            };
            for &u in inputs {
                let (next, v) = step(s, u);
                let mut m = metric[usize::from(s)];
                let mut ri = 0usize;
                for (i, &on) in lines.iter().enumerate() {
                    if on {
                        if v[i] != received[ri] {
                            m += 1;
                        }
                        ri += 1;
                    }
                }
                let slot = usize::from(next);
                if m < next_metric[slot] {
                    next_metric[slot] = m;
                    surv[slot] = (s, u);
                }
            }
        }
        metric = next_metric;
        survivors.push(surv);
    }

    // Terminated streams end in state 0; otherwise take the best.
    let mut state: u8 = if terminated {
        if metric[0] >= INF {
            return Err(Error::EpFrameInvalid);
        }
        0
    } else {
        let (best, m) = metric
            .iter()
            .enumerate()
            .min_by_key(|(_, &m)| m)
            .map(|(s, &m)| (s as u8, m))
            .unwrap_or((0, INF));
        if m >= INF {
            return Err(Error::EpFrameInvalid);
        }
        best
    };

    let mut bits = vec![false; steps];
    for t in (0..steps).rev() {
        let (prev, u) = survivors[t][usize::from(state)];
        bits[t] = u;
        state = prev;
    }
    bits.truncate(n_info);
    Ok(bits)
}

/// One §1.8.4.3 basic block code, selected by the protected length.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeaderFec {
    /// 1–2 bits: majority (each bit repeated 3 times).
    Majority,
    /// 3–4 bits: BCH(7,4), g = x³ + x + 1.
    Bch7,
    /// 5–7 bits: BCH(15,7), g = x⁸ + x⁷ + x⁶ + x⁴ + 1.
    Bch15,
    /// 8–12 bits: Golay(23,12), g = x¹¹ + x⁹ + x⁷ + x⁶ + x⁵ + x + 1.
    Golay23,
    /// 13–16 bits: BCH(31,16),
    /// g = x¹⁵ + x¹¹ + x¹⁰ + x⁹ + x⁸ + x⁷ + x⁵ + x³ + x² + x + 1.
    Bch31,
    /// 17+ bits: CRC4 + terminated SRCPC 8/16.
    Srcpc16,
}

impl HeaderFec {
    /// The Table 1.59 length-driven selection.
    pub fn for_len(l: usize) -> Result<Self> {
        Ok(match l {
            0 => return Err(Error::EpFrameInvalid),
            1..=2 => HeaderFec::Majority,
            3..=4 => HeaderFec::Bch7,
            5..=7 => HeaderFec::Bch15,
            8..=12 => HeaderFec::Golay23,
            13..=16 => HeaderFec::Bch31,
            _ => HeaderFec::Srcpc16,
        })
    }

    /// `(generator polynomial bits above x⁰ .. as u32 with implicit
    /// leading term INCLUDED, parity bit count, correction capability)`
    /// for the polynomial codes.
    fn poly(self) -> Option<(u32, usize, usize)> {
        match self {
            // x³+x+1 → 0b1011, 3 parity bits, t = 1.
            HeaderFec::Bch7 => Some((0b1011, 3, 1)),
            // x⁸+x⁷+x⁶+x⁴+1 → 1_1101_0001, 8 parity bits, t = 2.
            HeaderFec::Bch15 => Some((0b1_1101_0001, 8, 2)),
            // x¹¹+x⁹+x⁷+x⁶+x⁵+x+1 → 1010_1110_0011, 11 parity, t = 3.
            HeaderFec::Golay23 => Some((0b1010_1110_0011, 11, 3)),
            // x¹⁵+x¹¹+x¹⁰+x⁹+x⁸+x⁷+x⁵+x³+x²+x+1, 15 parity, t = 3.
            HeaderFec::Bch31 => Some((0b1000_1111_1010_1111, 15, 3)),
            _ => None,
        }
    }

    /// Number of parity bits appended for `l` protected bits.
    pub fn parity_bits(self, l: usize) -> Result<usize> {
        Ok(match self {
            HeaderFec::Majority => 2 * l,
            HeaderFec::Srcpc16 => {
                // CRC4 + terminated SRCPC 8/16 over (l + 4) info bits;
                // parity = coded − l.
                srcpc_coded_len(l + 4, 8, true)? - l
            }
            other => other.poly().map(|(_, p, _)| p).unwrap_or(0),
        })
    }
}

/// Polynomial-division parity `R(x)` of `M(x)·x^deg(G) mod G(x)`,
/// MSB-first over `info` (§1.8.4.3).
fn poly_parity(info: &[bool], gen: u32, parity: usize) -> Vec<bool> {
    let top = 1u32 << parity; // the implicit leading term position
    let mut reg: u32 = 0;
    for &bit in info {
        reg = (reg << 1) | u32::from(bit);
        if reg & top != 0 {
            reg ^= gen;
        }
    }
    for _ in 0..parity {
        reg <<= 1;
        if reg & top != 0 {
            reg ^= gen;
        }
    }
    (0..parity)
        .map(|i| reg & (1 << (parity - 1 - i)) != 0)
        .collect()
}

/// Encode a §1.8.4.3 header part: returns the parity bit sequence to
/// transmit after the `l` information bits (`Npred_parity` /
/// `Nattrib_parity`).
pub fn header_fec_encode(info: &[bool]) -> Result<Vec<bool>> {
    let fec = HeaderFec::for_len(info.len())?;
    Ok(match fec {
        HeaderFec::Majority => {
            let mut v = Vec::with_capacity(info.len() * 2);
            v.extend_from_slice(info);
            v.extend_from_slice(info);
            v
        }
        HeaderFec::Srcpc16 => {
            // CRC4 over the info, then terminated SRCPC 8/16 over
            // info + CRC; the parity is everything past the
            // systematic prefix of the coded stream... the coded
            // stream is emitted interleaved per time step, so the
            // whole codeword replaces info + parity: return the full
            // codeword minus the leading l systematic copies is not
            // separable. Instead the parity field carries the coded
            // stream's non-systematic remainder: we transmit the
            // complete coded stream in place of info+parity, so the
            // parity here is the coded stream with the systematic
            // prefix removed positionally. See `header_fec_decode`,
            // which reassembles the same layout.
            let crc = crc_bits(CrcPoly::Crc4, info);
            let mut m: Vec<bool> = info.to_vec();
            for i in (0..4).rev() {
                m.push(crc & (1 << i) != 0);
            }
            let coded = srcpc_encode(&m, 8, true)?;
            // Systematic v1 bits occupy known positions; the parity
            // field is the stream with those positions removed — the
            // decoder re-merges them.
            let mut parity = Vec::with_capacity(coded.len() - info.len());
            for (idx, chunk) in coded.chunks(2).enumerate() {
                // rate 8/16 keeps v1 and v2 every step.
                if idx < info.len() {
                    // chunk[0] is systematic (v1) — drop, it equals
                    // info[idx].
                    parity.push(chunk[1]);
                } else {
                    parity.push(chunk[0]);
                    parity.push(chunk[1]);
                }
            }
            parity
        }
        other => {
            let (gen, p, _) = other.poly().ok_or(Error::EpFrameInvalid)?;
            poly_parity(info, gen, p)
        }
    })
}

/// Decode a §1.8.4.3 header part: `info` are the received (possibly
/// corrupted) information bits, `parity` the received parity bits.
/// Returns the corrected information bits; uncorrectable words
/// surface [`Error::EpFrameInvalid`].
pub fn header_fec_decode(info: &[bool], parity: &[bool]) -> Result<Vec<bool>> {
    let l = info.len();
    let fec = HeaderFec::for_len(l)?;
    if parity.len() != fec.parity_bits(l)? {
        return Err(Error::EpFrameInvalid);
    }
    match fec {
        HeaderFec::Majority => {
            let mut out = Vec::with_capacity(l);
            for i in 0..l {
                let votes = u8::from(info[i]) + u8::from(parity[i]) + u8::from(parity[l + i]);
                out.push(votes >= 2);
            }
            Ok(out)
        }
        HeaderFec::Srcpc16 => {
            // Re-merge the coded stream: v1 comes from `info` for the
            // first l steps, both bits from `parity` afterwards.
            let mut coded = Vec::with_capacity(l + parity.len());
            let mut pi = 0usize;
            for &i_bit in info.iter().take(l) {
                coded.push(i_bit);
                coded.push(parity[pi]);
                pi += 1;
            }
            coded.extend_from_slice(&parity[pi..]);
            let decoded = srcpc_decode(&coded, l + 4, 8, true)?;
            let (msg, crc_bits_rx) = decoded.split_at(l);
            let want = crc_bits(CrcPoly::Crc4, msg);
            let mut got = 0u64;
            for &b in crc_bits_rx {
                got = (got << 1) | u64::from(b);
            }
            if got != want {
                return Err(Error::EpFrameInvalid);
            }
            Ok(msg.to_vec())
        }
        other => {
            let (gen, p, t) = other.poly().ok_or(Error::EpFrameInvalid)?;
            let mut word: Vec<bool> = Vec::with_capacity(l + p);
            word.extend_from_slice(info);
            word.extend_from_slice(parity);
            if poly_syndrome_ok(&word, gen, p) {
                return Ok(info.to_vec());
            }
            // Bounded-distance decoding: search error patterns of
            // weight <= t over the (shortened) codeword.
            let n = word.len();
            let mut positions: Vec<usize> = Vec::with_capacity(t);
            if search_errors(&mut word, gen, p, t, 0, n, &mut positions) {
                return Ok(word[..l].to_vec());
            }
            Err(Error::EpFrameInvalid)
        }
    }
}

/// `true` iff the codeword (info ‖ parity) has an all-zero syndrome
/// under `gen`.
fn poly_syndrome_ok(word: &[bool], gen: u32, parity: usize) -> bool {
    let top = 1u32 << parity;
    let mut reg: u32 = 0;
    for &bit in word {
        reg = (reg << 1) | u32::from(bit);
        if reg & top != 0 {
            reg ^= gen;
        }
    }
    reg == 0
}

/// Recursive bounded-distance search: flip up to `budget` bits from
/// index `from` and test the syndrome. On success the corrected word
/// is left in `word` and `true` is returned.
fn search_errors(
    word: &mut [bool],
    gen: u32,
    parity: usize,
    budget: usize,
    from: usize,
    n: usize,
    positions: &mut Vec<usize>,
) -> bool {
    if budget == 0 {
        return false;
    }
    for i in from..n {
        word[i] = !word[i];
        positions.push(i);
        if poly_syndrome_ok(word, gen, parity)
            || search_errors(word, gen, parity, budget - 1, i + 1, n, positions)
        {
            return true;
        }
        positions.pop();
        word[i] = !word[i];
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prand_bits(n: usize, mut seed: u32) -> Vec<bool> {
        let mut v = Vec::with_capacity(n);
        for _ in 0..n {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            v.push(seed & 0x8000_0000 != 0);
        }
        v
    }

    /// The Table 1.60 tail-bit listing is the closed form of the
    /// `u = d` termination rule.
    #[test]
    fn termination_matches_table_1_60() {
        // Table 1.60 rows: state (m4 m3 m2 m1) -> tail (un-3..un).
        let table: [(u8, [u8; 4]); 16] = [
            (0b0000, [0, 0, 0, 0]),
            (0b0001, [1, 1, 0, 1]),
            (0b0010, [1, 0, 1, 0]),
            (0b0011, [0, 1, 1, 1]),
            (0b0100, [0, 1, 0, 0]),
            (0b0101, [1, 0, 0, 1]),
            (0b0110, [1, 1, 1, 0]),
            (0b0111, [0, 0, 1, 1]),
            (0b1000, [1, 0, 0, 0]),
            (0b1001, [0, 1, 0, 1]),
            (0b1010, [0, 0, 1, 0]),
            (0b1011, [1, 1, 1, 1]),
            (0b1100, [1, 1, 0, 0]),
            (0b1101, [0, 0, 0, 1]),
            (0b1110, [0, 1, 1, 0]),
            (0b1111, [1, 0, 1, 1]),
        ];
        for (packed, tail) in table {
            // Repack (m4 m3 m2 m1) into the module's bit-0 = m1 layout.
            let mut state = 0u8;
            if packed & 0b0001 != 0 {
                state |= 1; // m1
            }
            if packed & 0b0010 != 0 {
                state |= 2; // m2
            }
            if packed & 0b0100 != 0 {
                state |= 4; // m3
            }
            if packed & 0b1000 != 0 {
                state |= 8; // m4
            }
            let mut s = state;
            for (step_i, &want) in tail.iter().enumerate() {
                let u = feedback(s);
                assert_eq!(u8::from(u), want, "state {packed:04b} tail step {step_i}");
                let (next, _) = step(s, u);
                s = next;
            }
            assert_eq!(s, 0, "state {packed:04b} did not terminate");
        }
    }

    #[test]
    fn puncture_patterns_match_table_1_61() {
        // Spot rows straight from Table 1.61.
        assert_eq!(puncture_pattern(0).unwrap(), [0xFF, 0x00, 0x00, 0x00]); // 8/8
        assert_eq!(puncture_pattern(3).unwrap(), [0xFF, 0xA8, 0x00, 0x00]); // 8/11
        assert_eq!(puncture_pattern(5).unwrap(), [0xFF, 0xEA, 0x00, 0x00]); // 8/13
        assert_eq!(puncture_pattern(8).unwrap(), [0xFF, 0xFF, 0x00, 0x00]); // 8/16
        assert_eq!(puncture_pattern(9).unwrap(), [0xFF, 0xFF, 0x80, 0x00]); // 8/17
        assert_eq!(puncture_pattern(16).unwrap(), [0xFF, 0xFF, 0xFF, 0x00]); // 8/24
        assert_eq!(puncture_pattern(17).unwrap(), [0xFF, 0xFF, 0xFF, 0x80]); // 8/25
        assert_eq!(puncture_pattern(24).unwrap(), [0xFF, 0xFF, 0xFF, 0xFF]); // 8/32
    }

    #[test]
    fn srcpc_roundtrip_all_rates() {
        for rate in [0u8, 1, 3, 8, 12, 17, 24] {
            for terminated in [false, true] {
                let info = prand_bits(97, 0xC0FFEE ^ u32::from(rate));
                let coded = srcpc_encode(&info, rate, terminated).unwrap();
                assert_eq!(
                    coded.len(),
                    srcpc_coded_len(info.len(), rate, terminated).unwrap()
                );
                // Rate 8/8 is purely systematic.
                if rate == 0 {
                    let systematic: Vec<bool> = coded
                        .iter()
                        .copied()
                        .take(if terminated {
                            info.len() + 4
                        } else {
                            info.len()
                        })
                        .collect();
                    assert_eq!(&systematic[..info.len()], &info[..]);
                }
                let decoded = srcpc_decode(&coded, info.len(), rate, terminated).unwrap();
                assert_eq!(decoded, info, "rate {rate} terminated {terminated}");
            }
        }
    }

    #[test]
    fn srcpc_corrects_errors() {
        // Rate 8/16 (one parity bit per info bit), terminated: a few
        // well-spread bit errors are corrected by the Viterbi pass.
        let info = prand_bits(120, 0xDEAD);
        let mut coded = srcpc_encode(&info, 8, true).unwrap();
        for &pos in &[10usize, 77, 150, 220] {
            coded[pos] = !coded[pos];
        }
        let decoded = srcpc_decode(&coded, info.len(), 8, true).unwrap();
        assert_eq!(decoded, info);
    }

    #[test]
    fn header_fec_roundtrip_all_classes() {
        for l in [1usize, 2, 3, 4, 5, 7, 8, 12, 13, 16, 17, 30] {
            let info = prand_bits(l, 0xBEEF ^ l as u32);
            let parity = header_fec_encode(&info).unwrap();
            assert_eq!(
                parity.len(),
                HeaderFec::for_len(l).unwrap().parity_bits(l).unwrap(),
                "len {l}"
            );
            let decoded = header_fec_decode(&info, &parity).unwrap();
            assert_eq!(decoded, info, "len {l}");
        }
    }

    #[test]
    fn header_fec_corrects_errors() {
        // Golay(23,12): three errors are correctable.
        let info = prand_bits(12, 0x1234);
        let parity = header_fec_encode(&info).unwrap();
        let mut rx_info = info.clone();
        let mut rx_parity = parity.clone();
        rx_info[3] = !rx_info[3];
        rx_info[9] = !rx_info[9];
        rx_parity[5] = !rx_parity[5];
        assert_eq!(header_fec_decode(&rx_info, &rx_parity).unwrap(), info);

        // Majority: one flip per repeated position corrects.
        let info = prand_bits(2, 0x9);
        let parity = header_fec_encode(&info).unwrap();
        let mut rx_info = info.clone();
        rx_info[0] = !rx_info[0];
        assert_eq!(header_fec_decode(&rx_info, &parity).unwrap(), info);

        // BCH(15,7): two errors.
        let info = prand_bits(6, 0x77);
        let parity = header_fec_encode(&info).unwrap();
        let mut rx_parity = parity.clone();
        rx_parity[0] = !rx_parity[0];
        rx_parity[6] = !rx_parity[6];
        assert_eq!(header_fec_decode(&info, &rx_parity).unwrap(), info);
    }
}
