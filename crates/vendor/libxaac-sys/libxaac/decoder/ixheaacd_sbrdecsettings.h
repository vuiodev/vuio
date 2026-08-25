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
#ifndef IXHEAACD_SBRDECSETTINGS_H
#define IXHEAACD_SBRDECSETTINGS_H

#define SBR_UPSAMPLE_FAC 2

#define SBR_UPSAMPLE_IDX_0_0 0
#define SBR_UPSAMPLE_IDX_2_1 1
#define SBR_UPSAMPLE_IDX_8_3 2
#define SBR_UPSAMPLE_IDX_4_1 3

#define MAX_FRAME_SIZE 1024

#define QMF_FILTER_STATE_SYN_SIZE 1280
#define QMF_FILTER_STATE_ANA_SIZE 320
#define NO_SYNTHESIS_CHANNELS 64
#define NO_ANALYSIS_CHANNELS (NO_SYNTHESIS_CHANNELS / SBR_UPSAMPLE_FAC)
#define NO_POLY (QMF_FILTER_STATE_ANA_SIZE / (2 * NO_ANALYSIS_CHANNELS))

#define NO_SYNTHESIS_CHANNELS_DOWN_SAMPLED (NO_SYNTHESIS_CHANNELS / 2)
#define QMF_FILTER_STATE_SYN_SIZE_DOWN_SAMPLED (QMF_FILTER_STATE_SYN_SIZE / 2)
#define NO_SYN_ANA_CHANNELS (NO_SYNTHESIS_CHANNELS - NO_ANALYSIS_CHANNELS)

#define CALC_STOP_BAND

#define MAX_NOISE_ENVELOPES 2
#define MAX_NOISE_COEFFS 5
#define MAX_NUM_NOISE_VALUES (MAX_NOISE_ENVELOPES * MAX_NOISE_COEFFS)
#define MAX_NUM_LIMITERS 12

#define MAX_ENVELOPES_SBR 5
#define MAX_ENVELOPES 8
#define MAX_FREQ_COEFFS 56

#define MAX_FREQ_COEFFS_FS44100 35
#define MAX_FREQ_COEFFS_FS48000 32
#define MAX_FREQ_COEFFS_SBR 48

#define MAX_NUM_ELEMENTS 16

#define SBR_TIME_STEP 2

#define MAX_NUM_ENVELOPE_VALUES (MAX_ENVELOPES * MAX_FREQ_COEFFS)

#define MAX_GAIN_EXP 34

#define LPC_ORDER 2

#define MAX_INVF_BANDS MAX_NOISE_COEFFS

#define MAX_COLS (MAX_FRAME_SIZE / NO_ANALYSIS_CHANNELS)
#define MAX_OV_COLS 6
#define MAX_ENV_COLS (MAX_COLS + MAX_OV_COLS)

#define MAX_ENV_COLS_960 (30 + MAX_OV_COLS)

#define SBR_FREQ_SCALE_DEFAULT 2
#define SBR_ALTER_SCALE_DEFAULT 1
#define SBR_NOISE_BANDS_DEFAULT 2

#define SBR_LIMITER_BANDS_DEFAULT 2
#define SBR_LIMITER_GAINS_DEFAULT 2
#define SBR_INTERPOL_FREQ_DEFAULT 1
#define SBR_SMOOTHING_LENGTH_DEFAULT 1

#define SBR_TIME_SLOTS (MAX_COLS / SBR_TIME_STEP)
#define SBR_OV_SLOTS (MAX_OV_COLS / SBR_TIME_STEP)

#endif
