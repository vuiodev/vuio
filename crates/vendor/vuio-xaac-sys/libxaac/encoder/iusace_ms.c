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
#include "iusace_cnst.h"
#include "iusace_bitbuffer.h"
#include "iusace_tns_usac.h"
#include "iusace_psy_mod.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaac_basic_ops.h"

VOID iusace_ms_apply(ia_psy_mod_data_struct *pstr_psy_data, FLOAT64 *ptr_spec_left,
                     FLOAT64 *ptr_spec_right, WORD32 *ms_select,
                     WORD32 ms_used[MAX_SHORT_WINDOWS][MAX_SFB_LONG], const WORD32 sfb_count,
                     const WORD32 sfb_per_group, const WORD32 max_sfb_per_grp,
                     const WORD32 *ptr_sfb_offsets, WORD32 chn, FLOAT64 *ptr_ms_spec) {
  FLOAT32 *ptr_sfb_enegry_left = pstr_psy_data[chn].ptr_sfb_energy_long;
  FLOAT32 *ptr_sfb_energy_right = pstr_psy_data[chn + 1].ptr_sfb_energy_long;
  const FLOAT32 *ptr_sfb_energy_mid = pstr_psy_data[chn].ptr_sfb_energy_long_ms;
  const FLOAT32 *ptr_sfb_energy_side = pstr_psy_data[chn + 1].ptr_sfb_energy_long_ms;
  FLOAT32 *ptr_sfb_thr_left = pstr_psy_data[chn].ptr_sfb_thr_long;
  FLOAT32 *ptr_sfb_thr_right = pstr_psy_data[chn + 1].ptr_sfb_thr_long;
  FLOAT32 *ptr_sfb_spread_energy_left = pstr_psy_data[chn].ptr_sfb_spreaded_energy_long;
  FLOAT32 *ptr_sfb_spread_energy_right = pstr_psy_data[chn + 1].ptr_sfb_spreaded_energy_long;
  WORD32 sfb, sfb_offsets, j;
  WORD32 grp = 0;
  WORD32 ms_counter = 0;
  WORD32 lr_counter = 0;

  *ms_select = 0;

  for (sfb = 0; sfb < sfb_count; sfb += sfb_per_group, grp++) {
    for (sfb_offsets = 0; sfb_offsets < max_sfb_per_grp; sfb_offsets++) {
      FLOAT32 left_right, mid_side, min_thr;
      WORD32 use_ms;
      ms_used[grp][sfb_offsets] = 0;

      min_thr = MIN(ptr_sfb_thr_left[sfb + sfb_offsets], ptr_sfb_thr_right[sfb + sfb_offsets]);

      left_right =
          (ptr_sfb_thr_left[sfb + sfb_offsets] /
           MAX(ptr_sfb_enegry_left[sfb + sfb_offsets], ptr_sfb_thr_left[sfb + sfb_offsets])) *
          (ptr_sfb_thr_right[sfb + sfb_offsets] /
           max(ptr_sfb_energy_right[sfb + sfb_offsets], ptr_sfb_thr_right[sfb + sfb_offsets]));

      mid_side = (min_thr / max(ptr_sfb_energy_mid[sfb + sfb_offsets], min_thr)) *
                 (min_thr / max(ptr_sfb_energy_side[sfb + sfb_offsets], min_thr));

      use_ms = (mid_side >= left_right);

      if (use_ms) {
        ms_used[grp][sfb_offsets] = 1;

        for (j = ptr_sfb_offsets[sfb + sfb_offsets]; j < ptr_sfb_offsets[sfb + sfb_offsets + 1];
             j++) {
          if (ptr_ms_spec != NULL) {
            ptr_spec_left[j] = ptr_ms_spec[j];
            ptr_spec_right[j] = ptr_ms_spec[1024 + j];
          } else {
            FLOAT64 tmp = ptr_spec_left[j];

            ptr_spec_left[j] = 0.5f * (ptr_spec_left[j] + ptr_spec_right[j]);

            ptr_spec_right[j] = 0.5f * (tmp - ptr_spec_right[j]);
          }
        }

        ptr_sfb_thr_left[sfb + sfb_offsets] = ptr_sfb_thr_right[sfb + sfb_offsets] = min_thr;

        ptr_sfb_enegry_left[sfb + sfb_offsets] = ptr_sfb_energy_mid[sfb + sfb_offsets];
        ptr_sfb_energy_right[sfb + sfb_offsets] = ptr_sfb_energy_side[sfb + sfb_offsets];

        ptr_sfb_spread_energy_left[sfb + sfb_offsets] =
            ptr_sfb_spread_energy_right[sfb + sfb_offsets] =
                min(ptr_sfb_spread_energy_left[sfb + sfb_offsets],
                    ptr_sfb_spread_energy_right[sfb + sfb_offsets]) *
                0.5f;

        ms_counter++;
      } else {
        ms_used[grp][sfb_offsets] = 0;
        lr_counter++;
      }
    }
  }

  if (ms_counter == 0) {
    *ms_select = 0;
  } else {
    if (lr_counter != 0) {
      *ms_select = 1;
    } else {
      *ms_select = 2;
    }
  }
  return;
}

VOID iusace_calc_ms_band_energy(const FLOAT64 *ptr_spec_left, const FLOAT64 *ptr_spec_right,
                                const WORD32 *ptr_band_offset, const WORD32 num_bands,
                                FLOAT32 *ptr_band_energy_mid, FLOAT32 *ptr_band_energy_side) {
  WORD32 i, j;

  j = 0;
  for (i = 0; i < num_bands; i++) {
    ptr_band_energy_mid[i] = 0.0f;
    ptr_band_energy_side[i] = 0.0f;

    while (j < ptr_band_offset[i + 1]) {
      FLOAT32 specm, specs;

      specm = (FLOAT32)(0.5f * (ptr_spec_left[j] + ptr_spec_right[j]));
      specs = (FLOAT32)(0.5f * (ptr_spec_left[j] - ptr_spec_right[j]));

      ptr_band_energy_mid[i] += specm * specm;
      ptr_band_energy_side[i] += specs * specs;

      j++;
    }
  }

  return;
}
