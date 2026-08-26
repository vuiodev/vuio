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
#include <memory.h>
#include "ixheaac_type_def.h"

#include "ixheaacd_cnst.h"
#include "ixheaac_constants.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops40.h"

const FLOAT32 ixheaacd_gamma_table[17] = {
    1.0f,      0.92f,     0.8464f,   0.778688f, 0.716393f, 0.659082f,
    0.606355f, 0.557847f, 0.513219f, 0.472161f, 0.434389f, 0.399637f,
    0.367666f, 0.338253f, 0.311193f, 0.286298f, 0.263394f};

WORD16 ixheaacd_rand_gen(WORD16 *seed) {
  *seed = (WORD16)(*seed * 31821L + 13849L);
  return (*seed);
}

VOID ixheaacd_preemphsis_tool(WORD32 *signal, WORD32 mu, WORD32 len,
                              WORD32 mem) {
  WORD32 i;
  for (i = len - 1; i > 0; i--) {
    signal[i] -= (WORD32)ixheaac_mul32_sh(mu, signal[i - 1], 16);
  }
  signal[0] -= (WORD32)ixheaac_mul32_sh(mu, mem, 16);
  return;
}

VOID ixheaacd_preemphsis_tool_float(FLOAT32 *signal, FLOAT32 mu, WORD32 len,
                                    FLOAT32 mem) {
  WORD32 i;
  for (i = len - 1; i > 0; i--) {
    signal[i] = signal[i] - mu * signal[i - 1];
  }
  signal[0] -= mu * mem;
  return;
}

VOID ixheaacd_deemphsis_tool(FLOAT32 *signal, WORD32 len, FLOAT32 mem) {
  WORD32 i;
  signal[0] = signal[0] + PREEMPH_FILT_FAC * mem;
  for (i = 1; i < len; i++) {
    signal[i] = signal[i] + PREEMPH_FILT_FAC * signal[i - 1];
  }
  return;
}

VOID ixheaacd_lpc_wt_synthesis_tool(FLOAT32 a[], FLOAT32 x[], WORD32 l) {
  FLOAT32 s;
  WORD32 i, j;

  for (i = 0; i < l; i++) {
    s = x[i];
    for (j = 1; j <= ORDER; j += 4) {
      s -= (a[j] * ixheaacd_gamma_table[j]) * x[i - j];
      s -= (a[j + 1] * ixheaacd_gamma_table[j + 1]) * x[i - (j + 1)];
      s -= (a[j + 2] * ixheaacd_gamma_table[j + 2]) * x[i - (j + 2)];
      s -= (a[j + 3] * ixheaacd_gamma_table[j + 3]) * x[i - (j + 3)];
    }
    x[i] = s;
  }

  return;
}

VOID ixheaacd_synthesis_tool_float(FLOAT32 a[], FLOAT32 x[], FLOAT32 y[],
                                   WORD32 l, FLOAT32 mem[]) {
  FLOAT32 buf[LEN_FRAME * 2];
  FLOAT32 s;
  FLOAT32 *yy;
  WORD32 i, j;
  memcpy(buf, mem, ORDER * sizeof(FLOAT32));
  yy = &buf[ORDER];
  for (i = 0; i < l; i++) {
    s = x[i];
    for (j = 1; j <= ORDER; j += 4) {
      s -= a[j] * yy[i - j];
      s -= a[j + 1] * yy[i - (j + 1)];
      s -= a[j + 2] * yy[i - (j + 2)];
      s -= a[j + 3] * yy[i - (j + 3)];
    }
    yy[i] = s;
    y[i] = s;
  }

  return;
}

VOID ixheaacd_synthesis_tool_float1(FLOAT32 a[], FLOAT32 x[], WORD32 l) {
  FLOAT32 s;
  WORD32 i, j;
  for (i = 0; i < l; i++) {
    s = x[i];
    for (j = 1; j <= ORDER; j += 4) {
      s -= a[j] * x[i - j];
      s -= a[j + 1] * x[i - (j + 1)];
      s -= a[j + 2] * x[i - (j + 2)];
      s -= a[j + 3] * x[i - (j + 3)];
    }
    x[i] = s;
  }

  return;
}

VOID ixheaacd_residual_tool(WORD32 *a, WORD32 *x, WORD32 *y, WORD32 l,
                            WORD32 count) {
  WORD32 s;
  WORD32 i, j;
  WORD32 n = l * count;

  for (i = 0; i < n; i++) {
    s = x[i];
    for (j = 1; j <= 16; j++)
      s += (WORD32)ixheaac_mul32_sh(a[j], x[i - j], 24);
    y[i] = s;
  }

  return;
}

VOID ixheaacd_residual_tool_float(FLOAT32 *a, FLOAT32 *x, FLOAT32 *y, WORD32 l,
                                  WORD32 loop_count) {
  FLOAT32 s;
  WORD32 i, j;
  for (j = 0; j < loop_count; j++) {
    for (i = 0; i < l; i++) {
      s = x[i];
      s += a[1] * x[i - 1];
      s += a[2] * x[i - 2];
      s += a[3] * x[i - 3];
      s += a[4] * x[i - 4];
      s += a[5] * x[i - 5];
      s += a[6] * x[i - 6];
      s += a[7] * x[i - 7];
      s += a[8] * x[i - 8];
      s += a[9] * x[i - 9];
      s += a[10] * x[i - 10];
      s += a[11] * x[i - 11];
      s += a[12] * x[i - 12];
      s += a[13] * x[i - 13];
      s += a[14] * x[i - 14];
      s += a[15] * x[i - 15];
      s += a[16] * x[i - 16];
      y[i] = s;
    }
    a += 17;
    x += l;
    y += l;
  }
  return;
}

VOID ixheaacd_residual_tool_float1(FLOAT32 *a, FLOAT32 *x, FLOAT32 *y, WORD32 l,
                                   WORD32 loop_count) {
  FLOAT32 s;
  WORD32 i, j;
  for (j = 0; j < loop_count; j++) {
    for (i = 0; i < l; i++) {
      s = x[i];
      s += a[1] * x[i - 1];
      s += a[2] * x[i - 2];
      s += a[3] * x[i - 3];
      s += a[4] * x[i - 4];
      s += a[5] * x[i - 5];
      s += a[6] * x[i - 6];
      s += a[7] * x[i - 7];
      s += a[8] * x[i - 8];
      s += a[9] * x[i - 9];
      s += a[10] * x[i - 10];
      s += a[11] * x[i - 11];
      s += a[12] * x[i - 12];
      s += a[13] * x[i - 13];
      s += a[14] * x[i - 14];
      s += a[15] * x[i - 15];
      s += a[16] * x[i - 16];
      y[i] = s;
    }
    x += l;
    y += l;
  }
  return;
}
