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
#ifndef IXHEAACD_FUNC_DEF_H
#define IXHEAACD_FUNC_DEF_H

#define restrict

WORD32 ixheaacd_calc_idx_offset(WORD32 qn1, WORD32 qn2);

VOID ixheaacd_qn_data(WORD32 nk_mode, WORD32 *qn,
                      ia_bit_buf_struct *it_bit_buff);

VOID ixheaacd_memset(FLOAT32 *x, WORD32 n);

VOID ixheaacd_mem_cpy(const FLOAT32 x[], FLOAT32 y[], WORD32 n);

VOID ixheaacd_vec_add(FLOAT32 x[], FLOAT32 y[], FLOAT32 z[], WORD32 n);

VOID ixheaacd_vec_cnst_mul(FLOAT32 a, FLOAT32 x[], FLOAT32 z[], WORD32 n);

WORD32 ixheaacd_lpd_dec(ia_usac_data_struct *usac_data,
                        ia_usac_lpd_decoder_handle st,
                        ia_td_frame_data_struct *pstr_td_frame_data,
                        FLOAT32 fsynth[], WORD32 first_lpd_flag,
                        WORD32 short_fac_flag, WORD32 bpf_control_info);

VOID ixheaacd_lpd_dec_update(ia_usac_lpd_decoder_handle tddec,
                             ia_usac_data_struct *usac_data, WORD32 i_ch);

VOID ixheaacd_acelp_update(ia_usac_data_struct *usac_data, FLOAT32 signal_out[],
                           ia_usac_lpd_decoder_handle st);

VOID ixheaacd_init_acelp_data(ia_usac_data_struct *usac_data,
                              ia_usac_lpd_decoder_handle st);

VOID ixheaacd_reset_acelp_data(ia_usac_data_struct *usac_data,
                               ia_usac_lpd_decoder_handle st,
                               FLOAT32 *ptr_ola_buffer, WORD32 last_was_short,
                               WORD32 tw_mdct);

VOID ixheaacd_acelp_alias_cnx(ia_usac_data_struct *usac_data,
                                ia_td_frame_data_struct *pstr_td_frame_data,
                                WORD32 k, FLOAT32 A[], FLOAT32 stab_fac,
                                ia_usac_lpd_decoder_handle st);

WORD32 ixheaacd_tcx_mdct(ia_usac_data_struct *usac_data,
                         ia_td_frame_data_struct *pstr_td_frame_data,
                         WORD32 frame_index, FLOAT32 A[], WORD32 long_frame,
                         ia_usac_lpd_decoder_handle st);

VOID ixheaacd_alg_vec_dequant(ia_td_frame_data_struct *pstr_td_frame_data, WORD32 first_lpd_flag,
                              FLOAT32 *lsf, WORD32 mod[], WORD32 ec_flag);

VOID ixheaacd_fac_decoding(WORD32 fac_len, WORD32 k, WORD32 *fac_prm,
                           ia_bit_buf_struct *it_bit_buff);

VOID ixheaacd_lpc_to_td(FLOAT32 *lpc_coeffs, WORD32 lpc_order,
                          FLOAT32 *mdct_gains, WORD32 lg);

VOID ixheaacd_noise_shaping(FLOAT32 x[], WORD32 lg, WORD32 fdns_npts,
                            FLOAT32 old_gains[], FLOAT32 new_gains[]);
VOID ixheaacd_interpolation_lsp_params(FLOAT32 lsp_old[], FLOAT32 lsp_new[],
                                       FLOAT32 lp_flt_coff_a[],
                                       WORD32 nb_subfr);

VOID ixheaacd_lpc_coef_gen(FLOAT32 lsf_old[], FLOAT32 lsf_new[], FLOAT32 a[],
                           WORD32 nb_subfr, WORD32 m);

VOID ixheaacd_tw_create(VOID);

WORD32 ixheaacd_reset_acelp_tw_data(ia_usac_data_struct *usac_data, WORD32 i,
                                    ia_usac_lpd_decoder_handle st);

WORD32 ixheaacd_tw_frame_process(ia_usac_data_struct *usac_data, WORD32 ch);

VOID ixheaacd_huffman_decode(WORD32 it_bit_buff, WORD16 *h_index, WORD16 *len,
                             const UWORD16 *input_table,
                             const UWORD32 *idx_table);

#endif
