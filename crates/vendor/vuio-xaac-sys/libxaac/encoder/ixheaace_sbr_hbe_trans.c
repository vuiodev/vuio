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
#include <stdlib.h>
#include <string.h>

#include "ixheaac_type_def.h"
#include "ixheaace_bitbuffer.h"
#include "iusace_tns_usac.h"
#include "iusace_cnst.h"
#include "ixheaace_sbr_def.h"

#include "ixheaace_sbr_hbe.h"
#include "ixheaace_sbr_hbe_fft.h"
#include "ixheaac_error_standards.h"
#include "ixheaace_error_codes.h"
#include "ixheaac_constants.h"
#include "ixheaac_esbr_rom.h"

static FLOAT32 *ixheaace_map_prot_filter(WORD32 filt_length) {
  switch (filt_length) {
    case 4:
      return (FLOAT32 *)&ixheaac_sub_samp_qmf_window_coeff[0];
      break;
    case 8:
      return (FLOAT32 *)&ixheaac_sub_samp_qmf_window_coeff[40];
      break;
    case 12:
      return (FLOAT32 *)&ixheaac_sub_samp_qmf_window_coeff[120];
      break;
    case 16:
      return (FLOAT32 *)&ixheaac_sub_samp_qmf_window_coeff[240];
      break;
    case 20:
      return (FLOAT32 *)&ixheaac_sub_samp_qmf_window_coeff[400];
      break;
    case 24:
      return (FLOAT32 *)&ixheaac_sub_samp_qmf_window_coeff[600];
      break;
    case 32:
      return (FLOAT32 *)&ixheaac_sub_samp_qmf_window_coeff[840];
      break;
    case 40:
      return (FLOAT32 *)&ixheaac_sub_samp_qmf_window_coeff[1160];
      break;
    default:
      return (FLOAT32 *)&ixheaac_sub_samp_qmf_window_coeff[0];
  }
}

IA_ERRORCODE ixheaace_qmf_hbe_data_reinit(ixheaace_str_esbr_hbe_txposer *pstr_hbe_txposer) {
  WORD32 synth_size, sfb, patch, stop_patch;
  WORD32 upsamp_4_flag = 0;

  if (pstr_hbe_txposer != NULL) {
    pstr_hbe_txposer->start_band = pstr_hbe_txposer->ptr_freq_band_tab[IXHEAACE_LOW][0];
    pstr_hbe_txposer->end_band =
        pstr_hbe_txposer
            ->ptr_freq_band_tab[IXHEAACE_LOW][pstr_hbe_txposer->num_sf_bands[IXHEAACE_LOW]];

    pstr_hbe_txposer->synth_size = 4 * ((pstr_hbe_txposer->start_band + 4) / 8 + 1);
    pstr_hbe_txposer->k_start = ixheaac_start_subband2kL_tbl[pstr_hbe_txposer->start_band];

    upsamp_4_flag = pstr_hbe_txposer->upsamp_4_flag;
    pstr_hbe_txposer->esbr_hq = 0;

    if (upsamp_4_flag) {
      if (pstr_hbe_txposer->k_start + pstr_hbe_txposer->synth_size > 16)
        pstr_hbe_txposer->k_start = 16 - pstr_hbe_txposer->synth_size;
    } else if (pstr_hbe_txposer->core_frame_length == 768) {
      if (pstr_hbe_txposer->k_start + pstr_hbe_txposer->synth_size > 24)
        pstr_hbe_txposer->k_start = 24 - pstr_hbe_txposer->synth_size;
    }
    memset(pstr_hbe_txposer->synth_buf, 0, sizeof(pstr_hbe_txposer->synth_buf));
    synth_size = pstr_hbe_txposer->synth_size;
    pstr_hbe_txposer->synth_buf_offset = 18 * synth_size;
    switch (synth_size) {
      case 4:
        pstr_hbe_txposer->ptr_syn_cos_tab = (FLOAT32 *)ixheaac_synth_cos_table_kl_4;
        pstr_hbe_txposer->ptr_analy_cos_sin_tab = (FLOAT32 *)ixheaac_analy_cos_sin_table_kl_8;
        pstr_hbe_txposer->ixheaace_real_synth_fft = &ixheaac_real_synth_fft_p2;
        pstr_hbe_txposer->ixheaace_cmplx_anal_fft = &ixheaac_cmplx_anal_fft_p2;
        break;
      case 8:
        pstr_hbe_txposer->ptr_syn_cos_tab = (FLOAT32 *)ixheaac_synth_cos_table_kl_8;
        pstr_hbe_txposer->ptr_analy_cos_sin_tab = (FLOAT32 *)ixheaac_analy_cos_sin_table_kl_16;
        pstr_hbe_txposer->ixheaace_real_synth_fft = &ixheaac_real_synth_fft_p2;
        pstr_hbe_txposer->ixheaace_cmplx_anal_fft = &ixheaac_cmplx_anal_fft_p2;
        break;
      case 12:
        pstr_hbe_txposer->ptr_syn_cos_tab = (FLOAT32 *)ixheaac_synth_cos_table_kl_12;
        pstr_hbe_txposer->ptr_analy_cos_sin_tab = (FLOAT32 *)ixheaac_analy_cos_sin_table_kl_24;
        pstr_hbe_txposer->ixheaace_real_synth_fft = &ixheaac_real_synth_fft_p3;
        pstr_hbe_txposer->ixheaace_cmplx_anal_fft = &ixheaac_cmplx_anal_fft_p3;
        break;
      case 16:
        pstr_hbe_txposer->ptr_syn_cos_tab = (FLOAT32 *)ixheaac_synth_cos_table_kl_16;
        pstr_hbe_txposer->ptr_analy_cos_sin_tab = (FLOAT32 *)ixheaac_analy_cos_sin_table_kl_32;
        pstr_hbe_txposer->ixheaace_real_synth_fft = &ixheaac_real_synth_fft_p2;
        pstr_hbe_txposer->ixheaace_cmplx_anal_fft = &ixheaac_cmplx_anal_fft_p2;
        break;
      case 20:
        pstr_hbe_txposer->ptr_syn_cos_tab = (FLOAT32 *)ixheaac_synth_cos_table_kl_20;
        pstr_hbe_txposer->ptr_analy_cos_sin_tab = (FLOAT32 *)ixheaac_analy_cos_sin_table_kl_40;
        break;
      default:
        pstr_hbe_txposer->ptr_syn_cos_tab = (FLOAT32 *)ixheaac_synth_cos_table_kl_4;
        pstr_hbe_txposer->ptr_analy_cos_sin_tab = (FLOAT32 *)ixheaac_analy_cos_sin_table_kl_8;
        pstr_hbe_txposer->ixheaace_real_synth_fft = &ixheaac_real_synth_fft_p2;
        pstr_hbe_txposer->ixheaace_cmplx_anal_fft = &ixheaac_cmplx_anal_fft_p2;
    }

    pstr_hbe_txposer->ptr_syn_win_coeff = ixheaace_map_prot_filter(synth_size);

    memset(pstr_hbe_txposer->analy_buf, 0, sizeof(pstr_hbe_txposer->analy_buf));
    synth_size = 2 * pstr_hbe_txposer->synth_size;
    pstr_hbe_txposer->ptr_ana_win_coeff = ixheaace_map_prot_filter(synth_size);

    memset(pstr_hbe_txposer->x_over_qmf, 0, sizeof(pstr_hbe_txposer->x_over_qmf));
    sfb = 0;
    if (upsamp_4_flag) {
      stop_patch = IXHEAACE_MAX_NUM_PATCHES;
      pstr_hbe_txposer->max_stretch = IXHEAACE_MAX_STRETCH;
    } else {
      stop_patch = IXHEAACE_MAX_STRETCH;
    }

    for (patch = 1; patch <= stop_patch; patch++) {
      while (sfb <= pstr_hbe_txposer->num_sf_bands[IXHEAACE_LOW] &&
             pstr_hbe_txposer->ptr_freq_band_tab[IXHEAACE_LOW][sfb] <=
                 patch * pstr_hbe_txposer->start_band)
        sfb++;
      if (sfb <= pstr_hbe_txposer->num_sf_bands[IXHEAACE_LOW]) {
        if ((patch * pstr_hbe_txposer->start_band -
             pstr_hbe_txposer->ptr_freq_band_tab[IXHEAACE_LOW][sfb - 1]) <= 3) {
          pstr_hbe_txposer->x_over_qmf[patch - 1] =
              pstr_hbe_txposer->ptr_freq_band_tab[IXHEAACE_LOW][sfb - 1];
        } else {
          WORD32 sfb_idx = 0;
          while (sfb_idx <= pstr_hbe_txposer->num_sf_bands[IXHEAACE_HIGH] &&
                 pstr_hbe_txposer->ptr_freq_band_tab[IXHEAACE_HIGH][sfb_idx] <=
                     patch * pstr_hbe_txposer->start_band)
            sfb_idx++;
          pstr_hbe_txposer->x_over_qmf[patch - 1] =
              pstr_hbe_txposer->ptr_freq_band_tab[IXHEAACE_HIGH][sfb_idx - 1];
        }
      } else {
        pstr_hbe_txposer->x_over_qmf[patch - 1] = pstr_hbe_txposer->end_band;
        pstr_hbe_txposer->max_stretch = min(patch, IXHEAACE_MAX_STRETCH);
        break;
      }
    }
    if (pstr_hbe_txposer->k_start < 0) {
      return IA_EXHEAACE_EXE_NONFATAL_ESBR_INVALID_START_BAND;
    }
  }
  return IA_NO_ERROR;
}

IA_ERRORCODE ixheaace_qmf_hbe_apply(ixheaace_str_esbr_hbe_txposer *pstr_hbe_txposer,
                                    FLOAT32 qmf_buf_real[][64], FLOAT32 qmf_buf_imag[][64],
                                    WORD32 num_columns, FLOAT32 pv_qmf_buf_real[][64],
                                    FLOAT32 pv_qmf_buf_imag[][64], WORD32 pitch_in_bins) {
  WORD32 i, qmf_band_idx;
  WORD32 qmf_voc_columns = pstr_hbe_txposer->no_bins / 2;
  WORD32 err_code = IA_NO_ERROR;

  memcpy(
      pstr_hbe_txposer->ptr_input_buf,
      pstr_hbe_txposer->ptr_input_buf + pstr_hbe_txposer->no_bins * pstr_hbe_txposer->synth_size,
      pstr_hbe_txposer->synth_size * sizeof(pstr_hbe_txposer->ptr_input_buf[0]));

  if (pstr_hbe_txposer->ixheaace_cmplx_anal_fft == NULL) {
    err_code = ixheaace_qmf_hbe_data_reinit(pstr_hbe_txposer);
    if (err_code) {
      return err_code;
    }
  }

  err_code = ixheaace_real_synth_filt(pstr_hbe_txposer, num_columns, qmf_buf_real, qmf_buf_imag);
  if (err_code) {
    return err_code;
  }

  for (i = 0; i < IXHEAACE_HBE_OPER_WIN_LEN - 1; i++) {
    memcpy(pstr_hbe_txposer->qmf_in_buf[i], pstr_hbe_txposer->qmf_in_buf[i + qmf_voc_columns],
           sizeof(pstr_hbe_txposer->qmf_in_buf[i]));
  }

  err_code = ixheaace_complex_anal_filt(pstr_hbe_txposer);
  if (err_code) {
    return err_code;
  }

  for (i = 0; i < (pstr_hbe_txposer->hbe_qmf_out_len - pstr_hbe_txposer->no_bins); i++) {
    memcpy(pstr_hbe_txposer->qmf_out_buf[i],
           pstr_hbe_txposer->qmf_out_buf[i + pstr_hbe_txposer->no_bins],
           sizeof(pstr_hbe_txposer->qmf_out_buf[i]));
  }

  for (; i < pstr_hbe_txposer->hbe_qmf_out_len; i++) {
    memset(pstr_hbe_txposer->qmf_out_buf[i], 0, sizeof(pstr_hbe_txposer->qmf_out_buf[i]));
  }

  err_code = ixheaace_hbe_post_anal_process(pstr_hbe_txposer, pitch_in_bins,
                                            pstr_hbe_txposer->upsamp_4_flag);
  if (err_code) {
    return err_code;
  }
  i = 0;
  while (i < pstr_hbe_txposer->no_bins) {
    for (qmf_band_idx = pstr_hbe_txposer->start_band; qmf_band_idx < pstr_hbe_txposer->end_band;
         qmf_band_idx++) {
      pv_qmf_buf_real[i][qmf_band_idx] =
          (FLOAT32)(pstr_hbe_txposer->qmf_out_buf[i][2 * qmf_band_idx] *
                        ixheaac_phase_vocoder_cos_table[qmf_band_idx] -
                    pstr_hbe_txposer->qmf_out_buf[i][2 * qmf_band_idx + 1] *
                        ixheaac_phase_vocoder_sin_table[qmf_band_idx]);

      pv_qmf_buf_imag[i][qmf_band_idx] =
          (FLOAT32)(pstr_hbe_txposer->qmf_out_buf[i][2 * qmf_band_idx] *
                        ixheaac_phase_vocoder_sin_table[qmf_band_idx] +
                    pstr_hbe_txposer->qmf_out_buf[i][2 * qmf_band_idx + 1] *
                        ixheaac_phase_vocoder_cos_table[qmf_band_idx]);
    }
    i++;
  }
  return err_code;
}

VOID ixheaace_norm_qmf_in_buf_4(ixheaace_str_esbr_hbe_txposer *pstr_hbe_txposer,
                                WORD32 qmf_band_idx) {
  WORD32 i;
  FLOAT32 *ptr_in_buf = &pstr_hbe_txposer->qmf_in_buf[0][2 * qmf_band_idx];
  FLOAT32 *ptr_norm_buf = &pstr_hbe_txposer->norm_qmf_in_buf[0][2 * qmf_band_idx];

  for (; qmf_band_idx <= pstr_hbe_txposer->x_over_qmf[3]; qmf_band_idx++) {
    for (i = 0; i < pstr_hbe_txposer->hbe_qmf_in_len; i++) {
      FLOAT32 mag_scaling_fac = 0.0f;
      FLOAT32 x_r, x_i, temp;
      FLOAT64 base = 1e-17;
      x_r = ptr_in_buf[0];
      x_i = ptr_in_buf[1];

      temp = x_r * x_r;
      base = base + temp;
      temp = x_i * x_i;
      base = base + temp;

      temp = (FLOAT32)sqrt(sqrt(base));
      mag_scaling_fac = temp * (FLOAT32)(sqrt(temp));

      mag_scaling_fac = 1 / mag_scaling_fac;

      x_r *= mag_scaling_fac;
      x_i *= mag_scaling_fac;

      ptr_norm_buf[0] = x_r;
      ptr_norm_buf[1] = x_i;

      ptr_in_buf += 128;
      ptr_norm_buf += 128;
    }

    ptr_in_buf -= (128 * (pstr_hbe_txposer->hbe_qmf_in_len) - 2);
    ptr_norm_buf -= (128 * (pstr_hbe_txposer->hbe_qmf_in_len) - 2);
  }
}

VOID ixheaace_norm_qmf_in_buf_2(ixheaace_str_esbr_hbe_txposer *pstr_hbe_txposer,
                                WORD32 qmf_band_idx) {
  WORD32 i;
  FLOAT32 *ptr_in_buf = &pstr_hbe_txposer->qmf_in_buf[0][2 * qmf_band_idx];
  FLOAT32 *ptr_norm_buf = &pstr_hbe_txposer->norm_qmf_in_buf[0][2 * qmf_band_idx];

  for (; qmf_band_idx <= pstr_hbe_txposer->x_over_qmf[1]; qmf_band_idx++) {
    for (i = 0; i < pstr_hbe_txposer->hbe_qmf_in_len; i++) {
      FLOAT32 mag_scaling_fac = 0.0f;
      FLOAT32 x_r, x_i, temp;
      FLOAT64 base = 1e-17;
      x_r = ptr_in_buf[0];
      x_i = ptr_in_buf[1];

      temp = x_r * x_r;
      base = base + temp;
      base = base + x_i * x_i;

      mag_scaling_fac = (FLOAT32)(1.0f / base);
      mag_scaling_fac = (FLOAT32)sqrt(sqrt(mag_scaling_fac));

      x_r *= mag_scaling_fac;
      x_i *= mag_scaling_fac;

      ptr_norm_buf[0] = x_r;
      ptr_norm_buf[1] = x_i;

      ptr_in_buf += 128;
      ptr_norm_buf += 128;
    }

    ptr_in_buf -= (128 * (pstr_hbe_txposer->hbe_qmf_in_len) - 2);
    ptr_norm_buf -= (128 * (pstr_hbe_txposer->hbe_qmf_in_len) - 2);
  }
}

VOID ixheaace_hbe_xprod_proc_3(ixheaace_str_esbr_hbe_txposer *pstr_hbe_txposer,
                               WORD32 qmf_band_idx, WORD32 qmf_col_idx, FLOAT32 p,
                               WORD32 pitch_in_bins_idx) {
  WORD32 tr, n1, n2, max_trans_fac, max_n1, max_n2;
  WORD32 k, addrshift;
  WORD32 inp_band_idx = 2 * qmf_band_idx / 3;

  FLOAT64 temp_fac;
  FLOAT32 max_mag_value;
  FLOAT32 mag_zero_band, mag_n1_band, mag_n2_band, temp;
  FLOAT32 temp_r, temp_i;
  FLOAT32 mag_cmplx_gain = 1.8856f;

  FLOAT32 *ptr_qmf_in_buf_ri =
      pstr_hbe_txposer->qmf_in_buf[qmf_col_idx + IXHEAACE_HBE_ZERO_BAND_IDX];

  mag_zero_band =
      ptr_qmf_in_buf_ri[2 * inp_band_idx] * ptr_qmf_in_buf_ri[2 * inp_band_idx] +
      ptr_qmf_in_buf_ri[2 * inp_band_idx + 1] * ptr_qmf_in_buf_ri[2 * inp_band_idx + 1];
  max_mag_value = 0;
  max_n1 = max_n2 = max_trans_fac = 0;

  for (tr = 1; tr < 3; tr++) {
    temp_fac = (2.0f * qmf_band_idx + 1 - tr * p) * 0.3333334;

    n1 = (WORD32)(temp_fac);
    n2 = (WORD32)(temp_fac + p);

    mag_n1_band = ptr_qmf_in_buf_ri[2 * n1] * ptr_qmf_in_buf_ri[2 * n1] +
                  ptr_qmf_in_buf_ri[2 * n1 + 1] * ptr_qmf_in_buf_ri[2 * n1 + 1];
    mag_n2_band = ptr_qmf_in_buf_ri[2 * n2] * ptr_qmf_in_buf_ri[2 * n2] +
                  ptr_qmf_in_buf_ri[2 * n2 + 1] * ptr_qmf_in_buf_ri[2 * n2 + 1];
    temp = min(mag_n1_band, mag_n2_band);

    if (temp > max_mag_value) {
      max_mag_value = temp;
      max_trans_fac = tr;
      max_n1 = n1;
      max_n2 = n2;
    }
  }

  if ((max_mag_value > mag_zero_band && max_n1 >= 0) &&
      (max_n2 < IXHEAACE_NUM_QMF_SYNTH_CHANNELS)) {
    FLOAT32 vec_y_r[2], vec_y_i[2], vec_o_r[2], vec_o_i[2];
    FLOAT32 coeff_real[2], coeff_imag[2];
    FLOAT32 d1, d2;
    WORD32 mid_trans_fac, idx;
    FLOAT64 base = 1e-17;
    FLOAT32 mag_scaling_fac = 0;
    FLOAT32 x_zero_band_r;
    FLOAT32 x_zero_band_i;

    mid_trans_fac = 3 - max_trans_fac;
    if (max_trans_fac == 1) {
      d1 = 0;
      d2 = 1.5;
      x_zero_band_r = ptr_qmf_in_buf_ri[2 * max_n1];
      x_zero_band_i = ptr_qmf_in_buf_ri[2 * max_n1 + 1];

      idx = max_n2 & 3;
      idx = (idx + 1) & 3;
      coeff_real[0] = ixheaac_hbe_post_anal_proc_interp_coeff[idx][0];
      coeff_imag[0] = ixheaac_hbe_post_anal_proc_interp_coeff[idx][1];

      coeff_real[1] = coeff_real[0];
      coeff_imag[1] = -coeff_imag[0];

      vec_y_r[1] = ptr_qmf_in_buf_ri[2 * max_n2];
      vec_y_i[1] = ptr_qmf_in_buf_ri[2 * max_n2 + 1];

      addrshift = -2;
      temp_r = pstr_hbe_txposer
                   ->qmf_in_buf[qmf_col_idx + addrshift + IXHEAACE_HBE_ZERO_BAND_IDX][2 * max_n2];
      temp_i =
          pstr_hbe_txposer
              ->qmf_in_buf[qmf_col_idx + addrshift + IXHEAACE_HBE_ZERO_BAND_IDX][2 * max_n2 + 1];

      vec_y_r[0] = coeff_real[1] * temp_r - coeff_imag[1] * temp_i;
      vec_y_i[0] = coeff_imag[1] * temp_r + coeff_real[1] * temp_i;

      temp_r =
          pstr_hbe_txposer
              ->qmf_in_buf[qmf_col_idx + addrshift + 1 + IXHEAACE_HBE_ZERO_BAND_IDX][2 * max_n2];
      temp_i = pstr_hbe_txposer->qmf_in_buf[qmf_col_idx + addrshift + 1 +
                                            IXHEAACE_HBE_ZERO_BAND_IDX][2 * max_n2 + 1];

      vec_y_r[0] += coeff_real[0] * temp_r - coeff_imag[0] * temp_i;
      vec_y_i[0] += coeff_imag[0] * temp_r + coeff_real[0] * temp_i;
    } else {
      d1 = 1.5;
      d2 = 0;
      mid_trans_fac = max_trans_fac;
      max_trans_fac = 3 - max_trans_fac;

      x_zero_band_r = ptr_qmf_in_buf_ri[2 * max_n2];
      x_zero_band_i = ptr_qmf_in_buf_ri[2 * max_n2 + 1];

      idx = (max_n1 & 3);
      idx = (idx + 1) & 3;
      coeff_real[0] = ixheaac_hbe_post_anal_proc_interp_coeff[idx][0];
      coeff_imag[0] = ixheaac_hbe_post_anal_proc_interp_coeff[idx][1];

      coeff_real[1] = coeff_real[0];
      coeff_imag[1] = -coeff_imag[0];

      vec_y_r[1] = ptr_qmf_in_buf_ri[2 * max_n1];
      vec_y_i[1] = ptr_qmf_in_buf_ri[2 * max_n1 + 1];

      addrshift = -2;

      temp_r = pstr_hbe_txposer
                   ->qmf_in_buf[qmf_col_idx + addrshift + IXHEAACE_HBE_ZERO_BAND_IDX][2 * max_n1];
      temp_i =
          pstr_hbe_txposer
              ->qmf_in_buf[qmf_col_idx + addrshift + IXHEAACE_HBE_ZERO_BAND_IDX][2 * max_n1 + 1];

      vec_y_r[0] = coeff_real[1] * temp_r - coeff_imag[1] * temp_i;
      vec_y_i[0] = coeff_imag[1] * temp_r + coeff_real[1] * temp_i;

      temp_r =
          pstr_hbe_txposer
              ->qmf_in_buf[qmf_col_idx + addrshift + 1 + IXHEAACE_HBE_ZERO_BAND_IDX][2 * max_n1];
      temp_i = pstr_hbe_txposer->qmf_in_buf[qmf_col_idx + addrshift + 1 +
                                            IXHEAACE_HBE_ZERO_BAND_IDX][2 * max_n1 + 1];

      vec_y_r[0] += coeff_real[0] * temp_r - coeff_imag[0] * temp_i;
      vec_y_i[0] += coeff_imag[0] * temp_r + coeff_real[0] * temp_i;
    }

    base = 1e-17;
    base = base + x_zero_band_r * x_zero_band_r;
    base = base + x_zero_band_i * x_zero_band_i;
    mag_scaling_fac = (FLOAT32)(ixheaace_cbrt_calc((FLOAT32)base));
    x_zero_band_r *= mag_scaling_fac;
    x_zero_band_i *= mag_scaling_fac;
    for (k = 0; k < 2; k++) {
      base = 1e-17;
      base = base + vec_y_r[k] * vec_y_r[k];
      base = base + vec_y_i[k] * vec_y_i[k];
      mag_scaling_fac = (FLOAT32)(ixheaace_cbrt_calc((FLOAT32)base));
      vec_y_r[k] *= mag_scaling_fac;
      vec_y_i[k] *= mag_scaling_fac;
    }

    temp_r = x_zero_band_r;
    temp_i = x_zero_band_i;
    for (idx = 0; idx < mid_trans_fac - 1; idx++) {
      FLOAT32 tmp = x_zero_band_r;
      x_zero_band_r = x_zero_band_r * temp_r - x_zero_band_i * temp_i;
      x_zero_band_i = tmp * temp_i + x_zero_band_i * temp_r;
    }

    for (k = 0; k < 2; k++) {
      temp_r = vec_y_r[k];
      temp_i = vec_y_i[k];
      for (idx = 0; idx < max_trans_fac - 1; idx++) {
        FLOAT32 tmp = vec_y_r[k];
        vec_y_r[k] = vec_y_r[k] * temp_r - vec_y_i[k] * temp_i;
        vec_y_i[k] = tmp * temp_i + vec_y_i[k] * temp_r;
      }
    }

    for (k = 0; k < 2; k++) {
      vec_o_r[k] = vec_y_r[k] * x_zero_band_r - vec_y_i[k] * x_zero_band_i;
      vec_o_i[k] = vec_y_r[k] * x_zero_band_i + vec_y_i[k] * x_zero_band_r;
    }

    {
      FLOAT32 cos_theta = ixheaac_hbe_x_prod_cos_table_trans_3[(pitch_in_bins_idx << 1) + 0];
      FLOAT32 sin_theta = ixheaac_hbe_x_prod_cos_table_trans_3[(pitch_in_bins_idx << 1) + 1];
      if (d2 < d1) {
        sin_theta = -sin_theta;
      }
      temp_r = vec_o_r[0];
      temp_i = vec_o_i[0];
      vec_o_r[0] = (FLOAT32)(cos_theta * temp_r - sin_theta * temp_i);
      vec_o_i[0] = (FLOAT32)(cos_theta * temp_i + sin_theta * temp_r);
    }

    for (k = 0; k < 2; k++) {
      pstr_hbe_txposer->qmf_out_buf[qmf_col_idx * 2 + (k + IXHEAACE_HBE_ZERO_BAND_IDX - 1)]
                                   [2 * qmf_band_idx] += (FLOAT32)(mag_cmplx_gain * vec_o_r[k]);
      pstr_hbe_txposer->qmf_out_buf[qmf_col_idx * 2 + (k + IXHEAACE_HBE_ZERO_BAND_IDX - 1)]
                                   [2 * qmf_band_idx + 1] +=
          (FLOAT32)(mag_cmplx_gain * vec_o_i[k]);
    }
  }
}

VOID ixheaace_hbe_xprod_proc_4(ixheaace_str_esbr_hbe_txposer *pstr_hbe_txposer,
                               WORD32 qmf_band_idx, WORD32 qmf_col_idx, FLOAT32 p,
                               WORD32 pitch_in_bins_idx) {
  WORD32 k;
  WORD32 inp_band_idx = qmf_band_idx >> 1;
  WORD32 tr, n1, n2, max_trans_fac, max_n1, max_n2;

  FLOAT64 temp_fac;
  FLOAT32 max_mag_value, mag_zero_band, mag_n1_band, mag_n2_band, temp;
  FLOAT32 temp_r, temp_i;
  FLOAT32 mag_cmplx_gain = 2.0f;

  FLOAT32 *ptr_qmf_in_buf_ri =
      pstr_hbe_txposer->qmf_in_buf[qmf_col_idx + IXHEAACE_HBE_ZERO_BAND_IDX];

  mag_zero_band =
      ptr_qmf_in_buf_ri[2 * inp_band_idx] * ptr_qmf_in_buf_ri[2 * inp_band_idx] +
      ptr_qmf_in_buf_ri[2 * inp_band_idx + 1] * ptr_qmf_in_buf_ri[2 * inp_band_idx + 1];

  max_mag_value = 0;
  max_n1 = max_n2 = max_trans_fac = 0;

  tr = 1;
  while (tr < 4) {
    temp_fac = (2.0 * qmf_band_idx + 1 - tr * p) * 0.25;
    n1 = ((WORD32)(temp_fac)) << 1;
    n2 = ((WORD32)(temp_fac + p)) << 1;

    mag_n1_band = ptr_qmf_in_buf_ri[n1] * ptr_qmf_in_buf_ri[n1] +
                  ptr_qmf_in_buf_ri[n1 + 1] * ptr_qmf_in_buf_ri[n1 + 1];
    mag_n2_band = ptr_qmf_in_buf_ri[n2] * ptr_qmf_in_buf_ri[n2] +
                  ptr_qmf_in_buf_ri[n2 + 1] * ptr_qmf_in_buf_ri[n2 + 1];

    temp = min(mag_n1_band, mag_n2_band);

    if (temp > max_mag_value) {
      max_mag_value = temp;
      max_trans_fac = tr;
      max_n1 = n1;
      max_n2 = n2;
    }
    tr++;
  }
  if (max_mag_value > mag_zero_band && max_n1 >= 0 && max_n2 < IXHEAACE_TWICE_QMF_SYNTH_CH_NUM) {
    FLOAT32 vec_y_r[2], vec_y_i[2], vec_o_r[2], vec_o_i[2];
    FLOAT32 d1, d2;
    WORD32 mid_trans_fac, idx;
    FLOAT32 x_zero_band_r;
    FLOAT32 x_zero_band_i;
    FLOAT64 base = 1e-17;
    FLOAT32 mag_scaling_fac = 0.0f;

    mid_trans_fac = 4 - max_trans_fac;

    if (max_trans_fac == 1) {
      d1 = 0;
      d2 = 2;
      x_zero_band_r = ptr_qmf_in_buf_ri[max_n1];
      x_zero_band_i = ptr_qmf_in_buf_ri[max_n1 + 1];
      for (k = 0; k < 2; k++) {
        vec_y_r[k] =
            pstr_hbe_txposer
                ->qmf_in_buf[qmf_col_idx + IXHEAACE_HBE_ZERO_BAND_IDX + 2 * (k - 1)][max_n2];
        vec_y_i[k] =
            pstr_hbe_txposer
                ->qmf_in_buf[qmf_col_idx + IXHEAACE_HBE_ZERO_BAND_IDX + 2 * (k - 1)][max_n2 + 1];
      }
    } else if (max_trans_fac == 2) {
      d1 = 0;
      d2 = 1;
      x_zero_band_r = ptr_qmf_in_buf_ri[max_n1];
      x_zero_band_i = ptr_qmf_in_buf_ri[max_n1 + 1];
      for (k = 0; k < 2; k++) {
        vec_y_r[k] = pstr_hbe_txposer
                         ->qmf_in_buf[qmf_col_idx + IXHEAACE_HBE_ZERO_BAND_IDX + (k - 1)][max_n2];
        vec_y_i[k] =
            pstr_hbe_txposer
                ->qmf_in_buf[qmf_col_idx + IXHEAACE_HBE_ZERO_BAND_IDX + (k - 1)][max_n2 + 1];
      }
    } else {
      d1 = 2;
      d2 = 0;
      mid_trans_fac = max_trans_fac;
      max_trans_fac = 4 - max_trans_fac;
      x_zero_band_r = ptr_qmf_in_buf_ri[max_n2];
      x_zero_band_i = ptr_qmf_in_buf_ri[max_n2 + 1];
      for (k = 0; k < 2; k++) {
        vec_y_r[k] =
            pstr_hbe_txposer
                ->qmf_in_buf[qmf_col_idx + IXHEAACE_HBE_ZERO_BAND_IDX + 2 * (k - 1)][max_n1];
        vec_y_i[k] =
            pstr_hbe_txposer
                ->qmf_in_buf[qmf_col_idx + IXHEAACE_HBE_ZERO_BAND_IDX + 2 * (k - 1)][max_n1 + 1];
      }
    }

    base = 1e-17;
    base = base + x_zero_band_r * x_zero_band_r;
    base = base + x_zero_band_i * x_zero_band_i;
    {
      temp = (FLOAT32)sqrt(sqrt(base));
      mag_scaling_fac = temp * (FLOAT32)(sqrt(temp));
      mag_scaling_fac = 1 / mag_scaling_fac;
    }

    x_zero_band_r *= mag_scaling_fac;
    x_zero_band_i *= mag_scaling_fac;
    for (k = 0; k < 2; k++) {
      base = 1e-17;
      base = base + vec_y_r[k] * vec_y_r[k];
      base = base + vec_y_i[k] * vec_y_i[k];
      {
        temp = (FLOAT32)sqrt(sqrt(base));
        mag_scaling_fac = temp * (FLOAT32)(sqrt(temp));

        mag_scaling_fac = 1 / mag_scaling_fac;
      }
      vec_y_r[k] *= mag_scaling_fac;
      vec_y_i[k] *= mag_scaling_fac;
    }

    temp_r = x_zero_band_r;
    temp_i = x_zero_band_i;
    for (idx = 0; idx < mid_trans_fac - 1; idx++) {
      FLOAT32 tmp = x_zero_band_r;
      x_zero_band_r = x_zero_band_r * temp_r - x_zero_band_i * temp_i;
      x_zero_band_i = tmp * temp_i + x_zero_band_i * temp_r;
    }

    for (k = 0; k < 2; k++) {
      temp_r = vec_y_r[k];
      temp_i = vec_y_i[k];
      for (idx = 0; idx < max_trans_fac - 1; idx++) {
        FLOAT32 tmp = vec_y_r[k];
        vec_y_r[k] = vec_y_r[k] * temp_r - vec_y_i[k] * temp_i;
        vec_y_i[k] = tmp * temp_i + vec_y_i[k] * temp_r;
      }
    }

    for (k = 0; k < 2; k++) {
      vec_o_r[k] = vec_y_r[k] * x_zero_band_r - vec_y_i[k] * x_zero_band_i;
      vec_o_i[k] = vec_y_r[k] * x_zero_band_i + vec_y_i[k] * x_zero_band_r;
    }

    {
      FLOAT32 cos_theta;
      FLOAT32 sin_theta;

      if (d2 == 1) {
        cos_theta = ixheaac_hbe_x_prod_cos_table_trans_4_1[(pitch_in_bins_idx << 1) + 0];
        sin_theta = ixheaac_hbe_x_prod_cos_table_trans_4_1[(pitch_in_bins_idx << 1) + 1];
      } else {
        cos_theta = ixheaac_hbe_x_prod_cos_table_trans_4[(pitch_in_bins_idx << 1) + 0];
        sin_theta = ixheaac_hbe_x_prod_cos_table_trans_4[(pitch_in_bins_idx << 1) + 1];
        if (d2 < d1) {
          sin_theta = -sin_theta;
        }
      }
      temp_r = vec_o_r[0];
      temp_i = vec_o_i[0];
      vec_o_r[0] = (FLOAT32)(cos_theta * temp_r - sin_theta * temp_i);
      vec_o_i[0] = (FLOAT32)(cos_theta * temp_i + sin_theta * temp_r);
    }

    for (k = 0; k < 2; k++) {
      pstr_hbe_txposer->qmf_out_buf[qmf_col_idx * 2 + (k + IXHEAACE_HBE_ZERO_BAND_IDX - 1)]
                                   [2 * qmf_band_idx] += (FLOAT32)(mag_cmplx_gain * vec_o_r[k]);
      pstr_hbe_txposer->qmf_out_buf[qmf_col_idx * 2 + (k + IXHEAACE_HBE_ZERO_BAND_IDX - 1)]
                                   [2 * qmf_band_idx + 1] +=
          (FLOAT32)(mag_cmplx_gain * vec_o_i[k]);
    }
  }
}

VOID ixheaace_hbe_post_anal_prod2(ixheaace_str_esbr_hbe_txposer *pstr_hbe_txposer,
                                  WORD32 qmf_voc_columns, WORD32 qmf_band_idx) {
  WORD32 i;
  FLOAT32 *ptr_norm = &pstr_hbe_txposer->norm_qmf_in_buf[1][2 * qmf_band_idx];
  FLOAT32 *ptr_out = &pstr_hbe_txposer->qmf_out_buf[1][2 * qmf_band_idx];
  FLOAT32 *ptr_x_norm =
      &pstr_hbe_txposer->norm_qmf_in_buf[IXHEAACE_HBE_ZERO_BAND_IDX][2 * qmf_band_idx];

  ixheaace_norm_qmf_in_buf_2(pstr_hbe_txposer, qmf_band_idx);

  for (; qmf_band_idx < pstr_hbe_txposer->x_over_qmf[1]; qmf_band_idx++) {
    for (i = 0; i < qmf_voc_columns; i++) {
      WORD32 k;
      FLOAT32 x_zero_band_r, x_zero_band_i;

      x_zero_band_r = *ptr_x_norm++;
      x_zero_band_i = *ptr_x_norm++;

      for (k = 0; k < IXHEAACE_HBE_OPER_BLK_LEN_2; k++) {
        register FLOAT32 tmp_r, tmp_i;
        tmp_r = *ptr_norm++;
        tmp_i = *ptr_norm++;

        *ptr_out++ += ((tmp_r * x_zero_band_r - tmp_i * x_zero_band_i) * 0.3333333f);

        *ptr_out++ += ((tmp_r * x_zero_band_i + tmp_i * x_zero_band_r) * 0.3333333f);

        ptr_norm += 126;
        ptr_out += 126;
      }

      ptr_norm -= 128 * 9;
      ptr_out -= 128 * 8;
      ptr_x_norm += 126;
    }
    ptr_out -= (128 * 2 * qmf_voc_columns) - 2;
    ptr_norm -= (128 * qmf_voc_columns) - 2;
    ptr_x_norm -= (128 * qmf_voc_columns) - 2;
  }
}

VOID ixheaace_hbe_post_anal_prod3(ixheaace_str_esbr_hbe_txposer *pstr_hbe_txposer,
                                  WORD32 qmf_voc_columns, WORD32 qmf_band_idx) {
  WORD32 i, inp_band_idx, rem;

  FLOAT32 *ptr_out_buf = &pstr_hbe_txposer->qmf_out_buf[2][2 * qmf_band_idx];

  for (; qmf_band_idx < pstr_hbe_txposer->x_over_qmf[2]; qmf_band_idx++) {
    FLOAT32 temp_r, temp_i;
    FLOAT32 temp_r1, temp_i1;
    const FLOAT32 *ptr_sel, *ptr_sel1;

    inp_band_idx = (2 * qmf_band_idx) / 3;
    ptr_sel = &ixheaac_sel_case[(inp_band_idx + 1) & 3][0];
    ptr_sel1 = &ixheaac_sel_case[((inp_band_idx + 1) & 3) + 1][0];
    rem = 2 * qmf_band_idx - 3 * inp_band_idx;

    if (rem == 0 || rem == 1) {
      FLOAT32 *ptr_in_buf = &pstr_hbe_txposer->qmf_in_buf[0][2 * inp_band_idx];

      for (i = 0; i < qmf_voc_columns; i += 1) {
        WORD32 k;
        FLOAT32 vec_x[2 * IXHEAACE_HBE_OPER_WIN_LEN];
        FLOAT32 *ptr_vec_x = &vec_x[0];
        FLOAT32 x_zero_band_r, x_zero_band_i;

        FLOAT32 mag_scaling_fac;

        for (k = 0; k < (IXHEAACE_HBE_OPER_BLK_LEN_3); k += 2) {
          FLOAT64 base1;
          FLOAT64 base = 1e-17;

          temp_r = ptr_in_buf[0];
          temp_i = ptr_in_buf[1];

          ptr_in_buf += 256;

          base1 = base + temp_r * temp_r;
          base1 = base1 + temp_i * temp_i;

          mag_scaling_fac = (FLOAT32)(ixheaace_cbrt_calc((FLOAT32)base1));

          ptr_vec_x[0] = temp_r * mag_scaling_fac;
          ptr_vec_x[1] = temp_i * mag_scaling_fac;

          temp_r = ptr_in_buf[0];
          temp_i = ptr_in_buf[1];

          ptr_in_buf -= 128;

          temp_r1 = ptr_sel[0] * temp_r + ptr_sel[1] * temp_i;
          temp_i1 = ptr_sel[2] * temp_r + ptr_sel[3] * temp_i;

          temp_r = ptr_in_buf[0];
          temp_i = ptr_in_buf[1];

          temp_r1 += ptr_sel[4] * temp_r + ptr_sel[5] * temp_i;
          temp_i1 += ptr_sel[6] * temp_r + ptr_sel[7] * temp_i;

          temp_r1 *= 0.3984033437f;
          temp_i1 *= 0.3984033437f;

          base1 = base + temp_r1 * temp_r1;
          base1 = base1 + temp_i1 * temp_i1;
          mag_scaling_fac = (FLOAT32)(ixheaace_cbrt_calc((FLOAT32)base1));

          ptr_vec_x[2] = temp_r1 * mag_scaling_fac;
          ptr_vec_x[3] = temp_i1 * mag_scaling_fac;

          ptr_vec_x += 4;
          ptr_in_buf += 256;
        }
        ptr_vec_x = &vec_x[0];
        temp_r = vec_x[2 * (IXHEAACE_HBE_ZERO_BAND_IDX - 2)];
        temp_i = vec_x[(2 * (IXHEAACE_HBE_ZERO_BAND_IDX - 2)) + 1];

        x_zero_band_r = temp_r * temp_r - temp_i * temp_i;
        x_zero_band_i = temp_r * temp_i + temp_i * temp_r;

        for (k = 0; k < (IXHEAACE_HBE_OPER_BLK_LEN_3); k++) {
          temp_r = ptr_vec_x[0] * x_zero_band_r - ptr_vec_x[1] * x_zero_band_i;
          temp_i = ptr_vec_x[0] * x_zero_band_i + ptr_vec_x[1] * x_zero_band_r;

          ptr_out_buf[0] += (temp_r * 0.4714045f);
          ptr_out_buf[1] += (temp_i * 0.4714045f);

          ptr_vec_x += 2;
          ptr_out_buf += 128;
        }

        ptr_in_buf -= 128 * 11;
        ptr_out_buf -= 128 * 6;
      }
    } else {
      FLOAT32 *ptr_in_buf = &pstr_hbe_txposer->qmf_in_buf[0][2 * inp_band_idx];
      FLOAT32 *ptr_in_buf1 = &pstr_hbe_txposer->qmf_in_buf[0][2 * (inp_band_idx + 1)];

      for (i = 0; i < qmf_voc_columns; i++) {
        WORD32 k;
        FLOAT32 vec_x[2 * IXHEAACE_HBE_OPER_WIN_LEN];
        FLOAT32 vec_x_cap[2 * IXHEAACE_HBE_OPER_WIN_LEN];

        FLOAT32 x_zero_band_r, x_zero_band_i;
        FLOAT32 *ptr_vec_x = &vec_x[0];
        FLOAT32 *ptr_vec_x_cap = &vec_x_cap[0];

        FLOAT32 mag_scaling_fac;

        for (k = 0; k < (IXHEAACE_HBE_OPER_BLK_LEN_3); k += 2) {
          FLOAT32 tmp_vr, tmp_vi;
          FLOAT32 tmp_cr, tmp_ci;
          FLOAT64 base1;
          FLOAT64 base = 1e-17;

          temp_r1 = ptr_in_buf[0];
          temp_i1 = ptr_in_buf[1];
          temp_r = ptr_in_buf1[0];
          temp_i = ptr_in_buf1[1];

          base1 = base + temp_r * temp_r;
          base1 = base1 + temp_i * temp_i;

          mag_scaling_fac = (FLOAT32)(ixheaace_cbrt_calc((FLOAT32)base1));

          ptr_vec_x[0] = temp_r * mag_scaling_fac;
          ptr_vec_x[1] = temp_i * mag_scaling_fac;

          base1 = base + temp_r1 * temp_r1;
          base1 = base1 + temp_i1 * temp_i1;

          mag_scaling_fac = (FLOAT32)(ixheaace_cbrt_calc((FLOAT32)base1));

          ptr_vec_x_cap[0] = temp_r1 * mag_scaling_fac;
          ptr_vec_x_cap[1] = temp_i1 * mag_scaling_fac;

          ptr_in_buf += 256;

          temp_r = ptr_in_buf[0];
          temp_i = ptr_in_buf[1];

          temp_r1 = ptr_sel[0] * temp_r + ptr_sel[1] * temp_i;
          temp_i1 = ptr_sel[2] * temp_r + ptr_sel[3] * temp_i;

          ptr_in_buf -= 128;

          temp_r = ptr_in_buf[0];
          temp_i = ptr_in_buf[1];

          tmp_cr = temp_r1 + ptr_sel[4] * temp_r + ptr_sel[5] * temp_i;
          tmp_ci = temp_i1 + ptr_sel[6] * temp_r + ptr_sel[7] * temp_i;

          ptr_in_buf1 += 256;

          temp_r = ptr_in_buf1[0];
          temp_i = ptr_in_buf1[1];

          temp_r1 = ptr_sel1[0] * temp_r + ptr_sel1[1] * temp_i;
          temp_i1 = ptr_sel1[2] * temp_r + ptr_sel1[3] * temp_i;

          ptr_in_buf1 -= 128;

          temp_r = ptr_in_buf1[0];
          temp_i = ptr_in_buf1[1];

          tmp_vr = temp_r1 + ptr_sel1[4] * temp_r + ptr_sel1[5] * temp_i;
          tmp_vi = temp_i1 + ptr_sel1[6] * temp_r + ptr_sel1[7] * temp_i;

          tmp_cr *= 0.3984033437f;
          tmp_ci *= 0.3984033437f;

          tmp_vr *= 0.3984033437f;
          tmp_vi *= 0.3984033437f;

          base1 = base + tmp_vr * tmp_vr;
          base1 = base1 + tmp_vi * tmp_vi;

          mag_scaling_fac = (FLOAT32)(ixheaace_cbrt_calc((FLOAT32)base1));

          ptr_vec_x[2] = tmp_vr * mag_scaling_fac;
          ptr_vec_x[3] = tmp_vi * mag_scaling_fac;

          base1 = base + tmp_cr * tmp_cr;
          base1 = base1 + tmp_ci * tmp_ci;

          mag_scaling_fac = (FLOAT32)(ixheaace_cbrt_calc((FLOAT32)base1));

          ptr_vec_x_cap[2] = tmp_cr * mag_scaling_fac;
          ptr_vec_x_cap[3] = tmp_ci * mag_scaling_fac;

          ptr_in_buf += 256;
          ptr_in_buf1 += 256;
          ptr_vec_x += 4;
          ptr_vec_x_cap += 4;
        }
        ptr_vec_x = &vec_x[0];
        ptr_vec_x_cap = &vec_x_cap[0];

        temp_r = vec_x_cap[2 * (IXHEAACE_HBE_ZERO_BAND_IDX - 2)];
        temp_i = vec_x_cap[2 * (IXHEAACE_HBE_ZERO_BAND_IDX - 2) + 1];
        temp_r1 = vec_x[2 * (IXHEAACE_HBE_ZERO_BAND_IDX - 2)];
        temp_i1 = vec_x[2 * (IXHEAACE_HBE_ZERO_BAND_IDX - 2) + 1];

        x_zero_band_r = temp_r * temp_r - temp_i * temp_i;
        x_zero_band_i = temp_r * temp_i + temp_i * temp_r;

        temp_r = temp_r1 * temp_r1 - temp_i1 * temp_i1;
        temp_i = temp_r1 * temp_i1 + temp_i1 * temp_r1;

        k = 0;
        while (k < (IXHEAACE_HBE_OPER_BLK_LEN_3)) {
          temp_r1 = ptr_vec_x[0] * x_zero_band_r - ptr_vec_x[1] * x_zero_band_i;
          temp_i1 = ptr_vec_x[0] * x_zero_band_i + ptr_vec_x[1] * x_zero_band_r;

          temp_r1 += ptr_vec_x_cap[0] * temp_r - ptr_vec_x_cap[1] * temp_i;
          temp_i1 += ptr_vec_x_cap[0] * temp_i + ptr_vec_x_cap[1] * temp_r;

          ptr_out_buf[0] += (temp_r1 * 0.23570225f);
          ptr_out_buf[1] += (temp_i1 * 0.23570225f);

          ptr_out_buf += 128;
          ptr_vec_x += 2;
          ptr_vec_x_cap += 2;

          k++;
        }

        ptr_in_buf -= 128 * 11;
        ptr_in_buf1 -= 128 * 11;
        ptr_out_buf -= 128 * 6;
      }
    }

    ptr_out_buf -= (256 * qmf_voc_columns) - 2;
  }
}

VOID ixheaace_hbe_post_anal_prod4(ixheaace_str_esbr_hbe_txposer *pstr_hbe_txposer,
                                  WORD32 qmf_voc_columns, WORD32 qmf_band_idx) {
  WORD32 i, inp_band_idx;
  FLOAT32 *ptr_out = &pstr_hbe_txposer->qmf_out_buf[3][2 * qmf_band_idx];

  ixheaace_norm_qmf_in_buf_4(pstr_hbe_txposer, ((qmf_band_idx >> 1) - 1));

  for (; qmf_band_idx < pstr_hbe_txposer->x_over_qmf[3]; qmf_band_idx++) {
    WORD32 ip_idx;
    FLOAT32 temp, temp_r, temp_i;
    FLOAT32 *ptr_norm, *ptr_x_norm;
    inp_band_idx = qmf_band_idx >> 1;
    ip_idx = (qmf_band_idx & 1) ? (inp_band_idx + 1) : (inp_band_idx - 1);

    ptr_norm = &pstr_hbe_txposer->norm_qmf_in_buf[0][2 * ip_idx];
    ptr_x_norm = &pstr_hbe_txposer->norm_qmf_in_buf[IXHEAACE_HBE_ZERO_BAND_IDX][2 * inp_band_idx];

    i = 0;
    while (i < qmf_voc_columns) {
      WORD32 k;
      FLOAT32 x_zero_band_r, x_zero_band_i;

      temp_r = x_zero_band_r = *ptr_x_norm++;
      temp_i = x_zero_band_i = *ptr_x_norm++;

      temp = x_zero_band_r * x_zero_band_r - x_zero_band_i * x_zero_band_i;
      x_zero_band_i = x_zero_band_r * x_zero_band_i + x_zero_band_i * x_zero_band_r;

      x_zero_band_r = temp_r * temp - temp_i * x_zero_band_i;
      x_zero_band_i = temp_r * x_zero_band_i + temp_i * temp;

      for (k = 0; k < IXHEAACE_HBE_OPER_BLK_LEN_4; k++) {
        temp = *ptr_norm++;
        temp_i = *ptr_norm++;

        temp_r = temp * x_zero_band_r - temp_i * x_zero_band_i;
        temp_i = temp * x_zero_band_i + temp_i * x_zero_band_r;

        *ptr_out++ += (temp_r * 0.6666667f);
        *ptr_out++ += (temp_i * 0.6666667f);

        ptr_norm += 254;
        ptr_out += 126;
      }

      ptr_norm -= 128 * 11;
      ptr_out -= 128 * 4;
      ptr_x_norm += 126;
      i++;
    }

    ptr_out -= (128 * 2 * qmf_voc_columns) - 2;
  }
}

VOID ixheaace_hbe_post_anal_xprod2(ixheaace_str_esbr_hbe_txposer *pstr_hbe_txposer,
                                   WORD32 qmf_voc_columns, WORD32 qmf_band_idx, FLOAT32 p,
                                   FLOAT32 *ptr_cos_sin_theta) {
  WORD32 i;
  FLOAT32 *ptr_norm = &pstr_hbe_txposer->norm_qmf_in_buf[1][2 * qmf_band_idx];
  FLOAT32 *ptr_out = &pstr_hbe_txposer->qmf_out_buf[1][2 * qmf_band_idx];
  FLOAT32 *ptr_x_norm =
      &pstr_hbe_txposer->norm_qmf_in_buf[IXHEAACE_HBE_ZERO_BAND_IDX][2 * qmf_band_idx];

  ixheaace_norm_qmf_in_buf_2(pstr_hbe_txposer, qmf_band_idx);

  while (qmf_band_idx < pstr_hbe_txposer->x_over_qmf[1]) {
    WORD32 n1, n2;
    FLOAT64 temp_fac;
    FLOAT32 mag_cmplx_gain = 1.666666667f;
    temp_fac = (2.0 * qmf_band_idx + 1 - p) * 0.5;
    n1 = ((WORD32)(temp_fac)) << 1;
    n2 = ((WORD32)(temp_fac + p)) << 1;

    for (i = 0; i < qmf_voc_columns; i++) {
      WORD32 k;
      FLOAT32 x_zero_band_r, x_zero_band_i;

      x_zero_band_r = *ptr_x_norm++;
      x_zero_band_i = *ptr_x_norm++;

      for (k = 1; k < (IXHEAACE_HBE_OPER_BLK_LEN_2 + 1); k++) {
        register FLOAT32 tmp_r, tmp_i;
        tmp_r = *ptr_norm++;
        tmp_i = *ptr_norm++;

        *ptr_out++ += ((tmp_r * x_zero_band_r - tmp_i * x_zero_band_i) * 0.3333333f);

        *ptr_out++ += ((tmp_r * x_zero_band_i + tmp_i * x_zero_band_r) * 0.3333333f);

        ptr_norm += 126;
        ptr_out += 126;
      }
      ptr_norm -= 128 * 9;
      ptr_out -= 128 * 8;
      ptr_x_norm += 126;

      {
        WORD32 max_trans_fac, max_n1, max_n2;
        FLOAT32 max_mag_value;
        FLOAT32 mag_zero_band, mag_n1_band, mag_n2_band, temp;

        FLOAT32 *ptr_qmf_in_buf_ri = pstr_hbe_txposer->qmf_in_buf[i + IXHEAACE_HBE_ZERO_BAND_IDX];

        mag_zero_band =
            ptr_qmf_in_buf_ri[2 * qmf_band_idx] * ptr_qmf_in_buf_ri[2 * qmf_band_idx] +
            ptr_qmf_in_buf_ri[2 * qmf_band_idx + 1] * ptr_qmf_in_buf_ri[2 * qmf_band_idx + 1];

        mag_n1_band = ptr_qmf_in_buf_ri[n1] * ptr_qmf_in_buf_ri[n1] +
                      ptr_qmf_in_buf_ri[n1 + 1] * ptr_qmf_in_buf_ri[n1 + 1];
        mag_n2_band = ptr_qmf_in_buf_ri[n2] * ptr_qmf_in_buf_ri[n2] +
                      ptr_qmf_in_buf_ri[n2 + 1] * ptr_qmf_in_buf_ri[n2 + 1];

        temp = min(mag_n1_band, mag_n2_band);

        max_mag_value = 0;
        max_trans_fac = 0;
        max_n1 = 0;
        max_n2 = 0;

        if (temp > 0) {
          max_mag_value = temp;
          max_trans_fac = 1;
          max_n1 = n1;
          max_n2 = n2;
        }

        if (max_mag_value > mag_zero_band && max_n1 >= 0 &&
            max_n2 < IXHEAACE_TWICE_QMF_SYNTH_CH_NUM - 1) {
          FLOAT32 vec_y_r[2], vec_y_i[2];
          FLOAT32 temp_r, temp_i, tmp_r1;
          WORD32 mid_trans_fac, idx;
          FLOAT64 base;
          FLOAT32 mag_scaling_fac = 0.0f;

          mid_trans_fac = 2 - max_trans_fac;

          x_zero_band_r = ptr_qmf_in_buf_ri[max_n1];
          x_zero_band_i = ptr_qmf_in_buf_ri[max_n1 + 1];
          base = 1e-17;
          base = base + x_zero_band_r * x_zero_band_r;
          base = base + x_zero_band_i * x_zero_band_i;

          mag_scaling_fac = (FLOAT32)(1.0f / base);
          mag_scaling_fac = (FLOAT32)sqrt(sqrt(mag_scaling_fac));

          x_zero_band_r *= mag_scaling_fac;
          x_zero_band_i *= mag_scaling_fac;

          temp_r = x_zero_band_r;
          temp_i = x_zero_band_i;
          for (idx = 0; idx < mid_trans_fac - 1; idx++) {
            FLOAT32 tmp = x_zero_band_r;
            x_zero_band_r = x_zero_band_r * temp_r - x_zero_band_i * temp_i;
            x_zero_band_i = tmp * temp_i + x_zero_band_i * temp_r;
          }

          for (k = 0; k < 2; k++) {
            temp_r = pstr_hbe_txposer->qmf_in_buf[i + IXHEAACE_HBE_ZERO_BAND_IDX - 1 + k][max_n2];

            temp_i =
                pstr_hbe_txposer->qmf_in_buf[i + IXHEAACE_HBE_ZERO_BAND_IDX - 1 + k][max_n2 + 1];

            base = 1e-17;
            base = base + temp_r * temp_r;
            base = base + temp_i * temp_i;

            mag_scaling_fac = (FLOAT32)(1.0f / base);
            mag_scaling_fac = (FLOAT32)sqrt(sqrt(mag_scaling_fac));

            temp_r *= mag_scaling_fac;
            temp_i *= mag_scaling_fac;

            vec_y_r[k] = temp_r;
            vec_y_i[k] = temp_i;
          }

          temp_r = vec_y_r[0] * x_zero_band_r - vec_y_i[0] * x_zero_band_i;
          temp_i = vec_y_r[0] * x_zero_band_i + vec_y_i[0] * x_zero_band_r;

          tmp_r1 = (FLOAT32)(ptr_cos_sin_theta[0] * temp_r - ptr_cos_sin_theta[1] * temp_i);
          temp_i = (FLOAT32)(ptr_cos_sin_theta[0] * temp_i + ptr_cos_sin_theta[1] * temp_r);

          pstr_hbe_txposer
              ->qmf_out_buf[i * 2 + (IXHEAACE_HBE_ZERO_BAND_IDX - 1)][2 * qmf_band_idx] +=
              (FLOAT32)(mag_cmplx_gain * tmp_r1);

          pstr_hbe_txposer
              ->qmf_out_buf[i * 2 + (IXHEAACE_HBE_ZERO_BAND_IDX - 1)][2 * qmf_band_idx + 1] +=
              (FLOAT32)(mag_cmplx_gain * temp_i);

          temp_r = vec_y_r[1] * x_zero_band_r - vec_y_i[1] * x_zero_band_i;
          temp_i = vec_y_r[1] * x_zero_band_i + vec_y_i[1] * x_zero_band_r;

          pstr_hbe_txposer
              ->qmf_out_buf[i * 2 + (1 + IXHEAACE_HBE_ZERO_BAND_IDX - 1)][2 * qmf_band_idx] +=
              (FLOAT32)(mag_cmplx_gain * temp_r);

          pstr_hbe_txposer
              ->qmf_out_buf[i * 2 + (1 + IXHEAACE_HBE_ZERO_BAND_IDX - 1)][2 * qmf_band_idx + 1] +=
              (FLOAT32)(mag_cmplx_gain * temp_i);
        }
      }
    }

    ptr_out -= (128 * 2 * qmf_voc_columns) - 2;
    ptr_norm -= (128 * qmf_voc_columns) - 2;
    ptr_x_norm -= (128 * qmf_voc_columns) - 2;
    qmf_band_idx++;
  }
}

VOID ixheaace_hbe_post_anal_xprod3(ixheaace_str_esbr_hbe_txposer *pstr_hbe_txposer,
                                   WORD32 qmf_voc_columns, WORD32 qmf_band_idx, FLOAT32 p,
                                   WORD32 pitch_in_bins_idx) {
  WORD32 i, inp_band_idx, rem;

  FLOAT32 *ptr_out_buf = &pstr_hbe_txposer->qmf_out_buf[2][2 * qmf_band_idx];

  while (qmf_band_idx < pstr_hbe_txposer->x_over_qmf[2]) {
    FLOAT32 temp_r, temp_i;
    FLOAT32 temp_r1, temp_i1;
    const FLOAT32 *ptr_sel, *ptr_sel1;

    inp_band_idx = (2 * qmf_band_idx) / 3;
    ptr_sel = &ixheaac_sel_case[(inp_band_idx + 1) & 3][0];
    ptr_sel1 = &ixheaac_sel_case[((inp_band_idx + 1) & 3) + 1][0];
    rem = 2 * qmf_band_idx - 3 * inp_band_idx;

    if (rem == 0 || rem == 1) {
      FLOAT32 *ptr_in_buf = &pstr_hbe_txposer->qmf_in_buf[0][2 * inp_band_idx];

      for (i = 0; i < qmf_voc_columns; i += 1) {
        WORD32 k;
        FLOAT32 vec_x[2 * IXHEAACE_HBE_OPER_WIN_LEN];
        FLOAT32 *ptr_vec_x = &vec_x[0];
        FLOAT32 x_zero_band_r, x_zero_band_i;

        FLOAT32 mag_scaling_fac;

        for (k = 0; k < (IXHEAACE_HBE_OPER_BLK_LEN_3); k += 2) {
          FLOAT64 base1;
          FLOAT64 base = 1e-17;

          temp_r = ptr_in_buf[0];
          temp_i = ptr_in_buf[1];

          ptr_in_buf += 256;

          base1 = base + temp_r * temp_r;
          base1 = base1 + temp_i * temp_i;

          mag_scaling_fac = (FLOAT32)(ixheaace_cbrt_calc((FLOAT32)base1));

          ptr_vec_x[0] = temp_r * mag_scaling_fac;
          ptr_vec_x[1] = temp_i * mag_scaling_fac;

          temp_r = ptr_in_buf[0];
          temp_i = ptr_in_buf[1];

          ptr_in_buf -= 128;

          temp_r1 = ptr_sel[0] * temp_r + ptr_sel[1] * temp_i;
          temp_i1 = ptr_sel[2] * temp_r + ptr_sel[3] * temp_i;

          temp_r = ptr_in_buf[0];
          temp_i = ptr_in_buf[1];

          temp_r1 += ptr_sel[4] * temp_r + ptr_sel[5] * temp_i;
          temp_i1 += ptr_sel[6] * temp_r + ptr_sel[7] * temp_i;

          temp_r1 *= 0.3984033437f;
          temp_i1 *= 0.3984033437f;

          base1 = base + temp_r1 * temp_r1;
          base1 = base1 + temp_i1 * temp_i1;
          mag_scaling_fac = (FLOAT32)(ixheaace_cbrt_calc((FLOAT32)base1));

          ptr_vec_x[2] = temp_r1 * mag_scaling_fac;
          ptr_vec_x[3] = temp_i1 * mag_scaling_fac;

          ptr_vec_x += 4;
          ptr_in_buf += 256;
        }
        ptr_vec_x = &vec_x[0];
        temp_r = vec_x[2 * (IXHEAACE_HBE_ZERO_BAND_IDX - 2)];
        temp_i = vec_x[(2 * (IXHEAACE_HBE_ZERO_BAND_IDX - 2)) + 1];

        x_zero_band_r = temp_r * temp_r - temp_i * temp_i;
        x_zero_band_i = temp_r * temp_i + temp_i * temp_r;

        for (k = 0; k < (IXHEAACE_HBE_OPER_BLK_LEN_3); k++) {
          temp_r = ptr_vec_x[0] * x_zero_band_r - ptr_vec_x[1] * x_zero_band_i;
          temp_i = ptr_vec_x[0] * x_zero_band_i + ptr_vec_x[1] * x_zero_band_r;

          ptr_out_buf[0] += (temp_r * 0.4714045f);
          ptr_out_buf[1] += (temp_i * 0.4714045f);

          ptr_vec_x += 2;
          ptr_out_buf += 128;
        }

        ixheaace_hbe_xprod_proc_3(pstr_hbe_txposer, qmf_band_idx, i, p, pitch_in_bins_idx);

        ptr_in_buf -= 128 * 11;
        ptr_out_buf -= 128 * 6;
      }
    } else {
      FLOAT32 *ptr_in_buf = &pstr_hbe_txposer->qmf_in_buf[0][2 * inp_band_idx];
      FLOAT32 *ptr_in_buf1 = &pstr_hbe_txposer->qmf_in_buf[0][2 * (inp_band_idx + 1)];

      for (i = 0; i < qmf_voc_columns; i++) {
        WORD32 k;
        FLOAT32 vec_x[2 * IXHEAACE_HBE_OPER_WIN_LEN];
        FLOAT32 vec_x_cap[2 * IXHEAACE_HBE_OPER_WIN_LEN];

        FLOAT32 x_zero_band_r, x_zero_band_i;
        FLOAT32 *ptr_vec_x = &vec_x[0];
        FLOAT32 *ptr_vec_x_cap = &vec_x_cap[0];

        FLOAT32 mag_scaling_fac;

        for (k = 0; k < (IXHEAACE_HBE_OPER_BLK_LEN_3); k += 2) {
          FLOAT32 tmp_vr, tmp_vi;
          FLOAT32 tmp_cr, tmp_ci;
          FLOAT64 base1;
          FLOAT64 base = 1e-17;

          temp_r1 = ptr_in_buf[0];
          temp_i1 = ptr_in_buf[1];
          temp_r = ptr_in_buf1[0];
          temp_i = ptr_in_buf1[1];

          base1 = base + temp_r * temp_r;
          base1 = base1 + temp_i * temp_i;

          mag_scaling_fac = (FLOAT32)(ixheaace_cbrt_calc((FLOAT32)base1));

          ptr_vec_x[0] = temp_r * mag_scaling_fac;
          ptr_vec_x[1] = temp_i * mag_scaling_fac;

          base1 = base + temp_r1 * temp_r1;
          base1 = base1 + temp_i1 * temp_i1;

          mag_scaling_fac = (FLOAT32)(ixheaace_cbrt_calc((FLOAT32)base1));

          ptr_vec_x_cap[0] = temp_r1 * mag_scaling_fac;
          ptr_vec_x_cap[1] = temp_i1 * mag_scaling_fac;

          ptr_in_buf += 256;

          temp_r = ptr_in_buf[0];
          temp_i = ptr_in_buf[1];

          temp_r1 = ptr_sel[0] * temp_r + ptr_sel[1] * temp_i;
          temp_i1 = ptr_sel[2] * temp_r + ptr_sel[3] * temp_i;

          ptr_in_buf -= 128;

          temp_r = ptr_in_buf[0];
          temp_i = ptr_in_buf[1];

          tmp_cr = temp_r1 + ptr_sel[4] * temp_r + ptr_sel[5] * temp_i;
          tmp_ci = temp_i1 + ptr_sel[6] * temp_r + ptr_sel[7] * temp_i;

          ptr_in_buf1 += 256;

          temp_r = ptr_in_buf1[0];
          temp_i = ptr_in_buf1[1];

          temp_r1 = ptr_sel1[0] * temp_r + ptr_sel1[1] * temp_i;
          temp_i1 = ptr_sel1[2] * temp_r + ptr_sel1[3] * temp_i;

          ptr_in_buf1 -= 128;

          temp_r = ptr_in_buf1[0];
          temp_i = ptr_in_buf1[1];

          tmp_vr = temp_r1 + ptr_sel1[4] * temp_r + ptr_sel1[5] * temp_i;
          tmp_vi = temp_i1 + ptr_sel1[6] * temp_r + ptr_sel1[7] * temp_i;

          tmp_cr *= 0.3984033437f;
          tmp_ci *= 0.3984033437f;

          tmp_vr *= 0.3984033437f;
          tmp_vi *= 0.3984033437f;

          base1 = base + tmp_vr * tmp_vr;
          base1 = base1 + tmp_vi * tmp_vi;

          mag_scaling_fac = (FLOAT32)(ixheaace_cbrt_calc((FLOAT32)base1));

          ptr_vec_x[2] = tmp_vr * mag_scaling_fac;
          ptr_vec_x[3] = tmp_vi * mag_scaling_fac;

          base1 = base + tmp_cr * tmp_cr;
          base1 = base1 + tmp_ci * tmp_ci;

          mag_scaling_fac = (FLOAT32)(ixheaace_cbrt_calc((FLOAT32)base1));

          ptr_vec_x_cap[2] = tmp_cr * mag_scaling_fac;
          ptr_vec_x_cap[3] = tmp_ci * mag_scaling_fac;

          ptr_in_buf += 256;
          ptr_in_buf1 += 256;
          ptr_vec_x += 4;
          ptr_vec_x_cap += 4;
        }
        ptr_vec_x = &vec_x[0];
        ptr_vec_x_cap = &vec_x_cap[0];

        temp_r = vec_x_cap[2 * (IXHEAACE_HBE_ZERO_BAND_IDX - 2)];
        temp_i = vec_x_cap[2 * (IXHEAACE_HBE_ZERO_BAND_IDX - 2) + 1];
        temp_r1 = vec_x[2 * (IXHEAACE_HBE_ZERO_BAND_IDX - 2)];
        temp_i1 = vec_x[2 * (IXHEAACE_HBE_ZERO_BAND_IDX - 2) + 1];

        x_zero_band_r = temp_r * temp_r - temp_i * temp_i;
        x_zero_band_i = temp_r * temp_i + temp_i * temp_r;

        temp_r = temp_r1 * temp_r1 - temp_i1 * temp_i1;
        temp_i = temp_r1 * temp_i1 + temp_i1 * temp_r1;

        for (k = 0; k < (IXHEAACE_HBE_OPER_BLK_LEN_3); k++) {
          temp_r1 = ptr_vec_x[0] * x_zero_band_r - ptr_vec_x[1] * x_zero_band_i;
          temp_i1 = ptr_vec_x[0] * x_zero_band_i + ptr_vec_x[1] * x_zero_band_r;

          temp_r1 += ptr_vec_x_cap[0] * temp_r - ptr_vec_x_cap[1] * temp_i;
          temp_i1 += ptr_vec_x_cap[0] * temp_i + ptr_vec_x_cap[1] * temp_r;

          ptr_out_buf[0] += (temp_r1 * 0.23570225f);
          ptr_out_buf[1] += (temp_i1 * 0.23570225f);

          ptr_out_buf += 128;
          ptr_vec_x += 2;
          ptr_vec_x_cap += 2;
        }

        ixheaace_hbe_xprod_proc_3(pstr_hbe_txposer, qmf_band_idx, i, p, pitch_in_bins_idx);

        ptr_in_buf -= 128 * 11;
        ptr_in_buf1 -= 128 * 11;
        ptr_out_buf -= 128 * 6;
      }
    }

    ptr_out_buf -= (256 * qmf_voc_columns) - 2;
    qmf_band_idx++;
  }
}

VOID ixheaace_hbe_post_anal_xprod4(ixheaace_str_esbr_hbe_txposer *pstr_hbe_txposer,
                                   WORD32 qmf_voc_columns, WORD32 qmf_band_idx, FLOAT32 p,
                                   WORD32 pitch_in_bins_idx) {
  WORD32 i, inp_band_idx;
  FLOAT32 *ptr_out = &pstr_hbe_txposer->qmf_out_buf[3][2 * qmf_band_idx];

  ixheaace_norm_qmf_in_buf_4(pstr_hbe_txposer, ((qmf_band_idx >> 1) - 1));

  while (qmf_band_idx < pstr_hbe_txposer->x_over_qmf[3]) {
    WORD32 ip_idx;
    FLOAT32 temp, temp_r, temp_i;
    FLOAT32 *ptr_norm, *ptr_x_norm;
    inp_band_idx = qmf_band_idx >> 1;
    ip_idx = (qmf_band_idx & 1) ? (inp_band_idx + 1) : (inp_band_idx - 1);

    ptr_norm = &pstr_hbe_txposer->norm_qmf_in_buf[0][2 * ip_idx];
    ptr_x_norm = &pstr_hbe_txposer->norm_qmf_in_buf[IXHEAACE_HBE_ZERO_BAND_IDX][2 * inp_band_idx];

    for (i = 0; i < qmf_voc_columns; i++) {
      WORD32 k;
      FLOAT32 x_zero_band_r, x_zero_band_i;

      temp_r = x_zero_band_r = *ptr_x_norm++;
      temp_i = x_zero_band_i = *ptr_x_norm++;

      temp = x_zero_band_r * x_zero_band_r - x_zero_band_i * x_zero_band_i;
      x_zero_band_i = x_zero_band_r * x_zero_band_i + x_zero_band_i * x_zero_band_r;

      x_zero_band_r = temp_r * temp - temp_i * x_zero_band_i;
      x_zero_band_i = temp_r * x_zero_band_i + temp_i * temp;

      for (k = 0; k < IXHEAACE_HBE_OPER_BLK_LEN_4; k++) {
        temp = *ptr_norm++;
        temp_i = *ptr_norm++;

        temp_r = temp * x_zero_band_r - temp_i * x_zero_band_i;
        temp_i = temp * x_zero_band_i + temp_i * x_zero_band_r;

        *ptr_out++ += (temp_r * 0.6666667f);
        *ptr_out++ += (temp_i * 0.6666667f);

        ptr_norm += 254;
        ptr_out += 126;
      }

      ptr_norm -= 128 * 11;
      ptr_out -= 128 * 4;
      ptr_x_norm += 126;

      ixheaace_hbe_xprod_proc_4(pstr_hbe_txposer, qmf_band_idx, i, p, pitch_in_bins_idx);
    }

    ptr_out -= (128 * 2 * qmf_voc_columns) - 2;
    qmf_band_idx++;
  }
}

IA_ERRORCODE ixheaace_hbe_post_anal_process(ixheaace_str_esbr_hbe_txposer *pstr_hbe_txposer,
                                            WORD32 pitch_in_bins, WORD32 sbr_upsamp_4_flg) {
  FLOAT32 p;
  WORD32 trans_fac;
  WORD32 qmf_voc_columns = pstr_hbe_txposer->no_bins / 2;
  FLOAT32 ptr_cos_sin_theta[2];

  p = (sbr_upsamp_4_flg) ? (FLOAT32)(pitch_in_bins * 0.04166666666666)
                         : (FLOAT32)(pitch_in_bins * 0.08333333333333);

  if (p < IXHEAACE_SBR_CONST_PMIN) {
    trans_fac = 2;
    if (trans_fac <= pstr_hbe_txposer->max_stretch) {
      ixheaace_hbe_post_anal_prod2(pstr_hbe_txposer, qmf_voc_columns,
                                   pstr_hbe_txposer->x_over_qmf[0]);
    }

    trans_fac = 3;
    if (trans_fac <= pstr_hbe_txposer->max_stretch) {
      ixheaace_hbe_post_anal_prod3(pstr_hbe_txposer, qmf_voc_columns,
                                   pstr_hbe_txposer->x_over_qmf[1]);
    }

    trans_fac = 4;
    if (trans_fac <= pstr_hbe_txposer->max_stretch) {
      if (pstr_hbe_txposer->x_over_qmf[2] <= 1) {
        return IA_EXHEAACE_EXE_NONFATAL_ESBR_INVALID_VALUE;
      }
      ixheaace_hbe_post_anal_prod4(pstr_hbe_txposer, qmf_voc_columns,
                                   pstr_hbe_txposer->x_over_qmf[2]);
    }
  } else {
    trans_fac = 2;
    if (trans_fac <= pstr_hbe_txposer->max_stretch) {
      ptr_cos_sin_theta[0] =
          ixheaac_hbe_x_prod_cos_table_trans_2[((pitch_in_bins + sbr_upsamp_4_flg * 128) << 1) +
                                               0];
      ptr_cos_sin_theta[1] =
          ixheaac_hbe_x_prod_cos_table_trans_2[((pitch_in_bins + sbr_upsamp_4_flg * 128) << 1) +
                                               1];

      ixheaace_hbe_post_anal_xprod2(pstr_hbe_txposer, qmf_voc_columns,
                                    pstr_hbe_txposer->x_over_qmf[0], p, ptr_cos_sin_theta);
    }

    trans_fac = 3;
    if (trans_fac <= pstr_hbe_txposer->max_stretch) {
      ixheaace_hbe_post_anal_xprod3(pstr_hbe_txposer, qmf_voc_columns,
                                    pstr_hbe_txposer->x_over_qmf[1], p,
                                    (pitch_in_bins + sbr_upsamp_4_flg * 128));
    }

    trans_fac = 4;
    if (trans_fac <= pstr_hbe_txposer->max_stretch) {
      if (pstr_hbe_txposer->x_over_qmf[2] <= 1) {
        return IA_EXHEAACE_EXE_NONFATAL_ESBR_INVALID_VALUE;
      }
      ixheaace_hbe_post_anal_xprod4(pstr_hbe_txposer, qmf_voc_columns,
                                    pstr_hbe_txposer->x_over_qmf[2], p,
                                    (pitch_in_bins + sbr_upsamp_4_flg * 128));
    }
  }
  return IA_NO_ERROR;
}
VOID ixheaace_hbe_repl_spec(WORD32 x_over_qmf[IXHEAACE_MAX_NUM_PATCHES],
                            FLOAT32 qmf_buf_real[][64], FLOAT32 qmf_buf_imag[][64],
                            WORD32 no_bins, WORD32 max_stretch) {
  WORD32 patch_bands;
  WORD32 patch, band, col, target, source_bands, i;
  WORD32 num_patches = 0;

  for (i = 1; i < IXHEAACE_MAX_NUM_PATCHES; i++) {
    if (x_over_qmf[i] != 0) {
      num_patches++;
    }
  }

  for (patch = (max_stretch - 1); patch < num_patches; patch++) {
    patch_bands = x_over_qmf[patch + 1] - x_over_qmf[patch];
    target = x_over_qmf[patch];
    source_bands = x_over_qmf[max_stretch - 1] - x_over_qmf[max_stretch - 2];

    while (patch_bands > 0) {
      WORD32 ixheaace_num_bands = source_bands;
      WORD32 start_band = x_over_qmf[max_stretch - 1] - 1;
      if (target + ixheaace_num_bands >= x_over_qmf[patch + 1]) {
        ixheaace_num_bands = x_over_qmf[patch + 1] - target;
      }
      if ((((target + ixheaace_num_bands - 1) & 1) + ((x_over_qmf[max_stretch - 1] - 1) & 1)) &
          1) {
        if (ixheaace_num_bands == source_bands) {
          ixheaace_num_bands--;
        } else {
          start_band--;
        }
      }

      if (!ixheaace_num_bands) {
        break;
      }

      for (col = 0; col < no_bins; col++) {
        band = target + ixheaace_num_bands - 1;
        if (64 <= band) {
          band = 63;
        }
        if (x_over_qmf[patch + 1] <= band) {
          band = x_over_qmf[patch + 1] - 1;
        }
        for (i = 0; i < ixheaace_num_bands; i++, band--) {
          qmf_buf_real[col][band] = qmf_buf_real[col][start_band - i];
          qmf_buf_imag[col][band] = qmf_buf_imag[col][start_band - i];
        }
      }
      target += ixheaace_num_bands;
      patch_bands -= ixheaace_num_bands;
    }
  }
}
