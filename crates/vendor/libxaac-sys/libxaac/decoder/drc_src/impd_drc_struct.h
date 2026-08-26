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
#ifndef IMPD_DRC_STURCT_H
#define IMPD_DRC_STURCT_H

#ifdef __cplusplus
extern "C" {
#endif

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

#define MEASUREMENT_SYSTEM_UNKNOWN_OTHER 0
#define MEASUREMENT_SYSTEM_EBU_R_128 1
#define MEASUREMENT_SYSTEM_BS_1770_4 2
#define MEASUREMENT_SYSTEM_BS_1770_3 MEASUREMENT_SYSTEM_BS_1770_4
#define MEASUREMENT_SYSTEM_BS_1770_4_PRE_PROCESSING 3
#define MEASUREMENT_SYSTEM_BS_1770_3_PRE_PROCESSING \
  MEASUREMENT_SYSTEM_BS_1770_4_PRE_PROCESSING
#define MEASUREMENT_SYSTEM_USER 4
#define MEASUREMENT_SYSTEM_EXPERT_PANEL 5
#define MEASUREMENT_SYSTEM_BS_1771_1 6
#define MEASUREMENT_SYSTEM_RESERVED_A 7
#define MEASUREMENT_SYSTEM_RESERVED_B 8
#define MEASUREMENT_SYSTEM_RESERVED_C 9
#define MEASUREMENT_SYSTEM_RESERVED_D 10
#define MEASUREMENT_SYSTEM_RESERVED_E 11

#define RELIABILITY_UKNOWN 0
#define RELIABILITY_UNVERIFIED 1
#define RELIABILITY_CEILING 2
#define RELIABILITY_ACCURATE 3

#define EFFECT_BIT_COUNT 12

#define EFFECT_BIT_NONE (-1)
#define EFFECT_BIT_NIGHT 0x0001
#define EFFECT_BIT_NOISY 0x0002
#define EFFECT_BIT_LIMITED 0x0004
#define EFFECT_BIT_LOWLEVEL 0x0008
#define EFFECT_BIT_DIALOG 0x0010
#define EFFECT_BIT_GENERAL_COMPR 0x0020
#define EFFECT_BIT_EXPAND 0x0040
#define EFFECT_BIT_ARTISTIC 0x0080
#define EFFECT_BIT_CLIPPING 0x0100
#define EFFECT_BIT_FADE 0x0200
#define EFFECT_BIT_DUCK_OTHER 0x0400
#define EFFECT_BIT_DUCK_SELF 0x0800

#define GAIN_CODING_PROFILE_REGULAR 0
#define GAIN_CODING_PROFILE_FADING 1
#define GAIN_CODING_PROFILE_CLIPPING 2
#define GAIN_CODING_PROFILE_CONSTANT 3
#define GAIN_CODING_PROFILE_DUCKING GAIN_CODING_PROFILE_CLIPPING

#define GAIN_INTERPOLATION_TYPE_SPLINE 0
#define GAIN_INTERPOLATION_TYPE_LINEAR 1

#define USER_METHOD_DEFINITION_DEFAULT 0
#define USER_METHOD_DEFINITION_PROGRAM_LOUDNESS 1
#define USER_METHOD_DEFINITION_ANCHOR_LOUDNESS 2

#define USER_MEASUREMENT_SYSTEM_DEFAULT 0
#define USER_MEASUREMENT_SYSTEM_BS_1770_4 1
#define USER_MEASUREMENT_SYSTEM_BS_1770_3 USER_MEASUREMENT_SYSTEM_BS_1770_4
#define USER_MEASUREMENT_SYSTEM_USER 2
#define USER_MEASUREMENT_SYSTEM_EXPERT_PANEL 3
#define USER_MEASUREMENT_SYSTEM_RESERVED_A 4
#define USER_MEASUREMENT_SYSTEM_RESERVED_B 5
#define USER_MEASUREMENT_SYSTEM_RESERVED_C 6
#define USER_MEASUREMENT_SYSTEM_RESERVED_D 7
#define USER_MEASUREMENT_SYSTEM_RESERVED_E 8

#define USER_LOUDNESS_PREPROCESSING_DEFAULT 0
#define USER_LOUDNESS_PREPROCESSING_OFF 1
#define USER_LOUDNESS_PREPROCESSING_HIGHPASS 2

#define LOUDNESS_DEVIATION_MAX_DEFAULT 63
#define LOUDNESS_NORMALIZATION_GAIN_MAX_DEFAULT 1000

#define SHORT_TERM_LOUDNESS_TO_AVG 0
#define MOMENTARY_LOUDNESS_TO_AVG 1
#define TOP_OF_LOUDNESS_RANGE_TO_AVG 2

#define DRC_COMPLEXITY_LEVEL_MAX 0xF
#define EQ_COMPLEXITY_LEVEL_MAX 0xF
#define COMPLEXITY_LEVEL_SUPPORTED_TOTAL 20.0f

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

#define LEFT_SIDE 0
#define RIGHT_SIDE 1

#define CHARACTERISTIC_SIGMOID 0
#define CHARACTERISTIC_NODES 1
#define CHARACTERISTIC_PASS_THRU 2

#define GAINFORMAT_QMF32 0x1
#define GAINFORMAT_QMFHYBRID39 0x2
#define GAINFORMAT_QMF64 0x3
#define GAINFORMAT_QMFHYBRID71 0x4
#define GAINFORMAT_QMF128 0x5
#define GAINFORMAT_QMFHYBRID135 0x6
#define GAINFORMAT_UNIFORM 0x7

#define DRC_INPUT_LOUDNESS_TARGET (-31.0f)

#define SHAPE_FILTER_TYPE_OFF 0
#define SHAPE_FILTER_TYPE_LF_CUT 1
#define SHAPE_FILTER_TYPE_LF_BOOST 2
#define SHAPE_FILTER_TYPE_HF_CUT 3
#define SHAPE_FILTER_TYPE_HF_BOOST 4

#define SHAPE_FILTER_DRC_GAIN_MAX_MINUS_ONE 1583.8931924611f

typedef struct {
  WORD32 type;
  FLOAT32 gain_offset;
  FLOAT32 y1_bound;
  FLOAT32 warped_gain_max;
  FLOAT32 factor;
  FLOAT32 coeff_sum;
  FLOAT32 partial_coeff_sum;
  FLOAT32 g_norm;
  FLOAT32 a1;
  FLOAT32 a2;
  FLOAT32 b1;
  FLOAT32 b2;
  FLOAT32 audio_in_state_1[MAX_CHANNEL_COUNT];
  FLOAT32 audio_in_state_2[MAX_CHANNEL_COUNT];
  FLOAT32 audio_out_state_1[MAX_CHANNEL_COUNT];
  FLOAT32 audio_out_state_2[MAX_CHANNEL_COUNT];
} ia_shape_filter_struct;

typedef struct {
  WORD32 shape_flter_block_flag;
  FLOAT32 drc_gain_last;
  ia_shape_filter_struct shape_filter[4];
} shape_filter_block;

typedef struct {
  WORD32 level_estim_k_weighting_type;
  WORD32 level_estim_integration_time_present;
  WORD32 level_estim_integration_time;
  WORD32 drc_curve_definition_type;
  WORD32 drc_characteristic;
  WORD32 node_count;
  WORD32 node_level[PARAM_DRC_TYPE_FF_NODE_COUNT_MAX];
  WORD32 node_gain[PARAM_DRC_TYPE_FF_NODE_COUNT_MAX];
  WORD32 drc_gain_smooth_parameters_present;
  WORD32 gain_smooth_attack_time_slow;
  WORD32 gain_smooth_release_time_slow;
  WORD32 gain_smooth_time_fast_present;
  WORD32 gain_smooth_attack_time_fast;
  WORD32 gain_smooth_release_time_fast;
  WORD32 gain_smooth_threshold_present;
  WORD32 gain_smooth_attack_threshold;
  WORD32 gain_smooth_rel_threshold;
  WORD32 gain_smooth_hold_off_count_present;
  WORD32 gain_smooth_hold_off;

  WORD32 disable_paramteric_drc;
} ia_parametric_drc_type_feed_forward_struct;

typedef struct {
  WORD32 parametric_lim_threshold_present;
  FLOAT32 parametric_lim_threshold;
  WORD32 parametric_lim_attack;
  WORD32 parametric_lim_release_present;
  WORD32 parametric_lim_release;
  WORD32 drc_characteristic;

  WORD32 disable_paramteric_drc;
} ia_parametric_drc_lim_struct;

typedef struct {
  WORD32 parametric_drc_id;
  WORD32 parametric_drc_look_ahead_flag;
  WORD32 parametric_drc_look_ahead;
  WORD32 parametric_drc_preset_id_present;
  WORD32 parametric_drc_preset_id;
  WORD32 parametric_drc_type;
  WORD32 len_bit_size;
  ia_parametric_drc_type_feed_forward_struct
      str_parametric_drc_type_feed_forward;
  ia_parametric_drc_lim_struct parametric_drc_lim;

  WORD32 drc_characteristic;
  WORD32 disable_paramteric_drc;
} ia_parametric_drc_instructions_struct;

typedef struct {
  WORD32 parametric_drc_id;
  WORD32 side_chain_config_type;
  WORD32 downmix_id;
  WORD32 level_estim_channel_weight_format;
  FLOAT32 level_estim_ch_weight[MAX_CHANNEL_COUNT];
  WORD32 drc_input_loudness_present;
  FLOAT32 drc_input_loudness;

  WORD32 ch_count_from_dwnmix_id;
} ia_parametric_drc_gain_set_params_struct;

typedef struct {
  WORD32 drc_location;
  WORD32 parametric_drc_frame_size_format;
  WORD32 parametric_drc_frame_size;
  WORD32 parametric_drc_delay_max_present;
  WORD32 parametric_drc_delay_max;
  WORD32 reset_parametric_drc;
  WORD32 parametric_drc_gain_set_count;
  ia_parametric_drc_gain_set_params_struct
      str_parametric_drc_gain_set_params[SEQUENCE_COUNT_MAX];
} ia_drc_coeff_parametric_drc_struct;

typedef struct {
  WORD32 base_channel_count;
  WORD32 layout_signaling_present;
  WORD32 defined_layout;
  WORD32 speaker_position[SPEAKER_POS_COUNT_MAX];
} ia_channel_layout_struct;

typedef struct {
  WORD32 downmix_id;
  WORD32 target_channel_count;
  WORD32 target_layout;
  WORD32 downmix_coefficients_present;
  FLOAT32 downmix_coefficient[DOWNMIX_COEFF_COUNT_MAX];
} ia_downmix_instructions_struct;

typedef struct {
  WORD32 gain_seq_idx;
  WORD32 drc_characteristic;
  WORD32 drc_characteristic_present;
  WORD32 drc_characteristic_format_is_cicp;
  WORD32 drc_characteristic_left_index;
  WORD32 drc_characteristic_right_index;
  WORD32 crossover_freq_idx;
  WORD32 start_subband_index;
} ia_gain_params_struct;

typedef struct {
  WORD32 ducking_scaling_flag;
  FLOAT32 ducking_scaling;
  FLOAT32 ducking_scaling_quantized;
} ia_ducking_modifiers_struct;

typedef struct {
  WORD32 target_characteristic_left_present[DRC_BAND_COUNT_MAX];
  WORD32 target_characteristic_left_index[DRC_BAND_COUNT_MAX];
  WORD32 target_characteristic_right_present[DRC_BAND_COUNT_MAX];
  WORD32 target_characteristic_right_index[DRC_BAND_COUNT_MAX];
  WORD32 shape_filter_flag;
  WORD32 shape_filter_idx;
  WORD32 gain_scaling_flag[BAND_COUNT_MAX];
  FLOAT32 attn_scaling[BAND_COUNT_MAX];
  FLOAT32 ampl_scaling[BAND_COUNT_MAX];
  WORD32 gain_offset_flag[BAND_COUNT_MAX];
  FLOAT32 gain_offset[BAND_COUNT_MAX];
} ia_gain_modifiers_struct;

typedef struct {
  WORD32 gain_coding_profile;
  WORD32 gain_interpolation_type;
  WORD32 full_frame;
  WORD32 time_alignment;
  WORD32 time_delt_min_flag;
  WORD32 time_delt_min_val;
  WORD32 band_count;
  WORD32 drc_band_type;
  ia_gain_params_struct gain_params[BAND_COUNT_MAX];

  WORD32 num_gain_max_values;
  ia_tables_struct str_tables;
} ia_gain_set_params_struct;

#define SPLIT_CHARACTERISTIC_NODE_COUNT_MAX 4
typedef struct {
  WORD32 characteristic_format;
  FLOAT32 in_out_ratio;
  FLOAT32 gain;
  FLOAT32 exp;
  WORD32 flip_sign;
  WORD32 characteristic_node_count;
  FLOAT32 node_level[SPLIT_CHARACTERISTIC_NODE_COUNT_MAX + 1];
  FLOAT32 node_gain[SPLIT_CHARACTERISTIC_NODE_COUNT_MAX + 1];
} ia_split_drc_characteristic_struct;

typedef struct {
  WORD32 corner_freq_index;
  WORD32 filter_strength_index;
} ia_shape_filter_params_struct;

typedef struct {
  WORD32 lf_cut_filter_present;
  ia_shape_filter_params_struct str_lf_cut_params;
  WORD32 lf_boost_filter_present;
  ia_shape_filter_params_struct str_lf_boost_params;
  WORD32 hf_cut_filter_present;
  ia_shape_filter_params_struct str_hfCutParams;
  WORD32 hf_boost_filter_present;
  ia_shape_filter_params_struct str_hf_boost_params;
} ia_shape_filter_block_params_struct;

typedef struct {
  WORD32 drc_location;
  WORD32 drc_characteristic;
} ia_drc_coefficients_basic_struct;

typedef struct {
  WORD32 version;
  WORD32 drc_location;
  WORD32 drc_frame_size_present;
  WORD32 drc_frame_size;
  WORD32 gain_set_count;
  ia_gain_set_params_struct gain_set_params[GAIN_SET_COUNT_MAX];
  WORD32 drc_characteristic_left_present;
  WORD32 characteristic_left_count;
  ia_split_drc_characteristic_struct
      str_split_characteristic_left[SPLIT_CHARACTERISTIC_COUNT_MAX];
  WORD32 drc_characteristic_right_present;
  WORD32 characteristic_right_count;
  ia_split_drc_characteristic_struct
      str_split_characteristic_right[SPLIT_CHARACTERISTIC_COUNT_MAX];
  WORD32 shape_filters_present;
  WORD32 shape_num_filter;
  ia_shape_filter_block_params_struct
      str_shape_filter_block_params[SHAPE_FILTER_COUNT_MAX + 1];
  WORD32 gain_sequence_count;
  WORD32 gain_set_params_index_for_gain_sequence[SEQUENCE_COUNT_MAX];
  WORD32 gain_set_count_plus;

} ia_uni_drc_coeffs_struct;

typedef struct {
  WORD32 drc_set_id;
  WORD32 drc_location;
  WORD32 dwnmix_id_count;
  WORD32 downmix_id[DOWNMIX_ID_COUNT_MAX];
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
  WORD32 requires_eq;
  WORD32 drc_apply_to_dwnmix;
  WORD32 drc_location;
  WORD32 dwnmix_id_count;
  WORD32 downmix_id[DOWNMIX_ID_COUNT_MAX];
  WORD32 depends_on_drc_set_present;
  WORD32 depends_on_drc_set;
  WORD32 no_independent_use;
  WORD32 drc_set_effect;
  WORD32 gain_set_index[MAX_CHANNEL_COUNT];
  ia_gain_modifiers_struct
      str_gain_modifiers_of_ch_group[CHANNEL_GROUP_COUNT_MAX];
  ia_ducking_modifiers_struct
      str_ducking_modifiers_for_channel[MAX_CHANNEL_COUNT];
  WORD32 limiter_peak_target_present;
  FLOAT32 limiter_peak_target;
  WORD32 drc_set_target_loudness_present;
  WORD32 drc_set_target_loudness_value_upper;
  WORD32 drc_set_target_loudness_value_lower_present;
  WORD32 drc_set_target_loudness_value_lower;

  WORD32 audio_num_chan;
  WORD32 num_drc_ch_groups;
  WORD32 gain_set_index_for_channel_group[CHANNEL_GROUP_COUNT_MAX];
  WORD32 band_count_of_ch_group[CHANNEL_GROUP_COUNT_MAX];
  WORD32 gain_interpolation_type_for_channel_group[CHANNEL_GROUP_COUNT_MAX];
  WORD32 time_delta_min_for_channel_group[CHANNEL_GROUP_COUNT_MAX];
  WORD32 time_alignment_for_channel_group[CHANNEL_GROUP_COUNT_MAX];
  ia_ducking_modifiers_struct
      str_ducking_modifiers_for_channel_group[CHANNEL_GROUP_COUNT_MAX];
  WORD32 channel_group_of_ch[MAX_CHANNEL_COUNT];
  WORD32 num_chan_per_ch_group[CHANNEL_GROUP_COUNT_MAX];
  WORD32 gain_element_count;
  WORD32 multiband_audio_sig_count;
  WORD32 ch_group_parametric_drc_flag[CHANNEL_GROUP_COUNT_MAX];
  WORD32 gain_set_idx_of_ch_group_parametric_drc[CHANNEL_GROUP_COUNT_MAX];
  WORD32 parametric_drc_look_ahead_samples[CHANNEL_GROUP_COUNT_MAX];
  WORD32 parametric_drc_look_ahead_samples_max;
#ifdef LOUDNESS_LEVELING_SUPPORT
  WORD32 leveling_present;
#endif
} ia_drc_instructions_struct;

typedef struct {
  WORD32 method_def;
  FLOAT32 method_val;
  WORD32 measurement_system;
  WORD32 reliability; /* Parsed but unused */
} ia_loudness_measure_struct;

typedef struct {
  WORD32 drc_set_id;
  WORD32 eq_set_id;
  WORD32 downmix_id;
  WORD32 sample_peak_level_present;
  FLOAT32 sample_peak_level;
  WORD32 true_peak_level_present;
  FLOAT32 true_peak_level;
  WORD32 true_peak_level_measurement_system; /* Parsed but unused */
  WORD32 true_peak_level_reliability;        /* Parsed but unused */
  WORD32 measurement_count;
  WORD32 anchor_loudness_present;
  WORD32 expert_loudness_present;
  ia_loudness_measure_struct loudness_measure[MEASUREMENT_COUNT_MAX];
} ia_loudness_info_struct;

typedef struct {
  WORD32 loud_eq_set_id;
  WORD32 drc_location;
  WORD32 dwnmix_id_count;
  WORD32 downmix_id[DOWNMIX_ID_COUNT_MAX];
  WORD32 drc_set_id_count;
  WORD32 drc_set_id[DRC_SET_ID_COUNT_MAX];
  WORD32 eq_set_id_count;
  WORD32 eq_set_id[EQ_SET_ID_COUNT_MAX];
  WORD32 loudness_after_drc;                            /* Parsed but unused */
  WORD32 loudness_after_eq;                             /* Parsed but unused */
  WORD32 loud_eq_gain_sequence_count;                   /* Parsed but unused */
  WORD32 gain_seq_idx[LOUD_EQ_GAIN_SEQUENCE_COUNT_MAX]; /* Parsed but unused */
  WORD32 drc_characteristic_format_is_cicp
      [LOUD_EQ_GAIN_SEQUENCE_COUNT_MAX]; /* Parsed but unused */
  WORD32 drc_characteristic[LOUD_EQ_GAIN_SEQUENCE_COUNT_MAX]; /* Parsed but
                                                                 unused */
  WORD32 drc_characteristic_left_index
      [LOUD_EQ_GAIN_SEQUENCE_COUNT_MAX]; /* Parsed but unused */
  WORD32 drc_characteristic_right_index
      [LOUD_EQ_GAIN_SEQUENCE_COUNT_MAX]; /* Parsed but unused */
  WORD32 frequency_range_index[LOUD_EQ_GAIN_SEQUENCE_COUNT_MAX]; /* Parsed but
                                                                    unused */
  FLOAT32
  loud_eq_scaling[LOUD_EQ_GAIN_SEQUENCE_COUNT_MAX]; /* Parsed but unused */
  FLOAT32
  loud_eq_offset[LOUD_EQ_GAIN_SEQUENCE_COUNT_MAX]; /* Parsed but unused */
} ia_loud_eq_instructions_struct;

typedef struct {
  WORD32 filt_ele_idx;
  WORD32 filt_ele_gain_flag;
  FLOAT32 filt_ele_gain;
} ia_filt_ele_struct;

typedef struct {
  WORD32 filter_element_count;
  ia_filt_ele_struct str_filter_element[FILTER_ELEMENT_COUNT_MAX];
} ia_filt_block_struct;

typedef struct {
  WORD32 eq_filter_format;
  WORD32 bs_real_zero_radius_one_count;
  WORD32 real_zero_count;
  WORD32 generic_zero_count;
  WORD32 real_pole_count;
  WORD32 cmplx_pole_count;
  WORD32 zero_sign[REAL_ZERO_RADIUS_ONE_COUNT_MAX];
  FLOAT32 real_zero_radius[REAL_ZERO_COUNT_MAX];
  FLOAT32 generic_zero_radius[COMPLEX_ZERO_COUNT_MAX];
  FLOAT32 generic_zero_angle[COMPLEX_ZERO_COUNT_MAX];
  FLOAT32 real_pole_radius[REAL_POLE_COUNT_MAX];
  FLOAT32 complex_pole_radius[COMPLEX_POLE_COUNT_MAX];
  FLOAT32 complex_pole_angle[COMPLEX_POLE_COUNT_MAX];
  WORD32 fir_filt_order;
  WORD32 fir_symmetry;
  FLOAT32 fir_coeff[FIR_ORDER_MAX / 2];
} ia_unique_td_filt_element;

typedef struct {
  WORD32 num_eq_nodes;
  FLOAT32 eq_slope[EQ_NODE_COUNT_MAX];
  WORD32 eq_freq_delta[EQ_NODE_COUNT_MAX];
  FLOAT32 eq_gain_initial;
  FLOAT32 eq_gain_delta[EQ_NODE_COUNT_MAX];
} ia_eq_subband_gain_spline_struct;

typedef struct {
  FLOAT32 eq_subband_gain[EQ_SUBBAND_GAIN_COUNT_MAX];
} ia_eq_subband_gain_vector;

typedef struct {
  WORD32 eq_delay_max_present;
  WORD32 eq_delay_max;
  WORD32 unique_filter_block_count;
  ia_filt_block_struct str_filter_block[FILTER_BLOCK_COUNT_MAX];
  WORD32 unique_td_filter_element_count;
  ia_unique_td_filt_element unique_td_filt_ele[FILTER_ELEMENT_COUNT_MAX];
  WORD32 unique_eq_subband_gains_count;
  WORD32 eq_subband_gain_representation;
  WORD32 eq_subband_gain_format;
  WORD32 eq_subband_gain_count;
  ia_eq_subband_gain_spline_struct
      str_eq_subband_gain_spline[UNIQUE_SUBBAND_GAIN_COUNT_MAX];
  ia_eq_subband_gain_vector
      str_eq_subband_gain_vector[UNIQUE_SUBBAND_GAIN_COUNT_MAX];
} ia_eq_coeff_struct;

typedef struct {
  WORD32 filter_block_count;
  WORD32 filter_block_index[EQ_FILTER_BLOCK_COUNT_MAX];
} ia_filter_block_refs_struct;

typedef struct {
  WORD32 eq_cascade_gain_present[EQ_CHANNEL_GROUP_COUNT_MAX];
  FLOAT32 eq_cascade_gain[EQ_CHANNEL_GROUP_COUNT_MAX];
  ia_filter_block_refs_struct str_filter_block_refs[EQ_CHANNEL_GROUP_COUNT_MAX];
  WORD32 eq_phase_alignment_present;
  WORD32 eq_phase_alignment[EQ_CHANNEL_GROUP_COUNT_MAX]
                           [EQ_CHANNEL_GROUP_COUNT_MAX];
} ia_td_filter_cascade_struct;

typedef struct {
  WORD32 eq_set_id;
  WORD32 eq_set_complexity_level;
  WORD32 dwnmix_id_count;
  WORD32 downmix_id[DOWNMIX_ID_COUNT_MAX];
  WORD32 eq_apply_to_downmix;
  WORD32 drc_set_id_count;
  WORD32 drc_set_id[DRC_SET_ID_COUNT_MAX];
  WORD32 eq_set_purpose;
  WORD32 depends_on_eq_set_present;
  WORD32 depends_on_eq_set;
  WORD32 no_independent_eq_use;
  WORD32 eq_channel_count;
  WORD32 eq_ch_group_count;
  WORD32 eq_ch_group_of_channel[MAX_CHANNEL_COUNT];
  WORD32 td_filter_cascade_present;
  ia_td_filter_cascade_struct str_td_filter_cascade;
  WORD32 subband_gains_present;
  WORD32 subband_gains_index[EQ_CHANNEL_GROUP_COUNT_MAX];
  WORD32 eq_transition_duration_present;
  WORD32 eq_transition_duration;
} ia_eq_instructions_struct;

typedef struct {
  WORD32 drc_config_ext_type[EXT_COUNT_MAX];
  WORD32 ext_bit_size[EXT_COUNT_MAX - 1];

  WORD32 parametric_drc_present;
  ia_drc_coeff_parametric_drc_struct str_drc_coeff_param_drc;
  WORD32 parametric_drc_instructions_count;
  ia_parametric_drc_instructions_struct
      str_parametric_drc_instructions[PARAM_DRC_INSTRUCTIONS_COUNT_MAX];
  WORD32 drc_extension_v1_present;
  WORD32 loud_eq_instructions_flag;
  WORD32 loud_eq_instructions_count;
  ia_loud_eq_instructions_struct
      loud_eq_instructions[LOUD_EQ_INSTRUCTIONS_COUNT_MAX];
  WORD32 eq_flag;
  ia_eq_coeff_struct str_eq_coeff;
  WORD32 eq_instructions_count;
  ia_eq_instructions_struct str_eq_instructions[EQ_INSTRUCTIONS_COUNT_MAX];
} ia_drc_config_ext;

typedef struct ia_drc_config {
  WORD32 sample_rate_present;
  WORD32 sampling_rate;
  WORD32 dwnmix_instructions_count;
  WORD32 drc_coefficients_drc_count;
  WORD32 drc_instructions_uni_drc_count;
  WORD32 drc_instructions_count_plus;
  WORD32 drc_description_basic_present;
  WORD32 drc_coefficients_basic_count;
  WORD32 drc_instructions_basic_count;
  WORD32 drc_config_ext_present;
  WORD32 apply_drc;
  ia_drc_config_ext str_drc_config_ext;
  ia_drc_coefficients_basic_struct
      str_drc_coefficients_basic[DRC_COEFF_COUNT_MAX];
  ia_drc_instructions_basic_struct
      str_drc_instructions_basic[DRC_INSTRUCTIONS_COUNT_MAX];
  ia_uni_drc_coeffs_struct
      str_p_loc_drc_coefficients_uni_drc[DRC_COEFF_COUNT_MAX];
  ia_drc_instructions_struct
      str_drc_instruction_str[DRC_INSTRUCTIONS_COUNT_MAX];
  ia_channel_layout_struct channel_layout;
  ia_downmix_instructions_struct
      dwnmix_instructions[DOWNMIX_INSTRUCTION_COUNT_MAX];
  WORD32 is_config_changed;
  WORD32 ln_gain_changed;
} ia_drc_config;

typedef struct {
  WORD32 loudness_info_set_ext_type[EXT_COUNT_MAX];
  WORD32 ext_bit_size[EXT_COUNT_MAX - 1];
} ia_loudness_info_set_ext_struct;

typedef struct ia_drc_loudness_info_set_struct {
  WORD32 loudness_info_album_count;
  WORD32 loudness_info_count;
  WORD32 loudness_info_set_ext_present;
  ia_loudness_info_struct str_loudness_info_album[LOUDNESS_INFO_COUNT_MAX];
  ia_loudness_info_struct loudness_info[LOUDNESS_INFO_COUNT_MAX];
  ia_loudness_info_set_ext_struct str_loudness_info_set_ext;
} ia_drc_loudness_info_set_struct;

typedef struct {
  FLOAT32 loc_db_gain;
  FLOAT32 slope;
  WORD32 time;
} ia_node_struct;

typedef struct {
  WORD32 drc_gain_coding_mode;
  WORD32 num_nodes;
  ia_node_struct str_node[NODE_COUNT_MAX];
} ia_spline_nodes_struct;

typedef struct {
  ia_spline_nodes_struct str_spline_nodes[1];
} ia_drc_gain_sequence_struct;

typedef struct {
  WORD32 uni_drc_gain_ext_type[EXT_COUNT_MAX];
  WORD32 ext_bit_size[EXT_COUNT_MAX - 1];
} ia_uni_drc_gain_ext_struct;

typedef struct ia_drc_gain_struct {
  WORD32 num_drc_gain_sequences;
  ia_drc_gain_sequence_struct drc_gain_sequence[SEQUENCE_COUNT_MAX];
  WORD32 uni_drc_gain_ext_flag;
  ia_uni_drc_gain_ext_struct uni_drc_gain_ext;
} ia_drc_gain_struct;

typedef struct {
  WORD32 delta_tmin_default;
  WORD32 drc_frame_size;
  WORD32 num_gain_values_max_default;
  WORD32 delay_mode;
  WORD32 lfe_channel_map_count;
  WORD32 lfe_channel_map[MAX_CHANNEL_COUNT];
} ia_drc_params_bs_dec_struct;

typedef struct ia_drc_bits_dec_struct {
  ia_tables_struct tables_default;
  ia_drc_params_bs_dec_struct ia_drc_params_struct;

} ia_drc_bits_dec_struct;

#ifdef __cplusplus
}
#endif
#endif
