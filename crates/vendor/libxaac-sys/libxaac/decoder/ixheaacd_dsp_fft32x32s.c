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
#include "ixheaacd_sbr_common.h"
#include "ixheaac_type_def.h"

#include "ixheaac_constants.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops16.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaac_basic_ops.h"
#include "ixheaacd_common_rom.h"
#include "ixheaacd_dsp_fft32x32s.h"
#include "ixheaacd_intrinsics.h"

#include "ixheaac_basic_op.h"

VOID ixheaacd_inv_dit_fft_8pt_dec(WORD32 *y, WORD32 *real, WORD32 *imag) {
  WORD32 a0, a1, a2, a3, a00, a10, a20, a30;
  WORD32 vr, vi;

  WORD32 x[16];

  a00 = ixheaac_add32_sat(y[0], y[8]);
  a0 = ixheaac_sub32_sat(y[0], y[8]);

  a20 = ixheaac_add32_sat(y[1], y[9]);
  a3 = ixheaac_sub32_sat(y[1], y[9]);

  a10 = ixheaac_add32_sat(y[4], y[12]);
  a2 = ixheaac_sub32_sat(y[4], y[12]);

  a30 = ixheaac_add32_sat(y[5], y[13]);
  a1 = ixheaac_sub32_sat(y[5], y[13]);

  x[0] = ixheaac_add32_sat(a00, a10);
  x[4] = ixheaac_sub32_sat(a00, a10);
  x[1] = ixheaac_add32_sat(a20, a30);
  x[5] = ixheaac_sub32_sat(a20, a30);

  x[2] = ixheaac_sub32_sat(a0, a1);
  x[6] = ixheaac_add32_sat(a0, a1);
  x[3] = ixheaac_add32_sat(a3, a2);
  x[7] = ixheaac_sub32_sat(a3, a2);

  a00 = ixheaac_add32_sat(y[2], y[10]);
  a0 = ixheaac_sub32_sat(y[2], y[10]);

  a20 = ixheaac_add32_sat(y[3], y[11]);
  a3 = ixheaac_sub32_sat(y[3], y[11]);

  a10 = ixheaac_add32_sat(y[6], y[14]);
  a2 = ixheaac_sub32_sat(y[6], y[14]);

  a30 = ixheaac_add32_sat(y[7], y[15]);
  a1 = ixheaac_sub32_sat(y[7], y[15]);

  x[8] = ixheaac_add32_sat(a00, a10);
  x[12] = ixheaac_sub32_sat(a00, a10);
  x[9] = ixheaac_add32_sat(a20, a30);
  x[13] = ixheaac_sub32_sat(a20, a30);

  x[10] = ixheaac_sub32_sat(a0, a1);
  x[14] = ixheaac_add32_sat(a0, a1);
  x[11] = ixheaac_add32_sat(a3, a2);
  x[15] = ixheaac_sub32_sat(a3, a2);

  real[0] = ixheaac_add32_sat(x[0], x[8]);
  imag[0] = ixheaac_add32_sat(x[1], x[9]);
  a00 = ixheaac_sub32_sat(x[0], x[8]);
  a10 = ixheaac_sub32_sat(x[1], x[9]);

  a0 = ixheaac_sub32_sat(x[4], x[13]);
  a1 = ixheaac_add32_sat(x[5], x[12]);

  real[4] = ixheaac_add32_sat(x[4], x[13]);
  imag[4] = ixheaac_sub32_sat(x[5], x[12]);

  vr = ixheaac_mult32x16in32_shl_sat(ixheaac_sub32_sat(x[10], x[11]), 0x5A82);
  vi = ixheaac_mult32x16in32_shl_sat(ixheaac_add32_sat(x[10], x[11]), 0x5A82);

  real[1] = ixheaac_add32_sat(x[2], vr);
  imag[1] = ixheaac_add32_sat(x[3], vi);

  a2 = ixheaac_sub32_sat(x[2], vr);
  a3 = ixheaac_sub32_sat(x[3], vi);

  real[2] = ixheaac_add32_sat(a0, a2);
  imag[2] = ixheaac_add32_sat(a1, a3);
  vr = ixheaac_mult32x16in32_shl_sat(ixheaac_add32_sat(x[14], x[15]), 0x5A82);
  vi = ixheaac_mult32x16in32_shl_sat(ixheaac_sub32_sat(x[14], x[15]), 0x5A82);

  a20 = ixheaac_sub32_sat(x[6], vr);
  a30 = ixheaac_add32_sat(x[7], vi);

  real[3] = ixheaac_add32_sat(a00, a20);
  imag[3] = ixheaac_add32_sat(a10, a30);

  real[5] = ixheaac_add32_sat(x[6], vr);
  imag[5] = ixheaac_sub32_sat(x[7], vi);
}
