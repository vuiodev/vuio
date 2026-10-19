/******************************************************************************
 *
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
#ifndef IXHEAACD_MPS_MACRO_DEF_H
#define IXHEAACD_MPS_MACRO_DEF_H

#define SQRT_TWO_Q15 (46341)
#define SQRT_THREE_Q15 (56784)

#define ONE_BY_THREE_Q15 (10923)
#define TWO_BY_THREE_Q15 (21845)
#define MINUS_ONE_BY_THREE_Q15 (-10923)
#define ONE_BY_SQRT_2_Q15 (23170)
#define ONE_BY_SQRT_8_Q15 (11585)
#define ONE_BY_FIVE_Q16 (13107)
#define TWO_PI_IN_Q15 (205887)
#define MINUS_PI_BY_EIGHT_Q15 (-12868)
#define TWO_PI_BY_FIFTEEN_Q15 (13726)
#define THIRTYONE_BY_TWO_IN_Q24 (260046848)

#define MINUS_ONE_IN_Q15 (-32768)
#define MINUS_ONE_IN_Q14 (-16384)
#define ONE_IN_Q13 (8192)
#define ONE_IN_Q14 (16384)
#define ONE_IN_Q15 (32768)
#define ONE_IN_Q16 (65536)
#define ONE_IN_Q30 (1073741824)
#define FOUR_IN_Q15 (131072)
#define ONE_BY_SQRT_3_Q15 (18919)
#define ONE_BY_NINE_Q16 (7282)
#define MINUS_SQRT_2_Q30 (-1518500250)
#define THIRTY_IN_Q16 (1966080)

#define ONE_IN_Q24 (16777216)
#define ONE_IN_Q27 (134217728)
#define ONE_IN_Q28 (268435456)

#define ONE_FORTYNINE_Q15 (4882432)

#define MINUS_POINT_NINE_EIGHT_Q15 (-32113)

#define MINUS_POINT_NINE_NINE_Q15 (-32440)

#define POINT_THREE_THREE_Q15 (10813)

#define POINT_EIGHT_Q15 (26214)

#define ONE_BY_SQRT_TWO_Q30 (759250125)

#define POINT_FOUR_TWO_Q15 (13763)

#define POINT_ONE_Q15 (3277)

#define POINT_NINE_Q15 (29491)

#define SUM_SIZE (2048)

#define RES_CHXQMFXTSX4 (184320)

#define RES_CHXQMFXTS (46080)

#define NR_QMF_BANDS_LFXTS (216)

#define QMF_BANDSXTSX6 (27648)

#define LOOP_COUNTER (32)

#define MAX_PSXPB (224)

#define SYN_BUFFER_SIZE (18432)

#define QBXTSX2 (9216)

#define PARAMETER_BANDSX2 (56)

#define PARAMETER_BANDSX3 (84)

#define PARAMETER_BANDSX1_5 (42)

#define QMF_BANDSX8 (512)

#define QBXTS (4608)

#define QBXTSX3 (13824)

#define TWO_BY_TWENTYFIVE_Q16 (5243)

#define QBX48 (3072)

#define TSXHB (5112)

#define MAX_TIMESLOTSX2 (144)

#define RESHAPE_OFFSET_1 (18)

#define RESHAPE_OFFSET_2 (54)

#define PB_OFFSET (9)

#define SCRATCH_OFFSET_SMOOTHING (176)

#define MAX_TIME_SLOTSX12 (864)

#define INPUT_CHX2 (12)

#define INPUT_CHX1_5 (9)

#define OUTPUT_CHX1_5 (20)

#define OUTPUT_CHX3 (41)

#define IN_CH_2XOUT_CH (32)

#define IN_CHXBP_SIZE (150)

#define OUT_CHXQB (512)

#define MAX_PARAMETER_BANDS_PLUS_1 (29)

#define PARAMETER_BANDSX52 (1456)

#define PARAMETER_BANDSX16 (448)

#define PARAMETER_BANDSX18 (504)

#define PARAMETER_BANDSX15 (420)

#define PARAMETER_BANDSX32 (896)

#define PARAMETER_BANDSX12 (336)

#define PARAMETER_BANDSX56 (1568)

#define PARAMETER_BANDSX24 (672)

#define PREV_GAINAT (5824)

#define ARBDMX_ALPHA (24)

#define M1_PREV (5376)

#define M2_PREV_RESID (2128)

#define M2_PREV_DECOR (1680)

#define QMF_DELAY_INPUT (7680)

#define ANA_BUF_SIZE (15360)

#define SYN_BUF_SIZE (18432 + sizeof(ia_mps_dec_synthesis_interface))

#define PBXPS (224)

#define MAX_NUM_DEN_LENGTH (21)

#define HYB_FILTER_STATE_SIZE sizeof(ia_mps_dec_thyb_filter_state_struct) * 16

#define TONALITY_STATE_SIZE sizeof(ia_mps_dec_tonality_state_struct)

#define SMOOTHING_STATE_SIZE sizeof(ia_mps_dec_smoothing_state_struct)

#define RESHAPE_STATE_SIZE sizeof(ia_mps_dec_reshape_bb_env_state_struct)

#define SUBBAND_TP_SIZE sizeof(ia_mps_dec_subband_tp_params_struct)

#define BLIND_DECODER_SIZE sizeof(ia_mps_dec_blind_decoder_struct)

#define ARRAY_STRUCT_SIZE sizeof(ia_mps_dec_reuse_array_struct)

#define QMF_RES_BUF_SIZE (368640)

#define QMF_BUF_SIZE (110592)

#define BUF_SIZE (163584)

#define MDCT_RES_BUF_SIZE (327680)

#define PCXQB (320)

#define RFX2XMDCTCOEF (8192)

#define MDCTCOEFX2 (2048)

#define TSXHBX5 (25560)

#define INCHXPBXPS (1344)

#define ONE_BIT_MASK (0x00000001)
#define TWO_BIT_MASK (0x00000003)
#define THREE_BIT_MASK (0x00000007)
#define FOUR_BIT_MASK (0x0000000F)
#define FIVE_BIT_MASK (0x0000001F)
#define SIX_BIT_MASK (0x0000003F)
#define SEVEN_BIT_MASK (0x0000007F)
#define WORD_LENGTH (32)

#define COS_PI_BY_8 (0x7642)
#define SIN_PI_BY_8 (0x30fc)

#endif /* IXHEAACD_MPS_MACRO_DEF_H */
