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
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "impd_type_def.h"
#include "impd_drc_extr_delta_coded_info.h"
#include "impd_drc_common.h"
#include "impd_drc_struct.h"
#include "impd_drc_interface.h"
#include "impd_drc_selection_process.h"

WORD32 impd_drc_sel_proc_init_dflt(
    ia_drc_sel_pro_struct* pstr_drc_uni_sel_proc) {
  if (pstr_drc_uni_sel_proc != NULL) {
    pstr_drc_uni_sel_proc->first_frame = 0;

    pstr_drc_uni_sel_proc->uni_drc_sel_proc_params.base_channel_count = -1;
    pstr_drc_uni_sel_proc->uni_drc_sel_proc_params.base_layout = -1;
    pstr_drc_uni_sel_proc->uni_drc_sel_proc_params.target_config_request_type =
        0;
    pstr_drc_uni_sel_proc->uni_drc_sel_proc_params.num_downmix_id_requests = 0;

    pstr_drc_uni_sel_proc->uni_drc_sel_proc_params.album_mode = 0;

    pstr_drc_uni_sel_proc->uni_drc_sel_proc_params.peak_limiter = 0;

    pstr_drc_uni_sel_proc->uni_drc_sel_proc_params.loudness_normalization_on =
        0;
    pstr_drc_uni_sel_proc->uni_drc_sel_proc_params.target_loudness = -24.0f;

    pstr_drc_uni_sel_proc->uni_drc_sel_proc_params.loudness_deviation_max =
        LOUDNESS_DEVIATION_MAX_DEFAULT;

    pstr_drc_uni_sel_proc->uni_drc_sel_proc_params.loudness_measurement_method =
        USER_METHOD_DEFINITION_DEFAULT;
    pstr_drc_uni_sel_proc->uni_drc_sel_proc_params.loudness_measurement_system =
        USER_MEASUREMENT_SYSTEM_DEFAULT;
    pstr_drc_uni_sel_proc->uni_drc_sel_proc_params
        .loudness_measurement_pre_proc = USER_LOUDNESS_PREPROCESSING_DEFAULT;
    pstr_drc_uni_sel_proc->uni_drc_sel_proc_params.device_cut_off_frequency =
        500;
    pstr_drc_uni_sel_proc->uni_drc_sel_proc_params.loudness_norm_gain_db_max =
        LOUDNESS_NORMALIZATION_GAIN_MAX_DEFAULT;
    pstr_drc_uni_sel_proc->uni_drc_sel_proc_params
        .loudness_norm_gain_modification_db = 0.0f;
    pstr_drc_uni_sel_proc->uni_drc_sel_proc_params.output_peak_level_max = 0.0f;
    if (pstr_drc_uni_sel_proc->uni_drc_sel_proc_params.peak_limiter == 1) {
      pstr_drc_uni_sel_proc->uni_drc_sel_proc_params.output_peak_level_max =
          6.0f;
    }

    pstr_drc_uni_sel_proc->uni_drc_sel_proc_params.dynamic_range_control_on = 1;
    pstr_drc_uni_sel_proc->uni_drc_sel_proc_params.num_bands_supported = 4;
    pstr_drc_uni_sel_proc->uni_drc_sel_proc_params.num_drc_feature_requests = 0;

    pstr_drc_uni_sel_proc->uni_drc_sel_proc_params.boost = 1.f;
    pstr_drc_uni_sel_proc->uni_drc_sel_proc_params.compress = 1.f;
    pstr_drc_uni_sel_proc->uni_drc_sel_proc_params.drc_characteristic_target =
        0;

    pstr_drc_uni_sel_proc->uni_drc_sel_proc_params.loudness_eq_request =
        LOUD_EQ_REQUEST_OFF;
    pstr_drc_uni_sel_proc->uni_drc_sel_proc_params.eq_set_purpose_request =
        EQ_PURPOSE_EQ_OFF;
    pstr_drc_uni_sel_proc->compl_level_supported_total =
        COMPLEXITY_LEVEL_SUPPORTED_TOTAL;

    pstr_drc_uni_sel_proc->drc_inst_index_sel = -1;
    pstr_drc_uni_sel_proc->drc_coef_index_sel = -1;
    pstr_drc_uni_sel_proc->downmix_inst_index_sel = -1;
    pstr_drc_uni_sel_proc->drc_instructions_index[0] = -1;
    pstr_drc_uni_sel_proc->drc_instructions_index[1] = -1;
    pstr_drc_uni_sel_proc->drc_instructions_index[2] = -1;
    pstr_drc_uni_sel_proc->drc_instructions_index[3] = -1;
    pstr_drc_uni_sel_proc->uni_drc_sel_proc_output.output_peak_level_db = 0;
    pstr_drc_uni_sel_proc->uni_drc_sel_proc_output
        .loudness_normalization_gain_db = 0;
    pstr_drc_uni_sel_proc->uni_drc_sel_proc_output.output_loudness =
        UNDEFINED_LOUDNESS_VALUE;
    pstr_drc_uni_sel_proc->uni_drc_sel_proc_output.num_sel_drc_sets = 0;
    pstr_drc_uni_sel_proc->uni_drc_sel_proc_output.active_downmix_id = 0;
    pstr_drc_uni_sel_proc->uni_drc_sel_proc_output.base_channel_count = 0;
    pstr_drc_uni_sel_proc->uni_drc_sel_proc_output.target_channel_count = 0;
    pstr_drc_uni_sel_proc->uni_drc_sel_proc_output.downmix_matrix_present = 0;

    pstr_drc_uni_sel_proc->eq_inst_index[0] = -1;
    pstr_drc_uni_sel_proc->eq_inst_index[1] = -1;
    pstr_drc_uni_sel_proc->sel_proc_request_flag = 1;

#ifdef LOUDNESS_LEVELING_SUPPORT
    pstr_drc_uni_sel_proc->uni_drc_sel_proc_params.loudness_leveling_on = 1;
#endif
  } else {
    return 1;
  }

  return 0;
}
WORD32
impd_drc_sel_proc_init_sel_proc_params(
    ia_drc_sel_pro_struct* pstr_drc_uni_sel_proc,
    ia_drc_sel_proc_params_struct* pstr_drc_sel_proc_params_struct) {
  if (pstr_drc_uni_sel_proc != NULL &&
      pstr_drc_sel_proc_params_struct != NULL) {
    if (memcmp(&pstr_drc_uni_sel_proc->uni_drc_sel_proc_params,
               pstr_drc_sel_proc_params_struct,
               sizeof(ia_drc_sel_proc_params_struct))) {
      pstr_drc_uni_sel_proc->uni_drc_sel_proc_params =
          *pstr_drc_sel_proc_params_struct;
      pstr_drc_uni_sel_proc->sel_proc_request_flag = 1;
    }
  }

  return 0;
}
VOID impd_drc_sel_proc_init_interface_params(
    ia_drc_sel_pro_struct* pstr_drc_uni_sel_proc,
    ia_drc_interface_struct* pstr_drc_interface) {
  WORD32 i, j;

  if (pstr_drc_uni_sel_proc != NULL && pstr_drc_interface != NULL) {
#ifdef LOUDNESS_LEVELING_SUPPORT
    pstr_drc_uni_sel_proc->uni_drc_sel_proc_params.loudness_leveling_on =
        pstr_drc_interface->drc_uni_interface_ext.loudness_leveling_on;
#endif
    if (pstr_drc_interface->system_interface_flag) {
      if (pstr_drc_uni_sel_proc->uni_drc_sel_proc_params
              .target_config_request_type !=
          pstr_drc_interface->system_interface.target_config_request_type) {
        pstr_drc_uni_sel_proc->uni_drc_sel_proc_params
            .target_config_request_type =
            pstr_drc_interface->system_interface.target_config_request_type;
        pstr_drc_uni_sel_proc->sel_proc_request_flag = 1;
      }
      switch (pstr_drc_uni_sel_proc->uni_drc_sel_proc_params
                  .target_config_request_type) {
        case 0:
          if (pstr_drc_uni_sel_proc->uni_drc_sel_proc_params
                  .num_downmix_id_requests !=
              pstr_drc_interface->system_interface.num_downmix_id_requests) {
            pstr_drc_uni_sel_proc->uni_drc_sel_proc_params
                .num_downmix_id_requests =
                pstr_drc_interface->system_interface.num_downmix_id_requests;
            pstr_drc_uni_sel_proc->sel_proc_request_flag = 1;
          }
          for (i = 0; i < pstr_drc_uni_sel_proc->uni_drc_sel_proc_params
                              .num_downmix_id_requests;
               i++) {
            if (pstr_drc_uni_sel_proc->uni_drc_sel_proc_params
                    .requested_dwnmix_id[i] !=
                pstr_drc_interface->system_interface.requested_dwnmix_id[i]) {
              pstr_drc_uni_sel_proc->uni_drc_sel_proc_params
                  .requested_dwnmix_id[i] =
                  pstr_drc_interface->system_interface.requested_dwnmix_id[i];
              pstr_drc_uni_sel_proc->sel_proc_request_flag = 1;
            }
          }
          break;
        case 1:
          if (pstr_drc_uni_sel_proc->uni_drc_sel_proc_params
                  .requested_target_layout !=
              pstr_drc_interface->system_interface.requested_target_layout) {
            pstr_drc_uni_sel_proc->uni_drc_sel_proc_params
                .requested_target_layout =
                pstr_drc_interface->system_interface.requested_target_layout;
            pstr_drc_uni_sel_proc->sel_proc_request_flag = 1;
          }
          break;
        case 2:
          if (pstr_drc_uni_sel_proc->uni_drc_sel_proc_params
                  .requested_target_ch_count !=
              pstr_drc_interface->system_interface.requested_target_ch_count) {
            pstr_drc_uni_sel_proc->uni_drc_sel_proc_params
                .requested_target_ch_count =
                pstr_drc_interface->system_interface.requested_target_ch_count;
            pstr_drc_uni_sel_proc->sel_proc_request_flag = 1;
          }
          break;
      }
    }
    if (pstr_drc_interface->loudness_norm_ctrl_interface_flag) {
      if (pstr_drc_uni_sel_proc->uni_drc_sel_proc_params
              .loudness_normalization_on !=
          pstr_drc_interface->loudness_norm_ctrl_interface
              .loudness_normalization_on) {
        pstr_drc_uni_sel_proc->uni_drc_sel_proc_params
            .loudness_normalization_on =
            pstr_drc_interface->loudness_norm_ctrl_interface
                .loudness_normalization_on;
        pstr_drc_uni_sel_proc->sel_proc_request_flag = 1;
      }
      if (pstr_drc_uni_sel_proc->uni_drc_sel_proc_params
              .loudness_normalization_on) {
        if (pstr_drc_uni_sel_proc->uni_drc_sel_proc_params.target_loudness !=
            pstr_drc_interface->loudness_norm_ctrl_interface.target_loudness) {
          pstr_drc_uni_sel_proc->uni_drc_sel_proc_params.target_loudness =
              pstr_drc_interface->loudness_norm_ctrl_interface.target_loudness;
          pstr_drc_uni_sel_proc->sel_proc_request_flag = 1;
        }
      }
    }
    if (pstr_drc_interface->loudness_norm_parameter_interface_flag) {
      if (pstr_drc_uni_sel_proc->uni_drc_sel_proc_params.album_mode !=
          pstr_drc_interface->loudness_norm_param_interface.album_mode) {
        pstr_drc_uni_sel_proc->uni_drc_sel_proc_params.album_mode =
            pstr_drc_interface->loudness_norm_param_interface.album_mode;
        pstr_drc_uni_sel_proc->sel_proc_request_flag = 1;
      }
      if (pstr_drc_uni_sel_proc->uni_drc_sel_proc_params.peak_limiter !=
          pstr_drc_interface->loudness_norm_param_interface.peak_limiter) {
        pstr_drc_uni_sel_proc->uni_drc_sel_proc_params.peak_limiter =
            pstr_drc_interface->loudness_norm_param_interface.peak_limiter;
        pstr_drc_uni_sel_proc->sel_proc_request_flag = 1;
      }
      if (pstr_drc_interface->loudness_norm_param_interface
              .change_loudness_deviation_max) {
        if (pstr_drc_uni_sel_proc->uni_drc_sel_proc_params
                .loudness_deviation_max !=
            pstr_drc_interface->loudness_norm_param_interface
                .loudness_deviation_max) {
          pstr_drc_uni_sel_proc->uni_drc_sel_proc_params
              .loudness_deviation_max =
              pstr_drc_interface->loudness_norm_param_interface
                  .loudness_deviation_max;
          pstr_drc_uni_sel_proc->sel_proc_request_flag = 1;
        }
      }
      if (pstr_drc_interface->loudness_norm_param_interface
              .change_loudness_measur_method) {
        if (pstr_drc_uni_sel_proc->uni_drc_sel_proc_params
                .loudness_measurement_method !=
            pstr_drc_interface->loudness_norm_param_interface
                .loudness_measurement_method) {
          pstr_drc_uni_sel_proc->uni_drc_sel_proc_params
              .loudness_measurement_method =
              pstr_drc_interface->loudness_norm_param_interface
                  .loudness_measurement_method;
          pstr_drc_uni_sel_proc->sel_proc_request_flag = 1;
        }
      }
      if (pstr_drc_interface->loudness_norm_param_interface
              .change_loudness_measur_pre_proc) {
        if (pstr_drc_uni_sel_proc->uni_drc_sel_proc_params
                .loudness_measurement_pre_proc !=
            pstr_drc_interface->loudness_norm_param_interface
                .loudness_measurement_pre_proc) {
          pstr_drc_uni_sel_proc->uni_drc_sel_proc_params
              .loudness_measurement_pre_proc =
              pstr_drc_interface->loudness_norm_param_interface
                  .loudness_measurement_pre_proc;
          pstr_drc_uni_sel_proc->sel_proc_request_flag = 1;
        }
      }
      if (pstr_drc_interface->loudness_norm_param_interface
              .change_loudness_measur_system) {
        if (pstr_drc_uni_sel_proc->uni_drc_sel_proc_params
                .loudness_measurement_system !=
            pstr_drc_interface->loudness_norm_param_interface
                .loudness_measurement_system) {
          pstr_drc_uni_sel_proc->uni_drc_sel_proc_params
              .loudness_measurement_system =
              pstr_drc_interface->loudness_norm_param_interface
                  .loudness_measurement_system;
          pstr_drc_uni_sel_proc->sel_proc_request_flag = 1;
        }
      }
      if (pstr_drc_interface->loudness_norm_param_interface
              .change_device_cut_off_freq) {
        if (pstr_drc_uni_sel_proc->uni_drc_sel_proc_params
                .device_cut_off_frequency !=
            pstr_drc_interface->loudness_norm_param_interface
                .device_cut_off_frequency) {
          pstr_drc_uni_sel_proc->uni_drc_sel_proc_params
              .device_cut_off_frequency =
              pstr_drc_interface->loudness_norm_param_interface
                  .device_cut_off_frequency;
          pstr_drc_uni_sel_proc->sel_proc_request_flag = 1;
        }
      }
      if (pstr_drc_interface->loudness_norm_param_interface
              .change_loudness_norm_gain_db_max) {
        if (pstr_drc_uni_sel_proc->uni_drc_sel_proc_params
                .loudness_norm_gain_db_max !=
            pstr_drc_interface->loudness_norm_param_interface
                .loudness_norm_gain_db_max) {
          pstr_drc_uni_sel_proc->uni_drc_sel_proc_params
              .loudness_norm_gain_db_max =
              pstr_drc_interface->loudness_norm_param_interface
                  .loudness_norm_gain_db_max;
          pstr_drc_uni_sel_proc->sel_proc_request_flag = 1;
        }
      }
      if (pstr_drc_interface->loudness_norm_param_interface
              .change_loudness_norm_gain_modification_db) {
        if (pstr_drc_uni_sel_proc->uni_drc_sel_proc_params
                .loudness_norm_gain_modification_db !=
            pstr_drc_interface->loudness_norm_param_interface
                .loudness_norm_gain_modification_db) {
          pstr_drc_uni_sel_proc->uni_drc_sel_proc_params
              .loudness_norm_gain_modification_db =
              pstr_drc_interface->loudness_norm_param_interface
                  .loudness_norm_gain_modification_db;
          pstr_drc_uni_sel_proc->sel_proc_request_flag = 1;
        }
      }
      if (pstr_drc_interface->loudness_norm_param_interface
              .change_output_peak_level_max) {
        if (pstr_drc_uni_sel_proc->uni_drc_sel_proc_params
                .output_peak_level_max !=
            pstr_drc_interface->loudness_norm_param_interface
                .output_peak_level_max) {
          pstr_drc_uni_sel_proc->uni_drc_sel_proc_params.output_peak_level_max =
              pstr_drc_interface->loudness_norm_param_interface
                  .output_peak_level_max;
          pstr_drc_uni_sel_proc->sel_proc_request_flag = 1;
        }
      }
    }
    if (pstr_drc_interface->drc_interface_flag) {
      if (pstr_drc_uni_sel_proc->uni_drc_sel_proc_params
              .dynamic_range_control_on !=
          pstr_drc_interface->drc_ctrl_interface.dynamic_range_control_on) {
        pstr_drc_uni_sel_proc->uni_drc_sel_proc_params
            .dynamic_range_control_on =
            pstr_drc_interface->drc_ctrl_interface.dynamic_range_control_on;
        pstr_drc_uni_sel_proc->sel_proc_request_flag = 1;
        if (!pstr_drc_uni_sel_proc->uni_drc_sel_proc_params
                 .dynamic_range_control_on) {
          pstr_drc_uni_sel_proc->uni_drc_sel_proc_params
              .num_drc_feature_requests = 0;
        }
      }
      if (pstr_drc_uni_sel_proc->uni_drc_sel_proc_params
              .dynamic_range_control_on) {
        if (pstr_drc_uni_sel_proc->uni_drc_sel_proc_params
                .num_drc_feature_requests !=
            pstr_drc_interface->drc_ctrl_interface.num_drc_feature_requests) {
          pstr_drc_uni_sel_proc->uni_drc_sel_proc_params
              .num_drc_feature_requests =
              pstr_drc_interface->drc_ctrl_interface.num_drc_feature_requests;
          pstr_drc_uni_sel_proc->sel_proc_request_flag = 1;
        }
        for (i = 0; i < pstr_drc_uni_sel_proc->uni_drc_sel_proc_params
                            .num_drc_feature_requests;
             i++) {
          if (pstr_drc_uni_sel_proc->uni_drc_sel_proc_params
                  .drc_feature_req_type[i] !=
              pstr_drc_interface->drc_ctrl_interface.drc_feature_req_type[i]) {
            pstr_drc_uni_sel_proc->uni_drc_sel_proc_params
                .drc_feature_req_type[i] =
                pstr_drc_interface->drc_ctrl_interface.drc_feature_req_type[i];
            pstr_drc_uni_sel_proc->sel_proc_request_flag = 1;
          }
          switch (pstr_drc_uni_sel_proc->uni_drc_sel_proc_params
                      .drc_feature_req_type[i]) {
            case 0:
              if (pstr_drc_uni_sel_proc->uni_drc_sel_proc_params
                      .requested_num_drc_effects[i] !=
                  pstr_drc_interface->drc_ctrl_interface
                      .requested_num_drc_effects[i]) {
                pstr_drc_uni_sel_proc->uni_drc_sel_proc_params
                    .requested_num_drc_effects[i] =
                    pstr_drc_interface->drc_ctrl_interface
                        .requested_num_drc_effects[i];
                pstr_drc_uni_sel_proc->sel_proc_request_flag = 1;
              }
              if (pstr_drc_uni_sel_proc->uni_drc_sel_proc_params
                      .desired_num_drc_effects_of_requested[i] !=
                  pstr_drc_interface->drc_ctrl_interface
                      .desired_num_drc_effects_of_requested[i]) {
                pstr_drc_uni_sel_proc->uni_drc_sel_proc_params
                    .desired_num_drc_effects_of_requested[i] =
                    pstr_drc_interface->drc_ctrl_interface
                        .desired_num_drc_effects_of_requested[i];
                pstr_drc_uni_sel_proc->sel_proc_request_flag = 1;
              }
              for (j = 0; j < pstr_drc_uni_sel_proc->uni_drc_sel_proc_params
                                  .requested_num_drc_effects[i];
                   j++) {
                if (pstr_drc_uni_sel_proc->uni_drc_sel_proc_params
                        .requested_drc_effect_type[i][j] !=
                    pstr_drc_interface->drc_ctrl_interface
                        .requested_drc_effect_type[i][j]) {
                  pstr_drc_uni_sel_proc->uni_drc_sel_proc_params
                      .requested_drc_effect_type[i][j] =
                      pstr_drc_interface->drc_ctrl_interface
                          .requested_drc_effect_type[i][j];
                  pstr_drc_uni_sel_proc->sel_proc_request_flag = 1;
                }
              }
              break;
            case 1:
              if (pstr_drc_uni_sel_proc->uni_drc_sel_proc_params
                      .requested_dyn_range_measur_type[i] !=
                  pstr_drc_interface->drc_ctrl_interface
                      .requested_dyn_rng_measurement_type[i]) {
                pstr_drc_uni_sel_proc->uni_drc_sel_proc_params
                    .requested_dyn_range_measur_type[i] =
                    pstr_drc_interface->drc_ctrl_interface
                        .requested_dyn_rng_measurement_type[i];
                pstr_drc_uni_sel_proc->sel_proc_request_flag = 1;
              }
              if (pstr_drc_uni_sel_proc->uni_drc_sel_proc_params
                      .requested_dyn_range_range_flag[i] !=
                  pstr_drc_interface->drc_ctrl_interface
                      .requested_dyn_range_is_single_val_flag[i]) {
                pstr_drc_uni_sel_proc->uni_drc_sel_proc_params
                    .requested_dyn_range_range_flag[i] =
                    pstr_drc_interface->drc_ctrl_interface
                        .requested_dyn_range_is_single_val_flag[i];
                pstr_drc_uni_sel_proc->sel_proc_request_flag = 1;
              }
              if (pstr_drc_uni_sel_proc->uni_drc_sel_proc_params
                      .requested_dyn_range_range_flag[i] == 0) {
                if (pstr_drc_uni_sel_proc->uni_drc_sel_proc_params
                        .requested_dyn_range_value[i] !=
                    pstr_drc_interface->drc_ctrl_interface
                        .requested_dyn_range_value[i]) {
                  pstr_drc_uni_sel_proc->uni_drc_sel_proc_params
                      .requested_dyn_range_value[i] =
                      pstr_drc_interface->drc_ctrl_interface
                          .requested_dyn_range_value[i];
                  pstr_drc_uni_sel_proc->sel_proc_request_flag = 1;
                }
              } else {
                if (pstr_drc_uni_sel_proc->uni_drc_sel_proc_params
                        .requested_dyn_range_min_val[i] !=
                    pstr_drc_interface->drc_ctrl_interface
                        .requested_dyn_range_min_val[i]) {
                  pstr_drc_uni_sel_proc->uni_drc_sel_proc_params
                      .requested_dyn_range_min_val[i] =
                      pstr_drc_interface->drc_ctrl_interface
                          .requested_dyn_range_min_val[i];
                  pstr_drc_uni_sel_proc->sel_proc_request_flag = 1;
                }
                if (pstr_drc_uni_sel_proc->uni_drc_sel_proc_params
                        .requested_dyn_range_max_val[i] !=
                    pstr_drc_interface->drc_ctrl_interface
                        .requested_dyn_range_max_val[i]) {
                  pstr_drc_uni_sel_proc->uni_drc_sel_proc_params
                      .requested_dyn_range_max_val[i] =
                      pstr_drc_interface->drc_ctrl_interface
                          .requested_dyn_range_max_val[i];
                  pstr_drc_uni_sel_proc->sel_proc_request_flag = 1;
                }
              }
              break;
            case 2:
              if (pstr_drc_uni_sel_proc->uni_drc_sel_proc_params
                      .requested_drc_characteristic[i] !=
                  pstr_drc_interface->drc_ctrl_interface
                      .requested_drc_characteristic[i]) {
                pstr_drc_uni_sel_proc->uni_drc_sel_proc_params
                    .requested_drc_characteristic[i] =
                    pstr_drc_interface->drc_ctrl_interface
                        .requested_drc_characteristic[i];
                pstr_drc_uni_sel_proc->sel_proc_request_flag = 1;
              }
              break;
          }
        }
      }
    }
    if (pstr_drc_interface->drc_parameter_interface_flag) {
      if (pstr_drc_interface->drc_parameter_interface.change_compress) {
        if (pstr_drc_uni_sel_proc->uni_drc_sel_proc_params.compress !=
            pstr_drc_interface->drc_parameter_interface.compress) {
          pstr_drc_uni_sel_proc->uni_drc_sel_proc_params.compress =
              pstr_drc_interface->drc_parameter_interface.compress;
          pstr_drc_uni_sel_proc->sel_proc_request_flag = 1;
        }
      }
      if (pstr_drc_interface->drc_parameter_interface.change_boost) {
        if (pstr_drc_uni_sel_proc->uni_drc_sel_proc_params.boost !=
            pstr_drc_interface->drc_parameter_interface.boost) {
          pstr_drc_uni_sel_proc->uni_drc_sel_proc_params.boost =
              pstr_drc_interface->drc_parameter_interface.boost;
          pstr_drc_uni_sel_proc->sel_proc_request_flag = 1;
        }
      }
      if (pstr_drc_interface->drc_parameter_interface
              .change_drc_characteristic_target) {
        if (pstr_drc_uni_sel_proc->uni_drc_sel_proc_params
                .drc_characteristic_target !=
            pstr_drc_interface->drc_parameter_interface
                .drc_characteristic_target) {
          pstr_drc_uni_sel_proc->uni_drc_sel_proc_params
              .drc_characteristic_target =
              pstr_drc_interface->drc_parameter_interface
                  .drc_characteristic_target;
          pstr_drc_uni_sel_proc->sel_proc_request_flag = 1;
        }
      }
    }
    if (pstr_drc_interface->drc_uni_interface_ext_flag) {
      ia_drc_uni_interface_ext_struct* drc_uni_interface_ext =
          &pstr_drc_interface->drc_uni_interface_ext;
      if (drc_uni_interface_ext->loudness_eq_parameter_interface_flag) {
        ia_loudness_eq_parameter_interface_struct*
            loudness_eq_parameter_interface =
                &drc_uni_interface_ext->loudness_eq_parameter_interface;
        if (loudness_eq_parameter_interface->loudness_eq_request_flag) {
          pstr_drc_uni_sel_proc->uni_drc_sel_proc_params.loudness_eq_request =
              loudness_eq_parameter_interface->loudness_eq_request;
          pstr_drc_uni_sel_proc->uni_drc_sel_proc_params.sensitivity =
              loudness_eq_parameter_interface->sensitivity;
          pstr_drc_uni_sel_proc->uni_drc_sel_proc_params.playback_gain =
              loudness_eq_parameter_interface->playback_gain;
        }
      }
      if (drc_uni_interface_ext->eq_ctrl_interface_flag) {
        pstr_drc_uni_sel_proc->uni_drc_sel_proc_params.eq_set_purpose_request =
            drc_uni_interface_ext->eq_ctrl_interface.eq_set_purpose_request;
      }
    }
  }

  return;
}
