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

typedef enum { IXHEAACE_SBR_AMP_RES_1_5 = 0, IXHEAACE_SBR_AMP_RES_3_0 } ixheaace_amp_res;

typedef enum { IXHEAACE_SINGLE_RATE, IXHEAACE_DUAL_RATE, IXHEAACE_QUAD_RATE } ixheaace_sr_mode;

typedef struct ixheaace_str_sbr_hdr_data {
  WORD32 protocol_version;
  ixheaace_amp_res sbr_amp_res;
  WORD32 sbr_start_freq;
  WORD32 sbr_stop_freq;
  WORD32 sbr_xover_band;
  WORD32 sbr_noise_bands;
  WORD32 sbr_data_extra;
  WORD32 header_extra_1;
  WORD32 header_extra_2;
  WORD32 sbr_limiter_bands;
  WORD32 sbr_limiter_gains;
  WORD32 sbr_interpol_freq;
  WORD32 sbr_smoothing_length;
  WORD32 alter_scale;
  WORD32 freq_scale;

  WORD32 sbr_pre_proc;
  WORD32 sbr_pvc_active;
  WORD32 sbr_pvc_mode;
  WORD32 sbr_harmonic;
  WORD32 sbr_inter_tes_active;
  WORD32 hq_esbr;

  ixheaace_sr_mode sample_rate_mode;

  WORD32 coupling;
  WORD32 prev_coupling;
} ixheaace_str_sbr_hdr_data, *ixheaace_pstr_sbr_hdr_data;
