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
struct ixheaace_mps_frame_win {
  WORD32 num_time_slots_max;
  WORD32 frame_keep_flag;
  WORD32 start_slope;
  WORD32 stop_slope;
  WORD32 start_rect;
  WORD32 stop_rect;

  WORD32 taper_ana_len;
  WORD32 taper_syn_len;

  FLOAT32 p_taper_ana_flt[MAX_TIME_SLOTS];
  FLOAT32 p_tapper_sync_flt[MAX_TIME_SLOTS];
};

struct ixheaace_mps_frame_win_config {
  WORD32 num_time_slots_max;
  WORD32 frame_keep_flag;
};

typedef struct {
  WORD32 slot;
  WORD32 hold;

} ixheaace_mps_frame_win_data;

struct ixheaace_mps_frame_win_list {
  ixheaace_mps_frame_win_data dat[MAX_NUM_PARAMS];
  WORD32 win_list_cnt;
};

typedef struct ixheaace_mps_frame_win ixheaace_mps_frame_win, *ixheaace_mps_pstr_frame_win;

typedef struct ixheaace_mps_frame_win_config ixheaace_mps_frame_win_config,
    *ixheaace_mps_pstr_frame_win_config;

typedef struct ixheaace_mps_frame_win_list ixheaace_mps_frame_win_list,
    *ixheaace_mps_pstr_frame_win_list;

IA_ERRORCODE ixheaace_mps_212_frame_window_init(
    ixheaace_mps_pstr_frame_win pstr_frame_window,
    const ixheaace_mps_pstr_frame_win_config pstr_frame_window_config);

IA_ERRORCODE ixheaace_mps_212_frame_window_get_window(
    ixheaace_mps_pstr_frame_win pstr_frame_window, WORD32 tr_pos[MAX_NUM_PARAMS],
    const WORD32 time_slots, ixheaace_mps_framing_info *const pstr_framing_info,
    FLOAT32 *p_window_ana_mps[MAX_NUM_PARAMS],
    ixheaace_mps_pstr_frame_win_list const p_frame_win_list, const WORD32 avoid_keep);

VOID ixheaace_mps_212_analysis_windowing(
    const WORD32 num_time_slots, const WORD32 start_time_slot, FLOAT32 *p_frame_window_ana_mps,
    ixheaace_cmplx_str pp_cmplx_data_in[MAX_ANA_TIME_SLOT][MAX_QMF_BANDS],
    ixheaace_cmplx_str pp_cmplx_data_out[MAX_ANA_TIME_SLOT][MAX_QMF_BANDS],
    const WORD32 num_hybrid_bands);
