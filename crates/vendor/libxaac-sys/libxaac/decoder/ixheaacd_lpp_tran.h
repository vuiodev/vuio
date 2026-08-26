/******************************************************************************
 *                                                                            *
 * Copyright (C) 2018 The Android Open Source Project
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
#ifndef IXHEAACD_LPP_TRAN_H
#define IXHEAACD_LPP_TRAN_H

#define MAX_NUM_PATCHES 6
#define GUARDBANDS 0
#define SHIFT_START_SB 1

typedef struct {
  WORD32 phi_11;
  WORD32 phi_22;
  WORD32 phi_01;
  WORD32 phi_02;
  WORD32 phi_12;
  WORD32 phi_01_im;
  WORD32 phi_02_im;
  WORD32 phi_12_im;
  WORD32 d;
} ia_lpp_trans_cov_matrix;

typedef struct {
  WORD16 src_start_band;
  WORD16 src_end_band;
  WORD16 guard_start_band;

  WORD16 dst_start_band;
  WORD16 dst_end_band;
  WORD16 num_bands_in_patch;
} ia_patch_param_struct;

typedef struct {
  WORD16 num_columns;
  WORD16 num_patches;
  WORD16 start_patch;
  WORD16 stop_patch;
  WORD16 bw_borders[MAX_NUM_NOISE_VALUES];
  ia_patch_param_struct str_patch_param[MAX_NUM_PATCHES];
} ia_transposer_settings_struct;

typedef struct {
  ia_transposer_settings_struct *pstr_settings;
  WORD32 bw_array_prev[MAX_NUM_PATCHES];
  WORD32 *lpc_filt_states_real[LPC_ORDER];
  WORD32 *lpc_filt_states_imag[LPC_ORDER];
} ia_sbr_hf_generator_struct;

VOID ixheaacd_low_pow_hf_generator(
    ia_sbr_hf_generator_struct *hf_generator, WORD32 **qmf_buff_re,
    WORD16 *degree_alias, WORD first_slot_offset, WORD last_slot_offset,
    WORD num_if_bands, WORD max_qmf_subband_aac, WORD32 *sbr_invf_mode,
    WORD32 *sbr_invf_mode_prev, WORD32 norm_max, WORD32 *ptr_qmf_matrix);

VOID ixheaacd_hf_generator(ia_sbr_hf_generator_struct *hf_generator,
                           ia_sbr_scale_fact_struct *sbr_scale_factor,
                           WORD32 **qmf_buff_re, WORD32 **qmf_buff_im,
                           WORD time_step, WORD first_slot_offset,
                           WORD last_slot_offset, WORD num_if_bands,
                           WORD max_qmf_subband_aac, WORD32 *sbr_invf_mode,
                           WORD32 *sbr_invf_mode_prev, WORD32 *ptr_qmf_matrix,
                           WORD audio_object_type);

VOID ixheaacd_invfilt_level_emphasis(ia_sbr_hf_generator_struct *hf_generator,
                                     WORD32 num_if_bands, WORD32 *sbr_invf_mode,
                                     WORD32 *sbr_invf_mode_prev,
                                     WORD32 *bw_array);

struct ixheaacd_lpp_trans_patch {
  WORD32 num_patches;
  WORD32 start_subband[MAX_NUM_PATCHES + 1];
};

VOID ixheaacd_covariance_matrix_calc_dec(
    WORD32 *sub_sign_xlow, ia_lpp_trans_cov_matrix *cov_matrix,
    WORD32 count, WORD32 auto_corr_length);

VOID ixheaacd_covariance_matrix_calc_dec_960(
    WORD32 *sub_sign_xlow, ia_lpp_trans_cov_matrix *cov_matrix,
    WORD32 count, WORD32 auto_corr_length);

VOID ixheaacd_covariance_matrix_calc_armv7(
    WORD32 *sub_sign_xlow, ia_lpp_trans_cov_matrix *cov_matrix,
    WORD32 count);

VOID ixheaacd_covariance_matrix_calc_2_dec(
    ia_lpp_trans_cov_matrix *cov_matrix, WORD32 *real_buffer,
    WORD32 ixheaacd_num_bands, WORD16 slots);

VOID ixheaacd_covariance_matrix_calc_2_armv7(
    ia_lpp_trans_cov_matrix *cov_matrix, WORD32 *real_buffer,
    WORD32 ixheaacd_num_bands, WORD16 slots);

#endif /* IXHEAACD_LPP_TRAN_H */
