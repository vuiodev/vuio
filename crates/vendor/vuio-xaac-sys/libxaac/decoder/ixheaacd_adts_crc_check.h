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
#ifndef IXHEAACD_ADTS_CRC_CHECK_H
#define IXHEAACD_ADTS_CRC_CHECK_H

VOID ixheaacd_adts_crc_open(ia_adts_crc_info_struct *ptr_adts_crc_info);
WORD32 ixheaacd_adts_crc_start_reg(ia_adts_crc_info_struct *ptr_adts_crc_info,
                                   ia_bit_buf_struct *it_bit_buff_src,
                                   WORD32 no_bits);
VOID ixheaacd_adts_crc_end_reg(ia_adts_crc_info_struct *ptr_adts_crc_info,
                               ia_bit_buf_struct *it_bit_buff_src, WORD32 reg);
WORD32 ixheaacd_adts_crc_check_crc(ia_adts_crc_info_struct *ptr_adts_crc_info);

#endif /* #ifndef IXHEAACD_ADTS_CRC_CHECK_H */
