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
#include "ixheaac_type_def.h"
#include "ixheaac_constants.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaac_basic_ops.h"
#include "ixheaacd_bitbuffer.h"
#include "ixheaac_basic_op.h"
#include "ixheaacd_mps_aac_struct.h"
#include "ixheaacd_mps_res_rom.h"
#include "ixheaacd_mps_res_block.h"
#include "ixheaacd_mps_res_huffman.h"

static PLATFORM_INLINE WORD32 ixheaacd_res_extract_symbol(WORD32 value, WORD32 l_shift,
                                                          WORD32 r_shift, WORD32 *pow_table_q17) {
  WORD32 out;
  out = (WORD16)((value << l_shift) >> r_shift);

  if (out < 0) {
    out = -out;
    out = pow_table_q17[out];
    out = -out;
  } else
    out = pow_table_q17[out];

  return out;
}

static PLATFORM_INLINE WORD32 ixheaacd_res_extract_signed_symbol(WORD32 value, WORD32 l_shift,
                                                                 WORD32 r_shift,
                                                                 WORD32 *pow_table_q17,
                                                                 WORD32 *temp_word,
                                                                 WORD32 *pr_bit_pos) {
  WORD32 out;
  out = ixheaac_extu(value, l_shift, r_shift);
  if (out) {
    WORD32 bit_pos = *pr_bit_pos;
    out = pow_table_q17[out];
    if (*temp_word & 0x80000000) {
      out = -out;
    }
    *temp_word = *temp_word << 1;
    bit_pos++;
    *pr_bit_pos = bit_pos;
  }
  return out;
}

VOID ixheaacd_res_inverse_quant_lb(WORD32 *x_invquant, WORD t_bands, WORD32 *pow_table_q17,
                                   WORD8 *pulse_data) {
  WORD32 j;
  WORD32 temp;
  WORD32 q_abs;

  for (j = t_bands - 1; j >= 0; j--) {
    q_abs = *pulse_data++;
    temp = (pow_table_q17[q_abs]);
    *x_invquant++ = -temp;
  }
}

static PLATFORM_INLINE WORD ixheaacd_res_c_block_decode_huff_word1(
    ia_bit_buf_struct *it_bit_buf, WORD32 *qp, WORD16 *offsets, WORD no_bands, WORD group_no,
    const UWORD16 *h_ori, WORD32 *pow_table_q17, WORD32 maximum_bins_short) {
  WORD32 sp1, sp2;
  WORD32 flush_cw;
  WORD32 i, value, norm_val, off;
  WORD idx, grp_idx;
  WORD32 out1, out2;
  WORD32 err_code = 0;
  WORD len_idx = 0;
  UWORD8 *ptr_read_next = it_bit_buf->ptr_read_next;
  WORD32 bit_pos = it_bit_buf->bit_pos;
  WORD32 read_word = ixheaacd_res_aac_showbits_32(ptr_read_next);
  ptr_read_next += 4;

  do {
    len_idx = offsets[1] - offsets[0];
    grp_idx = group_no;
    do {
      qp = qp + offsets[0];
      idx = len_idx;
      do {
        {
          UWORD16 first_offset;
          WORD16 sign_ret_val;
          UWORD32 read_word1;
          UWORD16 *h;

          read_word1 = read_word << bit_pos;

          h = (UWORD16 *)h_ori;
          h += (read_word1) >> (27);
          sign_ret_val = *h;

          first_offset = 5;
          while (sign_ret_val > 0) {
            bit_pos += first_offset;
            ixheaacd_aac_read_byte_corr(&ptr_read_next, &bit_pos, &read_word,
                                        it_bit_buf->ptr_bit_buf_end);
            read_word1 = (read_word1) << (first_offset);
            first_offset = (sign_ret_val >> 11);
            h += sign_ret_val & (0x07FF);
            h += (read_word1) >> (32 - first_offset);
            sign_ret_val = *h;
          }
          bit_pos += ((sign_ret_val & 0x7fff) >> 11);
          bit_pos = min(bit_pos, 31);
          value = sign_ret_val & (0x07FF);
        }
        out1 = (value & 0x3E0) >> 5;
        out2 = value & 0x1F;

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
                                    it_bit_buf->ptr_bit_buf_end);
        if (sp1 == 16) {
          i = 4;
          value = ixheaac_extu(read_word, bit_pos, 23);
          value = value | 0xfffffe00;
          norm_val = ixheaac_norm32(value);

          i += (norm_val - 22);
          bit_pos += (norm_val - 21);

          ixheaacd_aac_read_byte_corr(&ptr_read_next, &bit_pos, &read_word,
                                      it_bit_buf->ptr_bit_buf_end);

          off = ixheaac_extu(read_word, bit_pos, 32 - i);

          bit_pos += i;

          ixheaacd_aac_read_byte_corr(&ptr_read_next, &bit_pos, &read_word,
                                      it_bit_buf->ptr_bit_buf_end);

          ixheaacd_aac_read_byte_corr(&ptr_read_next, &bit_pos, &read_word,
                                      it_bit_buf->ptr_bit_buf_end);

          i = off + ((WORD32)1 << i);

          if (i <= IQ_TABLE_SIZE_HALF)
            i = pow_table_q17[i];
          else {
            err_code |= ixheaacd_res_inv_quant(&i, pow_table_q17);
          }

          if (out1 < 0) {
            out1 = -i;
          } else {
            out1 = i;
          }
          *qp++ = out1;
        } else {
          if (out1 <= 0) {
            out1 = -out1;
            out1 = pow_table_q17[out1];
            *qp++ = -out1;
          } else {
            out1 = pow_table_q17[out1];
            *qp++ = out1;
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
                                      it_bit_buf->ptr_bit_buf_end);

          off = ixheaac_extu(read_word, bit_pos, 32 - i);

          bit_pos += i;

          ixheaacd_aac_read_byte_corr(&ptr_read_next, &bit_pos, &read_word,
                                      it_bit_buf->ptr_bit_buf_end);

          ixheaacd_aac_read_byte_corr(&ptr_read_next, &bit_pos, &read_word,
                                      it_bit_buf->ptr_bit_buf_end);

          i = off + ((WORD32)1 << i);

          if (i <= IQ_TABLE_SIZE_HALF)
            i = pow_table_q17[i];
          else {
            err_code |= ixheaacd_res_inv_quant(&i, pow_table_q17);
          }

          if (out2 < 0) {
            out2 = -i;
          } else {
            out2 = i;
          }
          *qp++ = out2;
        } else {
          if (out2 <= 0) {
            out2 = -out2;
            out2 = pow_table_q17[out2];
            *qp++ = -out2;
          } else {
            out2 = pow_table_q17[out2];
            *qp++ = out2;
          }
        }

        idx -= 2;
      } while (idx != 0);

      qp += (maximum_bins_short - offsets[1]);
      grp_idx--;
    } while (grp_idx != 0);

    offsets++;
    qp -= (maximum_bins_short * group_no);
    no_bands--;
  } while (no_bands >= 0);

  it_bit_buf->bit_pos = bit_pos;
  it_bit_buf->ptr_read_next = ptr_read_next - 4;

  return err_code;
}

static PLATFORM_INLINE WORD ixheaacd_res_c_block_decode_huff_word1_lb(
    ia_bit_buf_struct *it_bif_buf, WORD32 len, const UWORD16 *h_ori, WORD32 *x_invquant,
    WORD32 *pow_table_q17, WORD8 *p_pul_arr) {
  WORD32 sp1, sp2;
  WORD32 flush_cw;
  WORD32 i, value, norm_val, off;
  WORD idx;
  WORD32 out1, out2;
  WORD32 err_code = 0;
  UWORD8 *ptr_read_next = it_bif_buf->ptr_read_next;
  WORD32 bit_pos = it_bif_buf->bit_pos;
  WORD32 read_word = ixheaacd_res_aac_showbits_32(ptr_read_next);
  ptr_read_next += 4;

  for (idx = len; idx != 0; idx -= 2) {
    {
      UWORD16 first_offset;
      WORD16 sign_ret_val;
      UWORD32 read_word1;
      UWORD16 *h;

      read_word1 = read_word << bit_pos;

      h = (UWORD16 *)h_ori;
      h += (read_word1) >> (27);
      sign_ret_val = *h;

      first_offset = 5;
      while (sign_ret_val > 0) {
        bit_pos += first_offset;
        ixheaacd_aac_read_byte_corr(&ptr_read_next, &bit_pos, &read_word,
                                    it_bif_buf->ptr_bit_buf_end);
        read_word1 = (read_word1) << (first_offset);

        first_offset = (sign_ret_val >> 11);
        h += sign_ret_val & (0x07FF);

        h += (read_word1) >> (32 - first_offset);
        sign_ret_val = *h;
      }
      bit_pos += ((sign_ret_val & 0x7fff) >> 11);
      bit_pos = min(bit_pos, 31);
      value = sign_ret_val & (0x07FF);
    }

    flush_cw = read_word << bit_pos;

    out1 = (value & 0x3E0) >> 5;
    out2 = value & 0x1F;

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
                                it_bif_buf->ptr_bit_buf_end);

    if (sp1 == 16) {
      i = 4;
      value = ixheaac_extu(read_word, bit_pos, 23);
      value = value | 0xfffffe00;
      norm_val = ixheaac_norm32(value);
      i += (norm_val - 22);
      bit_pos += (norm_val - 21);

      ixheaacd_aac_read_byte_corr(&ptr_read_next, &bit_pos, &read_word,
                                  it_bif_buf->ptr_bit_buf_end);

      off = ixheaac_extu(read_word, bit_pos, 32 - i);

      bit_pos += i;

      ixheaacd_aac_read_byte_corr(&ptr_read_next, &bit_pos, &read_word,
                                  it_bif_buf->ptr_bit_buf_end);
      value = *p_pul_arr++;
      ixheaacd_aac_read_byte_corr(&ptr_read_next, &bit_pos, &read_word,
                                  it_bif_buf->ptr_bit_buf_end);
      i = off + ((WORD32)1 << i);
      i = add_d(i, value);

      if (i <= IQ_TABLE_SIZE_HALF)
        i = pow_table_q17[i];
      else {
        err_code |= ixheaacd_res_inv_quant(&i, pow_table_q17);
      }
      if (out1 < 0) {
        i = -i;
      }
      *x_invquant++ = i;
    } else {
      WORD8 temp = *p_pul_arr++;
      if (out1 <= 0) {
        out1 = sub_d(temp, out1);
        out1 = pow_table_q17[out1];
        *x_invquant++ = -out1;
      } else {
        out1 = add_d(out1, temp);
        out1 = pow_table_q17[out1];
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
                                  it_bif_buf->ptr_bit_buf_end);

      off = ixheaac_extu(read_word, bit_pos, 32 - i);

      bit_pos += i;

      ixheaacd_aac_read_byte_corr(&ptr_read_next, &bit_pos, &read_word,
                                  it_bif_buf->ptr_bit_buf_end);
      value = *p_pul_arr++;
      ixheaacd_aac_read_byte_corr(&ptr_read_next, &bit_pos, &read_word,
                                  it_bif_buf->ptr_bit_buf_end);

      i = off + ((WORD32)1 << i);
      i = add_d(i, value);
      if (i <= IQ_TABLE_SIZE_HALF)
        i = pow_table_q17[i];
      else {
        err_code |= ixheaacd_res_inv_quant(&i, pow_table_q17);
      }

      if (out2 < 0) {
        i = -i;
      }
      *x_invquant++ = i;
    } else {
      WORD8 temp = *p_pul_arr++;
      if (out2 <= 0) {
        out2 = sub_d(temp, out2);
        out2 = pow_table_q17[out2];
        *x_invquant++ = -out2;
      } else {
        out2 = add_d(out2, temp);
        out2 = pow_table_q17[out2];
        *x_invquant++ = out2;
      }
    }
  }

  it_bif_buf->ptr_read_next = ptr_read_next - 4;
  it_bif_buf->bit_pos = bit_pos;

  return err_code;
}

static PLATFORM_INLINE WORD ixheaacd_res_c_block_decode_huff_word2_4(
    ia_bit_buf_struct *it_bit_buf, WORD32 *qp, WORD16 *offsets, WORD no_bands, WORD group_no,
    const UWORD16 *h_ori, WORD32 *pow_table_q17, WORD32 sign, WORD32 maximum_bins_short) {
  WORD32 value;
  WORD idx, grp_idx;
  WORD idx_len;
  WORD32 *qp_org;

  UWORD8 *ptr_read_next = it_bit_buf->ptr_read_next;
  WORD32 bit_pos = it_bit_buf->bit_pos;
  WORD32 read_word = ixheaacd_res_aac_showbits_32(ptr_read_next);
  ptr_read_next += 4;
  qp_org = qp;
  do {
    idx_len = offsets[1] - offsets[0];
    grp_idx = group_no;

    do {
      qp = qp + offsets[0];
      idx = idx_len;
      do {
        UWORD16 first_offset;
        WORD16 sign_ret_val;
        UWORD32 read_word1;
        UWORD16 *h;

        read_word1 = read_word << bit_pos;

        h = (UWORD16 *)h_ori;
        h += (read_word1) >> (27);
        sign_ret_val = *h;

        first_offset = 5;
        while (sign_ret_val > 0) {
          bit_pos += first_offset;
          ixheaacd_aac_read_byte_corr(&ptr_read_next, &bit_pos, &read_word,
                                      it_bit_buf->ptr_bit_buf_end);
          read_word1 = (read_word1) << (first_offset);

          first_offset = (sign_ret_val >> 11);
          h += sign_ret_val & (0x07FF);

          h += (read_word1) >> (32 - first_offset);
          sign_ret_val = *h;
        }
        bit_pos += ((sign_ret_val & 0x7fff) >> 11);
        bit_pos = min(bit_pos, 31);
        value = sign_ret_val & (0x07FF);

        if (sign) {
          WORD32 temp_word;
          temp_word = read_word << bit_pos;

          *qp++ = ixheaacd_res_extract_signed_symbol(value, 24, 30, pow_table_q17, &temp_word,
                                                     &bit_pos);
          *qp++ = ixheaacd_res_extract_signed_symbol(value, 26, 30, pow_table_q17, &temp_word,
                                                     &bit_pos);
          *qp++ = ixheaacd_res_extract_signed_symbol(value, 28, 30, pow_table_q17, &temp_word,
                                                     &bit_pos);
          *qp++ = ixheaacd_res_extract_signed_symbol(value, 30, 30, pow_table_q17, &temp_word,
                                                     &bit_pos);
        } else {
          *qp++ = ixheaacd_res_extract_symbol(value, 24, 30, pow_table_q17);
          *qp++ = ixheaacd_res_extract_symbol(value, 26, 30, pow_table_q17);
          *qp++ = ixheaacd_res_extract_symbol(value, 28, 30, pow_table_q17);
          *qp++ = ixheaacd_res_extract_symbol(value, 30, 30, pow_table_q17);
        }

        ixheaacd_aac_read_byte_corr(&ptr_read_next, &bit_pos, &read_word,
                                    it_bit_buf->ptr_bit_buf_end);
        idx -= 4;
      } while (idx != 0);

      qp += (maximum_bins_short - offsets[1]);
      grp_idx--;
    } while (grp_idx != 0);
    offsets++;
    qp = qp_org;
    no_bands--;
  } while (no_bands >= 0);

  it_bit_buf->ptr_read_next = ptr_read_next - 4;
  it_bit_buf->bit_pos = bit_pos;

  return 0;
}

static PLATFORM_INLINE WORD ixheaacd_res_c_block_decode_huff_word2_4_lb(
    ia_bit_buf_struct *it_bit_buf, WORD32 len, const UWORD16 *h_ori, WORD32 *x_invquant,
    WORD32 *pow_table_q17, WORD8 *p_pul_arr, WORD32 sign) {
  WORD32 value;
  WORD idx;

  UWORD8 *ptr_read_next = it_bit_buf->ptr_read_next;
  WORD32 bit_pos = it_bit_buf->bit_pos;
  WORD32 read_word = ixheaacd_res_aac_showbits_32(ptr_read_next);
  ptr_read_next += 4;

  for (idx = len; idx != 0; idx -= 4) {
    WORD32 res;
    WORD32 ampres, ampres1;
    WORD32 ampres2, ampres3;
    UWORD16 first_offset;
    WORD16 sign_ret_val;
    UWORD32 read_word1;
    UWORD16 *h;

    read_word1 = read_word << bit_pos;

    h = (UWORD16 *)h_ori;
    h += (read_word1) >> (27);
    sign_ret_val = *h;

    first_offset = 5;
    while (sign_ret_val > 0) {
      bit_pos += first_offset;
      ixheaacd_aac_read_byte_corr(&ptr_read_next, &bit_pos, &read_word,
                                  it_bit_buf->ptr_bit_buf_end);
      read_word1 = (read_word1) << (first_offset);

      first_offset = (sign_ret_val >> 11);
      h += sign_ret_val & (0x07FF);

      h += (read_word1) >> (32 - first_offset);
      sign_ret_val = *h;
    }
    bit_pos += ((sign_ret_val & 0x7fff) >> 11);
    bit_pos = min(bit_pos, 31);

    value = sign_ret_val & (0x07FF);

    if (sign) {
      WORD32 out0, out1, out2, out3;
      WORD32 ampout0, ampout1, ampout2, ampout3;
      WORD32 temp_word;
      temp_word = read_word << bit_pos;

      out0 = (ixheaac_extu(value, 24, 30));
      ampout0 = add_d(out0, *p_pul_arr++);
      ampout0 = pow_table_q17[ampout0];

      if (out0) {
        if (temp_word & 0x80000000) {
          ampout0 = -ampout0;
        }
        temp_word = temp_word << 1;
        bit_pos++;
      } else {
        ampout0 = -ampout0;
      }

      out1 = (ixheaac_extu(value, 26, 30));
      ampout1 = add_d(out1, *p_pul_arr++);
      ampout1 = pow_table_q17[ampout1];
      if (out1) {
        if (temp_word & 0x80000000) {
          ampout1 = -(ampout1);
        }
        temp_word = temp_word << 1;
        bit_pos++;
      } else {
        ampout1 = -ampout1;
      }
      out2 = (ixheaac_extu(value, 28, 30));
      ampout2 = add_d(out2, *p_pul_arr++);
      ampout2 = pow_table_q17[ampout2];
      if (out2) {
        if (temp_word & 0x80000000) {
          ampout2 = -(ampout2);
        }
        temp_word = temp_word << 1;
        bit_pos++;
      } else {
        ampout2 = -ampout2;
      }

      *x_invquant++ = ampout0;
      *x_invquant++ = ampout1;
      *x_invquant++ = ampout2;

      out3 = (ixheaac_extu(value, 30, 30));
      ampout3 = add_d(out3, *p_pul_arr++);
      ampout3 = pow_table_q17[ampout3];
      if (out3) {
        if (temp_word & 0x80000000) {
          ampout3 = -(ampout3);
        }
        temp_word = temp_word << 1;
        bit_pos++;
      } else {
        ampout3 = -ampout3;
      }

      *x_invquant++ = ampout3;
    } else {
      ampres = *p_pul_arr++;
      res = (ixheaacd_res_exts(value, 24, 30));
      if (res > 0) {
        ampres = add_d(res, ampres);
        ampres = pow_table_q17[ampres];
      } else {
        ampres = sub_d(ampres, res);
        ampres = pow_table_q17[ampres];
        ampres = -ampres;
      }
      res = (ixheaacd_res_exts(value, 26, 30));
      ampres1 = *p_pul_arr++;
      if (res > 0) {
        ampres1 = add_d(res, ampres1);
        ampres1 = pow_table_q17[ampres1];
      } else {
        ampres1 = sub_d(ampres1, res);
        ampres1 = pow_table_q17[ampres1];
        ampres1 = -ampres1;
      }
      res = (ixheaacd_res_exts(value, 28, 30));
      ampres2 = *p_pul_arr++;
      if (res > 0) {
        ampres2 = add_d(res, ampres2);
        ampres2 = pow_table_q17[ampres2];
      } else {
        ampres2 = sub_d(ampres2, res);
        ampres2 = pow_table_q17[ampres2];
        ampres2 = -ampres2;
      }
      res = (ixheaacd_res_exts(value, 30, 30));
      ampres3 = *p_pul_arr++;
      if (res > 0) {
        ampres3 = add_d(res, ampres3);
        ampres3 = pow_table_q17[ampres3];
      } else {
        ampres3 = sub_d(ampres3, res);
        ampres3 = pow_table_q17[ampres3];
        ampres3 = -ampres3;
      }
      *x_invquant++ = ampres;
      *x_invquant++ = ampres1;
      *x_invquant++ = ampres2;
      *x_invquant++ = ampres3;
    }
    ixheaacd_aac_read_byte_corr(&ptr_read_next, &bit_pos, &read_word,
                                it_bit_buf->ptr_bit_buf_end);
  }

  it_bit_buf->ptr_read_next = ptr_read_next - 4;
  it_bit_buf->bit_pos = bit_pos;

  return 0;
}

static PLATFORM_INLINE WORD ixheaacd_res_c_block_decode_huff_word2_2(
    ia_bit_buf_struct *it_bif_buf, WORD32 *qp, WORD16 *offsets, WORD no_bands, WORD group_no,
    const UWORD16 *h_ori, WORD32 *pow_table_q17, WORD32 sign, WORD32 maximum_bins_short)

{
  WORD32 value;
  WORD idx, grp_idx;
  WORD len_idx;

  WORD32 *qp_org = qp;

  UWORD8 *ptr_read_next = it_bif_buf->ptr_read_next;
  WORD32 bit_pos = it_bif_buf->bit_pos;
  WORD32 read_word = ixheaacd_res_aac_showbits_32(ptr_read_next);
  ptr_read_next += 4;

  do {
    len_idx = offsets[1] - offsets[0];
    grp_idx = group_no;
    do {
      qp += offsets[0];
      idx = len_idx;
      do {
        UWORD16 first_offset;
        WORD16 sign_ret_val;
        UWORD32 read_word1;
        UWORD16 *h;

        read_word1 = read_word << bit_pos;

        h = (UWORD16 *)h_ori;
        h += (read_word1) >> (27);
        sign_ret_val = *h;

        first_offset = 5;
        while (sign_ret_val > 0) {
          bit_pos += first_offset;
          ixheaacd_aac_read_byte_corr(&ptr_read_next, &bit_pos, &read_word,
                                      it_bif_buf->ptr_bit_buf_end);
          read_word1 = (read_word1) << (first_offset);

          first_offset = (sign_ret_val >> 11);
          h += sign_ret_val & (0x07FF);

          h += (read_word1) >> (32 - first_offset);
          sign_ret_val = *h;
        }
        bit_pos += ((sign_ret_val & 0x7fff) >> 11);
        bit_pos = min(bit_pos, 31);
        value = sign_ret_val & (0x07FF);

        if (sign) {
          WORD32 temp_word;
          temp_word = read_word << bit_pos;

          *qp++ = ixheaacd_res_extract_signed_symbol(value, 24, 28, pow_table_q17, &temp_word,
                                                     &bit_pos);
          *qp++ = ixheaacd_res_extract_signed_symbol(value, 28, 28, pow_table_q17, &temp_word,
                                                     &bit_pos);
        } else {
          *qp++ = ixheaacd_res_extract_symbol(value, 24, 28, pow_table_q17);
          *qp++ = ixheaacd_res_extract_symbol(value, 28, 28, pow_table_q17);
        }

        ixheaacd_aac_read_byte_corr(&ptr_read_next, &bit_pos, &read_word,
                                    it_bif_buf->ptr_bit_buf_end);
        idx -= 2;
      } while (idx != 0);

      qp += (maximum_bins_short - offsets[1]);
      grp_idx--;
    } while (grp_idx != 0);

    offsets++;
    qp = qp_org;
    no_bands--;
  } while (no_bands >= 0);

  it_bif_buf->ptr_read_next = ptr_read_next - 4;
  it_bif_buf->bit_pos = bit_pos;

  return 0;
}

static PLATFORM_INLINE WORD ixheaacd_res_c_block_decode_huff_word2_2_lb(
    ia_bit_buf_struct *it_bit_buf, WORD32 len, const UWORD16 *h_ori, WORD32 *x_invquant,
    WORD32 *pow_table_q17, WORD8 *p_pul_arr, WORD32 sign) {
  WORD32 value, res, ampres;
  WORD idx;

  UWORD8 *ptr_read_next = it_bit_buf->ptr_read_next;
  WORD32 bit_pos = it_bit_buf->bit_pos;
  WORD32 read_word = ixheaacd_res_aac_showbits_32(ptr_read_next);
  ptr_read_next += 4;

  for (idx = len; idx != 0; idx -= 2) {
    {
      UWORD16 first_offset;
      WORD16 sign_ret_val;
      UWORD32 read_word1;
      UWORD16 *h;

      read_word1 = read_word << bit_pos;

      h = (UWORD16 *)h_ori;
      h += (read_word1) >> (27);
      sign_ret_val = *h;

      first_offset = 5;
      while (sign_ret_val > 0) {
        bit_pos += first_offset;
        ixheaacd_aac_read_byte_corr(&ptr_read_next, &bit_pos, &read_word,
                                    it_bit_buf->ptr_bit_buf_end);
        read_word1 = (read_word1) << (first_offset);

        first_offset = (sign_ret_val >> 11);
        h += sign_ret_val & (0x07FF);

        h += (read_word1) >> (32 - first_offset);
        sign_ret_val = *h;
      }
      bit_pos += ((sign_ret_val & 0x7fff) >> 11);
      bit_pos = min(bit_pos, 31);

      value = sign_ret_val & (0x07FF);
    }

    if (sign) {
      WORD32 out0, out1, temp_word;
      WORD32 ampout0, ampout1;

      ampout0 = *p_pul_arr++;
      ampout1 = *p_pul_arr++;

      out0 = value & 0xf0;

      ampout0 = add_d(ampout0, (UWORD32)out0 >> 4);
      ampout0 = pow_table_q17[ampout0];

      out1 = value & 0xf;
      ampout1 = add_d(out1, ampout1);
      ampout1 = pow_table_q17[ampout1];

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
                                  it_bit_buf->ptr_bit_buf_end);
      *x_invquant++ = ampout0;
      *x_invquant++ = ampout1;
    } else {
      res = ((value << 24) >> 28);
      ampres = *p_pul_arr++;
      if (res > 0) {
        ampres = add_d(res, ampres);
        *x_invquant++ = pow_table_q17[ampres];
      } else {
        ampres = sub_d(ampres, res);
        ampres = pow_table_q17[ampres];
        *x_invquant++ = -ampres;
      }

      res = ((value << 28) >> 28);
      value = *p_pul_arr++;
      if (res > 0) {
        ampres = add_d(res, value);
        *x_invquant++ = pow_table_q17[ampres];
      } else {
        ampres = sub_d(value, res);
        ampres = pow_table_q17[ampres];
        *x_invquant++ = -ampres;
      }
    }
    ixheaacd_aac_read_byte_corr(&ptr_read_next, &bit_pos, &read_word,
                                it_bit_buf->ptr_bit_buf_end);
  }
  it_bit_buf->ptr_read_next = ptr_read_next - 4;
  it_bit_buf->bit_pos = bit_pos;

  return 0;
}

WORD ixheaacd_res_c_block_decode_huff_word_all(
    ia_bit_buf_struct *it_bit_buf, WORD32 code_no, WORD32 *quantized_coef, WORD16 *band_offsets,
    WORD start, WORD band, WORD group_no, ia_mps_dec_residual_aac_tables_struct *aac_tables_ptr,
    WORD32 maximum_bins_short) {
  WORD ret_val = 0;
  WORD start_bit_pos = it_bit_buf->bit_pos;
  UWORD8 *start_read_pos = it_bit_buf->ptr_read_next;
  const UWORD16 *h_ori = (UWORD16 *)(aac_tables_ptr->code_book[code_no]);
  WORD32 *pow_table = (WORD32 *)aac_tables_ptr->res_block_tables_ptr->pow_table_q17;
  WORD32 no_bands = band - start - 1;
  WORD16 *p_band_off = band_offsets + start;

  if (code_no == 11) {
    const UWORD16 *h_ori = aac_tables_ptr->res_huffmann_tables_ptr->huffman_codebook_11;
    ret_val =
        ixheaacd_res_c_block_decode_huff_word1(it_bit_buf, quantized_coef, p_band_off, no_bands,
                                               group_no, h_ori, pow_table, maximum_bins_short);
  } else if (code_no <= 4) {
    WORD32 sign = 0;

    if (code_no > 2) sign = 1;
    ret_val = ixheaacd_res_c_block_decode_huff_word2_4(it_bit_buf, quantized_coef, p_band_off,
                                                       no_bands, group_no, h_ori, pow_table, sign,
                                                       maximum_bins_short);
  }

  else if (code_no <= 10) {
    WORD32 sign = 0;

    if (code_no > 6) sign = 1;
    ret_val = ixheaacd_res_c_block_decode_huff_word2_2(it_bit_buf, quantized_coef, p_band_off,
                                                       no_bands, group_no, h_ori, pow_table, sign,
                                                       maximum_bins_short);
  }
  {
    WORD bits_cons;
    bits_cons = (WORD)(((it_bit_buf->ptr_read_next - start_read_pos) << 3) +
                       (it_bit_buf->bit_pos - start_bit_pos));
    it_bit_buf->cnt_bits -= bits_cons;
  }
  return ret_val;
}

WORD ixheaacd_res_c_block_decode_huff_word_all_lb(
    ia_bit_buf_struct *it_bit_buf, WORD32 code_no, WORD32 len,
    ia_mps_dec_residual_aac_tables_struct *aac_tables_ptr, WORD32 *x_invquant, WORD8 *p_pul_arr) {
  WORD ret_val = 0;
  WORD start_bit_pos = it_bit_buf->bit_pos;
  WORD32 *pow_table = (WORD32 *)aac_tables_ptr->res_block_tables_ptr->pow_table_q17;
  UWORD8 *start_read_pos = it_bit_buf->ptr_read_next;

  const UWORD16 *h_ori = (UWORD16 *)(aac_tables_ptr->code_book[code_no]);

  if (code_no == 11) {
    const UWORD16 *h_ori = aac_tables_ptr->res_huffmann_tables_ptr->huffman_codebook_11;
    ret_val = ixheaacd_res_c_block_decode_huff_word1_lb(it_bit_buf, len, h_ori, x_invquant,
                                                        pow_table, p_pul_arr);
  } else if (code_no <= 4) {
    WORD32 sign = 0;
    if (code_no > 2) sign = 1;
    ret_val = ixheaacd_res_c_block_decode_huff_word2_4_lb(it_bit_buf, len, h_ori, x_invquant,
                                                          pow_table, p_pul_arr, sign);
  } else if (code_no <= 10) {
    WORD32 sign = 0;
    if (code_no > 6) sign = 1;
    ret_val = ixheaacd_res_c_block_decode_huff_word2_2_lb(it_bit_buf, len, h_ori, x_invquant,
                                                          pow_table, p_pul_arr, sign);
  }

  {
    WORD bits_cons;
    if (it_bit_buf->bit_pos <= 7) {
      bits_cons = (WORD)(((it_bit_buf->ptr_read_next - start_read_pos) << 3) +
                         (it_bit_buf->bit_pos - start_bit_pos));
      it_bit_buf->cnt_bits -= bits_cons;
    } else {
      it_bit_buf->ptr_read_next += (it_bit_buf->bit_pos) >> 3;
      it_bit_buf->bit_pos = it_bit_buf->bit_pos & 0x7;

      bits_cons = (WORD)(((it_bit_buf->ptr_read_next - start_read_pos) << 3) +
                         ((it_bit_buf->bit_pos - start_bit_pos)));
      it_bit_buf->cnt_bits -= bits_cons;
    }
  }
  return ret_val;
}

static VOID ixheaacd_res_apply_one_scf(WORD32 scale_factor, WORD32 *x_invquant, WORD32 end,
                                       WORD32 *scale_table_ptr) {
  WORD32 j;

  WORD32 temp_1;
  WORD32 q_factor;
  WORD32 buffer1;
  WORD16 scale_short;

  if (scale_factor < 24) {
    for (j = end; j > 0; j--) {
      *x_invquant++ = 0;
    }
  } else {
    WORD32 shift;
    q_factor = 37 - (scale_factor >> 2);

    scale_short = scale_table_ptr[(scale_factor & 0x0003)];

    shift = q_factor;

    if (shift > 0) {
      if (scale_short == (WORD16)0x8000) {
        for (j = end; j > 0; j--) {
          temp_1 = *x_invquant;

          buffer1 = ixheaac_mult32x16in32_shl_sat(temp_1, scale_short);
          buffer1 = ixheaac_shr32(buffer1, shift);
          *x_invquant++ = buffer1;
        }
      } else {
        for (j = end; j > 0; j--) {
          temp_1 = *x_invquant;

          buffer1 = ixheaac_mult32x16in32_shl(temp_1, scale_short);

          buffer1 = ixheaac_shr32(buffer1, shift);
          *x_invquant++ = buffer1;
        }
      }
    } else {
      shift = -shift;
      if (shift > 0) {
        if (scale_short == (WORD16)0x8000) {
          for (j = end; j > 0; j--) {
            temp_1 = *x_invquant;
            temp_1 = ixheaac_shl32(temp_1, shift - 1);

            buffer1 = ixheaac_mult32x16in32_shl_sat(temp_1, scale_short);

            buffer1 = ixheaac_shl32(buffer1, 1);
            *x_invquant++ = buffer1;
          }
        } else {
          for (j = end; j > 0; j--) {
            temp_1 = *x_invquant;
            temp_1 = ixheaac_shl32(temp_1, shift - 1);

            buffer1 = ixheaac_mult32x16in32_shl(temp_1, scale_short);

            buffer1 = ixheaac_shl32(buffer1, 1);
            *x_invquant++ = buffer1;
          }
        }
      } else {
        if (scale_short == (WORD16)0x8000) {
          for (j = end; j > 0; j--) {
            temp_1 = *x_invquant;

            buffer1 = ixheaac_mult32x16in32_shl_sat(temp_1, scale_short);

            *x_invquant++ = buffer1;
          }
        } else {
          for (j = end; j > 0; j--) {
            temp_1 = *x_invquant;

            buffer1 = ixheaac_mult32x16in32_shl(temp_1, scale_short);

            *x_invquant++ = buffer1;
          }
        }
      }
    }
  }
}

VOID ixheaacd_res_apply_scfs(WORD32 *x_invquant, WORD16 *sc_factor, WORD t_bands, WORD8 *offset,
                             WORD32 *scale_table_ptr) {
  WORD32 i;
  WORD16 scale_factor;

  for (i = t_bands - 1; i >= 0; i--) {
    scale_factor = *sc_factor++;
    ixheaacd_res_apply_one_scf(scale_factor, x_invquant, *offset, scale_table_ptr);
    x_invquant += *offset;
    offset++;
  }
}
