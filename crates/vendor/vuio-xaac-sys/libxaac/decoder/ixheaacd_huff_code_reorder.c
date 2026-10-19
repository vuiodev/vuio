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
#include <string.h>
#include "ixheaacd_sbr_common.h"
#include "ixheaac_type_def.h"
#include "ixheaac_error_standards.h"

#include "ixheaac_constants.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaac_basic_ops.h"

#include "ixheaacd_bitbuffer.h"
#include "ixheaacd_defines.h"
#include "ixheaacd_aac_rom.h"
#include "ixheaacd_pulsedata.h"

#include "ixheaacd_pns.h"

#include "ixheaacd_lt_predict.h"
#include "ixheaacd_cnst.h"
#include "ixheaacd_ec_defines.h"
#include "ixheaacd_ec_struct_def.h"
#include "ixheaacd_channelinfo.h"

#include "ixheaacd_drc_data_struct.h"
#include "ixheaacd_drc_dec.h"

#include "ixheaacd_sbrdecoder.h"

#include "ixheaacd_block.h"
#include "ixheaacd_channel.h"
#include "ixheaacd_common_rom.h"

#include "ixheaacd_aacdec.h"

#include "ixheaacd_sbrdecsettings.h"
#include "ixheaacd_sbr_scale.h"
#include "ixheaacd_env_extr_part.h"
#include "ixheaacd_sbr_rom.h"
#include "ixheaacd_audioobjtypes.h"
#include "ixheaacd_memory_standards.h"
#include "ixheaacd_latmdemux.h"
#include "ixheaacd_mps_polyphase.h"
#include "ixheaacd_config.h"
#include "ixheaacd_hybrid.h"
#include "ixheaacd_ps_dec.h"
#include "ixheaacd_qmf_dec.h"
#include "ixheaacd_mps_macro_def.h"
#include "ixheaacd_mps_struct_def.h"
#include "ixheaacd_mps_res_rom.h"
#include "ixheaacd_mps_aac_struct.h"
#include "ixheaacd_mps_dec.h"
#include "ixheaacd_struct_def.h"

#include "ixheaacd_rvlc.h"

const UWORD8 ixheaacd_min_huff_cb_pair_tbl[MAX_CB_PAIRS] = {
    0,  1,  3,  5,  7,  9,  16, 17, 18, 19, 20, 21,
    22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 11};
const UWORD8 ixheaacd_max_huff_cb_pair_table[MAX_CB_PAIRS] = {
    0,  2,  4,  6,  8,  10, 16, 17, 18, 19, 20, 21,
    22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 11};
const UWORD8 ixheaacd_max_huff_cw_len_table[MAX_CB] = {
    0,  11, 9,  20, 16, 13, 11, 14, 12, 17, 14, 49, 0,  0,  0,  0,
    14, 17, 21, 21, 25, 25, 29, 29, 29, 29, 33, 33, 33, 37, 37, 41};
const UWORD8 ixheaacd_huff_cb_dim_table[MAX_CB] = {
    2, 4, 4, 4, 4, 2, 2, 2, 2, 2, 2, 2, 1, 2, 2, 2,
    2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2};
const UWORD8 ixheaacd_huff_cb_dim_shift_table[MAX_CB] = {
    1, 2, 2, 2, 2, 1, 1, 1, 1, 1, 1, 1, 0, 1, 1, 1,
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1};
const UWORD8 ixheaacd_huff_cb_sign_table[MAX_CB] = {
    0, 0, 0, 1, 1, 0, 0, 1, 1, 1, 1, 1, 0, 0, 0, 0,
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1};
const UWORD8 ixheaacd_huff_cb_priority_table[MAX_CB] = {
    0, 1, 1, 2, 2,  3,  3,  4,  4,  5,  5,  22, 0,  0,  0,  0,
    6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21};
const UWORD16 ixheaacd_huff_reord_lav_table[MAX_CB] = {
    0,    1,   1,   2,   2,   4,   4,   7,   7,    12,  12,
    8191, 0,   0,   0,   0,   15,  31,  47,  63,   95,  127,
    159,  191, 223, 255, 319, 383, 511, 767, 1023, 2047};

VOID ixheaacd_huff_code_reorder_tbl_init(ia_hcr_info_struct *ptr_hcr_info) {
  ptr_hcr_info->codebook_pairs.ptr_min_cb_pair_tbl =
      ixheaacd_min_huff_cb_pair_tbl;
  ptr_hcr_info->codebook_pairs.ptr_max_cb_pair_tbl =
      ixheaacd_max_huff_cb_pair_table;
  ptr_hcr_info->table_info.ptr_max_cw_len_tbl = ixheaacd_max_huff_cw_len_table;
  ptr_hcr_info->table_info.ptr_cb_dimension_tbl = ixheaacd_huff_cb_dim_table;
  ptr_hcr_info->table_info.ptr_cb_dim_shift_tbl =
      ixheaacd_huff_cb_dim_shift_table;
  ptr_hcr_info->table_info.ptr_cb_sign_tbl = ixheaacd_huff_cb_sign_table;
  ptr_hcr_info->table_info.ptr_cb_priority = ixheaacd_huff_cb_priority_table;
  ptr_hcr_info->table_info.ptr_lav_tbl = ixheaacd_huff_reord_lav_table;
}

VOID ixheaacd_huff_mute_erroneous_lines(ia_hcr_info_struct *ptr_hcr_info) {
  WORD32 c;
  WORD32 *ptr_long = ptr_hcr_info->str_dec_io.ptr_quant_spec_coeff_base;

  for (c = 0; c < 1024; c++) {
    if (ptr_long[c] == (WORD32)8192) {
      ptr_long[c] = 0;
    }
  }
}

static UWORD8 ixheaacd_err_detect_pcw_segment(WORD8 remaining_bits_in_segment,
                                              ia_hcr_info_struct *ptr_hcr_info,
                                              ia_pcw_type_struct kind,
                                              WORD32 *qsc_base_of_cw,
                                              UWORD8 dimension) {
  WORD8 i;
  if (remaining_bits_in_segment < 0) {
    switch (kind) {
      case PCW:
        ptr_hcr_info->str_dec_io.err_log |= (ERROR_POS << 31);
        break;
      case PCW_SIGN:
        ptr_hcr_info->str_dec_io.err_log |= (ERROR_POS << 30);
        break;
      case PCW_ESC_SIGN:
        ptr_hcr_info->str_dec_io.err_log |= (ERROR_POS << 29);
        break;
    }
    for (i = dimension; i != 0; i--) {
      *qsc_base_of_cw++ = (WORD32)8192;
    }
    return 1;
  }
  return 0;
}

static VOID ixheaacd_nonpcw_sideinfo_init(ia_hcr_info_struct *ptr_hcr_info) {
  UWORD16 i, k;
  UWORD8 cb_dim;
  UWORD8 *ptr_cb = ptr_hcr_info->str_non_pcw_side_info.ptr_cb;
  UWORD16 *res_ptr_idx = ptr_hcr_info->str_non_pcw_side_info.res_ptr_idx;
  UWORD16 *ptr_num_ext_sorted_cw_in_sect =
      ptr_hcr_info->sect_info.ptr_num_ext_sorted_cw_in_sect;
  WORD32 num_ext_sorted_cw_in_sect_idx =
      ptr_hcr_info->sect_info.num_ext_sorted_cw_in_sect_idx;
  UWORD8 *ptr_ext_sorted_cw = ptr_hcr_info->sect_info.ptr_ext_sorted_cw;
  WORD32 ext_sorted_cw_idx = ptr_hcr_info->sect_info.ext_sorted_cw_idx;
  UWORD16 *ptr_num_ext_sorted_sect_in_sets =
      ptr_hcr_info->sect_info.ptr_num_ext_sorted_sect_in_sets;
  WORD32 num_ext_sorted_sect_in_sets_idx =
      ptr_hcr_info->sect_info.num_ext_sorted_sect_in_sets_idx;
  WORD32 quant_spec_coeff_idx = ptr_hcr_info->str_dec_io.quant_spec_coeff_idx;
  const UWORD8 *ptr_cb_dimension_tbl =
      ptr_hcr_info->table_info.ptr_cb_dimension_tbl;
  WORD32 loop_idx = 0;

  for (i = ptr_num_ext_sorted_sect_in_sets[num_ext_sorted_sect_in_sets_idx];
       i != 0; i--) {
    cb_dim = ptr_cb_dimension_tbl[ptr_ext_sorted_cw[ext_sorted_cw_idx]];

    for (k = ptr_num_ext_sorted_cw_in_sect[num_ext_sorted_cw_in_sect_idx];
         k != 0; k--) {
      loop_idx++;
      if (loop_idx > 256) {
        return;
      }
      *ptr_cb++ = ptr_ext_sorted_cw[ext_sorted_cw_idx];
      *res_ptr_idx++ = quant_spec_coeff_idx;
      quant_spec_coeff_idx += cb_dim;
      if (quant_spec_coeff_idx >= 1024) {
        return;
      }
    }
    num_ext_sorted_cw_in_sect_idx++;
    ext_sorted_cw_idx++;
    if (num_ext_sorted_cw_in_sect_idx >= (MAX_SFB_HCR + MAX_HCR_SETS) ||
        ext_sorted_cw_idx >= (MAX_SFB_HCR + MAX_HCR_SETS)) {
      return;
    }
  }
  num_ext_sorted_sect_in_sets_idx++;
  if (num_ext_sorted_cw_in_sect_idx >= (MAX_SFB_HCR + MAX_HCR_SETS)) {
    return;
  }

  ptr_hcr_info->sect_info.num_ext_sorted_cw_in_sect_idx =
      num_ext_sorted_cw_in_sect_idx;
  ptr_hcr_info->sect_info.ext_sorted_cw_idx = ext_sorted_cw_idx;
  ptr_hcr_info->sect_info.num_ext_sorted_sect_in_sets_idx =
      num_ext_sorted_sect_in_sets_idx;
  ptr_hcr_info->sect_info.num_ext_sorted_cw_in_sect_idx =
      num_ext_sorted_cw_in_sect_idx;
  ptr_hcr_info->str_dec_io.quant_spec_coeff_idx = quant_spec_coeff_idx;
}

static VOID ixheaacd_calc_num_ext_sorted_sect_sets(
    UWORD32 num_segment, UWORD16 *ptr_num_ext_sorted_cw_in_sect,
    WORD32 num_ext_sorted_cw_in_sect_idx,
    UWORD16 *ptr_num_ext_sorted_sect_in_sets,
    WORD32 num_ext_sorted_sect_in_sets_idx) {
  UWORD16 counter = 0;
  UWORD32 cw_sum = 0;
  UWORD16 *ptr_num_ext_sort_cw_in_sect = ptr_num_ext_sorted_cw_in_sect;
  UWORD16 *ptr_num_ext_sort_sect_in_sets = ptr_num_ext_sorted_sect_in_sets;

  while (ptr_num_ext_sort_cw_in_sect[num_ext_sorted_cw_in_sect_idx] != 0) {
    cw_sum += ptr_num_ext_sort_cw_in_sect[num_ext_sorted_cw_in_sect_idx];
    num_ext_sorted_cw_in_sect_idx++;
    if (num_ext_sorted_cw_in_sect_idx >= (MAX_SFB_HCR + MAX_HCR_SETS)) {
      return;
    }
    if (cw_sum > num_segment) {
      return;
    }
    counter++;
    if (counter > 256) {
      return;
    }
    if (cw_sum == num_segment) {
      ptr_num_ext_sort_sect_in_sets[num_ext_sorted_sect_in_sets_idx] = counter;
      num_ext_sorted_sect_in_sets_idx++;
      if (num_ext_sorted_sect_in_sets_idx >= MAX_HCR_SETS) {
        return;
      }
      counter = 0;
      cw_sum = 0;
    }
  }
  ptr_num_ext_sort_sect_in_sets[num_ext_sorted_sect_in_sets_idx] = counter;
}

static VOID ixheaacd_validate_hcr_sideinfo(WORD8 cb, WORD32 num_line,
                                           UWORD32 *error_word) {
  if (cb < ZERO_HCB || cb >= MAX_CB_CHECK || cb == (ESC_HCB + 1)) {
    *error_word |= (ERROR_POS << 4);
  }
  if (num_line < 0 || num_line > 1024) {
    *error_word |= (ERROR_POS << 5);
  }
}

static VOID ixheaacd_validate_hcr_lengths(WORD8 longest_cw_len,
                                          WORD16 reordered_spec_data_len,
                                          UWORD32 *error_word) {
  if (reordered_spec_data_len < longest_cw_len) {
    *error_word |= (ERROR_POS << 8);
  }
}

UWORD32 ixheaacd_huff_code_reorder_init(
    ia_hcr_info_struct *ptr_hcr_info,
    ia_aac_dec_channel_info_struct *ptr_aac_dec_channel_info,
    ia_aac_dec_tables_struct *ptr_aac_tables, ia_bit_buf_struct *itt_bit_buff) {
  ia_ics_info_struct *ptr_ics_info = &ptr_aac_dec_channel_info->str_ics_info;
  WORD16 *ptr_num_sect_lines;
  UWORD8 *ptr_cb;
  WORD16 num_sect;
  WORD8 cb;
  WORD32 num_line;
  WORD32 i;

  ptr_hcr_info->str_dec_io.reordered_spec_data_len =
      ptr_aac_dec_channel_info->reorder_spect_data_len;
  ptr_hcr_info->str_dec_io.longest_cw_len =
      ptr_aac_dec_channel_info->longest_cw_len;
  ptr_hcr_info->str_dec_io.ptr_quant_spec_coeff_base =
      ptr_aac_dec_channel_info->ptr_spec_coeff;
  ptr_hcr_info->str_dec_io.quant_spec_coeff_idx = 0;
  ptr_hcr_info->str_dec_io.ptr_cb = ptr_aac_dec_channel_info->cb4_hcr_arr;
  ptr_hcr_info->str_dec_io.ptr_num_line_in_sect =
      ptr_aac_dec_channel_info->num_line_in_sec4_hcr_arr;
  ptr_hcr_info->str_dec_io.num_sect = ptr_aac_dec_channel_info->number_sect;
  ptr_hcr_info->str_dec_io.err_log = 0;
  ptr_hcr_info->str_non_pcw_side_info.ptr_result_base =
      ptr_aac_dec_channel_info->ptr_spec_coeff;

  ptr_hcr_info->str_dec_io.bit_str_idx =
      itt_bit_buff->size - itt_bit_buff->cnt_bits;
  itt_bit_buff->byte_ptr = (UWORD8 *)ptr_aac_dec_channel_info->scratch_buf_ptr;
  itt_bit_buff->ptr_start = (UWORD8 *)ptr_aac_dec_channel_info->scratch_buf_ptr;

  if (ptr_aac_dec_channel_info->str_ics_info.window_sequence ==
      EIGHT_SHORT_SEQUENCE) {
    WORD16 band;
    WORD16 max_band;
    WORD8 group;
    WORD8 win_group_len;
    WORD8 window;
    WORD8 num_unit_in_band;
    WORD8 cnt_unit_in_band;
    WORD8 grp_win;
    WORD8 cb_prev;

    WORD8 *ptr_code_book;
    const WORD16 *band_offsets;
    WORD16 num_groups;

    ptr_code_book = ptr_aac_dec_channel_info->ptr_code_book;
    ptr_num_sect_lines = ptr_hcr_info->str_dec_io.ptr_num_line_in_sect;
    ptr_cb = ptr_hcr_info->str_dec_io.ptr_cb;
    band_offsets = (WORD16 *)ixheaacd_getscalefactorbandoffsets(ptr_ics_info,
                                                                ptr_aac_tables);
    num_groups = ptr_ics_info->num_window_groups;

    num_line = 0;
    num_sect = 0;
    cb = ptr_code_book[0];
    cb_prev = ptr_code_book[0];

    *ptr_cb++ = cb_prev;

    max_band = ptr_ics_info->max_sfb;
    for (band = 0; band < max_band; band++) {
      num_unit_in_band = ((band_offsets[band + 1] - band_offsets[band]) >> 2);
      for (cnt_unit_in_band = num_unit_in_band; cnt_unit_in_band != 0;
           cnt_unit_in_band--) {
        for (window = 0, group = 0; group < num_groups; group++) {
          win_group_len = ptr_ics_info->window_group_length[group];
          for (grp_win = win_group_len; grp_win != 0; grp_win--, window++) {
            cb = ptr_code_book[group * 16 + band];
            if (cb != cb_prev) {
              ixheaacd_validate_hcr_sideinfo(cb, num_line,
                                             &ptr_hcr_info->str_dec_io.err_log);
              if (ptr_hcr_info->str_dec_io.err_log != 0) {
                return (ptr_hcr_info->str_dec_io.err_log);
              }
              *ptr_cb++ = cb;
              *ptr_num_sect_lines++ = num_line;
              num_sect++;

              cb_prev = cb;
              num_line = LINES_PER_UNIT;
            } else {
              num_line += LINES_PER_UNIT;
            }
          }
        }
      }
    }

    num_sect++;

    ixheaacd_validate_hcr_sideinfo(cb, num_line,
                                   &ptr_hcr_info->str_dec_io.err_log);
    if (num_sect <= 0 || num_sect > 1024 / 2) {
      ptr_hcr_info->str_dec_io.err_log |= (ERROR_POS << 7);
    }
    ixheaacd_validate_hcr_lengths(
        ptr_hcr_info->str_dec_io.longest_cw_len,
        ptr_hcr_info->str_dec_io.reordered_spec_data_len,
        &ptr_hcr_info->str_dec_io.err_log);
    if (ptr_hcr_info->str_dec_io.err_log != 0) {
      return (ptr_hcr_info->str_dec_io.err_log);
    }

    *ptr_cb = cb;
    *ptr_num_sect_lines = num_line;
    ptr_hcr_info->str_dec_io.num_sect = num_sect;

  } else {
    ixheaacd_validate_hcr_lengths(
        ptr_hcr_info->str_dec_io.longest_cw_len,
        ptr_hcr_info->str_dec_io.reordered_spec_data_len,
        &ptr_hcr_info->str_dec_io.err_log);
    num_sect = ptr_hcr_info->str_dec_io.num_sect;
    ptr_num_sect_lines = ptr_hcr_info->str_dec_io.ptr_num_line_in_sect;
    ptr_cb = ptr_hcr_info->str_dec_io.ptr_cb;
    if (num_sect <= 0 || num_sect > 64) {
      ptr_hcr_info->str_dec_io.err_log |= (ERROR_POS << 6);
      num_sect = 0;
    }

    for (i = num_sect; i != 0; i--) {
      cb = *ptr_cb++;

      if (cb < ZERO_HCB || cb >= MAX_CB_CHECK || cb == (ESC_HCB + 1)) {
        ptr_hcr_info->str_dec_io.err_log |= (ERROR_POS << 2);
      }

      num_line = *ptr_num_sect_lines++;

      if ((num_line <= 0) || (num_line > 1024)) {
        ptr_hcr_info->str_dec_io.err_log |= (ERROR_POS << 3);
      }
    }
    if (ptr_hcr_info->str_dec_io.err_log != 0) {
      return (ptr_hcr_info->str_dec_io.err_log);
    }
  }

  ptr_cb = ptr_hcr_info->str_dec_io.ptr_cb;
  for (i = 0; i < num_sect; i++) {
    if ((*ptr_cb == NOISE_HCB) || (*ptr_cb == INTENSITY_HCB2) ||
        (*ptr_cb == INTENSITY_HCB)) {
      *ptr_cb = 0;
    }
    ptr_cb++;
  }

  return (ptr_hcr_info->str_dec_io.err_log);
}

static VOID ixheaacd_huff_calc_num_cwd(ia_hcr_info_struct *ptr_hcr_info) {
  WORD32 sect_idx;
  UWORD32 num_code_word;

  UWORD32 num_sect = ptr_hcr_info->str_dec_io.num_sect;
  UWORD8 *ptr_cb = ptr_hcr_info->str_dec_io.ptr_cb;
  WORD16 *ptr_num_line_in_sect = ptr_hcr_info->str_dec_io.ptr_num_line_in_sect;
  const UWORD8 *ptr_cb_dim_shift_tbl =
      ptr_hcr_info->table_info.ptr_cb_dim_shift_tbl;
  UWORD16 *ptr_num_cw_in_sect = ptr_hcr_info->sect_info.ptr_num_cw_in_sect;

  num_code_word = 0;
  for (sect_idx = num_sect; sect_idx != 0; sect_idx--) {
    *ptr_num_cw_in_sect =
        *ptr_num_line_in_sect++ >> ptr_cb_dim_shift_tbl[*ptr_cb];
    if (*ptr_cb != 0) {
      num_code_word += *ptr_num_cw_in_sect;
    }
    ptr_num_cw_in_sect++;
    ptr_cb++;
  }
  ptr_hcr_info->sect_info.num_code_word = num_code_word;
}

static VOID ixheaacd_huff_sort_sect_cb_cwd(ia_hcr_info_struct *ptr_hcr_info) {
  UWORD32 i, j, k;
  UWORD8 temp;
  UWORD32 counter;
  UWORD32 start_offset;
  UWORD32 num_zero_sect;
  UWORD8 *ptr_dest;
  UWORD32 num_sect_dec;

  UWORD32 num_sect = ptr_hcr_info->str_dec_io.num_sect;
  UWORD8 *ptr_cb = ptr_hcr_info->str_dec_io.ptr_cb;
  UWORD8 *ptr_sorted_cb = ptr_hcr_info->sect_info.ptr_sorted_cb;
  UWORD16 *ptr_num_cw_in_sect = ptr_hcr_info->sect_info.ptr_num_cw_in_sect;
  UWORD16 *ptr_num_sorted_cw_in_sect =
      ptr_hcr_info->sect_info.ptr_num_sorted_cw_in_sect;
  UWORD8 *ptr_cb_switch = ptr_hcr_info->sect_info.ptr_cb_switch;
  UWORD16 *ptr_reorder_offset = ptr_hcr_info->sect_info.ptr_reorder_offset;
  const UWORD8 *ptr_cb_priority = ptr_hcr_info->table_info.ptr_cb_priority;
  const UWORD8 *ptr_min_cb_pair_tbl =
      ptr_hcr_info->codebook_pairs.ptr_min_cb_pair_tbl;
  const UWORD8 *ptr_max_cb_pair_tbl =
      ptr_hcr_info->codebook_pairs.ptr_max_cb_pair_tbl;
  const UWORD8 *ptr_cb_dim_shift_tbl =
      ptr_hcr_info->table_info.ptr_cb_dim_shift_tbl;

  UWORD32 search_start_idx = 0;

  ptr_dest = ptr_sorted_cb;
  num_zero_sect = 0;
  for (i = num_sect; i != 0; i--) {
    if (ptr_cb_priority[*ptr_cb] == 0) {
      num_zero_sect += 1;
    }
    *ptr_dest++ = ptr_cb_priority[*ptr_cb++];
  }
  ptr_hcr_info->sect_info.num_sorted_section = num_sect - num_zero_sect;
  ptr_cb = ptr_hcr_info->str_dec_io.ptr_cb;

  num_sect_dec = num_sect - 1;
  if (num_sect_dec > 0) {
    counter = num_sect_dec;
    for (j = num_sect_dec; j != 0; j--) {
      for (i = 0; i < counter; i++) {
        if (ptr_sorted_cb[i + 1] > ptr_sorted_cb[i]) {
          temp = ptr_sorted_cb[i];
          ptr_sorted_cb[i] = ptr_sorted_cb[i + 1];
          ptr_sorted_cb[i + 1] = temp;
        }
      }
      counter -= 1;
    }
  }

  for (i = num_sect; i != 0; i--) {
    *ptr_cb_switch++ = 0;
  }
  ptr_cb_switch = ptr_hcr_info->sect_info.ptr_cb_switch;

  for (j = 0; j < num_sect; j++) {
    for (i = search_start_idx; i < num_sect; i++) {
      if (ptr_cb_switch[i] == 0 &&
          (ptr_min_cb_pair_tbl[ptr_sorted_cb[j]] == ptr_cb[i] ||
           ptr_max_cb_pair_tbl[ptr_sorted_cb[j]] == ptr_cb[i])) {
        ptr_cb_switch[i] = 1;
        ptr_sorted_cb[j] = ptr_cb[i];
        ptr_num_sorted_cw_in_sect[j] = ptr_num_cw_in_sect[i];

        start_offset = 0;
        for (k = 0; k < i; k++) {
          start_offset += ptr_num_cw_in_sect[k]
                          << ptr_cb_dim_shift_tbl[ptr_cb[k]];
        }
        ptr_reorder_offset[j] = start_offset;

        if (i == search_start_idx) {
          UWORD32 k = i;
          while (ptr_cb_switch[k++] == 1) search_start_idx++;
        }
        break;
      }
    }
  }
}

static VOID ixheaacd_huff_ext_sect_info(ia_hcr_info_struct *ptr_hcr_info) {
  UWORD32 srt_sec_cnt = 0;
  UWORD32 x_srt_sc_cnt = 0;
  UWORD32 remain_num_cw_sort_sec;
  UWORD32 in_segment_remain_num_cw;

  UWORD32 num_sorted_section = ptr_hcr_info->sect_info.num_sorted_section;
  UWORD8 *ptr_sorted_cb = ptr_hcr_info->sect_info.ptr_sorted_cb;
  UWORD16 *ptr_num_sorted_cw_in_sect =
      ptr_hcr_info->sect_info.ptr_num_sorted_cw_in_sect;
  UWORD8 *ptr_extended_sorted_code_book =
      ptr_hcr_info->sect_info.ptr_ext_sorted_cw;
  UWORD16 *ptr_num_ext_sort_cw_sect =
      ptr_hcr_info->sect_info.ptr_num_ext_sorted_cw_in_sect;
  UWORD32 num_segment = ptr_hcr_info->str_segment_info.num_segment;
  UWORD8 *ptr_ext_sorted_sect_max_cb_len =
      ptr_hcr_info->sect_info.ptr_ext_sorted_sect_max_cb_len;
  WORD8 longest_cw_len = ptr_hcr_info->str_dec_io.longest_cw_len;
  const UWORD8 *ptr_max_cw_len_tbl =
      ptr_hcr_info->table_info.ptr_max_cw_len_tbl;

  remain_num_cw_sort_sec = ptr_num_sorted_cw_in_sect[srt_sec_cnt];
  in_segment_remain_num_cw = num_segment;

  while (srt_sec_cnt < num_sorted_section) {
    if (in_segment_remain_num_cw < remain_num_cw_sort_sec) {
      ptr_num_ext_sort_cw_sect[x_srt_sc_cnt] = in_segment_remain_num_cw;
      ptr_extended_sorted_code_book[x_srt_sc_cnt] = ptr_sorted_cb[srt_sec_cnt];

      remain_num_cw_sort_sec -= in_segment_remain_num_cw;
      in_segment_remain_num_cw = num_segment;
    } else if (in_segment_remain_num_cw == remain_num_cw_sort_sec) {
      ptr_num_ext_sort_cw_sect[x_srt_sc_cnt] = in_segment_remain_num_cw;
      ptr_extended_sorted_code_book[x_srt_sc_cnt] = ptr_sorted_cb[srt_sec_cnt];

      srt_sec_cnt++;
      remain_num_cw_sort_sec = ptr_num_sorted_cw_in_sect[srt_sec_cnt];
      in_segment_remain_num_cw = num_segment;
    } else {
      ptr_num_ext_sort_cw_sect[x_srt_sc_cnt] = remain_num_cw_sort_sec;
      ptr_extended_sorted_code_book[x_srt_sc_cnt] = ptr_sorted_cb[srt_sec_cnt];

      in_segment_remain_num_cw -= remain_num_cw_sort_sec;
      srt_sec_cnt++;
      remain_num_cw_sort_sec = ptr_num_sorted_cw_in_sect[srt_sec_cnt];
    }
    ptr_ext_sorted_sect_max_cb_len[x_srt_sc_cnt] =
        min(ptr_max_cw_len_tbl[ptr_extended_sorted_code_book[x_srt_sc_cnt]],
            longest_cw_len);

    x_srt_sc_cnt += 1;

    if (x_srt_sc_cnt >= (MAX_SFB_HCR + MAX_HCR_SETS)) {
      ptr_hcr_info->str_dec_io.err_log |= (ERROR_POS << 28);
      return;
    }
  }
  ptr_num_ext_sort_cw_sect[x_srt_sc_cnt] = 0;
}

static UWORD32 ixheaacd_hcr_prepare_segmentation_grid(
    ia_hcr_info_struct *ptr_hcr_info) {
  UWORD16 i, j;
  UWORD16 num_segment = 0;
  UWORD16 segment_start = 0;
  UWORD8 segment_width;
  UWORD8 last_segment_width;
  UWORD8 sorted_code_book;
  UWORD8 end_flag = 0;
  UWORD16 intermediate_result;

  WORD8 longest_cw_len = ptr_hcr_info->str_dec_io.longest_cw_len;
  WORD16 reordered_spec_data_len =
      ptr_hcr_info->str_dec_io.reordered_spec_data_len;
  UWORD32 num_sorted_section = ptr_hcr_info->sect_info.num_sorted_section;
  UWORD8 *ptr_sorted_cb = ptr_hcr_info->sect_info.ptr_sorted_cb;
  UWORD16 *ptr_num_sorted_cw_in_sect =
      ptr_hcr_info->sect_info.ptr_num_sorted_cw_in_sect;
  UWORD16 *arr_seg_start_l = ptr_hcr_info->str_segment_info.arr_seg_start_l;
  UWORD16 *arr_seg_start_r = ptr_hcr_info->str_segment_info.arr_seg_start_r;
  WORD8 *p_remaining_bits_in_seg =
      ptr_hcr_info->str_segment_info.p_remaining_bits_in_seg;
  UWORD16 bit_str_idx = ptr_hcr_info->str_dec_io.bit_str_idx;
  const UWORD8 *ptr_max_cw_len_tbl =
      ptr_hcr_info->table_info.ptr_max_cw_len_tbl;

  for (i = num_sorted_section; i != 0; i--) {
    sorted_code_book = *ptr_sorted_cb++;
    segment_width = min(ptr_max_cw_len_tbl[sorted_code_book], longest_cw_len);

    for (j = *ptr_num_sorted_cw_in_sect; j != 0; j--) {
      intermediate_result = bit_str_idx + segment_start;
      if ((segment_start + segment_width) <= reordered_spec_data_len) {
        *arr_seg_start_l++ = intermediate_result;
        *arr_seg_start_r++ = intermediate_result + segment_width - 1;
        *p_remaining_bits_in_seg++ = segment_width;
        segment_start += segment_width;
        num_segment += 1;
      } else {
        arr_seg_start_l--;
        arr_seg_start_r--;
        p_remaining_bits_in_seg--;
        segment_start = *arr_seg_start_l - bit_str_idx;

        last_segment_width = reordered_spec_data_len - segment_start;
        *p_remaining_bits_in_seg = last_segment_width;
        *arr_seg_start_r = bit_str_idx + segment_start + last_segment_width - 1;
        end_flag = 1;
        break;
      }
    }
    ptr_num_sorted_cw_in_sect++;
    if (end_flag != 0) {
      break;
    }
  }

  if (num_segment == 0) ptr_hcr_info->str_dec_io.err_log |= (ERROR_POS << 9);

  ptr_hcr_info->str_segment_info.num_segment = num_segment;

  return (ptr_hcr_info->str_dec_io.err_log);
}

static PLATFORM_INLINE UWORD16 *ixheaacd_huff_dec_pair_hcr_pcw(
    ia_hcr_info_struct *ptr_hcr_info, ia_bit_buf_struct *it_bit_buff,
    WORD no_bands, const UWORD16 *code_book_tbl, WORD32 *read_word,
    WORD32 tbl_sign, const UWORD32 *idx_table, UWORD16 *arr_seg_start_l,
    WORD32 *read_bits, WORD32 huff_mode, WORD8 *p_remaining_bits_in_seg,
    WORD32 *ptr_num_decoded_bits)

{
  WORD32 spec_index = ptr_hcr_info->str_dec_io.quant_spec_coeff_idx;
  WORD32 *spec_coef =
      ptr_hcr_info->str_dec_io.ptr_quant_spec_coeff_base + spec_index;
  WORD16 index, length;
  WORD32 y, z;
  UWORD8 *ptr_read_next = it_bit_buff->ptr_read_next;
  WORD32 *bit_pos = &it_bit_buff->bit_pos;

  do {
    UWORD32 read_word1;

    WORD32 read_bit_offset =
        *arr_seg_start_l - (it_bit_buff->size - *read_bits);

    if (read_bit_offset) {
      *read_bits -= read_bit_offset;
      *bit_pos += read_bit_offset;
      ixheaacd_aac_read_byte_corr(&ptr_read_next, bit_pos, read_word,
                                  it_bit_buff->ptr_bit_buf_end);
    }
    *bit_pos = max(0, *bit_pos);
    read_word1 = *read_word << *bit_pos;
    ixheaacd_huffman_decode(read_word1, &index, &length, code_book_tbl,
                            idx_table);
    *bit_pos += length;
    *ptr_num_decoded_bits += length;
    *p_remaining_bits_in_seg -= length;
    *arr_seg_start_l += length;
    *read_bits -= length;
    ixheaacd_aac_read_byte_corr(&ptr_read_next, bit_pos, read_word,
                                it_bit_buff->ptr_bit_buf_end);
    if (tbl_sign) {
      WORD32 temp_word;
      temp_word = *read_word << *bit_pos;
      y = index / huff_mode;
      z = index - huff_mode * y;

      if (y) {
        if (temp_word & 0x80000000) y = -y;

        temp_word = temp_word << 1;
        *bit_pos += 1;
        *p_remaining_bits_in_seg -= 1;
        *ptr_num_decoded_bits += 1;
        *arr_seg_start_l += 1;
        *read_bits -= 1;
      }
      *spec_coef++ = y;
      spec_index++;

      if (z) {
        if (temp_word & 0x80000000) {
          z = -z;
        }
        temp_word <<= 1;
        *bit_pos += 1;
        *p_remaining_bits_in_seg -= 1;
        *ptr_num_decoded_bits += 1;
        *arr_seg_start_l += 1;
        *read_bits -= 1;
      }
      *spec_coef++ = z;
      spec_index++;
    } else {
      y = (index / huff_mode) - 4;
      z = index - ((y + 4) * huff_mode) - 4;

      *spec_coef++ = y;
      *spec_coef++ = z;
      spec_index += 2;
    }
    ixheaacd_aac_read_byte_corr(&ptr_read_next, bit_pos, read_word,
                                it_bit_buff->ptr_bit_buf_end);
    no_bands--;
    arr_seg_start_l++;
    p_remaining_bits_in_seg++;
  } while (no_bands != 0);

  it_bit_buff->ptr_read_next = ptr_read_next;
  ptr_hcr_info->str_dec_io.quant_spec_coeff_idx = spec_index;

  return arr_seg_start_l;
}

static PLATFORM_INLINE WORD16 ixheaacd_huff_dec_pair_hcr_non_pcw(
    ia_bit_buf_struct *itt_bit_buff, WORD32 *spec_coef,
    const UWORD16 *code_book_tbl, WORD32 tbl_sign, const UWORD32 *idx_table,
    WORD32 huff_mode)

{
  WORD16 index, length;
  WORD32 y, z;
  WORD32 read_word1;
  WORD32 read_word;

  read_word = ixheaacd_aac_showbits_32(itt_bit_buff->byte_ptr,
                                       itt_bit_buff->bit_count, NULL);

  ixheaacd_huffman_decode(read_word, &index, &length, code_book_tbl, idx_table);
  read_word1 = read_word << length;
  if (tbl_sign) {
    WORD32 temp_word;
    temp_word = read_word1;
    y = index / huff_mode;
    z = index - huff_mode * y;

    if (y) {
      if (temp_word & 0x80000000) y = -y;

      temp_word = temp_word << 1;
      length++;
    }
    *spec_coef++ = y;

    if (z) {
      if (temp_word & 0x80000000) {
        z = -z;
      }
      temp_word <<= 1;
      length++;
    }
    *spec_coef++ = z;
  } else {
    y = (index / huff_mode) - 4;
    z = index - ((y + 4) * huff_mode) - 4;

    *spec_coef++ = y;
    *spec_coef++ = z;
  }

  return length;
}

static PLATFORM_INLINE UWORD16 *ixheaacd_huff_dec_quad_hcr_pcw(
    ia_hcr_info_struct *ptr_hcr_info, ia_bit_buf_struct *it_bit_buff,
    WORD no_bands, const UWORD16 *code_book_tbl, WORD32 tbl_sign,
    const UWORD32 *idx_table, WORD32 *read_word, WORD32 *read_bits,
    UWORD16 *arr_seg_start_l, WORD8 *p_remaining_bits_in_seg,
    WORD32 *ptr_num_decoded_bits) {
  WORD16 index, length;

  UWORD8 *ptr_read_next = it_bit_buff->ptr_read_next;
  WORD32 spec_index = ptr_hcr_info->str_dec_io.quant_spec_coeff_idx;
  WORD32 *spec_coef =
      ptr_hcr_info->str_dec_io.ptr_quant_spec_coeff_base + spec_index;
  WORD32 *bit_pos = &it_bit_buff->bit_pos;

  do {
    UWORD32 read_word1;

    WORD32 read_bit_offset =
        *arr_seg_start_l - (it_bit_buff->size - *read_bits);

    if (read_bit_offset) {
      *read_bits -= read_bit_offset;
      *bit_pos += read_bit_offset;
      ixheaacd_aac_read_byte_corr(&ptr_read_next, bit_pos, read_word,
                                  it_bit_buff->ptr_bit_buf_end);
    }
    *bit_pos = max(0, *bit_pos);
    read_word1 = *read_word << *bit_pos;
    ixheaacd_huffman_decode(read_word1, &index, &length, code_book_tbl,
                            idx_table);
    *bit_pos += length;
    *p_remaining_bits_in_seg -= length;
    *read_bits -= length;
    *ptr_num_decoded_bits += length;
    *arr_seg_start_l += length;
    ixheaacd_aac_read_byte_corr(&ptr_read_next, bit_pos, read_word,
                                it_bit_buff->ptr_bit_buf_end);
    if (tbl_sign) {
      WORD32 temp_word;
      WORD32 w, x, y, z;
      temp_word = *read_word << *bit_pos;
      w = index / 27;
      index = index - w * 27;
      x = index / 9;
      index = index - x * 9;
      y = index / 3;
      z = index - y * 3;
      if (w) {
        if (temp_word & 0x80000000) w = -w;
        temp_word <<= 1;
        *bit_pos += 1;
        *p_remaining_bits_in_seg -= 1;
        *read_bits -= 1;
        *ptr_num_decoded_bits += 1;
        *arr_seg_start_l += 1;
      }
      *spec_coef++ = w;
      spec_index++;

      if (x) {
        if (temp_word & 0x80000000) x = -x;
        temp_word <<= 1;
        *bit_pos += 1;
        *p_remaining_bits_in_seg -= 1;
        *read_bits -= 1;
        *ptr_num_decoded_bits += 1;
        *arr_seg_start_l += 1;
      }
      *spec_coef++ = x;
      spec_index++;
      if (y) {
        if (temp_word & 0x80000000) y = -y;
        temp_word <<= 1;
        *bit_pos += 1;
        *p_remaining_bits_in_seg -= 1;
        *read_bits -= 1;
        *ptr_num_decoded_bits += 1;
        *arr_seg_start_l += 1;
      }
      *spec_coef++ = y;
      spec_index++;
      if (z) {
        if (temp_word & 0x80000000) z = -z;
        temp_word <<= 1;
        *bit_pos += 1;
        *p_remaining_bits_in_seg -= 1;
        *read_bits -= 1;
        *ptr_num_decoded_bits += 1;
        *arr_seg_start_l += 1;
      }
      *spec_coef++ = z;
      spec_index++;

    }

    else {
      WORD32 w, x, y, z;

      w = index / 27 - 1;
      index = index - (w + 1) * 27;
      x = index / 9 - 1;
      index = index - (x + 1) * 9;
      y = index / 3 - 1;
      z = index - ((y + 1) * 3) - 1;
      *spec_coef++ = w;

      *spec_coef++ = x;

      *spec_coef++ = y;

      *spec_coef++ = z;
      spec_index += 4;
    }

    ixheaacd_aac_read_byte_corr(&ptr_read_next, bit_pos, read_word,
                                it_bit_buff->ptr_bit_buf_end);
    arr_seg_start_l++;
    p_remaining_bits_in_seg++;
    no_bands--;
  } while (no_bands != 0);

  it_bit_buff->ptr_read_next = ptr_read_next;
  ptr_hcr_info->str_dec_io.quant_spec_coeff_idx = spec_index;

  return arr_seg_start_l;
}

static UWORD16 *ixheaacd_huff_dec_word_hcr_pcw(
    ia_hcr_info_struct *ptr_hcr_info, ia_bit_buf_struct *it_bit_buff,
    WORD no_bands, const UWORD16 *code_book_tbl, WORD32 *read_word,
    const UWORD32 *idx_table, UWORD16 *arr_seg_start_l, WORD32 *read_bits,
    WORD8 *p_remaining_bits_in_seg, WORD32 *ptr_num_decoded_bits) {
  WORD32 sp1, sp2;
  WORD32 flush_cw;
  WORD32 i, value, norm_val, off;
  WORD32 out1, out2;
  WORD16 index;
  WORD32 length;
  UWORD8 *ptr_read_next = it_bit_buff->ptr_read_next;
  WORD32 spec_index = ptr_hcr_info->str_dec_io.quant_spec_coeff_idx;
  WORD32 *spec_coef =
      ptr_hcr_info->str_dec_io.ptr_quant_spec_coeff_base + spec_index;
  WORD32 *bit_pos = &it_bit_buff->bit_pos;

  do {
    UWORD32 read_word1;

    WORD32 read_bit_offset =
        *arr_seg_start_l - (it_bit_buff->size - *read_bits);

    if (read_bit_offset) {
      *read_bits -= read_bit_offset;
      *bit_pos += read_bit_offset;
      ixheaacd_aac_read_byte_corr1(&ptr_read_next, bit_pos, read_word,
                                   it_bit_buff->ptr_bit_buf_end);
    }
    *bit_pos = max(0, *bit_pos);
    read_word1 = *read_word << *bit_pos;
    ixheaacd_huff_sfb_table(read_word1, &index, &length, code_book_tbl,
                            idx_table);
    *bit_pos += length;
    *read_bits -= length;
    *arr_seg_start_l += length;
    *p_remaining_bits_in_seg -= length;
    *ptr_num_decoded_bits += length;
    ixheaacd_aac_read_byte_corr(&ptr_read_next, bit_pos, read_word,
                                it_bit_buff->ptr_bit_buf_end);

    out1 = index / 17;
    out2 = index - out1 * 17;
    flush_cw = *read_word << *bit_pos;

    sp1 = out1;
    sp2 = out2;

    if (out1) {
      if (flush_cw & 0x80000000) {
        out1 = -out1;
      }
      *bit_pos += 1;
      *read_bits -= 1;
      *p_remaining_bits_in_seg -= 1;
      *ptr_num_decoded_bits += 1;
      *arr_seg_start_l += 1;
      flush_cw = (WORD32)flush_cw << 1;
    }

    if (out2) {
      *bit_pos += 1;
      *read_bits -= 1;
      *p_remaining_bits_in_seg -= 1;
      *ptr_num_decoded_bits += 1;
      *arr_seg_start_l += 1;
      if (flush_cw & 0x80000000) {
        out2 = -out2;
      }
    }

    ixheaacd_aac_read_byte_corr(&ptr_read_next, bit_pos, read_word,
                                it_bit_buff->ptr_bit_buf_end);

    if (sp1 == 16) {
      i = 4;
      value = ixheaac_extu(*read_word, *bit_pos, 23);
      value = value | 0xfffffe00;
      norm_val = ixheaac_norm32(value);

      i += (norm_val - 22);
      *bit_pos += (norm_val - 21);
      *p_remaining_bits_in_seg -= (norm_val - 21);
      *ptr_num_decoded_bits += (norm_val - 21);
      *read_bits -= (norm_val - 21);
      *arr_seg_start_l += (norm_val - 21);

      ixheaacd_aac_read_byte_corr(&ptr_read_next, bit_pos, read_word,
                                  it_bit_buff->ptr_bit_buf_end);

      off = ixheaac_extu(*read_word, *bit_pos, 32 - i);

      *bit_pos += i;
      *p_remaining_bits_in_seg -= i;
      *ptr_num_decoded_bits += i;
      *read_bits -= i;
      *arr_seg_start_l += i;

      ixheaacd_aac_read_byte_corr(&ptr_read_next, bit_pos, read_word,
                                  it_bit_buff->ptr_bit_buf_end);
      ixheaacd_aac_read_byte_corr(&ptr_read_next, bit_pos, read_word,
                                  it_bit_buff->ptr_bit_buf_end);

      i = off + ((WORD32)1 << i);

      if (out1 < 0)
        *spec_coef++ = -i;
      else
        *spec_coef++ = i;
      spec_index++;
    } else {
      *spec_coef++ = out1;
      spec_index++;
    }

    if (sp2 == 16) {
      i = 4;
      value = ixheaac_extu(*read_word, *bit_pos, 23);
      value = value | 0xfffffe00;
      norm_val = ixheaac_norm32(value);

      i += (norm_val - 22);

      *bit_pos += (norm_val - 21);
      *read_bits -= (norm_val - 21);
      *p_remaining_bits_in_seg -= (norm_val - 21);
      *ptr_num_decoded_bits += (norm_val - 21);
      *arr_seg_start_l += (norm_val - 21);
      ixheaacd_aac_read_byte_corr(&ptr_read_next, bit_pos, read_word,
                                  it_bit_buff->ptr_bit_buf_end);

      off = ixheaac_extu(*read_word, *bit_pos, 32 - i);

      *bit_pos += i;
      *p_remaining_bits_in_seg -= i;
      *ptr_num_decoded_bits += i;
      *read_bits -= i;
      *arr_seg_start_l += i;

      ixheaacd_aac_read_byte_corr(&ptr_read_next, bit_pos, read_word,
                                  it_bit_buff->ptr_bit_buf_end);
      ixheaacd_aac_read_byte_corr(&ptr_read_next, bit_pos, read_word,
                                  it_bit_buff->ptr_bit_buf_end);

      i = off + ((WORD32)1 << i);

      if (out2 < 0)
        *spec_coef++ = -i;
      else
        *spec_coef++ = i;
      spec_index++;
    } else {
      *spec_coef++ = out2;
      spec_index++;
    }

    arr_seg_start_l++;
    p_remaining_bits_in_seg++;

    no_bands--;
  } while (no_bands != 0);

  it_bit_buff->ptr_read_next = ptr_read_next;
  ptr_hcr_info->str_dec_io.quant_spec_coeff_idx = spec_index;

  return arr_seg_start_l;
}

static VOID ixheaacd_decode_pcw(ia_bit_buf_struct *itt_bit_buff,
                                ia_hcr_info_struct *ptr_hcr_info,
                                ia_aac_dec_tables_struct *ptr_aac_tables) {
  UWORD16 ext_sort_sec;
  UWORD16 cur_ext_sort_cw_sec;
  UWORD8 codebook;
  UWORD8 dimension;
  WORD32 increment;

  WORD32 num_ext_sorted_cw_in_sect_idx =
      ptr_hcr_info->sect_info.num_ext_sorted_cw_in_sect_idx;
  UWORD8 *ptr_ext_sorted_cw = ptr_hcr_info->sect_info.ptr_ext_sorted_cw;
  WORD32 ext_sorted_cw_idx = ptr_hcr_info->sect_info.ext_sorted_cw_idx;
  UWORD16 *ptr_num_ext_sorted_sect_in_sets =
      ptr_hcr_info->sect_info.ptr_num_ext_sorted_sect_in_sets;
  WORD32 num_ext_sorted_sect_in_sets_idx =
      ptr_hcr_info->sect_info.num_ext_sorted_sect_in_sets_idx;
  WORD32 *ptr_quant_spec_coeff =
      ptr_hcr_info->str_dec_io.ptr_quant_spec_coeff_base;
  UWORD16 *arr_seg_start_l = ptr_hcr_info->str_segment_info.arr_seg_start_l;
  WORD8 *p_remaining_bits_in_seg =
      ptr_hcr_info->str_segment_info.p_remaining_bits_in_seg;
  UWORD8 *ptr_ext_sorted_sect_max_cb_len =
      ptr_hcr_info->sect_info.ptr_ext_sorted_sect_max_cb_len;
  WORD32 ext_sorted_sect_max_cb_len_idx =
      ptr_hcr_info->sect_info.ext_sorted_sect_max_cb_len_idx;
  UWORD8 max_allowed_cw_len;
  WORD32 num_decoded_bits;
  const UWORD8 *ptr_cb_dimension_tbl =
      ptr_hcr_info->table_info.ptr_cb_dimension_tbl;

  WORD32 read_word = ixheaacd_aac_showbits_32(
      itt_bit_buff->ptr_read_next, itt_bit_buff->cnt_bits, &increment);
  WORD32 read_bits = itt_bit_buff->cnt_bits;

  itt_bit_buff->ptr_read_next += increment;

  for (ext_sort_sec =
           ptr_num_ext_sorted_sect_in_sets[num_ext_sorted_sect_in_sets_idx];
       ext_sort_sec != 0; ext_sort_sec--) {
    codebook = ptr_ext_sorted_cw[ext_sorted_cw_idx];
    if (codebook <= 0) return;

    ext_sorted_cw_idx++;
    if (ext_sorted_cw_idx >= (MAX_SFB_HCR + MAX_HCR_SETS)) {
      return;
    }
    dimension = ptr_cb_dimension_tbl[codebook];
    max_allowed_cw_len =
        ptr_ext_sorted_sect_max_cb_len[ext_sorted_sect_max_cb_len_idx];
    ext_sorted_sect_max_cb_len_idx++;
    if (ext_sorted_sect_max_cb_len_idx >= (MAX_SFB_HCR + MAX_HCR_SETS)) {
      return;
    }

    if (codebook <= 4) {
      WORD32 tbl_sign = 0;
      const UWORD16 *cb_table =
          (UWORD16 *)(ptr_aac_tables->code_book[codebook]);
      const UWORD32 *idx_table =
          (UWORD32 *)(ptr_aac_tables->index_table[codebook]);

      if (codebook > 2) {
        tbl_sign = 1;
      }

      {
        num_decoded_bits = 0;
        cur_ext_sort_cw_sec =
            ptr_hcr_info->sect_info
                .ptr_num_ext_sorted_cw_in_sect[num_ext_sorted_cw_in_sect_idx];

        arr_seg_start_l = ixheaacd_huff_dec_quad_hcr_pcw(
            ptr_hcr_info, itt_bit_buff, cur_ext_sort_cw_sec, cb_table, tbl_sign,
            idx_table, &read_word, &read_bits, arr_seg_start_l,
            p_remaining_bits_in_seg, &num_decoded_bits);

        p_remaining_bits_in_seg += cur_ext_sort_cw_sec;

        if (cur_ext_sort_cw_sec * max_allowed_cw_len < num_decoded_bits) {
          ptr_hcr_info->str_dec_io.err_log |= (ERROR_POS << 19);
        }

        if (1 ==
            ixheaacd_err_detect_pcw_segment(
                *p_remaining_bits_in_seg, ptr_hcr_info, PCW,
                ptr_quant_spec_coeff +
                    ptr_hcr_info->str_dec_io.quant_spec_coeff_idx - dimension,
                dimension)) {
          return;
        }
      }
    } else if (codebook < 11) {
      {
        WORD32 tbl_sign = 0;
        WORD32 huff_mode = 9;
        const UWORD16 *cb_table =
            (UWORD16 *)(ptr_aac_tables->code_book[codebook]);
        const UWORD32 *idx_table =
            (UWORD32 *)(ptr_aac_tables->index_table[codebook]);
        num_decoded_bits = 0;

        if (codebook > 6) {
          if (codebook > 8)
            huff_mode = 13;
          else
            huff_mode = 8;
          tbl_sign = 1;
        }

        cur_ext_sort_cw_sec =
            ptr_hcr_info->sect_info
                .ptr_num_ext_sorted_cw_in_sect[num_ext_sorted_cw_in_sect_idx];

        arr_seg_start_l = ixheaacd_huff_dec_pair_hcr_pcw(
            ptr_hcr_info, itt_bit_buff, cur_ext_sort_cw_sec, cb_table,
            &read_word, tbl_sign, idx_table, arr_seg_start_l, &read_bits,
            huff_mode, p_remaining_bits_in_seg, &num_decoded_bits);

        p_remaining_bits_in_seg += cur_ext_sort_cw_sec;

        if (cur_ext_sort_cw_sec * max_allowed_cw_len < num_decoded_bits) {
          ptr_hcr_info->str_dec_io.err_log |= (ERROR_POS << 18);
        }

        if (1 ==
            ixheaacd_err_detect_pcw_segment(
                *p_remaining_bits_in_seg, ptr_hcr_info, PCW_SIGN,
                ptr_quant_spec_coeff +
                    ptr_hcr_info->str_dec_io.quant_spec_coeff_idx - dimension,
                dimension)) {
          return;
        }
      }
    } else if ((codebook >= 11)) {
      const UWORD32 *idx_table =
          ptr_aac_tables->pstr_huffmann_tables->idx_table_hf11;
      const UWORD16 *cb_table =
          ptr_aac_tables->pstr_huffmann_tables->input_table_cb11;
      num_decoded_bits = 0;

      cur_ext_sort_cw_sec =
          ptr_hcr_info->sect_info
              .ptr_num_ext_sorted_cw_in_sect[num_ext_sorted_cw_in_sect_idx];

      arr_seg_start_l = ixheaacd_huff_dec_word_hcr_pcw(
          ptr_hcr_info, itt_bit_buff, cur_ext_sort_cw_sec, cb_table, &read_word,
          idx_table, arr_seg_start_l, &read_bits, p_remaining_bits_in_seg,
          &num_decoded_bits);

      p_remaining_bits_in_seg += cur_ext_sort_cw_sec;

      if (cur_ext_sort_cw_sec * max_allowed_cw_len < num_decoded_bits) {
        ptr_hcr_info->str_dec_io.err_log |= (ERROR_POS << 17);
      }

      if (1 == ixheaacd_err_detect_pcw_segment(
                   *p_remaining_bits_in_seg, ptr_hcr_info, PCW_ESC_SIGN,
                   ptr_quant_spec_coeff +
                       ptr_hcr_info->str_dec_io.quant_spec_coeff_idx - 2,
                   2)) {
        return;
      }
    }

    num_ext_sorted_cw_in_sect_idx++;
    if (num_ext_sorted_cw_in_sect_idx >= MAX_SFB_HCR + MAX_HCR_SETS) {
      return;
    }
  }

  num_ext_sorted_sect_in_sets_idx++;
  if (num_ext_sorted_sect_in_sets_idx >= MAX_HCR_SETS) {
    return;
  }

  itt_bit_buff->cnt_bits = read_bits;

  ptr_hcr_info->sect_info.num_ext_sorted_cw_in_sect_idx =
      num_ext_sorted_cw_in_sect_idx;
  ptr_hcr_info->sect_info.ext_sorted_cw_idx = ext_sorted_cw_idx;
  ptr_hcr_info->sect_info.num_ext_sorted_sect_in_sets_idx =
      num_ext_sorted_sect_in_sets_idx;
  ptr_hcr_info->sect_info.ext_sorted_sect_max_cb_len_idx =
      ext_sorted_sect_max_cb_len_idx;
}

static UWORD32 ixheaacd_init_segment_bit_field(WORD32 num_segment,
                                               WORD8 *p_remaining_bits_in_seg) {
  WORD16 i;
  WORD16 num_valid_segment = 0;

  for (i = 0; i < num_segment; i++) {
    if (p_remaining_bits_in_seg[i] != 0) {
      num_valid_segment += 1;
    }
  }

  return num_valid_segment;
}

UWORD8 ixheaacd_toggle_read_dir(UWORD8 read_direction) {
  if (read_direction == FROM_LEFT_TO_RIGHT) {
    return FROM_RIGHT_TO_LEFT;
  } else {
    return FROM_LEFT_TO_RIGHT;
  }
}

static PLATFORM_INLINE UWORD16 ixheaacd_huff_dec_quad_hcr_non_pcw(
    ia_bit_buf_struct *itt_bit_buff, WORD32 *spec_coef,
    const UWORD16 *code_book_tbl, WORD32 tbl_sign, const UWORD32 *idx_table) {
  WORD16 index, length;
  WORD16 cw_len;
  WORD32 read_word;

  read_word = ixheaacd_aac_showbits_32(itt_bit_buff->byte_ptr,
                                       itt_bit_buff->bit_count, NULL);
  ixheaacd_huffman_decode(read_word, &index, &length, code_book_tbl, idx_table);
  cw_len = length;
  if (tbl_sign) {
    WORD32 temp_word;
    WORD32 w, x, y, z;
    temp_word = read_word << length;
    w = index / 27;
    index = index - w * 27;
    x = index / 9;
    index = index - x * 9;
    y = index / 3;
    z = index - y * 3;
    if (w) {
      if (temp_word & 0x80000000) w = -w;
      temp_word <<= 1;
      cw_len++;
    }
    *spec_coef++ = w;

    if (x) {
      if (temp_word & 0x80000000) x = -x;
      temp_word <<= 1;
      cw_len++;
    }
    *spec_coef++ = x;
    if (y) {
      if (temp_word & 0x80000000) y = -y;
      temp_word <<= 1;
      cw_len++;
    }
    *spec_coef++ = y;
    if (z) {
      if (temp_word & 0x80000000) z = -z;
      temp_word <<= 1;
      cw_len++;
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
    *spec_coef++ = w;
    *spec_coef++ = x;
    *spec_coef++ = y;
    *spec_coef++ = z;
  }

  return cw_len;
}

static PLATFORM_INLINE UWORD16 ixheaacd_huff_dec_word_hcr_non_pcw(
    ia_bit_buf_struct *itt_bit_buff, WORD32 *spec_coef,
    const UWORD16 *code_book_tbl, const UWORD32 *idx_table) {
  WORD32 sp1, sp2;
  WORD32 flush_cw;
  WORD32 i, value, norm_val, off;
  WORD32 out1, out2;
  UWORD16 cw_len;

  WORD16 index;
  WORD32 length;

  WORD32 read_word;
  WORD32 increment;
  UWORD8 *ptr_read_next;

  read_word = ixheaacd_aac_showbits_32(itt_bit_buff->byte_ptr,
                                       itt_bit_buff->bit_count, &increment);

  ptr_read_next = itt_bit_buff->byte_ptr;
  ptr_read_next += increment;

  ixheaacd_huff_sfb_table(read_word, &index, &length, code_book_tbl, idx_table);
  cw_len = length;

  ixheaacd_aac_read_byte_corr1(&ptr_read_next, &length, &read_word, NULL);

  out1 = index / 17;
  out2 = index - out1 * 17;
  flush_cw = read_word << length;

  sp1 = out1;
  sp2 = out2;

  if (out1) {
    if (flush_cw & 0x80000000) {
      out1 = -out1;
    }
    flush_cw = (WORD32)flush_cw << 1;
    length++;
    cw_len++;
  }

  if (out2) {
    if (flush_cw & 0x80000000) {
      out2 = -out2;
    }
    length++;
    cw_len++;
  }

  ixheaacd_aac_read_byte_corr1(&ptr_read_next, &length, &read_word, NULL);

  if (sp1 == 16) {
    i = 4;
    value = ixheaac_extu(read_word, length, 23);
    value = value | 0xfffffe00;
    norm_val = ixheaac_norm32(value);

    i += (norm_val - 22);
    length += (norm_val - 21);
    cw_len += (norm_val - 21);

    ixheaacd_aac_read_byte_corr1(&ptr_read_next, &length, &read_word, NULL);

    off = ixheaac_extu(read_word, length, 32 - i);
    length += i;
    cw_len += i;

    ixheaacd_aac_read_byte_corr1(&ptr_read_next, &length, &read_word, NULL);

    i = off + ((WORD32)1 << i);

    if (out1 < 0)
      *spec_coef++ = -i;
    else
      *spec_coef++ = i;
  } else {
    *spec_coef++ = out1;
  }

  if (sp2 == 16) {
    i = 4;
    value = ixheaac_extu(read_word, length, 23);
    value = value | 0xfffffe00;
    norm_val = ixheaac_norm32(value);

    i += (norm_val - 22);
    length += (norm_val - 21);
    cw_len += (norm_val - 21);

    ixheaacd_aac_read_byte_corr1(&ptr_read_next, &length, &read_word, NULL);

    off = ixheaac_extu(read_word, length, 32 - i);
    length += i;
    cw_len += i;

    ixheaacd_aac_read_byte_corr1(&ptr_read_next, &length, &read_word, NULL);
    i = off + ((WORD32)1 << i);

    if (out2 < 0)
      *spec_coef++ = -i;
    else
      *spec_coef++ = i;
  } else {
    *spec_coef++ = out2;
  }

  return cw_len;
}

static VOID ixheaacd_decode_hcr_non_pcw(
    ia_bit_buf_struct *itt_bit_buff, ia_hcr_info_struct *ptr_hcr_info,
    ia_aac_dec_tables_struct *ptr_aac_tables, WORD32 *cw_offset, WORD32 trial,
    WORD32 start) {
  WORD16 codeword_len = 0;
  WORD8 seg_bits_left;
  UWORD8 tot_bits_to_save, code_bits_to_save, extra_code_bits;
  WORD32 segment_offset = 0;
  WORD8 *p_remaining_bits_in_seg =
      ptr_hcr_info->str_segment_info.p_remaining_bits_in_seg;
  WORD32 num_segment = ptr_hcr_info->str_segment_info.num_segment;

  for (segment_offset = start; segment_offset < trial;
       segment_offset++, *cw_offset += 1) {
    if (p_remaining_bits_in_seg[segment_offset] &&
        !ptr_hcr_info->str_segment_info.is_decoded[*cw_offset]) {
      {
        UWORD32 i_qsc;
        WORD8 current_seg_bits = p_remaining_bits_in_seg[segment_offset];

        itt_bit_buff->byte_ptr = itt_bit_buff->ptr_start;
        itt_bit_buff->valid_bits = 0;
        itt_bit_buff->byte = 0;
        itt_bit_buff->bit_count = 0;
        itt_bit_buff->write_bit_count = 0;

        if (ptr_hcr_info->str_segment_info.p_num_bits[*cw_offset]) {
          extra_code_bits = max(
              ptr_hcr_info->str_segment_info.p_num_bits[*cw_offset] - 32, 0);
          code_bits_to_save =
              min(ptr_hcr_info->str_segment_info.p_num_bits[*cw_offset], 32);

          ixheaacd_write_bit(
              itt_bit_buff,
              ptr_hcr_info->str_segment_info.code_extra[*cw_offset],
              extra_code_bits);
          ixheaacd_write_bit(itt_bit_buff,
                             ptr_hcr_info->str_segment_info.code[*cw_offset],
                             code_bits_to_save);
        }
        {
          UWORD32 bit;
          WORD32 read_bit_offset;

          if (ptr_hcr_info->str_segment_info.read_direction ==
              FROM_LEFT_TO_RIGHT) {
            read_bit_offset =
                ptr_hcr_info->str_segment_info.arr_seg_start_l[segment_offset] -
                (itt_bit_buff->size - itt_bit_buff->cnt_bits);
            if (read_bit_offset) {
              itt_bit_buff->cnt_bits += -read_bit_offset;
            }
            itt_bit_buff->ptr_read_next =
                itt_bit_buff->ptr_bit_buf_base +
                ((itt_bit_buff->size - itt_bit_buff->cnt_bits) >> 3);
            itt_bit_buff->bit_pos =
                ((itt_bit_buff->size - itt_bit_buff->cnt_bits) & 7);

            for (; p_remaining_bits_in_seg[segment_offset] > 0;
                 p_remaining_bits_in_seg[segment_offset] -= 1) {
              bit = ixheaacd_aac_read_bit_rev(itt_bit_buff);
              ptr_hcr_info->str_segment_info.arr_seg_start_l[segment_offset] +=
                  1;

              ixheaacd_write_bit(itt_bit_buff, bit, 1);
            }

          } else {
            read_bit_offset =
                ptr_hcr_info->str_segment_info.arr_seg_start_r[segment_offset] -
                (itt_bit_buff->size - itt_bit_buff->cnt_bits);
            if (read_bit_offset) {
              itt_bit_buff->cnt_bits += -read_bit_offset;
            }
            itt_bit_buff->ptr_read_next =
                itt_bit_buff->ptr_bit_buf_base +
                ((itt_bit_buff->size - itt_bit_buff->cnt_bits) >> 3);
            itt_bit_buff->bit_pos =
                ((itt_bit_buff->size - itt_bit_buff->cnt_bits) & 7);

            for (; p_remaining_bits_in_seg[segment_offset] > 0;
                 p_remaining_bits_in_seg[segment_offset] -= 1) {
              bit = ixheaacd_aac_read_bit(itt_bit_buff);
              ptr_hcr_info->str_segment_info.arr_seg_start_r[segment_offset] -=
                  1;
              ixheaacd_write_bit(itt_bit_buff, bit, 1);
            }
          }
        }

        ixheaacd_write_bit(itt_bit_buff, 0, 32 - itt_bit_buff->bit_count % 32);
        itt_bit_buff->valid_bits = 8;
        itt_bit_buff->byte_ptr = itt_bit_buff->ptr_start;
        itt_bit_buff->byte = *itt_bit_buff->ptr_start;

        if (current_seg_bits) {
          i_qsc = ptr_hcr_info->str_non_pcw_side_info
                      .res_ptr_idx[*cw_offset % num_segment];

          if (ptr_hcr_info->str_non_pcw_side_info
                  .ptr_cb[*cw_offset % num_segment] <= 4) {
            WORD32 tbl_sign = 0;
            const UWORD16 *cb_table =
                (UWORD16
                     *)(ptr_aac_tables
                            ->code_book[ptr_hcr_info->str_non_pcw_side_info
                                            .ptr_cb[*cw_offset % num_segment]]);
            const UWORD32 *idx_table =
                (UWORD32 *)(ptr_aac_tables->index_table
                                [ptr_hcr_info->str_non_pcw_side_info
                                     .ptr_cb[*cw_offset % num_segment]]);

            if (ptr_hcr_info->str_non_pcw_side_info
                    .ptr_cb[*cw_offset % num_segment] > 2) {
              tbl_sign = 1;
            }

            codeword_len = ixheaacd_huff_dec_quad_hcr_non_pcw(
                itt_bit_buff,
                &ptr_hcr_info->str_non_pcw_side_info.ptr_result_base[i_qsc],
                cb_table, tbl_sign, idx_table);

            seg_bits_left =
                current_seg_bits - codeword_len +
                ptr_hcr_info->str_segment_info.p_num_bits[*cw_offset];

          }

          else if (ptr_hcr_info->str_non_pcw_side_info
                       .ptr_cb[*cw_offset % num_segment] < 11) {
            WORD32 tbl_sign = 0;
            WORD32 huff_mode = 9;

            const UWORD16 *cb_table =
                (UWORD16
                     *)(ptr_aac_tables
                            ->code_book[ptr_hcr_info->str_non_pcw_side_info
                                            .ptr_cb[*cw_offset % num_segment]]);
            const UWORD32 *idx_table =
                (UWORD32 *)(ptr_aac_tables->index_table
                                [ptr_hcr_info->str_non_pcw_side_info
                                     .ptr_cb[*cw_offset % num_segment]]);

            if (ptr_hcr_info->str_non_pcw_side_info
                    .ptr_cb[*cw_offset % num_segment] > 6) {
              if (ptr_hcr_info->str_non_pcw_side_info
                      .ptr_cb[*cw_offset % num_segment] > 8)
                huff_mode = 13;
              else
                huff_mode = 8;
              tbl_sign = 1;
            }
            codeword_len = ixheaacd_huff_dec_pair_hcr_non_pcw(
                itt_bit_buff,
                &ptr_hcr_info->str_non_pcw_side_info.ptr_result_base[i_qsc],
                cb_table, tbl_sign, idx_table, huff_mode);

            seg_bits_left =
                current_seg_bits - codeword_len +
                ptr_hcr_info->str_segment_info.p_num_bits[*cw_offset];
          }
          if (ptr_hcr_info->str_non_pcw_side_info
                  .ptr_cb[*cw_offset % num_segment] >= 11) {
            const UWORD32 *idx_table =
                ptr_aac_tables->pstr_huffmann_tables->idx_table_hf11;
            const UWORD16 *cb_table =
                ptr_aac_tables->pstr_huffmann_tables->input_table_cb11;

            codeword_len = ixheaacd_huff_dec_word_hcr_non_pcw(
                itt_bit_buff,
                &ptr_hcr_info->str_non_pcw_side_info.ptr_result_base[i_qsc],
                cb_table, idx_table);

            seg_bits_left =
                current_seg_bits - codeword_len +
                ptr_hcr_info->str_segment_info.p_num_bits[*cw_offset];
          }
          if (seg_bits_left < 0) {
            tot_bits_to_save =
                current_seg_bits +
                ptr_hcr_info->str_segment_info.p_num_bits[*cw_offset];
            extra_code_bits = max(tot_bits_to_save - 32, 0);
            code_bits_to_save = min(tot_bits_to_save, 32);

            ptr_hcr_info->str_segment_info.code_extra[*cw_offset] =
                ixheaacd_read_bit(itt_bit_buff, extra_code_bits);
            ptr_hcr_info->str_segment_info.code[*cw_offset] =
                ixheaacd_read_bit(itt_bit_buff, code_bits_to_save);
            ptr_hcr_info->str_segment_info.p_num_bits[*cw_offset] =
                tot_bits_to_save;

            p_remaining_bits_in_seg[segment_offset] = 0;
            if (p_remaining_bits_in_seg[segment_offset] < 0)
              p_remaining_bits_in_seg[segment_offset] = 0;
          } else {
            p_remaining_bits_in_seg[segment_offset] =
                current_seg_bits -
                (codeword_len -
                 ptr_hcr_info->str_segment_info.p_num_bits[*cw_offset]);
            ptr_hcr_info->str_segment_info.p_num_bits[*cw_offset] = 0;
            ptr_hcr_info->str_segment_info.is_decoded[*cw_offset] = 1;
            if (p_remaining_bits_in_seg[segment_offset] < 0)
              p_remaining_bits_in_seg[segment_offset] = 0;
          }

          if (p_remaining_bits_in_seg[segment_offset] > 0) {
            if (ptr_hcr_info->str_segment_info.read_direction ==
                FROM_LEFT_TO_RIGHT)
              ptr_hcr_info->str_segment_info.arr_seg_start_l[segment_offset] -=
                  (p_remaining_bits_in_seg[segment_offset]);
            else
              ptr_hcr_info->str_segment_info.arr_seg_start_r[segment_offset] +=
                  (p_remaining_bits_in_seg[segment_offset]);
          }
        }
      }
    }
  }
}

VOID ixheaacd_decode_non_pcw(ia_bit_buf_struct *itt_bit_buff,
                             ia_hcr_info_struct *ptr_hcr_info,
                             ia_aac_dec_tables_struct *ptr_aac_tables) {
  UWORD32 num_valid_segment;
  WORD32 cw_offset;
  WORD32 trial;
  WORD32 num_segment;
  WORD32 num_code_word;
  UWORD8 num_set;
  UWORD8 current_set;
  WORD32 code_word_set;
  WORD32 loop1, loop2;

  num_segment = ptr_hcr_info->str_segment_info.num_segment;

  num_valid_segment = ixheaacd_init_segment_bit_field(
      num_segment, ptr_hcr_info->str_segment_info.p_remaining_bits_in_seg);

  if (num_valid_segment != 0) {
    num_code_word = ptr_hcr_info->sect_info.num_code_word;
    num_set = ((num_code_word - 1) / num_segment) + 1;

    ptr_hcr_info->str_segment_info.read_direction = FROM_RIGHT_TO_LEFT;

    for (current_set = 1; current_set < num_set; current_set++) {
      num_code_word -= num_segment;
      if (num_code_word < num_segment) {
        code_word_set = num_code_word;
      } else {
        code_word_set = num_segment;
      }

      ixheaacd_nonpcw_sideinfo_init(ptr_hcr_info);

      cw_offset = num_segment * current_set;

      ixheaacd_decode_hcr_non_pcw(itt_bit_buff, ptr_hcr_info, ptr_aac_tables,
                                  &cw_offset, code_word_set, 0);

      for (trial = 1; trial < num_segment; trial++) {
        cw_offset = num_segment * current_set;

        loop1 = min(num_segment, trial + code_word_set);
        loop2 = max(0, trial + code_word_set - num_segment);

        ixheaacd_decode_hcr_non_pcw(itt_bit_buff, ptr_hcr_info, ptr_aac_tables,
                                    &cw_offset, loop1, trial);

        ixheaacd_decode_hcr_non_pcw(itt_bit_buff, ptr_hcr_info, ptr_aac_tables,
                                    &cw_offset, loop2, 0);
      }

      ptr_hcr_info->str_segment_info.read_direction = ixheaacd_toggle_read_dir(
          ptr_hcr_info->str_segment_info.read_direction);
    }
  }
}

static VOID ixheaacd_hcr_reorder_quantized_spec_coeff(
    ia_hcr_info_struct *ptr_hcr_info,
    ia_aac_dec_channel_info_struct *ptr_aac_dec_channel_info) {
  WORD32 qsc;
  UWORD32 abs_qsc;
  UWORD32 i, j;
  UWORD16 num_spec_val_sect;
  WORD32 *ptr_teva;
  UWORD16 lav_err_cnt = 0;

  UWORD32 num_sect = ptr_hcr_info->str_dec_io.num_sect;
  WORD32 *ptr_quant_spec_coeff_base =
      ptr_hcr_info->str_dec_io.ptr_quant_spec_coeff_base;
  WORD32 *ptr_quant_spec_coeff =
      ptr_hcr_info->str_dec_io.ptr_quant_spec_coeff_base;
  const UWORD8 *ptr_cb_dim_shift_tbl =
      ptr_hcr_info->table_info.ptr_cb_dim_shift_tbl;
  const UWORD16 *ptr_lav_tbl = ptr_hcr_info->table_info.ptr_lav_tbl;
  UWORD8 *ptr_sorted_cb = ptr_hcr_info->sect_info.ptr_sorted_cb;
  UWORD16 *ptr_num_sorted_cw_in_sect =
      ptr_hcr_info->sect_info.ptr_num_sorted_cw_in_sect;
  UWORD16 *ptr_reorder_offset = ptr_hcr_info->sect_info.ptr_reorder_offset;
  WORD32 *arr_temp_values = ptr_hcr_info->str_segment_info.arr_temp_values;
  WORD32 *ptr_bak = ptr_hcr_info->str_segment_info.arr_temp_values;

  for (i = num_sect; i != 0; i--) {
    num_spec_val_sect = *ptr_num_sorted_cw_in_sect++
                        << ptr_cb_dim_shift_tbl[*ptr_sorted_cb];
    ptr_teva = &arr_temp_values[*ptr_reorder_offset++];
    for (j = num_spec_val_sect; j != 0; j--) {
      qsc = *ptr_quant_spec_coeff++;
      abs_qsc = ixheaac_abs32(qsc);
      if (abs_qsc <= ptr_lav_tbl[*ptr_sorted_cb]) {
        *ptr_teva++ = (WORD32)qsc;
      } else {
        if (abs_qsc == 8192) {
          *ptr_teva++ = (WORD32)qsc;
        } else {
          *ptr_teva++ = (WORD32)8192;
          lav_err_cnt += 1;
        }
      }
    }
    ptr_sorted_cb++;
  }

  if (ptr_aac_dec_channel_info->str_ics_info.window_sequence ==
      EIGHT_SHORT_SEQUENCE) {
    WORD32 *ptr_out;
    WORD8 window;

    ptr_bak = ptr_hcr_info->str_segment_info.arr_temp_values;
    for (window = 0; window < 8; window++) {
      ptr_out = ptr_quant_spec_coeff_base +
                (window * ptr_aac_dec_channel_info->granule_len);
      for (i = 0; i < (LINES_PER_UNIT_GROUP); i++) {
        ptr_teva = ptr_bak + (window << 2) + i * 32;
        for (j = (LINES_PER_UNIT); j != 0; j--) {
          *ptr_out++ = *ptr_teva++;
        }
      }
    }
  } else {
    ptr_quant_spec_coeff = ptr_quant_spec_coeff_base;
    for (i = 1024; i != 0; i--) {
      *ptr_quant_spec_coeff++ = *ptr_bak++;
    }
  }

  if (lav_err_cnt != 0) {
    ptr_hcr_info->str_dec_io.err_log |= (ERROR_POS << 1);
  }
}

static VOID ixheaacd_err_detect_segmentation_final(
    ia_hcr_info_struct *ptr_hcr_info) {
  UWORD8 segmentation_err_flag = 0;
  UWORD16 i;
  WORD8 *p_remaining_bits_in_seg =
      ptr_hcr_info->str_segment_info.p_remaining_bits_in_seg;
  UWORD32 num_segment = ptr_hcr_info->str_segment_info.num_segment;

  for (i = num_segment; i != 0; i--) {
    if (*p_remaining_bits_in_seg++ != 0) {
      segmentation_err_flag = 1;
    }
  }
  if (segmentation_err_flag == 1) {
    ptr_hcr_info->str_dec_io.err_log |= ERROR_POS;
  }
}

UWORD32 ixheaacd_hcr_decoder(
    ia_hcr_info_struct *ptr_hcr_info,
    ia_aac_dec_channel_info_struct *ptr_aac_dec_channel_info,
    ia_aac_dec_tables_struct *ptr_aac_tables, ia_bit_buf_struct *itt_bit_buff) {
  WORD32 ptr_tmp1, ptr_tmp2, ptr_tmp3, ptr_tmp4;
  WORD32 ptr_tmp5;

  WORD32 bit_cnt_offset;
  UWORD32 save_bit_cnt = itt_bit_buff->cnt_bits;

  ixheaacd_huff_calc_num_cwd(ptr_hcr_info);

  ixheaacd_huff_sort_sect_cb_cwd(ptr_hcr_info);

  if (ixheaacd_hcr_prepare_segmentation_grid(ptr_hcr_info) != 0)
    return (ptr_hcr_info->str_dec_io.err_log);

  ixheaacd_huff_ext_sect_info(ptr_hcr_info);

  if ((ptr_hcr_info->str_dec_io.err_log & HCR_FATAL_PCW_ERROR_MASK) != 0) {
    return (ptr_hcr_info->str_dec_io.err_log);
  }

  ixheaacd_calc_num_ext_sorted_sect_sets(
      ptr_hcr_info->str_segment_info.num_segment,
      ptr_hcr_info->sect_info.ptr_num_ext_sorted_cw_in_sect,
      ptr_hcr_info->sect_info.num_ext_sorted_cw_in_sect_idx,
      ptr_hcr_info->sect_info.ptr_num_ext_sorted_sect_in_sets,
      ptr_hcr_info->sect_info.num_ext_sorted_sect_in_sets_idx);

  ptr_tmp1 = ptr_hcr_info->sect_info.num_ext_sorted_cw_in_sect_idx;
  ptr_tmp2 = ptr_hcr_info->sect_info.ext_sorted_cw_idx;
  ptr_tmp3 = ptr_hcr_info->sect_info.num_ext_sorted_sect_in_sets_idx;
  ptr_tmp4 = ptr_hcr_info->str_dec_io.quant_spec_coeff_idx;
  ptr_tmp5 = ptr_hcr_info->sect_info.ext_sorted_sect_max_cb_len_idx;

  ixheaacd_decode_pcw(itt_bit_buff, ptr_hcr_info, ptr_aac_tables);

  if ((ptr_hcr_info->str_dec_io.err_log & HCR_FATAL_PCW_ERROR_MASK) == 0) {
    ixheaacd_decode_non_pcw(itt_bit_buff, ptr_hcr_info, ptr_aac_tables);
  }

  ixheaacd_err_detect_segmentation_final(ptr_hcr_info);

  ptr_hcr_info->sect_info.num_ext_sorted_cw_in_sect_idx = ptr_tmp1;
  ptr_hcr_info->sect_info.ext_sorted_cw_idx = ptr_tmp2;
  ptr_hcr_info->sect_info.num_ext_sorted_sect_in_sets_idx = ptr_tmp3;
  ptr_hcr_info->str_dec_io.quant_spec_coeff_idx = ptr_tmp4;
  ptr_hcr_info->sect_info.ext_sorted_sect_max_cb_len_idx = ptr_tmp5;

  ixheaacd_hcr_reorder_quantized_spec_coeff(ptr_hcr_info,
                                            ptr_aac_dec_channel_info);

  bit_cnt_offset = (WORD32)itt_bit_buff->cnt_bits - (WORD32)save_bit_cnt;
  if (bit_cnt_offset) {
    itt_bit_buff->cnt_bits += -bit_cnt_offset;
    itt_bit_buff->ptr_read_next =
        itt_bit_buff->ptr_bit_buf_base +
        ((itt_bit_buff->size - itt_bit_buff->cnt_bits) >> 3);
    itt_bit_buff->bit_pos = (itt_bit_buff->size - itt_bit_buff->cnt_bits) & 7;
  }

  return (ptr_hcr_info->str_dec_io.err_log);
}
