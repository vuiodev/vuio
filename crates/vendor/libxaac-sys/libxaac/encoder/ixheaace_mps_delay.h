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

#pragma once
typedef struct {
  WORD32 b_dmx_align;
  WORD32 b_minimize_delay;
  WORD32 b_sac_time_alignment_dynamic_out;

  WORD32 num_qmf_len;
  WORD32 num_frame_len;
  WORD32 n_surround_delay;
  WORD32 n_arb_dmx_delay;
  WORD32 n_limiter_delay;
  WORD32 num_core_coder_delay;
  WORD32 num_sac_stream_mux_delay;
  WORD32 n_sac_time_alignment;
} ixheaace_mps_delay_config;

struct ixheaace_mps_delay {
  ixheaace_mps_delay_config delay_config;
  WORD32 num_dmx_align_buffer;
  WORD32 num_surround_analysis_buffer;
  WORD32 num_arb_dmx_analysis_buffer;
  WORD32 num_output_audio_buffer;
  WORD32 num_bitstream_frame_buffer;
  WORD32 num_output_audio_qmf_frame_buffer;
  WORD32 num_discard_out_frames;
  WORD32 num_bitstream_frame_buffer_size;
  WORD32 num_info_dmx_delay;
  WORD32 num_info_codec_delay;
  WORD32 num_info_decoder_delay;
};

typedef struct ixheaace_mps_delay ixheaace_mps_delay, *ixheaace_mps_pstr_delay;

VOID ixheaace_mps_212_delay_sub_calculate_buffer_delays(ixheaace_mps_pstr_delay pstr_delay);
