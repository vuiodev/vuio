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

#define LOG2_1 1.442695041f /* 1/log(2) */

VOID iaace_calc_form_fac_per_chan(FLOAT32 *ptr_sfb_form_factor,
                                  FLOAT32 *ptr_sfb_num_relevant_lines,
                                  ixheaace_psy_out_channel *pstr_psy_out_chan,
                                  FLOAT32 *ptr_sfb_ld_energy);

VOID iaace_estimate_scfs_chan(
    ixheaace_psy_out_channel **pstr_psy_out_ch,
    ixheaace_qc_out_channel **pstr_qc_out_ch,
    FLOAT32 sfb_form_factor_ch[IXHEAACE_MAX_CH_IN_BS_ELE][MAXIMUM_GROUPED_SCALE_FACTOR_BAND],
    FLOAT32 sfb_num_relevant_lines_ch[][MAXIMUM_GROUPED_SCALE_FACTOR_BAND], WORD32 num_channels,
    WORD32 chn, WORD32 frame_len_long);
