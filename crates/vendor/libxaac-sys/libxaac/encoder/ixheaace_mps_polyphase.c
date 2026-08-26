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

#include <string.h>
#include <math.h>
#include "ixheaac_type_def.h"
#include "ixheaace_mps_common_fix.h"
#include "ixheaace_mps_defines.h"
#include "ixheaace_mps_common_define.h"
#include "ixheaace_bitbuffer.h"

#include "ixheaace_mps_struct_def.h"
#include "ixheaace_mps_sac_polyphase.h"
#include "ixheaace_mps_sac_hybfilter.h"
#include "ixheaace_mps_spatial_bitstream.h"
#include "ixheaace_mps_bitstream.h"
#include "ixheaace_mps_param_extract.h"
#include "ixheaace_mps_tree.h"
#include "ixheaace_mps_rom.h"

static VOID ixheaace_mps_bit_reversal(FLOAT32 *a, FLOAT32 *b) {
  FLOAT32 temp;
  temp = *a;
  *a = *b;
  *b = temp;
}

static VOID ixheaace_mps_bit_add_sub1(FLOAT32 *a, FLOAT32 *b) {
  FLOAT32 temp;
  temp = *a + *b;
  *b = *b - *a;
  *a = temp;
}

static VOID ixheaace_mps_bit_add_sub2(FLOAT32 *a, FLOAT32 *b) {
  FLOAT32 temp;
  temp = *a + *b;
  *b = *a - *b;
  *a = temp;
}

static VOID ixheaace_mps_fft32(FLOAT32 *subband) {
  FLOAT32 val_0, val_1, val_2, val_3, val_4, val_5, val_6, val_7, val_8, val_9, val_10, val_11,
      val_12, val_13, val_14, val_15;
  FLOAT32 val_1_0, val_1_1, val_1_2, val_1_3, val_1_4, val_1_5, val_1_6, val_1_7, val_1_8,
      val_1_9, val_1_10, val_1_11, val_1_12, val_1_13, val_1_14, val_1_15;
  FLOAT32 val_2_0, val_2_1, val_2_2, val_2_3, val_2_4, val_2_5, val_2_6, val_2_7, val_2_8,
      val_2_9, val_2_10, val_2_11, val_2_12, val_2_13, val_2_14, val_2_15;
  FLOAT32 val_3_0, val_3_1, val_3_2, val_3_3, val_3_4, val_3_5, val_3_6, val_3_7, val_3_8,
      val_3_9, val_3_10, val_3_11, val_3_12, val_3_13, val_3_14, val_3_15;
  FLOAT32 val_3_16, val_3_17, val_3_18, val_3_19, val_3_20, val_3_21, val_3_22, val_3_23,
      val_3_24, val_3_25, val_3_26, val_3_27, val_3_28, val_3_29, val_3_30, val_3_31;
  FLOAT32 val_4_0, val_4_1, val_4_2, val_4_3, val_4_4, val_4_5, val_4_6, val_4_7, val_4_8,
      val_4_9, val_4_10, val_4_11, val_4_12, val_4_13, val_4_14, val_4_15;

  val_2_0 = subband[2] - subband[34];
  val_2_1 = subband[3] - subband[35];
  val_3_0 = subband[0] + subband[32];
  val_3_1 = subband[1] + subband[33];
  val_3_2 = subband[2] + subband[34];
  val_3_3 = subband[3] + subband[35];

  val_2_2 = subband[6] - subband[38];
  val_2_3 = subband[7] - subband[39];
  val_3_4 = subband[4] + subband[36];
  val_3_5 = subband[5] + subband[37];
  val_3_6 = subband[6] + subband[38];
  val_3_7 = subband[7] + subband[39];

  val_2_4 = subband[10] - subband[42];
  val_2_5 = subband[11] - subband[43];
  val_3_8 = subband[8] + subband[40];
  val_3_9 = subband[9] + subband[41];
  val_3_10 = subband[10] + subband[42];
  val_3_11 = subband[11] + subband[43];

  val_2_6 = subband[14] - subband[46];
  val_2_7 = subband[15] - subband[47];
  val_3_12 = subband[12] + subband[44];
  val_3_13 = subband[13] + subband[45];
  val_3_14 = subband[14] + subband[46];
  val_3_15 = subband[15] + subband[47];

  val_2_8 = subband[18] - subband[50];
  val_2_9 = subband[19] - subband[51];
  val_3_16 = subband[16] + subband[48];
  val_3_17 = subband[17] + subband[49];
  val_3_18 = subband[18] + subband[50];
  val_3_19 = subband[19] + subband[51];

  val_2_10 = subband[22] - subband[54];
  val_2_11 = subband[23] - subband[55];
  val_3_20 = subband[20] + subband[52];
  val_3_21 = subband[21] + subband[53];
  val_3_22 = subband[22] + subband[54];
  val_3_23 = subband[23] + subband[55];

  val_2_12 = subband[26] - subband[58];
  val_2_13 = subband[27] - subband[59];
  val_3_24 = subband[24] + subband[56];
  val_3_25 = subband[25] + subband[57];
  val_3_26 = subband[26] + subband[58];
  val_3_27 = subband[27] + subband[59];

  val_2_14 = subband[30] - subband[62];
  val_2_15 = subband[31] - subband[63];
  val_3_28 = subband[28] + subband[60];
  val_3_29 = subband[29] + subband[61];
  val_3_30 = subband[30] + subband[62];
  val_3_31 = subband[31] + subband[63];

  val_4_1 = -(val_2_0 + val_2_14);
  val_4_2 = val_2_0 - val_2_14;
  val_4_0 = val_2_1 + val_2_15;
  val_4_3 = val_2_1 - val_2_15;
  val_4_5 = -(val_2_2 + val_2_12);
  val_4_6 = val_2_2 - val_2_12;
  val_4_4 = val_2_3 + val_2_13;
  val_4_7 = val_2_3 - val_2_13;
  val_4_9 = -(val_2_4 + val_2_10);
  val_4_10 = val_2_4 - val_2_10;
  val_4_8 = val_2_5 + val_2_11;
  val_4_11 = val_2_5 - val_2_11;
  val_4_13 = -(val_2_6 + val_2_8);
  val_4_14 = val_2_6 - val_2_8;
  val_4_12 = val_2_7 + val_2_9;
  val_4_15 = val_2_7 - val_2_9;

  val_2_0 = val_4_0 * fft_c[3] + val_4_4 * fft_c[2] + val_4_8 * fft_c[1] + val_4_12 * fft_c[0];

  val_2_4 = val_4_0 * fft_c[2] + val_4_4 * fft_c[0] + val_4_8 * fft_c[3] - val_4_12 * fft_c[1];
  val_2_8 = val_4_0 * fft_c[1] + val_4_4 * fft_c[3] - val_4_8 * fft_c[0] + val_4_12 * fft_c[2];

  val_2_12 = val_4_0 * fft_c[0] - val_4_4 * fft_c[1] + val_4_8 * fft_c[2] - val_4_12 * fft_c[3];

  val_2_1 = val_4_1 * fft_c[3] + val_4_5 * fft_c[2] + val_4_9 * fft_c[1] + val_4_13 * fft_c[0];

  val_2_5 = val_4_1 * fft_c[2] + val_4_5 * fft_c[0] + val_4_9 * fft_c[3] - val_4_13 * fft_c[1];
  val_2_9 = val_4_1 * fft_c[1] + val_4_5 * fft_c[3] - val_4_9 * fft_c[0] + val_4_13 * fft_c[2];

  val_2_13 = val_4_1 * fft_c[0] - val_4_5 * fft_c[1] + val_4_9 * fft_c[2] - val_4_13 * fft_c[3];

  val_2_2 = val_4_2 * fft_c[0] + val_4_6 * fft_c[1] + val_4_10 * fft_c[2] + val_4_14 * fft_c[3];

  val_2_6 = val_4_2 * fft_c[1] - val_4_6 * fft_c[3] - val_4_10 * fft_c[0] - val_4_14 * fft_c[2];

  val_2_10 = val_4_2 * fft_c[2] - val_4_6 * fft_c[0] + val_4_10 * fft_c[3] + val_4_14 * fft_c[1];

  val_2_14 = val_4_2 * fft_c[3] - val_4_6 * fft_c[2] + val_4_10 * fft_c[1] - val_4_14 * fft_c[0];

  val_2_3 = val_4_3 * fft_c[0] + val_4_7 * fft_c[1] + val_4_11 * fft_c[2] + val_4_15 * fft_c[3];

  val_2_7 = val_4_3 * fft_c[1] - val_4_7 * fft_c[3] - val_4_11 * fft_c[0] - val_4_15 * fft_c[2];

  val_2_11 = val_4_3 * fft_c[2] - val_4_7 * fft_c[0] + val_4_11 * fft_c[3] + val_4_15 * fft_c[1];

  val_2_15 = val_4_3 * fft_c[3] - val_4_7 * fft_c[2] + val_4_11 * fft_c[1] - val_4_15 * fft_c[0];

  val_4_0 = val_2_0 + val_2_2;
  val_4_14 = val_2_0 - val_2_2;
  val_4_1 = val_2_1 + val_2_3;
  val_4_15 = val_2_1 - val_2_3;
  val_4_2 = val_2_4 + val_2_6;
  val_4_12 = val_2_4 - val_2_6;
  val_4_3 = val_2_5 + val_2_7;
  val_4_13 = val_2_5 - val_2_7;
  val_4_4 = val_2_8 + val_2_10;
  val_4_10 = val_2_8 - val_2_10;
  val_4_5 = val_2_9 + val_2_11;
  val_4_11 = val_2_9 - val_2_11;
  val_4_6 = val_2_12 + val_2_14;
  val_4_8 = val_2_12 - val_2_14;
  val_4_7 = val_2_13 + val_2_15;
  val_4_9 = val_2_13 - val_2_15;

  val_1_0 = val_3_0 + val_3_16;
  val_1_1 = val_3_1 + val_3_17;
  val_1_2 = val_3_2 + val_3_18;
  val_1_3 = val_3_3 + val_3_19;
  val_1_4 = val_3_4 + val_3_20;
  val_1_5 = val_3_5 + val_3_21;
  val_1_6 = val_3_6 + val_3_22;
  val_1_7 = val_3_7 + val_3_23;
  val_1_8 = val_3_8 + val_3_24;
  val_1_9 = val_3_9 + val_3_25;
  val_1_10 = val_3_10 + val_3_26;
  val_1_11 = val_3_11 + val_3_27;
  val_1_12 = val_3_12 + val_3_28;
  val_1_13 = val_3_13 + val_3_29;
  val_1_14 = val_3_14 + val_3_30;
  val_1_15 = val_3_15 + val_3_31;

  val_0 = val_1_0 + val_1_8;
  val_2 = val_1_0 - val_1_8;
  val_1 = val_1_1 + val_1_9;
  val_3 = val_1_1 - val_1_9;
  val_4 = val_1_2 + val_1_10;
  val_6 = val_1_2 - val_1_10;
  val_5 = val_1_3 + val_1_11;
  val_7 = val_1_3 - val_1_11;
  val_8 = val_1_4 + val_1_12;
  val_10 = val_1_4 - val_1_12;
  val_9 = val_1_5 + val_1_13;
  val_11 = val_1_5 - val_1_13;
  val_12 = val_1_6 + val_1_14;
  val_14 = val_1_6 - val_1_14;
  val_13 = val_1_7 + val_1_15;
  val_15 = val_1_7 - val_1_15;

  val_2_0 = val_0 + val_8;
  val_2_4 = val_0 - val_8;
  val_2_1 = val_1 + val_9;
  val_2_5 = val_1 - val_9;
  val_2_8 = val_2 - val_11;
  val_2_10 = val_2 + val_11;
  val_2_9 = val_3 + val_10;
  val_2_11 = val_3 - val_10;
  val_2_2 = val_4 + val_12;
  val_2_7 = val_4 - val_12;
  val_2_3 = val_5 + val_13;
  val_2_6 = val_13 - val_5;

  val_1 = val_6 + val_14;
  val_2 = val_6 - val_14;
  val_0 = val_7 + val_15;
  val_3 = val_7 - val_15;

  val_2_12 = (val_0 + val_2) * MPS_INV_SQRT2;
  val_2_14 = (val_0 - val_2) * MPS_INV_SQRT2;
  val_2_13 = (val_3 - val_1) * MPS_INV_SQRT2;
  val_2_15 = (val_1 + val_3) * -MPS_INV_SQRT2;

  val_1_0 = val_3_0 - val_3_16;
  val_1_1 = val_3_1 - val_3_17;
  val_1_2 = val_3_2 - val_3_18;
  val_1_3 = val_3_3 - val_3_19;
  val_1_4 = val_3_4 - val_3_20;
  val_1_5 = val_3_5 - val_3_21;
  val_1_6 = val_3_6 - val_3_22;
  val_1_7 = val_3_7 - val_3_23;
  val_1_8 = val_3_8 - val_3_24;
  val_1_9 = val_3_9 - val_3_25;
  val_1_10 = val_3_10 - val_3_26;
  val_1_11 = val_3_11 - val_3_27;
  val_1_12 = val_3_12 - val_3_28;
  val_1_13 = val_3_13 - val_3_29;
  val_1_14 = val_3_14 - val_3_30;
  val_1_15 = val_3_15 - val_3_31;

  val_3_0 = val_2_0 + val_2_2;
  val_3_16 = val_2_0 - val_2_2;
  val_3_1 = val_2_1 + val_2_3;
  val_3_17 = val_2_1 - val_2_3;
  val_3_8 = val_2_4 - val_2_6;
  val_3_24 = val_2_4 + val_2_6;
  val_3_9 = val_2_5 - val_2_7;
  val_3_25 = val_2_5 + val_2_7;
  val_3_12 = val_2_8 + val_2_14;
  val_3_28 = val_2_8 - val_2_14;
  val_3_13 = val_2_9 + val_2_15;
  val_3_29 = val_2_9 - val_2_15;
  val_3_4 = val_2_10 + val_2_12;
  val_3_20 = val_2_10 - val_2_12;
  val_3_5 = val_2_11 + val_2_13;
  val_3_21 = val_2_11 - val_2_13;

  val_9 = (val_1_2 + val_1_14) * -MPS_COS_3PI_DIV8;
  val_10 = (val_1_2 - val_1_14) * MPS_COS_PI_DIV8;
  val_8 = (val_1_3 + val_1_15) * MPS_COS_3PI_DIV8;
  val_11 = (val_1_3 - val_1_15) * MPS_COS_PI_DIV8;
  val_5 = (val_1_4 + val_1_12) * -MPS_INV_SQRT2;
  val_6 = (val_1_4 - val_1_12) * MPS_INV_SQRT2;
  val_4 = (val_1_5 + val_1_13) * MPS_INV_SQRT2;
  val_7 = (val_1_5 - val_1_13) * MPS_INV_SQRT2;
  val_13 = (val_1_6 + val_1_10) * -MPS_COS_PI_DIV8;
  val_14 = (val_1_6 - val_1_10) * MPS_COS_3PI_DIV8;
  val_12 = (val_1_7 + val_1_11) * MPS_COS_PI_DIV8;
  val_15 = (val_1_7 - val_1_11) * MPS_COS_3PI_DIV8;

  val_1_2 = val_8 * MPS_SQRT2PLUS1 - val_12 * MPS_SQRT2MINUS1;
  val_1_3 = val_9 * MPS_SQRT2PLUS1 - val_13 * MPS_SQRT2MINUS1;
  val_1_4 = val_10 * MPS_SQRT2MINUS1 - val_14 * MPS_SQRT2PLUS1;
  val_1_5 = val_11 * MPS_SQRT2MINUS1 - val_15 * MPS_SQRT2PLUS1;

  val_8 += val_12;
  val_9 += val_13;
  val_10 += val_14;
  val_11 += val_15;
  val_1_6 = val_1_0 + val_4;
  val_1_10 = val_1_0 - val_4;
  val_1_7 = val_1_1 + val_5;
  val_1_11 = val_1_1 - val_5;

  val_1_12 = val_6 - val_1_9;
  val_1_14 = val_6 + val_1_9;
  val_1_13 = val_1_8 + val_7;
  val_1_15 = val_1_8 - val_7;

  val_0 = val_1_6 - val_1_14;
  val_2 = val_1_6 + val_1_14;
  val_1 = val_1_7 + val_1_15;
  val_3 = val_1_7 - val_1_15;
  val_4 = val_1_10 + val_1_12;
  val_6 = val_1_10 - val_1_12;
  val_5 = val_1_11 + val_1_13;
  val_7 = val_1_11 - val_1_13;

  val_1_10 = val_8 + val_10;
  val_10 = val_8 - val_10;
  val_1_11 = val_9 + val_11;
  val_11 = val_9 - val_11;

  val_12 = val_1_2 + val_1_4;
  val_14 = val_1_2 - val_1_4;
  val_13 = val_1_3 + val_1_5;
  val_15 = val_1_3 - val_1_5;

  val_3_2 = val_2 + val_1_10;
  val_3_18 = val_2 - val_1_10;
  val_3_3 = val_3 + val_1_11;
  val_3_19 = val_3 - val_1_11;
  val_3_6 = val_0 + val_12;
  val_3_22 = val_0 - val_12;
  val_3_7 = val_1 + val_13;
  val_3_23 = val_1 - val_13;
  val_3_14 = val_4 + val_10;
  val_3_30 = val_4 - val_10;
  val_3_15 = val_5 + val_11;
  val_3_31 = val_5 - val_11;
  val_3_10 = val_6 + val_14;
  val_3_26 = val_6 - val_14;
  val_3_11 = val_7 + val_15;
  val_3_27 = val_7 - val_15;

  val_1_0 = subband[0] - subband[32];
  val_1_1 = subband[1] - subband[33];
  val_1_2 = subband[4] - subband[36];
  val_1_3 = subband[5] - subband[37];
  val_1_4 = subband[8] - subband[40];
  val_1_5 = subband[9] - subband[41];
  val_1_6 = subband[12] - subband[44];
  val_1_7 = subband[13] - subband[45];
  val_1_8 = subband[16] - subband[48];
  val_1_9 = subband[17] - subband[49];
  val_1_10 = subband[20] - subband[52];
  val_1_11 = subband[21] - subband[53];
  val_1_12 = subband[24] - subband[56];
  val_1_13 = subband[25] - subband[57];
  val_1_14 = subband[28] - subband[60];
  val_1_15 = subband[29] - subband[61];

  val_9 = (val_1_2 + val_1_14) * -MPS_COS_3PI_DIV8;
  val_10 = (val_1_2 - val_1_14) * MPS_COS_PI_DIV8;
  val_8 = (val_1_3 + val_1_15) * MPS_COS_3PI_DIV8;
  val_11 = (val_1_3 - val_1_15) * MPS_COS_PI_DIV8;
  val_5 = (val_1_4 + val_1_12) * -MPS_INV_SQRT2;
  val_6 = (val_1_4 - val_1_12) * MPS_INV_SQRT2;
  val_4 = (val_1_5 + val_1_13) * MPS_INV_SQRT2;
  val_7 = (val_1_5 - val_1_13) * MPS_INV_SQRT2;
  val_13 = (val_1_6 + val_1_10) * -MPS_COS_PI_DIV8;
  val_14 = (val_1_6 - val_1_10) * MPS_COS_3PI_DIV8;
  val_12 = (val_1_7 + val_1_11) * MPS_COS_PI_DIV8;
  val_15 = (val_1_7 - val_1_11) * MPS_COS_3PI_DIV8;

  val_1_2 = val_8 * MPS_SQRT2PLUS1 - val_12 * MPS_SQRT2MINUS1;
  val_1_3 = val_9 * MPS_SQRT2PLUS1 - val_13 * MPS_SQRT2MINUS1;
  val_1_4 = val_10 * MPS_SQRT2MINUS1 - val_14 * MPS_SQRT2PLUS1;
  val_1_5 = val_11 * MPS_SQRT2MINUS1 - val_15 * MPS_SQRT2PLUS1;

  val_8 += val_12;
  val_9 += val_13;
  val_10 += val_14;
  val_11 += val_15;
  val_1_6 = val_1_0 + val_4;
  val_1_10 = val_1_0 - val_4;
  val_1_7 = val_1_1 + val_5;
  val_1_11 = val_1_1 - val_5;

  val_1_12 = val_6 - val_1_9;
  val_1_14 = val_6 + val_1_9;
  val_1_13 = val_1_8 + val_7;
  val_1_15 = val_1_8 - val_7;

  val_0 = val_1_6 - val_1_14;
  val_2 = val_1_6 + val_1_14;
  val_1 = val_1_7 + val_1_15;
  val_3 = val_1_7 - val_1_15;
  val_4 = val_1_10 + val_1_12;
  val_6 = val_1_10 - val_1_12;
  val_5 = val_1_11 + val_1_13;
  val_7 = val_1_11 - val_1_13;

  val_1_10 = val_8 + val_10;
  val_10 = val_8 - val_10;
  val_1_11 = val_9 + val_11;
  val_11 = val_9 - val_11;

  val_12 = val_1_2 + val_1_4;
  val_14 = val_1_2 - val_1_4;
  val_13 = val_1_3 + val_1_5;
  val_15 = val_1_3 - val_1_5;

  val_1_0 = val_2 + val_1_10;
  val_1_8 = val_2 - val_1_10;
  val_1_1 = val_3 + val_1_11;
  val_1_9 = val_3 - val_1_11;
  val_1_2 = val_0 + val_12;
  val_1_10 = val_0 - val_12;
  val_1_3 = val_1 + val_13;
  val_1_11 = val_1 - val_13;
  val_1_6 = val_4 + val_10;
  val_1_14 = val_4 - val_10;
  val_1_7 = val_5 + val_11;
  val_1_15 = val_5 - val_11;
  val_1_4 = val_6 + val_14;
  val_1_12 = val_6 - val_14;
  val_1_5 = val_7 + val_15;
  val_1_13 = val_7 - val_15;

  *subband++ = val_3_0;
  *subband++ = val_3_1;
  *subband++ = val_1_0 + val_4_0;
  *subband++ = val_1_1 + val_4_1;
  *subband++ = val_3_2;
  *subband++ = val_3_3;
  *subband++ = val_1_2 + val_4_2;
  *subband++ = val_1_3 + val_4_3;
  *subband++ = val_3_4;
  *subband++ = val_3_5;
  *subband++ = val_1_4 + val_4_4;
  *subband++ = val_1_5 + val_4_5;
  *subband++ = val_3_6;
  *subband++ = val_3_7;
  *subband++ = val_1_6 + val_4_6;
  *subband++ = val_1_7 + val_4_7;
  *subband++ = val_3_8;
  *subband++ = val_3_9;
  *subband++ = val_1_8 + val_4_8;
  *subband++ = val_1_9 + val_4_9;
  *subband++ = val_3_10;
  *subband++ = val_3_11;
  *subband++ = val_1_10 + val_4_10;
  *subband++ = val_1_11 + val_4_11;
  *subband++ = val_3_12;
  *subband++ = val_3_13;
  *subband++ = val_1_12 + val_4_12;
  *subband++ = val_1_13 + val_4_13;
  *subband++ = val_3_14;
  *subband++ = val_3_15;
  *subband++ = val_1_14 + val_4_14;
  *subband++ = val_1_15 + val_4_15;
  *subband++ = val_3_16;
  *subband++ = val_3_17;
  *subband++ = val_1_0 - val_4_0;
  *subband++ = val_1_1 - val_4_1;
  *subband++ = val_3_18;
  *subband++ = val_3_19;
  *subband++ = val_1_2 - val_4_2;
  *subband++ = val_1_3 - val_4_3;
  *subband++ = val_3_20;
  *subband++ = val_3_21;
  *subband++ = val_1_4 - val_4_4;
  *subband++ = val_1_5 - val_4_5;
  *subband++ = val_3_22;
  *subband++ = val_3_23;
  *subband++ = val_1_6 - val_4_6;
  *subband++ = val_1_7 - val_4_7;
  *subband++ = val_3_24;
  *subband++ = val_3_25;
  *subband++ = val_1_8 - val_4_8;
  *subband++ = val_1_9 - val_4_9;
  *subband++ = val_3_26;
  *subband++ = val_3_27;
  *subband++ = val_1_10 - val_4_10;
  *subband++ = val_1_11 - val_4_11;
  *subband++ = val_3_28;
  *subband++ = val_3_29;
  *subband++ = val_1_12 - val_4_12;
  *subband++ = val_1_13 - val_4_13;
  *subband++ = val_3_30;
  *subband++ = val_3_31;
  *subband++ = val_1_14 - val_4_14;
  *subband++ = val_1_15 - val_4_15;
}

static VOID ixheaace_mps_cos_mod(
    FLOAT32 *subband, ixheaace_mps_sac_pstr_qmf_synth_filter_bank pstr_qmf_synth_filter_bank) {
  WORD32 idx, length;
  FLOAT32 weight_imag, weight_real;
  FLOAT32 real_1, real_2;
  FLOAT32 imag_1, imag_2;
  FLOAT32 accu1, accu2;

  length = 32;

  for (idx = 0; idx < (length / 2); idx++) {
    real_1 = subband[2 * idx];
    imag_2 = subband[2 * idx + 1];
    real_2 = subband[2 * length - 2 - 2 * idx];
    imag_1 = subband[2 * length - 1 - 2 * idx];

    weight_imag = pstr_qmf_synth_filter_bank->sin_twiddle[idx];
    weight_real = pstr_qmf_synth_filter_bank->cos_twiddle[idx];

    accu1 = imag_1 * weight_imag + real_1 * weight_real;
    accu2 = imag_1 * weight_real - real_1 * weight_imag;

    subband[2 * idx] = accu1;
    subband[2 * idx + 1] = accu2;

    weight_imag = pstr_qmf_synth_filter_bank->sin_twiddle[length - 1 - idx];
    weight_real = pstr_qmf_synth_filter_bank->cos_twiddle[length - 1 - idx];

    accu1 = imag_2 * weight_imag + real_2 * weight_real;
    accu2 = imag_2 * weight_real - real_2 * weight_imag;

    subband[2 * length - 2 - 2 * idx] = accu1;
    subband[2 * length - 1 - 2 * idx] = accu2;
  }

  ixheaace_mps_fft32(subband);

  weight_imag = pstr_qmf_synth_filter_bank->alt_sin_twiddle[0];
  weight_real = pstr_qmf_synth_filter_bank->alt_sin_twiddle[length];

  for (idx = 0; idx < length / 2; idx++) {
    real_1 = subband[2 * idx];
    imag_1 = subband[2 * idx + 1];
    real_2 = subband[2 * length - 2 - 2 * idx];
    imag_2 = subband[2 * length - 1 - 2 * idx];

    accu1 = real_1 * weight_real + imag_1 * weight_imag;
    accu2 = real_1 * weight_imag - imag_1 * weight_real;

    subband[2 * idx] = accu1;
    subband[2 * length - 1 - 2 * idx] = accu2;

    weight_imag = pstr_qmf_synth_filter_bank->alt_sin_twiddle[idx + 1];
    weight_real = pstr_qmf_synth_filter_bank->alt_sin_twiddle[length - 1 - idx];

    accu1 = real_2 * weight_imag + imag_2 * weight_real;
    accu2 = real_2 * weight_real - imag_2 * weight_imag;

    subband[2 * length - 2 - 2 * idx] = accu1;
    subband[2 * idx + 1] = accu2;
  }
}

static VOID ixheaace_mps_sin_mod(
    FLOAT32 *subband, ixheaace_mps_sac_pstr_qmf_synth_filter_bank pstr_qmf_synth_filter_bank) {
  WORD32 idx, length;
  FLOAT32 weight_real, weight_imag;
  FLOAT32 real_1, imag_1, real_2, imag_2;
  FLOAT32 accu1, accu2;

  length = 32;

  for (idx = 0; idx < length / 2; idx++) {
    real_1 = subband[2 * idx];
    imag_2 = subband[2 * idx + 1];
    real_2 = subband[2 * length - 2 - 2 * idx];
    imag_1 = subband[2 * length - 1 - 2 * idx];

    weight_real = pstr_qmf_synth_filter_bank->sin_twiddle[idx];
    weight_imag = pstr_qmf_synth_filter_bank->cos_twiddle[idx];

    accu1 = imag_1 * weight_imag + real_1 * weight_real;
    accu2 = imag_1 * weight_real - real_1 * weight_imag;

    subband[2 * idx + 1] = accu1;
    subband[2 * idx] = accu2;

    weight_real = pstr_qmf_synth_filter_bank->sin_twiddle[length - 1 - idx];
    weight_imag = pstr_qmf_synth_filter_bank->cos_twiddle[length - 1 - idx];

    accu1 = imag_2 * weight_imag + real_2 * weight_real;
    accu2 = imag_2 * weight_real - real_2 * weight_imag;

    subband[2 * length - 1 - 2 * idx] = accu1;
    subband[2 * length - 2 - 2 * idx] = accu2;
  }

  ixheaace_mps_fft32(subband);

  weight_imag = pstr_qmf_synth_filter_bank->alt_sin_twiddle[0];
  weight_real = pstr_qmf_synth_filter_bank->alt_sin_twiddle[length];

  for (idx = 0; idx < length / 2; idx++) {
    real_1 = subband[2 * idx];
    imag_1 = subband[2 * idx + 1];
    real_2 = subband[2 * length - 2 - 2 * idx];
    imag_2 = subband[2 * length - 1 - 2 * idx];

    accu1 = -(real_1 * weight_real + imag_1 * weight_imag);
    accu2 = -(real_1 * weight_imag - imag_1 * weight_real);

    subband[2 * length - 1 - 2 * idx] = accu1;
    subband[2 * idx] = accu2;

    weight_imag = pstr_qmf_synth_filter_bank->alt_sin_twiddle[idx + 1];
    weight_real = pstr_qmf_synth_filter_bank->alt_sin_twiddle[length - 1 - idx];

    accu1 = -(real_2 * weight_imag + imag_2 * weight_real);
    accu2 = -(real_2 * weight_real - imag_2 * weight_imag);

    subband[2 * idx + 1] = accu1;
    subband[2 * length - 2 - 2 * idx] = accu2;
  }
}

static VOID ixheaace_mps_inverse_modulation(
    FLOAT32 *qmf_real, FLOAT32 *qmf_imag,
    ixheaace_mps_sac_pstr_qmf_synth_filter_bank pstr_qmf_synth_filter_bank) {
  WORD32 idx, no_synthesis_channels, length;

  FLOAT32 r1, i1, r2, i2;

  no_synthesis_channels = NUM_QMF_BANDS;

  length = no_synthesis_channels / 2;

  ixheaace_mps_cos_mod(qmf_real, pstr_qmf_synth_filter_bank);
  ixheaace_mps_sin_mod(qmf_imag, pstr_qmf_synth_filter_bank);

  for (idx = 0; idx < length; idx++) {
    r1 = qmf_real[idx];
    i2 = qmf_imag[no_synthesis_channels - 1 - idx];
    r2 = qmf_real[no_synthesis_channels - idx - 1];
    i1 = qmf_imag[idx];

    qmf_real[idx] = (r1 - i1);
    qmf_imag[no_synthesis_channels - 1 - idx] = -(r1 + i1);
    qmf_real[no_synthesis_channels - idx - 1] = (r2 - i2);
    qmf_imag[idx] = -(r2 + i2);
  }
}

static VOID ixheaace_mps_fct3_4(FLOAT32 *subband) {
  FLOAT32 val_00, val_01, val_10, val_11;

  subband[1] *= MPS_INV_SQRT2;

  val_00 = subband[0] + subband[1];
  val_01 = subband[0] - subband[1];

  val_10 = subband[2] * MPS_COS_6_PI_BY_16 + subband[3] * MPS_SIN_6_PI_BY_16;
  val_11 = subband[2] * MPS_SIN_6_PI_BY_16 - subband[3] * MPS_COS_6_PI_BY_16;

  subband[0] = val_00 + val_10;
  subband[3] = val_00 - val_10;
  subband[1] = val_01 + val_11;
  subband[2] = val_01 - val_11;
}

static VOID ixheaace_mps_fst3_4r(FLOAT32 *subband) {
  FLOAT32 val_00, val_01, val_10, val_11;

  subband[2] *= MPS_INV_SQRT2;

  val_00 = subband[3] + subband[2];
  val_01 = subband[3] - subband[2];

  val_10 = subband[1] * MPS_COS_6_PI_BY_16 + subband[0] * MPS_SIN_6_PI_BY_16;
  val_11 = subband[0] * MPS_COS_6_PI_BY_16 - subband[1] * MPS_SIN_6_PI_BY_16;

  subband[3] = val_00 + val_10;
  subband[0] = val_10 - val_00;
  subband[2] = val_11 - val_01;
  subband[1] = val_11 + val_01;
}

static VOID ixheaace_mps_fct4_4r(FLOAT32 *subband) {
  FLOAT32 val_00, val_01, val_10, val_11;
  subband[1] *= MPS_INV_SQRT2;

  val_00 = subband[0] + subband[1];
  val_01 = subband[0] - subband[1];

  subband[2] *= MPS_INV_SQRT2;

  val_11 = subband[3] - subband[2];
  val_10 = subband[3] + subband[2];

  subband[3] = val_00 * MPS_COS_PI_BY_16 + val_10 * MPS_SIN_PI_BY_16;
  subband[0] = val_00 * MPS_SIN_PI_BY_16 - val_10 * MPS_COS_PI_BY_16;

  subband[2] = val_01 * MPS_COS_3_PI_BY_16 - val_11 * MPS_SIN_3_PI_BY_16;
  subband[1] = val_01 * MPS_SIN_3_PI_BY_16 + val_11 * MPS_COS_3_PI_BY_16;
}

static VOID ixheaace_mps_fst4_4(FLOAT32 *subband) {
  FLOAT32 val_00, val_01, val_10, val_11;

  subband[1] *= MPS_INV_SQRT2;

  val_10 = subband[0] + subband[1];
  val_11 = subband[0] - subband[1];

  subband[2] *= MPS_INV_SQRT2;

  val_01 = subband[3] - subband[2];
  val_00 = subband[3] + subband[2];

  subband[0] = val_00 * MPS_COS_PI_BY_16 + val_10 * MPS_SIN_PI_BY_16;
  subband[3] = val_10 * MPS_COS_PI_BY_16 - val_00 * MPS_SIN_PI_BY_16;

  subband[1] = val_11 * MPS_SIN_3_PI_BY_16 - val_01 * MPS_COS_3_PI_BY_16;
  subband[2] = val_01 * MPS_SIN_3_PI_BY_16 + val_11 * MPS_COS_3_PI_BY_16;
}

static VOID ixheaace_mps_fct3_64(FLOAT32 *subband) {
  WORD32 idx;
  const FLOAT32 *t_ptr;
  FLOAT32 cos_val, sine_val;
  FLOAT32 xp;

  ixheaace_mps_bit_reversal(&subband[1], &subband[32]);
  ixheaace_mps_bit_reversal(&subband[2], &subband[16]);
  ixheaace_mps_bit_reversal(&subband[3], &subband[48]);
  ixheaace_mps_bit_reversal(&subband[4], &subband[8]);
  ixheaace_mps_bit_reversal(&subband[5], &subband[40]);
  ixheaace_mps_bit_reversal(&subband[6], &subband[24]);
  ixheaace_mps_bit_reversal(&subband[7], &subband[56]);
  ixheaace_mps_bit_reversal(&subband[9], &subband[36]);
  ixheaace_mps_bit_reversal(&subband[10], &subband[20]);
  ixheaace_mps_bit_reversal(&subband[11], &subband[52]);
  ixheaace_mps_bit_reversal(&subband[13], &subband[44]);
  ixheaace_mps_bit_reversal(&subband[14], &subband[28]);
  ixheaace_mps_bit_reversal(&subband[15], &subband[60]);
  ixheaace_mps_bit_reversal(&subband[17], &subband[34]);
  ixheaace_mps_bit_reversal(&subband[19], &subband[50]);
  ixheaace_mps_bit_reversal(&subband[21], &subband[42]);
  ixheaace_mps_bit_reversal(&subband[22], &subband[26]);
  ixheaace_mps_bit_reversal(&subband[23], &subband[58]);
  ixheaace_mps_bit_reversal(&subband[25], &subband[38]);
  ixheaace_mps_bit_reversal(&subband[27], &subband[54]);
  ixheaace_mps_bit_reversal(&subband[29], &subband[46]);
  ixheaace_mps_bit_reversal(&subband[31], &subband[62]);
  ixheaace_mps_bit_reversal(&subband[35], &subband[49]);
  ixheaace_mps_bit_reversal(&subband[37], &subband[41]);
  ixheaace_mps_bit_reversal(&subband[39], &subband[57]);
  ixheaace_mps_bit_reversal(&subband[43], &subband[53]);
  ixheaace_mps_bit_reversal(&subband[47], &subband[61]);
  ixheaace_mps_bit_reversal(&subband[55], &subband[59]);

  ixheaace_mps_bit_add_sub1(&subband[33], &subband[62]);
  ixheaace_mps_bit_add_sub1(&subband[34], &subband[60]);
  ixheaace_mps_bit_add_sub1(&subband[35], &subband[61]);
  ixheaace_mps_bit_add_sub1(&subband[36], &subband[56]);
  ixheaace_mps_bit_add_sub1(&subband[37], &subband[57]);
  ixheaace_mps_bit_add_sub1(&subband[38], &subband[58]);
  ixheaace_mps_bit_add_sub1(&subband[39], &subband[59]);
  ixheaace_mps_bit_add_sub1(&subband[40], &subband[48]);
  ixheaace_mps_bit_add_sub1(&subband[41], &subband[49]);
  ixheaace_mps_bit_add_sub1(&subband[42], &subband[50]);
  ixheaace_mps_bit_add_sub1(&subband[43], &subband[51]);
  ixheaace_mps_bit_add_sub1(&subband[44], &subband[52]);
  ixheaace_mps_bit_add_sub1(&subband[45], &subband[53]);
  ixheaace_mps_bit_add_sub1(&subband[46], &subband[54]);
  ixheaace_mps_bit_add_sub1(&subband[47], &subband[55]);
  ixheaace_mps_bit_add_sub1(&subband[17], &subband[30]);
  ixheaace_mps_bit_add_sub1(&subband[18], &subband[28]);
  ixheaace_mps_bit_add_sub1(&subband[19], &subband[29]);
  ixheaace_mps_bit_add_sub1(&subband[20], &subband[24]);
  ixheaace_mps_bit_add_sub1(&subband[21], &subband[25]);
  ixheaace_mps_bit_add_sub1(&subband[22], &subband[26]);
  ixheaace_mps_bit_add_sub1(&subband[23], &subband[27]);
  ixheaace_mps_bit_add_sub1(&subband[9], &subband[14]);
  ixheaace_mps_bit_add_sub1(&subband[10], &subband[12]);
  ixheaace_mps_bit_add_sub1(&subband[11], &subband[13]);
  ixheaace_mps_bit_add_sub1(&subband[41], &subband[46]);
  ixheaace_mps_bit_add_sub1(&subband[42], &subband[44]);
  ixheaace_mps_bit_add_sub1(&subband[43], &subband[45]);
  ixheaace_mps_bit_add_sub1(&subband[54], &subband[49]);
  ixheaace_mps_bit_add_sub1(&subband[52], &subband[50]);
  ixheaace_mps_bit_add_sub1(&subband[53], &subband[51]);
  ixheaace_mps_bit_add_sub1(&subband[5], &subband[6]);
  ixheaace_mps_bit_add_sub1(&subband[21], &subband[22]);
  ixheaace_mps_bit_add_sub1(&subband[26], &subband[25]);
  ixheaace_mps_bit_add_sub1(&subband[37], &subband[38]);
  ixheaace_mps_bit_add_sub1(&subband[58], &subband[57]);

  ixheaace_mps_fct3_4(subband);
  ixheaace_mps_fct4_4r(subband + 4);
  ixheaace_mps_fct3_4(subband + 8);
  ixheaace_mps_fst3_4r(subband + 12);
  ixheaace_mps_fct3_4(subband + 16);
  ixheaace_mps_fct4_4r(subband + 20);
  ixheaace_mps_fst4_4(subband + 24);
  ixheaace_mps_fst3_4r(subband + 28);
  ixheaace_mps_fct3_4(subband + 32);
  ixheaace_mps_fct4_4r(subband + 36);
  ixheaace_mps_fct3_4(subband + 40);
  ixheaace_mps_fst3_4r(subband + 44);
  ixheaace_mps_fct3_4(subband + 48);
  ixheaace_mps_fst3_4r(subband + 52);
  ixheaace_mps_fst4_4(subband + 56);
  ixheaace_mps_fst3_4r(subband + 60);

  for (idx = 0; idx < 4; idx++) {
    ixheaace_mps_bit_add_sub2(&subband[idx], &subband[7 - idx]);
    ixheaace_mps_bit_add_sub2(&subband[16 + idx], &subband[23 - idx]);
    ixheaace_mps_bit_add_sub1(&subband[31 - idx], &subband[24 + idx]);
    ixheaace_mps_bit_add_sub2(&subband[32 + idx], &subband[39 - idx]);
    ixheaace_mps_bit_add_sub1(&subband[63 - idx], &subband[56 + idx]);
  }
  t_ptr = trig_data_fct4_8;

  for (idx = 0; idx < 4; idx++) {
    cos_val = *t_ptr++;
    sine_val = *t_ptr++;

    xp = subband[8 + idx] * cos_val + subband[15 - idx] * sine_val;
    subband[8 + idx] = subband[8 + idx] * sine_val - subband[15 - idx] * cos_val;
    subband[15 - idx] = xp;
    xp = subband[40 + idx] * cos_val + subband[47 - idx] * sine_val;
    subband[40 + idx] = subband[40 + idx] * sine_val - subband[47 - idx] * cos_val;
    subband[47 - idx] = xp;
    xp = subband[48 + idx] * sine_val + subband[55 - idx] * cos_val;
    subband[55 - idx] = subband[48 + idx] * cos_val - subband[55 - idx] * sine_val;
    subband[48 + idx] = xp;
  }

  for (idx = 0; idx < 8; idx++) {
    ixheaace_mps_bit_add_sub2(&subband[idx], &subband[15 - idx]);
    ixheaace_mps_bit_add_sub2(&subband[32 + idx], &subband[47 - idx]);
    ixheaace_mps_bit_add_sub1(&subband[63 - idx], &subband[48 + idx]);
  }

  t_ptr = trig_data_fct4_16;

  for (idx = 0; idx < 8; idx++) {
    cos_val = *t_ptr++;
    sine_val = *t_ptr++;

    xp = subband[16 + idx] * cos_val + subband[31 - idx] * sine_val;
    subband[16 + idx] = subband[16 + idx] * sine_val - subband[31 - idx] * cos_val;
    subband[31 - idx] = xp;
  }

  for (idx = 0; idx < 16; idx++) {
    ixheaace_mps_bit_add_sub2(&subband[idx], &subband[31 - idx]);
  }

  t_ptr = trig_data_fct4_32;

  for (idx = 0; idx < 16; idx++) {
    cos_val = *t_ptr++;
    sine_val = *t_ptr++;

    xp = subband[32 + idx] * cos_val + subband[63 - idx] * sine_val;
    subband[32 + idx] = subband[32 + idx] * sine_val - subband[63 - idx] * cos_val;
    subband[63 - idx] = xp;
  }

  for (idx = 0; idx < 32; idx++) {
    ixheaace_mps_bit_add_sub2(&subband[idx], &subband[63 - idx]);
  }
}

static VOID ixheaace_mps_fst3_64(FLOAT32 *subband) {
  WORD32 k;

  for (k = 0; k < 32; k++) {
    ixheaace_mps_bit_reversal(&subband[k], &subband[63 - k]);
  }

  ixheaace_mps_fct3_64(subband);

  for (k = 1; k < NUM_QMF_BANDS; k += 2) {
    subband[k] = -subband[k];
  }
}

static VOID ixheaace_mps_forward_modulation(const FLOAT32 *time_in, FLOAT32 *real_subband,
                                            FLOAT32 *imag_subband) {
  WORD32 idx;
  real_subband[0] = time_in[0];

  for (idx = 1; idx < NUM_QMF_BANDS; idx++) {
    real_subband[idx] = time_in[idx] - time_in[(NUM_QMF_BANDS * 2) - idx];
    imag_subband[idx - 1] = time_in[idx] + time_in[(NUM_QMF_BANDS * 2) - idx];
  }

  imag_subband[63] = time_in[NUM_QMF_BANDS];

  ixheaace_mps_fct3_64(real_subband);

  ixheaace_mps_fst3_64(imag_subband);
}

VOID ixheaace_mps_515_calculate_sbr_syn_filterbank(
    FLOAT32 *real_subband, FLOAT32 *imag_subband, FLOAT32 *time_sig, WORD32 channel,
    ixheaace_mps_sac_pstr_qmf_synth_filter_bank pstr_qmf_synth_filter_bank, WORD32 slots,
    FLOAT32 *sbr_qmf_states_synthesis_per) {
  WORD32 idx, subband, slot;
  const FLOAT32 *ptr_filter;
  FLOAT32 accumlate;
  FLOAT32 *synth_buf;

  synth_buf = &(sbr_qmf_states_synthesis_per[channel * QMF_FILTER_STATE_SYN_SIZE]);

  for (slot = 0; slot < slots; slot++) {
    ptr_filter = pstr_qmf_synth_filter_bank->p_filter;
    ixheaace_mps_inverse_modulation(real_subband + NUM_QMF_BANDS * slot,
                                    imag_subband + NUM_QMF_BANDS * slot,
                                    pstr_qmf_synth_filter_bank);

    for (subband = 0; subband < NUM_QMF_BANDS; subband++) {
      real_subband[NUM_QMF_BANDS * slot + subband] =
          real_subband[NUM_QMF_BANDS * slot + subband] * (-0.015625f);
      imag_subband[NUM_QMF_BANDS * slot + subband] =
          imag_subband[NUM_QMF_BANDS * slot + subband] * (-0.015625f);
    }

    for (subband = 0; subband < NUM_QMF_BANDS; subband++) {
      FLOAT32 new_sample;
      new_sample = imag_subband[NUM_QMF_BANDS * slot + 63 - subband];

      for (idx = 0; idx < 5; idx++) {
        accumlate = synth_buf[2 * idx * NUM_QMF_BANDS + subband] + (*ptr_filter++) * new_sample;
        synth_buf[2 * idx * NUM_QMF_BANDS + subband] = accumlate;
      }
    }

    for (idx = 0; idx < 5; idx++) {
      accumlate = synth_buf[2 * idx * NUM_QMF_BANDS + NUM_QMF_BANDS + (NUM_QMF_BANDS - 1)] +
                  (*ptr_filter++) * real_subband[NUM_QMF_BANDS * slot];
      synth_buf[2 * idx * NUM_QMF_BANDS + NUM_QMF_BANDS + (NUM_QMF_BANDS - 1)] = accumlate;
    }
    time_sig[0] = accumlate;

    ptr_filter -= 10;

    for (subband = 0; subband < 63; subband++) {
      FLOAT32 new_sample;

      new_sample = real_subband[NUM_QMF_BANDS * slot + 63 - subband];

      for (idx = 0; idx < 5; idx++) {
        accumlate = synth_buf[2 * idx * NUM_QMF_BANDS + NUM_QMF_BANDS + subband] +
                    (*--ptr_filter) * new_sample;
        synth_buf[2 * idx * NUM_QMF_BANDS + NUM_QMF_BANDS + subband] = accumlate;
      }

      time_sig[NUM_QMF_BANDS - 1 - subband] = accumlate;
    }

    time_sig += NUM_QMF_BANDS;

    memmove((synth_buf + NUM_QMF_BANDS), synth_buf, (640 - NUM_QMF_BANDS) * sizeof(FLOAT32));

    memset((synth_buf), 0, NUM_QMF_BANDS * sizeof(FLOAT32));
  }
}

VOID ixheaace_mps_515_calculate_ana_filterbank(
    ixheaace_mps_sac_sbr_encoder_ana_filter_bank *pstr_sbr_enc_ana_filter_bank, FLOAT32 *time_sig,
    FLOAT32 *real_subband, FLOAT32 *imag_subband,
    ixheaace_mps_sac_pstr_qmf_ana_filter_bank pstr_qmf_synth_filter_bank) {
  WORD32 idx;
  FLOAT32 temp_array[128];
  const FLOAT32 *ptr_filter = pstr_qmf_synth_filter_bank->p_filter;
  FLOAT32 accumlate;

  for (idx = 0; idx < 576; idx++) {
    pstr_sbr_enc_ana_filter_bank->x[idx] = pstr_sbr_enc_ana_filter_bank->x[idx + NUM_QMF_BANDS];
  }
  for (idx = 0; idx < NUM_QMF_BANDS; idx++) {
    pstr_sbr_enc_ana_filter_bank->x[idx + 576] = time_sig[idx];
  }

  for (idx = 0; idx < NUM_QMF_BANDS; idx++) {
    accumlate = 0.0f;
    accumlate += (*ptr_filter++ * pstr_sbr_enc_ana_filter_bank->x[idx]);
    accumlate += (*ptr_filter++ * pstr_sbr_enc_ana_filter_bank->x[idx + (NUM_QMF_BANDS * 2)]);
    accumlate += (*ptr_filter++ * pstr_sbr_enc_ana_filter_bank->x[idx + (NUM_QMF_BANDS * 4)]);
    accumlate += (*ptr_filter++ * pstr_sbr_enc_ana_filter_bank->x[idx + (NUM_QMF_BANDS * 6)]);
    accumlate += (*ptr_filter++ * pstr_sbr_enc_ana_filter_bank->x[idx + (NUM_QMF_BANDS * 8)]);
    temp_array[127 - idx] = accumlate;
  }

  accumlate = 0.0f;
  for (idx = 0; idx < 5; idx++) {
    accumlate +=
        (*ptr_filter++ * pstr_sbr_enc_ana_filter_bank->x[127 + idx * (NUM_QMF_BANDS * 2)]);
  }
  temp_array[0] = accumlate;

  ptr_filter -= 10;
  for (idx = 0; idx < 63; idx++) {
    accumlate = 0.0f;
    accumlate += (*--ptr_filter * pstr_sbr_enc_ana_filter_bank->x[NUM_QMF_BANDS + idx]);
    accumlate += (*--ptr_filter *
                  pstr_sbr_enc_ana_filter_bank->x[NUM_QMF_BANDS + idx + (NUM_QMF_BANDS * 2)]);
    accumlate += (*--ptr_filter *
                  pstr_sbr_enc_ana_filter_bank->x[NUM_QMF_BANDS + idx + (NUM_QMF_BANDS * 4)]);
    accumlate += (*--ptr_filter *
                  pstr_sbr_enc_ana_filter_bank->x[NUM_QMF_BANDS + idx + (NUM_QMF_BANDS * 6)]);
    accumlate += (*--ptr_filter *
                  pstr_sbr_enc_ana_filter_bank->x[NUM_QMF_BANDS + idx + (NUM_QMF_BANDS * 8)]);
    temp_array[63 - idx] = accumlate;
  }

  ixheaace_mps_forward_modulation(temp_array, real_subband, imag_subband);
}
