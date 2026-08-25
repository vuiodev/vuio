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
#include <string.h>
#include <math.h>
#include "impd_type_def.h"
#include "impd_drc_bitbuffer.h"
#include "impd_drc_extr_delta_coded_info.h"
#include "impd_drc_common.h"
#include "impd_drc_struct.h"
#include "impd_drc_parser.h"
#include "impd_drc_filter_bank.h"
#include "impd_drc_rom.h"

static VOID impd_parametric_drc_ffwd_init_drc_curve_params(
    WORD32 drc_characteristic, ia_parametric_drc_type_feed_forward_struct*
                                   str_parametric_drc_type_feed_forward) {
  WORD32* node_level = str_parametric_drc_type_feed_forward->node_level;
  WORD32* node_gain = str_parametric_drc_type_feed_forward->node_gain;

  switch (drc_characteristic) {
    case 7:
      str_parametric_drc_type_feed_forward->node_count = 5;
      node_level[0] = -22;
      node_gain[0] = 6;
      node_level[1] = -10;
      node_gain[1] = 0;
      node_level[2] = 10;
      node_gain[2] = 0;
      node_level[3] = 20;
      node_gain[3] = -5;
      node_level[4] = 40;
      node_gain[4] = -24;
      break;
    case 8:
      str_parametric_drc_type_feed_forward->node_count = 5;
      node_level[0] = -12;
      node_gain[0] = 6;
      node_level[1] = 0;
      node_gain[1] = 0;
      node_level[2] = 5;
      node_gain[2] = 0;
      node_level[3] = 15;
      node_gain[3] = -5;
      node_level[4] = 35;
      node_gain[4] = -24;
      break;
    case 9:
      str_parametric_drc_type_feed_forward->node_count = 4;
      node_level[0] = -34;
      node_gain[0] = 12;
      node_level[1] = -10;
      node_gain[1] = 0;
      node_level[2] = 10;
      node_gain[2] = 0;
      node_level[3] = 40;
      node_gain[3] = -15;
      break;
    case 10:
      str_parametric_drc_type_feed_forward->node_count = 5;
      node_level[0] = -24;
      node_gain[0] = 12;
      node_level[1] = 0;
      node_gain[1] = 0;
      node_level[2] = 5;
      node_gain[2] = 0;
      node_level[3] = 15;
      node_gain[3] = -5;
      node_level[4] = 35;
      node_gain[4] = -24;
      break;
    case 11:
      str_parametric_drc_type_feed_forward->node_count = 5;
      node_level[0] = -19;
      node_gain[0] = 15;
      node_level[1] = 0;
      node_gain[1] = 0;
      node_level[2] = 5;
      node_gain[2] = 0;
      node_level[3] = 15;
      node_gain[3] = -5;
      node_level[4] = 35;
      node_gain[4] = -24;
      break;
    default:
      str_parametric_drc_type_feed_forward->disable_paramteric_drc = 1;
  }

  return;
}

static VOID impd_parametric_drc_ffwd_init_drc_gain_smooth_params(
    WORD32 drc_characteristic, ia_parametric_drc_type_feed_forward_struct*
                                   str_parametric_drc_type_feed_forward) {
  str_parametric_drc_type_feed_forward->gain_smooth_attack_time_slow = 100;
  str_parametric_drc_type_feed_forward->gain_smooth_time_fast_present = 1;
  str_parametric_drc_type_feed_forward->gain_smooth_attack_time_fast = 10;
  str_parametric_drc_type_feed_forward->gain_smooth_threshold_present = 1;
  str_parametric_drc_type_feed_forward->gain_smooth_hold_off_count_present = 1;
  str_parametric_drc_type_feed_forward->gain_smooth_hold_off = 10;

  switch (drc_characteristic) {
    case 7:
    case 8:
    case 9:
      str_parametric_drc_type_feed_forward->gain_smooth_release_time_slow =
          3000;
      str_parametric_drc_type_feed_forward->gain_smooth_release_time_fast =
          1000;
      str_parametric_drc_type_feed_forward->gain_smooth_attack_threshold = 15;
      str_parametric_drc_type_feed_forward->gain_smooth_rel_threshold = 20;
      break;
    case 10:
      str_parametric_drc_type_feed_forward->gain_smooth_release_time_slow =
          10000;
      str_parametric_drc_type_feed_forward->gain_smooth_release_time_fast =
          1000;
      str_parametric_drc_type_feed_forward->gain_smooth_attack_threshold = 15;
      str_parametric_drc_type_feed_forward->gain_smooth_rel_threshold = 20;
      break;
    case 11:
      str_parametric_drc_type_feed_forward->gain_smooth_release_time_slow =
          1000;
      str_parametric_drc_type_feed_forward->gain_smooth_release_time_fast = 200;
      str_parametric_drc_type_feed_forward->gain_smooth_attack_threshold = 10;
      str_parametric_drc_type_feed_forward->gain_smooth_rel_threshold = 10;
      break;
    default:
      str_parametric_drc_type_feed_forward->gain_smooth_release_time_slow =
          3000;
      str_parametric_drc_type_feed_forward->gain_smooth_release_time_fast =
          1000;
      str_parametric_drc_type_feed_forward->gain_smooth_attack_threshold = 15;
      str_parametric_drc_type_feed_forward->gain_smooth_rel_threshold = 20;
      break;
  }

  return;
}

static WORD32 impd_parse_parametric_drc_ffwd(
    ia_bit_buf_struct* it_bit_buff, WORD32 parametric_drc_frame_size,
    ia_parametric_drc_type_feed_forward_struct*
        str_parametric_drc_type_feed_forward) {
  WORD32 i = 0, tmp = 0;
  // WORD32 err = 0;

  str_parametric_drc_type_feed_forward->disable_paramteric_drc = 0;

  tmp = impd_read_bits_buf(it_bit_buff, 3);
  if (it_bit_buff->error) return it_bit_buff->error;

  str_parametric_drc_type_feed_forward->level_estim_k_weighting_type =
      (tmp >> 1) & 3;
  str_parametric_drc_type_feed_forward->level_estim_integration_time_present =
      tmp & 1;

  if (str_parametric_drc_type_feed_forward
          ->level_estim_integration_time_present) {
    tmp = impd_read_bits_buf(it_bit_buff, 6);
    if (it_bit_buff->error) return it_bit_buff->error;
    str_parametric_drc_type_feed_forward->level_estim_integration_time =
        (tmp + 1) * parametric_drc_frame_size;
  } else {
    str_parametric_drc_type_feed_forward->level_estim_integration_time =
        parametric_drc_frame_size;
  }

  str_parametric_drc_type_feed_forward->drc_curve_definition_type =
      impd_read_bits_buf(it_bit_buff, 1);
  if (it_bit_buff->error) return it_bit_buff->error;

  if (str_parametric_drc_type_feed_forward->drc_curve_definition_type == 0) {
    str_parametric_drc_type_feed_forward->drc_characteristic =
        impd_read_bits_buf(it_bit_buff, 7);
    if (it_bit_buff->error) return it_bit_buff->error;
    impd_parametric_drc_ffwd_init_drc_curve_params(
        str_parametric_drc_type_feed_forward->drc_characteristic,
        str_parametric_drc_type_feed_forward);
  } else {
    str_parametric_drc_type_feed_forward->drc_characteristic = 0;

    tmp = impd_read_bits_buf(it_bit_buff, 15);
    if (it_bit_buff->error) return it_bit_buff->error;

    str_parametric_drc_type_feed_forward->node_count = ((tmp >> 12) & 3) + 2;
    str_parametric_drc_type_feed_forward->node_level[0] =
        -11 - ((tmp >> 6) & 0x3f);
    str_parametric_drc_type_feed_forward->node_gain[0] = (tmp & 0x3f) - 39;

    for (i = 1; i < str_parametric_drc_type_feed_forward->node_count; i++) {
      tmp = impd_read_bits_buf(it_bit_buff, 11);
      if (it_bit_buff->error) return it_bit_buff->error;

      str_parametric_drc_type_feed_forward->node_level[i] =
          str_parametric_drc_type_feed_forward->node_level[i - 1] + 1 +
          ((tmp >> 6) & 0x1f);
      str_parametric_drc_type_feed_forward->node_gain[i] = (tmp & 0x3f) - 39;
    }
  }

  impd_parametric_drc_ffwd_init_drc_gain_smooth_params(
      str_parametric_drc_type_feed_forward->drc_characteristic,
      str_parametric_drc_type_feed_forward);

  str_parametric_drc_type_feed_forward->drc_gain_smooth_parameters_present =
      impd_read_bits_buf(it_bit_buff, 1);
  if (it_bit_buff->error) return it_bit_buff->error;
  if (str_parametric_drc_type_feed_forward
          ->drc_gain_smooth_parameters_present) {
    tmp = impd_read_bits_buf(it_bit_buff, 17);
    if (it_bit_buff->error) return it_bit_buff->error;

    str_parametric_drc_type_feed_forward->gain_smooth_attack_time_slow =
        ((tmp >> 9) & 0xff) * 5;
    str_parametric_drc_type_feed_forward->gain_smooth_release_time_slow =
        ((tmp >> 1) & 0xff) * 40;
    str_parametric_drc_type_feed_forward->gain_smooth_time_fast_present =
        tmp & 1;

    if (str_parametric_drc_type_feed_forward->gain_smooth_time_fast_present) {
      tmp = impd_read_bits_buf(it_bit_buff, 17);
      if (it_bit_buff->error) return it_bit_buff->error;

      str_parametric_drc_type_feed_forward->gain_smooth_attack_time_fast =
          ((tmp >> 9) & 0xff) * 5;
      str_parametric_drc_type_feed_forward->gain_smooth_release_time_fast =
          ((tmp >> 1) & 0xff) * 20;
      str_parametric_drc_type_feed_forward->gain_smooth_threshold_present =
          tmp & 1;

      if (str_parametric_drc_type_feed_forward->gain_smooth_threshold_present) {
        str_parametric_drc_type_feed_forward->gain_smooth_attack_threshold =
            impd_read_bits_buf(it_bit_buff, 5);
        if (it_bit_buff->error) return it_bit_buff->error;
        if (str_parametric_drc_type_feed_forward
                ->gain_smooth_attack_threshold == 31) {
          str_parametric_drc_type_feed_forward->gain_smooth_attack_threshold =
              1000;
        }

        str_parametric_drc_type_feed_forward->gain_smooth_rel_threshold =
            impd_read_bits_buf(it_bit_buff, 5);
        if (it_bit_buff->error) return it_bit_buff->error;
        if (str_parametric_drc_type_feed_forward->gain_smooth_rel_threshold ==
            31) {
          str_parametric_drc_type_feed_forward->gain_smooth_rel_threshold =
              1000;
        }
      }
    } else {
      str_parametric_drc_type_feed_forward->gain_smooth_attack_time_fast =
          str_parametric_drc_type_feed_forward->gain_smooth_attack_time_slow;
      str_parametric_drc_type_feed_forward->gain_smooth_release_time_fast =
          str_parametric_drc_type_feed_forward->gain_smooth_release_time_slow;
    }

    str_parametric_drc_type_feed_forward->gain_smooth_hold_off_count_present =
        impd_read_bits_buf(it_bit_buff, 1);
    if (it_bit_buff->error) return it_bit_buff->error;

    if (str_parametric_drc_type_feed_forward
            ->gain_smooth_hold_off_count_present) {
      str_parametric_drc_type_feed_forward->gain_smooth_hold_off =
          impd_read_bits_buf(it_bit_buff, 7);
      if (it_bit_buff->error) return it_bit_buff->error;
    }
  }
  return 0;
}

static WORD32 impd_parse_parametric_drc_lim(
    ia_bit_buf_struct* it_bit_buff,
    ia_parametric_drc_lim_struct* parametric_drc_lim) {
  // WORD32 err = 0;
  WORD32 tmp = 0;

  parametric_drc_lim->disable_paramteric_drc = 0;

  parametric_drc_lim->parametric_lim_threshold_present =
      impd_read_bits_buf(it_bit_buff, 1);
  if (it_bit_buff->error) return it_bit_buff->error;

  if (parametric_drc_lim->parametric_lim_threshold_present) {
    tmp = impd_read_bits_buf(it_bit_buff, 8);
    if (it_bit_buff->error) return it_bit_buff->error;
    parametric_drc_lim->parametric_lim_threshold = -tmp * 0.125f;
  } else {
    parametric_drc_lim->parametric_lim_threshold =
        PARAM_DRC_TYPE_LIM_THRESHOLD_DEFAULT;
  }

  parametric_drc_lim->parametric_lim_release_present =
      impd_read_bits_buf(it_bit_buff, 1);
  if (it_bit_buff->error) return it_bit_buff->error;
  if (parametric_drc_lim->parametric_lim_release_present) {
    tmp = impd_read_bits_buf(it_bit_buff, 8);
    if (it_bit_buff->error) return it_bit_buff->error;
    parametric_drc_lim->parametric_lim_release = tmp * 10;
  } else {
    parametric_drc_lim->parametric_lim_release =
        PARAM_DRC_TYPE_LIM_RELEASE_DEFAULT;
  }

  parametric_drc_lim->parametric_lim_attack = PARAM_DRC_TYPE_LIM_ATTACK_DEFAULT;
  parametric_drc_lim->drc_characteristic = 0;

  return 0;
}

WORD32
impd_parametric_drc_parse_gain_set_params(
    ia_bit_buf_struct* it_bit_buff, ia_drc_config* drc_config,
    ia_parametric_drc_gain_set_params_struct*
        str_parametric_drc_gain_set_params) {
  WORD32 i = 0, bsDrcInputLoudness = 0, bs_channel_weight = 0, temp;
  // WORD32 err = 0;
  temp = impd_read_bits_buf(it_bit_buff, 7);
  if (it_bit_buff->error) return it_bit_buff->error;

  str_parametric_drc_gain_set_params->parametric_drc_id = (temp >> 3) & 0xf;
  str_parametric_drc_gain_set_params->side_chain_config_type = temp & 7;

  if (str_parametric_drc_gain_set_params->side_chain_config_type == 1) {
    temp = impd_read_bits_buf(it_bit_buff, 8);
    if (it_bit_buff->error) return it_bit_buff->error;

    str_parametric_drc_gain_set_params->downmix_id = (temp >> 1) & 0x7f;
    str_parametric_drc_gain_set_params->level_estim_channel_weight_format =
        temp & 1;

    if (str_parametric_drc_gain_set_params->downmix_id == ID_FOR_BASE_LAYOUT) {
      str_parametric_drc_gain_set_params->ch_count_from_dwnmix_id =
          drc_config->channel_layout.base_channel_count;
    } else if (str_parametric_drc_gain_set_params->downmix_id ==
               ID_FOR_ANY_DOWNMIX) {
      str_parametric_drc_gain_set_params->ch_count_from_dwnmix_id = 1;
    } else {
      for (i = 0; i < drc_config->dwnmix_instructions_count; i++) {
        if (str_parametric_drc_gain_set_params->downmix_id ==
            drc_config->dwnmix_instructions[i].downmix_id)
          break;
      }
      if (i == drc_config->dwnmix_instructions_count) {
        /* dwnmix_instructions not found */
        return (UNEXPECTED_ERROR);
      }
      str_parametric_drc_gain_set_params->ch_count_from_dwnmix_id =
          drc_config->dwnmix_instructions[i].target_channel_count;
    }

    for (i = 0; i < str_parametric_drc_gain_set_params->ch_count_from_dwnmix_id;
         i++) {
      if (str_parametric_drc_gain_set_params
              ->level_estim_channel_weight_format == 0) {
        str_parametric_drc_gain_set_params->level_estim_ch_weight[i] =
            (FLOAT32)impd_read_bits_buf(it_bit_buff, 1);
        if (it_bit_buff->error) return it_bit_buff->error;
      } else {
        bs_channel_weight = impd_read_bits_buf(it_bit_buff, 4);
        if (it_bit_buff->error) return it_bit_buff->error;
        str_parametric_drc_gain_set_params->level_estim_ch_weight[i] =
            (FLOAT32)pow(10.0f, 0.05f * channel_weight[bs_channel_weight]);
      }
    }
  } else {
    str_parametric_drc_gain_set_params->downmix_id = 0;
    str_parametric_drc_gain_set_params->ch_count_from_dwnmix_id = 0;
  }

  str_parametric_drc_gain_set_params->drc_input_loudness_present =
      impd_read_bits_buf(it_bit_buff, 1);
  if (it_bit_buff->error) return it_bit_buff->error;

  if (str_parametric_drc_gain_set_params->drc_input_loudness_present) {
    bsDrcInputLoudness = impd_read_bits_buf(it_bit_buff, 8);
    if (it_bit_buff->error) return it_bit_buff->error;
    str_parametric_drc_gain_set_params->drc_input_loudness =
        -57.75f + bsDrcInputLoudness * 0.25f;
  }

  return 0;
}

static WORD32 impd_parametric_drc_gen_virtual_gain_sets(
    ia_drc_config* drc_config) {
  WORD32 i = 0, j = 0, c1 = -1, c0 = -1, parametric_drc_id = 0,
         drc_characteristic = 0;
  ia_uni_drc_coeffs_struct* str_p_loc_drc_coefficients_uni_drc;
  ia_parametric_drc_instructions_struct* str_parametric_drc_instructions;
  ia_drc_coeff_parametric_drc_struct* str_drc_coeff_param_drc =
      &(drc_config->str_drc_config_ext.str_drc_coeff_param_drc);

  for (i = 0; i < drc_config->drc_coefficients_drc_count; i++) {
    if (drc_config->str_p_loc_drc_coefficients_uni_drc[i].drc_location ==
        str_drc_coeff_param_drc->drc_location) {
      if (drc_config->str_p_loc_drc_coefficients_uni_drc[i].version == 0) {
        c0 = i;
      } else {
        c1 = i;
      }
    }
  }
  if (c1 >= 0) {
    str_p_loc_drc_coefficients_uni_drc =
        &(drc_config->str_p_loc_drc_coefficients_uni_drc[c1]);
  } else if (c0 >= 0) {
    str_p_loc_drc_coefficients_uni_drc =
        &(drc_config->str_p_loc_drc_coefficients_uni_drc[c0]);
  } else {
    str_p_loc_drc_coefficients_uni_drc =
        &drc_config->str_p_loc_drc_coefficients_uni_drc
             [drc_config->drc_coefficients_drc_count];

    str_p_loc_drc_coefficients_uni_drc->version = 1;
    str_p_loc_drc_coefficients_uni_drc->drc_location =
        str_drc_coeff_param_drc->drc_location;
    str_p_loc_drc_coefficients_uni_drc->drc_frame_size_present = 0;

    str_p_loc_drc_coefficients_uni_drc->gain_set_count = 0;
    str_p_loc_drc_coefficients_uni_drc->gain_set_count_plus = 0;

    str_p_loc_drc_coefficients_uni_drc->drc_characteristic_left_present = 0;
    str_p_loc_drc_coefficients_uni_drc->drc_characteristic_right_present = 0;
    str_p_loc_drc_coefficients_uni_drc->shape_filters_present = 0;
    str_p_loc_drc_coefficients_uni_drc->gain_sequence_count = 0;
    drc_config->drc_coefficients_drc_count += 1;
  }
  {
    WORD32 tmp = str_p_loc_drc_coefficients_uni_drc->gain_set_count +
                 str_drc_coeff_param_drc->parametric_drc_gain_set_count;
    if (tmp > GAIN_SET_COUNT_MAX) return UNEXPECTED_ERROR;
    str_p_loc_drc_coefficients_uni_drc->gain_set_count_plus = tmp;
  }

  for (i = str_p_loc_drc_coefficients_uni_drc->gain_set_count;
       i < str_p_loc_drc_coefficients_uni_drc->gain_set_count_plus; i++) {
    str_p_loc_drc_coefficients_uni_drc->gain_set_params[i].band_count = 1;

    parametric_drc_id =
        drc_config->str_drc_config_ext.str_drc_coeff_param_drc
            .str_parametric_drc_gain_set_params
                [i - str_p_loc_drc_coefficients_uni_drc->gain_set_count]
            .parametric_drc_id;

    for (j = 0;
         j < drc_config->str_drc_config_ext.parametric_drc_instructions_count;
         j++) {
      if (parametric_drc_id ==
          drc_config->str_drc_config_ext.str_parametric_drc_instructions[j]
              .parametric_drc_id)
        break;
    }
    if (j == drc_config->str_drc_config_ext.parametric_drc_instructions_count) {
      /* str_parametric_drc_instructions not found */
      return (UNEXPECTED_ERROR);
    }
    str_parametric_drc_instructions =
        &drc_config->str_drc_config_ext.str_parametric_drc_instructions[j];

    drc_characteristic = 0;
    if (str_parametric_drc_instructions->parametric_drc_preset_id_present) {
      drc_characteristic = str_parametric_drc_instructions->drc_characteristic;
    } else if (str_parametric_drc_instructions->parametric_drc_type ==
               PARAM_DRC_TYPE_FF) {
      if (str_parametric_drc_instructions->str_parametric_drc_type_feed_forward
              .drc_curve_definition_type == 0) {
        drc_characteristic =
            str_parametric_drc_instructions
                ->str_parametric_drc_type_feed_forward.drc_characteristic;
      }
    }
    if (drc_characteristic != 0) {
      str_p_loc_drc_coefficients_uni_drc->gain_set_params[i]
          .gain_params[0]
          .drc_characteristic_present = 1;
      str_p_loc_drc_coefficients_uni_drc->gain_set_params[i]
          .gain_params[0]
          .drc_characteristic_format_is_cicp = 1;
      str_p_loc_drc_coefficients_uni_drc->gain_set_params[i]
          .gain_params[0]
          .drc_characteristic = drc_characteristic;
    } else {
      str_p_loc_drc_coefficients_uni_drc->gain_set_params[i]
          .gain_params[0]
          .drc_characteristic_present = 0;
      str_p_loc_drc_coefficients_uni_drc->gain_set_params[i]
          .gain_params[0]
          .drc_characteristic_format_is_cicp = 0;
      str_p_loc_drc_coefficients_uni_drc->gain_set_params[i]
          .gain_params[0]
          .drc_characteristic = 0;
    }
  }

  return 0;
}

static WORD32 impd_parametic_drc_parse_coeff(
    ia_bit_buf_struct* it_bit_buff, ia_drc_config* drc_config,
    ia_drc_coeff_parametric_drc_struct* str_drc_coeff_param_drc) {
  WORD32 i = 0, err = 0, code = 0, mu = 0, nu = 0, temp;

  temp = impd_read_bits_buf(it_bit_buff, 5);
  if (it_bit_buff->error) return it_bit_buff->error;

  str_drc_coeff_param_drc->drc_location = (temp >> 1) & 0xf;
  if (str_drc_coeff_param_drc->drc_location < 1 ||
      str_drc_coeff_param_drc->drc_location > 4)
    return UNEXPECTED_ERROR;
  str_drc_coeff_param_drc->parametric_drc_frame_size_format = temp & 1;

  if (str_drc_coeff_param_drc->parametric_drc_frame_size_format) {
    code = impd_read_bits_buf(it_bit_buff, 15);
    if (it_bit_buff->error) return it_bit_buff->error;
    str_drc_coeff_param_drc->parametric_drc_frame_size = code + 1;
  } else {
    code = impd_read_bits_buf(it_bit_buff, 4);
    if (it_bit_buff->error) return it_bit_buff->error;
    str_drc_coeff_param_drc->parametric_drc_frame_size = 1 << code;
  }

  str_drc_coeff_param_drc->parametric_drc_delay_max_present =
      impd_read_bits_buf(it_bit_buff, 1);
  if (it_bit_buff->error) return it_bit_buff->error;
  if (str_drc_coeff_param_drc->parametric_drc_delay_max_present) {
    temp = impd_read_bits_buf(it_bit_buff, 8);
    if (it_bit_buff->error) return it_bit_buff->error;

    mu = (temp >> 3) & 0x1f;
    nu = temp & 3;

    str_drc_coeff_param_drc->parametric_drc_delay_max = 16 * mu * (1 << nu);
  }

  temp = impd_read_bits_buf(it_bit_buff, 7);
  if (it_bit_buff->error) return it_bit_buff->error;

  str_drc_coeff_param_drc->reset_parametric_drc = (temp >> 6) & 1;
  str_drc_coeff_param_drc->parametric_drc_gain_set_count = temp & 0x3f;

  if (str_drc_coeff_param_drc->parametric_drc_gain_set_count >
      SEQUENCE_COUNT_MAX)
    return (UNEXPECTED_ERROR);

  for (i = 0; i < str_drc_coeff_param_drc->parametric_drc_gain_set_count; i++) {
    err = impd_parametric_drc_parse_gain_set_params(
        it_bit_buff, drc_config,
        &(str_drc_coeff_param_drc->str_parametric_drc_gain_set_params[i]));
    if (err) return (err);
  }

  return 0;
}

static WORD32 impd_parse_parametric_drc_instructions(
    ia_bit_buf_struct* it_bit_buff, WORD32 parametric_drc_frame_size,
    ia_parametric_drc_instructions_struct* str_parametric_drc_instructions) {
  WORD32 i = 0, err = 0, temp;
  WORD32 bit_size_len, bit_size;

  str_parametric_drc_instructions->drc_characteristic = 0;
  str_parametric_drc_instructions->disable_paramteric_drc = 0;

  temp = impd_read_bits_buf(it_bit_buff, 5);
  if (it_bit_buff->error) return it_bit_buff->error;

  str_parametric_drc_instructions->parametric_drc_id = (temp >> 1) & 0xf;
  str_parametric_drc_instructions->parametric_drc_look_ahead_flag = temp & 1;

  if (str_parametric_drc_instructions->parametric_drc_look_ahead_flag) {
    str_parametric_drc_instructions->parametric_drc_look_ahead =
        impd_read_bits_buf(it_bit_buff, 7);
    if (it_bit_buff->error) return it_bit_buff->error;
  } else {
    str_parametric_drc_instructions->parametric_drc_look_ahead = 0;
  }

  str_parametric_drc_instructions->parametric_drc_preset_id_present =
      impd_read_bits_buf(it_bit_buff, 1);
  if (it_bit_buff->error) return it_bit_buff->error;

  if (str_parametric_drc_instructions->parametric_drc_preset_id_present) {
    str_parametric_drc_instructions->parametric_drc_preset_id =
        impd_read_bits_buf(it_bit_buff, 7);
    if (it_bit_buff->error) return it_bit_buff->error;

    switch (str_parametric_drc_instructions->parametric_drc_preset_id) {
      case 0:
      case 1:
      case 2:
      case 3:
      case 4:
        str_parametric_drc_instructions->drc_characteristic =
            str_parametric_drc_instructions->parametric_drc_preset_id + 7;
        str_parametric_drc_instructions->parametric_drc_type =
            PARAM_DRC_TYPE_FF;

        str_parametric_drc_instructions->str_parametric_drc_type_feed_forward
            .level_estim_k_weighting_type = 2;
        str_parametric_drc_instructions->str_parametric_drc_type_feed_forward
            .level_estim_integration_time = parametric_drc_frame_size;

        impd_parametric_drc_ffwd_init_drc_curve_params(
            str_parametric_drc_instructions->drc_characteristic,
            &str_parametric_drc_instructions
                 ->str_parametric_drc_type_feed_forward);
        impd_parametric_drc_ffwd_init_drc_gain_smooth_params(
            str_parametric_drc_instructions->drc_characteristic,
            &str_parametric_drc_instructions
                 ->str_parametric_drc_type_feed_forward);

        break;
      default:
        str_parametric_drc_instructions->disable_paramteric_drc = 1;
        break;
    }
  } else {
    str_parametric_drc_instructions->parametric_drc_type =
        impd_read_bits_buf(it_bit_buff, 3);
    if (it_bit_buff->error) return it_bit_buff->error;

    if (str_parametric_drc_instructions->parametric_drc_type ==
        PARAM_DRC_TYPE_FF) {
      err = impd_parse_parametric_drc_ffwd(
          it_bit_buff, parametric_drc_frame_size,
          &(str_parametric_drc_instructions
                ->str_parametric_drc_type_feed_forward));
      if (err) return (err);
      str_parametric_drc_instructions->disable_paramteric_drc =
          str_parametric_drc_instructions->str_parametric_drc_type_feed_forward
              .disable_paramteric_drc;
      str_parametric_drc_instructions->drc_characteristic =
          str_parametric_drc_instructions->str_parametric_drc_type_feed_forward
              .drc_characteristic;
    } else if (str_parametric_drc_instructions->parametric_drc_type ==
               PARAM_DRC_TYPE_LIM) {
      err = impd_parse_parametric_drc_lim(
          it_bit_buff, &(str_parametric_drc_instructions->parametric_drc_lim));
      if (err) return (err);
      str_parametric_drc_instructions->disable_paramteric_drc =
          str_parametric_drc_instructions->parametric_drc_lim
              .disable_paramteric_drc;
      str_parametric_drc_instructions->drc_characteristic =
          str_parametric_drc_instructions->parametric_drc_lim
              .drc_characteristic;
      if (str_parametric_drc_instructions->parametric_drc_look_ahead_flag) {
        str_parametric_drc_instructions->parametric_drc_lim
            .parametric_lim_attack =
            str_parametric_drc_instructions->parametric_drc_look_ahead;
      }
    } else {
      bit_size_len = impd_read_bits_buf(it_bit_buff, 3) + 4;
      if (it_bit_buff->error) return it_bit_buff->error;

      bit_size = impd_read_bits_buf(it_bit_buff, bit_size_len);
      if (it_bit_buff->error) return it_bit_buff->error;
      str_parametric_drc_instructions->len_bit_size = bit_size + 1;

      switch (str_parametric_drc_instructions->parametric_drc_type) {
        default:
          str_parametric_drc_instructions->disable_paramteric_drc = 1;
          for (i = 0; i < str_parametric_drc_instructions->len_bit_size; i++) {
            impd_read_bits_buf(it_bit_buff, 1);
            if (it_bit_buff->error) return it_bit_buff->error;
          }
          break;
      }
    }
  }

  return 0;
}

WORD32 impd_parse_loud_info_set_ext_eq(
    ia_bit_buf_struct* it_bit_buff,
    ia_drc_loudness_info_set_struct* loudness_info_set) {
  WORD32 err, i, offset, version = 1, temp;
  WORD32 loudness_info_v1_album_cnt, loudness_info_v1_cnt;

  temp = impd_read_bits_buf(it_bit_buff, 12);
  if (it_bit_buff->error) return it_bit_buff->error;

  loudness_info_v1_album_cnt = (temp >> 6) & 0x3f;
  loudness_info_v1_cnt = temp & 0x3f;

  offset = loudness_info_set->loudness_info_album_count;
  loudness_info_set->loudness_info_album_count += loudness_info_v1_album_cnt;
  if ((offset + loudness_info_v1_album_cnt) > LOUDNESS_INFO_COUNT_MAX)
    return (UNEXPECTED_ERROR);
  for (i = 0; i < loudness_info_v1_album_cnt; i++) {
    err = impd_parse_loudness_info(
        it_bit_buff, version,
        &loudness_info_set->str_loudness_info_album[i + offset]);
    if (err) return (err);
  }
  offset = loudness_info_set->loudness_info_count;
  loudness_info_set->loudness_info_count += loudness_info_v1_cnt;
  if (offset + loudness_info_v1_cnt > LOUDNESS_INFO_COUNT_MAX)
    return (UNEXPECTED_ERROR);
  for (i = 0; i < loudness_info_v1_cnt; i++) {
    err = impd_parse_loudness_info(
        it_bit_buff, version, &loudness_info_set->loudness_info[i + offset]);
    if (err) return (err);
  }
  return (0);
}

WORD32 impd_parse_ch_layout(ia_bit_buf_struct* it_bit_buff,
                            ia_drc_params_bs_dec_struct* ia_drc_params_struct,
                            ia_channel_layout_struct* channel_layout) {
  // WORD32 err = 0;
  WORD32 i;

  channel_layout->base_channel_count = impd_read_bits_buf(it_bit_buff, 7);
  if (it_bit_buff->error) return it_bit_buff->error;
  if ((channel_layout->base_channel_count > MAX_CHANNEL_COUNT) ||
      (channel_layout->base_channel_count == 0))
    return (UNEXPECTED_ERROR);
  if (ia_drc_params_struct->lfe_channel_map_count != -1 &&
      channel_layout->base_channel_count !=
          ia_drc_params_struct->lfe_channel_map_count) {
    return (UNEXPECTED_ERROR);
  }
  channel_layout->layout_signaling_present = impd_read_bits_buf(it_bit_buff, 1);
  if (it_bit_buff->error) return it_bit_buff->error;

  if (channel_layout->layout_signaling_present) {
    channel_layout->defined_layout = impd_read_bits_buf(it_bit_buff, 8);
    if (it_bit_buff->error) return it_bit_buff->error;

    if (channel_layout->defined_layout == 0) {
      for (i = 0; i < channel_layout->base_channel_count; i++) {
        channel_layout->speaker_position[i] =
            impd_read_bits_buf(it_bit_buff, 7);
        if (it_bit_buff->error) return it_bit_buff->error;
        if (channel_layout->speaker_position[i] == 3 ||
            channel_layout->speaker_position[i] == 26) {
          ia_drc_params_struct->lfe_channel_map[i] = 1;
        } else {
          ia_drc_params_struct->lfe_channel_map[i] = 0;
        }
      }
    }
  }

  return (0);
}

WORD32
impd_parse_dwnmix_instructions(
    ia_bit_buf_struct* it_bit_buff, WORD32 version,
    ia_drc_params_bs_dec_struct* ia_drc_params_struct,
    ia_channel_layout_struct* channel_layout,
    ia_downmix_instructions_struct* dwnmix_instructions) {
  // WORD32 err = 0;
  WORD32 i, j, k, temp;

  temp = impd_read_bits_buf(it_bit_buff, 23);
  if (it_bit_buff->error) return it_bit_buff->error;

  dwnmix_instructions->downmix_id = (temp >> 16) & 0x7f;
  dwnmix_instructions->target_channel_count = (temp >> 9) & 0x7f;
  if (dwnmix_instructions->target_channel_count > MAX_CHANNEL_COUNT)
    return (UNEXPECTED_ERROR);
  dwnmix_instructions->target_layout = (temp >> 1) & 0xff;
  dwnmix_instructions->downmix_coefficients_present = temp & 1;

  if (dwnmix_instructions->downmix_coefficients_present) {
    if (version == 0) {
      WORD32 dmix_coeff;
      k = 0;
      for (i = 0; i < dwnmix_instructions->target_channel_count; i++) {
        for (j = 0; j < channel_layout->base_channel_count; j++) {
          dmix_coeff = impd_read_bits_buf(it_bit_buff, 4);
          if (it_bit_buff->error) return it_bit_buff->error;

          if (k >= DOWNMIX_COEFF_COUNT_MAX) return (UNEXPECTED_ERROR);
          if (ia_drc_params_struct->lfe_channel_map[j]) {
            dwnmix_instructions->downmix_coefficient[k] =
                (FLOAT32)pow(10.0f, 0.05f * dwnmix_coeff_lfe[dmix_coeff]);
          } else {
            dwnmix_instructions->downmix_coefficient[k] =
                (FLOAT32)pow(10.0f, 0.05f * dwnmix_coeff[dmix_coeff]);
          }
          k++;
        }
      }
    } else {
      WORD32 dmix_coeff_v1, bs_dmix_offset;
      FLOAT32 a, b, dmix_offset, sum;

      bs_dmix_offset = impd_read_bits_buf(it_bit_buff, 4);
      if (it_bit_buff->error) return it_bit_buff->error;
      k = 0;

      for (i = 0; i < dwnmix_instructions->target_channel_count; i++) {
        for (j = 0; j < channel_layout->base_channel_count; j++) {
          dmix_coeff_v1 = impd_read_bits_buf(it_bit_buff, 5);
          if (it_bit_buff->error) return it_bit_buff->error;
          if (k >= DOWNMIX_COEFF_COUNT_MAX) return (UNEXPECTED_ERROR);
          dwnmix_instructions->downmix_coefficient[k] =
              dwnmix_coeff_v1[dmix_coeff_v1];
          k++;
        }
      }
      switch (bs_dmix_offset) {
        case 0:
          dmix_offset = 0.0f;
          break;
        case 1:
          a = 20.0f * (FLOAT32)log10(
                          (FLOAT32)dwnmix_instructions->target_channel_count /
                          (FLOAT32)channel_layout->base_channel_count);
          dmix_offset = 0.5f * (FLOAT32)floor(0.5f + a);
          break;
        case 2:
          a = 20.0f * (FLOAT32)log10(
                          (FLOAT32)dwnmix_instructions->target_channel_count /
                          (FLOAT32)channel_layout->base_channel_count);
          dmix_offset = 0.5f * (FLOAT32)floor(0.5f + 2.0f * a);
          break;
        case 3:
          sum = 0.0f;
          for (k = 0; k < dwnmix_instructions->target_channel_count *
                              channel_layout->base_channel_count;
               k++) {
            sum += (FLOAT32)pow(
                10.0f, 0.1f * dwnmix_instructions->downmix_coefficient[k]);
          }
          b = 10.0f * (FLOAT32)log10(sum);
          dmix_offset = 0.5f * (FLOAT32)floor(0.5f + 2.0f * b);
          break;

        default:
          return (BITSTREAM_ERROR);
          break;
      }
      for (k = 0; k < dwnmix_instructions->target_channel_count *
                          channel_layout->base_channel_count;
           k++) {
        dwnmix_instructions->downmix_coefficient[k] = (FLOAT32)pow(
            10.0f, 0.05f * (dwnmix_instructions->downmix_coefficient[k] +
                            dmix_offset));
      }
    }
  }
  return (0);
}

VOID impd_drc_gen_instructions_for_drc_off(ia_drc_config* drc_config) {
  WORD32 i, k, s;
  ia_drc_instructions_struct* str_drc_instruction_str;
  s = -1;

  k = drc_config->drc_instructions_uni_drc_count;

  str_drc_instruction_str = &(drc_config->str_drc_instruction_str[k]);
  memset(str_drc_instruction_str, 0, sizeof(ia_drc_instructions_struct));
  str_drc_instruction_str->drc_set_id = s;
  s--;
  str_drc_instruction_str->downmix_id[0] = ID_FOR_BASE_LAYOUT;
  str_drc_instruction_str->dwnmix_id_count = 1;
  str_drc_instruction_str->drc_apply_to_dwnmix = 0;
  str_drc_instruction_str->depends_on_drc_set_present = 0;
  str_drc_instruction_str->no_independent_use = 0;
  str_drc_instruction_str->gain_element_count = 0;
  for (i = 1; i < drc_config->dwnmix_instructions_count + 1; i++) {
    str_drc_instruction_str = &(drc_config->str_drc_instruction_str[k + i]);
    memset(str_drc_instruction_str, 0, sizeof(ia_drc_instructions_struct));
    str_drc_instruction_str->drc_set_id = s;
    s--;
    str_drc_instruction_str->drc_set_complexity_level = 0;
    str_drc_instruction_str->requires_eq = 0;
    str_drc_instruction_str->downmix_id[0] =
        drc_config->dwnmix_instructions[i - 1].downmix_id;
    str_drc_instruction_str->dwnmix_id_count = 1;
    str_drc_instruction_str->drc_apply_to_dwnmix = 0;
    str_drc_instruction_str->depends_on_drc_set_present = 0;
    str_drc_instruction_str->no_independent_use = 0;
    str_drc_instruction_str->gain_element_count = 0;
  }
  drc_config->drc_instructions_count_plus =
      drc_config->drc_instructions_uni_drc_count +
      drc_config->dwnmix_instructions_count + 1;
  return;
}

WORD32
impd_parse_drc_config_ext(ia_bit_buf_struct* it_bit_buff,
                          ia_drc_params_bs_dec_struct* ia_drc_params_struct,
                          ia_drc_config* drc_config,
                          ia_drc_config_ext* str_drc_config_ext) {
  WORD32 err = 0, i, k;
  WORD32 bit_size_len, ext_size_bits, bit_size;

  k = 0;
  str_drc_config_ext->drc_config_ext_type[k] =
      impd_read_bits_buf(it_bit_buff, 4);
  if (it_bit_buff->error) return it_bit_buff->error;
  while (str_drc_config_ext->drc_config_ext_type[k] != UNIDRCCONFEXT_TERM) {
    if (k >= (EXT_COUNT_MAX - 1)) return UNEXPECTED_ERROR;
    bit_size_len = impd_read_bits_buf(it_bit_buff, 4);
    if (it_bit_buff->error) return it_bit_buff->error;
    ext_size_bits = bit_size_len + 4;

    bit_size = impd_read_bits_buf(it_bit_buff, ext_size_bits);
    if (it_bit_buff->error) return it_bit_buff->error;
    str_drc_config_ext->ext_bit_size[k] = bit_size + 1;

    switch (str_drc_config_ext->drc_config_ext_type[k]) {
      case UNIDRCCONFEXT_PARAM_DRC:
        str_drc_config_ext->parametric_drc_present = 1;
        err = impd_parametic_drc_parse_coeff(
            it_bit_buff, drc_config,
            &(str_drc_config_ext->str_drc_coeff_param_drc));
        if (err) return (err);
        str_drc_config_ext->parametric_drc_instructions_count =
            impd_read_bits_buf(it_bit_buff, 4);
        if (it_bit_buff->error) return it_bit_buff->error;
        if (str_drc_config_ext->parametric_drc_instructions_count >
            PARAM_DRC_INSTRUCTIONS_COUNT_MAX)
          return (UNEXPECTED_ERROR);

        for (i = 0; i < str_drc_config_ext->parametric_drc_instructions_count;
             i++) {
          err = impd_parse_parametric_drc_instructions(
              it_bit_buff, str_drc_config_ext->str_drc_coeff_param_drc
                               .parametric_drc_frame_size,
              &(str_drc_config_ext->str_parametric_drc_instructions[i]));
          if (err) return (err);
        }
        break;
      case UNIDRCCONFEXT_V1:
        str_drc_config_ext->drc_extension_v1_present = 1;
        err = impd_parse_drc_ext_v1(it_bit_buff, ia_drc_params_struct,
                                    drc_config, str_drc_config_ext);
        if (err) return (err);
        break;

#ifdef LOUDNESS_LEVELING_SUPPORT
      case UNIDRCCONFEXT_LEVELING:
        err = impd_leveling_instructions(it_bit_buff, drc_config);
        if (err) return err;
        break;
#endif
      default:
        for (i = 0; i < str_drc_config_ext->ext_bit_size[k]; i++) {
          impd_read_bits_buf(it_bit_buff, 1);
          if (it_bit_buff->error) return it_bit_buff->error;
        }
        break;
    }
    k++;
    str_drc_config_ext->drc_config_ext_type[k] =
        impd_read_bits_buf(it_bit_buff, 4);
    if (it_bit_buff->error) return it_bit_buff->error;
  }

  return (0);
}

static WORD32 impd_parse_split_drc_characteristic(
    ia_bit_buf_struct* it_bit_buff, const WORD32 side,
    ia_split_drc_characteristic_struct* split_drc_characteristic) {
  // WORD32 err = 0;
  WORD32 i, temp;

  split_drc_characteristic->characteristic_format =
      impd_read_bits_buf(it_bit_buff, 1);
  if (it_bit_buff->error) return it_bit_buff->error;
  if (split_drc_characteristic->characteristic_format == 0) {
    WORD32 bsGain, bsIoRatio, bsExp;
    bsGain = impd_read_bits_buf(it_bit_buff, 6);
    if (it_bit_buff->error) return it_bit_buff->error;
    if (side == LEFT_SIDE) {
      split_drc_characteristic->gain = (FLOAT32)bsGain;
    } else {
      split_drc_characteristic->gain = (FLOAT32)-bsGain;
    }
    temp = impd_read_bits_buf(it_bit_buff, 8);
    if (it_bit_buff->error) return it_bit_buff->error;

    bsIoRatio = (temp >> 4) & 0xf;
    bsExp = temp & 0xf;
    split_drc_characteristic->in_out_ratio = 0.05f + 0.15f * bsIoRatio;

    if (bsExp < 15) {
      split_drc_characteristic->exp = 1.0f + 2.0f * bsExp;
    } else {
      split_drc_characteristic->exp = 1000.0f;
    }
    split_drc_characteristic->flip_sign = impd_read_bits_buf(it_bit_buff, 1);
    if (it_bit_buff->error) return it_bit_buff->error;
  } else {
    WORD32 char_node_cnt, node_level_delta, node_gain;
    char_node_cnt = impd_read_bits_buf(it_bit_buff, 2);
    if (it_bit_buff->error) return it_bit_buff->error;
    split_drc_characteristic->characteristic_node_count = char_node_cnt + 1;
    split_drc_characteristic->node_level[0] = DRC_INPUT_LOUDNESS_TARGET;
    split_drc_characteristic->node_gain[0] = 0.0f;
    for (i = 1; i <= split_drc_characteristic->characteristic_node_count; i++) {
      node_level_delta = impd_read_bits_buf(it_bit_buff, 5);
      if (it_bit_buff->error) return it_bit_buff->error;
      if (side == LEFT_SIDE) {
        split_drc_characteristic->node_level[i] =
            split_drc_characteristic->node_level[i - 1] -
            (1.0f + node_level_delta);
      } else {
        split_drc_characteristic->node_level[i] =
            split_drc_characteristic->node_level[i - 1] +
            (1.0f + node_level_delta);
      }
      node_gain = impd_read_bits_buf(it_bit_buff, 8);
      if (it_bit_buff->error) return it_bit_buff->error;
      split_drc_characteristic->node_gain[i] = 0.5f * node_gain - 64.0f;
    }
  }
  return (0);
}

WORD32
impd_drc_gen_instructions_derived_data(
    ia_drc_config* drc_config,
    ia_drc_params_bs_dec_struct* ia_drc_params_struct,
    ia_drc_instructions_struct* str_drc_instruction_str) {
  WORD32 n, g;
  ia_uni_drc_coeffs_struct* str_p_loc_drc_coefficients_uni_drc = NULL;
  ia_drc_coeff_parametric_drc_struct* str_drc_coeff_param_drc = NULL;
  WORD32 gain_element_count = 0;

  for (n = 0; n < drc_config->drc_coefficients_drc_count; n++) {
    if (drc_config->str_p_loc_drc_coefficients_uni_drc[n].drc_location ==
        str_drc_instruction_str->drc_location)
      break;
  }
  if ((n == drc_config->drc_coefficients_drc_count) &&
      (drc_config->drc_coefficients_drc_count > 0)) {
    return -1;
  }
  str_p_loc_drc_coefficients_uni_drc =
      &(drc_config->str_p_loc_drc_coefficients_uni_drc[n]);

  if (drc_config->drc_config_ext_present &&
      drc_config->str_drc_config_ext.parametric_drc_present &&
      drc_config->str_drc_config_ext.str_drc_coeff_param_drc.drc_location ==
          str_drc_instruction_str->drc_location) {
    str_drc_coeff_param_drc =
        &drc_config->str_drc_config_ext.str_drc_coeff_param_drc;
  }

  for (g = 0; g < str_drc_instruction_str->num_drc_ch_groups; g++) {
    WORD32 seq = str_drc_instruction_str->gain_set_index_for_channel_group[g];
    if (seq != -1 &&
        (drc_config->drc_coefficients_drc_count == 0 ||
         seq >= str_p_loc_drc_coefficients_uni_drc->gain_set_count)) {
      str_drc_instruction_str->ch_group_parametric_drc_flag[g] = 1;
      if (drc_config->drc_coefficients_drc_count != 0) {
        seq = seq - str_p_loc_drc_coefficients_uni_drc->gain_set_count;
      }
      str_drc_instruction_str->gain_set_idx_of_ch_group_parametric_drc[g] = seq;

      if (str_drc_coeff_param_drc == NULL ||
          seq >= str_drc_coeff_param_drc->parametric_drc_gain_set_count) {
        /* parametric drc gain set not available */
        return (EXTERNAL_ERROR);
      }
      str_drc_instruction_str->gain_interpolation_type_for_channel_group[g] = 1;
      str_drc_instruction_str->time_delta_min_for_channel_group[g] =
          str_drc_coeff_param_drc->parametric_drc_frame_size;
      str_drc_instruction_str->time_alignment_for_channel_group[g] = 0;
    } else {
      str_drc_instruction_str->ch_group_parametric_drc_flag[g] = 0;
    }
    if (str_drc_instruction_str->ch_group_parametric_drc_flag[g] == 0) {
      if (seq >= str_p_loc_drc_coefficients_uni_drc->gain_set_count) {
        return -1;
      }
      str_drc_instruction_str->gain_interpolation_type_for_channel_group[g] =
          str_p_loc_drc_coefficients_uni_drc->gain_set_params[seq]
              .gain_interpolation_type;
      if (str_p_loc_drc_coefficients_uni_drc->gain_set_params[seq]
              .time_delt_min_flag) {
        str_drc_instruction_str->time_delta_min_for_channel_group[g] =
            str_p_loc_drc_coefficients_uni_drc->gain_set_params[seq]
                .time_delt_min_val;
      } else {
        str_drc_instruction_str->time_delta_min_for_channel_group[g] =
            ia_drc_params_struct->delta_tmin_default;
      }
      str_drc_instruction_str->time_alignment_for_channel_group[g] =
          str_p_loc_drc_coefficients_uni_drc->gain_set_params[seq]
              .time_alignment;
    }
  }

  if (str_drc_instruction_str->drc_set_effect &
      (EFFECT_BIT_DUCK_OTHER | EFFECT_BIT_DUCK_SELF)) {
    str_drc_instruction_str->gain_element_count =
        str_drc_instruction_str->num_drc_ch_groups;
  } else {
    for (g = 0; g < str_drc_instruction_str->num_drc_ch_groups; g++) {
      if (str_drc_instruction_str->ch_group_parametric_drc_flag[g] == 1) {
        gain_element_count++;
        str_drc_instruction_str->band_count_of_ch_group[g] = 1;
      } else {
        WORD32 seq, band_count;
        seq = str_drc_instruction_str->gain_set_index_for_channel_group[g];
        band_count =
            str_p_loc_drc_coefficients_uni_drc->gain_set_params[seq].band_count;
        str_drc_instruction_str->band_count_of_ch_group[g] = band_count;
        gain_element_count += band_count;
      }
    }
    str_drc_instruction_str->gain_element_count = gain_element_count;
  }

  return (0);
}

WORD32
impd_parse_drc_config(ia_bit_buf_struct* it_bit_buff,
                      ia_drc_params_bs_dec_struct* ia_drc_params_struct,
                      ia_drc_config* drc_config) {
  WORD32 i, err = 0, temp;
  WORD32 version = 0;

  drc_config->sample_rate_present = impd_read_bits_buf(it_bit_buff, 1);
  if (it_bit_buff->error) return it_bit_buff->error;

  if (drc_config->sample_rate_present == 1) {
    WORD32 bssample_rate;
    bssample_rate = impd_read_bits_buf(it_bit_buff, 18);
    if (it_bit_buff->error) return it_bit_buff->error;
    drc_config->sampling_rate = bssample_rate + 1000;
  }

  temp = impd_read_bits_buf(it_bit_buff, 8);
  if (it_bit_buff->error) return it_bit_buff->error;

  drc_config->dwnmix_instructions_count = (temp >> 1) & 0x7f;
  if (drc_config->dwnmix_instructions_count > DOWNMIX_INSTRUCTION_COUNT_MAX)
    return (UNEXPECTED_ERROR);

  drc_config->drc_description_basic_present = temp & 1;

  if (drc_config->drc_description_basic_present == 1) {
    temp = impd_read_bits_buf(it_bit_buff, 7);
    if (it_bit_buff->error) return it_bit_buff->error;

    drc_config->drc_coefficients_basic_count = (temp >> 4) & 7;
    drc_config->drc_instructions_basic_count = temp & 0xf;

  } else {
    drc_config->drc_coefficients_basic_count = 0;
    drc_config->drc_instructions_basic_count = 0;
  }

  temp = impd_read_bits_buf(it_bit_buff, 9);
  if (it_bit_buff->error) return it_bit_buff->error;

  drc_config->drc_coefficients_drc_count = (temp >> 6) & 7;
  drc_config->drc_instructions_uni_drc_count = temp & 0x3f;

  if (drc_config->drc_instructions_uni_drc_count > DRC_INSTRUCTIONS_COUNT_MAX)
    return (UNEXPECTED_ERROR);

  err = impd_parse_ch_layout(it_bit_buff, ia_drc_params_struct,
                             &drc_config->channel_layout);
  if (err) return (err);

  for (i = 0; i < drc_config->dwnmix_instructions_count; i++) {
    err = impd_parse_dwnmix_instructions(
        it_bit_buff, version, ia_drc_params_struct, &drc_config->channel_layout,
        &(drc_config->dwnmix_instructions[i]));
    if (err) return (err);
  }
  for (i = 0; i < drc_config->drc_coefficients_basic_count; i++) {
    temp = impd_read_bits_buf(it_bit_buff, 11);
    if (it_bit_buff->error) return it_bit_buff->error;

    drc_config->str_drc_coefficients_basic[i].drc_location = (temp >> 7) & 0xf;
    drc_config->str_drc_coefficients_basic[i].drc_characteristic = temp & 0x3f;
  }
  for (i = 0; i < drc_config->drc_instructions_basic_count; i++) {
    err = impd_drc_parse_instructions_basic(
        it_bit_buff, &(drc_config->str_drc_instructions_basic[i]));
    if (err) return (err);
  }
  for (i = 0; i < drc_config->drc_coefficients_drc_count; i++) {
    err = impd_drc_parse_coeff(
        it_bit_buff, version, ia_drc_params_struct,
        &(drc_config->str_p_loc_drc_coefficients_uni_drc[i]));
    if (err) return (err);
  }
  for (i = 0; i < drc_config->drc_instructions_uni_drc_count; i++) {
    err = impd_parse_drc_instructions_uni_drc(
        it_bit_buff, version, drc_config,
        &(drc_config->str_drc_instruction_str[i]));
    if (err) return (err);
  }

  drc_config->drc_config_ext_present = impd_read_bits_buf(it_bit_buff, 1);
  if (it_bit_buff->error) return it_bit_buff->error;

  if (drc_config->drc_config_ext_present == 1) {
    err =
        impd_parse_drc_config_ext(it_bit_buff, ia_drc_params_struct, drc_config,
                                  &(drc_config->str_drc_config_ext));
    if (err) return (err);
  }

  if (drc_config->str_drc_config_ext.parametric_drc_present) {
    err = impd_parametric_drc_gen_virtual_gain_sets(drc_config);
    if (err) return (err);
  }

  for (i = 0; i < drc_config->drc_instructions_uni_drc_count; i++) {
    err = impd_drc_gen_instructions_derived_data(
        drc_config, ia_drc_params_struct,
        &(drc_config->str_drc_instruction_str[i]));
    if (err) return (err);
  }

  if ((drc_config->drc_instructions_uni_drc_count +
       drc_config->dwnmix_instructions_count) >= DRC_INSTRUCTIONS_COUNT_MAX)
    return (UNEXPECTED_ERROR);
  impd_drc_gen_instructions_for_drc_off(drc_config);
  return (0);
}

WORD32
impd_dec_method_value(ia_bit_buf_struct* it_bit_buff, WORD32 method_def,
                      FLOAT32* method_val) {
  // WORD32 err = 0;
  WORD32 tmp;
  FLOAT32 val;
  switch (method_def) {
    case METHOD_DEFINITION_UNKNOWN_OTHER:
    case METHOD_DEFINITION_PROGRAM_LOUDNESS:
    case METHOD_DEFINITION_ANCHOR_LOUDNESS:
    case METHOD_DEFINITION_MAX_OF_LOUDNESS_RANGE:
    case METHOD_DEFINITION_MOMENTARY_LOUDNESS_MAX:
    case METHOD_DEFINITION_SHORT_TERM_LOUDNESS_MAX:
      tmp = impd_read_bits_buf(it_bit_buff, 8);
      if (it_bit_buff->error) return it_bit_buff->error;
      val = -57.75f + tmp * 0.25f;
      break;
    case METHOD_DEFINITION_LOUDNESS_RANGE:
      tmp = impd_read_bits_buf(it_bit_buff, 8);
      if (it_bit_buff->error) return it_bit_buff->error;
      if (tmp == 0)
        val = 0.0f;
      else if (tmp <= 128)
        val = tmp * 0.25f;
      else if (tmp <= 204)
        val = 0.5f * tmp - 32.0f;
      else
        val = tmp - 134.0f;
      break;
    case METHOD_DEFINITION_MIXING_LEVEL:
      tmp = impd_read_bits_buf(it_bit_buff, 5);
      if (it_bit_buff->error) return it_bit_buff->error;
      val = tmp + 80.0f;
      break;
    case METHOD_DEFINITION_ROOM_TYPE:
      tmp = impd_read_bits_buf(it_bit_buff, 2);
      if (it_bit_buff->error) return it_bit_buff->error;
      val = (FLOAT32)tmp;
      break;
    case METHOD_DEFINITION_SHORT_TERM_LOUDNESS:
      tmp = impd_read_bits_buf(it_bit_buff, 8);
      if (it_bit_buff->error) return it_bit_buff->error;
      val = -116.f + tmp * 0.5f;
      break;
    default:
      return -1;
      break;
  }
  *method_val = val;
  return 0;
}

WORD32
impd_parse_loudness_info_set(
    ia_bit_buf_struct* it_bit_buff,
    ia_drc_loudness_info_set_struct* loudness_info_set) {
  WORD32 err = 0, i, version = 0, offset, temp;
  WORD32 loudness_info_album_count, loudness_info_count;

  temp = impd_read_bits_buf(it_bit_buff, 12);
  if (it_bit_buff->error) return it_bit_buff->error;

  loudness_info_album_count = (temp >> 6) & 0x3f;
  loudness_info_count = temp & 0x3f;

  offset = loudness_info_set->loudness_info_album_count;
  loudness_info_set->loudness_info_album_count += loudness_info_album_count;
  if ((offset + loudness_info_set->loudness_info_album_count) >
      LOUDNESS_INFO_COUNT_MAX)
    return (UNEXPECTED_ERROR);
  for (i = 0; i < loudness_info_set->loudness_info_album_count; i++) {
    err = impd_parse_loudness_info(
        it_bit_buff, version,
        &(loudness_info_set->str_loudness_info_album[i + offset]));
    if (err) return (err);
  }

  offset = loudness_info_set->loudness_info_count;
  loudness_info_set->loudness_info_count += loudness_info_count;
  if ((offset + loudness_info_set->loudness_info_count) >
      LOUDNESS_INFO_COUNT_MAX)
    return (UNEXPECTED_ERROR);
  for (i = 0; i < loudness_info_set->loudness_info_count; i++) {
    err = impd_parse_loudness_info(
        it_bit_buff, version, &(loudness_info_set->loudness_info[i + offset]));
    if (err) return (err);
  }

  loudness_info_set->loudness_info_set_ext_present =
      impd_read_bits_buf(it_bit_buff, 1);
  if (it_bit_buff->error) return it_bit_buff->error;

  if (loudness_info_set->loudness_info_set_ext_present == 1) {
    err = impd_parse_loudness_info_set_ext(it_bit_buff, loudness_info_set);
    if (err) return (err);
  }

  return (0);
}

WORD32
impd_parse_gain_set_params_characteristics(ia_bit_buf_struct* it_bit_buff,
                                           WORD32 version,
                                           ia_gain_params_struct* gain_params) {
  // WORD32 err = 0;
  WORD32 temp;
  if (version == 0) {
    gain_params->drc_characteristic = impd_read_bits_buf(it_bit_buff, 7);
    if (it_bit_buff->error) return it_bit_buff->error;
    if (gain_params->drc_characteristic > 0) {
      gain_params->drc_characteristic_present = 1;
      gain_params->drc_characteristic_format_is_cicp = 1;
    } else {
      gain_params->drc_characteristic_present = 0;
    }
  } else {
    gain_params->drc_characteristic_present =
        impd_read_bits_buf(it_bit_buff, 1);
    if (it_bit_buff->error) return it_bit_buff->error;
    if (gain_params->drc_characteristic_present) {
      gain_params->drc_characteristic_format_is_cicp =
          impd_read_bits_buf(it_bit_buff, 1);
      if (it_bit_buff->error) return it_bit_buff->error;
      if (gain_params->drc_characteristic_format_is_cicp) {
        gain_params->drc_characteristic = impd_read_bits_buf(it_bit_buff, 7);
        if (it_bit_buff->error) return it_bit_buff->error;
      } else {
        temp = impd_read_bits_buf(it_bit_buff, 8);
        if (it_bit_buff->error) return it_bit_buff->error;

        gain_params->drc_characteristic_left_index = (temp >> 4) & 0xf;
        gain_params->drc_characteristic_right_index = temp & 0xf;
      }
    }
  }
  return (0);
}

WORD32
impd_parse_loudness_measure(ia_bit_buf_struct* it_bit_buff,
                            ia_loudness_measure_struct* loudness_measure) {
  WORD32 err = 0, temp;

  loudness_measure->method_def = impd_read_bits_buf(it_bit_buff, 4);
  if (it_bit_buff->error) return it_bit_buff->error;

  err = impd_dec_method_value(it_bit_buff, loudness_measure->method_def,
                              &(loudness_measure->method_val));
  if (err) return err;

  temp = impd_read_bits_buf(it_bit_buff, 6);
  if (it_bit_buff->error) return it_bit_buff->error;

  loudness_measure->measurement_system = (temp >> 2) & 0xf;
  if (loudness_measure->measurement_system > MEASUREMENT_SYSTEM_RESERVED_E)
    return (UNEXPECTED_ERROR);
  /* Parsed but unused */
  loudness_measure->reliability = temp & 3;

  return (0);
}

WORD32
impd_dec_gain_modifiers(ia_bit_buf_struct* it_bit_buff, WORD32 version,
                        WORD32 band_count,
                        ia_gain_modifiers_struct* pstr_gain_modifiers) {
  // WORD32 err = 0;
  WORD32 sign, temp;

  if (version > 0) {
    WORD32 b;
    for (b = 0; b < band_count; b++) {
      pstr_gain_modifiers->target_characteristic_left_present[b] =
          impd_read_bits_buf(it_bit_buff, 1);
      if (it_bit_buff->error) return it_bit_buff->error;
      if (pstr_gain_modifiers->target_characteristic_left_present[b]) {
        WORD32 tmp_index = impd_read_bits_buf(it_bit_buff, 4);
        if (it_bit_buff->error) return it_bit_buff->error;
        if (tmp_index >= SPLIT_CHARACTERISTIC_COUNT_MAX)
          return (UNEXPECTED_ERROR);
        pstr_gain_modifiers->target_characteristic_left_index[b] = tmp_index;
      }
      pstr_gain_modifiers->target_characteristic_right_present[b] =
          impd_read_bits_buf(it_bit_buff, 1);
      if (it_bit_buff->error) return it_bit_buff->error;
      if (pstr_gain_modifiers->target_characteristic_right_present[b]) {
        WORD32 tmp_index = impd_read_bits_buf(it_bit_buff, 4);
        if (it_bit_buff->error) return it_bit_buff->error;
        if (tmp_index >= SPLIT_CHARACTERISTIC_COUNT_MAX)
          return (UNEXPECTED_ERROR);
        pstr_gain_modifiers->target_characteristic_right_index[b] = tmp_index;
      }
      pstr_gain_modifiers->gain_scaling_flag[b] =
          impd_read_bits_buf(it_bit_buff, 1);
      if (it_bit_buff->error) return it_bit_buff->error;
      if (pstr_gain_modifiers->gain_scaling_flag[b]) {
        temp = impd_read_bits_buf(it_bit_buff, 8);
        if (it_bit_buff->error) return it_bit_buff->error;

        pstr_gain_modifiers->attn_scaling[b] = ((temp >> 4) & 0xf) * 0.125f;
        pstr_gain_modifiers->ampl_scaling[b] = (temp & 0xf) * 0.125f;
      }

      pstr_gain_modifiers->gain_offset_flag[b] =
          impd_read_bits_buf(it_bit_buff, 1);
      if (it_bit_buff->error) return it_bit_buff->error;
      if (pstr_gain_modifiers->gain_offset_flag[b]) {
        FLOAT32 gain_offset;
        temp = impd_read_bits_buf(it_bit_buff, 6);
        if (it_bit_buff->error) return it_bit_buff->error;

        sign = ((temp >> 5) & 1);
        gain_offset = (1 + (temp & 0x1f)) * 0.25f;

        if (sign) {
          gain_offset = -gain_offset;
        }
        pstr_gain_modifiers->gain_offset[b] = gain_offset;
      }
    }
    if (band_count == 1) {
      WORD32 tmp;
      pstr_gain_modifiers->shape_filter_flag =
          impd_read_bits_buf(it_bit_buff, 1);
      if (it_bit_buff->error) return it_bit_buff->error;
      if (pstr_gain_modifiers->shape_filter_flag) {
        tmp = impd_read_bits_buf(it_bit_buff, 4);
        if (it_bit_buff->error) return it_bit_buff->error;
        if (tmp >= (SHAPE_FILTER_COUNT_MAX + 1)) return UNEXPECTED_ERROR;
        pstr_gain_modifiers->shape_filter_idx = tmp;
      }
    }
  } else if (version == 0) {
    WORD32 b, gain_scaling_flag, gain_offset_flag;
    FLOAT32 attn_scaling = 1.0f, ampl_scaling = 1.0f, gain_offset = 0.0f;

    gain_scaling_flag = impd_read_bits_buf(it_bit_buff, 1);
    if (it_bit_buff->error) return it_bit_buff->error;
    if (gain_scaling_flag) {
      temp = impd_read_bits_buf(it_bit_buff, 8);
      if (it_bit_buff->error) return it_bit_buff->error;

      attn_scaling = ((temp >> 4) & 0xf) * 0.125f;
      ampl_scaling = (temp & 0xf) * 0.125f;
    }

    gain_offset_flag = impd_read_bits_buf(it_bit_buff, 1);
    if (it_bit_buff->error) return it_bit_buff->error;
    if (gain_offset_flag) {
      temp = impd_read_bits_buf(it_bit_buff, 6);
      if (it_bit_buff->error) return it_bit_buff->error;

      sign = ((temp >> 5) & 1);
      gain_offset = (1 + (temp & 0x1f)) * 0.25f;

      if (sign) {
        gain_offset = -gain_offset;
      }
    }
    for (b = 0; b < band_count; b++) {
      pstr_gain_modifiers->target_characteristic_left_present[b] = 0;
      pstr_gain_modifiers->target_characteristic_right_present[b] = 0;
      pstr_gain_modifiers->gain_scaling_flag[b] = gain_scaling_flag;
      pstr_gain_modifiers->attn_scaling[b] = attn_scaling;
      pstr_gain_modifiers->ampl_scaling[b] = ampl_scaling;
      pstr_gain_modifiers->gain_offset_flag[b] = gain_offset_flag;
      pstr_gain_modifiers->gain_offset[b] = gain_offset;
    }
    pstr_gain_modifiers->shape_filter_flag = 0;
  }
  return (0);
}

WORD32
impd_parse_gain_set_params(ia_bit_buf_struct* it_bit_buff, WORD32 version,
                           WORD32* gain_seq_idx,
                           ia_gain_set_params_struct* gain_set_params) {
  WORD32 err = 0, i, temp;

  temp = impd_read_bits_buf(it_bit_buff, 6);
  if (it_bit_buff->error) return it_bit_buff->error;

  gain_set_params->gain_coding_profile = (temp >> 4) & 3;
  gain_set_params->gain_interpolation_type = (temp >> 3) & 1;
  gain_set_params->full_frame = (temp >> 2) & 1;
  gain_set_params->time_alignment = (temp >> 1) & 1;
  gain_set_params->time_delt_min_flag = temp & 1;

  if (gain_set_params->time_delt_min_flag) {
    WORD32 time_delta_min;
    time_delta_min = impd_read_bits_buf(it_bit_buff, 11);
    if (it_bit_buff->error) return it_bit_buff->error;
    gain_set_params->time_delt_min_val = time_delta_min + 1;
  }

  if (gain_set_params->gain_coding_profile == GAIN_CODING_PROFILE_CONSTANT) {
    gain_set_params->band_count = 1;
    *gain_seq_idx = (*gain_seq_idx) + 1;
  } else {
    gain_set_params->band_count = impd_read_bits_buf(it_bit_buff, 4);
    if (it_bit_buff->error) return it_bit_buff->error;

    if (gain_set_params->band_count > BAND_COUNT_MAX) return (UNEXPECTED_ERROR);

    if (gain_set_params->band_count > 1) {
      gain_set_params->drc_band_type = impd_read_bits_buf(it_bit_buff, 1);
      if (it_bit_buff->error) return it_bit_buff->error;
    }
    for (i = 0; i < gain_set_params->band_count; i++) {
      if (version == 0) {
        *gain_seq_idx = (*gain_seq_idx) + 1;
      } else {
        WORD32 indexPresent;
        indexPresent = impd_read_bits_buf(it_bit_buff, 1);
        if (it_bit_buff->error) return it_bit_buff->error;
        if (indexPresent) {
          WORD32 bsIndex;
          bsIndex = impd_read_bits_buf(it_bit_buff, 6);
          if (it_bit_buff->error) return it_bit_buff->error;
          *gain_seq_idx = bsIndex;
        } else {
          *gain_seq_idx = (*gain_seq_idx) + 1;
        }
      }

      if (*gain_seq_idx >= SEQUENCE_COUNT_MAX) return UNEXPECTED_ERROR;

      gain_set_params->gain_params[i].gain_seq_idx = *gain_seq_idx;
      err = impd_parse_gain_set_params_characteristics(
          it_bit_buff, version, &(gain_set_params->gain_params[i]));
      if (err) return (err);
    }
    if (gain_set_params->drc_band_type) {
      for (i = 1; i < gain_set_params->band_count; i++) {
        gain_set_params->gain_params[i].crossover_freq_idx =
            impd_read_bits_buf(it_bit_buff, 4);
        if (it_bit_buff->error) return it_bit_buff->error;
      }
    } else {
      for (i = 1; i < gain_set_params->band_count; i++) {
        gain_set_params->gain_params[i].start_subband_index =
            impd_read_bits_buf(it_bit_buff, 10);
        if (it_bit_buff->error) return it_bit_buff->error;
      }
    }
  }

  return (0);
}

WORD32
impd_sel_drc_coeff(
    ia_drc_config* drc_config, WORD32 location,
    ia_uni_drc_coeffs_struct** str_p_loc_drc_coefficients_uni_drc) {
  WORD32 n;
  WORD32 c1 = -1;
  WORD32 c0 = -1;
  for (n = 0; n < drc_config->drc_coefficients_drc_count; n++) {
    if (drc_config->str_p_loc_drc_coefficients_uni_drc[n].drc_location ==
        location) {
      if (drc_config->str_p_loc_drc_coefficients_uni_drc[n].version == 0) {
        c0 = n;
      } else {
        c1 = n;
      }
    }
  }
  if (c1 >= 0) {
    *str_p_loc_drc_coefficients_uni_drc =
        &(drc_config->str_p_loc_drc_coefficients_uni_drc[c1]);
  } else if (c0 >= 0) {
    *str_p_loc_drc_coefficients_uni_drc =
        &(drc_config->str_p_loc_drc_coefficients_uni_drc[c0]);
  } else {
    *str_p_loc_drc_coefficients_uni_drc = NULL;
  }
  return (0);
}

WORD32
impd_parse_loudness_info_set_ext(
    ia_bit_buf_struct* it_bit_buff,
    ia_drc_loudness_info_set_struct* loudness_info_set) {
  WORD32 err = 0, i, k;
  WORD32 bit_size_len, ext_size_bits, bit_size;

  k = 0;
  loudness_info_set->str_loudness_info_set_ext.loudness_info_set_ext_type[k] =
      impd_read_bits_buf(it_bit_buff, 4);
  if (it_bit_buff->error) return it_bit_buff->error;
  while (loudness_info_set->str_loudness_info_set_ext
             .loudness_info_set_ext_type[k] != UNIDRCLOUDEXT_TERM) {
    bit_size_len = impd_read_bits_buf(it_bit_buff, 4);
    if (it_bit_buff->error) return it_bit_buff->error;
    ext_size_bits = bit_size_len + 4;

    bit_size = impd_read_bits_buf(it_bit_buff, ext_size_bits);
    if (it_bit_buff->error) return it_bit_buff->error;
    if (k >= (EXT_COUNT_MAX - 1)) return UNEXPECTED_ERROR;
    loudness_info_set->str_loudness_info_set_ext.ext_bit_size[k] = bit_size + 1;

    switch (loudness_info_set->str_loudness_info_set_ext
                .loudness_info_set_ext_type[k]) {
      case UNIDRCLOUDEXT_EQ:
        err = impd_parse_loud_info_set_ext_eq(it_bit_buff, loudness_info_set);
        if (err) return (err);
        break;
      default:
        for (i = 0;
             i < loudness_info_set->str_loudness_info_set_ext.ext_bit_size[k];
             i++) {
          impd_read_bits_buf(it_bit_buff, 1);
          if (it_bit_buff->error) return it_bit_buff->error;
        }
        break;
    }
    k++;
    loudness_info_set->str_loudness_info_set_ext.loudness_info_set_ext_type[k] =
        impd_read_bits_buf(it_bit_buff, 4);
    if (it_bit_buff->error) return it_bit_buff->error;
  }

  return (0);
}

WORD32
impd_drc_parse_coeff(
    ia_bit_buf_struct* it_bit_buff, WORD32 version,
    ia_drc_params_bs_dec_struct* ia_drc_params_struct,
    ia_uni_drc_coeffs_struct* str_p_loc_drc_coefficients_uni_drc) {
  WORD32 err = 0, i, drc_frame_size, temp;
  WORD32 gain_seq_idx = -1;

  str_p_loc_drc_coefficients_uni_drc->version = version;
  if (version == 0) {
    WORD32 gain_sequence_count = 0;
    temp = impd_read_bits_buf(it_bit_buff, 5);
    if (it_bit_buff->error) return it_bit_buff->error;

    str_p_loc_drc_coefficients_uni_drc->drc_location = (temp >> 1) & 0xf;
    str_p_loc_drc_coefficients_uni_drc->drc_frame_size_present = temp & 1;

    if (str_p_loc_drc_coefficients_uni_drc->drc_frame_size_present == 1) {
      drc_frame_size = impd_read_bits_buf(it_bit_buff, 15);
      if (it_bit_buff->error) return it_bit_buff->error;
      str_p_loc_drc_coefficients_uni_drc->drc_frame_size = drc_frame_size + 1;
      if (str_p_loc_drc_coefficients_uni_drc->drc_frame_size >
          MAX_DRC_FRAME_SIZE)
        return UNEXPECTED_ERROR;
    }

    str_p_loc_drc_coefficients_uni_drc->drc_characteristic_left_present = 0;
    str_p_loc_drc_coefficients_uni_drc->drc_characteristic_right_present = 0;
    str_p_loc_drc_coefficients_uni_drc->shape_filters_present = 0;
    str_p_loc_drc_coefficients_uni_drc->gain_set_count =
        impd_read_bits_buf(it_bit_buff, 6);
    if (it_bit_buff->error) return it_bit_buff->error;

    if (str_p_loc_drc_coefficients_uni_drc->gain_set_count > GAIN_SET_COUNT_MAX)
      return (UNEXPECTED_ERROR);

    str_p_loc_drc_coefficients_uni_drc->gain_set_count_plus =
        str_p_loc_drc_coefficients_uni_drc->gain_set_count;
    for (i = 0; i < str_p_loc_drc_coefficients_uni_drc->gain_set_count; i++) {
      err = impd_parse_gain_set_params(
          it_bit_buff, version, &gain_seq_idx,
          &(str_p_loc_drc_coefficients_uni_drc->gain_set_params[i]));
      if (err) return (err);

      if (str_p_loc_drc_coefficients_uni_drc->gain_set_params[i]
              .time_delt_min_flag) {
        if (str_p_loc_drc_coefficients_uni_drc->gain_set_params[i]
                .time_delt_min_val > ia_drc_params_struct->drc_frame_size) {
          /* drc time interval too big */
          return (PARAM_ERROR);
        }
        str_p_loc_drc_coefficients_uni_drc->gain_set_params[i]
            .num_gain_max_values =
            ia_drc_params_struct->drc_frame_size /
            str_p_loc_drc_coefficients_uni_drc->gain_set_params[i]
                .time_delt_min_val;
        if (str_p_loc_drc_coefficients_uni_drc->gain_set_params[i]
                .num_gain_max_values >
            (N_DELTA_TIME_CODE_TABLE_ENTRIES_MAX / 2 - 1))
          return (UNEXPECTED_ERROR);
        impd_init_tbls(str_p_loc_drc_coefficients_uni_drc->gain_set_params[i]
                           .num_gain_max_values,
                       &(str_p_loc_drc_coefficients_uni_drc->gain_set_params[i]
                             .str_tables));
      }
      gain_sequence_count +=
          str_p_loc_drc_coefficients_uni_drc->gain_set_params[i].band_count;
    }
    str_p_loc_drc_coefficients_uni_drc->gain_sequence_count =
        gain_sequence_count;
  } else {
    ia_shape_filter_block_params_struct* pstr_shape_filter_block_params;
    for (i = 0; i < SEQUENCE_COUNT_MAX; i++) {
      str_p_loc_drc_coefficients_uni_drc
          ->gain_set_params_index_for_gain_sequence[i] = -1;
    }

    temp = impd_read_bits_buf(it_bit_buff, 5);
    if (it_bit_buff->error) return it_bit_buff->error;

    str_p_loc_drc_coefficients_uni_drc->drc_location = (temp >> 1) & 0xf;
    str_p_loc_drc_coefficients_uni_drc->drc_frame_size_present = temp & 1;

    if (str_p_loc_drc_coefficients_uni_drc->drc_frame_size_present == 1) {
      drc_frame_size = impd_read_bits_buf(it_bit_buff, 15);
      if (it_bit_buff->error) return it_bit_buff->error;
      str_p_loc_drc_coefficients_uni_drc->drc_frame_size = drc_frame_size + 1;
    }
    str_p_loc_drc_coefficients_uni_drc->drc_characteristic_left_present =
        impd_read_bits_buf(it_bit_buff, 1);
    if (it_bit_buff->error) return it_bit_buff->error;
    if (str_p_loc_drc_coefficients_uni_drc->drc_characteristic_left_present ==
        1) {
      str_p_loc_drc_coefficients_uni_drc->characteristic_left_count =
          impd_read_bits_buf(it_bit_buff, 4);
      if (it_bit_buff->error) return it_bit_buff->error;

      if (str_p_loc_drc_coefficients_uni_drc->characteristic_left_count >
          SPLIT_CHARACTERISTIC_COUNT_MAX)
        return (UNEXPECTED_ERROR);

      for (i = 1;
           i <= str_p_loc_drc_coefficients_uni_drc->characteristic_left_count;
           i++) {
        err = impd_parse_split_drc_characteristic(
            it_bit_buff, LEFT_SIDE, &(str_p_loc_drc_coefficients_uni_drc
                                          ->str_split_characteristic_left[i]));
        if (err) return (err);
      }
    }
    str_p_loc_drc_coefficients_uni_drc->drc_characteristic_right_present =
        impd_read_bits_buf(it_bit_buff, 1);
    if (it_bit_buff->error) return it_bit_buff->error;
    if (str_p_loc_drc_coefficients_uni_drc->drc_characteristic_right_present ==
        1) {
      str_p_loc_drc_coefficients_uni_drc->characteristic_right_count =
          impd_read_bits_buf(it_bit_buff, 4);
      if (it_bit_buff->error) return it_bit_buff->error;

      if (str_p_loc_drc_coefficients_uni_drc->characteristic_right_count >
          SPLIT_CHARACTERISTIC_COUNT_MAX)
        return (UNEXPECTED_ERROR);
      for (i = 1;
           i <= str_p_loc_drc_coefficients_uni_drc->characteristic_right_count;
           i++) {
        err = impd_parse_split_drc_characteristic(
            it_bit_buff, RIGHT_SIDE,
            &(str_p_loc_drc_coefficients_uni_drc
                  ->str_split_characteristic_right[i]));
        if (err) return (err);
      }
    }
    str_p_loc_drc_coefficients_uni_drc->shape_filters_present =
        impd_read_bits_buf(it_bit_buff, 1);
    if (it_bit_buff->error) return it_bit_buff->error;
    if (str_p_loc_drc_coefficients_uni_drc->shape_filters_present == 1) {
      str_p_loc_drc_coefficients_uni_drc->shape_num_filter =
          impd_read_bits_buf(it_bit_buff, 4);
      if (it_bit_buff->error) return it_bit_buff->error;
      if (str_p_loc_drc_coefficients_uni_drc->shape_num_filter >
          SHAPE_FILTER_COUNT_MAX)
        return (UNEXPECTED_ERROR);
      for (i = 1; i <= str_p_loc_drc_coefficients_uni_drc->shape_num_filter;
           i++) {
        pstr_shape_filter_block_params =
            &(str_p_loc_drc_coefficients_uni_drc
                  ->str_shape_filter_block_params[i]);
        pstr_shape_filter_block_params->lf_cut_filter_present =
            impd_read_bits_buf(it_bit_buff, 1);
        if (it_bit_buff->error) return it_bit_buff->error;

        if (pstr_shape_filter_block_params->lf_cut_filter_present == 1) {
          temp = impd_read_bits_buf(it_bit_buff, 5);
          if (it_bit_buff->error) return it_bit_buff->error;

          pstr_shape_filter_block_params->str_lf_cut_params.corner_freq_index =
              (temp >> 2) & 7;
          pstr_shape_filter_block_params->str_lf_cut_params
              .filter_strength_index = temp & 3;
        }
        pstr_shape_filter_block_params->lf_boost_filter_present =
            impd_read_bits_buf(it_bit_buff, 1);
        if (it_bit_buff->error) return it_bit_buff->error;
        if (pstr_shape_filter_block_params->lf_boost_filter_present == 1) {
          temp = impd_read_bits_buf(it_bit_buff, 5);
          if (it_bit_buff->error) return it_bit_buff->error;

          pstr_shape_filter_block_params->str_lf_boost_params
              .corner_freq_index = (temp >> 2) & 7;
          pstr_shape_filter_block_params->str_lf_boost_params
              .filter_strength_index = temp & 3;
        }
        pstr_shape_filter_block_params->hf_cut_filter_present =
            impd_read_bits_buf(it_bit_buff, 1);
        if (it_bit_buff->error) return it_bit_buff->error;
        if (pstr_shape_filter_block_params->hf_cut_filter_present == 1) {
          temp = impd_read_bits_buf(it_bit_buff, 5);
          if (it_bit_buff->error) return it_bit_buff->error;

          pstr_shape_filter_block_params->str_hfCutParams.corner_freq_index =
              (temp >> 2) & 7;
          pstr_shape_filter_block_params->str_hfCutParams
              .filter_strength_index = temp & 3;
        }
        pstr_shape_filter_block_params->hf_boost_filter_present =
            impd_read_bits_buf(it_bit_buff, 1);
        if (it_bit_buff->error) return it_bit_buff->error;
        if (pstr_shape_filter_block_params->hf_boost_filter_present == 1) {
          temp = impd_read_bits_buf(it_bit_buff, 5);
          if (it_bit_buff->error) return it_bit_buff->error;

          pstr_shape_filter_block_params->str_hf_boost_params
              .corner_freq_index = (temp >> 2) & 7;
          pstr_shape_filter_block_params->str_hf_boost_params
              .filter_strength_index = temp & 3;
        }
      }
    }

    temp = impd_read_bits_buf(it_bit_buff, 12);
    if (it_bit_buff->error) return it_bit_buff->error;

    str_p_loc_drc_coefficients_uni_drc->gain_sequence_count =
        (temp >> 6) & 0x3f;

    if (str_p_loc_drc_coefficients_uni_drc->gain_sequence_count >
        SEQUENCE_COUNT_MAX)
      return UNEXPECTED_ERROR;

    str_p_loc_drc_coefficients_uni_drc->gain_set_count = temp & 0x3f;

    if (str_p_loc_drc_coefficients_uni_drc->gain_set_count > GAIN_SET_COUNT_MAX)
      return (UNEXPECTED_ERROR);

    str_p_loc_drc_coefficients_uni_drc->gain_set_count_plus =
        str_p_loc_drc_coefficients_uni_drc->gain_set_count;
    for (i = 0; i < str_p_loc_drc_coefficients_uni_drc->gain_set_count; i++) {
      err = impd_parse_gain_set_params(
          it_bit_buff, version, &gain_seq_idx,
          &(str_p_loc_drc_coefficients_uni_drc->gain_set_params[i]));
      if (err) return (err);

      if (str_p_loc_drc_coefficients_uni_drc->gain_set_params[i]
              .time_delt_min_flag) {
        if (str_p_loc_drc_coefficients_uni_drc->gain_set_params[i]
                .time_delt_min_val > ia_drc_params_struct->drc_frame_size) {
          /* drc time interval too big */
          return (PARAM_ERROR);
        }
        str_p_loc_drc_coefficients_uni_drc->gain_set_params[i]
            .num_gain_max_values =
            ia_drc_params_struct->drc_frame_size /
            str_p_loc_drc_coefficients_uni_drc->gain_set_params[i]
                .time_delt_min_val;
        if (str_p_loc_drc_coefficients_uni_drc->gain_set_params[i]
                .num_gain_max_values >
            (N_DELTA_TIME_CODE_TABLE_ENTRIES_MAX / 2 - 1))
          return (UNEXPECTED_ERROR);
        impd_init_tbls(str_p_loc_drc_coefficients_uni_drc->gain_set_params[i]
                           .num_gain_max_values,
                       &(str_p_loc_drc_coefficients_uni_drc->gain_set_params[i]
                             .str_tables));
      }
    }

    for (i = 0; i < str_p_loc_drc_coefficients_uni_drc->gain_set_count; i++) {
      WORD32 b;
      for (b = 0;
           b <
           str_p_loc_drc_coefficients_uni_drc->gain_set_params[i].band_count;
           b++) {
        str_p_loc_drc_coefficients_uni_drc
            ->gain_set_params_index_for_gain_sequence
                [str_p_loc_drc_coefficients_uni_drc->gain_set_params[i]
                     .gain_params[b]
                     .gain_seq_idx] = i;
      }
    }
  }
  return (0);
}

WORD32
impd_drc_parse_instructions_basic(
    ia_bit_buf_struct* it_bit_buff,
    ia_drc_instructions_basic_struct* str_drc_instructions_basic) {
  // WORD32 err = 0;
  WORD32 i, limiter_peak_target, temp;
  WORD32 additional_dmix_id_present, additional_dmix_id_cnt;

  temp = impd_read_bits_buf(it_bit_buff, 18);
  if (it_bit_buff->error) return it_bit_buff->error;

  str_drc_instructions_basic->drc_set_id = (temp >> 12) & 0x3f;
  str_drc_instructions_basic->drc_location = (temp >> 8) & 0xf;
  str_drc_instructions_basic->downmix_id[0] = (temp >> 1) & 0x7f;
  additional_dmix_id_present = temp & 1;
  str_drc_instructions_basic->dwnmix_id_count = 1;

  if (additional_dmix_id_present) {
    additional_dmix_id_cnt = impd_read_bits_buf(it_bit_buff, 3);
    if (it_bit_buff->error) return it_bit_buff->error;
    for (i = 0; i < additional_dmix_id_cnt; i++) {
      str_drc_instructions_basic->downmix_id[i + 1] =
          impd_read_bits_buf(it_bit_buff, 7);
      if (it_bit_buff->error) return it_bit_buff->error;
    }
    str_drc_instructions_basic->dwnmix_id_count = 1 + additional_dmix_id_cnt;
  }

  str_drc_instructions_basic->drc_set_effect =
      impd_read_bits_buf(it_bit_buff, 16);
  if (it_bit_buff->error) return it_bit_buff->error;

  if ((str_drc_instructions_basic->drc_set_effect &
       (EFFECT_BIT_DUCK_OTHER | EFFECT_BIT_DUCK_SELF)) == 0) {
    str_drc_instructions_basic->limiter_peak_target_present =
        impd_read_bits_buf(it_bit_buff, 1);
    if (it_bit_buff->error) return it_bit_buff->error;
    if (str_drc_instructions_basic->limiter_peak_target_present) {
      limiter_peak_target = impd_read_bits_buf(it_bit_buff, 8);
      if (it_bit_buff->error) return it_bit_buff->error;
      str_drc_instructions_basic->limiter_peak_target =
          -limiter_peak_target * 0.125f;
    }
  }

  str_drc_instructions_basic->drc_set_target_loudness_present =
      impd_read_bits_buf(it_bit_buff, 1);
  if (it_bit_buff->error) return it_bit_buff->error;

  str_drc_instructions_basic->drc_set_target_loudness_value_upper = 0;
  str_drc_instructions_basic->drc_set_target_loudness_value_lower = -63;

  if (str_drc_instructions_basic->drc_set_target_loudness_present == 1) {
    WORD32 bsDrcSetTargetLoudnessValueUpper, bsDrcSetTargetLoudnessValueLower;
    bsDrcSetTargetLoudnessValueUpper = impd_read_bits_buf(it_bit_buff, 6);
    if (it_bit_buff->error) return it_bit_buff->error;
    str_drc_instructions_basic->drc_set_target_loudness_value_upper =
        bsDrcSetTargetLoudnessValueUpper - 63;

    str_drc_instructions_basic->drc_set_target_loudness_value_lower_present =
        impd_read_bits_buf(it_bit_buff, 1);
    if (it_bit_buff->error) return it_bit_buff->error;

    if (str_drc_instructions_basic
            ->drc_set_target_loudness_value_lower_present == 1) {
      bsDrcSetTargetLoudnessValueLower = impd_read_bits_buf(it_bit_buff, 6);
      if (it_bit_buff->error) return it_bit_buff->error;
      str_drc_instructions_basic->drc_set_target_loudness_value_lower =
          bsDrcSetTargetLoudnessValueLower - 63;
    }
  }

  return (0);
}

WORD32
impd_dec_ducking_scaling(ia_bit_buf_struct* it_bit_buff,
                         WORD32* ducking_scaling_flag,
                         FLOAT32* p_ducking_scaling) {
  WORD32 ducking_scaling_present, ducking_scaling, sigma, mu;

  ducking_scaling_present = impd_read_bits_buf(it_bit_buff, 1);
  if (it_bit_buff->error) return it_bit_buff->error;

  if (ducking_scaling_present == 0) {
    *ducking_scaling_flag = 0;
    *p_ducking_scaling = 1.0f;
  } else {
    *ducking_scaling_flag = 1;
    ducking_scaling = impd_read_bits_buf(it_bit_buff, 4);
    if (it_bit_buff->error) return it_bit_buff->error;

    sigma = ducking_scaling >> 3;
    mu = ducking_scaling & 0x7;

    if (sigma == 0) {
      *p_ducking_scaling = 1.0f + 0.125f * (1.0f + mu);
    } else {
      *p_ducking_scaling = 1.0f - 0.125f * (1.0f + mu);
    }
  }
  return (0);
}

WORD32
impd_parse_drc_instructions_uni_drc(
    ia_bit_buf_struct* it_bit_buff, WORD32 version, ia_drc_config* drc_config,
    ia_drc_instructions_struct* str_drc_instruction_str) {
  WORD32 err = 0, i, n, k, g, c, limiter_peak_target, idx;
  WORD32 additional_dmix_id_present, additional_dmix_id_cnt;
  ia_uni_drc_coeffs_struct* str_p_loc_drc_coefficients_uni_drc = NULL;
  WORD32 ch_cnt;
  WORD32 unique_idx[MAX_CHANNEL_COUNT];
  FLOAT32 unique_scaling[MAX_CHANNEL_COUNT];
  WORD32 match;
  WORD32 dmix_id_present;
  WORD32 repeat_parameters, repeat_parameters_cnt;
  WORD32 ducking_sequence;
  FLOAT32 factor;

  str_drc_instruction_str->drc_set_id = impd_read_bits_buf(it_bit_buff, 6);
  if (it_bit_buff->error) return it_bit_buff->error;
  if (str_drc_instruction_str->drc_set_id >= DRC_INSTRUCTIONS_COUNT_MAX)
    return UNEXPECTED_ERROR;
  if (version == 0) {
    str_drc_instruction_str->drc_set_complexity_level =
        DRC_COMPLEXITY_LEVEL_MAX;
  } else {
    str_drc_instruction_str->drc_set_complexity_level =
        impd_read_bits_buf(it_bit_buff, 4);
    if (it_bit_buff->error) return it_bit_buff->error;
  }
  str_drc_instruction_str->drc_location = impd_read_bits_buf(it_bit_buff, 4);
  if (it_bit_buff->error) return it_bit_buff->error;
  dmix_id_present = 1;
  if (version >= 1) {
    dmix_id_present = impd_read_bits_buf(it_bit_buff, 1);
    if (it_bit_buff->error) return it_bit_buff->error;
  }
  if (dmix_id_present == 1) {
    str_drc_instruction_str->downmix_id[0] = impd_read_bits_buf(it_bit_buff, 7);
    if (it_bit_buff->error) return it_bit_buff->error;
    if (version >= 1) {
      str_drc_instruction_str->drc_apply_to_dwnmix =
          impd_read_bits_buf(it_bit_buff, 1);
      if (it_bit_buff->error) return it_bit_buff->error;
    }
    if (version == 0) {
      if (str_drc_instruction_str->downmix_id[0] == 0) {
        str_drc_instruction_str->drc_apply_to_dwnmix = 0;
      } else {
        str_drc_instruction_str->drc_apply_to_dwnmix = 1;
      }
    }
    additional_dmix_id_present = impd_read_bits_buf(it_bit_buff, 1);
    if (it_bit_buff->error) return it_bit_buff->error;

    if (additional_dmix_id_present) {
      additional_dmix_id_cnt = impd_read_bits_buf(it_bit_buff, 3);
      if (it_bit_buff->error) return it_bit_buff->error;
      for (i = 0; i < additional_dmix_id_cnt; i++) {
        str_drc_instruction_str->downmix_id[i + 1] =
            impd_read_bits_buf(it_bit_buff, 7);
        if (it_bit_buff->error) return it_bit_buff->error;
      }
      str_drc_instruction_str->dwnmix_id_count = 1 + additional_dmix_id_cnt;
    } else {
      str_drc_instruction_str->dwnmix_id_count = 1;
    }
  } else {
    str_drc_instruction_str->downmix_id[0] = 0;
    str_drc_instruction_str->dwnmix_id_count = 1;
  }

  str_drc_instruction_str->drc_set_effect = impd_read_bits_buf(it_bit_buff, 16);
  if (it_bit_buff->error) return it_bit_buff->error;

  if ((str_drc_instruction_str->drc_set_effect &
       (EFFECT_BIT_DUCK_OTHER | EFFECT_BIT_DUCK_SELF)) == 0) {
    str_drc_instruction_str->limiter_peak_target_present =
        impd_read_bits_buf(it_bit_buff, 1);
    if (it_bit_buff->error) return it_bit_buff->error;
    if (str_drc_instruction_str->limiter_peak_target_present) {
      limiter_peak_target = impd_read_bits_buf(it_bit_buff, 8);
      if (it_bit_buff->error) return it_bit_buff->error;
      str_drc_instruction_str->limiter_peak_target =
          -limiter_peak_target * 0.125f;
    }
  }

  str_drc_instruction_str->drc_set_target_loudness_present =
      impd_read_bits_buf(it_bit_buff, 1);
  if (it_bit_buff->error) return it_bit_buff->error;

  str_drc_instruction_str->drc_set_target_loudness_value_upper = 0;
  str_drc_instruction_str->drc_set_target_loudness_value_lower = -63;

  if (str_drc_instruction_str->drc_set_target_loudness_present == 1) {
    WORD32 bsDrcSetTargetLoudnessValueUpper, bsDrcSetTargetLoudnessValueLower;
    bsDrcSetTargetLoudnessValueUpper = impd_read_bits_buf(it_bit_buff, 6);
    if (it_bit_buff->error) return it_bit_buff->error;
    str_drc_instruction_str->drc_set_target_loudness_value_upper =
        bsDrcSetTargetLoudnessValueUpper - 63;
    str_drc_instruction_str->drc_set_target_loudness_value_lower_present =
        impd_read_bits_buf(it_bit_buff, 1);
    if (it_bit_buff->error) return it_bit_buff->error;
    if (str_drc_instruction_str->drc_set_target_loudness_value_lower_present ==
        1) {
      bsDrcSetTargetLoudnessValueLower = impd_read_bits_buf(it_bit_buff, 6);
      if (it_bit_buff->error) return it_bit_buff->error;
      str_drc_instruction_str->drc_set_target_loudness_value_lower =
          bsDrcSetTargetLoudnessValueLower - 63;
    }
  }

  str_drc_instruction_str->depends_on_drc_set_present =
      impd_read_bits_buf(it_bit_buff, 1);
  if (it_bit_buff->error) return it_bit_buff->error;

  str_drc_instruction_str->no_independent_use = 0;
  if (str_drc_instruction_str->depends_on_drc_set_present) {
    str_drc_instruction_str->depends_on_drc_set =
        impd_read_bits_buf(it_bit_buff, 6);
    if (it_bit_buff->error) return it_bit_buff->error;
  } else {
    str_drc_instruction_str->no_independent_use =
        impd_read_bits_buf(it_bit_buff, 1);
    if (it_bit_buff->error) return it_bit_buff->error;
  }
  if (version == 0) {
    str_drc_instruction_str->requires_eq = 0;
  } else {
    str_drc_instruction_str->requires_eq = impd_read_bits_buf(it_bit_buff, 1);
    if (it_bit_buff->error) return it_bit_buff->error;
  }

  err = impd_sel_drc_coeff(drc_config, str_drc_instruction_str->drc_location,
                           &str_p_loc_drc_coefficients_uni_drc);
  if (err) return (err);

  ch_cnt = drc_config->channel_layout.base_channel_count;

  if (ch_cnt > MAX_CHANNEL_COUNT) return (UNEXPECTED_ERROR);
  for (c = 0; c < MAX_CHANNEL_COUNT; c++) {
    unique_idx[c] = -10;
    unique_scaling[c] = -10.0f;
  }

  if (str_drc_instruction_str->drc_set_effect &
      (EFFECT_BIT_DUCK_OTHER | EFFECT_BIT_DUCK_SELF)) {
    c = 0;
    while (c < ch_cnt) {
      WORD32 bs_gain_set_idx;
      bs_gain_set_idx = impd_read_bits_buf(it_bit_buff, 6);
      if (it_bit_buff->error) return it_bit_buff->error;
      if (bs_gain_set_idx > GAIN_SET_COUNT_MAX) return UNEXPECTED_ERROR;
      str_drc_instruction_str->gain_set_index[c] = bs_gain_set_idx - 1;
      impd_dec_ducking_scaling(
          it_bit_buff,
          &(str_drc_instruction_str->str_ducking_modifiers_for_channel[c]
                .ducking_scaling_flag),
          &(str_drc_instruction_str->str_ducking_modifiers_for_channel[c]
                .ducking_scaling));

      c++;

      repeat_parameters = impd_read_bits_buf(it_bit_buff, 1);
      if (it_bit_buff->error) return it_bit_buff->error;

      if (repeat_parameters == 1) {
        repeat_parameters_cnt = impd_read_bits_buf(it_bit_buff, 5);
        if (it_bit_buff->error) return it_bit_buff->error;

        repeat_parameters_cnt += 1;
        for (k = 0; k < repeat_parameters_cnt; k++) {
          if (c > (MAX_CHANNEL_COUNT-1))
            return (UNEXPECTED_ERROR);
          str_drc_instruction_str->gain_set_index[c] =
              str_drc_instruction_str->gain_set_index[c - 1];
          str_drc_instruction_str->str_ducking_modifiers_for_channel[c] =
              str_drc_instruction_str->str_ducking_modifiers_for_channel[c - 1];
          c++;
        }
      }
    }
    if (c > ch_cnt) {
      return (UNEXPECTED_ERROR);
    }
    ducking_sequence = -1;
    g = 0;
    if (str_drc_instruction_str->drc_set_effect & EFFECT_BIT_DUCK_OTHER) {
      for (c = 0; c < ch_cnt; c++) {
        match = 0;
        idx = str_drc_instruction_str->gain_set_index[c];
        factor = str_drc_instruction_str->str_ducking_modifiers_for_channel[c]
                     .ducking_scaling;
        if (idx < 0) {
          for (n = 0; n < g; n++) {
            if (unique_scaling[n] == factor) {
              match = 1;
              str_drc_instruction_str->channel_group_of_ch[c] = n;
              break;
            }
          }
          if (match == 0) {
            unique_idx[g] = idx;
            unique_scaling[g] = factor;
            str_drc_instruction_str->channel_group_of_ch[c] = g;
            g++;
          }
        } else {
          if ((ducking_sequence > 0) && (ducking_sequence != idx)) {
            /* drc for ducking can have only one ducking sequence */
            return (UNEXPECTED_ERROR);
          }
          ducking_sequence = idx;
          str_drc_instruction_str->channel_group_of_ch[c] = -1;
        }
      }
      str_drc_instruction_str->num_drc_ch_groups = g;
      if (ducking_sequence == -1) {
        /* ducking sequence not found */
        return (UNEXPECTED_ERROR);
      }
    } else if (str_drc_instruction_str->drc_set_effect & EFFECT_BIT_DUCK_SELF) {
      for (c = 0; c < ch_cnt; c++) {
        match = 0;
        idx = str_drc_instruction_str->gain_set_index[c];
        factor = str_drc_instruction_str->str_ducking_modifiers_for_channel[c]
                     .ducking_scaling;
        if (idx >= 0) {
          for (n = 0; n < g; n++) {
            if ((unique_idx[n] == idx) && (unique_scaling[n] == factor)) {
              match = 1;
              str_drc_instruction_str->channel_group_of_ch[c] = n;
              break;
            }
          }
          if (match == 0) {
            unique_idx[g] = idx;
            unique_scaling[g] = factor;
            str_drc_instruction_str->channel_group_of_ch[c] = g;
            g++;
          }
        } else {
          str_drc_instruction_str->channel_group_of_ch[c] = -1;
        }
      }
      str_drc_instruction_str->num_drc_ch_groups = g;
    }

    if (str_drc_instruction_str->num_drc_ch_groups >
        min(CHANNEL_GROUP_COUNT_MAX, MAX_CHANNEL_COUNT))
      return UNEXPECTED_ERROR;
    for (g = 0; g < str_drc_instruction_str->num_drc_ch_groups; g++) {
      WORD32 set =
          (str_drc_instruction_str->drc_set_effect & EFFECT_BIT_DUCK_OTHER)
              ? ducking_sequence
              : unique_idx[g];
      if (set < 0) return UNEXPECTED_ERROR;
      str_drc_instruction_str->gain_set_index_for_channel_group[g] = set;
      str_drc_instruction_str->str_ducking_modifiers_for_channel_group[g]
          .ducking_scaling = unique_scaling[g];
      if (unique_scaling[g] != 1.0f) {
        str_drc_instruction_str->str_ducking_modifiers_for_channel_group[g]
            .ducking_scaling_flag = 1;
      } else {
        str_drc_instruction_str->str_ducking_modifiers_for_channel_group[g]
            .ducking_scaling_flag = 0;
      }
      str_drc_instruction_str->band_count_of_ch_group[g] = 1;
    }
  } else {
    if (((version == 0) ||
         (str_drc_instruction_str->drc_apply_to_dwnmix != 0)) &&
        (str_drc_instruction_str->downmix_id[0] != 0) &&
        (str_drc_instruction_str->downmix_id[0] != ID_FOR_ANY_DOWNMIX) &&
        (str_drc_instruction_str->dwnmix_id_count == 1)) {
      for (i = 0; i < drc_config->dwnmix_instructions_count; i++) {
        if (str_drc_instruction_str->downmix_id[0] ==
            drc_config->dwnmix_instructions[i].downmix_id)
          break;
      }
      if (i == drc_config->dwnmix_instructions_count) {
        /* dwnmix_instructions not found */
        return (UNEXPECTED_ERROR);
      }
      ch_cnt = drc_config->dwnmix_instructions[i].target_channel_count;
    } else if (((version == 0) ||
                (str_drc_instruction_str->drc_apply_to_dwnmix != 0)) &&
               ((str_drc_instruction_str->downmix_id[0] ==
                 ID_FOR_ANY_DOWNMIX) ||
                (str_drc_instruction_str->dwnmix_id_count > 1))) {
      ch_cnt = 1;
    }

    if (ch_cnt > MAX_CHANNEL_COUNT) return (UNEXPECTED_ERROR);
    c = 0;
    while (c < ch_cnt) {
      WORD32 bs_gain_set_idx;
      WORD32 repeat_gain_set_idx, repeat_gain_set_idx_cnt, temp;

      temp = impd_read_bits_buf(it_bit_buff, 7);
      if (it_bit_buff->error) return it_bit_buff->error;

      bs_gain_set_idx = (temp >> 1) & 0x7f;
      repeat_gain_set_idx = temp & 1;

      if (bs_gain_set_idx > GAIN_SET_COUNT_MAX) return UNEXPECTED_ERROR;

      str_drc_instruction_str->gain_set_index[c] = bs_gain_set_idx - 1;
      c++;

      if (repeat_gain_set_idx == 1) {
        repeat_gain_set_idx_cnt = impd_read_bits_buf(it_bit_buff, 5);
        if (it_bit_buff->error) return it_bit_buff->error;

        repeat_gain_set_idx_cnt += 1;
        for (k = 0; k < repeat_gain_set_idx_cnt; k++) {
          if (c > (MAX_CHANNEL_COUNT - 1))
            return (UNEXPECTED_ERROR);
          str_drc_instruction_str->gain_set_index[c] = bs_gain_set_idx - 1;
          c++;
        }
      }
    }
    if (c > ch_cnt) {
      return (UNEXPECTED_ERROR);
    }

    g = 0;
    if ((str_drc_instruction_str->downmix_id[0] == ID_FOR_ANY_DOWNMIX) ||
        (str_drc_instruction_str->dwnmix_id_count > 1)) {
      WORD32 idx = str_drc_instruction_str->gain_set_index[0];
      if (idx >= 0) {
        unique_idx[0] = idx;
        g = 1;
      }
    } else {
      for (c = 0; c < ch_cnt; c++) {
        WORD32 idx = str_drc_instruction_str->gain_set_index[c];
        match = 0;
        if (idx >= 0) {
          for (n = 0; n < g; n++) {
            if (unique_idx[n] == idx) {
              match = 1;
              str_drc_instruction_str->channel_group_of_ch[c] = n;
              break;
            }
          }
          if (match == 0) {
            unique_idx[g] = idx;
            str_drc_instruction_str->channel_group_of_ch[c] = g;
            g++;
          }
        } else {
          str_drc_instruction_str->channel_group_of_ch[c] = -1;
        }
      }
    }

    str_drc_instruction_str->num_drc_ch_groups = g;

    if (str_drc_instruction_str->num_drc_ch_groups >
        min(CHANNEL_GROUP_COUNT_MAX, MAX_CHANNEL_COUNT))
      return UNEXPECTED_ERROR;
    for (g = 0; g < str_drc_instruction_str->num_drc_ch_groups; g++) {
      WORD32 set, band_count;

      set = unique_idx[g];
      if (set < 0) return UNEXPECTED_ERROR;
      str_drc_instruction_str->gain_set_index_for_channel_group[g] = set;

      if (str_p_loc_drc_coefficients_uni_drc != NULL &&
          set < str_p_loc_drc_coefficients_uni_drc->gain_set_count) {
        band_count =
            str_p_loc_drc_coefficients_uni_drc->gain_set_params[set].band_count;
      } else {
        band_count = 1;
      }

      err = impd_dec_gain_modifiers(
          it_bit_buff, version, band_count,
          &(str_drc_instruction_str->str_gain_modifiers_of_ch_group[g]));
      if (err) return (err);
    }
  }

  return (0);
}
WORD32
impd_parse_loudness_info(ia_bit_buf_struct* it_bit_buff, WORD32 version,
                         ia_loudness_info_struct* loudness_info) {
  WORD32 err = 0, sample_peak_level, true_peak_level, i, temp;

  loudness_info->drc_set_id = impd_read_bits_buf(it_bit_buff, 6);
  if (it_bit_buff->error) return it_bit_buff->error;

  if (version >= 1) {
    loudness_info->eq_set_id = impd_read_bits_buf(it_bit_buff, 6);
    if (it_bit_buff->error) return it_bit_buff->error;
  } else {
    loudness_info->eq_set_id = 0;
  }

  temp = impd_read_bits_buf(it_bit_buff, 8);
  if (it_bit_buff->error) return it_bit_buff->error;

  loudness_info->downmix_id = (temp >> 1) & 0x7f;
  loudness_info->sample_peak_level_present = temp & 1;

  if (loudness_info->sample_peak_level_present) {
    sample_peak_level = impd_read_bits_buf(it_bit_buff, 12);
    if (it_bit_buff->error) return it_bit_buff->error;

    if (sample_peak_level == 0) {
      loudness_info->sample_peak_level_present = 0;
      loudness_info->sample_peak_level = 0.0f;
    } else {
      loudness_info->sample_peak_level = 20.0f - sample_peak_level * 0.03125f;
    }
  }

  loudness_info->true_peak_level_present = impd_read_bits_buf(it_bit_buff, 1);
  if (it_bit_buff->error) return it_bit_buff->error;

  if (loudness_info->true_peak_level_present) {
    true_peak_level = impd_read_bits_buf(it_bit_buff, 12);
    if (it_bit_buff->error) return it_bit_buff->error;

    if (true_peak_level == 0) {
      loudness_info->true_peak_level_present = 0;
      loudness_info->true_peak_level = 0.0f;
    } else {
      loudness_info->true_peak_level = 20.0f - true_peak_level * 0.03125f;
    }

    temp = impd_read_bits_buf(it_bit_buff, 6);
    if (it_bit_buff->error) return it_bit_buff->error;

    /* Parsed but unused */
    loudness_info->true_peak_level_measurement_system = (temp >> 2) & 0xf;
    /* Parsed but unused */
    loudness_info->true_peak_level_reliability = temp & 3;
  }

  loudness_info->measurement_count = impd_read_bits_buf(it_bit_buff, 4);
  if (it_bit_buff->error) return it_bit_buff->error;

  for (i = 0; i < loudness_info->measurement_count; i++) {
    err = impd_parse_loudness_measure(it_bit_buff,
                                      &(loudness_info->loudness_measure[i]));

    if (loudness_info->loudness_measure[i].method_def ==
        METHOD_DEFINITION_ANCHOR_LOUDNESS)
      loudness_info->anchor_loudness_present = 1;

    if (loudness_info->loudness_measure[i].measurement_system ==
        MEASUREMENT_SYSTEM_EXPERT_PANEL)
      loudness_info->expert_loudness_present = 1;

    if (err) return (err);
  }

  return (0);
}
