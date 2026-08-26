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

#include "iusace_type_def.h"
#include "ixheaace_mps_common_define.h"
#include "iusace_cnst.h"
#include "iusace_block_switch_const.h"
#include "iusace_block_switch_struct_def.h"
#include "iusace_rom.h"

static FLOAT32 iusace_fmult(FLOAT32 a, FLOAT32 b) { return (a * b); }

static FLOAT32 iusace_fadd(FLOAT32 a, FLOAT32 b) { return (a + b); }

VOID iusace_init_block_switching(ia_block_switch_ctrl *pstr_blk_switch_ctrl,
                                 const WORD32 bit_rate, const WORD32 num_chans) {
  WORD32 i, w;

  if ((num_chans == 1 && bit_rate > 24000) || (num_chans > 1 && bit_rate / num_chans > 16000)) {
    pstr_blk_switch_ctrl->inv_attack_ratio = INV_ATTACK_RATIO_HIGH_BR;
  } else {
    pstr_blk_switch_ctrl->inv_attack_ratio = INV_ATTACK_RATIO_LOW_BR;
  }

  for (i = 0; i < BLK_SWITCH_FILT_LEN; i++) {
    pstr_blk_switch_ctrl->iir_states[i] = 0;
  }

  /* Clear Filtered Window Energies */
  for (w = 0; w < MAX_SHORT_WINDOWS; w++) {
    pstr_blk_switch_ctrl->win_energy_filt[0][w] = 0;
    pstr_blk_switch_ctrl->win_energy_filt[1][w] = 0;
    pstr_blk_switch_ctrl->win_energy[0][w] = 0;
    pstr_blk_switch_ctrl->win_energy[1][w] = 0;
  }
  pstr_blk_switch_ctrl->acc_win_energy = 0;

  pstr_blk_switch_ctrl->window_seq = ONLY_LONG_SEQUENCE;
  pstr_blk_switch_ctrl->next_win_seq = ONLY_LONG_SEQUENCE;

  pstr_blk_switch_ctrl->attack = 0;
  pstr_blk_switch_ctrl->lastattack = 0;
  pstr_blk_switch_ctrl->attack_idx = 0;
  pstr_blk_switch_ctrl->last_attack_idx = 0;

  return;
}

static FLOAT32 iusace_srch_max_with_idx(const FLOAT32 *ptr_in, WORD32 *index) {
  FLOAT32 max;
  WORD32 i, idx;

  max = 0;
  idx = 0;

  for (i = 0; i < MAX_SHORT_WINDOWS; i++) {
    if (ptr_in[i + 1] > max) {
      max = ptr_in[i + 1];
      idx = i;
    }
  }
  *index = idx;

  return max;
}

static VOID iusace_blk_switch_iir_filt(const FLOAT32 *ptr_in, const FLOAT32 *ptr_iir_coeff,
                                       const WORD32 w, FLOAT32 *ptr_iir_states,
                                       FLOAT32 *energy_accu, WORD32 block_len) {
  FLOAT32 accu1;

  WORD32 i;

  FLOAT32 accu_unfilt = 0.0f;
  FLOAT32 accu_filt = 0.0f;
  FLOAT32 accu2, temp2, temp1;

  FLOAT32 state0 = ptr_iir_states[0];
  FLOAT32 state1 = ptr_iir_states[1];

  FLOAT32 coeff0 = ptr_iir_coeff[0];
  FLOAT32 coeff1 = ptr_iir_coeff[1];

  const FLOAT32 *p_time_signal = &ptr_in[(block_len * w)];

  for (i = 0; i < block_len; i++) {
    accu2 = iusace_fmult(state0, coeff1);
    accu1 = iusace_fmult(state1, coeff0);
    accu1 += accu2;

    state0 = p_time_signal[i];
    state1 = iusace_fmult(state0, coeff1);
    state1 = (state1 - accu1);

    temp1 = iusace_fmult(state0, state0);
    temp2 = iusace_fmult(state1, state1);

    accu_unfilt = iusace_fadd(accu_unfilt, temp1);
    accu_filt = iusace_fadd(accu_filt, temp2);
  }

  energy_accu[0] = accu_unfilt;
  energy_accu[1] = accu_filt;

  ptr_iir_states[0] = state0;
  ptr_iir_states[1] = state1;

  return;
}

static VOID iusace_calc_window_energy(ia_block_switch_ctrl *ptr_blk_switch_ctrl,
                                      const FLOAT32 *ptr_in, FLOAT32 *max, WORD32 ccfl) {
  WORD32 w;

  FLOAT32 energy_accu[2];
  *max = 0.0f;

  for (w = 0; w < MAX_SHORT_WINDOWS; w++) {
    // block length for calculating energy is corecoder frame length / MAX_SHORT_WINDOWS
    iusace_blk_switch_iir_filt(ptr_in, iusace_iir_hipass_coeffs, w,
                               ptr_blk_switch_ctrl->iir_states, &energy_accu[0], ccfl >> 3);

    ptr_blk_switch_ctrl->win_energy[1][w] = energy_accu[0];
    ptr_blk_switch_ctrl->win_energy_filt[1][w] = energy_accu[1];

    if (ptr_blk_switch_ctrl->win_energy_filt[1][w] > *max)
      *max = ptr_blk_switch_ctrl->win_energy_filt[1][w];
  }
  return;
}

VOID iusace_block_switching(ia_block_switch_ctrl *ptr_blk_switch_ctrl, const FLOAT32 *ptr_in,
                            WORD32 ccfl) {
  WORD32 i;

  FLOAT32 temp1, temp2;
  FLOAT32 max;
  FLOAT32 energy, energy_max;

  for (i = 0; i < MAX_SHORT_WINDOWS; i++) {
    ptr_blk_switch_ctrl->group_len[i] = 0;
  }

  ptr_blk_switch_ctrl->max_win_energy =
      iusace_srch_max_with_idx(&ptr_blk_switch_ctrl->win_energy[0][MAX_SHORT_WINDOWS - 1],
                               &ptr_blk_switch_ctrl->attack_idx);

  ptr_blk_switch_ctrl->attack_idx = ptr_blk_switch_ctrl->last_attack_idx;
  ptr_blk_switch_ctrl->tot_grps_cnt = MAXIMUM_NO_OF_GROUPS;

  for (i = 0; i < MAXIMUM_NO_OF_GROUPS; i++) {
    ptr_blk_switch_ctrl->group_len[i] =
        iusace_suggested_grouping_table[ptr_blk_switch_ctrl->attack_idx][i];
  }

  for (i = 0; i < MAX_SHORT_WINDOWS; i++) {
    ptr_blk_switch_ctrl->win_energy[0][i] = ptr_blk_switch_ctrl->win_energy[1][i];
    ptr_blk_switch_ctrl->win_energy_filt[0][i] = ptr_blk_switch_ctrl->win_energy_filt[1][i];
  }

  iusace_calc_window_energy(ptr_blk_switch_ctrl, ptr_in, &max, ccfl);

  ptr_blk_switch_ctrl->attack = FALSE;

  energy_max = 0.0f;

  energy = ptr_blk_switch_ctrl->win_energy_filt[0][MAX_SHORT_WINDOWS - 1];

  for (i = 0; i < MAX_SHORT_WINDOWS; i++) {
    temp1 = iusace_fmult(ONE_MINUS_ACC_WINDOW_NRG_FAC, ptr_blk_switch_ctrl->acc_win_energy);
    temp2 = iusace_fmult(ACC_WINDOW_NRG_FAC, energy);
    ptr_blk_switch_ctrl->acc_win_energy = iusace_fadd(temp1, temp2);

    temp1 = iusace_fmult(ptr_blk_switch_ctrl->win_energy_filt[1][i],
                         ptr_blk_switch_ctrl->inv_attack_ratio);
    if (temp1 > ptr_blk_switch_ctrl->acc_win_energy) {
      ptr_blk_switch_ctrl->attack = TRUE;
      ptr_blk_switch_ctrl->last_attack_idx = i;
    }

    energy = ptr_blk_switch_ctrl->win_energy_filt[1][i];
    if (energy_max < energy) energy_max = energy;
  }

  if (ccfl == LEN_SUPERFRAME_768) {
    energy_max = (energy_max * 4) / 3.0f;
  }
  if (energy_max < USAC_MIN_ATTACK_NRG) {
    ptr_blk_switch_ctrl->attack = FALSE;
  }

  if ((!ptr_blk_switch_ctrl->attack) && (ptr_blk_switch_ctrl->lastattack)) {
    if (ptr_blk_switch_ctrl->attack_idx == MAX_SHORT_WINDOWS - 1) {
      ptr_blk_switch_ctrl->attack = TRUE;
    }
    ptr_blk_switch_ctrl->lastattack = FALSE;
  } else {
    ptr_blk_switch_ctrl->lastattack = ptr_blk_switch_ctrl->attack;
  }
  ptr_blk_switch_ctrl->window_seq = ptr_blk_switch_ctrl->next_win_seq;

  if (ptr_blk_switch_ctrl->attack) {
    ptr_blk_switch_ctrl->next_win_seq = EIGHT_SHORT_SEQUENCE;
  } else {
    ptr_blk_switch_ctrl->next_win_seq = ONLY_LONG_SEQUENCE;
  }
  if (ptr_blk_switch_ctrl->next_win_seq == EIGHT_SHORT_SEQUENCE) {
    if (ptr_blk_switch_ctrl->window_seq == ONLY_LONG_SEQUENCE) {
      ptr_blk_switch_ctrl->window_seq = LONG_START_SEQUENCE;
    }

    if (ptr_blk_switch_ctrl->window_seq == LONG_STOP_SEQUENCE) {
      ptr_blk_switch_ctrl->window_seq = EIGHT_SHORT_SEQUENCE;
      ptr_blk_switch_ctrl->tot_grps_cnt = 3;
      ptr_blk_switch_ctrl->group_len[0] = 3;
      ptr_blk_switch_ctrl->group_len[1] = 3;
      ptr_blk_switch_ctrl->group_len[2] = 2;
    }
  }

  if (ptr_blk_switch_ctrl->next_win_seq == ONLY_LONG_SEQUENCE) {
    if (ptr_blk_switch_ctrl->window_seq == EIGHT_SHORT_SEQUENCE) {
      ptr_blk_switch_ctrl->next_win_seq = LONG_STOP_SEQUENCE;
    }
  }
  return;
}

VOID iusace_sync_block_switching(ia_block_switch_ctrl *ptr_blk_switch_left_ctrl,
                                 ia_block_switch_ctrl *ptr_blk_switch_right_ctrl) {
  WORD32 i;
  WORD32 patch_type = ONLY_LONG_SEQUENCE;

  patch_type = iusace_synchronized_block_types[patch_type][ptr_blk_switch_left_ctrl->window_seq];
  patch_type = iusace_synchronized_block_types[patch_type][ptr_blk_switch_right_ctrl->window_seq];

  ptr_blk_switch_left_ctrl->window_seq = patch_type;
  ptr_blk_switch_right_ctrl->window_seq = patch_type;

  if (patch_type != EIGHT_SHORT_SEQUENCE) { /* tns_data_long Blocks */
    ptr_blk_switch_left_ctrl->tot_grps_cnt = 1;
    ptr_blk_switch_right_ctrl->tot_grps_cnt = 1;
    ptr_blk_switch_left_ctrl->group_len[0] = 1;
    ptr_blk_switch_right_ctrl->group_len[0] = 1;

    for (i = 1; i < MAX_SHORT_WINDOWS; i++) {
      ptr_blk_switch_left_ctrl->group_len[i] = 0;
      ptr_blk_switch_right_ctrl->group_len[i] = 0;
    }
  } else { /* tns_data_short Blocks */
    if (ptr_blk_switch_left_ctrl->max_win_energy > ptr_blk_switch_right_ctrl->max_win_energy) {
      ptr_blk_switch_right_ctrl->tot_grps_cnt = ptr_blk_switch_left_ctrl->tot_grps_cnt;
      for (i = 0; i < ptr_blk_switch_right_ctrl->tot_grps_cnt; i++) {
        ptr_blk_switch_right_ctrl->group_len[i] = ptr_blk_switch_left_ctrl->group_len[i];
      }
    } else {
      ptr_blk_switch_left_ctrl->tot_grps_cnt = ptr_blk_switch_right_ctrl->tot_grps_cnt;
      for (i = 0; i < ptr_blk_switch_left_ctrl->tot_grps_cnt; i++) {
        ptr_blk_switch_left_ctrl->group_len[i] = ptr_blk_switch_right_ctrl->group_len[i];
      }
    }
  }

  return;
}
