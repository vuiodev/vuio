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
#ifndef IXHEAAC_SBR_CONST_H
#define IXHEAAC_SBR_CONST_H

#define SBR_AMPLITUDE_RESOLUTION_1_5 0
#define SBR_AMPLITUDE_RESOLUTION_3_0 1
#define NOISE_FLOOR_OFFSET_INT 6

#define LOW 0
#define HIGH 1

#define DTDF_DIR_TIME 1
#define DTDF_DIR_FREQ 0

#define SBR_CYC_REDCY_CHK_BITS 10

#define SBR_SAMP_FEQ_LVL_DEF 2
#define SBR_CHANGE_LVL_DEF 1
#define SBR_NOISE_BND_DEF 2

#define SBR_BND_LIMIT_DEF 2
#define SBR_GAIN_LIMIT_DEF 2
#define SBR_INTERPOL_SAMP_FEQ_DEF 1
#define SBR_SMOOTH_LEN_DEF 1

#define SBR_AMPLITUDE_RESOLUTION_BITS 1
#define SBR_BEGIN_SAMP_FREQ_BITS 4
#define SBR_END_SAMP_FREQ_BITS 4
#define SBR_CROSS_OVER_BND_BITS 3

#define ESBR_CROSS_OVER_BND_BITS 4
#define ESBR_PRE_FLAT_BITS 1
#define ESBR_PVC_MODE_BITS 2

#define ESBR_HEADER_EXTRA_3_BITS 1
#define PVC_MODE_BITS 2

#define SBR_HDR_EXTR_1_BITS 1
#define SBR_HDR_EXTR_2_BITS 1

#define SBR_SAMP_FREQ_LVL_BITS 2
#define SBR_CHANGE_LVL_BITS 1
#define SBR_NOISE_BND_BITS 2

#define SBR_BND_LIMIT_BITS 2
#define SBR_GAIN_LIMIT_BITS 2
#define SBR_INTERPOL_SAMP_FREQ_BITS 1
#define SBR_SMOOTH_LEN_BITS 1
#define SBR_STOCK_HE2_BITS 1
#define SBR_HDR_RESERV_BITS 2
#define SBR_SCE_RESERV_BITS 4

#define SBR_COUPLNG_MODE_BITS 1

#define SBR_INVERSE_FILT_MODE_BITS 2

#define SBR_ENLARGED_DATA_BITS 1
#define SBR_CONT_SIZE_BITS 4
#define SBR_CONT_ESC_CNT_BITS 8
#define SBR_CONT_ID_BITS 2

#define SBR_DEL_COD_DIR_BITS 1

#define SBR_ADD_SINE_FLAG_BITS 1

#define SBR_BEGIN_ENVN_BITS_AMPLITUDE_RESOLUTION_3_0 6
#define SBR_BEGIN_ENVN_BITS_BALNCE_AMPLITUDE_RESOLUTION_3_0 5
#define SBR_BEGIN_NOISE_BITS_AMPLITUDE_RESOLUTION_3_0 5
#define SBR_BEGIN_NOISE_BITS_BALNCE_AMPLITUDE_RESOLUTION_3_0 5

#define SBR_BEGIN_ENVN_BITS_AMPLITUDE_RESOLUTION_1_5 7
#define SBR_BEGIN_ENVN_BITS_BALNCE_AMPLITUDE_RESOLUTION_1_5 6

#define NOISE_FLOOR_OFFSET 6.0f

#define QMF_BUFFER_SIZE 64
#define TIMESLOT_BUFFER_SIZE 78

#define MAX_NUM_QMF_BANDS_ESBR 128

#define SBR_HF_ADJ_OFFSET 2
#define MPS_SBR_DELAY (6)

#define MPS_SBR_DELAY_960 5

#define ESBR_HBE_DELAY_OFFSET (32)

#define ESBR_HBE_DELAY_OFFSET_960 30

#define HBE_OPER_WIN_LEN (13)
#define NO_QMF_SYNTH_CHANNELS 64
#define TWICE_QMF_SYNTH_CHANNELS_NUM 128

#define MAX_NO_COLS_VALUE 64
#define MAX_NO_COLS_VALUE_BY_2 (MAX_NO_COLS_VALUE >> 1)

#define MAX_QMF_X_INBUF_SIZE MAX_NO_COLS_VALUE
#define MAX_QMF_X_OUTBUF_SIZE 2 * MAX_QMF_X_INBUF_SIZE

#define MAX_QMF_X_IN_REAL_BUF (NO_QMF_SYNTH_CHANNELS * MAX_QMF_X_INBUF_SIZE)
#define MAX_QMF_X_IN_IMAG_BUF (NO_QMF_SYNTH_CHANNELS * MAX_QMF_X_INBUF_SIZE)

#define MAX_QMF_X_OUT_REAL_BUF (NO_QMF_SYNTH_CHANNELS * MAX_QMF_X_OUTBUF_SIZE)
#define MAX_QMF_X_OUT_IMAG_BUF (NO_QMF_SYNTH_CHANNELS * MAX_QMF_X_OUTBUF_SIZE)

#define X_INBUF_SIZE (MAX_FRAME_SIZE + NO_QMF_SYNTH_CHANNELS)
#define X_OUTBUF_SIZE (X_INBUF_SIZE * 2)

#define HBE_OPER_BLK_LEN_2 10
#define HBE_OPER_BLK_LEN_3 8
#define HBE_OPER_BLK_LEN_4 6

#define MAX_HBE_PERSISTENT_SIZE                                                          \
  (MAX_QMF_X_INBUF_SIZE * sizeof(FLOAT32*) + MAX_QMF_X_OUTBUF_SIZE * sizeof(FLOAT32*) +  \
   MAX_QMF_X_IN_REAL_BUF * sizeof(FLOAT32) + MAX_QMF_X_IN_IMAG_BUF * sizeof(FLOAT32) +   \
   MAX_QMF_X_OUT_REAL_BUF * sizeof(FLOAT32) + MAX_QMF_X_OUT_IMAG_BUF * sizeof(FLOAT32) + \
   X_INBUF_SIZE * sizeof(FLOAT32) + X_OUTBUF_SIZE * sizeof(FLOAT32))

#define MAX_QMF_BUF_LEN 78

#define SBR_FRAME_CLASS_BITS 2
#define SBR_VAR_BORD_BITS 2
#define SBR_FRQ_RES_BITS 1
#define SBR_REL_BITS 2
#define SBR_ENV_BITS 2
#define SBR_NUM_BITS 2

#define FIXFIX 0
#define FIXVAR 1
#define VARFIX 2
#define VARVAR 3

#define LEN_NIBBLE (4)

#define PI 3.14159265358979323846264338327950288
#define EPS 1e-12f
#define LOG2 0.69314718056f

#define MAX_STRETCH 4
#define MAXDEG 3

#define EXP_FOR_SQRT 0.5f

#define SBR_HF_RELAXATION_PARAM 0.999999f

#define ESBR_PATCHING_MODE_BITS 1
#define ESBR_OVERSAMPLING_FLAG_BITS 1
#define ESBR_PITCHIN_FLAG_BITS 1
#define ESBR_PITCHIN_BINS_BITS 7
#define ESBR_RESERVED_PRESENT 1
#define ESBR_RESERVED_BITS_DATA 4
#define ESBR_INVF_MODE_BITS 2
#define ESBR_NOISE_MODE_BITS 1
#define ESBR_DOMAIN_BITS 1

#define SBR_NUM_QMF_BANDS 64
#define SBR_NUM_QMF_BANDS_2 32

#define PVC_NUM_TIME_SLOTS 16
#define PVC_ESG_MIN_VAL 0.1f

#define PVC_10LOG10_ESG_MIN_VAL -10.0f

#define PVC_DIV_MODE_BITS 3
#define PVC_NS_MODE_BITS 1
#define PVC_GRID_INFO_BITS 1
#define PVC_REUSE_PVC_ID_BITS 1
#define PVC_ID_BITS 7
#define PVC_NB_HIGH_MODE1 8
#define PVC_NB_HIGH_MODE2 6

#define PVC_NB_LOW 3
#define PVC_ID_NUM_GROUPS 3
#define PVC_NB_HIGH 128
#define PVC_ID_NBIT 7

#define ESC_SIN_POS 31
#define MAX_OCTAVE 29
#define MAX_SECOND_REGION 50

#define SBR_ENERGY_PAN_OFFSET 12
#define SBR_ENV_SF_MAX_VAL_1_5 70
#define MAX_NOISE_FLOOR_FAC_VAL 35
#define MIN_NOISE_FLOOR_FAC_VAL 0

#define HBE_ZERO_BAND_IDX 6

#define MAX_OV_DELAY 12

#define LD_TRAN 1
#define LD_ENV_TIME_SLOT 7
#define LD_ENV_TBL_512 16
#define LD_ENV_TBL_480 15
#define LD_ENV_TBL_SIZE 4
#define SBR_TRAN_BITS 4
#define SBRLD_CLA_BITS 1
#define SBR_ENVT_NUMENV 0
#define SBR_ENVT_TRANIDX 3

#define SBR_CLA_BITS 2
#define SBR_ABS_BITS 2
#define SBR_RES_BITS 1
#define SBR_REL_BITS 2
#define SBR_ENV_BITS 2
#define SBR_NUM_BITS 2

#define FIXFIX 0
#define FIXVAR 1
#define VARFIX 2
#define VARVAR 3

#define LEN_NIBBLE (4)

#endif /* IXHEAAC_SBR_CONST_H */
