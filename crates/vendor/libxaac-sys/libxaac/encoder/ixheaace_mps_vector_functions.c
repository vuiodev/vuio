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

#include <math.h>
#include "ixheaac_type_def.h"
#include "ixheaace_mps_common_fix.h"
#include "ixheaace_mps_defines.h"
#include "ixheaace_mps_common_define.h"

FLOAT32
ixheaace_mps_212_sum_up_cplx_pow_2_dim_2(ixheaace_cmplx_str inp[MAX_ANA_TIME_SLOT][MAX_QMF_BANDS],
                                         const WORD32 start_dim_1, const WORD32 stop_dim_1,
                                         const WORD32 start_dim_2, const WORD32 stop_dim_2) {
  WORD32 idx_1, idx_2;

  FLOAT32 sum;
  sum = 0.0f;
  for (idx_1 = start_dim_1; idx_1 < stop_dim_1; idx_1++) {
    for (idx_2 = start_dim_2; idx_2 < stop_dim_2; idx_2++) {
      sum += inp[idx_1][idx_2].re * inp[idx_1][idx_2].re;
      sum += inp[idx_1][idx_2].im * inp[idx_1][idx_2].im;
    }
  }
  return (sum / 2);
}

VOID ixheaace_mps_212_cplx_scalar_product(
    ixheaace_cmplx_str *const out, ixheaace_cmplx_str inp_1[MAX_ANA_TIME_SLOT][MAX_QMF_BANDS],
    ixheaace_cmplx_str inp_2[MAX_ANA_TIME_SLOT][MAX_QMF_BANDS], const WORD32 start_dim_1,
    const WORD32 stop_dim_1, const WORD32 start_dim_2, const WORD32 stop_dim_2) {
  WORD32 idx_1, idx_2;
  FLOAT32 re_x, re_y, im_x, im_y, re, im;
  re = 0.0f;
  im = 0.0f;

  for (idx_1 = start_dim_1; idx_1 < stop_dim_1; idx_1++) {
    for (idx_2 = start_dim_2; idx_2 < stop_dim_2; idx_2++) {
      re_x = inp_1[idx_1][idx_2].re;
      im_x = inp_1[idx_1][idx_2].im;
      re_y = inp_2[idx_1][idx_2].re;
      im_y = inp_2[idx_1][idx_2].im;
      re += (re_x * re_y) + (im_x * im_y);
      im += (im_x * re_y) - (re_x * im_y);
    }
  }

  out->re = re / 2;
  out->im = im / 2;
}

FLOAT32 ixheaace_mps_212_sum_up_cplx_pow_2(const ixheaace_cmplx_str *const inp,
                                           const WORD32 len) {
  WORD32 idx;
  FLOAT32 sum;
  sum = 0.0f;

  for (idx = 0; idx < len; idx++) {
    sum += inp[idx].re * inp[idx].re;
    sum += inp[idx].im * inp[idx].im;
  }

  return (sum / 2);
}
