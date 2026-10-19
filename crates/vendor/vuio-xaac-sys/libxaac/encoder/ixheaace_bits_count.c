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
#include <limits.h>
#include <stdlib.h>
#include "ixheaac_type_def.h"
#include "ixheaac_constants.h"
#include "impd_drc_common_enc.h"
#include "impd_drc_uni_drc.h"
#include "impd_drc_tables.h"
#include "impd_drc_api.h"
#include "ixheaace_api.h"
#include "ixheaace_aac_constants.h"

#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops16.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaac_basic_ops.h"

#include "ixheaace_bitbuffer.h"
#include "ixheaace_psy_const.h"
#include "ixheaace_tns.h"
#include "ixheaace_tns_params.h"
#include "ixheaace_rom.h"
#include "ixheaace_bits_count.h"

#include "ixheaace_common_rom.h"
#include "ixheaace_common_utils.h"

VOID ia_enhaacplus_enc_count1_2_3_4_5_6_7_8_9_10_11(const WORD16 *values, const WORD32 width,
                                                    WORD32 *bitcnt,
                                                    ixheaace_huffman_tables *pstr_huffman_tbl,
                                                    WORD32 invalid_bitcnt) {
  WORD32 i;
  WORD32 bits1_2, bits3_4, bits5_6, bits7_8, bits9_10, bit11, sc;
  WORD32 temp_0, temp_1, temp_2, temp_3;
  (VOID) invalid_bitcnt;
  bits1_2 = 0;
  bits3_4 = 0;
  bits5_6 = 0;
  bits7_8 = 0;
  bits9_10 = 0;
  bit11 = 0;
  sc = 0;

  for (i = 0; i < width; i += 4) {
    temp_0 = values[i + 0];
    temp_1 = values[i + 1];
    temp_2 = values[i + 2];
    temp_3 = values[i + 3];

    bits1_2 +=
        EXPAND(pstr_huffman_tbl->huff_ltab1_2[temp_0 + 1][temp_1 + 1][temp_2 + 1][temp_3 + 1]);

    bits5_6 += EXPAND(pstr_huffman_tbl->huff_ltab5_6[temp_0 + 4][temp_1 + 4]);

    bits5_6 += EXPAND(pstr_huffman_tbl->huff_ltab5_6[temp_2 + 4][temp_3 + 4]);

    sc += (temp_0 != 0) + (temp_1 != 0) + (temp_2 != 0) + (temp_3 != 0);

    temp_0 = abs32(temp_0);
    temp_1 = abs32(temp_1);
    temp_2 = abs32(temp_2);
    temp_3 = abs32(temp_3);

    bits3_4 += EXPAND(pstr_huffman_tbl->huff_ltab3_4[temp_0][temp_1][temp_2][temp_3]);

    bits7_8 += EXPAND(pstr_huffman_tbl->huff_ltab7_8[temp_0][temp_1]);

    bits7_8 += EXPAND(pstr_huffman_tbl->huff_ltab7_8[temp_2][temp_3]);

    bits9_10 += EXPAND(pstr_huffman_tbl->huff_ltab9_10[temp_0][temp_1]);

    bits9_10 += EXPAND(pstr_huffman_tbl->huff_ltab9_10[temp_2][temp_3]);

    bit11 += EXPAND(pstr_huffman_tbl->huff_ltab11[temp_0][temp_1]);

    bit11 += EXPAND(pstr_huffman_tbl->huff_ltab11[temp_2][temp_3]);
  }

  bitcnt[1] = HI_EXPLTAB(bits1_2);

  bitcnt[2] = LO_EXPLTAB(bits1_2);

  bitcnt[3] = HI_EXPLTAB(bits3_4) + sc;

  bitcnt[4] = LO_EXPLTAB(bits3_4) + sc;

  bitcnt[5] = HI_EXPLTAB(bits5_6);

  bitcnt[6] = LO_EXPLTAB(bits5_6);

  bitcnt[7] = HI_EXPLTAB(bits7_8) + sc;

  bitcnt[8] = LO_EXPLTAB(bits7_8) + sc;

  bitcnt[9] = HI_EXPLTAB(bits9_10) + sc;

  bitcnt[10] = LO_EXPLTAB(bits9_10) + sc;

  bitcnt[11] = bit11 + sc;
}

static VOID ia_enhaacplus_enc_count3_4_5_6_7_8_9_10_11(const WORD16 *values, const WORD32 width,
                                                       WORD32 *bitcnt,
                                                       ixheaace_huffman_tables *pstr_huffman_tbl,
                                                       WORD32 invalid_bitcnt) {
  WORD32 i;
  WORD32 bits3_4, bits5_6, bits7_8, bits9_10, bit11, sc;
  WORD32 temp_0, temp_1, temp_2, temp_3;

  bits3_4 = 0;
  bits5_6 = 0;
  bits7_8 = 0;
  bits9_10 = 0;
  bit11 = 0;
  sc = 0;

  for (i = 0; i < width; i += 4) {
    temp_0 = values[i + 0];
    temp_1 = values[i + 1];
    temp_2 = values[i + 2];
    temp_3 = values[i + 3];

    bits5_6 += EXPAND(pstr_huffman_tbl->huff_ltab5_6[temp_0 + 4][temp_1 + 4]);

    bits5_6 += EXPAND(pstr_huffman_tbl->huff_ltab5_6[temp_2 + 4][temp_3 + 4]);

    temp_0 = abs32(temp_0);
    temp_1 = abs32(temp_1);
    temp_2 = abs32(temp_2);
    temp_3 = abs32(temp_3);

    bits3_4 += EXPAND(pstr_huffman_tbl->huff_ltab3_4[temp_0][temp_1][temp_2][temp_3]);

    bits7_8 += EXPAND(pstr_huffman_tbl->huff_ltab7_8[temp_0][temp_1]);

    bits7_8 += EXPAND(pstr_huffman_tbl->huff_ltab7_8[temp_2][temp_3]);

    bits9_10 += EXPAND(pstr_huffman_tbl->huff_ltab9_10[temp_0][temp_1]);

    bits9_10 += EXPAND(pstr_huffman_tbl->huff_ltab9_10[temp_2][temp_3]);

    bit11 += EXPAND(pstr_huffman_tbl->huff_ltab11[temp_0][temp_1]);

    bit11 += EXPAND(pstr_huffman_tbl->huff_ltab11[temp_2][temp_3]);

    sc += (temp_0 > 0) + (temp_1 > 0) + (temp_2 > 0) + (temp_3 > 0);
  }

  bitcnt[1] = invalid_bitcnt;
  bitcnt[2] = invalid_bitcnt;

  bitcnt[3] = HI_EXPLTAB(bits3_4) + sc;

  bitcnt[4] = LO_EXPLTAB(bits3_4) + sc;

  bitcnt[5] = HI_EXPLTAB(bits5_6);

  bitcnt[6] = LO_EXPLTAB(bits5_6);

  bitcnt[7] = HI_EXPLTAB(bits7_8) + sc;

  bitcnt[8] = LO_EXPLTAB(bits7_8) + sc;

  bitcnt[9] = HI_EXPLTAB(bits9_10) + sc;

  bitcnt[10] = LO_EXPLTAB(bits9_10) + sc;

  bitcnt[11] = bit11 + sc;
}

static VOID ia_enhaacplus_enc_count5_6_7_8_9_10_11(const WORD16 *values, const WORD32 width,
                                                   WORD32 *bitcnt,
                                                   ixheaace_huffman_tables *pstr_huffman_tbl,
                                                   WORD32 invalid_bitcnt) {
  WORD32 i;
  WORD32 bits5_6, bits7_8, bits9_10, bit11, sc;
  WORD32 temp_0, temp_1;

  bits5_6 = 0;
  bits7_8 = 0;
  bits9_10 = 0;
  bit11 = 0;
  sc = 0;

  for (i = 0; i < width; i += 2) {
    temp_0 = values[i + 0];
    temp_1 = values[i + 1];

    bits5_6 += EXPAND(pstr_huffman_tbl->huff_ltab5_6[temp_0 + 4][temp_1 + 4]);

    temp_0 = abs32(temp_0);
    temp_1 = abs32(temp_1);

    bits7_8 += EXPAND(pstr_huffman_tbl->huff_ltab7_8[temp_0][temp_1]);

    bits9_10 += EXPAND(pstr_huffman_tbl->huff_ltab9_10[temp_0][temp_1]);

    bit11 += EXPAND(pstr_huffman_tbl->huff_ltab11[temp_0][temp_1]);

    sc += (temp_0 > 0) + (temp_1 > 0);
  }

  bitcnt[1] = invalid_bitcnt;
  bitcnt[2] = invalid_bitcnt;
  bitcnt[3] = invalid_bitcnt;
  bitcnt[4] = invalid_bitcnt;

  bitcnt[5] = HI_EXPLTAB(bits5_6);

  bitcnt[6] = LO_EXPLTAB(bits5_6);

  bitcnt[7] = HI_EXPLTAB(bits7_8) + sc;

  bitcnt[8] = LO_EXPLTAB(bits7_8) + sc;

  bitcnt[9] = HI_EXPLTAB(bits9_10) + sc;

  bitcnt[10] = LO_EXPLTAB(bits9_10) + sc;

  bitcnt[11] = bit11 + sc;
}

static VOID ia_enhaacplus_enc_count7_8_9_10_11(const WORD16 *values, const WORD32 width,
                                               WORD32 *bitcnt,
                                               ixheaace_huffman_tables *pstr_huffman_tbl,
                                               WORD32 invalid_bitcnt) {
  WORD32 i;
  WORD32 bits7_8, bits9_10, bit11, sc;
  WORD32 temp_0, temp_1;

  bits7_8 = 0;
  bits9_10 = 0;
  bit11 = 0;
  sc = 0;

  for (i = 0; i < width; i += 2) {
    temp_0 = abs32(values[i + 0]);
    temp_1 = abs32(values[i + 1]);

    bits7_8 += EXPAND(pstr_huffman_tbl->huff_ltab7_8[temp_0][temp_1]);

    bits9_10 += EXPAND(pstr_huffman_tbl->huff_ltab9_10[temp_0][temp_1]);

    bit11 += EXPAND(pstr_huffman_tbl->huff_ltab11[temp_0][temp_1]);

    sc += (temp_0 > 0) + (temp_1 > 0);
  }

  bitcnt[1] = invalid_bitcnt;
  bitcnt[2] = invalid_bitcnt;
  bitcnt[3] = invalid_bitcnt;
  bitcnt[4] = invalid_bitcnt;
  bitcnt[5] = invalid_bitcnt;
  bitcnt[6] = invalid_bitcnt;

  bitcnt[7] = HI_EXPLTAB(bits7_8) + sc;

  bitcnt[8] = LO_EXPLTAB(bits7_8) + sc;

  bitcnt[9] = HI_EXPLTAB(bits9_10) + sc;

  bitcnt[10] = LO_EXPLTAB(bits9_10) + sc;

  bitcnt[11] = bit11 + sc;
}

static VOID ia_enhaacplus_enc_count9_10_11(const WORD16 *values, const WORD32 width,
                                           WORD32 *bitcnt,
                                           ixheaace_huffman_tables *pstr_huffman_tbl,
                                           WORD32 invalid_bitcnt) {
  WORD32 i;
  WORD32 bits9_10, bit11, sc;
  WORD32 temp_0, temp_1;

  bits9_10 = 0;
  bit11 = 0;
  sc = 0;

  for (i = 0; i < width; i += 2) {
    temp_0 = abs32(values[i + 0]);
    temp_1 = abs32(values[i + 1]);

    bits9_10 += EXPAND(pstr_huffman_tbl->huff_ltab9_10[temp_0][temp_1]);

    bit11 += EXPAND(pstr_huffman_tbl->huff_ltab11[temp_0][temp_1]);

    sc += (temp_0 > 0) + (temp_1 > 0);
  }

  bitcnt[1] = invalid_bitcnt;
  bitcnt[2] = invalid_bitcnt;
  bitcnt[3] = invalid_bitcnt;
  bitcnt[4] = invalid_bitcnt;
  bitcnt[5] = invalid_bitcnt;
  bitcnt[6] = invalid_bitcnt;
  bitcnt[7] = invalid_bitcnt;
  bitcnt[8] = invalid_bitcnt;

  bitcnt[9] = HI_EXPLTAB(bits9_10) + sc;

  bitcnt[10] = LO_EXPLTAB(bits9_10) + sc;

  bitcnt[11] = bit11 + sc;
}

static VOID ia_enhaacplus_enc_count11(const WORD16 *values, const WORD32 width, WORD32 *bitcnt,
                                      ixheaace_huffman_tables *pstr_huffman_tbl,
                                      WORD32 invalid_bitcnt) {
  WORD32 i;
  WORD32 bit11, sc;
  WORD32 temp_0, temp_1;

  bit11 = 0;
  sc = 0;

  for (i = 0; i < width; i += 2) {
    temp_0 = abs32(values[i + 0]);
    temp_1 = abs32(values[i + 1]);

    bit11 += EXPAND(pstr_huffman_tbl->huff_ltab11[temp_0][temp_1]);

    sc += (temp_0 > 0) + (temp_1 > 0);
  }

  bitcnt[1] = invalid_bitcnt;
  bitcnt[2] = invalid_bitcnt;
  bitcnt[3] = invalid_bitcnt;
  bitcnt[4] = invalid_bitcnt;
  bitcnt[5] = invalid_bitcnt;
  bitcnt[6] = invalid_bitcnt;
  bitcnt[7] = invalid_bitcnt;
  bitcnt[8] = invalid_bitcnt;
  bitcnt[9] = invalid_bitcnt;
  bitcnt[10] = invalid_bitcnt;

  bitcnt[11] = bit11 + sc;
}

static VOID ia_enhaacplus_enc_count_esc(const WORD16 *values, const WORD32 width, WORD32 *bitcnt,
                                        ixheaace_huffman_tables *pstr_huffman_tbl,
                                        WORD32 invalid_bitcnt) {
  WORD32 i;
  WORD32 bit11, ec, sc;
  WORD32 temp_0, temp_1, t00, t01;

  bit11 = 0;
  sc = 0;
  ec = 0;

  for (i = 0; i < width; i += 2) {
    temp_0 = abs32(values[i + 0]);
    temp_1 = abs32(values[i + 1]);

    sc += (temp_0 > 0) + (temp_1 > 0);
    t00 = MIN(temp_0, 16);
    t01 = MIN(temp_1, 16);

    bit11 += EXPAND(pstr_huffman_tbl->huff_ltab11[t00][t01]);

    if (temp_0 >= 16) {
      ec += 5;

      while ((temp_0 >>= 1) >= 16) {
        ec += 2;
      }
    }

    if (temp_1 >= 16) {
      ec += 5;

      while ((temp_1 >>= 1) >= 16) {
        ec += 2;
      }
    }
  }

  bitcnt[1] = invalid_bitcnt;
  bitcnt[2] = invalid_bitcnt;
  bitcnt[3] = invalid_bitcnt;
  bitcnt[4] = invalid_bitcnt;
  bitcnt[5] = invalid_bitcnt;
  bitcnt[6] = invalid_bitcnt;
  bitcnt[7] = invalid_bitcnt;
  bitcnt[8] = invalid_bitcnt;
  bitcnt[9] = invalid_bitcnt;
  bitcnt[10] = invalid_bitcnt;

  bitcnt[11] = bit11 + sc + ec;
}

typedef VOID (*COUNT_FUNCTION)(const WORD16 *values, const WORD32 width, WORD32 *bitcnt,
                               ixheaace_huffman_tables *pstr_huffman_tbl, WORD32 invalid_bitcnt);

static COUNT_FUNCTION ia_enhaacplus_enc_count_func_tab[CODE_BCK_ESC_LAV + 1] = {

    ia_enhaacplus_enc_count1_2_3_4_5_6_7_8_9_10_11, /* 0  */
    ia_enhaacplus_enc_count1_2_3_4_5_6_7_8_9_10_11, /* 1  */
    ia_enhaacplus_enc_count3_4_5_6_7_8_9_10_11,     /* 2  */
    ia_enhaacplus_enc_count5_6_7_8_9_10_11,         /* 3  */
    ia_enhaacplus_enc_count5_6_7_8_9_10_11,         /* 4  */
    ia_enhaacplus_enc_count7_8_9_10_11,             /* 5  */
    ia_enhaacplus_enc_count7_8_9_10_11,             /* 6  */
    ia_enhaacplus_enc_count7_8_9_10_11,             /* 7  */
    ia_enhaacplus_enc_count9_10_11,                 /* 8  */
    ia_enhaacplus_enc_count9_10_11,                 /* 9  */
    ia_enhaacplus_enc_count9_10_11,                 /* 10 */
    ia_enhaacplus_enc_count9_10_11,                 /* 11 */
    ia_enhaacplus_enc_count9_10_11,                 /* 12 */
    ia_enhaacplus_enc_count11,                      /* 13 */
    ia_enhaacplus_enc_count11,                      /* 14 */
    ia_enhaacplus_enc_count11,                      /* 15 */
    ia_enhaacplus_enc_count_esc                     /* 16 */
};

VOID ia_enhaacplus_enc_bitcount(const WORD16 *ptr_values, const WORD32 width, WORD32 max_val,
                                WORD32 *bit_cnt, ixheaace_huffman_tables *pstr_huffman_tbl,
                                WORD32 aot) {
  WORD32 invalid_bitcnt;

  if (max_val == 0) {
    *bit_cnt = 0;
  } else {
    switch (aot) {
      case AOT_AAC_LC:
      case AOT_SBR:
      case AOT_PS:
        *bit_cnt = INVALID_BITCOUNT_LC;
        break;

      case AOT_AAC_LD:
      case AOT_AAC_ELD:
        *bit_cnt = INVALID_BITCOUNT_LD;
        break;
    }
  }
  invalid_bitcnt = *bit_cnt;
  max_val = MIN(max_val, CODE_BCK_ESC_LAV);

  ia_enhaacplus_enc_count_func_tab[max_val](ptr_values, width, bit_cnt, pstr_huffman_tbl,
                                            invalid_bitcnt);
}

VOID ia_enhaacplus_enc_code_values(WORD16 *ptr_values, WORD32 width, WORD32 code_book,
                                   ixheaace_bit_buf_handle pstr_bitstream,
                                   ixheaace_huffman_tables *pstr_huffman_tbl) {
  WORD32 i, temp_0, temp_1, temp_2, temp_3, t00, t01;
  WORD32 code_word, code_length;
  WORD32 sign, sign_length;
  WORD16 *ptr_temp_values = ptr_values;

  switch (code_book) {
    case CODE_BCK_ZERO_NO:
      break;

    case CODE_BCK_1_NO:
      width = width >> 2;

      for (i = width - 1; i >= 0; i--) {
        temp_0 = *ptr_temp_values++;
        temp_1 = *ptr_temp_values++;
        temp_2 = *ptr_temp_values++;
        temp_3 = *ptr_temp_values++;

        code_word = pstr_huffman_tbl->huff_ctab1[temp_0 + 1][temp_1 + 1][temp_2 + 1][temp_3 + 1];

        code_length = HI_LTAB(
            pstr_huffman_tbl->huff_ltab1_2[temp_0 + 1][temp_1 + 1][temp_2 + 1][temp_3 + 1]);

        ixheaace_write_bits(pstr_bitstream, code_word, (UWORD8)code_length);
      }
      break;

    case CODE_BCK_2_NO:

      width = width >> 2;

      for (i = width - 1; i >= 0; i--) {
        temp_0 = *ptr_temp_values++;
        temp_1 = *ptr_temp_values++;
        temp_2 = *ptr_temp_values++;
        temp_3 = *ptr_temp_values++;

        code_word = pstr_huffman_tbl->huff_ctab2[temp_0 + 1][temp_1 + 1][temp_2 + 1][temp_3 + 1];

        code_length = LO_LTAB(
            pstr_huffman_tbl->huff_ltab1_2[temp_0 + 1][temp_1 + 1][temp_2 + 1][temp_3 + 1]);

        ixheaace_write_bits(pstr_bitstream, code_word, (UWORD8)code_length);
      }
      break;

    case CODE_BCK_3_NO:

      for (i = 0; i < width; i += 4) {
        sign = 0;
        sign_length = 0;
        temp_0 = ptr_values[i + 0];

        if (temp_0 != 0) {
          sign_length++;

          sign <<= 1;

          if (temp_0 < 0) {
            sign |= 1;

            temp_0 = abs32(temp_0);
          }
        }

        temp_1 = ptr_values[i + 1];

        if (temp_1 != 0) {
          sign_length++;

          sign <<= 1;

          if (temp_1 < 0) {
            sign |= 1;

            temp_1 = abs32(temp_1);
          }
        }

        temp_2 = ptr_values[i + 2];

        if (temp_2 != 0) {
          sign_length++;

          sign <<= 1;

          if (temp_2 < 0) {
            sign |= 1;

            temp_2 = abs32(temp_2);
          }
        }

        temp_3 = ptr_values[i + 3];

        if (temp_3 != 0) {
          sign_length++;

          sign <<= 1;

          if (temp_3 < 0) {
            sign |= 1;

            temp_3 = abs32(temp_3);
          }
        }

        code_word = pstr_huffman_tbl->huff_ctab3[temp_0][temp_1][temp_2][temp_3];

        code_length = HI_LTAB(pstr_huffman_tbl->huff_ltab3_4[temp_0][temp_1][temp_2][temp_3]);

        ixheaace_write_bits(pstr_bitstream, code_word, (UWORD8)code_length);

        ixheaace_write_bits(pstr_bitstream, sign, (UWORD8)sign_length);
      }
      break;

    case CODE_BCK_4_NO:

      for (i = 0; i < width; i += 4) {
        sign = 0;
        sign_length = 0;
        temp_0 = ptr_values[i + 0];

        if (temp_0 != 0) {
          sign_length++;

          sign <<= 1;

          if (temp_0 < 0) {
            sign |= 1;

            temp_0 = abs32(temp_0);
          }
        }

        temp_1 = ptr_values[i + 1];

        if (temp_1 != 0) {
          sign_length++;

          sign <<= 1;

          if (temp_1 < 0) {
            sign |= 1;

            temp_1 = abs32(temp_1);
          }
        }

        temp_2 = ptr_values[i + 2];

        if (temp_2 != 0) {
          sign_length++;

          sign <<= 1;

          if (temp_2 < 0) {
            sign |= 1;

            temp_2 = abs32(temp_2);
          }
        }

        temp_3 = ptr_values[i + 3];

        if (temp_3 != 0) {
          sign_length++;

          sign <<= 1;

          if (temp_3 < 0) {
            sign |= 1;

            temp_3 = abs32(temp_3);
          }
        }

        code_word = pstr_huffman_tbl->huff_ctab4[temp_0][temp_1][temp_2][temp_3];

        code_length = LO_LTAB(pstr_huffman_tbl->huff_ltab3_4[temp_0][temp_1][temp_2][temp_3]);

        ixheaace_write_bits(pstr_bitstream, code_word, (UWORD8)code_length);

        ixheaace_write_bits(pstr_bitstream, sign, (UWORD8)sign_length);
      }
      break;

    case CODE_BCK_5_NO:

      width = width >> 1;

      for (i = width - 1; i >= 0; i--) {
        temp_0 = *ptr_temp_values++;
        temp_1 = *ptr_temp_values++;

        code_word = pstr_huffman_tbl->huff_ctab5[temp_0 + 4][temp_1 + 4];

        code_length = HI_LTAB(pstr_huffman_tbl->huff_ltab5_6[temp_0 + 4][temp_1 + 4]);

        ixheaace_write_bits(pstr_bitstream, code_word, (UWORD8)code_length);
      }
      break;

    case CODE_BCK_6_NO:

      width = width >> 1;

      for (i = width - 1; i >= 0; i--) {
        temp_0 = *ptr_temp_values++;
        temp_1 = *ptr_temp_values++;

        code_word = pstr_huffman_tbl->huff_ctab6[temp_0 + 4][temp_1 + 4];

        code_length = LO_LTAB(pstr_huffman_tbl->huff_ltab5_6[temp_0 + 4][temp_1 + 4]);

        ixheaace_write_bits(pstr_bitstream, code_word, (UWORD8)code_length);
      }
      break;

    case CODE_BCK_7_NO:

      for (i = 0; i < width; i += 2) {
        sign = 0;
        sign_length = 0;
        temp_0 = ptr_values[i + 0];

        if (temp_0 != 0) {
          sign_length++;

          sign <<= 1;

          if (temp_0 < 0) {
            sign |= 1;

            temp_0 = abs32(temp_0);
          }
        }

        temp_1 = ptr_values[i + 1];

        if (temp_1 != 0) {
          sign_length++;

          sign <<= 1;

          if (temp_1 < 0) {
            sign |= 1;

            temp_1 = abs32(temp_1);
          }
        }

        code_word = pstr_huffman_tbl->huff_ctab7[temp_0][temp_1];

        code_length = HI_LTAB(pstr_huffman_tbl->huff_ltab7_8[temp_0][temp_1]);

        ixheaace_write_bits(pstr_bitstream, code_word, (UWORD8)code_length);

        ixheaace_write_bits(pstr_bitstream, sign, (UWORD8)sign_length);
      }
      break;

    case CODE_BCK_8_NO:

      for (i = 0; i < width; i += 2) {
        sign = 0;
        sign_length = 0;
        temp_0 = ptr_values[i + 0];

        if (temp_0 != 0) {
          sign_length++;

          sign <<= 1;

          if (temp_0 < 0) {
            sign |= 1;

            temp_0 = abs32(temp_0);
          }
        }

        temp_1 = ptr_values[i + 1];

        if (temp_1 != 0) {
          sign_length++;

          sign <<= 1;

          if (temp_1 < 0) {
            sign |= 1;

            temp_1 = abs32(temp_1);
          }
        }

        code_word = pstr_huffman_tbl->huff_ctab8[temp_0][temp_1];

        code_length = LO_LTAB(pstr_huffman_tbl->huff_ltab7_8[temp_0][temp_1]);

        ixheaace_write_bits(pstr_bitstream, code_word, (UWORD8)code_length);

        ixheaace_write_bits(pstr_bitstream, sign, (UWORD8)sign_length);
      }
      break;

    case CODE_BCK_9_NO:

      for (i = 0; i < width; i += 2) {
        sign = 0;
        sign_length = 0;
        temp_0 = ptr_values[i + 0];

        if (temp_0 != 0) {
          sign_length++;

          sign <<= 1;

          if (temp_0 < 0) {
            sign |= 1;

            temp_0 = abs32(temp_0);
          }
        }

        temp_1 = ptr_values[i + 1];

        if (temp_1 != 0) {
          sign_length++;

          sign <<= 1;

          if (temp_1 < 0) {
            sign |= 1;

            temp_1 = abs32(temp_1);
          }
        }

        code_word = pstr_huffman_tbl->huff_ctab9[temp_0][temp_1];

        code_length = HI_LTAB(pstr_huffman_tbl->huff_ltab9_10[temp_0][temp_1]);

        ixheaace_write_bits(pstr_bitstream, code_word, (UWORD8)code_length);

        ixheaace_write_bits(pstr_bitstream, sign, (UWORD8)sign_length);
      }
      break;

    case CODE_BCK_10_NO:

      for (i = 0; i < width; i += 2) {
        sign = 0;
        sign_length = 0;
        temp_0 = ptr_values[i + 0];

        if (temp_0 != 0) {
          sign_length++;

          sign <<= 1;

          if (temp_0 < 0) {
            sign |= 1;

            temp_0 = abs32(temp_0);
          }
        }

        temp_1 = ptr_values[i + 1];

        if (temp_1 != 0) {
          sign_length++;

          sign <<= 1;

          if (temp_1 < 0) {
            sign |= 1;

            temp_1 = abs32(temp_1);
          }
        }

        code_word = pstr_huffman_tbl->huff_ctab10[temp_0][temp_1];

        code_length = LO_LTAB(pstr_huffman_tbl->huff_ltab9_10[temp_0][temp_1]);

        ixheaace_write_bits(pstr_bitstream, code_word, (UWORD8)code_length);

        ixheaace_write_bits(pstr_bitstream, sign, (UWORD8)sign_length);
      }
      break;

    case CODE_BCK_ESC_NO:

      for (i = 0; i < width; i += 2) {
        sign = 0;
        sign_length = 0;
        temp_0 = ptr_values[i + 0];

        if (temp_0 != 0) {
          sign_length++;

          sign <<= 1;

          if (temp_0 < 0) {
            sign |= 1;

            temp_0 = abs32(temp_0);
          }
        }

        temp_1 = ptr_values[i + 1];

        if (temp_1 != 0) {
          sign_length++;

          sign <<= 1;

          if (temp_1 < 0) {
            sign |= 1;

            temp_1 = abs32(temp_1);
          }
        }

        t00 = MIN(temp_0, 16);
        t01 = MIN(temp_1, 16);

        code_word = pstr_huffman_tbl->huff_ctab11[t00][t01];
        code_length = pstr_huffman_tbl->huff_ltab11[t00][t01];

        ixheaace_write_bits(pstr_bitstream, code_word, (UWORD8)code_length);

        ixheaace_write_bits(pstr_bitstream, sign, (UWORD8)sign_length);

        if (temp_0 >= 16) {
          WORD32 n, p;

          n = 0;
          p = temp_0;

          while ((p >>= 1) >= 16) {
            ixheaace_write_bits(pstr_bitstream, 1, 1);

            n++;
          }

          ixheaace_write_bits(pstr_bitstream, 0, 1);

          ixheaace_write_bits(pstr_bitstream, temp_0 - (1 << (n + 4)), (UWORD8)(n + 4));
        }
        if (temp_1 >= 16) {
          WORD32 n, p;

          n = 0;
          p = temp_1;

          while ((p >>= 1) >= 16) {
            ixheaace_write_bits(pstr_bitstream, 1, 1);

            n++;
          }

          ixheaace_write_bits(pstr_bitstream, 0, 1);

          ixheaace_write_bits(pstr_bitstream, temp_1 - (1 << (n + 4)), (UWORD8)(n + 4));
        }
      }
      break;

    default:
      break;
  }
}

VOID ia_enhaacplus_enc_code_scale_factor_delta(WORD32 delta,
                                               ixheaace_bit_buf_handle h_bitstream,
                                               ixheaace_huffman_tables *pstr_huffman_tbl) {
  WORD32 code_word, code_length;

  code_word = pstr_huffman_tbl->huff_ctabscf[delta + CODE_BCK_SCF_LAV];
  code_length = pstr_huffman_tbl->huff_ltabscf[delta + CODE_BCK_SCF_LAV];

  ixheaace_write_bits(h_bitstream, code_word, (UWORD8)code_length);
}
