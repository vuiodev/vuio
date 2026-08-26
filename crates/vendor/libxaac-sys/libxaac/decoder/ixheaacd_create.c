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
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <assert.h>

#include "ixheaac_type_def.h"
#include "ixheaac_constants.h"
#include "ixheaacd_cnst.h"

#include "ixheaacd_bitbuffer.h"
#include "ixheaacd_config.h"
#include "ixheaacd_interface.h"
#include "ixheaacd_acelp_info.h"

#include "ixheaacd_tns_usac.h"
#include "ixheaacd_sbrdecsettings.h"
#include "ixheaacd_info.h"
#include "ixheaacd_struct.h"
#include "ixheaacd_sbr_common.h"
#include "ixheaacd_drc_data_struct.h"
#include "ixheaacd_drc_dec.h"

#include "ixheaacd_sbr_scale.h"
#include "ixheaacd_env_extr_part.h"
#include "ixheaacd_sbr_rom.h"
#include "ixheaacd_hybrid.h"
#include "ixheaacd_ps_dec.h"
#include "ixheaacd_common_rom.h"
#include "ixheaacd_qmf_dec.h"
#include "ixheaac_sbr_const.h"
#include "ixheaacd_lpp_tran.h"
#include "ixheaacd_sbrdecoder.h"
#include "ixheaacd_env_extr.h"
#include "ixheaacd_env_calc.h"
#include "ixheaacd_pvc_dec.h"
#include "ixheaacd_sbr_dec.h"
#include "ixheaacd_mps_polyphase.h"

#include "ixheaacd_memory_standards.h"
#include "ixheaacd_defines.h"
#include "ixheaacd_aac_rom.h"
#include "ixheaacd_bitbuffer.h"
#include "ixheaacd_pulsedata.h"
#include "ixheaacd_pns.h"

#include "ixheaacd_ec_defines.h"
#include "ixheaacd_ec_struct_def.h"
#include "ixheaacd_main.h"
#include "ixheaacd_channelinfo.h"
#include "ixheaacd_ec.h"
#include "ixheaacd_audioobjtypes.h"
#include "ixheaacd_latmdemux.h"
#include "ixheaacd_aacdec.h"
#include "ixheaacd_mps_struct_def.h"
#include "ixheaacd_mps_res_rom.h"
#include "ixheaacd_mps_aac_struct.h"
#include "ixheaacd_mps_dec.h"
#include "ixheaacd_struct_def.h"

#include "ixheaacd_create.h"

#include "ixheaacd_mps_interface.h"

#include "ixheaacd_func_def.h"

extern const ia_huff_code_word_struct ixheaacd_huff_book_scl[];

extern const WORD32 ixheaacd_book_scl_index[];
extern const WORD16 ixheaacd_book_scl_code_book[];

extern const ia_usac_samp_rate_info ixheaacd_samp_rate_info[];
extern const WORD32 ixheaacd_sampling_boundaries[(1 << LEN_SAMP_IDX)];

const WORD32 ixheaacd_sampl_freq_idx_table[17] = {
    96000, 88200, 64000, 48000, 44100, 32000, 24000, 22050, 16000,
    12000, 11025, 8000,  7350,  -1,    -1,    -1,    -1};

static VOID ixheaacd_info_init(const ia_usac_samp_rate_info *ptr_samp_info,
                               WORD32 block_size_samples,
                               ia_sfb_info_struct *pstr_sfb_info_long,
                               ia_sfb_info_struct *pstr_sfb_info_short,
                               WORD16 *sfb_width_short,
                               WORD16 *sfb_width_long) {
  WORD32 i, j, k, n, ws;
  const WORD16 *sfbands;
  ia_sfb_info_struct *pstr_sfb_info_ip;

  pstr_sfb_info_long->islong = 1;
  pstr_sfb_info_long->max_win_len = 1;
  pstr_sfb_info_long->samp_per_bk = block_size_samples;

  switch (block_size_samples) {
    case 480:
      pstr_sfb_info_long->ptr_sfb_tbl = ptr_samp_info->ptr_sfb_480;
      pstr_sfb_info_long->sfb_per_sbk = ptr_samp_info->num_sfb_480;
      break;
    case 512:
      pstr_sfb_info_long->ptr_sfb_tbl = ptr_samp_info->ptr_sfb_512;
      pstr_sfb_info_long->sfb_per_sbk = ptr_samp_info->num_sfb_512;
      break;
    case 768:
      pstr_sfb_info_long->sfb_per_sbk = ptr_samp_info->num_sfb_768;
      pstr_sfb_info_long->ptr_sfb_tbl = ptr_samp_info->ptr_sfb_768;
      break;
    case 960:
      pstr_sfb_info_long->ptr_sfb_tbl = ptr_samp_info->ptr_sfb_960;
      pstr_sfb_info_long->sfb_per_sbk = ptr_samp_info->num_sfb_960;
      break;
    case 1024:
      pstr_sfb_info_long->sfb_per_sbk = ptr_samp_info->num_sfb_1024;
      pstr_sfb_info_long->ptr_sfb_tbl = ptr_samp_info->ptr_sfb_1024;
      break;
    default:
      assert(0);
      break;
  }

  pstr_sfb_info_long->sfb_width = sfb_width_long;
  pstr_sfb_info_long->num_groups = 1;
  pstr_sfb_info_long->group_len[0] = 1;

  for (i = 0, j = 0, n = pstr_sfb_info_long->sfb_per_sbk; i < n; i++) {
    k = pstr_sfb_info_long->ptr_sfb_tbl[i];
    pstr_sfb_info_long->sfb_width[i] = k - j;
    j = k;
  }

  pstr_sfb_info_short->islong = 0;
  pstr_sfb_info_short->max_win_len = NSHORT;
  pstr_sfb_info_short->samp_per_bk = block_size_samples;

  for (i = 0; i < pstr_sfb_info_short->max_win_len; i++) {
    switch (block_size_samples) {
      case 768:
        pstr_sfb_info_short->ptr_sfb_tbl = ptr_samp_info->ptr_sfb_96;
        pstr_sfb_info_short->sfb_per_sbk = ptr_samp_info->num_sfb_96;
        break;
      case 960:
        pstr_sfb_info_short->ptr_sfb_tbl = ptr_samp_info->ptr_sfb_120;
        pstr_sfb_info_short->sfb_per_sbk = ptr_samp_info->num_sfb_120;
        break;
      case 1024:
        pstr_sfb_info_short->ptr_sfb_tbl = ptr_samp_info->ptr_sfb_128;
        pstr_sfb_info_short->sfb_per_sbk = ptr_samp_info->num_sfb_128;
        break;
      default:
        assert(0);
        break;
    }
  }

  pstr_sfb_info_short->sfb_width = sfb_width_short;
  for (i = 0, j = 0, n = pstr_sfb_info_short->sfb_per_sbk; i < n; i++) {
    k = pstr_sfb_info_short->ptr_sfb_tbl[i];
    pstr_sfb_info_short->sfb_width[i] = k - j;
    j = k;
  }

  pstr_sfb_info_ip = pstr_sfb_info_long;
  for (ws = 0; ws < 2; ws++) {
    pstr_sfb_info_ip->sfb_per_bk = 0;
    k = 0;
    n = 0;
    for (i = 0; i < pstr_sfb_info_ip->max_win_len; i++) {
      pstr_sfb_info_ip->bins_per_sbk =
          pstr_sfb_info_ip->samp_per_bk / pstr_sfb_info_ip->max_win_len;

      pstr_sfb_info_ip->sfb_per_bk += pstr_sfb_info_ip->sfb_per_sbk;

      sfbands = pstr_sfb_info_ip->ptr_sfb_tbl;
      for (j = 0; j < pstr_sfb_info_ip->sfb_per_sbk; j++)
        pstr_sfb_info_ip->sfb_idx_tbl[j + k] = sfbands[j] + n;

      n += pstr_sfb_info_ip->bins_per_sbk;
      k += pstr_sfb_info_ip->sfb_per_sbk;
    }
    pstr_sfb_info_ip = pstr_sfb_info_short;
  }
}

WORD32 ixheaacd_decode_init(
    VOID *handle, WORD32 sample_rate, ia_usac_data_struct *usac_data,
    ia_audio_specific_config_struct *pstr_stream_config) {
  WORD32 i;
  ia_exhaacplus_dec_api_struct *codec_handle =
      (ia_exhaacplus_dec_api_struct *)handle;
  ia_aac_dec_state_struct *aac_dec_handle = codec_handle->p_state_aac;
  WORD32 fscale;

  WORD32 ele_id = 0;

  ia_usac_config_struct *ptr_usac_config =
      &(pstr_stream_config->str_usac_config);
  ia_usac_decoder_config_struct *ptr_usac_dec_config =
      &(pstr_stream_config->str_usac_config.str_usac_dec_config);
  WORD32 num_elements = ptr_usac_dec_config->num_elements;
  WORD32 chan = 0;

  usac_data->ec_flag = codec_handle->aac_config.ui_err_conceal;
  usac_data->huffman_code_book_scl = aac_dec_handle->huffman_code_book_scl;
  usac_data->huffman_code_book_scl_index =
      aac_dec_handle->huffman_code_book_scl_index;

  usac_data->tns_coeff3_32 =
      aac_dec_handle->pstr_aac_tables->pstr_block_tables->tns_coeff3_32;
  usac_data->tns_coeff4_32 =
      aac_dec_handle->pstr_aac_tables->pstr_block_tables->tns_coeff4_32;
  usac_data->tns_max_bands_tbl_usac =
      &aac_dec_handle->pstr_aac_tables->pstr_block_tables
           ->tns_max_bands_tbl_usac;
  for (WORD32 ch = 0; ch < MAX_NUM_CHANNELS; ch++) {
    ixheaacd_usac_ec_init(&usac_data->str_error_concealment[ch], usac_data->core_mode);
    memset(&usac_data->overlap_data_ptr[ch][0], 0, sizeof(usac_data->overlap_data_ptr[ch]));
  }

  for (i = 0; i < 11; i++) {
    if (ixheaacd_sampling_boundaries[i] <= sample_rate) break;
  }

  if (i == (1 << LEN_SAMP_IDX)) return -1;
  usac_data->sampling_rate_idx = i;

  fscale = (WORD32)((double)sample_rate * (double)FSCALE_DENOM / 12800.0f);

  for (i = 0; i < MAX_NUM_CHANNELS; i++) {
    usac_data->window_shape_prev[i] = 0;
    usac_data->window_shape[i] = 0;
  }

  ixheaacd_hufftab(&ixheaacd_book, ixheaacd_huff_book_scl,
                   ixheaacd_book_scl_code_book, ixheaacd_book_scl_index, 1, 60,
                   60, 1, 19);

  usac_data->pstr_usac_winmap[0] = &usac_data->str_only_long_info;
  usac_data->pstr_usac_winmap[1] = &usac_data->str_only_long_info;
  usac_data->pstr_usac_winmap[2] = &usac_data->str_eight_short_info;
  usac_data->pstr_usac_winmap[3] = &usac_data->str_only_long_info;
  usac_data->pstr_usac_winmap[4] = &usac_data->str_only_long_info;

  if ((usac_data->ccfl != 480) && (usac_data->ccfl != 512) &&
      (usac_data->ccfl != 768) && (usac_data->ccfl != 960) &&
      (usac_data->ccfl != 1024))
    return -1;
  ixheaacd_info_init(&ixheaacd_samp_rate_info[usac_data->sampling_rate_idx],
                     usac_data->ccfl, usac_data->pstr_usac_winmap[0],
                     usac_data->pstr_usac_winmap[2], usac_data->sfb_width_short,
                     usac_data->sfb_width_long);

  for (i = 0; i < MAX_NUM_CHANNELS; i++) {
    usac_data->str_tddec[i] = &usac_data->arr_str_tddec[i];
    if (usac_data->ccfl == 768)
      usac_data->str_tddec[i]->fscale = pstr_stream_config->sampling_frequency;
    else
      usac_data->str_tddec[i]->fscale =
          ((fscale)*usac_data->ccfl) / LEN_SUPERFRAME;
    usac_data->len_subfrm = usac_data->ccfl / 4;

    {
      WORD32 fac_length = usac_data->len_subfrm / 4;
      if (fac_length & (fac_length - 1)) {
        if ((fac_length != 48) && (fac_length != 96) && (fac_length != 192) &&
            (fac_length != 384) && (fac_length != 768)) {
          return -1;
        }
      }
    }
    usac_data->num_subfrm = (MAX_NUM_SUBFR * usac_data->ccfl) / LEN_SUPERFRAME;

    ixheaacd_init_acelp_data(usac_data, usac_data->str_tddec[i]);

    usac_data->str_tddec[i]->fd_synth =
        &usac_data->str_tddec[i]->fd_synth_buf[LEN_FRAME];
  }

  for (ele_id = 0; ele_id < num_elements; ele_id++) {
    UWORD32 ele_type;
    WORD32 stereo_config_index;

    ia_usac_dec_element_config_struct *ptr_usac_ele_config =
        &ptr_usac_config->str_usac_dec_config.str_usac_element_config[ele_id];

    if (ptr_usac_ele_config) {
      if (usac_data->tw_mdct[ele_id]) {
        if (usac_data->ec_flag) {
          usac_data->tw_mdct[ele_id] = 0;
        } else
          return -1;
      }

      usac_data->noise_filling_config[ele_id] = ptr_usac_ele_config->noise_filling;
    }

    ele_type = ptr_usac_config->str_usac_dec_config.usac_element_type[ele_id];

    stereo_config_index = ptr_usac_ele_config->stereo_config_index;

    switch (ele_type) {
      case ID_USAC_SCE:
      case ID_USAC_LFE:

        if ((chan + 1) > MAX_NUM_CHANNELS_USAC_LVL2) return -1;
        usac_data->seed_value[chan] = 0x3039;
        chan++;

        break;

      case ID_USAC_CPE: {
        static const WORD32 frame_len_tbl[MAX_CORE_SBR_FRAME_LEN_IDX + 1] = {
            -1, -1, 32, 32, 64};

        if ((chan + 2) > MAX_NUM_CHANNELS_USAC_LVL2) return -1;
        usac_data->seed_value[chan] = 0x3039;
        chan++;

        usac_data->seed_value[chan] = 0x10932;
        chan++;

        if (stereo_config_index > 0) {
          WORD32 bs_frame_length =
              frame_len_tbl[ptr_usac_config->core_sbr_framelength_index];
          WORD32 bs_residual_coding = (stereo_config_index > 1) ? 1 : 0;

          ia_usac_dec_mps_config_struct *ptr_usac_mps212_config =
              &(ptr_usac_config->str_usac_dec_config
                    .str_usac_element_config[ele_id]
                    .str_usac_mps212_config);
          aac_dec_handle->mps_dec_handle.ec_flag = aac_dec_handle->ec_enable;

          if (ixheaacd_mps_create(&aac_dec_handle->mps_dec_handle,
                                  bs_frame_length, bs_residual_coding,
                                  ptr_usac_mps212_config)) {
            return -1;
          }
        }
        break;
      }

      break;
      case ID_USAC_EXT:
        break;
      default:
        return -1;
        break;
    }
  }

  return 0;
}

WORD32 ixheaacd_dec_data_init(VOID *handle,
                              ia_frame_data_struct *pstr_frame_data,
                              ia_usac_data_struct *usac_data) {
  ia_audio_specific_config_struct *pstr_stream_config, *layer_config;
  WORD32 err_code = 0;
  WORD32 i_ch, i, ele_id;
  WORD32 num_elements;

  WORD32 out_frame_len, sbr_ratio_idx;

  ia_usac_config_struct *ptr_usac_config =
      &(pstr_frame_data->str_audio_specific_config.str_usac_config);

  usac_data->last_frame_ok = 1;
  usac_data->frame_ok = 1;
  usac_data->window_shape_prev[0] = WIN_SEL_0;
  usac_data->window_shape_prev[1] = WIN_SEL_0;

  pstr_frame_data->str_layer.bit_rate =
      pstr_frame_data->str_audio_specific_config.avg_bit_rate;
  pstr_stream_config = &pstr_frame_data->str_audio_specific_config;
  layer_config = &pstr_frame_data->str_audio_specific_config;

  sbr_ratio_idx = ixheaacd_sbr_params(
      ptr_usac_config->core_sbr_framelength_index, &out_frame_len,
      &usac_data->ccfl, &usac_data->output_samples,
      &pstr_frame_data->str_layer.sample_rate_layer,
      &layer_config->samp_frequency_index);
  if (!pstr_frame_data->str_layer.sample_rate_layer) {
    return -1;
  }
  pstr_stream_config->sampling_frequency =
      pstr_frame_data->str_layer.sample_rate_layer;
  pstr_stream_config->samp_frequency_index = layer_config->samp_frequency_index;

  num_elements = ptr_usac_config->str_usac_dec_config.num_elements;

  for (ele_id = 0; ele_id < num_elements; ele_id++) {
    ia_usac_dec_element_config_struct *ptr_usac_ele_config =
        &(ptr_usac_config->str_usac_dec_config.str_usac_element_config[ele_id]);

    if (ptr_usac_ele_config) {
      usac_data->tw_mdct[ele_id] = ptr_usac_ele_config->tw_mdct;
    }

    {
      ia_usac_dec_mps_config_struct *ptr_usac_mps212_config =
          &ptr_usac_ele_config->str_usac_mps212_config;
      WORD32 stereo_config_index = ptr_usac_ele_config->stereo_config_index;

      usac_data->mps_pseudo_lr[ele_id] =
          (stereo_config_index > 1) ? ptr_usac_mps212_config->bs_pseudo_lr : 0;
    }
  }

  usac_data->sbr_ratio_idx = sbr_ratio_idx;
  usac_data->esbr_bit_str[0].no_elements = 0;
  usac_data->esbr_bit_str[1].no_elements = 0;

  if (usac_data->ccfl == 768)
    pstr_frame_data->str_layer.sample_rate_layer =
        4 * pstr_frame_data->str_layer.sample_rate_layer / 3;

  for (i = 0; i < MAX_NUM_CHANNELS; i++) {
    usac_data->coef_fix[i] = &usac_data->arr_coef_fix[i][0];
    usac_data->coef[i] = &usac_data->arr_coef[i][0];
    usac_data->coef_save[i] = &usac_data->arr_coef_save[i][0];
    usac_data->factors[i] = &usac_data->arr_factors[i][0];
    usac_data->group_dis[i] = &usac_data->arr_group_dis[i][0];
    usac_data->pstr_tns[i] = &usac_data->arr_str_tns[i];
    usac_data->tw_ratio[i] = &usac_data->arr_tw_ratio[i][0];
    usac_data->ms_used[i] = &usac_data->arr_ms_used[i][0];
    usac_data->window_shape_prev[i] = WIN_SEL_0;

    usac_data->seed_value[i] = 0x0;

    usac_data->fac_data_present[i] = 0;
  }

  err_code =
      ixheaacd_decode_init(handle, pstr_frame_data->str_layer.sample_rate_layer,
                           usac_data, pstr_stream_config);
  if (err_code != 0) return err_code;

  for (i_ch = 0; i_ch < MAX_NUM_CHANNELS; i_ch++) {
    if (usac_data->tw_mdct[0] == 1) {
      WORD32 i;
      for (i = 0; i < 2 * usac_data->ccfl; i++) {
        usac_data->warp_cont_mem[i_ch][i] = 1.0;
      }
      usac_data->warp_sum[i_ch][0] = usac_data->warp_sum[i_ch][1] =
          (FLOAT32)usac_data->ccfl;
    }
  }
  return err_code;
}

static VOID ixheaacd_count_tracks_per_layer(int *max_layer, int *stream_count,
                                            int *tracks_in_layer) {
  WORD32 stream;
  WORD32 num_layer;
  WORD32 num_streams;
  WORD32 layer = 0;

  if (stream_count == NULL)
    num_streams = 0;
  else
    num_streams = *stream_count;
  if (max_layer == NULL)
    num_layer = num_streams;
  else
    num_layer = *max_layer;
  if (num_layer < 0) num_layer = num_streams;

  for (stream = 0; (layer <= num_layer) && (stream < num_streams);) {
    *tracks_in_layer = 1;
    stream += 1;
    layer++;
    if (layer <= num_layer) *tracks_in_layer = 0;
  }

  if (max_layer) *max_layer = (layer - 1);
  if (stream_count) *stream_count = stream;
}

WORD32 ixheaacd_frm_data_init(ia_audio_specific_config_struct *pstr_audio_conf,
                              ia_dec_data_struct *pstr_dec_data)

{
  WORD32 layer;
  WORD32 track;
  WORD32 num_dec_streams;
  ia_frame_data_struct *pstr_frame_data;

  WORD32 stream_count = 1;
  WORD32 max_layer = -1;

  memset(pstr_dec_data, 0, IXHEAAC_GET_SIZE_ALIGNED(sizeof(ia_dec_data_struct), BYTE_ALIGN_8));
  memset(&(pstr_dec_data->str_frame_data), 0,
         sizeof(pstr_dec_data->str_frame_data));

  pstr_frame_data = &(pstr_dec_data->str_frame_data);

  if (max_layer < 0) max_layer = stream_count - 1;

  ixheaacd_count_tracks_per_layer(&max_layer, &stream_count,
                                  &pstr_frame_data->tracks_in_layer);

  pstr_frame_data->scal_out_select = max_layer;

  pstr_frame_data->stream_count = 0;

  num_dec_streams = track = 0;
  for (layer = 0; layer < (signed)pstr_frame_data->scal_out_select + 1;
       layer++) {
    WORD32 j;
    for (j = 0; j < 1; j++, num_dec_streams++) {
      pstr_frame_data->str_audio_specific_config = *pstr_audio_conf;
      pstr_frame_data->str_layer.sample_rate_layer =
          pstr_frame_data->str_audio_specific_config.sampling_frequency;
      pstr_frame_data->str_layer.bit_rate =
          pstr_frame_data->str_audio_specific_config.avg_bit_rate;
    }

    track += pstr_frame_data->tracks_in_layer;
  }

  pstr_frame_data->stream_count = num_dec_streams;

  return num_dec_streams;
}

WORD32 ixheaacd_decode_create(ia_exhaacplus_dec_api_struct *handle,
                              ia_dec_data_struct *pstr_dec_data,
                              WORD32 tracks_for_decoder) {
  WORD32 stream;

  WORD32 err = 0;
  ia_frame_data_struct *pstr_frame_data;
  WORD32 stream_count;
  ia_aac_dec_state_struct *aac_dec_handle = handle->p_state_aac;
  pstr_frame_data = &(pstr_dec_data->str_frame_data);
  stream_count = pstr_frame_data->stream_count;
  pstr_frame_data->stream_count = tracks_for_decoder;

  for (stream = 0; stream < stream_count; stream++) {
    UWORD32 aot = pstr_frame_data->str_audio_specific_config.audio_object_type;

    switch (aot) {
      case AOT_USAC:

        err = ixheaacd_dec_data_init(handle, pstr_frame_data,
                                     &(pstr_dec_data->str_usac_data));

        if (err != 0) {
          if (handle->aac_config.ui_err_conceal) {
            pstr_dec_data->str_usac_data.frame_ok = 0;
          } else
            return err;
        }

        pstr_dec_data->str_usac_data.sampling_rate = pstr_frame_data->str_layer.sample_rate_layer;

        switch (pstr_dec_data->str_usac_data.sbr_ratio_idx) {
          case 0:
            handle->aac_config.ui_sbr_mode = 0;
            break;
          case 1:
            handle->aac_config.ui_sbr_mode = 1;
            break;
          case 2:
            handle->aac_config.ui_sbr_mode = 1;
            break;
          case 3:
            handle->aac_config.ui_sbr_mode = 3;
            break;

          default:
            handle->aac_config.ui_sbr_mode = 0;
        }

        if (!aac_dec_handle->peak_lim_init && !handle->aac_config.peak_limiter_off &&
            handle->aac_config.ui_err_conceal) {
          memset(&aac_dec_handle->peak_limiter, 0, sizeof(ia_peak_limiter_struct));
          ixheaacd_peak_limiter_init(&aac_dec_handle->peak_limiter, MAX_NUM_CHANNELS,
                                     pstr_dec_data->str_usac_data.sampling_rate,
                                     &aac_dec_handle->peak_limiter.buffer[0],
                                     &aac_dec_handle->delay_in_samples);
          aac_dec_handle->peak_lim_init++;
        }

        break;

      default:

        break;
    }
  }

  pstr_frame_data->scal_out_object_type =
      pstr_frame_data->str_audio_specific_config.audio_object_type;
  pstr_frame_data->scal_out_num_channels =
      pstr_frame_data->str_audio_specific_config.channel_configuration;
  pstr_frame_data->scal_out_sampling_frequency =
      pstr_frame_data->str_audio_specific_config.sampling_frequency;

  if (&(pstr_dec_data->str_usac_data) != NULL) {
    ia_sbr_header_data_struct usac_def_header;
    ia_audio_specific_config_struct *pstr_aud_spec_config =
        &pstr_frame_data->str_audio_specific_config;
    ia_usac_config_struct *ptr_usac_config =
        &(pstr_frame_data->str_audio_specific_config.str_usac_config);

    WORD32 inter_test_flag = 0;
    WORD32 bs_pvc_flag = 0;
    WORD32 harmonic_Sbr_flag = 0;

    ia_usac_decoder_config_struct *ptr_usac_dec_config =
        &ptr_usac_config->str_usac_dec_config;
    WORD32 const num_ele = ptr_usac_dec_config->num_elements;
    WORD32 elem_idx = 0;

    memset(&usac_def_header, 0, sizeof(ia_sbr_header_data_struct));

    for (elem_idx = 0; elem_idx < num_ele; elem_idx++) {
      UWORD32 usac_ele_type =
          ptr_usac_config->str_usac_dec_config.usac_element_type[elem_idx];
      ia_usac_dec_element_config_struct *ptr_usac_ele_config =
          &ptr_usac_config->str_usac_dec_config
               .str_usac_element_config[elem_idx];

      ia_usac_dec_sbr_config_struct *ptr_usac_sbr_config =
          &(ptr_usac_dec_config->str_usac_element_config[elem_idx]
                .str_usac_sbr_config);

      if (ptr_usac_sbr_config->bs_inter_tes) inter_test_flag = 1;
      if (ptr_usac_sbr_config->bs_pvc) bs_pvc_flag = 1;
      if (ptr_usac_sbr_config->harmonic_sbr) harmonic_Sbr_flag = 1;

      if ((usac_ele_type != ID_USAC_LFE) && (usac_ele_type != ID_USAC_EXT)) {
        ia_usac_dec_sbr_config_struct *ptr_usac_sbr_config =
            &(ptr_usac_ele_config->str_usac_sbr_config);

        usac_def_header.start_freq = ptr_usac_sbr_config->dflt_start_freq;
        usac_def_header.stop_freq = ptr_usac_sbr_config->dflt_stop_freq;
        usac_def_header.header_extra_1 =
            ptr_usac_sbr_config->dflt_header_extra1;
        usac_def_header.header_extra_2 =
            ptr_usac_sbr_config->dflt_header_extra2;
        usac_def_header.freq_scale = ptr_usac_sbr_config->dflt_freq_scale;
        usac_def_header.alter_scale = ptr_usac_sbr_config->dflt_alter_scale;
        usac_def_header.noise_bands = ptr_usac_sbr_config->dflt_noise_bands;
        usac_def_header.limiter_bands = ptr_usac_sbr_config->dflt_limiter_bands;
        usac_def_header.limiter_gains = ptr_usac_sbr_config->dflt_limiter_gains;
        usac_def_header.interpol_freq = ptr_usac_sbr_config->dflt_interpol_freq;
        usac_def_header.smoothing_mode =
            ptr_usac_sbr_config->dflt_smoothing_mode;
      }
    }

    pstr_dec_data->str_usac_data.down_samp_sbr = 0;

    if (pstr_dec_data->str_usac_data.sbr_ratio_idx > 0) {
      if (pstr_aud_spec_config->ext_sampling_frequency ==
          pstr_aud_spec_config->sampling_frequency) {
        pstr_dec_data->str_usac_data.down_samp_sbr = 1;
      }
      if (pstr_dec_data->str_usac_data.down_samp_sbr == 0) {
        if (pstr_dec_data->str_usac_data.sbr_ratio_idx == 3) {
          pstr_frame_data->scal_out_sampling_frequency =
              4 * pstr_frame_data->scal_out_sampling_frequency;
        } else {
          pstr_frame_data->scal_out_sampling_frequency =
              2 * pstr_frame_data->scal_out_sampling_frequency;
        }
      }

      {
        void *sbr_persistent_mem_v = aac_dec_handle->sbr_persistent_mem_u;

        pstr_dec_data->str_usac_data.pstr_esbr_dec = ixheaacd_init_sbr(
            pstr_frame_data->str_layer.sample_rate_layer,
            pstr_dec_data->str_usac_data.ccfl,
            &pstr_dec_data->str_usac_data.down_samp_sbr, sbr_persistent_mem_v,
            NULL, pstr_frame_data->scal_out_num_channels, 0,
            pstr_dec_data->str_usac_data.sbr_ratio_idx,
            pstr_dec_data->str_usac_data.output_samples, &harmonic_Sbr_flag,
            (void *)&usac_def_header, aac_dec_handle->str_sbr_config,
            pstr_dec_data->str_usac_data.audio_object_type, 0, 0);
        pstr_dec_data->str_usac_data.sbr_scratch_mem_base =
            aac_dec_handle->sbr_scratch_mem_u;
        if (num_ele)
          ixheaacd_setesbr_flags(sbr_persistent_mem_v, bs_pvc_flag,
                                 harmonic_Sbr_flag, inter_test_flag);
      }

      if (pstr_dec_data->str_usac_data.pstr_esbr_dec == NULL) {
        return -1;
      } else {
        pstr_dec_data->str_usac_data.pstr_esbr_dec->xaac_jmp_buf =
            &(aac_dec_handle->xaac_jmp_buf);
      }
    }
  }
  aac_dec_handle->decode_create_done = 1;
  return 0;
}
