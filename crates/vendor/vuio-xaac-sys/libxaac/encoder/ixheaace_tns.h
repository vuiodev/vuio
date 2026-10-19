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
#define TEMPORAL_NOISE_SHAPING_MAX_ORDER (12)
#define TEMPORAL_NOISE_SHAPING_MAX_ORDER_SHORT (5)
#define TEMPORAL_NOISE_SHAPING_START_FREQ (1275)
#define TEMPORAL_NOISE_SHAPING_START_FREQ_SHORT (2750)
#define TEMPORAL_NOISE_SHAPING_COEF_RES (4)
#define TEMPORAL_NOISE_SHAPING_COEF_RES_SHORT (3)
#define FILTER_DIRECTION (0)
#define PI_BY_1000 (0.00314159265358979323f)
#define TEMPORAL_NOISE_SHAPING_MODIFY_BEGIN 2600 /* Hz */
#define RATIO_PATCH_LOWER_BORDER 380             /* Hz */
#define INT_BITS 32
typedef struct {
  WORD32 channel_bit_rate;
  WORD32 bandwidth_mono;
  WORD32 bandwidth_stereo;
} ixheaace_bandwidth_table;
typedef struct {
  FLOAT32 thresh_on;
  WORD32 lpc_start_freq;
  WORD32 lpc_stop_freq;
  FLOAT32 tns_time_resolution;
} ixheaace_temporal_noise_shaping_config_tabulated;

typedef struct {
  WORD8 tns_active;
  WORD32 tns_max_sfb;
  WORD32 max_order;
  WORD32 tns_start_freq;
  WORD32 coef_res;
  ixheaace_temporal_noise_shaping_config_tabulated conf_tab;
  FLOAT32 acf_window_float[TEMPORAL_NOISE_SHAPING_MAX_ORDER + 1];
  WORD32 tns_start_band;
  WORD32 tns_start_line;
  WORD32 tns_stop_band;
  WORD32 tns_stop_line;
  WORD32 lpc_start_band;
  WORD32 lpc_start_line;
  WORD32 lpc_stop_band;
  WORD32 lpc_stop_line;
  WORD32 tns_ratio_patch_lowest_cb;
  WORD32 tns_modify_begin_cb;
  FLOAT32 threshold;
} ixheaace_temporal_noise_shaping_config;

typedef struct {
  WORD8 tns_active;
  FLOAT32 parcor[TEMPORAL_NOISE_SHAPING_MAX_ORDER];
  FLOAT32 prediction_gain;
} ixheaace_temporal_noise_shaping_subblock_info_long;

typedef struct {
  WORD8 tns_active;
  FLOAT32 parcor[TEMPORAL_NOISE_SHAPING_MAX_ORDER];
  FLOAT32 prediction_gain;
} ixheaace_temporal_noise_shaping_subblock_info_short;

typedef struct {
  ixheaace_temporal_noise_shaping_subblock_info_short sub_block_info[TRANS_FAC];
} ixheaace_temporal_noise_shaping_data_short;

typedef struct {
  ixheaace_temporal_noise_shaping_subblock_info_long sub_block_info;
} ixheaace_temporal_noise_shaping_data_long;

typedef struct {
  ixheaace_temporal_noise_shaping_data_long tns_data_long;
  ixheaace_temporal_noise_shaping_data_short tns_data_short;
} ixheaace_temporal_noise_shaping_data_raw;

typedef struct {
  WORD32 numOfSubblocks;
  ixheaace_temporal_noise_shaping_data_raw data_raw;
} ixheaace_temporal_noise_shaping_data;

typedef struct {
  WORD8 tns_active[TRANS_FAC];
  WORD8 coef_res[TRANS_FAC];
  WORD32 length[TRANS_FAC];
  WORD32 order[TRANS_FAC];
  WORD32 coef[TRANS_FAC * TEMPORAL_NOISE_SHAPING_MAX_ORDER_SHORT];
} ixheaace_temporal_noise_shaping_params;
