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
#ifndef IMPD_DRC_ROM_H
#define IMPD_DRC_ROM_H
#define MAX_NUM_QMF_BANDS 128

#define NUM_GAIN_TBL_PROF_0_1_ENTRIES 25
#define NUM_GAIN_TBL_PROF_2_ENTRIES 49
#define NUM_SLOPE_TBL_ENTRIES 15

extern const FLOAT32 samp_rate_tbl[13][12];

extern const ia_delta_gain_code_table_struct
    ia_drc_gain_tbls_prof_0_1[NUM_GAIN_TBL_PROF_0_1_ENTRIES];

extern const ia_delta_gain_code_table_struct
    ia_drc_gain_tbls_prof_2[NUM_GAIN_TBL_PROF_2_ENTRIES];

extern const FLOAT32 channel_weight[];

extern const FLOAT32 dwnmix_coeff_v1[];
extern const FLOAT32 eq_slope_tbl[];

extern const FLOAT32 eq_gain_delta_tbl[];

extern const FLOAT32 zero_pole_radius_tbl[];
extern const FLOAT32 zero_pole_angle_tbl[];

extern const FLOAT32 shape_filt_lf_y1_bound_tbl[][3];
extern const FLOAT32 shape_filt_hf_y1_bound_tbl[][3];

extern const FLOAT32 shape_filt_lf_gain_offset_tbl[][3];

extern const FLOAT32 shape_filt_hf_gain_offset_tbl[][3];

extern const FLOAT32 shape_filt_lf_radius_tbl[];

extern const FLOAT32 shape_filt_hf_radius_tbl[];

extern const FLOAT32 shape_filt_cutoff_freq_norm_hf_tbl[];

extern const ia_cicp_sigmoid_characteristic_param_struct
    pstr_cicp_sigmoid_characteristic_param[];

extern const ia_slope_code_table_struct
    slope_code_tbl_entries_by_size[NUM_SLOPE_TBL_ENTRIES];

extern const FLOAT32 dwnmix_coeff[];

extern const FLOAT32 dwnmix_coeff_lfe[];

extern const WORD32 drc_characteristic_order_default[][3];

extern const WORD32 measurement_system_default_tbl[];

extern const WORD32 measurement_system_bs1770_3_tbl[];
extern const WORD32 measurement_system_user_tbl[];
extern const WORD32 measurement_system_expert_tbl[];
extern const WORD32 measurement_system_rms_a_tbl[];
extern const WORD32 measurement_system_rms_b_tbl[];
extern const WORD32 measurement_system_rms_c_tbl[];
extern const WORD32 measurement_system_rms_d_tbl[];
extern const WORD32 measurement_system_rms_e_tbl[];
extern const WORD32 measurement_method_prog_loudness_tbl[];
extern const WORD32 measurement_method_peak_loudness_tbl[];

#define MAX_NUM_DOWNMIX_ID_REQUESTS_LOCAL 3

typedef struct {
  WORD32 target_config_request_type;
  WORD32 num_downmix_id_requests;
  WORD32 requested_dwnmix_id[MAX_NUM_DOWNMIX_ID_REQUESTS_LOCAL];
  WORD32 requested_target_layout;
  WORD32 requested_target_ch_count;
} ia_loc_sys_interface_struct;

extern const ia_loc_sys_interface_struct loc_sys_interface[];

typedef struct {
  WORD32 loudness_normalization_on;
  FLOAT32 target_loudness;
} ia_loc_loudness_norm_ctrl_interface_struct;

extern const ia_loc_loudness_norm_ctrl_interface_struct
    loc_loudness_norm_ctrl_interface[];

typedef struct {
  WORD32 album_mode;
  WORD32 peak_limiter;
  WORD32 loudness_deviation_max;
  WORD32 loudness_measurement_method;
  WORD32 loudness_measurement_system;
  WORD32 loudness_measurement_pre_proc;
  WORD32 device_cut_off_frequency;
  FLOAT32 loudness_norm_gain_db_max;
  FLOAT32 loudness_norm_gain_modification_db;
  FLOAT32 output_peak_level_max;
} ia_loc_loudness_norm_param_interface_struct;

extern const ia_loc_loudness_norm_param_interface_struct
    loc_loudness_norm_param_interface[];

#define MAX_NUM_DRC_FEATURE_REQUESTS_LOCAL 3
typedef struct {
  WORD32 dynamic_range_control_on;
  WORD32 num_drc_feature_requests;
  WORD32 drc_feature_req_type[MAX_NUM_DRC_FEATURE_REQUESTS_LOCAL];
  WORD32 requested_dyn_rng_measurement_type;
  WORD32 requested_dyn_range_is_single_val_flag;
  FLOAT32 requested_dyn_range_value;
  FLOAT32 requested_dyn_range_min_val;
  FLOAT32 requested_dyn_range_max_val;
  WORD32 requested_drc_characteristic;
} ia_loc_drc_interface_struct;

extern const ia_loc_drc_interface_struct loc_dyn_range_ctrl_interface[];

#define MAX_NUM_DRC_EFFECT_TYPE_REQUESTS_LOCAL 5
typedef struct {
  WORD32 requested_num_drc_effects;
  WORD32 desired_num_drc_effects_of_requested;
  WORD32 requested_drc_effect_type[MAX_NUM_DRC_EFFECT_TYPE_REQUESTS_LOCAL];
} ia_loc_requested_drc_effect_struct;

extern const ia_loc_requested_drc_effect_struct
    loc_requested_drc_effect_type_str[];

typedef struct {
  FLOAT32 compress;
  FLOAT32 boost;
  WORD32 drc_characteristic_target;
} ia_loc_drc_parameter_interface_struct;

extern const ia_loc_drc_parameter_interface_struct
    loc_drc_parameter_interface[];

extern const FLOAT32 f_bands_nrm_QMF71[71];
extern const FLOAT32 f_bands_nrm_QMF64[64];
extern const FLOAT32 f_bands_nrm_STFT256[257];

extern const FLOAT64 qmf_filter_coeff[640];

extern const ia_filter_bank_params_struct
    normal_cross_freq[FILTER_BANK_PARAMETER_COUNT];
#endif
