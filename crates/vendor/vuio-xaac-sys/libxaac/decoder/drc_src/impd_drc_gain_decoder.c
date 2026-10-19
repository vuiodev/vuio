/******************************************************************************
 *
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
#include <stdio.h>
#include <math.h>
#include <string.h>

#include "impd_type_def.h"
#include "impd_error_standards.h"
#include "impd_drc_extr_delta_coded_info.h"
#include "ixheaac_constants.h"
#include "impd_drc_common.h"
#include "impd_drc_struct.h"
#include "impd_parametric_drc_dec.h"
#include "impd_drc_gain_dec.h"
#include "impd_drc_filter_bank.h"
#include "impd_drc_multi_band.h"
#include "impd_drc_interface.h"
#include "impd_drc_gain_dec.h"
#include "impd_drc_eq.h"
#include "impd_drc_process_audio.h"
#include "impd_drc_gain_decoder.h"
#include "impd_drc_dec.h"

IA_ERRORCODE impd_init_drc_decode(
    WORD32 frame_size, WORD32 sample_rate, WORD32 gain_delay_samples,
    WORD32 delay_mode, WORD32 sub_band_domain_mode,
    ia_drc_gain_dec_struct* p_drc_gain_dec_structs) {
  IA_ERRORCODE err_code = IA_NO_ERROR;

  err_code = impd_init_drc_params(
      frame_size, sample_rate, gain_delay_samples, delay_mode,
      sub_band_domain_mode, &p_drc_gain_dec_structs->ia_drc_params_struct);

  if (err_code != IA_NO_ERROR) return (err_code);

  err_code = impd_init_parametric_drc(
      p_drc_gain_dec_structs->ia_drc_params_struct.drc_frame_size, sample_rate,
      sub_band_domain_mode, &p_drc_gain_dec_structs->parametricdrc_params);

  if (err_code != IA_NO_ERROR) return (err_code);

  return err_code;
}

IA_ERRORCODE impd_init_drc_decode_post_config(
    WORD32 audio_num_chan, WORD32* drc_set_id_processed,
    WORD32* downmix_id_processed, WORD32 num_sets_processed,
    WORD32 eq_set_id_processed,

    ia_drc_gain_dec_struct* p_drc_gain_dec_structs,
    ia_drc_config* pstr_drc_config,
    ia_drc_loudness_info_set_struct* pstr_loudness_info, pVOID* mem_ptr) {
  IA_ERRORCODE err_code = 0;
  WORD32 i, j, k, maxMultibandAudioSignalCount = 0;
  ia_drc_params_struct* p_drc_params_struct =
      &p_drc_gain_dec_structs->ia_drc_params_struct;
  ia_audio_in_out_buf* p_audio_in_out_buf =
      &p_drc_gain_dec_structs->audio_in_out_buf;

  for (i = 0; i < num_sets_processed; i++) {
    err_code = impd_init_selected_drc_set(
        pstr_drc_config, p_drc_params_struct,
        &p_drc_gain_dec_structs->parametricdrc_params, audio_num_chan,
        drc_set_id_processed[i], downmix_id_processed[i],
        &p_drc_gain_dec_structs->ia_filter_banks_struct,
        &p_drc_gain_dec_structs->str_overlap_params,
        p_drc_gain_dec_structs->shape_filter_block);
    if (err_code) return (err_code);
  }

  p_drc_gain_dec_structs->audio_num_chan = audio_num_chan;
  p_drc_gain_dec_structs->ia_drc_params_struct.audio_delay_samples =
      p_drc_gain_dec_structs->ia_drc_params_struct.parametric_drc_delay;
  if (pstr_drc_config->str_drc_config_ext.parametric_drc_present) {
    err_code = impd_init_parametric_drc_after_config(
        pstr_drc_config, pstr_loudness_info,
        &p_drc_gain_dec_structs->parametricdrc_params, mem_ptr);
    if (err_code) return (err_code);
  }

  p_audio_in_out_buf->audio_num_chan = audio_num_chan;
  p_audio_in_out_buf->audio_delay_samples =
      p_drc_params_struct->audio_delay_samples;
  p_audio_in_out_buf->frame_size = p_drc_params_struct->drc_frame_size;

  if (p_drc_params_struct->sub_band_domain_mode == SUBBAND_DOMAIN_MODE_QMF64) {
    p_audio_in_out_buf->audio_delay_sub_band_samples =
        p_drc_params_struct->audio_delay_samples /
        AUDIO_CODEC_SUBBAND_DOWNSAMPLING_FACTOR_QMF64;
    p_audio_in_out_buf->audio_sub_band_frame_size =
        p_drc_params_struct->drc_frame_size /
        AUDIO_CODEC_SUBBAND_DOWNSAMPLING_FACTOR_QMF64;
    p_audio_in_out_buf->audio_sub_band_count = AUDIO_CODEC_SUBBAND_COUNT_QMF64;
  } else if (p_drc_params_struct->sub_band_domain_mode ==
             SUBBAND_DOMAIN_MODE_QMF71) {
    p_audio_in_out_buf->audio_delay_sub_band_samples =
        p_drc_params_struct->audio_delay_samples /
        AUDIO_CODEC_SUBBAND_DOWNSAMPLING_FACTOR_QMF71;
    p_audio_in_out_buf->audio_sub_band_frame_size =
        p_drc_params_struct->drc_frame_size /
        AUDIO_CODEC_SUBBAND_DOWNSAMPLING_FACTOR_QMF71;
    p_audio_in_out_buf->audio_sub_band_count = AUDIO_CODEC_SUBBAND_COUNT_QMF71;
  } else if (p_drc_params_struct->sub_band_domain_mode ==
             SUBBAND_DOMAIN_MODE_STFT256) {
    p_audio_in_out_buf->audio_delay_sub_band_samples =
        p_drc_params_struct->audio_delay_samples /
        AUDIO_CODEC_SUBBAND_DOWNSAMPLING_FACTOR_STFT256;
    p_audio_in_out_buf->audio_sub_band_frame_size =
        p_drc_params_struct->drc_frame_size /
        AUDIO_CODEC_SUBBAND_DOWNSAMPLING_FACTOR_STFT256;
    p_audio_in_out_buf->audio_sub_band_count =
        AUDIO_CODEC_SUBBAND_COUNT_STFT256;
  } else {
    p_audio_in_out_buf->audio_delay_sub_band_samples = 0;
    p_audio_in_out_buf->audio_sub_band_frame_size = 0;
    p_audio_in_out_buf->audio_sub_band_count = 0;
  }

  for (k = 0; k < SEL_DRC_COUNT; k++) {
    if (p_drc_params_struct->sel_drc_array[k].drc_instructions_index >= 0) {
      ia_drc_instructions_struct* drc_instruction_str =
          &(pstr_drc_config->str_drc_instruction_str
                [p_drc_params_struct->sel_drc_array[k].drc_instructions_index]);
      if (drc_instruction_str->gain_element_count > 0) {
        p_drc_gain_dec_structs->drc_gain_buffers.pstr_gain_buf[k]
            .buf_interpolation = (ia_interp_buf_struct*)*mem_ptr;
        *mem_ptr = (pVOID)((SIZE_T)*mem_ptr +
                           IXHEAAC_GET_SIZE_ALIGNED(drc_instruction_str->gain_element_count *
                                                        sizeof(ia_interp_buf_struct),
                                                    BYTE_ALIGN_8));
        p_drc_gain_dec_structs->drc_gain_buffers.pstr_gain_buf[k]
            .buf_interpolation_count = drc_instruction_str->gain_element_count;
        for (i = 0;
             i < p_drc_gain_dec_structs->drc_gain_buffers.pstr_gain_buf[k]
                     .buf_interpolation_count;
             i++) {
          p_drc_gain_dec_structs->drc_gain_buffers.pstr_gain_buf[k]
              .buf_interpolation[i]
              .str_node.time = 0;
          p_drc_gain_dec_structs->drc_gain_buffers.pstr_gain_buf[k]
              .buf_interpolation[i]
              .prev_node.time = -1;
          p_drc_gain_dec_structs->drc_gain_buffers.pstr_gain_buf[k]
              .buf_interpolation[i]
              .str_node.loc_db_gain = 0.0f;
          p_drc_gain_dec_structs->drc_gain_buffers.pstr_gain_buf[k]
              .buf_interpolation[i]
              .str_node.slope = 0.0f;

          for (j = 0; j < 2 * AUDIO_CODEC_FRAME_SIZE_MAX + MAX_SIGNAL_DELAY;
               j++) {
            p_drc_gain_dec_structs->drc_gain_buffers.pstr_gain_buf[k]
                .buf_interpolation[i]
                .lpcm_gains[j] = 1.f;
          }
        }
      }
    }
  }

  if (eq_set_id_processed > 0) {
    for (i = 0; i < pstr_drc_config->str_drc_config_ext.eq_instructions_count;
         i++) {
      if (pstr_drc_config->str_drc_config_ext.str_eq_instructions[i]
              .eq_set_id == eq_set_id_processed)
        break;
    }
    if (i == pstr_drc_config->str_drc_config_ext.eq_instructions_count) {
      return -1;
    }

    p_drc_gain_dec_structs->eq_set = (ia_eq_set_struct*)*mem_ptr;
    *mem_ptr = (pVOID)((SIZE_T)*mem_ptr +
                       IXHEAAC_GET_SIZE_ALIGNED(sizeof(ia_eq_set_struct), BYTE_ALIGN_8));

    if (err_code) return (err_code);

    err_code = impd_derive_eq_set(
        &pstr_drc_config->str_drc_config_ext.str_eq_coeff,
        &(pstr_drc_config->str_drc_config_ext.str_eq_instructions[i]),
        (FLOAT32)p_drc_gain_dec_structs->ia_drc_params_struct.sample_rate,
        p_drc_gain_dec_structs->ia_drc_params_struct.drc_frame_size,
        p_drc_gain_dec_structs->ia_drc_params_struct.sub_band_domain_mode,
        p_drc_gain_dec_structs->eq_set);
    if (err_code) return (err_code);

    impd_get_eq_set_delay(
        p_drc_gain_dec_structs->eq_set,
        &p_drc_gain_dec_structs->ia_drc_params_struct.eq_delay);
  }

  for (i = 0; i < p_drc_params_struct->drc_set_counter; i++) {
    ia_drc_instructions_struct* drc_instruction_str;
    drc_instruction_str =
        &(pstr_drc_config->str_drc_instruction_str
              [p_drc_params_struct->sel_drc_array[i].drc_instructions_index]);
    maxMultibandAudioSignalCount =
        max(maxMultibandAudioSignalCount,
            drc_instruction_str->multiband_audio_sig_count);
  }

  p_drc_gain_dec_structs->audio_band_buffer.non_interleaved_audio = *mem_ptr;
  *mem_ptr =
      (pVOID)((SIZE_T)*mem_ptr +
              IXHEAAC_GET_SIZE_ALIGNED(
                  (maxMultibandAudioSignalCount *
                   sizeof(p_drc_gain_dec_structs->audio_band_buffer.non_interleaved_audio[0])),
                  BYTE_ALIGN_8));

  for (i = 0; i < maxMultibandAudioSignalCount; i++) {
    p_drc_gain_dec_structs->audio_band_buffer.non_interleaved_audio[i] =
        *mem_ptr;
    *mem_ptr =
        (pVOID)((SIZE_T)*mem_ptr +
                IXHEAAC_GET_SIZE_ALIGNED(
                    (p_drc_params_struct->drc_frame_size *
                     sizeof(
                         p_drc_gain_dec_structs->audio_band_buffer.non_interleaved_audio[i][0])),
                    BYTE_ALIGN_8));
  }
  p_drc_gain_dec_structs->audio_band_buffer.multiband_audio_sig_count =
      maxMultibandAudioSignalCount;
  p_drc_gain_dec_structs->audio_band_buffer.frame_size =
      p_drc_params_struct->drc_frame_size;
  ;

  if (p_drc_params_struct->sub_band_domain_mode == SUBBAND_DOMAIN_MODE_OFF &&
      p_audio_in_out_buf->audio_delay_samples) {
    p_audio_in_out_buf->audio_io_buffer_delayed = *mem_ptr;
    *mem_ptr =
        (pVOID)((SIZE_T)*mem_ptr +
                IXHEAAC_GET_SIZE_ALIGNED((p_audio_in_out_buf->audio_num_chan *
                                          sizeof(p_audio_in_out_buf->audio_io_buffer_delayed[0])),
                                         BYTE_ALIGN_8));
    p_audio_in_out_buf->audio_in_out_buf = *mem_ptr;
    *mem_ptr = (pVOID)((SIZE_T)*mem_ptr +
                       IXHEAAC_GET_SIZE_ALIGNED((p_audio_in_out_buf->audio_num_chan *
                                                 sizeof(p_audio_in_out_buf->audio_in_out_buf[0])),
                                                BYTE_ALIGN_8));

    for (i = 0; i < p_audio_in_out_buf->audio_num_chan; i++) {
      p_audio_in_out_buf->audio_io_buffer_delayed[i] = *mem_ptr;
      *mem_ptr = (pVOID)((SIZE_T)*mem_ptr +
                         IXHEAAC_GET_SIZE_ALIGNED(
                             ((p_audio_in_out_buf->frame_size +
                               p_audio_in_out_buf->audio_delay_samples) *
                              sizeof(p_audio_in_out_buf->audio_io_buffer_delayed[i][0])),
                             BYTE_ALIGN_8));
      p_audio_in_out_buf->audio_in_out_buf[i] =
          &p_audio_in_out_buf->audio_io_buffer_delayed
               [i][p_audio_in_out_buf->audio_delay_samples];
    }
  }
  if (p_drc_params_struct->sub_band_domain_mode != SUBBAND_DOMAIN_MODE_OFF &&
      p_audio_in_out_buf->audio_delay_sub_band_samples) {
    p_audio_in_out_buf->audio_buffer_delayed_real = *mem_ptr;
    *mem_ptr =
        (pVOID)((SIZE_T)*mem_ptr + IXHEAAC_GET_SIZE_ALIGNED(
                                       (p_audio_in_out_buf->audio_num_chan *
                                        sizeof(p_audio_in_out_buf->audio_buffer_delayed_real[0])),
                                       BYTE_ALIGN_8));
    p_audio_in_out_buf->audio_buffer_delayed_imag = *mem_ptr;
    *mem_ptr =
        (pVOID)((SIZE_T)*mem_ptr + IXHEAAC_GET_SIZE_ALIGNED(
                                       (p_audio_in_out_buf->audio_num_chan *
                                        sizeof(p_audio_in_out_buf->audio_buffer_delayed_imag[0])),
                                       BYTE_ALIGN_8));
    p_audio_in_out_buf->audio_real_buff = *mem_ptr;
    *mem_ptr = (pVOID)((SIZE_T)*mem_ptr +
                       IXHEAAC_GET_SIZE_ALIGNED((p_audio_in_out_buf->audio_num_chan *
                                                 sizeof(p_audio_in_out_buf->audio_real_buff[0])),
                                                BYTE_ALIGN_8));
    p_audio_in_out_buf->audio_imag_buff = *mem_ptr;
    *mem_ptr = (pVOID)((SIZE_T)*mem_ptr +
                       IXHEAAC_GET_SIZE_ALIGNED((p_audio_in_out_buf->audio_num_chan *
                                                 sizeof(p_audio_in_out_buf->audio_imag_buff[0])),
                                                BYTE_ALIGN_8));

    for (i = 0; i < p_audio_in_out_buf->audio_num_chan; i++) {
      p_audio_in_out_buf->audio_buffer_delayed_real[i] = *mem_ptr;
      *mem_ptr = (pVOID)((SIZE_T)*mem_ptr +
                         IXHEAAC_GET_SIZE_ALIGNED(
                             ((p_audio_in_out_buf->audio_sub_band_frame_size +
                               p_audio_in_out_buf->audio_delay_sub_band_samples) *
                              sizeof(p_audio_in_out_buf->audio_buffer_delayed_real[i][0])),
                             BYTE_ALIGN_8));
      p_audio_in_out_buf->audio_buffer_delayed_imag[i] = *mem_ptr;
      *mem_ptr = (pVOID)((SIZE_T)*mem_ptr +
                         IXHEAAC_GET_SIZE_ALIGNED(
                             ((p_audio_in_out_buf->audio_sub_band_frame_size +
                               p_audio_in_out_buf->audio_delay_sub_band_samples) *
                              sizeof(p_audio_in_out_buf->audio_buffer_delayed_imag[i][0])),
                             BYTE_ALIGN_8));

      p_audio_in_out_buf->audio_real_buff[i] =
          &p_audio_in_out_buf->audio_buffer_delayed_real
               [i][p_audio_in_out_buf->audio_delay_sub_band_samples *
                   p_audio_in_out_buf->audio_sub_band_count];
      p_audio_in_out_buf->audio_imag_buff[i] =
          &p_audio_in_out_buf->audio_buffer_delayed_imag
               [i][p_audio_in_out_buf->audio_delay_sub_band_samples *
                   p_audio_in_out_buf->audio_sub_band_count];
    }
  }

  return err_code;
}

IA_ERRORCODE impd_drc_process_time_domain(
    ia_drc_gain_dec_struct* p_drc_gain_dec_structs,
    ia_drc_config* pstr_drc_config, ia_drc_gain_struct* pstr_drc_gain,
    FLOAT32* audio_in_out_buf[], FLOAT32 loudness_normalization_gain_db,
    FLOAT32 boost, FLOAT32 compress, WORD32 drc_characteristic_target) {
  WORD32 sel_drc_index;
  IA_ERRORCODE err_code = 0;
  WORD32 passThru;
  ia_drc_instructions_struct* str_drc_instruction_str =
      pstr_drc_config->str_drc_instruction_str;

  if (p_drc_gain_dec_structs->eq_set) {
    WORD32 ch;
    FLOAT32* audio_channel;
    for (ch = 0; ch < p_drc_gain_dec_structs->eq_set->audio_num_chan; ch++) {
      audio_channel = audio_in_out_buf[ch];

      err_code = impd_process_eq_set_time_domain(
          p_drc_gain_dec_structs->eq_set, ch, audio_channel, audio_channel,
          p_drc_gain_dec_structs->ia_drc_params_struct.drc_frame_size);
      if (err_code) return (err_code);
    }
  }

  impd_store_audio_io_buffer_time(audio_in_out_buf,
                                  &p_drc_gain_dec_structs->audio_in_out_buf);

  if (pstr_drc_config->apply_drc) {
    for (sel_drc_index = 0;
         sel_drc_index <
         p_drc_gain_dec_structs->ia_drc_params_struct.drc_set_counter;
         sel_drc_index++) {
      err_code = impd_get_drc_gain(
          p_drc_gain_dec_structs, pstr_drc_config, pstr_drc_gain, compress,
          boost, drc_characteristic_target, loudness_normalization_gain_db,
          sel_drc_index, &p_drc_gain_dec_structs->drc_gain_buffers);
      if (err_code != IA_NO_ERROR) return (err_code);
    }

    if (p_drc_gain_dec_structs->ia_drc_params_struct.drc_set_counter == 0) {
      impd_retrieve_audio_io_buffer_time(
          audio_in_out_buf, &p_drc_gain_dec_structs->audio_in_out_buf);

    } else {
      for (sel_drc_index = 0;
           sel_drc_index <
           p_drc_gain_dec_structs->ia_drc_params_struct.drc_set_counter;
           sel_drc_index++) {
        if (p_drc_gain_dec_structs->ia_drc_params_struct
                .multiband_sel_drc_idx == sel_drc_index) {
          passThru = 0;
        } else {
          passThru = 1;
        }
        err_code = impd_filter_banks_process(
            str_drc_instruction_str,
            p_drc_gain_dec_structs->ia_drc_params_struct
                .sel_drc_array[sel_drc_index]
                .drc_instructions_index,
            &p_drc_gain_dec_structs->ia_drc_params_struct,
            p_drc_gain_dec_structs->audio_in_out_buf.audio_io_buffer_delayed,
            &p_drc_gain_dec_structs->audio_band_buffer,
            &p_drc_gain_dec_structs->ia_filter_banks_struct, passThru);
        if (err_code != IA_NO_ERROR) return (err_code);

        impd_apply_gains_and_add(
            str_drc_instruction_str,
            p_drc_gain_dec_structs->ia_drc_params_struct
                .sel_drc_array[sel_drc_index]
                .drc_instructions_index,
            &p_drc_gain_dec_structs->ia_drc_params_struct,
            &(p_drc_gain_dec_structs->drc_gain_buffers
                  .pstr_gain_buf[sel_drc_index]),
            p_drc_gain_dec_structs->shape_filter_block,
            p_drc_gain_dec_structs->audio_band_buffer.non_interleaved_audio,
            audio_in_out_buf, 1);
      }
    }
  }

  impd_advance_audio_io_buffer_time(&p_drc_gain_dec_structs->audio_in_out_buf);

  return err_code;
}

VOID impd_get_parametric_drc_delay(
    ia_drc_gain_dec_struct* p_drc_gain_dec_structs,
    ia_drc_config* pstr_drc_config, WORD32* parametric_drc_delay,
    WORD32* parametric_drc_delay_max) {
  *parametric_drc_delay =
      p_drc_gain_dec_structs->ia_drc_params_struct.parametric_drc_delay;

  if (pstr_drc_config->str_drc_config_ext.parametric_drc_present &&
      pstr_drc_config->str_drc_config_ext.str_drc_coeff_param_drc
          .parametric_drc_delay_max_present) {
    *parametric_drc_delay_max =
        pstr_drc_config->str_drc_config_ext.str_drc_coeff_param_drc
            .parametric_drc_delay_max;
  } else if (pstr_drc_config->str_drc_config_ext.parametric_drc_present == 0) {
    *parametric_drc_delay_max = 0;
  } else {
    *parametric_drc_delay_max = -1;
  }

  return;
}

VOID impd_get_eq_delay(ia_drc_gain_dec_struct* p_drc_gain_dec_structs,
                       ia_drc_config* pstr_drc_config, WORD32* eq_delay,
                       WORD32* eq_delay_max) {
  *eq_delay = p_drc_gain_dec_structs->ia_drc_params_struct.eq_delay;

  if (pstr_drc_config->str_drc_config_ext.eq_flag &&
      pstr_drc_config->str_drc_config_ext.str_eq_coeff.eq_delay_max_present) {
    *eq_delay_max =
        pstr_drc_config->str_drc_config_ext.str_eq_coeff.eq_delay_max;
  } else if (pstr_drc_config->str_drc_config_ext.eq_flag == 0) {
    *eq_delay_max = 0;
  } else {
    *eq_delay_max = -1;
  }

  return;
}
