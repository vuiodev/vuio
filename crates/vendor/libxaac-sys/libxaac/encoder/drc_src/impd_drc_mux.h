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
#define MIN_DRC_GAIN_CODING_PROFILE0 (-31.875f)
#define MAX_DRC_GAIN_CODING_PROFILE0 (31.875f)

#define MIN_DRC_GAIN_CODING_PROFILE1 (-128.0f)
#define MAX_DRC_GAIN_CODING_PROFILE1 (0.0f)

#define MIN_DRC_GAIN_CODING_PROFILE2 (-32.0f)
#define MAX_DRC_GAIN_CODING_PROFILE2 (0.0f)

#define INV_LOG10_2 (3.32192809488736f) /* 1.0 / log10(2.0) */

IA_ERRORCODE impd_drc_enc_initial_gain(const WORD32 gain_coding_profile, FLOAT32 gain_initial,
                                       FLOAT32 *gain_initial_quant, WORD32 *code_size,
                                       WORD32 *code);
