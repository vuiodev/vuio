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
#include <float.h>
#include <stdlib.h>
#include <stdio.h>
#include <math.h>
#include <string.h>
#include "ixheaac_type_def.h"
#include "ixheaacd_bitbuffer.h"

#include "ixheaacd_interface.h"

#include "ixheaacd_tns_usac.h"
#include "ixheaacd_cnst.h"

#include "ixheaacd_acelp_info.h"

#include "ixheaacd_td_mdct.h"

#include "ixheaacd_sbrdecsettings.h"
#include "ixheaacd_info.h"
#include "ixheaacd_sbr_common.h"
#include "ixheaacd_drc_data_struct.h"
#include "ixheaacd_drc_dec.h"
#include "ixheaacd_sbrdecoder.h"
#include "ixheaacd_mps_polyphase.h"
#include "ixheaac_sbr_const.h"

#include "ixheaacd_ec_defines.h"
#include "ixheaacd_ec_struct_def.h"
#include "ixheaacd_main.h"
#include "ixheaacd_arith_dec.h"
#include "ixheaacd_func_def.h"
#include "ixheaacd_windows.h"
#include "ixheaacd_acelp_com.h"

#include "ixheaac_constants.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops40.h"

#define ABS(A) ((A) < 0 ? (-A) : (A))

VOID ixheaacd_lpc_coeff_wt_apply(FLOAT32 *a, FLOAT32 *ap) {
  FLOAT32 f;
  WORD32 i;
  ap[0] = a[0];
  f = 0.92f;
  for (i = 1; i <= 16; i++) {
    ap[i] = f * a[i];
    f *= 0.92f;
  }
  return;
}

WORD8 ixheaacd_float2fix(FLOAT32 *x, WORD32 *int_x, WORD32 length) {
  WORD32 k, itemp;
  FLOAT32 ftemp = 0.0;
  WORD8 shiftp;
  for (k = 0; k < length; k++) {
    if (ABS(x[k]) > ftemp) ftemp = ABS(x[k]);
  }

  itemp = (WORD32)(ftemp);
  shiftp = ixheaac_norm32(itemp);

  for (k = 0; k < length; k++) {
    int_x[k] = (WORD32)(x[k] * (FLOAT32)((WORD64)1 << shiftp));
  }

  return (shiftp);
}

VOID ixheaacd_fix2float(WORD32 *int_xn1, FLOAT32 *xn1, WORD32 length,
                        WORD8 *shiftp, WORD32 *preshift) {
  WORD32 k;
  FLOAT32 qfac;
  if ((*shiftp - *preshift) > 0) {
    qfac = 1.0f / (FLOAT32)((WORD64)1 << (*shiftp - *preshift));
    for (k = 0; k < length; k++) {
      xn1[k] = (FLOAT32)((FLOAT32)int_xn1[k] * qfac);
    }
  } else {
    for (k = 0; k < length; k++) {
      xn1[k] = (FLOAT32)((FLOAT32)int_xn1[k] *
                         (FLOAT32)((WORD64)1 << (*preshift - *shiftp)));
    }
  }
}

static VOID ixheaacd_low_fq_deemphasis(FLOAT32 x[], WORD32 lg,
                                       FLOAT32 gains[]) {
  WORD32 i, j, k, i_max;
  FLOAT32 max, factor, rm;

  k = 8;
  i_max = lg / 4;

  max = 0.01f;
  for (i = 0; i < i_max; i += k) {
    rm = 0.01f;
    for (j = i; j < i + k; j++) rm += x[j] * x[j];

    if (rm > max) max = rm;
  }

  factor = 0.1f;
  for (i = 0; i < i_max; i += k) {
    rm = 0.01f;
    for (j = i; j < i + k; j++) rm += x[j] * x[j];

    rm = (FLOAT32)sqrt(rm / max);
    if (rm > factor) factor = rm;

    for (j = i; j < i + k; j++) x[j] *= factor;

    gains[i / k] = factor;
  }

  return;
}

static WORD32 ixheaacd_calc_max_pitch(FLOAT32 x[LEN_SUPERFRAME], WORD32 n) {
  FLOAT32 max_m;
  FLOAT32 t_est;
  WORD32 i, i_max, pitch_tcx;

  max_m = 0;
  i_max = 1;

  for (i = 1; i < n; i++) {
    FLOAT32 mag = (x[2 * i] * x[2 * i]) + (x[2 * i + 1] * x[2 * i + 1]);
    if (mag > max_m) {
      max_m = mag;
      i_max = i;
    }
  }

  t_est = (n / (FLOAT32)i_max);

  if (t_est >= 256) {
    pitch_tcx = 256;
  } else {
    FLOAT32 tmp_est = t_est;
    while (tmp_est < 256) {
      tmp_est += t_est;
    }
    pitch_tcx = (WORD32)(tmp_est - t_est);
  }

  return (pitch_tcx);
}

WORD32 ixheaacd_tcx_mdct(ia_usac_data_struct *usac_data,
                         ia_td_frame_data_struct *pstr_td_frame_data,
                         WORD32 frame_index, FLOAT32 lp_flt_coff_a[], WORD32 lg,
                         ia_usac_lpd_decoder_handle st) {
  WORD32 i, mode;
  WORD32 *ptr_tcx_quant;
  FLOAT32 tmp, gain_tcx = 0.0f, noise_level, energy, temp;
  FLOAT32 *ptr_a, i_ap[ORDER + 1];
  const FLOAT32 *sine_window_prev, *sine_window;
  WORD32 fac_length_prev;
  FLOAT32 alfd_gains[LEN_SUPERFRAME / (4 * 8)] = {0};
  FLOAT32 x[LEN_SUPERFRAME], buf[ORDER + LEN_SUPERFRAME];
  WORD32 int_x[LEN_SUPERFRAME + (2 * FAC_LENGTH)];
  WORD32 int_xn1[LEN_SUPERFRAME + (2 * FAC_LENGTH)];
  FLOAT32 gain1[LEN_SUPERFRAME], gain2[LEN_SUPERFRAME];
  FLOAT32 xn_buf[LEN_SUPERFRAME + (2 * FAC_LENGTH)];
  FLOAT32 *xn;
  FLOAT32 xn1[2 * FAC_LENGTH], facwindow[2 * FAC_LENGTH];
  WORD32 TTT;
  WORD8 shiftp = 0;
  WORD32 preshift = 0;
  WORD32 loop_count = 0;
  FLOAT32 *exc = &usac_data->exc_buf[usac_data->len_subfrm * frame_index +
                                     MAX_PITCH + INTER_LP_FIL_ORDER + 1];
  FLOAT32 *synth =
      &usac_data->synth_buf[usac_data->len_subfrm * frame_index + MAX_PITCH +
                            (((NUM_FRAMES * usac_data->num_subfrm) / 2) - 1) *
                                LEN_SUBFR];

  WORD32 *ptr_scratch = &usac_data->scratch_buffer[0];

  WORD32 fac_length = (usac_data->len_subfrm) / 2;
  WORD32 err = 0;

  mode = lg / (usac_data->len_subfrm);
  if (mode > 2) mode = 3;

  if (st->mode_prev == -2)
    fac_length_prev = (usac_data->ccfl) / 16;

  else
    fac_length_prev = fac_length;

  if (fac_length == 96)
    sine_window = ixheaacd_sine_window192;
  else
    sine_window = ixheaacd__sine_window256;

  if (fac_length_prev == 48)
    sine_window_prev = ixheaacd_sine_window96;

  else if (fac_length_prev == 64)
    sine_window_prev = ixheaacd_sine_window128;

  else if (fac_length_prev == 96)
    sine_window_prev = ixheaacd_sine_window192;

  else
    sine_window_prev = ixheaacd__sine_window256;

  xn = xn_buf + fac_length;

  if (st->mode_prev != 0) {
    if (st->mode_prev > 0) {
      for (i = 0; i < (2 * fac_length_prev); i++) {
        st->exc_prev[i + fac_length - fac_length_prev + 1] *=
            sine_window_prev[(2 * fac_length_prev) - 1 - i];
      }
    }
    for (i = 0; i < fac_length - fac_length_prev; i++) {
      st->exc_prev[i + fac_length + fac_length_prev + 1] = 0.0f;
    }
  }
  if (usac_data->frame_ok == 1) {
    noise_level = 0.0625f * (8.0f - ((FLOAT32)pstr_td_frame_data->noise_factor[frame_index]));

  ptr_tcx_quant = pstr_td_frame_data->x_tcx_invquant;
  for (i = 0; i < frame_index; i++)
    ptr_tcx_quant += pstr_td_frame_data->tcx_lg[i];

  for (i = 0; i < lg; i++) x[i] = (FLOAT32)ptr_tcx_quant[i];

    if (usac_data->ec_flag) {
      st->last_tcx_pitch = ixheaacd_calc_max_pitch(x, (lg >> 5));
    }

  for (i = lg / 6; i < lg; i += 8) {
    WORD32 k, max_k = min(lg, i + 8);
    FLOAT32 tmp = 0.0f;
    for (k = i; k < max_k; k++) tmp += ptr_tcx_quant[k] * ptr_tcx_quant[k];

    if (tmp == 0.0f) {
      for (k = i; k < max_k; k++)
        x[k] = noise_level *
               ixheaacd_randomsign(
                   &(usac_data->seed_value[usac_data->present_chan]));
    }
  }

  ixheaacd_low_fq_deemphasis(x, lg, alfd_gains);

  ixheaacd_lpc_coeff_wt_apply(lp_flt_coff_a + (ORDER + 1), i_ap);
  ixheaacd_lpc_to_td(i_ap, ORDER, gain1, usac_data->len_subfrm / 4);

  ixheaacd_lpc_coeff_wt_apply(lp_flt_coff_a + (2 * (ORDER + 1)), i_ap);
  ixheaacd_lpc_to_td(i_ap, ORDER, gain2, usac_data->len_subfrm / 4);

  energy = 0.01f;
  for (i = 0; i < lg; i++) energy += x[i] * x[i];

  temp = (FLOAT32)sqrt(energy) / lg;

  gain_tcx =
      (FLOAT32)pow(
          10.0f,
          ((FLOAT32)pstr_td_frame_data->global_gain[frame_index]) / 28.0f) /
      (temp * 2.0f);
  }
  if (usac_data->ec_flag) {
    if (usac_data->frame_ok == 1) {
      usac_data->past_gain_tcx[usac_data->present_chan] = gain_tcx;
    } else {
      gain_tcx = usac_data->past_gain_tcx[usac_data->present_chan];
    }
  }
  if (usac_data->frame_ok == 1) {
  ixheaacd_noise_shaping(x, lg, (usac_data->len_subfrm) / 4, gain1, gain2);
  shiftp = ixheaacd_float2fix(x, int_x, lg);
  }
  if (usac_data->ec_flag == 1) {
    if (st->mode_prev != 0) {
      if (usac_data->frame_ok == 1) {
        memcpy(usac_data->tcx_spec_coeffs[usac_data->present_chan], int_x, lg * sizeof(int_x[0]));
        usac_data->last_shiftp = shiftp;
      } else {
        memcpy(int_x, usac_data->tcx_spec_coeffs[usac_data->present_chan], lg * sizeof(int_x[0]));
        shiftp = usac_data->last_shiftp;
      }
    }
  } else {
    if (lg & (lg - 1)) {
      if ((lg != 48) && (lg != 96) && (lg != 192) && (lg != 384) && (lg != 768)) {
        return -1;
      }
    }
  }

  ixheaacd_acelp_mdct_main(usac_data, int_x, int_xn1, (2 * fac_length), lg - (2 * fac_length),
                           &preshift);

  ixheaacd_fix2float(int_xn1, xn_buf, (lg + (2 * fac_length)), &shiftp,
                     &preshift);

  ixheaacd_vec_cnst_mul((2.0f / lg), xn_buf, xn_buf, lg + (2 * fac_length));

  st->fac_gain =
      gain_tcx * 0.5f * (FLOAT32)sqrt(((FLOAT32)fac_length) / (FLOAT32)lg);

  for (i = 0; i < fac_length / 4; i++)
    st->fac_fd_data[i] = alfd_gains[i * lg / (8 * fac_length)];

  if (st->mode_prev == 0) {
    for (i = 0; i < fac_length_prev; i++) {
      facwindow[i] =
          sine_window_prev[i] * sine_window_prev[(2 * fac_length_prev) - 1 - i];
      facwindow[fac_length_prev + i] =
          1.0f - (sine_window_prev[fac_length_prev + i] *
                  sine_window_prev[fac_length_prev + i]);
    }

    for (i = 0; i < fac_length / 2; i++) {
      x[i] = st->fac_gain *
             (FLOAT32)pstr_td_frame_data->fac[frame_index * FAC_LENGTH + 2 * i];
      x[fac_length / 2 + i] =
          st->fac_gain *
          (FLOAT32)pstr_td_frame_data
              ->fac[frame_index * FAC_LENGTH + fac_length - 2 * i - 1];
    }

    for (i = 0; i < fac_length / 8; i++) {
      x[i] *= st->fac_fd_data[2 * i];
      x[fac_length - i - 1] *= st->fac_fd_data[2 * i + 1];
    }

    preshift = 0;
    shiftp = ixheaacd_float2fix(x, int_x, fac_length);

    if (usac_data->ec_flag == 0) {
      if (fac_length & (fac_length - 1)) {
        if ((fac_length != 48) && (fac_length != 96) && (fac_length != 192) &&
            (fac_length != 384) && (fac_length != 768)) {
          return -1;
        }
      }
    }

    ixheaacd_acelp_mdct(int_x, int_xn1, &preshift, fac_length, ptr_scratch);

    ixheaacd_fix2float(int_xn1, xn1, fac_length, &shiftp, &preshift);

    ixheaacd_vec_cnst_mul((2.0f / (FLOAT32)fac_length), xn1, xn1, fac_length);

    memset(xn1 + fac_length, 0, fac_length * sizeof(FLOAT32));

    ixheaacd_lpc_coeff_wt_apply(lp_flt_coff_a + (ORDER + 1), i_ap);
    ixheaacd_synthesis_tool_float(i_ap, xn1, xn1, 2 * fac_length,
                                  xn1 + fac_length);

    for (i = 0; i < fac_length; i++) {
      temp = st->exc_prev[1 + fac_length + i] * facwindow[fac_length + i] +
             st->exc_prev[fac_length - i] * facwindow[fac_length - 1 - i];
      xn1[i] += temp;
    }
  }

  for (i = 0; i < lg + (2 * fac_length); i++) xn_buf[i] *= gain_tcx;

  for (i = 0; i < (2 * fac_length_prev); i++)
    xn_buf[i + fac_length - fac_length_prev] *= sine_window_prev[i];

  for (i = 0; i < fac_length - fac_length_prev; i++) xn_buf[i] = 0.0f;

  if (st->mode_prev != 0) {
    for (i = fac_length - fac_length_prev; i < (fac_length + fac_length_prev);
         i++)
      xn_buf[i] += st->exc_prev[1 + i];
  } else {
    for (i = fac_length - fac_length_prev; i < (fac_length + fac_length_prev);
         i++)
      xn_buf[i + fac_length] += xn1[i];
  }

  ixheaacd_mem_cpy(xn_buf + lg - 1, st->exc_prev, 1 + (2 * fac_length));

  for (i = 0; i < (2 * fac_length); i++) {
    xn_buf[i + lg] *= sine_window[(2 * fac_length) - 1 - i];
  }

  if (st->mode_prev != 0) {
    ixheaacd_mem_cpy(xn_buf + fac_length - fac_length_prev,
                     synth - fac_length_prev, fac_length_prev);

    for (i = 0; i < ORDER + fac_length; i++)
      buf[i] = synth[i - ORDER - fac_length] -
               (PREEMPH_FILT_FAC * synth[i - ORDER - fac_length - 1]);

    ptr_a = st->lp_flt_coeff_a_prev;
    TTT = fac_length % LEN_SUBFR;
    if (TTT != 0)
      ixheaacd_residual_tool_float(ptr_a, &buf[ORDER], &exc[-fac_length], TTT,
                                   1);

    loop_count = (fac_length - TTT) / LEN_SUBFR;
    ixheaacd_residual_tool_float(ptr_a, &buf[ORDER + TTT],
                                 &exc[TTT - fac_length], LEN_SUBFR, loop_count);
  }

  ixheaacd_mem_cpy(xn, synth, lg);

  ixheaacd_mem_cpy(synth - ORDER - 1, xn - ORDER - 1, ORDER + 1);
  tmp = xn[-ORDER - 1];
  ixheaacd_preemphsis_tool_float(xn - ORDER, PREEMPH_FILT_FAC, ORDER + lg, tmp);

  ptr_a = lp_flt_coff_a + (2 * (ORDER + 1));

  ixheaacd_residual_tool_float(ptr_a, xn, exc, lg, 1);

  ixheaacd_mem_cpy(ptr_a, st->lp_flt_coeff_a_prev, ORDER + 1);
  ixheaacd_mem_cpy(ptr_a, st->lp_flt_coeff_a_prev + ORDER + 1, ORDER + 1);

  return err;
}

FLOAT32 ixheaacd_randomsign(UWORD32 *seed) {
  FLOAT32 sign = 0.0f;
  *seed = (UWORD32)(((UWORD64)(*seed) * (UWORD64)69069) + 5);

  if (((*seed) & 0x10000) > 0)
    sign = -1.f;
  else
    sign = +1.f;

  return sign;
}
