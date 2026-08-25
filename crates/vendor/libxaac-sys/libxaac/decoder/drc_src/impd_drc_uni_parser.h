/******************************************************************************
 *
 * Copyright (C) 2015 The Android Open Source Project
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
#ifndef IMPD_DRC_UNI_PARSER_H
#define IMPD_DRC_UNI_PARSER_H

WORD32
impd_dec_ducking_scaling(ia_bit_buf_struct* it_bit_buff,
                         WORD32* ducking_scaling_flag,
                         FLOAT32* ducking_scaling);

WORD32
impd_parse_loudness_info(ia_bit_buf_struct* it_bit_buff, WORD32 version,
                         ia_loudness_info_struct* loudness_info);

WORD32
impd_parse_loudness_info_set_ext(
    ia_bit_buf_struct* it_bit_buff,
    ia_drc_loudness_info_set_struct* loudness_info_set);

WORD32
impd_sel_drc_coeff(
    ia_drc_config* drc_config, WORD32 location,
    ia_uni_drc_coeffs_struct** str_p_loc_drc_coefficients_uni_drc);

WORD32
impd_drc_parse_instructions_basic(
    ia_bit_buf_struct* it_bit_buff, ia_drc_config* drc_config,
    ia_drc_instructions_basic_struct* str_drc_instructions_basic);

WORD32
impd_parse_drc_instructions_uni_drc(
    ia_bit_buf_struct* it_bit_buff, WORD32 version, ia_drc_config* drc_config,
    ia_channel_layout_struct* channel_layout,
    ia_drc_params_bs_dec_struct* ia_drc_params_struct,
    ia_drc_instructions_struct* str_drc_instruction_str);

WORD32
impd_parse_gain_set_params(ia_bit_buf_struct* it_bit_buff, WORD32 version,
                           WORD32* gain_seq_idx,
                           ia_gain_set_params_struct* gain_set_params);

WORD32
impd_drc_parse_coeff(
    ia_bit_buf_struct* it_bit_buff, WORD32 version,
    ia_drc_params_bs_dec_struct* ia_drc_params_struct,
    ia_uni_drc_coeffs_struct* str_p_loc_drc_coefficients_uni_drc);

WORD32
impd_parse_dwnmix_instructions(
    ia_bit_buf_struct* it_bit_buff, WORD32 version,
    ia_drc_params_bs_dec_struct* ia_drc_params_struct,
    ia_channel_layout_struct* channel_layout,
    ia_downmix_instructions_struct* dwnmix_instructions);

WORD32
impd_parse_drc_config(ia_bit_buf_struct* it_bit_buff,
                      ia_drc_params_bs_dec_struct* ia_drc_params_struct,
                      ia_drc_config* drc_config);

WORD32
impd_parse_loudness_info_set(
    ia_bit_buf_struct* it_bit_buff,
    ia_drc_params_bs_dec_struct* ia_drc_params_struct,
    ia_drc_loudness_info_set_struct* loudness_info_set);

WORD32
impd_parse_drc_gain_sequence(ia_bit_buf_struct* it_bit_buff,
                             ia_drc_bits_dec_struct* pstr_drc_uni_bs_dec,
                             ia_gain_set_params_struct* gain_set_params,
                             ia_drc_gain_sequence_struct* drc_gain_sequence);

WORD32
impd_parse_uni_drc_gain_ext(ia_bit_buf_struct* it_bit_buff,
                            ia_uni_drc_gain_ext_struct* uni_drc_gain_ext);

WORD32 impd_drc_uni_gain_read(ia_bit_buf_struct* it_bit_buff,
                              ia_drc_bits_dec_struct* pstr_drc_uni_bs_dec,
                              ia_drc_config* drc_config,
                              ia_drc_gain_struct* pstr_uni_drc_gain);

#endif
