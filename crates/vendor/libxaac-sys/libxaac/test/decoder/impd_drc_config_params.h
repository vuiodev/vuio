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

#ifndef IMPD_DRC_CONFIG_PARAMS_H
#define IMPD_DRC_CONFIG_PARAMS_H

#define IA_DRC_DEC_CONFIG_PARAM_PCM_WDSZ 0x0000
#define IA_DRC_DEC_CONFIG_PARAM_SAMP_FREQ 0x0001
#define IA_DRC_DEC_CONFIG_PARAM_NUM_CHANNELS 0x0002
#define IA_DRC_DEC_CONFIG_PARAM_DEC_TYPE 0x0003
#define IA_DRC_DEC_CONFIG_PARAM_PEAK_LIMITER 0x0004
#define IA_DRC_DEC_CONFIG_PARAM_CTRL_PARAM 0x0005
#define IA_DRC_DEC_CONFIG_PARAM_VER_MODE 0x0006
#define IA_DRC_DEC_CONFIG_PARAM_BITS_FORMAT 0x0007
#define IA_DRC_DEC_CONFIG_PARAM_INT_PRESENT 0x0008
#define IA_DRC_DEC_CONFIG_PARAM_DELAY_MODE 0x0009
#define IA_DRC_DEC_CONFIG_PARAM_GAIN_DELAY 0x000A
#define IA_DRC_DEC_CONFIG_PARAM_AUDIO_DELAY 0x000B
#define IA_DRC_DEC_CONFIG_PARAM_CON_DELAY_MODE 0x000C
#define IA_DRC_DEC_CONFIG_PARAM_ABSO_DELAY_OFF 0x000D
#define IA_DRC_DEC_CONFIG_PARAM_FRAME_SIZE 0x000E
#define IA_DRC_DEC_CONFIG_PROC_OUT_PTR 0x000F
#define IA_DRC_DEC_CONFIG_GAIN_STREAM_FLAG 0x0010
#define IA_DRC_DEC_CONFIG_DRC_EFFECT_TYPE 0x0011
#define IA_DRC_DEC_CONFIG_DRC_TARGET_LOUDNESS 0x0012
#define IA_DRC_DEC_CONFIG_DRC_LOUD_NORM 0x0013
#define IA_DRC_DEC_CONFIG_DRC_BOOST 0x0015
#define IA_DRC_DEC_CONFIG_DRC_CUT 0x0016
#define IA_DRC_DEC_CONFIG_PARAM_APPLY_CROSSFADE 0x0017
#define IA_DRC_DEC_CONFIG_PARAM_CONFIG_CHANGED 0x0018
#ifdef LOUDNESS_LEVELING_SUPPORT
#define IA_DRC_DEC_CONFIG_DRC_LOUDNESS_LEVELING 0x0019
#endif
#define IA_API_CMD_SET_INPUT_BYTES_BS 0x0026
#define IA_API_CMD_SET_INPUT_BYTES_IC_BS 0x0027
#define IA_API_CMD_SET_INPUT_BYTES_IL_BS 0x0029
#define IA_API_CMD_SET_INPUT_BYTES_IN_BS 0x002A

#define IA_CMD_TYPE_INIT_CPY_BSF_BUFF 0x0201
#define IA_CMD_TYPE_INIT_CPY_IC_BSF_BUFF 0x0202
#define IA_CMD_TYPE_INIT_CPY_IL_BSF_BUFF 0x0203
#define IA_CMD_TYPE_INIT_CPY_IN_BSF_BUFF 0x0205

#define IA_CMD_TYPE_INIT_SET_BUFF_PTR 0x020B

#endif
