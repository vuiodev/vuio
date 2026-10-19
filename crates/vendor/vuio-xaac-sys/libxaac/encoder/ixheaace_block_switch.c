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

#include "ixheaac_type_def.h"
#include "ixheaac_constants.h"
#include "ixheaace_aac_constants.h"
#include "ixheaace_psy_const.h"
#include "ixheaace_tns.h"
#include "ixheaace_tns_params.h"
#include "ixheaace_rom.h"
#include "ixheaace_common_rom.h"
#include "ixheaace_bitbuffer.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaac_basic_ops.h"
#include "ixheaace_block_switch.h"
#include "iusace_block_switch_struct_def.h"
#include <string.h>

static FLOAT32 iaace_fmult(FLOAT32 a, FLOAT32 b) { return (a * b); }

static FLOAT32 iaace_fadd(FLOAT32 a, FLOAT32 b) { return (a + b); }

VOID iaace_init_block_switching(ixheaace_block_switch_control *pstr_blk_switch_ctrl,
                                const WORD32 bit_rate, const WORD32 num_chans) {
  if ((num_chans == 1 && bit_rate > BLK_SWITCH_HIGH_BR_MONO) ||
      (num_chans > 1 && bit_rate / num_chans > BLK_SWITCH_HIGH_BR_STEREO)) {
    pstr_blk_switch_ctrl->inv_attack_ratio = INV_ATTACK_RATIO_HIGH_BR;
  } else {
    pstr_blk_switch_ctrl->inv_attack_ratio = INV_ATTACK_RATIO_LOW_BR;
  }

  memset(pstr_blk_switch_ctrl->iir_states, 0,
         BLK_SWITCH_FILT_LEN * sizeof(pstr_blk_switch_ctrl->iir_states[0]));
  /* Clear Filtered Window Energies */

  memset(pstr_blk_switch_ctrl->win_energy_filt, 0,
         BLK_SWITCH_WIN * sizeof(pstr_blk_switch_ctrl->win_energy_filt[0][0]));
  memset(pstr_blk_switch_ctrl->win_energy, 0,
         BLK_SWITCH_WIN * sizeof(pstr_blk_switch_ctrl->win_energy[0][0]));

  pstr_blk_switch_ctrl->acc_win_energy = 0;

  pstr_blk_switch_ctrl->win_seq = LONG_WINDOW;
  pstr_blk_switch_ctrl->nxt_win_seq = LONG_WINDOW;

  pstr_blk_switch_ctrl->attack = 0;
  pstr_blk_switch_ctrl->lastattack = 0;
  pstr_blk_switch_ctrl->attack_idx = 0;
  pstr_blk_switch_ctrl->last_attack_idx = 0;
}

static FLOAT32 iaace_search_max_with_idx(const FLOAT32 *ptr_in, WORD32 *ptr_index, WORD32 len) {
  FLOAT32 max = 0;
  WORD32 i = 0, idx = 0;

  do {
    if (ptr_in[i + 1] > max) {
      max = ptr_in[i + 1];
      idx = i;
    }
    i++;
  } while (i < len);

  *ptr_index = idx;

  return max;
}

static FLOAT32 iaace_blk_switch_iir_filt(const FLOAT32 input, const FLOAT32 *ptr_iir_coeff,
                                         FLOAT32 *ptr_iir_states) {
  FLOAT32 accu_1, accu_2;
  FLOAT32 out;

  accu_1 = ptr_iir_coeff[1] * (input - ptr_iir_states[0]);

  accu_2 = ptr_iir_coeff[0] * ptr_iir_states[1];

  out = accu_1 - accu_2;
  ptr_iir_states[0] = input;
  ptr_iir_states[1] = out;

  return out;
}

static VOID iaace_calc_window_energy(ixheaace_block_switch_control *pstr_blk_switch_ctrl,
                                     const FLOAT32 *ptr_time_signal, WORD32 ch_increment,
                                     WORD32 win_len) {
  WORD32 i, w;
  FLOAT32 acc_nrg_unfilt, acc_nrg_filt;
  FLOAT32 tmp_nrg_unfilt, tmp_nrg_filt;
  for (w = 0; w < BLK_SWITCH_WIN; w++) {
    acc_nrg_unfilt = 0.0f;
    acc_nrg_filt = 0.0f;

    for (i = 0; i < win_len; i++) {
      tmp_nrg_unfilt = ptr_time_signal[(win_len * w + i) * ch_increment];
      tmp_nrg_filt = iaace_blk_switch_iir_filt(tmp_nrg_unfilt, iaace_iir_hipass_coeffs,
                                               pstr_blk_switch_ctrl->iir_states);

      acc_nrg_unfilt += (tmp_nrg_unfilt * tmp_nrg_unfilt);
      acc_nrg_filt += (tmp_nrg_filt * tmp_nrg_filt);
    }

    pstr_blk_switch_ctrl->win_energy[1][w] = acc_nrg_unfilt;
    pstr_blk_switch_ctrl->win_energy_filt[1][w] = acc_nrg_filt;
  }
}

VOID iaace_block_switching(ixheaace_block_switch_control *pstr_blk_switch_ctrl,
                           const FLOAT32 *ptr_time_signal, WORD32 frame_length,
                           WORD32 num_chans) {
  WORD32 win_idx;
  FLOAT32 tmp_nrg_1, tmp_nrg_2;
  FLOAT32 prev_win_nrg, max_nrg;

  memset(pstr_blk_switch_ctrl->group_len, 0,
         TRANS_FAC * sizeof(pstr_blk_switch_ctrl->group_len[0]));

  pstr_blk_switch_ctrl->max_win_energy =
      iaace_search_max_with_idx(&pstr_blk_switch_ctrl->win_energy[0][BLK_SWITCH_WIN - 1],
                                &pstr_blk_switch_ctrl->attack_idx, BLK_SWITCH_WIN);

  pstr_blk_switch_ctrl->attack_idx = pstr_blk_switch_ctrl->last_attack_idx;
  pstr_blk_switch_ctrl->total_groups_cnt = MAXIMUM_NO_OF_GROUPS;

  memcpy(pstr_blk_switch_ctrl->group_len,
         iaace_suggested_grouping_table[pstr_blk_switch_ctrl->attack_idx],
         MAXIMUM_NO_OF_GROUPS * sizeof(pstr_blk_switch_ctrl->group_len[0]));

  memcpy(pstr_blk_switch_ctrl->win_energy[0], pstr_blk_switch_ctrl->win_energy[1],
         BLK_SWITCH_WIN * sizeof(pstr_blk_switch_ctrl->win_energy[0][0]));
  memcpy(pstr_blk_switch_ctrl->win_energy_filt[0], pstr_blk_switch_ctrl->win_energy_filt[1],
         BLK_SWITCH_WIN * sizeof(pstr_blk_switch_ctrl->win_energy_filt[0][0]));

  iaace_calc_window_energy(
      pstr_blk_switch_ctrl, ptr_time_signal, num_chans,
      (frame_length == FRAME_LEN_960 ? FRAME_LEN_SHORT_120 : FRAME_LEN_SHORT_128));

  pstr_blk_switch_ctrl->attack = FALSE;

  max_nrg = 0.0f;

  prev_win_nrg = pstr_blk_switch_ctrl->win_energy_filt[0][BLK_SWITCH_WIN - 1];

  for (win_idx = 0; win_idx < BLK_SWITCH_WIN; win_idx++) {
    tmp_nrg_1 = iaace_fmult(ONE_MINUS_ACC_WINDOW_NRG_FAC, pstr_blk_switch_ctrl->acc_win_energy);
    tmp_nrg_2 = iaace_fmult(ACC_WINDOW_NRG_FAC, prev_win_nrg);
    pstr_blk_switch_ctrl->acc_win_energy = iaace_fadd(tmp_nrg_1, tmp_nrg_2);

    tmp_nrg_1 = iaace_fmult(pstr_blk_switch_ctrl->win_energy_filt[1][win_idx],
                            pstr_blk_switch_ctrl->inv_attack_ratio);
    if (tmp_nrg_1 > pstr_blk_switch_ctrl->acc_win_energy) {
      pstr_blk_switch_ctrl->attack = TRUE;
      pstr_blk_switch_ctrl->last_attack_idx = win_idx;
    }

    prev_win_nrg = pstr_blk_switch_ctrl->win_energy_filt[1][win_idx];

    max_nrg = MAX(prev_win_nrg, max_nrg);
  }

  if (max_nrg < MIN_ATTACK_NRG) {
    pstr_blk_switch_ctrl->attack = FALSE;
  }

  if ((!pstr_blk_switch_ctrl->attack) && (pstr_blk_switch_ctrl->lastattack)) {
    if (pstr_blk_switch_ctrl->attack_idx == (TRANS_FAC - 1)) {
      pstr_blk_switch_ctrl->attack = TRUE;
    }
    pstr_blk_switch_ctrl->lastattack = FALSE;
  } else {
    pstr_blk_switch_ctrl->lastattack = pstr_blk_switch_ctrl->attack;
  }
  pstr_blk_switch_ctrl->win_seq = pstr_blk_switch_ctrl->nxt_win_seq;

  pstr_blk_switch_ctrl->nxt_win_seq =
      (pstr_blk_switch_ctrl->attack == 1) ? SHORT_WINDOW : LONG_WINDOW;

  if (pstr_blk_switch_ctrl->nxt_win_seq == SHORT_WINDOW) {
    if (pstr_blk_switch_ctrl->win_seq == LONG_WINDOW) {
      pstr_blk_switch_ctrl->win_seq = START_WINDOW;
    } else if (pstr_blk_switch_ctrl->win_seq == STOP_WINDOW) {
      pstr_blk_switch_ctrl->win_seq = SHORT_WINDOW;
      pstr_blk_switch_ctrl->total_groups_cnt = 3;
      pstr_blk_switch_ctrl->group_len[0] = 3;
      pstr_blk_switch_ctrl->group_len[1] = 3;
      pstr_blk_switch_ctrl->group_len[2] = 2;
    }
  }

  if (pstr_blk_switch_ctrl->nxt_win_seq == LONG_WINDOW &&
      pstr_blk_switch_ctrl->win_seq == SHORT_WINDOW) {
    pstr_blk_switch_ctrl->nxt_win_seq = STOP_WINDOW;
  }
}

VOID iaace_sync_block_switching(ixheaace_block_switch_control *pstr_blk_switch_left_ctrl,
                                ixheaace_block_switch_control *pstr_blk_switch_right_ctrl,
                                const WORD32 num_channels) {
  WORD32 i;
  WORD32 patch_type = LONG_WINDOW;

  if (num_channels == 1) {
    if (pstr_blk_switch_left_ctrl->win_seq != SHORT_WINDOW) {
      pstr_blk_switch_left_ctrl->total_groups_cnt = 1;
      pstr_blk_switch_left_ctrl->group_len[0] = 1;

      for (i = 1; i < TRANS_FAC; i++) {
        pstr_blk_switch_left_ctrl->group_len[i] = 0;
      }
    }
  } else {
    /* Stereo */

    patch_type = iaace_synchronized_block_types[patch_type][pstr_blk_switch_left_ctrl->win_seq];
    patch_type = iaace_synchronized_block_types[patch_type][pstr_blk_switch_right_ctrl->win_seq];

    pstr_blk_switch_left_ctrl->win_seq = patch_type;
    pstr_blk_switch_right_ctrl->win_seq = patch_type;

    if (patch_type != SHORT_WINDOW) { /* tns_data_long Blocks */
      pstr_blk_switch_left_ctrl->total_groups_cnt = 1;
      pstr_blk_switch_right_ctrl->total_groups_cnt = 1;
      pstr_blk_switch_left_ctrl->group_len[0] = 1;
      pstr_blk_switch_right_ctrl->group_len[0] = 1;

      for (i = 1; i < TRANS_FAC; i++) {
        pstr_blk_switch_left_ctrl->group_len[i] = 0;
        pstr_blk_switch_right_ctrl->group_len[i] = 0;
      }
    } else { /* tns_data_short Blocks */
      if (pstr_blk_switch_left_ctrl->max_win_energy >
          pstr_blk_switch_right_ctrl->max_win_energy) {
        pstr_blk_switch_right_ctrl->total_groups_cnt =
            pstr_blk_switch_left_ctrl->total_groups_cnt;
        for (i = 0; i < TRANS_FAC; i++) {
          pstr_blk_switch_right_ctrl->group_len[i] = pstr_blk_switch_left_ctrl->group_len[i];
        }
      } else {
        pstr_blk_switch_left_ctrl->total_groups_cnt =
            pstr_blk_switch_right_ctrl->total_groups_cnt;
        for (i = 0; i < TRANS_FAC; i++) {
          pstr_blk_switch_left_ctrl->group_len[i] = pstr_blk_switch_right_ctrl->group_len[i];
        }
      }
    }
  }
}
