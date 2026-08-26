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
  FLOAT32 inv_attack_ratio;
  WORD32 window_seq;
  WORD32 next_win_seq;
  WORD32 attack;
  WORD32 lastattack;
  WORD32 attack_idx;
  WORD32 last_attack_idx;

  WORD32 tot_grps_cnt;
  WORD32 group_len[TRANS_FAC];

  FLOAT32 win_energy[2][BLK_SWITCH_WIN];
  FLOAT32 win_energy_filt[2][BLK_SWITCH_WIN];
  FLOAT32 iir_states[BLK_SWITCH_FILT_LEN];
  FLOAT32 max_win_energy;
  FLOAT32 acc_win_energy;
} ia_block_switch_ctrl;
