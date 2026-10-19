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

#include <stdlib.h>
#include <ixheaac_type_def.h>
#include "ixheaac_constants.h"
#include "ixheaace_constants.h"
#include "iusace_basic_ops_flt.h"
#include "ixheaace_common_utils.h"
#include "ixheaac_fft_ifft_rom.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaac_basic_ops.h"

#define DIG_REV(i, m, j)                                    \
  do {                                                      \
    unsigned _ = (i);                                       \
    _ = ((_ & 0x33333333) << 2) | ((_ & ~0x33333333) >> 2); \
    _ = ((_ & 0x0F0F0F0F) << 4) | ((_ & ~0x0F0F0F0F) >> 4); \
    _ = ((_ & 0x00FF00FF) << 8) | ((_ & ~0x00FF00FF) >> 8); \
    (j) = _ >> (m);                                         \
  } while (0)

#define CPLX_MPY_FFT(re, im, a, b, c, d) \
  do {                                   \
    re = ((a * c) - (b * d));            \
    im = ((a * d) + (b * c));            \
  } while (0)

#define CPLX_MPY_IFFT(re, im, a, b, c, d) \
  do {                                    \
    re = ((a * c) + (b * d));             \
    im = (-(a * d) + (b * c));            \
  } while (0)

VOID ixheaace_hbe_apply_ifft_7(FLOAT32 *ptr_inp, FLOAT32 *ptr_op) {
  FLOAT32 x0r, x1r, x2r, x3r, x4r, x5r, x6r, x7r, x8r;
  FLOAT32 x0i, x1i, x2i, x3i, x4i, x5i, x6i, x7i, x8i;
  FLOAT32 y0r, y1r, y2r, y3r, y4r, y5r, y6r, y7r, y8r;
  FLOAT32 y0i, y1i, y2i, y3i, y4i, y5i, y6i, y7i, y8i;

  /*
   * Node 1 of Winograd FFT for 7 point
   *
   * 1   0   0   0   0   0   0
   * 0   1   0   0   0   0   1
   * 0   1   0   0   0   0  -1
   * 0   0   1   0   0   1   0
   * 0   0   1   0   0  -1   0
   * 0   0   0   1   1   0   0
   * 0   0   0  -1   1   0   0
   *
   */

  x0r = ptr_inp[0];
  x0i = ptr_inp[1];
  x1r = ptr_inp[2] + ptr_inp[12];
  x1i = ptr_inp[3] + ptr_inp[13];
  x2r = ptr_inp[2] - ptr_inp[12];
  x2i = ptr_inp[3] - ptr_inp[13];
  x3r = ptr_inp[4] + ptr_inp[10];
  x3i = ptr_inp[5] + ptr_inp[11];
  x4r = ptr_inp[4] - ptr_inp[10];
  x4i = ptr_inp[5] - ptr_inp[11];
  x5r = ptr_inp[8] + ptr_inp[6];
  x5i = ptr_inp[9] + ptr_inp[7];
  x6r = ptr_inp[8] - ptr_inp[6];
  x6i = ptr_inp[9] - ptr_inp[7];

  /*
   * Node 2 of Winograd FFT for 7 point
   *
   * 1   0   0   0   0   0   0
   * 0   1   0   1   0   1   0
   * 0   1   0  -1   0   0   0
   * 0  -1   0   0   0   1   0
   * 0   0   0   1   0  -1   0
   * 0   0   1   0   1   0   1
   * 0   0   1   0  -1   0   0
   * 0   0  -1   0   0   0   1
   * 0   0   0   0   1   0  -1
   *
   */

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

  /*
   * Node 3 of Winograd FFT for 7 point
   *
   * 1    1    0    0    0     0     0     0     0
   * 1  c70    0    0    0     0     0     0     0
   * 0    0  c71    0    0     0     0     0     0
   * 0    0    0  c72    0     0     0     0     0
   * 0    0    0    0  c73     0     0     0     0
   * 0    0    0    0    0  jc74     0     0     0
   * 0    0    0    0    0     0  jc75     0     0
   * 0    0    0    0    0     0     0  jc76     0
   * 0    0    0    0    0     0     0     0  jc77
   *
   */
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

  /*
   * Node 4 of Winograd FFT for 7 point
   *
   * 1   0   0   0   0   0   0   0   0
   * 0   1   1   0   1   0   0   0   0
   * 0   1  -1  -1   0   0   0   0   0
   * 0   1   0   1  -1   0   0   0   0
   * 0   0   0   0   0   1   1   0   1
   * 0   0   0   0   0   1  -1  -1   0
   * 0   0   0   0   0   1   0   1  -1
   *
   */

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

  /*
   * Node 5 of Winograd FFT for 7 point
   *
   * 1   0   0   0   0   0   0
   * 0   1   0   0   1   0   0
   * 0   0   0   1   0   0   1
   * 0   0   1   0   0  -1   0
   * 0   0   1   0   0   1   0
   * 0   0   0   1   0   0  -1
   * 0   1   0   0  -1   0   0
   *
   */
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

  ptr_op[0] = x0r;
  ptr_op[1] = x0i;
  ptr_op[2] = x1r;
  ptr_op[3] = x1i;
  ptr_op[4] = x2r;
  ptr_op[5] = x2i;
  ptr_op[6] = x3r;
  ptr_op[7] = x3i;
  ptr_op[8] = x4r;
  ptr_op[9] = x4i;
  ptr_op[10] = x5r;
  ptr_op[11] = x5i;
  ptr_op[12] = x6r;
  ptr_op[13] = x6i;
}

VOID ixheaace_hbe_apply_fft_3(FLOAT32 *ptr_inp, FLOAT32 *ptr_op, WORD32 i_sign) {
  FLOAT32 add_r, sub_r;
  FLOAT32 add_i, sub_i;
  FLOAT32 x_01_r, x_01_i, temp;

  FLOAT32 p1, p2, p3, p4;

  /* mu = PI / 3; The cos and sin values are in Q31
     cosmu is 0.5 so used >> 1 instead of multiplication */

  FLOAT64 sinmu;
  sinmu = -0.866025403784439 * (FLOAT64)i_sign;

  x_01_r = ptr_inp[0] + ptr_inp[2];
  x_01_i = ptr_inp[1] + ptr_inp[3];

  add_r = ptr_inp[2] + ptr_inp[4];
  add_i = ptr_inp[3] + ptr_inp[5];

  sub_r = ptr_inp[2] - ptr_inp[4];
  sub_i = ptr_inp[3] - ptr_inp[5];

  p1 = add_r / (FLOAT32)2.0;
  p4 = add_i / (FLOAT32)2.0;
  p2 = (FLOAT32)((FLOAT64)sub_i * sinmu);
  p3 = (FLOAT32)((FLOAT64)sub_r * sinmu);

  temp = ptr_inp[0] - p1;

  ptr_op[0] = x_01_r + ptr_inp[4];
  ptr_op[1] = x_01_i + ptr_inp[5];
  ptr_op[2] = temp + p2;
  ptr_op[3] = (ptr_inp[1] - p3) - p4;
  ptr_op[4] = temp - p2;
  ptr_op[5] = (ptr_inp[1] + p3) - p4;
}

VOID ixheaace_hbe_apply_tw_mult_ifft(FLOAT32 *ptr_inp, FLOAT32 *ptr_op, WORD32 dim1, WORD32 dim2,
                                     const FLOAT32 *ptr_tw) {
  FLOAT32 accu1, accu2;
  WORD32 i, j;
  WORD32 step_val = (dim2 - 1) << 1;
  for (i = 0; i < (dim2); i++) {
    ptr_op[0] = ptr_inp[0];
    ptr_op[1] = ptr_inp[1];
    ptr_op += 2;
    ptr_inp += 2;
  }

  for (j = 0; j < (dim1 - 1); j++) {
    ptr_op[0] = ptr_inp[0];
    ptr_op[1] = ptr_inp[1];
    ptr_inp += 2;
    ptr_op += 2;
    for (i = 0; i < (dim2 - 1); i++) {
      CPLX_MPY_IFFT(accu1, accu2, ptr_inp[2 * i + 0], ptr_inp[2 * i + 1], ptr_tw[2 * i + 1],
                    ptr_tw[2 * i]);
      ptr_op[2 * i + 0] = accu1;
      ptr_op[2 * i + 1] = accu2;
    }
    ptr_inp += step_val;
    ptr_op += step_val;
    ptr_tw += (dim2 - 1) * 2;
  }
}

VOID ixheaace_hbe_apply_tw_mult_fft(FLOAT32 *ptr_inp, FLOAT32 *ptr_op, WORD32 dim1, WORD32 dim2,
                                    const FLOAT32 *ptr_tw) {
  FLOAT32 accu1, accu2;
  WORD32 i, j;
  WORD32 step_val = (dim2 - 1) << 1;
  for (i = 0; i < (dim2); i++) {
    ptr_op[0] = ptr_inp[0];
    ptr_op[1] = ptr_inp[1];
    ptr_op += 2;
    ptr_inp += 2;
  }

  for (j = 0; j < (dim1 - 1); j++) {
    ptr_op[0] = ptr_inp[0];
    ptr_op[1] = ptr_inp[1];
    ptr_inp += 2;
    ptr_op += 2;
    for (i = 0; i < (dim2 - 1); i++) {
      CPLX_MPY_FFT(accu1, accu2, ptr_inp[2 * i + 0], ptr_inp[2 * i + 1], ptr_tw[2 * i + 1],
                   ptr_tw[2 * i]);
      ptr_op[2 * i + 0] = accu1;
      ptr_op[2 * i + 1] = accu2;
    }
    ptr_inp += step_val;
    ptr_op += step_val;
    ptr_tw += (dim2 - 1) * 2;
  }
}

VOID ixheaace_hbe_apply_cfftn(FLOAT32 re[], FLOAT32 *ptr_scratch, WORD32 n_pass, WORD32 i_sign) {
  WORD32 i, j, k, n_stages, h2;
  FLOAT32 x0r, x0i, x1r, x1i, x2r, x2i, x3r, x3i;
  WORD32 del, nodespacing, in_loop_cnt;
  WORD32 not_power_4;
  WORD32 dig_rev_shift;
  WORD32 mpass = n_pass;
  WORD32 npoints = n_pass;
  const FLOAT64 *ptr_w;
  FLOAT32 *ptr_x = ptr_scratch;
  FLOAT32 *y = ptr_scratch + (2 * n_pass);
  FLOAT32 *ptr_y = y;

  dig_rev_shift = ixheaac_norm32(mpass) + 1 - 16;
  n_stages = 30 - ixheaac_norm32(mpass); /* log2(npoints), if npoints=2^m */
  not_power_4 = n_stages & 1;

  n_stages = n_stages >> 1;

  ptr_w = ixheaac_twid_tbl_fft_double;
  ptr_x = re;

  dig_rev_shift = MAX(dig_rev_shift, 0);

  if (i_sign == -1) {
    for (i = 0; i < npoints; i += 4) {
      FLOAT32 *ptr_inp = ptr_x;
      FLOAT32 tmk;

      DIG_REV(i, dig_rev_shift, h2);
      if (not_power_4) {
        h2 += 1;
        h2 &= ~1;
      }
      ptr_inp += (h2);

      x0r = *ptr_inp;
      x0i = *(ptr_inp + 1);
      ptr_inp += (npoints >> 1);

      x1r = *ptr_inp;
      x1i = *(ptr_inp + 1);
      ptr_inp += (npoints >> 1);

      x2r = *ptr_inp;
      x2i = *(ptr_inp + 1);
      ptr_inp += (npoints >> 1);

      x3r = *ptr_inp;
      x3i = *(ptr_inp + 1);

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
      const FLOAT64 *ptr_twiddle = ptr_w;
      FLOAT32 *data = ptr_y;
      FLOAT64 w_1, w_2, w_3, w_4, w_5, w_6;
      WORD32 sec_loop_cnt;

      for (k = in_loop_cnt; k != 0; k--) {
        x0r = (*data);
        x0i = (*(data + 1));
        data += ((SIZE_T)del << 1);

        x1r = (*data);
        x1i = (*(data + 1));
        data += ((SIZE_T)del << 1);

        x2r = (*data);
        x2i = (*(data + 1));
        data += ((SIZE_T)del << 1);

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
        data += ((SIZE_T)del << 1);

        *data = x2r;
        *(data + 1) = x2i;
        data += ((SIZE_T)del << 1);

        *data = x1r;
        *(data + 1) = x1i;
        data += ((SIZE_T)del << 1);

        *data = x3i;
        *(data + 1) = x3r;
        data += ((SIZE_T)del << 1);
      }
      data = ptr_y + 2;

      sec_loop_cnt = (nodespacing * del);
      sec_loop_cnt = (sec_loop_cnt / 4) + (sec_loop_cnt / 8) - (sec_loop_cnt / 16) +
                     (sec_loop_cnt / 32) - (sec_loop_cnt / 64) + (sec_loop_cnt / 128) -
                     (sec_loop_cnt / 256);

      for (j = nodespacing; j <= sec_loop_cnt; j += nodespacing) {
        w_1 = *(ptr_twiddle + j);
        w_4 = *(ptr_twiddle + j + 257);
        w_2 = *(ptr_twiddle + ((SIZE_T)j << 1));
        w_5 = *(ptr_twiddle + ((SIZE_T)j << 1) + 257);
        w_3 = *(ptr_twiddle + j + ((SIZE_T)j << 1));
        w_6 = *(ptr_twiddle + j + ((SIZE_T)j << 1) + 257);

        for (k = in_loop_cnt; k != 0; k--) {
          FLOAT32 tmp;
          FLOAT32 x0r1, x0i1, x1r1, x1i1, x2r1, x2i1, x3r1, x3i1;
          /*x0 is loaded later to avoid register crunch*/

          data += ((SIZE_T)del << 1);

          x1r1 = *data;
          x1i1 = *(data + 1);
          data += ((SIZE_T)del << 1);

          x2r1 = *data;
          x2i1 = *(data + 1);
          data += ((SIZE_T)del << 1);

          x3r1 = *data;
          x3i1 = *(data + 1);
          data -= 3 * (del << 1);

          tmp =
              (FLOAT32)(ixheaace_dmult((FLOAT64)x1r1, w_1) - ixheaace_dmult((FLOAT64)x1i1, w_4));
          x1i1 = (FLOAT32)ixheaace_dmac(ixheaace_dmult((FLOAT64)x1r1, w_4), (FLOAT64)x1i1, w_1);
          x1r1 = tmp;

          tmp =
              (FLOAT32)(ixheaace_dmult((FLOAT64)x2r1, w_2) - ixheaace_dmult((FLOAT64)x2i1, w_5));
          x2i1 = (FLOAT32)ixheaace_dmac(ixheaace_dmult((FLOAT64)x2r1, w_5), (FLOAT64)x2i1, w_2);
          x2r1 = tmp;

          tmp =
              (FLOAT32)(ixheaace_dmult((FLOAT64)x3r1, w_3) - ixheaace_dmult((FLOAT64)x3i1, w_6));
          x3i1 = (FLOAT32)ixheaace_dmac(ixheaace_dmult((FLOAT64)x3r1, w_6), (FLOAT64)x3i1, w_3);
          x3r1 = tmp;

          x0r1 = (*data);
          x0i1 = (*(data + 1));

          x0r1 = x0r1 + (x2r1);
          x0i1 = x0i1 + (x2i1);
          x2r1 = x0r1 - (x2r1 * 2);
          x2i1 = x0i1 - (x2i1 * 2);
          x1r1 = x1r1 + x3r1;
          x1i1 = x1i1 + x3i1;
          x3r1 = x1r1 - (x3r1 * 2);
          x3i1 = x1i1 - (x3i1 * 2);

          x0r1 = x0r1 + (x1r1);
          x0i1 = x0i1 + (x1i1);
          x1r1 = x0r1 - (x1r1 * 2);
          x1i1 = x0i1 - (x1i1 * 2);
          x2r1 = x2r1 + (x3i1);
          x2i1 = x2i1 - (x3r1);
          x3i1 = x2r1 - (x3i1 * 2);
          x3r1 = x2i1 + (x3r1 * 2);

          *data = x0r1;
          *(data + 1) = x0i1;
          data += ((SIZE_T)del << 1);

          *data = x2r1;
          *(data + 1) = x2i1;
          data += ((SIZE_T)del << 1);

          *data = x1r1;
          *(data + 1) = x1i1;
          data += ((SIZE_T)del << 1);

          *data = x3i1;
          *(data + 1) = x3r1;
          data += ((SIZE_T)del << 1);
        }
        data -= 2 * npoints;
        data += 2;
      }
      for (; j <= (nodespacing * del) >> 1; j += nodespacing) {
        w_1 = *(ptr_twiddle + j);
        w_4 = *(ptr_twiddle + j + 257);
        w_2 = *(ptr_twiddle + ((SIZE_T)j << 1));
        w_5 = *(ptr_twiddle + ((SIZE_T)j << 1) + 257);
        w_3 = *(ptr_twiddle + j + ((SIZE_T)j << 1) - 256);
        w_6 = *(ptr_twiddle + j + ((SIZE_T)j << 1) + 1);

        for (k = in_loop_cnt; k != 0; k--) {
          FLOAT32 tmp;
          FLOAT32 x0r1, x0i1, x1r1, x1i1, x2r1, x2i1, x3r1, x3i1;
          /*x0 is loaded later to avoid register crunch*/

          data += ((SIZE_T)del << 1);

          x1r1 = *data;
          x1i1 = *(data + 1);
          data += ((SIZE_T)del << 1);

          x2r1 = *data;
          x2i1 = *(data + 1);
          data += ((SIZE_T)del << 1);

          x3r1 = *data;
          x3i1 = *(data + 1);
          data -= 3 * (del << 1);

          tmp =
              (FLOAT32)(ixheaace_dmult((FLOAT64)x1r1, w_1) - ixheaace_dmult((FLOAT64)x1i1, w_4));
          x1i1 = (FLOAT32)ixheaace_dmac(ixheaace_dmult((FLOAT64)x1r1, w_4), (FLOAT64)x1i1, w_1);
          x1r1 = tmp;

          tmp =
              (FLOAT32)(ixheaace_dmult((FLOAT64)x2r1, w_2) - ixheaace_dmult((FLOAT64)x2i1, w_5));
          x2i1 = (FLOAT32)ixheaace_dmac(ixheaace_dmult((FLOAT64)x2r1, w_5), (FLOAT64)x2i1, w_2);
          x2r1 = tmp;

          tmp =
              (FLOAT32)(ixheaace_dmult((FLOAT64)x3r1, w_6) + ixheaace_dmult((FLOAT64)x3i1, w_3));
          x3i1 =
              (FLOAT32)(-ixheaace_dmult((FLOAT64)x3r1, w_3) + ixheaace_dmult((FLOAT64)x3i1, w_6));
          x3r1 = tmp;

          x0r1 = (*data);
          x0i1 = (*(data + 1));

          x0r1 = x0r1 + (x2r1);
          x0i1 = x0i1 + (x2i1);
          x2r1 = x0r1 - (x2r1 * 2);
          x2i1 = x0i1 - (x2i1 * 2);
          x1r1 = x1r1 + x3r1;
          x1i1 = x1i1 + x3i1;
          x3r1 = x1r1 - (x3r1 * 2);
          x3i1 = x1i1 - (x3i1 * 2);

          x0r1 = x0r1 + (x1r1);
          x0i1 = x0i1 + (x1i1);
          x1r1 = x0r1 - (x1r1 * 2);
          x1i1 = x0i1 - (x1i1 * 2);
          x2r1 = x2r1 + (x3i1);
          x2i1 = x2i1 - (x3r1);
          x3i1 = x2r1 - (x3i1 * 2);
          x3r1 = x2i1 + (x3r1 * 2);

          *data = x0r1;
          *(data + 1) = x0i1;
          data += ((SIZE_T)del << 1);

          *data = x2r1;
          *(data + 1) = x2i1;
          data += ((SIZE_T)del << 1);

          *data = x1r1;
          *(data + 1) = x1i1;
          data += ((SIZE_T)del << 1);

          *data = x3i1;
          *(data + 1) = x3r1;
          data += ((SIZE_T)del << 1);
        }
        data -= 2 * npoints;
        data += 2;
      }
      for (; j <= sec_loop_cnt * 2; j += nodespacing) {
        w_1 = *(ptr_twiddle + j);
        w_4 = *(ptr_twiddle + j + 257);
        w_2 = *(ptr_twiddle + ((SIZE_T)j << 1) - 256);
        w_5 = *(ptr_twiddle + ((SIZE_T)j << 1) + 1);
        w_3 = *(ptr_twiddle + j + ((SIZE_T)j << 1) - 256);
        w_6 = *(ptr_twiddle + j + ((SIZE_T)j << 1) + 1);

        for (k = in_loop_cnt; k != 0; k--) {
          FLOAT32 tmp;
          FLOAT32 x0r1, x0i1, x1r1, x1i1, x2r1, x2i1, x3r1, x3i1;
          /*x0 is loaded later to avoid register crunch*/

          data += ((SIZE_T)del << 1);

          x1r1 = *data;
          x1i1 = *(data + 1);
          data += ((SIZE_T)del << 1);

          x2r1 = *data;
          x2i1 = *(data + 1);
          data += ((SIZE_T)del << 1);

          x3r1 = *data;
          x3i1 = *(data + 1);
          data -= 3 * (del << 1);

          tmp =
              (FLOAT32)(ixheaace_dmult((FLOAT64)x1r1, w_1) - ixheaace_dmult((FLOAT64)x1i1, w_4));
          x1i1 = (FLOAT32)ixheaace_dmac(ixheaace_dmult(x1r1, w_4), x1i1, w_1);
          x1r1 = tmp;

          tmp =
              (FLOAT32)(ixheaace_dmult((FLOAT64)x2r1, w_5) + ixheaace_dmult((FLOAT64)x2i1, w_2));
          x2i1 = (FLOAT32)(-ixheaace_dmult(x2r1, w_2) + ixheaace_dmult(x2i1, w_5));
          x2r1 = tmp;

          tmp =
              (FLOAT32)(ixheaace_dmult((FLOAT64)x3r1, w_6) + ixheaace_dmult((FLOAT64)x3i1, w_3));
          x3i1 =
              (FLOAT32)(-ixheaace_dmult((FLOAT64)x3r1, w_3) + ixheaace_dmult((FLOAT64)x3i1, w_6));
          x3r1 = tmp;

          x0r1 = (*data);
          x0i1 = (*(data + 1));

          x0r1 = x0r1 + (x2r1);
          x0i1 = x0i1 + (x2i1);
          x2r1 = x0r1 - (x2r1 * 2);
          x2i1 = x0i1 - (x2i1 * 2);
          x1r1 = x1r1 + x3r1;
          x1i1 = x1i1 + x3i1;
          x3r1 = x1r1 - (x3r1 * 2);
          x3i1 = x1i1 - (x3i1 * 2);

          x0r1 = x0r1 + (x1r1);
          x0i1 = x0i1 + (x1i1);
          x1r1 = x0r1 - (x1r1 * 2);
          x1i1 = x0i1 - (x1i1 * 2);
          x2r1 = x2r1 + (x3i1);
          x2i1 = x2i1 - (x3r1);
          x3i1 = x2r1 - (x3i1 * 2);
          x3r1 = x2i1 + (x3r1 * 2);

          *data = x0r1;
          *(data + 1) = x0i1;
          data += ((SIZE_T)del << 1);

          *data = x2r1;
          *(data + 1) = x2i1;
          data += ((SIZE_T)del << 1);

          *data = x1r1;
          *(data + 1) = x1i1;
          data += ((SIZE_T)del << 1);

          *data = x3i1;
          *(data + 1) = x3r1;
          data += ((SIZE_T)del << 1);
        }
        data -= 2 * npoints;
        data += 2;
      }
      for (; j < nodespacing * del; j += nodespacing) {
        w_1 = *(ptr_twiddle + j);
        w_4 = *(ptr_twiddle + j + 257);
        w_2 = *(ptr_twiddle + ((SIZE_T)j << 1) - 256);
        w_5 = *(ptr_twiddle + ((SIZE_T)j << 1) + 1);
        w_3 = *(ptr_twiddle + j + ((SIZE_T)j << 1) - 512);
        w_6 = *(ptr_twiddle + j + ((SIZE_T)j << 1) - 512 + 257);

        for (k = in_loop_cnt; k != 0; k--) {
          FLOAT32 tmp;
          FLOAT32 x0r1, x0i1, x1r1, x1i1, x2r1, x2i1, x3r1, x3i1;
          /*x0 is loaded later to avoid register crunch*/

          data += ((SIZE_T)del << 1);

          x1r1 = *data;
          x1i1 = *(data + 1);
          data += ((SIZE_T)del << 1);

          x2r1 = *data;
          x2i1 = *(data + 1);
          data += ((SIZE_T)del << 1);

          x3r1 = *data;
          x3i1 = *(data + 1);
          data -= 3 * (del << 1);

          tmp =
              (FLOAT32)(ixheaace_dmult((FLOAT64)x1r1, w_1) - ixheaace_dmult((FLOAT64)x1i1, w_4));
          x1i1 = (FLOAT32)ixheaace_dmac(ixheaace_dmult((FLOAT64)x1r1, w_4), (FLOAT64)x1i1, w_1);
          x1r1 = tmp;

          tmp =
              (FLOAT32)(ixheaace_dmult((FLOAT64)x2r1, w_5) + ixheaace_dmult((FLOAT64)x2i1, w_2));
          x2i1 =
              (FLOAT32)(-ixheaace_dmult((FLOAT64)x2r1, w_2) + ixheaace_dmult((FLOAT64)x2i1, w_5));
          x2r1 = tmp;

          tmp =
              (FLOAT32)(-ixheaace_dmult((FLOAT64)x3r1, w_3) + ixheaace_dmult((FLOAT64)x3i1, w_6));
          x3i1 = (FLOAT32)ixheaace_dmac(ixheaace_dmult((FLOAT64)x3r1, w_6), (FLOAT64)x3i1, w_3);
          x3r1 = tmp;

          x0r1 = (*data);
          x0i1 = (*(data + 1));

          x0r1 = x0r1 + (x2r1);
          x0i1 = x0i1 + (x2i1);
          x2r1 = x0r1 - (x2r1 * 2);
          x2i1 = x0i1 - (x2i1 * 2);
          x1r1 = x1r1 + x3r1;
          x1i1 = x1i1 - x3i1;
          x3r1 = x1r1 - (x3r1 * 2);
          x3i1 = x1i1 + (x3i1 * 2);

          x0r1 = x0r1 + (x1r1);
          x0i1 = x0i1 + (x1i1);
          x1r1 = x0r1 - (x1r1 * 2);
          x1i1 = x0i1 - (x1i1 * 2);
          x2r1 = x2r1 + (x3i1);
          x2i1 = x2i1 - (x3r1);
          x3i1 = x2r1 - (x3i1 * 2);
          x3r1 = x2i1 + (x3r1 * 2);

          *data = x0r1;
          *(data + 1) = x0i1;
          data += ((SIZE_T)del << 1);

          *data = x2r1;
          *(data + 1) = x2i1;
          data += ((SIZE_T)del << 1);

          *data = x1r1;
          *(data + 1) = x1i1;
          data += ((SIZE_T)del << 1);

          *data = x3i1;
          *(data + 1) = x3r1;
          data += ((SIZE_T)del << 1);
        }
        data -= 2 * npoints;
        data += 2;
      }
      nodespacing >>= 2;
      del <<= 2;
      in_loop_cnt >>= 2;
    }
    if (not_power_4) {
      const double *ptr_twiddle = ptr_w;
      nodespacing <<= 1;

      for (j = del / 2; j != 0; j--) {
        FLOAT64 w_1 = *ptr_twiddle;
        FLOAT64 w_4 = *(ptr_twiddle + 257);
        FLOAT32 tmp;
        ptr_twiddle += nodespacing;

        x0r = *ptr_y;
        x0i = *(ptr_y + 1);
        ptr_y += ((SIZE_T)del << 1);

        x1r = *ptr_y;
        x1i = *(ptr_y + 1);

        tmp = (FLOAT32)(ixheaace_dmult((FLOAT64)x1r, w_1) - ixheaace_dmult((FLOAT64)x1i, w_4));
        x1i = (FLOAT32)ixheaace_dmac(ixheaace_dmult((FLOAT64)x1r, w_4), (FLOAT64)x1i, w_1);
        x1r = tmp;

        *ptr_y = (x0r) - (x1r);
        *(ptr_y + 1) = (x0i) - (x1i);
        ptr_y -= ((SIZE_T)del << 1);

        *ptr_y = (x0r) + (x1r);
        *(ptr_y + 1) = (x0i) + (x1i);
        ptr_y += 2;
      }
      ptr_twiddle = ptr_w;
      for (j = del / 2; j != 0; j--) {
        FLOAT64 w_1 = *ptr_twiddle;
        FLOAT64 w_4 = *(ptr_twiddle + 257);
        FLOAT32 tmp;
        ptr_twiddle += nodespacing;

        x0r = *ptr_y;
        x0i = *(ptr_y + 1);
        ptr_y += ((SIZE_T)del << 1);

        x1r = *ptr_y;
        x1i = *(ptr_y + 1);

        tmp = (FLOAT32)(ixheaace_dmult((FLOAT64)x1r, w_4) + ixheaace_dmult((FLOAT64)x1i, w_1));
        x1i = (FLOAT32)(-ixheaace_dmult((FLOAT64)x1r, w_1) + ixheaace_dmult((FLOAT64)x1i, w_4));
        x1r = tmp;

        *ptr_y = (x0r) - (x1r);
        *(ptr_y + 1) = (x0i) - (x1i);
        ptr_y -= ((SIZE_T)del << 1);

        *ptr_y = (x0r) + (x1r);
        *(ptr_y + 1) = (x0i) + (x1i);
        ptr_y += 2;
      }
    }
  }

  /**********************IFFT******************************************/

  else {
    for (i = 0; i < npoints; i += 4) {
      FLOAT32 *ptr_inp = ptr_x;

      DIG_REV(i, dig_rev_shift, h2);
      if (not_power_4) {
        h2 += 1;
        h2 &= ~1;
      }
      ptr_inp += (h2);

      x0r = *ptr_inp;
      x0i = *(ptr_inp + 1);
      ptr_inp += (npoints >> 1);

      x1r = *ptr_inp;
      x1i = *(ptr_inp + 1);
      ptr_inp += (npoints >> 1);

      x2r = *ptr_inp;
      x2i = *(ptr_inp + 1);
      ptr_inp += (npoints >> 1);

      x3r = *ptr_inp;
      x3i = *(ptr_inp + 1);

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
      const double *ptr_twiddle = ptr_w;
      float *data = ptr_y;
      double w_1, w_2, w_3, w_4, w_5, w_6;
      int sec_loop_cnt;

      for (k = in_loop_cnt; k != 0; k--) {
        x0r = (*data);
        x0i = (*(data + 1));
        data += ((SIZE_T)del << 1);

        x1r = (*data);
        x1i = (*(data + 1));
        data += ((SIZE_T)del << 1);

        x2r = (*data);
        x2i = (*(data + 1));
        data += ((SIZE_T)del << 1);

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
        data += ((SIZE_T)del << 1);

        *data = x2r;
        *(data + 1) = x2i;
        data += ((SIZE_T)del << 1);

        *data = x1r;
        *(data + 1) = x1i;
        data += ((SIZE_T)del << 1);

        *data = x3i;
        *(data + 1) = x3r;
        data += ((SIZE_T)del << 1);
      }
      data = ptr_y + 2;

      sec_loop_cnt = (nodespacing * del);
      sec_loop_cnt = (sec_loop_cnt / 4) + (sec_loop_cnt / 8) - (sec_loop_cnt / 16) +
                     (sec_loop_cnt / 32) - (sec_loop_cnt / 64) + (sec_loop_cnt / 128) -
                     (sec_loop_cnt / 256);

      for (j = nodespacing; j <= sec_loop_cnt; j += nodespacing) {
        w_1 = *(ptr_twiddle + j);
        w_4 = *(ptr_twiddle + j + 257);
        w_2 = *(ptr_twiddle + ((SIZE_T)j << 1));
        w_5 = *(ptr_twiddle + ((SIZE_T)j << 1) + 257);
        w_3 = *(ptr_twiddle + j + ((SIZE_T)j << 1));
        w_6 = *(ptr_twiddle + j + ((SIZE_T)j << 1) + 257);

        for (k = in_loop_cnt; k != 0; k--) {
          FLOAT32 tmp;
          FLOAT32 x0r1, x0i1, x1r1, x1i1, x2r1, x2i1, x3r1, x3i1;
          /*x0 is loaded later to avoid register crunch*/

          data += ((SIZE_T)del << 1);

          x1r1 = *data;
          x1i1 = *(data + 1);
          data += ((SIZE_T)del << 1);

          x2r1 = *data;
          x2i1 = *(data + 1);
          data += ((SIZE_T)del << 1);

          x3r1 = *data;
          x3i1 = *(data + 1);
          data -= 3 * (del << 1);

          tmp = (FLOAT32)(((FLOAT64)x1r1 * w_1) + ((FLOAT64)x1i1 * w_4));
          x1i1 = (FLOAT32)(-((FLOAT64)x1r1 * w_4) + (FLOAT64)x1i1 * w_1);
          x1r1 = tmp;

          tmp = (FLOAT32)(((FLOAT64)x2r1 * w_2) + ((FLOAT64)x2i1 * w_5));
          x2i1 = (FLOAT32)(-((FLOAT64)x2r1 * w_5) + (FLOAT64)x2i1 * w_2);
          x2r1 = tmp;

          tmp = (FLOAT32)(((FLOAT64)x3r1 * w_3) + ((FLOAT64)x3i1 * w_6));
          x3i1 = (FLOAT32)(-((FLOAT64)x3r1 * w_6) + (FLOAT64)x3i1 * w_3);
          x3r1 = tmp;

          x0r1 = (*data);
          x0i1 = (*(data + 1));

          x0r1 = x0r1 + (x2r1);
          x0i1 = x0i1 + (x2i1);
          x2r1 = x0r1 - (x2r1 * 2);
          x2i1 = x0i1 - (x2i1 * 2);
          x1r1 = x1r1 + x3r1;
          x1i1 = x1i1 + x3i1;
          x3r1 = x1r1 - (x3r1 * 2);
          x3i1 = x1i1 - (x3i1 * 2);

          x0r1 = x0r1 + (x1r1);
          x0i1 = x0i1 + (x1i1);
          x1r1 = x0r1 - (x1r1 * 2);
          x1i1 = x0i1 - (x1i1 * 2);
          x2r1 = x2r1 - (x3i1);
          x2i1 = x2i1 + (x3r1);
          x3i1 = x2r1 + (x3i1 * 2);
          x3r1 = x2i1 - (x3r1 * 2);

          *data = x0r1;
          *(data + 1) = x0i1;
          data += ((SIZE_T)del << 1);

          *data = x2r1;
          *(data + 1) = x2i1;
          data += ((SIZE_T)del << 1);

          *data = x1r1;
          *(data + 1) = x1i1;
          data += ((SIZE_T)del << 1);

          *data = x3i1;
          *(data + 1) = x3r1;
          data += ((SIZE_T)del << 1);
        }
        data -= 2 * npoints;
        data += 2;
      }
      for (; j <= (nodespacing * del) >> 1; j += nodespacing) {
        w_1 = *(ptr_twiddle + j);
        w_4 = *(ptr_twiddle + j + 257);
        w_2 = *(ptr_twiddle + ((SIZE_T)j << 1));
        w_5 = *(ptr_twiddle + ((SIZE_T)j << 1) + 257);
        w_3 = *(ptr_twiddle + j + ((SIZE_T)j << 1) - 256);
        w_6 = *(ptr_twiddle + j + ((SIZE_T)j << 1) + 1);

        for (k = in_loop_cnt; k != 0; k--) {
          FLOAT32 tmp;
          FLOAT32 x0r1, x0i1, x1r1, x1i1, x2r1, x2i1, x3r1, x3i1;
          /*x0 is loaded later to avoid register crunch*/

          data += ((SIZE_T)del << 1);

          x1r1 = *data;
          x1i1 = *(data + 1);
          data += ((SIZE_T)del << 1);

          x2r1 = *data;
          x2i1 = *(data + 1);
          data += ((SIZE_T)del << 1);

          x3r1 = *data;
          x3i1 = *(data + 1);
          data -= 3 * (del << 1);

          tmp = (FLOAT32)(((FLOAT64)x1r1 * w_1) + ((FLOAT64)x1i1 * w_4));
          x1i1 = (FLOAT32)(-((FLOAT64)x1r1 * w_4) + (FLOAT64)x1i1 * w_1);
          x1r1 = tmp;

          tmp = (FLOAT32)(((FLOAT64)x2r1 * w_2) + ((FLOAT64)x2i1 * w_5));
          x2i1 = (FLOAT32)(-((FLOAT64)x2r1 * w_5) + (FLOAT64)x2i1 * w_2);
          x2r1 = tmp;

          tmp = (FLOAT32)(((FLOAT64)x3r1 * w_6) - ((FLOAT64)x3i1 * w_3));
          x3i1 = (FLOAT32)(((FLOAT64)x3r1 * w_3) + ((FLOAT64)x3i1 * w_6));
          x3r1 = tmp;

          x0r1 = (*data);
          x0i1 = (*(data + 1));

          x0r1 = x0r1 + (x2r1);
          x0i1 = x0i1 + (x2i1);
          x2r1 = x0r1 - (x2r1 * 2);
          x2i1 = x0i1 - (x2i1 * 2);
          x1r1 = x1r1 + x3r1;
          x1i1 = x1i1 + x3i1;
          x3r1 = x1r1 - (x3r1 * 2);
          x3i1 = x1i1 - (x3i1 * 2);

          x0r1 = x0r1 + (x1r1);
          x0i1 = x0i1 + (x1i1);
          x1r1 = x0r1 - (x1r1 * 2);
          x1i1 = x0i1 - (x1i1 * 2);
          x2r1 = x2r1 - (x3i1);
          x2i1 = x2i1 + (x3r1);
          x3i1 = x2r1 + (x3i1 * 2);
          x3r1 = x2i1 - (x3r1 * 2);

          *data = x0r1;
          *(data + 1) = x0i1;
          data += ((SIZE_T)del << 1);

          *data = x2r1;
          *(data + 1) = x2i1;
          data += ((SIZE_T)del << 1);

          *data = x1r1;
          *(data + 1) = x1i1;
          data += ((SIZE_T)del << 1);

          *data = x3i1;
          *(data + 1) = x3r1;
          data += ((SIZE_T)del << 1);
        }
        data -= 2 * npoints;
        data += 2;
      }
      for (; j <= sec_loop_cnt * 2; j += nodespacing) {
        w_1 = *(ptr_twiddle + j);
        w_4 = *(ptr_twiddle + j + 257);
        w_2 = *(ptr_twiddle + ((SIZE_T)j << 1) - 256);
        w_5 = *(ptr_twiddle + ((SIZE_T)j << 1) + 1);
        w_3 = *(ptr_twiddle + j + ((SIZE_T)j << 1) - 256);
        w_6 = *(ptr_twiddle + j + ((SIZE_T)j << 1) + 1);

        for (k = in_loop_cnt; k != 0; k--) {
          FLOAT32 tmp;
          FLOAT32 x0r1, x0i1, x1r1, x1i1, x2r1, x2i1, x3r1, x3i1;
          /*x0 is loaded later to avoid register crunch*/

          data += ((SIZE_T)del << 1);

          x1r1 = *data;
          x1i1 = *(data + 1);
          data += ((SIZE_T)del << 1);

          x2r1 = *data;
          x2i1 = *(data + 1);
          data += ((SIZE_T)del << 1);

          x3r1 = *data;
          x3i1 = *(data + 1);
          data -= 3 * (del << 1);

          tmp = (FLOAT32)(((FLOAT64)x1r1 * w_1) + ((FLOAT64)x1i1 * w_4));
          x1i1 = (FLOAT32)(-((FLOAT64)x1r1 * w_4) + (FLOAT64)x1i1 * w_1);
          x1r1 = tmp;

          tmp = (FLOAT32)(((FLOAT64)x2r1 * w_5) - ((FLOAT64)x2i1 * w_2));
          x2i1 = (FLOAT32)(((FLOAT64)x2r1 * w_2) + ((FLOAT64)x2i1 * w_5));
          x2r1 = tmp;

          tmp = (FLOAT32)(((FLOAT64)x3r1 * w_6) - ((FLOAT64)x3i1 * w_3));
          x3i1 = (FLOAT32)(((FLOAT64)x3r1 * w_3) + ((FLOAT64)x3i1 * w_6));
          x3r1 = tmp;

          x0r1 = (*data);
          x0i1 = (*(data + 1));

          x0r1 = x0r1 + (x2r1);
          x0i1 = x0i1 + (x2i1);
          x2r1 = x0r1 - (x2r1 * 2);
          x2i1 = x0i1 - (x2i1 * 2);
          x1r1 = x1r1 + x3r1;
          x1i1 = x1i1 + x3i1;
          x3r1 = x1r1 - (x3r1 * 2);
          x3i1 = x1i1 - (x3i1 * 2);

          x0r1 = x0r1 + (x1r1);
          x0i1 = x0i1 + (x1i1);
          x1r1 = x0r1 - (x1r1 * 2);
          x1i1 = x0i1 - (x1i1 * 2);
          x2r1 = x2r1 - (x3i1);
          x2i1 = x2i1 + (x3r1);
          x3i1 = x2r1 + (x3i1 * 2);
          x3r1 = x2i1 - (x3r1 * 2);

          *data = x0r1;
          *(data + 1) = x0i1;
          data += ((SIZE_T)del << 1);

          *data = x2r1;
          *(data + 1) = x2i1;
          data += ((SIZE_T)del << 1);

          *data = x1r1;
          *(data + 1) = x1i1;
          data += ((SIZE_T)del << 1);

          *data = x3i1;
          *(data + 1) = x3r1;
          data += ((SIZE_T)del << 1);
        }
        data -= 2 * npoints;
        data += 2;
      }
      for (; j < nodespacing * del; j += nodespacing) {
        w_1 = *(ptr_twiddle + j);
        w_4 = *(ptr_twiddle + j + 257);
        w_2 = *(ptr_twiddle + ((SIZE_T)j << 1) - 256);
        w_5 = *(ptr_twiddle + ((SIZE_T)j << 1) + 1);
        w_3 = *(ptr_twiddle + j + ((SIZE_T)j << 1) - 512);
        w_6 = *(ptr_twiddle + j + ((SIZE_T)j << 1) - 512 + 257);

        for (k = in_loop_cnt; k != 0; k--) {
          FLOAT32 tmp;
          FLOAT32 x0r1, x0i1, x1r1, x1i1, x2r1, x2i1, x3r1, x3i1;
          /*x0 is loaded later to avoid register crunch*/

          data += ((SIZE_T)del << 1);

          x1r1 = *data;
          x1i1 = *(data + 1);
          data += ((SIZE_T)del << 1);

          x2r1 = *data;
          x2i1 = *(data + 1);
          data += ((SIZE_T)del << 1);

          x3r1 = *data;
          x3i1 = *(data + 1);
          data -= 3 * (del << 1);

          tmp = (FLOAT32)(((FLOAT64)x1r1 * w_1) + ((FLOAT64)x1i1 * w_4));
          x1i1 = (FLOAT32)(-((FLOAT64)x1r1 * w_4) + (FLOAT64)x1i1 * w_1);
          x1r1 = tmp;

          tmp = (FLOAT32)(((FLOAT64)x2r1 * w_5) - ((FLOAT64)x2i1 * w_2));
          x2i1 = (FLOAT32)(((FLOAT64)x2r1 * w_2) + ((FLOAT64)x2i1 * w_5));
          x2r1 = tmp;

          tmp = (FLOAT32)(-((FLOAT64)x3r1 * w_3) - ((FLOAT64)x3i1 * w_6));
          x3i1 = (FLOAT32)(-((FLOAT64)x3r1 * w_6) + (FLOAT64)x3i1 * w_3);
          x3r1 = tmp;

          x0r1 = (*data);
          x0i1 = (*(data + 1));

          x0r1 = x0r1 + (x2r1);
          x0i1 = x0i1 + (x2i1);
          x2r1 = x0r1 - (x2r1 * 2);
          x2i1 = x0i1 - (x2i1 * 2);
          x1r1 = x1r1 + x3r1;
          x1i1 = x1i1 - x3i1;
          x3r1 = x1r1 - (x3r1 * 2);
          x3i1 = x1i1 + (x3i1 * 2);

          x0r1 = x0r1 + (x1r1);
          x0i1 = x0i1 + (x1i1);
          x1r1 = x0r1 - (x1r1 * 2);
          x1i1 = x0i1 - (x1i1 * 2);
          x2r1 = x2r1 - (x3i1);
          x2i1 = x2i1 + (x3r1);
          x3i1 = x2r1 + (x3i1 * 2);
          x3r1 = x2i1 - (x3r1 * 2);

          *data = x0r1;
          *(data + 1) = x0i1;
          data += ((SIZE_T)del << 1);

          *data = x2r1;
          *(data + 1) = x2i1;
          data += ((SIZE_T)del << 1);

          *data = x1r1;
          *(data + 1) = x1i1;
          data += ((SIZE_T)del << 1);

          *data = x3i1;
          *(data + 1) = x3r1;
          data += ((SIZE_T)del << 1);
        }
        data -= 2 * npoints;
        data += 2;
      }
      nodespacing >>= 2;
      del <<= 2;
      in_loop_cnt >>= 2;
    }

    if (not_power_4) {
      const FLOAT64 *ptr_twiddle = ptr_w;
      nodespacing <<= 1;

      for (j = del / 2; j != 0; j--) {
        FLOAT64 w_1 = *ptr_twiddle;
        FLOAT64 w_4 = *(ptr_twiddle + 257);
        FLOAT32 tmp;
        ptr_twiddle += nodespacing;

        x0r = *ptr_y;
        x0i = *(ptr_y + 1);
        ptr_y += ((SIZE_T)del << 1);

        x1r = *ptr_y;
        x1i = *(ptr_y + 1);

        tmp = (FLOAT32)(((FLOAT64)x1r * w_1) + ((FLOAT64)x1i * w_4));
        x1i = (FLOAT32)(-((FLOAT64)x1r * w_4) + (FLOAT64)x1i * w_1);
        x1r = tmp;

        *ptr_y = (x0r) - (x1r);
        *(ptr_y + 1) = (x0i) - (x1i);
        ptr_y -= ((SIZE_T)del << 1);

        *ptr_y = (x0r) + (x1r);
        *(ptr_y + 1) = (x0i) + (x1i);
        ptr_y += 2;
      }
      ptr_twiddle = ptr_w;
      for (j = del / 2; j != 0; j--) {
        FLOAT64 w_1 = *ptr_twiddle;
        FLOAT64 w_4 = *(ptr_twiddle + 257);
        FLOAT32 tmp;
        ptr_twiddle += nodespacing;

        x0r = *ptr_y;
        x0i = *(ptr_y + 1);
        ptr_y += ((SIZE_T)del << 1);

        x1r = *ptr_y;
        x1i = *(ptr_y + 1);

        tmp = (FLOAT32)(((FLOAT64)x1r * w_4) - ((FLOAT64)x1i * w_1));
        x1i = (FLOAT32)(((FLOAT64)x1r * w_1) + ((FLOAT64)x1i * w_4));
        x1r = tmp;

        *ptr_y = (x0r) - (x1r);
        *(ptr_y + 1) = (x0i) - (x1i);
        ptr_y -= ((SIZE_T)del << 1);

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

VOID ixheaace_hbe_apply_cfftn_gen(FLOAT32 in[], FLOAT32 *ptr_scratch, WORD32 n_pass,
                                  WORD32 i_sign) {
  WORD32 i, j;
  WORD32 m_points = n_pass;
  FLOAT32 *y, *re_3;
  FLOAT32 *ptr_x, *ptr_y;
  ptr_x = ptr_scratch;
  ptr_scratch += 2 * m_points;
  ptr_y = y = ptr_scratch;
  ptr_scratch += 4 * m_points;
  re_3 = ptr_scratch;
  ptr_scratch += 2 * m_points;
  WORD32 cnfac;
  WORD32 mpass = n_pass;

  cnfac = 0;
  while (mpass % 3 == 0) {
    mpass /= 3;
    cnfac++;
  }

  for (i = 0; i < 3 * cnfac; i++) {
    for (j = 0; j < mpass; j++) {
      re_3[2 * j + 0] = in[6 * j + 2 * i + 0];
      re_3[2 * j + 1] = in[6 * j + 2 * i + 1];
    }

    ixheaace_hbe_apply_cfftn(re_3, ptr_scratch, mpass, i_sign);

    for (j = 0; j < mpass; j++) {
      in[6 * j + 2 * i + 0] = re_3[2 * j + 0];
      in[6 * j + 2 * i + 1] = re_3[2 * j + 1];
    }
  }

  {
    FLOAT64 *ptr_w1r, *ptr_w1i;
    FLOAT32 tmp;
    ptr_w1r = (FLOAT64 *)ixheaac_twid_tbl_fft_ntwt3r;
    ptr_w1i = (FLOAT64 *)ixheaac_twid_tbl_fft_ntwt3i;

    if (i_sign < 0) {
      i = 0;
      while (i < n_pass) {
        tmp =
            (FLOAT32)((FLOAT64)in[2 * i + 0] * (*ptr_w1r) - (FLOAT64)in[2 * i + 1] * (*ptr_w1i));
        in[2 * i + 1] =
            (FLOAT32)((FLOAT64)in[2 * i + 0] * (*ptr_w1i) + (FLOAT64)in[2 * i + 1] * (*ptr_w1r));
        in[2 * i + 0] = tmp;

        ptr_w1r++;
        ptr_w1i++;

        tmp =
            (FLOAT32)((FLOAT64)in[2 * i + 2] * (*ptr_w1r) - (FLOAT64)in[2 * i + 3] * (*ptr_w1i));
        in[2 * i + 3] =
            (FLOAT32)((FLOAT64)in[2 * i + 2] * (*ptr_w1i) + (FLOAT64)in[2 * i + 3] * (*ptr_w1r));
        in[2 * i + 2] = tmp;

        ptr_w1r++;
        ptr_w1i++;

        tmp =
            (FLOAT32)((FLOAT64)in[2 * i + 4] * (*ptr_w1r) - (FLOAT64)in[2 * i + 5] * (*ptr_w1i));
        in[2 * i + 5] =
            (FLOAT32)((FLOAT64)in[2 * i + 4] * (*ptr_w1i) + (FLOAT64)in[2 * i + 5] * (*ptr_w1r));
        in[2 * i + 4] = tmp;

        ptr_w1r += 3 * (128 / mpass - 1) + 1;
        ptr_w1i += 3 * (128 / mpass - 1) + 1;
        i += 3;
      }
    }

    else {
      i = 0;
      while (i < n_pass) {
        tmp =
            (FLOAT32)((FLOAT64)in[2 * i + 0] * (*ptr_w1r) + (FLOAT64)in[2 * i + 1] * (*ptr_w1i));
        in[2 * i + 1] =
            (FLOAT32)(-(FLOAT64)in[2 * i + 0] * (*ptr_w1i) + (FLOAT64)in[2 * i + 1] * (*ptr_w1r));
        in[2 * i + 0] = tmp;

        ptr_w1r++;
        ptr_w1i++;

        tmp =
            (FLOAT32)((FLOAT64)in[2 * i + 2] * (*ptr_w1r) + (FLOAT64)in[2 * i + 3] * (*ptr_w1i));
        in[2 * i + 3] =
            (FLOAT32)(-(FLOAT64)in[2 * i + 2] * (*ptr_w1i) + (FLOAT64)in[2 * i + 3] * (*ptr_w1r));
        in[2 * i + 2] = tmp;

        ptr_w1r++;
        ptr_w1i++;

        tmp =
            (FLOAT32)((FLOAT64)in[2 * i + 4] * (*ptr_w1r) + (FLOAT64)in[2 * i + 5] * (*ptr_w1i));
        in[2 * i + 5] =
            (FLOAT32)(-(FLOAT64)in[2 * i + 4] * (*ptr_w1i) + (FLOAT64)in[2 * i + 5] * (*ptr_w1r));
        in[2 * i + 4] = tmp;

        ptr_w1r += 3 * (128 / mpass - 1) + 1;
        ptr_w1i += 3 * (128 / mpass - 1) + 1;
        i += 3;
      }
    }
  }

  for (i = 0; i < n_pass; i++) {
    ptr_x[2 * i + 0] = in[2 * i + 0];
    ptr_x[2 * i + 1] = in[2 * i + 1];
  }
  for (i = 0; i < mpass; i++) {
    ixheaace_hbe_apply_fft_3(ptr_x, ptr_y, i_sign);

    ptr_x = ptr_x + 6;
    ptr_y = ptr_y + 6;
  }

  for (i = 0; i < mpass; i++) {
    in[2 * i + 0] = y[6 * i + 0];
    in[2 * i + 1] = y[6 * i + 1];
  }

  for (i = 0; i < mpass; i++) {
    in[2 * mpass + 2 * i + 0] = y[6 * i + 2];
    in[2 * mpass + 2 * i + 1] = y[6 * i + 3];
  }

  for (i = 0; i < mpass; i++) {
    in[4 * mpass + 2 * i + 0] = y[6 * i + 4];
    in[4 * mpass + 2 * i + 1] = y[6 * i + 5];
  }
}

VOID ixheaace_hbe_apply_fft_288(FLOAT32 *ptr_inp, FLOAT32 *ptr_scratch, WORD32 len,
                                WORD32 i_sign) {
  /* Dividing the 288-point FFT into 96x3 i.e nx3*/
  FLOAT32 *ptr_op = ptr_scratch;
  WORD32 mpoints = len / 96;
  WORD32 fpoints = len / 3;
  WORD32 ii, jj;
  ptr_scratch += 2 * len;

  for (ii = 0; ii < mpoints; ii++) {
    for (jj = 0; jj < fpoints; jj++) {
      ptr_op[2 * jj + 0] = ptr_inp[2 * mpoints * jj + 2 * ii];
      ptr_op[2 * jj + 1] = ptr_inp[2 * mpoints * jj + 2 * ii + 1];
    }

    /* 96-point (32x3-point) of FFT */
    if (fpoints & (fpoints - 1))
      ixheaace_hbe_apply_cfftn_gen(ptr_op, ptr_scratch, fpoints, i_sign);
    else
      ixheaace_hbe_apply_cfftn(ptr_op, ptr_scratch, fpoints, i_sign);

    for (jj = 0; jj < fpoints; jj++) {
      ptr_inp[mpoints * 2 * jj + 2 * ii + 0] = ptr_op[2 * jj + 0];
      ptr_inp[mpoints * 2 * jj + 2 * ii + 1] = ptr_op[2 * jj + 1];
    }
  }

  /* Multiplication FFT with twiddle table */
  ixheaace_hbe_apply_tw_mult_fft(ptr_inp, ptr_op, fpoints, mpoints, ixheaac_twid_tbl_fft_288);

  for (ii = 0; ii < fpoints; ii++) {
    /* 3-point of FFT */
    ixheaace_hbe_apply_fft_3(ptr_op, ptr_scratch, i_sign);
    ptr_op = ptr_op + (mpoints * 2);
    ptr_scratch = ptr_scratch + (mpoints * 2);
  }

  ptr_scratch -= fpoints * mpoints * 2;

  for (jj = 0; jj < fpoints; jj++) {
    ptr_inp[2 * jj + 0] = ptr_scratch[6 * jj];
    ptr_inp[2 * jj + 1] = ptr_scratch[6 * jj + 1];
  }
  for (jj = 0; jj < fpoints; jj++) {
    ptr_inp[2 * fpoints + 2 * jj + 0] = ptr_scratch[6 * jj + 2];
    ptr_inp[2 * fpoints + 2 * jj + 1] = ptr_scratch[6 * jj + 3];
  }
  for (jj = 0; jj < fpoints; jj++) {
    ptr_inp[4 * fpoints + 2 * jj + 0] = ptr_scratch[6 * jj + 4];
    ptr_inp[4 * fpoints + 2 * jj + 1] = ptr_scratch[6 * jj + 5];
  }
}

VOID ixheaace_hbe_apply_ifft_224(FLOAT32 *ptr_inp, FLOAT32 *ptr_scratch, WORD32 len,
                                 WORD32 i_sign) {
  /* Dividing 224-point IFFT into 32x7 */
  WORD32 mpoints = len / 32;
  WORD32 fpoints = len / 7;
  WORD32 ii, jj;
  FLOAT32 *ptr_op = ptr_scratch;
  ptr_scratch += 2 * len;

  for (ii = 0; ii < mpoints; ii++) {
    for (jj = 0; jj < fpoints; jj++) {
      ptr_op[2 * jj + 0] = ptr_inp[2 * mpoints * jj + 2 * ii];
      ptr_op[2 * jj + 1] = ptr_inp[2 * mpoints * jj + 2 * ii + 1];
    }

    /* 32-point of IFFT*/
    if (fpoints & (fpoints - 1))
      ixheaace_hbe_apply_cfftn_gen(ptr_op, ptr_scratch, fpoints, i_sign);
    else
      ixheaace_hbe_apply_cfftn(ptr_op, ptr_scratch, fpoints, i_sign);

    for (jj = 0; jj < fpoints; jj++) {
      ptr_inp[mpoints * 2 * jj + 2 * ii + 0] = ptr_op[2 * jj + 0];
      ptr_inp[mpoints * 2 * jj + 2 * ii + 1] = ptr_op[2 * jj + 1];
    }
  }

  /* Multiplication IFFT with twiddle table */
  ixheaace_hbe_apply_tw_mult_ifft(ptr_inp, ptr_op, fpoints, mpoints, ixheaac_twid_tbl_fft_224);

  for (ii = 0; ii < fpoints; ii++) {
    /* 7-point of IFFT */
    ixheaace_hbe_apply_ifft_7(ptr_op, ptr_scratch);
    ptr_scratch += (mpoints * 2);
    ptr_op += (mpoints * 2);
  }

  ptr_scratch -= fpoints * mpoints * 2;

  for (jj = 0; jj < fpoints; jj++) {
    for (ii = 0; ii < mpoints; ii++) {
      ptr_inp[fpoints * ii * 2 + 2 * jj + 0] = ptr_scratch[mpoints * jj * 2 + 2 * ii + 0];
      ptr_inp[fpoints * ii * 2 + 2 * jj + 1] = ptr_scratch[mpoints * jj * 2 + 2 * ii + 1];
    }
  }
}

VOID ixheaace_hbe_apply_ifft_336(FLOAT32 *ptr_inp, FLOAT32 *ptr_scratch, WORD32 len,
                                 WORD32 i_sign) {
  WORD32 i, j;
  WORD32 m_points = len / 7;
  WORD32 n_points = len / 48;
  FLOAT32 *ptr_real, *ptr_imag, *ptr_real_1, *ptr_scratch_local;
  ptr_real = ptr_scratch;
  ptr_scratch += 2 * len;
  ptr_imag = ptr_scratch;
  ptr_scratch += len;
  ptr_scratch_local = ptr_scratch;
  ptr_scratch += len;
  ptr_real_1 = ptr_scratch;
  ptr_scratch += len;

  for (i = 0; i < len; i++) {
    ptr_real[i] = ptr_inp[2 * i + 0];
    ptr_imag[i] = ptr_inp[2 * i + 1];
  }

  for (i = 0; i < m_points; i++) {
    for (j = 0; j < n_points; j++) {
      ptr_real_1[2 * j + 0] = ptr_inp[m_points * 2 * j + 2 * i + 0];
      ptr_real_1[2 * j + 1] = ptr_inp[m_points * 2 * j + 2 * i + 1];
    }

    ixheaace_hbe_apply_ifft_7(ptr_real_1, ptr_scratch);

    for (j = 0; j < n_points; j++) {
      ptr_inp[m_points * 2 * j + 2 * i + 0] = ptr_scratch[2 * j + 0];
      ptr_inp[m_points * 2 * j + 2 * i + 1] = ptr_scratch[2 * j + 1];
    }
  }

  switch (m_points) {
    case 48:
      ixheaace_hbe_apply_tw_mult_ifft(ptr_inp, ptr_scratch_local, n_points, m_points,
                                      ixheaac_twid_tbl_fft_336);
      break;

    default:
      ixheaace_hbe_apply_tw_mult_ifft(ptr_inp, ptr_scratch_local, n_points, m_points,
                                      ixheaac_twid_tbl_fft_168);
      break;
  }
  for (i = 0; i < len; i++) {
    ptr_real[2 * i + 0] = ptr_scratch_local[2 * i + 0];
    ptr_real[2 * i + 1] = ptr_scratch_local[2 * i + 1];
  }

  for (i = 0; i < n_points; i++) {
    ixheaace_hbe_apply_cfftn_gen(ptr_real, ptr_scratch, m_points, i_sign);
    ptr_real += (2 * m_points);
  }

  ptr_real -= n_points * 2 * m_points;

  for (j = 0; j < n_points; j++) {
    for (i = 0; i < m_points; i++) {
      ptr_inp[n_points * 2 * i + 2 * j + 0] = ptr_real[2 * m_points * j + 2 * i + 0];
      ptr_inp[n_points * 2 * i + 2 * j + 1] = ptr_real[2 * m_points * j + 2 * i + 1];
    }
  }
}
