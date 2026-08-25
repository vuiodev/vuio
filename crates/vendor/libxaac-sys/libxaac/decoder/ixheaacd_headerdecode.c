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
#include <stdlib.h>
#include "ixheaac_type_def.h"
#include "ixheaac_error_standards.h"
#include "ixheaac_constants.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops16.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaacd_sbr_common.h"

#include "ixheaacd_bitbuffer.h"
#include "ixheaacd_defines.h"
#include "ixheaacd_aac_rom.h"

#include "ixheaacd_sbrdecsettings.h"
#include "ixheaacd_sbr_scale.h"
#include "ixheaacd_env_extr_part.h"
#include "ixheaacd_sbr_rom.h"

#include "ixheaacd_lpp_tran.h"
#include "ixheaacd_hybrid.h"
#include "ixheaacd_ps_dec.h"

#include "ixheaacd_env_extr.h"
#include "ixheaacd_common_rom.h"

#include "ixheaacd_pulsedata.h"

#include "ixheaacd_pns.h"
#include "ixheaacd_drc_data_struct.h"

#include "ixheaacd_lt_predict.h"

#include "ixheaacd_cnst.h"
#include "ixheaacd_ec_defines.h"
#include "ixheaacd_ec_struct_def.h"
#include "ixheaacd_channelinfo.h"
#include "ixheaacd_drc_dec.h"
#include "ixheaacd_sbrdecoder.h"
#include "ixheaacd_block.h"
#include "ixheaacd_channel.h"

#include "ixheaacd_adts.h"
#include "ixheaacd_audioobjtypes.h"
#include "ixheaacd_sbrdecoder.h"
#include "ixheaacd_memory_standards.h"

#include "ixheaacd_latmdemux.h"

#include "ixheaacd_aacdec.h"
#include "ixheaacd_mps_polyphase.h"
#include "ixheaacd_config.h"
#include "ixheaacd_qmf_dec.h"
#include "ixheaacd_mps_macro_def.h"
#include "ixheaacd_mps_struct_def.h"
#include "ixheaacd_mps_res_rom.h"
#include "ixheaacd_mps_aac_struct.h"
#include "ixheaacd_mps_dec.h"
#include "ixheaacd_struct_def.h"
#include "ixheaacd_error_codes.h"
#include "ixheaacd_definitions.h"
#include "ixheaacd_adts_crc_check.h"

#include "ixheaacd_headerdecode.h"

#include "ixheaacd_interface.h"
#include "ixheaacd_info.h"
#include "ixheaacd_mps_interface.h"
#include "ixheaacd_config.h"

#include "ixheaacd_struct.h"
#include "ixheaacd_function_selector.h"
#include "ixheaacd_ld_mps_dec.h"

#include "ixheaac_error_standards.h"

#define ELDEXT_SAOC 1
#define ELDEXT_TERM 0
#define ELDEXT_LDSAC 2

extern const WORD32 ixheaacd_sampl_freq_idx_table[17];

#define AAC_LC_PROFILE (2)

#define ADTS_HEADER_LENGTH 7

static PLATFORM_INLINE VOID
ixheaacd_aac_bytealign(struct ia_bit_buf_struct *it_bit_buff) {
  WORD16 num_bit;
  num_bit = (it_bit_buff->bit_pos + 1);
  if (num_bit != 8) {
    it_bit_buff->bit_pos = 7;
    it_bit_buff->cnt_bits -= num_bit;
    it_bit_buff->ptr_read_next += 1;
  }
}

WORD32 ixheaacd_read_pce_channel_info(WORD32 ch, WORD8 *ptr_is_cpe,
                                      WORD8 *ptr_tag_select,
                                      struct ia_bit_buf_struct *it_bit_buff) {
  WORD32 num_ch = 0, i, tmp;
  for (i = 0; i < ch; i++) {
    tmp = ixheaacd_read_bits_buf(it_bit_buff, 5);
    ptr_is_cpe[i] = (tmp & 0x10) >> 4;

    if (ptr_is_cpe[i]) {
      num_ch += 2;
    } else {
      num_ch++;
    }

    ptr_tag_select[i] = (tmp & 0xF);
  }
  return num_ch;
}

VOID ixheaacd_read_pce_mixdown_data(struct ia_bit_buf_struct *it_bit_buff,
                                    WORD32 mix_down_present,
                                    WORD32 mix_down_element_no) {
  WORD32 mix_down_flag = ixheaacd_read_bits_buf(it_bit_buff, mix_down_present);
  if (mix_down_flag == 1) {
    ixheaacd_read_bits_buf(it_bit_buff, mix_down_element_no);
  }
}

VOID ixheaacd_skip_bits(struct ia_bit_buf_struct *it_bit_buff, WORD32 bits,
                        WORD32 num_element) {
  WORD32 i;
  for (i = 0; i < num_element; i++) {
    ixheaacd_read_bits_buf(it_bit_buff, bits);
  }
}

WORD32 ixheaacd_read_prog_config_element(
    ia_program_config_struct *ptr_config_element,
    struct ia_bit_buf_struct *it_bit_buff) {
  WORD32 i, tmp;
  WORD count = 0, num_ch = 0;

  tmp = ixheaacd_read_bits_buf(it_bit_buff, 6);

  ptr_config_element->element_instance_tag = (tmp >> 2);
  ptr_config_element->object_type = tmp & 0x3;

  if ((ptr_config_element->object_type + 1) != 2

      && (ptr_config_element->object_type + 1) != 4

      ) {
    return IA_XHEAAC_DEC_INIT_FATAL_DEC_INIT_FAIL;
  }

  ptr_config_element->samp_freq_index = ixheaacd_read_bits_buf(it_bit_buff, 4);
  if (ptr_config_element->samp_freq_index > 11) {
    return IA_XHEAAC_DEC_EXE_NONFATAL_DECODE_FRAME_ERROR;
  }

  tmp = ixheaacd_read_bits_buf(it_bit_buff, 21);

  count += ptr_config_element->num_front_channel_elements = (tmp >> 17);
  count += ptr_config_element->num_side_channel_elements =
      (tmp & 0x1E000) >> 13;
  count += ptr_config_element->num_back_channel_elements = (tmp & 0x1E00) >> 9;
  count += ptr_config_element->num_lfe_channel_elements = (tmp & 0x180) >> 7;
  ptr_config_element->num_assoc_data_elements = (tmp & 0x70) >> 4;
  count += ptr_config_element->num_valid_cc_elements = tmp & 0xF;

  if (count > MAX_BS_ELEMENT) {
    return IA_XHEAAC_DEC_INIT_FATAL_STREAM_CHAN_GT_MAX;
  }

  ixheaacd_read_pce_mixdown_data(it_bit_buff, 1, 4);
  ixheaacd_read_pce_mixdown_data(it_bit_buff, 1, 4);
  ixheaacd_read_pce_mixdown_data(it_bit_buff, 1, 3);

  num_ch += ixheaacd_read_pce_channel_info(
      ptr_config_element->num_front_channel_elements,
      ptr_config_element->front_element_is_cpe,
      ptr_config_element->front_element_tag_select, it_bit_buff);

  num_ch += ixheaacd_read_pce_channel_info(
      ptr_config_element->num_side_channel_elements,
      ptr_config_element->side_element_is_cpe,
      ptr_config_element->side_element_tag_select, it_bit_buff);

  num_ch += ixheaacd_read_pce_channel_info(
      ptr_config_element->num_back_channel_elements,
      ptr_config_element->back_element_is_cpe,
      ptr_config_element->back_element_tag_select, it_bit_buff);

  num_ch += ptr_config_element->num_lfe_channel_elements;

  for (i = 0; i < (ptr_config_element->num_lfe_channel_elements); i++) {
    ptr_config_element->lfe_element_tag_select[i] =
        ixheaacd_read_bits_buf(it_bit_buff, 4);
  }

  ptr_config_element->channels = num_ch;

  for (i = 0; i < (ptr_config_element->num_assoc_data_elements); i++) {
    ixheaacd_read_bits_buf(it_bit_buff, 4);
  }

  ixheaacd_skip_bits(it_bit_buff, 5, ptr_config_element->num_valid_cc_elements);

  {
    WORD32 bits_to_read = ptr_config_element->alignment_bits;
    if (bits_to_read <= it_bit_buff->bit_pos) {
      bits_to_read = it_bit_buff->bit_pos - bits_to_read;
    } else {
      bits_to_read = 8 - (bits_to_read) + it_bit_buff->bit_pos;
    }
    tmp = ixheaacd_read_bits_buf(it_bit_buff, bits_to_read);
  }
  tmp = ixheaacd_read_bits_buf(it_bit_buff, 8);

  ixheaacd_skip_bits(it_bit_buff, 8, tmp);

  return 0;
}

WORD ixheaacd_decode_pce(struct ia_bit_buf_struct *it_bit_buff,
                         UWORD32 *ui_pce_found_in_hdr,
                         ia_program_config_struct *ptr_prog_config) {
  WORD32 error_code = 0;

  if (*ui_pce_found_in_hdr == 1 || *ui_pce_found_in_hdr == 3) {
    ia_program_config_struct ptr_config_element = {0};
    ptr_config_element.alignment_bits = ptr_prog_config->alignment_bits;
    error_code =
        ixheaacd_read_prog_config_element(&ptr_config_element, it_bit_buff);
    *ui_pce_found_in_hdr = 3;
  } else {
    error_code =
        ixheaacd_read_prog_config_element(ptr_prog_config, it_bit_buff);
    *ui_pce_found_in_hdr = 2;
  }
  return error_code;
}

static PLATFORM_INLINE WORD32 ixheaacd_get_adif_header(
    ia_adif_header_struct *adif, struct ia_bit_buf_struct *it_bit_buff) {
  WORD32 i;
  WORD32 ret_val = 0, tmp;

  ixheaacd_read_bits_buf(it_bit_buff, 16);
  tmp = ixheaacd_read_bits_buf(it_bit_buff, 17);

  if (tmp & 0x1) {
    ixheaacd_skip_bits(it_bit_buff, 8, 9);
  }

  tmp = ixheaacd_read_bits_buf(it_bit_buff, 3);

  adif->bit_stream_type = (tmp & 0x1);

  ixheaacd_read_bits_buf(it_bit_buff, 23);

  tmp = ixheaacd_read_bits_buf(it_bit_buff, 4);

  for (i = 0; i <= tmp; i++) {
    if (adif->bit_stream_type == 0) {
      ixheaacd_read_bits_buf(it_bit_buff, 20);
    }

    adif->prog_config_present = 1;
    adif->str_prog_config.alignment_bits = 7;
    ret_val =
        ixheaacd_read_prog_config_element(&adif->str_prog_config, it_bit_buff);
    if (ret_val) {
      return ret_val;
    }
  }

  return 0;
}

WORD32 ixheaacd_find_syncword(ia_adts_header_struct *adts,
                              struct ia_bit_buf_struct *it_bit_buff) {
  adts->sync_word = (WORD16)ixheaacd_read_bits_buf(it_bit_buff, 12);
  if (adts->sync_word == 0xFFF) {
    return 0;
  }

  while (1) {
    ixheaacd_read_bidirection(it_bit_buff, -4);
    if (it_bit_buff->cnt_bits < 12) {
      return IA_XHEAAC_DEC_INIT_FATAL_DEC_INIT_FAIL;
    }
    adts->sync_word = (WORD16)ixheaacd_read_bits_buf(it_bit_buff, 12);
    if (adts->sync_word == 0xFFF) {
      ixheaacd_read_bidirection(it_bit_buff, -12);
      return IA_XHEAAC_DEC_INIT_FATAL_DEC_INIT_FAIL;
    }
  }
}

WORD32 ixheaacd_adtsframe(ia_adts_header_struct *adts,
                          struct ia_bit_buf_struct *it_bit_buff) {
  WORD32 tmp;
  IA_ERRORCODE err = IA_NO_ERROR;
  WORD32 crc_reg;
  ia_adts_crc_info_struct *ptr_adts_crc_info = it_bit_buff->pstr_adts_crc_info;
  ptr_adts_crc_info->crc_active = 1;
  ptr_adts_crc_info->no_reg = 0;
  ixheaacd_read_bidirection(it_bit_buff, -12);
  crc_reg = ixheaacd_adts_crc_start_reg(ptr_adts_crc_info, it_bit_buff,
                                        CRC_ADTS_HEADER_LEN);
  err = ixheaacd_find_syncword(adts, it_bit_buff);
  if (err) return err;

  tmp = ixheaacd_read_bits_buf(it_bit_buff, 10);

  adts->id = (tmp & 0x200) >> 9;
  adts->layer = (tmp & 0x180) >> 7;
  adts->protection_absent = (tmp & 0x40) >> 6;
  adts->profile = (tmp & 0x30) >> 4;
  { adts->profile++; }
  adts->samp_freq_index = (tmp & 0xF);

  if (((adts->profile != AAC_LC_PROFILE)) || (adts->samp_freq_index > 11))

  {
    return IA_XHEAAC_DEC_INIT_FATAL_DEC_INIT_FAIL;
  }

  tmp = ixheaacd_read_bits_buf(it_bit_buff, 21);

  adts->channel_configuration = (WORD32)(tmp & 0xE0000) >> 17;

  adts->aac_frame_length = (tmp & 0x1FFF);

  tmp = ixheaacd_read_bits_buf(it_bit_buff, 13);

  adts->no_raw_data_blocks = (tmp & 0x3);

  ixheaacd_adts_crc_end_reg(ptr_adts_crc_info, it_bit_buff, crc_reg);

  if (adts->protection_absent == 0) {
    ixheaacd_skip_bits(it_bit_buff, 16, adts->no_raw_data_blocks);
    adts->crc_check = ixheaacd_read_bits_buf(it_bit_buff, 16);

    ptr_adts_crc_info->crc_active = 1;
    ptr_adts_crc_info->file_value = adts->crc_check;
  } else
    ptr_adts_crc_info->crc_active = 0;

  ixheaacd_aac_bytealign(it_bit_buff);
  return 0;
}

WORD32 ixheaacd_get_samp_rate(
    struct ia_bit_buf_struct *it_bit_buff,
    ia_sampling_rate_info_struct *pstr_samp_rate_info,
    ia_audio_specific_config_struct *pstr_audio_specific_config) {
  WORD32 index;
  WORD32 sampling_rate;
  index = ixheaacd_read_bits_buf(it_bit_buff, 4);
  pstr_audio_specific_config->samp_frequency_index = index;

  if (index == 0x0F) {
    sampling_rate = ixheaacd_read_bits_buf(it_bit_buff, 24);

    if (pstr_audio_specific_config->audio_object_type != AOT_USAC) {
      if (sampling_rate < 9391)
        sampling_rate = 8000;
      else if ((sampling_rate >= 9391) && (sampling_rate < 11502))
        sampling_rate = 11025;
      else if ((sampling_rate >= 11502) && (sampling_rate < 13856))
        sampling_rate = 12000;
      else if ((sampling_rate >= 13856) && (sampling_rate < 18783))
        sampling_rate = 16000;
      else if ((sampling_rate >= 18783) && (sampling_rate < 23004))
        sampling_rate = 22050;
      else if ((sampling_rate >= 23004) && (sampling_rate < 27713))
        sampling_rate = 24000;
      else if ((sampling_rate >= 27713) && (sampling_rate < 37566))
        sampling_rate = 32000;
      else if ((sampling_rate >= 37566) && (sampling_rate < 46009))
        sampling_rate = 44100;
      else if ((sampling_rate >= 46009) && (sampling_rate < 55426))
        sampling_rate = 48000;
      else if ((sampling_rate >= 55426) && (sampling_rate < 75132))
        sampling_rate = 64000;
      else if ((sampling_rate >= 75132) && (sampling_rate < 92017))
        sampling_rate = 88200;
      else if (sampling_rate >= 92017)
        sampling_rate = 96000;
    }
    return sampling_rate;
  } else if ((index > 12) && (index < 15)) {
    return -1;
  } else {
    return ((pstr_samp_rate_info[index].sampling_frequency));
  }
}
static int ixheaacd_get_ld_sbr_header(
    ia_bit_buf_struct *it_bit_buff,
    ia_sbr_header_data_struct *sbr_header_data) {
  WORD32 header_extra_1, header_extra_2;
  UWORD32 tmp, bit_cnt = 0;

  tmp = ixheaacd_read_bits_buf(it_bit_buff, 16);
  bit_cnt += 16;

  sbr_header_data->amp_res = (tmp & 0x8000) >> 15;
  sbr_header_data->start_freq = (tmp & 0x7800) >> 11;
  sbr_header_data->stop_freq = (tmp & 0x780) >> 7;
  sbr_header_data->xover_band = (tmp & 0x70) >> 4;
  header_extra_1 = (tmp & 0x0002) >> 1;
  header_extra_2 = (tmp & 0x0001);

  if (header_extra_1) {
    sbr_header_data->freq_scale = ixheaacd_read_bits_buf(it_bit_buff, 2);
    sbr_header_data->alter_scale = ixheaacd_read_bits_buf(it_bit_buff, 1);
    sbr_header_data->noise_bands = ixheaacd_read_bits_buf(it_bit_buff, 2);
  } else {
    sbr_header_data->freq_scale = 2;
    sbr_header_data->alter_scale = 1;
    sbr_header_data->noise_bands = 2;
  }

  if (header_extra_2) {
    sbr_header_data->limiter_bands = ixheaacd_read_bits_buf(it_bit_buff, 2);
    sbr_header_data->limiter_gains = ixheaacd_read_bits_buf(it_bit_buff, 2);
    sbr_header_data->interpol_freq = ixheaacd_read_bits_buf(it_bit_buff, 1);
    sbr_header_data->smoothing_mode = ixheaacd_read_bits_buf(it_bit_buff, 1);
  } else {
    sbr_header_data->limiter_bands = 2;
    sbr_header_data->limiter_gains = 2;
    sbr_header_data->interpol_freq = 1;
    sbr_header_data->smoothing_mode = 1;
  }

  return (bit_cnt);
}

WORD32 ixheaacd_eld_sbr_header(ia_bit_buf_struct *it_bit_buff, WORD32 channels,
                               ia_sbr_header_data_struct *pstr_sbr_config) {
  int num_sbr_header, el, bit_cnt = 0;
  switch (channels) {
    default:
      num_sbr_header = 0;
      break;
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
  }
  for (el = 0; el < num_sbr_header; el++) {
    bit_cnt = ixheaacd_get_ld_sbr_header(it_bit_buff, pstr_sbr_config);
  }
  return (bit_cnt);
}

WORD32 ixheaacd_ga_hdr_dec(ia_aac_dec_state_struct *aac_state_struct,
                           WORD32 header_len, WORD32 *bytes_consumed,
                           ia_sampling_rate_info_struct *pstr_samp_rate_info,
                           struct ia_bit_buf_struct *it_bit_buff) {
  WORD32 tmp;
  WORD32 cnt_bits = it_bit_buff->cnt_bits;
  UWORD32 aot_init;
  UWORD32 tmp_aot;

  ia_audio_specific_config_struct *pstr_audio_specific_config;

  memset(aac_state_struct->ia_audio_specific_config, 0,
         sizeof(ia_audio_specific_config_struct));

  memset(&(aac_state_struct->eld_specific_config), 0,
         sizeof(ia_eld_specific_config_struct));

  pstr_audio_specific_config = aac_state_struct->ia_audio_specific_config;

  aac_state_struct->p_config->str_prog_config.alignment_bits =
      it_bit_buff->bit_pos;

  aot_init = aac_state_struct->audio_object_type;
  aac_state_struct->frame_length = FRAME_SIZE;

  tmp_aot = ixheaacd_read_bits_buf(it_bit_buff, 5);

  if (tmp_aot == 31) {
    tmp = ixheaacd_read_bits_buf(it_bit_buff, 6);
    tmp_aot = 32 + tmp;
  }

  if (aac_state_struct->header_dec_done || aac_state_struct->ui_init_done) {
    if (tmp_aot != aot_init && tmp_aot != AOT_SBR && tmp_aot != AOT_PS)
      return IA_FATAL_ERROR;
  }

  pstr_audio_specific_config->audio_object_type =
      aac_state_struct->audio_object_type = tmp_aot;

  tmp = ixheaacd_get_samp_rate(it_bit_buff, pstr_samp_rate_info,
                               pstr_audio_specific_config);
  pstr_audio_specific_config->sampling_frequency = tmp;

  if (tmp == -1) {
    *bytes_consumed = 1;
    return IA_XHEAAC_DEC_INIT_FATAL_DEC_INIT_FAIL;
  } else
    aac_state_struct->sampling_rate = tmp;
  aac_state_struct->p_config->ui_samp_freq = tmp;

  aac_state_struct->ch_config = ixheaacd_read_bits_buf(it_bit_buff, 4);

  if (aac_state_struct->audio_object_type == AOT_USAC &&
      ((aac_state_struct->ch_config >= 3) && (aac_state_struct->ch_config != 8))) {
    return IA_XHEAAC_DEC_INIT_FATAL_DEC_INIT_FAIL;
  }

  pstr_audio_specific_config->channel_configuration =
      aac_state_struct->ch_config;

  if (aac_state_struct->audio_object_type == AOT_SBR ||
      aac_state_struct->audio_object_type == AOT_PS) {
    tmp = ixheaacd_get_samp_rate(it_bit_buff, pstr_samp_rate_info,
                                 pstr_audio_specific_config);
    aac_state_struct->sbr_present_flag = 1;
    if (tmp == -1) {
      *bytes_consumed = 1;
      return IA_XHEAAC_DEC_INIT_FATAL_DEC_INIT_FAIL;
    } else
      aac_state_struct->extension_samp_rate = tmp;

    aac_state_struct->audio_object_type =
        ixheaacd_read_bits_buf(it_bit_buff, 5);
  }

  if (aac_state_struct->header_dec_done || aac_state_struct->ui_init_done) {
    if (aac_state_struct->audio_object_type != aot_init) return IA_FATAL_ERROR;
  }

  if (((aac_state_struct->audio_object_type >= AOT_AAC_MAIN &&
        aac_state_struct->audio_object_type <= AOT_AAC_LTP) ||
       aac_state_struct->audio_object_type == AOT_AAC_SCAL ||
       aac_state_struct->audio_object_type == AOT_TWIN_VQ ||
       aac_state_struct->audio_object_type == AOT_ER_AAC_LD ||
       aac_state_struct->audio_object_type == AOT_ER_AAC_ELD ||
       aac_state_struct->audio_object_type == AOT_ER_AAC_SCAL ||
       aac_state_struct->audio_object_type == AOT_ER_AAC_LC) &&
      aac_state_struct->audio_object_type != AOT_USAC)

  {
    aac_state_struct->usac_flag = 0;

    aac_state_struct->frame_len_flag = ixheaacd_read_bits_buf(it_bit_buff, 1);
    if (aac_state_struct->audio_object_type != AOT_ER_AAC_ELD) {
      aac_state_struct->depends_on_core_coder =
          ixheaacd_read_bits_buf(it_bit_buff, 1);
      aac_state_struct->extension_flag = ixheaacd_read_bits_buf(it_bit_buff, 1);

      if (aac_state_struct->ch_config == 0) {
        WORD32 error_code;
        error_code = ixheaacd_read_prog_config_element(
            &aac_state_struct->p_config->str_prog_config, it_bit_buff);
        if (error_code != 0) {
          *bytes_consumed = 1;
          return error_code;
        }
        aac_state_struct->p_config->ui_pce_found_in_hdr = 1;
      }
    }
    if (aac_state_struct->audio_object_type == AOT_ER_AAC_ELD ||
        aac_state_struct->audio_object_type == AOT_ER_AAC_LD ||
        aac_state_struct->audio_object_type == AOT_ER_AAC_LC ||
        aac_state_struct->audio_object_type == AOT_ER_AAC_SCAL) {
      aac_state_struct->eld_specific_config.aac_sect_data_resil_flag = 0;
      aac_state_struct->eld_specific_config.aac_sf_data_resil_flag = 0;
      aac_state_struct->eld_specific_config.aac_spect_data_resil_flag = 0;
      aac_state_struct->eld_specific_config.ep_config = 0;
      if ((aac_state_struct->extension_flag == 1) ||
          aac_state_struct->audio_object_type == AOT_ER_AAC_ELD) {
        if (aac_state_struct->audio_object_type >= ER_OBJECT_START) {
          aac_state_struct->eld_specific_config.aac_sect_data_resil_flag =
              ixheaacd_read_bits_buf(it_bit_buff, 1);
          aac_state_struct->eld_specific_config.aac_sf_data_resil_flag =
              ixheaacd_read_bits_buf(it_bit_buff, 1);
          aac_state_struct->eld_specific_config.aac_spect_data_resil_flag =
              ixheaacd_read_bits_buf(it_bit_buff, 1);
          if (aac_state_struct->audio_object_type == AOT_ER_AAC_LD)
            aac_state_struct->eld_specific_config.ep_config =
                ixheaacd_read_bits_buf(it_bit_buff, 2);
          if (aac_state_struct->audio_object_type == AOT_ER_AAC_ELD)
            aac_state_struct->eld_specific_config.ld_sbr_flag_present =
                ixheaacd_read_bits_buf(it_bit_buff, 1);
        }
      }
    }
  }
  if (pstr_audio_specific_config->audio_object_type == AOT_USAC) {
    {
      pstr_audio_specific_config->sbr_present_flag = 0;
      pstr_audio_specific_config->ext_audio_object_type = 0;
    }

    {
      {
        SIZE_T tmp = 0xf;
        UWORD32 i;
        WORD32 err = 0;

        aac_state_struct->usac_flag = 1;

        ixheaacd_conf_default(&(pstr_audio_specific_config->str_usac_config));
        err = ixheaacd_config(it_bit_buff, &(pstr_audio_specific_config->str_usac_config),
                              &(pstr_audio_specific_config->channel_configuration),
                              aac_state_struct->ec_enable);
        if (err != 0) return err;

        pstr_audio_specific_config->sampling_frequency =
            pstr_audio_specific_config->str_usac_config.usac_sampling_frequency;

        if (pstr_audio_specific_config->audio_object_type == AOT_USAC) {
          pstr_audio_specific_config->sbr_present_flag = 1;
          pstr_audio_specific_config->ext_audio_object_type = AOT_SBR;
          pstr_audio_specific_config->ext_sampling_frequency =
              pstr_audio_specific_config->sampling_frequency;
          pstr_audio_specific_config->ext_samp_frequency_index =
              pstr_audio_specific_config->samp_frequency_index;

          for (i = 0; i < sizeof(ixheaacd_sampl_freq_idx_table) /
                              sizeof(ixheaacd_sampl_freq_idx_table[0]);
               i++) {
            if (ixheaacd_sampl_freq_idx_table[i] ==
                (int)(pstr_audio_specific_config->sampling_frequency)) {
              tmp = i;
              break;
            }
          }
          pstr_audio_specific_config->samp_frequency_index = (UINT32)tmp;
        } else {
          pstr_audio_specific_config->sbr_present_flag = 0;
        }
      }
    }

    if (aac_state_struct->bs_format != LOAS_BSFORMAT) {
      ixheaacd_skip_bits_buf(it_bit_buff, it_bit_buff->cnt_bits);

      if ((SIZE_T)it_bit_buff->ptr_read_next ==
          (SIZE_T)it_bit_buff->ptr_bit_buf_base) {
        *bytes_consumed = ((WORD32)it_bit_buff->size) >> 3;
      } else {
        *bytes_consumed = (WORD32)((((((SIZE_T)it_bit_buff->ptr_read_next -
                              (SIZE_T)it_bit_buff->ptr_bit_buf_base))
                            << 3) +
                            7 - it_bit_buff->bit_pos + 7) >>
                            3);
      }
    }
    return 0;
  }

  aac_state_struct->frame_length = FRAME_SIZE;
  if (aac_state_struct->frame_len_flag)
    aac_state_struct->frame_length = FRAME_SIZE_SMALL;

  if (aac_state_struct->extension_flag)
    aac_state_struct->extension_flag_3 = ixheaacd_read_bits_buf(it_bit_buff, 1);

  if (aac_state_struct->audio_object_type == AOT_ER_AAC_LD)
    aac_state_struct->frame_length >>= 1;

  if (aac_state_struct->audio_object_type == AOT_ER_AAC_ELD) {
    aac_state_struct->frame_length >>= 1;
    if (aac_state_struct->eld_specific_config.ld_sbr_flag_present) {
      aac_state_struct->eld_specific_config.ld_sbr_samp_rate =
          ixheaacd_read_bits_buf(it_bit_buff, 1);
      aac_state_struct->eld_specific_config.ld_sbr_crc_flag =
          ixheaacd_read_bits_buf(it_bit_buff, 1);

      ixheaacd_eld_sbr_header(it_bit_buff, aac_state_struct->ch_config,
                              &aac_state_struct->str_sbr_config);

      aac_state_struct->dwnsmp_signal =
          !aac_state_struct->eld_specific_config.ld_sbr_samp_rate;
    }

    {
      UWORD16 len, eld_ext_type;
      WORD32 err;
      UWORD16 cnt;

      if (ixheaacd_show_bits_buf(it_bit_buff, 4) != ELDEXT_TERM) {
        while ((eld_ext_type = ixheaacd_read_bits_buf(it_bit_buff, 4)) !=
               ELDEXT_TERM) {
          len = ixheaacd_read_bits_buf(it_bit_buff, 4);
          if (len == 15) {
            len += ixheaacd_read_bits_buf(it_bit_buff, 8);
          }
          if (len == 15 + 255) {
            len += ixheaacd_read_bits_buf(it_bit_buff, 16);
          }

          switch (eld_ext_type) {
            case ELDEXT_SAOC:
              break;
            case ELDEXT_LDSAC:
              aac_state_struct->ldmps_present = 1;
              aac_state_struct->mps_dec_handle.ldmps_config.ldmps_present_flag =
                  1;
              aac_state_struct->mps_dec_handle.object_type =
                  aac_state_struct->audio_object_type;
              aac_state_struct->mps_dec_handle.ec_flag = aac_state_struct->ec_enable;
              err = ixheaacd_ld_spatial_specific_config(
                  &(aac_state_struct->mps_dec_handle.ldmps_config), it_bit_buff);
              if (err) return err;

              if (ixheaacd_mps_create(
                      &aac_state_struct->mps_dec_handle,
                      aac_state_struct->mps_dec_handle.ldmps_config
                          .bs_frame_length,
                      0, &(aac_state_struct->mps_dec_handle.ldmps_config))) {
                return -1;
              }
              break;
            default:
              for (cnt = 0; cnt < len; cnt++) {
                ixheaacd_read_bits_buf(it_bit_buff, 8);
              }

              break;
          }
          break;
        }
      }
    }
    if (!((aac_state_struct->ldmps_present == 1) &&
          !(it_bit_buff->cnt_bits >= 1)))
    ixheaacd_read_bits_buf(it_bit_buff, 1);
  }
  if (!((aac_state_struct->ldmps_present == 1) && !(it_bit_buff->cnt_bits > 0))) {
    if (aac_state_struct->audio_object_type == AOT_ER_AAC_ELD ||
        aac_state_struct->audio_object_type == AOT_ER_AAC_LC ||
        aac_state_struct->audio_object_type == AOT_ER_AAC_SCAL) {
      WORD32 ep_config = ixheaacd_read_bits_buf(it_bit_buff, 2);
      if (ep_config == 2 || ep_config == 3) {
      }
      if (ep_config == 3) {
        WORD32 direct_map = ixheaacd_read_bits_buf(it_bit_buff, 1);
        if (!direct_map) {
        }
      }
    }

    tmp = (header_len * 8) - it_bit_buff->cnt_bits;

    if (aac_state_struct->audio_object_type != AOT_SBR &&
        (it_bit_buff->cnt_bits >= 16)) {
      tmp = ixheaacd_read_bits_buf(it_bit_buff, 11);

      if (tmp == 0x2b7) {
        tmp = ixheaacd_read_bits_buf(it_bit_buff, 5);

        if (tmp == AOT_SBR) {
          WORD32 sbr_present_flag = ixheaacd_read_bits_buf(it_bit_buff, 1);
          if (sbr_present_flag) {
            tmp = ixheaacd_get_samp_rate(it_bit_buff, pstr_samp_rate_info,
                                         pstr_audio_specific_config);
            if (tmp == -1) {
              *bytes_consumed = 1;
              return IA_XHEAAC_DEC_INIT_FATAL_DEC_INIT_FAIL;
            } else
              aac_state_struct->extension_samp_rate = tmp;

            if (it_bit_buff->cnt_bits >= 12) {
              tmp = ixheaacd_read_bits_buf(it_bit_buff, 11);
              if (tmp == 0x548) {
                tmp = ixheaacd_read_bits_buf(it_bit_buff, 1);
              }
            }
          }
        }
      } else if (aac_state_struct->bs_format == LOAS_BSFORMAT) {
        ixheaacd_read_bidirection(it_bit_buff, -11);
      }
    }
  }

  if (aac_state_struct->audio_object_type != AOT_AAC_LC &&
      aac_state_struct->audio_object_type != AOT_SBR &&
      aac_state_struct->audio_object_type != AOT_PS &&
      aac_state_struct->audio_object_type != AOT_ER_AAC_LC &&
      aac_state_struct->audio_object_type != AOT_ER_AAC_LD &&
      aac_state_struct->audio_object_type != AOT_ER_AAC_ELD &&
      aac_state_struct->audio_object_type != AOT_AAC_LTP) {
    *bytes_consumed = 1;
    return IA_XHEAAC_DEC_INIT_FATAL_AUDIOOBJECTTYPE_NOT_SUPPORTED;
  } else {
    if (aac_state_struct->bs_format == LOAS_BSFORMAT) {
      *bytes_consumed = (cnt_bits - it_bit_buff->cnt_bits) >> 3;
      if (it_bit_buff->bit_pos < 7) *bytes_consumed += 1;

    } else
      *bytes_consumed = header_len;

    return 0;
  }
}

WORD32 ixheaacd_check_if_adts(ia_adts_header_struct *adts,
                              struct ia_bit_buf_struct *it_bit_buff,
                              WORD32 usr_max_ch) {
  WORD32 max_frm_len_per_ch, result = 0;

  result = ixheaacd_adtsframe(adts, it_bit_buff);

  max_frm_len_per_ch = ixheaac_mult32(768, (adts->no_raw_data_blocks + 1));

  if (adts->channel_configuration != 0)
    max_frm_len_per_ch =
        ixheaac_mult32(max_frm_len_per_ch, adts->channel_configuration);
  else
    max_frm_len_per_ch = max_frm_len_per_ch * usr_max_ch;

  return ((result != 0) || (adts->aac_frame_length < 8) || (adts->layer != 0) ||
          (adts->profile != AAC_LC_PROFILE));
}

IA_ERRORCODE ixheaacd_latm_header_decode(
    ia_aac_dec_state_struct *aac_state_struct,
    struct ia_bit_buf_struct *it_bit_buff, WORD32 *bytes_consumed,
    ia_sampling_rate_info_struct *pstr_samp_rate_info) {
  WORD32 sync;
  IA_ERRORCODE result;
  WORD32 next_sync, audio_mux_len_bytes_last;
  WORD32 audio_mux_len_bits_last;
  WORD32 sync_status = aac_state_struct->sync_status;
  WORD32 bit_count = aac_state_struct->bit_count;
  WORD32 cnt_bits = it_bit_buff->cnt_bits;

  *bytes_consumed = 0;

  aac_state_struct->bs_format = LOAS_BSFORMAT;

  if (sync_status == 0) {
    do {
      sync = ixheaacd_read_bits_buf(it_bit_buff, 11);
      bit_count += 11;
      while (sync != 0x2B7) {
        sync = ((sync & 0x3ff) << 1) | ixheaacd_read_bits_buf(it_bit_buff, 1);
        bit_count += 1;
        if (it_bit_buff->cnt_bits < 13) {
          ixheaacd_read_bidirection(it_bit_buff, -11);
          *bytes_consumed = (cnt_bits - it_bit_buff->cnt_bits) / 8;
          return (IA_XHEAAC_DEC_INIT_NONFATAL_HEADER_NOT_AT_START);
        }
      }

      audio_mux_len_bytes_last = ixheaacd_read_bits_buf(it_bit_buff, 13);
      bit_count += 13;
      audio_mux_len_bits_last = audio_mux_len_bytes_last << 3;
      if (it_bit_buff->cnt_bits >= (audio_mux_len_bits_last + 11)) {
        ixheaacd_read_bidirection(it_bit_buff, audio_mux_len_bits_last);
        bit_count += audio_mux_len_bits_last;

        next_sync = ixheaacd_read_bits_buf(it_bit_buff, 11);
        bit_count += 11;

        if (next_sync == 0x2B7) {
          ixheaacd_read_bidirection(it_bit_buff,
                                    -(11 + audio_mux_len_bits_last + 13 + 11));
          bit_count -= 11 + audio_mux_len_bits_last + 13 + 11;
          break;
        } else {
          ixheaacd_read_bidirection(it_bit_buff,
                                    -(audio_mux_len_bits_last + 24 + 11 - 1));
          bit_count -= audio_mux_len_bits_last + 24 + 11 - 1;
        }

      } else {
        if (it_bit_buff->cnt_bits != audio_mux_len_bits_last) {
          ixheaacd_read_bidirection(it_bit_buff, -(13 + 11));
          bit_count -= (13 + 11);
          *bytes_consumed = (cnt_bits - it_bit_buff->cnt_bits) / 8;
          return IA_XHEAAC_DEC_EXE_NONFATAL_INSUFFICIENT_INPUT_BYTES;
        } else {
          ixheaacd_read_bidirection(it_bit_buff, -(13 + 11));
          bit_count -= (13 + 11);
          break;
        }
      }
    } while (1);

    do {
      WORD32 audio_mux_len_bytes_last;
      WORD32 use_same_stream_mux;

      sync = ixheaacd_read_bits_buf(it_bit_buff, 11);
      bit_count += 11;

      if (sync != 0x2b7) {
        ixheaacd_read_bidirection(it_bit_buff, -25);
        bit_count -= 11;
        *bytes_consumed = (cnt_bits - it_bit_buff->cnt_bits) / 8;
        return IA_XHEAAC_DEC_INIT_NONFATAL_HEADER_NOT_AT_START;
      }

      audio_mux_len_bytes_last = ixheaacd_read_bits_buf(it_bit_buff, 13);
      bit_count += 13;

      use_same_stream_mux = ixheaacd_read_bits_buf(it_bit_buff, 1);
      bit_count += 1;

      if (!use_same_stream_mux) {
        ixheaacd_read_bidirection(it_bit_buff, -25);
        bit_count -= 25;
        sync_status = 1;
        aac_state_struct->sync_status = sync_status;
        break;
      } else {
        ixheaacd_read_bidirection(it_bit_buff, -25);
        bit_count -= 25;
        aac_state_struct->bit_count = bit_count;
        *bytes_consumed = 1;
        return IA_XHEAAC_DEC_EXE_NONFATAL_INSUFFICIENT_INPUT_BYTES;
      }


      ixheaacd_read_bidirection(it_bit_buff, audio_mux_len_bytes_last * 8 - 1);
      bit_count += audio_mux_len_bytes_last * 8 - 1;

    } while (sync_status == 0);

    *bytes_consumed = (cnt_bits - it_bit_buff->cnt_bits) / 8;
    {
      ixheaacd_latm_struct latm_struct_element;
      WORD32 sync;
      memset(&latm_struct_element, 0, sizeof(ixheaacd_latm_struct));

      sync = ixheaacd_read_bits_buf(it_bit_buff, 11);
      if (sync == 0x2b7) {
        result = ixheaacd_latm_audio_mux_element(
            it_bit_buff, &latm_struct_element, aac_state_struct,
            pstr_samp_rate_info);
        if (result != 0) {
          sync_status = 0;
          aac_state_struct->sync_status = sync_status;

          *bytes_consumed += 1;
          return result;
        }
      }
    }
  }
  return IA_NO_ERROR;
}

WORD32 ixheaacd_aac_headerdecode(
    ia_exhaacplus_dec_api_struct *p_obj_exhaacplus_dec, UWORD8 *buffer,
    WORD32 *bytes_consumed,
    const ia_aac_dec_huffman_tables_struct *pstr_huffmann_tables) {
  struct ia_bit_buf_struct it_bit_buff = {0}, *handle_bit_buff;
  ia_adif_header_struct adif = {0};
  ia_adts_header_struct adts = {0};
  WORD32 result;
  WORD32 header_len;
  WORD32 sync = 0;

  WORD32 disable_sync = p_obj_exhaacplus_dec->aac_config.ui_disable_sync;
  WORD32 is_ga_header = p_obj_exhaacplus_dec->aac_config.ui_mp4_flag;

  WORD32 loas_present = p_obj_exhaacplus_dec->aac_config.loas_present;

  ia_aac_dec_state_struct *aac_state_struct =
      p_obj_exhaacplus_dec->pp_mem_aac[IA_ENHAACPLUS_DEC_PERSIST_IDX];
  WORD32 usr_max_ch = aac_state_struct->p_config->ui_max_channels;

  ia_sampling_rate_info_struct *pstr_samp_rate_info =
      (ia_sampling_rate_info_struct *)&pstr_huffmann_tables
          ->str_sample_rate_info[0];

  if (buffer == 0) {
    return IA_XHEAAC_DEC_INIT_FATAL_DEC_INIT_FAIL;
  }

  header_len = aac_state_struct->ui_in_bytes;

  handle_bit_buff = ixheaacd_create_bit_buf(&it_bit_buff, (UWORD8 *)buffer,
                                            (WORD16)header_len);
  handle_bit_buff->cnt_bits += (header_len << 3);
  handle_bit_buff->xaac_jmp_buf = &aac_state_struct->xaac_jmp_buf;

  if (is_ga_header == 1) {
    return ixheaacd_ga_hdr_dec(aac_state_struct, header_len, bytes_consumed,
                               pstr_samp_rate_info, handle_bit_buff);
  } else if (loas_present) {
    return ixheaacd_latm_header_decode(aac_state_struct, &it_bit_buff,
                                       bytes_consumed, pstr_samp_rate_info);
  }

  else {
    WORD32 header_found = 0;
    WORD32 bytes_taken = -1;
    WORD32 prev_offset = 0;
    WORD32 run_once = 1;
    if (disable_sync == 0) run_once = 0;

    do {
      bytes_taken++;
      buffer += (bytes_taken - prev_offset);

      prev_offset = bytes_taken;

      handle_bit_buff = ixheaacd_create_bit_buf(
          &it_bit_buff, (UWORD8 *)buffer, (WORD16)(header_len - bytes_taken));
      handle_bit_buff->cnt_bits += (8 * (header_len - bytes_taken));

      handle_bit_buff->pstr_adts_crc_info = &handle_bit_buff->str_adts_crc_info;
      ixheaacd_adts_crc_open(handle_bit_buff->pstr_adts_crc_info);

      aac_state_struct->ldmps_present = 0;
      aac_state_struct->latm_initialized = 0;
      memset(&aac_state_struct->mps_dec_handle, 0,
             sizeof(aac_state_struct->mps_dec_handle));
      memset(&aac_state_struct->eld_specific_config, 0,
             sizeof(aac_state_struct->eld_specific_config));

      if ((buffer[0] == 'A') && (buffer[1] == 'D') && (buffer[2] == 'I') &&
          (buffer[3] == 'F')) {
        adif.prog_config_present = 0;
        result = ixheaacd_get_adif_header(&adif, handle_bit_buff);
        if (result == 0) {
          if (adif.prog_config_present == 1) {
            aac_state_struct->p_config->ui_pce_found_in_hdr = 1;
            aac_state_struct->p_config->str_prog_config = adif.str_prog_config;
          }
          aac_state_struct->s_adif_hdr_present = 1;
          aac_state_struct->audio_object_type =
              adif.str_prog_config.object_type;
          aac_state_struct->sampling_rate =
              pstr_samp_rate_info[adif.str_prog_config.samp_freq_index]
                  .sampling_frequency;
          aac_state_struct->ch_config = adif.str_prog_config.channels;
          bytes_taken +=
              ((handle_bit_buff->size - handle_bit_buff->cnt_bits) >> 3);

          header_found = 1;
          aac_state_struct->frame_length = FRAME_SIZE;
          if (aac_state_struct->audio_object_type == AOT_ER_AAC_LD)
            aac_state_struct->frame_length >>= 1;
        }
      }

      else if ((sync = ixheaacd_read_bits_buf(&it_bit_buff, 12)) == 0xfff) {
        result = ixheaacd_check_if_adts(&adts, handle_bit_buff, usr_max_ch);
        if (result != 0) {
          continue;
        }

        if ((adts.aac_frame_length + ADTS_HEADER_LENGTH) <
            (header_len - bytes_taken)) {
          ia_adts_header_struct adts_loc = {0};

          ixheaacd_create_init_bit_buf(
              &it_bit_buff, (UWORD8 *)(buffer + adts.aac_frame_length),
              (WORD16)(header_len - bytes_taken - adts.aac_frame_length));

          handle_bit_buff = &it_bit_buff;

          adts_loc.sync_word =
              (WORD16)ixheaacd_read_bits_buf(handle_bit_buff, 12);

          if (adts_loc.sync_word != 0xFFF) {
            continue;
          }

          result =
              ixheaacd_check_if_adts(&adts_loc, handle_bit_buff, usr_max_ch);
          if ((result != 0) ||
              (adts.samp_freq_index != adts_loc.samp_freq_index) ||
              (adts.channel_configuration != adts_loc.channel_configuration)) {
            continue;
          }
        }

        {
          WORD32 obj_type;
          obj_type = adts.profile;

          aac_state_struct->audio_object_type = obj_type;
          aac_state_struct->sampling_rate =
              ((pstr_samp_rate_info[adts.samp_freq_index].sampling_frequency));
          aac_state_struct->ch_config = adts.channel_configuration;
          aac_state_struct->s_adts_hdr_present = 1;
          header_found = 1;
          aac_state_struct->bs_format = ADTS_BSFORMAT;
          aac_state_struct->frame_length = FRAME_SIZE;
          if (aac_state_struct->audio_object_type == AOT_ER_AAC_LD)
            aac_state_struct->frame_length >>= 1;
        }
      } else if (0x2b7 == (sync >> 1)) {
        ixheaacd_read_bidirection(&it_bit_buff, -12);
        result =
            ixheaacd_latm_header_decode(aac_state_struct, &it_bit_buff,
                                        bytes_consumed, pstr_samp_rate_info);
        if (result != 0) {
          if ((result == (WORD32)IA_XHEAAC_DEC_EXE_NONFATAL_INSUFFICIENT_INPUT_BYTES) ||
              (result < 0)) {
            bytes_taken += *bytes_consumed;
            *bytes_consumed = bytes_taken;
            return result;
          } else
            bytes_taken += *bytes_consumed - 1;
          continue;
        }
        header_found = 1;
        aac_state_struct->bs_format = LOAS_BSFORMAT;
        bytes_taken += *bytes_consumed;
      }

    } while ((header_found == 0 && ((bytes_taken + 1) < (header_len - 68))) &&
             run_once != 1);

    if (header_found == 0 && disable_sync == 1) {
      WORD32 err_code;
      ixheaacd_read_bidirection(&it_bit_buff, -12);
      err_code =
          ixheaacd_ga_hdr_dec(aac_state_struct, header_len, bytes_consumed,
                              pstr_samp_rate_info, handle_bit_buff);

      if (err_code == 0) p_obj_exhaacplus_dec->aac_config.ui_mp4_flag = 1;
      return err_code;
    }

    switch (aac_state_struct->audio_object_type) {
        case AOT_AAC_MAIN:
        case AOT_AAC_LC:
        case AOT_AAC_SSR:
        case AOT_AAC_LTP:
        case AOT_AAC_SCAL:
        case AOT_TWIN_VQ:
        case AOT_ER_AAC_LD:
        case AOT_ER_AAC_ELD:
        case AOT_ER_AAC_LC:
        case AOT_USAC:
          break;
        default:
          return IA_XHEAAC_DEC_INIT_FATAL_AUDIOOBJECTTYPE_NOT_SUPPORTED;
      }

    if (aac_state_struct->audio_object_type != AOT_USAC)
      aac_state_struct->usac_flag = 0;
    *bytes_consumed = bytes_taken;

    if ((handle_bit_buff->cnt_bits < 0) &&
        (handle_bit_buff->size <
         (usr_max_ch * (IA_ENHAACPLUS_DEC_INP_BUF_SIZE << 3)))) {
      return (WORD16)(
          (WORD32)IA_XHEAAC_DEC_EXE_NONFATAL_INSUFFICIENT_INPUT_BYTES);
    }

    if (header_found == 0) {
      *bytes_consumed = bytes_taken + 1;
      return IA_XHEAAC_DEC_INIT_NONFATAL_HEADER_NOT_AT_START;
    } else {
      return 0;
    }
  }
}
