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
#include <math.h>
#include "ixheaac_type_def.h"
#include "impd_drc_common_enc.h"
#include "impd_drc_uni_drc.h"
#include "impd_drc_tables.h"
#include "impd_drc_api.h"
#include "ixheaace_api.h"
#include "ixheaac_error_standards.h"
#include "ixheaace_error_codes.h"
#include "ixheaace_mps_common_fix.h"
#include "ixheaace_mps_defines.h"
#include "ixheaace_mps_common_define.h"
#include "ixheaac_constants.h"
#include "ixheaace_bitbuffer.h"
#include "ixheaace_sbr_def.h"
#include "ixheaace_mps_buf.h"
#include "ixheaace_mps_lib.h"
#include "ixheaace_mps_main_structure.h"
#include "ixheaace_mps_bitstream.h"
#include "ixheaace_mps_onset_detect.h"
#include "ixheaace_mps_filter.h"
#include "ixheaace_mps_param_extract.h"
#include "ixheaace_mps_static_gain.h"

#include "ixheaace_mps_dmx_tdom_enh.h"
#include "ixheaace_mps_tools_rom.h"
#include "ixheaace_mps_qmf.h"
#include "ixheaace_mps_struct_def.h"
#include "ixheaace_mps_sac_polyphase.h"
#include "ixheaace_mps_sac_hybfilter.h"
#include "ixheaace_mps_spatial_bitstream.h"
#include "ixheaace_mps_tree.h"
#include "ixheaace_mps_rom.h"
#include "ixheaace_mps_delay.h"
#include "ixheaace_mps_frame_windowing.h"
#include "ixheaace_mps_structure.h"
#include "ixheaace_mps_memory.h"
#include "ixheaace_mps_enc.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaac_basic_ops.h"

static UWORD8 ixheaace_mps_212_space_get_num_qmf_bands(const UWORD32 num_sample_rate) {
  UWORD8 num_qmf_bands = 0;
  if (num_sample_rate < 27713)
    num_qmf_bands = 32;
  else if (num_sample_rate < 55426)
    num_qmf_bands = 64;

  return num_qmf_bands;
}

static VOID ixheaace_mps_212_space_tree_open(ixheaace_mps_pstr_space_tree *pstr_ph_space_tree,
                                             ixheaace_mps_212_memory_struct *pstr_mps_memory) {
  ixheaace_mps_pstr_space_tree pstr_space_tree = NULL;
  WORD32 box;

  pstr_space_tree = &pstr_mps_memory->spacce_tree;

  for (box = 0; box < IXHEAACE_MPS_MAX_NUM_BOXES; box++) {
    ixheaace_mps_pstr_tto_box pstr_tto_box = NULL;
    pstr_tto_box = &pstr_mps_memory->tto_box;

    if (NULL != pstr_space_tree) {
      pstr_space_tree->pstr_tto_box[box] = pstr_tto_box;
    }
  }
  *pstr_ph_space_tree = pstr_space_tree;
}

static VOID ixheaace_mps_212_init_num_param_bands(
    ixheaace_mps_pstr_space_structure pstr_space_enc, const WORD32 num_param_bands) {
  WORD32 k = 0;
  const WORD32 n = sizeof(valid_bands_ld) / sizeof(UWORD8);
  const UWORD8 *p_bands = valid_bands_ld;

  while (k < n && p_bands[k] != (UWORD8)num_param_bands) ++k;
  if (k == n) {
    pstr_space_enc->num_param_bands = IXHEAACE_MPS_SAC_BANDS_INVALID;
  } else {
    pstr_space_enc->num_param_bands = num_param_bands;
  }
}

static IA_ERRORCODE ixheaace_mps_212_configure(
    ixheaace_mps_pstr_space_structure pstr_space_enc,
    const ixheaace_mps_pstr_spece_enc_setup pstr_h_set_up) {
  IA_ERRORCODE error = IA_NO_ERROR;

  pstr_space_enc->num_sample_rate = pstr_h_set_up->sample_rate;
  pstr_space_enc->num_qmf_bands =
      ixheaace_mps_212_space_get_num_qmf_bands(pstr_space_enc->num_sample_rate);
  pstr_space_enc->time_alignment = pstr_h_set_up->time_alignment;
  pstr_space_enc->quant_mode = pstr_h_set_up->quant_mode;

  pstr_space_enc->use_coarse_quant_cld = pstr_h_set_up->b_use_coarse_quant;
  pstr_space_enc->use_coarse_quant_cpc = pstr_h_set_up->b_use_coarse_quant;
  pstr_space_enc->use_frame_keep = (pstr_h_set_up->b_ld_mode == 2);
  pstr_space_enc->use_coarse_quant_icc = 0;
  pstr_space_enc->use_coarse_quant_arb_dmx = 0;
  pstr_space_enc->independency_factor = pstr_h_set_up->independency_factor;
  pstr_space_enc->independency_count = 0;
  pstr_space_enc->independency_flag = 1;

  pstr_space_enc->num_hybrid_bands = pstr_space_enc->num_qmf_bands;
  pstr_space_enc->num_frame_time_slots = pstr_h_set_up->frame_time_slots;
  ixheaace_mps_212_init_num_param_bands(pstr_space_enc, pstr_h_set_up->num_param_bands);

  return error;
}

static WORD32 ixheaace_mps_212_get_analysis_length_time_slots(FLOAT32 *ptr_frame_window_ana,
                                                              WORD32 num_time_slots) {
  WORD32 i;
  for (i = num_time_slots - 1; i >= 0; i--) {
    if (ptr_frame_window_ana[i] != 0) {
      break;
    }
  }
  num_time_slots = i + 1;
  return num_time_slots;
}

static WORD32 ixheaace_mps_212_get_analysis_start_time_slot(FLOAT32 *ptr_frame_window_ana,
                                                            WORD32 num_time_slots) {
  WORD32 start_time_slot = 0;
  WORD32 i;
  for (i = 0; i < num_time_slots; i++) {
    if (ptr_frame_window_ana[i] != 0) {
      break;
    }
  }
  start_time_slot = i;
  return start_time_slot;
}

static VOID ixheaace_mps_212_memcpy_flex_pcm(FLOAT32 *const ptr_dst, const WORD32 dst_stride,
                                             const FLOAT32 *const ptr_src,
                                             const WORD32 src_stride, const WORD32 num_samples) {
  WORD32 i;

  for (i = 0; i < num_samples; i++) {
    ptr_dst[i * dst_stride] = ptr_src[i * src_stride];
  }
}

static IA_ERRORCODE ixheaace_mps_212_feed_de_inter_pre_scale(
    ixheaace_mps_pstr_space_structure pstr_space_enc, FLOAT32 const *const ptr_samples,
    FLOAT32 *const ptr_output_samples, WORD32 const num_samples,
    UWORD32 const is_input_inter_leaved, UWORD32 const input_buffer_size_per_channel,
    UWORD32 *const ptr_n_samples_fed) {
  IA_ERRORCODE error = IA_NO_ERROR;
  WORD32 ch;
  const WORD32 num_ch_in = pstr_space_enc->n_input_channels;
  const WORD32 num_ch_in_with_dmx = num_ch_in;
  const WORD32 samples_to_feed =
      MIN(num_samples, pstr_space_enc->n_samples_next - pstr_space_enc->n_samples_valid);
  const WORD32 num_samples_per_channel = samples_to_feed / num_ch_in_with_dmx;

  if ((samples_to_feed < 0) || (samples_to_feed % num_ch_in_with_dmx != 0) ||
      (samples_to_feed > num_ch_in_with_dmx * (WORD32)pstr_space_enc->n_frame_length)) {
    return IA_EXHEAACE_CONFIG_NONFATAL_MPS_INVALID_CONFIG;
  }
  const FLOAT32 *p_input_mps;
  const FLOAT32 *p_input_2_mps;
  {
    p_input_mps = ptr_samples;
    p_input_2_mps = ptr_samples + (pstr_space_enc->n_input_delay * num_ch_in_with_dmx);
  }
  for (ch = 0; ch < num_ch_in; ch++) {
    memcpy(&(pstr_space_enc->time_signal_in[ch][0]),
           &(pstr_space_enc->time_signal_delay_in[ch][0]),
           pstr_space_enc->n_surround_analysis_buffer_delay * sizeof(FLOAT32));

    if (is_input_inter_leaved) {
      ixheaace_mps_212_memcpy_flex_pcm(
          &(pstr_space_enc->time_signal_in[ch][pstr_space_enc->n_surround_analysis_buffer_delay]),
          1, p_input_mps + ch, num_ch_in_with_dmx, pstr_space_enc->n_input_delay);
      ixheaace_mps_212_memcpy_flex_pcm(
          &(pstr_space_enc->time_signal_in[ch][pstr_space_enc->n_surround_analysis_buffer_delay +
                                               pstr_space_enc->n_input_delay]),
          1, p_input_2_mps + ch, num_ch_in_with_dmx,
          num_samples_per_channel - pstr_space_enc->n_input_delay);
    } else {
      memcpy(
          &(pstr_space_enc->time_signal_in[ch][pstr_space_enc->n_surround_analysis_buffer_delay]),
          p_input_mps + ch * input_buffer_size_per_channel,
          pstr_space_enc->n_input_delay * sizeof(FLOAT32));
      memcpy(
          &(pstr_space_enc->time_signal_in[ch][pstr_space_enc->n_surround_analysis_buffer_delay +
                                               pstr_space_enc->n_input_delay]),
          p_input_2_mps + ch * input_buffer_size_per_channel,
          (num_samples_per_channel - pstr_space_enc->n_input_delay) * sizeof(FLOAT32));
    }

    memcpy(&(pstr_space_enc->time_signal_delay_in[ch][0]),
           &(pstr_space_enc->time_signal_in[ch][pstr_space_enc->n_frame_length]),
           pstr_space_enc->n_surround_analysis_buffer_delay * sizeof(FLOAT32));
  }

  error = ixheaace_mps_212_apply_enhanced_time_domain_dmx(
      pstr_space_enc->pstr_enhanced_time_dmx, pstr_space_enc->time_signal_in, ptr_output_samples,
      pstr_space_enc->n_surround_analysis_buffer_delay);
  if (error) {
    return error;
  }

  pstr_space_enc->n_samples_valid += samples_to_feed;

  *ptr_n_samples_fed = samples_to_feed;
  return error;
}

static IA_ERRORCODE ixheaace_mps_212_init_delay_compensation(
    ixheaace_mps_pstr_space_structure pstr_space_enc, const WORD32 core_coder_delay, WORD32 aot) {
  IA_ERRORCODE error = IA_NO_ERROR;
  WORD32 i;
  memset(&pstr_space_enc->pstr_delay->delay_config, 0,
         sizeof(pstr_space_enc->pstr_delay->delay_config));
  pstr_space_enc->core_coder_delay = core_coder_delay;
  pstr_space_enc->pstr_delay->delay_config.num_qmf_len = pstr_space_enc->num_qmf_bands;
  pstr_space_enc->pstr_delay->delay_config.num_frame_len = pstr_space_enc->n_frame_length;
  pstr_space_enc->pstr_delay->delay_config.num_core_coder_delay = core_coder_delay;
  pstr_space_enc->pstr_delay->delay_config.num_sac_stream_mux_delay =
      pstr_space_enc->time_alignment;
  pstr_space_enc->pstr_delay->delay_config.b_dmx_align = 0;
  pstr_space_enc->pstr_delay->delay_config.b_minimize_delay = 1;

  ixheaace_mps_212_delay_sub_calculate_buffer_delays(pstr_space_enc->pstr_delay);

  pstr_space_enc->n_bitstream_delay_buffer =
      pstr_space_enc->pstr_delay->num_bitstream_frame_buffer_size;
  pstr_space_enc->n_output_buffer_delay = pstr_space_enc->pstr_delay->num_output_audio_buffer;
  pstr_space_enc->n_surround_analysis_buffer_delay =
      pstr_space_enc->pstr_delay->num_surround_analysis_buffer;
  pstr_space_enc->n_bitstream_buffer_read = 0;
  pstr_space_enc->n_bitstream_buffer_write = pstr_space_enc->n_bitstream_delay_buffer - 1;
  pstr_space_enc->num_discard_out_frames = pstr_space_enc->pstr_delay->num_discard_out_frames;
  pstr_space_enc->n_input_delay = pstr_space_enc->pstr_delay->num_dmx_align_buffer;
  pstr_space_enc->independency_count = 0;
  pstr_space_enc->independency_flag = 1;

  for (i = 0; i < pstr_space_enc->n_bitstream_delay_buffer - 1; i++) {
    ixheaace_mps_spatial_frame *pstr_frame_data = NULL;
    pstr_frame_data = &pstr_space_enc->pstr_bitstream_formatter->frame;
    pstr_frame_data->bs_independency_flag = 1;
    pstr_frame_data->framing_info.num_param_sets = 1;
    pstr_frame_data->framing_info.bs_framing_type = 0;

    error = ixheaace_mps_212_write_spatial_frame(
        pstr_space_enc->bit_stream_delay_buffer[i], MAX_MPEGS_BYTES,
        &pstr_space_enc->pn_output_bits[i], pstr_space_enc->pstr_bitstream_formatter, aot);
    if (error) {
      return error;
    }
  }

  if ((pstr_space_enc->n_input_delay > MAX_DELAY_INPUT) ||
      (pstr_space_enc->n_output_buffer_delay > MAX_DELAY_OUTPUT) ||
      (pstr_space_enc->n_surround_analysis_buffer_delay > MAX_DELAY_SURROUND_ANALYSIS) ||
      (pstr_space_enc->n_bitstream_delay_buffer > MAX_BITSTREAM_DELAY)) {
    return IA_EXHEAACE_INIT_FATAL_MPS_INIT_FAILED;
  }
  return error;
}

static IA_ERRORCODE ixheaace_mps_212_fill_spatial_specific_config(
    const ixheaace_mps_pstr_space_structure pstr_space_enc,
    ixheaace_mps_spatial_specific_config *const pstr_h_ssc) {
  IA_ERRORCODE error = IA_NO_ERROR;

  ixheaace_mps_space_tree_description space_tree_description;
  WORD32 i;
  space_tree_description = pstr_space_enc->pstr_space_tree->descr;
  memset(pstr_h_ssc, 0, sizeof(ixheaace_mps_spatial_specific_config));

  pstr_h_ssc->num_bands = pstr_space_enc->space_tree_setup.num_param_bands;
  pstr_h_ssc->tree_description.num_ott_boxes = space_tree_description.num_ott_boxes;
  pstr_h_ssc->tree_description.num_in_chan = space_tree_description.num_in_channels;
  pstr_h_ssc->tree_description.num_out_chan = space_tree_description.num_out_channels;

  for (i = 0; i < IXHEAACE_MPS_MAX_NUM_BOXES; i++) {
    pstr_h_ssc->ott_config[i].bs_ott_bands = pstr_h_ssc->num_bands;
  }
  pstr_h_ssc->bs_tree_config = IXHEAACE_MPS_TREE_212;
  pstr_h_ssc->bs_sampling_frequency = pstr_space_enc->num_sample_rate;
  pstr_h_ssc->bs_frame_length = pstr_space_enc->num_frame_time_slots - 1;
  pstr_h_ssc->bs_decorr_config = IXHEAACE_MPS_DECORR_QMFSPLIT0;
  pstr_h_ssc->bs_quant_mode = pstr_space_enc->quant_mode;
  if (pstr_space_enc->pstr_static_gain->fixed_gain_dmx > IXHEAACE_MPS_MAX_FIXED_GAIN_DMX) {
    pstr_h_ssc->bs_fixed_gain_dmx = IXHEAACE_MPS_FIXED_GAIN_DMX_INVALID;
  } else {
    pstr_h_ssc->bs_fixed_gain_dmx = pstr_space_enc->pstr_static_gain->fixed_gain_dmx;
  }
  pstr_h_ssc->bs_env_quant_mode = 0;
  return error;
}

static IA_ERRORCODE ixheaace_mps_212_fill_space_tree_setup(
    const ixheaace_mps_pstr_space_structure pstr_space_enc,
    ixheaace_mps_space_tree_setup *const ptr_space_tree_setup) {
  IA_ERRORCODE error = IA_NO_ERROR;

  ptr_space_tree_setup->num_param_bands = pstr_space_enc->num_param_bands;
  ptr_space_tree_setup->use_coarse_quant_tto_cld_flag = pstr_space_enc->use_coarse_quant_cld;
  ptr_space_tree_setup->use_coarse_quant_tto_icc_flag = pstr_space_enc->use_coarse_quant_icc;
  ptr_space_tree_setup->quant_mode = pstr_space_enc->quant_mode;
  ptr_space_tree_setup->num_hybrid_bands_max = pstr_space_enc->num_hybrid_bands;
  ptr_space_tree_setup->num_channels_in_max = 2;
  return error;
}

static VOID ixheaace_mps_212_get_info(const ixheaace_mps_pstr_space_structure pstr_space_enc,
                                      ixheaace_mps_space_info *const pstr_space_info) {
  pstr_space_info->num_sample_rate = pstr_space_enc->num_sample_rate;
  pstr_space_info->num_samples_frame = pstr_space_enc->n_frame_length;
  pstr_space_info->num_total_input_channels = pstr_space_enc->n_input_channels;
  pstr_space_info->dmx_delay = pstr_space_enc->pstr_delay->num_info_dmx_delay;
  pstr_space_info->codec_delay = pstr_space_enc->pstr_delay->num_info_codec_delay;
  pstr_space_info->decoder_delay = pstr_space_enc->pstr_delay->num_info_decoder_delay;
  pstr_space_info->pay_load_delay =
      pstr_space_enc->pstr_delay->num_bitstream_frame_buffer_size - 1;
  pstr_space_info->num_discard_out_frames = pstr_space_enc->num_discard_out_frames;
  pstr_space_info->p_ssc_buf = &pstr_space_enc->ssc_buf;
}

static VOID ixheaace_mps_212_duplicate_parameter_set(
    const ixheaace_mps_spatial_frame *const pstr_spatial_frame_from, const WORD32 set_from,
    ixheaace_mps_spatial_frame *const pstr_spatial_frame_to, const WORD32 set_to) {
  WORD32 box;
  for (box = 0; box < IXHEAACE_MPS_MAX_NUM_BOXES; box++) {
    memcpy(pstr_spatial_frame_to->ott_data.cld[box][set_to],
           pstr_spatial_frame_from->ott_data.cld[box][set_from],
           sizeof(pstr_spatial_frame_from->ott_data.cld[0][0]));
    pstr_spatial_frame_to->cld_lossless_data.bs_xxx_data_mode[box][set_to] =
        pstr_spatial_frame_from->cld_lossless_data.bs_xxx_data_mode[box][set_from];
    pstr_spatial_frame_to->cld_lossless_data.bs_data_pair[box][set_to] =
        pstr_spatial_frame_from->cld_lossless_data.bs_data_pair[box][set_from];
    pstr_spatial_frame_to->cld_lossless_data.bs_quant_coarse_xxx[box][set_to] =
        pstr_spatial_frame_from->cld_lossless_data.bs_quant_coarse_xxx[box][set_from];
    pstr_spatial_frame_to->cld_lossless_data.bs_freq_res_stride_xxx[box][set_to] =
        pstr_spatial_frame_from->cld_lossless_data.bs_freq_res_stride_xxx[box][set_from];

    memcpy(pstr_spatial_frame_to->ott_data.icc[box][set_to],
           pstr_spatial_frame_from->ott_data.icc[box][set_from],
           sizeof(pstr_spatial_frame_from->ott_data.icc[0][0]));
    pstr_spatial_frame_to->icc_lossless_data.bs_xxx_data_mode[box][set_to] =
        pstr_spatial_frame_from->icc_lossless_data.bs_xxx_data_mode[box][set_from];
    pstr_spatial_frame_to->icc_lossless_data.bs_data_pair[box][set_to] =
        pstr_spatial_frame_from->icc_lossless_data.bs_data_pair[box][set_from];
    pstr_spatial_frame_to->icc_lossless_data.bs_quant_coarse_xxx[box][set_to] =
        pstr_spatial_frame_from->icc_lossless_data.bs_quant_coarse_xxx[box][set_from];
    pstr_spatial_frame_to->icc_lossless_data.bs_freq_res_stride_xxx[box][set_to] =
        pstr_spatial_frame_from->icc_lossless_data.bs_freq_res_stride_xxx[box][set_from];
  }
}

static IA_ERRORCODE ixheaace_mps_212_encode(
    const ixheaace_mps_pstr_space_structure pstr_space_enc,
    const ixheaace_mps_buf_descr *ptr_in_buf_desc, const ixheaace_mps_buf_descr *ptr_out_buf_desc,
    const ixheaace_mps_in_args *pstr_in_args, ixheaace_mps_out_args *pstr_out_args, WORD32 aot,
    WORD8 *ptr_scratch) {
  IA_ERRORCODE error = IA_NO_ERROR;
  WORD32 num_output_samples;
  WORD32 i, ch, ps, win_cnt, ts, slot;
  WORD32 curr_trans_pos = -1;
  WORD32 num_ch_in;
  WORD32 num_ch_in_with_dmx;
  WORD32 num_ch_out;
  WORD32 num_samples_per_channel;
  WORD32 num_output_samples_max;
  WORD32 num_frame_time_slots;
  WORD32 num_frame_time_slots_reduction;
  ixheaace_mps_spatial_frame *pstr_frame_data = NULL;
  UWORD8 *ptr_bitstream_delay_buffer;
  const FLOAT32 *ptr_input_samples =
      (const FLOAT32 *)ptr_in_buf_desc->pp_base[IXHEAACE_MPS_INPUT_BUFFER_IDX];

  FLOAT32 *const ptr_output_samples =
      (FLOAT32 *)ptr_out_buf_desc->pp_base[IXHEAACE_MPS_OUTUT_BUFFER_IDX];

  const WORD32 num_output_samples_buffer_size =
      ptr_out_buf_desc->p_buf_size[IXHEAACE_MPS_OUTUT_BUFFER_IDX] /
      ptr_out_buf_desc->p_ele_size[IXHEAACE_MPS_OUTUT_BUFFER_IDX];
  num_ch_in = pstr_space_enc->n_input_channels;
  num_ch_in_with_dmx = num_ch_in;
  num_ch_out = pstr_space_enc->n_output_channels;
  num_samples_per_channel = pstr_in_args->num_input_samples / num_ch_in_with_dmx;
  num_output_samples_max = num_samples_per_channel * num_ch_out;
  num_frame_time_slots = pstr_space_enc->num_frame_time_slots;
  num_frame_time_slots_reduction = pstr_space_enc->num_frame_time_slots >> 1;
  if ((0 != pstr_in_args->num_input_samples % num_ch_in_with_dmx)) {
    return IA_EXHEAACE_CONFIG_NONFATAL_MPS_INVALID_CONFIG;
  }
  pstr_frame_data = &pstr_space_enc->pstr_bitstream_formatter->frame;

  if (aot != AOT_USAC) {
    if (pstr_space_enc->num_discard_out_frames > 0) {
      pstr_space_enc->independency_count = 0;
      pstr_space_enc->independency_flag = 1;
    } else {
      pstr_space_enc->independency_flag = (pstr_space_enc->independency_count == 0) ? 1 : 0;
      if (pstr_space_enc->independency_factor > 0) {
        pstr_space_enc->independency_count++;
        pstr_space_enc->independency_count =
            pstr_space_enc->independency_count % ((WORD32)pstr_space_enc->independency_factor);
      } else {
        pstr_space_enc->independency_count = -1;
      }
    }
  }

  error = ixheaace_mps_212_feed_de_inter_pre_scale(
      pstr_space_enc, ptr_input_samples, ptr_output_samples, pstr_in_args->num_input_samples,
      pstr_in_args->is_input_inter_leaved, pstr_in_args->input_buffer_size_per_channel,
      &pstr_out_args->num_samples_consumed);
  if (error) {
    return error;
  }

  if (pstr_space_enc->n_samples_next != pstr_space_enc->n_samples_valid) {
    return IA_EXHEAACE_CONFIG_NONFATAL_MPS_INVALID_CONFIG;
  }

  for (ch = 0; ch < num_ch_in; ch++) {
    for (slot = 0; slot < num_frame_time_slots; slot++) {
      ixheaace_cmplx_str *pstr_cmplx_data;

      pstr_cmplx_data =
          pstr_space_enc
              ->cmplx_hybrid_in[ch][pstr_space_enc->n_update_hybrid_position_time_slots +
                                    num_frame_time_slots - num_frame_time_slots_reduction + slot];
      memset(pstr_cmplx_data, 0, 2 * pstr_space_enc->num_hybrid_bands * sizeof(WORD32));
    }
  }

  for (ch = 0; ch < num_ch_in; ch++) {
    FLOAT32 qmf_in_real[MAX_QMF_BANDS];
    FLOAT32 qmf_in_imag[MAX_QMF_BANDS];
    FLOAT32 p_work_buffer[MAX_QMF_BANDS << 1];
    FLOAT32 *ptr_pre_gain = pstr_space_enc->pstr_static_gain->pre_gain;
    for (ts = 0; ts < num_frame_time_slots; ts++) {
      FLOAT32 *ptr_spec_real;
      FLOAT32 *ptr_spec_imag;

      FLOAT32 *ptr_time_in =
          &pstr_space_enc->time_signal_in[ch][(ts * pstr_space_enc->num_qmf_bands)];

      ixheaace_mps_212_apply_dc_filter(pstr_space_enc->pstr_dc_filter[ch], ptr_time_in,
                                       ptr_time_in, pstr_space_enc->num_qmf_bands);

      error = ixheaace_mps_212_qmf_analysis_filtering_slot(pstr_space_enc->pstr_qmf_filter_in[ch],
                                                           qmf_in_real, qmf_in_imag, ptr_time_in,
                                                           1, p_work_buffer, ptr_scratch);
      if (error != IA_NO_ERROR) {
        return error;
      }

      ptr_spec_real = qmf_in_real;
      ptr_spec_imag = qmf_in_imag;
      if (1.0f != ptr_pre_gain[ch]) {
        for (i = 0; i < pstr_space_enc->num_hybrid_bands; i++) {
          pstr_space_enc
              ->cmplx_hybrid_in[ch][pstr_space_enc->n_analysis_lookahead_time_slots + ts][i]
              .re = ptr_spec_real[i] * ptr_pre_gain[ch];
          pstr_space_enc
              ->cmplx_hybrid_in[ch][pstr_space_enc->n_analysis_lookahead_time_slots + ts][i]
              .im = ptr_spec_imag[i] * ptr_pre_gain[ch];
        }
      } else {
        for (i = 0; i < pstr_space_enc->num_hybrid_bands; i++) {
          pstr_space_enc
              ->cmplx_hybrid_in[ch][pstr_space_enc->n_analysis_lookahead_time_slots + ts][i]
              .re = ptr_spec_real[i];
          pstr_space_enc
              ->cmplx_hybrid_in[ch][pstr_space_enc->n_analysis_lookahead_time_slots + ts][i]
              .im = ptr_spec_imag[i];
        }
      }
    }
  }

  for (ch = 0; ch < num_ch_in; ch++) {
    for (slot = 0; slot < (WORD32)(pstr_space_enc->n_update_hybrid_position_time_slots +
                                   num_frame_time_slots - num_frame_time_slots_reduction);
         slot++) {
      memmove(pstr_space_enc->cmplx_hybrid_in[ch][slot],
              pstr_space_enc->cmplx_hybrid_in_static[ch][slot],
              sizeof(ixheaace_cmplx_str) * pstr_space_enc->num_hybrid_bands);
    }

    for (slot = 0; slot < (WORD32)(pstr_space_enc->n_update_hybrid_position_time_slots +
                                   num_frame_time_slots - num_frame_time_slots_reduction);
         slot++) {
      memmove(pstr_space_enc->cmplx_hybrid_in_static[ch][slot],
              pstr_space_enc->cmplx_hybrid_in[ch][num_frame_time_slots + slot],
              sizeof(ixheaace_cmplx_str) * pstr_space_enc->num_hybrid_bands);
    }

    error = ixheaace_mps_212_onset_detect_apply(
        pstr_space_enc->pstr_onset_detect[ch], num_frame_time_slots,
        &pstr_space_enc->cmplx_hybrid_in[ch][pstr_space_enc->n_analysis_lookahead_time_slots],
        pstr_space_enc->tr_prev_pos[1], pstr_space_enc->pp_tr_curr_pos[ch]);
    if (error) {
      return error;
    }

    if (pstr_space_enc->use_frame_keep == 0) {
      pstr_space_enc->pp_tr_curr_pos[ch][0] = -1;
    }

    if ((pstr_space_enc->pp_tr_curr_pos[ch][0] >= 0) &&
        ((curr_trans_pos < 0) || (pstr_space_enc->pp_tr_curr_pos[ch][0] < curr_trans_pos))) {
      curr_trans_pos = pstr_space_enc->pp_tr_curr_pos[ch][0];
    }
  }
  if (pstr_space_enc->use_frame_keep == 1) {
    if ((curr_trans_pos != -1) || (pstr_space_enc->independency_flag == 1)) {
      pstr_space_enc->avoid_keep = NUM_KEEP_WINDOWS;
      curr_trans_pos = -1;
    }
  }

  pstr_space_enc->tr_prev_pos[0] = MAX(-1, pstr_space_enc->tr_prev_pos[1] - num_frame_time_slots);
  pstr_space_enc->tr_prev_pos[1] = curr_trans_pos;

  for (ch = 0; ch < num_ch_in; ch++) {
    error = ixheaace_mps_212_onset_detect_update(pstr_space_enc->pstr_onset_detect[ch],
                                                 num_frame_time_slots);
    if (error) {
      return error;
    }
  }

  ixheaace_mps_212_frame_window_get_window(
      pstr_space_enc->h_frame_window, pstr_space_enc->tr_prev_pos, num_frame_time_slots,
      &pstr_frame_data->framing_info, pstr_space_enc->ptr_frame_window_ana,
      &pstr_space_enc->frame_win_list, pstr_space_enc->avoid_keep);

  for (ps = 0, win_cnt = 0; ps < pstr_space_enc->frame_win_list.win_list_cnt; ++ps) {
    if (pstr_space_enc->frame_win_list.dat[ps].hold == IXHEAACE_MPS_FRAME_WINDOWING_HOLD) {
      ixheaace_mps_212_duplicate_parameter_set(&pstr_space_enc->save_frame, 0, pstr_frame_data,
                                               ps);
    } else {
      WORD32 num_analysis_length_time_slots, analysis_start_time_slot;
      FLOAT32 *ptr_frame_window_ana;
      num_analysis_length_time_slots = ixheaace_mps_212_get_analysis_length_time_slots(
          pstr_space_enc->ptr_frame_window_ana[win_cnt],
          pstr_space_enc->num_analysis_length_time_slots);

      analysis_start_time_slot = ixheaace_mps_212_get_analysis_start_time_slot(
          pstr_space_enc->ptr_frame_window_ana[win_cnt],
          pstr_space_enc->num_analysis_length_time_slots);

      ptr_frame_window_ana = pstr_space_enc->ptr_frame_window_ana[win_cnt];

      error = ixheaace_mps_212_space_tree_apply(
          pstr_space_enc->pstr_space_tree, ps, num_ch_in, num_analysis_length_time_slots,
          analysis_start_time_slot, pstr_space_enc->num_hybrid_bands, ptr_frame_window_ana,
          pstr_space_enc->cmplx_hybrid_in, pstr_space_enc->cmplx_hybrid_in, pstr_frame_data,
          pstr_space_enc->avoid_keep);
      if (error) {
        return error;
      }

      ixheaace_mps_212_duplicate_parameter_set(pstr_frame_data, ps, &pstr_space_enc->save_frame,
                                               0);
      ++win_cnt;
    }
    if (pstr_space_enc->avoid_keep > 0) {
      pstr_space_enc->avoid_keep--;
    }
  }

  memset(&pstr_frame_data->smg_data, 0, sizeof(pstr_frame_data->smg_data));

  ptr_bitstream_delay_buffer =
      (UWORD8 *)ptr_out_buf_desc->pp_base[IXHEAACE_MPS_BITSTREAM_BUFFER_IDX];
  pstr_frame_data->bs_independency_flag = pstr_space_enc->independency_flag;

  error = ixheaace_mps_212_write_spatial_frame(
      ptr_bitstream_delay_buffer, MAX_MPEGS_BYTES,
      &pstr_space_enc->pn_output_bits[pstr_space_enc->n_bitstream_buffer_write],
      pstr_space_enc->pstr_bitstream_formatter, aot);
  if (error) {
    return error;
  }

  if ((pstr_space_enc->num_discard_out_frames == 0) &&
      (IXHEAACE_MPS_BITSTREAM_BUFFER_IDX != -1)) {
    const WORD32 idx = IXHEAACE_MPS_BITSTREAM_BUFFER_IDX;
    const WORD32 out_bits =
        pstr_space_enc->pn_output_bits[pstr_space_enc->n_bitstream_buffer_read];

    if (((out_bits + 7) / 8) >
        (WORD32)(ptr_out_buf_desc->p_buf_size[idx] / ptr_out_buf_desc->p_ele_size[idx])) {
      pstr_out_args->num_output_bits = 0;
      return IA_EXHEAACE_EXE_NONFATAL_MPS_ENCODE_ERROR;
    }
    pstr_out_args->num_output_bits = out_bits;
  } else {
    pstr_out_args->num_output_bits = 0;
  }

  pstr_space_enc->n_bitstream_buffer_read =
      (pstr_space_enc->n_bitstream_buffer_read + 1) % pstr_space_enc->n_bitstream_delay_buffer;
  pstr_space_enc->n_bitstream_buffer_write =
      (pstr_space_enc->n_bitstream_buffer_write + 1) % pstr_space_enc->n_bitstream_delay_buffer;

  num_output_samples =
      (pstr_space_enc->num_discard_out_frames == 0) ? (num_output_samples_max) : 0;
  if (num_output_samples > num_output_samples_buffer_size) {
    return IA_EXHEAACE_CONFIG_NONFATAL_MPS_INVALID_CONFIG;
  }
  pstr_out_args->num_output_samples = num_output_samples;

  if (pstr_space_enc->num_discard_out_frames > 0) {
    pstr_space_enc->num_discard_out_frames--;
  }
  pstr_space_enc->n_samples_valid = 0;

  return error;
}

static IA_ERRORCODE ixheaace_mps_212_init(ixheaace_mps_pstr_space_structure pstr_space_enc,
                                          const WORD32 dmx_delay, WORD32 aot) {
  IA_ERRORCODE error = IA_NO_ERROR;
  WORD32 ch;
  WORD32 num_ch_in_arb_dmx;
  ixheaace_mps_space_tree_description space_tree_description;
  ixheaace_mps_onset_detect_config onset_detect_config;
  ixheaace_mps_frame_win_config frame_window_config;

  error = ixheaace_mps_212_configure(pstr_space_enc, &pstr_space_enc->user);
  if (error) {
    return error;
  }
  pstr_space_enc->b_enc_mode_212_only = pstr_space_enc->setup->b_enc_mode_212;

  if (pstr_space_enc->num_frame_time_slots < 1) {
    return IA_EXHEAACE_CONFIG_NONFATAL_MPS_INVALID_CONFIG;
  }
  pstr_space_enc->n_frame_length =
      pstr_space_enc->num_qmf_bands * pstr_space_enc->num_frame_time_slots;

  if (pstr_space_enc->use_frame_keep == 1) {
    pstr_space_enc->num_analysis_length_time_slots = 3 * pstr_space_enc->num_frame_time_slots;
    pstr_space_enc->n_update_hybrid_position_time_slots = pstr_space_enc->num_frame_time_slots;
  } else {
    pstr_space_enc->num_analysis_length_time_slots = 2 * pstr_space_enc->num_frame_time_slots;
    pstr_space_enc->n_update_hybrid_position_time_slots = 0;
  }

  pstr_space_enc->n_analysis_lookahead_time_slots =
      pstr_space_enc->num_analysis_length_time_slots -
      3 * pstr_space_enc->num_frame_time_slots / 2;

  ixheaace_mps_212_calc_parameter_band_to_hybrid_band_offset(
      pstr_space_enc->num_hybrid_bands, pstr_space_enc->ptr_parameter_band_2_hybrid_band_offset,
      aot);

  error =
      ixheaace_mps_212_fill_space_tree_setup(pstr_space_enc, &pstr_space_enc->space_tree_setup);
  if (error) {
    return error;
  }
  error = ixheaace_mps_212_space_tree_init(
      pstr_space_enc->pstr_space_tree, &pstr_space_enc->space_tree_setup,
      pstr_space_enc->ptr_parameter_band_2_hybrid_band_offset, pstr_space_enc->use_frame_keep,
      aot);
  if (error) {
    return error;
  }

  space_tree_description = pstr_space_enc->pstr_space_tree->descr;
  pstr_space_enc->n_input_channels = space_tree_description.num_out_channels;
  pstr_space_enc->n_output_channels = space_tree_description.num_in_channels;
  frame_window_config.num_time_slots_max = pstr_space_enc->num_frame_time_slots;
  frame_window_config.frame_keep_flag = pstr_space_enc->use_frame_keep;
  onset_detect_config.max_time_slots = pstr_space_enc->num_frame_time_slots;
  onset_detect_config.lower_bound_onset_detection =
      ((2 * 1725 * pstr_space_enc->num_qmf_bands) / pstr_space_enc->num_sample_rate);
  onset_detect_config.upper_bound_onset_detection = pstr_space_enc->num_hybrid_bands;
  num_ch_in_arb_dmx = 0;

  for (ch = 0; ch < pstr_space_enc->n_input_channels; ch++) {
    ixheaace_mps_212_qmf_init_filter_bank(
        pstr_space_enc->pstr_qmf_filter_in[ch],
        pstr_space_enc->pstr_qmf_filter_in[ch]->ptr_filter_states, 1,
        pstr_space_enc->num_qmf_bands, pstr_space_enc->num_qmf_bands,
        pstr_space_enc->num_qmf_bands);
    if (error) {
      return error;
    }
    ixheaace_mps_212_init_dc_filter(pstr_space_enc->pstr_dc_filter[ch],
                                    pstr_space_enc->num_sample_rate);
    error = ixheaace_mps_212_onset_detect_init(pstr_space_enc->pstr_onset_detect[ch],
                                               &onset_detect_config, 1);
    if (error) {
      return error;
    }
  }

  error =
      ixheaace_mps_212_frame_window_init(pstr_space_enc->h_frame_window, &frame_window_config);
  if (error) {
    return error;
  }

  error = ixheaace_mps_212_static_gain_init(pstr_space_enc->pstr_static_gain,
                                            pstr_space_enc->pstr_static_gain_config);
  if (error) {
    return error;
  }

  error = ixheaace_mps_212_init_enhanced_time_domain_dmx(
      pstr_space_enc->pstr_enhanced_time_dmx, pstr_space_enc->pstr_static_gain->pre_gain,
      pstr_space_enc->pstr_static_gain->post_gain, pstr_space_enc->n_frame_length);
  if (error) {
    return error;
  }

  memset(pstr_space_enc->pstr_bitstream_formatter, 0, sizeof(ixheaace_mps_spatial_frame));
  pstr_space_enc->pstr_bitstream_formatter->frame.bs_independency_flag = 1;
  pstr_space_enc->pstr_bitstream_formatter->frame.framing_info.num_param_sets = 1;

  error = ixheaace_mps_212_fill_spatial_specific_config(
      pstr_space_enc, &pstr_space_enc->pstr_bitstream_formatter->spatial_specific_config);
  if (error) {
    return error;
  }

  error = ixheaace_mps_212_write_spatial_specific_config(
      &pstr_space_enc->pstr_bitstream_formatter->spatial_specific_config,
      pstr_space_enc->ssc_buf.ptr_ssc, MAX_SSC_BYTES, &pstr_space_enc->ssc_buf.num_ssc_size_bits,
      aot);
  if (error) {
    return error;
  }

  error = ixheaace_mps_212_init_delay_compensation(pstr_space_enc, dmx_delay, aot);
  if (error) {
    return error;
  }

  pstr_space_enc->n_samples_next =
      pstr_space_enc->n_frame_length * (pstr_space_enc->n_input_channels + num_ch_in_arb_dmx);
  pstr_space_enc->n_samples_valid = 0;

  return error;
}

static VOID ixheaace_mps_open(ixheaace_mps_pstr_space_structure *pstr_space_enc_structure,
                              ixheaace_mps_212_memory_struct *pstr_mps_memory) {
  WORD32 ch;
  WORD32 param;
  ixheaace_mps_pstr_space_structure pstr_space_enc = NULL;
  pstr_space_enc = &pstr_mps_memory->mp4_space_encoder_instance;
  if (NULL != pstr_space_enc) {
    memset(pstr_space_enc, 0, sizeof(struct ixheaace_mps_space_structure));
  }
  pstr_space_enc->setup = &pstr_mps_memory->setup;
  pstr_space_enc->setup->max_sampling_rate = MAX_SAMPLING_RATE;
  pstr_space_enc->setup->max_frame_time_slots = MAX_FRAME_TIME_SLOT;
  pstr_space_enc->setup->max_analysis_length_time_slots =
      3 * pstr_space_enc->setup->max_frame_time_slots;
  pstr_space_enc->setup->max_qmf_bands =
      ixheaace_mps_212_space_get_num_qmf_bands(pstr_space_enc->setup->max_sampling_rate);
  pstr_space_enc->setup->max_hybrid_bands = pstr_space_enc->setup->max_qmf_bands;
  pstr_space_enc->setup->max_frame_length =
      pstr_space_enc->setup->max_qmf_bands * pstr_space_enc->setup->max_frame_time_slots;
  pstr_space_enc->setup->max_ch_in = 2;
  pstr_space_enc->setup->max_ch_out = 1;
  pstr_space_enc->setup->max_ch_tot_out = pstr_space_enc->setup->max_ch_out;
  pstr_space_enc->setup->b_enc_mode_212 = 1;
  pstr_space_enc->setup->max_hybrid_in_static_slots = 24;

  pstr_space_enc->pstr_static_gain_config = &pstr_mps_memory->static_gain_config;
  pstr_space_enc->pstr_enhanced_time_dmx = &pstr_mps_memory->enhanced_time_dmx;
  pstr_space_enc->pstr_enhanced_time_dmx->sinus_window = pstr_mps_memory->sinus_window_flt;
  pstr_space_enc->ptr_parameter_band_2_hybrid_band_offset =
      pstr_mps_memory->parameter_band_2_hybrid_band_offset;
  ixheaace_mps_212_space_tree_open(&pstr_space_enc->pstr_space_tree, pstr_mps_memory);
  pstr_space_enc->pstr_qmf_filter_in = pstr_mps_memory->pstr_qmf_filter_bank;
  for (ch = 0; ch < pstr_space_enc->setup->max_ch_in; ch++) {
    pstr_space_enc->pstr_qmf_filter_in[ch] = &pstr_mps_memory->qmf_filter_bank[ch];
    pstr_space_enc->pstr_qmf_filter_in[ch]->ptr_filter_states =
        &pstr_mps_memory->ptr_filter_states[ch];
    pstr_space_enc->pstr_dc_filter[ch] = &pstr_mps_memory->dc_filter[ch];
    pstr_space_enc->pstr_onset_detect[ch] = &pstr_mps_memory->onset_detect[ch];
    pstr_space_enc->pstr_onset_detect[ch]->p_energy_hist =
        &pstr_mps_memory->energy_hist_float[ch][0];
  }
  pstr_space_enc->h_frame_window = &pstr_mps_memory->frame_window;
  pstr_space_enc->pstr_static_gain = &pstr_mps_memory->static_gain;
  pstr_space_enc->pstr_bitstream_formatter = &pstr_mps_memory->bitstream;
  pstr_space_enc->ssc_buf.ptr_ssc = pstr_mps_memory->ssc;
  pstr_space_enc->pstr_delay = &pstr_mps_memory->delay;
  pstr_space_enc->pn_output_bits = pstr_mps_memory->n_output_bits;
  for (param = 0; param < MAX_NUM_PARAMS; param++) {
    pstr_space_enc->ptr_frame_window_ana[param] =
        &pstr_mps_memory->frame_window_ana_flt[param][0];
  }
  pstr_space_enc->pstr_enhanced_time_dmx->max_frame_length =
      pstr_space_enc->setup->max_frame_length;
  for (ch = 0; ch < pstr_space_enc->setup->max_ch_in; ch++) {
    pstr_space_enc->pstr_onset_detect[ch]->max_time_slots =
        pstr_space_enc->setup->max_frame_time_slots;
    pstr_space_enc->pstr_onset_detect[ch]->min_trans_dist = 8;
    pstr_space_enc->pstr_onset_detect[ch]->avg_energy_dist = 16;
    pstr_space_enc->pstr_onset_detect[ch]->avg_energy_dist_scale = 4;
  }
  pstr_space_enc->pstr_static_gain_config->fixed_gain_dmx = IXHEAACE_MPS_DMX_GAIN_DEFAULT;
  pstr_space_enc->pstr_static_gain_config->pre_gain_factor_db = 0;

  if (NULL != pstr_space_enc_structure) {
    *pstr_space_enc_structure = pstr_space_enc;
  }
}

static WORD32 ixheaace_mps_212_get_closest_bit_rate(const WORD32 audio_object_type,
                                                    const UWORD32 sampling_rate,
                                                    const UWORD32 sbr_ratio, UWORD32 bitrate) {
  UWORD32 idx;
  WORD32 target_bitrate = -1;

  for (idx = 0; idx < sizeof(mps_config_tab) / sizeof(ixheaace_mps_config_table); idx++) {
    if ((mps_config_tab[idx].sampling_rate == sampling_rate) &&
        (mps_config_tab[idx].audio_object_type == audio_object_type) &&
        (mps_config_tab[idx].sbr_ratio == sbr_ratio)) {
      target_bitrate =
          MIN(MAX(bitrate, mps_config_tab[idx].bitrate_min), mps_config_tab[idx].bitrate_max);
    }
  }

  return target_bitrate;
}

static WORD32 ixheaace_mps_212_write_spatial_specific_config_data(
    ixheaace_mps_pstr_struct pstr_mps_enc, ixheaace_bit_buf_handle pstr_bit_buf) {
  WORD32 idx;
  WORD32 ssc_bits = 0;
  WORD32 written_bits = 0;
  ixheaace_mps_space_info pstr_space_encoder_info;
  ixheaace_mps_212_get_info(pstr_mps_enc->ptr_sac_encoder, &pstr_space_encoder_info);

  for (idx = 0; idx<pstr_space_encoder_info.p_ssc_buf->num_ssc_size_bits>> 3; idx++) {
    ixheaace_write_bits(pstr_bit_buf, pstr_space_encoder_info.p_ssc_buf->ptr_ssc[idx], 8);
    written_bits += 8;
  }
  ixheaace_write_bits(
      pstr_bit_buf, pstr_space_encoder_info.p_ssc_buf->ptr_ssc[idx],
      (UWORD8)(pstr_space_encoder_info.p_ssc_buf->num_ssc_size_bits - written_bits));

  ssc_bits = pstr_space_encoder_info.p_ssc_buf->num_ssc_size_bits;
  return ssc_bits;
}

VOID ixheaace_mps_212_open(VOID **pstr_handle_mps,
                           ixheaace_mps_212_memory_struct *pstr_mps_memory) {
  ixheaace_mps_pstr_struct pstr_mps_enc = NULL;
  pstr_mps_enc = &pstr_mps_memory->mps_encoder_instance;
  memset(pstr_mps_enc, 0, sizeof(ixheaace_mps_struct));
  ixheaace_mps_open(&pstr_mps_enc->ptr_sac_encoder, pstr_mps_memory);
  *pstr_handle_mps = pstr_mps_enc;
}

VOID ixheaace_mps_212_close(VOID **pstr_handle_mps) {
  ixheaace_mps_pstr_struct *pstr_mps_enc = (ixheaace_mps_pstr_struct *)pstr_handle_mps;
  if (*pstr_mps_enc != NULL) {
    *pstr_mps_enc = NULL;
  }
}

IA_ERRORCODE ixheaace_mps_212_initialise(VOID *pstr_handle_mps, const WORD32 audio_object_type,
                                         const UWORD32 sampling_rate, WORD32 *ptr_bitrate,
                                         const UWORD32 sbr_ratio, const UWORD32 frame_length,
                                         const UWORD32 input_buffer_size_per_channel,
                                         const UWORD32 core_coder_delay, WORD8 *ptr_scratch) {
  IA_ERRORCODE error = IA_NO_ERROR;
  const UWORD32 fs_low = 27713;
  const UWORD32 fs_high = 55426;
  const UWORD32 no_inter_frame_coding = 0;
  UWORD32 num_time_slots = 0, num_qmf_bands_ld = 0;
  ixheaace_mps_pstr_struct pstr_mps_enc = (ixheaace_mps_pstr_struct)pstr_handle_mps;
  switch (sbr_ratio) {
    case 1:
      if (!(sampling_rate < fs_low)) {
        return IA_EXHEAACE_INIT_FATAL_MPS_INIT_FAILED;
      }
      break;
    case 2:
    case 4:
      if (!((sampling_rate >= fs_low) && (sampling_rate < fs_high))) {
        return IA_EXHEAACE_INIT_FATAL_MPS_INIT_FAILED;
      }
      break;
    case 0:
    default:;
  }

  num_qmf_bands_ld = (sampling_rate < fs_low) ? 5 : ((sampling_rate > fs_high) ? 7 : 6);
  num_time_slots = frame_length >> num_qmf_bands_ld;
  *ptr_bitrate = ixheaace_mps_212_get_closest_bit_rate(audio_object_type, sampling_rate,
                                                       sbr_ratio, *ptr_bitrate);

  pstr_mps_enc->ptr_sac_encoder->user.b_ld_mode = ((no_inter_frame_coding == 1) ? 1 : 2);
  pstr_mps_enc->ptr_sac_encoder->user.sample_rate = sampling_rate;
  pstr_mps_enc->ptr_sac_encoder->user.frame_time_slots = num_time_slots;
  if (audio_object_type == AOT_AAC_ELD) {
    pstr_mps_enc->ptr_sac_encoder->user.num_param_bands = IXHEAACE_MPS_SAC_BANDS_ld;
    pstr_mps_enc->ptr_sac_encoder->user.independency_factor = 20;
  } else {
    pstr_mps_enc->ptr_sac_encoder->user.num_param_bands = IXHEAACE_MPS_SAC_BANDS_usac;
    pstr_mps_enc->ptr_sac_encoder->user.independency_factor = 25;
  }

  pstr_mps_enc->ptr_sac_encoder->user.b_use_coarse_quant = 0;
  pstr_mps_enc->ptr_sac_encoder->user.quant_mode = IXHEAACE_MPS_QUANTMODE_FINE;
  pstr_mps_enc->ptr_sac_encoder->user.time_alignment = 0;
  pstr_mps_enc->audio_object_type = audio_object_type;
  error =
      ixheaace_mps_212_init(pstr_mps_enc->ptr_sac_encoder, core_coder_delay, audio_object_type);
  if (error) {
    return IA_EXHEAACE_INIT_FATAL_MPS_INIT_FAILED;
  }

  pstr_mps_enc->in_buf_desc.pp_base = (VOID **)&pstr_mps_enc->p_in_buffer;
  pstr_mps_enc->in_buf_desc.p_buf_size = pstr_mps_enc->p_in_buffer_size;
  pstr_mps_enc->in_buf_desc.p_ele_size = pstr_mps_enc->p_in_buffer_el_size;
  pstr_mps_enc->in_buf_desc.p_buf_type = pstr_mps_enc->p_in_buffer_type;
  pstr_mps_enc->in_buf_desc.num_bufs = 1;

  pstr_mps_enc->out_buf_desc.pp_base = (VOID **)&pstr_mps_enc->p_out_buffer;
  pstr_mps_enc->out_buf_desc.p_buf_size = pstr_mps_enc->p_out_buffer_size;
  pstr_mps_enc->out_buf_desc.p_ele_size = pstr_mps_enc->p_out_buffer_el_size;
  pstr_mps_enc->out_buf_desc.p_buf_type = pstr_mps_enc->p_out_buffer_type;
  pstr_mps_enc->out_buf_desc.num_bufs = 2;

  pstr_mps_enc->p_in_buffer[0] = NULL;
  pstr_mps_enc->p_in_buffer_size[0] = 0;
  pstr_mps_enc->p_in_buffer_el_size[0] = sizeof(FLOAT32);
  pstr_mps_enc->p_in_buffer_type[0] = IXHEAACE_MPS_INPUT_BUFFER_IDX;

  pstr_mps_enc->p_out_buffer[0] = NULL;
  pstr_mps_enc->p_out_buffer_size[0] = 0;
  pstr_mps_enc->p_out_buffer_el_size[0] = sizeof(FLOAT32);
  pstr_mps_enc->p_out_buffer_type[0] = IXHEAACE_MPS_OUTUT_BUFFER_IDX;

  pstr_mps_enc->p_out_buffer[1] = NULL;
  pstr_mps_enc->p_out_buffer_size[1] = 0;
  pstr_mps_enc->p_out_buffer_el_size[1] = sizeof(UWORD8);
  pstr_mps_enc->p_out_buffer_type[1] = IXHEAACE_MPS_BITSTREAM_BUFFER_IDX;

  pstr_mps_enc->in_args.is_input_inter_leaved = 0;
  pstr_mps_enc->in_args.input_buffer_size_per_channel = input_buffer_size_per_channel;

  pstr_mps_enc->ptr_scratch = ptr_scratch;

  return error;
}

IA_ERRORCODE ixheaace_mps_212_process(VOID *pstr_handle_mps, FLOAT32 *const ptr_audio_samples,
                                      const WORD32 num_audio_samples,
                                      ixheaace_mps_enc_ext_payload *pstr_mps_ext_payload) {
  IA_ERRORCODE error = IA_NO_ERROR;
  ixheaace_mps_pstr_struct pstr_mps_enc = (ixheaace_mps_pstr_struct)pstr_handle_mps;
  WORD32 aot = pstr_mps_enc->audio_object_type;
  WORD32 sac_header_flag = 0;
  WORD32 sac_out_buffer_offset = 0;

  if (aot == AOT_AAC_ELD) {
    pstr_mps_enc->sac_out_buffer[0] = (sac_header_flag == 0) ? 0x3 : 0x7;
    sac_out_buffer_offset += 1;
  }

  pstr_mps_enc->p_in_buffer[0] = (VOID *)ptr_audio_samples;
  pstr_mps_enc->in_args.num_input_samples = num_audio_samples;

  pstr_mps_enc->p_out_buffer[0] = (VOID *)ptr_audio_samples;
  pstr_mps_enc->p_out_buffer_size[0] = sizeof(FLOAT32) * ((WORD32)num_audio_samples) / 2;

  pstr_mps_enc->p_out_buffer[1] = (VOID *)&pstr_mps_enc->sac_out_buffer[sac_out_buffer_offset];
  pstr_mps_enc->p_out_buffer_size[1] =
      sizeof(pstr_mps_enc->sac_out_buffer) - sac_out_buffer_offset;

  error = ixheaace_mps_212_encode(pstr_mps_enc->ptr_sac_encoder, &pstr_mps_enc->in_buf_desc,
                                  &pstr_mps_enc->out_buf_desc, &pstr_mps_enc->in_args,
                                  &pstr_mps_enc->out_args, aot, pstr_mps_enc->ptr_scratch);
  if (error) {
    return error;
  }

  pstr_mps_ext_payload->p_data = (UWORD8 *)pstr_mps_enc->sac_out_buffer;
  pstr_mps_ext_payload->data_size = pstr_mps_enc->out_args.num_output_bits;
  if (aot == AOT_AAC_ELD) {
    pstr_mps_ext_payload->data_size += 8 * (sac_out_buffer_offset - 1);
  }
  pstr_mps_ext_payload->data_type = IXHEAACE_MPS_EXT_LDSAC_DATA;
  pstr_mps_ext_payload->associated_ch_element = -1;

  return error;
}

WORD32 ixheaace_mps_212_get_spatial_specific_config(VOID *pstr_handle_mps, WORD8 *ptr_out_buffer,
                                                    WORD32 buf_size, WORD32 aot) {
  ixheaace_bit_buf bit_buf;
  ixheaace_bit_buf_handle pstr_bit_buf =
      ia_enhaacplus_enc_create_bitbuffer(&bit_buf, (UWORD8 *)ptr_out_buffer, buf_size);
  ixheaace_mps_212_write_spatial_specific_config_data(pstr_handle_mps, pstr_bit_buf);
  if (aot == AOT_AAC_ELD) {
    ixheaace_byte_align_buffer(pstr_bit_buf);
  }
  return ia_enhaacplus_enc_get_bits_available(pstr_bit_buf);
}

WORD32 ixheaace_mps_515_scratch_size(VOID) {
  WORD32 size = 0;
  size += ((MAX_INPUT_CHANNELS * MAX_BUFFER_SIZE) * sizeof(FLOAT32));
  size += ((MAX_OUTPUT_CHANNELS * MAX_BUFFER_SIZE) * sizeof(FLOAT32));
  size +=
      (((MAX_INPUT_CHANNELS * MAX_BUFFER_SIZE) + (2 * MAX_BUFFER_SIZE) + (2 * MAX_BUFFER_SIZE)) *
       sizeof(FLOAT32));
  size += (((6 * MAX_TIME_SLOTS * NUM_QMF_BANDS) + (6 * MAX_TIME_SLOTS * NUM_QMF_BANDS) +
            (6 * MAX_TIME_SLOTS * NUM_QMF_BANDS) + (6 * MAX_TIME_SLOTS * NUM_QMF_BANDS) +
            (6 * MAX_TIME_SLOTS * MAX_HYBRID_BANDS) + (6 * MAX_TIME_SLOTS * MAX_HYBRID_BANDS)) *
           sizeof(FLOAT32));
  size += (INPUT_LEN_DOWNSAMPLE * IXHEAACE_MAX_CH_IN_BS_ELE * UPSAMPLE_FAC * sizeof(FLOAT32));
  size += (INPUT_LEN_DOWNSAMPLE * IXHEAACE_MAX_CH_IN_BS_ELE * UPSAMPLE_FAC * sizeof(FLOAT32));
  size = IXHEAAC_GET_SIZE_ALIGNED(size, BYTE_ALIGN_8);
  return size;
}

IA_ERRORCODE ixheaace_mps_515_open(VOID **pstr_handle_mps, WORD32 sample_freq, WORD32 tree_config,
                                   ixheaace_bit_buf_handle pstr_bitstream,
                                   WORD32 *ptr_bits_written,
                                   ixheaace_mps_515_memory_struct *pstr_mps_memory,
                                   WORD32 flag_480) {
  WORD32 i;
  IA_ERRORCODE error = IA_NO_ERROR;
  ixheaace_mps_sac_specific_config *pstr_mps_specific_config;
  ixheaace_mps_sac_pstr_enc pstr_mps_enc = &pstr_mps_memory->spatial_enc_instance;

  ixheaace_mps_sac_pstr_qmf_ana_filter_bank pstr_qmf_ana_bank = &pstr_mps_enc->qmf_fltbank;
  ixheaace_mps_sac_pstr_qmf_synth_filter_bank pstr_qmf_synth_bank =
      &pstr_mps_enc->qmf_synth_fltbank;

  memset(pstr_mps_enc, 0, sizeof(ixheaace_mps_sac_enc));
  pstr_mps_enc->tree_config = tree_config;
  switch (tree_config) {
    case 5151:
    case 5152:
      pstr_mps_enc->output_channels = 1;
      break;
    case 525:
      pstr_mps_enc->output_channels = 2;
  }
  if (flag_480) {
    pstr_mps_enc->time_slots = 15;
  } else {
    pstr_mps_enc->time_slots = 16;
  }

  pstr_mps_enc->frame_size = NUM_QMF_BANDS * pstr_mps_enc->time_slots;

  pstr_qmf_ana_bank->p_filter = ia_mps_enc_qmf_64_640;

  pstr_qmf_synth_bank->p_filter = ia_mps_enc_qmf_64_640;
  pstr_qmf_synth_bank->alt_sin_twiddle = sbr_alt_sin_twiddle;
  pstr_qmf_synth_bank->cos_twiddle = sbr_cos_twiddle;
  pstr_qmf_synth_bank->sin_twiddle = sbr_sin_twiddle;

  for (i = 0; i < 6; i++) {
    memset(&pstr_mps_enc->filterbank[i], 0, sizeof(ixheaace_mps_sac_sbr_encoder_ana_filter_bank));

    memset(&pstr_mps_enc->hyb_state[i], 0, sizeof(ixheaace_mps_hyb_filter_state));
  }

  pstr_mps_enc->bitstream_formatter = &pstr_mps_memory->bsf_memory_instance;
  pstr_mps_specific_config = &pstr_mps_enc->bitstream_formatter->spatial_specific_config;

  memset(pstr_mps_specific_config, 0, sizeof(ixheaace_mps_sac_specific_config));

  pstr_mps_enc->bitstream_formatter->current_frame.bs_independency_flag_count = 0;
  pstr_mps_specific_config->bs_sampling_frequency = sample_freq;

  switch (tree_config) {
    case 5151:
      pstr_mps_specific_config->bs_tree_config = IXHEAACE_MPS_TREE_5151;
      pstr_mps_specific_config->ott_config[4].bs_ott_bands = 2;
      break;

    case 5152:
      pstr_mps_specific_config->bs_tree_config = IXHEAACE_MPS_TREE_5152;
      pstr_mps_specific_config->ott_config[2].bs_ott_bands = 2;
      break;

    case 525:
      pstr_mps_specific_config->bs_tree_config = IXHEAACE_MPS_TREE_525;
      pstr_mps_specific_config->ott_config[0].bs_ott_bands = 2;
      pstr_mps_specific_config->ttt_config->bs_ttt_bands_low = PARAMETER_BANDS;
      pstr_mps_specific_config->ttt_config->bs_ttt_mode_low = 5;
  }

  pstr_mps_specific_config->bs_frame_length = pstr_mps_enc->time_slots - 1;
  pstr_mps_specific_config->bs_freq_res = 2;

  pstr_mps_specific_config->bs_fixed_gain_sur = 2;
  pstr_mps_specific_config->bs_fixed_gain_lfe = 1;
  pstr_mps_specific_config->bs_fixed_gain_dmx = 0;

  error = ixheaace_mps_515_write_spatial_specific_config(pstr_bitstream,
                                                         pstr_mps_enc->bitstream_formatter);

  *ptr_bits_written = ia_enhaacplus_enc_get_bits_available(pstr_bitstream);
  *pstr_handle_mps = pstr_mps_enc;
  return error;
}

IA_ERRORCODE ixheaace_mps_515_apply(ixheaace_mps_sac_enc *pstr_mps_enc, FLOAT32 *ptr_audio_input,
                                    FLOAT32 *ptr_audio_output,
                                    ixheaace_bit_buf_handle pstr_bitstream, VOID *ptr_scratch) {
  IA_ERRORCODE error = IA_NO_ERROR;
  FLOAT32 *pstr_scratch = (FLOAT32 *)ptr_scratch;

  FLOAT32 *in = (FLOAT32 *)pstr_scratch;
  memset(in, 0, sizeof(*in) * MAX_INPUT_CHANNELS * MAX_BUFFER_SIZE);
  pstr_scratch += MAX_INPUT_CHANNELS * MAX_BUFFER_SIZE;

  FLOAT32 *out = (FLOAT32 *)pstr_scratch;
  memset(out, 0, sizeof(*out) * 2 * MAX_BUFFER_SIZE);
  pstr_scratch += 2 * MAX_BUFFER_SIZE;

  FLOAT32 *out1 = (FLOAT32 *)pstr_scratch;
  memset(out1, 0, sizeof(*out1) * 2 * MAX_BUFFER_SIZE);
  pstr_scratch += 2 * MAX_BUFFER_SIZE;

  FLOAT32 *m_qmf_real = (FLOAT32 *)pstr_scratch;
  memset(m_qmf_real, 0, sizeof(*m_qmf_real) * 6 * MAX_TIME_SLOTS * NUM_QMF_BANDS);
  pstr_scratch += 6 * MAX_TIME_SLOTS * NUM_QMF_BANDS;

  FLOAT32 *m_qmf_imag = (FLOAT32 *)pstr_scratch;
  memset(m_qmf_imag, 0, sizeof(*m_qmf_imag) * 6 * MAX_TIME_SLOTS * NUM_QMF_BANDS);
  pstr_scratch += 6 * MAX_TIME_SLOTS * NUM_QMF_BANDS;

  FLOAT32 *m_hybrid_real = (FLOAT32 *)pstr_scratch;
  memset(m_hybrid_real, 0, sizeof(*m_hybrid_real) * 6 * MAX_TIME_SLOTS * MAX_HYBRID_BANDS);
  pstr_scratch += 6 * MAX_TIME_SLOTS * MAX_HYBRID_BANDS;

  FLOAT32 *m_hybrid_imag = (FLOAT32 *)pstr_scratch;
  memset(m_hybrid_imag, 0, sizeof(*m_hybrid_imag) * 6 * MAX_TIME_SLOTS * MAX_HYBRID_BANDS);
  pstr_scratch += 6 * MAX_TIME_SLOTS * MAX_HYBRID_BANDS;

  FLOAT32 *m_qmf_real_out = (FLOAT32 *)pstr_scratch;
  memset(m_qmf_real_out, 0, sizeof(*m_qmf_real_out) * 6 * MAX_TIME_SLOTS * NUM_QMF_BANDS);
  pstr_scratch += 6 * MAX_TIME_SLOTS * NUM_QMF_BANDS;

  FLOAT32 *m_qmf_imag_out = (FLOAT32 *)pstr_scratch;
  memset(m_qmf_imag_out, 0, sizeof(*m_qmf_imag_out) * 6 * MAX_TIME_SLOTS * NUM_QMF_BANDS);

  ixheaace_mps_sac_spatial_frame *pstr_frame_data;
  ixheaace_mps_sac_pstr_qmf_ana_filter_bank pstr_qmf_ana_bank = &pstr_mps_enc->qmf_fltbank;
  ixheaace_mps_sac_pstr_qmf_synth_filter_bank pstr_qmf_synth_bank =
      &pstr_mps_enc->qmf_synth_fltbank;

  WORD32 i, j, k, l;

  FLOAT32 p_pre_scale[6] = {1.f, 1.f, 1.f, 0.3162f, 0.7071f, 0.7071f};
  for (i = 0; i < DELAY_COMPENSATION; i++) {
    for (j = 0; j < 6; j++) {
      pstr_mps_enc->in1[j][i] = pstr_mps_enc->in1[j][i + pstr_mps_enc->frame_size];
    }
  }

  for (i = 0; i < pstr_mps_enc->frame_size; i++) {
    for (j = 0; j < 6; j++) {
      pstr_mps_enc->in1[j][i + DELAY_COMPENSATION] = ptr_audio_input[i * 6 + j] * p_pre_scale[j];
    }
  }

  for (i = 0; i < pstr_mps_enc->frame_size; i++) {
    for (j = 0; j < 6; j++) {
      in[j * MAX_BUFFER_SIZE + i] = ptr_audio_input[i * 6 + j] * p_pre_scale[j];
    }
  }

  for (i = 0; i < pstr_mps_enc->frame_size; i++) {
    if (pstr_mps_enc->output_channels == 1)
      out1[i] = in[i] + in[MAX_BUFFER_SIZE + i] + in[2 * MAX_BUFFER_SIZE + i] +
                in[3 * MAX_BUFFER_SIZE + i] + in[4 * MAX_BUFFER_SIZE + i] +
                in[5 * MAX_BUFFER_SIZE + i];
    else {
      out1[i] = in[i] + 0.7071f * (in[2 * MAX_BUFFER_SIZE + i] + in[3 * MAX_BUFFER_SIZE + i]) +
                in[4 * MAX_BUFFER_SIZE + i];
      out1[MAX_BUFFER_SIZE + i] =
          in[MAX_BUFFER_SIZE + i] +
          0.7071f * (in[2 * MAX_BUFFER_SIZE + i] + in[3 * MAX_BUFFER_SIZE + i]) +
          in[5 * MAX_BUFFER_SIZE + i];
    }
  }

  for (i = 0; i < pstr_mps_enc->frame_size; i++) {
    for (j = 0; j < 6; j++) {
      in[j * MAX_BUFFER_SIZE + i] = pstr_mps_enc->in1[j][i];
    }
  }

  for (l = 0; l < pstr_mps_enc->time_slots; l++) {
    for (j = 0; j < 6; j++) {
      ixheaace_mps_515_calculate_ana_filterbank(
          &pstr_mps_enc->filterbank[j], in + (j * MAX_BUFFER_SIZE) + (l * 64),
          m_qmf_real + (j * MAX_TIME_SLOTS * NUM_QMF_BANDS) + (l * NUM_QMF_BANDS),
          m_qmf_imag + (j * MAX_TIME_SLOTS * NUM_QMF_BANDS) + (l * NUM_QMF_BANDS),
          pstr_qmf_ana_bank);
    }
  }

  for (k = 0; k < 6; k++) {
    ixheaace_mps_515_apply_ana_hyb_filterbank(
        &pstr_mps_enc->hyb_state[k], m_qmf_real + (k * MAX_TIME_SLOTS * NUM_QMF_BANDS),
        m_qmf_imag + (k * MAX_TIME_SLOTS * NUM_QMF_BANDS), pstr_mps_enc->time_slots,
        m_hybrid_real + (k * MAX_TIME_SLOTS * MAX_HYBRID_BANDS),
        m_hybrid_imag + k * MAX_TIME_SLOTS * MAX_HYBRID_BANDS);
  }

  pstr_frame_data = &pstr_mps_enc->bitstream_formatter->current_frame;
  switch (pstr_mps_enc->tree_config) {
    case 5151:
      ixheaace_mps_515_ott_box(pstr_mps_enc->time_slots, m_hybrid_real, m_hybrid_imag,
                               m_hybrid_real + MAX_TIME_SLOTS * MAX_HYBRID_BANDS,
                               m_hybrid_imag + MAX_TIME_SLOTS * MAX_HYBRID_BANDS,
                               pstr_frame_data->ott_data.cld[3][0],
                               pstr_frame_data->ott_data.icc[3][0]);
      ixheaace_mps_515_ott_box(
          pstr_mps_enc->time_slots, m_hybrid_real + 4 * MAX_TIME_SLOTS * MAX_HYBRID_BANDS,
          m_hybrid_imag + 4 * MAX_TIME_SLOTS * MAX_HYBRID_BANDS,
          m_hybrid_real + 5 * MAX_TIME_SLOTS * MAX_HYBRID_BANDS,
          m_hybrid_imag + 5 * MAX_TIME_SLOTS * MAX_HYBRID_BANDS,
          pstr_frame_data->ott_data.cld[2][0], pstr_frame_data->ott_data.icc[2][0]);
      ixheaace_mps_515_ott_box(
          pstr_mps_enc->time_slots, m_hybrid_real + 2 * MAX_TIME_SLOTS * MAX_HYBRID_BANDS,
          m_hybrid_imag + 2 * MAX_TIME_SLOTS * MAX_HYBRID_BANDS,
          m_hybrid_real + 3 * MAX_TIME_SLOTS * MAX_HYBRID_BANDS,
          m_hybrid_imag + 3 * MAX_TIME_SLOTS * MAX_HYBRID_BANDS,
          pstr_frame_data->ott_data.cld[4][0], pstr_frame_data->ott_data.icc[4][0]);
      ixheaace_mps_515_ott_box(pstr_mps_enc->time_slots, m_hybrid_real, m_hybrid_imag,
                               m_hybrid_real + 2 * MAX_TIME_SLOTS * MAX_HYBRID_BANDS,
                               m_hybrid_imag + 2 * MAX_TIME_SLOTS * MAX_HYBRID_BANDS,
                               pstr_frame_data->ott_data.cld[1][0],
                               pstr_frame_data->ott_data.icc[1][0]);
      ixheaace_mps_515_ott_box(pstr_mps_enc->time_slots, m_hybrid_real, m_hybrid_imag,
                               m_hybrid_real + 4 * MAX_TIME_SLOTS * MAX_HYBRID_BANDS,
                               m_hybrid_imag + 4 * MAX_TIME_SLOTS * MAX_HYBRID_BANDS,
                               pstr_frame_data->ott_data.cld[0][0],
                               pstr_frame_data->ott_data.icc[0][0]);

      break;
    case 5152:
      ixheaace_mps_515_ott_box(pstr_mps_enc->time_slots, m_hybrid_real, m_hybrid_imag,
                               m_hybrid_real + 4 * MAX_TIME_SLOTS * MAX_HYBRID_BANDS,
                               m_hybrid_imag + 4 * MAX_TIME_SLOTS * MAX_HYBRID_BANDS,
                               pstr_frame_data->ott_data.cld[3][0],
                               pstr_frame_data->ott_data.icc[3][0]);
      ixheaace_mps_515_ott_box(
          pstr_mps_enc->time_slots, m_hybrid_real + MAX_TIME_SLOTS * MAX_HYBRID_BANDS,
          m_hybrid_imag + MAX_TIME_SLOTS * MAX_HYBRID_BANDS,
          m_hybrid_real + 5 * MAX_TIME_SLOTS * MAX_HYBRID_BANDS,
          m_hybrid_imag + 5 * MAX_TIME_SLOTS * MAX_HYBRID_BANDS,
          pstr_frame_data->ott_data.cld[4][0], pstr_frame_data->ott_data.icc[4][0]);
      ixheaace_mps_515_ott_box(
          pstr_mps_enc->time_slots, m_hybrid_real + 2 * MAX_TIME_SLOTS * MAX_HYBRID_BANDS,
          m_hybrid_imag + 2 * MAX_TIME_SLOTS * MAX_HYBRID_BANDS,
          m_hybrid_real + 3 * MAX_TIME_SLOTS * MAX_HYBRID_BANDS,
          m_hybrid_imag + 3 * MAX_TIME_SLOTS * MAX_HYBRID_BANDS,
          pstr_frame_data->ott_data.cld[2][0], pstr_frame_data->ott_data.icc[2][0]);
      ixheaace_mps_515_ott_box(pstr_mps_enc->time_slots, m_hybrid_real, m_hybrid_imag,
                               m_hybrid_real + MAX_TIME_SLOTS * MAX_HYBRID_BANDS,
                               m_hybrid_imag + MAX_TIME_SLOTS * MAX_HYBRID_BANDS,
                               pstr_frame_data->ott_data.cld[1][0],
                               pstr_frame_data->ott_data.icc[1][0]);
      ixheaace_mps_515_ott_box(pstr_mps_enc->time_slots, m_hybrid_real, m_hybrid_imag,
                               m_hybrid_real + 2 * MAX_TIME_SLOTS * MAX_HYBRID_BANDS,
                               m_hybrid_imag + 2 * MAX_TIME_SLOTS * MAX_HYBRID_BANDS,
                               pstr_frame_data->ott_data.cld[0][0],
                               pstr_frame_data->ott_data.icc[0][0]);
      break;
    case 525:
      ixheaace_mps_515_ott_box(pstr_mps_enc->time_slots, m_hybrid_real, m_hybrid_imag,
                               m_hybrid_real + 4 * MAX_TIME_SLOTS * MAX_HYBRID_BANDS,
                               m_hybrid_imag + 4 * MAX_TIME_SLOTS * MAX_HYBRID_BANDS,
                               pstr_frame_data->ott_data.cld[1][0],
                               pstr_frame_data->ott_data.icc[1][0]);
      ixheaace_mps_515_ott_box(
          pstr_mps_enc->time_slots, m_hybrid_real + MAX_TIME_SLOTS * MAX_HYBRID_BANDS,
          m_hybrid_imag + MAX_TIME_SLOTS * MAX_HYBRID_BANDS,
          m_hybrid_real + 5 * MAX_TIME_SLOTS * MAX_HYBRID_BANDS,
          m_hybrid_imag + 5 * MAX_TIME_SLOTS * MAX_HYBRID_BANDS,
          pstr_frame_data->ott_data.cld[2][0], pstr_frame_data->ott_data.icc[2][0]);
      ixheaace_mps_515_ott_box(
          pstr_mps_enc->time_slots, m_hybrid_real + 2 * MAX_TIME_SLOTS * MAX_HYBRID_BANDS,
          m_hybrid_imag + 2 * MAX_TIME_SLOTS * MAX_HYBRID_BANDS,
          m_hybrid_real + 3 * MAX_TIME_SLOTS * MAX_HYBRID_BANDS,
          m_hybrid_imag + 3 * MAX_TIME_SLOTS * MAX_HYBRID_BANDS,
          pstr_frame_data->ott_data.cld[0][0], pstr_frame_data->ott_data.icc[0][0]);
      ixheaace_mps_515_ttt_box(pstr_mps_enc->time_slots, m_hybrid_real, m_hybrid_imag,
                               m_hybrid_real + MAX_TIME_SLOTS * MAX_HYBRID_BANDS,
                               m_hybrid_imag + MAX_TIME_SLOTS * MAX_HYBRID_BANDS,
                               m_hybrid_real + 2 * MAX_TIME_SLOTS * MAX_HYBRID_BANDS,
                               m_hybrid_imag + 2 * MAX_TIME_SLOTS * MAX_HYBRID_BANDS,
                               pstr_frame_data->ttt_data.cpc_cld1[0][0],
                               pstr_frame_data->ttt_data.cpc_cld2[0][0]);
  }

  pstr_frame_data->framing_info.bs_framing_type = 1;
  pstr_frame_data->framing_info.bs_num_param_sets = 1;

  pstr_frame_data->framing_info.bs_param_slots[0] = 31;
  if (pstr_frame_data->bs_independency_flag_count == 0) {
    pstr_frame_data->bs_independency_flag = 1;
  }
  pstr_frame_data->bs_independency_flag_count =
      (pstr_frame_data->bs_independency_flag_count + 1) % 10;

  error = ixheaace_mps_515_write_spatial_frame(pstr_bitstream, pstr_mps_enc->bitstream_formatter);
  if (error) {
    return error;
  }

  if (pstr_mps_enc->output_channels == 1) {
    ixheaace_mps_515_apply_syn_hyb_filterbank(
        m_hybrid_real, m_hybrid_imag, pstr_mps_enc->time_slots, m_qmf_real_out, m_qmf_imag_out);

    ixheaace_mps_515_calculate_sbr_syn_filterbank(m_qmf_real_out, m_qmf_imag_out, out, 0,
                                                  pstr_qmf_synth_bank, pstr_mps_enc->time_slots,
                                                  pstr_mps_enc->sbr_qmf_states_synthesis);

    for (j = 0; j < pstr_mps_enc->frame_size; j++) {
      ptr_audio_output[j] = out1[j];
    }
  } else {
    ixheaace_mps_515_apply_syn_hyb_filterbank(
        m_hybrid_real, m_hybrid_imag, pstr_mps_enc->time_slots, m_qmf_real_out, m_qmf_imag_out);
    ixheaace_mps_515_apply_syn_hyb_filterbank(m_hybrid_real + MAX_TIME_SLOTS * MAX_HYBRID_BANDS,
                                              m_hybrid_imag + MAX_TIME_SLOTS * MAX_HYBRID_BANDS,
                                              pstr_mps_enc->time_slots,
                                              m_qmf_real_out + MAX_TIME_SLOTS * MAX_HYBRID_BANDS,
                                              m_qmf_imag_out + MAX_TIME_SLOTS * MAX_HYBRID_BANDS);

    ixheaace_mps_515_calculate_sbr_syn_filterbank(m_qmf_real_out, m_qmf_imag_out, out, 0,
                                                  pstr_qmf_synth_bank, pstr_mps_enc->time_slots,
                                                  pstr_mps_enc->sbr_qmf_states_synthesis);
    ixheaace_mps_515_calculate_sbr_syn_filterbank(
        m_qmf_real_out + MAX_TIME_SLOTS * MAX_HYBRID_BANDS,
        m_qmf_imag_out + MAX_TIME_SLOTS * MAX_HYBRID_BANDS, out + MAX_BUFFER_SIZE, 1,
        pstr_qmf_synth_bank, pstr_mps_enc->time_slots, pstr_mps_enc->sbr_qmf_states_synthesis);

    for (j = 0; j < pstr_mps_enc->frame_size; j++) {
      ptr_audio_output[2 * j] = out1[j];
      ptr_audio_output[2 * j + 1] = out1[MAX_BUFFER_SIZE + j];
    }
  }
  return IA_NO_ERROR;
}

VOID ixheaace_mps_515_close(ixheaace_mps_sac_enc *pstr_mps_enc) {
  if (pstr_mps_enc != NULL) {
    pstr_mps_enc->bitstream_formatter = NULL;
  }
}
