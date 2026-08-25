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
#define MAX_EQ_CHANNEL_COUNT (8)
#define MAX_EQ_AUDIO_DELAY (1024)
#define MAX_EQ_FIR_FILTER_SIZE (128)
#define MAX_EQ_SUBBAND_COUNT (256)
#define MAX_EQ_INTERMEDIATE_2ND_ORDER_PARAMS_COUNT (32)
#define MAX_EQ_FILTER_SECTION_COUNT (8)
#define MAX_EQ_FILTER_ELEMENT_COUNT (4)
#define MAX_MATCHING_PHASE_FILTER_COUNT (32)

#define EQ_FILTER_DOMAIN_NONE 0
#define EQ_FILTER_DOMAIN_TIME (1)
#define EQ_FILTER_DOMAIN_SUBBAND (2)

#define CONFIG_REAL_POLE (0)
#define CONFIG_COMPLEX_POLE (1)
#define CONFIG_REAL_ZERO_RADIUS_ONE (2)
#define CONFIG_REAL_ZERO (3)
#define CONFIG_GENERIC_ZERO (4)

#define STEP_RATIO_F_LOW (20.0f)

#define FILTER_ELEMENT_FORMAT_POLE_ZERO (0)
#define FILTER_ELEMENT_FORMAT_FIR (1)

#ifndef M_PI
#define M_PI (3.14159265358979323846)
#endif

#define STEP_RATIO_COMPUTED (0.0739601776f)

typedef struct {
  WORD32 delay;
  FLOAT32 state[MAX_EQ_CHANNEL_COUNT][MAX_EQ_AUDIO_DELAY];
} ia_drc_audio_delay_struct;

typedef struct {
  FLOAT32 radius;
  FLOAT32 coeff[2];
} ia_drc_second_order_filter_params_struct;

typedef struct {
  WORD32 coeff_count;
  FLOAT32 coeff[MAX_EQ_FIR_FILTER_SIZE];
  FLOAT32 state[MAX_EQ_CHANNEL_COUNT][MAX_EQ_FIR_FILTER_SIZE];
} ia_drc_fir_filter_struct;

typedef struct {
  WORD32 eq_frame_size_subband;
  WORD32 coeff_count;
  FLOAT32 subband_coeff[MAX_EQ_SUBBAND_COUNT];
} ia_drc_subband_filter_struct;

typedef struct {
  WORD32 filter_format;
  WORD32 filter_param_count_for_zeros;
  ia_drc_second_order_filter_params_struct
      str_second_order_filter_params_for_zeros[MAX_EQ_INTERMEDIATE_2ND_ORDER_PARAMS_COUNT];
  WORD32 filter_param_count_for_poles;
  ia_drc_second_order_filter_params_struct
      str_second_order_filter_params_for_poles[MAX_EQ_INTERMEDIATE_2ND_ORDER_PARAMS_COUNT];
  ia_drc_fir_filter_struct str_fir_filter;
} ia_drc_intermediate_filter_params_struct;

typedef struct {
  FLOAT32 state_in_1;
  FLOAT32 state_in_2;
  FLOAT32 state_out_1;
  FLOAT32 state_out_2;
} ia_drc_filter_section_state_struct;

typedef struct {
  FLOAT32 var_a1;
  FLOAT32 var_a2;
  FLOAT32 var_b1;
  FLOAT32 var_b2;
  ia_drc_filter_section_state_struct str_filter_section_state[MAX_EQ_CHANNEL_COUNT];
} ia_drc_filter_section_struct;

typedef struct {
  WORD32 member_count;
  WORD32 member_index[EQ_MAX_CHANNEL_GROUP_COUNT];
} ia_drc_cascade_alignment_group_struct;

typedef struct {
  WORD32 is_valid;
  WORD32 matches_filter_count;
  WORD32 matches_filter[MAX_EQ_FILTER_SECTION_COUNT];
  FLOAT32 gain;
  WORD32 section_count;
  ia_drc_filter_section_struct str_filter_section[MAX_EQ_FILTER_SECTION_COUNT];
  ia_drc_audio_delay_struct str_audio_delay;
} ia_drc_phase_alignment_filter_struct;

typedef ia_drc_phase_alignment_filter_struct ia_drc_matching_phase_filter_struct;

typedef struct {
  WORD32 matches_cascade_index;
  WORD32 allpass_count;
  ia_drc_matching_phase_filter_struct str_matching_phase_filter[MAX_MATCHING_PHASE_FILTER_COUNT];
} ia_drc_allpass_chain_struct;

typedef struct {
  WORD32 section_count;
  ia_drc_filter_section_struct str_filter_section[MAX_EQ_FILTER_SECTION_COUNT];
  WORD32 fir_coeffs_present;
  ia_drc_fir_filter_struct str_fir_filter;
  ia_drc_audio_delay_struct str_audio_delay;
} ia_drc_pole_zero_filter_struct;

typedef struct {
  FLOAT32 element_gain_linear;
  WORD32 format;
  ia_drc_pole_zero_filter_struct str_pole_zero_filter;
  ia_drc_fir_filter_struct str_fir_filter;
  WORD32 phase_alignment_filter_count;
  ia_drc_phase_alignment_filter_struct str_phase_alignment_filter[MAX_EQ_FILTER_ELEMENT_COUNT];
} ia_drc_eq_filter_element_struct;

typedef struct {
  WORD32 element_count;
  ia_drc_eq_filter_element_struct str_eq_filter_element[MAX_EQ_FILTER_ELEMENT_COUNT];
  ia_drc_matching_phase_filter_struct str_matching_phase_filter_element_0;
} ia_drc_eq_filter_block_struct;

typedef struct {
  FLOAT32 cascade_gain_linear;
  WORD32 block_count;
  ia_drc_eq_filter_block_struct str_eq_filter_block[EQ_FILTER_BLOCK_COUNT_MAX];
  WORD32 phase_alignment_filter_count;
  ia_drc_phase_alignment_filter_struct
      str_phase_alignment_filter[EQ_FILTER_BLOCK_COUNT_MAX * EQ_FILTER_BLOCK_COUNT_MAX];
} ia_drc_filter_cascade_t_domain_struct;

typedef struct {
  WORD32 domain;
  WORD32 audio_channel_count;
  WORD32 eq_channel_group_count;
  WORD32 eq_channel_group_for_channel[MAX_EQ_CHANNEL_COUNT];
  ia_drc_filter_cascade_t_domain_struct str_filter_cascade_t_domain[EQ_MAX_CHANNEL_GROUP_COUNT];
  ia_drc_subband_filter_struct str_subband_filter[EQ_MAX_CHANNEL_GROUP_COUNT];
} ia_drc_eq_set_struct;

IA_ERRORCODE impd_drc_derive_eq_set(ia_drc_eq_coefficients_struct *pstr_eq_coefficients,
                                    ia_drc_eq_instructions_struct *pstr_eq_instructions,
                                    const FLOAT32 audio_sample_rate, const WORD32 drc_frame_size,
                                    const WORD32 sub_band_domain_mode,
                                    ia_drc_eq_set_struct *pstr_eq_set, VOID *ptr_scratch,
                                    WORD32 *scratch_used);

IA_ERRORCODE impd_drc_get_eq_complexity(ia_drc_eq_set_struct *pstr_eq_set,
                                        WORD32 *eq_complexity_level);
