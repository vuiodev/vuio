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
#include <string.h>
#include "ixheaac_type_def.h"
#include "ixheaacd_bitbuffer.h"
#include "ixheaacd_interface.h"
#include "ixheaacd_sbr_common.h"
#include "ixheaacd_drc_data_struct.h"
#include "ixheaacd_drc_dec.h"

#include "ixheaac_sbr_const.h"
#include "ixheaacd_sbrdecsettings.h"
#include "ixheaacd_sbrdecoder.h"
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
#include "ixheaacd_qmf_poly.h"
#include "ixheaac_esbr_rom.h"

WORD32 ixheaacd_complex_anal_filt(ia_esbr_hbe_txposer_struct *ptr_hbe_txposer) {
  WORD32 idx;
  WORD32 anal_size = 2 * ptr_hbe_txposer->synth_size;
  WORD32 N = (10 * anal_size);
  WORD32 no_bins = ptr_hbe_txposer->no_bins >> 1;

  if (ptr_hbe_txposer->esbr_hq != 0) {
    anal_size = 2 * ptr_hbe_txposer->analy_size;
    no_bins = ptr_hbe_txposer->no_bins;
  }

  for (idx = 0; idx < no_bins; idx++) {
    WORD32 i, j, k, l;
    FLOAT32 window_output[640];
    FLOAT32 u[128], u_in[256], u_out[256];
    FLOAT32 accu_r, accu_i;
    const FLOAT32 *inp_signal;
    FLOAT32 *anal_buf;

    FLOAT32 *analy_cos_sin_tab = ptr_hbe_txposer->analy_cos_sin_tab;
    const FLOAT32 *interp_window_coeff = ptr_hbe_txposer->analy_wind_coeff;
    FLOAT32 *x = ptr_hbe_txposer->analy_buf;
    if (ptr_hbe_txposer->esbr_hq != 0) {
      memset(ptr_hbe_txposer->qmf_in_buf[idx], 0,
          TWICE_QMF_SYNTH_CHANNELS_NUM * sizeof(FLOAT32));
      inp_signal = ptr_hbe_txposer->ptr_output_buf +
          idx * ptr_hbe_txposer->analy_size + 1;
      anal_buf = &ptr_hbe_txposer->qmf_in_buf[idx]
          [4 * ptr_hbe_txposer->a_start];
    } else {
    memset(ptr_hbe_txposer->qmf_in_buf[idx + HBE_OPER_WIN_LEN - 1], 0,
           TWICE_QMF_SYNTH_CHANNELS_NUM * sizeof(FLOAT32));

    inp_signal = ptr_hbe_txposer->ptr_input_buf +
                 idx * 2 * ptr_hbe_txposer->synth_size + 1;
    anal_buf = &ptr_hbe_txposer->qmf_in_buf[idx + HBE_OPER_WIN_LEN - 1]
                                           [4 * ptr_hbe_txposer->k_start];
    }

    for (i = N - 1; i >= anal_size; i--) {
      x[i] = x[i - anal_size];
    }

    for (i = anal_size - 1; i >= 0; i--) {
      x[i] = inp_signal[anal_size - 1 - i];
    }

    for (i = 0; i < N; i++) {
      window_output[i] = x[i] * interp_window_coeff[i];
    }

    for (i = 0; i < 2 * anal_size; i++) {
      accu_r = 0.0;
      for (j = 0; j < 5; j++) {
        accu_r = accu_r + window_output[i + j * 2 * anal_size];
      }
      u[i] = accu_r;
    }
    if (anal_size == 40 || anal_size == 56) {
      for (i = 1; i < anal_size; i++) {
        FLOAT32 temp1 = u[i] + u[2 * anal_size - i];
        FLOAT32 temp2 = u[i] - u[2 * anal_size - i];
        u[i] = temp1;
        u[2 * anal_size - i] = temp2;
      }

      for (k = 0; k < anal_size; k++) {
        accu_r = u[anal_size];
        if (k & 1)
          accu_i = u[0];
        else
          accu_i = -u[0];
        for (l = 1; l < anal_size; l++) {
          accu_r = accu_r + u[0 + l] * analy_cos_sin_tab[2 * l + 0];
          accu_i = accu_i + u[2 * anal_size - l] * analy_cos_sin_tab[2 * l + 1];
        }
        analy_cos_sin_tab += (2 * anal_size);
        *anal_buf++ = (FLOAT32)accu_r;
        *anal_buf++ = (FLOAT32)accu_i;
      }
    } else {
      FLOAT32 *ptr_u = u_in;
      FLOAT32 *ptr_v = u_out;
      for (k = 0; k < anal_size * 2; k++) {
        *ptr_u++ = ((*analy_cos_sin_tab++) * u[k]);
        *ptr_u++ = ((*analy_cos_sin_tab++) * u[k]);
      }
      if (ptr_hbe_txposer->ixheaacd_cmplx_anal_fft != NULL)
        (*(ptr_hbe_txposer->ixheaacd_cmplx_anal_fft))(u_in, u_out,
                                                      anal_size * 2);
      else
        return -1;

      for (k = 0; k < anal_size / 2; k++) {
        *(anal_buf + 1) = -*ptr_v++;
        *anal_buf = *ptr_v++;

        anal_buf += 2;

        *(anal_buf + 1) = *ptr_v++;
        *anal_buf = -*ptr_v++;

        anal_buf += 2;
      }
    }
  }
  return 0;
}

WORD32 ixheaacd_real_synth_filt(ia_esbr_hbe_txposer_struct *ptr_hbe_txposer,
                                WORD32 num_columns, FLOAT32 qmf_buf_real[][64],
                                FLOAT32 qmf_buf_imag[][64]) {
  WORD32 i, j, k, l, idx;
  FLOAT32 g[640];
  FLOAT32 w[640];
  FLOAT32 synth_out[128];
  FLOAT32 accu_r;
  WORD32 synth_size = ptr_hbe_txposer->synth_size;
  FLOAT32 *ptr_cos_tab_trans_qmf =
      (FLOAT32 *)&ixheaac_cos_table_trans_qmf[0][0] +
      ptr_hbe_txposer->k_start * 32;
  FLOAT32 *buffer = ptr_hbe_txposer->synth_buf;
  FLOAT32 *ptr_inp_buf = ptr_hbe_txposer->ptr_input_buf + ptr_hbe_txposer->ana_fft_size[0];

  for (idx = 0; idx < num_columns; idx++) {
    FLOAT32 loc_qmf_buf[64];
    FLOAT32 *synth_buf_r = loc_qmf_buf;
    FLOAT32 *out_buf;
    if (ptr_hbe_txposer->esbr_hq == 1) {
        out_buf = ptr_inp_buf +
                  (idx - 1) * ptr_hbe_txposer->synth_size;
    } else {
      out_buf = ptr_hbe_txposer->ptr_input_buf +
                       (idx + 1) * ptr_hbe_txposer->synth_size;
    }
    FLOAT32 *synth_cos_tab = ptr_hbe_txposer->synth_cos_tab;
    const FLOAT32 *interp_window_coeff = ptr_hbe_txposer->synth_wind_coeff;
    if (ptr_hbe_txposer->k_start < 0) return -1;
    for (k = 0; k < synth_size; k++) {
      WORD32 ki = ptr_hbe_txposer->k_start + k;
      synth_buf_r[k] = (FLOAT32)(
          ptr_cos_tab_trans_qmf[(k << 1) + 0] * qmf_buf_real[idx][ki] +
          ptr_cos_tab_trans_qmf[(k << 1) + 1] * qmf_buf_imag[idx][ki]);

      synth_buf_r[k + ptr_hbe_txposer->synth_size] = 0;
    }

    for (l = (20 * synth_size - 1); l >= 2 * synth_size; l--) {
      buffer[l] = buffer[l - 2 * synth_size];
    }

    if (synth_size == 20) {
      FLOAT32 *psynth_cos_tab = synth_cos_tab;

      for (l = 0; l < (synth_size + 1); l++) {
        accu_r = 0.0;
        for (k = 0; k < synth_size; k++) {
          accu_r += synth_buf_r[k] * psynth_cos_tab[k];
        }
        buffer[0 + l] = accu_r;
        buffer[synth_size - l] = accu_r;
        psynth_cos_tab = psynth_cos_tab + synth_size;
      }
      for (l = (synth_size + 1); l < (2 * synth_size - synth_size / 2); l++) {
        accu_r = 0.0;
        for (k = 0; k < synth_size; k++) {
          accu_r += synth_buf_r[k] * psynth_cos_tab[k];
        }
        buffer[0 + l] = accu_r;
        buffer[3 * synth_size - l] = -accu_r;
        psynth_cos_tab = psynth_cos_tab + synth_size;
      }
      accu_r = 0.0;
      for (k = 0; k < synth_size; k++) {
        accu_r += synth_buf_r[k] * psynth_cos_tab[k];
      }
      buffer[3 * synth_size >> 1] = accu_r;
    } else {
      FLOAT32 tmp;
      FLOAT32 *ptr_u = synth_out;
      WORD32 kmax = (synth_size >> 1);
      FLOAT32 *syn_buf = &buffer[kmax];
      kmax += synth_size;

      if (ptr_hbe_txposer->ixheaacd_real_synth_fft != NULL)
        (*(ptr_hbe_txposer->ixheaacd_real_synth_fft))(synth_buf_r, synth_out,
                                                      synth_size * 2);
      else
        return -1;

      for (k = 0; k < kmax; k++) {
        tmp = ((*ptr_u++) * (*synth_cos_tab++));
        tmp -= ((*ptr_u++) * (*synth_cos_tab++));
        *syn_buf++ = tmp;
      }

      syn_buf = &buffer[0];
      kmax -= synth_size;

      for (k = 0; k < kmax; k++) {
        tmp = ((*ptr_u++) * (*synth_cos_tab++));
        tmp -= ((*ptr_u++) * (*synth_cos_tab++));
        *syn_buf++ = tmp;
      }
    }

    for (i = 0; i < 5; i++) {
      memcpy(&g[(2 * i + 0) * synth_size], &buffer[(4 * i + 0) * synth_size],
             sizeof(FLOAT32) * synth_size);
      memcpy(&g[(2 * i + 1) * synth_size], &buffer[(4 * i + 3) * synth_size],
             sizeof(FLOAT32) * synth_size);
    }

    for (k = 0; k < 10 * synth_size; k++) {
      w[k] = g[k] * interp_window_coeff[k];
    }

    for (i = 0; i < synth_size; i++) {
      accu_r = 0.0;
      for (j = 0; j < 10; j++) {
        accu_r = accu_r + w[synth_size * j + i];
      }
      out_buf[i] = (FLOAT32)accu_r;
    }
  }
  return 0;
}

WORD32 ixheaacd_dft_hbe_cplx_anal_filt(ia_esbr_hbe_txposer_struct *ptr_hbe_txposer,
                                       FLOAT32 qmf_buf_real[][64],
                                       FLOAT32 qmf_buf_imag[][64]) {
  WORD32 idx;

  WORD32 anal_size = ptr_hbe_txposer->analy_size;

  WORD32 N = (10 * ptr_hbe_txposer->analy_size);

  for (idx = 0; idx < ptr_hbe_txposer->no_bins; idx++) {
    WORD32 i, j, k, l;
    FLOAT32 window_output[640];
    FLOAT32 u[128];
    FLOAT32 accu_r, accu_i;
    const FLOAT32 *inp_signal;
    FLOAT32 *qmf_buf_r = &qmf_buf_real[idx][ptr_hbe_txposer->a_start];
    FLOAT32 *qmf_buf_i = &qmf_buf_imag[idx][ptr_hbe_txposer->a_start];

    const FLOAT32 *interp_window_coeff = ptr_hbe_txposer->analy_wind_coeff;
    FLOAT32 *x = ptr_hbe_txposer->analy_buf;

    memset(&qmf_buf_real[idx][ptr_hbe_txposer->a_start], 0,
        (NO_QMF_SYNTH_CHANNELS - ptr_hbe_txposer->a_start) *
        sizeof(qmf_buf_real[idx][ptr_hbe_txposer->a_start]));
    memset(&qmf_buf_imag[idx][ptr_hbe_txposer->a_start], 0,
        TWICE_QMF_SYNTH_CHANNELS_NUM * sizeof(qmf_buf_imag[idx][ptr_hbe_txposer->a_start]));

    inp_signal = ptr_hbe_txposer->ptr_output_buf +
        idx * ptr_hbe_txposer->analy_size + 1;

    for (i = N - 1; i >= anal_size; i--) {
        x[i] = x[i - anal_size];
    }

    for (i = anal_size - 1; i >= 0; i--) {
        x[i] = inp_signal[anal_size - 1 - i];
    }

    for (i = 0; i < N; i++) {
        window_output[i] = x[i] * interp_window_coeff[i];
    }

    for (i = 0; i < 2 * anal_size; i++) {
      accu_r = 0.0;
      for (j = 0; j < 5; j++) {
        accu_r = accu_r + window_output[i + j * 2 * anal_size];
      }
      u[i] = accu_r;
    }

    for (k = 0; k < anal_size; k++) {
      accu_r = 0;
      accu_i = 0;
      for (l = 0; l < 2 * anal_size; l++) {
        accu_r = accu_r + u[l] * ptr_hbe_txposer->str_dft_hbe_anal_coeff.real[k][l];
        accu_i = accu_i + u[l] * ptr_hbe_txposer->str_dft_hbe_anal_coeff.imag[k][l];
      }
      qmf_buf_r[k] = (FLOAT32)accu_r;
      qmf_buf_i[k] = (FLOAT32)accu_i;
    }
  }
  return 0;
}
