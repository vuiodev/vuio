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
#include "iusace_type_def.h"
#include "iusace_bitbuffer.h"
/* DRC */
#include "impd_drc_common_enc.h"
#include "impd_drc_uni_drc.h"
#include "impd_drc_tables.h"
#include "impd_drc_api.h"
#include "impd_drc_uni_drc_eq.h"
#include "impd_drc_uni_drc_filter_bank.h"
#include "impd_drc_gain_enc.h"
#include "impd_drc_struct_def.h"
#include "iusace_cnst.h"

#include "ixheaace_api.h"
#include "iusace_tns_usac.h"
#include "iusace_psy_mod.h"
#include "ixheaace_sbr_header.h"
#include "ixheaace_config.h"
#include "iusace_config.h"
#include "ixheaace_asc_write.h"
#include "iusace_block_switch_const.h"
#include "iusace_rom.h"
#include "ixheaac_constants.h"
#include "ixheaace_aac_constants.h"
#include "ixheaace_sbr_def.h"
#include "ixheaace_common_rom.h"
#include "ixheaace_error_codes.h"

static WORD32 ixheaace_spatial_specific_config(ia_bit_buf_struct *pstr_it_bit_buff,
                                               ia_aace_config_struct *pstr_eld_config) {
  WORD32 bit_cnt = 0, cnt = 0;
  WORD32 num_bytes = pstr_eld_config->num_sac_cfg_bits >> 3;
  for (cnt = 0; cnt < num_bytes; cnt++) {
    bit_cnt += iusace_write_bits_buf(pstr_it_bit_buff, (pstr_eld_config->sac_cfg_data[cnt]), 8);
  }
  if (pstr_eld_config->num_sac_cfg_bits & 0x7) {
    bit_cnt += iusace_write_bits_buf(pstr_it_bit_buff, (pstr_eld_config->sac_cfg_data[cnt]),
                                     (pstr_eld_config->num_sac_cfg_bits & 0x7));
  }

  return bit_cnt;
}
static WORD32 iusace_config_extension(ia_bit_buf_struct *pstr_it_bit_buff,
                                      ia_usac_config_struct *pstr_usac_config) {
  WORD32 bit_cnt = 0;
  UWORD32 i, j;
  UWORD32 fill_byte_val = 0xa5;

  bit_cnt += iusace_write_escape_value(pstr_it_bit_buff,
                                       pstr_usac_config->num_config_extensions - 1, 2, 4, 8);

  for (j = 0; j < pstr_usac_config->num_config_extensions; j++) {
    bit_cnt += iusace_write_escape_value(pstr_it_bit_buff,
                                         pstr_usac_config->usac_config_ext_type[j], 4, 8, 16);

    bit_cnt += iusace_write_escape_value(pstr_it_bit_buff,
                                         pstr_usac_config->usac_config_ext_len[j], 4, 8, 16);

    switch (pstr_usac_config->usac_config_ext_type[j]) {
      case ID_CONFIG_EXT_FILL:
        for (i = 0; i < pstr_usac_config->usac_config_ext_len[j]; i++) {
          bit_cnt += iusace_write_bits_buf(pstr_it_bit_buff, fill_byte_val, 8);
        }
        break;
      case ID_CONFIG_EXT_LOUDNESS_INFO:
        for (i = 0; i < pstr_usac_config->usac_config_ext_len[j]; i++) {
          bit_cnt += iusace_write_bits_buf(pstr_it_bit_buff,
                                           pstr_usac_config->usac_config_ext_buf[j][i], 8);
        }
        break;
      case ID_CONFIG_EXT_STREAM_ID:
        bit_cnt +=
            iusace_write_bits_buf(pstr_it_bit_buff, pstr_usac_config->stream_identifier, 16);
        break;
      default:
        for (i = 0; i < pstr_usac_config->usac_config_ext_len[j]; i++) {
          bit_cnt += iusace_write_bits_buf(
              pstr_it_bit_buff, (UWORD32)pstr_usac_config->usac_cfg_ext_info_buf[j][i], 8);
        }
        break;
    }
  }
  return bit_cnt;
}

static WORD32 iusace_sbr_config(ia_bit_buf_struct *pstr_it_bit_buff,
                                ia_usac_enc_sbr_config_struct *pstr_usac_sbr_config) {
  WORD32 bit_cnt = 0;

  bit_cnt += iusace_write_bits_buf(pstr_it_bit_buff, (pstr_usac_sbr_config->harmonic_sbr), 1);
  bit_cnt += iusace_write_bits_buf(pstr_it_bit_buff, (pstr_usac_sbr_config->bs_inter_tes), 1);
  bit_cnt += iusace_write_bits_buf(pstr_it_bit_buff, (pstr_usac_sbr_config->bs_pvc), 1);
  bit_cnt += iusace_write_bits_buf(pstr_it_bit_buff, (pstr_usac_sbr_config->dflt_start_freq), 4);
  bit_cnt += iusace_write_bits_buf(pstr_it_bit_buff, (pstr_usac_sbr_config->dflt_stop_freq), 4);
  bit_cnt +=
      iusace_write_bits_buf(pstr_it_bit_buff, (pstr_usac_sbr_config->dflt_header_extra1), 1);
  bit_cnt +=
      iusace_write_bits_buf(pstr_it_bit_buff, (pstr_usac_sbr_config->dflt_header_extra2), 1);

  if (pstr_usac_sbr_config->dflt_header_extra1) {
    bit_cnt +=
        iusace_write_bits_buf(pstr_it_bit_buff, (pstr_usac_sbr_config->dflt_freq_scale), 2);
    bit_cnt +=
        iusace_write_bits_buf(pstr_it_bit_buff, (pstr_usac_sbr_config->dflt_alter_scale), 2);
    bit_cnt +=
        iusace_write_bits_buf(pstr_it_bit_buff, (pstr_usac_sbr_config->dflt_noise_bands), 2);
  }

  if (pstr_usac_sbr_config->dflt_header_extra2) {
    bit_cnt +=
        iusace_write_bits_buf(pstr_it_bit_buff, (pstr_usac_sbr_config->dflt_limiter_bands), 2);
    bit_cnt +=
        iusace_write_bits_buf(pstr_it_bit_buff, (pstr_usac_sbr_config->dflt_limiter_gains), 2);
    bit_cnt +=
        iusace_write_bits_buf(pstr_it_bit_buff, (pstr_usac_sbr_config->dflt_interpol_freq), 1);
    bit_cnt +=
        iusace_write_bits_buf(pstr_it_bit_buff, (pstr_usac_sbr_config->dflt_smoothing_mode), 1);
  }

  return bit_cnt;
}

static WORD32 iusace_cpe_config(ia_bit_buf_struct *pstr_it_bit_buff,
                                ia_usac_enc_element_config_struct *pstr_usac_enc_conf,
                                WORD32 sbr_ratio_idx, ia_aace_config_struct *pstr_eld_config) {
  WORD32 bit_count = 0;

  if (sbr_ratio_idx > 0) {
    bit_count += iusace_sbr_config(pstr_it_bit_buff, &(pstr_usac_enc_conf->str_usac_sbr_config));
    bit_count +=
        iusace_write_bits_buf(pstr_it_bit_buff, (pstr_usac_enc_conf->stereo_config_index), 2);
  }

  if (pstr_usac_enc_conf->stereo_config_index > 0) {
    if (pstr_eld_config->num_sac_cfg_bits) {
      {
        bit_count += ixheaace_spatial_specific_config(pstr_it_bit_buff, pstr_eld_config);
      }
    }
  }

  return bit_count;
}
static WORD32 iusace_ext_element_config(ia_bit_buf_struct *pstr_it_bit_buff,
                                        ia_usac_enc_element_config_struct *pstr_usac_enc_conf) {
  WORD32 bit_count = 0;

  bit_count += iusace_write_escape_value(pstr_it_bit_buff, pstr_usac_enc_conf->usac_ext_ele_type,
                                         4, 8, 16);
  bit_count += iusace_write_escape_value(pstr_it_bit_buff,
                                         pstr_usac_enc_conf->usac_ext_ele_cfg_len, 4, 8, 16);
  bit_count += iusace_write_bits_buf(pstr_it_bit_buff,
                                     (pstr_usac_enc_conf->usac_ext_ele_dflt_len_present), 1);

  if (pstr_usac_enc_conf->usac_ext_ele_dflt_len_present) {
    bit_count += iusace_write_escape_value(
        pstr_it_bit_buff, pstr_usac_enc_conf->usac_ext_ele_dflt_len - 1, 8, 16, 0);
  }
  bit_count += iusace_write_bits_buf(pstr_it_bit_buff,
                                     (pstr_usac_enc_conf->usac_ext_ele_payload_present), 1);

  switch (pstr_usac_enc_conf->usac_ext_ele_type) {
    case ID_EXT_ELE_FILL:
      break;
    case ID_EXT_ELE_UNI_DRC: {
      UWORD32 i;
      for (i = 0; i < pstr_usac_enc_conf->usac_ext_ele_cfg_len; i++) {
        bit_count +=
            iusace_write_bits_buf(pstr_it_bit_buff, pstr_usac_enc_conf->drc_config_data[i], 8);
      }
    } break;
    default:
      break;
  }

  return bit_count;
}

static WORD32 iusace_encoder_config(ia_bit_buf_struct *pstr_it_bit_buff,
                                    ia_usac_config_struct *pstr_usac_cfg, WORD32 sbr_ratio_idx,
                                    ia_aace_config_struct *pstr_eld_config) {
  WORD32 bit_cnt = 0;
  UWORD32 elem_idx = 0;
  ia_usac_enc_element_config_struct *pstr_usac_enc_conf;

  bit_cnt +=
      iusace_write_escape_value(pstr_it_bit_buff, pstr_usac_cfg->num_elements - 1, 4, 8, 16);

  for (elem_idx = 0; elem_idx < pstr_usac_cfg->num_elements; elem_idx++) {
    unsigned long tmp = pstr_usac_cfg->usac_element_type[elem_idx];
    pstr_usac_enc_conf = &pstr_usac_cfg->str_usac_element_config[elem_idx];
    bit_cnt += iusace_write_bits_buf(pstr_it_bit_buff, tmp, 2);

    switch (pstr_usac_cfg->usac_element_type[elem_idx]) {
      case ID_USAC_SCE:
        bit_cnt += iusace_write_bits_buf(
            pstr_it_bit_buff, (pstr_usac_enc_conf->tw_mdct),
            1);  // For extended HE AAC profile tw_mdct shall be encoded with 0.
        bit_cnt +=
            iusace_write_bits_buf(pstr_it_bit_buff, (pstr_usac_enc_conf->noise_filling), 1);
        if (sbr_ratio_idx > 0) {
          bit_cnt +=
              iusace_sbr_config(pstr_it_bit_buff, &(pstr_usac_enc_conf->str_usac_sbr_config));
        }
        break;
      case ID_USAC_CPE:
        bit_cnt += iusace_write_bits_buf(
            pstr_it_bit_buff, (pstr_usac_enc_conf->tw_mdct),
            1);  // For extended HE AAC profile tw_mdct shall be encoded with 0.
        bit_cnt +=
            iusace_write_bits_buf(pstr_it_bit_buff, (pstr_usac_enc_conf->noise_filling), 1);
        bit_cnt += iusace_cpe_config(pstr_it_bit_buff, pstr_usac_enc_conf, sbr_ratio_idx,
                                     pstr_eld_config);
        break;
      case ID_USAC_EXT:
        bit_cnt += iusace_ext_element_config(pstr_it_bit_buff, pstr_usac_enc_conf);
        break;
      default:
        return IA_EXHEAACE_INIT_FATAL_USAC_INVALID_ELEMENT_TYPE;
        break;
    }
  }

  return bit_cnt;
}

static UWORD32 ixheaace_sbr_ratio(UWORD32 core_sbr_framelength_idx) {
  UWORD32 sbr_ratio_index = 0x0FF;

  switch (core_sbr_framelength_idx) {
    case 0:
    case 1:
      sbr_ratio_index = USAC_SBR_RATIO_NO_SBR;
      break;
    case 2:
      sbr_ratio_index = USAC_SBR_RATIO_INDEX_8_3;
      break;
    case 3:
      sbr_ratio_index = USAC_SBR_RATIO_INDEX_2_1;
      break;
    case 4:
      sbr_ratio_index = USAC_SBR_RATIO_INDEX_4_1;
      break;
  }

  return sbr_ratio_index;
}
static WORD32 sbr_header(ia_bit_buf_struct *pstr_it_bit_buff,
                         ixheaace_pstr_sbr_hdr_data pstr_sbr_config) {
  WORD32 bit_cnt = 0;
  bit_cnt += iusace_write_bits_buf(pstr_it_bit_buff, (pstr_sbr_config->sbr_amp_res), 1);
  bit_cnt += iusace_write_bits_buf(pstr_it_bit_buff, (pstr_sbr_config->sbr_start_freq), 4);
  bit_cnt += iusace_write_bits_buf(pstr_it_bit_buff, (pstr_sbr_config->sbr_stop_freq), 4);
  bit_cnt += iusace_write_bits_buf(pstr_it_bit_buff, (pstr_sbr_config->sbr_xover_band), 3);
  bit_cnt += iusace_write_bits_buf(pstr_it_bit_buff, (0), 2);  // reserved bits
  bit_cnt += iusace_write_bits_buf(pstr_it_bit_buff, (pstr_sbr_config->header_extra_1), 1);
  bit_cnt += iusace_write_bits_buf(pstr_it_bit_buff, (pstr_sbr_config->header_extra_2), 1);
  if (pstr_sbr_config->header_extra_1) {
    bit_cnt += iusace_write_bits_buf(pstr_it_bit_buff, (pstr_sbr_config->freq_scale), 2);
    bit_cnt += iusace_write_bits_buf(pstr_it_bit_buff, (pstr_sbr_config->alter_scale), 1);
    bit_cnt += iusace_write_bits_buf(pstr_it_bit_buff, (pstr_sbr_config->sbr_noise_bands), 2);
  }

  if (pstr_sbr_config->header_extra_2) {
    bit_cnt += iusace_write_bits_buf(pstr_it_bit_buff, (pstr_sbr_config->sbr_limiter_bands), 2);
    bit_cnt += iusace_write_bits_buf(pstr_it_bit_buff, (pstr_sbr_config->sbr_limiter_gains), 2);
    bit_cnt += iusace_write_bits_buf(pstr_it_bit_buff, (pstr_sbr_config->sbr_interpol_freq), 1);
    bit_cnt +=
        iusace_write_bits_buf(pstr_it_bit_buff, (pstr_sbr_config->sbr_smoothing_length), 1);
  }
  return bit_cnt;
}

static WORD32 ld_sbr_header(ia_bit_buf_struct *pstr_it_bit_buff,
                            ixheaace_pstr_sbr_hdr_data pstr_sbr_config,
                            WORD32 channel_configuration) {
  WORD32 num_sbr_header, el, bit_cnt = 0;
  switch (channel_configuration) {
    case 1:
    case 2:
      num_sbr_header = 1;
      break;
    case 3:
      num_sbr_header = 2;
      break;
    case 4:
    case 5:
    case 6:
      num_sbr_header = 3;
      break;
    case 7:
      num_sbr_header = 4;
      break;
    default:
      num_sbr_header = 0;
      break;
  }
  for (el = 0; el < num_sbr_header; el++) {
    bit_cnt += sbr_header(pstr_it_bit_buff, pstr_sbr_config);
  }
  return bit_cnt;
}

static WORD32 iaace_get_eld_specific_config_bytes(ia_bit_buf_struct *pstr_it_bit_buff,
                                                  ia_aace_config_struct *pstr_eld_config,
                                                  WORD32 channel_configuration) {
  WORD32 bit_cnt = 0;
  bit_cnt += iusace_write_bits_buf(pstr_it_bit_buff, (pstr_eld_config->frame_length_flag), 1);
  bit_cnt +=
      iusace_write_bits_buf(pstr_it_bit_buff, (pstr_eld_config->aac_sec_data_resilience_flag), 1);
  bit_cnt +=
      iusace_write_bits_buf(pstr_it_bit_buff, (pstr_eld_config->aac_sf_data_resilience_flag), 1);
  bit_cnt += iusace_write_bits_buf(pstr_it_bit_buff,
                                   (pstr_eld_config->aac_spec_data_resilience_flag), 1);
  bit_cnt += iusace_write_bits_buf(pstr_it_bit_buff, (pstr_eld_config->ld_sbr_present_flag), 1);

  if (pstr_eld_config->ld_sbr_present_flag) {
    bit_cnt += iusace_write_bits_buf(pstr_it_bit_buff, (pstr_eld_config->ld_sbr_sample_rate), 1);
    bit_cnt += iusace_write_bits_buf(pstr_it_bit_buff, (pstr_eld_config->ld_sbr_crc_flag), 1);
    bit_cnt +=
        ld_sbr_header(pstr_it_bit_buff, pstr_eld_config->sbr_config, channel_configuration);
  }

  if (pstr_eld_config->num_sac_cfg_bits) {
    bit_cnt += iusace_write_bits_buf(pstr_it_bit_buff, (pstr_eld_config->eld_ext_type[0]), 4);
    bit_cnt +=
        iusace_write_escape_value(pstr_it_bit_buff, (pstr_eld_config->eld_ext_len[0]), 4, 8, 16);
    if (IAAC_ELDEXT_LDSAC == pstr_eld_config->eld_ext_type[0]) {
      bit_cnt += ixheaace_spatial_specific_config(pstr_it_bit_buff, pstr_eld_config);
    }
  }
  bit_cnt += iusace_write_bits_buf(pstr_it_bit_buff, (IAAC_ELDEXT_TERM), 4);

  return bit_cnt;
}

static WORD32 iaace_ga_specific_config_bytes(ia_bit_buf_struct *pstr_it_bit_buff,
                                             ia_aace_config_struct *pstr_ga_specific_config,
                                             WORD32 channel_configuration, WORD32 aot) {
  WORD32 bit_cnt = 0;
  bit_cnt +=
      iusace_write_bits_buf(pstr_it_bit_buff, (pstr_ga_specific_config->frame_length_flag), 1);
  bit_cnt += iusace_write_bits_buf(pstr_it_bit_buff,
                                   (pstr_ga_specific_config->depends_on_core_coder), 1);
  if (pstr_ga_specific_config->depends_on_core_coder) {
    bit_cnt +=
        iusace_write_bits_buf(pstr_it_bit_buff, (pstr_ga_specific_config->core_coder_delay), 14);
  }
  if (!channel_configuration) {
  }

  if (AOT_AAC_LD == aot) {
    pstr_ga_specific_config->extension_flag = 1;
  }
  if (AOT_AAC_LC == aot) {
    pstr_ga_specific_config->extension_flag = 0;
  }
  bit_cnt +=
      iusace_write_bits_buf(pstr_it_bit_buff, (pstr_ga_specific_config->extension_flag), 1);

  if ((pstr_ga_specific_config->extension_flag) && ((AOT_AAC_LD == aot))) {
    bit_cnt += iusace_write_bits_buf(pstr_it_bit_buff,
                                     (pstr_ga_specific_config->aac_sec_data_resilience_flag), 1);
    bit_cnt += iusace_write_bits_buf(pstr_it_bit_buff,
                                     (pstr_ga_specific_config->aac_sf_data_resilience_flag), 1);
    bit_cnt += iusace_write_bits_buf(pstr_it_bit_buff,
                                     (pstr_ga_specific_config->aac_spec_data_resilience_flag), 1);

    // extension flag 3
    bit_cnt += iusace_write_bits_buf(pstr_it_bit_buff, 0, 1);
  }
  return bit_cnt;
}

WORD32 ixheaace_get_audiospecific_config_bytes(
    ia_bit_buf_struct *pstr_it_bit_buff,
    ixheaace_audio_specific_config_struct *pstr_audio_specific_config, WORD32 aot,
    WORD32 ccfl_idx) {
  WORD32 bit_cnt = 0, i;
  UWORD32 tmp = 0x0f;  // initialized to indicate no sampling frequency index field
  WORD32 ext_aot = -1;
  WORD32 ext_id = 0;
  WORD32 sbr_present_flag = 0, ps_present_flag = 0;

  if (((AOT_AAC_ELD == aot) &&
       (1 == pstr_audio_specific_config->str_aac_config.ld_sbr_sample_rate)) ||
      (AOT_SBR == aot) || (AOT_PS == aot)) {
    // dual rate
    pstr_audio_specific_config->ext_sampling_frequency =
        pstr_audio_specific_config->sampling_frequency;
    pstr_audio_specific_config->sampling_frequency /= 2;

    if ((AOT_SBR == aot) || (AOT_PS == aot)) {
      aot = AOT_AAC_LC;
    }
  }
  pstr_audio_specific_config->audio_object_type = aot;
  if (aot == AOT_USAC) {
    for (i = 0; i < sizeof(iusace_sampl_freq_idx_table) / sizeof(iusace_sampl_freq_idx_table[0]);
         i++) {
      if (pstr_audio_specific_config->sampling_frequency == iusace_sampl_freq_idx_table[i]) {
        tmp = i;
        break;
      }
    }
  } else {
    for (i = 0; i < sizeof(ia_sampl_freq_table) / sizeof(ia_sampl_freq_table[0]); i++) {
      if (ia_sampl_freq_table[i] == pstr_audio_specific_config->sampling_frequency) {
        tmp = i;
        break;
      }
    }
  }

  pstr_audio_specific_config->samp_freq_index = (UWORD32)tmp;

  // Write Audio Object Type
  if (pstr_audio_specific_config->audio_object_type > 31) {
    tmp = pstr_audio_specific_config->audio_object_type - 32;
    pstr_audio_specific_config->audio_object_type = 31;
  }
  bit_cnt +=
      iusace_write_bits_buf(pstr_it_bit_buff, (pstr_audio_specific_config->audio_object_type), 5);
  if (pstr_audio_specific_config->audio_object_type == 31) {
    bit_cnt += iusace_write_bits_buf(pstr_it_bit_buff, tmp, 6);
    pstr_audio_specific_config->audio_object_type = tmp + 32;
  }

  // Write Audio Object Type
  bit_cnt +=
      iusace_write_bits_buf(pstr_it_bit_buff, (pstr_audio_specific_config->samp_freq_index), 4);
  if (pstr_audio_specific_config->samp_freq_index == 0xf) {
    bit_cnt += iusace_write_bits_buf(pstr_it_bit_buff,
                                     (pstr_audio_specific_config->sampling_frequency), 24);
  } else if (AOT_USAC != aot) {
    pstr_audio_specific_config->sampling_frequency =
        ia_sampl_freq_table[pstr_audio_specific_config->samp_freq_index];
  }
  bit_cnt += iusace_write_bits_buf(pstr_it_bit_buff,
                                   (pstr_audio_specific_config->channel_configuration), 4);

  if ((AOT_SBR == aot) || (AOT_PS == aot)) {
    ext_aot = AOT_SBR;
    sbr_present_flag = 1;
    tmp = 0x0f;
    for (i = 0; i < sizeof(ia_sampl_freq_table) / sizeof(ia_sampl_freq_table[0]); i++) {
      if (ia_sampl_freq_table[i] == pstr_audio_specific_config->ext_sampling_frequency) {
        tmp = i;
        break;
      }
    }
    pstr_audio_specific_config->ext_samp_freq_index = (UWORD32)tmp;
    bit_cnt += iusace_write_bits_buf(pstr_it_bit_buff,
                                     (pstr_audio_specific_config->ext_samp_freq_index), 4);
    if (pstr_audio_specific_config->ext_samp_freq_index == 0xf) {
      bit_cnt += iusace_write_bits_buf(pstr_it_bit_buff,
                                       (pstr_audio_specific_config->ext_sampling_frequency), 24);
    }
    if (AOT_PS == aot) {
      ps_present_flag = 1;
    }

    bit_cnt += iusace_write_bits_buf(pstr_it_bit_buff, AOT_AAC_LC, 5);
  }
  switch (aot) {
    case AOT_AAC_ELD: {
      bit_cnt += iaace_get_eld_specific_config_bytes(
          pstr_it_bit_buff, &pstr_audio_specific_config->str_aac_config,
          pstr_audio_specific_config->channel_configuration);
      break;
    }
    case AOT_AAC_LD:
    case AOT_AAC_LC:
    case AOT_SBR:
    case AOT_PS: {
      bit_cnt += iaace_ga_specific_config_bytes(
          pstr_it_bit_buff, &pstr_audio_specific_config->str_aac_config,
          pstr_audio_specific_config->channel_configuration, aot);

      if (AOT_AAC_LD == aot) {
        // epconfig
        bit_cnt += iusace_write_bits_buf(pstr_it_bit_buff, 0, 2);
      }
      if (AOT_SBR == ext_aot) {
        ext_id = 0x2b7;
        bit_cnt += iusace_write_bits_buf(pstr_it_bit_buff, ext_id, 11);

        bit_cnt += iusace_write_bits_buf(pstr_it_bit_buff, aot, 5);
        bit_cnt += iusace_write_bits_buf(pstr_it_bit_buff, sbr_present_flag, 1);
        if (sbr_present_flag) {
          bit_cnt += iusace_write_bits_buf(pstr_it_bit_buff,
                                           (pstr_audio_specific_config->ext_samp_freq_index), 4);
          if (pstr_audio_specific_config->ext_samp_freq_index == 0xf) {
            bit_cnt += iusace_write_bits_buf(
                pstr_it_bit_buff, (pstr_audio_specific_config->ext_sampling_frequency), 24);
          }

          if (AOT_PS == aot) {
            ext_id = 0x548;
            bit_cnt += iusace_write_bits_buf(pstr_it_bit_buff, ext_id, 11);
            bit_cnt += iusace_write_bits_buf(pstr_it_bit_buff, ps_present_flag, 1);
          }
        }
      }
      break;
    }

    case AOT_USAC: {
      WORD32 sbr_ratio_idx;
      ia_usac_config_struct *pstr_usac_config = &(pstr_audio_specific_config->str_usac_config);
      WORD32 ia_ccfl_tbl[5] = {768, 1024, 768, 1024, 1024};
      pstr_audio_specific_config->core_sbr_framelength_index =
          ccfl_idx;  // 768 core coder frame length without SBR
      pstr_usac_config->ccfl =
          ia_ccfl_tbl[pstr_audio_specific_config->core_sbr_framelength_index];
      tmp = 0x1f;
      for (i = 0; i < sizeof(ia_usac_sampl_freq_table) / sizeof(ia_usac_sampl_freq_table[0]);
           i++) {
        if (ia_usac_sampl_freq_table[i] == pstr_audio_specific_config->sampling_frequency) {
          tmp = i;
          break;
        }
      }
      pstr_audio_specific_config->samp_freq_index = (UWORD32)tmp;

      if (pstr_audio_specific_config->samp_freq_index == 0x1f) {
        bit_cnt += iusace_write_bits_buf(pstr_it_bit_buff, 0x1f, 5);
        bit_cnt += iusace_write_bits_buf(pstr_it_bit_buff,
                                         (pstr_audio_specific_config->sampling_frequency), 24);
      } else {
        bit_cnt += iusace_write_bits_buf(pstr_it_bit_buff,
                                         (pstr_audio_specific_config->samp_freq_index), 5);
      }

      bit_cnt += iusace_write_bits_buf(
          pstr_it_bit_buff, (pstr_audio_specific_config->core_sbr_framelength_index), 3);

      bit_cnt += iusace_write_bits_buf(pstr_it_bit_buff,
                                       (pstr_audio_specific_config->channel_configuration), 5);

      if (pstr_audio_specific_config->channel_configuration == 0) {
        bit_cnt += iusace_write_escape_value(
            pstr_it_bit_buff, pstr_audio_specific_config->num_audio_channels, 5, 8, 16);

        for (i = 0; i < pstr_audio_specific_config->num_audio_channels; i++) {
          tmp = pstr_audio_specific_config->output_channel_pos[i];
          bit_cnt += iusace_write_bits_buf(pstr_it_bit_buff, tmp, 5);
        }
      }

      sbr_ratio_idx = ixheaace_sbr_ratio(pstr_audio_specific_config->core_sbr_framelength_index);

      bit_cnt += iusace_encoder_config(pstr_it_bit_buff, pstr_usac_config, sbr_ratio_idx,
                                       &pstr_audio_specific_config->str_aac_config);

      bit_cnt +=
          iusace_write_bits_buf(pstr_it_bit_buff, pstr_usac_config->usac_cfg_ext_present, 1);
      if (pstr_usac_config->usac_cfg_ext_present) {
        bit_cnt += iusace_config_extension(pstr_it_bit_buff, pstr_usac_config);
      }

      if (sbr_ratio_idx)
        pstr_audio_specific_config->sbr_present_flag = 1;
      else
        pstr_audio_specific_config->sbr_present_flag = 0;

      pstr_audio_specific_config->ext_audio_object_type = 0;

      if (pstr_audio_specific_config->ext_audio_object_type == AOT_SBR) {
        pstr_audio_specific_config->ext_sync_word = 0x2b7;
        bit_cnt += iusace_write_bits_buf(pstr_it_bit_buff,
                                         (pstr_audio_specific_config->ext_sync_word), 11);

        bit_cnt += iusace_write_bits_buf(pstr_it_bit_buff,
                                         (pstr_audio_specific_config->ext_audio_object_type), 5);

        bit_cnt += iusace_write_bits_buf(pstr_it_bit_buff,
                                         (pstr_audio_specific_config->sbr_present_flag), 1);

        if (pstr_audio_specific_config->sbr_present_flag == 1) {
          bit_cnt += iusace_write_bits_buf(pstr_it_bit_buff,
                                           (pstr_audio_specific_config->ext_samp_freq_index), 4);

          if (pstr_audio_specific_config->ext_samp_freq_index == 0xf) {
            bit_cnt += iusace_write_bits_buf(
                pstr_it_bit_buff, (pstr_audio_specific_config->ext_sampling_frequency), 24);
          }
        }
      }
    }
    default:
      break;
  }
  return bit_cnt;
}

WORD32 ixheaace_get_usac_config_bytes(
    ia_bit_buf_struct *pstr_it_bit_buff,
    ixheaace_audio_specific_config_struct *pstr_audio_specific_config, WORD32 ccfl_idx) {
  WORD32 sbr_ratio_idx;
  ia_usac_config_struct *pstr_usac_config = &(pstr_audio_specific_config->str_usac_config);
  WORD32 ia_ccfl_tbl[5] = {768, 1024, 768, 1024, 1024};
  WORD32 i, tmp, bit_cnt = 0;
  pstr_audio_specific_config->core_sbr_framelength_index =
      ccfl_idx;  // 768 core coder frame length without SBR
  pstr_usac_config->ccfl = ia_ccfl_tbl[pstr_audio_specific_config->core_sbr_framelength_index];
  tmp = 0x1f;
  for (i = 0; i < sizeof(ia_usac_sampl_freq_table) / sizeof(ia_usac_sampl_freq_table[0]); i++) {
    if (ia_usac_sampl_freq_table[i] == pstr_audio_specific_config->sampling_frequency) {
      tmp = i;
      break;
    }
  }
  pstr_audio_specific_config->samp_freq_index = (UWORD32)tmp;

  if (pstr_audio_specific_config->samp_freq_index == 0x1f) {
    bit_cnt += iusace_write_bits_buf(pstr_it_bit_buff, 0x1f, 5);
    bit_cnt += iusace_write_bits_buf(pstr_it_bit_buff,
                                     (pstr_audio_specific_config->sampling_frequency), 24);
  } else {
    bit_cnt +=
        iusace_write_bits_buf(pstr_it_bit_buff, (pstr_audio_specific_config->samp_freq_index), 5);
  }

  bit_cnt += iusace_write_bits_buf(pstr_it_bit_buff,
                                   (pstr_audio_specific_config->core_sbr_framelength_index), 3);

  bit_cnt += iusace_write_bits_buf(pstr_it_bit_buff,
                                   (pstr_audio_specific_config->channel_configuration), 5);

  if (pstr_audio_specific_config->channel_configuration == 0) {
    bit_cnt += iusace_write_escape_value(
        pstr_it_bit_buff, pstr_audio_specific_config->num_audio_channels, 5, 8, 16);

    for (i = 0; i < pstr_audio_specific_config->num_audio_channels; i++) {
      tmp = pstr_audio_specific_config->output_channel_pos[i];
      bit_cnt += iusace_write_bits_buf(pstr_it_bit_buff, tmp, 5);
    }
  }

  sbr_ratio_idx = ixheaace_sbr_ratio(pstr_audio_specific_config->core_sbr_framelength_index);

  bit_cnt += iusace_encoder_config(pstr_it_bit_buff, pstr_usac_config, sbr_ratio_idx,
                                   &pstr_audio_specific_config->str_aac_config);

  bit_cnt += iusace_write_bits_buf(pstr_it_bit_buff, pstr_usac_config->usac_cfg_ext_present, 1);
  if (pstr_usac_config->usac_cfg_ext_present) {
    bit_cnt += iusace_config_extension(pstr_it_bit_buff, pstr_usac_config);
  }

  if (sbr_ratio_idx)
    pstr_audio_specific_config->sbr_present_flag = 1;
  else
    pstr_audio_specific_config->sbr_present_flag = 0;

  pstr_audio_specific_config->ext_audio_object_type = 0;

  if (pstr_audio_specific_config->ext_audio_object_type == AOT_SBR) {
    pstr_audio_specific_config->ext_sync_word = 0x2b7;
    bit_cnt +=
        iusace_write_bits_buf(pstr_it_bit_buff, (pstr_audio_specific_config->ext_sync_word), 11);

    bit_cnt += iusace_write_bits_buf(pstr_it_bit_buff,
                                     (pstr_audio_specific_config->ext_audio_object_type), 5);

    bit_cnt += iusace_write_bits_buf(pstr_it_bit_buff,
                                     (pstr_audio_specific_config->sbr_present_flag), 1);

    if (pstr_audio_specific_config->sbr_present_flag == 1) {
      bit_cnt += iusace_write_bits_buf(pstr_it_bit_buff,
                                       (pstr_audio_specific_config->ext_samp_freq_index), 4);

      if (pstr_audio_specific_config->ext_samp_freq_index == 0xf) {
        bit_cnt += iusace_write_bits_buf(
            pstr_it_bit_buff, (pstr_audio_specific_config->ext_sampling_frequency), 24);
      }
    }
  }
  return bit_cnt;
}