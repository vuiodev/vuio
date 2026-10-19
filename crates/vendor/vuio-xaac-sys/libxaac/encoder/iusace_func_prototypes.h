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
VOID iusace_write_bits2buf(WORD32 value, WORD32 no_of_bits, WORD16 *bitstream);

WORD32 iusace_get_num_params(WORD32 *qn);

VOID iusace_highpass_50hz_12k8(FLOAT32 *signal, WORD32 lg, FLOAT32 *mem, WORD32 fscale);

IA_ERRORCODE iusace_lpd_frm_enc(ia_usac_data_struct *usac_data, WORD32 *mod_out,
                                WORD32 const usac_independency_flg, WORD32 len_frame, WORD32 i_ch,
                                ia_bit_buf_struct *pstr_it_bit_buff);

VOID iusace_init_td_data(ia_usac_td_encoder_struct *st, WORD32 len_frame);

VOID iusace_config_acelp_core_mode(ia_usac_td_encoder_struct *st, WORD32 sampling_rate,
                                   WORD32 bitrate);

VOID iusace_reset_td_enc(ia_usac_td_encoder_struct *st);

VOID iusace_core_lpd_encode(ia_usac_data_struct *usac_data, FLOAT32 *speech, WORD32 *mode,
                            WORD32 *num_tcx_param, WORD32 ch_idx);

VOID iusace_encode_fac_params(WORD32 *mod, WORD32 *n_param_tcx, ia_usac_data_struct *usac_data,
                              WORD32 const usac_independency_flag,
                              ia_bit_buf_struct *pstr_it_bit_buff, WORD32 ch_idx);

VOID iusace_acelp_encode(FLOAT32 *lp_filt_coeff, FLOAT32 *quant_lp_filt_coeff, FLOAT32 *speech_in,
                         FLOAT32 *wsig_in, FLOAT32 *synth_out, FLOAT32 *wsynth_out,
                         WORD16 acelp_core_mode, ia_usac_lpd_state_struct *lpd_state,
                         WORD32 len_subfrm, FLOAT32 norm_corr, FLOAT32 norm_corr2,
                         WORD32 ol_pitch_lag1, WORD32 ol_pitch_lag2, WORD32 pit_adj,
                         WORD32 *acelp_params, iusace_scratch_mem *pstr_scratch);

VOID iusace_tcx_fac_encode(ia_usac_data_struct *usac_data, FLOAT32 *lpc_coeffs,
                           FLOAT32 *lpc_coeffs_quant, FLOAT32 *speech, WORD32 frame_len,
                           WORD32 num_bits_per_supfrm, ia_usac_lpd_state_struct *lpd_state,
                           WORD32 *params, WORD32 *n_param, WORD32 ch_idx, WORD32 k_idx);

VOID iusace_fac_apply(FLOAT32 *orig, WORD32 len_subfrm, WORD32 fac_len, WORD32 low_pass_line,
                      WORD32 target_br, FLOAT32 *synth, FLOAT32 *ptr_lpc_coeffs,
                      WORD16 *fac_bits_word, WORD32 *num_fac_bits,
                      iusace_scratch_mem *pstr_scratch);

VOID iusace_quantize_lpc_avq(FLOAT32 *ptr_lsf, FLOAT32 *ptr_lsfq, WORD32 lpc0,
                             WORD32 *ptr_lpc_idx, WORD32 *nb_indices, WORD32 *nbbits);

VOID iusace_lsp_2_lsf_conversion(FLOAT32 *lsp, FLOAT32 *lsf);
VOID iusace_lsp_to_lp_conversion(FLOAT32 *lsp, FLOAT32 *lp_flt_coff_a);

VOID iusace_find_weighted_speech(FLOAT32 *filter_coef, FLOAT32 *speech, FLOAT32 *wsp,
                                 FLOAT32 *mem_wsp, WORD32 length);

IA_ERRORCODE iusace_fd_fac(WORD32 *sfb_offsets, WORD32 sfb_active, FLOAT64 *orig_sig_dbl,
                           WORD32 window_sequence, FLOAT64 *synth_time,
                           ia_usac_td_encoder_struct *pstr_acelp, WORD32 last_subfr_was_acelp,
                           WORD32 next_frm_lpd, WORD16 *fac_prm_out, WORD32 *num_fac_bits,
                           iusace_scratch_mem *pstr_scratch);

FLOAT32 iusace_cal_segsnr(FLOAT32 *sig1, FLOAT32 *sig2, WORD16 len, WORD16 nseg);

WORD32 iusace_fd_encode_fac(WORD32 *prm, WORD16 *ptr_bit_buf, WORD32 fac_length);
