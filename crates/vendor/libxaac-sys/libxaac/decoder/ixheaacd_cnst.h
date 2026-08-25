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
#ifndef IXHEAACD_CNST_H
#define IXHEAACD_CNST_H

#define MAX_ELEMENTS 68

#define LEN_SUPERFRAME 1024
#define LEN_FRAME 256
#define NUM_FRAMES 4
#define MAX_NUM_SUBFR 4

#define ORDER 16
#define ORDER_BY_2 (ORDER / 2)

#define FAC_LENGTH 128

#define NUM_SUBFR_SUPERFRAME 16
#define NUM_SUBFR_SUPERFRAME_BY2 (NUM_SUBFR_SUPERFRAME / 2)
#define SYNTH_DELAY_LMAX ((NUM_SUBFR_SUPERFRAME_BY2 - 1) * LEN_SUBFR)

#define FSCALE_DENOM 12800
#define FAC_FSCALE_MAX 24000
#define FAC_FSCALE_MIN 6000

#define FILTER_DELAY 12

#define TMIN 34
#define TFR2 (162 - TMIN)
#define TFR1 160
#define TMAX (27 + 6 * TMIN)

#define UP_SAMP 4
#define INTER_LP_FIL_ORDER 16
#define INTER_LP_FIL_LEN (UP_SAMP * INTER_LP_FIL_ORDER + 1)

#define MAX_PITCH \
  (TMAX +         \
   (6 *           \
    ((((FAC_FSCALE_MAX * TMIN) + (FSCALE_DENOM / 2)) / FSCALE_DENOM) - TMIN)))

#define PI 3.14159265358979323846264338327950288

#define PREEMPH_FILT_FAC 0.68f
#define PREEMPH_FAC_FIX 44564

#define IGAMMA1 1975684956

#define TILT_CODE 0.3f

#define BLOCK_LEN_LONG 1024
#define BLOCK_LEN_SHORT 128
#define BLOCK_LEN_LONG_S 960
#define BLOCK_LEN_SHORT_S 120

#define WIN_LEN_1024 1024
#define WIN_LEN_768 768
#define WIN_LEN_512 512
#define WIN_LEN_192 192
#define WIN_LEN_128 128
#define WIN_LEN_960 960
#define WIN_LEN_480 480
#define WIN_LEN_256 256
#define WIN_LEN_120 120
#define WIN_LEN_96 96
#define WIN_LEN_64 64
#define WIN_LEN_48 48
#define WIN_LEN_16 16
#define WIN_LEN_4 4

#define TW_IPLEN2S 12
#define TW_OS_FACTOR_WIN 16

#define WIN_SEL_0 0
#define WIN_SEL_1 1
#define WIN_SEL_2 2

#define NUM_TW_NODES 16

#define CORE_MODE_FD 0
#define CORE_MODE_LPD 1

#define ONLY_LONG_SEQUENCE 0
#define LONG_START_SEQUENCE 1
#define EIGHT_SHORT_SEQUENCE 2
#define LONG_STOP_SEQUENCE 3
#define STOP_START_SEQUENCE 4
#define NUM_WIN_SEQ 5

#define NSFB_SHORT 16
#define MAX_SHORT_IN_LONG_BLOCK 8
#define MAX_SHORT_WINDOWS 8

#define MAX_NUM_CHANNELS 6
#define MAX_NUM_CHANNELS_USAC_LVL2 2

#define SFB_NUM_MAX ((NSFB_SHORT + 1) * MAX_SHORT_IN_LONG_BLOCK)

#define MAX_31 (WORD32)0x3FFFFFFF
#define MIN_31 (WORD32)0xC0000000
#define LEN_SUBFR 64

#define SFB_PER_PRED_BAND 2

#define MAX_HEADER_BUF_SIZE 8300

#endif /* IXHEAACD_CNST_H */
