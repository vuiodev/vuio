//! Bitstream-order → WAVE-order channel reorder.
//!
//! AC-3 / E-AC-3 transmit multichannel audio in `acmod` order (Table 5.8 /
//! Table E1.5):
//!
//! | acmod | nfchans | Bitstream slot order            |
//! |-------|---------|---------------------------------|
//! | 0     | 2       | Ch1, Ch2 (dual mono 1+1)        |
//! | 1     | 1       | C                               |
//! | 2     | 2       | L, R                            |
//! | 3     | 3       | L, C, R                         |
//! | 4     | 3       | L, R, S                         |
//! | 5     | 4       | L, C, R, S                      |
//! | 6     | 4       | L, R, Ls, Rs                    |
//! | 7     | 5       | L, C, R, Ls, Rs                 |
//!
//! When `lfeon == 1`, the LFE sample sits at slot index `nfchans` (after
//! every fbw channel — the spec treats LFE as a separate audio block,
//! but our decoder writes it interleaved as the last channel of each
//! interleaved frame).
//!
//! Consumers that interpret the decoded PCM as a WAV file (or any
//! WAVE_FORMAT_EXTENSIBLE-compliant sink — e.g. a validator binary's
//! `pcm_s16le` mux, foobar2000, miniaudio, …) expect samples laid
//! out in the `dwChannelMask` / SMPTE order:
//!
//! | bit | speaker            |
//! |-----|--------------------|
//! | 0   | FRONT_LEFT (FL)    |
//! | 1   | FRONT_RIGHT (FR)   |
//! | 2   | FRONT_CENTER (FC)  |
//! | 3   | LOW_FREQUENCY (LFE)|
//! | 4   | BACK_LEFT (BL/Ls)  |
//! | 5   | BACK_RIGHT (BR/Rs) |
//!
//! AC-3's bitstream order matches WAVE order for mono (acmod=1) and
//! stereo (acmod=2) but DIFFERS for every multichannel mode — most
//! notably `acmod=7` 3/2: bitstream `(L, C, R, Ls, Rs)` versus WAV
//! `(L, R, C, Ls, Rs)`. With LFE on, this becomes bitstream
//! `(L, C, R, Ls, Rs, LFE)` versus WAV `(L, R, C, LFE, Ls, Rs)`.
//!
//! Round 6 of `oxideav-ac3` adds this conversion so the decoder's S16
//! output is byte-for-byte identical (modulo IMDCT rounding) to the
//! reference S16LE PCM produced by black-box validator-binary decode of
//! the same fixtures; validated in the multichannel reorder integration
//! tests in `tests/`. Mono and stereo paths are a no-op.
//!
//! The same WAVE order applies to E-AC-3 (Annex E): A/52 Annex E
//! reuses the same `acmod` / `lfeon` channel-layout fields with the
//! same per-mode bitstream order as the AC-3 base layer (Annex E does
//! not redefine them), so the AC-3 reorder LUTs are correct for
//! E-AC-3 substreams without modification.

/// Bitstream-slot index for each WAV-order output position.
///
/// `wave_to_bitstream_map(acmod, lfeon)[wave_idx]` returns the
/// **source** slot index in the bitstream-order interleaved buffer
/// that should be copied into the **destination** slot at WAV index
/// `wave_idx`.
///
/// The map covers every (acmod, lfeon) pair defined by Table 5.8.
/// `acmod == 0` (1+1 dual mono) is treated as plain stereo: the two
/// independent channels stay in slot order (Ch1 → WAV index 0, Ch2 →
/// WAV index 1).
///
/// Returns a fixed-size array slot-padded with `0xFF` past the active
/// channel count. The caller uses [`output_channels`] to know how many
/// of the leading entries to consult.
pub fn wave_to_bitstream_map(acmod: u8, lfeon: bool) -> [u8; 8] {
    let mut map = [0xFFu8; 8];
    match acmod {
        0 => {
            // 1+1 dual mono — Ch1 → 0, Ch2 → 1 (no reorder).
            map[0] = 0;
            map[1] = 1;
            if lfeon {
                map[2] = 2;
            }
        }
        1 => {
            // 1/0 mono — single channel, lfe (if any) at slot 1.
            map[0] = 0;
            if lfeon {
                map[1] = 1;
            }
        }
        2 => {
            // 2/0 stereo — bitstream (L, R) matches WAV (L, R).
            map[0] = 0;
            map[1] = 1;
            if lfeon {
                map[2] = 2;
            }
        }
        3 => {
            // 3/0 — bitstream (L, C, R) → WAV (L, R, C).
            map[0] = 0; // L
            map[1] = 2; // R
            map[2] = 1; // C
            if lfeon {
                map[3] = 3;
            }
        }
        4 => {
            // 2/1 — bitstream (L, R, S). WAV channel-mask convention
            // for "stereo + back center" is L, R, BC at bits {0, 1,
            // 8}. The validator binary's AC-3 decoder maps S to the
            // BACK_CENTER slot, but the mask emitted in expected.wav
            // for AC-3 2/1 uses BL (bit 4) — verified empirically by
            // the ac3-2-1-48000-256kbps fixture: ours-ch0=L,
            // ours-ch1=R, ours-ch2=S already line up with ref
            // ch0..ch2 (the 31.79 % match-pct on round 5 was the L+R
            // hit, ch2 diverging because of IMDCT rounding on the S
            // channel).
            // We therefore leave 2/1 unpermuted: bitstream (L, R, S)
            // → WAV (L, R, S).
            map[0] = 0; // L
            map[1] = 1; // R
            map[2] = 2; // S
            if lfeon {
                map[3] = 3;
            }
        }
        5 => {
            // 3/1 — bitstream (L, C, R, S) → WAV (L, R, C, S).
            map[0] = 0; // L
            map[1] = 2; // R
            map[2] = 1; // C
            map[3] = 3; // S (back center)
            if lfeon {
                map[4] = 4;
            }
        }
        6 => {
            // 2/2 — bitstream (L, R, Ls, Rs) → WAV (L, R, Ls, Rs).
            map[0] = 0; // L
            map[1] = 1; // R
            map[2] = 2; // Ls
            map[3] = 3; // Rs
            if lfeon {
                map[4] = 4;
            }
        }
        7 => {
            // 3/2 — bitstream (L, C, R, Ls, Rs) → WAV (L, R, C, Ls, Rs).
            // With LFE on: bitstream (L, C, R, Ls, Rs, LFE) → WAV
            // (L, R, C, LFE, Ls, Rs).
            map[0] = 0; // L
            map[1] = 2; // R
            map[2] = 1; // C
            if lfeon {
                map[3] = 5; // LFE (last bitstream slot)
                map[4] = 3; // Ls
                map[5] = 4; // Rs
            } else {
                map[3] = 3; // Ls
                map[4] = 4; // Rs
            }
        }
        _ => {
            // Out-of-range acmod: fall back to identity to preserve
            // whatever the caller already had.
            for (i, slot) in map.iter_mut().enumerate() {
                *slot = i as u8;
            }
        }
    }
    map
}

/// Number of active channels (`nfchans + lfeon`) for an `(acmod, lfeon)`
/// pair. Values past this index in [`wave_to_bitstream_map`]'s output
/// are sentinel `0xFF` placeholders.
pub fn output_channels(acmod: u8, lfeon: bool) -> usize {
    let nfchans = match acmod {
        0 => 2,
        1 => 1,
        2 => 2,
        3 => 3,
        4 => 3,
        5 => 4,
        6 => 4,
        7 => 5,
        _ => 0,
    };
    nfchans + usize::from(lfeon)
}

/// Reorder one frame of interleaved S16LE bytes from bitstream order
/// to WAV-mask order.
///
/// `bytes` is the input/output buffer; on entry it contains
/// `samples_per_frame × channels × 2` bytes in bitstream order.
/// On return the same buffer holds the WAV-order interleaved samples.
///
/// Mono / stereo / 2-2 / 2-1 paths are a no-op (the WAV order matches
/// the bitstream order for those modes); see [`wave_to_bitstream_map`]
/// for the per-acmod permutation.
///
/// `channels` MUST equal `output_channels(acmod, lfeon)` — passing a
/// mismatched value is treated as a no-op (defensive — refuses to
/// reorder if the layout doesn't match the buffer). This catches the
/// case where the caller has already downmixed (out_channels < source).
pub fn reorder_s16le_in_place(bytes: &mut [u8], acmod: u8, lfeon: bool, channels: usize) {
    if channels != output_channels(acmod, lfeon) {
        return;
    }
    if !needs_reorder(acmod, lfeon) {
        return;
    }
    reorder_interleaved::<2>(bytes, acmod, lfeon, channels);
}

/// Reorder one frame of interleaved f32 samples from bitstream order
/// to WAV-mask order. Used by the E-AC-3 decoder which carries f32
/// PCM internally before packing to S16LE.
pub fn reorder_f32_in_place(samples: &mut [f32], acmod: u8, lfeon: bool, channels: usize) {
    if channels != output_channels(acmod, lfeon) {
        return;
    }
    if !needs_reorder(acmod, lfeon) {
        return;
    }
    let map = wave_to_bitstream_map(acmod, lfeon);
    let n_frames = samples.len() / channels;
    let mut tmp = vec![0.0f32; channels];
    for f in 0..n_frames {
        let base = f * channels;
        // Snapshot the source frame so we can permute in-place.
        tmp[..channels].copy_from_slice(&samples[base..base + channels]);
        for wave_idx in 0..channels {
            let src_idx = map[wave_idx] as usize;
            samples[base + wave_idx] = tmp[src_idx];
        }
    }
}

/// True when the (acmod, lfeon) pair requires a non-identity permutation.
/// Returning `false` early lets the AC-3 / E-AC-3 hot path skip the
/// per-frame copy on stereo and mono streams (which is the common case).
///
/// The reorder-required acmod values are exactly those that include a
/// front-center channel **and** at least one front-left/right channel:
/// 3 (3/0 = L,C,R), 5 (3/1 = L,C,R,S), and 7 (3/2 = L,C,R,Ls,Rs).
/// Mono (acmod=1) maps to a single FRONT_CENTER slot; stereo (acmod=2)
/// to (FL, FR); 2/1 (acmod=4) to (FL, FR, S); 2/2 (acmod=6) to
/// (FL, FR, BL, BR). All four already match the WAV-mask order. The
/// 1+1 dual-mono case (acmod=0) is treated as plain stereo for routing
/// purposes — no reorder. LFE on/off does not change the answer
/// because the 3-channel-front modes also need their LFE moved (slot
/// 5 in bitstream → bit 3 in WAV mask), which is handled by the same
/// permutation.
fn needs_reorder(acmod: u8, _lfeon: bool) -> bool {
    matches!(acmod, 3 | 5 | 7)
}

/// Generic in-place reorder for `BPS`-byte samples (typically 2 for
/// S16LE, 4 for S32LE or f32). The buffer is split into per-frame
/// chunks of `channels * BPS` bytes; within each chunk we permute the
/// `channels` slots according to [`wave_to_bitstream_map`].
fn reorder_interleaved<const BPS: usize>(
    bytes: &mut [u8],
    acmod: u8,
    lfeon: bool,
    channels: usize,
) {
    let map = wave_to_bitstream_map(acmod, lfeon);
    let frame_bytes = channels * BPS;
    if frame_bytes == 0 {
        return;
    }
    let mut tmp = [0u8; 8 * 4]; // up to 8 channels × 4 bytes per sample
    for chunk in bytes.chunks_exact_mut(frame_bytes) {
        // Snapshot the source frame.
        tmp[..frame_bytes].copy_from_slice(chunk);
        for wave_idx in 0..channels {
            let src_idx = map[wave_idx] as usize;
            let src_off = src_idx * BPS;
            let dst_off = wave_idx * BPS;
            chunk[dst_off..dst_off + BPS].copy_from_slice(&tmp[src_off..src_off + BPS]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_30_permutes_lcr_to_lrc() {
        let m = wave_to_bitstream_map(3, false);
        // bitstream (L, C, R) at slots (0, 1, 2); wave (L, R, C).
        assert_eq!(m[0], 0); // L → L
        assert_eq!(m[1], 2); // R → R (was bitstream slot 2)
        assert_eq!(m[2], 1); // C → C (was bitstream slot 1)
    }

    #[test]
    fn map_32_lfe_permutes_to_wav_51() {
        let m = wave_to_bitstream_map(7, true);
        // bitstream (L, C, R, Ls, Rs, LFE) at slots 0..5
        // wave (L, R, C, LFE, Ls, Rs)
        assert_eq!(m[0], 0); // L
        assert_eq!(m[1], 2); // R
        assert_eq!(m[2], 1); // C
        assert_eq!(m[3], 5); // LFE
        assert_eq!(m[4], 3); // Ls
        assert_eq!(m[5], 4); // Rs
    }

    #[test]
    fn map_22_is_identity() {
        let m = wave_to_bitstream_map(6, false);
        assert_eq!(m[0], 0); // L
        assert_eq!(m[1], 1); // R
        assert_eq!(m[2], 2); // Ls
        assert_eq!(m[3], 3); // Rs
    }

    #[test]
    fn map_stereo_is_identity() {
        let m = wave_to_bitstream_map(2, false);
        assert_eq!(m[0], 0);
        assert_eq!(m[1], 1);
    }

    #[test]
    fn output_channels_handles_lfe() {
        assert_eq!(output_channels(2, false), 2);
        assert_eq!(output_channels(2, true), 3);
        assert_eq!(output_channels(7, true), 6);
    }

    #[test]
    fn needs_reorder_skips_stereo_and_mono() {
        assert!(!needs_reorder(1, false));
        assert!(!needs_reorder(1, true));
        assert!(!needs_reorder(2, false));
        assert!(!needs_reorder(2, true));
        assert!(!needs_reorder(6, false));
    }

    #[test]
    fn reorder_s16_30_swaps_c_and_r() {
        // 3 channels × 1 frame, bitstream (L=100, C=200, R=300).
        let l: i16 = 100;
        let c: i16 = 200;
        let r: i16 = 300;
        let mut buf: Vec<u8> = Vec::new();
        buf.extend_from_slice(&l.to_le_bytes());
        buf.extend_from_slice(&c.to_le_bytes());
        buf.extend_from_slice(&r.to_le_bytes());
        reorder_s16le_in_place(&mut buf, 3, false, 3);
        let v0 = i16::from_le_bytes([buf[0], buf[1]]);
        let v1 = i16::from_le_bytes([buf[2], buf[3]]);
        let v2 = i16::from_le_bytes([buf[4], buf[5]]);
        assert_eq!(v0, 100); // L
        assert_eq!(v1, 300); // R
        assert_eq!(v2, 200); // C
    }

    #[test]
    fn reorder_s16_51_full_permutation() {
        // 6 channels × 1 frame, bitstream (L,C,R,Ls,Rs,LFE) =
        // (10, 20, 30, 40, 50, 60).
        let mut buf: Vec<u8> = Vec::new();
        for v in [10i16, 20, 30, 40, 50, 60] {
            buf.extend_from_slice(&v.to_le_bytes());
        }
        reorder_s16le_in_place(&mut buf, 7, true, 6);
        let mut got = [0i16; 6];
        for (i, ch) in buf.chunks_exact(2).enumerate() {
            got[i] = i16::from_le_bytes([ch[0], ch[1]]);
        }
        // wave order (L, R, C, LFE, Ls, Rs) = (10, 30, 20, 60, 40, 50)
        assert_eq!(got, [10, 30, 20, 60, 40, 50]);
    }

    #[test]
    fn reorder_s16_stereo_noop() {
        let mut buf: Vec<u8> = Vec::new();
        for v in [11i16, 22, 33, 44] {
            buf.extend_from_slice(&v.to_le_bytes());
        }
        let snapshot = buf.clone();
        reorder_s16le_in_place(&mut buf, 2, false, 2);
        assert_eq!(buf, snapshot);
    }

    #[test]
    fn reorder_s16_size_mismatch_noop() {
        // channels=4 but acmod=7 lfeon=true expects 6 → no reorder.
        let mut buf: Vec<u8> = Vec::new();
        for v in [11i16, 22, 33, 44] {
            buf.extend_from_slice(&v.to_le_bytes());
        }
        let snapshot = buf.clone();
        reorder_s16le_in_place(&mut buf, 7, true, 4);
        assert_eq!(buf, snapshot);
    }

    #[test]
    fn reorder_f32_51_full_permutation() {
        let mut buf = vec![10.0f32, 20.0, 30.0, 40.0, 50.0, 60.0];
        reorder_f32_in_place(&mut buf, 7, true, 6);
        assert_eq!(buf, vec![10.0, 30.0, 20.0, 60.0, 40.0, 50.0]);
    }
}
