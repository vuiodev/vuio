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
#ifndef IXHEAACD_SBR_CRC_H
#define IXHEAACD_SBR_CRC_H

#define SBR_CRC_POLY 0x0233
#define SBR_CRC_MASK 0x0200
#define SBR_CRC_START 0x0000
#define SBR_CRC_RANGE 0x03FF

FLAG ixheaacd_sbr_crccheck(ia_bit_buf_struct* it_bit_buff, WORD32 crc_bits_len);

#endif
