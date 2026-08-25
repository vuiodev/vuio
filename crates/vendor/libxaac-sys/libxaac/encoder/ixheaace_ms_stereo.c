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
#include <stdlib.h>

#include "ixheaace_psy_const.h"
#include "ixheaace_tns.h"
#include "ixheaace_tns_params.h"
#include "ixheaace_rom.h"
#include "ixheaace_common_rom.h"
#include "ixheaace_bitbuffer.h"

#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops16.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaac_basic_ops.h"

#include <math.h>
#include "ixheaace_block_switch.h"
#include "ixheaace_psy_utils_spreading.h"
#include "ixheaace_psy_utils.h"
#include "ixheaace_calc_ms_band_energy.h"
#include "ixheaace_psy_configuration.h"
#include "ixheaace_psy_data.h"
#include "ixheaace_ms_stereo.h"
#include "ixheaace_common_utils.h"

VOID iaace_ms_apply(ixheaace_psy_data **ptr_psy_data, FLOAT32 *ptr_spec_left,
                    FLOAT32 *ptr_spec_right, WORD32 *ptr_ms_select, WORD32 *ptr_ms_used,
                    const WORD32 sfb_count, const WORD32 sfb_per_group,
                    const WORD32 max_sfb_per_grp, const WORD32 *ptr_sfb_offset,
                    FLOAT32 *ptr_weight_ms_lr_pe_ratio) {
  FLOAT32 *ptr_sfb_enegry_left = ptr_psy_data[0]->sfb_energy.long_nrg;
  FLOAT32 *ptr_sfb_energy_right = ptr_psy_data[1]->sfb_energy.long_nrg;
  const FLOAT32 *ptr_sfb_energy_mid = ptr_psy_data[0]->sfb_energy_ms.long_nrg;
  const FLOAT32 *ptr_sfb_energy_side = ptr_psy_data[1]->sfb_energy_ms.long_nrg;
  FLOAT32 *ptr_sfb_thr_left = ptr_psy_data[0]->sfb_threshold.long_nrg;
  FLOAT32 *ptr_sfb_thr_right = ptr_psy_data[1]->sfb_threshold.long_nrg;
  FLOAT32 *ptr_sfb_spread_energy_left = ptr_psy_data[0]->sfb_sreaded_energy.long_nrg;
  FLOAT32 *ptr_sfb_spread_energy_right = ptr_psy_data[1]->sfb_sreaded_energy.long_nrg;
  WORD32 sfb, sfb_offsets, j;
  WORD32 grp = 0;
  WORD32 ms_counter = 0;
  WORD32 lr_counter = 0;
  FLOAT32 sum_ss_sr_pe_ratio = 0;
  WORD32 cnt = 0;
  FLOAT32 atan_val;
  *ptr_ms_select = 0;

  for (sfb = 0; sfb < sfb_count; sfb += sfb_per_group, grp++) {
    for (sfb_offsets = 0; sfb_offsets < max_sfb_per_grp; sfb_offsets++) {
      FLOAT32 left_right, mid_side, min_thr;
      WORD32 use_ms;
      ptr_ms_used[sfb + sfb_offsets] = 0;

      min_thr = MIN(ptr_sfb_thr_left[sfb + sfb_offsets], ptr_sfb_thr_right[sfb + sfb_offsets]);

      left_right =
          (ptr_sfb_thr_left[sfb + sfb_offsets] /
           MAX(ptr_sfb_enegry_left[sfb + sfb_offsets], ptr_sfb_thr_left[sfb + sfb_offsets])) *
          (ptr_sfb_thr_right[sfb + sfb_offsets] /
           MAX(ptr_sfb_energy_right[sfb + sfb_offsets], ptr_sfb_thr_right[sfb + sfb_offsets]));

      mid_side = (min_thr / MAX(ptr_sfb_energy_mid[sfb + sfb_offsets], min_thr)) *
                 (min_thr / MAX(ptr_sfb_energy_side[sfb + sfb_offsets], min_thr));

      sum_ss_sr_pe_ratio += (left_right + 1.0e-9f) / (mid_side + 1.0e-9f);
      cnt++;
      use_ms = (mid_side >= left_right);

      if (use_ms) {
        ptr_ms_used[sfb + sfb_offsets] = 1;

        for (j = ptr_sfb_offset[sfb + sfb_offsets]; j < ptr_sfb_offset[sfb + sfb_offsets + 1];
             j++) {
          FLOAT32 tmp = ptr_spec_left[j];

          ptr_spec_left[j] = 0.5f * (ptr_spec_left[j] + ptr_spec_right[j]);

          ptr_spec_right[j] = 0.5f * (tmp - ptr_spec_right[j]);
        }

        ptr_sfb_thr_left[sfb + sfb_offsets] = ptr_sfb_thr_right[sfb + sfb_offsets] = min_thr;

        ptr_sfb_enegry_left[sfb + sfb_offsets] = ptr_sfb_energy_mid[sfb + sfb_offsets];
        ptr_sfb_energy_right[sfb + sfb_offsets] = ptr_sfb_energy_side[sfb + sfb_offsets];

        ptr_sfb_spread_energy_left[sfb + sfb_offsets] =
            ptr_sfb_spread_energy_right[sfb + sfb_offsets] =
                MIN(ptr_sfb_spread_energy_left[sfb + sfb_offsets],
                    ptr_sfb_spread_energy_right[sfb + sfb_offsets]) *
                0.5f;

        ms_counter++;
      } else {
        ptr_ms_used[sfb + sfb_offsets] = 0;
        lr_counter++;
      }
    }
  }

  if (ms_counter == 0) {
    *ptr_ms_select = 0;
  } else {
    if (lr_counter != 0) {
      *ptr_ms_select = 1;
    } else {
      *ptr_ms_select = 2;
    }
  }

  cnt = MAX(1, cnt);

  atan_val = iaace_atan_approx((FLOAT32)(0.37f * (sum_ss_sr_pe_ratio / cnt - 6.5f)));

  *ptr_weight_ms_lr_pe_ratio = (FLOAT32)((0.28f * atan_val) + 1.25f);
}
