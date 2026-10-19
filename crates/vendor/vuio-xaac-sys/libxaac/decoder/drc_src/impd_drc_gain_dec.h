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
#ifndef IMPD_DRC_GAIN_DEC_H
#define IMPD_DRC_GAIN_DEC_H

#ifdef __cplusplus
extern "C" {
#endif

typedef struct {
  ia_node_struct str_node;
  ia_node_struct prev_node;
  FLOAT32 lpcm_gains[2 * AUDIO_CODEC_FRAME_SIZE_MAX + MAX_SIGNAL_DELAY];
} ia_interp_buf_struct;

typedef struct {
  WORD32 buf_interpolation_count;
  ia_interp_buf_struct* buf_interpolation;
} ia_gain_buffer_struct;

typedef struct {
  ia_gain_buffer_struct pstr_gain_buf[SEL_DRC_COUNT];
} ia_drc_gain_buffers_struct;

typedef struct {
  WORD32 gain_interpolation_type;
  WORD32 gain_modification_flag;
  WORD32 ducking_flag;
  WORD32 clipping_flag;
  ia_ducking_modifiers_struct* pstr_ducking_modifiers;
  ia_gain_modifiers_struct* pstr_gain_modifiers;
  WORD32 drc_characteristic_present;
  WORD32 drc_source_characteristic_cicp_format;
  WORD32 source_drc_characteristic;
  ia_split_drc_characteristic_struct* split_source_characteristic_left;
  ia_split_drc_characteristic_struct* split_source_characteristic_right;
  WORD32 drc_target_characteristic_cicp_format;
  WORD32 target_drc_characteristic;
  ia_split_drc_characteristic_struct* split_target_characteristic_left;
  ia_split_drc_characteristic_struct* split_target_characteristic_right;
  WORD32 interpolation_loud_eq;
  WORD32 limiter_peak_target_present;
  FLOAT32 limiter_peak_target;
  FLOAT32 loudness_normalization_gain_db;
  WORD32 delta_tmin;
  WORD32 characteristic_index;
  FLOAT32 compress;
  FLOAT32 boost;
} ia_interp_params_struct;

typedef struct {
  WORD32 drc_instructions_index;
  WORD32 drc_coeff_idx;
  WORD32 dwnmix_instructions_index;
} ia_sel_drc_struct;

typedef struct {
  WORD32 sample_rate;
  WORD32 delta_tmin_default;
  WORD32 drc_frame_size;
  WORD32 delay_mode;
  WORD32 sub_band_domain_mode;
  WORD32 gain_delay_samples;
  WORD32 parametric_drc_delay;
  WORD32 eq_delay;
  WORD32 audio_delay_samples;
  WORD32 drc_set_counter;
  WORD32 multiband_sel_drc_idx;

  ia_sel_drc_struct sel_drc_array[SEL_DRC_COUNT];

} ia_drc_params_struct;

#ifdef __cplusplus
}
#endif
#endif
