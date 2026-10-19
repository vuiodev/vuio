/******************************************************************************
 *
 * Copyright (C) 2015 The Android Open Source Project
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
#ifndef IMPD_DRC_UNI_GAIN_DEC_H
#define IMPD_DRC_UNI_GAIN_DEC_H

#ifdef __cplusplus
extern "C" {
#endif

typedef struct {
  ia_node_struct str_node;
  ia_node_struct prev_node;
  FLOAT32 lpcm_gains[2 * AUDIO_CODEC_FRAME_SIZE_MAX + MAX_SIGNAL_DELAY];
} ia_interp_buf_struct;

typedef struct {
#if DRC_GAIN_DEBUG_FILE
  FILE* f_gain_debug;
#endif
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
  WORD32 audio_delay_samples;
  WORD32 drc_set_counter;
  WORD32 multiband_sel_drc_idx;

  ia_sel_drc_struct sel_drc_array[SEL_DRC_COUNT];

} ia_drc_params_struct;

WORD32
impd_gain_buf_init(WORD32 index, WORD32 gain_element_count,
                   WORD32 drc_frame_size, ia_gain_buffer_struct* pstr_gain_buf);

WORD32
impd_advance_buf(WORD32 drc_frame_size, ia_gain_buffer_struct* pstr_gain_buf);

WORD32
impd_map_gain(FLOAT32 gain_in_db, FLOAT32* gain_out_db);

WORD32
impd_conv_to_linear_domain(ia_interp_params_struct* interp_params_str,
                           WORD32 drc_band, FLOAT32 loc_db_gain,
                           FLOAT32 in_param_db_slope,
                           FLOAT32* out_param_lin_gain,
                           FLOAT32* out_param_lin_slope);

WORD32
impd_interpolate_drc_gain(ia_interp_params_struct* interp_params_str,
                          WORD32 drc_band, WORD32 gain_step_tdomain,
                          FLOAT32 gain0, FLOAT32 gain1, FLOAT32 slope0,
                          FLOAT32 slope1, FLOAT32* result);

WORD32
impd_concatenate_segments(WORD32 drc_frame_size, WORD32 drc_band,
                          ia_interp_params_struct* interp_params_str,
                          ia_spline_nodes_struct* str_spline_nodes,
                          ia_interp_buf_struct* buf_interpolation);

#ifdef __cplusplus
}
#endif
#endif
