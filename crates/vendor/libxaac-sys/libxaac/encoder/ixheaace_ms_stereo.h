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

VOID iaace_ms_apply(ixheaace_psy_data **ptr_psy_data, FLOAT32 *ptr_spec_left,
                    FLOAT32 *ptr_spec_right, WORD32 *ptr_ms_select, WORD32 *ptr_ms_used,
                    const WORD32 sfb_count,

                    const WORD32 sfb_per_group, const WORD32 max_sfb_per_grp,
                    const WORD32 *ptr_sfb_offset, FLOAT32 *ptr_weight_ms_lr_pe_ratio);

FLOAT32 iaace_atan_approx(FLOAT32 val);
