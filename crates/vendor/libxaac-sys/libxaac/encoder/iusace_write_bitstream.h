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
WORD32 iusace_write_ics_info(ia_bit_buf_struct *it_bit_buf, ia_sfb_params_struct *pstr_sfb_prms,
                             WORD32 ch);

WORD32 iusace_write_cpe(ia_sfb_params_struct *pstr_sfb_prms, ia_bit_buf_struct *it_bit_buf,
                        WORD32 *tns_data_present, WORD32 const usac_independency_flg,
                        ia_usac_encoder_config_struct *ptr_usac_config,
                        ia_usac_data_struct *ptr_usac_data, WORD32 ch);

WORD32 iusace_write_fd_data(ia_bit_buf_struct *it_bit_buf, ia_sfb_params_struct *pstr_sfb_prms,
                            WORD32 num_fac_bits, WORD32 usac_independency_flg,
                            ia_usac_data_struct *ptr_usac_data,
                            ia_usac_encoder_config_struct *ptr_usac_config, WORD32 ch_idx,
                            WORD32 ele_id, WORD32 idx);

WORD32 iusace_count_fd_bits(ia_sfb_params_struct *pstr_sfb_prms,
                            ia_usac_data_struct *ptr_usac_data, WORD32 usac_independency_flg,
                            ia_usac_encoder_config_struct *ptr_usac_config, WORD32 ch_idx,
                            WORD32 idx);

WORD32 iusace_write_fill_ele(ia_bit_buf_struct *it_bit_buf, WORD32 num_bits);

WORD32 iusace_write_tns_data(ia_bit_buf_struct *it_bit_buf, ia_tns_info *pstr_tns_info,
                             WORD32 window_sequence, WORD32 core_mode);

WORD32 iusace_write_cplx_pred_data(ia_bit_buf_struct *it_bit_buf, WORD32 num_win_grps,
                                   WORD32 num_sfb, WORD32 complex_coef,
                                   WORD32 pred_coeffs_re[MAX_SHORT_WINDOWS][MAX_SFB_LONG],
                                   WORD32 pred_coeffs_im[MAX_SHORT_WINDOWS][MAX_SFB_LONG],
                                   const WORD32 huff_tab[CODE_BOOK_ALPHA_LAV][2],
                                   WORD32 const usac_independency_flg, WORD32 pred_dir,
                                   WORD32 cplx_pred_used[MAX_SHORT_WINDOWS][MAX_SFB_LONG],
                                   WORD32 cplx_pred_all, WORD32 *ptr_prev_alpha_coeff_re,
                                   WORD32 *ptr_prev_alpha_coeff_im, WORD32 *delta_code_time);

WORD32 iusace_write_ms_data(ia_bit_buf_struct *it_bit_buf, WORD32 ms_mask,
                            WORD32 ms_used[MAX_SHORT_WINDOWS][MAX_SFB_LONG], WORD32 num_win_grps,
                            WORD32 nr_of_sfb);

WORD32 iusace_write_scf_data(ia_bit_buf_struct *it_bit_buf, WORD32 max_sfb, WORD32 num_sfb,
                             const WORD32 *scale_factors, WORD32 num_win_grps, WORD32 global_gain,
                             const WORD32 huff_tab[CODE_BOOK_ALPHA_LAV][2]);
