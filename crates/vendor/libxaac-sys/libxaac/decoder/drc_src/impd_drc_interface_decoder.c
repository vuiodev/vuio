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
#include <math.h>
#include <assert.h>
#include <string.h>
#include "impd_type_def.h"
#include "impd_drc_bitbuffer.h"
#include "impd_drc_common.h"
#include "impd_drc_interface.h"
#include "impd_drc_parser_interface.h"

WORD32
impd_drc_dec_interface_add_effect_type(ia_drc_interface_struct* pstr_drc_interface,
                                       WORD32 drc_effect_type, WORD32 target_loudness,
                                       WORD32 loud_norm, WORD32 album_mode, FLOAT32 boost,
                                       FLOAT32 compress
#ifdef LOUDNESS_LEVELING_SUPPORT
                                       ,
                                       WORD32 loudness_leveling_flag
#endif
) {
  WORD32 err = 0;
  WORD32 i = 0;

  if (pstr_drc_interface != NULL) {
    pstr_drc_interface->interface_signat_flag = 0;
    for (i = 0; i < MAX_SIGNATURE_DATA_LENGTH_PLUS_ONE * 8; i++)
      pstr_drc_interface->drc_uni_interface_signature.interface_signat_data[i] =
          0;

    pstr_drc_interface->drc_uni_interface_signature.interface_signat_data_len =
        0;

    pstr_drc_interface->system_interface_flag = 1;

    pstr_drc_interface->system_interface.target_config_request_type = 0;
    pstr_drc_interface->system_interface.num_downmix_id_requests = 0;
    for (i = 0; i < MAX_NUM_DOWNMIX_ID_REQUESTS; i++) {
      pstr_drc_interface->system_interface.requested_dwnmix_id[i] = 0;
    }
    pstr_drc_interface->system_interface.requested_target_layout = 0;
    pstr_drc_interface->system_interface.requested_target_ch_count = 0;

    pstr_drc_interface->loudness_norm_ctrl_interface_flag = 1;
    if (loud_norm == 1) {
      pstr_drc_interface->loudness_norm_ctrl_interface
          .loudness_normalization_on = 1;
    } else {
      pstr_drc_interface->loudness_norm_ctrl_interface
          .loudness_normalization_on = 0;
    }
    pstr_drc_interface->loudness_norm_ctrl_interface.target_loudness =
        (FLOAT32)target_loudness;
#ifdef LOUDNESS_LEVELING_SUPPORT
    pstr_drc_interface->drc_uni_interface_ext.loudness_leveling_on = loudness_leveling_flag;
#endif
    pstr_drc_interface->loudness_norm_parameter_interface_flag = 1;
    pstr_drc_interface->loudness_norm_param_interface.album_mode = album_mode;
    pstr_drc_interface->loudness_norm_param_interface.peak_limiter = 0;
    pstr_drc_interface->loudness_norm_param_interface
        .change_loudness_deviation_max = 1;
    pstr_drc_interface->loudness_norm_param_interface.loudness_deviation_max =
        63;
    pstr_drc_interface->loudness_norm_param_interface
        .change_loudness_measur_method = 1;
    pstr_drc_interface->loudness_norm_param_interface
        .loudness_measurement_method = 1;
    pstr_drc_interface->loudness_norm_param_interface
        .change_loudness_measur_system = 1;
    pstr_drc_interface->loudness_norm_param_interface
        .loudness_measurement_system = 1;
    pstr_drc_interface->loudness_norm_param_interface
        .change_loudness_measur_pre_proc = 0;
    pstr_drc_interface->loudness_norm_param_interface
        .loudness_measurement_pre_proc = 0;
    pstr_drc_interface->loudness_norm_param_interface
        .change_device_cut_off_freq = 1;
    pstr_drc_interface->loudness_norm_param_interface.device_cut_off_frequency =
        20;
    pstr_drc_interface->loudness_norm_param_interface
        .change_loudness_norm_gain_db_max = 1;
    pstr_drc_interface->loudness_norm_param_interface
        .loudness_norm_gain_db_max = 1000.0;
    pstr_drc_interface->loudness_norm_param_interface
        .change_loudness_norm_gain_modification_db = 1;
    pstr_drc_interface->loudness_norm_param_interface
        .loudness_norm_gain_modification_db = 0.0;
    pstr_drc_interface->loudness_norm_param_interface
        .change_output_peak_level_max = 1;
    pstr_drc_interface->loudness_norm_param_interface.output_peak_level_max =
        0.0;

    pstr_drc_interface->drc_interface_flag = 1;
    if (drc_effect_type == -1) {
      pstr_drc_interface->drc_ctrl_interface.dynamic_range_control_on = 0;
      pstr_drc_interface->drc_ctrl_interface.requested_drc_effect_type[0][0] =
          0;
    } else if (drc_effect_type == 0) {
      pstr_drc_interface->drc_ctrl_interface.dynamic_range_control_on = 1;
      pstr_drc_interface->drc_ctrl_interface.num_drc_feature_requests = 0;
    } else {
      pstr_drc_interface->drc_ctrl_interface.dynamic_range_control_on = 1;
      pstr_drc_interface->drc_ctrl_interface.requested_drc_effect_type[0][0] =
          drc_effect_type;
      pstr_drc_interface->drc_ctrl_interface.num_drc_feature_requests = 3;
      pstr_drc_interface->drc_ctrl_interface.drc_feature_req_type[0] = 0;
      pstr_drc_interface->drc_ctrl_interface.drc_feature_req_type[1] = 1;
      pstr_drc_interface->drc_ctrl_interface.drc_feature_req_type[2] = 2;
      pstr_drc_interface->drc_ctrl_interface.requested_num_drc_effects[0] = 1;
      pstr_drc_interface->drc_ctrl_interface
          .desired_num_drc_effects_of_requested[0] = 1;
    }

    pstr_drc_interface->drc_ctrl_interface
        .requested_dyn_rng_measurement_type[0] = 0;
    pstr_drc_interface->drc_ctrl_interface
        .requested_dyn_range_is_single_val_flag[0] = 0;
    pstr_drc_interface->drc_ctrl_interface
        .requested_dyn_range_is_single_val_flag[1] = 0;
    pstr_drc_interface->drc_ctrl_interface.requested_dyn_range_min_val[1] =
        3.0f;
    pstr_drc_interface->drc_ctrl_interface.requested_dyn_range_max_val[1] =
        10.0f;
    pstr_drc_interface->drc_ctrl_interface.requested_drc_characteristic[2] = 3;

    pstr_drc_interface->drc_parameter_interface_flag = 1;
    pstr_drc_interface->drc_parameter_interface.change_compress = 1;
    pstr_drc_interface->drc_parameter_interface.change_boost = 1;
    pstr_drc_interface->drc_parameter_interface.compress = compress;
    pstr_drc_interface->drc_parameter_interface.boost = boost;
    pstr_drc_interface->drc_parameter_interface
        .change_drc_characteristic_target = 1;
    pstr_drc_interface->drc_parameter_interface.drc_characteristic_target = 0;

    pstr_drc_interface->drc_uni_interface_ext_flag = 0;

  } else {
    return UNEXPECTED_ERROR;
  }

  return err;
}
