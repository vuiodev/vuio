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
extern const FLOAT32 impd_drc_downmix_coeff[16];
extern const FLOAT32 impd_drc_downmix_coeff_lfe[16];
extern const FLOAT32 impd_drc_channel_weight[16];
extern const FLOAT32 impd_drc_downmix_coeff_v1[32];
extern const FLOAT32 impd_drc_eq_slope_table[16];
extern const FLOAT32 impd_drc_eq_gain_delta_table[32];
extern const FLOAT32 impd_drc_zero_pole_radius_table[128];
extern const FLOAT32 impd_drc_zero_pole_angle_table[128];

typedef struct {
  WORD32 size;
  WORD32 code;
  WORD32 value;
} ia_drc_delta_time_code_table_entry_struct;

typedef struct {
  WORD32 size;
  WORD32 code;
  FLOAT32 value;
  WORD32 index;
} ia_drc_slope_code_table_entry_struct;

typedef struct {
  WORD32 size;
  WORD32 code;
  FLOAT32 value;
} ia_drc_delta_gain_code_entry_struct;

VOID impd_drc_generate_delta_time_code_table(
    const WORD32 num_gain_values_max,
    ia_drc_delta_time_code_table_entry_struct *delta_time_code_table_item);

VOID impd_drc_get_delta_gain_code_table(
    const WORD32 gain_coding_profile,
    ia_drc_delta_gain_code_entry_struct const **pstr_delta_gain_code_table, WORD32 *num_entries);

const ia_drc_slope_code_table_entry_struct *impd_drc_get_slope_code_table_by_value(VOID);

FLOAT32 impd_drc_decode_slope_idx_value(const WORD32 slope_code_index);

FLOAT32 impd_drc_decode_slope_idx_magnitude(const WORD32 slope_code_index);
