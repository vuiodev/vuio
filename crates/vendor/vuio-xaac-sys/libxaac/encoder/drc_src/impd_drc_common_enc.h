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
#define MAX_DRC_PAYLOAD_BYTES (2048)
#define MAX_SPEAKER_POS_COUNT (128)
#define MAX_DOWNMIX_COEFF_COUNT (32 * 32)
#define MAX_CHANNEL_COUNT (8)
#define MAX_BAND_COUNT (8)
#define MAX_SEQUENCE_COUNT (8)
#define MAX_MEASUREMENT_COUNT (15)
#define MAX_DOWNMIX_INSTRUCTION_COUNT (16)
#define MAX_DRC_COEFF_COUNT (7)
#define MAX_DRC_INSTRUCTIONS_COUNT (MAX_DOWNMIX_INSTRUCTION_COUNT + 16)
#define MAX_LOUDNESS_INFO_COUNT (MAX_DOWNMIX_INSTRUCTION_COUNT + 16)
#define MAX_AUDIO_CODEC_FRAME_SIZE (2048)
#define MAX_DRC_CODEC_FRAME_SIZE (MAX_AUDIO_CODEC_FRAME_SIZE / 8)
#define MAX_NODE_COUNT (MAX_DRC_CODEC_FRAME_SIZE)
#define MAX_CHANNEL_GROUP_COUNT (MAX_SEQUENCE_COUNT)
#define MAX_ADDITIONAL_DOWNMIX_ID (7)
#define DELAY_MODE_REGULAR_DELAY (0)
#ifdef LOUDNESS_LEVELING_SUPPORT
#define MAX_EXT_COUNT (3)
#else
#define MAX_EXT_COUNT (2)
#endif
#define MAX_GAIN_POINTS (256)
#define MAX_DRC_INSTRUCTIONS_BASIC_COUNT (15)

#define UNIDRC_GAIN_EXT_TERM (0x0)
#define UNIDRC_LOUD_EXT_TERM (0x0)
#define UNIDRC_CONF_EXT_TERM (0x0)
#define UNIDRC_CONF_EXT_PARAM_DRC (0x1)
#define UNIDRC_CONF_EXT_V1 (0x2)
#define UNIDRC_LOUD_EXT_EQ (0x1)
#ifdef LOUDNESS_LEVELING_SUPPORT
#define UNIDRCCONFEXT_LEVELING (0x4)
#endif

#define MAX_PARAM_DRC_INSTRUCTIONS_COUNT (8)

#define PARAM_DRC_TYPE_FF (0x0)
#define MAX_PARAM_DRC_TYPE_FF_NODE_COUNT (9)

#define PARAM_DRC_TYPE_LIM (0x1)
#define PARAM_DRC_TYPE_LIM_ATTACK_DEFAULT (5)

#define SUBBAND_DOMAIN_MODE_OFF (0)
#define SUBBAND_DOMAIN_MODE_QMF64 (1)
#define SUBBAND_DOMAIN_MODE_QMF71 (2)
#define SUBBAND_DOMAIN_MODE_STFT256 (3)

#define QMF64_AUDIO_CODEC_SUBBAND_COUNT (64)
#define QMF64_AUDIO_CODEC_SUBBAND_DOWNSAMPLING_FACTOR (64)

#define QMF71_AUDIO_CODEC_SUBBAND_COUNT (71)
#define QMF71_AUDIO_CODEC_SUBBAND_DOWNSAMPLING_FACTOR (64)

#define STFT256_AUDIO_CODEC_SUBBAND_COUNT (256)
#define STFT256_AUDIO_CODEC_SUBBAND_DOWNSAMPLING_FACTOR (256)

#define TIME_DOMAIN (1)
#define SUBBAND_DOMAIN (2)
#define SLOPE_FACTOR_DB_TO_LINEAR (0.1151f) /* ln(10) / 20 */

#define MAX_DRC_CONFIG_SIZE_EXPECTED (14336) /* 14 KB*/

#ifndef MIN
#define MIN(a, b) ((a) < (b) ? (a) : (b))
#endif
#ifndef MAX
#define MAX(a, b) ((a) > (b) ? (a) : (b))
#endif
