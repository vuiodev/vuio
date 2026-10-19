/******************************************************************************
 *
 * Copyright (C) 2015 The Android Open Source Project
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
#ifndef IMPD_DRC_UNI_TABLES_H
#define IMPD_DRC_UNI_TABLES_H

#ifdef __cplusplus
extern "C" {
#endif

#define N_DELTA_TIME_CODE_TABLE_ENTRIES_MAX (512 + 14)

typedef struct {
  WORD32 size;
  WORD32 code;
  WORD32 value;
} ia_delta_time_code_table_entry_struct;

typedef struct {
  WORD32 size;
  WORD32 code;
  FLOAT32 value;
  WORD32 index;
} ia_slope_code_table_struct;

typedef struct {
  WORD32 size;
  WORD32 code;
  FLOAT32 value;
} ia_delta_gain_code_table_struct;

typedef struct {
  ia_delta_time_code_table_entry_struct
      delta_time_code_table[N_DELTA_TIME_CODE_TABLE_ENTRIES_MAX];
} ia_tables_struct;

typedef struct {
  FLOAT32 in_out_ratio;
  FLOAT32 exp_lo;
  FLOAT32 exp_hi;
} ia_cicp_sigmoid_characteristic_param_struct;

typedef struct {
  FLOAT32 inLevel;
  FLOAT32 gain;
} ia_characteristic_node_coordinate_struct;

typedef struct {
  WORD32 coordinateCount;
  ia_characteristic_node_coordinate_struct characteristicNodeCoordinate[5];
} ia_cicp_node_characteristic_param;

WORD32
impd_init_tbls(const WORD32 num_gain_max_values, ia_tables_struct* str_tables);

void impd_gen_delta_time_code_tbl(
    const WORD32 num_gain_max_values,
    ia_delta_time_code_table_entry_struct* delta_time_code_tbl_item);

void impd_get_delta_gain_code_tbl(
    const WORD32 gain_coding_profile,
    ia_delta_gain_code_table_struct const** delta_time_code_tbl,
    WORD32* num_entries);

void impd_get_slope_code_tbl_and_size(
    ia_slope_code_table_struct const** slope_code_tbl_entry,
    WORD32* num_slope_code_tbl_entries);

WORD32
impd_get_delta_tmin(const WORD32 sampling_rate);

#ifdef __cplusplus
}
#endif
#endif
