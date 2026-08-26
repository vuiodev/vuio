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
#include "ixheaace_adjust_threshold_data.h"
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
#include "iusace_psy_utils.h"
#include "iusace_tns_usac.h"
#include "iusace_config.h"
#include "iusace_arith_enc.h"
#include "iusace_fd_qc_util.h"
#include "iusace_fd_quant.h"
#include "iusace_block_switch_const.h"
#include "iusace_block_switch_struct_def.h"
#include "iusace_ms.h"
#include "iusace_signal_classifier.h"
#include "ixheaace_sbr_header.h"
#include "ixheaace_config.h"
#include "ixheaace_asc_write.h"
#include "iusace_main.h"
#include "iusace_lpd_rom.h"
#include "iusace_lpd.h"
#include "iusace_func_prototypes.h"
#include "ixheaac_error_standards.h"

VOID iusace_init_td_data(ia_usac_td_encoder_struct *st, WORD32 len_frame) {
  WORD32 len_window;
  WORD32 num_frames = NUM_FRAMES;
  st->len_subfrm = len_frame / num_frames;

  st->len_frame = len_frame;
  st->num_subfrm = (MAX_NUM_SUBFR * len_frame) / LEN_SUPERFRAME;

  iusace_reset_td_enc(st);
  st->prev_mode = -1;
  st->arith_reset_flag = 1;

  if (st->fscale <= FSCALE_DENOM) {
    len_window = (LEN_LP_WINDOW * len_frame) / LEN_SUPERFRAME;
  } else {
    len_window = (LEN_LP_WINDOW_HIGH_RATE * len_frame) / LEN_SUPERFRAME;
  }

  switch (len_window) {
    case 512:
      st->lp_analysis_window = iusace_cos_window_512;
      break;
    case 448:
      st->lp_analysis_window = iusace_cos_window_448;
      break;
    case 384:
      st->lp_analysis_window = iexheaac_cos_window_384;
      break;
    default:
      st->lp_analysis_window = iusace_cos_window_512;
      break;
  }
  return;
}

VOID iusace_config_acelp_core_mode(ia_usac_td_encoder_struct *st, WORD32 sampling_rate,
                                   WORD32 bitrate) {
  WORD32 max_bits, coder_bits;
  const WORD32 *p_acelp_core_numbits_table;

  p_acelp_core_numbits_table = (WORD32 *)iusace_acelp_core_numbits_1024;

  max_bits = (WORD32)((FLOAT32)(bitrate * LEN_SUPERFRAME) / (FLOAT32)sampling_rate);

  for (st->acelp_core_mode = 5; st->acelp_core_mode >= 0; st->acelp_core_mode--) {
    coder_bits = p_acelp_core_numbits_table[st->acelp_core_mode];
    if (coder_bits <= max_bits) {
      return;
    }
  }
  if (st->acelp_core_mode == -1) {
    st->acelp_core_mode = 0;
  }
  return;
}

VOID iusace_reset_td_enc(ia_usac_td_encoder_struct *st) {
  WORD32 i;

  memset(st->old_speech_pe, 0, (ORDER + LEN_NEXT_HIGH_RATE) * sizeof(FLOAT32));
  memset(st->prev_exc, 0, (MAX_PITCH + LEN_INTERPOL) * sizeof(FLOAT32));
  memset(st->prev_wsp, 0, (MAX_PITCH / OPL_DECIM) * sizeof(FLOAT32));
  memset(st->mem_lp_decim2, 0, 3 * sizeof(FLOAT32));
  memset(st->weighted_sig, 0, 128 * sizeof(FLOAT32));

  st->lpd_state.mode = -1;
  st->lpd_state.num_bits = 0;
  memset(st->lpd_state.lpc_coeffs_quant, 0, (2 * (ORDER + 1)) * sizeof(FLOAT32));
  memset(st->lpd_state.lpc_coeffs, 0, (2 * (ORDER + 1)) * sizeof(FLOAT32));
  memset(st->lpd_state.synth, 0, (ORDER + 128) * sizeof(FLOAT32));
  memset(st->lpd_state.wsynth, 0, (1 + 128) * sizeof(FLOAT32));

  memset(st->lpd_state.acelp_exc, 0, (2 * LEN_FRAME) * sizeof(FLOAT32));

  memset(st->lpd_state.tcx_mem, 0, 128 * sizeof(FLOAT32));
  memset(st->lpd_state.tcx_quant, 0, (1 + 256) * sizeof(FLOAT32));
  st->lpd_state.tcx_fac = 0.0f;

  memset(st->prev_hp_wsp, 0,
         (LEN_SUPERFRAME / OPL_DECIM + (MAX_PITCH / OPL_DECIM)) * sizeof(FLOAT32));

  memset(st->hp_ol_ltp_mem, 0, (3 * 2 + 1) * sizeof(FLOAT32));
  for (i = 0; i < 5; i++) st->prev_ol_lags[i] = 40;
  st->prev_wsyn_mem = 0.0;
  st->prev_wsp_mem = 0.0;
  st->prev_xnq_mem = 0.0;
  st->mem_wsp = 0.0;
  st->prev_ovlp_size = 0;
  st->prev_pitch_med = 40;
  st->ol_wght_flg = 0;
  st->ada_w = 0.0;

  memcpy(st->isf_old, iusace_lsf_init, ORDER * sizeof(FLOAT32));
  memcpy(st->isp_old, iusace_ispold_init, ORDER * sizeof(FLOAT32));
  memcpy(st->isp_old_q, st->isp_old, ORDER * sizeof(FLOAT32));

  st->mem_preemph = 0.0;
  memset(st->mem_sig_in, 0, 4 * sizeof(FLOAT32));
  memset(st->xn_buffer, 0, 128 * sizeof(FLOAT32));

  return;
}

VOID iusace_highpass_prev_wsp(ia_usac_td_encoder_struct *st, FLOAT32 *decim_sig,
                              WORD32 pitch_max) {
  WORD32 i, k;
  WORD32 wsp_offset = pitch_max / OPL_DECIM;
  WORD32 num_frames = (2 * LEN_SUBFR) / OPL_DECIM;
  FLOAT32 *hp_wsp_mem = st->hp_ol_ltp_mem;
  FLOAT32 *prev_hp_wsp = st->prev_hp_wsp;
  for (i = 0; i < 2 * st->len_subfrm / OPL_DECIM; i += num_frames) {
    FLOAT32 *data_a, *data_b, *hp_wsp, o;
    FLOAT32 *wsp = decim_sig + ORDER + i;
    data_a = hp_wsp_mem;
    data_b = hp_wsp_mem + HP_ORDER;
    hp_wsp = prev_hp_wsp + wsp_offset;
    for (k = 0; k < num_frames; k++) {
      data_b[0] = data_b[1];
      data_b[1] = data_b[2];
      data_b[2] = data_b[3];
      data_b[HP_ORDER] = wsp[k];
      o = data_b[0] * 0.83787057505665F;
      o += data_b[1] * -2.50975570071058F;
      o += data_b[2] * 2.50975570071058F;
      o += data_b[3] * -0.83787057505665F;
      o -= data_a[0] * -2.64436711600664F;
      o -= data_a[1] * 2.35087386625360F;
      o -= data_a[2] * -0.70001156927424F;
      data_a[2] = data_a[1];
      data_a[1] = data_a[0];
      data_a[0] = o;
      hp_wsp[k] = o;
    }
    memmove(prev_hp_wsp, &prev_hp_wsp[num_frames], wsp_offset * sizeof(FLOAT32));
  }
}

VOID iusace_find_weighted_speech(FLOAT32 *filter_coef, FLOAT32 *speech, FLOAT32 *wsp,
                                 FLOAT32 *mem_wsp, WORD32 length) {
  WORD32 i_subfr;
  FLOAT32 weighted_lpc[ORDER + 1];
  for (i_subfr = 0; i_subfr < length; i_subfr += LEN_SUBFR) {
    iusace_get_weighted_lpc(filter_coef, weighted_lpc);
    iusace_compute_lp_residual(weighted_lpc, &speech[i_subfr], &wsp[i_subfr], LEN_SUBFR);
    filter_coef += (ORDER + 1);
  }
  iusace_apply_deemph(wsp, TILT_FAC, length, mem_wsp);
  return;
}

VOID iusace_get_interpolated_lpc(FLOAT32 *lsp_old, FLOAT32 *lsp_new, FLOAT32 *lpc,
                                 WORD32 num_subfrm) {
  FLOAT32 lsp[ORDER], *p_lpc, inc, fnew, fold;
  WORD32 i, k;

  inc = 1.0f / (FLOAT32)num_subfrm;
  p_lpc = lpc;
  fnew = 0.0f;

  for (k = 0; k < num_subfrm; k++) {
    fold = 1.0f - fnew;
    for (i = 0; i < ORDER; i++) {
      lsp[i] = (FLOAT32)(lsp_old[i] * fold + lsp_new[i] * fnew);
    }
    fnew += inc;
    iusace_lsp_to_lp_conversion(lsp, p_lpc);
    p_lpc += (ORDER + 1);
  }

  iusace_lsp_to_lp_conversion(lsp_new, p_lpc);
}

VOID iusace_core_lpd_encode(ia_usac_data_struct *usac_data, FLOAT32 *speech, WORD32 *mode,
                            WORD32 *num_tcx_param, WORD32 ch_idx) {
  WORD32 first_lpd_flag = (usac_data->core_mode_prev[ch_idx] == CORE_MODE_FD);
  iusace_scratch_mem *pstr_scratch = &usac_data->str_scratch;
  WORD32 pit_adj = usac_data->td_encoder[ch_idx]->fscale;
  WORD32 *num_fac_bits = &usac_data->num_td_fac_bits[ch_idx];
  WORD16 *serial_fac_out = usac_data->fac_out_stream[ch_idx];
  WORD32 *lpc_params = usac_data->param_buf + (NUM_FRAMES * MAX_NUM_TCX_PRM_PER_DIV);
  ia_usac_td_encoder_struct *st = usac_data->td_encoder[ch_idx];
  WORD32 *acelp_tcx_params = usac_data->param_buf;
  WORD16 *codec_mode = &st->acelp_core_mode;
  const FLOAT32 *lp_analysis_window = st->lp_analysis_window;
  FLOAT32 *lp_filter_coeff = pstr_scratch->p_lp_filter_coeff;
  FLOAT32 *lp_filter_coeff_q = pstr_scratch->p_lp_filter_coeff_q;

  FLOAT32 *ptr_stack_mem = (FLOAT32 *)pstr_scratch->ptr_stack_mem;

  FLOAT32 *auto_corr_vector = ptr_stack_mem;
  ptr_stack_mem += (ORDER + 1);
  memset(auto_corr_vector, 0, sizeof(*auto_corr_vector) * (ORDER + 1));

  FLOAT32 *isp_new = ptr_stack_mem;
  ptr_stack_mem += ORDER;
  memset(isp_new, 0, sizeof(*isp_new) * (ORDER));

  FLOAT32 *isp_curr = ptr_stack_mem;
  ptr_stack_mem += ((NUM_FRAMES + 1) * ORDER);
  memset(isp_curr, 0, sizeof(*isp_curr) * ((NUM_FRAMES + 1) * ORDER));

  FLOAT32 *isp_curr_q = ptr_stack_mem;
  ptr_stack_mem += ((NUM_FRAMES + 1) * ORDER);
  memset(isp_curr_q, 0, sizeof(*isp_curr_q) * ((NUM_FRAMES + 1) * ORDER));

  FLOAT32 *isf_curr = ptr_stack_mem;
  ptr_stack_mem += ((NUM_FRAMES + 1) * ORDER);
  memset(isf_curr, 0, sizeof(*isf_curr) * ((NUM_FRAMES + 1) * ORDER));

  FLOAT32 *isf_curr_q = ptr_stack_mem;
  ptr_stack_mem += ((NUM_FRAMES + 1) * ORDER);
  memset(isf_curr_q, 0, sizeof(*isf_curr_q) * ((NUM_FRAMES + 1) * ORDER));

  FLOAT32 *auto_corr_lp_filter_coeff = ptr_stack_mem;
  ptr_stack_mem += (ORDER + 1);
  memset(auto_corr_lp_filter_coeff, 0, sizeof(*auto_corr_lp_filter_coeff) * (ORDER + 1));

  WORD32 num_indices = 0, num_bits = 0;
  WORD32 *prm_tcx = pstr_scratch->p_prm_tcx;
  FLOAT32 *p_wsp_prev_buf;
  FLOAT32 *wsp_prev_buf = pstr_scratch->p_wsp_prev_buf;
  memset(lp_filter_coeff, 0, ((NUM_SUBFR_SUPERFRAME + 1) * (ORDER + 1)) * sizeof(FLOAT32));
  memset(lp_filter_coeff_q, 0, ((NUM_SUBFR_SUPERFRAME + 1) * (ORDER + 1)) * sizeof(FLOAT32));
  memset(wsp_prev_buf, 0, ((MAX_PITCH1 / OPL_DECIM) + LEN_FRAME) * sizeof(FLOAT32));
  WORD32 i, j, k, i1, i2;
  WORD32 *p_params;
  FLOAT32 energy = 0, max_corr = 0, t0 = 0, *p, *p1;
  WORD32 ol_pitch_lag[2 * NUM_FRAMES] = {0};
  FLOAT32 norm_corr[2 * NUM_FRAMES] = {0};
  WORD32 num_params, pitch_min, pitch_max;

  ia_usac_lpd_state_struct *lpd_state[6], *lpd_state_temp;
  lpd_state_temp = (ia_usac_lpd_state_struct *)ptr_stack_mem;
  ptr_stack_mem +=
      (sizeof(ia_usac_lpd_state_struct) + sizeof(*ptr_stack_mem)) / (sizeof(*ptr_stack_mem));
  memset(lpd_state_temp, 0, sizeof(*lpd_state_temp));
  for (j = 0; j < 6; j++) {
    lpd_state[j] = (ia_usac_lpd_state_struct *)ptr_stack_mem;
    ptr_stack_mem +=
        (sizeof(ia_usac_lpd_state_struct) + sizeof(*ptr_stack_mem)) / (sizeof(*ptr_stack_mem));
    memset(lpd_state[j], 0, sizeof(*lpd_state[0]));
  }

  WORD32 num_bits_acelp = 0, num_bits_tcx = 0;
  WORD32 range_pitch_search = 0;
  WORD32 len_subfrm = 0;
  WORD32 num_subfrm = 0;
  WORD32 num_sbfrm_per_supfrm = 0;
  WORD32 window_len = 0;
  WORD32 len = (MAX_PITCH / OPL_DECIM);
  FLOAT32 mem_wsyn;
  FLOAT32 ssnr_256 = 0.0f, ssnr_512 = 0.0f, ssnr_1024 = 0.0f;
  FLOAT32 tmp_ssnr = 0.0f;

  len_subfrm = st->len_subfrm;
  num_subfrm = st->num_subfrm;
  num_sbfrm_per_supfrm = NUM_FRAMES * num_subfrm;

  if (pit_adj <= FSCALE_DENOM) {
    window_len = (LEN_LP_WINDOW * len_subfrm) / LEN_FRAME;
  } else {
    window_len = (LEN_LP_WINDOW_HIGH_RATE * len_subfrm) / LEN_FRAME;
  }

  memcpy(pstr_scratch->p_wsig_buf, st->weighted_sig, 128 * sizeof(FLOAT32));

  num_bits_acelp = (iusace_acelp_core_numbits_1024[*codec_mode] - NBITS_MODE) >> 2;

  num_bits_tcx = (WORD32)(0.85f * num_bits_acelp - NBITS_LPC);

  if (pit_adj == 0) {
    pitch_min = TMIN;
    pitch_max = TMAX;
  } else {
    i = (((pit_adj * TMIN) + (FSCALE_DENOM / 2)) / FSCALE_DENOM) - TMIN;
    pitch_min = TMIN + i;
    pitch_max = TMAX + (6 * i);
  }

  p_wsp_prev_buf = wsp_prev_buf + MAX_PITCH1 / OPL_DECIM;
  memcpy(wsp_prev_buf, st->prev_wsp, (WORD32)((MAX_PITCH / OPL_DECIM) * sizeof(FLOAT32)));

  if (first_lpd_flag) {
    memcpy(st->isp_old, iusace_ispold_init, ORDER * sizeof(FLOAT32));

    iusace_autocorr_plus(&speech[-(window_len / 2)], auto_corr_vector, window_len,
                         (FLOAT32 *)lp_analysis_window, pstr_scratch->p_buf_aut_corr);

    for (j = 0; j <= ORDER; j++) {
      auto_corr_vector[j] *= (FLOAT32)iusace_lag_window[j];
    }

    iusace_levinson_durbin_algo(auto_corr_vector, auto_corr_lp_filter_coeff);
    iusace_lpc_2_lsp_conversion(auto_corr_lp_filter_coeff, isp_new, st->isp_old);
    memcpy(st->isp_old, isp_new, ORDER * sizeof(FLOAT32));
    iusace_lsp_2_lsf_conversion(isp_new, isf_curr);
    memcpy(st->isf_old, isf_curr, ORDER * sizeof(FLOAT32));
  }

  memcpy(isp_curr, st->isp_old, ORDER * sizeof(FLOAT32));

  for (i = 0; i < NUM_FRAMES; i++) {
    iusace_autocorr_plus(&speech[((i + 1) * len_subfrm) - (window_len / 2)], auto_corr_vector,
                         window_len, (FLOAT32 *)lp_analysis_window, pstr_scratch->p_buf_aut_corr);

    for (j = 0; j <= ORDER; j++) {
      auto_corr_vector[j] *= (FLOAT32)iusace_lag_window[j];
    }

    iusace_levinson_durbin_algo(auto_corr_vector, auto_corr_lp_filter_coeff);
    iusace_lpc_2_lsp_conversion(auto_corr_lp_filter_coeff, isp_new, st->isp_old);
    memcpy(&isp_curr[(i + 1) * ORDER], isp_new, ORDER * sizeof(FLOAT32));
    iusace_interpolation_lsp_params(st->isp_old, isp_new,
                                    &lp_filter_coeff[i * num_subfrm * (ORDER + 1)], num_subfrm);
    iusace_lsp_2_lsf_conversion(&isp_curr[(i + 1) * ORDER], &isf_curr[(i + 1) * ORDER]);
    memcpy(st->isp_old, isp_new, ORDER * sizeof(FLOAT32));
  }

  memcpy(isf_curr, st->isf_old, ORDER * sizeof(FLOAT32));
  memcpy(st->isf_old, &isf_curr[NUM_FRAMES * ORDER], ORDER * sizeof(FLOAT32));

  if (!first_lpd_flag) {
    iusace_lsp_2_lsf_conversion(st->isp_old_q, isf_curr_q);
  }

  iusace_quantize_lpc_avq(&isf_curr[ORDER], &isf_curr_q[ORDER], first_lpd_flag, &lpc_params[0],
                          &num_indices, &num_bits);

  for (i = 0; i < NUM_FRAMES; i++) {
    iusace_lsf_2_lsp_conversion(&isf_curr_q[(i + 1) * ORDER], &isp_curr_q[(i + 1) * ORDER]);
  }

  if (first_lpd_flag) {
    iusace_lsf_2_lsp_conversion(isf_curr_q, isp_curr_q);
    memcpy(st->isp_old_q, isp_curr_q, ORDER * sizeof(FLOAT32));
  }

  *num_fac_bits = 0;
  if (first_lpd_flag) {
    FLOAT32 *temp_speech = pstr_scratch->p_buf_speech;
    FLOAT32 *temp_res = pstr_scratch->p_buf_res;
    FLOAT32 lpc[9 * (ORDER + 1)];
    FLOAT32 hp_mem[4];
    FLOAT32 premph_mem = 0.0f;
    WORD32 fac_length;
    WORD32 num_bits_fac = (WORD32)((FLOAT32)num_bits_tcx / 2.f);
    FLOAT32 *temp_signal = pstr_scratch->p_buf_signal;

    if (st->last_was_short) {
      fac_length = (st->len_frame) / 16;
    } else {
      fac_length = len_subfrm / 2;
    }

    iusace_get_interpolated_lpc(st->isp_old_q, st->isp_old_q, lpc, (2 * len_subfrm) / LEN_SUBFR);

    memset(temp_speech, 0, (ORDER + 2 * len_subfrm) * sizeof(FLOAT32));
    memset(temp_res, 0, (2 * len_subfrm) * sizeof(FLOAT32));

    iusace_fac_apply(&st->fd_orig[1 + ORDER], len_subfrm, fac_length, st->low_pass_line,
                     num_bits_fac, &st->fd_synth[1 + ORDER], lpc, serial_fac_out, num_fac_bits,
                     pstr_scratch);
    memset(hp_mem, 0, 4 * sizeof(FLOAT32));
    iusace_highpass_50hz_12k8(st->fd_orig, 2 * len_subfrm + 1 + ORDER, hp_mem, pit_adj);
    premph_mem = 0.0f;
    iusace_apply_preemph(st->fd_orig, PREEMPH_FILT_FAC, 2 * len_subfrm + 1 + ORDER, &premph_mem);

    memcpy(temp_signal, st->fd_orig + len_subfrm + 1, (len_subfrm + ORDER) * sizeof(FLOAT32));
    premph_mem = temp_signal[0];
    iusace_apply_deemph(temp_signal, PREEMPH_FILT_FAC, len_subfrm + ORDER, &premph_mem);
    memcpy(st->lpd_state.tcx_mem, &temp_signal[len_subfrm + ORDER - 128], 128 * sizeof(FLOAT32));

    premph_mem = 0.0f;
    iusace_apply_preemph(st->fd_synth, PREEMPH_FILT_FAC, 2 * len_subfrm + 1 + ORDER, &premph_mem);
    memcpy(st->lpd_state.synth, st->fd_synth + 2 * len_subfrm - ORDER - 128 + 1 + ORDER,
           (ORDER + 128) * sizeof(FLOAT32));
    memcpy(temp_speech + ORDER, st->fd_synth + 1 + ORDER, 2 * len_subfrm * sizeof(FLOAT32));

    premph_mem = 0.0f;
    iusace_find_weighted_speech(lpc, temp_speech + ORDER, temp_res, &premph_mem, 2 * len_subfrm);
    st->prev_wsyn_mem = premph_mem;
    memcpy(st->lpd_state.wsynth, temp_res + 2 * len_subfrm - ORDER - 128,
           (ORDER + 128) * sizeof(FLOAT32));
    memcpy(temp_speech + ORDER, st->fd_synth + 1 + ORDER, 2 * len_subfrm * sizeof(FLOAT32));
    memset(temp_res, 0, 2 * len_subfrm * sizeof(FLOAT32));
    for (i = 0; i < 2 * len_subfrm; i += LEN_SUBFR) {
      iusace_compute_lp_residual(lpc, &temp_speech[ORDER + i], &temp_res[i], LEN_SUBFR);
    }
    memcpy(st->lpd_state.acelp_exc, temp_res, 2 * len_subfrm * sizeof(FLOAT32));
    premph_mem = 0.0f;
    iusace_find_weighted_speech(lp_filter_coeff, st->fd_orig + 1 + ORDER, temp_speech + ORDER,
                                &(st->mem_wsp), 2 * len_subfrm);
    memcpy(st->weighted_sig, temp_speech + ORDER + 2 * len_subfrm - 128, 128 * sizeof(FLOAT32));
    memcpy(pstr_scratch->p_wsig_buf, st->weighted_sig, 128 * sizeof(FLOAT32));
    for (i = 0; i < 2 * len_subfrm; i += len_subfrm) {
      iusace_decim2_fir_filter(&temp_speech[i + ORDER], len_subfrm, st->mem_lp_decim2,
                               pstr_scratch->p_fir_sig_buf);
      memcpy(temp_speech + ORDER + i / OPL_DECIM, temp_speech + ORDER + i,
             (len_subfrm / OPL_DECIM) * sizeof(FLOAT32));
    }
    memcpy(wsp_prev_buf, temp_speech + ORDER + 2 * len_subfrm / OPL_DECIM - MAX_PITCH / OPL_DECIM,
           (WORD32)((MAX_PITCH / OPL_DECIM) * sizeof(FLOAT32)));
    iusace_highpass_prev_wsp(st, temp_speech, pitch_max);
  }
  memcpy(isp_curr_q, st->isp_old_q, ORDER * sizeof(FLOAT32));
  memcpy(st->isp_old_q, &isp_curr_q[NUM_FRAMES * ORDER], ORDER * sizeof(FLOAT32));

  for (i = 0; i < NUM_FRAMES; i++) {
    iusace_find_weighted_speech(
        &lp_filter_coeff[i * (num_sbfrm_per_supfrm / NUM_FRAMES) * (ORDER + 1)],
        &speech[i * len_subfrm], &pstr_scratch->p_wsig_buf[i * len_subfrm], &(st->mem_wsp),
        len_subfrm);
    memcpy(p_wsp_prev_buf, &pstr_scratch->p_wsig_buf[i * len_subfrm],
           len_subfrm * sizeof(FLOAT32));

    iusace_decim2_fir_filter(p_wsp_prev_buf, len_subfrm, st->mem_lp_decim2,
                             pstr_scratch->p_fir_sig_buf);
    range_pitch_search = 2 * LEN_SUBFR;
    if (num_subfrm < 4) {
      range_pitch_search = 3 * LEN_SUBFR;
    }

    iusace_open_loop_search(p_wsp_prev_buf, (pitch_min / OPL_DECIM) + 1, pitch_max / OPL_DECIM,
                            range_pitch_search / OPL_DECIM, &ol_pitch_lag[i * 2], st);

    if (st->ol_gain > 0.6) {
      st->prev_pitch_med = iusace_get_ol_lag_median(ol_pitch_lag[i * 2], st->prev_ol_lags);
      st->ada_w = 1.0;
    } else {
      st->ada_w = st->ada_w * 0.9f;
    }
    if (st->ada_w < 0.8) {
      st->ol_wght_flg = 0;
    } else {
      st->ol_wght_flg = 1;
    }

    max_corr = 0.0f;
    p = &p_wsp_prev_buf[0];
    p1 = p_wsp_prev_buf - ol_pitch_lag[i * 2];
    for (j = 0; j < range_pitch_search / OPL_DECIM; j++) {
      max_corr += *p++ * *p1++;
    }

    t0 = 0.01f;
    p = p_wsp_prev_buf - ol_pitch_lag[i * 2];
    for (j = 0; j < range_pitch_search / OPL_DECIM; j++, p++) {
      t0 += *p * *p;
    }
    t0 = (FLOAT32)(1.0 / sqrt(t0));
    norm_corr[i * 2] = max_corr * t0;

    energy = 0.01f;
    for (j = 0; j < range_pitch_search / OPL_DECIM; j++) {
      energy += p_wsp_prev_buf[j] * p_wsp_prev_buf[j];
    }
    energy = (FLOAT32)(1.0 / sqrt(energy));
    norm_corr[i * 2] *= energy;

    if (num_subfrm < 4) {
      ol_pitch_lag[(i * 2) + 1] = ol_pitch_lag[i * 2];
      norm_corr[(i * 2) + 1] = norm_corr[i * 2];
    } else {
      iusace_open_loop_search(p_wsp_prev_buf + ((2 * LEN_SUBFR) / OPL_DECIM),
                              (pitch_min / OPL_DECIM) + 1, pitch_max / OPL_DECIM,
                              (2 * LEN_SUBFR) / OPL_DECIM, &ol_pitch_lag[(i * 2) + 1], st);

      if (st->ol_gain > 0.6) {
        st->prev_pitch_med =
            iusace_get_ol_lag_median(ol_pitch_lag[(i * 2) + 1], st->prev_ol_lags);
        st->ada_w = 1.0;
      } else {
        st->ada_w = st->ada_w * 0.9f;
      }
      if (st->ada_w < 0.8) {
        st->ol_wght_flg = 0;
      } else {
        st->ol_wght_flg = 1;
      }
      max_corr = 0.0f;
      p = p_wsp_prev_buf + (2 * LEN_SUBFR) / OPL_DECIM;
      p1 = p_wsp_prev_buf + ((2 * LEN_SUBFR) / OPL_DECIM) - ol_pitch_lag[(i * 2) + 1];
      for (j = 0; j < (2 * LEN_SUBFR) / OPL_DECIM; j++) {
        max_corr += *p++ * *p1++;
      }

      t0 = 0.01f;
      p = p_wsp_prev_buf + ((2 * LEN_SUBFR) / OPL_DECIM) - ol_pitch_lag[(i * 2) + 1];
      for (j = 0; j < (2 * LEN_SUBFR) / OPL_DECIM; j++, p++) {
        t0 += *p * *p;
      }
      t0 = (FLOAT32)(1.0 / sqrt(t0));
      norm_corr[(i * 2) + 1] = max_corr * t0;

      energy = 0.01f;
      for (j = 0; j < (2 * LEN_SUBFR) / OPL_DECIM; j++) {
        energy += p_wsp_prev_buf[((2 * LEN_SUBFR) / OPL_DECIM) + j] *
                  p_wsp_prev_buf[((2 * LEN_SUBFR) / OPL_DECIM) + j];
      }
      energy = (FLOAT32)(1.0 / sqrt(energy));
      norm_corr[(i * 2) + 1] *= energy;
    }

    memmove(wsp_prev_buf, &wsp_prev_buf[len_subfrm / OPL_DECIM],
            (WORD32)((MAX_PITCH / OPL_DECIM) * sizeof(FLOAT32)));
  }

  memcpy(lpd_state[0], &st->lpd_state, sizeof(*lpd_state[0]));

  ssnr_1024 = 0;
  if (usac_data->use_acelp_only) {
    for (i1 = 0; i1 < 2; i1++) {
      for (i2 = 0; i2 < 2; i2++) {
        k = (i1 * 2) + i2;
        p_params = acelp_tcx_params + (k * MAX_NUM_TCX_PRM_PER_DIV);

        iusace_interpolation_lsp_params(&isp_curr_q[k * ORDER], &isp_curr_q[(k + 1) * ORDER],
          lp_filter_coeff_q, st->num_subfrm);

        memcpy(lpd_state[k + 1], lpd_state[k], sizeof(*lpd_state[0]));

        iusace_acelp_encode(
          &lp_filter_coeff[k * (num_sbfrm_per_supfrm / 4) * (ORDER + 1)], lp_filter_coeff_q,
          &speech[k * st->len_subfrm], &pstr_scratch->p_wsig_buf[k * st->len_subfrm],
          &pstr_scratch->p_synth_buf[k * st->len_subfrm],
          &pstr_scratch->p_wsyn_buf[k * st->len_subfrm], *codec_mode, lpd_state[k + 1],
          st->len_subfrm, norm_corr[k * 2], norm_corr[(k * 2) + 1], ol_pitch_lag[k * 2],
          ol_pitch_lag[(k * 2) + 1], pit_adj, p_params, pstr_scratch);

        mem_wsyn = lpd_state[k]->mem_wsyn;

        iusace_find_weighted_speech(
            &lp_filter_coeff[k * (num_sbfrm_per_supfrm / 4) * (ORDER + 1)],
            &pstr_scratch->p_synth_buf[k * st->len_subfrm], pstr_scratch->p_temp_wsyn_buf,
            &mem_wsyn, st->len_subfrm);

        lpd_state[k + 1]->mem_wsyn = mem_wsyn;
        mode[k] = 0;
        num_tcx_param[k] = 0;
      }
    }
    memcpy(&st->lpd_state, lpd_state[4], sizeof(*lpd_state[4]));
    memcpy(st->weighted_sig, pstr_scratch->p_wsig_buf + (st->len_frame), 128 * sizeof(FLOAT32));
    memcpy(st->prev_wsp, wsp_prev_buf, (len * sizeof(FLOAT32)));
    return;
  }
  for (i1 = 0; i1 < 2; i1++) {
    ssnr_512 = 0;
    for (i2 = 0; i2 < 2; i2++) {
      k = (i1 * 2) + i2;
      p_params = acelp_tcx_params + (k * MAX_NUM_TCX_PRM_PER_DIV);

      iusace_interpolation_lsp_params(&isp_curr_q[k * ORDER], &isp_curr_q[(k + 1) * ORDER],
                                      lp_filter_coeff_q, st->num_subfrm);

      memcpy(lpd_state[k + 1], lpd_state[k], sizeof(*lpd_state[0]));

      iusace_acelp_encode(
          &lp_filter_coeff[k * (num_sbfrm_per_supfrm / 4) * (ORDER + 1)], lp_filter_coeff_q,
          &speech[k * st->len_subfrm], &pstr_scratch->p_wsig_buf[k * st->len_subfrm],
          &pstr_scratch->p_synth_buf[k * st->len_subfrm],
          &pstr_scratch->p_wsyn_buf[k * st->len_subfrm], *codec_mode, lpd_state[k + 1],
          st->len_subfrm, norm_corr[k * 2], norm_corr[(k * 2) + 1], ol_pitch_lag[k * 2],
          ol_pitch_lag[(k * 2) + 1], pit_adj, p_params, pstr_scratch);

      mem_wsyn = lpd_state[k]->mem_wsyn;

      iusace_find_weighted_speech(&lp_filter_coeff[k * (num_sbfrm_per_supfrm / 4) * (ORDER + 1)],
                                  &pstr_scratch->p_synth_buf[k * st->len_subfrm],
                                  pstr_scratch->p_temp_wsyn_buf, &mem_wsyn, st->len_subfrm);

      lpd_state[k + 1]->mem_wsyn = mem_wsyn;

      ssnr_256 = iusace_cal_segsnr(&pstr_scratch->p_wsig_buf[k * st->len_subfrm],
                                   pstr_scratch->p_temp_wsyn_buf, st->len_subfrm, LEN_SUBFR);

      mode[k] = 0;
      num_tcx_param[k] = 0;

      iusace_lpc_coef_gen(&isp_curr_q[k * ORDER], &isp_curr_q[(k + 1) * ORDER], lp_filter_coeff_q,
                          st->num_subfrm, ORDER);

      memcpy(lpd_state_temp, lpd_state[k], sizeof(*lpd_state[0]));

      iusace_tcx_fac_encode(usac_data,
                            &lp_filter_coeff[k * (num_sbfrm_per_supfrm / 4) * (ORDER + 1)],
                            lp_filter_coeff_q, &speech[k * st->len_subfrm], st->len_subfrm,
                            num_bits_tcx, lpd_state_temp, prm_tcx, &num_params, ch_idx, k);

      mem_wsyn = lpd_state[k]->mem_wsyn;

      iusace_find_weighted_speech(&lp_filter_coeff[k * (num_sbfrm_per_supfrm / 4) * (ORDER + 1)],
                                  pstr_scratch->p_synth_tcx_buf, pstr_scratch->p_temp_wsyn_buf,
                                  &mem_wsyn, st->len_subfrm);

      lpd_state_temp->mem_wsyn = mem_wsyn;

      tmp_ssnr = iusace_cal_segsnr(&pstr_scratch->p_wsig_buf[k * st->len_subfrm],
                                   pstr_scratch->p_temp_wsyn_buf, st->len_subfrm, LEN_SUBFR);

      if (tmp_ssnr > ssnr_256) {
        ssnr_256 = tmp_ssnr;
        mode[k] = 1;
        num_tcx_param[k] = num_params;
        memcpy(lpd_state[k + 1], lpd_state_temp, sizeof(*lpd_state[0]));

        memcpy(&pstr_scratch->p_synth_buf[(k * st->len_subfrm) - 128],
               pstr_scratch->p_synth_tcx_buf - 128, (st->len_subfrm + 128) * sizeof(FLOAT32));

        memcpy(&pstr_scratch->p_wsyn_buf[(k * st->len_subfrm) - 128],
               pstr_scratch->p_wsyn_tcx_buf - 128, (st->len_subfrm + 128) * sizeof(FLOAT32));

        memcpy(p_params, prm_tcx, NUM_TCX20_PRM * sizeof(WORD32));
      }
      ssnr_512 += 0.50f * ssnr_256;
    }

    k = i1 * 2;

    p_params = acelp_tcx_params + (k * MAX_NUM_TCX_PRM_PER_DIV);

    iusace_lpc_coef_gen(&isp_curr_q[2 * i1 * ORDER], &isp_curr_q[(2 * i1 + 2) * ORDER],
                        lp_filter_coeff_q, (num_sbfrm_per_supfrm / 2), ORDER);

    memcpy(lpd_state_temp, lpd_state[2 * i1], sizeof(*lpd_state[0]));
    iusace_tcx_fac_encode(usac_data,
                          &lp_filter_coeff[2 * i1 * (num_sbfrm_per_supfrm / 4) * (ORDER + 1)],
                          lp_filter_coeff_q, &speech[2 * i1 * st->len_subfrm], 2 * st->len_subfrm,
                          2 * num_bits_tcx, lpd_state_temp, prm_tcx, &num_params, ch_idx, 2 * i1);

    mem_wsyn = lpd_state[2 * i1]->mem_wsyn;

    iusace_find_weighted_speech(
        &lp_filter_coeff[2 * i1 * (num_sbfrm_per_supfrm / 4) * (ORDER + 1)],
        pstr_scratch->p_synth_tcx_buf, pstr_scratch->p_temp_wsyn_buf, &mem_wsyn,
        st->len_subfrm * 2);

    lpd_state_temp->mem_wsyn = mem_wsyn;

    tmp_ssnr = iusace_cal_segsnr(&pstr_scratch->p_wsig_buf[2 * i1 * st->len_subfrm],
                                 pstr_scratch->p_temp_wsyn_buf, st->len_subfrm * 2, LEN_SUBFR);

    if (tmp_ssnr > ssnr_512) {
      ssnr_512 = tmp_ssnr;
      for (i = 0; i < 2; i++) {
        mode[k + i] = 2;
        num_tcx_param[k + i] = num_params;
      }
      memcpy(lpd_state[k + 2], lpd_state_temp, sizeof(*lpd_state[0]));

      memcpy(&pstr_scratch->p_synth_buf[(2 * i1 * st->len_subfrm) - 128],
             pstr_scratch->p_synth_tcx_buf - 128, ((2 * st->len_subfrm) + 128) * sizeof(FLOAT32));
      memcpy(&pstr_scratch->p_wsyn_buf[(2 * i1 * st->len_subfrm) - 128],
             pstr_scratch->p_wsyn_tcx_buf - 128, ((2 * st->len_subfrm) + 128) * sizeof(FLOAT32));
      memcpy(p_params, prm_tcx, NUM_TCX40_PRM * sizeof(WORD32));
    }
    ssnr_1024 += 0.50f * ssnr_512;
  }

  k = 0;

  p_params = acelp_tcx_params + (k * MAX_NUM_TCX_PRM_PER_DIV);

  iusace_lpc_coef_gen(&isp_curr_q[k * ORDER], &isp_curr_q[(k + 4) * ORDER], lp_filter_coeff_q,
                      num_sbfrm_per_supfrm, ORDER);

  memcpy(lpd_state_temp, lpd_state[k], sizeof(*lpd_state[0]));

  iusace_tcx_fac_encode(usac_data, &lp_filter_coeff[k * (num_sbfrm_per_supfrm / 4) * (ORDER + 1)],
                        lp_filter_coeff_q, &speech[k * st->len_subfrm], 4 * st->len_subfrm,
                        4 * num_bits_tcx, lpd_state_temp, prm_tcx, &num_params, ch_idx, k);

  mem_wsyn = lpd_state[k]->mem_wsyn;

  iusace_find_weighted_speech(&lp_filter_coeff[k * (num_sbfrm_per_supfrm / 4) * (ORDER + 1)],
                              pstr_scratch->p_synth_tcx_buf, pstr_scratch->p_temp_wsyn_buf,
                              &mem_wsyn, st->len_subfrm * 4);

  lpd_state_temp->mem_wsyn = mem_wsyn;

  tmp_ssnr = iusace_cal_segsnr(&pstr_scratch->p_wsig_buf[k * st->len_subfrm],
                               pstr_scratch->p_temp_wsyn_buf, st->len_subfrm * 4, LEN_SUBFR);

  if (tmp_ssnr > ssnr_1024) {
    for (i = 0; i < 4; i++) {
      mode[k + i] = 3;
      num_tcx_param[k + i] = num_params;
    }
    memcpy(lpd_state[k + 4], lpd_state_temp, sizeof(*lpd_state[0]));

    memcpy(&pstr_scratch->p_synth_buf[(k * st->len_subfrm) - 128],
           pstr_scratch->p_synth_tcx_buf - 128, ((4 * st->len_subfrm) + 128) * sizeof(FLOAT32));
    memcpy(&pstr_scratch->p_wsyn_buf[(k * st->len_subfrm) - 128],
           pstr_scratch->p_wsyn_tcx_buf - 128, ((4 * st->len_subfrm) + 128) * sizeof(FLOAT32));
    memcpy(p_params, prm_tcx, NUM_TCX80_PRM * sizeof(WORD32));
  }
  memcpy(&st->lpd_state, lpd_state[4], sizeof(*lpd_state[4]));

  memcpy(st->weighted_sig, pstr_scratch->p_wsig_buf + (st->len_frame), 128 * sizeof(FLOAT32));
  memcpy(st->prev_wsp, wsp_prev_buf, (len * sizeof(FLOAT32)));

  return;
}

IA_ERRORCODE iusace_lpd_frm_enc(ia_usac_data_struct *usac_data, WORD32 *mod_out,
                                WORD32 const usac_independency_flg, WORD32 len_frame, WORD32 i_ch,
                                ia_bit_buf_struct *pstr_it_bit_buff) {
  WORD32 i;
  WORD32 len_next_high_rate = (LEN_NEXT_HIGH_RATE * len_frame) / LEN_SUPERFRAME;
  WORD32 len_lpc0 = (LEN_LPC0 * len_frame) / LEN_SUPERFRAME;
  FLOAT32 *input_data = &usac_data->td_in_buf[i_ch][len_next_high_rate];
  ia_usac_td_encoder_struct *td_encoder = usac_data->td_encoder[i_ch];
  WORD32 fscale = usac_data->td_encoder[i_ch]->fscale;
  WORD32 first_lpd_flag = (usac_data->core_mode_prev[i_ch] == CORE_MODE_FD);
  FLOAT32 *speech_buf = usac_data->speech_buf;
  FLOAT32 *ptr_scratch_buf = usac_data->str_scratch.p_lpd_frm_enc_scratch;
  FLOAT32 *speech, *new_speech;
  WORD32 mode_buf[1 + NUM_FRAMES] = {0}, *mode;
  WORD32 num_tcx_params[NUM_FRAMES] = {0};
  WORD32 len_subfrm;

  len_subfrm = td_encoder->len_subfrm;

  if (usac_data->core_mode_prev[i_ch] == CORE_MODE_FD) {
    iusace_reset_td_enc(usac_data->td_encoder[i_ch]);

    FLOAT32 *in_data = usac_data->td_in_prev_buf[i_ch];
    FLOAT32 *ptr_speech = usac_data->speech_buf;
    WORD32 length = len_next_high_rate + len_lpc0;
    ia_usac_td_encoder_struct *st = usac_data->td_encoder[i_ch];
    memcpy(ptr_speech, in_data, length * sizeof(FLOAT32));

    iusace_highpass_50hz_12k8(ptr_speech, length, st->mem_sig_in, st->fscale);

    iusace_apply_preemph(ptr_speech, PREEMPH_FILT_FAC, length, &(st->mem_preemph));
    memcpy(st->old_speech_pe + ORDER, ptr_speech, length * sizeof(FLOAT32));
  }

  if (first_lpd_flag) {
    td_encoder->prev_mode = -1;
  }

  mode = mode_buf + 1;
  mode[-1] = td_encoder->prev_mode;
  fscale = (fscale * len_subfrm) / LEN_FRAME;

  new_speech = speech_buf + ORDER + (LEN_NEXT_HIGH_RATE * len_subfrm) / LEN_FRAME;
  speech = speech_buf + ORDER;
  if (first_lpd_flag) {
    new_speech += (LEN_LPC0 * len_subfrm) / LEN_FRAME;
    speech += (LEN_LPC0 * len_subfrm) / LEN_FRAME;
  }
  memcpy(new_speech, input_data, td_encoder->len_frame * sizeof(FLOAT32));

  iusace_highpass_50hz_12k8(new_speech, td_encoder->len_frame, td_encoder->mem_sig_in, fscale);
  iusace_apply_preemph(new_speech, PREEMPH_FILT_FAC, td_encoder->len_frame,
                       &(td_encoder->mem_preemph));

  if (first_lpd_flag) {
    memcpy(speech_buf, td_encoder->old_speech_pe,
           ((ORDER + (((LEN_NEXT_HIGH_RATE + LEN_LPC0) * len_subfrm) / LEN_FRAME))) *
               sizeof(FLOAT32));
    for (i = 0; i < (len_subfrm + 1); i++) {
      ptr_scratch_buf[i] = speech[-len_subfrm - 1 + i];
    }
    iusace_apply_deemph(ptr_scratch_buf, PREEMPH_FILT_FAC, len_subfrm + 1, &ptr_scratch_buf[0]);
    memcpy(td_encoder->lpd_state.tcx_mem, &ptr_scratch_buf[len_subfrm - 128 + 1],
           128 * sizeof(FLOAT32));
  } else {
    memcpy(speech_buf, td_encoder->old_speech_pe,
           ((ORDER + ((LEN_NEXT_HIGH_RATE * len_subfrm) / LEN_FRAME))) * sizeof(FLOAT32));
  }

  iusace_core_lpd_encode(usac_data, speech, mode, num_tcx_params, i_ch);

  if (first_lpd_flag) {
    memcpy(td_encoder->old_speech_pe,
           &speech_buf[(td_encoder->len_frame) + (LEN_LPC0 * len_subfrm) / LEN_FRAME],
           (ORDER + ((LEN_NEXT_HIGH_RATE * len_subfrm) / LEN_FRAME)) * sizeof(FLOAT32));
  } else {
    memcpy(td_encoder->old_speech_pe, &speech_buf[(td_encoder->len_frame)],
           (ORDER + ((LEN_NEXT_HIGH_RATE * len_subfrm) / LEN_FRAME)) * sizeof(FLOAT32));
  }

  iusace_encode_fac_params(mode, num_tcx_params, usac_data, usac_independency_flg,
                           pstr_it_bit_buff, i_ch);

  td_encoder->prev_mode = (WORD16)mode[3];

  memcpy(mod_out, mode, 4 * sizeof(WORD32));
  return IA_NO_ERROR;
}
