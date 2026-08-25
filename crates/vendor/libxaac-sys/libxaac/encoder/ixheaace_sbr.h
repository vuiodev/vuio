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

struct ixheaace_str_sbr_bitstream_data {
  WORD32 total_bits;
  WORD32 payload_bits;
  WORD32 fill_bits;
  WORD32 header_active;
  WORD32 crc_active;
  WORD32 nr_send_header_data;
  WORD32 count_send_header_data;
  WORD32 usac_indep_flag;
};

struct ixheaace_str_sbr_config_data {
  WORD32 num_ch;
  WORD32 num_scf[2];
  WORD32 num_master;
  WORD32 sample_freq;
  WORD32 is_ld_sbr;
  WORD32 is_esbr;
  WORD32 xover_freq;
  UWORD8 sbr_freq_band_tab_lo[MAXIMUM_FREQ_COEFFS / 2 + 1];
  UWORD8 sbr_freq_band_tab_hi[MAXIMUM_FREQ_COEFFS + 1];
  UWORD8 *ptr_freq_band_tab[2];
  UWORD8 *ptr_v_k_master;
  UWORD8 sbr_v_k_master[MAXIMUM_FREQ_COEFFS + 1];
  ixheaace_sbr_stereo_mode stereo_mode;
  WORD32 detect_missing_harmonics;
  WORD32 use_parametric_coding;
  WORD32 xpos_control_switch;
  WORD32 sbr_ratio_idx;
  ixheaace_sbr_codec_type sbr_codec;
};

struct ixheaace_str_sbr_env_data {
  WORD32 sbr_xpos_ctrl;
  ixheaace_freq_res freq_res_fix;
  ixheaace_invf_mode sbr_invf_mode;
  ixheaace_invf_mode sbr_invf_mode_vec[MAXIMUM_NUM_NOISE_VALUES];
  ixheaace_pvc_bs_info pvc_info;
  ixheaace_sbr_xpos_mode sbr_xpos_mode;
  WORD32 ienvelope[IXHEAACE_MAX_ENV][MAXIMUM_FREQ_COEFFS];
  WORD32 code_book_scf_lav_balance;
  WORD32 code_book_scf_lav;
  const WORD32 *ptr_huff_tab_time_c;
  const WORD32 *ptr_huff_tab_freq_c;
  const UWORD8 *ptr_huff_tab_time_l;
  const UWORD8 *ptr_huff_tab_freq_l;
  const WORD32 *ptr_huff_tab_lvl_time_c;
  const WORD32 *ptr_huff_tab_bal_time_c;
  const WORD32 *ptr_huff_tab_lvl_freq_c;
  const WORD32 *ptr_huff_tab_bal_freq_c;
  const UWORD8 *ptr_huff_tab_lvl_time_l;
  const UWORD8 *ptr_huff_tab_bal_time_l;
  const UWORD8 *ptr_huff_tab_lvl_freq_l;
  const UWORD8 *ptr_huff_tab_bal_freq_l;
  const UWORD8 *ptr_huff_tab_noise_time_l;
  const WORD32 *ptr_huff_tab_noise_time_c;
  const UWORD8 *ptr_huff_tab_noise_freq_l;
  const WORD32 *ptr_huff_tab_noise_freq_c;
  const UWORD8 *ptr_huff_tab_noise_lvl_time_l;
  const WORD32 *ptr_huff_tab_noise_lvl_time_c;
  const UWORD8 *ptr_huff_tab_noise_bal_time_l;
  const WORD32 *ptr_huff_tab_noise_bal_time_c;
  const UWORD8 *ptr_huff_tab_noise_lvl_freq_l;
  const WORD32 *ptr_huff_tab_noise_lvl_freq_c;
  const UWORD8 *ptr_huff_tab_noise_bal_freq_l;
  const WORD32 *ptr_huff_tab_noise_bal_freq_c;
  ixheaace_pstr_sbr_grid pstr_sbr_bs_grid;
  WORD32 synthetic_coding;
  WORD32 no_harmonics;
  WORD32 add_harmonic_flag;
  UWORD8 add_harmonic[MAXIMUM_FREQ_COEFFS];
  WORD32 si_sbr_start_env_bits_balance;
  WORD32 si_sbr_start_env_bits;
  WORD32 si_sbr_start_noise_bits_balance;
  WORD32 si_sbr_start_noise_bits;
  WORD32 no_of_envelopes;
  WORD32 no_scf_bands[IXHEAACE_MAX_ENV];
  WORD32 domain_vec[IXHEAACE_MAX_ENV];
  WORD32 domain_vec_noise[IXHEAACE_MAX_ENV];
  WORD32 noise_level[MAXIMUM_FREQ_COEFFS_HEAAC];
  WORD32 noise_band_count;
  WORD32 balance;
  WORD32 init_sbr_amp_res;
  WORD32 sbr_patching_mode;
  WORD32 sbr_oversampling_flag;
  WORD32 sbr_pitchin_bins_flag;
  WORD32 sbr_coupling;
  WORD32 sbr_preprocessing;
  WORD32 sbr_pitchin_bins;
  WORD32 harmonic_sbr;
  WORD32 usac_indep_flag;
  WORD32 usac_harmonic_sbr;
  WORD32 sbr_pvc_mode;
  WORD32 sbr_sinusoidal_pos_flag;
  WORD32 sbr_sinusoidal_pos;
  WORD32 sbr_inter_tes;
  WORD32 *ptr_sbr_inter_tes_shape;
  WORD32 *ptr_sbr_inter_tes_shape_mode;
  WORD32 curr_sbr_amp_res;
};

struct ixheaace_str_enc_channel {
  ixheaace_str_sbr_trans_detector str_sbr_trans_detector;
  ixheaace_str_sbr_code_envelope str_sbr_code_env;
  ixheaace_str_sbr_code_envelope str_sbr_code_noise_floor;
  ixheaace_str_sbr_extr_env str_sbr_extract_env;
  ixheaace_str_sbr_qmf_filter_bank str_sbr_qmf;
  ixheaace_str_sbr_env_frame str_sbr_env_frame;
  ixheaace_str_sbr_ton_corr_est str_ton_corr;
  ixheaace_str_inter_tes_params str_inter_tes_enc;
  ixheaace_str_hbe_enc *pstr_hbe_enc;
  WORD32 sbr_amp_res_init;
  struct ixheaace_str_sbr_env_data enc_env_data;
};

typedef struct {
  WORD32 sbr_num_chan;
  WORD32 sbr_coupling;
  UWORD8 sbr_preprocessing;
  UWORD8 sbr_patching_mode[2];
  UWORD8 sbr_oversampling_flag[2];
  UWORD8 sbr_pitchin_flags[2];
  UWORD8 sbr_pitchin_bins[2];
} ixheaace_str_esbr_bs_data;

typedef struct ixheaace_str_sbr_bitstream_data *ixheaace_pstr_sbr_bitstream_data;
typedef struct ixheaace_str_sbr_config_data *ixheaace_pstr_sbr_config_data;
typedef struct ixheaace_str_enc_channel *ixheaace_pstr_enc_channel;
typedef struct ixheaace_str_sbr_env_data *ixheaace_pstr_sbr_env_data;
