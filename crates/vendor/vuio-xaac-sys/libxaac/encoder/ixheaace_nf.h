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
VOID iusace_noise_filling(WORD32 *noise_level, WORD32 *noise_offset, FLOAT64 *spectrum,
                          ia_usac_quant_info_struct *pstr_quant_info, WORD32 *sfb_offset,
                          WORD32 max_sfb, WORD32 window_size_samples, WORD32 num_window_groups,
                          const WORD32 *window_group_length, WORD32 noise_filling_start_offset,
                          FLOAT64 *ptr_scratch_buf);
