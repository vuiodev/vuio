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
  WORD32 num_ssc_size_bits;
  UWORD8 *ptr_ssc;

} ixheaace_mps_space_ssc_buf;

typedef struct {
  WORD32 num_sample_rate;
  WORD32 num_samples_frame;
  WORD32 num_total_input_channels;
  WORD32 dmx_delay;
  WORD32 codec_delay;

  WORD32 decoder_delay;
  WORD32 pay_load_delay;
  WORD32 num_discard_out_frames;

  ixheaace_mps_space_ssc_buf *p_ssc_buf;

} ixheaace_mps_space_info;

typedef struct {
  WORD32 num_input_samples;
  UWORD32 input_buffer_size_per_channel;
  UWORD32 is_input_inter_leaved;

} ixheaace_mps_in_args;

typedef struct {
  WORD32 num_output_bits;
  WORD32 num_output_samples;
  UWORD32 num_samples_consumed;

} ixheaace_mps_out_args;

typedef struct ixheaace_mps_space_structure ixheaace_mps_space_structure,
    *ixheaace_mps_pstr_space_structure;
