//! # oxideav-dts
//!
//! Pure-Rust DTS Coherent Acoustics decoder for the
//! [oxideav](https://github.com/OxideAV/oxideav) framework.
//!
//! **Status:** clean-room rebuild round 6 (frame-header parser +
//! 14-bit sync unpacking + trailing-flag fields + optional
//! 16-bit header CRC field + 16-bit post-CRC trailing window
//! [multirate-inter / version / copy-history / PCMR / front-sum /
//! surround-sum / dialnorm] + `oxideav-core` `Decoder`
//! integration + multi-frame iterator / resync helper).
//!
//! Round 1 (2026-05-21) landed a structural [`DtsFrameHeader`] parser
//! for the DTS Core frame sync header (per the multimedia.cx wiki
//! snapshot at `docs/audio/dts/wiki/DTS.wiki`, which mirrors the
//! ETSI TS 102 114 §5.3 bit layout). Round 2 (2026-05-21) adds a
//! 14-bit unpacker so both 14-bit container forms decode through
//! the same structural parser as the two 16-bit raw forms.
//! Round 3 (2026-05-21) extends the typed header through the 13
//! trailing single-bit / small-field flags after RATE (downmix,
//! dynamic-range, time-stamp, aux-data, HDCD, ext-audio-descr,
//! ext-audio-coding, ASPF, 2-bit LFE mode, predictor-history) plus
//! the optional 16-bit `HEADER_CRC` field that follows when
//! [`DtsFrameHeader::crc_present`] is set. (The Annex B CRC
//! algorithm landed in round 408 as [`dts_crc16`]; the core `HCRC`
//! stays unverified by design — §5.3.1 states "The CRC value test
//! shall not be applied" — so
//! [`DtsFrameHeader::verify_header_crc`] returns `None` and the raw
//! 16-bit field is surfaced for pass-through callers.)
//! Round 4 (2026-05-22) wires the crate into `oxideav-core`'s
//! [`oxideav_core::Decoder`] surface (behind a default-on `registry`
//! cargo feature) plus a standalone [`probe_dts`] helper. The
//! `DtsDecoderHandle` returned by the factory parses the frame
//! header eagerly inside `send_packet`; `receive_frame` returns
//! `Error::Unsupported` because PCM output is gated on the
//! SFREQ/RATE/AMODE value tables landing in `docs/`. Bitstream /
//! subframe decoding is **not** part of this round.
//!
//! Round 5 (2026-05-25) extends [`DtsFrameHeader`] through the
//! 16-bit post-CRC trailing window the wiki snapshot enumerates
//! after `HEADER_CRC`: `multirate_inter` (1), `version` (4),
//! `copy_history` (2), `source_pcm_resolution_index` (3),
//! `front_sum` (1), `surround_sum` (1), `dialog_normalization`
//! (4). These bits are consumed unconditionally (the wiki shows
//! them following the HEADER_CRC slot whether or not CRC was
//! emitted). The PCMR→bits-per-sample and DIALNORM→dB mappings
//! are still missing from `docs/`, so
//! [`DtsFrameHeader::source_pcm_bits_per_sample`] and
//! [`DtsFrameHeader::dialog_normalization_db`] return `None`
//! until those tables land. Bitstream / subframe decoding is
//! still **not** part of this round.
//!
//! Round 6 (2026-05-25) adds a multi-frame iterator and a resync
//! helper on top of the existing single-frame parsers:
//! [`find_next_sync`] scans a byte buffer for the next DTS sync
//! sequence at or after a given offset (all four documented
//! encodings); [`iter_frames`] walks a raw-16-bit DTS Core byte
//! stream frame by frame, using each frame's
//! [`DtsFrameHeader::frame_size_bytes`] to advance to the next
//! sync. A new 5-frame ffmpeg-generated fixture
//! (`tests/fixtures/dts_5_frames.bin`, 5 120 bytes) exercises the
//! iterator end-to-end and confirms every frame's header decodes
//! identically. The iterator is documented as raw-16-bit-only
//! because the wiki snapshot does not enumerate the 14-bit
//! container-byte advance rule (see `README.md`'s round-6 docs
//! gap #7); the iterator therefore yields
//! [`Error::UnsupportedFourteenBit`] and terminates if a 14-bit
//! sync is encountered.
//!
//! Round 141 (2026-05-26) closes the parse↔encode round-trip on the
//! frame-sync header window: [`encode_frame_header_be`] serialises a
//! parsed [`DtsFrameHeader`] back into the raw-BE on-wire bytes
//! prescribed by the wiki bit-table (exactly
//! [`DtsFrameHeader::header_byte_length`] bytes long — 13 or 15 — and
//! always beginning with the canonical raw-BE sync). The encoder is
//! the inverse of [`parse_frame_header`] and validates the same
//! structural bounds plus per-field bit-width bounds via the new
//! [`Error::FieldOutOfRange`] variant.
//!
//! Round 151 (2026-05-26) adds [`find_all_syncs`], the bulk-scan
//! counterpart to [`find_next_sync`]: instead of returning the first
//! sync at or after a cursor, it walks the entire input buffer and
//! returns every documented sync occurrence (all four encodings) as a
//! `Vec<SyncMatch>`. Useful for stream-integrity tooling that needs to
//! know about every resync point up front rather than walking one at a
//! time. Same `O(n)` cost as a `find_next_sync` loop from cursor + 1;
//! the bulk helper just materialises the result. The round also closes
//! a missing coverage gap by testing [`iter_frames`] against a raw-LE
//! multi-frame stream — the iterator already supported raw-LE because
//! `frame_size_bytes` is byte-equivalent across both raw encodings
//! (per the wiki), but the previous test grid only exercised raw-BE
//! via the bundled ffmpeg fixture.
//!
//! Round 159 (2026-05-27) adds an error-tolerant counterpart to
//! [`iter_frames`]: [`iter_frames_resync`] / [`FrameIteratorResync`]
//! treat a candidate sync whose subsequent header bits fail the
//! structural NBLKS / FSIZE bounds (or whose declared
//! `frame_size_bytes` overruns end-of-buffer) as a false-positive
//! sync rather than a hard fault — they surface a [`ResyncEvent`]
//! documenting the discarded offset + cause and continue scanning
//! one byte further instead of terminating. This lets demuxers and
//! stream-integrity tooling walk a partially-corrupted `.dts`
//! stream past malformed-sync patches and recover frames after the
//! damage. The fail-fast [`iter_frames`] remains exactly as
//! documented (returns the first parse error and terminates).
//!
//! Round 165 (2026-05-27) gates the inner-loop multi-byte
//! [`crate::SyncWordEncoding`] detection of [`find_next_sync`] (and
//! therefore [`find_all_syncs`], [`iter_frames`], and
//! [`iter_frames_resync`]) behind a one-byte first-byte filter. The
//! four documented sync sequences all begin with one of `0x7F` /
//! `0xFE` / `0x1F` / `0xFF` (distinct first bytes per the wiki
//! bit-table), so 252 of 256 possible payload bytes are
//! short-circuited before the multi-byte comparison fires. On
//! random payload the inner loop visits ~98.4% of positions with a
//! single byte read + branch rather than the previous 4-byte raw
//! check + 6-byte 14-bit check. The walk order, returned offsets,
//! and matched encoding tags are unchanged from round 6 — round 165
//! also adds 8 new tests (171 total) including a
//! `find_next_sync_matches_pre_optimization_reference_on_candidate_dense_payload`
//! equivalence harness, a pseudo-random-buffer cross-check against
//! a brute-force pre-optimisation reference, an all-`0xFF` payload
//! stress test (every position is a first-byte candidate so the
//! gate's negative-filter property must hold from the
//! multi-byte side), and an exhaustive 256-input check that the
//! first-byte filter accepts exactly `{0x1F, 0x7F, 0xFE, 0xFF}`.
//!
//! Round 179 (2026-05-29) adds a streaming counterpart to
//! [`find_all_syncs`] plus a small accessor surface on
//! [`SyncWordEncoding`] and [`SyncMatch`], all derived directly from
//! the wiki snapshot's "How to distinguish different versions"
//! sync-sequence table:
//!
//! - [`iter_syncs`] / [`SyncIterator`] — a lazy `Iterator<Item =
//!   SyncMatch>` over every sync sequence in a byte buffer. Same
//!   matching rules, walk order, and `O(n)` cost as
//!   [`find_all_syncs`], but without the upfront `Vec<SyncMatch>`
//!   allocation. Lets stream-integrity tooling consume matches one
//!   at a time, stop early after a [`Iterator::take`] window, or
//!   route through standard combinators (e.g.
//!   `iter_syncs(bytes).filter(|m| m.encoding.is_raw_16bit())`).
//! - [`SyncWordEncoding::sync_byte_length`] — wiki-table-derived
//!   byte count of the on-wire sync sequence (4 for the raw
//!   encodings, 6 for the 14-bit-packed encodings).
//!   [`SyncWordEncoding::is_raw_16bit`] and
//!   [`SyncWordEncoding::is_14bit_packed`] are convenience
//!   predicates so callers can branch on the raw-vs-packed
//!   distinction without spelling out the four-arm `matches!`.
//! - [`SyncMatch::sync_byte_length`] and
//!   [`SyncMatch::sync_byte_range`] — thin accessors that delegate
//!   to the encoding's wiki-derived length so the common pattern
//!   "advance past the matched sync" or "highlight the matched
//!   bytes" reads naturally
//!   (`cursor = m.offset + m.sync_byte_length()` /
//!   `&bytes[m.sync_byte_range()]`).
//!
//! No new docs gap is introduced. The byte-length values are read
//! verbatim from `docs/audio/dts/wiki/DTS.wiki`'s sync table. The
//! then-open #928 / #1055 / #1084 docs gaps (SFREQ / RATE / AMODE
//! tables, HEADER_CRC polynomial, PCMR / DIALNORM tables,
//! 14-bit container-byte advance rule) have since been resolved
//! (the CRC algorithm by `docs/audio/dts/dts-crc16.md`, round 408).
//!
//! Round 148 (2026-05-26) completes the encoder surface across all
//! four documented sync encodings. The two new primitives,
//! [`encode_frame_header_14bit_be`] and [`encode_frame_header_14bit_le`],
//! compose [`encode_frame_header_be`] with [`pack_16bit_to_14bit`]:
//! the raw-BE 13 or 15-byte header window is zero-padded to 16 bytes
//! (the minimum the parser's 14-bit pre-unpack step needs to land a
//! 16-byte raw-BE window) and re-packed into 14-bit-payload containers
//! in the requested byte order. Both encoders emit exactly **18 bytes**
//! (nine 14-bit containers carrying 126 payload bits) regardless of
//! `crc_present`; the trailing 16 padding bits land in what would be
//! the first SUBFRAMES bits of a real frame and are inert for
//! parsing. The 14-bit-LE output is the pairwise byte-swap of the
//! 14-bit-BE output, matching the wiki's `1F FF E8 00 …` vs
//! `FF 1F 00 E8 …` sync-prefix relationship. The
//! `parse_frame_header_14bit(encode_frame_header_14bit_<be|le>(hdr))`
//! round-trip recovers `hdr` on every field except `sync_word_encoding`
//! (the parser reports the encoding it detected at the input).
//!
//! Round 145 (2026-05-26) extends the encoder side with two new
//! primitives: [`encode_frame_header_le`] emits the raw-LE on-wire
//! header window (canonical sync `FE 7F 01 80`, always 16 bytes long
//! — the parser's minimum input length for the raw-LE branch — i.e.
//! `encode_frame_header_be` zero-padded to 16 and 16-bit-word-swapped);
//! and [`pack_16bit_to_14bit`] is the inverse of
//! [`unpack_14bit_to_16bit`], packing an MSB-first 16-bit-equivalent
//! byte stream into 14-bit-payload containers with the wiki's "sign
//! bit extension" rule applied to the upper 2 bits of each container.
//! `pack_16bit_to_14bit` returns the packed bytes plus the
//! `payload_bit_count` so callers can recover the exact pre-pack bit
//! length on the receiving end; together with the existing
//! `unpack_14bit_to_16bit` it completes the bidirectional 14↔16-bit
//! container conversion the wiki snapshot prescribes. The 14-bit
//! sync prefix bytes `1F FF E8 00 …` (BE) and `FF 1F 00 E8 …` (LE)
//! the wiki documents are reproduced byte-for-byte by feeding the
//! 32-bit raw-BE syncword `7F FE 80 01` into `pack_16bit_to_14bit`
//! (the last container of the wiki's example carries 10 bits of the
//! following field that are not part of the syncword — that's why
//! the wiki shows `07 Fx` rather than a literal trailing byte).
//!
//! Round 138 (2026-05-26) surfaces the header→SUBFRAMES boundary
//! through two new accessors and one [`FrameView`] helper:
//! [`DtsFrameHeader::header_bit_length`] returns the bit-count the
//! parser consumed (104 or 120 depending on `crc_present`, both
//! exact multiples of 8 by the wiki bit-table arithmetic);
//! [`DtsFrameHeader::header_byte_length`] returns the byte-count
//! (13 or 15); and [`FrameView::payload`] carves out the SUBFRAMES
//! region (`data[header_byte_length()..]`) so downstream re-muxers
//! and the future subframe decoder can address the payload window
//! without recomputing the header boundary. The values are
//! fully derived from the wiki bit-table in
//! `docs/audio/dts/wiki/DTS.wiki`; no new doc dependency.
//!
//! The parser distinguishes the four documented bitstream encodings
//! via the 32-bit (or 40-bit) syncword (see [`SyncWordEncoding`]) and
//! decodes the structural fields whose semantics are spelled out
//! verbatim in the wiki:
//!
//! - frame type (termination vs normal),
//! - per-block sample count (`deficit + 1`),
//! - CRC-present flag,
//! - number of blocks in the frame (5..=128),
//! - frame size in bytes (95..=16384),
//! - channel configuration index (0..=15 standard, 16..=63
//!   user-defined),
//! - sample-frequency index (4 bits),
//! - transmission-bitrate index (5 bits).
//!
//! The structural parser returns the raw indices and exposes
//! `Option` resolvers for the value tables. The transmission-bitrate
//! table landed in round 185 from ETSI TS 102 114 §5.3.1 Table 5-7
//! (`docs/audio/dts/dts-core-extracts.md` §1): [`TargetedBitRate`] /
//! [`DtsFrameHeader::targeted_bit_rate`] /
//! [`DtsFrameHeader::bit_rate_bps`] now resolve. Round 202 landed
//! the sample-frequency, channel-configuration, and source-PCM-
//! resolution tables (ETSI §5.3.1 Tables 5-5 / 5-4 / 5-17 from the
//! staged PDF): [`SampleFrequency`] / [`DtsFrameHeader::sample_rate_hz`]
//! / [`DtsFrameHeader::sample_frequency`], [`AmodeArrangement`] /
//! [`DtsFrameHeader::channel_count`] /
//! [`DtsFrameHeader::amode_arrangement`], and [`SourcePcmResolution`]
//! / [`DtsFrameHeader::source_pcm_bits_per_sample`] /
//! [`DtsFrameHeader::source_pcm_resolution`] now resolve. The
//! dialog-normalization table (Table 5-20) is still pending, so
//! [`DtsFrameHeader::dialog_normalization_db`] returns `None` until
//! it lands in `docs/`. See `README.md`'s "Docs gaps" section.
//!
//! ## What does *not* belong here
//!
//! - Container muxing (Wav / MP4 / Matroska carriage).
//! - DTS-HD / EXSS / XLL / X96 / XCH extension substreams.
//! - PCM decoding (subband + QMF + Huffman, all deferred to future
//!   rounds).
//!
//! ## Public API
//!
//! - [`DtsFrameHeader`] — typed parse result.
//! - [`SyncWordEncoding`] — the four documented sync variants.
//! - [`FrameType`] — termination vs normal.
//! - [`TargetedBitRate`] — `RATE`-field resolution (fixed / open /
//!   invalid) per ETSI §5.3.1 Table 5-7 (added in round 185).
//! - [`SampleFrequency`] — `SFREQ`-field resolution (fixed / invalid)
//!   per ETSI §5.3.1 Table 5-5 (added in round 202).
//! - [`AmodeArrangement`] — `AMODE`-field resolution (sixteen
//!   standard arrangements + user-defined codes) per ETSI §5.3.1
//!   Table 5-4 (added in round 202).
//! - [`SourcePcmResolution`] — `PCMR`-field resolution (valid
//!   bits + ES flag, or invalid) per ETSI §5.3.1 Table 5-17 (added
//!   in round 202).
//! - [`parse_frame_header`] — non-allocating single-frame parser
//!   for the two raw 16-bit syncs.
//! - [`parse_frame_header_14bit`] — single-frame parser for the two
//!   14-bit packed syncs (added in round 2).
//! - [`encode_frame_header_be`] — inverse of [`parse_frame_header`]
//!   that emits the wiki bit-table back into raw-BE bytes (added in
//!   round 141).
//! - [`encode_frame_header_le`] — raw-LE encoder variant
//!   (`encode_frame_header_be` zero-padded to 16 bytes + 16-bit-word
//!   swap; added in round 145).
//! - [`encode_frame_header_14bit_be`] / [`encode_frame_header_14bit_le`]
//!   — 14-bit-packed encoder variants (`encode_frame_header_be` padded
//!   to 16 bytes then re-packed through [`pack_16bit_to_14bit`]; added
//!   in round 148). Both emit exactly 18 bytes regardless of
//!   `crc_present`, matching the parser's minimum 14-bit input length.
//! - [`unpack_14bit_to_16bit`] / [`pack_16bit_to_14bit`] /
//!   [`FourteenBitByteOrder`] — the 14↔16-bit container conversion
//!   primitives. `unpack_14bit_to_16bit` added in round 2;
//!   `pack_16bit_to_14bit` added in round 145.
//! - [`find_next_sync`] / [`find_all_syncs`] / [`iter_frames`] /
//!   [`FrameIterator`] / [`FrameView`] / [`SyncMatch`] — multi-frame
//!   walker + resync helpers. `find_next_sync` / `iter_frames` /
//!   `FrameIterator` / `FrameView` / `SyncMatch` added in round 6;
//!   `find_all_syncs` added in round 151.
//! - [`iter_frames_14bit`] / [`FrameIterator14`] / [`FrameView14`] —
//!   14-bit-packed container-stream frame walker (added in round 192).
//!   Walks the same kind of self-delimited Core stream as
//!   [`iter_frames`] but in the 14-bit-per-container-word domain; uses
//!   the round-189 [`DtsFrameHeader::frame_size_container_bytes`]
//!   accessor for the per-frame container-byte advance.
//! - [`iter_syncs`] / [`SyncIterator`] — lazy streaming counterpart
//!   to [`find_all_syncs`] (added in round 179).
//! - [`iter_frames_resync`] / [`FrameIteratorResync`] /
//!   [`ResyncEvent`] / [`ResyncCause`] — error-tolerant walker that
//!   skips past false-positive sync candidates (added in round 159).
//! - [`precal_cos_mod`] / [`COS_MOD_LEN`] / [`COS_MOD_BLOCK1_START`] /
//!   [`COS_MOD_BLOCK2_START`] / [`COS_MOD_BLOCK3_START`] /
//!   [`COS_MOD_BLOCK4_START`] — the 544-entry cosine-modulation
//!   matrix used by the §C.2.5 32-band synthesis QMF (added in
//!   round 208). Per-block start indices match the four-block
//!   decomposition of `PreCalCosMod()` in
//!   `docs/audio/dts/dts-core-extracts.md` §2.3.
//! - [`cos_mod_stage`] / [`NUM_SUBBAND`] — the cosine-modulation
//!   stage of `QMFInterpolation()` (§C.2.5, PDF p.185, per
//!   `docs/audio/dts/dts-core-extracts.md` §2.4): consumes one
//!   per-sample subband vector `raXin[0..32]` plus the
//!   [`precal_cos_mod`] matrix, returns the 32 leading entries
//!   `raX[0..32]` written into the synthesis filter's shift
//!   register before the 512-tap FIR convolution. Independent of
//!   the §D.8 FIR coefficient tables (round-208 docs gap #9), so
//!   it ships ahead of the full `QMFInterpolation()` driver. Added
//!   in round 255.
//! - [`assemble_xin`] / [`shift_x_history`] / [`X_HISTORY_LEN`] /
//!   [`QmfAssembleError`] — the FIR-independent per-sample
//!   raXin assembly + raX shift-register update steps of
//!   `QMFInterpolation()` (§C.2.5, PDF p.185, per
//!   `docs/audio/dts/dts-core-extracts.md` §2.4 lines 182-183 and
//!   217). [`assemble_xin`] builds the input vector
//!   [`cos_mod_stage`] consumes (active subbands copied,
//!   inactive tail zero-filled); [`shift_x_history`] rotates the
//!   512-entry raX register by 32 entries to make room for the
//!   next per-sample cosine-modulation output. Both ship ahead of
//!   the §D.8-dependent FIR step they bracket. Added in round 259.
//! - [`shift_z_output`] / [`Z_OUTPUT_LEN`] — the FIR-independent
//!   post-PCM rotate of the 64-entry `raZ[]` output accumulator
//!   (§C.2.5, PDF p.185, per `docs/audio/dts/dts-core-extracts.md`
//!   §2.4 lines 218-219): shifts the high block `raZ[32..64]` down
//!   into `raZ[0..32]` and zero-fills the freed high block for the
//!   next per-sample FIR accumulation. Pure index manipulation —
//!   reads no §D.8 FIR coefficients — so it ships ahead of the FIR
//!   convolution that fills `raZ[]`. Added in round 271.
//! - [`write_pcm_output`] / [`PCM_OUTPUT_PER_SAMPLE`] — the
//!   FIR-independent PCM-output step of `QMFInterpolation()`
//!   (§C.2.5, PDF p.185, per `docs/audio/dts/dts-core-extracts.md`
//!   §2.4 lines 213-214): consumes the 32 low entries `raZ[0..32]`,
//!   scales each by the per-channel `rScale` multiplier, applies the
//!   spec's `int()` truncate-toward-zero cast, and writes 32
//!   integer PCM samples into the channel buffer at the running
//!   `nChIndex` cursor (returning the advanced cursor). Reads no
//!   §D.8 FIR coefficients — it consumes the already-accumulated
//!   `raZ[0..32]` — so it ships ahead of the FIR step that fills
//!   them. Added in round 274.
//! - [`FilterBankSelection`] — typed selector for the §C.2.5
//!   `QMFInterpolation()` 512-tap FIR coefficient set (the §D.8
//!   `raCoeffLossy` / `raCoeffLossLess` named tables).
//!   `from_filts(u8) -> Self` resolves the §C.2.5 `FILTS` flag per
//!   the pseudocode's `if (FILTS==0) prCoeff = raCoeffLossy; else
//!   prCoeff = raCoeffLossLess;` branch
//!   (`docs/audio/dts/dts-core-extracts.md` §2.4 lines 175-178).
//!   FIR-coefficient-independent: names the two sets but does not
//!   read any coefficient values (round-208 docs gap #9 still
//!   blocks the value transcription). Added in round 263.
//! - [`sum_difference_decode_i32`] / [`sum_difference_decode_f64`] /
//!   [`sum_difference_decode_subband_pair_i32`] /
//!   [`sum_difference_decode_subband_pair_f64`] /
//!   [`front_sum_difference_required`] /
//!   [`surround_sum_difference_required`] — §C.2.4 sum/difference
//!   matrix decoder for the front-channel (`SUMF` or `AMODE == 3`)
//!   and surround-channel (`SUMS`) joint encodings (added in
//!   round 214). Single-pair, subband-pair, and dispatch-predicate
//!   primitives transcribed verbatim from ETSI TS 102 114 V1.3.1
//!   Annex C §C.2.4 (PDF p.184).
//! - [`joint_subband_decode_range_i32`] /
//!   [`joint_subband_decode_range_f64`] /
//!   [`joint_subband_required`] / [`joint_source_channel`] — §C.2.3
//!   joint-subband copy + per-subband scale, the destination
//!   channel's high-end-subband reconstruction step from the source
//!   channel `JOINX[ch] - 1` over the subband range
//!   `[nSUBS[ch], nSUBS[nSourceCh])` (added in round 223). i32 and
//!   f64 variants, dispatch predicate, and source-channel resolver,
//!   transcribed verbatim from ETSI TS 102 114 V1.3.1 Annex C §C.2.3
//!   (PDF p.184).
//! - [`inverse_adpcm_decode_i32`] / [`inverse_adpcm_decode_f64`] /
//!   [`update_history_i32`] / [`update_history_f64`] /
//!   [`inverse_adpcm_required`] / [`NUM_ADPCM_COEFF`] — §C.2.2
//!   inverse-ADPCM predictor (added in round 228). Fixed-order
//!   (4-tap) FIR over the reconstructed signal, run per-subband
//!   whenever `PMODE == 1`. i32 and f64 decode variants, the
//!   rolling four-sample history slide between decode blocks, and
//!   the dispatch predicate, transcribed verbatim from ETSI TS 102
//!   114 V1.3.1 Annex C §C.2.2 (PDF p.183).
//! - [`decode_block_code`] / [`block_code_offset`] /
//!   [`block_code_max_code`] — §C.2.1 block-code modulus /
//!   integer-division decoder (added in round 232). Turns one
//!   mixed-radix code word into the array of quantisation indices
//!   the rest of the §C.2 chain consumes. Worked-example matched:
//!   `code=64`, `n_levels=3`, four elements → `[0, -1, 0, +1]`.
//!   Transcribed verbatim from ETSI TS 102 114 V1.3.1 Annex C §C.2.1
//!   (PDF p.182–183).
//! - [`decode_block_code_table`] / [`d6_book_for_levels`] /
//!   [`D6BlockBook`] / [`D6_BOOK_3`]..[`D6_BOOK_25`] /
//!   [`D6_BLOCK_ELEMENTS`] — the §C.2.1 *table-look-up* block-code
//!   decoder variant and the Annex D §D.6 "Block Code Books" it walks
//!   (added in round 309, the named round-232 follow-up). The seven
//!   §D.6 4-element books (`3/5/7/9/13/17/25` levels) are transcribed
//!   from ETSI TS 102 114 V1.3.1 Annex D §D.6 (PDF p.231–236); the
//!   decoder follows the §C.2.1 Table C-1 last-element-first walk and
//!   produces output identical to [`decode_block_code`] (cross-checked
//!   over the full §D.6 code domains).
//! - [`decode_core_frame`] / [`CoreStreamDecoder`] / [`SubframePcmDecoder`]
//!   — the §5.3/§5.4/§5.5 + §C.2.5 raw-bytes-to-PCM reconstruction.
//!   [`decode_core_frame`] decodes one frame with cleared filter history
//!   (single-frame semantics); [`CoreStreamDecoder`] persists the
//!   per-channel §C.2.5 filter tail across frames so a multi-frame
//!   elementary stream reconstructs without a per-frame filter-warmup
//!   transient. Round 356 validated [`CoreStreamDecoder`] against a
//!   black-box `ffmpeg -c:a dca` reference decode of the bundled 5-frame
//!   fixture: carrying the inter-frame filter tail makes channel-0 PCM
//!   shape-identical to the reference (Pearson correlation 1.0 over the
//!   whole stream) versus 0.73 with the filter reset per frame.
//! - [`DMIX_TABLE`] / [`INV_DMIX_TABLE`] / [`dmix_scale`] /
//!   [`inv_dmix_scale`] / [`decode_dmix_code`] — the §D.11 downmix
//!   scale-factor tables (Q15 `DmixTable`, 241 entries; Q16
//!   `InvDmixTbl`, 201 entries for `DmixTblIndex >= 40`) plus the
//!   §5.7.1 Table 5-31 9-bit dynamic-downmix coefficient-code
//!   resolution (phase MSB, one-biased low byte, `0` → exact `0.0`).
//!   Transcribed from ETSI TS 102 114 V1.3.1 §D.11 (PDF p.256-259)
//!   and unit-verified against the spec's own closed-form dB-ramp
//!   derivation. Added in round 406.
//! - [`dts_crc16`] / [`dts_crc16_update`] / [`DTS_CRC16_POLY`] /
//!   [`DTS_CRC16_INIT`] / [`DTS_CRC16_TABLE`] — the Annex B
//!   (normative) CRC-16 every DTS check word uses (CRC-CCITT:
//!   polynomial `0x1021`, init `0xFFFF`, MSB-first, no reflection, no
//!   final XOR), in one-shot, incremental, and compile-time-table
//!   forms. Per `docs/audio/dts/dts-crc16.md`. Added in round 408.
//! - [`scan_hf_vq_indices_at`] / [`unpack_hfreq_vq_entry`] /
//!   [`adpcm_vq_coeff`] + the `HFREQ_VQ_*` / `ADPCM_VQ_*` constants —
//!   the §D.10 VQ code books' wire facts (index widths, book sizes,
//!   vector lengths, the §D.10.2 two-signed-bytes ÷ 2⁴ / §D.10.1
//!   ÷ 2¹³ entry scalings), plus the structural §5.5 phase-1 index
//!   scanner. Added in round 408; the §D.10.2 divisor and intra-entry
//!   order corrected in round 439 per the staged recovery record
//!   (`docs/audio/dts/tables/dts-d10-2-hfreq-vq.meta.md`).
//! - [`HfVqCodebook`] / [`AdpcmVqCodebook`] / [`VqCodebooks`] +
//!   [`SubframePcmDecoder::set_vq_codebooks`] — the §D.10 code books
//!   and their decode paths. Rounds 408/434 landed the full HF-VQ
//!   (`nVQSUB < nSUBS`) and inverse-ADPCM (`PMODE != 0`, §C.2.2,
//!   [`AdpcmHistory`], §5.3.1 `HFLAG` frame gate) machinery behind a
//!   drop-in container while the books' numeric contents were the
//!   long-standing `docs/audio/dts/dts-d10-vq-tables-GAP.md` gap;
//!   round 439 ships the **real books built in**
//!   ([`VqCodebooks::builtin`], transcribed from the staged
//!   clean-room tables `docs/audio/dts/tables/dts-d10-*.csv`), the
//!   default of every decoder — §D.10 frames now decode out of the
//!   box, black-box-validated against a reference decode of the
//!   §D.10-bearing fixture.
//! - [`dts_dynrng_to_db`] / [`dts_dynrng_to_linear`] — the §5.4.1 /
//!   §5.7.2 DRC code resolution: the 8-bit `RANGE` /
//!   `subsubFrameDRC_Rev2AUX[]` byte is **signed Q2 two's-complement**
//!   (`dB = (int8)code × 0.25`, linear gain `10^(dB/20)`), per
//!   `docs/audio/dts/dts-drc-dynrng.md`. The §D.4 table
//!   ([`DRC_RANGE_MULTIPLIER`] / [`drc_range`]) remains available as
//!   reference data keyed by its offset-binary printed Index
//!   (`Index = signed_code + 127`) — do not raw-index it with wire
//!   codes. Added in round 408 (which also switched the
//!   `decode_core_frame` `DYNF` application and
//!   `Rev2Drc::multipliers` to the signed-Q2 function).
//! - [`find_aux_data`] / [`parse_aux_data`] / [`parse_aux_data_at`] /
//!   [`AuxData`] / [`DownmixType`] / [`DynamicDownmix`] — the §5.7.1
//!   Auxiliary Data chunk (Table 5-31): DWORD-aligned backward sync
//!   search (`0x9A1105A0`), the 36-bit decode time stamp with its
//!   `0b1011` markers, and the dynamic (embedded) downmix
//!   coefficient table resolved through §D.11
//!   ([`DynamicDownmix::coefficient_matrix`]) and applicable to
//!   planar PCM ([`DynamicDownmix::apply_planar`]). Also reachable
//!   from the frame iterator via `FrameView::aux_data`. Added in
//!   round 406.
//! - [`find_rev2_aux`] / [`parse_rev2_aux`] / [`parse_rev2_aux_at`] /
//!   [`Rev2AuxChunk`] / [`Rev2Drc`] — the §5.7.2 Rev2 Auxiliary Data
//!   Chunk (Table 5-33): DWORD-aligned backward sync search
//!   (`0x7004C070`), the embedded-ES downmix scale (§D.11), the
//!   size-gated broadcast metadata (per-subsubframe DRC values for
//!   version 1 + `DIALNORM_rev2aux`), and the size-located
//!   `nRev2AUXCRC16`. Also reachable via `FrameView::rev2_aux`.
//!   Added in round 406.
//! - [`Error`] — crate-local error type.
//!
//! Behind the default-on `registry` cargo feature (round 4):
//!
//! - [`register`] / [`register_codecs`] — wire the DTS decoder factory
//!   plus `dts` / `dtsc` FourCC tags into an
//!   [`oxideav_core::RuntimeContext`] / [`oxideav_core::CodecRegistry`].
//! - [`make_decoder`] — factory that builds a boxed
//!   [`oxideav_core::Decoder`] (the [`DtsDecoderHandle`]).
//! - [`DtsDecoderHandle`] — the decoder handle. `send_packet` eagerly
//!   parses the frame header (unpacking 14-bit container frames);
//!   `receive_frame` runs the full §5.3/§5.4/§5.5 + §C.2.5
//!   reconstruction — including §D.10 VQ/ADPCM frames via the
//!   built-in code books — to a planar S32 `AudioFrame`.
//! - [`probe_dts`] — standalone confidence helper (1.0 / 0.5 / 0.0).
//! - [`CODEC_ID_STR`] — canonical codec id `"dts"`.
//!
//! The crate `forbid`s `unsafe`.

#![forbid(unsafe_code)]
#![warn(missing_debug_implementations)]
#![warn(missing_docs)]

mod audio_array;
mod audio_data;
mod audio_header;
mod audio_huff;
mod aux_data;
mod bitreader;
mod block_code;
mod cos_mod;
mod crc16;
#[doc(hidden)]
pub mod d10_tables;
mod d10_vq;
mod d6_block_book;
mod dmix_coeff;
mod drc_range;
mod dsync;
mod filter_bank;
mod fir_coeff;
mod header;
mod inverse_adpcm;
mod iter;
mod join_scale;
mod joint_subband;
mod lfe_fir_coeff;
mod lfe_interp;
mod lfe_synth;
mod optional_info;
mod qmf_assemble;
mod qmf_multichannel;
mod qmf_synth;
mod rev2_aux;
mod side_info;
mod step_size;
mod subframe;
mod subframe_pcm;
mod sum_diff;
#[cfg(test)]
mod test_util;
mod unpack14;

#[cfg(feature = "registry")]
mod registry;

// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub use crate::audio_array::{
    decode_audio_data_subframe_at, decode_audio_data_subframe_partial_at,
    decode_audio_data_subframe_vq_at, decode_lfe_phase_at, AdpcmContext, AudioArrayDecodeError,
    AudioArrayError, HfVqFill, SubbandSampleMatrix,
};
// Stable: the persistent §C.2.2 reconstruction history a caller may
// inspect/reset around the recovered-§D.10 ADPCM decode path.
pub use crate::audio_array::AdpcmHistory;
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub use crate::audio_data::{
    audio_quant_type, terminal_sel_index, AudioQuantType, ABITS_MAX_BLOCK_CODE, ABITS_MAX_SEL,
    ABITS_TABLE_LEN, CODEBOOK_GROUP_SIZE, QUANT_LEVELS,
};
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub use crate::audio_header::{decode_audio_coding_header_at, AudioCodingHeader, SEL_PLANE_LEN};
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub use crate::audio_huff::{decode_audio_huff_at, AudioHuffCodebook};
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub use crate::aux_data::{
    find_aux_data, parse_aux_data, parse_aux_data_at, AuxData, DownmixType, DynamicDownmix,
    AUX_SYNC_WORD, AUX_TIME_STAMP_MARKER,
};
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub use crate::block_code::{block_code_max_code, block_code_offset, decode_block_code};
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub use crate::cos_mod::{
    cos_mod_stage, precal_cos_mod, COS_MOD_BLOCK1_START, COS_MOD_BLOCK2_START,
    COS_MOD_BLOCK3_START, COS_MOD_BLOCK4_START, COS_MOD_LEN, NUM_SUBBAND,
};
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub use crate::crc16::{
    dts_crc16, dts_crc16_update, DTS_CRC16_INIT, DTS_CRC16_POLY, DTS_CRC16_TABLE,
};
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub use crate::d10_vq::{
    adpcm_vq_coeff, scan_hf_vq_indices_at, unpack_hfreq_vq_entry, ADPCM_VQ_BOOK_SIZE,
    ADPCM_VQ_COEFF_DIVISOR, ADPCM_VQ_INDEX_BITS, ADPCM_VQ_VECTOR_LEN, HFREQ_VQ_BOOK_SIZE,
    HFREQ_VQ_ELEMENT_DIVISOR, HFREQ_VQ_ENTRIES_PER_VECTOR, HFREQ_VQ_INDEX_BITS,
    HFREQ_VQ_VECTOR_LEN,
};
// Stable: the §D.10 VQ code books. `VqCodebooks::builtin()` (the
// decoder default) carries the real books, transcribed from the
// staged clean-room tables; the caller-supplied constructors remain.
pub use crate::d10_vq::{AdpcmVqCodebook, HfVqCodebook, VqCodebookShapeError, VqCodebooks};
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub use crate::d6_block_book::{
    d6_book_for_levels, decode_block_code_table, D6BlockBook, D6_BLOCK_ELEMENTS, D6_BOOK_13,
    D6_BOOK_17, D6_BOOK_25, D6_BOOK_3, D6_BOOK_5, D6_BOOK_7, D6_BOOK_9,
};
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub use crate::dmix_coeff::{
    decode_dmix_code, dmix_scale, inv_dmix_scale, DMIX_TABLE, DMIX_TABLE_LEN,
    DMIX_TABLE_UNITY_INDEX, INV_DMIX_INDEX_OFFSET, INV_DMIX_TABLE, INV_DMIX_TABLE_LEN,
};
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub use crate::drc_range::{
    drc_range, dts_dynrng_to_db, dts_dynrng_to_linear, DRC_RANGE_LEN, DRC_RANGE_MULTIPLIER,
    DRC_RANGE_UNITY_INDEX,
};
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub use crate::dsync::{decode_dsync_at, dsync_present, DSYNC_WIRE_BITS, DSYNC_WORD};
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub use crate::filter_bank::FilterBankSelection;
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub use crate::fir_coeff::{FIR_COEFF_LEN, RA_COEFF_LOSSLESS, RA_COEFF_LOSSY};
pub use crate::header::{
    encode_frame_header_14bit_be, encode_frame_header_14bit_le, encode_frame_header_be,
    encode_frame_header_le, parse_frame_header, parse_frame_header_14bit, AmodeArrangement,
    DialogNormalization, DtsFrameHeader, FrameType, LfeMode, SampleFrequency, SourcePcmResolution,
    SyncWordEncoding, TargetedBitRate,
};
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub use crate::inverse_adpcm::{
    inverse_adpcm_decode_f64, inverse_adpcm_decode_i32, inverse_adpcm_required, update_history_f64,
    update_history_i32, NUM_ADPCM_COEFF,
};
pub use crate::iter::{
    find_all_syncs, find_next_sync, iter_frames, iter_frames_14bit, iter_frames_resync, iter_syncs,
    FrameIterator, FrameIterator14, FrameIteratorResync, FrameView, FrameView14, ResyncCause,
    ResyncEvent, SyncIterator, SyncMatch,
};
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub use crate::join_scale::{
    join_scale, JOIN_SCALE_FACTOR, JOIN_SCALE_LEN, JOIN_SCALE_UNITY_INDEX,
};
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub use crate::joint_subband::{
    joint_source_channel, joint_subband_decode_range_f64, joint_subband_decode_range_i32,
    joint_subband_required,
};
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub use crate::lfe_fir_coeff::{LFE_FIR_COEFF_LEN, RA_COEFF_LFE128, RA_COEFF_LFE64};
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub use crate::lfe_interp::LfeInterpolationSelection;
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub use crate::lfe_synth::{
    LfeChannel, LfeChannelError, LfeInterpError, LfeInterpolator, LFE_HISTORY_LEN, LFE_SCALE_STEP,
};
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub use crate::optional_info::{decode_optional_info_at, OptionalInfo, MAX_AUX_BYTE_COUNT};
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub use crate::qmf_assemble::{
    assemble_xin, fir_step, shift_x_history, shift_z_output, write_pcm_output, QmfAssembleError,
    PCM_OUTPUT_PER_SAMPLE, X_HISTORY_LEN, Z_OUTPUT_LEN,
};
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub use crate::qmf_multichannel::{MultiChannelQmf, MultiChannelQmfError};
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub use crate::qmf_synth::QmfSynthesis;
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub use crate::rev2_aux::{
    find_rev2_aux, parse_rev2_aux, parse_rev2_aux_at, Rev2AuxChunk, Rev2Drc, REV2_AUX_SYNC_WORD,
    REV2_DRC_VERSION_SINGLE_BAND,
};
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub use crate::side_info::{
    decode_abits_at, decode_adj_at, decode_join_scale_at, decode_scales_at,
    decode_subsubframe_count_at, decode_tmode_at, AbitsCodebook, ScaleFactorAdjustment,
    ScalesCodebook, SubsubframeCount, TmodeCodebook, RMS_6BIT, RMS_7BIT,
};
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub use crate::step_size::{
    dequant_scale, dequant_subsubframe, scale_subsubframe_samples, transient_scale_index,
    StepSizeTable, RATE_LOSSLESS, SAMPLES_PER_SUBSUBFRAME, STEP_SIZE_FIRST_INVALID,
    STEP_SIZE_LOSSLESS, STEP_SIZE_LOSSY, STEP_SIZE_SCALE_SHIFT, STEP_SIZE_TABLE_LEN,
};
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub use crate::subframe::{
    decode_primary_side_info_at, decode_primary_side_info_tail_at, ChannelSideInfo,
    ChannelSideInfoParams, PrimarySideInfo, SideInfoTail, MAX_PRIMARY_CHANNELS,
};
pub use crate::subframe_pcm::{
    decode_core_frame, decode_core_frame_with_info, CoreFrameDecodeError, CoreStreamDecoder,
    Subframe, SubframePcm, SubframePcmDecoder, SubframePcmError, PCM_PER_SUBBAND_ROW,
};
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub use crate::sum_diff::{
    front_sum_difference_required, sum_difference_decode_f64, sum_difference_decode_i32,
    sum_difference_decode_subband_pair_f64, sum_difference_decode_subband_pair_i32,
    surround_sum_difference_required,
};
pub use crate::unpack14::{pack_16bit_to_14bit, unpack_14bit_to_16bit, FourteenBitByteOrder};

#[cfg(feature = "registry")]
pub use crate::registry::{
    make_decoder, probe_dts, register, register_codecs, DtsDecoderHandle, CODEC_ID_STR,
};

// `oxideav_core::register!("dts", register)` lives inside the
// `registry` submodule; its `__oxideav_entry` wrapper needs to be
// reachable at the crate root so `oxideav-meta`'s build-time
// discovery (which calls `<crate>::__oxideav_entry(ctx)`) finds it.
// internal — exposed for tests/fuzz; not part of the stable API
#[cfg(feature = "registry")]
#[doc(hidden)]
pub use crate::registry::__oxideav_entry;

/// Crate-local error type. Round 1 surfaces only the parser-related
/// variants; future rounds will extend this enum as decoding stages
/// land.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// The input buffer was too short for the field being read.
    UnexpectedEof,
    /// None of the four documented DTS sync words matched the first
    /// 4–5 bytes of the input.
    NoSync,
    /// A 14-bit DTS sync was detected at the 16-bit-input entry
    /// point [`parse_frame_header`]. Round 2 added a dedicated
    /// [`parse_frame_header_14bit`] entry point plus
    /// [`unpack_14bit_to_16bit`] for callers that want to convert
    /// 14-bit-packed bytes into the raw-BE form. This variant
    /// remains for callers that route by sync up-front.
    UnsupportedFourteenBit,
    /// A raw 16-bit DTS sync was detected at the 14-bit-input entry
    /// point [`crate::iter_frames_14bit`]. Symmetric counterpart to
    /// [`Self::UnsupportedFourteenBit`]: the 14-bit iterator only
    /// walks 14-bit-packed container streams, so a raw-16-bit sync at
    /// the cursor is out-of-domain. Callers walking raw 16-bit input
    /// should switch to [`crate::iter_frames`].
    UnsupportedRaw16Bit,
    /// The decoded `NBLKS` field reported fewer than 5 blocks per
    /// frame — the wiki/spec disallow this.
    BlockCountOutOfRange {
        /// Decoded number of blocks (after the +1 increment).
        blocks: u8,
    },
    /// The decoded frame-size field reported fewer than 95 bytes —
    /// the wiki/spec disallow this.
    FrameSizeOutOfRange {
        /// Decoded frame size in bytes (after the +1 increment).
        frame_size: u16,
    },
    /// A field passed to [`crate::encode_frame_header_be`] does not
    /// fit the bit-width the wiki bit-table documents (e.g. AMODE > 63
    /// for a 6-bit field, VERSION > 15 for a 4-bit field, or a
    /// `header_crc: Some(_)` paired with `crc_present == false`).
    /// Only the encoder returns this variant; the parser cannot
    /// produce out-of-range values because every field is read from
    /// the bit-vector through a width-bounded read.
    FieldOutOfRange {
        /// Static name of the offending [`crate::DtsFrameHeader`]
        /// field.
        field: &'static str,
        /// Caller-supplied value.
        value: u32,
        /// Maximum value the field's documented bit-width can hold.
        /// For the `header_crc` mismatch variant this is set to `0`
        /// (the value being out-of-range is the `Some` vs `None`
        /// disagreement with `crc_present`, not the integer payload).
        max: u32,
    },
    /// A Primary Audio Coding Side Information field
    /// (§5.4.1 Table 5-28) carried a value the ETSI spec marks as
    /// reserved/invalid: a `BHUFF` / `SHUFF` selector equal to `7`,
    /// a `SCALES` accumulator that walked outside the documented
    /// range of the §D.1.1 / §D.1.2 RMS square-root table (e.g.
    /// index 63 in the 6-bit table, or indices 125..=127 in the
    /// 7-bit table, both written as "invalid" in the staged PDF),
    /// or a [`crate::decode_primary_side_info_at`] loop bound
    /// outside its §5.3.2 range (`"nPCHS"` above 5 per PDF p.25,
    /// `"nSUBS"` above the `NumSubband = 32` cap, or `"VQSUB"`
    /// above the channel's `nSUBS`).
    InvalidSideInfo {
        /// Static name of the offending side-info field: `"BHUFF"`,
        /// `"SHUFF"`, `"SCALES"`, `"nPCHS"`, `"nSUBS"`, or
        /// `"VQSUB"`.
        field: &'static str,
        /// The reserved value the bit stream carried.
        value: u32,
    },
    /// The bit stream's prefix did not match any entry in the named
    /// Annex D Huffman codebook (`A12`, `B12`, `C12`, `D12`, `E12`,
    /// `A5`, `B5`, `C5`, `A7`, `B7`, `A4`, `B4`, `C4`, `D4`) within
    /// the maximum documented code length. Surfaced by the ABITS /
    /// SCALES / TMODE side-info decoders when the input is
    /// structurally corrupt or the wrong codebook was selected
    /// upstream.
    HuffmanDecodeFailed {
        /// Static name of the codebook that failed to match.
        table: &'static str,
    },
    /// The left and right slice arguments to a §C.2.4 sum/difference
    /// decoder ([`crate::sum_difference_decode_i32`],
    /// [`crate::sum_difference_decode_f64`], or one of their
    /// subband-pair counterparts) had different lengths. The §C.2.4
    /// pseudocode requires a one-to-one pairing of left- and right-
    /// channel samples.
    SumDiffLengthMismatch {
        /// Length of the left-channel slice.
        left_len: usize,
        /// Length of the right-channel slice.
        right_len: usize,
    },
    /// The slice arguments to a §C.2.3 joint-subband decoder
    /// ([`crate::joint_subband_decode_range_i32`] or
    /// [`crate::joint_subband_decode_range_f64`]) violated one of the
    /// §C.2.3 pseudocode's structural invariants: `n_subs_dst >
    /// n_subs_src` (the imported range would run backwards), a
    /// per-channel subband array shorter than `n_subs_src` (no
    /// storage for the imported range), a `scales` slice whose length
    /// disagrees with `n_subs_src - n_subs_dst`, or a per-subband
    /// destination/source sample-length disagreement. `dst_len` /
    /// `src_len` carry the lengths that disagreed (the meaning is
    /// context-dependent: see the documented invariants on each
    /// constructor site).
    JointSubbandShapeMismatch {
        /// Length-like value on the destination side of the disagreement.
        dst_len: usize,
        /// Length-like value on the source side of the disagreement.
        src_len: usize,
    },
    /// A slice argument to a §C.2.2 inverse-ADPCM decoder
    /// ([`crate::inverse_adpcm_decode_i32`] or
    /// [`crate::inverse_adpcm_decode_f64`]) had a length that disagrees
    /// with the spec's fixed `NumADPCMCoeff = 4` invariant. The
    /// predictor requires a four-sample history (carrying
    /// `raSample[-4..0]`) and four ADPCM coefficients
    /// (`raADPCMCoeff[0..4]`); either slice having any other length is
    /// out-of-domain.
    InverseAdpcmShapeMismatch {
        /// Caller-supplied history-buffer length (spec requires 4).
        history_len: usize,
        /// Caller-supplied coefficient-array length (spec requires 4).
        coeffs_len: usize,
    },
    /// The `n_levels` argument to [`crate::decode_block_code`] was
    /// less than 2. A one-level alphabet has only the index `0` and
    /// the §C.2.1 mixed-radix recurrence is undefined for
    /// `n_levels < 2` (division by zero / one would never advance).
    BlockCodeLevelsOutOfRange {
        /// Caller-supplied `n_levels` value (spec requires `>= 2`).
        n_levels: u32,
    },
    /// A §C.2.1 block-code word produced a non-zero residual after
    /// consuming every output element via the spec's
    /// `nCode % nNumLevel` / `nCode /= nNumLevel` recurrence. The
    /// spec text treats this as a fatal "ERROR: block code look-up
    /// fail" condition (PDF p.183); the Rust API surfaces it as a
    /// recoverable error carrying the residual + block dimensions
    /// so the caller can distinguish bit-stream corruption from a
    /// structural decoder bug.
    BlockCodeResidual {
        /// The residual code-word value after the last extraction.
        residual: u32,
        /// Element count passed to the decoder (`output.len()`).
        n_elements: usize,
        /// Quantisation-level count passed to the decoder.
        n_levels: u32,
    },
    /// An `ABITS` bit-allocation index passed to the §D.2
    /// quantization-step-size lookup ([`crate::StepSizeTable::step_size`]
    /// or a `dequant_*` composer) was one of the `27..=31` indices the
    /// staged §D.2.1 / §D.2.2 tables write "invalid" (PDF p.193-194),
    /// or was `>= 32`. A defined `ABITS` index is `0..=26`.
    InvalidStepSize {
        /// The reserved / out-of-range `ABITS` index.
        abits: u8,
    },
    /// A slice argument to a §5.5 subsubframe dequantizer
    /// ([`crate::scale_subsubframe_samples`] /
    /// [`crate::dequant_subsubframe`]) was not exactly
    /// [`crate::SAMPLES_PER_SUBSUBFRAME`] (= 8) samples long. The §5.5
    /// `Audio Data` block scales one subband analysis subwindow of
    /// eight samples per call (PDF p.31-32).
    SampleCountMismatch {
        /// The number of samples the §5.5 loop requires (always 8).
        expected: usize,
        /// The slice length the caller actually supplied.
        found: usize,
    },
    /// The bytes at the offset handed to
    /// [`crate::parse_aux_data_at`] were not the §5.7.1 DWORD-aligned
    /// auxiliary-data sync word `0x9A1105A0` (`nSYNCAUX`).
    AuxSyncMismatch {
        /// The 32-bit word actually read at the offset.
        found: u32,
    },
    /// One of the two 4-bit markers bracketing the §5.7.1 Table 5-31
    /// 36-bit `nAUXTimeStamp` halves was not the documented `0b1011`
    /// (`nMaker==1011`), indicating a false-positive sync match or a
    /// corrupt auxiliary-data chunk.
    AuxTimeStampMarkerMismatch {
        /// The 4-bit value actually read.
        found: u8,
    },
    /// The §5.7.1 `DeriveNumDwnMixCodeCoeffs()` input-channel count
    /// (`nPriCh = anNumCh[AMODE]`) could not be resolved because the
    /// frame's `AMODE` is a user-defined code (`16..=63`) whose
    /// channel count Table 5-4 does not define.
    AuxChannelCountUnresolved {
        /// The unresolvable 6-bit `AMODE` code.
        amode: u8,
    },
    /// The bytes at the offset handed to
    /// [`crate::parse_rev2_aux_at`] were not the §5.7.2 DWORD-aligned
    /// Rev2 auxiliary sync word `0x7004C070` (`nSYNCRev2AUX`).
    Rev2AuxSyncMismatch {
        /// The 32-bit word actually read at the offset.
        found: u32,
    },
    /// The §5.7.2 `nRev2AUXDataByteSize` field was outside its valid
    /// `3..=128` range ("Error: Invalid range of Rev 2 Auxiliary Data
    /// Chunk Size"), or the chunk's declared fields overran the
    /// size-located `nRev2AUXCRC16` position.
    Rev2AuxSizeOutOfRange {
        /// The decoded chunk byte size (after the `+1` increment).
        size: u8,
    },
    /// The §5.7.2 `nEmbESDownMixScaleIndex` was outside its valid
    /// `40..=240` range (the encode side limits the `ESDmixScale`
    /// parameters to `[-40 dB, 0 dB]`).
    Rev2AuxEsScaleIndexOutOfRange {
        /// The out-of-range 8-bit index.
        index: u8,
    },
    /// The §5.7.2 per-subsubframe DRC value count could not be
    /// derived: one 8-bit value is transmitted per 256-sample
    /// subsubframe (Table 5-34), but the frame's block count is not a
    /// whole number of subsubframes (`32·(NBLKS+1)` not a multiple of
    /// 256).
    Rev2AuxDrcCountUnresolved {
        /// The frame's `NBLKS + 1` block count.
        blocks: u8,
    },
    /// A §5.7.1 Table 5-31 dynamic-downmix coefficient code word
    /// passed to [`crate::decode_dmix_code`] was wider than the
    /// documented 9-bit field, or its one-biased low byte resolved
    /// past the end of the 241-entry §D.11 `DmixTable` (the
    /// pseudocode's `if (nTmp > nTblSize) return false;` arm).
    DownmixCodeOutOfRange {
        /// The offending 9-bit (or wider) code word.
        code: u16,
    },
    /// The planar PCM handed to
    /// [`crate::DynamicDownmix::apply_planar`] did not match the
    /// coefficient table's shape: the plane count must equal the
    /// §5.7.1 `nPriCh` input channel count, and every plane must have
    /// the same sample count.
    DownmixInputShapeMismatch {
        /// The expected plane count (or, for the unequal-length case,
        /// the first plane's sample count).
        expected: usize,
        /// The offending count actually supplied.
        found: usize,
    },
    /// The §5.5 Table 5-29 `DSYNC` subsubframe synchronization check
    /// word ([`crate::decode_dsync_at`]) read a 16-bit value other than
    /// `0xffff` (PDF p.32: `if ( DSYNC != 0xffff )`). The spec text only
    /// emits a diagnostic and keeps decoding; this crate surfaces it as
    /// a recoverable typed error carrying the bad word and the
    /// zero-based subsubframe index it trailed, mirroring the spec's
    /// `"DSYNC error at end of subsubframe #%d"` message — it is the
    /// only in-band integrity check the Core profile provides for the
    /// audio-data array.
    DsyncMismatch {
        /// The 16-bit value read where `0xffff` was expected.
        found: u16,
        /// The zero-based `nSubSubFrame` index the trailer followed.
        n_subsubframe: u8,
    },
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Error::UnexpectedEof => write!(f, "oxideav-dts: unexpected end of input"),
            Error::NoSync => {
                write!(f, "oxideav-dts: no DTS sync word found at offset 0")
            }
            Error::UnsupportedFourteenBit => write!(
                f,
                "oxideav-dts: 14-bit DTS sync detected at the 16-bit-input \
                 entry point; call parse_frame_header_14bit (or \
                 unpack_14bit_to_16bit + parse_frame_header) instead"
            ),
            Error::UnsupportedRaw16Bit => write!(
                f,
                "oxideav-dts: raw 16-bit DTS sync detected at the \
                 14-bit-input entry point; call iter_frames (or \
                 parse_frame_header) instead"
            ),
            Error::BlockCountOutOfRange { blocks } => write!(
                f,
                "oxideav-dts: NBLKS={blocks} is out of the documented 5..=128 \
                 range (spec mandates >=5)"
            ),
            Error::FrameSizeOutOfRange { frame_size } => write!(
                f,
                "oxideav-dts: frame size {frame_size} B is out of the documented \
                 95..=16384 range (spec mandates >=95)"
            ),
            Error::FieldOutOfRange { field, value, max } => write!(
                f,
                "oxideav-dts: field `{field}` value {value} exceeds the wiki \
                 bit-table maximum {max}"
            ),
            Error::InvalidSideInfo { field, value } => write!(
                f,
                "oxideav-dts: side-info field `{field}` value {value} is \
                 reserved/invalid per ETSI TS 102 114 §5.4.1 (Table 5-24/5-25 \
                 selector 7, or §D.1.1/§D.1.2 RMS table index marked invalid)"
            ),
            Error::HuffmanDecodeFailed { table } => write!(
                f,
                "oxideav-dts: bit stream did not match any entry in Annex D \
                 Huffman codebook `{table}` within the documented maximum \
                 code length"
            ),
            Error::SumDiffLengthMismatch {
                left_len,
                right_len,
            } => write!(
                f,
                "oxideav-dts: §C.2.4 sum/difference decode requires matched \
                 left/right slice lengths; got left={left_len} right={right_len}"
            ),
            Error::JointSubbandShapeMismatch { dst_len, src_len } => write!(
                f,
                "oxideav-dts: §C.2.3 joint-subband decode shape mismatch; got \
                 dst-side={dst_len} src-side={src_len} (see ETSI TS 102 114 \
                 §C.2.3 for the structural invariants)"
            ),
            Error::InverseAdpcmShapeMismatch {
                history_len,
                coeffs_len,
            } => write!(
                f,
                "oxideav-dts: §C.2.2 inverse-ADPCM decode requires a 4-sample \
                 history and 4 ADPCM coefficients (NumADPCMCoeff = 4); got \
                 history_len={history_len} coeffs_len={coeffs_len}"
            ),
            Error::BlockCodeLevelsOutOfRange { n_levels } => write!(
                f,
                "oxideav-dts: §C.2.1 block-code decode requires n_levels >= 2; \
                 got n_levels={n_levels}"
            ),
            Error::BlockCodeResidual {
                residual,
                n_elements,
                n_levels,
            } => write!(
                f,
                "oxideav-dts: §C.2.1 block-code decode residual {residual} != 0 \
                 after extracting {n_elements} element(s) from a base-{n_levels} \
                 block (code-word out of range for the declared block dimensions)"
            ),
            Error::InvalidStepSize { abits } => write!(
                f,
                "oxideav-dts: §D.2 quantization step-size lookup: ABITS index \
                 {abits} is reserved/invalid (defined range is 0..=26)"
            ),
            Error::SampleCountMismatch { expected, found } => write!(
                f,
                "oxideav-dts: §5.5 subsubframe dequantization expects {expected} \
                 samples per subband analysis subwindow, got {found}"
            ),
            Error::AuxSyncMismatch { found } => write!(
                f,
                "oxideav-dts: §5.7.1 auxiliary-data parse expected the \
                 DWORD-aligned sync word 0x9a1105a0, read 0x{found:08x}"
            ),
            Error::AuxTimeStampMarkerMismatch { found } => write!(
                f,
                "oxideav-dts: §5.7.1 auxiliary time-stamp marker mismatch: \
                 read 0b{found:04b}, expected 0b1011"
            ),
            Error::AuxChannelCountUnresolved { amode } => write!(
                f,
                "oxideav-dts: §5.7.1 dynamic-downmix coefficient count cannot \
                 be derived: AMODE {amode} is a user-defined channel \
                 arrangement with no Table 5-4 channel count"
            ),
            Error::Rev2AuxSyncMismatch { found } => write!(
                f,
                "oxideav-dts: §5.7.2 Rev2 auxiliary-data parse expected the \
                 DWORD-aligned sync word 0x7004c070, read 0x{found:08x}"
            ),
            Error::Rev2AuxSizeOutOfRange { size } => write!(
                f,
                "oxideav-dts: §5.7.2 Rev2 auxiliary chunk size {size} B is \
                 invalid (valid range 3..=128, and the declared fields must \
                 fit in front of the size-located CRC)"
            ),
            Error::Rev2AuxEsScaleIndexOutOfRange { index } => write!(
                f,
                "oxideav-dts: §5.7.2 embedded-ES downmix scale index {index} \
                 is outside the valid 40..=240 range ([-40 dB, 0 dB])"
            ),
            Error::Rev2AuxDrcCountUnresolved { blocks } => write!(
                f,
                "oxideav-dts: §5.7.2 Rev2AUX DRC value count cannot be \
                 derived: {blocks} blocks per frame is not a whole number of \
                 256-sample subsubframes"
            ),
            Error::DownmixCodeOutOfRange { code } => write!(
                f,
                "oxideav-dts: §5.7.1 dynamic-downmix coefficient code 0x{code:03x} \
                 is out of domain (must be a 9-bit word whose one-biased low byte \
                 indexes the 241-entry §D.11 DmixTable)"
            ),
            Error::DownmixInputShapeMismatch { expected, found } => write!(
                f,
                "oxideav-dts: §5.7.1 downmix fold shape mismatch: expected \
                 {expected}, got {found} (plane count must equal the nPriCh \
                 input channel count and all planes must be equal-length)"
            ),
            Error::DsyncMismatch {
                found,
                n_subsubframe,
            } => write!(
                f,
                "oxideav-dts: §5.5 DSYNC error at end of subsubframe #{n_subsubframe}: \
                 read 0x{found:04x}, expected 0xffff"
            ),
        }
    }
}

impl std::error::Error for Error {}

/// Convenience alias for [`Result`] specialised to this crate's
/// [`Error`].
pub type Result<T> = core::result::Result<T, Error>;
