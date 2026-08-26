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
#include <stdlib.h>
#include "ixheaacd_sbr_common.h"
#include "ixheaac_type_def.h"

#include "ixheaac_constants.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops16.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaac_basic_ops.h"
#include "ixheaacd_common_rom.h"
#include "ixheaacd_basic_funcs.h"
#include "ixheaacd_defines.h"
#include "ixheaacd_aac_rom.h"
#include "ixheaacd_bitbuffer.h"
#include "ixheaacd_intrinsics.h"
#include "ixheaacd_pulsedata.h"

#include "ixheaacd_pns.h"
#include "ixheaacd_drc_data_struct.h"

#include "ixheaacd_lt_predict.h"

#include "ixheaacd_cnst.h"
#include "ixheaacd_ec_defines.h"
#include "ixheaacd_ec_struct_def.h"
#include "ixheaacd_channelinfo.h"
#include "ixheaacd_drc_dec.h"

#include "ixheaacd_block.h"
#include "ixheaacd_channel.h"

#include "ixheaac_basic_op.h"

#include "ixheaacd_tns.h"
#include "ixheaacd_sbrdecoder.h"
#include "ixheaacd_error_codes.h"

#include "ixheaacd_audioobjtypes.h"
#include "ixheaacd_latmdemux.h"

#include "ixheaacd_aacdec.h"

static PLATFORM_INLINE WORD32 ixheaacd_mac32x16in32_sat(WORD32 a, WORD32 b,
                                                        WORD16 c) {
  WORD32 acc;

  acc = ixheaac_mult32x16in32_sat(b, c);

  acc = ixheaac_add32_sat(a, acc);

  return acc;
}

WORD32 ixheaacd_cnt_leading_ones(WORD32 a);

VOID ixheaacd_huff_sfb_table(WORD32 it_bit_buff, WORD16 *huff_index,
                             WORD32 *len, const UWORD16 *code_book_tbl,
                             const UWORD32 *idx_table) {
  UWORD32 temp = 0;
  UWORD32 temp1 = 0;
  WORD32 found = 0;
  UWORD32 mask = 0x80000000;

  WORD32 leading_ones;
  WORD32 max_len;
  WORD32 ixheaacd_drc_offset = 0;
  WORD32 length;
  UWORD32 code_word;
  WORD32 len_end;

  max_len = code_book_tbl[0];
  mask = mask - (1 << (31 - max_len));
  mask = mask << 1;

  temp = (UWORD32)((it_bit_buff & mask));

  len_end = code_book_tbl[0];
  leading_ones = ixheaacd_cnt_leading_ones(temp);
  do {
    ixheaacd_drc_offset = (idx_table[leading_ones] >> 20) & 0x1ff;
    length = code_book_tbl[ixheaacd_drc_offset + 1] & 0x1f;
    code_word = idx_table[leading_ones] & 0xfffff;
    temp1 = temp >> (32 - length);
    if (temp1 <= code_word) {
      ixheaacd_drc_offset = ixheaacd_drc_offset - (code_word - temp1);
      found = 1;
    } else {
      len_end = len_end + ((idx_table[leading_ones] >> 29) & 0x7);
      leading_ones = len_end;
    }
  } while (!found);
  *huff_index = code_book_tbl[ixheaacd_drc_offset + 1] >> 5;
  *len = length;
}

VOID ixheaacd_inverse_quantize(WORD32 *x_invquant, WORD no_band,
                               WORD32 *ixheaacd_pow_table_Q13,
                               WORD8 *scratch_in) {
  WORD32 j;
  WORD32 temp;
  WORD32 q_abs;

  for (j = no_band - 1; j >= 0; j--) {
    q_abs = *scratch_in++;
    temp = (ixheaacd_pow_table_Q13[q_abs]);
    *x_invquant++ = -temp;
  }
}

static PLATFORM_INLINE WORD ixheaacd_huffman_dec_word1(
    ia_bit_buf_struct *it_bit_buff, WORD32 *spec_coef, WORD16 *offsets,
    WORD no_bands, WORD group_len, const UWORD16 *code_book_tbl,
    WORD32 *ixheaacd_pow_table_Q13, const UWORD32 *idx_table, WORD32 maximum_bins_short) {
  WORD32 sp1, sp2;
  WORD32 flush_cw;
  WORD32 i, value, norm_val, off;
  WORD idx, grp_idx;
  WORD32 out1, out2;
  WORD32 err_code = 0;
  WORD len_idx = 0;
  UWORD8 *ptr_read_next = it_bit_buff->ptr_read_next;
  WORD32 bit_pos = it_bit_buff->bit_pos;
  WORD16 index;
  WORD32 length;
  WORD32 read_word;
  WORD32 increment;

  read_word = ixheaacd_aac_showbits_32(ptr_read_next, it_bit_buff->cnt_bits,
                                       &increment);
  ptr_read_next += increment;

  do {
    len_idx = offsets[1] - offsets[0];
    grp_idx = group_len;

    do {
      spec_coef = spec_coef + offsets[0];
      idx = len_idx;
      do {
        {
          UWORD32 read_word1;

          read_word1 = read_word << bit_pos;
          ixheaacd_huff_sfb_table(read_word1, &index, &length, code_book_tbl,
                                  idx_table);
          bit_pos += length;
          ixheaacd_aac_read_byte_corr(&ptr_read_next, &bit_pos, &read_word,
                                      it_bit_buff->ptr_bit_buf_end);
        }

        out1 = index / 17;
        out2 = index - out1 * 17;
        flush_cw = read_word << bit_pos;

        sp1 = out1;
        sp2 = out2;

        if (out1) {
          if (flush_cw & 0x80000000) {
            out1 = -out1;
          }
          bit_pos++;
          flush_cw = (WORD32)flush_cw << 1;
        }

        if (out2) {
          bit_pos++;
          if (flush_cw & 0x80000000) {
            out2 = -out2;
          }
        }
        ixheaacd_aac_read_byte_corr(&ptr_read_next, &bit_pos, &read_word,
                                    it_bit_buff->ptr_bit_buf_end);

        if (sp1 == 16) {
          i = 4;
          value = ixheaac_extu(read_word, bit_pos, 23);
          value = value | 0xfffffe00;
          norm_val = ixheaac_norm32(value);

          i += (norm_val - 22);
          bit_pos += (norm_val - 21);
          ixheaacd_aac_read_byte_corr(&ptr_read_next, &bit_pos, &read_word,
                                      it_bit_buff->ptr_bit_buf_end);

          off = ixheaac_extu(read_word, bit_pos, 32 - i);

          bit_pos += i;

          ixheaacd_aac_read_byte_corr(&ptr_read_next, &bit_pos, &read_word,
                                      it_bit_buff->ptr_bit_buf_end);
          ixheaacd_aac_read_byte_corr(&ptr_read_next, &bit_pos, &read_word,
                                      it_bit_buff->ptr_bit_buf_end);

          i = off + ((WORD32)1 << i);

          if (i <= IQ_TABLE_SIZE_HALF)
            i = ixheaacd_pow_table_Q13[i];
          else {
            err_code |= ixheaacd_inv_quant(&i, ixheaacd_pow_table_Q13);
          }

          if (out1 < 0) {
            out1 = -i;
          } else {
            out1 = i;
          }
          *spec_coef++ = out1;
        } else {
          if (out1 <= 0) {
            out1 = -out1;
            out1 = ixheaacd_pow_table_Q13[out1];
            *spec_coef++ = -out1;
          } else {
            out1 = ixheaacd_pow_table_Q13[out1];
            *spec_coef++ = out1;
          }
        }

        if (sp2 == 16) {
          i = 4;
          value = ixheaac_extu(read_word, bit_pos, 23);
          value = value | 0xfffffe00;
          norm_val = ixheaac_norm32(value);

          i += (norm_val - 22);

          bit_pos += (norm_val - 21);
          ixheaacd_aac_read_byte_corr(&ptr_read_next, &bit_pos, &read_word,
                                      it_bit_buff->ptr_bit_buf_end);

          off = ixheaac_extu(read_word, bit_pos, 32 - i);

          bit_pos += i;

          ixheaacd_aac_read_byte_corr(&ptr_read_next, &bit_pos, &read_word,
                                      it_bit_buff->ptr_bit_buf_end);
          ixheaacd_aac_read_byte_corr(&ptr_read_next, &bit_pos, &read_word,
                                      it_bit_buff->ptr_bit_buf_end);

          i = off + ((WORD32)1 << i);

          if (i <= IQ_TABLE_SIZE_HALF)
            i = ixheaacd_pow_table_Q13[i];
          else {
            err_code |= ixheaacd_inv_quant(&i, ixheaacd_pow_table_Q13);
          }

          if (out2 < 0) {
            out2 = -i;
          } else {
            out2 = i;
          }
          *spec_coef++ = out2;
        } else {
          if (out2 <= 0) {
            out2 = -out2;
            out2 = ixheaacd_pow_table_Q13[out2];
            *spec_coef++ = -out2;
          } else {
            out2 = ixheaacd_pow_table_Q13[out2];
            *spec_coef++ = out2;
          }
        }

        idx -= 2;
      } while (idx != 0);

      if (maximum_bins_short == 120) {
        spec_coef += (maximum_bins_short - offsets[1]);
      } else {
        spec_coef += (MAX_BINS_SHORT - offsets[1]);
      }

      grp_idx--;
    } while (grp_idx != 0);

    offsets++;

    if (maximum_bins_short == 120) {
      spec_coef -= (maximum_bins_short * group_len);
    } else {
      spec_coef -= (MAX_BINS_SHORT * group_len);
    }

    no_bands--;
  } while (no_bands >= 0);

  ptr_read_next = ptr_read_next - increment;
  ixheaacd_aac_read_byte_corr1(&ptr_read_next, &bit_pos, &read_word,
                               it_bit_buff->ptr_bit_buf_end);

  it_bit_buff->bit_pos = bit_pos;
  it_bit_buff->ptr_read_next = ptr_read_next;

  return err_code;
}

static PLATFORM_INLINE WORD ixheaacd_huffman_dec_word2_11(
    ia_bit_buf_struct *it_bit_buff, WORD32 width, const UWORD16 *code_book_tbl,
    WORD32 *x_invquant, WORD32 *ixheaacd_pow_table_Q13, WORD8 *ptr_scratch,
    const UWORD32 *idx_table) {
  WORD32 sp1, sp2;
  WORD32 flush_cw;
  WORD32 i, value, norm_val, off;
  WORD idx;
  WORD32 out1, out2;
  WORD32 err_code = 0;
  WORD16 index;
  WORD32 length;
  UWORD8 *ptr_read_next = it_bit_buff->ptr_read_next;
  WORD32 bit_pos = it_bit_buff->bit_pos;
  WORD32 read_word;
  WORD32 increment;

  read_word = ixheaacd_aac_showbits_32(ptr_read_next, it_bit_buff->cnt_bits,
                                       &increment);
  ptr_read_next += increment;

  for (idx = width; idx != 0; idx -= 2) {
    {
      UWORD32 read_word1;

      read_word1 = read_word << bit_pos;
      ixheaacd_huff_sfb_table(read_word1, &index, &length, code_book_tbl,
                              idx_table);
      bit_pos += length;
      ixheaacd_aac_read_byte_corr(&ptr_read_next, &bit_pos, &read_word,
                                  it_bit_buff->ptr_bit_buf_end);
    }

    flush_cw = read_word << bit_pos;
    out1 = index / 17;
    out2 = index - out1 * 17;
    sp1 = out1;

    if (out1) {
      if (flush_cw & 0x80000000) {
        out1 = -out1;
      }

      bit_pos++;
      flush_cw = (WORD32)flush_cw << 1;
    }

    sp2 = out2;
    if (out2) {
      bit_pos++;
      if (flush_cw & 0x80000000) {
        out2 = -out2;
      }
    }

    ixheaacd_aac_read_byte_corr(&ptr_read_next, &bit_pos, &read_word,
                                it_bit_buff->ptr_bit_buf_end);

    if (sp1 == 16) {
      i = 4;
      value = ixheaac_extu(read_word, bit_pos, 23);
      value = value | 0xfffffe00;
      norm_val = ixheaac_norm32(value);
      i += (norm_val - 22);
      bit_pos += (norm_val - 21);

      ixheaacd_aac_read_byte_corr(&ptr_read_next, &bit_pos, &read_word,
                                  it_bit_buff->ptr_bit_buf_end);

      off = ixheaac_extu(read_word, bit_pos, 32 - i);

      bit_pos += i;
      ixheaacd_aac_read_byte_corr(&ptr_read_next, &bit_pos, &read_word,
                                  it_bit_buff->ptr_bit_buf_end);

      value = *ptr_scratch++;

      ixheaacd_aac_read_byte_corr(&ptr_read_next, &bit_pos, &read_word,
                                  it_bit_buff->ptr_bit_buf_end);
      i = off + ((WORD32)1 << i);
      i += value;

      if (i <= IQ_TABLE_SIZE_HALF)
        i = ixheaacd_pow_table_Q13[i];
      else {
        err_code |= ixheaacd_inv_quant(&i, ixheaacd_pow_table_Q13);
      }
      if (out1 < 0) {
        i = -i;
      }
      *x_invquant++ = i;
    } else {
      WORD8 temp = *ptr_scratch++;
      if (out1 <= 0) {
        out1 = temp - out1;
        out1 = ixheaacd_pow_table_Q13[out1];
        *x_invquant++ = -out1;
      } else {
        out1 += temp;
        out1 = ixheaacd_pow_table_Q13[out1];
        *x_invquant++ = out1;
      }
    }

    if (sp2 == 16) {
      i = 4;
      value = ixheaac_extu(read_word, bit_pos, 23);
      value = value | 0xfffffe00;
      norm_val = ixheaac_norm32(value);

      i += (norm_val - 22);

      bit_pos += (norm_val - 21);
      ixheaacd_aac_read_byte_corr(&ptr_read_next, &bit_pos, &read_word,
                                  it_bit_buff->ptr_bit_buf_end);

      off = ixheaac_extu(read_word, bit_pos, 32 - i);

      bit_pos += i;
      ixheaacd_aac_read_byte_corr(&ptr_read_next, &bit_pos, &read_word,
                                  it_bit_buff->ptr_bit_buf_end);
      value = *ptr_scratch++;
      ixheaacd_aac_read_byte_corr(&ptr_read_next, &bit_pos, &read_word,
                                  it_bit_buff->ptr_bit_buf_end);

      i = off + ((WORD32)1 << i);
      i += value;
      if (i <= IQ_TABLE_SIZE_HALF)
        i = ixheaacd_pow_table_Q13[i];
      else {
        err_code |= ixheaacd_inv_quant(&i, ixheaacd_pow_table_Q13);
      }

      if (out2 < 0) {
        i = -i;
      }
      *x_invquant++ = i;

    } else {
      WORD8 temp = *ptr_scratch++;
      if (out2 <= 0) {
        out2 = temp - out2;
        out2 = ixheaacd_pow_table_Q13[out2];
        *x_invquant++ = -out2;
      } else {
        out2 += temp;
        out2 = ixheaacd_pow_table_Q13[out2];
        *x_invquant++ = out2;
      }
    }
  }
  ptr_read_next = ptr_read_next - increment;
  ixheaacd_aac_read_byte_corr1(&ptr_read_next, &bit_pos, &read_word,
                               it_bit_buff->ptr_bit_buf_end);

  it_bit_buff->ptr_read_next = ptr_read_next;
  it_bit_buff->bit_pos = bit_pos;

  return err_code;
}

static PLATFORM_INLINE WORD ixheaacd_huffman_dec_quad(
    ia_bit_buf_struct *it_bit_buff, WORD32 *spec_coef, WORD16 *offsets,
    WORD no_bands, WORD group_len, const UWORD16 *code_book_tbl,
    WORD32 *ixheaacd_pow_table_Q13, WORD32 tbl_sign, const UWORD32 *idx_table,
    WORD32 maximum_bins_short) {
  WORD idx, grp_idx;
  WORD idx_len;
  WORD32 *spec_orig;
  WORD16 index, length;
  UWORD8 *ptr_read_next = it_bit_buff->ptr_read_next;
  WORD32 bit_pos = it_bit_buff->bit_pos;
  WORD32 read_word;
  WORD32 increment;

  read_word = ixheaacd_aac_showbits_32(ptr_read_next, it_bit_buff->cnt_bits,
                                       &increment);
  ptr_read_next += increment;
  spec_orig = spec_coef;
  do {
    idx_len = offsets[1] - offsets[0];
    grp_idx = group_len;

    do {
      spec_coef = spec_coef + offsets[0];
      idx = idx_len;
      do {
        UWORD32 read_word1;

        read_word1 = read_word << bit_pos;
        ixheaacd_huffman_decode(read_word1, &index, &length, code_book_tbl,
                                idx_table);
        bit_pos += length;
        ixheaacd_aac_read_byte_corr(&ptr_read_next, &bit_pos, &read_word,
                                    it_bit_buff->ptr_bit_buf_end);
        if (tbl_sign) {
          WORD32 temp_word;
          WORD32 w, x, y, z;
          temp_word = read_word << bit_pos;
          w = index / 27;
          index = index - w * 27;
          x = index / 9;
          index = index - x * 9;
          y = index / 3;
          z = index - y * 3;
          if (w) {
            w = ixheaacd_pow_table_Q13[w];
            if (temp_word & 0x80000000) w = -w;
            temp_word <<= 1;
            bit_pos++;
          }
          *spec_coef++ = w;

          if (x) {
            x = ixheaacd_pow_table_Q13[x];
            if (temp_word & 0x80000000) x = -x;
            temp_word <<= 1;
            bit_pos++;
          }
          *spec_coef++ = x;
          if (y) {
            y = ixheaacd_pow_table_Q13[y];
            if (temp_word & 0x80000000) y = -y;
            temp_word <<= 1;
            bit_pos++;
          }
          *spec_coef++ = y;
          if (z) {
            z = ixheaacd_pow_table_Q13[z];
            if (temp_word & 0x80000000) z = -z;
            temp_word <<= 1;
            bit_pos++;
          }
          *spec_coef++ = z;

        }

        else {
          WORD32 w, x, y, z;

          w = index / 27 - 1;
          index = index - (w + 1) * 27;
          x = index / 9 - 1;
          index = index - (x + 1) * 9;
          y = index / 3 - 1;
          z = index - ((y + 1) * 3) - 1;
          if (w < 0) {
            w = -w;
            w = ixheaacd_pow_table_Q13[w];
            w = -w;
          } else
            w = ixheaacd_pow_table_Q13[w];

          *spec_coef++ = w;

          if (x < 0) {
            x = -x;
            x = ixheaacd_pow_table_Q13[x];
            x = -x;
          } else
            x = ixheaacd_pow_table_Q13[x];

          *spec_coef++ = x;

          if (y < 0) {
            y = -y;
            y = ixheaacd_pow_table_Q13[y];
            y = -y;
          } else
            y = ixheaacd_pow_table_Q13[y];

          *spec_coef++ = y;

          if (z < 0) {
            z = -z;
            z = ixheaacd_pow_table_Q13[z];
            z = -z;
          } else
            z = ixheaacd_pow_table_Q13[z];

          *spec_coef++ = z;
        }

        ixheaacd_aac_read_byte_corr(&ptr_read_next, &bit_pos, &read_word,
                                    it_bit_buff->ptr_bit_buf_end);
        idx -= 4;
      } while (idx != 0);

      if (maximum_bins_short == 120) {
        spec_coef += (maximum_bins_short - offsets[1]);
      } else {
        spec_coef += (MAX_BINS_SHORT - offsets[1]);
      }

      grp_idx--;
    } while (grp_idx != 0);
    offsets++;
    spec_coef = spec_orig;
    no_bands--;
  } while (no_bands >= 0);

  ptr_read_next = ptr_read_next - increment;
  ixheaacd_aac_read_byte_corr1(&ptr_read_next, &bit_pos, &read_word,
                               it_bit_buff->ptr_bit_buf_end);
  it_bit_buff->ptr_read_next = ptr_read_next;
  it_bit_buff->bit_pos = bit_pos;

  return 0;
}

static PLATFORM_INLINE WORD ixheaacd_huffman_dec_word2_quad(
    ia_bit_buf_struct *it_bit_buff, WORD32 width, const UWORD16 *code_book_tbl,
    WORD32 *x_invquant, WORD32 *ixheaacd_pow_table_Q13, WORD8 *ptr_scratch,
    WORD32 tbl_sign, const UWORD32 *idx_table) {
  WORD idx;
  WORD16 index, length;
  UWORD8 *ptr_read_next = it_bit_buff->ptr_read_next;
  WORD32 bit_pos = it_bit_buff->bit_pos;
  WORD32 read_word;
  WORD32 increment;

  read_word = ixheaacd_aac_showbits_32(ptr_read_next, it_bit_buff->cnt_bits,
                                       &increment);
  ptr_read_next += increment;

  for (idx = width; idx != 0; idx -= 4) {
    WORD32 ampres, ampres1;
    WORD32 ampres2, ampres3;
    UWORD32 read_word1;

    read_word1 = read_word << bit_pos;
    ixheaacd_huffman_decode(read_word1, &index, &length, code_book_tbl,
                            idx_table);
    bit_pos += length;
    ixheaacd_aac_read_byte_corr(&ptr_read_next, &bit_pos, &read_word,
                                it_bit_buff->ptr_bit_buf_end);
    if (tbl_sign) {
      WORD32 w, x, y, z;
      WORD32 ampout0, ampout1, ampout2, ampout3;
      WORD32 temp_word;
      temp_word = read_word << bit_pos;

      w = index / 27;
      index = index - w * 27;
      x = index / 9;
      index = index - x * 9;
      y = index / 3;
      z = index - y * 3;

      ampout0 = w + *ptr_scratch++;
      ampout0 = ixheaacd_pow_table_Q13[ampout0];

      if (w) {
        if (temp_word & 0x80000000) {
          ampout0 = -ampout0;
        }
        temp_word = temp_word << 1;
        bit_pos++;
      } else {
        ampout0 = -ampout0;
      }

      ampout1 = x + *ptr_scratch++;
      ampout1 = ixheaacd_pow_table_Q13[ampout1];

      if (x) {
        if (temp_word & 0x80000000) {
          ampout1 = -ampout1;
        }
        temp_word = temp_word << 1;
        bit_pos++;
      } else {
        ampout1 = -ampout1;
      }

      ampout2 = y + *ptr_scratch++;
      ampout2 = ixheaacd_pow_table_Q13[ampout2];

      if (y) {
        if (temp_word & 0x80000000) {
          ampout2 = -ampout2;
        }
        temp_word = temp_word << 1;
        bit_pos++;
      } else {
        ampout2 = -ampout2;
      }

      ampout3 = z + *ptr_scratch++;
      ampout3 = ixheaacd_pow_table_Q13[ampout3];

      if (z) {
        if (temp_word & 0x80000000) {
          ampout3 = -ampout3;
        }
        temp_word = temp_word << 1;
        bit_pos++;
      } else {
        ampout3 = -ampout3;
      }
      *x_invquant++ = ampout0;
      *x_invquant++ = ampout1;
      *x_invquant++ = ampout2;
      *x_invquant++ = ampout3;
    } else {
      WORD32 w, x, y, z;
      ampres = *ptr_scratch++;
      ampres1 = *ptr_scratch++;
      ampres2 = *ptr_scratch++;
      ampres3 = *ptr_scratch++;

      w = index / 27 - 1;
      index = index - (w + 1) * 27;
      x = index / 9 - 1;
      index = index - (x + 1) * 9;
      y = index / 3 - 1;
      z = index - ((y + 1) * 3) - 1;
      if (w <= 0) {
        ampres = ampres - w;
        ampres = ixheaacd_pow_table_Q13[ampres];
        ampres = -ampres;
      } else {
        ampres += w;
        ampres = ixheaacd_pow_table_Q13[ampres];
      }

      if (x <= 0) {
        ampres1 = ampres1 - x;
        ampres1 = ixheaacd_pow_table_Q13[ampres1];
        ampres1 = -ampres1;
      } else {
        ampres1 += x;
        ampres1 = ixheaacd_pow_table_Q13[ampres1];
      }

      if (y <= 0) {
        ampres2 = ampres2 - y;
        ampres2 = ixheaacd_pow_table_Q13[ampres2];
        ampres2 = -ampres2;
      } else {
        ampres2 += y;
        ampres2 = ixheaacd_pow_table_Q13[ampres2];
      }

      if (z <= 0) {
        ampres3 = ampres3 - z;
        ampres3 = ixheaacd_pow_table_Q13[ampres3];
        ampres3 = -ampres3;
      } else {
        ampres3 += z;
        ampres3 = ixheaacd_pow_table_Q13[ampres3];
      }

      *x_invquant++ = ampres;
      *x_invquant++ = ampres1;
      *x_invquant++ = ampres2;
      *x_invquant++ = ampres3;
    }

    ixheaacd_aac_read_byte_corr(&ptr_read_next, &bit_pos, &read_word,
                                it_bit_buff->ptr_bit_buf_end);
  }

  ptr_read_next = ptr_read_next - increment;
  ixheaacd_aac_read_byte_corr1(&ptr_read_next, &bit_pos, &read_word,
                               it_bit_buff->ptr_bit_buf_end);
  it_bit_buff->ptr_read_next = ptr_read_next;
  it_bit_buff->bit_pos = bit_pos;

  return 0;
}

static PLATFORM_INLINE WORD ixheaacd_huffman_dec_pair(
    ia_bit_buf_struct *it_bit_buff, WORD32 *spec_coef, WORD16 *offsets,
    WORD no_bands, WORD group_len, const UWORD16 *code_book_tbl,
    WORD32 *ixheaacd_pow_table_Q13, WORD32 tbl_sign, const UWORD32 *idx_table,
    WORD32 huff_mode, WORD32 maximum_bins_short) {
  WORD idx, grp_idx;
  WORD len_idx;
  WORD16 index, length;
  WORD32 y, z;
  WORD32 *spec_orig = spec_coef;

  UWORD8 *ptr_read_next = it_bit_buff->ptr_read_next;
  WORD32 bit_pos = it_bit_buff->bit_pos;
  WORD32 read_word;
  WORD32 increment;

  read_word = ixheaacd_aac_showbits_32(ptr_read_next, it_bit_buff->cnt_bits,
                                       &increment);
  ptr_read_next += increment;

  do {
    len_idx = offsets[1] - offsets[0];
    grp_idx = group_len;
    do {
      spec_coef += offsets[0];
      idx = len_idx;
      do {
        UWORD32 read_word1;
        read_word1 = read_word << bit_pos;
        ixheaacd_huffman_decode(read_word1, &index, &length, code_book_tbl,
                                idx_table);
        bit_pos += length;
        ixheaacd_aac_read_byte_corr(&ptr_read_next, &bit_pos, &read_word,
                                    it_bit_buff->ptr_bit_buf_end);
        if (tbl_sign) {
          WORD32 temp_word;
          temp_word = read_word << bit_pos;
          y = index / huff_mode;
          z = index - huff_mode * y;
          if (y) {
            y = ixheaacd_pow_table_Q13[y];
            if (temp_word & 0x80000000) y = -y;

            temp_word = temp_word << 1;
            bit_pos++;
          }
          *spec_coef++ = y;

          if (z) {
            z = ixheaacd_pow_table_Q13[z];
            if (temp_word & 0x80000000) {
              z = -z;
            }
            temp_word <<= 1;
            bit_pos++;
          }
          *spec_coef++ = z;
        } else {
          y = (index / huff_mode) - 4;
          z = index - ((y + 4) * huff_mode) - 4;
          if (y < 0) {
            y = -y;
            y = ixheaacd_pow_table_Q13[y];
            y = -y;
          } else
            y = ixheaacd_pow_table_Q13[y];

          if (z < 0) {
            z = -z;
            z = ixheaacd_pow_table_Q13[z];
            z = -z;
          } else
            z = ixheaacd_pow_table_Q13[z];

          *spec_coef++ = y;
          *spec_coef++ = z;
        }
        ixheaacd_aac_read_byte_corr(&ptr_read_next, &bit_pos, &read_word,
                                    it_bit_buff->ptr_bit_buf_end);
        idx -= 2;
      } while (idx != 0);

      if (maximum_bins_short == 120)
        spec_coef += (maximum_bins_short - offsets[1]);
      else
        spec_coef += (MAX_BINS_SHORT - offsets[1]);

      grp_idx--;
    } while (grp_idx != 0);

    offsets++;
    spec_coef = spec_orig;
    no_bands--;
  } while (no_bands >= 0);

  ptr_read_next = ptr_read_next - increment;
  ixheaacd_aac_read_byte_corr1(&ptr_read_next, &bit_pos, &read_word,
                               it_bit_buff->ptr_bit_buf_end);
  it_bit_buff->ptr_read_next = ptr_read_next;
  it_bit_buff->bit_pos = bit_pos;

  return 0;
}

static PLATFORM_INLINE WORD ixheaacd_huffman_dec_word2_pair(
    ia_bit_buf_struct *it_bit_buff, WORD32 width, const UWORD16 *code_book_tbl,
    WORD32 *x_invquant, WORD32 *ixheaacd_pow_table_Q13, WORD8 *ptr_scratch,
    WORD32 tbl_sign, const UWORD32 *idx_table, WORD32 huff_mode)

{
  WORD32 ampres;
  WORD idx;
  WORD16 index, length;
  UWORD8 *ptr_read_next = it_bit_buff->ptr_read_next;
  WORD32 bit_pos = it_bit_buff->bit_pos;
  WORD32 read_word;
  WORD32 increment;

  read_word = ixheaacd_aac_showbits_32(ptr_read_next, it_bit_buff->cnt_bits,
                                       &increment);
  ptr_read_next += increment;

  for (idx = width; idx != 0; idx -= 2) {
    {
      UWORD32 read_word1;
      read_word1 = read_word << bit_pos;
      ixheaacd_huffman_decode(read_word1, &index, &length, code_book_tbl,
                              idx_table);
      bit_pos += length;
      ixheaacd_aac_read_byte_corr(&ptr_read_next, &bit_pos, &read_word,
                                  it_bit_buff->ptr_bit_buf_end);
    }

    if (tbl_sign) {
      WORD32 out0, out1, temp_word;
      WORD32 ampout0, ampout1;

      ampout0 = *ptr_scratch++;
      ampout1 = *ptr_scratch++;
      out0 = index / huff_mode;
      out1 = index - huff_mode * out0;
      ampout0 += out0;
      ampout0 = ixheaacd_pow_table_Q13[ampout0];

      ampout1 += out1;
      ampout1 = ixheaacd_pow_table_Q13[ampout1];
      temp_word = read_word << bit_pos;
      if (out0) {
        if (temp_word & 0x80000000) {
          ampout0 = -(ampout0);
        }

        bit_pos++;
        temp_word = temp_word << 1;
      } else {
        ampout0 = -(ampout0);
      }

      if (out1) {
        if (temp_word & 0x80000000) {
          ampout1 = -(ampout1);
        }
        bit_pos++;
      } else {
        ampout1 = -(ampout1);
      }

      ixheaacd_aac_read_byte_corr(&ptr_read_next, &bit_pos, &read_word,
                                  it_bit_buff->ptr_bit_buf_end);
      *x_invquant++ = ampout0;
      *x_invquant++ = ampout1;
    } else {
      WORD32 y, z;
      y = (index / huff_mode) - 4;
      z = index - ((y + 4) * huff_mode) - 4;

      ampres = *ptr_scratch++;
      if (y <= 0) {
        ampres = ampres - y;
        ampres = ixheaacd_pow_table_Q13[ampres];
        *x_invquant++ = -ampres;
      } else {
        ampres += y;
        *x_invquant++ = ixheaacd_pow_table_Q13[ampres];
      }
      ampres = *ptr_scratch++;
      if (z <= 0) {
        ampres = ampres - z;
        ampres = ixheaacd_pow_table_Q13[ampres];
        *x_invquant++ = -ampres;
      } else {
        ampres += z;
        *x_invquant++ = ixheaacd_pow_table_Q13[ampres];
      }
    }
    ixheaacd_aac_read_byte_corr(&ptr_read_next, &bit_pos, &read_word,
                                it_bit_buff->ptr_bit_buf_end);
  }

  ptr_read_next = ptr_read_next - increment;
  ixheaacd_aac_read_byte_corr1(&ptr_read_next, &bit_pos, &read_word,
                               it_bit_buff->ptr_bit_buf_end);
  it_bit_buff->ptr_read_next = ptr_read_next;
  it_bit_buff->bit_pos = bit_pos;

  return 0;
}

WORD ixheaacd_decode_huffman(ia_bit_buf_struct *it_bit_buff, WORD32 cb_no,
                             WORD32 *spec_coef, WORD16 *sfb_offset, WORD start,
                             WORD sfb, WORD group_len,
                             ia_aac_dec_tables_struct *ptr_aac_tables,
                             WORD32 maximum_bins_short) {
  WORD ret_val = 0;
  WORD start_bit_pos = it_bit_buff->bit_pos;
  UWORD8 *start_read_pos = it_bit_buff->ptr_read_next;
  const UWORD16 *cb_table = (UWORD16 *)(ptr_aac_tables->code_book[cb_no]);
  WORD32 huff_mode;
  const UWORD32 *idx_table = (UWORD32 *)(ptr_aac_tables->index_table[cb_no]);
  WORD32 *pow_table =
      (WORD32 *)ptr_aac_tables->pstr_block_tables->ixheaacd_pow_table_Q13;
  WORD32 no_bands = sfb - start - 1;
  WORD16 *band_offset = sfb_offset + start;

  if (cb_no == 11) {
    const UWORD32 *idx_table =
        ptr_aac_tables->pstr_huffmann_tables->idx_table_hf11;
    const UWORD16 *cb_table =
        ptr_aac_tables->pstr_huffmann_tables->input_table_cb11;

    ret_val = ixheaacd_huffman_dec_word1(it_bit_buff, spec_coef, band_offset,
                                         no_bands, group_len, cb_table,
                                         pow_table, idx_table, maximum_bins_short);

  } else if (cb_no <= 4) {
    WORD32 tbl_sign = 0;

    if (cb_no > 2) {
      tbl_sign = 1;
    }
    ret_val = ixheaacd_huffman_dec_quad(it_bit_buff, spec_coef, band_offset,
                                        no_bands, group_len, cb_table,
                                        pow_table, tbl_sign, idx_table, maximum_bins_short);
  }

  else if (cb_no <= 10) {
    WORD32 tbl_sign = 0;
    huff_mode = 9;
    if (cb_no > 6) {
      if (cb_no > 8)
        huff_mode = 13;
      else
        huff_mode = 8;
      tbl_sign = 1;
    }
    ret_val = ixheaacd_huffman_dec_pair(
        it_bit_buff, spec_coef, band_offset, no_bands, group_len, cb_table,
        pow_table, tbl_sign, idx_table, huff_mode, maximum_bins_short);
  }

  {
    WORD bits_cons;
    bits_cons = (WORD)(((it_bit_buff->ptr_read_next - start_read_pos) << 3) +
                       (it_bit_buff->bit_pos - start_bit_pos));
    it_bit_buff->cnt_bits -= bits_cons;
  }
  return ret_val;
}

WORD ixheaacd_huffman_dec_word2(ia_bit_buf_struct *it_bit_buff, WORD32 cb_no,
                                WORD32 width,
                                ia_aac_dec_tables_struct *ptr_aac_tables,
                                WORD32 *x_invquant, WORD8 *scratch_ptr) {
  WORD ret_val = 0;
  WORD32 huff_mode;
  WORD start_bit_pos = it_bit_buff->bit_pos;
  WORD32 cnt_bits = it_bit_buff->cnt_bits;
  WORD32 *pow_table =
      (WORD32 *)ptr_aac_tables->pstr_block_tables->ixheaacd_pow_table_Q13;
  UWORD8 *start_read_pos = it_bit_buff->ptr_read_next;

  const UWORD16 *cb_table = (UWORD16 *)(ptr_aac_tables->code_book[cb_no]);
  const UWORD32 *idx_table = (UWORD32 *)(ptr_aac_tables->index_table[cb_no]);

  if (cb_no == 11) {
    const UWORD16 *cb_table =
        ptr_aac_tables->pstr_huffmann_tables->input_table_cb11;

    ret_val = ixheaacd_huffman_dec_word2_11(
        it_bit_buff, width, cb_table, x_invquant, pow_table, scratch_ptr,
        ptr_aac_tables->pstr_huffmann_tables->idx_table_hf11);
  } else if (cb_no <= 4) {
    WORD32 tbl_sign = 0;
    if (cb_no > 2) tbl_sign = 1;
    ret_val = ixheaacd_huffman_dec_word2_quad(it_bit_buff, width, cb_table,
                                              x_invquant, pow_table,
                                              scratch_ptr, tbl_sign, idx_table);
  } else if (cb_no <= 10) {
    WORD32 tbl_sign = 0;
    huff_mode = 9;
    if (cb_no > 6) {
      if (cb_no > 8) {
        huff_mode = 13;
      } else {
        huff_mode = 8;
      }

      tbl_sign = 1;
    }
    ret_val = ixheaacd_huffman_dec_word2_pair(
        it_bit_buff, width, cb_table, x_invquant, pow_table, scratch_ptr,
        tbl_sign, idx_table, huff_mode);
  }

  {
    WORD bits_cons;
    if (it_bit_buff->bit_pos <= 7) {
      bits_cons = (WORD)(((it_bit_buff->ptr_read_next - start_read_pos) << 3) +
                         (it_bit_buff->bit_pos - start_bit_pos));
      if (bits_cons > cnt_bits)
      {
        return IA_XHEAAC_DEC_EXE_NONFATAL_INSUFFICIENT_INPUT_BYTES;
      }
      it_bit_buff->cnt_bits = cnt_bits - bits_cons;
    } else {
      it_bit_buff->ptr_read_next += (it_bit_buff->bit_pos) >> 3;
      it_bit_buff->bit_pos = it_bit_buff->bit_pos & 0x7;
      if ((SIZE_T)(it_bit_buff->ptr_read_next) > (SIZE_T)(it_bit_buff->ptr_bit_buf_end + 1))
      {
        it_bit_buff->ptr_read_next = it_bit_buff->ptr_bit_buf_end + 1;
        return IA_XHEAAC_DEC_EXE_NONFATAL_INSUFFICIENT_INPUT_BYTES;
      }

      bits_cons = (WORD)(((it_bit_buff->ptr_read_next - start_read_pos) << 3) +
                         (it_bit_buff->bit_pos - start_bit_pos));
      it_bit_buff->cnt_bits = cnt_bits - bits_cons;
    }
  }
  return ret_val;
}

VOID ixheaacd_dec_copy_outsample(WORD32 *out_samples, WORD32 *p_overlap_buffer,
                                 WORD32 size, WORD16 stride) {
  WORD32 i;

  for (i = 0; i < size; i++) {
    out_samples[stride*i] = (ixheaac_shl16_sat((WORD16)(p_overlap_buffer[i]), 1) << 14);
  }
}

VOID ixheaacd_lap1_512_480(WORD32 *coef, WORD32 *prev, VOID *out_tmp,
                           const WORD16 *window, WORD16 q_shift, WORD16 size,
                           WORD16 stride , WORD slot_element) {
  WORD32 accu;
  WORD32 i;
  WORD16 rounding_fac = -0x2000;

  WORD32 *window_i = (WORD32 *)window;

  WORD16 *ptr_out1, *ptr_out2;

  WORD32 *pwin1, *pwin2;
  WORD32 *pCoef = &coef[size * 2 - 1 - 0];

  WORD16 * out  = (WORD16*)out_tmp - slot_element;

  pwin1 = &window_i[size - 1 - 0];
  pwin2 = &window_i[size + 0];

  ptr_out1 = &out[stride * (size - 1 - 0)];
  ptr_out2 = &out[stride * (size + 0)];

  for (i = 0; i < size; i++) {
    WORD32 win1, win2, coeff;
    WORD32 prev_data = *prev++;

    win1 = *pwin1--;
    coeff = *pCoef--;
    win2 = *pwin2++;

    accu = ixheaac_sub32_sat(
        ixheaac_shl32_dir_sat_limit(ixheaac_mult32_shl(coeff, win1), q_shift),
        ixheaac_mac32x16in32_shl(rounding_fac, win2, (WORD16)(prev_data)));

    accu = ixheaac_add32_sat(accu, accu);
    accu = ixheaac_add32_sat(accu, accu);

    *ptr_out1 = ixheaac_shr32(accu, 16);
    ptr_out1 -= stride;

    accu = ixheaac_sub32_sat(
        ixheaac_shl32_dir_sat_limit(
            ixheaac_mult32_shl(ixheaac_negate32_sat(coeff), win2), q_shift),
        ixheaac_mac32x16in32_shl(rounding_fac, win1, (WORD16)(prev_data)));

    accu = ixheaac_add32_sat(accu, accu);
    accu = ixheaac_add32_sat(accu, accu);

    *ptr_out2 = ixheaac_shr32(accu, 16);
    ptr_out2 += stride;
  }
}

VOID ixheaacd_over_lap_add1_dec(WORD32 *coef, WORD32 *prev, WORD32 *out,
                                const WORD16 *window, WORD16 q_shift,
                                WORD16 size, WORD16 ch_fac) {
  WORD32 accu;
  WORD32 i;
  WORD16 rounding_fac = 0;

  for (i = 0; i < size; i++) {
    WORD16 window1, window2;

    window1 = window[2 * size - 2 * i - 1];
    window2 = window[2 * size - 2 * i - 2];
    accu = ixheaac_sub32_sat(
        ixheaac_shl32_dir_sat_limit(
            ixheaac_mult32x16in32(coef[size * 2 - 1 - i], window2), q_shift),
        ixheaacd_mac32x16in32_sat(rounding_fac, prev[i], window1));
    out[ch_fac * (size - i - 1)] = accu;
    accu = ixheaac_sub32_sat(
        ixheaac_shl32_dir_sat_limit(
            ixheaac_mult32x16in32(ixheaac_negate32_sat(coef[size * 2 - 1 - i]),
                                   window1),
            q_shift),
        ixheaacd_mac32x16in32_sat(rounding_fac, prev[i], window2));
    out[ch_fac * (size + i)] = accu;
  }
}

VOID ixheaacd_over_lap_add2_dec(WORD32 *coef, WORD32 *prev, WORD32 *out,
                                const WORD16 *window, WORD16 q_shift,
                                WORD16 size, WORD16 ch_fac) {
  WORD32 accu;
  WORD32 i;

  for (i = 0; i < size; i++) {
    accu = ixheaac_sub32_sat(
        ixheaac_mult32x16in32(coef[size + i], window[2 * i]),
        ixheaac_mult32x16in32(prev[size - 1 - i], window[2 * i + 1]));
    out[ch_fac * i] = ixheaac_shr32_sat(accu, 16 - (q_shift + 1));
  }

  for (i = 0; i < size; i++) {
    accu = ixheaac_sub32_sat(
        ixheaac_mult32x16in32(ixheaac_negate32_sat(coef[size * 2 - 1 - i]),
                               window[2 * size - 2 * i - 1]),
        ixheaac_mult32x16in32(prev[i], window[2 * size - 2 * i - 2]));
    out[ch_fac * (i + size)] = ixheaac_shr32_sat(accu, 16 - (q_shift + 1));
  }
}

VOID ixheaacd_process_single_scf(WORD32 scale_factor, WORD32 *x_invquant,
                                 WORD32 width, WORD32 *ptr_scale_table,
                                 WORD32 total_channels, WORD32 object_type,
                                 WORD32 aac_sf_data_resil_flag) {
  WORD32 j;

  WORD32 temp1;
  WORD32 q_factor;
  WORD32 buffer1;
  WORD16 scale_short;

  object_type = 0;
  aac_sf_data_resil_flag = 0;

  if (scale_factor < 24) {
    for (j = width; j > 0; j--) {
      *x_invquant++ = 0;
    }
  } else {
    WORD32 shift;

    if (total_channels > 2)
      q_factor = 34 - (scale_factor >> 2);
    else
      q_factor = 37 - (scale_factor >> 2);

    scale_short = ptr_scale_table[(scale_factor & 0x0003)];
    shift = q_factor;
    if (shift > 0) {
      if (scale_short == (WORD16)0x8000) {
        for (j = width; j > 0; j--) {
          temp1 = *x_invquant;
          buffer1 = ixheaac_mult32x16in32_shl_sat(temp1, scale_short);
          buffer1 = ixheaac_shr32(buffer1, shift);
          *x_invquant++ = buffer1;
        }
      } else {
        for (j = width; j > 0; j--) {
          temp1 = *x_invquant;
          buffer1 = ixheaac_mult32x16in32_shl(temp1, scale_short);
          buffer1 = ixheaac_shr32(buffer1, shift);
          *x_invquant++ = buffer1;
        }
      }
    } else {
      shift = -shift;
      if (shift > 0) {
        if (scale_short == (WORD16)0x8000) {
          for (j = width; j > 0; j--) {
            temp1 = *x_invquant;
            temp1 = ixheaac_shl32(temp1, shift - 1);
            buffer1 = ixheaac_mult32x16in32_shl_sat(temp1, scale_short);
            buffer1 = ixheaac_shl32(buffer1, 1);
            *x_invquant++ = buffer1;
          }
        } else {
          for (j = width; j > 0; j--) {
            temp1 = *x_invquant;
            temp1 = ixheaac_shl32(temp1, shift - 1);
            buffer1 = ixheaac_mult32x16in32_shl(temp1, scale_short);
            buffer1 = ixheaac_shl32(buffer1, 1);
            *x_invquant++ = buffer1;
          }
        }

      } else {
        if (scale_short == (WORD16)0x8000) {
          for (j = width; j > 0; j--) {
            temp1 = *x_invquant;
            buffer1 = ixheaac_mult32x16in32_shl_sat(temp1, scale_short);
            *x_invquant++ = buffer1;
          }
        } else {
          for (j = width; j > 0; j--) {
            temp1 = *x_invquant;
            buffer1 = ixheaac_mult32x16in32_shl(temp1, scale_short);
            *x_invquant++ = buffer1;
          }
        }
      }
    }
  }
}

VOID ixheaacd_scale_factor_process_dec(WORD32 *x_invquant, WORD16 *scale_fact,
                                       WORD no_band, WORD8 *width,
                                       WORD32 *ptr_scale_table,
                                       WORD32 total_channels,
                                       WORD32 object_type,
                                       WORD32 aac_sf_data_resil_flag) {
  WORD32 i;
  WORD16 scale_factor;

  for (i = no_band - 1; i >= 0; i--) {
    scale_factor = *scale_fact++;
    ixheaacd_process_single_scf(scale_factor, x_invquant, *width,
                                ptr_scale_table, total_channels, object_type,
                                aac_sf_data_resil_flag);

    x_invquant += *width;
    width++;
  }
}

VOID ixheaacd_right_shift_block(WORD32 *p_spectrum, WORD32 length,
                                WORD32 shift_val) {
  WORD32 i;
  WORD32 temp1, temp2;
  WORD32 *temp_ptr = &p_spectrum[0];
  length = length >> 2;

  for (i = length - 1; i >= 0; i--) {
    temp1 = *temp_ptr;
    temp2 = *(temp_ptr + 1);
    *temp_ptr++ = temp1 >> shift_val;
    temp1 = *(temp_ptr + 1);
    *temp_ptr++ = temp2 >> shift_val;
    temp2 = *(temp_ptr + 1);
    *temp_ptr++ = temp1 >> shift_val;
    *temp_ptr++ = temp2 >> shift_val;
  }
}
