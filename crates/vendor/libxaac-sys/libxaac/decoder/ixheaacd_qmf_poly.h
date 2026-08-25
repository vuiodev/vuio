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
#ifndef IXHEAACD_QMF_POLY_H
#define IXHEAACD_QMF_POLY_H

double *ixheaacd_interpo_esbr_fcoff(const double *orig_prot, WORD32 no,
                                    WORD32 lo, WORD32 li);

WORD32 ixheaacd_complex_anal_filt(ia_esbr_hbe_txposer_struct *ptr_hbe_txposer);

WORD32 ixheaacd_real_synth_filt(ia_esbr_hbe_txposer_struct *ptr_hbe_txposer,
                                WORD32 num_columns, FLOAT32 qmf_buf_real[][64],
                                FLOAT32 qmf_buf_imag[][64]);

VOID ixheaac_cmplx_anal_fft_p2(FLOAT32 *inp, FLOAT32 *out, WORD32 n_points);

VOID ixheaac_cmplx_anal_fft_p3(FLOAT32 *inp, FLOAT32 *out, WORD32 n_points);

VOID ixheaac_real_synth_fft_p2(FLOAT32 *inp, FLOAT32 *out, WORD32 n_points);

VOID ixheaac_real_synth_fft_p3(FLOAT32 *inp, FLOAT32 *out, WORD32 n_points);

WORD32 ixheaacd_dft_hbe_cplx_anal_filt(ia_esbr_hbe_txposer_struct *ptr_hbe_txposer,
                                       FLOAT32 qmf_buf_real[][64],
                                       FLOAT32 qmf_buf_imag[][64]);

#endif
