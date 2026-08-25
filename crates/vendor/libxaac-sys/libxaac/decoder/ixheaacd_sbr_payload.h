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
#ifndef IXHEAACD_SBR_PAYLOAD_H
#define IXHEAACD_SBR_PAYLOAD_H

FLAG ixheaacd_check_for_sbr_payload(
    ia_bit_buf_struct *it_bit_buff,
    ia_aac_dec_sbr_bitstream_struct *pstr_stream_sbr, WORD16 prev_element,
    ia_drc_dec_struct *pstr_drc_dec, WORD32 object_type, WORD32 adtsheader,
    WORD32 cnt_bits, WORD32 ld_sbr_crc_flag, ia_drc_dec_struct *drc_dummy,
    UWORD8 *mps_buffer, WORD32 *mps_header, WORD32 *mps_bytes,
    WORD32 is_init, WORD32 *is_first, WORD32 ec_flag);

WORD8 ixheaacd_aac_plus_get_payload(
    ia_bit_buf_struct *it_bit_buff,
    ia_sbr_element_stream_struct *pstr_stream_sbr,
    ia_drc_dec_struct *pstr_drc_dec);

#endif
