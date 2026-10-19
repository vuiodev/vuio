/******************************************************************************
 *
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

#ifndef IMPD_DRC_ERROR_CODES_H
#define IMPD_DRC_ERROR_CODES_H

#define IA_ERROR_CODE WORD32
#define IA_NO_ERROR 0x00000000

/*API Non-Fatal Errors */
#define IA_DRC_DEC_API_NONFATAL_NO_ERROR 0x00000000
#define IA_DRC_DEC_API_NONFATAL_CMD_NOT_SUPPORTED 0x00000001
#define IA_DRC_DEC_API_NONFATAL_CMD_TYPE_NOT_SUPPORTED 0x00000002

/*API Fatal Errors */
#define IA_DRC_DEC_API_FATAL_INVALID_MEMTAB_INDEX 0xFFFF8000
#define IA_DRC_DEC_API_FATAL_INVALID_LIB_ID_STRINGS_IDX 0xFFFF8001
#define IA_DRC_DEC_API_FATAL_MEM_ALLOC 0xFFFF8002
#define IA_DRC_DEC_API_FATAL_INVALID_CONFIG_PARAM 0xFFFF8003
#define IA_DRC_DEC_API_FATAL_INVALID_EXECUTE_TYPE 0xFFFF8004
#define IA_DRC_DEC_API_FATAL_INVALID_CMD 0xFFFF8005
#define IA_DRC_DEC_API_FATAL_MEM_ALIGN 0xFFFF8006
/*****************************************************************************/
/* Class 1: Configuration Errors                                             */
/*****************************************************************************/

/* Non Fatal Errors */
#define IA_DRC_DEC_CONFIG_NON_FATAL_INVALID_NUM_OF_CHANNELS 0x00000800
#define IA_DRC_DEC_CONFIG_NON_FATAL_INVALID_SAMP_FREQ 0x00000801
#define IA_DRC_DEC_CONFIG_NON_FATAL_INVALID_PCM_SIZE 0x00000802
#define IA_DRC_DEC_CONFIG_NON_FATAL_INVALID_FRAME_SIZE 0x00000803
#define IA_DRC_DEC_CONFIG_NON_FATAL_INVALID_DELAY_MODE 0x00000804
#define IA_DRC_DEC_CONFIG_NON_FATAL_INVALID_DECODE_TYPE 0x00000805
#define IA_DRC_DEC_CONFIG_NON_FATAL_INVALID_PEAK_LIM_FLAG 0x00000806
#define IA_DRC_DEC_CONFIG_NON_FATAL_INVALID_CTRL_PARAM_IDX 0x00000807
#define IA_DRC_DEC_CONFIG_NON_FATAL_INVALID_GAIN_DELAY 0x00000808
#define IA_DRC_DEC_CONFIG_NON_FATAL_INVALID_CONST_DELAY_MODE 0x00000809

#endif
