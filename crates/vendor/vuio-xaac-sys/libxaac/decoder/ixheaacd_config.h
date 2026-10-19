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
#ifndef IXHEAACD_CONFIG_H
#define IXHEAACD_CONFIG_H

#define USAC_MAX_ELEMENTS (16)

#define USAC_MAX_CONFIG_EXTENSIONS (16)

#define ID_USAC_SCE 0
#define ID_USAC_CPE 1
#define ID_USAC_LFE 2
#define ID_USAC_EXT 3
#define ID_USAC_INVALID 0xFF

#define USAC_SBR_RATIO_NO_SBR 0
#define USAC_SBR_RATIO_INDEX_2_1 1
#define USAC_SBR_RATIO_INDEX_8_3 2
#define USAC_SBR_RATIO_INDEX_4_1 3

#define USAC_OUT_FRAMELENGTH_768 768
#define USAC_OUT_FRAMELENGTH_1024 1024
#define USAC_OUT_FRAMELENGTH_2048 2048
#define USAC_OUT_FRAMELENGTH_4096 4096

#define MAX_CORE_SBR_FRAME_LEN_IDX (4)
#define IA_MAX_INP_BUFFER_SIZE (8 * 1024 + 11)

#define ID_EXT_ELE_FILL 0
#define ID_EXT_ELE_MPEGS 1
#define ID_EXT_ELE_SAOC 2
#define ID_EXT_ELE_AUDIOPREROLL 3
#define ID_EXT_ELE_UNI_DRC 4

#define ID_CONFIG_EXT_FILL 0
#define ID_CONFIG_EXT_LOUDNESS_INFO (2)

#define MAX_PARAMETER_BANDS (28)
#define MAX_NUM_OTT (5)
#define MAX_NUM_TTT (1)
#define MAX_INPUT_CHANNELS (2)
#define MAX_RESIDUAL_CHANNELS (3)
#define MAX_OUTPUT_CHANNELS (7)
#define MAX_NUM_EXT_TYPES (8)
#define MAX_M_INPUT (2)
#define MAX_M_OUTPUT (2)
#define QMF_BANDS_TO_HYBRID (3)
#define MAX_HYBRID_ONLY_BANDS_PER_QMF (8)
#define MAX_ARBITRARY_TREE_LEVELS (2)
#define MAX_OUTPUT_CHANNELS_AT \
  (MAX_OUTPUT_CHANNELS * (1 << MAX_ARBITRARY_TREE_LEVELS))
#define MAX_ARBITRARY_TREE_INDEX ((1 << (MAX_ARBITRARY_TREE_LEVELS + 1)) - 1)

#define MAX_RES_SAMP_FREQ_IDX 11

#if defined(__clang__)
#define ATTR_NO_SANITIZE_INTEGER __attribute__((no_sanitize("integer")))
#else
#define ATTR_NO_SANITIZE_INTEGER
#endif

typedef UWORD8 UINT8;
typedef UWORD32 UINT32;

typedef struct {
  UINT32 harmonic_sbr;
  UINT32 bs_inter_tes;
  UINT32 bs_pvc;
  WORD16 dflt_start_freq;
  WORD16 dflt_stop_freq;
  WORD16 dflt_header_extra1;
  WORD16 dflt_header_extra2;
  WORD16 dflt_freq_scale;
  WORD16 dflt_alter_scale;
  WORD16 dflt_noise_bands;
  WORD16 dflt_limiter_bands;
  WORD16 dflt_limiter_gains;
  WORD16 dflt_interpol_freq;
  WORD16 dflt_smoothing_mode;
} ia_usac_dec_sbr_config_struct;

typedef struct {
  UINT32 bs_freq_res;
  UINT32 bs_fixed_gain_dmx;
  UINT32 bs_temp_shape_config;
  UINT32 bs_decorr_config;
  UINT32 bs_high_rate_mode;
  UINT32 bs_phase_coding;
  UINT32 bs_ott_bands_phase_present;
  UINT32 bs_ott_bands_phase;
  UINT32 bs_residual_bands;
  UINT32 bs_pseudo_lr;
  UINT32 bs_env_quant_mode;
  UINT32 ldmps_present_flag;

  UINT32 bs_sampling_freq_index;
  UINT32 bs_fampling_frequency;
  UINT32 bs_frame_length;
  UINT32 bs_tree_config;
  UINT32 bs_quant_mode;
  UINT32 bs_one_icc;
  UINT32 bs_arbitrary_downmix;
  UINT32 bs_residual_coding;
  UINT32 bs_fixed_gain_sur;
  UINT32 bs_fixed_gain_LFE;
  UINT32 bs_matrix_mode;
  UINT32 bs_3D_audio_mode;
  UINT32 bs_3D_audio_HRTF_set;
  UINT32 bs_HRTF_freq_res;
  UINT32 HRTF_num_band;
  UINT32 HRTF_num_phase;  // new
  UINT32 bs_HRTF_num_chan;
  UINT32 bs_HRTF_asymmetric;
  UINT32 bs_HRTF_level_left[MAX_OUTPUT_CHANNELS][MAX_PARAMETER_BANDS];
  UINT32 bs_HRTF_level_right[MAX_OUTPUT_CHANNELS][MAX_PARAMETER_BANDS];
  UINT32 bs_HRTF_phase[MAX_OUTPUT_CHANNELS];
  UINT32 bs_HRTF_phase_LR[MAX_OUTPUT_CHANNELS][MAX_PARAMETER_BANDS];
  UINT32 bs_HRTF_icc[MAX_OUTPUT_CHANNELS];
  UINT32 bs_HRTF_icc_LR[MAX_OUTPUT_CHANNELS][MAX_PARAMETER_BANDS];
  UINT32 bs_ott_bands[MAX_NUM_OTT];
  UINT32 bs_ttt_dual_mode[MAX_NUM_TTT];
  UINT32 bs_ttt_mode_low[MAX_NUM_TTT];
  UINT32 bs_ttt_mode_high[MAX_NUM_TTT];
  UINT32 bs_ttt_bands_low[MAX_NUM_TTT];
  UINT32 bs_ttt_bands_high[MAX_NUM_TTT];

  UINT32 bs_sac_ext_type[MAX_NUM_EXT_TYPES];
  UINT32 sac_ext_cnt;

  UINT32 bs_residual_present[MAX_RESIDUAL_CHANNELS];
  UINT32 bs_residual_sampling_freq_index;
  UINT32 bs_residual_frames_per_spatial_frame;

  UINT32 bs_residual_bands_ld_mps[MAX_RESIDUAL_CHANNELS];

  UINT32 bs_arbitrary_downmix_residual_sampling_freq_index;
  UINT32 bs_arbitrary_downmix_residual_frames_per_spatial_frame;
  WORD32 bs_arbitrary_downmix_residual_bands;

  UINT32 num_out_chan_AT;
  UINT32 num_ott_boxes_AT;
  UINT32 bs_output_channel_pos_AT[MAX_OUTPUT_CHANNELS_AT];
  UINT32 bs_ott_box_present_AT[MAX_OUTPUT_CHANNELS][MAX_ARBITRARY_TREE_INDEX];
  UINT32 bs_ott_default_cld_AT[MAX_OUTPUT_CHANNELS * MAX_ARBITRARY_TREE_INDEX];
  UINT32 bs_ott_mode_lfe_AT[MAX_OUTPUT_CHANNELS * MAX_ARBITRARY_TREE_INDEX];
  UINT32 bs_ott_bands_AT[MAX_OUTPUT_CHANNELS * MAX_ARBITRARY_TREE_INDEX];

  WORD32 num_ott_boxes;
  WORD32 num_ttt_boxes;
  WORD32 num_input_channels;
  WORD32 num_output_channels;
  WORD32 ott_mode_lfe[MAX_NUM_OTT];  // new
  WORD32 no_ldsbr_present;
} ia_usac_dec_mps_config_struct;

#define BS_OUTPUT_CHANNEL_POS_NA -1   /* n/a                                */
#define BS_OUTPUT_CHANNEL_POS_L 0     /* Left Front                          */
#define BS_OUTPUT_CHANNEL_POS_R 1     /* Right Front                         */
#define BS_OUTPUT_CHANNEL_POS_C 2     /* Center Front                        */
#define BS_OUTPUT_CHANNEL_POS_LFE 3   /* Low Frequency Enhancement           */
#define BS_OUTPUT_CHANNEL_POS_LS 4    /* Left Surround                       */
#define BS_OUTPUT_CHANNEL_POS_RS 5    /* Right Surround                      */
#define BS_OUTPUT_CHANNEL_POS_LC 6    /* Left Front Center                   */
#define BS_OUTPUT_CHANNEL_POS_RC 7    /* Right Front Center                  */
#define BS_OUTPUT_CHANNEL_POS_LSR 8   /* Rear Surround Left                  */
#define BS_OUTPUT_CHANNEL_POS_RSR 9   /* Rear Surround Right                 */
#define BS_OUTPUT_CHANNEL_POS_CS 10   /* Rear Center                         */
#define BS_OUTPUT_CHANNEL_POS_LSD 11  /* Left Surround Direct                */
#define BS_OUTPUT_CHANNEL_POS_RSD 12  /* Right Surround Direct               */
#define BS_OUTPUT_CHANNEL_POS_LSS 13  /* Left Side Surround                  */
#define BS_OUTPUT_CHANNEL_POS_RSS 14  /* Right Side Surround                 */
#define BS_OUTPUT_CHANNEL_POS_LW 15   /* Left Wide Front                     */
#define BS_OUTPUT_CHANNEL_POS_RW 16   /* Right Wide Front                    */
#define BS_OUTPUT_CHANNEL_POS_LV 17   /* Left Front Vertical Height          */
#define BS_OUTPUT_CHANNEL_POS_RV 18   /* Right Front Vertical Height         */
#define BS_OUTPUT_CHANNEL_POS_CV 19   /* Center Front Vertical Height        */
#define BS_OUTPUT_CHANNEL_POS_LVR 20  /* Left Surround Vertical Height Rear  */
#define BS_OUTPUT_CHANNEL_POS_RVR 21  /* Right Surround Vertical Height Rear */
#define BS_OUTPUT_CHANNEL_POS_CVR 22  /* Center Vertical Height Rear         */
#define BS_OUTPUT_CHANNEL_POS_LVSS 23 /* Left Vertical Height Side Surround */
#define BS_OUTPUT_CHANNEL_POS_RVSS                                             \
  24                                /* Right Vertical Height Side Surround \ \ \
                                       */
#define BS_OUTPUT_CHANNEL_POS_TS 25 /* Top Center Surround                 */
#define BS_OUTPUT_CHANNEL_POS_LFE2 26 /* Low Frequency Enhancement 2 */
#define BS_OUTPUT_CHANNEL_POS_LB 27   /* Left Front Vertical Bottom          */
#define BS_OUTPUT_CHANNEL_POS_RB 28   /* Right Front Vertical Bottom         */
#define BS_OUTPUT_CHANNEL_POS_CB 29   /* Center Front Vertical Bottom        */
#define BS_OUTPUT_CHANNEL_POS_LVS 30  /* Left Vertical Height Surround       */
#define BS_OUTPUT_CHANNEL_POS_RVS 31  /* Right Vertical Height Surround      */

#define BS_MAX_NUM_OUT_CHANNELS (255)

#define EXT_COUNT_MAX (2)
#define MAX_CHANNEL_COUNT (128)
#define SEQUENCE_COUNT_MAX (24)
#define PARAM_DRC_TYPE_FF_NODE_COUNT_MAX (9)
#define PARAM_DRC_INSTRUCTIONS_COUNT_MAX (8)
#define DOWNMIX_ID_COUNT_MAX (8)
#define DRC_SET_ID_COUNT_MAX (16)
#define EQ_SET_ID_COUNT_MAX (8)
#define LOUD_EQ_GAIN_SEQUENCE_COUNT_MAX (4)
#define LOUD_EQ_INSTRUCTIONS_COUNT_MAX (8)
#define FILTER_ELEMENT_COUNT_MAX (16)
#define FILTER_BLOCK_COUNT_MAX (16)
#define REAL_ZERO_RADIUS_ONE_COUNT_MAX (14)
#define REAL_ZERO_COUNT_MAX (64)
#define COMPLEX_ZERO_COUNT_MAX (64)
#define REAL_POLE_COUNT_MAX (16)
#define COMPLEX_POLE_COUNT_MAX (16)
#define FIR_ORDER_MAX (128)
#define EQ_NODE_COUNT_MAX (33)
#define UNIQUE_SUBBAND_GAIN_COUNT_MAX (16)
#define EQ_SUBBAND_GAIN_COUNT_MAX (135)
#define EQ_CHANNEL_GROUP_COUNT_MAX (4)
#define EQ_FILTER_BLOCK_COUNT_MAX (4)
#define EQ_INSTRUCTIONS_COUNT_MAX (8)
#define DRC_COEFF_COUNT_MAX (8)
#define DOWNMIX_INSTRUCTION_COUNT_MAX (16)
#define DRC_INSTRUCTIONS_COUNT_MAX (DOWNMIX_INSTRUCTION_COUNT_MAX + 20)
#define BAND_COUNT_MAX (8)

#define N_DELTA_TIME_CODE_TABLE_ENTRIES_MAX (512 + 14)
#define GAIN_SET_COUNT_MAX SEQUENCE_COUNT_MAX
#define SPLIT_CHARACTERISTIC_NODE_COUNT_MAX (4)
#define SPLIT_CHARACTERISTIC_COUNT_MAX (8)
#define SHAPE_FILTER_COUNT_MAX (8)
#define CHANNEL_GROUP_COUNT_MAX SEQUENCE_COUNT_MAX
#define DRC_BAND_COUNT_MAX BAND_COUNT_MAX
#define SPEAKER_POS_COUNT_MAX (128)
#define DOWNMIX_COEFF_COUNT_MAX (32 * 32)
#define MAX_AUDIO_PREROLLS 3

typedef struct {
  UINT32 tw_mdct;
  UINT32 noise_filling;
  UINT32 stereo_config_index;

  UINT32 usac_ext_eleme_def_len;
  UINT32 usac_ext_elem_pld_frag;

  ia_usac_dec_sbr_config_struct str_usac_sbr_config;
  ia_usac_dec_mps_config_struct str_usac_mps212_config;

} ia_usac_dec_element_config_struct;

typedef struct {
  UWORD32 num_elements;
  UWORD32 num_config_extensions;
  UWORD32 usac_element_type[USAC_MAX_ELEMENTS];
  ia_usac_dec_element_config_struct str_usac_element_config[USAC_MAX_ELEMENTS];

  WORD32 usac_cfg_ext_info_present[USAC_MAX_CONFIG_EXTENSIONS];
  WORD32 usac_ext_ele_payload_present[USAC_MAX_ELEMENTS];
  WORD32 usac_cfg_ext_info_len[USAC_MAX_CONFIG_EXTENSIONS];
  WORD32 usac_ext_ele_payload_len[USAC_MAX_ELEMENTS];
  WORD32 usac_ext_gain_payload_len[MAX_AUDIO_PREROLLS + 2];
  UWORD8 usac_cfg_ext_info_buf[USAC_MAX_CONFIG_EXTENSIONS][768];
  UWORD8 usac_ext_ele_payload_buf[USAC_MAX_ELEMENTS][768];
  UWORD8 usac_ext_gain_payload_buf[MAX_AUDIO_PREROLLS * 768];
  UWORD32 preroll_bytes[MAX_AUDIO_PREROLLS +
                        2];  // Contain the number of bytes in each preroll
  WORD32 preroll_counter;    // count the number of prerolls in a frame.

  WORD32 preroll_flag;

} ia_usac_decoder_config_struct;

typedef struct {
  UINT32 usac_sampling_frequency_index;
  UINT32 usac_sampling_frequency;
  UINT32 core_sbr_framelength_index;
  UINT32 channel_configuration_index;

  UINT32 num_out_channels;
  UINT32 output_channel_pos[BS_MAX_NUM_OUT_CHANNELS];
  ia_usac_decoder_config_struct str_usac_dec_config;

} ia_usac_config_struct;

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
  WORD32 loud_eq_set_id;
  WORD32 drc_location;
  WORD32 dwnmix_id_count;
  WORD32 downmix_id[DOWNMIX_ID_COUNT_MAX];
  WORD32 drc_set_id_count;
  WORD32 drc_set_id[DRC_SET_ID_COUNT_MAX];
  WORD32 eq_set_id_count;
  WORD32 eq_set_id[EQ_SET_ID_COUNT_MAX];
  WORD32 loudness_after_drc;
  WORD32 loudness_after_eq;
  WORD32 loud_eq_gain_sequence_count;
  WORD32 gain_seq_idx[LOUD_EQ_GAIN_SEQUENCE_COUNT_MAX];
  WORD32 drc_characteristic_format_is_cicp[LOUD_EQ_GAIN_SEQUENCE_COUNT_MAX];
  WORD32 drc_characteristic[LOUD_EQ_GAIN_SEQUENCE_COUNT_MAX];
  WORD32 drc_characteristic_left_index[LOUD_EQ_GAIN_SEQUENCE_COUNT_MAX];
  WORD32 drc_characteristic_right_index[LOUD_EQ_GAIN_SEQUENCE_COUNT_MAX];
  WORD32 frequency_range_index[LOUD_EQ_GAIN_SEQUENCE_COUNT_MAX];
  FLOAT32 loud_eq_scaling[LOUD_EQ_GAIN_SEQUENCE_COUNT_MAX];
  FLOAT32 loud_eq_offset[LOUD_EQ_GAIN_SEQUENCE_COUNT_MAX];
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

typedef struct {
  WORD32 drc_location;
  WORD32 drc_characteristic;
} ia_drc_coefficients_basic_struct;

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
  WORD32 size;
  WORD32 code;
  WORD32 value;
} ia_delta_time_code_table_entry_struct;
typedef struct {
  ia_delta_time_code_table_entry_struct
      delta_time_code_table[N_DELTA_TIME_CODE_TABLE_ENTRIES_MAX];
} ia_tables_struct;

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
  ia_shape_filter_params_struct str_hf_cut_params;
  WORD32 hf_boost_filter_present;
  ia_shape_filter_params_struct str_hf_boost_params;
} ia_shape_filter_block_params_struct;

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
  WORD32 ducking_scaling_flag;
  FLOAT32 ducking_scaling;
  FLOAT32 ducking_scaling_quantized;
} ia_ducking_modifiers_struct;

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
} ia_drc_instructions_struct;

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
} ia_drc_config;

VOID ixheaacd_conf_default(ia_usac_config_struct *pstr_usac_conf);

UWORD32 ixheaacd_sbr_ratio(UWORD32 core_sbr_frame_len_idx);

UWORD32 ixheaacd_sbr_params(UWORD32 core_sbr_frame_len_idx,
                            WORD32 *output_frame_length, WORD32 *block_size,
                            WORD32 *output_samples, WORD32 *sampling_freq,
                            UWORD32 *sample_freq_indx);

WORD32 ixheaacd_config(ia_bit_buf_struct *bit_buff,
                       ia_usac_config_struct *pstr_usac_conf, UINT32 *chan, WORD32 ec_flag);

#endif /* IXHEAACD_CONFIG_H */
