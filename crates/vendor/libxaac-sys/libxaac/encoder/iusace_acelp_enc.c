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
#include <math.h>
#include "ixheaac_type_def.h"
#include "iusace_bitbuffer.h"

/* DRC */
#include "impd_drc_common_enc.h"
#include "impd_drc_uni_drc.h"
#include "impd_drc_tables.h"
#include "impd_drc_api.h"
#include "impd_drc_uni_drc_eq.h"
#include "impd_drc_uni_drc_filter_bank.h"
#include "impd_drc_gain_enc.h"
#include "impd_drc_struct_def.h"

#include "iusace_cnst.h"
#include "iusace_tns_usac.h"
#include "iusace_psy_mod.h"
#include "ixheaace_adjust_threshold_data.h"
#include "iusace_fd_qc_util.h"
#include "iusace_config.h"
#include "iusace_arith_enc.h"
#include "iusace_block_switch_const.h"
#include "iusace_block_switch_struct_def.h"
#include "iusace_lpd_rom.h"
#include "iusace_lpd.h"

VOID iusace_acelp_encode(FLOAT32 *lp_filt_coeff, FLOAT32 *quant_lp_filt_coeff, FLOAT32 *speech_in,
                         FLOAT32 *wsig_in, FLOAT32 *synth_out, FLOAT32 *wsynth_out,
                         WORD16 acelp_core_mode, ia_usac_lpd_state_struct *lpd_state,
                         WORD32 len_subfrm, FLOAT32 norm_corr, FLOAT32 norm_corr2,
                         WORD32 ol_pitch_lag1, WORD32 ol_pitch_lag2, WORD32 pit_adj,
                         WORD32 *acelp_params, iusace_scratch_mem *pstr_scratch) {
  WORD32 i, i_subfr, num_bits, t;
  WORD32 t0, t0_min, t0_max, index, subfrm_flag;
  WORD32 t0_frac;
  FLOAT32 temp, energy, max_ener, mean_ener_code;
  FLOAT32 pitch_gain, code_gain, gain1, gain2;
  FLOAT32 tgt_cb_corr[5], tgt_cb_corr2[2];
  FLOAT32 *p_lp_filt_coeff, *p_quant_lp_filt_coeff, weighted_lpc[ORDER + 1];
  FLOAT32 imp_res[LEN_SUBFR];
  FLOAT32 code[LEN_SUBFR];
  WORD16 cb_exc[LEN_SUBFR];
  FLOAT32 error[ORDER + LEN_SUBFR + 8];
  FLOAT32 cn[LEN_SUBFR];
  FLOAT32 xn[LEN_SUBFR];
  FLOAT32 xn2[LEN_SUBFR];
  FLOAT32 dn[LEN_SUBFR];
  FLOAT32 y0[LEN_SUBFR];
  FLOAT32 y1[LEN_SUBFR];
  FLOAT32 y2[LEN_SUBFR];
  WORD32 min_pitch_lag_res1_4;
  WORD32 min_pitch_lag_res1_2;
  WORD32 min_pitch_lag_res1;
  WORD32 max_pitch_lag;
  FLOAT32 *exc_buf = pstr_scratch->p_acelp_exc_buf;
  FLOAT32 *exc;
  FLOAT32 mem_txn, mem_txnq;
  WORD32 fac_length = len_subfrm / 2;
  if (lpd_state->mode > 0) {
    for (i = 0; i < fac_length; i++) {
      acelp_params[i] = lpd_state->avq_params[i];
    }
    acelp_params += fac_length;
  }

  if (pit_adj == SR_MAX)
    exc = exc_buf + (2 * len_subfrm) + 41;
  else
    exc = exc_buf + (2 * len_subfrm);

  memset(exc_buf, 0, (2 * len_subfrm) * sizeof(exc_buf[0]));
  memcpy(exc_buf, lpd_state->acelp_exc, 2 * len_subfrm * sizeof(FLOAT32));
  memcpy(synth_out - 128, &(lpd_state->synth[ORDER]), 128 * sizeof(FLOAT32));
  memcpy(wsynth_out - 128, &(lpd_state->wsynth[1]), 128 * sizeof(FLOAT32));

  num_bits = ((iusace_acelp_core_numbits_1024[acelp_core_mode] - NBITS_MODE) / 4) - NBITS_LPC;

  if (pit_adj == 0) {
    min_pitch_lag_res1_4 = TMIN;
    min_pitch_lag_res1_2 = TFR2;
    min_pitch_lag_res1 = TFR1;
    max_pitch_lag = TMAX;
  } else {
    i = (((pit_adj * TMIN) + (FSCALE_DENOM / 2)) / FSCALE_DENOM) - TMIN;
    min_pitch_lag_res1_4 = TMIN + i;
    min_pitch_lag_res1_2 = TFR2 - i;
    min_pitch_lag_res1 = TFR1;
    max_pitch_lag = TMAX + (6 * i);
  }

  ol_pitch_lag1 *= OPL_DECIM;
  ol_pitch_lag2 *= OPL_DECIM;

  t0_min = ol_pitch_lag1 - 8;

  t = MIN(ol_pitch_lag1, ol_pitch_lag2) - 4;
  if (t0_min < t) t0_min = t;

  if (t0_min < min_pitch_lag_res1_4) {
    t0_min = min_pitch_lag_res1_4;
  }
  t0_max = t0_min + 15;
  t = MAX(ol_pitch_lag1, ol_pitch_lag2) + 4;
  if (t0_max > t) t0_max = t;

  if (t0_max > max_pitch_lag) {
    t0_max = max_pitch_lag;
    t0_min = t0_max - 15;
  }

  max_ener = 0.0;
  mean_ener_code = 0.0;
  p_quant_lp_filt_coeff = quant_lp_filt_coeff;
  for (i_subfr = 0; i_subfr < len_subfrm; i_subfr += LEN_SUBFR) {
    iusace_compute_lp_residual(p_quant_lp_filt_coeff, &speech_in[i_subfr], &exc[i_subfr],
                               LEN_SUBFR);
    energy = 0.01f;
    for (i = 0; i < LEN_SUBFR; i++) {
      energy += exc[i + i_subfr] * exc[i + i_subfr];
    }
    energy = 10.0f * (FLOAT32)log10(energy / ((FLOAT32)LEN_SUBFR));
    if (energy < 0.0) {
      energy = 0.0;
    }
    if (energy > max_ener) {
      max_ener = energy;
    }
    mean_ener_code += 0.25f * energy;
    p_quant_lp_filt_coeff += (ORDER + 1);
  }

  mean_ener_code -= 5.0f * norm_corr;
  mean_ener_code -= 5.0f * norm_corr2;

  temp = (mean_ener_code - 18.0f) / 12.0f;
  index = (WORD32)floor(temp + 0.5);
  if (index < 0) {
    index = 0;
  }
  if (index > 3) {
    index = 3;
  }
  mean_ener_code = (((FLOAT32)index) * 12.0f) + 18.0f;

  while ((mean_ener_code < (max_ener - 27.0)) && (index < 3)) {
    index++;
    mean_ener_code += 12.0;
  }
  *acelp_params = index;
  acelp_params++;

  p_lp_filt_coeff = lp_filt_coeff;
  p_quant_lp_filt_coeff = quant_lp_filt_coeff;
  for (i_subfr = 0; i_subfr < len_subfrm; i_subfr += LEN_SUBFR) {
    subfrm_flag = i_subfr;
    if ((len_subfrm == 256) && (i_subfr == (2 * LEN_SUBFR))) {
      subfrm_flag = 0;

      t0_min = ol_pitch_lag2 - 8;

      t = MIN(ol_pitch_lag1, ol_pitch_lag2) - 4;
      if (t0_min < t) t0_min = t;

      if (t0_min < min_pitch_lag_res1_4) {
        t0_min = min_pitch_lag_res1_4;
      }
      t0_max = t0_min + 15;

      t = MAX(ol_pitch_lag1, ol_pitch_lag2) + 4;
      if (t0_max > t) t0_max = t;

      if (t0_max > max_pitch_lag) {
        t0_max = max_pitch_lag;
        t0_min = t0_max - 15;
      }
    }

    memcpy(xn, &wsig_in[i_subfr], LEN_SUBFR * sizeof(FLOAT32));

    memcpy(error, &synth_out[i_subfr - ORDER], ORDER * sizeof(FLOAT32));
    memset(error + ORDER, 0, LEN_SUBFR * sizeof(FLOAT32));
    iusace_synthesis_tool_float(p_quant_lp_filt_coeff, error + ORDER, error + ORDER, LEN_SUBFR,
                                error, pstr_scratch->p_buf_synthesis_tool);
    iusace_get_weighted_lpc(p_lp_filt_coeff, weighted_lpc);
    iusace_compute_lp_residual(weighted_lpc, error + ORDER, xn2, LEN_SUBFR);

    temp = wsynth_out[i_subfr - 1];
    iusace_apply_deemph(xn2, TILT_FAC, LEN_SUBFR, &temp);
    memcpy(y0, xn2, LEN_SUBFR * sizeof(FLOAT32));

    for (i = 0; i < LEN_SUBFR; i++) {
      xn[i] -= xn2[i];
    }
    iusace_compute_lp_residual(p_quant_lp_filt_coeff, &speech_in[i_subfr], &exc[i_subfr],
                               LEN_SUBFR);

    memset(&code[0], 0, ORDER * sizeof(code[0]));
    memcpy(code + ORDER, xn, (LEN_SUBFR / 2) * sizeof(FLOAT32));
    temp = 0.0;
    iusace_apply_preemph(code + ORDER, TILT_FAC, LEN_SUBFR / 2, &temp);
    iusace_get_weighted_lpc(p_lp_filt_coeff, weighted_lpc);
    iusace_synthesis_tool_float(weighted_lpc, code + ORDER, code + ORDER, LEN_SUBFR / 2, code,
                                pstr_scratch->p_buf_synthesis_tool);
    iusace_compute_lp_residual(p_quant_lp_filt_coeff, code + ORDER, cn, LEN_SUBFR / 2);
    memcpy(cn + (LEN_SUBFR / 2), &exc[i_subfr + (LEN_SUBFR / 2)],
           (LEN_SUBFR / 2) * sizeof(FLOAT32));

    iusace_get_weighted_lpc(p_lp_filt_coeff, weighted_lpc);
    memset(imp_res, 0, LEN_SUBFR * sizeof(FLOAT32));
    memcpy(imp_res, weighted_lpc, (ORDER + 1) * sizeof(FLOAT32));
    iusace_synthesis_tool_float(p_quant_lp_filt_coeff, imp_res, imp_res, LEN_SUBFR,
                                &imp_res[ORDER + 1], pstr_scratch->p_buf_synthesis_tool);
    temp = 0.0;
    iusace_apply_deemph(imp_res, TILT_FAC, LEN_SUBFR, &temp);

    iusace_closed_loop_search(&exc[i_subfr], xn, imp_res, t0_min, t0_max, &t0_frac, subfrm_flag,
                              min_pitch_lag_res1_2, min_pitch_lag_res1, &t0);

    if (subfrm_flag == 0) {
      if (t0 < min_pitch_lag_res1_2) {
        index = t0 * 4 + t0_frac - (min_pitch_lag_res1_4 * 4);
      } else if (t0 < min_pitch_lag_res1) {
        index = t0 * 2 + (t0_frac >> 1) - (min_pitch_lag_res1_2 * 2) +
                ((min_pitch_lag_res1_2 - min_pitch_lag_res1_4) * 4);
      } else {
        index = t0 - min_pitch_lag_res1 + ((min_pitch_lag_res1_2 - min_pitch_lag_res1_4) * 4) +
                ((min_pitch_lag_res1 - min_pitch_lag_res1_2) * 2);
      }

      t0_min = t0 - 8;
      if (t0_min < min_pitch_lag_res1_4) {
        t0_min = min_pitch_lag_res1_4;
      }
      t0_max = t0_min + 15;
      if (t0_max > max_pitch_lag) {
        t0_max = max_pitch_lag;
        t0_min = t0_max - 15;
      }
    } else {
      i = t0 - t0_min;
      index = i * 4 + t0_frac;
    }
    *acelp_params = index;
    acelp_params++;

    iusace_acelp_ltpred_cb_exc(&exc[i_subfr], t0, t0_frac, LEN_SUBFR + 1);
    iusace_convolve(&exc[i_subfr], imp_res, y1);
    gain1 = iusace_acelp_tgt_cb_corr2(xn, y1, tgt_cb_corr);
    iusace_acelp_cb_target_update(xn, xn2, y1, gain1);
    energy = 0.0;
    for (i = 0; i < LEN_SUBFR; i++) {
      energy += xn2[i] * xn2[i];
    }

    for (i = 0; i < LEN_SUBFR; i++) {
      code[i] = (FLOAT32)(0.18 * exc[i - 1 + i_subfr] + 0.64 * exc[i + i_subfr] +
                          0.18 * exc[i + 1 + i_subfr]);
    }
    iusace_convolve(code, imp_res, y2);
    gain2 = iusace_acelp_tgt_cb_corr2(xn, y2, tgt_cb_corr2);

    iusace_acelp_cb_target_update(xn, xn2, y2, gain2);
    temp = 0.0;
    for (i = 0; i < LEN_SUBFR; i++) {
      temp += xn2[i] * xn2[i];
    }

    if (temp < energy) {
      *acelp_params = 0;
      memcpy(&exc[i_subfr], code, LEN_SUBFR * sizeof(FLOAT32));
      memcpy(y1, y2, LEN_SUBFR * sizeof(FLOAT32));
      pitch_gain = gain2;
      tgt_cb_corr[0] = tgt_cb_corr2[0];
      tgt_cb_corr[1] = tgt_cb_corr2[1];
    } else {
      *acelp_params = 1;
      pitch_gain = gain1;
    }
    acelp_params++;

    iusace_acelp_cb_target_update(xn, xn2, y1, pitch_gain);
    iusace_acelp_cb_target_update(cn, cn, &exc[i_subfr], pitch_gain);

    temp = 0.0;
    iusace_apply_preemph(imp_res, TILT_CODE, LEN_SUBFR, &temp);
    if (t0_frac > 2) {
      t0++;
    }

    for (i = t0; i < LEN_SUBFR; i++) {
      imp_res[i] += imp_res[i - t0] * PIT_SHARP;
    }

    iusace_acelp_tgt_ir_corr(xn2, imp_res, dn);

    if (acelp_core_mode == ACELP_CORE_MODE_9k6) {
      iusace_acelp_cb_exc(dn, cn, imp_res, cb_exc, y2, ACELP_NUM_BITS_20, acelp_params,
                          pstr_scratch->p_acelp_ir_buf);
      acelp_params += 4;
    } else if (acelp_core_mode == ACELP_CORE_MODE_11k2) {
      iusace_acelp_cb_exc(dn, cn, imp_res, cb_exc, y2, ACELP_NUM_BITS_28, acelp_params,
                          pstr_scratch->p_acelp_ir_buf);
      acelp_params += 4;
    } else if (acelp_core_mode == ACELP_CORE_MODE_12k8) {
      iusace_acelp_cb_exc(dn, cn, imp_res, cb_exc, y2, ACELP_NUM_BITS_36, acelp_params,
                          pstr_scratch->p_acelp_ir_buf);
      acelp_params += 4;
    } else if (acelp_core_mode == ACELP_CORE_MODE_14k4) {
      iusace_acelp_cb_exc(dn, cn, imp_res, cb_exc, y2, ACELP_NUM_BITS_44, acelp_params,
                          pstr_scratch->p_acelp_ir_buf);
      acelp_params += 4;
    } else if (acelp_core_mode == ACELP_CORE_MODE_16k) {
      iusace_acelp_cb_exc(dn, cn, imp_res, cb_exc, y2, ACELP_NUM_BITS_52, acelp_params,
                          pstr_scratch->p_acelp_ir_buf);
      acelp_params += 4;
    } else if (acelp_core_mode == ACELP_CORE_MODE_18k4) {
      iusace_acelp_cb_exc(dn, cn, imp_res, cb_exc, y2, ACELP_NUM_BITS_64, acelp_params,
                          pstr_scratch->p_acelp_ir_buf);
      acelp_params += 8;
    } else {
      iusace_acelp_cb_exc(dn, cn, imp_res, cb_exc, y2, ACELP_NUM_BITS_64, acelp_params,
                          pstr_scratch->p_acelp_ir_buf);
      acelp_params += 8;
    }

    for (i = 0; i < LEN_SUBFR; i++) {
      code[i] = (FLOAT32)(cb_exc[i] / 512);
    }

    temp = 0.0;
    iusace_apply_preemph(code, TILT_CODE, LEN_SUBFR, &temp);
    for (i = t0; i < LEN_SUBFR; i++) {
      code[i] += code[i - t0] * PIT_SHARP;
    }

    iusace_acelp_tgt_cb_corr1(xn, y1, y2, tgt_cb_corr);
    iusace_acelp_quant_gain(code, &pitch_gain, &code_gain, tgt_cb_corr, mean_ener_code,
                            acelp_params);
    acelp_params++;

    temp = 0.0;
    for (i = 0; i < LEN_SUBFR; i++) {
      temp += code[i] * code[i];
    }
    temp *= code_gain * code_gain;

    for (i = 0; i < LEN_SUBFR; i++) {
      wsynth_out[i + i_subfr] = y0[i] + (pitch_gain * y1[i]) + (code_gain * y2[i]);
    }

    for (i = 0; i < LEN_SUBFR; i++) {
      exc[i + i_subfr] = pitch_gain * exc[i + i_subfr] + code_gain * code[i];
    }

    iusace_synthesis_tool_float(p_quant_lp_filt_coeff, &exc[i_subfr], &synth_out[i_subfr],
                                LEN_SUBFR, &synth_out[i_subfr - ORDER],
                                pstr_scratch->p_buf_synthesis_tool);
    p_lp_filt_coeff += (ORDER + 1);
    p_quant_lp_filt_coeff += (ORDER + 1);
  }

  memcpy(lpd_state->acelp_exc, exc - len_subfrm, 2 * len_subfrm * sizeof(FLOAT32));
  memcpy(lpd_state->synth, synth_out + len_subfrm - (ORDER + 128),
         (ORDER + 128) * sizeof(FLOAT32));
  memcpy(lpd_state->wsynth, wsynth_out + len_subfrm - (1 + 128), (1 + 128) * sizeof(FLOAT32));
  memcpy(lpd_state->lpc_coeffs_quant, p_quant_lp_filt_coeff - (2 * (ORDER + 1)),
         (2 * (ORDER + 1)) * sizeof(FLOAT32));
  memcpy(lpd_state->lpc_coeffs, p_lp_filt_coeff - (2 * (ORDER + 1)),
         (2 * (ORDER + 1)) * sizeof(FLOAT32));

  mem_txn = lpd_state->tcx_mem[128 - 1];
  mem_txnq = lpd_state->tcx_fac;

  p_quant_lp_filt_coeff = quant_lp_filt_coeff;
  for (i_subfr = 0; i_subfr < (len_subfrm - 2 * LEN_SUBFR); i_subfr += LEN_SUBFR) {
    iusace_get_weighted_lpc(p_quant_lp_filt_coeff, weighted_lpc);

    memcpy(error, &speech_in[i_subfr], LEN_SUBFR * sizeof(FLOAT32));
    iusace_apply_deemph(error, TILT_FAC, LEN_SUBFR, &mem_txn);

    memcpy(error, &synth_out[i_subfr], LEN_SUBFR * sizeof(FLOAT32));
    iusace_apply_deemph(error, TILT_FAC, LEN_SUBFR, &mem_txnq);

    p_quant_lp_filt_coeff += (ORDER + 1);
  }

  lpd_state->tcx_quant[0] = mem_txnq;
  for (i_subfr = 0; i_subfr < (2 * LEN_SUBFR); i_subfr += LEN_SUBFR) {
    iusace_get_weighted_lpc(p_quant_lp_filt_coeff, weighted_lpc);

    memcpy(&(lpd_state->tcx_mem[i_subfr]), &speech_in[i_subfr + (len_subfrm - 2 * LEN_SUBFR)],
           LEN_SUBFR * sizeof(FLOAT32));
    iusace_apply_deemph(&(lpd_state->tcx_mem[i_subfr]), TILT_FAC, LEN_SUBFR, &mem_txn);

    memcpy(&(lpd_state->tcx_quant[1 + i_subfr]),
           &synth_out[i_subfr + (len_subfrm - 2 * LEN_SUBFR)], LEN_SUBFR * sizeof(FLOAT32));
    iusace_apply_deemph(&(lpd_state->tcx_quant[1 + i_subfr]), TILT_FAC, LEN_SUBFR, &mem_txnq);
    p_quant_lp_filt_coeff += (ORDER + 1);
  }
  lpd_state->tcx_fac = mem_txnq;

  iusace_get_weighted_lpc(p_quant_lp_filt_coeff, weighted_lpc);

  memcpy(error, &synth_out[len_subfrm - ORDER], ORDER * sizeof(FLOAT32));
  for (i_subfr = (2 * LEN_SUBFR); i_subfr < (4 * LEN_SUBFR); i_subfr += LEN_SUBFR) {
    memset(error + ORDER, 0, LEN_SUBFR * sizeof(FLOAT32));

    iusace_synthesis_tool_float(p_quant_lp_filt_coeff, error + ORDER, error + ORDER, LEN_SUBFR,
                                error, pstr_scratch->p_buf_synthesis_tool);
    memcpy(&(lpd_state->tcx_quant[1 + i_subfr]), error + ORDER, LEN_SUBFR * sizeof(FLOAT32));
    iusace_apply_deemph(&(lpd_state->tcx_quant[1 + i_subfr]), TILT_FAC, LEN_SUBFR, &mem_txnq);
    memcpy(error, error + LEN_SUBFR, ORDER * sizeof(FLOAT32));
  }

  lpd_state->mode = 0;

  lpd_state->num_bits = num_bits;
}
