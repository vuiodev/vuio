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

VOID iaace_group_short_data(FLOAT32 *ptr_mdct_spectrum, FLOAT32 *ptr_tmp_spectrum,
                            ixheaace_sfb_energy *pstr_sfb_threshold,
                            ixheaace_sfb_energy *pstr_sfb_energy,
                            ixheaace_sfb_energy *pstr_sfb_energy_ms,
                            ixheaace_sfb_energy *pstr_sfb_spreaded_energy, const WORD32 sfb_cnt,
                            const WORD32 *ptr_sfb_offset, const FLOAT32 *ptr_sfb_min_snr,
                            WORD32 *ptr_grouped_sfb_offset, WORD32 *ptr_max_sfb_per_group,
                            FLOAT32 *ptr_grouped_sfb_min_snr, const WORD32 no_of_groups,
                            const WORD32 *ptr_group_len, WORD32 frame_length);
