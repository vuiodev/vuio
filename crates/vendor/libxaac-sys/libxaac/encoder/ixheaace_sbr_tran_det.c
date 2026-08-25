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
#include "ixheaac_error_standards.h"
#include "ixheaace_error_codes.h"
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

static IA_ERRORCODE ixheaace_spectral_change(FLOAT32 *ptr_energies[16], FLOAT32 total_energy,
                                             WORD32 num_sfb, WORD32 start, WORD32 border,
                                             WORD32 stop, WORD32 is_ld_sbr, FLOAT32 *ptr_delta) {
  WORD32 i, j;
  WORD32 len1 = border - start;
  WORD32 len2 = stop - border;
  FLOAT32 energy_1[MAXIMUM_FREQ_COEFFS] = {0};
  FLOAT32 energy_2[MAXIMUM_FREQ_COEFFS] = {0};
  FLOAT32 len_ratio = (FLOAT32)len1 / (FLOAT32)(len2);
  FLOAT32 delta, delta_sum = 0.0f;
  FLOAT32 pos_wt = (0.5f - (FLOAT32)len1 / (FLOAT32)(len1 + len2));
  pos_wt = 1.0f - 4.0f * pos_wt * pos_wt;

  if (total_energy < SBR_EPS) {
    *ptr_delta = 0.0f;
    return IA_NO_ERROR;
  }

  if (!is_ld_sbr) {
    for (j = 0; j < num_sfb; j++) {
      energy_1[j] = 1.0e6f * len1;
      energy_2[j] = 1.0e6f * len2;
    }
  }

  for (j = 0; j < num_sfb; j++) {
    for (i = start; i < border; i++) {
      energy_1[j] += ptr_energies[i][j];
    }

    for (i = border; i < stop; i++) {
      energy_2[j] += ptr_energies[i][j];
    }
    if (energy_1[j] <= EPS) {
      energy_1[j] = (FLOAT32)len1;
    }
    if (energy_2[j] <= EPS) {
      energy_2[j] = (FLOAT32)len2;
    }
    delta = (FLOAT32)fabs(log((energy_2[j] / energy_1[j]) * len_ratio));
    delta_sum += (FLOAT32)(sqrt((energy_1[j] + energy_2[j]) / total_energy) * delta);
  }

  *ptr_delta = delta_sum * pos_wt;
  return IA_NO_ERROR;
}

FLOAT32 ixheaace_add_lowband_energies(FLOAT32 **ptr_energies, UWORD8 *ptr_freq_band_tab,
                                      WORD32 time_slots, WORD32 is_ld_sbr, WORD32 time_step) {
  WORD32 band, ts;
  FLOAT32 energy = 1.0f;
  WORD32 tran_offset = 0;
  if (is_ld_sbr) {
    tran_offset = 7;
    energy = 0.0f;
  } else {
    tran_offset = time_slots / 2;
  }

  for (ts = tran_offset; ts < time_slots + tran_offset; ts++) {
    for (band = 0; band < ptr_freq_band_tab[0]; band++) {
      energy += ptr_energies[ts][band];
    }
  }

  energy *= time_step;

  return energy;
}

static FLOAT32 ixheaace_add_highband_energies(FLOAT32 **ptr_energies, FLOAT32 *ptr_energies_m[16],
                                              UWORD8 *ptr_freq_band_tab, WORD32 num_sfb,
                                              WORD32 time_slots, WORD32 time_step,
                                              WORD32 is_ld_sbr) {
  WORD32 band, ts, sfb, low_band, high_band;
  FLOAT32 energy = 1.0f, tmp;
  if (is_ld_sbr) {
    energy = 0.0f;
  }
  for (ts = 0; ts < time_slots; ts++) {
    for (sfb = 0; sfb < num_sfb; sfb++) {
      tmp = 0;
      low_band = ptr_freq_band_tab[sfb];
      high_band = ptr_freq_band_tab[sfb + 1];
      band = low_band;
      while (band < high_band) {
        tmp += (ptr_energies[ts][band] * time_step);
        band++;
      }
      ptr_energies_m[ts][sfb] = tmp;
      if (is_ld_sbr || time_step == 4) {
        energy += tmp;
      } else {
        energy += ptr_energies[ts][sfb];
      }
    }
  }
  return energy;
}

IA_ERRORCODE
ixheaace_frame_splitter(FLOAT32 **ptr_energies,
                        ixheaace_pstr_sbr_trans_detector pstr_sbr_trans_detector,
                        UWORD8 *ptr_freq_band_tab, WORD32 num_scf, WORD32 time_step,
                        WORD32 no_cols, WORD32 *ptr_tran_vector,
                        FLOAT32 *ptr_frame_splitter_scratch, WORD32 is_ld_sbr) {
  WORD32 border, i;
  WORD32 num_sbr_slots = no_cols / time_step;
  FLOAT32 *ptr_energies_m[16] = {0};
  FLOAT32 low_band_energy, high_band_energy, total_energy;
  FLOAT32 delta = 0.0f;
  IA_ERRORCODE err_code = IA_NO_ERROR;

  if ((num_sbr_slots <= 0) || (num_sbr_slots * time_step != no_cols)) {
    return IA_EXHEAACE_EXE_FATAL_SBR_INVALID_TIME_SLOTS;
  }

  memset(ptr_frame_splitter_scratch, 0,
         sizeof(ptr_energies_m[0][0]) * MAXIMUM_FREQ_COEFFS * num_sbr_slots);

  for (i = 0; i < num_sbr_slots; i++) {
    ptr_energies_m[i] = ptr_frame_splitter_scratch;
    ptr_frame_splitter_scratch += MAXIMUM_FREQ_COEFFS;
  }

  low_band_energy = ixheaace_add_lowband_energies(ptr_energies, ptr_freq_band_tab, num_sbr_slots,
                                                  is_ld_sbr, time_step);

  high_band_energy =
      ixheaace_add_highband_energies(ptr_energies, ptr_energies_m, ptr_freq_band_tab, num_scf,
                                     num_sbr_slots, time_step, is_ld_sbr);
  border = (num_sbr_slots + 1) >> 1;

  total_energy = 0.5f * (low_band_energy + pstr_sbr_trans_detector->prev_low_band_energy);
  total_energy += high_band_energy;
  if ((total_energy > IXHEAACE_SBR_ENERGY_THRESHOLD) || (!is_ld_sbr)) {
    err_code = ixheaace_spectral_change(ptr_energies_m, total_energy, num_scf, 0, border,
                                        num_sbr_slots, is_ld_sbr, &delta);
    if (err_code) {
      return err_code;
    }
  } else if (is_ld_sbr) {
    delta = 0;
  }

  if (delta > pstr_sbr_trans_detector->split_thr) {
    ptr_tran_vector[0] = 1;
  } else {
    ptr_tran_vector[0] = 0;
  }
  pstr_sbr_trans_detector->prev_low_band_energy = low_band_energy;
  return err_code;
}

VOID ixheaace_create_sbr_transient_detector(
    ixheaace_pstr_sbr_trans_detector pstr_sbr_trans_detector, WORD32 sample_freq,
    WORD32 total_bitrate, WORD32 codec_bitrate, WORD32 tran_thr, WORD32 mode, WORD32 tran_fc,
    WORD32 frame_flag_480, WORD32 is_ld_sbr, WORD32 sbr_ratio_idx,
    ixheaace_sbr_codec_type sbr_codec, WORD32 start_band) {
  WORD32 no_cols = 32, buffer_length = 96;
  FLOAT32 br_fac;
  FLOAT32 frm_dur = 2048.0f / (FLOAT32)sample_freq;
  FLOAT32 split_thr_fac = frm_dur - 0.01f;
  if ((sbr_codec == USAC_SBR) && (sbr_ratio_idx == USAC_SBR_RATIO_INDEX_4_1)) {
    frm_dur = frm_dur * 2;
    split_thr_fac = frm_dur - 0.01f;
    no_cols = 64;
  }
  if ((1 == is_ld_sbr) && (1 == frame_flag_480)) {
    no_cols = 30;
    buffer_length = 90;
  }

  memset(pstr_sbr_trans_detector, 0, sizeof(ixheaace_str_sbr_trans_detector));

  br_fac = codec_bitrate ? (FLOAT32)total_bitrate / (FLOAT32)codec_bitrate : 1.0f;

  split_thr_fac = MAX(split_thr_fac, 0.0001f);
  split_thr_fac = 0.000075f / (split_thr_fac * split_thr_fac);

  pstr_sbr_trans_detector->split_thr = split_thr_fac * br_fac;

  if (is_ld_sbr) {
    WORD32 i;
    FLOAT32 ratio = ((sample_freq / 2) / IXHEAACE_QMF_CHANNELS) * 0.00075275f;

    FLOAT32 tmp = 1024.0f / sample_freq;
    tmp -= 0.01f;
    tmp = MAX(tmp, 0.001f);

    if (1 == frame_flag_480) {
      no_cols = 30;
      buffer_length = 90;
    }
    pstr_sbr_trans_detector->split_thr = (br_fac * 0.000075f) / (tmp * tmp) / 2.0f;
    pstr_sbr_trans_detector->look_ahead = 2;
    pstr_sbr_trans_detector->time_slots = no_cols / 2;
    pstr_sbr_trans_detector->buffer_size =
        pstr_sbr_trans_detector->look_ahead + pstr_sbr_trans_detector->time_slots;
    pstr_sbr_trans_detector->stop_band =
        (WORD32)fmin(13500 / ((sample_freq >> 1) / IXHEAACE_QMF_CHANNELS), IXHEAACE_QMF_CHANNELS);
    pstr_sbr_trans_detector->start_band =
        (WORD32)fmin(start_band, pstr_sbr_trans_detector->stop_band - 4);

    memset(pstr_sbr_trans_detector->energy, 0,
           pstr_sbr_trans_detector->buffer_size * sizeof(pstr_sbr_trans_detector->energy[0]));
    memset(pstr_sbr_trans_detector->sbr_transients, 0,
           pstr_sbr_trans_detector->buffer_size *
               sizeof(pstr_sbr_trans_detector->sbr_transients[0]));
    memset(
        pstr_sbr_trans_detector->delta_energy, 0,
        pstr_sbr_trans_detector->buffer_size * sizeof(pstr_sbr_trans_detector->delta_energy[0]));

    for (i = 0; i < 64; i++) {
      pstr_sbr_trans_detector->coeff[i] = (FLOAT32)pow(2.0, ratio * (i + 1));
    }
  }
  pstr_sbr_trans_detector->no_cols = no_cols;
  pstr_sbr_trans_detector->tran_fc = tran_fc;

  pstr_sbr_trans_detector->buffer_length = buffer_length;

  pstr_sbr_trans_detector->no_rows = 64;
  pstr_sbr_trans_detector->mode = mode;

  pstr_sbr_trans_detector->prev_low_band_energy = 0;
  pstr_sbr_trans_detector->tran_thr = (FLOAT32)tran_thr;

  pstr_sbr_trans_detector->ptr_thresholds = &(pstr_sbr_trans_detector->sbr_thresholds[0]);

  memset(pstr_sbr_trans_detector->ptr_thresholds, 0,
         sizeof(pstr_sbr_trans_detector->ptr_thresholds[0]) * IXHEAACE_QMF_CHANNELS);

  pstr_sbr_trans_detector->ptr_transients = &(pstr_sbr_trans_detector->sbr_transients[0]);
  memset(pstr_sbr_trans_detector->ptr_transients, 0,
         sizeof(pstr_sbr_trans_detector->ptr_transients[0]) *
             pstr_sbr_trans_detector->buffer_length);
}
