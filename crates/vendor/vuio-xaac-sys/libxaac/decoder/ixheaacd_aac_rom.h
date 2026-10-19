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
#ifndef IXHEAACD_AAC_ROM_H
#define IXHEAACD_AAC_ROM_H

#define MAX_CB_SIZE 13

typedef struct {
  WORD32 ixheaacd_pow_table_Q13[129];
  WORD32 scale_table[4];
  WORD8 tns_max_bands_tbl[12][2];
  WORD16 tns_coeff3_16[8];
  WORD16 tns_coeff4_16[16];
  WORD32 scale_mant_tab[4];

  WORD32 tns_coeff3[8];
  WORD32 tns_coeff4[16];

  WORD32 tns_coeff3_32[8];
  WORD32 tns_coeff4_32[16];
  WORD32 tns_max_bands_tbl_usac[16][2];

  WORD8 tns_max_bands_tbl_ld[12];
  WORD8 tns_max_bands_tbl_480[12];
  WORD32 scale_table_960[4];
} ia_aac_dec_block_tables_struct;

extern const ia_aac_dec_block_tables_struct ixheaacd_aac_block_tables;

typedef struct {
  WORD8 ixheaacd_sfb_96_1024[43];
  WORD8 ixheaacd_sfb_96_128[14];
  WORD8 ixheaacd_sfb_64_1024[49];
  WORD8 ixheaacd_sfb_48_1024[51];
  WORD8 ixheaacd_sfb_48_128[16];
  WORD8 ixheaacd_sfb_32_1024[53];
  WORD8 ixheaacd_sfb_24_1024[49];
  WORD8 ixheaacd_sfb_24_128[17];
  WORD8 ixheaacd_sfb_16_1024[45];
  WORD8 ixheaacd_sfb_16_128[17];
  WORD8 ixheaacd_sfb_8_1024[42];
  WORD8 ixheaacd_sfb_8_128[17];

  ia_sampling_rate_info_struct str_sample_rate_info[13];

  UWORD32 idx_table_hf11[21];
  UWORD32 idx_table_hf10[20];
  UWORD32 idx_table_hf9[23];
  UWORD32 idx_table_hf8[17];
  UWORD32 idx_table_hf7[18];
  UWORD32 idx_table_hf6[17];
  UWORD32 idx_table_hf5[19];
  UWORD32 idx_table_hf4[19];
  UWORD32 idx_table_hf3[27];
  UWORD32 idx_table_hf2[16];
  UWORD32 idx_table_hf1[12];

  UWORD16 input_table_cb11[290];
  UWORD16 input_table_cb10[170];
  UWORD16 input_table_cb9[170];
  UWORD16 input_table_cb8[65];
  UWORD16 input_table_cb7[65];
  UWORD16 input_table_cb6[82];
  UWORD16 input_table_cb5[82];
  UWORD16 input_table_cb4[82];
  UWORD16 input_table_cb3[82];
  UWORD16 input_table_cb2[82];
  UWORD16 input_table_cb1[82];
  UWORD16 huffman_code_book_scl[122];
  UWORD32 huffman_code_book_scl_index[33];

  WORD8 ixheaacd_sfb_48_512[37];
  WORD8 ixheaacd_sfb_32_512[38];
  WORD8 ixheaacd_sfb_24_512[32];

  WORD8 ixheaacd_sfb_48_480[36];
  WORD8 ixheaacd_sfb_32_480[38];
  WORD8 ixheaacd_sfb_24_480[31];

  WORD8 ixheaacd_sfb_96_960[41];
  WORD8 ixheaacd_sfb_96_120[13];
  WORD8 ixheaacd_sfb_64_960[47];
  WORD8 ixheaacd_sfb_48_960[50];
  WORD8 ixheaacd_sfb_48_120[15];
  WORD8 ixheaacd_sfb_24_960[47];
  WORD8 ixheaacd_sfb_24_120[16];
  WORD8 ixheaacd_sfb_16_960[43];
  WORD8 ixheaacd_sfb_16_120[16];
  WORD8 ixheaacd_sfb_8_960[41];
  WORD8 ixheaacd_sfb_8_120[16];
} ia_aac_dec_huffman_tables_struct;

extern const ia_aac_dec_huffman_tables_struct ixheaacd_aac_huffmann_tables;

typedef struct {
  WORD16 cosine_array_2048_256[514];
  WORD8 dig_rev_table8_long[64];
  WORD8 dig_rev_table8_short[8];
  WORD32 fft_twiddle[64 * 7];

  WORD16 only_long_window_sine[1024];
  WORD16 only_long_window_kbd[1024];
  WORD16 only_short_window_sine[128];
  WORD16 only_short_window_kbd[128];

  WORD16 cosine_array_2048_256p[514];
  WORD32 w1024[768];
  UWORD8 bit_rev_1024[256];
  UWORD8 bit_rev_512[64];
  UWORD8 bit_rev_128[16];
  UWORD8 bit_rev_32[4];
  WORD32 w_256[504];
  WORD32 low_overlap_win[512];
  WORD32 window_sine_512[512];
  WORD32 cosine_array_1024[512];

  WORD32 low_overlap_win_480[480];
  WORD32 window_sine_480[480];

  UWORD8 re_arr_tab_16[240];
  UWORD8 re_arr_tab_sml_240[240];

  WORD32 cosine_array_960[480];
  WORD32 w_16[24];

  WORD16 window_sine_480_eld[1920];
  WORD16 window_sine_512_eld[2048];

  WORD16 only_long_window_sine_960[960];
  WORD16 only_long_window_kbd_960[960];
  WORD16 only_short_window_sine_120[120];
  WORD16 only_short_window_kbd_120[120];

  WORD16 re_arr_tab_32[480];
  WORD16 re_arr_tab_sml[16];
  WORD16 re_arr_tab_4[60];
  WORD16 re_arr_tab_15_4[60];
  WORD16 re_arr_tab_120[60];
  WORD16 re_arr_tab_5[16];
  WORD16 re_arr_tab_3[16];

  WORD16 re_arr_tab_sml_480[480];

  WORD32 cosine_array_1920[960];

  WORD16 w_512[2 * 510];
  WORD16 w_32[60];
  WORD16 cosine_array_240[120];
} ia_aac_dec_imdct_tables_struct;

extern const ia_aac_dec_imdct_tables_struct ixheaacd_imdct_tables;

typedef struct {
  WORD16 *sfb_index;
  WORD8 *sfb_width;

} ia_aac_sfb_info;

typedef struct {
  ia_aac_dec_block_tables_struct *pstr_block_tables;
  ia_aac_dec_huffman_tables_struct *pstr_huffmann_tables;
  ia_aac_dec_imdct_tables_struct *pstr_imdct_tables;

  ia_aac_sfb_info str_aac_sfb_info[4];
  WORD8 *scale_factor_bands_long[24];
  WORD8 *scale_factor_bands_short[24];
  WORD16 sfb_long_table[52];
  WORD16 sfb_short_table[16];

  UWORD16 *code_book[MAX_CB_SIZE];
  UWORD32 *index_table[MAX_CB_SIZE];

  WORD8 *scale_fac_bands_512[16];
  WORD8 *scale_fac_bands_480[16];

} ia_aac_dec_tables_struct;

#define ixheaacd_huff_cb_0 0
#define ixheaacd_huff_cb_1 1
#define ixheaacd_huff_cb_2 2
#define ixheaacd_huff_cb_3 3
#define ixheaacd_huff_cb_4 4
#define ixheaacd_huff_cb_5 5
#define ixheaacd_huff_cb_6 6
#define ixheaacd_huff_cb_7 7

#endif /* #ifndef IXHEAACD_AAC_ROM_H */
