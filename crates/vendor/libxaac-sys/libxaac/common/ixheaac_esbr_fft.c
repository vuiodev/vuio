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

#include <stdio.h>
#include <stdlib.h>
#include "ixheaac_type_def.h"
#include "ixheaac_constants.h"
#include "ixheaac_basic_ops32.h"

#define PLATFORM_INLINE __inline

#define DIG_REV(i, m, j)                                    \
  do {                                                      \
    unsigned _ = (i);                                       \
    _ = ((_ & 0x33333333) << 2) | ((_ & ~0x33333333) >> 2); \
    _ = ((_ & 0x0F0F0F0F) << 4) | ((_ & ~0x0F0F0F0F) >> 4); \
    _ = ((_ & 0x00FF00FF) << 8) | ((_ & ~0x00FF00FF) >> 8); \
    (j) = _ >> (m);                                         \
  } while (0)

extern const FLOAT32 ixheaac_twiddle_table_fft_float[514];
extern const FLOAT32 ixheaac_twidle_tbl_48[64];
extern const FLOAT32 ixheaac_twidle_tbl_24[32];

void ixheaac_real_synth_fft_p2(FLOAT32 *ptr_x, FLOAT32 *ptr_y, WORD32 npoints) {
  WORD32 i, j, k, n_stages, h2;
  FLOAT32 x0r, x0i, x1r, x1i, x2r, x2i, x3r, x3i;
  WORD32 del, nodespacing, in_loop_cnt;
  WORD32 not_power_4;
  WORD32 dig_rev_shift;
  const FLOAT32 *ptr_w;

  dig_rev_shift = ixheaac_norm32(npoints) + 1 - 16;
  n_stages = 30 - ixheaac_norm32(npoints);
  not_power_4 = n_stages & 1;

  n_stages = n_stages >> 1;

  ptr_w = ixheaac_twiddle_table_fft_float;

  for (i = 0; i < npoints; i += 4) {
    FLOAT32 *inp = ptr_x;

    DIG_REV(i, dig_rev_shift, h2);
    if (not_power_4) {
      h2 += 1;
      h2 &= ~1;
    }
    inp += (h2 >> 1);

    x0r = *inp;
    inp += (npoints >> 2);

    x1r = *inp;
    inp += (npoints >> 2);

    x2r = *inp;
    inp += (npoints >> 2);

    x3r = *inp;

    x0r = x0r + x2r;
    x2r = x0r - (x2r * 2);
    x1r = x1r + x3r;
    x3r = x1r - (x3r * 2);
    x0r = x0r + x1r;
    x1r = x0r - (x1r * 2);

    *ptr_y++ = x0r;
    *ptr_y++ = 0;
    *ptr_y++ = x2r;
    *ptr_y++ = x3r;
    *ptr_y++ = x1r;
    *ptr_y++ = 0;
    *ptr_y++ = x2r;
    *ptr_y++ = -x3r;
  }
  ptr_y -= 2 * npoints;
  del = 4;
  nodespacing = 64;
  in_loop_cnt = npoints >> 4;
  for (i = n_stages - 1; i > 0; i--) {
    const FLOAT32 *twiddles = ptr_w;
    FLOAT32 *data = ptr_y;
    FLOAT32 W1, W2, W3, W4, W5, W6;
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
      data -= 3 * ((SIZE_T)del << 1);

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
      W1 = *(twiddles + j);
      W4 = *(twiddles + j + 257);
      W2 = *(twiddles + ((SIZE_T)j << 1));
      W5 = *(twiddles + ((SIZE_T)j << 1) + 257);
      W3 = *(twiddles + j + ((SIZE_T)j << 1));
      W6 = *(twiddles + j + ((SIZE_T)j << 1) + 257);

      for (k = in_loop_cnt; k != 0; k--) {
        FLOAT32 tmp;
        FLOAT32 x0r, x0i, x1r, x1i, x2r, x2i, x3r, x3i;

        data += ((SIZE_T)del << 1);

        x1r = *data;
        x1i = *(data + 1);
        data += ((SIZE_T)del << 1);

        x2r = *data;
        x2i = *(data + 1);
        data += ((SIZE_T)del << 1);

        x3r = *data;
        x3i = *(data + 1);
        data -= 3 * ((SIZE_T)del << 1);

        tmp = (FLOAT32)(((FLOAT32)x1r * W1) + ((FLOAT32)x1i * W4));
        x1i = (FLOAT32)(-((FLOAT32)x1r * W4) + (FLOAT32)x1i * W1);
        x1r = tmp;

        tmp = (FLOAT32)(((FLOAT32)x2r * W2) + ((FLOAT32)x2i * W5));
        x2i = (FLOAT32)(-((FLOAT32)x2r * W5) + (FLOAT32)x2i * W2);
        x2r = tmp;

        tmp = (FLOAT32)(((FLOAT32)x3r * W3) + ((FLOAT32)x3i * W6));
        x3i = (FLOAT32)(-((FLOAT32)x3r * W6) + (FLOAT32)x3i * W3);
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
      data -= 2 * npoints;
      data += 2;
    }
    for (; j <= (nodespacing * del) >> 1; j += nodespacing) {
      W1 = *(twiddles + j);
      W4 = *(twiddles + j + 257);
      W2 = *(twiddles + ((SIZE_T)j << 1));
      W5 = *(twiddles + ((SIZE_T)j << 1) + 257);
      W3 = *(twiddles + j + ((SIZE_T)j << 1) - 256);
      W6 = *(twiddles + j + ((SIZE_T)j << 1) + 1);

      for (k = in_loop_cnt; k != 0; k--) {
        FLOAT32 tmp;
        FLOAT32 x0r, x0i, x1r, x1i, x2r, x2i, x3r, x3i;

        data += ((SIZE_T)del << 1);

        x1r = *data;
        x1i = *(data + 1);
        data += ((SIZE_T)del << 1);

        x2r = *data;
        x2i = *(data + 1);
        data += ((SIZE_T)del << 1);

        x3r = *data;
        x3i = *(data + 1);
        data -= 3 * ((SIZE_T)del << 1);

        tmp = (FLOAT32)(((FLOAT32)x1r * W1) + ((FLOAT32)x1i * W4));
        x1i = (FLOAT32)(-((FLOAT32)x1r * W4) + (FLOAT32)x1i * W1);
        x1r = tmp;

        tmp = (FLOAT32)(((FLOAT32)x2r * W2) + ((FLOAT32)x2i * W5));
        x2i = (FLOAT32)(-((FLOAT32)x2r * W5) + (FLOAT32)x2i * W2);
        x2r = tmp;

        tmp = (FLOAT32)(((FLOAT32)x3r * W6) - ((FLOAT32)x3i * W3));
        x3i = (FLOAT32)(((FLOAT32)x3r * W3) + ((FLOAT32)x3i * W6));
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
      data -= 2 * npoints;
      data += 2;
    }
    for (; j <= sec_loop_cnt * 2; j += nodespacing) {
      W1 = *(twiddles + j);
      W4 = *(twiddles + j + 257);
      W2 = *(twiddles + ((SIZE_T)j << 1) - 256);
      W5 = *(twiddles + ((SIZE_T)j << 1) + 1);
      W3 = *(twiddles + j + ((SIZE_T)j << 1) - 256);
      W6 = *(twiddles + j + ((SIZE_T)j << 1) + 1);

      for (k = in_loop_cnt; k != 0; k--) {
        FLOAT32 tmp;
        FLOAT32 x0r, x0i, x1r, x1i, x2r, x2i, x3r, x3i;

        data += ((SIZE_T)del << 1);

        x1r = *data;
        x1i = *(data + 1);
        data += ((SIZE_T)del << 1);

        x2r = *data;
        x2i = *(data + 1);
        data += ((SIZE_T)del << 1);

        x3r = *data;
        x3i = *(data + 1);
        data -= 3 * ((SIZE_T)del << 1);

        tmp = (FLOAT32)(((FLOAT32)x1r * W1) + ((FLOAT32)x1i * W4));
        x1i = (FLOAT32)(-((FLOAT32)x1r * W4) + (FLOAT32)x1i * W1);
        x1r = tmp;

        tmp = (FLOAT32)(((FLOAT32)x2r * W5) - ((FLOAT32)x2i * W2));
        x2i = (FLOAT32)(((FLOAT32)x2r * W2) + ((FLOAT32)x2i * W5));
        x2r = tmp;

        tmp = (FLOAT32)(((FLOAT32)x3r * W6) - ((FLOAT32)x3i * W3));
        x3i = (FLOAT32)(((FLOAT32)x3r * W3) + ((FLOAT32)x3i * W6));
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
      data -= 2 * npoints;
      data += 2;
    }
    for (; j < nodespacing * del; j += nodespacing) {
      W1 = *(twiddles + j);
      W4 = *(twiddles + j + 257);
      W2 = *(twiddles + ((SIZE_T)j << 1) - 256);
      W5 = *(twiddles + ((SIZE_T)j << 1) + 1);
      W3 = *(twiddles + j + ((SIZE_T)j << 1) - 512);
      W6 = *(twiddles + j + ((SIZE_T)j << 1) - 512 + 257);

      for (k = in_loop_cnt; k != 0; k--) {
        FLOAT32 tmp;
        FLOAT32 x0r, x0i, x1r, x1i, x2r, x2i, x3r, x3i;

        data += ((SIZE_T)del << 1);

        x1r = *data;
        x1i = *(data + 1);
        data += ((SIZE_T)del << 1);

        x2r = *data;
        x2i = *(data + 1);
        data += ((SIZE_T)del << 1);

        x3r = *data;
        x3i = *(data + 1);
        data -= 3 * ((SIZE_T)del << 1);

        tmp = (FLOAT32)(((FLOAT32)x1r * W1) + ((FLOAT32)x1i * W4));
        x1i = (FLOAT32)(-((FLOAT32)x1r * W4) + (FLOAT32)x1i * W1);
        x1r = tmp;

        tmp = (FLOAT32)(((FLOAT32)x2r * W5) - ((FLOAT32)x2i * W2));
        x2i = (FLOAT32)(((FLOAT32)x2r * W2) + ((FLOAT32)x2i * W5));
        x2r = tmp;

        tmp = (FLOAT32)(-((FLOAT32)x3r * W3) - ((FLOAT32)x3i * W6));
        x3i = (FLOAT32)(-((FLOAT32)x3r * W6) + (FLOAT32)x3i * W3);
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
      data -= 2 * npoints;
      data += 2;
    }
    nodespacing >>= 2;
    del <<= 2;
    in_loop_cnt >>= 2;
  }

  if (not_power_4) {
    const FLOAT32 *twiddles = ptr_w;
    nodespacing <<= 1;

    for (j = del / 2; j != 0; j--) {
      FLOAT32 W1 = *twiddles;
      FLOAT32 W4 = *(twiddles + 257);
      FLOAT32 tmp;
      twiddles += nodespacing;

      x0r = *ptr_y;
      x0i = *(ptr_y + 1);
      ptr_y += ((SIZE_T)del << 1);

      x1r = *ptr_y;
      x1i = *(ptr_y + 1);

      tmp = (FLOAT32)(((FLOAT32)x1r * W1) + ((FLOAT32)x1i * W4));
      x1i = (FLOAT32)(-((FLOAT32)x1r * W4) + (FLOAT32)x1i * W1);
      x1r = tmp;

      *ptr_y = (x0r) - (x1r);
      *(ptr_y + 1) = (x0i) - (x1i);
      ptr_y -= ((SIZE_T)del << 1);

      *ptr_y = (x0r) + (x1r);
      *(ptr_y + 1) = (x0i) + (x1i);
      ptr_y += 2;
    }
    twiddles = ptr_w;
    for (j = del / 2; j != 0; j--) {
      FLOAT32 W1 = *twiddles;
      FLOAT32 W4 = *(twiddles + 257);
      FLOAT32 tmp;
      twiddles += nodespacing;

      x0r = *ptr_y;
      x0i = *(ptr_y + 1);
      ptr_y += ((SIZE_T)del << 1);

      x1r = *ptr_y;
      x1i = *(ptr_y + 1);
      tmp = (FLOAT32)(((FLOAT32)x1r * W4) - ((FLOAT32)x1i * W1));
      x1i = (FLOAT32)(((FLOAT32)x1r * W1) + ((FLOAT32)x1i * W4));
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

void ixheaac_cmplx_anal_fft_p2(FLOAT32 *ptr_x, FLOAT32 *ptr_y, WORD32 npoints) {
  WORD32 i, j, k, n_stages, h2;
  FLOAT32 x0r, x0i, x1r, x1i, x2r, x2i, x3r, x3i;
  WORD32 del, nodespacing, in_loop_cnt;
  WORD32 not_power_4;
  WORD32 dig_rev_shift;
  const FLOAT32 *ptr_w;

  dig_rev_shift = ixheaac_norm32(npoints) + 1 - 16;
  n_stages = 30 - ixheaac_norm32(npoints);
  not_power_4 = n_stages & 1;

  n_stages = n_stages >> 1;

  ptr_w = ixheaac_twiddle_table_fft_float;

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
    const FLOAT32 *twiddles = ptr_w;
    FLOAT32 *data = ptr_y;
    FLOAT32 W1, W2, W3, W4, W5, W6;
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
      data -= 3 * ((SIZE_T)del << 1);

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
      W1 = *(twiddles + j);
      W4 = *(twiddles + j + 257);
      W2 = *(twiddles + ((SIZE_T)j << 1));
      W5 = *(twiddles + ((SIZE_T)j << 1) + 257);
      W3 = *(twiddles + j + ((SIZE_T)j << 1));
      W6 = *(twiddles + j + ((SIZE_T)j << 1) + 257);

      for (k = in_loop_cnt; k != 0; k--) {
        FLOAT32 tmp;
        FLOAT32 x0r, x0i, x1r, x1i, x2r, x2i, x3r, x3i;

        data += ((SIZE_T)del << 1);

        x1r = *data;
        x1i = *(data + 1);
        data += ((SIZE_T)del << 1);

        x2r = *data;
        x2i = *(data + 1);
        data += ((SIZE_T)del << 1);

        x3r = *data;
        x3i = *(data + 1);
        data -= 3 * ((SIZE_T)del << 1);

        tmp = (FLOAT32)(((FLOAT32)x1r * W1) + ((FLOAT32)x1i * W4));
        x1i = (FLOAT32)(-((FLOAT32)x1r * W4) + (FLOAT32)x1i * W1);
        x1r = tmp;

        tmp = (FLOAT32)(((FLOAT32)x2r * W2) + ((FLOAT32)x2i * W5));
        x2i = (FLOAT32)(-((FLOAT32)x2r * W5) + (FLOAT32)x2i * W2);
        x2r = tmp;

        tmp = (FLOAT32)(((FLOAT32)x3r * W3) + ((FLOAT32)x3i * W6));
        x3i = (FLOAT32)(-((FLOAT32)x3r * W6) + (FLOAT32)x3i * W3);
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
      data -= 2 * npoints;
      data += 2;
    }
    for (; j <= (nodespacing * del) >> 1; j += nodespacing) {
      W1 = *(twiddles + j);
      W4 = *(twiddles + j + 257);
      W2 = *(twiddles + ((SIZE_T)j << 1));
      W5 = *(twiddles + ((SIZE_T)j << 1) + 257);
      W3 = *(twiddles + j + ((SIZE_T)j << 1) - 256);
      W6 = *(twiddles + j + ((SIZE_T)j << 1) + 1);

      for (k = in_loop_cnt; k != 0; k--) {
        FLOAT32 tmp;
        FLOAT32 x0r, x0i, x1r, x1i, x2r, x2i, x3r, x3i;

        data += ((SIZE_T)del << 1);

        x1r = *data;
        x1i = *(data + 1);
        data += ((SIZE_T)del << 1);

        x2r = *data;
        x2i = *(data + 1);
        data += ((SIZE_T)del << 1);

        x3r = *data;
        x3i = *(data + 1);
        data -= 3 * ((SIZE_T)del << 1);

        tmp = (FLOAT32)(((FLOAT32)x1r * W1) + ((FLOAT32)x1i * W4));
        x1i = (FLOAT32)(-((FLOAT32)x1r * W4) + (FLOAT32)x1i * W1);
        x1r = tmp;

        tmp = (FLOAT32)(((FLOAT32)x2r * W2) + ((FLOAT32)x2i * W5));
        x2i = (FLOAT32)(-((FLOAT32)x2r * W5) + (FLOAT32)x2i * W2);
        x2r = tmp;

        tmp = (FLOAT32)(((FLOAT32)x3r * W6) - ((FLOAT32)x3i * W3));
        x3i = (FLOAT32)(((FLOAT32)x3r * W3) + ((FLOAT32)x3i * W6));
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
      data -= 2 * npoints;
      data += 2;
    }
    for (; j <= sec_loop_cnt * 2; j += nodespacing) {
      W1 = *(twiddles + j);
      W4 = *(twiddles + j + 257);
      W2 = *(twiddles + ((SIZE_T)j << 1) - 256);
      W5 = *(twiddles + ((SIZE_T)j << 1) + 1);
      W3 = *(twiddles + j + ((SIZE_T)j << 1) - 256);
      W6 = *(twiddles + j + ((SIZE_T)j << 1) + 1);

      for (k = in_loop_cnt; k != 0; k--) {
        FLOAT32 tmp;
        FLOAT32 x0r, x0i, x1r, x1i, x2r, x2i, x3r, x3i;

        data += ((SIZE_T)del << 1);

        x1r = *data;
        x1i = *(data + 1);
        data += ((SIZE_T)del << 1);

        x2r = *data;
        x2i = *(data + 1);
        data += ((SIZE_T)del << 1);

        x3r = *data;
        x3i = *(data + 1);
        data -= 3 * ((SIZE_T)del << 1);

        tmp = (FLOAT32)(((FLOAT32)x1r * W1) + ((FLOAT32)x1i * W4));
        x1i = (FLOAT32)(-((FLOAT32)x1r * W4) + (FLOAT32)x1i * W1);
        x1r = tmp;

        tmp = (FLOAT32)(((FLOAT32)x2r * W5) - ((FLOAT32)x2i * W2));
        x2i = (FLOAT32)(((FLOAT32)x2r * W2) + ((FLOAT32)x2i * W5));
        x2r = tmp;

        tmp = (FLOAT32)(((FLOAT32)x3r * W6) - ((FLOAT32)x3i * W3));
        x3i = (FLOAT32)(((FLOAT32)x3r * W3) + ((FLOAT32)x3i * W6));
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
      data -= 2 * npoints;
      data += 2;
    }
    for (; j < nodespacing * del; j += nodespacing) {
      W1 = *(twiddles + j);
      W4 = *(twiddles + j + 257);
      W2 = *(twiddles + ((SIZE_T)j << 1) - 256);
      W5 = *(twiddles + ((SIZE_T)j << 1) + 1);
      W3 = *(twiddles + j + ((SIZE_T)j << 1) - 512);
      W6 = *(twiddles + j + ((SIZE_T)j << 1) - 512 + 257);

      for (k = in_loop_cnt; k != 0; k--) {
        FLOAT32 tmp;
        FLOAT32 x0r, x0i, x1r, x1i, x2r, x2i, x3r, x3i;

        data += ((SIZE_T)del << 1);

        x1r = *data;
        x1i = *(data + 1);
        data += ((SIZE_T)del << 1);

        x2r = *data;
        x2i = *(data + 1);
        data += ((SIZE_T)del << 1);

        x3r = *data;
        x3i = *(data + 1);
        data -= 3 * ((SIZE_T)del << 1);

        tmp = (FLOAT32)(((FLOAT32)x1r * W1) + ((FLOAT32)x1i * W4));
        x1i = (FLOAT32)(-((FLOAT32)x1r * W4) + (FLOAT32)x1i * W1);
        x1r = tmp;

        tmp = (FLOAT32)(((FLOAT32)x2r * W5) - ((FLOAT32)x2i * W2));
        x2i = (FLOAT32)(((FLOAT32)x2r * W2) + ((FLOAT32)x2i * W5));
        x2r = tmp;

        tmp = (FLOAT32)(-((FLOAT32)x3r * W3) - ((FLOAT32)x3i * W6));
        x3i = (FLOAT32)(-((FLOAT32)x3r * W6) + (FLOAT32)x3i * W3);
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
      data -= 2 * npoints;
      data += 2;
    }
    nodespacing >>= 2;
    del <<= 2;
    in_loop_cnt >>= 2;
  }

  if (not_power_4) {
    const FLOAT32 *twiddles = ptr_w;
    nodespacing <<= 1;

    for (j = del / 2; j != 0; j--) {
      FLOAT32 W1 = *twiddles;
      FLOAT32 W4 = *(twiddles + 257);
      FLOAT32 tmp;
      twiddles += nodespacing;

      x0r = *ptr_y;
      x0i = *(ptr_y + 1);
      ptr_y += ((SIZE_T)del << 1);

      x1r = *ptr_y;
      x1i = *(ptr_y + 1);

      tmp = (FLOAT32)(((FLOAT32)x1r * W1) + ((FLOAT32)x1i * W4));
      x1i = (FLOAT32)(-((FLOAT32)x1r * W4) + (FLOAT32)x1i * W1);
      x1r = tmp;

      *ptr_y = (x0r) - (x1r);
      *(ptr_y + 1) = (x0i) - (x1i);
      ptr_y -= ((SIZE_T)del << 1);

      *ptr_y = (x0r) + (x1r);
      *(ptr_y + 1) = (x0i) + (x1i);
      ptr_y += 2;
    }
    twiddles = ptr_w;
    for (j = del / 2; j != 0; j--) {
      FLOAT32 W1 = *twiddles;
      FLOAT32 W4 = *(twiddles + 257);
      FLOAT32 tmp;
      twiddles += nodespacing;

      x0r = *ptr_y;
      x0i = *(ptr_y + 1);
      ptr_y += ((SIZE_T)del << 1);

      x1r = *ptr_y;
      x1i = *(ptr_y + 1);

      tmp = (FLOAT32)(((FLOAT32)x1r * W4) - ((FLOAT32)x1i * W1));
      x1i = (FLOAT32)(((FLOAT32)x1r * W1) + ((FLOAT32)x1i * W4));
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

static PLATFORM_INLINE void ixheaac_aac_ld_dec_fft_3_float(FLOAT32 *inp, FLOAT32 *op) {
  FLOAT32 add_r, sub_r;
  FLOAT32 add_i, sub_i;
  FLOAT32 temp_real, temp_imag, temp;

  FLOAT32 p1, p2, p3, p4;

  FLOAT32 sinmu;
  sinmu = -0.866025403784439f;

  temp_real = inp[0] + inp[2];
  temp_imag = inp[1] + inp[3];

  add_r = inp[2] + inp[4];
  add_i = inp[3] + inp[5];

  sub_r = inp[2] - inp[4];
  sub_i = inp[3] - inp[5];

  p1 = add_r / 2.0f;
  p4 = add_i / 2.0f;
  p2 = sub_i * sinmu;
  p3 = sub_r * sinmu;

  temp = inp[0] - p1;

  op[0] = temp_real + inp[4];
  op[1] = temp_imag + inp[5];
  op[2] = temp + p2;
  op[3] = (inp[1] - p3) - p4;
  op[4] = temp - p2;
  op[5] = (inp[1] + p3) - p4;

  return;
}

void ixheaac_real_synth_fft_p3(FLOAT32 *x_in, FLOAT32 *x_out, WORD32 npoints) {
  WORD32 i, j;
  FLOAT32 x_3[8];
  FLOAT32 y_3[16];
  FLOAT32 y[48];
  FLOAT32 x[48];
  FLOAT32 *ptr_y = y;
  FLOAT32 *y_p3 = y;
  FLOAT32 *x_p3 = x;

  for (i = 0; i < 3; i += 1) {
    for (j = 0; j < (npoints / 3); j++) {
      x_3[j] = x_in[3 * j + i];
    }

    ixheaac_real_synth_fft_p2(x_3, y_3, 8);

    for (j = 0; j < 16; j += 2) {
      x[3 * j + 2 * i] = y_3[j];
      x[3 * j + 2 * i + 1] = y_3[j + 1];
    }
  }

  {
    FLOAT32 *wr;
    FLOAT32 tmp;
    FLOAT32 *x_tw = x;
    wr = (FLOAT32 *)ixheaac_twidle_tbl_24;
    x_tw += 2;

    for (i = 0; i < (npoints / 3); i++) {
      tmp = ((*x_tw) * (*wr) + (*(x_tw + 1)) * (*(wr + 1)));
      *(x_tw + 1) = (-(*x_tw) * (*(wr + 1)) + (*(x_tw + 1)) * (*wr));
      *x_tw = tmp;

      wr += 2;
      x_tw += 2;

      tmp = ((*x_tw) * (*wr) + (*(x_tw + 1)) * (*(wr + 1)));
      *(x_tw + 1) = (-(*x_tw) * (*(wr + 1)) + (*(x_tw + 1)) * (*wr));
      *x_tw = tmp;

      wr += 2;
      x_tw += 4;
    }
  }

  for (i = 0; i < (npoints / 3); i++) {
    ixheaac_aac_ld_dec_fft_3_float(x_p3, y_p3);

    x_p3 = x_p3 + 6;
    y_p3 = y_p3 + 6;
  }

  for (i = 0; i < 16; i += 2) {
    x_out[i] = *ptr_y++;
    x_out[i + 1] = *ptr_y++;
    x_out[16 + i] = *ptr_y++;
    x_out[16 + i + 1] = *ptr_y++;
    x_out[32 + i] = *ptr_y++;
    x_out[32 + i + 1] = *ptr_y++;
  }
}

void ixheaac_cmplx_anal_fft_p3(FLOAT32 *x_in, FLOAT32 *x_out, WORD32 npoints) {
  WORD32 i, j;
  FLOAT32 x_3[32];
  FLOAT32 y_3[32];
  FLOAT32 y[96];
  FLOAT32 *ptr_x = x_in;
  FLOAT32 *ptr_y = y;
  FLOAT32 *y_p3 = y;

  for (i = 0; i < 6; i += 2) {
    for (j = 0; j < 32; j += 2) {
      x_3[j] = x_in[3 * j + i];
      x_3[j + 1] = x_in[3 * j + i + 1];
    }

    ixheaac_cmplx_anal_fft_p2(x_3, y_3, 16);

    for (j = 0; j < 32; j += 2) {
      x_in[3 * j + i] = y_3[j];
      x_in[3 * j + i + 1] = y_3[j + 1];
    }
  }

  {
    FLOAT32 *wr;
    FLOAT32 tmp;
    wr = (FLOAT32 *)ixheaac_twidle_tbl_48;
    x_in += 2;

    for (i = 0; i < (npoints / 3); i++) {
      tmp = ((*x_in) * (*wr) + (*(x_in + 1)) * (*(wr + 1)));
      *(x_in + 1) = (-(*x_in) * (*(wr + 1)) + (*(x_in + 1)) * (*wr));
      *x_in = tmp;

      wr += 2;
      x_in += 2;

      tmp = ((*x_in) * (*wr) + (*(x_in + 1)) * (*(wr + 1)));
      *(x_in + 1) = (-(*x_in) * (*(wr + 1)) + (*(x_in + 1)) * (*wr));
      *x_in = tmp;

      wr += 2;
      x_in += 4;
    }
  }

  for (i = 0; i < (npoints / 3); i++) {
    ixheaac_aac_ld_dec_fft_3_float(ptr_x, ptr_y);

    ptr_x = ptr_x + 6;
    ptr_y = ptr_y + 6;
  }

  for (i = 0; i < 32; i += 2) {
    x_out[i] = *y_p3++;
    x_out[i + 1] = *y_p3++;
    x_out[32 + i] = *y_p3++;
    x_out[32 + i + 1] = *y_p3++;
    x_out[64 + i] = *y_p3++;
    x_out[64 + i + 1] = *y_p3++;
  }
}
