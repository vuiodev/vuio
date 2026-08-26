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

#define NULL_PTR ((void *)0)

typedef enum { IXHEAACE_SBR_ID_SCE = 1, IXHEAACE_SBR_ID_CPE } ixheaace_sbr_element_type;

IA_ERRORCODE
ixheaace_write_env_single_channel_element(
    ixheaace_pstr_sbr_hdr_data pstr_sbr_hdr, ixheaace_pstr_sbr_bitstream_data pstr_sbr_bs,
    ixheaace_pstr_sbr_env_data pstr_sbr_env_info, struct ixheaace_ps_enc *pstr_ps_handle,
    ixheaace_pstr_common_data pstr_cmon_data, ixheaace_str_sbr_tabs *pstr_sbr_tab,
    ixheaace_sbr_codec_type sbr_codec, WORD32 is_esbr, ixheaace_str_esbr_bs_data *pstr_esbr_data,
    WORD32 *ptr_num_bits);

IA_ERRORCODE
ixheaace_write_env_channel_pair_element(
    ixheaace_pstr_sbr_hdr_data pstr_sbr_hdr, ixheaace_pstr_sbr_bitstream_data pstr_sbr_bs,
    ixheaace_pstr_sbr_env_data pstr_sbr_env_data_left,
    ixheaace_pstr_sbr_env_data pstr_sbr_env_data_right, ixheaace_pstr_common_data pstr_cmon_data,
    ixheaace_str_sbr_tabs *pstr_sbr_tab, ixheaace_sbr_codec_type sbr_codec, WORD32 is_esbr,
    ixheaace_str_esbr_bs_data *pstr_esbr_data, WORD32 *ptr_num_bits);

IA_ERRORCODE
ixheaace_count_sbr_channel_pair_element(
    ixheaace_pstr_sbr_hdr_data pstr_sbr_hdr, ixheaace_pstr_sbr_bitstream_data pstr_sbr_bs,
    ixheaace_pstr_sbr_env_data pstr_sbr_env_data_left,
    ixheaace_pstr_sbr_env_data pstr_sbr_env_data_right, ixheaace_pstr_common_data pstr_cmon_data,
    ixheaace_str_sbr_tabs *pstr_sbr_tab, ixheaace_sbr_codec_type sbr_codec, WORD32 is_esbr,
    ixheaace_str_esbr_bs_data *pstr_esbr_data, WORD32 *ptr_num_bits);
