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
#ifndef IXHEAACD_INFO_H
#define IXHEAACD_INFO_H

#define EXT_SBR_DATA 13

typedef struct {
  WORD32 samp_rate;
  WORD32 num_sfb_1024;
  const WORD16 *ptr_sfb_1024;
  WORD32 num_sfb_128;
  const WORD16 *ptr_sfb_128;
  WORD32 num_sfb_960;
  const WORD16 *ptr_sfb_960;
  WORD32 num_sfb_120;
  const WORD16 *ptr_sfb_120;
  WORD32 num_sfb_768;
  const WORD16 *ptr_sfb_768;
  WORD32 num_sfb_96;
  const WORD16 *ptr_sfb_96;
  WORD32 short_fss_width;
  WORD32 long_fss_groups;
  WORD32 num_sfb_480;
  const WORD16 *ptr_sfb_480;
  WORD32 num_sfb_512;
  const WORD16 *ptr_sfb_512;
} ia_usac_samp_rate_info;

typedef struct {
  WORD32 index;
  WORD32 len;
  UWORD32 code_word;
} ia_huff_code_word_struct;

typedef struct {
  WORD32 num;
  WORD32 dim;
  WORD32 lav;
  WORD32 lav_incr_esc;
  WORD32 huff_mode;
  WORD32 off;
  WORD32 sign_code_book;
  UWORD16 max_code_word_len;
  const ia_huff_code_word_struct *pstr_huff_code_word;
  const WORD16 *code_book_tbl;
  const WORD32 *idx_tbl;
} ia_huff_code_book_struct;

typedef struct {
  WORD32 num_ele;
  WORD32 ele_is_cpe[(1 << LEN_TAG)];
  WORD32 ele_tag[(1 << LEN_TAG)];
} ia_ele_list_struct;

typedef struct {
  WORD32 present;
  WORD32 ele_tag;
  WORD32 pseudo_enab;
} ia_mix_dwn_struct;

typedef struct {
  WORD32 tag;
  WORD32 profile;
  WORD32 sampling_rate_idx;
  ia_ele_list_struct front;
  ia_ele_list_struct side;
  ia_ele_list_struct back;
  ia_ele_list_struct lfe;
  ia_ele_list_struct data;
  ia_ele_list_struct coupling;
  ia_mix_dwn_struct mono_mix;
  ia_mix_dwn_struct stereo_mix;
  ia_mix_dwn_struct matrix_mix;
  WORD8 comments[(1 << LEN_PC_COMM) + 1];
  WORD32 buffer_fullness;
} ia_prog_config_struct;

extern ia_huff_code_book_struct ixheaacd_book;

VOID ixheaacd_hufftab(ia_huff_code_book_struct *ptr_huff_code_book,
                      const ia_huff_code_word_struct *ptr_huff_code_word,
                      const WORD16 *code_book_tbl, const WORD32 *index,
                      WORD32 dim, WORD32 lav, WORD32 lav_incr_esc,
                      WORD32 sign_code_book, UWORD8 max_code_word_len);

WORD32 ixheaacd_huff_codeword(const ia_huff_code_word_struct *h,
                              UWORD16 data_present,
                              ia_bit_buf_struct *it_bit_buff);

#endif /* IXHEAACD_INFO_H */
