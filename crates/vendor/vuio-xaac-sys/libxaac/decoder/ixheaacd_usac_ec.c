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
#include <string.h>
#include <ixheaac_type_def.h>
#include <ixheaac_constants.h>
#include <ixheaac_basic_ops32.h>
#include <ixheaac_basic_ops16.h>
#include <ixheaac_basic_ops40.h>
#include "ixheaacd_sbr_common.h"
#include "ixheaacd_intrinsics.h"
#include "ixheaacd_common_rom.h"
#include "ixheaacd_sbrdecsettings.h"
#include "ixheaacd_bitbuffer.h"
#include "ixheaacd_defines.h"
#include "ixheaacd_pns.h"
#include <ixheaacd_aac_rom.h>
#include "ixheaacd_pulsedata.h"
#include "ixheaacd_lt_predict.h"
#include "ixheaacd_cnst.h"
#include "ixheaacd_drc_data_struct.h"
#include "ixheaacd_ec_defines.h"
#include "ixheaacd_ec_rom.h"
#include "ixheaacd_ec_struct_def.h"
#include "ixheaacd_channelinfo.h"
#include "ixheaacd_interface.h"
#include "ixheaacd_acelp_info.h"
#include "ixheaacd_tns_usac.h"
#include "ixheaacd_info.h"
#include "ixheaacd_drc_dec.h"
#include "ixheaacd_sbrdecoder.h"
#include "ixheaacd_mps_polyphase.h"
#include "ixheaac_sbr_const.h"
#include "ixheaacd_ec_defines.h"
#include "ixheaacd_ec_struct_def.h"
#include "ixheaacd_main.h"
#include "ixheaacd_acelp_com.h"

static WORD32 ixheaacd_usac_ec_get_win_seq(WORD32 prev_win_seq) {
  if (prev_win_seq == LONG_START_SEQUENCE || prev_win_seq == EIGHT_SHORT_SEQUENCE) {
    return LONG_STOP_SEQUENCE;
  } else {
    return ONLY_LONG_SEQUENCE;
  }
}

static VOID ixheaacd_usac_flip_spec_sign(WORD32 *ptr_spec_coeff, WORD32 samples_per_frame,
                                         UWORD32 *seed_value) {
  WORD32 i;
  for (i = 0; i < samples_per_frame; i++) {
    ptr_spec_coeff[i] = ixheaac_mult32x16in32_sat(ptr_spec_coeff[i],
                            (WORD16)ixheaacd_randomsign(seed_value));
  }
}

static VOID iexheaace_ec_sfb_nrg_q(WORD32 *ptr_spectrum, ia_ec_sfb_str *pstr_ec_sfb,
                                   WORD32 win_seq, WORD32 win_trans, WORD32 *ptr_sfb_enrg) {
  WORD16 *ptr_sfb_offset = pstr_ec_sfb->ptr_sfb_long;
  WORD32 l = 0, sfb, num_sfb = pstr_ec_sfb->num_sfb_long;
  switch (win_seq) {
    case EIGHT_SHORT_SEQUENCE:
      if (win_trans == NO_TRANSITION) {
        num_sfb = pstr_ec_sfb->num_sfb_short;
        ptr_sfb_offset = pstr_ec_sfb->ptr_sfb_short;
        for (sfb = 0; sfb < num_sfb; sfb++) {
          WORD64 accu = (WORD64)1;
          WORD32 q_nrg = (sizeof(accu) << 3) -
                         ixheaac_norm32(ptr_sfb_offset[sfb + 1] - ptr_sfb_offset[sfb]);
          for (; l < ptr_sfb_offset[sfb + 1]; l++) {
            accu += ixheaac_mul32_sh(ptr_spectrum[l], ptr_spectrum[l], q_nrg);
          }
          ptr_sfb_enrg[sfb] = ixheaac_norm32((WORD32)accu);
        }
      } else {
        num_sfb = pstr_ec_sfb->num_sfb_long;
        ptr_sfb_offset = pstr_ec_sfb->ptr_sfb_long;

        for (sfb = 0; sfb < num_sfb; sfb++) {
          WORD64 accu = (WORD64)1;
          WORD32 q_nrg = (sizeof(accu) << 3) -
                         ixheaac_norm32(ptr_sfb_offset[sfb + 1] - ptr_sfb_offset[sfb]);
          for (; l < ptr_sfb_offset[sfb + 1]; l++) {
            accu += ixheaac_mul32_sh(ptr_spectrum[(l >> 3)], ptr_spectrum[(l >> 3)], q_nrg);
          }
          ptr_sfb_enrg[sfb] = ixheaac_norm32((WORD32)accu);
        }
      }
      break;
    case ONLY_LONG_SEQUENCE:
    case LONG_START_SEQUENCE:
    case LONG_STOP_SEQUENCE:
      if (win_trans == NO_TRANSITION) {
        num_sfb = pstr_ec_sfb->num_sfb_long;
        ptr_sfb_offset = pstr_ec_sfb->ptr_sfb_long;

        for (sfb = 0; sfb < num_sfb; sfb++) {
          WORD64 accu = (WORD64)1;
          WORD32 q_nrg = (sizeof(accu) << 3) -
                         ixheaac_norm32(ptr_sfb_offset[sfb + 1] - ptr_sfb_offset[sfb]);
          for (; l < ptr_sfb_offset[sfb + 1]; l++) {
            accu += ixheaac_mul32_sh(ptr_spectrum[l], ptr_spectrum[l], q_nrg);
          }
          ptr_sfb_enrg[sfb] = ixheaac_norm32((WORD32)accu);
        }
      } else {
        num_sfb = pstr_ec_sfb->num_sfb_short;
        ptr_sfb_offset = pstr_ec_sfb->ptr_sfb_short;

        for (sfb = 0; sfb < num_sfb; sfb++) {
          WORD64 accu = (WORD64)1;
          WORD32 q_nrg = (sizeof(accu) << 3) -
                         ixheaac_norm32(ptr_sfb_offset[sfb + 1] - ptr_sfb_offset[sfb]);
          for (; l < ptr_sfb_offset[sfb + 1] << 3; l++) {
            accu += (accu + (ixheaac_mul32_sh(ptr_spectrum[l], ptr_spectrum[l], q_nrg))) >> 3;
          }
          ptr_sfb_enrg[sfb] = ixheaac_norm32((WORD32)accu);
        }
      }
      break;
  }
}

static VOID ixheaacd_usac_ec_interpolate(WORD32 *ptr_spectrum, WORD16 *pq_spec_coeff_prev,
                                         WORD16 *pq_spec_coeff_act, WORD16 *pq_spec_coeff_out,
                                         WORD32 *ptr_nrg_prev, WORD32 *ptr_nrg_act,
                                         WORD32 num_sfb, WORD16 *ptr_sfb_offset) {
  WORD32 sfb, l = 0;
  WORD32 fac_shift;
  WORD32 fac_mod;

  for (sfb = 0; sfb < num_sfb; sfb++) {
    fac_shift =
        ptr_nrg_prev[sfb] - ptr_nrg_act[sfb] + ((*pq_spec_coeff_act - *pq_spec_coeff_prev) << 1);
    fac_mod = fac_shift & 3;
    fac_shift = (fac_shift >> 2) + 1;
    fac_shift += *pq_spec_coeff_prev - ixheaac_max16(*pq_spec_coeff_prev, *pq_spec_coeff_act);
    fac_shift = ixheaac_max32(ixheaac_min32(fac_shift, INT_BITS - 1), -(INT_BITS - 1));

    for (; l < ptr_sfb_offset[sfb + 1]; l++) {
      WORD32 accu = ixheaac_shl32_sat(
          ixheaac_mult32x32in32(ptr_spectrum[l], (WORD32)(ia_ec_interpolation_fac[fac_mod])), 1);
      ptr_spectrum[l] = ixheaac_shl32_dir_sat((WORD32)accu, fac_shift);
    }
  }
  *pq_spec_coeff_out = ixheaac_max16(*pq_spec_coeff_prev, *pq_spec_coeff_act);
}

static VOID ixheaacd_usac_ec_interpolate_frame(ia_usac_data_struct *pstr_usac_data,
                                               ia_ec_state_str *pstr_ec_state,
                                               const ia_usac_samp_rate_info *pstr_samp_rate_info,
                                               WORD32 frame_ok, WORD32 chn) {
  WORD32 frame_length = pstr_usac_data->ccfl;
  WORD32 *ptr_spec_coeff = pstr_usac_data->coef_fix[chn];
  WORD16 *ptr_spec_sf = pstr_usac_data->spec_scale[chn];

  WORD32 i;
  ia_ec_scratch_str *pstr_ec_scratch = pstr_ec_state->pstr_ec_scratch;
  WORD16 num_sfb_long;
  WORD16 *ptr_sfb_long = NULL;
  WORD16 num_sfb_short;
  WORD16 *ptr_sfb_short = NULL;

  if (pstr_usac_data->core_mode == CORE_MODE_FD) {
    num_sfb_long = pstr_samp_rate_info->num_sfb_1024;
    ptr_sfb_long = (WORD16 *)pstr_samp_rate_info->ptr_sfb_1024;
    num_sfb_short = pstr_samp_rate_info->num_sfb_128;
    ptr_sfb_short = (WORD16 *)pstr_samp_rate_info->ptr_sfb_128;
    if (pstr_usac_data->ccfl == WIN_LEN_768) {
      num_sfb_long = pstr_samp_rate_info->num_sfb_768;
      ptr_sfb_long = (WORD16 *)pstr_samp_rate_info->ptr_sfb_768;
      num_sfb_short = pstr_samp_rate_info->num_sfb_96;
      ptr_sfb_short = (WORD16 *)pstr_samp_rate_info->ptr_sfb_96;
    }
    pstr_ec_state->str_ec_sfb.num_sfb_long = num_sfb_long;
    pstr_ec_state->str_ec_sfb.num_sfb_long = num_sfb_long;
    pstr_ec_state->str_ec_sfb.ptr_sfb_long = ptr_sfb_long;
    pstr_ec_state->str_ec_sfb.ptr_sfb_long = ptr_sfb_long;

    memset(pstr_ec_scratch->prev_sfb_nrg, 0, sizeof(pstr_ec_scratch->prev_sfb_nrg));
    memset(pstr_ec_scratch->pres_sfb_nrg, 0, sizeof(pstr_ec_scratch->pres_sfb_nrg));

    if (!frame_ok) {
      pstr_usac_data->window_shape[chn] = pstr_ec_state->win_shape;
      pstr_usac_data->window_sequence[chn] = pstr_ec_state->win_seq;
      memcpy(ptr_spec_coeff, pstr_ec_state->spectral_coeff,
             sizeof(*ptr_spec_coeff) * frame_length);
      memcpy(ptr_spec_sf, pstr_ec_state->q_spec_coeff, sizeof(pstr_ec_state->q_spec_coeff));
    }
  }

  if (!pstr_ec_state->prev_frame_ok[1]) {
    if (frame_ok && pstr_ec_state->prev_frame_ok[0] &&
        pstr_usac_data->core_mode == CORE_MODE_FD) {
      if (pstr_usac_data->window_sequence[chn] == EIGHT_SHORT_SEQUENCE) {
        WORD32 wnd;

        if (pstr_ec_state->win_seq == EIGHT_SHORT_SEQUENCE) {
          WORD32 num_sfb = num_sfb_short;
          WORD16 *ptr_sfb_offset = ptr_sfb_short;
          pstr_usac_data->window_shape[chn] = 1;
          pstr_usac_data->window_sequence[chn] = EIGHT_SHORT_SEQUENCE;

          for (wnd = 0; wnd < 8; wnd++) {
            iexheaace_ec_sfb_nrg_q(&ptr_spec_coeff[wnd * (frame_length >> 3)],
                                   &pstr_ec_state->str_ec_sfb, EIGHT_SHORT_SEQUENCE,
                                   NO_TRANSITION, pstr_ec_scratch->prev_sfb_nrg);

            iexheaace_ec_sfb_nrg_q(&pstr_ec_state->spectral_coeff[wnd * (frame_length >> 3)],
                                   &pstr_ec_state->str_ec_sfb, EIGHT_SHORT_SEQUENCE,
                                   NO_TRANSITION, pstr_ec_scratch->pres_sfb_nrg);

            ixheaacd_usac_ec_interpolate(&ptr_spec_coeff[wnd * (frame_length / 8)],
                                         &ptr_spec_sf[wnd], &pstr_ec_state->q_spec_coeff[wnd],
                                         &ptr_spec_sf[wnd], pstr_ec_scratch->prev_sfb_nrg,
                                         pstr_ec_scratch->pres_sfb_nrg, num_sfb, ptr_sfb_offset);
          }
        } else {
          WORD32 num_sfb = num_sfb_long;
          WORD16 *ptr_sfb_offset = ptr_sfb_long;
          WORD16 q_spec_coeff_out;

          iexheaace_ec_sfb_nrg_q(&ptr_spec_coeff[frame_length - (frame_length >> 3)],
                                 &pstr_ec_state->str_ec_sfb, EIGHT_SHORT_SEQUENCE,
                                 TRANS_SHORT_LONG, pstr_ec_scratch->pres_sfb_nrg);

          iexheaace_ec_sfb_nrg_q(pstr_ec_state->spectral_coeff, &pstr_ec_state->str_ec_sfb,
                                 ONLY_LONG_SEQUENCE, NO_TRANSITION,
                                 pstr_ec_scratch->prev_sfb_nrg);

          pstr_usac_data->window_shape[chn] = 0;
          pstr_usac_data->window_sequence[chn] = LONG_STOP_SEQUENCE;
          memcpy(&ptr_spec_coeff[0], pstr_ec_state->spectral_coeff,
                 frame_length * sizeof(ptr_spec_coeff[0]));

          for (i = 0; i < 8; i++) {
            if (ptr_spec_sf[i] > ptr_spec_sf[0]) {
              ptr_spec_sf[0] = ptr_spec_sf[i];
            }
          }

          ixheaacd_usac_ec_interpolate(ptr_spec_coeff, &pstr_ec_state->q_spec_coeff[0],
                                       &ptr_spec_sf[0], &q_spec_coeff_out,
                                       pstr_ec_scratch->prev_sfb_nrg,
                                       pstr_ec_scratch->pres_sfb_nrg, num_sfb, ptr_sfb_offset);

          ptr_spec_sf[0] = q_spec_coeff_out;
        }
      } else {
        WORD32 num_sfb = num_sfb_long;
        WORD16 *ptr_sfb_offset = ptr_sfb_long;
        WORD16 q_spec_coeff_act = pstr_ec_state->q_spec_coeff[0];

        iexheaace_ec_sfb_nrg_q(ptr_spec_coeff, &pstr_ec_state->str_ec_sfb, ONLY_LONG_SEQUENCE,
                               NO_TRANSITION, pstr_ec_scratch->prev_sfb_nrg);

        if (pstr_ec_state->win_seq == EIGHT_SHORT_SEQUENCE) {
          pstr_usac_data->window_shape[chn] = 1;
          pstr_usac_data->window_sequence[chn] = LONG_START_SEQUENCE;

          for (i = 1; i < 8; i++) {
            if (pstr_ec_state->q_spec_coeff[i] > q_spec_coeff_act) {
              q_spec_coeff_act = pstr_ec_state->q_spec_coeff[i];
            }
          }

          iexheaace_ec_sfb_nrg_q(pstr_ec_state->spectral_coeff, &pstr_ec_state->str_ec_sfb,
                                 EIGHT_SHORT_SEQUENCE, TRANS_SHORT_LONG,
                                 pstr_ec_scratch->pres_sfb_nrg);
        } else {
          pstr_usac_data->window_shape[chn] = 0;
          pstr_usac_data->window_sequence[chn] = ONLY_LONG_SEQUENCE;
          iexheaace_ec_sfb_nrg_q(pstr_ec_state->spectral_coeff, &pstr_ec_state->str_ec_sfb,
                                 ONLY_LONG_SEQUENCE, NO_TRANSITION,
                                 pstr_ec_scratch->pres_sfb_nrg);
        }
        ixheaacd_usac_ec_interpolate(ptr_spec_coeff, &ptr_spec_sf[0], &q_spec_coeff_act,
                                     &ptr_spec_sf[0], pstr_ec_scratch->prev_sfb_nrg,
                                     pstr_ec_scratch->pres_sfb_nrg, num_sfb, ptr_sfb_offset);
      }
    }
    ixheaacd_usac_flip_spec_sign(ptr_spec_coeff, frame_length, &pstr_usac_data->seed_value[chn]);
  }

  if (FRAME_MUTE == pstr_ec_state->conceal_state) {
    pstr_usac_data->window_shape[chn] = pstr_ec_state->win_shape;
    pstr_usac_data->window_sequence[chn] = ixheaacd_usac_ec_get_win_seq(pstr_ec_state->win_seq);
    pstr_ec_state->win_seq = pstr_usac_data->window_sequence[chn];
    memset(ptr_spec_coeff, 0, frame_length * sizeof(ptr_spec_coeff[0]));
  }

  return;
}

static VOID ixheaacd_usac_lpc_ec_state(ia_ec_state_str *pstr_ec_state, WORD32 frame_ok) {
  if (frame_ok == 0) {
    if (pstr_ec_state->fade_idx < MAX_FADE_FRAMES) {
      pstr_ec_state->fade_idx++;
    }
    pstr_ec_state->conceal_state = FRAME_CONCEAL_SINGLE;
  } else {
    if (pstr_ec_state->fade_idx > 0) {
      pstr_ec_state->fade_idx--;
    }
    pstr_ec_state->conceal_state = FRAME_OKAY;
  }
  if (pstr_ec_state->fade_idx >= MAX_FADE_FRAMES) {
    pstr_ec_state->fade_idx = MAX_FADE_FRAMES;
    pstr_ec_state->conceal_state = FRAME_MUTE;
  }
  if (pstr_ec_state->fade_idx < 0) {
    pstr_ec_state->fade_idx = 0;
  }
  return;
}

static VOID ixheaacd_usac_ec_state(ia_ec_state_str *pstr_ec_state, WORD32 frame_ok) {
  WORD32 ec_state_val = (pstr_ec_state->prev_frame_ok[0] << 2) +
                        (pstr_ec_state->prev_frame_ok[1] << 1) + (frame_ok);

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

VOID ixheaacd_usac_ec_init(ia_ec_state_str *pstr_ec_state, WORD32 core_coder_mode) {
  pstr_ec_state->win_shape = 1;
  pstr_ec_state->win_seq = ONLY_LONG_SEQUENCE;
  pstr_ec_state->prev_win_group_len = 1;

  pstr_ec_state->conceal_state = FRAME_OKAY;

  memset(pstr_ec_state->spectral_coeff, 0, sizeof(pstr_ec_state->spectral_coeff));
  memset(pstr_ec_state->q_spec_coeff, 0, sizeof(pstr_ec_state->q_spec_coeff));

  pstr_ec_state->prev_frame_ok[0] = 1;
  pstr_ec_state->prev_frame_ok[1] = 1;

  pstr_ec_state->fade_idx = 0;

  pstr_ec_state->prev_core_mode = core_coder_mode;
}

VOID ixheaacd_usac_lpc_ec(FLOAT32 lsp[][ORDER], FLOAT32 *lpc4_lsf, FLOAT32 *lsf_adaptive_mean,
                          const WORD32 first_lpd_flag) {
  WORD32 i, j;

  if (first_lpd_flag) {
    memcpy(lsp[0], lsf_init, sizeof(lsf_init));
    memcpy(lpc4_lsf, lsf_init, sizeof(lsf_init));
  } else {
    memcpy(lsp[0], lpc4_lsf, ORDER * sizeof(lpc4_lsf[0]));
  }

  for (i = 0; i < ORDER; i++) {
    FLOAT32 lsf_mean = (BETA * lsf_init[i]) + (ONE_BETA * lsf_adaptive_mean[i]);
    lsp[1][i] = (BFI_FAC * lpc4_lsf[i]) + (ONE_BFI_FAC * lsf_mean);
  }

  for (j = 2; j <= 4; j++) {
    for (i = 0; i < ORDER; i++) {
      FLOAT32 lsf_mean = ((BETA + (j * ONE_BFI_FAC)) * lsf_init[i]) +
                         ((ONE_BETA - (j * ONE_BFI_FAC)) * lsf_adaptive_mean[i]);
      lsp[j][i] = (BFI_FAC * lsp[j - 1][i]) + (ONE_BFI_FAC * lsf_mean);
    }
  }

  memcpy(lpc4_lsf, lsp[4], ORDER * sizeof(lpc4_lsf[0]));
}

VOID ixheaacd_usac_ec_save_states(ia_ec_state_str *pstr_ec_state,
                                  ia_usac_data_struct *pstr_usac_data, WORD32 ch) {
  if (pstr_usac_data->core_mode == CORE_MODE_FD &&
      (pstr_usac_data->frame_ok == 1 && pstr_ec_state->prev_frame_ok[1] == 1)) {
    WORD32 *ptr_spec_coeff = pstr_usac_data->coef_fix[ch];
    WORD16 *ptr_spec_scale = pstr_usac_data->spec_scale[ch];
    WORD16 q_spec_coeff[MAX_SPEC_SCALE_LEN_EC];
    UWORD8 win_shape = pstr_ec_state->win_shape;
    UWORD8 win_shape_prev = pstr_ec_state->win_shape_prev;
    WORD32 win_seq = pstr_ec_state->win_seq;
    WORD32 td_frame_prev = pstr_ec_state->td_frame_prev;
    WORD32 fac_data_present = pstr_ec_state->fac_data_present;

    ia_sfb_info_struct *sfb_info =
        pstr_usac_data->pstr_usac_winmap[pstr_usac_data->window_sequence[ch]];
    WORD32 *ptr_scratch_buf = &pstr_ec_state->pstr_ec_scratch->spec_coeff[0];

    memcpy(q_spec_coeff, pstr_ec_state->q_spec_coeff, sizeof(q_spec_coeff));
    pstr_ec_state->win_seq = pstr_usac_data->window_sequence[ch];
    pstr_ec_state->win_shape = pstr_usac_data->window_shape[ch];
    pstr_ec_state->td_frame_prev = pstr_usac_data->td_frame_prev[ch];
    pstr_ec_state->fac_data_present = pstr_usac_data->fac_data_present[ch];
    pstr_ec_state->win_shape_prev = pstr_usac_data->window_shape_prev[ch];
    pstr_ec_state->prev_win_group_len = (WORD32)sfb_info->group_len[sfb_info->num_groups - 1];

    memcpy(pstr_ec_state->q_spec_coeff, ptr_spec_scale, sizeof(pstr_ec_state->q_spec_coeff));

    memcpy(ptr_scratch_buf, ptr_spec_coeff, pstr_usac_data->ccfl * sizeof(ptr_scratch_buf[0]));
    memcpy(ptr_spec_coeff, &pstr_ec_state->spectral_coeff[0],
           pstr_usac_data->ccfl * sizeof(ptr_spec_coeff[0]));
    memcpy(&pstr_ec_state->spectral_coeff[0], ptr_scratch_buf,
           pstr_usac_data->ccfl * sizeof(ptr_spec_coeff[0]));

    if (!pstr_usac_data->first_frame) {
      pstr_usac_data->window_sequence[ch] = win_seq;
      pstr_usac_data->window_shape[ch] = win_shape;
      pstr_usac_data->td_frame_prev_ec[ch] = td_frame_prev;
      pstr_usac_data->fac_data_present[ch] = fac_data_present;
      pstr_usac_data->window_shape_prev[ch] = win_shape_prev;
    }

    memcpy(ptr_spec_scale, q_spec_coeff, sizeof(q_spec_coeff));
  }
}

VOID ixheaacd_usac_apply_ec(ia_usac_data_struct *pstr_usac_data,
                            const ia_usac_samp_rate_info *pstr_samp_rate_info, WORD32 ch) {
  WORD32 frame_ok = pstr_usac_data->frame_ok;
  ia_ec_state_str *pstr_ec_state = &pstr_usac_data->str_error_concealment[ch];

  if (pstr_usac_data->core_mode == CORE_MODE_FD) {
    if (pstr_ec_state->win_shape == (UWORD8)-1) {
      pstr_ec_state->win_shape = pstr_usac_data->window_shape[ch];
    }

    ixheaacd_usac_ec_state(pstr_ec_state, frame_ok);

    if (pstr_ec_state->conceal_state == FRAME_OKAY) {
      pstr_ec_state->prev_core_mode = pstr_usac_data->core_mode;
      ixheaacd_usac_ec_save_states(pstr_ec_state, pstr_usac_data, ch);
    } else if (pstr_ec_state->conceal_state == FRAME_CONCEAL_SINGLE) {
      ixheaacd_usac_ec_interpolate_frame(pstr_usac_data, pstr_ec_state, pstr_samp_rate_info,
                                         frame_ok, ch);
    } else {
    }
    if (!frame_ok) {
      WORD32 *ptr_spec_coeff = pstr_usac_data->coef_fix[ch];
      WORD16 *ptr_spec_scale = pstr_usac_data->spec_scale[ch];

      pstr_usac_data->window_sequence[ch] = pstr_ec_state->win_seq;
      pstr_usac_data->window_shape[ch] = pstr_ec_state->win_shape;

      if (pstr_ec_state->conceal_state != FRAME_MUTE) {
        memcpy(ptr_spec_scale, pstr_ec_state->q_spec_coeff, sizeof(pstr_ec_state->q_spec_coeff));
        memcpy(ptr_spec_coeff, pstr_ec_state->spectral_coeff,
               sizeof(pstr_ec_state->spectral_coeff));
      } else {
        memset(ptr_spec_scale, 0, MAX_SPEC_SCALE_LEN * sizeof(ptr_spec_scale[0]));
        memset(ptr_spec_coeff, 0, pstr_usac_data->ccfl * sizeof(ptr_spec_coeff[0]));
      }
    }
  } else {
    ixheaacd_usac_lpc_ec_state(pstr_ec_state, frame_ok);

    if (pstr_ec_state->conceal_state == FRAME_OKAY) {
      memcpy(pstr_ec_state->lsf4, pstr_usac_data->lpc4_lsf, sizeof(pstr_ec_state->lsf4));
    } else if (pstr_ec_state->conceal_state == FRAME_CONCEAL_SINGLE) {
      WORD32 frame_length = pstr_usac_data->ccfl;
      WORD32 *ptr_spec_coeff = pstr_usac_data->tcx_spec_coeffs[ch];

      ixheaacd_usac_flip_spec_sign(ptr_spec_coeff, frame_length,
                                   &pstr_usac_data->seed_value[ch]);
    } else {
      WORD32 *ptr_spec_coeff = pstr_usac_data->tcx_spec_coeffs[ch];
      memset(ptr_spec_coeff, 0, pstr_usac_data->ccfl * sizeof(ptr_spec_coeff[0]));
    }
    if (!frame_ok) {
      memcpy(pstr_usac_data->lpc4_lsf, pstr_ec_state->lsf4, sizeof(pstr_usac_data->lpc4_lsf));
    }
  }

  pstr_ec_state->prev_frame_ok[0] = pstr_ec_state->prev_frame_ok[1];
  pstr_ec_state->prev_frame_ok[1] = frame_ok;

  return;
}

static VOID ixheaacd_lpc_wt_tool(FLOAT32 a[], WORD32 l) {
  WORD32 i;

  for (i = 0; i < l; i++) {
    a[i] = a[i] * ixheaacd_gamma_table[i];
  }

  return;
}
static VOID ixheaacd_lpc_coef_gen_ec(FLOAT32 lsf_old[], FLOAT32 lsf_new[], FLOAT32 a[],
                                     WORD32 m) {
  FLOAT32 lsf[ORDER], *ptr_a;
  FLOAT32 inc, fnew, fold;
  WORD32 i;

  ptr_a = a;

  inc = 1.0f / (FLOAT32)m;
  fnew = 0.5f - (0.5f * inc);
  fold = 1.0f - fnew;

  for (i = 0; i < ORDER; i++) {
    lsf[i] = (lsf_old[i] * fold) + (lsf_new[i] * fnew);
  }
  ixheaacd_lsp_to_lp_conversion(lsf, ptr_a);

  return;
}

VOID ixheaacd_usac_tcx_ec(ia_usac_data_struct *pstr_usac_data, ia_usac_lpd_decoder_handle st,
                          FLOAT32 *ptr_lsp_curr, WORD32 frame_idx, FLOAT32 *lp_flt_coff_a) {
  WORD32 ch = pstr_usac_data->present_chan;
  FLOAT32 synth_buf[ORDER + LEN_FRAME], temp;
  FLOAT32 exc_buf[MAX_PITCH + ORDER + 1 + LEN_FRAME];
  FLOAT32 *ptr_syn = synth_buf + ORDER;
  FLOAT32 *ptr_exc = exc_buf + MAX_PITCH + ORDER + 1;
  FLOAT32 est_fac_est = 0.1f;
  WORD32 i, sf_idx;
  FLOAT32 synth_sig_buf[LEN_SUBFR + 1];
  FLOAT32 *synth_signal = synth_sig_buf + 1;
  WORD32 num_lost_frames = pstr_usac_data->num_lost_lpd_frames[ch];
  WORD32 len_subfrm = pstr_usac_data->len_subfrm;
  FLOAT32 past_tcx_gain = pstr_usac_data->past_gain_tcx[ch];
  WORD32 l_div_part = MAX_PITCH + ORDER + 1 - len_subfrm;
  FLOAT32 *synth = pstr_usac_data->synth_buf + MAX_PITCH - LEN_SUBFR;
  FLOAT32 *ptr_synth = &synth[512 + frame_idx * len_subfrm];
  FLOAT32 syn_buf[MAX_PITCH + ORDER + 1];
  FLOAT32 *ptr_syn_buf = &syn_buf[ORDER];

  memcpy(syn_buf, &ptr_synth[-(MAX_PITCH + ORDER + 1)],
         sizeof(syn_buf));
  memcpy(st->synth_prev_ec, &syn_buf[MAX_PITCH + 1], sizeof(st->synth_prev_ec));
  ixheaacd_residual_tool_float(pstr_usac_data->lp_flt_coff_a_ec, ptr_syn_buf, st->xcitation_prev,
                               pstr_usac_data->len_subfrm, 1);
  ixheaacd_residual_tool_float(lp_flt_coff_a, &syn_buf[l_div_part],
                               st->xcitation_prev + l_div_part, pstr_usac_data->len_subfrm, 1);
  if (st->last_tcx_pitch > MAX_PITCH) {
    st->last_tcx_pitch = MAX_PITCH;
  }

  memcpy(synth_buf, st->synth_prev_ec, ORDER * sizeof(FLOAT32));
  memcpy(exc_buf, st->xcitation_prev, (MAX_PITCH + ORDER + 1) * sizeof(FLOAT32));

  if (num_lost_frames <= 8) {
    est_fac_est = ixheaacd_exc_fade_fac[num_lost_frames - 1];
  }

  for (i = 0; i < len_subfrm; i++) {
    ptr_exc[i] = est_fac_est * ptr_exc[i - st->last_tcx_pitch];
  }
  synth_signal[-1] = ptr_exc[-1];

  for (sf_idx = 0; sf_idx < len_subfrm; sf_idx += LEN_SUBFR) {
    FLOAT32 lp_coef[ORDER + 1];

    ixheaacd_lpc_coef_gen_ec(st->lspold, ptr_lsp_curr, lp_coef, len_subfrm / LEN_SUBFR);

    ixheaacd_synthesis_tool_float(lp_coef, &ptr_exc[sf_idx], &ptr_syn[sf_idx], LEN_SUBFR,
                                  synth_buf);

    ixheaacd_lpc_wt_tool(lp_coef, ORDER);

    ixheaacd_residual_tool_float(lp_coef, &ptr_syn[sf_idx], synth_signal, LEN_SUBFR, 1);

    ixheaacd_deemphsis_tool(synth_signal, LEN_SUBFR, synth_signal[-1]);

    temp = (est_fac_est * past_tcx_gain);

    for (i = 0; i < LEN_SUBFR; i++) {
      if (synth_signal[i] > temp) {
        synth_signal[i] = temp;
      } else {
        if (synth_signal[i] < -temp) {
          synth_signal[i] = -temp;
        }
      }
    }

    for (i = LEN_SUBFR - 1; i >= 0; i--) {
      synth_signal[i] = (synth_signal[i] - (PREEMPH_FILT_FAC * synth_signal[i - 1]));
    }
    ixheaacd_synthesis_tool_float(lp_coef, synth_signal, &ptr_syn[sf_idx], LEN_SUBFR, synth_buf);

    memmove(&ptr_synth[sf_idx], &ptr_syn[sf_idx], LEN_SUBFR * sizeof(FLOAT32));
  }

  memcpy(st->xcitation_prev, exc_buf + len_subfrm,
         sizeof(FLOAT32) * (MAX_PITCH + ORDER + 1));
  memcpy(st->synth_prev_ec, synth_buf + len_subfrm, sizeof(FLOAT32) * ORDER);
  return;
}
