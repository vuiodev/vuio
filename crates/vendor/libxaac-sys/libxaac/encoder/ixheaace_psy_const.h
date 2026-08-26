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
#ifndef TRUE
#define TRUE (1)
#endif
#ifndef FALSE
#define FALSE (0)
#endif

#define TRANS_FAC 8
#define FRAME_LEN_SHORT_128 (128)
#define FRAME_LEN_SHORT_120 (120)

/* Block types */
enum { LONG_WINDOW = 0, START_WINDOW, SHORT_WINDOW, STOP_WINDOW };

/* Window shapes */
enum { SINE_WINDOW = 0, KBD_WINDOW = 1 };

enum { LD_WINDOW = 1 };

/*  MS stuff */
enum { SI_MS_MASK_NONE = 0, SI_MS_MASK_SOME = 1, SI_MS_MASK_ALL = 2 };

#define MDCT_LEN 480
#define MDCT_LEN_BY2 240
#define FFT5 (5)
#define FFT2 (2)
#define FFT16 (16)
#define FFT4 (4)
#define FFT3 (3)
#define FFT15 (15)
#define FFT15X2 (30)
#define FFT32 (32)
#define FFT16X2 (32)
#define FFT32X2 (FFT32 * 2)

#define MAX_FLT_VAL (3.402823466e+38F)
#define MIN_FLT_VAL (1.175494351e-38F)
#define MIN_SHRT_VAL (-32768)
#define MAX_SHRT_VAL (32767)

#define MAXIMUM_NO_OF_GROUPS 4

#define MAXIMUM_SCALE_FACTOR_BAND_SHORT 15
#define MAXIMUM_SCALE_FACTOR_BAND_LONG 51

/* = MAXIMUM_SCALE_FACTOR_BAND_LONG */
#define MAXIMUM_SCALE_FACTOR_BAND                                   \
  (MAXIMUM_SCALE_FACTOR_BAND_SHORT > MAXIMUM_SCALE_FACTOR_BAND_LONG \
       ? MAXIMUM_SCALE_FACTOR_BAND_SHORT                            \
       : MAXIMUM_SCALE_FACTOR_BAND_LONG)
#define MAXIMUM_GROUPED_SCALE_FACTOR_BAND                                                  \
  (MAXIMUM_NO_OF_GROUPS * MAXIMUM_SCALE_FACTOR_BAND_SHORT > MAXIMUM_SCALE_FACTOR_BAND_LONG \
       ? MAXIMUM_NO_OF_GROUPS * MAXIMUM_SCALE_FACTOR_BAND_SHORT                            \
       : MAXIMUM_SCALE_FACTOR_BAND_LONG)

#define MAX_CHANNELS (2)
/* For 2:1 resampler -> max phase delay * resamp_fac */
#define MAX_DS_2_1_FILTER_DELAY (16)

/* For 4:1 resampler -> max phase delay * resamp_fac */
#define MAX_DS_4_1_FILTER_DELAY (64)

/* For 8:1 resampler -> max phase delay * resamp_fac */
#define MAX_DS_8_1_FILTER_DELAY (248)

/* For 1:3 resampler -> max phase delay * resamp_fac */
#define MAX_DS_1_3_FILTER_DELAY (36)

#define BLK_SWITCH_OFFSET_LC_128 (1 * 1024 + 3 * 128 + 64 + 128)
#define BLK_SWITCH_OFFSET_LC_120 (1 * 960 + 3 * 120 + 60 + 120)
#define BLK_SWITCH_OFFSET_LD (2048)

#define MAXIMUM_CHANNEL_BITS_1024 6144
#define MAXIMUM_CHANNEL_BITS_960 5760
#define MAXIMUM_CHANNEL_BITS_512 3072
#define MAXIMUM_CHANNEL_BITS_480 2880

#define TRANSFORM_OFFSET_SHORT_128 448
#define TRANSFORM_OFFSET_SHORT_120 420

#define PCM_LEVEL 1.0f
#define NORMALIZED_PCM (PCM_LEVEL)
#define CLIP_ENERGY_VALUE_LONG (1.0e9f)
#define FADE_OUT_LEN (6)
#define MIN_THRESH_FAC (0.01f)
