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
#include "ixheaac_constants.h"
#include "ixheaace_aac_constants.h"
#include "ixheaace_psy_const.h"
#include "ixheaace_tns.h"
#include "ixheaace_tns_params.h"
#include "ixheaace_rom.h"
#include "ixheaace_quant.h"
#include "ixheaace_common_rom.h"
#include "ixheaace_bitbuffer.h"

VOID iaace_quantize_lines(const WORD32 gain, const WORD32 num_lines, FLOAT32 *ptr_exp_spec,
                          WORD16 *ptr_quant_spec, FLOAT32 *ptr_mdct_spec) {
  FLOAT32 quantizer;
  FLOAT32 k = -0.0946f + 0.5f;
  WORD32 line;

  quantizer = ixheaace_fd_quant_table[gain + 128];

  for (line = 0; line < num_lines; line++) {
    FLOAT32 tmp = ptr_mdct_spec[line];
    if (tmp < 0.0f) {
      ptr_exp_spec[line] = (FLOAT32)sqrt(-tmp);
      ptr_exp_spec[line] *= (FLOAT32)sqrt(ptr_exp_spec[line]);
      ptr_quant_spec[line] = -(WORD16)(k + quantizer * ptr_exp_spec[line]);
    } else {
      ptr_exp_spec[line] = (FLOAT32)sqrt(tmp);
      ptr_exp_spec[line] *= (FLOAT32)sqrt(ptr_exp_spec[line]);
      ptr_quant_spec[line] = (WORD16)(k + quantizer * ptr_exp_spec[line]);
    }
  }
}
