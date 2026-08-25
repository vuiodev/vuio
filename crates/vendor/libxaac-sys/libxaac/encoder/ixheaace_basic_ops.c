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

#include <float.h>
#include <math.h>
#include "ixheaac_type_def.h"
#include "ixheaac_constants.h"
#include "ixheaace_aac_constants.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops16.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaac_basic_ops.h"

WORD ia_enhaacplus_enc_norm32_arr(const WORD32 *word32_arr, LOOPINDEX n) {
  WORD32 max_bits = 0;
  for (; n != 0; n--) {
    max_bits = max_bits | ixheaac_abs32_sat(*word32_arr++);
  }
  return (ixheaac_pnorm32(max_bits));
}

FLOAT32 ixheaace_div32(FLOAT32 num, FLOAT32 den) {
  if (fabs(den) < FLT_EPSILON) {
    if (den < 0.0f) {
      return -num;
    }
    else {
      return num;
    }
  }
  else {
    return num / den;
  }
}

FLOAT64 ixheaace_div64(FLOAT64 num, FLOAT64 den) {
  if (fabs(den) < FLT_EPSILON) {
    if (den < 0.0) {
      return -num;
    }
    else {
      return num;
    }
  }
  else {
    return num / den;
  }
}
