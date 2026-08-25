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
#include "ixheaac_constants.h"
#include "ixheaace_aac_constants.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops16.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaac_basic_ops.h"

#include "ixheaace_common_rom.h"
#include "ixheaace_sbr_header.h"
#include "ixheaace_sbr_def.h"
#include "ixheaace_resampler.h"
#include "ixheaace_sbr_rom.h"
#include "ixheaace_sbr_tran_det.h"
#include "ixheaace_sbr_main.h"
#include "ixheaace_sbr_frame_info_gen.h"

static VOID ixheaace_calc_thresholds(FLOAT32 **ptr_energies, WORD32 num_cols, WORD32 num_rows,
                                     FLOAT32 *ptr_thresholds, ixheaace_sbr_codec_type sbr_codec) {
  FLOAT32 mean_val, std_val, thr;
  FLOAT32 *ptr_energy;
  FLOAT32 inv_num_cols = 1.0f / (FLOAT32)(num_cols + num_cols / 2);
  FLOAT32 inv_num_cols_1 = 1.0f / (FLOAT32)(num_cols + num_cols / 2 - 1);

  WORD32 i = 0;
  WORD32 j;

  while (i < num_rows) {
    mean_val = std_val = 0;

    j = num_cols >> 2;
    while (j < num_cols) {
      ptr_energy = &ptr_energies[j][i];
      mean_val += (*ptr_energy);
      ptr_energy += 64;
      mean_val += (*ptr_energy);
      j += 2;
    }

    mean_val *= inv_num_cols * 2.0f;

    j = num_cols >> 2;
    while (j < num_cols) {
      FLOAT32 tmp_var;
      tmp_var = (sbr_codec == HEAAC_SBR) ? mean_val - ptr_energies[j][i] : ptr_energies[j][i];
      std_val += tmp_var * tmp_var;
      j++;
    }

    std_val = (FLOAT32)((sbr_codec == HEAAC_SBR)
                            ? sqrt(std_val * inv_num_cols_1)
                            : sqrt(fabs((mean_val * mean_val) - std_val * inv_num_cols * 2.0f)));

    thr = 0.66f * ptr_thresholds[i] + 0.34f * IXHEAACE_SBR_TRAN_STD_FAC * std_val;
    ptr_thresholds[i] = MAX(thr, IXHEAACE_SBR_TRAN_ABS_THR);

    i++;
  }
}

static VOID ixheaace_extract_transient_candidates(FLOAT32 **ptr_energies, FLOAT32 *ptr_thresholds,
                                                  FLOAT32 *ptr_transients, WORD32 num_cols,
                                                  WORD32 start_band, WORD32 stop_band,
                                                  WORD32 buf_len)

{
  WORD32 idx;
  WORD32 buf_move = buf_len >> 1;
  FLOAT32 dt_1, dt_2, dt_3, inv_thr;
  WORD32 len = num_cols + (num_cols >> 1) - 3;
  WORD32 band = start_band;

  memmove(ptr_transients, ptr_transients + num_cols, buf_move * sizeof(ptr_transients[0]));
  memset(ptr_transients + buf_move, 0, (buf_len - buf_move) * sizeof(ptr_transients[0]));

  while (band < stop_band) {
    inv_thr = (FLOAT32)1.0f / ptr_thresholds[band];
    FLOAT32 temp_energy_1 = ptr_energies[((num_cols >> 1) - 2) / 2][band];
    FLOAT32 temp_energy_2 = ptr_energies[((num_cols >> 1)) / 2][band];
    FLOAT32 temp_energy_3 = ptr_energies[((num_cols >> 1) + 2) / 2][band];
    for (idx = 0; idx < len; idx++) {
      if (!idx) {
        dt_1 = temp_energy_2 - temp_energy_1;
        dt_2 = temp_energy_3 - temp_energy_1;
        dt_3 = temp_energy_3 - ptr_energies[((num_cols >> 1) - 4) / 2][band];
      } else {
        FLOAT32 temp_energy_4 = ptr_energies[(idx + (num_cols >> 1) + 3) / 2][band];
        dt_1 = temp_energy_3 - temp_energy_2;
        dt_2 = temp_energy_3 - temp_energy_1 + dt_1;
        dt_3 = temp_energy_4 - temp_energy_1 + dt_2;
        temp_energy_1 = temp_energy_2;
        temp_energy_2 = temp_energy_3;
        temp_energy_3 = temp_energy_4;
      }
      if (dt_1 > ptr_thresholds[band]) {
        ptr_transients[idx + buf_move] += dt_1 * inv_thr - 1.0f;
      }
      if (dt_2 > ptr_thresholds[band]) {
        ptr_transients[idx + buf_move] += dt_2 * inv_thr - 1.0f;
      }
      if (dt_3 > ptr_thresholds[band]) {
        ptr_transients[idx + buf_move] += dt_3 * inv_thr - 1.0f;
      }
    }
    for (idx = 1; idx < len; idx += 2) {
      ptr_transients[idx + buf_move + 1] = ptr_transients[idx + buf_move];
    }
    band++;
  }
}

VOID ixheaace_detect_transient(FLOAT32 **ptr_energies,
                               ixheaace_pstr_sbr_trans_detector pstr_sbr_trans_det,
                               WORD32 *ptr_tran_vector, WORD32 time_step,
                               ixheaace_sbr_codec_type sbr_codec) {
  WORD32 i;
  WORD32 no_cols = pstr_sbr_trans_det->no_cols;
  WORD32 qmf_start_sample = no_cols + time_step * 4;
  FLOAT32 int_thr = (FLOAT32)pstr_sbr_trans_det->tran_thr / (FLOAT32)pstr_sbr_trans_det->no_rows;
  FLOAT32 *ptr_trans = &(pstr_sbr_trans_det->ptr_transients[qmf_start_sample]);

  ptr_tran_vector[0] = 0;
  ptr_tran_vector[1] = 0;

  ixheaace_calc_thresholds(ptr_energies, pstr_sbr_trans_det->no_cols, pstr_sbr_trans_det->no_rows,
                           pstr_sbr_trans_det->ptr_thresholds, sbr_codec);

  ixheaace_extract_transient_candidates(
      ptr_energies, pstr_sbr_trans_det->ptr_thresholds, pstr_sbr_trans_det->ptr_transients,
      pstr_sbr_trans_det->no_cols, 0, pstr_sbr_trans_det->no_rows,
      pstr_sbr_trans_det->buffer_length);

  for (i = 0; i < no_cols; i++) {
    if ((ptr_trans[i] < 0.9f * ptr_trans[i - 1]) && (ptr_trans[i - 1] > int_thr)) {
      ptr_tran_vector[0] = (WORD32)floor(i / time_step);
      ptr_tran_vector[1] = 1;
      break;
    }
  }
}

static VOID ixheaace_calc_thresholds_4_1(FLOAT32 **ptr_energies, WORD32 num_cols, WORD32 num_rows,
                                         FLOAT32 *ptr_thresholds, WORD32 time_step) {
  FLOAT32 mean_val, std_val, thr;
  FLOAT32 *ptr_energy;
  FLOAT32 inv_num_cols = 1.0f / (FLOAT32)((num_cols + num_cols / 2) / time_step);
  FLOAT32 inv_num_cols_1 = 1.0f / (FLOAT32)((num_cols + num_cols / 2 - 1) / time_step);

  WORD32 i = 0;
  WORD32 j;
  WORD32 start_band = 8;
  WORD32 end_band = 32;

  while (i < num_rows) {
    mean_val = std_val = 0;

    j = start_band;
    while (j < end_band) {
      ptr_energy = &ptr_energies[j][i];
      mean_val += (*ptr_energy);
      j++;
    }

    mean_val *= inv_num_cols;

    j = start_band;
    while (j < end_band) {
      FLOAT32 tmp_var;
      tmp_var = mean_val - ptr_energies[j][i];
      std_val += tmp_var * tmp_var;
      j++;
    }

    std_val = (FLOAT32)sqrt(std_val * inv_num_cols_1);

    thr = 0.66f * ptr_thresholds[i] + 0.34f * IXHEAACE_SBR_TRAN_STD_FAC * std_val;
    ptr_thresholds[i] = MAX(thr, IXHEAACE_SBR_TRAN_ABS_THR);

    i++;
  }
}

static VOID ixheaace_extract_transient_candidates_4_1(FLOAT32 **ptr_energies,
                                                      FLOAT32 *ptr_thresholds,
                                                      FLOAT32 *ptr_transients, WORD32 num_cols,
                                                      WORD32 start_band, WORD32 stop_band,
                                                      WORD32 time_step)

{
  WORD32 idx;
  WORD32 buf_move = num_cols / 2;
  WORD32 band = start_band;

  memmove(ptr_transients, ptr_transients + num_cols, buf_move * sizeof(ptr_transients[0]));
  memset(ptr_transients + buf_move, 0, num_cols * sizeof(ptr_transients[0]));

  while (band < stop_band) {
    for (idx = buf_move; idx < num_cols + buf_move; idx++) {
      float l = 0, r = 0;
      for (int d = 1; d < 4; d++) {
        l = ptr_energies[(idx - d) / time_step][band];
        r = ptr_energies[(idx + d) / time_step][band];
        if (r - l > ptr_thresholds[band])
          ptr_transients[idx] += (r - l - ptr_thresholds[band]) / ptr_thresholds[band];
      }
    }
    band++;
  }
}

VOID ixheaace_detect_transient_4_1(FLOAT32 **ptr_energies,
                                   ixheaace_pstr_sbr_trans_detector pstr_sbr_trans_det,
                                   WORD32 *ptr_tran_vector, WORD32 time_step) {
  WORD32 i;
  WORD32 no_cols = pstr_sbr_trans_det->no_cols;
  WORD32 qmf_start_sample = time_step * 4;
  FLOAT32 int_thr = (FLOAT32)pstr_sbr_trans_det->tran_thr / (FLOAT32)pstr_sbr_trans_det->no_rows;
  FLOAT32 *ptr_trans = &(pstr_sbr_trans_det->ptr_transients[qmf_start_sample]);

  ptr_tran_vector[0] = 0;
  ptr_tran_vector[1] = 0;

  ixheaace_calc_thresholds_4_1(ptr_energies, pstr_sbr_trans_det->no_cols,
                               pstr_sbr_trans_det->no_rows, pstr_sbr_trans_det->ptr_thresholds,
                               time_step);

  ixheaace_extract_transient_candidates_4_1(
      ptr_energies, pstr_sbr_trans_det->ptr_thresholds, pstr_sbr_trans_det->ptr_transients,
      pstr_sbr_trans_det->no_cols, 0, pstr_sbr_trans_det->no_rows, time_step);

  for (i = 0; i < no_cols; i++) {
    if ((ptr_trans[i] < 0.9f * ptr_trans[i - 1]) && (ptr_trans[i - 1] > int_thr)) {
      ptr_tran_vector[0] = (WORD32)floor(i / time_step);
      ptr_tran_vector[1] = 1;
      break;
    }
  }
}

VOID ixheaace_detect_transient_eld(FLOAT32 **ptr_energies,
                                   ixheaace_pstr_sbr_trans_detector pstr_sbr_trans_det,
                                   WORD32 *ptr_tran_vector) {
  WORD32 i, band;
  WORD32 max_idx = 0, is_transient = 0;
  FLOAT32 delta_max = 0, tmp, min_energy;
  WORD32 num_slots = pstr_sbr_trans_det->time_slots;
  WORD32 look_ahead = pstr_sbr_trans_det->look_ahead;
  WORD32 start_band = pstr_sbr_trans_det->start_band;
  WORD32 stop_band = pstr_sbr_trans_det->stop_band;
  FLOAT32 *ptr_energy = pstr_sbr_trans_det->energy;
  FLOAT32 *ptr_delta_energy = pstr_sbr_trans_det->delta_energy;
  FLOAT32 *ptr_transients = pstr_sbr_trans_det->ptr_transients;
  WORD32 ts = look_ahead;
  FLOAT32 weighted_energy;

  ptr_tran_vector[0] = ptr_tran_vector[1] = 0;
  ptr_tran_vector[2] = 0;

  memset(ptr_transients + look_ahead, 0, num_slots * sizeof(ptr_transients[0]));

  while (ts < num_slots + look_ahead) {
    tmp = 0.0f;
    max_idx = 0;
    delta_max = 0;
    is_transient = 0;
    i = 0;
    band = start_band;

    while (band < stop_band) {
      tmp += (ptr_energies[ts][band] * pstr_sbr_trans_det->coeff[i]);
      band++;
      i++;
    }

    ptr_energy[ts] = tmp;
    min_energy = (ptr_energy[ts - 1]) + IXHEAACE_SMALL_ENERGY;
    ptr_delta_energy[ts] = ptr_energy[ts] / min_energy;

    weighted_energy = ptr_energy[ts] * (1.0f / 1.4f);

    if ((ptr_delta_energy[ts] >= IXHEAACE_TRANSIENT_THRESHOLD) &&
        (((ptr_transients[ts - 2] == 0) && (ptr_transients[ts - 1] == 0)) ||
         (weighted_energy >= ptr_energy[ts - 1]) || (weighted_energy >= ptr_energy[ts - 2]))) {
      ptr_transients[ts] = 1;
    }

    ts++;
  }

  for (ts = 0; ts < num_slots; ts++) {
    if (ptr_transients[ts] && (ptr_delta_energy[ts] > delta_max)) {
      delta_max = ptr_delta_energy[ts];
      max_idx = ts;
      is_transient = 1;
    }
  }

  if (is_transient) {
    ptr_tran_vector[0] = max_idx;
    ptr_tran_vector[1] = 1;
  }

  for (ts = 0; ts < look_ahead; ts++) {
    if (ptr_transients[ts + num_slots]) {
      ptr_tran_vector[2] = 1;
    }
    ptr_energy[ts] = ptr_energy[num_slots + ts];
    ptr_transients[ts] = ptr_transients[num_slots + ts];
    ptr_delta_energy[ts] = ptr_delta_energy[num_slots + ts];
  }
}
