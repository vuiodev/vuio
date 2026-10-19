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
#include <math.h>
#include <stdio.h>
#include <string.h>
#include "ixheaac_type_def.h"

#include "ixheaacd_bitbuffer.h"

#include "ixheaacd_interface.h"

#include "ixheaacd_tns_usac.h"
#include "ixheaacd_cnst.h"
#include "ixheaacd_error_codes.h"
#include "ixheaacd_acelp_info.h"

#include "ixheaacd_sbrdecsettings.h"
#include "ixheaacd_info.h"
#include "ixheaacd_sbr_common.h"
#include "ixheaacd_drc_data_struct.h"
#include "ixheaacd_drc_dec.h"
#include "ixheaacd_sbrdecoder.h"
#include "ixheaacd_mps_polyphase.h"
#include "ixheaac_sbr_const.h"
#include "ixheaacd_ec_defines.h"
#include "ixheaacd_ec_struct_def.h"
#include "ixheaacd_main.h"
#include "ixheaacd_arith_dec.h"

#include "ixheaacd_bit_extract.h"

#include "ixheaacd_func_def.h"
#include "ixheaacd_interface.h"
#include "ixheaacd_info.h"

#include "ixheaacd_defines.h"

#define MAX_NR_OF_SWB 120

VOID ixheaacd_calc_grp_offset(ia_sfb_info_struct *ptr_sfb_info, pUWORD8 group) {
  WORD32 group_offset;
  WORD32 group_idx;
  WORD32 ixheaacd_drc_offset;
  WORD16 *group_offset_p;
  WORD32 sfb, len;

  group_offset = 0;
  group_idx = 0;
  do {
    ptr_sfb_info->group_len[group_idx] = group[group_idx] - group_offset;
    group_offset = group[group_idx];
    group_idx++;
  } while (group_offset < 8);
  ptr_sfb_info->num_groups = group_idx;
  group_offset_p = ptr_sfb_info->sfb_idx_tbl;
  ixheaacd_drc_offset = 0;
  for (group_idx = 0; group_idx < ptr_sfb_info->num_groups; group_idx++) {
    len = ptr_sfb_info->group_len[group_idx];
    for (sfb = 0; sfb < ptr_sfb_info->sfb_per_sbk; sfb++) {
      ixheaacd_drc_offset += ptr_sfb_info->sfb_width[sfb] * len;
      *group_offset_p++ = ixheaacd_drc_offset;
    }
  }
}

VOID ixheaacd_read_tns_u(ia_sfb_info_struct *ptr_sfb_info,
                         ia_tns_frame_info_struct *pstr_tns_frame_info,
                         ia_bit_buf_struct *it_bit_buff) {
  WORD32 j, k, top, coef_res, resolution, compress;
  WORD32 short_flag, i;
  WORD16 *sp, tmp, s_mask, n_mask;
  ia_tns_filter_struct *tns_filt;
  ia_tns_info_struct *pstr_tns_info;
  static const WORD16 sgn_mask[] = {0x2, 0x4, 0x8};
  static const WORD16 neg_mask[] = {(WORD16)0xfffc, (WORD16)0xfff8,
                                    (WORD16)0xfff0};

  WORD16 n_filt_bits;
  WORD16 start_band_bits;
  WORD16 order_bits;

  short_flag = (!ptr_sfb_info->islong);
  pstr_tns_frame_info->n_subblocks = ptr_sfb_info->max_win_len;

  if (!short_flag) {
    n_filt_bits = 2;
    start_band_bits = 6;
    order_bits = 4;
  } else {
    n_filt_bits = 1;
    start_band_bits = 4;
    order_bits = 3;
  }

  for (i = 0; i < pstr_tns_frame_info->n_subblocks; i++) {
    pstr_tns_info = &pstr_tns_frame_info->str_tns_info[i];
    if (!(pstr_tns_info->n_filt =
              ixheaacd_read_bits_buf(it_bit_buff, n_filt_bits)))
      continue;

    pstr_tns_info->coef_res = coef_res =
        ixheaacd_read_bits_buf(it_bit_buff, 1) + 3;
    top = ptr_sfb_info->sfb_per_sbk;
    tns_filt = &pstr_tns_info->str_filter[0];

    for (j = pstr_tns_info->n_filt; j > 0; j--) {
      tns_filt->stop_band = top;
      top = tns_filt->start_band =
          top - ixheaacd_read_bits_buf(it_bit_buff, start_band_bits);
      tns_filt->order = ixheaacd_read_bits_buf(it_bit_buff, order_bits);

      if (tns_filt->order) {
        tns_filt->direction = ixheaacd_read_bits_buf(it_bit_buff, 1);
        compress = ixheaacd_read_bits_buf(it_bit_buff, 1);
        resolution = coef_res - compress;
        s_mask = sgn_mask[resolution - 2];
        n_mask = neg_mask[resolution - 2];
        sp = tns_filt->coef;

        for (k = tns_filt->order; k > 0; k--) {
          tmp = ixheaacd_read_bits_buf(it_bit_buff, resolution);
          *sp++ = (tmp & s_mask) ? (tmp | n_mask) : tmp;
        }
      }

      tns_filt++;
    }
  }
}
VOID ixheaacd_scale_factor_data(ia_sfb_info_struct *info, WORD32 tot_sfb,
                                WORD32 max_sfb, WORD32 sfb_per_sbk,
                                WORD8 *ptr_code_book) {
  WORD band;
  WORD sect_cb;
  WORD sect_len;
  WORD8 *ptr_codebook = ptr_code_book;
  WORD8 *temp_ptr_codebook = ptr_codebook;
  WORD32 win_group = info->max_win_len;

  memset(ptr_codebook, 0, 128);

  band = 0;
  while (band < tot_sfb || win_group != 0) {
    sect_cb = 11;
    sect_len = max_sfb;

    band = band + sfb_per_sbk;

    sect_len = sect_len - 1;
    for (; sect_len >= 0; sect_len--) {
      *temp_ptr_codebook++ = sect_cb;
    }
    ptr_codebook += 16;
    temp_ptr_codebook = ptr_codebook;
    win_group--;
  }
  return;
}

WORD32
ixheaacd_win_seq_select(WORD32 window_sequence_curr,
                        WORD32 window_sequence_last) {
  WORD32 window_sequence;

  switch (window_sequence_curr) {
    case ONLY_LONG_SEQUENCE:
      window_sequence = ONLY_LONG_SEQUENCE;
      break;

    case LONG_START_SEQUENCE:
      if ((window_sequence_last == LONG_START_SEQUENCE) ||
          (window_sequence_last == EIGHT_SHORT_SEQUENCE) ||
          (window_sequence_last == STOP_START_SEQUENCE)) {
        window_sequence = STOP_START_SEQUENCE;
      } else {
        window_sequence = LONG_START_SEQUENCE;
      }
      break;

    case LONG_STOP_SEQUENCE:
      window_sequence = LONG_STOP_SEQUENCE;
      break;

    case EIGHT_SHORT_SEQUENCE:
      window_sequence = EIGHT_SHORT_SEQUENCE;
      break;

    default:
      return -1;
  }

  return window_sequence;
}

VOID ixheaacd_tns_reset(ia_sfb_info_struct *ptr_sfb_info,
                        ia_tns_frame_info_struct *pstr_tns_frame_info) {
  WORD32 s;

  pstr_tns_frame_info->n_subblocks = ptr_sfb_info->max_win_len;
  for (s = 0; s < pstr_tns_frame_info->n_subblocks; s++)
    pstr_tns_frame_info->str_tns_info[s].n_filt = 0;
}

VOID ixheaacd_section_data(ia_usac_data_struct *usac_data,
                           ia_bit_buf_struct *g_bs, ia_sfb_info_struct *info,
                           WORD16 global_gain, pWORD16 factors, pUWORD8 groups,
                           WORD8 *ptr_code_book) {
  WORD32 band;
  WORD16 position = 0;
  WORD32 group;
  WORD16 factor = global_gain;
  WORD8 *temp_codebook_ptr;
  WORD16 *ptr_scale_fac, *temp_ptr_scale_fac;
  WORD16 norm_val;
  WORD32 window_grps, trans_sfb;
  WORD16 index, length;
  const UWORD16 *hscf = usac_data->huffman_code_book_scl;
  const UWORD32 *idx_tab = usac_data->huffman_code_book_scl_index;

  WORD32 start_bit_pos = g_bs->bit_pos;
  UWORD8 *start_read_pos = g_bs->ptr_read_next;
  UWORD8 *ptr_read_next = g_bs->ptr_read_next;
  WORD32 bit_pos = 7 - g_bs->bit_pos;
  WORD32 is_1_group = 1;

  WORD32 bb = 0, i;
  WORD32 increment;
  WORD32 read_word =
      ixheaacd_aac_showbits_32(ptr_read_next, g_bs->cnt_bits, &increment);
  ptr_read_next = g_bs->ptr_read_next + increment;

  trans_sfb = info->sfb_per_sbk;
  temp_ptr_scale_fac = factors;
  window_grps = info->max_win_len;
  memset(factors, 0, MAXBANDS);
  band = trans_sfb - 1;

  for (group = 0; group < window_grps;) {
    temp_codebook_ptr = &ptr_code_book[group * 16];
    ptr_scale_fac = temp_ptr_scale_fac;
    group = *groups++;
    for (band = trans_sfb - 1; band >= 0; band--) {
      WORD32 cb_num = *temp_codebook_ptr++;

      if ((band == trans_sfb - 1) && (is_1_group == 1)) {
        *temp_ptr_scale_fac = factor;
        temp_ptr_scale_fac++;
        continue;
      }

      if (cb_num == ZERO_HCB)
        *temp_ptr_scale_fac++ = 0;
      else {
        WORD32 pns_band;
        WORD16 curr_energy = 0;

        UWORD32 read_word1;

        read_word1 = read_word << bit_pos;

        ixheaacd_huffman_decode(read_word1, &index, &length, hscf, idx_tab);

        bit_pos += length;
        ixheaacd_aac_read_byte_corr1(&ptr_read_next, &bit_pos, &read_word,
                                     g_bs->ptr_bit_buf_end);
        norm_val = index - 60;

        if (cb_num > NOISE_HCB) {
          position = position + norm_val;
          *temp_ptr_scale_fac++ = -position;

        } else if (cb_num < NOISE_HCB) {
          factor = factor + norm_val;
          *temp_ptr_scale_fac++ = factor;

        } else {
          curr_energy += norm_val;

          pns_band = (group << 4) + trans_sfb - band - 1;

          temp_ptr_scale_fac[pns_band] = curr_energy;

          temp_ptr_scale_fac++;
        }
      }
    }
    is_1_group = 0;

    if (!(info->islong)) {
      for (bb++; bb < group; bb++) {
        for (i = 0; i < trans_sfb; i++) {
          ptr_scale_fac[i + trans_sfb] = ptr_scale_fac[i];
        }
        temp_ptr_scale_fac += trans_sfb;
        ptr_scale_fac += trans_sfb;
      }
    }
  }
  ptr_read_next = ptr_read_next - increment;
  ixheaacd_aac_read_byte_corr1(&ptr_read_next, &bit_pos, &read_word,
                               g_bs->ptr_bit_buf_end);

  g_bs->ptr_read_next = ptr_read_next;

  g_bs->bit_pos = 7 - bit_pos;
  {
    WORD32 bits_consumed;
    bits_consumed = (WORD32)(((g_bs->ptr_read_next - start_read_pos) << 3) +
                             (start_bit_pos - g_bs->bit_pos));
    g_bs->cnt_bits -= bits_consumed;
  }
}

WORD32 ixheaacd_fd_channel_stream(
    ia_usac_data_struct *usac_data,
    ia_usac_tmp_core_coder_struct *pstr_core_coder, UWORD8 *max_sfb,
    WORD32 window_sequence_last, WORD32 chn, WORD32 noise_filling, WORD32 ch,
    ia_bit_buf_struct *it_bit_buff

    )

{
  WORD32 i;

  WORD32 tot_sfb;
  WORD32 noise_level = 0;
  WORD32 arith_reset_flag;

  WORD32 arth_size;
  WORD16 global_gain;
  WORD32 max_spec_coefficients;
  WORD32 err_code = 0;
  WORD32 noise_offset;

  WORD32 *fac_data;
  ia_sfb_info_struct *info;

  WORD8 *ptr_code_book = (WORD8 *)&usac_data->scratch_buffer;

  global_gain = ixheaacd_read_bits_buf(it_bit_buff, 8);

  if (noise_filling) {
    noise_level = ixheaacd_read_bits_buf(it_bit_buff, 3);
    noise_offset = ixheaacd_read_bits_buf(it_bit_buff, 5);

  } else {
    noise_level = 0;
    noise_offset = 0;
  }

  if (!pstr_core_coder->common_window) {
    err_code = ixheaacd_ics_info(usac_data, chn, max_sfb, it_bit_buff,
                                 window_sequence_last);

    if (err_code == -1) {
      if (usac_data->ec_flag) {
        memcpy(usac_data->max_sfb, pstr_core_coder->max_sfb, sizeof(pstr_core_coder->max_sfb));
        longjmp(*(it_bit_buff->xaac_jmp_buf),
                IA_XHEAAC_DEC_EXE_NONFATAL_INSUFFICIENT_INPUT_BYTES);
      } else {
        return err_code;
      }
    }
  }
  info = usac_data->pstr_sfb_info[chn];

  if (!pstr_core_coder->common_tw && usac_data->tw_mdct[0] == 1) {
    usac_data->tw_data_present[chn] = ixheaacd_read_bits_buf(it_bit_buff, 1);
    if (usac_data->tw_data_present[chn]) {
      WORD32 i;
      for (i = 0; i < NUM_TW_NODES; i++) {
        usac_data->tw_ratio[chn][i] = ixheaacd_read_bits_buf(it_bit_buff, 3);
      }
    }
  }

  if (*max_sfb == 0) {
    tot_sfb = 0;
  } else {
    i = 0;
    tot_sfb = info->sfb_per_sbk;

    while (usac_data->group_dis[chn][i++] < info->max_win_len) {
      tot_sfb += info->sfb_per_sbk;
    }
  }

  ixheaacd_scale_factor_data(info, tot_sfb, *max_sfb, info->sfb_per_sbk,
                             ptr_code_book);

  if ((it_bit_buff->ptr_read_next > it_bit_buff->ptr_bit_buf_end - 3) &&
      (it_bit_buff->size == it_bit_buff->max_size)) {
    return -1;
  }

  ixheaacd_section_data(usac_data, it_bit_buff, info, global_gain,
                        usac_data->factors[chn], usac_data->group_dis[chn],
                        ptr_code_book);

  if (pstr_core_coder->tns_data_present[ch] == 0)
    ixheaacd_tns_reset(info, usac_data->pstr_tns[chn]);

  if (pstr_core_coder->tns_data_present[ch] == 1)
    ixheaacd_read_tns_u(info, usac_data->pstr_tns[chn], it_bit_buff);

  if (*max_sfb > 0) {
    max_spec_coefficients =
        info->sfb_idx_tbl[*max_sfb - 1] / info->group_len[0];
  } else {
    max_spec_coefficients = 0;
  }

  if (usac_data->usac_independency_flg)
    arith_reset_flag = 1;
  else
    arith_reset_flag = ixheaacd_read_bits_buf(it_bit_buff, 1);

  switch (usac_data->window_sequence[chn]) {
    case EIGHT_SHORT_SEQUENCE:
      arth_size = usac_data->ccfl / 8;
      break;
    default:
      arth_size = usac_data->ccfl;
      break;
  }

  err_code = ixheaacd_ac_spectral_data(
      usac_data, max_spec_coefficients, noise_level, noise_offset, arth_size,
      it_bit_buff, *max_sfb, arith_reset_flag, noise_filling, chn);

  if (err_code != 0) return err_code;

  usac_data->fac_data_present[chn] = ixheaacd_read_bits_buf(it_bit_buff, 1);

  if (usac_data->fac_data_present[chn]) {
    WORD32 fac_len;
    if ((usac_data->window_sequence[chn]) == EIGHT_SHORT_SEQUENCE) {
      fac_len = (usac_data->ccfl) / 16;
    } else {
      fac_len = (usac_data->ccfl) / 8;
    }

    fac_data = usac_data->fac_data[chn];
    fac_data[0] = ixheaacd_read_bits_buf(it_bit_buff, 7);
    ixheaacd_fac_decoding(fac_len, 0, &fac_data[1], it_bit_buff);
  }

  return 0;
}