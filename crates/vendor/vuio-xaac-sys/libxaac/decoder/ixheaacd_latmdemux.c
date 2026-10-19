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
#include "ixheaacd_sbr_common.h"
#include "ixheaac_type_def.h"
#include "ixheaac_constants.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops16.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaac_basic_ops.h"
#include "ixheaacd_bitbuffer.h"

#include "ixheaac_basic_op.h"
#include "ixheaacd_intrinsics.h"

#include "ixheaacd_defines.h"

#include "ixheaacd_aac_rom.h"

#include "ixheaacd_definitions.h"

#include "ixheaacd_error_codes.h"

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

#include "ixheaacd_sbr_payload.h"
#include "ixheaacd_common_rom.h"
#include "ixheaacd_sbrdecsettings.h"
#include "ixheaacd_sbr_scale.h"
#include "ixheaacd_env_extr_part.h"
#include "ixheaacd_sbr_rom.h"
#include "ixheaacd_stereo.h"
#include "ixheaacd_lpp_tran.h"
#include "ixheaacd_hybrid.h"
#include "ixheaacd_ps_dec.h"

#include "ixheaacd_env_extr.h"
#include "ixheaacd_adts.h"
#include "ixheaacd_audioobjtypes.h"
#include "ixheaacd_memory_standards.h"

#include "ixheaacd_latmdemux.h"

#include "ixheaacd_aacdec.h"
#include "ixheaacd_config.h"
#include "ixheaacd_qmf_dec.h"
#include "ixheaacd_mps_polyphase.h"
#include "ixheaacd_mps_macro_def.h"
#include "ixheaacd_mps_struct_def.h"
#include "ixheaacd_mps_res_rom.h"
#include "ixheaacd_mps_aac_struct.h"
#include "ixheaacd_mps_dec.h"
#include "ixheaacd_struct_def.h"

#include "ixheaacd_multichannel.h"
#include "ixheaacd_headerdecode.h"
#include "ixheaac_error_standards.h"

WORD32 ixheaacd_latm_au_chunk_length_info(
    struct ia_bit_buf_struct *it_bit_buff) {
  UWORD8 reading_done;
  WORD32 len = 0;

  do {
    UWORD32 tmp = ixheaacd_read_bits_buf(it_bit_buff, 8);
    reading_done = (tmp < 255);

    len += tmp;

  } while (reading_done == 0);

  len <<= 3;

  return len;
}

WORD32 ixheaacd_latm_payload_length_info(struct ia_bit_buf_struct *it_bit_buff,
                                         ixheaacd_latm_struct *latm_element) {
  WORD32 error_code = AAC_DEC_OK;
  UWORD32 prog, lay;

  if (latm_element->all_streams_same_time_framing == 1) {
    for (prog = 0; prog < latm_element->num_program; prog++) {
      for (lay = 0; lay < latm_element->num_layer; lay++) {
        ixheaacd_latm_layer_info *layer_info =
            &latm_element->layer_info[prog][lay];

        switch (layer_info->frame_len_type) {
          case 0:
            layer_info->frame_len_bits =
                ixheaacd_latm_au_chunk_length_info(it_bit_buff);
            if (layer_info->frame_len_bits % 8 != 0) {
              error_code = IA_XHEAAC_DEC_EXE_FATAL_INVALID_LOAS_HEADER;
              return error_code;
            }

            latm_element->frame_length = layer_info->frame_len_bits >> 3;
            latm_element->frame_length +=
                (it_bit_buff->size - it_bit_buff->cnt_bits) >> 3;
            break;

          default:
            error_code = IA_XHEAAC_DEC_EXE_FATAL_INVALID_LOAS_HEADER;
            return error_code;
        }
      }
    }
  } else {
    error_code = IA_XHEAAC_DEC_EXE_FATAL_INVALID_LOAS_HEADER;
    return error_code;
  }

  return (error_code);
}

static UWORD32 ixheaacd_latm_get_value(ia_bit_buf_struct *it_bit_buff) {
  UWORD32 bytes_read;

  bytes_read = ixheaacd_read_bits_buf(it_bit_buff, 2) + 1;

  if (bytes_read <= 3)
    return ixheaacd_read_bits_buf(it_bit_buff, 8 * bytes_read);
  else
    return ixheaac_add32_sat(ixheaac_shl32_sat(ixheaacd_read_bits_buf(it_bit_buff, 24), 8),
                              ixheaacd_read_bits_buf(it_bit_buff, 8));
}

IA_ERRORCODE ixheaacd_latm_stream_mux_config(
    struct ia_bit_buf_struct *it_bit_buff, ixheaacd_latm_struct *latm_element,
    ia_aac_dec_state_struct *aac_state_struct,
    ia_sampling_rate_info_struct *sample_rate_info) {
  UWORD32 prog;
  UWORD32 lay;
  WORD32 bytes_consumed;
  WORD32 audio_mux_version_a;
  IA_ERRORCODE error_code = AAC_DEC_OK;
  ixheaacd_latm_layer_info *layer_info = 0;

  latm_element->audio_mux_version = ixheaacd_read_bits_buf(it_bit_buff, 1);

  if (latm_element->audio_mux_version == 1)
    audio_mux_version_a = ixheaacd_read_bits_buf(it_bit_buff, 1);
  else
    audio_mux_version_a = 0;

  if (audio_mux_version_a == 0) {
    if (latm_element->audio_mux_version == 1) {
      ixheaacd_latm_get_value(it_bit_buff);/*tara_buf_fullness*/
    }
    latm_element->all_streams_same_time_framing =
        ixheaacd_read_bits_buf(it_bit_buff, 1);

    latm_element->num_sub_frames = ixheaacd_read_bits_buf(it_bit_buff, 6) + 1;

    if (latm_element->num_sub_frames != 1)
      return IA_XHEAAC_DEC_EXE_FATAL_INVALID_LOAS_HEADER;

    latm_element->num_program = ixheaacd_read_bits_buf(it_bit_buff, 4) + 1;

    if (latm_element->num_program > LATM_MAX_PROG) return IA_FATAL_ERROR;

    for (prog = 0; prog < latm_element->num_program; prog++) {
      latm_element->num_layer = ixheaacd_read_bits_buf(it_bit_buff, 3) + 1;

      for (lay = 0; lay < latm_element->num_layer; lay++) {
        layer_info = &latm_element->layer_info[prog][lay];
        layer_info->frame_len_bits = 0;

        if ((prog == 0) && (lay == 0)) {
          WORD32 asc_len, pos;

          latm_element->use_same_config = 0;

          asc_len = (latm_element->audio_mux_version == 1)
                        ? ixheaacd_latm_get_value(it_bit_buff)
                        : 0;
          pos = it_bit_buff->size - it_bit_buff->cnt_bits;

          if (asc_len > it_bit_buff->size - 106 || asc_len > 2592 ||
              asc_len < 0) {
            return IA_XHEAAC_DEC_INIT_FATAL_DEC_INIT_FAIL;
          }

          if ((error_code = ixheaacd_ga_hdr_dec(
                   aac_state_struct, it_bit_buff->cnt_bits, &bytes_consumed,
                   sample_rate_info, it_bit_buff)))
            return (error_code);

          if (asc_len) {
            asc_len -= (it_bit_buff->size - it_bit_buff->cnt_bits) - pos;
            ixheaacd_read_bidirection(it_bit_buff, asc_len);
          }

          layer_info->asc.aot = aac_state_struct->audio_object_type;
          layer_info->asc.channel_config = aac_state_struct->ch_config;
          layer_info->asc.samples_per_frame = aac_state_struct->frame_len_flag;
          layer_info->asc.sampling_freq = aac_state_struct->sampling_rate;
          layer_info->asc.samples_per_frame = aac_state_struct->frame_length;
        } else {
          latm_element->use_same_config =
              ixheaacd_read_bits_buf(it_bit_buff, 1);

          if (latm_element->use_same_config && (lay > 0)) {
            layer_info->asc = latm_element->layer_info[prog][lay - 1].asc;
          } else {
            if ((error_code = ixheaacd_ga_hdr_dec(
                     aac_state_struct, it_bit_buff->cnt_bits, &bytes_consumed,
                     sample_rate_info, it_bit_buff)))
              return (error_code);
          }
        }

        layer_info->frame_len_type = ixheaacd_read_bits_buf(it_bit_buff, 3);

        switch (layer_info->frame_len_type) {
          case 0:
            layer_info->buffer_fullness =
                ixheaacd_read_bits_buf(it_bit_buff, 8);

            if (!latm_element->all_streams_same_time_framing) {
              if ((lay > 0) && layer_info->asc.aot == AOT_AAC_SCAL) {
              }
            }
            break;

          case 1:
            latm_element->frame_length = ixheaacd_read_bits_buf(it_bit_buff, 9);
          default:
            return IA_XHEAAC_DEC_EXE_FATAL_INVALID_LOAS_HEADER;
        }
      }
    }

    latm_element->other_data_present = ixheaacd_read_bits_buf(it_bit_buff, 1);

    if (latm_element->other_data_present) {
      if (latm_element->audio_mux_version == 1) {
        latm_element->other_data_length = ixheaacd_latm_get_value(it_bit_buff);
      } else {
        UWORD32 other_data_len;
        latm_element->other_data_length = 0;
        do {
          other_data_len = ixheaacd_read_bits_buf(it_bit_buff, 1);
          latm_element->other_data_length <<= 8;
          latm_element->other_data_length +=
              ixheaacd_read_bits_buf(it_bit_buff, 8);
          if (latm_element->other_data_length > (UWORD32)it_bit_buff->cnt_bits)
            return IA_FATAL_ERROR;
        } while (other_data_len);
      }
    }

    latm_element->crc_check_present = ixheaacd_read_bits_buf(it_bit_buff, 1);

    if (latm_element->crc_check_present) {
      latm_element->crc_check_sum = ixheaacd_read_bits_buf(it_bit_buff, 8);
    }
  } else {
    error_code = IA_XHEAAC_DEC_EXE_FATAL_INVALID_LOAS_HEADER;
  }
  return (error_code);
}

IA_ERRORCODE ixheaacd_latm_audio_mux_element(
    struct ia_bit_buf_struct *it_bit_buff, ixheaacd_latm_struct *latm_element,
    ia_aac_dec_state_struct *aac_state_struct,
    ia_sampling_rate_info_struct *sample_rate_info) {
  UWORD32 i;
  IA_ERRORCODE error_code = AAC_DEC_OK;

  ixheaacd_read_bits_buf(it_bit_buff, 13);

  latm_element->use_same_stream_mux = ixheaacd_read_bits_buf(it_bit_buff, 1);

  if (!latm_element->use_same_stream_mux) {
    if ((error_code = ixheaacd_latm_stream_mux_config(
             it_bit_buff, latm_element, aac_state_struct, sample_rate_info))) {
      return (error_code);
    }
  }

  for (i = 0; i < latm_element->num_sub_frames; i++) {
    if ((error_code =
             ixheaacd_latm_payload_length_info(it_bit_buff, latm_element))) {
      if (error_code != 0) return (error_code);
    }
  }

  return (error_code);
}
