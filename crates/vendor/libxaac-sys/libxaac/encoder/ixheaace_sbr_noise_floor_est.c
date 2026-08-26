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
#include <math.h>

#include "ixheaac_type_def.h"
#include "ixheaac_constants.h"
#include "ixheaac_error_standards.h"
#include "ixheaace_error_codes.h"
#include "ixheaace_aac_constants.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops16.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaac_basic_ops.h"

#include "ixheaace_sbr_header.h"
#include "ixheaace_sbr_def.h"
#include "ixheaace_resampler.h"
#include "ixheaace_sbr_rom.h"
#include "ixheaace_common_rom.h"
#include "ixheaace_sbr_hbe.h"
#include "ixheaace_sbr_qmf_enc.h"
#include "ixheaace_sbr_tran_det.h"
#include "ixheaace_sbr_frame_info_gen.h"
#include "ixheaace_sbr_env_est.h"
#include "ixheaace_sbr_code_envelope.h"
#include "ixheaace_sbr_main.h"
#include "ixheaace_sbr_missing_harmonics_det.h"
#include "ixheaace_sbr_inv_filtering_estimation.h"
#include "ixheaace_sbr_noise_floor_est.h"

#include "ixheaace_sbr_ton_corr.h"
#include "iusace_esbr_pvc.h"
#include "iusace_esbr_inter_tes.h"
#include "ixheaace_sbr.h"
#include "ixheaace_common_utils.h"

static VOID ia_enhaacplus_enc_smoothing_noise_levels(
    FLOAT32 *ptr_noise_lvls, WORD32 num_env, WORD32 num_noise_bands,
    FLOAT32 prev_noise_lvls[IXHEAACE_NF_SMOOTHING_LENGTH][MAXIMUM_NUM_NOISE_VALUES],
    const FLOAT32 *ptr_smooth_filter, WORD32 transient_flag, WORD32 is_ld_sbr) {
  WORD32 i, band, env;

  for (env = 0; env < num_env; env++) {
    if (is_ld_sbr) {
      if (transient_flag) {
        for (i = 0; i < IXHEAACE_NF_SMOOTHING_LENGTH; i++) {
          memcpy(prev_noise_lvls[i], ptr_noise_lvls + env * num_noise_bands,
                 num_noise_bands * sizeof(prev_noise_lvls[i][0]));
        }
      } else {
        for (i = 1; i < IXHEAACE_NF_SMOOTHING_LENGTH; i++) {
          memcpy(prev_noise_lvls[i - 1], prev_noise_lvls[i],
                 num_noise_bands * sizeof(prev_noise_lvls[i - 1][0]));
        }
      }
    } else {
      for (i = 1; i < IXHEAACE_NF_SMOOTHING_LENGTH; i++) {
        memcpy(prev_noise_lvls[i - 1], prev_noise_lvls[i],
               num_noise_bands * sizeof(prev_noise_lvls[i - 1][0]));
      }
    }
    memcpy(prev_noise_lvls[IXHEAACE_NF_SMOOTHING_LENGTH - 1],
           ptr_noise_lvls + env * num_noise_bands,
           num_noise_bands * sizeof(prev_noise_lvls[IXHEAACE_NF_SMOOTHING_LENGTH - 1][0]));

    for (band = 0; band < num_noise_bands; band++) {
      ptr_noise_lvls[band + env * num_noise_bands] = 0;

      for (i = 0; i < IXHEAACE_NF_SMOOTHING_LENGTH; i++) {
        ptr_noise_lvls[band + env * num_noise_bands] +=
            ptr_smooth_filter[i] * prev_noise_lvls[i][band];
      }
    }
  }
}

static VOID ia_enhaacplus_enc_qmf_based_noise_floor_detection(
    FLOAT32 *ptr_noise_lvl, FLOAT32 **ptr_quota_orig, FLOAT32 weight_fac, FLOAT32 max_lvl,
    FLOAT32 noise_floor_offset, WORD8 *ptr_idx_vx, WORD32 start_index, WORD32 stop_index,
    WORD32 start_channel, WORD32 stop_channel, WORD32 missing_harmonic_flag,
    ixheaace_invf_mode thr_offset, ixheaace_invf_mode inv_filtering_lvl) {
  WORD32 ch, idx;
  FLOAT32 ton_org, ton_sbr, mean_org = 0, mean_sbr = 0, diff;

  if (1 == missing_harmonic_flag) {
    for (ch = start_channel; ch < stop_channel; ch++) {
      ton_org = 0;
      ton_sbr = 0;
      for (idx = start_index; idx < stop_index; idx++) {
        ton_org += ptr_quota_orig[idx][ch];
        ton_sbr += ptr_quota_orig[idx][ptr_idx_vx[ch]];
      }

      ton_org /= (stop_index - start_index);
      ton_sbr /= (stop_index - start_index);

      if (ton_org > mean_org) {
        mean_org = ton_org;
      }

      if (ton_sbr > mean_sbr) {
        mean_sbr = ton_sbr;
      }
    }
  } else {
    for (ch = start_channel; ch < stop_channel; ch++) {
      ton_org = 0;
      ton_sbr = 0;
      for (idx = start_index; idx < stop_index; idx++) {
        ton_org += ptr_quota_orig[idx][ch];
        ton_sbr += ptr_quota_orig[idx][ptr_idx_vx[ch]];
      }

      ton_org /= (stop_index - start_index);
      ton_sbr /= (stop_index - start_index);

      mean_org += ton_org;
      mean_sbr += ton_sbr;
    }
    mean_org /= (stop_channel - start_channel);
    mean_sbr /= (stop_channel - start_channel);
  }

  if (mean_org < SBR_TON_MEAN_P0009 && mean_sbr < SBR_TON_MEAN_P0009) {
    mean_org = mean_sbr = SBR_TON_MEAN_101P59;
  }

  if (mean_org < 1.0f) {
    mean_org = 1.0f;
  }

  if (mean_sbr < 1.0f) {
    mean_sbr = 1.0f;
  }

  if (1 == missing_harmonic_flag) {
    diff = 1.0f;
  } else {
    if (1.0f > (weight_fac * mean_sbr / mean_org)) {
      diff = 1.0f;
    } else {
      diff = weight_fac * mean_sbr / mean_org;
    }
  }

  if (inv_filtering_lvl == IXHEAACE_INVF_MID_LEVEL ||
      inv_filtering_lvl == IXHEAACE_INVF_LOW_LEVEL || inv_filtering_lvl == IXHEAACE_INVF_OFF) {
    diff = 1.0f;
  }

  if (inv_filtering_lvl <= thr_offset) {
    diff = 1.0f;
  }

  *ptr_noise_lvl = diff / mean_org;
  *ptr_noise_lvl *= noise_floor_offset;

  if (*ptr_noise_lvl > max_lvl) {
    *ptr_noise_lvl = max_lvl;
  }
}

VOID ixheaace_sbr_noise_floor_estimate_qmf(
    ixheaace_pstr_noise_flr_est_sbr pstr_noise_floor_est_sbr,
    const ixheaace_str_frame_info_sbr *ptr_frame_info, FLOAT32 *ptr_noise_lvls,
    FLOAT32 **ptr_quota_orig, WORD8 *ptr_idx_vx, WORD32 missing_harmonics_flag,
    WORD32 start_index, WORD32 transient_flag, ixheaace_invf_mode *ptr_inv_filt_levels,
    WORD32 is_ld_sbr) {
  WORD32 n_noise_envelopes, start_pos[2], stop_pos[2], env, band;
  WORD32 num_of_noise_bands = pstr_noise_floor_est_sbr->num_of_noise_bands;
  WORD32 *ptr_freq_band_tab = pstr_noise_floor_est_sbr->s_freq_qmf_band_tbl;

  n_noise_envelopes = ptr_frame_info->n_noise_envelopes;

  if (n_noise_envelopes == 1) {
    start_pos[0] = start_index;
    stop_pos[0] = start_index + 2;
  } else {
    start_pos[0] = start_index;
    stop_pos[0] = start_index + 1;
    start_pos[1] = start_index + 1;
    stop_pos[1] = start_index + 2;
  }

  for (env = 0; env < n_noise_envelopes; env++) {
    for (band = 0; band < num_of_noise_bands; band++) {
      ia_enhaacplus_enc_qmf_based_noise_floor_detection(
          &ptr_noise_lvls[band + env * num_of_noise_bands], ptr_quota_orig,
          pstr_noise_floor_est_sbr->weight_fac, pstr_noise_floor_est_sbr->max_level, 1.0f,
          ptr_idx_vx, start_pos[env], stop_pos[env], ptr_freq_band_tab[band],
          ptr_freq_band_tab[band + 1], missing_harmonics_flag,
          pstr_noise_floor_est_sbr->thr_offset, ptr_inv_filt_levels[band]);
    }
  }

  ia_enhaacplus_enc_smoothing_noise_levels(
      ptr_noise_lvls, n_noise_envelopes, pstr_noise_floor_est_sbr->num_of_noise_bands,
      pstr_noise_floor_est_sbr->prev_noise_lvls, pstr_noise_floor_est_sbr->ptr_smooth_filter,
      transient_flag, is_ld_sbr);

  for (env = 0; env < n_noise_envelopes; env++) {
    for (band = 0; band < num_of_noise_bands; band++) {
      ptr_noise_lvls[band + env * num_of_noise_bands] =
          (FLOAT32)SBR_NOISE_FLOOR_OFFSET -
          (FLOAT32)(SBR_INV_LOG_2 * log(ptr_noise_lvls[band + env * num_of_noise_bands]));
    }
  }
}

static IA_ERRORCODE ia_enhaacplus_enc_down_sample_lo_res(WORD32 *ptr_result, WORD32 num_result,
                                                         const UWORD8 *ptr_freq_band_tab_ref,
                                                         WORD32 num_ref) {
  WORD32 step;
  WORD32 i, j;
  WORD32 org_length, result_length;
  WORD32 v_index[MAXIMUM_FREQ_COEFFS / 2];

  org_length = num_ref;
  result_length = num_result;

  v_index[0] = 0;

  i = 0;

  while (org_length > 0) {
    i++;

    step = org_length / result_length;

    org_length = org_length - step;

    result_length--;

    v_index[i] = v_index[i - 1] + step;
  }

  if (i != num_result) {
    return IA_EXEHAACE_INIT_FATAL_SBR_NOISE_BAND_NOT_SUPPORTED;
  }

  for (j = 0; j <= i; j++) {
    ptr_result[j] = ptr_freq_band_tab_ref[v_index[j]];
  }

  return IA_NO_ERROR;
}

IA_ERRORCODE
ixheaace_create_sbr_noise_floor_estimate(ixheaace_pstr_noise_flr_est_sbr pstr_noise_floor_est_sbr,
                                         WORD32 ana_max_level, const UWORD8 *ptr_freq_band_tab,
                                         WORD32 num_scf, WORD32 noise_groups,
                                         UWORD32 use_speech_config,
                                         ixheaace_str_qmf_tabs *ptr_qmf_tab) {
  memset(pstr_noise_floor_est_sbr, 0, sizeof(ixheaace_str_noise_flr_est_sbr));

  pstr_noise_floor_est_sbr->ptr_smooth_filter = ptr_qmf_tab->ptr_smooth_filter;

  if (use_speech_config) {
    pstr_noise_floor_est_sbr->weight_fac = 1.0f;
    pstr_noise_floor_est_sbr->thr_offset = IXHEAACE_INVF_LOW_LEVEL;
  } else {
    pstr_noise_floor_est_sbr->weight_fac = 0.25f;
    pstr_noise_floor_est_sbr->thr_offset = IXHEAACE_INVF_MID_LEVEL;
  }

  if (ana_max_level == -3) {
    pstr_noise_floor_est_sbr->max_level = 0.5f;
  } else if (ana_max_level == 3) {
    pstr_noise_floor_est_sbr->max_level = 2.0f;
  } else if (ana_max_level == 6) {
    pstr_noise_floor_est_sbr->max_level = 4.0f;
  } else {
  }

  pstr_noise_floor_est_sbr->noise_groups = noise_groups;

  return ixheaace_reset_sbr_noise_floor_estimate(pstr_noise_floor_est_sbr, ptr_freq_band_tab,
                                                 num_scf);
}

IA_ERRORCODE
ixheaace_reset_sbr_noise_floor_estimate(ixheaace_pstr_noise_flr_est_sbr pstr_noise_floor_est_sbr,
                                        const UWORD8 *ptr_freq_band_tab, WORD32 num_scf) {
  WORD32 k2, kx;

  k2 = ptr_freq_band_tab[num_scf];
  kx = ptr_freq_band_tab[0];

  /* ISO_IEC spec 14496-3 section 4.6.18.3.2.2 Derived frequency band tables */
  pstr_noise_floor_est_sbr->num_of_noise_bands =
      (WORD32)((pstr_noise_floor_est_sbr->noise_groups * log((FLOAT32)k2 / kx) * SBR_INV_LOG_2) +
               0.5f);

  pstr_noise_floor_est_sbr->num_of_noise_bands =
      MAX(1, pstr_noise_floor_est_sbr->num_of_noise_bands);

  /* ISO_IEC spec 14496-3 section 4.6.18.3.6  Requirements */
  pstr_noise_floor_est_sbr->num_of_noise_bands =
      MIN(pstr_noise_floor_est_sbr->num_of_noise_bands, MAXIMUM_NUM_NOISE_COEFFS);

  return ia_enhaacplus_enc_down_sample_lo_res(pstr_noise_floor_est_sbr->s_freq_qmf_band_tbl,
                                              pstr_noise_floor_est_sbr->num_of_noise_bands,
                                              ptr_freq_band_tab, num_scf);
}
