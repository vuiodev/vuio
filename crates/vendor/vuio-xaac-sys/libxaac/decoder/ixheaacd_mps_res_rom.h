/******************************************************************************
 *
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
#ifndef IXHEAACD_MPS_RES_ROM_H
#define IXHEAACD_MPS_RES_ROM_H

typedef struct {
  WORD32 pow_table_q17[129];
  WORD32 scale_table[4];
  WORD32 scale_table_960[4];
  WORD8 tns_max_bands_tbl[12][2];
  WORD16 tns_coeff3_16[8];
  WORD16 tns_coeff4_16[16];
} ia_mps_dec_res_block_tables_struct;

extern const ia_mps_dec_res_block_tables_struct ixheaacd_mps_dec_res_block_tables;

typedef struct {
  WORD8 sfb_96_1024[42 + 1];
  WORD8 sfb_96_128[13 + 1];
  WORD8 sfb_64_1024[48 + 1];
  WORD8 sfb_48_1024[50 + 1];
  WORD8 sfb_48_128[15 + 1];
  WORD8 sfb_32_1024[52 + 1];
  WORD8 sfb_24_1024[48 + 1];
  WORD8 sfb_24_128[16 + 1];
  WORD8 sfb_16_1024[44 + 1];
  WORD8 sfb_16_128[16 + 1];
  WORD8 sfb_8_1024[41 + 1];
  WORD8 sfb_8_128[16 + 1];
  WORD8 sfb_96_960[40 + 1];
  WORD8 sfb_96_120[12 + 1];
  WORD8 sfb_64_960[46 + 1];
  WORD8 sfb_48_960[49 + 1];
  WORD8 sfb_48_120[14 + 1];
  WORD8 sfb_24_960[46 + 1];
  WORD8 sfb_24_120[15 + 1];
  WORD8 sfb_16_960[42 + 1];
  WORD8 sfb_16_120[15 + 1];
  WORD8 sfb_8_960[40 + 1];
  WORD8 sfb_8_120[15 + 1];

  UWORD16 huffman_code_book_1[108];
  UWORD16 huffman_code_book_2[110];
  UWORD16 huffman_code_book_3[136];
  UWORD16 huffman_code_book_4[116];
  UWORD16 huffman_code_book_5[126];
  UWORD16 huffman_code_book_6[120];
  UWORD16 huffman_code_book_7[112];
  UWORD16 huffman_code_book_8[92];
  UWORD16 huffman_code_book_9[236];
  UWORD16 huffman_code_book_10[218];
  UWORD16 huffman_codebook_11[344];
  UWORD16 huffman_code_book_scl[273];

} ia_mps_dec_res_huffmann_tables_struct;

extern const ia_mps_dec_res_huffmann_tables_struct ixheaacd_mps_dec_res_huffmann_tables;

typedef struct {
  ia_mps_dec_res_block_tables_struct *res_block_tables_ptr;
  ia_mps_dec_res_huffmann_tables_struct *res_huffmann_tables_ptr;
  WORD8 *scale_factor_bands_long[24];
  WORD8 *scale_factor_bands_short[24];
  WORD16 *sfb_index_long;
  WORD16 *sfb_index_short;
  WORD8 *sfb_index_long_width;
  WORD8 *sfb_index_short_width;
  UWORD16 *code_book[13];

} ia_mps_dec_residual_aac_tables_struct;

#endif /* IXHEAACD_MPS_RES_ROM_H */
