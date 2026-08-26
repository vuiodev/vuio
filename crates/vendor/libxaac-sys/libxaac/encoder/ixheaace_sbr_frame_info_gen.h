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

#define IXHEAACE_MAX_ENV_VARVAR (IXHEAACE_MAX_ENV)
#define IXHEAACE_MAX_ENV_FIXVAR_VARFIX (4)
#define IXHEAACE_MAX_NUM_REL (3)
#define IXHEAACE_MAX_ABS_BORDERS (9)
#define IXHEAACE_DC (4711)
#define IXHEAACE_EMPTY (-99)
#define TIME_SLOTS_15 (15)
#define NUM_ENVELOPE_1 (1)
#define NUM_ENVELOPE_2 (2)
#define NUM_ENVELOPE_4 (4)

/* SBR frame class definitions */
typedef enum {
  IXHEAACE_FIXFIX = 0,
  IXHEAACE_FIXVAR,
  IXHEAACE_VARFIX,
  IXHEAACE_VARVAR
} ixheaace_frame_class;

typedef struct {
  ixheaace_frame_class frame_type;
  WORD32 bs_num_env;
  WORD32 bs_abs_bord;
  WORD32 n;
  WORD32 p;
  WORD32 bs_rel_bord[IXHEAACE_MAX_NUM_REL];
  ixheaace_freq_res v_f[IXHEAACE_MAX_ENV_FIXVAR_VARFIX];
  WORD32 bs_abs_bord_0;
  WORD32 bs_abs_bord_1;
  WORD32 bs_num_rel_0;
  WORD32 bs_num_rel_1;
  WORD32 bs_rel_bord_0[IXHEAACE_MAX_NUM_REL];
  WORD32 bs_rel_bord_1[IXHEAACE_MAX_NUM_REL];
  ixheaace_freq_res v_f_lr[IXHEAACE_MAX_ENV_VARVAR];
  WORD32 bs_transient_position;
  WORD32 abs_bord_vec[IXHEAACE_MAX_ABS_BORDERS];
} ixheaace_str_sbr_grid;

typedef ixheaace_str_sbr_grid *ixheaace_pstr_sbr_grid;

typedef ixheaace_str_frame_info_sbr *ixheaace_pstr_sbr_frame_info;

typedef struct {
  WORD32 static_framing;
  WORD32 num_env_static;
  ixheaace_freq_res freq_res_fix;
  WORD32 *ptr_v_tuning_segm;
  WORD32 dmin;
  WORD32 dmax;
  WORD32 allow_spread;
  ixheaace_frame_class frame_type_old;
  WORD32 spread_flag;
  WORD32 v_bord[2 * IXHEAACE_MAX_ENV_VARVAR + 1];
  WORD32 length_v_bord;
  ixheaace_freq_res v_freq[2 * IXHEAACE_MAX_ENV_VARVAR + 1];
  WORD32 length_v_freq;
  WORD32 v_bord_follow[IXHEAACE_MAX_ENV_VARVAR];
  WORD32 length_v_bord_follow;
  WORD32 i_tran_follow;
  WORD32 i_fill_follow;
  ixheaace_freq_res v_freq_follow[IXHEAACE_MAX_ENV_VARVAR];
  WORD32 length_v_freq_follow;
  ixheaace_str_sbr_grid sbr_grid;
  ixheaace_str_frame_info_sbr sbr_frame_info;
  WORD32 use_low_freq_res;
} ixheaace_str_sbr_env_frame;

typedef ixheaace_str_sbr_env_frame *ixheaace_pstr_sbr_env_frame;

VOID ixheaace_create_frame_info_generator(ixheaace_pstr_sbr_env_frame sbr_env_frame_handle,
                                          WORD32 allow_spread, WORD32 num_env_static,
                                          WORD32 static_framing, ixheaace_freq_res freq_res_fix,
                                          WORD32 use_low_freq_res);

IA_ERRORCODE
ixheaace_frame_info_generator(ixheaace_pstr_sbr_env_frame pstr_sbr_env_frame,
                              WORD32 *ptr_v_pre_transient_info, WORD32 *ptr_v_transient_info,
                              WORD32 *ptr_v_tuning, ixheaace_str_qmf_tabs *ptr_qmf_tab,
                              WORD32 num_time_slots, WORD32 is_ld_sbr,
                              ixheaace_pstr_sbr_frame_info *ptr_frame_info,
                              WORD32 flag_framelength_small);
