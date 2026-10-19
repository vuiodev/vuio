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
#include "ixheaace_sbr_main.h"
#include "ixheaace_sbr_inv_filtering_estimation.h"
#include "ixheaace_sbr_misc.h"
#include "ixheaace_common_utils.h"

static VOID ixheaace_calculate_detector_values(FLOAT32 **ptr_quota_mtx_org, WORD8 *ptr_idx_vx,
                                               FLOAT32 *ptr_energy_vec,
                                               ixheaace_str_detector_values *ptr_detector_values,
                                               WORD32 start_channel, WORD32 stop_channel,
                                               WORD32 start_index, WORD32 stop_index) {
  WORD32 i, j;

  FLOAT32 quota_vec_org[IXHEAACE_QMF_CHANNELS] = {0};
  FLOAT32 quota_vec_sbr[IXHEAACE_QMF_CHANNELS] = {0};
  FLOAT32 org_quota = 0.0f, sbr_quota = 0.0f;

  ptr_detector_values->avg_energy = 0.0f;
  for (j = start_index; j < stop_index; j++) {
    for (i = start_channel; i < stop_channel; i++) {
      quota_vec_org[i] += ptr_quota_mtx_org[j][i];
      if (ptr_idx_vx[i] != -1) {
        quota_vec_sbr[i] += ptr_quota_mtx_org[j][ptr_idx_vx[i]];
      }
    }
    ptr_detector_values->avg_energy += ptr_energy_vec[j];
  }
  ptr_detector_values->avg_energy /= (stop_index - start_index);
  org_quota = 0.0f;
  sbr_quota = 0.0f;

  for (i = start_channel; i < stop_channel; i++) {
    quota_vec_org[i] /= (stop_index - start_index);
    quota_vec_sbr[i] /= (stop_index - start_index);
    org_quota += quota_vec_org[i];
    sbr_quota += quota_vec_sbr[i];
  }

  org_quota /= (stop_channel - start_channel);
  sbr_quota /= (stop_channel - start_channel);

  memmove(ptr_detector_values->org_quota_mean, ptr_detector_values->org_quota_mean + 1,
          IXHEAACE_INVF_SMOOTHING_LENGTH * sizeof(ptr_detector_values->org_quota_mean[0]));
  memmove(ptr_detector_values->sbr_quota_mean, ptr_detector_values->sbr_quota_mean + 1,
          IXHEAACE_INVF_SMOOTHING_LENGTH * sizeof(ptr_detector_values->org_quota_mean[0]));

  ptr_detector_values->org_quota_mean[IXHEAACE_INVF_SMOOTHING_LENGTH] = org_quota;
  ptr_detector_values->sbr_quota_mean[IXHEAACE_INVF_SMOOTHING_LENGTH] = sbr_quota;

  ptr_detector_values->org_quota_mean_filt = 0.0f;
  ptr_detector_values->sbr_quota_mean_filt = 0.0f;

  for (i = 0; i < IXHEAACE_INVF_SMOOTHING_LENGTH + 1; i++) {
    ptr_detector_values->org_quota_mean_filt +=
        ptr_detector_values->org_quota_mean[i] * filter[i];
    ptr_detector_values->sbr_quota_mean_filt +=
        ptr_detector_values->sbr_quota_mean[i] * filter[i];
  }
}

static WORD32 ixheaace_find_region(FLOAT32 curr_val, const FLOAT32 *ptr_borders,
                                   const WORD32 num_borders) {
  WORD32 i;

  if (curr_val < ptr_borders[0]) {
    return 0;
  }

  for (i = 1; i < num_borders; i++) {
    if (curr_val >= ptr_borders[i - 1] && curr_val < ptr_borders[i]) {
      return i;
    }
  }

  if (curr_val > ptr_borders[num_borders - 1]) {
    return num_borders;
  }

  return 0;
}

static ixheaace_invf_mode ixheaace_decision_algorithm(
    const ixheaace_str_det_params *ptr_detector_params,
    ixheaace_str_detector_values ptr_detector_values, WORD32 transient_flag,
    WORD32 *ptr_prev_region_sbr, WORD32 *ptr_prev_region_orig, WORD32 is_ld_sbr) {
  WORD32 inv_filt_level, region_sbr, region_orig, region_nrg;

  const WORD32 hysteresis = 1;
  const WORD32 num_regions_sbr = ptr_detector_params->num_regions_sbr;
  const WORD32 num_regions_orig = ptr_detector_params->num_regions_orig;
  const WORD32 num_regions_nrg = ptr_detector_params->num_regions_nrg;

  FLOAT32 quant_steps_sbr_tmp[IXHEAACE_MAX_NUM_REGIONS];
  FLOAT32 quant_steps_org_tmp[IXHEAACE_MAX_NUM_REGIONS];

  FLOAT32 org_quota_mean_filt, sbr_quota_mean_filt, energy;
  if (is_ld_sbr) {
    org_quota_mean_filt =
        (FLOAT32)(SBR_INV_LOG_2 * 3.0f * log(ptr_detector_values.org_quota_mean_filt + EPS));

    sbr_quota_mean_filt =
        (FLOAT32)(SBR_INV_LOG_2 * 3.0f * log(ptr_detector_values.sbr_quota_mean_filt + EPS));

    energy = (FLOAT32)(SBR_INV_LOG_2 * 1.5f * log(ptr_detector_values.avg_energy + EPS));
  } else {
    org_quota_mean_filt =
        (FLOAT32)(SBR_INV_LOG_2 * 3.0f * log(ptr_detector_values.org_quota_mean_filt * EPS));
    sbr_quota_mean_filt =
        (FLOAT32)(SBR_INV_LOG_2 * 3.0f * log(ptr_detector_values.sbr_quota_mean_filt * EPS));
    energy = (FLOAT32)(SBR_INV_LOG_2 * 1.5f * log(ptr_detector_values.avg_energy * EPS));
  }
  memcpy(quant_steps_org_tmp, ptr_detector_params->quant_steps_org,
         num_regions_orig * sizeof(quant_steps_org_tmp[0]));
  memcpy(quant_steps_sbr_tmp, ptr_detector_params->quant_steps_sbr,
         num_regions_sbr * sizeof(quant_steps_org_tmp[0]));

  if (*ptr_prev_region_sbr < num_regions_sbr) {
    quant_steps_sbr_tmp[*ptr_prev_region_sbr] =
        ptr_detector_params->quant_steps_sbr[*ptr_prev_region_sbr] + (FLOAT32)hysteresis;
  }

  if (*ptr_prev_region_sbr > 0) {
    quant_steps_sbr_tmp[*ptr_prev_region_sbr - 1] =
        ptr_detector_params->quant_steps_sbr[*ptr_prev_region_sbr - 1] - (FLOAT32)hysteresis;
  }

  if (*ptr_prev_region_orig < num_regions_orig) {
    quant_steps_org_tmp[*ptr_prev_region_orig] =
        ptr_detector_params->quant_steps_org[*ptr_prev_region_orig] + (FLOAT32)hysteresis;
  }

  if (*ptr_prev_region_orig > 0) {
    quant_steps_org_tmp[*ptr_prev_region_orig - 1] =
        ptr_detector_params->quant_steps_org[*ptr_prev_region_orig - 1] - (FLOAT32)hysteresis;
  }

  region_sbr = ixheaace_find_region(sbr_quota_mean_filt, quant_steps_sbr_tmp, num_regions_sbr);

  region_orig = ixheaace_find_region(org_quota_mean_filt, quant_steps_org_tmp, num_regions_orig);

  region_nrg = ixheaace_find_region(energy, ptr_detector_params->energy_brdrs, num_regions_nrg);

  *ptr_prev_region_sbr = region_sbr;
  *ptr_prev_region_orig = region_orig;

  inv_filt_level = (transient_flag == 1)
                       ? ptr_detector_params->region_space_transient[region_sbr][region_orig]
                       : ptr_detector_params->region_space[region_sbr][region_orig];

  inv_filt_level =
      ixheaac_max32(inv_filt_level + ptr_detector_params->energy_comp_factor[region_nrg], 0);

  return (ixheaace_invf_mode)(inv_filt_level);
}

VOID ixheaace_qmf_inverse_filtering_detector(ixheaace_pstr_sbr_inv_filt_est pstr_inv_filt,
                                             FLOAT32 **ptr_quota_mtx, FLOAT32 *ptr_energy_vec,
                                             WORD8 *ptr_idx_vx, WORD32 start_index,
                                             WORD32 stop_index, WORD32 transient_flag,
                                             ixheaace_invf_mode *ptr_inf_vec, WORD32 is_ld_sbr) {
  WORD32 band;

  for (band = 0; band < pstr_inv_filt->no_detector_bands; band++) {
    WORD32 start_channel = pstr_inv_filt->freq_band_tab_inv_filt[band];
    WORD32 stop_channel = pstr_inv_filt->freq_band_tab_inv_filt[band + 1];

    ixheaace_calculate_detector_values(ptr_quota_mtx, ptr_idx_vx, ptr_energy_vec,
                                       &pstr_inv_filt->detector_values[band], start_channel,
                                       stop_channel, start_index, stop_index);

    ptr_inf_vec[band] = ixheaace_decision_algorithm(
        pstr_inv_filt->ptr_detector_params, pstr_inv_filt->detector_values[band], transient_flag,
        &pstr_inv_filt->prev_region_sbr[band], &pstr_inv_filt->prev_region_orig[band], is_ld_sbr);
  }
}

static VOID ixheaace_reset_inv_filt_detector(ixheaace_pstr_sbr_inv_filt_est pstr_inv_filt,
                                             WORD32 *ptr_freq_band_tab_detector,
                                             WORD32 num_det_bands) {
  memcpy(pstr_inv_filt->freq_band_tab_inv_filt, ptr_freq_band_tab_detector,
         (num_det_bands + 1) * sizeof(pstr_inv_filt->freq_band_tab_inv_filt[0]));

  pstr_inv_filt->no_detector_bands = num_det_bands;
}

VOID ixheaace_create_inv_filt_detector(ixheaace_pstr_sbr_inv_filt_est pstr_inv_filt,
                                       WORD32 *ptr_freq_band_tab_detector, WORD32 num_det_bands,
                                       UWORD32 use_speech_config,
                                       ixheaace_str_qmf_tabs *ptr_qmf_tab) {
  WORD32 i;

  memset(pstr_inv_filt, 0, sizeof(ixheaace_str_sbr_inv_filt_est));

  if (use_speech_config) {
    pstr_inv_filt->ptr_detector_params = &(ptr_qmf_tab->detector_params_aac_speech);
  } else {
    pstr_inv_filt->ptr_detector_params = &(ptr_qmf_tab->detector_params_aac);
  }

  pstr_inv_filt->no_detector_bands_max = num_det_bands;

  for (i = 0; i < pstr_inv_filt->no_detector_bands_max; i++) {
    memset(&pstr_inv_filt->detector_values[i], 0, sizeof(ixheaace_str_detector_values));

    pstr_inv_filt->prev_invf_mode[i] = IXHEAACE_INVF_OFF;
    pstr_inv_filt->prev_region_orig[i] = 0;
    pstr_inv_filt->prev_region_sbr[i] = 0;
  }

  ixheaace_reset_inv_filt_detector(pstr_inv_filt, ptr_freq_band_tab_detector,
                                   pstr_inv_filt->no_detector_bands_max);
}
