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
VOID iusace_autocorr_plus(FLOAT32 *speech, FLOAT32 *auto_corr_vector, WORD32 window_len,
                          const FLOAT32 *lp_analysis_win, FLOAT32 *temp_aut_corr);
VOID iusace_compute_lp_residual(FLOAT32 *a, FLOAT32 *x, FLOAT32 *y, WORD32 l);
VOID iusace_convolve(FLOAT32 *signal, FLOAT32 *wsynth_filter_ir, FLOAT32 *conv_out);
VOID iusace_synthesis_tool_float(FLOAT32 *a, FLOAT32 *x, FLOAT32 *y, WORD32 l, FLOAT32 *mem,
                                 FLOAT32 *buf_synth_tool);
VOID iusace_apply_preemph(FLOAT32 *signal, FLOAT32 factor, WORD32 length, FLOAT32 *mem);
VOID iusace_apply_deemph(FLOAT32 *signal, FLOAT32 factor, WORD32 length, FLOAT32 *mem);
VOID iusace_lpc_2_lsp_conversion(FLOAT32 *lpc, FLOAT32 *lsp, FLOAT32 *prev_lsp);
VOID iusace_levinson_durbin_algo(FLOAT32 *auto_corr_input, FLOAT32 *lpc);
VOID iusace_get_weighted_lpc(FLOAT32 *lpc, FLOAT32 *weighted_lpc);
VOID iusace_lsp_2_lsf_conversion(FLOAT32 *lsp, FLOAT32 *lsf);
VOID iusace_lsf_2_lsp_conversion(FLOAT32 *lsf, FLOAT32 *lsp);
VOID iusace_open_loop_search(FLOAT32 *wsp, WORD32 min_pitch_lag, WORD32 max_pitch_lag,
                             WORD32 num_frame, WORD32 *ol_pitch_lag,
                             ia_usac_td_encoder_struct *st);
WORD32 iusace_get_ol_lag_median(WORD32 prev_ol_lag, WORD32 *prev_ol_lags);
VOID iusace_closed_loop_search(FLOAT32 *exc, FLOAT32 *xn, FLOAT32 *wsyn_filt_ir,
                               WORD32 search_range_min, WORD32 search_range_max, WORD32 *pit_frac,
                               WORD32 is_first_subfrm, WORD32 min_pitch_lag_res1_2,
                               WORD32 min_pitch_lag_res_1, WORD32 *pitch_lag_out);
VOID iusace_decim2_fir_filter(FLOAT32 *signal, WORD32 length, FLOAT32 *mem,
                              FLOAT32 *scratch_fir_sig_buf);
FLOAT32 iusace_calc_sq_gain(FLOAT32 *x, WORD32 num_bits, WORD32 length,
                            FLOAT32 *scratch_sq_gain_en);
VOID iusace_lpc_coef_gen(FLOAT32 *lsf_old, FLOAT32 *lsf_new, FLOAT32 *a, WORD32 nb_subfr,
                         WORD32 m);
VOID iusace_interpolation_lsp_params(FLOAT32 *lsp_old, FLOAT32 *lsp_new, FLOAT32 *lp_flt_coff_a,
                                     WORD32 nb_subfr);

VOID iusace_acelp_tgt_ir_corr(FLOAT32 *x, FLOAT32 *ir_wsyn, FLOAT32 *corr_out);

VOID iusace_acelp_tgt_cb_corr1(FLOAT32 *xn, FLOAT32 *y1, FLOAT32 *y2, FLOAT32 *corr_out);

FLOAT32 iusace_acelp_tgt_cb_corr2(FLOAT32 *xn, FLOAT32 *y1, FLOAT32 *corr_out);

VOID iusace_acelp_cb_target_update(FLOAT32 *x, FLOAT32 *x2, FLOAT32 *y, FLOAT32 gain);

VOID iusace_acelp_cb_exc(FLOAT32 *corr_input, FLOAT32 *lp_residual, FLOAT32 *ir_wsyn,
                         WORD16 *alg_cb_exc_out, FLOAT32 *filt_cb_exc, WORD32 num_bits_cb,
                         WORD32 *acelp_param_out, FLOAT32 *scratch_acelp_ir_buf);

VOID iusace_acelp_ltpred_cb_exc(FLOAT32 *exc, WORD32 t0, WORD32 t0_frac, WORD32 len_subfrm);

VOID iusace_acelp_quant_gain(FLOAT32 *code, FLOAT32 *pitch_gain, FLOAT32 *code_gain,
                             FLOAT32 *tgt_cb_corr_data, FLOAT32 mean_energy, WORD32 *qunt_idx);
