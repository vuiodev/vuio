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
  UWORD8 modify_min_snr;
  WORD32 start_sfb_long;
  WORD32 start_sfb_short;
} ia_ah_param_struct;

typedef struct {
  FLOAT32 max_red;
  FLOAT32 start_ratio;
  FLOAT32 max_ratio;
  FLOAT32 red_ratio_fac;
  FLOAT32 red_offs;
} ia_min_snr_adapt_param_struct;

typedef struct {
  FLOAT32 clip_save_low;
  FLOAT32 clip_save_high;
  FLOAT32 min_bit_save;
  FLOAT32 max_bit_save;
  FLOAT32 clip_spend_low;
  FLOAT32 clip_spend_high;
  FLOAT32 min_bits_spend;
  FLOAT32 max_bits_spend;
} ia_bitres_param_struct;

typedef struct {
  FLOAT32 pe_min;
  FLOAT32 pe_max;
  FLOAT32 pe_offset;
  ia_ah_param_struct str_ah_param;
  ia_min_snr_adapt_param_struct str_min_snr_adapt_params;
  FLOAT32 pe_last;
  WORD32 dyn_bits_last;
  FLOAT32 pe_correction_fac;
} ia_adj_thr_elem_struct;

typedef struct {
  ia_bitres_param_struct str_bitres_params_long;
  ia_bitres_param_struct str_bitres_params_short;
  ia_adj_thr_elem_struct str_adj_thr_ele;
} ia_adj_thr_state_struct;
