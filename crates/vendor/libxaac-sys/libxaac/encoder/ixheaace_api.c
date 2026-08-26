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

#include <stdlib.h>
#include <string.h>
#include <stdlib.h>
#include "ixheaac_type_def.h"
#include "ixheaac_constants.h"
#include "ixheaace_aac_constants.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops16.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaac_basic_ops.h"
/* standard */
#include "ixheaac_error_standards.h"

#include "ixheaace_config_params.h"

/* library */
#include "ixheaace_definitions.h"
#include "ixheaace_error_codes.h"

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
#include "impd_drc_enc.h"

#include "ixheaace_sbr_header.h"
#include "ixheaace_sbr_def.h"
#include "ixheaace_resampler.h"

#include "ixheaace_psy_const.h"
#include "ixheaace_tns.h"
#include "ixheaace_tns_params.h"
#include "ixheaace_rom.h"
#include "ixheaace_common_rom.h"
#include "ixheaace_bitbuffer.h"

#include "ixheaace_sbr_rom.h"
#include "ixheaace_common_rom.h"
#include "ixheaace_sbr_main.h"
#include "ixheaace_definitions.h"
#include "ixheaace_api.h"
#include "ixheaace_memory_standards.h"
#include "iusace_block_switch_const.h"
#include "iusace_block_switch_struct_def.h"
#include "iusace_cnst.h"
#include "iusace_tns_usac.h"
#include "iusace_psy_mod.h"
#include "iusace_config.h"
#include "iusace_arith_enc.h"
#include "ixheaace_version_number.h"

#include "ixheaace_adjust_threshold_data.h"
#include "ixheaace_dynamic_bits.h"
#include "ixheaace_qc_data.h"
#include "ixheaace_channel_map.h"
#include "ixheaace_block_switch.h"
#include "ixheaace_psy_data.h"
#include "ixheaace_interface.h"
#include "ixheaace_write_bitstream.h"
#include "ixheaace_psy_configuration.h"
#include "ixheaace_common_rom.h"

#include "ixheaace_psy_mod.h"
#include "iusace_fd_qc_util.h"
#include "iusace_fd_quant.h"
#include "iusace_ms.h"
#include "iusace_signal_classifier.h"
#include "ixheaace_sbr_header.h"

#include "ixheaace_config.h"
#include "ixheaace_asc_write.h"
#include "iusace_main.h"
#include "ixheaace_stereo_preproc.h"
#include "ixheaace_enc_main.h"
#include "ixheaace_qc_util.h"

// MPS header
#include "ixheaace_mps_common_fix.h"
#include "ixheaace_mps_defines.h"
#include "ixheaace_mps_common_define.h"

#include "ixheaace_mps_struct_def.h"
#include "ixheaace_mps_sac_polyphase.h"
#include "ixheaace_mps_sac_hybfilter.h"
#include "ixheaace_mps_bitstream.h"
#include "ixheaace_mps_spatial_bitstream.h"

#include "ixheaace_mps_buf.h"
#include "ixheaace_mps_lib.h"
#include "ixheaace_mps_main_structure.h"
#include "ixheaace_mps_onset_detect.h"

#include "ixheaace_mps_param_extract.h"

#include "ixheaace_mps_static_gain.h"
#include "ixheaace_mps_filter.h"
#include "ixheaace_mps_delay.h"
#include "ixheaace_mps_dmx_tdom_enh.h"
#include "ixheaace_mps_main_structure.h"
#include "ixheaace_mps_tools_rom.h"
#include "ixheaace_mps_qmf.h"
#include "ixheaace_mps_tree.h"
#include "ixheaace_mps_frame_windowing.h"

#include "ixheaace_mps_structure.h"
#include "ixheaace_mps_memory.h"
#include "ixheaace_mps_enc.h"
#include "ixheaace_struct_def.h"
#include "ixheaace_api_defs.h"

#include "ixheaace_write_adts_adif.h"
#include "ixheaace_loudness_measurement.h"
#include "iusace_psy_utils.h"

static WORD32 iusace_scratch_size(VOID) {
  WORD32 scr_size;
  scr_size = IXHEAAC_GET_SIZE_ALIGNED(USACE_MAX_SCR_SIZE, BYTE_ALIGN_8);
  return scr_size;
}

static WORD32 iusace_calc_pers_buf_sizes(ixheaace_api_struct *pstr_api_struct) {
  WORD32 pers_size = 0;
  ia_usac_encoder_config_struct *pstr_config = &pstr_api_struct->config[0].usac_config;

  pers_size += IXHEAAC_GET_SIZE_ALIGNED(pstr_config->channels * sizeof(FLOAT32 *), BYTE_ALIGN_8);
  pers_size += IXHEAAC_GET_SIZE_ALIGNED(pstr_config->channels * sizeof(FLOAT32 *), BYTE_ALIGN_8);
  pers_size += IXHEAAC_GET_SIZE_ALIGNED(pstr_config->channels * sizeof(FLOAT32 *), BYTE_ALIGN_8);
  pers_size += IXHEAAC_GET_SIZE_ALIGNED(pstr_config->channels * sizeof(FLOAT32 *), BYTE_ALIGN_8);

  pers_size +=
      (IXHEAAC_GET_SIZE_ALIGNED((2 * pstr_config->ccfl * sizeof(FLOAT32)), BYTE_ALIGN_8) *
       pstr_config->channels);
  pers_size += (IXHEAAC_GET_SIZE_ALIGNED((2 * pstr_config->drc_frame_size * sizeof(FLOAT32)),
                                         BYTE_ALIGN_8) *
                pstr_config->channels);
  if (pstr_config->use_delay_adjustment == 1) {
    pers_size +=
        (IXHEAAC_GET_SIZE_ALIGNED(
             ((CC_DELAY_ADJUSTMENT * pstr_config->ccfl) / FRAME_LEN_1024) * sizeof(FLOAT32),
             BYTE_ALIGN_8) *
         pstr_config->channels);
    pers_size += (IXHEAAC_GET_SIZE_ALIGNED(
                      ((CC_DELAY_ADJUSTMENT * pstr_config->drc_frame_size) / FRAME_LEN_1024) *
                          sizeof(FLOAT32),
                      BYTE_ALIGN_8) *
                  pstr_config->channels);
  }

  pers_size +=
      (IXHEAAC_GET_SIZE_ALIGNED((2 * pstr_config->ccfl * sizeof(FLOAT64)), BYTE_ALIGN_8) *
       pstr_config->channels);

  pers_size += (IXHEAAC_GET_SIZE_ALIGNED((pstr_config->ccfl * sizeof(FLOAT64)), BYTE_ALIGN_8) *
                pstr_config->channels);

  pers_size +=
      (IXHEAAC_GET_SIZE_ALIGNED((2 * pstr_config->ccfl * sizeof(FLOAT64)), BYTE_ALIGN_8) *
       pstr_config->channels);

  pers_size +=
      (IXHEAAC_GET_SIZE_ALIGNED((3 * pstr_config->ccfl * sizeof(FLOAT64)), BYTE_ALIGN_8) *
       pstr_config->channels);

  if (pstr_config->tns_select != 0) {
    pers_size +=
        (IXHEAAC_GET_SIZE_ALIGNED(sizeof(ia_tns_info), BYTE_ALIGN_8) * pstr_config->channels);
  }

  pers_size += (IXHEAAC_GET_SIZE_ALIGNED(sizeof(ia_usac_td_encoder_struct), BYTE_ALIGN_8) *
                pstr_config->channels);
  return pers_size;
}

static WORD32 ia_enhaacplus_enc_sizeof_delay_buffer(FLAG flag_framelength_small, WORD32 aot,
                                                    WORD32 resamp_idx, WORD32 delay_buf_size,
                                                    FLAG mps_enable) {
  WORD32 downsample_fac;
  if (resamp_idx > 1) {
    downsample_fac = resamp_idx / 2;
  } else {
    downsample_fac = 1;
  }
  // Set the downsampler delay
  WORD32 max_downsamp_delay, max_upsamp_delay;
  if (resamp_idx == 2)  // 8:3
  {
    max_downsamp_delay = MAXIMUM_DS_8_1_FILTER_DELAY;
    max_upsamp_delay = MAXIMUM_DS_1_3_FILTER_DELAY;
  } else if (resamp_idx == 3)  // 2:1
  {
    max_downsamp_delay = MAXIMUM_DS_2_1_FILTER_DELAY;
    max_upsamp_delay = 0;
  } else if (resamp_idx == 4)  // 4:1
  {
    max_downsamp_delay = MAXIMUM_DS_4_1_FILTER_DELAY;
    max_upsamp_delay = 0;
  } else {
    max_downsamp_delay = MAXIMUM_DS_2_1_FILTER_DELAY;
    max_upsamp_delay = 0;
  }

  if (aot == AOT_SBR || aot == AOT_PS) {
    if (flag_framelength_small)
      return (FRAME_LEN_960 * (1 << downsample_fac) + max_upsamp_delay + max_downsamp_delay +
              INPUT_DELAY_LC) *
             IXHEAACE_MAX_CH_IN_BS_ELE * delay_buf_size;
    else
      return (FRAME_LEN_1024 * (1 << downsample_fac) + max_upsamp_delay + max_downsamp_delay +
              INPUT_DELAY_LC) *
             IXHEAACE_MAX_CH_IN_BS_ELE * delay_buf_size;
  } else if (aot == AOT_AAC_LC) {
    if (flag_framelength_small)
      return (FRAME_LEN_960 * 2 + MAXIMUM_DS_2_1_FILTER_DELAY + INPUT_DELAY_LC) *
             IXHEAACE_MAX_CH_IN_BS_ELE * delay_buf_size;
    else
      return (FRAME_LEN_1024 * 2 + MAXIMUM_DS_2_1_FILTER_DELAY + INPUT_DELAY_LC) *
             IXHEAACE_MAX_CH_IN_BS_ELE * delay_buf_size;
  } else if (aot == AOT_USAC) {
    return (FRAME_LEN_1024 * (1 << downsample_fac) + max_upsamp_delay + max_downsamp_delay +
            INPUT_DELAY_LC) *
           IXHEAACE_MAX_CH_IN_BS_ELE * delay_buf_size;
  } else if (aot == AOT_AAC_LD) {
    if (flag_framelength_small)
      return (FRAME_LEN_480 * 2 + MAXIMUM_DS_2_1_FILTER_DELAY + INPUT_DELAY_LD_480) *
             IXHEAACE_MAX_CH_IN_BS_ELE * delay_buf_size;
    else
      return (FRAME_LEN_512 * 2 + MAXIMUM_DS_2_1_FILTER_DELAY + INPUT_DELAY_LD_512) *
             IXHEAACE_MAX_CH_IN_BS_ELE * delay_buf_size;
  } else if (aot == AOT_AAC_ELD) {
    if (flag_framelength_small) {
      if (mps_enable) {
        return (FRAME_LEN_480 * (1 << downsample_fac) + max_upsamp_delay + max_downsamp_delay +
                INPUT_DELAY_ELDV2_480) *
               IXHEAACE_MAX_CH_IN_BS_ELE * delay_buf_size;
      } else {
        return (FRAME_LEN_480 * (1 << downsample_fac) + max_upsamp_delay + max_downsamp_delay +
                INPUT_DELAY_ELD_480) *
               IXHEAACE_MAX_CH_IN_BS_ELE * delay_buf_size;
      }
    } else {
      if (mps_enable) {
        return (FRAME_LEN_512 * (1 << downsample_fac) + max_upsamp_delay + max_downsamp_delay +
                INPUT_DELAY_ELDV2_512) *
               IXHEAACE_MAX_CH_IN_BS_ELE * delay_buf_size;
      } else {
        return (FRAME_LEN_512 * (1 << downsample_fac) + max_upsamp_delay + max_downsamp_delay +
                INPUT_DELAY_ELD_512) *
               IXHEAACE_MAX_CH_IN_BS_ELE * delay_buf_size;
      }
    }
  } else {
    // return LC Delay buffer size by default
    return (FRAME_LEN_1024 * 2 + MAXIMUM_DS_2_1_FILTER_DELAY + INPUT_DELAY_LC) *
           IXHEAACE_MAX_CH_IN_BS_ELE * delay_buf_size;
  }
}

static VOID ia_enhaacplus_enc_find_slots_for_elements(WORD32 i_channel_mask,
                                                      WORD32 *slots_for_elements,
                                                      WORD32 i_num_coupling_chan) {
  WORD32 slot = 0, i;

  if ((i_channel_mask & 0x3)) {
    slots_for_elements[FRONT_LEFT_RIGHT] = slot;
    slot += 2;
  }

  if ((i_channel_mask & 0x4)) {
    slots_for_elements[FRONT_CENTER] = slot;
    slot += 1;
  }

  if ((i_channel_mask & 0x8)) {
    slots_for_elements[LFE_CHANNEL] = slot;
    slot += 1;
  }

  if ((i_channel_mask & 0x30)) {
    slots_for_elements[BACK_LEFT_RIGHT] = slot;
    slot += 2;
  }

  if ((i_channel_mask & 0x100)) {
    slots_for_elements[REAR_CENTER] = slot;
    slot += 1;
  }

  if (i_num_coupling_chan != 0) {
    for (i = 0; i < i_num_coupling_chan; i++) {
      slots_for_elements[COUPLING_CH + i] = slot;
      slot += 1;
    }
  }

  return;
}

static VOID ia_enhaacplus_enc_find_channel_config(WORD32 *num_bs_elements, WORD32 *chan_config,
                                                  WORD32 *element_type, WORD32 *element_slot,
                                                  WORD32 *element_instance_tag,
                                                  WORD32 i_num_coupling_chan,
                                                  WORD32 i_channel_mask) {
  WORD32 i;
  WORD32 slots_for_elements[2 * MAXIMUM_BS_ELE];
  *num_bs_elements = 0;

  ia_enhaacplus_enc_find_slots_for_elements(i_channel_mask, slots_for_elements,
                                            i_num_coupling_chan);

  if ((i_channel_mask & 0x4)) {
    /*Front Center Present*/
    chan_config[*num_bs_elements] = 1;
    element_type[*num_bs_elements] = ID_SCE;
    element_slot[*num_bs_elements] = slots_for_elements[FRONT_CENTER];
    element_instance_tag[*num_bs_elements] = 0;
    (*num_bs_elements)++;
  }

  if ((i_channel_mask & 0x3)) {
    /*Front Left and Right Present*/
    chan_config[*num_bs_elements] = 2;
    element_type[*num_bs_elements] = ID_CPE;
    element_slot[*num_bs_elements] = slots_for_elements[FRONT_LEFT_RIGHT];
    element_instance_tag[*num_bs_elements] = 0;
    (*num_bs_elements)++;
  }

  if ((i_channel_mask & 0x30)) {
    /*Back Left and Right Present*/
    chan_config[*num_bs_elements] = 2;
    element_type[*num_bs_elements] = ID_CPE;
    element_slot[*num_bs_elements] = slots_for_elements[BACK_LEFT_RIGHT];
    element_instance_tag[*num_bs_elements] = 1;
    (*num_bs_elements)++;
  }

  if ((i_channel_mask & 0x100)) {
    /* Rear Center Present*/
    chan_config[*num_bs_elements] = 1;
    element_type[*num_bs_elements] = ID_SCE;
    element_slot[*num_bs_elements] = slots_for_elements[REAR_CENTER];
    element_instance_tag[*num_bs_elements] = 1;
    (*num_bs_elements)++;
  }

  if ((i_channel_mask & 0x8)) {
    /*LFE channel Present*/
    chan_config[*num_bs_elements] = 1;
    element_type[*num_bs_elements] = ID_LFE;
    element_slot[*num_bs_elements] = slots_for_elements[LFE_CHANNEL];
    element_instance_tag[*num_bs_elements] = 0;
    (*num_bs_elements)++;
  }

  if (i_num_coupling_chan != 0) {
    for (i = 0; i < i_num_coupling_chan; i++) {
      /*Coupling Channel Present*/
      chan_config[*num_bs_elements] = 1;
      element_type[*num_bs_elements] = ID_CCE;
      element_slot[*num_bs_elements] = slots_for_elements[COUPLING_CH + i];
      element_instance_tag[*num_bs_elements] = i_num_coupling_chan - i - 1;
      (*num_bs_elements)++;
    }
  }
}

static VOID ia_enhaacplus_enc_allocate_bitrate_between_channels(
    ixheaace_api_struct *pstr_api_struct, WORD32 *bitrate, WORD32 inp_bitrate) {
  WORD32 ele_idx;
  WORD32 num_lfe = 0, num_mono = 0, num_stereo = 0;
  WORD32 bitrate_per_stereo, bitrate_per_mono;
  for (ele_idx = 0; ele_idx < pstr_api_struct->config[0].num_bs_elements; ele_idx++) {
    switch (pstr_api_struct->config[ele_idx].element_type) {
      case ID_SCE:
      case ID_CCE:
        num_mono++;
        break;
      case ID_CPE:
        num_stereo++;
        break;
      case ID_LFE:
        num_lfe++;
        break;
      default:
        return;
    }
  }
  bitrate_per_stereo = (WORD32)((inp_bitrate - (num_lfe)*8000) / (num_mono * 0.625 + num_stereo));
  bitrate_per_mono = (WORD32)(0.625 * bitrate_per_stereo);
  for (ele_idx = 0; ele_idx < pstr_api_struct->config[0].num_bs_elements; ele_idx++) {
    switch (pstr_api_struct->config[ele_idx].element_type) {
      case ID_SCE:
      case ID_CCE:
        bitrate[ele_idx] = bitrate_per_mono;
        break;
      case ID_CPE:
        bitrate[ele_idx] = bitrate_per_stereo;
        break;
      case ID_LFE:
        bitrate[ele_idx] = 8000;
        break;
      default:
        return;
    }
  }
}

static IA_ERRORCODE ixheaace_validate_channel_mask(WORD32 ch_mask, WORD32 num_ch) {
  IA_ERRORCODE err_code = IA_NO_ERROR;
  // If ch_mask not supported, return error
  WORD32 temp_mask;
  switch (num_ch) {
    case 1:
      temp_mask = CH_MASK_CENTER_FRONT;
      break;
    case 2:
      temp_mask = CH_MASK_LEFT_RIGHT_FRONT;
      break;
    case 3:
      temp_mask = CH_MASK_CENTER_FRONT | CH_MASK_LEFT_RIGHT_FRONT;
      break;
    case 4:
      temp_mask = CH_MASK_CENTER_FRONT | CH_MASK_LEFT_RIGHT_FRONT | CH_MASK_REAR_CENTER;
      break;
    case 5:
      temp_mask = CH_MASK_CENTER_FRONT | CH_MASK_LEFT_RIGHT_FRONT | CH_MASK_LEFT_RIGHT_BACK;
      break;
    case 6:
      temp_mask =
          CH_MASK_CENTER_FRONT | CH_MASK_LEFT_RIGHT_FRONT | CH_MASK_LEFT_RIGHT_BACK | CH_MASK_LFE;
      break;
    default:
      temp_mask = 0;
      break;
  }
  if (ch_mask != temp_mask) {
    err_code =  IA_EXHEAACE_CONFIG_FATAL_CHANNELS_MASK;
  }
  return err_code;
}

static VOID ixheaace_set_default_channel_mask(WORD32 *ch_mask, WORD32 num_ch) {
  switch (num_ch) {
  case 1:
    *ch_mask = CH_MASK_CENTER_FRONT;
    break;
  case 2:
    *ch_mask = CH_MASK_LEFT_RIGHT_FRONT;
    break;
  case 3:
    *ch_mask = (CH_MASK_CENTER_FRONT | CH_MASK_LEFT_RIGHT_FRONT);
    break;
  case 4:
    *ch_mask = (CH_MASK_CENTER_FRONT | CH_MASK_LEFT_RIGHT_FRONT | CH_MASK_REAR_CENTER);
    break;
  case 5:
    *ch_mask = (CH_MASK_CENTER_FRONT | CH_MASK_LEFT_RIGHT_FRONT | CH_MASK_LEFT_RIGHT_BACK);
    break;
  case 6:
    *ch_mask =
      (CH_MASK_CENTER_FRONT | CH_MASK_LEFT_RIGHT_FRONT | CH_MASK_LEFT_RIGHT_BACK | CH_MASK_LFE);
    break;
  }
}

static VOID ixheaace_set_default_config(ixheaace_api_struct *pstr_api_struct,
                                        ixheaace_input_config *pstr_input_config) {
  ia_usac_encoder_config_struct *pstr_usac_config = &pstr_api_struct->config[0].usac_config;
  WORD32 i;

  for (i = 0; i < MAXIMUM_BS_ELE; i++) {
    ia_enhaacplus_enc_aac_init_default_config(&pstr_api_struct->config[i].aac_config,
                                              pstr_input_config->aot);

    pstr_api_struct->config[i].i_channels = NUM_CHANNELS_CONFIG_PARAM_DEFAULT_VALUE;
    pstr_api_struct->config[i].sample_rate = AAC_SAMP_FREQ_CONFIG_PARAM_DEFAULT_VALUE;
    pstr_api_struct->config[i].native_sample_rate = AAC_SAMP_FREQ_CONFIG_PARAM_DEFAULT_VALUE;
    pstr_api_struct->config[i].i_n_memtabs = NUM_MEMTABS_CONFIG_PARAM_DEFAULT_VALUE;
    pstr_api_struct->config[i].aac_classic = AAC_CLASSIC_CONFIG_PARAM_DEFAULT_VALUE;
    pstr_api_struct->config[i].use_parametric_stereo = USE_PS_CONFIG_PARAM_DEFAULT_VALUE;
    pstr_api_struct->config[i].chmode_nchannels = CHMODE_NUM_CHANNELS_CONFIG_PARAM_DEFAULT_VALUE;
    pstr_api_struct->config[i].chmode = CHMODE_CONFIG_PARAM_DEFAULT_VALUE; /*stereo*/
    pstr_api_struct->config[i].adts_flag = ADTS_FLAG_CONFIG_PARAM_DEFAULT_VALUE;
    pstr_api_struct->config[i].num_bs_elements = NUM_BS_ELEMENTS_CONFIG_PARAM_DEFAULT_VALUE;
    pstr_api_struct->config[i].i_channels_mask = CHANNEL_MASK_CONFIG_PARAM_DEFAULT_VALUE;
    pstr_api_struct->config[i].i_num_coupling_chan =
        NUM_COUPLING_CHANNEL_CONFIG_PARAM_DEFAULT_VALUE;
    pstr_api_struct->config[i].element_type = ELEMENT_TYPE_CONFIG_PARAM_DEFAULT_VALUE;
    pstr_api_struct->config[i].element_slot = ELEMENT_SLOT_CONFIG_PARAM_DEFAULT_VALUE;
    pstr_api_struct->config[i].num_bs_elements = NUM_BS_ELEMENTS_CONFIG_PARAM_DEFAULT_VALUE;
    pstr_api_struct->config[i].element_instance_tag =
        ELEMENT_INSTANCE_TAG_CONFIG_PARAM_DEFAULT_VALUE;
    pstr_api_struct->config[i].aac_config.calc_crc = AAC_CFG_CALC_CRC_CONFIG_PARAM_DEFAULT_VALUE;
    pstr_api_struct->config[i].aac_config.full_bandwidth =
        AAC_CFG_FULL_BW_CONFIG_PARAM_DEFAULT_VALUE;
    pstr_api_struct->config[i].eldsbr_found = ELDSBR_FOUND_CONFIG_PARAM_DEFAULT_VALUE;
    pstr_api_struct->config[i].use_mps = USE_MPS_PARAM_DEFAULT_VALUE;
    pstr_api_struct->config[i].mps_tree_config = USE_MPS_TREE_CONFIG_PARAM_DEFAULT_VALUE;
  }
  if (pstr_input_config->aot == AOT_USAC) {
    memset(pstr_usac_config, 0, sizeof(*pstr_usac_config));
    pstr_usac_config->channels = NUM_CHANNELS_CONFIG_PARAM_DEFAULT_VALUE;
    pstr_usac_config->sample_rate = USAC_SAMP_FREQ_CONFIG_PARAM_DEFAULT_VALUE;
    pstr_usac_config->core_sample_rate = USAC_SAMP_FREQ_CONFIG_PARAM_DEFAULT_VALUE;
    pstr_usac_config->native_sample_rate = USAC_SAMP_FREQ_CONFIG_PARAM_DEFAULT_VALUE;
    pstr_usac_config->sbr_pvc_active = USAC_SBR_PVC_DEFAULT_VALUE;
    pstr_usac_config->sbr_inter_tes_active = USAC_SBR_INTER_TES_DEFAULT_VALUE;
    pstr_usac_config->sbr_harmonic = USAC_SBR_HARMONIC_DEFAULT_VALUE;
    pstr_usac_config->bit_rate = USAC_BITRATE_DEFAULT_VALUE;
    pstr_usac_config->use_fill_element = USAC_FILL_ELEMENT_DEFAULT_VALUE;
    pstr_usac_config->use_drc_element = USAC_DRC_DEFAULT_VALUE;
    pstr_usac_config->cmplx_pred_flag = USAC_COMPLEX_PREDECTION_DEFAULT_VALUE;
    pstr_usac_config->tns_select = USAC_TNS_DEFAULT_VALUE;
    pstr_usac_config->flag_noiseFilling = USAC_FLAG_NOISE_FILLING_DEFAULT_VALUE;
    pstr_usac_config->use_acelp_only = USAC_DEFAULT_ACELP_FLAG_VALUE;
    pstr_usac_config->is_first_frame = USAC_FIRST_FRAME_FLAG_DEFAULT_VALUE;
    pstr_usac_config->num_preroll_frames = CC_NUM_PREROLL_FRAMES;
    pstr_usac_config->stream_id = USAC_DEFAULT_STREAM_ID_VALUE;
    pstr_usac_config->use_delay_adjustment = USAC_DEFAULT_DELAY_ADJUSTMENT_VALUE;
    pstr_usac_config->is_loudness_configured = USAC_DEFAULT_MEASURED_LOUDNESS_FLAG_VALUE;
  }
  /* Initialize table pointers */
  ia_enhaacplus_enc_init_aac_tabs(&(pstr_api_struct->pstr_aac_tabs));
  ia_enhaacplus_enc_init_sbr_tabs(&(pstr_api_struct->spectral_band_replication_tabs));
  pstr_api_struct->common_tabs.pstr_common_tab =
      (ixheaace_common_tables *)&ia_enhaacplus_enc_common_tab;
}

static IA_ERRORCODE ixheaace_validate_config_params(ixheaace_input_config *pstr_input_config) {
  IA_ERRORCODE err_code = IA_NO_ERROR;
  if (pstr_input_config->aot != AOT_AAC_ELD && pstr_input_config->aot != AOT_AAC_LC &&
      pstr_input_config->aot != AOT_AAC_LD && pstr_input_config->aot != AOT_PS &&
      pstr_input_config->aot != AOT_SBR && pstr_input_config->aot != AOT_USAC) {
    pstr_input_config->aot = AOT_AAC_LC;
  }
  pstr_input_config->i_native_samp_freq = pstr_input_config->i_samp_freq;
  if (pstr_input_config->aot != AOT_USAC) {
    pstr_input_config->i_samp_freq = iusace_map_sample_rate(pstr_input_config->i_samp_freq);
  } else {
    err_code = iusace_validate_baseline_profile_sample_rate(pstr_input_config->i_samp_freq);
    if (err_code) {
      return err_code;
    }
  }

  if ((pstr_input_config->i_channels < MIN_NUM_CORE_CODER_CHANNELS) ||
      (pstr_input_config->i_channels > MAX_NUM_CORE_CODER_CHANNELS)) {
    pstr_input_config->i_channels = 1;
  }
  if (pstr_input_config->esbr_flag != 1 && pstr_input_config->esbr_flag != 0) {
    pstr_input_config->esbr_flag = 0;
  }
  if ((pstr_input_config->esbr_flag == 1) &&
      ((pstr_input_config->aot != AOT_SBR) && (pstr_input_config->aot != AOT_PS) &&
       (pstr_input_config->aot != AOT_USAC))) {
    pstr_input_config->esbr_flag = 0;
  }
  if (pstr_input_config->i_use_mps != 1 && pstr_input_config->i_use_mps != 0) {
    pstr_input_config->i_use_mps = 0;
  }

  if ((pstr_input_config->i_channels != 2) && (pstr_input_config->i_channels != 6)) {
    pstr_input_config->i_use_mps = 0;
  }
  if (pstr_input_config->aot != AOT_AAC_ELD && pstr_input_config->aot != AOT_USAC) {
    pstr_input_config->i_use_mps = 0;
  }
  if (pstr_input_config->aot == AOT_USAC && pstr_input_config->i_use_mps == 1) {
    if (pstr_input_config->ccfl_idx < SBR_8_3) {
      pstr_input_config->ccfl_idx = SBR_2_1;
    }
  }
  if (AOT_USAC == pstr_input_config->aot) {
    if ((pstr_input_config->i_channels != 2) || (pstr_input_config->i_samp_freq > 48000)) {
      // Num qmf bands is mapped only till 48000. Hence, disable mps if fs > 48000 or if input
      // channels is not 2
      pstr_input_config->i_use_mps = 0;
    }
  }
  if (pstr_input_config->i_use_mps == 1) {
    if (pstr_input_config->i_channels == 2) {
      if (pstr_input_config->i_mps_tree_config != TREE_212) {
        pstr_input_config->i_mps_tree_config = TREE_212;
      }
    } else {
      if (pstr_input_config->i_mps_tree_config != TREE_5151 &&
          pstr_input_config->i_mps_tree_config != TREE_5152 &&
          pstr_input_config->i_mps_tree_config != TREE_525) {
        pstr_input_config->i_mps_tree_config = TREE_5151;
      }
    }
  } else {
    pstr_input_config->i_mps_tree_config = INVALID_TREE_CONFIG;
  }
  if (pstr_input_config->aot == AOT_USAC || pstr_input_config->aot == AOT_AAC_ELD ||
      pstr_input_config->aot == AOT_AAC_LD) {
    pstr_input_config->i_use_adts = 0;
    pstr_input_config->i_use_es = 1;
  }
  if (pstr_input_config->aot == AOT_USAC) {
    if (pstr_input_config->codec_mode != USAC_SWITCHED &&
        pstr_input_config->codec_mode != USAC_ONLY_FD &&
        pstr_input_config->codec_mode != USAC_ONLY_TD) {
      pstr_input_config->codec_mode = USAC_ONLY_FD;
    }
    if (pstr_input_config->ccfl_idx < NO_SBR_CCFL_768 || pstr_input_config->ccfl_idx > SBR_4_1) {
      pstr_input_config->ccfl_idx = NO_SBR_CCFL_1024;  // default value
    }
    if ((pstr_input_config->ccfl_idx == SBR_4_1) && (pstr_input_config->i_samp_freq < 32000))
    {
      if (pstr_input_config->i_samp_freq >= 16000)
      {
        pstr_input_config->ccfl_idx = SBR_2_1;
      }
      else
      {
        pstr_input_config->ccfl_idx = NO_SBR_CCFL_1024;
      }
    }
    if (pstr_input_config->cplx_pred != 1 && pstr_input_config->cplx_pred != 0) {
      pstr_input_config->cplx_pred = 0;
    }
    if (pstr_input_config->use_drc_element != 0 && pstr_input_config->use_drc_element != 1) {
      pstr_input_config->use_drc_element = 0;
    }

    if (pstr_input_config->hq_esbr != 0 && pstr_input_config->hq_esbr != 1) {
      pstr_input_config->hq_esbr = 0;
    }
    if (pstr_input_config->harmonic_sbr != 0 && pstr_input_config->harmonic_sbr != 1) {
      pstr_input_config->harmonic_sbr = 0;
    }
    if (pstr_input_config->pvc_active != 0 && pstr_input_config->pvc_active != 1) {
      pstr_input_config->pvc_active = 0;
    }
    if (pstr_input_config->inter_tes_active != 0 && pstr_input_config->inter_tes_active != 1) {
      pstr_input_config->inter_tes_active = 0;
    }
    if ((pstr_input_config->ccfl_idx != 3 && pstr_input_config->ccfl_idx != 4)) {
      pstr_input_config->harmonic_sbr = 0;
    }
    if (pstr_input_config->harmonic_sbr != 1) {
      pstr_input_config->hq_esbr = 0;
    }
    if (pstr_input_config->i_bitrate != 64000 && pstr_input_config->i_bitrate != 96000) {
      pstr_input_config->i_bitrate = USAC_BITRATE_DEFAULT_VALUE;
    }
    {
      if (pstr_input_config->i_bitrate < MINIMUM_BITRATE * pstr_input_config->i_channels) {
        pstr_input_config->i_bitrate = MINIMUM_BITRATE * pstr_input_config->i_channels;
      }
      if (pstr_input_config->ccfl_idx == NO_SBR_CCFL_768 ||
          pstr_input_config->ccfl_idx == NO_SBR_CCFL_1024) {
        if (pstr_input_config->i_bitrate >
            (WORD32)(6 * pstr_input_config->i_samp_freq * pstr_input_config->i_channels)) {
          pstr_input_config->i_bitrate =
              (6 * pstr_input_config->i_samp_freq * pstr_input_config->i_channels);
        }
      } else if (pstr_input_config->ccfl_idx == SBR_8_3) {
        if (pstr_input_config->i_bitrate >
            (WORD32)(6 * ((pstr_input_config->i_samp_freq * 3) / 8) *
                     pstr_input_config->i_channels)) {
          pstr_input_config->i_bitrate =
              (6 * ((pstr_input_config->i_samp_freq * 3) / 8) * pstr_input_config->i_channels);
        }
      } else if (pstr_input_config->ccfl_idx == SBR_2_1) {
        if (pstr_input_config->i_bitrate >
            (WORD32)(6 * (pstr_input_config->i_samp_freq / 2) * pstr_input_config->i_channels)) {
          pstr_input_config->i_bitrate =
              (6 * (pstr_input_config->i_samp_freq / 2) * pstr_input_config->i_channels);
        }
      } else if (pstr_input_config->ccfl_idx == SBR_4_1) {
        if (pstr_input_config->i_bitrate >
            (WORD32)(6 * (pstr_input_config->i_samp_freq / 4) * pstr_input_config->i_channels)) {
          pstr_input_config->i_bitrate =
              (6 * (pstr_input_config->i_samp_freq / 4) * pstr_input_config->i_channels);
        }
      }
    }

    {
      if ((pstr_input_config->codec_mode == USAC_SWITCHED ||
           pstr_input_config->codec_mode == USAC_ONLY_TD) && pstr_input_config->esbr_flag &&
          pstr_input_config->i_samp_freq > 24000) {
        if (pstr_input_config->ccfl_idx == NO_SBR_CCFL_768) {
          pstr_input_config->ccfl_idx = SBR_8_3;  // Use 8:3 eSBR
        }
        if (pstr_input_config->ccfl_idx == NO_SBR_CCFL_1024) {
          pstr_input_config->ccfl_idx = SBR_2_1;  // Use 2:1 eSBR
        }
      }

      if (pstr_input_config->codec_mode == USAC_ONLY_FD &&
          pstr_input_config->i_samp_freq > 24000 && pstr_input_config->esbr_flag &&
          pstr_input_config->i_bitrate <= MAX_USAC_ESBR_BITRATE) {
        if (pstr_input_config->ccfl_idx == NO_SBR_CCFL_768) {
          pstr_input_config->ccfl_idx = SBR_8_3;  // Use 8:3 eSBR
        }
        if (pstr_input_config->ccfl_idx == NO_SBR_CCFL_1024) {
          pstr_input_config->ccfl_idx = SBR_2_1;  // Use 2:1 eSBR
        }
      }

      if (pstr_input_config->ccfl_idx == NO_SBR_CCFL_768 ||
          pstr_input_config->ccfl_idx == SBR_8_3) {
        pstr_input_config->frame_length = LEN_SUPERFRAME_768;
      } else {
        pstr_input_config->frame_length = LEN_SUPERFRAME;
      }
    }
    if (pstr_input_config->random_access_interval < MIN_RAP_INTERVAL_IN_MS) {
      pstr_input_config->random_access_interval = DEFAULT_RAP_INTERVAL_IN_MS;
    }
    if (pstr_input_config->method_def > MAX_METHOD_DEFINITION_TYPE) {
      pstr_input_config->method_def = METHOD_DEFINITION_PROGRAM_LOUDNESS;
    }
    if (pstr_input_config->measurement_system != MEASUREMENT_SYSTEM_BS_1770_3) {
      pstr_input_config->measurement_system = MEASUREMENT_SYSTEM_BS_1770_3;
    }
    if (pstr_input_config->measured_loudness > MAX_METHOD_VALUE ||
        pstr_input_config->measured_loudness < MIN_METHOD_VALUE) {
      pstr_input_config->measured_loudness = DEFAULT_METHOD_VALUE;
    }
    if (pstr_input_config->sample_peak_level > MAX_SAMPLE_PEAK_LEVEL ||
        pstr_input_config->sample_peak_level < MIN_SAMPLE_PEAK_LEVEL) {
      pstr_input_config->sample_peak_level = DEFAULT_SAMPLE_PEAK_VALUE;
    }
    if (pstr_input_config->use_delay_adjustment != 0 &&
        pstr_input_config->use_delay_adjustment != 1) {
      pstr_input_config->use_delay_adjustment = USAC_DEFAULT_DELAY_ADJUSTMENT_VALUE;
    }
    if (pstr_input_config->use_drc_element) {
      ia_drc_input_config *pstr_drc_cfg = (ia_drc_input_config *)pstr_input_config->pv_drc_cfg;
      err_code = impd_drc_validate_config_params(pstr_drc_cfg);
      if (err_code & IA_FATAL_ERROR) {
        return err_code;
      }
      if (err_code) {
        pstr_input_config->use_drc_element = 0;
        err_code = IA_NO_ERROR;
      }
    }
  } else {
    pstr_input_config->cplx_pred = 0;
    pstr_input_config->harmonic_sbr = 0;
    pstr_input_config->pvc_active = 0;
    pstr_input_config->inter_tes_active = 0;
    pstr_input_config->use_drc_element = 0;
    pstr_input_config->hq_esbr = 0;
    pstr_input_config->use_delay_adjustment = 0;
    if (pstr_input_config->i_channels != 2 && pstr_input_config->aot == AOT_PS) {
      pstr_input_config->aot = AOT_SBR;
    }
    if (pstr_input_config->aac_config.use_tns != 1 &&
        pstr_input_config->aac_config.use_tns != 0) {
      pstr_input_config->aac_config.use_tns = 0;
    }
    if (pstr_input_config->aac_config.noise_filling != 1 &&
        pstr_input_config->aac_config.noise_filling != 0) {
      pstr_input_config->aac_config.noise_filling = 0;
    }
    if (pstr_input_config->i_use_adts != 1 && pstr_input_config->i_use_adts != 0) {
      pstr_input_config->i_use_adts = 0;
      pstr_input_config->i_use_es = 1;
    }
    if (pstr_input_config->aac_config.full_bandwidth != 1 &&
        pstr_input_config->aac_config.full_bandwidth != 0) {
      pstr_input_config->aac_config.full_bandwidth = 0;
    }
    {
      if (pstr_input_config->i_bitrate < MINIMUM_BITRATE * pstr_input_config->i_channels) {
        pstr_input_config->i_bitrate = MINIMUM_BITRATE * pstr_input_config->i_channels;
      }
      if (pstr_input_config->i_bitrate >
          (WORD32)(6 * pstr_input_config->i_samp_freq * pstr_input_config->i_channels)) {
        pstr_input_config->i_bitrate =
            (6 * pstr_input_config->i_samp_freq * pstr_input_config->i_channels);
      }
    }

    {
      if (pstr_input_config->aot == AOT_AAC_LC || pstr_input_config->aot == AOT_SBR ||
          pstr_input_config->aot == AOT_PS) {
        if (pstr_input_config->frame_length != LEN_SUPERFRAME &&
            pstr_input_config->frame_length != FRAME_LEN_960) {
          pstr_input_config->frame_length = LEN_SUPERFRAME;
        }
      } else if (pstr_input_config->aot == AOT_AAC_LD || pstr_input_config->aot == AOT_AAC_ELD) {
        if (pstr_input_config->frame_length != FRAME_LEN_512 &&
            pstr_input_config->frame_length != FRAME_LEN_480) {
          pstr_input_config->frame_length = FRAME_LEN_512;
        }
      } else {
        pstr_input_config->frame_length = LEN_SUPERFRAME;
      }
    }
    if ((pstr_input_config->frame_length == FRAME_LEN_960) &&
        (pstr_input_config->esbr_flag == 1)) {
      pstr_input_config->esbr_flag = 0;
    }
  }
  return err_code;
}

static IA_ERRORCODE ixheaace_set_config_params(ixheaace_api_struct *pstr_api_struct,
                                               ixheaace_input_config *pstr_input_config) {
  WORD32 ele_idx;
  IA_ERRORCODE err_code = IA_NO_ERROR;
  ia_usac_encoder_config_struct *pstr_usac_config = &pstr_api_struct->config[0].usac_config;
  err_code = ixheaace_validate_config_params(pstr_input_config);
  if (err_code) {
    return err_code;
  }

  if (pstr_input_config->ui_pcm_wd_sz != 16) {
    return (IA_EXHEAACE_CONFIG_FATAL_PCM_WDSZ);
  }
  if ((pstr_input_config->aac_config.inv_quant != 0) &&
      (pstr_input_config->aac_config.inv_quant != 1) &&
      (pstr_input_config->aac_config.inv_quant != 2)) {
    return (IA_EXHEAACE_CONFIG_FATAL_QUALITY_LEVEL);
  }
  if ((pstr_input_config->write_program_config_element != 0) &&
      (pstr_input_config->write_program_config_element != 1)) {
    return (IA_EXHEAACE_CONFIG_FATAL_WRITE_PCE);
  }
  if ((pstr_input_config->aac_config.f_no_stereo_preprocessing != 0) &&
      (pstr_input_config->aac_config.f_no_stereo_preprocessing != 1)) {
    return (IA_EXHEAACE_CONFIG_FATAL_USE_STEREO_PRE_PROC);
  }
  if ((pstr_input_config->aac_config.use_tns != 0) &&
      (pstr_input_config->aac_config.use_tns != 1)) {
    return (IA_EXHEAACE_CONFIG_FATAL_USE_TNS);
  }
  if ((pstr_input_config->aac_config.full_bandwidth != 0) &&
      (pstr_input_config->aac_config.full_bandwidth != 1)) {
    return (IA_EXHEAACE_CONFIG_FATAL_USE_FULL_BANDWIDTH);
  }

  for (ele_idx = 0; ele_idx < MAXIMUM_BS_ELE; ele_idx++) {
    pstr_api_struct->config[ele_idx].aac_classic = 1;
    pstr_api_struct->config[ele_idx].firstframe = 1;
    pstr_api_struct->config[ele_idx].aot = pstr_input_config->aot;
    pstr_api_struct->config[ele_idx].adts_flag = pstr_input_config->i_use_adts;
    pstr_api_struct->config[ele_idx].sample_rate = pstr_input_config->i_samp_freq;
    pstr_api_struct->config[ele_idx].native_sample_rate = pstr_input_config->i_native_samp_freq;

    pstr_api_struct->config[ele_idx].i_channels = pstr_input_config->i_channels;
    pstr_api_struct->config[ele_idx].i_native_channels = pstr_input_config->i_channels;
    pstr_api_struct->config[ele_idx].i_channels_mode = pstr_input_config->i_channels;
    pstr_api_struct->config[ele_idx].aac_config.bit_rate = pstr_input_config->i_bitrate;
    pstr_api_struct->config[ele_idx].esbr_flag = pstr_input_config->esbr_flag;
    pstr_api_struct->config[ele_idx].use_mps = pstr_input_config->i_use_mps;
    pstr_api_struct->config[ele_idx].mps_tree_config = pstr_input_config->i_mps_tree_config;
    pstr_api_struct->config[ele_idx].i_num_coupling_chan = pstr_input_config->i_num_coupling_chan;
    pstr_api_struct->config[ele_idx].ui_pcm_wd_sz = pstr_input_config->ui_pcm_wd_sz;
    pstr_api_struct->config[ele_idx].write_program_config_element =
        pstr_input_config->write_program_config_element;
    pstr_api_struct->config[ele_idx].frame_length = pstr_input_config->frame_length;
    pstr_api_struct->config[ele_idx].aac_config.num_stereo_preprocessing =
        pstr_input_config->aac_config.f_no_stereo_preprocessing;

    pstr_api_struct->config[ele_idx].aac_config.use_tns = pstr_input_config->aac_config.use_tns;
    if (pstr_input_config->aot == AOT_AAC_LD || pstr_input_config->aot == AOT_AAC_ELD) {
      pstr_api_struct->config[ele_idx].aac_config.inv_quant =
          pstr_input_config->aac_config.inv_quant;
      if (pstr_input_config->frame_length == FRAME_LEN_512) {
        pstr_api_struct->config[ele_idx].aac_config.flag_framelength_small = 0;
      } else if (pstr_input_config->frame_length == FRAME_LEN_480) {
        pstr_api_struct->config[ele_idx].aac_config.flag_framelength_small = 1;
      }
    } else if (pstr_input_config->aot == AOT_AAC_LC || pstr_input_config->aot == AOT_SBR ||
               pstr_input_config->aot == AOT_PS) {
      pstr_api_struct->config[ele_idx].aac_config.inv_quant = 0;
      if (pstr_input_config->frame_length == FRAME_LEN_1024) {
        pstr_api_struct->config[ele_idx].aac_config.flag_framelength_small = 0;
      } else if (pstr_input_config->frame_length == FRAME_LEN_960) {
        pstr_api_struct->config[ele_idx].aac_config.flag_framelength_small = 1;
      }
    }
    if ((AOT_SBR == pstr_input_config->aot) || (AOT_PS == pstr_input_config->aot) ||
        (AOT_AAC_ELD == pstr_input_config->aot)) {
      pstr_api_struct->config[ele_idx].aac_classic = 0;
    }
    if (pstr_api_struct->config[ele_idx].sample_rate < 32000) {
      if (pstr_api_struct->config[ele_idx].aac_classic == 0) {
        pstr_api_struct->config[ele_idx].aac_classic = 1;
      }
      if (pstr_input_config->aot == AOT_SBR || pstr_input_config->aot == AOT_PS) {
        pstr_input_config->aot = AOT_AAC_LC;
      }
      if (pstr_input_config->aot == AOT_AAC_ELD) {
        pstr_input_config->aot = AOT_AAC_LD;
      }
    }
    pstr_api_struct->config[ele_idx].eldsbr_found =
        !(pstr_api_struct->config[ele_idx].aac_classic);
  }
  if (pstr_input_config->aot == AOT_USAC) {
    if (pstr_input_config->i_channels > 2) {
      return IA_EXHEAACE_CONFIG_FATAL_NUM_CHANNELS;
    }
    if ((pstr_input_config->i_samp_freq < 6000) || (pstr_input_config->i_samp_freq > 96000)) {
      return (IA_EXHEAACE_CONFIG_FATAL_SAMP_FREQ);
    }
    pstr_api_struct->usac_en = 1;

    pstr_usac_config->codec_mode = pstr_input_config->codec_mode;
    pstr_usac_config->channels = pstr_input_config->i_channels;

    pstr_usac_config->core_sample_rate = pstr_input_config->i_samp_freq;
    pstr_usac_config->sample_rate = pstr_input_config->i_samp_freq;
    pstr_usac_config->native_sample_rate = pstr_input_config->i_native_samp_freq;

    pstr_usac_config->ui_pcm_wd_sz = pstr_input_config->ui_pcm_wd_sz;
    pstr_usac_config->ccfl_idx = pstr_input_config->ccfl_idx;
    pstr_usac_config->bit_rate = pstr_input_config->i_bitrate;
    pstr_usac_config->basic_bitrate = pstr_input_config->i_bitrate;
    pstr_usac_config->tns_select = pstr_input_config->aac_config.use_tns;
    pstr_usac_config->cmplx_pred_flag = pstr_input_config->cplx_pred;
    pstr_usac_config->flag_noiseFilling = pstr_input_config->aac_config.noise_filling;
    pstr_usac_config->sbr_pvc_active = pstr_input_config->pvc_active;
    pstr_usac_config->sbr_harmonic = pstr_input_config->harmonic_sbr;
    pstr_usac_config->hq_esbr = pstr_input_config->hq_esbr;
    pstr_usac_config->sbr_inter_tes_active = pstr_input_config->inter_tes_active;
    pstr_api_struct->config[0].chmode_nchannels = pstr_api_struct->config[0].i_channels;

    switch (pstr_input_config->ccfl_idx) {
      case NO_SBR_CCFL_768:
        pstr_usac_config->ccfl = LEN_SUPERFRAME_768;
        pstr_usac_config->in_frame_length = LEN_SUPERFRAME_768;
        pstr_usac_config->sbr_enable = 0;
        pstr_usac_config->drc_frame_size = LEN_SUPERFRAME_768;
        break;
      case NO_SBR_CCFL_1024:
        pstr_usac_config->ccfl = LEN_SUPERFRAME;
        pstr_usac_config->in_frame_length = LEN_SUPERFRAME;
        pstr_usac_config->sbr_enable = 0;
        pstr_usac_config->drc_frame_size = LEN_SUPERFRAME;
        break;
      case SBR_8_3:
        pstr_usac_config->ccfl = LEN_SUPERFRAME_768;
        pstr_usac_config->in_frame_length = (LEN_SUPERFRAME_768 * 8) / 3;
        pstr_usac_config->sbr_enable = 1;
        pstr_usac_config->drc_frame_size = (LEN_SUPERFRAME_768 * 8) / 3;
        break;
      case SBR_2_1:
        pstr_usac_config->ccfl = LEN_SUPERFRAME;
        pstr_usac_config->in_frame_length = (LEN_SUPERFRAME * 2);
        pstr_usac_config->sbr_enable = 1;
        pstr_usac_config->drc_frame_size = (LEN_SUPERFRAME * 2);
        break;
      case SBR_4_1:
        pstr_usac_config->ccfl = LEN_SUPERFRAME;
        pstr_usac_config->in_frame_length = (LEN_SUPERFRAME * 4);
        pstr_usac_config->sbr_enable = 1;
        pstr_usac_config->drc_frame_size = (LEN_SUPERFRAME * 4);
        break;
      default:
        pstr_usac_config->ccfl = LEN_SUPERFRAME;
        pstr_usac_config->in_frame_length = LEN_SUPERFRAME;
        pstr_usac_config->sbr_enable = 0;
        pstr_usac_config->drc_frame_size = LEN_SUPERFRAME;
        break;
    }
    pstr_usac_config->random_access_interval = pstr_input_config->random_access_interval;
    if (pstr_usac_config->random_access_interval > 0) {
      pstr_usac_config->random_access_interval =
          (WORD32)((((WORD64)pstr_usac_config->random_access_interval *
                     pstr_input_config->i_native_samp_freq) +
                    (pstr_usac_config->ccfl * 1000 - 1)) /
                   (pstr_usac_config->ccfl * 1000));
    }
    pstr_usac_config->use_delay_adjustment = pstr_input_config->use_delay_adjustment;
    if (pstr_usac_config->random_access_interval) {
      pstr_usac_config->preroll_flag = 1;
    }
    if (pstr_usac_config->sbr_enable == 1) {
      pstr_usac_config->num_preroll_frames++;
      if (pstr_usac_config->sbr_harmonic == 1) {
        pstr_usac_config->num_preroll_frames++;
      }
    } else {
      if (pstr_usac_config->use_delay_adjustment == 1) {
        pstr_usac_config->num_preroll_frames++;
      }
    }
    pstr_usac_config->stream_id = pstr_input_config->stream_id;
    if (pstr_input_config->ccfl_idx < NO_SBR_CCFL_768 || pstr_input_config->ccfl_idx > SBR_4_1) {
      pstr_api_struct->config[0].ccfl_idx = NO_SBR_CCFL_1024;  // default value
    } else {
      pstr_api_struct->config[0].ccfl_idx = pstr_input_config->ccfl_idx;
    }
    if (pstr_api_struct->config[0].ccfl_idx == SBR_8_3) {
      pstr_api_struct->spectral_band_replication_tabs.ptr_sos_upsamp_tab =
          (ixheaace_resampler_sos_table *)&iixheaace_resamp_1_to_3_filt_params;

      pstr_api_struct->spectral_band_replication_tabs.ptr_sos_downsamp_tab =
          (ixheaace_resampler_sos_table *)&iixheaace_resamp_8_to_1_filt_params;
    } else if (pstr_api_struct->config[0].ccfl_idx == SBR_2_1) {
      pstr_api_struct->spectral_band_replication_tabs.ptr_resamp_tab =
          (ixheaace_resampler_table *)&ixheaace_resamp_2_to_1_iir_filt_params;
    } else if (pstr_api_struct->config[0].ccfl_idx == SBR_4_1) {
      pstr_api_struct->spectral_band_replication_tabs.ptr_resamp_tab =
          (ixheaace_resampler_table *)&ixheaace_resamp_4_to_1_iir_filt_params;
    }
    pstr_usac_config->use_drc_element = pstr_input_config->use_drc_element;
    {
      ia_drc_input_config *pstr_drc_cfg = (ia_drc_input_config *)pstr_input_config->pv_drc_cfg;
      pstr_drc_cfg->str_uni_drc_config.str_channel_layout.base_ch_count =
          pstr_input_config->i_channels;
      pstr_drc_cfg->str_enc_params.sample_rate = pstr_input_config->i_samp_freq;
      pstr_drc_cfg->str_enc_params.domain = TIME_DOMAIN;
      pstr_drc_cfg->str_uni_drc_config.sample_rate = pstr_drc_cfg->str_enc_params.sample_rate;
      if (pstr_usac_config->use_drc_element) {
        for (WORD32 i = 0; i < pstr_drc_cfg->str_uni_drc_config.drc_coefficients_uni_drc_count;
             i++) {
          for (WORD32 j = 0;
               j <
               pstr_drc_cfg->str_uni_drc_config.str_drc_coefficients_uni_drc[i].gain_set_count;
               j++) {
            pstr_drc_cfg->str_uni_drc_config.str_drc_coefficients_uni_drc[i]
                .str_gain_set_params[j]
                .delta_tmin =
                impd_drc_get_delta_t_min(pstr_drc_cfg->str_uni_drc_config.sample_rate);
          }
        }
        for (WORD32 i = 0; i < pstr_drc_cfg->str_uni_drc_config.str_uni_drc_config_ext
                                   .drc_coefficients_uni_drc_v1_count;
             i++) {
          for (WORD32 j = 0; j < pstr_drc_cfg->str_uni_drc_config.str_uni_drc_config_ext
                                     .str_drc_coefficients_uni_drc_v1[i]
                                     .gain_set_count;
               j++) {
            pstr_drc_cfg->str_uni_drc_config.str_uni_drc_config_ext
                .str_drc_coefficients_uni_drc_v1[i]
                .str_gain_set_params[j]
                .delta_tmin =
                impd_drc_get_delta_t_min(pstr_drc_cfg->str_uni_drc_config.sample_rate);
          }
        }
      }
      pstr_usac_config->str_drc_cfg = *pstr_drc_cfg;
      pstr_usac_config->str_drc_cfg.str_enc_params.frame_size = pstr_usac_config->drc_frame_size;
      pstr_usac_config->str_drc_cfg.str_uni_drc_config.str_drc_coefficients_uni_drc
          ->drc_frame_size = pstr_usac_config->drc_frame_size;
      pstr_input_config->drc_frame_size = pstr_usac_config->drc_frame_size;

      ia_drc_loudness_info_set_struct *pstr_enc_loudness_info_set =
          &pstr_usac_config->str_drc_cfg.str_enc_loudness_info_set;

      if ((pstr_usac_config->use_drc_element &&
           ((pstr_enc_loudness_info_set->loudness_info_count != 0) ||
            (pstr_enc_loudness_info_set->loudness_info_album_count != 0) ||
            (pstr_enc_loudness_info_set->str_loudness_info_set_extension
                 .str_loudness_info_set_ext_eq.loudness_info_v1_count != 0) ||
            (pstr_enc_loudness_info_set->str_loudness_info_set_extension
                 .str_loudness_info_set_ext_eq.loudness_info_v1_album_count != 0)))) {
        pstr_usac_config->is_loudness_configured = 1;
      } else {
        pstr_usac_config->is_loudness_configured = 0;
      }
    }
  } else {
    WORD32 max_bitreservoir_size;
    if ((pstr_input_config->i_channels > MAX_NUM_CORE_CODER_CHANNELS)) {
      return (IA_EXHEAACE_CONFIG_FATAL_NUM_CHANNELS);
    }
    if (!((pstr_input_config->i_native_samp_freq == 7350) ||
          (pstr_input_config->i_native_samp_freq == 8000) ||
          (pstr_input_config->i_native_samp_freq == 11025) ||
          (pstr_input_config->i_native_samp_freq == 12000) ||
          (pstr_input_config->i_native_samp_freq == 16000) ||
          (pstr_input_config->i_native_samp_freq == 22050) ||
          (pstr_input_config->i_native_samp_freq == 24000) ||
          (pstr_input_config->i_native_samp_freq == 32000) ||
          (pstr_input_config->i_native_samp_freq == 44100) ||
          (pstr_input_config->i_native_samp_freq == 48000) ||
          (pstr_input_config->i_native_samp_freq == 64000) ||
          (pstr_input_config->i_native_samp_freq == 88200) ||
          (pstr_input_config->i_native_samp_freq == 96000))) {
      return (IA_EXHEAACE_CONFIG_FATAL_SAMP_FREQ);
    }

    if ((pstr_input_config->aot == AOT_AAC_ELD) && (pstr_input_config->i_use_mps == 1) &&
        (pstr_input_config->i_channels > 2)) {
      pstr_api_struct->config[0].num_bs_elements = 1;
      pstr_api_struct->config[0].i_channels = pstr_input_config->i_channels;
      pstr_api_struct->config[0].i_native_channels = pstr_input_config->i_channels;
      pstr_api_struct->config[0].chmode_nchannels = pstr_input_config->i_channels;
      pstr_api_struct->config[0].element_type = ID_SCE;
      if (pstr_api_struct->config[0].mps_tree_config == TREE_525) {
        pstr_api_struct->config[0].element_type = ID_CPE;
      }
      pstr_api_struct->config[0].element_slot = 0;
      pstr_api_struct->config[0].element_instance_tag = 0;
      pstr_api_struct->config[0].aac_config.bit_rate = pstr_input_config->i_bitrate;
      pstr_api_struct->config[0].use_parametric_stereo = 0;
    }
    if (pstr_input_config->i_channels_mask == 0) {
      ixheaace_set_default_channel_mask(&pstr_input_config->i_channels_mask,
                                        pstr_input_config->i_channels);
    }
    if (pstr_api_struct->config[0].aac_config.dual_mono != 1) {
      if (pstr_input_config->aot != AOT_AAC_ELD || (pstr_input_config->i_use_mps != 1)) {
        WORD32 num_bs_elements, chan_config[MAXIMUM_BS_ELE], element_type[MAXIMUM_BS_ELE],
            element_slot[MAXIMUM_BS_ELE], element_instance_tag[MAXIMUM_BS_ELE],
            bitrate[MAXIMUM_BS_ELE];
        if ((pstr_input_config->i_channels_mask > 0x3FFFF)) {
          return (IA_EXHEAACE_CONFIG_FATAL_CHANNELS_MASK);
        }
        if (ixheaace_validate_channel_mask(pstr_input_config->i_channels_mask,
                                           pstr_input_config->i_channels)) {
          return IA_EXHEAACE_CONFIG_FATAL_CHANNELS_MASK;
        }
        for (ele_idx = 0; ele_idx < MAXIMUM_BS_ELE; ele_idx++) {
          pstr_api_struct->config[ele_idx].i_channels_mask = pstr_input_config->i_channels_mask;
        }
        ia_enhaacplus_enc_find_channel_config(
            &num_bs_elements, chan_config, element_type, element_slot, element_instance_tag,
            pstr_api_struct->config[0].i_num_coupling_chan, pstr_input_config->i_channels_mask);
        for (ele_idx = 0; ele_idx < num_bs_elements; ele_idx++) {
          pstr_api_struct->config[ele_idx].i_channels = chan_config[ele_idx];
          pstr_api_struct->config[ele_idx].i_native_channels = chan_config[ele_idx];
          pstr_api_struct->config[ele_idx].chmode_nchannels = chan_config[ele_idx];
          pstr_api_struct->config[ele_idx].element_type = element_type[ele_idx];
          pstr_api_struct->config[ele_idx].element_slot = element_slot[ele_idx];
          pstr_api_struct->config[ele_idx].num_bs_elements = num_bs_elements;
          pstr_api_struct->config[ele_idx].element_instance_tag = element_instance_tag[ele_idx];
        }

        if (pstr_api_struct->config[0].num_bs_elements > 1) {
          ia_enhaacplus_enc_allocate_bitrate_between_channels(pstr_api_struct, bitrate,
                                                              pstr_input_config->i_bitrate);

          for (ele_idx = 0; ele_idx < pstr_api_struct->config[0].num_bs_elements; ele_idx++) {
            pstr_api_struct->config[ele_idx].aac_config.bit_rate = bitrate[ele_idx];
          }

          for (ele_idx = 0; ele_idx < MAXIMUM_BS_ELE; ele_idx++) {
            pstr_api_struct->config[ele_idx].use_parametric_stereo = 0;
          }
        }
      }
    } else {
      WORD32 num_bs_elements;
      WORD32 bitrate[MAXIMUM_BS_ELE];

      num_bs_elements = 2;

      pstr_api_struct->config[0].i_channels = 1;
      pstr_api_struct->config[0].chmode_nchannels = 1;
      pstr_api_struct->config[0].element_type = ID_SCE;
      pstr_api_struct->config[0].element_slot = 0;
      pstr_api_struct->config[0].num_bs_elements = num_bs_elements;
      pstr_api_struct->config[0].element_instance_tag = 0;

      pstr_api_struct->config[1].i_channels = 1;
      pstr_api_struct->config[1].chmode_nchannels = 1;
      pstr_api_struct->config[1].element_type = ID_SCE;
      pstr_api_struct->config[1].element_slot = 1;
      pstr_api_struct->config[1].num_bs_elements = num_bs_elements;
      pstr_api_struct->config[1].element_instance_tag = 1;

      if (pstr_api_struct->config[0].num_bs_elements > 1) {
        ia_enhaacplus_enc_allocate_bitrate_between_channels(
            pstr_api_struct, bitrate, pstr_api_struct->config[0].aac_config.bit_rate);

        for (ele_idx = 0; ele_idx < pstr_api_struct->config[0].num_bs_elements; ele_idx++) {
          pstr_api_struct->config[ele_idx].aac_config.bit_rate = bitrate[ele_idx];
        }

        for (ele_idx = 0; ele_idx < MAXIMUM_BS_ELE; ele_idx++) {
          pstr_api_struct->config[ele_idx].use_parametric_stereo = 0;
        }
      }
    }

    if (pstr_input_config->aot == AOT_PS && pstr_input_config->i_channels == 2) {
      pstr_api_struct->config[0].use_parametric_stereo = 1;
      pstr_api_struct->config[0].chmode_nchannels = 2;
    } else {
      for (ele_idx = 0; ele_idx < pstr_api_struct->config[0].num_bs_elements; ele_idx++) {
        pstr_api_struct->config[ele_idx].chmode_nchannels =
            pstr_api_struct->config[ele_idx].i_channels;
      }
    }

    /* Right shift by 10 as 768 is the max bit reservoir calculated for framelength 1024 */
    max_bitreservoir_size = (BITRESERVOIR_SIZE_CONFIG_PARAM_DEFAULT_VALUE *
                             pstr_api_struct->config[0].frame_length) >>
                            10;
    if ((pstr_input_config->aac_config.bitreservoir_size > max_bitreservoir_size) ||
        (pstr_input_config->aac_config.bitreservoir_size < -1)) {
      pstr_input_config->aac_config.bitreservoir_size = max_bitreservoir_size;
    }
    for (ele_idx = 0; ele_idx < MAXIMUM_BS_ELE; ele_idx++) {
      pstr_api_struct->config[ele_idx].aac_config.bitreservoir_size =
        pstr_input_config->aac_config.bitreservoir_size;
      pstr_api_struct->config[ele_idx].aac_config.full_bandwidth =
        pstr_input_config->aac_config.full_bandwidth;
    }
  }

  return IA_NO_ERROR;
}

static VOID ixheaace_fill_mem_tabs(ixheaace_api_struct *pstr_api_struct, WORD32 aot) {
  WORD32 ele_idx;
  WORD32 num_channel;
  WORD32 frame_length = LEN_SUPERFRAME;
  ixheaace_mem_info_struct *pstr_mem_info;
  frame_length = pstr_api_struct->config[0].frame_length;
  WORD32 offset_size = 0;
  if (pstr_api_struct->usac_en) {
    WORD32 fac_downsample = 1;
    if (pstr_api_struct->config[0].ccfl_idx > NO_SBR_CCFL_1024) {
      fac_downsample = pstr_api_struct->config[0].ccfl_idx >> 1;
    } else {
      fac_downsample = 1;
    }
    /* persistant */
    {
      pstr_mem_info = &pstr_api_struct->pstr_mem_info[IA_ENHAACPLUSENC_PERSIST_IDX];
      {
        pstr_mem_info->ui_size =
            IXHEAAC_GET_SIZE_ALIGNED(sizeof(ixheaace_state_struct), BYTE_ALIGN_8) +
            iusace_calc_pers_buf_sizes(pstr_api_struct);
        if (pstr_api_struct->config[0].usac_config.sbr_enable) {
          pstr_mem_info->ui_size += ixheaace_sbr_enc_pers_size(
              2, 0, pstr_api_struct->config[0].usac_config.sbr_harmonic);
        }
        offset_size = 2 *
                      ia_enhaacplus_enc_sizeof_delay_buffer(
                          0, AOT_USAC, pstr_api_struct->config[0].ccfl_idx,
                          sizeof(pstr_api_struct->pstr_state->inp_delay[0]),
                          pstr_api_struct->config[0].use_mps) *
                      pstr_api_struct->config[0].num_bs_elements;
        pstr_mem_info->ui_size += IXHEAAC_GET_SIZE_ALIGNED(offset_size, BYTE_ALIGN_8);

        if (pstr_api_struct->config[0].use_mps) {
          offset_size =
              (MAX_INPUT_SAMPLES) * sizeof(pstr_api_struct->pstr_state->time_signal_mps[0]);
          pstr_mem_info->ui_size += IXHEAAC_GET_SIZE_ALIGNED(offset_size, BYTE_ALIGN_8);

          offset_size =
              (MAX_MPS_BS_PAYLOAD_SIZE) * sizeof(pstr_api_struct->pstr_state->mps_bs[0]);
          pstr_mem_info->ui_size += IXHEAAC_GET_SIZE_ALIGNED(offset_size, BYTE_ALIGN_8);

          pstr_mem_info->ui_size +=
              IXHEAAC_GET_SIZE_ALIGNED(sizeof(ixheaace_mps_212_memory_struct), BYTE_ALIGN_8);
        }
        if (1 == pstr_api_struct->config[0].usac_config.sbr_enable) {
          offset_size =
              (MAX_FRAME_LEN * (1 << fac_downsample) + MAX_DS_8_1_FILTER_DELAY + INPUT_DELAY) *
              MAX_CHANNELS * sizeof(pstr_mem_info->ui_size);
          pstr_mem_info->ui_size += IXHEAAC_GET_SIZE_ALIGNED(offset_size, BYTE_ALIGN_8);
        }
        if ((2 != pstr_api_struct->config[0].usac_config.channels) &&
            (1 == pstr_api_struct->config[0].usac_config.sbr_enable)) {
          offset_size = (MAX_INPUT_SAMPLES) * sizeof(pstr_api_struct->pstr_state->time_signal[0]);
          pstr_mem_info->ui_size += IXHEAAC_GET_SIZE_ALIGNED(offset_size, BYTE_ALIGN_8);
        }
      }

      pstr_mem_info->ui_alignment = BYTE_ALIGN_8;
      pstr_mem_info->ui_type = IA_MEMTYPE_PERSIST;
      pstr_mem_info->ui_placement[0] = 0;
      pstr_mem_info->ui_placement[1] = 0;
      pstr_mem_info->ui_priority = IA_MEMPRIORITY_ANYWHERE;
      pstr_mem_info->ui_placed[0] = 0;
      pstr_mem_info->ui_placed[1] = 0;
    }

    /* scratch */
    {
      pstr_mem_info = &pstr_api_struct->pstr_mem_info[IA_ENHAACPLUSENC_SCRATCH_IDX];
      UWORD32 usac_scr_size = iusace_scratch_size();
      if (pstr_api_struct->config[0].usac_config.sbr_enable) {
        UWORD32 sbr_scr_size = ixheaace_sbr_enc_scr_size() + ixheaace_resampler_scr_size();
        pstr_mem_info->ui_size = max(usac_scr_size, sbr_scr_size);
      } else {
        pstr_mem_info->ui_size = usac_scr_size;
      }

      pstr_mem_info->ui_alignment = BYTE_ALIGN_8;
      pstr_mem_info->ui_type = IA_MEMTYPE_SCRATCH;
      pstr_mem_info->ui_placement[0] = 0;
      pstr_mem_info->ui_placement[1] = 0;
      pstr_mem_info->ui_priority = IA_MEMPRIORITY_ANYWHERE;
      pstr_mem_info->ui_placed[0] = 0;
      pstr_mem_info->ui_placed[1] = 0;
    }

    /* input */
    {
      pstr_mem_info = &pstr_api_struct->pstr_mem_info[IA_ENHAACPLUSENC_INPUT_IDX];
      WORD32 pcm_wd_sz;
      num_channel = pstr_api_struct->config[0].i_channels;
      pcm_wd_sz = pstr_api_struct->config[0].usac_config.ui_pcm_wd_sz;
      pstr_mem_info->ui_size = frame_length * num_channel * (pcm_wd_sz >> 3);
      if (1 == pstr_api_struct->config[0].usac_config.sbr_enable) {
        switch (pstr_api_struct->config[0].ccfl_idx) {
          case SBR_8_3:  // 8:3
            pstr_mem_info->ui_size *= 8;
            pstr_mem_info->ui_size /= 3;
            break;

          case SBR_2_1:  // 2:1
            pstr_mem_info->ui_size *= 2;
            break;

          case SBR_4_1:  // 4:1
            pstr_mem_info->ui_size *= 4;
            break;
        }
      }

      pstr_mem_info->ui_alignment =
          BYTE_ALIGN_8; /* As input is used as scratch memory internally */
      pstr_mem_info->ui_type = IA_MEMTYPE_INPUT;
      pstr_mem_info->ui_placement[0] = 0;
      pstr_mem_info->ui_placement[1] = 0;
      pstr_mem_info->ui_priority = IA_MEMPRIORITY_ANYWHERE;
      pstr_mem_info->ui_placed[0] = 0;
      pstr_mem_info->ui_placed[1] = 0;
    }

    /* output */
    {
      pstr_mem_info = &pstr_api_struct->pstr_mem_info[IA_ENHAACPLUSENC_OUTPUT_IDX];
      pstr_mem_info->ui_size =
          ((MAX_PREROLL_FRAMES + 1) * (MAX_CHANNEL_BITS / BYTE_NUMBIT) * num_channel) +
          MAX_PREROLL_CONFIG_SIZE;
      pstr_mem_info->ui_alignment = BYTE_ALIGN_8;
      pstr_mem_info->ui_type = IA_MEMTYPE_OUTPUT;
      pstr_mem_info->ui_placement[0] = 0;
      pstr_mem_info->ui_placement[1] = 0;
      pstr_mem_info->ui_priority = IA_MEMPRIORITY_ANYWHERE;
      pstr_mem_info->ui_placed[0] = 0;
      pstr_mem_info->ui_placed[1] = 0;
    }
  } else {
    /* persistant */
    {
      pstr_mem_info = &pstr_api_struct->pstr_mem_info[IA_ENHAACPLUSENC_PERSIST_IDX];
      {
        if (pstr_api_struct->config[0].num_bs_elements == 1) {
          num_channel = pstr_api_struct->config[0].aac_classic
                            ? pstr_api_struct->config[0].chmode_nchannels
                            : (pstr_api_struct->config[0].use_parametric_stereo
                                   ? 1
                                   : pstr_api_struct->config[0].chmode_nchannels);
          pstr_mem_info->ui_size =
              IXHEAAC_GET_SIZE_ALIGNED(sizeof(ixheaace_state_struct), BYTE_ALIGN_8) +
              ia_enhaacplus_enc_aac_enc_pers_size(num_channel, aot);
          if (pstr_api_struct->config[0].aot != AOT_AAC_LC &&
              pstr_api_struct->config[0].aot != AOT_AAC_LD) {
            pstr_mem_info->ui_size += ixheaace_sbr_enc_pers_size(
                num_channel, pstr_api_struct->config[0].use_parametric_stereo, 0);
          }
          offset_size = ia_enhaacplus_enc_sizeof_delay_buffer(
                            pstr_api_struct->config[0].aac_config.flag_framelength_small, aot, 3,
                            sizeof(pstr_api_struct->pstr_state->inp_delay[0]),
                            pstr_api_struct->config[0].use_mps) *
                        pstr_api_struct->config[0].num_bs_elements;
          pstr_mem_info->ui_size += IXHEAAC_GET_SIZE_ALIGNED(offset_size, BYTE_ALIGN_8);
        }
        if (pstr_api_struct->config[0].num_bs_elements > 1) {
          offset_size = IXHEAAC_GET_SIZE_ALIGNED(sizeof(ixheaace_state_struct), BYTE_ALIGN_8) +
                        ia_enhaacplus_enc_sizeof_delay_buffer(
                            pstr_api_struct->config[0].aac_config.flag_framelength_small, aot, 3,
                            sizeof(pstr_api_struct->pstr_state->inp_delay[0]),
                            pstr_api_struct->config[0].use_mps) *
                            pstr_api_struct->config[0].num_bs_elements;
          pstr_mem_info->ui_size += IXHEAAC_GET_SIZE_ALIGNED(offset_size, BYTE_ALIGN_8);
          for (ele_idx = 0; ele_idx < pstr_api_struct->config[0].num_bs_elements; ele_idx++) {
            num_channel = pstr_api_struct->config[ele_idx].i_channels;
            if (pstr_api_struct->config[ele_idx].element_type != ID_LFE)
              pstr_mem_info->ui_size += ixheaace_sbr_enc_pers_size(num_channel, 0, 0);
            pstr_mem_info->ui_size += ia_enhaacplus_enc_aac_enc_pers_size(num_channel, aot) + 32;
          }
        }

        if (pstr_api_struct->config[0].use_mps) {
          if (pstr_api_struct->config[0].aac_config.flag_framelength_small) {
            offset_size =
                (MAX_INPUT_SAMPLES + (INPUT_DELAY_ELDV2_480 * IXHEAACE_MAX_CH_IN_BS_ELE)) *
                sizeof(pstr_api_struct->pstr_state->time_signal_mps[0]);
          } else {
            offset_size =
                (MAX_INPUT_SAMPLES + (INPUT_DELAY_ELDV2_512 * IXHEAACE_MAX_CH_IN_BS_ELE)) *
                sizeof(pstr_api_struct->pstr_state->time_signal_mps[0]);
          }
          pstr_mem_info->ui_size += IXHEAAC_GET_SIZE_ALIGNED(offset_size, BYTE_ALIGN_8);
          if (pstr_api_struct->config[0].mps_tree_config == TREE_212) {
            pstr_mem_info->ui_size +=
                IXHEAAC_GET_SIZE_ALIGNED(sizeof(ixheaace_mps_212_memory_struct), BYTE_ALIGN_8);
          } else {
            pstr_mem_info->ui_size +=
                IXHEAAC_GET_SIZE_ALIGNED(sizeof(ixheaace_mps_515_memory_struct), BYTE_ALIGN_8);
          }
          pstr_mem_info->ui_size += IXHEAAC_GET_SIZE_ALIGNED(
              (MAX_MPS_BS_PAYLOAD_SIZE) * sizeof(pstr_api_struct->pstr_state->mps_bs[0]),
              BYTE_ALIGN_8);
        }

        offset_size = IXHEAACE_MAX_PAYLOAD_SIZE * pstr_api_struct->config[0].num_bs_elements;
        pstr_mem_info->ui_size += IXHEAAC_GET_SIZE_ALIGNED(offset_size, BYTE_ALIGN_8);
        offset_size = (MAX_FRAME_LEN * 2 + MAX_DS_2_1_FILTER_DELAY + INPUT_DELAY) *
                      MAX_INPUT_CHAN * sizeof(pstr_mem_info->ui_size);
        pstr_mem_info->ui_size += IXHEAAC_GET_SIZE_ALIGNED(offset_size, BYTE_ALIGN_8);
      }

      pstr_mem_info->ui_alignment = BYTE_ALIGN_8;
      pstr_mem_info->ui_type = IA_MEMTYPE_PERSIST;
      pstr_mem_info->ui_placement[0] = 0;
      pstr_mem_info->ui_placement[1] = 0;
      pstr_mem_info->ui_priority = IA_MEMPRIORITY_ANYWHERE;
      pstr_mem_info->ui_placed[0] = 0;
      pstr_mem_info->ui_placed[1] = 0;
    }

    /* scratch */
    {
      pstr_mem_info = &pstr_api_struct->pstr_mem_info[IA_ENHAACPLUSENC_SCRATCH_IDX];
      {
        pstr_mem_info->ui_size = ia_enhaacplus_enc_aac_enc_scr_size();
        UWORD32 sbr_scr_size = ixheaace_sbr_enc_scr_size() + ixheaace_resampler_scr_size();
        UWORD32 mps_scr_size = 0;
        if (pstr_api_struct->config[0].use_mps) {
          if (pstr_api_struct->config[0].mps_tree_config != TREE_212) {
            mps_scr_size = ixheaace_mps_515_scratch_size();
          }
        }
        pstr_mem_info->ui_size += MAX(sbr_scr_size, mps_scr_size);
      }
      pstr_mem_info->ui_alignment = BYTE_ALIGN_8;
      pstr_mem_info->ui_type = IA_MEMTYPE_SCRATCH;
      pstr_mem_info->ui_placement[0] = 0;
      pstr_mem_info->ui_placement[1] = 0;
      pstr_mem_info->ui_priority = IA_MEMPRIORITY_ANYWHERE;
      pstr_mem_info->ui_placed[0] = 0;
      pstr_mem_info->ui_placed[1] = 0;
    }

    /* input */
    {
      pstr_mem_info = &pstr_api_struct->pstr_mem_info[IA_ENHAACPLUSENC_INPUT_IDX];

      WORD32 pcm_wd_sz;
      pcm_wd_sz = pstr_api_struct->config[0].ui_pcm_wd_sz;
      num_channel = 0;
      for (ele_idx = 0; ele_idx < pstr_api_struct->config[0].num_bs_elements; ele_idx++) {
        num_channel += pstr_api_struct->config[ele_idx].i_channels;
      }
      {
        if (pstr_api_struct->config[0].aac_classic) {
          pstr_mem_info->ui_size = (frame_length * (pcm_wd_sz >> 3)) * num_channel;
        } else {
          pstr_mem_info->ui_size = (frame_length * (pcm_wd_sz >> 3)) * 2 * num_channel;
        }
      }
      pstr_mem_info->ui_alignment =
          BYTE_ALIGN_8; /* As input is used as scratch memory internally */
      pstr_mem_info->ui_type = IA_MEMTYPE_INPUT;
      pstr_mem_info->ui_placement[0] = 0;
      pstr_mem_info->ui_placement[1] = 0;
      pstr_mem_info->ui_priority = IA_MEMPRIORITY_ANYWHERE;
      pstr_mem_info->ui_placed[0] = 0;
      pstr_mem_info->ui_placed[1] = 0;
    }

    /* output */
    {
      pstr_mem_info = &pstr_api_struct->pstr_mem_info[IA_ENHAACPLUSENC_OUTPUT_IDX];
      if (aot == AOT_AAC_LC || aot == AOT_SBR || aot == AOT_PS) {
        pstr_mem_info->ui_size = (6 * frame_length / 8) * num_channel;
        pstr_mem_info->ui_size += (7) * IXHEAACE_MAX_CH_IN_BS_ELE * MAXIMUM_BS_ELE;
      } else if (aot == AOT_AAC_LD || aot == AOT_AAC_ELD) {
        pstr_mem_info->ui_size = (6 * frame_length / 8) * num_channel;
      }
      pstr_mem_info->ui_alignment = BYTE_ALIGN_8;
      pstr_mem_info->ui_type = IA_MEMTYPE_OUTPUT;
      pstr_mem_info->ui_placement[0] = 0;
      pstr_mem_info->ui_placement[1] = 0;
      pstr_mem_info->ui_priority = IA_MEMPRIORITY_ANYWHERE;
      pstr_mem_info->ui_placed[0] = 0;
      pstr_mem_info->ui_placed[1] = 0;
    }
  }

  return;
}

static WORD32 get_drc_config_size(ixheaace_api_struct *pstr_api_struct,
                                  ixheaace_input_config *ptr_in_cfg) {
  WORD32 bit_count = 0;
  WORD32 total_byte_cnt = 0;

  ia_drc_enc_state *pstr_drc_state =
      &pstr_api_struct->pstr_state->str_usac_enc_data.str_drc_state;
  ia_drc_input_config *pstr_in_drc_cfg = (ia_drc_input_config *)ptr_in_cfg->pv_drc_cfg;

  memset(pstr_drc_state, 0, sizeof(*pstr_drc_state));

  pstr_drc_state->str_enc_params = pstr_in_drc_cfg->str_enc_params;
  pstr_drc_state->str_uni_drc_config = pstr_in_drc_cfg->str_uni_drc_config;
  pstr_drc_state->str_gain_enc.str_loudness_info_set = pstr_in_drc_cfg->str_enc_loudness_info_set;
  pstr_drc_state->str_enc_gain_extension = pstr_in_drc_cfg->str_enc_gain_extension;
  pstr_drc_state->str_gain_enc.str_uni_drc_config = pstr_in_drc_cfg->str_uni_drc_config;
  pstr_drc_state->drc_scratch_mem =
      pstr_api_struct->pstr_state->str_usac_enc_data.str_scratch.ptr_scratch_buf;
  pstr_drc_state->str_gain_enc.base_ch_count = ptr_in_cfg->i_channels;

  //uniDrc payload size
  impd_drc_write_uni_drc_config(pstr_drc_state, &bit_count, 0);
  total_byte_cnt += ((bit_count + 7) >> 3);
  bit_count = 0;

  // LoudnessInfo payload size
  impd_drc_write_loudness_info_set(pstr_drc_state, NULL, &bit_count, 0);
  total_byte_cnt += ((bit_count + 7) >> 3);

  return total_byte_cnt;
}

static IA_ERRORCODE ixheaace_alloc_and_assign_mem(ixheaace_api_struct *pstr_api_struct,
                                                  ixheaace_output_config *ptr_out_cfg,
                                                  ixheaace_input_config *ptr_in_cfg) {
  IA_ERRORCODE err_code = IA_NO_ERROR;
  UWORD32 i_idx;
  pVOID pv_value;
  for (i_idx = 0; i_idx < 4; i_idx++) {
    if (i_idx == IA_ENHAACPLUSENC_OUTPUT_IDX &&
        pstr_api_struct->config[0].usac_config.use_drc_element) {
      WORD32 drc_config_size_expected =
          get_drc_config_size(pstr_api_struct, ptr_in_cfg);
      if (drc_config_size_expected > MAX_DRC_CONFIG_SIZE_EXPECTED) {
        return IA_EXHEAACE_CONFIG_FATAL_DRC_INVALID_CONFIG;
      }
      pstr_api_struct->pstr_mem_info[i_idx].ui_size += drc_config_size_expected;
    }
    ptr_out_cfg->mem_info_table[i_idx].ui_size = pstr_api_struct->pstr_mem_info[i_idx].ui_size;
    ptr_out_cfg->mem_info_table[i_idx].ui_alignment =
        pstr_api_struct->pstr_mem_info[i_idx].ui_alignment;
    ptr_out_cfg->mem_info_table[i_idx].ui_type = pstr_api_struct->pstr_mem_info[i_idx].ui_type;

    ptr_out_cfg->arr_alloc_memory[ptr_out_cfg->malloc_count] =
        ptr_out_cfg->malloc_xheaace(ptr_out_cfg->mem_info_table[i_idx].ui_size +
                                        ptr_out_cfg->mem_info_table[i_idx].ui_alignment,
                                    ptr_out_cfg->mem_info_table[i_idx].ui_alignment);

    if (NULL == ptr_out_cfg->arr_alloc_memory[ptr_out_cfg->malloc_count]) {
      return IA_EXHEAACE_API_FATAL_MEM_ALLOC;
    }
    memset(ptr_out_cfg->arr_alloc_memory[ptr_out_cfg->malloc_count], 0,
           ptr_out_cfg->mem_info_table[i_idx].ui_size);
    if ((i_idx == IA_ENHAACPLUSENC_PERSIST_IDX) || (i_idx == IA_ENHAACPLUSENC_SCRATCH_IDX)) {
      ptr_out_cfg->ui_rem =
          (SIZE_T)((SIZE_T)ptr_out_cfg->arr_alloc_memory[ptr_out_cfg->malloc_count] %
                   ptr_out_cfg->mem_info_table[i_idx].ui_alignment);

      pv_value = ptr_out_cfg->mem_info_table[i_idx].mem_ptr =
          (pVOID)((WORD8 *)ptr_out_cfg->arr_alloc_memory[ptr_out_cfg->malloc_count] +
                  ptr_out_cfg->mem_info_table[i_idx].ui_alignment - ptr_out_cfg->ui_rem);
    } else {
      pv_value = ptr_out_cfg->mem_info_table[i_idx].mem_ptr =
          ptr_out_cfg->arr_alloc_memory[ptr_out_cfg->malloc_count];
    }

    pstr_api_struct->pp_mem[i_idx] = ptr_out_cfg->mem_info_table[i_idx].mem_ptr;
    memset(pstr_api_struct->pp_mem[i_idx], 0, pstr_api_struct->pstr_mem_info[i_idx].ui_size);

    pstr_api_struct->pp_mem[i_idx] = pv_value;

    if (i_idx == IA_ENHAACPLUSENC_PERSIST_IDX) {
      WORD32 offset_size = IXHEAAC_GET_SIZE_ALIGNED(sizeof(ixheaace_state_struct), BYTE_ALIGN_8);
      WORD8 *p_offset = NULL;

      /* Set persistent memory pointer in api obj */
      pstr_api_struct->pstr_state = (ixheaace_state_struct *)pv_value;
      WORD32 i, inp_delay_size;
      WORD8 *p_temp;
      if (pstr_api_struct->usac_en) {
        memset(pstr_api_struct->pstr_state, 0, sizeof(*(pstr_api_struct->pstr_state)));
        ia_usac_encoder_config_struct *pstr_usac_config = &pstr_api_struct->config[0].usac_config;
        ixheaace_state_struct *pstr_state = pstr_api_struct->pstr_state;
        ia_usac_data_struct *pstr_usac_enc_data = &(pstr_state->str_usac_enc_data);

        pstr_state->ptr_in_buf = (FLOAT32 **)((WORD8 *)pstr_state + offset_size);

        offset_size = pstr_usac_config->channels * sizeof(FLOAT32 *);
        p_offset = (WORD8 *)pstr_state->ptr_in_buf +
                   IXHEAAC_GET_SIZE_ALIGNED(offset_size, BYTE_ALIGN_8);

        // Input delay
        pstr_state->inp_delay = (FLOAT32 *)(p_offset);
        inp_delay_size =
            2 *
            ia_enhaacplus_enc_sizeof_delay_buffer(
                0, AOT_USAC, pstr_api_struct->config[0].ccfl_idx,
                sizeof(pstr_state->inp_delay[0]), pstr_api_struct->config[0].use_mps) *
            pstr_api_struct->config[0].num_bs_elements;
        memset(pstr_state->inp_delay, 0, inp_delay_size);
        p_offset += IXHEAAC_GET_SIZE_ALIGNED(inp_delay_size, BYTE_ALIGN_8);
        if (1 == pstr_usac_config->sbr_enable) {
          if (2 != pstr_usac_config->channels) {
            pstr_api_struct->pstr_state->time_signal = (FLOAT32 *)(p_offset);

            memset(pstr_api_struct->pstr_state->time_signal, 0,
                   (MAX_INPUT_SAMPLES) * sizeof(pstr_api_struct->pstr_state->time_signal[0]));
            offset_size =
                (MAX_INPUT_SAMPLES) * sizeof(pstr_api_struct->pstr_state->time_signal[0]);
            p_offset += IXHEAAC_GET_SIZE_ALIGNED(offset_size, BYTE_ALIGN_8);
          }

          pstr_api_struct->pstr_state->spectral_band_replication_enc_pers_mem[0] =
              (struct ixheaace_str_sbr_enc *)p_offset;
          p_offset = p_offset + ixheaace_sbr_enc_pers_size(pstr_usac_config->channels, 0,
                                                           pstr_usac_config->sbr_harmonic);
        }
        if (1 == pstr_api_struct->config[0].use_mps) {
          pstr_api_struct->pstr_state->time_signal_mps = (FLOAT32 *)(p_offset);

          memset(pstr_api_struct->pstr_state->time_signal_mps, 0,
                 (MAX_INPUT_SAMPLES) * sizeof(pstr_api_struct->pstr_state->time_signal_mps[0]));
          offset_size =
              (MAX_INPUT_SAMPLES) * sizeof(pstr_api_struct->pstr_state->time_signal_mps[0]);
          p_offset += IXHEAAC_GET_SIZE_ALIGNED(offset_size, BYTE_ALIGN_8);
          pstr_api_struct->pstr_state->mps_bs = (UWORD8 *)(p_offset);

          memset(pstr_api_struct->pstr_state->mps_bs, 0,
                 (MAX_MPS_BS_PAYLOAD_SIZE) * sizeof(pstr_api_struct->pstr_state->mps_bs[0]));

          offset_size =
              (MAX_MPS_BS_PAYLOAD_SIZE) * sizeof(pstr_api_struct->pstr_state->mps_bs[0]);
          p_offset += IXHEAAC_GET_SIZE_ALIGNED(offset_size, BYTE_ALIGN_8);

          pstr_api_struct->pstr_state->mps_pers_mem = (ixheaace_mps_212_memory_struct *)p_offset;
          p_offset +=
              IXHEAAC_GET_SIZE_ALIGNED(sizeof(ixheaace_mps_212_memory_struct), BYTE_ALIGN_8);
        } else {
          pstr_api_struct->pstr_state->mps_bs = NULL;
        }
        if (1 == pstr_usac_config->use_drc_element) {
          pstr_state->pp_drc_in_buf = (FLOAT32 **)((WORD8 *)p_offset);
          offset_size = pstr_usac_config->channels * sizeof(pstr_state->pp_drc_in_buf[0]);
          p_offset += IXHEAAC_GET_SIZE_ALIGNED(offset_size, BYTE_ALIGN_8);
          p_temp = p_offset;

          for (i = 0; i < pstr_usac_config->channels; i++) {
            pstr_state->pp_drc_in_buf[i] = (FLOAT32 *)(p_offset);
            p_offset += IXHEAAC_GET_SIZE_ALIGNED(
                (pstr_usac_config->drc_frame_size * sizeof(pstr_state->pp_drc_in_buf[0][0]) * 2),
                BYTE_ALIGN_8);
            if (pstr_usac_config->use_delay_adjustment == 1) {
              p_offset += IXHEAAC_GET_SIZE_ALIGNED(
                  ((CC_DELAY_ADJUSTMENT * pstr_usac_config->drc_frame_size) / FRAME_LEN_1024) *
                      sizeof(pstr_state->pp_drc_in_buf[0][0]),
                  BYTE_ALIGN_8);
            }
          }
          memset(p_temp, 0, (p_offset - p_temp));
        }
        p_temp = p_offset;

        for (i = 0; i < pstr_usac_config->channels; i++) {
          pstr_state->ptr_in_buf[i] = (FLOAT32 *)(p_offset);
          p_offset += IXHEAAC_GET_SIZE_ALIGNED((pstr_usac_config->ccfl * sizeof(FLOAT32) * 2),
                                               BYTE_ALIGN_8);
          if (pstr_usac_config->use_delay_adjustment) {
            p_offset += IXHEAAC_GET_SIZE_ALIGNED(
                ((CC_DELAY_ADJUSTMENT * pstr_usac_config->ccfl) / FRAME_LEN_1024) *
                    sizeof(FLOAT32),
                BYTE_ALIGN_8);
          }
        }
        memset(p_temp, 0, (p_offset - p_temp));

        p_temp = p_offset;
        for (i = 0; i < pstr_usac_config->channels; i++) {
          pstr_usac_enc_data->ptr_time_data[i] = (FLOAT64 *)(p_offset);
          p_offset += IXHEAAC_GET_SIZE_ALIGNED((2 * (pstr_usac_config->ccfl) * sizeof(FLOAT64)),
                                                BYTE_ALIGN_8);
        }

        for (i = 0; i < pstr_usac_config->channels; i++) {
          pstr_usac_enc_data->ptr_look_ahead_time_data[i] = (FLOAT64 *)(p_offset);
          p_offset +=
              IXHEAAC_GET_SIZE_ALIGNED((pstr_usac_config->ccfl * sizeof(FLOAT64)), BYTE_ALIGN_8);
        }

        for (i = 0; i < pstr_usac_config->channels; i++) {
          pstr_usac_enc_data->spectral_line_vector[i] = (FLOAT64 *)(p_offset);
          p_offset += IXHEAAC_GET_SIZE_ALIGNED((2 * pstr_usac_config->ccfl * sizeof(FLOAT64)),
                                                BYTE_ALIGN_8);
        }

        for (i = 0; i < pstr_usac_config->channels; i++) {
          pstr_usac_enc_data->ptr_2frame_time_data[i] = (FLOAT64 *)(p_offset);
          p_offset += IXHEAAC_GET_SIZE_ALIGNED((3 * pstr_usac_config->ccfl * sizeof(FLOAT64)),
                                                BYTE_ALIGN_8);
        }
        memset(p_temp, 0, p_offset - p_temp);

        if (pstr_usac_config->tns_select != 0) {
          p_temp = p_offset;
          for (i = 0; i < pstr_usac_config->channels; i++) {
            pstr_usac_enc_data->pstr_tns_info[i] = (ia_tns_info *)(p_offset);
            p_offset += IXHEAAC_GET_SIZE_ALIGNED(sizeof(ia_tns_info), BYTE_ALIGN_8);
          }
          memset(p_temp, 0, p_offset - p_temp);
        }

        p_temp = p_offset;
        for (i = 0; i < pstr_usac_config->channels; i++) {
          pstr_usac_enc_data->td_encoder[i] = (ia_usac_td_encoder_struct *)(p_offset);
          p_offset += IXHEAAC_GET_SIZE_ALIGNED(sizeof(ia_usac_td_encoder_struct), BYTE_ALIGN_8);
        }
        memset(p_temp, 0, p_offset - p_temp);
      } else {
        WORD32 num_aac_chan;
        ixheaace_state_struct *pstr_state = pstr_api_struct->pstr_state;
        memset(pstr_api_struct->pstr_state, 0, sizeof(*(pstr_api_struct->pstr_state)));

        pstr_api_struct->pstr_state->inp_delay = (FLOAT32 *)((WORD8 *)pstr_state + offset_size);
        offset_size = ia_enhaacplus_enc_sizeof_delay_buffer(
                          pstr_api_struct->config[0].aac_config.flag_framelength_small,
                          pstr_api_struct->config[0].aot, 3,
                          sizeof(pstr_api_struct->pstr_state->inp_delay[0]),
                          pstr_api_struct->config[0].use_mps) *
                      pstr_api_struct->config[0].num_bs_elements;
        p_offset = (WORD8 *)pstr_api_struct->pstr_state->inp_delay +
                   IXHEAAC_GET_SIZE_ALIGNED(offset_size, BYTE_ALIGN_8);

        if (pstr_api_struct->config[0].use_mps) {
          pstr_api_struct->pstr_state->time_signal_mps = (FLOAT32 *)(p_offset);

          memset(pstr_api_struct->pstr_state->time_signal_mps, 0,
                 (MAX_INPUT_SAMPLES + (INPUT_DELAY_ELDV2_512 * IXHEAACE_MAX_CH_IN_BS_ELE)) *
                     sizeof(pstr_api_struct->pstr_state->time_signal_mps[0]));
          offset_size =
              (MAX_INPUT_SAMPLES + (INPUT_DELAY_ELDV2_512 * IXHEAACE_MAX_CH_IN_BS_ELE)) *
              sizeof(pstr_api_struct->pstr_state->time_signal_mps[0]);
          p_offset += IXHEAAC_GET_SIZE_ALIGNED(offset_size, BYTE_ALIGN_8);

          pstr_api_struct->pstr_state->mps_bs = (UWORD8 *)(p_offset);
          memset(pstr_api_struct->pstr_state->mps_bs, 0,
                 (MAX_MPS_BS_PAYLOAD_SIZE) * sizeof(pstr_api_struct->pstr_state->mps_bs[0]));
          offset_size =
              (MAX_MPS_BS_PAYLOAD_SIZE) * sizeof(pstr_api_struct->pstr_state->mps_bs[0]);
          p_offset += IXHEAAC_GET_SIZE_ALIGNED(offset_size, BYTE_ALIGN_8);

          if (pstr_api_struct->config[0].mps_tree_config == TREE_212) {
            pstr_api_struct->pstr_state->mps_pers_mem =
                (ixheaace_mps_212_memory_struct *)p_offset;
            p_offset +=
                IXHEAAC_GET_SIZE_ALIGNED(sizeof(ixheaace_mps_212_memory_struct), BYTE_ALIGN_8);
          } else {
            pstr_api_struct->pstr_state->mps_515_pers_mem =
                (ixheaace_mps_515_memory_struct *)p_offset;
            p_offset +=
                IXHEAAC_GET_SIZE_ALIGNED(sizeof(ixheaace_mps_515_memory_struct), BYTE_ALIGN_8);
          }
        } else {
          pstr_api_struct->pstr_state->mps_bs = NULL;
        }

        for (WORD32 ele_idx = 0; ele_idx < pstr_api_struct->config[0].num_bs_elements;
             ele_idx++) {
          num_aac_chan = pstr_api_struct->config[ele_idx].aac_classic
                             ? pstr_api_struct->config[ele_idx].chmode_nchannels
                             : (pstr_api_struct->config[ele_idx].use_parametric_stereo
                                    ? 1
                                    : pstr_api_struct->config[ele_idx].chmode_nchannels);

          /* Set aac enc persistent memory pointer in api obj */
          pstr_api_struct->pstr_state->aac_enc_pers_mem[ele_idx] =
              (iexheaac_encoder_str *)p_offset;
          p_offset = p_offset + ia_enhaacplus_enc_aac_enc_pers_size(
                                    num_aac_chan, pstr_api_struct->config[ele_idx].aot);

          if (pstr_api_struct->config[ele_idx].aot != AOT_AAC_LC &&
              pstr_api_struct->config[ele_idx].aot != AOT_AAC_LD) {
            if (pstr_api_struct->config[ele_idx].element_type != ID_LFE) {
              /* Set spectral_band_replication_ enc persistent memory pointer in api obj */
              pstr_api_struct->pstr_state->spectral_band_replication_enc_pers_mem[ele_idx] =
                  (struct ixheaace_str_sbr_enc *)p_offset;

              p_offset =
                  p_offset +
                  ixheaace_sbr_enc_pers_size(
                      num_aac_chan, pstr_api_struct->config[ele_idx].use_parametric_stereo, 0);
            }
          }
        }
      }
    }

    if ((i_idx == IA_MEMTYPE_SCRATCH) && pstr_api_struct->usac_en) {
      pstr_api_struct->pstr_state->str_usac_enc_data.str_scratch.ptr_scratch_buf =
          (UWORD8 *)pstr_api_struct->pp_mem[IA_MEMTYPE_SCRATCH];
      memset(pstr_api_struct->pp_mem[IA_MEMTYPE_SCRATCH], 0,
             pstr_api_struct->pstr_mem_info[i_idx].ui_size);
    }
    if (i_idx == IA_ENHAACPLUSENC_INPUT_IDX) {
      ptr_out_cfg->ui_inp_buf_size = ptr_out_cfg->mem_info_table[i_idx].ui_size;
    }
    ptr_out_cfg->malloc_count++;
  }
  return err_code;
}

static VOID ixheaace_write_audio_preroll_data(ixheaace_api_struct *pstr_api_struct,
                                              ia_bit_buf_struct *it_bit_buff) {
  ixheaace_config_struct *pstr_config = &pstr_api_struct->config[0];
  ixheaace_state_struct *pstr_enc_state = pstr_api_struct->pstr_state;
  ia_usac_data_struct *pstr_usac_data = &pstr_enc_state->str_usac_enc_data;
  ia_usac_encoder_config_struct *pstr_usac_config = &pstr_config->usac_config;
  WORD32 i, j, padding_bits;

  if (pstr_usac_config->is_ipf) {
    if (pstr_usac_config->iframes_interval == pstr_usac_config->num_preroll_frames) {
      WORD32 config_len = 0, num_bits = 0, au_len = 0, config_bits = 0;
      WORD32 bytes_to_write;
      UWORD8 *ptr_out = (UWORD8 *)pstr_api_struct->pp_mem[IA_MEMTYPE_OUTPUT];
      WORD32 max_output_size =
          ((MAX_CHANNEL_BITS / BYTE_NUMBIT) * pstr_usac_config->channels) * MAX_PREROLL_FRAMES +
          MAX_PREROLL_CONFIG_SIZE;
      UWORD8 *out_data = ptr_out + max_output_size;
      UWORD8 residual_bits = 0, residual_data = 0;
      memmove(ptr_out + max_output_size, ptr_out, pstr_enc_state->i_out_bytes);
      pstr_usac_config->is_ipf = 0;

      config_bits = ixheaace_get_usac_config_bytes(NULL, &pstr_enc_state->audio_specific_config,
                                                   pstr_config->ccfl_idx);
      config_len = (config_bits + 7) >> 3;
      num_bits = iusace_write_escape_value(NULL, config_len, 4, 4, 8);
      num_bits += (config_len * 8);  // config data bits
      num_bits++;                    // apply-crossfade
      num_bits++;                    // apr-reserved
      // bits for number of preroll frames
      num_bits += iusace_write_escape_value(NULL, pstr_usac_config->num_preroll_frames, 2, 4, 0);
      // bits for au_len
      for (i = 0; i < pstr_usac_config->num_preroll_frames; i++) {
        num_bits += iusace_write_escape_value(NULL, pstr_usac_data->prev_out_bytes[i], 16, 16, 0);
        au_len += pstr_usac_data->prev_out_bytes[i];
      }
      iusace_reset_bit_buffer(it_bit_buff);
      // total bytes to write
      bytes_to_write = (num_bits + 7) >> 3;
      // usacIndependencyFlag
      iusace_write_bits_buf(it_bit_buff, pstr_usac_data->usac_independency_flag, 1);
      iusace_write_bits_buf(it_bit_buff, 1, 1);  // usacExtElementPresent
      iusace_write_bits_buf(it_bit_buff, 0, 1);  // usacExtElementUseDefaultLength

      if (au_len + bytes_to_write >= MAXIMUM_VALUE_8BIT) {
        iusace_write_escape_value(it_bit_buff, au_len + bytes_to_write + 2, 8, 16, 0);
      } else {
        iusace_write_bits_buf(it_bit_buff, au_len + bytes_to_write, 8);
      }

      iusace_write_escape_value(it_bit_buff, config_len, 4, 4, 8);  // configLen
      // Config
      ixheaace_get_usac_config_bytes(it_bit_buff, &pstr_enc_state->audio_specific_config,
                                     pstr_config->ccfl_idx);

      if (config_bits % 8) {
        iusace_write_bits_buf(it_bit_buff, 0, (UWORD8)((config_len << 3) - config_bits));
      }

      iusace_write_bits_buf(it_bit_buff, 0, 1);  // applyCrossfade
      iusace_write_bits_buf(it_bit_buff, 0, 1);  // apr_reserved
      // numPreRollFrames
      iusace_write_escape_value(it_bit_buff, pstr_usac_config->num_preroll_frames, 2, 4, 0);
      for (i = 0; i < pstr_usac_config->num_preroll_frames; i++) {
        au_len = pstr_usac_data->prev_out_bytes[i];

        if (pstr_usac_config->iframes_interval != 0) {
          out_data = pstr_usac_data->prev_out_data[i];
        }

        // auLen
        iusace_write_escape_value(it_bit_buff, au_len, 16, 16, 0);

        // AccessUnit
        for (j = 0; j < au_len; j++) {
          iusace_write_bits_buf(it_bit_buff, *out_data, 8);
          out_data++;
        }
      }

      if (num_bits % 8) {
        iusace_write_bits_buf(it_bit_buff, 0, (UWORD8)((bytes_to_write << 3) - num_bits));
      }
      // current frame
      au_len = pstr_enc_state->i_out_bits >> 3;
      residual_bits = (UWORD8)(pstr_enc_state->i_out_bits - (au_len << 3));
      out_data = ptr_out + max_output_size;
      for (j = 0; j < au_len; j++) {
        iusace_write_bits_buf(it_bit_buff, *out_data, 8);
        out_data++;
      }
      residual_data = *out_data >> (8 - residual_bits);
      iusace_write_bits_buf(it_bit_buff, residual_data, residual_bits);

      padding_bits = 8 - (it_bit_buff->cnt_bits & 7);
      if (padding_bits > 0 && padding_bits < 8) {
        ptr_out[it_bit_buff->cnt_bits >> 3] =
            (WORD8)((UWORD32)ptr_out[it_bit_buff->cnt_bits >> 3]) & (0xFF << padding_bits);
      }
      pstr_enc_state->i_out_bytes = (it_bit_buff->cnt_bits + 7) >> 3;
      pstr_usac_config->preroll_idx++;

      if (!pstr_usac_config->is_first_frame) {
        pstr_usac_config->preroll_idx = pstr_usac_config->num_preroll_frames + 1;
      }
      if (pstr_usac_config->is_first_frame) {
        pstr_usac_config->is_first_frame = 0;
      }
    } else {
      if (pstr_usac_config->preroll_idx < pstr_usac_config->num_preroll_frames) {
        WORD32 *ptr_prev_out_bytes = pstr_usac_data->prev_out_bytes;
        WORD32 pr_idx = pstr_usac_config->preroll_idx;
        UWORD8 *ptr_out = (UWORD8 *)pstr_api_struct->pp_mem[IA_MEMTYPE_OUTPUT];
        ptr_prev_out_bytes[pr_idx] = pstr_enc_state->i_out_bytes;
        memcpy(pstr_usac_data->prev_out_data[pr_idx++], ptr_out, pstr_enc_state->i_out_bytes);
        pstr_usac_config->preroll_idx = pr_idx;
        pstr_enc_state->i_out_bytes = 0;
      }
    }
  } else {
    for (j = 0; j < pstr_usac_config->num_preroll_frames - 1; j++) {
      pstr_usac_data->prev_out_bytes[j] = pstr_usac_data->prev_out_bytes[j + 1];
    }
    if (pstr_usac_config->num_preroll_frames) {
      pstr_usac_data->prev_out_bytes[pstr_usac_config->num_preroll_frames - 1] =
          pstr_enc_state->i_out_bytes;
    }
    pstr_usac_config->preroll_idx = pstr_usac_config->num_preroll_frames + 1;
  }
  return;
}

static IA_ERRORCODE ia_usac_enc_init(ixheaace_api_struct *pstr_api_struct, WORD32 ccfl_idx) {
  IA_ERRORCODE error = IA_NO_ERROR;
  WORD32 i = 0;
  ixheaace_config_struct *pstr_config = &pstr_api_struct->config[0];
  ixheaace_state_struct *pstr_enc_state = pstr_api_struct->pstr_state;
  ia_usac_data_struct *pstr_enc_data = &pstr_enc_state->str_usac_enc_data;
  ia_usac_encoder_config_struct *pstr_usac_config = &pstr_config->usac_config;

  pstr_usac_config->bit_rate = pstr_api_struct->config[0].aac_config.bit_rate;

  if ((pstr_usac_config->codec_mode == USAC_SWITCHED) ||
      (pstr_usac_config->codec_mode == USAC_ONLY_FD)) {
    pstr_usac_config->aac_allow_scalefacs = 1;
    if (pstr_usac_config->aac_scale_facs == 0) pstr_usac_config->aac_allow_scalefacs = 0;
  }

  if (pstr_usac_config->codec_mode == USAC_ONLY_TD) {
    for (i = 0; i < pstr_config->i_channels; i++) {
      pstr_enc_data->core_mode_prev[i] = CORE_MODE_FD;
      pstr_enc_data->core_mode[i] = CORE_MODE_TD;
    }
  } else {
    for (i = 0; i < pstr_config->i_channels; i++) {
      pstr_enc_data->core_mode_prev[i] = CORE_MODE_FD;
      pstr_enc_data->core_mode[i] = CORE_MODE_FD;
    }
  }

  if (1 == pstr_usac_config->use_drc_element) {
    pstr_enc_data->str_scratch.drc_scratch = pstr_enc_data->str_scratch.ptr_scratch_buf;
  }
  if (pstr_usac_config->sbr_enable) {
    WORD8 *sbr_scr_ptr = (WORD8 *)pstr_enc_data->str_scratch.ptr_scratch_buf;
    ixheaace_audio_specific_config_struct *pstr_asc = &pstr_enc_state->audio_specific_config;
    ixheaace_str_sbr_cfg spectral_band_replication_config;
    // SBR defaults
    iaace_config *pstr_sbr_config = &(pstr_api_struct->config[0].aac_config);
    WORD32 sbr_ratio = 0;
    WORD32 samples = pstr_usac_config->ccfl;
    // Set scratch buffers for SBR and resampler
    pstr_api_struct->pstr_state->temp_buff_sbr =
        (WORD8 *)pstr_enc_data->str_scratch.ptr_scratch_buf;
    pstr_api_struct->pstr_state->ptr_temp_buff_resamp =
        (WORD8 *)pstr_enc_data->str_scratch.ptr_scratch_buf + ixheaace_sbr_enc_scr_size();

    ixheaace_initialize_sbr_defaults(&spectral_band_replication_config);
    // Set SBR codec as USAC
    spectral_band_replication_config.sbr_codec = USAC_SBR;
    spectral_band_replication_config.sbr_pvc_active = pstr_usac_config->sbr_pvc_active;
    spectral_band_replication_config.sbr_harmonic = pstr_usac_config->sbr_harmonic;
    spectral_band_replication_config.hq_esbr = pstr_usac_config->hq_esbr;
    pstr_usac_config->core_sample_rate = pstr_usac_config->sample_rate / 2;
    switch (pstr_usac_config->ccfl_idx) {
      case SBR_4_1:
        spectral_band_replication_config.sbr_ratio_idx = USAC_SBR_RATIO_INDEX_4_1;
        spectral_band_replication_config.sbr_pvc_rate = USAC_SBR_DOWNSAMPLE_RATIO_4_1;
        pstr_usac_config->core_sample_rate = pstr_usac_config->sample_rate / 4;
        sbr_ratio = 4;
        samples *= 4;
        break;
      case SBR_8_3:
        spectral_band_replication_config.sbr_ratio_idx = USAC_SBR_RATIO_INDEX_8_3;
        spectral_band_replication_config.sbr_pvc_rate = USAC_SBR_DOWNSAMPLE_RATIO_2_1;
        sbr_ratio = 2;
        samples *= 8;
        samples /= 3;
        break;
      case SBR_2_1:
        spectral_band_replication_config.sbr_ratio_idx = USAC_SBR_RATIO_INDEX_2_1;
        spectral_band_replication_config.sbr_pvc_rate = USAC_SBR_DOWNSAMPLE_RATIO_2_1;
        sbr_ratio = 2;
        samples *= 2;
        break;
      default:
        spectral_band_replication_config.sbr_ratio_idx = USAC_SBR_RATIO_NO_SBR;
        spectral_band_replication_config.sbr_pvc_rate = 2;
        sbr_ratio = 2;
        break;
    }
    if (pstr_api_struct->pstr_state->mps_enable) {
      ixheaace_mps_212_memory_struct *pstr_mps_memory;
      pstr_mps_memory = pstr_api_struct->pstr_state->mps_pers_mem;
      ixheaace_mps_212_open(&pstr_api_struct->pstr_mps_212_enc, pstr_mps_memory);
      pstr_asc->str_aac_config.num_sac_cfg_bits = 0;

      error = ixheaace_mps_212_initialise(
          pstr_api_struct->pstr_mps_212_enc, AOT_USAC, pstr_usac_config->sample_rate,
          &pstr_sbr_config->bit_rate, sbr_ratio, (WORD32)samples, samples, 515 * sbr_ratio,
          (WORD8 *)pstr_api_struct->pstr_state->ptr_temp_buff_resamp);
      if (error) {
        return error;
      }

      pstr_asc->str_aac_config.num_sac_cfg_bits = ixheaace_mps_212_get_spatial_specific_config(
          pstr_api_struct->pstr_mps_212_enc, (WORD8 *)pstr_asc->str_aac_config.sac_cfg_data,
          sizeof(pstr_asc->str_aac_config.sac_cfg_data), AOT_USAC);
    }
    ixheaace_adjust_sbr_settings(
        &spectral_band_replication_config, pstr_sbr_config->bit_rate,
        (pstr_api_struct->pstr_state->mps_enable != 1) ? pstr_config->i_channels : 1,
        pstr_usac_config->core_sample_rate, AACENC_TRANS_FAC, 24000,
        pstr_api_struct->spectral_band_replication_tabs.ptr_qmf_tab,
        pstr_api_struct->pstr_state->aot, (pstr_api_struct->config[0].ccfl_idx == SBR_4_1));

    error = ixheaace_env_open(
        &pstr_api_struct->pstr_state->spectral_band_replication_enc_pers_mem[0],
        &spectral_band_replication_config, &pstr_sbr_config->band_width, sbr_scr_ptr,
        &(pstr_api_struct->spectral_band_replication_tabs), &pstr_asc->str_aac_config.sbr_config);
    if (error) {
      return error;
    }

    if (pstr_api_struct->config[0].ccfl_idx >= 2) {
      pstr_api_struct->pstr_state->downsample[0] = 1;
    } else {
      pstr_api_struct->pstr_state->downsample[0] = 0;
    }

    if (pstr_api_struct->pstr_state->downsample[0]) {
      IA_ERRORCODE resamp_error = IA_NO_ERROR;
      WORD32 resamp_ratio = 0, upsamp_fac = 0, downsamp_fac = 0;
      WORD32 ele_idx = 0, ch_idx = 0;

      if (pstr_api_struct->config[0].ccfl_idx == SBR_8_3) {
        upsamp_fac = 3;
        downsamp_fac = 8;
        pstr_usac_config->sample_rate /= 2;
      } else if (pstr_api_struct->config[0].ccfl_idx == SBR_2_1) {
        resamp_ratio = 2;
        pstr_usac_config->sample_rate /= 2;
      } else if (pstr_api_struct->config[0].ccfl_idx == SBR_4_1) {
        resamp_ratio = 4;
        pstr_usac_config->sample_rate /= 4;
      }

      if (pstr_api_struct->config[0].ccfl_idx == SBR_8_3) {
        if (upsamp_fac != 3 || downsamp_fac != 8) {
          return IA_EXHEAACE_CONFIG_FATAL_USAC_RESAMPLER_RATIO;
        }
      } else {
        if (resamp_ratio != 2 && resamp_ratio != 4) {
          return IA_EXHEAACE_CONFIG_FATAL_USAC_RESAMPLER_RATIO;
        }
      }
      if (pstr_api_struct->config[0].ccfl_idx == SBR_8_3) {  // Upsampler initialization
        resamp_error = ia_enhaacplus_enc_init_iir_sos_resampler(
            &(pstr_api_struct->pstr_state->up_sampler[ele_idx][ch_idx]), upsamp_fac,
            pstr_api_struct->spectral_band_replication_tabs.ptr_sos_upsamp_tab);
        if (resamp_error) {
          return resamp_error;
        }
        if (pstr_api_struct->config[0].i_channels > 1) {
          resamp_error = ia_enhaacplus_enc_init_iir_sos_resampler(
              &(pstr_api_struct->pstr_state->up_sampler[ele_idx][ch_idx + 1]), upsamp_fac,
              pstr_api_struct->spectral_band_replication_tabs.ptr_sos_upsamp_tab);
          if (resamp_error) {
            return resamp_error;
          }
        }
        if (pstr_usac_config->sbr_harmonic) {
          resamp_error = ia_enhaacplus_enc_init_iir_sos_resampler(
              &(pstr_api_struct->pstr_state->hbe_up_sampler[ele_idx][ch_idx]), upsamp_fac,
              pstr_api_struct->spectral_band_replication_tabs.ptr_sos_upsamp_tab);
          if (resamp_error) {
            return resamp_error;
          }
          if (pstr_api_struct->config[0].i_channels > 1) {
            resamp_error = ia_enhaacplus_enc_init_iir_sos_resampler(
                &(pstr_api_struct->pstr_state->hbe_up_sampler[ele_idx][ch_idx + 1]), upsamp_fac,
                pstr_api_struct->spectral_band_replication_tabs.ptr_sos_upsamp_tab);
            if (resamp_error) {
              return resamp_error;
            }
          }
        }
        // Downsampler initialization
        resamp_error = ia_enhaacplus_enc_init_iir_sos_resampler(
            &(pstr_api_struct->pstr_state->down_samp_sos[ele_idx][ch_idx]), downsamp_fac,
            pstr_api_struct->spectral_band_replication_tabs.ptr_sos_downsamp_tab);
        if (resamp_error) {
          return resamp_error;
        }
        if (pstr_api_struct->config[0].i_channels > 1) {
          resamp_error = ia_enhaacplus_enc_init_iir_sos_resampler(
              &(pstr_api_struct->pstr_state->down_samp_sos[ele_idx][ch_idx + 1]), downsamp_fac,
              pstr_api_struct->spectral_band_replication_tabs.ptr_sos_downsamp_tab);
          if (resamp_error) {
            return resamp_error;
          }
        }
        if (pstr_usac_config->sbr_harmonic) {
          resamp_error = ia_enhaacplus_enc_init_iir_sos_resampler(
              &(pstr_api_struct->pstr_state->hbe_down_samp_sos[ele_idx][ch_idx]), downsamp_fac,
              pstr_api_struct->spectral_band_replication_tabs.ptr_sos_downsamp_tab);
          if (resamp_error) {
            return resamp_error;
          }
          if (pstr_api_struct->config[0].i_channels > 1) {
            resamp_error = ia_enhaacplus_enc_init_iir_sos_resampler(
                &(pstr_api_struct->pstr_state->hbe_down_samp_sos[ele_idx][ch_idx + 1]),
                downsamp_fac,
                pstr_api_struct->spectral_band_replication_tabs.ptr_sos_downsamp_tab);
            if (resamp_error) {
              return resamp_error;
            }
          }
        }
      } else if (pstr_api_struct->config[0].ccfl_idx == SBR_2_1 ||
                 pstr_api_struct->config[0].ccfl_idx == SBR_4_1) {
        resamp_error = ia_enhaacplus_enc_init_iir_resampler(
            &(pstr_api_struct->pstr_state->down_sampler[ele_idx][ch_idx]), resamp_ratio,
            pstr_api_struct->spectral_band_replication_tabs.ptr_resamp_tab);
        if (resamp_error) {
          return resamp_error;
        }
        if (pstr_api_struct->config[0].i_channels > 1) {
          resamp_error = ia_enhaacplus_enc_init_iir_resampler(
              &(pstr_api_struct->pstr_state->down_sampler[ele_idx][ch_idx + 1]), resamp_ratio,
              pstr_api_struct->spectral_band_replication_tabs.ptr_resamp_tab);
          if (resamp_error) {
            return resamp_error;
          }
        }
        if (pstr_usac_config->sbr_harmonic) {
          resamp_error = ia_enhaacplus_enc_init_iir_resampler(
              &(pstr_api_struct->pstr_state->hbe_down_sampler[ele_idx][ch_idx]), resamp_ratio,
              pstr_api_struct->spectral_band_replication_tabs.ptr_resamp_tab);
          if (resamp_error) {
            return resamp_error;
          }
          if (pstr_api_struct->config[0].i_channels > 1) {
            resamp_error = ia_enhaacplus_enc_init_iir_resampler(
                &(pstr_api_struct->pstr_state->hbe_down_sampler[ele_idx][ch_idx + 1]),
                resamp_ratio, pstr_api_struct->spectral_band_replication_tabs.ptr_resamp_tab);
            if (resamp_error) {
              return resamp_error;
            }
          }
        }
      }
    }
  }

  error = iusace_enc_init(pstr_usac_config, &pstr_api_struct->pstr_state->audio_specific_config,
                          &pstr_api_struct->pstr_state->str_usac_enc_data);
  if (error & IA_FATAL_ERROR) {
    return error;
  }
  pstr_api_struct->pstr_state->str_usac_enc_data.frame_count = 0;
  pstr_usac_config->is_ipf = 1;
  pstr_enc_data->stereo_config_index = (pstr_enc_state->mps_enable == 1) ? 2 : 0;
  ia_bit_buf_struct *pstr_ia_asc_bit_buf;
  pstr_ia_asc_bit_buf = iusace_create_bit_buffer(
      &(pstr_api_struct->pstr_state->str_bit_buf), pstr_api_struct->pp_mem[IA_MEMTYPE_OUTPUT],
      pstr_api_struct->pstr_mem_info[IA_MEMTYPE_OUTPUT].ui_size, 1);
  if (pstr_usac_config->sbr_enable) {
    for (UWORD32 idx = 0; idx < pstr_usac_config->num_elements; idx++) {
      switch (pstr_enc_state->audio_specific_config.str_usac_config.usac_element_type[idx]) {
        case ID_USAC_SCE:
        case ID_USAC_CPE:
          pstr_enc_state->audio_specific_config.str_usac_config.str_usac_element_config[idx]
              .stereo_config_index = pstr_enc_data->stereo_config_index;
          pstr_enc_state->audio_specific_config.str_usac_config.str_usac_element_config[idx]
              .str_usac_sbr_config.bs_inter_tes = pstr_usac_config->sbr_inter_tes_active;
          pstr_enc_state->audio_specific_config.str_usac_config.str_usac_element_config[idx]
              .str_usac_sbr_config.bs_pvc = pstr_usac_config->sbr_pvc_active;
          pstr_enc_state->audio_specific_config.str_usac_config.str_usac_element_config[idx]
              .str_usac_sbr_config.dflt_header_extra1 = 0;
          pstr_enc_state->audio_specific_config.str_usac_config.str_usac_element_config[idx]
              .str_usac_sbr_config.dflt_header_extra2 = 0;
          pstr_enc_state->audio_specific_config.str_usac_config.str_usac_element_config[idx]
              .str_usac_sbr_config.dflt_start_freq = 0;
          pstr_enc_state->audio_specific_config.str_usac_config.str_usac_element_config[idx]
              .str_usac_sbr_config.dflt_stop_freq = 4;
          pstr_enc_state->audio_specific_config.str_usac_config.str_usac_element_config[idx]
              .str_usac_sbr_config.harmonic_sbr = pstr_usac_config->sbr_harmonic;
          break;
        default:
          continue;
      }
    }
  }

  ixheaace_get_audiospecific_config_bytes(pstr_ia_asc_bit_buf,
                                          &pstr_api_struct->pstr_state->audio_specific_config,
                                          AOT_USAC, ccfl_idx);
  pstr_api_struct->pstr_state->i_out_bytes = (pstr_ia_asc_bit_buf->cnt_bits + 7) >> 3;

  return IA_NO_ERROR;
}

static IA_ERRORCODE ia_enhaacplus_enc_init(ixheaace_api_struct *pstr_api_struct, WORD32 ele_idx) {
  IA_ERRORCODE error = IA_NO_ERROR;
  WORD32 anc_flag = 0;
  WORD32 anc_rate = -1;
  WORD32 anc_mode = 0;
  WORD32 core_ch;
  WORD32 asc_channels;
  WORD8 *ptr_resampler_scratch;
  iaace_scratch *pstr_aac_scratch;
  WORD8 *ptr_spectral_band_replication_scratch;
  UWORD32 core_sample_rate;
  ixheaace_state_struct *pstr_enc_state = pstr_api_struct->pstr_state;
  ixheaace_audio_specific_config_struct *pstr_asc = &pstr_enc_state->audio_specific_config;

  iaace_config *pstr_aac_config = &(pstr_api_struct->config[ele_idx].aac_config);

  ixheaace_config_ancillary *pstr_ancillary = &(pstr_api_struct->config[ele_idx].pstr_ancillary);
  pstr_aac_scratch = (iaace_scratch *)pstr_api_struct->pp_mem[IA_ENHAACPLUSENC_SCRATCH_IDX];
  ptr_spectral_band_replication_scratch =
      ((pWORD8)pstr_aac_scratch) + ia_enhaacplus_enc_aac_enc_scr_size();
  ptr_resampler_scratch = ((pWORD8)pstr_aac_scratch) + ia_enhaacplus_enc_aac_enc_scr_size() +
                          ixheaace_sbr_enc_scr_size();

  /* fill pstr_ancillary data */
  pstr_ancillary->anc_flag = anc_flag;
  pstr_ancillary->anc_mode = anc_mode;
  pstr_ancillary->anc_rate = anc_rate;
  pstr_api_struct->pstr_state->temp_buff_aac =
      (void *)pstr_api_struct->pp_mem[IA_ENHAACPLUSENC_SCRATCH_IDX];
  pstr_api_struct->pstr_state->temp_buff_sbr =
      (void *)((WORD8 *)pstr_api_struct->pp_mem[IA_ENHAACPLUSENC_SCRATCH_IDX] +
               ia_enhaacplus_enc_aac_enc_scr_size());

  if (pstr_api_struct->config[ele_idx].use_mps &&
      pstr_api_struct->config[ele_idx].mps_tree_config != TREE_212) {
    pstr_api_struct->pstr_state->mps_scratch =
        (FLOAT32 *)((WORD8 *)pstr_api_struct->pp_mem[IA_ENHAACPLUSENC_SCRATCH_IDX] +
                    ia_enhaacplus_enc_aac_enc_scr_size());

    pstr_api_struct->pstr_state->ptr_temp_buff_resamp =
        (void *)((WORD8 *)pstr_api_struct->pp_mem[IA_ENHAACPLUSENC_SCRATCH_IDX] +
                 ia_enhaacplus_enc_aac_enc_scr_size() + ixheaace_sbr_enc_scr_size());
  } else {
    pstr_api_struct->pstr_state->ptr_temp_buff_resamp =
        (void *)((WORD8 *)pstr_api_struct->pp_mem[IA_ENHAACPLUSENC_SCRATCH_IDX] +
                 ia_enhaacplus_enc_aac_enc_scr_size() + ixheaace_sbr_enc_scr_size());
  }

  if (pstr_api_struct->config[ele_idx].chmode_nchannels == 2) {
    /* When the chmode option is not used,
      the number of channels depends on the input file */
    pstr_api_struct->config[ele_idx].chmode_nchannels =
        pstr_api_struct->config[ele_idx].i_channels;
  }

  switch (pstr_api_struct->pstr_state->aot) {
    case AOT_AAC_LC:
    case AOT_SBR:
    case AOT_PS:
      pstr_api_struct->pstr_state->buffer_offset = INPUT_DELAY_LC;
      break;

    case AOT_AAC_LD:
      if (pstr_aac_config->flag_framelength_small == 1) {
        pstr_api_struct->pstr_state->buffer_offset = INPUT_DELAY_LD_480;
      } else {
        pstr_api_struct->pstr_state->buffer_offset = INPUT_DELAY_LD_512;
      }
      break;

    case AOT_AAC_ELD:
      if (pstr_aac_config->flag_framelength_small == 1) {
        pstr_api_struct->pstr_state->buffer_offset = INPUT_DELAY_ELD_480;
      } else {
        pstr_api_struct->pstr_state->buffer_offset = INPUT_DELAY_ELD_512;
      }
      break;
  }
  pstr_api_struct->pstr_state->downsample[ele_idx] = 0;
  pstr_api_struct->config->frame_count = 0;
  core_ch = pstr_api_struct->config[ele_idx].chmode_nchannels;
  pstr_aac_config->sample_rate = pstr_api_struct->config[ele_idx].sample_rate;
  pstr_aac_config->core_sample_rate = pstr_api_struct->config[ele_idx].sample_rate;
  pstr_aac_config->native_sample_rate = pstr_api_struct->config[ele_idx].native_sample_rate;
  pstr_aac_config->num_in_channels = pstr_api_struct->config[ele_idx].i_channels;
  pstr_aac_config->num_out_channels = pstr_api_struct->config[ele_idx].chmode_nchannels;
  if (pstr_api_struct->config[ele_idx].aac_classic == 0) {
    pstr_aac_config->core_sample_rate = pstr_aac_config->core_sample_rate / 2;
  }
  if (pstr_api_struct->pstr_state->mps_enable) {
    switch (pstr_api_struct->pstr_state->mps_tree_config) {
      case TREE_212:
      case TREE_5151:
      case TREE_5152:
        pstr_aac_config->num_out_channels = 1;
        core_ch = 1;
        break;

      case TREE_525:
        pstr_aac_config->num_out_channels = 2;
        core_ch = 2;
        break;
    }
  }

  if (pstr_api_struct->config[ele_idx].use_parametric_stereo) {
    pstr_api_struct->config[ele_idx].chmode_nchannels = 2;
    pstr_aac_config->num_out_channels = 1;
    core_ch = 1;
    pstr_api_struct->config[ele_idx].element_type = ID_SCE;
  }
  if ((pstr_api_struct->config[ele_idx].i_channels == 2) &&
      (pstr_api_struct->config[ele_idx].chmode == 0)) {
    pstr_api_struct->config[ele_idx].chmode_nchannels = 1;
    pstr_aac_config->num_out_channels = 1;
  }
  if ((pstr_api_struct->config[ele_idx].use_parametric_stereo) &&
      (pstr_api_struct->config[ele_idx].aac_classic)) {
    return (IA_EXHEAACE_CONFIG_FATAL_AAC_CLASSIC_WITH_PS);
  }

  if (!(pstr_api_struct->config[ele_idx].adts_flag) &&
      (pstr_api_struct->config[0].aac_config.calc_crc)) {
    // set here default for crc as 0
    pstr_api_struct->config[0].aac_config.calc_crc = 0;
    return (IA_EXHEAACE_CONFIG_NONFATAL_INVALID_CONFIG);
  }

  if ((pstr_api_struct->config[0].chmode == 3) &&
      (pstr_api_struct->config[0].aac_config.calc_crc == 1)) {
    // dual mono case so crc if enabled must be disabled
    pstr_api_struct->config[0].aac_config.calc_crc = 0;
    return (IA_EXHEAACE_CONFIG_NONFATAL_INVALID_CONFIG);
  }

  pstr_aac_config->bit_rate = ixheaac_min32(360000 * core_ch, pstr_aac_config->bit_rate);
  pstr_aac_config->bit_rate = ixheaac_max32(8000 * core_ch, pstr_aac_config->bit_rate);
  switch (pstr_api_struct->pstr_state->aot) {
    case AOT_AAC_LC:
    case AOT_SBR:
    case AOT_PS:
      pstr_aac_config->bit_rate = ia_enhaacplus_aac_limitbitrate(
          pstr_aac_config->core_sample_rate,
          (pstr_aac_config->flag_framelength_small ? FRAME_LEN_960 : FRAME_LEN_1024), core_ch,
          pstr_aac_config->bit_rate);
      break;

    case AOT_AAC_LD:
    case AOT_AAC_ELD:
      pstr_aac_config->bit_rate = ia_enhaacplus_aac_limitbitrate(
          pstr_aac_config->core_sample_rate,
          (pstr_aac_config->flag_framelength_small ? FRAME_LEN_480 : FRAME_LEN_512), core_ch,
          pstr_aac_config->bit_rate);
      break;
  }
  if (pstr_api_struct->config[ele_idx].eldsbr_found == 1) {
    WORD32 num_iter = 0;
    WORD32 initial_bitrate, adjusted_bitrate;
    adjusted_bitrate = pstr_aac_config->bit_rate;

    /* Find total bitrate which provides valid configuration for each SBR element. */
    do {
      WORD32 e;
      WORD32 q_format;
      initial_bitrate = adjusted_bitrate;

      for (e = 0; e < pstr_api_struct->config[ele_idx].num_bs_elements; e++) {
        WORD32 sbr_element_bitate_in, sbr_bitrate_out;

        sbr_element_bitate_in = pstr_aac_config->bit_rate;
        sbr_bitrate_out = ixheaace_sbr_limit_bitrate(
            sbr_element_bitate_in, core_ch, pstr_aac_config->core_sample_rate,
            pstr_api_struct->spectral_band_replication_tabs.ptr_qmf_tab,
            pstr_api_struct->pstr_state->aot);

        if (sbr_bitrate_out == 0) {
          pstr_aac_config->bit_rate = 0;
        }

        /* If bitrates don't match, distribution and limiting needs to be determined again.
        Abort element loop and restart with adapted bitrate. */
        if (sbr_element_bitate_in != sbr_bitrate_out) {
          if (sbr_element_bitate_in < sbr_bitrate_out) {
            adjusted_bitrate = ixheaac_max32(
                initial_bitrate,
                (WORD32)ixheaac_div32((WORD32)(sbr_bitrate_out + 8), MAX_32, &q_format));
            adjusted_bitrate = adjusted_bitrate >> (q_format - 31);
            break;
          }

          if (sbr_element_bitate_in > sbr_bitrate_out) {
            adjusted_bitrate = ixheaac_min32(
                initial_bitrate,
                (WORD32)ixheaac_div32((WORD32)(sbr_bitrate_out - 8), MAX_32, &q_format));
            break;
          }

        } /* sbr_element_bitate_in != sbr_bitrate_out */

      } /* elements */

      num_iter++; /* restrict iteration to worst case of num elements */

    } while ((initial_bitrate != adjusted_bitrate) &&
             (num_iter <= pstr_api_struct->config[ele_idx].num_bs_elements));

    /* Unequal bitrates mean that no reasonable bitrate configuration found. */
    pstr_aac_config->bit_rate = (initial_bitrate == adjusted_bitrate) ? adjusted_bitrate : 0;
  }

  pstr_asc->str_aac_config.frame_length_flag = pstr_aac_config->flag_framelength_small;
  pstr_asc->sampling_frequency = pstr_api_struct->config[0].native_sample_rate;
  asc_channels = pstr_api_struct->config[ele_idx].i_channels_mode;
  pstr_asc->channel_configuration =
      (pstr_api_struct->pstr_state->mps_enable ? core_ch : asc_channels);
  pstr_asc->channel_configuration =
      pstr_api_struct->config[ele_idx].use_parametric_stereo
          ? 1
          : (pstr_api_struct->pstr_state->mps_enable
                 ? core_ch
                 : pstr_api_struct->config[ele_idx].i_channels_mode);

  if (!(pstr_api_struct->config[ele_idx].aac_classic) &&
      ixheaace_is_sbr_setting_available(
          pstr_aac_config->bit_rate,
          (pstr_api_struct->pstr_state->mps_enable
               ? 1
               : pstr_api_struct->config[ele_idx].chmode_nchannels),
          pstr_aac_config->sample_rate, &core_sample_rate,
          pstr_api_struct->spectral_band_replication_tabs.ptr_qmf_tab,
          pstr_api_struct->pstr_state->aot)) {
    ixheaace_str_sbr_cfg spectral_band_replication_config;
    ixheaace_initialize_sbr_defaults(&spectral_band_replication_config);

    spectral_band_replication_config.use_ps =
        pstr_api_struct->config[ele_idx].use_parametric_stereo;
    spectral_band_replication_config.crc_sbr = 0;
    spectral_band_replication_config.parametric_coding = 1;
    spectral_band_replication_config.is_esbr = pstr_api_struct->config[0].esbr_flag;
    if (pstr_api_struct->pstr_state->aot == AOT_AAC_ELD) {
      spectral_band_replication_config.is_ld_sbr = 1;
      spectral_band_replication_config.sbr_codec = ELD_SBR;
      spectral_band_replication_config.frame_flag_480 = pstr_aac_config->flag_framelength_small;
    } else if (pstr_api_struct->pstr_state->aot == AOT_SBR ||
               pstr_api_struct->pstr_state->aot == AOT_PS) {
      spectral_band_replication_config.frame_flag_960 = pstr_aac_config->flag_framelength_small;
      spectral_band_replication_config.sbr_codec = HEAAC_SBR;
    }

    if (pstr_api_struct->config[ele_idx].aac_classic == 0) {
      pstr_api_struct->pstr_state->downsample[ele_idx] = 1;
    }

    if (pstr_api_struct->pstr_state->mps_enable) {
      if (pstr_api_struct->pstr_state->mps_tree_config == TREE_212) {
        WORD32 delay =
            ((pstr_aac_config->flag_framelength_small ? FRAME_LEN_480 / 2 : FRAME_LEN_512 / 2) *
                 2 +
             4);
        ixheaace_mps_212_memory_struct *pstr_mps_memory;
        pstr_mps_memory = pstr_api_struct->pstr_state->mps_pers_mem;
        ixheaace_mps_212_open(&pstr_api_struct->pstr_mps_212_enc, pstr_mps_memory);

        pstr_asc->str_aac_config.num_sac_cfg_bits = 0;

        error = ixheaace_mps_212_initialise(
            pstr_api_struct->pstr_mps_212_enc, AOT_AAC_ELD, pstr_aac_config->sample_rate,
            &pstr_aac_config->bit_rate,
            pstr_api_struct->config[ele_idx].eldsbr_found ? 2 /*hAacConfig->sbrRatio*/ : 0,
            (pstr_aac_config->flag_framelength_small ? FRAME_LEN_480 * 2 : FRAME_LEN_512 * 2),
            /* for dual rate sbr this value is already multiplied by 2 */
            (pstr_aac_config->flag_framelength_small
                 ? FRAME_LEN_480 * 2
                 : FRAME_LEN_512 * 2) /* samples read per ch*/,
            delay, (WORD8 *)pstr_api_struct->pstr_state->ptr_temp_buff_resamp);
        if (error) {
          return error;
        }

        pstr_asc->str_aac_config.num_sac_cfg_bits = ixheaace_mps_212_get_spatial_specific_config(
            pstr_api_struct->pstr_mps_212_enc, (WORD8 *)pstr_asc->str_aac_config.sac_cfg_data,
            sizeof(pstr_asc->str_aac_config.sac_cfg_data), AOT_AAC_ELD);
        pstr_asc->str_aac_config.eld_ext_type[0] = IAAC_ELDEXT_LDSAC;
        pstr_asc->str_aac_config.eld_ext_len[0] =
            (pstr_asc->str_aac_config.num_sac_cfg_bits + 7) >> 3;
      } else {
        WORD32 tree_config;
        WORD32 bits_written;
        ixheaace_bit_buf bit_buf;
        ixheaace_bit_buf_handle ptr_bit_buf;
        ixheaace_mps_515_memory_struct *pstr_mps_memory;
        if (pstr_api_struct->pstr_state->mps_tree_config == TREE_525) {
          tree_config = 525;
        } else if (pstr_api_struct->pstr_state->mps_tree_config == TREE_5152) {
          tree_config = 5152;
        } else {
          tree_config = 5151;
        }
        pstr_mps_memory = pstr_api_struct->pstr_state->mps_515_pers_mem;
        ptr_bit_buf = ia_enhaacplus_enc_create_bitbuffer(
            &bit_buf, (UWORD8 *)pstr_asc->str_aac_config.sac_cfg_data,
            sizeof(pstr_asc->str_aac_config.sac_cfg_data));

        error = ixheaace_mps_515_open(
            &pstr_api_struct->pstr_mps_515_enc, pstr_aac_config->sample_rate, tree_config,
            ptr_bit_buf, &bits_written, pstr_mps_memory, pstr_aac_config->flag_framelength_small);
        pstr_asc->str_aac_config.num_sac_cfg_bits = bits_written;
        pstr_asc->str_aac_config.eld_ext_type[0] = IAAC_ELDEXT_LDSAC;
        pstr_asc->str_aac_config.eld_ext_len[0] = (bits_written + 7) >> 3;
        if (error) {
          return error;
        }
      }
    }

    ixheaace_adjust_sbr_settings(
        &spectral_band_replication_config, pstr_aac_config->bit_rate,
        pstr_aac_config->num_out_channels, core_sample_rate, AACENC_TRANS_FAC, 24000,
        pstr_api_struct->spectral_band_replication_tabs.ptr_qmf_tab,
        pstr_api_struct->pstr_state->aot, (pstr_api_struct->config[0].ccfl_idx == SBR_4_1));

    if (pstr_api_struct->config[ele_idx].element_type != ID_LFE) {
      /* open SBR PART, set core bandwidth */
      error = ixheaace_env_open(
          &pstr_api_struct->pstr_state->spectral_band_replication_enc_pers_mem[ele_idx],
          &spectral_band_replication_config, &pstr_aac_config->band_width,
          ptr_spectral_band_replication_scratch,
          &(pstr_api_struct->spectral_band_replication_tabs),
          &pstr_asc->str_aac_config.sbr_config);
      pstr_asc->str_aac_config.ld_sbr_present_flag =
          pstr_api_struct->config[ele_idx].eldsbr_found;
      if (error) {
        return error;
      }
      pstr_asc->str_aac_config.ld_sbr_sample_rate =
          (spectral_band_replication_config.codec_settings.sample_freq !=
           pstr_aac_config->sample_rate)
              ? 1
              : 0;
      pstr_asc->str_aac_config.ld_sbr_crc_flag = spectral_band_replication_config.crc_sbr;
    }

    if (!pstr_api_struct->config[ele_idx].use_parametric_stereo) {
      IA_ERRORCODE resamp_error = IA_NO_ERROR;
      WORD32 ch_idx = 0;

      resamp_error = ia_enhaacplus_enc_init_iir_resampler(
          &(pstr_api_struct->pstr_state->down_sampler[ele_idx][ch_idx]), 2,
          pstr_api_struct->spectral_band_replication_tabs.ptr_resamp_tab);
      if (resamp_error) {
        return resamp_error;
      }
      if (pstr_api_struct->config[ele_idx].i_channels > 1) {
        resamp_error = ia_enhaacplus_enc_init_iir_resampler(
            &(pstr_api_struct->pstr_state->down_sampler[ele_idx][ch_idx + 1]), 2,
            pstr_api_struct->spectral_band_replication_tabs.ptr_resamp_tab);
        if (resamp_error) {
          return resamp_error;
        }
      }
    }
  } else {
    if (!(pstr_api_struct->config[ele_idx].aac_classic &&
          !(pstr_api_struct->config[ele_idx].eldsbr_found))) {
      if (pstr_api_struct->config[ele_idx].aac_classic == 0) {
        error = IA_EXHEAACE_INIT_FATAL_AACPLUS_NOT_AVAIL;
      } else {
        error = IA_EXHEAACE_INIT_FATAL_BITRATE_NOT_SUPPORTED;
      }
      return error;
    }
    pstr_api_struct->pstr_state->buffer_offset = 0;
    pstr_api_struct->config[ele_idx].aac_classic = 1;
  }
  {
    WORD32 *shared_buf1, *shared_buf2;
    WORD32 *shared_buf3;
    WORD8 *shared_buf4;
    switch (pstr_api_struct->pstr_state->aot) {
      case AOT_AAC_LC:
      case AOT_SBR:
      case AOT_PS:
        ia_enhaacplus_enc_get_shared_bufs(
            ptr_spectral_band_replication_scratch, &shared_buf1, &shared_buf2, &shared_buf3,
            &shared_buf4,
            (pstr_aac_config->flag_framelength_small == 1) ? FRAME_LEN_960 : FRAME_LEN_1024);
        break;

      case AOT_AAC_LD:
      case AOT_AAC_ELD:
        ia_enhaacplus_enc_get_shared_bufs(
            ptr_spectral_band_replication_scratch, &shared_buf1, &shared_buf2, &shared_buf3,
            &shared_buf4,
            (pstr_aac_config->flag_framelength_small == 1) ? FRAME_LEN_480 : FRAME_LEN_512);
        break;
    }

    ia_enhaacplus_enc_set_shared_bufs(pstr_aac_scratch, &shared_buf1, &shared_buf2, &shared_buf3,
                                      &ptr_resampler_scratch);

    error = ia_enhaacplus_enc_aac_enc_open(
        &(pstr_api_struct->pstr_state->aac_enc_pers_mem[ele_idx]),
        *pstr_aac_config /*, *pstr_ancillary*/, pstr_aac_scratch,
        &(pstr_api_struct->pstr_aac_tabs), pstr_api_struct->config[ele_idx].element_type,
        pstr_api_struct->config[ele_idx].element_instance_tag, pstr_api_struct->pstr_state->aot);
    if (error != IA_NO_ERROR) {
      return error;
    }
    if (error) {
      if (pstr_api_struct->pstr_mps_212_enc && pstr_api_struct->pstr_state->mps_enable) {
        ixheaace_mps_212_close(&(pstr_api_struct->pstr_mps_212_enc));
      }
      if (pstr_api_struct->pstr_mps_515_enc && pstr_api_struct->pstr_state->mps_enable) {
        ixheaace_mps_515_close(pstr_api_struct->pstr_mps_212_enc);
      }

      return (IA_EXHEAACE_INIT_FATAL_AAC_INIT_FAILED);
    }
  }

  return error;
}

static IA_ERRORCODE ia_enhaacplus_enc_execute(ixheaace_api_struct *pstr_api_struct,
                                              WORD32 ele_idx) {
  IA_ERRORCODE error = IA_NO_ERROR;
  WORD32 downsample;
  WORD32 aot;
  WORD32 idx;
  WORD32 header_bytes = 0;
  WORD32 num_samples_read;
  WORD32 env_read_offset = 0;
  WORD32 write_off_set = 0;
  WORD32 aacenc_blocksize;
  WORD32 ch, out_samples;
  WORD32 total_channels = 0, ele, slot;
  WORD32 time_in_stride;
  WORD32 num_bs_elements;
  FLAG flag_last_element;
  WORD32 i_num_coup_channels;
  WORD32 i_channels_mask;
  ixheaace_pstr_sbr_enc pstr_sbr_encoder = NULL;
  iexheaac_encoder_str **pstr_aac_enc;
  iaace_config *pstr_aac_config;
  pWORD16 pw_inp_buf = NULL;
  pUWORD8 pub_out_buf = NULL;
  FLOAT32 *ptr_input_buffer = NULL;
  FLOAT32 *ptr_input_buffer_mps = NULL;
  FLOAT32 *shared_buf1_ring, *shared_buf2_ring;
  WORD32 out_stride = IXHEAACE_MAX_CH_IN_BS_ELE;
  WORD32 *pstr_aac_scratch = (pWORD32)pstr_api_struct->pp_mem[IA_ENHAACPLUSENC_SCRATCH_IDX];
  WORD8 *ptr_spectral_band_replication_scratch =
      ((pWORD8)pstr_aac_scratch) + ia_enhaacplus_enc_aac_enc_scr_size();
  WORD32 *total_fill_bits = &(pstr_api_struct->pstr_state->total_fill_bits);
  WORD32 *write_program_config_element =
      &(pstr_api_struct->config[0].write_program_config_element);

  aot = pstr_api_struct->pstr_state->aot;
  num_bs_elements = pstr_api_struct->config[0].num_bs_elements;
  flag_last_element = (ele_idx == (num_bs_elements - 1));
  i_num_coup_channels = pstr_api_struct->config[0].i_num_coupling_chan;
  i_channels_mask = pstr_api_struct->config[0].i_channels_mask;
  aacenc_blocksize = pstr_api_struct->config[0].frame_length;
  downsample = pstr_api_struct->pstr_state->downsample[ele_idx];
  num_samples_read =
      aacenc_blocksize * pstr_api_struct->config[ele_idx].i_native_channels * (1 << downsample);
  for (ele = 0; ele < num_bs_elements; ele++) {
    total_channels += pstr_api_struct->config[ele].i_native_channels;
  }
  pstr_aac_config = &(pstr_api_struct->config[ele_idx].aac_config);
  if ((pstr_api_struct->config[ele_idx].aac_classic == 0) &&
      pstr_api_struct->config[ele_idx].use_parametric_stereo) {
    time_in_stride = 1;
  } else if ((pstr_api_struct->config[ele_idx].aac_classic == 1) &&
             (pstr_api_struct->config[ele_idx].num_bs_elements == 1)) {
    time_in_stride = pstr_aac_config->num_out_channels;
  } else {
    time_in_stride = IXHEAACE_MAX_CH_IN_BS_ELE;
  }
  pstr_aac_enc = pstr_api_struct->pstr_state->aac_enc_pers_mem;

  pw_inp_buf = (pWORD16)pstr_api_struct->pp_mem[IA_ENHAACPLUSENC_INPUT_IDX];
  if (ele_idx == 0) {
    pstr_api_struct->pstr_state->i_out_bytes = 0;
    pub_out_buf = ((pUWORD8)pstr_api_struct->pp_mem[IA_ENHAACPLUSENC_OUTPUT_IDX]);
    *total_fill_bits = 0;
  }

  if (pstr_api_struct->config->adts_flag) {
    pub_out_buf = (pUWORD8)(pub_out_buf + 7);
  }

  for (ch = 0; ch < IXHEAACE_MAX_CH_IN_BS_ELE; ch++)
    pstr_api_struct->pstr_state->num_anc_data_bytes[ele_idx][ch] = 0;

  if (aot == AOT_SBR || aot == AOT_PS) {
    write_off_set = INPUT_DELAY_LC * IXHEAACE_MAX_CH_IN_BS_ELE;
  } else if (aot == AOT_AAC_ELD && pstr_api_struct->pstr_state->mps_enable != 1) {
    if (pstr_api_struct->config[0].aac_config.flag_framelength_small)
      write_off_set = INPUT_DELAY_ELD_480 * IXHEAACE_MAX_CH_IN_BS_ELE;
    else
      write_off_set = INPUT_DELAY_ELD_512 * IXHEAACE_MAX_CH_IN_BS_ELE;
  } else if (aot == AOT_AAC_ELD && pstr_api_struct->pstr_state->mps_enable == 1 &&
             pstr_api_struct->pstr_state->mps_tree_config == TREE_212) {
    if (pstr_api_struct->config[0].aac_config.flag_framelength_small)
      write_off_set = INPUT_DELAY_ELDV2_480 * IXHEAACE_MAX_CH_IN_BS_ELE;
    else
      write_off_set = INPUT_DELAY_ELDV2_512 * IXHEAACE_MAX_CH_IN_BS_ELE;
  } else if (aot == AOT_AAC_ELD && pstr_api_struct->pstr_state->mps_enable == 1 &&
             pstr_api_struct->pstr_state->mps_tree_config != TREE_212) {
    write_off_set = INPUT_DELAY_ELD_512_MPS * IXHEAACE_MAX_CH_IN_BS_ELE;
  }
  if (pstr_api_struct->config[ele_idx].aac_classic == 1) {
    write_off_set = 0;
  }
  if (aot == AOT_AAC_ELD && pstr_api_struct->pstr_state->mps_enable == 1) {
    if (pstr_api_struct->pstr_state->mps_tree_config == TREE_212) {
      env_read_offset = INPUT_DELAY_ELD_512_MPS;
    } else {
      env_read_offset = write_off_set;
    }
  }
  if (pstr_api_struct->pstr_state->downsample[ele_idx]) {
    if (!pstr_api_struct->config[ele_idx].use_parametric_stereo) {
      if (aot == AOT_AAC_ELD && pstr_api_struct->pstr_state->mps_enable != 1) {
        write_off_set +=
            4 * IXHEAACE_MAX_CH_IN_BS_ELE;  // Downsampler delay = 4 samples per channel @input SR
      } else {
        write_off_set += (pstr_api_struct->pstr_state->down_sampler[ele_idx][0].delay) *
                         IXHEAACE_MAX_CH_IN_BS_ELE;
      }
    }
    if (pstr_api_struct->config[ele_idx].use_parametric_stereo) {
      env_read_offset = (MAXIMUM_DS_2_1_FILTER_DELAY + INPUT_DELAY) * IXHEAACE_MAX_CH_IN_BS_ELE;
      write_off_set = env_read_offset;
    }
  }

  if (aot == AOT_AAC_LC || aot == AOT_SBR || aot == AOT_PS) {
    ptr_input_buffer = pstr_api_struct->pstr_state->inp_delay +
                       (aacenc_blocksize * 2 + MAXIMUM_DS_2_1_FILTER_DELAY + INPUT_DELAY_LC) *
                           IXHEAACE_MAX_CH_IN_BS_ELE * ele_idx;
  } else if (aot == AOT_AAC_LD) {
    if (pstr_api_struct->config[0].aac_config.flag_framelength_small) {
      ptr_input_buffer = pstr_api_struct->pstr_state->inp_delay +
                         (FRAME_LEN_480 * 2 + MAXIMUM_DS_2_1_FILTER_DELAY + INPUT_DELAY_LD_480) *
                             IXHEAACE_MAX_CH_IN_BS_ELE * ele_idx;
    } else {
      ptr_input_buffer = pstr_api_struct->pstr_state->inp_delay +
                         (FRAME_LEN_512 * 2 + MAXIMUM_DS_2_1_FILTER_DELAY + INPUT_DELAY_LD_512) *
                             IXHEAACE_MAX_CH_IN_BS_ELE * ele_idx;
    }
  } else if (aot == AOT_AAC_ELD) {
    if (pstr_api_struct->config[0].aac_config.flag_framelength_small) {
      if (pstr_api_struct->pstr_state->mps_enable == 1) {
        ptr_input_buffer =
            pstr_api_struct->pstr_state->inp_delay +
            (FRAME_LEN_480 * 2 + MAXIMUM_DS_2_1_FILTER_DELAY + INPUT_DELAY_ELDV2_480) *
                IXHEAACE_MAX_CH_IN_BS_ELE * ele_idx;
      } else {
        ptr_input_buffer =
            pstr_api_struct->pstr_state->inp_delay +
            (FRAME_LEN_480 * 2 + MAXIMUM_DS_2_1_FILTER_DELAY + INPUT_DELAY_ELD_480) *
                IXHEAACE_MAX_CH_IN_BS_ELE * ele_idx;
      }
    } else {
      if (pstr_api_struct->pstr_state->mps_enable == 1) {
        ptr_input_buffer =
            pstr_api_struct->pstr_state->inp_delay +
            (FRAME_LEN_512 * 2 + MAXIMUM_DS_2_1_FILTER_DELAY + INPUT_DELAY_ELDV2_512) *
                IXHEAACE_MAX_CH_IN_BS_ELE * ele_idx;
      } else {
        ptr_input_buffer =
            pstr_api_struct->pstr_state->inp_delay +
            (FRAME_LEN_512 * 2 + MAXIMUM_DS_2_1_FILTER_DELAY + INPUT_DELAY_ELD_512) *
                IXHEAACE_MAX_CH_IN_BS_ELE * ele_idx;
      }
    }
  } else {
    return IA_EXHEAACE_EXE_FATAL_UNSUPPORTED_AOT;
  }

  if (aot != AOT_AAC_LD && aot != AOT_AAC_LC) {
    pstr_sbr_encoder =
        pstr_api_struct->pstr_state->spectral_band_replication_enc_pers_mem[ele_idx];
    if (pstr_api_struct->config[ele_idx].element_type != ID_LFE) {
      ixheaace_sbr_set_scratch_ptr(pstr_sbr_encoder, ptr_spectral_band_replication_scratch);
    }
  }

  {
    ixheaace_mps_enc_ext_payload mps_extension_payload;
    UWORD8 *mps_bs = pstr_api_struct->pstr_state->mps_bs;
    memset(&mps_extension_payload, 0, sizeof(ixheaace_mps_enc_ext_payload));
    mps_extension_payload.p_data = mps_bs;

    if ((pstr_api_struct->config[ele_idx].num_bs_elements == 1) &&
        (pstr_api_struct->config[ele_idx].i_channels <= 2)) {
      if (pstr_api_struct->config[ele_idx].aac_classic != 1) {
        if ((pstr_api_struct->config[ele_idx].i_channels == 2 &&
             pstr_api_struct->config[ele_idx].chmode_nchannels == 2) &&
            (!((pstr_api_struct->pstr_mps_212_enc != NULL) &&
               pstr_api_struct->pstr_state->mps_enable))) {
          for (idx = 0; idx < (num_samples_read); idx++) {
            ptr_input_buffer[write_off_set + idx] = (FLOAT32)pw_inp_buf[idx];
          }
        } else if (pstr_api_struct->config[ele_idx].i_channels == 1) {
          for (idx = 0; idx < num_samples_read; idx++) {
            ptr_input_buffer[write_off_set + (IXHEAACE_MAX_CH_IN_BS_ELE * idx)] =
                (FLOAT32)pw_inp_buf[idx];
          }
        } else if ((pstr_api_struct->pstr_mps_212_enc != NULL) &&
                   pstr_api_struct->pstr_state->mps_enable) {
          ptr_input_buffer_mps = pstr_api_struct->pstr_state->time_signal_mps;
          for (idx = 0; idx < (num_samples_read / 2); idx++) {
            ptr_input_buffer_mps[idx] = (FLOAT32)pw_inp_buf[2 * idx];
            ptr_input_buffer_mps[(num_samples_read / 2) + idx] =
                (FLOAT32)pw_inp_buf[(2 * idx) + 1];
          }
        }
      } else {
        for (idx = 0; idx < (num_samples_read + write_off_set); idx++) {
          ptr_input_buffer[idx] = (FLOAT32)pw_inp_buf[idx];
        }
      }
    } else {
      if (pstr_api_struct->config[ele_idx].i_channels == 2) {
        slot = pstr_api_struct->config[ele_idx].element_slot;
        for (idx = 0; idx < num_samples_read / 2; idx++) {
          ptr_input_buffer[2 * idx + write_off_set] =
              (FLOAT32)pw_inp_buf[total_channels * idx + slot];
          ptr_input_buffer[2 * idx + write_off_set + 1] =
              (FLOAT32)pw_inp_buf[total_channels * idx + slot + 1];
        }
      }

      if (pstr_api_struct->config[ele_idx].i_channels == 1) {
        slot = pstr_api_struct->config[ele_idx].element_slot;
        for (idx = 0; idx < num_samples_read; idx++) {
          ptr_input_buffer[write_off_set + (IXHEAACE_MAX_CH_IN_BS_ELE * idx)] =
              (FLOAT32)pw_inp_buf[total_channels * idx + slot];
        }
      }

      if (pstr_api_struct->config[ele_idx].i_channels == 6) {
        ptr_input_buffer_mps = pstr_api_struct->pstr_state->time_signal_mps;
        for (idx = 0; idx < num_samples_read; idx++) {
          ptr_input_buffer_mps[idx] = (FLOAT32)pw_inp_buf[idx];
        }
      }
    }

    if ((pstr_api_struct->pstr_mps_212_enc != NULL) && pstr_api_struct->pstr_state->mps_enable) {
      ptr_input_buffer_mps = pstr_api_struct->pstr_state->time_signal_mps;
      error = ixheaace_mps_212_process(pstr_api_struct->pstr_mps_212_enc, ptr_input_buffer_mps,
                                       num_samples_read, &mps_extension_payload);
      if (error) {
        return error;
      }
      num_samples_read /= 2;
      for (idx = 0; idx < num_samples_read; idx++) {
        ptr_input_buffer[2 * idx + write_off_set] = (FLOAT32)ptr_input_buffer_mps[idx];
      }
      env_read_offset = write_off_set;
    }
    if ((pstr_api_struct->pstr_mps_515_enc != NULL) && pstr_api_struct->pstr_state->mps_enable) {
      ixheaace_bit_buf bit_buf;
      ixheaace_bit_buf_handle ptr_bit_buf = NULL;
      FLOAT32 *ptr_downmix_buffer_mps = pstr_api_struct->pstr_state->mps_scratch;
      VOID *ptr_scratch_515_mps = (VOID *)(pstr_api_struct->pstr_state->mps_scratch +
                                           (MAX_INPUT_CHANNELS * MAX_BUFFER_SIZE) +
                                           (MAX_OUTPUT_CHANNELS * MAX_BUFFER_SIZE));
      ptr_bit_buf = ia_enhaacplus_enc_create_bitbuffer(&bit_buf, mps_bs, MAX_MPS_BS_PAYLOAD_SIZE);

      error =
          ixheaace_mps_515_apply(pstr_api_struct->pstr_mps_515_enc, &ptr_input_buffer_mps[0],
                                 &ptr_downmix_buffer_mps[0], ptr_bit_buf, ptr_scratch_515_mps);
      if (error) {
        return error;
      }
      mps_extension_payload.data_size = ptr_bit_buf->cnt_bits;
      mps_extension_payload.data_type = IXHEAACE_MPS_EXT_LDSAC_DATA;
      mps_extension_payload.associated_ch_element = -1;

      if (pstr_api_struct->pstr_state->mps_tree_config == TREE_5151 ||
          pstr_api_struct->pstr_state->mps_tree_config == TREE_5152) {
        num_samples_read /= 6;
        for (idx = 0; idx < num_samples_read; idx++) {
          ptr_input_buffer[2 * idx + write_off_set] = (FLOAT32)ptr_downmix_buffer_mps[idx];
        }
      } else {
        num_samples_read /= 3;
        for (idx = 0; idx < num_samples_read; idx++) {
          ptr_input_buffer[idx + write_off_set] = (FLOAT32)ptr_downmix_buffer_mps[idx];
        }
      }
      env_read_offset = write_off_set;
    }

    if (pstr_api_struct->pstr_state->downsample[ele_idx]) {
      ixheaace_resampler_scratch *pstr_scratch_resampler =
          (ixheaace_resampler_scratch *)pstr_api_struct->pstr_state->ptr_temp_buff_resamp;

      if (pstr_api_struct->config[ele_idx].element_type != ID_LFE) {
        error = ixheaace_env_encode_frame(
            pstr_sbr_encoder, ptr_input_buffer + env_read_offset, ptr_input_buffer,
            IXHEAACE_MAX_CH_IN_BS_ELE,
            &(pstr_api_struct->pstr_state->num_anc_data_bytes[ele_idx][0]),
            pstr_api_struct->pstr_state->anc_data_bytes[ele_idx],
            &(pstr_api_struct->spectral_band_replication_tabs), &(pstr_api_struct->common_tabs),
            &(mps_extension_payload.p_data[0]), mps_extension_payload.data_size,
            pstr_api_struct->config[0].aac_config.flag_framelength_small, NULL);

        if (error != IA_NO_ERROR) {
          return error;
        }
      }

      if (!pstr_api_struct->config[ele_idx].use_parametric_stereo) {
        for (ch = 0; ch < pstr_aac_config->num_out_channels; ch++) {
          ia_enhaacplus_enc_get_scratch_bufs(pstr_api_struct->pstr_state->temp_buff_sbr,
                                             &shared_buf1_ring, &shared_buf2_ring);
          {
            ia_enhaacplus_enc_iir_downsampler(
                &(pstr_api_struct->pstr_state->down_sampler[ele_idx][ch]),
                ptr_input_buffer + write_off_set + ch,
                num_samples_read / pstr_aac_config->num_out_channels, IXHEAACE_MAX_CH_IN_BS_ELE,
                ptr_input_buffer + ch, &out_samples, out_stride, shared_buf1_ring,
                shared_buf2_ring, pstr_scratch_resampler);
          }
        }
      }
    }

    error = ia_enhaacplus_enc_aac_core_encode(
        pstr_aac_enc, ptr_input_buffer, time_in_stride,
        pstr_api_struct->pstr_state->anc_data_bytes[ele_idx],
        pstr_api_struct->pstr_state->num_anc_data_bytes[ele_idx], pub_out_buf,
        &(pstr_api_struct->pstr_state->i_out_bytes), &(pstr_api_struct->pstr_aac_tabs),
        pstr_api_struct->pstr_state->pstr_bit_stream_handle,
        &(pstr_api_struct->pstr_state->bit_stream), flag_last_element,
        write_program_config_element, i_num_coup_channels, i_channels_mask, ele_idx,
        total_fill_bits, total_channels, aot, pstr_api_struct->config->adts_flag,
        num_bs_elements, &pstr_api_struct->pstr_state->is_quant_spec_zero,
        &pstr_api_struct->pstr_state->is_gain_limited);
    if (error != IA_NO_ERROR) {
      return error;
    }
    if (pstr_sbr_encoder && !(pstr_api_struct->config[ele_idx].use_parametric_stereo)) {
      if (pstr_sbr_encoder && (ptr_input_buffer != NULL)) {
        memmove(ptr_input_buffer,
                ptr_input_buffer + aacenc_blocksize * 2 * IXHEAACE_MAX_CH_IN_BS_ELE,
                write_off_set * sizeof(ptr_input_buffer[0]));
      }
    }
  }
  {}
  /*ADTS Header Write*/
  if (pstr_api_struct->config->adts_flag) {
    pub_out_buf = ((pUWORD8)pstr_api_struct->pp_mem[IA_ENHAACPLUSENC_OUTPUT_IDX]);
    {
      WORD32 num_channels = 0;

      for (ele = 0; ele < pstr_api_struct->config[0].num_bs_elements; ele++) {
        num_channels += pstr_api_struct->config[ele].i_channels;
      }
      {
        header_bytes = ia_enhaacplus_enc_write_ADTS_header(
            pub_out_buf, pstr_api_struct->pstr_state->i_out_bytes,
            pstr_api_struct->config->aac_config.core_sample_rate,
            (pstr_api_struct->config[0].num_bs_elements > 1)
                ? num_channels
                : pstr_api_struct->config[0].aac_config.num_out_channels);
      }
    }

    pstr_api_struct->pstr_state->i_out_bytes += header_bytes;
  }

  return IA_NO_ERROR;
}
static IA_ERRORCODE iusace_process(ixheaace_api_struct *pstr_api_struct) {
  IA_ERRORCODE error = IA_NO_ERROR;
  WORD32 idx;
  WORD32 write_off_set = 0;
  WORD32 core_coder_frame_length;
  WORD32 usac_independency_flg;
  UWORD32 padding_bits = 0;
  WORD32 core_sample;
  WORD32 drc_sample;
  WORD32 ptr_inp_buf_offset = 0;
  WORD32 num_ch;
  WORD16 *ps_inp_buf = NULL;
  WORD8 *ps_out_buf = NULL;
  FLOAT32 *ptr_input_buffer = NULL;
  FLOAT32 *ptr_inp_buf[MAX_TIME_CHANNELS];
  FLOAT32 *ptr_drc_inp_buf[MAX_TIME_CHANNELS];
  WORD32 delay = 0;
  ixheaace_state_struct *pstr_state = pstr_api_struct->pstr_state;
  ia_bit_buf_struct *pstr_it_bit_buff = &pstr_state->str_bit_buf;
  ia_usac_encoder_config_struct *pstr_config = &pstr_api_struct->config[0].usac_config;
  ia_usac_data_struct *pstr_usac_data = &pstr_api_struct->pstr_state->str_usac_enc_data;
  iusace_scratch_mem *pstr_scratch = &pstr_usac_data->str_scratch;
  ia_classification_struct *pstr_sig_class_data =
      &pstr_state->str_usac_enc_data.str_sig_class_data;
  core_sample = (pstr_config->ccfl * pstr_config->channels);
  drc_sample = pstr_config->drc_frame_size * pstr_config->channels;
  core_coder_frame_length = pstr_config->ccfl;
  num_ch = pstr_config->channels;
  usac_independency_flg = pstr_usac_data->usac_independency_flag;
  ps_inp_buf = (WORD16 *)pstr_api_struct->pp_mem[IA_MEMTYPE_INPUT];
  ps_out_buf = (WORD8 *)pstr_api_struct->pp_mem[IA_MEMTYPE_OUTPUT];

  if (pstr_config->use_drc_element) {
    if (pstr_config->use_delay_adjustment == 1) {
      delay = (CC_DELAY_ADJUSTMENT * pstr_config->drc_frame_size / FRAME_LEN_1024) * num_ch;
    }
    for (idx = 0; idx < core_sample + delay; idx++) {
      pstr_api_struct->pstr_state->pp_drc_in_buf[idx % num_ch][idx / num_ch] =
          pstr_api_struct->pstr_state
              ->pp_drc_in_buf[idx % num_ch][idx / num_ch + pstr_config->drc_frame_size];
    }
    ptr_inp_buf_offset = pstr_config->drc_frame_size;
    for (idx = 0; idx < num_ch; idx++) {
      ptr_drc_inp_buf[idx] = pstr_api_struct->pstr_state->pp_drc_in_buf[idx];
    }
  }

  ixheaace_pstr_sbr_enc pstr_sbr_encoder =
      pstr_api_struct->pstr_state->spectral_band_replication_enc_pers_mem[0];
  if (pstr_config->sbr_enable) {
    ixheaace_mps_enc_ext_payload mps_extension_payload;
    UWORD8 *mps_bs = pstr_api_struct->pstr_state->mps_bs;
    FLOAT32 *time_signal_mps = pstr_api_struct->pstr_state->time_signal_mps;
    WORD32 sbr_pvc_mode = 0;
    WORD32 sbr_patching_mode = 1;
    WORD32 ccfl_size;
    WORD32 num_samples_read;
    WORD32 out_samples, ch;
    WORD32 resamp_ratio =
        ia_enhaacplus_enc_compute_resampling_ratio(pstr_api_struct->config[0].ccfl_idx);
    switch (pstr_config->codec_mode) {
      case USAC_SWITCHED:
        if (pstr_usac_data->str_sig_class_data.coding_mode == 2) {
          sbr_pvc_mode = 0;
        } else {
          sbr_pvc_mode = 2;
        }
        sbr_patching_mode = 1;
        break;
      case USAC_ONLY_FD:
        sbr_pvc_mode = 0;
        sbr_patching_mode = 0;
        break;
      case USAC_ONLY_TD:
        sbr_pvc_mode = 2;
        sbr_patching_mode = 1;
        break;
    }

    write_off_set = INPUT_DELAY_LC * IXHEAACE_MAX_CH_IN_BS_ELE;

    if (pstr_api_struct->config[0].ccfl_idx == SBR_4_1) {
      write_off_set = write_off_set * 2;
    }

    if (pstr_api_struct->pstr_state->downsample[0]) {
      if (pstr_api_struct->config[0].ccfl_idx == SBR_8_3) {
        write_off_set +=
            (pstr_api_struct->pstr_state->down_samp_sos[0][0].delay) * IXHEAACE_MAX_CH_IN_BS_ELE;

        write_off_set +=
            (pstr_api_struct->pstr_state->up_sampler[0][0].delay) * IXHEAACE_MAX_CH_IN_BS_ELE;
      } else if (pstr_api_struct->config[0].ccfl_idx == SBR_2_1 ||
                 pstr_api_struct->config[0].ccfl_idx == SBR_4_1) {
        write_off_set +=
            (pstr_api_struct->pstr_state->down_sampler[0][0].delay) * IXHEAACE_MAX_CH_IN_BS_ELE;
      }
    }

    ptr_input_buffer = pstr_api_struct->pstr_state->inp_delay;
    ccfl_size = pstr_api_struct->config[0].usac_config.ccfl;
    num_samples_read = ccfl_size * pstr_api_struct->config[0].i_channels;
    switch (pstr_api_struct->config[0].ccfl_idx) {
      case SBR_8_3:
        num_samples_read *= 8;
        num_samples_read /= 3;
        break;

      case SBR_2_1:
        num_samples_read *= 2;
        break;

      case SBR_4_1:
        num_samples_read *= 4;
        break;
    }

    mps_extension_payload.p_data = mps_bs;
    memset(&mps_extension_payload, 0, sizeof(ixheaace_mps_enc_ext_payload));

    if ((pstr_api_struct->pstr_mps_212_enc != NULL) && pstr_api_struct->pstr_state->mps_enable) {
      for (idx = 0; idx < num_samples_read / 2; idx++) {
        time_signal_mps[idx] = (FLOAT32)ps_inp_buf[2 * idx];
        time_signal_mps[num_samples_read / 2 + idx] = (FLOAT32)ps_inp_buf[2 * idx + 1];
      }
      ixheaace_mps_pstr_struct pstr_mps_enc =
          (ixheaace_mps_pstr_struct)pstr_api_struct->pstr_mps_212_enc;
      pstr_mps_enc->ptr_sac_encoder->independency_flag = usac_independency_flg;

      error = ixheaace_mps_212_process(pstr_api_struct->pstr_mps_212_enc, time_signal_mps,
                                       num_samples_read, &mps_extension_payload);
      if (error) {
        return error;
      }
      if (pstr_api_struct->pstr_state->mps_enable == 1) {
        for (idx = 0; idx < num_samples_read / 2; idx++) {
          ptr_input_buffer[write_off_set + 2 * idx] = time_signal_mps[idx];
          ptr_input_buffer[write_off_set + 2 * idx + 1] =
              time_signal_mps[num_samples_read / 2 + idx];
        }
      }
    } else if (pstr_api_struct->config[0].i_channels == 2 &&
               pstr_api_struct->config[0].chmode_nchannels == 2) {
      for (idx = 0; idx < (num_samples_read); idx++) {
        ptr_input_buffer[write_off_set + idx] = (FLOAT32)ps_inp_buf[idx];
      }
    } else if (pstr_api_struct->config[0].i_channels == 1) {
      for (idx = 0; idx < num_samples_read; idx++) {
        ptr_input_buffer[write_off_set + (IXHEAACE_MAX_CH_IN_BS_ELE * idx)] =
            (FLOAT32)ps_inp_buf[idx];
      }
    }

    if (num_ch == 2) {
      if (1 == pstr_config->use_drc_element) {
        for (idx = 0; idx < drc_sample; idx++) {
          ptr_drc_inp_buf[idx % num_ch][(idx >> 1) + ptr_inp_buf_offset] = ptr_input_buffer[idx];
        }
      }

      // update Header and bit-stream parameters
      if (0 == pstr_config->sbr_pvc_active) {
        sbr_pvc_mode = 0;
      }

      ixheaace_set_usac_sbr_params(
          pstr_sbr_encoder, usac_independency_flg, 0, pstr_config->sbr_pvc_active, sbr_pvc_mode,
          pstr_config->sbr_inter_tes_active, pstr_config->sbr_harmonic, sbr_patching_mode);

      // Downsample SBR input buffer for Harmonic SBR
      if (pstr_config->sbr_harmonic) {
        FLOAT32 *in_buffer_temp;
        ixheaace_get_input_scratch_buf(pstr_api_struct->pstr_state->ptr_temp_buff_resamp,
                                       &in_buffer_temp);
        FLOAT32 *outbuf = ixheaace_get_hbe_resample_buffer(pstr_sbr_encoder);
        if (pstr_api_struct->config[0].ccfl_idx == SBR_8_3) {
          WORD32 input_tot = num_samples_read / pstr_api_struct->config[0].i_channels;
          ixheaace_upsampling_inp_buf_generation(ptr_input_buffer, in_buffer_temp, input_tot,
                                                 UPSAMPLE_FAC, 0);
        }

        // Resampler
        for (ch = 0; ch < num_ch; ch++) {
          FLOAT32 *shared_buf1_ring, *shared_buf2_ring;
          ixheaace_resampler_scratch *pstr_scratch_resampler =
              (ixheaace_resampler_scratch *)(ixheaace_resampler_scratch *)
                  pstr_api_struct->pstr_state->ptr_temp_buff_resamp;

          ia_enhaacplus_enc_get_scratch_bufs(pstr_api_struct->pstr_state->temp_buff_sbr,
                                             &shared_buf1_ring, &shared_buf2_ring);

          if (pstr_api_struct->config[0].ccfl_idx == SBR_8_3) {
            // Upsampling by factor 3 - SOS implementation
            ia_enhaacplus_enc_iir_sos_upsampler(
                &(pstr_api_struct->pstr_state->hbe_up_sampler[0][ch]), in_buffer_temp + ch,
                num_samples_read / pstr_api_struct->config[0].i_channels,
                IXHEAACE_MAX_CH_IN_BS_ELE, in_buffer_temp + ch, &out_samples, shared_buf1_ring,
                shared_buf2_ring, pstr_scratch_resampler);

            // Downsampling by factor 8
            ia_enhaacplus_enc_iir_sos_downsampler(
                &(pstr_api_struct->pstr_state->hbe_down_samp_sos[0][ch]), in_buffer_temp + ch,
                out_samples, IXHEAACE_MAX_CH_IN_BS_ELE, outbuf + ch, &out_samples,
                shared_buf1_ring, shared_buf2_ring, pstr_scratch_resampler);
          } else {
            WORD32 out_stride = IXHEAACE_MAX_CH_IN_BS_ELE * resamp_ratio;

            ia_enhaacplus_enc_iir_downsampler(
                &(pstr_api_struct->pstr_state->hbe_down_sampler[0][ch]), ptr_input_buffer + ch,
                num_samples_read / pstr_api_struct->config[0].i_channels,
                IXHEAACE_MAX_CH_IN_BS_ELE, outbuf + ch, &out_samples, out_stride,
                shared_buf1_ring, shared_buf2_ring, pstr_scratch_resampler);
          }
        }
      }

      // SBR Encode
      error = ixheaace_env_encode_frame(
          pstr_sbr_encoder, ptr_input_buffer, ptr_input_buffer,
          pstr_api_struct->config[0].i_channels,
          &(pstr_api_struct->pstr_state->num_anc_data_bytes[0][0]),
          pstr_api_struct->pstr_state->anc_data_bytes[0],
          &(pstr_api_struct->spectral_band_replication_tabs), &(pstr_api_struct->common_tabs),
          &(mps_extension_payload.p_data[0]), mps_extension_payload.data_size, 0,
          &pstr_api_struct->pstr_state->str_usac_enc_data.num_sbr_bits);
      if (error != IA_NO_ERROR) {
        return error;
      }
    } else {
      if (0 == pstr_config->sbr_pvc_active) {
        sbr_pvc_mode = 0;
      }

      ixheaace_set_usac_sbr_params(
          pstr_sbr_encoder, usac_independency_flg, 0, pstr_config->sbr_pvc_active, sbr_pvc_mode,
          pstr_config->sbr_inter_tes_active, pstr_config->sbr_harmonic, sbr_patching_mode);
      if (pstr_config->sbr_harmonic) {
        FLOAT32 *in_buffer_temp;
        ixheaace_get_input_scratch_buf(pstr_api_struct->pstr_state->ptr_temp_buff_resamp,
                                       &in_buffer_temp);
        FLOAT32 *outbuf = ixheaace_get_hbe_resample_buffer(pstr_sbr_encoder);
        if (pstr_api_struct->config[0].ccfl_idx == SBR_8_3) {
          WORD32 input_tot = num_samples_read / pstr_api_struct->config[0].i_channels;
          ixheaace_upsampling_inp_buf_generation(ptr_input_buffer, in_buffer_temp, input_tot,
                                                 UPSAMPLE_FAC, 0);
        }

        // Resampler
        for (ch = 0; ch < num_ch; ch++) {
          FLOAT32 *shared_buf1_ring, *shared_buf2_ring;
          ixheaace_resampler_scratch *pstr_scratch_resampler =
              (ixheaace_resampler_scratch *)(ixheaace_resampler_scratch *)
                  pstr_api_struct->pstr_state->ptr_temp_buff_resamp;

          ia_enhaacplus_enc_get_scratch_bufs(pstr_api_struct->pstr_state->temp_buff_sbr,
                                             &shared_buf1_ring, &shared_buf2_ring);

          if (pstr_api_struct->config[0].ccfl_idx == SBR_8_3) {
            // Upsampling by factor 3 - SOS implementation
            ia_enhaacplus_enc_iir_sos_upsampler(
                &(pstr_api_struct->pstr_state->hbe_up_sampler[0][ch]), in_buffer_temp + ch,
                num_samples_read / pstr_api_struct->config[0].i_channels,
                IXHEAACE_MAX_CH_IN_BS_ELE, in_buffer_temp + ch, &out_samples, shared_buf1_ring,
                shared_buf2_ring, pstr_scratch_resampler);

            // Downsampling by factor 8
            ia_enhaacplus_enc_iir_sos_downsampler(
                &(pstr_api_struct->pstr_state->hbe_down_samp_sos[0][ch]), in_buffer_temp + ch,
                out_samples, IXHEAACE_MAX_CH_IN_BS_ELE, outbuf + ch, &out_samples,
                shared_buf1_ring, shared_buf2_ring, pstr_scratch_resampler);
          } else {
            WORD32 out_stride = IXHEAACE_MAX_CH_IN_BS_ELE * resamp_ratio;

            ia_enhaacplus_enc_iir_downsampler(
                &(pstr_api_struct->pstr_state->hbe_down_sampler[0][ch]),
                ptr_input_buffer /*input_buffer_fix + write_off_set*/ + ch,
                num_samples_read / pstr_api_struct->config[0].i_channels,
                IXHEAACE_MAX_CH_IN_BS_ELE, outbuf + ch, &out_samples, out_stride,
                shared_buf1_ring, shared_buf2_ring, pstr_scratch_resampler);
          }
        }
      }

      FLOAT32 *time_signal = pstr_api_struct->pstr_state->time_signal;
      for (idx = 0; idx < num_samples_read; idx++) {
        time_signal[idx] = (FLOAT32)ptr_input_buffer[idx << 1];
      }

      if (1 == pstr_config->use_drc_element) {
        for (idx = 0; idx < drc_sample; idx++) {
          ptr_drc_inp_buf[0][idx + ptr_inp_buf_offset] = time_signal[idx];
        }
      }

      // SBR Encode
      error = ixheaace_env_encode_frame(
          pstr_sbr_encoder, time_signal, time_signal, pstr_api_struct->config[0].i_channels,
          &(pstr_api_struct->pstr_state->num_anc_data_bytes[0][0]),
          pstr_api_struct->pstr_state->anc_data_bytes[0],
          &(pstr_api_struct->spectral_band_replication_tabs), &(pstr_api_struct->common_tabs),
          &(mps_extension_payload.p_data[0]), mps_extension_payload.data_size, 0,
          &pstr_api_struct->pstr_state->str_usac_enc_data.num_sbr_bits);
      if (error != IA_NO_ERROR) {
        return error;
      }
    }

    /* Resampling for USAC core */
    {
      FLOAT32 *in_buffer_temp;
      ixheaace_get_input_scratch_buf(pstr_api_struct->pstr_state->ptr_temp_buff_resamp,
                                     &in_buffer_temp);
      if (pstr_api_struct->config[0].ccfl_idx == SBR_8_3) {
        WORD32 input_tot = num_samples_read / pstr_api_struct->config[0].i_channels;
        ixheaace_upsampling_inp_buf_generation(ptr_input_buffer, in_buffer_temp, input_tot,
                                               UPSAMPLE_FAC, write_off_set - delay);
      }

      for (ch = 0; ch < num_ch; ch++) {
        FLOAT32 *shared_buf1_ring, *shared_buf2_ring;
        ixheaace_resampler_scratch *pstr_scratch_resampler =
            (ixheaace_resampler_scratch *)pstr_api_struct->pstr_state->ptr_temp_buff_resamp;

        ia_enhaacplus_enc_get_scratch_bufs(pstr_api_struct->pstr_state->temp_buff_sbr,
                                           &shared_buf1_ring, &shared_buf2_ring);

        if (pstr_api_struct->config[0].ccfl_idx == SBR_8_3) {
          // Upsampling by factor 3 - SOS implementation
          ia_enhaacplus_enc_iir_sos_upsampler(
              &(pstr_api_struct->pstr_state->up_sampler[0][ch]), in_buffer_temp + ch,
              num_samples_read / pstr_api_struct->config[0].i_channels, IXHEAACE_MAX_CH_IN_BS_ELE,
              in_buffer_temp + ch, &out_samples, shared_buf1_ring, shared_buf2_ring,
              pstr_scratch_resampler);

          // Downsampling by factor 8
          ia_enhaacplus_enc_iir_sos_downsampler(
              &(pstr_api_struct->pstr_state->down_samp_sos[0][ch]), in_buffer_temp + ch,
              out_samples, IXHEAACE_MAX_CH_IN_BS_ELE, ptr_input_buffer + ch, &out_samples,
              shared_buf1_ring, shared_buf2_ring, pstr_scratch_resampler);
        } else {
          WORD32 out_stride = IXHEAACE_MAX_CH_IN_BS_ELE * resamp_ratio;
          ia_enhaacplus_enc_iir_downsampler(
              &(pstr_api_struct->pstr_state->down_sampler[0][ch]),
              ptr_input_buffer + write_off_set - delay + ch,
              num_samples_read / pstr_api_struct->config[0].i_channels, IXHEAACE_MAX_CH_IN_BS_ELE,
              ptr_input_buffer + ch, &out_samples, out_stride, shared_buf1_ring, shared_buf2_ring,
              pstr_scratch_resampler);
        }
      }
    }

    if (num_ch != 0) {
      for (idx = 0; idx < num_ch; idx++) {
        ptr_inp_buf[idx] = pstr_api_struct->pstr_state->ptr_in_buf[idx];
      }

      if (num_ch == 1) {
        for (idx = 0; idx < core_sample; idx++) {
          ptr_inp_buf[0][idx] = ptr_input_buffer[idx << 1];
        }
      } else {
        for (idx = 0; idx < core_sample; idx++) {
          ptr_inp_buf[idx % num_ch][idx / num_ch] = ptr_input_buffer[idx];
        }
      }
    }
  } else {
    if (pstr_config->use_delay_adjustment == 1) {
      delay = ((CC_DELAY_ADJUSTMENT * core_coder_frame_length) / FRAME_LEN_1024) * num_ch;
    }
    if (num_ch != 0) {
      for (idx = 0; idx < num_ch; idx++) {
        ptr_inp_buf[idx] = pstr_api_struct->pstr_state->ptr_in_buf[idx];
      }

      for (idx = 0; idx < core_sample; idx++) {
        ptr_inp_buf[idx % num_ch][(idx + delay) / num_ch] = ps_inp_buf[idx];
      }

      if (1 == pstr_config->use_drc_element) {
        for (idx = 0; idx < drc_sample; idx++) {
          ptr_drc_inp_buf[idx % num_ch][(idx + delay) / num_ch + ptr_inp_buf_offset] =
              ps_inp_buf[idx];
        }
      }
    }
  }

  if (pstr_sig_class_data->is_switch_mode) {
    for (idx = 0; idx < core_coder_frame_length; idx++) {
      pstr_sig_class_data->input_samples[pstr_sig_class_data->n_buffer_samples + idx] =
          pstr_api_struct->pstr_state->ptr_in_buf[0][idx];
    }
    pstr_sig_class_data->n_buffer_samples += core_coder_frame_length;
    iusace_classification(pstr_sig_class_data, pstr_scratch, core_coder_frame_length);
  }

  pstr_it_bit_buff =
      iusace_create_bit_buffer(pstr_it_bit_buff, pstr_api_struct->pp_mem[IA_MEMTYPE_OUTPUT],
                               pstr_api_struct->pstr_mem_info[IA_MEMTYPE_OUTPUT].ui_size, 1);
  if (pstr_it_bit_buff == NULL) {
    return IA_EXHEAACE_INIT_FATAL_USAC_BITBUFFER_INIT_FAILED;
  }
  error =
      ixheaace_usac_encode(pstr_api_struct->pstr_state->ptr_in_buf, pstr_config,
                           &pstr_api_struct->pstr_state->str_usac_enc_data,
                           &pstr_api_struct->pstr_state->audio_specific_config, pstr_it_bit_buff,
                           pstr_sbr_encoder, pstr_api_struct->pstr_state->pp_drc_in_buf,
                           &pstr_api_struct->pstr_state->is_quant_spec_zero,
                           &pstr_api_struct->pstr_state->is_gain_limited);
  if (error) return error;

  padding_bits = 8 - (pstr_it_bit_buff->cnt_bits & 7);
  if (padding_bits > 0 && padding_bits < 8) {
    ps_out_buf[pstr_it_bit_buff->cnt_bits >> 3] =
        (WORD8)((UWORD32)ps_out_buf[pstr_it_bit_buff->cnt_bits >> 3]) & (0xFF << padding_bits);
  }
  pstr_api_struct->pstr_state->i_out_bytes =
      (padding_bits > 0 && padding_bits < 8) ? (pstr_it_bit_buff->cnt_bits + padding_bits) >> 3
                                             : pstr_it_bit_buff->cnt_bits >> 3;
  pstr_api_struct->pstr_state->i_out_bits = pstr_it_bit_buff->cnt_bits;
  ixheaace_write_audio_preroll_data(pstr_api_struct, pstr_it_bit_buff);
  pstr_state->str_usac_enc_data.frame_count++;
  pstr_usac_data->usac_independency_flag_count =
      (pstr_usac_data->usac_independency_flag_count + 1) %
      pstr_usac_data->usac_independency_flag_interval;

  if (pstr_config->sbr_enable) {
    WORD32 num_samples = pstr_api_struct->config[0].usac_config.ccfl * IXHEAACE_MAX_CH_IN_BS_ELE;
    switch (pstr_api_struct->config[0].ccfl_idx) {
      case SBR_8_3:
        num_samples *= 8;
        num_samples /= 3;
        break;

      case SBR_2_1:
        num_samples *= 2;
        break;

      case SBR_4_1:
        num_samples *= 4;
        break;
    }

    if (ptr_input_buffer != NULL) {
      memmove(ptr_input_buffer, ptr_input_buffer + num_samples,
              write_off_set * sizeof(ptr_input_buffer[0]));
    }
  } else if (!pstr_config->sbr_enable && pstr_config->use_delay_adjustment) {
    for (idx = 0; idx < num_ch; idx++) {
      memmove(&ptr_inp_buf[idx][0], &ptr_inp_buf[idx][core_sample / num_ch],
              sizeof(ptr_inp_buf[idx][0]) * delay / num_ch);
    }
  }

  return IA_NO_ERROR;
}
IA_ERRORCODE ixheaace_get_lib_id_strings(pVOID pv_output) {
  IA_ERRORCODE err_code = IA_NO_ERROR;
  ixheaace_version *pstr_output_config = (ixheaace_version *)pv_output;
  pstr_output_config->p_lib_name = (WORD8 *)LIB_NAME;
  pstr_output_config->p_version_num = (WORD8 *)ITTIAM_VER;

  return err_code;
}

static void ixheaace_get_measured_loudness_info(ixheaace_api_struct *pstr_api_struct,
                                                ixheaace_input_config *pstr_input_config) {
  ia_drc_input_config *pstr_internal_drc_cfg;
  ia_drc_uni_drc_config_struct *pstr_uni_drc_config;
  ia_drc_loudness_info_set_struct *pstr_enc_loudness_info_set;
  WORD32 n, m;

  pstr_internal_drc_cfg =
      (ia_drc_input_config *)&pstr_api_struct->config[0].usac_config.str_drc_cfg;
  if (pstr_input_config->use_drc_element == 0) {
    memset(pstr_internal_drc_cfg, 0, sizeof(ia_drc_input_config));
  }
  pstr_uni_drc_config = &pstr_internal_drc_cfg->str_uni_drc_config;
  pstr_enc_loudness_info_set = &pstr_internal_drc_cfg->str_enc_loudness_info_set;

  pstr_uni_drc_config->sample_rate_present = 1;
  pstr_uni_drc_config->loudness_info_set_present = 1;
  pstr_enc_loudness_info_set->loudness_info_count = 1;

  for (n = 0; n < pstr_enc_loudness_info_set->loudness_info_count; n++) {
    pstr_enc_loudness_info_set->str_loudness_info[n].drc_set_id = 0;
    pstr_enc_loudness_info_set->str_loudness_info[n].downmix_id = 0;
    pstr_enc_loudness_info_set->str_loudness_info[n].sample_peak_level_present = 1;
    pstr_enc_loudness_info_set->str_loudness_info[n].sample_peak_level =
        pstr_input_config->sample_peak_level;
    pstr_enc_loudness_info_set->str_loudness_info[n].true_peak_level_present = 0;
    pstr_enc_loudness_info_set->str_loudness_info[n].measurement_count = 1;

    for (m = 0; m < pstr_enc_loudness_info_set->str_loudness_info[n].measurement_count; m++) {
      pstr_enc_loudness_info_set->str_loudness_info[n].str_loudness_measure[m].method_definition =
          pstr_input_config->method_def;
      pstr_enc_loudness_info_set->str_loudness_info[n].str_loudness_measure[m].method_value =
          (FLOAT32)pstr_input_config->measured_loudness;
      pstr_enc_loudness_info_set->str_loudness_info[n]
          .str_loudness_measure[m]
          .measurement_system = pstr_input_config->measurement_system;
      pstr_enc_loudness_info_set->str_loudness_info[n].str_loudness_measure[m].reliability = 3;
    }
  }
}

IA_ERRORCODE ixheaace_allocate(pVOID pv_input, pVOID pv_output) {
  IA_ERRORCODE err_code = IA_NO_ERROR;
  WORD32 ui_api_size;
  pVOID pv_value;
  ixheaace_input_config *pstr_input_config = (ixheaace_input_config *)pv_input;
  ixheaace_output_config *pstr_output_config = (ixheaace_output_config *)pv_output;
  ixheaace_api_struct *pstr_api_struct;
  if (1 == pstr_input_config->usac_en) {
    pstr_input_config->aot = AOT_USAC;
  }
  if (pstr_input_config->aot != AOT_AAC_ELD && pstr_input_config->aot != AOT_AAC_LC &&
      pstr_input_config->aot != AOT_AAC_LD && pstr_input_config->aot != AOT_SBR &&
      pstr_input_config->aot != AOT_PS && pstr_input_config->aot != AOT_USAC) {
    return IA_EXHEAACE_API_FATAL_UNSUPPORTED_AOT;
  }
  ui_api_size = sizeof(ixheaace_api_struct);
  pstr_output_config->arr_alloc_memory[pstr_output_config->malloc_count] =
      pstr_output_config->malloc_xheaace(ui_api_size + EIGHT_BYTE_SIZE, DEFAULT_MEM_ALIGN_8);
  if (NULL == pstr_output_config->arr_alloc_memory[pstr_output_config->malloc_count]) {
    return IA_EXHEAACE_API_FATAL_MEM_ALLOC;
  }
  memset(pstr_output_config->arr_alloc_memory[pstr_output_config->malloc_count], 0, ui_api_size);

  pstr_output_config->ui_rem =
      (SIZE_T)((SIZE_T)pstr_output_config->arr_alloc_memory[pstr_output_config->malloc_count] %
               BYTE_ALIGN_8);

  pstr_output_config->pv_ia_process_api_obj =
      (pVOID)((WORD8 *)pstr_output_config->arr_alloc_memory[pstr_output_config->malloc_count] +
              BYTE_ALIGN_8 - pstr_output_config->ui_rem);
  pstr_output_config->malloc_count++;

  pstr_api_struct = (ixheaace_api_struct *)pstr_output_config->pv_ia_process_api_obj;
  memset(pstr_api_struct, 0, sizeof(*pstr_api_struct));

  ixheaace_set_default_config(pstr_api_struct, pstr_input_config);

  err_code = ixheaace_set_config_params(pstr_api_struct, pstr_input_config);
  if (err_code) {
    return err_code;
  }

  if (pstr_input_config->aot == AOT_USAC) {
    if (pstr_api_struct->config[0].usac_config.is_loudness_configured == 0) {
      ixheaace_get_measured_loudness_info(pstr_api_struct, pstr_input_config);
      memcpy(pstr_input_config->pv_drc_cfg, &pstr_api_struct->config[0].usac_config.str_drc_cfg,
             sizeof(ia_drc_input_config));
    }
  }

  pstr_output_config->ui_proc_mem_tabs_size =
      (sizeof(ixheaace_mem_info_struct) + sizeof(pVOID *)) * 4;
  pstr_output_config->arr_alloc_memory[pstr_output_config->malloc_count] =
      pstr_output_config->malloc_xheaace(
          pstr_output_config->ui_proc_mem_tabs_size + EIGHT_BYTE_SIZE, DEFAULT_MEM_ALIGN_8);
  if (NULL == pstr_output_config->arr_alloc_memory[pstr_output_config->malloc_count]) {
    return IA_EXHEAACE_API_FATAL_MEM_ALLOC;
  }
  memset(pstr_output_config->arr_alloc_memory[pstr_output_config->malloc_count], 0,
         pstr_output_config->ui_proc_mem_tabs_size);

  pstr_output_config->ui_rem =
      (SIZE_T)((SIZE_T)pstr_output_config->arr_alloc_memory[pstr_output_config->malloc_count] %
               BYTE_ALIGN_8);

  pv_value =
      (pVOID)((WORD8 *)pstr_output_config->arr_alloc_memory[pstr_output_config->malloc_count] +
              BYTE_ALIGN_8 - pstr_output_config->ui_rem);
  if (pv_value == NULL) {
    return IA_EXHEAACE_API_FATAL_MEM_ALLOC;
  }
  memset(pv_value, 0, (sizeof(ixheaace_mem_info_struct) + sizeof(pVOID *)) * 4);

  pstr_api_struct->pstr_mem_info = (ixheaace_mem_info_struct *)pv_value;
  pstr_api_struct->pp_mem = (pVOID *)((WORD8 *)pv_value + sizeof(ixheaace_mem_info_struct) * 4);

  pstr_output_config->malloc_count++;

  ixheaace_fill_mem_tabs(pstr_api_struct, pstr_input_config->aot);

  err_code =
      ixheaace_alloc_and_assign_mem(pstr_api_struct, pstr_output_config, pstr_input_config);
  if (err_code) {
    return err_code;
  }

  pstr_output_config->is_loudness_configured =
      pstr_api_struct->config[0].usac_config.is_loudness_configured;

  return err_code;
}

IA_ERRORCODE ixheaace_init(pVOID pstr_obj_ixheaace, pVOID pv_input, pVOID pv_output) {
  IA_ERRORCODE error = IA_NO_ERROR;
  WORD32 frame_length;
  WORD32 channels, ele_idx;
  ixheaace_api_struct *pstr_api_struct = (ixheaace_api_struct *)pstr_obj_ixheaace;
  ixheaace_input_config *pstr_input_config = (ixheaace_input_config *)pv_input;
  ixheaace_output_config *pstr_output_config = (ixheaace_output_config *)pv_output;
  WORD32 total_bitrate_used = 0;
  frame_length = pstr_input_config->frame_length;
  channels = 0;
  for (ele_idx = 0; ele_idx < pstr_api_struct->config[0].num_bs_elements; ele_idx++) {
    channels += pstr_api_struct->config[ele_idx].i_channels;
  }
  pstr_api_struct->pstr_state->aot = pstr_input_config->aot;

  if ((pstr_api_struct->config[0].use_mps == 1) &&
      (0 == pstr_api_struct->config->aac_classic ||
       pstr_api_struct->pstr_state->aot == AOT_USAC)) {
    pstr_api_struct->pstr_state->mps_enable = pstr_api_struct->config[0].use_mps;
    pstr_api_struct->pstr_state->mps_tree_config = pstr_api_struct->config[0].mps_tree_config;
  }
  if (pstr_api_struct->config[0].num_bs_elements == 1) {
    pstr_api_struct->config[ele_idx].write_program_config_element = 0;
  }

  if (pstr_api_struct->pstr_state->aot != AOT_USAC) {
    for (ele_idx = 0; ele_idx < pstr_api_struct->config[0].num_bs_elements; ele_idx++) {
      /* Set config pointer in api obj */
      pstr_api_struct->pstr_state->pstr_config[ele_idx] = &pstr_api_struct->config[ele_idx];

      error = ia_enhaacplus_enc_init(pstr_api_struct, ele_idx);
      if (error) {
        return error;
      }

      pstr_api_struct->pstr_state->ui_init_done = 1;
      total_bitrate_used += pstr_api_struct->config[ele_idx].aac_config.bit_rate;
    }
    if (pstr_input_config->i_bitrate != total_bitrate_used) {
      pstr_input_config->i_bitrate = total_bitrate_used;
    }
    if (pstr_api_struct->config[0].aac_config.bitreservoir_size != -1) {
      WORD32 avg_bytes_per_frame_per_ch = pstr_api_struct->config[0].aac_config.bitreservoir_size;
      if (pstr_api_struct->config[0].aac_config.flag_framelength_small) {
        if (pstr_api_struct->config[0].aot == AOT_AAC_LC ||
            pstr_api_struct->config[0].aot == AOT_PS ||
            pstr_api_struct->config[0].aot == AOT_SBR) {
          avg_bytes_per_frame_per_ch = (pstr_api_struct->config[0].aac_config.bit_rate) *
                                       FRAME_LEN_960 /
                                       (pstr_api_struct->config[0].aac_config.core_sample_rate *
                                        pstr_api_struct->config[0].i_channels * 8);
        }
        if (pstr_input_config->aot == AOT_AAC_LD || pstr_input_config->aot == AOT_AAC_ELD) {
          avg_bytes_per_frame_per_ch = (pstr_api_struct->config[0].aac_config.bit_rate) *
                                       FRAME_LEN_480 /
                                       (pstr_api_struct->config[0].aac_config.core_sample_rate *
                                        pstr_api_struct->config[0].i_channels * 8);
        }
      } else {
        if (pstr_api_struct->config[0].aot == AOT_AAC_LC ||
            pstr_api_struct->config[0].aot == AOT_PS ||
            pstr_api_struct->config[0].aot == AOT_SBR) {
          avg_bytes_per_frame_per_ch = (pstr_api_struct->config[0].aac_config.bit_rate) *
                                       FRAME_LEN_1024 /
                                       (pstr_api_struct->config[0].aac_config.core_sample_rate *
                                        pstr_api_struct->config[0].i_channels * 8);
        }
        if (pstr_input_config->aot == AOT_AAC_LD || pstr_input_config->aot == AOT_AAC_ELD) {
          avg_bytes_per_frame_per_ch = (pstr_api_struct->config[0].aac_config.bit_rate) *
                                       FRAME_LEN_512 /
                                       (pstr_api_struct->config[0].aac_config.core_sample_rate *
                                        pstr_api_struct->config[0].i_channels * 8);
        }
      }

      if (pstr_api_struct->config[0].aac_config.bitreservoir_size < avg_bytes_per_frame_per_ch) {
        return IA_EXHEAACE_CONFIG_NONFATAL_BITRES_SIZE_TOO_SMALL;
      }
    }
    if (pstr_input_config->i_use_es) {
      // Write GA header
      ia_bit_buf_struct *pstr_ia_asc_bit_buf;
      pstr_ia_asc_bit_buf = iusace_create_bit_buffer(
          &(pstr_api_struct->pstr_state->str_bit_buf), pstr_api_struct->pp_mem[IA_MEMTYPE_OUTPUT],
          pstr_api_struct->pstr_mem_info[IA_MEMTYPE_OUTPUT].ui_size, 1);

      ixheaace_get_audiospecific_config_bytes(pstr_ia_asc_bit_buf,
                                              &pstr_api_struct->pstr_state->audio_specific_config,
                                              pstr_api_struct->pstr_state->aot,
                                              pstr_input_config->ccfl_idx);

      pstr_api_struct->pstr_state->i_out_bytes = (pstr_ia_asc_bit_buf->cnt_bits + 7) >> 3;
    }
    if (pstr_api_struct->config->aac_classic) {
      pstr_output_config->input_size =
          frame_length * channels * pstr_api_struct->config[0].ui_pcm_wd_sz / 8;
    } else {
      pstr_output_config->input_size =
          2 * frame_length * channels * pstr_api_struct->config[0].ui_pcm_wd_sz / 8;
    }
    pstr_output_config->samp_freq = pstr_api_struct->config[0].native_sample_rate;
    pstr_output_config->header_samp_freq = pstr_api_struct->config[0].aac_config.core_sample_rate;
    pstr_output_config->down_sampling_ratio =
        pstr_api_struct->config->aac_classic == 0 ? 2.0f : 1.0f;
    switch (pstr_api_struct->config->aot) {
      case AOT_AAC_LC:
        pstr_output_config->audio_profile = AUDIO_PROFILE_AAC_LC_L5;
        break;
      case AOT_SBR:
        pstr_output_config->audio_profile = AUDIO_PROFILE_HEAAC_L5;
        break;
      case AOT_PS:
        pstr_output_config->audio_profile = AUDIO_PROFILE_HEAAC_V2_L5;
        break;
      case AOT_AAC_LD:
        pstr_output_config->audio_profile = AUDIO_PROFILE_AAC_LD_L4;
        break;
      case AOT_AAC_ELD:
        if (pstr_api_struct->config[0].use_mps) {
          if (pstr_api_struct->config[0].mps_tree_config == TREE_212) {
            pstr_output_config->audio_profile = AUDIO_PROFILE_AAC_ELD_L2;
          } else {
            pstr_output_config->audio_profile = AUDIO_PROFILE_AAC_ELD_L4;
          }
        } else {
          pstr_output_config->audio_profile = AUDIO_PROFILE_AAC_ELD_L1;
        }
        break;
      default:
        pstr_output_config->audio_profile = AUDIO_PROFILE_NOT_SPECIFIED;
        break;
    }
  }

  else {
    pstr_api_struct->pstr_state->pstr_config[0] = &pstr_api_struct->config[0];
    error = ia_usac_enc_init(pstr_api_struct, pstr_input_config->ccfl_idx);
    if (error) {
      return error;
    }

    pstr_output_config->input_size =
        frame_length * channels * (pstr_api_struct->config[0].usac_config.ui_pcm_wd_sz >> 3);

    if (pstr_api_struct->config[0].usac_config.use_drc_element) {
      ia_drc_input_config *pstr_drc_cfg = (ia_drc_input_config *)(pstr_input_config->pv_drc_cfg);
      memcpy(pstr_drc_cfg, &pstr_api_struct->config[0].usac_config.str_drc_cfg,
             sizeof(ia_drc_input_config));
    }

    pstr_output_config->down_sampling_ratio = 1;
    if (pstr_api_struct->config[0].usac_config.sbr_enable == 1) {
      switch (pstr_api_struct->config[0].ccfl_idx) {
        case SBR_8_3:
          pstr_output_config->input_size *= 8;
          pstr_output_config->input_size /= 3;
          pstr_output_config->down_sampling_ratio = 8.0f / 3.0f;
          break;

        case SBR_2_1:
          pstr_output_config->input_size *= 2;
          pstr_output_config->down_sampling_ratio = 2;
          break;

        case SBR_4_1:
          pstr_output_config->input_size *= 4;
          pstr_output_config->down_sampling_ratio = 4;
          break;
      }
    }
    pstr_output_config->samp_freq = pstr_api_struct->config[0].usac_config.native_sample_rate;
    pstr_output_config->header_samp_freq =
        pstr_api_struct->config[0].usac_config.native_sample_rate;
    pstr_output_config->audio_profile = AUDIO_PROFILE_USAC_L2;
    if (pstr_input_config->use_drc_element !=
        pstr_api_struct->config[0].usac_config.use_drc_element) {
      error = IA_EXHEAACE_EXE_NONFATAL_USAC_INVALID_GAIN_POINTS;
    }
    pstr_input_config->use_drc_element = pstr_api_struct->config[0].usac_config.use_drc_element;
  }

  pstr_api_struct->pstr_state->ui_init_done = 1;
  pstr_output_config->i_out_bytes = pstr_api_struct->pstr_state->i_out_bytes;
  if (pstr_output_config->input_size) {
    pstr_output_config->expected_frame_count =
        (pstr_input_config->aac_config.length + (pstr_output_config->input_size - 1)) /
        pstr_output_config->input_size;
    if (pstr_api_struct->config[0].usac_config.use_delay_adjustment == 1) {
      pstr_output_config->expected_frame_count -=
          pstr_api_struct->config[0].usac_config.num_preroll_frames;
    }
  }

  return error;
}

IA_ERRORCODE ixheaace_create(pVOID pv_input, pVOID pv_output) {
  IA_ERRORCODE err_code = IA_NO_ERROR;
  ixheaace_output_config *pstr_out_cfg = (ixheaace_output_config *)pv_output;
  err_code = ixheaace_allocate(pv_input, pv_output);
  if (!err_code) {
    err_code = ixheaace_init(pstr_out_cfg->pv_ia_process_api_obj, pv_input, pv_output);
  }
  if (err_code & IA_FATAL_ERROR) {
    IXHEAACE_MEM_FREE(pv_output);
  }
  return err_code;
}

IA_ERRORCODE ixheaace_process(pVOID pstr_obj_ixheaace, pVOID pv_input, pVOID pv_output) {
  IA_ERRORCODE error = IA_NO_ERROR;
  WORD32 ele_idx;
  (VOID) pv_input;
  ixheaace_api_struct *pstr_api_struct = (ixheaace_api_struct *)pstr_obj_ixheaace;
  ixheaace_output_config *pstr_output_config = (ixheaace_output_config *)pv_output;
  pstr_api_struct->pstr_state->is_quant_spec_zero = 0;
  pstr_api_struct->pstr_state->is_gain_limited = 0;
  if (!pstr_api_struct->usac_en) {
    for (ele_idx = 0; ele_idx < pstr_api_struct->config[0].num_bs_elements; ele_idx++) {
      error = ia_enhaacplus_enc_execute(pstr_api_struct, ele_idx);
      if (error != IA_NO_ERROR) {
        return error;
      }
    }
    if ((error == IA_NO_ERROR) && (pstr_api_struct->pstr_state->is_quant_spec_zero)) {
      error = IA_EXHEAACE_EXE_NONFATAL_QUANTIZATION_SPECTRUM_ZERO;
    }
    if ((error == IA_NO_ERROR) && (pstr_api_struct->pstr_state->is_gain_limited)) {
      error = IA_EXHEAACE_EXE_NONFATAL_QUANTIZATION_INSUFFICIENT_BITRES;
    }
  } else {
    ia_usac_encoder_config_struct *usac_config = &pstr_api_struct->config[0].usac_config;
    if (usac_config->iframes_interval <= usac_config->num_preroll_frames) {
      pstr_api_struct->pstr_state->str_usac_enc_data.usac_independency_flag = 1;
      if (usac_config->iframes_interval == usac_config->num_preroll_frames &&
          usac_config->is_first_frame == 0) {
        usac_config->is_ipf = 1;
      }
    } else {
      pstr_api_struct->pstr_state->str_usac_enc_data.usac_independency_flag = 0;
    }
    if (pstr_api_struct->pstr_state->str_usac_enc_data.frame_count >
        usac_config->num_preroll_frames) {
      if (usac_config->iframes_interval <= usac_config->num_preroll_frames) {
        pstr_api_struct->pstr_state->str_usac_enc_data.usac_independency_flag = 1;
      } else {
        pstr_api_struct->pstr_state->str_usac_enc_data.usac_independency_flag = 0;
      }
    }

    {
      error = iusace_process(pstr_api_struct);
      if (error & IA_FATAL_ERROR) {
        pstr_output_config->i_out_bytes = 0;
        return error;
      }
      if ((error == IA_NO_ERROR) && (pstr_api_struct->pstr_state->is_quant_spec_zero)) {
        error = IA_EXHEAACE_EXE_NONFATAL_USAC_QUANTIZATION_SPECTRUM_ZERO;
      }
      if ((error == IA_NO_ERROR) && (pstr_api_struct->pstr_state->is_gain_limited)) {
        error = IA_EXHEAACE_EXE_NONFATAL_USAC_QUANTIZATION_INSUFFICIENT_BITRES;
      }
    }

    usac_config->iframes_interval++;
    if (usac_config->iframes_interval ==
        (usac_config->random_access_interval - usac_config->num_preroll_frames)) {
      usac_config->iframes_interval = 0;
    }
  }
  pstr_output_config->i_out_bytes = pstr_api_struct->pstr_state->i_out_bytes;
  return error;
}

IA_ERRORCODE ixheaace_delete(pVOID pv_output) {
  IXHEAACE_MEM_FREE(pv_output);
  return IA_NO_ERROR;
}
