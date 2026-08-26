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

#include "ixheaacd_cnst.h"
#include "ixheaac_type_def.h"
#include "ixheaacd_bitbuffer.h"
#include "ixheaacd_acelp_com.h"

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

#include "ixheaac_constants.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaacd_ec_defines.h"
#include "ixheaacd_ec_struct_def.h"
#include "ixheaacd_main.h"
#include "ixheaacd_arith_dec.h"

#define FREQ_MAX 6400.0f

#define ABS(A) ((A) < 0 ? (-A) : (A))

static VOID ixheaacd_compute_coeff_poly_f(FLOAT32 lsp[], FLOAT32 *f1,
                                          FLOAT32 *f2) {
  FLOAT32 b1, b2;
  FLOAT32 *ptr_lsp;
  WORD32 i, j;

  ptr_lsp = lsp;
  f1[0] = f2[0] = 1.0f;

  for (i = 1; i <= ORDER_BY_2; i++) {
    b1 = -2.0f * (*ptr_lsp++);
    b2 = -2.0f * (*ptr_lsp++);
    f1[i] = (b1 * f1[i - 1]) + (2.0f * f1[i - 2]);
    f2[i] = (b2 * f2[i - 1]) + (2.0f * f2[i - 2]);
    for (j = i - 1; j > 0; j--) {
      f1[j] += (b1 * f1[j - 1]) + f1[j - 2];
      f2[j] += (b2 * f2[j - 1]) + f2[j - 2];
    }
  }

  return;
}
VOID ixheaacd_lsp_to_lp_conversion(FLOAT32 *lsp, FLOAT32 *lp_flt_coff_a) {
  WORD32 i;
  FLOAT32 *ppoly_f1, *ppoly_f2;
  FLOAT32 *plp_flt_coff_a_bott, *plp_flt_coff_a_top;
  FLOAT32 poly1[ORDER_BY_2 + 2], poly2[ORDER_BY_2 + 2];

  poly1[0] = 0.0f;
  poly2[0] = 0.0f;

  ixheaacd_compute_coeff_poly_f(lsp, &poly1[1], &poly2[1]);

  ppoly_f1 = poly1 + ORDER_BY_2 + 1;
  ppoly_f2 = poly2 + ORDER_BY_2 + 1;

  for (i = 0; i < ORDER_BY_2; i++) {
    ppoly_f1[0] += ppoly_f1[-1];
    ppoly_f2[0] -= ppoly_f2[-1];
    ppoly_f1--;
    ppoly_f2--;
  }

  plp_flt_coff_a_bott = lp_flt_coff_a;
  *plp_flt_coff_a_bott++ = 1.0f;
  plp_flt_coff_a_top = lp_flt_coff_a + ORDER;
  ppoly_f1 = poly1 + 2;
  ppoly_f2 = poly2 + 2;
  for (i = 0; i < ORDER_BY_2; i++) {
    *plp_flt_coff_a_bott++ = 0.5f * (*ppoly_f1 + *ppoly_f2);
    *plp_flt_coff_a_top-- = 0.5f * (*ppoly_f1++ - *ppoly_f2++);
  }

  return;
}

VOID ixheaacd_lpc_to_td(FLOAT32 *coeff, WORD32 order, FLOAT32 *gains, WORD32 lg) {
  FLOAT32 data_r[LEN_SUPERFRAME * 2];
  FLOAT32 data_i[LEN_SUPERFRAME * 2];
  FLOAT64 avg_fac;
  WORD32 idata_r[LEN_SUPERFRAME * 2];
  WORD32 idata_i[LEN_SUPERFRAME * 2];
  WORD8 qshift;
  WORD32 preshift = 0;
  WORD32 itemp;
  FLOAT32 ftemp = 0;
  FLOAT32 tmp, qfac;
  WORD32 i, size_n;

  size_n = 2 * lg;
  avg_fac = PI / (FLOAT32)(size_n);

  for (i = 0; i < order + 1; i++) {
    tmp = (FLOAT32)(((FLOAT32)i) * avg_fac);
    data_r[i] = (FLOAT32)(coeff[i] * cos(tmp));
    data_i[i] = (FLOAT32)(-coeff[i] * sin(tmp));
  }
  for (; i < size_n; i++) {
    data_r[i] = 0.f;
    data_i[i] = 0.f;
  }

  for (i = 0; i < size_n; i++) {
    if (ABS(data_r[i]) > ftemp) ftemp = ABS(data_r[i]);
    if (ABS(data_i[i]) > ftemp) ftemp = ABS(data_i[i]);
  }

  itemp = (WORD32)ftemp;
  qshift = ixheaac_norm32(itemp);

  for (i = 0; i < size_n; i++) {
    idata_r[i] = (WORD32)(data_r[i] * ((WORD64)1 << qshift));
    idata_i[i] = (WORD32)(data_i[i] * ((WORD64)1 << qshift));
  }

  ixheaacd_complex_fft(idata_r, idata_i, size_n, -1, &preshift);

  qfac = 1.0f / ((FLOAT32)((WORD64)1 << (qshift - preshift)));

  for (i = 0; i < size_n; i++) {
    data_r[i] = (FLOAT32)((FLOAT32)idata_r[i] * qfac);
    data_i[i] = (FLOAT32)((FLOAT32)idata_i[i] * qfac);
  }

  for (i = 0; i < size_n / 2; i++) {
    gains[i] =
        (FLOAT32)(1.0f / (sqrt(data_r[i] * data_r[i] + data_i[i] * data_i[i]) + FLT_EPSILON));
  }

  return;
}

VOID ixheaacd_noise_shaping(FLOAT32 r[], WORD32 lg, WORD32 M, FLOAT32 g1[],
                            FLOAT32 g2[]) {
  WORD32 i, k;
  FLOAT32 rr_prev, a = 0, b = 0;
  FLOAT32 rr[1024];

  k = lg / M;

  rr_prev = 0;

  memcpy(&rr, r, lg * sizeof(FLOAT32));

  for (i = 0; i < lg; i++) {
    if ((i % k) == 0) {
      a = 2.0f * g1[i / k] * g2[i / k] / (g1[i / k] + g2[i / k]);
      b = (g2[i / k] - g1[i / k]) / (g1[i / k] + g2[i / k]);
    }

    rr[i] = a * rr[i] + b * rr_prev;
    rr_prev = rr[i];
  }

  for (i = 0; i < lg / 2; i++) {
    r[i] = rr[2 * i];
    r[lg / 2 + i] = rr[lg - 2 * i - 1];
  }
  return;
}

VOID ixheaacd_lpc_coef_gen(FLOAT32 lsf_old[], FLOAT32 lsf_new[], FLOAT32 a[],
                           WORD32 nb_subfr, WORD32 m) {
  FLOAT32 lsf[ORDER], *ptr_a;
  FLOAT32 inc, fnew, fold;
  WORD32 i;

  ptr_a = a;

  inc = 1.0f / (FLOAT32)nb_subfr;
  fnew = 0.5f - (0.5f * inc);
  fold = 1.0f - fnew;

  for (i = 0; i < m; i++) {
    lsf[i] = (lsf_old[i] * fold) + (lsf_new[i] * fnew);
  }
  ixheaacd_lsp_to_lp_conversion(lsf, ptr_a);
  ptr_a += (m + 1);
  ixheaacd_lsp_to_lp_conversion(lsf_old, ptr_a);
  ptr_a += (m + 1);
  ixheaacd_lsp_to_lp_conversion(lsf_new, ptr_a);
  ptr_a += (m + 1);

  return;
}

VOID ixheaacd_interpolation_lsp_params(FLOAT32 lsp_old[], FLOAT32 lsp_new[],
                                       FLOAT32 lp_flt_coff_a[],
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

    ixheaacd_lsp_to_lp_conversion(lsp, lp_flt_coff_a);

    lp_flt_coff_a += (ORDER + 1);
  }

  ixheaacd_lsp_to_lp_conversion(lsp_new, lp_flt_coff_a);

  return;
}
