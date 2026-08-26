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

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "ixheaac_type_def.h"
#include "iusace_bitbuffer.h"
#include "impd_drc_common_enc.h"
#include "impd_drc_uni_drc.h"
#include "impd_drc_tables.h"
#include "impd_drc_api.h"
#include "impd_drc_uni_drc_eq.h"
#include "impd_drc_uni_drc_filter_bank.h"
#include "impd_drc_gain_enc.h"
#include "impd_drc_struct_def.h"
#include "impd_drc_enc.h"
#include "impd_drc_user_config.h"

static FLOAT32 impd_drc_get_float_value(FILE *fp) {
  WORD32 i = 0;
  FLOAT32 result = 0.0f;
  CHAR8 line[1024];
  pCHAR8 retval;
  retval = fgets(line, sizeof(line), fp);
  if (retval) {
    pCHAR8 c = line;
    while ((line[0] == '#' || line[0] == '\n') && (retval != NULL)) {
      retval = fgets(line, sizeof(line), fp);
    }
    while (line[i] != ':') {
      c++;
      i++;
    }
    c++;
    result = (FLOAT32)atof(c);
  }
  return result;
}

static WORD32 impd_drc_get_integer_value(FILE *fp) {
  WORD32 i = 0;
  WORD32 result = 0;
  CHAR8 line[1024];
  pCHAR8 retval;
  retval = fgets(line, sizeof(line), fp);
  if (retval) {
    pCHAR8 c = line;
    while ((line[0] == '#' || line[0] == '\n') && (retval != NULL)) {
      retval = fgets(line, sizeof(line), fp);
    }
    while (line[i] != ':') {
      c++;
      i++;
    }
    c++;
    if (c[0] == '0' && c[1] == 'x') {
      result = (WORD32)strtol(c, NULL, 16);
    } else {
      result = atoi(c);
    }
  }
  return result;
}

VOID ixheaace_read_drc_config_params(FILE *fp, ia_drc_enc_params_struct *pstr_enc_params,
                                     ia_drc_uni_drc_config_struct *pstr_uni_drc_config,
                                     ia_drc_loudness_info_set_struct *pstr_enc_loudness_info_set,
                                     ia_drc_uni_drc_gain_ext_struct *pstr_enc_gain_extension,
                                     WORD32 in_ch) {
  WORD32 n, g, s, m, ch, p;
  WORD32 gain_set_channels;

  pstr_enc_params->delay_mode = DELAY_MODE_REGULAR_DELAY;
  pstr_uni_drc_config->sample_rate_present = 1;
  pstr_uni_drc_config->str_drc_coefficients_uni_drc->drc_frame_size_present = 0;
  pstr_uni_drc_config->loudness_info_set_present = 1;
#ifdef LOUDNESS_LEVELING_SUPPORT
  WORD16 loudness_leveling_extension_present = 0;
  UWORD32 config_extension_count = 0;
#endif
  /***********  str_drc_instructions_uni_drc  *************/

  pstr_uni_drc_config->drc_instructions_uni_drc_count = impd_drc_get_integer_value(fp);
  pstr_uni_drc_config->drc_instructions_uni_drc_count =
      MIN(pstr_uni_drc_config->drc_instructions_uni_drc_count, MAX_DRC_INSTRUCTIONS_COUNT);
  for (n = 0; n < pstr_uni_drc_config->drc_instructions_uni_drc_count; n++) {
    ia_drc_instructions_uni_drc *pstr_drc_instructions_uni_drc =
        &pstr_uni_drc_config->str_drc_instructions_uni_drc[n];
    pstr_drc_instructions_uni_drc->drc_set_id = n + 1;
    pstr_drc_instructions_uni_drc->downmix_id = impd_drc_get_integer_value(fp);
    pstr_drc_instructions_uni_drc->additional_downmix_id_present = 0;
    pstr_drc_instructions_uni_drc->additional_downmix_id_count = 0;
    pstr_drc_instructions_uni_drc->drc_location = 1;
    pstr_drc_instructions_uni_drc->depends_on_drc_set_present = 0;
    pstr_drc_instructions_uni_drc->depends_on_drc_set = 0;
    pstr_drc_instructions_uni_drc->no_independent_use = 0;
    pstr_drc_instructions_uni_drc->drc_set_effect = impd_drc_get_integer_value(fp);
    pstr_drc_instructions_uni_drc->drc_set_target_loudness_present = 0;
    pstr_drc_instructions_uni_drc->drc_set_target_loudness_value_upper = 0;
    pstr_drc_instructions_uni_drc->drc_set_target_loudness_value_lower_present = 0;
    pstr_drc_instructions_uni_drc->drc_set_target_loudness_value_lower = 0;

    gain_set_channels = impd_drc_get_integer_value(fp);
    gain_set_channels = MIN(gain_set_channels, MAX_CHANNEL_COUNT);
    for (ch = 0; ch < gain_set_channels; ch++) {
      pstr_drc_instructions_uni_drc->gain_set_index[ch] = impd_drc_get_integer_value(fp);
    }
    for (; ch < MAX_CHANNEL_COUNT; ch++) {
      if (gain_set_channels > 0) {
        pstr_drc_instructions_uni_drc->gain_set_index[ch] =
            pstr_drc_instructions_uni_drc->gain_set_index[gain_set_channels - 1];
      } else {
        pstr_drc_instructions_uni_drc->gain_set_index[ch] = 0;
      }
    }

    pstr_drc_instructions_uni_drc->num_drc_channel_groups = impd_drc_get_integer_value(fp);
    pstr_drc_instructions_uni_drc->num_drc_channel_groups =
        MIN(pstr_drc_instructions_uni_drc->num_drc_channel_groups, MAX_CHANNEL_GROUP_COUNT);
    for (g = 0; g < pstr_drc_instructions_uni_drc->num_drc_channel_groups; g++) {
      pstr_drc_instructions_uni_drc->str_gain_modifiers[g].gain_scaling_present[0] = 0;
      pstr_drc_instructions_uni_drc->str_gain_modifiers[g].attenuation_scaling[0] = 1.5f;
      pstr_drc_instructions_uni_drc->str_gain_modifiers[g].amplification_scaling[0] = 1.5f;
      pstr_drc_instructions_uni_drc->str_gain_modifiers[g].gain_offset_present[0] = 0;
      pstr_drc_instructions_uni_drc->str_gain_modifiers[g].gain_offset[0] = 8.0f;
    }

    pstr_drc_instructions_uni_drc->limiter_peak_target_present = 0;
    pstr_drc_instructions_uni_drc->limiter_peak_target = 0.0f;
    pstr_drc_instructions_uni_drc->drc_instructions_type = 0;
    pstr_drc_instructions_uni_drc->mae_group_id = 0;
    pstr_drc_instructions_uni_drc->mae_group_preset_id = 0;

#ifdef LOUDNESS_LEVELING_SUPPORT
    if (pstr_drc_instructions_uni_drc->drc_set_effect & EFFECT_BIT_DUCK_SELF) {
      pstr_drc_instructions_uni_drc->leveling_present = impd_drc_get_integer_value(fp);
      pstr_drc_instructions_uni_drc->ducking_only_set_present = impd_drc_get_integer_value(fp);
      loudness_leveling_extension_present = 1;
    }
#endif  // LOUDNESS_LEVELING_SUPPORT
  }
  /***********  str_drc_coefficients_uni_drc  *************/

  pstr_uni_drc_config->drc_coefficients_uni_drc_count = impd_drc_get_integer_value(fp);
  pstr_uni_drc_config->drc_coefficients_uni_drc_count =
      MIN(pstr_uni_drc_config->drc_coefficients_uni_drc_count, MAX_DRC_COEFF_COUNT);
  for (n = 0; n < pstr_uni_drc_config->drc_coefficients_uni_drc_count; n++) {
    ia_drc_coefficients_uni_drc_struct *pstr_drc_coefficients_uni_drc =
        &pstr_uni_drc_config->str_drc_coefficients_uni_drc[n];
    pstr_drc_coefficients_uni_drc->drc_location = 1;
    pstr_drc_coefficients_uni_drc->gain_set_count = impd_drc_get_integer_value(fp);
    pstr_drc_coefficients_uni_drc->gain_set_count =
        MIN(pstr_drc_coefficients_uni_drc->gain_set_count, GAIN_SET_COUNT_MAX);
    for (s = 0; s < pstr_drc_coefficients_uni_drc->gain_set_count; s++) {
      pstr_drc_coefficients_uni_drc->str_gain_set_params[s].gain_coding_profile = 0;
      pstr_drc_coefficients_uni_drc->str_gain_set_params[s].gain_interpolation_type = 1;
      pstr_drc_coefficients_uni_drc->str_gain_set_params[s].full_frame = 0;
      pstr_drc_coefficients_uni_drc->str_gain_set_params[s].time_alignment = 0;
      pstr_drc_coefficients_uni_drc->str_gain_set_params[s].time_delta_min_present = 0;
      pstr_drc_coefficients_uni_drc->str_gain_set_params[s].band_count =
          impd_drc_get_integer_value(fp);
      pstr_drc_coefficients_uni_drc->str_gain_set_params[s].band_count =
          MIN(pstr_drc_coefficients_uni_drc->str_gain_set_params[s].band_count, MAX_BAND_COUNT);
      if (pstr_drc_coefficients_uni_drc->str_gain_set_params[s].band_count == 1) {
        pstr_drc_coefficients_uni_drc->str_gain_set_params[s].gain_params[0].nb_points =
            impd_drc_get_integer_value(fp);
        pstr_drc_coefficients_uni_drc->str_gain_set_params[s].gain_params[0].nb_points =
            MIN(pstr_drc_coefficients_uni_drc->str_gain_set_params[s].gain_params[0].nb_points,
                MAX_GAIN_POINTS);
        for (p = 0;
             p < pstr_drc_coefficients_uni_drc->str_gain_set_params[s].gain_params[0].nb_points;
             p++) {
          pstr_drc_coefficients_uni_drc->str_gain_set_params[s].gain_params[0].gain_points[p].x =
              impd_drc_get_float_value(fp);
          pstr_drc_coefficients_uni_drc->str_gain_set_params[s].gain_params[0].gain_points[p].y =
              impd_drc_get_float_value(fp);
        }
        pstr_drc_coefficients_uni_drc->str_gain_set_params[s].gain_params[0].width =
            impd_drc_get_float_value(fp);
        pstr_drc_coefficients_uni_drc->str_gain_set_params[s].gain_params[0].attack =
            impd_drc_get_float_value(fp);
        pstr_drc_coefficients_uni_drc->str_gain_set_params[s].gain_params[0].decay =
            impd_drc_get_float_value(fp);
        pstr_drc_coefficients_uni_drc->str_gain_set_params[s].gain_params[0].drc_characteristic =
            0;
        pstr_drc_coefficients_uni_drc->str_gain_set_params[s]
            .gain_params[0]
            .crossover_freq_index = 0;
      } else {
        for (m = 0; m < pstr_drc_coefficients_uni_drc->str_gain_set_params[s].band_count; m++) {
          pstr_drc_coefficients_uni_drc->str_gain_set_params[s].gain_params[m].nb_points =
              impd_drc_get_integer_value(fp);
          pstr_drc_coefficients_uni_drc->str_gain_set_params[s].gain_params[m].nb_points =
              MIN(pstr_drc_coefficients_uni_drc->str_gain_set_params[s].gain_params[m].nb_points,
                  MAX_GAIN_POINTS);
          for (p = 0;
               p < pstr_drc_coefficients_uni_drc->str_gain_set_params[s].gain_params[m].nb_points;
               p++) {
            pstr_drc_coefficients_uni_drc->str_gain_set_params[s]
                .gain_params[m]
                .gain_points[p]
                .x = impd_drc_get_float_value(fp);
            pstr_drc_coefficients_uni_drc->str_gain_set_params[s]
                .gain_params[m]
                .gain_points[p]
                .y = impd_drc_get_float_value(fp);
          }
          pstr_drc_coefficients_uni_drc->str_gain_set_params[s].gain_params[m].width =
              impd_drc_get_float_value(fp);
          pstr_drc_coefficients_uni_drc->str_gain_set_params[s].gain_params[m].attack =
              impd_drc_get_float_value(fp);
          pstr_drc_coefficients_uni_drc->str_gain_set_params[s].gain_params[m].decay =
              impd_drc_get_float_value(fp);
          pstr_drc_coefficients_uni_drc->str_gain_set_params[s].drc_band_type = 0;
          pstr_drc_coefficients_uni_drc->str_gain_set_params[s]
              .gain_params[m]
              .start_sub_band_index = impd_drc_get_integer_value(fp);
          pstr_drc_coefficients_uni_drc->str_gain_set_params[s]
              .gain_params[m]
              .drc_characteristic = 0;
          pstr_drc_coefficients_uni_drc->str_gain_set_params[s]
              .gain_params[m]
              .crossover_freq_index = 0;
        }
      }
    }
  }
  /***********  str_channel_layout  *************/

  pstr_uni_drc_config->str_channel_layout.layout_signaling_present = 0;
  pstr_uni_drc_config->str_channel_layout.defined_layout = 0;
  pstr_uni_drc_config->str_channel_layout.speaker_position[0] = 0;

  /***********  str_downmix_instructions  *************/

  pstr_uni_drc_config->downmix_instructions_count = 0;

  pstr_uni_drc_config->drc_description_basic_present = 0;

  pstr_uni_drc_config->uni_drc_config_ext_present = impd_drc_get_integer_value(fp);
  if (pstr_uni_drc_config->uni_drc_config_ext_present == 1) {
#ifdef LOUDNESS_LEVELING_SUPPORT
    pstr_uni_drc_config->str_uni_drc_config_ext.uni_drc_config_ext_type[config_extension_count] =
        UNIDRC_CONF_EXT_V1;
    config_extension_count++;
#else
    pstr_uni_drc_config->str_uni_drc_config_ext.uni_drc_config_ext_type[0] = UNIDRC_CONF_EXT_V1;
#endif

    pstr_uni_drc_config->str_uni_drc_config_ext.downmix_instructions_v1_present =
        impd_drc_get_integer_value(fp);
    if (pstr_uni_drc_config->str_uni_drc_config_ext.downmix_instructions_v1_present == 1) {

      /***********  str_downmix_instructions_v1  *************/
      pstr_uni_drc_config->str_uni_drc_config_ext.downmix_instructions_v1_count = impd_drc_get_integer_value(fp);

      pstr_uni_drc_config->str_uni_drc_config_ext.downmix_instructions_v1_count =
        MIN(pstr_uni_drc_config->str_uni_drc_config_ext.downmix_instructions_v1_count, MAX_DOWNMIX_INSTRUCTION_COUNT);
      for (n = 0; n < pstr_uni_drc_config->str_uni_drc_config_ext.downmix_instructions_v1_count; n++)
      {
        ia_drc_downmix_instructions_struct *pstr_downmix_instructions_v1 =
          &pstr_uni_drc_config->str_uni_drc_config_ext.str_downmix_instructions_v1[n];
        pstr_downmix_instructions_v1->downmix_id = n + 1;
        pstr_downmix_instructions_v1->target_ch_count = 1;
        pstr_downmix_instructions_v1->target_layout = impd_drc_get_integer_value(fp);
        pstr_downmix_instructions_v1->downmix_coefficients_present = 1;
        if (pstr_downmix_instructions_v1->downmix_coefficients_present) {
          FLOAT32 dwn_mix_coeff = 0.0f;
          for (s = 0; s < pstr_downmix_instructions_v1->target_layout; s++) {
            dwn_mix_coeff = impd_drc_get_float_value(fp);
            for (ch = 0; ch < in_ch; ch++) {
              pstr_downmix_instructions_v1->downmix_coeff[in_ch * s + ch] = dwn_mix_coeff;
            }
          }
        }
      }
    }

    pstr_uni_drc_config->str_uni_drc_config_ext.drc_coeffs_and_instructions_uni_drc_v1_present =
        impd_drc_get_integer_value(fp);
    if (pstr_uni_drc_config->str_uni_drc_config_ext
            .drc_coeffs_and_instructions_uni_drc_v1_present == 1) {
      /***********  str_drc_coefficients_uni_drc_v1  *************/
      pstr_uni_drc_config->str_uni_drc_config_ext.drc_coefficients_uni_drc_v1_count = impd_drc_get_integer_value(fp);

      pstr_uni_drc_config->str_uni_drc_config_ext.drc_coefficients_uni_drc_v1_count =
        MIN(pstr_uni_drc_config->str_uni_drc_config_ext.drc_coefficients_uni_drc_v1_count, MAX_DRC_COEFF_COUNT);
      for (n = 0; n < pstr_uni_drc_config->str_uni_drc_config_ext.drc_coefficients_uni_drc_v1_count; n++) {
        ia_drc_coefficients_uni_drc_struct *pstr_drc_coefficients_uni_drc_v1 =
          &pstr_uni_drc_config->str_uni_drc_config_ext.str_drc_coefficients_uni_drc_v1[n];
        pstr_drc_coefficients_uni_drc_v1->drc_location = 1;
        pstr_drc_coefficients_uni_drc_v1->gain_set_count = impd_drc_get_integer_value(fp);
        pstr_drc_coefficients_uni_drc_v1->gain_set_count =
          MIN(pstr_drc_coefficients_uni_drc_v1->gain_set_count, GAIN_SET_COUNT_MAX);
        for (s = 0; s < pstr_drc_coefficients_uni_drc_v1->gain_set_count; s++) {
          pstr_drc_coefficients_uni_drc_v1->str_gain_set_params[s].gain_coding_profile = 0;
          pstr_drc_coefficients_uni_drc_v1->str_gain_set_params[s].gain_interpolation_type = 1;
          pstr_drc_coefficients_uni_drc_v1->str_gain_set_params[s].full_frame = 0;
          pstr_drc_coefficients_uni_drc_v1->str_gain_set_params[s].time_alignment = 0;
          pstr_drc_coefficients_uni_drc_v1->str_gain_set_params[s].time_delta_min_present = 0;
          pstr_drc_coefficients_uni_drc_v1->str_gain_set_params[s].band_count =
            impd_drc_get_integer_value(fp);
          pstr_drc_coefficients_uni_drc_v1->str_gain_set_params[s].band_count =
            MIN(pstr_drc_coefficients_uni_drc_v1->str_gain_set_params[s].band_count, MAX_BAND_COUNT);
          if (pstr_drc_coefficients_uni_drc_v1->str_gain_set_params[s].band_count == 1) {
            pstr_drc_coefficients_uni_drc_v1->str_gain_set_params[s].gain_params[0].nb_points =
              impd_drc_get_integer_value(fp);
            pstr_drc_coefficients_uni_drc_v1->str_gain_set_params[s].gain_params[0].nb_points =
              MIN(pstr_drc_coefficients_uni_drc_v1->str_gain_set_params[s].gain_params[0].nb_points,
                MAX_GAIN_POINTS);
            for (p = 0;
              p < pstr_drc_coefficients_uni_drc_v1->str_gain_set_params[s].gain_params[0].nb_points;
              p++) {
              pstr_drc_coefficients_uni_drc_v1->str_gain_set_params[s].gain_params[0].gain_points[p].x =
                impd_drc_get_float_value(fp);
              pstr_drc_coefficients_uni_drc_v1->str_gain_set_params[s].gain_params[0].gain_points[p].y =
                impd_drc_get_float_value(fp);
            }
            pstr_drc_coefficients_uni_drc_v1->str_gain_set_params[s].gain_params[0].width =
              impd_drc_get_float_value(fp);
            pstr_drc_coefficients_uni_drc_v1->str_gain_set_params[s].gain_params[0].attack =
              impd_drc_get_float_value(fp);
            pstr_drc_coefficients_uni_drc_v1->str_gain_set_params[s].gain_params[0].decay =
              impd_drc_get_float_value(fp);
            pstr_drc_coefficients_uni_drc_v1->str_gain_set_params[s].gain_params[0].drc_characteristic =
              0;
            pstr_drc_coefficients_uni_drc_v1->str_gain_set_params[s]
              .gain_params[0]
              .crossover_freq_index = 0;
          }
          else {
            for (m = 0; m < pstr_drc_coefficients_uni_drc_v1->str_gain_set_params[s].band_count; m++) {
              pstr_drc_coefficients_uni_drc_v1->str_gain_set_params[s].gain_params[m].nb_points =
                impd_drc_get_integer_value(fp);
              pstr_drc_coefficients_uni_drc_v1->str_gain_set_params[s].gain_params[m].nb_points =
                MIN(pstr_drc_coefficients_uni_drc_v1->str_gain_set_params[s].gain_params[m].nb_points,
                  MAX_GAIN_POINTS);
              for (p = 0;
                p < pstr_drc_coefficients_uni_drc_v1->str_gain_set_params[s].gain_params[m].nb_points;
                p++) {
                pstr_drc_coefficients_uni_drc_v1->str_gain_set_params[s]
                  .gain_params[m]
                  .gain_points[p]
                  .x = impd_drc_get_float_value(fp);
                pstr_drc_coefficients_uni_drc_v1->str_gain_set_params[s]
                  .gain_params[m]
                  .gain_points[p]
                  .y = impd_drc_get_float_value(fp);
              }
              pstr_drc_coefficients_uni_drc_v1->str_gain_set_params[s].gain_params[m].width =
                impd_drc_get_float_value(fp);
              pstr_drc_coefficients_uni_drc_v1->str_gain_set_params[s].gain_params[m].attack =
                impd_drc_get_float_value(fp);
              pstr_drc_coefficients_uni_drc_v1->str_gain_set_params[s].gain_params[m].decay =
                impd_drc_get_float_value(fp);
              pstr_drc_coefficients_uni_drc_v1->str_gain_set_params[s].drc_band_type = 0;
              pstr_drc_coefficients_uni_drc_v1->str_gain_set_params[s]
                .gain_params[m]
                .start_sub_band_index = impd_drc_get_integer_value(fp);
              pstr_drc_coefficients_uni_drc_v1->str_gain_set_params[s]
                .gain_params[m]
                .drc_characteristic = 0;
              pstr_drc_coefficients_uni_drc_v1->str_gain_set_params[s]
                .gain_params[m]
                .crossover_freq_index = 0;
            }
          }
        }
      }

      /***********  str_drc_instructions_uni_drc_v1  *************/
      pstr_uni_drc_config->str_uni_drc_config_ext.drc_instructions_uni_drc_v1_count = impd_drc_get_integer_value(fp);
      pstr_uni_drc_config->str_uni_drc_config_ext.drc_instructions_uni_drc_v1_count =
        MIN(pstr_uni_drc_config->str_uni_drc_config_ext.drc_instructions_uni_drc_v1_count, MAX_DRC_INSTRUCTIONS_COUNT);
      for (n = 0; n < pstr_uni_drc_config->str_uni_drc_config_ext.drc_instructions_uni_drc_v1_count; n++) {
        ia_drc_instructions_uni_drc *pstr_drc_instructions_uni_drc =
          &pstr_uni_drc_config->str_uni_drc_config_ext.str_drc_instructions_uni_drc_v1[n];
        pstr_drc_instructions_uni_drc->drc_set_id = n + 1;
        pstr_drc_instructions_uni_drc->downmix_id = impd_drc_get_integer_value(fp);
        pstr_drc_instructions_uni_drc->additional_downmix_id_present = 0;
        pstr_drc_instructions_uni_drc->additional_downmix_id_count = 0;
        pstr_drc_instructions_uni_drc->drc_location = 1;
        pstr_drc_instructions_uni_drc->depends_on_drc_set_present = 0;
        pstr_drc_instructions_uni_drc->depends_on_drc_set = 0;
        pstr_drc_instructions_uni_drc->no_independent_use = 0;
        pstr_drc_instructions_uni_drc->drc_set_effect = impd_drc_get_integer_value(fp);
        pstr_drc_instructions_uni_drc->drc_set_target_loudness_present = 0;
        pstr_drc_instructions_uni_drc->drc_set_target_loudness_value_upper = 0;
        pstr_drc_instructions_uni_drc->drc_set_target_loudness_value_lower_present = 0;
        pstr_drc_instructions_uni_drc->drc_set_target_loudness_value_lower = 0;

        gain_set_channels = impd_drc_get_integer_value(fp);
        gain_set_channels = MIN(gain_set_channels, MAX_CHANNEL_COUNT);
        for (ch = 0; ch < gain_set_channels; ch++) {
          pstr_drc_instructions_uni_drc->gain_set_index[ch] = impd_drc_get_integer_value(fp);
        }
        for (; ch < MAX_CHANNEL_COUNT; ch++) {
          if (gain_set_channels > 0) {
            pstr_drc_instructions_uni_drc->gain_set_index[ch] =
              pstr_drc_instructions_uni_drc->gain_set_index[gain_set_channels - 1];
          }
          else {
            pstr_drc_instructions_uni_drc->gain_set_index[ch] = 0;
          }
        }

        pstr_drc_instructions_uni_drc->num_drc_channel_groups = impd_drc_get_integer_value(fp);
        pstr_drc_instructions_uni_drc->num_drc_channel_groups =
          MIN(pstr_drc_instructions_uni_drc->num_drc_channel_groups, MAX_CHANNEL_GROUP_COUNT);
        for (g = 0; g < pstr_drc_instructions_uni_drc->num_drc_channel_groups; g++) {
          pstr_drc_instructions_uni_drc->str_gain_modifiers[g].gain_scaling_present[0] = 0;
          pstr_drc_instructions_uni_drc->str_gain_modifiers[g].attenuation_scaling[0] = 1.5f;
          pstr_drc_instructions_uni_drc->str_gain_modifiers[g].amplification_scaling[0] = 1.5f;
          pstr_drc_instructions_uni_drc->str_gain_modifiers[g].gain_offset_present[0] = 0;
          pstr_drc_instructions_uni_drc->str_gain_modifiers[g].gain_offset[0] = 8.0f;
        }

        pstr_drc_instructions_uni_drc->limiter_peak_target_present = 0;
        pstr_drc_instructions_uni_drc->limiter_peak_target = 0.0f;
        pstr_drc_instructions_uni_drc->drc_instructions_type = 0;
        pstr_drc_instructions_uni_drc->mae_group_id = 0;
        pstr_drc_instructions_uni_drc->mae_group_preset_id = 0;
#ifdef LOUDNESS_LEVELING_SUPPORT
        if (pstr_drc_instructions_uni_drc->drc_set_effect & EFFECT_BIT_DUCK_SELF) {
          pstr_drc_instructions_uni_drc->leveling_present = impd_drc_get_integer_value(fp);
          pstr_drc_instructions_uni_drc->ducking_only_set_present =
              impd_drc_get_integer_value(fp);
          loudness_leveling_extension_present = 1;
        }
#endif  // LOUDNESS_LEVELING_SUPPORT
      }
    }
  }

#ifdef LOUDNESS_LEVELING_SUPPORT
  if (loudness_leveling_extension_present) {
    pstr_uni_drc_config->str_uni_drc_config_ext.uni_drc_config_ext_type[config_extension_count] =
        UNIDRCCONFEXT_LEVELING;
    config_extension_count++;
  }
#endif

  /*********************   str_loudness_info   *********************/
  pstr_enc_loudness_info_set->loudness_info_count = impd_drc_get_integer_value(fp);
  pstr_enc_loudness_info_set->loudness_info_count =
      MIN(pstr_enc_loudness_info_set->loudness_info_count, MAX_LOUDNESS_INFO_COUNT);
  for (n = 0; n < pstr_enc_loudness_info_set->loudness_info_count; n++) {
    pstr_enc_loudness_info_set->str_loudness_info[n].drc_set_id = impd_drc_get_integer_value(fp);
    pstr_enc_loudness_info_set->str_loudness_info[n].downmix_id = impd_drc_get_integer_value(fp);
    pstr_enc_loudness_info_set->str_loudness_info[n].sample_peak_level_present =
        impd_drc_get_integer_value(fp);
    if (1 == pstr_enc_loudness_info_set->str_loudness_info[n].sample_peak_level_present) {
      pstr_enc_loudness_info_set->str_loudness_info[n].sample_peak_level =
          impd_drc_get_float_value(fp);
    }
    pstr_enc_loudness_info_set->str_loudness_info[n].true_peak_level_present =
        impd_drc_get_integer_value(fp);
    if (1 == pstr_enc_loudness_info_set->str_loudness_info[n].true_peak_level_present) {
      pstr_enc_loudness_info_set->str_loudness_info[n].true_peak_level =
          impd_drc_get_float_value(fp);
      pstr_enc_loudness_info_set->str_loudness_info[n].true_peak_level_measurement_system =
          impd_drc_get_integer_value(fp);
      pstr_enc_loudness_info_set->str_loudness_info[n].true_peak_level_reliability =
          impd_drc_get_integer_value(fp);
    }
    pstr_enc_loudness_info_set->str_loudness_info[n].measurement_count =
        impd_drc_get_integer_value(fp);
    pstr_enc_loudness_info_set->str_loudness_info[n].measurement_count =
        MIN(pstr_enc_loudness_info_set->str_loudness_info[n].measurement_count,
            MAX_MEASUREMENT_COUNT);

    for (m = 0; m < pstr_enc_loudness_info_set->str_loudness_info[n].measurement_count; m++) {
      pstr_enc_loudness_info_set->str_loudness_info[n].str_loudness_measure[m].method_definition =
          impd_drc_get_integer_value(fp);
      pstr_enc_loudness_info_set->str_loudness_info[n].str_loudness_measure[m].method_value =
          impd_drc_get_float_value(fp);
      pstr_enc_loudness_info_set->str_loudness_info[n]
          .str_loudness_measure[m]
          .measurement_system = impd_drc_get_integer_value(fp);
      pstr_enc_loudness_info_set->str_loudness_info[n].str_loudness_measure[m].reliability =
          impd_drc_get_integer_value(fp);
    }
  }

  pstr_enc_loudness_info_set->loudness_info_album_count = impd_drc_get_integer_value(fp);
  pstr_enc_loudness_info_set->loudness_info_album_count =
      MIN(pstr_enc_loudness_info_set->loudness_info_album_count, MAX_LOUDNESS_INFO_COUNT);
  for (n = 0; n < pstr_enc_loudness_info_set->loudness_info_album_count; n++) {
    pstr_enc_loudness_info_set->str_loudness_info_album[n].drc_set_id =
        impd_drc_get_integer_value(fp);
    pstr_enc_loudness_info_set->str_loudness_info_album[n].downmix_id =
        impd_drc_get_integer_value(fp);
    pstr_enc_loudness_info_set->str_loudness_info_album[n].sample_peak_level_present =
        impd_drc_get_integer_value(fp);
    if (1 == pstr_enc_loudness_info_set->str_loudness_info_album[n].sample_peak_level_present) {
      pstr_enc_loudness_info_set->str_loudness_info_album[n].sample_peak_level =
          impd_drc_get_float_value(fp);
    }
    pstr_enc_loudness_info_set->str_loudness_info_album[n].true_peak_level_present =
        impd_drc_get_integer_value(fp);
    if (1 == pstr_enc_loudness_info_set->str_loudness_info_album[n].true_peak_level_present) {
      pstr_enc_loudness_info_set->str_loudness_info_album[n].true_peak_level =
          impd_drc_get_float_value(fp);
      pstr_enc_loudness_info_set->str_loudness_info_album[n].true_peak_level_measurement_system =
          impd_drc_get_integer_value(fp);
      pstr_enc_loudness_info_set->str_loudness_info_album[n].true_peak_level_reliability =
          impd_drc_get_integer_value(fp);
    }
    pstr_enc_loudness_info_set->str_loudness_info_album[n].measurement_count =
        impd_drc_get_integer_value(fp);
    pstr_enc_loudness_info_set->str_loudness_info_album[n].measurement_count =
        MIN(pstr_enc_loudness_info_set->str_loudness_info_album[n].measurement_count,
            MAX_MEASUREMENT_COUNT);
    for (m = 0; m < pstr_enc_loudness_info_set->str_loudness_info_album[n].measurement_count;
         m++) {
      pstr_enc_loudness_info_set->str_loudness_info_album[n]
          .str_loudness_measure[m]
          .method_definition = impd_drc_get_integer_value(fp);
      pstr_enc_loudness_info_set->str_loudness_info_album[n]
          .str_loudness_measure[m]
          .method_value = impd_drc_get_float_value(fp);
      pstr_enc_loudness_info_set->str_loudness_info_album[n]
          .str_loudness_measure[m]
          .measurement_system = impd_drc_get_integer_value(fp);
      pstr_enc_loudness_info_set->str_loudness_info_album[n].str_loudness_measure[m].reliability =
          impd_drc_get_integer_value(fp);
    }
  }

  /*********************   loudness info set extension   *********************/
  pstr_enc_loudness_info_set->loudness_info_set_ext_present = impd_drc_get_integer_value(fp);
  if (pstr_enc_loudness_info_set->loudness_info_set_ext_present == 1) {
    ia_drc_loudness_info_set_ext_eq_struct *pstr_loudness_info_set_ext_eq =
        &pstr_enc_loudness_info_set->str_loudness_info_set_extension.str_loudness_info_set_ext_eq;

    pstr_enc_loudness_info_set->str_loudness_info_set_extension.loudness_info_set_ext_type[0] =
        UNIDRC_LOUD_EXT_EQ;

    /*********************   loudness_info_v1_album   *********************/
    pstr_loudness_info_set_ext_eq->loudness_info_v1_album_count = impd_drc_get_integer_value(fp);
    pstr_loudness_info_set_ext_eq->loudness_info_v1_album_count =
        MIN(pstr_loudness_info_set_ext_eq->loudness_info_v1_album_count, MAX_LOUDNESS_INFO_COUNT);
    for (n = 0; n < pstr_loudness_info_set_ext_eq->loudness_info_v1_album_count; n++) {
      pstr_loudness_info_set_ext_eq->str_loudness_info_v1_album[n].drc_set_id =
          impd_drc_get_integer_value(fp);
      pstr_loudness_info_set_ext_eq->str_loudness_info_v1_album[n].downmix_id =
          impd_drc_get_integer_value(fp);
      pstr_loudness_info_set_ext_eq->str_loudness_info_v1_album[n].sample_peak_level_present =
          impd_drc_get_integer_value(fp);
      if (pstr_loudness_info_set_ext_eq->str_loudness_info_v1_album[n]
              .sample_peak_level_present == 1) {
        pstr_loudness_info_set_ext_eq->str_loudness_info_v1_album[n].sample_peak_level =
            impd_drc_get_float_value(fp);
      }
      pstr_loudness_info_set_ext_eq->str_loudness_info_v1_album[n].true_peak_level_present =
          impd_drc_get_integer_value(fp);
      if (pstr_loudness_info_set_ext_eq->str_loudness_info_v1_album[n].true_peak_level_present ==
          1) {
        pstr_loudness_info_set_ext_eq->str_loudness_info_v1_album[n].true_peak_level =
            impd_drc_get_float_value(fp);
        pstr_loudness_info_set_ext_eq->str_loudness_info_v1_album[n]
            .true_peak_level_measurement_system = impd_drc_get_integer_value(fp);
        pstr_loudness_info_set_ext_eq->str_loudness_info_v1_album[n].true_peak_level_reliability =
            impd_drc_get_integer_value(fp);
      }
      pstr_loudness_info_set_ext_eq->str_loudness_info_v1_album[n].measurement_count =
          impd_drc_get_integer_value(fp);
      pstr_loudness_info_set_ext_eq->str_loudness_info_v1_album[n].measurement_count =
          MIN(pstr_loudness_info_set_ext_eq->str_loudness_info_v1_album[n].measurement_count,
              MAX_MEASUREMENT_COUNT);

      for (m = 0;
           m < pstr_loudness_info_set_ext_eq->str_loudness_info_v1_album[n].measurement_count;
           m++) {
        pstr_loudness_info_set_ext_eq->str_loudness_info_v1_album[n]
            .str_loudness_measure[m]
            .method_definition = impd_drc_get_integer_value(fp);
        pstr_loudness_info_set_ext_eq->str_loudness_info_v1_album[n]
            .str_loudness_measure[m]
            .method_value = impd_drc_get_float_value(fp);
        pstr_loudness_info_set_ext_eq->str_loudness_info_v1_album[n]
            .str_loudness_measure[m]
            .measurement_system = impd_drc_get_integer_value(fp);
        pstr_loudness_info_set_ext_eq->str_loudness_info_v1_album[n]
            .str_loudness_measure[m]
            .reliability = impd_drc_get_integer_value(fp);
      }
    }

    /*********************   loudness_info_v1   *********************/
    pstr_loudness_info_set_ext_eq->loudness_info_v1_count = impd_drc_get_integer_value(fp);
    pstr_loudness_info_set_ext_eq->loudness_info_v1_count =
        MIN(pstr_loudness_info_set_ext_eq->loudness_info_v1_count, MAX_LOUDNESS_INFO_COUNT);
    for (n = 0; n < pstr_loudness_info_set_ext_eq->loudness_info_v1_count; n++) {
      pstr_loudness_info_set_ext_eq->str_loudness_info_v1[n].drc_set_id =
          impd_drc_get_integer_value(fp);
      pstr_loudness_info_set_ext_eq->str_loudness_info_v1[n].downmix_id =
          impd_drc_get_integer_value(fp);
      pstr_loudness_info_set_ext_eq->str_loudness_info_v1[n].sample_peak_level_present =
          impd_drc_get_integer_value(fp);
      if (pstr_loudness_info_set_ext_eq->str_loudness_info_v1[n].sample_peak_level_present == 1) {
        pstr_loudness_info_set_ext_eq->str_loudness_info_v1[n].sample_peak_level =
            impd_drc_get_float_value(fp);
      }
      pstr_loudness_info_set_ext_eq->str_loudness_info_v1[n].true_peak_level_present =
          impd_drc_get_integer_value(fp);
      if (pstr_loudness_info_set_ext_eq->str_loudness_info_v1[n].true_peak_level_present == 1) {
        pstr_loudness_info_set_ext_eq->str_loudness_info_v1[n].true_peak_level =
            impd_drc_get_float_value(fp);
        pstr_loudness_info_set_ext_eq->str_loudness_info_v1[n]
            .true_peak_level_measurement_system = impd_drc_get_integer_value(fp);
        pstr_loudness_info_set_ext_eq->str_loudness_info_v1[n].true_peak_level_reliability =
            impd_drc_get_integer_value(fp);
      }
      pstr_loudness_info_set_ext_eq->str_loudness_info_v1[n].measurement_count =
          impd_drc_get_integer_value(fp);
      pstr_loudness_info_set_ext_eq->str_loudness_info_v1[n].measurement_count =
          MIN(pstr_loudness_info_set_ext_eq->str_loudness_info_v1[n].measurement_count,
              MAX_MEASUREMENT_COUNT);

      for (m = 0; m < pstr_loudness_info_set_ext_eq->str_loudness_info_v1[n].measurement_count;
           m++) {
        pstr_loudness_info_set_ext_eq->str_loudness_info_v1[n]
            .str_loudness_measure[m]
            .method_definition = impd_drc_get_integer_value(fp);
        pstr_loudness_info_set_ext_eq->str_loudness_info_v1[n]
            .str_loudness_measure[m]
            .method_value = impd_drc_get_float_value(fp);
        pstr_loudness_info_set_ext_eq->str_loudness_info_v1[n]
            .str_loudness_measure[m]
            .measurement_system = impd_drc_get_integer_value(fp);
        pstr_loudness_info_set_ext_eq->str_loudness_info_v1[n]
            .str_loudness_measure[m]
            .reliability = impd_drc_get_integer_value(fp);
      }
    }
  }

  pstr_enc_gain_extension->uni_drc_gain_ext_present = 0;
}
