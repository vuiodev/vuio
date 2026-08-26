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

#pragma once
typedef struct {
  WORD32 ms_mask;
  WORD32 ms_used[MAX_SHORT_WINDOWS][MAX_SFB_LONG];
} ia_ms_info_struct;

VOID iusace_ms_apply(ia_psy_mod_data_struct *pstr_psy_data, FLOAT64 *ptr_spec_left,
                     FLOAT64 *ptr_spec_right, WORD32 *ms_select,
                     WORD32 ms_used[MAX_SHORT_WINDOWS][MAX_SFB_LONG], const WORD32 sfb_count,
                     const WORD32 sfb_per_group, const WORD32 max_sfb_per_grp,
                     const WORD32 *ptr_sfb_offsets, WORD32 chn, FLOAT64 *ptr_ms_spec);

VOID iusace_calc_ms_band_energy(const FLOAT64 *ptr_spec_left, const FLOAT64 *ptr_spec_right,
                                const WORD32 *ptr_band_offset, const WORD32 num_bands,
                                FLOAT32 *ptr_band_energy_mid, FLOAT32 *ptr_band_energy_side);
