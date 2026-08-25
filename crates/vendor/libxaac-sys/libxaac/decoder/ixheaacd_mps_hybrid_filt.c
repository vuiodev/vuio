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
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaacd_mps_macro_def.h"
#include "ixheaacd_mps_basic_op.h"

extern const FLOAT32 ixheaacd_ia_mps_hyb_filter_coeff_8[QMF_HYBRID_FILT_ORDER];
extern const FLOAT32 ixheaacd_mps_hyb_filter_coeff_2[QMF_HYBRID_FILT_ORDER];

extern const FLOAT32 ixheaacd_sine[8][8];
extern const FLOAT32 ixheaacd_cosine[8][8];

static VOID ixheaacd_mps_hyb_filt_type1(ia_cmplx_flt_struct *input,
                                        ia_cmplx_flt_struct output[8][MAX_TIME_SLOTS],
                                        WORD32 num_samples, const FLOAT32 *filt_coeff) {
  WORD32 i, n, q;

  FLOAT32 in_re, in_im;
  FLOAT32 coeff;
  FLOAT32 acc_re_l, acc_re_h, acc_im_l, acc_im_h;

  for (i = 0; i < num_samples; i++) {
    FLOAT32 x0_re[13], x0_im[13], x0_1_re[8], x0_1_im[8];
    FLOAT32 acc_re_val[8], acc_im_val[8];
    for (n = 0; n < QMF_HYBRID_FILT_ORDER; n++) {
      in_re = input[n + i].re;
      in_im = input[n + i].im;

      coeff = filt_coeff[QMF_HYBRID_FILT_ORDER - 1 - n];

      x0_re[n] = coeff * in_re;
      x0_im[n] = coeff * in_im;
    }

    x0_1_re[0] = x0_re[6];
    x0_1_im[0] = x0_im[6];

    x0_1_re[1] = x0_re[7];
    x0_1_im[1] = x0_im[7];

    x0_1_re[2] = x0_re[8] - x0_re[0];
    x0_1_im[2] = x0_im[8] - x0_im[0];

    x0_1_re[3] = x0_re[9] - x0_re[1];
    x0_1_im[3] = x0_im[9] - x0_im[1];

    x0_1_re[4] = x0_re[10] - x0_re[2];
    x0_1_im[4] = x0_im[10] - x0_im[2];

    x0_1_re[5] = x0_re[11] - x0_re[3];
    x0_1_im[5] = x0_im[11] - x0_im[3];

    x0_1_re[6] = x0_re[12] - x0_re[4];
    x0_1_im[6] = x0_im[12] - x0_im[4];

    x0_1_re[7] = -(x0_re[5]);
    x0_1_im[7] = -(x0_im[5]);

    acc_re_val[0] = x0_1_re[0];
    acc_re_val[1] = x0_1_re[1] - x0_1_re[7];
    acc_re_val[2] = x0_1_re[2] - x0_1_re[6];
    acc_re_val[3] = x0_1_re[3] - x0_1_re[5];
    acc_re_val[4] = x0_1_im[1] + x0_1_im[7];
    acc_re_val[5] = x0_1_im[2] + x0_1_im[6];
    acc_re_val[6] = x0_1_im[3] + x0_1_im[5];
    acc_re_val[7] = x0_1_im[4];

    acc_im_val[0] = x0_1_im[0];
    acc_im_val[1] = x0_1_im[1] - x0_1_im[7];
    acc_im_val[2] = x0_1_im[2] - x0_1_im[6];
    acc_im_val[3] = x0_1_im[3] - x0_1_im[5];
    acc_im_val[4] = x0_1_re[1] + x0_1_re[7];
    acc_im_val[5] = x0_1_re[2] + x0_1_re[6];
    acc_im_val[6] = x0_1_re[3] + x0_1_re[5];
    acc_im_val[7] = x0_1_re[4];

    for (q = 0; q < 4; q++) {
      acc_re_l = 0;
      acc_im_l = 0;
      acc_re_h = 0;
      acc_im_h = 0;

      acc_re_l += acc_re_val[0];
      acc_re_l += acc_re_val[1] * ixheaacd_cosine[q][1];
      acc_re_l += acc_re_val[2] * ixheaacd_cosine[q][2];
      acc_re_l += acc_re_val[3] * ixheaacd_cosine[q][3];

      acc_re_h = acc_re_l;

      acc_re_l -= acc_re_val[4] * ixheaacd_sine[q][1];
      acc_re_l -= acc_re_val[5] * ixheaacd_sine[q][2];
      acc_re_l -= acc_re_val[6] * ixheaacd_sine[q][3];
      acc_re_l -= acc_re_val[7] * ixheaacd_sine[q][4];

      acc_re_h = acc_re_h - (acc_re_l - acc_re_h);

      acc_im_l += acc_im_val[0];
      acc_im_l += acc_im_val[1] * ixheaacd_cosine[q][1];
      acc_im_l += acc_im_val[2] * ixheaacd_cosine[q][2];
      acc_im_l += acc_im_val[3] * ixheaacd_cosine[q][3];

      acc_im_h = acc_im_l;

      acc_im_l += acc_im_val[4] * ixheaacd_sine[q][1];
      acc_im_l += acc_im_val[5] * ixheaacd_sine[q][2];
      acc_im_l += acc_im_val[6] * ixheaacd_sine[q][3];
      acc_im_l += acc_im_val[7] * ixheaacd_sine[q][4];

      acc_im_h = acc_im_h - (acc_im_l - acc_im_h);

      output[q][i].re = acc_re_l;
      output[q][i].im = acc_im_l;

      output[7 - q][i].re = acc_re_h;
      output[7 - q][i].im = acc_im_h;
    }
  }
}

static VOID ixheaacd_mps_hyb_filt_type2(ia_cmplx_flt_struct *input,
                                        ia_cmplx_flt_struct output[2][MAX_TIME_SLOTS],
                                        WORD32 num_samples, const FLOAT32 *filt_coeff) {
  WORD32 i, n;

  FLOAT32 in_re, in_im;
  FLOAT32 coeff;
  FLOAT32 acc_re[2], acc_im[2];

  for (i = 0; i < num_samples; i++) {
    FLOAT32 x_0_re[13], x_0_im[13];

    for (n = 1; n < 6; n = n + 2) {
      in_re = input[n + i].re;
      in_im = input[n + i].im;

      in_re += input[12 - n + i].re;
      in_im += input[12 - n + i].im;

      coeff = filt_coeff[QMF_HYBRID_FILT_ORDER - 1 - n];

      x_0_re[n] = coeff * in_re;
      x_0_im[n] = coeff * in_im;
    }

    n = 6;
    in_re = input[n + i].re;
    in_im = input[n + i].im;

    coeff = filt_coeff[QMF_HYBRID_FILT_ORDER - 1 - n];

    x_0_re[n] = coeff * in_re;
    x_0_im[n] = coeff * in_im;

    x_0_re[1] = x_0_re[1] + x_0_re[3] + x_0_re[5];
    x_0_im[1] = x_0_im[1] + x_0_im[3] + x_0_im[5];

    acc_re[0] = x_0_re[6] + x_0_re[1];
    acc_im[0] = x_0_im[6] + x_0_im[1];

    acc_re[1] = x_0_re[6] - x_0_re[1];
    acc_im[1] = x_0_im[6] - x_0_im[1];

    output[0][i].re = acc_re[0];
    output[0][i].im = acc_im[0];

    output[1][i].re = acc_re[1];
    output[1][i].im = acc_im[1];
  }
}

VOID ixheaacd_mps_qmf_hybrid_analysis_init(ia_mps_hybrid_filt_struct *handle) {
  memset(handle->lf_buffer, 0,
         QMF_BANDS_TO_HYBRID * BUFFER_LEN_LF_MPS * sizeof(ia_cmplx_w32_struct));
  memset(handle->hf_buffer, 0,
         MAX_NUM_QMF_BANDS_MPS * BUFFER_LEN_HF_MPS * sizeof(ia_cmplx_flt_struct));
}

VOID ixheaacd_mps_qmf_hybrid_analysis_no_pre_mix(
    ia_mps_hybrid_filt_struct *handle,
    ia_cmplx_flt_struct in_qmf[MAX_NUM_QMF_BANDS_MPS][MAX_TIME_SLOTS], WORD32 num_bands,
    WORD32 num_samples, ia_cmplx_flt_struct v[MAX_TIME_SLOTS][MAX_HYBRID_BANDS_MPS]) {
  WORD32 lf_samples_shift;
  WORD32 hf_samples_shift;
  WORD32 lf_qmf_bands;
  WORD32 k, n;

  ia_cmplx_flt_struct scratch[MAX_HYBRID_ONLY_BANDS_PER_QMF][MAX_TIME_SLOTS];

  lf_samples_shift = BUFFER_LEN_LF_MPS - num_samples;
  hf_samples_shift = BUFFER_LEN_HF_MPS - num_samples;

  lf_qmf_bands = QMF_BANDS_TO_HYBRID;

  for (k = 0; k < lf_qmf_bands; k++) {
    memmove(&handle->lf_buffer[k][0].re, &handle->lf_buffer[k][num_samples].re,
            2 * lf_samples_shift * sizeof(FLOAT32));
  }

  for (k = 0; k < lf_qmf_bands; k++) {
    memcpy(&handle->lf_buffer[k][lf_samples_shift].re, &in_qmf[k][0].re,
           2 * num_samples * sizeof(FLOAT32));
  }

  for (k = 0; k < MAX_NUM_QMF_BANDS_SAC / 2 - lf_qmf_bands; k++) {
    memmove(&handle->hf_buffer[k][0].re, &handle->hf_buffer[k][num_samples].re,
            2 * hf_samples_shift * sizeof(FLOAT32));
  }

  for (k = 0; k < num_bands - lf_qmf_bands; k++) {
    memcpy(&handle->hf_buffer[k][hf_samples_shift].re, &in_qmf[k + lf_qmf_bands][0].re,
           2 * num_samples * sizeof(FLOAT32));
  }

  ixheaacd_mps_hyb_filt_type1(
      &(handle->lf_buffer[0][lf_samples_shift + 1 - QMF_HYBRID_FILT_ORDER]), scratch, num_samples,
      ixheaacd_ia_mps_hyb_filter_coeff_8);

  for (k = 0; k < 2; k++) {
    for (n = 0; n < num_samples; n++) {
      v[n][k].re = scratch[k + 6][n].re;
      v[n][k + 2].re = scratch[k][n].re;
      v[n][k + 4].re = scratch[k + 2][n].re;
      v[n][k + 4].re += scratch[5 - k][n].re;

      v[n][k].im = scratch[k + 6][n].im;
      v[n][k + 2].im = scratch[k][n].im;
      v[n][k + 4].im = scratch[k + 2][n].im;
      v[n][k + 4].im += scratch[5 - k][n].im;
    }
  }

  ixheaacd_mps_hyb_filt_type2(
      &(handle->lf_buffer[1][lf_samples_shift + 1 - QMF_HYBRID_FILT_ORDER]), scratch, num_samples,
      ixheaacd_mps_hyb_filter_coeff_2);

  for (k = 0; k < 2; k++) {
    for (n = 0; n < num_samples; n++) {
      v[n][k + 6].re = scratch[1 - k][n].re;
      v[n][k + 6].im = scratch[1 - k][n].im;
    }
  }

  ixheaacd_mps_hyb_filt_type2(
      &(handle->lf_buffer[2][lf_samples_shift + 1 - QMF_HYBRID_FILT_ORDER]), scratch, num_samples,
      ixheaacd_mps_hyb_filter_coeff_2);

  for (k = 0; k < 2; k++) {
    for (n = 0; n < num_samples; n++) {
      v[n][k + 8].re = scratch[k][n].re;
      v[n][k + 8].im = scratch[k][n].im;
    }
  }

  for (k = 0; k < num_bands - lf_qmf_bands; k++) {
    for (n = 0; n < num_samples; n++) {
      v[n][k + 10].re = (handle->hf_buffer[k][n + hf_samples_shift].re);
      v[n][k + 10].im = (handle->hf_buffer[k][n + hf_samples_shift].im);
    }
  }
}

VOID ixheaacd_mps_qmf_hybrid_analysis(
    ia_mps_hybrid_filt_struct *handle,
    ia_cmplx_flt_struct in_qmf[MAX_NUM_QMF_BANDS_MPS_NEW][MAX_TIME_SLOTS], WORD32 num_bands,
    WORD32 num_samples, ia_cmplx_flt_struct hyb[MAX_HYBRID_BANDS_MPS][MAX_TIME_SLOTS]) {
  WORD32 lf_samples_shift;
  WORD32 hf_samples_shift;
  WORD32 lf_qmf_bands;
  WORD32 k, n;

  ia_cmplx_flt_struct scratch[MAX_HYBRID_ONLY_BANDS_PER_QMF][MAX_TIME_SLOTS];

  lf_samples_shift = BUFFER_LEN_LF_MPS - num_samples;
  hf_samples_shift = BUFFER_LEN_HF_MPS - num_samples;

  lf_qmf_bands = QMF_BANDS_TO_HYBRID;

  for (k = 0; k < lf_qmf_bands; k++) {
    memmove(&handle->lf_buffer[k][0].re, &handle->lf_buffer[k][num_samples].re,
            2 * lf_samples_shift * sizeof(FLOAT32));
  }

  for (k = 0; k < lf_qmf_bands; k++) {
    memcpy(&handle->lf_buffer[k][lf_samples_shift].re, &in_qmf[k][0].re,
           2 * num_samples * sizeof(FLOAT32));
  }

  for (k = 0; k < MAX_NUM_QMF_BANDS_SAC / 2 - lf_qmf_bands; k++) {
    memmove(&handle->hf_buffer[k][0].re, &handle->hf_buffer[k][num_samples].re,
            2 * hf_samples_shift * sizeof(FLOAT32));
  }

  for (k = 0; k < num_bands - lf_qmf_bands; k++) {
    memcpy(&handle->hf_buffer[k][hf_samples_shift].re, &in_qmf[k + lf_qmf_bands][0].re,
           2 * num_samples * sizeof(FLOAT32));
  }

  ixheaacd_mps_hyb_filt_type1(
      &(handle->lf_buffer[0][lf_samples_shift + 1 - QMF_HYBRID_FILT_ORDER]), scratch, num_samples,
      ixheaacd_ia_mps_hyb_filter_coeff_8);

  for (k = 0; k < 2; k++) {
    for (n = 0; n < num_samples; n++) {
      hyb[k][n].re = scratch[k + 6][n].re;
      hyb[k + 2][n].re = scratch[k][n].re;
      hyb[k + 4][n].re = scratch[k + 2][n].re;
      hyb[k + 4][n].re += scratch[5 - k][n].re;

      hyb[k][n].im = scratch[k + 6][n].im;
      hyb[k + 2][n].im = scratch[k][n].im;
      hyb[k + 4][n].im = scratch[k + 2][n].im;
      hyb[k + 4][n].im += scratch[5 - k][n].im;
    }
  }

  ixheaacd_mps_hyb_filt_type2(
      &(handle->lf_buffer[1][lf_samples_shift + 1 - QMF_HYBRID_FILT_ORDER]), scratch, num_samples,
      ixheaacd_mps_hyb_filter_coeff_2);

  for (k = 0; k < 2; k++) {
    for (n = 0; n < num_samples; n++) {
      hyb[k + 6][n].re = scratch[1 - k][n].re;
      hyb[k + 6][n].im = scratch[1 - k][n].im;
    }
  }

  ixheaacd_mps_hyb_filt_type2(
      &(handle->lf_buffer[2][lf_samples_shift + 1 - QMF_HYBRID_FILT_ORDER]), scratch, num_samples,
      ixheaacd_mps_hyb_filter_coeff_2);

  for (k = 0; k < 2; k++) {
    for (n = 0; n < num_samples; n++) {
      hyb[k + 8][n].re = scratch[k][n].re;
      hyb[k + 8][n].im = scratch[k][n].im;
    }
  }

  for (k = 0; k < num_bands - lf_qmf_bands; k++) {
    memcpy(&hyb[k + 10][0].re, &handle->hf_buffer[k][hf_samples_shift].re,
           2 * num_samples * sizeof(FLOAT32));
  }
}

VOID ixheaacd_mps_qmf_hybrid_synthesis(
    ia_cmplx_flt_struct hyb[MAX_TIME_SLOTS][MAX_HYBRID_BANDS_MPS], WORD32 num_bands,
    WORD32 num_samples, ia_cmplx_flt_struct in_qmf[MAX_TIME_SLOTS][MAX_NUM_QMF_BANDS_MPS]) {
  WORD32 k, n;

  for (n = 0; n < num_samples; n++) {
    in_qmf[n][0].re = hyb[n][0].re;
    in_qmf[n][0].im = hyb[n][0].im;

    for (k = 1; k < 6; k++) {
      in_qmf[n][0].re += hyb[n][k].re;
      in_qmf[n][0].im += hyb[n][k].im;
    }

    in_qmf[n][1].re = hyb[n][6].re + hyb[n][7].re;
    in_qmf[n][1].im = hyb[n][6].im + hyb[n][7].im;

    in_qmf[n][2].re = hyb[n][8].re + hyb[n][9].re;
    in_qmf[n][2].im = hyb[n][8].im + hyb[n][9].im;

    memcpy(&in_qmf[n][3].re, &hyb[n][10].re, 2 * (num_bands - 3) * sizeof(FLOAT32));
  }
}

VOID ixheaacd_mps_fft(complex *out, LOOPINDEX idx, WORD32 nob,
                      const ia_mps_dec_hybrid_tables_struct *hyb_tab) {
  LOOPINDEX block_per_stage, stage_num, inner;
  const WORD32 *cosine_array = hyb_tab->cosine_array;
  const WORD32 *sine_array = hyb_tab->sine_array;
  WORD32 index_1, index_2, index, tab_modifier;
  WORD32 len, increment, i;

  WORD32 cos_val;
  WORD32 sin_val;

  WORD16 index1;
  WORD32 re_temp;
  WORD32 im_temp;
  WORD32 *out1_w32, *out2_w32;

  len = idx;
  i = 1;
  increment = 0;

  len = len >> 1;
  index_1 = 0;
  increment += 1;

  index = 11 - increment;
  tab_modifier = ixheaac_shl32(1, index);

  out1_w32 = (WORD32 *)&out[index_1];
  out2_w32 = (WORD32 *)&out[index_1 + 1];

  for (block_per_stage = 0; block_per_stage < len; block_per_stage++) {
    re_temp = out2_w32[0];
    im_temp = out2_w32[1];

    out2_w32[0] = ixheaac_sub32_sat(out1_w32[0], re_temp);
    out2_w32[1] = ixheaac_sub32_sat(out1_w32[1], im_temp);

    out1_w32[0] = ixheaac_add32_sat(re_temp, out1_w32[0]);
    out1_w32[1] = ixheaac_add32_sat(im_temp, out1_w32[1]);

    out1_w32 += 4;
    out2_w32 += 4;
  }

  i <<= 1;

  for (stage_num = 1; stage_num < nob; stage_num++) {
    len = len >> 1;
    index_1 = 0;
    increment += 1;

    index = 11 - increment;
    tab_modifier = ixheaac_shl32(1, index);

    for (block_per_stage = 0; block_per_stage < len; block_per_stage++) {
      index_2 = index_1 + i;

      out1_w32 = (WORD32 *)&out[index_1];
      out2_w32 = (WORD32 *)&out[index_2];

      re_temp = out1_w32[0];
      im_temp = out1_w32[1];

      out1_w32[0] = ((WORD64)re_temp + (WORD64)out2_w32[0]) >> 1;
      out1_w32[1] = ((WORD64)im_temp + (WORD64)out2_w32[1]) >> 1;

      out2_w32[0] = ((WORD64)re_temp - (WORD64)out2_w32[0]) >> 1;
      out2_w32[1] = ((WORD64)im_temp - (WORD64)out2_w32[1]) >> 1;

      index1 = tab_modifier;

      out1_w32 += 2;
      out2_w32 += 2;

      for (inner = 0; inner < ((i - 1) << 1); inner += 2) {
        cos_val = cosine_array[index1];
        sin_val = sine_array[index1];

        re_temp = ixheaacd_mps_mult32x16_shr_16(out2_w32[inner], cos_val) +
                  ixheaacd_mps_mult32x16_shr_16(out2_w32[inner + 1], sin_val);
        im_temp = ixheaacd_mps_mult32x16_shr_16(out2_w32[inner + 1], cos_val) -
                  ixheaacd_mps_mult32x16_shr_16(out2_w32[inner], sin_val);

        out1_w32[inner] >>= 1;
        out1_w32[inner + 1] >>= 1;

        out2_w32[inner] = ixheaac_sub32_sat(out1_w32[inner], re_temp);
        out2_w32[inner + 1] = ixheaac_sub32_sat(out1_w32[inner + 1], im_temp);

        out1_w32[inner] = ixheaac_add32_sat(out1_w32[inner], re_temp);
        out1_w32[inner + 1] = ixheaac_add32_sat(out1_w32[inner + 1], im_temp);

        index1 += tab_modifier;
      }

      index_1 += ixheaac_shl32(1, increment);
    }
    i <<= 1;
  }
}

VOID ixheaacd_8ch_filtering(const WORD32 *p_qmf_real, const WORD32 *p_qmf_imag,
                            WORD32 *m_hybrid_real, WORD32 *m_hybrid_imag,
                            const ia_mps_dec_hybrid_tables_struct *hyb_tab) {
  WORD32 n;
  WORD32 real, imag;
  const WORD16 tcos = COS_PI_BY_8;
  const WORD16 tsin = SIN_PI_BY_8;
  WORD32 cum[16];
  WORD32 *p_complex;
  const WORD16 *p8_13 = hyb_tab->p8_13;

  real = ixheaac_shl32((ixheaac_mult32x16in32(p_qmf_real[4], p8_13[4]) +
                         ixheaac_mult32x16in32(p_qmf_real[12], p8_13[12])),
                        1);
  imag = ixheaac_shl32((ixheaac_mult32x16in32(p_qmf_imag[4], p8_13[4]) +
                         ixheaac_mult32x16in32(p_qmf_imag[12], p8_13[12])),
                        1);

  cum[5] = ixheaac_sub32_sat(imag, real);
  cum[4] = -ixheaac_add32_sat(imag, real);

  real = ixheaac_shl32((ixheaac_mult32x16in32(p_qmf_real[3], p8_13[3]) +
                         ixheaac_mult32x16in32(p_qmf_real[11], p8_13[11])),
                        1);
  imag = ixheaac_shl32((ixheaac_mult32x16in32(p_qmf_imag[3], p8_13[3]) +
                         ixheaac_mult32x16in32(p_qmf_imag[11], p8_13[11])),
                        1);

  cum[13] = ixheaac_shl32(
      (ixheaac_mult32x16in32(imag, tcos) - ixheaac_mult32x16in32(real, tsin)), 1);
  cum[12] = ixheaac_shl32(
      -((ixheaac_mult32x16in32(imag, tsin) + ixheaac_mult32x16in32(real, tcos))), 1);

  cum[2] = ixheaac_shl32((ixheaac_mult32x16in32(p_qmf_real[2], p8_13[10]) -
                          ixheaac_mult32x16in32(p_qmf_real[10], p8_13[10])),
                         1);
  cum[3] = ixheaac_shl32((ixheaac_mult32x16in32(p_qmf_imag[2], p8_13[2]) -
                          ixheaac_mult32x16in32(p_qmf_imag[10], p8_13[2])),
                         1);
  real = ixheaac_shl32((ixheaac_mult32x16in32(p_qmf_real[1], p8_13[1]) +
                         ixheaac_mult32x16in32(p_qmf_real[9], p8_13[9])),
                        1);
  imag = ixheaac_shl32((ixheaac_mult32x16in32(p_qmf_imag[1], p8_13[1]) +
                         ixheaac_mult32x16in32(p_qmf_imag[9], p8_13[9])),
                        1);

  cum[11] = ixheaac_shl32(
      (ixheaac_mult32x16in32(imag, tcos) + ixheaac_mult32x16in32(real, tsin)), 1);
  cum[10] = ixheaac_shl32(
      (ixheaac_mult32x16in32(imag, tsin) - ixheaac_mult32x16in32(real, tcos)), 1);

  real = ixheaac_shl32((ixheaac_mult32x16in32(p_qmf_real[0], p8_13[0]) +
                         ixheaac_mult32x16in32(p_qmf_real[8], p8_13[8])),
                        1);
  imag = ixheaac_shl32((ixheaac_mult32x16in32(p_qmf_imag[0], p8_13[0]) +
                         ixheaac_mult32x16in32(p_qmf_imag[8], p8_13[8])),
                        1);

  cum[7] = ixheaac_add32_sat(imag, real);
  cum[6] = ixheaac_sub32_sat(imag, real);

  cum[15] = ixheaac_shl32((ixheaac_mult32x16in32(p_qmf_imag[7], p8_13[14]) +
                            ixheaac_mult32x16in32(p_qmf_real[7], p8_13[13])),
                           1);
  cum[14] = ixheaac_shl32((ixheaac_mult32x16in32(p_qmf_imag[7], p8_13[13]) -
                            ixheaac_mult32x16in32(p_qmf_real[7], p8_13[14])),
                           1);

  cum[1] = ixheaac_shl32(
      ixheaac_mult32x16in32(p_qmf_real[HYBRID_FILTER_DELAY], p8_13[HYBRID_FILTER_DELAY]), 1);
  cum[0] = ixheaac_shl32(
      ixheaac_mult32x16in32(p_qmf_imag[HYBRID_FILTER_DELAY], p8_13[HYBRID_FILTER_DELAY]), 1);

  cum[9] = ixheaac_shl32((ixheaac_mult32x16in32(p_qmf_real[5], p8_13[13]) -
                           ixheaac_mult32x16in32(p_qmf_imag[5], p8_13[14])),
                          1);
  cum[8] = ixheaac_shl32((ixheaac_mult32x16in32(p_qmf_real[5], p8_13[14]) +
                           ixheaac_mult32x16in32(p_qmf_imag[5], p8_13[13])),
                          1);

  ixheaacd_mps_fft((complex *)cum, 8, 3, hyb_tab);

  p_complex = cum;

  for (n = 0; n < 8; n++) {
    m_hybrid_imag[n] = *p_complex++;
    m_hybrid_real[n] = *p_complex++;
  }
}

VOID ixheaacd_2ch_filtering(WORD32 *p_qmf, WORD32 *m_hybrid,
                            const ia_mps_dec_hybrid_tables_struct *hyb_tab_ptr) {
  WORD32 cum0, cum1;
  WORD64 temp;
  const WORD16 *p2_6 = hyb_tab_ptr->p2_6;

  cum0 = (WORD32)p_qmf[HYBRID_FILTER_DELAY] >> 1;

  temp = (WORD64)((WORD64)p2_6[0] * ((WORD64)p_qmf[1] + (WORD64)p_qmf[11]) +
                  (WORD64)p2_6[1] * ((WORD64)p_qmf[3] + (WORD64)p_qmf[9]));
  temp += (WORD64)p2_6[2] * ((WORD64)p_qmf[5] + (WORD64)p_qmf[7]);
  cum1 = (WORD32)(temp >> 16);

  m_hybrid[0] = ixheaac_add32_sat(cum0, cum1);
  m_hybrid[1] = ixheaac_sub32_sat(cum0, cum1);
}

WORD32 ixheaacd_get_qmf_sb(
    WORD32 hybrid_subband,
    const ia_mps_dec_mdct2qmf_table_struct *ixheaacd_mps_dec_mdct2qmf_table) {
  return ixheaacd_mps_dec_mdct2qmf_table->hybrid_2_qmf[hybrid_subband];
}

VOID ixheaacd_init_ana_hyb_filt_bank(ia_mps_dec_thyb_filter_state_struct *hyb_state) {
  WORD32 k, n;

  for (k = 0; k < QMF_BANDS_TO_HYBRID; k++) {
    for (n = 0; n < PROTO_LEN - 1 + MAX_TIME_SLOTS; n++) {
      hyb_state->buffer_lf_real[k][n] = 0;
      hyb_state->buffer_lf_imag[k][n] = 0;
      hyb_state->qmf_lf_real[k][n] = 0;
      hyb_state->qmf_lf_imag[k][n] = 0;
    }
  }

  for (k = 0; k < MAX_NUM_QMF_BANDS; k++) {
    for (n = 0; n < ((PROTO_LEN - 1) >> 1) + MAX_TIME_SLOTS; n++) {
      hyb_state->buffer_hf_real[k][n] = 0;
      hyb_state->buffer_hf_imag[k][n] = 0;
    }
  }
}

VOID ixheaacd_apply_ana_hyb_filt_bank_create_x(
    ia_mps_dec_thyb_filter_state_struct *hyb_state, WORD32 *m_qmf_real, WORD32 *m_qmf_imag,
    WORD32 nr_bands, WORD32 nr_samples, WORD32 *m_hybrid_real, WORD32 *m_hybrid_imag,
    const ia_mps_dec_hybrid_tables_struct *hyb_tab_ptr) {
  WORD32 nr_samples_shift_lf;
  WORD32 nr_qmf_bands_lf;
  WORD32 k, n;
  WORD32 time_slot;

  WORD32 proto_len = (PROTO_LEN - 1) >> 1;
  WORD32 val = nr_samples - proto_len;
  WORD32 val_xhb = val * MAX_HYBRID_BANDS;
  WORD32 loop_cnt, loop_cnt_x4;
  WORD32 *p_qmf_real, *p_qmf_re, *p_qmf_imag, *p_qmf_im;

  WORD32 m_temp_output_real[MAX_HYBRID_ONLY_BANDS_PER_QMF];
  WORD32 m_temp_output_imag[MAX_HYBRID_ONLY_BANDS_PER_QMF];

  WORD32 *p_hybrid_real = m_hybrid_real + 10;
  WORD32 *p_hybrid_imag = m_hybrid_imag + 10;

  WORD32 *p_hybrid_re, *p_hybrid_im;

  nr_samples_shift_lf = BUFFER_LEN_LF - nr_samples;

  nr_qmf_bands_lf = QMF_BANDS_TO_HYBRID;
  loop_cnt = nr_bands - nr_qmf_bands_lf;
  loop_cnt_x4 = (loop_cnt << 2);

  for (k = 0; k < nr_qmf_bands_lf; k++) {
    for (n = 0; n < nr_samples_shift_lf; n++) {
      hyb_state->buffer_lf_real[k][n] = hyb_state->buffer_lf_real[k][n + nr_samples];
      hyb_state->buffer_lf_imag[k][n] = hyb_state->buffer_lf_imag[k][n + nr_samples];

      hyb_state->qmf_lf_real[k][n] = hyb_state->qmf_lf_real[k][n + nr_samples];
      hyb_state->qmf_lf_imag[k][n] = hyb_state->qmf_lf_imag[k][n + nr_samples];
    }
  }

  p_qmf_real = m_qmf_real;
  p_qmf_imag = m_qmf_imag;
  for (k = 0; k < nr_qmf_bands_lf; k++) {
    p_qmf_re = p_qmf_real;
    p_qmf_im = p_qmf_imag;

    for (n = 0; n < nr_samples; n++) {
      hyb_state->buffer_lf_real[k][n + nr_samples_shift_lf] = *p_qmf_re;
      hyb_state->buffer_lf_imag[k][n + nr_samples_shift_lf] = *p_qmf_im;

      hyb_state->qmf_lf_imag[k][n + nr_samples_shift_lf] = *p_qmf_im;
      hyb_state->qmf_lf_real[k][n + nr_samples_shift_lf] = *p_qmf_re;

      p_qmf_re += MAX_HYBRID_BANDS;
      p_qmf_im += MAX_HYBRID_BANDS;
    }

    p_qmf_real++;
    p_qmf_imag++;
  }

  p_qmf_real = m_qmf_real + nr_qmf_bands_lf + val_xhb;
  p_qmf_imag = m_qmf_imag + nr_qmf_bands_lf + val_xhb;

  for (n = 0; n < proto_len; n++) {
    p_qmf_re = p_qmf_real;
    p_qmf_im = p_qmf_imag;

    p_hybrid_re = p_hybrid_real;
    p_hybrid_im = p_hybrid_imag;

    for (k = 0; k < loop_cnt; k++) {
      *p_hybrid_re++ = hyb_state->buffer_hf_real[k][n];
      *p_hybrid_im++ = hyb_state->buffer_hf_imag[k][n];

      hyb_state->buffer_hf_real[k][n] = *p_qmf_re++;
      hyb_state->buffer_hf_imag[k][n] = *p_qmf_im++;
    }
    p_qmf_real += MAX_HYBRID_BANDS;
    p_qmf_imag += MAX_HYBRID_BANDS;

    p_hybrid_real += MAX_HYBRID_BANDS;
    p_hybrid_imag += MAX_HYBRID_BANDS;
  }

  p_qmf_real = m_qmf_real;
  p_qmf_imag = m_qmf_imag;

  p_hybrid_real = m_hybrid_real + 10;
  p_hybrid_imag = m_hybrid_imag + 10;

  k = proto_len * MAX_HYBRID_BANDS;

  p_hybrid_re = p_hybrid_real + k;
  p_hybrid_im = p_hybrid_imag + k;

  p_qmf_re = p_qmf_real + nr_qmf_bands_lf;
  p_qmf_im = p_qmf_imag + nr_qmf_bands_lf;

  for (n = 0; n < val; n++) {
    memcpy(p_hybrid_re, p_qmf_re, loop_cnt_x4);
    memcpy(p_hybrid_im, p_qmf_im, loop_cnt_x4);

    p_qmf_re += MAX_HYBRID_BANDS;
    p_qmf_im += MAX_HYBRID_BANDS;

    p_hybrid_re += MAX_HYBRID_BANDS;
    p_hybrid_im += MAX_HYBRID_BANDS;
  }

  p_hybrid_real = m_hybrid_real;
  p_hybrid_imag = m_hybrid_imag;

  for (time_slot = 0; time_slot < nr_samples; time_slot++) {
    p_hybrid_re = p_hybrid_real;
    p_hybrid_im = p_hybrid_imag;

    ixheaacd_8ch_filtering(
        &(hyb_state->buffer_lf_real[0][time_slot + nr_samples_shift_lf + 1 - PROTO_LEN]),
        &(hyb_state->buffer_lf_imag[0][time_slot + nr_samples_shift_lf + 1 - PROTO_LEN]),
        m_temp_output_real, m_temp_output_imag, hyb_tab_ptr);

    *p_hybrid_re++ = m_temp_output_real[6];
    *p_hybrid_re++ = m_temp_output_real[7];
    *p_hybrid_re++ = m_temp_output_real[0];
    *p_hybrid_re++ = m_temp_output_real[1];
    *p_hybrid_re++ = ixheaac_add32_sat(m_temp_output_real[2], m_temp_output_real[5]);
    *p_hybrid_re++ = ixheaac_add32_sat(m_temp_output_real[3], m_temp_output_real[4]);

    *p_hybrid_im++ = m_temp_output_imag[6];
    *p_hybrid_im++ = m_temp_output_imag[7];
    *p_hybrid_im++ = m_temp_output_imag[0];
    *p_hybrid_im++ = m_temp_output_imag[1];
    *p_hybrid_im++ = ixheaac_add32_sat(m_temp_output_imag[2], m_temp_output_imag[5]);
    *p_hybrid_im++ = ixheaac_add32_sat(m_temp_output_imag[3], m_temp_output_imag[4]);

    ixheaacd_2ch_filtering(
        &(hyb_state->buffer_lf_real[1][time_slot + nr_samples_shift_lf + 1 - PROTO_LEN]),
        m_temp_output_real, hyb_tab_ptr);

    ixheaacd_2ch_filtering(
        &(hyb_state->buffer_lf_imag[1][time_slot + nr_samples_shift_lf + 1 - PROTO_LEN]),
        m_temp_output_imag, hyb_tab_ptr);

    *p_hybrid_re++ = m_temp_output_real[1];
    *p_hybrid_re++ = m_temp_output_real[0];

    *p_hybrid_im++ = m_temp_output_imag[1];
    *p_hybrid_im++ = m_temp_output_imag[0];

    ixheaacd_2ch_filtering(
        &(hyb_state->buffer_lf_real[2][time_slot + nr_samples_shift_lf + 1 - PROTO_LEN]),
        m_temp_output_real, hyb_tab_ptr);

    ixheaacd_2ch_filtering(
        &(hyb_state->buffer_lf_imag[2][time_slot + nr_samples_shift_lf + 1 - PROTO_LEN]),
        m_temp_output_imag, hyb_tab_ptr);

    *p_hybrid_re++ = m_temp_output_real[0];
    *p_hybrid_re++ = m_temp_output_real[1];

    *p_hybrid_im++ = m_temp_output_imag[0];
    *p_hybrid_im++ = m_temp_output_imag[1];

    p_hybrid_real += MAX_HYBRID_BANDS;
    p_hybrid_imag += MAX_HYBRID_BANDS;
  }

  p_qmf_real = m_qmf_real;
  p_qmf_imag = m_qmf_imag;
  for (k = 0; k < nr_qmf_bands_lf; k++) {
    p_qmf_re = p_qmf_real;
    p_qmf_im = p_qmf_imag;
    for (n = MAX_TIME_SLOTS; n < nr_samples_shift_lf; n++) {
      hyb_state->buffer_lf_real[k][n] = hyb_state->qmf_lf_real[k][n];
      hyb_state->buffer_lf_imag[k][n] = hyb_state->qmf_lf_imag[k][n];
    }
    for (n = 0; n < nr_samples; n++) {
      hyb_state->buffer_lf_real[k][n + nr_samples_shift_lf] = *p_qmf_re;
      hyb_state->buffer_lf_imag[k][n + nr_samples_shift_lf] = *p_qmf_im;

      p_qmf_re += MAX_HYBRID_BANDS;
      p_qmf_im += MAX_HYBRID_BANDS;
    }
    p_qmf_real++;
    p_qmf_imag++;
  }
}

ATTR_NO_SANITIZE_INTEGER
VOID ixheaacd_apply_ana_hyb_filt_bank_merge_res_decor(
    ia_mps_dec_thyb_filter_state_struct *hyb_state, WORD32 *m_qmf_real, WORD32 *m_qmf_imag,
    WORD32 nr_bands, WORD32 nr_samples, WORD32 *m_hybrid_real, WORD32 *m_hybrid_imag,
    const ia_mps_dec_hybrid_tables_struct *hyb_tab_ptr) {
  WORD32 nr_samples_shift_lf;
  WORD32 nr_qmf_bands_lf;
  WORD32 k, n;
  WORD32 time_slot;

  WORD32 m_temp_output_real[MAX_HYBRID_ONLY_BANDS_PER_QMF];
  WORD32 m_temp_output_imag[MAX_HYBRID_ONLY_BANDS_PER_QMF];

  WORD32 proto_len = (PROTO_LEN - 1) >> 1;
  WORD32 val = nr_samples - proto_len;
  WORD32 loop_cnt;

  WORD32 *p_qmf_real = m_qmf_real;
  WORD32 *p_qmf_imag = m_qmf_imag;

  WORD32 *p_hybrid_real = m_hybrid_real + 10;
  WORD32 *p_hybrid_imag = m_hybrid_imag + 10;
  WORD32 *p_buffer_lf_real, *p_buffer_lf_imag;

  WORD32 nr_samples_x4 = nr_samples << 2;

  nr_samples_shift_lf = BUFFER_LEN_LF - nr_samples;

  nr_qmf_bands_lf = QMF_BANDS_TO_HYBRID;
  loop_cnt = nr_bands - nr_qmf_bands_lf;

  for (k = 0; k < nr_qmf_bands_lf; k++) {
    for (n = 0; n < nr_samples_shift_lf; n++) {
      hyb_state->buffer_lf_real[k][n] = hyb_state->buffer_lf_real[k][n + nr_samples];
      hyb_state->buffer_lf_imag[k][n] = hyb_state->buffer_lf_imag[k][n + nr_samples];

      hyb_state->qmf_lf_real[k][n] = hyb_state->qmf_lf_real[k][n + nr_samples];
      hyb_state->qmf_lf_imag[k][n] = hyb_state->qmf_lf_imag[k][n + nr_samples];
    }
  }
  for (k = 0; k < nr_qmf_bands_lf; k++) {
    WORD32 *qmf_real = p_qmf_real;
    WORD32 *qmf_imag = p_qmf_imag;
    for (n = 0; n < nr_samples; n++) {
      hyb_state->buffer_lf_real[k][n + nr_samples_shift_lf] = *qmf_real;
      hyb_state->buffer_lf_imag[k][n + nr_samples_shift_lf] = *qmf_imag;

      hyb_state->qmf_lf_imag[k][n + nr_samples_shift_lf] = *qmf_imag++;
      hyb_state->qmf_lf_real[k][n + nr_samples_shift_lf] = *qmf_real++;
    }
    p_qmf_real += MAX_TIME_SLOTS;
    p_qmf_imag += MAX_TIME_SLOTS;
  }

  p_qmf_real = m_qmf_real + nr_qmf_bands_lf * MAX_TIME_SLOTS;
  p_qmf_imag = m_qmf_imag + nr_qmf_bands_lf * MAX_TIME_SLOTS;

  for (k = 0; k < loop_cnt; k++) {
    WORD32 *qmf_real = p_qmf_real + val;
    WORD32 *qmf_imag = p_qmf_imag + val;

    WORD32 *hybrid_real = p_hybrid_real;
    WORD32 *hybrid_imag = p_hybrid_imag;

    for (n = 0; n < proto_len; n++) {
      *hybrid_real = hyb_state->buffer_hf_real[k][n];
      *hybrid_imag = hyb_state->buffer_hf_imag[k][n];

      hyb_state->buffer_hf_real[k][n] = *qmf_real++;
      hyb_state->buffer_hf_imag[k][n] = *qmf_imag++;

      hybrid_real += MAX_HYBRID_BANDS;
      hybrid_imag += MAX_HYBRID_BANDS;
    }

    p_qmf_real += MAX_TIME_SLOTS;
    p_qmf_imag += MAX_TIME_SLOTS;

    p_hybrid_real++;
    p_hybrid_imag++;
  }

  p_qmf_real = m_qmf_real + NR_QMF_BANDS_LFXTS;
  p_qmf_imag = m_qmf_imag + NR_QMF_BANDS_LFXTS;

  p_hybrid_real = m_hybrid_real + 10;
  p_hybrid_imag = m_hybrid_imag + 10;

  for (k = 0; k < loop_cnt; k++) {
    WORD32 *qmf_real = p_qmf_real;
    WORD32 *qmf_imag = p_qmf_imag;

    WORD32 *hybrid_real = p_hybrid_real + proto_len * MAX_HYBRID_BANDS;
    WORD32 *hybrid_imag = p_hybrid_imag + proto_len * MAX_HYBRID_BANDS;

    for (n = 0; n < val; n++) {
      *hybrid_real = *qmf_real++;
      *hybrid_imag = *qmf_imag++;

      hybrid_real += MAX_HYBRID_BANDS;
      hybrid_imag += MAX_HYBRID_BANDS;
    }

    p_qmf_real += MAX_TIME_SLOTS;
    p_qmf_imag += MAX_TIME_SLOTS;

    p_hybrid_real++;
    p_hybrid_imag++;
  }

  p_hybrid_real = m_hybrid_real;
  p_hybrid_imag = m_hybrid_imag;

  for (time_slot = 0; time_slot < nr_samples; time_slot++) {
    WORD32 *hybrid_real = p_hybrid_real;
    WORD32 *hybrid_imag = p_hybrid_imag;

    ixheaacd_8ch_filtering(
        &(hyb_state->buffer_lf_real[0][time_slot + nr_samples_shift_lf + 1 - PROTO_LEN]),
        &(hyb_state->buffer_lf_imag[0][time_slot + nr_samples_shift_lf + 1 - PROTO_LEN]),
        m_temp_output_real, m_temp_output_imag, hyb_tab_ptr);

    *hybrid_real++ = m_temp_output_real[6];
    *hybrid_real++ = m_temp_output_real[7];
    *hybrid_real++ = m_temp_output_real[0];
    *hybrid_real++ = m_temp_output_real[1];
    *hybrid_imag++ = (m_temp_output_imag[2] + m_temp_output_imag[5]);
    *hybrid_real++ = (m_temp_output_real[3] + m_temp_output_real[4]);

    *hybrid_imag++ = m_temp_output_imag[6];
    *hybrid_imag++ = m_temp_output_imag[7];
    *hybrid_imag++ = m_temp_output_imag[0];
    *hybrid_imag++ = m_temp_output_imag[1];
    *hybrid_real++ = (m_temp_output_real[2] + m_temp_output_real[5]);
    *hybrid_imag++ = (m_temp_output_imag[3] + m_temp_output_imag[4]);

    ixheaacd_2ch_filtering(
        &(hyb_state->buffer_lf_real[1][time_slot + nr_samples_shift_lf + 1 - PROTO_LEN]),
        m_temp_output_real, hyb_tab_ptr);

    ixheaacd_2ch_filtering(
        &(hyb_state->buffer_lf_imag[1][time_slot + nr_samples_shift_lf + 1 - PROTO_LEN]),
        m_temp_output_imag, hyb_tab_ptr);

    *hybrid_real++ = m_temp_output_real[1];
    *hybrid_real++ = m_temp_output_real[0];

    *hybrid_imag++ = m_temp_output_imag[0];
    *hybrid_imag++ = m_temp_output_imag[1];

    ixheaacd_2ch_filtering(
        &(hyb_state->buffer_lf_real[2][time_slot + nr_samples_shift_lf + 1 - PROTO_LEN]),
        m_temp_output_real, hyb_tab_ptr);

    ixheaacd_2ch_filtering(
        &(hyb_state->buffer_lf_imag[2][time_slot + nr_samples_shift_lf + 1 - PROTO_LEN]),
        m_temp_output_imag, hyb_tab_ptr);

    *hybrid_real++ = m_temp_output_real[0];
    *hybrid_real++ = m_temp_output_real[1];

    *hybrid_imag++ = m_temp_output_imag[0];
    *hybrid_imag++ = m_temp_output_imag[1];

    p_hybrid_real += MAX_HYBRID_BANDS;
    p_hybrid_imag += MAX_HYBRID_BANDS;
  }

  p_qmf_real = m_qmf_real;
  p_qmf_imag = m_qmf_imag;

  p_buffer_lf_real = &hyb_state->buffer_lf_real[0][nr_samples_shift_lf];
  p_buffer_lf_imag = &hyb_state->buffer_lf_imag[0][nr_samples_shift_lf];

  for (k = 0; k < nr_qmf_bands_lf; k++) {
    for (n = MAX_TIME_SLOTS; n < nr_samples_shift_lf; n++) {
      hyb_state->buffer_lf_real[k][n] = hyb_state->qmf_lf_real[k][n];
      hyb_state->buffer_lf_imag[k][n] = hyb_state->qmf_lf_imag[k][n];
    }
    {
      memcpy(p_buffer_lf_real, p_qmf_real, nr_samples_x4);
      memcpy(p_buffer_lf_imag, p_qmf_imag, nr_samples_x4);
    }
    p_qmf_real += MAX_TIME_SLOTS;
    p_qmf_imag += MAX_TIME_SLOTS;

    p_buffer_lf_real += BUFFER_LEN_LF;
    p_buffer_lf_imag += BUFFER_LEN_LF;
  }
}

VOID ixheaacd_apply_ana_hyb_filt_bank_create_x_res(
    ia_mps_dec_thyb_filter_state_struct *hyb_state, WORD32 *m_qmf_real, WORD32 *m_qmf_imag,
    WORD32 nr_bands, WORD32 nr_samples, WORD32 *m_hybrid_real, WORD32 *m_hybrid_imag,
    SIZE_T *indx, WORD32 res, WORD32 hyb_bands, WORD32 num_parameter_bands, WORD32 *counter,
    const ia_mps_dec_hybrid_tables_struct *hyb_tab_ptr) {
  WORD32 nr_samples_shift_lf;
  WORD32 nr_qmf_bands_lf;
  WORD32 k, n, qs;
  WORD32 time_slot, ch_off_set;
  SIZE_T *idx = indx;

  WORD32 proto_len = (PROTO_LEN - 1) >> 1;
  WORD32 val = nr_samples - proto_len;

  WORD32 *p_qmf_real = m_qmf_real;
  WORD32 *p_qmf_imag = m_qmf_imag;
  WORD32 loop_cnt;

  WORD32 m_temp_output_real[MAX_HYBRID_ONLY_BANDS_PER_QMF];
  WORD32 m_temp_output_imag[MAX_HYBRID_ONLY_BANDS_PER_QMF];

  WORD32 *p_hybrid_real = m_hybrid_real + 10;
  WORD32 *p_hybrid_imag = m_hybrid_imag + 10;

  WORD32 *p_hybrid_re, *p_hybrid_im;

  WORD32 *p_buffer_lf_real, *p_buffer_lf_imag;

  WORD32 nr_samples_x4 = nr_samples << 2;

  nr_samples_shift_lf = BUFFER_LEN_LF - nr_samples;

  nr_qmf_bands_lf = QMF_BANDS_TO_HYBRID;
  loop_cnt = nr_bands - nr_qmf_bands_lf;
  ch_off_set = 0;

  for (k = 0; k < nr_qmf_bands_lf; k++) {
    for (n = 0; n < nr_samples_shift_lf; n++) {
      hyb_state->buffer_lf_real[k][n] = hyb_state->buffer_lf_real[k][n + nr_samples];
      hyb_state->buffer_lf_imag[k][n] = hyb_state->buffer_lf_imag[k][n + nr_samples];

      hyb_state->qmf_lf_real[k][n] = hyb_state->qmf_lf_real[k][n + nr_samples];
      hyb_state->qmf_lf_imag[k][n] = hyb_state->qmf_lf_imag[k][n + nr_samples];
    }
  }
  for (k = 0; k < nr_qmf_bands_lf; k++) {
    WORD32 *qmf_real = p_qmf_real;
    WORD32 *qmf_imag = p_qmf_imag;

    for (n = 0; n < nr_samples; n++) {
      hyb_state->buffer_lf_real[k][n + nr_samples_shift_lf] = *qmf_real;
      hyb_state->buffer_lf_imag[k][n + nr_samples_shift_lf] = *qmf_imag;

      hyb_state->qmf_lf_imag[k][n + nr_samples_shift_lf] = *qmf_imag++;
      hyb_state->qmf_lf_real[k][n + nr_samples_shift_lf] = *qmf_real++;
    }
    p_qmf_real += MAX_TIME_SLOTS;
    p_qmf_imag += MAX_TIME_SLOTS;
  }

  p_qmf_real = m_qmf_real + NR_QMF_BANDS_LFXTS;
  p_qmf_imag = m_qmf_imag + NR_QMF_BANDS_LFXTS;

  for (k = 0; k < loop_cnt; k++) {
    WORD32 *qmf_real = p_qmf_real + val;
    WORD32 *qmf_imag = p_qmf_imag + val;

    p_hybrid_re = p_hybrid_real;
    p_hybrid_im = p_hybrid_imag;

    for (n = 0; n < proto_len; n++) {
      *p_hybrid_re = hyb_state->buffer_hf_real[k][n];
      *p_hybrid_im = hyb_state->buffer_hf_imag[k][n];

      hyb_state->buffer_hf_real[k][n] = *qmf_real++;
      hyb_state->buffer_hf_imag[k][n] = *qmf_imag++;

      p_hybrid_re += MAX_HYBRID_BANDS;
      p_hybrid_im += MAX_HYBRID_BANDS;
    }
    p_qmf_real += MAX_TIME_SLOTS;
    p_qmf_imag += MAX_TIME_SLOTS;

    p_hybrid_real++;
    p_hybrid_imag++;
  }

  p_qmf_real = m_qmf_real + NR_QMF_BANDS_LFXTS;
  p_qmf_imag = m_qmf_imag + NR_QMF_BANDS_LFXTS;

  p_hybrid_real = m_hybrid_real + 10;
  p_hybrid_imag = m_hybrid_imag + 10;

  for (k = 0; k < loop_cnt; k++) {
    WORD32 *qmf_real = p_qmf_real;
    WORD32 *qmf_imag = p_qmf_imag;

    p_hybrid_re = p_hybrid_real + proto_len * MAX_HYBRID_BANDS;
    p_hybrid_im = p_hybrid_imag + proto_len * MAX_HYBRID_BANDS;

    for (n = 0; n < val; n++) {
      *p_hybrid_re = *qmf_real++;
      *p_hybrid_im = *qmf_imag++;

      p_hybrid_re += MAX_HYBRID_BANDS;
      p_hybrid_im += MAX_HYBRID_BANDS;
    }
    p_qmf_real += MAX_TIME_SLOTS;
    p_qmf_imag += MAX_TIME_SLOTS;

    p_hybrid_real++;
    p_hybrid_imag++;
  }

  if (res == 1 && (num_parameter_bands == 20 || num_parameter_bands == 28))
    *counter = 3;
  else {
    idx = indx;
    for (qs = 0; qs < hyb_bands; qs++) {
      if (*idx++ >= (SIZE_T)res) {
        *counter = qs;
        qs = hyb_bands;
      }
    }
  }

  p_hybrid_real = m_hybrid_real;
  p_hybrid_imag = m_hybrid_imag;
  for (time_slot = 0; time_slot < nr_samples; time_slot++) {
    idx = indx;
    p_hybrid_re = p_hybrid_real;
    p_hybrid_im = p_hybrid_imag;

    ixheaacd_8ch_filtering(
        &(hyb_state->buffer_lf_real[0][time_slot + nr_samples_shift_lf + 1 - PROTO_LEN]),
        &(hyb_state->buffer_lf_imag[0][time_slot + nr_samples_shift_lf + 1 - PROTO_LEN]),
        m_temp_output_real, m_temp_output_imag, hyb_tab_ptr);

    *p_hybrid_re++ = m_temp_output_real[6];
    *p_hybrid_re++ = m_temp_output_real[7];
    *p_hybrid_re++ = m_temp_output_real[0];

    *p_hybrid_re++ = m_temp_output_real[1];

    *p_hybrid_im++ = m_temp_output_imag[6];
    *p_hybrid_im++ = m_temp_output_imag[7];
    *p_hybrid_im++ = m_temp_output_imag[0];
    *p_hybrid_im++ = m_temp_output_imag[1];

    if (*counter > 4) {
      *p_hybrid_re++ = ixheaac_add32_sat(m_temp_output_real[2], m_temp_output_real[5]);
      *p_hybrid_im++ = ixheaac_add32_sat(m_temp_output_imag[2], m_temp_output_imag[5]);
    }

    if (*counter > 5) {
      *p_hybrid_re++ = ixheaac_add32_sat(m_temp_output_real[3], m_temp_output_real[4]);
      *p_hybrid_im++ = ixheaac_add32_sat(m_temp_output_imag[3], m_temp_output_imag[4]);
    }

    ch_off_set = 6;
    p_hybrid_re = p_hybrid_real + ch_off_set;
    p_hybrid_im = p_hybrid_imag + ch_off_set;

    ixheaacd_2ch_filtering(
        &(hyb_state->buffer_lf_real[1][time_slot + nr_samples_shift_lf + 1 - PROTO_LEN]),
        m_temp_output_real, hyb_tab_ptr);

    ixheaacd_2ch_filtering(
        &(hyb_state->buffer_lf_imag[1][time_slot + nr_samples_shift_lf + 1 - PROTO_LEN]),
        m_temp_output_imag, hyb_tab_ptr);

    *p_hybrid_re++ = m_temp_output_real[1];
    *p_hybrid_re++ = m_temp_output_real[0];

    *p_hybrid_im++ = m_temp_output_imag[1];
    *p_hybrid_im++ = m_temp_output_imag[0];

    ixheaacd_2ch_filtering(
        &(hyb_state->buffer_lf_real[2][time_slot + nr_samples_shift_lf + 1 - PROTO_LEN]),
        m_temp_output_real, hyb_tab_ptr);

    ixheaacd_2ch_filtering(
        &(hyb_state->buffer_lf_imag[2][time_slot + nr_samples_shift_lf + 1 - PROTO_LEN]),
        m_temp_output_imag, hyb_tab_ptr);

    *p_hybrid_re++ = m_temp_output_real[0];
    *p_hybrid_re++ = m_temp_output_real[1];

    *p_hybrid_im++ = m_temp_output_imag[0];
    *p_hybrid_im++ = m_temp_output_imag[1];

    p_hybrid_real += MAX_HYBRID_BANDS;
    p_hybrid_imag += MAX_HYBRID_BANDS;
  }
  p_qmf_real = m_qmf_real;
  p_qmf_imag = m_qmf_imag;

  p_buffer_lf_real = &hyb_state->buffer_lf_real[0][nr_samples_shift_lf];
  p_buffer_lf_imag = &hyb_state->buffer_lf_imag[0][nr_samples_shift_lf];

  for (k = 0; k < nr_qmf_bands_lf; k++) {
    for (n = MAX_TIME_SLOTS; n < nr_samples_shift_lf; n++) {
      hyb_state->buffer_lf_real[k][n] = hyb_state->qmf_lf_real[k][n];
      hyb_state->buffer_lf_imag[k][n] = hyb_state->qmf_lf_imag[k][n];
    }
    {
      memcpy(p_buffer_lf_real, p_qmf_real, nr_samples_x4);
      memcpy(p_buffer_lf_imag, p_qmf_imag, nr_samples_x4);
    }
    p_qmf_real += MAX_TIME_SLOTS;
    p_qmf_imag += MAX_TIME_SLOTS;

    p_buffer_lf_real += BUFFER_LEN_LF;
    p_buffer_lf_imag += BUFFER_LEN_LF;
  }
}
