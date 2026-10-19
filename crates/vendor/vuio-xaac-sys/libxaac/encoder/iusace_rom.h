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
extern const FLOAT64 iusace_twiddle_table_fft_32x32[514];
extern const FLOAT64 iusace_twiddle_table_3pr[1155];
extern const FLOAT64 iusace_twiddle_table_3pi[1155];
extern const FLOAT64 iusace_twiddle_sin_2048[1024];
extern const FLOAT64 iusace_twiddle_cos_2048[1024];
extern const FLOAT32 ia_fft_twiddle_table_float[514];
extern const FLOAT32 ia_mixed_rad_twiddle_cos[16384];
extern const FLOAT32 ia_mixed_rad_twiddle_sin[16384];

#define WIN_LEN_1024 1024
#define WIN_LEN_768 768
#define WIN_LEN_128 128
#define WIN_LEN_256 256
#define WIN_LEN_192 192
#define WIN_LEN_96 96

extern const FLOAT32 iusace_iir_hipass_coeffs[BLK_SWITCH_FILT_LEN];
extern const WORD32 iusace_suggested_grouping_table[MAX_SHORT_WINDOWS][MAXIMUM_NO_OF_GROUPS];
extern const WORD32 iusace_synchronized_block_types[4][4];

extern const FLOAT32 iusace_gamma_table[ORDER + 1];
extern const FLOAT32 iusace_chebyshev_polyn_grid[101];

extern const UWORD32 iusace_sampl_freq_idx_table[32];
extern const WORD32 iusace_bandwidth_table[8][2];

extern const WORD32 iusace_huffman_code_table[121][2];

extern const FLOAT64 iusace_pre_post_twid_cos_2048[512];
extern const FLOAT64 iusace_pre_post_twid_sin_2048[512];
extern const FLOAT64 iexheaac_pre_post_twid_cos_1536[384];
extern const FLOAT64 iexheaac_pre_post_twid_sin_1536[384];
extern const FLOAT64 iusace_pre_post_twid_cos_256[64];
extern const FLOAT64 iusace_pre_post_twid_sin_256[64];
extern const FLOAT64 iexheaac_pre_post_twid_cos_192[48];
extern const FLOAT64 iexheaac_pre_post_twid_sin_192[48];
extern const FLOAT64 iusace_kbd_win1024[1024];
extern const FLOAT64 iusace_kbd_win256[256];
extern const FLOAT64 iusace_kbd_win128[128];

extern const FLOAT64 iexheaac_kbd_win_768[768];
extern const FLOAT64 iexheaac_kbd_win_192[192];
extern const FLOAT64 iexheaac_kbd_win_96[96];

extern const FLOAT64 iusace_sine_win_1024[1024];
extern const FLOAT64 iexheaac_sine_win_768[768];
extern const FLOAT64 iusace_sine_win_256[256];
extern const FLOAT64 iusace_sine_win_128[128];
extern const FLOAT64 iexheaac_sine_win_192[192];
extern const FLOAT64 iexheaac_sine_win_96[96];

extern const UWORD16 iusace_ari_cf_r[3][4];
extern const UWORD16 iusace_ari_lookup_m[742];
extern const UWORD32 iusace_ari_hash_m[742];
extern const UWORD8 iusace_ari_hash_m_lsb[742];
extern const UWORD16 iusace_ari_cf_m[64][17];

extern const FLOAT32 iusace_pre_post_twid_cos_sin_512[4][512];
extern const FLOAT32 iusace_pre_post_twid_cos_sin_256[4][256];
extern const FLOAT32 iusace_pre_post_twid_cos_sin_128[4][128];
extern const FLOAT32 iusace_pre_post_twid_cos_sin_64[4][64];
extern const FLOAT32 iusace_pre_post_twid_cos_sin_32[4][32];

extern const FLOAT64 iusace_pow_table[9000];

extern const FLOAT64 iusace_mdst_fcoeff_long_sin[];
extern const FLOAT64 iusace_mdst_fcoeff_long_kbd[];
extern const FLOAT64 iusace_mdst_fcoeff_long_sin_kbd[];
extern const FLOAT64 iusace_mdst_fcoeff_long_kbd_sin[];
extern const FLOAT64 *const iusace_mdst_fcoeff_longshort_curr[2][2];

extern const FLOAT64 iusace_mdst_fcoeff_start_sin[];
extern const FLOAT64 iusace_mdst_fcoeff_start_kbd[];
extern const FLOAT64 iusace_mdst_fcoeff_start_sin_kbd[];
extern const FLOAT64 iusace_mdst_fcoeff_start_kbd_sin[];

extern const FLOAT64 *const iusace_mdst_fcoeff_start_curr[2][2];

extern const FLOAT64 iusace_mdst_fcoeff_stop_sin[];
extern const FLOAT64 iusace_mdst_fcoeff_stop_kbd[];
extern const FLOAT64 iusace_mdst_fcoeff_stop_sin_kbd[];
extern const FLOAT64 iusace_mdst_fcoeff_stop_kbd_sin[];

extern const FLOAT64 *const iusace_mdst_fcoeff_stop_cur[2][2];

extern const FLOAT64 iusace_mdst_fcoeff_stopstart_sin[];
extern const FLOAT64 iusace_mdst_fcoeff_stopstart_kbd[];
extern const FLOAT64 iusace_mdst_fcoeff_stopstart_sin_kbd[];
extern const FLOAT64 iusace_mdst_fcoeff_stopstart_kbd_sin[];

extern const FLOAT64 *const iusace_mdst_fcoeff_stopstart_cur[2][2];

extern const FLOAT64 iusace_mdst_fcoeff_l_s_start_left_sin[];
extern const FLOAT64 iusace_mdst_fcoeff_l_s_start_left_kbd[];

extern const FLOAT64 iusace_mdst_fcoeff_stop_stopstart_left_sin[];
extern const FLOAT64 iusace_mdst_fcoeff_stop_stopstart_left_kbd[];

extern const FLOAT64 *const iusace_mdst_fcoeff_l_s_start_left_prev[2];

extern const FLOAT64 *const iusace_mdst_fcoeff_stop_stopstart_left_prev[2];

extern const FLOAT32 ia_rad_3_fft_twiddle_re[1155];

extern const FLOAT32 ia_rad_3_fft_twiddle_im[1155];

extern const FLOAT32 ia_fft_mix_rad_twid_tbl_336[564];

extern const FLOAT32 ia_fft_mix_rad_twid_tbl_168[276];
