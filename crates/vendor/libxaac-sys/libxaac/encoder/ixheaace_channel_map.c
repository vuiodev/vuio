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
#include <string.h>
#include "ixheaac_type_def.h"
#include "ixheaac_constants.h"
#include "impd_drc_common_enc.h"
#include "impd_drc_uni_drc.h"
#include "impd_drc_tables.h"
#include "impd_drc_api.h"
#include "ixheaace_api.h"
#include "ixheaac_error_standards.h"
#include "ixheaace_error_codes.h"
#include "ixheaace_aac_constants.h"
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

static IA_ERRORCODE ia_enhaacplus_enc_init_element(ixheaace_element_info *pstr_element_info,
                                                   ixheaace_element_type el_type,
                                                   WORD32 element_instance_tag) {
  IA_ERRORCODE error = IA_NO_ERROR;

  pstr_element_info->el_type = el_type;

  switch (pstr_element_info->el_type) {
    case ID_SCE:
    case ID_CCE:
      pstr_element_info->n_channels_in_el = NUM_CHANS_MONO;

      pstr_element_info->channel_index[0] = 0;

      pstr_element_info->instance_tag = element_instance_tag;
      break;

    case ID_LFE:
      pstr_element_info->n_channels_in_el = NUM_CHANS_MONO;

      pstr_element_info->channel_index[0] = 0;

      pstr_element_info->instance_tag = element_instance_tag;
      break;

    case ID_CPE:

      pstr_element_info->n_channels_in_el = NUM_CHANS_STEREO;

      pstr_element_info->channel_index[0] = 0;
      pstr_element_info->channel_index[1] = 1;

      pstr_element_info->instance_tag = element_instance_tag;
      break;

    default:
      return IA_EXHEAACE_INIT_FATAL_INVALID_ELEMENT_TYPE;
  }

  return error;
}

IA_ERRORCODE ia_enhaacplus_enc_init_element_info(WORD32 num_channels,
                                                 ixheaace_element_info *pstr_element_info,
                                                 WORD32 ele_type, WORD32 element_instance_tag) {
  IA_ERRORCODE error = IA_NO_ERROR;

  if (ele_type != ID_LFE) {
    switch (num_channels) {
      case NUM_CHANS_MONO:
        if (ele_type == -1) {
          error = ia_enhaacplus_enc_init_element(pstr_element_info, ID_SCE, element_instance_tag);
        } else {
          error =
              ia_enhaacplus_enc_init_element(pstr_element_info, ele_type, element_instance_tag);
        }
        break;

      case NUM_CHANS_STEREO:
        error = ia_enhaacplus_enc_init_element(pstr_element_info, ID_CPE, element_instance_tag);
        break;

      default:
        return IA_EXHEAACE_INIT_FATAL_NUM_CHANNELS_NOT_SUPPORTED;
    }
  } else {
    error = ia_enhaacplus_enc_init_element(pstr_element_info, ID_LFE, element_instance_tag);
  }

  return error;
}

IA_ERRORCODE ia_enhaacplus_enc_init_element_bits(ixheaace_element_bits *element_bits,
                                                 ixheaace_element_info pstr_element_info,
                                                 WORD32 bitrate_tot, WORD32 average_bits_tot,
                                                 WORD32 aot, WORD32 static_bits_tot,
                                                 WORD32 bit_res, FLAG flag_framelength_small) {
  IA_ERRORCODE error = IA_NO_ERROR;

  switch (pstr_element_info.n_channels_in_el) {
    case NUM_CHANS_MONO:
      element_bits->ch_bitrate = bitrate_tot;

      element_bits->average_bits = (average_bits_tot - static_bits_tot);
      switch (aot) {
        case AOT_AAC_LC:
        case AOT_SBR:
        case AOT_PS:
          if (flag_framelength_small) {
            element_bits->max_bits = MAXIMUM_CHANNEL_BITS_960;

            element_bits->max_bit_res_bits = MAXIMUM_CHANNEL_BITS_960 - average_bits_tot;
          } else {
            element_bits->max_bits = MAXIMUM_CHANNEL_BITS_1024;

            element_bits->max_bit_res_bits = MAXIMUM_CHANNEL_BITS_1024 - average_bits_tot;
          }
          break;

        case AOT_AAC_LD:
        case AOT_AAC_ELD:
          if (bit_res) {
            element_bits->max_bits = bit_res;
            element_bits->max_bit_res_bits = bit_res - average_bits_tot;
          } else {
            element_bits->max_bits = average_bits_tot;
            element_bits->max_bit_res_bits = 0;
          }
          break;
      }

      element_bits->max_bit_res_bits -= (element_bits[0].max_bit_res_bits % 8);
      element_bits->bit_res_level = element_bits[0].max_bit_res_bits;
      element_bits->relative_bits = 1;
      break;

    case NUM_CHANS_STEREO:
      element_bits->ch_bitrate = bitrate_tot >> 1;

      element_bits->average_bits = (average_bits_tot - static_bits_tot);
      switch (aot) {
        case AOT_AAC_LC:
        case AOT_SBR:
        case AOT_PS:
          if (flag_framelength_small) {
            element_bits->max_bits = NUM_CHANS_STEREO * MAXIMUM_CHANNEL_BITS_960;

            element_bits->max_bit_res_bits =
                NUM_CHANS_STEREO * MAXIMUM_CHANNEL_BITS_960 - average_bits_tot;
          } else {
            element_bits->max_bits = NUM_CHANS_STEREO * MAXIMUM_CHANNEL_BITS_1024;

            element_bits->max_bit_res_bits =
                NUM_CHANS_STEREO * MAXIMUM_CHANNEL_BITS_1024 - average_bits_tot;
          }
          break;

        case AOT_AAC_LD:
        case AOT_AAC_ELD:
          if (bit_res) {
            element_bits->max_bits = bit_res;
            element_bits->max_bit_res_bits = bit_res - average_bits_tot;
          } else {
            element_bits->max_bits = average_bits_tot;
            element_bits->max_bit_res_bits = 0;
          }
          break;
      }

      element_bits->max_bit_res_bits -= (element_bits[0].max_bit_res_bits % 8);

      element_bits->bit_res_level = element_bits[0].max_bit_res_bits;
      element_bits->relative_bits = 1;
      break;

    default:
      return IA_EXHEAACE_INIT_FATAL_INVALID_NUM_CHANNELS_IN_ELE;
  }
  /* Bits carried over from previous frame */
  element_bits->carry_bits = 0;

  return error;
}
