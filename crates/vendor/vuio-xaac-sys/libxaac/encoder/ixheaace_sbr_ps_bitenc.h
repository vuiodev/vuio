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
#define CODE_BCK_LAV_IID 14
#define CODE_BCK_LAV_ICC 7
#define NO_IID_STEPS (7)
#define NO_ICC_STEPS (8)

struct ixheaace_ps_enc;

WORD32 ixheaace_enc_write_ps_data(ixheaace_pstr_ps_enc pstr_ps_handle, WORD32 b_header_active,
                                  ixheaace_str_ps_tab *ps_tabs);

WORD32 ixheaace_append_ps_bitstream(ixheaace_pstr_ps_enc pstr_ps_handle,
                                    ixheaace_bit_buf_handle hdl_bs, WORD32 *sbr_hdr_bits);
