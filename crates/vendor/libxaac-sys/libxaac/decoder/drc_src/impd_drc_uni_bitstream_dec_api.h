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
#ifndef IMPD_DRC_UNI_BITSTREAM_DEC_API_H
#define IMPD_DRC_UNI_BITSTREAM_DEC_API_H

WORD32 impd_init_drc_bitstream_dec(
    ia_drc_bits_dec_struct* p_uni_drc_bs_dec_struct, WORD32 sample_rate,
    WORD32 frame_size, WORD32 delay_mode, WORD32 lfe_channel_map_count,
    WORD32* lfe_channel_map);

WORD32 impd_process_drc_bitstream_dec(
    ia_drc_bits_dec_struct* p_uni_drc_bs_dec_struct,
    ia_bit_buf_struct* it_bit_buff, ia_drc_config* pstr_drc_config,
    ia_drc_loudness_info_set_struct* pstr_loudness_info,
    UWORD8* bitstream_config, WORD32 num_bytes, WORD32 num_bits_offset,
    WORD32* num_bits_read);

WORD32 impd_process_drc_bitstream_dec_config(
    ia_drc_bits_dec_struct* p_uni_drc_bs_dec_struct,
    ia_bit_buf_struct* it_bit_buff, ia_drc_config* pstr_drc_config,
    UWORD8* bitstream_config, WORD32 num_bytes);

WORD32 impd_process_drc_bitstream_dec_gain(
    ia_drc_bits_dec_struct* p_uni_drc_bs_dec_struct,
    ia_bit_buf_struct* it_bit_buff, ia_drc_config* pstr_drc_config,
    ia_drc_gain_struct* pstr_drc_gain, UWORD8* bitstream_gain, WORD32 num_bytes,
    WORD32 num_bits_offset, WORD32* num_bits_read);

WORD32 impd_process_drc_bitstream_dec_loudness_info_set(
    ia_drc_bits_dec_struct* p_uni_drc_bs_dec_struct,
    ia_bit_buf_struct* it_bit_buff,
    ia_drc_loudness_info_set_struct* pstr_loudness_info,
    UWORD8* bitstream_loudness, WORD32 num_bytes_loudness);
#endif
