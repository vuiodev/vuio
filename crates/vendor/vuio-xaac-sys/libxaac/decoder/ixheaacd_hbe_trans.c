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
#include <math.h>
#include <stdlib.h>
#include <string.h>

#include "ixheaac_type_def.h"

#include "ixheaacd_bitbuffer.h"

#include "ixheaacd_interface.h"

#include "ixheaacd_tns_usac.h"
#include "ixheaacd_cnst.h"

#include "ixheaacd_acelp_info.h"

#include "ixheaacd_sbrdecsettings.h"
#include "ixheaacd_info.h"
#include "ixheaacd_sbr_common.h"
#include "ixheaacd_drc_data_struct.h"
#include "ixheaacd_drc_dec.h"
#include "ixheaacd_sbrdecoder.h"
#include "ixheaacd_mps_polyphase.h"
#include "ixheaac_sbr_const.h"

#include "ixheaacd_env_extr_part.h"
#include "ixheaacd_sbr_rom.h"
#include "ixheaacd_common_rom.h"
#include "ixheaacd_hybrid.h"
#include "ixheaacd_sbr_scale.h"
#include "ixheaacd_ps_dec.h"
#include "ixheaacd_freq_sca.h"
#include "ixheaacd_lpp_tran.h"
#include "ixheaacd_bitbuffer.h"
#include "ixheaacd_env_extr.h"
#include "ixheaacd_qmf_dec.h"
#include "ixheaacd_env_calc.h"
#include "ixheaacd_pvc_dec.h"

#include "ixheaacd_sbr_dec.h"
#include "ixheaac_error_standards.h"
#include "ixheaacd_sbrqmftrans.h"
#include "ixheaacd_qmf_poly.h"

#include "ixheaac_constants.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_op.h"

#include "ixheaac_esbr_rom.h"

#define SBR_CONST_PMIN 1.0f

static FLOAT32 *ixheaacd_map_prot_filter(WORD32 filt_length) {
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

WORD32 ixheaacd_qmf_hbe_data_reinit(ia_esbr_hbe_txposer_struct *ptr_hbe_txposer,
                                    WORD16 *p_freq_band_tab[2],
                                    WORD16 *p_num_sfb, WORD32 upsamp_4_flag) {
  WORD32 synth_size, sfb, patch, stop_patch;

  if (ptr_hbe_txposer != NULL) {
    ptr_hbe_txposer->start_band = p_freq_band_tab[LOW][0];
    ptr_hbe_txposer->end_band = p_freq_band_tab[LOW][p_num_sfb[LOW]];

    ptr_hbe_txposer->synth_size =
        4 * ((ptr_hbe_txposer->start_band + 4) / 8 + 1);
    ptr_hbe_txposer->k_start =
        ixheaac_start_subband2kL_tbl[ptr_hbe_txposer->start_band];

    ptr_hbe_txposer->upsamp_4_flag = upsamp_4_flag;
    ptr_hbe_txposer->esbr_hq = 0;

    if (upsamp_4_flag) {
      if (ptr_hbe_txposer->k_start + ptr_hbe_txposer->synth_size > 16)
        ptr_hbe_txposer->k_start = 16 - ptr_hbe_txposer->synth_size;
    } else if (ptr_hbe_txposer->core_frame_length == 768) {
      if (ptr_hbe_txposer->k_start + ptr_hbe_txposer->synth_size > 24)
        ptr_hbe_txposer->k_start = 24 - ptr_hbe_txposer->synth_size;
    }

    memset(ptr_hbe_txposer->synth_buf, 0, 1280 * sizeof(FLOAT32));
    synth_size = ptr_hbe_txposer->synth_size;
    ptr_hbe_txposer->synth_buf_offset = 18 * synth_size;
    switch (synth_size) {
      case 4:
        ptr_hbe_txposer->synth_cos_tab =
            (FLOAT32 *)ixheaac_synth_cos_table_kl_4;
        ptr_hbe_txposer->analy_cos_sin_tab =
            (FLOAT32 *)ixheaac_analy_cos_sin_table_kl_8;
        ptr_hbe_txposer->ixheaacd_real_synth_fft = &ixheaac_real_synth_fft_p2;
        ptr_hbe_txposer->ixheaacd_cmplx_anal_fft = &ixheaac_cmplx_anal_fft_p2;
        break;
      case 8:
        ptr_hbe_txposer->synth_cos_tab =
            (FLOAT32 *)ixheaac_synth_cos_table_kl_8;
        ptr_hbe_txposer->analy_cos_sin_tab =
            (FLOAT32 *)ixheaac_analy_cos_sin_table_kl_16;
        ptr_hbe_txposer->ixheaacd_real_synth_fft = &ixheaac_real_synth_fft_p2;
        ptr_hbe_txposer->ixheaacd_cmplx_anal_fft = &ixheaac_cmplx_anal_fft_p2;
        break;
      case 12:
        ptr_hbe_txposer->synth_cos_tab =
            (FLOAT32 *)ixheaac_synth_cos_table_kl_12;
        ptr_hbe_txposer->analy_cos_sin_tab =
            (FLOAT32 *)ixheaac_analy_cos_sin_table_kl_24;
        ptr_hbe_txposer->ixheaacd_real_synth_fft = &ixheaac_real_synth_fft_p3;
        ptr_hbe_txposer->ixheaacd_cmplx_anal_fft = &ixheaac_cmplx_anal_fft_p3;
        break;
      case 16:
        ptr_hbe_txposer->synth_cos_tab =
            (FLOAT32 *)ixheaac_synth_cos_table_kl_16;
        ptr_hbe_txposer->analy_cos_sin_tab =
            (FLOAT32 *)ixheaac_analy_cos_sin_table_kl_32;
        ptr_hbe_txposer->ixheaacd_real_synth_fft = &ixheaac_real_synth_fft_p2;
        ptr_hbe_txposer->ixheaacd_cmplx_anal_fft = &ixheaac_cmplx_anal_fft_p2;
        break;
      case 20:
        ptr_hbe_txposer->synth_cos_tab =
            (FLOAT32 *)ixheaac_synth_cos_table_kl_20;
        ptr_hbe_txposer->analy_cos_sin_tab =
            (FLOAT32 *)ixheaac_analy_cos_sin_table_kl_40;
        break;
      default:
        ptr_hbe_txposer->synth_cos_tab =
            (FLOAT32 *)ixheaac_synth_cos_table_kl_4;
        ptr_hbe_txposer->analy_cos_sin_tab =
            (FLOAT32 *)ixheaac_analy_cos_sin_table_kl_8;
        ptr_hbe_txposer->ixheaacd_real_synth_fft = &ixheaac_real_synth_fft_p2;
        ptr_hbe_txposer->ixheaacd_cmplx_anal_fft = &ixheaac_cmplx_anal_fft_p2;
    }

    ptr_hbe_txposer->synth_wind_coeff = ixheaacd_map_prot_filter(synth_size);

    memset(ptr_hbe_txposer->analy_buf, 0, 640 * sizeof(FLOAT32));
    synth_size = 2 * ptr_hbe_txposer->synth_size;
    ptr_hbe_txposer->analy_wind_coeff = ixheaacd_map_prot_filter(synth_size);

    memset(ptr_hbe_txposer->x_over_qmf, 0, MAX_NUM_PATCHES * sizeof(WORD32));
    sfb = 0;
    if (upsamp_4_flag) {
      stop_patch = MAX_NUM_PATCHES;
      ptr_hbe_txposer->max_stretch = MAX_STRETCH;
    } else {
      stop_patch = MAX_STRETCH;
    }

    for (patch = 1; patch <= stop_patch; patch++) {
      while (sfb <= p_num_sfb[LOW] &&
             p_freq_band_tab[LOW][sfb] <= patch * ptr_hbe_txposer->start_band)
        sfb++;
      if (sfb <= p_num_sfb[LOW]) {
        if ((patch * ptr_hbe_txposer->start_band -
             p_freq_band_tab[LOW][sfb - 1]) <= 3) {
          ptr_hbe_txposer->x_over_qmf[patch - 1] =
              p_freq_band_tab[LOW][sfb - 1];
        } else {
          WORD32 sfb = 0;
          while (sfb <= p_num_sfb[HIGH] &&
                 p_freq_band_tab[HIGH][sfb] <=
                     patch * ptr_hbe_txposer->start_band)
            sfb++;
          ptr_hbe_txposer->x_over_qmf[patch - 1] =
              p_freq_band_tab[HIGH][sfb - 1];
        }
      } else {
        ptr_hbe_txposer->x_over_qmf[patch - 1] = ptr_hbe_txposer->end_band;
        ptr_hbe_txposer->max_stretch = min(patch, MAX_STRETCH);
        break;
      }
    }
    if (ptr_hbe_txposer->k_start < 0) {
      return -1;
    }
  }
  return 0;
}

WORD32 ixheaacd_qmf_hbe_apply(ia_esbr_hbe_txposer_struct *ptr_hbe_txposer,
                              FLOAT32 qmf_buf_real[][64],
                              FLOAT32 qmf_buf_imag[][64], WORD32 num_columns,
                              FLOAT32 pv_qmf_buf_real[][64],
                              FLOAT32 pv_qmf_buf_imag[][64],
                              WORD32 pitch_in_bins,
                              ia_sbr_header_data_struct *ptr_header_data) {
  WORD32 i, qmf_band_idx;
  WORD32 qmf_voc_columns = ptr_hbe_txposer->no_bins / 2;
  WORD32 err_code = 0;

  memcpy(ptr_hbe_txposer->ptr_input_buf,
         ptr_hbe_txposer->ptr_input_buf +
             ptr_hbe_txposer->no_bins * ptr_hbe_txposer->synth_size,
         ptr_hbe_txposer->synth_size * sizeof(FLOAT32));

  if (ptr_hbe_txposer->ixheaacd_cmplx_anal_fft == NULL) {
    WORD32 err = ixheaacd_qmf_hbe_data_reinit(
        ptr_hbe_txposer,
        ptr_header_data->pstr_freq_band_data->freq_band_table,
        ptr_header_data->pstr_freq_band_data->num_sf_bands,
        ptr_header_data->is_usf_4);
    if (err)
      return err;
  }

  err_code = ixheaacd_real_synth_filt(ptr_hbe_txposer, num_columns,
                                      qmf_buf_real, qmf_buf_imag);
  if (err_code) return err_code;

  for (i = 0; i < HBE_OPER_WIN_LEN - 1; i++) {
    memcpy(ptr_hbe_txposer->qmf_in_buf[i],
           ptr_hbe_txposer->qmf_in_buf[i + qmf_voc_columns],
           TWICE_QMF_SYNTH_CHANNELS_NUM * sizeof(FLOAT32));
  }

  err_code = ixheaacd_complex_anal_filt(ptr_hbe_txposer);
  if (err_code) return err_code;

  for (i = 0; i < (ptr_hbe_txposer->hbe_qmf_out_len - ptr_hbe_txposer->no_bins);
       i++) {
    memcpy(ptr_hbe_txposer->qmf_out_buf[i],
           ptr_hbe_txposer->qmf_out_buf[i + ptr_hbe_txposer->no_bins],
           TWICE_QMF_SYNTH_CHANNELS_NUM * sizeof(FLOAT32));
  }

  for (; i < ptr_hbe_txposer->hbe_qmf_out_len; i++) {
    memset(ptr_hbe_txposer->qmf_out_buf[i], 0,
           TWICE_QMF_SYNTH_CHANNELS_NUM * sizeof(FLOAT32));
  }

  err_code = ixheaacd_hbe_post_anal_process(ptr_hbe_txposer, pitch_in_bins,
                                            ptr_hbe_txposer->upsamp_4_flag);
  if (err_code) return err_code;

  for (i = 0; i < ptr_hbe_txposer->no_bins; i++) {
    for (qmf_band_idx = ptr_hbe_txposer->start_band;
         qmf_band_idx < ptr_hbe_txposer->end_band; qmf_band_idx++) {
      pv_qmf_buf_real[i][qmf_band_idx] =
          (FLOAT32)(ptr_hbe_txposer->qmf_out_buf[i][2 * qmf_band_idx] *
                        ixheaac_phase_vocoder_cos_table[qmf_band_idx] -
                    ptr_hbe_txposer->qmf_out_buf[i][2 * qmf_band_idx + 1] *
                        ixheaac_phase_vocoder_sin_table[qmf_band_idx]);

      pv_qmf_buf_imag[i][qmf_band_idx] =
          (FLOAT32)(ptr_hbe_txposer->qmf_out_buf[i][2 * qmf_band_idx] *
                        ixheaac_phase_vocoder_sin_table[qmf_band_idx] +
                    ptr_hbe_txposer->qmf_out_buf[i][2 * qmf_band_idx + 1] *
                        ixheaac_phase_vocoder_cos_table[qmf_band_idx]);
    }
  }
  return 0;
}

VOID ixheaacd_norm_qmf_in_buf_4(ia_esbr_hbe_txposer_struct *ptr_hbe_txposer,
                                WORD32 qmf_band_idx) {
  WORD32 i;
  FLOAT32 *in_buf = &ptr_hbe_txposer->qmf_in_buf[0][2 * qmf_band_idx];
  FLOAT32 *norm_buf = &ptr_hbe_txposer->norm_qmf_in_buf[0][2 * qmf_band_idx];

  for (; qmf_band_idx <= ptr_hbe_txposer->x_over_qmf[3]; qmf_band_idx++) {
    for (i = 0; i < ptr_hbe_txposer->hbe_qmf_in_len; i++) {
      FLOAT32 mag_scaling_fac = 0.0f;
      FLOAT32 x_r, x_i, temp;
      FLOAT64 base = 1e-17;
      x_r = in_buf[0];
      x_i = in_buf[1];

      temp = x_r * x_r;
      base = base + temp;
      temp = x_i * x_i;
      base = base + temp;

      temp = (FLOAT32)sqrt(sqrt(base));
      mag_scaling_fac = temp * (FLOAT32)(sqrt(temp));

      mag_scaling_fac = 1 / mag_scaling_fac;

      x_r *= mag_scaling_fac;
      x_i *= mag_scaling_fac;

      norm_buf[0] = x_r;
      norm_buf[1] = x_i;

      in_buf += 128;
      norm_buf += 128;
    }

    in_buf -= (128 * (ptr_hbe_txposer->hbe_qmf_in_len) - 2);
    norm_buf -= (128 * (ptr_hbe_txposer->hbe_qmf_in_len) - 2);
  }
}

VOID ixheaacd_norm_qmf_in_buf_2(ia_esbr_hbe_txposer_struct *ptr_hbe_txposer,
                                WORD32 qmf_band_idx) {
  WORD32 i;
  FLOAT32 *in_buf = &ptr_hbe_txposer->qmf_in_buf[0][2 * qmf_band_idx];
  FLOAT32 *norm_buf = &ptr_hbe_txposer->norm_qmf_in_buf[0][2 * qmf_band_idx];

  for (; qmf_band_idx <= ptr_hbe_txposer->x_over_qmf[1]; qmf_band_idx++) {
    for (i = 0; i < ptr_hbe_txposer->hbe_qmf_in_len; i++) {
      FLOAT32 mag_scaling_fac = 0.0f;
      FLOAT32 x_r, x_i, temp;
      FLOAT64 base = 1e-17;
      x_r = in_buf[0];
      x_i = in_buf[1];

      temp = x_r * x_r;
      base = base + temp;
      temp = x_i * x_i;
      base = base + x_i * x_i;

      mag_scaling_fac = (FLOAT32)(1.0f / base);
      mag_scaling_fac = (FLOAT32)sqrt(sqrt(mag_scaling_fac));

      x_r *= mag_scaling_fac;
      x_i *= mag_scaling_fac;

      norm_buf[0] = x_r;
      norm_buf[1] = x_i;

      in_buf += 128;
      norm_buf += 128;
    }

    in_buf -= (128 * (ptr_hbe_txposer->hbe_qmf_in_len) - 2);
    norm_buf -= (128 * (ptr_hbe_txposer->hbe_qmf_in_len) - 2);
  }
}

VOID ixheaacd_hbe_xprod_proc_3(ia_esbr_hbe_txposer_struct *ptr_hbe_txposer,
                               WORD32 qmf_band_idx, WORD32 qmf_col_idx,
                               FLOAT32 p, WORD32 pitch_in_bins_idx) {
  WORD32 tr, n1, n2, max_trans_fac, max_n1, max_n2;
  WORD32 k, addrshift;
  WORD32 inp_band_idx = 2 * qmf_band_idx / 3;

  FLOAT64 temp_fac;
  FLOAT32 max_mag_value;
  FLOAT32 mag_zero_band, mag_n1_band, mag_n2_band, temp;
  FLOAT32 temp_r, temp_i;
  FLOAT32 mag_cmplx_gain = 1.8856f;

  FLOAT32 *qmf_in_buf_ri =
      ptr_hbe_txposer->qmf_in_buf[qmf_col_idx + HBE_ZERO_BAND_IDX];

  mag_zero_band =
      qmf_in_buf_ri[2 * inp_band_idx] * qmf_in_buf_ri[2 * inp_band_idx] +
      qmf_in_buf_ri[2 * inp_band_idx + 1] * qmf_in_buf_ri[2 * inp_band_idx + 1];
  max_mag_value = 0;
  max_n1 = max_n2 = max_trans_fac = 0;

  for (tr = 1; tr < 3; tr++) {
    temp_fac = (2.0f * qmf_band_idx + 1 - tr * p) * 0.3333334;

    n1 = (WORD32)(temp_fac);
    n2 = (WORD32)(temp_fac + p);

    mag_n1_band = qmf_in_buf_ri[2 * n1] * qmf_in_buf_ri[2 * n1] +
                  qmf_in_buf_ri[2 * n1 + 1] * qmf_in_buf_ri[2 * n1 + 1];
    mag_n2_band = qmf_in_buf_ri[2 * n2] * qmf_in_buf_ri[2 * n2] +
                  qmf_in_buf_ri[2 * n2 + 1] * qmf_in_buf_ri[2 * n2 + 1];
    temp = min(mag_n1_band, mag_n2_band);

    if (temp > max_mag_value) {
      max_mag_value = temp;
      max_trans_fac = tr;
      max_n1 = n1;
      max_n2 = n2;
    }
  }

  if (max_mag_value > mag_zero_band && max_n1 >= 0 &&
      max_n2 < NO_QMF_SYNTH_CHANNELS) {
    FLOAT32 vec_y_r[2], vec_y_i[2], vec_o_r[2], vec_o_i[2];
    FLOAT32 coeff_real[2], coeff_imag[2];
    FLOAT32 d1, d2;
    WORD32 mid_trans_fac, idx;
    FLOAT64 base = 1e-17;
    FLOAT32 mag_scaling_fac = 0;
    FLOAT32 x_zero_band_r;
    FLOAT32 x_zero_band_i;

    x_zero_band_r = 0;
    x_zero_band_i = 0;
    mid_trans_fac = 3 - max_trans_fac;
    if (max_trans_fac == 1) {
      WORD32 idx;
      d1 = 0;
      d2 = 1.5;
      x_zero_band_r = qmf_in_buf_ri[2 * max_n1];
      x_zero_band_i = qmf_in_buf_ri[2 * max_n1 + 1];

      idx = max_n2 & 3;
      idx = (idx + 1) & 3;
      coeff_real[0] = ixheaac_hbe_post_anal_proc_interp_coeff[idx][0];
      coeff_imag[0] = ixheaac_hbe_post_anal_proc_interp_coeff[idx][1];

      coeff_real[1] = coeff_real[0];
      coeff_imag[1] = -coeff_imag[0];

      vec_y_r[1] = qmf_in_buf_ri[2 * max_n2];
      vec_y_i[1] = qmf_in_buf_ri[2 * max_n2 + 1];

      addrshift = -2;
      temp_r = ptr_hbe_txposer->qmf_in_buf[qmf_col_idx + addrshift +
                                           HBE_ZERO_BAND_IDX][2 * max_n2];
      temp_i = ptr_hbe_txposer->qmf_in_buf[qmf_col_idx + addrshift +
                                           HBE_ZERO_BAND_IDX][2 * max_n2 + 1];

      vec_y_r[0] = coeff_real[1] * temp_r - coeff_imag[1] * temp_i;
      vec_y_i[0] = coeff_imag[1] * temp_r + coeff_real[1] * temp_i;

      temp_r = ptr_hbe_txposer->qmf_in_buf[qmf_col_idx + addrshift + 1 +
                                           HBE_ZERO_BAND_IDX][2 * max_n2];
      temp_i = ptr_hbe_txposer->qmf_in_buf[qmf_col_idx + addrshift + 1 +
                                           HBE_ZERO_BAND_IDX][2 * max_n2 + 1];

      vec_y_r[0] += coeff_real[0] * temp_r - coeff_imag[0] * temp_i;
      vec_y_i[0] += coeff_imag[0] * temp_r + coeff_real[0] * temp_i;

    } else {
      WORD32 idx;
      d1 = 1.5;
      d2 = 0;
      mid_trans_fac = max_trans_fac;
      max_trans_fac = 3 - max_trans_fac;

      x_zero_band_r = qmf_in_buf_ri[2 * max_n2];
      x_zero_band_i = qmf_in_buf_ri[2 * max_n2 + 1];

      idx = (max_n1 & 3);
      idx = (idx + 1) & 3;
      coeff_real[0] = ixheaac_hbe_post_anal_proc_interp_coeff[idx][0];
      coeff_imag[0] = ixheaac_hbe_post_anal_proc_interp_coeff[idx][1];

      coeff_real[1] = coeff_real[0];
      coeff_imag[1] = -coeff_imag[0];

      vec_y_r[1] = qmf_in_buf_ri[2 * max_n1];
      vec_y_i[1] = qmf_in_buf_ri[2 * max_n1 + 1];

      addrshift = -2;

      temp_r = ptr_hbe_txposer->qmf_in_buf[qmf_col_idx + addrshift +
                                           HBE_ZERO_BAND_IDX][2 * max_n1];
      temp_i = ptr_hbe_txposer->qmf_in_buf[qmf_col_idx + addrshift +
                                           HBE_ZERO_BAND_IDX][2 * max_n1 + 1];

      vec_y_r[0] = coeff_real[1] * temp_r - coeff_imag[1] * temp_i;
      vec_y_i[0] = coeff_imag[1] * temp_r + coeff_real[1] * temp_i;

      temp_r = ptr_hbe_txposer->qmf_in_buf[qmf_col_idx + addrshift + 1 +
                                           HBE_ZERO_BAND_IDX][2 * max_n1];
      temp_i = ptr_hbe_txposer->qmf_in_buf[qmf_col_idx + addrshift + 1 +
                                           HBE_ZERO_BAND_IDX][2 * max_n1 + 1];

      vec_y_r[0] += coeff_real[0] * temp_r - coeff_imag[0] * temp_i;
      vec_y_i[0] += coeff_imag[0] * temp_r + coeff_real[0] * temp_i;
    }

    base = 1e-17;
    base = base + x_zero_band_r * x_zero_band_r;
    base = base + x_zero_band_i * x_zero_band_i;
    mag_scaling_fac = (FLOAT32)(ixheaac_cbrt_calc((FLOAT32)base));
    x_zero_band_r *= mag_scaling_fac;
    x_zero_band_i *= mag_scaling_fac;
    for (k = 0; k < 2; k++) {
      base = 1e-17;
      base = base + vec_y_r[k] * vec_y_r[k];
      base = base + vec_y_i[k] * vec_y_i[k];
      mag_scaling_fac = (FLOAT32)(ixheaac_cbrt_calc((FLOAT32)base));
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
      FLOAT32 cos_theta =
          ixheaac_hbe_x_prod_cos_table_trans_3[(pitch_in_bins_idx << 1) + 0];
      FLOAT32 sin_theta =
          ixheaac_hbe_x_prod_cos_table_trans_3[(pitch_in_bins_idx << 1) + 1];
      if (d2 < d1) {
        sin_theta = -sin_theta;
      }
      temp_r = vec_o_r[0];
      temp_i = vec_o_i[0];
      vec_o_r[0] = (FLOAT32)(cos_theta * temp_r - sin_theta * temp_i);
      vec_o_i[0] = (FLOAT32)(cos_theta * temp_i + sin_theta * temp_r);
    }

    for (k = 0; k < 2; k++) {
      ptr_hbe_txposer->qmf_out_buf[qmf_col_idx * 2 + (k + HBE_ZERO_BAND_IDX -
                                                      1)][2 * qmf_band_idx] +=
          (FLOAT32)(mag_cmplx_gain * vec_o_r[k]);
      ptr_hbe_txposer
          ->qmf_out_buf[qmf_col_idx * 2 + (k + HBE_ZERO_BAND_IDX - 1)]
                       [2 * qmf_band_idx + 1] +=
          (FLOAT32)(mag_cmplx_gain * vec_o_i[k]);
    }
  }
}

VOID ixheaacd_hbe_xprod_proc_4(ia_esbr_hbe_txposer_struct *ptr_hbe_txposer,
                               WORD32 qmf_band_idx, WORD32 qmf_col_idx,
                               FLOAT32 p, WORD32 pitch_in_bins_idx) {
  WORD32 k;
  WORD32 inp_band_idx = qmf_band_idx >> 1;
  WORD32 tr, n1, n2, max_trans_fac, max_n1, max_n2;

  FLOAT64 temp_fac;
  FLOAT32 max_mag_value, mag_zero_band, mag_n1_band, mag_n2_band, temp;
  FLOAT32 temp_r, temp_i;
  FLOAT32 mag_cmplx_gain = 2.0f;

  FLOAT32 *qmf_in_buf_ri =
      ptr_hbe_txposer->qmf_in_buf[qmf_col_idx + HBE_ZERO_BAND_IDX];

  mag_zero_band =
      qmf_in_buf_ri[2 * inp_band_idx] * qmf_in_buf_ri[2 * inp_band_idx] +
      qmf_in_buf_ri[2 * inp_band_idx + 1] * qmf_in_buf_ri[2 * inp_band_idx + 1];

  max_mag_value = 0;
  max_n1 = max_n2 = max_trans_fac = 0;

  for (tr = 1; tr < 4; tr++) {
    temp_fac = (2.0 * qmf_band_idx + 1 - tr * p) * 0.25;
    n1 = ((WORD32)(temp_fac)) << 1;
    n2 = ((WORD32)(temp_fac + p)) << 1;

    mag_n1_band = qmf_in_buf_ri[n1] * qmf_in_buf_ri[n1] +
                  qmf_in_buf_ri[n1 + 1] * qmf_in_buf_ri[n1 + 1];
    mag_n2_band = qmf_in_buf_ri[n2] * qmf_in_buf_ri[n2] +
                  qmf_in_buf_ri[n2 + 1] * qmf_in_buf_ri[n2 + 1];

    temp = min(mag_n1_band, mag_n2_band);

    if (temp > max_mag_value) {
      max_mag_value = temp;
      max_trans_fac = tr;
      max_n1 = n1;
      max_n2 = n2;
    }
  }
  if (max_mag_value > mag_zero_band && max_n1 >= 0 &&
      max_n2 < TWICE_QMF_SYNTH_CHANNELS_NUM) {
    FLOAT32 vec_y_r[2], vec_y_i[2], vec_o_r[2], vec_o_i[2];
    FLOAT32 d1, d2;
    WORD32 mid_trans_fac, idx;
    FLOAT32 x_zero_band_r;
    FLOAT32 x_zero_band_i;
    FLOAT64 base = 1e-17;
    FLOAT32 mag_scaling_fac = 0.0f;

    x_zero_band_r = 0;
    x_zero_band_i = 0;
    mid_trans_fac = 4 - max_trans_fac;

    if (max_trans_fac == 1) {
      d1 = 0;
      d2 = 2;
      x_zero_band_r = qmf_in_buf_ri[max_n1];
      x_zero_band_i = qmf_in_buf_ri[max_n1 + 1];
      for (k = 0; k < 2; k++) {
        vec_y_r[k] =
            ptr_hbe_txposer->qmf_in_buf[qmf_col_idx + HBE_ZERO_BAND_IDX +
                                        2 * (k - 1)][max_n2];
        vec_y_i[k] =
            ptr_hbe_txposer->qmf_in_buf[qmf_col_idx + HBE_ZERO_BAND_IDX +
                                        2 * (k - 1)][max_n2 + 1];
      }
    } else if (max_trans_fac == 2) {
      d1 = 0;
      d2 = 1;
      x_zero_band_r = qmf_in_buf_ri[max_n1];
      x_zero_band_i = qmf_in_buf_ri[max_n1 + 1];
      for (k = 0; k < 2; k++) {
        vec_y_r[k] =
            ptr_hbe_txposer
                ->qmf_in_buf[qmf_col_idx + HBE_ZERO_BAND_IDX + (k - 1)][max_n2];
        vec_y_i[k] =
            ptr_hbe_txposer->qmf_in_buf[qmf_col_idx + HBE_ZERO_BAND_IDX +
                                        (k - 1)][max_n2 + 1];
      }
    } else {
      d1 = 2;
      d2 = 0;
      mid_trans_fac = max_trans_fac;
      max_trans_fac = 4 - max_trans_fac;
      x_zero_band_r = qmf_in_buf_ri[max_n2];
      x_zero_band_i = qmf_in_buf_ri[max_n2 + 1];
      for (k = 0; k < 2; k++) {
        vec_y_r[k] =
            ptr_hbe_txposer->qmf_in_buf[qmf_col_idx + HBE_ZERO_BAND_IDX +
                                        2 * (k - 1)][max_n1];
        vec_y_i[k] =
            ptr_hbe_txposer->qmf_in_buf[qmf_col_idx + HBE_ZERO_BAND_IDX +
                                        2 * (k - 1)][max_n1 + 1];
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
        cos_theta =
            ixheaac_hbe_x_prod_cos_table_trans_4_1[(pitch_in_bins_idx << 1) +
                                                    0];
        sin_theta =
            ixheaac_hbe_x_prod_cos_table_trans_4_1[(pitch_in_bins_idx << 1) +
                                                    1];
      } else {
        cos_theta =
            ixheaac_hbe_x_prod_cos_table_trans_4[(pitch_in_bins_idx << 1) + 0];
        sin_theta =
            ixheaac_hbe_x_prod_cos_table_trans_4[(pitch_in_bins_idx << 1) + 1];
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
      ptr_hbe_txposer->qmf_out_buf[qmf_col_idx * 2 + (k + HBE_ZERO_BAND_IDX -
                                                      1)][2 * qmf_band_idx] +=
          (FLOAT32)(mag_cmplx_gain * vec_o_r[k]);
      ptr_hbe_txposer
          ->qmf_out_buf[qmf_col_idx * 2 + (k + HBE_ZERO_BAND_IDX - 1)]
                       [2 * qmf_band_idx + 1] +=
          (FLOAT32)(mag_cmplx_gain * vec_o_i[k]);
    }
  }
}

VOID ixheaacd_hbe_post_anal_prod2(ia_esbr_hbe_txposer_struct *ptr_hbe_txposer,
                                  WORD32 qmf_voc_columns, WORD32 qmf_band_idx) {
  WORD32 i;
  FLOAT32 *norm_ptr = &ptr_hbe_txposer->norm_qmf_in_buf[1][2 * qmf_band_idx];
  FLOAT32 *out_ptr = &ptr_hbe_txposer->qmf_out_buf[1][2 * qmf_band_idx];
  FLOAT32 *x_norm_ptr =
      &ptr_hbe_txposer->norm_qmf_in_buf[HBE_ZERO_BAND_IDX][2 * qmf_band_idx];

  ixheaacd_norm_qmf_in_buf_2(ptr_hbe_txposer, qmf_band_idx);

  for (; qmf_band_idx < ptr_hbe_txposer->x_over_qmf[1]; qmf_band_idx++) {
    for (i = 0; i < qmf_voc_columns; i++) {
      WORD32 k;
      FLOAT32 x_zero_band_r, x_zero_band_i;

      x_zero_band_r = *x_norm_ptr++;
      x_zero_band_i = *x_norm_ptr++;

      for (k = 0; k < HBE_OPER_BLK_LEN_2; k++) {
        register FLOAT32 tmp_r, tmp_i;
        tmp_r = *norm_ptr++;
        tmp_i = *norm_ptr++;

        *out_ptr++ +=
            ((tmp_r * x_zero_band_r - tmp_i * x_zero_band_i) * 0.3333333f);

        *out_ptr++ +=
            ((tmp_r * x_zero_band_i + tmp_i * x_zero_band_r) * 0.3333333f);

        norm_ptr += 126;
        out_ptr += 126;
      }

      norm_ptr -= 128 * 9;
      out_ptr -= 128 * 8;
      x_norm_ptr += 126;
    }
    out_ptr -= (128 * 2 * qmf_voc_columns) - 2;
    norm_ptr -= (128 * qmf_voc_columns) - 2;
    x_norm_ptr -= (128 * qmf_voc_columns) - 2;
  }
}

VOID ixheaacd_hbe_post_anal_prod3(ia_esbr_hbe_txposer_struct *ptr_hbe_txposer,
                                  WORD32 qmf_voc_columns, WORD32 qmf_band_idx) {
  WORD32 i, inp_band_idx, rem;

  FLOAT32 *out_buf = &ptr_hbe_txposer->qmf_out_buf[2][2 * qmf_band_idx];

  for (; qmf_band_idx < ptr_hbe_txposer->x_over_qmf[2]; qmf_band_idx++) {
    FLOAT32 temp_r, temp_i;
    FLOAT32 temp_r1, temp_i1;
    const FLOAT32 *ptr_sel, *ptr_sel1;

    inp_band_idx = (2 * qmf_band_idx) / 3;
    ptr_sel = &ixheaac_sel_case[(inp_band_idx + 1) & 3][0];
    ptr_sel1 = &ixheaac_sel_case[((inp_band_idx + 1) & 3) + 1][0];
    rem = 2 * qmf_band_idx - 3 * inp_band_idx;

    if (rem == 0 || rem == 1) {
      FLOAT32 *in_buf = &ptr_hbe_txposer->qmf_in_buf[0][2 * inp_band_idx];

      for (i = 0; i < qmf_voc_columns; i += 1) {
        WORD32 k;
        FLOAT32 vec_x[2 * HBE_OPER_WIN_LEN];
        FLOAT32 *ptr_vec_x = &vec_x[0];
        FLOAT32 x_zero_band_r, x_zero_band_i;

        FLOAT32 mag_scaling_fac;

        for (k = 0; k < (HBE_OPER_BLK_LEN_3); k += 2) {
          FLOAT64 base1;
          FLOAT64 base = 1e-17;

          temp_r = in_buf[0];
          temp_i = in_buf[1];

          in_buf += 256;

          base1 = base + temp_r * temp_r;
          base1 = base1 + temp_i * temp_i;

          mag_scaling_fac = (FLOAT32)(ixheaac_cbrt_calc((FLOAT32)base1));

          ptr_vec_x[0] = temp_r * mag_scaling_fac;
          ptr_vec_x[1] = temp_i * mag_scaling_fac;

          temp_r = in_buf[0];
          temp_i = in_buf[1];

          in_buf -= 128;

          temp_r1 = ptr_sel[0] * temp_r + ptr_sel[1] * temp_i;
          temp_i1 = ptr_sel[2] * temp_r + ptr_sel[3] * temp_i;

          temp_r = in_buf[0];
          temp_i = in_buf[1];

          temp_r1 += ptr_sel[4] * temp_r + ptr_sel[5] * temp_i;
          temp_i1 += ptr_sel[6] * temp_r + ptr_sel[7] * temp_i;

          temp_r1 *= 0.3984033437f;
          temp_i1 *= 0.3984033437f;

          base1 = base + temp_r1 * temp_r1;
          base1 = base1 + temp_i1 * temp_i1;
          mag_scaling_fac = (FLOAT32)(ixheaac_cbrt_calc((FLOAT32)base1));

          ptr_vec_x[2] = temp_r1 * mag_scaling_fac;
          ptr_vec_x[3] = temp_i1 * mag_scaling_fac;

          ptr_vec_x += 4;
          in_buf += 256;
        }
        ptr_vec_x = &vec_x[0];
        temp_r = vec_x[2 * (HBE_ZERO_BAND_IDX - 2)];
        temp_i = vec_x[(2 * (HBE_ZERO_BAND_IDX - 2)) + 1];

        x_zero_band_r = temp_r * temp_r - temp_i * temp_i;
        x_zero_band_i = temp_r * temp_i + temp_i * temp_r;

        for (k = 0; k < (HBE_OPER_BLK_LEN_3); k++) {
          temp_r = ptr_vec_x[0] * x_zero_band_r - ptr_vec_x[1] * x_zero_band_i;
          temp_i = ptr_vec_x[0] * x_zero_band_i + ptr_vec_x[1] * x_zero_band_r;

          out_buf[0] += (temp_r * 0.4714045f);
          out_buf[1] += (temp_i * 0.4714045f);

          ptr_vec_x += 2;
          out_buf += 128;
        }

        in_buf -= 128 * 11;
        out_buf -= 128 * 6;
      }
    } else {
      FLOAT32 *in_buf = &ptr_hbe_txposer->qmf_in_buf[0][2 * inp_band_idx];
      FLOAT32 *in_buf1 =
          &ptr_hbe_txposer->qmf_in_buf[0][2 * (inp_band_idx + 1)];

      for (i = 0; i < qmf_voc_columns; i++) {
        WORD32 k;
        FLOAT32 vec_x[2 * HBE_OPER_WIN_LEN];
        FLOAT32 vec_x_cap[2 * HBE_OPER_WIN_LEN];

        FLOAT32 x_zero_band_r, x_zero_band_i;
        FLOAT32 *ptr_vec_x = &vec_x[0];
        FLOAT32 *ptr_vec_x_cap = &vec_x_cap[0];

        FLOAT32 mag_scaling_fac;

        for (k = 0; k < (HBE_OPER_BLK_LEN_3); k += 2) {
          FLOAT32 tmp_vr, tmp_vi;
          FLOAT32 tmp_cr, tmp_ci;
          FLOAT64 base1;
          FLOAT64 base = 1e-17;

          temp_r1 = in_buf[0];
          temp_i1 = in_buf[1];
          temp_r = in_buf1[0];
          temp_i = in_buf1[1];

          base1 = base + temp_r * temp_r;
          base1 = base1 + temp_i * temp_i;

          mag_scaling_fac = (FLOAT32)(ixheaac_cbrt_calc((FLOAT32)base1));

          ptr_vec_x[0] = temp_r * mag_scaling_fac;
          ptr_vec_x[1] = temp_i * mag_scaling_fac;

          base1 = base + temp_r1 * temp_r1;
          base1 = base1 + temp_i1 * temp_i1;

          mag_scaling_fac = (FLOAT32)(ixheaac_cbrt_calc((FLOAT32)base1));

          ptr_vec_x_cap[0] = temp_r1 * mag_scaling_fac;
          ptr_vec_x_cap[1] = temp_i1 * mag_scaling_fac;

          in_buf += 256;

          temp_r = in_buf[0];
          temp_i = in_buf[1];

          temp_r1 = ptr_sel[0] * temp_r + ptr_sel[1] * temp_i;
          temp_i1 = ptr_sel[2] * temp_r + ptr_sel[3] * temp_i;

          in_buf -= 128;

          temp_r = in_buf[0];
          temp_i = in_buf[1];

          tmp_cr = temp_r1 + ptr_sel[4] * temp_r + ptr_sel[5] * temp_i;
          tmp_ci = temp_i1 + ptr_sel[6] * temp_r + ptr_sel[7] * temp_i;

          in_buf1 += 256;

          temp_r = in_buf1[0];
          temp_i = in_buf1[1];

          temp_r1 = ptr_sel1[0] * temp_r + ptr_sel1[1] * temp_i;
          temp_i1 = ptr_sel1[2] * temp_r + ptr_sel1[3] * temp_i;

          in_buf1 -= 128;

          temp_r = in_buf1[0];
          temp_i = in_buf1[1];

          tmp_vr = temp_r1 + ptr_sel1[4] * temp_r + ptr_sel1[5] * temp_i;
          tmp_vi = temp_i1 + ptr_sel1[6] * temp_r + ptr_sel1[7] * temp_i;

          tmp_cr *= 0.3984033437f;
          tmp_ci *= 0.3984033437f;

          tmp_vr *= 0.3984033437f;
          tmp_vi *= 0.3984033437f;

          base1 = base + tmp_vr * tmp_vr;
          base1 = base1 + tmp_vi * tmp_vi;

          mag_scaling_fac = (FLOAT32)(ixheaac_cbrt_calc((FLOAT32)base1));

          ptr_vec_x[2] = tmp_vr * mag_scaling_fac;
          ptr_vec_x[3] = tmp_vi * mag_scaling_fac;

          base1 = base + tmp_cr * tmp_cr;
          base1 = base1 + tmp_ci * tmp_ci;

          mag_scaling_fac = (FLOAT32)(ixheaac_cbrt_calc((FLOAT32)base1));

          ptr_vec_x_cap[2] = tmp_cr * mag_scaling_fac;
          ptr_vec_x_cap[3] = tmp_ci * mag_scaling_fac;

          in_buf += 256;
          in_buf1 += 256;
          ptr_vec_x += 4;
          ptr_vec_x_cap += 4;
        }
        ptr_vec_x = &vec_x[0];
        ptr_vec_x_cap = &vec_x_cap[0];

        temp_r = vec_x_cap[2 * (HBE_ZERO_BAND_IDX - 2)];
        temp_i = vec_x_cap[2 * (HBE_ZERO_BAND_IDX - 2) + 1];
        temp_r1 = vec_x[2 * (HBE_ZERO_BAND_IDX - 2)];
        temp_i1 = vec_x[2 * (HBE_ZERO_BAND_IDX - 2) + 1];

        x_zero_band_r = temp_r * temp_r - temp_i * temp_i;
        x_zero_band_i = temp_r * temp_i + temp_i * temp_r;

        temp_r = temp_r1 * temp_r1 - temp_i1 * temp_i1;
        temp_i = temp_r1 * temp_i1 + temp_i1 * temp_r1;

        for (k = 0; k < (HBE_OPER_BLK_LEN_3); k++) {
          temp_r1 = ptr_vec_x[0] * x_zero_band_r - ptr_vec_x[1] * x_zero_band_i;
          temp_i1 = ptr_vec_x[0] * x_zero_band_i + ptr_vec_x[1] * x_zero_band_r;

          temp_r1 += ptr_vec_x_cap[0] * temp_r - ptr_vec_x_cap[1] * temp_i;
          temp_i1 += ptr_vec_x_cap[0] * temp_i + ptr_vec_x_cap[1] * temp_r;

          out_buf[0] += (temp_r1 * 0.23570225f);
          out_buf[1] += (temp_i1 * 0.23570225f);

          out_buf += 128;
          ptr_vec_x += 2;
          ptr_vec_x_cap += 2;
        }

        in_buf -= 128 * 11;
        in_buf1 -= 128 * 11;
        out_buf -= 128 * 6;
      }
    }

    out_buf -= (256 * qmf_voc_columns) - 2;
  }
}

VOID ixheaacd_hbe_post_anal_prod4(ia_esbr_hbe_txposer_struct *ptr_hbe_txposer,
                                  WORD32 qmf_voc_columns, WORD32 qmf_band_idx) {
  WORD32 i, inp_band_idx;
  FLOAT32 *out_ptr = &ptr_hbe_txposer->qmf_out_buf[3][2 * qmf_band_idx];

  ixheaacd_norm_qmf_in_buf_4(ptr_hbe_txposer, ((qmf_band_idx >> 1) - 1));

  for (; qmf_band_idx < ptr_hbe_txposer->x_over_qmf[3]; qmf_band_idx++) {
    WORD32 ip_idx;
    FLOAT32 temp, temp_r, temp_i;
    FLOAT32 *norm_ptr, *x_norm_ptr;
    inp_band_idx = qmf_band_idx >> 1;
    ip_idx = (qmf_band_idx & 1) ? (inp_band_idx + 1) : (inp_band_idx - 1);

    norm_ptr = &ptr_hbe_txposer->norm_qmf_in_buf[0][2 * ip_idx];
    x_norm_ptr =
        &ptr_hbe_txposer->norm_qmf_in_buf[HBE_ZERO_BAND_IDX][2 * inp_band_idx];

    for (i = 0; i < qmf_voc_columns; i++) {
      WORD32 k;
      FLOAT32 x_zero_band_r, x_zero_band_i;

      temp_r = x_zero_band_r = *x_norm_ptr++;
      temp_i = x_zero_band_i = *x_norm_ptr++;

      temp = x_zero_band_r * x_zero_band_r - x_zero_band_i * x_zero_band_i;
      x_zero_band_i =
          x_zero_band_r * x_zero_band_i + x_zero_band_i * x_zero_band_r;

      x_zero_band_r = temp_r * temp - temp_i * x_zero_band_i;
      x_zero_band_i = temp_r * x_zero_band_i + temp_i * temp;

      for (k = 0; k < HBE_OPER_BLK_LEN_4; k++) {
        temp = *norm_ptr++;
        temp_i = *norm_ptr++;

        temp_r = temp * x_zero_band_r - temp_i * x_zero_band_i;
        temp_i = temp * x_zero_band_i + temp_i * x_zero_band_r;

        *out_ptr++ += (temp_r * 0.6666667f);
        *out_ptr++ += (temp_i * 0.6666667f);

        norm_ptr += 254;
        out_ptr += 126;
      }

      norm_ptr -= 128 * 11;
      out_ptr -= 128 * 4;
      x_norm_ptr += 126;
    }

    out_ptr -= (128 * 2 * qmf_voc_columns) - 2;
  }
}

VOID ixheaacd_hbe_post_anal_xprod2(ia_esbr_hbe_txposer_struct *ptr_hbe_txposer,
                                   WORD32 qmf_voc_columns, WORD32 qmf_band_idx,
                                   FLOAT32 p, FLOAT32 *cos_sin_theta) {
  WORD32 i;
  FLOAT32 *norm_ptr = &ptr_hbe_txposer->norm_qmf_in_buf[1][2 * qmf_band_idx];
  FLOAT32 *out_ptr = &ptr_hbe_txposer->qmf_out_buf[1][2 * qmf_band_idx];
  FLOAT32 *x_norm_ptr =
      &ptr_hbe_txposer->norm_qmf_in_buf[HBE_ZERO_BAND_IDX][2 * qmf_band_idx];

  ixheaacd_norm_qmf_in_buf_2(ptr_hbe_txposer, qmf_band_idx);

  for (; qmf_band_idx < ptr_hbe_txposer->x_over_qmf[1]; qmf_band_idx++) {
    WORD32 n1, n2;
    FLOAT64 temp_fac;
    FLOAT32 mag_cmplx_gain = 1.666666667f;
    temp_fac = (2.0 * qmf_band_idx + 1 - p) * 0.5;
    n1 = ((WORD32)(temp_fac)) << 1;
    n2 = ((WORD32)(temp_fac + p)) << 1;

    for (i = 0; i < qmf_voc_columns; i++) {
      WORD32 k;
      FLOAT32 x_zero_band_r, x_zero_band_i;

      x_zero_band_r = *x_norm_ptr++;
      x_zero_band_i = *x_norm_ptr++;

      for (k = 1; k < (HBE_OPER_BLK_LEN_2 + 1); k++) {
        register FLOAT32 tmp_r, tmp_i;
        tmp_r = *norm_ptr++;
        tmp_i = *norm_ptr++;

        *out_ptr++ +=
            ((tmp_r * x_zero_band_r - tmp_i * x_zero_band_i) * 0.3333333f);

        *out_ptr++ +=
            ((tmp_r * x_zero_band_i + tmp_i * x_zero_band_r) * 0.3333333f);

        norm_ptr += 126;
        out_ptr += 126;
      }
      norm_ptr -= 128 * 9;
      out_ptr -= 128 * 8;
      x_norm_ptr += 126;

      {
        WORD32 max_trans_fac, max_n1, max_n2;
        FLOAT32 max_mag_value;
        FLOAT32 mag_zero_band, mag_n1_band, mag_n2_band, temp;

        FLOAT32 *qmf_in_buf_ri =
            ptr_hbe_txposer->qmf_in_buf[i + HBE_ZERO_BAND_IDX];

        mag_zero_band =
            qmf_in_buf_ri[2 * qmf_band_idx] * qmf_in_buf_ri[2 * qmf_band_idx] +
            qmf_in_buf_ri[2 * qmf_band_idx + 1] *
                qmf_in_buf_ri[2 * qmf_band_idx + 1];

        mag_n1_band = qmf_in_buf_ri[n1] * qmf_in_buf_ri[n1] +
                      qmf_in_buf_ri[n1 + 1] * qmf_in_buf_ri[n1 + 1];
        mag_n2_band = qmf_in_buf_ri[n2] * qmf_in_buf_ri[n2] +
                      qmf_in_buf_ri[n2 + 1] * qmf_in_buf_ri[n2 + 1];

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
            max_n2 < TWICE_QMF_SYNTH_CHANNELS_NUM) {
          FLOAT32 vec_y_r[2], vec_y_i[2];
          FLOAT32 temp_r, temp_i, tmp_r1;
          WORD32 mid_trans_fac, idx;
          FLOAT64 base;
          WORD32 k;
          FLOAT32 mag_scaling_fac = 0.0f;
          FLOAT32 x_zero_band_r = 0;
          FLOAT32 x_zero_band_i = 0;

          mid_trans_fac = 2 - max_trans_fac;

          x_zero_band_r = qmf_in_buf_ri[max_n1];
          x_zero_band_i = qmf_in_buf_ri[max_n1 + 1];
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
            temp_r = ptr_hbe_txposer
                         ->qmf_in_buf[i + HBE_ZERO_BAND_IDX - 1 + k][max_n2];
            temp_i =
                ptr_hbe_txposer
                    ->qmf_in_buf[i + HBE_ZERO_BAND_IDX - 1 + k][max_n2 + 1];

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

          tmp_r1 =
              (FLOAT32)(cos_sin_theta[0] * temp_r - cos_sin_theta[1] * temp_i);
          temp_i =
              (FLOAT32)(cos_sin_theta[0] * temp_i + cos_sin_theta[1] * temp_r);

          ptr_hbe_txposer->qmf_out_buf[i * 2 + (HBE_ZERO_BAND_IDX - 1)]
                                      [2 * qmf_band_idx] +=
              (FLOAT32)(mag_cmplx_gain * tmp_r1);

          ptr_hbe_txposer->qmf_out_buf[i * 2 + (HBE_ZERO_BAND_IDX - 1)]
                                      [2 * qmf_band_idx + 1] +=
              (FLOAT32)(mag_cmplx_gain * temp_i);

          temp_r = vec_y_r[1] * x_zero_band_r - vec_y_i[1] * x_zero_band_i;
          temp_i = vec_y_r[1] * x_zero_band_i + vec_y_i[1] * x_zero_band_r;

          ptr_hbe_txposer->qmf_out_buf[i * 2 + (1 + HBE_ZERO_BAND_IDX - 1)]
                                      [2 * qmf_band_idx] +=
              (FLOAT32)(mag_cmplx_gain * temp_r);

          ptr_hbe_txposer->qmf_out_buf[i * 2 + (1 + HBE_ZERO_BAND_IDX - 1)]
                                      [2 * qmf_band_idx + 1] +=
              (FLOAT32)(mag_cmplx_gain * temp_i);
        }
      }
    }

    out_ptr -= (128 * 2 * qmf_voc_columns) - 2;
    norm_ptr -= (128 * qmf_voc_columns) - 2;
    x_norm_ptr -= (128 * qmf_voc_columns) - 2;
  }
}

VOID ixheaacd_hbe_post_anal_xprod3(ia_esbr_hbe_txposer_struct *ptr_hbe_txposer,
                                   WORD32 qmf_voc_columns, WORD32 qmf_band_idx,
                                   FLOAT32 p, WORD32 pitch_in_bins_idx) {
  WORD32 i, inp_band_idx, rem;

  FLOAT32 *out_buf = &ptr_hbe_txposer->qmf_out_buf[2][2 * qmf_band_idx];

  for (; qmf_band_idx < ptr_hbe_txposer->x_over_qmf[2]; qmf_band_idx++) {
    FLOAT32 temp_r, temp_i;
    FLOAT32 temp_r1, temp_i1;
    const FLOAT32 *ptr_sel, *ptr_sel1;

    inp_band_idx = (2 * qmf_band_idx) / 3;
    ptr_sel = &ixheaac_sel_case[(inp_band_idx + 1) & 3][0];
    ptr_sel1 = &ixheaac_sel_case[((inp_band_idx + 1) & 3) + 1][0];
    rem = 2 * qmf_band_idx - 3 * inp_band_idx;

    if (rem == 0 || rem == 1) {
      FLOAT32 *in_buf = &ptr_hbe_txposer->qmf_in_buf[0][2 * inp_band_idx];

      for (i = 0; i < qmf_voc_columns; i += 1) {
        WORD32 k;
        FLOAT32 vec_x[2 * HBE_OPER_WIN_LEN];
        FLOAT32 *ptr_vec_x = &vec_x[0];
        FLOAT32 x_zero_band_r, x_zero_band_i;

        FLOAT32 mag_scaling_fac;

        for (k = 0; k < (HBE_OPER_BLK_LEN_3); k += 2) {
          FLOAT64 base1;
          FLOAT64 base = 1e-17;

          temp_r = in_buf[0];
          temp_i = in_buf[1];

          in_buf += 256;

          base1 = base + temp_r * temp_r;
          base1 = base1 + temp_i * temp_i;

          mag_scaling_fac = (FLOAT32)(ixheaac_cbrt_calc((FLOAT32)base1));

          ptr_vec_x[0] = temp_r * mag_scaling_fac;
          ptr_vec_x[1] = temp_i * mag_scaling_fac;

          temp_r = in_buf[0];
          temp_i = in_buf[1];

          in_buf -= 128;

          temp_r1 = ptr_sel[0] * temp_r + ptr_sel[1] * temp_i;
          temp_i1 = ptr_sel[2] * temp_r + ptr_sel[3] * temp_i;

          temp_r = in_buf[0];
          temp_i = in_buf[1];

          temp_r1 += ptr_sel[4] * temp_r + ptr_sel[5] * temp_i;
          temp_i1 += ptr_sel[6] * temp_r + ptr_sel[7] * temp_i;

          temp_r1 *= 0.3984033437f;
          temp_i1 *= 0.3984033437f;

          base1 = base + temp_r1 * temp_r1;
          base1 = base1 + temp_i1 * temp_i1;
          mag_scaling_fac = (FLOAT32)(ixheaac_cbrt_calc((FLOAT32)base1));

          ptr_vec_x[2] = temp_r1 * mag_scaling_fac;
          ptr_vec_x[3] = temp_i1 * mag_scaling_fac;

          ptr_vec_x += 4;
          in_buf += 256;
        }
        ptr_vec_x = &vec_x[0];
        temp_r = vec_x[2 * (HBE_ZERO_BAND_IDX - 2)];
        temp_i = vec_x[(2 * (HBE_ZERO_BAND_IDX - 2)) + 1];

        x_zero_band_r = temp_r * temp_r - temp_i * temp_i;
        x_zero_band_i = temp_r * temp_i + temp_i * temp_r;

        for (k = 0; k < (HBE_OPER_BLK_LEN_3); k++) {
          temp_r = ptr_vec_x[0] * x_zero_band_r - ptr_vec_x[1] * x_zero_band_i;
          temp_i = ptr_vec_x[0] * x_zero_band_i + ptr_vec_x[1] * x_zero_band_r;

          out_buf[0] += (temp_r * 0.4714045f);
          out_buf[1] += (temp_i * 0.4714045f);

          ptr_vec_x += 2;
          out_buf += 128;
        }

        ixheaacd_hbe_xprod_proc_3(ptr_hbe_txposer, qmf_band_idx, i, p,
                                  pitch_in_bins_idx);

        in_buf -= 128 * 11;
        out_buf -= 128 * 6;
      }
    } else {
      FLOAT32 *in_buf = &ptr_hbe_txposer->qmf_in_buf[0][2 * inp_band_idx];
      FLOAT32 *in_buf1 =
          &ptr_hbe_txposer->qmf_in_buf[0][2 * (inp_band_idx + 1)];

      for (i = 0; i < qmf_voc_columns; i++) {
        WORD32 k;
        FLOAT32 vec_x[2 * HBE_OPER_WIN_LEN];
        FLOAT32 vec_x_cap[2 * HBE_OPER_WIN_LEN];

        FLOAT32 x_zero_band_r, x_zero_band_i;
        FLOAT32 *ptr_vec_x = &vec_x[0];
        FLOAT32 *ptr_vec_x_cap = &vec_x_cap[0];

        FLOAT32 mag_scaling_fac;

        for (k = 0; k < (HBE_OPER_BLK_LEN_3); k += 2) {
          FLOAT32 tmp_vr, tmp_vi;
          FLOAT32 tmp_cr, tmp_ci;
          FLOAT64 base1;
          FLOAT64 base = 1e-17;

          temp_r1 = in_buf[0];
          temp_i1 = in_buf[1];
          temp_r = in_buf1[0];
          temp_i = in_buf1[1];

          base1 = base + temp_r * temp_r;
          base1 = base1 + temp_i * temp_i;

          mag_scaling_fac = (FLOAT32)(ixheaac_cbrt_calc((FLOAT32)base1));

          ptr_vec_x[0] = temp_r * mag_scaling_fac;
          ptr_vec_x[1] = temp_i * mag_scaling_fac;

          base1 = base + temp_r1 * temp_r1;
          base1 = base1 + temp_i1 * temp_i1;

          mag_scaling_fac = (FLOAT32)(ixheaac_cbrt_calc((FLOAT32)base1));

          ptr_vec_x_cap[0] = temp_r1 * mag_scaling_fac;
          ptr_vec_x_cap[1] = temp_i1 * mag_scaling_fac;

          in_buf += 256;

          temp_r = in_buf[0];
          temp_i = in_buf[1];

          temp_r1 = ptr_sel[0] * temp_r + ptr_sel[1] * temp_i;
          temp_i1 = ptr_sel[2] * temp_r + ptr_sel[3] * temp_i;

          in_buf -= 128;

          temp_r = in_buf[0];
          temp_i = in_buf[1];

          tmp_cr = temp_r1 + ptr_sel[4] * temp_r + ptr_sel[5] * temp_i;
          tmp_ci = temp_i1 + ptr_sel[6] * temp_r + ptr_sel[7] * temp_i;

          in_buf1 += 256;

          temp_r = in_buf1[0];
          temp_i = in_buf1[1];

          temp_r1 = ptr_sel1[0] * temp_r + ptr_sel1[1] * temp_i;
          temp_i1 = ptr_sel1[2] * temp_r + ptr_sel1[3] * temp_i;

          in_buf1 -= 128;

          temp_r = in_buf1[0];
          temp_i = in_buf1[1];

          tmp_vr = temp_r1 + ptr_sel1[4] * temp_r + ptr_sel1[5] * temp_i;
          tmp_vi = temp_i1 + ptr_sel1[6] * temp_r + ptr_sel1[7] * temp_i;

          tmp_cr *= 0.3984033437f;
          tmp_ci *= 0.3984033437f;

          tmp_vr *= 0.3984033437f;
          tmp_vi *= 0.3984033437f;

          base1 = base + tmp_vr * tmp_vr;
          base1 = base1 + tmp_vi * tmp_vi;

          mag_scaling_fac = (FLOAT32)(ixheaac_cbrt_calc((FLOAT32)base1));

          ptr_vec_x[2] = tmp_vr * mag_scaling_fac;
          ptr_vec_x[3] = tmp_vi * mag_scaling_fac;

          base1 = base + tmp_cr * tmp_cr;
          base1 = base1 + tmp_ci * tmp_ci;

          mag_scaling_fac = (FLOAT32)(ixheaac_cbrt_calc((FLOAT32)base1));

          ptr_vec_x_cap[2] = tmp_cr * mag_scaling_fac;
          ptr_vec_x_cap[3] = tmp_ci * mag_scaling_fac;

          in_buf += 256;
          in_buf1 += 256;
          ptr_vec_x += 4;
          ptr_vec_x_cap += 4;
        }
        ptr_vec_x = &vec_x[0];
        ptr_vec_x_cap = &vec_x_cap[0];

        temp_r = vec_x_cap[2 * (HBE_ZERO_BAND_IDX - 2)];
        temp_i = vec_x_cap[2 * (HBE_ZERO_BAND_IDX - 2) + 1];
        temp_r1 = vec_x[2 * (HBE_ZERO_BAND_IDX - 2)];
        temp_i1 = vec_x[2 * (HBE_ZERO_BAND_IDX - 2) + 1];

        x_zero_band_r = temp_r * temp_r - temp_i * temp_i;
        x_zero_band_i = temp_r * temp_i + temp_i * temp_r;

        temp_r = temp_r1 * temp_r1 - temp_i1 * temp_i1;
        temp_i = temp_r1 * temp_i1 + temp_i1 * temp_r1;

        for (k = 0; k < (HBE_OPER_BLK_LEN_3); k++) {
          temp_r1 = ptr_vec_x[0] * x_zero_band_r - ptr_vec_x[1] * x_zero_band_i;
          temp_i1 = ptr_vec_x[0] * x_zero_band_i + ptr_vec_x[1] * x_zero_band_r;

          temp_r1 += ptr_vec_x_cap[0] * temp_r - ptr_vec_x_cap[1] * temp_i;
          temp_i1 += ptr_vec_x_cap[0] * temp_i + ptr_vec_x_cap[1] * temp_r;

          out_buf[0] += (temp_r1 * 0.23570225f);
          out_buf[1] += (temp_i1 * 0.23570225f);

          out_buf += 128;
          ptr_vec_x += 2;
          ptr_vec_x_cap += 2;
        }

        ixheaacd_hbe_xprod_proc_3(ptr_hbe_txposer, qmf_band_idx, i, p,
                                  pitch_in_bins_idx);

        in_buf -= 128 * 11;
        in_buf1 -= 128 * 11;
        out_buf -= 128 * 6;
      }
    }

    out_buf -= (256 * qmf_voc_columns) - 2;
  }
}

VOID ixheaacd_hbe_post_anal_xprod4(ia_esbr_hbe_txposer_struct *ptr_hbe_txposer,
                                   WORD32 qmf_voc_columns, WORD32 qmf_band_idx,
                                   FLOAT32 p, WORD32 pitch_in_bins_idx) {
  WORD32 i, inp_band_idx;
  FLOAT32 *out_ptr = &ptr_hbe_txposer->qmf_out_buf[3][2 * qmf_band_idx];

  ixheaacd_norm_qmf_in_buf_4(ptr_hbe_txposer, ((qmf_band_idx >> 1) - 1));

  for (; qmf_band_idx < ptr_hbe_txposer->x_over_qmf[3]; qmf_band_idx++) {
    WORD32 ip_idx;
    FLOAT32 temp, temp_r, temp_i;
    FLOAT32 *norm_ptr, *x_norm_ptr;
    inp_band_idx = qmf_band_idx >> 1;
    ip_idx = (qmf_band_idx & 1) ? (inp_band_idx + 1) : (inp_band_idx - 1);

    norm_ptr = &ptr_hbe_txposer->norm_qmf_in_buf[0][2 * ip_idx];
    x_norm_ptr =
        &ptr_hbe_txposer->norm_qmf_in_buf[HBE_ZERO_BAND_IDX][2 * inp_band_idx];

    for (i = 0; i < qmf_voc_columns; i++) {
      WORD32 k;
      FLOAT32 x_zero_band_r, x_zero_band_i;

      temp_r = x_zero_band_r = *x_norm_ptr++;
      temp_i = x_zero_band_i = *x_norm_ptr++;

      temp = x_zero_band_r * x_zero_band_r - x_zero_band_i * x_zero_band_i;
      x_zero_band_i =
          x_zero_band_r * x_zero_band_i + x_zero_band_i * x_zero_band_r;

      x_zero_band_r = temp_r * temp - temp_i * x_zero_band_i;
      x_zero_band_i = temp_r * x_zero_band_i + temp_i * temp;

      for (k = 0; k < HBE_OPER_BLK_LEN_4; k++) {
        temp = *norm_ptr++;
        temp_i = *norm_ptr++;

        temp_r = temp * x_zero_band_r - temp_i * x_zero_band_i;
        temp_i = temp * x_zero_band_i + temp_i * x_zero_band_r;

        *out_ptr++ += (temp_r * 0.6666667f);
        *out_ptr++ += (temp_i * 0.6666667f);

        norm_ptr += 254;
        out_ptr += 126;
      }

      norm_ptr -= 128 * 11;
      out_ptr -= 128 * 4;
      x_norm_ptr += 126;

      ixheaacd_hbe_xprod_proc_4(ptr_hbe_txposer, qmf_band_idx, i, p,
                                pitch_in_bins_idx);
    }

    out_ptr -= (128 * 2 * qmf_voc_columns) - 2;
  }
}

IA_ERRORCODE ixheaacd_hbe_post_anal_process(
    ia_esbr_hbe_txposer_struct *ptr_hbe_txposer, WORD32 pitch_in_bins,
    WORD32 sbr_upsamp_4_flg) {
  FLOAT32 p;
  WORD32 trans_fac;
  WORD32 qmf_voc_columns = ptr_hbe_txposer->no_bins / 2;
  FLOAT32 cos_sin_theta[2];

  p = (sbr_upsamp_4_flg) ? (FLOAT32)(pitch_in_bins * 0.04166666666666)
                         : (FLOAT32)(pitch_in_bins * 0.08333333333333);

  if (p < SBR_CONST_PMIN) {
    trans_fac = 2;
    if (trans_fac <= ptr_hbe_txposer->max_stretch)
      ixheaacd_hbe_post_anal_prod2(ptr_hbe_txposer, qmf_voc_columns,
                                   ptr_hbe_txposer->x_over_qmf[0]);

    trans_fac = 3;
    if (trans_fac <= ptr_hbe_txposer->max_stretch)
      ixheaacd_hbe_post_anal_prod3(ptr_hbe_txposer, qmf_voc_columns,
                                   ptr_hbe_txposer->x_over_qmf[1]);

    trans_fac = 4;
    if (trans_fac <= ptr_hbe_txposer->max_stretch) {
      if (ptr_hbe_txposer->x_over_qmf[2] <= 1) return IA_FATAL_ERROR;
      ixheaacd_hbe_post_anal_prod4(ptr_hbe_txposer, qmf_voc_columns,
                                   ptr_hbe_txposer->x_over_qmf[2]);
    }

  } else {
    trans_fac = 2;
    if (trans_fac <= ptr_hbe_txposer->max_stretch) {
      cos_sin_theta[0] = ixheaac_hbe_x_prod_cos_table_trans_2
          [((pitch_in_bins + sbr_upsamp_4_flg * 128) << 1) + 0];
      cos_sin_theta[1] = ixheaac_hbe_x_prod_cos_table_trans_2
          [((pitch_in_bins + sbr_upsamp_4_flg * 128) << 1) + 1];

      ixheaacd_hbe_post_anal_xprod2(ptr_hbe_txposer, qmf_voc_columns,
                                    ptr_hbe_txposer->x_over_qmf[0], p,
                                    cos_sin_theta);
    }

    trans_fac = 3;
    if (trans_fac <= ptr_hbe_txposer->max_stretch)
      ixheaacd_hbe_post_anal_xprod3(ptr_hbe_txposer, qmf_voc_columns,
                                    ptr_hbe_txposer->x_over_qmf[1], p,
                                    (pitch_in_bins + sbr_upsamp_4_flg * 128));

    trans_fac = 4;
    if (trans_fac <= ptr_hbe_txposer->max_stretch) {
      if (ptr_hbe_txposer->x_over_qmf[2] <= 1) return IA_FATAL_ERROR;
      ixheaacd_hbe_post_anal_xprod4(ptr_hbe_txposer, qmf_voc_columns,
                                    ptr_hbe_txposer->x_over_qmf[2], p,
                                    (pitch_in_bins + sbr_upsamp_4_flg * 128));
    }
  }
  return IA_NO_ERROR;
}
