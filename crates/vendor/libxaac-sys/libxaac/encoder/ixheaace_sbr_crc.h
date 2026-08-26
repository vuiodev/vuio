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
#define CRC_POLYNOMIAL_SBR (0x0233)
#define CRC_MASK_SBR (0x0200)
#define CRC_RANGE_SBR (0x03FF)
#define CRC_MAXREGS_SBR (1)
#define CRCINIT_SBR (0x0)

#define SI_FIL_SBR (13)
#define SI_FIL_CRC_SBR (14)

#define SI_CRC_ENABLE_BITS_SBR (0)
#define SI_CRC_BITS_SBR (10)

#define SI_ID_BITS_AAC (3)
#define SI_FILL_COUNT_BITS (4)
#define SI_FILL_ESC_COUNT_BITS (8)
#define SI_FILL_EXTENTION_BITS (4)
#define ID_FIL (6)

struct ixheaace_str_common_data;

VOID ixheaace_init_sbr_bitstream(ixheaace_pstr_common_data pstr_cmon_data,
                                 UWORD8 *ptr_memory_base, WORD32 memory_size, WORD32 crc_active,
                                 ixheaace_sbr_codec_type sbr_codec);

VOID ixheaace_assemble_sbr_bitstream(ixheaace_pstr_common_data pstr_cmon_data,
                                     ixheaace_sbr_codec_type sbr_codec);
