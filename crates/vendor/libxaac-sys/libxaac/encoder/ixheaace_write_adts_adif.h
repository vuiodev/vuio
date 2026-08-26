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

WORD32 ia_enhaacplus_enc_write_pce(WORD32 samp_rate, WORD32 ch_mask, WORD32 num_core_coder_chans,
                                   ixheaace_bit_buf_handle pstr_bit_stream_handle);

WORD32 ia_enhaacplus_enc_write_ADTS_header(pUWORD8 buffer, WORD32 bytes_used, WORD32 samp_rate,
                                           WORD32 num_ch);
