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
#include <math.h>
#include "ixheaac_type_def.h"
#include "ixheaace_mps_common_fix.h"

#include "ixheaace_mps_defines.h"
#include "ixheaace_mps_common_define.h"
#include "ixheaace_mps_filter.h"

VOID ixheaace_mps_212_init_dc_filter(ixheaace_mps_pstr_dc_filter pstr_dc_filter,
                                     const UWORD32 sample_rate) {
  FLOAT32 exp;

  exp = (-ONE_BY_LN_TWO * 10 / (FLOAT32)sample_rate);
  exp = (FLOAT32)pow(2, exp);
  pstr_dc_filter->exp_c = exp;
  pstr_dc_filter->state = 0.0f;
}

VOID ixheaace_mps_212_apply_dc_filter(ixheaace_mps_pstr_dc_filter pstr_dc_filter,
                                      const FLOAT32 *const ptr_input, FLOAT32 *const ptr_output,
                                      const WORD32 signal_length) {
  WORD32 idx;
  FLOAT32 inp_1, inp_2, out;
  FLOAT32 exp_const = pstr_dc_filter->exp_c;
  FLOAT32 *const y = ptr_output;
  FLOAT32 *const state = &pstr_dc_filter->state;

  inp_2 = inp_1 = (ptr_input[0] / 2);
  out = inp_1 + (*state);

  for (idx = 1; idx < signal_length; idx++) {
    inp_1 = ptr_input[idx] / 2;
    y[idx - 1] = out;
    out = inp_1 - inp_2 + (exp_const * out);
    inp_2 = inp_1;
  }

  *state = (exp_const * out) - inp_2;
  y[idx - 1] = out;
}
