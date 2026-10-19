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

#include <string.h>
#include "ixheaac_type_def.h"
#include "ixheaac_error_standards.h"
#include "ixheaace_error_codes.h"

#include "iusace_bitbuffer.h"
#include "iusace_cnst.h"
#include "impd_drc_common_enc.h"
#include "impd_drc_uni_drc.h"
#include "impd_drc_tables.h"
#include "impd_drc_api.h"
#include "impd_drc_uni_drc_eq.h"
#include "impd_drc_uni_drc_filter_bank.h"
#include "impd_drc_gain_enc.h"
#include "impd_drc_struct_def.h"
#include "impd_drc_enc.h"

#define IMPD_DRC_BOUND_CHECK(var, lower_bound, upper_bound) \
  {                                                         \
    var = MIN(var, upper_bound);                            \
    var = MAX(var, lower_bound);                            \
  }

IA_ERRORCODE impd_drc_validate_config_params(ia_drc_input_config *pstr_inp_config) {
  LOOPIDX i, j, k;
  WORD32 curr_start_subband_idx, next_start_subband_idx;
  ia_drc_uni_drc_config_struct *pstr_uni_drc_config = &pstr_inp_config->str_uni_drc_config;
  ia_drc_loudness_info_set_struct *pstr_enc_loudness_info_set =
      &pstr_inp_config->str_enc_loudness_info_set;

  IMPD_DRC_BOUND_CHECK(pstr_uni_drc_config->drc_instructions_uni_drc_count, 0,
                       MAX_DRC_INSTRUCTIONS_COUNT);
  for (i = 0; i < pstr_uni_drc_config->drc_instructions_uni_drc_count; i++) {
    IMPD_DRC_BOUND_CHECK(pstr_uni_drc_config->str_drc_instructions_uni_drc[i].drc_set_id, 0,
                         MAX_DRC_SET_ID);
    IMPD_DRC_BOUND_CHECK(
        pstr_uni_drc_config->str_drc_instructions_uni_drc[i].additional_downmix_id_count, 0,
        ADDITIONAL_DOWNMIX_ID_COUNT_MAX);
    IMPD_DRC_BOUND_CHECK(pstr_uni_drc_config->str_drc_instructions_uni_drc[i].drc_location, 0,
                         MAX_DRC_LOCATION);
    IMPD_DRC_BOUND_CHECK(
        pstr_uni_drc_config->str_drc_instructions_uni_drc[i].drc_set_target_loudness_value_upper,
        MIN_DRC_TARGET_LOUDNESS, 0);
    IMPD_DRC_BOUND_CHECK(
        pstr_uni_drc_config->str_drc_instructions_uni_drc[i].drc_set_target_loudness_value_lower,
        MIN_DRC_TARGET_LOUDNESS, 0);
    for (j = 0; j < MAX_CHANNEL_COUNT; j++) {
      IMPD_DRC_BOUND_CHECK(pstr_uni_drc_config->str_drc_instructions_uni_drc[i].gain_set_index[j],
                           0, GAIN_SET_COUNT_MAX - 1);
    }
    IMPD_DRC_BOUND_CHECK(
        pstr_uni_drc_config->str_drc_instructions_uni_drc[i].num_drc_channel_groups, 0,
        MAX_CHANNEL_GROUP_COUNT);
    for (j = 0; j < pstr_uni_drc_config->str_drc_instructions_uni_drc[i].num_drc_channel_groups;
         j++) {
      IMPD_DRC_BOUND_CHECK(pstr_uni_drc_config->str_drc_instructions_uni_drc[i]
                               .str_gain_modifiers[j]
                               .attenuation_scaling[0],
                           0, MAX_ATTENUATION_SCALING);
      IMPD_DRC_BOUND_CHECK(pstr_uni_drc_config->str_drc_instructions_uni_drc[i]
                               .str_gain_modifiers[j]
                               .amplification_scaling[0],
                           0, MAX_AMPLIFICATION_SCALING);
      IMPD_DRC_BOUND_CHECK(pstr_uni_drc_config->str_drc_instructions_uni_drc[i]
                               .str_gain_modifiers[j]
                               .gain_offset[0],
                           MIN_DRC_GAIN_OFFSET, MAX_DRC_GAIN_OFFSET);
    }
    IMPD_DRC_BOUND_CHECK(pstr_uni_drc_config->str_drc_instructions_uni_drc[i].limiter_peak_target,
                         MIN_LIMITER_PEAK_TARGET, 0.0f);
#ifdef LOUDNESS_LEVELING_SUPPORT
    if (pstr_uni_drc_config->str_drc_instructions_uni_drc[i].drc_set_effect &
        EFFECT_BIT_DUCK_SELF) {
      if (pstr_uni_drc_config->str_drc_instructions_uni_drc[i].leveling_present != 0 &&
          pstr_uni_drc_config->str_drc_instructions_uni_drc[i].leveling_present != 1) {
        pstr_uni_drc_config->str_drc_instructions_uni_drc[i].leveling_present = 0;
      }
      if (pstr_uni_drc_config->str_drc_instructions_uni_drc[i].ducking_only_set_present != 0 &&
          pstr_uni_drc_config->str_drc_instructions_uni_drc[i].ducking_only_set_present != 1) {
        pstr_uni_drc_config->str_drc_instructions_uni_drc[i].ducking_only_set_present = 0;
      }
    }
#endif
  }

  IMPD_DRC_BOUND_CHECK(pstr_uni_drc_config->drc_coefficients_uni_drc_count, 0,
                       MAX_DRC_COEFF_COUNT);
  for (i = 0; i < pstr_uni_drc_config->drc_coefficients_uni_drc_count; i++) {
    IMPD_DRC_BOUND_CHECK(pstr_uni_drc_config->str_drc_coefficients_uni_drc[i].drc_location, 0,
                         MAX_DRC_LOCATION);
    IMPD_DRC_BOUND_CHECK(pstr_uni_drc_config->str_drc_coefficients_uni_drc[i].gain_set_count, 0,
                         MAX_CHANNEL_GROUP_COUNT);
    for (j = 0; j < pstr_uni_drc_config->str_drc_coefficients_uni_drc[i].gain_set_count; j++) {
      IMPD_DRC_BOUND_CHECK(pstr_uni_drc_config->str_drc_coefficients_uni_drc[i]
                               .str_gain_set_params[j]
                               .gain_coding_profile,
                           0, MAX_GAIN_CODING_PROFILE);
      IMPD_DRC_BOUND_CHECK(
          pstr_uni_drc_config->str_drc_coefficients_uni_drc[i].str_gain_set_params[j].band_count,
          0, MAX_BAND_COUNT);
      for (k = 0;
           k <
           pstr_uni_drc_config->str_drc_coefficients_uni_drc[i].str_gain_set_params[j].band_count;
           k++) {
        IMPD_DRC_BOUND_CHECK(pstr_uni_drc_config->str_drc_coefficients_uni_drc[i]
                                 .str_gain_set_params[j]
                                 .gain_params[k]
                                 .nb_points,
                             0, MAX_GAIN_POINTS);
        IMPD_DRC_BOUND_CHECK(pstr_uni_drc_config->str_drc_coefficients_uni_drc[i]
                                 .str_gain_set_params[j]
                                 .gain_params[k]
                                 .drc_characteristic,
                             0, MAX_DRC_CHARACTERISTIC_VALUE);
        IMPD_DRC_BOUND_CHECK(pstr_uni_drc_config->str_drc_coefficients_uni_drc[i]
                                 .str_gain_set_params[j]
                                 .gain_params[k]
                                 .crossover_freq_index,
                             0, MAX_CROSSOVER_FREQ_INDEX);
        IMPD_DRC_BOUND_CHECK(pstr_uni_drc_config->str_drc_coefficients_uni_drc[i]
                                 .str_gain_set_params[j]
                                 .gain_params[k]
                                 .start_sub_band_index,
                             0, STFT256_HOP_SIZE - 1);
        IMPD_DRC_BOUND_CHECK(pstr_uni_drc_config->str_drc_coefficients_uni_drc[i]
                                 .str_gain_set_params[j]
                                 .gain_params[k]
                                 .width,
                             -MAX_FLT_VAL_DB, MAX_FLT_VAL_DB);
        for (WORD32 m = 0; m < pstr_uni_drc_config->str_drc_coefficients_uni_drc[i]
                                   .str_gain_set_params[j]
                                   .gain_params[k]
                                   .nb_points;
             m++) {
          IMPD_DRC_BOUND_CHECK(pstr_uni_drc_config->str_drc_coefficients_uni_drc[i]
                                   .str_gain_set_params[j]
                                   .gain_params[k]
                                   .gain_points[m]
                                   .x,
                               -MAX_FLT_VAL_DB, MAX_FLT_VAL_DB);
          IMPD_DRC_BOUND_CHECK(pstr_uni_drc_config->str_drc_coefficients_uni_drc[i]
                                   .str_gain_set_params[j]
                                   .gain_params[k]
                                   .gain_points[m]
                                   .y,
                               -MAX_FLT_VAL_DB, MAX_FLT_VAL_DB);
        }
      }
      for (k = 0; k < pstr_uni_drc_config->str_drc_coefficients_uni_drc[i]
                              .str_gain_set_params[j]
                              .band_count -
                          1;
           k++) {
        curr_start_subband_idx = pstr_uni_drc_config->str_drc_coefficients_uni_drc[i]
                                     .str_gain_set_params[j]
                                     .gain_params[k]
                                     .start_sub_band_index;
        next_start_subband_idx = pstr_uni_drc_config->str_drc_coefficients_uni_drc[i]
                                     .str_gain_set_params[j]
                                     .gain_params[k + 1]
                                     .start_sub_band_index;
        /* It is assumed that the start index of a subband is greater than
           the start index of its previous subbands for a multiband */
        if (next_start_subband_idx <= curr_start_subband_idx) {
          return IA_EXHEAACE_EXE_NONFATAL_USAC_INVALID_SUBBAND_INDEX;
        }
      }
    }
  }
  IMPD_DRC_BOUND_CHECK(pstr_uni_drc_config->downmix_instructions_count, 0,
                       MAX_DOWNMIX_INSTRUCTION_COUNT);

  IMPD_DRC_BOUND_CHECK(pstr_enc_loudness_info_set->loudness_info_count, 0,
                       MAX_LOUDNESS_INFO_COUNT);
  for (i = 0; i < pstr_enc_loudness_info_set->loudness_info_count; i++) {
    IMPD_DRC_BOUND_CHECK(pstr_enc_loudness_info_set->str_loudness_info[i].sample_peak_level,
                         MIN_SAMPLE_PEAK_LEVEL, MAX_SAMPLE_PEAK_LEVEL);
    IMPD_DRC_BOUND_CHECK(pstr_enc_loudness_info_set->str_loudness_info[i].true_peak_level,
                         MIN_TRUE_PEAK_LEVEL, MAX_TRUE_PEAK_LEVEL);
    IMPD_DRC_BOUND_CHECK(
        pstr_enc_loudness_info_set->str_loudness_info[i].true_peak_level_measurement_system, 0,
        MAX_MEASUREMENT_SYSTEM_TYPE);
    IMPD_DRC_BOUND_CHECK(
        pstr_enc_loudness_info_set->str_loudness_info[i].true_peak_level_reliability, 0,
        MAX_RELIABILITY_TYPE);
    IMPD_DRC_BOUND_CHECK(pstr_enc_loudness_info_set->str_loudness_info[i].measurement_count, 0,
                         MAX_MEASUREMENT_COUNT);
    for (j = 0; j < pstr_enc_loudness_info_set->str_loudness_info[i].measurement_count; j++) {
      IMPD_DRC_BOUND_CHECK(pstr_enc_loudness_info_set->str_loudness_info[i]
                               .str_loudness_measure[j]
                               .method_definition,
                           0, MAX_METHOD_DEFINITION_TYPE);
      IMPD_DRC_BOUND_CHECK(
          pstr_enc_loudness_info_set->str_loudness_info[i].str_loudness_measure[j].method_value,
          MIN_METHOD_VALUE, MAX_METHOD_VALUE);
      IMPD_DRC_BOUND_CHECK(pstr_enc_loudness_info_set->str_loudness_info[i]
                               .str_loudness_measure[j]
                               .measurement_system,
                           0, MAX_MEASUREMENT_SYSTEM_TYPE);
      IMPD_DRC_BOUND_CHECK(
          pstr_enc_loudness_info_set->str_loudness_info[i].str_loudness_measure[j].reliability, 0,
          MAX_RELIABILITY_TYPE);
    }
  }
  IMPD_DRC_BOUND_CHECK(pstr_enc_loudness_info_set->loudness_info_album_count, 0,
                       MAX_LOUDNESS_INFO_COUNT);
  for (i = 0; i < pstr_enc_loudness_info_set->loudness_info_album_count; i++) {
    IMPD_DRC_BOUND_CHECK(pstr_enc_loudness_info_set->str_loudness_info_album[i].sample_peak_level,
                         MIN_SAMPLE_PEAK_LEVEL, MAX_SAMPLE_PEAK_LEVEL);
    IMPD_DRC_BOUND_CHECK(pstr_enc_loudness_info_set->str_loudness_info_album[i].true_peak_level,
                         MIN_TRUE_PEAK_LEVEL, MAX_TRUE_PEAK_LEVEL);
    IMPD_DRC_BOUND_CHECK(
        pstr_enc_loudness_info_set->str_loudness_info_album[i].true_peak_level_measurement_system,
        0, MAX_MEASUREMENT_SYSTEM_TYPE);
    IMPD_DRC_BOUND_CHECK(
        pstr_enc_loudness_info_set->str_loudness_info_album[i].true_peak_level_reliability, 0,
        MAX_RELIABILITY_TYPE);
    IMPD_DRC_BOUND_CHECK(pstr_enc_loudness_info_set->str_loudness_info_album[i].measurement_count,
                         0, MAX_MEASUREMENT_COUNT);
    for (j = 0; j < pstr_enc_loudness_info_set->str_loudness_info_album[i].measurement_count;
         j++) {
      IMPD_DRC_BOUND_CHECK(pstr_enc_loudness_info_set->str_loudness_info_album[i]
                               .str_loudness_measure[j]
                               .method_definition,
                           0, MAX_METHOD_DEFINITION_TYPE);
      IMPD_DRC_BOUND_CHECK(pstr_enc_loudness_info_set->str_loudness_info_album[i]
                               .str_loudness_measure[j]
                               .method_value,
                           MIN_METHOD_VALUE, MAX_METHOD_VALUE);
      IMPD_DRC_BOUND_CHECK(pstr_enc_loudness_info_set->str_loudness_info_album[i]
                               .str_loudness_measure[j]
                               .measurement_system,
                           0, MAX_MEASUREMENT_SYSTEM_TYPE);
      IMPD_DRC_BOUND_CHECK(pstr_enc_loudness_info_set->str_loudness_info_album[i]
                               .str_loudness_measure[j]
                               .reliability,
                           0, MAX_RELIABILITY_TYPE);
    }
  }

  if (pstr_uni_drc_config->uni_drc_config_ext_present != 0 &&
      pstr_uni_drc_config->uni_drc_config_ext_present != 1) {
    pstr_uni_drc_config->uni_drc_config_ext_present = 0;
  }

  if (pstr_uni_drc_config->uni_drc_config_ext_present &&
      (pstr_uni_drc_config->drc_instructions_uni_drc_count > 0 ||
       pstr_uni_drc_config->drc_coefficients_uni_drc_count > 0)) {
    memset(pstr_uni_drc_config->str_drc_instructions_uni_drc, 0,
           sizeof(pstr_uni_drc_config->str_drc_instructions_uni_drc[0]) *
               pstr_uni_drc_config->drc_instructions_uni_drc_count);
    memset(pstr_uni_drc_config->str_drc_coefficients_uni_drc, 0,
           sizeof(pstr_uni_drc_config->str_drc_coefficients_uni_drc[0]) *
               pstr_uni_drc_config->drc_coefficients_uni_drc_count);
    pstr_uni_drc_config->drc_instructions_uni_drc_count = 0;
    pstr_uni_drc_config->drc_coefficients_uni_drc_count = 0;
  }
  if (pstr_uni_drc_config->uni_drc_config_ext_present) {
    ia_drc_uni_drc_config_ext_struct *pstr_uni_drc_config_ext =
        &pstr_uni_drc_config->str_uni_drc_config_ext;
    if (pstr_uni_drc_config_ext->downmix_instructions_v1_present != 0 &&
        pstr_uni_drc_config_ext->downmix_instructions_v1_present != 1) {
      pstr_uni_drc_config_ext->downmix_instructions_v1_present = 0;
    }
    if (pstr_uni_drc_config_ext->downmix_instructions_v1_present) {
      IMPD_DRC_BOUND_CHECK(pstr_uni_drc_config_ext->downmix_instructions_v1_count, 0,
                           DOWNMIX_INSTRUCTIONS_COUNT_MAX);
      for (i = 0; i < pstr_uni_drc_config_ext->downmix_instructions_v1_count; i++) {
        IMPD_DRC_BOUND_CHECK(
            pstr_uni_drc_config_ext->str_downmix_instructions_v1[i].target_layout, 0,
            MAX_TARGET_LAYOUT_COUNT);
        IMPD_DRC_BOUND_CHECK(
            pstr_uni_drc_config_ext->str_downmix_instructions_v1[i].target_ch_count, 0,
            MAX_CHANNEL_COUNT);
      }
    }

    if (pstr_uni_drc_config_ext->drc_coeffs_and_instructions_uni_drc_v1_present != 0 &&
        pstr_uni_drc_config_ext->drc_coeffs_and_instructions_uni_drc_v1_present != 1) {
      pstr_uni_drc_config_ext->drc_coeffs_and_instructions_uni_drc_v1_present = 0;
    }
    if (pstr_uni_drc_config_ext->drc_coeffs_and_instructions_uni_drc_v1_present) {
      IMPD_DRC_BOUND_CHECK(pstr_uni_drc_config_ext->drc_coefficients_uni_drc_v1_count, 0,
                           DRC_COEFFICIENTS_UNIDRC_V1_COUNT_MAX);
      for (i = 0; i < pstr_uni_drc_config_ext->drc_coefficients_uni_drc_v1_count; i++) {
        IMPD_DRC_BOUND_CHECK(
            pstr_uni_drc_config_ext->str_drc_coefficients_uni_drc_v1[i].gain_set_count, 0,
            GAIN_SET_COUNT_MAX);
        for (j = 0;
             j < pstr_uni_drc_config_ext->str_drc_coefficients_uni_drc_v1[i].gain_set_count;
             j++) {
          IMPD_DRC_BOUND_CHECK(pstr_uni_drc_config_ext->str_drc_coefficients_uni_drc_v1[i]
                                   .str_gain_set_params[j]
                                   .gain_coding_profile,
                               0, MAX_GAIN_CODING_PROFILE);
          IMPD_DRC_BOUND_CHECK(pstr_uni_drc_config_ext->str_drc_coefficients_uni_drc_v1[i]
                                   .str_gain_set_params[j]
                                   .band_count,
                               0, MAX_BAND_COUNT);
          for (k = 0; k < pstr_uni_drc_config_ext->str_drc_coefficients_uni_drc_v1[i]
                              .str_gain_set_params[j]
                              .band_count;
               k++) {
            IMPD_DRC_BOUND_CHECK(pstr_uni_drc_config_ext->str_drc_coefficients_uni_drc_v1[i]
                                     .str_gain_set_params[j]
                                     .gain_params[k]
                                     .nb_points,
                                 0, MAX_GAIN_POINTS);
            IMPD_DRC_BOUND_CHECK(pstr_uni_drc_config_ext->str_drc_coefficients_uni_drc_v1[i]
                                     .str_gain_set_params[j]
                                     .gain_params[k]
                                     .drc_characteristic,
                                 0, MAX_DRC_CHARACTERISTIC_VALUE);
            IMPD_DRC_BOUND_CHECK(pstr_uni_drc_config_ext->str_drc_coefficients_uni_drc_v1[i]
                                     .str_gain_set_params[j]
                                     .gain_params[k]
                                     .crossover_freq_index,
                                 0, MAX_CROSSOVER_FREQ_INDEX);
            IMPD_DRC_BOUND_CHECK(pstr_uni_drc_config_ext->str_drc_coefficients_uni_drc_v1[i]
                                     .str_gain_set_params[j]
                                     .gain_params[k]
                                     .start_sub_band_index,
                                 0, STFT256_HOP_SIZE - 1);
            IMPD_DRC_BOUND_CHECK(pstr_uni_drc_config_ext->str_drc_coefficients_uni_drc_v1[i]
                                     .str_gain_set_params[j]
                                     .gain_params[k]
                                     .width,
                                 -MAX_FLT_VAL_DB, MAX_FLT_VAL_DB);
            for (WORD32 m = 0; m < pstr_uni_drc_config_ext->str_drc_coefficients_uni_drc_v1[i]
                                       .str_gain_set_params[j]
                                       .gain_params[k]
                                       .nb_points;
                 m++) {
              IMPD_DRC_BOUND_CHECK(pstr_uni_drc_config_ext->str_drc_coefficients_uni_drc_v1[i]
                                       .str_gain_set_params[j]
                                       .gain_params[k]
                                       .gain_points[m]
                                       .x,
                                   -MAX_FLT_VAL_DB, MAX_FLT_VAL_DB);
              IMPD_DRC_BOUND_CHECK(pstr_uni_drc_config_ext->str_drc_coefficients_uni_drc_v1[i]
                                       .str_gain_set_params[j]
                                       .gain_params[k]
                                       .gain_points[m]
                                       .y,
                                   -MAX_FLT_VAL_DB, MAX_FLT_VAL_DB);
            }
          }
          for (k = 0; k < pstr_uni_drc_config_ext->str_drc_coefficients_uni_drc_v1[i]
                                  .str_gain_set_params[j]
                                  .band_count -
                              1;
               k++) {
            curr_start_subband_idx = pstr_uni_drc_config_ext->str_drc_coefficients_uni_drc_v1[i]
                                         .str_gain_set_params[j]
                                         .gain_params[k]
                                         .start_sub_band_index;
            next_start_subband_idx = pstr_uni_drc_config_ext->str_drc_coefficients_uni_drc_v1[i]
                                         .str_gain_set_params[j]
                                         .gain_params[k + 1]
                                         .start_sub_band_index;
            /* It is assumed that the start index of a subband is greater than
               the start index of its previous subbands for a multiband */
            if (next_start_subband_idx <= curr_start_subband_idx) {
              return IA_EXHEAACE_EXE_NONFATAL_USAC_INVALID_SUBBAND_INDEX;
            }
          }
        }
      }

      IMPD_DRC_BOUND_CHECK(pstr_uni_drc_config_ext->drc_instructions_uni_drc_v1_count, 0,
                           DRC_INSTRUCTIONS_UNIDRC_V1_COUNT_MAX);
      for (i = 0; i < pstr_uni_drc_config_ext->drc_instructions_uni_drc_v1_count; i++) {
        IMPD_DRC_BOUND_CHECK(
            pstr_uni_drc_config_ext->str_drc_instructions_uni_drc_v1[i].drc_set_id, 0,
            MAX_DRC_SET_ID);
        IMPD_DRC_BOUND_CHECK(pstr_uni_drc_config_ext->str_drc_instructions_uni_drc_v1[i]
                                 .additional_downmix_id_count,
                             0, MAX_ADDITIONAL_DOWNMIX_ID);
        IMPD_DRC_BOUND_CHECK(
            pstr_uni_drc_config_ext->str_drc_instructions_uni_drc_v1[i].drc_location, 0,
            MAX_DRC_LOCATION);
        IMPD_DRC_BOUND_CHECK(pstr_uni_drc_config_ext->str_drc_instructions_uni_drc_v1[i]
                                 .drc_set_target_loudness_value_upper,
                             MIN_DRC_TARGET_LOUDNESS, 0);
        IMPD_DRC_BOUND_CHECK(pstr_uni_drc_config_ext->str_drc_instructions_uni_drc_v1[i]
                                 .drc_set_target_loudness_value_lower,
                             MIN_DRC_TARGET_LOUDNESS, 0);
        for (j = 0; j < MAX_CHANNEL_COUNT; j++) {
          IMPD_DRC_BOUND_CHECK(
              pstr_uni_drc_config_ext->str_drc_instructions_uni_drc_v1[i].gain_set_index[j], 0,
              GAIN_SET_COUNT_MAX - 1);
        }
        IMPD_DRC_BOUND_CHECK(
            pstr_uni_drc_config_ext->str_drc_instructions_uni_drc_v1[i].num_drc_channel_groups, 0,
            MAX_CHANNEL_GROUP_COUNT);
        for (j = 0;
             j <
             pstr_uni_drc_config_ext->str_drc_instructions_uni_drc_v1[i].num_drc_channel_groups;
             j++) {
          IMPD_DRC_BOUND_CHECK(pstr_uni_drc_config_ext->str_drc_instructions_uni_drc_v1[i]
                                   .str_gain_modifiers[j]
                                   .attenuation_scaling[0],
                               0, MAX_ATTENUATION_SCALING);
          IMPD_DRC_BOUND_CHECK(pstr_uni_drc_config_ext->str_drc_instructions_uni_drc_v1[i]
                                   .str_gain_modifiers[j]
                                   .amplification_scaling[0],
                               0, MAX_AMPLIFICATION_SCALING);
          IMPD_DRC_BOUND_CHECK(pstr_uni_drc_config_ext->str_drc_instructions_uni_drc_v1[i]
                                   .str_gain_modifiers[j]
                                   .gain_offset[0],
                               MIN_DRC_GAIN_OFFSET, MAX_DRC_GAIN_OFFSET);
        }
        IMPD_DRC_BOUND_CHECK(
            pstr_uni_drc_config_ext->str_drc_instructions_uni_drc_v1[i].limiter_peak_target,
            MIN_LIMITER_PEAK_TARGET, 0.0f);
#ifdef LOUDNESS_LEVELING_SUPPORT
        if (pstr_uni_drc_config_ext->str_drc_instructions_uni_drc_v1[i].drc_set_effect &
            EFFECT_BIT_DUCK_SELF) {
          if (pstr_uni_drc_config_ext->str_drc_instructions_uni_drc_v1[i].leveling_present != 0 &&
              pstr_uni_drc_config_ext->str_drc_instructions_uni_drc_v1[i].leveling_present != 1) {
            pstr_uni_drc_config_ext->str_drc_instructions_uni_drc_v1[i].leveling_present = 0;
          }
          if (pstr_uni_drc_config_ext->str_drc_instructions_uni_drc_v1[i]
                      .ducking_only_set_present != 0 &&
              pstr_uni_drc_config_ext->str_drc_instructions_uni_drc_v1[i]
                      .ducking_only_set_present != 1) {
            pstr_uni_drc_config_ext->str_drc_instructions_uni_drc_v1[i].ducking_only_set_present =
                0;
          }
        }
#endif
      }
    }
  }

  if (pstr_enc_loudness_info_set->loudness_info_set_ext_present != 0 &&
      pstr_enc_loudness_info_set->loudness_info_set_ext_present != 1) {
    pstr_enc_loudness_info_set->loudness_info_set_ext_present = 0;
  }
  if (pstr_enc_loudness_info_set->loudness_info_set_ext_present) {
    ia_drc_loudness_info_set_ext_eq_struct *pstr_enc_loudness_info_set_ext =
        &pstr_enc_loudness_info_set->str_loudness_info_set_extension.str_loudness_info_set_ext_eq;
    IMPD_DRC_BOUND_CHECK(pstr_enc_loudness_info_set_ext->loudness_info_v1_count, 0,
                         MAX_LOUDNESS_INFO_COUNT);
    for (i = 0; i < pstr_enc_loudness_info_set_ext->loudness_info_v1_count; i++) {
      IMPD_DRC_BOUND_CHECK(
          pstr_enc_loudness_info_set_ext->str_loudness_info_v1[i].sample_peak_level,
          MIN_SAMPLE_PEAK_LEVEL, MAX_SAMPLE_PEAK_LEVEL);
      IMPD_DRC_BOUND_CHECK(
          pstr_enc_loudness_info_set_ext->str_loudness_info_v1[i].true_peak_level,
          MIN_TRUE_PEAK_LEVEL, MAX_TRUE_PEAK_LEVEL);
      IMPD_DRC_BOUND_CHECK(pstr_enc_loudness_info_set_ext->str_loudness_info_v1[i]
                               .true_peak_level_measurement_system,
                           0, MAX_MEASUREMENT_SYSTEM_TYPE);
      IMPD_DRC_BOUND_CHECK(
          pstr_enc_loudness_info_set_ext->str_loudness_info_v1[i].true_peak_level_reliability, 0,
          MAX_RELIABILITY_TYPE);
      IMPD_DRC_BOUND_CHECK(
          pstr_enc_loudness_info_set_ext->str_loudness_info_v1[i].measurement_count, 0,
          MAX_MEASUREMENT_COUNT);
      for (j = 0; j < pstr_enc_loudness_info_set_ext->str_loudness_info_v1[i].measurement_count;
           j++) {
        IMPD_DRC_BOUND_CHECK(pstr_enc_loudness_info_set_ext->str_loudness_info_v1[i]
                                 .str_loudness_measure[j]
                                 .method_definition,
                             0, MAX_METHOD_DEFINITION_TYPE);
        IMPD_DRC_BOUND_CHECK(pstr_enc_loudness_info_set_ext->str_loudness_info_v1[i]
                                 .str_loudness_measure[j]
                                 .method_value,
                             MIN_METHOD_VALUE, MAX_METHOD_VALUE);
        IMPD_DRC_BOUND_CHECK(pstr_enc_loudness_info_set_ext->str_loudness_info_v1[i]
                                 .str_loudness_measure[j]
                                 .measurement_system,
                             0, MAX_MEASUREMENT_SYSTEM_TYPE);
        IMPD_DRC_BOUND_CHECK(pstr_enc_loudness_info_set_ext->str_loudness_info_v1[i]
                                 .str_loudness_measure[j]
                                 .reliability,
                             0, MAX_RELIABILITY_TYPE);
      }
    }

    IMPD_DRC_BOUND_CHECK(pstr_enc_loudness_info_set_ext->loudness_info_v1_album_count, 0,
                         MAX_LOUDNESS_INFO_COUNT);
    for (i = 0; i < pstr_enc_loudness_info_set_ext->loudness_info_v1_album_count; i++) {
      IMPD_DRC_BOUND_CHECK(
          pstr_enc_loudness_info_set_ext->str_loudness_info_v1_album[i].sample_peak_level,
          MIN_SAMPLE_PEAK_LEVEL, MAX_SAMPLE_PEAK_LEVEL);
      IMPD_DRC_BOUND_CHECK(
          pstr_enc_loudness_info_set_ext->str_loudness_info_v1_album[i].true_peak_level,
          MIN_TRUE_PEAK_LEVEL, MAX_TRUE_PEAK_LEVEL);
      IMPD_DRC_BOUND_CHECK(pstr_enc_loudness_info_set_ext->str_loudness_info_v1_album[i]
                               .true_peak_level_measurement_system,
                           0, MAX_MEASUREMENT_SYSTEM_TYPE);
      IMPD_DRC_BOUND_CHECK(pstr_enc_loudness_info_set_ext->str_loudness_info_v1_album[i]
                               .true_peak_level_reliability,
                           0, MAX_RELIABILITY_TYPE);
      IMPD_DRC_BOUND_CHECK(
          pstr_enc_loudness_info_set_ext->str_loudness_info_v1_album[i].measurement_count, 0,
          MAX_MEASUREMENT_COUNT);
      for (j = 0;
           j < pstr_enc_loudness_info_set_ext->str_loudness_info_v1_album[i].measurement_count;
           j++) {
        IMPD_DRC_BOUND_CHECK(pstr_enc_loudness_info_set_ext->str_loudness_info_v1_album[i]
                                 .str_loudness_measure[j]
                                 .method_definition,
                             0, MAX_METHOD_DEFINITION_TYPE);
        IMPD_DRC_BOUND_CHECK(pstr_enc_loudness_info_set_ext->str_loudness_info_v1_album[i]
                                 .str_loudness_measure[j]
                                 .method_value,
                             MIN_METHOD_VALUE, MAX_METHOD_VALUE);
        IMPD_DRC_BOUND_CHECK(pstr_enc_loudness_info_set_ext->str_loudness_info_v1_album[i]
                                 .str_loudness_measure[j]
                                 .measurement_system,
                             0, MAX_MEASUREMENT_SYSTEM_TYPE);
        IMPD_DRC_BOUND_CHECK(pstr_enc_loudness_info_set_ext->str_loudness_info_v1_album[i]
                                 .str_loudness_measure[j]
                                 .reliability,
                             0, MAX_RELIABILITY_TYPE);
      }
    }
  }

  return IA_NO_ERROR;
}

static IA_ERRORCODE impd_drc_validate_drc_instructions(
    ia_drc_uni_drc_config_struct *pstr_uni_drc_config) {
  LOOPIDX i, j;
  WORD32 profile_found = FALSE;

  for (j = 0; j < pstr_uni_drc_config->drc_coefficients_uni_drc_count; j++) {
    if (pstr_uni_drc_config->str_drc_coefficients_uni_drc[j].drc_location == 1) {
      profile_found = TRUE;
      break;
    }
  }
  if (pstr_uni_drc_config->uni_drc_config_ext_present &&
      pstr_uni_drc_config->str_uni_drc_config_ext.parametric_drc_present &&
      pstr_uni_drc_config->str_uni_drc_config_ext.str_drc_coeff_parametric_drc.drc_location ==
          1) {
    profile_found = TRUE;
  }
  if ((pstr_uni_drc_config->uni_drc_config_ext_present == 0) && profile_found == FALSE) {
    return IA_EXHEAACE_CONFIG_FATAL_DRC_INVALID_CONFIG;
  }
#ifdef LOUDNESS_LEVELING_SUPPORT
  for (i = 0; i < pstr_uni_drc_config->drc_instructions_uni_drc_count; i++) {
    if (pstr_uni_drc_config->str_drc_instructions_uni_drc[i].drc_set_effect &
        EFFECT_BIT_DUCK_SELF) {
      if (pstr_uni_drc_config->str_drc_instructions_uni_drc[i].leveling_present &&
          pstr_uni_drc_config->str_drc_instructions_uni_drc[i].ducking_only_set_present) {
        if (i < pstr_uni_drc_config->drc_instructions_uni_drc_count - 1) {
          if (pstr_uni_drc_config->str_drc_instructions_uni_drc[i + 1].drc_set_effect !=
                  EFFECT_BIT_DUCK_SELF &&
              pstr_uni_drc_config->str_drc_instructions_uni_drc[i + 1].drc_set_effect !=
                  EFFECT_BIT_DUCK_OTHER) {
            pstr_uni_drc_config->str_drc_instructions_uni_drc[i + 1].drc_set_effect =
                EFFECT_BIT_DUCK_SELF;
          }
          pstr_uni_drc_config->str_drc_instructions_uni_drc[i + 1].leveling_present = 0;
          pstr_uni_drc_config->str_drc_instructions_uni_drc[i + 1].ducking_only_set_present = 0;
        } else {
          pstr_uni_drc_config->str_drc_instructions_uni_drc[i].ducking_only_set_present = 0;
        }
      } else if (!pstr_uni_drc_config->str_drc_instructions_uni_drc[i].leveling_present &&
                 pstr_uni_drc_config->str_drc_instructions_uni_drc[i].ducking_only_set_present) {
        pstr_uni_drc_config->str_drc_instructions_uni_drc[i].ducking_only_set_present = 0;
      }
    }
  }
#endif

  if (pstr_uni_drc_config->uni_drc_config_ext_present) {
    ia_drc_uni_drc_config_ext_struct *pstr_uni_drc_config_ext =
        &pstr_uni_drc_config->str_uni_drc_config_ext;
    profile_found = FALSE;
    for (j = 0; j < pstr_uni_drc_config_ext->drc_coefficients_uni_drc_v1_count; j++) {
      if (pstr_uni_drc_config_ext->str_drc_coefficients_uni_drc_v1[j].drc_location == 1) {
        profile_found = TRUE;
        break;
      }
    }
    if (pstr_uni_drc_config->str_uni_drc_config_ext.parametric_drc_present &&
        pstr_uni_drc_config->str_uni_drc_config_ext.str_drc_coeff_parametric_drc.drc_location ==
            1) {
      profile_found = TRUE;
    }
    if (profile_found == FALSE) {
      return IA_EXHEAACE_CONFIG_FATAL_DRC_INVALID_CONFIG;
    }
#ifdef LOUDNESS_LEVELING_SUPPORT
    for (i = 0; i < pstr_uni_drc_config_ext->drc_instructions_uni_drc_v1_count; i++) {
      if (pstr_uni_drc_config_ext->str_drc_instructions_uni_drc_v1[i].drc_set_effect &
          EFFECT_BIT_DUCK_SELF) {
        if (pstr_uni_drc_config_ext->str_drc_instructions_uni_drc_v1[i].leveling_present &&
            pstr_uni_drc_config_ext->str_drc_instructions_uni_drc_v1[i]
                .ducking_only_set_present) {
          if (i < pstr_uni_drc_config_ext->drc_instructions_uni_drc_v1_count - 1) {
            if (pstr_uni_drc_config_ext->str_drc_instructions_uni_drc_v1[i + 1].drc_set_effect !=
                    EFFECT_BIT_DUCK_SELF &&
                pstr_uni_drc_config_ext->str_drc_instructions_uni_drc_v1[i + 1].drc_set_effect !=
                    EFFECT_BIT_DUCK_OTHER) {
              pstr_uni_drc_config_ext->str_drc_instructions_uni_drc_v1[i + 1].drc_set_effect =
                  EFFECT_BIT_DUCK_SELF;
            }
            pstr_uni_drc_config_ext->str_drc_instructions_uni_drc_v1[i + 1].leveling_present = 0;
            pstr_uni_drc_config_ext->str_drc_instructions_uni_drc_v1[i + 1]
                .ducking_only_set_present = 0;
          } else {
            pstr_uni_drc_config_ext->str_drc_instructions_uni_drc_v1[i].ducking_only_set_present =
                0;
          }
        } else if (!pstr_uni_drc_config_ext->str_drc_instructions_uni_drc_v1[i]
                        .leveling_present &&
                   pstr_uni_drc_config_ext->str_drc_instructions_uni_drc_v1[i]
                       .ducking_only_set_present) {
          pstr_uni_drc_config_ext->str_drc_instructions_uni_drc_v1[i].ducking_only_set_present =
              0;
        }
      }
    }
#endif
  }

  return IA_NO_ERROR;
}

IA_ERRORCODE impd_drc_enc_init(VOID *pstr_drc_state, VOID *ptr_drc_scratch,
                               ia_drc_input_config *pstr_inp_config) {
  IA_ERRORCODE err_code = IA_NO_ERROR;
  WORD32 bit_count = 0;
  ia_drc_enc_state *pstr_drc_state_local = pstr_drc_state;

  pstr_drc_state_local->drc_scratch_mem = ptr_drc_scratch;
  pstr_drc_state_local->drc_scratch_used = 0;

  iusace_create_bit_buffer(&pstr_drc_state_local->str_bit_buf_cfg,
                           pstr_drc_state_local->bit_buf_base_cfg,
                           sizeof(pstr_drc_state_local->bit_buf_base_cfg), 1);

  iusace_create_bit_buffer(&pstr_drc_state_local->str_bit_buf_cfg_ext,
                           pstr_drc_state_local->bit_buf_base_cfg_ext,
                           sizeof(pstr_drc_state_local->bit_buf_base_cfg_ext), 1);

  iusace_create_bit_buffer(&pstr_drc_state_local->str_bit_buf_cfg_tmp,
                           pstr_drc_state_local->bit_buf_base_cfg_tmp,
                           sizeof(pstr_drc_state_local->bit_buf_base_cfg_tmp), 1);

  iusace_create_bit_buffer(&pstr_drc_state_local->str_bit_buf_out,
                           pstr_drc_state_local->bit_buf_base_out,
                           sizeof(pstr_drc_state_local->bit_buf_base_out), 1);

  err_code = impd_drc_gain_enc_init(
      &pstr_drc_state_local->str_gain_enc, &pstr_inp_config->str_uni_drc_config,
      &pstr_inp_config->str_enc_loudness_info_set, pstr_inp_config->str_enc_params.frame_size,
      pstr_inp_config->str_enc_params.sample_rate, pstr_inp_config->str_enc_params.delay_mode,
      pstr_inp_config->str_enc_params.domain);
  if (err_code) {
    return err_code;
  }

  err_code = impd_drc_validate_drc_instructions(&pstr_inp_config->str_uni_drc_config);

  if (err_code & IA_FATAL_ERROR) {
    return IA_EXHEAACE_CONFIG_FATAL_DRC_INVALID_CONFIG;
  }

  pstr_drc_state_local->str_enc_params = pstr_inp_config->str_enc_params;
  pstr_drc_state_local->str_uni_drc_config = pstr_inp_config->str_uni_drc_config;
  pstr_drc_state_local->str_enc_gain_extension = pstr_inp_config->str_enc_gain_extension;
  pstr_drc_state_local->str_gain_enc.str_uni_drc_config = pstr_inp_config->str_uni_drc_config;

  pstr_drc_state_local->str_enc_params.gain_sequence_present = FALSE;
  for (WORD16 k = 0; k < pstr_drc_state_local->str_uni_drc_config.drc_coefficients_uni_drc_count;
       k++) {
    if ((pstr_drc_state_local->str_uni_drc_config.str_drc_coefficients_uni_drc[k].drc_location ==
         1) &&
        (pstr_drc_state_local->str_uni_drc_config.str_drc_coefficients_uni_drc[k].gain_set_count >
         0)) {
      pstr_drc_state_local->str_enc_params.gain_sequence_present = TRUE;
      break;
    }
  }

  if (pstr_drc_state_local->str_enc_params.gain_sequence_present == FALSE) {
    for (WORD16 k = 0; k < pstr_drc_state_local->str_uni_drc_config.str_uni_drc_config_ext
                               .drc_coefficients_uni_drc_v1_count;
         k++) {
      if ((pstr_drc_state_local->str_uni_drc_config.str_uni_drc_config_ext
               .str_drc_coefficients_uni_drc_v1[k]
               .drc_location == 1) &&
          (pstr_drc_state_local->str_uni_drc_config.str_uni_drc_config_ext
               .str_drc_coefficients_uni_drc_v1[k]
               .gain_set_count > 0)) {
        pstr_drc_state_local->str_enc_params.gain_sequence_present = TRUE;
        break;
      }
    }
  }

  err_code = impd_drc_write_uni_drc_config(pstr_drc_state_local, &bit_count, 1);
  if (err_code & IA_FATAL_ERROR) {
    return err_code;
  }
  pstr_drc_state_local->drc_config_data_size_bit = bit_count;

  // Loudness info set
  if (pstr_drc_state_local->str_gain_enc.str_uni_drc_config.loudness_info_set_present == 1) {
    bit_count = 0;
    iusace_reset_bit_buffer(&pstr_drc_state_local->str_bit_buf_cfg_ext);
    memset(pstr_drc_state_local->bit_buf_base_cfg_ext, 0,
           sizeof(MAX_DRC_PAYLOAD_BYTES * sizeof(pstr_drc_state_local->bit_buf_base_cfg_ext[0])));
    err_code = impd_drc_write_loudness_info_set(
        pstr_drc_state, &pstr_drc_state_local->str_bit_buf_cfg_ext, &bit_count, 1);
    if (err_code & IA_FATAL_ERROR) {
      return (err_code);
    }
    pstr_drc_state_local->drc_config_ext_data_size_bit = bit_count;
  }

  return err_code;
}

IA_ERRORCODE impd_loudness_info_init(VOID *pstr_drc_state, ia_drc_input_config *pstr_inp_config) {
  IA_ERRORCODE err_code = IA_NO_ERROR;
  ia_drc_enc_state *pstr_drc_state_local = pstr_drc_state;

  iusace_create_bit_buffer(&pstr_drc_state_local->str_bit_buf_cfg_ext,
                           pstr_drc_state_local->bit_buf_base_cfg_ext,
                           sizeof(pstr_drc_state_local->bit_buf_base_cfg_ext), 1);

  memcpy(&pstr_drc_state_local->str_gain_enc.str_loudness_info_set,
         &pstr_inp_config->str_enc_loudness_info_set, sizeof(ia_drc_loudness_info_set_struct));

  err_code = impd_drc_write_measured_loudness_info(pstr_drc_state_local);
  return err_code;
}

IA_ERRORCODE impd_drc_enc(VOID *pstr_drc_state, FLOAT32 **pptr_input, UWORD32 inp_offset,
                          WORD32 *ptr_bits_written, VOID *pstr_scratch) {
  LOOPIDX i, j, k;
  WORD32 band_count = 0;
  WORD32 stop_sub_band_index;
  WORD32 num_bits_payload = 0;
  UWORD8 is_fft_ready[MAX_NUM_CHANNELS] = {0};
  ia_drc_enc_state *pstr_drc_state_local = pstr_drc_state;
  ia_drc_gain_enc_struct *pstr_gain_enc = &pstr_drc_state_local->str_gain_enc;
  ia_drc_uni_drc_config_struct *pstr_uni_drc_config = &pstr_drc_state_local->str_uni_drc_config;
  ia_drc_compand_struct *pstr_drc_compand;
  ia_drc_stft_gain_calc_struct *pstr_drc_stft_gain_calc;
  IA_ERRORCODE err_code = IA_NO_ERROR;
  ia_drc_coefficients_uni_drc_struct *pstr_drc_coefficients_uni_drc;

  if (pstr_uni_drc_config->str_uni_drc_config_ext.drc_coefficients_uni_drc_v1_count > 0) {
    pstr_drc_coefficients_uni_drc =
        &pstr_uni_drc_config->str_uni_drc_config_ext.str_drc_coefficients_uni_drc_v1[0];
  } else {
    pstr_drc_coefficients_uni_drc = &pstr_uni_drc_config->str_drc_coefficients_uni_drc[0];
  }

  if (pstr_drc_state_local->str_enc_params.gain_sequence_present) {
    for (i = 0; i < MAX_DRC_COEFF_COUNT; i++) {
      for (j = 0; j < GAIN_SET_COUNT_MAX; j++) {
        pstr_drc_stft_gain_calc = &pstr_gain_enc->str_drc_stft_gain_handle[i][j][0];
        pstr_drc_compand = &pstr_gain_enc->str_drc_compand[i][j];
        if ((pstr_drc_compand->is_valid == 0) && (pstr_drc_stft_gain_calc->is_valid == 0)) {
          break;
        }

        if (pstr_drc_compand->is_valid == 0) {
          if (is_fft_ready[pstr_drc_stft_gain_calc->ch_idx] == 0) {
            impd_drc_stft_drc_convert_to_fd(
                pstr_gain_enc, &pptr_input[pstr_drc_stft_gain_calc->ch_idx][inp_offset],
                pstr_drc_stft_gain_calc->ch_idx, pstr_drc_state_local->str_enc_params.frame_size,
                pstr_gain_enc->complex_fft_ptr[pstr_drc_stft_gain_calc->ch_idx], pstr_scratch);
            is_fft_ready[pstr_drc_stft_gain_calc->ch_idx] = 1;
          }

          for (k = 0; k < pstr_drc_coefficients_uni_drc[i].str_gain_set_params[j].band_count;
               k++) {
            if (k == pstr_drc_coefficients_uni_drc[i].str_gain_set_params[j].band_count - 1) {
              stop_sub_band_index = STFT256_HOP_SIZE - 1;
            } else {
              stop_sub_band_index = pstr_drc_coefficients_uni_drc[i]
                                        .str_gain_set_params[j]
                                        .gain_params[k + 1]
                                        .start_sub_band_index -
                                    1;
            }

            impd_drc_stft_drc_gain_calc_process(
                pstr_gain_enc, i, j, k,
                pstr_drc_coefficients_uni_drc[i]
                    .str_gain_set_params[j]
                    .gain_params[k]
                    .start_sub_band_index,
                stop_sub_band_index, pstr_drc_state_local->str_enc_params.frame_size,
                pstr_gain_enc->complex_fft_ptr[pstr_drc_stft_gain_calc->ch_idx],
                pstr_drc_state_local->gain_buffer[band_count + k]);
          }
        } else {
          impd_drc_td_drc_gain_calc_process(pstr_gain_enc, i, j,
                                            pstr_drc_state_local->str_enc_params.frame_size,
                                            &pptr_input[pstr_drc_compand->ch_idx][inp_offset],
                                            pstr_drc_state_local->gain_buffer[band_count]);
        }

        band_count += pstr_drc_coefficients_uni_drc[i].str_gain_set_params[j].band_count;
      }
    }
  }
  err_code = impd_drc_encode_uni_drc_gain(pstr_gain_enc, pstr_drc_state_local->gain_buffer[0],
                                          pstr_scratch);
  if (err_code) {
    return err_code;
  }

  if (pstr_drc_state_local->is_first_drc_process_complete == 1) {
    impd_drc_write_uni_drc_gain(pstr_drc_state_local, &num_bits_payload);
  }

  *ptr_bits_written = num_bits_payload;
  return err_code;
}
