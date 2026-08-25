/******************************************************************************
 *
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
#include "ixheaac_type_def.h"
#include "ixheaac_constants.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops16.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaac_basic_ops.h"
#include "ixheaacd_defines.h"

VOID ixheaacd_res_tns_parcor_2_lpc_32x16(WORD16 *parcor, WORD16 *lpc, WORD16 *scale, WORD order)

{
  WORD i, j, status;
  WORD32 z1;
  WORD16 z[MAX_ORDER + 1];
  WORD16 w[MAX_ORDER + 1];
  WORD32 accu1, accu2;

  status = 1;
  *scale = 0;
  while (status) {
    status = 0;

    for (i = MAX_ORDER; i >= 0; i--) {
      z[i] = 0;
      w[i] = 0;
    }

    accu1 = (0x7fffffff >> *scale);

    for (i = 0; i <= order; i++) {
      z1 = accu1;

      for (j = 0; j < order; j++) {
        w[j] = ixheaac_round16(accu1);

        accu1 = ixheaac_mac16x16in32_shl_sat(accu1, parcor[j], z[j]);
        if (ixheaac_abs32_sat(accu1) == 0x7fffffff) status = 1;
      }
      for (j = (order - 1); j >= 0; j--) {
        accu2 = ixheaac_deposit16h_in32(z[j]);
        accu2 = ixheaac_mac16x16in32_shl_sat(accu2, parcor[j], w[j]);
        z[j + 1] = ixheaac_round16(accu2);
        if (ixheaac_abs32_sat(accu2) == 0x7fffffff) status = 1;
      }

      z[0] = ixheaac_round16(z1);
      lpc[i] = ixheaac_round16(accu1);
      accu1 = 0;
    }
    accu1 = (status - 1);
    if (accu1 == 0) {
      *scale = *scale + 1;
    }
  }
}

VOID ixheaacd_res_tns_ar_filter_fixed_32x16(WORD32 *spectrum, WORD32 size, WORD32 inc,
                                            WORD16 *lpc, WORD32 order, WORD32 shift_value,
                                            WORD scale_spec) {
  WORD32 i, j;
  WORD32 y, state[MAX_ORDER + 1];

  if ((order & 3) != 0) {
    for (i = order + 1; i < ((WORD32)(order & 0xfffffffc) + 4); i++) {
      lpc[i] = 0;
    }
    lpc[i] = 0;
    order = ((order & 0xfffffffc) + 4);
    order = order & 31;
  }

  for (i = 0; i < order; i++) {
    y = (*spectrum) << scale_spec;
    for (j = i; j > 0; j--) {
      y = ixheaac_sub32_sat(y, ixheaac_mult32x16in32_shl_sat(state[j - 1], lpc[j]));
      state[j] = state[j - 1];
    }

    state[0] = ixheaac_shl32_dir_sat_limit(y, shift_value);
    *spectrum = y >> scale_spec;
    spectrum += inc;
  }

  for (i = order; i < size; i++) {
    y = (*spectrum) << scale_spec;

    for (j = order; j > 0; j--) {
      y = ixheaac_sub32_sat(y, ixheaac_mult32x16in32_shl_sat(state[j - 1], lpc[j]));
      state[j] = state[j - 1];
    }

    state[0] = ixheaac_shl32_dir_sat_limit(y, shift_value);
    *spectrum = y >> scale_spec;
    spectrum += inc;
  }
}

WORD32 ixheaacd_res_calc_max_spectral_line(WORD32 *p_tmp, WORD32 size) {
  WORD32 max_spectral_line = 0, i;
  WORD count, remaining, temp_1, temp_2, temp3, temp4;

  count = size >> 3;
  for (i = count; i--;) {
    temp_1 = *p_tmp++;
    temp_2 = *p_tmp++;
    temp3 = *p_tmp++;
    temp4 = *p_tmp++;

    max_spectral_line = ixheaac_abs32_nrm(temp_1) | max_spectral_line;
    max_spectral_line = ixheaac_abs32_nrm(temp_2) | max_spectral_line;
    max_spectral_line = ixheaac_abs32_nrm(temp3) | max_spectral_line;
    max_spectral_line = ixheaac_abs32_nrm(temp4) | max_spectral_line;
    temp_1 = *p_tmp++;
    temp_2 = *p_tmp++;
    temp3 = *p_tmp++;
    temp4 = *p_tmp++;

    max_spectral_line = ixheaac_abs32_nrm(temp_1) | max_spectral_line;
    max_spectral_line = ixheaac_abs32_nrm(temp_2) | max_spectral_line;
    max_spectral_line = ixheaac_abs32_nrm(temp3) | max_spectral_line;
    max_spectral_line = ixheaac_abs32_nrm(temp4) | max_spectral_line;
  }

  remaining = size - (count << 3);
  if (remaining) {
    for (i = remaining; i--;) {
      max_spectral_line = ixheaac_abs32_nrm(*p_tmp) | max_spectral_line;
      p_tmp++;
    }
  }

  return ixheaac_norm32(max_spectral_line);
}
