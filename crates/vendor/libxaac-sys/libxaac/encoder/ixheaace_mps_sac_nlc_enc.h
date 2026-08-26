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
VOID ixheaace_mps_515_ec_data_pair_enc(ixheaace_bit_buf_handle strm,
                                         WORD32 aa_in_data[][MAXBANDS],
                                         WORD32 a_history[MAXBANDS], WORD32 data_type,
                                         WORD32 set_idx, WORD32 start_band, WORD32 data_bands,
                                         WORD32 pair_flag, WORD32 coarse_flag,
                                         WORD32 independency_flag);
