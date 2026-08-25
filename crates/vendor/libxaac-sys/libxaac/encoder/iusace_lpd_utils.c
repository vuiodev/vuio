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
#include "ixheaace_adjust_threshold_data.h"
#include "iusace_bitbuffer.h"

/* DRC */
#include "impd_drc_common_enc.h"
#include "impd_drc_uni_drc.h"
#include "impd_drc_tables.h"
#include "impd_drc_api.h"
#include "impd_drc_uni_drc_eq.h"
#include "impd_drc_uni_drc_filter_bank.h"
#include "impd_drc_gain_enc.h"
#include "impd_drc_struct_def.h"

#include "iusace_cnst.h"
#include "iusace_tns_usac.h"
#include "iusace_psy_mod.h"
#include "iusace_fd_qc_util.h"
#include "iusace_tns_usac.h"
#include "iusace_config.h"
#include "iusace_arith_enc.h"
#include "iusace_fd_quant.h"
#include "iusace_block_switch_const.h"
#include "iusace_block_switch_struct_def.h"
#include "iusace_ms.h"
#include "iusace_signal_classifier.h"
#include "ixheaace_sbr_header.h"
#include "ixheaace_config.h"
#include "ixheaace_asc_write.h"
#include "iusace_main.h"
#include "iusace_func_prototypes.h"
#include "iusace_lpd_rom.h"
#include "ixheaace_common_utils.h"

WORD32 ia_get_sample_rate(WORD32 sample_rate) {
  if (92017 <= sample_rate) {
    return 11;
  }
  if (75132 <= sample_rate) {
    return 10;
  }
  if (55426 <= sample_rate) {
    return 9;
  }
  if (46009 <= sample_rate) {
    return 8;
  }
  if (37566 <= sample_rate) {
    return 7;
  }
  if (27713 <= sample_rate) {
    return 6;
  }
  if (23004 <= sample_rate) {
    return 5;
  }
  if (18783 <= sample_rate) {
    return 4;
  }
  if (13856 <= sample_rate) {
    return 3;
  }
  if (11502 <= sample_rate) {
    return 2;
  }
  if (9391 <= sample_rate) {
    return 1;
  }
  return 0;
}

VOID iusace_write_bits2buf(WORD32 value, WORD32 no_of_bits, WORD16 *bitstream) {
  WORD16 *pt_bitstream;
  WORD32 i;
  pt_bitstream = bitstream + no_of_bits;
  for (i = 0; i < no_of_bits; i++) {
    *--pt_bitstream = (WORD16)(value & MASK);
    value >>= 1;
  }
  return;
}

WORD32 iusace_get_num_params(WORD32 *qn) {
  return 2 + ((qn[0] > 0) ? 9 : 0) + ((qn[1] > 0) ? 9 : 0);
}

FLOAT32 iusace_cal_segsnr(FLOAT32 *sig1, FLOAT32 *sig2, WORD16 len, WORD16 nseg) {
  FLOAT32 snr = 0.0f;
  FLOAT32 signal, noise, error, fac;
  WORD16 i, j;
  for (i = 0; i < len; i += nseg) {
    signal = 1e-6f;
    noise = 1e-6f;
    for (j = 0; j < nseg; j++) {
      signal += (*sig1) * (*sig1);
      error = *sig1++ - *sig2++;
      noise += error * error;
    }
    snr += (FLOAT32)log10((FLOAT64)(signal / noise));
  }
  fac = ((FLOAT32)(10 * nseg)) / (FLOAT32)len;
  snr = fac * snr;
  if (snr < -99.0f) {
    snr = -99.0f;
  }
  return (snr);
}

VOID iusace_highpass_50hz_12k8(FLOAT32 *signal, WORD32 lg, FLOAT32 *mem, WORD32 fscale) {
  WORD32 i;
  WORD32 sr_idx = 0;
  FLOAT32 x0, x1, x2, y0, y1, y2;
  const FLOAT32 *a = NULL, *b = NULL;

  y1 = mem[0];
  y2 = mem[1];
  x0 = mem[2];
  x1 = mem[3];
  sr_idx = ia_get_sample_rate(fscale);
  a = &iusace_hp20_filter_coeffs[sr_idx][0];
  b = &iusace_hp20_filter_coeffs[sr_idx][2];

  for (i = 0; i < lg; i++) {
    x2 = x1;
    x1 = x0;
    x0 = signal[i];
    y0 = (y1 * a[0]) + (y2 * a[1]) + (x0 * b[1]) + (x1 * b[0]) + (x2 * b[1]);
    signal[i] = y0;
    y2 = y1;
    y1 = y0;
  }

  mem[0] = ((y1 > 1e-10) | (y1 < -1e-10)) ? y1 : 0;
  mem[1] = ((y2 > 1e-10) | (y2 < -1e-10)) ? y2 : 0;
  mem[2] = ((x0 > 1e-10) | (x0 < -1e-10)) ? x0 : 0;
  mem[3] = ((x1 > 1e-10) | (x1 < -1e-10)) ? x1 : 0;
}

VOID iusace_apply_preemph(FLOAT32 *signal, FLOAT32 factor, WORD32 length, FLOAT32 *mem) {
  WORD32 i;
  FLOAT32 temp;
  temp = signal[length - 1];
  for (i = length - 1; i > 0; i--) {
    signal[i] = signal[i] - factor * signal[i - 1];
  }
  signal[0] -= factor * (*mem);
  *mem = temp;
}

VOID iusace_apply_deemph(FLOAT32 *signal, FLOAT32 factor, WORD32 length, FLOAT32 *mem) {
  WORD32 i;
  signal[0] = signal[0] + factor * (*mem);
  for (i = 1; i < length; i++) {
    signal[i] = signal[i] + factor * signal[i - 1];
  }
  *mem = signal[length - 1];
  if ((*mem < 1e-10) & (*mem > -1e-10)) {
    *mem = 0;
  }
}

VOID iusace_synthesis_tool_float(FLOAT32 *a, FLOAT32 *x, FLOAT32 *y, WORD32 l, FLOAT32 *mem,
                                 FLOAT32 *scratch_synth_tool) {
  FLOAT32 s;
  FLOAT32 *yy;
  WORD32 i, j;
  memcpy(scratch_synth_tool, mem, ORDER * sizeof(FLOAT32));
  yy = &scratch_synth_tool[ORDER];
  for (i = 0; i < l; i++) {
    s = x[i];
    for (j = 1; j <= ORDER; j += 4) {
      s -= a[j] * yy[i - j];
      s -= a[j + 1] * yy[i - (j + 1)];
      s -= a[j + 2] * yy[i - (j + 2)];
      s -= a[j + 3] * yy[i - (j + 3)];
    }
    yy[i] = s;
    y[i] = s;
  }
}

VOID iusace_compute_lp_residual(FLOAT32 *a, FLOAT32 *x, FLOAT32 *y, WORD32 l) {
  FLOAT32 s;
  WORD32 i;
  for (i = 0; i < l; i++) {
    s = x[i];
    s += a[1] * x[i - 1];
    s += a[2] * x[i - 2];
    s += a[3] * x[i - 3];
    s += a[4] * x[i - 4];
    s += a[5] * x[i - 5];
    s += a[6] * x[i - 6];
    s += a[7] * x[i - 7];
    s += a[8] * x[i - 8];
    s += a[9] * x[i - 9];
    s += a[10] * x[i - 10];
    s += a[11] * x[i - 11];
    s += a[12] * x[i - 12];
    s += a[13] * x[i - 13];
    s += a[14] * x[i - 14];
    s += a[15] * x[i - 15];
    s += a[16] * x[i - 16];
    y[i] = s;
  }
}

VOID iusace_convolve(FLOAT32 *signal, FLOAT32 *wsynth_filter_ir, FLOAT32 *conv_out) {
  FLOAT32 temp;
  WORD32 i, n;
  for (n = 0; n < LEN_SUBFR; n += 2) {
    temp = 0.0f;
    for (i = 0; i <= n; i++) {
      temp += signal[i] * wsynth_filter_ir[n - i];
    }
    conv_out[n] = temp;
    temp = 0.0f;
    for (i = 0; i <= (n + 1); i += 2) {
      temp += signal[i] * wsynth_filter_ir[(n + 1) - i];
      temp += signal[i + 1] * wsynth_filter_ir[n - i];
    }
    conv_out[n + 1] = temp;
  }
}

VOID iusace_autocorr_plus(FLOAT32 *speech, FLOAT32 *auto_corr_vector, WORD32 window_len,
                          const FLOAT32 *lp_analysis_win, FLOAT32 *temp_aut_corr) {
  FLOAT32 val;
  WORD16 i, j;
  for (i = 0; i < window_len; i++) {
    temp_aut_corr[i] = speech[i] * lp_analysis_win[i];
  }
  for (i = 0; i <= ORDER; i++) {
    val = 0.0f;
    for (j = 0; j < window_len - i; j++) {
      val += temp_aut_corr[j] * temp_aut_corr[j + i];
    }
    auto_corr_vector[i] = val;
  }
  if (auto_corr_vector[0] < 1.0) {
    auto_corr_vector[0] = 1.0;
  }
}

static VOID iusace_get_norm_correlation(FLOAT32 *exc, FLOAT32 *xn, FLOAT32 *wsyn_filt_ir,
                                        WORD32 min_interval, WORD32 max_interval,
                                        FLOAT32 *norm_corr) {
  WORD32 i, j, k;
  FLOAT32 filt_prev_exc[LEN_SUBFR];
  FLOAT32 energy_filt_exc, corr, norm;
  k = -min_interval;

  iusace_convolve(&exc[k], wsyn_filt_ir, filt_prev_exc);

  for (i = min_interval; i <= max_interval; i++) {
    corr = 0.0F;
    energy_filt_exc = 0.01F;
    for (j = 0; j < LEN_SUBFR; j++) {
      corr += xn[j] * filt_prev_exc[j];
      energy_filt_exc += filt_prev_exc[j] * filt_prev_exc[j];
    }

    norm = (FLOAT32)(1.0f / sqrt(energy_filt_exc));
    norm_corr[i - min_interval] = corr * norm;

    if (i != max_interval) {
      k--;
      for (j = LEN_SUBFR - 1; j > 0; j--) {
        filt_prev_exc[j] = filt_prev_exc[j - 1] + exc[k] * wsyn_filt_ir[j];
      }
      filt_prev_exc[0] = exc[k];
    }
  }
}

static FLOAT32 iusace_corr_interpolate(FLOAT32 *x, WORD32 fraction) {
  FLOAT32 interpol_value, *x1, *x2;
  const FLOAT32 *p1_interp4_1_table, *p2_interp4_1_table;
  if (fraction < 0) {
    fraction += 4;
    x--;
  }
  x1 = &x[0];
  x2 = &x[1];
  p1_interp4_1_table = &iusace_interp4_1[fraction];
  p2_interp4_1_table = &iusace_interp4_1[4 - fraction];
  interpol_value = x1[0] * p1_interp4_1_table[0] + x2[0] * p2_interp4_1_table[0];
  interpol_value += x1[-1] * p1_interp4_1_table[4] + x2[1] * p2_interp4_1_table[4];
  interpol_value += x1[-2] * p1_interp4_1_table[8] + x2[2] * p2_interp4_1_table[8];
  interpol_value += x1[-3] * p1_interp4_1_table[12] + x2[3] * p2_interp4_1_table[12];

  return interpol_value;
}

VOID iusace_open_loop_search(FLOAT32 *wsp, WORD32 min_pitch_lag, WORD32 max_pitch_lag,
                             WORD32 num_frame, WORD32 *ol_pitch_lag,
                             ia_usac_td_encoder_struct *st) {
  WORD32 i, j, k;
  FLOAT32 r, corr, energy1, energy2, corr_max = -1.0e23f;
  const FLOAT32 *p1_ol_cw_table, *p2_ol_cw_table;
  FLOAT32 *data_a, *data_b, *hp_wsp, *p, *p1;

  p1_ol_cw_table = &iusace_ol_corr_weight[453];
  p2_ol_cw_table = &iusace_ol_corr_weight[259 + max_pitch_lag - st->prev_pitch_med];
  *ol_pitch_lag = 0;
  for (i = max_pitch_lag; i > min_pitch_lag; i--) {
    p = &wsp[0];
    p1 = &wsp[-i];
    corr = 0.0;
    for (j = 0; j < num_frame; j += 2) {
      corr += p[j] * p1[j];
      corr += p[j + 1] * p1[j + 1];
    }
    corr *= *p1_ol_cw_table--;
    if ((st->prev_pitch_med > 0) && (st->ol_wght_flg == 1)) {
      corr *= *p2_ol_cw_table--;
    }
    if (corr >= corr_max) {
      corr_max = corr;
      *ol_pitch_lag = i;
    }
  }
  data_a = st->hp_ol_ltp_mem;
  data_b = st->hp_ol_ltp_mem + HP_ORDER;
  hp_wsp = st->prev_hp_wsp + max_pitch_lag;
  for (k = 0; k < num_frame; k++) {
    data_b[0] = data_b[1];
    data_b[1] = data_b[2];
    data_b[2] = data_b[3];
    data_b[HP_ORDER] = wsp[k];
    r = data_b[0] * 0.83787057505665F;
    r += data_b[1] * -2.50975570071058F;
    r += data_b[2] * 2.50975570071058F;
    r += data_b[3] * -0.83787057505665F;
    r -= data_a[0] * -2.64436711600664F;
    r -= data_a[1] * 2.35087386625360F;
    r -= data_a[2] * -0.70001156927424F;
    data_a[2] = data_a[1];
    data_a[1] = data_a[0];
    data_a[0] = r;
    hp_wsp[k] = r;
  }
  p = &hp_wsp[0];
  p1 = &hp_wsp[-(*ol_pitch_lag)];
  corr = 0.0F;
  energy1 = 0.0F;
  energy2 = 0.0F;
  for (j = 0; j < num_frame; j++) {
    energy1 += p1[j] * p1[j];
    energy2 += p[j] * p[j];
    corr += p[j] * p1[j];
  }
  st->ol_gain = (FLOAT32)(corr / (sqrt(energy1 * energy2) + 1e-5));
  memmove(st->prev_hp_wsp, &st->prev_hp_wsp[num_frame], max_pitch_lag * sizeof(FLOAT32));
}

WORD32 iusace_get_ol_lag_median(WORD32 prev_ol_lag, WORD32 *prev_ol_lags) {
  WORD32 sorted_ol_lags_out[NUM_OPEN_LOOP_LAGS + 1] = {0};
  WORD32 i, j, idx, val;
  WORD32 num_lags = NUM_OPEN_LOOP_LAGS;
  for (i = NUM_OPEN_LOOP_LAGS - 1; i > 0; i--) {
    prev_ol_lags[i] = prev_ol_lags[i - 1];
  }
  prev_ol_lags[0] = prev_ol_lag;
  for (i = 0; i < NUM_OPEN_LOOP_LAGS; i++) {
    sorted_ol_lags_out[i + 1] = prev_ol_lags[i];
  }

  idx = (NUM_OPEN_LOOP_LAGS >> 1) + 1;
  for (;;) {
    if (idx > 1) {
      val = sorted_ol_lags_out[--idx];
    } else {
      val = sorted_ol_lags_out[num_lags];
      sorted_ol_lags_out[num_lags] = sorted_ol_lags_out[1];
      if (--num_lags == 1) {
        sorted_ol_lags_out[1] = val;
        break;
      }
    }
    i = idx;
    j = idx << 1;
    while (j <= num_lags) {
      if (j < num_lags && sorted_ol_lags_out[j] < sorted_ol_lags_out[j + 1]) {
        ++j;
      }
      if (val < sorted_ol_lags_out[j]) {
        sorted_ol_lags_out[i] = sorted_ol_lags_out[j];
        i = j;
        j *= 2;
      } else {
        j = num_lags + 1;
      }
    }
    sorted_ol_lags_out[i] = val;
  }

  return sorted_ol_lags_out[OPEN_LOOP_LAG_MEDIAN];
}

VOID iusace_closed_loop_search(FLOAT32 *exc, FLOAT32 *xn, FLOAT32 *wsyn_filt_ir,
                               WORD32 search_range_min, WORD32 search_range_max, WORD32 *pit_frac,
                               WORD32 is_first_subfrm, WORD32 min_pitch_lag_res1_2,
                               WORD32 min_pitch_lag_res_1, WORD32 *pitch_lag_out) {
  WORD32 i, fraction, step;
  FLOAT32 corr_vector[15 + 2 * LEN_INTERPOL1 + 1] = {0};
  FLOAT32 corr_max, temp;
  FLOAT32 *p_norm_corr_vector;
  WORD32 min_interval, max_interval;
  min_interval = search_range_min - LEN_INTERPOL1;
  max_interval = search_range_max + LEN_INTERPOL1;
  p_norm_corr_vector = &corr_vector[0];
  iusace_get_norm_correlation(exc, xn, wsyn_filt_ir, min_interval, max_interval,
                              p_norm_corr_vector);

  corr_max = p_norm_corr_vector[LEN_INTERPOL1];
  *pitch_lag_out = search_range_min;
  for (i = search_range_min + 1; i <= search_range_max; i++) {
    if (p_norm_corr_vector[i - search_range_min + LEN_INTERPOL1] > corr_max) {
      corr_max = p_norm_corr_vector[i - search_range_min + LEN_INTERPOL1];
      *pitch_lag_out = i;
    }
  }
  if ((is_first_subfrm == 0) && (*pitch_lag_out >= min_pitch_lag_res_1)) {
    *pit_frac = 0;
  } else {
    step = 1;
    fraction = -3;
    if (((is_first_subfrm == 0) && (*pitch_lag_out >= min_pitch_lag_res1_2)) ||
        (min_pitch_lag_res1_2 == TMIN)) {
      step = 2;
      fraction = -2;
    }
    if (*pitch_lag_out == search_range_min) {
      fraction = 0;
    }
    corr_max = iusace_corr_interpolate(
        &p_norm_corr_vector[(*pitch_lag_out) - search_range_min + LEN_INTERPOL1], fraction);
    for (i = (fraction + step); i <= 3; i += step) {
      temp = iusace_corr_interpolate(
          &p_norm_corr_vector[(*pitch_lag_out) - search_range_min + LEN_INTERPOL1], i);
      if (temp > corr_max) {
        corr_max = temp;
        fraction = i;
      }
    }
    if (fraction < 0) {
      fraction += 4;
      (*pitch_lag_out) -= 1;
    }
    *pit_frac = fraction;
  }
}

VOID iusace_decim2_fir_filter(FLOAT32 *signal, WORD32 length, FLOAT32 *mem,
                              FLOAT32 *scratch_fir_sig_buf) {
  FLOAT32 *sig_buf = scratch_fir_sig_buf;
  FLOAT32 temp;
  WORD32 i, j;
  memcpy(sig_buf, mem, DECIM2_FIR_FILT_MEM_SIZE * sizeof(FLOAT32));
  memcpy(sig_buf + DECIM2_FIR_FILT_MEM_SIZE, signal, length * sizeof(FLOAT32));
  for (i = 0; i < DECIM2_FIR_FILT_MEM_SIZE; i++) {
    mem[i] = ((signal[length - DECIM2_FIR_FILT_MEM_SIZE + i] > 1e-10) ||
              (signal[length - DECIM2_FIR_FILT_MEM_SIZE + i] < -1e-10))
                 ? signal[length - DECIM2_FIR_FILT_MEM_SIZE + i]
                 : 0;
  }
  for (i = 0, j = 0; i < length; i += 2, j++) {
    temp = sig_buf[i] * 0.13F;
    temp += sig_buf[i + 1] * 0.23F;
    temp += sig_buf[i + 2] * 0.28F;
#ifdef _WIN32
#pragma warning(suppress : 6385)
#endif
    temp += sig_buf[i + 3] * 0.23F;
    temp += sig_buf[i + 4] * 0.13F;
    signal[j] = temp;
  }
}

FLOAT32 iusace_calc_sq_gain(FLOAT32 *x, WORD32 num_bits, WORD32 length,
                            FLOAT32 *scratch_sq_gain_en) {
  WORD32 i, j, k;
  FLOAT32 gain, ener, temp, target, factor, offset;
  FLOAT32 *en = scratch_sq_gain_en;

  for (i = 0; i < length; i += 4) {
    ener = 0.01f;
    for (j = i; j < i + 4; j++) {
      ener += x[j] * x[j];
    }

    temp = (FLOAT32)log10(ener);
    en[i / 4] = 9.0f + 10.0f * temp;
  }

  target = (6.0f / 4.0f) * (FLOAT32)(num_bits - (length / 16));

  factor = 128.0f;
  offset = factor;

  for (k = 0; k < 10; k++) {
    factor *= 0.5f;
    offset -= factor;
    ener = 0.0f;
    for (i = 0; i < length / 4; i++) {
      temp = en[i] - offset;

      if (temp > 3.0f) {
        ener += temp;
      }
    }
    if (ener > target) {
      offset += factor;
    }
  }

  gain = (FLOAT32)pow(10.0f, offset / 20.0f);

  return (gain);
}

VOID iusace_lpc_coef_gen(FLOAT32 *lsf_old, FLOAT32 *lsf_new, FLOAT32 *a, WORD32 nb_subfr,
                         WORD32 m) {
  FLOAT32 lsf[ORDER] = {0}, *ptr_a;
  FLOAT32 inc, fnew, fold;
  WORD32 i = 0;

  ptr_a = a;

  inc = 1.0f / (FLOAT32)nb_subfr;
  fnew = 0.5f - (0.5f * inc);
  fold = 1.0f - fnew;
  for (i = 0; i < m; i++) {
    lsf[i] = (lsf_old[i] * fold) + (lsf_new[i] * fnew);
  }
  iusace_lsp_to_lp_conversion(lsf, ptr_a);
  ptr_a += (m + 1);
  iusace_lsp_to_lp_conversion(lsf_old, ptr_a);
  ptr_a += (m + 1);
  iusace_lsp_to_lp_conversion(lsf_new, ptr_a);

  return;
}

VOID iusace_interpolation_lsp_params(FLOAT32 *lsp_old, FLOAT32 *lsp_new, FLOAT32 *lp_flt_coff_a,
                                     WORD32 nb_subfr) {
  FLOAT32 lsp[ORDER];
  FLOAT32 factor;
  WORD32 i, k;
  FLOAT32 x_plus_y, x_minus_y;

  factor = 1.0f / (FLOAT32)nb_subfr;

  x_plus_y = 0.5f * factor;

  for (k = 0; k < nb_subfr; k++) {
    x_minus_y = 1.0f - x_plus_y;
    for (i = 0; i < ORDER; i++) {
      lsp[i] = (lsp_old[i] * x_minus_y) + (lsp_new[i] * x_plus_y);
    }
    x_plus_y += factor;

    iusace_lsp_to_lp_conversion(lsp, lp_flt_coff_a);

    lp_flt_coff_a += (ORDER + 1);
  }

  iusace_lsp_to_lp_conversion(lsp_new, lp_flt_coff_a);

  return;
}
