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

#include "ixheaac_error_standards.h"
#include "ixheaace_error_codes.h"
#include <string.h>
#include "ixheaac_type_def.h"
#include "ixheaac_constants.h"
#include "impd_drc_common_enc.h"
#include "impd_drc_uni_drc.h"
#include "impd_drc_tables.h"
#include "impd_drc_api.h"
#include "ixheaace_api.h"
#include "ixheaace_aac_constants.h"
#include "ixheaace_psy_const.h"
#include "ixheaace_tns.h"
#include "ixheaace_tns_params.h"
#include "ixheaace_rom.h"
#include "ixheaace_common_rom.h"
#include "ixheaace_bitbuffer.h"

IA_ERRORCODE
ia_enhaacplus_enc_get_tns_param(
    ixheaace_temporal_noise_shaping_config_tabulated *pstr_tns_config_tab, WORD32 bit_rate,
    WORD32 channels, WORD32 block_type,
    const ixheaace_temporal_noise_shaping_info_tab *pstr_tns_info_tab, WORD32 size) {
  UWORD32 i;

  if (pstr_tns_config_tab == NULL) {
    return IA_EXHEAACE_INIT_FATAL_TNS_CONFIG_INIT_FAILED;
  }

  pstr_tns_config_tab->thresh_on = -1;

  for (i = 0; i < size / sizeof(ixheaace_temporal_noise_shaping_info_tab); i++) {
    if ((bit_rate >= pstr_tns_info_tab[i].bit_rate_from) &&
        bit_rate <= pstr_tns_info_tab[i].bit_rate_to) {
      switch (block_type) {
        case LONG_WINDOW:
          switch (channels) {
            case NUM_CHANS_MONO:

              pstr_tns_config_tab->thresh_on = pstr_tns_info_tab[i].param_mono_long.thresh_on;
              pstr_tns_config_tab->lpc_start_freq =
                  pstr_tns_info_tab[i].param_mono_long.lpc_start_freq;
              pstr_tns_config_tab->lpc_stop_freq =
                  pstr_tns_info_tab[i].param_mono_long.lpc_stop_freq;
              pstr_tns_config_tab->tns_time_resolution =
                  pstr_tns_info_tab[i].param_mono_long.tns_time_resolution;

              break;
            case NUM_CHANS_STEREO:

              pstr_tns_config_tab->thresh_on = pstr_tns_info_tab[i].param_stereo_long.thresh_on;
              pstr_tns_config_tab->lpc_start_freq =
                  pstr_tns_info_tab[i].param_stereo_long.lpc_start_freq;
              pstr_tns_config_tab->lpc_stop_freq =
                  pstr_tns_info_tab[i].param_stereo_long.lpc_stop_freq;
              pstr_tns_config_tab->tns_time_resolution =
                  pstr_tns_info_tab[i].param_stereo_long.tns_time_resolution;

              break;
          }
          break;

        case SHORT_WINDOW:
          switch (channels) {
            case NUM_CHANS_MONO:

              pstr_tns_config_tab->thresh_on = pstr_tns_info_tab[i].param_mono_short.thresh_on;
              pstr_tns_config_tab->lpc_start_freq =
                  pstr_tns_info_tab[i].param_mono_short.lpc_start_freq;
              pstr_tns_config_tab->lpc_stop_freq =
                  pstr_tns_info_tab[i].param_mono_short.lpc_stop_freq;
              pstr_tns_config_tab->tns_time_resolution =
                  pstr_tns_info_tab[i].param_mono_short.tns_time_resolution;
              break;
            case NUM_CHANS_STEREO:

              pstr_tns_config_tab->thresh_on = pstr_tns_info_tab[i].param_stereo_short.thresh_on;
              pstr_tns_config_tab->lpc_start_freq =
                  pstr_tns_info_tab[i].param_stereo_short.lpc_start_freq;
              pstr_tns_config_tab->lpc_stop_freq =
                  pstr_tns_info_tab[i].param_stereo_short.lpc_stop_freq;
              pstr_tns_config_tab->tns_time_resolution =
                  pstr_tns_info_tab[i].param_stereo_short.tns_time_resolution;
              break;
          }

          break;
      }
    }
  }

  // This check is not being done now
  if (pstr_tns_config_tab->thresh_on == -1) {
    return IA_EXHEAACE_INIT_FATAL_INVALID_TNS_PARAM;
  }

  return IA_NO_ERROR;
}

VOID ia_enhaacplus_enc_get_tns_max_bands(
    WORD32 sampling_rate, WORD32 block_type, WORD32 *tns_max_sfb,
    const ixheaace_temporal_noise_shaping_max_table *pstr_tns_max_bands_tab, WORD32 size,
    WORD32 aot, WORD32 frame_length) {
  UWORD32 i;

  *tns_max_sfb = -1;

  for (i = 0; i < size / sizeof(ixheaace_temporal_noise_shaping_max_table); i++) {
    if (sampling_rate == pstr_tns_max_bands_tab[i].sampling_rate) {
      if (aot == AOT_AAC_LC || aot == AOT_SBR || aot == AOT_PS) {
        if (block_type == SHORT_WINDOW) {
          *tns_max_sfb = (frame_length == (FRAME_LEN_SHORT_128))
                             ? pstr_tns_max_bands_tab[i].max_band_1024_short_lc
                             : pstr_tns_max_bands_tab[i].max_band_960_short_lc;
        } else {
          *tns_max_sfb = (frame_length == FRAME_LEN_1024)
                             ? pstr_tns_max_bands_tab[i].max_band_1024_long_lc
                             : pstr_tns_max_bands_tab[i].max_band_960_long_lc;
        }
      } else if (aot == AOT_AAC_LD || aot == AOT_AAC_ELD) {
        *tns_max_sfb = (frame_length == FRAME_LEN_512)
                           ? pstr_tns_max_bands_tab[i].max_band_512_ld
                           : pstr_tns_max_bands_tab[i].max_band_480_ld;
      }
      break;
    }
  }
}
