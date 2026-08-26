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
extern const FLOAT32 iusace_lsf_init[ORDER];
extern const FLOAT32 iusace_sin_window_96[96];
extern const FLOAT32 iusace_sin_window_128[128];
extern const FLOAT32 iusace_sin_window_192[192];
extern const FLOAT32 iusace_sin_window_256[256];
extern const FLOAT32 iusace_res_interp_filter1_4[INTER_LP_FIL_LEN + 4];
extern const FLOAT32 iusace_lag_window[17];
extern const FLOAT32 iusace_lsf_init[ORDER];
extern const FLOAT32 iusace_ispold_init[ORDER];
extern const FLOAT32 iusace_cos_window_512[512];
extern const FLOAT32 iusace_cos_window_448[448];
extern const WORD32 iusace_acelp_core_numbits_1024[NUM_ACELP_CORE_MODES];
extern const FLOAT32 iusace_acelp_quant_gain_table[];
extern const UWORD8 iusace_acelp_ipos[36];
extern const FLOAT32 iexheaac_cos_window_384[384];
extern const FLOAT32 iusace_hp20_filter_coeffs[12][4];
extern const FLOAT32 iusace_ol_corr_weight[518];
extern const FLOAT32 iusace_interp4_1[17];
