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
#ifndef IXHEAACD_MPS_HYBFILTER_H
#define IXHEAACD_MPS_HYBFILTER_H

VOID ixheaacd_mps_qmf_hybrid_analysis_init(ia_mps_hybrid_filt_struct *handle);

VOID ixheaacd_mps_qmf_hybrid_analysis(
    ia_mps_hybrid_filt_struct *handle,
    ia_cmplx_flt_struct in_qmf[MAX_NUM_QMF_BANDS_MPS_NEW][MAX_TIME_SLOTS],
    WORD32 num_bands, WORD32 num_samples,
    ia_cmplx_flt_struct out_hyb[MAX_HYBRID_BANDS_MPS][MAX_TIME_SLOTS]);

VOID ixheaacd_mps_qmf_hybrid_analysis_no_pre_mix(
    ia_mps_hybrid_filt_struct *handle,
    ia_cmplx_flt_struct in_qmf[MAX_NUM_QMF_BANDS_MPS][MAX_TIME_SLOTS],
    WORD32 num_bands, WORD32 num_samples,
    ia_cmplx_flt_struct v[MAX_TIME_SLOTS][MAX_HYBRID_BANDS_MPS]);

VOID ixheaacd_mps_qmf_hybrid_synthesis(
    ia_cmplx_flt_struct in_hyb[MAX_TIME_SLOTS][MAX_HYBRID_BANDS_MPS],
    WORD32 num_bands, WORD32 num_samples,
    ia_cmplx_flt_struct in_qmf[MAX_TIME_SLOTS][MAX_NUM_QMF_BANDS_MPS]);

WORD32 ixheaacd_get_qmf_sb(
    WORD32 hybrid_subband,
    const ia_mps_dec_mdct2qmf_table_struct *ixheaacd_mps_dec_mdct2qmf_table);

VOID ixheaacd_init_ana_hyb_filt_bank(
    ia_mps_dec_thyb_filter_state_struct *hyb_state);

VOID ixheaacd_apply_ana_hyb_filt_bank_merge_res_decor(
    ia_mps_dec_thyb_filter_state_struct *hyb_state, WORD32 *m_qmf_real,
    WORD32 *m_qmf_imag, WORD32 nr_bands, WORD32 nr_samples,
    WORD32 *m_hybrid_real, WORD32 *m_hybrid_imag,
    const ia_mps_dec_hybrid_tables_struct *hyb_tab_ptr);

VOID ixheaacd_apply_ana_hyb_filt_bank_create_x(
    ia_mps_dec_thyb_filter_state_struct *hyb_state, WORD32 *m_qmf_real,
    WORD32 *m_qmf_imag, WORD32 nr_bands, WORD32 nr_samples,
    WORD32 *m_hybrid_real, WORD32 *m_hybrid_imag,
    const ia_mps_dec_hybrid_tables_struct *hyb_tab_ptr);

VOID ixheaacd_apply_ana_hyb_filt_bank_create_x_res(
    ia_mps_dec_thyb_filter_state_struct *hyb_state, WORD32 *m_qmf_real,
    WORD32 *m_qmf_imag, WORD32 nr_bands, WORD32 nr_samples,
    WORD32 *m_hybrid_real, WORD32 *m_hybrid_imag, SIZE_T *indx, WORD32 res,
    WORD32 hyb_bands, WORD32 num_parameter_bands, WORD32 *counter,
    const ia_mps_dec_hybrid_tables_struct *hyb_tab_ptr);

VOID ixheaacd_8ch_filtering(const WORD32 *p_qmf_real, const WORD32 *p_qmf_imag,
                            WORD32 *m_hybrid_real, WORD32 *m_hybrid_imag,
                            const ia_mps_dec_hybrid_tables_struct *hyb_tab);

VOID ixheaacd_2ch_filtering(WORD32 *p_qmf, WORD32 *m_hybrid,
                            const ia_mps_dec_hybrid_tables_struct *hyb_tab);
#endif /* IXHEAACD_MPS_HYBFILTER_H */
