/******************************************************************************
 *
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
#ifndef IXHEAACD_MPS_CALC_M1M2_COMMON_H
#define IXHEAACD_MPS_CALC_M1M2_COMMON_H

VOID ixheaacd_param_2_umx_ps(ia_heaac_mps_state_struct *pstr_mps_state, WORD32 *h11, WORD32 *h12,
                             WORD32 *h21, WORD32 *h22, WORD32 *h12_res, WORD32 *h22_res,
                             WORD16 *c_l, WORD16 *c_r, WORD32 ott_box_indx,
                             WORD32 parameter_set_indx, WORD32 res_bands);

VOID ixheaacd_get_matrix_inversion_weights(
    WORD32 iid_lf_ls_idx, WORD32 iid_rf_rs_idx, WORD32 prediction_mode, WORD32 c1, WORD32 c2,
    WORD32 *weight1, WORD32 *weight2, ia_mps_dec_mps_tables_struct *ia_mps_dec_mps_table_ptr);

VOID ixheaacd_invert_matrix(WORD32 weight1, WORD32 weight2, WORD32 h_real[][2],
                            WORD32 h_imag[][2],
                            const ia_mps_dec_common_tables_struct *common_tab_ptr);

VOID ixheaacd_calculate_arb_dmx_mtx(ia_heaac_mps_state_struct *pstr_mps_state, WORD32 ps,
                                    WORD32 pb, WORD32 g_real[]);

VOID ixheaacd_param_2_umx_ps_core_tables(
    WORD32 *cld, WORD32 *icc, WORD32 num_ott_bands, WORD32 res_bands, WORD32 *h11, WORD32 *h12,
    WORD32 *h21, WORD32 *h22, WORD32 *h12_res, WORD32 *h22_res, WORD16 *c_l, WORD16 *c_r,
    const ia_mps_dec_m1_m2_tables_struct *ixheaacd_mps_dec_m1_m2_tables);

VOID ixheaacd_calculate_ttt(ia_heaac_mps_state_struct *pstr_mps_state, WORD32 ps, WORD32 pb,
                            WORD32 ttt_mode, WORD32 m_ttt[][3]);

VOID ixheaacd_calculate_mtx_inv(ia_heaac_mps_state_struct *pstr_mps_state, WORD32 ps, WORD32 pb,
                                WORD32 mode, WORD32 h_real[][2], WORD32 h_imag[][2]);

WORD32 ixheaacd_quantize(WORD32 cld);

#endif /* IXHEAACD_MPS_CALC_M1M2_COMMON_H */
