/******************************************************************************
 *                                                                            *
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
#include <stdlib.h>
#include <math.h>
#include <string.h>

#include "ixheaac_type_def.h"
#include "ixheaac_error_standards.h"
#include "ixheaac_sbr_const.h"
#include "ixheaacd_sbrdecsettings.h"
#include "ixheaacd_bitbuffer.h"
#include "ixheaacd_sbr_common.h"
#include "ixheaacd_drc_data_struct.h"
#include "ixheaacd_drc_dec.h"
#include "ixheaacd_sbrdecoder.h"
#include "ixheaacd_bitbuffer.h"
#include "ixheaacd_env_extr_part.h"
#include "ixheaacd_sbr_rom.h"
#include "ixheaacd_common_rom.h"
#include "ixheaacd_hybrid.h"
#include "ixheaacd_sbr_scale.h"
#include "ixheaacd_ps_dec.h"
#include "ixheaacd_freq_sca.h"
#include "ixheaacd_lpp_tran.h"
#include "ixheaacd_env_extr.h"

#include "ixheaac_esbr_rom.h"

#define ABS(A) fabs(A)

VOID ixheaacd_shellsort(WORD32 *in, WORD32 n) {
  WORD32 i, j, v;
  WORD32 inc = 1;

  do
    inc = 3 * inc + 1;
  while (inc <= n);

  do {
    inc = inc / 3;
    for (i = inc + 1; i <= n; i++) {
      v = in[i - 1];
      j = i;
      while (in[j - inc - 1] > v) {
        in[j - 1] = in[j - inc - 1];
        j -= inc;
        if (j <= inc) break;
      }
      in[j - 1] = v;
    }
  } while (inc > 1);
}

WORD32 ixheaacd_sbr_env_calc(ia_sbr_frame_info_data_struct *frame_data, FLOAT32 input_real[][64],
                             FLOAT32 input_imag[][64], FLOAT32 input_real1[][64],
                             FLOAT32 input_imag1[][64], WORD32 x_over_qmf[MAX_NUM_PATCHES],
                             FLOAT32 *scratch_buff, FLOAT32 *env_out, WORD32 ldmps_present,
                             WORD32 ec_flag) {
  IA_ERRORCODE err_code = IA_NO_ERROR;
  WORD8 harmonics[64];
  FLOAT32(*env_tmp)[48];
  FLOAT32(*noise_level_pvc)[48];
  FLOAT32(*nrg_est_pvc)[48];
  FLOAT32(*nrg_ref_pvc)[48];
  FLOAT32(*nrg_gain_pvc)[48];
  FLOAT32(*nrg_tone_pvc)[48];

  WORD32 n, c, li, ui, i, j, k = 0, l, m = 0, kk = 0, o, next = -1, ui2, flag,
                             tmp, noise_absc_flag, smooth_length;
  WORD32 upsamp_4_flag = frame_data->pstr_sbr_header->is_usf_4;

  FLOAT32 *ptr_real_buf, *ptr_imag_buf, nrg = 0, p_ref, p_est, avg_gain, g_max,
                                        p_adj, boost_gain, sb_gain, sb_noise,
                                        temp[64];

  WORD32 t;
  WORD32 start_pos = 0;
  WORD32 end_pos = 0;

  WORD32 slot_idx;

  FLOAT32 *prev_env_noise_level = frame_data->prev_noise_level;
  FLOAT32 *nrg_tone = scratch_buff;
  FLOAT32 *noise_level = scratch_buff + 64;
  FLOAT32 *nrg_est = scratch_buff + 128;
  FLOAT32 *nrg_ref = scratch_buff + 192;
  FLOAT32 *nrg_gain = scratch_buff + 256;

  const FLOAT32 *smooth_filt;

  FLOAT32 *sfb_nrg = frame_data->flt_env_sf_arr;
  FLOAT32 *noise_floor = frame_data->flt_noise_floor;
  ia_frame_info_struct *p_frame_info = &frame_data->str_frame_info_details;

  ia_frame_info_struct *pvc_frame_info = &frame_data->str_pvc_frame_info;
  WORD32 smoothing_length = frame_data->pstr_sbr_header->smoothing_mode ? 0 : 4;
  WORD32 int_mode = frame_data->pstr_sbr_header->interpol_freq;
  WORD32 limiter_band = frame_data->pstr_sbr_header->limiter_bands;
  WORD32 limiter_gains = frame_data->pstr_sbr_header->limiter_gains;
  WORD32 *add_harmonics = frame_data->add_harmonics;
  WORD32 sub_band_start =
      frame_data->pstr_sbr_header->pstr_freq_band_data->sub_band_start;
  WORD32 sub_band_end =
      frame_data->pstr_sbr_header->pstr_freq_band_data->sub_band_end;
  WORD32 reset = frame_data->reset_flag;
  WORD32 num_subbands = sub_band_end - sub_band_start;
  WORD32 bs_num_env = p_frame_info->num_env;
  WORD32 trans_env = p_frame_info->transient_env;
  WORD32 sbr_mode = frame_data->sbr_mode;
  WORD32 prev_sbr_mode = frame_data->prev_sbr_mode;

  WORD16 *freq_band_table[2];
  const WORD16 *num_sf_bands =
      frame_data->pstr_sbr_header->pstr_freq_band_data->num_sf_bands;
  WORD16 *freq_band_table_noise =
      frame_data->pstr_sbr_header->pstr_freq_band_data->freq_band_tbl_noise;
  WORD32 num_nf_bands =
      frame_data->pstr_sbr_header->pstr_freq_band_data->num_nf_bands;

  WORD32 harm_index = frame_data->harm_index;
  WORD32 phase_index = frame_data->phase_index;
  WORD32 esbr_start_up = frame_data->pstr_sbr_header->esbr_start_up;
  WORD32 esbr_start_up_pvc = frame_data->pstr_sbr_header->esbr_start_up_pvc;
  WORD8(*harm_flag_prev)[64] = &frame_data->harm_flag_prev;
  FLOAT32(*e_gain)[5][64] = &frame_data->e_gain;
  FLOAT32(*noise_buf)[5][64] = &frame_data->noise_buf;
  WORD32(*lim_table)[4][12 + 1] = &frame_data->lim_table;
  WORD32(*gate_mode)[4] = &frame_data->gate_mode;
  WORD32 freq_inv = 1;

  WORD8(*harm_flag_varlen_prev)[64] = &frame_data->harm_flag_varlen_prev;
  WORD8(*harm_flag_varlen)[64] = &frame_data->harm_flag_varlen;
  WORD32 band_loop_end;

  WORD32 rate = upsamp_4_flag ? 4 : 2;
  FLOAT64 guard = 1e-17;

  if (ldmps_present == 1) rate = 1;

  env_tmp = frame_data->env_tmp;
  noise_level_pvc = frame_data->noise_level_pvc;
  nrg_est_pvc = frame_data->nrg_est_pvc;
  nrg_ref_pvc = frame_data->nrg_ref_pvc;
  nrg_gain_pvc = frame_data->nrg_gain_pvc;
  nrg_tone_pvc = frame_data->nrg_tone_pvc;

  freq_band_table[0] =
      frame_data->pstr_sbr_header->pstr_freq_band_data->freq_band_table[0];
  freq_band_table[1] =
      frame_data->pstr_sbr_header->pstr_freq_band_data->freq_band_table[1];

  if (reset || (ldmps_present == 1)) {
    esbr_start_up = 1;
    esbr_start_up_pvc = 1;
    if (reset) phase_index = 0;
    if (ixheaacd_createlimiterbands(
            (*lim_table), (*gate_mode),
            frame_data->pstr_sbr_header->pstr_freq_band_data->freq_band_tbl_lo, num_sf_bands[LOW],
            x_over_qmf, frame_data->sbr_patching_mode, upsamp_4_flag, &frame_data->patch_param,
            ec_flag))
      return IA_FATAL_ERROR;
  }

  if (frame_data->sbr_patching_mode != frame_data->prev_sbr_patching_mode) {
    if (ixheaacd_createlimiterbands(
            (*lim_table), (*gate_mode),
            frame_data->pstr_sbr_header->pstr_freq_band_data->freq_band_tbl_lo, num_sf_bands[LOW],
            x_over_qmf, frame_data->sbr_patching_mode, upsamp_4_flag, &frame_data->patch_param,
            ec_flag))
      return IA_FATAL_ERROR;

    frame_data->prev_sbr_patching_mode = frame_data->sbr_patching_mode;
  }

  memset(harmonics, 0, 64 * sizeof(WORD8));

  if (sbr_mode == PVC_SBR) {
    for (i = 0; i < num_sf_bands[HIGH]; i++) {
      li =
          frame_data->pstr_sbr_header->pstr_freq_band_data->freq_band_tbl_hi[i];
      ui = frame_data->pstr_sbr_header->pstr_freq_band_data
               ->freq_band_tbl_hi[i + 1];
      tmp = ((ui + li) - (sub_band_start << 1)) >> 1;

      if ((tmp >= 64) || (tmp < 0)) {
        if (ec_flag)
          tmp = 0;
        else
          return -1;
      }

      harmonics[tmp] = add_harmonics[i];
    }

    for (t = 0; t < p_frame_info->border_vec[0]; t++) {
      for (c = 0; c < 64; c++) {
        frame_data->qmapped_pvc[c][t] = frame_data->qmapped_pvc[c][t + 16];
      }
    }

    for (i = 0; i < bs_num_env; i++) {
      if (kk > MAX_NOISE_ENVELOPES) {
        if (ec_flag)
          kk = MAX_NOISE_ENVELOPES;
        else
          return IA_FATAL_ERROR;
      }
      if (p_frame_info->border_vec[i] == p_frame_info->noise_border_vec[kk]) kk++, next++;

      start_pos = p_frame_info->border_vec[i];
      end_pos = p_frame_info->border_vec[i + 1];
      if ((start_pos < 0) || (end_pos > MAX_FREQ_COEFFS_SBR)) {
        if (ec_flag) {
          start_pos = 0;
          end_pos = MAX_FREQ_COEFFS_SBR;
        } else
          return IA_FATAL_ERROR;
      }

      for (t = start_pos; t < end_pos; t++) {
        band_loop_end = num_sf_bands[p_frame_info->freq_res[i]];

        for (c = 0, o = 0, j = 0; j < band_loop_end; j++) {
          li = freq_band_table[p_frame_info->freq_res[i]][j];
          ui = freq_band_table[p_frame_info->freq_res[i]][j + 1];
          ui2 = frame_data->pstr_sbr_header->pstr_freq_band_data
                    ->freq_band_tbl_noise[o + 1];

          for (k = 0; k < ui - li; k++) {
            o = (k + li >= ui2) ? o + 1 : o;
            if (o >= MAX_NOISE_COEFFS) {
              if (ec_flag)
                o = MAX_NOISE_COEFFS - 1;
              else
                return IA_FATAL_ERROR;
            }
            ui2 = freq_band_table_noise[o + 1];

            frame_data->qmapped_pvc[c][t] =
                noise_floor[next * num_nf_bands + o];
            c++;
          }
        }
      }
    }

    kk = 0;
    next = -1;

    for (i = 0; i < bs_num_env; i++) {
      if (kk > MAX_NOISE_ENVELOPES) {
        if (ec_flag)
          kk = MAX_NOISE_ENVELOPES;
        else
          return IA_FATAL_ERROR;
      }
      if (p_frame_info->border_vec[i] == p_frame_info->noise_border_vec[kk]) kk++, next++;

      start_pos = pvc_frame_info->border_vec[i];
      end_pos = pvc_frame_info->border_vec[i + 1];
      if ((start_pos < 0) || (end_pos > MAX_FREQ_COEFFS_SBR)) {
        if (ec_flag) {
          start_pos = 0;
          end_pos = MAX_FREQ_COEFFS_SBR;
        } else
          return IA_FATAL_ERROR;
      }
      for (t = start_pos; t < end_pos; t++) {
        for (c = 0; c < 64; c++) {
          env_tmp[c][t] = env_out[64 * t + c];
        }
      }

      noise_absc_flag =
          (i == trans_env || i == frame_data->env_short_flag_prev) ? 1 : 0;

      if (prev_sbr_mode == ORIG_SBR) noise_absc_flag = 0;

      smooth_length = (noise_absc_flag ? 0 : smoothing_length);
      smooth_filt = *ixheaac_fir_table[smooth_length];

      for (t = start_pos; t < frame_data->sin_len_for_cur_top; t++) {
        band_loop_end =
            num_sf_bands[frame_data->str_frame_info_prev
                             .freq_res[frame_data->var_len_id_prev]];

        for (c = 0, o = 0, j = 0; j < band_loop_end; j++) {
          double tmp;

          li = freq_band_table[frame_data->str_frame_info_prev
                                   .freq_res[frame_data->var_len_id_prev]][j];
          ui = freq_band_table[frame_data->str_frame_info_prev
                                   .freq_res[frame_data->var_len_id_prev]]
                              [j + 1];
          ui2 = frame_data->pstr_sbr_header->pstr_freq_band_data
                    ->freq_band_tbl_noise[o + 1];

          for (flag = 0, k = li; k < ui; k++) {
            flag = ((*harm_flag_varlen)[c] &&
                    (t >= frame_data->sin_start_for_cur_top ||
                     (*harm_flag_varlen_prev)[c + sub_band_start]))
                       ? 1
                       : flag;

            nrg_ref_pvc[c][t] = env_tmp[k][t];
            for (nrg = 0, l = 0; l < rate; l++) {
              nrg +=
                  (input_real[rate * t + l][k] * input_real[rate * t + l][k]) +
                  (input_imag[rate * t + l][k] * input_imag[rate * t + l][k]);
            }
            nrg_est_pvc[c][t] = nrg / rate;
            c++;
          }

          if (!int_mode && ui != li) {
            for (nrg = 0, k = c - (ui - li); k < c; k++) {
              nrg += nrg_est_pvc[k][t];
            }
            nrg /= (ui - li);
          } else {
            nrg = 0;
          }
          c -= (ui - li);

          for (k = 0; k < ui - li; k++) {
            o = (k + li >= ui2) ? o + 1 : o;
            if (o >= MAX_NOISE_COEFFS) {
              if (ec_flag)
                o = MAX_NOISE_COEFFS - 1;
              else
                return IA_FATAL_ERROR;
            }
            ui2 = freq_band_table_noise[o + 1];
            nrg_est_pvc[c][t] = (!int_mode) ? nrg : nrg_est_pvc[c][t];
            nrg_tone_pvc[c][t] = 0.0f;

            tmp = frame_data->qmapped_pvc[c][t] /
                  (1 + frame_data->qmapped_pvc[c][t] + guard);

            if (flag) {
              nrg_gain_pvc[c][t] = (FLOAT32)sqrt(nrg_ref_pvc[c][t] * tmp /
                                                 (nrg_est_pvc[c][t] + 1));

              nrg_tone_pvc[c][t] = (FLOAT32)(
                  (harmonics[c] && (t >= frame_data->sine_position ||
                                    (*harm_flag_prev)[c + sub_band_start]))
                      ? sqrt(nrg_ref_pvc[c][t] * tmp /
                             (frame_data->qmapped_pvc[c][t] + guard))
                      : nrg_tone_pvc[c][t]);

              nrg_tone_pvc[c][t] =
                  (FLOAT32)(((*harm_flag_varlen)[c] &&
                             (t >= frame_data->sin_start_for_cur_top ||
                              (*harm_flag_varlen_prev)[c + sub_band_start]))
                                ? sqrt(nrg_ref_pvc[c][t] * tmp /
                                       (prev_env_noise_level[o] + guard))
                                : nrg_tone_pvc[c][t]);

            } else {
              if (noise_absc_flag) {
                nrg_gain_pvc[c][t] =
                    (FLOAT32)sqrt(nrg_ref_pvc[c][t] / (nrg_est_pvc[c][t] + 1));
              } else {
                nrg_gain_pvc[c][t] = (FLOAT32)sqrt(
                    nrg_ref_pvc[c][t] * tmp /
                    ((nrg_est_pvc[c][t] + 1) * (frame_data->qmapped_pvc[c][t] + guard)));
              }
            }

            noise_level_pvc[c][t] = (FLOAT32)sqrt(nrg_ref_pvc[c][t] * tmp);
            c++;
          }
        }

        for (c = 0; c < (*gate_mode)[limiter_band]; c++) {
          p_ref = p_est = 0.0f;
          p_adj = 0;
          for (k = (*lim_table)[limiter_band][c];
               k < (*lim_table)[limiter_band][c + 1]; k++) {
            p_ref += nrg_ref_pvc[k][t];
            p_est += nrg_est_pvc[k][t];
          }
          avg_gain = (FLOAT32)sqrt((p_ref + EPS) / (p_est + EPS));
          g_max = avg_gain * ixheaac_g_lim_gains[limiter_gains];
          g_max > 1.0e5f ? g_max = 1.0e5f : 0;
          for (k = (*lim_table)[limiter_band][c];
               k < (*lim_table)[limiter_band][c + 1]; k++) {
            if (g_max <= nrg_gain_pvc[k][t]) {
              noise_level_pvc[k][t] =
                  (FLOAT32)(noise_level_pvc[k][t] * (g_max / (nrg_gain_pvc[k][t] + guard)));
              nrg_gain_pvc[k][t] = g_max;
            }

            p_adj +=
                nrg_gain_pvc[k][t] * nrg_gain_pvc[k][t] * nrg_est_pvc[k][t];

            if (nrg_tone_pvc[k][t]) {
              p_adj += nrg_tone_pvc[k][t] * nrg_tone_pvc[k][t];
            } else if (!noise_absc_flag) {
              p_adj += noise_level_pvc[k][t] * noise_level_pvc[k][t];
            }
          }
          boost_gain = (FLOAT32)sqrt((p_ref + EPS) / (p_adj + EPS));
          boost_gain = boost_gain > 1.584893192f ? 1.584893192f : boost_gain;

          for (k = (*lim_table)[limiter_band][c];
               k < (*lim_table)[limiter_band][c + 1]; k++) {
            nrg_gain_pvc[k][t] *= boost_gain;
            noise_level_pvc[k][t] *= boost_gain;
            nrg_tone_pvc[k][t] *= boost_gain;
          }
        }
      }

      for (; t < end_pos; t++) {
        band_loop_end = num_sf_bands[pvc_frame_info->freq_res[i]];

        for (c = 0, o = 0, j = 0; j < band_loop_end; j++) {
          double tmp;

          li = freq_band_table[pvc_frame_info->freq_res[i]][j];
          ui = freq_band_table[pvc_frame_info->freq_res[i]][j + 1];
          ui2 = frame_data->pstr_sbr_header->pstr_freq_band_data
                    ->freq_band_tbl_noise[o + 1];

          for (flag = 0, k = li; k < ui; k++) {
            flag = (harmonics[c] && (t >= frame_data->sine_position ||
                                     (*harm_flag_prev)[c + sub_band_start]))
                       ? 1
                       : flag;

            nrg_ref_pvc[c][t] = env_tmp[k][t];
            for (nrg = 0, l = 0; l < rate; l++) {
              nrg +=
                  (input_real[rate * t + l][k] * input_real[rate * t + l][k]) +
                  (input_imag[rate * t + l][k] * input_imag[rate * t + l][k]);
            }
            nrg_est_pvc[c][t] = nrg / rate;
            c++;
          }

          if (!int_mode && ui != li) {
            for (nrg = 0, k = c - (ui - li); k < c; k++) {
              nrg += nrg_est_pvc[k][t];
            }
            nrg /= (ui - li);
          } else {
            nrg = 0;
          }
          c -= (ui - li);

          for (k = 0; k < ui - li; k++) {
            o = (k + li >= ui2) ? o + 1 : o;
            if (o >= MAX_NOISE_COEFFS) {
              if (ec_flag)
                o = MAX_NOISE_COEFFS - 1;
              else
                return IA_FATAL_ERROR;
            }
            ui2 = freq_band_table_noise[o + 1];
            nrg_est_pvc[c][t] = (!int_mode) ? nrg : nrg_est_pvc[c][t];
            nrg_tone_pvc[c][t] = 0.0f;

            tmp = frame_data->qmapped_pvc[c][t] /
                  (1 + frame_data->qmapped_pvc[c][t] + guard);

            if (flag) {
              nrg_gain_pvc[c][t] = (FLOAT32)sqrt(nrg_ref_pvc[c][t] * tmp /
                                                 (nrg_est_pvc[c][t] + 1));

              nrg_tone_pvc[c][t] = (FLOAT32)(
                  (harmonics[c] && (t >= frame_data->sine_position ||
                                    (*harm_flag_prev)[c + sub_band_start]))
                      ? sqrt(nrg_ref_pvc[c][t] * tmp /
                             (frame_data->qmapped_pvc[c][t] + guard))
                      : nrg_tone_pvc[c][t]);
            } else {
              if (noise_absc_flag) {
                nrg_gain_pvc[c][t] =
                    (FLOAT32)sqrt(nrg_ref_pvc[c][t] / (nrg_est_pvc[c][t] + 1));
              } else {
                nrg_gain_pvc[c][t] = (FLOAT32)sqrt(
                    nrg_ref_pvc[c][t] * tmp /
                    ((nrg_est_pvc[c][t] + 1) * (frame_data->qmapped_pvc[c][t] + guard)));
              }
            }

            noise_level_pvc[c][t] = (FLOAT32)sqrt(nrg_ref_pvc[c][t] * tmp);
            c++;
          }
        }

        for (c = 0; c < (*gate_mode)[limiter_band]; c++) {
          p_ref = p_est = 0.0f;
          p_adj = 0;
          for (k = (*lim_table)[limiter_band][c];
               k < (*lim_table)[limiter_band][c + 1]; k++) {
            p_ref += nrg_ref_pvc[k][t];
            p_est += nrg_est_pvc[k][t];
          }
          avg_gain = (FLOAT32)sqrt((p_ref + EPS) / (p_est + EPS));
          g_max = avg_gain * ixheaac_g_lim_gains[limiter_gains];
          g_max > 1.0e5f ? g_max = 1.0e5f : 0;

          for (k = (*lim_table)[limiter_band][c];
               k < (*lim_table)[limiter_band][c + 1]; k++) {
            if (g_max <= nrg_gain_pvc[k][t]) {
              noise_level_pvc[k][t] =
                  (FLOAT32)(noise_level_pvc[k][t] *
                            (g_max / (nrg_gain_pvc[k][t] + guard)));
              nrg_gain_pvc[k][t] = g_max;
            }

            p_adj +=
                nrg_gain_pvc[k][t] * nrg_gain_pvc[k][t] * nrg_est_pvc[k][t];

            if (nrg_tone_pvc[k][t]) {
              p_adj += nrg_tone_pvc[k][t] * nrg_tone_pvc[k][t];
            } else if (!noise_absc_flag) {
              p_adj += noise_level_pvc[k][t] * noise_level_pvc[k][t];
            }
          }

          boost_gain = (FLOAT32)sqrt((p_ref + EPS) / (p_adj + EPS));
          boost_gain = boost_gain > 1.584893192f ? 1.584893192f : boost_gain;

          for (k = (*lim_table)[limiter_band][c];
               k < (*lim_table)[limiter_band][c + 1]; k++) {
            nrg_gain_pvc[k][t] *= boost_gain;
            noise_level_pvc[k][t] *= boost_gain;
            nrg_tone_pvc[k][t] *= boost_gain;
          }
        }
      }

      if (esbr_start_up_pvc) {
        for (n = 0; n < 4; n++) {
          for (c = 0; c < num_subbands; c++) {
            (*e_gain)[n][c] = nrg_gain_pvc[c][start_pos];
            (*noise_buf)[n][c] = noise_level_pvc[c][start_pos];
          }
        }
        esbr_start_up_pvc = 0;
        esbr_start_up = 0;
      }
      for (l = rate * pvc_frame_info->border_vec[i];
           l < rate * pvc_frame_info->border_vec[1 + i]; l++) {
        ptr_real_buf = *(input_real + l) + sub_band_start;
        ptr_imag_buf = *(input_imag + l) + sub_band_start;

        slot_idx = (WORD32)l / rate;
        if (sub_band_start & 1) {
          freq_inv = -1;
        } else {
          freq_inv = 1;
        }

        for (k = 0; k < num_subbands; k++) {
          (*e_gain)[4][k] = nrg_gain_pvc[k][slot_idx];
          (*noise_buf)[4][k] = noise_level_pvc[k][slot_idx];
          c = 0, sb_gain = 0, sb_noise = 0;
          for (n = 4 - smooth_length; n <= 4; n++) {
            sb_gain += (*e_gain)[n][k] * smooth_filt[c];
            sb_noise += (*noise_buf)[n][k] * smooth_filt[c++];
          }
          phase_index = (phase_index + 1) & 511;
          sb_noise = (nrg_tone_pvc[k][slot_idx] != 0 || noise_absc_flag)
                         ? 0
                         : sb_noise;

          *ptr_real_buf =
              *ptr_real_buf * sb_gain +
              sb_noise * ixheaac_random_phase[phase_index][0] +
              nrg_tone_pvc[k][slot_idx] * ixheaac_hphase_tbl[0][harm_index];
          *ptr_imag_buf = *ptr_imag_buf * sb_gain +
                          sb_noise * ixheaac_random_phase[phase_index][1] +
                          nrg_tone_pvc[k][slot_idx] * freq_inv *
                              ixheaac_hphase_tbl[1][harm_index];

          ptr_real_buf++;
          ptr_imag_buf++;
          freq_inv = -freq_inv;
        }

        harm_index = (harm_index + 1) & 3;

        memcpy(temp, (*e_gain)[0], 64 * sizeof(FLOAT32));
        for (n = 0; n < 4; n++) {
          memcpy((*e_gain)[n], (*e_gain)[n + 1], 64 * sizeof(FLOAT32));
        }
        memcpy((*e_gain)[4], temp, 64 * sizeof(FLOAT32));

        memcpy(temp, (*noise_buf)[0], 64 * sizeof(FLOAT32));
        for (n = 0; n < 4; n++) {
          memcpy((*noise_buf)[n], (*noise_buf)[n + 1], 64 * sizeof(FLOAT32));
        }
        memcpy((*noise_buf)[4], temp, 64 * sizeof(FLOAT32));
      }
    }
  } else {
    for (i = 0; i < num_sf_bands[HIGH]; i++) {
      li =
          frame_data->pstr_sbr_header->pstr_freq_band_data->freq_band_tbl_hi[i];
      ui = frame_data->pstr_sbr_header->pstr_freq_band_data
               ->freq_band_tbl_hi[i + 1];
      tmp = ((ui + li) - (sub_band_start << 1)) >> 1;
      if ((tmp >= 64) || (tmp < 0)) return -1;

      harmonics[tmp] = add_harmonics[i];
    }

    for (i = 0; i < bs_num_env; i++) {
      if (kk > MAX_NOISE_ENVELOPES) {
        if (ec_flag)
          kk = MAX_NOISE_ENVELOPES;
        else
          return IA_FATAL_ERROR;
      }

      if (p_frame_info->border_vec[i] == p_frame_info->noise_border_vec[kk])
        kk++, next++;

      noise_absc_flag =
          (i == trans_env || i == frame_data->env_short_flag_prev) ? 1 : 0;

      smooth_length = (noise_absc_flag ? 0 : smoothing_length);
      smooth_filt = *ixheaac_fir_table[smooth_length];

      if (sbr_mode == ORIG_SBR) {
        for (c = 0, o = 0, j = 0; j < num_sf_bands[p_frame_info->freq_res[i]];
             j++) {
          double tmp;
          li = freq_band_table[p_frame_info->freq_res[i]][j];
          ui = freq_band_table[p_frame_info->freq_res[i]][j + 1];
          ui2 = frame_data->pstr_sbr_header->pstr_freq_band_data
                    ->freq_band_tbl_noise[o + 1];

          if (p_frame_info->border_vec[i] >= p_frame_info->border_vec[i + 1]) {
            for (flag = 0, k = li; k < ui; k++) {
              flag = (harmonics[c] &&
                (i >= trans_env || (*harm_flag_prev)[c + sub_band_start]))
                ? 1
                : flag;
              nrg_est[c++] = 0;
            }
          } else {
            for (flag = 0, k = li; k < ui; k++) {
              for (nrg = 0, l = rate * p_frame_info->border_vec[i];
                l < rate * p_frame_info->border_vec[i + 1]; l++) {
                nrg += (input_real[l][k] * input_real[l][k]) +
                  (input_imag[l][k] * input_imag[l][k]);
              }
              flag = (harmonics[c] &&
                (i >= trans_env || (*harm_flag_prev)[c + sub_band_start]))
                ? 1
                : flag;
              nrg_est[c++] = nrg / (rate * p_frame_info->border_vec[i + 1] -
                rate * p_frame_info->border_vec[i]);
            }
          }
          if (!int_mode && ui != li) {
            for (nrg = 0, k = c - (ui - li); k < c; k++) {
              nrg += nrg_est[k];
            }
            nrg /= (ui - li);
          } else {
            nrg = 0;
          }
          c -= (ui - li);

          for (k = 0; k < ui - li; k++) {
            o = (k + li >= ui2) ? o + 1 : o;
            if (o >= MAX_NOISE_COEFFS) {
              if (ec_flag)
                o = MAX_NOISE_COEFFS - 1;
              else
                return IA_FATAL_ERROR;
            }
            ui2 = frame_data->pstr_sbr_header->pstr_freq_band_data
                      ->freq_band_tbl_noise[o + 1];
            nrg_ref[c] = sfb_nrg[m];
            nrg_est[c] = (!int_mode) ? nrg : nrg_est[c];
            nrg_tone[c] = 0;
            tmp = noise_floor[next * num_nf_bands + o] /
                  (1 + noise_floor[next * num_nf_bands + o] + guard);
            if (flag) {
              nrg_gain[c] = (FLOAT32)sqrt(nrg_ref[c] * tmp / (nrg_est[c] + 1));
              nrg_tone[c] = (FLOAT32)(
                  (harmonics[c] &&
                   (i >= trans_env || (*harm_flag_prev)[c + sub_band_start]))
                      ? sqrt(nrg_ref[c] * tmp /
                             ABS(noise_floor[next * num_nf_bands + o] + guard))
                      : nrg_tone[c]);
            } else {
              if (noise_absc_flag)
                nrg_gain[c] = (FLOAT32)sqrt(nrg_ref[c] / (nrg_est[c] + 1));
              else
                nrg_gain[c] = (FLOAT32)sqrt(
                    nrg_ref[c] * tmp /
                    ((nrg_est[c] + 1) *
                     ABS(noise_floor[next * num_nf_bands + o] + guard)));
            }
            noise_level[c] = (FLOAT32)sqrt(nrg_ref[c] * tmp);
            c++;
          }
          m++;
        }

        for (c = 0; c < (*gate_mode)[limiter_band]; c++) {
          p_ref = p_est = 0;
          for (k = (*lim_table)[limiter_band][c];
               k < (*lim_table)[limiter_band][c + 1]; k++) {
            p_ref += nrg_ref[k];
            p_est += nrg_est[k];
          }
          avg_gain = (FLOAT32)sqrt((p_ref + EPS) / (p_est + EPS));
          g_max = avg_gain * ixheaac_g_lim_gains[limiter_gains];
          g_max > 1.0e5f ? g_max = 1.0e5f : 0;
          for (k = (*lim_table)[limiter_band][c];
               k < (*lim_table)[limiter_band][c + 1]; k++) {
            if (g_max <= nrg_gain[k]) {
              noise_level[k] = (FLOAT32)(noise_level[k] * (g_max / (nrg_gain[k] + guard)));
              nrg_gain[k] = g_max;
            }
          }
          p_adj = 0;
          for (k = (*lim_table)[limiter_band][c];
               k < (*lim_table)[limiter_band][c + 1]; k++) {
            p_adj += nrg_gain[k] * nrg_gain[k] * nrg_est[k];
            if (nrg_tone[k])
              p_adj += nrg_tone[k] * nrg_tone[k];
            else if (!noise_absc_flag)
              p_adj += noise_level[k] * noise_level[k];
          }
          boost_gain = (FLOAT32)sqrt((p_ref + EPS) / (p_adj + EPS));
          boost_gain = boost_gain > 1.584893192f ? 1.584893192f : boost_gain;
          for (k = (*lim_table)[limiter_band][c];
               k < (*lim_table)[limiter_band][c + 1]; k++) {
            nrg_gain[k] *= boost_gain;
            noise_level[k] *= boost_gain;
            nrg_tone[k] *= boost_gain;
          }
        }

        if (esbr_start_up && (ldmps_present != 1)) {
          for (n = 0; n < 4; n++) {
            memcpy((*e_gain)[n], nrg_gain, num_subbands * sizeof(FLOAT32));
            memcpy((*noise_buf)[n], noise_level,
                   num_subbands * sizeof(FLOAT32));
          }
          esbr_start_up = 0;
          esbr_start_up_pvc = 0;
        }

        for (l = rate * p_frame_info->border_vec[i];
             l < rate * p_frame_info->border_vec[i + 1]; l++) {
          ptr_real_buf = *(input_real + l) + sub_band_start;
          ptr_imag_buf = *(input_imag + l) + sub_band_start;

          freq_inv = 1;
          if ((ldmps_present == 1) && (sub_band_start & 1)) freq_inv = -1;

          for (k = 0; k < num_subbands; k++) {
            (*e_gain)[4][k] = nrg_gain[k];
            (*noise_buf)[4][k] = noise_level[k];
            c = 0, sb_gain = 0, sb_noise = 0;
            for (n = 4 - smooth_length; n <= 4; n++) {
              sb_gain += (*e_gain)[n][k] * smooth_filt[c];
              sb_noise += (*noise_buf)[n][k] * smooth_filt[c++];
            }

            phase_index = (phase_index + 1) & 511;
            sb_noise = (nrg_tone[k] != 0 || noise_absc_flag) ? 0 : sb_noise;

            if (ldmps_present == 1) {
              *ptr_real_buf = *ptr_real_buf * sb_gain +
                              sb_noise * ixheaac_random_phase[phase_index][0] +
                              nrg_tone[k] * ixheaac_hphase_tbl[0][harm_index];
              *ptr_imag_buf =
                  *ptr_imag_buf * sb_gain +
                  sb_noise * ixheaac_random_phase[phase_index][1] +
                  nrg_tone[k] * freq_inv * ixheaac_hphase_tbl[1][harm_index];

              freq_inv = -freq_inv;
            } else {
                *ptr_real_buf = *ptr_real_buf * sb_gain +
                            sb_noise * ixheaac_random_phase[phase_index][0];
                *ptr_imag_buf = *ptr_imag_buf * sb_gain +
                            sb_noise * ixheaac_random_phase[phase_index][1];
            }

            ptr_real_buf++;
            ptr_imag_buf++;
          }
          if (ldmps_present == 1) harm_index = (harm_index + 1) & 3;

          memcpy(temp, (*e_gain)[0], 64 * sizeof(FLOAT32));
          for (n = 0; n < 4; n++)
            memcpy((*e_gain)[n], (*e_gain)[n + 1], 64 * sizeof(FLOAT32));
          memcpy((*e_gain)[4], temp, 64 * sizeof(FLOAT32));
          memcpy(temp, (*noise_buf)[0], 64 * sizeof(FLOAT32));
          for (n = 0; n < 4; n++)
            memcpy((*noise_buf)[n], (*noise_buf)[n + 1], 64 * sizeof(FLOAT32));
          memcpy((*noise_buf)[4], temp, 64 * sizeof(FLOAT32));
        }

        if (ldmps_present != 1) {
          err_code = ixheaacd_apply_inter_tes(
              *(input_real1 + rate * p_frame_info->border_vec[i]),
              *(input_imag1 + rate * p_frame_info->border_vec[i]),
              *(input_real + rate * p_frame_info->border_vec[i]),
              *(input_imag + rate * p_frame_info->border_vec[i]),
              rate * p_frame_info->border_vec[i + 1] - rate * p_frame_info->border_vec[i],
              sub_band_start, num_subbands, frame_data->inter_temp_shape_mode[i]);
          if (err_code != 0) {
            return err_code;
          }

        for (l = rate * p_frame_info->border_vec[i];
             l < rate * p_frame_info->border_vec[i + 1]; l++) {
          ptr_real_buf = *(input_real + l) + sub_band_start;
          ptr_imag_buf = *(input_imag + l) + sub_band_start;
          if (sub_band_start & 1) {
            freq_inv = -1;
          } else {
            freq_inv = 1;
          }
          for (k = 0; k < num_subbands; k++) {
            *ptr_real_buf += nrg_tone[k] * ixheaac_hphase_tbl[0][harm_index];
            *ptr_imag_buf +=
                nrg_tone[k] * freq_inv * ixheaac_hphase_tbl[1][harm_index];

            ptr_real_buf++;
            ptr_imag_buf++;
            freq_inv = -freq_inv;
          }
          harm_index = (harm_index + 1) & 3;
        }
      }
    }
  }
}

  for (i = 0; i < 64; i++) {
    (*harm_flag_varlen_prev)[i] = (*harm_flag_prev)[i];
    (*harm_flag_varlen)[i] = harmonics[i];
  }

  memcpy(&((*harm_flag_prev)[0]) + sub_band_start, harmonics,
         (64 - sub_band_start) * sizeof(WORD8));

  if (trans_env == bs_num_env) {
    frame_data->env_short_flag_prev = 0;
  } else {
    frame_data->env_short_flag_prev = -1;
  }

  if (ldmps_present != 1) {
  memcpy((VOID *)&frame_data->str_frame_info_prev,
         (VOID *)&frame_data->str_frame_info_details,
         sizeof(ia_frame_info_struct));

  if (frame_data->str_frame_info_details.num_env == 1) {
    frame_data->var_len_id_prev = 0;
  } else if (frame_data->str_frame_info_details.num_env == 2) {
    frame_data->var_len_id_prev = 1;
  }

  if ((frame_data->str_frame_info_details.num_noise_env < 1) ||
        (frame_data->str_frame_info_details.num_noise_env > 2)) {
      if (ec_flag)
        frame_data->str_frame_info_details.num_noise_env = 1;
      else
        return IA_FATAL_ERROR;
    }

  for (i = 0; i < num_nf_bands; i++) {
    prev_env_noise_level[i] =
        frame_data->flt_noise_floor
            [(frame_data->str_frame_info_details.num_noise_env - 1) *
                 num_nf_bands +
             i];
  }
}

  frame_data->harm_index = harm_index;
  frame_data->phase_index = phase_index;
  frame_data->pstr_sbr_header->esbr_start_up = esbr_start_up;
  frame_data->pstr_sbr_header->esbr_start_up_pvc = esbr_start_up_pvc;
  return 0;
}

IA_ERRORCODE ixheaacd_createlimiterbands(WORD32 lim_table[4][12 + 1], WORD32 gate_mode[4],
                                         WORD16 *freq_band_tbl, WORD32 ixheaacd_num_bands,
                                         WORD32 x_over_qmf[MAX_NUM_PATCHES],
                                         WORD32 b_patching_mode, WORD32 upsamp_4_flag,
                                         struct ixheaacd_lpp_trans_patch *patch_param,
                                         WORD32 ec_flag) {
  WORD32 i, j, k, is_patch_border[2];
  WORD32 patch_borders[MAX_NUM_PATCHES + 1];
  WORD32 temp_limiter_band_calc[32 + MAX_NUM_PATCHES + 1];

  double num_octave;
  WORD32 num_patches;

  WORD32 sub_band_start = freq_band_tbl[0];
  WORD32 sub_band_end = freq_band_tbl[ixheaacd_num_bands];

  const double log2 = log(2.0);
  const double limbnd_per_oct[4] = {0, 1.2, 2.0, 3.0};

  if (!b_patching_mode && (x_over_qmf != NULL)) {
    num_patches = 0;
    if (upsamp_4_flag) {
      for (i = 1; i < MAX_NUM_PATCHES; i++)
        if (x_over_qmf[i] != 0) num_patches++;
    } else {
      for (i = 1; i < 4; i++)
        if (x_over_qmf[i] != 0) num_patches++;
    }
    for (i = 0; i < num_patches; i++) {
      patch_borders[i] = x_over_qmf[i] - sub_band_start;
    }
  } else {
    num_patches = patch_param->num_patches;
    for (i = 0; i < num_patches; i++) {
      patch_borders[i] = patch_param->start_subband[i] - sub_band_start;
    }
  }
  patch_borders[i] = sub_band_end - sub_band_start;

  lim_table[0][0] = freq_band_tbl[0] - sub_band_start;
  lim_table[0][1] = freq_band_tbl[ixheaacd_num_bands] - sub_band_start;
  gate_mode[0] = 1;

  for (i = 1; i < 4; i++) {
    for (k = 0; k <= ixheaacd_num_bands; k++) {
      temp_limiter_band_calc[k] = freq_band_tbl[k] - sub_band_start;
    }

    for (k = 1; k < num_patches; k++) {
      temp_limiter_band_calc[ixheaacd_num_bands + k] = patch_borders[k];
    }

    gate_mode[i] = ixheaacd_num_bands + num_patches - 1;
    ixheaacd_shellsort(temp_limiter_band_calc, gate_mode[i] + 1);

    for (j = 1; j <= gate_mode[i]; j++) {
      num_octave = log((double)(temp_limiter_band_calc[j] + sub_band_start) /
                       (temp_limiter_band_calc[j - 1] + sub_band_start)) /
                   log2;

      if (num_octave * limbnd_per_oct[i] < 0.49) {
        if (temp_limiter_band_calc[j] == temp_limiter_band_calc[j - 1]) {
          temp_limiter_band_calc[j] = sub_band_end;
          ixheaacd_shellsort(temp_limiter_band_calc, gate_mode[i] + 1);
          gate_mode[i]--;
          j--;
          continue;
        }

        is_patch_border[0] = is_patch_border[1] = 0;

        for (k = 0; k <= num_patches; k++) {
          if (temp_limiter_band_calc[j - 1] == patch_borders[k]) {
            is_patch_border[0] = 1;
            break;
          }
        }

        for (k = 0; k <= num_patches; k++) {
          if (temp_limiter_band_calc[j] == patch_borders[k]) {
            is_patch_border[1] = 1;
            break;
          }
        }

        if (!is_patch_border[1]) {
          temp_limiter_band_calc[j] = sub_band_end;
          ixheaacd_shellsort(temp_limiter_band_calc, gate_mode[i] + 1);
          gate_mode[i]--;
          j--;
        } else if (!is_patch_border[0]) {
          temp_limiter_band_calc[j - 1] = sub_band_end;
          ixheaacd_shellsort(temp_limiter_band_calc, gate_mode[i] + 1);
          gate_mode[i]--;
          j--;
        }
      }
    }
    if (gate_mode[i] > 12) {
      if (ec_flag)
        gate_mode[i] = 12;
      else
        return IA_FATAL_ERROR;
    }
    for (k = 0; k <= gate_mode[i]; k++) {
      lim_table[i][k] = temp_limiter_band_calc[k];
    }
  }
  return IA_NO_ERROR;
}

WORD32 ixheaacd_apply_inter_tes(FLOAT32 *qmf_real1, FLOAT32 *qmf_imag1,
                                FLOAT32 *qmf_real, FLOAT32 *qmf_imag,
                                WORD32 num_sample, WORD32 sub_band_start,
                                WORD32 num_subband, WORD32 gamma_idx) {
  WORD32 sub_band_end = sub_band_start + num_subband;
  FLOAT32 subsample_power_high[TIMESLOT_BUFFER_SIZE],
      subsample_power_low[TIMESLOT_BUFFER_SIZE];
  FLOAT32 total_power_high = 0.0f;
  FLOAT32 total_power_low = 0.0f, total_power_high_after = 1.0e-6f;
  FLOAT32 gain[TIMESLOT_BUFFER_SIZE];
  FLOAT32 gain_adj, gain_adj_2;
  FLOAT32 gamma = ixheaac_q_gamma_table[gamma_idx];
  WORD32 i, j;

  if (num_sample > TIMESLOT_BUFFER_SIZE)
  {
    return IA_FATAL_ERROR;
  }
  if (gamma > 0) {
    for (i = 0; i < num_sample; i++) {
      memcpy(&qmf_real[64 * i], &qmf_real1[64 * i],
             sub_band_start * sizeof(FLOAT32));
      memcpy(&qmf_imag[64 * i], &qmf_imag1[64 * i],
             sub_band_start * sizeof(FLOAT32));
    }

    for (i = 0; i < num_sample; i++) {
      subsample_power_low[i] = 0.0f;
      for (j = 0; j < sub_band_start; j++) {
        subsample_power_low[i] += qmf_real[64 * i + j] * qmf_real[64 * i + j];
        subsample_power_low[i] += qmf_imag[64 * i + j] * qmf_imag[64 * i + j];
      }
      subsample_power_high[i] = 0.0f;
      for (j = sub_band_start; j < sub_band_end; j++) {
        subsample_power_high[i] += qmf_real[64 * i + j] * qmf_real[64 * i + j];
        subsample_power_high[i] += qmf_imag[64 * i + j] * qmf_imag[64 * i + j];
      }
      total_power_low += subsample_power_low[i];
      total_power_high += subsample_power_high[i];
    }

    for (i = 0; i < num_sample; i++) {
      gain[i] = (FLOAT32)(sqrt(subsample_power_low[i] * num_sample /
                               (total_power_low + 1.0e-6f)));
    }

    for (i = 0; i < num_sample; i++) {
      gain[i] = (FLOAT32)(1.0f + gamma * (gain[i] - 1.0f));
    }

    for (i = 0; i < num_sample; i++) {
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

      for (j = sub_band_start; j < sub_band_end; j++) {
        qmf_real[64 * i + j] *= gain[i];
        qmf_imag[64 * i + j] *= gain[i];
      }
    }
  }
  return 0;
}
