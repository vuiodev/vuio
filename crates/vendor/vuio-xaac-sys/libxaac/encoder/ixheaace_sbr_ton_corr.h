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

#define IXHEAACE_MAXIMUM_NUM_PATCHES (6)

typedef struct {
  FLOAT32 r00r;
  FLOAT32 r11r;
  FLOAT32 r01r;
  FLOAT32 r01i;
  FLOAT32 r02r;
  FLOAT32 r02i;
  FLOAT32 r12r;
  FLOAT32 r12i;
  FLOAT32 r22r;
  FLOAT32 det;
} ixheaace_acorr_coeffs;

typedef struct {
  WORD32 source_start_band;
  WORD32 source_stop_band;
  WORD32 guard_start_band;

  WORD32 target_start_band;
  WORD32 target_band_offs;
  WORD32 num_bands_in_patch;
} ixheaace_patch_param;

typedef struct {
  WORD32 switch_inverse_filt;
  WORD32 num_qmf_ch;
  WORD32 est_cnt;
  WORD32 est_cnt_per_frame;
  WORD32 move;
  WORD32 frame_start_index;
  WORD32 start_index_matrix;
  WORD32 frame_start_index_invf_est;

  WORD32 prev_trans_flag;
  WORD32 trans_nxt_frame;
  WORD32 trans_pos_offset;

  FLOAT32 sbr_quota_mtx[NO_OF_ESTIMATES * IXHEAACE_QMF_CHANNELS];

  FLOAT32 *ptr_quota_mtx[NO_OF_ESTIMATES];
  FLOAT32 energy_vec[NO_OF_ESTIMATES];
  WORD8 idx_vx[IXHEAACE_QMF_CHANNELS];

  ixheaace_patch_param str_patch_param[IXHEAACE_MAXIMUM_NUM_PATCHES];
  WORD32 guard;
  WORD32 shift_start_sb;
  WORD32 no_of_patches;

  ixheaace_str_sbr_missing_har_detector sbr_missing_har_detector;
  ixheaace_str_noise_flr_est_sbr sbr_noise_floor_est;
  ixheaace_str_sbr_inv_filt_est sbr_noise_inv_filt;
} ixheaace_str_sbr_ton_corr_est;

typedef ixheaace_str_sbr_ton_corr_est *ixheaace_pstr_sbr_ton_corr_est;

IA_ERRORCODE
ixheaace_create_ton_corr_param_extr(WORD32 ch, ixheaace_pstr_sbr_ton_corr_est pstr_ton_corr,
                                    WORD32 fs, WORD32 num_qmf_ch, WORD32 xpos_ctrl,
                                    WORD32 high_band_start_sb, UWORD8 *ptr_vk_master,
                                    WORD32 num_master, WORD32 ana_max_level,
                                    UWORD8 *ptr_freq_band_tab[2], WORD32 *ptr_num_scf,
                                    WORD32 noise_groups, UWORD32 use_speech_config,
                                    WORD32 *ptr_common_buffer,
                                    ixheaace_str_qmf_tabs *pstr_qmf_tab, WORD32 is_ld_sbr);

VOID ixheaace_ton_corr_param_extr(ixheaace_pstr_sbr_ton_corr_est pstr_ton_corr,
                                  ixheaace_invf_mode *pstr_inf_vec, FLOAT32 *ptr_noise_lvls,
                                  WORD32 *ptr_missing_harmonic_flag,
                                  UWORD8 *ptr_missing_harmonic_index, WORD8 *ptr_env_compensation,
                                  const ixheaace_str_frame_info_sbr *pstr_frame_info,
                                  WORD32 *ptr_trans_info, UWORD8 *ptr_freq_band_tab,
                                  WORD32 ptr_num_scf, ixheaace_sbr_xpos_mode xpos_type,
                                  WORD8 *ptr_sbr_scratch, WORD32 is_ld_sbr);

VOID ixheaace_calculate_tonality_quotas(ixheaace_pstr_sbr_ton_corr_est pstr_ton_corr,
                                        FLOAT32 **ptr_real, FLOAT32 **ptr_imag, WORD32 usb,
                                        WORD32 num_time_slots, WORD32 is_ld_sbr);
