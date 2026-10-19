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
VOID ia_enhaacplus_enc_init_pre_echo_control(FLOAT32 *pb_threshold_nm1, WORD32 num_pb,
                                             FLOAT32 *pb_threshold_quiet);

VOID ia_enhaacplus_enc_pre_echo_control(FLOAT32 *ptr_threshold_nm1, WORD32 num_sfb,
                                        FLOAT32 min_rem_thr_factor, FLOAT32 *ptr_threshold);
