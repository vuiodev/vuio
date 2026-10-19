/******************************************************************************
 *                                                                            *
 * Copyright (C) 2018 The Android Open Source Project
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
#include <float.h>
#include <stdlib.h>
#include <stdio.h>
#include <math.h>
#include <string.h>

#include "ixheaac_type_def.h"

#include "ixheaacd_bitbuffer.h"

#include "ixheaacd_interface.h"

#include "ixheaacd_tns_usac.h"
#include "ixheaacd_cnst.h"

#include "ixheaacd_acelp_info.h"

#include "ixheaacd_td_mdct.h"

#include "ixheaacd_sbrdecsettings.h"
#include "ixheaacd_info.h"
#include "ixheaacd_sbr_common.h"
#include "ixheaacd_drc_data_struct.h"
#include "ixheaacd_drc_dec.h"
#include "ixheaacd_sbrdecoder.h"
#include "ixheaacd_mps_polyphase.h"
#include "ixheaac_sbr_const.h"
#include "ixheaacd_ec_defines.h"
#include "ixheaacd_ec_struct_def.h"
#include "ixheaacd_main.h"
#include "ixheaacd_arith_dec.h"
#include "ixheaacd_func_def.h"

#include "ixheaacd_acelp_com.h"

#define F_PIT_SHARP 0.85F
#define MEAN_ENER 30

extern const FLOAT32 ixheaacd_interpol_filt[INTER_LP_FIL_LEN];

VOID ixheaacd_acelp_pitch_sharpening(FLOAT32 *x, WORD32 pit_lag) {
  WORD32 i;
  for (i = pit_lag; i < LEN_SUBFR; i++) {
    x[i] += x[i - pit_lag] * F_PIT_SHARP;
  }
  return;
}

static VOID ixheaacd_acelp_decode_1sp_per_track(WORD32 idx_1p, WORD32 M,
                                                WORD32 ixheaacd_drc_offset,
                                                WORD32 track,
                                                FLOAT32 code_vec[]) {
  WORD32 sign_index, mask, m;
  WORD32 sp_pos;
  mask = ((1 << M) - 1);

  sp_pos = (idx_1p & mask) + ixheaacd_drc_offset;
  sign_index = ((idx_1p >> M) & 1);

  m = (sp_pos << 2) + track;
  if (sign_index == 1)
    code_vec[m] = (code_vec[m] - 1.0f);
  else
    code_vec[m] = (code_vec[m] + 1.0f);

  return;
}

static VOID ixheaacd_acelp_decode_2sp_per_track(WORD32 idx_2p, WORD32 M,
                                                WORD32 ixheaacd_drc_offset,
                                                WORD32 track,
                                                FLOAT32 code_vec[]) {
  WORD32 sign_index;
  WORD32 mask, m0, m1;
  WORD32 sp_pos[2];
  mask = ((1 << M) - 1);

  sp_pos[0] = (((idx_2p >> M) & mask) + ixheaacd_drc_offset);
  sp_pos[1] = ((idx_2p & mask) + ixheaacd_drc_offset);

  sign_index = (idx_2p >> 2 * M) & 1;

  m0 = (sp_pos[0] << 2) + track;
  m1 = (sp_pos[1] << 2) + track;

  if ((sp_pos[1] - sp_pos[0]) < 0) {
    if (sign_index == 1) {
      code_vec[m0] = (code_vec[m0] - 1.0f);
      code_vec[m1] = (code_vec[m1] + 1.0f);
    } else {
      code_vec[m0] = (code_vec[m0] + 1.0f);
      code_vec[m1] = (code_vec[m1] - 1.0f);
    }
  } else {
    if (sign_index == 1) {
      code_vec[m0] = (code_vec[m0] - 1.0f);
      code_vec[m1] = (code_vec[m1] - 1.0f);
    } else {
      code_vec[m0] = (code_vec[m0] + 1.0f);
      code_vec[m1] = (code_vec[m1] + 1.0f);
    }
  }
  return;
}

static VOID ixheaacd_acelp_decode_3sp_per_track(WORD32 idx_3p, WORD32 M,
                                                WORD32 ixheaacd_drc_offset,
                                                WORD32 track,
                                                FLOAT32 code_vec[]) {
  WORD32 j, mask, idx_2p, idx_1p;

  mask = ((1 << (2 * M - 1)) - 1);
  idx_2p = idx_3p & mask;
  j = ixheaacd_drc_offset;
  if (((idx_3p >> ((2 * M) - 1)) & 1) == 1) {
    j += (1 << (M - 1));
  }
  ixheaacd_acelp_decode_2sp_per_track(idx_2p, M - 1, j, track, code_vec);
  mask = ((1 << (M + 1)) - 1);
  idx_1p = (idx_3p >> 2 * M) & mask;
  ixheaacd_acelp_decode_1sp_per_track(idx_1p, M, ixheaacd_drc_offset, track,
                                      code_vec);
  return;
}

static VOID ixheaacd_d_acelp_decode_4sp_per_track_section(
    WORD32 index, WORD32 ixheaacd_drc_offset, WORD32 track,
    FLOAT32 code_vec[]) {
  WORD32 j, idx_2p;

  idx_2p = index & 31;
  j = ixheaacd_drc_offset;
  if (((index >> 5) & 1) == 1) {
    j += 4;
  }
  ixheaacd_acelp_decode_2sp_per_track(idx_2p, 2, j, track, code_vec);
  idx_2p = (index >> 6) & 127;
  ixheaacd_acelp_decode_2sp_per_track(idx_2p, 3, ixheaacd_drc_offset, track,
                                      code_vec);
  return;
}

static VOID ixheaacd_acelp_decode_4sp_per_track(WORD32 idx_4p, WORD32 track,
                                                FLOAT32 code_vec[]) {
  WORD32 idx_1p, idx_2p, idx_3p;

  switch ((idx_4p >> 14) & 3) {
    case 0:
      if (((idx_4p >> 13) & 1) == 0)
        ixheaacd_d_acelp_decode_4sp_per_track_section(idx_4p, 0, track,
                                                      code_vec);
      else
        ixheaacd_d_acelp_decode_4sp_per_track_section(idx_4p, 8, track,
                                                      code_vec);
      break;
    case 1:
      idx_1p = idx_4p >> 10;
      ixheaacd_acelp_decode_1sp_per_track(idx_1p, 3, 0, track, code_vec);
      ixheaacd_acelp_decode_3sp_per_track(idx_4p, 3, 8, track, code_vec);
      break;
    case 2:
      idx_2p = idx_4p >> 7;
      ixheaacd_acelp_decode_2sp_per_track(idx_2p, 3, 0, track, code_vec);
      ixheaacd_acelp_decode_2sp_per_track(idx_4p, 3, 8, track, code_vec);
      break;
    case 3:
      idx_3p = idx_4p >> 4;
      ixheaacd_acelp_decode_3sp_per_track(idx_3p, 3, 0, track, code_vec);
      ixheaacd_acelp_decode_1sp_per_track(idx_4p, 3, 8, track, code_vec);
      break;
  }
  return;
}

static VOID ixheaacd_d_acelp_add_pulse(WORD32 pos[], WORD32 nb_pulse,
                                       WORD32 track, FLOAT32 code[]) {
  WORD32 i, k;
  for (k = 0; k < nb_pulse; k++) {
    i = ((pos[k] & (16 - 1)) << 2) + track;
    if ((pos[k] & 16) == 0) {
      code[i] = (WORD16)(code[i] + 1.0f);
    } else {
      code[i] = (WORD16)(code[i] - 1.0f);
    }
  }
  return;
}

static VOID ixheaacd_d_acelp_decode_1p_n1(WORD32 index, WORD32 N,
                                          WORD32 ixheaacd_drc_offset,
                                          WORD32 pos[]) {
  WORD32 i, pos1, mask;
  mask = ((1 << N) - 1);

  pos1 = ((index & mask) + ixheaacd_drc_offset);
  i = ((index >> N) & 1);
  if (i == 1) {
    pos1 += 16;
  }
  pos[0] = pos1;
  return;
}

VOID ixheaacd_acelp_decode_pulses_per_track(WORD32 cb_index[],
                                            const WORD16 code_bits,
                                            FLOAT32 code_vec[]) {
  WORD32 track_idx, index, ixheaacd_drc_offset, pos[6], i;
  memset(code_vec, 0, 64 * sizeof(FLOAT32));

  if (code_bits == 12) {
    for (track_idx = 0; track_idx < 4; track_idx += 2) {
      ixheaacd_drc_offset = cb_index[2 * (track_idx / 2)];
      index = cb_index[2 * (track_idx / 2) + 1];
      ixheaacd_d_acelp_decode_1p_n1(index, 4, 0, pos);
      ixheaacd_d_acelp_add_pulse(
          pos, 1, 2 * ixheaacd_drc_offset + track_idx / 2, code_vec);
    }
  } else if (code_bits == 16) {
    i = 0;
    ixheaacd_drc_offset = cb_index[i++];
    ixheaacd_drc_offset = (ixheaacd_drc_offset == 0) ? 1 : 3;
    for (track_idx = 0; track_idx < 4; track_idx++) {
      if (track_idx != ixheaacd_drc_offset) {
        index = cb_index[i++];
        ixheaacd_d_acelp_decode_1p_n1(index, 4, 0, pos);
        ixheaacd_d_acelp_add_pulse(pos, 1, track_idx, code_vec);
      }
    }
  } else if (code_bits == 20) {
    for (track_idx = 0; track_idx < 4; track_idx++) {
      index = cb_index[track_idx];
      ixheaacd_acelp_decode_1sp_per_track(index, 4, 0, track_idx, code_vec);
    }
  } else if (code_bits == 28) {
    for (track_idx = 0; track_idx < 2; track_idx++) {
      index = cb_index[track_idx];
      ixheaacd_acelp_decode_2sp_per_track(index, 4, 0, track_idx, code_vec);
    }
    for (track_idx = 2; track_idx < 4; track_idx++) {
      index = cb_index[track_idx];
      ixheaacd_acelp_decode_1sp_per_track(index, 4, 0, track_idx, code_vec);
    }
  } else if (code_bits == 36) {
    for (track_idx = 0; track_idx < 4; track_idx++) {
      index = cb_index[track_idx];
      ixheaacd_acelp_decode_2sp_per_track(index, 4, 0, track_idx, code_vec);
    }
  } else if (code_bits == 44) {
    for (track_idx = 0; track_idx < 2; track_idx++) {
      index = cb_index[track_idx];
      ixheaacd_acelp_decode_3sp_per_track(index, 4, 0, track_idx, code_vec);
    }
    for (track_idx = 2; track_idx < 4; track_idx++) {
      index = cb_index[track_idx];
      ixheaacd_acelp_decode_2sp_per_track(index, 4, 0, track_idx, code_vec);
    }
  } else if (code_bits == 52) {
    for (track_idx = 0; track_idx < 4; track_idx++) {
      index = cb_index[track_idx];
      ixheaacd_acelp_decode_3sp_per_track(index, 4, 0, track_idx, code_vec);
    }
  } else if (code_bits == 64) {
    for (track_idx = 0; track_idx < 4; track_idx++) {
      index = ((cb_index[track_idx] << 14) + cb_index[track_idx + 4]);
      ixheaacd_acelp_decode_4sp_per_track(index, track_idx, code_vec);
    }
  }
  return;
}

static void ixheaacd_acelp_decode_gains(WORD32 index, FLOAT32 code_vec[],
                                        FLOAT32 *pitch_gain,
                                        FLOAT32 *codebook_gain,
                                        FLOAT32 mean_exc_energy,
                                        FLOAT32 *energy) {
  WORD32 i;
  FLOAT32 avg_innov_energy, est_gain;
  const FLOAT32 *gain_table = ixheaacd_int_leave_gain_table;

  avg_innov_energy = 0.01f;
  for (i = 0; i < LEN_SUBFR; i++) {
    avg_innov_energy += code_vec[i] * code_vec[i];
  }
  *energy = avg_innov_energy;

  avg_innov_energy =
      (FLOAT32)(10.0 * log10(avg_innov_energy / (FLOAT32)LEN_SUBFR));

  est_gain = mean_exc_energy - avg_innov_energy;

  est_gain = (FLOAT32)pow(10.0, 0.05 * est_gain);
  *pitch_gain = gain_table[index * 2];

  *codebook_gain = gain_table[index * 2 + 1] * est_gain;

  return;
}

static VOID ixheaacd_acelp_decode_gains_with_ec(WORD32 index, FLOAT32 code_vec[],
                                                FLOAT32 *pitch_gain, FLOAT32 *codebook_gain,
                                                FLOAT32 mean_exc_energy, FLOAT32 *energy,
                                                FLOAT32 *past_pitch_gain, FLOAT32 *past_gain_code,
                                                WORD32 bfi) {
  WORD32 i;
  FLOAT32 avg_innov_energy, est_gain, gain_inov;
  const FLOAT32 *gain_table = ixheaacd_int_leave_gain_table;

  avg_innov_energy = 0.01f;
  for (i = 0; i < LEN_SUBFR; i++) {
    avg_innov_energy += code_vec[i] * code_vec[i];
  }
  *energy = avg_innov_energy;
  gain_inov = (FLOAT32)(1 / sqrt(avg_innov_energy / LEN_SUBFR));

  if (bfi) {
    FLOAT32 tgpit = (*past_pitch_gain);

    if (tgpit > 0.95f) {
      tgpit = 0.95f;
    } else if (tgpit < 0.5f) {
      tgpit = 0.5f;
    }
    *pitch_gain = (FLOAT32)tgpit;
    tgpit = tgpit * 0.95f;
    *past_pitch_gain = (FLOAT32)tgpit;

    tgpit = 1.4f - tgpit;
    tgpit = *past_gain_code * tgpit;
    *codebook_gain = tgpit * gain_inov;

    *past_gain_code = tgpit;
    return;
  }

  avg_innov_energy = (FLOAT32)(10.0 * log10(avg_innov_energy / (FLOAT32)LEN_SUBFR));
  est_gain = mean_exc_energy - avg_innov_energy;

  est_gain = (FLOAT32)pow(10.0, 0.05 * est_gain);
  if (!bfi) {
    *pitch_gain = gain_table[index * 2];
    *past_pitch_gain = *pitch_gain;
  }

  *codebook_gain = gain_table[index * 2 + 1] * est_gain;
  *past_gain_code = (*codebook_gain) / gain_inov;

  return;
}

static VOID ixheaacd_cb_exc_calc(FLOAT32 xcitation_curr[], WORD32 pitch_lag,
                                 WORD32 frac) {
  WORD32 i, j;
  FLOAT32 s, *x0, *x1, *x2;
  const FLOAT32 *c1, *c2;

  x0 = &xcitation_curr[-pitch_lag];
  frac = -frac;
  if (frac < 0) {
    frac += UP_SAMP;
    x0--;
  }
  for (j = 0; j < LEN_SUBFR + 1; j++) {
    x1 = x0++;
    x2 = x1 + 1;
    c1 = &ixheaacd_interpol_filt[frac];
    c2 = &ixheaacd_interpol_filt[UP_SAMP - frac];
    s = 0.0;
    for (i = 0; i < INTER_LP_FIL_ORDER; i++, c1 += UP_SAMP, c2 += UP_SAMP) {
      s += (*x1--) * (*c1) + (*x2++) * (*c2);
    }
    xcitation_curr[j] = s;
  }
  return;
}

VOID ixheaacd_acelp_alias_cnx(ia_usac_data_struct *usac_data,
                              ia_td_frame_data_struct *pstr_td_frame_data, WORD32 k,
                              FLOAT32 lp_filt_coeff[], FLOAT32 stability_factor,
                              ia_usac_lpd_decoder_handle st) {
  WORD32 i, subfr_idx;
  WORD32 pitch_lag = 0, pitch_lag_frac = 0, index, pitch_flag, pitch_lag_max;
  WORD32 pitch_lag_min = 0;
  FLOAT32 tmp, pitch_gain, gain_code, voicing_factor, r_v, innov_energy,
      pitch_energy, mean_ener_code;
  FLOAT32 gain_smooth, gain_code0, cpe;
  FLOAT32 code[LEN_SUBFR] = {0}, synth_temp[128 + 16] = {0};
  FLOAT32 post_process_exc[LEN_SUBFR] = {0};
  FLOAT32 gain_smooth_factor;
  FLOAT32 *ptr_lp_filt_coeff;
  WORD32 pitch_min;
  WORD32 pitch_fr2;
  WORD32 pitch_fr1;
  WORD32 pitch_max;
  WORD32 subfr_nb = 0;
  const WORD16 num_codebits_table[8] = {20, 28, 36, 44, 52, 64, 12, 16};
  FLOAT32 x[FAC_LENGTH] = {0}, xn2[2 * FAC_LENGTH + 16] = {0};
  WORD32 int_x[FAC_LENGTH] = {0};
  WORD32 TTT;
  WORD32 len_subfr = usac_data->len_subfrm;
  WORD32 fac_length;
  WORD8 shiftp;
  WORD32 preshift;
  WORD32 *ptr_scratch = &usac_data->scratch_buffer[0];
  WORD32 *int_xn2 = &usac_data->x_ac_dec[0];
  WORD32 loop_count = 0;
  WORD32 core_mode = pstr_td_frame_data->acelp_core_mode;
  FLOAT32 *synth_signal =
      &usac_data->synth_buf[len_subfr * k + MAX_PITCH +
                            (((NUM_FRAMES * usac_data->num_subfrm) / 2) - 1) *
                                LEN_SUBFR];
  FLOAT32 *xcitation_curr =
      &usac_data->exc_buf[len_subfr * k + MAX_PITCH + (INTER_LP_FIL_ORDER + 1)];
  FLOAT32 *ptr_pitch_gain =
      &usac_data->pitch_gain[k * usac_data->num_subfrm +
                             (((NUM_FRAMES * usac_data->num_subfrm) / 2) - 1)];
  WORD32 *ptr_pitch =
      &usac_data->pitch[k * usac_data->num_subfrm +
                        (((NUM_FRAMES * usac_data->num_subfrm) / 2) - 1)];
  fac_length = len_subfr / 2;

  WORD32 bfi = (usac_data->num_lost_lpd_frames[usac_data->present_chan] > 0) ? 1 : 0;
  WORD32 i_offset =
      (usac_data->str_tddec[usac_data->present_chan]->fscale * TMIN + (FSCALE_DENOM / 2)) /
          FSCALE_DENOM -
      TMIN;
  const WORD32 pitch_max_val = TMAX + (6 * i_offset);
  WORD16 code_t[LEN_SUBFR];

  if (st->mode_prev > 0) {
    for (i = 0; i < fac_length / 2; i++) {
      x[i] = st->fac_gain * pstr_td_frame_data->fac[k * FAC_LENGTH + 2 * i];
      x[fac_length / 2 + i] =
          st->fac_gain *
          pstr_td_frame_data->fac[k * FAC_LENGTH + fac_length - 2 * i - 1];
    }
    for (i = 0; i < fac_length / 8; i++) {
      x[i] *= st->fac_fd_data[2 * i];
      x[fac_length - i - 1] *= st->fac_fd_data[2 * i + 1];
    }

    preshift = 0;
    shiftp = ixheaacd_float2fix(x, int_x, fac_length);

    ixheaacd_acelp_mdct(int_x, int_xn2, &preshift, fac_length, ptr_scratch);

    ixheaacd_fix2float(int_xn2, xn2 + fac_length, fac_length, &shiftp,
                       &preshift);

    ixheaacd_vec_cnst_mul((2.0f / (FLOAT32)fac_length), xn2 + fac_length,
                          xn2 + fac_length, fac_length);

    memset(xn2, 0, fac_length * sizeof(FLOAT32));

    ixheaacd_lpc_wt_synthesis_tool(st->lp_flt_coeff_a_prev, xn2 + fac_length,
                                   fac_length);

    for (i = 0; i < 2 * fac_length; i++)
      xn2[i] += synth_signal[i - (2 * fac_length)];

    memcpy(synth_signal - fac_length, xn2 + fac_length,
           fac_length * sizeof(FLOAT32));

    tmp = 0.0;
    ixheaacd_preemphsis_tool_float(xn2, PREEMPH_FILT_FAC, 2 * fac_length, tmp);

    ptr_lp_filt_coeff = st->lp_flt_coeff_a_prev;
    TTT = fac_length % LEN_SUBFR;
    if (TTT != 0) {
      ixheaacd_residual_tool_float(
          ptr_lp_filt_coeff, &xn2[fac_length],
          &xcitation_curr[fac_length - (2 * fac_length)], TTT, 1);
      ptr_lp_filt_coeff += (ORDER + 1);
    }

    loop_count = (fac_length + TTT) / LEN_SUBFR;
    ixheaacd_residual_tool_float(ptr_lp_filt_coeff, &xn2[fac_length + TTT],
                                 &xcitation_curr[TTT - fac_length], LEN_SUBFR,
                                 loop_count);
  }

  for (i = 0; i < ORDER; i++)
    synth_temp[i] = synth_signal[i - ORDER] -
                    (PREEMPH_FILT_FAC * synth_signal[i - ORDER - 1]);

  i = (((st->fscale * TMIN) + (FSCALE_DENOM / 2)) / FSCALE_DENOM) - TMIN;
  pitch_min = TMIN + i;
  pitch_fr2 = TFR2 - i;
  pitch_fr1 = TFR1;
  pitch_max = TMAX + (6 * i);

  ptr_lp_filt_coeff = lp_filt_coeff;
  for (subfr_idx = 0; subfr_idx < len_subfr; subfr_idx += LEN_SUBFR) {
    pitch_flag = subfr_idx;
    if ((len_subfr == 256) && (subfr_idx == (2 * LEN_SUBFR))) {
      pitch_flag = 0;
    }
    index = pstr_td_frame_data->acb_index[k * 4 + subfr_nb];

    if (pitch_flag == 0) {
      if (index < (pitch_fr2 - pitch_min) * 4) {
        pitch_lag = pitch_min + (index / 4);
        pitch_lag_frac = index - (pitch_lag - pitch_min) * 4;
      } else if (index <
                 ((pitch_fr2 - pitch_min) * 4 + (pitch_fr1 - pitch_fr2) * 2)) {
        index -= (pitch_fr2 - pitch_min) * 4;
        pitch_lag = pitch_fr2 + (index / 2);
        pitch_lag_frac = index - (pitch_lag - pitch_fr2) * 2;
        pitch_lag_frac *= 2;
      } else {
        pitch_lag = index + pitch_fr1 - ((pitch_fr2 - pitch_min) * 4) -
                    ((pitch_fr1 - pitch_fr2) * 2);
        pitch_lag_frac = 0;
      }
      pitch_lag_min = pitch_lag - 8;
      if (pitch_lag_min < pitch_min) pitch_lag_min = pitch_min;

      pitch_lag_max = pitch_lag_min + 15;
      if (pitch_lag_max > pitch_max) {
        pitch_lag_max = pitch_max;
        pitch_lag_min = pitch_lag_max - 15;
      }
    } else {
      pitch_lag = pitch_lag_min + index / 4;
      pitch_lag_frac = index - (pitch_lag - pitch_lag_min) * 4;
    }

    if (usac_data->ec_flag) {
      if (bfi) {
        if (usac_data->pitch_lag_old >= pitch_max_val) {
          usac_data->pitch_lag_old = (pitch_max_val - 5);
        }
        pitch_lag = usac_data->pitch_lag_old;
        pitch_lag_frac = usac_data->pitch_lag_frac_old;
      }
    }
    ixheaacd_cb_exc_calc(&xcitation_curr[subfr_idx], pitch_lag, pitch_lag_frac);

    mean_ener_code =
        (((FLOAT32)pstr_td_frame_data->mean_energy[k]) * 12.0f) + 18.0f;

    if (pstr_td_frame_data->ltp_filtering_flag[k * 4 + subfr_nb] == 0) {
      for (i = 0; i < LEN_SUBFR; i++)
        code[i] = (FLOAT32)(0.18 * xcitation_curr[i - 1 + subfr_idx] +
                            0.64 * xcitation_curr[i + subfr_idx] +
                            0.18 * xcitation_curr[i + 1 + subfr_idx]);

      ixheaacd_mem_cpy(code, &xcitation_curr[subfr_idx], LEN_SUBFR);
    }
    if (usac_data->frame_ok == 1) {
      ixheaacd_acelp_decode_pulses_per_track(
          &(pstr_td_frame_data->icb_index[k * 4 + subfr_nb][0]), num_codebits_table[core_mode],
          code);
    } else {
      if (usac_data->ec_flag) {
        WORD32 idx;
        if (bfi) {
          for (idx = 0; idx < LEN_SUBFR; idx++) {
            usac_data->seed_ace = ((((WORD32)usac_data->seed_ace * 31821) >> 1) + 13849);
            code_t[idx] = (WORD16)((usac_data->seed_ace) >> 4);
            code[idx] = ((FLOAT32)code_t[idx] / 512);
          }
        }
      }
    }

    tmp = 0.0;
    ixheaacd_preemphsis_tool_float(code, TILT_CODE, LEN_SUBFR, tmp);
    i = pitch_lag;
    if (pitch_lag_frac > 2) i++;
    if (i >= 0) ixheaacd_acelp_pitch_sharpening(code, i);

    index = pstr_td_frame_data->gains[k * 4 + subfr_nb];
    if (usac_data->ec_flag) {
      ixheaacd_acelp_decode_gains_with_ec(index, code, &pitch_gain, &gain_code, mean_ener_code,
                                          &innov_energy, &usac_data->past_pitch_gain,
                                          &usac_data->past_gain_code, bfi);
    } else {
      ixheaacd_acelp_decode_gains(index, code, &pitch_gain, &gain_code, mean_ener_code,
                                  &innov_energy);
    }

    pitch_energy = 0.0;
    for (i = 0; i < LEN_SUBFR; i++)
      pitch_energy +=
          xcitation_curr[i + subfr_idx] * xcitation_curr[i + subfr_idx];

    pitch_energy *= (pitch_gain * pitch_gain);

    innov_energy *= gain_code * gain_code;

    r_v = (FLOAT32)((pitch_energy - innov_energy) /
                    (pitch_energy + innov_energy));

    for (i = 0; i < LEN_SUBFR; i++)
      post_process_exc[i] = pitch_gain * xcitation_curr[i + subfr_idx];

    for (i = 0; i < LEN_SUBFR; i++)
      xcitation_curr[i + subfr_idx] =
          pitch_gain * xcitation_curr[i + subfr_idx] + gain_code * code[i];

    i = pitch_lag;
    if (pitch_lag_frac > 2) i++;

    if (i > pitch_max) i = pitch_max;

    *ptr_pitch++ = i;
    *ptr_pitch_gain++ = pitch_gain;

    voicing_factor = (FLOAT32)(0.5 * (1.0 - r_v));
    gain_smooth_factor = stability_factor * voicing_factor;
    gain_code0 = gain_code;
    if (gain_code0 < st->gain_threshold) {
      gain_code0 = (FLOAT32)(gain_code0 * 1.19);
      if (gain_code0 > st->gain_threshold) gain_code0 = st->gain_threshold;
    } else {
      gain_code0 = (FLOAT32)(gain_code0 / 1.19);
      if (gain_code0 < st->gain_threshold) gain_code0 = st->gain_threshold;
    }
    st->gain_threshold = gain_code0;
    gain_smooth = (FLOAT32)((gain_smooth_factor * gain_code0) +
                            ((1.0 - gain_smooth_factor) * gain_code));
    for (i = 0; i < LEN_SUBFR; i++) code[i] *= gain_smooth;

    cpe = (FLOAT32)(0.125 * (1.0 + r_v));

    post_process_exc[0] += code[0] - (cpe * code[1]);

    for (i = 1; i < LEN_SUBFR - 1; i++)
      post_process_exc[i] += code[i] - (cpe * (code[i - 1] + code[i + 1]));

    post_process_exc[LEN_SUBFR - 1] +=
        code[LEN_SUBFR - 1] - (cpe * code[LEN_SUBFR - 2]);

    ixheaacd_synthesis_tool_float(ptr_lp_filt_coeff, post_process_exc,
                                  &synth_signal[subfr_idx], LEN_SUBFR,
                                  synth_temp);
    memcpy(synth_temp, &synth_signal[subfr_idx + LEN_SUBFR - ORDER],
           ORDER * sizeof(FLOAT32));

    ptr_lp_filt_coeff += (ORDER + 1);
    subfr_nb++;
  }

  ixheaacd_deemphsis_tool(synth_signal, len_subfr, synth_signal[-1]);

  memset(synth_temp + 16, 0, 128 * sizeof(FLOAT32));
  ixheaacd_synthesis_tool_float1(ptr_lp_filt_coeff, synth_temp + 16, 128);

  ptr_lp_filt_coeff -= (2 * (ORDER + 1));
  memcpy(st->lp_flt_coeff_a_prev, ptr_lp_filt_coeff,
         (2 * (ORDER + 1)) * sizeof(FLOAT32));

  memcpy(st->exc_prev, synth_signal + len_subfr - (1 + fac_length),
         (1 + fac_length) * sizeof(FLOAT32));

  memcpy(st->exc_prev + 1 + fac_length, synth_temp + 16,
         fac_length * sizeof(FLOAT32));
  ixheaacd_deemphsis_tool(st->exc_prev + 1 + fac_length, fac_length,
                          synth_signal[len_subfr - 1]);

  if (usac_data->ec_flag) {
    usac_data->pitch_lag_old = pitch_lag;
    usac_data->pitch_lag_frac_old = pitch_lag_frac;
  }
  return;
}
