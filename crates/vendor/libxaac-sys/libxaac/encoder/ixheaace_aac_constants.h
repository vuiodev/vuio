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
// FLOAT values
#define IXHEAACE_COS_PI_BY_4 (0.70709228515625f)      // 0.7071067812f
#define IXHEAACE_COS_PI_BY_8 (0.92388916015625f)      // 0.923879532511287f
#define IXHEAACE_SIN_PI_BY_8 (0.3826904296875f)       // 0.382683432365090f
#define IXHEAACE_COS_PI_BY_16 (0.98077392578125f)     // 0.980785280403230f
#define IXHEAACE_SIN_PI_BY_16 (0.195098876953125f)    // 0.195090322016128f
#define IXHEAACE_COS_3_PI_BY_16 (0.83148193359375f)   // 0.831469612302545f
#define IXHEAACE_SIN_3_PI_BY_16 (0.555572509765625f)  // 0.555570233019602f
#define IXHEAACE_INV_SQRT2 (7.071067811865475e-1f)
#define IXHEAACE_COS_PI_DIV8 (0.923879532511287f)
#define IXHEAACE_COS_3PI_DIV8 (3.826834323650898e-1f)
#define IXHEAACE_SQRT2PLUS1 (2.414213562373095f)
#define IXHEAACE_SQRT2MINUS1 (4.142135623730952e-1f)

#define INV_SQRT_2_FLOAT 0x3F3504F3  // 1/sqrt(2) in float

#define Q_POWER2_TABLE 30

#define DIV_FAC_24_BIT_PCM (256.0f)
#define DIV_FAC_32_BIT_PCM (65536.0f)

#define AUDIO_PROFILE_AAC_LC_L5 (0x2B)
#define AUDIO_PROFILE_HEAAC_L5 (0x2F)
#define AUDIO_PROFILE_HEAAC_V2_L5 (0x33)
#define AUDIO_PROFILE_AAC_LD_L4 (0x19)
#define AUDIO_PROFILE_AAC_ELD_L1 (0x4C)
#define AUDIO_PROFILE_AAC_ELD_L2 (0x4D)
#define AUDIO_PROFILE_AAC_ELD_L4 (0x4F)
#define AUDIO_PROFILE_USAC_L2 (0x45)
#define AUDIO_PROFILE_NOT_SPECIFIED (0xFE)

#define CLIP_SAVE_LO_LONG (0.2f)
#define CLIP_SAVE_HI_LONG (0.95f)
#define MIN_BITS_SAVE_LONG (-0.05f)
#define MAX_BITS_SAVE_LONG (0.3f)
#define CLIP_SPEND_LO_LONG (0.2f)
#define CLIP_SPEND_HI_LONG (0.95f)
#define MIN_BITS_SPEND_LONG (-0.10f)
#define MAX_BITS_SPEND_LONG (0.4f)
#define CLIP_SAVE_LO_SHORT (0.2f)
#define CLIP_SAVE_HI_SHORT (0.75f)
#define MIN_BITS_SAVE_SHORT (0.0f)
#define MAX_BITS_SAVE_SHORT (0.2f)
#define CLIP_SPEND_LO_SHORT (0.2f)
#define CLIP_SPEND_HI_SHORT (0.75f)
#define MIN_BITS_SPEND_SHORT (-0.05f)
#define MAX_BITS_SPEND_SHORT (0.5f)
typedef struct {
  FLOAT32 re;
  FLOAT32 im;
} complex;

typedef struct {
  UWORD8 *data;       /* data bits */
  WORD32 num_bit;     /* number of bits in buffer */
  WORD32 size;        /* buffer size in bytes */
  WORD32 current_bit; /* current bit position in bit stream */
} ixheaace_bitstream_params;

/* bits in byte (char) */
#define BYTE_NUMBIT 8
#define EIGHT_BYTE_SIZE (8)

/* here we distinguish between stereo and the mono only encoder */
#define IXHEAACE_MAX_CH_IN_BS_ELE (2)

#define MAXIMUM_BS_ELE \
  8 /* 1 <SCE> 2 <CPE> 3<CPE> 4<CPE> 5<LFE> 6<SCE> - 8.1 channel + 2 cc channels*/

#define FRAME_LEN_1024 1024
#define FRAME_LEN_512 512
#define FRAME_LEN_480 480
#define FRAME_LEN_960 960

#define AACENC_TRANS_FAC 8   /* encoder WORD16 long ratio */
#define AACENC_PCM_LEVEL 1.0 /* encoder pcm 0db refernence */

#define MAX_INPUT_CHAN (IXHEAACE_MAX_CH_IN_BS_ELE)

#define MAX_FRAME_LEN (1024)

/* channel masking*/
#define CH_MASK_CENTER_FRONT (0x4)
#define CH_MASK_LEFT_RIGHT_FRONT (0x3)
#define CH_MASK_REAR_CENTER (0x100)
#define CH_MASK_LEFT_RIGHT_BACK (0X30)
#define CH_MASK_LFE (0x08)

// Change to accommodate 4:1 resampler - input = 4096 samples per channel
#define MAX_INPUT_SAMPLES (MAX_FRAME_LEN * MAX_INPUT_CHAN * 4)

#define NUM_CHANS_MONO (1)
#define NUM_CHANS_STEREO (2)
#define MAX_NUM_CORE_CODER_CHANNELS (6)
#define MIN_NUM_CORE_CODER_CHANNELS (1)
/*-------------------------- defines --------------------------------------*/

#define BUFFERSIZE 1024 /* anc data */
#define MAX_GAIN_INDEX (128)