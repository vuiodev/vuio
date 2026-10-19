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
#include "ixheaacd_cnst.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops16.h"
#include "ixheaacd_bitbuffer.h"
#include "ixheaacd_mps_aac_struct.h"
#include "ixheaacd_mps_res_rom.h"
#include "ixheaacd_mps_res.h"
#include "ixheaacd_mps_res_huffman.h"

#define LONG_BLOCK_SECT_LEN 5
#define SHORT_BLOCK_SECT_LEN 3

WORD16 ixheaacd_c_block_read_section_data(
    ia_bit_buf_struct *it_bit_buf,
    ia_mps_dec_residual_channel_info_struct *p_aac_decoder_channel_info) {
  WORD band;
  WORD sect_cb;
  WORD sect_len;
  WORD sect_len_incr;
  WORD sect_esc_val;
  ia_mps_dec_residual_ics_info_struct *p_ics_info = &p_aac_decoder_channel_info->ics_info;
  WORD sfb_transmitted = p_ics_info->max_sf_bands;
  WORD win_group = p_ics_info->window_groups;

  WORD8 *p_code_book = p_aac_decoder_channel_info->p_code_book;
  WORD8 *p_code_book_temp = p_code_book;
  WORD32 sect_bitlen = LONG_BLOCK_SECT_LEN;

  if (p_aac_decoder_channel_info->ics_info.window_sequence == EIGHT_SHORT_SEQUENCE)
    sect_bitlen = SHORT_BLOCK_SECT_LEN;

  sect_esc_val = (1 << sect_bitlen) - 1;

  do {
    band = 0;

    while (band < sfb_transmitted) {
      WORD32 temp_word;
      sect_len = 0;
      temp_word = ixheaacd_read_bits_buf(it_bit_buf, 4 + sect_bitlen);
      sect_cb = temp_word >> sect_bitlen;
      sect_len_incr = temp_word & sect_esc_val;

      while (sect_len_incr == sect_esc_val) {
        sect_len = (sect_len + sect_esc_val);
        sect_len_incr = ixheaacd_read_bits_buf(it_bit_buf, sect_bitlen);
      }

      sect_len = (sect_len + sect_len_incr);

      band = (band + sect_len);
      if (band > sfb_transmitted) {
        return (WORD16)((WORD32)IA_XHEAAC_DEC_EXE_NONFATAL_EXCEEDS_SFB_TRANSMITTED);
      }

      if (sect_cb == BOOKSCL) {
        return (WORD16)((WORD32)AAC_DEC_INVALID_CODE_BOOK);
      }

      sect_len = sect_len - 1;
      for (; sect_len >= 0; sect_len--) {
        *p_code_book_temp++ = sect_cb;
      }
    }
    p_code_book += MAX_SFB_SHORT;
    p_code_book_temp = p_code_book;
    win_group--;
  } while (win_group != 0);
  return AAC_DEC_OK;
}

VOID ixheaacd_res_c_block_read_scf_data(
    ia_bit_buf_struct *it_bit_buf,
    ia_mps_dec_residual_channel_info_struct *p_aac_decoder_channel_info, WORD16 global_gain,
    ia_mps_dec_residual_aac_tables_struct *aac_tables_ptr) {
  WORD band;
  WORD16 position = 0;
  WORD group;
  WORD16 factor = global_gain;
  WORD8 *p_code_book, *p_codebook_tmp;
  WORD16 *p_scale_factor, *p_scale_factor_tmp;
  WORD16 norm_value;
  ia_mps_dec_residual_ics_info_struct *p_ics_info;
  WORD window_groups, sfb_transmitted;
  UWORD16 *h;

  const UWORD16 *hscf = &aac_tables_ptr->res_huffmann_tables_ptr->huffman_code_book_scl[1];

  WORD start_bit_pos = it_bit_buf->bit_pos;
  UWORD8 *start_read_pos = it_bit_buf->ptr_read_next;
  UWORD8 *ptr_read_next = it_bit_buf->ptr_read_next;
  WORD32 bit_pos = 7 - it_bit_buf->bit_pos;

  WORD32 read_word;
  WORD32 diffbytes;

  diffbytes = (WORD32)(it_bit_buf->ptr_bit_buf_end - ptr_read_next);
  diffbytes++;
  if (diffbytes >= 4) {
    read_word = ixheaacd_res_aac_showbits_32(ptr_read_next);
    diffbytes = 4;
    ptr_read_next = it_bit_buf->ptr_read_next + 4;
  } else {
    WORD32 ii;
    read_word = 0;
    for (ii = 0; ii < diffbytes; ii++) {
      read_word = (read_word << 8) | (*ptr_read_next);
      ptr_read_next++;
    }
    read_word <<= ((4 - diffbytes) << 3);
  }
  p_code_book = p_aac_decoder_channel_info->p_code_book;

  p_ics_info = &p_aac_decoder_channel_info->ics_info;
  sfb_transmitted = p_ics_info->max_sf_bands;

  p_scale_factor = p_aac_decoder_channel_info->p_scale_factor;
  window_groups = p_ics_info->window_groups;
  band = sfb_transmitted - 1;

  for (group = 0; group < window_groups; group++) {
    p_codebook_tmp = &p_code_book[group * MAX_SFB_SHORT];
    p_scale_factor_tmp = &p_scale_factor[group * MAX_SFB_SHORT];
    for (band = sfb_transmitted - 1; band >= 0; band--) {
      WORD32 cb_num = *p_codebook_tmp++;

      if (cb_num == ZERO_HCB)
        *p_scale_factor_tmp++ = 0;
      else {
        {
          WORD32 flag = 1;
          WORD pns_band;
          ia_mps_dec_residual_pns_data_struct *p_pns_data = &p_aac_decoder_channel_info->pns_data;
          if (cb_num == NOISE_HCB && (p_pns_data->pns_active != 1)) flag = 0;

          if (flag) {
            UWORD16 first_offset;
            WORD16 sign_ret_val;
            UWORD32 read_word1;

            read_word1 = read_word << bit_pos;
            h = (UWORD16 *)(hscf);
            first_offset = 7;
            h += (read_word1) >> (32 - first_offset);
            sign_ret_val = *h;

            while (sign_ret_val > 0) {
              bit_pos += first_offset;

              ixheaacd_aac_read_byte_corr(&ptr_read_next, &bit_pos, &read_word,
                                          it_bit_buf->ptr_bit_buf_end);
              read_word1 = (read_word1) << (first_offset);

              first_offset = (sign_ret_val >> 11);
              first_offset = (sign_ret_val >> 11);
              h += sign_ret_val & (0x07FF);
              h += (read_word1) >> (32 - first_offset);
              sign_ret_val = *h;
            }

            bit_pos += ((sign_ret_val & 0x7fff) >> 11);

            ixheaacd_aac_read_byte_corr(&ptr_read_next, &bit_pos, &read_word,
                                        it_bit_buf->ptr_bit_buf_end);
            norm_value = (sign_ret_val & (0x07FF)) - 60;
          } else {
            WORD32 noise_start_value;
            UWORD32 temp;
            temp = (read_word << bit_pos);
            temp = ((UWORD32)temp >> (32 - 9));
            noise_start_value = temp;
            bit_pos += 9;

            ixheaacd_aac_read_2bytes(&ptr_read_next, &bit_pos, &read_word);

            norm_value = noise_start_value - 256;
            p_pns_data->pns_active = 1;

            p_pns_data->current_energy = global_gain - NOISE_OFFSET;
          }

          if (cb_num > NOISE_HCB) {
            position = position + norm_value;
            *p_scale_factor_tmp++ = -position;
          } else if (cb_num < NOISE_HCB) {
            factor = factor + norm_value;
            *p_scale_factor_tmp++ = factor;
          } else {
            p_pns_data->current_energy =
                ixheaac_add16_sat(p_pns_data->current_energy, norm_value);

            pns_band = (group << 4) + sfb_transmitted - band - 1;
            p_aac_decoder_channel_info->p_scale_factor[pns_band] = p_pns_data->current_energy;

            p_pns_data->pns_used[pns_band] = 1;
            p_scale_factor_tmp++;
          }
        }
      }
    }
  }

  it_bit_buf->ptr_read_next = ptr_read_next - diffbytes;

  it_bit_buf->bit_pos = 7 - bit_pos;
  {
    WORD bits_cons;
    bits_cons = (WORD)(((it_bit_buf->ptr_read_next - start_read_pos) << 3) +
                       (start_bit_pos - it_bit_buf->bit_pos));
    it_bit_buf->cnt_bits -= bits_cons;
  }
}
