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
#include "ixheaac_type_def.h"
#include "ixheaac_constants.h"
#include "ixheaacd_bitbuffer.h"
#include "ixheaacd_interface.h"
#include "ixheaacd_common_rom.h"
#include "ixheaacd_sbrdecsettings.h"
#include "ixheaacd_sbr_scale.h"
#include "ixheaacd_env_extr_part.h"
#include "ixheaacd_sbr_rom.h"
#include "ixheaacd_hybrid.h"
#include "ixheaacd_ps_dec.h"
#include "ixheaacd_config.h"
#include "ixheaacd_qmf_dec.h"
#include "ixheaacd_mps_polyphase.h"
#include "ixheaacd_mps_struct_def.h"
#include "ixheaacd_mps_res_rom.h"
#include "ixheaacd_mps_aac_struct.h"
#include "ixheaacd_mps_dec.h"
#include "ixheaacd_function_selector.h"

extern const FLOAT32
    ixheaacd_mps_polyphase_filter_coeff[10 * MAX_NUM_QMF_BANDS_SAC / 2];
extern const FLOAT32 ixheaacd_mps_post_twid[30];
extern const FLOAT32 ixheaacd_mps_pre_twid[64];

extern const FLOAT32 ixheaacd_ldmps_polyphase_filter_coeff[1280];

extern const FLOAT32 ixheaacd_ldmps_pre_twid[32];
extern const FLOAT32 ixheaacd_mps_post_re_32[64];
extern const FLOAT32 ixheaacd_mps_post_im_32[64];


VOID ixheaacd_mps_synt_init(FLOAT32 state[POLY_PHASE_SYNTH_SIZE]) {
  memset(state, 0, sizeof(FLOAT32) * POLY_PHASE_SYNTH_SIZE);
}

VOID ixheaacd_mps_synt_post_fft_twiddle_dec(WORD32 resolution, FLOAT32 *fin_re,
  FLOAT32 *fin_im,
  const FLOAT32 *table_re,
  const FLOAT32 *table_im,
  FLOAT32 *state) {
  WORD32 l;
  for (l = 0; l < 2 * resolution; l++) {
    state[2 * resolution - l - 1] =
      ((fin_re[l] * table_re[l]) + (fin_im[l] * table_im[l]));
  }
}

VOID ixheaacd_mps_synt_out_calc_dec(WORD32 resolution, FLOAT32 *out,
                                    FLOAT32 *state,
                                    const FLOAT32 *filter_coeff) {
  WORD32 l, k;
  FLOAT32 *out1, *out2, *state1, *state2;
  out1 = out;
  out2 = out + resolution;
  state1 = state;
  state2 = state + (3 * resolution);

  for (k = 0; k < 5; k++) {
    for (l = 0; l < resolution; l++) {
      *out1++ = (*state1++) * (*filter_coeff++);
      *out2++ = (*state2++) * (*filter_coeff++);
    }
    out1 += resolution;
    out2 += resolution;
    state1 += (3 * resolution);
    state2 += (3 * resolution);
  }
}

VOID ixheaacd_mps_synt_out_calc_dec_ldmps(WORD32 resolution, FLOAT32 *out,
  FLOAT32 *state, const FLOAT32 *filter_coeff) {
  WORD32 l, k;
  FLOAT32 *out1, *out2, *state1, *state2;
  const FLOAT32 *filter1, *filter2;
  filter1 = filter_coeff;
  filter2 = filter_coeff + resolution;
  out1 = out;
  out2 = out + resolution;
  state1 = state;
  state2 = state + (3 * resolution);

  for (k = 0; k < 5; k++) {
    for (l = 0; l < resolution; l++) {
      *out1++ = (*state1++) * (*filter1++);
      *out2++ = (*state2++) * (*filter2++);
    }
    filter1 += resolution;
    filter2 += resolution;
    out1 += resolution;
    out2 += resolution;
    state1 += (3 * resolution);
    state2 += (3 * resolution);
  }
}

VOID ixheaacd_mps_synt_out_calc_dec_ldmps_32(WORD32 resolution, FLOAT32 *out,
  FLOAT32 *state, const FLOAT32 *filter_coeff) {
  WORD32 l, k;
  FLOAT32 *out1, *out2, *state1, *state2;
  const FLOAT32 *filter1, *filter2;
  filter1 = filter_coeff;
  filter2 = filter_coeff + 2 * resolution;
  out1 = out;
  out2 = out + resolution;
  state1 = state;
  state2 = state + (3 * resolution);

  for (k = 0; k < 5; k++) {
    for (l = 0; l < resolution; l++) {
      *out1++ = ((*state1++) * (filter1[2*l] + filter1[2*l+1])/2);
      *out2++ = ((*state2++) *  (filter2[2 * l] + filter2[2 * l + 1])/2);
    }
    filter1 += 4 * resolution;
    filter2 += 4 * resolution;
    out1 += resolution;
    out2 += resolution;
    state1 += (3 * resolution);
    state2 += (3 * resolution);
  }
}

VOID ixheaacd_mps_synth_pre_twidle(FLOAT32 *out_re, FLOAT32 *out_im,
                                   FLOAT32 *c_in, WORD32 len) {
  WORD32 i;
  FLOAT32 *c_s = c_in;
  FLOAT32 *p_re_s = out_re;
  FLOAT32 *p_im_s = out_im;
  FLOAT32 *c_e = c_in + (len << 1) - 1;
  FLOAT32 *p_im_e = out_im + len - 1;
  FLOAT32 *p_re_e = out_re + len - 1;
  const FLOAT32 *prtw = ixheaacd_mps_pre_twid;

  for (i = 0; i < len; i += 4) {
    *p_re_s = ((*c_s++) * (*prtw));
    p_re_s++;
    *p_im_s = -((*c_s--) * (*prtw));
    p_im_s++;
    *p_im_s = ((*c_e--) * (*prtw));
    p_im_s--;
    *p_re_s = ((*c_e++) * (*prtw++));
    p_re_s--;
    *p_im_s += ((*c_e--) * (*prtw));
    p_im_s++;
    *p_re_s += ((*c_e--) * (*prtw));
    p_re_s++;
    *p_re_s -= ((*c_s++) * (*prtw));
    p_re_s++;
    *p_im_s += ((*c_s++) * (*prtw++));
    p_im_s++;
    *p_im_e = ((*c_e--) * (*prtw));
    p_im_e--;
    *p_re_e = -((*c_e++) * (*prtw));
    p_re_e--;
    *p_re_e = ((*c_s++) * (*prtw));
    p_re_e++;
    *p_im_e = ((*c_s--) * (*prtw++));
    p_im_e++;
    *p_re_e += ((*c_s++) * (*prtw));
    p_re_e--;
    *p_im_e += ((*c_s++) * (*prtw));
    p_im_e--;
    *p_im_e -= ((*c_e--) * (*prtw));
    p_im_e--;
    *p_re_e += ((*c_e--) * (*prtw++));
    p_re_e--;
  }
}

VOID ixheaacd_mps_synth_post_twidle(FLOAT32 *state, FLOAT32 *out_re,
                                    FLOAT32 *out_im, WORD32 len) {
  WORD32 i;
  {
    FLOAT32 x_0, x_1, x_2, x_3;
    FLOAT32 *p_re_e, *p_im_e;
    const FLOAT32 *potw = ixheaacd_mps_post_twid;
    FLOAT32 *p_re_s = out_re;
    FLOAT32 *p_im_s = out_im;

    p_re_e = p_re_s + (len - 2);
    p_im_e = p_im_s + (len - 2);
    x_0 = *p_re_e;
    x_1 = *(p_re_e + 1);
    x_2 = *p_im_e;
    x_3 = *(p_im_e + 1);

    *(p_re_e + 1) = -*(p_re_s + 1);
    *(p_im_e + 1) = -*p_im_s;
    *p_im_s = *(p_im_s + 1);

    for (i = 5; i < len; i += 4) {
      FLOAT32 twdr = *potw++;
      FLOAT32 twdi = *potw++;
      FLOAT32 tmp;

      *p_re_e = (x_0 * twdi);
      *p_re_e += (x_1 * twdr);
      p_re_e--;
      p_re_s++;
      *p_re_s = (x_0 * twdr);
      *p_re_s -= (x_1 * twdi);
      p_re_s++;
      x_1 = *p_re_e--;
      x_0 = *p_re_e++;
      *p_re_e = (*p_re_s++ * twdi);
      *p_re_e += -(*p_re_s * twdr);
      p_re_e--;
      tmp = (*p_re_s-- * twdi);
      *p_re_s = tmp + (*p_re_s * twdr);

      *p_im_e = -(x_2 * twdr);
      *p_im_e += (x_3 * twdi);
      p_im_e--;
      p_im_s++;
      *p_im_s = -(x_2 * twdi);
      *p_im_s -= (x_3 * twdr);
      p_im_s++;
      x_3 = *p_im_e--;
      x_2 = *p_im_e++;
      *p_im_e = -(*p_im_s++ * twdr);
      *p_im_e -= (*p_im_s * twdi);
      p_im_e--;
      tmp = (*p_im_s-- * twdr);
      *p_im_s = tmp - (*p_im_s * twdi);
    }

    *p_re_e = 0.7071067f * (x_1 + x_0);
    *p_im_e = 0.7071067f * (x_3 - x_2);
    *(p_re_s + 1) = -0.7071067f * (x_1 - x_0);
    *(p_im_s + 1) = -0.7071067f * (x_3 + x_2);
  }

  for (i = 0; i < len; i++) {
    state[i] = out_im[i] - out_re[i];
    state[len + i] = out_im[len - i - 1] + out_re[len - i - 1];
    state[len - i - 1] = out_im[len - i - 1] - out_re[len - i - 1];
    state[2 * len - i - 1] = out_im[i] + out_re[i];
  }
}

VOID ixheaacd_mps_synt_pre_twiddle_dec(FLOAT32 *ptr_in, const FLOAT32 *table,
  FLOAT32 *fin_re, FLOAT32 *fin_im,
  WORD32 resolution) {
  WORD32 k;
  FLOAT32 *c_s = ptr_in;
  FLOAT32 *p_re_s = fin_re;
  FLOAT32 *p_im_s = fin_im;
  FLOAT32 *c_e = ptr_in + (resolution << 1) - 1;
  FLOAT32 *p_im_e = fin_im + resolution - 1;
  FLOAT32 *p_re_e = fin_re + resolution - 1;

  for (k = 0; k < resolution; k += 2) {
    *p_re_s = (*c_s++) * (*table);
    *p_im_s = (*c_s) * (*table);

    *p_re_e = (*c_e--) * (*table);
    *p_im_e = -(*c_e) * (*table++);

    *p_re_s += (*c_s--) * (*table);
    *p_im_s += -(*c_s++) * (*table);
    p_re_s++;
    p_im_s++;
    c_s++;

    *p_re_e += (*c_e++) * (*table);
    *p_im_e += (*c_e--) * (*table++);
    p_re_e--;
    p_im_e--;
    c_e--;
  }
}

VOID ixheaacd_mps_synt_calc(ia_mps_dec_state_struct *self) {
  WORD32 k, l, ts, ch;
  FLOAT32 *state, *tmp_state, *out;
  const FLOAT32 *filt_coeff;
  FLOAT32 *tmp_buf = self->tmp_buf;
  FLOAT32 fin_re[64] = {0};
  FLOAT32 fin_im[64] = {0};

  WORD32 resolution = self->resolution;
  WORD32 m_resolution = resolution >> 1;
  const FLOAT32 *ixheaacd_mps_post_re, *ixheaacd_mps_post_im;
  VOID(*ixheaacd_mps_synt_out_calc_pointer)
  (WORD32 resolution, FLOAT32 *out, FLOAT32 *state, const FLOAT32 *filter_coeff);

  if (self->ldmps_config.ldmps_present_flag)
  {
    ixheaacd_mps_synt_out_calc_pointer = &ixheaacd_mps_synt_out_calc_dec_ldmps;
    filt_coeff = ixheaacd_ldmps_polyphase_filter_coeff;
  }
  else
  {
    ixheaacd_mps_synt_out_calc_pointer = ixheaacd_mps_synt_out_calc;
    filt_coeff = ixheaacd_mps_polyphase_filter_coeff;
  }
  if (self->qmf_band_count == 32)
  {
    for (ch = 0; ch < self->out_ch_count; ch++) {
      tmp_state = self->qmf_filt_state[ch];
      state = &tmp_buf[self->time_slots * 2 * resolution];
      memcpy(state, tmp_state, sizeof(FLOAT32) * 18 * resolution);
      out = &tmp_buf[74 * MAX_NUM_QMF_BANDS_SAC];

      ixheaacd_mps_post_re = ixheaacd_mps_post_re_32;
      ixheaacd_mps_post_im = ixheaacd_mps_post_im_32;

      for (ts = 0; ts < self->time_slots; ts++) {

        state -= (2 * resolution);

        ixheaacd_mps_synt_pre_twiddle_dec(&self->qmf_out_dir[ch][ts][0].re,
            ixheaacd_ldmps_pre_twid, fin_re, fin_im, resolution);

        for (k = resolution; k < 2 * resolution; k++)
        {
          fin_re[k] = 0;
          fin_im[k] = 0;
        }

        ixheaacd_mps_complex_fft(fin_re, fin_im, 2 * resolution);

        ixheaacd_mps_synt_post_fft_twiddle_dec(resolution, fin_re, fin_im,
                                              ixheaacd_mps_post_re,
                                              ixheaacd_mps_post_im, state);

        ixheaacd_mps_synt_out_calc_dec_ldmps_32(resolution, out, state, filt_coeff);

        for (k = 0; k < resolution; k++) {
          FLOAT32 acc = out[k];
          for (l = 1; l < 10; l++) {
            acc += out[resolution * l + k];
          }
          self->output_buffer[ch][self->qmf_band_count * ts + k] = acc;
        }
      }
      memcpy(tmp_state, state, sizeof(FLOAT32) * 18 * resolution);
    }
  }
  else
  {
    for (ch = 0; ch < self->out_ch_count; ch++) {
      tmp_state = self->qmf_filt_state[ch];
      state = &tmp_buf[self->time_slots * 2 * resolution];
      memcpy(state, tmp_state, sizeof(FLOAT32) * 18 * resolution);
      out = &tmp_buf[74 * MAX_NUM_QMF_BANDS_SAC];

      for (ts = 0; ts < self->time_slots; ts++) {

        state -= (2 * resolution);

        ixheaacd_mps_synth_pre_twidle(
          fin_re, fin_im, &self->qmf_out_dir[ch][ts][0].re, resolution);

        ixheaacd_mps_synth_calc_fft(fin_re, fin_im, m_resolution);

        ixheaacd_mps_synth_post_twidle(state, fin_re, fin_im, resolution);
        (*ixheaacd_mps_synt_out_calc_pointer)(resolution, out, state, filt_coeff);

        for (k = 0; k < resolution; k++) {
          FLOAT32 acc = out[k];
          for (l = 1; l < 10; l++) {
            acc += out[resolution * l + k];
          }
          self->output_buffer[ch][self->qmf_band_count * ts + k] = acc;
        }
      }
      memcpy(tmp_state, state, sizeof(FLOAT32) * 18 * resolution);
    }
  }
}
