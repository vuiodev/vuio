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
#ifndef IXHEAACD_STRUCT_H
#define IXHEAACD_STRUCT_H

typedef struct {
  UINT32 audio_object_type;
  UINT32 samp_frequency_index;
  UINT32 sampling_frequency;
  UINT32 channel_configuration;
  UINT32 ext_audio_object_type;
  UINT32 ext_samp_frequency_index;
  UINT32 ext_sampling_frequency;
  UINT32 sbr_present_flag;

  UINT32 frame_length;
  ia_prog_config_struct* str_prog_config;
  ia_usac_config_struct str_usac_config;

  WORD32 num_front_channel;
  WORD32 num_side_channel;
  WORD32 num_back_channel;
  WORD32 num_lfe_channel;
  WORD32 num_ind_cce;
  UINT32 avg_bit_rate;
} ia_audio_specific_config_struct;

typedef struct {
  WORD32 sample_rate_layer;
  WORD32 bit_rate;

} ia_layer_data_struct;

typedef struct {
  UINT32 stream_count;
  ia_audio_specific_config_struct str_audio_specific_config;
  ia_layer_data_struct str_layer;
  WORD32 tracks_in_layer;
  UWORD32 scal_out_select;
  WORD32 scal_out_object_type;
  WORD32 scal_out_num_channels;
  WORD32 scal_out_sampling_frequency;

} ia_frame_data_struct;

#endif
