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

#include <stdio.h>
#include <string.h>
#include <math.h>
#include "ixheaac_type_def.h"
#include "ixheaac_constants.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops16.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaac_basic_ops.h"
#include "ixheaacd_sbr_common.h"
#include "ixheaac_error_standards.h"

#include <ixheaacd_cnst.h>
#include "ixheaac_constants.h"
#include "ixheaacd_intrinsics.h"
#include "ixheaacd_common_rom.h"
#include "ixheaacd_sbrdecsettings.h"
#include "ixheaacd_bitbuffer.h"
#include "ixheaacd_defines.h"

#include "ixheaacd_pns.h"

#include <ixheaacd_aac_rom.h>
#include "ixheaacd_pulsedata.h"

#include "ixheaacd_drc_data_struct.h"

#include "ixheaacd_interface.h"
#include "ixheaacd_info.h"
#include "ixheaacd_lt_predict.h"
#include "ixheaacd_ec_defines.h"
#include "ixheaacd_ec_rom.h"
#include "ixheaacd_ec_struct_def.h"
#include "ixheaacd_channelinfo.h"
#include "ixheaac_error_standards.h"

#include "ixheaacd_aac_rom.h"

static WORD32 ixheaacd_aac_ec_get_win_seq(WORD32 prev_win_seq) {
  WORD32 new_win_seq = ONLY_LONG_SEQUENCE;

  if (prev_win_seq == LONG_START_SEQUENCE || prev_win_seq == EIGHT_SHORT_SEQUENCE) {
    new_win_seq = LONG_STOP_SEQUENCE;
  }

  return new_win_seq;
}

static VOID ixheaacd_aac_ec_flip_spec_sign(WORD32 *ptr_spec_coeff, WORD32 num_samples) {
  WORD32 idx;
  WORD32 random_value;

  for (idx = 0; idx < num_samples; idx++) {
    random_value = ptr_spec_coeff[idx] ^ idx;
    if ((random_value & 1) == 0) {
      ptr_spec_coeff[idx] = ixheaac_negate32_sat(ptr_spec_coeff[idx]);
    }
  }
}

static VOID ixheaacd_aac_ec_store(ia_ec_state_str *pstr_ec_state,
                                  ia_aac_dec_channel_info_struct *pstr_aac_dec_channel_info,
                                  ia_ics_info_struct *pstr_ics_info) {
  WORD32 *ptr_spec_coeff = pstr_aac_dec_channel_info->ptr_spec_coeff;
  WORD16 *ptr_spec_scale = pstr_aac_dec_channel_info->ptr_scale_factor;
  UWORD8 win_shape = pstr_ec_state->win_shape;
  WORD32 win_seq = pstr_ec_state->win_seq;
  WORD16 q_spec_scale[MAX_SPEC_SCALE_LEN];
  WORD32 *ptr_temp_spec_coeff = &pstr_ec_state->str_ec_scratch.spec_coeff[0];

  memcpy(q_spec_scale, pstr_ec_state->q_spec_coeff, sizeof(q_spec_scale));

  pstr_ec_state->win_seq = pstr_ics_info->window_sequence;
  pstr_ec_state->win_shape = (UWORD8)pstr_ics_info->window_shape;
  pstr_ec_state->prev_win_group_len =
      *(pstr_ics_info->window_group_length + pstr_ics_info->num_window_groups - 1);

  memcpy(pstr_ec_state->q_spec_coeff, ptr_spec_scale, sizeof(pstr_ec_state->q_spec_coeff));

  memcpy(ptr_temp_spec_coeff, ptr_spec_coeff, LEN_SUPERFRAME * sizeof(ptr_temp_spec_coeff[0]));
  memcpy(ptr_spec_coeff, pstr_ec_state->spectral_coeff,
         LEN_SUPERFRAME * sizeof(ptr_spec_coeff[0]));
  memcpy(pstr_ec_state->spectral_coeff, ptr_temp_spec_coeff,
         sizeof(pstr_ec_state->spectral_coeff));
  pstr_ics_info->window_sequence = win_seq;
  pstr_ics_info->window_shape = win_shape;

  memcpy(ptr_spec_scale, q_spec_scale, MAX_SPEC_SCALE_LEN * sizeof(ptr_spec_scale[0]));
}

static VOID ixheaacd_aac_ec_calc_sfb_nrg(WORD32 *ptr_spec_coeff,
                                         const ia_usac_samp_rate_info *pstr_samp_rate_info,
                                         const WORD32 win_seq, WORD32 win_trans,
                                         WORD32 *ptr_sfb_energy) {
  const WORD16 *ptr_sfb_offset;
  WORD32 line = 0, sfb, total_scale_factor_bands = 0;

  switch (win_seq) {
    case EIGHT_SHORT_SEQUENCE:

      if (win_trans == NO_TRANSITION) {
        total_scale_factor_bands = pstr_samp_rate_info->num_sfb_128 - 1;
        ptr_sfb_offset = pstr_samp_rate_info->ptr_sfb_128;

        for (sfb = 0; sfb < total_scale_factor_bands; sfb++) {
          WORD32 accu = 1;
          WORD32 q_nrg;
          if (sfb == 0) {
            q_nrg = (sizeof(accu) << 3) - ixheaac_norm32(ptr_sfb_offset[sfb] - 0);
            for (; line < ptr_sfb_offset[sfb]; line++) {
              accu += ixheaac_mult32(ptr_spec_coeff[line], ptr_spec_coeff[line]) >> q_nrg;
            }
            ptr_sfb_energy[sfb] = ixheaac_norm32(accu);
          }
          q_nrg = (sizeof(accu) << 3) -
                  ixheaac_norm32(ptr_sfb_offset[sfb + 1] - ptr_sfb_offset[sfb]);
          for (; line < ptr_sfb_offset[sfb + 1]; line++) {
            accu += ixheaac_mult32(ptr_spec_coeff[line], ptr_spec_coeff[line]) >> q_nrg;
          }
          ptr_sfb_energy[sfb] = ixheaac_norm32(accu);
        }
      } else {
        total_scale_factor_bands = pstr_samp_rate_info->num_sfb_1024 - 1;
        ptr_sfb_offset = pstr_samp_rate_info->ptr_sfb_1024;

        for (sfb = 0; sfb < total_scale_factor_bands; sfb++) {
          WORD32 accu = 1;
          WORD32 q_nrg;
          if (sfb == 0) {
            q_nrg = (sizeof(accu) << 3) - ixheaac_norm32(ptr_sfb_offset[sfb] - 0);
            for (; line < ptr_sfb_offset[sfb]; line++) {
              accu +=
                  ixheaac_mult32(ptr_spec_coeff[line >> 3], ptr_spec_coeff[line >> 3]) >> q_nrg;
            }
            ptr_sfb_energy[sfb] = ixheaac_norm32(accu);
          }
          q_nrg = (sizeof(accu) << 3) -
                  ixheaac_norm32(ptr_sfb_offset[sfb + 1] - ptr_sfb_offset[sfb]);
          for (; line < ptr_sfb_offset[sfb + 1]; line++) {
            accu +=
                ixheaac_mult32(ptr_spec_coeff[line >> 3], ptr_spec_coeff[line >> 3]) >> q_nrg;
          }
          ptr_sfb_energy[sfb] = ixheaac_norm32(accu);
        }
      }
      break;

    case ONLY_LONG_SEQUENCE:
    case LONG_START_SEQUENCE:
    case LONG_STOP_SEQUENCE:

      if (win_trans == NO_TRANSITION) {
        total_scale_factor_bands = pstr_samp_rate_info->num_sfb_1024 - 1;
        ptr_sfb_offset = pstr_samp_rate_info->ptr_sfb_1024;

        for (sfb = 0; sfb < total_scale_factor_bands; sfb++) {
          WORD32 accu = 1;
          WORD32 q_nrg;
          if (sfb == 0) {
            q_nrg = (sizeof(accu) << 3) - ixheaac_norm32(ptr_sfb_offset[sfb] - 0);
            for (; line < ptr_sfb_offset[sfb]; line++) {
              accu += ixheaac_mult32(ptr_spec_coeff[line], ptr_spec_coeff[line]) >> q_nrg;
            }
            ptr_sfb_energy[sfb] = ixheaac_norm32(accu);
          }
          q_nrg = (sizeof(accu) << 3) -
                  ixheaac_norm32(ptr_sfb_offset[sfb + 1] - ptr_sfb_offset[sfb]);
          for (; line < ptr_sfb_offset[sfb + 1]; line++) {
            accu += ixheaac_mult32(ptr_spec_coeff[line], ptr_spec_coeff[line]) >> q_nrg;
          }
          ptr_sfb_energy[sfb] = ixheaac_norm32(accu);
        }
      } else {
        total_scale_factor_bands = pstr_samp_rate_info->num_sfb_128 - 1;
        ptr_sfb_offset = pstr_samp_rate_info->ptr_sfb_128;

        for (sfb = 0; sfb < total_scale_factor_bands; sfb++) {
          WORD32 accu = 1;
          WORD32 q_nrg;
          if (sfb == 0) {
            q_nrg = (sizeof(accu) << 3) - ixheaac_norm32(ptr_sfb_offset[sfb] - 0);
            for (; line < ptr_sfb_offset[sfb] << 3; line++) {
              accu += (accu +
                       (ixheaac_mult32(ptr_spec_coeff[line], ptr_spec_coeff[line]) >> q_nrg)) >>
                      3;
            }
            ptr_sfb_energy[sfb] = ixheaac_norm32(accu);
          }
          q_nrg = (sizeof(accu) << 3) -
                  ixheaac_norm32(ptr_sfb_offset[sfb + 1] - ptr_sfb_offset[sfb]);
          for (; line < ptr_sfb_offset[sfb + 1] << 3; line++) {
            accu +=
                (accu + (ixheaac_mult32(ptr_spec_coeff[line], ptr_spec_coeff[line]) >> q_nrg)) >>
                3;
          }
          ptr_sfb_energy[sfb] = ixheaac_norm32(accu);
        }
      }
      break;
  }
}

static VOID ixheaacd_aac_ec_interpolate(WORD32 *ptr_spec_coeff, WORD16 *ptr_spec_scale_prev,
                                        WORD16 *ptr_spec_scale_act, WORD16 *ptr_spec_scale_out,
                                        WORD32 *nrg_prev, WORD32 *nrg_act, WORD32 num_sfb,
                                        const WORD16 *ptr_sfb_offset) {
  WORD32 sfb, line = 0;
  WORD32 fac_shift;
  WORD32 fac_mod;

  for (sfb = 0; sfb < num_sfb; sfb++) {
    fac_shift =
        nrg_prev[sfb] - nrg_act[sfb] + ((*ptr_spec_scale_act - *ptr_spec_scale_prev) << 1);
    fac_mod = fac_shift & 3;
    fac_shift = (fac_shift >> 2) + 1;
    fac_shift += *ptr_spec_scale_prev - max(*ptr_spec_scale_prev, *ptr_spec_scale_act);
    fac_shift = max(min(fac_shift, INT_BITS - 1), -(INT_BITS - 1));

    for (; line < ptr_sfb_offset[sfb]; line++) {
      WORD32 accu =
          ixheaac_mult32x16in32_shl(ptr_spec_coeff[line], ia_ec_interpolation_fac[fac_mod]);
      ptr_spec_coeff[line] = ixheaac_shl32_dir_sat(accu, fac_shift);
    }
  }
  *ptr_spec_scale_out = max(*ptr_spec_scale_prev, *ptr_spec_scale_act);
}

static VOID ixheaacd_aac_ec_state(ia_ec_state_str *pstr_ec_state, WORD32 frame_status) {
  WORD32 ec_state_val = (pstr_ec_state->prev_frame_ok[0] << 2) +
                        (pstr_ec_state->prev_frame_ok[1] << 1) + (frame_status);

  switch (ec_state_val) {
    case 0:
    case 4:
      if (pstr_ec_state->fade_idx < MAX_FADE_FRAMES) {
        pstr_ec_state->fade_idx++;
      }
      pstr_ec_state->conceal_state = FRAME_CONCEAL_SINGLE;
      break;
    case 1:
    case 2:
      if (pstr_ec_state->fade_idx > 0) {
        pstr_ec_state->fade_idx--;
      }
      pstr_ec_state->conceal_state = FRAME_FADE;
      break;
    case 5:
      if (pstr_ec_state->fade_idx > 0) {
        pstr_ec_state->fade_idx--;
      }
      pstr_ec_state->conceal_state = FRAME_OKAY;
      break;
      break;
    case 3:
    case 6:
    case 7:
      if (pstr_ec_state->fade_idx > 0) {
        pstr_ec_state->fade_idx--;
      }
      pstr_ec_state->conceal_state = FRAME_OKAY;
      break;
    default:
      pstr_ec_state->conceal_state = FRAME_OKAY;
  }
  if (pstr_ec_state->fade_idx > MAX_FADE_FRAMES) {
    pstr_ec_state->fade_idx = MAX_FADE_FRAMES;
  }
  if (pstr_ec_state->fade_idx == MAX_FADE_FRAMES) {
    pstr_ec_state->conceal_state = FRAME_MUTE;
  }
  if (pstr_ec_state->fade_idx < 0) {
    pstr_ec_state->fade_idx = 0;
  }
}

static VOID ixheaacd_aac_ec_interpolate_frame(
    ia_ec_state_str *pstr_ec_state, ia_aac_dec_channel_info_struct *pstr_aac_dec_channel_info,
    const ia_usac_samp_rate_info *pstr_samp_rate_info, const WORD32 num_samples,
    const WORD32 frame_status, ia_ics_info_struct *pstr_ics_info) {
  WORD32 *ptr_spec_coeff = pstr_aac_dec_channel_info->ptr_spec_coeff;
  WORD16 *ptr_spec_scale = pstr_aac_dec_channel_info->ptr_scale_factor;

  WORD32 sfb_nrg_prev[WIN_LEN_64];
  WORD32 sfb_nrg_act[WIN_LEN_64];

  WORD32 idx;

  memset(sfb_nrg_prev, 0, sizeof(sfb_nrg_prev));
  memset(sfb_nrg_act, 0, sizeof(sfb_nrg_act));

  if (!frame_status) {
    pstr_ics_info->window_shape = pstr_ec_state->win_shape;
    pstr_ics_info->window_sequence = pstr_ec_state->win_seq;

    for (idx = 0; idx < num_samples; idx++) {
      ptr_spec_coeff[idx] = pstr_ec_state->spectral_coeff[idx];
    }

    memcpy(ptr_spec_scale, pstr_ec_state->q_spec_coeff, 8 * sizeof(ptr_spec_scale[0]));
  }

  if (!pstr_ec_state->prev_frame_ok[1]) {
    if (frame_status && pstr_ec_state->prev_frame_ok[0]) {
      if (pstr_ics_info->window_sequence == EIGHT_SHORT_SEQUENCE) {
        WORD32 window;
        if (pstr_ec_state->win_seq == EIGHT_SHORT_SEQUENCE) {
          WORD32 total_scale_factor_bands = pstr_samp_rate_info->num_sfb_128 - 1;
          const WORD16 *ptr_sfb_offset = pstr_samp_rate_info->ptr_sfb_128;
          pstr_ics_info->window_shape = 1;
          pstr_ics_info->window_sequence = EIGHT_SHORT_SEQUENCE;

          for (window = 0; window < 8; window++) {
            ixheaacd_aac_ec_calc_sfb_nrg(&ptr_spec_coeff[window * (num_samples / 8)],
                                         pstr_samp_rate_info, EIGHT_SHORT_SEQUENCE, NO_TRANSITION,
                                         sfb_nrg_prev);

            ixheaacd_aac_ec_calc_sfb_nrg(
                &pstr_ec_state->spectral_coeff[window * (num_samples / 8)], pstr_samp_rate_info,
                EIGHT_SHORT_SEQUENCE, NO_TRANSITION, sfb_nrg_act);

            ixheaacd_aac_ec_interpolate(
                &ptr_spec_coeff[window * (num_samples / 8)], &ptr_spec_scale[window],
                &pstr_ec_state->q_spec_coeff[window], &ptr_spec_scale[window], sfb_nrg_prev,
                sfb_nrg_act, total_scale_factor_bands, ptr_sfb_offset);
          }
        } else {
          WORD32 total_scale_factor_bands = pstr_samp_rate_info->num_sfb_1024 - 1;
          const WORD16 *ptr_sfb_offset = pstr_samp_rate_info->ptr_sfb_1024;
          WORD16 spec_scale_out;

          ixheaacd_aac_ec_calc_sfb_nrg(&ptr_spec_coeff[num_samples - (num_samples / 8)],
                                       pstr_samp_rate_info, EIGHT_SHORT_SEQUENCE,
                                       TRANS_SHORT_LONG, sfb_nrg_act);

          ixheaacd_aac_ec_calc_sfb_nrg(pstr_ec_state->spectral_coeff, pstr_samp_rate_info,
                                       ONLY_LONG_SEQUENCE, NO_TRANSITION, sfb_nrg_prev);

          pstr_ics_info->window_shape = 0;
          pstr_ics_info->window_sequence = LONG_STOP_SEQUENCE;

          for (idx = 0; idx < num_samples; idx++) {
            ptr_spec_coeff[idx] = pstr_ec_state->spectral_coeff[idx];
          }

          for (idx = 0; idx < 8; idx++) {
            if (ptr_spec_scale[idx] > ptr_spec_scale[0]) {
              ptr_spec_scale[0] = ptr_spec_scale[idx];
            }
          }

          ixheaacd_aac_ec_interpolate(ptr_spec_coeff, &pstr_ec_state->q_spec_coeff[0],
                                      &ptr_spec_scale[0], &spec_scale_out, sfb_nrg_prev,
                                      sfb_nrg_act, total_scale_factor_bands, ptr_sfb_offset);

          ptr_spec_scale[0] = spec_scale_out;
        }
      } else {
        WORD32 total_scale_factor_bands = pstr_samp_rate_info->num_sfb_1024 - 1;
        const WORD16 *ptr_sfb_offset = pstr_samp_rate_info->ptr_sfb_1024;
        WORD16 spec_scale_act = pstr_ec_state->q_spec_coeff[0];

        ixheaacd_aac_ec_calc_sfb_nrg(ptr_spec_coeff, pstr_samp_rate_info, ONLY_LONG_SEQUENCE,
                                     NO_TRANSITION, sfb_nrg_prev);

        if (pstr_ec_state->win_seq == EIGHT_SHORT_SEQUENCE) {
          pstr_ics_info->window_shape = 1;
          pstr_ics_info->window_sequence = LONG_START_SEQUENCE;

          for (idx = 1; idx < 8; idx++) {
            if (pstr_ec_state->q_spec_coeff[idx] > spec_scale_act) {
              spec_scale_act = pstr_ec_state->q_spec_coeff[idx];
            }
          }

          ixheaacd_aac_ec_calc_sfb_nrg(pstr_ec_state->spectral_coeff, pstr_samp_rate_info,
                                       EIGHT_SHORT_SEQUENCE, TRANS_SHORT_LONG, sfb_nrg_act);
        } else {
          pstr_ics_info->window_shape = 0;
          pstr_ics_info->window_sequence = ONLY_LONG_SEQUENCE;

          ixheaacd_aac_ec_calc_sfb_nrg(pstr_ec_state->spectral_coeff, pstr_samp_rate_info,
                                       ONLY_LONG_SEQUENCE, NO_TRANSITION, sfb_nrg_act);
        }

        ixheaacd_aac_ec_interpolate(ptr_spec_coeff, &ptr_spec_scale[0], &spec_scale_act,
                                    &ptr_spec_scale[0], sfb_nrg_prev, sfb_nrg_act,
                                    total_scale_factor_bands, ptr_sfb_offset);
      }
    }

    ixheaacd_aac_ec_flip_spec_sign(ptr_spec_coeff, num_samples);
  }

  if (FRAME_MUTE == pstr_ec_state->conceal_state) {
    pstr_ics_info->window_shape = pstr_ec_state->win_shape;
    pstr_ics_info->window_sequence = ixheaacd_aac_ec_get_win_seq(pstr_ec_state->win_seq);
    pstr_ec_state->win_seq = pstr_ics_info->window_sequence;
    memset(ptr_spec_coeff, 0, num_samples * sizeof(ptr_spec_coeff[0]));
  }
}

VOID ixheaacd_aac_ec_init(ia_ec_state_str *pstr_ec_state) {
  pstr_ec_state->win_shape = CONCEAL_NOT_DEFINED;
  pstr_ec_state->win_seq = ONLY_LONG_SEQUENCE;
  pstr_ec_state->prev_win_group_len = 1;

  pstr_ec_state->conceal_state = FRAME_OKAY;

  memset(pstr_ec_state->spectral_coeff, 0, sizeof(pstr_ec_state->spectral_coeff));
  memset(pstr_ec_state->q_spec_coeff, 0, sizeof(pstr_ec_state->q_spec_coeff));

  pstr_ec_state->prev_frame_ok[0] = 1;
  pstr_ec_state->prev_frame_ok[1] = 1;
  pstr_ec_state->fade_idx = 0;
}

VOID ixheaacd_aac_apply_ec(ia_ec_state_str *pstr_ec_state,
                           ia_aac_dec_channel_info_struct *pstr_aac_dec_channel_info,
                           const ia_usac_samp_rate_info *pstr_samp_rate_info,
                           const WORD32 num_samples, ia_ics_info_struct *pstr_ics_info,
                           const WORD32 frame_status) {
  if (pstr_ec_state->win_shape == CONCEAL_NOT_DEFINED) {
    pstr_ec_state->win_shape = (UWORD8)pstr_ics_info->window_shape;
  }

  if (frame_status && pstr_ec_state->prev_frame_ok[1]) {
    ixheaacd_aac_ec_store(pstr_ec_state, pstr_aac_dec_channel_info, pstr_ics_info);
  }

  ixheaacd_aac_ec_state(pstr_ec_state, frame_status);

  ixheaacd_aac_ec_interpolate_frame(pstr_ec_state, pstr_aac_dec_channel_info, pstr_samp_rate_info,
                                    num_samples, frame_status, pstr_ics_info);

  pstr_ec_state->prev_frame_ok[0] = pstr_ec_state->prev_frame_ok[1];
  pstr_ec_state->prev_frame_ok[1] = frame_status;
}
