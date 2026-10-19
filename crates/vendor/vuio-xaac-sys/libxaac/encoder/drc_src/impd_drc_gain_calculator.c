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
#include <float.h>
#include "ixheaac_type_def.h"
#include "ixheaac_error_standards.h"
#include "ixheaace_error_codes.h"

#include "iusace_cnst.h"
#include "iusace_block_switch_const.h"
#include "iusace_bitbuffer.h"

#include "impd_drc_common_enc.h"
#include "impd_drc_uni_drc.h"
#include "impd_drc_tables.h"
#include "impd_drc_api.h"

#include "iusace_tns_usac.h"
#include "iusace_psy_mod.h"
#include "ixheaace_sbr_header.h"
#include "ixheaace_config.h"
#include "iusace_config.h"

#include "iusace_rom.h"
#include "iusace_fft.h"

#include "impd_drc_uni_drc_eq.h"
#include "impd_drc_uni_drc_filter_bank.h"
#include "impd_drc_gain_enc.h"
#include "impd_drc_struct_def.h"
#include "impd_drc_enc.h"
#include "ixheaace_common_utils.h"

static VOID impd_drc_compand_update_volume(ia_drc_compand_chan_param_struct *pstr_channel_param,
                                           FLOAT64 in_value) {
  FLOAT64 delta = in_value - pstr_channel_param->volume;

  if (delta <= 0.0) {
    pstr_channel_param->volume += delta * pstr_channel_param->decay;
  } else {
    pstr_channel_param->volume += delta * pstr_channel_param->attack;
  }
}

static FLOAT64 impd_drc_compand_get_volume(ia_drc_compand_struct *pstr_drc_compand,
                                           FLOAT64 in_lin) {
  ULOOPIDX idx;
  FLOAT64 in_log, out_log;
  ia_drc_compand_segment_struct *pstr_compand_segment;

  if (in_lin < pstr_drc_compand->in_min_lin) {
    return pstr_drc_compand->out_min_lin;
  }

  if (fabs(in_lin) <= FLT_EPSILON) {
    in_log = log(FLT_EPSILON);
  }
  else {
    in_log = log(fabs(in_lin));
  }

  for (idx = 1; idx < pstr_drc_compand->nb_segments; idx++) {
    if (in_log <= pstr_drc_compand->str_segment[idx].x) {
      break;
    }
  }

  pstr_compand_segment = &pstr_drc_compand->str_segment[idx - 1];
  in_log -= pstr_compand_segment->x;
  out_log = pstr_compand_segment->y +
            in_log * (pstr_compand_segment->a * in_log + pstr_compand_segment->b);

  return exp(out_log);
}

VOID impd_drc_td_drc_gain_calc_process(ia_drc_gain_enc_struct *pstr_drc_gain_enc,
                                       WORD32 drc_coefficients_uni_drc_idx, WORD32 gain_set_idx,
                                       WORD32 num_samples, FLOAT32 *in_buff, FLOAT32 *out_buff) {
  LOOPIDX idx;
  FLOAT64 gain;
  ia_drc_compand_chan_param_struct *pstr_channel_param;
  ia_drc_compand_struct *pstr_drc_compand =
      &pstr_drc_gain_enc->str_drc_compand[drc_coefficients_uni_drc_idx][gain_set_idx];

  pstr_channel_param = &pstr_drc_compand->str_channel_param;

  for (idx = 0; idx < num_samples; idx++) {
    impd_drc_compand_update_volume(pstr_channel_param, fabs((FLOAT64)in_buff[idx] / 32768.0));

    gain = impd_drc_compand_get_volume(pstr_drc_compand, pstr_channel_param->volume);
    out_buff[idx] = (FLOAT32)(20.0 * log10(gain));
  }
}

IA_ERRORCODE impd_drc_td_drc_gain_calc_init(ia_drc_gain_enc_struct *pstr_drc_gain_enc,
                                            WORD32 drc_coefficients_uni_drc_idx,
                                            WORD32 gain_set_idx) {
  ULOOPIDX i, j;
  UWORD32 num_points;
  FLOAT64 g1, g2;
  FLOAT64 x, y, cx, cy, r;
  FLOAT64 inp_1, inp_2, out_1, out_2, theta, length, radius;
  ia_drc_compand_struct *pstr_drc_compand;
  ia_drc_compand_chan_param_struct *pstr_chan_param;

  if ((drc_coefficients_uni_drc_idx >= MAX_DRC_COEFF_COUNT) ||
      (gain_set_idx >= GAIN_SET_COUNT_MAX)) {
    return IA_EXHEAACE_CONFIG_FATAL_DRC_COMPAND_FAILED;
  }

  pstr_drc_compand =
      &pstr_drc_gain_enc->str_drc_compand[drc_coefficients_uni_drc_idx][gain_set_idx];

  for (i = 0; i < pstr_drc_compand->nb_points; i++) {
    if (i && pstr_drc_compand->str_segment[2 * ((i - 1) + 1)].x >
                 pstr_drc_compand->str_segment[2 * ((i) + 1)].x) {
      return IA_EXHEAACE_CONFIG_FATAL_DRC_COMPAND_FAILED;
    }
    pstr_drc_compand->str_segment[2 * (i + 1)].y -= pstr_drc_compand->str_segment[2 * (i + 1)].x;
  }
  num_points = pstr_drc_compand->nb_points;

  if (num_points == 0 || pstr_drc_compand->str_segment[2 * ((num_points - 1) + 1)].x) {
    num_points++;
  }

  pstr_drc_compand->str_segment[0].x =
      pstr_drc_compand->str_segment[2].x - 2 * pstr_drc_compand->width_db;
  pstr_drc_compand->str_segment[0].y = pstr_drc_compand->str_segment[2].y;
  num_points++;

  radius = pstr_drc_compand->width_db * M_LN10_DIV_20;

  for (i = 2; i < num_points; i++) {
    g1 = (pstr_drc_compand->str_segment[2 * (i - 1)].y -
          pstr_drc_compand->str_segment[2 * (i - 2)].y) *
         (pstr_drc_compand->str_segment[2 * i].x - pstr_drc_compand->str_segment[2 * (i - 1)].x);
    g2 = (pstr_drc_compand->str_segment[2 * i].y - pstr_drc_compand->str_segment[2 * (i - 1)].y) *
         (pstr_drc_compand->str_segment[2 * (i - 1)].x -
          pstr_drc_compand->str_segment[2 * (i - 2)].x);

    if (fabs(g1 - g2)) {
      continue;
    }
    num_points--;

    for (j = --i; j < num_points; j++) {
      pstr_drc_compand->str_segment[2 * j] = pstr_drc_compand->str_segment[2 * (j + 1)];
    }
  }

  pstr_drc_compand->nb_segments = num_points * 2;
  for (i = 0; i < pstr_drc_compand->nb_segments; i += 2) {
    pstr_drc_compand->str_segment[i].y += pstr_drc_compand->gain_db;
    pstr_drc_compand->str_segment[i].x *= M_LN10_DIV_20;
    pstr_drc_compand->str_segment[i].y *= M_LN10_DIV_20;
  }

  for (i = 4; i < pstr_drc_compand->nb_segments; i += 2) {
    FLOAT64 num = 0.0;
    FLOAT64 den = 0.0f;

    num = pstr_drc_compand->str_segment[i - 2].y - pstr_drc_compand->str_segment[i - 4].y;
    den = pstr_drc_compand->str_segment[i - 2].x - pstr_drc_compand->str_segment[i - 4].x;
    length = hypot(num, den);
    if (length < FLT_EPSILON) {
      return IA_EXHEAACE_EXE_NONFATAL_USAC_INVALID_GAIN_POINTS;
    }
    pstr_drc_compand->str_segment[i - 4].a = 0;
    pstr_drc_compand->str_segment[i - 4].b = ixheaace_div64(num, den);
    theta = atan2(num, den);
    r = MIN(radius, length);
    pstr_drc_compand->str_segment[i - 3].x =
        pstr_drc_compand->str_segment[i - 2].x - r * cos(theta);
    pstr_drc_compand->str_segment[i - 3].y =
        pstr_drc_compand->str_segment[i - 2].y - r * sin(theta);

    num = pstr_drc_compand->str_segment[i].y - pstr_drc_compand->str_segment[i - 2].y;
    den = pstr_drc_compand->str_segment[i].x - pstr_drc_compand->str_segment[i - 2].x;
    length = hypot(num, den);
    if (length < FLT_EPSILON) {
      return IA_EXHEAACE_EXE_NONFATAL_USAC_INVALID_GAIN_POINTS;
    }
    pstr_drc_compand->str_segment[i - 2].a = 0;
    pstr_drc_compand->str_segment[i - 2].b = ixheaace_div64(num, den);
    theta = atan2(num, den);
    r = MIN(radius, length / 2);
    x = pstr_drc_compand->str_segment[i - 2].x + r * cos(theta);
    y = pstr_drc_compand->str_segment[i - 2].y + r * sin(theta);

    cx =
        (pstr_drc_compand->str_segment[i - 3].x + pstr_drc_compand->str_segment[i - 2].x + x) / 3;
    cy =
        (pstr_drc_compand->str_segment[i - 3].y + pstr_drc_compand->str_segment[i - 2].y + y) / 3;

    pstr_drc_compand->str_segment[i - 2].x = x;
    pstr_drc_compand->str_segment[i - 2].y = y;

    inp_1 = cx - pstr_drc_compand->str_segment[i - 3].x;
    out_1 = cy - pstr_drc_compand->str_segment[i - 3].y;
    inp_2 = pstr_drc_compand->str_segment[i - 2].x - pstr_drc_compand->str_segment[i - 3].x;
    out_2 = pstr_drc_compand->str_segment[i - 2].y - pstr_drc_compand->str_segment[i - 3].y;

    num = (out_2 * inp_1) - (inp_2 * out_1);
    den = (inp_2 - inp_1) * inp_1 * inp_2;
    pstr_drc_compand->str_segment[i - 3].a = ixheaace_div64(num, den);

    num = out_1 - (pstr_drc_compand->str_segment[i - 3].a * inp_1 * inp_1);
    den = inp_1;
    pstr_drc_compand->str_segment[i - 3].b = ixheaace_div64(num, den);
  }
  pstr_drc_compand->str_segment[i - 3].x = 0;
  pstr_drc_compand->str_segment[i - 3].y = pstr_drc_compand->str_segment[i - 3].y;

  pstr_drc_compand->in_min_lin = exp(pstr_drc_compand->str_segment[1].x);
  pstr_drc_compand->out_min_lin = exp(pstr_drc_compand->str_segment[1].y);

  pstr_chan_param = &pstr_drc_compand->str_channel_param;

  if (pstr_chan_param->attack < 1.0 / pstr_drc_gain_enc->sample_rate) {
    pstr_chan_param->attack = 1.0;
  } else {
    pstr_chan_param->attack =
        1.0 - exp(-1.0 / (pstr_drc_gain_enc->sample_rate * pstr_chan_param->attack));
  }

  if (pstr_chan_param->decay < 1.0 / pstr_drc_gain_enc->sample_rate) {
    pstr_chan_param->decay = 1.0;
  } else {
    pstr_chan_param->decay =
        1.0 - exp(-1.0 / (pstr_drc_gain_enc->sample_rate * pstr_chan_param->decay));
  }
  pstr_chan_param->volume = EXP10(pstr_drc_compand->initial_volume / 20);

  return IA_NO_ERROR;
}

static FLOAT32 impd_drc_stft_drc_compand_get_volume(
    ia_drc_stft_gain_calc_struct *pstr_drc_stft_gain_handle, FLOAT32 in_db) {
  ULOOPIDX idx;
  FLOAT32 in_log, out_log;
  ia_drc_compand_segment_struct *pstr_compand_segment;

  if (in_db < pstr_drc_stft_gain_handle->in_min_db) {
    return pstr_drc_stft_gain_handle->out_min_db;
  }

  in_log = (FLOAT32)(in_db * M_LN10_DIV_20);

  for (idx = 1; idx < pstr_drc_stft_gain_handle->nb_segments; idx++) {
    if (in_log <= pstr_drc_stft_gain_handle->str_segment[idx].x) {
      break;
    }
  }

  pstr_compand_segment = &pstr_drc_stft_gain_handle->str_segment[idx - 1];
  in_log -= (FLOAT32)(pstr_compand_segment->x);
  out_log = (FLOAT32)(pstr_compand_segment->y +
                      in_log * (pstr_compand_segment->a * in_log + pstr_compand_segment->b));

  return (FLOAT32)(out_log * M_LOG10_E * 20.0f);
}

VOID impd_drc_stft_drc_gain_calc_process(ia_drc_gain_enc_struct *pstr_drc_gain_enc,
                                         WORD32 drc_coefficients_uni_drc_idx, WORD32 gain_set_idx,
                                         WORD32 band_idx, WORD32 start_sub_band_index,
                                         WORD32 stop_sub_band_index, UWORD32 num_frames,
                                         FLOAT32 *in_buff, FLOAT32 *gain_values) {
  ULOOPIDX idx;
  LOOPIDX band;
  FLOAT32 xg, xl, yl, cdb;
  FLOAT32 in_real, in_imag;
  FLOAT32 abs_val_sqr;
  UWORD32 num_time_slot = num_frames / STFT256_HOP_SIZE;

  ia_drc_stft_gain_calc_struct *pstr_drc_stft_gain_handle =
      &pstr_drc_gain_enc
           ->str_drc_stft_gain_handle[drc_coefficients_uni_drc_idx][gain_set_idx][band_idx];

  for (idx = 0; idx < num_time_slot; idx++) {
    abs_val_sqr = 0.0f;
    for (band = start_sub_band_index; band <= stop_sub_band_index; band++) {
      in_imag = in_buff[((idx * STFT256_HOP_SIZE + band) << 1) + 1];
      in_real = in_buff[(idx * STFT256_HOP_SIZE + band) << 1];

      abs_val_sqr += sqrtf(powf(in_real, 2.0f) + powf(in_imag, 2.0f));
    }

    abs_val_sqr /= (FLOAT32)((stop_sub_band_index - start_sub_band_index + 1) << 4);

    abs_val_sqr = powf(abs_val_sqr, 2.0f);
    xg = 10.0f * log10f((abs_val_sqr) + 2e-13f);

    xl = -impd_drc_stft_drc_compand_get_volume(pstr_drc_stft_gain_handle, xg);

    if (xl > pstr_drc_stft_gain_handle->yl_z1[band]) {
      yl = (pstr_drc_stft_gain_handle->alpha_a * pstr_drc_stft_gain_handle->yl_z1[band]) +
           ((1.0f - pstr_drc_stft_gain_handle->alpha_a) * xl);
    } else {
      yl = (pstr_drc_stft_gain_handle->alpha_r * pstr_drc_stft_gain_handle->yl_z1[band]) +
           ((1.0f - pstr_drc_stft_gain_handle->alpha_r) * xl);
    }

    pstr_drc_stft_gain_handle->yl_z1[band] = yl;
    cdb = -yl;
    cdb = MAX(IMPD_DRCSPECTRAL_FLOOR, (powf(10.0f, cdb / 20.0f)));
    cdb = 20.0f * log10f(cdb);

    for (band = 0; band < STFT256_HOP_SIZE; band++) {
      gain_values[idx * STFT256_HOP_SIZE + band] = cdb;
    }
  }
}

IA_ERRORCODE impd_drc_stft_drc_gain_calc_init(ia_drc_gain_enc_struct *pstr_drc_gain_enc,
                                              WORD32 drc_coefficients_uni_drc_idx,
                                              WORD32 gain_set_idx, WORD32 band_idx) {
  ULOOPIDX i, j;
  UWORD32 num_points;
  FLOAT32 width_e, tmp;
  FLOAT64 g1, g2;
  FLOAT64 x, y, cx, cy, r;
  FLOAT64 inp_1, inp_2, out_1, out_2, theta, len;
  ia_drc_compand_chan_param_struct *pstr_chan_param;
  ia_drc_stft_gain_calc_struct *pstr_drc_stft_gain_handle;

  if ((drc_coefficients_uni_drc_idx >= MAX_DRC_COEFF_COUNT) ||
      (gain_set_idx >= GAIN_SET_COUNT_MAX) || (band_idx >= MAX_BAND_COUNT)) {
    return IA_EXHEAACE_CONFIG_FATAL_DRC_COMPAND_FAILED;
  }

  pstr_drc_stft_gain_handle =
      &pstr_drc_gain_enc
           ->str_drc_stft_gain_handle[drc_coefficients_uni_drc_idx][gain_set_idx][band_idx];

  width_e = (FLOAT32)(pstr_drc_stft_gain_handle->width_db * M_LN10_DIV_20);

  for (i = 0; i < pstr_drc_stft_gain_handle->nb_points; i++) {
    if (i && pstr_drc_stft_gain_handle->str_segment[2 * ((i - 1) + 1)].x >
                 pstr_drc_stft_gain_handle->str_segment[2 * (i + 1)].x) {
      return IA_EXHEAACE_CONFIG_FATAL_DRC_COMPAND_FAILED;
    }
    pstr_drc_stft_gain_handle->str_segment[2 * (i + 1)].y -=
        pstr_drc_stft_gain_handle->str_segment[2 * (i + 1)].x;
  }
  num_points = pstr_drc_stft_gain_handle->nb_points;

  if (num_points == 0 || pstr_drc_stft_gain_handle->str_segment[2 * ((num_points - 1) + 1)].x) {
    num_points++;
  }

  pstr_drc_stft_gain_handle->str_segment[0].x =
      pstr_drc_stft_gain_handle->str_segment[2].x - pstr_drc_stft_gain_handle->width_db;
  pstr_drc_stft_gain_handle->str_segment[0].y = pstr_drc_stft_gain_handle->str_segment[2].y;
  num_points++;

  for (i = 2; i < num_points; i++) {
    g1 = (pstr_drc_stft_gain_handle->str_segment[2 * (i - 1)].y -
          pstr_drc_stft_gain_handle->str_segment[2 * (i - 2)].y) *
         (pstr_drc_stft_gain_handle->str_segment[2 * i].x -
          pstr_drc_stft_gain_handle->str_segment[2 * (i - 1)].x);
    g2 = (pstr_drc_stft_gain_handle->str_segment[2 * i].y -
          pstr_drc_stft_gain_handle->str_segment[2 * (i - 1)].y) *
         (pstr_drc_stft_gain_handle->str_segment[2 * (i - 1)].x -
          pstr_drc_stft_gain_handle->str_segment[2 * (i - 2)].x);

    if (fabs(g1 - g2)) {
      continue;
    }
    num_points--;

    for (j = --i; j < num_points; j++) {
      pstr_drc_stft_gain_handle->str_segment[2 * j] =
          pstr_drc_stft_gain_handle->str_segment[2 * (j + 1)];
    }
  }
  pstr_drc_stft_gain_handle->nb_segments = num_points * 2;
  for (i = 0; i < pstr_drc_stft_gain_handle->nb_segments; i += 2) {
    pstr_drc_stft_gain_handle->str_segment[i].y += pstr_drc_stft_gain_handle->gain_db;
    pstr_drc_stft_gain_handle->str_segment[i].x *= M_LN10_DIV_20;
    pstr_drc_stft_gain_handle->str_segment[i].y *= M_LN10_DIV_20;
  }

  for (i = 4; i < pstr_drc_stft_gain_handle->nb_segments; i += 2) {
    FLOAT64 denominator;
    FLOAT64 numerator;

    denominator = pstr_drc_stft_gain_handle->str_segment[i - 2].x -
                  pstr_drc_stft_gain_handle->str_segment[i - 4].x;
    numerator = pstr_drc_stft_gain_handle->str_segment[i - 2].y -
                pstr_drc_stft_gain_handle->str_segment[i - 4].y;
    len = hypot(denominator , numerator);
    if (len < FLT_EPSILON) {
      return IA_EXHEAACE_EXE_NONFATAL_USAC_INVALID_GAIN_POINTS;
    }
    pstr_drc_stft_gain_handle->str_segment[i - 4].a = 0;
    pstr_drc_stft_gain_handle->str_segment[i - 4].b = ixheaace_div64(numerator, denominator);
    theta = atan2(numerator, denominator);
    r = MIN(width_e / (2.0f * cos(theta)), len / 2);

    pstr_drc_stft_gain_handle->str_segment[i - 3].x =
      pstr_drc_stft_gain_handle->str_segment[i - 2].x - r * cos(theta);
    pstr_drc_stft_gain_handle->str_segment[i - 3].y =
      pstr_drc_stft_gain_handle->str_segment[i - 2].y - r * sin(theta);

    denominator = pstr_drc_stft_gain_handle->str_segment[i].x -
                  pstr_drc_stft_gain_handle->str_segment[i - 2].x;
    numerator = pstr_drc_stft_gain_handle->str_segment[i].y -
                pstr_drc_stft_gain_handle->str_segment[i - 2].y;
    len = hypot(denominator, numerator);
    if (len < FLT_EPSILON) {
      return IA_EXHEAACE_EXE_NONFATAL_USAC_INVALID_GAIN_POINTS;
    }
    pstr_drc_stft_gain_handle->str_segment[i - 2].a = 0;
    pstr_drc_stft_gain_handle->str_segment[i - 2].b = ixheaace_div64(numerator, denominator);

    theta = atan2(numerator, denominator);
    r = MIN(width_e / (2.0f * cos(theta)), len / 2);
    x = pstr_drc_stft_gain_handle->str_segment[i - 2].x + r * cos(theta);
    y = pstr_drc_stft_gain_handle->str_segment[i - 2].y + r * sin(theta);

    cx = (pstr_drc_stft_gain_handle->str_segment[i - 3].x +
          pstr_drc_stft_gain_handle->str_segment[i - 2].x + x) /
         3;
    cy = (pstr_drc_stft_gain_handle->str_segment[i - 3].y +
          pstr_drc_stft_gain_handle->str_segment[i - 2].y + y) /
         3;

    pstr_drc_stft_gain_handle->str_segment[i - 2].x = x;
    pstr_drc_stft_gain_handle->str_segment[i - 2].y = y;

    inp_1 = cx - pstr_drc_stft_gain_handle->str_segment[i - 3].x;
    out_1 = cy - pstr_drc_stft_gain_handle->str_segment[i - 3].y;
    inp_2 = pstr_drc_stft_gain_handle->str_segment[i - 2].x -
            pstr_drc_stft_gain_handle->str_segment[i - 3].x;
    out_2 = pstr_drc_stft_gain_handle->str_segment[i - 2].y -
            pstr_drc_stft_gain_handle->str_segment[i - 3].y;
    numerator = (out_2 * inp_1) - (inp_2 * out_1);
    denominator = (inp_2 - inp_1) * inp_2 * inp_1;
    pstr_drc_stft_gain_handle->str_segment[i - 3].a = ixheaace_div64(numerator, denominator);

    numerator = out_1 - (pstr_drc_stft_gain_handle->str_segment[i - 3].a * inp_1 * inp_1);
    denominator = inp_1;
    pstr_drc_stft_gain_handle->str_segment[i - 3].b = ixheaace_div64(numerator, denominator);
  }
  pstr_drc_stft_gain_handle->str_segment[i - 3].x = 0;
  pstr_drc_stft_gain_handle->str_segment[i - 3].y =
      pstr_drc_stft_gain_handle->str_segment[i - 2].y;

  pstr_drc_stft_gain_handle->in_min_db =
      (FLOAT32)(pstr_drc_stft_gain_handle->str_segment[1].x * M_LOG10_E * 20.0f);
  pstr_drc_stft_gain_handle->out_min_db =
      (FLOAT32)(pstr_drc_stft_gain_handle->str_segment[1].y * M_LOG10_E * 20.0f);

  pstr_chan_param = &pstr_drc_stft_gain_handle->str_channel_param;

  pstr_chan_param->volume = EXP10(pstr_drc_stft_gain_handle->initial_volume / 20.0f);

  for (i = 0; i < STFT256_HOP_SIZE; i++) {
    pstr_drc_stft_gain_handle->yl_z1[i] = 0.0f;
  }

  tmp = (pstr_drc_stft_gain_handle->attack_ms / STFT256_HOP_SIZE) *
      pstr_drc_gain_enc->sample_rate * 0.001f;
  if ((fabs(tmp) < FLT_EPSILON) && (tmp >= 0.0f)) {
    pstr_drc_stft_gain_handle->alpha_a = 0;
  }
  else {
    pstr_drc_stft_gain_handle->alpha_a = expf(ixheaace_div32(-1.0f, tmp));
  }

  tmp = (pstr_drc_stft_gain_handle->release_ms / STFT256_HOP_SIZE) *
      pstr_drc_gain_enc->sample_rate * 0.001f;
  if ((fabs(tmp) < FLT_EPSILON) && (tmp >= 0.0f)) {
    pstr_drc_stft_gain_handle->alpha_r = 0;
  }
  else {
    pstr_drc_stft_gain_handle->alpha_r = expf(ixheaace_div32(-1.0f, tmp));
  }

  return IA_NO_ERROR;
}

VOID impd_drc_stft_drc_convert_to_fd(ia_drc_gain_enc_struct *pstr_drc_gain_enc,
                                     FLOAT32 *ptr_input, UWORD32 ch_idx, UWORD32 frame_size,
                                     FLOAT32 *ptr_output, VOID *pstr_scratch) {
  ULOOPIDX i, j;
  UWORD32 num_time_slot = frame_size / STFT256_HOP_SIZE;
  FLOAT32 time_sample_vector;
  iusace_scratch_mem *ptr_scratch = (iusace_scratch_mem *)(pstr_scratch);
  pFLOAT32 scratch_buff = ptr_scratch->ptr_drc_scratch_buf;

  for (i = 0; i < num_time_slot; i++) {
    for (j = 0; j < STFT256_HOP_SIZE; j++) {
      time_sample_vector = (FLOAT32)(ptr_input[i * STFT256_HOP_SIZE + j] / (32768.0));

      scratch_buff[(j << 1)] =
          (FLOAT32)(pstr_drc_gain_enc->stft_tmp_in_buf_time[ch_idx][j] * iusace_sine_win_256[j]);
      scratch_buff[(j << 1) + 1] = 0.0f;

      scratch_buff[(STFT256_HOP_SIZE + j) << 1] =
          (FLOAT32)(iusace_sine_win_256[STFT256_HOP_SIZE - 1 - j] * time_sample_vector);
      scratch_buff[((STFT256_HOP_SIZE + j) << 1) + 1] = 0.0f;

      pstr_drc_gain_enc->stft_tmp_in_buf_time[ch_idx][j] = time_sample_vector;
    }

    iusace_complex_fft(scratch_buff, STFT256_HOP_SIZE << 1, ptr_scratch);

    ptr_output[(i * STFT256_HOP_SIZE) << 1] = scratch_buff[0];
    ptr_output[((i * STFT256_HOP_SIZE) << 1) + 1] = scratch_buff[STFT256_HOP_SIZE << 1];

    for (j = 1; j < STFT256_HOP_SIZE; j++) {
      ptr_output[(i * STFT256_HOP_SIZE + j) << 1] = scratch_buff[j << 1];
      ptr_output[((i * STFT256_HOP_SIZE + j) << 1) + 1] = scratch_buff[(j << 1) + 1];
    }
  }
}
