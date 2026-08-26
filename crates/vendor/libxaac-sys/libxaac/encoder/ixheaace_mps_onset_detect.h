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
struct ixheaace_mps_onset_detect {
  WORD8 avg_energy_dist_scale;
  WORD32 max_time_slots;
  WORD32 min_trans_dist;
  WORD32 avg_energy_dist;
  WORD32 lower_bound_onset_detection;
  WORD32 upper_bound_onset_detection;
  FLOAT32 *p_energy_hist;
};

struct ixheaace_mps_onset_detect_config {
  WORD32 max_time_slots;
  WORD32 lower_bound_onset_detection;
  WORD32 upper_bound_onset_detection;
};

typedef struct ixheaace_mps_onset_detect ixheaace_mps_onset_detect,
    *ixheaace_mps_pstr_onset_detect;

typedef struct ixheaace_mps_onset_detect_config ixheaace_mps_onset_detect_config,
    *ixheaace_mps_pstr_onset_detect_config;

IA_ERRORCODE ixheaace_mps_212_onset_detect_init(
    ixheaace_mps_pstr_onset_detect pstr_onset_detect,
    const ixheaace_mps_pstr_onset_detect_config p_onset_detect_config, const UWORD32 init_flags);

IA_ERRORCODE
ixheaace_mps_212_onset_detect_update(ixheaace_mps_pstr_onset_detect pstr_onset_detect,
                                     const WORD32 time_slots);
IA_ERRORCODE ixheaace_mps_212_onset_detect_apply(
    ixheaace_mps_pstr_onset_detect pstr_onset_detect, const WORD32 num_time_slots,
    ixheaace_cmplx_str pp_complex_hybrid_data[MAX_ANA_TIME_SLOT][MAX_QMF_BANDS],
    const WORD32 prev_pos, WORD32 p_transient_pos[MAX_NUM_TRANS]);
