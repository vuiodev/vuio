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

#include <string.h>

#include "ixheaac_type_def.h"
#include "ixheaace_aac_constants.h"

#include "ixheaace_calc_ms_band_energy.h"

VOID ia_enhaacplus_enc_calc_band_energy(const FLOAT32 *ptr_spec_coeffs,
                                        const WORD32 *ptr_band_offset, const WORD32 num_bands,
                                        FLOAT32 *ptr_band_energy, WORD32 sfb_count,
                                        FLOAT32 *ptr_band_energy_sum) {
  WORD32 i, j;

  j = 0;
  memset(ptr_band_energy, 0, sfb_count * sizeof(*ptr_band_energy));
  *ptr_band_energy_sum = 0;

  for (i = 0; i < num_bands; i++) {
    while (j < ptr_band_offset[i + 1]) {
      ptr_band_energy[i] += (FLOAT32)((FLOAT64)ptr_spec_coeffs[j] * ptr_spec_coeffs[j]);
      j++;
    }
    *ptr_band_energy_sum = *ptr_band_energy_sum + ptr_band_energy[i];
  }
}

VOID ia_enhaacplus_enc_calc_band_energy_ms(
    const FLOAT32 *ptr_mdct_spectrum_left_fix, const FLOAT32 *ptr_mdct_spectrum_right_fix,
    const WORD32 *ptr_sfb_offset, const WORD32 num_sfb_active, const WORD32 num_sfb_total,
    FLOAT32 *ptr_band_nrg_mid, FLOAT32 *ptr_band_nrg_mid_sum, FLOAT32 *ptr_band_nrg_side,
    FLOAT32 *ptr_band_nrg_side_sum) {
  WORD32 i;
  const FLOAT32 *ptr_spec_left = ptr_mdct_spectrum_left_fix;
  const FLOAT32 *ptr_spec_right = ptr_mdct_spectrum_right_fix;

  *ptr_band_nrg_mid_sum = 0;
  *ptr_band_nrg_side_sum = 0;

  for (i = 0; i < num_sfb_active; i++) {
    WORD16 offset = (WORD16)(ptr_sfb_offset[i + 1] - ptr_sfb_offset[i]);
    FLOAT32 band_nrgy_mid = 0, band_nrg_side = 0;
    ptr_band_nrg_mid[i] = 0;
    ptr_band_nrg_side[i] = 0;
    FLOAT32 spec_mid, spec_side;
    FLOAT32 temp_mid, temp_side;

    while (offset--) {
      spec_mid = (*ptr_spec_left + *ptr_spec_right) * 0.5f;
      spec_side = (*ptr_spec_left++ - *ptr_spec_right++) * 0.5f;

      temp_mid = (spec_mid * spec_mid);
      temp_side = (spec_side * spec_side);

      band_nrgy_mid = (band_nrgy_mid + temp_mid);
      band_nrg_side = (band_nrg_side + temp_side);
    }

    ptr_band_nrg_mid[i] = band_nrgy_mid;
    ptr_band_nrg_side[i] = band_nrg_side;

    *ptr_band_nrg_mid_sum += ptr_band_nrg_mid[i];
    *ptr_band_nrg_side_sum += ptr_band_nrg_side[i];
  }

  memset(&ptr_band_nrg_mid[num_sfb_active], 0,
         (num_sfb_total - num_sfb_active) * sizeof(*ptr_band_nrg_mid));
  memset(&ptr_band_nrg_side[num_sfb_active], 0,
         (num_sfb_total - num_sfb_active) * sizeof(*ptr_band_nrg_side));
}
