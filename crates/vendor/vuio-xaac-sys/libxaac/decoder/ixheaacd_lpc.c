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
#include <assert.h>
#include <float.h>
#include <stdlib.h>
#include <stdio.h>
#include <math.h>
#include <string.h>

#include "ixheaac_type_def.h"
#include "ixheaacd_bitbuffer.h"
#include "ixheaacd_interface.h"
#include "ixheaacd_defines.h"
#include "ixheaacd_aac_rom.h"
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

#include "ixheaacd_pulsedata.h"
#include "ixheaacd_pns.h"
#include "ixheaacd_lt_predict.h"
#include "ixheaacd_ec_defines.h"
#include "ixheaacd_ec_struct_def.h"
#include "ixheaacd_main.h"
#include "ixheaacd_channelinfo.h"
#include "ixheaacd_ec.h"
#include "ixheaacd_arith_dec.h"

#include "ixheaacd_func_def.h"
#include "ixheaacd_windows.h"
#include "ixheaacd_acelp_com.h"
#include "ixheaac_constants.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops40.h"

#define LSF_GAP_F 50.0f
#define FREQ_MAX_F 6400.0f
#define FREQ_DIV_F 400.0f

extern const FLOAT32 ixheaacd_fir_lp_filt[1 + FILTER_DELAY];

const WORD32 ixheaacd_pow_10_i_by_128[128] = {
    16384,     17788,     19312,     20968,     22765,     24716,     26835,
    29135,     31632,     34343,     37287,     40483,     43953,     47720,
    51810,     56251,     61072,     66307,     71990,     78161,     84860,
    92134,     100030,    108604,    117913,    128019,    138992,    150905,
    163840,    177882,    193129,    209682,    227654,    247167,    268352,
    291353,    316325,    343438,    372874,    404834,    439532,    477205,
    518107,    562515,    610728,    663075,    719908,    781612,    848605,
    921340,    1000309,   1086046,   1179133,   1280197,   1389925,   1509057,
    1638400,   1778829,   1931294,   2096827,   2276549,   2471675,   2683525,
    2913532,   3163255,   3434381,   3728745,   4048340,   4395328,   4772057,
    5181075,   5625151,   6107289,   6630752,   7199081,   7816122,   8486051,
    9213400,   10003091,  10860467,  11791330,  12801978,  13899250,  15090570,
    16384000,  17788290,  19312945,  20968279,  22765494,  24716750,  26835250,
    29135329,  31632551,  34343813,  37287459,  40483409,  43953287,  47720573,
    51810757,  56251515,  61072895,  66307521,  71990813,  78161226,  84860513,
    92134002,  100030911, 108604672, 117913300, 128019781, 138992500, 150905703,
    163840000, 177882909, 193129453, 209682794, 227654941, 247167501, 268352504,
    291353298, 316325515, 343438130, 372874596, 404834095, 439532879, 477205734,
    518107571, 562515151};

VOID ixheaacd_lsf_weight_2st_flt(float *lsfq, float *w, WORD32 mode);

static PLATFORM_INLINE WORD32 ixheaacd_mult32_m(WORD32 a, WORD32 b) {
  WORD32 result;
  WORD64 temp_result;

  temp_result = (WORD64)a * (WORD64)b;
  result = (WORD32)(temp_result >> 31);

  return (result);
}

void ixheaacd_reset_acelp_data_fix(ia_usac_data_struct *usac_data,
                                   ia_usac_lpd_decoder_handle st,
                                   WORD32 *ptr_overlap_buf,
                                   WORD32 was_last_short, WORD32 tw_mdct) {
  WORD32 i;

  if (was_last_short == 1) {
    st->mode_prev = -2;
  } else {
    st->mode_prev = -1;
  }

  for (i = 0; i < NUM_SUBFR_SUPERFRAME_BY2 - 1; i++) {
    st->pitch_prev[i] = 64;
    st->gain_prev[i] = 0;
  }

  st->bpf_active_prev = 0;

  if (ptr_overlap_buf != NULL && !tw_mdct) {
    const WORD32 *ptr_window_coeff;
    WORD32 fac_length;
    if (was_last_short) {
      fac_length = (usac_data->ccfl) / 16;
    } else {
      fac_length = (usac_data->len_subfrm) / 2;
    }

    if (fac_length == 48) {
      ptr_window_coeff = ixheaacd_sine_win_96;
    } else if (fac_length == 64) {
      ptr_window_coeff = ixheaacd_sine_win_128;
    } else if (fac_length == 96) {
      ptr_window_coeff = ixheaacd_sine_win_192;
    } else {
      ptr_window_coeff = ixheaacd_sine_win_256;
    }

    for (i = 0; i < 2 * fac_length; i++) {
      ptr_overlap_buf[(usac_data->ccfl) / 2 - fac_length + i] =
          ixheaacd_mult32_m(
              ptr_overlap_buf[(usac_data->ccfl) / 2 - fac_length + i],
              ptr_window_coeff[2 * fac_length - 1 - i]);
    }
    for (i = 0; i < (usac_data->ccfl) / 2 - fac_length; i++) {
      ptr_overlap_buf[(usac_data->ccfl) / 2 + fac_length + i] = 0;
    }

    if (ptr_overlap_buf != NULL) {
      for (i = 0; i < (usac_data->len_subfrm) / 2 - fac_length; i++) {
        st->exc_prev[i] = 0.0f;
      }
      for (i = 0; i < 2 * fac_length + 1; i++) {
        st->exc_prev[(usac_data->len_subfrm) / 2 - fac_length + i] =
            ptr_overlap_buf[i + usac_data->ccfl / 2 - fac_length - 1] /
            (float)(16384);
      }
    } else {
      ixheaacd_memset(st->exc_prev, 1 + (2 * FAC_LENGTH));
    }
  }

  return;
}

VOID ixheaacd_fix2flt_data(ia_usac_data_struct *usac_data,
                           ia_usac_lpd_decoder_handle st, WORD32 k) {
  WORD32 i;
  WORD32 fac_length;
  WORD32 window_sequence_last = usac_data->window_sequence_last[k];
  WORD32 *p_ola_buffer = usac_data->overlap_data_ptr[k];
  if (window_sequence_last == EIGHT_SHORT_SEQUENCE) {
    fac_length = (usac_data->ccfl) / 16;
  } else {
    fac_length = (usac_data->len_subfrm) / 2;
  }

  ixheaacd_memset(st->lp_flt_coeff_a_prev, 2 * (ORDER + 1));
  ixheaacd_memset(st->xcitation_prev, MAX_PITCH + INTER_LP_FIL_ORDER + 1);
  ixheaacd_memset(st->synth_prev, MAX_PITCH + SYNTH_DELAY_LMAX);
  ixheaacd_memset(st->bpf_prev, FILTER_DELAY + LEN_SUBFR);

  st->gain_threshold = 0.0f;

  if (p_ola_buffer != NULL) {
    for (i = 0; i < (usac_data->len_subfrm) / 2 - fac_length; i++) {
      st->exc_prev[i] = 0;
    }
    for (i = 0; i < 2 * fac_length + 1; i++) {
      st->exc_prev[(usac_data->len_subfrm) / 2 - fac_length + i] = (FLOAT32)(
          p_ola_buffer[i + usac_data->ccfl / 2 - fac_length - 1] / 16384.0);
    }
  } else {
    ixheaacd_memset(st->exc_prev, 1 + (2 * FAC_LENGTH));
  }

  return;
}

void ixheaacd_init_acelp_data(ia_usac_data_struct *usac_data,
                              ia_usac_lpd_decoder_handle st) {
  ixheaacd_reset_acelp_data_fix(usac_data, st, NULL, 0, 0);
}

#define PI_BY_6400 (PI / 6400.0)
#define SCALE1 (6400.0 / PI)

void ixheaacd_lsp_2_lsf_conversion(float lsp[], float lsf[], WORD32 m) {
  short i;
  for (i = 0; i < m; i++) {
    lsf[i] = (float)(acos(lsp[i]) * SCALE1);
  }
  return;
}

static VOID ixheaacd_lsf_2_lsp_conversion_float(FLOAT32 lsf[], FLOAT32 lsp[],
                                                WORD32 m) {
  WORD32 i;
  for (i = 0; i < m; i++)
    lsp[i] = (FLOAT32)cos((double)lsf[i] * (double)PI_BY_6400);

  return;
}

static WORD32 ixheaacd_bass_post_filter(FLOAT32 *synth_sig, WORD32 *pitch, FLOAT32 *pitch_gain,
                                        FLOAT32 *synth_out, WORD32 len_fr, WORD32 len2,
                                        FLOAT32 bpf_prev[], WORD32 ec_flag) {
  WORD32 i, j, sf, num_subfr, pitch_lag, lg;
  FLOAT32 x_energy, xy_corr, y_energy, norm_corr, energy, gain, tmp, alpha;
  FLOAT32 noise_buf[FILTER_DELAY + (2 * LEN_SUBFR)], *noise_tmp1, *noise_tmp2,
      *x, *y;

  noise_tmp1 = noise_buf + FILTER_DELAY;
  noise_tmp2 = noise_buf + FILTER_DELAY + LEN_SUBFR;

  memcpy(synth_out, synth_sig - LEN_SUBFR, len_fr * sizeof(FLOAT32));

  if (len_fr % 64)
    memset(synth_out + len_fr, 0, (LEN_SUBFR - len_fr % 64) * sizeof(FLOAT32));

  sf = 0;
  for (num_subfr = 0; num_subfr < len_fr; num_subfr += LEN_SUBFR, sf++) {
    pitch_lag = pitch[sf];
    gain = pitch_gain[sf];
    if (((pitch_lag >> 1) + 96 - num_subfr) > MAX_PITCH) {
      if (ec_flag) {
        pitch_lag = (MAX_PITCH + num_subfr - 96) << 1;
      } else {
        return -1;
      }
    }
    if (gain > 1.0f) gain = 1.0f;
    if (gain < 0.0f) gain = 0.0f;

    x = &synth_sig[num_subfr - 96];
    y = &synth_sig[num_subfr - pitch_lag / 2 - 96];

    x_energy = 0.01f;
    xy_corr = 0.01f;
    y_energy = 0.01f;
    for (i = 0; i < LEN_SUBFR + 96; i++) {
      x_energy += x[i] * x[i];
      xy_corr += x[i] * y[i];
      y_energy += y[i] * y[i];
    }

    norm_corr = xy_corr / (FLOAT32)sqrt(x_energy * y_energy);

    if (norm_corr > 0.95f) pitch_lag >>= 1;

    lg = len_fr + len2 - pitch_lag - num_subfr;
    if (lg < 0) lg = 0;
    if (lg > LEN_SUBFR) lg = LEN_SUBFR;

    if (pitch_lag > MAX_PITCH) {
      if (ec_flag) {
        pitch_lag = MAX_PITCH;
      } else {
        return -1;
      }
    }

    if (gain > 0) {
      if (lg > 0) {
        tmp = 0.01f;
        for (i = 0; i < lg; i++) {
          tmp += synth_sig[i + num_subfr] * synth_sig[i + num_subfr];
        }
        energy = 0.01f;
        for (i = 0; i < lg; i++) {
          energy += synth_sig[i + num_subfr + pitch_lag] *
                    synth_sig[i + num_subfr + pitch_lag];
        }
        tmp = (FLOAT32)sqrt(tmp / energy);
        if (tmp < gain) gain = tmp;
      }

      alpha = 0.5f * gain;
      for (i = 0; i < lg; i++) {
        noise_tmp2[i] = alpha * (synth_sig[i + num_subfr] -
                                 0.5f * synth_sig[i + num_subfr - pitch_lag] -
                                 0.5f * synth_sig[i + num_subfr + pitch_lag]);
      }
      for (i = lg; i < LEN_SUBFR; i++) {
        noise_tmp2[i] = alpha * (synth_sig[i + num_subfr] -
                                 synth_sig[i + num_subfr - pitch_lag]);
      }
    } else {
      memset(noise_tmp2, 0, LEN_SUBFR * sizeof(FLOAT32));
    }

    memcpy(noise_buf, bpf_prev, (FILTER_DELAY + LEN_SUBFR) * sizeof(FLOAT32));
    memcpy(bpf_prev, noise_buf + LEN_SUBFR,
           (FILTER_DELAY + LEN_SUBFR) * sizeof(FLOAT32));

    for (i = 0; i < LEN_SUBFR; i++) {
      tmp = ixheaacd_fir_lp_filt[0] * noise_tmp1[i];
      for (j = 1; j <= FILTER_DELAY; j++) {
        tmp +=
            ixheaacd_fir_lp_filt[j] * (noise_tmp1[i - j] + noise_tmp1[i + j]);
      }
      synth_out[i + num_subfr] -= tmp;
    }
  }

  return 0;
}

void ixheaacd_reorder_lsf(float *lsf, float min_dist, int n) {
  int i;
  float lsf_min;

  lsf_min = min_dist;
  for (i = 0; i < n; i++) {
    if (lsf[i] < lsf_min) lsf[i] = lsf_min;

    lsf_min = lsf[i] + min_dist;
  }

  lsf_min = FREQ_MAX_F - min_dist;
  for (i = n - 1; i >= 0; i--) {
    if (lsf[i] > lsf_min) lsf[i] = lsf_min;

    lsf_min = lsf[i] - min_dist;
  }

  return;
}

WORD32 ixheaacd_lpd_dec(ia_usac_data_struct *usac_data,
                        ia_usac_lpd_decoder_handle st,
                        ia_td_frame_data_struct *pstr_td_frame_data,
                        FLOAT32 fsynth[], WORD32 first_lpd_flag,
                        WORD32 short_fac_flag, WORD32 bpf_control_info) {
  FLOAT32 *synth_buf = usac_data->synth_buf;
  FLOAT32 *xcitation_buff = usac_data->exc_buf;
  FLOAT32 lsp_curr[ORDER];
  FLOAT32 lsf_curr[ORDER];
  FLOAT32 *lp_flt_coff_a = usac_data->lp_flt_coff;
  FLOAT32 *synth, *xcitation_curr;
  WORD32 *pitch = usac_data->pitch;
  FLOAT32 *pitch_gain = usac_data->pitch_gain;
  FLOAT32 lsf_flt[(2 * NUM_FRAMES + 1) * ORDER];

  WORD32 i, k, tp, mode;
  WORD32 *mod;
  FLOAT32 gain, stability_factor = 0.0f;
  FLOAT32 tmp, synth_corr, synth_energy;

  WORD32 len_fr;
  WORD32 len_subfrm;
  WORD32 num_subfr;
  WORD32 num_subfr_in_superfr;
  WORD32 num_subfr_by2;
  WORD32 synth_delay;
  WORD32 num_samples = 0;

  WORD32 *ptr_scratch = &usac_data->scratch_buffer[0];

  WORD32 subfr_len = 0, n_subfr = 0;
  WORD32 err = 0;
  WORD32 ch = usac_data->present_chan;

  len_fr = usac_data->ccfl;
  len_subfrm = usac_data->len_subfrm;
  num_subfr = usac_data->num_subfrm;
  num_subfr_in_superfr = NUM_FRAMES * num_subfr;
  num_subfr_by2 = (num_subfr_in_superfr / 2) - 1;
  synth_delay = num_subfr_by2 * LEN_SUBFR;

  synth = synth_buf + MAX_PITCH + synth_delay;
  ixheaacd_mem_cpy(st->synth_prev, synth_buf, MAX_PITCH + synth_delay);
  ixheaacd_memset(synth, SYNTH_DELAY_LMAX + LEN_SUPERFRAME - synth_delay);

  xcitation_curr = xcitation_buff + MAX_PITCH + INTER_LP_FIL_ORDER + 1;
  ixheaacd_mem_cpy(st->xcitation_prev, xcitation_buff,
                   MAX_PITCH + INTER_LP_FIL_ORDER + 1);
  memset(xcitation_curr, 0, sizeof(FLOAT32) * (LEN_SUPERFRAME + 1));

  mod = pstr_td_frame_data->mod;
  if (usac_data->frame_ok == 1) {
    usac_data->num_lost_lpd_frames[usac_data->present_chan] = 0;
  }

  if (usac_data->ec_flag && usac_data->frame_ok == 0) {
    ixheaacd_usac_lpc_ec(usac_data->lsp_coeff, usac_data->lpc4_lsf, usac_data->lsf_adaptive_mean,
                         first_lpd_flag);
  }
  for (i = 0; i < num_subfr_by2; i++) {
    pitch[i] = st->pitch_prev[i];
    pitch_gain[i] = st->gain_prev[i];
  }
  for (i = 0; i < num_subfr_in_superfr; i++) {
    pitch[i + num_subfr_by2] = 64;
    pitch_gain[i + num_subfr_by2] = 0.0f;
  }

  if (usac_data->frame_ok) {
    if (!first_lpd_flag) {
      ixheaacd_lsp_2_lsf_conversion(st->lspold, lsf_flt, ORDER);
    }

    ixheaacd_alg_vec_dequant(pstr_td_frame_data, first_lpd_flag, lsf_flt, pstr_td_frame_data->mod,
                             usac_data->ec_flag);
  }
  if (usac_data->ec_flag && !(usac_data->frame_ok)) {
    for (i = 0; i < 5; i++) {
      memcpy(&lsf_flt[i * ORDER], &usac_data->lsp_coeff[i], ORDER * sizeof(lsf_flt[0]));
    }
  }
  if (first_lpd_flag) {
    ixheaacd_mem_cpy(&lsf_flt[0], st->lsf_prev, ORDER);
    ixheaacd_lsf_2_lsp_conversion_float(st->lsf_prev, st->lspold, ORDER);
  }

  if ((first_lpd_flag && mod[0] == 0) || (first_lpd_flag && mod[1] == 0) ||
      ((first_lpd_flag && mod[2] == 0 && len_subfrm != LEN_FRAME))) {
    FLOAT32 lp_flt_coeff_a[9 * (ORDER + 1)];
    FLOAT32 tmp_buf[3 * LEN_FRAME + ORDER];
    FLOAT32 tmp_res_buf[3 * LEN_FRAME];
    FLOAT32 *tmp = &(tmp_buf[LEN_FRAME]);
    FLOAT32 *ptr_tmp = &(tmp_res_buf[LEN_FRAME]);
    WORD32 tmp_start;
    FLOAT32 mem = 0;
    WORD32 gain;
    WORD32 length;

    ixheaacd_interpolation_lsp_params(st->lspold, st->lspold, lp_flt_coeff_a,
                                      8);

    memcpy(st->lp_flt_coeff_a_prev, lp_flt_coeff_a,
           (ORDER + 1) * sizeof(FLOAT32));
    memcpy(st->lp_flt_coeff_a_prev + ORDER + 1, lp_flt_coeff_a,
           (ORDER + 1) * sizeof(FLOAT32));

    if (mod[0] == 0) {
      WORD32 fac_length;
      if (short_fac_flag) {
        fac_length = (len_subfrm * NUM_FRAMES) / 16;
      } else {
        fac_length = len_subfrm / 2;
      }

      if (usac_data->frame_ok == 0) {
        memset(&pstr_td_frame_data->fac_data[0], 0, sizeof(pstr_td_frame_data->fac_data));
      }
      gain = ixheaacd_pow_10_i_by_128[pstr_td_frame_data->fac_data[0]];

      memcpy(ptr_scratch, &pstr_td_frame_data->fac_data[0],
             129 * sizeof(WORD32));

      for (i = 0; i < fac_length / 2; i++) {
        pstr_td_frame_data->fac_data[i] = ptr_scratch[2 * i + 1] << 16;
        pstr_td_frame_data->fac_data[fac_length / 2 + i] =
            ptr_scratch[fac_length - 2 * i] << 16;
      }

      if (usac_data->ec_flag == 0) {
        if (fac_length & (fac_length - 1)) {
          if ((fac_length != 48) && (fac_length != 96) && (fac_length != 192) &&
              (fac_length != 384) && (fac_length != 768)) {
            return -1;
          }
        }
      }

      ixheaacd_fwd_alias_cancel_tool(usac_data, pstr_td_frame_data, fac_length, lp_flt_coeff_a,
                                     gain);

      memset(&usac_data->overlap_data_ptr[usac_data->present_chan][(len_fr / 2)], 0,
             fac_length * sizeof(WORD32));
    }

    for (i = 0; i < 2 * len_subfrm; i++)
      st->fd_synth[ORDER + i] = (FLOAT32)(
          (FLOAT32)usac_data->overlap_data_ptr[usac_data->present_chan][i] /
          16384.0);
    num_samples = min(2 * len_subfrm, MAX_PITCH + synth_delay);

    ixheaacd_mem_cpy(st->fd_synth + ORDER, synth - 2 * len_subfrm,
                     2 * len_subfrm);

    ixheaacd_preemphsis_tool_float(st->fd_synth + ORDER, PREEMPH_FILT_FAC,
                                   2 * len_subfrm, mem);

    ixheaacd_memset(tmp, ORDER);
    ixheaacd_mem_cpy(st->fd_synth + ORDER, tmp + ORDER, 2 * len_subfrm);
    tmp_start = 0;

    ixheaacd_memset(ptr_tmp - len_subfrm, 3 * len_subfrm);
    memset(st->fd_synth, 0, ORDER * sizeof(WORD32));
    length = (2 * len_subfrm - tmp_start) / LEN_SUBFR;

    ixheaacd_residual_tool_float1(lp_flt_coeff_a,
                                  &st->fd_synth[ORDER + tmp_start],
                                  &ptr_tmp[tmp_start], LEN_SUBFR, length);

    if (mod[0] != 0 && (len_subfrm == LEN_FRAME || mod[1] != 0)) {
      num_samples = min(len_subfrm, MAX_PITCH + INTER_LP_FIL_ORDER + 1);
    } else {
      num_samples = min(2 * len_subfrm, MAX_PITCH + INTER_LP_FIL_ORDER + 1);
    }
    ixheaacd_mem_cpy(ptr_tmp + 2 * len_subfrm - num_samples,
                     xcitation_curr - num_samples, num_samples);
  }

  k = 0;

  while (k < 4) {
    if (usac_data->ec_flag && usac_data->frame_ok == 0) {
      if (mod[k] != 0 && usac_data->frame_ok == 0 &&
          usac_data->str_error_concealment[ch].prev_frame_ok[0] == 0 && k == 0) {
        memcpy(st->lspold, usac_data->lspold_ec, sizeof(st->lspold));
      }
      usac_data->num_lost_lpd_frames[usac_data->present_chan]++;
    }

    mode = mod[k];
    if ((st->mode_prev == 0) && (mode > 0) &&
        (k != 0 || st->bpf_active_prev == 1)) {
      i = (k * num_subfr) + num_subfr_by2;
      pitch[i + 1] = pitch[i] = pitch[i - 1];
      pitch_gain[i + 1] = pitch_gain[i] = pitch_gain[i - 1];
    }
    if (usac_data->frame_ok == 0) {
      memcpy(lsf_curr, &lsf_flt[(k + 1) * ORDER], ORDER * sizeof(FLOAT32));
    } else {
      if ((mode == 0) || (mode == 1))
        memcpy(lsf_curr, &lsf_flt[(k + 1) * ORDER], ORDER * sizeof(FLOAT32));
      else if (mode == 2)
        memcpy(lsf_curr, &lsf_flt[(k + 2) * ORDER], ORDER * sizeof(FLOAT32));
      else
        memcpy(lsf_curr, &lsf_flt[(k + 4) * ORDER], ORDER * sizeof(FLOAT32));
    }

    ixheaacd_lsf_2_lsp_conversion_float(lsf_curr, lsp_curr, ORDER);
    if (usac_data->frame_ok) {
      tmp = 0.0f;
      for (i = 0; i < ORDER; i++) {
        tmp += (lsf_curr[i] - st->lsf_prev[i]) * (lsf_curr[i] - st->lsf_prev[i]);
      }
      stability_factor = (FLOAT32)(1.25f - (tmp / 400000.0f));
      if (stability_factor > 1.0f) {
        stability_factor = 1.0f;
      }
      if (stability_factor < 0.0f) {
        stability_factor = 0.0f;
      }
      if (usac_data->ec_flag) {
        usac_data->stability_factor_old = stability_factor;
      }
    }
    if (usac_data->ec_flag && !(usac_data->frame_ok)) {
      stability_factor = usac_data->stability_factor_old;
    }
    if (usac_data->frame_ok == 0) {
      mode = st->mode_prev;
    }
    if ((usac_data->frame_ok == 1 && mode == 0) ||
        (usac_data->frame_ok == 0 && (st->mode_prev == 0 || st->mode_prev == 1))) {
      ixheaacd_interpolation_lsp_params(st->lspold, lsp_curr, lp_flt_coff_a, num_subfr);

      if (usac_data->frame_ok == 1 || (usac_data->frame_ok == 0 && st->mode_prev == 0)) {
        ixheaacd_acelp_alias_cnx(usac_data, pstr_td_frame_data, k, lp_flt_coff_a,
                                 stability_factor, st);
      }

      if (usac_data->frame_ok == 0 && st->mode_prev == 1) {
        ixheaacd_usac_tcx_ec(usac_data, st, lsp_curr, k, lp_flt_coff_a);
      }

      if ((st->mode_prev != 0) && bpf_control_info) {
        i = (k * num_subfr) + num_subfr_by2;
        pitch[i - 1] = pitch[i];
        pitch_gain[i - 1] = pitch_gain[i];
        if (st->mode_prev != -2) {
          pitch[i - 2] = pitch[i];
          pitch_gain[i - 2] = pitch_gain[i];
        }
      }
      k++;
    } else {
      if (mode == 1) {
        subfr_len = len_subfrm;
        n_subfr = num_subfr;
      } else if (mode == 2) {
        subfr_len = len_subfrm << 1;
        n_subfr = num_subfr_in_superfr / 2;
      } else if (mode == 3) {
        subfr_len = len_subfrm << 2;
        n_subfr = num_subfr_in_superfr;
      } else {
        if (usac_data->frame_ok == 0) {
          mode = 3;
          subfr_len = len_subfrm << 2;
          n_subfr = num_subfr_in_superfr;
        }
      }

      ixheaacd_lpc_coef_gen(st->lspold, lsp_curr, lp_flt_coff_a, n_subfr,
                            ORDER);

      err = ixheaacd_tcx_mdct(usac_data, pstr_td_frame_data, k, lp_flt_coff_a,
                              subfr_len, st);
      if (err) return err;
      if (usac_data->frame_ok == 1 && k == 2) {
        memcpy(usac_data->lp_flt_coff_a_ec, &lp_flt_coff_a[k * (ORDER + 1)],
               sizeof(usac_data->lp_flt_coff_a_ec));
      }
      k += (1 << (mode - 1));
    }

    st->mode_prev = mode;
    if (usac_data->frame_ok == 0) {
      memcpy(usac_data->lspold_ec, st->lspold, sizeof(st->lspold));
    }
    ixheaacd_mem_cpy(lsp_curr, st->lspold, ORDER);
    ixheaacd_mem_cpy(lsf_curr, st->lsf_prev, ORDER);
  }

  ixheaacd_mem_cpy(xcitation_buff + len_fr, st->xcitation_prev,
                   MAX_PITCH + INTER_LP_FIL_ORDER + 1);

  ixheaacd_mem_cpy(synth_buf + len_fr, st->synth_prev, MAX_PITCH + synth_delay);

  if (!bpf_control_info) {
    if (mod[0] != 0 && st->bpf_active_prev) {
      for (i = 2; i < num_subfr_in_superfr; i++)
        pitch_gain[num_subfr_by2 + i] = 0.0;
    } else {
      for (i = 0; i < num_subfr_in_superfr; i++)
        pitch_gain[num_subfr_by2 + i] = 0.0;
    }
  }
  st->bpf_active_prev = bpf_control_info;

  for (i = 0; i < num_subfr_by2; i++) {
    st->pitch_prev[i] = pitch[num_subfr_in_superfr + i];
    st->gain_prev[i] = pitch_gain[num_subfr_in_superfr + i];
  }

  synth = synth_buf + MAX_PITCH;

  for (i = 0; i < num_subfr_in_superfr; i++) {
    tp = pitch[i];
    gain = pitch_gain[i];
    if (gain > 0.0f) {
      synth_corr = 0.0f, synth_energy = 1e-6f;
      if ((((i * LEN_SUBFR) + LEN_SUBFR) > LEN_SUPERFRAME) ||
          ((((i * LEN_SUBFR) + LEN_SUBFR) - tp) > LEN_SUPERFRAME)) {
        if (usac_data->ec_flag) {
          tp = LEN_SUPERFRAME - LEN_SUBFR - (i * LEN_SUBFR);
        } else
          return -1;
      }
      for (k = 0; k < LEN_SUBFR; k++) {
        synth_corr +=
            synth[i * LEN_SUBFR + k] * synth[(i * LEN_SUBFR) - tp + k];
        synth_energy +=
            synth[(i * LEN_SUBFR) - tp + k] * synth[(i * LEN_SUBFR) - tp + k];
      }
      pitch_gain[i] = synth_corr / synth_energy;
    }
  }

  if (mod[3] == 0) {
    err = ixheaacd_bass_post_filter(synth, pitch, pitch_gain, fsynth, len_fr, synth_delay,
                                    st->bpf_prev, usac_data->ec_flag);
  } else {
    err = ixheaacd_bass_post_filter(synth, pitch, pitch_gain, fsynth, len_fr,
                                    synth_delay - (len_subfrm / 2), st->bpf_prev,
                                    usac_data->ec_flag);
  }
  if (err) return err;
  if (usac_data->ec_flag && usac_data->frame_ok) {
    memcpy(usac_data->lpc4_lsf, pstr_td_frame_data->lpc4_lsf, sizeof(usac_data->lpc4_lsf));
    memcpy(usac_data->str_error_concealment[ch].lsf4, usac_data->lpc4_lsf,
           sizeof(usac_data->lpc4_lsf));
    memcpy(usac_data->lsf_adaptive_mean, pstr_td_frame_data->lsf_adaptive_mean_cand,
           sizeof(usac_data->lsf_adaptive_mean));
  }
  return err;
}

VOID ixheaacd_lpd_dec_update(ia_usac_lpd_decoder_handle tddec,
                             ia_usac_data_struct *usac_data, WORD32 i_ch) {
  WORD32 i, k;

  WORD32 *ptr_overlap = &usac_data->overlap_data_ptr[i_ch][0];
  WORD32 len_fr, lpd_sbf_len, lpd_delay, num_subfr_by2, synth_delay, fac_length;

  if (usac_data->tw_mdct[0])
    ptr_overlap = &usac_data->overlap_data_ptr[i_ch][usac_data->ccfl / 2];

  len_fr = usac_data->ccfl;
  lpd_sbf_len = (NUM_FRAMES * usac_data->num_subfrm) / 2;
  lpd_delay = lpd_sbf_len * LEN_SUBFR;
  num_subfr_by2 = lpd_sbf_len - 1;
  synth_delay = num_subfr_by2 * LEN_SUBFR;
  fac_length = (usac_data->len_subfrm) / 2;

  for (i = 0; i < LEN_SUBFR + synth_delay; i++)
    ptr_overlap[i] = (WORD32)(
        (FLOAT32)tddec->synth_prev[MAX_PITCH - (LEN_SUBFR) + i] * 16384.0);

  ptr_overlap += LEN_SUBFR + synth_delay - fac_length;

  for (k = 0; k < 2 * fac_length; k++)
    ptr_overlap[k] = (WORD32)((FLOAT32)tddec->exc_prev[k + 1] * 16384.0);

  ptr_overlap = &usac_data->overlap_data_ptr[i_ch][lpd_delay + fac_length];

  for (i = 0; i < len_fr - lpd_delay - fac_length; i++) ptr_overlap[i] = 0;

  usac_data->window_shape[i_ch] = WIN_SEL_0;
  usac_data->window_sequence_last[i_ch] = EIGHT_SHORT_SEQUENCE;
  usac_data->td_frame_prev[i_ch] = 1;

  if (tddec->mode_prev == 0) {
    memmove(usac_data->lpc_prev[i_ch], &tddec->lp_flt_coeff_a_prev[ORDER + 1],
            (ORDER + 1) * sizeof(FLOAT32));
    memmove(usac_data->acelp_in[i_ch], tddec->exc_prev,
            (1 + (2 * FAC_LENGTH)) * sizeof(FLOAT32));
  }

  return;
}

WORD32 ixheaacd_lpd_bpf_fix(ia_usac_data_struct *usac_data,
                            WORD32 is_short_flag, FLOAT32 out_buffer[],
                            ia_usac_lpd_decoder_handle st) {
  WORD32 i, tp, k;
  float synth_buf[MAX_PITCH + SYNTH_DELAY_LMAX + LEN_SUPERFRAME];
  float signal_out[LEN_SUPERFRAME];
  float *synth, synth_corr, synth_energy;
  WORD32 pitch[NUM_SUBFR_SUPERFRAME_BY2 + 3];
  float pitch_gain[NUM_SUBFR_SUPERFRAME_BY2 + 3];
  WORD32 len_fr, lpd_sbf_len, num_subfr_by2, synth_delay;
  WORD32 err = 0;

  len_fr = usac_data->ccfl;
  lpd_sbf_len = (NUM_FRAMES * usac_data->num_subfrm) / 2;
  num_subfr_by2 = lpd_sbf_len - 1;
  synth_delay = num_subfr_by2 * LEN_SUBFR;

  ixheaacd_memset(synth_buf, MAX_PITCH + synth_delay + len_fr);
  ixheaacd_mem_cpy(st->synth_prev, synth_buf, MAX_PITCH + synth_delay);
  ixheaacd_mem_cpy(out_buffer, synth_buf + MAX_PITCH - (LEN_SUBFR),
                   synth_delay + len_fr + (LEN_SUBFR));

  for (i = 0; i < num_subfr_by2; i++) {
    pitch[i] = st->pitch_prev[i];
    pitch_gain[i] = st->gain_prev[i];
  }
  for (i = num_subfr_by2; i < lpd_sbf_len + 3; i++) {
    pitch[i] = 64;
    pitch_gain[i] = 0.0f;
  }
  if (st->mode_prev == 0) {
    pitch[num_subfr_by2] = pitch[num_subfr_by2 - 1];
    pitch_gain[num_subfr_by2] = pitch_gain[num_subfr_by2 - 1];
    if (!is_short_flag) {
      pitch[num_subfr_by2 + 1] = pitch[num_subfr_by2];
      pitch_gain[num_subfr_by2 + 1] = pitch_gain[num_subfr_by2];
    }
  }

  synth = synth_buf + MAX_PITCH;

  for (i = 0; i < num_subfr_by2 + 2; i++) {
    tp = pitch[i];
    if ((i * LEN_SUBFR + MAX_PITCH) < tp) {
      if (usac_data->ec_flag == 0)
        return -1;
      else {
        tp = MAX_PITCH - (i * LEN_SUBFR);
      }
    } else if (((i * LEN_SUBFR + MAX_PITCH - tp) >= 1883) ||
               (((i * LEN_SUBFR) + LEN_SUBFR) > LEN_SUPERFRAME) ||
               ((((i * LEN_SUBFR) + LEN_SUBFR) - tp) > LEN_SUPERFRAME)) {
      if (usac_data->ec_flag == 0)
        return -1;
      else {
        tp = (i * LEN_SUBFR + MAX_PITCH - 1882);
      }
    }

    if (pitch_gain[i] > 0.0f) {
      synth_corr = 0.0f, synth_energy = 1e-6f;
      for (k = 0; k < LEN_SUBFR; k++) {
        synth_corr +=
            synth[i * LEN_SUBFR + k] * synth[(i * LEN_SUBFR) - tp + k];
        synth_energy +=
            synth[(i * LEN_SUBFR) - tp + k] * synth[(i * LEN_SUBFR) - tp + k];
      }
      pitch_gain[i] = synth_corr / synth_energy;
    }
  }

  err = ixheaacd_bass_post_filter(
      synth, pitch, pitch_gain, signal_out, (lpd_sbf_len + 2) * LEN_SUBFR + LEN_SUBFR,
      len_fr - (lpd_sbf_len + 4) * LEN_SUBFR, st->bpf_prev, usac_data->ec_flag);
  if (err != 0) return err;

  ixheaacd_mem_cpy(signal_out, out_buffer,
                   (lpd_sbf_len + 2) * LEN_SUBFR + LEN_SUBFR);
  return err;
}
