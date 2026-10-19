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

#include "ixheaac_type_def.h"
#include "ixheaac_constants.h"
#include "ixheaace_aac_constants.h"

#include "ixheaace_psy_utils_spreading.h"

VOID ia_enhaacplus_enc_spreading_max(const WORD32 sfb_active, const FLOAT32 *ptr_mask_low_factor,
                                     const FLOAT32 *ptr_mask_high_factor,
                                     FLOAT32 *ptr_spreaded_energy) {
  WORD32 i;
  FLOAT32 temp;
  const FLOAT32 *ptr_mask_low = &ptr_mask_low_factor[sfb_active - 2];
  const FLOAT32 *ptr_mask_high = &ptr_mask_high_factor[1];
  FLOAT32 temp_nrg = ptr_spreaded_energy[0];
  FLOAT32 *ptr_spreaded_temp = ptr_spreaded_energy;

  for (i = sfb_active - 2; i >= 0; i--) {
    FLOAT32 temp_mask = *ptr_mask_high++;
    temp = (temp_mask * temp_nrg);
    temp_nrg = *++ptr_spreaded_temp;
    if (temp_nrg < temp) {
      *ptr_spreaded_temp = temp;
      temp_nrg = temp;
    }
  }

  temp_nrg = ptr_spreaded_energy[sfb_active - 1];

  for (i = sfb_active - 2; i >= 0; i--) {
    FLOAT32 temp_mask = *ptr_mask_low--;
    temp = (temp_mask * temp_nrg);
    temp_nrg = ptr_spreaded_energy[i];
    if (temp_nrg < temp) {
      ptr_spreaded_energy[i] = temp;
      temp_nrg = temp;
    }
  }
}
