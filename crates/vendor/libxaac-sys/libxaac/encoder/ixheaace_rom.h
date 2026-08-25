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
#define AACENC_MAX_CHANNELS 2 /* encoder max channels */
#define AACENC_TRANS_FAC 8    /* encoder WORD16 long ratio */
#define AACENC_PCM_LEVEL 1.0  /* encoder pcm 0db refernence */

#define LD_FFT_TWIDDLE_TABLE_SIZE 9
#define FFT_TWIDDLE_TABLE_SIZE (1 << LD_FFT_TWIDDLE_TABLE_SIZE)

/*MDCT Tables*/
typedef struct {
  UWORD8 re_arr_tab_16[240];
  UWORD8 re_arr_tab_sml_240[240];
  const FLOAT32 cosine_array_960[480];
  WORD32 dig_rev_tab_16[2];
  WORD16 w_16[24];
  const FLOAT32 cosine_array_1024[512];
  const WORD16 w_256[512];
  const WORD32 dig_rev_table_512[32];
  const FLOAT32 win_512_ld[1920];
  const FLOAT32 win_480_ld[1800];
} ixheaace_mdct_tables;
extern const ixheaace_mdct_tables ixheaace_enc_mdct_tab;

/*Huffman Tables*/
typedef struct {
  const UWORD16 huff_ltab1_2[3][3][3][3];
  const UWORD16 huff_ltab3_4[3][3][3][3];
  const UWORD16 huff_ltab5_6[9][27];
  const UWORD16 huff_ltab7_8[8][27];
  const UWORD16 huff_ltab9_10[13][27];
  const UWORD16 huff_ltab11[17][27];
  const UWORD16 huff_ltabscf[121];
  const UWORD16 huff_ctab1[3][3][3][3];
  const UWORD16 huff_ctab2[3][3][3][3];
  const UWORD16 huff_ctab3[3][3][3][3];
  const UWORD16 huff_ctab4[3][3][3][3];
  const UWORD16 huff_ctab5[9][9];
  const UWORD16 huff_ctab6[9][9];
  const UWORD16 huff_ctab7[8][8];
  const UWORD16 huff_ctab8[8][8];
  const UWORD16 huff_ctab9[13][13];
  const UWORD16 huff_ctab10[13][13];
  const UWORD16 huff_ctab11[17][17];
  const UWORD32 huff_ctabscf[121];
} ixheaace_huffman_tables;

extern const ixheaace_huffman_tables ixheaace_enc_huff_tab;

typedef struct {
  WORD32 sample_rate;
  const UWORD8 param_long[MAXIMUM_SCALE_FACTOR_BAND_LONG];
  const UWORD8 param_short[MAXIMUM_SCALE_FACTOR_BAND_SHORT];
  const UWORD8 param_long_960[MAXIMUM_SCALE_FACTOR_BAND_LONG];
  const UWORD8 param_short_120[1 + MAXIMUM_SCALE_FACTOR_BAND_SHORT];
  const UWORD8 param_long_512_ld[MAXIMUM_SCALE_FACTOR_BAND_LONG];
  const UWORD8 param_long_480_ld[MAXIMUM_SCALE_FACTOR_BAND_LONG];
} ixheaace_sfb_info_tab;

/*Psychoacostics Tables*/
typedef struct {
  const FLOAT32 ixheaace_bark_quiet_thr_val[25];
  const WORD32 pow_point_25_Q23[128];
  const WORD32 q_mod_4_inv_pow_point_25_Q32[4];
  const ixheaace_sfb_info_tab sfb_info_tab[12];
} ixheaace_psycho_tables;

extern const ixheaace_psycho_tables ixheaace_enc_psycho_tab;

/*Quantizer tables*/
typedef struct {
  const WORD q_quant_table_E_fix[17];
  const WORD q_inv_quant_table_E_fix[17];
  const WORD32 quant_table_q_Q30[16];
  const WORD32 inv_quant_table_q_Q27[16];
  const WORD32 power_3_by_4_tab[512];
  const WORD32 power_4_by_3_tab_fix[64];
  const WORD16 power_2_n_4_by_3_tab[10];
} ixheaace_quant_tables;

extern const ixheaace_quant_tables ixheaace_enc_quant_tab;
typedef struct {
  const ixheaace_temporal_noise_shaping_max_table tns_max_bands_table[12];
  const ixheaace_temporal_noise_shaping_info_tab tns_info_tab[5];
  const FLOAT32 tns_coeff_3[8];
  const FLOAT32 tns_coeff_3_borders[8];
  const FLOAT32 tns_coeff_4[16];
  const FLOAT32 tns_coeff_4_borders[16];
} ixheaace_temporal_noise_shaping_tables;

extern const ixheaace_temporal_noise_shaping_tables ixheaace_enhaacplus_enc_tns_tab;

typedef struct {
  ixheaace_mdct_tables *pstr_mdct_tab;
  ixheaace_huffman_tables *pstr_huff_tab;
  ixheaace_psycho_tables *pstr_psycho_tab;
  ixheaace_quant_tables *pstr_quant_tab;
  ixheaace_temporal_noise_shaping_tables *pstr_tns_tab;
} ixheaace_aac_tables;

extern ixheaace_aac_tables ixheaace_enc_aac_tab;

/***************/
/*
input buffer (1ch)

  |------- 288 --------|--------------- 2048 -------------|
  spectral_band_replication_2core delay      Read, SBR, downmix and ds area
  (downsampled)
*/
#define CORE_DELAY_LC (1600)
// LC Delay = (Core-delay LC) * 2 + SBR Decoder Delay - SBR Encoder Delay + Magic
#define INPUT_DELAY_LC ((CORE_DELAY_LC)*2 + 6 * 64 - 2048 + 1)
#define CORE_DELAY_LD_512 (512)
#define INPUT_DELAY_LD_512 ((CORE_DELAY_LD_512)*2 - 1024 + 1)
// ELD Delay = (Core-delay LD) * 2 + SBR Decoder Delay - SBR Encoder Delay
#define INPUT_DELAY_ELD_512 ((CORE_DELAY_LD_512)*2 + 0 - (16 * 64))
// Original ELD delay value retained for multichannel files with MPS
#define INPUT_DELAY_ELD_512_MPS ((CORE_DELAY_LD_512)*2 + 1)
#define INPUT_DELAY_ELDV2_512 ((CORE_DELAY_LD_512 * 2) + 128 + 1)

#define CORE_DELAY_LD_480 (480)
#define INPUT_DELAY_LD_480 ((CORE_DELAY_LD_480)*2 - 1024 + 1)
// ELD Delay = (Core-delay LD / 2) * 2 + SBR Decoder Delay - SBR Encoder Delay
#define INPUT_DELAY_ELD_480 ((CORE_DELAY_LD_480 / 2) * 2 + 0 - (15 * 64) + 576)
// Original ELD delay value retained for multichannel files with MPS
#define INPUT_DELAY_ELD_480_MPS (CORE_DELAY_LD_480 * 2 + 1)
#define INPUT_DELAY_ELDV2_480 ((CORE_DELAY_LD_480 * 2) + 128 + 1)
#define CORE_DELAY (1600)

/* ((1600 (core codec)*2 (multi rate) + 6 *(spectral_band_replication_ dec delay) -
 * (spectral_band_replication_ enc delay) + magic*/
#define INPUT_DELAY ((CORE_DELAY)*2 + 6 * 64 - 2048 + 1)

/* For 2:1 resampler -> max phase delay * resamp_fac */
#define MAXIMUM_DS_2_1_FILTER_DELAY (16)

/* For 4:1 resampler -> max phase delay * resamp_fac */
#define MAXIMUM_DS_4_1_FILTER_DELAY (64)

/* For 8:1 resampler -> max phase delay * resamp_fac */
#define MAXIMUM_DS_8_1_FILTER_DELAY (248)

/* For 1:3 resampler -> max phase delay * resamp_fac */
#define MAXIMUM_DS_1_3_FILTER_DELAY (36)

#define CC_DELAY_ADJUSTMENT (448)

extern const FLOAT32 ixheaace_fd_quant_table[257];
extern const FLOAT32 ixheaace_fd_inv_quant_table[257];
extern const FLOAT32 ixheaace_pow_4_3_table[64];
extern const WORD32 ixheaace_huffman_code_table[121][2];
extern const FLOAT32 long_window_KBD[1024];
extern const FLOAT32 long_window_sine_960[960];
extern const FLOAT32 fft_twiddle_tab[FFT_TWIDDLE_TABLE_SIZE + 1];
extern const FLOAT32 long_window_sine[1024];
extern const FLOAT32 long_window_sine_ld[512];
extern const FLOAT32 long_window_sine_ld_480[480];
extern const FLOAT32 short_window_sine[FRAME_LEN_SHORT_128];

extern const FLOAT32 short_window_sine_120[FRAME_LEN_SHORT_120];

extern const FLOAT32 iaace_iir_hipass_coeffs[2];

extern const WORD32 iaace_suggested_grouping_table[TRANS_FAC][MAXIMUM_NO_OF_GROUPS];

extern const WORD32 iaace_synchronized_block_types[4][4];
extern const FLOAT32 ixheaace_mix_rad_twid_tbl[TRANS_FAC];
extern const FLOAT32 ixheaace_mix_rad_twid_tbl_h[TRANS_FAC];
extern const FLOAT32 cos_sin_table_flt[960];
extern const FLOAT32 ixheaace_fft_mix_rad_twid_tbl_32[(FFT16 - 1) * (FFT2 - 1)];
extern const FLOAT32 ixheaace_fft_mix_rad_twid_tbl_h_32[(FFT16 - 1) * (FFT2 - 1)];
extern const FLOAT32 ixheaace_fft_mix_rad_twid_tbl_480[(FFT15 - 1) * (FFT32 - 1)];
extern const FLOAT32 ixheaace_fft_mix_rad_twid_h_tbl_480[(FFT15 - 1) * (FFT32 - 1)];
extern const FLOAT32 ixheaace_cosine_array_240[120];
extern const WORD16 re_arr_tab_5[16];
extern const WORD16 re_arr_tab_3[16];
extern const WORD16 re_arr_tab_sml[16];
extern const WORD16 re_arr_tab_4[60];
extern const WORD16 re_arr_tab_120[60];
extern const WORD16 re_arr_tab_15_4[60];
extern const FLOAT32 low_delay_window_eld[2048];
extern const ixheaace_bandwidth_table bandwidth_table_lc[9];

extern const ixheaace_bandwidth_table bandwidth_table_ld_22050[11];

extern const ixheaace_bandwidth_table bandwidth_table_ld_24000[11];

extern const ixheaace_bandwidth_table bandwidth_table_ld_32000[11];

extern const ixheaace_bandwidth_table bandwidth_table_ld_44100[11];

extern const ixheaace_bandwidth_table bandwidth_table_ld_48000[11];
