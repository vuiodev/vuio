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
#include "ixheaac_type_def.h"
#include "ixheaac_error_standards.h"
#include "ixheaace_error_codes.h"
#include "ixheaace_mps_common_fix.h"
#include "ixheaace_mps_defines.h"
#include "ixheaace_mps_common_define.h"
#include "ixheaace_mps_onset_detect.h"
#include "ixheaace_mps_vector_functions.h"
#include "ixheaac_constants.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaac_basic_ops.h"

IA_ERRORCODE ixheaace_mps_212_onset_detect_init(
    ixheaace_mps_pstr_onset_detect pstr_onset_detect,
    const ixheaace_mps_pstr_onset_detect_config pstr_onset_detect_config,
    const UWORD32 init_flag) {
  IA_ERRORCODE error = IA_NO_ERROR;
  WORD32 slot;
  if ((pstr_onset_detect_config->max_time_slots > pstr_onset_detect->max_time_slots) ||
      (pstr_onset_detect_config->upper_bound_onset_detection <
       pstr_onset_detect->lower_bound_onset_detection)) {
    return IA_EXHEAACE_CONFIG_NONFATAL_MPS_INVALID_CONFIG;
  }
  pstr_onset_detect->avg_energy_dist_scale = 4;
  pstr_onset_detect->min_trans_dist = 8;
  pstr_onset_detect->avg_energy_dist = 16;
  pstr_onset_detect->lower_bound_onset_detection =
      pstr_onset_detect_config->lower_bound_onset_detection;
  pstr_onset_detect->upper_bound_onset_detection =
      pstr_onset_detect_config->upper_bound_onset_detection;
  pstr_onset_detect->max_time_slots = pstr_onset_detect_config->max_time_slots;
  if (init_flag) {
    for (slot = 0; slot < pstr_onset_detect->avg_energy_dist + pstr_onset_detect->max_time_slots;
         slot++) {
      pstr_onset_detect->p_energy_hist[slot] = EPSILON;
    }
  }
  return error;
}

IA_ERRORCODE
ixheaace_mps_212_onset_detect_update(ixheaace_mps_pstr_onset_detect pstr_onset_detect,
                                     const WORD32 time_slots) {
  IA_ERRORCODE error = IA_NO_ERROR;
  WORD32 slot;
  if (time_slots > pstr_onset_detect->max_time_slots) {
    error = IA_EXHEAACE_CONFIG_NONFATAL_MPS_INVALID_CONFIG;
  } else {
    for (slot = 0; slot < pstr_onset_detect->avg_energy_dist; slot++) {
      pstr_onset_detect->p_energy_hist[slot] =
          pstr_onset_detect->p_energy_hist[slot + time_slots];
    }

    for (slot = 0; slot < time_slots; slot++) {
      pstr_onset_detect->p_energy_hist[pstr_onset_detect->avg_energy_dist + slot] = EPSILON;
    }
  }
  return error;
}

IA_ERRORCODE ixheaace_mps_212_onset_detect_apply(
    ixheaace_mps_pstr_onset_detect pstr_onset_detect, const WORD32 num_time_slots,
    ixheaace_cmplx_str pp_hybrid_data[MAX_ANA_TIME_SLOT][MAX_QMF_BANDS], const WORD32 prev_pos,
    WORD32 p_transient_pos[MAX_NUM_TRANS]) {
  IA_ERRORCODE error = IA_NO_ERROR;
  WORD32 slot, tran_cnt, curr_pos;
  WORD32 trans;
  WORD32 lower_bound_onset_detection = pstr_onset_detect->lower_bound_onset_detection;
  WORD32 upper_bound_onset_detection = pstr_onset_detect->upper_bound_onset_detection;
  WORD32 avg_energy = pstr_onset_detect->avg_energy_dist;
  WORD32 curr_pos_prev;
  FLOAT32 p1, p2;
  FLOAT32 threshold_square = (FLOAT32)SPACE_ONSET_THRESHOLD_SQUARE_FLT;
  FLOAT32 *ptr_energy = pstr_onset_detect->p_energy_hist;
  FLOAT32 energy[16 + MAX_TIME_SLOTS];
  memset(energy, 0, (16 + MAX_TIME_SLOTS) * sizeof(FLOAT32));
  tran_cnt = 0;
  p2 = 0.0f;
  for (trans = 0; trans < MAX_NUM_TRANS; trans++) {
    p_transient_pos[trans] = -1;
  }

  if (prev_pos <= 0) {
    curr_pos = num_time_slots;
  } else {
    curr_pos = MAX(num_time_slots, prev_pos - num_time_slots + pstr_onset_detect->min_trans_dist);
  }

  for (slot = 0; slot < num_time_slots; slot++) {
    ptr_energy[avg_energy + slot] = ixheaace_mps_212_sum_up_cplx_pow_2(
        &pp_hybrid_data[slot][lower_bound_onset_detection + 1],
        upper_bound_onset_detection - lower_bound_onset_detection - 1);
  }

  for (slot = 0; slot < (num_time_slots + avg_energy); slot++) {
    energy[slot] = ptr_energy[slot];
  }

  curr_pos_prev = curr_pos;
  for (; (curr_pos < (num_time_slots << 1)) && (tran_cnt < MAX_NUM_TRANS); curr_pos++) {
    p1 = energy[curr_pos - num_time_slots + avg_energy] * threshold_square;

    if (curr_pos_prev != (curr_pos - 1)) {
      p2 = 0.0f;
      for (slot = 0; slot < avg_energy; slot++) {
        p2 += energy[curr_pos - num_time_slots + slot];
      }
    } else {
      p2 += energy[curr_pos - num_time_slots + avg_energy - 1];
#ifdef _WIN32
#pragma warning(suppress : 6385)
#endif
      p2 -= energy[curr_pos_prev - num_time_slots];
    }
    curr_pos_prev = curr_pos;

    if (p1 > p2) {
      p_transient_pos[tran_cnt++] = curr_pos;
      curr_pos += pstr_onset_detect->min_trans_dist;
    }
  }

  return error;
}
