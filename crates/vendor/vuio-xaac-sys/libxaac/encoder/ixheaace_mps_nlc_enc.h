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
IA_ERRORCODE ixheaace_mps_212_ec_data_pair_enc(
    ixheaace_bit_buf_handle ptr_bit_buf, WORD16 aa_in_data[][IXHEAACE_MPS_SAC_MAX_FREQ_BANDS],
    WORD16 a_history[IXHEAACE_MPS_SAC_MAX_FREQ_BANDS], const WORD32 data_type,
    const WORD32 set_idx, const WORD32 start_band, const WORD16 data_bands,
    const WORD32 coarse_flag, const WORD32 independency_flag);

IA_ERRORCODE ixheaace_mps_212_ec_data_single_enc(
    ixheaace_bit_buf_handle ptr_bit_buf, WORD16 aa_in_data[][IXHEAACE_MPS_SAC_MAX_FREQ_BANDS],
    WORD16 a_history[IXHEAACE_MPS_SAC_MAX_FREQ_BANDS], const WORD32 data_type,
    const WORD32 set_idx, const WORD32 start_band, const WORD16 data_bands,
    const WORD32 coarse_flag, const WORD32 independency_flag);
