//! Per-frame multi-channel 32-band synthesis QMF driver — the
//! channel-loop wrapper around the §C.2.5 `QMFInterpolation()`
//! per-channel call.
//!
//! The §C.2.5 normative driver (transcribed verbatim in
//! `docs/audio/dts/dts-core-extracts.md` §2.4, ETSI TS 102 114 V1.3.1
//! Annex C §C.2.5, staged PDF p.185) is invoked **once per channel**:
//!
//! ```text
//! aPrmCh[ch].QMFInterpolation(FILTS, nSUBS[ch]);
//! ```
//!
//! where `aPrmCh[ch]` is the persistent per-channel filter object
//! ([`crate::QmfSynthesis`]) and `nSUBS[ch]` is that channel's count of
//! active subbands (§C.2.5: "higher subbands zero; joint-intensity-coded
//! subbands take the source channel's value"). `FILTS` (the §5.3.1
//! Table 5-15 "Multirate Interpolator Switch") is a single frame-header
//! flag shared by every channel, and the output `rScale` is the single
//! post-filterbank float→PCM gain derived from the frame header's `PCMR`
//! source resolution (`docs/audio/dts/dts-qmf-driver.md` §1/§2). Both
//! are constant across the frame's channels.
//!
//! [`MultiChannelQmf`] owns one [`QmfSynthesis`] per channel and runs
//! the §C.2.5 per-channel call for all of them over one block of
//! subband samples, so a caller that has decoded a frame's per-channel
//! subband samples reconstructs the whole frame's PCM in one call. The
//! per-channel filter state (`raX[]` / `raZ[]`) persists across calls
//! exactly as each underlying [`QmfSynthesis`] persists it, so feeding
//! a stream's frames in order carries every channel's inter-frame
//! filter tail correctly.
//!
//! # Scope: composition of landed primitives
//!
//! This module composes the already-landed [`QmfSynthesis`] driver (the
//! §C.2.5 per-sample loop body) across channels. It adds no new spec
//! step — the only spec construct it materialises is the channel loop
//! around `aPrmCh[ch].QMFInterpolation(...)` and the planar/interleaved
//! arrangement of the per-channel `naCh[]` outputs. The header-sourced
//! `FILTS` / `rScale` come from [`crate::DtsFrameHeader`] accessors
//! ([`crate::DtsFrameHeader::filter_bank_selection`] /
//! [`crate::DtsFrameHeader::output_r_scale`]) per the round-335 bridge.

use crate::cos_mod::NUM_SUBBAND;
use crate::filter_bank::FilterBankSelection;
use crate::qmf_assemble::{QmfAssembleError, PCM_OUTPUT_PER_SAMPLE};
use crate::qmf_synth::QmfSynthesis;

/// Errors specific to the multi-channel synthesis driver, layered over
/// the per-channel [`QmfAssembleError`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum MultiChannelQmfError {
    /// A per-channel synthesis step failed; carries the channel index
    /// `ch` at which it failed and the underlying [`QmfAssembleError`].
    Channel {
        /// 0-based channel index whose §C.2.5 call failed.
        ch: usize,
        /// The underlying per-channel error.
        source: QmfAssembleError,
    },
    /// The caller-supplied per-channel `n_subs` slice length did not
    /// match the driver's channel count.
    NSubsLenMismatch {
        /// The driver's configured channel count.
        channels: usize,
        /// The length of the supplied `n_subs` slice.
        got: usize,
    },
    /// The caller-supplied per-channel subband-sample slice count did
    /// not match the driver's channel count.
    ChannelSlicesLenMismatch {
        /// The driver's configured channel count.
        channels: usize,
        /// The number of per-channel slices supplied.
        got: usize,
    },
    /// Two channels carried a different number of sample rows. Every
    /// channel of one frame block must carry the same number of
    /// per-sample subband rows (the §C.2.5 outer loop runs
    /// `nStart..nEnd` identically for every channel of a frame).
    RowCountMismatch {
        /// Row count of channel 0 (the reference).
        expected: usize,
        /// 0-based channel index whose row count differed.
        ch: usize,
        /// That channel's row count.
        got: usize,
    },
}

impl core::fmt::Display for MultiChannelQmfError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            MultiChannelQmfError::Channel { ch, source } => {
                write!(f, "channel {ch} synthesis failed: {source}")
            }
            MultiChannelQmfError::NSubsLenMismatch { channels, got } => {
                write!(
                    f,
                    "n_subs length {got} does not match channel count {channels}"
                )
            }
            MultiChannelQmfError::ChannelSlicesLenMismatch { channels, got } => {
                write!(
                    f,
                    "channel-slices count {got} does not match channel count {channels}"
                )
            }
            MultiChannelQmfError::RowCountMismatch { expected, ch, got } => {
                write!(
                    f,
                    "channel {ch} carried {got} sample rows, expected {expected}"
                )
            }
        }
    }
}

impl std::error::Error for MultiChannelQmfError {}

/// Persistent per-frame, multi-channel 32-band synthesis QMF.
///
/// Holds one [`QmfSynthesis`] per channel — the §C.2.5 `aPrmCh[ch]`
/// filter objects — and drives the per-channel `QMFInterpolation()`
/// call for every channel over one block of subband samples. The
/// per-channel filter state persists across [`MultiChannelQmf::synthesize_planar`]
/// / [`MultiChannelQmf::synthesize_interleaved`] calls, so a multi-frame
/// stream reuses one instance and feeds its frames in order.
#[derive(Debug, Clone)]
pub struct MultiChannelQmf {
    /// One persistent §C.2.5 filter object per channel.
    channels: Vec<QmfSynthesis>,
}

impl MultiChannelQmf {
    /// Construct a driver for `channels` channels, each with a freshly
    /// cleared per-channel filter history (matching the §C.2.5
    /// per-channel filter's initial state before the first subframe).
    ///
    /// `channels` is the frame's audio-channel count — e.g. the value
    /// from [`crate::DtsFrameHeader::channel_count`]. A zero-channel
    /// driver is permitted (it produces no output) for callers that
    /// resolve the channel count dynamically.
    #[must_use]
    pub fn new(channels: usize) -> Self {
        Self {
            channels: (0..channels).map(|_| QmfSynthesis::new()).collect(),
        }
    }

    /// The driver's channel count.
    #[must_use]
    pub fn channel_count(&self) -> usize {
        self.channels.len()
    }

    /// Borrow the per-channel [`QmfSynthesis`] filter objects (the
    /// §C.2.5 `aPrmCh[]` array). Exposed for callers that want to
    /// inspect or checkpoint a channel's inter-frame filter tail.
    #[must_use]
    pub fn channels(&self) -> &[QmfSynthesis] {
        &self.channels
    }

    /// Run the §C.2.5 per-channel `QMFInterpolation()` call for every
    /// channel and append the result **planar** — channel 0's PCM
    /// first, then channel 1's, and so on — to `output[ch]`.
    ///
    /// `channel_samples[ch]` is channel `ch`'s block of per-sample
    /// subband rows (one `[f64; 32]` per `nSubIndex`, the same layout
    /// [`QmfSynthesis::synthesize`] consumes). `n_subs[ch]` is that
    /// channel's count of active subbands (`nSUBS[ch]`). `filter` and
    /// `r_scale` are the frame-wide §C.2.5 parameters (the resolved
    /// `FILTS` branch and the post-filterbank output `rScale`); see
    /// [`MultiChannelQmf::synthesize_planar_from_header`] for sourcing
    /// them from a parsed header.
    ///
    /// `output` must have one `Vec<i32>` per channel; each channel's
    /// reconstructed PCM (`rows * 32` samples) is appended to its own
    /// vec. The per-channel filter state persists for the next call.
    ///
    /// # Errors
    ///
    /// - [`MultiChannelQmfError::NSubsLenMismatch`] /
    ///   [`MultiChannelQmfError::ChannelSlicesLenMismatch`] if the
    ///   per-channel slice lengths do not match the channel count (or
    ///   the `output` length differs, which is reported as a
    ///   channel-slices mismatch against `output`).
    /// - [`MultiChannelQmfError::RowCountMismatch`] if two channels
    ///   carry a different number of sample rows.
    /// - [`MultiChannelQmfError::Channel`] wrapping the per-channel
    ///   [`QmfAssembleError`] if a channel's §C.2.5 call fails.
    pub fn synthesize_planar(
        &mut self,
        channel_samples: &[&[[f64; NUM_SUBBAND]]],
        n_subs: &[usize],
        filter: FilterBankSelection,
        r_scale: f64,
        output: &mut [Vec<i32>],
    ) -> Result<(), MultiChannelQmfError> {
        self.check_lengths(channel_samples, n_subs, output.len())?;

        for (ch, q) in self.channels.iter_mut().enumerate() {
            q.synthesize(
                channel_samples[ch],
                n_subs[ch],
                filter,
                r_scale,
                &mut output[ch],
            )
            .map_err(|source| MultiChannelQmfError::Channel { ch, source })?;
        }
        Ok(())
    }

    /// Run the §C.2.5 per-channel call for every channel and append the
    /// result **interleaved** (sample-major: for each output sample
    /// index, channel 0's value then channel 1's, …) to `output`.
    ///
    /// All channels emit the same number of PCM samples
    /// (`rows * 32`), so the interleaving is well-defined: the output
    /// length grows by `channels * rows * 32`. Arguments otherwise
    /// match [`MultiChannelQmf::synthesize_planar`].
    ///
    /// # Errors
    ///
    /// Same as [`MultiChannelQmf::synthesize_planar`]. With zero
    /// channels this is a no-op.
    pub fn synthesize_interleaved(
        &mut self,
        channel_samples: &[&[[f64; NUM_SUBBAND]]],
        n_subs: &[usize],
        filter: FilterBankSelection,
        r_scale: f64,
        output: &mut Vec<i32>,
    ) -> Result<(), MultiChannelQmfError> {
        let channels = self.channels.len();
        self.check_lengths(channel_samples, n_subs, channels)?;
        if channels == 0 {
            return Ok(());
        }

        // Every channel produces `rows * 32` samples (verified equal by
        // check_lengths' row-count check). Synthesize each channel into
        // a scratch planar buffer, then interleave.
        let rows = channel_samples[0].len();
        let per_channel = rows * PCM_OUTPUT_PER_SAMPLE;

        let mut planar: Vec<Vec<i32>> = (0..channels)
            .map(|_| Vec::with_capacity(per_channel))
            .collect();
        for (ch, q) in self.channels.iter_mut().enumerate() {
            q.synthesize(
                channel_samples[ch],
                n_subs[ch],
                filter,
                r_scale,
                &mut planar[ch],
            )
            .map_err(|source| MultiChannelQmfError::Channel { ch, source })?;
        }

        output.reserve(channels * per_channel);
        for s in 0..per_channel {
            for plane in &planar {
                output.push(plane[s]);
            }
        }
        Ok(())
    }

    /// Convenience: drive [`MultiChannelQmf::synthesize_planar`] with
    /// the two frame-wide §C.2.5 parameters sourced directly from a
    /// parsed [`crate::DtsFrameHeader`] — `FILTS` via
    /// [`crate::DtsFrameHeader::filter_bank_selection`] and the output
    /// `rScale` via [`crate::DtsFrameHeader::output_r_scale`] (the
    /// round-335 header bridge).
    ///
    /// # Errors
    ///
    /// Returns `Ok(None)` if the header's `PCMR` code is one of the two
    /// reserved values (so [`crate::DtsFrameHeader::output_r_scale`]
    /// yields `None` and no full-scale gain is defined); otherwise
    /// runs the synthesis and returns `Ok(Some(()))`, or the same
    /// errors as [`MultiChannelQmf::synthesize_planar`].
    pub fn synthesize_planar_from_header(
        &mut self,
        header: &crate::header::DtsFrameHeader,
        channel_samples: &[&[[f64; NUM_SUBBAND]]],
        n_subs: &[usize],
        output: &mut [Vec<i32>],
    ) -> Result<Option<()>, MultiChannelQmfError> {
        let Some(r_scale) = header.output_r_scale() else {
            return Ok(None);
        };
        let filter = header.filter_bank_selection();
        self.synthesize_planar(channel_samples, n_subs, filter, r_scale, output)?;
        Ok(Some(()))
    }

    /// Validate the per-channel slice lengths and equal-row-count
    /// invariant before any per-channel synthesis runs, so a length
    /// error leaves every channel's filter state untouched.
    fn check_lengths(
        &self,
        channel_samples: &[&[[f64; NUM_SUBBAND]]],
        n_subs: &[usize],
        output_len: usize,
    ) -> Result<(), MultiChannelQmfError> {
        let channels = self.channels.len();
        if channel_samples.len() != channels {
            return Err(MultiChannelQmfError::ChannelSlicesLenMismatch {
                channels,
                got: channel_samples.len(),
            });
        }
        if n_subs.len() != channels {
            return Err(MultiChannelQmfError::NSubsLenMismatch {
                channels,
                got: n_subs.len(),
            });
        }
        if output_len != channels {
            return Err(MultiChannelQmfError::ChannelSlicesLenMismatch {
                channels,
                got: output_len,
            });
        }
        // All channels of one frame block must carry the same row count.
        if let Some(first) = channel_samples.first() {
            let expected = first.len();
            for (ch, rows) in channel_samples.iter().enumerate().skip(1) {
                if rows.len() != expected {
                    return Err(MultiChannelQmfError::RowCountMismatch {
                        expected,
                        ch,
                        got: rows.len(),
                    });
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A `channels`-channel planar synthesis must equal `channels`
    /// independent single-channel [`QmfSynthesis`] runs — the driver is
    /// a faithful per-channel composition with no cross-channel
    /// coupling.
    #[test]
    fn planar_matches_independent_single_channel_runs() {
        let channels = 3;
        let filter = FilterBankSelection::PerfectReconstruction;
        let r_scale = 32768.0;
        let n_subs = [4usize, 6, 2];

        // Distinct deterministic input per channel.
        let make_rows = |seed: usize| -> Vec<[f64; NUM_SUBBAND]> {
            (0..8)
                .map(|s| {
                    let mut row = [0.0_f64; NUM_SUBBAND];
                    for (i, slot) in row.iter_mut().enumerate() {
                        *slot = (((s * 13 + i * 7 + seed * 5) % 17) as f64 - 8.0) * 1000.0;
                    }
                    row
                })
                .collect()
        };
        let rows: Vec<Vec<[f64; NUM_SUBBAND]>> = (0..channels).map(make_rows).collect();
        let refs: Vec<&[[f64; NUM_SUBBAND]]> = rows.iter().map(|r| r.as_slice()).collect();

        // Multi-channel planar.
        let mut mc = MultiChannelQmf::new(channels);
        let mut planar: Vec<Vec<i32>> = vec![Vec::new(); channels];
        mc.synthesize_planar(&refs, &n_subs, filter, r_scale, &mut planar)
            .unwrap();

        // Independent single-channel runs.
        for ch in 0..channels {
            let mut q = QmfSynthesis::new();
            let mut expect = Vec::new();
            q.synthesize(&rows[ch], n_subs[ch], filter, r_scale, &mut expect)
                .unwrap();
            assert_eq!(planar[ch], expect, "channel {ch} mismatch");
        }
        // Non-vacuous: at least one channel produced non-zero PCM.
        assert!(planar.iter().any(|p| p.iter().any(|&s| s != 0)));
    }

    /// Interleaved output is the planar output transposed sample-major:
    /// `interleaved[s*channels + ch] == planar[ch][s]`.
    #[test]
    fn interleaved_is_planar_transposed() {
        let channels = 2;
        let filter = FilterBankSelection::NonPerfectReconstruction;
        let r_scale = 8192.0;
        let n_subs = [5usize, 3];

        let rows0: Vec<[f64; NUM_SUBBAND]> = (0..4)
            .map(|s| {
                let mut r = [0.0; NUM_SUBBAND];
                r[0] = (s as f64 + 1.0) * 1e5;
                r
            })
            .collect();
        let rows1: Vec<[f64; NUM_SUBBAND]> = (0..4)
            .map(|s| {
                let mut r = [0.0; NUM_SUBBAND];
                r[1] = (s as f64 + 1.0) * -1e5;
                r
            })
            .collect();
        let refs: Vec<&[[f64; NUM_SUBBAND]]> = vec![rows0.as_slice(), rows1.as_slice()];

        let mut mc_p = MultiChannelQmf::new(channels);
        let mut planar = vec![Vec::new(); channels];
        mc_p.synthesize_planar(&refs, &n_subs, filter, r_scale, &mut planar)
            .unwrap();

        let mut mc_i = MultiChannelQmf::new(channels);
        let mut interleaved = Vec::new();
        mc_i.synthesize_interleaved(&refs, &n_subs, filter, r_scale, &mut interleaved)
            .unwrap();

        let per_channel = planar[0].len();
        assert_eq!(interleaved.len(), channels * per_channel);
        for s in 0..per_channel {
            for (ch, plane) in planar.iter().enumerate() {
                assert_eq!(
                    interleaved[s * channels + ch],
                    plane[s],
                    "interleave mismatch at sample {s} channel {ch}"
                );
            }
        }
        assert!(interleaved.iter().any(|&s| s != 0));
    }

    /// Persisting one [`MultiChannelQmf`] across two calls equals a
    /// single call over the concatenated per-channel input — every
    /// channel's inter-frame filter tail (`raX[]`) carries across calls.
    #[test]
    fn split_calls_match_single_concatenated_call() {
        let channels = 2;
        let filter = FilterBankSelection::PerfectReconstruction;
        let r_scale = 32768.0;
        let n_subs = [6usize, 4];

        let make_rows = |seed: usize| -> Vec<[f64; NUM_SUBBAND]> {
            (0..10)
                .map(|s| {
                    let mut row = [0.0_f64; NUM_SUBBAND];
                    for (i, slot) in row.iter_mut().take(6).enumerate() {
                        *slot = (((s * 9 + i * 5 + seed) % 13) as f64 - 6.0) * 2000.0;
                    }
                    row
                })
                .collect()
        };
        let rows: Vec<Vec<[f64; NUM_SUBBAND]>> = (0..channels).map(make_rows).collect();
        let refs: Vec<&[[f64; NUM_SUBBAND]]> = rows.iter().map(|r| r.as_slice()).collect();

        // Single call over all 10 rows.
        let mut mc_single = MultiChannelQmf::new(channels);
        let mut single = vec![Vec::new(); channels];
        mc_single
            .synthesize_planar(&refs, &n_subs, filter, r_scale, &mut single)
            .unwrap();

        // Two calls split 4 + 6, reusing the same instance.
        let first: Vec<&[[f64; NUM_SUBBAND]]> = rows.iter().map(|r| &r[..4]).collect();
        let second: Vec<&[[f64; NUM_SUBBAND]]> = rows.iter().map(|r| &r[4..]).collect();
        let mut mc_split = MultiChannelQmf::new(channels);
        let mut split = vec![Vec::new(); channels];
        mc_split
            .synthesize_planar(&first, &n_subs, filter, r_scale, &mut split)
            .unwrap();
        mc_split
            .synthesize_planar(&second, &n_subs, filter, r_scale, &mut split)
            .unwrap();

        assert_eq!(single, split);
        assert!(single.iter().any(|p| p.iter().any(|&s| s != 0)));
    }

    /// Length-mismatch errors are returned before any channel's filter
    /// state is touched.
    #[test]
    fn length_mismatches_are_rejected_before_synthesis() {
        let mut mc = MultiChannelQmf::new(2);
        let rows = vec![[0.0_f64; NUM_SUBBAND]; 2];
        let refs: Vec<&[[f64; NUM_SUBBAND]]> = vec![rows.as_slice(), rows.as_slice()];

        // n_subs too short.
        let mut out = vec![Vec::new(); 2];
        assert_eq!(
            mc.synthesize_planar(
                &refs,
                &[4],
                FilterBankSelection::PerfectReconstruction,
                1.0,
                &mut out
            )
            .unwrap_err(),
            MultiChannelQmfError::NSubsLenMismatch {
                channels: 2,
                got: 1
            }
        );

        // channel-slices count wrong.
        let one: Vec<&[[f64; NUM_SUBBAND]]> = vec![rows.as_slice()];
        assert_eq!(
            mc.synthesize_planar(
                &one,
                &[4, 4],
                FilterBankSelection::PerfectReconstruction,
                1.0,
                &mut out
            )
            .unwrap_err(),
            MultiChannelQmfError::ChannelSlicesLenMismatch {
                channels: 2,
                got: 1
            }
        );

        // No output written, filters untouched.
        assert!(out.iter().all(|p| p.is_empty()));
        assert!(mc
            .channels()
            .iter()
            .all(|q| q.x_history().iter().all(|&v| v == 0.0)));
    }

    /// Channels with differing row counts are rejected (the §C.2.5
    /// outer loop runs identically for every channel of one frame).
    #[test]
    fn unequal_row_counts_are_rejected() {
        let mut mc = MultiChannelQmf::new(2);
        let rows_a = vec![[0.0_f64; NUM_SUBBAND]; 4];
        let rows_b = vec![[0.0_f64; NUM_SUBBAND]; 3];
        let refs: Vec<&[[f64; NUM_SUBBAND]]> = vec![rows_a.as_slice(), rows_b.as_slice()];
        let mut out = vec![Vec::new(); 2];
        assert_eq!(
            mc.synthesize_planar(
                &refs,
                &[4, 4],
                FilterBankSelection::NonPerfectReconstruction,
                1.0,
                &mut out
            )
            .unwrap_err(),
            MultiChannelQmfError::RowCountMismatch {
                expected: 4,
                ch: 1,
                got: 3
            }
        );
    }

    /// A zero-channel driver is a no-op for both layouts.
    #[test]
    fn zero_channels_is_a_noop() {
        let mut mc = MultiChannelQmf::new(0);
        assert_eq!(mc.channel_count(), 0);

        let mut planar: Vec<Vec<i32>> = Vec::new();
        mc.synthesize_planar(
            &[],
            &[],
            FilterBankSelection::PerfectReconstruction,
            1.0,
            &mut planar,
        )
        .unwrap();
        assert!(planar.is_empty());

        let mut interleaved = Vec::new();
        mc.synthesize_interleaved(
            &[],
            &[],
            FilterBankSelection::PerfectReconstruction,
            1.0,
            &mut interleaved,
        )
        .unwrap();
        assert!(interleaved.is_empty());
    }
}
