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
#include "ixheaacd_bitbuffer.h"
#include "ixheaacd_common_rom.h"
#include "ixheaacd_basic_funcs.h"

#include "ixheaac_basic_op.h"
#include "ixheaacd_intrinsics.h"

#include "ixheaacd_pulsedata.h"

#include "ixheaacd_pns.h"
#include "ixheaacd_drc_data_struct.h"

#include "ixheaacd_lt_predict.h"
#include "ixheaacd_cnst.h"
#include "ixheaacd_ec_defines.h"
#include "ixheaacd_ec_struct_def.h"
#include "ixheaacd_channelinfo.h"
#include "ixheaacd_channel.h"
#include "ixheaacd_drc_dec.h"
#include "ixheaacd_audioobjtypes.h"

#include "ixheaacd_stereo.h"

VOID ixheaacd_ms_stereo_process(
    ia_aac_dec_channel_info_struct *ptr_aac_dec_channel_info[CHANNELS],
    ia_aac_dec_tables_struct *ptr_aac_tables)

{
  WORD32 win_grp, grp_len, k;
  WORD32 *l_spec = ptr_aac_dec_channel_info[LEFT]->ptr_spec_coeff;
  WORD32 *r_spec = ptr_aac_dec_channel_info[RIGHT]->ptr_spec_coeff;
  WORD16 maximum_bins_short =
      (ptr_aac_dec_channel_info[LEFT]->str_ics_info.frame_length) >> 3;
  WORD8 *ptr_group_len =
      ptr_aac_dec_channel_info[LEFT]->str_ics_info.window_group_length;
  const WORD8 *ptr_sfb_width =
      ptr_aac_tables
          ->str_aac_sfb_info[ptr_aac_dec_channel_info[RIGHT]
                                 ->str_ics_info.window_sequence]
          .sfb_width;

  UWORD8 *ptr_ms_used =
      &ptr_aac_dec_channel_info[LEFT]->pstr_stereo_info->ms_used[0][0];

  for (win_grp = 0;
       win_grp < ptr_aac_dec_channel_info[LEFT]->str_ics_info.num_window_groups;
       win_grp++) {
    for (grp_len = 0; grp_len < ptr_group_len[win_grp]; grp_len++) {
      WORD32 sfb;
      WORD32 ixheaacd_drc_offset = 0;

      for (sfb = 0; sfb < ptr_aac_dec_channel_info[LEFT]->str_ics_info.max_sfb;
           sfb++) {
        ixheaacd_drc_offset += ptr_sfb_width[sfb];

        if (*ptr_ms_used++) {
          for (k = 0; k < ptr_sfb_width[sfb]; k = k + 2) {
            WORD32 left_coef = *l_spec;
            WORD32 right_coef = *r_spec;
            WORD32 left_coef2 = *(l_spec + 1);
            WORD32 right_coef2 = *(r_spec + 1);

            *l_spec++ = ixheaac_add32_sat(left_coef, right_coef);
            *r_spec++ = ixheaac_sub32_sat(left_coef, right_coef);
            *l_spec++ = ixheaac_add32_sat(left_coef2, right_coef2);
            *r_spec++ = ixheaac_sub32_sat(left_coef2, right_coef2);
          }
        } else {
          l_spec += ptr_sfb_width[sfb];
          r_spec += ptr_sfb_width[sfb];
        }
      }
      ptr_ms_used -= ptr_aac_dec_channel_info[LEFT]->str_ics_info.max_sfb;

      if (maximum_bins_short == 120) {
        l_spec = l_spec + maximum_bins_short - ixheaacd_drc_offset;
        r_spec = r_spec + maximum_bins_short - ixheaacd_drc_offset;
      } else {
        l_spec = l_spec + 128 - ixheaacd_drc_offset;
        r_spec = r_spec + 128 - ixheaacd_drc_offset;
      }
    }

    ptr_ms_used += JOINT_STEREO_MAX_BANDS;
  }
}

static PLATFORM_INLINE WORD32 ixheaacd_mult32x16in32l(WORD32 a, WORD32 b) {
  WORD32 result;
  WORD64 temp_result;

  temp_result = (WORD64)a * (WORD64)b;

  result = (WORD32)(temp_result >> 16);

  return (result);
}

VOID ixheaacd_intensity_stereo_process(
    ia_aac_dec_channel_info_struct *ptr_aac_dec_channel_info[CHANNELS],
    ia_aac_dec_tables_struct *ptr_aac_tables, WORD32 object_type, WORD32 aac_sf_data_resil_flag,
    WORD16 framelength) {
  UWORD8 *ptr_ms_used =
      &ptr_aac_dec_channel_info[LEFT]->pstr_stereo_info->ms_used[0][0];
  WORD8 *ptr_code_book = &ptr_aac_dec_channel_info[RIGHT]->ptr_code_book[0];
  WORD16 *ptr_scale_factor =
      &ptr_aac_dec_channel_info[RIGHT]->ptr_scale_factor[0];
  WORD32 *r_spec = &ptr_aac_dec_channel_info[RIGHT]->ptr_spec_coeff[0];
  WORD32 *l_spec = &ptr_aac_dec_channel_info[LEFT]->ptr_spec_coeff[0];
  WORD16 maximum_bins_short =
      (ptr_aac_dec_channel_info[LEFT]->str_ics_info.frame_length) >> 3;
  WORD8 *ptr_group_len =
      ptr_aac_dec_channel_info[RIGHT]->str_ics_info.window_group_length;
  const WORD8 *ptr_sfb_width =
      ptr_aac_tables
          ->str_aac_sfb_info[ptr_aac_dec_channel_info[RIGHT]
                                 ->str_ics_info.window_sequence]
          .sfb_width;

  WORD32 *ptr_scale_table;

  if (960 == framelength)
    ptr_scale_table = ptr_aac_tables->pstr_block_tables->scale_table_960;
  else
    ptr_scale_table = ptr_aac_tables->pstr_block_tables->scale_table;

  WORD32 win_grp, grp_len, k;

  for (win_grp = 0;
       win_grp <
       ptr_aac_dec_channel_info[RIGHT]->str_ics_info.num_window_groups;
       win_grp++) {
    for (grp_len = 0; grp_len < ptr_group_len[win_grp]; grp_len++) {
      WORD32 sfb;
      WORD32 ixheaacd_drc_offset = 0;

      for (sfb = 0; sfb < ptr_aac_dec_channel_info[RIGHT]->str_ics_info.max_sfb;
           sfb++) {
        WORD8 code_book = ptr_code_book[sfb];
        ixheaacd_drc_offset += ptr_sfb_width[sfb];

        if (((code_book >= INTENSITY_HCB2) &&
             ((object_type != AOT_ER_AAC_ELD) &&
              (object_type != AOT_ER_AAC_LD))) ||
            (((code_book == INTENSITY_HCB2) || (code_book == INTENSITY_HCB)) &&
             ((object_type == AOT_ER_AAC_ELD) ||
              (object_type == AOT_ER_AAC_LD))))

        {
          WORD32 sfb_factor, scale;
          WORD32 scf_exp;

          sfb_factor = (ptr_scale_factor[sfb]);
          if (aac_sf_data_resil_flag) sfb_factor = -sfb_factor;

          scf_exp = (sfb_factor >> 2);
          scale = *(ptr_scale_table + (sfb_factor & 3));
          if (!((ptr_ms_used[sfb]) ^ (code_book & 0x1))) {
            scale = ixheaac_negate32(scale);
          }

          scf_exp = -(scf_exp + 2);

          for (k = 0; k < ptr_sfb_width[sfb]; k++) {
            WORD32 temp, shift_val;
            temp = *l_spec++;

            shift_val = ixheaac_norm32(temp);
            temp = ixheaac_shl32(temp, shift_val);

            temp = ixheaacd_mult32x16in32l(temp, scale);
            shift_val = shift_val + scf_exp;

            if (shift_val < 0) {
              if (shift_val < -31) {
                shift_val = -31;
              }
              temp = ixheaac_shl32_sat(temp, -shift_val);
            } else {
              if (shift_val > 31) {
                shift_val = 31;
              }
              temp = ixheaac_shr32(temp, shift_val);
            }
            *r_spec++ = temp;
          }

        } else {
          l_spec += ptr_sfb_width[sfb];
          r_spec += ptr_sfb_width[sfb];
        }
      }

      if (maximum_bins_short == 120) {
        l_spec += maximum_bins_short - ixheaacd_drc_offset;
        r_spec += maximum_bins_short - ixheaacd_drc_offset;
      } else {
        l_spec += 128 - ixheaacd_drc_offset;
        r_spec += 128 - ixheaacd_drc_offset;
      }
    }
    ptr_ms_used += 64;
    ptr_code_book += 16;
    ptr_scale_factor += 16;
  }
}
