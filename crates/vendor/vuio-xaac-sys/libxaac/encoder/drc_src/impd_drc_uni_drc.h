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
/* Defines for bitstream payload */
#define METHOD_DEFINITION_UNKNOWN_OTHER 0
#define METHOD_DEFINITION_PROGRAM_LOUDNESS 1
#define METHOD_DEFINITION_ANCHOR_LOUDNESS 2
#define METHOD_DEFINITION_MAX_OF_LOUDNESS_RANGE 3
#define METHOD_DEFINITION_MOMENTARY_LOUDNESS_MAX 4
#define METHOD_DEFINITION_SHORT_TERM_LOUDNESS_MAX 5
#define METHOD_DEFINITION_LOUDNESS_RANGE 6
#define METHOD_DEFINITION_MIXING_LEVEL 7
#define METHOD_DEFINITION_ROOM_TYPE 8
#define METHOD_DEFINITION_SHORT_TERM_LOUDNESS 9

#define MEASUREMENT_SYSTEM_BS_1770_3 2

#define EFFECT_BIT_NONE (-1) /* this effect bit is virtual */
#define EFFECT_BIT_DUCK_OTHER 0x0400
#define EFFECT_BIT_DUCK_SELF 0x0800

#define GAIN_CODING_PROFILE_REGULAR 0
#define GAIN_CODING_PROFILE_FADING 1
#define GAIN_CODING_PROFILE_CLIPPING 2
#define GAIN_CODING_PROFILE_CONSTANT 3

#define GAIN_INTERPOLATION_TYPE_SPLINE 0
#define GAIN_INTERPOLATION_TYPE_LINEAR 1

#define LOUDNESS_NORMALIZATION_GAIN_MAX_DEFAULT 1000 /* infinity as default */
#define GAIN_SET_COUNT_MAX 8                         /* reduced size */

#define LEFT_SIDE 0
#define RIGHT_SIDE 1
#define SPLIT_CHARACTERISTIC_COUNT_MAX 8
#define SPLIT_CHARACTERISTIC_MAX_NODE_COUNT 4 /* one side of characteristic */

#define GAINFORMAT_QMF32 0x1
#define GAINFORMAT_QMFHYBRID39 0x2
#define GAINFORMAT_QMF64 0x3
#define GAINFORMAT_QMFHYBRID71 0x4
#define GAINFORMAT_QMF128 0x5
#define GAINFORMAT_QMFHYBRID135 0x6
#define GAINFORMAT_UNIFORM 0x7

#define DRC_INPUT_LOUDNESS_TARGET (-31.0f) /* dB */

#define SHAPE_FILTER_COUNT_MAX 8

#define SHAPE_FILTER_DRC_GAIN_MAX_MINUS_ONE 1583.8931924611f /* 10^3.2 - 1 */

#define DOWNMIX_INSTRUCTIONS_COUNT_MAX 8       /* reduced size */
#define DRC_COEFFICIENTS_UNIDRC_V1_COUNT_MAX 2 /* reduced size */
#define DRC_INSTRUCTIONS_UNIDRC_V1_COUNT_MAX 8 /* reduced size */
#define SPLIT_CHARACTERISTIC_COUNT_MAX 8       /* reduced size */
#define SHAPE_FILTER_COUNT_MAX 8               /* reduced size */
#define ADDITIONAL_DOWNMIX_ID_COUNT_MAX MAX_ADDITIONAL_DOWNMIX_ID
#define MAX_TARGET_LAYOUT_COUNT 127
#define ADDITIONAL_DRC_SET_ID_COUNT_MAX 16
#define ADDITIONAL_EQ_SET_ID_COUNT_MAX 8
#define LOUD_EQ_GAIN_MAX_SEQUENCE_COUNT 4
#define FILTER_ELEMENT_COUNT_MAX 16 /* reduced size */
#define REAL_ZERO_RADIUS_ONE_COUNT_MAX 14
#define REAL_ZERO_COUNT_MAX 64
#define COMPLEX_ZERO_COUNT_MAX 64
#define REAL_POLE_COUNT_MAX 16
#define COMPLEX_POLE_COUNT_MAX 16
#define FIR_ORDER_MAX 128
#define EQ_MAX_NODE_COUNT 33
#define EQ_SUBBAND_GAIN_COUNT_MAX 135
#define UNIQUE_SUBBAND_GAIN_COUNT_MAX 16 /* reduced size */
#define FILTER_BLOCK_COUNT_MAX 16
#define FILTER_ELEMENT_COUNT_MAX 16      /* reduced size */
#define UNIQUE_SUBBAND_GAINS_COUNT_MAX 8 /* reduced size */
#define EQ_MAX_CHANNEL_GROUP_COUNT 4     /* reduced size */
#define EQ_FILTER_BLOCK_COUNT_MAX 4      /* reduced size */
#define LOUD_EQ_INSTRUCTIONS_COUNT_MAX 8 /* reduced size */
#define EQ_INSTRUCTIONS_COUNT_MAX 8

#define DRC_COMPLEXITY_LEVEL_MAX 15
#define EQ_COMPLEXITY_LEVEL_MAX 15
#define COMPLEXITY_W_SUBBAND_EQ 2.5f
#define COMPLEXITY_W_FIR 0.4f
#define COMPLEXITY_W_IIR 5.0f
#define COMPLEXITY_W_MOD_TIME 1.0f
#define COMPLEXITY_W_MOD_SUBBAND 2.0f
#define COMPLEXITY_W_LAP 2.0f
#define COMPLEXITY_W_SHAPE 6.0f
#define COMPLEXITY_W_SPLINE 5.0f
#define COMPLEXITY_W_LINEAR 2.5f
#define COMPLEXITY_W_PARAM_DRC_FILT 5.0f
#define COMPLEXITY_W_PARAM_DRC_SUBBAND 5.0f
#define COMPLEXITY_W_PARAM_LIM_FILT 4.5f
#define COMPLEXITY_W_PARAM_DRC_ATTACK 136.0f

#define MAX_DRC_SET_ID (63)
#define MAX_DRC_LOCATION (4)
#define MIN_DRC_TARGET_LOUDNESS (-63)
#define MAX_ATTENUATION_SCALING (1.875f)
#define MAX_AMPLIFICATION_SCALING (1.875f)
#define MIN_DRC_GAIN_OFFSET (-8.0f)
#define MAX_DRC_GAIN_OFFSET (8.0f)
#define MIN_LIMITER_PEAK_TARGET (-31.875f)
#define MAX_GAIN_CODING_PROFILE (3)
#define MAX_DRC_CHARACTERISTIC_VALUE (11)
#define MAX_CROSSOVER_FREQ_INDEX (15)
#define MIN_SAMPLE_PEAK_LEVEL (-107.0f)
#define MAX_SAMPLE_PEAK_LEVEL (20.0f)
#define MIN_TRUE_PEAK_LEVEL (-107.0f)
#define MAX_TRUE_PEAK_LEVEL (20.0f)
#define MAX_MEASUREMENT_SYSTEM_TYPE (11)
#define MAX_RELIABILITY_TYPE (3)
#define MAX_METHOD_DEFINITION_TYPE (9)
#define MIN_METHOD_VALUE (-116.0f)
#define MAX_METHOD_VALUE (121.0f)
#define MAX_FLT_VAL_DB (770.6367883810890080451095799195f)
#define DEFAULT_METHOD_VALUE (-31.0f)
#define DEFAULT_SAMPLE_PEAK_VALUE (-31.0f)

typedef struct {
  WORD32 level_estim_k_weighting_type;
  WORD32 level_estim_integration_time_present;
  WORD32 level_estim_integration_time;
  WORD32 drc_curve_definition_type;
  WORD32 drc_characteristic;
  WORD32 node_count;
  WORD32 node_level[MAX_PARAM_DRC_TYPE_FF_NODE_COUNT];
  WORD32 node_gain[MAX_PARAM_DRC_TYPE_FF_NODE_COUNT];
  WORD32 drc_gain_smooth_parameters_present;
  WORD32 gain_smooth_attack_time_slow;
  WORD32 gain_smooth_release_time_slow;
  WORD32 gain_smooth_time_fast_present;
  WORD32 gain_smooth_attack_time_fast;
  WORD32 gain_smooth_release_time_fast;
  WORD32 gain_smooth_threshold_present;
  WORD32 gain_smooth_attack_threshold;
  WORD32 gain_smooth_release_threshold;
  WORD32 gain_smooth_hold_off_count_present;
  WORD32 gain_smooth_hold_off;
  WORD32 disable_paramtric_drc;
} ia_drc_parametric_drc_type_feed_forward_struct;

typedef struct {
  WORD32 parametric_lim_threshold_present;
  FLOAT32 parametric_lim_threshold;
  WORD32 parametric_lim_attack;
  WORD32 parametric_lim_release_present;
  WORD32 parametric_lim_release;
  WORD32 drc_characteristic;
  WORD32 disable_paramtric_drc;
} ia_drc_parametric_drc_type_lim_struct;

typedef struct {
  WORD32 parametric_drc_id;
  WORD32 parametric_drc_look_ahead_present;
  WORD32 parametric_drc_look_ahead;
  WORD32 parametric_drc_preset_id_present;
  WORD32 parametric_drc_preset_id;
  WORD32 parametric_drc_type;
  WORD32 len_bit_size;
  ia_drc_parametric_drc_type_feed_forward_struct str_parametric_drc_type_feed_forward;
  ia_drc_parametric_drc_type_lim_struct str_parametric_drc_type_lim;
  WORD32 disable_paramtric_drc;
} ia_drc_parametric_drc_instructions_struct;

typedef struct {
  WORD32 parametric_drc_id;
  WORD32 side_chain_config_type;
  WORD32 downmix_id;
  WORD32 level_estim_channel_weight_format;
  FLOAT32 level_estim_channel_weight[MAX_CHANNEL_COUNT];
  WORD32 drc_input_loudness_present;
  FLOAT32 drc_input_loudness;

  /* derived data */
  WORD32 channel_count_drom_downmix_id;
} ia_drc_parametric_drc_gain_set_params_struct;

typedef struct {
  WORD32 drc_location;
  WORD32 parametric_drc_frame_size_format;
  WORD32 parametric_drc_frame_size;
  WORD32 parametric_drc_delay_max_present;
  WORD32 parametric_drc_delay_max;
  WORD32 reset_parametric_drc;
  WORD32 parametric_drc_gain_set_count;
  ia_drc_parametric_drc_gain_set_params_struct parametric_drc_gain_set_params[MAX_SEQUENCE_COUNT];
} ia_drc_coeff_parametric_drc_struct;

typedef struct {
  WORD32 base_ch_count;
  WORD32 layout_signaling_present;
  WORD32 defined_layout;
  WORD32 speaker_position[MAX_SPEAKER_POS_COUNT];
} ia_drc_channel_layout_struct;

typedef struct {
  WORD32 downmix_id;
  WORD32 target_ch_count;
  WORD32 target_layout;
  WORD32 downmix_coefficients_present;
  FLOAT32 downmix_coeff[MAX_DOWNMIX_COEFF_COUNT];
} ia_drc_downmix_instructions_struct;

typedef struct {
  FLOAT32 x;
  FLOAT32 y;
} ia_drc_gain_points_struct;

typedef struct {
  WORD32 gain_sequence_index;
  WORD32 drc_characteristic_present;
  WORD32 drc_characteristic_format_is_cicp;
  WORD32 drc_characteristic;
  WORD32 drc_characteristic_left_index;
  WORD32 drc_characteristic_right_index;
  WORD32 crossover_freq_index;
  WORD32 start_sub_band_index;
  WORD32 nb_points;
  FLOAT32 width;
  FLOAT32 attack;
  FLOAT32 decay;
  ia_drc_gain_points_struct gain_points[MAX_GAIN_POINTS];
} ia_drc_gain_params_struct;

typedef struct {
  WORD32 ducking_scaling_present;
  FLOAT32 ducking_scaling;
  FLOAT32 ducking_scaling_quantized;
} ia_drc_ducking_modifiers_struct;

typedef struct {
  WORD32 target_characteristic_left_present[MAX_BAND_COUNT];
  WORD32 target_characteristic_left_index[MAX_BAND_COUNT];
  WORD32 target_characteristic_right_present[MAX_BAND_COUNT];
  WORD32 target_characteristic_right_index[MAX_BAND_COUNT];
  WORD32 shape_filter_present;
  WORD32 shape_filter_index;

  WORD32 gain_scaling_present[MAX_BAND_COUNT];
  FLOAT32 attenuation_scaling[MAX_BAND_COUNT];
  FLOAT32 amplification_scaling[MAX_BAND_COUNT];
  WORD32 gain_offset_present[MAX_BAND_COUNT];
  FLOAT32 gain_offset[MAX_BAND_COUNT];

} ia_drc_gain_modifiers_struct;

typedef struct {
  WORD32 gain_coding_profile;
  WORD32 gain_interpolation_type;
  WORD32 full_frame;
  WORD32 time_alignment;
  WORD32 time_delta_min_present;
  WORD32 delta_tmin;
  WORD32 band_count;
  WORD32 drc_band_type;
  ia_drc_gain_params_struct gain_params[MAX_BAND_COUNT];
} ia_drc_gain_set_params_struct;

typedef struct {
  WORD32 characteristic_format;
  WORD32 bs_gain;
  WORD32 bs_io_ratio;
  WORD32 bs_exp;
  WORD32 flip_sign;
  WORD32 characteristic_node_count;
  FLOAT32 node_level[SPLIT_CHARACTERISTIC_MAX_NODE_COUNT + 1];
  FLOAT32 node_gain[SPLIT_CHARACTERISTIC_MAX_NODE_COUNT + 1];
} ia_drc_split_drc_characteristic_struct;

typedef struct {
  WORD32 corner_freq_index;
  WORD32 filter_strength_index;
} ia_drc_shape_filter_params_struct;

typedef struct {
  WORD32 lf_cut_filter_present;
  ia_drc_shape_filter_params_struct str_lf_cut_params;
  WORD32 lf_boost_filter_present;
  ia_drc_shape_filter_params_struct str_lf_boost_params;
  WORD32 hf_cut_filter_present;
  ia_drc_shape_filter_params_struct str_hf_cut_params;
  WORD32 hf_boost_filter_present;
  ia_drc_shape_filter_params_struct str_hf_boost_params;
} ia_drc_shape_filter_block_params_struct;

typedef struct {
  WORD32 drc_location;
  WORD32 drc_characteristic;
} ia_drc_coefficients_basic_struct;

typedef struct {
  WORD32 drc_location;
  WORD32 drc_frame_size_present;
  WORD32 drc_frame_size;
  WORD32 drc_characteristic_left_present;
  WORD32 characteristic_left_count;
  ia_drc_split_drc_characteristic_struct
      str_split_characteristic_left[SPLIT_CHARACTERISTIC_COUNT_MAX + 1];
  WORD32 drc_characteristic_right_present;
  WORD32 characteristic_right_count;
  ia_drc_split_drc_characteristic_struct
      str_split_characteristic_right[SPLIT_CHARACTERISTIC_COUNT_MAX];
  WORD32 shape_filters_present;
  WORD32 shape_filter_count;
  ia_drc_shape_filter_block_params_struct
      str_shape_filter_block_params[SHAPE_FILTER_COUNT_MAX + 1];
  WORD32 gain_sequence_count;
  WORD32 gain_set_count;
  ia_drc_gain_set_params_struct str_gain_set_params[GAIN_SET_COUNT_MAX];
} ia_drc_coefficients_uni_drc_struct;

typedef struct {
  WORD32 drc_set_id;
  WORD32 drc_location;
  WORD32 downmix_id;
  WORD32 additional_downmix_id_present;
  WORD32 additional_downmix_id_count;
  WORD32 additional_downmix_id[MAX_ADDITIONAL_DOWNMIX_ID];
  WORD32 drc_set_effect;
  WORD32 limiter_peak_target_present;
  FLOAT32 limiter_peak_target;
  WORD32 drc_set_target_loudness_present;
  WORD32 drc_set_target_loudness_value_upper;
  WORD32 drc_set_target_loudness_value_lower_present;
  WORD32 drc_set_target_loudness_value_lower;
} ia_drc_instructions_basic_struct;

typedef struct {
  WORD32 drc_set_id;
  WORD32 drc_set_complexity_level;
  WORD32 drc_apply_to_downmix;
  WORD32 requires_eq;
  WORD32 downmix_id_present;
  WORD32 drc_location;
  WORD32 downmix_id;
  WORD32 additional_downmix_id_present;
  WORD32 additional_downmix_id_count;
  WORD32 additional_downmix_id[MAX_ADDITIONAL_DOWNMIX_ID];
  WORD32 depends_on_drc_set_present;
  WORD32 depends_on_drc_set;
  WORD32 no_independent_use;
  WORD32 drc_set_effect;
  WORD32 gain_set_index[MAX_CHANNEL_COUNT];
  ia_drc_gain_modifiers_struct str_gain_modifiers[MAX_CHANNEL_GROUP_COUNT];
  ia_drc_ducking_modifiers_struct str_ducking_modifiers_for_channel[MAX_CHANNEL_COUNT];
  WORD32 limiter_peak_target_present;
  FLOAT32 limiter_peak_target;
  WORD32 drc_set_target_loudness_present;
  WORD32 drc_set_target_loudness_value_upper;
  WORD32 drc_set_target_loudness_value_lower_present;
  WORD32 drc_set_target_loudness_value_lower;
  WORD32 drc_instructions_type;
  WORD32 mae_group_id;
  WORD32 mae_group_preset_id;

  WORD32 drc_channel_count;
  WORD32 num_drc_channel_groups;
  WORD32 gain_set_index_for_channel_group[MAX_CHANNEL_GROUP_COUNT];
  WORD32 band_count_for_channel_group[MAX_CHANNEL_GROUP_COUNT];
  WORD32 gain_coding_profile_for_channel_group[MAX_CHANNEL_GROUP_COUNT];
  WORD32 gain_interpolation_type_for_channel_group[MAX_CHANNEL_GROUP_COUNT];
  WORD32 time_delta_min_for_channel_group[MAX_CHANNEL_GROUP_COUNT];
  WORD32 time_alignment_for_channel_group[MAX_CHANNEL_GROUP_COUNT];
  ia_drc_ducking_modifiers_struct
      str_ducking_modifiers_for_channel_group[MAX_CHANNEL_GROUP_COUNT];
  WORD32 channel_group_for_channel[MAX_CHANNEL_COUNT];
  WORD32 num_channels_per_channel_group[MAX_CHANNEL_GROUP_COUNT];
  WORD32 gain_element_count;
  WORD32 multiband_audio_signal_count;
  WORD32 channel_group_is_parametric_drc[MAX_CHANNEL_GROUP_COUNT];
  WORD32 gain_set_idx_for_ch_group_parametric_drc[MAX_CHANNEL_GROUP_COUNT];
#ifdef LOUDNESS_LEVELING_SUPPORT
  WORD32 leveling_present;
  WORD32 ducking_only_set_present;
#endif
} ia_drc_instructions_uni_drc;

typedef struct {
  WORD32 method_definition;
  FLOAT32 method_value;
  WORD32 measurement_system;
  WORD32 reliability;
} ia_drc_loudness_measure_struct;

typedef struct {
  WORD32 drc_set_id;
  WORD32 eq_set_id;
  WORD32 downmix_id;
  WORD32 sample_peak_level_present;
  FLOAT32 sample_peak_level;
  WORD32 true_peak_level_present;
  FLOAT32 true_peak_level;
  WORD32 true_peak_level_measurement_system;
  WORD32 true_peak_level_reliability;
  WORD32 measurement_count;
  ia_drc_loudness_measure_struct str_loudness_measure[MAX_MEASUREMENT_COUNT];
  WORD32 loudness_info_type;
  WORD32 mae_group_id;
  WORD32 mae_group_preset_id;
} ia_drc_loudness_info_struct;

typedef struct {
  WORD32 loud_eq_set_id;
  WORD32 drc_location;
  WORD32 downmix_id_present;
  WORD32 downmix_id;
  WORD32 additional_downmix_id_present;
  WORD32 additional_downmix_id_count;
  WORD32 additional_downmix_id[ADDITIONAL_DOWNMIX_ID_COUNT_MAX];
  WORD32 drc_set_id_present;
  WORD32 drc_set_id;
  WORD32 additional_drc_set_id_present;
  WORD32 additional_drc_set_id_count;
  WORD32 additional_drc_set_id[ADDITIONAL_DRC_SET_ID_COUNT_MAX];
  WORD32 eq_set_id_present;
  WORD32 eq_set_id;
  WORD32 additional_eq_set_id_present;
  WORD32 additional_eq_set_id_count;
  WORD32 additional_eq_set_id[ADDITIONAL_EQ_SET_ID_COUNT_MAX];
  WORD32 loudness_after_drc;
  WORD32 loudness_after_eq;
  WORD32 loud_eq_gain_sequence_count;
  WORD32 gain_sequence_index[LOUD_EQ_GAIN_MAX_SEQUENCE_COUNT];
  WORD32 drc_characteristic_format_is_cicp[LOUD_EQ_GAIN_MAX_SEQUENCE_COUNT];
  WORD32 drc_characteristic[LOUD_EQ_GAIN_MAX_SEQUENCE_COUNT];
  WORD32 drc_characteristic_left_index[LOUD_EQ_GAIN_MAX_SEQUENCE_COUNT];
  WORD32 drc_characteristic_right_index[LOUD_EQ_GAIN_MAX_SEQUENCE_COUNT];
  WORD32 frequency_range_index[LOUD_EQ_GAIN_MAX_SEQUENCE_COUNT];
  FLOAT32 loud_eq_scaling[LOUD_EQ_GAIN_MAX_SEQUENCE_COUNT];
  FLOAT32 loud_eq_offset[LOUD_EQ_GAIN_MAX_SEQUENCE_COUNT];
} ia_drc_loud_eq_instructions_struct;

typedef struct {
  WORD32 filter_element_index;
  WORD32 filter_element_gain_present;
  FLOAT32 filter_element_gain;
} ia_drc_filter_element_struct;

typedef struct {
  WORD32 filter_element_count;
  ia_drc_filter_element_struct filter_element[FILTER_ELEMENT_COUNT_MAX];
} ia_drc_filter_block_struct;

typedef struct {
  WORD32 eq_filter_format;
  WORD32 real_zero_radius_one_count;
  WORD32 real_zero_count;
  WORD32 generic_zero_count;
  WORD32 real_pole_count;
  WORD32 complex_pole_count;
  FLOAT32 zero_sign[REAL_ZERO_RADIUS_ONE_COUNT_MAX];
  FLOAT32 real_zero_radius[REAL_ZERO_COUNT_MAX];
  FLOAT32 generic_zero_radius[COMPLEX_ZERO_COUNT_MAX];
  FLOAT32 generic_zero_angle[COMPLEX_ZERO_COUNT_MAX];
  FLOAT32 real_pole_radius[REAL_POLE_COUNT_MAX];
  FLOAT32 complex_pole_radius[COMPLEX_POLE_COUNT_MAX];
  FLOAT32 complex_pole_angle[COMPLEX_POLE_COUNT_MAX];
  WORD32 fir_filter_order;
  WORD32 fir_symmetry;
  FLOAT32 fir_coefficient[FIR_ORDER_MAX / 2];
} ia_drc_unique_td_filter_element_struct;

typedef struct {
  WORD32 n_eq_nodes;
  FLOAT32 eq_slope[EQ_MAX_NODE_COUNT];
  WORD32 eq_freq_delta[EQ_MAX_NODE_COUNT];
  FLOAT32 eq_gain_initial;
  FLOAT32 eq_gain_delta[EQ_MAX_NODE_COUNT];
} ia_drc_eq_subband_gain_spline_struct;

typedef struct {
  WORD32 eq_subband_gain[EQ_SUBBAND_GAIN_COUNT_MAX];
} ia_drc_eq_subband_gain_vector_struct;

typedef struct {
  WORD32 eq_delay_max_present;
  WORD32 eq_delay_max;
  WORD32 unique_filter_block_count;
  ia_drc_filter_block_struct str_filter_block[FILTER_BLOCK_COUNT_MAX];
  WORD32 unique_td_filter_element_count;
  ia_drc_unique_td_filter_element_struct str_unique_td_filter_element[FILTER_ELEMENT_COUNT_MAX];
  WORD32 unique_eq_subband_gains_count;
  WORD32 eq_subband_gain_representation;
  WORD32 eq_subband_gain_format;
  WORD32 eq_subband_gain_count;
  ia_drc_eq_subband_gain_spline_struct str_eq_subband_gain_spline[UNIQUE_SUBBAND_GAIN_COUNT_MAX];
  ia_drc_eq_subband_gain_vector_struct str_eq_subband_gain_vector[UNIQUE_SUBBAND_GAIN_COUNT_MAX];
} ia_drc_eq_coefficients_struct;

typedef struct {
  WORD32 filter_block_count;
  WORD32 filter_block_index[EQ_FILTER_BLOCK_COUNT_MAX];
} ia_drc_filter_block_refs_struct;

typedef struct {
  WORD32 eq_cascade_gain_present[EQ_MAX_CHANNEL_GROUP_COUNT];
  WORD32 eq_cascade_gain[EQ_MAX_CHANNEL_GROUP_COUNT];
  ia_drc_filter_block_refs_struct str_filter_block_refs[EQ_MAX_CHANNEL_GROUP_COUNT];
  WORD32 eq_phase_alignment_present;
  WORD32 eq_phase_alignment[EQ_MAX_CHANNEL_GROUP_COUNT][EQ_MAX_CHANNEL_GROUP_COUNT];
} ia_drc_td_filter_cascade_struct;

typedef struct {
  WORD32 eq_set_id;
  WORD32 eq_set_complexity_level;
  WORD32 downmix_id_present;
  WORD32 downmix_id;
  WORD32 eq_apply_to_downmix;
  WORD32 additional_downmix_id_present;
  WORD32 additional_downmix_id_count;
  WORD32 additional_downmix_id[ADDITIONAL_DOWNMIX_ID_COUNT_MAX];
  WORD32 drc_set_id;
  WORD32 additional_drc_set_id_present;
  WORD32 additional_drc_set_id_count;
  WORD32 additional_drc_set_id[ADDITIONAL_DRC_SET_ID_COUNT_MAX];
  WORD32 eq_set_purpose;
  WORD32 depends_on_eq_set_present;
  WORD32 depends_on_eq_set;
  WORD32 no_independent_eq_use;
  WORD32 eq_channel_count;
  WORD32 eq_channel_group_count;
  WORD32 eq_channel_group_for_channel[MAX_CHANNEL_COUNT];
  WORD32 td_filter_cascade_present;
  ia_drc_td_filter_cascade_struct str_td_filter_cascade;
  WORD32 subband_gains_present;
  WORD32 subband_gains_index[EQ_MAX_CHANNEL_GROUP_COUNT];
  WORD32 eq_transition_duration_present;
  FLOAT32 eq_transition_duration;
} ia_drc_eq_instructions_struct;

typedef struct {
  WORD32 uni_drc_config_ext_type[MAX_EXT_COUNT];
  WORD32 ext_bit_size[MAX_EXT_COUNT - 1];
  /* UNIDRC_CONF_EXT_PARAM_DRC */
  WORD32 parametric_drc_present;
  ia_drc_coeff_parametric_drc_struct str_drc_coeff_parametric_drc;
  WORD32 parametric_drc_instructions_count;
  ia_drc_parametric_drc_instructions_struct
      str_parametric_drc_instructions[MAX_PARAM_DRC_INSTRUCTIONS_COUNT];

  /* UNIDRC_CONF_EXT_V1 */
  WORD32 drc_extension_v1_present;
  WORD32 downmix_instructions_v1_present;
  WORD32 downmix_instructions_v1_count;
  ia_drc_downmix_instructions_struct str_downmix_instructions_v1[DOWNMIX_INSTRUCTIONS_COUNT_MAX];
  WORD32 drc_coeffs_and_instructions_uni_drc_v1_present;
  WORD32 drc_coefficients_uni_drc_v1_count;
  ia_drc_coefficients_uni_drc_struct
      str_drc_coefficients_uni_drc_v1[DRC_COEFFICIENTS_UNIDRC_V1_COUNT_MAX];
  WORD32 drc_instructions_uni_drc_v1_count;
  ia_drc_instructions_uni_drc
      str_drc_instructions_uni_drc_v1[DRC_INSTRUCTIONS_UNIDRC_V1_COUNT_MAX];
  WORD32 loud_eq_instructions_present;
  WORD32 loud_eq_instructions_count;
  ia_drc_loud_eq_instructions_struct str_loud_eq_instructions[LOUD_EQ_INSTRUCTIONS_COUNT_MAX];
  WORD32 eq_present;
  ia_drc_eq_coefficients_struct str_eq_coefficients;
  WORD32 eq_instructions_count;
  ia_drc_eq_instructions_struct str_eq_instructions[EQ_INSTRUCTIONS_COUNT_MAX];
} ia_drc_uni_drc_config_ext_struct;

typedef struct {
  WORD32 sample_rate_present;
  WORD32 sample_rate;
  WORD32 downmix_instructions_count;
  WORD32 drc_coefficients_uni_drc_count;
  WORD32 drc_instructions_uni_drc_count;
  WORD32 drc_instructions_count_plus;
  WORD32 drc_description_basic_present;
  WORD32 drc_coefficients_basic_count;
  WORD32 drc_instructions_basic_count;
  WORD32 uni_drc_config_ext_present;
  ia_drc_uni_drc_config_ext_struct str_uni_drc_config_ext;
  ia_drc_coefficients_basic_struct str_drc_coefficients_basic[MAX_DRC_COEFF_COUNT];
  ia_drc_instructions_basic_struct str_drc_instructions_basic[MAX_DRC_INSTRUCTIONS_BASIC_COUNT];
  ia_drc_coefficients_uni_drc_struct str_drc_coefficients_uni_drc[MAX_DRC_COEFF_COUNT];
  ia_drc_instructions_uni_drc str_drc_instructions_uni_drc[MAX_DRC_INSTRUCTIONS_COUNT];
  ia_drc_channel_layout_struct str_channel_layout;
  ia_drc_downmix_instructions_struct str_downmix_instructions[MAX_DOWNMIX_INSTRUCTION_COUNT];
  WORD32 loudness_info_set_present;
} ia_drc_uni_drc_config_struct;

typedef struct {
  WORD32 loudness_info_v1_album_count;
  WORD32 loudness_info_v1_count;
  ia_drc_loudness_info_struct str_loudness_info_v1_album[MAX_LOUDNESS_INFO_COUNT];
  ia_drc_loudness_info_struct str_loudness_info_v1[MAX_LOUDNESS_INFO_COUNT];
} ia_drc_loudness_info_set_ext_eq_struct;

typedef struct {
  WORD32 loudness_info_set_ext_type[MAX_EXT_COUNT];
  WORD32 ext_bit_size[MAX_EXT_COUNT - 1];
  ia_drc_loudness_info_set_ext_eq_struct str_loudness_info_set_ext_eq;
} ia_drc_loudness_info_set_extension_struct;

typedef struct {
  WORD32 loudness_info_album_count;
  WORD32 loudness_info_count;
  WORD32 loudness_info_set_ext_present;
  ia_drc_loudness_info_struct str_loudness_info_album[MAX_LOUDNESS_INFO_COUNT];
  ia_drc_loudness_info_struct str_loudness_info[MAX_LOUDNESS_INFO_COUNT];
  ia_drc_loudness_info_set_extension_struct str_loudness_info_set_extension;
} ia_drc_loudness_info_set_struct;

typedef struct {
  WORD32 uni_drc_gain_ext_present;
  WORD32 uni_drc_gain_ext_type[MAX_EXT_COUNT];
  WORD32 ext_bit_size[MAX_EXT_COUNT - 1];
} ia_drc_uni_drc_gain_ext_struct;
