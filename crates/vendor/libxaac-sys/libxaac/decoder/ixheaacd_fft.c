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
#include <stdio.h>

#include "ixheaac_type_def.h"
#include "ixheaacd_interface.h"
#include "ixheaac_constants.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaacd_function_selector.h"

extern const WORD32 ixheaacd_twiddle_table_fft_32x32[514];
extern const FLOAT32 ixheaacd_twiddle_table_fft[514];
extern const FLOAT32 ixheaacd_twiddle_table_fft_flt[16];
extern const WORD32 ixheaacd_twiddle_table_3pr[1155];
extern const WORD32 ixheaacd_twiddle_table_3pi[1155];
extern const WORD8 ixheaacd_mps_dig_rev[8];

#define PLATFORM_INLINE __inline

#define DIG_REV(i, m, j)                                    \
  do {                                                      \
    unsigned _ = (i);                                       \
    _ = ((_ & 0x33333333) << 2) | ((_ & ~0x33333333) >> 2); \
    _ = ((_ & 0x0F0F0F0F) << 4) | ((_ & ~0x0F0F0F0F) >> 4); \
    _ = ((_ & 0x00FF00FF) << 8) | ((_ & ~0x00FF00FF) >> 8); \
    (j) = _ >> (m);                                         \
  } while (0)

static PLATFORM_INLINE WORD32 ixheaacd_mult32_sat(WORD32 a, WORD32 b) {
  WORD32 result;
  WORD64 temp_result;

  temp_result = (WORD64)a * (WORD64)b;
  result = ixheaac_sat64_32(temp_result >> 31);

  return (result);
}

static PLATFORM_INLINE WORD32 ixheaacd_mac32_sat(WORD32 a, WORD32 b, WORD32 c) {
  WORD32 result;

  result = ixheaac_add32_sat(a, ixheaacd_mult32_sat(b, c));

  return (result);
}

static PLATFORM_INLINE FLOAT32 ixheaacd_mult32X32float(FLOAT32 a, FLOAT32 b) {
  FLOAT32 result;

  result = a * b;

  return result;
}

static PLATFORM_INLINE FLOAT32 ixheaacd_mac32X32float(FLOAT32 a, FLOAT32 b, FLOAT32 c) {
  FLOAT32 result;

  result = a + b * c;

  return result;
}

VOID ixheaacd_mps_synth_calc_fft(FLOAT32 *ptr_xr, FLOAT32 *ptr_xi,
                                 WORD32 npoints) {
  WORD32 i, j, k;
  FLOAT32 y[64], z[64];
  FLOAT32 *ptr_y = y, *ptr_z = z;
  const FLOAT32 *ptr_w = ixheaacd_twiddle_table_fft_flt;

  for (i = 0; i < npoints; i += 4) {
    FLOAT32 x0r, x0i, x1r, x1i, x2r, x2i, x3r, x3i;
    FLOAT32 *inp = ptr_xr;
    FLOAT32 tmk;

    WORD32 h2 = ixheaacd_mps_dig_rev[i >> 2];

    inp += (h2);

    x0r = *inp;
    x0i = *(inp + 1);
    inp += 16;

    x1r = *inp;
    x1i = *(inp + 1);
    inp += 16;

    x2r = *inp;
    x2i = *(inp + 1);
    inp += 16;

    x3r = *inp;
    x3i = *(inp + 1);

    x0r = x0r + x2r;
    x0i = x0i + x2i;

    tmk = x0r - x2r;
    x2r = tmk - x2r;
    tmk = x0i - x2i;
    x2i = tmk - x2i;

    x1r = x1r + x3r;
    x1i = x1i + x3i;

    tmk = x1r - x3r;
    x3r = tmk - x3r;
    tmk = x1i - x3i;
    x3i = tmk - x3i;

    x0r = x0r + x1r;
    x0i = x0i + x1i;

    tmk = x0r - x1r;
    x1r = tmk - x1r;
    tmk = x0i - x1i;
    x1i = tmk - x1i;

    x2r = x2r + x3i;
    x2i = x2i - x3r;

    tmk = x2r - x3i;
    x3i = tmk - x3i;
    tmk = x2i + x3r;
    x3r = tmk + x3r;

    *ptr_y++ = x0r;
    *ptr_y++ = x0i;
    *ptr_y++ = x2r;
    *ptr_y++ = x2i;
    *ptr_y++ = x1r;
    *ptr_y++ = x1i;
    *ptr_y++ = x3i;
    *ptr_y++ = x3r;

    inp = ptr_xi;

    inp += (h2);

    x0r = *inp;
    x0i = *(inp + 1);
    inp += 16;

    x1r = *inp;
    x1i = *(inp + 1);
    inp += 16;

    x2r = *inp;
    x2i = *(inp + 1);
    inp += 16;

    x3r = *inp;
    x3i = *(inp + 1);

    x0r = x0r + x2r;
    x0i = x0i + x2i;

    tmk = x0r - x2r;
    x2r = tmk - x2r;
    tmk = x0i - x2i;
    x2i = tmk - x2i;

    x1r = x1r + x3r;
    x1i = x1i + x3i;

    tmk = x1r - x3r;
    x3r = tmk - x3r;
    tmk = x1i - x3i;
    x3i = tmk - x3i;

    x0r = x0r + x1r;
    x0i = x0i + x1i;

    tmk = x0r - x1r;
    x1r = tmk - x1r;
    tmk = x0i - x1i;
    x1i = tmk - x1i;

    x2r = x2r + x3i;
    x2i = x2i - x3r;

    tmk = x2r - x3i;
    x3i = tmk - x3i;
    tmk = x2i + x3r;
    x3r = tmk + x3r;

    *ptr_z++ = x0r;
    *ptr_z++ = x0i;
    *ptr_z++ = x2r;
    *ptr_z++ = x2i;
    *ptr_z++ = x1r;
    *ptr_z++ = x1i;
    *ptr_z++ = x3i;
    *ptr_z++ = x3r;
  }
  ptr_y -= 64;
  ptr_z -= 64;
  {
    FLOAT32 *data_r = ptr_y;
    FLOAT32 *data_i = ptr_z;
    for (k = 2; k != 0; k--) {
      FLOAT32 x0r, x0i, x1r, x1i, x2r, x2i, x3r, x3i;

      x0r = (*data_r);
      x0i = (*(data_r + 1));
      data_r += 8;

      x1r = (*data_r);
      x1i = (*(data_r + 1));
      data_r += 8;

      x2r = (*data_r);
      x2i = (*(data_r + 1));
      data_r += 8;

      x3r = (*data_r);
      x3i = (*(data_r + 1));
      data_r -= 24;

      x0r = x0r + x2r;
      x0i = x0i + x2i;
      x2r = x0r - (x2r * 2);
      x2i = x0i - (x2i * 2);
      x1r = x1r + x3r;
      x1i = x1i + x3i;
      x3r = x1r - (x3r * 2);
      x3i = x1i - (x3i * 2);

      x0r = x0r + x1r;
      x0i = x0i + x1i;
      x1r = x0r - (x1r * 2);
      x1i = x0i - (x1i * 2);
      x2r = x2r + x3i;
      x2i = x2i - x3r;
      x3i = x2r - (x3i * 2);
      x3r = x2i + (x3r * 2);

      *data_r = x0r;
      *(data_r + 1) = x0i;
      data_r += 8;

      *data_r = x2r;
      *(data_r + 1) = x2i;
      data_r += 8;

      *data_r = x1r;
      *(data_r + 1) = x1i;
      data_r += 8;

      *data_r = x3i;
      *(data_r + 1) = x3r;
      data_r += 8;

      x0r = (*data_i);
      x0i = (*(data_i + 1));
      data_i += 8;

      x1r = (*data_i);
      x1i = (*(data_i + 1));
      data_i += 8;

      x2r = (*data_i);
      x2i = (*(data_i + 1));
      data_i += 8;

      x3r = (*data_i);
      x3i = (*(data_i + 1));
      data_i -= 24;

      x0r = x0r + x2r;
      x0i = x0i + x2i;
      x2r = x0r - (x2r * 2);
      x2i = x0i - (x2i * 2);
      x1r = x1r + x3r;
      x1i = x1i + x3i;
      x3r = x1r - (x3r * 2);
      x3i = x1i - (x3i * 2);

      x0r = x0r + x1r;
      x0i = x0i + x1i;
      x1r = x0r - (x1r * 2);
      x1i = x0i - (x1i * 2);
      x2r = x2r + x3i;
      x2i = x2i - x3r;
      x3i = x2r - (x3i * 2);
      x3r = x2i + (x3r * 2);

      *data_i = x0r;
      *(data_i + 1) = x0i;
      data_i += 8;

      *data_i = x2r;
      *(data_i + 1) = x2i;
      data_i += 8;

      *data_i = x1r;
      *(data_i + 1) = x1i;
      data_i += 8;

      *data_i = x3i;
      *(data_i + 1) = x3r;
      data_i += 8;
    }
    data_r = ptr_y + 2;
    data_i = ptr_z + 2;

    for (k = 2; k != 0; k--) {
      FLOAT32 tmp;
      FLOAT32 x0r, x0i, x1r, x1i, x2r, x2i, x3r, x3i;

      data_r += 8;

      x1r = *data_r;
      x1i = *(data_r + 1);
      data_r += 8;

      x2r = *data_r;
      x2i = *(data_r + 1);
      data_r += 8;

      x3r = *data_r;
      x3i = *(data_r + 1);
      data_r -= 24;

      tmp = (FLOAT32)(ixheaacd_mult32X32float((FLOAT32)x1r, 0.923880f) -
                      ixheaacd_mult32X32float((FLOAT32)x1i, -0.382683f));
      x1i = (FLOAT32)ixheaacd_mac32X32float(ixheaacd_mult32X32float((FLOAT32)x1r, -0.382683f),
                                   (FLOAT32)x1i, 0.923880f);
      x1r = tmp;

      tmp = (FLOAT32)(ixheaacd_mult32X32float((FLOAT32)x2r, 0.707107f) -
                      ixheaacd_mult32X32float((FLOAT32)x2i, -0.707107f));
      x2i = (FLOAT32)ixheaacd_mac32X32float(ixheaacd_mult32X32float((FLOAT32)x2r, -0.707107f),
                                   (FLOAT32)x2i, 0.707107f);
      x2r = tmp;

      tmp = (FLOAT32)(ixheaacd_mult32X32float((FLOAT32)x3r, 0.382683f) -
                      ixheaacd_mult32X32float((FLOAT32)x3i, -0.923880f));
      x3i = (FLOAT32)ixheaacd_mac32X32float(ixheaacd_mult32X32float((FLOAT32)x3r, -0.923880f),
                                   (FLOAT32)x3i, 0.382683f);
      x3r = tmp;

      x0r = (*data_r);
      x0i = (*(data_r + 1));

      x0r = x0r + (x2r);
      x0i = x0i + (x2i);
      x2r = x0r - (x2r * 2);
      x2i = x0i - (x2i * 2);
      x1r = x1r + x3r;
      x1i = x1i + x3i;
      x3r = x1r - (x3r * 2);
      x3i = x1i - (x3i * 2);

      x0r = x0r + (x1r);
      x0i = x0i + (x1i);
      x1r = x0r - (x1r * 2);
      x1i = x0i - (x1i * 2);
      x2r = x2r + (x3i);
      x2i = x2i - (x3r);
      x3i = x2r - (x3i * 2);
      x3r = x2i + (x3r * 2);

      *data_r = x0r;
      *(data_r + 1) = x0i;
      data_r += 8;

      *data_r = x2r;
      *(data_r + 1) = x2i;
      data_r += 8;

      *data_r = x1r;
      *(data_r + 1) = x1i;
      data_r += 8;

      *data_r = x3i;
      *(data_r + 1) = x3r;
      data_r += 8;
      data_i += 8;

      x1r = *data_i;
      x1i = *(data_i + 1);
      data_i += 8;

      x2r = *data_i;
      x2i = *(data_i + 1);
      data_i += 8;

      x3r = *data_i;
      x3i = *(data_i + 1);
      data_i -= 24;

      tmp = (FLOAT32)(ixheaacd_mult32X32float((FLOAT32)x1r, 0.923880f) -
                      ixheaacd_mult32X32float((FLOAT32)x1i, -0.382683f));
      x1i = (FLOAT32)ixheaacd_mac32X32float(ixheaacd_mult32X32float((FLOAT32)x1r, -0.382683f),
                                   (FLOAT32)x1i, 0.923880f);
      x1r = tmp;

      tmp = (FLOAT32)(ixheaacd_mult32X32float((FLOAT32)x2r, 0.707107f) -
                      ixheaacd_mult32X32float((FLOAT32)x2i, -0.707107f));
      x2i = (FLOAT32)ixheaacd_mac32X32float(ixheaacd_mult32X32float((FLOAT32)x2r, -0.707107f),
                                   (FLOAT32)x2i, 0.707107f);
      x2r = tmp;

      tmp = (FLOAT32)(ixheaacd_mult32X32float((FLOAT32)x3r, 0.382683f) -
                      ixheaacd_mult32X32float((FLOAT32)x3i, -0.923880f));
      x3i = (FLOAT32)ixheaacd_mac32X32float(ixheaacd_mult32X32float((FLOAT32)x3r, -0.923880f),
                                   (FLOAT32)x3i, 0.382683f);
      x3r = tmp;

      x0r = (*data_i);
      x0i = (*(data_i + 1));

      x0r = x0r + (x2r);
      x0i = x0i + (x2i);
      x2r = x0r - (x2r * 2);
      x2i = x0i - (x2i * 2);
      x1r = x1r + x3r;
      x1i = x1i + x3i;
      x3r = x1r - (x3r * 2);
      x3i = x1i - (x3i * 2);

      x0r = x0r + (x1r);
      x0i = x0i + (x1i);
      x1r = x0r - (x1r * 2);
      x1i = x0i - (x1i * 2);
      x2r = x2r + (x3i);
      x2i = x2i - (x3r);
      x3i = x2r - (x3i * 2);
      x3r = x2i + (x3r * 2);

      *data_i = x0r;
      *(data_i + 1) = x0i;
      data_i += 8;

      *data_i = x2r;
      *(data_i + 1) = x2i;
      data_i += 8;

      *data_i = x1r;
      *(data_i + 1) = x1i;
      data_i += 8;

      *data_i = x3i;
      *(data_i + 1) = x3r;
      data_i += 8;
    }
    data_r -= 62;
    data_i -= 62;
    for (k = 2; k != 0; k--) {
      FLOAT32 tmp;
      FLOAT32 x0r, x0i, x1r, x1i, x2r, x2i, x3r, x3i;

      data_r += 8;

      x1r = *data_r;
      x1i = *(data_r + 1);
      data_r += 8;

      x2r = *data_r;
      x2i = *(data_r + 1);
      data_r += 8;

      x3r = *data_r;
      x3i = *(data_r + 1);
      data_r -= 24;

      tmp = (FLOAT32)(ixheaacd_mult32X32float((FLOAT32)x1r, 0.707107f) -
                      ixheaacd_mult32X32float((FLOAT32)x1i, -0.707107f));
      x1i = (FLOAT32)ixheaacd_mac32X32float(ixheaacd_mult32X32float((FLOAT32)x1r, -0.707107f),
                                   (FLOAT32)x1i, 0.707107f);
      x1r = tmp;

      tmp = x2i;
      x2i = -x2r;
      x2r = tmp;

      tmp = (FLOAT32)(ixheaacd_mult32X32float((FLOAT32)x3r, -0.707107f) +
                      ixheaacd_mult32X32float((FLOAT32)x3i, 0.707107f));
      x3i = (FLOAT32)(-ixheaacd_mult32X32float((FLOAT32)x3r, 0.707107f) +
                      ixheaacd_mult32X32float((FLOAT32)x3i, -0.707107f));
      x3r = tmp;

      x0r = (*data_r);
      x0i = (*(data_r + 1));

      x0r = x0r + (x2r);
      x0i = x0i + (x2i);
      x2r = x0r - (x2r * 2);
      x2i = x0i - (x2i * 2);
      x1r = x1r + x3r;
      x1i = x1i + x3i;
      x3r = x1r - (x3r * 2);
      x3i = x1i - (x3i * 2);

      x0r = x0r + (x1r);
      x0i = x0i + (x1i);
      x1r = x0r - (x1r * 2);
      x1i = x0i - (x1i * 2);
      x2r = x2r + (x3i);
      x2i = x2i - (x3r);
      x3i = x2r - (x3i * 2);
      x3r = x2i + (x3r * 2);

      *data_r = x0r;
      *(data_r + 1) = x0i;
      data_r += 8;

      *data_r = x2r;
      *(data_r + 1) = x2i;
      data_r += 8;

      *data_r = x1r;
      *(data_r + 1) = x1i;
      data_r += 8;

      *data_r = x3i;
      *(data_r + 1) = x3r;
      data_r += 8;
      data_i += 8;

      x1r = *data_i;
      x1i = *(data_i + 1);
      data_i += 8;

      x2r = *data_i;
      x2i = *(data_i + 1);
      data_i += 8;

      x3r = *data_i;
      x3i = *(data_i + 1);
      data_i -= 24;

      tmp = (FLOAT32)(ixheaacd_mult32X32float((FLOAT32)x1r, 0.707107f) -
                      ixheaacd_mult32X32float((FLOAT32)x1i, -0.707107f));
      x1i = (FLOAT32)ixheaacd_mac32X32float(ixheaacd_mult32X32float((FLOAT32)x1r, -0.707107f),
                                   (FLOAT32)x1i, 0.707107f);
      x1r = tmp;

      tmp = x2i;
      x2i = -x2r;
      x2r = tmp;

      tmp = (FLOAT32)(ixheaacd_mult32X32float((FLOAT32)x3r, -0.707107f) +
                      ixheaacd_mult32X32float((FLOAT32)x3i, 0.707107f));
      x3i = (FLOAT32)(-ixheaacd_mult32X32float((FLOAT32)x3r, 0.707107f) +
                      ixheaacd_mult32X32float((FLOAT32)x3i, -0.707107f));
      x3r = tmp;

      x0r = (*data_i);
      x0i = (*(data_i + 1));

      x0r = x0r + (x2r);
      x0i = x0i + (x2i);
      x2r = x0r - (x2r * 2);
      x2i = x0i - (x2i * 2);
      x1r = x1r + x3r;
      x1i = x1i + x3i;
      x3r = x1r - (x3r * 2);
      x3i = x1i - (x3i * 2);

      x0r = x0r + (x1r);
      x0i = x0i + (x1i);
      x1r = x0r - (x1r * 2);
      x1i = x0i - (x1i * 2);
      x2r = x2r + (x3i);
      x2i = x2i - (x3r);
      x3i = x2r - (x3i * 2);
      x3r = x2i + (x3r * 2);

      *data_i = x0r;
      *(data_i + 1) = x0i;
      data_i += 8;

      *data_i = x2r;
      *(data_i + 1) = x2i;
      data_i += 8;

      *data_i = x1r;
      *(data_i + 1) = x1i;
      data_i += 8;

      *data_i = x3i;
      *(data_i + 1) = x3r;
      data_i += 8;
    }
    data_r -= 62;
    data_i -= 62;
    for (k = 2; k != 0; k--) {
      FLOAT32 tmp;
      FLOAT32 x0r, x0i, x1r, x1i, x2r, x2i, x3r, x3i;

      data_r += 8;

      x1r = *data_r;
      x1i = *(data_r + 1);
      data_r += 8;

      x2r = *data_r;
      x2i = *(data_r + 1);
      data_r += 8;

      x3r = *data_r;
      x3i = *(data_r + 1);
      data_r -= 24;

      tmp = (FLOAT32)(ixheaacd_mult32X32float((FLOAT32)x1r, 0.382683f) -
                      ixheaacd_mult32X32float((FLOAT32)x1i, -0.923880f));
      x1i = (FLOAT32)ixheaacd_mac32X32float(ixheaacd_mult32X32float((FLOAT32)x1r, -0.923880f),
                                   (FLOAT32)x1i, 0.382683f);
      x1r = tmp;

      tmp = (FLOAT32)(ixheaacd_mult32X32float((FLOAT32)x2r, -0.707107f) +
                      ixheaacd_mult32X32float((FLOAT32)x2i, 0.707107f));
      x2i = (FLOAT32)(-ixheaacd_mult32X32float((FLOAT32)x2r, 0.707107f) +
                      ixheaacd_mult32X32float((FLOAT32)x2i, -0.707107f));
      x2r = tmp;

      tmp = (FLOAT32)(-ixheaacd_mult32X32float((FLOAT32)x3r, 0.923880f) +
                      ixheaacd_mult32X32float((FLOAT32)x3i, -0.382683f));
      x3i = (FLOAT32)ixheaacd_mac32X32float(ixheaacd_mult32X32float((FLOAT32)x3r, -0.382683f),
                                   (FLOAT32)x3i, 0.923880f);
      x3r = tmp;

      x0r = (*data_r);
      x0i = (*(data_r + 1));

      x0r = x0r + (x2r);
      x0i = x0i + (x2i);
      x2r = x0r - (x2r * 2);
      x2i = x0i - (x2i * 2);
      x1r = x1r + x3r;
      x1i = x1i - x3i;
      x3r = x1r - (x3r * 2);
      x3i = x1i + (x3i * 2);

      x0r = x0r + (x1r);
      x0i = x0i + (x1i);
      x1r = x0r - (x1r * 2);
      x1i = x0i - (x1i * 2);
      x2r = x2r + (x3i);
      x2i = x2i - (x3r);
      x3i = x2r - (x3i * 2);
      x3r = x2i + (x3r * 2);

      *data_r = x0r;
      *(data_r + 1) = x0i;
      data_r += 8;

      *data_r = x2r;
      *(data_r + 1) = x2i;
      data_r += 8;

      *data_r = x1r;
      *(data_r + 1) = x1i;
      data_r += 8;

      *data_r = x3i;
      *(data_r + 1) = x3r;
      data_r += 8;
      data_i += 8;

      x1r = *data_i;
      x1i = *(data_i + 1);
      data_i += 8;

      x2r = *data_i;
      x2i = *(data_i + 1);
      data_i += 8;

      x3r = *data_i;
      x3i = *(data_i + 1);
      data_i -= 24;

      tmp = (FLOAT32)(ixheaacd_mult32X32float((FLOAT32)x1r, 0.382683f) -
                      ixheaacd_mult32X32float((FLOAT32)x1i, -0.923880f));
      x1i = (FLOAT32)ixheaacd_mac32X32float(ixheaacd_mult32X32float((FLOAT32)x1r, -0.923880f),
                                   (FLOAT32)x1i, 0.382683f);
      x1r = tmp;

      tmp = (FLOAT32)(ixheaacd_mult32X32float((FLOAT32)x2r, -0.707107f) +
                      ixheaacd_mult32X32float((FLOAT32)x2i, 0.707107f));
      x2i = (FLOAT32)(-ixheaacd_mult32X32float((FLOAT32)x2r, 0.707107f) +
                      ixheaacd_mult32X32float((FLOAT32)x2i, -0.707107f));
      x2r = tmp;

      tmp = (FLOAT32)(-ixheaacd_mult32X32float((FLOAT32)x3r, 0.923880f) +
                      ixheaacd_mult32X32float((FLOAT32)x3i, -0.382683f));
      x3i = (FLOAT32)ixheaacd_mac32X32float(ixheaacd_mult32X32float((FLOAT32)x3r, -0.382683f),
                                   (FLOAT32)x3i, 0.923880f);
      x3r = tmp;

      x0r = (*data_i);
      x0i = (*(data_i + 1));

      x0r = x0r + (x2r);
      x0i = x0i + (x2i);
      x2r = x0r - (x2r * 2);
      x2i = x0i - (x2i * 2);
      x1r = x1r + x3r;
      x1i = x1i - x3i;
      x3r = x1r - (x3r * 2);
      x3i = x1i + (x3i * 2);

      x0r = x0r + (x1r);
      x0i = x0i + (x1i);
      x1r = x0r - (x1r * 2);
      x1i = x0i - (x1i * 2);
      x2r = x2r + (x3i);
      x2i = x2i - (x3r);
      x3i = x2r - (x3i * 2);
      x3r = x2i + (x3r * 2);

      *data_i = x0r;
      *(data_i + 1) = x0i;
      data_i += 8;

      *data_i = x2r;
      *(data_i + 1) = x2i;
      data_i += 8;

      *data_i = x1r;
      *(data_i + 1) = x1i;
      data_i += 8;

      *data_i = x3i;
      *(data_i + 1) = x3r;
      data_i += 8;
    }
    data_r -= 62;
    data_i -= 62;
  }
  {
    const FLOAT32 *twiddles = ptr_w;
    FLOAT32 x0r, x0i, x1r, x1i;
    for (j = 8; j != 0; j--) {
      FLOAT32 W1 = *twiddles;
      twiddles++;
      FLOAT32 W4 = *twiddles;
      twiddles++;
      FLOAT32 tmp;

      x0r = *ptr_y;
      x0i = *(ptr_y + 1);
      ptr_y += 32;
      ptr_xr += 32;

      x1r = *ptr_y;
      x1i = *(ptr_y + 1);

      tmp = (FLOAT32)(ixheaacd_mult32X32float((FLOAT32)x1r, W1) -
                      ixheaacd_mult32X32float((FLOAT32)x1i, W4));
      x1i = (FLOAT32)ixheaacd_mac32X32float(ixheaacd_mult32X32float((FLOAT32)x1r, W4),
                                   (FLOAT32)x1i, W1);
      x1r = tmp;

      *ptr_xr = (x0r) - (x1r);
      *(ptr_xr + 1) = (x0i) - (x1i);
      ptr_y -= 32;
      ptr_xr -= 32;

      *ptr_xr = (x0r) + (x1r);
      *(ptr_xr + 1) = (x0i) + (x1i);
      ptr_y += 2;
      ptr_xr += 2;

      x0r = *ptr_z;
      x0i = *(ptr_z + 1);
      ptr_z += 32;
      ptr_xi += 32;

      x1r = *ptr_z;
      x1i = *(ptr_z + 1);

      tmp = (FLOAT32)(ixheaacd_mult32X32float((FLOAT32)x1r, W1) -
                      ixheaacd_mult32X32float((FLOAT32)x1i, W4));
      x1i = (FLOAT32)ixheaacd_mac32X32float(ixheaacd_mult32X32float((FLOAT32)x1r, W4),
                                   (FLOAT32)x1i, W1);
      x1r = tmp;

      *ptr_xi = (x0r) - (x1r);
      *(ptr_xi + 1) = (x0i) - (x1i);
      ptr_z -= 32;
      ptr_xi -= 32;

      *ptr_xi = (x0r) + (x1r);
      *(ptr_xi + 1) = (x0i) + (x1i);
      ptr_z += 2;
      ptr_xi += 2;
    }
    twiddles = ptr_w;
    for (j = 8; j != 0; j--) {
      FLOAT32 W1 = *twiddles;
      twiddles++;
      FLOAT32 W4 = *twiddles;
      twiddles++;
      FLOAT32 tmp;

      x0r = *ptr_y;
      x0i = *(ptr_y + 1);
      ptr_y += 32;
      ptr_xr += 32;

      x1r = *ptr_y;
      x1i = *(ptr_y + 1);

      tmp = (FLOAT32)(ixheaacd_mult32X32float((FLOAT32)x1r, W4) +
                      ixheaacd_mult32X32float((FLOAT32)x1i, W1));
      x1i = (FLOAT32)(-ixheaacd_mult32X32float((FLOAT32)x1r, W1) +
                      ixheaacd_mult32X32float((FLOAT32)x1i, W4));
      x1r = tmp;

      *ptr_xr = (x0r) - (x1r);
      *(ptr_xr + 1) = (x0i) - (x1i);
      ptr_y -= 32;
      ptr_xr -= 32;

      *ptr_xr = (x0r) + (x1r);
      *(ptr_xr + 1) = (x0i) + (x1i);
      ptr_y += 2;
      ptr_xr += 2;

      x0r = *ptr_z;
      x0i = *(ptr_z + 1);
      ptr_z += 32;
      ptr_xi += 32;

      x1r = *ptr_z;
      x1i = *(ptr_z + 1);

      tmp = (FLOAT32)(ixheaacd_mult32X32float((FLOAT32)x1r, W4) +
                      ixheaacd_mult32X32float((FLOAT32)x1i, W1));
      x1i = (FLOAT32)(-ixheaacd_mult32X32float((FLOAT32)x1r, W1) +
                      ixheaacd_mult32X32float((FLOAT32)x1i, W4));
      x1r = tmp;

      *ptr_xi = (x0r) - (x1r);
      *(ptr_xi + 1) = (x0i) - (x1i);
      ptr_z -= 32;
      ptr_xi -= 32;

      *ptr_xi = (x0r) + (x1r);
      *(ptr_xi + 1) = (x0i) + (x1i);
      ptr_z += 2;
      ptr_xi += 2;
    }
  }
}

VOID ixheaacd_mps_complex_fft(FLOAT32 *xr, FLOAT32 *xi, WORD32 nlength) {
  WORD32 i, j, k, n_stages, h2;
  FLOAT32 x0r, x0i, x1r, x1i, x2r, x2i, x3r, x3i;
  WORD32 del, nodespacing, in_loop_cnt;
  WORD32 dig_rev_shift;
  WORD32 not_power_4;
  FLOAT32 ptr_x[256];
  FLOAT32 y[256];
  WORD32 npoints = nlength;
  FLOAT32 *ptr_y = y;
  const FLOAT32 *ptr_w;
  dig_rev_shift = ixheaac_norm32(npoints) + 1 - 16;
  n_stages = 30 - ixheaac_norm32(npoints);
  not_power_4 = n_stages & 1;

  n_stages = n_stages >> 1;


  for (i = 0; i<nlength; i++)
  {
    ptr_x[2 * i] = xr[i];
    ptr_x[2 * i + 1] = xi[i];
  }

  ptr_w = ixheaacd_twiddle_table_fft;

  for (i = 0; i<npoints; i += 4)
  {
    FLOAT32 *inp = ptr_x;

    DIG_REV(i, dig_rev_shift, h2);
    if (not_power_4)
    {
      h2 += 1;
      h2 &= ~1;
    }
    inp += (h2);

    x0r = *inp;
    x0i = *(inp + 1);
    inp += (npoints >> 1);

    x1r = *inp;
    x1i = *(inp + 1);
    inp += (npoints >> 1);

    x2r = *inp;
    x2i = *(inp + 1);
    inp += (npoints >> 1);

    x3r = *inp;
    x3i = *(inp + 1);

    x0r = x0r + x2r;
    x0i = x0i + x2i;
    x2r = x0r - (x2r * 2);
    x2i = x0i - (x2i * 2);
    x1r = x1r + x3r;
    x1i = x1i + x3i;
    x3r = x1r - (x3r * 2);
    x3i = x1i - (x3i * 2);

    x0r = x0r + x1r;
    x0i = x0i + x1i;
    x1r = x0r - (x1r * 2);
    x1i = x0i - (x1i * 2);
    x2r = x2r + x3i;
    x2i = x2i - x3r;
    x3i = x2r - (x3i * 2);
    x3r = x2i + (x3r * 2);

    *ptr_y++ = x0r;
    *ptr_y++ = x0i;
    *ptr_y++ = x2r;
    *ptr_y++ = x2i;
    *ptr_y++ = x1r;
    *ptr_y++ = x1i;
    *ptr_y++ = x3i;
    *ptr_y++ = x3r;
  }
  ptr_y -= 2 * npoints;
  del = 4;
  nodespacing = 64;
  in_loop_cnt = npoints >> 4;
  for (i = n_stages - 1; i>0; i--)
  {
    const FLOAT32 *twiddles = ptr_w;
    FLOAT32 *data = ptr_y;
    FLOAT32 w1h, w2h, w3h, w1l, w2l, w3l;
    WORD32 sec_loop_cnt;

    for (k = in_loop_cnt; k != 0; k--)
    {
      x0r = (*data);
      x0i = (*(data + 1));
      data += (del << 1);

      x1r = (*data);
      x1i = (*(data + 1));
      data += (del << 1);

      x2r = (*data);
      x2i = (*(data + 1));
      data += (del << 1);

      x3r = (*data);
      x3i = (*(data + 1));
      data -= 3 * (del << 1);

      x0r = x0r + x2r;
      x0i = x0i + x2i;
      x2r = x0r - (x2r * 2);
      x2i = x0i - (x2i * 2);
      x1r = x1r + x3r;
      x1i = x1i + x3i;
      x3r = x1r - (x3r * 2);
      x3i = x1i - (x3i * 2);

      x0r = x0r + x1r;
      x0i = x0i + x1i;
      x1r = x0r - (x1r * 2);
      x1i = x0i - (x1i * 2);
      x2r = x2r + x3i;
      x2i = x2i - x3r;
      x3i = x2r - (x3i * 2);
      x3r = x2i + (x3r * 2);

      *data = x0r;
      *(data + 1) = x0i;
      data += (del << 1);

      *data = x2r;
      *(data + 1) = x2i;
      data += (del << 1);

      *data = x1r;
      *(data + 1) = x1i;
      data += (del << 1);

      *data = x3i;
      *(data + 1) = x3r;
      data += (del << 1);
    }
    data = ptr_y + 2;

    sec_loop_cnt = (nodespacing * del);
    sec_loop_cnt = (sec_loop_cnt / 4) + (sec_loop_cnt / 8) - (sec_loop_cnt / 16) \
            + (sec_loop_cnt / 32) - (sec_loop_cnt / 64) + (sec_loop_cnt / 128) \
            - (sec_loop_cnt / 256);
    j = nodespacing;

    for (j = nodespacing; j <= sec_loop_cnt; j += nodespacing)
    {
      w1h = *(twiddles + 2 * j);
      w1l = *(twiddles + 2 * j + 1);
      w2h = *(twiddles + 2 * (j << 1));
      w2l = *(twiddles + 2 * (j << 1) + 1);
      w3h = *(twiddles + 2 * j + 2 * (j << 1));
      w3l = *(twiddles + 2 * j + 2 * (j << 1) + 1);

      for (k = in_loop_cnt; k != 0; k--)
      {
        FLOAT32 tmp;
        FLOAT32 x0r, x0i, x1r, x1i, x2r, x2i, x3r, x3i;

        data += (del << 1);

        x1r = *data;
        x1i = *(data + 1);
        data += (del << 1);

        x2r = *data;
        x2i = *(data + 1);
        data += (del << 1);

        x3r = *data;
        x3i = *(data + 1);
        data -= 3 * (del << 1);

        tmp = (ixheaacd_mult32X32float(x1r, w1l) - ixheaacd_mult32X32float(x1i, w1h));
        x1i = ixheaacd_mac32X32float(ixheaacd_mult32X32float(x1r, w1h), x1i, w1l);
        x1r = tmp;

        tmp = (ixheaacd_mult32X32float(x2r, w2l) - ixheaacd_mult32X32float(x2i, w2h));
        x2i = ixheaacd_mac32X32float(ixheaacd_mult32X32float(x2r, w2h), x2i, w2l);
        x2r = tmp;

        tmp = (ixheaacd_mult32X32float(x3r, w3l) - ixheaacd_mult32X32float(x3i, w3h));
        x3i = ixheaacd_mac32X32float(ixheaacd_mult32X32float(x3r, w3h), x3i, w3l);
        x3r = tmp;

        x0r = (*data);
        x0i = (*(data + 1));

        x0r = x0r + (x2r);
        x0i = x0i + (x2i);
        x2r = x0r - (x2r * 2);
        x2i = x0i - (x2i * 2);
        x1r = x1r + x3r;
        x1i = x1i + x3i;
        x3r = x1r - (x3r * 2);
        x3i = x1i - (x3i * 2);

        x0r = x0r + (x1r);
        x0i = x0i + (x1i);
        x1r = x0r - (x1r * 2);
        x1i = x0i - (x1i * 2);
        x2r = x2r + (x3i);
        x2i = x2i - (x3r);
        x3i = x2r - (x3i * 2);
        x3r = x2i + (x3r * 2);

        *data = x0r;
        *(data + 1) = x0i;
        data += (del << 1);

        *data = x2r;
        *(data + 1) = x2i;
        data += (del << 1);

        *data = x1r;
        *(data + 1) = x1i;
        data += (del << 1);

        *data = x3i;
        *(data + 1) = x3r;
        data += (del << 1);
      }
      data -= 2 * npoints;
      data += 2;
    }
    for (; j <= (nodespacing * del) >> 1; j += nodespacing)
    {
      w1h = *(twiddles + 2 * j);
      w2h = *(twiddles + 2 * (j << 1));
      w3h = *(twiddles + 2 * j + 2 * (j << 1) - 512);
      w1l = *(twiddles + 2 * j + 1);
      w2l = *(twiddles + 2 * (j << 1) + 1);
      w3l = *(twiddles + 2 * j + 2 * (j << 1) - 511);

      for (k = in_loop_cnt; k != 0; k--)
      {
        FLOAT32 tmp;
        FLOAT32 x0r, x0i, x1r, x1i, x2r, x2i, x3r, x3i;

        data += (del << 1);

        x1r = *data;
        x1i = *(data + 1);
        data += (del << 1);

        x2r = *data;
        x2i = *(data + 1);
        data += (del << 1);

        x3r = *data;
        x3i = *(data + 1);
        data -= 3 * (del << 1);

        tmp = (ixheaacd_mult32X32float(x1r, w1l) - ixheaacd_mult32X32float(x1i, w1h));
        x1i = ixheaacd_mac32X32float(ixheaacd_mult32X32float(x1r, w1h), x1i, w1l);
        x1r = tmp;

        tmp = (ixheaacd_mult32X32float(x2r, w2l) - ixheaacd_mult32X32float(x2i, w2h));
        x2i = ixheaacd_mac32X32float(ixheaacd_mult32X32float(x2r, w2h), x2i, w2l);
        x2r = tmp;

        tmp = (ixheaacd_mult32X32float(x3r, w3h) + ixheaacd_mult32X32float(x3i, w3l));
        x3i = -ixheaacd_mult32X32float(x3r, w3l) + ixheaacd_mult32X32float(x3i, w3h);
        x3r = tmp;

        x0r = (*data);
        x0i = (*(data + 1));

        x0r = x0r + (x2r);
        x0i = x0i + (x2i);
        x2r = x0r - (x2r * 2);
        x2i = x0i - (x2i * 2);
        x1r = x1r + x3r;
        x1i = x1i + x3i;
        x3r = x1r - (x3r * 2);
        x3i = x1i - (x3i * 2);

        x0r = x0r + (x1r);
        x0i = x0i + (x1i);
        x1r = x0r - (x1r * 2);
        x1i = x0i - (x1i * 2);
        x2r = x2r + (x3i);
        x2i = x2i - (x3r);
        x3i = x2r - (x3i * 2);
        x3r = x2i + (x3r * 2);

        *data = x0r;
        *(data + 1) = x0i;
        data += (del << 1);

        *data = x2r;
        *(data + 1) = x2i;
        data += (del << 1);

        *data = x1r;
        *(data + 1) = x1i;
        data += (del << 1);

        *data = x3i;
        *(data + 1) = x3r;
        data += (del << 1);
      }
      data -= 2 * npoints;
      data += 2;
    }
    for (; j <= sec_loop_cnt * 2; j += nodespacing)
    {
      w1h = *(twiddles + 2 * j);
      w2h = *(twiddles + 2 * (j << 1) - 512);
      w3h = *(twiddles + 2 * j + 2 * (j << 1) - 512);
      w1l = *(twiddles + 2 * j + 1);
      w2l = *(twiddles + 2 * (j << 1) - 511);
      w3l = *(twiddles + 2 * j + 2 * (j << 1) - 511);

      for (k = in_loop_cnt; k != 0; k--)
      {
        FLOAT32 tmp;
        FLOAT32 x0r, x0i, x1r, x1i, x2r, x2i, x3r, x3i;

        data += (del << 1);

        x1r = *data;
        x1i = *(data + 1);
        data += (del << 1);

        x2r = *data;
        x2i = *(data + 1);
        data += (del << 1);

        x3r = *data;
        x3i = *(data + 1);
        data -= 3 * (del << 1);

        tmp = (ixheaacd_mult32X32float(x1r, w1l) - ixheaacd_mult32X32float(x1i, w1h));
        x1i = ixheaacd_mac32X32float(ixheaacd_mult32X32float(x1r, w1h), x1i, w1l);
        x1r = tmp;

        tmp = (ixheaacd_mult32X32float(x2r, w2h) + ixheaacd_mult32X32float(x2i, w2l));
        x2i = -ixheaacd_mult32X32float(x2r, w2l) + ixheaacd_mult32X32float(x2i, w2h);
        x2r = tmp;

        tmp = (ixheaacd_mult32X32float(x3r, w3h) + ixheaacd_mult32X32float(x3i, w3l));
        x3i = -ixheaacd_mult32X32float(x3r, w3l) + ixheaacd_mult32X32float(x3i, w3h);
        x3r = tmp;

        x0r = (*data);
        x0i = (*(data + 1));

        x0r = x0r + (x2r);
        x0i = x0i + (x2i);
        x2r = x0r - (x2r * 2);
        x2i = x0i - (x2i * 2);
        x1r = x1r + x3r;
        x1i = x1i + x3i;
        x3r = x1r - (x3r * 2);
        x3i = x1i - (x3i * 2);

        x0r = x0r + (x1r);
        x0i = x0i + (x1i);
        x1r = x0r - (x1r * 2);
        x1i = x0i - (x1i * 2);
        x2r = x2r + (x3i);
        x2i = x2i - (x3r);
        x3i = x2r - (x3i * 2);
        x3r = x2i + (x3r * 2);

        *data = x0r;
        *(data + 1) = x0i;
        data += (del << 1);

        *data = x2r;
        *(data + 1) = x2i;
        data += (del << 1);

        *data = x1r;
        *(data + 1) = x1i;
        data += (del << 1);

        *data = x3i;
        *(data + 1) = x3r;
        data += (del << 1);
      }
      data -= 2 * npoints;
      data += 2;
    }
    for (; j<nodespacing * del; j += nodespacing)
    {
      w1h = *(twiddles + 2 * j);
      w2h = *(twiddles + 2 * (j << 1) - 512);
      w3h = *(twiddles + 2 * j + 2 * (j << 1) - 1024);
      w1l = *(twiddles + 2 * j + 1);
      w2l = *(twiddles + 2 * (j << 1) - 511);
      w3l = *(twiddles + 2 * j + 2 * (j << 1) - 1023);

      for (k = in_loop_cnt; k != 0; k--)
      {
        FLOAT32 tmp;
        FLOAT32 x0r, x0i, x1r, x1i, x2r, x2i, x3r, x3i;

        data += (del << 1);

        x1r = *data;
        x1i = *(data + 1);
        data += (del << 1);

        x2r = *data;
        x2i = *(data + 1);
        data += (del << 1);

        x3r = *data;
        x3i = *(data + 1);
        data -= 3 * (del << 1);

        tmp = (ixheaacd_mult32X32float(x1r, w1l) - ixheaacd_mult32X32float(x1i, w1h));
        x1i = ixheaacd_mac32X32float(ixheaacd_mult32X32float(x1r, w1h), x1i, w1l);
        x1r = tmp;

        tmp = (ixheaacd_mult32X32float(x2r, w2h) + ixheaacd_mult32X32float(x2i, w2l));
        x2i = -ixheaacd_mult32X32float(x2r, w2l) + ixheaacd_mult32X32float(x2i, w2h);
        x2r = tmp;

        tmp = (-ixheaacd_mult32X32float(x3r, w3l) + ixheaacd_mult32X32float(x3i, w3h));
        x3i = ixheaacd_mac32X32float(ixheaacd_mult32X32float(x3r, w3h), x3i, w3l);
        x3r = tmp;

        x0r = (*data);
        x0i = (*(data + 1));

        x0r = x0r + (x2r);
        x0i = x0i + (x2i);
        x2r = x0r - (x2r * 2);
        x2i = x0i - (x2i * 2);
        x1r = x1r + x3r;
        x1i = x1i - x3i;
        x3r = x1r - (x3r * 2);
        x3i = x1i + (x3i * 2);

        x0r = x0r + (x1r);
        x0i = x0i + (x1i);
        x1r = x0r - (x1r * 2);
        x1i = x0i - (x1i * 2);
        x2r = x2r + (x3i);
        x2i = x2i - (x3r);
        x3i = x2r - (x3i * 2);
        x3r = x2i + (x3r * 2);

        *data = x0r;
        *(data + 1) = x0i;
        data += (del << 1);

        *data = x2r;
        *(data + 1) = x2i;
        data += (del << 1);

        *data = x1r;
        *(data + 1) = x1i;
        data += (del << 1);

        *data = x3i;
        *(data + 1) = x3r;
        data += (del << 1);
      }
      data -= 2 * npoints;
      data += 2;
    }
    nodespacing >>= 2;
    del <<= 2;
    in_loop_cnt >>= 2;
  }
  if (not_power_4)
  {
    const FLOAT32 *twiddles = ptr_w;
    nodespacing <<= 1;

    for (j = del / 2; j != 0; j--)
    {
      FLOAT32 w1h = *twiddles;
      FLOAT32 w1l = *(twiddles + 1);
      FLOAT32 tmp;
      twiddles += nodespacing * 2;

      x0r = *ptr_y;
      x0i = *(ptr_y + 1);
      ptr_y += (del << 1);

      x1r = *ptr_y;
      x1i = *(ptr_y + 1);

      tmp = (ixheaacd_mult32X32float(x1r, w1l) - ixheaacd_mult32X32float(x1i, w1h));
      x1i = ixheaacd_mac32X32float(ixheaacd_mult32X32float(x1r, w1h), x1i, w1l);
      x1r = tmp;

      *ptr_y = (x0r) - (x1r);
      *(ptr_y + 1) = (x0i) - (x1i);
      ptr_y -= (del << 1);

      *ptr_y = (x0r) + (x1r);
      *(ptr_y + 1) = (x0i) + (x1i);
      ptr_y += 2;
    }
    twiddles = ptr_w;
    for (j = del / 2; j != 0; j--)
    {
      FLOAT32 w1h = *twiddles;
      FLOAT32 w1l = *(twiddles + 1);
      FLOAT32 tmp;
      twiddles += nodespacing * 2;

      x0r = *ptr_y;
      x0i = *(ptr_y + 1);
      ptr_y += (del << 1);

      x1r = *ptr_y;
      x1i = *(ptr_y + 1);

      tmp = (ixheaacd_mult32X32float(x1r, w1h) + ixheaacd_mult32X32float(x1i, w1l));
      x1i = -ixheaacd_mult32X32float(x1r, w1l) + ixheaacd_mult32X32float(x1i, w1h);
      x1r = tmp;

      *ptr_y = (x0r) - (x1r);
      *(ptr_y + 1) = (x0i) - (x1i);
      ptr_y -= (del << 1);

      *ptr_y = (x0r) + (x1r);
      *(ptr_y + 1) = (x0i) + (x1i);
      ptr_y += 2;
    }
  }

  for (i = 0; i<nlength; i++)
  {
    xr[i] = y[2 * i];
    xi[i] = y[2 * i + 1];
  }

  return;
}

VOID ixheaacd_complex_fft_p2_dec(WORD32 *xr, WORD32 *xi, WORD32 nlength,
                                 WORD32 fft_mode, WORD32 *preshift) {
  WORD32 i, j, k, n_stages;
  WORD32 h2, x0r, x0i, x1r, x1i, x2r, x2i, x3r, x3i;
  WORD32 del, nodespacing, in_loop_cnt;
  WORD32 not_power_4;
  WORD32 npts, shift;
  WORD32 dig_rev_shift;
  WORD32 ptr_x[1024];
  WORD32 y[1024];
  WORD32 npoints = nlength;
  WORD32 n = 0;
  WORD32 *ptr_y = y;
  const WORD32 *ptr_w;
  dig_rev_shift = ixheaac_norm32(npoints) + 1 - 16;
  n_stages = 30 - ixheaac_norm32(npoints);
  not_power_4 = n_stages & 1;

  n_stages = n_stages >> 1;

  npts = npoints;
  while (npts >> 1) {
    n++;
    npts = npts >> 1;
  }

  if (n % 2 == 0)
    shift = ((n + 4)) / 2;
  else
    shift = ((n + 3) / 2);

  for (i = 0; i < nlength; i++) {
    ptr_x[2 * i] = (xr[i] / (1 << (shift)));
    ptr_x[2 * i + 1] = (xi[i] / (1 << (shift)));
  }

  if (fft_mode == -1) {
    ptr_w = ixheaacd_twiddle_table_fft_32x32;

    for (i = 0; i < npoints; i += 4) {
      WORD32 *inp = ptr_x;

      DIG_REV(i, dig_rev_shift, h2);
      if (not_power_4) {
        h2 += 1;
        h2 &= ~1;
      }
      inp += (h2);

      x0r = *inp;
      x0i = *(inp + 1);
      inp += (npoints >> 1);

      x1r = *inp;
      x1i = *(inp + 1);
      inp += (npoints >> 1);

      x2r = *inp;
      x2i = *(inp + 1);
      inp += (npoints >> 1);

      x3r = *inp;
      x3i = *(inp + 1);

      x0r = ixheaac_add32_sat(x0r, x2r);
      x0i = ixheaac_add32_sat(x0i, x2i);
      x2r = ixheaac_sub32_sat(x0r, ixheaac_shl32_sat(x2r, 1));
      x2i = ixheaac_sub32_sat(x0i, ixheaac_shl32_sat(x2i, 1));
      x1r = ixheaac_add32_sat(x1r, x3r);
      x1i = ixheaac_add32_sat(x1i, x3i);
      x3r = ixheaac_sub32_sat(x1r, ixheaac_shl32_sat(x3r, 1));
      x3i = ixheaac_sub32_sat(x1i, ixheaac_shl32_sat(x3i, 1));

      x0r = ixheaac_add32_sat(x0r, x1r);
      x0i = ixheaac_add32_sat(x0i, x1i);
      x1r = ixheaac_sub32_sat(x0r, ixheaac_shl32_sat(x1r, 1));
      x1i = ixheaac_sub32_sat(x0i, ixheaac_shl32_sat(x1i, 1));
      x2r = ixheaac_add32_sat(x2r, x3i);
      x2i = ixheaac_sub32_sat(x2i, x3r);
      x3i = ixheaac_sub32_sat(x2r, ixheaac_shl32_sat(x3i, 1));
      x3r = ixheaac_add32_sat(x2i, ixheaac_shl32_sat(x3r, 1));

      *ptr_y++ = x0r;
      *ptr_y++ = x0i;
      *ptr_y++ = x2r;
      *ptr_y++ = x2i;
      *ptr_y++ = x1r;
      *ptr_y++ = x1i;
      *ptr_y++ = x3i;
      *ptr_y++ = x3r;
    }
    ptr_y -= 2 * npoints;
    del = 4;
    nodespacing = 64;
    in_loop_cnt = npoints >> 4;
    for (i = n_stages - 1; i > 0; i--) {
      const WORD32 *twiddles = ptr_w;
      WORD32 *data = ptr_y;
      WORD32 w1h, w2h, w3h, w1l, w2l, w3l;
      WORD32 sec_loop_cnt;

      for (k = in_loop_cnt; k != 0; k--) {
        x0r = (*data);
        x0i = (*(data + 1));
        data += (del << 1);

        x1r = (*data);
        x1i = (*(data + 1));
        data += (del << 1);

        x2r = (*data);
        x2i = (*(data + 1));
        data += (del << 1);

        x3r = (*data);
        x3i = (*(data + 1));
        data -= 3 * (del << 1);

        x0r = ixheaac_add32_sat(x0r, x2r);
        x0i = ixheaac_add32_sat(x0i, x2i);
        x2r = ixheaac_sub32_sat(x0r, ixheaac_shl32_sat(x2r, 1));
        x2i = ixheaac_sub32_sat(x0i, ixheaac_shl32_sat(x2i, 1));
        x1r = ixheaac_add32_sat(x1r, x3r);
        x1i = ixheaac_add32_sat(x1i, x3i);
        x3r = ixheaac_sub32_sat(x1r, ixheaac_shl32_sat(x3r, 1));
        x3i = ixheaac_sub32_sat(x1i, ixheaac_shl32_sat(x3i, 1));

        x0r = ixheaac_add32_sat(x0r, x1r);
        x0i = ixheaac_add32_sat(x0i, x1i);
        x1r = ixheaac_sub32_sat(x0r, ixheaac_shl32_sat(x1r, 1));
        x1i = ixheaac_sub32_sat(x0i, ixheaac_shl32_sat(x1i, 1));
        x2r = ixheaac_add32_sat(x2r, x3i);
        x2i = ixheaac_sub32_sat(x2i, x3r);
        x3i = ixheaac_sub32_sat(x2r, ixheaac_shl32_sat(x3i, 1));
        x3r = ixheaac_add32_sat(x2i, ixheaac_shl32_sat(x3r, 1));

        *data = x0r;
        *(data + 1) = x0i;
        data += (del << 1);

        *data = x2r;
        *(data + 1) = x2i;
        data += (del << 1);

        *data = x1r;
        *(data + 1) = x1i;
        data += (del << 1);

        *data = x3i;
        *(data + 1) = x3r;
        data += (del << 1);
      }
      data = ptr_y + 2;

      sec_loop_cnt = (nodespacing * del);
      sec_loop_cnt = (sec_loop_cnt / 4) + (sec_loop_cnt / 8) -
                     (sec_loop_cnt / 16) + (sec_loop_cnt / 32) -
                     (sec_loop_cnt / 64) + (sec_loop_cnt / 128) -
                     (sec_loop_cnt / 256);
      j = nodespacing;

      for (j = nodespacing; j <= sec_loop_cnt; j += nodespacing) {
        w1h = *(twiddles + 2 * j);
        w1l = *(twiddles + 2 * j + 1);
        w2h = *(twiddles + 2 * (j << 1));
        w2l = *(twiddles + 2 * (j << 1) + 1);
        w3h = *(twiddles + 2 * j + 2 * (j << 1));
        w3l = *(twiddles + 2 * j + 2 * (j << 1) + 1);

        for (k = in_loop_cnt; k != 0; k--) {
          WORD32 tmp;
          WORD32 x0r, x0i, x1r, x1i, x2r, x2i, x3r, x3i;

          data += (del << 1);

          x1r = *data;
          x1i = *(data + 1);
          data += (del << 1);

          x2r = *data;
          x2i = *(data + 1);
          data += (del << 1);

          x3r = *data;
          x3i = *(data + 1);
          data -= 3 * (del << 1);

          tmp = ixheaac_sub32_sat(ixheaacd_mult32_sat(x1r, w1l),
                                   ixheaacd_mult32_sat(x1i, w1h));
          x1i = ixheaacd_mac32_sat(ixheaacd_mult32_sat(x1r, w1h), x1i, w1l);
          x1r = tmp;

          tmp = ixheaac_sub32_sat(ixheaacd_mult32_sat(x2r, w2l),
                                   ixheaacd_mult32_sat(x2i, w2h));
          x2i = ixheaacd_mac32_sat(ixheaacd_mult32_sat(x2r, w2h), x2i, w2l);
          x2r = tmp;

          tmp = ixheaac_sub32_sat(ixheaacd_mult32_sat(x3r, w3l),
                                   ixheaacd_mult32_sat(x3i, w3h));
          x3i = ixheaacd_mac32_sat(ixheaacd_mult32_sat(x3r, w3h), x3i, w3l);
          x3r = tmp;

          x0r = (*data);
          x0i = (*(data + 1));

          x0r = ixheaac_add32_sat(x0r, x2r);
          x0i = ixheaac_add32_sat(x0i, x2i);
          x2r = ixheaac_sub32_sat(x0r, ixheaac_shl32_sat(x2r, 1));
          x2i = ixheaac_sub32_sat(x0i, ixheaac_shl32_sat(x2i, 1));
          x1r = ixheaac_add32_sat(x1r, x3r);
          x1i = ixheaac_add32_sat(x1i, x3i);
          x3r = ixheaac_sub32_sat(x1r, ixheaac_shl32_sat(x3r, 1));
          x3i = ixheaac_sub32_sat(x1i, ixheaac_shl32_sat(x3i, 1));

          x0r = ixheaac_add32_sat(x0r, x1r);
          x0i = ixheaac_add32_sat(x0i, x1i);
          x1r = ixheaac_sub32_sat(x0r, ixheaac_shl32_sat(x1r, 1));
          x1i = ixheaac_sub32_sat(x0i, ixheaac_shl32_sat(x1i, 1));
          x2r = ixheaac_add32_sat(x2r, x3i);
          x2i = ixheaac_sub32_sat(x2i, x3r);
          x3i = ixheaac_sub32_sat(x2r, ixheaac_shl32_sat(x3i, 1));
          x3r = ixheaac_add32_sat(x2i, ixheaac_shl32_sat(x3r, 1));

          *data = x0r;
          *(data + 1) = x0i;
          data += (del << 1);

          *data = x2r;
          *(data + 1) = x2i;
          data += (del << 1);

          *data = x1r;
          *(data + 1) = x1i;
          data += (del << 1);

          *data = x3i;
          *(data + 1) = x3r;
          data += (del << 1);
        }
        data -= 2 * npoints;
        data += 2;
      }
      for (; j <= (nodespacing * del) >> 1; j += nodespacing) {
        w1h = *(twiddles + 2 * j);
        w2h = *(twiddles + 2 * (j << 1));
        w3h = *(twiddles + 2 * j + 2 * (j << 1) - 512);
        w1l = *(twiddles + 2 * j + 1);
        w2l = *(twiddles + 2 * (j << 1) + 1);
        w3l = *(twiddles + 2 * j + 2 * (j << 1) - 511);

        for (k = in_loop_cnt; k != 0; k--) {
          WORD32 tmp;
          WORD32 x0r, x0i, x1r, x1i, x2r, x2i, x3r, x3i;
          data += (del << 1);

          x1r = *data;
          x1i = *(data + 1);
          data += (del << 1);

          x2r = *data;
          x2i = *(data + 1);
          data += (del << 1);

          x3r = *data;
          x3i = *(data + 1);
          data -= 3 * (del << 1);

          tmp = ixheaac_sub32_sat(ixheaacd_mult32_sat(x1r, w1l),
                                   ixheaacd_mult32_sat(x1i, w1h));
          x1i = ixheaacd_mac32_sat(ixheaacd_mult32_sat(x1r, w1h), x1i, w1l);
          x1r = tmp;

          tmp = ixheaac_sub32_sat(ixheaacd_mult32_sat(x2r, w2l),
                                   ixheaacd_mult32_sat(x2i, w2h));
          x2i = ixheaacd_mac32_sat(ixheaacd_mult32_sat(x2r, w2h), x2i, w2l);
          x2r = tmp;

          tmp = ixheaac_add32_sat(ixheaacd_mult32_sat(x3r, w3h),
                                   ixheaacd_mult32_sat(x3i, w3l));
          x3i = ixheaac_sub32_sat(ixheaacd_mult32_sat(x3i, w3h),
                                   ixheaacd_mult32_sat(x3r, w3l));
          x3r = tmp;

          x0r = (*data);
          x0i = (*(data + 1));

          x0r = ixheaac_add32_sat(x0r, x2r);
          x0i = ixheaac_add32_sat(x0i, x2i);
          x2r = ixheaac_sub32_sat(x0r, ixheaac_shl32_sat(x2r, 1));
          x2i = ixheaac_sub32_sat(x0i, ixheaac_shl32_sat(x2i, 1));
          x1r = ixheaac_add32_sat(x1r, x3r);
          x1i = ixheaac_add32_sat(x1i, x3i);
          x3r = ixheaac_sub32_sat(x1r, ixheaac_shl32_sat(x3r, 1));
          x3i = ixheaac_sub32_sat(x1i, ixheaac_shl32_sat(x3i, 1));

          x0r = ixheaac_add32_sat(x0r, x1r);
          x0i = ixheaac_add32_sat(x0i, x1i);
          x1r = ixheaac_sub32_sat(x0r, ixheaac_shl32_sat(x1r, 1));
          x1i = ixheaac_sub32_sat(x0i, ixheaac_shl32_sat(x1i, 1));
          x2r = ixheaac_add32_sat(x2r, x3i);
          x2i = ixheaac_sub32_sat(x2i, x3r);
          x3i = ixheaac_sub32_sat(x2r, ixheaac_shl32_sat(x3i, 1));
          x3r = ixheaac_add32_sat(x2i, ixheaac_shl32_sat(x3r, 1));

          *data = x0r;
          *(data + 1) = x0i;
          data += (del << 1);

          *data = x2r;
          *(data + 1) = x2i;
          data += (del << 1);

          *data = x1r;
          *(data + 1) = x1i;
          data += (del << 1);

          *data = x3i;
          *(data + 1) = x3r;
          data += (del << 1);
        }
        data -= 2 * npoints;
        data += 2;
      }
      for (; j <= sec_loop_cnt * 2; j += nodespacing) {
        w1h = *(twiddles + 2 * j);
        w2h = *(twiddles + 2 * (j << 1) - 512);
        w3h = *(twiddles + 2 * j + 2 * (j << 1) - 512);
        w1l = *(twiddles + 2 * j + 1);
        w2l = *(twiddles + 2 * (j << 1) - 511);
        w3l = *(twiddles + 2 * j + 2 * (j << 1) - 511);

        for (k = in_loop_cnt; k != 0; k--) {
          WORD32 tmp;
          WORD32 x0r, x0i, x1r, x1i, x2r, x2i, x3r, x3i;

          data += (del << 1);

          x1r = *data;
          x1i = *(data + 1);
          data += (del << 1);

          x2r = *data;
          x2i = *(data + 1);
          data += (del << 1);

          x3r = *data;
          x3i = *(data + 1);
          data -= 3 * (del << 1);

          tmp = ixheaac_sub32_sat(ixheaacd_mult32_sat(x1r, w1l),
                                   ixheaacd_mult32_sat(x1i, w1h));
          x1i = ixheaacd_mac32_sat(ixheaacd_mult32_sat(x1r, w1h), x1i, w1l);
          x1r = tmp;

          tmp = ixheaac_add32_sat(ixheaacd_mult32_sat(x2r, w2h),
                                   ixheaacd_mult32_sat(x2i, w2l));
          x2i = ixheaac_sub32_sat(ixheaacd_mult32_sat(x2i, w2h),
                                   ixheaacd_mult32_sat(x2r, w2l));
          x2r = tmp;

          tmp = ixheaac_add32_sat(ixheaacd_mult32_sat(x3r, w3h),
                                   ixheaacd_mult32_sat(x3i, w3l));
          x3i = ixheaac_sub32_sat(ixheaacd_mult32_sat(x3i, w3h),
                                   ixheaacd_mult32_sat(x3r, w3l));
          x3r = tmp;

          x0r = (*data);
          x0i = (*(data + 1));

          x0r = ixheaac_add32_sat(x0r, x2r);
          x0i = ixheaac_add32_sat(x0i, x2i);
          x2r = ixheaac_sub32_sat(x0r, ixheaac_shl32_sat(x2r, 1));
          x2i = ixheaac_sub32_sat(x0i, ixheaac_shl32_sat(x2i, 1));
          x1r = ixheaac_add32_sat(x1r, x3r);
          x1i = ixheaac_add32_sat(x1i, x3i);
          x3r = ixheaac_sub32_sat(x1r, ixheaac_shl32_sat(x3r, 1));
          x3i = ixheaac_sub32_sat(x1i, ixheaac_shl32_sat(x3i, 1));

          x0r = ixheaac_add32_sat(x0r, x1r);
          x0i = ixheaac_add32_sat(x0i, x1i);
          x1r = ixheaac_sub32_sat(x0r, ixheaac_shl32_sat(x1r, 1));
          x1i = ixheaac_sub32_sat(x0i, ixheaac_shl32_sat(x1i, 1));
          x2r = ixheaac_add32_sat(x2r, x3i);
          x2i = ixheaac_sub32_sat(x2i, x3r);
          x3i = ixheaac_sub32_sat(x2r, ixheaac_shl32_sat(x3i, 1));
          x3r = ixheaac_add32_sat(x2i, ixheaac_shl32_sat(x3r, 1));

          *data = x0r;
          *(data + 1) = x0i;
          data += (del << 1);

          *data = x2r;
          *(data + 1) = x2i;
          data += (del << 1);

          *data = x1r;
          *(data + 1) = x1i;
          data += (del << 1);

          *data = x3i;
          *(data + 1) = x3r;
          data += (del << 1);
        }
        data -= 2 * npoints;
        data += 2;
      }
      for (; j < nodespacing * del; j += nodespacing) {
        w1h = *(twiddles + 2 * j);
        w2h = *(twiddles + 2 * (j << 1) - 512);
        w3h = *(twiddles + 2 * j + 2 * (j << 1) - 1024);
        w1l = *(twiddles + 2 * j + 1);
        w2l = *(twiddles + 2 * (j << 1) - 511);
        w3l = *(twiddles + 2 * j + 2 * (j << 1) - 1023);

        for (k = in_loop_cnt; k != 0; k--) {
          WORD32 tmp;
          WORD32 x0r, x0i, x1r, x1i, x2r, x2i, x3r, x3i;

          data += (del << 1);

          x1r = *data;
          x1i = *(data + 1);
          data += (del << 1);

          x2r = *data;
          x2i = *(data + 1);
          data += (del << 1);

          x3r = *data;
          x3i = *(data + 1);
          data -= 3 * (del << 1);

          tmp = ixheaac_sub32_sat(ixheaacd_mult32_sat(x1r, w1l),
                                   ixheaacd_mult32_sat(x1i, w1h));
          x1i = ixheaacd_mac32_sat(ixheaacd_mult32_sat(x1r, w1h), x1i, w1l);
          x1r = tmp;

          tmp = ixheaac_add32_sat(ixheaacd_mult32_sat(x2r, w2h),
                                   ixheaacd_mult32_sat(x2i, w2l));
          x2i = ixheaac_sub32_sat(ixheaacd_mult32_sat(x2i, w2h),
                                   ixheaacd_mult32_sat(x2r, w2l));
          x2r = tmp;

          tmp = ixheaac_sub32_sat(ixheaacd_mult32_sat(x3i, w3h),
                                   ixheaacd_mult32_sat(x3r, w3l));
          x3i = ixheaacd_mac32_sat(ixheaacd_mult32_sat(x3r, w3h), x3i, w3l);
          x3r = tmp;

          x0r = (*data);
          x0i = (*(data + 1));

          x0r = ixheaac_add32_sat(x0r, x2r);
          x0i = ixheaac_add32_sat(x0i, x2i);
          x2r = ixheaac_sub32_sat(x0r, ixheaac_shl32_sat(x2r, 1));
          x2i = ixheaac_sub32_sat(x0i, ixheaac_shl32_sat(x2i, 1));
          x1r = ixheaac_add32_sat(x1r, x3r);
          x1i = ixheaac_sub32_sat(x1i, x3i);
          x3r = ixheaac_sub32_sat(x1r, ixheaac_shl32_sat(x3r, 1));
          x3i = ixheaac_add32_sat(x1i, ixheaac_shl32_sat(x3i, 1));

          x0r = ixheaac_add32_sat(x0r, x1r);
          x0i = ixheaac_add32_sat(x0i, x1i);
          x1r = ixheaac_sub32_sat(x0r, ixheaac_shl32_sat(x1r, 1));
          x1i = ixheaac_sub32_sat(x0i, ixheaac_shl32_sat(x1i, 1));
          x2r = ixheaac_add32_sat(x2r, x3i);
          x2i = ixheaac_sub32_sat(x2i, x3r);
          x3i = ixheaac_sub32_sat(x2r, ixheaac_shl32_sat(x3i, 1));
          x3r = ixheaac_add32_sat(x2i, ixheaac_shl32_sat(x3r, 1));

          *data = x0r;
          *(data + 1) = x0i;
          data += (del << 1);

          *data = x2r;
          *(data + 1) = x2i;
          data += (del << 1);

          *data = x1r;
          *(data + 1) = x1i;
          data += (del << 1);

          *data = x3i;
          *(data + 1) = x3r;
          data += (del << 1);
        }
        data -= 2 * npoints;
        data += 2;
      }
      nodespacing >>= 2;
      del <<= 2;
      in_loop_cnt >>= 2;
    }
    if (not_power_4) {
      const WORD32 *twiddles = ptr_w;
      nodespacing <<= 1;
      shift += 1;

      for (j = del / 2; j != 0; j--) {
        WORD32 w1h = *twiddles;
        WORD32 w1l = *(twiddles + 1);
        WORD32 tmp;
        twiddles += nodespacing * 2;

        x0r = *ptr_y;
        x0i = *(ptr_y + 1);
        ptr_y += (del << 1);

        x1r = *ptr_y;
        x1i = *(ptr_y + 1);

        tmp = ixheaac_sub32_sat(ixheaacd_mult32_sat(x1r, w1l),
                                 ixheaacd_mult32_sat(x1i, w1h));
        x1i = ixheaacd_mac32_sat(ixheaacd_mult32_sat(x1r, w1h), x1i, w1l);
        x1r = tmp;

        *ptr_y = (x0r) / 2 - (x1r) / 2;
        *(ptr_y + 1) = (x0i) / 2 - (x1i) / 2;
        ptr_y -= (del << 1);

        *ptr_y = (x0r) / 2 + (x1r) / 2;
        *(ptr_y + 1) = (x0i) / 2 + (x1i) / 2;
        ptr_y += 2;
      }
      twiddles = ptr_w;
      for (j = del / 2; j != 0; j--) {
        WORD32 w1h = *twiddles;
        WORD32 w1l = *(twiddles + 1);
        WORD32 tmp;
        twiddles += nodespacing * 2;

        x0r = *ptr_y;
        x0i = *(ptr_y + 1);
        ptr_y += (del << 1);

        x1r = *ptr_y;
        x1i = *(ptr_y + 1);

        tmp = ixheaac_add32_sat(ixheaacd_mult32_sat(x1r, w1h),
                                 ixheaacd_mult32_sat(x1i, w1l));
        x1i = ixheaac_sub32_sat(ixheaacd_mult32_sat(x1i, w1h),
                                 ixheaacd_mult32_sat(x1r, w1l));
        x1r = tmp;

        *ptr_y = (x0r) / 2 - (x1r) / 2;
        *(ptr_y + 1) = (x0i) / 2 - (x1i) / 2;
        ptr_y -= (del << 1);

        *ptr_y = (x0r) / 2 + (x1r) / 2;
        *(ptr_y + 1) = (x0i) / 2 + (x1i) / 2;
        ptr_y += 2;
      }
    }
  }

  else {
    ptr_w = ixheaacd_twiddle_table_fft_32x32;

    for (i = 0; i < npoints; i += 4) {
      WORD32 *inp = ptr_x;

      DIG_REV(i, dig_rev_shift, h2);
      if (not_power_4) {
        h2 += 1;
        h2 &= ~1;
      }
      inp += (h2);

      x0r = *inp;
      x0i = *(inp + 1);
      inp += (npoints >> 1);

      x1r = *inp;
      x1i = *(inp + 1);
      inp += (npoints >> 1);

      x2r = *inp;
      x2i = *(inp + 1);
      inp += (npoints >> 1);

      x3r = *inp;
      x3i = *(inp + 1);

      x0r = ixheaac_add32_sat(x0r, x2r);
      x0i = ixheaac_add32_sat(x0i, x2i);
      x2r = ixheaac_sub32_sat(x0r, ixheaac_shl32_sat(x2r, 1));
      x2i = ixheaac_sub32_sat(x0i, ixheaac_shl32_sat(x2i, 1));
      x1r = ixheaac_add32_sat(x1r, x3r);
      x1i = ixheaac_add32_sat(x1i, x3i);
      x3r = ixheaac_sub32_sat(x1r, ixheaac_shl32_sat(x3r, 1));
      x3i = ixheaac_sub32_sat(x1i, ixheaac_shl32_sat(x3i, 1));

      x0r = ixheaac_add32_sat(x0r, x1r);
      x0i = ixheaac_add32_sat(x0i, x1i);
      x1r = ixheaac_sub32_sat(x0r, ixheaac_shl32_sat(x1r, 1));
      x1i = ixheaac_sub32_sat(x0i, ixheaac_shl32_sat(x1i, 1));
      x2r = ixheaac_sub32_sat(x2r, x3i);
      x2i = ixheaac_add32_sat(x2i, x3r);
      x3i = ixheaac_add32_sat(x2r, ixheaac_shl32_sat(x3i, 1));
      x3r = ixheaac_sub32_sat(x2i, ixheaac_shl32_sat(x3r, 1));

      *ptr_y++ = x0r;
      *ptr_y++ = x0i;
      *ptr_y++ = x2r;
      *ptr_y++ = x2i;
      *ptr_y++ = x1r;
      *ptr_y++ = x1i;
      *ptr_y++ = x3i;
      *ptr_y++ = x3r;
    }
    ptr_y -= 2 * npoints;
    del = 4;
    nodespacing = 64;
    in_loop_cnt = npoints >> 4;
    for (i = n_stages - 1; i > 0; i--) {
      const WORD32 *twiddles = ptr_w;
      WORD32 *data = ptr_y;
      WORD32 w1h, w2h, w3h, w1l, w2l, w3l;
      WORD32 sec_loop_cnt;

      for (k = in_loop_cnt; k != 0; k--) {
        x0r = (*data);
        x0i = (*(data + 1));
        data += (del << 1);

        x1r = (*data);
        x1i = (*(data + 1));
        data += (del << 1);

        x2r = (*data);
        x2i = (*(data + 1));
        data += (del << 1);

        x3r = (*data);
        x3i = (*(data + 1));
        data -= 3 * (del << 1);

        x0r = ixheaac_add32_sat(x0r, x2r);
        x0i = ixheaac_add32_sat(x0i, x2i);
        x2r = ixheaac_sub32_sat(x0r, ixheaac_shl32_sat(x2r, 1));
        x2i = ixheaac_sub32_sat(x0i, ixheaac_shl32_sat(x2i, 1));
        x1r = ixheaac_add32_sat(x1r, x3r);
        x1i = ixheaac_add32_sat(x1i, x3i);
        x3r = ixheaac_sub32_sat(x1r, ixheaac_shl32_sat(x3r, 1));
        x3i = ixheaac_sub32_sat(x1i, ixheaac_shl32_sat(x3i, 1));

        x0r = ixheaac_add32_sat(x0r, x1r);
        x0i = ixheaac_add32_sat(x0i, x1i);
        x1r = ixheaac_sub32_sat(x0r, ixheaac_shl32_sat(x1r, 1));
        x1i = ixheaac_sub32_sat(x0i, ixheaac_shl32_sat(x1i, 1));
        x2r = ixheaac_sub32_sat(x2r, x3i);
        x2i = ixheaac_add32_sat(x2i, x3r);
        x3i = ixheaac_add32_sat(x2r, ixheaac_shl32_sat(x3i, 1));
        x3r = ixheaac_sub32_sat(x2i, ixheaac_shl32_sat(x3r, 1));

        *data = x0r;
        *(data + 1) = x0i;
        data += (del << 1);

        *data = x2r;
        *(data + 1) = x2i;
        data += (del << 1);

        *data = x1r;
        *(data + 1) = x1i;
        data += (del << 1);

        *data = x3i;
        *(data + 1) = x3r;
        data += (del << 1);
      }
      data = ptr_y + 2;

      sec_loop_cnt = (nodespacing * del);
      sec_loop_cnt = (sec_loop_cnt / 4) + (sec_loop_cnt / 8) -
                     (sec_loop_cnt / 16) + (sec_loop_cnt / 32) -
                     (sec_loop_cnt / 64) + (sec_loop_cnt / 128) -
                     (sec_loop_cnt / 256);
      j = nodespacing;

      for (j = nodespacing; j <= sec_loop_cnt; j += nodespacing) {
        w1h = *(twiddles + 2 * j);
        w2h = *(twiddles + 2 * (j << 1));
        w3h = *(twiddles + 2 * j + 2 * (j << 1));
        w1l = *(twiddles + 2 * j + 1);
        w2l = *(twiddles + 2 * (j << 1) + 1);
        w3l = *(twiddles + 2 * j + 2 * (j << 1) + 1);

        for (k = in_loop_cnt; k != 0; k--) {
          WORD32 tmp;
          WORD32 x0r, x0i, x1r, x1i, x2r, x2i, x3r, x3i;

          data += (del << 1);

          x1r = *data;
          x1i = *(data + 1);
          data += (del << 1);

          x2r = *data;
          x2i = *(data + 1);
          data += (del << 1);

          x3r = *data;
          x3i = *(data + 1);
          data -= 3 * (del << 1);

          tmp = ixheaac_add32_sat(ixheaacd_mult32_sat(x1r, w1l),
                                   ixheaacd_mult32_sat(x1i, w1h));
          x1i = ixheaacd_mac32_sat(-ixheaacd_mult32_sat(x1r, w1h), x1i, w1l);
          x1r = tmp;

          tmp = ixheaac_add32_sat(ixheaacd_mult32_sat(x2r, w2l),
                                   ixheaacd_mult32_sat(x2i, w2h));
          x2i = ixheaacd_mac32_sat(-ixheaacd_mult32_sat(x2r, w2h), x2i, w2l);
          x2r = tmp;

          tmp = ixheaac_add32_sat(ixheaacd_mult32_sat(x3r, w3l),
                                   ixheaacd_mult32_sat(x3i, w3h));
          x3i = ixheaacd_mac32_sat(-ixheaacd_mult32_sat(x3r, w3h), x3i, w3l);
          x3r = tmp;

          x0r = (*data);
          x0i = (*(data + 1));

          x0r = ixheaac_add32_sat(x0r, x2r);
          x0i = ixheaac_add32_sat(x0i, x2i);
          x2r = ixheaac_sub32_sat(x0r, ixheaac_shl32_sat(x2r, 1));
          x2i = ixheaac_sub32_sat(x0i, ixheaac_shl32_sat(x2i, 1));
          x1r = ixheaac_add32_sat(x1r, x3r);
          x1i = ixheaac_add32_sat(x1i, x3i);
          x3r = ixheaac_sub32_sat(x1r, ixheaac_shl32_sat(x3r, 1));
          x3i = ixheaac_sub32_sat(x1i, ixheaac_shl32_sat(x3i, 1));

          x0r = ixheaac_add32_sat(x0r, x1r);
          x0i = ixheaac_add32_sat(x0i, x1i);
          x1r = ixheaac_sub32_sat(x0r, ixheaac_shl32_sat(x1r, 1));
          x1i = ixheaac_sub32_sat(x0i, ixheaac_shl32_sat(x1i, 1));
          x2r = ixheaac_sub32_sat(x2r, x3i);
          x2i = ixheaac_add32_sat(x2i, x3r);
          x3i = ixheaac_add32_sat(x2r, ixheaac_shl32_sat(x3i, 1));
          x3r = ixheaac_sub32_sat(x2i, ixheaac_shl32_sat(x3r, 1));

          *data = x0r;
          *(data + 1) = x0i;
          data += (del << 1);

          *data = x2r;
          *(data + 1) = x2i;
          data += (del << 1);

          *data = x1r;
          *(data + 1) = x1i;
          data += (del << 1);

          *data = x3i;
          *(data + 1) = x3r;
          data += (del << 1);
        }
        data -= 2 * npoints;
        data += 2;
      }
      for (; j <= (nodespacing * del) >> 1; j += nodespacing) {
        w1h = *(twiddles + 2 * j);
        w2h = *(twiddles + 2 * (j << 1));
        w3h = *(twiddles + 2 * j + 2 * (j << 1) - 512);
        w1l = *(twiddles + 2 * j + 1);
        w2l = *(twiddles + 2 * (j << 1) + 1);
        w3l = *(twiddles + 2 * j + 2 * (j << 1) - 511);

        for (k = in_loop_cnt; k != 0; k--) {
          WORD32 tmp;
          WORD32 x0r, x0i, x1r, x1i, x2r, x2i, x3r, x3i;

          data += (del << 1);

          x1r = *data;
          x1i = *(data + 1);
          data += (del << 1);

          x2r = *data;
          x2i = *(data + 1);
          data += (del << 1);

          x3r = *data;
          x3i = *(data + 1);
          data -= 3 * (del << 1);

          tmp = ixheaac_add32_sat(ixheaacd_mult32_sat(x1r, w1l),
                                   ixheaacd_mult32_sat(x1i, w1h));
          x1i = ixheaacd_mac32_sat(-ixheaacd_mult32_sat(x1r, w1h), x1i, w1l);
          x1r = tmp;

          tmp = ixheaac_add32_sat(ixheaacd_mult32_sat(x2r, w2l),
                                   ixheaacd_mult32_sat(x2i, w2h));
          x2i = ixheaacd_mac32_sat(-ixheaacd_mult32_sat(x2r, w2h), x2i, w2l);
          x2r = tmp;

          tmp = ixheaac_sub32_sat(ixheaacd_mult32_sat(x3r, w3h),
                                   ixheaacd_mult32_sat(x3i, w3l));
          x3i = ixheaac_add32_sat(ixheaacd_mult32_sat(x3r, w3l),
                                   ixheaacd_mult32_sat(x3i, w3h));
          x3r = tmp;

          x0r = (*data);
          x0i = (*(data + 1));

          x0r = ixheaac_add32_sat(x0r, x2r);
          x0i = ixheaac_add32_sat(x0i, x2i);
          x2r = ixheaac_sub32_sat(x0r, ixheaac_shl32_sat(x2r, 1));
          x2i = ixheaac_sub32_sat(x0i, ixheaac_shl32_sat(x2i, 1));
          x1r = ixheaac_add32_sat(x1r, x3r);
          x1i = ixheaac_add32_sat(x1i, x3i);
          x3r = ixheaac_sub32_sat(x1r, ixheaac_shl32_sat(x3r, 1));
          x3i = ixheaac_sub32_sat(x1i, ixheaac_shl32_sat(x3i, 1));

          x0r = ixheaac_add32_sat(x0r, x1r);
          x0i = ixheaac_add32_sat(x0i, x1i);
          x1r = ixheaac_sub32_sat(x0r, ixheaac_shl32_sat(x1r, 1));
          x1i = ixheaac_sub32_sat(x0i, ixheaac_shl32_sat(x1i, 1));
          x2r = ixheaac_sub32_sat(x2r, x3i);
          x2i = ixheaac_add32_sat(x2i, x3r);
          x3i = ixheaac_add32_sat(x2r, ixheaac_shl32_sat(x3i, 1));
          x3r = ixheaac_sub32_sat(x2i, ixheaac_shl32_sat(x3r, 1));

          *data = x0r;
          *(data + 1) = x0i;
          data += (del << 1);

          *data = x2r;
          *(data + 1) = x2i;
          data += (del << 1);

          *data = x1r;
          *(data + 1) = x1i;
          data += (del << 1);

          *data = x3i;
          *(data + 1) = x3r;
          data += (del << 1);
        }
        data -= 2 * npoints;
        data += 2;
      }
      for (; j <= sec_loop_cnt * 2; j += nodespacing) {
        w1h = *(twiddles + 2 * j);
        w2h = *(twiddles + 2 * (j << 1) - 512);
        w3h = *(twiddles + 2 * j + 2 * (j << 1) - 512);
        w1l = *(twiddles + 2 * j + 1);
        w2l = *(twiddles + 2 * (j << 1) - 511);
        w3l = *(twiddles + 2 * j + 2 * (j << 1) - 511);

        for (k = in_loop_cnt; k != 0; k--) {
          WORD32 tmp;
          WORD32 x0r, x0i, x1r, x1i, x2r, x2i, x3r, x3i;

          data += (del << 1);

          x1r = *data;
          x1i = *(data + 1);
          data += (del << 1);

          x2r = *data;
          x2i = *(data + 1);
          data += (del << 1);

          x3r = *data;
          x3i = *(data + 1);
          data -= 3 * (del << 1);

          tmp = ixheaac_add32_sat(ixheaacd_mult32_sat(x1r, w1l),
                                   ixheaacd_mult32_sat(x1i, w1h));
          x1i = ixheaacd_mac32_sat(-ixheaacd_mult32_sat(x1r, w1h), x1i, w1l);
          x1r = tmp;

          tmp = ixheaac_sub32_sat(ixheaacd_mult32_sat(x2r, w2h),
                                   ixheaacd_mult32_sat(x2i, w2l));
          x2i = ixheaac_add32_sat(ixheaacd_mult32_sat(x2r, w2l),
                                   ixheaacd_mult32_sat(x2i, w2h));
          x2r = tmp;

          tmp = ixheaac_sub32_sat(ixheaacd_mult32_sat(x3r, w3h),
                                   ixheaacd_mult32_sat(x3i, w3l));
          x3i = ixheaac_add32_sat(ixheaacd_mult32_sat(x3r, w3l),
                                   ixheaacd_mult32_sat(x3i, w3h));
          x3r = tmp;

          x0r = (*data);
          x0i = (*(data + 1));

          x0r = ixheaac_add32_sat(x0r, x2r);
          x0i = ixheaac_add32_sat(x0i, x2i);
          x2r = ixheaac_sub32_sat(x0r, ixheaac_shl32_sat(x2r, 1));
          x2i = ixheaac_sub32_sat(x0i, ixheaac_shl32_sat(x2i, 1));
          x1r = ixheaac_add32_sat(x1r, x3r);
          x1i = ixheaac_add32_sat(x1i, x3i);
          x3r = ixheaac_sub32_sat(x1r, ixheaac_shl32_sat(x3r, 1));
          x3i = ixheaac_sub32_sat(x1i, ixheaac_shl32_sat(x3i, 1));

          x0r = ixheaac_add32_sat(x0r, x1r);
          x0i = ixheaac_add32_sat(x0i, x1i);
          x1r = ixheaac_sub32_sat(x0r, ixheaac_shl32_sat(x1r, 1));
          x1i = ixheaac_sub32_sat(x0i, ixheaac_shl32_sat(x1i, 1));
          x2r = ixheaac_sub32_sat(x2r, x3i);
          x2i = ixheaac_add32_sat(x2i, x3r);
          x3i = ixheaac_add32_sat(x2r, ixheaac_shl32_sat(x3i, 1));
          x3r = ixheaac_sub32_sat(x2i, ixheaac_shl32_sat(x3r, 1));

          *data = x0r;
          *(data + 1) = x0i;
          data += (del << 1);

          *data = x2r;
          *(data + 1) = x2i;
          data += (del << 1);

          *data = x1r;
          *(data + 1) = x1i;
          data += (del << 1);

          *data = x3i;
          *(data + 1) = x3r;
          data += (del << 1);
        }
        data -= 2 * npoints;
        data += 2;
      }
      for (; j < nodespacing * del; j += nodespacing) {
        w1h = *(twiddles + 2 * j);
        w2h = *(twiddles + 2 * (j << 1) - 512);
        w3h = *(twiddles + 2 * j + 2 * (j << 1) - 1024);
        w1l = *(twiddles + 2 * j + 1);
        w2l = *(twiddles + 2 * (j << 1) - 511);
        w3l = *(twiddles + 2 * j + 2 * (j << 1) - 1023);

        for (k = in_loop_cnt; k != 0; k--) {
          WORD32 tmp;
          WORD32 x0r, x0i, x1r, x1i, x2r, x2i, x3r, x3i;

          data += (del << 1);

          x1r = *data;
          x1i = *(data + 1);
          data += (del << 1);

          x2r = *data;
          x2i = *(data + 1);
          data += (del << 1);

          x3r = *data;
          x3i = *(data + 1);
          data -= 3 * (del << 1);

          tmp = ixheaac_add32_sat(ixheaacd_mult32_sat(x1r, w1l),
                                   ixheaacd_mult32_sat(x1i, w1h));
          x1i = ixheaacd_mac32_sat(-ixheaacd_mult32_sat(x1r, w1h), x1i, w1l);
          x1r = tmp;

          tmp = ixheaac_sub32_sat(ixheaacd_mult32_sat(x2r, w2h),
                                   ixheaacd_mult32_sat(x2i, w2l));
          x2i = ixheaac_add32_sat(ixheaacd_mult32_sat(x2r, w2l),
                                   ixheaacd_mult32_sat(x2i, w2h));
          x2r = tmp;

          tmp = -ixheaac_add32_sat(ixheaacd_mult32_sat(x3r, w3l),
                                    ixheaacd_mult32_sat(x3i, w3h));
          x3i = ixheaacd_mac32_sat(-ixheaacd_mult32_sat(x3r, w3h), x3i, w3l);
          x3r = tmp;

          x0r = (*data);
          x0i = (*(data + 1));

          x0r = ixheaac_add32_sat(x0r, x2r);
          x0i = ixheaac_add32_sat(x0i, x2i);
          x2r = ixheaac_sub32_sat(x0r, ixheaac_shl32_sat(x2r, 1));
          x2i = ixheaac_sub32_sat(x0i, ixheaac_shl32_sat(x2i, 1));
          x1r = ixheaac_add32_sat(x1r, x3r);
          x1i = ixheaac_sub32_sat(x1i, x3i);
          x3r = ixheaac_sub32_sat(x1r, ixheaac_shl32_sat(x3r, 1));
          x3i = ixheaac_add32_sat(x1i, ixheaac_shl32_sat(x3i, 1));

          x0r = ixheaac_add32_sat(x0r, x1r);
          x0i = ixheaac_add32_sat(x0i, x1i);
          x1r = ixheaac_sub32_sat(x0r, ixheaac_shl32_sat(x1r, 1));
          x1i = ixheaac_sub32_sat(x0i, ixheaac_shl32_sat(x1i, 1));
          x2r = ixheaac_sub32_sat(x2r, x3i);
          x2i = ixheaac_add32_sat(x2i, x3r);
          x3i = ixheaac_add32_sat(x2r, ixheaac_shl32_sat(x3i, 1));
          x3r = ixheaac_sub32_sat(x2i, ixheaac_shl32_sat(x3r, 1));

          *data = x0r;
          *(data + 1) = x0i;
          data += (del << 1);

          *data = x2r;
          *(data + 1) = x2i;
          data += (del << 1);

          *data = x1r;
          *(data + 1) = x1i;
          data += (del << 1);

          *data = x3i;
          *(data + 1) = x3r;
          data += (del << 1);
        }
        data -= 2 * npoints;
        data += 2;
      }
      nodespacing >>= 2;
      del <<= 2;
      in_loop_cnt >>= 2;
    }
    if (not_power_4) {
      const WORD32 *twiddles = ptr_w;
      nodespacing <<= 1;
      shift += 1;
      for (j = del / 2; j != 0; j--) {
        WORD32 w1h = *twiddles;
        WORD32 w1l = *(twiddles + 1);

        WORD32 tmp;
        twiddles += nodespacing * 2;

        x0r = *ptr_y;
        x0i = *(ptr_y + 1);
        ptr_y += (del << 1);

        x1r = *ptr_y;
        x1i = *(ptr_y + 1);

        tmp = ixheaac_add32_sat(ixheaacd_mult32_sat(x1r, w1l),
                                 ixheaacd_mult32_sat(x1i, w1h));
        x1i = ixheaacd_mac32_sat(-ixheaacd_mult32_sat(x1r, w1h), x1i, w1l);
        x1r = tmp;

        *ptr_y = (x0r) / 2 - (x1r) / 2;
        *(ptr_y + 1) = (x0i) / 2 - (x1i) / 2;
        ptr_y -= (del << 1);

        *ptr_y = (x0r) / 2 + (x1r) / 2;
        *(ptr_y + 1) = (x0i) / 2 + (x1i) / 2;
        ptr_y += 2;
      }
      twiddles = ptr_w;
      for (j = del / 2; j != 0; j--) {
        WORD32 w1h = *twiddles;
        WORD32 w1l = *(twiddles + 1);
        WORD32 tmp;
        twiddles += nodespacing * 2;

        x0r = *ptr_y;
        x0i = *(ptr_y + 1);
        ptr_y += (del << 1);

        x1r = *ptr_y;
        x1i = *(ptr_y + 1);

        tmp = ixheaac_sub32_sat(ixheaacd_mult32_sat(x1r, w1h),
                                 ixheaacd_mult32_sat(x1i, w1l));
        x1i = ixheaac_add32_sat(ixheaacd_mult32_sat(x1r, w1l),
                                 ixheaacd_mult32_sat(x1i, w1h));
        x1r = tmp;

        *ptr_y = (x0r) / 2 - (x1r) / 2;
        *(ptr_y + 1) = (x0i) / 2 - (x1i) / 2;
        ptr_y -= (del << 1);

        *ptr_y = (x0r) / 2 + (x1r) / 2;
        *(ptr_y + 1) = (x0i) / 2 + (x1i) / 2;
        ptr_y += 2;
      }
    }
  }

  for (i = 0; i < nlength; i++) {
    xr[i] = y[2 * i];
    xi[i] = y[2 * i + 1];
  }

  *preshift = shift - *preshift;
  return;
}

static PLATFORM_INLINE void ixheaacd_complex_3point_fft(WORD32 *inp, WORD32 *op,
                                                        WORD32 sign_dir) {
  WORD32 add_r, sub_r;
  WORD32 add_i, sub_i;
  WORD32 temp_real, temp_imag, temp;

  WORD32 p1, p2, p3, p4;

  WORD32 sinmu;
  sinmu = -1859775393 * sign_dir;

  temp_real = ixheaac_add32_sat(inp[0], inp[2]);
  temp_imag = ixheaac_add32_sat(inp[1], inp[3]);

  add_r = ixheaac_add32_sat(inp[2], inp[4]);
  add_i = ixheaac_add32_sat(inp[3], inp[5]);

  sub_r = ixheaac_sub32_sat(inp[2], inp[4]);
  sub_i = ixheaac_sub32_sat(inp[3], inp[5]);

  p1 = add_r >> 1;
  p4 = add_i >> 1;
  p2 = ixheaac_mult32_shl(sub_i, sinmu);
  p3 = ixheaac_mult32_shl(sub_r, sinmu);

  temp = ixheaac_sub32(inp[0], p1);

  op[0] = ixheaac_add32_sat(temp_real, inp[4]);
  op[1] = ixheaac_add32_sat(temp_imag, inp[5]);
  op[2] = ixheaac_add32_sat(temp, p2);
  op[3] = ixheaac_sub32_sat(ixheaac_sub32_sat(inp[1], p3), p4);
  op[4] = ixheaac_sub32_sat(temp, p2);
  op[5] = ixheaac_sub32_sat(ixheaac_add32_sat(inp[1], p3), p4);

  return;
}

VOID ixheaacd_complex_fft_p3(WORD32 *xr, WORD32 *xi, WORD32 nlength,
                             WORD32 fft_mode, WORD32 *preshift) {
  WORD32 i, j;
  WORD32 shift = 0;
  WORD32 xr_3[384];
  WORD32 xi_3[384];
  WORD32 x[1024];
  WORD32 y[1024];
  WORD32 cnfac, npts;
  WORD32 mpass = nlength;
  WORD32 n = 0;
  WORD32 *ptr_x = x;
  WORD32 *ptr_y = y;

  cnfac = 0;
  while (mpass % 3 == 0) {
    mpass /= 3;
    cnfac++;
  }
  npts = mpass;

  for (i = 0; i < 3 * cnfac; i++) {
    for (j = 0; j < mpass; j++) {
      xr_3[j] = xr[3 * j + i];
      xi_3[j] = xi[3 * j + i];
    }

    (*ixheaacd_complex_fft_p2)(xr_3, xi_3, mpass, fft_mode, &shift);

    for (j = 0; j < mpass; j++) {
      xr[3 * j + i] = xr_3[j];
      xi[3 * j + i] = xi_3[j];
    }
  }

  while (npts >> 1) {
    n++;
    npts = npts >> 1;
  }

  if (n % 2 == 0)
    shift = ((n + 4)) / 2;
  else
    shift = ((n + 5) / 2);

  *preshift = shift - *preshift + 1;

  for (i = 0; i < nlength; i++) {
    ptr_x[2 * i] = (xr[i] >> 1);
    ptr_x[2 * i + 1] = (xi[i] >> 1);
  }

  {
    const WORD32 *w1r, *w1i;
    WORD32 tmp;
    w1r = ixheaacd_twiddle_table_3pr;
    w1i = ixheaacd_twiddle_table_3pi;

    if (fft_mode < 0) {
      for (i = 0; i < nlength; i += 3) {
        w1r++;
        w1i++;

        tmp = ixheaac_sub32_sat(ixheaacd_mult32_sat(ptr_x[2 * i + 2], (*w1r)),
                                 ixheaacd_mult32_sat(ptr_x[2 * i + 3], (*w1i)));
        ptr_x[2 * i + 3] =
            ixheaac_add32_sat(ixheaacd_mult32_sat(ptr_x[2 * i + 2], (*w1i)),
                               ixheaacd_mult32_sat(ptr_x[2 * i + 3], (*w1r)));
        ptr_x[2 * i + 2] = tmp;

        w1r++;
        w1i++;

        tmp = ixheaac_sub32_sat(ixheaacd_mult32_sat(ptr_x[2 * i + 4], (*w1r)),
                                 ixheaacd_mult32_sat(ptr_x[2 * i + 5], (*w1i)));
        ptr_x[2 * i + 5] =
            ixheaac_add32_sat(ixheaacd_mult32_sat(ptr_x[2 * i + 4], (*w1i)),
                               ixheaacd_mult32_sat(ptr_x[2 * i + 5], (*w1r)));
        ptr_x[2 * i + 4] = tmp;

        w1r += 3 * (128 / mpass - 1) + 1;
        w1i += 3 * (128 / mpass - 1) + 1;
      }
    }

    else {
      for (i = 0; i < nlength; i += 3) {
        w1r++;
        w1i++;

        tmp = ixheaac_add32_sat(ixheaacd_mult32_sat(ptr_x[2 * i + 2], (*w1r)),
                                 ixheaacd_mult32_sat(ptr_x[2 * i + 3], (*w1i)));
        ptr_x[2 * i + 3] =
            ixheaac_sub32_sat(ixheaacd_mult32_sat(ptr_x[2 * i + 3], (*w1r)),
                               ixheaacd_mult32_sat(ptr_x[2 * i + 2], (*w1i)));
        ptr_x[2 * i + 2] = tmp;

        w1r++;
        w1i++;

        tmp = ixheaac_add32_sat(ixheaacd_mult32_sat(ptr_x[2 * i + 4], (*w1r)),
                                 ixheaacd_mult32_sat(ptr_x[2 * i + 5], (*w1i)));
        ptr_x[2 * i + 5] =
            ixheaac_sub32_sat(ixheaacd_mult32_sat(ptr_x[2 * i + 5], (*w1r)),
                               ixheaacd_mult32_sat(ptr_x[2 * i + 4], (*w1i)));
        ptr_x[2 * i + 4] = tmp;

        w1r += 3 * (128 / mpass - 1) + 1;
        w1i += 3 * (128 / mpass - 1) + 1;
      }
    }
  }

  for (i = 0; i < mpass; i++) {
    ixheaacd_complex_3point_fft(ptr_x, ptr_y, fft_mode);

    ptr_x = ptr_x + 6;
    ptr_y = ptr_y + 6;
  }

  ptr_y = y;
  for (i = 0; i < mpass; i++) {
    xr[i] = *ptr_y++;
    xi[i] = *ptr_y++;
    xr[mpass + i] = *ptr_y++;
    xi[mpass + i] = *ptr_y++;
    xr[2 * mpass + i] = *ptr_y++;
    xi[2 * mpass + i] = *ptr_y++;
  }

  return;
}

VOID ixheaacd_complex_fft(WORD32 *data_r, WORD32 *data_i, WORD32 nlength, WORD32 fft_mode,
                          WORD32 *preshift) {
  if (nlength & (nlength - 1)) {
    ixheaacd_complex_fft_p3(data_r, data_i, nlength, fft_mode, preshift);
  } else
    (*ixheaacd_complex_fft_p2)(data_r, data_i, nlength, fft_mode, preshift);

  return;
}
