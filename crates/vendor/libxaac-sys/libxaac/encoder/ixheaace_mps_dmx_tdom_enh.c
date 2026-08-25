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
#include "ixheaac_type_def.h"
#include "ixheaac_constants.h"
#include "ixheaac_error_standards.h"
#include "ixheaace_error_codes.h"
#include "ixheaace_mps_common_fix.h"

#include "ixheaace_mps_common_define.h"
#include "ixheaace_mps_defines.h"
#include "ixheaac_constants.h"
#include "ixheaace_mps_dmx_tdom_enh.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaac_basic_ops.h"

static VOID ixheaace_mps_212_calculate_ratio(const FLOAT32 sqrt_lin_cld_m,
                                             const FLOAT32 lin_cld_m, const FLOAT32 icc_m,
                                             FLOAT32 g_m[2]) {
  if (icc_m >= 0.f) {
    g_m[0] = g_m[1] = 1.0f;
  } else {
    FLOAT32 max_gain_factor = 2.0f;
    FLOAT32 numerator, denominator;
    FLOAT32 q = 0.0f;
    numerator = (lin_cld_m + 0.5f) + (icc_m * sqrt_lin_cld_m);
    denominator = (lin_cld_m + 0.5f) - (icc_m * sqrt_lin_cld_m);

    if ((numerator > (0.f)) && (denominator > (0.f))) {
      FLOAT32 intermediate;
      intermediate = numerator / denominator;
      intermediate = (FLOAT32)sqrt(intermediate);
      intermediate = (FLOAT32)sqrt(intermediate);
      q = (intermediate >= max_gain_factor) ? max_gain_factor : intermediate;
    }

    g_m[0] = max_gain_factor - q;
    g_m[1] = q;
  }
}

static VOID ixheaace_mps_212_calculate_dmx_gains(const FLOAT32 lin_cld_m,
                                                 const FLOAT32 lin_cld_2_m, const FLOAT32 icc_m,
                                                 const FLOAT32 g_m[2], FLOAT32 h_1_m[2]) {
  const FLOAT32 max_gain_factor = 2.0f;
  FLOAT32 energy_right, energy_left, cross_energy, inverse_weight_num, inverse_weight_den,
      inverse_weight, inverse_weight_limited;
  energy_right = lin_cld_2_m - 0.5f;
  energy_right = (FLOAT32)sqrt(energy_right);
  energy_left = lin_cld_m * energy_right;
  cross_energy = (FLOAT32)sqrt(energy_left * energy_right);
  inverse_weight_num = energy_right + energy_left;
  inverse_weight_den = ((g_m[0] * g_m[0]) * energy_left) + ((g_m[1] * g_m[1]) * energy_right);

  inverse_weight_den = ((((g_m[0] * g_m[1]) * cross_energy) * icc_m) * 2) + inverse_weight_den;

  if (inverse_weight_den > (0.f)) {
    inverse_weight = inverse_weight_num / inverse_weight_den;

    inverse_weight = (FLOAT32)sqrt(inverse_weight);

    inverse_weight_limited =
        (inverse_weight >= max_gain_factor) ? max_gain_factor : inverse_weight;
  } else {
    inverse_weight_limited = max_gain_factor;
  }

  h_1_m[1] = g_m[1] * inverse_weight_limited;
  h_1_m[0] = g_m[0] * inverse_weight_limited;
}

IA_ERRORCODE ixheaace_mps_212_init_enhanced_time_domain_dmx(
    ixheaace_mps_pstr_enhanced_time_domain_dmx pstr_enhanced_time_domain_dmx,
    const FLOAT32 *const ptr_input_gain_m_flt, const FLOAT32 output_gain_m_flt,
    const WORD32 frame_length) {
  IA_ERRORCODE error = IA_NO_ERROR;
  WORD32 sample_idx;
  FLOAT32 delta;
  if (frame_length > pstr_enhanced_time_domain_dmx->max_frame_length) {
    return IA_EXHEAACE_INIT_FATAL_MPS_INIT_FAILED;
  }
  pstr_enhanced_time_domain_dmx->frame_length = frame_length;
  delta = (FLOAT32)(PI_FLT / (2.0 * pstr_enhanced_time_domain_dmx->frame_length));
  for (sample_idx = 0; sample_idx < pstr_enhanced_time_domain_dmx->frame_length + 1;
       sample_idx++) {
    FLOAT32 sin_val = (FLOAT32)sin(sample_idx * delta);
    pstr_enhanced_time_domain_dmx->sinus_window[sample_idx] = ALPHA * sin_val * sin_val;
  }

  pstr_enhanced_time_domain_dmx->prev_left_energy =
      pstr_enhanced_time_domain_dmx->prev_right_energy =
          pstr_enhanced_time_domain_dmx->prev_x_energy = 0.0f;
  pstr_enhanced_time_domain_dmx->cld_weight =
      (ptr_input_gain_m_flt[LEFT_CH]) / (ptr_input_gain_m_flt[RIGHT_CH]);
  pstr_enhanced_time_domain_dmx->gain_weight[LEFT_CH] =
      (ptr_input_gain_m_flt[LEFT_CH] * output_gain_m_flt);
  pstr_enhanced_time_domain_dmx->gain_weight[RIGHT_CH] =
      (ptr_input_gain_m_flt[RIGHT_CH] * output_gain_m_flt);
  pstr_enhanced_time_domain_dmx->prev_gain[LEFT_CH] =
      pstr_enhanced_time_domain_dmx->gain_weight[LEFT_CH];
  pstr_enhanced_time_domain_dmx->prev_gain[RIGHT_CH] =
      pstr_enhanced_time_domain_dmx->gain_weight[RIGHT_CH];
  pstr_enhanced_time_domain_dmx->prev_h1[LEFT_CH] =
      pstr_enhanced_time_domain_dmx->gain_weight[LEFT_CH];
  pstr_enhanced_time_domain_dmx->prev_h1[RIGHT_CH] =
      pstr_enhanced_time_domain_dmx->gain_weight[RIGHT_CH];
  return error;
}

IA_ERRORCODE ixheaace_mps_212_apply_enhanced_time_domain_dmx(
    ixheaace_mps_pstr_enhanced_time_domain_dmx pstr_enhanced_time_domain_dmx,
    FLOAT32 input_time[2][MPS_MAX_FRAME_LENGTH + MAX_DELAY_SURROUND_ANALYSIS],
    FLOAT32 *const ptr_output_time_dmx, const WORD32 input_delay) {
  IA_ERRORCODE error = IA_NO_ERROR;

  WORD32 sample_idx;
  FLOAT32 lin_bb_cld, lin_cld, corr, sqrt_lin_cld, g[2], h1[2], gain_left, gain_right;
  FLOAT32 prev_energy_left, prev_energy_right, prev_energy, energy_left, energy_right, energy_out;
  WORD32 granule_length = MIN(128, pstr_enhanced_time_domain_dmx->frame_length);

  sample_idx = 0;
  prev_energy_left = prev_energy_right = prev_energy = .5f;

  do {
    WORD32 offset = sample_idx;
    FLOAT32 partial_left, partial_right, partial_out;
    partial_left = partial_right = partial_out = 0.0;

    WORD32 value = MIN(offset + granule_length, pstr_enhanced_time_domain_dmx->frame_length);
    for (sample_idx = offset; sample_idx < value; sample_idx++) {
      FLOAT32 input_left = input_time[LEFT_CH][sample_idx];
      FLOAT32 input_right = input_time[RIGHT_CH][sample_idx];

      partial_left += (FLOAT32)pow(input_left, 2);
      partial_right += (FLOAT32)pow(input_right, 2);
      partial_out += input_left * input_right;
    }

    prev_energy_left = prev_energy_left + partial_left;
    prev_energy_right = prev_energy_right + partial_right;
    prev_energy = prev_energy + partial_out;

  } while (sample_idx < pstr_enhanced_time_domain_dmx->frame_length);

  energy_left = pstr_enhanced_time_domain_dmx->prev_left_energy + prev_energy_left;
  energy_right = pstr_enhanced_time_domain_dmx->prev_right_energy + prev_energy_right;
  energy_out = pstr_enhanced_time_domain_dmx->prev_x_energy + prev_energy;

  lin_bb_cld = pstr_enhanced_time_domain_dmx->cld_weight * (energy_left / energy_right);
  corr = energy_out * (1 / (FLOAT32)sqrt(energy_left * energy_right));

  pstr_enhanced_time_domain_dmx->prev_left_energy = prev_energy_left;
  pstr_enhanced_time_domain_dmx->prev_right_energy = prev_energy_right;
  pstr_enhanced_time_domain_dmx->prev_x_energy = prev_energy;

  lin_cld = (FLOAT32)sqrt(lin_bb_cld);
  sqrt_lin_cld = (FLOAT32)sqrt(lin_cld);

  ixheaace_mps_212_calculate_ratio(sqrt_lin_cld, lin_cld, corr, g);

  ixheaace_mps_212_calculate_dmx_gains(lin_cld, lin_bb_cld, corr, g, h1);

  h1[LEFT_CH] = h1[LEFT_CH] * pstr_enhanced_time_domain_dmx->gain_weight[LEFT_CH];
  h1[RIGHT_CH] = h1[RIGHT_CH] * pstr_enhanced_time_domain_dmx->gain_weight[RIGHT_CH];

  gain_left = pstr_enhanced_time_domain_dmx->prev_gain[LEFT_CH];
  gain_right = pstr_enhanced_time_domain_dmx->prev_gain[RIGHT_CH];

  for (sample_idx = 0; sample_idx < pstr_enhanced_time_domain_dmx->frame_length; sample_idx++) {
    WORD32 frame_length = pstr_enhanced_time_domain_dmx->frame_length;
    FLOAT32 intermediate_gain_left, intermediate_gain_right, temp;

    intermediate_gain_left =
        ((pstr_enhanced_time_domain_dmx->sinus_window[sample_idx] * h1[LEFT_CH]) +
         (pstr_enhanced_time_domain_dmx->sinus_window[frame_length - sample_idx] *
          pstr_enhanced_time_domain_dmx->prev_h1[LEFT_CH]));

    intermediate_gain_right =
        ((pstr_enhanced_time_domain_dmx->sinus_window[sample_idx] * h1[RIGHT_CH]) +
         (pstr_enhanced_time_domain_dmx->sinus_window[frame_length - sample_idx] *
          pstr_enhanced_time_domain_dmx->prev_h1[RIGHT_CH]));

    gain_left = intermediate_gain_left + ((1.f - ALPHA) * gain_left);
    gain_right = intermediate_gain_right + ((1.f - ALPHA) * gain_right);
    temp = (gain_left * (FLOAT32)input_time[LEFT_CH][sample_idx + input_delay]) +
           (gain_right * (FLOAT32)input_time[RIGHT_CH][sample_idx + input_delay]);
    ptr_output_time_dmx[sample_idx] = (FLOAT32)temp;
  }

  pstr_enhanced_time_domain_dmx->prev_gain[LEFT_CH] = gain_left;
  pstr_enhanced_time_domain_dmx->prev_gain[RIGHT_CH] = gain_right;
  pstr_enhanced_time_domain_dmx->prev_h1[LEFT_CH] = h1[LEFT_CH];
  pstr_enhanced_time_domain_dmx->prev_h1[RIGHT_CH] = h1[RIGHT_CH];

  return error;
}
