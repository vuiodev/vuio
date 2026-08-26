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

#define IXHEAACE_NF_SMOOTHING_LENGTH (4)

typedef struct {
  FLOAT32 prev_noise_lvls[IXHEAACE_NF_SMOOTHING_LENGTH][MAXIMUM_NUM_NOISE_VALUES];
  WORD32 s_freq_qmf_band_tbl[MAXIMUM_NUM_NOISE_VALUES + 1];
  WORD32 maxi_levl_fix;
  FLOAT32 weight_fac;
  FLOAT32 max_level;
  WORD32 num_of_noise_bands;
  WORD32 noise_groups;
  const FLOAT32 *ptr_smooth_filter;
  ixheaace_invf_mode thr_offset;
} ixheaace_str_noise_flr_est_sbr;

typedef ixheaace_str_noise_flr_est_sbr *ixheaace_pstr_noise_flr_est_sbr;

VOID ixheaace_sbr_noise_floor_estimate_qmf(
    ixheaace_pstr_noise_flr_est_sbr pstr_noise_floor_est_sbr,
    const ixheaace_str_frame_info_sbr *ptr_frame_info, FLOAT32 *ptr_noise_lvls,
    FLOAT32 **ptr_quota_orig, WORD8 *ptr_idx_vx, WORD32 missing_harmonics_flag,
    WORD32 start_index, WORD32 transient_flag, ixheaace_invf_mode *ptr_inv_filt_levels,
    WORD32 is_ld_sbr);

IA_ERRORCODE
ixheaace_create_sbr_noise_floor_estimate(ixheaace_pstr_noise_flr_est_sbr pstr_noise_floor_est_sbr,
                                         WORD32 ana_max_level, const UWORD8 *ptr_freq_band_tab,
                                         WORD32 num_scf, WORD32 noise_groups,
                                         UWORD32 use_speech_config,
                                         ixheaace_str_qmf_tabs *ptr_qmf_tab);

IA_ERRORCODE
ixheaace_reset_sbr_noise_floor_estimate(ixheaace_pstr_noise_flr_est_sbr pstr_noise_floor_est_sbr,
                                        const UWORD8 *ptr_freq_band_tab, WORD32 num_scf);
