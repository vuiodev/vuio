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

#include <math.h>
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
#include "impd_drc_mux.h"

static IA_ERRORCODE impd_drc_get_drc_complexity_level(
    ia_drc_uni_drc_config_struct *pstr_uni_drc_config, ia_drc_gain_enc_struct *pstr_drc_gain_enc,
    ia_drc_instructions_uni_drc *pstr_drc_instructions_uni_drc, VOID *ptr_scratch) {
  IA_ERRORCODE err_code = IA_NO_ERROR;
  LOOPIDX c, g, k, i, group;
  WORD32 band_count;
  WORD32 gain_set_index;
  WORD32 skip_set;
  WORD32 gain_set_index_offset;
  WORD32 parametric_drc_type = 0;
  WORD32 channel_count_side_chain;
  WORD32 channel_count_drom_downmix_id;
  WORD32 channel_count_temp;
  WORD32 weighting_filter_order;
  WORD32 channel_count = pstr_uni_drc_config->str_channel_layout.base_ch_count;
  FLOAT32 w_mod, cplx, cplx_tmp, ratio;
  ia_drc_gain_modifiers_struct *pstr_gain_modifiers = NULL;
  ia_drc_shape_filter_block_params_struct *pstr_shape_filter_block_params = NULL;
  ia_drc_gain_set_params_struct *pstr_gain_set_params = NULL;
  ia_drc_parametric_drc_instructions_struct *pstr_parametric_drc_instructions = NULL;
  ia_drc_parametric_drc_gain_set_params_struct *pstr_parametric_drc_gain_set_params = NULL;

  if (pstr_drc_instructions_uni_drc->drc_apply_to_downmix != 0 &&
      ((pstr_drc_instructions_uni_drc->downmix_id == 0x7F) ||
       (pstr_drc_instructions_uni_drc->additional_downmix_id_count != 0))) {
    channel_count = 1;
  } else if ((pstr_drc_instructions_uni_drc->drc_apply_to_downmix != 0) &&
             (pstr_drc_instructions_uni_drc->downmix_id != 0) &&
             (pstr_drc_instructions_uni_drc->downmix_id != 0x7F) &&
             (pstr_drc_instructions_uni_drc->additional_downmix_id_count == 0)) {
    for (i = 0; i < pstr_uni_drc_config->downmix_instructions_count; i++) {
      if (pstr_drc_instructions_uni_drc->downmix_id ==
          pstr_uni_drc_config->str_downmix_instructions[i].downmix_id) {
        break;
      }
    }
    if (i == pstr_uni_drc_config->downmix_instructions_count) {
      return IA_EXHEAACE_CONFIG_FATAL_DRC_INVALID_CONFIG;
    }
    channel_count = pstr_uni_drc_config->str_downmix_instructions[i].target_ch_count;
  }

  pstr_drc_instructions_uni_drc->drc_channel_count = channel_count;
  group = 0;
  for (c = 0; c < pstr_drc_instructions_uni_drc->drc_channel_count; c++) {
    gain_set_index = pstr_drc_instructions_uni_drc->gain_set_index[c];
    skip_set = FALSE;
    if (gain_set_index < 0) {
      pstr_drc_instructions_uni_drc->channel_group_for_channel[c] = -1;
    } else {
      for (k = c - 1; k >= 0; k--) {
        if (pstr_drc_instructions_uni_drc->gain_set_index[k] == gain_set_index) {
          pstr_drc_instructions_uni_drc->channel_group_for_channel[c] =
              pstr_drc_instructions_uni_drc->channel_group_for_channel[k];
          skip_set = TRUE;
        }
      }
      if (skip_set == FALSE) {
        pstr_drc_instructions_uni_drc->channel_group_for_channel[c] = group;
        group++;
      }
    }
  }
  if (group != pstr_drc_instructions_uni_drc->num_drc_channel_groups) {
    return IA_EXHEAACE_CONFIG_FATAL_DRC_INVALID_CONFIG;
  }

  for (g = 0; g < pstr_drc_instructions_uni_drc->num_drc_channel_groups; g++) {
    pstr_drc_instructions_uni_drc->num_channels_per_channel_group[g] = 0;
    for (c = 0; c < pstr_drc_instructions_uni_drc->drc_channel_count; c++) {
      if (pstr_drc_instructions_uni_drc->channel_group_for_channel[c] == g) {
        pstr_drc_instructions_uni_drc->num_channels_per_channel_group[g]++;
      }
    }
  }

  cplx = 0.0f;

  if (pstr_drc_gain_enc->domain == TIME_DOMAIN) {
    w_mod = COMPLEXITY_W_MOD_TIME;
  } else {
    w_mod = COMPLEXITY_W_MOD_SUBBAND;
  }
  for (c = 0; c < pstr_drc_instructions_uni_drc->drc_channel_count; c++) {
    if (pstr_drc_instructions_uni_drc->gain_set_index[c] >= 0) {
      cplx += w_mod;
    }
  }

  if (pstr_drc_gain_enc->domain != TIME_DOMAIN) {
    pstr_gain_set_params =
        pstr_uni_drc_config->str_uni_drc_config_ext.str_drc_coefficients_uni_drc_v1[0]
            .str_gain_set_params;
    for (c = 0; c < pstr_drc_instructions_uni_drc->drc_channel_count; c++) {
      gain_set_index = pstr_drc_instructions_uni_drc->gain_set_index[c];
      if (gain_set_index >= 0) {
        if (pstr_gain_set_params[gain_set_index].drc_band_type == 1) {
          band_count = pstr_gain_set_params[gain_set_index].band_count;
          if (band_count > 1) {
            cplx += COMPLEXITY_W_LAP * band_count;
          }
        }
      }
    }
  } else {
    err_code = impd_drc_init_all_filter_banks(
        &pstr_uni_drc_config->str_uni_drc_config_ext.str_drc_coefficients_uni_drc_v1[0],
        pstr_drc_instructions_uni_drc, &pstr_drc_gain_enc->str_filter_banks, ptr_scratch);

    if (err_code) return err_code;

    cplx += COMPLEXITY_W_IIR * pstr_drc_gain_enc->str_filter_banks.complexity;

    for (g = 0; g < pstr_drc_instructions_uni_drc->num_drc_channel_groups; g++) {
      pstr_gain_modifiers = &pstr_drc_instructions_uni_drc->str_gain_modifiers[g];
      if (pstr_gain_modifiers->shape_filter_present == 1) {
        cplx_tmp = 0.0f;
        pstr_shape_filter_block_params =
            &pstr_uni_drc_config->str_uni_drc_config_ext.str_drc_coefficients_uni_drc_v1[0]
                 .str_shape_filter_block_params[pstr_gain_modifiers->shape_filter_index];
        if (pstr_shape_filter_block_params->lf_cut_filter_present == 1) {
          cplx_tmp += COMPLEXITY_W_SHAPE;
        }
        if (pstr_shape_filter_block_params->lf_boost_filter_present == 1) {
          cplx_tmp += COMPLEXITY_W_SHAPE;
        }
        if (pstr_shape_filter_block_params->hf_cut_filter_present == 1) {
          cplx_tmp += COMPLEXITY_W_SHAPE * 2.0f;
        }
        if (pstr_shape_filter_block_params->hf_boost_filter_present == 1) {
          cplx_tmp += COMPLEXITY_W_SHAPE * 2.0f;
        }
        cplx += cplx_tmp * pstr_drc_instructions_uni_drc->num_channels_per_channel_group[g];
      }
    }
  }

  for (g = 0; g < pstr_drc_instructions_uni_drc->num_drc_channel_groups; g++) {
    gain_set_index_offset = 0;
    gain_set_index = -1;
    for (c = 0; c < pstr_drc_instructions_uni_drc->drc_channel_count; c++) {
      if (pstr_drc_instructions_uni_drc->channel_group_for_channel[c] == g) {
        gain_set_index = pstr_drc_instructions_uni_drc->gain_set_index[c];
        break;
      }
    }
    if (pstr_uni_drc_config->str_uni_drc_config_ext.drc_coefficients_uni_drc_v1_count > 0) {
      gain_set_index_offset =
          pstr_uni_drc_config->str_uni_drc_config_ext.str_drc_coefficients_uni_drc_v1[0]
              .gain_set_count;
      pstr_gain_set_params =
          pstr_uni_drc_config->str_uni_drc_config_ext.str_drc_coefficients_uni_drc_v1[0]
              .str_gain_set_params;
    }
    if (gain_set_index >= gain_set_index_offset) {
      pstr_parametric_drc_instructions = NULL;
      pstr_parametric_drc_gain_set_params =
          &pstr_uni_drc_config->str_uni_drc_config_ext.str_drc_coeff_parametric_drc
               .parametric_drc_gain_set_params[gain_set_index - gain_set_index_offset];
      for (i = 0;
           i < pstr_uni_drc_config->str_uni_drc_config_ext.parametric_drc_instructions_count;
           i++) {
        if (pstr_parametric_drc_gain_set_params->parametric_drc_id ==
            pstr_uni_drc_config->str_uni_drc_config_ext.str_parametric_drc_instructions[i]
                .parametric_drc_id) {
          pstr_parametric_drc_instructions =
              &pstr_uni_drc_config->str_uni_drc_config_ext.str_parametric_drc_instructions[i];
          break;
        }
      }
      if (pstr_parametric_drc_instructions != NULL) {
        if (pstr_parametric_drc_instructions->parametric_drc_preset_id_present) {
          switch (pstr_parametric_drc_instructions->parametric_drc_preset_id) {
            case 0:
            case 1:
            case 2:
            case 3:
            case 4:
              parametric_drc_type = PARAM_DRC_TYPE_FF;
              break;
            case 5:
              parametric_drc_type = PARAM_DRC_TYPE_LIM;
              break;
            default:
              return IA_EXHEAACE_CONFIG_FATAL_DRC_INVALID_CONFIG;
              break;
          }
        } else {
          parametric_drc_type = pstr_parametric_drc_instructions->parametric_drc_type;
        }
      }
      channel_count_side_chain = pstr_drc_instructions_uni_drc->num_channels_per_channel_group[g];
      if (pstr_parametric_drc_gain_set_params->side_chain_config_type == 1) {
        channel_count_temp = 0;
        if (pstr_parametric_drc_gain_set_params->downmix_id == 0x0) {
          channel_count_drom_downmix_id = pstr_uni_drc_config->str_channel_layout.base_ch_count;
        } else if (pstr_parametric_drc_gain_set_params->downmix_id == 0x7F) {
          channel_count_drom_downmix_id = 1;
        } else {
          for (i = 0; i < pstr_uni_drc_config->downmix_instructions_count; i++) {
            if (pstr_parametric_drc_gain_set_params->downmix_id ==
                pstr_uni_drc_config->str_downmix_instructions[i].downmix_id) {
              break;
            }
          }
          if (i == pstr_uni_drc_config->downmix_instructions_count) {
            return IA_EXHEAACE_CONFIG_FATAL_DRC_INVALID_CONFIG;
          }
          channel_count_drom_downmix_id =
              pstr_uni_drc_config->str_downmix_instructions[i].target_ch_count;
        }

        for (i = 0; i < channel_count_drom_downmix_id; i++) {
          if (pstr_parametric_drc_gain_set_params->level_estim_channel_weight[i] != 0) {
            channel_count_temp++;
          }
        }
        channel_count_side_chain = channel_count_temp;
      }
      if (pstr_parametric_drc_instructions != NULL) {
        if (pstr_drc_gain_enc->domain == TIME_DOMAIN) {
          if (parametric_drc_type == PARAM_DRC_TYPE_FF) {
            weighting_filter_order = 2;
            if (pstr_parametric_drc_instructions->parametric_drc_preset_id_present == 0) {
              if (pstr_parametric_drc_instructions->str_parametric_drc_type_feed_forward
                      .level_estim_k_weighting_type == 0) {
                weighting_filter_order = 0;
              } else if (pstr_parametric_drc_instructions->str_parametric_drc_type_feed_forward
                             .level_estim_k_weighting_type == 1) {
                weighting_filter_order = 1;
              }
            }
            cplx += channel_count_side_chain *
                        (COMPLEXITY_W_PARAM_DRC_FILT * weighting_filter_order + 1) +
                    3;
          } else if (parametric_drc_type == PARAM_DRC_TYPE_LIM) {
            ratio = 1.0f;
            if (pstr_parametric_drc_instructions->parametric_drc_look_ahead_present == 1) {
              ratio = (FLOAT32)pstr_parametric_drc_instructions->parametric_drc_look_ahead /
                      (FLOAT32)PARAM_DRC_TYPE_LIM_ATTACK_DEFAULT;
            }
            cplx += (FLOAT32)(channel_count_side_chain * COMPLEXITY_W_PARAM_LIM_FILT +
                              COMPLEXITY_W_PARAM_DRC_ATTACK * sqrt(ratio));
          }
        } else {
          if (parametric_drc_type == PARAM_DRC_TYPE_FF) {
            cplx += channel_count_side_chain * COMPLEXITY_W_PARAM_DRC_SUBBAND;
          }
        }
      }
    } else {
      if (pstr_drc_gain_enc->domain == TIME_DOMAIN && pstr_gain_set_params != NULL) {
        if (pstr_gain_set_params[gain_set_index].gain_interpolation_type ==
            GAIN_INTERPOLATION_TYPE_SPLINE) {
          cplx += COMPLEXITY_W_SPLINE;
        }
        if (pstr_gain_set_params[gain_set_index].gain_interpolation_type ==
            GAIN_INTERPOLATION_TYPE_LINEAR) {
          cplx += COMPLEXITY_W_LINEAR;
        }
      }
    }
  }

  if (pstr_drc_instructions_uni_drc->downmix_id == 0x7F) {
    channel_count = pstr_uni_drc_config->str_channel_layout.base_ch_count;
  }

  cplx = (FLOAT32)(log10(cplx / channel_count) / log10(2.0f));
  pstr_drc_instructions_uni_drc->drc_set_complexity_level = (WORD32)MAX(0, ceil(cplx));

  if (pstr_drc_instructions_uni_drc->drc_set_complexity_level > DRC_COMPLEXITY_LEVEL_MAX) {
    return IA_EXHEAACE_CONFIG_FATAL_DRC_INVALID_CONFIG;
  }

  return IA_NO_ERROR;
}

static IA_ERRORCODE impd_drc_get_eq_complexity_level(
    ia_drc_uni_drc_config_ext_struct *pstr_uni_drc_config_ext,
    ia_drc_gain_enc_struct *pstr_drc_params, ia_drc_eq_instructions_struct *pstr_eq_instructions,
    VOID *ptr_scratch, WORD32 *scratch_used) {
  IA_ERRORCODE err_code = IA_NO_ERROR;
  WORD32 subband_domain_mode;
  ia_drc_eq_set_struct *pstr_eq_set = &pstr_drc_params->str_eq_set;

  if ((pstr_drc_params->domain == TIME_DOMAIN) &&
      (pstr_eq_instructions->td_filter_cascade_present == 0) &&
      (pstr_eq_instructions->subband_gains_present == 0)) {
    pstr_eq_instructions->eq_set_complexity_level = 0;
    return err_code;
  }

  subband_domain_mode = SUBBAND_DOMAIN_MODE_OFF;
  if (pstr_eq_instructions->subband_gains_present == 1) {
    subband_domain_mode = SUBBAND_DOMAIN_MODE_QMF64;
  }
  memset(pstr_eq_set, 0, sizeof(ia_drc_eq_set_struct));

  err_code = impd_drc_derive_eq_set(&pstr_uni_drc_config_ext->str_eq_coefficients,
                                    pstr_eq_instructions, (FLOAT32)pstr_drc_params->sample_rate,
                                    pstr_drc_params->drc_frame_size, subband_domain_mode,
                                    pstr_eq_set, ptr_scratch, scratch_used);
  if (err_code & IA_FATAL_ERROR) {
    return err_code;
  }

  err_code =
      impd_drc_get_eq_complexity(pstr_eq_set, &pstr_eq_instructions->eq_set_complexity_level);
  if (err_code & IA_FATAL_ERROR) {
    return err_code;
  }

  return err_code;
}

static WORD32 impd_drc_encode_downmix_coefficient(FLOAT32 downmix_coeff, FLOAT32 downmix_offset) {
  WORD32 idx, code;
  FLOAT32 coeff_db;
  const FLOAT32 *coeff_table;

  coeff_table = impd_drc_downmix_coeff_v1;
  coeff_db = 20.0f * (FLOAT32)log10(downmix_coeff) - downmix_offset;

  if (coeff_db >= coeff_table[30]) {
    idx = 0;
    while (coeff_db < coeff_table[idx]) {
      idx++;
    }
    if ((idx > 0) && (coeff_db > 0.5f * (coeff_table[idx - 1] + coeff_table[idx]))) {
      idx--;
    }
    code = idx;
  } else {
    code = 31;
  }

  return code;
}

static VOID impd_drc_dec_write_downmix_coeff_v1(ia_bit_buf_struct *it_bit_buf,
                                                const FLOAT32 downmix_coeff[],
                                                const WORD32 base_ch_count,
                                                const WORD32 target_ch_count,
                                                WORD32 *ptr_bit_cnt) {
  LOOPIDX i, j;
  WORD32 bs_downmix_offset = 0, code;
  WORD32 bit_cnt_local = 0;
  FLOAT32 downmix_offset[3];
  FLOAT32 tmp;
  FLOAT32 quant_err, quant_err_min;
  const FLOAT32 *coeff_table;

  coeff_table = impd_drc_downmix_coeff_v1;
  tmp = (FLOAT32)log10((FLOAT32)target_ch_count / (FLOAT32)base_ch_count);
  downmix_offset[0] = 0.0f;
  downmix_offset[1] = (FLOAT32)(0.5f * floor(0.5f + 20.0f * tmp));
  downmix_offset[2] = (FLOAT32)(0.5f * floor(0.5f + 40.0f * tmp));

  quant_err_min = 1000.0f;
  for (i = 0; i < 3; i++) {
    quant_err = 0.0f;
    for (j = 0; j < (target_ch_count * base_ch_count); j++) {
      code = impd_drc_encode_downmix_coefficient(downmix_coeff[j], downmix_offset[i]);
      quant_err += (FLOAT32)fabs(20.0f * log10(downmix_coeff[j]) -
                                 (coeff_table[code] + downmix_offset[i]));
    }
    if (quant_err_min > quant_err) {
      quant_err_min = quant_err;
      bs_downmix_offset = i;
    }
  }

  bit_cnt_local += iusace_write_bits_buf(it_bit_buf, bs_downmix_offset, 4);

  for (j = 0; j < target_ch_count * base_ch_count; j++) {
    code =
        impd_drc_encode_downmix_coefficient(downmix_coeff[j], downmix_offset[bs_downmix_offset]);
    bit_cnt_local += iusace_write_bits_buf(it_bit_buf, code, 5);
  }

  *ptr_bit_cnt += bit_cnt_local;
}

static VOID impd_drc_enc_downmix_coeff(const FLOAT32 downmix_coeff_var,
                                       const WORD32 is_lfe_channel, WORD32 *code_size,
                                       WORD32 *code) {
  LOOPIDX idx;
  const FLOAT32 *coeff_table;
  FLOAT32 coeff_db;

  coeff_db = 20.0f * (FLOAT32)log10(downmix_coeff_var);

  if (is_lfe_channel == TRUE) {
    coeff_table = impd_drc_downmix_coeff_lfe;
  } else {
    coeff_table = impd_drc_downmix_coeff;
  }
  if (coeff_db >= coeff_table[14]) {
    idx = 0;
    while (coeff_db < coeff_table[idx]) {
      idx++;
    }
    if ((idx > 0) && (coeff_db > 0.5f * (coeff_table[idx - 1] + coeff_table[idx]))) {
      idx--;
    }
    *code = idx;
  } else {
    *code = 15;
  }

  *code_size = 4;
}

static VOID impd_drc_enc_peak(const FLOAT32 peak_level, WORD32 *code, WORD32 *code_size) {
  WORD32 bits;

  bits = ((WORD32)(0.5f + 32.0f * (20.0f - peak_level) + 10000.0f)) - 10000;
  bits = MIN(0x0FFF, bits);
  bits = MAX(0x1, bits);

  *code = bits;
  *code_size = 12;
}

static IA_ERRORCODE impd_drc_enc_method_value(const WORD32 method_definition,
                                              const FLOAT32 method_value, WORD32 *code_size,
                                              WORD32 *code) {
  WORD32 bits;
  switch (method_definition) {
    case METHOD_DEFINITION_UNKNOWN_OTHER:
    case METHOD_DEFINITION_PROGRAM_LOUDNESS:
    case METHOD_DEFINITION_ANCHOR_LOUDNESS:
    case METHOD_DEFINITION_MAX_OF_LOUDNESS_RANGE:
    case METHOD_DEFINITION_MOMENTARY_LOUDNESS_MAX:
    case METHOD_DEFINITION_SHORT_TERM_LOUDNESS_MAX:
      bits = ((WORD32)(0.5f + 4.0f * (method_value + 57.75f) + 10000.0f)) - 10000;
      bits = MIN(0x0FF, bits);
      bits = MAX(0x0, bits);
      *code_size = 8;
      break;
    case METHOD_DEFINITION_LOUDNESS_RANGE:
      if (method_value >= 121.0f) {
        bits = 255;
      } else if (method_value > 70.0f) {
        bits = ((WORD32)((method_value - 70.0f) + 0.5f)) + 204;
      } else if (method_value > 32.0f) {
        bits = ((WORD32)(2.0f * (method_value - 32.0f) + 0.5f)) + 128;
      } else if (method_value >= 0.0f) {
        bits = (WORD32)(4.0f * method_value + 0.5f);
      } else {
        bits = 0;
      }
      *code_size = 8;
      break;
    case METHOD_DEFINITION_MIXING_LEVEL:
      bits = (WORD32)(0.5f + method_value - 80.0f);
      bits = MIN(0x1F, bits);
      bits = MAX(0x0, bits);
      *code_size = 5;
      break;
    case METHOD_DEFINITION_ROOM_TYPE:
      bits = (WORD32)(0.5f + method_value);
      if (bits > 0x2) {
        return IA_EXHEAACE_CONFIG_FATAL_DRC_PARAM_OUT_OF_RANGE;
      }
      bits = MIN(0x2, bits);
      bits = MAX(0x0, bits);
      *code_size = 2;
      break;
    case METHOD_DEFINITION_SHORT_TERM_LOUDNESS:
      bits = ((WORD32)(0.5f + 2.0f * (method_value + 116.f) + 10000.0f)) - 10000;
      bits = MIN(0x0FF, bits);
      bits = MAX(0x0, bits);
      *code_size = 8;
      break;
    default: {
      return IA_EXHEAACE_CONFIG_FATAL_DRC_PARAM_OUT_OF_RANGE;
    }
  }
  *code = bits;

  return IA_NO_ERROR;
}

static VOID impd_drc_quantize_ducking_scaling(
    ia_drc_ducking_modifiers_struct *pstr_ducking_modifiers) {
  WORD32 mu;
  FLOAT32 delta;

  if (pstr_ducking_modifiers->ducking_scaling_present) {
    delta = pstr_ducking_modifiers->ducking_scaling - 1.0f;

    if (delta <= 0.0f) {
      mu = -1 + (WORD32)(0.5f - 8.0f * delta);
      if (mu != -1) {
        mu = MIN(7, mu);
        mu = MAX(0, mu);
        pstr_ducking_modifiers->ducking_scaling_quantized = 1.0f - 0.125f * (1.0f + mu);
      } else {
        pstr_ducking_modifiers->ducking_scaling_quantized = 1.0f;
        pstr_ducking_modifiers->ducking_scaling_present = FALSE;
      }
    } else {
      mu = -1 + (WORD32)(0.5f + 8.0f * delta);
      if (mu != -1) {
        mu = MIN(7, mu);
        mu = MAX(0, mu);
        pstr_ducking_modifiers->ducking_scaling_quantized = 1.0f + 0.125f * (1.0f + mu);
      } else {
        pstr_ducking_modifiers->ducking_scaling_quantized = 1.0f;
        pstr_ducking_modifiers->ducking_scaling_present = FALSE;
      }
    }
  } else {
    pstr_ducking_modifiers->ducking_scaling_quantized = 1.0f;
  }
}

static VOID impd_drc_enc_ducking_scaling(const FLOAT32 scaling, WORD32 *bits,
                                         FLOAT32 *scaling_quantized,
                                         WORD32 *remove_scaling_value) {
  WORD32 mu, sigma;
  FLOAT32 delta;

  delta = scaling - 1.0f;
  *remove_scaling_value = FALSE;
  if (delta <= 0.0f) {
    mu = -1 + (WORD32)(0.5f - 8.0f * delta);
    sigma = -1;
    *bits = 1 << 3;
  } else {
    mu = -1 + (WORD32)(0.5f + 8.0f * delta);
    sigma = 0;
    *bits = 0;
  }
  if (mu != -1) {
    mu = MIN(7, mu);
    mu = MAX(0, mu);
    *bits += mu;
    if (sigma == 0) {
      *scaling_quantized = 1.0f + 0.125f * (1.0f + mu);
    } else {
      *scaling_quantized = 1.0f - 0.125f * (1.0f + mu);
    }
  } else {
    *scaling_quantized = 1.0f;
    *remove_scaling_value = TRUE;
  }
}

static VOID impd_drc_enc_ducking_modifiers(
    ia_bit_buf_struct *it_bit_buf, ia_drc_ducking_modifiers_struct *pstr_ducking_modifiers,
    WORD32 *ptr_bit_cnt) {
  WORD32 bits;
  WORD32 remove_scaling_value;
  WORD32 bit_cnt_local = 0;

  if (pstr_ducking_modifiers->ducking_scaling_present == FALSE) {
    bit_cnt_local +=
        iusace_write_bits_buf(it_bit_buf, pstr_ducking_modifiers->ducking_scaling_present, 1);
  } else {
    impd_drc_enc_ducking_scaling(pstr_ducking_modifiers->ducking_scaling, &bits,
                                 &(pstr_ducking_modifiers->ducking_scaling_quantized),
                                 &remove_scaling_value);

    if (remove_scaling_value) {
      pstr_ducking_modifiers->ducking_scaling_present = FALSE;
    }

    bit_cnt_local +=
        iusace_write_bits_buf(it_bit_buf, pstr_ducking_modifiers->ducking_scaling_present, 1);

    if (pstr_ducking_modifiers->ducking_scaling_present) {
      bit_cnt_local += iusace_write_bits_buf(it_bit_buf, bits, 4);
    }
  }

  *ptr_bit_cnt += bit_cnt_local;
}

static VOID impd_drc_enc_gain_modifiers(ia_bit_buf_struct *it_bit_buf, const WORD32 version,
                                        const WORD32 band_count,
                                        ia_drc_gain_modifiers_struct *pstr_gain_modifiers,
                                        WORD32 *ptr_bit_cnt) {
  LOOPIDX idx;
  WORD32 tmp, sign;
  WORD32 bit_cnt_local = 0;

  if (version == 1) {
    for (idx = 0; idx < band_count; idx++) {
      bit_cnt_local += iusace_write_bits_buf(
          it_bit_buf, pstr_gain_modifiers->target_characteristic_left_present[idx], 1);
      if (pstr_gain_modifiers->target_characteristic_left_present[idx]) {
        bit_cnt_local += iusace_write_bits_buf(
            it_bit_buf, pstr_gain_modifiers->target_characteristic_left_index[idx], 4);
      }

      bit_cnt_local += iusace_write_bits_buf(
          it_bit_buf, pstr_gain_modifiers->target_characteristic_right_present[idx], 1);
      if (pstr_gain_modifiers->target_characteristic_right_present[idx]) {
        bit_cnt_local += iusace_write_bits_buf(
            it_bit_buf, pstr_gain_modifiers->target_characteristic_right_index[idx], 4);
      }

      bit_cnt_local +=
          iusace_write_bits_buf(it_bit_buf, pstr_gain_modifiers->gain_scaling_present[idx], 1);
      if (pstr_gain_modifiers->gain_scaling_present[idx]) {
        tmp = (WORD32)(0.5f + 8.0f * pstr_gain_modifiers->attenuation_scaling[idx]);
        bit_cnt_local += iusace_write_bits_buf(it_bit_buf, tmp, 4);

        tmp = (WORD32)(0.5f + 8.0f * pstr_gain_modifiers->amplification_scaling[idx]);
        bit_cnt_local += iusace_write_bits_buf(it_bit_buf, tmp, 4);
      }

      bit_cnt_local +=
          iusace_write_bits_buf(it_bit_buf, pstr_gain_modifiers->gain_offset_present[idx], 1);
      if (pstr_gain_modifiers->gain_offset_present[idx]) {
        if (pstr_gain_modifiers->gain_offset[idx] >= 0.0f) {
          tmp = (WORD32)(0.5f + MAX(0.0f, 4.0f * pstr_gain_modifiers->gain_offset[idx] - 1.0f));
          sign = 0;
        } else {
          tmp = (WORD32)(0.5f + MAX(0.0f, -4.0f * pstr_gain_modifiers->gain_offset[idx] - 1.0f));
          sign = 1;
        }
        bit_cnt_local += iusace_write_bits_buf(it_bit_buf, sign, 1);
        bit_cnt_local += iusace_write_bits_buf(it_bit_buf, tmp, 5);
      }
    }
    if (band_count == 1) {
      bit_cnt_local +=
          iusace_write_bits_buf(it_bit_buf, pstr_gain_modifiers->shape_filter_present, 1);
      if (pstr_gain_modifiers->shape_filter_present) {
        bit_cnt_local +=
            iusace_write_bits_buf(it_bit_buf, pstr_gain_modifiers->shape_filter_index, 4);
      }
    }
  } else if (version == 0) {
    bit_cnt_local +=
        iusace_write_bits_buf(it_bit_buf, pstr_gain_modifiers->gain_scaling_present[0], 1);

    if (pstr_gain_modifiers->gain_scaling_present[0]) {
      tmp = (WORD32)(0.5f + 8.0f * pstr_gain_modifiers->attenuation_scaling[0]);
      bit_cnt_local += iusace_write_bits_buf(it_bit_buf, tmp, 4);

      tmp = (WORD32)(0.5f + 8.0f * pstr_gain_modifiers->amplification_scaling[0]);
      bit_cnt_local += iusace_write_bits_buf(it_bit_buf, tmp, 4);
    }

    bit_cnt_local +=
        iusace_write_bits_buf(it_bit_buf, pstr_gain_modifiers->gain_offset_present[0], 1);
    if (pstr_gain_modifiers->gain_offset_present[0]) {
      if (pstr_gain_modifiers->gain_offset[0] >= 0.0f) {
        tmp = (WORD32)(0.5f + MAX(0.0f, 4.0f * pstr_gain_modifiers->gain_offset[0] - 1.0f));
        sign = 0;
      } else {
        tmp = (WORD32)(0.5f + MAX(0.0f, -4.0f * pstr_gain_modifiers->gain_offset[0] - 1.0f));
        sign = 1;
      }
      bit_cnt_local += iusace_write_bits_buf(it_bit_buf, sign, 1);
      bit_cnt_local += iusace_write_bits_buf(it_bit_buf, tmp, 5);
    }
  }

  *ptr_bit_cnt += bit_cnt_local;
}

static IA_ERRORCODE impd_drc_write_loudness_measure(
    ia_bit_buf_struct *it_bit_buf, ia_drc_loudness_measure_struct *pstr_loudness_measure,
    WORD32 *ptr_bit_cnt) {
  IA_ERRORCODE err_code = IA_NO_ERROR;
  WORD32 code, code_size;
  WORD32 bit_cnt_local = 0;

  bit_cnt_local += iusace_write_bits_buf(it_bit_buf, pstr_loudness_measure->method_definition, 4);

  err_code = impd_drc_enc_method_value(pstr_loudness_measure->method_definition,
                                       pstr_loudness_measure->method_value, &code_size, &code);
  if (err_code) return (err_code);

  bit_cnt_local += iusace_write_bits_buf(it_bit_buf, code, (UWORD8)code_size);
  bit_cnt_local +=
      iusace_write_bits_buf(it_bit_buf, pstr_loudness_measure->measurement_system, 4);
  bit_cnt_local += iusace_write_bits_buf(it_bit_buf, pstr_loudness_measure->reliability, 2);

  *ptr_bit_cnt += bit_cnt_local;

  return IA_NO_ERROR;
}

static IA_ERRORCODE impd_drc_write_loudness_info(ia_bit_buf_struct *it_bit_buf,
                                                 const WORD32 version,
                                                 ia_drc_loudness_info_struct *pstr_loudness_info,
                                                 WORD32 *ptr_bit_cnt) {
  IA_ERRORCODE err_code = IA_NO_ERROR;
  LOOPIDX idx;
  WORD32 code, code_size;
  WORD32 bit_cnt_local = 0;

  bit_cnt_local += iusace_write_bits_buf(it_bit_buf, pstr_loudness_info->drc_set_id, 6);
  if (version >= 1) {
    bit_cnt_local += iusace_write_bits_buf(it_bit_buf, pstr_loudness_info->eq_set_id, 6);
  }
  bit_cnt_local += iusace_write_bits_buf(it_bit_buf, pstr_loudness_info->downmix_id, 7);

  bit_cnt_local +=
      iusace_write_bits_buf(it_bit_buf, pstr_loudness_info->sample_peak_level_present, 1);
  if (pstr_loudness_info->sample_peak_level_present) {
    impd_drc_enc_peak(pstr_loudness_info->sample_peak_level, &code, &code_size);

    bit_cnt_local += iusace_write_bits_buf(it_bit_buf, code, (UWORD8)code_size);
  }

  bit_cnt_local +=
      iusace_write_bits_buf(it_bit_buf, pstr_loudness_info->true_peak_level_present, 1);
  if (pstr_loudness_info->true_peak_level_present) {
    impd_drc_enc_peak(pstr_loudness_info->true_peak_level, &code, &code_size);

    bit_cnt_local += iusace_write_bits_buf(it_bit_buf, code, (UWORD8)code_size);

    bit_cnt_local += iusace_write_bits_buf(
        it_bit_buf, pstr_loudness_info->true_peak_level_measurement_system, 4);

    bit_cnt_local +=
        iusace_write_bits_buf(it_bit_buf, pstr_loudness_info->true_peak_level_reliability, 2);
  }

  bit_cnt_local += iusace_write_bits_buf(it_bit_buf, pstr_loudness_info->measurement_count, 4);

  for (idx = 0; idx < pstr_loudness_info->measurement_count; idx++) {
    err_code = impd_drc_write_loudness_measure(
        it_bit_buf, &(pstr_loudness_info->str_loudness_measure[idx]), &bit_cnt_local);

    if (err_code) return (err_code);
  }

  *ptr_bit_cnt += bit_cnt_local;

  return IA_NO_ERROR;
}

static IA_ERRORCODE impd_drc_write_drc_instruct_uni_drc(
    ia_bit_buf_struct *it_bit_buf, const WORD32 version,
    ia_drc_uni_drc_config_struct *pstr_uni_drc_config, ia_drc_gain_enc_struct *pstr_gain_enc,
    ia_drc_instructions_uni_drc *pstr_drc_instructions_uni_drc, VOID *ptr_scratch,
    WORD32 *ptr_bit_cnt) {
  IA_ERRORCODE err_code = IA_NO_ERROR;
  LOOPIDX i, j, n;
  WORD32 g, k, tmp, tmp_2, match, channel_count;
  WORD32 bs_sequence_index, sequence_index_prev, repeat_sequence_count;
  WORD32 ducking_sequence, index;
  WORD32 repeat_parameters_count;
  WORD32 bit_cnt_local = 0;
  WORD32 band_count = 0;
  WORD32 *unique_index;
  FLOAT32 *unique_scaling;
  FLOAT32 ducking_scaling_quantized_prev, factor;
  ia_drc_ducking_modifiers_struct *pstr_ducking_modifiers;

  bit_cnt_local +=
      iusace_write_bits_buf(it_bit_buf, pstr_drc_instructions_uni_drc->drc_set_id, 6);
  if (version == 1) {
    err_code = impd_drc_get_drc_complexity_level(pstr_uni_drc_config, pstr_gain_enc,
                                                 pstr_drc_instructions_uni_drc, ptr_scratch);
    if (err_code & IA_FATAL_ERROR) {
      return (err_code);
    }

    bit_cnt_local += iusace_write_bits_buf(
        it_bit_buf, pstr_drc_instructions_uni_drc->drc_set_complexity_level, 4);
  }

  unique_index = (WORD32 *)ptr_scratch;
  unique_scaling =
      (FLOAT32 *)((UWORD8 *)ptr_scratch + (MAX_CHANNEL_COUNT) * sizeof(unique_index[0]));
  bit_cnt_local +=
      iusace_write_bits_buf(it_bit_buf, pstr_drc_instructions_uni_drc->drc_location, 4);

  if (version != 1) {
    pstr_drc_instructions_uni_drc->downmix_id_present = 1;
  } else {
    bit_cnt_local +=
        iusace_write_bits_buf(it_bit_buf, pstr_drc_instructions_uni_drc->downmix_id_present, 1);
  }

  if (pstr_drc_instructions_uni_drc->downmix_id_present != 1) {
    pstr_drc_instructions_uni_drc->downmix_id = 0;
  } else {
    bit_cnt_local +=
        iusace_write_bits_buf(it_bit_buf, pstr_drc_instructions_uni_drc->downmix_id, 7);
    if (version == 1) {
      bit_cnt_local += iusace_write_bits_buf(
          it_bit_buf, pstr_drc_instructions_uni_drc->drc_apply_to_downmix, 1);
    }
    bit_cnt_local += iusace_write_bits_buf(
        it_bit_buf, pstr_drc_instructions_uni_drc->additional_downmix_id_present, 1);

    if (pstr_drc_instructions_uni_drc->additional_downmix_id_present) {
      bit_cnt_local += iusace_write_bits_buf(
          it_bit_buf, pstr_drc_instructions_uni_drc->additional_downmix_id_count, 3);
      for (i = 0; i < pstr_drc_instructions_uni_drc->additional_downmix_id_count; i++) {
        bit_cnt_local += iusace_write_bits_buf(
            it_bit_buf, pstr_drc_instructions_uni_drc->additional_downmix_id[i], 7);
      }
    } else {
      pstr_drc_instructions_uni_drc->additional_downmix_id_count = 0;
    }
  }

  bit_cnt_local +=
      iusace_write_bits_buf(it_bit_buf, pstr_drc_instructions_uni_drc->drc_set_effect, 16);

  if ((pstr_drc_instructions_uni_drc->drc_set_effect &
       (EFFECT_BIT_DUCK_OTHER | EFFECT_BIT_DUCK_SELF)) == 0) {
    bit_cnt_local += iusace_write_bits_buf(
        it_bit_buf, pstr_drc_instructions_uni_drc->limiter_peak_target_present, 1);
    if (pstr_drc_instructions_uni_drc->limiter_peak_target_present) {
      tmp = (WORD32)(0.5f - 8.0f * pstr_drc_instructions_uni_drc->limiter_peak_target);
      tmp = MAX(0, tmp);
      tmp = MIN(0xFF, tmp);
      bit_cnt_local += iusace_write_bits_buf(it_bit_buf, tmp, 8);
    }
  }

  bit_cnt_local += iusace_write_bits_buf(
      it_bit_buf, pstr_drc_instructions_uni_drc->drc_set_target_loudness_present, 1);

  if (pstr_drc_instructions_uni_drc->drc_set_target_loudness_present == 1) {
    bit_cnt_local += iusace_write_bits_buf(
        it_bit_buf, pstr_drc_instructions_uni_drc->drc_set_target_loudness_value_upper + 63, 6);

    bit_cnt_local += iusace_write_bits_buf(
        it_bit_buf, pstr_drc_instructions_uni_drc->drc_set_target_loudness_value_lower_present,
        1);
    if (pstr_drc_instructions_uni_drc->drc_set_target_loudness_value_lower_present == 1) {
      bit_cnt_local += iusace_write_bits_buf(
          it_bit_buf, pstr_drc_instructions_uni_drc->drc_set_target_loudness_value_lower + 63, 6);
    }
  }

  bit_cnt_local += iusace_write_bits_buf(
      it_bit_buf, pstr_drc_instructions_uni_drc->depends_on_drc_set_present, 1);

  if (pstr_drc_instructions_uni_drc->depends_on_drc_set_present) {
    bit_cnt_local +=
        iusace_write_bits_buf(it_bit_buf, pstr_drc_instructions_uni_drc->depends_on_drc_set, 6);
  } else {
    bit_cnt_local +=
        iusace_write_bits_buf(it_bit_buf, pstr_drc_instructions_uni_drc->no_independent_use, 1);
  }

  if (version == 1) {
    bit_cnt_local +=
        iusace_write_bits_buf(it_bit_buf, pstr_drc_instructions_uni_drc->requires_eq, 1);
  }

  channel_count = pstr_uni_drc_config->str_channel_layout.base_ch_count;
  if (pstr_drc_instructions_uni_drc->drc_set_effect &
      (EFFECT_BIT_DUCK_OTHER | EFFECT_BIT_DUCK_SELF)) {
    i = 0;
    while (i < channel_count) {
      pstr_ducking_modifiers = pstr_drc_instructions_uni_drc->str_ducking_modifiers_for_channel;
      bs_sequence_index = pstr_drc_instructions_uni_drc->gain_set_index[i] + 1;

      bit_cnt_local += iusace_write_bits_buf(it_bit_buf, bs_sequence_index, 6);

      impd_drc_enc_ducking_modifiers(it_bit_buf, &(pstr_ducking_modifiers[i]), &bit_cnt_local);

      sequence_index_prev = pstr_drc_instructions_uni_drc->gain_set_index[i];
      ducking_scaling_quantized_prev = pstr_ducking_modifiers[i].ducking_scaling_quantized;
      i++;

      if (i < channel_count) {
        impd_drc_quantize_ducking_scaling(&(pstr_ducking_modifiers[i]));
      }

      repeat_parameters_count = 0;
      while ((i < channel_count) && (repeat_parameters_count <= 32) &&
             (sequence_index_prev == pstr_drc_instructions_uni_drc->gain_set_index[i]) &&
             (ducking_scaling_quantized_prev ==
              pstr_ducking_modifiers[i].ducking_scaling_quantized)) {
        repeat_parameters_count++;
        i++;
        if (i < channel_count) {
          impd_drc_quantize_ducking_scaling(&(pstr_ducking_modifiers[i]));
        }
      }
      if (repeat_parameters_count <= 0) {
        bit_cnt_local += iusace_write_bits_buf(it_bit_buf, 0, 1);
      } else {
        bit_cnt_local += iusace_write_bits_buf(it_bit_buf, 1, 1);
        bit_cnt_local += iusace_write_bits_buf(it_bit_buf, repeat_parameters_count - 1, 5);
      }
    }
    for (j = 0; j < MAX_CHANNEL_COUNT; j++) {
      unique_index[j] = -10;
      unique_scaling[j] = -10.0f;
    }

    ducking_sequence = -1;
    g = 0;

    if (pstr_drc_instructions_uni_drc->drc_set_effect & EFFECT_BIT_DUCK_SELF) {
      for (j = 0; j < channel_count; j++) {
        match = FALSE;
        index = pstr_drc_instructions_uni_drc->gain_set_index[j];
        factor =
            pstr_drc_instructions_uni_drc->str_ducking_modifiers_for_channel[j].ducking_scaling;
        for (n = 0; n < g; n++) {
          if ((index >= 0) && (unique_index[n] == index) && (unique_scaling[n] == factor)) {
            match = TRUE;
            break;
          }
        }
        if (match == FALSE) {
          if (index >= 0) {
            unique_index[g] = index;
            unique_scaling[g] = factor;
            g++;
          }
        }
      }
      pstr_drc_instructions_uni_drc->num_drc_channel_groups = g;
    } else if (pstr_drc_instructions_uni_drc->drc_set_effect & EFFECT_BIT_DUCK_OTHER) {
      for (j = 0; j < channel_count; j++) {
        match = FALSE;
        index = pstr_drc_instructions_uni_drc->gain_set_index[j];
        factor =
            pstr_drc_instructions_uni_drc->str_ducking_modifiers_for_channel[j].ducking_scaling;
        for (n = 0; n < g; n++) {
          if (((index >= 0) && (unique_index[n] == index)) ||
              ((index < 0) && (unique_scaling[n] == factor))) {
            match = TRUE;
            break;
          }
        }
        if (match == FALSE) {
          if (index < 0) {
            unique_index[g] = index;
            unique_scaling[g] = factor;
            g++;
          } else {
            if ((ducking_sequence > 0) && (ducking_sequence != index)) {
              return IA_EXHEAACE_CONFIG_FATAL_DRC_INVALID_CONFIG;
            }
            ducking_sequence = index;
          }
        }
      }
      pstr_drc_instructions_uni_drc->num_drc_channel_groups = g;
      if (ducking_sequence < 0) {
        return IA_EXHEAACE_CONFIG_FATAL_DRC_INVALID_CONFIG;
      }
    }

    for (g = 0; g < pstr_drc_instructions_uni_drc->num_drc_channel_groups; g++) {
      if (pstr_drc_instructions_uni_drc->drc_set_effect & EFFECT_BIT_DUCK_SELF) {
        pstr_drc_instructions_uni_drc->gain_set_index_for_channel_group[g] = unique_index[g];
      } else if (pstr_drc_instructions_uni_drc->drc_set_effect & EFFECT_BIT_DUCK_OTHER) {
        pstr_drc_instructions_uni_drc->gain_set_index_for_channel_group[g] = -1;
      }

      pstr_drc_instructions_uni_drc->str_ducking_modifiers_for_channel_group[g].ducking_scaling =
          unique_scaling[g];
      if (unique_scaling[g] == 1.0f) {
        pstr_drc_instructions_uni_drc->str_ducking_modifiers_for_channel_group[g]
            .ducking_scaling_present = FALSE;
      } else {
        pstr_drc_instructions_uni_drc->str_ducking_modifiers_for_channel_group[g]
            .ducking_scaling_present = TRUE;
      }
    }
  } else {
    if ((version == 0 || pstr_drc_instructions_uni_drc->drc_apply_to_downmix != 0) &&
        ((pstr_drc_instructions_uni_drc->downmix_id == 0x7F) ||
         (pstr_drc_instructions_uni_drc->additional_downmix_id_count != 0))) {
      channel_count = 1;
    } else if ((version == 0 || pstr_drc_instructions_uni_drc->drc_apply_to_downmix != 0) &&
               (pstr_drc_instructions_uni_drc->downmix_id != 0) &&
               (pstr_drc_instructions_uni_drc->downmix_id != 0x7F) &&
               (pstr_drc_instructions_uni_drc->additional_downmix_id_count == 0)) {
      for (i = 0; i < pstr_uni_drc_config->downmix_instructions_count; i++) {
        if (pstr_drc_instructions_uni_drc->downmix_id ==
            pstr_uni_drc_config->str_downmix_instructions[i].downmix_id) {
          break;
        }
      }
      if (i == pstr_uni_drc_config->downmix_instructions_count) {
        return IA_EXHEAACE_CONFIG_FATAL_DRC_INVALID_CONFIG;
      }
      channel_count = pstr_uni_drc_config->str_downmix_instructions[i].target_ch_count;
    }

    i = 0;
    while (i < channel_count) {
      bs_sequence_index = pstr_drc_instructions_uni_drc->gain_set_index[i] + 1;
      bit_cnt_local += iusace_write_bits_buf(it_bit_buf, bs_sequence_index, 6);
      sequence_index_prev = pstr_drc_instructions_uni_drc->gain_set_index[i];
      i++;

      repeat_sequence_count = 0;
      while ((i < channel_count) &&
             (sequence_index_prev == pstr_drc_instructions_uni_drc->gain_set_index[i]) &&
             (repeat_sequence_count <= 32)) {
        repeat_sequence_count++;
        i++;
      }
      if (repeat_sequence_count <= 0) {
        bit_cnt_local += iusace_write_bits_buf(it_bit_buf, 0, 1);
      } else {
        bit_cnt_local += iusace_write_bits_buf(it_bit_buf, 1, 1);
        bit_cnt_local += iusace_write_bits_buf(it_bit_buf, repeat_sequence_count - 1, 5);
      }
    }
    for (i = 0; i < MAX_CHANNEL_COUNT; i++) {
      unique_index[i] = -1;
    }

    k = 0;
    for (i = 0; i < channel_count; i++) {
      tmp_2 = pstr_drc_instructions_uni_drc->gain_set_index[i];
      if (tmp_2 >= 0) {
        match = FALSE;
        for (n = 0; n < k; n++) {
          if (unique_index[n] == tmp_2) {
            match = TRUE;
          }
        }
        if (match == FALSE) {
          unique_index[k] = tmp_2;
          k++;
        }
      }
    }
    pstr_drc_instructions_uni_drc->num_drc_channel_groups = k;
    for (i = 0; i < pstr_drc_instructions_uni_drc->num_drc_channel_groups; i++) {
      band_count = 0;
      pstr_drc_instructions_uni_drc->gain_set_index_for_channel_group[i] = unique_index[i];

      if (pstr_uni_drc_config->str_uni_drc_config_ext.drc_coefficients_uni_drc_v1_count > 0) {
        band_count =
            pstr_uni_drc_config->str_uni_drc_config_ext.str_drc_coefficients_uni_drc_v1[0]
                .str_gain_set_params[pstr_drc_instructions_uni_drc
                                         ->gain_set_index_for_channel_group[i]]
                .band_count;
      } else if (pstr_uni_drc_config->drc_coefficients_uni_drc_count > 0) {
        band_count = pstr_uni_drc_config->str_drc_coefficients_uni_drc[0]
                         .str_gain_set_params[pstr_drc_instructions_uni_drc
                                                  ->gain_set_index_for_channel_group[i]]
                         .band_count;
      }

      impd_drc_enc_gain_modifiers(it_bit_buf, version, band_count,
                                  &(pstr_drc_instructions_uni_drc->str_gain_modifiers[i]),
                                  &bit_cnt_local);
    }
  }

  *ptr_bit_cnt += bit_cnt_local;
  return err_code;
}

#ifdef LOUDNESS_LEVELING_SUPPORT
static UWORD32 get_num_ducking_only_drc_sets(
    ia_drc_instructions_uni_drc const *pstr_drc_instructions_uni_drc,
    UWORD32 drc_intructions_uni_drc_count) {
  UWORD32 num_ducking_only_drc_sets = 0;
  for (UWORD16 i = 0; i < drc_intructions_uni_drc_count; i++) {
    if (pstr_drc_instructions_uni_drc[i].ducking_only_set_present) {
      num_ducking_only_drc_sets++;
    }
  }

  return num_ducking_only_drc_sets;
}

static WORD32 write_loudness_leveling_extension(ia_bit_buf_struct *it_bit_buf,
                                                ia_drc_uni_drc_config_struct *pstr_uni_drc_config,
                                                ia_drc_gain_enc_struct *pstr_gain_enc,
                                                VOID *ptr_scratch, WORD32 *bit_cnt) {
  IA_ERRORCODE err_code = IA_NO_ERROR;
  WORD32 bit_cnt_local = 0;
  WORD32 drc_instructions_uni_drc_count_v1 =
      pstr_uni_drc_config->str_uni_drc_config_ext.drc_instructions_uni_drc_v1_count;
  WORD32 drc_instructions_uni_drc_count = pstr_uni_drc_config->drc_instructions_uni_drc_count;
  WORD32 version = 0;
  // V0 instructions
  for (WORD16 i = 0; i < drc_instructions_uni_drc_count; i++) {
    ia_drc_instructions_uni_drc *pstr_drc_instruction =
        &pstr_uni_drc_config->str_drc_instructions_uni_drc[i];
    if (pstr_drc_instruction->drc_set_effect & EFFECT_BIT_DUCK_SELF) {
      if (i > 0 && pstr_uni_drc_config->str_drc_instructions_uni_drc[i - 1].leveling_present &&
          pstr_uni_drc_config->str_drc_instructions_uni_drc[i - 1].ducking_only_set_present) {
        err_code = impd_drc_write_drc_instruct_uni_drc(it_bit_buf, version, pstr_uni_drc_config,
                                                       pstr_gain_enc, pstr_drc_instruction,
                                                       ptr_scratch, &bit_cnt_local);
        if (err_code & IA_FATAL_ERROR) {
          return (err_code);
        }
      } else {
        bit_cnt_local +=
            iusace_write_bits_buf(it_bit_buf, pstr_drc_instruction->leveling_present, 1);
        if (pstr_drc_instruction->leveling_present) {
          bit_cnt_local += iusace_write_bits_buf(
              it_bit_buf, pstr_drc_instruction->ducking_only_set_present, 1);
        }
      }
    }
  }

  // V1 instructions
  version = 1;
  for (WORD16 i = 0; i < drc_instructions_uni_drc_count_v1; i++) {
    ia_drc_instructions_uni_drc *pstr_drc_instruction =
        &pstr_uni_drc_config->str_uni_drc_config_ext.str_drc_instructions_uni_drc_v1[i];
    if (pstr_drc_instruction->drc_set_effect & EFFECT_BIT_DUCK_SELF) {
      if (i > 0 &&
          pstr_uni_drc_config->str_uni_drc_config_ext.str_drc_instructions_uni_drc_v1[i - 1]
              .leveling_present &&
          pstr_uni_drc_config->str_uni_drc_config_ext.str_drc_instructions_uni_drc_v1[i - 1]
              .ducking_only_set_present) {
        err_code = impd_drc_write_drc_instruct_uni_drc(it_bit_buf, version, pstr_uni_drc_config,
                                                       pstr_gain_enc, pstr_drc_instruction,
                                                       ptr_scratch, &bit_cnt_local);
        if (err_code & IA_FATAL_ERROR) {
          return (err_code);
        }
      } else {
        bit_cnt_local +=
            iusace_write_bits_buf(it_bit_buf, pstr_drc_instruction->leveling_present, 1);
        if (pstr_drc_instruction->leveling_present) {
          bit_cnt_local += iusace_write_bits_buf(
              it_bit_buf, pstr_drc_instruction->ducking_only_set_present, 1);
        }
      }
    }
  }
  *bit_cnt += bit_cnt_local;
  return IA_NO_ERROR;
}
#endif

static VOID impd_drc_write_gain_params(ia_bit_buf_struct *it_bit_buf, const WORD32 version,
                                       const WORD32 band_count, const WORD32 drc_band_type,
                                       ia_drc_gain_params_struct *pstr_gain_params,
                                       WORD32 *ptr_bit_cnt) {
  LOOPIDX idx;
  WORD32 bit_cnt_local = 0;

  if (version != 1) {
    for (idx = 0; idx < band_count; idx++) {
      bit_cnt_local +=
          iusace_write_bits_buf(it_bit_buf, pstr_gain_params[idx].drc_characteristic, 7);
    }
  } else {
    WORD32 index_present;
    WORD32 gain_sequence_index_last = -1;
    for (idx = 0; idx < band_count; idx++) {
      if (pstr_gain_params[idx].gain_sequence_index == gain_sequence_index_last + 1) {
        index_present = 0;
      } else {
        index_present = 1;
      }
      bit_cnt_local += iusace_write_bits_buf(it_bit_buf, index_present, 1);

      if (index_present == 1) {
        bit_cnt_local +=
            iusace_write_bits_buf(it_bit_buf, pstr_gain_params[idx].gain_sequence_index, 6);
        gain_sequence_index_last = pstr_gain_params[idx].gain_sequence_index;
      }

      bit_cnt_local +=
          iusace_write_bits_buf(it_bit_buf, pstr_gain_params[idx].drc_characteristic_present, 1);
      if (pstr_gain_params[idx].drc_characteristic_present) {
        bit_cnt_local += iusace_write_bits_buf(
            it_bit_buf, pstr_gain_params[idx].drc_characteristic_format_is_cicp, 1);
        if (pstr_gain_params[idx].drc_characteristic_format_is_cicp == 1) {
          bit_cnt_local +=
              iusace_write_bits_buf(it_bit_buf, pstr_gain_params[idx].drc_characteristic, 7);
        } else {
          bit_cnt_local += iusace_write_bits_buf(
              it_bit_buf, pstr_gain_params[idx].drc_characteristic_left_index, 4);

          bit_cnt_local += iusace_write_bits_buf(
              it_bit_buf, pstr_gain_params[idx].drc_characteristic_right_index, 4);
        }
      }
    }
  }

  for (idx = 1; idx < band_count; idx++) {
    if (drc_band_type) {
      bit_cnt_local +=
          iusace_write_bits_buf(it_bit_buf, pstr_gain_params[idx].crossover_freq_index, 4);
    } else {
      bit_cnt_local +=
          iusace_write_bits_buf(it_bit_buf, pstr_gain_params[idx].start_sub_band_index, 10);
    }
  }

  *ptr_bit_cnt += bit_cnt_local;
}

static VOID impd_drc_write_gain_set_params(ia_bit_buf_struct *it_bit_buf, const WORD32 version,
                                           ia_drc_gain_set_params_struct *pstr_gain_set_params,
                                           WORD32 *ptr_bit_cnt) {
  WORD32 bit_cnt_local = 0;

  bit_cnt_local +=
      iusace_write_bits_buf(it_bit_buf, pstr_gain_set_params->gain_coding_profile, 2);

  bit_cnt_local +=
      iusace_write_bits_buf(it_bit_buf, pstr_gain_set_params->gain_interpolation_type, 1);

  bit_cnt_local += iusace_write_bits_buf(it_bit_buf, pstr_gain_set_params->full_frame, 1);

  bit_cnt_local += iusace_write_bits_buf(it_bit_buf, pstr_gain_set_params->time_alignment, 1);

  bit_cnt_local +=
      iusace_write_bits_buf(it_bit_buf, pstr_gain_set_params->time_delta_min_present, 1);

  if (pstr_gain_set_params->time_delta_min_present) {
    bit_cnt_local += iusace_write_bits_buf(it_bit_buf, pstr_gain_set_params->delta_tmin - 1, 11);
  }
  if (pstr_gain_set_params->gain_coding_profile != GAIN_CODING_PROFILE_CONSTANT) {
    bit_cnt_local += iusace_write_bits_buf(it_bit_buf, pstr_gain_set_params->band_count, 4);
    if (pstr_gain_set_params->band_count > 1) {
      bit_cnt_local += iusace_write_bits_buf(it_bit_buf, pstr_gain_set_params->drc_band_type, 1);
    }

    impd_drc_write_gain_params(it_bit_buf, version, pstr_gain_set_params->band_count,
                               pstr_gain_set_params->drc_band_type,
                               pstr_gain_set_params->gain_params, &bit_cnt_local);
  }

  *ptr_bit_cnt += bit_cnt_local;
}

static VOID impd_drc_write_split_drc_characteristic(
    ia_bit_buf_struct *it_bit_buf, const WORD32 side,
    ia_drc_split_drc_characteristic_struct *pstr_split_characteristic, WORD32 *ptr_bit_cnt) {
  LOOPIDX idx;
  WORD32 bs_node_gain, bs_node_level_delta;
  WORD32 bit_cnt_local = 0;
  FLOAT32 bs_node_level_previous;

  bit_cnt_local +=
      iusace_write_bits_buf(it_bit_buf, pstr_split_characteristic->characteristic_format, 1);

  if (pstr_split_characteristic->characteristic_format != 0) {
    bs_node_level_previous = DRC_INPUT_LOUDNESS_TARGET;

    bit_cnt_local += iusace_write_bits_buf(
        it_bit_buf, pstr_split_characteristic->characteristic_node_count - 1, 2);

    for (idx = 1; idx <= pstr_split_characteristic->characteristic_node_count; idx++) {
      bs_node_level_delta = (WORD32)(floor(fabs(pstr_split_characteristic->node_level[idx] -
                                                bs_node_level_previous) +
                0.5f) -
          1);

      if (bs_node_level_delta < 0) {
        bs_node_level_delta = 0;
      }
      if (bs_node_level_delta > 31) {
        bs_node_level_delta = 31;
      }
      if (side == RIGHT_SIDE) {
        bs_node_level_previous = bs_node_level_previous + (bs_node_level_delta + 1);
      } else {
        bs_node_level_previous = bs_node_level_previous - (bs_node_level_delta + 1);
      }

      bit_cnt_local += iusace_write_bits_buf(it_bit_buf, bs_node_level_delta, 5);

      bs_node_gain =
          (WORD32)floor((pstr_split_characteristic->node_gain[idx] + 64.0f) * 2.0f + 0.5f);

      if (bs_node_gain < 0) {
        bs_node_gain = 0;
      }
      if (bs_node_gain > 255) {
        bs_node_gain = 255;
      }

      bit_cnt_local += iusace_write_bits_buf(it_bit_buf, bs_node_gain, 8);
    }
  } else {
    bit_cnt_local += iusace_write_bits_buf(it_bit_buf, pstr_split_characteristic->bs_gain, 6);

    bit_cnt_local += iusace_write_bits_buf(it_bit_buf, pstr_split_characteristic->bs_io_ratio, 4);

    bit_cnt_local += iusace_write_bits_buf(it_bit_buf, pstr_split_characteristic->bs_exp, 4);

    bit_cnt_local += iusace_write_bits_buf(it_bit_buf, pstr_split_characteristic->flip_sign, 1);
  }

  *ptr_bit_cnt += bit_cnt_local;
}

static VOID impd_drc_write_shape_filter_block_params(
    ia_bit_buf_struct *it_bit_buf,
    ia_drc_shape_filter_block_params_struct *pstr_shape_filter_block_params,
    WORD32 *ptr_bit_cnt) {
  WORD32 bit_cnt_local = 0;

  bit_cnt_local +=
      iusace_write_bits_buf(it_bit_buf, pstr_shape_filter_block_params->lf_cut_filter_present, 1);
  if (pstr_shape_filter_block_params->lf_cut_filter_present == 1) {
    bit_cnt_local += iusace_write_bits_buf(
        it_bit_buf, pstr_shape_filter_block_params->str_lf_cut_params.corner_freq_index, 3);

    bit_cnt_local += iusace_write_bits_buf(
        it_bit_buf, pstr_shape_filter_block_params->str_lf_cut_params.filter_strength_index, 2);
  }

  bit_cnt_local += iusace_write_bits_buf(
      it_bit_buf, pstr_shape_filter_block_params->lf_boost_filter_present, 1);
  if (pstr_shape_filter_block_params->lf_boost_filter_present == 1) {
    bit_cnt_local += iusace_write_bits_buf(
        it_bit_buf, pstr_shape_filter_block_params->str_lf_boost_params.corner_freq_index, 3);

    bit_cnt_local += iusace_write_bits_buf(
        it_bit_buf, pstr_shape_filter_block_params->str_lf_boost_params.filter_strength_index, 2);
  }

  bit_cnt_local +=
      iusace_write_bits_buf(it_bit_buf, pstr_shape_filter_block_params->hf_cut_filter_present, 1);
  if (pstr_shape_filter_block_params->hf_cut_filter_present == 1) {
    bit_cnt_local += iusace_write_bits_buf(
        it_bit_buf, pstr_shape_filter_block_params->str_hf_cut_params.corner_freq_index, 3);

    bit_cnt_local += iusace_write_bits_buf(
        it_bit_buf, pstr_shape_filter_block_params->str_hf_cut_params.filter_strength_index, 2);
  }

  bit_cnt_local += iusace_write_bits_buf(
      it_bit_buf, pstr_shape_filter_block_params->hf_boost_filter_present, 1);
  if (pstr_shape_filter_block_params->hf_boost_filter_present == 1) {
    bit_cnt_local += iusace_write_bits_buf(
        it_bit_buf, pstr_shape_filter_block_params->str_hf_boost_params.corner_freq_index, 3);

    bit_cnt_local += iusace_write_bits_buf(
        it_bit_buf, pstr_shape_filter_block_params->str_hf_boost_params.filter_strength_index, 2);
  }

  *ptr_bit_cnt += bit_cnt_local;
}

static VOID impd_drc_write_drc_coeff_uni_drc(
    ia_bit_buf_struct *it_bit_buf, const WORD32 version,
    ia_drc_coefficients_uni_drc_struct *pstr_drc_coefficients_uni_drc, WORD32 *ptr_bit_cnt) {
  LOOPIDX idx;
  WORD32 bit_cnt_local = 0;

  bit_cnt_local +=
      iusace_write_bits_buf(it_bit_buf, pstr_drc_coefficients_uni_drc->drc_location, 4);

  bit_cnt_local +=
      iusace_write_bits_buf(it_bit_buf, pstr_drc_coefficients_uni_drc->drc_frame_size_present, 1);

  if (pstr_drc_coefficients_uni_drc->drc_frame_size_present) {
    bit_cnt_local += iusace_write_bits_buf(
        it_bit_buf, (pstr_drc_coefficients_uni_drc->drc_frame_size - 1), 15);
  }

  if (version != 1) {
    bit_cnt_local +=
        iusace_write_bits_buf(it_bit_buf, pstr_drc_coefficients_uni_drc->gain_set_count, 6);

    for (idx = 0; idx < pstr_drc_coefficients_uni_drc->gain_set_count; idx++) {
      impd_drc_write_gain_set_params(it_bit_buf, version,
                                     &(pstr_drc_coefficients_uni_drc->str_gain_set_params[idx]),
                                     &bit_cnt_local);
    }
  } else {
    bit_cnt_local += iusace_write_bits_buf(
        it_bit_buf, pstr_drc_coefficients_uni_drc->drc_characteristic_left_present, 1);
    if (pstr_drc_coefficients_uni_drc->drc_characteristic_left_present) {
      bit_cnt_local += iusace_write_bits_buf(
          it_bit_buf, pstr_drc_coefficients_uni_drc->characteristic_left_count, 4);

      for (idx = 1; idx <= pstr_drc_coefficients_uni_drc->characteristic_left_count; idx++) {
        impd_drc_write_split_drc_characteristic(
            it_bit_buf, LEFT_SIDE,
            &pstr_drc_coefficients_uni_drc->str_split_characteristic_left[idx], &bit_cnt_local);
      }
    }

    bit_cnt_local += iusace_write_bits_buf(
        it_bit_buf, pstr_drc_coefficients_uni_drc->drc_characteristic_right_present, 1);
    if (pstr_drc_coefficients_uni_drc->drc_characteristic_right_present) {
      bit_cnt_local += iusace_write_bits_buf(
          it_bit_buf, pstr_drc_coefficients_uni_drc->characteristic_right_count, 4);
      for (idx = 1; idx <= pstr_drc_coefficients_uni_drc->characteristic_right_count; idx++) {
        impd_drc_write_split_drc_characteristic(
            it_bit_buf, RIGHT_SIDE,
            &pstr_drc_coefficients_uni_drc->str_split_characteristic_right[idx], &bit_cnt_local);
      }
    }

    bit_cnt_local += iusace_write_bits_buf(
        it_bit_buf, pstr_drc_coefficients_uni_drc->shape_filters_present, 1);
    if (pstr_drc_coefficients_uni_drc->shape_filters_present) {
      bit_cnt_local +=
          iusace_write_bits_buf(it_bit_buf, pstr_drc_coefficients_uni_drc->shape_filter_count, 4);
      for (idx = 1; idx <= pstr_drc_coefficients_uni_drc->shape_filter_count; idx++) {
        impd_drc_write_shape_filter_block_params(
            it_bit_buf, &pstr_drc_coefficients_uni_drc->str_shape_filter_block_params[idx],
            &bit_cnt_local);
      }
    }

    bit_cnt_local +=
        iusace_write_bits_buf(it_bit_buf, pstr_drc_coefficients_uni_drc->gain_sequence_count, 6);

    bit_cnt_local +=
        iusace_write_bits_buf(it_bit_buf, pstr_drc_coefficients_uni_drc->gain_set_count, 6);

    for (idx = 0; idx < pstr_drc_coefficients_uni_drc->gain_set_count; idx++) {
      impd_drc_write_gain_set_params(it_bit_buf, version,
                                     &(pstr_drc_coefficients_uni_drc->str_gain_set_params[idx]),
                                     &bit_cnt_local);
    }
  }

  *ptr_bit_cnt += bit_cnt_local;
}

static VOID impd_drc_write_downmix_instructions(
    ia_bit_buf_struct *it_bit_buf, const WORD32 version, ia_drc_gain_enc_struct *pstr_gain_enc,
    ia_drc_downmix_instructions_struct *pstr_downmix_instructions, WORD32 *ptr_bit_cnt) {
  LOOPIDX idx;
  WORD32 code, code_size;
  WORD32 bit_cnt_local = 0;

  bit_cnt_local += iusace_write_bits_buf(it_bit_buf, pstr_downmix_instructions->downmix_id, 7);

  bit_cnt_local +=
      iusace_write_bits_buf(it_bit_buf, pstr_downmix_instructions->target_ch_count, 7);

  bit_cnt_local += iusace_write_bits_buf(it_bit_buf, pstr_downmix_instructions->target_layout, 8);

  bit_cnt_local += iusace_write_bits_buf(
      it_bit_buf, pstr_downmix_instructions->downmix_coefficients_present, 1);

  if (pstr_downmix_instructions->downmix_coefficients_present) {
    if (version != 1) {
      WORD32 is_lfe_channel = FALSE;
      for (idx = 0;
           idx < (pstr_downmix_instructions->target_ch_count * pstr_gain_enc->base_ch_count);
           idx++) {
        impd_drc_enc_downmix_coeff(pstr_downmix_instructions->downmix_coeff[idx], is_lfe_channel,
                                   &code_size, &code);
        bit_cnt_local += iusace_write_bits_buf(it_bit_buf, code, (UWORD8)code_size);
      }
    } else {
      impd_drc_dec_write_downmix_coeff_v1(
          it_bit_buf, pstr_downmix_instructions->downmix_coeff, pstr_gain_enc->base_ch_count,
          pstr_downmix_instructions->target_ch_count, &bit_cnt_local);
    }
  }

  *ptr_bit_cnt += bit_cnt_local;
}

static VOID impd_drc_enc_channel_weight(const FLOAT32 channel_weight_lin, WORD32 *code_size,
                                        WORD32 *code) {
  LOOPIDX idx;
  FLOAT32 channel_weight_db;
  const FLOAT32 *channel_weight_table;

  channel_weight_table = impd_drc_channel_weight;
  channel_weight_db = 20.0f * (FLOAT32)log10(channel_weight_lin);

  if (channel_weight_db >= channel_weight_table[14]) {
    idx = 0;
    while (channel_weight_db < channel_weight_table[idx]) {
      idx++;
    }
    if ((idx > 0) && (channel_weight_db >
                      0.5f * (channel_weight_table[idx - 1] + channel_weight_table[idx]))) {
      idx--;
    }
    *code = idx;
  } else {
    *code = 15;
  }

  *code_size = 4;
}

static IA_ERRORCODE impd_drc_write_parametric_drc_gain_set_params(
    ia_bit_buf_struct *it_bit_buf, ia_drc_uni_drc_config_struct *pstr_uni_drc_config,
    ia_drc_parametric_drc_gain_set_params_struct *pstr_parametric_drc_gain_set_params,
    WORD32 *ptr_bit_cnt) {
  LOOPIDX idx;
  WORD32 code_size = 0, code = 0;
  WORD32 bit_cnt_local = 0;

  bit_cnt_local += iusace_write_bits_buf(
      it_bit_buf, pstr_parametric_drc_gain_set_params->parametric_drc_id, 4);

  bit_cnt_local += iusace_write_bits_buf(
      it_bit_buf, pstr_parametric_drc_gain_set_params->side_chain_config_type, 3);

  if (pstr_parametric_drc_gain_set_params->side_chain_config_type == 1) {
    bit_cnt_local +=
        iusace_write_bits_buf(it_bit_buf, pstr_parametric_drc_gain_set_params->downmix_id, 7);

    bit_cnt_local += iusace_write_bits_buf(
        it_bit_buf, pstr_parametric_drc_gain_set_params->level_estim_channel_weight_format, 1);

    if (pstr_parametric_drc_gain_set_params->downmix_id == 0x7F) {
      pstr_parametric_drc_gain_set_params->channel_count_drom_downmix_id = 1;
    } else if (pstr_parametric_drc_gain_set_params->downmix_id == 0x0) {
      pstr_parametric_drc_gain_set_params->channel_count_drom_downmix_id =
          pstr_uni_drc_config->str_channel_layout.base_ch_count;
    } else {
      for (idx = 0; idx < pstr_uni_drc_config->downmix_instructions_count; idx++) {
        if (pstr_parametric_drc_gain_set_params->downmix_id ==
            pstr_uni_drc_config->str_downmix_instructions[idx].downmix_id) {
          break;
        }
      }
      if (idx == pstr_uni_drc_config->downmix_instructions_count) {
        return IA_EXHEAACE_CONFIG_FATAL_DRC_INVALID_CONFIG;
      }
      pstr_parametric_drc_gain_set_params->channel_count_drom_downmix_id =
          pstr_uni_drc_config->str_downmix_instructions[idx].target_ch_count;
    }

    for (idx = 0; idx < pstr_parametric_drc_gain_set_params->channel_count_drom_downmix_id;
         idx++) {
      if (pstr_parametric_drc_gain_set_params->level_estim_channel_weight_format != 0) {
        impd_drc_enc_channel_weight(
            pstr_parametric_drc_gain_set_params->level_estim_channel_weight[idx], &code_size,
            &code);
        bit_cnt_local += iusace_write_bits_buf(it_bit_buf, code, (UWORD8)code_size);
      } else {
        if (pstr_parametric_drc_gain_set_params->level_estim_channel_weight[idx] == 0) {
          code = 0;
        } else {
          code = 1;
        }
        bit_cnt_local += iusace_write_bits_buf(it_bit_buf, code, 1);
      }
    }
  }

  bit_cnt_local += iusace_write_bits_buf(
      it_bit_buf, pstr_parametric_drc_gain_set_params->drc_input_loudness_present, 1);
  if (pstr_parametric_drc_gain_set_params->drc_input_loudness_present) {
    code = ((WORD32)(0.5f +
                     4.0f * (pstr_parametric_drc_gain_set_params->drc_input_loudness + 57.75f) +
                     10000.0f)) -
           10000;
    code = MIN(0x0FF, code);
    code = MAX(0x0, code);
    bit_cnt_local += iusace_write_bits_buf(it_bit_buf, code, 8);
  }

  *ptr_bit_cnt += bit_cnt_local;

  return IA_NO_ERROR;
}

static VOID impd_drc_write_parametric_drc_type_feed_forward(
    ia_bit_buf_struct *it_bit_buf, WORD32 drc_frame_size_parametric_drc,
    ia_drc_parametric_drc_type_feed_forward_struct *pstr_parametric_drc_type_feed_forward,
    WORD32 *ptr_bit_cnt) {
  LOOPIDX idx;
  WORD32 code = 0;
  WORD32 bit_cnt_local = 0;

  bit_cnt_local += iusace_write_bits_buf(
      it_bit_buf, pstr_parametric_drc_type_feed_forward->level_estim_k_weighting_type, 2);

  bit_cnt_local += iusace_write_bits_buf(
      it_bit_buf, pstr_parametric_drc_type_feed_forward->level_estim_integration_time_present, 1);

  if (pstr_parametric_drc_type_feed_forward->level_estim_integration_time_present) {
    code =
        (WORD32)(((FLOAT32)pstr_parametric_drc_type_feed_forward->level_estim_integration_time /
                      drc_frame_size_parametric_drc +
                  0.5f) -
                 1);

    bit_cnt_local += iusace_write_bits_buf(it_bit_buf, code, 6);
  }

  bit_cnt_local += iusace_write_bits_buf(
      it_bit_buf, pstr_parametric_drc_type_feed_forward->drc_curve_definition_type, 1);
  if (pstr_parametric_drc_type_feed_forward->drc_curve_definition_type != 0) {
    bit_cnt_local += iusace_write_bits_buf(
        it_bit_buf, pstr_parametric_drc_type_feed_forward->node_count - 2, 3);

    for (idx = 0; idx < pstr_parametric_drc_type_feed_forward->node_count; idx++) {
      if (idx == 0) {
        code = -11 - pstr_parametric_drc_type_feed_forward->node_level[0];
        bit_cnt_local += iusace_write_bits_buf(it_bit_buf, code, 6);
      } else {
        code = pstr_parametric_drc_type_feed_forward->node_level[idx] -
               pstr_parametric_drc_type_feed_forward->node_level[idx - 1] - 1;
        bit_cnt_local += iusace_write_bits_buf(it_bit_buf, code, 5);
      }
      code = pstr_parametric_drc_type_feed_forward->node_gain[idx] + 39;
      bit_cnt_local += iusace_write_bits_buf(it_bit_buf, code, 6);
    }
  } else {
    bit_cnt_local += iusace_write_bits_buf(
        it_bit_buf, pstr_parametric_drc_type_feed_forward->drc_characteristic, 7);
  }

  bit_cnt_local += iusace_write_bits_buf(
      it_bit_buf, pstr_parametric_drc_type_feed_forward->drc_gain_smooth_parameters_present, 1);
  if (pstr_parametric_drc_type_feed_forward->drc_gain_smooth_parameters_present) {
    code = (WORD32)(pstr_parametric_drc_type_feed_forward->gain_smooth_attack_time_slow * 0.2);
    bit_cnt_local += iusace_write_bits_buf(it_bit_buf, code, 8);

    code = (WORD32)(pstr_parametric_drc_type_feed_forward->gain_smooth_release_time_slow * 0.025);
    bit_cnt_local += iusace_write_bits_buf(it_bit_buf, code, 8);

    bit_cnt_local += iusace_write_bits_buf(
        it_bit_buf, pstr_parametric_drc_type_feed_forward->gain_smooth_time_fast_present, 1);
    if (pstr_parametric_drc_type_feed_forward->gain_smooth_time_fast_present) {
      code = (WORD32)(pstr_parametric_drc_type_feed_forward->gain_smooth_attack_time_fast * 0.2);
      bit_cnt_local += iusace_write_bits_buf(it_bit_buf, code, 8);

      code =
          (WORD32)(pstr_parametric_drc_type_feed_forward->gain_smooth_release_time_fast * 0.05);
      bit_cnt_local += iusace_write_bits_buf(it_bit_buf, code, 8);

      bit_cnt_local += iusace_write_bits_buf(
          it_bit_buf, pstr_parametric_drc_type_feed_forward->gain_smooth_threshold_present, 1);
      if (pstr_parametric_drc_type_feed_forward->gain_smooth_threshold_present) {
        if (pstr_parametric_drc_type_feed_forward->gain_smooth_attack_threshold <= 30) {
          code = pstr_parametric_drc_type_feed_forward->gain_smooth_attack_threshold;
        } else {
          code = 31;
        }
        bit_cnt_local += iusace_write_bits_buf(it_bit_buf, code, 5);

        if (pstr_parametric_drc_type_feed_forward->gain_smooth_release_threshold <= 30) {
          code = pstr_parametric_drc_type_feed_forward->gain_smooth_release_threshold;
        } else {
          code = 31;
        }
        bit_cnt_local += iusace_write_bits_buf(it_bit_buf, code, 5);
      }
    }

    bit_cnt_local += iusace_write_bits_buf(
        it_bit_buf, pstr_parametric_drc_type_feed_forward->gain_smooth_hold_off_count_present, 1);
    if (pstr_parametric_drc_type_feed_forward->gain_smooth_hold_off_count_present) {
      bit_cnt_local += iusace_write_bits_buf(
          it_bit_buf, pstr_parametric_drc_type_feed_forward->gain_smooth_hold_off, 7);
    }
  }

  *ptr_bit_cnt += bit_cnt_local;
}

static VOID impd_drc_write_parametric_drc_type_lim(
    ia_bit_buf_struct *it_bit_buf,
    ia_drc_parametric_drc_type_lim_struct *pstr_parametric_drc_type_lim, WORD32 *ptr_bit_cnt) {
  WORD32 temp = 0;
  WORD32 bit_cnt_local = 0;

  bit_cnt_local += iusace_write_bits_buf(
      it_bit_buf, pstr_parametric_drc_type_lim->parametric_lim_threshold_present, 1);
  if (pstr_parametric_drc_type_lim->parametric_lim_threshold_present) {
    temp = (WORD32)(0.5f - 8.0f * pstr_parametric_drc_type_lim->parametric_lim_threshold);
    temp = MAX(0, temp);
    temp = MIN(0xFF, temp);
    bit_cnt_local += iusace_write_bits_buf(it_bit_buf, temp, 8);
  }

  bit_cnt_local += iusace_write_bits_buf(
      it_bit_buf, pstr_parametric_drc_type_lim->parametric_lim_release_present, 1);
  if (pstr_parametric_drc_type_lim->parametric_lim_release_present) {
    temp = (WORD32)(pstr_parametric_drc_type_lim->parametric_lim_release * 0.1);
    bit_cnt_local += iusace_write_bits_buf(it_bit_buf, temp, 8);
  }

  *ptr_bit_cnt += bit_cnt_local;
}

static IA_ERRORCODE impd_drc_write_parametric_drc_instructions(
    ia_bit_buf_struct *it_bit_buf, WORD32 drc_frame_size_parametric_drc,
    ia_drc_parametric_drc_instructions_struct *pstr_parametric_drc_instructions,
    WORD32 *ptr_bit_cnt) {
  WORD32 bit_size = 0, len_size_bits = 0, bit_size_len = 0;
  WORD32 bit_cnt_local = 0;

  bit_cnt_local +=
      iusace_write_bits_buf(it_bit_buf, pstr_parametric_drc_instructions->parametric_drc_id, 4);

  bit_cnt_local += iusace_write_bits_buf(
      it_bit_buf, pstr_parametric_drc_instructions->parametric_drc_look_ahead_present, 1);

  if (pstr_parametric_drc_instructions->parametric_drc_look_ahead_present) {
    bit_cnt_local += iusace_write_bits_buf(
        it_bit_buf, pstr_parametric_drc_instructions->parametric_drc_look_ahead, 7);
  }

  bit_cnt_local += iusace_write_bits_buf(
      it_bit_buf, pstr_parametric_drc_instructions->parametric_drc_preset_id_present, 1);
  if (!(pstr_parametric_drc_instructions->parametric_drc_preset_id_present)) {
    bit_cnt_local += iusace_write_bits_buf(
        it_bit_buf, pstr_parametric_drc_instructions->parametric_drc_type, 3);

    if (pstr_parametric_drc_instructions->parametric_drc_type == PARAM_DRC_TYPE_LIM) {
      impd_drc_write_parametric_drc_type_lim(
          it_bit_buf, &(pstr_parametric_drc_instructions->str_parametric_drc_type_lim),
          &bit_cnt_local);
    } else if (pstr_parametric_drc_instructions->parametric_drc_type == PARAM_DRC_TYPE_FF) {
      impd_drc_write_parametric_drc_type_feed_forward(
          it_bit_buf, drc_frame_size_parametric_drc,
          &(pstr_parametric_drc_instructions->str_parametric_drc_type_feed_forward),
          &bit_cnt_local);
    } else {
      bit_size = pstr_parametric_drc_instructions->len_bit_size - 1;
      len_size_bits = (WORD32)(log((FLOAT32)bit_size) / log(2.f)) + 1;
      bit_size_len = len_size_bits - 4;
      bit_cnt_local += iusace_write_bits_buf(it_bit_buf, bit_size_len, 4);
      bit_cnt_local += iusace_write_bits_buf(it_bit_buf, bit_size, (UWORD8)bit_size);
      switch (pstr_parametric_drc_instructions->parametric_drc_type) {
        default:
          return IA_EXHEAACE_CONFIG_FATAL_DRC_INVALID_CONFIG;
      }
    }
  } else {
    bit_cnt_local += iusace_write_bits_buf(
        it_bit_buf, pstr_parametric_drc_instructions->parametric_drc_preset_id, 7);
  }

  *ptr_bit_cnt += bit_cnt_local;

  return IA_NO_ERROR;
}

static IA_ERRORCODE impd_drc_write_drc_coeff_parametric_drc(
    ia_bit_buf_struct *it_bit_buf, ia_drc_uni_drc_config_struct *pstr_uni_drc_config,
    ia_drc_coeff_parametric_drc_struct *pstr_drc_coeff_parametric_drc, WORD32 *ptr_bit_cnt) {
  IA_ERRORCODE err_code = IA_NO_ERROR;
  LOOPIDX idx;
  WORD32 bits = 0, mu = 0, nu = 0;
  WORD32 bit_cnt_local = 0;
  FLOAT32 exp = 0.f;

  bit_cnt_local +=
      iusace_write_bits_buf(it_bit_buf, pstr_drc_coeff_parametric_drc->drc_location, 4);

  exp = (FLOAT32)(log(pstr_drc_coeff_parametric_drc->parametric_drc_frame_size) / log(2));
  if (exp == (FLOAT32)((WORD32)exp)) {
    pstr_drc_coeff_parametric_drc->parametric_drc_frame_size_format = 0;
  } else {
    pstr_drc_coeff_parametric_drc->parametric_drc_frame_size_format = 1;
  }

  bit_cnt_local += iusace_write_bits_buf(
      it_bit_buf, pstr_drc_coeff_parametric_drc->parametric_drc_frame_size_format, 1);
  if (!(pstr_drc_coeff_parametric_drc->parametric_drc_frame_size_format)) {
    bits = (WORD32)exp;
    bit_cnt_local += iusace_write_bits_buf(it_bit_buf, bits, 4);
  } else {
    bit_cnt_local += iusace_write_bits_buf(
        it_bit_buf, (pstr_drc_coeff_parametric_drc->parametric_drc_frame_size - 1), 15);
  }

  bit_cnt_local += iusace_write_bits_buf(
      it_bit_buf, pstr_drc_coeff_parametric_drc->parametric_drc_delay_max_present, 1);
  if (pstr_drc_coeff_parametric_drc->parametric_drc_delay_max_present == 1) {
    for (nu = 0; nu < 8; nu++) {
      mu = pstr_drc_coeff_parametric_drc->parametric_drc_delay_max / (16 << nu);
      if (mu * (16 << nu) < pstr_drc_coeff_parametric_drc->parametric_drc_delay_max) {
        mu++;
      }
      if (mu < 32) {
        break;
      }
    }
    if (nu == 8) {
      mu = 31;
      nu = 7;
    }
    bit_cnt_local += iusace_write_bits_buf(it_bit_buf, mu, 5);
    bit_cnt_local += iusace_write_bits_buf(it_bit_buf, nu, 3);
  }

  bit_cnt_local +=
      iusace_write_bits_buf(it_bit_buf, pstr_drc_coeff_parametric_drc->reset_parametric_drc, 1);

  bit_cnt_local += iusace_write_bits_buf(
      it_bit_buf, pstr_drc_coeff_parametric_drc->parametric_drc_gain_set_count, 6);
  for (idx = 0; idx < pstr_drc_coeff_parametric_drc->parametric_drc_gain_set_count; idx++) {
    err_code = impd_drc_write_parametric_drc_gain_set_params(
        it_bit_buf, pstr_uni_drc_config,
        &(pstr_drc_coeff_parametric_drc->parametric_drc_gain_set_params[idx]), &bit_cnt_local);

    if (err_code & IA_FATAL_ERROR) {
      return err_code;
    }
  }

  *ptr_bit_cnt += bit_cnt_local;

  return err_code;
}

static VOID impd_drc_write_loud_eq_instructions(
    ia_bit_buf_struct *it_bit_buf, ia_drc_loud_eq_instructions_struct *pstr_loud_eq_instructions,
    WORD32 *ptr_bit_cnt) {
  LOOPIDX idx;
  WORD32 bit_cnt_local = 0;
  WORD32 bs_loud_eq_offset;
  WORD32 bs_loud_eq_scaling;

  bit_cnt_local +=
      iusace_write_bits_buf(it_bit_buf, pstr_loud_eq_instructions->loud_eq_set_id, 4);

  bit_cnt_local += iusace_write_bits_buf(it_bit_buf, pstr_loud_eq_instructions->drc_location, 4);

  bit_cnt_local +=
      iusace_write_bits_buf(it_bit_buf, pstr_loud_eq_instructions->downmix_id_present, 1);
  if (pstr_loud_eq_instructions->downmix_id_present) {
    bit_cnt_local += iusace_write_bits_buf(it_bit_buf, pstr_loud_eq_instructions->downmix_id, 7);

    bit_cnt_local += iusace_write_bits_buf(
        it_bit_buf, pstr_loud_eq_instructions->additional_downmix_id_present, 1);
    if (!(pstr_loud_eq_instructions->additional_downmix_id_present)) {
      pstr_loud_eq_instructions->additional_downmix_id_count = 0;
    } else {
      bit_cnt_local += iusace_write_bits_buf(
          it_bit_buf, pstr_loud_eq_instructions->additional_downmix_id_count, 7);
      for (idx = 0; idx < pstr_loud_eq_instructions->additional_downmix_id_count; idx++) {
        bit_cnt_local += iusace_write_bits_buf(
            it_bit_buf, pstr_loud_eq_instructions->additional_downmix_id[idx], 7);
      }
    }
  }

  bit_cnt_local +=
      iusace_write_bits_buf(it_bit_buf, pstr_loud_eq_instructions->drc_set_id_present, 1);
  if (pstr_loud_eq_instructions->drc_set_id_present) {
    bit_cnt_local += iusace_write_bits_buf(it_bit_buf, pstr_loud_eq_instructions->drc_set_id, 6);

    bit_cnt_local += iusace_write_bits_buf(
        it_bit_buf, pstr_loud_eq_instructions->additional_drc_set_id_present, 1);
    if (!(pstr_loud_eq_instructions->additional_drc_set_id_present)) {
      pstr_loud_eq_instructions->additional_drc_set_id_count = 0;
    } else {
      bit_cnt_local += iusace_write_bits_buf(
          it_bit_buf, pstr_loud_eq_instructions->additional_drc_set_id_count, 6);
      for (idx = 0; idx < pstr_loud_eq_instructions->additional_drc_set_id_count; idx++) {
        bit_cnt_local += iusace_write_bits_buf(
            it_bit_buf, pstr_loud_eq_instructions->additional_drc_set_id[idx], 6);
      }
    }
  }

  bit_cnt_local +=
      iusace_write_bits_buf(it_bit_buf, pstr_loud_eq_instructions->eq_set_id_present, 1);
  if (pstr_loud_eq_instructions->eq_set_id_present) {
    bit_cnt_local += iusace_write_bits_buf(it_bit_buf, pstr_loud_eq_instructions->eq_set_id, 6);

    bit_cnt_local += iusace_write_bits_buf(
        it_bit_buf, pstr_loud_eq_instructions->additional_eq_set_id_present, 1);
    if (!(pstr_loud_eq_instructions->additional_eq_set_id_present)) {
      pstr_loud_eq_instructions->additional_eq_set_id_count = 0;
    } else {
      bit_cnt_local += iusace_write_bits_buf(
          it_bit_buf, pstr_loud_eq_instructions->additional_eq_set_id_count, 6);
      for (idx = 0; idx < pstr_loud_eq_instructions->additional_eq_set_id_count; idx++) {
        bit_cnt_local += iusace_write_bits_buf(
            it_bit_buf, pstr_loud_eq_instructions->additional_eq_set_id[idx], 6);
      }
    }
  }

  bit_cnt_local +=
      iusace_write_bits_buf(it_bit_buf, pstr_loud_eq_instructions->loudness_after_drc, 1);

  bit_cnt_local +=
      iusace_write_bits_buf(it_bit_buf, pstr_loud_eq_instructions->loudness_after_eq, 1);

  bit_cnt_local += iusace_write_bits_buf(
      it_bit_buf, pstr_loud_eq_instructions->loud_eq_gain_sequence_count, 6);
  for (idx = 0; idx < pstr_loud_eq_instructions->loud_eq_gain_sequence_count; idx++) {
    bit_cnt_local +=
        iusace_write_bits_buf(it_bit_buf, pstr_loud_eq_instructions->gain_sequence_index[idx], 6);

    bit_cnt_local += iusace_write_bits_buf(
        it_bit_buf, pstr_loud_eq_instructions->drc_characteristic_format_is_cicp[idx], 1);
    if (pstr_loud_eq_instructions->drc_characteristic_format_is_cicp[idx] == 1) {
      bit_cnt_local += iusace_write_bits_buf(
          it_bit_buf, pstr_loud_eq_instructions->drc_characteristic[idx], 7);
    } else {
      bit_cnt_local += iusace_write_bits_buf(
          it_bit_buf, pstr_loud_eq_instructions->drc_characteristic_left_index[idx], 4);

      bit_cnt_local += iusace_write_bits_buf(
          it_bit_buf, pstr_loud_eq_instructions->drc_characteristic_right_index[idx], 4);
    }

    bit_cnt_local += iusace_write_bits_buf(
        it_bit_buf, pstr_loud_eq_instructions->frequency_range_index[idx], 6);
    bs_loud_eq_scaling = (WORD32)floor(
        0.5f - 2.0f * INV_LOG10_2 * log10(pstr_loud_eq_instructions->loud_eq_scaling[idx]));
    if (bs_loud_eq_scaling < 0) {
      bs_loud_eq_scaling = 0;
    } else if (bs_loud_eq_scaling > 7) {
      bs_loud_eq_scaling = 7;
    }

    bit_cnt_local += iusace_write_bits_buf(it_bit_buf, bs_loud_eq_scaling, 3);
    bs_loud_eq_offset =
        (WORD32)floor(0.5f + pstr_loud_eq_instructions->loud_eq_offset[idx] / 1.5f + 16.0f);
    if (bs_loud_eq_offset < 0) {
      bs_loud_eq_offset = 0;
    } else if (bs_loud_eq_offset > 31) {
      bs_loud_eq_offset = 31;
    }

    bit_cnt_local += iusace_write_bits_buf(it_bit_buf, bs_loud_eq_offset, 5);
  }

  *ptr_bit_cnt += bit_cnt_local;
}

static VOID impd_drc_write_filter_element(ia_bit_buf_struct *it_bit_buf,
                                          ia_drc_filter_element_struct *pstr_filter_element,
                                          WORD32 *ptr_bit_cnt) {
  WORD32 bs_filter_element_gain;
  WORD32 bit_cnt_local = 0;

  bit_cnt_local +=
      iusace_write_bits_buf(it_bit_buf, pstr_filter_element->filter_element_index, 6);

  bit_cnt_local +=
      iusace_write_bits_buf(it_bit_buf, pstr_filter_element->filter_element_gain_present, 1);
  if (pstr_filter_element->filter_element_gain_present) {
    bs_filter_element_gain =
        (WORD32)floor(0.5f + 8.0f * (pstr_filter_element->filter_element_gain + 96.0f));
    bs_filter_element_gain = MAX(0, bs_filter_element_gain);
    bs_filter_element_gain = MIN(1023, bs_filter_element_gain);
    bit_cnt_local += iusace_write_bits_buf(it_bit_buf, bs_filter_element_gain, 10);
  }

  *ptr_bit_cnt += bit_cnt_local;
}

static VOID impd_drc_write_filter_block(ia_bit_buf_struct *it_bit_buf,
                                        ia_drc_filter_block_struct *pstr_filter_block,
                                        WORD32 *ptr_bit_cnt) {
  LOOPIDX idx;
  WORD32 bit_cnt_local = 0;

  bit_cnt_local += iusace_write_bits_buf(it_bit_buf, pstr_filter_block->filter_element_count, 6);
  for (idx = 0; idx < pstr_filter_block->filter_element_count; idx++) {
    impd_drc_write_filter_element(it_bit_buf, &(pstr_filter_block->filter_element[idx]),
                                  &bit_cnt_local);
  }

  *ptr_bit_cnt += bit_cnt_local;
}

static IA_ERRORCODE impd_drc_encode_radius(FLOAT32 radius, WORD32 *code) {
  LOOPIDX idx;
  FLOAT32 rho;

  if (radius < 0.0f) {
    return IA_EXHEAACE_CONFIG_FATAL_DRC_INVALID_CONFIG;
  }
  rho = 1.0f - radius;
  if ((rho < 0.0f) || (rho > 1.0f)) {
    if (rho < 0.0f) {
      rho = 0.0f;
    }
    if (rho > 1.0f) {
      rho = 1.0f;
    }
  }
  if (rho > impd_drc_zero_pole_radius_table[127]) {
    rho = impd_drc_zero_pole_radius_table[127];
  }
  idx = 0;
  while (rho > impd_drc_zero_pole_radius_table[idx]) {
    idx++;
  }
  if (idx == 0) {
    return IA_EXHEAACE_CONFIG_FATAL_DRC_INVALID_CONFIG;
  }
  if (rho <
      0.5f * (impd_drc_zero_pole_radius_table[idx - 1] + impd_drc_zero_pole_radius_table[idx])) {
    idx--;
  }
  *code = idx;

  return IA_NO_ERROR;
}

static LOOPIDX impd_drc_encode_angle(FLOAT32 angle) {
  LOOPIDX idx;

  if ((angle < 0.0f) || (angle > 1.0f)) {
    if (angle < 0.0f) {
      angle = 0.0f;
    }
    if (angle > 1.0f) {
      angle = 1.0f;
    }
  }
  idx = 0;
  while (angle > impd_drc_zero_pole_angle_table[idx]) {
    idx++;
  }
  if (idx == 0) {
    return idx;
  }
  if (angle <
      0.5f * (impd_drc_zero_pole_angle_table[idx - 1] + impd_drc_zero_pole_angle_table[idx])) {
    idx--;
  }

  return (idx);
}

static IA_ERRORCODE impd_drc_write_unique_td_filter_element(
    ia_bit_buf_struct *it_bit_buf,
    ia_drc_unique_td_filter_element_struct *pstr_unique_td_filter_element, WORD32 *ptr_bit_cnt) {
  IA_ERRORCODE err_code = IA_NO_ERROR;
  LOOPIDX idx;
  WORD32 sign, code;
  WORD32 bs_real_zero_radius_one_count;
  WORD32 bs_fir_coefficient;
  WORD32 bit_cnt_local = 0;

  bit_cnt_local +=
      iusace_write_bits_buf(it_bit_buf, pstr_unique_td_filter_element->eq_filter_format, 1);
  if (pstr_unique_td_filter_element->eq_filter_format != 0) {
    bit_cnt_local +=
        iusace_write_bits_buf(it_bit_buf, pstr_unique_td_filter_element->fir_filter_order, 7);

    bit_cnt_local +=
        iusace_write_bits_buf(it_bit_buf, pstr_unique_td_filter_element->fir_symmetry, 1);

    for (idx = 0; idx < pstr_unique_td_filter_element->fir_filter_order / 2 + 1; idx++) {
      if (pstr_unique_td_filter_element->fir_coefficient[idx] >= 0.0f) {
        sign = 0;
      } else {
        sign = 1;
      }
      bit_cnt_local += iusace_write_bits_buf(it_bit_buf, sign, 1);

      bs_fir_coefficient =
          (WORD32)floor(0.5f - log10(fabs(pstr_unique_td_filter_element->fir_coefficient[idx])) /
                                   (0.05f * 0.0625f));

      if (bs_fir_coefficient > 1023) {
        bs_fir_coefficient = 1023;
      }
      if (bs_fir_coefficient < 0) {
        bs_fir_coefficient = 0;
      }
      bit_cnt_local += iusace_write_bits_buf(it_bit_buf, bs_fir_coefficient, 10);
    }
  } else {
    bs_real_zero_radius_one_count = pstr_unique_td_filter_element->real_zero_radius_one_count / 2;
    if ((pstr_unique_td_filter_element->real_zero_radius_one_count ==
         2 * bs_real_zero_radius_one_count) &&
        (bs_real_zero_radius_one_count < 8)) {
      bit_cnt_local += iusace_write_bits_buf(it_bit_buf, bs_real_zero_radius_one_count, 3);
    } else {
      return IA_EXHEAACE_CONFIG_FATAL_DRC_INVALID_CONFIG;
    }

    bit_cnt_local +=
        iusace_write_bits_buf(it_bit_buf, pstr_unique_td_filter_element->real_zero_count, 6);

    bit_cnt_local +=
        iusace_write_bits_buf(it_bit_buf, pstr_unique_td_filter_element->generic_zero_count, 6);

    bit_cnt_local +=
        iusace_write_bits_buf(it_bit_buf, pstr_unique_td_filter_element->real_pole_count, 4);

    bit_cnt_local +=
        iusace_write_bits_buf(it_bit_buf, pstr_unique_td_filter_element->complex_pole_count, 4);

    for (idx = 0; idx < pstr_unique_td_filter_element->real_zero_radius_one_count; idx++) {
      bit_cnt_local += iusace_write_bits_buf(
          it_bit_buf, (UWORD32)pstr_unique_td_filter_element->zero_sign[idx], 1);
    }

    for (idx = 0; idx < pstr_unique_td_filter_element->real_zero_count; idx++) {
      err_code = impd_drc_encode_radius(
          (FLOAT32)fabs(pstr_unique_td_filter_element->real_zero_radius[idx]), &code);

      if (err_code & IA_FATAL_ERROR) {
        return err_code;
      }
      bit_cnt_local += iusace_write_bits_buf(it_bit_buf, code, 7);

      if (pstr_unique_td_filter_element->real_zero_radius[idx] >= 0.0f) {
        sign = 0;
      } else {
        sign = 1;
      }
      bit_cnt_local += iusace_write_bits_buf(it_bit_buf, sign, 1);
    }

    for (idx = 0; idx < pstr_unique_td_filter_element->generic_zero_count; idx++) {
      err_code =
          impd_drc_encode_radius(pstr_unique_td_filter_element->generic_zero_radius[idx], &code);
      if (err_code & IA_FATAL_ERROR) {
        return err_code;
      }
      bit_cnt_local += iusace_write_bits_buf(it_bit_buf, code, 7);

      bit_cnt_local += iusace_write_bits_buf(
          it_bit_buf,
          impd_drc_encode_angle(pstr_unique_td_filter_element->generic_zero_angle[idx]), 7);
    }

    for (idx = 0; idx < pstr_unique_td_filter_element->real_pole_count; idx++) {
      err_code = impd_drc_encode_radius(
          (FLOAT32)fabs(pstr_unique_td_filter_element->real_pole_radius[idx]), &code);
      if (err_code & IA_FATAL_ERROR) {
        return err_code;
      }
      bit_cnt_local += iusace_write_bits_buf(it_bit_buf, code, 7);

      if (pstr_unique_td_filter_element->real_pole_radius[idx] >= 0.0f) {
        sign = 0;
      } else {
        sign = 1;
      }
      bit_cnt_local += iusace_write_bits_buf(it_bit_buf, sign, 1);
    }

    for (idx = 0; idx < pstr_unique_td_filter_element->complex_pole_count; idx++) {
      err_code =
          impd_drc_encode_radius(pstr_unique_td_filter_element->complex_pole_radius[idx], &code);
      if (err_code & IA_FATAL_ERROR) {
        return err_code;
      }
      bit_cnt_local += iusace_write_bits_buf(it_bit_buf, code, 7);

      bit_cnt_local += iusace_write_bits_buf(
          it_bit_buf,
          impd_drc_encode_angle(pstr_unique_td_filter_element->complex_pole_angle[idx]), 7);
    }
  }

  *ptr_bit_cnt += bit_cnt_local;

  return err_code;
}

static VOID impd_drc_encode_eq_slope(FLOAT32 eq_slope, WORD32 *size, WORD32 *code) {
  LOOPIDX idx;

  if (fabs(eq_slope) >= 0.5f) {
    *size = 5;
    if (eq_slope > 32.0f) {
      *code = 15;
    } else if (eq_slope <= -32.0f) {
      *code = 0;
    } else {
      idx = 1;
      while (eq_slope > impd_drc_eq_slope_table[idx]) {
        idx++;
      }
      if (eq_slope < 0.5f * (impd_drc_eq_slope_table[idx - 1] + impd_drc_eq_slope_table[idx])) {
        idx--;
      }
      *code = idx;
    }
  } else {
    *size = 1;
    *code = 1;
  }
}

static VOID impd_drc_encode_eq_gain_initial(FLOAT32 eq_gain_initial, WORD32 *prefix_code,
                                            WORD32 *size, WORD32 *code) {
  if ((eq_gain_initial > -8.5f) && (eq_gain_initial < 7.75f)) {
    *size = 5;
    *prefix_code = 0;
    *code = (WORD32)floor(0.5f + 2.0f * (MAX(-8.0f, eq_gain_initial) + 8.0f));
  } else if (eq_gain_initial < 0.0f) {
    if (eq_gain_initial > -17.0f) {
      *size = 4;
      *prefix_code = 1;
      *code = (WORD32)floor(0.5f + MAX(-16.0f, eq_gain_initial) + 16.0f);
    } else if (eq_gain_initial > -34.0f) {
      *size = 4;
      *prefix_code = 2;
      *code = (WORD32)floor(0.5f + 0.5f * (MAX(-32.0f, eq_gain_initial) + 32.0f));
    } else {
      *size = 3;
      *prefix_code = 3;
      *code = (WORD32)floor(0.5f + 0.25f * (MAX(-64.0f, eq_gain_initial) + 64.0f));
    }
  } else {
    if (eq_gain_initial >= 15.5f) {
      *size = 4;
      *prefix_code = 2;
      *code = (WORD32)floor(0.5f + 0.5f * MIN(30.0f, eq_gain_initial));
    } else {
      *size = 4;
      *prefix_code = 1;
      *code = (WORD32)floor(0.5f + eq_gain_initial);
    }
  }
}

static VOID impd_drc_encode_eq_gain_delta(FLOAT32 eq_gain_delta, WORD32 *code) {
  LOOPIDX idx;

  if (eq_gain_delta >= 32.0f) {
    *code = 31;
  } else if (eq_gain_delta <= -22.0f) {
    *code = 0;
  } else {
    idx = 1;
    while (eq_gain_delta > impd_drc_eq_gain_delta_table[idx]) {
      idx++;
    }
    if (eq_gain_delta <
        0.5f * (impd_drc_eq_gain_delta_table[idx - 1] + impd_drc_eq_gain_delta_table[idx])) {
      idx--;
    }
    *code = idx;
  }
}

static VOID impd_drc_write_eq_subband_gain_spline(
    ia_bit_buf_struct *it_bit_buf,
    ia_drc_eq_subband_gain_spline_struct *pstr_eq_subband_gain_spline, WORD32 *ptr_bit_cnt) {
  LOOPIDX idx;
  WORD32 size, code, prefix_code;
  WORD32 bit_cnt_local = 0;
  WORD32 bs_eq_node_count = pstr_eq_subband_gain_spline->n_eq_nodes - 2;

  bit_cnt_local += iusace_write_bits_buf(it_bit_buf, bs_eq_node_count, 5);

  for (idx = 0; idx < pstr_eq_subband_gain_spline->n_eq_nodes; idx++) {
    impd_drc_encode_eq_slope(pstr_eq_subband_gain_spline->eq_slope[idx], &size, &code);

    bit_cnt_local += iusace_write_bits_buf(it_bit_buf, code, (UWORD8)size);
  }

  for (idx = 1; idx < pstr_eq_subband_gain_spline->n_eq_nodes; idx++) {
    code = MIN(15, pstr_eq_subband_gain_spline->eq_freq_delta[idx] - 1);
    bit_cnt_local += iusace_write_bits_buf(it_bit_buf, code, 4);
  }

  impd_drc_encode_eq_gain_initial(pstr_eq_subband_gain_spline->eq_gain_initial, &prefix_code,
                                  &size, &code);

  bit_cnt_local += iusace_write_bits_buf(it_bit_buf, prefix_code, 2);
  bit_cnt_local += iusace_write_bits_buf(it_bit_buf, code, (UWORD8)size);

  for (idx = 1; idx < pstr_eq_subband_gain_spline->n_eq_nodes; idx++) {
    impd_drc_encode_eq_gain_delta(pstr_eq_subband_gain_spline->eq_gain_delta[idx], &code);

    bit_cnt_local += iusace_write_bits_buf(it_bit_buf, code, 5);
  }

  *ptr_bit_cnt += bit_cnt_local;
}

static VOID impd_drc_write_eq_subband_gain_vector(
    ia_bit_buf_struct *it_bit_buf, WORD32 eq_subband_gain_count,
    ia_drc_eq_subband_gain_vector_struct *pstr_eq_subband_gain_vector, WORD32 *ptr_bit_cnt) {
  LOOPIDX idx = 0;
  WORD32 sign;
  WORD32 bs_eq_subband_gain;
  WORD32 bit_cnt_local = 0;

  for (idx = 0; idx < eq_subband_gain_count; idx++) {
    bs_eq_subband_gain =
        (WORD32)floor(0.5f + fabs(pstr_eq_subband_gain_vector->eq_subband_gain[idx] * 8.0f));

    if (pstr_eq_subband_gain_vector->eq_subband_gain[idx] >= 0.0f) {
      sign = 0;
    } else {
      sign = 1;
    }

    bit_cnt_local += iusace_write_bits_buf(it_bit_buf, sign, 1);
    bit_cnt_local += iusace_write_bits_buf(it_bit_buf, bs_eq_subband_gain, 8);
  }

  *ptr_bit_cnt += bit_cnt_local;
}

static IA_ERRORCODE impd_drc_write_eq_coefficients(
    ia_bit_buf_struct *it_bit_buf, ia_drc_eq_coefficients_struct *pstr_eq_coefficients,
    WORD32 *ptr_bit_cnt) {
  IA_ERRORCODE err_code = IA_NO_ERROR;
  LOOPIDX idx;
  WORD32 bs_eq_gain_count;
  WORD32 mu = 0, nu = 0;
  WORD32 bit_cnt_local = 0;

  bit_cnt_local +=
      iusace_write_bits_buf(it_bit_buf, pstr_eq_coefficients->eq_delay_max_present, 1);
  if (pstr_eq_coefficients->eq_delay_max_present == 1) {
    for (nu = 0; nu < 8; nu++) {
      mu = pstr_eq_coefficients->eq_delay_max / (16 << nu);
      if (mu * (16 << nu) < pstr_eq_coefficients->eq_delay_max) {
        mu++;
      }
      if (mu < 32) {
        break;
      }
    }
    if (nu == 8) {
      mu = 31;
      nu = 7;
    }
    bit_cnt_local += iusace_write_bits_buf(it_bit_buf, mu, 5);
    bit_cnt_local += iusace_write_bits_buf(it_bit_buf, nu, 3);
  }

  bit_cnt_local +=
      iusace_write_bits_buf(it_bit_buf, pstr_eq_coefficients->unique_filter_block_count, 6);
  for (idx = 0; idx < pstr_eq_coefficients->unique_filter_block_count; idx++) {
    impd_drc_write_filter_block(it_bit_buf, &(pstr_eq_coefficients->str_filter_block[idx]),
                                &bit_cnt_local);
  }

  bit_cnt_local +=
      iusace_write_bits_buf(it_bit_buf, pstr_eq_coefficients->unique_td_filter_element_count, 6);
  for (idx = 0; idx < pstr_eq_coefficients->unique_td_filter_element_count; idx++) {
    err_code = impd_drc_write_unique_td_filter_element(
        it_bit_buf, &(pstr_eq_coefficients->str_unique_td_filter_element[idx]), &bit_cnt_local);
    if (err_code & IA_FATAL_ERROR) {
      return err_code;
    }
  }

  bit_cnt_local +=
      iusace_write_bits_buf(it_bit_buf, pstr_eq_coefficients->unique_eq_subband_gains_count, 6);
  if (pstr_eq_coefficients->unique_eq_subband_gains_count > 0) {
    bit_cnt_local += iusace_write_bits_buf(
        it_bit_buf, pstr_eq_coefficients->eq_subband_gain_representation, 1);

    bit_cnt_local +=
        iusace_write_bits_buf(it_bit_buf, pstr_eq_coefficients->eq_subband_gain_format, 4);

    switch (pstr_eq_coefficients->eq_subband_gain_format) {
      case GAINFORMAT_QMFHYBRID135:
        pstr_eq_coefficients->eq_subband_gain_count = 135;
        break;
      case GAINFORMAT_QMF128:
        pstr_eq_coefficients->eq_subband_gain_count = 128;
        break;
      case GAINFORMAT_QMFHYBRID71:
        pstr_eq_coefficients->eq_subband_gain_count = 71;
        break;
      case GAINFORMAT_QMF64:
        pstr_eq_coefficients->eq_subband_gain_count = 64;
        break;
      case GAINFORMAT_QMFHYBRID39:
        pstr_eq_coefficients->eq_subband_gain_count = 39;
        break;
      case GAINFORMAT_QMF32:
        pstr_eq_coefficients->eq_subband_gain_count = 32;
        break;
      case GAINFORMAT_UNIFORM:
      default:
        bs_eq_gain_count = pstr_eq_coefficients->eq_subband_gain_count - 1;
        bit_cnt_local += iusace_write_bits_buf(it_bit_buf, bs_eq_gain_count, 8);
        break;
    }

    for (idx = 0; idx < pstr_eq_coefficients->unique_eq_subband_gains_count; idx++) {
      if (pstr_eq_coefficients->eq_subband_gain_representation != 1) {
        impd_drc_write_eq_subband_gain_vector(
            it_bit_buf, pstr_eq_coefficients->eq_subband_gain_count,
            &(pstr_eq_coefficients->str_eq_subband_gain_vector[idx]), &bit_cnt_local);
      } else {
        impd_drc_write_eq_subband_gain_spline(
            it_bit_buf, &(pstr_eq_coefficients->str_eq_subband_gain_spline[idx]), &bit_cnt_local);
      }
    }
  }

  *ptr_bit_cnt += bit_cnt_local;

  return err_code;
}

static VOID impd_drc_write_filter_block_refs(
    ia_bit_buf_struct *it_bit_buf, ia_drc_filter_block_refs_struct *pstr_filter_block_refs,
    WORD32 *ptr_bit_cnt) {
  LOOPIDX idx;
  WORD32 bit_cnt_local = 0;

  bit_cnt_local +=
      iusace_write_bits_buf(it_bit_buf, pstr_filter_block_refs->filter_block_count, 4);
  for (idx = 0; idx < pstr_filter_block_refs->filter_block_count; idx++) {
    bit_cnt_local +=
        iusace_write_bits_buf(it_bit_buf, pstr_filter_block_refs->filter_block_index[idx], 7);
  }

  *ptr_bit_cnt += bit_cnt_local;
}

static VOID impd_drc_write_td_filter_cascade(
    ia_bit_buf_struct *it_bit_buf, const WORD32 eq_channel_group_count,
    ia_drc_td_filter_cascade_struct *pstr_td_filter_cascade, WORD32 *ptr_bit_cnt) {
  LOOPIDX i, j;
  WORD32 bs_eq_cascade_gain;
  WORD32 bit_cnt_local = 0;

  for (i = 0; i < eq_channel_group_count; i++) {
    bit_cnt_local +=
        iusace_write_bits_buf(it_bit_buf, pstr_td_filter_cascade->eq_cascade_gain_present[i], 1);
    if (pstr_td_filter_cascade->eq_cascade_gain_present[i] == 1) {
      bs_eq_cascade_gain =
          (WORD32)floor(0.5f + 8.0f * (pstr_td_filter_cascade->eq_cascade_gain[i] + 96.0f));
      bs_eq_cascade_gain = MAX(0, bs_eq_cascade_gain);
      bs_eq_cascade_gain = MIN(1023, bs_eq_cascade_gain);
      bit_cnt_local += iusace_write_bits_buf(it_bit_buf, bs_eq_cascade_gain, 10);
    }
    impd_drc_write_filter_block_refs(
        it_bit_buf, &(pstr_td_filter_cascade->str_filter_block_refs[i]), &bit_cnt_local);
  }

  bit_cnt_local +=
      iusace_write_bits_buf(it_bit_buf, pstr_td_filter_cascade->eq_phase_alignment_present, 1);
  if (pstr_td_filter_cascade->eq_phase_alignment_present == 1) {
    for (i = 0; i < eq_channel_group_count; i++) {
      for (j = i + 1; j < eq_channel_group_count; j++) {
        bit_cnt_local += iusace_write_bits_buf(
            it_bit_buf, pstr_td_filter_cascade->eq_phase_alignment[i][j], 1);
      }
    }
  }

  *ptr_bit_cnt += bit_cnt_local;
}

static IA_ERRORCODE impd_drc_write_eq_instructions(
    ia_bit_buf_struct *it_bit_buf, ia_drc_uni_drc_config_ext_struct *pstr_uni_drc_config_ext,
    ia_drc_gain_enc_struct *pstr_gain_enc, ia_drc_eq_instructions_struct *pstr_eq_instructions,
    WORD32 *ptr_bit_cnt, VOID *ptr_scratch, WORD32 *scratch_used) {
  IA_ERRORCODE err_code = IA_NO_ERROR;
  LOOPIDX idx;
  WORD32 bs_eq_transition_duration;
  WORD32 bit_cnt_local = 0;
  FLOAT32 temp;

  bit_cnt_local += iusace_write_bits_buf(it_bit_buf, pstr_eq_instructions->eq_set_id, 6);

  err_code = impd_drc_get_eq_complexity_level(pstr_uni_drc_config_ext, pstr_gain_enc,
                                              pstr_eq_instructions, ptr_scratch, scratch_used);
  if (err_code & IA_FATAL_ERROR) {
    return err_code;
  }

  bit_cnt_local +=
      iusace_write_bits_buf(it_bit_buf, pstr_eq_instructions->eq_set_complexity_level, 4);

  bit_cnt_local += iusace_write_bits_buf(it_bit_buf, pstr_eq_instructions->downmix_id_present, 1);
  if (pstr_eq_instructions->downmix_id_present == 1) {
    bit_cnt_local += iusace_write_bits_buf(it_bit_buf, pstr_eq_instructions->downmix_id, 7);

    bit_cnt_local +=
        iusace_write_bits_buf(it_bit_buf, pstr_eq_instructions->eq_apply_to_downmix, 1);

    bit_cnt_local +=
        iusace_write_bits_buf(it_bit_buf, pstr_eq_instructions->additional_downmix_id_present, 1);
    if (pstr_eq_instructions->additional_downmix_id_present == 1) {
      bit_cnt_local +=
          iusace_write_bits_buf(it_bit_buf, pstr_eq_instructions->additional_downmix_id_count, 7);
      for (idx = 0; idx < pstr_eq_instructions->additional_downmix_id_count; idx++) {
        bit_cnt_local += iusace_write_bits_buf(
            it_bit_buf, pstr_eq_instructions->additional_downmix_id[idx], 7);
      }
    }
  }

  bit_cnt_local += iusace_write_bits_buf(it_bit_buf, pstr_eq_instructions->drc_set_id, 6);

  bit_cnt_local +=
      iusace_write_bits_buf(it_bit_buf, pstr_eq_instructions->additional_drc_set_id_present, 1);
  if (pstr_eq_instructions->additional_drc_set_id_present == 1) {
    bit_cnt_local +=
        iusace_write_bits_buf(it_bit_buf, pstr_eq_instructions->additional_drc_set_id_count, 6);
    for (idx = 0; idx < pstr_eq_instructions->additional_drc_set_id_count; idx++) {
      bit_cnt_local +=
          iusace_write_bits_buf(it_bit_buf, pstr_eq_instructions->additional_drc_set_id[idx], 6);
    }
  }

  bit_cnt_local += iusace_write_bits_buf(it_bit_buf, pstr_eq_instructions->eq_set_purpose, 16);

  bit_cnt_local +=
      iusace_write_bits_buf(it_bit_buf, pstr_eq_instructions->depends_on_eq_set_present, 1);
  if (pstr_eq_instructions->depends_on_eq_set_present != 1) {
    bit_cnt_local +=
        iusace_write_bits_buf(it_bit_buf, pstr_eq_instructions->no_independent_eq_use, 1);
  } else {
    bit_cnt_local +=
        iusace_write_bits_buf(it_bit_buf, pstr_eq_instructions->depends_on_eq_set, 6);
  }

  for (idx = 0; idx < pstr_eq_instructions->eq_channel_count; idx++) {
    bit_cnt_local += iusace_write_bits_buf(
        it_bit_buf, pstr_eq_instructions->eq_channel_group_for_channel[idx], 7);
  }

  bit_cnt_local +=
      iusace_write_bits_buf(it_bit_buf, pstr_eq_instructions->td_filter_cascade_present, 1);
  if (pstr_eq_instructions->td_filter_cascade_present == 1) {
    impd_drc_write_td_filter_cascade(it_bit_buf, pstr_eq_instructions->eq_channel_group_count,
                                     &(pstr_eq_instructions->str_td_filter_cascade),
                                     &bit_cnt_local);
  }

  bit_cnt_local +=
      iusace_write_bits_buf(it_bit_buf, pstr_eq_instructions->subband_gains_present, 1);
  if (pstr_eq_instructions->subband_gains_present == 1) {
    for (idx = 0; idx < pstr_eq_instructions->eq_channel_group_count; idx++) {
      bit_cnt_local +=
          iusace_write_bits_buf(it_bit_buf, pstr_eq_instructions->subband_gains_index[idx], 6);
    }
  }

  bit_cnt_local +=
      iusace_write_bits_buf(it_bit_buf, pstr_eq_instructions->eq_transition_duration_present, 1);
  if (pstr_eq_instructions->eq_transition_duration_present == 1) {
    temp = MAX(0.004f, pstr_eq_instructions->eq_transition_duration);
    temp = MIN(0.861f, temp);
    bs_eq_transition_duration =
        (WORD32)floor(0.5f + 4.0f * (log10(1000.0f * temp) / log10(2.0f) - 2.0f));
    bit_cnt_local += iusace_write_bits_buf(it_bit_buf, bs_eq_transition_duration, 5);
  }

  *ptr_bit_cnt += bit_cnt_local;

  return err_code;
}

static IA_ERRORCODE impd_drc_write_uni_drc_config_extn(
    ia_drc_enc_state *pstr_drc_state, ia_drc_gain_enc_struct *pstr_gain_enc,
    ia_drc_uni_drc_config_struct *pstr_uni_drc_config,
    ia_drc_uni_drc_config_ext_struct *pstr_uni_drc_config_ext, WORD32 *ptr_bit_cnt,
    FLAG write_bs) {
  IA_ERRORCODE err_code = IA_NO_ERROR;
  LOOPIDX idx;
  WORD32 version;
  WORD32 counter = 0;
  WORD32 ext_size_bits = 0, bit_size_len = 0, bit_size = 0;
  WORD32 bit_cnt_local = 0, bit_cnt_local_ext = 0;
  WORD32 *scratch_used = &pstr_drc_state->drc_scratch_used;
  VOID *ptr_scratch = &pstr_drc_state->drc_scratch_mem;
  ia_bit_buf_struct *it_bit_buf = NULL;
  ia_bit_buf_struct *ptr_bit_buf_ext = NULL;

  if (write_bs) {
    it_bit_buf = &pstr_drc_state->str_bit_buf_cfg;
    ptr_bit_buf_ext = &pstr_drc_state->str_bit_buf_cfg_ext;

    iusace_reset_bit_buffer(ptr_bit_buf_ext);
  }

  bit_cnt_local += iusace_write_bits_buf(
      it_bit_buf, pstr_uni_drc_config_ext->uni_drc_config_ext_type[counter], 4);
  while (counter < 2 &&
         pstr_uni_drc_config_ext->uni_drc_config_ext_type[counter] != UNIDRC_CONF_EXT_TERM) {
    switch (pstr_uni_drc_config_ext->uni_drc_config_ext_type[counter]) {
      case UNIDRC_CONF_EXT_PARAM_DRC: {
#ifdef LOUDNESS_LEVELING_SUPPORT
        bit_cnt_local_ext = 0;
#endif  // LOUDNESS_LEVELING_SUPPORT
        err_code = impd_drc_write_drc_coeff_parametric_drc(
            ptr_bit_buf_ext, pstr_uni_drc_config,
            &(pstr_uni_drc_config_ext->str_drc_coeff_parametric_drc), &bit_cnt_local_ext);

        if (err_code & IA_FATAL_ERROR) {
          return err_code;
        }

        bit_cnt_local_ext += iusace_write_bits_buf(
            ptr_bit_buf_ext, pstr_uni_drc_config_ext->parametric_drc_instructions_count, 4);
        for (idx = 0; idx < pstr_uni_drc_config_ext->parametric_drc_instructions_count; idx++) {
          err_code = impd_drc_write_parametric_drc_instructions(
              ptr_bit_buf_ext,
              pstr_uni_drc_config_ext->str_drc_coeff_parametric_drc.parametric_drc_frame_size,
              &(pstr_uni_drc_config_ext->str_parametric_drc_instructions[idx]),
              &bit_cnt_local_ext);
          if (err_code & IA_FATAL_ERROR) {
            return err_code;
          }
        }
      } break;
      case UNIDRC_CONF_EXT_V1: {
        version = 1;
#ifdef LOUDNESS_LEVELING_SUPPORT
        bit_cnt_local_ext = 0;
#endif  // LOUDNESS_LEVELING_SUPPORT
        bit_cnt_local_ext += iusace_write_bits_buf(
            ptr_bit_buf_ext, pstr_uni_drc_config_ext->downmix_instructions_v1_present, 1);
        if (pstr_uni_drc_config_ext->downmix_instructions_v1_present == 1) {
          bit_cnt_local_ext += iusace_write_bits_buf(
              ptr_bit_buf_ext, pstr_uni_drc_config_ext->downmix_instructions_v1_count, 7);
          for (idx = 0; idx < pstr_uni_drc_config_ext->downmix_instructions_v1_count; idx++) {
            impd_drc_write_downmix_instructions(
                ptr_bit_buf_ext, version, pstr_gain_enc,
                &(pstr_uni_drc_config_ext->str_downmix_instructions_v1[idx]), &bit_cnt_local_ext);
          }
        }

        bit_cnt_local_ext += iusace_write_bits_buf(
            ptr_bit_buf_ext,
            pstr_uni_drc_config_ext->drc_coeffs_and_instructions_uni_drc_v1_present, 1);
        if (pstr_uni_drc_config_ext->drc_coeffs_and_instructions_uni_drc_v1_present == 1) {
          bit_cnt_local_ext += iusace_write_bits_buf(
              ptr_bit_buf_ext, pstr_uni_drc_config_ext->drc_coefficients_uni_drc_v1_count, 3);
          for (idx = 0; idx < pstr_uni_drc_config_ext->drc_coefficients_uni_drc_v1_count; idx++) {
            impd_drc_write_drc_coeff_uni_drc(
                ptr_bit_buf_ext, version,
                &(pstr_uni_drc_config_ext->str_drc_coefficients_uni_drc_v1[idx]),
                &bit_cnt_local_ext);
          }
#ifdef LOUDNESS_LEVELING_SUPPORT
          UWORD32 num_ducking_only_drc_sets = get_num_ducking_only_drc_sets(
              pstr_uni_drc_config_ext->str_drc_instructions_uni_drc_v1,
              pstr_uni_drc_config_ext->drc_instructions_uni_drc_v1_count);

          bit_cnt_local_ext +=
              iusace_write_bits_buf(ptr_bit_buf_ext,
                                    pstr_uni_drc_config_ext->drc_instructions_uni_drc_v1_count -
                                        num_ducking_only_drc_sets,
                                    6);
#else
          bit_cnt_local_ext += iusace_write_bits_buf(
              ptr_bit_buf_ext, pstr_uni_drc_config_ext->drc_instructions_uni_drc_v1_count, 6);
#endif
          for (idx = 0; idx < pstr_uni_drc_config_ext->drc_instructions_uni_drc_v1_count; idx++) {
#ifdef LOUDNESS_LEVELING_SUPPORT
            if (idx > 0 && pstr_uni_drc_config_ext->str_drc_instructions_uni_drc_v1[idx - 1]
                               .ducking_only_set_present) {
              continue;
            }
#endif
            err_code = impd_drc_write_drc_instruct_uni_drc(
                ptr_bit_buf_ext, version, pstr_uni_drc_config, pstr_gain_enc,
                &(pstr_uni_drc_config_ext->str_drc_instructions_uni_drc_v1[idx]), ptr_scratch,
                &bit_cnt_local_ext);
            if (err_code & IA_FATAL_ERROR) {
              return (err_code);
            }
          }
        }

        bit_cnt_local_ext += iusace_write_bits_buf(
            ptr_bit_buf_ext, pstr_uni_drc_config_ext->loud_eq_instructions_present, 1);
        if (pstr_uni_drc_config_ext->loud_eq_instructions_present == 1) {
          bit_cnt_local_ext += iusace_write_bits_buf(
              ptr_bit_buf_ext, pstr_uni_drc_config_ext->loud_eq_instructions_count, 4);
          for (idx = 0; idx < pstr_uni_drc_config_ext->loud_eq_instructions_count; idx++) {
            impd_drc_write_loud_eq_instructions(
                ptr_bit_buf_ext, &(pstr_uni_drc_config_ext->str_loud_eq_instructions[idx]),
                &bit_cnt_local_ext);
          }
        }

        bit_cnt_local_ext +=
            iusace_write_bits_buf(ptr_bit_buf_ext, pstr_uni_drc_config_ext->eq_present, 1);
        if (pstr_uni_drc_config_ext->eq_present == 1) {
          err_code = impd_drc_write_eq_coefficients(
              ptr_bit_buf_ext, &(pstr_uni_drc_config_ext->str_eq_coefficients),
              &bit_cnt_local_ext);
          if (err_code & IA_FATAL_ERROR) {
            return err_code;
          }

          bit_cnt_local_ext += iusace_write_bits_buf(
              ptr_bit_buf_ext, pstr_uni_drc_config_ext->eq_instructions_count, 4);
          for (idx = 0; idx < pstr_uni_drc_config_ext->eq_instructions_count; idx++) {
            err_code = impd_drc_write_eq_instructions(
                ptr_bit_buf_ext, pstr_uni_drc_config_ext, pstr_gain_enc,
                &(pstr_uni_drc_config_ext->str_eq_instructions[idx]), &bit_cnt_local_ext,
                ptr_scratch, scratch_used);
            if (err_code & IA_FATAL_ERROR) {
              return (err_code);
            }
          }
        }
      } break;

#ifdef LOUDNESS_LEVELING_SUPPORT
      case UNIDRCCONFEXT_LEVELING: {
        bit_cnt_local_ext = 0;
        err_code = write_loudness_leveling_extension(
            ptr_bit_buf_ext, pstr_uni_drc_config, pstr_gain_enc, ptr_scratch, &bit_cnt_local_ext);
        if (err_code & IA_FATAL_ERROR) {
          return (err_code);
        }
      } break;
#endif
      default:
        break;
    }

    pstr_uni_drc_config_ext->ext_bit_size[counter] = bit_cnt_local_ext;
    bit_size = pstr_uni_drc_config_ext->ext_bit_size[counter] - 1;
    ext_size_bits = (WORD32)(log((FLOAT32)bit_size) / log(2.f)) + 1;
#ifdef LOUDNESS_LEVELING_SUPPORT
    ext_size_bits = (ext_size_bits < 4) ? 4 : ext_size_bits;
#endif  // LOUDNESS_LEVELING_SUPPORT
    bit_size_len = ext_size_bits - 4;

    bit_cnt_local += iusace_write_bits_buf(it_bit_buf, bit_size_len, 4);

    bit_cnt_local += iusace_write_bits_buf(it_bit_buf, bit_size, (UWORD8)ext_size_bits);

    switch (pstr_uni_drc_config_ext->uni_drc_config_ext_type[counter]) {
      case UNIDRC_CONF_EXT_PARAM_DRC: {
        err_code = impd_drc_write_drc_coeff_parametric_drc(
            it_bit_buf, pstr_uni_drc_config,
            &(pstr_uni_drc_config_ext->str_drc_coeff_parametric_drc), &bit_cnt_local);

        if (err_code & IA_FATAL_ERROR) {
          return (err_code);
        }

        bit_cnt_local += iusace_write_bits_buf(
            it_bit_buf, pstr_uni_drc_config_ext->parametric_drc_instructions_count, 4);
        for (idx = 0; idx < pstr_uni_drc_config_ext->parametric_drc_instructions_count; idx++) {
          err_code = impd_drc_write_parametric_drc_instructions(
              it_bit_buf,
              pstr_uni_drc_config_ext->str_drc_coeff_parametric_drc.parametric_drc_frame_size,
              &(pstr_uni_drc_config_ext->str_parametric_drc_instructions[idx]), &bit_cnt_local);
          if (err_code & IA_FATAL_ERROR) {
            return (err_code);
          }
        }
      } break;
      case UNIDRC_CONF_EXT_V1: {
        version = 1;
        bit_cnt_local += iusace_write_bits_buf(
            it_bit_buf, pstr_uni_drc_config_ext->downmix_instructions_v1_present, 1);
        if (pstr_uni_drc_config_ext->downmix_instructions_v1_present == 1) {
          bit_cnt_local += iusace_write_bits_buf(
              it_bit_buf, pstr_uni_drc_config_ext->downmix_instructions_v1_count, 7);
          for (idx = 0; idx < pstr_uni_drc_config_ext->downmix_instructions_v1_count; idx++) {
            impd_drc_write_downmix_instructions(
                it_bit_buf, version, pstr_gain_enc,
                &(pstr_uni_drc_config_ext->str_downmix_instructions_v1[idx]), &bit_cnt_local);
          }
        }

        bit_cnt_local += iusace_write_bits_buf(
            it_bit_buf, pstr_uni_drc_config_ext->drc_coeffs_and_instructions_uni_drc_v1_present,
            1);
        if (pstr_uni_drc_config_ext->drc_coeffs_and_instructions_uni_drc_v1_present == 1) {
          version = 1;
          bit_cnt_local += iusace_write_bits_buf(
              it_bit_buf, pstr_uni_drc_config_ext->drc_coefficients_uni_drc_v1_count, 3);
          for (idx = 0; idx < pstr_uni_drc_config_ext->drc_coefficients_uni_drc_v1_count; idx++) {
            impd_drc_write_drc_coeff_uni_drc(
                it_bit_buf, version,
                &(pstr_uni_drc_config_ext->str_drc_coefficients_uni_drc_v1[idx]), &bit_cnt_local);
          }
#ifdef LOUDNESS_LEVELING_SUPPORT
          UWORD32 num_ducking_only_drc_sets = get_num_ducking_only_drc_sets(
              pstr_uni_drc_config_ext->str_drc_instructions_uni_drc_v1,
              pstr_uni_drc_config_ext->drc_instructions_uni_drc_v1_count);
          bit_cnt_local +=
              iusace_write_bits_buf(it_bit_buf,
                                    pstr_uni_drc_config_ext->drc_instructions_uni_drc_v1_count -
                                        num_ducking_only_drc_sets,
                                    6);
#else
          bit_cnt_local += iusace_write_bits_buf(
              it_bit_buf, pstr_uni_drc_config_ext->drc_instructions_uni_drc_v1_count, 6);
#endif
          for (idx = 0; idx < pstr_uni_drc_config_ext->drc_instructions_uni_drc_v1_count; idx++) {
#ifdef LOUDNESS_LEVELING_SUPPORT
            if (idx > 0 && pstr_uni_drc_config_ext->str_drc_instructions_uni_drc_v1[idx - 1]
                               .ducking_only_set_present) {
              continue;
            }
#endif
            err_code = impd_drc_write_drc_instruct_uni_drc(
                it_bit_buf, version, pstr_uni_drc_config, pstr_gain_enc,
                &(pstr_uni_drc_config_ext->str_drc_instructions_uni_drc_v1[idx]), ptr_scratch,
                &bit_cnt_local);
            if (err_code & IA_FATAL_ERROR) {
              return (err_code);
            }
          }
        }

        bit_cnt_local += iusace_write_bits_buf(
            it_bit_buf, pstr_uni_drc_config_ext->loud_eq_instructions_present, 1);
        if (pstr_uni_drc_config_ext->loud_eq_instructions_present == 1) {
          bit_cnt_local += iusace_write_bits_buf(
              it_bit_buf, pstr_uni_drc_config_ext->loud_eq_instructions_count, 4);
          for (idx = 0; idx < pstr_uni_drc_config_ext->loud_eq_instructions_count; idx++) {
            impd_drc_write_loud_eq_instructions(
                it_bit_buf, &(pstr_uni_drc_config_ext->str_loud_eq_instructions[idx]),
                &bit_cnt_local);
          }
        }

        bit_cnt_local +=
            iusace_write_bits_buf(it_bit_buf, pstr_uni_drc_config_ext->eq_present, 1);
        if (pstr_uni_drc_config_ext->eq_present == 1) {
          err_code = impd_drc_write_eq_coefficients(
              it_bit_buf, &(pstr_uni_drc_config_ext->str_eq_coefficients), &bit_cnt_local);
          if (err_code & IA_FATAL_ERROR) {
            return err_code;
          }

          bit_cnt_local += iusace_write_bits_buf(
              it_bit_buf, pstr_uni_drc_config_ext->eq_instructions_count, 4);
          for (idx = 0; idx < pstr_uni_drc_config_ext->eq_instructions_count; idx++) {
            err_code = impd_drc_write_eq_instructions(
                it_bit_buf, pstr_uni_drc_config_ext, pstr_gain_enc,
                &(pstr_uni_drc_config_ext->str_eq_instructions[idx]), &bit_cnt_local, ptr_scratch,
                scratch_used);
            if (err_code & IA_FATAL_ERROR) {
              return err_code;
            }
          }
        }
      } break;
#ifdef LOUDNESS_LEVELING_SUPPORT
      case UNIDRCCONFEXT_LEVELING: {
        err_code = write_loudness_leveling_extension(it_bit_buf, pstr_uni_drc_config,
                                                     pstr_gain_enc, ptr_scratch, &bit_cnt_local);
        if (err_code & IA_FATAL_ERROR) {
          return (err_code);
        }
      } break;
#endif
      default:
        for (idx = 0; idx < pstr_uni_drc_config_ext->ext_bit_size[counter]; idx++) {
          bit_cnt_local += iusace_write_bits_buf(it_bit_buf, 0, 1);
        }
        break;
    }

    counter++;

    bit_cnt_local += iusace_write_bits_buf(
        it_bit_buf, pstr_uni_drc_config_ext->uni_drc_config_ext_type[counter], 4);
  }

  *ptr_bit_cnt += bit_cnt_local;

  return err_code;
}

IA_ERRORCODE impd_drc_write_loudness_info_set_extension(
    ia_drc_enc_state *pstr_drc_state, ia_bit_buf_struct *it_bit_buf,
    ia_drc_loudness_info_set_extension_struct *pstr_loudness_info_set_extension,
    WORD32 *ptr_bit_cnt, FLAG write_bs) {
  IA_ERRORCODE err_code = IA_NO_ERROR;
  LOOPIDX idx;
  WORD32 counter = 0, version = 1;
  WORD32 ext_size_bits = 0, bit_size_len = 0, bit_size = 0;
  WORD32 bit_cnt_local = 0;
  WORD32 bit_cnt_local_tmp = 0;
  ia_drc_loudness_info_set_ext_eq_struct *pstr_loudness_info_set_ext_eq =
      &pstr_loudness_info_set_extension->str_loudness_info_set_ext_eq;
  ia_bit_buf_struct *ptr_bit_buf_tmp = NULL;

  if (write_bs) {
    ptr_bit_buf_tmp = &pstr_drc_state->str_bit_buf_cfg_tmp;

    iusace_reset_bit_buffer(ptr_bit_buf_tmp);
  }

  bit_cnt_local += iusace_write_bits_buf(
      it_bit_buf, pstr_loudness_info_set_extension->loudness_info_set_ext_type[counter], 4);
  while ((counter < 2) &&
         (pstr_loudness_info_set_extension->loudness_info_set_ext_type[counter] !=
          UNIDRC_LOUD_EXT_TERM)) {
    switch (pstr_loudness_info_set_extension->loudness_info_set_ext_type[counter]) {
      case UNIDRC_LOUD_EXT_EQ: {
        bit_cnt_local_tmp += iusace_write_bits_buf(
            ptr_bit_buf_tmp, pstr_loudness_info_set_ext_eq->loudness_info_v1_album_count, 6);

        bit_cnt_local_tmp += iusace_write_bits_buf(
            ptr_bit_buf_tmp, pstr_loudness_info_set_ext_eq->loudness_info_v1_count, 6);
        for (idx = 0; idx < pstr_loudness_info_set_ext_eq->loudness_info_v1_album_count; idx++) {
          err_code = impd_drc_write_loudness_info(
              ptr_bit_buf_tmp, version,
              &(pstr_loudness_info_set_ext_eq->str_loudness_info_v1_album[idx]),
              &bit_cnt_local_tmp);
          if (err_code) {
            return err_code;
          }
        }
        for (idx = 0; idx < pstr_loudness_info_set_ext_eq->loudness_info_v1_count; idx++) {
          err_code = impd_drc_write_loudness_info(
              ptr_bit_buf_tmp, version,
              &(pstr_loudness_info_set_ext_eq->str_loudness_info_v1[idx]), &bit_cnt_local_tmp);
          if (err_code) {
            return (err_code);
          }
        }
      } break;
      default:
        break;
    }
    pstr_loudness_info_set_extension->ext_bit_size[counter] = bit_cnt_local_tmp;
    bit_size = pstr_loudness_info_set_extension->ext_bit_size[counter] - 1;
    ext_size_bits = (WORD32)(log((FLOAT32)bit_size) / log(2.f)) + 1;
    bit_size_len = ext_size_bits - 4;
    bit_cnt_local += iusace_write_bits_buf(it_bit_buf, bit_size_len, 4);
    bit_cnt_local += iusace_write_bits_buf(it_bit_buf, bit_size, (UWORD8)ext_size_bits);

    switch (pstr_loudness_info_set_extension->loudness_info_set_ext_type[counter]) {
      case UNIDRC_LOUD_EXT_EQ: {
        bit_cnt_local += iusace_write_bits_buf(
            it_bit_buf, pstr_loudness_info_set_ext_eq->loudness_info_v1_album_count, 6);

        bit_cnt_local += iusace_write_bits_buf(
            it_bit_buf, pstr_loudness_info_set_ext_eq->loudness_info_v1_count, 6);
        for (idx = 0; idx < pstr_loudness_info_set_ext_eq->loudness_info_v1_album_count; idx++) {
          err_code = impd_drc_write_loudness_info(
              it_bit_buf, version,
              &(pstr_loudness_info_set_ext_eq->str_loudness_info_v1_album[idx]), &bit_cnt_local);
          if (err_code) {
            return err_code;
          }
        }
        for (idx = 0; idx < pstr_loudness_info_set_ext_eq->loudness_info_v1_count; idx++) {
          err_code = impd_drc_write_loudness_info(
              it_bit_buf, version, &(pstr_loudness_info_set_ext_eq->str_loudness_info_v1[idx]),
              &bit_cnt_local);
          if (err_code) {
            return (err_code);
          }
        }
      } break;
      default:
        for (idx = 0; idx < pstr_loudness_info_set_extension->ext_bit_size[counter]; idx++) {
          bit_cnt_local += iusace_write_bits_buf(it_bit_buf, 0, 1);
        }
        break;
    }
    counter++;

    bit_cnt_local += iusace_write_bits_buf(
        it_bit_buf, pstr_loudness_info_set_extension->loudness_info_set_ext_type[counter], 4);
  }

  *ptr_bit_cnt += bit_cnt_local;

  return IA_NO_ERROR;
}

IA_ERRORCODE impd_drc_write_loudness_info_set(ia_drc_enc_state *pstr_drc_state,
                                              ia_bit_buf_struct *it_bit_buf, WORD32 *ptr_bit_cnt,
                                              FLAG write_bs) {
  IA_ERRORCODE err_code = IA_NO_ERROR;
  LOOPIDX idx;
  WORD32 version = 0;
  WORD32 bit_cnt_local = 0;
  ia_drc_gain_enc_struct *pstr_gain_enc = &pstr_drc_state->str_gain_enc;
  ia_drc_loudness_info_set_struct *pstr_loudness_info_set =
      &(pstr_gain_enc->str_loudness_info_set);

  bit_cnt_local +=
      iusace_write_bits_buf(it_bit_buf, pstr_loudness_info_set->loudness_info_album_count, 6);

  bit_cnt_local +=
      iusace_write_bits_buf(it_bit_buf, pstr_loudness_info_set->loudness_info_count, 6);

  for (idx = 0; idx < pstr_loudness_info_set->loudness_info_album_count; idx++) {
    err_code = impd_drc_write_loudness_info(it_bit_buf, version,
                                            &pstr_loudness_info_set->str_loudness_info_album[idx],
                                            &bit_cnt_local);
    if (err_code) {
      return err_code;
    }
  }

  for (idx = 0; idx < pstr_loudness_info_set->loudness_info_count; idx++) {
    err_code = impd_drc_write_loudness_info(
        it_bit_buf, version, &pstr_loudness_info_set->str_loudness_info[idx], &bit_cnt_local);
    if (err_code) {
      return err_code;
    }
  }

  bit_cnt_local +=
      iusace_write_bits_buf(it_bit_buf, pstr_loudness_info_set->loudness_info_set_ext_present, 1);
  if (pstr_loudness_info_set->loudness_info_set_ext_present) {
    err_code = impd_drc_write_loudness_info_set_extension(
        pstr_drc_state, it_bit_buf, &pstr_loudness_info_set->str_loudness_info_set_extension,
        &bit_cnt_local, write_bs);
    if (err_code) {
      return err_code;
    }
  }

  *ptr_bit_cnt += bit_cnt_local;

  return err_code;
}

IA_ERRORCODE impd_drc_write_uni_drc_config(ia_drc_enc_state *pstr_drc_state, WORD32 *ptr_bit_cnt,
                                           FLAG write_bs) {
  IA_ERRORCODE err_code = IA_NO_ERROR;
  LOOPIDX idx;
  WORD32 version = 0;
  WORD32 bit_cnt_local = 0;
  VOID *ptr_scratch = pstr_drc_state->drc_scratch_mem;
  ia_bit_buf_struct *it_bit_buf = NULL;
  ia_drc_gain_enc_struct *pstr_gain_enc = &pstr_drc_state->str_gain_enc;
  ia_drc_uni_drc_config_struct *pstr_uni_drc_config = &(pstr_gain_enc->str_uni_drc_config);

  if (write_bs) {
    it_bit_buf = &pstr_drc_state->str_bit_buf_cfg;
  }

  bit_cnt_local += iusace_write_bits_buf(it_bit_buf, pstr_uni_drc_config->sample_rate_present, 1);

  if (1 == pstr_uni_drc_config->sample_rate_present) {
    bit_cnt_local +=
        iusace_write_bits_buf(it_bit_buf, (pstr_uni_drc_config->sample_rate - 1000), 18);
  }

  bit_cnt_local +=
      iusace_write_bits_buf(it_bit_buf, pstr_uni_drc_config->downmix_instructions_count, 7);

  bit_cnt_local +=
      iusace_write_bits_buf(it_bit_buf, pstr_uni_drc_config->drc_description_basic_present, 1);

  if (1 == pstr_uni_drc_config->drc_description_basic_present) {
    bit_cnt_local +=
        iusace_write_bits_buf(it_bit_buf, pstr_uni_drc_config->drc_coefficients_basic_count, 3);
    bit_cnt_local +=
        iusace_write_bits_buf(it_bit_buf, pstr_uni_drc_config->drc_instructions_basic_count, 4);
  }

  bit_cnt_local +=
      iusace_write_bits_buf(it_bit_buf, pstr_uni_drc_config->drc_coefficients_uni_drc_count, 3);

#ifdef LOUDNESS_LEVELING_SUPPORT
  UWORD32 num_ducking_only_drc_sets =
      get_num_ducking_only_drc_sets(pstr_uni_drc_config->str_drc_instructions_uni_drc,
                                    pstr_uni_drc_config->drc_instructions_uni_drc_count);
  bit_cnt_local += iusace_write_bits_buf(
      it_bit_buf, pstr_uni_drc_config->drc_instructions_uni_drc_count - num_ducking_only_drc_sets,
      6);
#else
  bit_cnt_local +=
      iusace_write_bits_buf(it_bit_buf, pstr_uni_drc_config->drc_instructions_uni_drc_count, 6);
#endif
  bit_cnt_local +=
      iusace_write_bits_buf(it_bit_buf, pstr_uni_drc_config->str_channel_layout.base_ch_count, 7);
  bit_cnt_local += iusace_write_bits_buf(
      it_bit_buf, pstr_uni_drc_config->str_channel_layout.layout_signaling_present, 1);
  if (1 == pstr_uni_drc_config->str_channel_layout.layout_signaling_present) {
    bit_cnt_local += iusace_write_bits_buf(
        it_bit_buf, pstr_uni_drc_config->str_channel_layout.defined_layout, 8);
    if (0 == pstr_uni_drc_config->str_channel_layout.defined_layout) {
      for (idx = 0; idx < pstr_uni_drc_config->str_channel_layout.base_ch_count; idx++) {
        bit_cnt_local += iusace_write_bits_buf(
            it_bit_buf, pstr_uni_drc_config->str_channel_layout.speaker_position[idx], 7);
      }
    }
  }

  for (idx = 0; idx < pstr_uni_drc_config->downmix_instructions_count; idx++) {
    // downmixInstructions();
  }

  for (idx = 0; idx < pstr_uni_drc_config->drc_coefficients_basic_count; idx++) {
    // drcCoefficientsBasic();
  }

  for (idx = 0; idx < pstr_uni_drc_config->drc_instructions_basic_count; idx++) {
    // drcInstructionsBasics();
  }

  for (idx = 0; idx < pstr_uni_drc_config->drc_coefficients_uni_drc_count; idx++) {
    impd_drc_write_drc_coeff_uni_drc(it_bit_buf, version,
                                     &(pstr_uni_drc_config->str_drc_coefficients_uni_drc[idx]),
                                     &bit_cnt_local);
  }

  for (idx = 0; idx < pstr_uni_drc_config->drc_instructions_uni_drc_count; idx++) {
#ifdef LOUDNESS_LEVELING_SUPPORT
    if (idx > 0 &&
        pstr_uni_drc_config->str_drc_instructions_uni_drc[idx - 1].ducking_only_set_present) {
      continue;
    }
#endif
    err_code = impd_drc_write_drc_instruct_uni_drc(
        it_bit_buf, version, pstr_uni_drc_config, pstr_gain_enc,
        &(pstr_uni_drc_config->str_drc_instructions_uni_drc[idx]), ptr_scratch, &bit_cnt_local);
    if (err_code & IA_FATAL_ERROR) {
      return err_code;
    }
  }

  bit_cnt_local +=
      iusace_write_bits_buf(it_bit_buf, pstr_uni_drc_config->uni_drc_config_ext_present, 1);
  if (pstr_uni_drc_config->uni_drc_config_ext_present) {
    err_code = impd_drc_write_uni_drc_config_extn(
        pstr_drc_state, pstr_gain_enc, pstr_uni_drc_config,
        &(pstr_uni_drc_config->str_uni_drc_config_ext), &bit_cnt_local, write_bs);
    if (err_code & IA_FATAL_ERROR) {
      return (err_code);
    }
  }

  *ptr_bit_cnt += bit_cnt_local;

  return err_code;
}

IA_ERRORCODE impd_drc_write_measured_loudness_info(ia_drc_enc_state *pstr_drc_state) {

  IA_ERRORCODE err_code = IA_NO_ERROR;
  ia_bit_buf_struct *it_bit_buf_lis = &pstr_drc_state->str_bit_buf_cfg_ext;
  WORD32 bit_cnt_lis = 0;
  err_code = impd_drc_write_loudness_info_set(pstr_drc_state, it_bit_buf_lis, &bit_cnt_lis, 1);
  if (err_code & IA_FATAL_ERROR) {
    return (err_code);
  }
  pstr_drc_state->drc_config_ext_data_size_bit = bit_cnt_lis;

  return err_code;
}

IA_ERRORCODE impd_drc_enc_initial_gain(const WORD32 gain_coding_profile, FLOAT32 gain_initial,
                                       FLOAT32 *gain_initial_quant, WORD32 *code_size,
                                       WORD32 *code) {
  WORD32 sign, magnitude, bits, size;

  switch (gain_coding_profile) {
    case GAIN_CODING_PROFILE_CONSTANT: {
      bits = 0;
      size = 0;
    } break;
    case GAIN_CODING_PROFILE_CLIPPING: {
      if (gain_initial > -0.0625f) {
        sign = 0;
        *gain_initial_quant = 0.0f;
        bits = sign;
        size = 1;
      } else {
        sign = 1;
        gain_initial = MAX(-1000.0f, gain_initial);
        magnitude = (WORD32)(-1.0f + 0.5f - 8.0f * gain_initial);
        magnitude = MIN(0xFF, magnitude);
        *gain_initial_quant = -(magnitude + 1) * 0.125f;
        bits = (sign << 8) + magnitude;
        size = 9;
      }
    } break;
    case GAIN_CODING_PROFILE_FADING: {
      if (gain_initial > -0.0625f) {
        sign = 0;
        *gain_initial_quant = 0.0f;
        bits = sign;
        size = 1;
      } else {
        sign = 1;
        gain_initial = MAX(-1000.0f, gain_initial);
        magnitude = (WORD32)(-1.0f + 0.5f - 8.0f * gain_initial);
        magnitude = MIN(0x3FF, magnitude);
        *gain_initial_quant = -(magnitude + 1) * 0.125f;
        bits = (sign << 10) + magnitude;
        size = 11;
      }
    } break;
    case GAIN_CODING_PROFILE_REGULAR: {
      if (gain_initial < 0.0f) {
        sign = 1;
        gain_initial = MAX(-1000.0f, gain_initial);
        magnitude = (WORD32)(0.5f - 8.0f * gain_initial);
        magnitude = MIN(0xFF, magnitude);
        *gain_initial_quant = -magnitude * 0.125f;
      } else {
        sign = 0;
        gain_initial = MIN(1000.0f, gain_initial);
        magnitude = (WORD32)(0.5f + 8.0f * gain_initial);
        magnitude = MIN(0xFF, magnitude);
        *gain_initial_quant = magnitude * 0.125f;
      }
      bits = (sign << 8) + magnitude;
      size = 9;
    } break;
    default:
      return IA_EXHEAACE_CONFIG_FATAL_DRC_INVALID_CONFIG;
  }

  *code = bits;
  *code_size = size;

  return IA_NO_ERROR;
}

static VOID impd_drc_write_spline_nodes(ia_bit_buf_struct *it_bit_buf,
                                        ia_drc_gain_enc_struct *pstr_gain_enc,
                                        ia_drc_gain_set_params_struct *pstr_gain_set_params,
                                        ia_drc_group_for_output_struct *pstr_drc_group_for_output,
                                        WORD32 *ptr_bit_cnt) {
  LOOPIDX idx;
  WORD32 frame_end_flag;
  WORD32 bit_cnt_local = 0;

  bit_cnt_local += iusace_write_bits_buf(it_bit_buf, pstr_drc_group_for_output->coding_mode, 1);
  if (pstr_drc_group_for_output->coding_mode != 0) {
    for (idx = 0; idx < pstr_drc_group_for_output->n_gain_values - 1; idx++) {
      bit_cnt_local += iusace_write_bits_buf(it_bit_buf, 0, 1);
    }
    bit_cnt_local += iusace_write_bits_buf(it_bit_buf, 1, 1);

    if (pstr_gain_set_params->gain_interpolation_type == GAIN_INTERPOLATION_TYPE_SPLINE) {
      for (idx = 0; idx < pstr_drc_group_for_output->n_gain_values; idx++) {
        bit_cnt_local +=
            iusace_write_bits_buf(it_bit_buf, pstr_drc_group_for_output->slope_code[idx],
                                  (UWORD8)pstr_drc_group_for_output->slope_code_size[idx]);
      }
    }

    if (pstr_gain_set_params->full_frame == 0 && pstr_drc_group_for_output->n_gain_values > 0) {
      if (pstr_drc_group_for_output->ts_gain_quant[pstr_drc_group_for_output->n_gain_values -
                                                   1] == (pstr_gain_enc->drc_frame_size - 1)) {
        frame_end_flag = 1;
      } else {
        frame_end_flag = 0;
      }
      bit_cnt_local += iusace_write_bits_buf(it_bit_buf, frame_end_flag, 1);
    } else {
      frame_end_flag = 1;
    }

    for (idx = 0; idx < pstr_drc_group_for_output->n_gain_values; idx++) {
      if (idx < (pstr_drc_group_for_output->n_gain_values - 1) || !frame_end_flag) {
        bit_cnt_local +=
            iusace_write_bits_buf(it_bit_buf, pstr_drc_group_for_output->time_delta_code[idx],
                                  (UWORD8)pstr_drc_group_for_output->time_delta_code_size[idx]);
      }
    }
    for (idx = 0; idx < pstr_drc_group_for_output->n_gain_values; idx++) {
      bit_cnt_local +=
          iusace_write_bits_buf(it_bit_buf, pstr_drc_group_for_output->gain_code[idx],
                                (UWORD8)pstr_drc_group_for_output->gain_code_length[idx]);
    }
  } else {
    bit_cnt_local +=
        iusace_write_bits_buf(it_bit_buf, pstr_drc_group_for_output->gain_code[0],
                              (UWORD8)pstr_drc_group_for_output->gain_code_length[0]);
  }

  *ptr_bit_cnt += bit_cnt_local;
}

static VOID impd_drc_write_uni_drc_gain_extension(
    ia_bit_buf_struct *it_bit_buf, ia_drc_uni_drc_gain_ext_struct *pstr_uni_drc_gain_ext,
    WORD32 *ptr_bit_cnt) {
  LOOPIDX idx;
  WORD32 counter = 0;
  WORD32 ext_size_bits, bit_size_len, bit_size;
  WORD32 bit_cnt_local = 0;

  bit_cnt_local +=
      iusace_write_bits_buf(it_bit_buf, pstr_uni_drc_gain_ext->uni_drc_gain_ext_type[counter], 4);
  while (pstr_uni_drc_gain_ext->uni_drc_gain_ext_type[counter] != UNIDRC_GAIN_EXT_TERM) {
    bit_size = pstr_uni_drc_gain_ext->ext_bit_size[counter] - 1;
    ext_size_bits = (WORD32)(log((FLOAT32)bit_size) / log(2.f)) + 1;
    bit_size_len = ext_size_bits - 4;
    bit_cnt_local += iusace_write_bits_buf(it_bit_buf, bit_size_len, 3);
    bit_cnt_local += iusace_write_bits_buf(it_bit_buf, bit_size, (UWORD8)ext_size_bits);
    switch (pstr_uni_drc_gain_ext->uni_drc_gain_ext_type[counter]) {
      default:
        for (idx = 0; idx < pstr_uni_drc_gain_ext->ext_bit_size[counter]; idx++) {
          bit_cnt_local += iusace_write_bits_buf(it_bit_buf, 0, 1);
        }
    }
    counter++;

    bit_cnt_local += iusace_write_bits_buf(
        it_bit_buf, pstr_uni_drc_gain_ext->uni_drc_gain_ext_type[counter], 4);
  }

  *ptr_bit_cnt += bit_cnt_local;
}

VOID impd_drc_write_uni_drc_gain(ia_drc_enc_state *pstr_drc_state, WORD32 *ptr_bit_cnt) {
  LOOPIDX idx;
  WORD32 bit_cnt_local = 0;
  ia_bit_buf_struct *it_bit_buf = &pstr_drc_state->str_bit_buf_out;
  ia_drc_gain_enc_struct *pstr_gain_enc = &pstr_drc_state->str_gain_enc;
  ia_drc_group_for_output_struct *pstr_drc_group_for_output;
  ia_drc_gain_set_params_struct *pstr_gain_set_params;
  ia_drc_uni_drc_gain_ext_struct str_uni_drc_gain_extension =
      pstr_drc_state->str_enc_gain_extension;

  for (idx = 0; idx < pstr_gain_enc->n_sequences; idx++) {
    pstr_drc_group_for_output =
        &(pstr_gain_enc->str_drc_gain_seq_buf[idx].str_drc_group_for_output);
    pstr_gain_set_params = &(pstr_gain_enc->str_drc_gain_seq_buf[idx].str_gain_set_params);

    if (pstr_gain_set_params->gain_coding_profile < GAIN_CODING_PROFILE_CONSTANT) {
      impd_drc_write_spline_nodes(it_bit_buf, pstr_gain_enc, pstr_gain_set_params,
                                  pstr_drc_group_for_output, &bit_cnt_local);
    }
  }

  bit_cnt_local +=
      iusace_write_bits_buf(it_bit_buf, str_uni_drc_gain_extension.uni_drc_gain_ext_present, 1);
  if (str_uni_drc_gain_extension.uni_drc_gain_ext_present) {
    impd_drc_write_uni_drc_gain_extension(it_bit_buf, &str_uni_drc_gain_extension,
                                          &bit_cnt_local);
  }

  if (bit_cnt_local & 0x07) {
    pstr_drc_state->bit_buf_base_out[bit_cnt_local >> 3] |= 0xFF >> (bit_cnt_local & 0x07);

    bit_cnt_local += 8 - (bit_cnt_local & 0x07);
  }

  *ptr_bit_cnt += bit_cnt_local;
}
