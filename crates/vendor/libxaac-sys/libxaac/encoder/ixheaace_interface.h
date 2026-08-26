/******************************************************************************
 *                                                                            *
 * Copyright (C) 2023 The Android Open Source Project
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at:
 *
 * http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 *
 *****************************************************************************
 * Originally developed and contributed by Ittiam Systems Pvt. Ltd, Bangalore
 */

#pragma once
enum { MS_NONE = 0, MS_SOME = 1, MS_ALL = 2 };

enum { MS_ON = 1 };

typedef struct {
  WORD32 ms_digest; /* 0 = no MS; 1 = some MS, 2 = all MS */
  WORD32 ms_mask[MAXIMUM_GROUPED_SCALE_FACTOR_BAND];
} ixheaace_tools_info;

typedef struct {
  WORD32 sfb_count;
  WORD32 sfb_per_group;
  WORD32 max_sfb_per_grp;
  WORD32 window_sequence;
  WORD32 window_shape;
  WORD32 grouping_mask;
  WORD32 sfb_offsets[MAXIMUM_GROUPED_SCALE_FACTOR_BAND + 1];
  FLOAT32 *ptr_sfb_energy;
  FLOAT32 *ptr_sfb_spread_energy;
  FLOAT32 *ptr_sfb_thr;
  FLOAT32 *ptr_spec_coeffs;
  FLOAT32 sfb_sum_ms_energy;
  FLOAT32 sfb_sum_lr_energy;
  FLOAT32 pe;
  FLOAT32 sfb_min_snr[MAXIMUM_GROUPED_SCALE_FACTOR_BAND];
  WORD32 ms_digest; /* 0 = no MS; 1 = some MS, 2 = all MS */
  WORD32 ms_used[MAXIMUM_GROUPED_SCALE_FACTOR_BAND];
  ixheaace_temporal_noise_shaping_params tns_info;
} ixheaace_psy_out_channel;

typedef struct {
  ixheaace_tools_info tools_info;
  FLOAT32 weight_ms_lr_pe_ratio;
} ixheaace_psy_out_element;

typedef struct {
  /* information shared by both channels  */
  ixheaace_psy_out_element psy_out_element;
  /* information specific to each channel */
  ixheaace_psy_out_channel *psy_out_ch[IXHEAACE_MAX_CH_IN_BS_ELE];
} ixheaace_psy_out;

VOID ia_enhaacplus_enc_build_interface(
    FLOAT32 *ptr_grouped_mdct_spec, ixheaace_sfb_energy *ptr_grouped_sfb_thres,
    ixheaace_sfb_energy *ptr_grouped_sfb_nrg, ixheaace_sfb_energy *ptr_grouped_sfb_spreaded_nrg,
    const ixheaace_sfb_energy_sum sfb_nrg_sum_lr, const ixheaace_sfb_energy_sum sfb_nrg_sum_ms,
    const WORD32 win_seq, const WORD32 win_shape, const WORD32 grouped_sfb_cnt,
    const WORD32 *ptr_grouped_sfb_offset, const WORD32 max_sfb_per_grp,
    const FLOAT32 *ptr_grouped_sfb_min_snr, const WORD32 total_groups_cnt,
    const WORD32 *ptr_group_len, ixheaace_psy_out_channel *ptr_psy_out_ch);
