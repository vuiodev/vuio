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

#include "ixheaac_type_def.h"
#include "ixheaac_constants.h"
#include "ixheaac_error_standards.h"
#include "ixheaace_error_codes.h"

#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaac_basic_ops.h"
#include "ixheaace_mps_common_define.h"
#include "ixheaace_mps_defines.h"
#include "ixheaace_mps_delay.h"
#include "ixheaace_mps_common_fix.h"

VOID ixheaace_mps_212_delay_sub_calculate_buffer_delays(ixheaace_mps_pstr_delay pstr_delay) {
  WORD32 num_encoder_an_delay, num_encoder_win_delay, num_decoder_an_delay, num_decoder_syn_delay,
      num_residual_coder_frame_delay, num_arb_dmx_residual_coder_frame_delay;
  WORD32 temp_delay_1, temp_delay_2, temp_delay_12, temp_delay_3;

  if (pstr_delay->delay_config.b_sac_time_alignment_dynamic_out > 0) {
    pstr_delay->delay_config.n_sac_time_alignment = 0;
  }

  num_encoder_an_delay =
      2 * pstr_delay->delay_config.num_qmf_len + pstr_delay->delay_config.num_qmf_len / 2;
  num_decoder_an_delay =
      2 * pstr_delay->delay_config.num_qmf_len + pstr_delay->delay_config.num_qmf_len / 2;
  num_decoder_syn_delay =
      1 * pstr_delay->delay_config.num_qmf_len + pstr_delay->delay_config.num_qmf_len / 2;
  num_encoder_win_delay = pstr_delay->delay_config.num_frame_len / 2;
  num_residual_coder_frame_delay = 0;
  num_arb_dmx_residual_coder_frame_delay = 0;

  temp_delay_1 =
      pstr_delay->delay_config.n_arb_dmx_delay - pstr_delay->delay_config.n_surround_delay;
  if (temp_delay_1 >= 0) {
    pstr_delay->num_surround_analysis_buffer = temp_delay_1;
    pstr_delay->num_arb_dmx_analysis_buffer = 0;
  } else {
    pstr_delay->num_surround_analysis_buffer = 0;
    pstr_delay->num_arb_dmx_analysis_buffer = -temp_delay_1;
  }

  temp_delay_1 = num_encoder_win_delay + pstr_delay->delay_config.n_surround_delay +
                 pstr_delay->num_surround_analysis_buffer + num_encoder_an_delay;
  temp_delay_2 = num_encoder_win_delay + pstr_delay->delay_config.n_arb_dmx_delay +
                 pstr_delay->num_arb_dmx_analysis_buffer + num_encoder_an_delay;
  temp_delay_3 = pstr_delay->delay_config.n_arb_dmx_delay +
                 pstr_delay->delay_config.n_limiter_delay +
                 pstr_delay->delay_config.num_core_coder_delay +
                 pstr_delay->delay_config.n_sac_time_alignment + num_decoder_an_delay;

  temp_delay_12 = MAX(num_residual_coder_frame_delay, num_arb_dmx_residual_coder_frame_delay) *
                  pstr_delay->delay_config.num_frame_len;
  temp_delay_12 += pstr_delay->delay_config.num_sac_stream_mux_delay;

  if (temp_delay_1 > temp_delay_2) {
    temp_delay_12 += temp_delay_1;
  } else {
    temp_delay_12 += temp_delay_2;
  }

  if (temp_delay_3 > temp_delay_12) {
    if (pstr_delay->delay_config.b_minimize_delay > 0) {
      pstr_delay->num_bitstream_frame_buffer =
          (temp_delay_3 - temp_delay_12) / pstr_delay->delay_config.num_frame_len;
      pstr_delay->num_output_audio_buffer = 0;
      pstr_delay->num_surround_analysis_buffer +=
          (temp_delay_3 - temp_delay_12 -
           (pstr_delay->num_bitstream_frame_buffer * pstr_delay->delay_config.num_frame_len));
      pstr_delay->num_arb_dmx_analysis_buffer +=
          (temp_delay_3 - temp_delay_12 -
           (pstr_delay->num_bitstream_frame_buffer * pstr_delay->delay_config.num_frame_len));
    } else {
      pstr_delay->num_bitstream_frame_buffer =
          ((temp_delay_3 - temp_delay_12) + pstr_delay->delay_config.num_frame_len - 1) /
          pstr_delay->delay_config.num_frame_len;
      pstr_delay->num_output_audio_buffer =
          pstr_delay->num_bitstream_frame_buffer * pstr_delay->delay_config.num_frame_len +
          temp_delay_12 - temp_delay_3;
    }
  } else {
    pstr_delay->num_bitstream_frame_buffer = 0;
    pstr_delay->num_output_audio_buffer = temp_delay_12 - temp_delay_3;
  }

  if (pstr_delay->delay_config.b_dmx_align > 0) {
    WORD32 temp_delay =
        pstr_delay->delay_config.n_arb_dmx_delay + pstr_delay->num_output_audio_buffer +
        pstr_delay->delay_config.n_limiter_delay + pstr_delay->delay_config.num_core_coder_delay;
    pstr_delay->num_discard_out_frames =
        (temp_delay + pstr_delay->delay_config.num_frame_len - 1) /
        pstr_delay->delay_config.num_frame_len;
    pstr_delay->num_dmx_align_buffer =
        pstr_delay->num_discard_out_frames * pstr_delay->delay_config.num_frame_len - temp_delay;
  } else {
    pstr_delay->num_discard_out_frames = 0;
    pstr_delay->num_dmx_align_buffer = 0;
  }

  pstr_delay->num_info_dmx_delay = pstr_delay->delay_config.n_arb_dmx_delay +
                                   pstr_delay->num_output_audio_buffer +
                                   pstr_delay->delay_config.n_limiter_delay;
  pstr_delay->num_info_codec_delay = pstr_delay->num_info_dmx_delay +
                                     pstr_delay->delay_config.num_core_coder_delay +
                                     pstr_delay->delay_config.n_sac_time_alignment +
                                     num_decoder_an_delay + num_decoder_syn_delay;
  pstr_delay->num_info_decoder_delay = num_decoder_an_delay + num_decoder_syn_delay;
  pstr_delay->num_bitstream_frame_buffer_size = pstr_delay->num_bitstream_frame_buffer + 1;
}
