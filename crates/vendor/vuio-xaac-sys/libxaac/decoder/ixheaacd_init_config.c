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
#include <stdio.h>
#include <stdarg.h>
#include <assert.h>

#include "ixheaac_type_def.h"
#include "ixheaac_constants.h"
#include "ixheaacd_bitbuffer.h"

#include "ixheaacd_defines.h"
#include "ixheaacd_memory_standards.h"
#include "ixheaacd_sbrdecsettings.h"
#include "ixheaacd_sbr_scale.h"
#include "ixheaacd_env_extr_part.h"
#include "ixheaacd_aac_rom.h"
#include "ixheaacd_common_rom.h"
#include "ixheaacd_sbr_rom.h"
#include "ixheaacd_pulsedata.h"
#include "ixheaacd_pns.h"

#include "ixheaacd_sbr_common.h"
#include "ixheaacd_drc_data_struct.h"
#include "ixheaacd_drc_dec.h"

#include "ixheaacd_lt_predict.h"
#include "ixheaacd_cnst.h"
#include "ixheaacd_ec_defines.h"
#include "ixheaacd_ec_struct_def.h"
#include "ixheaacd_channelinfo.h"
#include "ixheaacd_channel.h"
#include "ixheaacd_sbrdecoder.h"
#include "ixheaacd_audioobjtypes.h"
#include "ixheaacd_latmdemux.h"
#include "ixheaacd_aacdec.h"
#include "ixheaacd_sbr_common.h"

#include "ixheaacd_hybrid.h"
#include "ixheaacd_ps_dec.h"
#include "ixheaacd_qmf_dec.h"
#include "ixheaacd_mps_macro_def.h"
#include "ixheaacd_mps_struct_def.h"
#include "ixheaacd_mps_res_rom.h"
#include "ixheaacd_mps_aac_struct.h"
#include "ixheaacd_mps_polyphase.h"
#include "ixheaacd_config.h"
#include "ixheaacd_mps_dec.h"
#include "ixheaacd_struct_def.h"

#include "ixheaacd_config.h"
#include "ixheaacd_interface.h"
#include "ixheaacd_info.h"
#include "ixheaacd_struct.h"
#include "ixheaac_error_standards.h"

#include "ixheaacd_error_codes.h"

static const WORD32 sampling_rate_tbl[] = {
    96000, 88200, 64000, 48000, 44100, 32000, 24000, 22050, 16000, 12000, 11025,
    8000,  7350,  0,     0,     57600, 51200, 40000, 38400, 34150, 28800, 25600,
    20000, 19200, 17075, 14400, 12800, 9600,  0,     0,     0};

UWORD32 ixheaacd_sbr_ratio(UWORD32 core_sbr_framelength_idx) {
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

static WORD32 ixheaacd_get_sample_freq_indx(WORD32 sampling_freq) {
  WORD32 index;
  const WORD32 tbl_size = sizeof(sampling_rate_tbl) / sizeof(WORD32) - 1;

  for (index = 0; index < tbl_size; index++) {
    if (sampling_rate_tbl[index] == sampling_freq) break;
  }
  return index;
}
UWORD32 ixheaacd_sbr_params(UWORD32 core_sbr_framelength_idx,
                            WORD32 *output_framelength, WORD32 *block_size,
                            WORD32 *output_samples, WORD32 *sample_rate_layer,
                            UWORD32 *sample_freq_indx) {
  UWORD32 sbr_ratio_index = 0x0FF;

  *output_framelength = -1;

  switch (core_sbr_framelength_idx) {
    case 0:
      sbr_ratio_index = USAC_SBR_RATIO_NO_SBR;
      *output_framelength = USAC_OUT_FRAMELENGTH_768;
      *block_size = 768;
      *output_samples = *block_size;
      break;
    case 1:
      sbr_ratio_index = USAC_SBR_RATIO_NO_SBR;
      *output_framelength = USAC_OUT_FRAMELENGTH_1024;
      *block_size = 1024;
      *output_samples = *block_size;
      break;
    case 2:
      sbr_ratio_index = USAC_SBR_RATIO_INDEX_8_3;
      *output_framelength = USAC_OUT_FRAMELENGTH_2048;
      *block_size = 768;
      *output_samples = (*block_size * 8) / 3;
      *sample_rate_layer = (*sample_rate_layer * 3) >> 3;
      break;
    case 3:
      sbr_ratio_index = USAC_SBR_RATIO_INDEX_2_1;
      *output_framelength = USAC_OUT_FRAMELENGTH_2048;
      *block_size = 1024;
      *output_samples = *block_size * 2;
      *sample_rate_layer = *sample_rate_layer >> 1;
      break;
    case 4:
      sbr_ratio_index = USAC_SBR_RATIO_INDEX_4_1;
      *output_framelength = USAC_OUT_FRAMELENGTH_4096;
      *block_size = 1024;
      *output_samples = *block_size * 4;
      *sample_rate_layer = *sample_rate_layer >> 2;
      break;
  }

  *sample_freq_indx = ixheaacd_get_sample_freq_indx(*sample_rate_layer);

  return sbr_ratio_index;
}

VOID ixheaacd_read_escape_value(ia_bit_buf_struct *it_bit_buff,
                                UWORD32 *ext_ele_value, UWORD32 no_bits1,
                                UWORD32 no_bits2, UWORD32 no_bits3) {
  UWORD32 value = 0;
  UWORD32 val_add = 0;
  UWORD32 max_val1 = (1 << no_bits1) - 1;
  UWORD32 max_val2 = (1 << no_bits2) - 1;

  value = ixheaacd_read_bits_buf(it_bit_buff, no_bits1);

  if (value == max_val1) {
    val_add = ixheaacd_read_bits_buf(it_bit_buff, no_bits2);

    value += val_add;

    if (val_add == max_val2) {
      val_add = ixheaacd_read_bits_buf(it_bit_buff, no_bits3);

      value += val_add;
    }
  }

  *ext_ele_value = value;
}

static IA_ERRORCODE ixheaacd_get_usac_chan_conf(ia_usac_config_struct *pstr_usac_config,
                        UWORD32 ch_config_index, ia_bit_buf_struct *it_bit_buff, WORD32 ec_flag)
{
  switch (ch_config_index) {
    case 1:
      pstr_usac_config->num_out_channels = 1;
      pstr_usac_config->output_channel_pos[0] = BS_OUTPUT_CHANNEL_POS_C;
      break;
    case 2:
      pstr_usac_config->num_out_channels = 2;
      pstr_usac_config->output_channel_pos[0] = BS_OUTPUT_CHANNEL_POS_L;
      pstr_usac_config->output_channel_pos[1] = BS_OUTPUT_CHANNEL_POS_R;
      break;
    case 8:
      pstr_usac_config->num_out_channels = 2;
      pstr_usac_config->output_channel_pos[0] = BS_OUTPUT_CHANNEL_POS_NA;
      pstr_usac_config->output_channel_pos[1] = BS_OUTPUT_CHANNEL_POS_NA;
      break;

    default:
      if (ec_flag)
        longjmp(*(it_bit_buff->xaac_jmp_buf),
            IA_XHEAAC_DEC_EXE_NONFATAL_INSUFFICIENT_INPUT_BYTES);
      else
        return IA_FATAL_ERROR;
      break;
  }
  return IA_NO_ERROR;
}

VOID ixheaacd_sbr_config(ia_bit_buf_struct *it_bit_buff,
                         ia_usac_dec_sbr_config_struct *pstr_usac_sbr_config) {
  pstr_usac_sbr_config->harmonic_sbr = ixheaacd_read_bits_buf(it_bit_buff, 1);
  pstr_usac_sbr_config->bs_inter_tes = ixheaacd_read_bits_buf(it_bit_buff, 1);
  pstr_usac_sbr_config->bs_pvc = ixheaacd_read_bits_buf(it_bit_buff, 1);
  pstr_usac_sbr_config->dflt_start_freq =
      ixheaacd_read_bits_buf(it_bit_buff, 4);

  pstr_usac_sbr_config->dflt_stop_freq = ixheaacd_read_bits_buf(it_bit_buff, 4);
  pstr_usac_sbr_config->dflt_header_extra1 =
      ixheaacd_read_bits_buf(it_bit_buff, 1);
  pstr_usac_sbr_config->dflt_header_extra2 =
      ixheaacd_read_bits_buf(it_bit_buff, 1);

  if (pstr_usac_sbr_config->dflt_header_extra1) {
    pstr_usac_sbr_config->dflt_freq_scale =
        ixheaacd_read_bits_buf(it_bit_buff, 2);
    pstr_usac_sbr_config->dflt_alter_scale =
        ixheaacd_read_bits_buf(it_bit_buff, 1);
    pstr_usac_sbr_config->dflt_noise_bands =
        ixheaacd_read_bits_buf(it_bit_buff, 2);
  }

  if (pstr_usac_sbr_config->dflt_header_extra2) {
    pstr_usac_sbr_config->dflt_limiter_bands =
        ixheaacd_read_bits_buf(it_bit_buff, 2);
    pstr_usac_sbr_config->dflt_limiter_gains =
        ixheaacd_read_bits_buf(it_bit_buff, 2);
    pstr_usac_sbr_config->dflt_interpol_freq =
        ixheaacd_read_bits_buf(it_bit_buff, 1);
    pstr_usac_sbr_config->dflt_smoothing_mode =
        ixheaacd_read_bits_buf(it_bit_buff, 1);
  }
}

WORD32 ixheaacd_ext_element_config(
    ia_bit_buf_struct *it_bit_buff,
    ia_usac_dec_element_config_struct *pstr_usac_element_config,
    UWORD8 *ptr_usac_ext_ele_payload, WORD32 *ptr_usac_ext_ele_payload_len,
    WORD32 *preroll_flag) {
  UWORD32 usac_ext_element_type, usac_ext_element_config_length, flag;

  UWORD32 i;

  ixheaacd_read_escape_value(it_bit_buff, &(usac_ext_element_type), 4, 8, 16);

  ixheaacd_read_escape_value(it_bit_buff, &(usac_ext_element_config_length), 4,
                             8, 16);
  if (usac_ext_element_config_length >= 768) return -1;

  flag = ixheaacd_read_bits_buf(it_bit_buff, 1);

  *ptr_usac_ext_ele_payload_len = 0;

  if (flag) {
    ixheaacd_read_escape_value(
        it_bit_buff,
        (UWORD32 *)(&(pstr_usac_element_config->usac_ext_eleme_def_len)), 8, 16,
        0);
    pstr_usac_element_config->usac_ext_eleme_def_len += 1;

  } else {
    pstr_usac_element_config->usac_ext_eleme_def_len = 0;
  }

  pstr_usac_element_config->usac_ext_elem_pld_frag =
      ixheaacd_read_bits_buf(it_bit_buff, 1);

  switch (usac_ext_element_type) {
    case ID_EXT_ELE_FILL:
      break;
    case ID_EXT_ELE_AUDIOPREROLL:
      *preroll_flag = 1;
      break;
    case ID_EXT_ELE_UNI_DRC:
      for (i = 0; i < usac_ext_element_config_length; i++) {
        ptr_usac_ext_ele_payload[i] = ixheaacd_read_bits_buf(it_bit_buff, 8);
      }
      *ptr_usac_ext_ele_payload_len = usac_ext_element_config_length;
      break;

    default:
      if ((it_bit_buff->cnt_bits >> 3) < (WORD32)usac_ext_element_config_length)
        return -1;
      it_bit_buff->ptr_read_next += usac_ext_element_config_length;
      it_bit_buff->cnt_bits -= (usac_ext_element_config_length << 3);

      break;
  }

  return 0;
}

IA_ERRORCODE ixheaacd_mps212_config(
    ia_bit_buf_struct *it_bit_buff,
    ia_usac_dec_mps_config_struct *pstr_usac_mps212_config,
    WORD32 stereo_config_index) {
  pstr_usac_mps212_config->bs_freq_res = ixheaacd_read_bits_buf(it_bit_buff, 3);

  pstr_usac_mps212_config->bs_fixed_gain_dmx =
      ixheaacd_read_bits_buf(it_bit_buff, 3);

  pstr_usac_mps212_config->bs_temp_shape_config =
      ixheaacd_read_bits_buf(it_bit_buff, 2);

  pstr_usac_mps212_config->bs_decorr_config =
      ixheaacd_read_bits_buf(it_bit_buff, 2);
  if (pstr_usac_mps212_config->bs_decorr_config > MAX_DECOR_CONFIG_IDX)
    return IA_FATAL_ERROR;

  pstr_usac_mps212_config->bs_high_rate_mode =
      ixheaacd_read_bits_buf(it_bit_buff, 1);

  pstr_usac_mps212_config->bs_phase_coding =
      ixheaacd_read_bits_buf(it_bit_buff, 1);

  pstr_usac_mps212_config->bs_ott_bands_phase_present =
      ixheaacd_read_bits_buf(it_bit_buff, 1);

  if (pstr_usac_mps212_config->bs_ott_bands_phase_present) {
    pstr_usac_mps212_config->bs_ott_bands_phase =
        ixheaacd_read_bits_buf(it_bit_buff, 5);
    if (pstr_usac_mps212_config->bs_ott_bands_phase > MAX_PARAMETER_BANDS)
      return IA_FATAL_ERROR;
  }

  if (stereo_config_index > 1) {
    pstr_usac_mps212_config->bs_residual_bands =
        ixheaacd_read_bits_buf(it_bit_buff, 5);

    if (pstr_usac_mps212_config->bs_residual_bands > MAX_PARAMETER_BANDS)
      return IA_FATAL_ERROR;

    pstr_usac_mps212_config->bs_ott_bands_phase =
        max(pstr_usac_mps212_config->bs_ott_bands_phase,
            pstr_usac_mps212_config->bs_residual_bands);

    pstr_usac_mps212_config->bs_pseudo_lr =
        ixheaacd_read_bits_buf(it_bit_buff, 1);
  }

  if (pstr_usac_mps212_config->bs_temp_shape_config == 2)
    pstr_usac_mps212_config->bs_env_quant_mode =
        ixheaacd_read_bits_buf(it_bit_buff, 1);

  return IA_NO_ERROR;
}

IA_ERRORCODE ixheaacd_cpe_config(
    ia_bit_buf_struct *it_bit_buff,
    ia_usac_dec_element_config_struct *pstr_usac_element_config,
    WORD32 sbr_ratio_index) {
  pstr_usac_element_config->tw_mdct = ixheaacd_read_bits_buf(it_bit_buff, 1);
  pstr_usac_element_config->noise_filling =
      ixheaacd_read_bits_buf(it_bit_buff, 1);

  if (sbr_ratio_index > 0) {
    ixheaacd_sbr_config(it_bit_buff,
                        &(pstr_usac_element_config->str_usac_sbr_config));
    pstr_usac_element_config->stereo_config_index =
        ixheaacd_read_bits_buf(it_bit_buff, 2);

  } else {
    pstr_usac_element_config->stereo_config_index = 0;
  }

  if (pstr_usac_element_config->stereo_config_index > 0)
    return ixheaacd_mps212_config(
        it_bit_buff, &(pstr_usac_element_config->str_usac_mps212_config),
        pstr_usac_element_config->stereo_config_index);

  return IA_NO_ERROR;
}

WORD32 ixheaacd_decoder_config(
    ia_bit_buf_struct *it_bit_buff,
    ia_usac_decoder_config_struct *pstr_usac_decoder_config,
    WORD32 sbr_ratio_index, UINT32 *chan, WORD32 ec_flag) {
  UWORD32 elem_idx = 0;
  UWORD32 err = 0;
  WORD32 num_channels = 0;

  ixheaacd_read_escape_value(
      it_bit_buff, &(pstr_usac_decoder_config->num_elements), 4, 8, 16);
  pstr_usac_decoder_config->num_elements += 1;
  pstr_usac_decoder_config->preroll_flag = 0;

  if (pstr_usac_decoder_config->num_elements > USAC_MAX_ELEMENTS) {
    if (ec_flag) {
      pstr_usac_decoder_config->num_elements = USAC_MAX_ELEMENTS;
      longjmp(*(it_bit_buff->xaac_jmp_buf), IA_XHEAAC_DEC_EXE_NONFATAL_INSUFFICIENT_INPUT_BYTES);
    } else {
      return IA_FATAL_ERROR;
    }
  }

  for (elem_idx = 0; elem_idx < pstr_usac_decoder_config->num_elements;
       elem_idx++) {
    ia_usac_dec_element_config_struct *pstr_usac_element_config =
        &(pstr_usac_decoder_config->str_usac_element_config[elem_idx]);

    pstr_usac_decoder_config->usac_element_type[elem_idx] =
        ixheaacd_read_bits_buf(it_bit_buff, 2);

    switch (pstr_usac_decoder_config->usac_element_type[elem_idx]) {
      case ID_USAC_SCE:
        num_channels++;
        pstr_usac_element_config->tw_mdct =
            ixheaacd_read_bits_buf(it_bit_buff, 1);
        pstr_usac_element_config->noise_filling =
            ixheaacd_read_bits_buf(it_bit_buff, 1);
        pstr_usac_element_config->stereo_config_index = 0;
        if (sbr_ratio_index > 0)
          ixheaacd_sbr_config(it_bit_buff,
                              &(pstr_usac_element_config->str_usac_sbr_config));

        break;

      case ID_USAC_CPE:
        num_channels += 2;
        if (ixheaacd_cpe_config(it_bit_buff, pstr_usac_element_config, sbr_ratio_index) !=
            IA_NO_ERROR) {
          if (ec_flag) {
            longjmp(*(it_bit_buff->xaac_jmp_buf),
                    IA_XHEAAC_DEC_EXE_NONFATAL_INSUFFICIENT_INPUT_BYTES);
          } else {
            return IA_FATAL_ERROR;
          }
        }
        if (pstr_usac_element_config->stereo_config_index > 1 && *chan < 2) {
          if (ec_flag) {
            longjmp(*(it_bit_buff->xaac_jmp_buf),
                    IA_XHEAAC_DEC_EXE_NONFATAL_INSUFFICIENT_INPUT_BYTES);
          } else {
            return IA_FATAL_ERROR;
          }
        }
        break;

      case ID_USAC_LFE:
        num_channels++;
        pstr_usac_element_config->tw_mdct = 0;
        pstr_usac_element_config->noise_filling = 0;
        pstr_usac_element_config->stereo_config_index = 0;
        break;

      case ID_USAC_EXT:
        err = ixheaacd_ext_element_config(
            it_bit_buff, pstr_usac_element_config,
            &pstr_usac_decoder_config->usac_ext_ele_payload_buf[elem_idx][0],
            &pstr_usac_decoder_config->usac_ext_ele_payload_len[elem_idx],
            &(pstr_usac_decoder_config->preroll_flag));

        if (pstr_usac_decoder_config->usac_ext_ele_payload_len[elem_idx] > 0) {
          pstr_usac_decoder_config->usac_ext_ele_payload_present[elem_idx] = 1;
        } else {
          pstr_usac_decoder_config->usac_ext_ele_payload_present[elem_idx] = 0;
        }
        if (err != 0) {
          if (ec_flag) {
            longjmp(*(it_bit_buff->xaac_jmp_buf),
                    IA_XHEAAC_DEC_EXE_NONFATAL_INSUFFICIENT_INPUT_BYTES);
          } else {
            return IA_FATAL_ERROR;
          }
        }
        break;
      default:
        if (ec_flag) {
          longjmp(*(it_bit_buff->xaac_jmp_buf),
                  IA_XHEAAC_DEC_EXE_NONFATAL_INSUFFICIENT_INPUT_BYTES);
        } else {
          return IA_FATAL_ERROR;
        }
        break;
    }
    if (num_channels > 2) {
      if (ec_flag) {
        longjmp(*(it_bit_buff->xaac_jmp_buf),
                IA_XHEAAC_DEC_EXE_NONFATAL_INSUFFICIENT_INPUT_BYTES);
      } else {
        return IA_FATAL_ERROR;
      }
    }
  }
  return err;
}

WORD32 ixheaacd_config_extension(ia_bit_buf_struct *it_bit_buff,
    ia_usac_decoder_config_struct *pstr_usac_decoder_config, WORD32 ec_flag) {
  UWORD32 i, j;
  UWORD32 num_config_extensions;
  UWORD32 usac_config_ext_type, usac_config_ext_len;

  ixheaacd_read_escape_value(it_bit_buff, &(num_config_extensions), 2, 4, 8);
  num_config_extensions += 1;
  if (USAC_MAX_CONFIG_EXTENSIONS < num_config_extensions) {
    if (ec_flag) {
      longjmp(*(it_bit_buff->xaac_jmp_buf), IA_XHEAAC_DEC_EXE_NONFATAL_INSUFFICIENT_INPUT_BYTES);
    } else {
      return IA_FATAL_ERROR;
    }
  }

  pstr_usac_decoder_config->num_config_extensions = num_config_extensions;
  memset(pstr_usac_decoder_config->usac_cfg_ext_info_len, 0,
         USAC_MAX_CONFIG_EXTENSIONS * sizeof(WORD32));
  memset(pstr_usac_decoder_config->usac_cfg_ext_info_present, 0,
         USAC_MAX_CONFIG_EXTENSIONS * sizeof(WORD32));

  for (j = 0; j < num_config_extensions; j++) {
    UWORD32 fill_byte_val = 0xa5;

    ixheaacd_read_escape_value(it_bit_buff, &(usac_config_ext_type), 4, 8, 16);
    ixheaacd_read_escape_value(it_bit_buff, &(usac_config_ext_len), 4, 8, 16);

    if (usac_config_ext_len > 768) {
      if (ec_flag) {
        longjmp(*(it_bit_buff->xaac_jmp_buf),
                IA_XHEAAC_DEC_EXE_NONFATAL_INSUFFICIENT_INPUT_BYTES);
      } else {
        return IA_FATAL_ERROR;
      }
    }

    switch (usac_config_ext_type) {
      case ID_CONFIG_EXT_FILL:
        for (i = 0; i < usac_config_ext_len; i++) {
          fill_byte_val = ixheaacd_read_bits_buf(it_bit_buff, 8);
          if (fill_byte_val != 0xa5) {
            if (ec_flag) {
              longjmp(*(it_bit_buff->xaac_jmp_buf),
                      IA_XHEAAC_DEC_EXE_NONFATAL_INSUFFICIENT_INPUT_BYTES);
            } else {
              return IA_FATAL_ERROR;
            }
          }
        }
        break;
      default:
        if ((WORD32)usac_config_ext_len > (it_bit_buff->cnt_bits >> 3)) {
          if (ec_flag) {
            longjmp(*(it_bit_buff->xaac_jmp_buf),
                    IA_XHEAAC_DEC_EXE_NONFATAL_INSUFFICIENT_INPUT_BYTES);
          } else {
            return IA_FATAL_ERROR;
          }
        }
        if (ID_CONFIG_EXT_LOUDNESS_INFO == usac_config_ext_type) {
          for (i = 0; i < usac_config_ext_len; i++) {
            UWORD8 byte_val = ixheaacd_read_bits_buf(it_bit_buff, 8);
            pstr_usac_decoder_config->usac_cfg_ext_info_buf[j][i] = byte_val;
          }
          pstr_usac_decoder_config->usac_cfg_ext_info_len[j] =
              usac_config_ext_len;
          pstr_usac_decoder_config->usac_cfg_ext_info_present[j] = 1;
        } else {
          for (i = 0; i < usac_config_ext_len; i++)
            ixheaacd_read_bits_buf(it_bit_buff, 8);
        }
        break;
    }
  }

  return 0;
}

WORD32 ixheaacd_config(ia_bit_buf_struct *it_bit_buff, ia_usac_config_struct *pstr_usac_conf,
                       UINT32 *chan, WORD32 ec_flag) {
  WORD32 tmp, err;
  err = 0;

  pstr_usac_conf->usac_sampling_frequency_index =
      ixheaacd_read_bits_buf(it_bit_buff, 5);

  if (pstr_usac_conf->usac_sampling_frequency_index == 0x1f) {
    pstr_usac_conf->usac_sampling_frequency =
        ixheaacd_read_bits_buf(it_bit_buff, 24);

    if (pstr_usac_conf->usac_sampling_frequency > USAC_MAX_SAMPLE_RATE) {
      if (ec_flag) {
        longjmp(*(it_bit_buff->xaac_jmp_buf),
                IA_XHEAAC_DEC_EXE_NONFATAL_INSUFFICIENT_INPUT_BYTES);
      } else {
        return IA_FATAL_ERROR;
      }
    }

  } else {
    pstr_usac_conf->usac_sampling_frequency =
        sampling_rate_tbl[pstr_usac_conf->usac_sampling_frequency_index];
  }

  if (pstr_usac_conf->usac_sampling_frequency == 0) {
    if (ec_flag) {
      longjmp(*(it_bit_buff->xaac_jmp_buf), IA_XHEAAC_DEC_EXE_NONFATAL_INSUFFICIENT_INPUT_BYTES);
    } else {
      return IA_FATAL_ERROR;
    }
  }
  pstr_usac_conf->core_sbr_framelength_index =
      ixheaacd_read_bits_buf(it_bit_buff, 3);

  if (pstr_usac_conf->core_sbr_framelength_index > MAX_CORE_SBR_FRAME_LEN_IDX) {
    if (ec_flag) {
      longjmp(*(it_bit_buff->xaac_jmp_buf), IA_XHEAAC_DEC_EXE_NONFATAL_INSUFFICIENT_INPUT_BYTES);
    } else {
      return IA_FATAL_ERROR;
    }
  }

  pstr_usac_conf->channel_configuration_index =
      ixheaacd_read_bits_buf(it_bit_buff, 5);
  if ((pstr_usac_conf->channel_configuration_index >= 3) &&
      (pstr_usac_conf->channel_configuration_index != 8)) {
    if (ec_flag) {
      longjmp(*(it_bit_buff->xaac_jmp_buf), IA_XHEAAC_DEC_EXE_NONFATAL_INSUFFICIENT_INPUT_BYTES);
    } else {
      return IA_FATAL_ERROR;
    }
  }

  if (pstr_usac_conf->channel_configuration_index == 0) {
    UWORD32 i;

    ixheaacd_read_escape_value(it_bit_buff,
                               (UWORD32 *)(&(pstr_usac_conf->num_out_channels)),
                               5, 8, 16);
    if (BS_MAX_NUM_OUT_CHANNELS < pstr_usac_conf->num_out_channels) {
      return IA_XHEAAC_DEC_INIT_FATAL_STREAM_CHAN_GT_MAX;
    }
    if (pstr_usac_conf->num_out_channels < 1) {
      return IA_XHEAAC_DEC_INIT_FATAL_DEC_INIT_FAIL;
    }
    for (i = 0; i < pstr_usac_conf->num_out_channels; i++)
      pstr_usac_conf->output_channel_pos[i] =
          ixheaacd_read_bits_buf(it_bit_buff, 5);

    if (ec_flag) {
      longjmp(*(it_bit_buff->xaac_jmp_buf), IA_XHEAAC_DEC_EXE_NONFATAL_INSUFFICIENT_INPUT_BYTES);
    }
  } else {
    err = ixheaacd_get_usac_chan_conf(pstr_usac_conf, pstr_usac_conf->channel_configuration_index,
                                      it_bit_buff, ec_flag);
    if (err != 0)
      return err;
  }

  err = ixheaacd_decoder_config(
      it_bit_buff, &(pstr_usac_conf->str_usac_dec_config),
      ixheaacd_sbr_ratio(pstr_usac_conf->core_sbr_framelength_index), chan, ec_flag);
  if (err != 0) return err;

  tmp = ixheaacd_read_bits_buf(it_bit_buff, 1);

  if (tmp) {
    err = ixheaacd_config_extension(it_bit_buff,
                                    &pstr_usac_conf->str_usac_dec_config, ec_flag);
    if (err != 0) return -1;
  }

  return err;
}

VOID ixheaacd_conf_default(ia_usac_config_struct *pstr_usac_conf) {
  WORD32 i;

  pstr_usac_conf->num_out_channels = 0;

  for (i = 0; i < BS_MAX_NUM_OUT_CHANNELS; i++)
    pstr_usac_conf->output_channel_pos[i] = BS_OUTPUT_CHANNEL_POS_NA;

  pstr_usac_conf->str_usac_dec_config.num_elements = 0;

  for (i = 0; i < USAC_MAX_ELEMENTS; i++)
    pstr_usac_conf->str_usac_dec_config.usac_element_type[i] = ID_USAC_INVALID;

  return;
}
