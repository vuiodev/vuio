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
#define MAX_QUANT 8192
#define SF_OFFSET 100

#define sgn(A) ((A) > 0 ? (1) : (-1))

typedef struct ia_usac_quant_info_struct {
  WORD32 scale_factor[MAX_SF_BANDS];
  WORD32 quant_degroup[FRAME_LEN_LONG];
  WORD32 arith_size_prev;
  WORD32 reset;
  WORD32 c_prev[(FRAME_LEN_LONG / 2) + 4];
  WORD32 c_pres[(FRAME_LEN_LONG / 2) + 4];
  WORD32 max_spec_coeffs;
} ia_usac_quant_info_struct;
