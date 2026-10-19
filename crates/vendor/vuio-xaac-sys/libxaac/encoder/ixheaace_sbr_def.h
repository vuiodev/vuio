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

#define IXHEAACE_MAX_CH_IN_BS_ELE (2)

#define INPUT_LEN_DOWNSAMPLE (2048)
#define UPSAMPLE_FAC (3)
#define DOWNSAMPLE_FAC_2_1 (2)
#define DOWNSAMPLE_FAC_4_1 (4)

/* Constants */
#define MAX_FLT_VAL (3.402823466e+38F)
#define MIN_FLT_VAL (1.175494351e-38F)

#define EPS (1e-18)
#define LOG2 (0.69314718056f)
#define RELAXATION (1e-6f)

#define SBR_NOISE_FLOOR_OFFSET (6)
#define SBR_INV_LOG_2 (1.442695041f)
#define SBR_EPS (1e-18)

#define SBR_DECAY_GUIDE_DIFF (0.5f)
#define SBR_THR_DIFF_GUIDE (1.26f)
#define SBR_THR_DIFF (25.0f)
#define SBR_THR_TONE (15.0f)
#define SBR_INV_THR_TONE (1.0f / 15.0f)
#define SBR_THR_TONE_GUIDE (1.26f)
#define SBR_DECAY_GUIDE_ORIG (0.3f)
#define SBR_THR_SFM_SBR (0.3f)
#define SBR_THR_SFM_ORG (0.1f)
#define SBR_MAX_COMP (2)
#define SBR_TONALITY_QUOTA (0.1f)
#define SBR_DIFF_QUOTA (0.75f)
#define SBR_TON_MEAN_P0009 (0.000976562f)
#define SBR_TON_MEAN_101P59 (101.5936673f)

#define MAXIMUM_NOISE_ENVELOPES (2)
#define MAXIMUM_NUM_NOISE_COEFFS (5)
#define MAXIMUM_NUM_NOISE_VALUES (MAXIMUM_NOISE_ENVELOPES * MAXIMUM_NUM_NOISE_COEFFS)
#define MAXIMUM_ENVELOPES_HEAAC (5)
#define MAXIMUM_FREQ_COEFFS_HEAAC (48)
#define MAXIMUM_FREQ_COEFFS_LE32KHZ (48)
#define MAXIMUM_FREQ_COEFFS_EQ44KHZ (35)
#define MAXIMUM_FREQ_COEFFS_GE48KHZ (32)
#define MAXIMUM_NUM_ENV_VALUES_HEAAC (MAXIMUM_FREQ_COEFFS_HEAAC * MAXIMUM_ENVELOPES_HEAAC)

#define IXHEAACE_QMF_CHANNELS (64)
#define QMF_FILTER_LENGTH (640)
#define CLD_FILTER_LENGTH (640)
#define IXHEAACE_QMF_TIME_SLOTS (32)

#define NO_OF_ESTIMATES (4)
#define NO_OF_ESTIMATES_ELD (3)

#define QMF_TIME_SLOTS_USAC_4_1 (64)
#define MAX_QMF_TIME_SLOTS (64)
#define MAXIMUM_FREQ_COEFFS_USAC (56)
#define MAXIMUM_ENVELOPES_USAC (8)
#define MAXIMUM_NUM_ENV_VALUES_USAC (MAXIMUM_FREQ_COEFFS_USAC * MAXIMUM_ENVELOPES_USAC)

#if MAXIMUM_FREQ_COEFFS_HEAAC > MAXIMUM_FREQ_COEFFS_USAC
#define MAXIMUM_FREQ_COEFFS (MAXIMUM_FREQ_COEFFS_HEAAC)
#else
#define MAXIMUM_FREQ_COEFFS (MAXIMUM_FREQ_COEFFS_USAC)
#endif
#if MAXIMUM_ENVELOPES_HEAAC > MAXIMUM_ENVELOPES_USAC
#define IXHEAACE_MAX_ENV (MAXIMUM_ENVELOPES_HEAAC)
#else
#define IXHEAACE_MAX_ENV (MAXIMUM_ENVELOPES_USAC)
#endif

#if MAXIMUM_NUM_ENV_VALUES_HEAAC > MAXIMUM_NUM_ENV_VALUES_HEAAC
#define MAXIMUM_NUM_ENVELOPE_VALUES (MAXIMUM_NUM_ENV_VALUES_HEAAC)
#else
#define MAXIMUM_NUM_ENVELOPE_VALUES (MAXIMUM_NUM_ENV_VALUES_USAC)
#endif

#define LOW_RES (0)
#define HIGH_RES (1)

#define LO (0)
#define HI (1)

#define SI_SBR_PROTOCOL_VERSION_ID (0)

#define SBR_XPOS_CTRL_DEFAULT (2)

#define SBR_FREQ_SCALE_DEFAULT (2)
#define SBR_ALTER_SCALE_DEFAULT (1)
#define SBR_NOISE_BANDS_DEFAULT (2)

#define SBR_LIMITER_BANDS_DEFAULT (2)
#define SBR_LIMITER_GAINS_DEFAULT (2)
#define SBR_INTERPOL_FREQ_DEFAULT (1)
#define SBR_SMOOTHING_LENGTH_DEFAULT (0)

/* ESBR resampler Size*/
#define ESBR_RESAMP_SAMPLES (4096)

/* spectral_band_replication_header */
#define SI_SBR_AMP_RES_BITS (1)
#define SI_SBR_COUPLING_BITS (1)
#define SI_SBR_START_FREQ_BITS (4)
#define SI_SBR_STOP_FREQ_BITS (4)
#define SI_SBR_XOVER_BAND_BITS (3)
#define SI_SBR_RESERVED_BITS (2)
#define SI_SBR_HEADER_EXTRA_1_BITS (1)
#define SI_SBR_HEADER_EXTRA_2_BITS (1)
#define SI_SBR_FREQ_SCALE_BITS (2)
#define SI_SBR_ALTER_SCALE_BITS (1)
#define SI_SBR_NOISE_BANDS_BITS (2)

#define SBR_START_FREQ_OFFSET_TBL_LEN (1 << SI_SBR_START_FREQ_BITS)
#define SBR_STOP_FREQ_OFFSET_TBL_LEN (14)

#define SI_SBR_LIMITER_BANDS_BITS (2)
#define SI_SBR_LIMITER_GAINS_BITS (2)
#define SI_SBR_INTERPOL_FREQ_BITS (1)
#define SI_SBR_SMOOTHING_LENGTH_BITS (1)

/* spectral_band_replication_grid */
#define SBR_CLA_BITS (2)
#define SBR_ENV_BITS (2)
#define SBR_PVC_NOISE_POSITION_BITS (4)
#define SBR_PVC_VAR_LEN_HF_BITS (1)

#define SBR_ABS_BITS (2)
#define SBR_NUM_BITS (2)
#define SBR_REL_BITS (2)
#define SBR_RES_BITS (1)
#define SBR_DIR_BITS (1)
#define LDSBR_CLA_BITS (1)

/* spectral_band_replication_data */

#define SI_SBR_INVF_MODE_BITS (2)

#define SI_SBR_START_ENV_BITS_AMP_RES_3_0 (6)
#define SI_SBR_START_ENV_BITS_BALANCE_AMP_RES_3_0 (5)
#define SI_SBR_START_NOISE_BITS_AMP_RES_3_0 (5)
#define SI_SBR_START_NOISE_BITS_BALANCE_AMP_RES_3_0 (5)

#define SI_SBR_START_ENV_BITS_AMP_RES_1_5 (7)
#define SI_SBR_START_ENV_BITS_BALANCE_AMP_RES_1_5 (6)

#define SI_SBR_EXTENDED_DATA_BITS (1)
#define SI_SBR_EXTENSION_SIZE_BITS (4)
#define SI_SBR_EXTENSION_ESC_COUNT_BITS (8)
#define SI_SBR_EXTENSION_ID_BITS (2)

#define SBR_EXTENDED_DATA_MAX_CNT (15 + 255)

#define EXTENSION_ID_PS_CODING (2)
#define EXTENSION_ID_ESBR_CODING (3)
#define IXHEAACE_MPS_EXT_LDSAC_DATA (0x09)
/* Envelope coding constants */
#define FREQ (0)
#define TIME (1)

/* huffman tables */
#define CODE_BCK_SCF_LAV10 (60)
#define CODE_BCK_SCF_LAV11 (31)
#define CODE_BCK_SCF_LAV_BALANCE11 (12)
#define CODE_BCK_SCF_LAV_BALANCE10 (24)

#define USAC_SBR_RATIO_NO_SBR (0)
#define USAC_SBR_RATIO_INDEX_2_1 (3)
#define USAC_SBR_RATIO_INDEX_8_3 (2)
#define USAC_SBR_RATIO_INDEX_4_1 (1)

#define USAC_SBR_DOWNSAMPLE_RATIO_2_1 (2)
#define USAC_SBR_DOWNSAMPLE_RATIO_4_1 (4)

#define IXHEAACE_DISTANCE_CEIL_VALUE (5000000)

typedef enum {
  IXHEAACE_XPOS_MDCT,
  IXHEAACE_XPOS_MDCT_CROSS,
  IXHEAACE_XPOS_LC,
  IXHEAACE_XPOS_RESERVED,
  IXHEAACE_XPOS_SWITCHED
} ixheaace_sbr_xpos_mode;

typedef enum { FREQ_RES_LOW, FREQ_RES_HIGH } ixheaace_freq_res;

typedef enum {
  IXHEAACE_SBR_MODE_MONO,
  IXHEAACE_SBR_MODE_LEFT_RIGHT,
  SBR_COUPLING,
  IXHEAACE_SBR_MODE_SWITCH_LRC
} ixheaace_sbr_stereo_mode;

typedef enum { HEAAC_SBR, ELD_SBR, USAC_SBR } ixheaace_sbr_codec_type;
