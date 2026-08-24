//! Custom Channel Map (Table E2.5 — `chanmap` field) decoder.
//!
//! The 16-bit `chanmap` field is emitted on dependent substreams when
//! `chanmape == 1` (§E.2.3.1.7-8). It assigns each coded channel of
//! the dep substream to a fixed channel location drawn from a 16-slot
//! reference grid (Table E2.5). The MSB of the field is bit 0
//! ("Left") and the LSB is bit 15 ("LFE").
//!
//! Per spec (§E.2.3.1.8):
//!
//! > Bit 0, which indicates the presence of the left channel, is
//! > stored in the most significant bit of the chanmap field. For
//! > each channel present in the dependent substream, the
//! > corresponding location bit in the chanmap is set to '1'. The
//! > order of the coded channels in the dependent substream is the
//! > same as the order of the enabled location bits in the chanmap.
//! > […] When the enabled location bit in the chanmap field refers
//! > to a pair of channels, this defines the channel location of two
//! > adjacent channels in the dependent substream.
//!
//! The pair-bits (Table E2.5 entries that expand to **two** adjacent
//! channels) are bits 5, 6, 9, 10, 11, and 13.
//!
//! The spec constraint "the number of channel locations indicated by
//! the chanmap field must equal the total number of coded channels
//! present in the dependent substream, as indicated by the acmod and
//! lfeon bit stream parameters" is enforced by
//! [`expand_chanmap_locations`] returning [`ChanmapError::CountMismatch`]
//! when the expanded count does not match `dep_nchans`.
//!
//! This module is consumed by [`crate::eac3::decoder`] when splicing a
//! dependent substream's PCM into the independent substream's program.

/// One physical channel-location slot per Table E2.5.
///
/// The numeric value is the bit index in the 16-bit `chanmap` field
/// (NOT the bit weight); pair-bits expand to two distinct enum
/// variants in the order specified by the spec text ("first coded
/// channel is the Left Surround channel, the second coded channel
/// is the Right Surround channel"). For pair bit 6 ("Lrs/Rrs pair")
/// this yields [`ChannelLocation::LeftRearSurround`] then
/// [`ChannelLocation::RightRearSurround`]; for bit 9 ("Lsd/Rsd
/// pair") this yields [`ChannelLocation::LeftSurroundDirect`] then
/// [`ChannelLocation::RightSurroundDirect`]; etc.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ChannelLocation {
    /// Bit 0 — Left.
    Left,
    /// Bit 1 — Center.
    Center,
    /// Bit 2 — Right.
    Right,
    /// Bit 3 — Left Surround.
    LeftSurround,
    /// Bit 4 — Right Surround.
    RightSurround,
    /// Bit 5 (pair) — left half of Lc/Rc.
    LeftCenter,
    /// Bit 5 (pair) — right half of Lc/Rc.
    RightCenter,
    /// Bit 6 (pair) — left half of Lrs/Rrs (left rear surround).
    LeftRearSurround,
    /// Bit 6 (pair) — right half of Lrs/Rrs (right rear surround).
    RightRearSurround,
    /// Bit 7 — Cs (center surround).
    CenterSurround,
    /// Bit 8 — Ts (top surround).
    TopSurround,
    /// Bit 9 (pair) — left half of Lsd/Rsd (left surround direct).
    LeftSurroundDirect,
    /// Bit 9 (pair) — right half of Lsd/Rsd (right surround direct).
    RightSurroundDirect,
    /// Bit 10 (pair) — left half of Lw/Rw.
    LeftWide,
    /// Bit 10 (pair) — right half of Lw/Rw.
    RightWide,
    /// Bit 11 (pair) — left half of Vhl/Vhr (vertical-height left).
    VerticalHeightLeft,
    /// Bit 11 (pair) — right half of Vhl/Vhr.
    VerticalHeightRight,
    /// Bit 12 — Vhc (vertical-height center).
    VerticalHeightCenter,
    /// Bit 13 (pair) — left half of Lts/Rts (top surround left).
    TopSurroundLeft,
    /// Bit 13 (pair) — right half of Lts/Rts.
    TopSurroundRight,
    /// Bit 14 — LFE2 (second low-frequency effect).
    Lfe2,
    /// Bit 15 — LFE.
    Lfe,
}

impl ChannelLocation {
    /// Every [`ChannelLocation`] variant, in Table E2.5 bit order
    /// (bit 0 → bit 15) with each pair-bit expanded to its two halves
    /// in the spec's documented left-then-right order. Lets a consumer
    /// iterate the full reference grid without re-deriving the variant
    /// list (e.g. building a lookup from a physical speaker position
    /// back to its [`ChannelLocation`]).
    pub const ALL: [ChannelLocation; 22] = [
        ChannelLocation::Left,
        ChannelLocation::Center,
        ChannelLocation::Right,
        ChannelLocation::LeftSurround,
        ChannelLocation::RightSurround,
        ChannelLocation::LeftCenter,
        ChannelLocation::RightCenter,
        ChannelLocation::LeftRearSurround,
        ChannelLocation::RightRearSurround,
        ChannelLocation::CenterSurround,
        ChannelLocation::TopSurround,
        ChannelLocation::LeftSurroundDirect,
        ChannelLocation::RightSurroundDirect,
        ChannelLocation::LeftWide,
        ChannelLocation::RightWide,
        ChannelLocation::VerticalHeightLeft,
        ChannelLocation::VerticalHeightRight,
        ChannelLocation::VerticalHeightCenter,
        ChannelLocation::TopSurroundLeft,
        ChannelLocation::TopSurroundRight,
        ChannelLocation::Lfe2,
        ChannelLocation::Lfe,
    ];

    /// The Table E2.5 location bit (`0..=15`) this variant was decoded
    /// from. Each of the six pair-bits (5, 6, 9, 10, 11, 13) maps both
    /// of its expanded halves to the same shared bit — e.g. both
    /// [`Self::LeftRearSurround`] and [`Self::RightRearSurround`] return
    /// `6` (the "Lrs/Rrs pair" row). This is the inverse of the
    /// [`expand_chanmap_locations`] decode: a consumer that wants to
    /// re-emit a `chanmap` field can OR together
    /// `1 << (15 - loc.table_e2_5_bit())` over the location list.
    pub fn table_e2_5_bit(self) -> u8 {
        match self {
            ChannelLocation::Left => 0,
            ChannelLocation::Center => 1,
            ChannelLocation::Right => 2,
            ChannelLocation::LeftSurround => 3,
            ChannelLocation::RightSurround => 4,
            ChannelLocation::LeftCenter | ChannelLocation::RightCenter => 5,
            ChannelLocation::LeftRearSurround | ChannelLocation::RightRearSurround => 6,
            ChannelLocation::CenterSurround => 7,
            ChannelLocation::TopSurround => 8,
            ChannelLocation::LeftSurroundDirect | ChannelLocation::RightSurroundDirect => 9,
            ChannelLocation::LeftWide | ChannelLocation::RightWide => 10,
            ChannelLocation::VerticalHeightLeft | ChannelLocation::VerticalHeightRight => 11,
            ChannelLocation::VerticalHeightCenter => 12,
            ChannelLocation::TopSurroundLeft | ChannelLocation::TopSurroundRight => 13,
            ChannelLocation::Lfe2 => 14,
            ChannelLocation::Lfe => 15,
        }
    }

    /// The 16-bit `chanmap` field weight for this location's Table E2.5
    /// bit — `1 << (15 - table_e2_5_bit())`. Bit 0 (Left) lives in the
    /// MSB per §E.2.3.1.8, so the weight of bit 0 is `0x8000` and the
    /// weight of bit 15 (LFE) is `0x0001`. Both halves of a pair-bit
    /// share the same weight (the single set bit that expanded to two
    /// channels).
    pub fn chanmap_weight(self) -> u16 {
        1u16 << (15 - self.table_e2_5_bit())
    }

    /// `true` when this location is one half of a Table E2.5 pair-bit
    /// (bits 5, 6, 9, 10, 11, 13 — `Lc/Rc`, `Lrs/Rrs`, `Lsd/Rsd`,
    /// `Lw/Rw`, `Vhl/Vhr`, `Lts/Rts`). A single set pair-bit decodes to
    /// two adjacent coded channels per §E.2.3.1.8, so a consumer that
    /// re-emits a `chanmap` must set the shared bit exactly once for the
    /// two halves rather than once each.
    pub fn is_pair_half(self) -> bool {
        matches!(
            self,
            ChannelLocation::LeftCenter
                | ChannelLocation::RightCenter
                | ChannelLocation::LeftRearSurround
                | ChannelLocation::RightRearSurround
                | ChannelLocation::LeftSurroundDirect
                | ChannelLocation::RightSurroundDirect
                | ChannelLocation::LeftWide
                | ChannelLocation::RightWide
                | ChannelLocation::VerticalHeightLeft
                | ChannelLocation::VerticalHeightRight
                | ChannelLocation::TopSurroundLeft
                | ChannelLocation::TopSurroundRight
        )
    }

    /// The companion half of a Table E2.5 pair-bit location, or `None`
    /// for a single-channel location. For [`Self::LeftRearSurround`]
    /// this returns [`Self::RightRearSurround`] and vice-versa — letting
    /// a consumer pair up the two adjacent coded channels a single set
    /// pair-bit expanded to.
    pub fn pair_companion(self) -> Option<ChannelLocation> {
        Some(match self {
            ChannelLocation::LeftCenter => ChannelLocation::RightCenter,
            ChannelLocation::RightCenter => ChannelLocation::LeftCenter,
            ChannelLocation::LeftRearSurround => ChannelLocation::RightRearSurround,
            ChannelLocation::RightRearSurround => ChannelLocation::LeftRearSurround,
            ChannelLocation::LeftSurroundDirect => ChannelLocation::RightSurroundDirect,
            ChannelLocation::RightSurroundDirect => ChannelLocation::LeftSurroundDirect,
            ChannelLocation::LeftWide => ChannelLocation::RightWide,
            ChannelLocation::RightWide => ChannelLocation::LeftWide,
            ChannelLocation::VerticalHeightLeft => ChannelLocation::VerticalHeightRight,
            ChannelLocation::VerticalHeightRight => ChannelLocation::VerticalHeightLeft,
            ChannelLocation::TopSurroundLeft => ChannelLocation::TopSurroundRight,
            ChannelLocation::TopSurroundRight => ChannelLocation::TopSurroundLeft,
            _ => return None,
        })
    }

    /// `true` for the two low-frequency-effects locations — `LFE`
    /// (Table E2.5 bit 15) and `LFE2` (bit 14). Lets a §7.8 downmix
    /// router or a WAVE-mask reorderer route the band-limited LFE feed
    /// to the dedicated `LOW_FREQUENCY` speaker slot without re-walking
    /// the location list.
    pub fn is_lfe(self) -> bool {
        matches!(self, ChannelLocation::Lfe | ChannelLocation::Lfe2)
    }

    /// `true` for the height-plane locations — the `Vhl/Vhr` pair
    /// (bit 11), `Vhc` (bit 12), and the `Lts/Rts` top-surround pair
    /// (bit 13), plus the single `Ts` top-surround (bit 8). These are
    /// the Table E2.5 rows that sit above the listener plane per
    /// SMPTE 428-3, distinguishing them from the ear-level surround
    /// rows for an immersive-capable renderer.
    pub fn is_height(self) -> bool {
        matches!(
            self,
            ChannelLocation::TopSurround
                | ChannelLocation::VerticalHeightLeft
                | ChannelLocation::VerticalHeightRight
                | ChannelLocation::VerticalHeightCenter
                | ChannelLocation::TopSurroundLeft
                | ChannelLocation::TopSurroundRight
        )
    }

    /// `true` for the ear-level surround locations — the base `Ls/Rs`
    /// pair (bits 3, 4), the `Cs` center-surround (bit 7), the
    /// `Lrs/Rrs` rear-surround pair (bit 6), and the `Lsd/Rsd`
    /// surround-direct pair (bit 9). Excludes the height-plane surround
    /// rows (see [`Self::is_height`]) and the front / wide rows.
    pub fn is_surround(self) -> bool {
        matches!(
            self,
            ChannelLocation::LeftSurround
                | ChannelLocation::RightSurround
                | ChannelLocation::CenterSurround
                | ChannelLocation::LeftRearSurround
                | ChannelLocation::RightRearSurround
                | ChannelLocation::LeftSurroundDirect
                | ChannelLocation::RightSurroundDirect
        )
    }
}

/// Errors raised by the chanmap decoder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChanmapError {
    /// The expanded chanmap location count does not match the dep
    /// substream's coded channel count. Per §E.2.3.1.8 this is a
    /// bit-stream violation.
    CountMismatch {
        expanded: u8,
        dep_nchans: u8,
        chanmap: u16,
    },
}

impl core::fmt::Display for ChanmapError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ChanmapError::CountMismatch {
                expanded,
                dep_nchans,
                chanmap,
            } => write!(
                f,
                "eac3 chanmap: expanded {} locations but dep substream codes {} channels (chanmap=0x{:04X})",
                expanded, dep_nchans, chanmap
            ),
        }
    }
}

impl core::error::Error for ChanmapError {}

/// Expand the 16-bit `chanmap` field into the ordered list of channel
/// locations carried by the dep substream.
///
/// `dep_nchans` is the dependent substream's total coded channel
/// count (`acmod_nfchans(acmod) + lfeon as u8`); the function checks
/// the spec invariant that the expanded count equals `dep_nchans`.
///
/// Iteration order is bit 0 (Left) → bit 15 (LFE), i.e. MSB→LSB of
/// the `chanmap` field. Pair-bits produce two consecutive entries in
/// the order documented on each variant.
pub fn expand_chanmap_locations(
    chanmap: u16,
    dep_nchans: u8,
) -> Result<Vec<ChannelLocation>, ChanmapError> {
    let mut out: Vec<ChannelLocation> = Vec::with_capacity(16);
    // Iterate bit indices 0..16. Bit 0 sits in the MSB of the 16-bit
    // field, so it has weight `1 << 15`.
    for bit in 0u8..16 {
        let mask = 1u16 << (15 - bit);
        if chanmap & mask == 0 {
            continue;
        }
        let push_results: &[ChannelLocation] = match bit {
            0 => &[ChannelLocation::Left],
            1 => &[ChannelLocation::Center],
            2 => &[ChannelLocation::Right],
            3 => &[ChannelLocation::LeftSurround],
            4 => &[ChannelLocation::RightSurround],
            5 => &[ChannelLocation::LeftCenter, ChannelLocation::RightCenter],
            6 => &[
                ChannelLocation::LeftRearSurround,
                ChannelLocation::RightRearSurround,
            ],
            7 => &[ChannelLocation::CenterSurround],
            8 => &[ChannelLocation::TopSurround],
            9 => &[
                ChannelLocation::LeftSurroundDirect,
                ChannelLocation::RightSurroundDirect,
            ],
            10 => &[ChannelLocation::LeftWide, ChannelLocation::RightWide],
            11 => &[
                ChannelLocation::VerticalHeightLeft,
                ChannelLocation::VerticalHeightRight,
            ],
            12 => &[ChannelLocation::VerticalHeightCenter],
            13 => &[
                ChannelLocation::TopSurroundLeft,
                ChannelLocation::TopSurroundRight,
            ],
            14 => &[ChannelLocation::Lfe2],
            15 => &[ChannelLocation::Lfe],
            _ => unreachable!(),
        };
        for &loc in push_results {
            // At most 16 distinct entries (pair-bits use 2 slots each;
            // total expanded count cannot exceed 16 since the spec
            // limits dep-substream coded channels to A/52's maximum).
            out.push(loc);
        }
    }

    let expanded = out.len() as u8;
    if expanded != dep_nchans {
        return Err(ChanmapError::CountMismatch {
            expanded,
            dep_nchans,
            chanmap,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Spec example #1 (§E.2.3.1.8): "if bits 0, 3, and 4 of the
    /// chanmap field are set to '1', and the dependent stream is
    /// coded with acmod = 3 and lfeon = 0, the first coded channel
    /// in the dependent stream is the Left channel, the second
    /// coded channel is the Left Surround channel, and the third
    /// coded channel is the Right Surround channel."
    #[test]
    fn spec_example_bits_0_3_4() {
        // Bit 0 = MSB = 0x8000; bit 3 = 0x1000; bit 4 = 0x0800.
        let chanmap = 0x8000 | 0x1000 | 0x0800;
        let dep_nchans = 3; // acmod=3 (3-ch) + lfeon=0
        let locs = expand_chanmap_locations(chanmap, dep_nchans).unwrap();
        assert_eq!(locs.len(), 3);
        assert_eq!(locs[0], ChannelLocation::Left);
        assert_eq!(locs[1], ChannelLocation::LeftSurround);
        assert_eq!(locs[2], ChannelLocation::RightSurround);
    }

    /// Spec example #2 (§E.2.3.1.8): "if bits 3, 4 and 6 of the
    /// chanmap field are set to '1', and the dependent stream is
    /// coded with acmod = 6 and lfeon = '0', the first coded channel
    /// in the dependent stream is the Left Surround channel, the
    /// second coded channel is the Right Surround channel, and the
    /// third and fourth channels are the Left Rear Surround and
    /// Right Rear Surround channels."
    #[test]
    fn spec_example_bits_3_4_6_pair() {
        // Bit 3 = 0x1000; bit 4 = 0x0800; bit 6 (pair) = 0x0200.
        let chanmap = 0x1000 | 0x0800 | 0x0200;
        let dep_nchans = 4; // acmod=6 (4-ch) + lfeon=0
        let locs = expand_chanmap_locations(chanmap, dep_nchans).unwrap();
        assert_eq!(locs.len(), 4);
        assert_eq!(locs[0], ChannelLocation::LeftSurround);
        assert_eq!(locs[1], ChannelLocation::RightSurround);
        assert_eq!(locs[2], ChannelLocation::LeftRearSurround);
        assert_eq!(locs[3], ChannelLocation::RightRearSurround);
    }

    /// The in-tree E-AC-3 encoder emits 7.1 as indep 5.1 (acmod=7,
    /// lfeon=1) plus a dep substream carrying the Lb/Rb pair with
    /// chanmap bit 6 ("Lrs/Rrs pair") set, dep acmod=2 (2 coded
    /// channels). Decoder must round-trip the pair as the two rear-
    /// surround channels.
    #[test]
    fn encoder_71_lb_rb_pair() {
        // Bit 6 weight = 1 << (15 - 6) = 0x0200.
        let chanmap = 0x0200;
        let dep_nchans = 2; // acmod=2 (2-ch) + lfeon=0
        let locs = expand_chanmap_locations(chanmap, dep_nchans).unwrap();
        assert_eq!(locs.len(), 2);
        assert_eq!(locs[0], ChannelLocation::LeftRearSurround);
        assert_eq!(locs[1], ChannelLocation::RightRearSurround);
    }

    /// Spec invariant: expanded count must equal dep_nchans. A pair
    /// bit set with dep_nchans=1 is a bit-stream violation.
    #[test]
    fn count_mismatch_rejected() {
        let chanmap = 0x0200; // bit 6 (pair) — expands to 2
        let dep_nchans = 1; // mismatched
        let err = expand_chanmap_locations(chanmap, dep_nchans).unwrap_err();
        assert!(matches!(err, ChanmapError::CountMismatch { .. }));
    }

    /// Bit 0 lives at MSB; bit 15 lives at LSB. Sanity-check the
    /// extreme bits and confirm the iteration order picks low-index
    /// (MSB) bits first.
    #[test]
    fn msb_lsb_extremes_and_order() {
        // Bits 0 (Left, MSB) + 15 (LFE, LSB).
        let chanmap = 0x8000 | 0x0001;
        let locs = expand_chanmap_locations(chanmap, 2).unwrap();
        assert_eq!(locs.len(), 2);
        assert_eq!(locs[0], ChannelLocation::Left);
        assert_eq!(locs[1], ChannelLocation::Lfe);
    }

    /// Every single-channel bit (i.e. non-pair bits) — exercises
    /// all 10 non-pair Table E2.5 rows.
    #[test]
    fn all_single_bits_decode() {
        let mut chanmap = 0u16;
        let single_bits = [0, 1, 2, 3, 4, 7, 8, 12, 14, 15];
        for &b in &single_bits {
            chanmap |= 1u16 << (15 - b);
        }
        let locs = expand_chanmap_locations(chanmap, single_bits.len() as u8).unwrap();
        assert_eq!(locs.len(), single_bits.len());
        let expected = [
            ChannelLocation::Left,
            ChannelLocation::Center,
            ChannelLocation::Right,
            ChannelLocation::LeftSurround,
            ChannelLocation::RightSurround,
            ChannelLocation::CenterSurround,
            ChannelLocation::TopSurround,
            ChannelLocation::VerticalHeightCenter,
            ChannelLocation::Lfe2,
            ChannelLocation::Lfe,
        ];
        for (i, &want) in expected.iter().enumerate() {
            assert_eq!(locs[i], want, "single-bit slot {i}");
        }
    }

    /// `ChannelLocation::ALL` lists exactly the 22 distinct variants in
    /// Table E2.5 bit order (pair-bits expanded left-then-right), with no
    /// duplicates and no omissions.
    #[test]
    fn all_lists_every_variant_in_table_order() {
        // 16 location bits, 6 of which are pairs → 16 + 6 = 22 entries.
        assert_eq!(ChannelLocation::ALL.len(), 22);
        // The bit indices are non-decreasing across the list (pair halves
        // share a bit; everything else strictly increases).
        let mut prev = 0u8;
        for (i, loc) in ChannelLocation::ALL.iter().enumerate() {
            let bit = loc.table_e2_5_bit();
            if i > 0 {
                assert!(bit >= prev, "ALL not in bit order at index {i}");
            }
            prev = bit;
        }
        // No duplicate variants.
        for (i, a) in ChannelLocation::ALL.iter().enumerate() {
            for b in &ChannelLocation::ALL[i + 1..] {
                assert_ne!(a, b, "duplicate variant in ALL");
            }
        }
    }

    /// `table_e2_5_bit` maps each variant to its Table E2.5 row; both
    /// halves of a pair-bit share the row's single bit index.
    #[test]
    fn table_e2_5_bit_maps_each_row() {
        assert_eq!(ChannelLocation::Left.table_e2_5_bit(), 0);
        assert_eq!(ChannelLocation::Center.table_e2_5_bit(), 1);
        assert_eq!(ChannelLocation::Right.table_e2_5_bit(), 2);
        assert_eq!(ChannelLocation::LeftSurround.table_e2_5_bit(), 3);
        assert_eq!(ChannelLocation::RightSurround.table_e2_5_bit(), 4);
        // Pair bit 5 — both halves share bit 5.
        assert_eq!(ChannelLocation::LeftCenter.table_e2_5_bit(), 5);
        assert_eq!(ChannelLocation::RightCenter.table_e2_5_bit(), 5);
        // Pair bit 6.
        assert_eq!(ChannelLocation::LeftRearSurround.table_e2_5_bit(), 6);
        assert_eq!(ChannelLocation::RightRearSurround.table_e2_5_bit(), 6);
        assert_eq!(ChannelLocation::CenterSurround.table_e2_5_bit(), 7);
        assert_eq!(ChannelLocation::TopSurround.table_e2_5_bit(), 8);
        assert_eq!(ChannelLocation::LeftSurroundDirect.table_e2_5_bit(), 9);
        assert_eq!(ChannelLocation::RightSurroundDirect.table_e2_5_bit(), 9);
        assert_eq!(ChannelLocation::LeftWide.table_e2_5_bit(), 10);
        assert_eq!(ChannelLocation::RightWide.table_e2_5_bit(), 10);
        assert_eq!(ChannelLocation::VerticalHeightLeft.table_e2_5_bit(), 11);
        assert_eq!(ChannelLocation::VerticalHeightRight.table_e2_5_bit(), 11);
        assert_eq!(ChannelLocation::VerticalHeightCenter.table_e2_5_bit(), 12);
        assert_eq!(ChannelLocation::TopSurroundLeft.table_e2_5_bit(), 13);
        assert_eq!(ChannelLocation::TopSurroundRight.table_e2_5_bit(), 13);
        assert_eq!(ChannelLocation::Lfe2.table_e2_5_bit(), 14);
        assert_eq!(ChannelLocation::Lfe.table_e2_5_bit(), 15);
    }

    /// `chanmap_weight` places bit 0 (Left) in the MSB and bit 15 (LFE)
    /// in the LSB per §E.2.3.1.8; pair halves share the single weight.
    #[test]
    fn chanmap_weight_msb_first() {
        assert_eq!(ChannelLocation::Left.chanmap_weight(), 0x8000);
        assert_eq!(ChannelLocation::Lfe.chanmap_weight(), 0x0001);
        // Bit 6 → weight 1 << (15 - 6) = 0x0200; both halves agree.
        assert_eq!(ChannelLocation::LeftRearSurround.chanmap_weight(), 0x0200);
        assert_eq!(ChannelLocation::RightRearSurround.chanmap_weight(), 0x0200);
    }

    /// A decoded location list re-OR's back into the original `chanmap`
    /// field via `chanmap_weight` — pair-bits, set once in the source,
    /// must not be double-counted (both halves OR the same weight).
    #[test]
    fn chanmap_weight_round_trips_decoded_list() {
        // Spec example #2: bits 3, 4, 6 set on a 4-channel dep substream.
        let chanmap = 0x1000 | 0x0800 | 0x0200;
        let locs = expand_chanmap_locations(chanmap, 4).unwrap();
        let reconstructed = locs
            .iter()
            .fold(0u16, |acc, loc| acc | loc.chanmap_weight());
        assert_eq!(reconstructed, chanmap);

        // A map with two pair-bits + a single bit (bits 0, 6, 9 →
        // Left + Lrs/Rrs + Lsd/Rsd = 5 coded channels).
        let chanmap = 0x8000 | 0x0200 | 0x0040;
        let locs = expand_chanmap_locations(chanmap, 5).unwrap();
        let reconstructed = locs
            .iter()
            .fold(0u16, |acc, loc| acc | loc.chanmap_weight());
        assert_eq!(reconstructed, chanmap);
    }

    /// `is_pair_half` is true exactly for the 12 expanded halves of the
    /// 6 Table E2.5 pair-bits, and `pair_companion` returns the other
    /// half (and `None` for single-channel locations).
    #[test]
    fn pair_half_and_companion() {
        let pair_halves = [
            (ChannelLocation::LeftCenter, ChannelLocation::RightCenter),
            (
                ChannelLocation::LeftRearSurround,
                ChannelLocation::RightRearSurround,
            ),
            (
                ChannelLocation::LeftSurroundDirect,
                ChannelLocation::RightSurroundDirect,
            ),
            (ChannelLocation::LeftWide, ChannelLocation::RightWide),
            (
                ChannelLocation::VerticalHeightLeft,
                ChannelLocation::VerticalHeightRight,
            ),
            (
                ChannelLocation::TopSurroundLeft,
                ChannelLocation::TopSurroundRight,
            ),
        ];
        let mut pair_count = 0;
        for (l, r) in pair_halves {
            assert!(l.is_pair_half());
            assert!(r.is_pair_half());
            assert_eq!(l.pair_companion(), Some(r));
            assert_eq!(r.pair_companion(), Some(l));
            // Companions share the Table E2.5 bit.
            assert_eq!(l.table_e2_5_bit(), r.table_e2_5_bit());
            pair_count += 2;
        }
        assert_eq!(pair_count, 12);

        // Single-channel locations are not pair halves and have no
        // companion.
        for loc in [
            ChannelLocation::Left,
            ChannelLocation::Center,
            ChannelLocation::CenterSurround,
            ChannelLocation::VerticalHeightCenter,
            ChannelLocation::Lfe,
            ChannelLocation::Lfe2,
        ] {
            assert!(!loc.is_pair_half());
            assert_eq!(loc.pair_companion(), None);
        }
    }

    /// `is_lfe` flags only the two LFE rows (bits 14, 15).
    #[test]
    fn is_lfe_flags_lfe_rows() {
        assert!(ChannelLocation::Lfe.is_lfe());
        assert!(ChannelLocation::Lfe2.is_lfe());
        for loc in ChannelLocation::ALL {
            if !matches!(loc, ChannelLocation::Lfe | ChannelLocation::Lfe2) {
                assert!(!loc.is_lfe(), "{loc:?} should not be LFE");
            }
        }
    }

    /// `is_height` flags exactly the SMPTE 428-3 above-plane rows: Ts
    /// (bit 8), Vhl/Vhr (bit 11), Vhc (bit 12), Lts/Rts (bit 13).
    #[test]
    fn is_height_flags_above_plane_rows() {
        let height = [
            ChannelLocation::TopSurround,
            ChannelLocation::VerticalHeightLeft,
            ChannelLocation::VerticalHeightRight,
            ChannelLocation::VerticalHeightCenter,
            ChannelLocation::TopSurroundLeft,
            ChannelLocation::TopSurroundRight,
        ];
        for loc in ChannelLocation::ALL {
            let want = height.contains(&loc);
            assert_eq!(loc.is_height(), want, "{loc:?} height classification");
        }
        // Height and LFE are disjoint; height and ear-level surround are
        // disjoint.
        for loc in ChannelLocation::ALL {
            if loc.is_height() {
                assert!(!loc.is_lfe());
                assert!(!loc.is_surround());
            }
        }
    }

    /// `is_surround` flags the ear-level surround rows (Ls/Rs, Cs,
    /// Lrs/Rrs, Lsd/Rsd) and excludes the height and front rows.
    #[test]
    fn is_surround_flags_ear_level_rows() {
        let surround = [
            ChannelLocation::LeftSurround,
            ChannelLocation::RightSurround,
            ChannelLocation::CenterSurround,
            ChannelLocation::LeftRearSurround,
            ChannelLocation::RightRearSurround,
            ChannelLocation::LeftSurroundDirect,
            ChannelLocation::RightSurroundDirect,
        ];
        for loc in ChannelLocation::ALL {
            let want = surround.contains(&loc);
            assert_eq!(loc.is_surround(), want, "{loc:?} surround classification");
        }
        // Front rows are neither surround nor height nor LFE.
        for loc in [
            ChannelLocation::Left,
            ChannelLocation::Center,
            ChannelLocation::Right,
            ChannelLocation::LeftCenter,
            ChannelLocation::RightCenter,
        ] {
            assert!(!loc.is_surround());
            assert!(!loc.is_height());
            assert!(!loc.is_lfe());
        }
    }
}
