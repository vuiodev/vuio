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
#define INVALID_BITCOUNT_LC (SHRT_MAX)
#define INVALID_BITCOUNT_LD (SHRT_MAX / 4)

#define abs32(a) (a > 0 ? a : -a)
#define HI_LTAB(a) (a >> 8)
#define LO_LTAB(a) (a & 0xff)

#define HI_EXPLTAB(a) (a >> 16)
#define LO_EXPLTAB(a) (a & 0xffff)
#define EXPAND(a) ((((WORD32)(a & 0xff00)) << 8) | (WORD32)(a & 0xff))

/*  code book number table */
enum codeBookNo {
  CODE_BCK_ZERO_NO = 0,
  CODE_BCK_1_NO = 1,
  CODE_BCK_2_NO = 2,
  CODE_BCK_3_NO = 3,
  CODE_BCK_4_NO = 4,
  CODE_BCK_5_NO = 5,
  CODE_BCK_6_NO = 6,
  CODE_BCK_7_NO = 7,
  CODE_BCK_8_NO = 8,
  CODE_BCK_9_NO = 9,
  CODE_BCK_10_NO = 10,
  CODE_BCK_ESC_NO = 11,
  CODE_BCK_RES_NO = 12,
  CODE_BCK_PNS_NO = 13
};

/*  code book index table */
enum codeBookNdx {
  CODE_BCK_ZERO_NDX,
  CODE_BCK_1_NDX,
  CODE_BCK_2_NDX,
  CODE_BCK_3_NDX,
  CODE_BCK_4_NDX,
  CODE_BCK_5_NDX,
  CODE_BCK_6_NDX,
  CODE_BCK_7_NDX,
  CODE_BCK_8_NDX,
  CODE_BCK_9_NDX,
  CODE_BCK_10_NDX,
  CODE_BCK_ESC_NDX,
  CODE_BCK_RES_NDX,
  CODE_BCK_PNS_NDX,
  NUMBER_OF_CODE_BOOKS
};

/*  code book lav table */
enum codeBookLav {
  CODE_BCK_ZERO_LAV = 0,
  CODE_BCK_1_LAV = 1,
  CODE_BCK_2_LAV = 1,
  CODE_BCK_3_LAV = 2,
  CODE_BCK_4_LAV = 2,
  CODE_BCK_5_LAV = 4,
  CODE_BCK_6_LAV = 4,
  CODE_BCK_7_LAV = 7,
  CODE_BCK_8_LAV = 7,
  CODE_BCK_9_LAV = 12,
  CODE_BCK_10_LAV = 12,
  CODE_BCK_ESC_LAV = 16,
  CODE_BCK_SCF_LAV = 60,
  CODE_BCK_PNS_LAV = 60
};

VOID ia_enhaacplus_enc_bitcount(const WORD16 *ptr_values, const WORD32 width, WORD32 max_val,
                                WORD32 *bit_cnt, ixheaace_huffman_tables *pstr_huffman_tbl,
                                WORD32 aot);

VOID ia_enhaacplus_enc_code_values(WORD16 *ptr_values, WORD32 width, WORD32 code_book,
                                   ixheaace_bit_buf_handle pstr_bitstream,
                                   ixheaace_huffman_tables *pstr_huffman_tbl);

VOID ia_enhaacplus_enc_code_scale_factor_delta(WORD32 scalefactor,
                                               ixheaace_bit_buf_handle h_bitstream,
                                               ixheaace_huffman_tables *pstr_huffman_tbl);

VOID ia_enhaacplus_enc_count1_2_3_4_5_6_7_8_9_10_11(const WORD16 *values, const WORD32 width,
                                                    WORD32 *bitcnt,
                                                    ixheaace_huffman_tables *pstr_huffman_tbl,
                                                    WORD32 invalid_bitcnt);
