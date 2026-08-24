//! Canonical multichannel output ordering — ISO/IEC 14496-3 Table 1.19.
//!
//! A `raw_data_block()` lists its channel elements (SCE / CPE / LFE) in
//! **bitstream order**, and [`crate::decode::StreamDecoder`] decodes each
//! element's time signal into that same element order. For the default
//! `channelConfiguration` values 1–7 (Table 1.19) the spec fixes which
//! loudspeaker each element feeds, but the loudspeaker order is *not* the
//! order a downstream interleaved-PCM sink expects: a 5.1 decoder emits
//! its elements as `SCE(C), CPE(L,R), CPE(Ls,Rs), LFE` — speaker order
//! `[C, L, R, Ls, Rs, LFE]` — whereas the canonical interleaved layout
//! is `[L, R, C, LFE, Ls, Rs]` (the WAVE_FORMAT_EXTENSIBLE / BS.775
//! convention that [`oxideav_core::ChannelLayout::Surround51`] adopts).
//!
//! This module owns the mapping from a `channelConfiguration` to:
//!
//! * the canonical [`ChannelLayout`] it denotes ([`layout_for_config`]),
//!   and
//! * the **permutation** that reorders the element-order channel buffers
//!   into that layout's canonical order ([`reorder_permutation`]).
//!
//! ## Element → speaker mapping (Table 1.19)
//!
//! Table 1.19's "channel to speaker mapping" column, read against the
//! "audio syntactic elements, listed in order received" column, gives the
//! per-element speaker assignment used here:
//!
//! | cfg | elements (in order)            | element speaker order            |
//! |-----|--------------------------------|----------------------------------|
//! | 1   | SCE                            | `[C]`                            |
//! | 2   | CPE                            | `[L, R]`                         |
//! | 3   | SCE, CPE                       | `[C, L, R]`                      |
//! | 4   | SCE, CPE, SCE                  | `[C, L, R, Cs]`                  |
//! | 5   | SCE, CPE, CPE                  | `[C, L, R, Ls, Rs]`              |
//! | 6   | SCE, CPE, CPE, LFE             | `[C, L, R, Ls, Rs, LFE]`         |
//!
//! Each `ChannelPosition` in that element order is then matched to its
//! slot in the canonical layout (`ChannelLayout::positions()`), producing
//! the index permutation. The reorder is applied by the decode driver
//! before interleaving (see [`crate::decode`]).
//!
//! | 7   | SCE, CPE, CPE, CPE, LFE        | `[C, Lc, Rc, L, R, Ls, Rs, LFE]` |
//!
//! Config 7 is the Table 1.19 7.1 arrangement (centre + inner
//! left/right *centre front* pair + outer left/right front pair +
//! surround pair + LFE); its canonical interleave follows the same
//! WAVE/BS.775 rank order as everything else, giving
//! `[L, R, C, LFE, Lc, Rc, Ls, Rs]`. `channelConfiguration == 0`
//! (custom layout) is handled by the §8.5.2.2 PCE mapping below
//! ([`pce_speaker_assignment`] / [`pce_reorder_permutation`]), driven
//! by the `program_config_element` the decoder captured; without an
//! active PCE the driver keeps bitstream element order.
//!
//! ## Clean-room provenance
//!
//! The element list and speaker mapping are transcribed from ISO/IEC
//! 14496-3:2009 §1.6.3.5 Table 1.19. The canonical interleaved order is
//! the WAVE_FORMAT_EXTENSIBLE / ITU-R BS.775 convention already encoded
//! in [`oxideav_core::ChannelLayout`].

use crate::pce::{ElementSelect, Pce};
use oxideav_core::{ChannelLayout, ChannelPosition};

/// The canonical [`ChannelLayout`] denoted by a Table 1.19
/// `channelConfiguration`, for the default values this crate reorders
/// (1–6). Returns `None` for `0` (PCE-defined), `7` (amendment-specific
/// 7.1), and any reserved value `≥ 8`.
#[must_use]
pub fn layout_for_config(channel_configuration: u8) -> Option<ChannelLayout> {
    Some(match channel_configuration {
        1 => ChannelLayout::Mono,
        2 => ChannelLayout::Stereo,
        3 => ChannelLayout::Surround30,
        4 => ChannelLayout::Surround40,
        5 => ChannelLayout::Surround50,
        6 => ChannelLayout::Surround51,
        _ => return None,
    })
}

/// The Table 1.19 per-element speaker order for a default
/// `channelConfiguration` — the loudspeaker each decoded channel feeds,
/// in the order the elements appear in the `raw_data_block()`.
///
/// Returns `None` for `0` (PCE-defined — see
/// [`pce_speaker_assignment`]) and reserved values.
#[must_use]
pub fn element_speaker_order(channel_configuration: u8) -> Option<&'static [ChannelPosition]> {
    use ChannelPosition::*;
    Some(match channel_configuration {
        1 => &[FrontCenter],
        2 => &[FrontLeft, FrontRight],
        3 => &[FrontCenter, FrontLeft, FrontRight],
        4 => &[FrontCenter, FrontLeft, FrontRight, BackCenter],
        5 => &[FrontCenter, FrontLeft, FrontRight, SideLeft, SideRight],
        6 => &[
            FrontCenter,
            FrontLeft,
            FrontRight,
            SideLeft,
            SideRight,
            LowFrequency,
        ],
        // Table 1.19 value 7 — 7+1: centre front; left, right CENTRE
        // front (the inner pair); left, right OUTSIDE front; left,
        // right surround rear (the same surround wording as configs
        // 5/6, mapped to the side-surround positions this crate uses
        // there); LFE.
        7 => &[
            FrontCenter,
            FrontLeftOfCenter,
            FrontRightOfCenter,
            FrontLeft,
            FrontRight,
            SideLeft,
            SideRight,
            LowFrequency,
        ],
        _ => return None,
    })
}

/// The permutation that reorders element-order channel buffers into the
/// canonical [`ChannelLayout`] order for a default `channelConfiguration`.
///
/// The returned vector `perm` has one entry per output channel: output
/// slot `i` (in canonical layout order) is sourced from element-order
/// channel `perm[i]`. Applying it is `out[i] = channels[perm[i]]`.
///
/// Returns `None` when no reordering is defined for this configuration
/// (`0` — PCE-defined — and reserved values); the caller keeps the
/// bitstream element order. An identity permutation (configs 1 and 2,
/// where element order already matches the canonical order) is
/// returned as `Some(vec![0, 1, …])` so the caller can still validate
/// the channel count.
#[must_use]
pub fn reorder_permutation(channel_configuration: u8) -> Option<Vec<usize>> {
    let element_order = element_speaker_order(channel_configuration)?;
    // Sort the element-order channels by their canonical WAVE/BS.775
    // interleave rank. For configs 1–6 this reproduces exactly the
    // `ChannelLayout::positions()` order of `layout_for_config` (the
    // named layouts list their speakers in mask order); config 7 has
    // no named `ChannelLayout` but ranks the same way.
    let mut perm: Vec<usize> = (0..element_order.len()).collect();
    let ranks: Vec<usize> = element_order
        .iter()
        .map(|&p| canonical_rank(p))
        .collect::<Option<Vec<usize>>>()?;
    perm.sort_by_key(|&i| ranks[i]);
    Some(perm)
}

/// Apply [`reorder_permutation`] to a set of element-order channel
/// buffers, returning the reordered set. When no permutation is defined
/// for `channel_configuration`, or the channel count does not match the
/// permutation length, the input order is preserved (returned unchanged).
///
/// This is the entry point the decode driver calls once a frame's
/// element-order channels are assembled.
#[must_use]
pub fn reorder_channels<T>(channel_configuration: u8, channels: Vec<Vec<T>>) -> Vec<Vec<T>> {
    let Some(perm) = reorder_permutation(channel_configuration) else {
        return channels;
    };
    if perm.len() != channels.len() {
        // Element count disagrees with the signalled configuration (a
        // malformed or PCE-overridden stream); leave the order untouched
        // rather than drop or duplicate a channel.
        return channels;
    }
    // `perm[i]` is the source slot for output slot `i`.
    apply_permutation(&perm, channels)
}

// ===== PCE-defined layouts (`channelConfiguration == 0`) =====
//
// ISO/IEC 13818-7 §8.5.2.2 (the PCE channel-configuration rules the
// 14496-3 GA payload inherits): the PCE carries a *list of front
// channels* "using the rule center outwards, left before right" (a
// center-channel SCE first, other SCEs in L/R pairs), then a list of
// *side channels* (CPEs or SCE pairs) "in the order of front to
// back", then a list of *back channels* "listed from outside in"
// (SCEs paired except that a final unpaired SCE is the rear center),
// then the LFE list. Each list references its elements by
// `*_element_is_cpe` + `*_element_tag_select`, so the mapping is by
// (element kind, instance tag), independent of the order the elements
// appear in the `raw_data_block()`.

/// Which channel-element type a PCE list entry (or a decoded element)
/// is — the key half of the PCE (kind, tag) element reference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PceElementKind {
    /// `single_channel_element()`.
    Sce,
    /// `channel_pair_element()`.
    Cpe,
    /// `lfe_channel_element()`.
    Lfe,
}

/// Canonical interleave rank of a [`ChannelPosition`] — the
/// WAVE_FORMAT_EXTENSIBLE / BS.775 speaker-mask bit order this crate's
/// default-config reorder already targets. Lower rank interleaves
/// first.
fn canonical_rank(pos: ChannelPosition) -> Option<usize> {
    use ChannelPosition::*;
    Some(match pos {
        FrontLeft => 0,
        FrontRight => 1,
        FrontCenter => 2,
        LowFrequency => 3,
        BackLeft => 4,
        BackRight => 5,
        FrontLeftOfCenter => 6,
        FrontRightOfCenter => 7,
        BackCenter => 8,
        SideLeft => 9,
        SideRight => 10,
        _ => return None,
    })
}

/// One PCE-addressed element with its speaker assignment: the
/// `(kind, instance tag)` reference and the position(s) its decoded
/// channel(s) feed, in the element's own channel order (`[left,
/// right]` for a CPE).
type PceAssignment = (PceElementKind, u8, Vec<ChannelPosition>);

/// Group a PCE element list into L/R pairs plus at most one unpaired
/// (center) SCE, preserving list order. CPEs are pairs by
/// construction; consecutive SCEs pair up left-then-right
/// (§8.5.2.2). Returns `(pairs, lone_sce_tag)` where each pair is
/// two `(is_cpe, tag)` halves (both halves of a CPE share its tag),
/// or `None` when the list leaves half an SCE pair over (an
/// ambiguous layout this crate leaves in element order).
#[allow(clippy::type_complexity)]
fn pair_up(list: &[ElementSelect], lone_first: bool) -> Option<(Vec<[(bool, u8); 2]>, Option<u8>)> {
    let sce_count = list.iter().filter(|e| !e.is_cpe).count();
    // At most one SCE can be unpaired; §8.5.2.2 puts a front center
    // first, while the back list's lone SCE (rear center) is last.
    // Encoders are seen emitting the front center *last* too, so the
    // rule keyed here is simply the parity: an odd SCE count means
    // exactly one lone (center) SCE, taken at the position
    // `lone_first` prefers when there is a choice.
    let mut lone: Option<u8> = None;
    let mut expect_lone = sce_count % 2 == 1;
    let mut pairs: Vec<[(bool, u8); 2]> = Vec::new();
    let mut pending_sce: Option<u8> = None;
    let sce_positions: Vec<usize> = (0..list.len()).filter(|&i| !list[i].is_cpe).collect();
    let lone_index = if expect_lone {
        if lone_first {
            sce_positions.first().copied()
        } else {
            sce_positions.last().copied()
        }
    } else {
        None
    };
    for (i, e) in list.iter().enumerate() {
        if e.is_cpe {
            pairs.push([(true, e.tag_select), (true, e.tag_select)]);
        } else if expect_lone && Some(i) == lone_index {
            lone = Some(e.tag_select);
            expect_lone = false;
        } else if let Some(left) = pending_sce.take() {
            pairs.push([(false, left), (false, e.tag_select)]);
        } else {
            pending_sce = Some(e.tag_select);
        }
    }
    if pending_sce.is_some() {
        return None; // half an SCE pair left over
    }
    Some((pairs, lone))
}

/// Push one L/R pair's two assignment halves.
fn push_pair(
    out: &mut Vec<PceAssignment>,
    pair: [(bool, u8); 2],
    left: ChannelPosition,
    right: ChannelPosition,
) {
    let [(l_cpe, l_tag), (r_cpe, r_tag)] = pair;
    if l_cpe {
        // One CPE carries both halves.
        debug_assert!(r_cpe && l_tag == r_tag);
        out.push((PceElementKind::Cpe, l_tag, vec![left, right]));
    } else {
        out.push((PceElementKind::Sce, l_tag, vec![left]));
        out.push((PceElementKind::Sce, r_tag, vec![right]));
    }
}

/// Derive the §8.5.2.2 element→speaker assignment of a PCE-defined
/// layout.
///
/// Returns `None` (caller keeps bitstream element order) for layouts
/// this crate cannot express in canonical positions: more than two
/// front pairs, more than one side pair, more than two back pairs,
/// more than one LFE, or a list shape §8.5.2.2 does not describe.
///
/// Position choices, mirroring Table 42's named speakers:
///
/// * front: the lone SCE (odd SCE count) is the front center; one
///   pair is the ordinary L/R; with two pairs, the first-listed
///   (inner — "center outwards") pair is the left/right *center*
///   front (`FrontLeftOfCenter` / `FrontRightOfCenter`) and the
///   second the outside L/R (the Table 42 index-7 arrangement).
/// * side: a single pair is the side surround `SideLeft`/`SideRight`.
/// * back: with two pairs ("listed from outside in") the first is
///   the side-most surround pair (`SideLeft`/`SideRight`) and the
///   second the rear `BackLeft`/`BackRight`; a single pair is the
///   rear `BackLeft`/`BackRight` when something else fixes the side
///   image (a side pair or a rear-center SCE), else the
///   `SideLeft`/`SideRight` surround pair of the 5.1-style layouts
///   (matching this crate's Table 1.19 config-5/6 mapping); a final
///   unpaired SCE is the `BackCenter`.
/// * every LFE-list entry is `LowFrequency` (at most one).
pub fn pce_speaker_assignment(pce: &Pce) -> Option<Vec<PceAssignment>> {
    use ChannelPosition::*;
    let mut out: Vec<PceAssignment> = Vec::new();

    // Front list: center outwards.
    let (front_pairs, front_center) = pair_up(&pce.front_elements, true)?;
    if let Some(tag) = front_center {
        out.push((PceElementKind::Sce, tag, vec![FrontCenter]));
    }
    match front_pairs.len() {
        0 => {}
        1 => push_pair(&mut out, front_pairs[0], FrontLeft, FrontRight),
        2 => {
            push_pair(
                &mut out,
                front_pairs[0],
                FrontLeftOfCenter,
                FrontRightOfCenter,
            );
            push_pair(&mut out, front_pairs[1], FrontLeft, FrontRight);
        }
        _ => return None,
    }

    // Side list: front to back; only one distinct side position pair.
    let (side_pairs, side_lone) = pair_up(&pce.side_elements, false)?;
    if side_lone.is_some() || side_pairs.len() > 1 {
        return None;
    }
    let have_side = side_pairs.len() == 1;
    if have_side {
        push_pair(&mut out, side_pairs[0], SideLeft, SideRight);
    }

    // Back list: outside in; a final lone SCE is the rear center.
    let (back_pairs, back_center) = pair_up(&pce.back_elements, false)?;
    match back_pairs.len() {
        0 => {}
        1 => {
            if have_side || back_center.is_some() {
                push_pair(&mut out, back_pairs[0], BackLeft, BackRight);
            } else {
                // The single surround pair of a 5.1-style layout —
                // the same SideLeft/SideRight this crate's Table 1.19
                // config-5/6 mapping uses.
                push_pair(&mut out, back_pairs[0], SideLeft, SideRight);
            }
        }
        2 => {
            if have_side {
                return None; // three distinct surround pairs
            }
            push_pair(&mut out, back_pairs[0], SideLeft, SideRight);
            push_pair(&mut out, back_pairs[1], BackLeft, BackRight);
        }
        _ => return None,
    }
    if let Some(tag) = back_center {
        out.push((PceElementKind::Sce, tag, vec![BackCenter]));
    }

    // LFE list.
    match pce.lfe_element_tag_selects.len() {
        0 => {}
        1 => out.push((
            PceElementKind::Lfe,
            pce.lfe_element_tag_selects[0],
            vec![LowFrequency],
        )),
        _ => return None, // §8.5.2.3: no mapping for multiple LFEs
    }

    // Every position must be distinct (and canonical-rankable).
    let mut seen = [false; 11];
    for (_, _, positions) in &out {
        for &p in positions {
            let r = canonical_rank(p)?;
            if seen[r] {
                return None;
            }
            seen[r] = true;
        }
    }
    Some(out)
}

/// The permutation that reorders a PCE-defined frame's element-order
/// channel buffers into canonical interleave order.
///
/// `elements` describes the decoded frame in bitstream order: one
/// `(kind, instance tag, channel count)` triple per channel element.
/// Every element must be referenced by the PCE exactly once with a
/// matching channel count, and the PCE's whole audio-element set must
/// appear in the frame; otherwise `None` is returned and the caller
/// keeps element order.
pub fn pce_reorder_permutation(
    pce: &Pce,
    elements: &[(PceElementKind, u8, usize)],
) -> Option<Vec<usize>> {
    let mut assignment = pce_speaker_assignment(pce)?;
    // Per decoded channel (element order): its canonical rank.
    let mut ranks: Vec<usize> = Vec::new();
    for &(kind, tag, n_ch) in elements {
        let idx = assignment
            .iter()
            .position(|&(k, t, _)| k == kind && t == tag)?;
        let (_, _, positions) = assignment.swap_remove(idx);
        if positions.len() != n_ch {
            return None; // e.g. a PS-widened SCE — keep element order
        }
        for p in positions {
            ranks.push(canonical_rank(p)?);
        }
    }
    if !assignment.is_empty() {
        return None; // PCE promises channels the frame did not carry
    }
    // Output slot i takes the source channel with the i-th smallest
    // rank. Ranks are distinct by construction.
    let mut perm: Vec<usize> = (0..ranks.len()).collect();
    perm.sort_by_key(|&i| ranks[i]);
    Some(perm)
}

/// Apply a permutation produced by [`pce_reorder_permutation`] to a
/// set of element-order channel buffers (same contract as
/// [`reorder_channels`]: `out[i] = channels[perm[i]]`).
#[must_use]
pub fn apply_permutation<T>(perm: &[usize], channels: Vec<Vec<T>>) -> Vec<Vec<T>> {
    if perm.len() != channels.len() {
        return channels;
    }
    let mut slots: Vec<Option<Vec<T>>> = channels.into_iter().map(Some).collect();
    let mut out = Vec::with_capacity(perm.len());
    for &src in perm {
        out.push(
            slots[src]
                .take()
                .expect("permutation is a bijection over the channel slots"),
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxideav_core::ChannelPosition::*;

    #[test]
    fn mono_and_stereo_are_identity() {
        assert_eq!(reorder_permutation(1), Some(vec![0]));
        assert_eq!(reorder_permutation(2), Some(vec![0, 1]));
    }

    #[test]
    fn surround30_moves_center_to_third_slot() {
        // element order [C, L, R] -> canonical [L, R, C]
        assert_eq!(reorder_permutation(3), Some(vec![1, 2, 0]));
    }

    #[test]
    fn surround40_keeps_back_center_last() {
        // element order [C, L, R, Cs] -> canonical [L, R, C, Cs]
        assert_eq!(reorder_permutation(4), Some(vec![1, 2, 0, 3]));
    }

    #[test]
    fn surround50_orders_front_then_surround() {
        // element order [C, L, R, Ls, Rs] -> canonical [L, R, C, Ls, Rs]
        assert_eq!(reorder_permutation(5), Some(vec![1, 2, 0, 3, 4]));
    }

    #[test]
    fn surround51_interleaves_lfe_before_surround() {
        // element order [C, L, R, Ls, Rs, LFE] -> canonical
        // [L, R, C, LFE, Ls, Rs]
        assert_eq!(reorder_permutation(6), Some(vec![1, 2, 0, 5, 3, 4]));
    }

    #[test]
    fn config_zero_and_reserved_are_unmapped() {
        assert_eq!(reorder_permutation(0), None);
        assert_eq!(reorder_permutation(8), None);
        assert_eq!(reorder_permutation(15), None);
        assert_eq!(layout_for_config(0), None);
        // Config 7 reorders but denotes no named core layout.
        assert_eq!(layout_for_config(7), None);
    }

    #[test]
    fn config_seven_lands_wave_rank_order() {
        // element order [C, Lc, Rc, L, R, Ls, Rs, LFE] → canonical
        // [L, R, C, LFE, Lc, Rc, Ls, Rs] (WAVE mask rank order).
        assert_eq!(reorder_permutation(7), Some(vec![3, 4, 0, 7, 1, 2, 5, 6]));
    }

    #[test]
    fn permutation_matches_layout_positions() {
        // The permutation must land each element on the layout slot whose
        // ChannelPosition equals the element's Table 1.19 speaker.
        for cfg in 1..=6u8 {
            let perm = reorder_permutation(cfg).unwrap();
            let elem = element_speaker_order(cfg).unwrap();
            let layout = layout_for_config(cfg).unwrap();
            let canonical = layout.positions();
            assert_eq!(perm.len(), canonical.len(), "cfg {cfg} length");
            assert_eq!(canonical.len(), elem.len(), "cfg {cfg} element count");
            for (out_slot, &src) in perm.iter().enumerate() {
                assert_eq!(
                    elem[src], canonical[out_slot],
                    "cfg {cfg}: output slot {out_slot} mismatched speaker"
                );
            }
        }
    }

    #[test]
    fn layout_channel_counts_agree_with_element_order() {
        for cfg in 1..=6u8 {
            let layout = layout_for_config(cfg).unwrap();
            let elem = element_speaker_order(cfg).unwrap();
            assert_eq!(
                usize::from(layout.channel_count()),
                elem.len(),
                "cfg {cfg} channel count"
            );
        }
    }

    #[test]
    fn reorder_channels_permutes_buffers() {
        // 5.1 element order [C, L, R, Ls, Rs, LFE] tagged by a sentinel
        // sample so we can see where each lands.
        let channels: Vec<Vec<i16>> = vec![
            vec![0], // C
            vec![1], // L
            vec![2], // R
            vec![3], // Ls
            vec![4], // Rs
            vec![5], // LFE
        ];
        let out = reorder_channels(6, channels);
        // canonical [L, R, C, LFE, Ls, Rs] = [1, 2, 0, 5, 3, 4]
        let got: Vec<i16> = out.iter().map(|c| c[0]).collect();
        assert_eq!(got, vec![1, 2, 0, 5, 3, 4]);
    }

    #[test]
    fn reorder_channels_passthrough_on_unmapped_config() {
        let channels: Vec<Vec<i16>> = vec![vec![9], vec![8]];
        let out = reorder_channels(0, channels.clone());
        assert_eq!(out, channels);
    }

    #[test]
    fn reorder_channels_passthrough_on_count_mismatch() {
        // cfg 6 expects 6 channels; a 4-channel input is left untouched.
        let channels: Vec<Vec<i16>> = vec![vec![0], vec![1], vec![2], vec![3]];
        let out = reorder_channels(6, channels.clone());
        assert_eq!(out, channels);
    }

    // ===== §8.5.2.2 PCE-defined layouts =====

    fn sce(tag: u8) -> ElementSelect {
        ElementSelect {
            is_cpe: false,
            tag_select: tag,
        }
    }
    fn cpe(tag: u8) -> ElementSelect {
        ElementSelect {
            is_cpe: true,
            tag_select: tag,
        }
    }
    fn pce_with(
        front: Vec<ElementSelect>,
        side: Vec<ElementSelect>,
        back: Vec<ElementSelect>,
        lfe: Vec<u8>,
    ) -> Pce {
        Pce {
            element_instance_tag: 0,
            object_type: 1,
            sampling_frequency_index: 3,
            front_elements: front,
            side_elements: side,
            back_elements: back,
            lfe_element_tag_selects: lfe,
            assoc_data_tag_selects: vec![],
            valid_cc_elements: vec![],
            mono_mixdown_element_number: None,
            stereo_mixdown_element_number: None,
            matrix_mixdown: None,
            comment_field: vec![],
        }
    }

    #[test]
    fn pce_5_1_matches_config_6_order() {
        // front [SCE0(C), CPE0(L/R)], back [CPE1(Ls/Rs)], lfe [0] —
        // the PCE spelling of the Table 1.19 config-6 layout. Element
        // order SCE, CPE0, CPE1, LFE must permute exactly like
        // config 6: [L, R, C, LFE, Ls, Rs].
        let pce = pce_with(vec![sce(0), cpe(0)], vec![], vec![cpe(1)], vec![0]);
        use PceElementKind::*;
        let perm =
            pce_reorder_permutation(&pce, &[(Sce, 0, 1), (Cpe, 0, 2), (Cpe, 1, 2), (Lfe, 0, 1)])
                .expect("5.1 PCE maps");
        assert_eq!(perm, vec![1, 2, 0, 5, 3, 4]);
    }

    #[test]
    fn pce_7_1_two_back_pairs_outside_in() {
        // The staged 7.1 fixture's PCE shape: front [SCE0, CPE0],
        // back [CPE1, CPE2] ("outside in": CPE1 the side-most
        // surround pair, CPE2 the rear pair), lfe [0]. Element order
        // SCE, CPE0, CPE1, CPE2, LFE → canonical
        // [FL FR FC LFE BL BR SL SR] =
        // [Cpe0.l, Cpe0.r, Sce, Lfe, Cpe2.l, Cpe2.r, Cpe1.l, Cpe1.r].
        let pce = pce_with(vec![sce(0), cpe(0)], vec![], vec![cpe(1), cpe(2)], vec![0]);
        use PceElementKind::*;
        let perm = pce_reorder_permutation(
            &pce,
            &[
                (Sce, 0, 1),
                (Cpe, 0, 2),
                (Cpe, 1, 2),
                (Cpe, 2, 2),
                (Lfe, 0, 1),
            ],
        )
        .expect("7.1 PCE maps");
        assert_eq!(perm, vec![1, 2, 0, 7, 5, 6, 3, 4]);
    }

    #[test]
    fn pce_hexagonal_lone_sces_are_centers() {
        // The staged hexagonal fixture's PCE: front [CPE0, SCE0]
        // (the lone front SCE is the center wherever it is listed),
        // back [CPE1, SCE1] (a final unpaired back SCE is the rear
        // center — §8.5.2.2). Element order CPE0, SCE0, CPE1, SCE1 →
        // canonical [FL FR FC BL BR BC].
        let pce = pce_with(vec![cpe(0), sce(0)], vec![], vec![cpe(1), sce(1)], vec![]);
        use PceElementKind::*;
        let perm =
            pce_reorder_permutation(&pce, &[(Cpe, 0, 2), (Sce, 0, 1), (Cpe, 1, 2), (Sce, 1, 1)])
                .expect("hexagonal PCE maps");
        assert_eq!(perm, vec![0, 1, 2, 3, 4, 5], "already canonical order");

        // The same layout with the block elements in a different
        // order still lands canonically (mapping is by (kind, tag)).
        let perm =
            pce_reorder_permutation(&pce, &[(Sce, 1, 1), (Sce, 0, 1), (Cpe, 1, 2), (Cpe, 0, 2)])
                .unwrap();
        // element-order channels: [BC, FC, BL, BR, FL, FR] →
        // canonical FL FR FC BL BR BC = sources [4, 5, 1, 2, 3, 0].
        assert_eq!(perm, vec![4, 5, 1, 2, 3, 0]);
    }

    #[test]
    fn pce_sce_pair_forms_lr() {
        // Two SCEs in the front list (even count) form one L/R pair.
        let pce = pce_with(vec![sce(0), sce(1)], vec![], vec![], vec![]);
        let assign = pce_speaker_assignment(&pce).unwrap();
        assert_eq!(
            assign,
            vec![
                (PceElementKind::Sce, 0, vec![FrontLeft]),
                (PceElementKind::Sce, 1, vec![FrontRight]),
            ]
        );
    }

    #[test]
    fn pce_side_pair_moves_single_back_pair_to_rear() {
        // side [CPE1] + back [CPE2]: the back pair is the rear
        // BL/BR (the side pair holds SL/SR).
        let pce = pce_with(vec![sce(0), cpe(0)], vec![cpe(1)], vec![cpe(2)], vec![]);
        let assign = pce_speaker_assignment(&pce).unwrap();
        let find = |tag: u8| {
            assign
                .iter()
                .find(|&&(k, t, _)| k == PceElementKind::Cpe && t == tag)
                .map(|(_, _, p)| p.clone())
                .unwrap()
        };
        assert_eq!(find(1), vec![SideLeft, SideRight]);
        assert_eq!(find(2), vec![BackLeft, BackRight]);
    }

    #[test]
    fn pce_unmappable_layouts_fall_back() {
        // Three front pairs: no canonical positions — None.
        let pce = pce_with(vec![cpe(0), cpe(1), cpe(2)], vec![], vec![], vec![]);
        assert!(pce_speaker_assignment(&pce).is_none());
        // Two LFEs: §8.5.2.3 defines no mapping.
        let pce = pce_with(vec![sce(0), cpe(0)], vec![], vec![], vec![0, 1]);
        assert!(pce_speaker_assignment(&pce).is_none());
    }

    #[test]
    fn pce_permutation_rejects_mismatches() {
        use PceElementKind::*;
        let pce = pce_with(vec![sce(0), cpe(0)], vec![], vec![], vec![]);
        // Channel-count mismatch (a PS-widened SCE): None.
        assert!(pce_reorder_permutation(&pce, &[(Sce, 0, 2), (Cpe, 0, 2)]).is_none());
        // An element the PCE does not reference: None.
        assert!(pce_reorder_permutation(&pce, &[(Sce, 0, 1), (Cpe, 0, 2), (Cpe, 5, 2)]).is_none());
        // A referenced element missing from the frame: None.
        assert!(pce_reorder_permutation(&pce, &[(Sce, 0, 1)]).is_none());
    }

    #[test]
    fn every_speaker_in_canonical_appears_in_element_order() {
        // Guards the bijection assumption reorder_channels relies on.
        for cfg in 1..=6u8 {
            let elem = element_speaker_order(cfg).unwrap();
            let layout = layout_for_config(cfg).unwrap();
            for &pos in layout.positions() {
                assert!(
                    elem.contains(&pos),
                    "cfg {cfg}: canonical speaker {pos:?} missing from element order"
                );
            }
        }
        // Sanity: a position only present in a higher layout is absent.
        let elem5 = element_speaker_order(5).unwrap();
        assert!(!elem5.contains(&LowFrequency));
    }
}
