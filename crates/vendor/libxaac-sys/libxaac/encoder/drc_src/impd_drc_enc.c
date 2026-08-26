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
#include "impd_drc_common_enc.h"
#include "impd_drc_uni_drc.h"
#include "impd_drc_tables.h"
#include "impd_drc_api.h"
#include "impd_drc_uni_drc_eq.h"
#include "impd_drc_uni_drc_filter_bank.h"
#include "impd_drc_gain_enc.h"
#include "impd_drc_struct_def.h"
#include "impd_drc_enc.h"

static VOID impd_drc_util_stft_read_gain_config(
    ia_drc_stft_gain_calc_struct *pstr_stft_drc_gain_handle, WORD32 band_count,
    ia_drc_gain_set_params_struct *str_gain_set_params) {
  LOOPIDX i, j;
  WORD32 num_points;

  for (i = 0; i < band_count; i++) {
    num_points = str_gain_set_params->gain_params[i].nb_points;
    pstr_stft_drc_gain_handle[i].nb_points = num_points;
    for (j = 0; j < num_points; j++) {
      pstr_stft_drc_gain_handle[i].str_segment[2 * (j + 1)].x =
          str_gain_set_params->gain_params[i].gain_points[j].x;
      pstr_stft_drc_gain_handle[i].str_segment[2 * (j + 1)].y =
          str_gain_set_params->gain_params[i].gain_points[j].y;
    }

    pstr_stft_drc_gain_handle[i].width_db = str_gain_set_params->gain_params[i].width;
    pstr_stft_drc_gain_handle[i].attack_ms = str_gain_set_params->gain_params[i].attack;
    pstr_stft_drc_gain_handle[i].release_ms = str_gain_set_params->gain_params[i].decay;
  }
}

static VOID impd_drc_util_td_read_gain_config(
    ia_drc_compand_struct *pstr_drc_compand, ia_drc_gain_set_params_struct *str_gain_set_params) {
  LOOPIDX idx;
  WORD32 num_points;

  num_points = str_gain_set_params->gain_params[0].nb_points;
  pstr_drc_compand->nb_points = num_points;
  for (idx = 0; idx < num_points; idx++) {
    pstr_drc_compand->str_segment[2 * (idx + 1)].x =
        str_gain_set_params->gain_params[0].gain_points[idx].x;
    pstr_drc_compand->str_segment[2 * (idx + 1)].y =
        str_gain_set_params->gain_params[0].gain_points[idx].y;
  }

  pstr_drc_compand->width_db = str_gain_set_params->gain_params[0].width;
  pstr_drc_compand->str_channel_param.attack = str_gain_set_params->gain_params[0].attack;
  pstr_drc_compand->str_channel_param.decay = str_gain_set_params->gain_params[0].decay;

  pstr_drc_compand->str_channel_param.attack /= 1000.0;
  pstr_drc_compand->str_channel_param.decay /= 1000.0;
}

IA_ERRORCODE impd_drc_gain_enc_init(ia_drc_gain_enc_struct *pstr_gain_enc,
                                    ia_drc_uni_drc_config_struct *pstr_uni_drc_config,
                                    ia_drc_loudness_info_set_struct *pstr_loudness_info_set,
                                    const WORD32 frame_size, const WORD32 sample_rate,
                                    const WORD32 delay_mode, const WORD32 domain) {
  IA_ERRORCODE err_code = IA_NO_ERROR;
  LOOPIDX i, j, k, l, m, ch;
  WORD32 num_gain_values_max;
  UWORD8 found_ch_idx;
  UWORD32 ch_idx;

  ia_drc_uni_drc_config_ext_struct *pstr_uni_drc_config_ext =
      &pstr_uni_drc_config->str_uni_drc_config_ext;
  ia_drc_coefficients_uni_drc_struct *pstr_drc_coefficients_uni_drc =
      &pstr_uni_drc_config->str_drc_coefficients_uni_drc[0];
  ia_drc_coefficients_uni_drc_struct *pstr_drc_coefficients_uni_drc_v1 =
      &pstr_uni_drc_config_ext->str_drc_coefficients_uni_drc_v1[0];

  if (pstr_uni_drc_config_ext->drc_coefficients_uni_drc_v1_count <= 0) {
    WORD32 all_band_gain_count = 0;
    WORD32 gain_set_count = pstr_drc_coefficients_uni_drc->gain_set_count;
    for (i = 0; i < gain_set_count; i++) {
      all_band_gain_count += pstr_drc_coefficients_uni_drc->str_gain_set_params[i].band_count;
    }
    pstr_gain_enc->n_sequences = all_band_gain_count;
  } else {
    for (i = 0; i < pstr_uni_drc_config_ext->drc_coefficients_uni_drc_v1_count; i++) {
      WORD32 all_band_gain_count = 0;
      for (j = 0; j < pstr_drc_coefficients_uni_drc_v1[i].gain_set_count; j++) {
        all_band_gain_count +=
            pstr_drc_coefficients_uni_drc_v1[i].str_gain_set_params[j].band_count;
      }
      pstr_drc_coefficients_uni_drc_v1[i].gain_sequence_count = all_band_gain_count;
      pstr_gain_enc->n_sequences += all_band_gain_count;
    }
  }

  if (pstr_gain_enc->n_sequences > IMPD_DRCMAX_NSEQ) {
    return IA_EXHEAACE_CONFIG_FATAL_DRC_PARAM_OUT_OF_RANGE;
  }

  if ((pstr_uni_drc_config_ext->drc_coefficients_uni_drc_v1_count > 0) &&
      (pstr_drc_coefficients_uni_drc_v1->drc_frame_size_present)) {
    pstr_gain_enc->drc_frame_size = pstr_drc_coefficients_uni_drc_v1->drc_frame_size;
  } else if ((pstr_uni_drc_config->drc_coefficients_uni_drc_count > 0) &&
             (pstr_drc_coefficients_uni_drc->drc_frame_size_present)) {
    pstr_gain_enc->drc_frame_size = pstr_drc_coefficients_uni_drc->drc_frame_size;
  } else {
    pstr_gain_enc->drc_frame_size = frame_size;
  }

  if (pstr_gain_enc->drc_frame_size > IMPD_DRCMAX_FRAMESIZE) {
    return IA_EXHEAACE_CONFIG_FATAL_DRC_PARAM_OUT_OF_RANGE;
  }
  if (pstr_gain_enc->drc_frame_size < 1) {
    return IA_EXHEAACE_CONFIG_FATAL_DRC_INVALID_CONFIG;
  }

  if (!pstr_uni_drc_config->sample_rate_present) {
    pstr_gain_enc->sample_rate = sample_rate;
  } else {
    pstr_gain_enc->sample_rate = pstr_uni_drc_config->sample_rate;
  }

  pstr_gain_enc->domain = domain;
  pstr_gain_enc->delay_mode = delay_mode;
  pstr_gain_enc->delta_tmin_default = impd_drc_get_delta_t_min(pstr_gain_enc->sample_rate);

  if ((pstr_uni_drc_config_ext->drc_coefficients_uni_drc_v1_count > 0) &&
      (pstr_drc_coefficients_uni_drc_v1->str_gain_set_params[0].time_delta_min_present == 1)) {
    pstr_gain_enc->delta_tmin =
        pstr_drc_coefficients_uni_drc_v1->str_gain_set_params[0].delta_tmin;
  } else if ((pstr_uni_drc_config->drc_coefficients_uni_drc_count > 0) &&
             (pstr_drc_coefficients_uni_drc->str_gain_set_params[0].time_delta_min_present ==
              1)) {
    pstr_gain_enc->delta_tmin = pstr_drc_coefficients_uni_drc->str_gain_set_params[0].delta_tmin;
  } else {
    pstr_gain_enc->delta_tmin = impd_drc_get_delta_t_min(pstr_gain_enc->sample_rate);
  }

  num_gain_values_max = pstr_gain_enc->drc_frame_size / pstr_gain_enc->delta_tmin;
  pstr_gain_enc->base_ch_count = pstr_uni_drc_config->str_channel_layout.base_ch_count;

  memcpy(&pstr_gain_enc->str_uni_drc_config, pstr_uni_drc_config,
         sizeof(ia_drc_uni_drc_config_struct));
  memcpy(&pstr_gain_enc->str_loudness_info_set, pstr_loudness_info_set,
         sizeof(ia_drc_loudness_info_set_struct));

  k = 0;
  if (pstr_uni_drc_config->drc_coefficients_uni_drc_count > 0) {
    for (j = 0; j < pstr_drc_coefficients_uni_drc->gain_set_count; j++) {
      ch_idx = 0;
      found_ch_idx = 0;
      ia_drc_gain_set_params_struct *pstr_gain_set_params =
          &pstr_drc_coefficients_uni_drc->str_gain_set_params[j];

      for (m = 0; m < pstr_uni_drc_config->drc_instructions_uni_drc_count; m++) {
        if (pstr_uni_drc_config->str_drc_instructions_uni_drc[m].drc_location ==
            pstr_drc_coefficients_uni_drc->drc_location) {
          for (ch = 0; ch < MAX_CHANNEL_COUNT; ch++) {
            if (pstr_uni_drc_config->str_drc_instructions_uni_drc[m].gain_set_index[ch] == j) {
              ch_idx = ch;
              found_ch_idx = 1;
              break;
            }
          }
        }
        if (found_ch_idx) {
          break;
        }
      }
      if (ch_idx >= (UWORD32)pstr_gain_enc->base_ch_count) {
        return IA_EXHEAACE_INIT_FATAL_DRC_INVALID_CHANNEL_INDEX;
      }
      if (pstr_gain_set_params->band_count > 1) {
        impd_drc_util_stft_read_gain_config(pstr_gain_enc->str_drc_stft_gain_handle[0][j],
                                            pstr_gain_set_params->band_count,
                                            pstr_gain_set_params);

        for (l = 0; l < pstr_gain_set_params->band_count; l++) {
          err_code = impd_drc_stft_drc_gain_calc_init(pstr_gain_enc, 0, j, l);
          if (err_code) {
            return err_code;
          }
          pstr_gain_enc->str_drc_stft_gain_handle[0][j][l].ch_idx = ch_idx;
          pstr_gain_enc->str_drc_stft_gain_handle[0][j][l].is_valid = 1;
        }
      } else if (pstr_gain_set_params->band_count == 1) {
        impd_drc_util_td_read_gain_config(&pstr_gain_enc->str_drc_compand[0][j],
                                          pstr_gain_set_params);

        pstr_gain_enc->str_drc_compand[0][j].initial_volume = 0.0f;

        err_code = impd_drc_td_drc_gain_calc_init(pstr_gain_enc, 0, j);
        if (err_code) {
          return err_code;
        }
        pstr_gain_enc->str_drc_compand[0][j].ch_idx = ch_idx;
        pstr_gain_enc->str_drc_compand[0][j].is_valid = 1;
      }

      for (l = 0; l < pstr_gain_set_params->band_count; l++) {
        pstr_gain_enc->str_drc_gain_seq_buf[k].str_drc_group.n_gain_values = 1;
        pstr_gain_enc->str_drc_gain_seq_buf[k].str_gain_set_params =
            pstr_drc_coefficients_uni_drc->str_gain_set_params[j];
        k++;
      }
    }
  }
  k = 0;
  for (i = 0; i < pstr_uni_drc_config_ext->drc_coefficients_uni_drc_v1_count; i++) {
    pstr_drc_coefficients_uni_drc_v1 =
        &pstr_uni_drc_config_ext->str_drc_coefficients_uni_drc_v1[i];
    for (j = 0; j < pstr_drc_coefficients_uni_drc_v1->gain_set_count; j++) {
      ch_idx = 0;
      found_ch_idx = 0;
      ia_drc_gain_set_params_struct *pstr_gain_set_params =
          &pstr_drc_coefficients_uni_drc_v1->str_gain_set_params[j];

      for (m = 0; m < pstr_uni_drc_config_ext->drc_instructions_uni_drc_v1_count; m++) {
        if (pstr_uni_drc_config_ext->str_drc_instructions_uni_drc_v1[m].drc_location ==
            pstr_drc_coefficients_uni_drc_v1->drc_location) {
          for (ch = 0; ch < MAX_CHANNEL_COUNT; ch++) {
            if (pstr_uni_drc_config_ext->str_drc_instructions_uni_drc_v1[m].gain_set_index[ch] ==
                j) {
              ch_idx = ch;
              found_ch_idx = 1;
              break;
            }
          }
        }
        if (found_ch_idx) {
          break;
        }
      }

      if (ch_idx >= (UWORD32)pstr_gain_enc->base_ch_count) {
        return IA_EXHEAACE_INIT_FATAL_DRC_INVALID_CHANNEL_INDEX;
      }

      if (pstr_gain_set_params->band_count > 1) {
        impd_drc_util_stft_read_gain_config(pstr_gain_enc->str_drc_stft_gain_handle[i][j],
                                            pstr_gain_set_params->band_count,
                                            pstr_gain_set_params);

        for (l = 0; l < pstr_gain_set_params->band_count; l++) {
          err_code = impd_drc_stft_drc_gain_calc_init(pstr_gain_enc, i, j, l);
          if (err_code) {
            return err_code;
          }
          pstr_gain_enc->str_drc_stft_gain_handle[i][j][l].ch_idx = ch_idx;
          pstr_gain_enc->str_drc_stft_gain_handle[i][j][l].is_valid = 1;
        }
      } else if (pstr_gain_set_params->band_count == 1) {
        impd_drc_util_td_read_gain_config(&pstr_gain_enc->str_drc_compand[i][j],
                                          pstr_gain_set_params);

        pstr_gain_enc->str_drc_compand[i][j].initial_volume = 0.0f;

        err_code = impd_drc_td_drc_gain_calc_init(pstr_gain_enc, i, j);
        if (err_code) {
          return err_code;
        }
        pstr_gain_enc->str_drc_compand[i][j].ch_idx = ch_idx;
        pstr_gain_enc->str_drc_compand[i][j].is_valid = 1;
      }

      for (l = 0; l < pstr_gain_set_params->band_count; l++) {
        pstr_gain_enc->str_drc_gain_seq_buf[k].str_drc_group.n_gain_values = 1;
        pstr_gain_enc->str_drc_gain_seq_buf[k].str_gain_set_params =
            pstr_drc_coefficients_uni_drc_v1->str_gain_set_params[j];
        k++;
      }
    }
  }

  impd_drc_generate_delta_time_code_table(num_gain_values_max,
                                          pstr_gain_enc->str_delta_time_code_table);

  for (i = num_gain_values_max - 1; i >= 0; i--) {
    pstr_gain_enc->delta_time_quant_table[i] = pstr_gain_enc->delta_tmin * (i + 1);
  }

  return err_code;
}

IA_ERRORCODE impd_drc_encode_uni_drc_gain(ia_drc_gain_enc_struct *pstr_gain_enc,
                                          FLOAT32 *ptr_gain_buffer, VOID *pstr_scratch) {
  LOOPIDX idx;
  IA_ERRORCODE err_code = IA_NO_ERROR;
  for (idx = 0; idx < pstr_gain_enc->n_sequences; idx++) {
    err_code = impd_drc_quantize_and_encode_drc_gain(
        pstr_gain_enc, &ptr_gain_buffer[idx * MAX_DRC_FRAME_SIZE],
        &(pstr_gain_enc->drc_gain_per_sample_with_prev_frame[idx][0]),
        pstr_gain_enc->str_delta_time_code_table, &(pstr_gain_enc->str_drc_gain_seq_buf[idx]),
        pstr_scratch);

    if (err_code) {
      return err_code;
    }
  }
  return err_code;
}

WORD32 impd_drc_get_delta_t_min(const WORD32 sample_rate) {
  WORD32 lower_bound;
  WORD32 result = 1;
  WORD32 sample_rate_local = sample_rate;

  if (sample_rate_local < 1000) {
    sample_rate_local = 1000;
  }
  lower_bound = (WORD32)((0.0005f * sample_rate_local) + 0.5f);

  while (result <= lower_bound) {
    result = result << 1;
  }
  return result;
}
