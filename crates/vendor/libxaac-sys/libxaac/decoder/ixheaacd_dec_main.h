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
#ifndef IXHEAACD_DEC_MAIN_H
#define IXHEAACD_DEC_MAIN_H

WORD32 ixheaacd_decode_init(
    VOID *dec_handle, WORD32 sampling_rate_decoded,
    ia_usac_data_struct *usac_data, WORD32 profile,
    ia_audio_specific_config_struct *pstr_stream_config);

WORD32 ixheaacd_usac_process(ia_dec_data_struct *pstr_dec_data,
                             WORD32 *num_out_channels, VOID *dec_handle);

#endif
