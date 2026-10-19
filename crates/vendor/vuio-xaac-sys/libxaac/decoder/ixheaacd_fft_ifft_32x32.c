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
#include <math.h>
#include "ixheaac_type_def.h"
#include "ixheaac_constants.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_fft_ifft_rom.h"
#include "ixheaacd_dsp_fft32x32s.h"

#define DIG_REV(i, m, j)                                    \
  do {                                                      \
    unsigned _ = (i);                                       \
    _ = ((_ & 0x33333333) << 2) | ((_ & ~0x33333333) >> 2); \
    _ = ((_ & 0x0F0F0F0F) << 4) | ((_ & ~0x0F0F0F0F) >> 4); \
    _ = ((_ & 0x00FF00FF) << 8) | ((_ & ~0x00FF00FF) >> 8); \
    (j) = _ >> (m);                                         \
  } while (0)

FLOAT64 ixheaacd_mult32X32float(FLOAT64 a, FLOAT64 b) {
  FLOAT64 result;

  result = a * b;

  return result;
}

FLOAT64 ixheaacd_mac32X32float(FLOAT64 a, FLOAT64 b, FLOAT64 c) {
  FLOAT64 result;

  result = a + b * c;

  return result;
}

VOID ixheaacd_hbe_apply_ifft_7(FLOAT32 *inp, FLOAT32 *op) {
  FLOAT32 x0r, x1r, x2r, x3r, x4r, x5r, x6r, x7r, x8r;
  FLOAT32 x0i, x1i, x2i, x3i, x4i, x5i, x6i, x7i, x8i;
  FLOAT32 y0r, y1r, y2r, y3r, y4r, y5r, y6r, y7r, y8r;
  FLOAT32 y0i, y1i, y2i, y3i, y4i, y5i, y6i, y7i, y8i;

  x0r = inp[0];
  x0i = inp[1];
  x1r = inp[2] + inp[12];
  x1i = inp[3] + inp[13];
  x2r = inp[2] - inp[12];
  x2i = inp[3] - inp[13];
  x3r = inp[4] + inp[10];
  x3i = inp[5] + inp[11];
  x4r = inp[4] - inp[10];
  x4i = inp[5] - inp[11];
  x5r = inp[8] + inp[6];
  x5i = inp[9] + inp[7];
  x6r = inp[8] - inp[6];
  x6i = inp[9] - inp[7];

  y0r = x0r;
  y0i = x0i;
  y1r = x1r + x3r + x5r;
  y1i = x1i + x3i + x5i;
  y2r = x1r - x3r;
  y2i = x1i - x3i;
  y3r = x5r - x1r;
  y3i = x5i - x1i;
  y4r = x3r - x5r;
  y4i = x3i - x5i;
  y5r = x2r + x4r + x6r;
  y5i = x2i + x4i + x6i;
  y6r = x2r - x4r;
  y6i = x2i - x4i;
  y7r = x6r - x2r;
  y7i = x6i - x2i;
  y8r = x4r - x6r;
  y8i = x4i - x6i;

  x0r = y0r + y1r;
  x0i = y0i + y1i;
  x1r = y0r + C70 * y1r;
  x1i = y0i + C70 * y1i;
  x2r = C71 * y2r;
  x2i = C71 * y2i;
  x3r = C72 * y3r;
  x3i = C72 * y3i;
  x4r = C73 * y4r;
  x4i = C73 * y4i;
  x5r = C74 * y5i;
  x5i = -C74 * y5r;
  x6r = C75 * y6i;
  x6i = -C75 * y6r;
  x7r = C76 * y7i;
  x7i = -C76 * y7r;
  x8r = C77 * y8i;
  x8i = -C77 * y8r;

  y0r = x0r;
  y0i = x0i;
  y1r = x1r + x2r + x4r;
  y1i = x1i + x2i + x4i;
  y2r = x1r - x2r - x3r;
  y2i = x1i - x2i - x3i;
  y3r = x1r + x3r - x4r;
  y3i = x1i + x3i - x4i;
  y4r = x5r + x6r + x8r;
  y4i = x5i + x6i + x8i;
  y5r = x5r - x6r - x7r;
  y5i = x5i - x6i - x7i;
  y6r = x5r + x7r - x8r;
  y6i = x5i + x7i - x8i;

  x0r = y0r;
  x0i = y0i;
  x1r = y1r + y4r;
  x1i = y1i + y4i;
  x2r = y3r + y6r;
  x2i = y3i + y6i;
  x3r = y2r - y5r;
  x3i = y2i - y5i;
  x4r = y2r + y5r;
  x4i = y2i + y5i;
  x5r = y3r - y6r;
  x5i = y3i - y6i;
  x6r = y1r - y4r;
  x6i = y1i - y4i;

  op[0] = x0r;
  op[1] = x0i;
  op[2] = x1r;
  op[3] = x1i;
  op[4] = x2r;
  op[5] = x2i;
  op[6] = x3r;
  op[7] = x3i;
  op[8] = x4r;
  op[9] = x4i;
  op[10] = x5r;
  op[11] = x5i;
  op[12] = x6r;
  op[13] = x6i;

  return;
}

VOID ixheaacd_hbe_apply_fft_3(FLOAT32 *inp, FLOAT32 *op, WORD32 i_sign) {
  FLOAT32 add_r, sub_r;
  FLOAT32 add_i, sub_i;
  FLOAT32 X01r, X01i, temp;

  FLOAT32 p1, p2, p3, p4;

  FLOAT64 sinmu;
  sinmu = -0.866025403784439 * (FLOAT64)i_sign;

  X01r = inp[0] + inp[2];
  X01i = inp[1] + inp[3];

  add_r = inp[2] + inp[4];
  add_i = inp[3] + inp[5];

  sub_r = inp[2] - inp[4];
  sub_i = inp[3] - inp[5];

  p1 = add_r / (FLOAT32)2.0;
  p4 = add_i / (FLOAT32)2.0;
  p2 = (FLOAT32)((FLOAT64)sub_i * sinmu);
  p3 = (FLOAT32)((FLOAT64)sub_r * sinmu);

  temp = inp[0] - p1;

  op[0] = X01r + inp[4];
  op[1] = X01i + inp[5];
  op[2] = temp + p2;
  op[3] = (inp[1] - p3) - p4;
  op[4] = temp - p2;
  op[5] = (inp[1] + p3) - p4;

  return;
}

VOID ixheaacd_hbe_apply_tw_mult_ifft(FLOAT32 *inp, FLOAT32 *op, WORD32 dim1, WORD32 dim2,
                                     const FLOAT32 *tw) {
  FLOAT32 accu1, accu2;
  WORD32 i, j;
  WORD32 step_val = (dim2 - 1) << 1;
  for (i = 0; i < (dim2); i++) {
    op[0] = inp[0];
    op[1] = inp[1];
    op += 2;
    inp += 2;
  }

  for (j = 0; j < (dim1 - 1); j++) {
    op[0] = inp[0];
    op[1] = inp[1];
    inp += 2;
    op += 2;
    for (i = 0; i < (dim2 - 1); i++) {
      CPLX_MPY_IFFT(accu1, accu2, inp[2 * i + 0], inp[2 * i + 1], tw[2 * i + 1], tw[2 * i]);
      op[2 * i + 0] = accu1;
      op[2 * i + 1] = accu2;
    }
    inp += step_val;
    op += step_val;
    tw += (dim2 - 1) * 2;
  }
}

VOID ixheaacd_hbe_apply_tw_mult_fft(FLOAT32 *inp, FLOAT32 *op, WORD32 dim1, WORD32 dim2,
                                    const FLOAT32 *tw) {
  FLOAT32 accu1, accu2;
  WORD32 i, j;
  WORD32 step_val = (dim2 - 1) << 1;
  for (i = 0; i < (dim2); i++) {
    op[0] = inp[0];
    op[1] = inp[1];
    op += 2;
    inp += 2;
  }

  for (j = 0; j < (dim1 - 1); j++) {
    op[0] = inp[0];
    op[1] = inp[1];
    inp += 2;
    op += 2;
    for (i = 0; i < (dim2 - 1); i++) {
      CPLX_MPY_FFT(accu1, accu2, inp[2 * i + 0], inp[2 * i + 1], tw[2 * i + 1], tw[2 * i]);
      op[2 * i + 0] = accu1;
      op[2 * i + 1] = accu2;
    }
    inp += step_val;
    op += step_val;
    tw += (dim2 - 1) * 2;
  }
}

VOID ixheaacd_hbe_apply_cfftn(FLOAT32 re[], FLOAT32 *scratch, WORD32 n_pass, WORD32 i_sign) {
  WORD32 i, j, k, n_stages, h2;
  FLOAT32 x0r, x0i, x1r, x1i, x2r, x2i, x3r, x3i;
  WORD32 del, nodespacing, in_loop_cnt;
  WORD32 not_power_4;
  WORD32 dig_rev_shift;
  WORD32 mpass = n_pass;
  WORD32 npoints = n_pass;
  const FLOAT64 *ptr_w;
  FLOAT32 *ptr_x = scratch;
  FLOAT32 *y = scratch + (2 * n_pass);
  FLOAT32 *ptr_y = y;

  dig_rev_shift = ixheaac_norm32(mpass) + 1 - 16;
  n_stages = 30 - ixheaac_norm32(mpass);
  not_power_4 = n_stages & 1;

  n_stages = n_stages >> 1;

  ptr_w = ixheaac_twid_tbl_fft_double;
  ptr_x = re;

  if (i_sign == -1) {
    for (i = 0; i < npoints; i += 4) {
      FLOAT32 *inp = ptr_x;
      FLOAT32 tmk;

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
    }
    ptr_y -= 2 * npoints;
    del = 4;
    nodespacing = 64;
    in_loop_cnt = npoints >> 4;
    for (i = n_stages - 1; i > 0; i--) {
      const FLOAT64 *twiddles = ptr_w;
      FLOAT32 *data = ptr_y;
      FLOAT64 W1, W2, W3, W4, W5, W6;
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
      sec_loop_cnt = (sec_loop_cnt / 4) + (sec_loop_cnt / 8) - (sec_loop_cnt / 16) +
                     (sec_loop_cnt / 32) - (sec_loop_cnt / 64) + (sec_loop_cnt / 128) -
                     (sec_loop_cnt / 256);
      j = nodespacing;

      for (j = nodespacing; j <= sec_loop_cnt; j += nodespacing) {
        W1 = *(twiddles + j);
        W4 = *(twiddles + j + 257);
        W2 = *(twiddles + (j << 1));
        W5 = *(twiddles + (j << 1) + 257);
        W3 = *(twiddles + j + (j << 1));
        W6 = *(twiddles + j + (j << 1) + 257);

        for (k = in_loop_cnt; k != 0; k--) {
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

          tmp = (FLOAT32)(ixheaacd_mult32X32float((FLOAT64)x1r, W1) -
                          ixheaacd_mult32X32float((FLOAT64)x1i, W4));
          x1i = (FLOAT32)ixheaacd_mac32X32float(ixheaacd_mult32X32float((FLOAT64)x1r, W4),
                                                      (FLOAT64)x1i, W1);
          x1r = tmp;

          tmp = (FLOAT32)(ixheaacd_mult32X32float((FLOAT64)x2r, W2) -
                          ixheaacd_mult32X32float((FLOAT64)x2i, W5));
          x2i = (FLOAT32)ixheaacd_mac32X32float(ixheaacd_mult32X32float((FLOAT64)x2r, W5),
                                                      (FLOAT64)x2i, W2);
          x2r = tmp;

          tmp = (FLOAT32)(ixheaacd_mult32X32float((FLOAT64)x3r, W3) -
                          ixheaacd_mult32X32float((FLOAT64)x3i, W6));
          x3i = (FLOAT32)ixheaacd_mac32X32float(ixheaacd_mult32X32float((FLOAT64)x3r, W6),
                                                      (FLOAT64)x3i, W3);
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
      for (; j <= (nodespacing * del) >> 1; j += nodespacing) {
        W1 = *(twiddles + j);
        W4 = *(twiddles + j + 257);
        W2 = *(twiddles + (j << 1));
        W5 = *(twiddles + (j << 1) + 257);
        W3 = *(twiddles + j + (j << 1) - 256);
        W6 = *(twiddles + j + (j << 1) + 1);

        for (k = in_loop_cnt; k != 0; k--) {
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

          tmp = (FLOAT32)(ixheaacd_mult32X32float((FLOAT64)x1r, W1) -
                          ixheaacd_mult32X32float((FLOAT64)x1i, W4));
          x1i = (FLOAT32)ixheaacd_mac32X32float(ixheaacd_mult32X32float((FLOAT64)x1r, W4),
                                                      (FLOAT64)x1i, W1);
          x1r = tmp;

          tmp = (FLOAT32)(ixheaacd_mult32X32float((FLOAT64)x2r, W2) -
                          ixheaacd_mult32X32float((FLOAT64)x2i, W5));
          x2i = (FLOAT32)ixheaacd_mac32X32float(ixheaacd_mult32X32float((FLOAT64)x2r, W5),
                                                      (FLOAT64)x2i, W2);
          x2r = tmp;

          tmp = (FLOAT32)(ixheaacd_mult32X32float((FLOAT64)x3r, W6) +
                          ixheaacd_mult32X32float((FLOAT64)x3i, W3));
          x3i = (FLOAT32)(-ixheaacd_mult32X32float((FLOAT64)x3r, W3) +
                          ixheaacd_mult32X32float((FLOAT64)x3i, W6));
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
      for (; j <= sec_loop_cnt * 2; j += nodespacing) {
        W1 = *(twiddles + j);
        W4 = *(twiddles + j + 257);
        W2 = *(twiddles + (j << 1) - 256);
        W5 = *(twiddles + (j << 1) + 1);
        W3 = *(twiddles + j + (j << 1) - 256);
        W6 = *(twiddles + j + (j << 1) + 1);

        for (k = in_loop_cnt; k != 0; k--) {
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

          tmp = (FLOAT32)(ixheaacd_mult32X32float((FLOAT64)x1r, W1) -
                          ixheaacd_mult32X32float((FLOAT64)x1i, W4));
          x1i = (FLOAT32)ixheaacd_mac32X32float(ixheaacd_mult32X32float(x1r, W4), x1i, W1);
          x1r = tmp;

          tmp = (FLOAT32)(ixheaacd_mult32X32float((FLOAT64)x2r, W5) +
                          ixheaacd_mult32X32float((FLOAT64)x2i, W2));
          x2i = (FLOAT32)(-ixheaacd_mult32X32float(x2r, W2) + ixheaacd_mult32X32float(x2i, W5));
          x2r = tmp;

          tmp = (FLOAT32)(ixheaacd_mult32X32float((FLOAT64)x3r, W6) +
                          ixheaacd_mult32X32float((FLOAT64)x3i, W3));
          x3i = (FLOAT32)(-ixheaacd_mult32X32float((FLOAT64)x3r, W3) +
                          ixheaacd_mult32X32float((FLOAT64)x3i, W6));
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
      for (; j < nodespacing * del; j += nodespacing) {
        W1 = *(twiddles + j);
        W4 = *(twiddles + j + 257);
        W2 = *(twiddles + (j << 1) - 256);
        W5 = *(twiddles + (j << 1) + 1);
        W3 = *(twiddles + j + (j << 1) - 512);
        W6 = *(twiddles + j + (j << 1) - 512 + 257);

        for (k = in_loop_cnt; k != 0; k--) {
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

          tmp = (FLOAT32)(ixheaacd_mult32X32float((FLOAT64)x1r, W1) -
                          ixheaacd_mult32X32float((FLOAT64)x1i, W4));
          x1i = (FLOAT32)ixheaacd_mac32X32float(ixheaacd_mult32X32float((FLOAT64)x1r, W4),
                                                      (FLOAT64)x1i, W1);
          x1r = tmp;

          tmp = (FLOAT32)(ixheaacd_mult32X32float((FLOAT64)x2r, W5) +
                          ixheaacd_mult32X32float((FLOAT64)x2i, W2));
          x2i = (FLOAT32)(-ixheaacd_mult32X32float((FLOAT64)x2r, W2) +
                          ixheaacd_mult32X32float((FLOAT64)x2i, W5));
          x2r = tmp;

          tmp = (FLOAT32)(-ixheaacd_mult32X32float((FLOAT64)x3r, W3) +
                          ixheaacd_mult32X32float((FLOAT64)x3i, W6));
          x3i = (FLOAT32)ixheaacd_mac32X32float(ixheaacd_mult32X32float((FLOAT64)x3r, W6),
                                                      (FLOAT64)x3i, W3);
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
    if (not_power_4) {
      const FLOAT64 *twiddles = ptr_w;
      nodespacing <<= 1;

      for (j = del / 2; j != 0; j--) {
        FLOAT64 W1 = *twiddles;
        FLOAT64 W4 = *(twiddles + 257);
        FLOAT32 tmp;
        twiddles += nodespacing;

        x0r = *ptr_y;
        x0i = *(ptr_y + 1);
        ptr_y += (del << 1);

        x1r = *ptr_y;
        x1i = *(ptr_y + 1);

        tmp = (FLOAT32)(ixheaacd_mult32X32float((FLOAT64)x1r, W1) -
                        ixheaacd_mult32X32float((FLOAT64)x1i, W4));
        x1i = (FLOAT32)ixheaacd_mac32X32float(ixheaacd_mult32X32float((FLOAT64)x1r, W4),
                                                    (FLOAT64)x1i, W1);
        x1r = tmp;

        *ptr_y = (x0r) - (x1r);
        *(ptr_y + 1) = (x0i) - (x1i);
        ptr_y -= (del << 1);

        *ptr_y = (x0r) + (x1r);
        *(ptr_y + 1) = (x0i) + (x1i);
        ptr_y += 2;
      }
      twiddles = ptr_w;
      for (j = del / 2; j != 0; j--) {
        FLOAT64 W1 = *twiddles;
        FLOAT64 W4 = *(twiddles + 257);
        FLOAT32 tmp;
        twiddles += nodespacing;

        x0r = *ptr_y;
        x0i = *(ptr_y + 1);
        ptr_y += (del << 1);

        x1r = *ptr_y;
        x1i = *(ptr_y + 1);

        tmp = (FLOAT32)(ixheaacd_mult32X32float((FLOAT64)x1r, W4) +
                        ixheaacd_mult32X32float((FLOAT64)x1i, W1));
        x1i = (FLOAT32)(-ixheaacd_mult32X32float((FLOAT64)x1r, W1) +
                        ixheaacd_mult32X32float((FLOAT64)x1i, W4));
        x1r = tmp;

        *ptr_y = (x0r) - (x1r);
        *(ptr_y + 1) = (x0i) - (x1i);
        ptr_y -= (del << 1);

        *ptr_y = (x0r) + (x1r);
        *(ptr_y + 1) = (x0i) + (x1i);
        ptr_y += 2;
      }
    }
  } else {
    for (i = 0; i < npoints; i += 4) {
      FLOAT32 *inp = ptr_x;

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
      x2r = x2r - x3i;
      x2i = x2i + x3r;
      x3i = x2r + (x3i * 2);
      x3r = x2i - (x3r * 2);

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
      const FLOAT64 *twiddles = ptr_w;
      FLOAT32 *data = ptr_y;
      FLOAT64 W1, W2, W3, W4, W5, W6;
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
        x2r = x2r - x3i;
        x2i = x2i + x3r;
        x3i = x2r + (x3i * 2);
        x3r = x2i - (x3r * 2);

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
      sec_loop_cnt = (sec_loop_cnt / 4) + (sec_loop_cnt / 8) - (sec_loop_cnt / 16) +
                     (sec_loop_cnt / 32) - (sec_loop_cnt / 64) + (sec_loop_cnt / 128) -
                     (sec_loop_cnt / 256);
      j = nodespacing;

      for (j = nodespacing; j <= sec_loop_cnt; j += nodespacing) {
        W1 = *(twiddles + j);
        W4 = *(twiddles + j + 257);
        W2 = *(twiddles + (j << 1));
        W5 = *(twiddles + (j << 1) + 257);
        W3 = *(twiddles + j + (j << 1));
        W6 = *(twiddles + j + (j << 1) + 257);

        for (k = in_loop_cnt; k != 0; k--) {
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

          tmp = (FLOAT32)(((FLOAT64)x1r * W1) + ((FLOAT64)x1i * W4));
          x1i = (FLOAT32)(-((FLOAT64)x1r * W4) + (FLOAT64)x1i * W1);
          x1r = tmp;

          tmp = (FLOAT32)(((FLOAT64)x2r * W2) + ((FLOAT64)x2i * W5));
          x2i = (FLOAT32)(-((FLOAT64)x2r * W5) + (FLOAT64)x2i * W2);
          x2r = tmp;

          tmp = (FLOAT32)(((FLOAT64)x3r * W3) + ((FLOAT64)x3i * W6));
          x3i = (FLOAT32)(-((FLOAT64)x3r * W6) + (FLOAT64)x3i * W3);
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
          x2r = x2r - (x3i);
          x2i = x2i + (x3r);
          x3i = x2r + (x3i * 2);
          x3r = x2i - (x3r * 2);

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
        W1 = *(twiddles + j);
        W4 = *(twiddles + j + 257);
        W2 = *(twiddles + (j << 1));
        W5 = *(twiddles + (j << 1) + 257);
        W3 = *(twiddles + j + (j << 1) - 256);
        W6 = *(twiddles + j + (j << 1) + 1);

        for (k = in_loop_cnt; k != 0; k--) {
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

          tmp = (FLOAT32)(((FLOAT64)x1r * W1) + ((FLOAT64)x1i * W4));
          x1i = (FLOAT32)(-((FLOAT64)x1r * W4) + (FLOAT64)x1i * W1);
          x1r = tmp;

          tmp = (FLOAT32)(((FLOAT64)x2r * W2) + ((FLOAT64)x2i * W5));
          x2i = (FLOAT32)(-((FLOAT64)x2r * W5) + (FLOAT64)x2i * W2);
          x2r = tmp;

          tmp = (FLOAT32)(((FLOAT64)x3r * W6) - ((FLOAT64)x3i * W3));
          x3i = (FLOAT32)(((FLOAT64)x3r * W3) + ((FLOAT64)x3i * W6));
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
          x2r = x2r - (x3i);
          x2i = x2i + (x3r);
          x3i = x2r + (x3i * 2);
          x3r = x2i - (x3r * 2);

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
        W1 = *(twiddles + j);
        W4 = *(twiddles + j + 257);
        W2 = *(twiddles + (j << 1) - 256);
        W5 = *(twiddles + (j << 1) + 1);
        W3 = *(twiddles + j + (j << 1) - 256);
        W6 = *(twiddles + j + (j << 1) + 1);

        for (k = in_loop_cnt; k != 0; k--) {
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

          tmp = (FLOAT32)(((FLOAT64)x1r * W1) + ((FLOAT64)x1i * W4));
          x1i = (FLOAT32)(-((FLOAT64)x1r * W4) + (FLOAT64)x1i * W1);
          x1r = tmp;

          tmp = (FLOAT32)(((FLOAT64)x2r * W5) - ((FLOAT64)x2i * W2));
          x2i = (FLOAT32)(((FLOAT64)x2r * W2) + ((FLOAT64)x2i * W5));
          x2r = tmp;

          tmp = (FLOAT32)(((FLOAT64)x3r * W6) - ((FLOAT64)x3i * W3));
          x3i = (FLOAT32)(((FLOAT64)x3r * W3) + ((FLOAT64)x3i * W6));
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
          x2r = x2r - (x3i);
          x2i = x2i + (x3r);
          x3i = x2r + (x3i * 2);
          x3r = x2i - (x3r * 2);

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
        W1 = *(twiddles + j);
        W4 = *(twiddles + j + 257);
        W2 = *(twiddles + (j << 1) - 256);
        W5 = *(twiddles + (j << 1) + 1);
        W3 = *(twiddles + j + (j << 1) - 512);
        W6 = *(twiddles + j + (j << 1) - 512 + 257);

        for (k = in_loop_cnt; k != 0; k--) {
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

          tmp = (FLOAT32)(((FLOAT64)x1r * W1) + ((FLOAT64)x1i * W4));
          x1i = (FLOAT32)(-((FLOAT64)x1r * W4) + (FLOAT64)x1i * W1);
          x1r = tmp;

          tmp = (FLOAT32)(((FLOAT64)x2r * W5) - ((FLOAT64)x2i * W2));
          x2i = (FLOAT32)(((FLOAT64)x2r * W2) + ((FLOAT64)x2i * W5));
          x2r = tmp;

          tmp = (FLOAT32)(-((FLOAT64)x3r * W3) - ((FLOAT64)x3i * W6));
          x3i = (FLOAT32)(-((FLOAT64)x3r * W6) + (FLOAT64)x3i * W3);
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
          x2r = x2r - (x3i);
          x2i = x2i + (x3r);
          x3i = x2r + (x3i * 2);
          x3r = x2i - (x3r * 2);

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
      const FLOAT64 *twiddles = ptr_w;
      nodespacing <<= 1;

      for (j = del / 2; j != 0; j--) {
        FLOAT64 W1 = *twiddles;
        FLOAT64 W4 = *(twiddles + 257);
        FLOAT32 tmp;
        twiddles += nodespacing;

        x0r = *ptr_y;
        x0i = *(ptr_y + 1);
        ptr_y += (del << 1);

        x1r = *ptr_y;
        x1i = *(ptr_y + 1);

        tmp = (FLOAT32)(((FLOAT64)x1r * W1) + ((FLOAT64)x1i * W4));
        x1i = (FLOAT32)(-((FLOAT64)x1r * W4) + (FLOAT64)x1i * W1);
        x1r = tmp;

        *ptr_y = (x0r) - (x1r);
        *(ptr_y + 1) = (x0i) - (x1i);
        ptr_y -= (del << 1);

        *ptr_y = (x0r) + (x1r);
        *(ptr_y + 1) = (x0i) + (x1i);
        ptr_y += 2;
      }
      twiddles = ptr_w;
      for (j = del / 2; j != 0; j--) {
        FLOAT64 W1 = *twiddles;
        FLOAT64 W4 = *(twiddles + 257);
        FLOAT32 tmp;
        twiddles += nodespacing;

        x0r = *ptr_y;
        x0i = *(ptr_y + 1);
        ptr_y += (del << 1);

        x1r = *ptr_y;
        x1i = *(ptr_y + 1);

        tmp = (FLOAT32)(((FLOAT64)x1r * W4) - ((FLOAT64)x1i * W1));
        x1i = (FLOAT32)(((FLOAT64)x1r * W1) + ((FLOAT64)x1i * W4));
        x1r = tmp;

        *ptr_y = (x0r) - (x1r);
        *(ptr_y + 1) = (x0i) - (x1i);
        ptr_y -= (del << 1);

        *ptr_y = (x0r) + (x1r);
        *(ptr_y + 1) = (x0i) + (x1i);
        ptr_y += 2;
      }
    }
  }

  for (i = 0; i < n_pass; i++) {
    re[2 * i + 0] = y[2 * i + 0];
    re[2 * i + 1] = y[2 * i + 1];
  }
}

VOID ixheaacd_hbe_apply_cfftn_gen(FLOAT32 re[], FLOAT32 *scratch, WORD32 n_pass,
                                  WORD32 i_sign) {
  WORD32 i, j;
  WORD32 m_points = n_pass;
  FLOAT32 *x, *y, *re3;
  FLOAT32 *ptr_x, *ptr_y;
  ptr_x = x = scratch;
  scratch += 2 * m_points;
  ptr_y = y = scratch;
  scratch += 4 * m_points;
  re3 = scratch;
  scratch += 2 * m_points;
  WORD32 cnfac;
  WORD32 mpass = n_pass;

  cnfac = 0;
  while (mpass % 3 == 0) {
    mpass /= 3;
    cnfac++;
  }

  for (i = 0; i < 3 * cnfac; i++) {
    for (j = 0; j < mpass; j++) {
      re3[2 * j + 0] = re[6 * j + 2 * i + 0];
      re3[2 * j + 1] = re[6 * j + 2 * i + 1];
    }

    ixheaacd_hbe_apply_cfftn(re3, scratch, mpass, i_sign);

    for (j = 0; j < mpass; j++) {
      re[6 * j + 2 * i + 0] = re3[2 * j + 0];
      re[6 * j + 2 * i + 1] = re3[2 * j + 1];
    }
  }

  {
    FLOAT64 *w1r, *w1i;
    FLOAT32 tmp;
    w1r = (FLOAT64 *)ixheaac_twid_tbl_fft_ntwt3r;
    w1i = (FLOAT64 *)ixheaac_twid_tbl_fft_ntwt3i;

    if (i_sign < 0) {

      for (i = 0; i < n_pass; i += 3) {
        tmp = (FLOAT32)((FLOAT64)re[2 * i + 0] * (*w1r) - (FLOAT64)re[2 * i + 1] * (*w1i));
        re[2 * i + 1] =
            (FLOAT32)((FLOAT64)re[2 * i + 0] * (*w1i) + (FLOAT64)re[2 * i + 1] * (*w1r));
        re[2 * i + 0] = tmp;

        w1r++;
        w1i++;

        tmp = (FLOAT32)((FLOAT64)re[2 * i + 2] * (*w1r) - (FLOAT64)re[2 * i + 3] * (*w1i));
        re[2 * i + 3] =
            (FLOAT32)((FLOAT64)re[2 * i + 2] * (*w1i) + (FLOAT64)re[2 * i + 3] * (*w1r));
        re[2 * i + 2] = tmp;

        w1r++;
        w1i++;

        tmp = (FLOAT32)((FLOAT64)re[2 * i + 4] * (*w1r) - (FLOAT64)re[2 * i + 5] * (*w1i));
        re[2 * i + 5] =
            (FLOAT32)((FLOAT64)re[2 * i + 4] * (*w1i) + (FLOAT64)re[2 * i + 5] * (*w1r));
        re[2 * i + 4] = tmp;

        w1r += 3 * (128 / mpass - 1) + 1;
        w1i += 3 * (128 / mpass - 1) + 1;
      }
    } else {
      for (i = 0; i < n_pass; i += 3) {
        tmp = (FLOAT32)((FLOAT64)re[2 * i + 0] * (*w1r) + (FLOAT64)re[2 * i + 1] * (*w1i));
        re[2 * i + 1] =
            (FLOAT32)(-(FLOAT64)re[2 * i + 0] * (*w1i) + (FLOAT64)re[2 * i + 1] * (*w1r));
        re[2 * i + 0] = tmp;

        w1r++;
        w1i++;

        tmp = (FLOAT32)((FLOAT64)re[2 * i + 2] * (*w1r) + (FLOAT64)re[2 * i + 3] * (*w1i));
        re[2 * i + 3] =
            (FLOAT32)(-(FLOAT64)re[2 * i + 2] * (*w1i) + (FLOAT64)re[2 * i + 3] * (*w1r));
        re[2 * i + 2] = tmp;

        w1r++;
        w1i++;

        tmp = (FLOAT32)((FLOAT64)re[2 * i + 4] * (*w1r) + (FLOAT64)re[2 * i + 5] * (*w1i));
        re[2 * i + 5] =
            (FLOAT32)(-(FLOAT64)re[2 * i + 4] * (*w1i) + (FLOAT64)re[2 * i + 5] * (*w1r));
        re[2 * i + 4] = tmp;

        w1r += 3 * (128 / mpass - 1) + 1;
        w1i += 3 * (128 / mpass - 1) + 1;
      }
    }
  }

  for (i = 0; i < n_pass; i++) {
    ptr_x[2 * i + 0] = re[2 * i + 0];
    ptr_x[2 * i + 1] = re[2 * i + 1];
  }
  for (i = 0; i < mpass; i++) {
    ixheaacd_hbe_apply_fft_3(ptr_x, ptr_y, i_sign);

    ptr_x = ptr_x + 6;
    ptr_y = ptr_y + 6;
  }

  for (i = 0; i < mpass; i++) {
    re[2 * i + 0] = y[6 * i + 0];
    re[2 * i + 1] = y[6 * i + 1];
  }

  for (i = 0; i < mpass; i++) {
    re[2 * mpass + 2 * i + 0] = y[6 * i + 2];
    re[2 * mpass + 2 * i + 1] = y[6 * i + 3];
  }

  for (i = 0; i < mpass; i++) {
    re[4 * mpass + 2 * i + 0] = y[6 * i + 4];
    re[4 * mpass + 2 * i + 1] = y[6 * i + 5];
  }
}

VOID ixheaacd_hbe_apply_fft_288(FLOAT32 *inp, FLOAT32 *scratch, WORD32 len, WORD32 i_sign) {
  FLOAT32 *op = scratch;
  WORD32 mpoints = len / 96;
  WORD32 fpoints = len / 3;
  WORD32 ii, jj;
  scratch += 2 * len;

  for (ii = 0; ii < mpoints; ii++) {
    for (jj = 0; jj < fpoints; jj++) {
      op[2 * jj + 0] = inp[2 * mpoints * jj + 2 * ii];
      op[2 * jj + 1] = inp[2 * mpoints * jj + 2 * ii + 1];
    }

    if (fpoints & (fpoints - 1))
      ixheaacd_hbe_apply_cfftn_gen(op, scratch, fpoints, i_sign);
    else
      ixheaacd_hbe_apply_cfftn(op, scratch, fpoints, i_sign);

    for (jj = 0; jj < fpoints; jj++) {
      inp[mpoints * 2 * jj + 2 * ii + 0] = op[2 * jj + 0];
      inp[mpoints * 2 * jj + 2 * ii + 1] = op[2 * jj + 1];
    }
  }

  ixheaacd_hbe_apply_tw_mult_fft(inp, op, fpoints, mpoints, ixheaac_twid_tbl_fft_288);

  for (ii = 0; ii < fpoints; ii++) {
    ixheaacd_hbe_apply_fft_3(op, scratch, i_sign);
    op = op + (mpoints * 2);
    scratch = scratch + (mpoints * 2);
  }

  scratch -= fpoints * mpoints * 2;

  for (jj = 0; jj < fpoints; jj++) {
    inp[2 * jj + 0] = scratch[6 * jj];
    inp[2 * jj + 1] = scratch[6 * jj + 1];
  }
  for (jj = 0; jj < fpoints; jj++) {
    inp[2 * fpoints + 2 * jj + 0] = scratch[6 * jj + 2];
    inp[2 * fpoints + 2 * jj + 1] = scratch[6 * jj + 3];
  }
  for (jj = 0; jj < fpoints; jj++) {
    inp[4 * fpoints + 2 * jj + 0] = scratch[6 * jj + 4];
    inp[4 * fpoints + 2 * jj + 1] = scratch[6 * jj + 5];
  }
}

VOID ixheaacd_hbe_apply_ifft_224(FLOAT32 *inp, FLOAT32 *scratch, WORD32 len, WORD32 i_sign) {
  WORD32 mpoints = len / 32;
  WORD32 fpoints = len / 7;
  WORD32 ii, jj;
  FLOAT32 *op = scratch;
  scratch += 2 * len;

  for (ii = 0; ii < mpoints; ii++) {
    for (jj = 0; jj < fpoints; jj++) {
      op[2 * jj + 0] = inp[2 * mpoints * jj + 2 * ii];
      op[2 * jj + 1] = inp[2 * mpoints * jj + 2 * ii + 1];
    }

    if (fpoints & (fpoints - 1))
      ixheaacd_hbe_apply_cfftn_gen(op, scratch, fpoints, i_sign);
    else
      ixheaacd_hbe_apply_cfftn(op, scratch, fpoints, i_sign);

    for (jj = 0; jj < fpoints; jj++) {
      inp[mpoints * 2 * jj + 2 * ii + 0] = op[2 * jj + 0];
      inp[mpoints * 2 * jj + 2 * ii + 1] = op[2 * jj + 1];
    }
  }

  ixheaacd_hbe_apply_tw_mult_ifft(inp, op, fpoints, mpoints, ixheaac_twid_tbl_fft_224);

  for (ii = 0; ii < fpoints; ii++) {
    ixheaacd_hbe_apply_ifft_7(op, scratch);
    scratch += (mpoints * 2);
    op += (mpoints * 2);
  }

  scratch -= fpoints * mpoints * 2;

  for (jj = 0; jj < fpoints; jj++) {
    for (ii = 0; ii < mpoints; ii++) {
      inp[fpoints * ii * 2 + 2 * jj + 0] = scratch[mpoints * jj * 2 + 2 * ii + 0];
      inp[fpoints * ii * 2 + 2 * jj + 1] = scratch[mpoints * jj * 2 + 2 * ii + 1];
    }
  }
}

VOID ixheaacd_hbe_apply_ifft_336(FLOAT32 *inp, FLOAT32 *ptr_scratch, WORD32 len,
                                 WORD32 i_sign) {
  WORD32 i, j;
  WORD32 m_points = len / 7;
  WORD32 n_points = len / 48;
  FLOAT32 *ptr_real, *ptr_imag, *p_real_1, *p_scratch;
  ptr_real = ptr_scratch;
  ptr_scratch += 2 * len;
  ptr_imag = ptr_scratch;
  ptr_scratch += len;
  p_scratch = ptr_scratch;
  ptr_scratch += len;
  p_real_1 = ptr_scratch;
  ptr_scratch += len;

  for (i = 0; i < len; i++) {
    ptr_real[i] = inp[2 * i + 0];
    ptr_imag[i] = inp[2 * i + 1];
  }

  for (i = 0; i < m_points; i++) {
    for (j = 0; j < n_points; j++) {
      p_real_1[2 * j + 0] = inp[m_points * 2 * j + 2 * i + 0];
      p_real_1[2 * j + 1] = inp[m_points * 2 * j + 2 * i + 1];
    }

    ixheaacd_hbe_apply_ifft_7(p_real_1, ptr_scratch);

    for (j = 0; j < n_points; j++) {
      inp[m_points * 2 * j + 2 * i + 0] = ptr_scratch[2 * j + 0];
      inp[m_points * 2 * j + 2 * i + 1] = ptr_scratch[2 * j + 1];
    }
  }

  if (m_points == 48)
    ixheaacd_hbe_apply_tw_mult_ifft(inp, p_scratch, n_points, m_points,
                                    ixheaac_twid_tbl_fft_336);
  else
    ixheaacd_hbe_apply_tw_mult_ifft(inp, p_scratch, n_points, m_points,
                                    ixheaac_twid_tbl_fft_168);

  for (i = 0; i < len; i++) {
    ptr_real[2 * i + 0] = p_scratch[2 * i + 0];
    ptr_real[2 * i + 1] = p_scratch[2 * i + 1];
  }

  for (i = 0; i < n_points; i++) {
    ixheaacd_hbe_apply_cfftn_gen(ptr_real, ptr_scratch, m_points, i_sign);
    ptr_real += (2 * m_points);
  }

  ptr_real -= n_points * 2 * m_points;

  for (j = 0; j < n_points; j++) {
    for (i = 0; i < m_points; i++) {
      inp[n_points * 2 * i + 2 * j + 0] = ptr_real[2 * m_points * j + 2 * i + 0];
      inp[n_points * 2 * i + 2 * j + 1] = ptr_real[2 * m_points * j + 2 * i + 1];
    }
  }
  return;
}

