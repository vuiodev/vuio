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

#include <stdlib.h>
#include <string.h>
#include <math.h>
#include <float.h>
#include "ixheaac_type_def.h"
#include "ixheaac_error_standards.h"
#include "ixheaace_error_codes.h"

#include "iusace_cnst.h"
#include "iusace_bitbuffer.h"
#include "impd_drc_common_enc.h"
#include "impd_drc_uni_drc.h"
#include "impd_drc_api.h"
#include "impd_drc_uni_drc_eq.h"
#include "impd_drc_uni_drc_filter_bank.h"
#include "impd_drc_gain_enc.h"
#include "impd_drc_struct_def.h"
#include "impd_drc_tables.h"
#include "impd_drc_enc.h"
#include "impd_drc_mux.h"
#include "iusace_block_switch_const.h"

#include "iusace_tns_usac.h"
#include "iusace_psy_mod.h"
#include "ixheaace_sbr_header.h"
#include "ixheaace_config.h"
#include "iusace_config.h"
#include "ixheaace_common_utils.h"

static FLOAT32 impd_drc_limit_drc_gain(const WORD32 gain_coding_profile, const FLOAT32 gain) {
  FLOAT32 limited_drc_gain;

  switch (gain_coding_profile) {
    case GAIN_CODING_PROFILE_CONSTANT:
      limited_drc_gain = gain;
      break;
    case GAIN_CODING_PROFILE_CLIPPING:
      limited_drc_gain =
          MAX(MIN(MAX_DRC_GAIN_CODING_PROFILE2, gain), MIN_DRC_GAIN_CODING_PROFILE2);
      break;
    case GAIN_CODING_PROFILE_FADING:
      limited_drc_gain =
          MAX(MIN(MAX_DRC_GAIN_CODING_PROFILE1, gain), MIN_DRC_GAIN_CODING_PROFILE1);
      break;
    case GAIN_CODING_PROFILE_REGULAR:
      limited_drc_gain =
          MAX(MIN(MAX_DRC_GAIN_CODING_PROFILE0, gain), MIN_DRC_GAIN_CODING_PROFILE0);
      break;
    default:
      limited_drc_gain = gain;
      break;
  }

  return limited_drc_gain;
}

static VOID impd_drc_get_quantized_delta_drc_gain(const WORD32 gain_coding_profile,
                                                  const FLOAT32 delta_gain,
                                                  FLOAT32 *delta_gain_quant, WORD32 *num_bits,
                                                  WORD32 *gain_code) {
  LOOPIDX idx;
  WORD32 num_entries, opt_index;
  WORD32 min_pos_diff_idx = 0;
  WORD32 min_neg_diff_idx = 0;
  FLOAT32 difference;
  FLOAT32 min_pos_diff = 1000.0f;
  FLOAT32 min_neg_diff = -1000.0f;
  ia_drc_delta_gain_code_entry_struct const *pstr_delta_gain_code_table;

  impd_drc_get_delta_gain_code_table(gain_coding_profile, &pstr_delta_gain_code_table,
                                     &num_entries);
  for (idx = 0; idx < num_entries; idx++) {
    difference = delta_gain - pstr_delta_gain_code_table[idx].value;
    if (difference <= 0.0f) {
      if (difference > min_neg_diff) {
        min_neg_diff = difference;
        min_neg_diff_idx = idx;
      }
    } else {
      if (difference < min_pos_diff) {
        min_pos_diff = difference;
        min_pos_diff_idx = idx;
      }
    }
  }
  if (min_pos_diff >= -min_neg_diff) {
    opt_index = min_neg_diff_idx;
  } else {
    opt_index = min_pos_diff_idx;
  }

  *delta_gain_quant = pstr_delta_gain_code_table[opt_index].value;
  *num_bits = pstr_delta_gain_code_table[opt_index].size;
  *gain_code = pstr_delta_gain_code_table[opt_index].code;
}

static VOID impd_drc_check_overshoot(const WORD32 t_gain_step, const FLOAT32 gain_0,
                                     const FLOAT32 gain_1, const FLOAT32 slope_0,
                                     const FLOAT32 slope_1, const WORD32 time_delta_min,
                                     WORD32 *overshoot_left, WORD32 *overshoot_right) {
  WORD32 t_connect;
  FLOAT32 norm_slope_0, norm_slope_1;
  FLOAT32 gain_left, gain_right;
  FLOAT32 t_gain_step_inv, t_gain_step_inv_2;
  FLOAT32 temp_a, temp_b, temp_c, temp_d;
  FLOAT32 curve_left, curve_right;
  FLOAT32 max_val, min_val;
  FLOAT32 tmp, tmp2;
  FLOAT32 g_extreme, t_extreme;
  FLOAT32 k1, k2;
  FLOAT32 slope_norm = 1.0f / (FLOAT32)time_delta_min;
  FLOAT32 margin = 0.2f;
  FLOAT32 step_inv_2 = ixheaace_div32(2.0f, (FLOAT32)t_gain_step);

  *overshoot_left = FALSE;
  *overshoot_right = FALSE;

  gain_left = (FLOAT32)pow((FLOAT64)10.0, (FLOAT64)(0.05f * gain_0));
  gain_right = (FLOAT32)pow((FLOAT64)10.0, (FLOAT64)(0.05f * gain_1));

  norm_slope_0 = slope_0 * slope_norm * SLOPE_FACTOR_DB_TO_LINEAR * gain_left;
  norm_slope_1 = slope_1 * slope_norm * SLOPE_FACTOR_DB_TO_LINEAR * gain_right;

  if ((FLOAT32)fabs((FLOAT64)norm_slope_0) < (FLOAT32)fabs((FLOAT64)norm_slope_1)) {
    t_connect = (WORD32)(0.5f + 2.0f * (gain_left - gain_right + norm_slope_0 * t_gain_step) /
                                    (norm_slope_0 - norm_slope_1));
    if ((t_gain_step >= t_connect) && (t_connect > 0)) {
      return;
    }
  } else if ((FLOAT32)fabs((FLOAT64)norm_slope_0) > (FLOAT32)fabs((FLOAT64)norm_slope_1)) {
    t_connect = (WORD32)(0.5f + 2.0f * (gain_right - gain_left - norm_slope_1 * t_gain_step) /
                                    (norm_slope_0 - norm_slope_1));
    if ((t_connect >= 0) && (t_connect < t_gain_step)) {
      return;
    }
  }

  tmp = 1.5f * step_inv_2 * (gain_right - gain_left) - norm_slope_1 - norm_slope_0;
  curve_left = step_inv_2 * (tmp - norm_slope_0);
  curve_right = step_inv_2 * (norm_slope_1 - tmp);

  tmp = -norm_slope_0 * t_gain_step - gain_left + gain_right;
  if (curve_left >= 0.0f) {
    if (tmp + margin < 0.0f) {
      *overshoot_left = TRUE;
    }
  } else {
    if (tmp - margin > 0.0f) {
      *overshoot_left = TRUE;
    }
  }
  tmp = norm_slope_1 * t_gain_step - gain_right + gain_left;
  if (curve_right >= 0.0f) {
    if (tmp + margin < 0.0f) {
      *overshoot_right = TRUE;
    }
  } else {
    if (tmp - margin > 0.0f) {
      *overshoot_right = TRUE;
    }
  }

  if ((!*overshoot_left) && (!*overshoot_right)) {
    t_gain_step_inv = ixheaace_div32(1.0f, (FLOAT32)t_gain_step);
    t_gain_step_inv_2 = t_gain_step_inv * t_gain_step_inv;
    k1 = (gain_right - gain_left) * t_gain_step_inv_2;
    k2 = norm_slope_1 + norm_slope_0;

    temp_a = t_gain_step_inv * (t_gain_step_inv * k2 - 2.0f * k1);
    temp_b = 3.0f * k1 - t_gain_step_inv * (k2 + norm_slope_0);
    temp_c = norm_slope_0;
    temp_d = gain_left;
    tmp = temp_b * temp_b - 3.0f * temp_a * temp_c;

    if (!((tmp < 0.0f) || (temp_a == 0.0f))) {
      max_val = MAX(gain_left, gain_right) + margin;
      min_val = MIN(gain_left, gain_right) - margin;
      tmp = (FLOAT32)sqrt((FLOAT64)tmp);
      tmp2 = (1.0f / (3.0f * temp_a));

      t_extreme = tmp2 * (-temp_b + tmp);
      if ((t_extreme > 0.0f) && (t_extreme < t_gain_step)) {
        g_extreme = (((temp_a * t_extreme + temp_b) * t_extreme + temp_c) * t_extreme) + temp_d;
        if ((g_extreme > max_val) || (g_extreme < min_val)) {
          *overshoot_left = TRUE;
        }
      }

      t_extreme = tmp2 * (-temp_b - tmp);
      if ((t_extreme > 0.0f) && (t_extreme < t_gain_step)) {
        g_extreme = (((temp_a * t_extreme + temp_b) * t_extreme + temp_c) * t_extreme) + temp_d;
        if ((g_extreme > max_val) || (g_extreme < min_val)) {
          *overshoot_left = TRUE;
        }
      }
    }
  }
}

static VOID impd_drc_quantize_slope(const FLOAT32 slope, FLOAT32 *slope_quant,
                                    WORD32 *slope_code_index) {
  LOOPIDX idx = 0;
  const ia_drc_slope_code_table_entry_struct *pstr_slope_code_table =
      impd_drc_get_slope_code_table_by_value();

  while ((idx < 14) && (slope > pstr_slope_code_table[idx].value)) {
    idx++;
  }
  if (idx > 0 && ((pstr_slope_code_table[idx].value - slope) >
                  (slope - pstr_slope_code_table[idx - 1].value))) {
    idx--;
  }

  *slope_quant = pstr_slope_code_table[idx].value;
  *slope_code_index = pstr_slope_code_table[idx].index;
}

static VOID impd_drc_get_preliminary_nodes(const ia_drc_gain_enc_struct *pstr_gain_enc,
                                           const FLOAT32 *ptr_drc_gain_per_sample,
                                           FLOAT32 *ptr_drc_gain_per_sample_with_prev_frame,
                                           ia_drc_group_struct *pstr_drc_group,
                                           const WORD32 full_frame, VOID *pstr_scratch) {
  LOOPIDX n, k;
  WORD32 t, index;
  WORD32 drc_frame_size = pstr_gain_enc->drc_frame_size;
  WORD32 time_delta_min = pstr_gain_enc->delta_tmin;
  WORD32 num_values = drc_frame_size / time_delta_min;
  WORD32 offset = time_delta_min / 2;
  WORD32 num_gain_values;
  WORD32 n_left, n_right;

  FLOAT32 gain, gain_quant, gain_quant_prev;
  FLOAT32 quant_error_prev = -1.0f;
  FLOAT32 quant_error;
  FLOAT32 slope_prev, slope_next;
  FLOAT32 f0 = 0.9f;
  FLOAT32 f1 = 1.0f - f0;

  WORD32 *ptr_time_at_node = pstr_drc_group->ts_gain;
  FLOAT32 *ptr_gain_at_node = pstr_drc_group->drc_gain;
  FLOAT32 *ptr_slope_at_node = pstr_drc_group->slope;
  FLOAT32 *ptr_gain = ptr_drc_gain_per_sample_with_prev_frame + drc_frame_size;
  iusace_scratch_mem *ptr_scratch = (iusace_scratch_mem *)(pstr_scratch);
  FLOAT32 *ptr_slope = (FLOAT32 *)ptr_scratch->ptr_drc_scratch_buf;

  memcpy(ptr_drc_gain_per_sample_with_prev_frame,
         &(ptr_drc_gain_per_sample_with_prev_frame[drc_frame_size]),
         drc_frame_size * sizeof(FLOAT32));
  memcpy(&(ptr_drc_gain_per_sample_with_prev_frame[drc_frame_size]), ptr_drc_gain_per_sample,
         drc_frame_size * sizeof(FLOAT32));

  for (n = 0; n < drc_frame_size; n++) {
    ptr_gain[n] *= SCALE_APPROXIMATE_DB;
  }
  for (n = 0; n < drc_frame_size; n++) {
    ptr_gain[n] = f0 * ptr_gain[n - 1] + f1 * ptr_gain[n];
  }

  if (pstr_drc_group->gain_prev_node < 0.f) {
    gain_quant_prev =
        GAIN_QUANT_STEP_SIZE *
        ((WORD32)(-0.5f + GAIN_QUANT_STEP_SIZE_INV * pstr_drc_group->gain_prev_node));
  } else {
    gain_quant_prev =
        GAIN_QUANT_STEP_SIZE *
        ((WORD32)(0.5f + GAIN_QUANT_STEP_SIZE_INV * pstr_drc_group->gain_prev_node));
  }

  k = -1;
  for (n = 1; n < num_values + 1; n++) {
    gain = ptr_gain[n * time_delta_min - 1];
    if (gain < 0.f) {
      gain_quant = GAIN_QUANT_STEP_SIZE * ((WORD32)(-0.5f + GAIN_QUANT_STEP_SIZE_INV * gain));
    } else {
      gain_quant = GAIN_QUANT_STEP_SIZE * ((WORD32)(0.5f + GAIN_QUANT_STEP_SIZE_INV * gain));
    }
    quant_error = (FLOAT32)fabs((FLOAT64)(gain - gain_quant));

    slope_prev = (gain - ptr_gain[(n - 1) * time_delta_min - 1]);
    if (n == num_values) {
      slope_next = 0.2f;
    } else {
      slope_next = (ptr_gain[(n + 1) * time_delta_min - 1] - gain);
    }

    if (gain_quant_prev != gain_quant) {
      k++;
      quant_error_prev = quant_error;
      gain_quant_prev = gain_quant;
      ptr_time_at_node[k] = n * time_delta_min - 1;
      if ((FLOAT32)fabs((FLOAT64)slope_prev) > 0.1f) {
        gain_quant_prev = 1000.0f;
      }
    } else {
      if ((FLOAT32)fabs((FLOAT64)slope_next) > 0.1f) {
        if (k < 0) {
          k = 0;
        }
        ptr_time_at_node[k] = n * time_delta_min - 1;
      } else {
        if (quant_error_prev > quant_error) {
          if (k < 0) {
            k = 0;
          }
          quant_error_prev = quant_error;
          ptr_time_at_node[k] = n * time_delta_min - 1;
        }
      }
    }
  }
  if (full_frame == 1) {
    if (ptr_time_at_node[k] != drc_frame_size - 1) {
      k++;
      ptr_time_at_node[k] = drc_frame_size - 1;
    }
  }

  num_gain_values = k + 1;
  if (num_gain_values <= 0) {
    if (k < 0) {
      k = 0;
    }
    n = num_values / 2;
    index = offset + n * time_delta_min - 1;
    ptr_slope[n] =
        ptr_drc_gain_per_sample[index + time_delta_min] - ptr_drc_gain_per_sample[index];
    t = (n + 1) * time_delta_min - 1;
    ptr_time_at_node[k] = t;
    ptr_slope_at_node[k] = ptr_slope[n];
    ptr_gain_at_node[k] = ptr_drc_gain_per_sample[t];
    num_gain_values++;
  }

  for (k = 0; k < num_gain_values; k++) {
    n_left = MAX(0, ptr_time_at_node[k] - time_delta_min);
    n_right = n_left + time_delta_min;
    ptr_slope_at_node[k] = ptr_gain[n_right] - ptr_gain[n_left];
    ptr_gain_at_node[k] = ptr_gain[ptr_time_at_node[k]];
  }

  pstr_drc_group->n_gain_values = num_gain_values;
}

static VOID impd_drc_advance_nodes(ia_drc_gain_enc_struct *pstr_gain_enc,
                                   ia_drc_gain_seq_buf_struct *pstr_drc_gain_seq_buf) {
  LOOPIDX idx;
  ia_drc_group_struct *pstr_drc_group = &(pstr_drc_gain_seq_buf->str_drc_group);
  ia_drc_group_for_output_struct *pstr_drc_group_for_output =
      &(pstr_drc_gain_seq_buf->str_drc_group_for_output);

  if (pstr_drc_group_for_output->n_gain_values > 0) {
    pstr_drc_group_for_output->time_quant_prev =
        pstr_drc_group_for_output->ts_gain_quant[pstr_drc_group_for_output->n_gain_values - 1] -
        pstr_gain_enc->drc_frame_size;
    pstr_drc_group_for_output->slope_code_index_prev =
        pstr_drc_group_for_output->slope_code_index[pstr_drc_group_for_output->n_gain_values - 1];
    pstr_drc_group_for_output->drc_gain_quant_prev =
        pstr_drc_group_for_output->drc_gain_quant[pstr_drc_group_for_output->n_gain_values - 1];
  }
  for (idx = 0; idx < pstr_drc_group->n_gain_values; idx++) {
    pstr_drc_group_for_output->ts_gain_quant[idx] = pstr_drc_group->ts_gain_quant[idx];
    pstr_drc_group_for_output->time_delta_quant[idx] = pstr_drc_group->time_delta_quant[idx];
    pstr_drc_group_for_output->slope_quant[idx] = pstr_drc_group->slope_quant[idx];
    pstr_drc_group_for_output->slope_code_index[idx] = pstr_drc_group->slope_code_index[idx];
    pstr_drc_group_for_output->gain_code[idx] = pstr_drc_group->gain_code[idx];
    pstr_drc_group_for_output->gain_code_length[idx] = pstr_drc_group->gain_code_length[idx];
    pstr_drc_group_for_output->drc_gain_quant[idx] = pstr_drc_group->drc_gain_quant[idx];
  }
  pstr_drc_group_for_output->n_gain_values = pstr_drc_group->n_gain_values;
}

static IA_ERRORCODE impd_drc_post_process_nodes(
    ia_drc_gain_enc_struct *pstr_gain_enc,
    ia_drc_delta_time_code_table_entry_struct *pstr_delta_time_code_table,
    ia_drc_gain_seq_buf_struct *pstr_drc_gain_seq_buf, VOID *pstr_scratch) {
  LOOPIDX k, n;
  WORD32 time_mandatory_node;
  WORD32 n_removed, move_on;
  WORD32 idx_left, idx_right;
  WORD32 idx_0, idx_1, idx_2, idx_3;
  WORD32 left, mid, right;
  WORD32 overshoot_right, overshoot_left;
  WORD32 cod_slope_zero = 0x7;
  WORD32 slope_changed = TRUE;
  WORD32 repeat_check = TRUE;
  WORD32 time_prev = -1;
  WORD32 time_delta_min = pstr_gain_enc->delta_tmin;

  FLOAT32 delta_gain;
  FLOAT32 delta_gain_quant;
  FLOAT32 gain_value_quant = 0;
  FLOAT32 slope_average;
  FLOAT32 slope_of_nodes_left;
  FLOAT32 slope_of_nodes_right;
  FLOAT32 thr_low, thr_high;
  FLOAT32 delta_left, delta_right;
  FLOAT32 slope_0, slope_1, slope_2;
  IA_ERRORCODE err_code = IA_NO_ERROR;

  const ia_drc_slope_code_table_entry_struct *pstr_slope_code_table;
  ia_drc_group_for_output_struct *pstr_drc_group_for_output =
      &(pstr_drc_gain_seq_buf->str_drc_group_for_output);
  WORD32 num_gain_values = pstr_drc_group_for_output->n_gain_values;
  FLOAT32 drc_gain_quant_prev = pstr_drc_group_for_output->drc_gain_quant_prev;

  iusace_scratch_mem *ptr_scratch = (iusace_scratch_mem *)(pstr_scratch);
  FLOAT32 *ptr_gain_buf = (FLOAT32 *)((UWORD8 *)ptr_scratch->ptr_drc_scratch_buf);
  WORD32 *ptr_time_buf =
      (WORD32 *)(((UWORD8 *)ptr_gain_buf) + (N_UNIDRC_GAIN_MAX + 2) * sizeof(ptr_gain_buf[0]));
  WORD32 *ptr_slope_code_index_buf =
      (WORD32 *)(((UWORD8 *)ptr_time_buf) + (N_UNIDRC_GAIN_MAX + 2) * sizeof(ptr_time_buf[0]));
  WORD32 *ptr_remove = (WORD32 *)(((UWORD8 *)ptr_slope_code_index_buf) +
                                  (N_UNIDRC_GAIN_MAX + 2) * sizeof(ptr_slope_code_index_buf[0]));

  if (pstr_drc_gain_seq_buf->str_gain_set_params.full_frame != 1) {
    time_mandatory_node = 99999999;
  } else {
    time_mandatory_node = pstr_gain_enc->drc_frame_size - 1;
  }

  ptr_time_buf[0] = pstr_drc_group_for_output->time_quant_prev;
  ptr_gain_buf[0] = pstr_drc_group_for_output->drc_gain_quant_prev;
  for (k = 0; k < num_gain_values; k++) {
    ptr_time_buf[k + 1] = pstr_drc_group_for_output->ts_gain_quant[k];
    ptr_gain_buf[k + 1] = pstr_drc_group_for_output->drc_gain_quant[k];
  }
  ptr_time_buf[k + 1] = pstr_drc_group_for_output->time_quant_next;
  ptr_gain_buf[k + 1] = pstr_drc_group_for_output->drc_gain_quant_next;

  if (num_gain_values > 1) {
    idx_left = 0;
    idx_right = 2;
    n_removed = 0;
    for (k = 0; k <= num_gain_values + 1; k++) {
      ptr_remove[k] = FALSE;
    }
    while (idx_right <= num_gain_values + 1) {
      if ((ptr_gain_buf[idx_left] == ptr_gain_buf[idx_right - 1]) &&
          (ptr_gain_buf[idx_right - 1] == ptr_gain_buf[idx_right]) &&
          (num_gain_values - n_removed > 1) &&
          (ptr_time_buf[idx_right - 1] != time_mandatory_node)) {
        ptr_remove[idx_right - 1] = TRUE;
        idx_right++;
        n_removed++;
      } else {
        idx_left = idx_right - 1;
        idx_right++;
      }
    }

    n = 1;
    for (k = 1; k <= num_gain_values + 1; k++) {
      if (!ptr_remove[k]) {
        ptr_time_buf[n] = ptr_time_buf[k];
        ptr_gain_buf[n] = ptr_gain_buf[k];
        n++;
      }
    }

    n = 0;
    for (k = 0; k < num_gain_values; k++) {
      if (!ptr_remove[k + 1]) {
        pstr_drc_group_for_output->ts_gain_quant[n] = pstr_drc_group_for_output->ts_gain_quant[k];
        pstr_drc_group_for_output->time_delta_quant[n] =
            pstr_drc_group_for_output->time_delta_quant[k];
        pstr_drc_group_for_output->slope_quant[n] = pstr_drc_group_for_output->slope_quant[k];
        pstr_drc_group_for_output->slope_code_index[n] =
            pstr_drc_group_for_output->slope_code_index[k];
        pstr_drc_group_for_output->gain_code[n] = pstr_drc_group_for_output->gain_code[k];
        pstr_drc_group_for_output->gain_code_length[n] =
            pstr_drc_group_for_output->gain_code_length[k];
        pstr_drc_group_for_output->drc_gain_quant[n] =
            pstr_drc_group_for_output->drc_gain_quant[k];
        n++;
      }
    }
    num_gain_values = n;
  }

  if (num_gain_values > 2) {
    move_on = FALSE;
    idx_0 = 0;
    idx_1 = 1;
    idx_2 = 2;
    idx_3 = 3;
    n_removed = 0;
    for (k = 0; k <= num_gain_values + 1; k++) {
      ptr_remove[k] = FALSE;
    }
    while (idx_3 < num_gain_values + 1) {
      if (move_on) {
        move_on = FALSE;
        idx_0 = idx_1;
        idx_1 = idx_2;
        idx_2 = idx_3;
        idx_3++;
      }
      if (ptr_gain_buf[idx_1] != ptr_gain_buf[idx_2]) {
        move_on = TRUE;
      } else {
        delta_left = ptr_gain_buf[idx_1] - ptr_gain_buf[idx_0];
        delta_right = ptr_gain_buf[idx_3] - ptr_gain_buf[idx_2];

        if (((FLOAT32)fabs((FLOAT64)delta_left) < 0.26f) ||
            ((FLOAT32)fabs((FLOAT64)delta_right) < 0.26f)) {
          if ((delta_left > 0.0f) && (delta_right > 0.0f) &&
              (ptr_time_buf[idx_1] != time_mandatory_node)) {
            ptr_remove[idx_1] = TRUE;
            pstr_drc_group_for_output->gain_code[idx_2 - 1] =
                pstr_drc_group_for_output->gain_code[idx_1 - 1];
            pstr_drc_group_for_output->gain_code_length[idx_2 - 1] =
                pstr_drc_group_for_output->gain_code_length[idx_1 - 1];
            idx_1 = idx_2;
            idx_2 = idx_3;
            idx_3++;
            n_removed++;
          } else if ((delta_left < 0.0f) && (delta_right < 0.0f) &&
                     (ptr_time_buf[idx_2] != time_mandatory_node)) {
            ptr_remove[idx_2] = TRUE;
            idx_2 = idx_3;
            idx_3++;
            n_removed++;
          } else {
            move_on = TRUE;
          }
        } else {
          move_on = TRUE;
        }
      }
    }

    n = 1;
    for (k = 1; k <= num_gain_values + 1; k++) {
      if (!ptr_remove[k]) {
        ptr_gain_buf[n] = ptr_gain_buf[k];
        ptr_time_buf[n] = ptr_time_buf[k];
        n++;
      }
    }

    n = 0;
    for (k = 0; k < num_gain_values; k++) {
      if (!ptr_remove[k + 1]) {
        pstr_drc_group_for_output->ts_gain_quant[n] = pstr_drc_group_for_output->ts_gain_quant[k];
        pstr_drc_group_for_output->time_delta_quant[n] =
            pstr_drc_group_for_output->time_delta_quant[k];
        pstr_drc_group_for_output->slope_quant[n] = pstr_drc_group_for_output->slope_quant[k];
        pstr_drc_group_for_output->slope_code_index[n] =
            pstr_drc_group_for_output->slope_code_index[k];
        pstr_drc_group_for_output->gain_code[n] = pstr_drc_group_for_output->gain_code[k];
        pstr_drc_group_for_output->gain_code_length[n] =
            pstr_drc_group_for_output->gain_code_length[k];
        pstr_drc_group_for_output->drc_gain_quant[n] =
            pstr_drc_group_for_output->drc_gain_quant[k];
        n++;
      }
    }
    num_gain_values = n;
  }

  for (k = 1; k <= num_gain_values; k++) {
    if ((ptr_gain_buf[k - 1] < ptr_gain_buf[k]) && (ptr_gain_buf[k] > ptr_gain_buf[k + 1])) {
      pstr_drc_group_for_output->slope_code_index[k - 1] = cod_slope_zero;
      pstr_drc_group_for_output->slope_quant[k - 1] = 0.0f;
    }
    if ((ptr_gain_buf[k - 1] > ptr_gain_buf[k]) && (ptr_gain_buf[k] < ptr_gain_buf[k + 1])) {
      pstr_drc_group_for_output->slope_code_index[k - 1] = cod_slope_zero;
      pstr_drc_group_for_output->slope_quant[k - 1] = 0.0f;
    }
  }

  if (ptr_gain_buf[0] == ptr_gain_buf[1]) {
    pstr_drc_group_for_output->slope_code_index[0] = cod_slope_zero;
    pstr_drc_group_for_output->slope_quant[0] = 0.0f;
  }
  for (k = 0; k < num_gain_values - 1; k++) {
    if (ptr_gain_buf[k + 1] == ptr_gain_buf[k + 2]) {
      pstr_drc_group_for_output->slope_code_index[k] = cod_slope_zero;
      pstr_drc_group_for_output->slope_code_index[k + 1] = cod_slope_zero;
      pstr_drc_group_for_output->slope_quant[k] = 0.0f;
      pstr_drc_group_for_output->slope_quant[k + 1] = 0.0f;
    }
  }
  if (ptr_gain_buf[k + 1] == ptr_gain_buf[k + 2]) {
    pstr_drc_group_for_output->slope_code_index[k] = cod_slope_zero;
    pstr_drc_group_for_output->slope_quant[k] = 0.0f;
  }

  ptr_slope_code_index_buf[0] = pstr_drc_group_for_output->slope_code_index_prev;
  for (k = 0; k < num_gain_values; k++) {
    ptr_slope_code_index_buf[k + 1] = pstr_drc_group_for_output->slope_code_index[k];
  }
  ptr_slope_code_index_buf[k + 1] = pstr_drc_group_for_output->slope_code_index_next;

  for (k = 0; k <= num_gain_values + 1; k++) {
    ptr_remove[k] = FALSE;
  }

  if (num_gain_values > 1) {
    left = 0;
    mid = 1;
    right = 2;
    n_removed = 0;
    while ((right <= num_gain_values + 1) && (num_gain_values - n_removed > 1)) {
      if (((ptr_time_buf[mid] - ptr_time_buf[left]) > 0) &&
          (FLOAT32)fabs((FLOAT64)(ptr_gain_buf[left] - ptr_gain_buf[right])) <
              MAX_DRC_GAIN_DELTA_BEFORE_QUANT) {
        slope_of_nodes_left =
            (ptr_gain_buf[mid] - ptr_gain_buf[left]) / (ptr_time_buf[mid] - ptr_time_buf[left]);
        slope_of_nodes_right =
            (ptr_gain_buf[right] - ptr_gain_buf[mid]) / (ptr_time_buf[right] - ptr_time_buf[mid]);

        if (slope_of_nodes_left >= 0.0f) {
          if ((slope_of_nodes_left < slope_of_nodes_right * SLOPE_CHANGE_THR) &&
              (slope_of_nodes_left * SLOPE_CHANGE_THR > slope_of_nodes_right)) {
            slope_average = 0.5f * time_delta_min * (slope_of_nodes_left + slope_of_nodes_right);
            thr_low = slope_average / SLOPE_QUANT_THR;
            thr_high = slope_average * SLOPE_QUANT_THR;
            slope_0 = impd_drc_decode_slope_idx_value(ptr_slope_code_index_buf[left]);
            slope_1 = impd_drc_decode_slope_idx_value(ptr_slope_code_index_buf[mid]);
            slope_2 = impd_drc_decode_slope_idx_value(ptr_slope_code_index_buf[right]);

            if (((slope_0 < thr_high) && (slope_0 > thr_low)) &&
                ((slope_1 < thr_high) && (slope_1 > thr_low)) &&
                ((slope_2 < thr_high) && (slope_2 > thr_low)) &&
                (ptr_time_buf[mid] != time_mandatory_node)) {
              ptr_remove[mid] = TRUE;
              n_removed++;
              mid = right;
              right++;
            } else {
              left = mid;
              mid = right;
              right++;
            }
          } else {
            left = mid;
            mid = right;
            right++;
          }
        } else {
          if ((-slope_of_nodes_left < -slope_of_nodes_right * SLOPE_CHANGE_THR) &&
              (-slope_of_nodes_left * SLOPE_CHANGE_THR > -slope_of_nodes_right)) {
            slope_average = -0.5f * time_delta_min * (slope_of_nodes_left + slope_of_nodes_right);
            thr_low = slope_average / SLOPE_QUANT_THR;
            thr_high = slope_average * SLOPE_QUANT_THR;
            slope_0 = -impd_drc_decode_slope_idx_value(ptr_slope_code_index_buf[left]);
            slope_1 = -impd_drc_decode_slope_idx_value(ptr_slope_code_index_buf[mid]);
            slope_2 = -impd_drc_decode_slope_idx_value(ptr_slope_code_index_buf[right]);

            if (((slope_0 < thr_high) && (slope_0 > thr_low)) &&
                ((slope_1 < thr_high) && (slope_1 > thr_low)) &&
                ((slope_2 < thr_high) && (slope_2 > thr_low)) &&
                (ptr_time_buf[mid] != time_mandatory_node)) {
              ptr_remove[mid] = TRUE;
              n_removed++;
              mid = right;
              right++;
            } else {
              left = mid;
              mid = right;
              right++;
            }
          } else {
            left = mid;
            mid = right;
            right++;
          }
        }
      } else {
        left = mid;
        mid = right;
        right++;
      }
    }

    n = 1;
    for (k = 1; k <= num_gain_values + 1; k++) {
      if (!ptr_remove[k]) {
        ptr_time_buf[n] = ptr_time_buf[k];
        ptr_gain_buf[n] = ptr_gain_buf[k];
        ptr_slope_code_index_buf[n] = ptr_slope_code_index_buf[k];
        n++;
      }
    }

    n = 0;
    for (k = 0; k < num_gain_values; k++) {
      if (!ptr_remove[k + 1]) {
        pstr_drc_group_for_output->ts_gain_quant[n] = pstr_drc_group_for_output->ts_gain_quant[k];
        pstr_drc_group_for_output->time_delta_quant[n] =
            pstr_drc_group_for_output->time_delta_quant[k];
        pstr_drc_group_for_output->gain_code[n] = pstr_drc_group_for_output->gain_code[k];
        pstr_drc_group_for_output->gain_code_length[n] =
            pstr_drc_group_for_output->gain_code_length[k];
        pstr_drc_group_for_output->slope_quant[n] = pstr_drc_group_for_output->slope_quant[k];
        pstr_drc_group_for_output->slope_code_index[n] =
            pstr_drc_group_for_output->slope_code_index[k];
        pstr_drc_group_for_output->drc_gain_quant[n] =
            pstr_drc_group_for_output->drc_gain_quant[k];
        n++;
      }
    }
    num_gain_values = n;
  }
  pstr_drc_group_for_output->n_gain_values = num_gain_values;

  k = 0;
  while (repeat_check) {
    repeat_check = FALSE;

    while (k < num_gain_values) {
      if (slope_changed) {
        slope_changed = FALSE;
      } else {
        k++;
      }
      if ((ptr_slope_code_index_buf[k] != cod_slope_zero) ||
          (ptr_slope_code_index_buf[k + 1] != cod_slope_zero)) {
        impd_drc_check_overshoot(ptr_time_buf[k + 1] - ptr_time_buf[k], ptr_gain_buf[k],
                                 ptr_gain_buf[k + 1],
                                 impd_drc_decode_slope_idx_value(ptr_slope_code_index_buf[k]),
                                 impd_drc_decode_slope_idx_value(ptr_slope_code_index_buf[k + 1]),
                                 time_delta_min, &overshoot_left, &overshoot_right);

        if (overshoot_right || overshoot_left) {
          if ((k == 0) ||
              (impd_drc_decode_slope_idx_magnitude(ptr_slope_code_index_buf[k]) <
               impd_drc_decode_slope_idx_magnitude(ptr_slope_code_index_buf[k + 1]))) {
            if (ptr_slope_code_index_buf[k + 1] < cod_slope_zero) {
              ptr_slope_code_index_buf[k + 1] = ptr_slope_code_index_buf[k + 1] + 1;
              slope_changed = TRUE;
            } else if (ptr_slope_code_index_buf[k + 1] > cod_slope_zero) {
              ptr_slope_code_index_buf[k + 1] = ptr_slope_code_index_buf[k + 1] - 1;
              slope_changed = TRUE;
            }
          } else if ((k == num_gain_values) ||
                     (impd_drc_decode_slope_idx_magnitude(ptr_slope_code_index_buf[k]) >
                      impd_drc_decode_slope_idx_magnitude(ptr_slope_code_index_buf[k + 1]))) {
            if (ptr_slope_code_index_buf[k] < cod_slope_zero) {
              ptr_slope_code_index_buf[k] = ptr_slope_code_index_buf[k] + 1;
              slope_changed = TRUE;
              repeat_check = TRUE;
            } else if (ptr_slope_code_index_buf[k] > cod_slope_zero) {
              ptr_slope_code_index_buf[k] = ptr_slope_code_index_buf[k] - 1;
              slope_changed = TRUE;
              repeat_check = TRUE;
            }
          }
        }
      }
    }
  }
  for (k = 0; k < num_gain_values; k++) {
    pstr_drc_group_for_output->slope_code_index[k] = ptr_slope_code_index_buf[k + 1];
    pstr_drc_group_for_output->slope_quant[k] =
        impd_drc_decode_slope_idx_value(ptr_slope_code_index_buf[k + 1]);
  }

  for (n = 0; n < num_gain_values; n++) {
    pstr_drc_group_for_output->time_delta_code_index[n] =
        MAX((pstr_drc_group_for_output->ts_gain_quant[n] - time_prev) / time_delta_min, 1);

    time_prev += (pstr_drc_group_for_output->time_delta_code_index[n]) * time_delta_min;

    if (n != 0) {
      delta_gain = pstr_drc_group_for_output->drc_gain_quant[n] - drc_gain_quant_prev;
      impd_drc_get_quantized_delta_drc_gain(
          pstr_drc_gain_seq_buf->str_gain_set_params.gain_coding_profile, delta_gain,
          &delta_gain_quant, &(pstr_drc_group_for_output->gain_code_length[n]),
          &(pstr_drc_group_for_output->gain_code[n]));
      gain_value_quant = delta_gain_quant + drc_gain_quant_prev;
    } else {
      err_code = impd_drc_enc_initial_gain(
          pstr_drc_gain_seq_buf->str_gain_set_params.gain_coding_profile,
          pstr_drc_group_for_output->drc_gain_quant[n], &gain_value_quant,
          &(pstr_drc_group_for_output->gain_code_length[n]),
          &(pstr_drc_group_for_output->gain_code[n]));
      if (err_code) {
        return err_code;
      }
    }
    drc_gain_quant_prev = gain_value_quant;
    pstr_drc_group_for_output->drc_gain_quant[n] = gain_value_quant;
  }

  pstr_drc_group_for_output->coding_mode = 1;
  if (num_gain_values == 1) {
    if (pstr_drc_gain_seq_buf->str_gain_set_params.gain_interpolation_type !=
        GAIN_INTERPOLATION_TYPE_SPLINE) {
      if (pstr_drc_group_for_output->time_delta_code_index[0] >
          (pstr_gain_enc->drc_frame_size / pstr_gain_enc->delta_tmin)) {
        pstr_drc_group_for_output->coding_mode = 0;
      }
    } else {
      if (impd_drc_decode_slope_idx_magnitude(ptr_slope_code_index_buf[1]) == 0.0f) {
        if ((pstr_drc_group_for_output->time_delta_code_index[0] == 0) ||
            (pstr_drc_group_for_output->time_delta_code_index[0] > 28)) {
          pstr_drc_group_for_output->coding_mode = 0;
        }
        if ((impd_drc_decode_slope_idx_magnitude(ptr_slope_code_index_buf[0]) == 0.0f) &&
            (impd_drc_decode_slope_idx_magnitude(ptr_slope_code_index_buf[2]) == 0.0f) &&
            ((FLOAT32)fabs((FLOAT64)(ptr_gain_buf[1] - ptr_gain_buf[0])) < 0.126f) &&
            ((FLOAT32)fabs((FLOAT64)(ptr_gain_buf[2] - ptr_gain_buf[1])) < 0.126f)) {
          pstr_drc_group_for_output->coding_mode = 0;
        }
      }
    }
  }

  if (pstr_drc_group_for_output->coding_mode == 1) {
    pstr_slope_code_table = impd_drc_get_slope_code_table_by_value();
    for (n = 0; n < num_gain_values; n++) {
      pstr_drc_group_for_output->slope_code_size[n] =
          pstr_slope_code_table[ptr_slope_code_index_buf[n + 1]].size;
      pstr_drc_group_for_output->slope_code[n] =
          pstr_slope_code_table[ptr_slope_code_index_buf[n + 1]].code;
    }

    for (n = 0; n < num_gain_values; n++) {
      pstr_drc_group_for_output->time_delta_code_size[n] =
          pstr_delta_time_code_table[pstr_drc_group_for_output->time_delta_code_index[n]].size;
      pstr_drc_group_for_output->time_delta_code[n] =
          pstr_delta_time_code_table[pstr_drc_group_for_output->time_delta_code_index[n]].code;
    }
  }
  return err_code;
}

static IA_ERRORCODE impd_drc_quantize_drc_frame(
    const WORD32 drc_frame_size, const WORD32 time_delta_min, const WORD32 num_gain_values_max,
    const FLOAT32 *ptr_drc_gain_per_sample_with_prev_frame,
    const WORD32 *ptr_delta_time_quant_table, const WORD32 gain_coding_profile,
    ia_drc_group_struct *pstr_drc_group,
    ia_drc_group_for_output_struct *pstr_drc_group_for_output) {
  LOOPIDX i, n;
  WORD32 t, k = 0;
  WORD32 num_bits = 0, code = 0, tmp;
  WORD32 t_left, t_right;
  WORD32 time_delta_left, time_delta_right;
  WORD32 restart = TRUE;
  WORD32 num_drc_gain_values = pstr_drc_group->n_gain_values;

  FLOAT32 slope;
  FLOAT32 delta_gain;
  FLOAT32 gain_value_quant = 0;
  FLOAT32 delta_gain_quant;
  FLOAT32 max_time_deviation;
  FLOAT32 drc_gain_per_sample_limited;

  WORD32 *ptr_time_at_node = pstr_drc_group->ts_gain;
  WORD32 *ptr_ts_gain_quant = pstr_drc_group->ts_gain_quant;
  WORD32 *ptr_slope_code_index = pstr_drc_group->slope_code_index;
  FLOAT32 *drc_gain_quant_prev = &(pstr_drc_group->drc_gain_quant_prev);
  FLOAT32 *ptr_gain_at_node = pstr_drc_group->drc_gain;
  FLOAT32 *ptr_slope_at_node = pstr_drc_group->slope;
  FLOAT32 *ptr_slope_quant = pstr_drc_group->slope_quant;
  const FLOAT32 *ptr_drc_gain_per_sample =
      ptr_drc_gain_per_sample_with_prev_frame + drc_frame_size;
  IA_ERRORCODE err_code = IA_NO_ERROR;

  while (restart) {
    n = 0;
    restart = FALSE;
    while ((n < num_drc_gain_values) && (restart == FALSE)) {
      if (n == 0) {
        time_delta_left = ptr_time_at_node[n];
        time_delta_right = ptr_time_at_node[n + 1] - ptr_time_at_node[n];
      } else if (n < num_drc_gain_values - 1) {
        time_delta_left = ptr_time_at_node[n] - ptr_time_at_node[n - 1];
        time_delta_right = ptr_time_at_node[n + 1] - ptr_time_at_node[n];
      } else {
        time_delta_left = ptr_time_at_node[n] - ptr_time_at_node[n - 1];
        time_delta_right = drc_frame_size - ptr_time_at_node[n];
      }
      max_time_deviation = MAX_TIME_DEVIATION_FACTOR * MIN(time_delta_left, time_delta_right);
      max_time_deviation = MAX(time_delta_min, max_time_deviation);

      i = 0;
      while ((i < num_gain_values_max - 2) && (ptr_delta_time_quant_table[i] < time_delta_left)) {
        i++;
      }
      if (i > 0) {
        if (ptr_delta_time_quant_table[i] - time_delta_left >
            time_delta_left - ptr_delta_time_quant_table[i - 1]) {
          i--;
        }
        if (ptr_delta_time_quant_table[i] >= drc_frame_size) {
          i--;
        }
      }
      if (abs(ptr_delta_time_quant_table[i] - time_delta_left) > max_time_deviation) {
        if (ptr_delta_time_quant_table[i] > time_delta_left) {
          i--;
        }
        for (k = num_drc_gain_values; k > n; k--) {
          ptr_time_at_node[k] = ptr_time_at_node[k - 1];
          ptr_slope_at_node[k] = ptr_slope_at_node[k - 1];
          ptr_gain_at_node[k] = ptr_gain_at_node[k - 1];
        }
        if (n <= 0) {
          ptr_time_at_node[n] = ptr_delta_time_quant_table[i];
        } else {
          ptr_time_at_node[n] = ptr_time_at_node[n - 1] + ptr_delta_time_quant_table[i];
        }

        t = ptr_time_at_node[n];
        ptr_gain_at_node[n] = ptr_drc_gain_per_sample[t];
        t_left = MAX(0, t - time_delta_min / 2);
        t_right = MIN(drc_frame_size, t_left + time_delta_min / 2);
        ptr_slope_at_node[n] = ptr_drc_gain_per_sample[t_right] - ptr_drc_gain_per_sample[t_left];
        num_drc_gain_values++;
        restart = TRUE;
      }
      n++;
    }
  }

  ptr_ts_gain_quant[0] =
      (WORD32)(time_delta_min * (ptr_time_at_node[0] + 0.5f) / (FLOAT32)time_delta_min);
  k = 1;
  for (n = 1; n < num_drc_gain_values; n++) {
    tmp = (WORD32)(time_delta_min * (ptr_time_at_node[n] + 0.5f) / (FLOAT32)time_delta_min);
    if (tmp > ptr_ts_gain_quant[k - 1]) {
      ptr_ts_gain_quant[k] = tmp;
      k++;
    }
  }

  num_drc_gain_values = k;
  pstr_drc_group->n_gain_values = num_drc_gain_values;
  for (n = 0; n < num_drc_gain_values; n++) {
    ptr_gain_at_node[n] = ptr_drc_gain_per_sample[ptr_ts_gain_quant[n]];
    drc_gain_per_sample_limited =
        impd_drc_limit_drc_gain(gain_coding_profile, ptr_gain_at_node[n]);

    if (n != 0) {
      delta_gain = drc_gain_per_sample_limited - *drc_gain_quant_prev;
      impd_drc_get_quantized_delta_drc_gain(gain_coding_profile, delta_gain, &delta_gain_quant,
                                            &num_bits, &code);
      gain_value_quant = delta_gain_quant + *drc_gain_quant_prev;
    } else {
      err_code = impd_drc_enc_initial_gain(gain_coding_profile, drc_gain_per_sample_limited,
                                           &gain_value_quant, &num_bits, &code);
      if (err_code) {
        return err_code;
      }
    }
    pstr_drc_group->gain_code[n] = code;
    pstr_drc_group->gain_code_length[n] = num_bits;
    pstr_drc_group->drc_gain_quant[n] = gain_value_quant;
    *drc_gain_quant_prev = gain_value_quant;

    t_right = MIN(drc_frame_size - 1, ptr_ts_gain_quant[n] + time_delta_min / 2);
    t_left = t_right - time_delta_min;
    slope = ptr_drc_gain_per_sample[t_right] - ptr_drc_gain_per_sample[t_left];
    ptr_slope_at_node[n] = slope;
    impd_drc_quantize_slope(slope, &(ptr_slope_quant[n]), &(ptr_slope_code_index[n]));
  }

  pstr_drc_group->n_gain_values = num_drc_gain_values;
  pstr_drc_group->gain_prev_node = ptr_gain_at_node[num_drc_gain_values - 1];
  pstr_drc_group_for_output->time_quant_next = pstr_drc_group->ts_gain_quant[0] + drc_frame_size;
  pstr_drc_group_for_output->slope_code_index_next = pstr_drc_group->slope_code_index[0];
  pstr_drc_group_for_output->drc_gain_quant_next = pstr_drc_group->drc_gain_quant[0];
  pstr_drc_group_for_output->drc_gain_quant_prev = pstr_drc_group->drc_gain_quant_prev;

  return err_code;
}

IA_ERRORCODE impd_drc_quantize_and_encode_drc_gain(
    ia_drc_gain_enc_struct *pstr_gain_enc, const FLOAT32 *ptr_drc_gain_per_sample,
    FLOAT32 *ptr_drc_gain_per_sample_with_prev_frame,
    ia_drc_delta_time_code_table_entry_struct *pstr_delta_time_code_table,
    ia_drc_gain_seq_buf_struct *pstr_drc_gain_seq_buf, VOID *pstr_scratch) {
  WORD32 drc_frame_size = pstr_gain_enc->drc_frame_size;
  const WORD32 *ptr_delta_time_quant_table = pstr_gain_enc->delta_time_quant_table;
  ia_drc_group_struct *pstr_drc_group;
  ia_drc_group_for_output_struct *pstr_drc_group_for_output;
  IA_ERRORCODE err_code = IA_NO_ERROR;
  impd_drc_advance_nodes(pstr_gain_enc, pstr_drc_gain_seq_buf);

  pstr_drc_group = &(pstr_drc_gain_seq_buf->str_drc_group);
  pstr_drc_group_for_output = &(pstr_drc_gain_seq_buf->str_drc_group_for_output);

  impd_drc_get_preliminary_nodes(
      pstr_gain_enc, ptr_drc_gain_per_sample, ptr_drc_gain_per_sample_with_prev_frame,
      pstr_drc_group, pstr_drc_gain_seq_buf->str_gain_set_params.full_frame, pstr_scratch);

  err_code = impd_drc_quantize_drc_frame(
      drc_frame_size, pstr_gain_enc->delta_tmin,
      pstr_gain_enc->drc_frame_size / pstr_gain_enc->delta_tmin,
      ptr_drc_gain_per_sample_with_prev_frame, ptr_delta_time_quant_table,
      pstr_drc_gain_seq_buf->str_gain_set_params.gain_coding_profile, pstr_drc_group,
      pstr_drc_group_for_output);

  if (err_code) {
    return err_code;
  }

  err_code = impd_drc_post_process_nodes(pstr_gain_enc, pstr_delta_time_code_table,
                                         pstr_drc_gain_seq_buf, pstr_scratch);
  return err_code;
}
