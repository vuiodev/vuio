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
#include <string.h>
#include "impd_type_def.h"
#include "impd_drc_extr_delta_coded_info.h"
#include "impd_drc_common.h"
#include "impd_drc_struct.h"
#include "impd_drc_interface.h"
#include "impd_drc_filter_bank.h"
#include "impd_drc_gain_dec.h"
#include "impd_parametric_drc_dec.h"
#include "impd_drc_multi_band.h"
#include "impd_drc_process_audio.h"
#include "impd_drc_eq.h"
#include "impd_drc_gain_decoder.h"

extern const ia_cicp_sigmoid_characteristic_param_struct
    pstr_cicp_sigmoid_characteristic_param[];

VOID impd_gain_db_to_lin(ia_interp_params_struct* interp_params_str,
                         WORD32 drc_band, FLOAT32 in_param_db_gain,
                         FLOAT32 in_param_db_slope, FLOAT32* out_param_lin_gain,
                         FLOAT32* out_param_lin_slope) {
  FLOAT32 loc_db_gain = in_param_db_gain;
  FLOAT32 gain_ratio = 1.0;

  ia_gain_modifiers_struct* pstr_gain_modifiers =
      interp_params_str->pstr_gain_modifiers;
  if (interp_params_str->gain_modification_flag) {
    if ((interp_params_str->characteristic_index > 0) &&
        (loc_db_gain != 0.0f)) {
      gain_ratio = 1.0f;
    }

    if (loc_db_gain < 0.0f) {
      gain_ratio *= interp_params_str->compress;
    } else {
      gain_ratio *= interp_params_str->boost;
    }
  }
  if (pstr_gain_modifiers->gain_scaling_flag[drc_band] == 1) {
    if (loc_db_gain < 0.0) {
      gain_ratio *= pstr_gain_modifiers->attn_scaling[drc_band];
    } else {
      gain_ratio *= pstr_gain_modifiers->ampl_scaling[drc_band];
    }
  }
  if ((interp_params_str->pstr_ducking_modifiers->ducking_scaling_flag == 1) &&
      (interp_params_str->ducking_flag == 1)) {
    gain_ratio *= interp_params_str->pstr_ducking_modifiers->ducking_scaling;
  }

  {
    *out_param_lin_gain =
        (FLOAT32)pow(2.0, (FLOAT64)(gain_ratio * loc_db_gain / 6.0f));
    *out_param_lin_slope = SLOPE_FACTOR_DB_TO_LINEAR * gain_ratio *
                           *out_param_lin_gain * in_param_db_slope;

    if (pstr_gain_modifiers->gain_offset_flag[drc_band] == 1) {
      *out_param_lin_gain *= (FLOAT32)pow(
          2.0, (FLOAT64)(pstr_gain_modifiers->gain_offset[drc_band] / 6.0f));
    }
    if ((interp_params_str->limiter_peak_target_present == 1) &&
        (interp_params_str->clipping_flag == 1)) {
      *out_param_lin_gain *= (FLOAT32)pow(
          2.0, max(0.0, -interp_params_str->limiter_peak_target -
                            interp_params_str->loudness_normalization_gain_db) /
                   6.0);
      if (*out_param_lin_gain >= 1.0) {
        *out_param_lin_gain = 1.0;
        *out_param_lin_slope = 0.0;
      }
    }
  }
  return;
}

WORD32
impd_compressor_io_sigmoid(
    ia_split_drc_characteristic_struct* split_drc_characteristic,
    FLOAT32 in_db_level, FLOAT32* out_db_gain) {
  FLOAT32 tmp;
  FLOAT32 in_out_ratio = split_drc_characteristic->in_out_ratio;
  FLOAT32 gainDbLimit = split_drc_characteristic->gain;
  FLOAT32 exp = split_drc_characteristic->exp;

  tmp = (DRC_INPUT_LOUDNESS_TARGET - in_db_level) * in_out_ratio;
  if (exp < 1000.0f) {
    FLOAT32 x = tmp / gainDbLimit;
    if (x < 0.0f) {
      return (UNEXPECTED_ERROR);
    }
    *out_db_gain = (FLOAT32)(tmp / pow(1.0f + pow(x, exp), 1.0f / exp));
  } else {
    *out_db_gain = tmp;
  }
  if (split_drc_characteristic->flip_sign == 1) {
    *out_db_gain = -*out_db_gain;
  }
  return (0);
}

WORD32
impd_compressor_io_sigmoid_inv(
    ia_split_drc_characteristic_struct* split_drc_characteristic,
    FLOAT32 loc_db_gain, FLOAT32* in_level) {
  FLOAT32 in_out_ratio = split_drc_characteristic->in_out_ratio;
  FLOAT32 gainDbLimit = split_drc_characteristic->gain;
  FLOAT32 exp = split_drc_characteristic->exp;
  FLOAT32 tmp = loc_db_gain;

  if (split_drc_characteristic->flip_sign == 1) {
    tmp = -loc_db_gain;
  }
  if (exp < 1000.0f) {
    FLOAT32 x = tmp / gainDbLimit;
    if (x < 0.0f) {
      return (UNEXPECTED_ERROR);
    }
    tmp = (FLOAT32)(tmp / pow(1.0f - pow(x, exp), 1.0f / exp));
  }
  *in_level = DRC_INPUT_LOUDNESS_TARGET - tmp / in_out_ratio;

  return (0);
}

WORD32
impd_compressor_io_nodes_lt(
    ia_split_drc_characteristic_struct* split_drc_characteristic,
    FLOAT32 in_db_level, FLOAT32* out_db_gain) {
  WORD32 n;
  FLOAT32 w;
  FLOAT32* node_level = split_drc_characteristic->node_level;
  FLOAT32* node_gain = split_drc_characteristic->node_gain;

  if (in_db_level > DRC_INPUT_LOUDNESS_TARGET) {
    return (UNEXPECTED_ERROR);
  }
  for (n = 1; n <= split_drc_characteristic->characteristic_node_count; n++) {
    if ((in_db_level <= node_level[n - 1]) && (in_db_level > node_level[n])) {
      w = (node_level[n] - in_db_level) / (node_level[n] - node_level[n - 1]);
      *out_db_gain = (FLOAT32)(w * node_gain[n - 1] + (1.0 - w) * node_gain[n]);
    }
  }
  *out_db_gain = node_gain[split_drc_characteristic->characteristic_node_count];
  return (0);
}

WORD32
impd_compressor_io_nodes_rt(
    ia_split_drc_characteristic_struct* split_drc_characteristic,
    FLOAT32 in_db_level, FLOAT32* out_db_gain) {
  WORD32 n;
  FLOAT32 w;
  FLOAT32* node_level = split_drc_characteristic->node_level;
  FLOAT32* node_gain = split_drc_characteristic->node_gain;

  if (in_db_level < DRC_INPUT_LOUDNESS_TARGET) {
    return (UNEXPECTED_ERROR);
  }
  for (n = 1; n <= split_drc_characteristic->characteristic_node_count; n++) {
    if ((in_db_level >= node_level[n - 1]) && (in_db_level < node_level[n])) {
      w = (FLOAT32)(node_level[n] - in_db_level) /
          (node_level[n] - node_level[n - 1]);
      *out_db_gain = (FLOAT32)(w * node_gain[n - 1] + (1.0 - w) * node_gain[n]);
    }
  }
  *out_db_gain =
      (node_gain[split_drc_characteristic->characteristic_node_count]);
  return (0);
}

VOID impd_compressor_io_nodes_inverse(
    ia_split_drc_characteristic_struct* split_drc_characteristic,
    FLOAT32 loc_db_gain, FLOAT32* in_level) {
  WORD32 n;
  FLOAT32 w;
  FLOAT32* node_level = split_drc_characteristic->node_level;
  FLOAT32* node_gain = split_drc_characteristic->node_gain;
  WORD32 node_count = split_drc_characteristic->characteristic_node_count;

  if (node_gain[1] < 0.0f) {
    if (loc_db_gain <= node_gain[node_count]) {
      *in_level = node_level[node_count];
    } else {
      if (loc_db_gain >= 0.0f) {
        *in_level = DRC_INPUT_LOUDNESS_TARGET;
      } else {
        for (n = 1; n <= node_count; n++) {
          if ((loc_db_gain <= node_gain[n - 1]) &&
              (loc_db_gain > node_gain[n])) {
            w = (node_gain[n] - loc_db_gain) /
                (node_gain[n] - node_gain[n - 1]);
            *in_level =
                (FLOAT32)(w * node_level[n - 1] + (1.0 - w) * node_level[n]);
          }
        }
      }
    }
  } else {
    if (loc_db_gain >= node_gain[node_count]) {
      *in_level = node_level[node_count];
    } else {
      if (loc_db_gain <= 0.0f) {
        *in_level = DRC_INPUT_LOUDNESS_TARGET;
      } else {
        for (n = 1; n <= node_count; n++) {
          if ((loc_db_gain >= node_gain[n - 1]) &&
              (loc_db_gain < node_gain[n])) {
            w = (FLOAT32)(node_gain[n] - loc_db_gain) /
                (node_gain[n] - node_gain[n - 1]);
            *in_level =
                (FLOAT32)(w * node_level[n - 1] + (1.0 - w) * node_level[n]);
          }
        }
      }
    }
  }
  return;
}

WORD32
impd_map_gain(
    ia_split_drc_characteristic_struct* split_drc_characteristic_source,
    ia_split_drc_characteristic_struct* split_drc_characteristic_target,
    FLOAT32 gain_in_db, FLOAT32* gain_out_db) {
  FLOAT32 in_level=0;
  WORD32 err = 0;

  switch (split_drc_characteristic_source->characteristic_format) {
    case CHARACTERISTIC_SIGMOID:
      err = impd_compressor_io_sigmoid_inv(split_drc_characteristic_source,
                                           gain_in_db, &in_level);
      if (err) return (err);
      break;
    case CHARACTERISTIC_NODES:
      impd_compressor_io_nodes_inverse(split_drc_characteristic_source,
                                       gain_in_db, &in_level);

      break;
    case CHARACTERISTIC_PASS_THRU:
      in_level = gain_in_db;
      break;
    default:
      return (UNEXPECTED_ERROR);
      break;
  }
  switch (split_drc_characteristic_target->characteristic_format) {
    case CHARACTERISTIC_SIGMOID:
      err = impd_compressor_io_sigmoid(split_drc_characteristic_target, in_level,
                                       gain_out_db);
      if (err) return (err);
      break;
    case CHARACTERISTIC_NODES:
      if (in_level < DRC_INPUT_LOUDNESS_TARGET) {
        err = impd_compressor_io_nodes_lt(split_drc_characteristic_target,
                                          in_level, gain_out_db);
        if (err) return (err);
      } else {
        err = impd_compressor_io_nodes_rt(split_drc_characteristic_target,
                                          in_level, gain_out_db);
        if (err) return (err);
      }
      break;
    case CHARACTERISTIC_PASS_THRU:
      *gain_out_db = in_level;
      break;
    default:
      break;
  }
  return (0);
}

WORD32
impd_conv_to_linear_domain(ia_interp_params_struct* interp_params_str,
                           WORD32 drc_band, FLOAT32 in_param_db_gain,
                           FLOAT32 in_param_db_slope,
                           FLOAT32* out_param_lin_gain,
                           FLOAT32* out_param_lin_slope) {
  WORD32 err = 0;
  FLOAT32 loc_db_gain = in_param_db_gain;
  FLOAT32 gain_ratio = 1.0;
  FLOAT32 mapped_db_gain;
  ia_gain_modifiers_struct* pstr_gain_modifiers =
      interp_params_str->pstr_gain_modifiers;
  if (interp_params_str->gain_modification_flag) {
    ia_split_drc_characteristic_struct* split_drc_characteristic_source;

    WORD32 slopeIsNegative;

    if (interp_params_str->drc_characteristic_present) {
      if (interp_params_str->drc_source_characteristic_cicp_format) {
      } else {
        slopeIsNegative = 0;
        split_drc_characteristic_source =
            interp_params_str->split_source_characteristic_left;
        if (split_drc_characteristic_source->characteristic_format == 0) {
          slopeIsNegative = 1;
        } else {
          if (split_drc_characteristic_source->node_gain[1] > 0.0f) {
            slopeIsNegative = 1;
          }
        }
        if (loc_db_gain == 0.0f) {
          if (((pstr_gain_modifiers
                    ->target_characteristic_left_present[drc_band] == 1) &&
               (interp_params_str->split_target_characteristic_left
                    ->characteristic_format == CHARACTERISTIC_PASS_THRU)) ||
              ((pstr_gain_modifiers
                    ->target_characteristic_right_present[drc_band] == 1) &&
               (interp_params_str->split_target_characteristic_right
                    ->characteristic_format == CHARACTERISTIC_PASS_THRU))) {
            mapped_db_gain = DRC_INPUT_LOUDNESS_TARGET;
            loc_db_gain = DRC_INPUT_LOUDNESS_TARGET;
          }
        } else {
          if (((loc_db_gain > 0.0f) && (slopeIsNegative == 1)) ||
              ((loc_db_gain < 0.0f) && (slopeIsNegative == 0))) {
            if (pstr_gain_modifiers
                    ->target_characteristic_left_present[drc_band] == 1) {
              err = impd_map_gain(
                  split_drc_characteristic_source,
                  interp_params_str->split_target_characteristic_left,
                  loc_db_gain, &mapped_db_gain);
              if (err) return (err);
              gain_ratio = mapped_db_gain / loc_db_gain;
            }

          } else if (((loc_db_gain < 0.0f) && (slopeIsNegative == 1)) ||
                     ((loc_db_gain > 0.0f) && (slopeIsNegative == 0))) {
            if (pstr_gain_modifiers
                    ->target_characteristic_right_present[drc_band] == 1) {
              split_drc_characteristic_source =
                  interp_params_str->split_source_characteristic_right;
              err = impd_map_gain(
                  split_drc_characteristic_source,
                  interp_params_str->split_target_characteristic_right,
                  loc_db_gain, &mapped_db_gain);
              if (err) return (err);
              gain_ratio = mapped_db_gain / loc_db_gain;
            }
          }
        }
      }
    }

    if (loc_db_gain < 0.0f) {
      gain_ratio *= interp_params_str->compress;
    } else {
      gain_ratio *= interp_params_str->boost;
    }
  }
  if (pstr_gain_modifiers->gain_scaling_flag[drc_band] == 1) {
    if (loc_db_gain < 0.0) {
      gain_ratio *= pstr_gain_modifiers->attn_scaling[drc_band];
    } else {
      gain_ratio *= pstr_gain_modifiers->ampl_scaling[drc_band];
    }
  }
  if ((interp_params_str->pstr_ducking_modifiers->ducking_scaling_flag == 1) &&
      (interp_params_str->ducking_flag == 1)) {
    gain_ratio *= interp_params_str->pstr_ducking_modifiers->ducking_scaling;
  }

  if (interp_params_str->interpolation_loud_eq == 1) {
    *out_param_lin_gain =
        gain_ratio * loc_db_gain + pstr_gain_modifiers->gain_offset[drc_band];
    *out_param_lin_slope = 0.0f;
  } else {
    *out_param_lin_gain =
        (FLOAT32)pow(2.0, (FLOAT64)(gain_ratio * loc_db_gain / 6.0f));
    *out_param_lin_slope = SLOPE_FACTOR_DB_TO_LINEAR * gain_ratio *
                           *out_param_lin_gain * in_param_db_slope;

    if (pstr_gain_modifiers->gain_offset_flag[drc_band] == 1) {
      *out_param_lin_gain *= (FLOAT32)pow(
          2.0, (FLOAT64)(pstr_gain_modifiers->gain_offset[drc_band] / 6.0f));
    }
    if ((interp_params_str->limiter_peak_target_present == 1) &&
        (interp_params_str->clipping_flag == 1)) {
      *out_param_lin_gain *= (FLOAT32)pow(
          2.0, max(0.0, -interp_params_str->limiter_peak_target -
                            interp_params_str->loudness_normalization_gain_db) /
                   6.0);
      if (*out_param_lin_gain >= 1.0) {
        *out_param_lin_gain = 1.0;
        *out_param_lin_slope = 0.0;
      }
    }
  }
  return (0);
}

WORD32 impd_interpolate_drc_gain(ia_interp_params_struct* interp_params_str,
                                 WORD32 drc_band, WORD32 gain_step_tdomain,
                                 FLOAT32 gain0, FLOAT32 gain1, FLOAT32 slope0,
                                 FLOAT32 slope1, FLOAT32* result) {
  WORD32 err = 0;
  WORD32 n;
  FLOAT32 k1, k2, a, b, c, d;
  FLOAT32 slope_t1;
  FLOAT32 slope_t2;
  FLOAT32 gain_t1;
  FLOAT32 gain_t2;

  WORD32 cubic_interpolation = 1;
  WORD32 node_inser;
  FLOAT32 node_inser_float;

  if (gain_step_tdomain <= 0) {
    return (UNEXPECTED_ERROR);
  }

  err = impd_conv_to_linear_domain(interp_params_str, drc_band, gain0, slope0,
                                   &gain_t1, &slope_t1);
  if (err) return (err);
  err = impd_conv_to_linear_domain(interp_params_str, drc_band, gain1, slope1,
                                   &gain_t2, &slope_t2);
  if (err) return (err);

  if (interp_params_str->gain_interpolation_type ==
      GAIN_INTERPOLATION_TYPE_SPLINE) {
    slope_t1 = slope_t1 / (FLOAT32)interp_params_str->delta_tmin;
    slope_t2 = slope_t2 / (FLOAT32)interp_params_str->delta_tmin;
    if ((FLOAT32)fabs((FLOAT64)slope_t1) > (FLOAT32)fabs((FLOAT64)slope_t2)) {
      node_inser_float = 2.0f *
                         (gain_t2 - gain_t1 - slope_t2 * gain_step_tdomain) /
                         (slope_t1 - slope_t2);
      node_inser = (WORD32)(0.5f + node_inser_float);
      if ((node_inser >= 0) && (node_inser < gain_step_tdomain)) {
        cubic_interpolation = 0;

        result[0] = gain_t1;
        result[gain_step_tdomain] = gain_t2;

        a = 0.5f * (slope_t2 - slope_t1) / node_inser_float;
        b = slope_t1;
        c = gain_t1;
        for (n = 1; n < node_inser; n++) {
          FLOAT32 t = (FLOAT32)n;
          result[n] = (a * t + b) * t + c;
          result[n] = max(0.0f, result[n]);
        }
        a = slope_t2;
        b = gain_t2;
        for (; n < gain_step_tdomain; n++) {
          FLOAT32 t = (FLOAT32)(n - gain_step_tdomain);
          result[n] = a * t + b;
        }
      }
    } else if ((FLOAT32)fabs((FLOAT64)slope_t1) <
               (FLOAT32)fabs((FLOAT64)slope_t2)) {
      node_inser_float = 2.0f *
                         (gain_t1 - gain_t2 + slope_t1 * gain_step_tdomain) /
                         (slope_t1 - slope_t2);
      node_inser_float = gain_step_tdomain - node_inser_float;
      node_inser = (WORD32)(0.5f + node_inser_float);
      if ((node_inser >= 0) && (node_inser < gain_step_tdomain)) {
        cubic_interpolation = 0;

        result[0] = gain_t1;
        result[gain_step_tdomain] = gain_t2;

        a = slope_t1;
        b = gain_t1;
        for (n = 1; n < node_inser; n++) {
          FLOAT32 t = (FLOAT32)n;
          result[n] = a * t + b;
        }
        a = (slope_t2 - slope_t1) /
            (2.0f * (gain_step_tdomain - node_inser_float));
        b = -slope_t2;
        c = gain_t2;
        for (; n < gain_step_tdomain; n++) {
          FLOAT32 t = (FLOAT32)(gain_step_tdomain - n);
          result[n] = (a * t + b) * t + c;
          result[n] = max(0.0f, result[n]);
        }
      }
    }

    if (cubic_interpolation == 1) {
      FLOAT32 gain_step_inv = 1.0f / (FLOAT32)gain_step_tdomain;
      FLOAT32 gain_step_inv2 = gain_step_inv * gain_step_inv;

      k1 = (gain_t2 - gain_t1) * gain_step_inv2;
      k2 = slope_t2 + slope_t1;

      a = gain_step_inv * (gain_step_inv * k2 - 2.0f * k1);
      b = 3.0f * k1 - gain_step_inv * (k2 + slope_t1);
      c = slope_t1;
      d = gain_t1;

      result[0] = gain_t1;
      result[gain_step_tdomain] = gain_t2;
      for (n = 1; n < gain_step_tdomain; n++) {
        FLOAT32 t = (FLOAT32)n;
        result[n] = (((a * t + b) * t + c) * t) + d;
        result[n] = max(0.0f, result[n]);
      }
    }
  } else {
    a = (gain_t2 - gain_t1) / (FLOAT32)gain_step_tdomain;
    b = gain_t1;
    result[0] = gain_t1;
    result[gain_step_tdomain] = gain_t2;
    for (n = 1; n < gain_step_tdomain; n++) {
      FLOAT32 t = (FLOAT32)n;
      result[n] = a * t + b;
    }
  }
  return 0;
}

VOID impd_advance_buf(WORD32 drc_frame_size,
                      ia_gain_buffer_struct* pstr_gain_buf) {
  WORD32 n;
  ia_interp_buf_struct* buf_interpolation;

  for (n = 0; n < pstr_gain_buf->buf_interpolation_count; n++) {
    buf_interpolation = &(pstr_gain_buf->buf_interpolation[n]);
    buf_interpolation->prev_node = buf_interpolation->str_node;
    buf_interpolation->prev_node.time -= drc_frame_size;
    memmove(buf_interpolation->lpcm_gains,
            buf_interpolation->lpcm_gains + drc_frame_size,
            sizeof(FLOAT32) * (drc_frame_size + MAX_SIGNAL_DELAY));
  }
  return;
}
WORD32
impd_concatenate_segments(WORD32 drc_frame_size, WORD32 drc_band,
                          ia_interp_params_struct* interp_params_str,
                          ia_spline_nodes_struct* str_spline_nodes,
                          ia_interp_buf_struct* buf_interpolation,
                          WORD32 sel_drc_index, WORD32 is_config_changed,
                          WORD32 loudness_changed) {
  WORD32 time_prev, duration, n, err = 0;
  FLOAT32 loc_db_gain = 0.0f, prev_db_gain, slope = 0.0f, slope_prev;

  time_prev = buf_interpolation->prev_node.time;
  prev_db_gain = buf_interpolation->prev_node.loc_db_gain;
  slope_prev = buf_interpolation->prev_node.slope;
  for (n = 0; n < str_spline_nodes->num_nodes; n++) {
    duration = str_spline_nodes->str_node[n].time - time_prev;
    loc_db_gain = str_spline_nodes->str_node[n].loc_db_gain;
    if (loudness_changed) {
      if (sel_drc_index == 0 && is_config_changed == 1) {
        loc_db_gain = str_spline_nodes->str_node[n].loc_db_gain +
                      interp_params_str->loudness_normalization_gain_db;
        if (prev_db_gain == 0) {
          prev_db_gain = buf_interpolation->prev_node.loc_db_gain +
                         interp_params_str->loudness_normalization_gain_db;
        }
      }
    }
    slope = str_spline_nodes->str_node[n].slope;

    err = impd_interpolate_drc_gain(
        interp_params_str, drc_band, duration, prev_db_gain, loc_db_gain,
        slope_prev, slope, buf_interpolation->lpcm_gains + MAX_SIGNAL_DELAY +
                              drc_frame_size + time_prev);
    if (err) return (err);

    time_prev = str_spline_nodes->str_node[n].time;
    prev_db_gain = loc_db_gain;
    slope_prev = slope;
  }

  buf_interpolation->str_node.loc_db_gain = loc_db_gain;
  buf_interpolation->str_node.slope = slope;
  buf_interpolation->str_node.time = time_prev;

  return (0);
}

WORD32
impd_get_drc_gain(ia_drc_gain_dec_struct* p_drc_gain_dec_structs,
                  ia_drc_config* pstr_drc_config,
                  ia_drc_gain_struct* pstr_drc_gain, FLOAT32 compress,
                  FLOAT32 boost, WORD32 characteristic_index,
                  FLOAT32 loudness_normalization_gain_db, WORD32 sel_drc_index,
                  ia_drc_gain_buffers_struct* drc_gain_buffers) {
  ia_drc_params_struct* ia_drc_params_struct =
      &(p_drc_gain_dec_structs->ia_drc_params_struct);
  WORD32 drc_instructions_index =
      ia_drc_params_struct->sel_drc_array[sel_drc_index].drc_instructions_index;
  if (drc_instructions_index >= 0) {
    WORD32 b, g, gain_element_index, err = 0;
    WORD32 parametric_drc_instance_index = 0;
    ia_interp_params_struct interp_params_str = {0};

    ia_drc_instructions_struct* str_drc_instruction_str =
        &(pstr_drc_config->str_drc_instruction_str[drc_instructions_index]);
    WORD32 drc_set_effect = str_drc_instruction_str->drc_set_effect;
    WORD32 num_drc_ch_groups = str_drc_instruction_str->num_drc_ch_groups;
    ia_uni_drc_coeffs_struct* str_p_loc_drc_coefficients_uni_drc = NULL;
    WORD32 drc_coeff_idx =
        ia_drc_params_struct->sel_drc_array[sel_drc_index].drc_coeff_idx;
    if (drc_coeff_idx >= 0) {
      str_p_loc_drc_coefficients_uni_drc =
          &(pstr_drc_config->str_p_loc_drc_coefficients_uni_drc[drc_coeff_idx]);
      interp_params_str.interpolation_loud_eq = 0;
    } else {
      return (UNEXPECTED_ERROR);
    }

    interp_params_str.loudness_normalization_gain_db =
        loudness_normalization_gain_db;
    interp_params_str.characteristic_index = characteristic_index;
    interp_params_str.compress = compress;
    interp_params_str.boost = boost;
    interp_params_str.limiter_peak_target_present =
        str_drc_instruction_str->limiter_peak_target_present;
    interp_params_str.limiter_peak_target =
        str_drc_instruction_str->limiter_peak_target;

    if (((drc_set_effect & (EFFECT_BIT_DUCK_OTHER | EFFECT_BIT_DUCK_SELF)) ==
         0) &&
        (drc_set_effect != EFFECT_BIT_FADE) &&
        (drc_set_effect != EFFECT_BIT_CLIPPING)) {
      interp_params_str.gain_modification_flag = 1;
    } else {
      interp_params_str.gain_modification_flag = 0;
    }
    if (drc_set_effect & (EFFECT_BIT_DUCK_OTHER | EFFECT_BIT_DUCK_SELF)) {
      interp_params_str.ducking_flag = 1;
    } else {
      interp_params_str.ducking_flag = 0;
    }
    if (drc_set_effect == EFFECT_BIT_CLIPPING) {
      interp_params_str.clipping_flag = 1;
    } else {
      interp_params_str.clipping_flag = 0;
    }

    impd_advance_buf(ia_drc_params_struct->drc_frame_size,
                     &(drc_gain_buffers->pstr_gain_buf[sel_drc_index]));

    gain_element_index = 0;
    for (g = 0; g < num_drc_ch_groups; g++) {
      WORD32 gainSet = 0;
      WORD32 num_drc_bands = 0;
      interp_params_str.gain_interpolation_type =
          str_drc_instruction_str->gain_interpolation_type_for_channel_group[g];
      interp_params_str.delta_tmin =
          str_drc_instruction_str->time_delta_min_for_channel_group[g];
      interp_params_str.pstr_ducking_modifiers = &(
          str_drc_instruction_str->str_ducking_modifiers_for_channel_group[g]);
      interp_params_str.pstr_gain_modifiers =
          &(str_drc_instruction_str->str_gain_modifiers_of_ch_group[g]);
      if (str_drc_instruction_str->ch_group_parametric_drc_flag[g] == 0) {
        gainSet = str_drc_instruction_str->gain_set_index_for_channel_group[g];
        num_drc_bands = str_drc_instruction_str->band_count_of_ch_group[g];
        for (b = 0; b < num_drc_bands; b++) {
          ia_gain_params_struct* gain_params =
              &(str_p_loc_drc_coefficients_uni_drc->gain_set_params[gainSet]
                    .gain_params[b]);
          WORD32 seq = gain_params->gain_seq_idx;
          interp_params_str.drc_characteristic_present =
              gain_params->drc_characteristic_present;
          interp_params_str.drc_source_characteristic_cicp_format =
              gain_params->drc_characteristic_format_is_cicp;
          interp_params_str.source_drc_characteristic =
              gain_params->drc_characteristic;
          interp_params_str.split_source_characteristic_left = &(
              str_p_loc_drc_coefficients_uni_drc->str_split_characteristic_left
                  [gain_params->drc_characteristic_left_index]);
          interp_params_str.split_source_characteristic_right = &(
              str_p_loc_drc_coefficients_uni_drc->str_split_characteristic_right
                  [gain_params->drc_characteristic_right_index]);
          interp_params_str.split_target_characteristic_left = &(
              str_p_loc_drc_coefficients_uni_drc->str_split_characteristic_left
                  [interp_params_str.pstr_gain_modifiers
                       ->target_characteristic_left_index[b]]);
          interp_params_str.split_target_characteristic_right = &(
              str_p_loc_drc_coefficients_uni_drc->str_split_characteristic_right
                  [interp_params_str.pstr_gain_modifiers
                       ->target_characteristic_right_index[b]]);
          err = impd_concatenate_segments(
              ia_drc_params_struct->drc_frame_size, b, &interp_params_str,
              &(pstr_drc_gain->drc_gain_sequence[seq].str_spline_nodes[0]),
              &(drc_gain_buffers->pstr_gain_buf[sel_drc_index]
                    .buf_interpolation[gain_element_index]),
              sel_drc_index, pstr_drc_config->is_config_changed,
              pstr_drc_config->ln_gain_changed);
          if (err) return (err);
          gain_element_index++;
        }
      } else {
        if (ia_drc_params_struct->sub_band_domain_mode ==
                SUBBAND_DOMAIN_MODE_OFF &&
            !(p_drc_gain_dec_structs->parametricdrc_params
                  .str_parametric_drc_instance_params
                      [parametric_drc_instance_index]
                  .parametric_drc_type == PARAM_DRC_TYPE_LIM)) {
          err = impd_parametric_drc_instance_process(
              p_drc_gain_dec_structs->audio_in_out_buf.audio_in_out_buf, NULL,
              NULL, &p_drc_gain_dec_structs->parametricdrc_params,
              &p_drc_gain_dec_structs->parametricdrc_params
                   .str_parametric_drc_instance_params
                       [parametric_drc_instance_index]);
          if (err) return (err);

          err = impd_concatenate_segments(
              ia_drc_params_struct->drc_frame_size, 0, &interp_params_str,
              &p_drc_gain_dec_structs->parametricdrc_params
                   .str_parametric_drc_instance_params
                       [parametric_drc_instance_index]
                   .str_spline_nodes,
              &(drc_gain_buffers->pstr_gain_buf[sel_drc_index]
                    .buf_interpolation[gain_element_index]),
              sel_drc_index, pstr_drc_config->is_config_changed,
              pstr_drc_config->ln_gain_changed);
          if (err) return (err);
        } else if (ia_drc_params_struct->sub_band_domain_mode ==
                       SUBBAND_DOMAIN_MODE_OFF &&
                   p_drc_gain_dec_structs->parametricdrc_params
                           .str_parametric_drc_instance_params
                               [parametric_drc_instance_index]
                           .parametric_drc_type == PARAM_DRC_TYPE_LIM) {
          FLOAT32* lpcm_gains = (drc_gain_buffers->pstr_gain_buf[sel_drc_index]
                                     .buf_interpolation[gain_element_index])
                                    .lpcm_gains +
                                MAX_SIGNAL_DELAY;
          impd_parametric_lim_type_drc_process(
              p_drc_gain_dec_structs->audio_in_out_buf.audio_in_out_buf,
              loudness_normalization_gain_db,
              &p_drc_gain_dec_structs->parametricdrc_params
                   .str_parametric_drc_instance_params
                       [parametric_drc_instance_index]
                   .str_parametric_drc_type_lim_params,
              lpcm_gains);

        } else if (ia_drc_params_struct->sub_band_domain_mode !=
                       SUBBAND_DOMAIN_MODE_OFF &&
                   !(p_drc_gain_dec_structs->parametricdrc_params
                         .str_parametric_drc_instance_params
                             [parametric_drc_instance_index]
                         .parametric_drc_type == PARAM_DRC_TYPE_LIM)) {
          err = impd_parametric_drc_instance_process(
              NULL, p_drc_gain_dec_structs->audio_in_out_buf.audio_real_buff,
              p_drc_gain_dec_structs->audio_in_out_buf.audio_imag_buff,
              &p_drc_gain_dec_structs->parametricdrc_params,
              &p_drc_gain_dec_structs->parametricdrc_params
                   .str_parametric_drc_instance_params
                       [parametric_drc_instance_index]);
          if (err) return (err);

          err = impd_concatenate_segments(
              ia_drc_params_struct->drc_frame_size, 0, &interp_params_str,
              &p_drc_gain_dec_structs->parametricdrc_params
                   .str_parametric_drc_instance_params
                       [parametric_drc_instance_index]
                   .str_spline_nodes,
              &(drc_gain_buffers->pstr_gain_buf[sel_drc_index]
                    .buf_interpolation[gain_element_index]),
                    sel_drc_index, pstr_drc_config->is_config_changed,
                    pstr_drc_config->ln_gain_changed);
          if (err) return (err);

        } else {
          return (UNEXPECTED_ERROR);
        }
        gain_element_index++;
        parametric_drc_instance_index++;
      }
    }
  }
  return (0);
}
