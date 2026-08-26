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

#include <string.h>
#include "ixheaac_type_def.h"
#include "ixheaac_constants.h"
#include "ixheaace_aac_constants.h"
#include "impd_drc_common_enc.h"
#include "impd_drc_uni_drc.h"
#include "impd_drc_tables.h"
#include "impd_drc_api.h"
#include "ixheaace_api.h"
#include "ixheaac_error_standards.h"
#include "ixheaace_error_codes.h"
#include "ixheaace_psy_const.h"
#include "ixheaace_tns.h"
#include "ixheaace_tns_params.h"
#include "ixheaace_rom.h"
#include "ixheaace_common_rom.h"
#include "ixheaace_bitbuffer.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaac_basic_ops.h"
#include "ixheaace_adjust_threshold_data.h"

#include "ixheaace_dynamic_bits.h"
#include "ixheaace_qc_data.h"
#include "ixheaace_channel_map.h"
#include "ixheaace_block_switch.h"
#include "ixheaace_psy_data.h"
#include "ixheaace_interface.h"
#include "ixheaace_write_bitstream.h"
#include "ixheaace_psy_configuration.h"
#include "ixheaace_psy_mod.h"
#include "ixheaace_stereo_preproc.h"
#include "ixheaace_enc_main.h"
#include "ixheaace_qc_util.h"
#include "ixheaace_config_params.h"
#include "ixheaace_common_utils.h"
#define ALIGNMENT_DEFINE __attribute__((aligned(8)))

static WORD32 ixheaace_calculate_bandwidth(const WORD32 sample_rate,
                                           const WORD32 channel_bit_rate, const WORD32 num_ch,
                                           WORD32 aot) {
  WORD32 bandwidth = -1;
  const ixheaace_bandwidth_table *pstr_bandwidth_table = NULL;
  WORD32 bandwidth_table_size = 0;
  if (aot == AOT_AAC_LC || aot == AOT_SBR || aot == AOT_PS) {
    pstr_bandwidth_table = bandwidth_table_lc;
    bandwidth_table_size = sizeof(bandwidth_table_lc) / sizeof(ixheaace_bandwidth_table);
  } else if (aot == AOT_AAC_LD || aot == AOT_AAC_ELD) {
    switch (sample_rate) {
      case 48000:
      case 64000:
      case 88200:
      case 96000:
        pstr_bandwidth_table = bandwidth_table_ld_48000;
        bandwidth_table_size =
            sizeof(bandwidth_table_ld_48000) / sizeof(ixheaace_bandwidth_table);
        break;
      case 44100:
        pstr_bandwidth_table = bandwidth_table_ld_44100;
        bandwidth_table_size =
            sizeof(bandwidth_table_ld_44100) / sizeof(ixheaace_bandwidth_table);
        break;
      case 32000:
        pstr_bandwidth_table = bandwidth_table_ld_32000;
        bandwidth_table_size =
            sizeof(bandwidth_table_ld_32000) / sizeof(ixheaace_bandwidth_table);
        break;
      case 24000:
        pstr_bandwidth_table = bandwidth_table_ld_24000;
        bandwidth_table_size =
            sizeof(bandwidth_table_ld_24000) / sizeof(ixheaace_bandwidth_table);
        break;
      case 8000:
      case 11025:
      case 12000:
      case 16000:
      case 22050:
        pstr_bandwidth_table = bandwidth_table_ld_22050;
        bandwidth_table_size =
            sizeof(bandwidth_table_ld_22050) / sizeof(ixheaace_bandwidth_table);
        break;
    }
  }
  for (WORD32 i = 0; i < bandwidth_table_size - 1; i++) {
    if (channel_bit_rate >= 96000) {
      if (aot == AOT_AAC_LC || aot == AOT_SBR || aot == AOT_PS) {
        bandwidth = 20000;
      } else {
        if (num_ch == 1) {
          bandwidth = 19000;
        } else {
          bandwidth = 22000;
        }
      }
      break;
    } else if (channel_bit_rate >= pstr_bandwidth_table[i].channel_bit_rate &&
               channel_bit_rate < pstr_bandwidth_table[i + 1].channel_bit_rate) {
      if (aot == AOT_AAC_LC || aot == AOT_SBR || aot == AOT_PS) {
        bandwidth = (num_ch == 1) ? pstr_bandwidth_table[i].bandwidth_mono
                                  : pstr_bandwidth_table[i].bandwidth_stereo;
        bandwidth = bandwidth - (pstr_bandwidth_table[i].channel_bit_rate / 32);
        break;
      } else if (aot == AOT_AAC_LD || aot == AOT_AAC_ELD) {
        WORD32 start_bandwidth, end_bandwidth, start_bitrate, end_bitrate;
        FLOAT32 bandwidth_fac;
        start_bandwidth = (num_ch == 1) ? pstr_bandwidth_table[i].bandwidth_mono
                                        : pstr_bandwidth_table[i].bandwidth_stereo;
        start_bandwidth = start_bandwidth - (pstr_bandwidth_table[i].channel_bit_rate / 32);
        end_bandwidth = (num_ch == 1) ? pstr_bandwidth_table[i + 1].bandwidth_mono
                                      : pstr_bandwidth_table[i + 1].bandwidth_stereo;
        end_bandwidth = end_bandwidth - (pstr_bandwidth_table[i + 1].channel_bit_rate / 32);
        start_bitrate = pstr_bandwidth_table[i].channel_bit_rate;
        end_bitrate = pstr_bandwidth_table[i + 1].channel_bit_rate;
        bandwidth_fac =
            (FLOAT32)((channel_bit_rate - start_bitrate) / (end_bitrate - start_bitrate));
        bandwidth = (WORD32)(bandwidth_fac * (end_bandwidth - start_bandwidth) + start_bandwidth);
        break;
      }
    }
  }
  return bandwidth;
}

static VOID ixheaace_determine_bandwidth(const WORD32 proposed_bandwidth, const WORD32 bitrate,
                                         const WORD32 sample_rate, const WORD32 channels,
                                         WORD32 *const bandwidth, WORD32 aot) {
  WORD32 channel_bit_rate = bitrate / channels;
  if (proposed_bandwidth == 0) {
    *bandwidth = ixheaace_calculate_bandwidth(sample_rate, channel_bit_rate, channels, aot);
  } else {
    *bandwidth = MIN(proposed_bandwidth, MIN(20000, sample_rate >> 1));
  }
  *bandwidth = MIN(*bandwidth, sample_rate / 2);
}

WORD32 ia_enhaacplus_enc_aac_enc_pers_size(WORD32 num_aac_chan, WORD32 aot) {
  WORD32 num_bytes;
  num_bytes = IXHEAAC_GET_SIZE_ALIGNED(sizeof(iexheaac_encoder_str), BYTE_ALIGN_8);
  num_bytes += (num_aac_chan *
    IXHEAAC_GET_SIZE_ALIGNED(sizeof(ixheaace_psy_out_channel), BYTE_ALIGN_8));
  num_bytes += (num_aac_chan *
    IXHEAAC_GET_SIZE_ALIGNED(sizeof(ixheaace_psy_data), BYTE_ALIGN_8));
  num_bytes += (num_aac_chan *
    IXHEAAC_GET_SIZE_ALIGNED(sizeof(ixheaace_temporal_noise_shaping_data), BYTE_ALIGN_8));
  if (aot == AOT_AAC_LC || aot == AOT_SBR || aot == AOT_PS) {
    num_bytes += (num_aac_chan *
      IXHEAAC_GET_SIZE_ALIGNED(BLK_SWITCH_OFFSET_LC_128 * sizeof(FLOAT32), BYTE_ALIGN_8));
  } else if (aot == AOT_AAC_LD || aot == AOT_AAC_ELD) {
    num_bytes += (num_aac_chan *
      IXHEAAC_GET_SIZE_ALIGNED(BLK_SWITCH_OFFSET_LD * sizeof(FLOAT32), BYTE_ALIGN_8));
  }

  num_bytes += (num_aac_chan *
    IXHEAAC_GET_SIZE_ALIGNED(sizeof(ixheaace_qc_out_channel), BYTE_ALIGN_8));
  return num_bytes;
}

WORD32 ia_enhaacplus_enc_aac_enc_scr_size(VOID) {
  return IXHEAAC_GET_SIZE_ALIGNED(sizeof(iaace_scratch), BYTE_ALIGN_8);
}

VOID ia_enhaacplus_enc_set_shared_bufs(iaace_scratch *scr, WORD32 **shared_buf1,
                                       WORD32 **shared_buf2, WORD32 **shared_buf3,
                                       WORD8 **shared_buf5) {
  iaace_scratch *pstr_aac_enc_scratch = scr;
  /* Fill addresses of shared buffers */
  pstr_aac_enc_scratch->shared_buffer1 = *shared_buf1;
  pstr_aac_enc_scratch->shared_buffer_2 = *shared_buf2;
  pstr_aac_enc_scratch->shared_buffer3 = *shared_buf3;
  pstr_aac_enc_scratch->shared_buffer5 = (WORD8 *)*shared_buf5;
}

VOID ia_enhaacplus_enc_aac_init_default_config(iaace_config *config, WORD32 aot) {
  memset(config, 0, sizeof(iaace_config));

  /* default configurations */
  config->bit_rate = AAC_BITRATE_DEFAULT_VALUE;
  config->band_width = 0;
  if (aot == AOT_AAC_LC || aot == AOT_SBR || aot == AOT_PS) {
    config->inv_quant = 0;
  } else if (aot == AOT_AAC_LD || aot == AOT_AAC_ELD) {
    config->inv_quant = 2;
  }
  config->bitreservoir_size = BITRESERVOIR_SIZE_CONFIG_PARAM_DEFAULT_VALUE;
  config->use_tns = 0;
  config->flag_framelength_small =
      USE_FRAMELENGTH_SMALL_PARAM_DEFAULT_VALUE;  // assume framelength large
}

static VOID ia_enhaacplus_enc_aac_set_scratch_ptr(iexheaac_encoder_str *pstr_exheaac_encoder,
                                                  iaace_scratch *pstr_scr) {
  pstr_exheaac_encoder->pstr_aac_scratch = pstr_scr;
}

VOID ia_enhaacplus_enc_init_aac_tabs(ixheaace_aac_tables *pstr_aac_tabs) {
  pstr_aac_tabs->pstr_mdct_tab = (ixheaace_mdct_tables *)&ixheaace_enc_mdct_tab;
  pstr_aac_tabs->pstr_huff_tab = (ixheaace_huffman_tables *)&ixheaace_enc_huff_tab;
  pstr_aac_tabs->pstr_psycho_tab = (ixheaace_psycho_tables *)&ixheaace_enc_psycho_tab;
  pstr_aac_tabs->pstr_quant_tab = (ixheaace_quant_tables *)&ixheaace_enc_quant_tab;
  pstr_aac_tabs->pstr_tns_tab =
      (ixheaace_temporal_noise_shaping_tables *)&ixheaace_enhaacplus_enc_tns_tab;
}

static VOID ia_enhaacplus_enc_aac_set_persist_buf(WORD8 *ptr_base, WORD32 num_chan, WORD32 aot) {
  iexheaac_encoder_str *pstr_exheaac_encoder;
  WORD8 *ptr_curr_mem = ptr_base +
    IXHEAAC_GET_SIZE_ALIGNED(sizeof(iexheaac_encoder_str), BYTE_ALIGN_8);
  WORD32 i;

  pstr_exheaac_encoder = (iexheaac_encoder_str *)ptr_base;

  for (i = 0; i < num_chan; i++) {
    pstr_exheaac_encoder->psy_out.psy_out_ch[i] = (ixheaace_psy_out_channel *)(ptr_curr_mem);
    ptr_curr_mem = ptr_curr_mem +
      IXHEAAC_GET_SIZE_ALIGNED(sizeof(ixheaace_psy_out_channel), BYTE_ALIGN_8);
  }

  for (i = 0; i < num_chan; i++) {
    pstr_exheaac_encoder->psy_kernel.psy_data[i] = (ixheaace_psy_data *)(ptr_curr_mem);
    ptr_curr_mem = ptr_curr_mem +
      IXHEAAC_GET_SIZE_ALIGNED(sizeof(ixheaace_psy_data), BYTE_ALIGN_8);
  }

  for (i = 0; i < num_chan; i++) {
    pstr_exheaac_encoder->psy_kernel.temporal_noise_shaping_data[i] =
        (ixheaace_temporal_noise_shaping_data *)(ptr_curr_mem);
    ptr_curr_mem = ptr_curr_mem +
      IXHEAAC_GET_SIZE_ALIGNED(sizeof(ixheaace_temporal_noise_shaping_data), BYTE_ALIGN_8);
  }

  for (i = 0; i < num_chan; i++) {
    switch (aot) {
      case AOT_AAC_LC:
      case AOT_SBR:
      case AOT_PS:
        pstr_exheaac_encoder->psy_kernel.psy_data[i]->ptr_mdct_delay_buf =
            (FLOAT32 *)(ptr_curr_mem);
        ptr_curr_mem = ptr_curr_mem +
          IXHEAAC_GET_SIZE_ALIGNED(sizeof(FLOAT32) * BLK_SWITCH_OFFSET_LC_128, BYTE_ALIGN_8);
        break;

      case AOT_AAC_LD:
      case AOT_AAC_ELD:
        pstr_exheaac_encoder->psy_kernel.psy_data[i]->ptr_mdct_delay_buf =
            (FLOAT32 *)(ptr_curr_mem);
        ptr_curr_mem = ptr_curr_mem +
          IXHEAAC_GET_SIZE_ALIGNED(sizeof(FLOAT32) * BLK_SWITCH_OFFSET_LD, BYTE_ALIGN_8);
        break;
    }
  }

  for (i = 0; i < num_chan; i++) {
    pstr_exheaac_encoder->qc_out.qc_channel[i] = (ixheaace_qc_out_channel *)(ptr_curr_mem);
    ptr_curr_mem = ptr_curr_mem +
      IXHEAAC_GET_SIZE_ALIGNED(sizeof(ixheaace_qc_out_channel), BYTE_ALIGN_8);
  }
}

IA_ERRORCODE ia_enhaacplus_enc_aac_enc_open(iexheaac_encoder_str **ppstr_exheaac_encoder,
                                            const iaace_config config,
                                            iaace_scratch *pstr_aac_scratch,
                                            ixheaace_aac_tables *pstr_aac_tabs, WORD32 ele_type,
                                            WORD32 element_instance_tag, WORD32 aot) {
  IA_ERRORCODE error = IA_NO_ERROR;
  WORD32 profile = 1;
  ixheaace_element_info *pstr_element_info = NULL;
  iexheaac_encoder_str *pstr_exheaac_encoder;
  WORD32 frame_len_long = FRAME_LEN_1024;
  switch (aot) {
    case AOT_AAC_LC:
    case AOT_SBR:
    case AOT_PS:
      if (config.flag_framelength_small) {
        frame_len_long = FRAME_LEN_960;
      } else {
        frame_len_long = FRAME_LEN_1024;
      }
      break;

    case AOT_AAC_LD:
    case AOT_AAC_ELD:
      if (config.flag_framelength_small) {
        frame_len_long = FRAME_LEN_480;
      } else {
        frame_len_long = FRAME_LEN_512;
      }
      break;
  }

  if ((config.num_in_channels < 1) || (config.num_out_channels > IXHEAACE_MAX_CH_IN_BS_ELE) ||
    (config.num_out_channels < 1) || (config.num_in_channels < config.num_out_channels)) {
    return IA_EXHEAACE_INIT_FATAL_INVALID_NUM_CHANNELS_IN_ELE;
  }
  if ((config.bit_rate != 0) && ((config.bit_rate / config.num_out_channels < 8000) ||
    (config.bit_rate / config.num_out_channels > 576000))) {
    error = IA_EXHEAACE_INIT_FATAL_BITRATE_NOT_SUPPORTED;
  }
  if (error != IA_NO_ERROR) {
    return error;
  }

  pstr_exheaac_encoder = *ppstr_exheaac_encoder;

  memset(pstr_exheaac_encoder, 0, sizeof(iexheaac_encoder_str));

  ia_enhaacplus_enc_aac_set_scratch_ptr(pstr_exheaac_encoder, pstr_aac_scratch);

  ia_enhaacplus_enc_aac_set_persist_buf((WORD8 *)pstr_exheaac_encoder, config.num_out_channels,
                                        aot);

  /* check sample rate */

  switch (config.core_sample_rate) {
    case 8000:
    case 11025:
    case 12000:
    case 16000:
    case 22050:
    case 24000:
    case 32000:
    case 44100:
    case 48000:
    case 64000:
    case 88200:
    case 96000:
      break;

    default:
      return IA_EXHEAACE_INIT_FATAL_INVALID_CORE_SAMPLE_RATE;
      break;
  }

  pstr_exheaac_encoder->config = config;

  error = ia_enhaacplus_enc_init_element_info(config.num_out_channels,
                                              &pstr_exheaac_encoder->element_info, ele_type,
                                              element_instance_tag);
  if (error != IA_NO_ERROR) {
    return error;
  }

  pstr_element_info = &pstr_exheaac_encoder->element_info;

  /* allocate the Psy aud Psy Out structure */

  error = (ia_enhaacplus_enc_psy_new(
      &pstr_exheaac_encoder->psy_kernel, pstr_element_info->n_channels_in_el,
      pstr_exheaac_encoder->pstr_aac_scratch->shared_buffer_2, frame_len_long));

  if (error != IA_NO_ERROR) {
    return error;
  }

  WORD32 tns_mask = config.use_tns;
  if (config.full_bandwidth) {
    pstr_exheaac_encoder->config.band_width = config.core_sample_rate >> 2;
  } else {
    ixheaace_determine_bandwidth(pstr_exheaac_encoder->config.band_width, config.bit_rate,
                                 config.core_sample_rate, pstr_element_info->n_channels_in_el,
                                 &pstr_exheaac_encoder->config.band_width, aot);
  }
  pstr_exheaac_encoder->bandwidth_90_dB = (WORD32)pstr_exheaac_encoder->config.band_width;
  if (ele_type == ID_LFE) {
    tns_mask = 0;
  }

  error = ia_enhaacplus_enc_psy_main_init(
      &pstr_exheaac_encoder->psy_kernel, config.core_sample_rate, config.bit_rate,
      pstr_element_info->n_channels_in_el, tns_mask, pstr_exheaac_encoder->bandwidth_90_dB, aot,
      pstr_aac_tabs, frame_len_long);
  if (error != IA_NO_ERROR) {
    return error;
  }

  /* allocate the Q&C Out structure */
  error = ia_enhaacplus_enc_qc_out_new(
      &pstr_exheaac_encoder->qc_out, pstr_element_info->n_channels_in_el,
      pstr_exheaac_encoder->pstr_aac_scratch->shared_buffer1,
      pstr_exheaac_encoder->pstr_aac_scratch->shared_buffer3, frame_len_long);

  if (error != IA_NO_ERROR) {
    return error;
  }

  /* allocate the Q&C kernel */
  error = ia_enhaacplus_enc_qc_new(&pstr_exheaac_encoder->qc_kernel,
                                   pstr_exheaac_encoder->pstr_aac_scratch->shared_buffer_2,
                                   frame_len_long);
  if (error != IA_NO_ERROR) {
    return error;
  }

  ixheaace_qc_init qc_init;

  qc_init.pstr_element_info = &pstr_exheaac_encoder->element_info;

  if (aot == AOT_AAC_LC || aot == AOT_SBR || aot == AOT_PS) {
    if (config.flag_framelength_small) {
      qc_init.max_bits = MAXIMUM_CHANNEL_BITS_960 * pstr_element_info->n_channels_in_el;
    } else {
      qc_init.max_bits = MAXIMUM_CHANNEL_BITS_1024 * pstr_element_info->n_channels_in_el;
    }

    qc_init.bit_res = qc_init.max_bits;
  }

  qc_init.average_bits = (config.bit_rate * frame_len_long) / config.core_sample_rate;

  if (aot == AOT_AAC_LD || aot == AOT_AAC_ELD) {
    if (pstr_exheaac_encoder->config.bitreservoir_size != -1) {
      qc_init.max_bits = (pstr_exheaac_encoder->config.bitreservoir_size * 8) *
                         pstr_element_info->n_channels_in_el;
      if (qc_init.max_bits > qc_init.average_bits) {
        qc_init.bit_res = (pstr_exheaac_encoder->config.bitreservoir_size * 8) *
                          pstr_element_info->n_channels_in_el;
      } else {
        qc_init.max_bits = qc_init.average_bits;
        qc_init.bit_res = 0;
      }
    } else {
      qc_init.max_bits = qc_init.average_bits;
      qc_init.bit_res = 0;
    }
  }

  qc_init.padding.padding_rest = config.core_sample_rate;

  qc_init.mean_pe = ((FLOAT32)10 * frame_len_long * pstr_exheaac_encoder->bandwidth_90_dB * 2) /
                    config.core_sample_rate;

  switch (aot) {
    case AOT_AAC_LC:
    case AOT_SBR:
    case AOT_PS:
      if (config.flag_framelength_small) {
        qc_init.max_bit_fac =
            (float)(MAXIMUM_CHANNEL_BITS_960 * pstr_element_info->n_channels_in_el) /
            (float)(qc_init.average_bits ? qc_init.average_bits : 1);
      } else {
        qc_init.max_bit_fac =
            (float)(MAXIMUM_CHANNEL_BITS_1024 * pstr_element_info->n_channels_in_el) /
            (float)(qc_init.average_bits ? qc_init.average_bits : 1);
      }
      break;

    case AOT_AAC_LD:
    case AOT_AAC_ELD:
      if (config.flag_framelength_small) {
        qc_init.max_bit_fac = (FLOAT32)((MAXIMUM_CHANNEL_BITS_480)*pstr_element_info
                                            ->n_channels_in_el);  // no anc data in aacld
      } else {
        qc_init.max_bit_fac = (FLOAT32)((MAXIMUM_CHANNEL_BITS_512)*pstr_element_info
                                            ->n_channels_in_el);  // no anc data in aacld
      }
      qc_init.max_bit_fac =
          qc_init.max_bit_fac / (qc_init.average_bits ? qc_init.average_bits : 1);
      break;
  }

  qc_init.bitrate = config.bit_rate;
  qc_init.inv_quant = config.inv_quant;

  error = ia_enhaacplus_enc_qc_init(&pstr_exheaac_encoder->qc_kernel, aot, &qc_init,
                                    config.flag_framelength_small);
  if (error != IA_NO_ERROR) {
    return error;
  }

  /* init bitstream encoder */
  pstr_exheaac_encoder->bse_init.num_channels = pstr_element_info->n_channels_in_el;
  pstr_exheaac_encoder->bse_init.bitrate = config.bit_rate;
  pstr_exheaac_encoder->bse_init.sample_rate = config.core_sample_rate;
  pstr_exheaac_encoder->bse_init.profile = profile;

  if (config.num_in_channels > config.num_out_channels) {
    pstr_exheaac_encoder->downmix = 1;
    pstr_exheaac_encoder->downmix_fac = config.num_in_channels / config.num_out_channels;
  }

  if (pstr_element_info->el_type == ID_CPE &&
      (config.core_sample_rate <= 24000 &&
       (config.bit_rate / pstr_element_info->n_channels_in_el * 2) < 60000)) {
    FLOAT32 scf_used_ratio = (FLOAT32)pstr_exheaac_encoder->psy_kernel.psy_conf_long.sfb_active /
                             pstr_exheaac_encoder->psy_kernel.psy_conf_long.sfb_cnt;

    error = iaace_init_stereo_pre_processing(&(pstr_exheaac_encoder->str_stereo_pre_pro),
                                             pstr_element_info->n_channels_in_el, config.bit_rate,
                                             config.core_sample_rate, scf_used_ratio);
  }

  if (error != IA_NO_ERROR) {
    return error;
  }

  *ppstr_exheaac_encoder = pstr_exheaac_encoder;

  return IA_NO_ERROR;
}
