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
#include "ixheaacd_sbr_common.h"
#include "ixheaac_type_def.h"
#include "ixheaac_constants.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops16.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaac_basic_ops.h"

#include "ixheaacd_defines.h"
#include "ixheaacd_aac_rom.h"
#include "ixheaacd_aac_imdct.h"
#include "ixheaacd_bitbuffer.h"
#include "ixheaac_basic_op.h"
#include "ixheaacd_intrinsics.h"

#include "ixheaacd_pulsedata.h"

#include "ixheaacd_pns.h"
#include "ixheaacd_common_rom.h"
#include "ixheaacd_drc_data_struct.h"

#include "ixheaacd_lt_predict.h"
#include "ixheaacd_cnst.h"
#include "ixheaacd_ec_defines.h"
#include "ixheaacd_ec_struct_def.h"
#include "ixheaacd_channelinfo.h"
#include "ixheaacd_drc_dec.h"

#include "ixheaacd_block.h"
#include "ixheaacd_channel.h"

#include "ixheaacd_pulsedata.h"
#include "ixheaacd_pns.h"
#include "ixheaacd_sbrdecoder.h"
#include "ixheaacd_error_codes.h"
#include "ixheaacd_audioobjtypes.h"
#include "ixheaacd_latmdemux.h"
#include "ixheaacd_aacdec.h"

#define LONG_BLOCK_SECT_LEN 5
#define SHORT_BLOCK_SECT_LEN 3

IA_ERRORCODE ixheaacd_read_section_data(
    ia_bit_buf_struct *it_bit_buff,
    ia_aac_dec_channel_info_struct *ptr_aac_dec_channel_info,
    WORD32 aac_spect_data_resil_flag, WORD32 aac_sect_data_resil_flag,
    ia_aac_dec_tables_struct *ptr_aac_tables) {
  WORD sfb;
  WORD sect_cb;
  WORD sect_len;
  WORD sect_len_incr;
  WORD sect_esc_val;
  ia_ics_info_struct *ptr_ics_info = &ptr_aac_dec_channel_info->str_ics_info;
  WORD max_sfb = ptr_ics_info->max_sfb;
  WORD num_win_group;

  WORD8 *ptr_code_book = ptr_aac_dec_channel_info->ptr_code_book;
  WORD8 *ptr_code_book_temp = ptr_code_book;
  WORD32 sect_bitlen = LONG_BLOCK_SECT_LEN;
  int num_lines_sec_idx = 0, top;
  short *ptr_num_sect_lines =
      ptr_aac_dec_channel_info->num_line_in_sec4_hcr_arr;
  UWORD8 *ptr_hcr_code_book = ptr_aac_dec_channel_info->cb4_hcr_arr;
  const short *band_offsets = (WORD16 *)ixheaacd_getscalefactorbandoffsets(
      &(ptr_aac_dec_channel_info->str_ics_info), ptr_aac_tables);
  ptr_aac_dec_channel_info->number_sect = 0;

  if (ptr_aac_dec_channel_info->str_ics_info.window_sequence ==
      EIGHT_SHORT_SEQUENCE) {
    sect_bitlen = SHORT_BLOCK_SECT_LEN;
  }

  sect_esc_val = (1 << sect_bitlen) - 1;

  for (num_win_group = 0; num_win_group < ptr_ics_info->num_window_groups;
       num_win_group++) {
    sfb = 0;

    while (sfb < max_sfb) {
      sect_len = 0;
      if (aac_sect_data_resil_flag) {
        sect_cb = ixheaacd_read_bits_buf(it_bit_buff, 5);
      } else {
        sect_cb = ixheaacd_read_bits_buf(it_bit_buff, 4);
      }

      if ((aac_sect_data_resil_flag == 0) ||
          ((sect_cb < 11) || ((sect_cb > 11) && (sect_cb < 16)))) {
        sect_len_incr = ixheaacd_read_bits_buf(it_bit_buff, sect_bitlen);
        while (sect_len_incr == sect_esc_val) {
          sect_len = (sect_len + sect_esc_val);
          sect_len_incr = ixheaacd_read_bits_buf(it_bit_buff, sect_bitlen);
        }
      } else
        sect_len_incr = 1;

      sect_len = (sect_len + sect_len_incr);

      if (aac_spect_data_resil_flag) {
        top = (sfb + sect_len);
        if ((num_lines_sec_idx >= MAX_SFB_HCR) ||
            (top >= MAX_SCALE_FACTOR_BANDS_LONG)) {
          return -1;
        }
        ptr_num_sect_lines[num_lines_sec_idx] =
            band_offsets[top] - band_offsets[sfb];
        num_lines_sec_idx++;
        if (sect_cb == (ESC_HCB + 1)) {
          return IA_XHEAAC_DEC_EXE_NONFATAL_INVALID_CODE_BOOK;
        } else {
          *ptr_hcr_code_book++ = sect_cb;
        }
        ptr_aac_dec_channel_info->number_sect++;
      }

      sfb = (sfb + sect_len);
      if (sfb > max_sfb) {
        return (WORD16)(
            (WORD32)IA_XHEAAC_DEC_EXE_NONFATAL_EXCEEDS_SFB_TRANSMITTED);
      }
      if (sect_cb == (ESC_HCB + 1)) {
        return (WORD16)(
            (WORD32)IA_XHEAAC_DEC_EXE_NONFATAL_INVALID_CODE_BOOK);
      }

      while (sect_len--) {
        *ptr_code_book_temp++ = sect_cb;
      }
    }
    ptr_code_book += MAX_SCALE_FACTOR_BANDS_SHORT;
    ptr_code_book_temp = ptr_code_book;
  }

  return AAC_DEC_OK;
}

VOID ixheaacd_read_scale_factor_data(
    ia_bit_buf_struct *it_bit_buff,
    ia_aac_dec_channel_info_struct *ptr_aac_dec_channel_info,
    ia_aac_dec_tables_struct *ptr_aac_tables, WORD32 object_type) {
  WORD sfb;
  WORD16 position = 0;
  WORD num_win_group;
  WORD16 factor = ptr_aac_dec_channel_info->global_gain;
  WORD8 *ptr_code_book, *ptr_code_book_short;
  WORD16 *ptr_scale_fact, *ptr_scale_fact_short;
  WORD16 norm_value;
  WORD16 index, length;
  const UWORD16 *hcod_sf =
      ptr_aac_tables->pstr_huffmann_tables->huffman_code_book_scl;
  const UWORD32 *table_idx =
      ptr_aac_tables->pstr_huffmann_tables->huffman_code_book_scl_index;

  WORD start_bit_pos = it_bit_buff->bit_pos;
  UWORD8 *start_read_pos = it_bit_buff->ptr_read_next;
  UWORD8 *ptr_read_next = it_bit_buff->ptr_read_next;
  WORD32 bit_pos = 7 - it_bit_buff->bit_pos;
  WORD32 read_word;
  WORD32 increment;
  read_word = ixheaacd_aac_showbits_32(ptr_read_next, it_bit_buff->cnt_bits,
                                       &increment);
  ptr_read_next += increment;

  ptr_code_book = ptr_aac_dec_channel_info->ptr_code_book;

  ptr_scale_fact = ptr_aac_dec_channel_info->ptr_scale_factor;

  for (num_win_group = 0;
       num_win_group < ptr_aac_dec_channel_info->str_ics_info.num_window_groups;
       num_win_group++) {
    ptr_code_book_short =
        &ptr_code_book[num_win_group * MAX_SCALE_FACTOR_BANDS_SHORT];
    ptr_scale_fact_short =
        &ptr_scale_fact[num_win_group * MAX_SCALE_FACTOR_BANDS_SHORT];
    for (sfb = ptr_aac_dec_channel_info->str_ics_info.max_sfb - 1; sfb >= 0;
         sfb--) {
      WORD32 sfb_cb = *ptr_code_book_short++;
      if (sfb_cb == ZERO_HCB)
        *ptr_scale_fact_short++ = 0;
      else {
        {
          WORD32 pns_present = 0;
          WORD pns_band;

          ia_pns_info_struct *ptr_pns_info =
              &ptr_aac_dec_channel_info->str_pns_info;

          if (sfb_cb == NOISE_HCB && (ptr_pns_info->pns_active != 1)) {
            pns_present = 1;
          }

          if (!pns_present) {
            UWORD32 read_word1;

            read_word1 = read_word << bit_pos;

            ixheaacd_huffman_decode(read_word1, &index, &length, hcod_sf,
                                    table_idx);

            bit_pos += length;
            ixheaacd_aac_read_byte_corr(&ptr_read_next, &bit_pos, &read_word,
                                        it_bit_buff->ptr_bit_buf_end);

            ixheaacd_aac_read_byte_corr1(&ptr_read_next, &bit_pos, &read_word,
                                         it_bit_buff->ptr_bit_buf_end);

            norm_value = index - 60;
          }

          else {
            WORD32 noise_start_value;
            UWORD32 temp;

            temp = (read_word << bit_pos);
            temp = ((UWORD32)temp >> (32 - 9));
            noise_start_value = temp;
            bit_pos += 9;

            ixheaacd_aac_read_byte_corr1(&ptr_read_next, &bit_pos, &read_word,
                                         it_bit_buff->ptr_bit_buf_end);

            norm_value = noise_start_value - 256;
            ptr_pns_info->pns_active = 1;

            ptr_pns_info->noise_energy =
                ptr_aac_dec_channel_info->global_gain - NOISE_OFFSET;
          }

          if ((object_type != AOT_ER_AAC_ELD) &&
              (object_type != AOT_ER_AAC_LD) && (object_type != AOT_ER_AAC_LC)) {
            if (sfb_cb > NOISE_HCB) {
              position = position + norm_value;
              *ptr_scale_fact_short++ = -position;
            } else if (sfb_cb < NOISE_HCB) {
              factor = factor + norm_value;
              *ptr_scale_fact_short++ = factor;
            } else {
              ptr_pns_info->noise_energy =
                  ixheaac_add16_sat(ptr_pns_info->noise_energy, norm_value);

              pns_band = (num_win_group << 4) +
                         ptr_aac_dec_channel_info->str_ics_info.max_sfb - sfb -
                         1;
              ptr_aac_dec_channel_info->ptr_scale_factor[pns_band] =
                  ptr_pns_info->noise_energy;

              ptr_pns_info->pns_used[pns_band] = 1;
              ptr_scale_fact_short++;
            }
          } else {
            if ((sfb_cb == INTENSITY_HCB) || (sfb_cb == INTENSITY_HCB2)) {
              position = position + norm_value;
              *ptr_scale_fact_short++ = -position;
            } else if (sfb_cb == NOISE_HCB) {
              ptr_pns_info->noise_energy =
                  ixheaac_add16_sat(ptr_pns_info->noise_energy, norm_value);

              pns_band = (num_win_group << 4) +
                         ptr_aac_dec_channel_info->str_ics_info.max_sfb - sfb -
                         1;
              ptr_aac_dec_channel_info->ptr_scale_factor[pns_band] =
                  ptr_pns_info->noise_energy;

              ptr_pns_info->pns_used[pns_band] = 1;
              ptr_scale_fact_short++;

            } else {
              factor = factor + norm_value;
              *ptr_scale_fact_short++ = factor;
            }
          }
        }
      }
    }
  }

  it_bit_buff->ptr_read_next = ptr_read_next - increment;

  it_bit_buff->bit_pos = 7 - bit_pos;
  {
    WORD bits_consumed;
    bits_consumed =
        (WORD)(((it_bit_buff->ptr_read_next - start_read_pos) << 3) +
               (start_bit_pos - it_bit_buff->bit_pos));
    it_bit_buff->cnt_bits -= bits_consumed;
  }
}
