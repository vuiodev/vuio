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
#include <string.h>

#include <assert.h>
#include "ixheaac_type_def.h"
#include "ixheaacd_bitbuffer.h"
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
#include "ixheaac_constants.h"
#include "ixheaacd_mps_struct_def.h"
#include "ixheaacd_mps_res_rom.h"
#include "ixheaacd_mps_aac_struct.h"
#include "ixheaacd_mps_dec.h"

#define ONE_IN_Q28 (268435456)
#define PI_BY_8_Q28 (105414352)
#define P_PI 3.1415926535897932
#define PI_IN_Q28 843314880
#define MULT(a, b) (a * b)
#define Q28_FLOAT_VAL ((FLOAT32)(1 << 28))
#define ONE_BY_Q28_FLOAT_VAL (1.0f / Q28_FLOAT_VAL)

extern const WORD32 ixheaacd_atan_table_Q28[16][8][31];
extern const WORD32 ixheaacd_ipd_de_quant_table_q28[16];

extern const FLOAT32 ixheaacd_im_weight[16][8][31];
extern const FLOAT32 ixheaacd_re_weight[16][8][31];
extern const FLOAT32 ixheaacd_beta[16][8][31];
extern const FLOAT32 ixheaacd_weight[16][8][31];
extern const FLOAT32 ixheaacd_c_l_table[31];
extern const FLOAT32 ixheaacd_sin_table[8][31];
extern const FLOAT32 ixheaacd_cos_table[8][31];

extern const WORD32 ixheaacd_mps_gain_set_indx[29];

static WORD32 ixheaacd_mps_phase_wraping(WORD32 phase) {
  const WORD32 pi_2 = 2 * PI_IN_Q28;

  while (phase < 0) phase += pi_2;
  while (phase >= pi_2) phase -= pi_2;
  assert((phase >= 0) && (phase < pi_2));

  return phase;
}

static VOID ixheaacd_mps_buffer_pre_and_mix_matrix(
    ia_mps_dec_state_struct *self) {
  WORD32 pb, row, col;

  for (pb = 0; pb < self->bs_param_bands; pb++) {
    for (row = 0; row < MAX_M_INPUT; row++) {
      for (col = 0; col < MAX_M_OUTPUT; col++) {
        self->m1_param_re_prev[pb][row][col] =
            self->m1_param_re[self->num_parameter_sets_prev - 1][pb][row][col];
        self->m1_param_im_prev[pb][row][col] =
            self->m1_param_im[self->num_parameter_sets_prev - 1][pb][row][col];
        self->m2_decor_re_prev[pb][row][col] =
            self->m2_decor_re[self->num_parameter_sets_prev - 1][pb][row][col];
        self->m2_decor_im_prev[pb][row][col] =
            self->m2_decor_im[self->num_parameter_sets_prev - 1][pb][row][col];
        self->m2_resid_re_prev[pb][row][col] =
            self->m2_resid_re[self->num_parameter_sets_prev - 1][pb][row][col];
        self->m2_resid_im_prev[pb][row][col] =
            self->m2_resid_im[self->num_parameter_sets_prev - 1][pb][row][col];
      }
    }
  }

  for (pb = 0; pb < self->bs_param_bands; pb++) {
    self->phase_l_prev[pb] =
        self->phase_l[self->num_parameter_sets_prev - 1][pb];
    self->phase_r_prev[pb] =
        self->phase_r[self->num_parameter_sets_prev - 1][pb];
  }
}

VOID ixheaacd_pre_and_mix_matrix_calculation(ia_mps_dec_state_struct *self) {
  WORD32 ps, pb;
  ia_mps_bs_frame *curr_bit_stream = &(self->bs_frame);
  FLOAT32 h_imag[2 * MAX_PARAMETER_BANDS];
  FLOAT32 h_real[6 * MAX_PARAMETER_BANDS];

  ixheaacd_mps_buffer_pre_and_mix_matrix(self);

  for (ps = 0; ps < self->num_parameter_sets; ps++) {
    FLOAT32 *h_im = &h_imag[0];
    FLOAT32 *h_re = &h_real[0];

    memset(h_real, 0, 6 * MAX_PARAMETER_BANDS * sizeof(FLOAT32));
    memset(h_imag, 0, 2 * MAX_PARAMETER_BANDS * sizeof(FLOAT32));

    switch (self->config->bs_phase_coding) {
      case 0:
        if (self->residual_coding) {
          ixheaacd_mps_par2umx_pred(self, curr_bit_stream, h_imag, h_real, ps,
                                    self->res_bands);
        } else {
          ixheaacd_mps_par2umx_ps(self, curr_bit_stream, h_real, ps);
        }

        break;
      case 1:
        ixheaacd_mps_par2umx_ps_ipd_opd(self, curr_bit_stream, h_real, ps);
        break;
      case 2:
        ixheaacd_mps_par2umx_pred(self, curr_bit_stream, h_imag, h_real, ps,
                                  self->res_bands);
        break;
    }

    for (pb = 0; pb < self->bs_param_bands; pb++) {
      self->m1_param_re[ps][pb][0][0] = 1.0f;
      self->m1_param_re[ps][pb][1][0] = 1.0f;

      self->m1_param_im[ps][pb][0][0] = 0;
      self->m1_param_im[ps][pb][1][0] = 0;

      self->m2_resid_re[ps][pb][0][0] = *h_re++;
      self->m2_resid_im[ps][pb][0][0] = *h_im++;
      self->m2_resid_im[ps][pb][0][1] = 0;

      self->m2_resid_re[ps][pb][1][0] = *h_re++;
      self->m2_resid_im[ps][pb][1][0] = *h_im++;
      self->m2_resid_im[ps][pb][1][1] = 0;

      self->m2_decor_re[ps][pb][0][0] = 0;
      self->m2_decor_im[ps][pb][0][0] = 0;
      self->m2_decor_re[ps][pb][0][1] = *h_re++;
      self->m2_decor_im[ps][pb][0][1] = 0;

      self->m2_decor_re[ps][pb][1][0] = 0;
      self->m2_decor_im[ps][pb][1][0] = 0;
      self->m2_decor_re[ps][pb][1][1] = *h_re++;
      self->m2_decor_im[ps][pb][1][1] = 0;

      self->m2_resid_re[ps][pb][0][1] = *h_re++;
      self->m2_resid_re[ps][pb][1][1] = *h_re++;
    }
  }
  ixheaacd_mps_smoothing_opd(self);
}

static VOID ixheaacd_mps_par2umx_ps_core(WORD32 cld[MAX_PARAMETER_BANDS],
                                         WORD32 icc[MAX_PARAMETER_BANDS],
                                         WORD32 ott_band_count,
                                         FLOAT32 *h_real) {
  WORD32 band;
  FLOAT32 c_l_temp, c_r_temp, temp;
  WORD32 cld_idx, icc_idx;

  for (band = 0; band < ott_band_count; band++) {
    cld_idx = *cld++ + 15;
    icc_idx = *icc++;

    icc_idx = icc_idx & 7;

    c_l_temp = (ixheaacd_c_l_table[cld_idx]);
    c_r_temp = (ixheaacd_c_l_table[30 - cld_idx]);

    temp = ixheaacd_cos_table[icc_idx][cld_idx];
    *h_real++ = MULT(temp, c_l_temp);

    temp = ixheaacd_cos_table[icc_idx][30 - cld_idx];
    *h_real++ = MULT(temp, c_r_temp);

    temp = ixheaacd_sin_table[icc_idx][cld_idx];
    *h_real++ = MULT(temp, c_l_temp);

    temp = -ixheaacd_sin_table[icc_idx][30 - cld_idx];
    *h_real++ = MULT(temp, c_r_temp);

    h_real += 2;
  }
}

VOID ixheaacd_mps_par2umx_ps(ia_mps_dec_state_struct *self,
                             ia_mps_bs_frame *curr_bit_stream, FLOAT32 *h_real,
                             WORD32 param_set_idx) {
  ixheaacd_mps_par2umx_ps_core(curr_bit_stream->cld_idx[param_set_idx],
                               curr_bit_stream->icc_idx[param_set_idx],
                               self->bs_param_bands, h_real);
}

static VOID ixheaacd_mps_opd_calc(ia_mps_dec_state_struct *self,
                                  ia_mps_bs_frame *curr_bit_stream,
                                  WORD32 param_set_idx,
                                  WORD32 opd[MAX_PARAMETER_BANDS]) {
  WORD32 band;

  for (band = 0; band < self->num_bands_ipd; band++) {
    WORD32 cld_idx = curr_bit_stream->cld_idx[param_set_idx][band] + 15;
    WORD32 ipd_idx = (curr_bit_stream->ipd_idx[param_set_idx][band]) & 15;
    WORD32 icc_idx = curr_bit_stream->icc_idx[param_set_idx][band];

    if ((cld_idx == 15) && (ipd_idx == 8))
      opd[band] = 0;
    else
      opd[band] = ixheaacd_atan_table_Q28[ipd_idx][icc_idx][cld_idx];
  }
}

VOID ixheaacd_mps_par2umx_ps_ipd_opd(ia_mps_dec_state_struct *self,
                                     ia_mps_bs_frame *curr_bit_stream,
                                     FLOAT32 *h_real, WORD32 param_set_idx) {
  WORD32 opd[MAX_PARAMETER_BANDS];
  WORD32 ott_band_count = self->bs_param_bands;
  WORD32 num_bands_ipd = self->num_bands_ipd;
  WORD32 band;

  ixheaacd_mps_par2umx_ps_core(curr_bit_stream->cld_idx[param_set_idx],
                               curr_bit_stream->icc_idx[param_set_idx],
                               ott_band_count, h_real);

  if (self->bs_phase_mode) {
    ixheaacd_mps_opd_calc(self, curr_bit_stream, param_set_idx, opd);

    for (band = 0; band < num_bands_ipd; band++) {
      WORD32 ipd_idx = curr_bit_stream->ipd_idx[param_set_idx][band] & 15;
      WORD32 ipd = ixheaacd_ipd_de_quant_table_q28[ipd_idx];

      self->phase_l[param_set_idx][band] =
          ixheaacd_mps_phase_wraping(opd[band]) * ONE_BY_Q28_FLOAT_VAL;
      self->phase_r[param_set_idx][band] =
          ixheaacd_mps_phase_wraping(opd[band] - ipd) * ONE_BY_Q28_FLOAT_VAL;
    }
  } else {
    num_bands_ipd = 0;
  }

  for (band = num_bands_ipd; band < ott_band_count; band++) {
    self->phase_l[param_set_idx][band] = 0;
    self->phase_r[param_set_idx][band] = 0;
  }
}

VOID ixheaacd_mps_par2umx_pred(ia_mps_dec_state_struct *self,
                               ia_mps_bs_frame *curr_bit_stream,
                               FLOAT32 *h_imag, FLOAT32 *h_real,
                               WORD32 param_set_idx, WORD32 res_bands) {
  WORD32 band;

  for (band = 0; band < self->bs_param_bands; band++) {
    WORD32 cld_idx = curr_bit_stream->cld_idx[param_set_idx][band] + 15;
    WORD32 icc_idx = curr_bit_stream->icc_idx[param_set_idx][band];
    WORD32 ipd_idx = curr_bit_stream->ipd_idx[param_set_idx][band] & 15;

    if ((band < self->num_bands_ipd) && (cld_idx == 15) && (icc_idx == 0) &&
        (ipd_idx == 8)) {
      FLOAT32 gain = 0.416666667f;
      *h_imag++ = 0;
      *h_imag++ = 0;

      if (band < res_bands) {
        *h_real++ = gain;
        *h_real++ = gain;
        h_real += 2;

        *h_real++ = gain;
        *h_real++ = -gain;
      } else {
        *h_real++ = gain;
        *h_real++ = -gain;

        h_real += 4;
      }
    } else {
      FLOAT32 weight, re_weight, im_weight;

      weight = ixheaacd_weight[ipd_idx][icc_idx][cld_idx];
      re_weight = ixheaacd_re_weight[ipd_idx][icc_idx][cld_idx];
      im_weight = ixheaacd_im_weight[ipd_idx][icc_idx][cld_idx];

      if (band < self->num_bands_ipd) {
        weight = ixheaacd_weight[ipd_idx][icc_idx][cld_idx];
        re_weight = ixheaacd_re_weight[ipd_idx][icc_idx][cld_idx];
        im_weight = ixheaacd_im_weight[ipd_idx][icc_idx][cld_idx];
      } else {
        weight = ixheaacd_weight[0][icc_idx][cld_idx];
        re_weight = ixheaacd_re_weight[0][icc_idx][cld_idx];
        im_weight = ixheaacd_im_weight[0][icc_idx][cld_idx];
      }

      *h_real++ = weight - re_weight;
      *h_imag++ = -im_weight;
      *h_real++ = weight + re_weight;
      *h_imag++ = im_weight;

      if (band < res_bands) {
        h_real += 2;

        *h_real++ = weight;
        *h_real++ = -weight;
      } else {
        FLOAT32 beta = ixheaacd_beta[ipd_idx][icc_idx][cld_idx];

        *h_real++ = beta;
        *h_real++ = -beta;
        h_real += 2;
      }
    }
  }
}

VOID ixheaacd_mps_apply_pre_matrix(ia_mps_dec_state_struct *self) {
  WORD32 ts, qs, row;
  if (self->pre_mix_req) {
    ixheaacd_mps_upmix_interp_type1(
        self->m1_param_re, self->r_out_re_in_m1, self->m1_param_re_prev,
        (self->dir_sig_count + self->decor_sig_count), 1, self, self->bs_high_rate_mode);

    for (ts = 0; ts < self->time_slots; ts++) {
      for (qs = 0; qs < 2; qs++) {
        WORD32 indx = self->hyb_band_to_processing_band_table[qs];

        FLOAT32 real =
            self->hyb_in[0][qs][ts].re * self->r_out_re_in_m1[ts][indx][0][0];
        FLOAT32 imag =
            self->hyb_in[0][qs][ts].im * self->r_out_re_in_m1[ts][indx][0][0];
        for (row = 0; row < (self->dir_sig_count + self->decor_sig_count);
             row++) {
          self->v[row][ts][qs].re = real;
          self->v[row][ts][qs].im = imag;
        }
      }
      for (qs = 2; qs < self->hyb_band_count[0]; qs++) {
        WORD32 indx = self->hyb_band_to_processing_band_table[qs];
        FLOAT32 real =
            self->hyb_in[0][qs][ts].re * self->r_out_re_in_m1[ts][indx][0][0];
        FLOAT32 imag =
            self->hyb_in[0][qs][ts].im * self->r_out_re_in_m1[ts][indx][0][0];
        for (row = 0; row < (self->dir_sig_count + self->decor_sig_count);
             row++) {
          self->v[row][ts][qs].re = real;
          self->v[row][ts][qs].im = imag;
        }
      }
    }
  } else {
    for (ts = 0; ts < self->time_slots; ts++) {
      for (qs = 0; qs < self->hyb_band_count[0]; qs++) {
        FLOAT32 real = self->hyb_in[0][qs][ts].re;
        FLOAT32 imag = self->hyb_in[0][qs][ts].im;
        for (row = 0; row < (self->dir_sig_count + self->decor_sig_count);
             row++) {
          self->v[row][ts][qs].re = real;
          self->v[row][ts][qs].im = imag;
        }
      }
    }
  }
  return;
}

VOID ixheaacd_mps_apply_mix_matrix(ia_mps_dec_state_struct *self) {
  WORD32 ts, qs, row, col;
  WORD32 complex_m2 = ((self->config->bs_phase_coding != 0));
  WORD32 phase_interpolation = (self->config->bs_phase_coding == 1);
  WORD32 num_col_iters = 0;

  ixheaacd_mps_upmix_interp(self->m2_decor_re, self->r_out_diff_re_in_m2, self->m2_decor_re_prev,
                            self->out_ch_count, (self->dir_sig_count + self->decor_sig_count),
                            self, 1);

  ixheaacd_mps_upmix_interp(self->m2_resid_re, self->r_out_re_in_m2, self->m2_resid_re_prev,
                            self->out_ch_count, (self->dir_sig_count + self->decor_sig_count),
                            self, 1);

  if (complex_m2 && !phase_interpolation) {
    ixheaacd_mps_upmix_interp(self->m2_decor_im, self->r_out_diff_im_in_m2,
                              self->m2_decor_im_prev, self->out_ch_count,
                              (self->dir_sig_count + self->decor_sig_count), self, 1);
    ixheaacd_mps_upmix_interp(self->m2_resid_im, self->r_out_im_in_m2, self->m2_resid_im_prev,
                              self->out_ch_count, (self->dir_sig_count + self->decor_sig_count),
                              self, 1);
  }

  if (phase_interpolation) {
    ixheaacd_mps_phase_interpolation(
        self->phase_l, self->phase_r, self->phase_l_prev, self->phase_r_prev,
        self->r_out_ph_re_in_m2, self->r_out_ph_im_in_m2, self);

    for (ts = 0; ts < self->time_slots; ts++) {
      WORD32 pb;
      for (pb = 0; pb < self->bs_param_bands; pb++) {
        self->r_out_im_in_m2[ts][pb][0][0] =
              self->r_out_re_in_m2[ts][pb][0][0] *
              self->r_out_ph_im_in_m2[ts][pb][0];

        self->r_out_im_in_m2[ts][pb][0][1] =
              self->r_out_re_in_m2[ts][pb][0][1] *
              self->r_out_ph_im_in_m2[ts][pb][0];

        self->r_out_im_in_m2[ts][pb][1][0] =
              self->r_out_re_in_m2[ts][pb][1][0] *
              self->r_out_ph_im_in_m2[ts][pb][1];

        self->r_out_im_in_m2[ts][pb][1][1] =
              self->r_out_re_in_m2[ts][pb][1][1] *
              self->r_out_ph_im_in_m2[ts][pb][1];

        self->r_out_re_in_m2[ts][pb][0][0] =
              self->r_out_re_in_m2[ts][pb][0][0] *
              self->r_out_ph_re_in_m2[ts][pb][0];

        self->r_out_re_in_m2[ts][pb][0][1] =
              self->r_out_re_in_m2[ts][pb][0][1] *
              self->r_out_ph_re_in_m2[ts][pb][0];

        self->r_out_re_in_m2[ts][pb][1][0] =
              self->r_out_re_in_m2[ts][pb][1][0] *
              self->r_out_ph_re_in_m2[ts][pb][1];

        self->r_out_re_in_m2[ts][pb][1][1] =
              self->r_out_re_in_m2[ts][pb][1][1] *
              self->r_out_ph_re_in_m2[ts][pb][1];

        self->r_out_diff_im_in_m2[ts][pb][0][0] = 0;
        self->r_out_diff_im_in_m2[ts][pb][0][1] =
              self->r_out_diff_re_in_m2[ts][pb][0][1] *
              self->r_out_ph_im_in_m2[ts][pb][0];

        self->r_out_diff_im_in_m2[ts][pb][1][0] = 0;
        self->r_out_diff_im_in_m2[ts][pb][1][1] =
              self->r_out_diff_re_in_m2[ts][pb][1][1] *
              self->r_out_ph_im_in_m2[ts][pb][1];

        self->r_out_diff_re_in_m2[ts][pb][0][0] = 0;
        self->r_out_diff_re_in_m2[ts][pb][0][1] =
              self->r_out_diff_re_in_m2[ts][pb][0][1] *
              self->r_out_ph_re_in_m2[ts][pb][0];

        self->r_out_diff_re_in_m2[ts][pb][1][0] = 0;
        self->r_out_diff_re_in_m2[ts][pb][1][1] =
              self->r_out_diff_re_in_m2[ts][pb][1][1] *
              self->r_out_ph_re_in_m2[ts][pb][1];
      }
    }
  }
  if (self->res_bands == 0) {
    num_col_iters = self->dir_sig_count;
  }
  else {
    num_col_iters = (self->dir_sig_count + self->decor_sig_count);
  }

  for (ts = 0; ts < self->time_slots; ts++) {
    for (qs = 0; qs < self->hyb_band_count_max; qs++) {
      WORD32 indx = self->hyb_band_to_processing_band_table[qs];

      for (row = 0; row < self->out_ch_count; row++) {
          FLOAT32 sum_re_dir = 0;
          FLOAT32 sum_im_dir = 0;
          for (col = 0; col < num_col_iters; col++) {
              sum_re_dir += self->w_dir[col][ts][qs].re *
                  self->r_out_re_in_m2[ts][indx][row][col];
              sum_im_dir += self->w_dir[col][ts][qs].im *
                  self->r_out_re_in_m2[ts][indx][row][col];
          }
          self->hyb_dir_out[row][ts][qs].re = sum_re_dir;
          self->hyb_dir_out[row][ts][qs].im = sum_im_dir;

          self->hyb_diff_out[row][ts][qs].re =
              self->w_diff[1][ts][qs].re *
              self->r_out_diff_re_in_m2[ts][indx][row][1];
          self->hyb_diff_out[row][ts][qs].im =
              self->w_diff[1][ts][qs].im *
              self->r_out_diff_re_in_m2[ts][indx][row][1];
      }
    }
  }

  if (complex_m2) {
    if (phase_interpolation) {
      for (ts = 0; ts < self->time_slots; ts++) {
        for (qs = 0; qs < 2; qs++) {
          WORD32 indx = self->hyb_band_to_processing_band_table[qs];
          for (row = 0; row < self->out_ch_count; row++) {
              FLOAT32 sum_re_dir = self->hyb_dir_out[row][ts][qs].re;
              FLOAT32 sum_im_dir = self->hyb_dir_out[row][ts][qs].im;
              for (col = 0; col < num_col_iters; col++) {
                  sum_re_dir += self->w_dir[col][ts][qs].im *
                      self->r_out_im_in_m2[ts][indx][row][col];
                  sum_im_dir -= self->w_dir[col][ts][qs].re *
                      self->r_out_im_in_m2[ts][indx][row][col];
              }
              self->hyb_dir_out[row][ts][qs].re = sum_re_dir;
              self->hyb_dir_out[row][ts][qs].im = sum_im_dir;
              self->hyb_diff_out[row][ts][qs].re +=
                  self->w_diff[1][ts][qs].im *
                  self->r_out_diff_im_in_m2[ts][indx][row][1];
              self->hyb_diff_out[row][ts][qs].im -=
                  self->w_diff[1][ts][qs].re *
                  self->r_out_diff_im_in_m2[ts][indx][row][1];
          }
        }
        for (qs = 2; qs < self->hyb_band_count_max; qs++) {
          WORD32 indx = self->hyb_band_to_processing_band_table[qs];
          for (row = 0; row < self->out_ch_count; row++) {
              FLOAT32 sum_re_dir = self->hyb_dir_out[row][ts][qs].re;
              FLOAT32 sum_im_dir = self->hyb_dir_out[row][ts][qs].im;
              for (col = 0; col < num_col_iters; col++) {
                  sum_re_dir -= self->w_dir[col][ts][qs].im *
                      self->r_out_im_in_m2[ts][indx][row][col];
                  sum_im_dir += self->w_dir[col][ts][qs].re *
                      self->r_out_im_in_m2[ts][indx][row][col];
              }
              self->hyb_dir_out[row][ts][qs].re = sum_re_dir;
              self->hyb_dir_out[row][ts][qs].im = sum_im_dir;
              self->hyb_diff_out[row][ts][qs].re -=
                  self->w_diff[1][ts][qs].im *
                  self->r_out_diff_im_in_m2[ts][indx][row][1];
              self->hyb_diff_out[row][ts][qs].im +=
                  self->w_diff[1][ts][qs].re *
                  self->r_out_diff_im_in_m2[ts][indx][row][1];
          }
        }
      }
    }
    else {
      WORD32 num_cols = (self->dir_sig_count + self->decor_sig_count) > 1
              ? 1
              : (self->dir_sig_count + self->decor_sig_count);
      for (ts = 0; ts < self->time_slots; ts++) {
        for (qs = 0; qs < 2; qs++) {
          WORD32 indx = self->hyb_band_to_processing_band_table[qs];
          for (row = 0; row < self->out_ch_count; row++) {
            FLOAT32 sum_re_dir = self->hyb_dir_out[row][ts][qs].re;
            FLOAT32 sum_im_dir = self->hyb_dir_out[row][ts][qs].im;
            if (num_cols > 0) {
              sum_re_dir += self->w_dir[0][ts][qs].im *
                  self->r_out_im_in_m2[ts][indx][row][0];
              sum_im_dir -= self->w_dir[0][ts][qs].re *
                  self->r_out_im_in_m2[ts][indx][row][0];
            }
            self->hyb_dir_out[row][ts][qs].re = sum_re_dir;
            self->hyb_dir_out[row][ts][qs].im = sum_im_dir;
          }
        }
        for (qs = 2; qs < self->hyb_band_count_max; qs++) {
          WORD32 indx = self->hyb_band_to_processing_band_table[qs];
          for (row = 0; row < self->out_ch_count; row++) {
            FLOAT32 sum_re_dir = self->hyb_dir_out[row][ts][qs].re;
            FLOAT32 sum_im_dir = self->hyb_dir_out[row][ts][qs].im;
            if (num_cols > 0) {
              sum_re_dir -= self->w_dir[0][ts][qs].im *
                              self->r_out_im_in_m2[ts][indx][row][0];
              sum_im_dir += self->w_dir[0][ts][qs].re *
                              self->r_out_im_in_m2[ts][indx][row][0];
            }
            self->hyb_dir_out[row][ts][qs].re = sum_re_dir;
            self->hyb_dir_out[row][ts][qs].im = sum_im_dir;
          }
        }
      }
    }
  }
  return;
}

VOID ixheaacd_mps_apply_mix_matrix_type1(ia_mps_dec_state_struct *self) {
  WORD32 ts, qs, row;

  ixheaacd_mps_upmix_interp_type2(self->m2_decor_re, self->r_out_diff_re_in_m2,
                                  self->m2_decor_re_prev, self->out_ch_count, self, 1);

  ixheaacd_mps_upmix_interp_type2(self->m2_resid_re, self->r_out_re_in_m2, self->m2_resid_re_prev,
                                  self->out_ch_count, self, 0);

  for (qs = 0; qs < self->hyb_band_count[0]; qs++) {
    WORD32 indx = self->hyb_band_to_processing_band_table[qs];
    for (ts = 0; ts < self->time_slots; ts++) {
      for (row = 0; row < self->out_ch_count; row++) {
        self->hyb_dir_out[row][ts][qs].re =
            self->w_dir[0][ts][qs].re * self->r_out_re_in_m2[ts][indx][row][0];
        self->hyb_dir_out[row][ts][qs].im =
            self->w_dir[0][ts][qs].im * self->r_out_re_in_m2[ts][indx][row][0];
        self->hyb_diff_out[row][ts][qs].re =
            self->w_diff[1][ts][qs].re *
            self->r_out_diff_re_in_m2[ts][indx][row][1];
        self->hyb_diff_out[row][ts][qs].im =
            self->w_diff[1][ts][qs].im *
            self->r_out_diff_re_in_m2[ts][indx][row][1];
      }
    }
  }
  return;
}

VOID ixheaacd_mps_apply_mix_matrix_type2(ia_mps_dec_state_struct *self) {
  WORD32 ts, qs, row, col;
  WORD32 complex_m2 = ((self->config->bs_phase_coding != 0));
  WORD32 phase_interpolation = (self->config->bs_phase_coding == 1);
  WORD32 num_col_iters = 0;

  ixheaacd_mps_upmix_interp_type1(self->m2_decor_re, self->r_out_diff_re_in_m2,
                                  self->m2_decor_re_prev, self->out_ch_count,
                                  (self->dir_sig_count + self->decor_sig_count), self, 1);
  ixheaacd_mps_upmix_interp_type1(self->m2_resid_re, self->r_out_re_in_m2, self->m2_resid_re_prev,
                                  self->out_ch_count,
                                  (self->dir_sig_count + self->decor_sig_count), self, 1);

  if (complex_m2 && !phase_interpolation) {
    ixheaacd_mps_upmix_interp_type1(self->m2_decor_im, self->r_out_diff_im_in_m2,
                                    self->m2_decor_im_prev, self->out_ch_count,
                                    (self->dir_sig_count + self->decor_sig_count), self, 1);
    ixheaacd_mps_upmix_interp_type1(self->m2_resid_im, self->r_out_im_in_m2,
                                    self->m2_resid_im_prev, self->out_ch_count,
                                    (self->dir_sig_count + self->decor_sig_count), self, 1);
  }

  if (phase_interpolation) {
    ixheaacd_mps_phase_interpolation(
        self->phase_l, self->phase_r, self->phase_l_prev, self->phase_r_prev,
        self->r_out_ph_re_in_m2, self->r_out_ph_im_in_m2, self);

    for (ts = 0; ts < self->time_slots; ts++) {
      WORD32 pb;
      for (pb = 0; pb < self->bs_param_bands; pb++) {
        self->r_out_im_in_m2[ts][pb][0][0] =
            self->r_out_re_in_m2[ts][pb][0][0] *
            self->r_out_ph_im_in_m2[ts][pb][0];

        self->r_out_im_in_m2[ts][pb][0][1] =
            self->r_out_re_in_m2[ts][pb][0][1] *
            self->r_out_ph_im_in_m2[ts][pb][0];
        self->r_out_im_in_m2[ts][pb][1][0] =
            self->r_out_re_in_m2[ts][pb][1][0] *
            self->r_out_ph_im_in_m2[ts][pb][1];

        self->r_out_im_in_m2[ts][pb][1][1] =
            self->r_out_re_in_m2[ts][pb][1][1] *
            self->r_out_ph_im_in_m2[ts][pb][1];

        self->r_out_re_in_m2[ts][pb][0][0] =
            self->r_out_re_in_m2[ts][pb][0][0] *
            self->r_out_ph_re_in_m2[ts][pb][0];

        self->r_out_re_in_m2[ts][pb][0][1] =
            self->r_out_re_in_m2[ts][pb][0][1] *
            self->r_out_ph_re_in_m2[ts][pb][0];

        self->r_out_re_in_m2[ts][pb][1][0] =
            self->r_out_re_in_m2[ts][pb][1][0] *
            self->r_out_ph_re_in_m2[ts][pb][1];

        self->r_out_re_in_m2[ts][pb][1][1] =
            self->r_out_re_in_m2[ts][pb][1][1] *
            self->r_out_ph_re_in_m2[ts][pb][1];

        self->r_out_diff_im_in_m2[ts][pb][0][0] = 0;
        self->r_out_diff_im_in_m2[ts][pb][0][1] =
            self->r_out_diff_re_in_m2[ts][pb][0][1] *
            self->r_out_ph_im_in_m2[ts][pb][0];

        self->r_out_diff_im_in_m2[ts][pb][1][0] = 0;
        self->r_out_diff_im_in_m2[ts][pb][1][1] =
            self->r_out_diff_re_in_m2[ts][pb][1][1] *
            self->r_out_ph_im_in_m2[ts][pb][1];

        self->r_out_diff_re_in_m2[ts][pb][0][0] = 0;
        self->r_out_diff_re_in_m2[ts][pb][0][1] =
            self->r_out_diff_re_in_m2[ts][pb][0][1] *
            self->r_out_ph_re_in_m2[ts][pb][0];

        self->r_out_diff_re_in_m2[ts][pb][1][0] = 0;
        self->r_out_diff_re_in_m2[ts][pb][1][1] =
            self->r_out_diff_re_in_m2[ts][pb][1][1] *
            self->r_out_ph_re_in_m2[ts][pb][1];
      }
    }
  }
  if (self->res_bands == 0) {
    num_col_iters = self->dir_sig_count;
  } else {
    num_col_iters = (self->dir_sig_count + self->decor_sig_count);
  }
  for (ts = 0; ts < self->time_slots; ts++) {
    for (qs = 0; qs < self->hyb_band_count_max; qs++) {
      WORD32 indx = self->hyb_band_to_processing_band_table[qs];

      for (row = 0; row < self->out_ch_count; row++) {
        FLOAT32 sum_re_dir = 0;
        FLOAT32 sum_im_dir = 0;
        for (col = 0; col < num_col_iters; col++) {
          sum_re_dir += self->w_dir[col][ts][qs].re *
                        self->r_out_re_in_m2[ts][indx][row][col];
          sum_im_dir += self->w_dir[col][ts][qs].im *
                        self->r_out_re_in_m2[ts][indx][row][col];
        }
        self->hyb_dir_out[row][ts][qs].re = sum_re_dir;
        self->hyb_dir_out[row][ts][qs].im = sum_im_dir;

        self->hyb_diff_out[row][ts][qs].re =
            self->w_diff[1][ts][qs].re *
            self->r_out_diff_re_in_m2[ts][indx][row][1];
        self->hyb_diff_out[row][ts][qs].im =
            self->w_diff[1][ts][qs].im *
            self->r_out_diff_re_in_m2[ts][indx][row][1];
      }
    }
  }

  if (complex_m2) {
    if (phase_interpolation) {
      for (ts = 0; ts < self->time_slots; ts++) {
        for (qs = 0; qs < 2; qs++) {
          WORD32 indx = self->hyb_band_to_processing_band_table[qs];
          for (row = 0; row < self->out_ch_count; row++) {
            FLOAT32 sum_re_dir = self->hyb_dir_out[row][ts][qs].re;
            FLOAT32 sum_im_dir = self->hyb_dir_out[row][ts][qs].im;
            for (col = 0; col < num_col_iters; col++) {
              sum_re_dir += self->w_dir[col][ts][qs].im *
                            self->r_out_im_in_m2[ts][indx][row][col];
              sum_im_dir -= self->w_dir[col][ts][qs].re *
                            self->r_out_im_in_m2[ts][indx][row][col];
            }
            self->hyb_dir_out[row][ts][qs].re = sum_re_dir;
            self->hyb_dir_out[row][ts][qs].im = sum_im_dir;
            self->hyb_diff_out[row][ts][qs].re +=
                self->w_diff[1][ts][qs].im *
                self->r_out_diff_im_in_m2[ts][indx][row][1];
            self->hyb_diff_out[row][ts][qs].im -=
                self->w_diff[1][ts][qs].re *
                self->r_out_diff_im_in_m2[ts][indx][row][1];
          }
        }
        for (qs = 2; qs < self->hyb_band_count_max; qs++) {
          WORD32 indx = self->hyb_band_to_processing_band_table[qs];
          for (row = 0; row < self->out_ch_count; row++) {
            FLOAT32 sum_re_dir = self->hyb_dir_out[row][ts][qs].re;
            FLOAT32 sum_im_dir = self->hyb_dir_out[row][ts][qs].im;
            for (col = 0; col < num_col_iters; col++) {
              sum_re_dir -= self->w_dir[col][ts][qs].im *
                            self->r_out_im_in_m2[ts][indx][row][col];
              sum_im_dir += self->w_dir[col][ts][qs].re *
                            self->r_out_im_in_m2[ts][indx][row][col];
            }
            self->hyb_dir_out[row][ts][qs].re = sum_re_dir;
            self->hyb_dir_out[row][ts][qs].im = sum_im_dir;
            self->hyb_diff_out[row][ts][qs].re -=
                self->w_diff[1][ts][qs].im *
                self->r_out_diff_im_in_m2[ts][indx][row][1];
            self->hyb_diff_out[row][ts][qs].im +=
                self->w_diff[1][ts][qs].re *
                self->r_out_diff_im_in_m2[ts][indx][row][1];
          }
        }
      }
    } else {
      WORD32 num_cols = (self->dir_sig_count + self->decor_sig_count) > 1
                         ? 1
                         : (self->dir_sig_count + self->decor_sig_count);
      for (ts = 0; ts < self->time_slots; ts++) {
        for (qs = 0; qs < 2; qs++) {
          WORD32 indx = self->hyb_band_to_processing_band_table[qs];
          for (row = 0; row < self->out_ch_count; row++) {
            FLOAT32 sum_re_dir = self->hyb_dir_out[row][ts][qs].re;
            FLOAT32 sum_im_dir = self->hyb_dir_out[row][ts][qs].im;
            if (num_cols > 0) {
              sum_re_dir += self->w_dir[0][ts][qs].im *
                            self->r_out_im_in_m2[ts][indx][row][0];
              sum_im_dir -= self->w_dir[0][ts][qs].re *
                            self->r_out_im_in_m2[ts][indx][row][0];
            }
            self->hyb_dir_out[row][ts][qs].re = sum_re_dir;
            self->hyb_dir_out[row][ts][qs].im = sum_im_dir;
          }
        }
        for (qs = 2; qs < self->hyb_band_count_max; qs++) {
          WORD32 indx = self->hyb_band_to_processing_band_table[qs];
          for (row = 0; row < self->out_ch_count; row++) {
            FLOAT32 sum_re_dir = self->hyb_dir_out[row][ts][qs].re;
            FLOAT32 sum_im_dir = self->hyb_dir_out[row][ts][qs].im;
            if (num_cols > 0) {
              sum_re_dir -= self->w_dir[0][ts][qs].im *
                            self->r_out_im_in_m2[ts][indx][row][0];
              sum_im_dir += self->w_dir[0][ts][qs].re *
                            self->r_out_im_in_m2[ts][indx][row][0];
            }
            self->hyb_dir_out[row][ts][qs].re = sum_re_dir;
            self->hyb_dir_out[row][ts][qs].im = sum_im_dir;
          }
        }
      }
    }
  }
  return;
}

VOID ixheaacd_mps_apply_mix_matrix_type3(ia_mps_dec_state_struct *self) {
  WORD32 ts, qs, row, col;
  WORD32 complex_m2 = ((self->config->bs_phase_coding != 0));
  WORD32 phase_interpolation = (self->config->bs_phase_coding == 1);
  WORD32 num_col_iters = 0;

  if (self->res_bands != 28) {
    ixheaacd_mps_upmix_interp_type2(self->m2_decor_re, self->r_out_diff_re_in_m2,
                                    self->m2_decor_re_prev, self->out_ch_count, self, 1);
  }
  if (self->res_bands == 0) {
    num_col_iters = self->dir_sig_count;
    ixheaacd_mps_upmix_interp_type2(self->m2_resid_re, self->r_out_re_in_m2,
                                    self->m2_resid_re_prev, self->out_ch_count, self, 0);
  } else {
    num_col_iters = (self->dir_sig_count + self->decor_sig_count);
    ixheaacd_mps_upmix_interp_type1(self->m2_resid_re, self->r_out_re_in_m2,
                                    self->m2_resid_re_prev, self->out_ch_count,
                                    (self->dir_sig_count + self->decor_sig_count), self, 1);
  }

  if (complex_m2 && !phase_interpolation) {
    ixheaacd_mps_upmix_interp_type2(self->m2_resid_im, self->r_out_im_in_m2,
                                    self->m2_resid_im_prev, self->out_ch_count, self, 0);
  }

  if (phase_interpolation) {
    ixheaacd_mps_phase_interpolation(
        self->phase_l, self->phase_r, self->phase_l_prev, self->phase_r_prev,
        self->r_out_ph_re_in_m2, self->r_out_ph_im_in_m2, self);

    if (self->res_bands == 0) {
      for (ts = 0; ts < self->time_slots; ts++) {
        WORD32 pb;
        for (pb = 0; pb < self->bs_param_bands; pb++) {
          self->r_out_im_in_m2[ts][pb][0][0] =
              self->r_out_re_in_m2[ts][pb][0][0] *
              self->r_out_ph_im_in_m2[ts][pb][0];

          self->r_out_im_in_m2[ts][pb][1][0] =
              self->r_out_re_in_m2[ts][pb][1][0] *
              self->r_out_ph_im_in_m2[ts][pb][1];

          self->r_out_re_in_m2[ts][pb][0][0] =
              self->r_out_re_in_m2[ts][pb][0][0] *
              self->r_out_ph_re_in_m2[ts][pb][0];

          self->r_out_re_in_m2[ts][pb][1][0] =
              self->r_out_re_in_m2[ts][pb][1][0] *
              self->r_out_ph_re_in_m2[ts][pb][1];

          self->r_out_diff_im_in_m2[ts][pb][0][1] =
              self->r_out_diff_re_in_m2[ts][pb][0][1] *
              self->r_out_ph_im_in_m2[ts][pb][0];

          self->r_out_diff_im_in_m2[ts][pb][1][1] =
              self->r_out_diff_re_in_m2[ts][pb][1][1] *
              self->r_out_ph_im_in_m2[ts][pb][1];

          self->r_out_diff_re_in_m2[ts][pb][0][1] =
              self->r_out_diff_re_in_m2[ts][pb][0][1] *
              self->r_out_ph_re_in_m2[ts][pb][0];

          self->r_out_diff_re_in_m2[ts][pb][1][1] =
              self->r_out_diff_re_in_m2[ts][pb][1][1] *
              self->r_out_ph_re_in_m2[ts][pb][1];
        }
      }
    } else if (self->res_bands == 28) {
      for (ts = 0; ts < self->time_slots; ts++) {
        WORD32 pb;
        for (pb = 0; pb < self->bs_param_bands; pb++) {
          self->r_out_im_in_m2[ts][pb][0][0] =
              self->r_out_re_in_m2[ts][pb][0][0] *
              self->r_out_ph_im_in_m2[ts][pb][0];

          self->r_out_im_in_m2[ts][pb][0][1] =
              self->r_out_re_in_m2[ts][pb][0][1] *
              self->r_out_ph_im_in_m2[ts][pb][0];

          self->r_out_im_in_m2[ts][pb][1][0] =
              self->r_out_re_in_m2[ts][pb][1][0] *
              self->r_out_ph_im_in_m2[ts][pb][1];

          self->r_out_im_in_m2[ts][pb][1][1] =
              self->r_out_re_in_m2[ts][pb][1][1] *
              self->r_out_ph_im_in_m2[ts][pb][1];

          self->r_out_re_in_m2[ts][pb][0][0] =
              self->r_out_re_in_m2[ts][pb][0][0] *
              self->r_out_ph_re_in_m2[ts][pb][0];

          self->r_out_re_in_m2[ts][pb][0][1] =
              self->r_out_re_in_m2[ts][pb][0][1] *
              self->r_out_ph_re_in_m2[ts][pb][0];

          self->r_out_re_in_m2[ts][pb][1][0] =
              self->r_out_re_in_m2[ts][pb][1][0] *
              self->r_out_ph_re_in_m2[ts][pb][1];

          self->r_out_re_in_m2[ts][pb][1][1] =
              self->r_out_re_in_m2[ts][pb][1][1] *
              self->r_out_ph_re_in_m2[ts][pb][1];
        }
      }
    } else {
      for (ts = 0; ts < self->time_slots; ts++) {
        WORD32 pb;
        for (pb = 0; pb < self->bs_param_bands; pb++) {
          self->r_out_im_in_m2[ts][pb][0][0] =
              self->r_out_re_in_m2[ts][pb][0][0] *
              self->r_out_ph_im_in_m2[ts][pb][0];

          self->r_out_im_in_m2[ts][pb][0][1] =
              self->r_out_re_in_m2[ts][pb][0][1] *
              self->r_out_ph_im_in_m2[ts][pb][0];

          self->r_out_im_in_m2[ts][pb][1][0] =
              self->r_out_re_in_m2[ts][pb][1][0] *
              self->r_out_ph_im_in_m2[ts][pb][1];

          self->r_out_im_in_m2[ts][pb][1][1] =
              self->r_out_re_in_m2[ts][pb][1][1] *
              self->r_out_ph_im_in_m2[ts][pb][1];

          self->r_out_re_in_m2[ts][pb][0][0] =
              self->r_out_re_in_m2[ts][pb][0][0] *
              self->r_out_ph_re_in_m2[ts][pb][0];

          self->r_out_re_in_m2[ts][pb][0][1] =
              self->r_out_re_in_m2[ts][pb][0][1] *
              self->r_out_ph_re_in_m2[ts][pb][0];

          self->r_out_re_in_m2[ts][pb][1][0] =
              self->r_out_re_in_m2[ts][pb][1][0] *
              self->r_out_ph_re_in_m2[ts][pb][1];

          self->r_out_re_in_m2[ts][pb][1][1] =
              self->r_out_re_in_m2[ts][pb][1][1] *
              self->r_out_ph_re_in_m2[ts][pb][1];

          self->r_out_diff_im_in_m2[ts][pb][0][1] =
              self->r_out_diff_re_in_m2[ts][pb][0][1] *
              self->r_out_ph_im_in_m2[ts][pb][0];

          self->r_out_diff_im_in_m2[ts][pb][1][1] =
              self->r_out_diff_re_in_m2[ts][pb][1][1] *
              self->r_out_ph_im_in_m2[ts][pb][1];

          self->r_out_diff_re_in_m2[ts][pb][0][1] =
              self->r_out_diff_re_in_m2[ts][pb][0][1] *
              self->r_out_ph_re_in_m2[ts][pb][0];

          self->r_out_diff_re_in_m2[ts][pb][1][1] =
              self->r_out_diff_re_in_m2[ts][pb][1][1] *
              self->r_out_ph_re_in_m2[ts][pb][1];
        }
      }
    }
  }
  if (self->res_bands == 0) {
    for (ts = 0; ts < self->time_slots; ts++) {
      for (qs = 0; qs < self->hyb_band_count[0]; qs++) {
        WORD32 indx = self->hyb_band_to_processing_band_table[qs];
        for (row = 0; row < self->out_ch_count; row++) {
          self->hyb_dir_out[row][ts][qs].re =
              self->w_dir[0][ts][qs].re *
              self->r_out_re_in_m2[ts][indx][row][0];
          self->hyb_dir_out[row][ts][qs].im =
              self->w_dir[0][ts][qs].im *
              self->r_out_re_in_m2[ts][indx][row][0];
          self->hyb_diff_out[row][ts][qs].re =
              self->w_diff[1][ts][qs].re *
              self->r_out_diff_re_in_m2[ts][indx][row][1];
          self->hyb_diff_out[row][ts][qs].im =
              self->w_diff[1][ts][qs].im *
              self->r_out_diff_re_in_m2[ts][indx][row][1];
        }
      }
    }
  } else if (self->res_bands == 28) {
    for (ts = 0; ts < self->time_slots; ts++) {
      for (qs = 0; qs < self->hyb_band_count[1]; qs++) {
        WORD32 indx = self->hyb_band_to_processing_band_table[qs];
        for (row = 0; row < self->out_ch_count; row++) {
          FLOAT32 sum_re_dir = 0;
          FLOAT32 sum_im_dir = 0;
          for (col = 0; col < num_col_iters; col++) {
            sum_re_dir += self->w_dir[col][ts][qs].re *
                          self->r_out_re_in_m2[ts][indx][row][col];
            sum_im_dir += self->w_dir[col][ts][qs].im *
                          self->r_out_re_in_m2[ts][indx][row][col];
          }
          self->hyb_dir_out[row][ts][qs].re = sum_re_dir;
          self->hyb_dir_out[row][ts][qs].im = sum_im_dir;
        }
      }
      for (; qs < self->hyb_band_count[0]; qs++) {
        WORD32 indx = self->hyb_band_to_processing_band_table[qs];
        for (row = 0; row < self->out_ch_count; row++) {
          self->hyb_dir_out[row][ts][qs].re =
              self->w_dir[0][ts][qs].re *
              self->r_out_re_in_m2[ts][indx][row][0];
          self->hyb_dir_out[row][ts][qs].im =
              self->w_dir[0][ts][qs].im *
              self->r_out_re_in_m2[ts][indx][row][0];
        }
      }
    }
  } else {
    WORD32 dif_s = ixheaacd_mps_gain_set_indx[self->res_bands];
    for (ts = 0; ts < self->time_slots; ts++) {
      for (qs = 0; qs < dif_s; qs++) {
        WORD32 indx = self->hyb_band_to_processing_band_table[qs];
        for (row = 0; row < self->out_ch_count; row++) {
          FLOAT32 sum_re_dir = 0;
          FLOAT32 sum_im_dir = 0;
          for (col = 0; col < num_col_iters; col++) {
            sum_re_dir += self->w_dir[col][ts][qs].re *
                          self->r_out_re_in_m2[ts][indx][row][col];
            sum_im_dir += self->w_dir[col][ts][qs].im *
                          self->r_out_re_in_m2[ts][indx][row][col];
          }
          self->hyb_dir_out[row][ts][qs].re = sum_re_dir;
          self->hyb_dir_out[row][ts][qs].im = sum_im_dir;
        }
      }
      for (; qs < self->hyb_band_count[1]; qs++) {
        WORD32 indx = self->hyb_band_to_processing_band_table[qs];
        for (row = 0; row < self->out_ch_count; row++) {
          FLOAT32 sum_re_dir = 0;
          FLOAT32 sum_im_dir = 0;
          for (col = 0; col < num_col_iters; col++) {
            sum_re_dir += self->w_dir[col][ts][qs].re *
                          self->r_out_re_in_m2[ts][indx][row][col];
            sum_im_dir += self->w_dir[col][ts][qs].im *
                          self->r_out_re_in_m2[ts][indx][row][col];
          }
          self->hyb_dir_out[row][ts][qs].re = sum_re_dir;
          self->hyb_dir_out[row][ts][qs].im = sum_im_dir;
          self->hyb_diff_out[row][ts][qs].re =
              self->w_diff[1][ts][qs].re *
              self->r_out_diff_re_in_m2[ts][indx][row][1];
          self->hyb_diff_out[row][ts][qs].im =
              self->w_diff[1][ts][qs].im *
              self->r_out_diff_re_in_m2[ts][indx][row][1];
        }
      }
      for (; qs < self->hyb_band_count[0]; qs++) {
        WORD32 indx = self->hyb_band_to_processing_band_table[qs];
        for (row = 0; row < self->out_ch_count; row++) {
          self->hyb_dir_out[row][ts][qs].re =
              self->w_dir[0][ts][qs].re *
              self->r_out_re_in_m2[ts][indx][row][0];
          self->hyb_dir_out[row][ts][qs].im =
              self->w_dir[0][ts][qs].im *
              self->r_out_re_in_m2[ts][indx][row][0];
          self->hyb_diff_out[row][ts][qs].re =
              self->w_diff[1][ts][qs].re *
              self->r_out_diff_re_in_m2[ts][indx][row][1];
          self->hyb_diff_out[row][ts][qs].im =
              self->w_diff[1][ts][qs].im *
              self->r_out_diff_re_in_m2[ts][indx][row][1];
        }
      }
    }
  }

  if (complex_m2) {
    if (phase_interpolation) {
      for (ts = 0; ts < self->time_slots; ts++) {
        for (qs = 0; qs < 2; qs++) {
          WORD32 indx = self->hyb_band_to_processing_band_table[qs];
          for (row = 0; row < self->out_ch_count; row++) {
            FLOAT32 sum_re_dir = self->hyb_dir_out[row][ts][qs].re;
            FLOAT32 sum_im_dir = self->hyb_dir_out[row][ts][qs].im;
            for (col = 0; col < num_col_iters; col++) {
              sum_re_dir += self->w_dir[col][ts][qs].im *
                            self->r_out_im_in_m2[ts][indx][row][col];
              sum_im_dir -= self->w_dir[col][ts][qs].re *
                            self->r_out_im_in_m2[ts][indx][row][col];
            }
            self->hyb_dir_out[row][ts][qs].re = sum_re_dir;
            self->hyb_dir_out[row][ts][qs].im = sum_im_dir;
            self->hyb_diff_out[row][ts][qs].re +=
                self->w_diff[1][ts][qs].im *
                self->r_out_diff_im_in_m2[ts][indx][row][1];
            self->hyb_diff_out[row][ts][qs].im -=
                self->w_diff[1][ts][qs].re *
                self->r_out_diff_im_in_m2[ts][indx][row][1];
          }
        }
        for (qs = 2; qs < self->hyb_band_count_max; qs++) {
          WORD32 indx = self->hyb_band_to_processing_band_table[qs];
          for (row = 0; row < self->out_ch_count; row++) {
            FLOAT32 sum_re_dir = self->hyb_dir_out[row][ts][qs].re;
            FLOAT32 sum_im_dir = self->hyb_dir_out[row][ts][qs].im;
            for (col = 0; col < num_col_iters; col++) {
              sum_re_dir -= self->w_dir[col][ts][qs].im *
                            self->r_out_im_in_m2[ts][indx][row][col];
              sum_im_dir += self->w_dir[col][ts][qs].re *
                            self->r_out_im_in_m2[ts][indx][row][col];
            }
            self->hyb_dir_out[row][ts][qs].re = sum_re_dir;
            self->hyb_dir_out[row][ts][qs].im = sum_im_dir;
            self->hyb_diff_out[row][ts][qs].re -=
                self->w_diff[1][ts][qs].im *
                self->r_out_diff_im_in_m2[ts][indx][row][1];
            self->hyb_diff_out[row][ts][qs].im +=
                self->w_diff[1][ts][qs].re *
                self->r_out_diff_im_in_m2[ts][indx][row][1];
          }
        }
      }
    } else {
      WORD32 num_cols = (self->dir_sig_count + self->decor_sig_count) > 1
                         ? 1
                         : (self->dir_sig_count + self->decor_sig_count);
      for (ts = 0; ts < self->time_slots; ts++) {
        for (qs = 0; qs < 2; qs++) {
          WORD32 indx = self->hyb_band_to_processing_band_table[qs];
          for (row = 0; row < self->out_ch_count; row++) {
            FLOAT32 sum_re_dir = self->hyb_dir_out[row][ts][qs].re;
            FLOAT32 sum_im_dir = self->hyb_dir_out[row][ts][qs].im;
            if (num_cols > 0) {
              sum_re_dir += self->w_dir[0][ts][qs].im *
                            self->r_out_im_in_m2[ts][indx][row][0];
              sum_im_dir -= self->w_dir[0][ts][qs].re *
                            self->r_out_im_in_m2[ts][indx][row][0];
            }
            self->hyb_dir_out[row][ts][qs].re = sum_re_dir;
            self->hyb_dir_out[row][ts][qs].im = sum_im_dir;
          }
        }
        for (qs = 2; qs < self->hyb_band_count_max; qs++) {
          WORD32 indx = self->hyb_band_to_processing_band_table[qs];
          for (row = 0; row < self->out_ch_count; row++) {
            FLOAT32 sum_re_dir = self->hyb_dir_out[row][ts][qs].re;
            FLOAT32 sum_im_dir = self->hyb_dir_out[row][ts][qs].im;
            if (num_cols > 0) {
              sum_re_dir -= self->w_dir[0][ts][qs].im *
                            self->r_out_im_in_m2[ts][indx][row][0];
              sum_im_dir += self->w_dir[0][ts][qs].re *
                            self->r_out_im_in_m2[ts][indx][row][0];
            }
            self->hyb_dir_out[row][ts][qs].re = sum_re_dir;
            self->hyb_dir_out[row][ts][qs].im = sum_im_dir;
          }
        }
      }
    }
  }
  return;
}

VOID ixheaacd_mps_upmix_interp(
    FLOAT32 m_matrix[MAX_PARAMETER_SETS_MPS][MAX_PARAMETER_BANDS][MAX_M_OUTPUT][MAX_M_INPUT],
    FLOAT32 r_matrix_float[MAX_TIME_SLOTS][MAX_PARAMETER_BANDS][MAX_M_OUTPUT][MAX_M_INPUT],
    FLOAT32 m_matrix_prev[MAX_PARAMETER_BANDS][MAX_M_OUTPUT][MAX_M_INPUT], WORD32 num_rows,
    WORD32 num_cols, ia_mps_dec_state_struct *self, WORD32 bs_high_rate_mode) {
  WORD32 ts, ps, pb, row, col, i;
  FLOAT32 ks, ms, ls;
  FLOAT32 fl_step, fl_base;

  for (pb = 0; pb < self->bs_param_bands; pb++) {
    for (row = 0; row < num_rows; row++) {
      for (col = 0; col < num_cols; col++) {
        ts = 0;
        ps = 0;
        ks = self->inv_param_slot_diff[ps];
        ms = m_matrix[ps][pb][row][col];
        ls = m_matrix_prev[pb][row][col];
        fl_step = ks * (ms - ls);
        fl_base = ls + fl_step;

        for (i = 1; i <= (WORD32)self->param_slot_diff[0]; i++) {
          r_matrix_float[ts][pb][row][col] = fl_base;
          fl_base += fl_step;
          ts++;
        }
        if (bs_high_rate_mode) {
          for (ps = 1; ps < self->num_parameter_sets; ps++) {
            ks = self->inv_param_slot_diff[ps];
            ms = m_matrix[ps][pb][row][col];
            ls = m_matrix[ps - 1][pb][row][col];
            fl_step = ks * (ms - ls);
            fl_base = ls + fl_step;

            for (i = 1; i <= (WORD32)self->param_slot_diff[ps]; i++) {
              r_matrix_float[ts][pb][row][col] = fl_base;
              fl_base += fl_step;
              ts++;
            }
          }
        }
      }
    }
  }
  return;
}

VOID ixheaacd_mps_upmix_interp_type1(
    FLOAT32 m_matrix[MAX_PARAMETER_SETS_MPS][MAX_PARAMETER_BANDS][MAX_M_OUTPUT][MAX_M_INPUT],
    FLOAT32 r_matrix_float[MAX_TIME_SLOTS][MAX_PARAMETER_BANDS][MAX_M_OUTPUT][MAX_M_INPUT],
    FLOAT32 m_matrix_prev[MAX_PARAMETER_BANDS][MAX_M_OUTPUT][MAX_M_INPUT], WORD32 num_rows,
    WORD32 num_cols, ia_mps_dec_state_struct *self, WORD32 bs_high_rate_mode) {
  WORD32 ts, ps, pb, row, col, i;
  FLOAT32 ks, ms, ls;
  FLOAT32 fl_step, fl_base;

  for (pb = 0; pb < self->bs_param_bands; pb++) {
    for (row = 0; row < num_rows; row++) {
      for (col = 0; col < num_cols; col++) {
        ts = 0;
        ps = 0;
        ks = self->inv_param_slot_diff[ps];
        ms = m_matrix[ps][pb][row][col];
        ls = m_matrix_prev[pb][row][col];
        fl_step = ks * (ms - ls);
        fl_base = ls + fl_step;

        for (i = 1; i <= (WORD32)self->param_slot_diff[0]; i++) {
          r_matrix_float[ts][pb][row][col] = fl_base;
          fl_base += fl_step;
          ts++;
        }
        if (bs_high_rate_mode) {
          for (ps = 1; ps < self->num_parameter_sets; ps++) {
            ks = self->inv_param_slot_diff[ps];
            ms = m_matrix[ps][pb][row][col];
            ls = m_matrix[ps - 1][pb][row][col];
            fl_step = ks * (ms - ls);
            fl_base = ls + fl_step;

            for (i = 1; i <= self->param_slot_diff[ps]; i++) {
              r_matrix_float[ts][pb][row][col] = fl_base;
              fl_base += fl_step;
              ts++;
            }
          }
        }
      }
    }
  }
  return;
}

VOID ixheaacd_mps_upmix_interp_type2(
    FLOAT32 m_matrix[MAX_PARAMETER_SETS_MPS][MAX_PARAMETER_BANDS][MAX_M_OUTPUT][MAX_M_INPUT],
    FLOAT32 r_matrix_float[MAX_TIME_SLOTS][MAX_PARAMETER_BANDS][MAX_M_OUTPUT][MAX_M_INPUT],
    FLOAT32 m_matrix_prev[MAX_PARAMETER_BANDS][MAX_M_OUTPUT][MAX_M_INPUT], WORD32 num_rows,
    ia_mps_dec_state_struct *self, WORD32 col) {
  WORD32 ts, ps, pb, row, i;
  FLOAT32 ks, ms, ls;
  FLOAT32 fl_step, fl_base;

  for (pb = 0; pb < self->bs_param_bands; pb++) {
    for (row = 0; row < num_rows; row++) {
      ts = 0;
      ps = 0;
      ks = self->inv_param_slot_diff[ps];
      ms = m_matrix[ps][pb][row][col];
      ls = m_matrix_prev[pb][row][col];
      fl_step = ks * (ms - ls);
      fl_base = ls + fl_step;

      for (i = 1; i <= (WORD32)self->param_slot_diff[0]; i++) {
        r_matrix_float[ts][pb][row][col] = fl_base;
        fl_base += fl_step;
        ts++;
      }
      for (ps = 1; ps < self->num_parameter_sets; ps++) {
        ks = self->inv_param_slot_diff[ps];
        ms = m_matrix[ps][pb][row][col];
        ls = m_matrix[ps - 1][pb][row][col];
        fl_step = ks * (ms - ls);
        fl_base = ls + fl_step;

        for (i = 1; i <= (WORD32)self->param_slot_diff[ps]; i++) {
          r_matrix_float[ts][pb][row][col] = fl_base;
          fl_base += fl_step;
          ts++;
        }
      }
    }
  }
  return;
}

static FLOAT32 ixheaacd_mps_angle_interpolation(FLOAT32 angle1, FLOAT32 angle2,
                                                FLOAT32 alpha, FLOAT32 *step) {
  while (angle2 - angle1 > (FLOAT32)P_PI) {
    angle1 = angle1 + 2.0f * (FLOAT32)P_PI;
  }
  while (angle1 - angle2 > (FLOAT32)P_PI) {
    angle2 = angle2 + 2.0f * (FLOAT32)P_PI;
  }
  *step = angle2 - angle1;
  return (1 - alpha) * angle1 + alpha * angle2;
}

VOID ixheaacd_mps_phase_interpolation(
    FLOAT32 pl[MAX_PARAMETER_SETS_MPS][MAX_PARAMETER_BANDS],
    FLOAT32 pr[MAX_PARAMETER_SETS_MPS][MAX_PARAMETER_BANDS],
    FLOAT32 pl_prev[MAX_PARAMETER_BANDS], FLOAT32 pr_prev[MAX_PARAMETER_BANDS],
    FLOAT32 r_re[MAX_TIME_SLOTS][MAX_PARAMETER_BANDS][2],
    FLOAT32 r_im[MAX_TIME_SLOTS][MAX_PARAMETER_BANDS][2],
    ia_mps_dec_state_struct *self) {
  WORD32 i, ts, ps, pb;
  FLOAT32 step_l, step_r, alpha, tl, tr;
  for (pb = 0; pb < self->bs_param_bands; pb++) {
    ps = 0;
    ts = 0;
    alpha = (FLOAT32)self->inv_param_slot_diff[ps];
    tl = ixheaacd_mps_angle_interpolation(pl_prev[pb], pl[ps][pb], alpha,
                                          &step_l);
    tr = ixheaacd_mps_angle_interpolation(pr_prev[pb], pr[ps][pb], alpha,
                                          &step_r);
    step_l *= alpha;
    step_r *= alpha;

    for (i = 1; i <= self->param_slot_diff[ps]; i++) {
      r_re[ts][pb][0] = (FLOAT32)cos(tl);
      r_im[ts][pb][0] = (FLOAT32)sin(tl);
      tl += step_l;

      r_re[ts][pb][1] = (FLOAT32)cos(tr);
      r_im[ts][pb][1] = (FLOAT32)sin(tr);
      tr += step_r;
      ts++;
    }

    for (ps = 1; ps < self->num_parameter_sets; ps++) {
      FLOAT32 alpha = self->inv_param_slot_diff[ps];
      tl = ixheaacd_mps_angle_interpolation(pl[ps - 1][pb], pl[ps][pb], alpha,
                                            &step_l);
      tr = ixheaacd_mps_angle_interpolation(pr[ps - 1][pb], pr[ps][pb], alpha,
                                            &step_r);
      step_l *= alpha;
      step_r *= alpha;
      for (i = 1; i <= self->param_slot_diff[ps]; i++) {
        if (ts < 72 && pb < 28) {
          r_re[ts][pb][0] = (FLOAT32)cos(tl);
          r_im[ts][pb][0] = (FLOAT32)sin(tl);
          tl += step_l;

          r_re[ts][pb][1] = (FLOAT32)cos(tr);
          r_im[ts][pb][1] = (FLOAT32)sin(tr);
          tr += step_r;
        }
        ts++;

        if (ts > 71) {
          ts = 0;
          break;
        }
        if (pb > 27) {
          pb = 0;
          break;
        }
      }
    }
  }
}

VOID ixheaacd_mps_init_pre_and_post_matrix(ia_mps_dec_state_struct *self) {
  memset(self->m1_param_re_prev, 0,
         MAX_PARAMETER_BANDS * MAX_M_OUTPUT * MAX_M_INPUT * sizeof(WORD32));
  memset(self->m1_param_im_prev, 0,
         MAX_PARAMETER_BANDS * MAX_M_OUTPUT * MAX_M_INPUT * sizeof(WORD32));
  memset(self->m1_param_re_prev, 0,
         MAX_PARAMETER_BANDS * MAX_M_OUTPUT * MAX_M_INPUT * sizeof(WORD32));
  memset(self->m2_decor_re_prev, 0,
         MAX_PARAMETER_BANDS * MAX_M_OUTPUT * MAX_M_INPUT * sizeof(WORD32));
  memset(self->m2_resid_re_prev, 0,
         MAX_PARAMETER_BANDS * MAX_M_OUTPUT * MAX_M_INPUT * sizeof(WORD32));
  memset(self->m2_resid_im_prev, 0,
         MAX_PARAMETER_BANDS * MAX_M_OUTPUT * MAX_M_INPUT * sizeof(WORD32));
}
