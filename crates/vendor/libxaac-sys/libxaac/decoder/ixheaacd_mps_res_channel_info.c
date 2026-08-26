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
#include "ixheaacd_bitbuffer.h"
#include "ixheaacd_cnst.h"
#include "ixheaacd_mps_aac_struct.h"
#include "ixheaacd_mps_res_rom.h"
#include "ixheaac_basic_op.h"
#include "ixheaacd_mps_res.h"

const WORD16 *ixheaacd_res_get_sfb_offsets(
    ia_mps_dec_residual_ics_info_struct *p_ics_info,
    ia_mps_dec_residual_aac_tables_struct *aac_tables_ptr) {
  if (p_ics_info->window_sequence != EIGHT_SHORT_SEQUENCE) {
    return aac_tables_ptr->sfb_index_long;
  } else {
    return aac_tables_ptr->sfb_index_short;
  }
}

const WORD8 *ixheaacd_res_get_sfb_width(ia_mps_dec_residual_ics_info_struct *p_ics_info,
                                        ia_mps_dec_residual_aac_tables_struct *aac_tables_ptr) {
  if (p_ics_info->window_sequence != EIGHT_SHORT_SEQUENCE) {
    return aac_tables_ptr->sfb_index_long_width;
  } else {
    return aac_tables_ptr->sfb_index_short_width;
  }
}

WORD16 ixheaacd_res_ics_read(ia_bit_buf_struct *it_bif_buf,
                             ia_mps_dec_residual_ics_info_struct *p_ics_info,
                             WORD8 tot_sf_bands_ls[2]) {
  WORD i;
  WORD mask;
  WORD tmp = 0;

  tmp = ixheaacd_read_bits_buf(it_bif_buf, 4);
  p_ics_info->window_sequence = (WORD16)((tmp & 0x6) >> 1);

  if (p_ics_info->window_sequence != EIGHT_SHORT_SEQUENCE) {
    p_ics_info->total_sf_bands = tot_sf_bands_ls[0];

    p_ics_info->window_groups = 1;
    p_ics_info->window_group_length[0] = 1;

    tmp = ixheaacd_read_bits_buf(it_bif_buf, 7);
    p_ics_info->max_sf_bands = (tmp & 0x7E) >> 1;

    if (tmp & 1) {
      return (WORD16)((WORD32)AAC_DEC_PREDICTION_NOT_SUPPORTED_IN_LC_AAC);
    }
  } else {
    WORD32 win_grp = 0, tmp2;
    p_ics_info->total_sf_bands = tot_sf_bands_ls[1];

    tmp = ixheaacd_read_bits_buf(it_bif_buf, 11);
    p_ics_info->max_sf_bands = (tmp & 0x780) >> 7;

    tmp2 = (tmp & 0x7F);

    for (i = 0; i < 7; i++) {
      mask = (1 << sub_d(6, i));
      p_ics_info->window_group_length[i] = 1;
      if (tmp2 & mask) {
        p_ics_info->window_group_length[win_grp] =
            add_d(p_ics_info->window_group_length[win_grp], 1);
      } else {
        win_grp = add_d(win_grp, 1);
      }
    }

    p_ics_info->window_group_length[7] = 1;
    p_ics_info->window_groups = add_d(win_grp, 1);
  }

  if (p_ics_info->max_sf_bands > p_ics_info->total_sf_bands)
    return (WORD16)IA_XHEAAC_DEC_EXE_NONFATAL_EXCEEDS_SFB_TRANSMITTED;

  return AAC_DEC_OK;
}
