//! BSAC arithmetic decoder — ISO/IEC 14496-3:2009 §4.5.2.6.2.7.
//!
//! The ER BSAC noiseless coder replaces the AAC Huffman machinery
//! with a single arithmetic code over the whole
//! `bsac_raw_data_block()` (or, in SBA mode, over each segment).
//! The spec normatively lists the decoding procedure as C source
//! (§4.5.2.6.2.7.4); this module transcribes it exactly:
//!
//! * [`ArithDecoder::decode_symbol`] — the general multi-symbol
//!   decode over a 14-bit cumulative-frequency model (`cband_si`,
//!   scalefactors, stereo / PNS side info).
//! * [`ArithDecoder::decode_bit`] — the binary decode over a 14-bit
//!   `p0` (spectral bit slices and sign bits).
//!
//! Both return the **estimated codeword length** (`est_cw_len`) the
//! spec defines — the renormalization shift that will be consumed
//! before the *next* symbol — which the §4.5.2.6.2.5 layer budget
//! (`available_len[]`) bookkeeping subtracts per decoded symbol.
//!
//! The register discipline follows the listing: `value` and `range`
//! are 32-bit quantities (held in `u64` here — the products
//! `range · cum_freq` stay under 2^30, so the arithmetic is
//! identical), `range` starts at 1 with `est_cw_len = 30`, and
//! renormalization scans the `half[]` table (2^29 … 2^14).
//!
//! Reads past the end of the segment buffer return the
//! §4.5.2.6.2.2.1 zero stuffing (a conforming stream never consumes
//! more than 32 such bits; the layer budgets bound all decode
//! loops, so the reader simply keeps yielding zeros).

/// The §4.5.2.6.2.7.1 `half[]` table: 32-bit fixed-point values of
/// ½ at descending magnitudes (2^29 down to 2^14).
const HALF: [u64; 16] = [
    0x2000_0000,
    0x1000_0000,
    0x0800_0000,
    0x0400_0000,
    0x0200_0000,
    0x0100_0000,
    0x0080_0000,
    0x0040_0000,
    0x0020_0000,
    0x0010_0000,
    0x0008_0000,
    0x0004_0000,
    0x0002_0000,
    0x0001_0000,
    0x0000_8000,
    0x0000_4000,
];

/// MSB-first bit reader over one arithmetic segment: a bit window
/// `[start_bit, end_bit)` of the frame buffer, followed by the
/// §4.5.2.6.2.2.1 zero stuffing (zeros for every read past the
/// window).
#[derive(Debug, Clone)]
pub struct SegmentReader<'a> {
    data: &'a [u8],
    /// Absolute next bit position within `data`.
    pos: u64,
    /// Absolute end of the segment window within `data`.
    end: u64,
    /// Bits consumed beyond `end` (the zero-stuffing tail).
    overrun: u64,
}

impl<'a> SegmentReader<'a> {
    /// A reader over bits `[start_bit, end_bit)` of `data`.
    /// `end_bit` is clamped to the buffer size.
    pub fn new(data: &'a [u8], start_bit: u64, end_bit: u64) -> Self {
        let cap = (data.len() as u64) * 8;
        SegmentReader {
            data,
            pos: start_bit.min(cap),
            end: end_bit.min(cap),
            overrun: 0,
        }
    }

    /// Read `n` bits MSB-first (zeros past the window end).
    fn read_bits(&mut self, n: u32) -> u64 {
        let mut v = 0u64;
        for _ in 0..n {
            let bit = if self.pos < self.end {
                let byte = self.data[(self.pos >> 3) as usize];
                u64::from((byte >> (7 - (self.pos & 7))) & 1)
            } else {
                self.overrun += 1;
                0
            };
            self.pos += 1;
            v = (v << 1) | bit;
        }
        v
    }

    /// Bits consumed past the segment window (the zero-stuffing
    /// depth). A conforming stream stays at or under 32.
    pub fn overrun(&self) -> u64 {
        self.overrun
    }
}

/// The §4.5.2.6.2.7 arithmetic decoder registers.
#[derive(Debug, Clone)]
pub struct ArithDecoder {
    value: u64,
    range: u64,
    est_cw_len: u32,
}

impl Default for ArithDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl ArithDecoder {
    /// §4.5.2.6.2.7.2 initialization: `value = 0`, `range = 1`,
    /// `est_cw_len = 30`. Called at the start of every segment.
    pub fn new() -> Self {
        ArithDecoder {
            value: 0,
            range: 1,
            est_cw_len: 30,
        }
    }

    /// Renormalize against `half[]`: the returned `est_cw_len` is
    /// the shift consumed before the next symbol.
    fn renormalize(&mut self) -> u32 {
        let mut est = 0u32;
        while est < HALF.len() as u32 && self.range < HALF[est as usize] {
            est += 1;
        }
        self.est_cw_len = est;
        est
    }

    /// The renormalization shift the next decode will consume.
    pub fn pending_est(&self) -> u32 {
        self.est_cw_len
    }

    /// §4.5.2.6.2.7.4 `decode_symbol()`: general arithmetic decode
    /// over a cumulative-frequency model (14-bit fixed point,
    /// strictly decreasing, last entry 0). Returns
    /// `(symbol, est_cw_len)`.
    pub fn decode_symbol(
        &mut self,
        reader: &mut SegmentReader<'_>,
        cum_freq: &[u16],
    ) -> (usize, u32) {
        if self.est_cw_len > 0 {
            self.range <<= self.est_cw_len;
            self.value = (self.value << self.est_cw_len) | reader.read_bits(self.est_cw_len);
        }
        self.range >>= 14;
        let cum = self.value.checked_div(self.range).unwrap_or(0);
        // The listing's `for (sym = 0; cum_freq[sym] > cum; sym++)`
        // — the last entry is 0 <= cum, so it terminates in range.
        let mut sym = 0usize;
        while sym + 1 < cum_freq.len() && u64::from(cum_freq[sym]) > cum {
            sym += 1;
        }
        self.value -= self.range * u64::from(cum_freq[sym]);
        let width = if sym > 0 {
            u64::from(cum_freq[sym - 1]) - u64::from(cum_freq[sym])
        } else {
            16384 - u64::from(cum_freq[sym])
        };
        self.range *= width;
        (sym, self.renormalize())
    }

    /// §4.5.2.6.2.7.4 `decode_symbol2()`: binary arithmetic decode
    /// with `p0` the 14-bit probability of the "0" symbol. Returns
    /// `(bit, est_cw_len)`.
    pub fn decode_bit(&mut self, reader: &mut SegmentReader<'_>, p0: u16) -> (u8, u32) {
        if self.est_cw_len > 0 {
            self.range <<= self.est_cw_len;
            self.value = (self.value << self.est_cw_len) | reader.read_bits(self.est_cw_len);
        }
        self.range >>= 14;
        let p0 = u64::from(p0);
        let bit;
        if p0 * self.range <= self.value {
            bit = 1;
            self.value -= self.range * p0;
            self.range *= 16384 - p0;
        } else {
            bit = 0;
            self.range *= p0;
        }
        (bit, self.renormalize())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Spec-inverse arithmetic *encoder*, derived from the
    /// §4.5.2.6.2.7.4 decoder listing: the decoder's `value` at
    /// step `k` equals the stream prefix (as an integer) minus the
    /// accumulated `range · cum_freq` subtractions shifted by the
    /// renormalization schedule, so the codeword is
    /// `Σ sub_k · 2^(A_L − A_k)` with `A_k` the bits consumed when
    /// symbol `k` decodes (final `value` chosen 0). Test-only: it
    /// exists to prove the decoder self-consistent on every model.
    struct Encoder {
        range: u64,
        est: u32,
        /// (subtrahend, alignment in bits when it applies).
        subs: Vec<(u64, u64)>,
        /// Bits consumed so far (A_k); starts at the 30-bit init.
        align: u64,
    }

    impl Encoder {
        fn new() -> Self {
            Encoder {
                range: 1,
                est: 30,
                subs: Vec::new(),
                align: 0,
            }
        }

        fn renorm(&mut self) {
            let mut est = 0u32;
            while est < HALF.len() as u32 && self.range < HALF[est as usize] {
                est += 1;
            }
            self.est = est;
        }

        fn push(&mut self, sub: u64, width: u64) {
            self.range <<= self.est;
            self.align += u64::from(self.est);
            self.range >>= 14;
            if sub > 0 {
                self.subs.push((sub, self.align));
            }
            self.range *= width;
            self.renorm();
        }

        fn encode_symbol(&mut self, cum_freq: &[u16], sym: usize) {
            let sub_base = u64::from(cum_freq[sym]);
            let width = if sym > 0 {
                u64::from(cum_freq[sym - 1]) - sub_base
            } else {
                16384 - sub_base
            };
            let rs_now = (self.range << self.est) >> 14;
            self.push(rs_now * sub_base, width);
        }

        fn encode_bit(&mut self, p0: u16, bit: u8) {
            let p0 = u64::from(p0);
            let rs_now = (self.range << self.est) >> 14;
            if bit == 1 {
                self.push(rs_now * p0, 16384 - p0);
            } else {
                self.push(0, p0);
            }
        }

        /// Assemble the codeword bytes (MSB-first bit order): the
        /// integer `Σ sub_k · 2^(total_bits − A_k)` emitted as
        /// `total_bits` bits (the final `value` is chosen 0, which
        /// is always inside the final range).
        fn finish(self) -> Vec<u8> {
            let total_bits = self.align as usize;
            // One accumulator slot per stream bit, MSB-first;
            // sub_k's bit j lands at index `A_k - 1 - j`.
            let mut acc = vec![0u32; total_bits];
            for (sub, align) in &self.subs {
                let mut v = *sub;
                let mut j = 0usize;
                while v > 0 {
                    acc[*align as usize - 1 - j] += (v & 1) as u32;
                    v >>= 1;
                    j += 1;
                }
            }
            // Carry-propagate from the LSB end.
            let mut carry = 0u32;
            for slot in acc.iter_mut().rev() {
                let s = *slot + carry;
                *slot = s & 1;
                carry = s >> 1;
            }
            assert_eq!(carry, 0, "test encoder codeword overflow");
            let mut out = vec![0u8; total_bits.div_ceil(8)];
            for (i, &b) in acc.iter().enumerate() {
                if b != 0 {
                    out[i / 8] |= 1 << (7 - (i % 8));
                }
            }
            out
        }
    }

    fn roundtrip_symbols(model: &[u16], syms: &[usize]) {
        let mut enc = Encoder::new();
        for &s in syms {
            enc.encode_symbol(model, s);
        }
        let bytes = enc.finish();
        let mut rd = SegmentReader::new(&bytes, 0, (bytes.len() as u64) * 8);
        let mut dec = ArithDecoder::new();
        for (i, &s) in syms.iter().enumerate() {
            let (got, _est) = dec.decode_symbol(&mut rd, model);
            assert_eq!(got, s, "symbol {i}");
        }
    }

    fn roundtrip_bits(p0s: &[u16], bits: &[u8]) {
        assert_eq!(p0s.len(), bits.len());
        let mut enc = Encoder::new();
        for (&p, &b) in p0s.iter().zip(bits) {
            enc.encode_bit(p, b);
        }
        let bytes = enc.finish();
        let mut rd = SegmentReader::new(&bytes, 0, (bytes.len() as u64) * 8);
        let mut dec = ArithDecoder::new();
        for (i, (&p, &b)) in p0s.iter().zip(bits).enumerate() {
            let (got, _est) = dec.decode_bit(&mut rd, p);
            assert_eq!(got, b, "bit {i}");
        }
    }

    #[test]
    fn symbol_roundtrip_over_every_model() {
        use crate::bsac_tables::*;
        let mut models: Vec<&[u16]> = vec![
            &MS_USED_MODEL,
            &STEREO_INFO_MODEL,
            &NOISE_FLAG_MODEL,
            &NOISE_MODE_MODEL,
            &CBAND_SI_MODEL_CBAND0,
        ];
        models.extend(CBAND_SI_MODELS.iter().copied());
        models.extend(SCF_MODELS.iter().flatten().copied());
        let mut seed = 0xC0FFEEu32;
        for model in models {
            let mut syms = Vec::new();
            for _ in 0..40 {
                seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                syms.push((seed >> 11) as usize % model.len());
            }
            roundtrip_symbols(model, &syms);
        }
    }

    #[test]
    fn bit_roundtrip_over_spectral_probabilities() {
        use crate::bsac_tables::spectral_p0;
        let mut seed = 0xBEEFu32;
        let mut p0s = Vec::new();
        let mut bits = Vec::new();
        for cband_si in [1u8, 4, 7, 9, 12, 15, 22] {
            let plane = crate::bsac_tables::CBAND_SI_MSB_PLANE[cband_si as usize];
            for snf in 1..=plane {
                for hbv in [0u32, 1, 3, 16] {
                    let rel = plane - snf;
                    if hbv != 0 && (rel == 0 || (rel < 31 && hbv >= (1 << rel))) {
                        continue;
                    }
                    for pos in [0usize, 7, 33, 64] {
                        let pos = if rel == 0 { pos.min(14) } else { pos };
                        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                        p0s.push(spectral_p0(cband_si, snf, hbv, pos));
                        bits.push(((seed >> 13) & 1) as u8);
                    }
                }
            }
        }
        roundtrip_bits(&p0s, &bits);
    }

    #[test]
    fn mixed_symbol_and_bit_roundtrip() {
        use crate::bsac_tables::{MS_USED_MODEL, SCF_MODELS, SIGN_P0};
        let scf = SCF_MODELS[3].unwrap();
        let mut enc = Encoder::new();
        enc.encode_symbol(scf, 5);
        enc.encode_bit(SIGN_P0, 1);
        enc.encode_symbol(&MS_USED_MODEL, 1);
        enc.encode_bit(0x3f00, 0);
        enc.encode_bit(0x0100, 1);
        enc.encode_symbol(scf, 15);
        let bytes = enc.finish();
        let mut rd = SegmentReader::new(&bytes, 0, (bytes.len() as u64) * 8);
        let mut dec = ArithDecoder::new();
        assert_eq!(dec.decode_symbol(&mut rd, scf).0, 5);
        assert_eq!(dec.decode_bit(&mut rd, SIGN_P0).0, 1);
        assert_eq!(dec.decode_symbol(&mut rd, &MS_USED_MODEL).0, 1);
        assert_eq!(dec.decode_bit(&mut rd, 0x3f00).0, 0);
        assert_eq!(dec.decode_bit(&mut rd, 0x0100).0, 1);
        assert_eq!(dec.decode_symbol(&mut rd, scf).0, 15);
    }

    #[test]
    fn zero_stuffing_supplies_zero_bits() {
        let mut rd = SegmentReader::new(&[0xff], 0, 8);
        assert_eq!(rd.read_bits(8), 0xff);
        assert_eq!(rd.read_bits(8), 0);
        assert_eq!(rd.overrun(), 8);
    }
}
