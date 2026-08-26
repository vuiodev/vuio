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

#define IXHEAACE_MAX_NUM_REGIONS 10

typedef struct {
  FLOAT32 org_quota_mean[IXHEAACE_INVF_SMOOTHING_LENGTH + 1];
  FLOAT32 sbr_quota_mean[IXHEAACE_INVF_SMOOTHING_LENGTH + 1];

  FLOAT32 org_quota_mean_filt;
  FLOAT32 sbr_quota_mean_filt;
  FLOAT32 avg_energy;
} ixheaace_str_detector_values;

typedef struct {
  WORD32 prev_region_sbr[MAXIMUM_NUM_NOISE_VALUES];
  WORD32 prev_region_orig[MAXIMUM_NUM_NOISE_VALUES];
  WORD32 freq_band_tab_inv_filt[MAXIMUM_NUM_NOISE_VALUES];
  WORD32 no_detector_bands;
  WORD32 no_detector_bands_max;
  const ixheaace_str_det_params *ptr_detector_params;
  ixheaace_invf_mode prev_invf_mode[MAXIMUM_NUM_NOISE_VALUES];
  ixheaace_str_detector_values detector_values[MAXIMUM_NUM_NOISE_VALUES];
} ixheaace_str_sbr_inv_filt_est;

typedef ixheaace_str_sbr_inv_filt_est *ixheaace_pstr_sbr_inv_filt_est;

VOID ixheaace_qmf_inverse_filtering_detector(ixheaace_pstr_sbr_inv_filt_est pstr_inv_filt,
                                             FLOAT32 **ptr_quota_mtx, FLOAT32 *ptr_energy_vec,
                                             WORD8 *ptr_idx_vx, WORD32 start_index,
                                             WORD32 stop_index, WORD32 transient_flag,
                                             ixheaace_invf_mode *ptr_inf_vec, WORD32 is_ld_sbr);

VOID ixheaace_create_inv_filt_detector(ixheaace_pstr_sbr_inv_filt_est pstr_inv_filt,
                                       WORD32 *ptr_freq_band_tab_detector, WORD32 num_det_bands,
                                       UWORD32 use_speech_config,
                                       ixheaace_str_qmf_tabs *ptr_qmf_tab);
