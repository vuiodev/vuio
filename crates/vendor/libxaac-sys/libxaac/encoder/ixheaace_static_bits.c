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

#include <stddef.h>
#include "ixheaac_type_def.h"
#include "ixheaace_aac_constants.h"
#include "impd_drc_common_enc.h"
#include "impd_drc_uni_drc.h"
#include "impd_drc_tables.h"
#include "impd_drc_api.h"
#include "ixheaace_api.h"
#include "ixheaace_adjust_threshold_data.h"
#include "ixheaace_psy_const.h"
#include "ixheaace_tns.h"
#include "ixheaace_tns_params.h"
#include "ixheaace_rom.h"
#include "ixheaace_common_rom.h"
#include "ixheaace_bitbuffer.h"

#include "ixheaace_dynamic_bits.h"
#include "ixheaace_qc_data.h"

#include "ixheaace_channel_map.h"
#include "ixheaace_block_switch.h"

#include "ixheaace_psy_data.h"
#include "ixheaace_interface.h"
#include "ixheaace_write_bitstream.h"

static WORD32 ia_enhaacplus_enc_count_ms_mask_bits(WORD32 sfb_cnt, WORD32 sfb_per_group,
                                                   WORD32 max_sfb_per_grp,
                                                   ixheaace_tools_info *pstr_tools_info) {
  WORD32 ms_bits = 0, sfb_off;

  switch (pstr_tools_info->ms_digest) {
    case MS_NONE:
    case MS_ALL:
      break;

    case MS_SOME:

      for (sfb_off = 0; sfb_off < sfb_cnt; sfb_off += sfb_per_group) {
        ms_bits += max_sfb_per_grp;
      }
      break;
  }

  return ms_bits;
}

static WORD32 ia_enhaacplus_enc_tns_count(ixheaace_temporal_noise_shaping_params *pstr_tns_info,
                                          WORD32 block_type) {
  WORD32 i, k;
  WORD32 tns_present;
  WORD32 num_windows;
  WORD32 count;
  WORD32 coef_bits;

  count = 0;
  num_windows = (block_type == 2 ? 8 : 1);
  tns_present = 0;

  for (i = 0; i < num_windows; i++) {
    if (pstr_tns_info->tns_active[i] == 1) {
      tns_present = 1;
    }
  }

  if (tns_present == 1) {
    for (i = 0; i < num_windows; i++) {
      count += (block_type == SHORT_WINDOW ? 1 : 2);

      if (pstr_tns_info->tns_active[i]) {
        count += (block_type == SHORT_WINDOW ? 8 : 12);

        if (pstr_tns_info->order[i]) {
          count += 2; /*coef_compression */

          if (pstr_tns_info->coef_res[i] == 4) {
            coef_bits = 3;

            for (k = 0; k < pstr_tns_info->order[i]; k++) {
              if (pstr_tns_info->coef[i * TEMPORAL_NOISE_SHAPING_MAX_ORDER_SHORT + k] > 3 ||
                  pstr_tns_info->coef[i * TEMPORAL_NOISE_SHAPING_MAX_ORDER_SHORT + k] < -4) {
                coef_bits = 4;
                break;
              }
            }
          } else {
            coef_bits = 2;

            for (k = 0; k < pstr_tns_info->order[i]; k++) {
              if (pstr_tns_info->coef[i * TEMPORAL_NOISE_SHAPING_MAX_ORDER_SHORT + k] > 1 ||
                  pstr_tns_info->coef[i * TEMPORAL_NOISE_SHAPING_MAX_ORDER_SHORT + k] < -2) {
                coef_bits = 3;
                break;
              }
            }
          }

          for (k = 0; k < pstr_tns_info->order[i]; k++) {
            count += coef_bits;
          }
        }
      }
    }
  }

  return count;
}

static WORD32 ia_enhaacplus_enc_count_tns_bits(
    ixheaace_temporal_noise_shaping_params *pstr_tns_info, WORD32 block_type) {
  return (ia_enhaacplus_enc_tns_count(pstr_tns_info, block_type));
}

WORD32 ia_enhaacplus_enc_count_static_bitdemand(
    ixheaace_psy_out_channel **psy_out_ch,
    ixheaace_psy_out_element *pstr_psy_out_element, WORD32 channels, WORD32 aot, WORD32 adts_flag,
    WORD32 stat_bits_flag, WORD32 flag_last_element) {
  WORD32 static_bits = 0;
  WORD32 ch;

  switch (channels) {
    case 1:

      static_bits += SI_ID_BITS + SI_SCE_BITS + SI_ICS_BITS;

      static_bits += ia_enhaacplus_enc_count_tns_bits(&(psy_out_ch[0]->tns_info),
                                                      psy_out_ch[0]->window_sequence);
      switch (psy_out_ch[0]->window_sequence) {
        case LONG_WINDOW:
        case START_WINDOW:
        case STOP_WINDOW:

          static_bits += SI_ICS_INFO_BITS_LONG;
          break;

        case SHORT_WINDOW:

          static_bits += SI_ICS_INFO_BITS_SHORT;
          break;
      }
      break;

    case 2:
      static_bits += SI_ID_BITS + SI_CPE_BITS + 2 * SI_ICS_BITS;

      static_bits += SI_CPE_MS_MASK_BITS;

      static_bits += ia_enhaacplus_enc_count_ms_mask_bits(
          psy_out_ch[0]->sfb_count, psy_out_ch[0]->sfb_per_group, psy_out_ch[0]->max_sfb_per_grp,
          &pstr_psy_out_element->tools_info);

      switch (psy_out_ch[0]->window_sequence) {
        case LONG_WINDOW:
        case START_WINDOW:
        case STOP_WINDOW:

          static_bits += SI_ICS_INFO_BITS_LONG;
          break;

        case SHORT_WINDOW:

          static_bits += SI_ICS_INFO_BITS_SHORT;
          break;
      }

      for (ch = 0; ch < 2; ch++) {
        static_bits += ia_enhaacplus_enc_count_tns_bits(&(psy_out_ch[ch]->tns_info),
                                                        psy_out_ch[ch]->window_sequence);
      }

      break;
  }

  if (aot == AOT_AAC_LC || aot == AOT_SBR || aot == AOT_PS) {
    if (!(adts_flag)) {
      return static_bits + stat_bits_flag * 8;
    }

    if (adts_flag && (stat_bits_flag) && (flag_last_element)) {
      return static_bits + 56;
    } else {
      return static_bits;
    }
  } else {
    return static_bits;  // Default Case
  }
}
