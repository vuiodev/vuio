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

#include "ixheaac_type_def.h"
#include "ixheaac_error_standards.h"
#include "ixheaace_error_codes.h"
#include "ixheaace_common_rom.h"
#include "ixheaace_sbr_def.h"
#include "ixheaace_resampler.h"
#include "ixheaace_sbr_rom.h"
#include "ixheaace_sbr_hybrid.h"

#include "ixheaace_aac_constants.h"

VOID ia_enhaacplus_enc_fft(complex *out, WORD32 N, ixheaace_common_tables *);

static VOID ixheaace_four_chan_filtering(const FLOAT32 *ptr_qmf_real, const FLOAT32 *ptr_qmf_imag,
                                         FLOAT32 **ptr_hyb_real, FLOAT32 **ptr_hyb_imag,
                                         WORD32 ch_offset, const FLOAT32 *ptr_p4_13,
                                         ixheaace_common_tables *pstr_common_tab) {
  WORD32 i, k, n;
  WORD32 mid_tap = IXHEAACE_HYBRID_FILTER_DELAY;

  FLOAT32 cum[8], *ptr_cum;
  FLOAT32 tmp1, tmp2, tmp_p4;
  FLOAT32 real, imag;
  const FLOAT32 *ptr_re, *ptr_im;
  FLOAT32 *ptr_hy_im, *ptr_hy_re;
  ptr_re = &ptr_qmf_real[0];
  ptr_im = &ptr_qmf_imag[0];
  ptr_hy_re = &ptr_hyb_real[0][ch_offset];
  ptr_hy_im = &ptr_hyb_imag[0][ch_offset];

  for (i = IXHEAACE_QMF_TIME_SLOTS - 1; i >= 0; i--) {
    real = imag = 0;
    for (k = 0; k < 16; k += 4) {
      tmp_p4 = ptr_p4_13[k];
      tmp1 = ptr_re[k];
      tmp2 = ptr_im[k];

      tmp1 = tmp_p4 * tmp1;
      real = real - tmp1;

      tmp1 = tmp_p4 * tmp2;
      imag = imag + tmp1;
    }

    cum[3] = imag;
    cum[2] = real;
    real = imag = 0;

    for (k = 3; k < 15; k += 4) {
      tmp_p4 = ptr_p4_13[k];
      tmp1 = ptr_re[k];
      tmp2 = ptr_im[k];
      tmp1 = tmp_p4 * tmp1;
      real = real + tmp1;
      tmp1 = tmp_p4 * tmp2;
      imag = imag + tmp1;
    }

    tmp1 = imag + real;
    cum[7] = tmp1 * IXHEAACE_COS_PI_BY_4;
    tmp1 = imag - real;
    cum[6] = tmp1 * IXHEAACE_COS_PI_BY_4;

    cum[1] = ptr_p4_13[mid_tap] * ptr_re[mid_tap];
    cum[0] = ptr_p4_13[mid_tap] * ptr_im[mid_tap];

    real = imag = 0;

    for (k = 1; k < 13; k += 4) {
      tmp_p4 = ptr_p4_13[k];
      tmp1 = ptr_re[k];
      tmp2 = ptr_im[k];
      tmp1 = tmp_p4 * tmp1;
      real = real + tmp1;
      tmp1 = tmp_p4 * tmp2;
      imag = imag + tmp1;
    }

    tmp1 = real - imag;
    cum[5] = tmp1 * IXHEAACE_COS_PI_BY_4;
    tmp1 = real + imag;
    cum[4] = tmp1 * IXHEAACE_COS_PI_BY_4;

    ia_enhaacplus_enc_fft((complex *)cum, 4, pstr_common_tab);

    ptr_cum = &cum[0];

    for (n = 3; n >= 0; n--) {
      tmp1 = *ptr_cum++;
      tmp2 = *ptr_cum++;
      *ptr_hy_im++ = tmp1;
      *ptr_hy_re++ = tmp2;
    }
    ptr_re++;
    ptr_im++;
    ptr_hy_re += IXHEAACE_QMF_TIME_SLOTS - 4;
    ptr_hy_im += IXHEAACE_QMF_TIME_SLOTS - 4;
  }
}

static VOID ixheaace_eight_chan_filtering(const FLOAT32 *ptr_qmf_real,
                                          const FLOAT32 *ptr_qmf_imag, FLOAT32 **ptr_hyb_real,
                                          FLOAT32 **ptr_hyb_imag, const FLOAT32 *ptr_p8_13,
                                          ixheaace_common_tables *pstr_common_tab)

{
  LOOPINDEX i, n;
  LOOPINDEX mid_tap = IXHEAACE_HYBRID_FILTER_DELAY;
  FLOAT32 real, imag;
  FLOAT32 cum[16], *ptr_cum;
  FLOAT32 tmp1, tmp2, tmp3;
  const FLOAT32 *ptr_re, *ptr_im;
  FLOAT32 *ptr_hy_re, *ptr_hy_im;
  ptr_re = &ptr_qmf_real[0];
  ptr_im = &ptr_qmf_imag[0];
  ptr_hy_re = &ptr_hyb_real[0][0];
  ptr_hy_im = &ptr_hyb_imag[0][0];

  for (i = IXHEAACE_QMF_TIME_SLOTS - 1; i >= 0; i--) {
    tmp1 = ptr_p8_13[4] * ptr_re[4];
    tmp2 = ptr_p8_13[12] * ptr_re[12];

    tmp3 = ptr_im[4];
    real = tmp1 + tmp2;
    tmp2 = ptr_im[12];
    tmp1 = ptr_p8_13[4] * tmp3;
    tmp2 = ptr_p8_13[12] * tmp2;
    imag = tmp1 + tmp2;

    tmp1 = imag - real;
    cum[5] = tmp1 * IXHEAACE_COS_PI_BY_4;

    tmp1 = imag + real;
    tmp3 = ptr_re[3];
    cum[4] = -tmp1 * IXHEAACE_COS_PI_BY_4;
    tmp2 = ptr_re[11];
    tmp1 = ptr_p8_13[3] * tmp3;
    tmp2 = ptr_p8_13[11] * tmp2;
    tmp3 = ptr_im[3];
    real = tmp1 + tmp2;
    tmp2 = ptr_im[11];
    tmp1 = ptr_p8_13[3] * tmp3;
    tmp2 = ptr_p8_13[11] * tmp2;
    imag = tmp1 + tmp2;

    tmp1 = imag * IXHEAACE_COS_PI_BY_8;
    tmp2 = real * IXHEAACE_SIN_PI_BY_8;
    cum[13] = tmp1 - tmp2;

    tmp1 = imag * IXHEAACE_SIN_PI_BY_8;
    tmp2 = real * IXHEAACE_COS_PI_BY_8;
    tmp3 = ptr_re[2];
    cum[12] = -tmp1 - tmp2;
    tmp2 = ptr_re[10];
    tmp1 = ptr_p8_13[2] * tmp3;
    tmp2 = ptr_p8_13[10] * tmp2;
    tmp3 = ptr_im[2];
    cum[2] = -tmp1 - tmp2;
    tmp2 = ptr_im[10];
    tmp1 = ptr_p8_13[2] * tmp3;
    tmp2 = ptr_p8_13[10] * tmp2;
    cum[3] = tmp1 + tmp2;

    tmp1 = ptr_p8_13[1] * ptr_re[1];
    tmp2 = ptr_p8_13[9] * ptr_re[9];
    real = tmp1 + tmp2;

    tmp1 = ptr_p8_13[1] * ptr_im[1];
    tmp2 = ptr_p8_13[9] * ptr_im[9];
    imag = tmp1 + tmp2;

    tmp1 = imag * IXHEAACE_COS_PI_BY_8;
    tmp2 = real * IXHEAACE_SIN_PI_BY_8;
    cum[11] = tmp1 + tmp2;

    tmp1 = imag * IXHEAACE_SIN_PI_BY_8;
    tmp2 = real * IXHEAACE_COS_PI_BY_8;
    cum[10] = tmp1 - tmp2;

    tmp1 = ptr_p8_13[0] * ptr_re[0];
    tmp2 = ptr_p8_13[8] * ptr_re[8];
    real = tmp1 + tmp2;

    tmp1 = ptr_p8_13[0] * ptr_im[0];
    tmp2 = ptr_p8_13[8] * ptr_im[8];
    imag = tmp1 + tmp2;

    tmp1 = imag + real;
    cum[7] = tmp1 * IXHEAACE_COS_PI_BY_4;

    tmp1 = imag - real;
    cum[6] = tmp1 * IXHEAACE_COS_PI_BY_4;

    real = ptr_p8_13[7] * ptr_re[7];
    imag = ptr_p8_13[7] * ptr_im[7];

    tmp1 = imag * IXHEAACE_SIN_PI_BY_8;
    tmp2 = real * IXHEAACE_COS_PI_BY_8;
    cum[15] = tmp1 + tmp2;

    tmp1 = imag * IXHEAACE_COS_PI_BY_8;
    tmp2 = real * IXHEAACE_SIN_PI_BY_8;
    cum[14] = tmp1 - tmp2;

    cum[1] = ptr_p8_13[mid_tap] * ptr_re[mid_tap];
    cum[0] = ptr_p8_13[mid_tap] * ptr_im[mid_tap];

    real = ptr_p8_13[5] * ptr_re[5];
    imag = ptr_p8_13[5] * ptr_im[5];

    tmp1 = real * IXHEAACE_COS_PI_BY_8;
    tmp2 = imag * IXHEAACE_SIN_PI_BY_8;
    cum[9] = tmp1 - tmp2;

    tmp1 = real * IXHEAACE_SIN_PI_BY_8;
    tmp2 = imag * IXHEAACE_COS_PI_BY_8;
    cum[8] = tmp1 + tmp2;

    ia_enhaacplus_enc_fft((complex *)cum, 8, pstr_common_tab);

    ptr_cum = &cum[0];

    for (n = 7; n >= 0; n--) {
      tmp1 = *ptr_cum++;
      tmp2 = *ptr_cum++;
      *ptr_hy_im++ = tmp1;
      *ptr_hy_re++ = tmp2;
    }
    ptr_re++;
    ptr_im++;
    ptr_hy_re += IXHEAACE_QMF_TIME_SLOTS - 8;
    ptr_hy_im += IXHEAACE_QMF_TIME_SLOTS - 8;
  }
}

IA_ERRORCODE ixheaace_hybrid_analysis(const FLOAT32 **ptr_qmf_real_in,
                                      const FLOAT32 **ptr_qmf_imag_in, FLOAT32 **ptr_hyb_real_in,
                                      FLOAT32 **ptr_hyb_imag_in, ixheaace_pstr_hybrid pstr_hybrid,
                                      ixheaace_str_ps_tab *pstr_ps_tab,
                                      ixheaace_common_tables *pstr_common_tab) {
  WORD32 band, i;
  ixheaace_hybrid_res hybrid_res;
  WORD32 ch_offset = 0;

  FLOAT32 *ptr_re, *ptr_im;
  const FLOAT32 *ptr_qmf_real, *ptr_qmf_imag;
  FLOAT32 tmp1, tmp2;

  for (band = 0; band < IXHEAACE_NUM_QMF_BANDS_IN_HYBRID; band++) {
    hybrid_res = (ixheaace_hybrid_res)pstr_ps_tab->a_hyb_res[band];

    memcpy(pstr_hybrid->ptr_work_real, pstr_hybrid->ptr_qmf_buf_real[band],
           IXHEAACE_QMF_BUFFER_MOVE * sizeof(FLOAT32));
    memcpy(pstr_hybrid->ptr_work_imag, pstr_hybrid->ptr_qmf_buf_imag[band],
           IXHEAACE_QMF_BUFFER_MOVE * sizeof(FLOAT32));
    ptr_re = &pstr_hybrid->ptr_work_real[IXHEAACE_QMF_BUFFER_MOVE];
    ptr_im = &pstr_hybrid->ptr_work_imag[IXHEAACE_QMF_BUFFER_MOVE];
    ptr_qmf_real = &ptr_qmf_real_in[0][band];
    ptr_qmf_imag = &ptr_qmf_imag_in[0][band];

    for (i = IXHEAACE_QMF_TIME_SLOTS - 1; i >= 0; i--) {
      tmp1 = *ptr_qmf_real;
      tmp2 = *ptr_qmf_imag;

      ptr_qmf_real += IXHEAACE_QMF_CHANNELS;
      ptr_qmf_imag += IXHEAACE_QMF_CHANNELS;

      *ptr_im++ = tmp2;
      *ptr_re++ = tmp1;
    }

    ptr_re = &pstr_hybrid->ptr_qmf_buf_real[band][0];
    ptr_im = &pstr_hybrid->ptr_qmf_buf_imag[band][0];

    ptr_qmf_real = &ptr_qmf_real_in[IXHEAACE_QMF_TIME_SLOTS - IXHEAACE_QMF_BUFFER_MOVE][band];
    ptr_qmf_imag = &ptr_qmf_imag_in[IXHEAACE_QMF_TIME_SLOTS - IXHEAACE_QMF_BUFFER_MOVE][band];

    for (i = 0; i < IXHEAACE_QMF_BUFFER_MOVE; i++) {
      tmp1 = *ptr_qmf_real;
      ptr_qmf_real += IXHEAACE_QMF_CHANNELS;
      tmp2 = *ptr_qmf_imag;
      ptr_qmf_imag += IXHEAACE_QMF_CHANNELS;
      *ptr_re++ = tmp1;
      *ptr_im++ = tmp2;
    }

    switch (hybrid_res) {
      case IXHEAACE_HYBRID_4_CPLX:
        ixheaace_four_chan_filtering(pstr_hybrid->ptr_work_real, pstr_hybrid->ptr_work_imag,
                                     ptr_hyb_real_in, ptr_hyb_imag_in, ch_offset,
                                     pstr_ps_tab->p4_13, pstr_common_tab);
        break;
      case IXHEAACE_HYBRID_8_CPLX:
        ixheaace_eight_chan_filtering(pstr_hybrid->ptr_work_real, pstr_hybrid->ptr_work_imag,
                                      ptr_hyb_real_in, ptr_hyb_imag_in, pstr_ps_tab->p8_13,
                                      pstr_common_tab);
        break;
      default:
        return IA_EXHEAACE_EXE_FATAL_PS_INVALID_HYBRID_RES_VAL;
        break;
    }
    ch_offset += hybrid_res;
  }
  return IA_NO_ERROR;
}

VOID ixheaace_hybrid_synthesis(const FLOAT32 **ptr_hybrid_real_flt,
                               const FLOAT32 **ptr_hybrid_imag_flt, FLOAT32 **ptr_qmf_real_flt,
                               FLOAT32 **ptr_qmf_imag_flt, const WORD32 *ptr_hyb_res) {
  WORD32 k, n, band;
  ixheaace_hybrid_res hybrid_res;
  WORD32 ch_offset = 0;

  FLOAT32 temp1, temp2;
  FLOAT32 *ptr_qmf_real;
  FLOAT32 *ptr_qmf_imag;

  for (band = 0; band < IXHEAACE_NUM_QMF_BANDS_IN_HYBRID; band++) {
    const FLOAT32 *ptr_hybrid_real = &ptr_hybrid_real_flt[0][ch_offset];
    const FLOAT32 *ptr_hybrid_imag = &ptr_hybrid_imag_flt[0][ch_offset];

    hybrid_res = (ixheaace_hybrid_res)ptr_hyb_res[band];

    ptr_qmf_real = &ptr_qmf_real_flt[0][band];
    ptr_qmf_imag = &ptr_qmf_imag_flt[0][band];

    for (n = 0; n < IXHEAACE_QMF_TIME_SLOTS; n++) {
      FLOAT32 temo_real = 0, temo_imag = 0;

      for (k = hybrid_res - 1; k >= 0; k--) {
        temp1 = *ptr_hybrid_real++;
        temp2 = *ptr_hybrid_imag++;
        temo_real += temp1;
        temo_imag += temp2;
      }

      ptr_hybrid_real += IXHEAACE_QMF_TIME_SLOTS - hybrid_res;
      ptr_hybrid_imag += IXHEAACE_QMF_TIME_SLOTS - hybrid_res;

      *ptr_qmf_real = temo_real;
      ptr_qmf_real += IXHEAACE_QMF_CHANNELS;

      *ptr_qmf_imag = temo_imag;
      ptr_qmf_imag += IXHEAACE_QMF_CHANNELS;
    }
    ch_offset += hybrid_res;
  }
}
