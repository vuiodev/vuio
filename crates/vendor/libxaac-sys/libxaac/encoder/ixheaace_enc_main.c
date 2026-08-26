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
#include <stdlib.h>
#include "ixheaac_type_def.h"
#include "ixheaac_constants.h"
#include "impd_drc_common_enc.h"
#include "impd_drc_uni_drc.h"
#include "impd_drc_tables.h"
#include "impd_drc_api.h"
#include "ixheaace_api.h"
#include "ixheaace_aac_constants.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops16.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaac_basic_ops.h"

#include "ixheaac_error_standards.h"
#include "ixheaace_psy_const.h"
#include "ixheaace_tns.h"
#include "ixheaace_tns_params.h"
#include "ixheaace_rom.h"
#include "ixheaace_common_rom.h"

#include "ixheaace_adjust_threshold_data.h"
#include "ixheaace_bitbuffer.h"
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
#include "ixheaace_error_codes.h"
#include "ixheaace_common_utils.h"

#define ALIGNMENT_DEFINE __attribute__((aligned(8)))

IA_ERRORCODE ia_enhaacplus_enc_aac_core_encode(
    iexheaac_encoder_str **pstr_aac_enc, FLOAT32 *ptr_time_signal, UWORD32 time_sn_stride,
    const UWORD8 *ptr_anc_bytes, UWORD8 *num_anc_bytes, UWORD8 *ptr_out_bytes,
    WORD32 *num_out_bytes, ixheaace_aac_tables *pstr_aac_tabs, VOID *ptr_bit_stream_handle,
    VOID *ptr_bit_stream, FLAG flag_last_element, WORD32 *write_program_config_element,
    WORD32 i_num_coup_channels, WORD32 i_channels_mask, WORD32 ele_idx, WORD32 *total_fill_bits,
    WORD32 total_channels, WORD32 aot, WORD32 adts_flag, WORD32 num_bs_elements,
    WORD32 *is_quant_spec_zero, WORD32 *is_gain_limited) {
  IA_ERRORCODE err_code = IA_NO_ERROR;
  iexheaac_encoder_str *pstr_aac_encoder = pstr_aac_enc[ele_idx];
  ixheaace_element_info *pstr_element_info = &pstr_aac_encoder->element_info;
  WORD32 glob_used_bits;
  WORD32 anc_data_bytes, anc_data_bytes_left;
  WORD32 stat_bits_flag = 0;
  WORD32 frame_len_long = FRAME_LEN_1024;
  WORD32 ch;

  if (aot == AOT_AAC_LC || aot == AOT_SBR || aot == AOT_PS) {
    frame_len_long =
        (pstr_aac_encoder->config.flag_framelength_small == 1) ? FRAME_LEN_960 : FRAME_LEN_1024;
  } else if (aot == AOT_AAC_LD || aot == AOT_AAC_ELD) {
    frame_len_long =
        (pstr_aac_encoder->config.flag_framelength_small == 1) ? FRAME_LEN_480 : FRAME_LEN_512;
  }

  if (ele_idx == 0) {
    if (aot == AOT_AAC_LC || aot == AOT_SBR || aot == AOT_PS) {
      ptr_bit_stream_handle = ia_enhaacplus_enc_create_bitbuffer(
          ptr_bit_stream, (UWORD8 *)ptr_out_bytes,
          (((pstr_aac_encoder->config.flag_framelength_small == 1) ? MAXIMUM_CHANNEL_BITS_960
                                                                   : MAXIMUM_CHANNEL_BITS_1024) /
           8) *
              total_channels);
    } else if (aot == AOT_AAC_LD || aot == AOT_AAC_ELD) {
      if (pstr_aac_encoder->config.bitreservoir_size != -1) {
        WORD32 avg_byte_perframe = (pstr_aac_encoder->config.bit_rate * frame_len_long /
                                    (pstr_aac_encoder->config.core_sample_rate * 8));

        if ((pstr_aac_encoder->config.bitreservoir_size * total_channels) > avg_byte_perframe) {
          ptr_bit_stream_handle = ia_enhaacplus_enc_create_bitbuffer(
              ptr_bit_stream, (UWORD8 *)ptr_out_bytes,
              pstr_aac_encoder->config.bitreservoir_size * total_channels);
        } else {
          ptr_bit_stream_handle = ia_enhaacplus_enc_create_bitbuffer(
              ptr_bit_stream, (UWORD8 *)ptr_out_bytes,
              (pstr_aac_encoder->config.bit_rate * frame_len_long /
               (pstr_aac_encoder->config.core_sample_rate * 8)));
        }
      } else {
        ptr_bit_stream_handle = ia_enhaacplus_enc_create_bitbuffer(
            ptr_bit_stream, (UWORD8 *)ptr_out_bytes,
            (pstr_aac_encoder->config.bit_rate * frame_len_long /
             (pstr_aac_encoder->config.core_sample_rate * 8)));
      }
    }
  } else {
    ptr_bit_stream_handle = ptr_bit_stream;
  }

  if (adts_flag) {
    stat_bits_flag = 0; /* fix for low bit-rate */
    if ((pstr_aac_encoder->config.core_sample_rate * 3) <= (pstr_aac_encoder->config.bit_rate)) {
      stat_bits_flag = 7; /* fix for low bit-rate */
    }
  }

  anc_data_bytes = anc_data_bytes_left = *num_anc_bytes;

  if (pstr_element_info->el_type == ID_CPE) {
    if (!pstr_aac_encoder->config.num_stereo_preprocessing) {
      iaace_apply_stereo_preproc(&pstr_aac_encoder->str_stereo_pre_pro, time_sn_stride,
                                 pstr_element_info, ptr_time_signal, frame_len_long);
    }
  }

  err_code = ia_enhaacplus_enc_psy_main(
      time_sn_stride, pstr_element_info, ptr_time_signal, aot,
      pstr_aac_encoder->psy_kernel.psy_data,
      pstr_aac_encoder->psy_kernel.temporal_noise_shaping_data,
      &pstr_aac_encoder->psy_kernel.psy_conf_long, &pstr_aac_encoder->psy_kernel.psy_conf_short,
      pstr_aac_encoder->psy_out.psy_out_ch,
      &pstr_aac_encoder->psy_out.psy_out_element,
      pstr_aac_encoder->psy_kernel.p_scratch_tns_float,
      (FLOAT32 *)pstr_aac_encoder->pstr_aac_scratch->shared_buffer1,
      pstr_aac_encoder->pstr_aac_scratch->shared_buffer5, pstr_aac_tabs, frame_len_long);

  if (err_code != IA_NO_ERROR) {
    return err_code;
  }

  ia_enhaacplus_enc_adjust_bitrate(
      &pstr_aac_encoder->qc_kernel, pstr_aac_encoder->config.bit_rate,
      pstr_aac_encoder->config.core_sample_rate, flag_last_element, frame_len_long);

  for (ch = 0; ch < pstr_element_info->n_channels_in_el; ch++) {
    pstr_aac_encoder->psy_out.psy_out_ch[ch]->ms_digest =
        pstr_aac_encoder->psy_out.psy_out_element.tools_info.ms_digest;

    memcpy(
        &pstr_aac_encoder->psy_out.psy_out_ch[ch]->ms_used[0],
        &pstr_aac_encoder->psy_out.psy_out_element.tools_info.ms_mask[0],
        MAXIMUM_GROUPED_SCALE_FACTOR_BAND *
            sizeof(pstr_aac_encoder->psy_out.psy_out_ch[ch]->ms_used[0]));
  }

  err_code = ia_enhaacplus_enc_qc_main(
      &pstr_aac_encoder->qc_kernel, pstr_element_info->n_channels_in_el,
      &pstr_aac_encoder->qc_kernel.element_bits,
      pstr_aac_encoder->psy_out.psy_out_ch,
      &pstr_aac_encoder->psy_out.psy_out_element,
      pstr_aac_encoder->qc_out.qc_channel,
      &pstr_aac_encoder->qc_out.qc_element, MIN(anc_data_bytes_left, anc_data_bytes),
      pstr_aac_tabs, adts_flag, aot, stat_bits_flag, flag_last_element, frame_len_long,
      pstr_aac_encoder->pstr_aac_scratch->shared_buffer5, is_quant_spec_zero,
      is_gain_limited);

  if (err_code != IA_NO_ERROR) {
    return err_code;
  }

  if (pstr_element_info->el_type == ID_CPE) {
    if (!pstr_aac_encoder->config.num_stereo_preprocessing) {
      iaace_update_stereo_pre_process(
          pstr_aac_encoder->psy_out.psy_out_ch,
          &pstr_aac_encoder->qc_out.qc_element, &pstr_aac_encoder->str_stereo_pre_pro,
          pstr_aac_encoder->psy_out.psy_out_element.weight_ms_lr_pe_ratio);
    }
  }

  /* Update bit reservoir levels */
  ia_enhaacplus_enc_update_bit_reservoir(&pstr_aac_encoder->qc_kernel, &pstr_aac_encoder->qc_out);

  err_code = ia_enhaacplus_enc_finalize_bit_consumption(
      &pstr_aac_encoder->qc_kernel, &pstr_aac_encoder->qc_out, flag_last_element,
      ((ixheaace_bit_buf_handle)ptr_bit_stream_handle)->cnt_bits, total_fill_bits, pstr_aac_enc,
      num_bs_elements, aot);

  if (err_code != IA_NO_ERROR) {
    return err_code;
  }

  err_code = ia_enhaacplus_enc_write_bitstream(
      ptr_bit_stream_handle, *pstr_element_info, &pstr_aac_encoder->qc_out,
      &pstr_aac_encoder->psy_out, &glob_used_bits, ptr_anc_bytes, pstr_aac_tabs,
      flag_last_element, write_program_config_element, i_num_coup_channels, i_channels_mask,
      pstr_aac_encoder->config.core_sample_rate, ele_idx, aot, total_fill_bits);
  if (err_code != IA_NO_ERROR) {
    return err_code;
  }
  /* write out the bitstream */
  *num_out_bytes = ia_enhaacplus_enc_get_bits_available(ptr_bit_stream_handle) / 8;

  /* Validate that this frame is not too large */
  if (pstr_aac_encoder->config.bitreservoir_size != -1) {
    WORD32 avg_bytes_perframe = (pstr_aac_encoder->config.bit_rate * frame_len_long /
                                 (pstr_aac_encoder->config.core_sample_rate * 8));

    if (pstr_aac_encoder->config.bitreservoir_size * total_channels > avg_bytes_perframe) {
      if (*num_out_bytes > (pstr_aac_encoder->config.bitreservoir_size * total_channels)) {
        err_code = IA_EXHEAACE_EXE_FATAL_INVALID_OUT_BYTES;
      }
    } else {
      if (*num_out_bytes > avg_bytes_perframe) {
        err_code = IA_EXHEAACE_EXE_FATAL_INVALID_OUT_BYTES;
      }
    }
  } else {
    WORD32 avg_bytes_perframe = (pstr_aac_encoder->config.bit_rate * frame_len_long /
                                 (pstr_aac_encoder->config.core_sample_rate * 8));
    if (*num_out_bytes > avg_bytes_perframe) {
      err_code = IA_EXHEAACE_EXE_FATAL_INVALID_OUT_BYTES;
    }
  }

  return err_code;
}
