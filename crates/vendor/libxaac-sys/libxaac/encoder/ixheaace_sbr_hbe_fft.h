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

VOID ixheaac_cmplx_anal_fft_p2(FLOAT32 *ptr_x, FLOAT32 *ptr_y, WORD32 npoints);

VOID ixheaac_cmplx_anal_fft_p3(FLOAT32 *ptr_x_in, FLOAT32 *ptr_x_out, WORD32 npoints);

VOID ixheaac_real_synth_fft_p2(FLOAT32 *ptr_x, FLOAT32 *ptr_y, WORD32 npoints);

VOID ixheaac_real_synth_fft_p3(FLOAT32 *ptr_x_in, FLOAT32 *ptr_x_out, WORD32 npoints);

IA_ERRORCODE ixheaace_complex_anal_filt(ixheaace_str_esbr_hbe_txposer *ptr_hbe_txposer);

IA_ERRORCODE ixheaace_real_synth_filt(ixheaace_str_esbr_hbe_txposer *ptr_hbe_txposer,
                                      WORD32 num_columns, FLOAT32 qmf_buf_real[][64],
                                      FLOAT32 qmf_buf_imag[][64]);

VOID ixheaace_dft_hbe_cplx_anal_filt(ixheaace_str_esbr_hbe_txposer *ptr_hbe_txposer,
                                     FLOAT32 qmf_buf_real[][64], FLOAT32 qmf_buf_imag[][64]);

VOID ixheaace_hbe_apply_cfftn(FLOAT32 re[], FLOAT32 *ptr_scratch, WORD32 n_pass, WORD32 i_sign);

VOID ixheaace_hbe_apply_cfftn_gen(FLOAT32 in[], FLOAT32 *ptr_scratch, WORD32 n_pass,
                                  WORD32 i_sign);

VOID ixheaace_hbe_apply_fft_288(FLOAT32 *ptr_inp, FLOAT32 *ptr_scratch, WORD32 len,
                                WORD32 i_sign);

VOID ixheaace_hbe_apply_ifft_224(FLOAT32 *ptr_inp, FLOAT32 *ptr_scratch, WORD32 len,
                                 WORD32 i_sign);

VOID ixheaace_hbe_apply_fft_3(FLOAT32 *ptr_inp, FLOAT32 *ptr_op, WORD32 i_sign);

VOID ixheaace_hbe_apply_ifft_7(FLOAT32 *ptr_inp, FLOAT32 *ptr_op);

VOID ixheaace_hbe_apply_ifft_336(FLOAT32 *ptr_inp, FLOAT32 *ptr_scratch, WORD32 len,
                                 WORD32 i_sign);
