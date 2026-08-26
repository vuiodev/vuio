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
FLOAT32 ixheaace_mps_212_sum_up_cplx_pow_2(const ixheaace_cmplx_str *const inp, const WORD32 n);

FLOAT32
ixheaace_mps_212_sum_up_cplx_pow_2_dim_2(ixheaace_cmplx_str inp[MAX_ANA_TIME_SLOT][MAX_QMF_BANDS],
                                         const WORD32 start_dim_1, const WORD32 stop_dim_1,
                                         const WORD32 start_dim_2, const WORD32 stop_dim_2);

VOID ixheaace_mps_212_cplx_scalar_product(
    ixheaace_cmplx_str *const out, ixheaace_cmplx_str inp_1[MAX_ANA_TIME_SLOT][MAX_QMF_BANDS],
    ixheaace_cmplx_str inp_2[MAX_ANA_TIME_SLOT][MAX_QMF_BANDS], const WORD32 start_dim_1,
    const WORD32 stop_dim_1, const WORD32 start_dim_2, const WORD32 stop_dim_2);
