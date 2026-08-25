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

#define BLOCK_SWITCH_WINDOWS TRANS_FAC /* number of windows for energy calculation */
#define BLOCK_SWITCH_WINDOW_LEN \
  FRAME_LEN_SHORT_128 /* minimal granularity of energy calculation */

#define BLK_SWITCH_WIN 8
#define BLK_SWITCH_FILT_LEN 2

/* Block types */
#define LONG_WINDOW 0
#define START_WINDOW 1
#define SHORT_WINDOW 2
#define STOP_WINDOW 3

/* Window shapes */
#define SINE_WINDOW 0
#define KBD_WINDOW 1

#define MAXIMUM_NO_OF_GROUPS 4

#define ACC_WINDOW_NRG_FAC 0.3f
#define ONE_MINUS_ACC_WINDOW_NRG_FAC 0.7f
#define INV_ATTACK_RATIO_HIGH_BR 0.1f
#define INV_ATTACK_RATIO_LOW_BR 0.056f
#define MIN_ATTACK_NRG (1000000)
#define BLK_SWITCH_HIGH_BR_MONO (24000)
#define BLK_SWITCH_HIGH_BR_STEREO (16000)

typedef struct {
  FLOAT32 inv_attack_ratio;
  WORD32 win_seq;
  WORD32 nxt_win_seq;
  WORD32 attack;
  WORD32 lastattack;
  WORD32 attack_idx;
  WORD32 last_attack_idx;
  WORD32 total_groups_cnt;
  WORD32 group_len[TRANS_FAC];
  FLOAT32 win_energy[BLK_SWITCH_FILT_LEN][BLK_SWITCH_WIN];
  FLOAT32 win_energy_filt[BLK_SWITCH_FILT_LEN][BLK_SWITCH_WIN];
  FLOAT32 iir_states[BLK_SWITCH_FILT_LEN];
  FLOAT32 max_win_energy;
  FLOAT32 acc_win_energy;
  WORD32 win_seq_ld;      /* these are used to save window decision for LD. */
  WORD32 next_win_seq_ld; /* nxt_win_seq and win_seq are always set to
                                 LONG_WINDOW */
} ixheaace_block_switch_control;

VOID iaace_init_block_switching(ixheaace_block_switch_control *pstr_blk_switch_ctrl,
                                const WORD32 bit_rate, const WORD32 num_chans);

VOID iaace_block_switching(ixheaace_block_switch_control *pstr_blk_switch_ctrl,
                           const FLOAT32 *ptr_time_signal, WORD32 frame_length, WORD32 num_chans);

VOID iaace_sync_block_switching(ixheaace_block_switch_control *pstr_blk_switch_left_ctrl,
                                ixheaace_block_switch_control *pstr_blk_switch_right_ctrl,
                                const WORD32 num_channels);
