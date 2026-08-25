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
#ifndef IMPD_DRC_SECLECTION_PROCESS_H
#define IMPD_DRC_SECLECTION_PROCESS_H

#ifdef __cplusplus
extern "C" {
#endif /* __cplusplus */

#define EFFECT_TYPE_REQUESTED_NONE 0
#define EFFECT_TYPE_REQUESTED_NIGHT 1
#define EFFECT_TYPE_REQUESTED_NOISY 2
#define EFFECT_TYPE_REQUESTED_LIMITED 3
#define EFFECT_TYPE_REQUESTED_LOWLEVEL 4
#define EFFECT_TYPE_REQUESTED_DIALOG 5
#define EFFECT_TYPE_REQUESTED_GENERAL_COMPR 6
#define EFFECT_TYPE_REQUESTED_EXPAND 7
#define EFFECT_TYPE_REQUESTED_ARTISTIC 8
#define EFFECT_TYPE_REQUESTED_COUNT 9

#define MATCH_EFFECT_TYPE 0
#define MATCH_DYNAMIC_RANGE 1
#define MATCH_DRC_CHARACTERISTIC 2

typedef struct ia_drc_sel_proc_params_struct {
  WORD32 base_channel_count;
  WORD32 base_layout;
  WORD32 target_config_request_type;
  WORD32 num_downmix_id_requests;
  WORD32 requested_dwnmix_id[MAX_NUM_DOWNMIX_ID_REQUESTS];
  WORD32 requested_target_layout;
  WORD32 requested_target_ch_count;
  WORD32 target_ch_count_prelim;

  WORD32 loudness_normalization_on;
  FLOAT32 target_loudness;
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

  WORD32 num_bands_supported;
  WORD32 dynamic_range_control_on;
  WORD32 num_drc_feature_requests;
  WORD32 drc_feature_req_type[MAX_NUM_DRC_FEATURE_REQUESTS];
  WORD32 requested_num_drc_effects[MAX_NUM_DRC_FEATURE_REQUESTS];
  WORD32 desired_num_drc_effects_of_requested[MAX_NUM_DRC_FEATURE_REQUESTS];
  WORD32 requested_drc_effect_type[MAX_NUM_DRC_FEATURE_REQUESTS]
                                  [MAX_NUM_DRC_EFFECT_TYPE_REQUESTS];
  WORD32 requested_dyn_range_measur_type[MAX_NUM_DRC_FEATURE_REQUESTS];
  WORD32 requested_dyn_range_range_flag[MAX_NUM_DRC_FEATURE_REQUESTS];
  FLOAT32 requested_dyn_range_value[MAX_NUM_DRC_FEATURE_REQUESTS];
  FLOAT32 requested_dyn_range_min_val[MAX_NUM_DRC_FEATURE_REQUESTS];
  FLOAT32 requested_dyn_range_max_val[MAX_NUM_DRC_FEATURE_REQUESTS];
  WORD32 requested_drc_characteristic[MAX_NUM_DRC_FEATURE_REQUESTS];

  WORD32 loudness_eq_request;
  FLOAT32 sensitivity;
  FLOAT32 playback_gain;
  WORD32 eq_set_purpose_request;

#ifdef LOUDNESS_LEVELING_SUPPORT
  WORD32 loudness_leveling_on;
#endif

  FLOAT32 boost;
  FLOAT32 compress;
  WORD32 drc_characteristic_target;
} ia_drc_sel_proc_params_struct;

typedef struct ia_drc_sel_pro_struct {
  WORD32 first_frame;
  WORD32 drc_config_flag;
  WORD32 loudness_info_set_flag;
  WORD32 sel_proc_request_flag;
  WORD32 subband_domain_mode;
  WORD32 eq_inst_index[SUB_EQ_COUNT];
  WORD32 drc_instructions_index[SUB_DRC_COUNT];

  ia_drc_sel_proc_params_struct uni_drc_sel_proc_params;

  ia_drc_config drc_config;
  ia_drc_loudness_info_set_struct loudness_info_set;

  WORD32 drc_inst_index_sel;
  WORD32 drc_coef_index_sel;
  WORD32 downmix_inst_index_sel;

  WORD32 drc_set_id_valid_flag[DRC_INSTRUCTIONS_COUNT_MAX];
  WORD32 eq_set_id_valid_flag[EQ_INSTRUCTIONS_COUNT_MAX];

  WORD32 eq_inst_index_sel;
  WORD32 loud_eq_inst_index_sel;

  FLOAT32 compl_level_supported_total;

  ia_drc_sel_proc_output_struct uni_drc_sel_proc_output;

} ia_drc_sel_pro_struct;

WORD32 impd_map_target_config_req_downmix_id(
    ia_drc_sel_pro_struct* pstr_drc_uni_sel_proc,
    ia_drc_config* pstr_drc_config);

VOID impd_sel_downmix_matrix(ia_drc_sel_pro_struct* pstr_drc_uni_sel_proc,
                             ia_drc_config* pstr_drc_config);

WORD32
impd_find_drc_instructions_uni_drc(
    ia_drc_config* drc_config, WORD32 drc_set_id_requested,
    ia_drc_instructions_struct** str_drc_instruction_str);
WORD32
impd_find_eq_instructions(ia_drc_config* drc_config, WORD32 eq_set_id_requested,
                          ia_eq_instructions_struct** str_eq_instructions);
WORD32
impd_find_downmix(ia_drc_config* drc_config, WORD32 requested_dwnmix_id,
                  ia_downmix_instructions_struct** dwnmix_instructions);

WORD32
impd_find_eq_set_no_compression(ia_drc_config* pstr_drc_config,
                                WORD32 requested_dwnmix_id,
                                WORD32* no_compression_eq_cnt,
                                WORD32* no_compression_eq_id);
WORD32
impd_match_eq_set(ia_drc_config* drc_config, WORD32 downmix_id,
                  WORD32 drc_set_id, WORD32* eq_set_id_valid_flag,
                  WORD32* matching_eq_set_cnt, WORD32* matching_eq_set_idx);

WORD32
impd_select_loud_eq(ia_drc_config* pstr_drc_config, WORD32 requested_dwnmix_id,
                    WORD32 drc_set_id_requested, WORD32 eq_set_id_requested,
                    WORD32* loud_eq_id_selected);

WORD32
impd_drc_uni_selction_proc_init(
    ia_drc_sel_pro_struct* pstr_drc_uni_sel_proc,
    ia_drc_sel_proc_params_struct* pstr_drc_sel_proc_params_struct,
    ia_drc_interface_struct* pstr_drc_interface, WORD32 sub_band_domain_mode);

WORD32
impd_drc_uni_sel_proc_process(
    ia_drc_sel_pro_struct* pstr_drc_uni_sel_proc,
    ia_drc_config* pstr_drc_config,
    ia_drc_loudness_info_set_struct* pstr_loudness_info,
    ia_drc_sel_proc_output_struct* hia_drc_sel_proc_output_struct);

WORD32
impd_find_loud_eq_instructions_idx_for_id(ia_drc_config* drc_config,
                                          WORD32 loud_eq_set_id_requested,
                                          WORD32* instructions_idx);

#ifdef __cplusplus
}
#endif /* __cplusplus */

#endif
