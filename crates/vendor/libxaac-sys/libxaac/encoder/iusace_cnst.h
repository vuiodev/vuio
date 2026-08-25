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

#ifndef PI
#define PI (3.14159265358979323846f)
#endif

#define CORE_MODE_FD (0)
#define CORE_MODE_TD (1)
#define USAC_SWITCHED (0)
#define USAC_ONLY_FD (1)
#define USAC_ONLY_TD (2)

#define TREE_212 (0)
#define TREE_5151 (1)
#define TREE_5152 (2)
#define TREE_525 (3)
#define INVALID_TREE_CONFIG (-1)

#define ONLY_LONG_SEQUENCE (0)
#define LONG_START_SEQUENCE (1)

#define LONG_STOP_SEQUENCE (3)
#define STOP_START_SEQUENCE (4)
#define NSFB_SHORT (16)
#define MAX_SHORT_IN_LONG_BLOCK (8)
#define MAX_SHORT_WINDOWS (8)
#define MAX_SFB_LONG 51
#define FRAME_LEN_LONG 1024
#define FRAME_LEN_SHORT_768 (96)
#define MAX_TIME_CHANNELS (2)
#define MAX_SF_BANDS ((NSFB_SHORT + 1) * MAX_SHORT_IN_LONG_BLOCK)
#define MAX_SHIFT_LEN_LONG (4096)
#define TD_BUFFER_OFFSET (448 + 64)
#define MAX_EXTENSION_PAYLOADS MAX_TIME_CHANNELS
#define MAX_EXTENSION_PAYLOAD_LEN 6144 / 8 * MAX_TIME_CHANNELS

#define WIN_SEL_0 0
#define WIN_SEL_1 1

#define LEN_SUPERFRAME (1024)
#define LEN_SUPERFRAME_768 (768)
#define LEN_WIN_PLUS (512)
#define OVERLAP_WIN_SIZE_576 (576)

#define ORDER (16)
#define ORDER_BY_2 (8)

#define LEN_FRAME (256)
#define NUM_FRAMES (4)
#define MAX_NUM_SUBFR (4)
#define LEN_SUBFR (64)

#define N_MAX (LEN_SUPERFRAME)
#define FAC_LENGTH (LEN_FRAME / 2)
#define NUM_SUBFR_SUPERFRAME (NUM_FRAMES * MAX_NUM_SUBFR)
#define FDNS_RESOLUTION (64)

#define LEN_NEXT_HIGH_RATE (288)
#define LEN_LPC0 (256)
#define LEN_LP_WINDOW (448)
#define LEN_LP_WINDOW_HIGH_RATE (512)

#define SR_MIN (6000)
#define SR_MAX (32000)

#define FSCALE_DENOM (12800)
#define FAC_FSCALE_MAX (24000)

#define LEN_TOTAL_HIGH_RATE (ORDER + LEN_SUPERFRAME + LEN_NEXT_HIGH_RATE)

#define FILTER_DELAY (12)

#define TMIN (34)
#define TFR2 (128)
#define TFR1 (160)
#define TMAX (231)

#define T_UP_SAMP (4)
#define INTER_LP_FIL_ORDER (16)
#define INTER_LP_FIL_LEN (T_UP_SAMP * INTER_LP_FIL_ORDER + 1)

/* upto 410 for 24k sampling rate */
#define MAX_PITCH \
  (TMAX + (6 * ((((FAC_FSCALE_MAX * TMIN) + (FSCALE_DENOM / 2)) / FSCALE_DENOM) - TMIN)))

/* upto 536 for 32k sampling rate */
#define MAX_PITCH1 (TMAX + (6 * ((((32000 * TMIN) + (FSCALE_DENOM / 2)) / FSCALE_DENOM) - TMIN)))

#define LEN_INTERPOL (16 + 1)
#define OPL_DECIM (2)
#define PREEMPH_FILT_FAC (0.68f)
#define TILT_FAC (0.68f)
#define PIT_SHARP (0.85f)
#define TILT_CODE (0.3f)

/* AMR_WB+ mode relative to AMR-WB core */
#define ACELP_CORE_MODE_9k6 (0)
#define ACELP_CORE_MODE_11k2 (1)
#define ACELP_CORE_MODE_12k8 (2)
#define ACELP_CORE_MODE_14k4 (3)
#define ACELP_CORE_MODE_16k (4)
#define ACELP_CORE_MODE_18k4 (5)

#define ACELP_NUM_BITS_20 (20)
#define ACELP_NUM_BITS_28 (28)
#define ACELP_NUM_BITS_36 (36)
#define ACELP_NUM_BITS_44 (44)
#define ACELP_NUM_BITS_52 (52)
#define ACELP_NUM_BITS_64 (64)
#define ACELP_NUM_BITS_72 (72)
#define ACELP_NUM_BITS_88 (88)

#define NUM_ACELP_CORE_MODES (6)
#define NBITS_MAX (48 * 80 + 46)

#define NBITS_MODE (4 * 2)
#define NBITS_LPC (46)

#define NUM_RE8_PRM (LEN_SUPERFRAME + (LEN_SUPERFRAME / 8))

#define NUM_TCX80_PRM (FAC_LENGTH + 2 + NUM_RE8_PRM)
#define NUM_TCX40_PRM (FAC_LENGTH + 2 + (NUM_RE8_PRM / 2))
#define NUM_TCX20_PRM (FAC_LENGTH + 2 + (320 + 320 / 8))

#define NUM_LPC_PRM (256)
#define MAX_NUM_TCX_PRM_PER_DIV (NUM_TCX20_PRM)

#define L_OLD_SPEECH_HIGH_RATE LEN_TOTAL_HIGH_RATE - LEN_SUPERFRAME

#define HP_ORDER (3)
#define LEN_INTERPOL1 (4)

#define NUM_OPEN_LOOP_LAGS (5)
#define OPEN_LOOP_LAG_MEDIAN (3)
#define DECIM2_FIR_FILT_MEM_SIZE (3)

#define NUM_QUANTIZATION_LEVEL (128)
#define LEV_DUR_MAX_ORDER (24)
#define PI_BY_6400 (PI / 6400.0)
#define LEN_FRAME_16K 320
#define ORDER_LP_FILT_16K (20)
#define LSP_2_LSF_SCALE (6400.0 / PI)
#define FREQ_MAX (6400.0f)
#define FREQ_DIV (400.0f)
#define CHEBYSHEV_NUM_ITER (4)
#define CHEBYSHEV_NUM_POINTS (100)
#define LSF_GAP (50.0f)

#define MAX_NUM_PULSES (24)
#define NPMAXPT ((MAX_NUM_PULSES + 4 - 1) / 4)
#define ACELP_GAIN_TBL_OFFSET (64)
#define ACELP_RANGE_GAIN_PT_IDX_SEARCH (NUM_QUANTIZATION_LEVEL - ACELP_GAIN_TBL_OFFSET)
#define ACELP_SEARCH_RANGE_QUANTIZER_IDX (128)

#define MAX_FLT_VAL (3.402823466e+38F)
#define MIN_FLT_VAL (1.175494351e-38F)
#define MIN_SHRT_VAL (-32768)
#define MAX_SHRT_VAL (32767)

#define LAG_MIN (64)   // if 48k is max sr-- corresponding pitch_min/2
#define LAG_MAX (408)  // if 48k is max sr-- corresponding pitch_max/2
#define LEN_CORR_R (LAG_MAX - LAG_MIN + 1)

#define CODE_BOOK_ALPHA_LAV 121

#define MDST_TX_FLAG (0)
#define MDCT_TX_FLAG (1)

#define NO_SBR_CCFL_768 (0)
#define NO_SBR_CCFL_1024 (1)
#define SBR_8_3 (2)
#define SBR_2_1 (3)
#define SBR_4_1 (4)

#define USACE_MAX_SCR_SIZE (733836)
#define USACE_SCR_STACK (10 * 1024)
#define MAX_USAC_ESBR_BITRATE (96000)

#define MAX_PREROLL_FRAMES (3)
#define MAX_OUTPUT_BYTES_PER_CH (768)
#define MAXIMUM_VALUE_8BIT (255)
#define DEFAULT_RAP_INTERVAL_IN_MS (-1)
#define MIN_RAP_INTERVAL_IN_MS (1000)
#define MAX_PREROLL_CONFIG_SIZE (1024)
#define CC_NUM_PREROLL_FRAMES (1)
#define USAC_FIRST_FRAME_FLAG_DEFAULT_VALUE (1)
#define USAC_DEFAULT_DELAY_ADJUSTMENT_VALUE (1)
#define USAC_DEFAULT_MEASURED_LOUDNESS_FLAG_VALUE (1)
