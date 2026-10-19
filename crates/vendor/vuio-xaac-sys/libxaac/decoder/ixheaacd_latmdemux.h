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
#ifndef LATM_DEMUX_H
#define LATM_DEMUX_H

#define LATM_MAX_PROG 8
#define LATM_MAX_LAYER 8
#define LATM_MAX_VAR_CHUNKS 16
#define LATM_MAX_ID 16

typedef struct {
  AUDIO_OBJECT_TYPE aot;
  UWORD32 sampling_freq_index;
  UWORD32 sampling_freq;
  WORD32 channel_config;
  UWORD32 samples_per_frame;
} ixheaacd_latm_specs;

typedef struct {
  UWORD32 frame_len_type;
  UWORD32 buffer_fullness;
  UWORD32 frame_len_bits;
  ixheaacd_latm_specs asc;
} ixheaacd_latm_layer_info;

typedef struct {
  UWORD32 use_same_stream_mux;
  UWORD32 audio_mux_version;
  UWORD32 all_streams_same_time_framing;
  UWORD32 num_sub_frames;
  UWORD32 num_program;
  UWORD32 num_layer;
  UWORD32 use_same_config;

  UWORD32 other_data_present;
  UWORD32 other_data_length;
  UWORD32 crc_check_present;
  UWORD32 crc_check_sum;
  UWORD32 frame_length;
  ixheaacd_latm_layer_info layer_info[LATM_MAX_PROG][LATM_MAX_LAYER];
} ixheaacd_latm_struct;

WORD32 ixheaacd_latm_au_chunk_length_info(ia_bit_buf_struct *it_bit_buff);

WORD32 ixheaacd_latm_payload_length_info(ia_bit_buf_struct *it_bit_buff,
                                         ixheaacd_latm_struct *latm_element);

#endif
