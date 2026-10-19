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
#ifndef IMPD_DRC_UNI_INTERFACE_H
#define IMPD_DRC_UNI_INTERFACE_H

typedef struct {
  WORD32 ext_size_bits;
  WORD32 ext_bit_size;
  WORD32 uni_drc_interface_ext_type;
} ia_specific_interface_extension_struct;

typedef struct {
  WORD32 interface_ext_count;
  ia_specific_interface_extension_struct specific_interface_ext[EXT_COUNT_MAX];
} ia_drc_uni_interface_ext_struct;

typedef struct {
  WORD32 change_compress;
  WORD32 change_boost;
  FLOAT32 compress;
  FLOAT32 boost;
  WORD32 change_drc_characteristic_target;
  WORD32 drc_characteristic_target;
} ia_drc_parameter_interface_struct;

typedef struct {
  WORD32 dynamic_range_control_on;
  WORD32 num_drc_feature_requests;
  WORD32 drc_feature_req_type[MAX_NUM_DRC_FEATURE_REQUESTS];
  WORD32 requested_num_drc_effects[MAX_NUM_DRC_FEATURE_REQUESTS];
  WORD32 desired_num_drc_effects_of_requested[MAX_NUM_DRC_FEATURE_REQUESTS];
  WORD32 requested_drc_effect_type[MAX_NUM_DRC_FEATURE_REQUESTS]
                                  [MAX_NUM_DRC_EFFECT_TYPE_REQUESTS];
  WORD32 requested_dyn_rng_measurement_type[MAX_NUM_DRC_FEATURE_REQUESTS];
  WORD32 requested_dyn_range_is_single_val_flag[MAX_NUM_DRC_FEATURE_REQUESTS];
  FLOAT32 requested_dyn_range_value[MAX_NUM_DRC_FEATURE_REQUESTS];
  FLOAT32 requested_dyn_range_min_val[MAX_NUM_DRC_FEATURE_REQUESTS];
  FLOAT32 requested_dyn_range_max_val[MAX_NUM_DRC_FEATURE_REQUESTS];
  WORD32 requested_drc_characteristic[MAX_NUM_DRC_FEATURE_REQUESTS];
} ia_dyn_rng_ctrl_interface_struct;

typedef struct {
  WORD32 album_mode;
  WORD32 peak_limiter;
  WORD32 change_loudness_deviation_max;
  WORD32 loudness_deviation_max;
  WORD32 change_loudness_measur_method;
  WORD32 loudness_measurement_method;
  WORD32 change_loudness_measur_system;
  WORD32 loudness_measurement_system;
  WORD32 change_loudness_measur_pre_proc;
  WORD32 loudness_measurement_pre_proc;
  WORD32 change_device_cut_off_freq;
  WORD32 device_cut_off_frequency;
  WORD32 change_loudness_norm_gain_db_max;
  FLOAT32 loudness_norm_gain_db_max;
  WORD32 change_loudness_norm_gain_modification_db;
  FLOAT32 loudness_norm_gain_modification_db;
  WORD32 change_output_peak_level_max;
  FLOAT32 output_peak_level_max;
} ia_loudness_norm_parameter_interface_struct;

typedef struct {
  WORD32 loudness_normalization_on;
  FLOAT32 target_loudness;
} ia_loudness_norm_ctrl_interface_struct;

typedef struct {
  WORD32 target_config_request_type;
  WORD32 num_downmix_id_requests;
  WORD32 requested_dwnmix_id[MAX_NUM_DOWNMIX_ID_REQUESTS];
  WORD32 requested_target_layout;
  WORD32 requested_target_ch_count;
} ia_system_interface_struct;

typedef struct {
  WORD32 interface_signat_type;
  WORD32 interface_signat_data_len;
  UWORD32 interface_signat_data[MAX_SIGNATURE_DATA_LENGTH_PLUS_ONE * 8];
} ia_drc_uni_interface_signat_struct;

typedef struct ia_drc_interface_struct {
  WORD32 interface_signat_flag;
  WORD32 system_interface_flag;
  WORD32 loudness_norm_ctrl_interface_flag;
  WORD32 loudness_norm_parameter_interface_flag;
  WORD32 drc_interface_flag;
  WORD32 drc_parameter_interface_flag;
  WORD32 drc_uni_interface_ext_flag;
  ia_drc_uni_interface_signat_struct drc_uni_interface_signature;
  ia_system_interface_struct system_interface;
  ia_loudness_norm_ctrl_interface_struct loudness_norm_ctrl_interface;
  ia_loudness_norm_parameter_interface_struct loudness_norm_param_interface;
  ia_dyn_rng_ctrl_interface_struct drc_ctrl_interface;
  ia_drc_parameter_interface_struct drc_parameter_interface;
  ia_drc_uni_interface_ext_struct drc_uni_interface_ext;
} ia_drc_interface_struct;

#endif
