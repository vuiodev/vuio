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
IA_ERRORCODE impd_drc_gain_enc_init(ia_drc_gain_enc_struct *pstr_gain_enc,
                                    ia_drc_uni_drc_config_struct *pstr_uni_drc_config,
                                    ia_drc_loudness_info_set_struct *pstr_loudness_info_set,
                                    const WORD32 frame_size, const WORD32 sample_rate,
                                    const WORD32 delay_mode, const WORD32 domain);

WORD32 impd_drc_get_delta_t_min(const WORD32 sample_rate);

IA_ERRORCODE impd_drc_encode_uni_drc_gain(ia_drc_gain_enc_struct *pstr_gain_enc,
                                          FLOAT32 *ptr_gain_buffer, VOID *pstr_scratch);

IA_ERRORCODE impd_drc_write_loudness_info_set_extension(
    ia_drc_enc_state *pstr_drc_state, ia_bit_buf_struct *it_bit_buf,
    ia_drc_loudness_info_set_extension_struct *pstr_loudness_info_set_extension,
    WORD32 *ptr_bit_cnt, FLAG write_bs);

IA_ERRORCODE impd_drc_write_uni_drc_config(ia_drc_enc_state *pstr_drc_state, WORD32 *ptr_bit_cnt,
                                           FLAG write_bs);

VOID impd_drc_write_uni_drc_gain(ia_drc_enc_state *pstr_drc_state, WORD32 *ptr_bit_cnt);

IA_ERRORCODE impd_drc_write_measured_loudness_info(ia_drc_enc_state *pstr_drc_state);

IA_ERRORCODE impd_drc_write_loudness_info_set(ia_drc_enc_state *pstr_drc_state,
                                              ia_bit_buf_struct *it_bit_buf, WORD32 *ptr_bit_cnt,
                                              FLAG write_bs);