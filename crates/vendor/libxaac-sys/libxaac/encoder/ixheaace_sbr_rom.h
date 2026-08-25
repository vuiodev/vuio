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

#define IXHEAACE_IPD_MASK_NEGATED (0x00001000)
#define IXHEAACE_INVF_SMOOTHING_LENGTH (2)

#define IXHEAACE_LD_TRAN (1)
#define IXHEAACE_SBR_TRAN_BITS (4)
#define IXHEAACE_SBR_ENVT_NUMENV (0)
#define IXHEAACE_SBR_ENVT_TRANIDX (3)

typedef struct {
  const WORD32 a_hyb_res[3];
  const WORD32 hi_res_band_borders[8 + 12 + 1];
  const WORD32 grp_borders_mix[28 + 1];
  const WORD32 shift_in_bins[20];
  const WORD32 bins_2_goup_map[28 + 1];
  const FLOAT32 p4_13[13];
  const FLOAT32 p8_13[13];
  const FLOAT32 pan_class[7];
  const FLOAT32 sa_class[7];
  /*Huffman tables for PS*/
  const WORD32 a_book_ps_iid_time_code[29];
  const WORD8 a_book_ps_iid_time_length[29];
  const WORD32 a_book_ps_iid_freq_code[29];
  const WORD8 a_book_ps_iid_freq_length[29];
  const WORD16 a_book_ps_icc_time_code[15];
  const WORD8 a_book_ps_icc_time_length[15];
  const WORD16 a_book_ps_icc_freq_code[15];
  const WORD8 a_book_ps_icc_freq_length[15];
} ixheaace_str_ps_tab;

typedef enum {
  IXHEAACE_INVF_OFF = 0,
  IXHEAACE_INVF_LOW_LEVEL,
  IXHEAACE_INVF_MID_LEVEL,
  IXHEAACE_INVF_HIGH_LEVEL,
  IXHEAACE_INVF_SWITCHED
} ixheaace_invf_mode;

typedef struct {
  const FLOAT32 quant_steps_sbr[4];
  const FLOAT32 quant_steps_org[4];
  const FLOAT32 energy_brdrs[4];
  WORD32 num_regions_sbr;
  WORD32 num_regions_orig;
  WORD32 num_regions_nrg;
  ixheaace_invf_mode region_space[5][5];
  ixheaace_invf_mode region_space_transient[5][5];
  WORD32 energy_comp_factor[5];
} ixheaace_str_det_params;

typedef struct {
  WORD32 n_envelopes;
  WORD32 borders[IXHEAACE_MAX_ENV + 1];
  ixheaace_freq_res freq_res[IXHEAACE_MAX_ENV];
  WORD32 short_env;
  WORD32 n_noise_envelopes;
  WORD32 borders_noise[MAXIMUM_NOISE_ENVELOPES + 1];
} ixheaace_str_frame_info_sbr;

typedef struct {
  WORD32 num_noise_bands;
  WORD32 noise_floor_offset;
  WORD32 noise_max_level;

} ixheaace_noise_config;

typedef struct {
  WORD32 start_freq;
  WORD32 stop_freq;
} ixheaace_freq_range;

typedef struct {
  UWORD32 bitrate_from;
  UWORD32 bitrate_to;

  ixheaace_freq_range freq_band;
  WORD32 freq_scale;

  ixheaace_noise_config noise;
  ixheaace_sbr_stereo_mode stereo_mode;
} ixheaace_sbr_tuning_tables;

typedef struct {
  const FLOAT32 sbr_qmf_64_640[QMF_FILTER_LENGTH + 10];
  const FLOAT32 sbr_cld_fb[CLD_FILTER_LENGTH];
  const FLOAT32 sbr_p_64_640_qmf[QMF_FILTER_LENGTH];
  const FLOAT32 sbr_alt_sin_twiddle[19];
  const FLOAT32 sbr_cos_sin_twiddle[32];
  const FLOAT32 cos_sin_fct4_32[32];
  const FLOAT32 cos_sin_fct4_16[16];
  const FLOAT32 cos_sin_fct4_8[8];
  const ixheaace_str_det_params detector_params_aac;        /*Not part of QMF filtering*/
  const ixheaace_str_det_params detector_params_aac_speech; /*but included here for convenience*/
  ixheaace_str_frame_info_sbr frame_info1_2048;
  ixheaace_str_frame_info_sbr frame_info2_2048;
  ixheaace_str_frame_info_sbr frame_info4_2048;
  ixheaace_str_frame_info_sbr frame_480_info1_2048;
  ixheaace_str_frame_info_sbr frame_480_info2_2048;
  ixheaace_str_frame_info_sbr frame_480_info4_2048;
  const FLOAT32 ptr_smooth_filter[4];
  UWORD32 supported_sample_rate[9];
  ixheaace_sbr_tuning_tables sbr_tuning_table_lc[10][2][10];
  ixheaace_sbr_tuning_tables sbr_tuning_table_ld[10][2][10];
} ixheaace_str_qmf_tabs;

typedef struct {
  const WORD32 v_huff_env_lvl_c10t[121];
  const UWORD8 v_huff_env_lvl_l10t[121];
  const WORD32 v_huff_env_lvl_c10f[121];
  const UWORD8 v_huff_env_lvl_l10f[121];
  const WORD32 book_sbr_env_bal_c10t[49];
  const UWORD8 book_sbr_env_bal_l10t[49];
  const WORD32 book_sbr_env_bal_c10f[49];
  const UWORD8 book_sbr_env_bal_l10f[49];
  const WORD32 v_huff_env_lvl_c11t[63];
  const UWORD8 v_huff_env_lvl_l11t[63];
  const WORD32 v_huff_env_lvl_c11f[63];
  const UWORD8 v_huff_env_lvl_l11f[63];
  const WORD32 book_sbr_env_bal_c11t[25];
  const UWORD8 book_sbr_env_bal_l11t[25];
  const WORD32 book_sbr_env_bal_c11f[25];
  const UWORD8 book_sbr_env_bal_l11f[25];
  const WORD32 v_huff_noise_lvl_c11t[63];
  const UWORD8 v_huff_noise_lvl_l11t[63];
  const WORD32 book_sbr_noise_bal_c11t[25];
  const UWORD8 book_sbr_noise_bal_l11t[25];
} ixheaace_str_sbr_huff_tabs;

typedef struct {
  const WORD32 sfb_bins_8k[21];
  const WORD32 sfb_bins_11k[23];
  const WORD32 sfb_bins_12k[23];
  const WORD32 sfb_bins_16k[23];
  const WORD32 sfb_bins_22k[25];
  const WORD32 sfb_bins_24k[25];
  const WORD32 sfb_bins_32k[27];
  const WORD32 sfb_bins_44k[26];
  const WORD32 sfb_bins_48k[26];
} ixheaace_str_esbr_sfb_bin_tabs;

typedef struct {
  ixheaace_str_ps_tab *ptr_ps_tab;
  ixheaace_str_qmf_tabs *ptr_qmf_tab;
  ixheaace_str_sbr_huff_tabs *ptr_sbr_huff_tab;
  ixheaace_resampler_table *ptr_resamp_tab;
  ixheaace_resampler_sos_table *ptr_sos_downsamp_tab;
  ixheaace_resampler_sos_table *ptr_sos_upsamp_tab;
  ixheaace_str_esbr_sfb_bin_tabs *ptr_esbr_sfb_tab;
} ixheaace_str_sbr_tabs;

extern const ixheaace_str_ps_tab ia_enhaacplus_enc_ps_tab;
extern const ixheaace_str_qmf_tabs ixheaace_qmf_tab;
extern const WORD32 vector_offset_16k[];
extern const WORD32 vector_offset_22k[];
extern const WORD32 vector_offset_24k[];
extern const WORD32 vector_offset_32k[];
extern const WORD32 vector_offset_44_48_64[];
extern const WORD32 vector_offset_88_96[];
extern const WORD32 vector_offset_def[];
extern const WORD32 vector_stop_freq_32[14];
extern const WORD32 vector_stop_freq_44[14];
extern const WORD32 vector_stop_freq_48[14];

/* Resampler tables */
extern const ixheaace_resampler_table ixheaace_resamp_2_to_1_iir_filt_params;
extern const ixheaace_resampler_table ixheaace_resamp_4_to_1_iir_filt_params;
extern const ixheaace_resampler_sos_table iixheaace_resamp_1_to_3_filt_params;
extern const ixheaace_resampler_sos_table iixheaace_resamp_8_to_1_filt_params;
extern const ixheaace_str_sbr_huff_tabs ixheaace_sbr_huff_tab;
extern const ixheaace_str_esbr_sfb_bin_tabs ia_esbr_sfb_bin_tabs;
extern const WORD32 ixheaace_ld_env_tab_480[15][4];
extern const WORD32 ixheaace_ld_env_tab_512[16][4];
extern const WORD32 ixheaace_start_freq_16k_4_1[SBR_START_FREQ_OFFSET_TBL_LEN];
extern const WORD32 ixheaace_start_freq_22k_4_1[SBR_START_FREQ_OFFSET_TBL_LEN];
extern const WORD32 ixheaace_start_freq_24k_4_1[SBR_START_FREQ_OFFSET_TBL_LEN];
extern const WORD32 ixheaace_start_freq_32k_4_1[SBR_START_FREQ_OFFSET_TBL_LEN];
extern const WORD32 ixheaace_start_freq_48k_4_1[SBR_START_FREQ_OFFSET_TBL_LEN];
extern const WORD32 ixheaace_start_freq_96k_4_1[SBR_START_FREQ_OFFSET_TBL_LEN];
extern const WORD32 ixheaace_start_freq_dflt_4_1[SBR_START_FREQ_OFFSET_TBL_LEN];

extern const WORD32 ixheaace_stop_freq_16k_4_1[SBR_STOP_FREQ_OFFSET_TBL_LEN];
extern const WORD32 ixheaace_stop_freq_22k_4_1[SBR_STOP_FREQ_OFFSET_TBL_LEN];
extern const WORD32 ixheaace_stop_freq_24k_4_1[SBR_STOP_FREQ_OFFSET_TBL_LEN];
extern const WORD32 ixheaace_stop_freq_32k_4_1[SBR_STOP_FREQ_OFFSET_TBL_LEN];
extern const WORD32 ixheaace_stop_freq_44k_4_1[SBR_STOP_FREQ_OFFSET_TBL_LEN];
extern const WORD32 ixheaace_stop_freq_48k_4_1[SBR_STOP_FREQ_OFFSET_TBL_LEN];

extern const WORD32 ixheaace_usac_stop_freq_16k[14];
extern const WORD32 ixheaace_usac_stop_freq_22k[14];
extern const WORD32 ixheaace_usac_stop_freq_24k[14];

extern const WORD32 ixheaace_stop_freq_16k[14];
extern const WORD32 ixheaace_stop_freq_22k[14];
extern const WORD32 ixheaace_stop_freq_24k[14];
extern const WORD32 ixheaace_stop_freq_32k[14];
extern const WORD32 ixheaace_stop_freq_44k[14];
extern const WORD32 ixheaace_stop_freq_48k[14];
extern const FLOAT32 filter[IXHEAACE_INVF_SMOOTHING_LENGTH + 1];

extern const FLOAT32 long_window_sine_ld_64[IXHEAACE_QMF_CHANNELS];
extern const FLOAT32 fft_twiddle_tab_32[IXHEAACE_QMF_TIME_SLOTS];
extern const FLOAT32 sbr_sin_cos_window[IXHEAACE_QMF_CHANNELS * 2];
extern const FLOAT32 cld_fb_64_640[CLD_FILTER_LENGTH];
