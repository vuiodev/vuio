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

#define DELTA_TIME 9
#define MAXIMUM_COMP 2
#define TONALITY_QUOTA 0.1f
#define DIFF_QUOTA 0.75f
#define THR_DIFF 25.0f
#define THR_DIFF_GUIDE 1.26f
#define THR_TONE 15.0f
#define I_THR_TONE (1.0f / 15.0f)
#define THR_TONE_GUIDE 1.26f
#define THR_SFM_SBR 0.3f
#define THR_SFM_ORIG 0.1f
#define DECAY_GUIDE_ORIG 0.3f
#define DECAY_GUIDE_DIFF 0.5f

typedef struct {
  FLOAT32 *ptr_guide_vec_diff;
  FLOAT32 *ptr_guide_vec_orig;
  UWORD8 *ptr_guide_vector_detected;
} ixheaace_str_guide_vectors;

typedef struct {
  WORD32 qmf_num_ch;
  WORD32 num_scf;
  WORD32 sample_freq;
  WORD32 prev_trans_flag;
  WORD32 prev_trans_frame;
  WORD32 prev_trans_pos;

  WORD32 no_vec_per_frame;
  WORD32 trans_pos_offset;

  WORD32 move;
  WORD32 tot_no_est;
  WORD32 no_est_per_frame;
  WORD32 time_slots;

  UWORD8 *ptr_guide_scfb;
  UWORD8 sbr_guide_scfb[MAXIMUM_FREQ_COEFFS];

  WORD8 *ptr_prev_env_compensation;
  WORD8 sbr_prev_env_compensation[MAXIMUM_FREQ_COEFFS];

  UWORD8 *ptr_detection_vectors[NO_OF_ESTIMATES];
  UWORD8 sbr_detection_vectors[NO_OF_ESTIMATES * MAXIMUM_FREQ_COEFFS];

  FLOAT32 *ptr_sfm_orig[NO_OF_ESTIMATES];
  FLOAT32 *ptr_sfm_sbr[NO_OF_ESTIMATES];
  FLOAT32 *ptr_tonal_diff[NO_OF_ESTIMATES];
  ixheaace_str_guide_vectors guide_vectors[NO_OF_ESTIMATES];
  UWORD8 sbr_guide_vector_detected[NO_OF_ESTIMATES * MAXIMUM_FREQ_COEFFS];
} ixheaace_str_sbr_missing_har_detector;

typedef ixheaace_str_sbr_missing_har_detector *ixheaace_pstr_sbr_missing_harmonics_detector;

VOID ixheaace_sbr_missing_harmonics_detector_qmf(
    ixheaace_pstr_sbr_missing_harmonics_detector pstr_sbr_mhd_et, FLOAT32 **ptr_quota_buf,
    WORD8 *ptr_idx_vx, const ixheaace_str_frame_info_sbr *ptr_frame_info,
    const WORD32 *ptr_tran_info, WORD32 *ptr_add_harmonics_flag, UWORD8 *ptr_add_harmonics_sfbs,
    const UWORD8 *ptr_freq_band_tab, WORD32 num_sfb, WORD8 *ptr_env_compensation);

VOID ixheaace_create_sbr_missing_harmonics_detector(
    WORD32 ch, ixheaace_pstr_sbr_missing_harmonics_detector pstr_sbr_mhdet, WORD32 sample_freq,
    WORD32 num_sfb, WORD32 qmf_num_ch, WORD32 tot_no_est, WORD32 move, WORD32 no_est_per_frame,
    WORD32 *ptr_common_buffer);
