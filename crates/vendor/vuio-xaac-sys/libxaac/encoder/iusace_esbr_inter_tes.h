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

#define IXHEAACE_ESBR_TES_ENERGY_MAX_THR (1.0e6f)
#define IXHEAACE_TIMESLOT_BUFFER_SIZE (78)
#define IXHEAACE_MAX_ENVELOPES (8)
#define IXHEAACE_MAX_NOISE_ENVELOPES (2)
#define IXHEAACE_MAX_NOISE_COEFFS (5)
#define IXHEAACE_MAX_NUM_NOISE_VALUES (IXHEAACE_MAX_NOISE_ENVELOPES * IXHEAACE_MAX_NOISE_COEFFS)
#define IXHEAACE_MAX_FREQ_COEFFS (56)
#define IXHEAACE_MAX_NUM_PATCHES (6)
#define IXHEAACE_MAX_NUM_LIMITERS (12)
#define IXHEAACE_MAXDEG (3)
#define IXHEAACE_SBR_HF_RELAXATION_PARAM (0.999999f)
#define IXHEAACE_ESBR_NUM_GAMMA_IDXS (4)
#define IXHEAACE_ESBR_HBE_DELAY_OFFSET (32)
#define IXHEAACE_SBR_HF_ADJ_OFFSET (2)
#define IXHEAACE_SBR_TES_SHAPE_BITS (1)
#define IXHEAACE_SBR_TES_SHAPE_MODE_BITS (2)

typedef struct {
  FLOAT32 phi_0_1_real;
  FLOAT32 phi_0_1_imag;
  FLOAT32 phi_0_2_real;
  FLOAT32 phi_0_2_imag;
  FLOAT32 phi_1_1;
  FLOAT32 phi_1_2_real;
  FLOAT32 phi_1_2_imag;
  FLOAT32 phi_2_2;
  FLOAT32 det;
} ixheaace_str_auto_corr_ele;

typedef struct {
  WORD16 num_sf_bands[2];
  WORD16 num_nf_bands;
  WORD16 num_mf_bands;
  WORD16 sub_band_start;
  WORD16 sub_band_end;
  WORD16 freq_band_tbl_lim[IXHEAACE_MAX_NUM_LIMITERS + 1];
  WORD16 num_lf_bands;
  WORD16 num_if_bands;
  WORD16 *ptr_freq_band_tab[2];
  WORD16 freq_band_tbl_lo[IXHEAACE_MAX_FREQ_COEFFS / 2 + 1];
  WORD16 freq_band_tbl_hi[IXHEAACE_MAX_FREQ_COEFFS + 1];
  WORD16 freq_band_tbl_noise[IXHEAACE_MAX_NOISE_COEFFS + 1];
  WORD16 f_master_tbl[IXHEAACE_MAX_FREQ_COEFFS + 1];
  WORD16 qmf_sb_prev;
} ia_str_freq_band_data;

typedef struct {
  WORD16 frame_class;
  WORD16 num_env;
  WORD16 transient_env;
  WORD16 num_noise_env;
  WORD16 border_vec[IXHEAACE_MAX_ENVELOPES + 1];
  WORD16 freq_res[IXHEAACE_MAX_ENVELOPES];
  WORD16 noise_border_vec[IXHEAACE_MAX_NOISE_ENVELOPES + 1];
} ia_str_frame_info;

typedef struct {
  WORD32 num_if_bands;
  WORD32 sub_band_start;
  WORD32 sub_band_end;
  WORD32 num_mf_bands;
  WORD32 sbr_patching_mode;
  WORD32 pre_proc_flag;
  WORD32 is_usf_4;
  WORD32 hbe_flag;
  WORD32 out_fs;
  WORD32 num_env;
  WORD32 op_delay;
  WORD32 codec_delay;
  WORD32 sbr_ratio_index;
  WORD16 invf_band_tbl[MAXIMUM_NUM_NOISE_VALUES + 1];
  WORD16 f_master_tbl[MAXIMUM_FREQ_COEFFS + 1];
  WORD32 inv_filt_mode[MAXIMUM_NUM_NOISE_VALUES];
  WORD32 inv_filt_mode_prev[IXHEAACE_MAX_NUM_NOISE_VALUES];
  FLOAT32 bw_array_prev[IXHEAACE_MAX_NUM_PATCHES];
  WORD16 border_vec[IXHEAACE_MAX_ENV + 1];
  WORD32 bs_tes_shape[IXHEAACE_MAX_ENV + 1];
  WORD32 bs_tes_shape_mode[IXHEAACE_MAX_ENV + 1];
  FLOAT32 qmf_buf_real[IXHEAACE_TIMESLOT_BUFFER_SIZE + 2 * IXHEAACE_ESBR_HBE_DELAY_OFFSET]
                      [IXHEAACE_QMF_CHANNELS];
  FLOAT32 qmf_buf_imag[IXHEAACE_TIMESLOT_BUFFER_SIZE + 2 * IXHEAACE_ESBR_HBE_DELAY_OFFSET]
                      [IXHEAACE_QMF_CHANNELS];
} ixheaace_str_inter_tes_params;

typedef struct {
  FLOAT32 dst_qmf_r[IXHEAACE_TIMESLOT_BUFFER_SIZE][IXHEAACE_QMF_CHANNELS];
  FLOAT32 dst_qmf_i[IXHEAACE_TIMESLOT_BUFFER_SIZE][IXHEAACE_QMF_CHANNELS];
} ixheaace_str_inter_tes_scr;

VOID ixheaace_init_esbr_inter_tes(ixheaace_str_inter_tes_params *pstr_tes_enc,
                                  WORD32 sbr_ratio_index);
IA_ERRORCODE ixheaace_process_inter_tes(ixheaace_str_inter_tes_params *pstr_tes_enc,
                                        WORD8 *ptr_scr);
