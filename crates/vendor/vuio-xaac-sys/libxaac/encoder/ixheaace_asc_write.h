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
  WORD32 audio_object_type;
  UWORD32 samp_freq_index;
  UWORD32 sampling_frequency;
  WORD32 core_sbr_framelength_index;
  UWORD32 channel_configuration;
  UWORD32 ext_audio_object_type;
  UWORD32 ext_samp_freq_index;
  UWORD32 ext_sampling_frequency;
  WORD32 num_audio_channels;
  WORD32 output_channel_pos[BS_MAX_NUM_OUT_CHANNELS];
  WORD32 sbr_present_flag;
  WORD32 cfg_ext_present;
  WORD32 ext_sync_word;
  ia_usac_config_struct str_usac_config;
  ia_aace_config_struct str_aac_config;
} ixheaace_audio_specific_config_struct;

WORD32 ixheaace_get_audiospecific_config_bytes(
    ia_bit_buf_struct *pstr_it_bit_buff,
    ixheaace_audio_specific_config_struct *pstr_audio_specific_config, WORD32 aot,
    WORD32 ccfl_idx);

WORD32 ixheaace_get_usac_config_bytes(
    ia_bit_buf_struct *pstr_it_bit_buff,
    ixheaace_audio_specific_config_struct *pstr_audio_specific_config, WORD32 ccfl_idx);