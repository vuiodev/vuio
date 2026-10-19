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
#include "ixheaace_aac_constants.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops16.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaac_error_standards.h"
#include "ixheaace_error_codes.h"

#include "ixheaace_bitbuffer.h"
#include "ixheaace_sbr_def.h"
#include "iusace_esbr_inter_tes.h"
#include "iusace_esbr_rom.h"

VOID ixheaace_init_esbr_inter_tes(ixheaace_str_inter_tes_params *pstr_tes_enc,
                                  WORD32 sbr_ratio_index) {
  WORD32 ts;
  WORD32 memset_sz = IXHEAACE_QMF_CHANNELS * sizeof(pstr_tes_enc->qmf_buf_real[0][0]);

  switch (sbr_ratio_index) {
    case USAC_SBR_RATIO_INDEX_2_1:
      pstr_tes_enc->op_delay = 6;
      pstr_tes_enc->codec_delay = 32;
      pstr_tes_enc->sbr_ratio_index = sbr_ratio_index;
      break;
    case USAC_SBR_RATIO_INDEX_4_1:
      pstr_tes_enc->op_delay = 6 * 2;
      pstr_tes_enc->codec_delay = 64;
      pstr_tes_enc->sbr_ratio_index = sbr_ratio_index;
      break;
  }

  memset(&pstr_tes_enc->bw_array_prev[0], 0,
         IXHEAACE_MAX_NUM_PATCHES * sizeof(pstr_tes_enc->bw_array_prev[0]));
  memset(&pstr_tes_enc->inv_filt_mode_prev[0], 0,
         IXHEAACE_MAX_NUM_NOISE_VALUES * sizeof(pstr_tes_enc->inv_filt_mode_prev[0]));

  for (ts = 0;
       ts < pstr_tes_enc->op_delay + pstr_tes_enc->codec_delay + IXHEAACE_SBR_HF_ADJ_OFFSET;
       ts++) {
    memset(pstr_tes_enc->qmf_buf_real[ts], 0, memset_sz);
    memset(pstr_tes_enc->qmf_buf_imag[ts], 0, memset_sz);
  }
  return;
}

static VOID ixheaace_apply_inter_tes(FLOAT32 *qmf_real1, FLOAT32 *qmf_imag1, FLOAT32 *qmf_real,
                                     FLOAT32 *qmf_imag, WORD32 num_sample, WORD32 sub_band_start,
                                     WORD32 num_subband, WORD32 gamma_idx) {
  WORD32 sub_band_end = sub_band_start + num_subband;
  FLOAT32 subsample_power_high[IXHEAACE_TIMESLOT_BUFFER_SIZE],
      subsample_power_low[IXHEAACE_TIMESLOT_BUFFER_SIZE];
  FLOAT32 total_power_high = 0.0f;
  FLOAT32 total_power_low = 0.0f, total_power_high_after = 1.0e-6f;
  FLOAT32 gain[IXHEAACE_TIMESLOT_BUFFER_SIZE];
  FLOAT32 gain_adj, gain_adj_2;
  FLOAT32 gamma = ixheaace_gamma_tab[gamma_idx];
  WORD32 i, j;
  WORD32 memcpy_sz = sub_band_start * sizeof(FLOAT32);

  if (gamma > 0) {
    for (i = 0; i < num_sample; i++) {
      memcpy(&qmf_real[IXHEAACE_QMF_CHANNELS * i], &qmf_real1[IXHEAACE_QMF_CHANNELS * i],
             memcpy_sz);
      memcpy(&qmf_imag[IXHEAACE_QMF_CHANNELS * i], &qmf_imag1[IXHEAACE_QMF_CHANNELS * i],
             memcpy_sz);
    }

    for (i = 0; i < num_sample; i++) {
      j = 0;
      subsample_power_low[i] = 0.0f;
      while (j < sub_band_start) {
        subsample_power_low[i] +=
            qmf_real[IXHEAACE_QMF_CHANNELS * i + j] * qmf_real[IXHEAACE_QMF_CHANNELS * i + j];
        subsample_power_low[i] +=
            qmf_imag[IXHEAACE_QMF_CHANNELS * i + j] * qmf_imag[IXHEAACE_QMF_CHANNELS * i + j];
        j++;
      }
      subsample_power_high[i] = 0.0f;
      while (j < sub_band_end) {
        subsample_power_high[i] +=
            qmf_real[IXHEAACE_QMF_CHANNELS * i + j] * qmf_real[IXHEAACE_QMF_CHANNELS * i + j];
        subsample_power_high[i] +=
            qmf_imag[IXHEAACE_QMF_CHANNELS * i + j] * qmf_imag[IXHEAACE_QMF_CHANNELS * i + j];
        j++;
      }
      total_power_low += subsample_power_low[i];
      total_power_high += subsample_power_high[i];
    }

    for (i = 0; i < num_sample; i++) {
      gain[i] =
          (FLOAT32)(sqrt(subsample_power_low[i] * num_sample / (total_power_low + 1.0e-6f)));
      gain[i] = (FLOAT32)(1.0f + gamma * (gain[i] - 1.0f));

      if (gain[i] < 0.2f) {
        gain[i] = 0.2f;
      }

      subsample_power_high[i] *= gain[i] * gain[i];
      total_power_high_after += subsample_power_high[i];
    }

    gain_adj_2 = total_power_high / total_power_high_after;
    gain_adj = (FLOAT32)(sqrt(gain_adj_2));

    for (i = 0; i < num_sample; i++) {
      gain[i] *= gain_adj;

      j = sub_band_start;
      while (j < sub_band_end) {
        qmf_real[IXHEAACE_QMF_CHANNELS * i + j] *= gain[i];
        qmf_imag[IXHEAACE_QMF_CHANNELS * i + j] *= gain[i];
        j++;
      }
    }
  }
}

static WORD32 ixheaace_inter_tes_sound_activity(FLOAT32 qmf_real[][IXHEAACE_QMF_CHANNELS],
                                                FLOAT32 qmf_imag[][IXHEAACE_QMF_CHANNELS],
                                                FLOAT32 energy[], WORD32 len, WORD32 start,
                                                WORD32 stop, WORD32 *is_transient) {
  WORD32 snd_act = 0, ts, idx;
  FLOAT32 ene_min = MAX_FLT_VAL, ene_max = 0.0f;

  for (ts = 0; ts < len; ts++) {
    idx = start;
    while (idx < stop) {
      energy[ts] += (qmf_real[ts][idx] * qmf_real[ts][idx]);
      energy[ts] += (qmf_imag[ts][idx] * qmf_imag[ts][idx]);
      idx++;
    }

    if (energy[ts] > ene_max) {
      ene_max = energy[ts];
    }
    if (energy[ts] < ene_min) {
      ene_min = energy[ts];
    }
  }

  snd_act = (ene_max > IXHEAACE_ESBR_TES_ENERGY_MAX_THR) ? 1 : 0;

  if ((ene_max / (ene_min + 1.0e-6f)) > 20) {
    *is_transient = 1;
  } else {
    *is_transient = 0;
  }
  return snd_act;
}

static WORD16 ixheaace_find_closest_entry(WORD32 goal_sb, WORD16 *ptr_master_tab,
                                          WORD16 num_mf_bands, WORD16 direction) {
  WORD32 index;

  if (goal_sb <= ptr_master_tab[0]) return ptr_master_tab[0];

  if (goal_sb >= ptr_master_tab[num_mf_bands]) return ptr_master_tab[num_mf_bands];

  if (direction) {
    index = 0;
    while (ptr_master_tab[index] < goal_sb) {
      index++;
    }
  } else {
    index = num_mf_bands;
    while (ptr_master_tab[index] > goal_sb) {
      index--;
    }
  }

  return ptr_master_tab[index];
}

static VOID ixheaace_esbr_calc_co_variance(ixheaace_str_auto_corr_ele *pstr_auto_corr,
                                           FLOAT32 ptr_vec_x_real[][IXHEAACE_QMF_CHANNELS],
                                           FLOAT32 ptr_vec_x_imag[][IXHEAACE_QMF_CHANNELS],
                                           WORD32 bd, WORD32 len) {
  WORD32 j = 0;

  FLOAT32 xr_j;
  FLOAT32 xr_j_minus_1 = ptr_vec_x_real[j - 1][bd];
  FLOAT32 xr_j_minus_2 = ptr_vec_x_real[j - 2][bd];

  FLOAT32 xi_j;
  FLOAT32 xi_j_minus_1 = ptr_vec_x_imag[j - 1][bd];
  FLOAT32 xi_j_minus_2 = ptr_vec_x_imag[j - 2][bd];

  memset(pstr_auto_corr, 0, sizeof(ixheaace_str_auto_corr_ele));

  for (j = 0; j < len; j++) {
    xr_j = ptr_vec_x_real[j][bd];
    xi_j = ptr_vec_x_imag[j][bd];

    pstr_auto_corr->phi_0_1_real += xr_j * xr_j_minus_1 + xi_j * xi_j_minus_1;

    pstr_auto_corr->phi_0_1_imag += xi_j * xr_j_minus_1 - xr_j * xi_j_minus_1;

    pstr_auto_corr->phi_0_2_real += xr_j * xr_j_minus_2 + xi_j * xi_j_minus_2;

    pstr_auto_corr->phi_0_2_imag += xi_j * xr_j_minus_2 - xr_j * xi_j_minus_2;

    pstr_auto_corr->phi_1_1 += xr_j_minus_1 * xr_j_minus_1 + xi_j_minus_1 * xi_j_minus_1;

    pstr_auto_corr->phi_1_2_real += xr_j_minus_1 * xr_j_minus_2 + xi_j_minus_1 * xi_j_minus_2;

    pstr_auto_corr->phi_1_2_imag += xi_j_minus_1 * xr_j_minus_2 - xr_j_minus_1 * xi_j_minus_2;

    pstr_auto_corr->phi_2_2 += xr_j_minus_2 * xr_j_minus_2 + xi_j_minus_2 * xi_j_minus_2;

    xr_j_minus_2 = xr_j_minus_1;
    xr_j_minus_1 = xr_j;

    xi_j_minus_2 = xi_j_minus_1;
    xi_j_minus_1 = xi_j;
  }

  pstr_auto_corr->det = pstr_auto_corr->phi_1_1 * pstr_auto_corr->phi_2_2 -
                        (pstr_auto_corr->phi_1_2_real * pstr_auto_corr->phi_1_2_real +
                         pstr_auto_corr->phi_1_2_imag * pstr_auto_corr->phi_1_2_imag) *
                            IXHEAACE_SBR_HF_RELAXATION_PARAM;
}

static VOID ixheaace_gausssolve(WORD32 n, FLOAT32 ptr_a[][IXHEAACE_MAXDEG + 1], FLOAT32 ptr_b[],
                                FLOAT32 ptr_y[]) {
  WORD32 i, j, k, imax;
  FLOAT32 v;

  for (i = 0; i < n; i++) {
    imax = i;
    k = i + 1;
    while (k < n) {
      if (fabs(ptr_a[k][i]) > fabs(ptr_a[imax][i])) {
        imax = k;
      }
      k++;
    }
    if (imax != i) {
      v = ptr_b[imax];
      ptr_b[imax] = ptr_b[i];
      ptr_b[i] = v;
      j = i;
      while (j < n) {
        v = ptr_a[imax][j];
        ptr_a[imax][j] = ptr_a[i][j];
        ptr_a[i][j] = v;
        j++;
      }
    }

    v = ptr_a[i][i];

    ptr_b[i] /= v;
    for (j = i; j < n; j++) {
      ptr_a[i][j] /= v;
    }

    for (k = i + 1; k < n; k++) {
      v = ptr_a[k][i];
      ptr_b[k] -= v * ptr_b[i];
      for (j = i + 1; j < n; j++) {
        ptr_a[k][j] -= v * ptr_a[i][j];
      }
    }
  }

  for (i = n - 1; i >= 0; i--) {
    ptr_y[i] = ptr_b[i];
    for (j = i + 1; j < n; j++) {
      ptr_y[i] -= ptr_a[i][j] * ptr_y[j];
    }
  }
}

static VOID ixheaace_polyfit(WORD32 n, FLOAT32 ptr_y[], FLOAT32 ptr_p[]) {
  WORD32 i, j, k;
  FLOAT32 ptr_a[IXHEAACE_MAXDEG + 1][IXHEAACE_MAXDEG + 1] = {{0}};
  FLOAT32 ptr_b[IXHEAACE_MAXDEG + 1] = {0};
  FLOAT32 v[2 * IXHEAACE_MAXDEG + 1];

  for (k = 0; k < n; k++) {
    v[0] = 1.0;
    for (i = 1; i <= 2 * IXHEAACE_MAXDEG; i++) {
      v[i] = k * v[i - 1];
    }

    for (i = 0; i <= IXHEAACE_MAXDEG; i++) {
      ptr_b[i] += v[IXHEAACE_MAXDEG - i] * ptr_y[k];
      for (j = 0; j <= IXHEAACE_MAXDEG; j++) {
        ptr_a[i][j] += v[2 * IXHEAACE_MAXDEG - i - j];
      }
    }
  }

  ixheaace_gausssolve(IXHEAACE_MAXDEG + 1, ptr_a, ptr_b, ptr_p);
}

static VOID ixheaace_esbr_chirp_fac_calc(WORD32 *ptr_inv_filt_mode,
                                         WORD32 *ptr_inv_filt_mode_prev, WORD32 num_if_bands,
                                         FLOAT32 *ptr_bw_array, FLOAT32 *ptr_bw_array_prev) {
  WORD32 i;

  for (i = 0; i < num_if_bands; i++) {
    ptr_bw_array[i] = ixheaace_new_bw_tab[ptr_inv_filt_mode_prev[i]][ptr_inv_filt_mode[i]];

    if (ptr_bw_array[i] < ptr_bw_array_prev[i]) {
      ptr_bw_array[i] = 0.75000f * ptr_bw_array[i] + 0.25000f * ptr_bw_array_prev[i];
    } else {
      ptr_bw_array[i] = 0.90625f * ptr_bw_array[i] + 0.09375f * ptr_bw_array_prev[i];
    }

    if (ptr_bw_array[i] < 0.015625) {
      ptr_bw_array[i] = 0;
    }
  }
}

static VOID ixheaace_pre_processing(FLOAT32 ptr_src_buf_real[][IXHEAACE_QMF_CHANNELS],
                                    FLOAT32 ptr_src_buf_imag[][IXHEAACE_QMF_CHANNELS],
                                    FLOAT32 ptr_gain_vector[], WORD32 num_bands,
                                    WORD32 start_sample, WORD32 end_sample) {
  WORD32 k, i;
  FLOAT32 poly_coeff[4];
  FLOAT32 mean_enrg = 0;
  FLOAT32 low_env_slope[IXHEAACE_QMF_CHANNELS];
  FLOAT32 low_env[IXHEAACE_QMF_CHANNELS];
  FLOAT32 a0;
  FLOAT32 a1;
  FLOAT32 a2;
  FLOAT32 a3;

  for (k = 0; k < num_bands; k++) {
    FLOAT32 temp = 0;
    for (i = start_sample; i < end_sample; i++) {
      temp += ptr_src_buf_real[i][k] * ptr_src_buf_real[i][k] +
              ptr_src_buf_imag[i][k] * ptr_src_buf_imag[i][k];
    }
    temp /= (end_sample - start_sample);
    low_env[k] = (FLOAT32)(10 * log10(temp + 1));
    mean_enrg += low_env[k];
  }
  mean_enrg /= num_bands;

  ixheaace_polyfit(num_bands, low_env, poly_coeff);

  a0 = poly_coeff[0];
  a1 = poly_coeff[1];
  a2 = poly_coeff[2];
  a3 = poly_coeff[3];
  for (k = 0; k < num_bands; k++) {
    low_env_slope[k] = a3 + a2 * k + a1 * k * k + a0 * k * k * k;
  }

  for (i = 0; i < num_bands; i++) {
    ptr_gain_vector[i] = (FLOAT32)pow(10, (mean_enrg - low_env_slope[i]) / 20.0f);
  }
}

static IA_ERRORCODE ixheaace_generate_hf(FLOAT32 ptr_src_buf_real[][64],
                                         FLOAT32 ptr_src_buf_imag[][64],
                                         FLOAT32 ptr_ph_vocod_buf_real[][64],
                                         FLOAT32 ptr_ph_vocod_buf_imag[][64],
                                         FLOAT32 ptr_dst_buf_real[][64],
                                         FLOAT32 ptr_dst_buf_imag[][64],
                                         ixheaace_str_inter_tes_params *pstr_tes_enc) {
  WORD32 bw_index, i, k, k2, patch = 0;
  WORD32 co_var_len;
  WORD32 start_sample, end_sample, goal_sb;
  WORD32 sb, source_start_band, patch_stride, num_bands_in_patch;
  WORD32 hbe_flag = 0;
  FLOAT32 a0r, a0i, a1r, a1i;
  FLOAT32 ptr_bw_array[IXHEAACE_MAX_NUM_PATCHES] = {0};

  ixheaace_str_auto_corr_ele str_auto_corr;

  WORD16 *ptr_invf_band_tbl = &pstr_tes_enc->invf_band_tbl[1];
  WORD32 num_if_bands = pstr_tes_enc->num_if_bands;
  WORD32 sub_band_start = pstr_tes_enc->sub_band_start;
  WORD16 *ptr_master_tab = pstr_tes_enc->f_master_tbl;
  WORD32 num_mf_bands = pstr_tes_enc->num_mf_bands;
  WORD32 *ptr_inv_filt_mode = pstr_tes_enc->inv_filt_mode;
  WORD32 *ptr_inv_filt_mode_prev = pstr_tes_enc->inv_filt_mode_prev;
  WORD32 sbr_patching_mode = 1;
  WORD32 pre_proc_flag = 0;
  WORD32 fs = pstr_tes_enc->out_fs;
  WORD32 cov_count;
  WORD32 lsb = ptr_master_tab[0];
  WORD32 usb = ptr_master_tab[num_mf_bands];
  WORD32 memset_sz = (IXHEAACE_QMF_CHANNELS - usb) * sizeof(FLOAT32);
  WORD32 xover_offset = sub_band_start - ptr_master_tab[0];
  FLOAT32 bw = 0.0f;
  FLOAT32 fac = 0.0f;
  FLOAT32 gain;
  FLOAT32 ptr_gain_vector[64];
  WORD32 slope_length = 0;
  WORD32 first_slot_offset = pstr_tes_enc->border_vec[0];
  WORD32 end_slot_offs = 0;
  FLOAT32 *ptr_bw_array_prev = pstr_tes_enc->bw_array_prev;

  end_slot_offs = pstr_tes_enc->border_vec[pstr_tes_enc->num_env] - 16;

  switch (pstr_tes_enc->sbr_ratio_index) {
    case USAC_SBR_RATIO_INDEX_2_1:
      start_sample = first_slot_offset * 2;
      end_sample = 32 + end_slot_offs * 2;
      co_var_len = 38;
      break;
    case USAC_SBR_RATIO_INDEX_4_1:
      start_sample = first_slot_offset * 4;
      end_sample = 64 + end_slot_offs * 4;
      co_var_len = 76;
      break;
    default:
      start_sample = first_slot_offset * 2;
      end_sample = 32 + end_slot_offs * 2;
      co_var_len = 38;
      break;
  }

  if (pre_proc_flag) {
    ixheaace_pre_processing(ptr_src_buf_real, ptr_src_buf_imag, ptr_gain_vector,
                            ptr_master_tab[0], start_sample, end_sample);
  }

  ixheaace_esbr_chirp_fac_calc(ptr_inv_filt_mode, ptr_inv_filt_mode_prev, num_if_bands,
                               ptr_bw_array, ptr_bw_array_prev);

  for (i = start_sample; i < end_sample; i++) {
    memset(ptr_dst_buf_real[i] + usb, 0, memset_sz);
    memset(ptr_dst_buf_imag[i] + usb, 0, memset_sz);
  }

  if (sbr_patching_mode || !hbe_flag) {
    FLOAT32 alpha_real[IXHEAACE_QMF_CHANNELS][2] = {{0}},
            alpha_imag[IXHEAACE_QMF_CHANNELS][2] = {{0}};
    cov_count = ptr_master_tab[0];

    for (k = 1; k < cov_count; k++) {
      ixheaace_esbr_calc_co_variance(&str_auto_corr, &ptr_src_buf_real[0], &ptr_src_buf_imag[0],
                                     k, co_var_len);
      if (str_auto_corr.det == 0.0f) {
        alpha_real[k][1] = alpha_imag[k][1] = 0;
      } else {
        fac = 1.0f / str_auto_corr.det;
        alpha_real[k][1] = (str_auto_corr.phi_0_1_real * str_auto_corr.phi_1_2_real -
                            str_auto_corr.phi_0_1_imag * str_auto_corr.phi_1_2_imag -
                            str_auto_corr.phi_0_2_real * str_auto_corr.phi_1_1) *
                           fac;
        alpha_imag[k][1] = (str_auto_corr.phi_0_1_imag * str_auto_corr.phi_1_2_real +
                            str_auto_corr.phi_0_1_real * str_auto_corr.phi_1_2_imag -
                            str_auto_corr.phi_0_2_imag * str_auto_corr.phi_1_1) *
                           fac;
      }

      if (str_auto_corr.phi_1_1 == 0) {
        alpha_real[k][0] = alpha_imag[k][0] = 0;
      } else {
        fac = 1.0f / str_auto_corr.phi_1_1;
        alpha_real[k][0] =
            -(str_auto_corr.phi_0_1_real + alpha_real[k][1] * str_auto_corr.phi_1_2_real +
              alpha_imag[k][1] * str_auto_corr.phi_1_2_imag) *
            fac;
        alpha_imag[k][0] =
            -(str_auto_corr.phi_0_1_imag + alpha_imag[k][1] * str_auto_corr.phi_1_2_real -
              alpha_real[k][1] * str_auto_corr.phi_1_2_imag) *
            fac;
      }

      if ((alpha_real[k][0] * alpha_real[k][0] + alpha_imag[k][0] * alpha_imag[k][0] >= 16.0f) ||
          (alpha_real[k][1] * alpha_real[k][1] + alpha_imag[k][1] * alpha_imag[k][1] >= 16.0f)) {
        alpha_real[k][0] = 0.0f;
        alpha_imag[k][0] = 0.0f;
        alpha_real[k][1] = 0.0f;
        alpha_imag[k][1] = 0.0f;
      }
    }

    goal_sb = (WORD32)(2.048e6f / fs + 0.5f);
    {
      WORD32 index;
      if (goal_sb < ptr_master_tab[num_mf_bands]) {
        for (index = 0; (ptr_master_tab[index] < goal_sb); index++)
          goal_sb = ptr_master_tab[index];
      } else {
        goal_sb = ptr_master_tab[num_mf_bands];
      }
    }

    source_start_band = xover_offset + 1;

    sb = lsb + xover_offset;

    patch = 0;
    while (sb < usb) {
      if (IXHEAACE_MAX_NUM_PATCHES <= patch) {
        return IA_EXHEAACE_EXE_NONFATAL_ESBR_INVALID_NUM_PATCH;
      }

      num_bands_in_patch = goal_sb - sb;

      if (num_bands_in_patch + source_start_band >= lsb) {
        patch_stride = sb - source_start_band;
        patch_stride = patch_stride & ~1;
        num_bands_in_patch = lsb - (sb - patch_stride);
        num_bands_in_patch = ixheaace_find_closest_entry(sb + num_bands_in_patch, ptr_master_tab,
                                                         (WORD16)(num_mf_bands), 0) -
                             (WORD32)(sb);
      }

      patch_stride = num_bands_in_patch + sb - lsb;
      patch_stride = (patch_stride + 1) & ~1;

      source_start_band = 1;

      if (goal_sb - (sb + num_bands_in_patch) < 3) {
        goal_sb = usb;
      }

      if ((num_bands_in_patch < 3) && (patch > 0) && (sb + num_bands_in_patch == usb)) {
        for (i = start_sample + slope_length; i < end_sample + slope_length; i++) {
          for (k2 = sb; k2 < sb + num_bands_in_patch; k2++) {
            if (k2 < 0 || k2 >= 64) {
              break;
            }
            ptr_dst_buf_real[i][k2] = 0.0f;
            ptr_dst_buf_imag[i][k2] = 0.0f;
          }
        }
        break;
      }

      if (num_bands_in_patch <= 0) {
        continue;
      }

      for (k2 = sb; k2 < sb + num_bands_in_patch; k2++) {
        k = k2 - patch_stride;
        bw_index = 0;
        while (k2 >= ptr_invf_band_tbl[bw_index]) {
          bw_index++;
          if (bw_index >= IXHEAACE_MAX_NOISE_COEFFS) {
            return IA_EXHEAACE_EXE_NONFATAL_ESBR_INVALID_BANDWIDTH_INDEX;
          }
        }

        if (bw_index >= IXHEAACE_MAX_NUM_PATCHES) {
          return IA_EXHEAACE_EXE_NONFATAL_ESBR_INVALID_BANDWIDTH_INDEX;
        }
        bw = ptr_bw_array[bw_index];

        a0r = bw * alpha_real[k][0];
        a0i = bw * alpha_imag[k][0];
        bw *= bw;
        a1r = bw * alpha_real[k][1];
        a1i = bw * alpha_imag[k][1];

        if (pre_proc_flag) {
          gain = ptr_gain_vector[k];
        } else {
          gain = 1.0f;
        }

        for (i = start_sample + slope_length; i < end_sample + slope_length; i++) {
          ptr_dst_buf_real[i][k2] = ptr_src_buf_real[i][k] * gain;

          ptr_dst_buf_imag[i][k2] = ptr_src_buf_imag[i][k] * gain;

          if (bw > 0.0f) {
            ptr_dst_buf_real[i][k2] +=
                (a0r * ptr_src_buf_real[i - 1][k] - a0i * ptr_src_buf_imag[i - 1][k] +
                 a1r * ptr_src_buf_real[i - 2][k] - a1i * ptr_src_buf_imag[i - 2][k]) *
                gain;
            ptr_dst_buf_imag[i][k2] +=
                (a0i * ptr_src_buf_real[i - 1][k] + a0r * ptr_src_buf_imag[i - 1][k] +
                 a1i * ptr_src_buf_real[i - 2][k] + a1r * ptr_src_buf_imag[i - 2][k]) *
                gain;
          }
        }
      }
      sb += num_bands_in_patch;
      patch++;
    }
  }

  if (hbe_flag && !sbr_patching_mode) {
    FLOAT32 alpha_real[2], alpha_imag[2];

    bw_index = 0, patch = 1;
    if (NULL == ptr_ph_vocod_buf_real || NULL == ptr_ph_vocod_buf_imag) {
      return IA_EXHEAACE_EXE_NONFATAL_ESBR_INVALID_VOCOD_BUF;
    }

    for (k2 = sub_band_start; k2 < ptr_master_tab[num_mf_bands]; k2++) {
      ixheaace_esbr_calc_co_variance(&str_auto_corr, &ptr_ph_vocod_buf_real[0],
                                     &ptr_ph_vocod_buf_imag[0], k2, co_var_len);

      if (str_auto_corr.det == 0.0f) {
        alpha_real[1] = alpha_imag[1] = 0;
      } else {
        fac = 1.0f / str_auto_corr.det;
        alpha_real[1] = (str_auto_corr.phi_0_1_real * str_auto_corr.phi_1_2_real -
                         str_auto_corr.phi_0_1_imag * str_auto_corr.phi_1_2_imag -
                         str_auto_corr.phi_0_2_real * str_auto_corr.phi_1_1) *
                        fac;
        alpha_imag[1] = (str_auto_corr.phi_0_1_imag * str_auto_corr.phi_1_2_real +
                         str_auto_corr.phi_0_1_real * str_auto_corr.phi_1_2_imag -
                         str_auto_corr.phi_0_2_imag * str_auto_corr.phi_1_1) *
                        fac;
      }

      if (str_auto_corr.phi_1_1 == 0) {
        alpha_real[0] = alpha_imag[0] = 0;
      } else {
        fac = 1.0f / str_auto_corr.phi_1_1;
        alpha_real[0] =
            -(str_auto_corr.phi_0_1_real + alpha_real[1] * str_auto_corr.phi_1_2_real +
              alpha_imag[1] * str_auto_corr.phi_1_2_imag) *
            fac;
        alpha_imag[0] =
            -(str_auto_corr.phi_0_1_imag + alpha_imag[1] * str_auto_corr.phi_1_2_real -
              alpha_real[1] * str_auto_corr.phi_1_2_imag) *
            fac;
      }

      if (alpha_real[0] * alpha_real[0] + alpha_imag[0] * alpha_imag[0] >= 16.0f ||
          alpha_real[1] * alpha_real[1] + alpha_imag[1] * alpha_imag[1] >= 16.0f) {
        alpha_real[0] = 0.0f;
        alpha_imag[0] = 0.0f;
        alpha_real[1] = 0.0f;
        alpha_imag[1] = 0.0f;
      }

      while (k2 >= ptr_invf_band_tbl[bw_index]) {
        bw_index++;
        if (bw_index >= IXHEAACE_MAX_NOISE_COEFFS) {
          return IA_EXHEAACE_EXE_NONFATAL_ESBR_INVALID_BANDWIDTH_INDEX;
        }
      }

      if (bw_index >= IXHEAACE_MAX_NUM_PATCHES) {
        return IA_EXHEAACE_EXE_NONFATAL_ESBR_INVALID_BANDWIDTH_INDEX;
      }
      bw = ptr_bw_array[bw_index];

      a0r = bw * alpha_real[0];
      a0i = bw * alpha_imag[0];
      bw *= bw;
      a1r = bw * alpha_real[1];
      a1i = bw * alpha_imag[1];

      if (bw > 0.0f) {
        for (i = start_sample; i < end_sample; i++) {
          FLOAT32 real1, imag1, real2, imag2;

          real1 = ptr_ph_vocod_buf_real[i - 1][k2];
          imag1 = ptr_ph_vocod_buf_imag[i - 1][k2];
          real2 = ptr_ph_vocod_buf_real[i - 2][k2];
          imag2 = ptr_ph_vocod_buf_imag[i - 2][k2];
          ptr_dst_buf_real[i][k2] = ptr_ph_vocod_buf_real[i][k2] +
                                    ((a0r * real1 - a0i * imag1) + (a1r * real2 - a1i * imag2));
          ptr_dst_buf_imag[i][k2] = ptr_ph_vocod_buf_imag[i][k2] +
                                    ((a0i * real1 + a0r * imag1) + (a1i * real2 + a1r * imag2));
        }
      } else {
        for (i = start_sample; i < end_sample; i++) {
          ptr_dst_buf_real[i][k2] = ptr_ph_vocod_buf_real[i][k2];
          ptr_dst_buf_imag[i][k2] = ptr_ph_vocod_buf_imag[i][k2];
        }
      }
    }
  }
  if (patch >= (IXHEAACE_MAX_NUM_PATCHES + 1)) {
    return IA_EXHEAACE_EXE_NONFATAL_ESBR_INVALID_NUM_PATCH;
  }
  for (i = 0; i < num_if_bands; i++) {
    ptr_bw_array_prev[i] = ptr_bw_array[i];
  }
  return IA_NO_ERROR;
}

IA_ERRORCODE ixheaace_process_inter_tes(ixheaace_str_inter_tes_params *pstr_tes_enc,
                                        WORD8 *ptr_scr) {
  WORD32 gi = 0, env, tes_enable = 0, ts, bd, start_ts, stop_ts;
  WORD32 is_sound_activity[IXHEAACE_MAX_ENVELOPES] = {0},
         is_transient[IXHEAACE_MAX_ENVELOPES] = {0};
  WORD32 tes_shape_mode = 0;
  WORD32 num_samples, num_bands;
  WORD32 len;
  IA_ERRORCODE status = IA_NO_ERROR;
  FLOAT32 energy_high[64] = {0};
  FLOAT32 energy[64] = {0};
  FLOAT32 gamma[IXHEAACE_ESBR_NUM_GAMMA_IDXS] = {0};
  FLOAT32 gamma_min = MAX_FLT_VAL;
  ixheaace_str_inter_tes_scr *tes_scr = (ixheaace_str_inter_tes_scr *)ptr_scr;
  num_bands = pstr_tes_enc->sub_band_end - pstr_tes_enc->sub_band_start;

  for (env = 0; env < pstr_tes_enc->num_env; env++) {
    tes_shape_mode = 0;
    len = 2 * (pstr_tes_enc->border_vec[env + 1] - pstr_tes_enc->border_vec[env]);
    is_sound_activity[env] = ixheaace_inter_tes_sound_activity(
        &pstr_tes_enc
             ->qmf_buf_real[IXHEAACE_SBR_HF_ADJ_OFFSET + 2 * pstr_tes_enc->border_vec[env]],
        &pstr_tes_enc
             ->qmf_buf_imag[IXHEAACE_SBR_HF_ADJ_OFFSET + 2 * pstr_tes_enc->border_vec[env]],
        &energy_high[IXHEAACE_SBR_HF_ADJ_OFFSET + 2 * pstr_tes_enc->border_vec[env]], len,
        pstr_tes_enc->sub_band_start, pstr_tes_enc->sub_band_end, &is_transient[env]);
    if (1 == is_transient[env] && 1 == is_sound_activity[env]) {
      tes_enable = 1;
    }
  }

  if (1 == tes_enable) {
    status = ixheaace_generate_hf(&pstr_tes_enc->qmf_buf_real[IXHEAACE_SBR_HF_ADJ_OFFSET],
                                  &pstr_tes_enc->qmf_buf_imag[IXHEAACE_SBR_HF_ADJ_OFFSET], NULL,
                                  NULL, &tes_scr->dst_qmf_r[IXHEAACE_SBR_HF_ADJ_OFFSET],
                                  &tes_scr->dst_qmf_i[IXHEAACE_SBR_HF_ADJ_OFFSET], pstr_tes_enc);

    if (status) {
      return status;
    }
    for (env = 0; env < pstr_tes_enc->num_env; env++) {
      if ((1 == is_sound_activity[env]) && (1 == is_transient[env])) {
        num_samples = (pstr_tes_enc->border_vec[env + 1] - pstr_tes_enc->border_vec[env]) * 2;
        start_ts = IXHEAACE_SBR_HF_ADJ_OFFSET + pstr_tes_enc->border_vec[env] * 2;
        stop_ts = start_ts + num_samples;

        for (gi = 0; gi < IXHEAACE_ESBR_NUM_GAMMA_IDXS; gi++) {
          ixheaace_apply_inter_tes(
              &tes_scr
                   ->dst_qmf_r[IXHEAACE_SBR_HF_ADJ_OFFSET + 2 * pstr_tes_enc->border_vec[env]][0],
              &tes_scr
                   ->dst_qmf_i[IXHEAACE_SBR_HF_ADJ_OFFSET + 2 * pstr_tes_enc->border_vec[env]][0],
              &pstr_tes_enc->qmf_buf_real[IXHEAACE_SBR_HF_ADJ_OFFSET +
                                          2 * pstr_tes_enc->border_vec[env]][0],
              &pstr_tes_enc->qmf_buf_imag[IXHEAACE_SBR_HF_ADJ_OFFSET +
                                          2 * pstr_tes_enc->border_vec[env]][0],
              num_samples, pstr_tes_enc->sub_band_start, num_bands, gi);
          for (ts = start_ts; ts < stop_ts; ts++) {
            energy[ts] = 0.0f;
            for (bd = pstr_tes_enc->sub_band_start; bd < pstr_tes_enc->sub_band_end; bd++) {
              energy[ts] += tes_scr->dst_qmf_r[ts][bd] * tes_scr->dst_qmf_r[ts][bd];
              energy[ts] += tes_scr->dst_qmf_i[ts][bd] * tes_scr->dst_qmf_i[ts][bd];
            }
            gamma[gi] += (FLOAT32)fabs(energy[ts] - energy_high[ts]) /
                         (FLOAT32)(pow((energy_high[ts] + 1e-6f), 0.9f));
          }
          if (gamma[gi] < gamma_min) {
            gamma_min = gamma[gi];
            tes_shape_mode = gi;
          }
        }
      }

      if (tes_shape_mode > 0) {
        pstr_tes_enc->bs_tes_shape[env] = 1;
        pstr_tes_enc->bs_tes_shape_mode[env] = tes_shape_mode;
      } else {
        pstr_tes_enc->bs_tes_shape[env] = 0;
        pstr_tes_enc->bs_tes_shape_mode[env] = 0;
      }
    }
  } else {
    for (env = 0; env < pstr_tes_enc->num_env; env++) {
      pstr_tes_enc->bs_tes_shape[env] = 0;
      pstr_tes_enc->bs_tes_shape_mode[env] = 0;
    }
  }
  return status;
}
