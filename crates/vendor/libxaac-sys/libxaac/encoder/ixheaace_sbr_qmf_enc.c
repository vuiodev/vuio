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
#include <stdlib.h>

#include "ixheaac_type_def.h"
#include "ixheaac_constants.h"
#include "ixheaace_sbr_def.h"
#include "ixheaace_resampler.h"
#include "ixheaace_sbr_rom.h"
#include "ixheaace_common_rom.h"
#include "ixheaace_sbr_hbe.h"
#include "ixheaace_sbr_qmf_enc.h"
#include "ixheaace_sbr_hybrid.h"

#include "ixheaace_aac_constants.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops16.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaac_basic_ops.h"

#include "ixheaace_psy_const.h"
#include "ixheaace_tns.h"
#include "ixheaace_tns_params.h"
#include "ixheaace_rom.h"
#include "ixheaace_fft.h"

static VOID ia_enhaacplus_enc_fct3_4(FLOAT32 *ptr_x) {
  FLOAT32 tmp00, tmp01, tmp10, tmp11, xp, xp2;

  xp2 = ptr_x[1];
  xp = ptr_x[0];

  xp2 = xp2 * IXHEAACE_COS_PI_BY_4;

  tmp00 = xp + xp2;
  tmp01 = xp - xp2;

  xp = ptr_x[2];
  xp2 = ptr_x[3];

  tmp10 = (xp * IXHEAACE_COS_PI_BY_8) + (xp2 * IXHEAACE_SIN_PI_BY_8);

  ptr_x[0] = tmp00 + tmp10;
  ptr_x[3] = tmp00 - tmp10;

  tmp11 = (xp * IXHEAACE_SIN_PI_BY_8) - (xp2 * IXHEAACE_COS_PI_BY_8);

  ptr_x[1] = tmp01 + tmp11;
  ptr_x[2] = tmp01 - tmp11;
}

static VOID ia_enhaacplus_enc_fst3_4r(FLOAT32 *ptr_x) {
  FLOAT32 tmp00, tmp01, tmp10, tmp11, xp, xp2;

  xp2 = ptr_x[2];
  xp = ptr_x[3];

  xp2 = xp2 * IXHEAACE_COS_PI_BY_4;

  tmp00 = xp + xp2;
  tmp01 = xp - xp2;

  xp = ptr_x[1];
  xp2 = ptr_x[0];

  tmp10 = (xp * IXHEAACE_COS_PI_BY_8) + (xp2 * IXHEAACE_SIN_PI_BY_8);

  ptr_x[3] = tmp00 + tmp10;
  ptr_x[0] = tmp10 - tmp00;

  tmp11 = (xp2 * IXHEAACE_COS_PI_BY_8) - (xp * IXHEAACE_SIN_PI_BY_8);

  ptr_x[2] = tmp11 - tmp01;
  ptr_x[1] = tmp11 + tmp01;
}

static VOID ia_enhaacplus_enc_fct4_4r(FLOAT32 *ptr_x) {
  FLOAT32 tmp00, tmp01, tmp10, tmp11, xp, xp2;

  xp = ptr_x[1];
  xp2 = ptr_x[0];

  xp = xp * IXHEAACE_COS_PI_BY_4;
  tmp00 = xp2 + xp;
  tmp01 = xp2 - xp;

  xp = ptr_x[2];
  xp2 = ptr_x[3];

  xp = xp * IXHEAACE_COS_PI_BY_4;
  tmp11 = xp2 - xp;
  tmp10 = xp2 + xp;

  ptr_x[3] = tmp00 * IXHEAACE_COS_PI_BY_16 + tmp10 * IXHEAACE_SIN_PI_BY_16;
  ptr_x[0] = tmp00 * IXHEAACE_SIN_PI_BY_16 - tmp10 * IXHEAACE_COS_PI_BY_16;

  ptr_x[2] = tmp01 * IXHEAACE_COS_3_PI_BY_16 - tmp11 * IXHEAACE_SIN_3_PI_BY_16;
  ptr_x[1] = tmp01 * IXHEAACE_SIN_3_PI_BY_16 + tmp11 * IXHEAACE_COS_3_PI_BY_16;
}

static VOID ia_enhaacplus_enc_fst4_4(FLOAT32 *ptr_x) {
  FLOAT32 tmp00, tmp01, tmp10, tmp11, xp, xp2;

  xp = ptr_x[1];
  xp2 = ptr_x[0];

  xp = xp * IXHEAACE_COS_PI_BY_4;
  tmp10 = xp2 + xp;
  tmp11 = xp2 - xp;

  xp = ptr_x[2];
  xp2 = ptr_x[3];

  xp = xp * IXHEAACE_COS_PI_BY_4;
  tmp01 = xp2 - xp;
  tmp00 = xp2 + xp;

  ptr_x[0] = tmp00 * IXHEAACE_COS_PI_BY_16 + tmp10 * IXHEAACE_SIN_PI_BY_16;
  ptr_x[3] = tmp10 * IXHEAACE_COS_PI_BY_16 - tmp00 * IXHEAACE_SIN_PI_BY_16;
  ptr_x[1] = tmp11 * IXHEAACE_SIN_3_PI_BY_16 - tmp01 * IXHEAACE_COS_3_PI_BY_16;
  ptr_x[2] = tmp01 * IXHEAACE_SIN_3_PI_BY_16 + tmp11 * IXHEAACE_COS_3_PI_BY_16;
}

static VOID ia_enhaacplus_enc_fct3_64(FLOAT32 *ptr_a, ixheaace_str_qmf_tabs *pstr_qmf_tab) {
  WORD32 k;

  const FLOAT32 *ptr_t1;
  FLOAT32 xp, xp2, xp3, xp4, wc, ws;

  FLOAT32 *ptr1, *ptr2;
  FLOAT32 *ptr_ap, *ptr_an;

  /* bit reversal */
  xp = ptr_a[1];
  ptr_a[1] = ptr_a[32];
  ptr_a[32] = xp;
  xp = ptr_a[2];
  ptr_a[2] = ptr_a[16];
  ptr_a[16] = xp;
  xp = ptr_a[3];
  ptr_a[3] = ptr_a[48];
  ptr_a[48] = xp;
  xp = ptr_a[4];
  ptr_a[4] = ptr_a[8];
  ptr_a[8] = xp;
  xp = ptr_a[5];
  ptr_a[5] = ptr_a[40];
  ptr_a[40] = xp;
  xp = ptr_a[6];
  ptr_a[6] = ptr_a[24];
  ptr_a[24] = xp;
  xp = ptr_a[7];
  ptr_a[7] = ptr_a[56];
  ptr_a[56] = xp;
  xp = ptr_a[9];
  ptr_a[9] = ptr_a[36];
  ptr_a[36] = xp;
  xp = ptr_a[10];
  ptr_a[10] = ptr_a[20];
  ptr_a[20] = xp;
  xp = ptr_a[11];
  ptr_a[11] = ptr_a[52];
  ptr_a[52] = xp;
  xp = ptr_a[13];
  ptr_a[13] = ptr_a[44];
  ptr_a[44] = xp;
  xp = ptr_a[14];
  ptr_a[14] = ptr_a[28];
  ptr_a[28] = xp;
  xp = ptr_a[15];
  ptr_a[15] = ptr_a[60];
  ptr_a[60] = xp;
  xp = ptr_a[17];
  ptr_a[17] = ptr_a[34];
  ptr_a[34] = xp;
  xp = ptr_a[19];
  ptr_a[19] = ptr_a[50];
  ptr_a[50] = xp;
  xp = ptr_a[21];
  ptr_a[21] = ptr_a[42];
  ptr_a[42] = xp;
  xp = ptr_a[22];
  ptr_a[22] = ptr_a[26];
  ptr_a[26] = xp;
  xp = ptr_a[23];
  ptr_a[23] = ptr_a[58];
  ptr_a[58] = xp;
  xp = ptr_a[25];
  ptr_a[25] = ptr_a[38];
  ptr_a[38] = xp;
  xp = ptr_a[27];
  ptr_a[27] = ptr_a[54];
  ptr_a[54] = xp;
  xp = ptr_a[29];
  ptr_a[29] = ptr_a[46];
  ptr_a[46] = xp;
  xp = ptr_a[31];
  ptr_a[31] = ptr_a[62];
  ptr_a[62] = xp;
  xp = ptr_a[35];
  ptr_a[35] = ptr_a[49];
  ptr_a[49] = xp;
  xp = ptr_a[37];
  ptr_a[37] = ptr_a[41];
  ptr_a[41] = xp;
  xp = ptr_a[39];
  ptr_a[39] = ptr_a[57];
  ptr_a[57] = xp;
  xp = ptr_a[43];
  ptr_a[43] = ptr_a[53];
  ptr_a[53] = xp;
  xp = ptr_a[47];
  ptr_a[47] = ptr_a[61];
  ptr_a[61] = xp;
  xp = ptr_a[55];
  ptr_a[55] = ptr_a[59];
  ptr_a[59] = xp;

  xp = ptr_a[33];
  xp2 = ptr_a[62];
  ptr_a[62] = xp2 - xp;
  ptr_a[33] = xp2 + xp;
  xp = ptr_a[34];
  xp2 = ptr_a[60];
  ptr_a[60] = xp2 - xp;
  ptr_a[34] = xp2 + xp;
  xp = ptr_a[35];
  xp2 = ptr_a[61];
  ptr_a[61] = xp2 - xp;
  ptr_a[35] = xp2 + xp;
  xp = ptr_a[36];
  xp2 = ptr_a[56];
  ptr_a[56] = xp2 - xp;
  ptr_a[36] = xp2 + xp;
  xp = ptr_a[37];
  xp2 = ptr_a[57];
  ptr_a[57] = xp2 - xp;
  ptr_a[37] = xp2 + xp;
  xp = ptr_a[38];
  xp2 = ptr_a[58];
  ptr_a[58] = xp2 - xp;
  ptr_a[38] = xp2 + xp;
  xp = ptr_a[39];
  xp2 = ptr_a[59];
  ptr_a[59] = xp2 - xp;
  ptr_a[39] = xp2 + xp;
  xp = ptr_a[40];
  xp2 = ptr_a[48];
  ptr_a[48] = xp2 - xp;
  ptr_a[40] = xp2 + xp;
  xp = ptr_a[41];
  xp2 = ptr_a[49];
  ptr_a[49] = xp2 - xp;
  ptr_a[41] = xp2 + xp;
  xp = ptr_a[42];
  xp2 = ptr_a[50];
  ptr_a[50] = xp2 - xp;
  ptr_a[42] = xp2 + xp;
  xp = ptr_a[43];
  xp2 = ptr_a[51];
  ptr_a[51] = xp2 - xp;
  ptr_a[43] = xp2 + xp;
  xp = ptr_a[44];
  xp2 = ptr_a[52];
  ptr_a[52] = xp2 - xp;
  ptr_a[44] = xp2 + xp;
  xp = ptr_a[45];
  xp2 = ptr_a[53];
  ptr_a[53] = xp2 - xp;
  ptr_a[45] = xp2 + xp;
  xp = ptr_a[46];
  xp2 = ptr_a[54];
  ptr_a[54] = xp2 - xp;
  ptr_a[46] = xp2 + xp;
  xp = ptr_a[47];
  xp2 = ptr_a[55];
  ptr_a[55] = xp2 - xp;
  ptr_a[47] = xp2 + xp;
  xp = ptr_a[17];
  xp2 = ptr_a[30];
  ptr_a[30] = xp2 - xp;
  ptr_a[17] = xp2 + xp;
  xp = ptr_a[18];
  xp2 = ptr_a[28];
  ptr_a[28] = xp2 - xp;
  ptr_a[18] = xp2 + xp;
  xp = ptr_a[19];
  xp2 = ptr_a[29];
  ptr_a[29] = xp2 - xp;
  ptr_a[19] = xp2 + xp;
  xp = ptr_a[20];
  xp2 = ptr_a[24];
  ptr_a[24] = xp2 - xp;
  ptr_a[20] = xp2 + xp;
  xp = ptr_a[21];
  xp2 = ptr_a[25];
  ptr_a[25] = xp2 - xp;
  ptr_a[21] = xp2 + xp;
  xp = ptr_a[22];
  xp2 = ptr_a[26];
  ptr_a[26] = xp2 - xp;
  ptr_a[22] = xp2 + xp;
  xp = ptr_a[23];
  xp2 = ptr_a[27];
  ptr_a[27] = xp2 - xp;
  ptr_a[23] = xp2 + xp;
  xp = ptr_a[9];
  xp2 = ptr_a[14];
  ptr_a[14] = xp2 - xp;
  ptr_a[9] = xp2 + xp;
  xp = ptr_a[10];
  xp2 = ptr_a[12];
  ptr_a[12] = xp2 - xp;
  ptr_a[10] = xp2 + xp;
  xp = ptr_a[11];
  xp2 = ptr_a[13];
  ptr_a[13] = xp2 - xp;
  ptr_a[11] = xp2 + xp;
  xp = ptr_a[41];
  xp2 = ptr_a[46];
  ptr_a[46] = xp2 - xp;
  ptr_a[41] = xp2 + xp;
  xp = ptr_a[42];
  xp2 = ptr_a[44];
  ptr_a[44] = xp2 - xp;
  ptr_a[42] = xp2 + xp;
  xp = ptr_a[43];
  xp2 = ptr_a[45];
  ptr_a[45] = xp2 - xp;
  ptr_a[43] = xp2 + xp;
  xp = ptr_a[49];
  xp2 = ptr_a[54];
  ptr_a[49] = xp - xp2;
  ptr_a[54] = xp2 + xp;
  xp = ptr_a[50];
  xp2 = ptr_a[52];
  ptr_a[50] = xp - xp2;
  ptr_a[52] = xp2 + xp;
  xp = ptr_a[51];
  xp2 = ptr_a[53];
  ptr_a[51] = xp - xp2;
  ptr_a[53] = xp2 + xp;
  xp = ptr_a[5];
  xp2 = ptr_a[6];
  ptr_a[6] = xp2 - xp;
  ptr_a[5] = xp2 + xp;
  xp = ptr_a[21];
  xp2 = ptr_a[22];
  ptr_a[22] = xp2 - xp;
  ptr_a[21] = xp2 + xp;
  xp = ptr_a[25];
  xp2 = ptr_a[26];
  ptr_a[25] = xp - xp2;
  ptr_a[26] = xp2 + xp;
  xp = ptr_a[37];
  xp2 = ptr_a[38];
  ptr_a[38] = xp2 - xp;
  ptr_a[37] = xp2 + xp;
  xp = ptr_a[57];
  xp2 = ptr_a[58];
  ptr_a[57] = xp - xp2;
  ptr_a[58] = xp2 + xp;

  ia_enhaacplus_enc_fct3_4(ptr_a);
  ia_enhaacplus_enc_fct4_4r(ptr_a + 4);
  ia_enhaacplus_enc_fct3_4(ptr_a + 8);
  ia_enhaacplus_enc_fst3_4r(ptr_a + 12);

  ia_enhaacplus_enc_fct3_4(ptr_a + 16);
  ia_enhaacplus_enc_fct4_4r(ptr_a + 20);
  ia_enhaacplus_enc_fst4_4(ptr_a + 24);
  ia_enhaacplus_enc_fst3_4r(ptr_a + 28);

  ia_enhaacplus_enc_fct3_4(ptr_a + 32);
  ia_enhaacplus_enc_fct4_4r(ptr_a + 36);
  ia_enhaacplus_enc_fct3_4(ptr_a + 40);
  ia_enhaacplus_enc_fst3_4r(ptr_a + 44);

  ia_enhaacplus_enc_fct3_4(ptr_a + 48);
  ia_enhaacplus_enc_fst3_4r(ptr_a + 52);
  ia_enhaacplus_enc_fst4_4(ptr_a + 56);
  ia_enhaacplus_enc_fst3_4r(ptr_a + 60);

  ptr_ap = ptr_a;
  ptr_an = ptr_a + 7;
  for (k = 4; k != 0; k--) {
    xp = *ptr_ap++;
    xp2 = *ptr_an--;
    xp3 = *(ptr_ap + 15);
    xp4 = *(ptr_an + 17);
    *(ptr_an + 1) = xp - xp2;
    *(ptr_ap - 1) = xp + xp2;

    xp = *(ptr_ap + 23);
    xp2 = *(ptr_an + 25);
    *(ptr_an + 17) = xp3 - xp4;
    *(ptr_ap + 15) = xp3 + xp4;

    xp3 = *(ptr_ap + 31);
    xp4 = *(ptr_an + 33);
    *(ptr_ap + 23) = xp - xp2;
    *(ptr_an + 25) = xp + xp2;

    xp = *(ptr_ap + 55);
    xp2 = *(ptr_an + 57);
    *(ptr_an + 33) = xp3 - xp4;
    *(ptr_ap + 31) = xp3 + xp4;

    *(ptr_ap + 55) = xp - xp2;
    *(ptr_an + 57) = xp + xp2;
  }

  ptr_t1 = pstr_qmf_tab->cos_sin_fct4_8;

  ptr_ap = ptr_a + 8;
  ptr_an = ptr_a + 15;
  for (k = 4; k != 0; k--) {
    wc = *ptr_t1++;
    ws = *ptr_t1++;

    xp = *ptr_ap * wc + *ptr_an * ws;
    *ptr_ap = *ptr_ap * ws - *ptr_an * wc;
    ptr_ap++;
    *ptr_an-- = xp;

    xp = *(ptr_ap + 31) * wc + *(ptr_an + 33) * ws;
    *(ptr_ap + 31) = *(ptr_ap + 31) * ws - *(ptr_an + 33) * wc;
    *(ptr_an + 33) = xp;

    xp = *(ptr_ap + 39) * ws + *(ptr_an + 41) * wc;
    *(ptr_an + 41) = *(ptr_ap + 39) * wc - *(ptr_an + 41) * ws;
    *(ptr_ap + 39) = xp;
  }

  ptr_ap = ptr_a;
  ptr_an = ptr_a + 15;
  for (k = 8; k != 0; k--) {
    xp = *ptr_ap++;
    xp2 = *ptr_an--;
    xp3 = *(ptr_ap + 31);
    xp4 = *(ptr_an + 33);

    *(ptr_an + 1) = xp - xp2;
    *(ptr_ap - 1) = xp + xp2;

    xp = *(ptr_ap + 47);
    xp2 = *(ptr_an + 49);
    *(ptr_an + 33) = xp3 - xp4;
    *(ptr_ap + 31) = xp3 + xp4;
    *(ptr_ap + 47) = xp - xp2;
    *(ptr_an + 49) = xp + xp2;
  }

  ptr_t1 = pstr_qmf_tab->cos_sin_fct4_16;

  ptr1 = &ptr_a[16];
  ptr2 = &ptr_a[31];
  for (k = 7; k >= 0; k--) {
    wc = *ptr_t1++;
    ws = *ptr_t1++;

    xp = *ptr1;
    xp2 = *ptr2;

    *ptr2-- = xp * wc + xp2 * ws;
    *ptr1++ = xp * ws - xp2 * wc;
  }

  ptr1 = &ptr_a[0];
  ptr2 = &ptr_a[31];
  for (k = 15; k >= 0; k--) {
    xp = *ptr1;
    xp2 = *ptr2;

    *ptr1++ = xp + xp2;
    *ptr2-- = xp - xp2;
  }

  ptr_t1 = pstr_qmf_tab->cos_sin_fct4_32;
  ptr1 = &ptr_a[32];
  ptr2 = &ptr_a[63];

  for (k = 15; k >= 0; k--) {
    wc = *ptr_t1++;
    ws = *ptr_t1++;

    xp = *ptr1;
    xp2 = *ptr2;

    *ptr2-- = xp * wc + xp2 * ws;
    *ptr1++ = xp * ws - xp2 * wc;
  }

  ptr1 = &ptr_a[0];
  ptr2 = &ptr_a[63];
  for (k = 31; k >= 0; k--) {
    xp = *ptr1;
    xp2 = *ptr2;

    *ptr1++ = xp + xp2;
    *ptr2-- = xp - xp2;
  }
}

static VOID ia_enhaacplus_enc_fst3_64(FLOAT32 *ptr_a, ixheaace_str_qmf_tabs *pstr_qmf_tab) {
  WORD32 k;
  FLOAT32 xp, xp2;
  FLOAT32 *ptr1, *ptr2;

  ptr1 = &ptr_a[0];
  ptr2 = &ptr_a[63];

  for (k = 31; k >= 0; k--) {
    xp = *ptr1;
    xp2 = *ptr2;
    *ptr2-- = xp;
    *ptr1++ = xp2;
  }

  ia_enhaacplus_enc_fct3_64(ptr_a, pstr_qmf_tab);

  ptr1 = &ptr_a[1];

  for (k = 15; k >= 0; k--) {
    xp = *ptr1;
    xp2 = *(ptr1 + 2);
    *ptr1++ = -xp;
    ptr1++;
    *ptr1++ = -xp2;
    ptr1++;
  }
}

static VOID ixheaace_sbr_pre_mdct(FLOAT32 *ptr_x, WORD32 len, const FLOAT32 *ptr_sine_window) {
  WORD32 i;
  FLOAT32 wre, wim, re1, re2, im1, im2;

  for (i = 0; i < len / 4; i++) {
    re1 = ptr_x[2 * i];
    re2 = ptr_x[2 * i + 1];
    im2 = ptr_x[len - 2 - 2 * i];
    im1 = ptr_x[len - 1 - 2 * i];

    wim = ptr_sine_window[2 * i];
    wre = ptr_sine_window[len - 1 - 2 * i];

    ptr_x[2 * i] = im1 * wim + re1 * wre;

    ptr_x[2 * i + 1] = im1 * wre - re1 * wim;

    wre = ptr_sine_window[len - 2 - 2 * i];
    wim = ptr_sine_window[2 * i + 1];

    ptr_x[len - 2 - 2 * i] = im2 * wim + re2 * wre;

    ptr_x[len - 1 - 2 * i] = -(im2 * wre - re2 * wim);
  }
}
static VOID ixheaace_sbr_pre_mdst(FLOAT32 *ptr_x, WORD32 len, const FLOAT32 *ptr_sine_window) {
  WORD32 i;
  FLOAT32 wre, wim, re1, re2, im1, im2;

  for (i = 0; i < len / 4; i++) {
    re1 = -ptr_x[2 * i];
    re2 = ptr_x[2 * i + 1];
    im2 = -ptr_x[len - 2 - 2 * i];
    im1 = ptr_x[len - 1 - 2 * i];

    wim = ptr_sine_window[2 * i];
    wre = ptr_sine_window[len - 1 - 2 * i];

    ptr_x[2 * i] = im1 * wim + re1 * wre;

    ptr_x[2 * i + 1] = im1 * wre - re1 * wim;

    wim = ptr_sine_window[2 * i + 1];
    wre = ptr_sine_window[len - 2 - 2 * i];

    ptr_x[len - 2 - 2 * i] = im2 * wim + re2 * wre;

    ptr_x[len - 1 - 2 * i] = -(im2 * wre - re2 * wim);
  }
}
static VOID ixheaace_sbr_post_mdct(FLOAT32 *ptr_x, WORD32 len, const FLOAT32 *ptr_trig_data) {
  WORD32 i;
  FLOAT32 wre, wim, re1, re2, im1, im2;

  FLOAT32 temp1 = -ptr_x[1];
  FLOAT32 temp2 = ptr_x[len / 2];
  FLOAT32 temp3 = ptr_x[len / 2 + 1];
  FLOAT32 val = 0.70709228515625f;

  re2 = ptr_x[len - 2];
  im2 = ptr_x[len - 1];
  for (i = 1; i < len / 4; i++) {
    wim = ptr_trig_data[i];
    wre = ptr_trig_data[len / 2 - 1 - i];

    ptr_x[2 * i - 1] = (re2 * wre - im2 * wim);
    ptr_x[len - 2 * i] = (re2 * wim + im2 * wre);

    re1 = ptr_x[2 * i];
    im1 = ptr_x[2 * i + 1];
    re2 = ptr_x[len - 2 - 2 * i];
    im2 = ptr_x[len - 1 - 2 * i];

    ptr_x[2 * i] = (re1 * wre + im1 * wim);
    ptr_x[len - 1 - 2 * i] = (re1 * wim - im1 * wre);
  }

  ptr_x[len / 2 - 1] = (temp2 - temp3) * val;
  ptr_x[len / 2] = (temp2 + temp3) * val;
  ptr_x[len - 1] = temp1;
}
static VOID ixheaace_sbr_post_mdst(FLOAT32 *ptr_x, WORD32 len, const FLOAT32 *ptr_trig_data) {
  WORD32 i;
  FLOAT32 wre, wim, re1, re2, im1, im2;
  FLOAT32 temp0 = -ptr_x[0];
  FLOAT32 temp1 = ptr_x[1];
  FLOAT32 temp2 = ptr_x[len / 2];
  FLOAT32 temp3 = ptr_x[len / 2 + 1];
  FLOAT32 val = 0.70709228515625f;

  re2 = ptr_x[len - 2];
  im2 = ptr_x[len - 1];

  for (i = 1; i < len / 4; i++) {
    wim = ptr_trig_data[i];
    wre = ptr_trig_data[len / 2 - 1 - i];

    ptr_x[2 * i - 1] = -(re2 * wim + im2 * wre);
    ptr_x[len - 2 * i] = -(re2 * wre - im2 * wim);

    re1 = ptr_x[2 * i];
    im1 = ptr_x[2 * i + 1];
    re2 = ptr_x[len - 2 - 2 * i];
    im2 = ptr_x[len - 1 - 2 * i];

    ptr_x[len - 1 - 2 * i] = -(re1 * wre + im1 * wim);
    ptr_x[2 * i] = -(re1 * wim - im1 * wre);
  }

  ptr_x[len / 2] = (temp3 - temp2) * val;
  ptr_x[len / 2 - 1] = -(temp3 + temp2) * val;

  ptr_x[0] = temp1;
  ptr_x[len - 1] = temp0;
}
static VOID ixheaace_sbr_mdct(FLOAT32 *ptr_dct_data, WORD32 n, FLOAT32 *ptr_sbr_scratch) {
  ixheaace_sbr_pre_mdct(ptr_dct_data, n, &long_window_sine_ld_64[0]);

  ia_enhaacplus_enc_complex_fft_p2(ptr_dct_data, n / 2, ptr_sbr_scratch);

  ixheaace_sbr_post_mdct(ptr_dct_data, n, &fft_twiddle_tab_32[0]);
}

static VOID ixheaace_sbr_mdst(FLOAT32 *ptr_dct_data, WORD32 n, FLOAT32 *ptr_sbr_scratch) {
  ixheaace_sbr_pre_mdst(ptr_dct_data, n, &long_window_sine_ld_64[0]);

  ia_enhaacplus_enc_complex_fft_p2(ptr_dct_data, n / 2, ptr_sbr_scratch);

  ixheaace_sbr_post_mdst(ptr_dct_data, n, &fft_twiddle_tab_32[0]);
}
static VOID ia_enhaacplus_enc_forward_modulation(const FLOAT32 *ptr_time_in,
                                                 FLOAT32 *ptr_r_subband, FLOAT32 *ptr_i_subband,
                                                 ixheaace_str_qmf_tabs *pstr_qmf_tab,
                                                 WORD32 is_ld_sbr, FLOAT32 *ptr_sbr_scratch) {
  WORD32 i;
  FLOAT32 tmp1, tmp2, tmp3, tmp4;
  const FLOAT32 *ptr_inp1, *ptr_inp2;
  FLOAT32 *ptr_re, *ptr_im;
  FLOAT32 real, imag;
  const FLOAT32 *ptr_window = &sbr_sin_cos_window[0];
  if (is_ld_sbr) {
    ptr_re = &ptr_r_subband[0];
    ptr_im = &ptr_i_subband[0];
    ptr_inp1 = &ptr_time_in[0];
  } else {
    ptr_r_subband[0] = ptr_time_in[0];
    ptr_re = &ptr_r_subband[1];
    ptr_im = &ptr_i_subband[0];
    ptr_inp1 = &ptr_time_in[1];
  }
  ptr_inp2 = &ptr_time_in[127];
  if (is_ld_sbr) {
    i = IXHEAACE_QMF_CHANNELS / 2 - 1;
    while (i >= 0) {
      tmp1 = *ptr_inp1++;
      tmp2 = *ptr_inp2--;
      tmp3 = *ptr_inp1++;
      tmp4 = *ptr_inp2--;
      *ptr_re++ = (tmp1 - tmp2);
      *ptr_im++ = (tmp1 + tmp2);
      *ptr_re++ = (tmp3 - tmp4);
      *ptr_im++ = (tmp3 + tmp4);
      i--;
    }

    ixheaace_sbr_mdct(ptr_r_subband, 64, ptr_sbr_scratch);
    ixheaace_sbr_mdst(ptr_i_subband, 64, ptr_sbr_scratch);

    i = 0;
    while (i < IXHEAACE_QMF_CHANNELS) {
      real = ptr_r_subband[i];
      imag = ptr_i_subband[i];
      ptr_r_subband[i] =
          imag * ptr_window[i] + real * ptr_window[2 * IXHEAACE_QMF_CHANNELS - 1 - i];
      ptr_i_subband[i] =
          imag * ptr_window[2 * IXHEAACE_QMF_CHANNELS - 1 - i] - real * ptr_window[i];
      i++;
    }
  } else {
    for (i = 30; i >= 0; i--) {
      tmp1 = *ptr_inp1++;
      tmp2 = *ptr_inp2--;
      tmp3 = *ptr_inp1++;

      *ptr_re++ = tmp1 - tmp2;
      tmp4 = *ptr_inp2--;
      *ptr_im++ = tmp1 + tmp2;
      *ptr_re++ = tmp3 - tmp4;
      *ptr_im++ = tmp3 + tmp4;
    }
    tmp1 = *ptr_inp1;
    tmp2 = *ptr_inp2;
    *ptr_re = tmp1 - tmp2;
    *ptr_im = tmp1 + tmp2;

    ptr_i_subband[63] = ptr_time_in[64];

    ia_enhaacplus_enc_fct3_64(ptr_r_subband, pstr_qmf_tab);

    ia_enhaacplus_enc_fst3_64(ptr_i_subband, pstr_qmf_tab);
  }
}

static VOID ia_enhaacplus_enc_sbr_qmf_analysis_win_add(const FLOAT32 *ptr_pf_l,
                                                       const FLOAT32 *ptr_pf_r, FLOAT32 *ptr_fp1,
                                                       FLOAT32 *ptr_fp2,
                                                       FLOAT32 *ptr_syn_buffer) {
  FLOAT32 accu_l = 0, accu_r = 0;
  WORD k;
  for (k = 0; k < 64; k++) {
    accu_l = 0;
    accu_r = 0;

    {
      accu_l += *ptr_pf_l++ * ptr_fp1[k];
      accu_l += *ptr_pf_l++ * ptr_fp1[128 + k];
      accu_l += *ptr_pf_l++ * ptr_fp1[256 + k];
      accu_l += *ptr_pf_l++ * ptr_fp1[384 + k];
      accu_l += *ptr_pf_l++ * ptr_fp1[512 + k];

      accu_r += *--ptr_pf_r * ptr_fp2[k];
      accu_r += *--ptr_pf_r * ptr_fp2[128 + k];
      accu_r += *--ptr_pf_r * ptr_fp2[256 + k];
      accu_r += *--ptr_pf_r * ptr_fp2[384 + k];
      accu_r += *--ptr_pf_r * ptr_fp2[512 + k];
    }
    ptr_syn_buffer[127 - k] = accu_l;
    ptr_syn_buffer[63 - k] = accu_r;
    ptr_pf_l += 5;
    ptr_pf_r -= 5;
  }
}

static VOID ia_enhaacplus_enc_sbr_cld_analysis_win_add(FLOAT32 *ptr_filter_states,
                                                       const FLOAT32 *ptr_coeffs,
                                                       FLOAT32 *ptr_sync_buffer,
                                                       WORD32 num_qmf_ch, WORD32 time_sn_stride) {
  WORD32 i;
  WORD32 len = num_qmf_ch * time_sn_stride;
  FLOAT32 *ptr_fil_states = &ptr_filter_states[CLD_FILTER_LENGTH - 1];

  memset(ptr_sync_buffer, 0, len * sizeof(*ptr_sync_buffer));

  for (i = 0; i < CLD_FILTER_LENGTH; i++) {
    ptr_sync_buffer[i % len] += (*ptr_fil_states-- * ptr_coeffs[i]);
  }
}
VOID ixheaace_sbr_analysis_filtering(const FLOAT32 *ptr_time_in, WORD32 time_sn_stride,
                                     FLOAT32 **ptr_ana_r, FLOAT32 **ptr_ana_i,
                                     ixheaace_pstr_sbr_qmf_filter_bank pstr_qmf_bank,
                                     ixheaace_str_qmf_tabs *pstr_qmf_tab, WORD32 num_qmf_subsamp,
                                     WORD32 is_ld_sbr, FLOAT32 *ptr_sbr_scratch,
                                     WORD32 is_ps_960) {
  WORD32 i, k;
  const FLOAT32 *ptr_pf_l, *ptr_pf_r;
  FLOAT32 *ptr_fp1, *ptr_fp2, *ptr_tmp;
  FLOAT32 syn_buffer[2 * IXHEAACE_QMF_CHANNELS] = {0};
  const FLOAT32 *ptr_start_coeff_l;
  const FLOAT32 *ptr_start_coeff_r;
  FLOAT32 *ptr_qmf_states_curr_pos;
  WORD32 start_coeff_cnt = 0;
  WORD16 flag = 0;
  WORD32 offset;
  if (is_ld_sbr) {
    offset = 576;
    i = 0;
    while (i < num_qmf_subsamp) {
      ptr_qmf_states_curr_pos = pstr_qmf_bank->ptr_qmf_states_curr_pos;

      if (ptr_time_in) {
        FLOAT32 *ptr_qmf_states = pstr_qmf_bank->ptr_qmf_states_curr_pos + offset;

        const FLOAT32 *ptr_inp = &ptr_time_in[(i * 64 * time_sn_stride)];
        FLOAT32 tmp1, tmp2, tmp3, tmp4;
        for (k = 15; k >= 0; k--) {
          tmp1 = *ptr_inp;
          ptr_inp += time_sn_stride;

          tmp2 = *ptr_inp;
          ptr_inp += time_sn_stride;

          tmp3 = *ptr_inp;
          ptr_inp += time_sn_stride;

          *ptr_qmf_states++ = tmp1;
          tmp4 = *ptr_inp;
          ptr_inp += time_sn_stride;

          *ptr_qmf_states++ = tmp2;
          *ptr_qmf_states++ = tmp3;
          *ptr_qmf_states++ = tmp4;
        }
      } else {
        memset(&ptr_qmf_states_curr_pos[0], 0,
               sizeof(ptr_qmf_states_curr_pos[0]) * IXHEAACE_QMF_CHANNELS);
      }

      ia_enhaacplus_enc_sbr_cld_analysis_win_add(&ptr_qmf_states_curr_pos[0],
                                                 pstr_qmf_bank->ptr_filter, syn_buffer,
                                                 IXHEAACE_QMF_CHANNELS, time_sn_stride);

      ia_enhaacplus_enc_forward_modulation(syn_buffer, &(ptr_ana_r[i][0]), &(ptr_ana_i[i][0]),
                                           pstr_qmf_tab, is_ld_sbr, ptr_sbr_scratch);

      for (k = 0; k < offset; k++) {
        pstr_qmf_bank->ptr_qmf_states_curr_pos[k] =
            pstr_qmf_bank->ptr_qmf_states_curr_pos[64 + k];
      }

      i++;
    }
  } else {
    flag = pstr_qmf_bank->flag;
    ptr_fp1 = pstr_qmf_bank->ptr_qmf_states_buf;
    ptr_fp2 = pstr_qmf_bank->ptr_qmf_states_buf + 64;
    pstr_qmf_bank->ptr_qmf_states_curr_pos =
        ((FLOAT32 *)(pstr_qmf_bank->ptr_qmf_states_buf) + (pstr_qmf_bank->offset * 64));

    if (pstr_qmf_bank->offset == 8) {
      pstr_qmf_bank->offset = 0;
    } else {
      pstr_qmf_bank->offset += 2;
    }

    ptr_start_coeff_l = pstr_qmf_bank->ptr_ref_coeff_l + pstr_qmf_bank->offset_l;
    ptr_start_coeff_r = pstr_qmf_bank->ptr_ref_coeff_r - pstr_qmf_bank->offset_r;

    for (i = 0; i < num_qmf_subsamp; i++) {
      ptr_qmf_states_curr_pos = pstr_qmf_bank->ptr_qmf_states_curr_pos;
      ptr_tmp = ptr_fp1;
      ptr_fp1 = ptr_fp2;
      ptr_fp2 = ptr_tmp;
      if (((1 == is_ld_sbr) && start_coeff_cnt) || ((0 == is_ld_sbr) && (i % 2))) {
        ptr_start_coeff_l--;
        if (ptr_start_coeff_l == pstr_qmf_bank->ptr_ref_coeff_l) {
          ptr_start_coeff_l += 5;
        }
      } else {
        ptr_start_coeff_r++;
        if (ptr_start_coeff_r == pstr_qmf_bank->ptr_ref_coeff_r) {
          ptr_start_coeff_r -= 5;
        }
      }

      if (ptr_time_in) {
        FLOAT32 *ptr_qmf_states = &ptr_qmf_states_curr_pos[0];
        const FLOAT32 *ptr_inp = &ptr_time_in[(i * 64 * time_sn_stride)];
        FLOAT32 tmp1, tmp2, tmp3, tmp4;
        for (k = 15; k >= 0; k--) {
          tmp1 = *ptr_inp;
          ptr_inp += time_sn_stride;

          tmp2 = *ptr_inp;
          ptr_inp += time_sn_stride;

          tmp3 = *ptr_inp;
          ptr_inp += time_sn_stride;

          *ptr_qmf_states++ = tmp1;
          tmp4 = *ptr_inp;
          ptr_inp += time_sn_stride;

          *ptr_qmf_states++ = tmp2;
          *ptr_qmf_states++ = tmp3;
          *ptr_qmf_states++ = tmp4;
        }
      } else {
        memset(&ptr_qmf_states_curr_pos[0], 0,
               sizeof(ptr_qmf_states_curr_pos[0]) * IXHEAACE_QMF_CHANNELS);
      }

      ptr_pf_l = ptr_start_coeff_l;
      ptr_pf_r = ptr_start_coeff_r;

      ia_enhaacplus_enc_sbr_qmf_analysis_win_add(ptr_pf_l, ptr_pf_r, ptr_fp1, ptr_fp2,
                                                 syn_buffer);

      ia_enhaacplus_enc_forward_modulation(syn_buffer, &(ptr_ana_r[i][0]), &(ptr_ana_i[i][0]),
                                           pstr_qmf_tab, is_ld_sbr, ptr_sbr_scratch);

      pstr_qmf_bank->ptr_qmf_states_curr_pos = pstr_qmf_bank->ptr_qmf_states_curr_pos + 64;
      if (pstr_qmf_bank->ptr_qmf_states_curr_pos == pstr_qmf_bank->ptr_qmf_states_buf + 640) {
        pstr_qmf_bank->ptr_qmf_states_curr_pos = pstr_qmf_bank->ptr_qmf_states_buf;
      }
    }

    pstr_qmf_bank->offset_l = ptr_start_coeff_l - pstr_qmf_bank->ptr_ref_coeff_l;
    pstr_qmf_bank->offset_r = pstr_qmf_bank->ptr_ref_coeff_r - ptr_start_coeff_r;
    pstr_qmf_bank->flag = flag;
  }
  if (is_ps_960 == 1) {
    memset(&ptr_ana_r[num_qmf_subsamp][0], 0, sizeof(ptr_ana_r[num_qmf_subsamp][0]) *
           IXHEAACE_QMF_CHANNELS * (IXHEAACE_QMF_TIME_SLOTS - num_qmf_subsamp));
    memset(&ptr_ana_i[num_qmf_subsamp][0], 0, sizeof(ptr_ana_i[num_qmf_subsamp][0]) *
           IXHEAACE_QMF_CHANNELS * (IXHEAACE_QMF_TIME_SLOTS - num_qmf_subsamp));
  }
}

VOID ixheaace_get_energy_from_cplx_qmf(
    FLOAT32 **ptr_energy_vals, FLOAT32 **ptr_real_values, FLOAT32 **ptr_imag_values,
    WORD32 is_ld_sbr, WORD32 num_time_slots, WORD32 samp_ratio_fac,
    ixheaace_str_hbe_enc *pstr_hbe_enc, WORD32 op_delay, WORD32 harmonic_sbr)

{
  WORD32 j, k;
  FLOAT32 avg_fac = 0.5f;
  if (samp_ratio_fac == 4) {
    avg_fac = 0.25f;
  }
  if (0 == is_ld_sbr) {
    FLOAT32 *ptr_energy_val = &ptr_energy_vals[0][0];
    FLOAT32 *ptr_hbe_real = NULL;
    FLOAT32 *ptr_hbe_imag = NULL;
    if (harmonic_sbr == 1) {
      ptr_hbe_real = &pstr_hbe_enc->qmf_buf_real[op_delay][0];
      ptr_hbe_imag = &pstr_hbe_enc->qmf_buf_imag[op_delay][0];
    }
    for (k = 0; k < num_time_slots; k++) {
      for (j = 0; j < IXHEAACE_QMF_CHANNELS; j++) {
        FLOAT32 tmp = 0.0f;
        if (harmonic_sbr == 1) {
          FLOAT32 real_hbe, imag_hbe;
          real_hbe = *(ptr_hbe_real);
          imag_hbe = *(ptr_hbe_imag);
          tmp += (real_hbe * real_hbe) + (imag_hbe * imag_hbe);
          *ptr_energy_val = tmp;
          ptr_hbe_real++;
          ptr_hbe_imag++;
          ptr_energy_val++;
        } else {
          FLOAT32 real, imag;
          WORD32 i, subband;
          subband = samp_ratio_fac * k;
          for (i = 0; i < samp_ratio_fac; i++) {
            real = ptr_real_values[subband + i][j];
            imag = ptr_imag_values[subband + i][j];
            tmp += (real * real) + (imag * imag);
          }
          ptr_energy_vals[k][j] = tmp * avg_fac;
        }
      }
      if (harmonic_sbr == 1) {
        ptr_hbe_real += 64;
        ptr_hbe_imag += 64;
      }
    }
  } else {
    FLOAT32 *ptr_real = &ptr_real_values[0][0];
    FLOAT32 *ptr_imag = &ptr_imag_values[0][0];
    for (k = 0; k < num_time_slots; k++) {
      FLOAT32 *ptr_energy_val = &ptr_energy_vals[k][0];
      for (j = 0; j < 64; j++) {
        FLOAT32 real, imag, tmp;
        real = *ptr_real;
        ptr_real++;
        imag = *ptr_imag;
        ptr_imag++;

        tmp = (real * real) + (imag * imag);
        *ptr_energy_val = tmp;
        ptr_energy_val++;
      }
    }
  }
}

VOID ixheaace_fft16(FLOAT32 *vector) {
  FLOAT32 var10, var11, var12, var13, var14, var15, var16, var17, var18, var19, var110, var111,
      var112, var113, var114, var115;
  FLOAT32 var20, var21, var22, var23, var24, var25, var26, var27, var28, var29, var210, var211,
      var212, var213, var214, var215;
  FLOAT32 arr0, arr1, arr2, arr3, arr4, arr5, arr6, arr7, arr8, arr9, arr10, arr11, arr12, arr13,
      arr14, arr15;

  arr0 = vector[0] + vector[16];
  arr8 = vector[8] + vector[24];
  var10 = arr0 + arr8;
  var12 = arr0 - arr8;
  arr1 = vector[1] + vector[17];
  arr9 = vector[9] + vector[25];
  var11 = arr1 + arr9;
  var13 = arr1 - arr9;
  arr2 = vector[2] + vector[18];
  arr10 = vector[10] + vector[26];
  var14 = arr2 + arr10;
  var16 = arr2 - arr10;
  arr3 = vector[3] + vector[19];
  arr11 = vector[11] + vector[27];
  var15 = arr3 + arr11;
  var17 = arr3 - arr11;
  arr4 = vector[4] + vector[20];
  arr12 = vector[12] + vector[28];
  var18 = arr4 + arr12;
  var110 = arr4 - arr12;
  arr5 = vector[5] + vector[21];
  arr13 = vector[13] + vector[29];
  var19 = arr5 + arr13;
  var111 = arr5 - arr13;
  arr6 = vector[6] + vector[22];
  arr14 = vector[14] + vector[30];
  var112 = arr6 + arr14;
  var114 = arr6 - arr14;
  arr7 = vector[7] + vector[23];
  arr15 = vector[15] + vector[31];
  var113 = arr7 + arr15;
  var115 = arr7 - arr15;

  var20 = var10 + var18;
  var24 = var10 - var18;
  var21 = var11 + var19;
  var25 = var11 - var19;
  var28 = var12 - var111;
  var210 = var12 + var111;
  var29 = var13 + var110;
  var211 = var13 - var110;
  var22 = var14 + var112;
  var27 = var14 - var112;
  var23 = var15 + var113;
  var26 = var113 - var15;

  var11 = var16 + var114;
  var12 = var16 - var114;
  var10 = var17 + var115;
  var13 = var17 - var115;

  var212 = (var10 + var12) * IXHEAACE_INV_SQRT2;
  var214 = (var10 - var12) * IXHEAACE_INV_SQRT2;
  var213 = (var13 - var11) * IXHEAACE_INV_SQRT2;
  var215 = (var11 + var13) * -IXHEAACE_INV_SQRT2;

  arr0 = vector[0] - vector[16];
  arr1 = vector[1] - vector[17];
  arr2 = vector[2] - vector[18];
  arr3 = vector[3] - vector[19];
  arr4 = vector[4] - vector[20];
  arr5 = vector[5] - vector[21];
  arr6 = vector[6] - vector[22];
  arr7 = vector[7] - vector[23];
  arr8 = vector[8] - vector[24];
  arr9 = vector[9] - vector[25];
  arr10 = vector[10] - vector[26];
  arr11 = vector[11] - vector[27];
  arr12 = vector[12] - vector[28];
  arr13 = vector[13] - vector[29];
  arr14 = vector[14] - vector[30];
  arr15 = vector[15] - vector[31];

  var19 = ((arr2 + arr14) * -IXHEAACE_COS_3PI_DIV8);
  var110 = ((arr2 - arr14) * IXHEAACE_COS_PI_DIV8);
  var18 = ((arr3 + arr15) * IXHEAACE_COS_3PI_DIV8);
  var111 = ((arr3 - arr15) * IXHEAACE_COS_PI_DIV8);
  var15 = ((arr4 + arr12) * -IXHEAACE_INV_SQRT2);
  var16 = ((arr4 - arr12) * IXHEAACE_INV_SQRT2);
  var14 = ((arr5 + arr13) * IXHEAACE_INV_SQRT2);
  var17 = ((arr5 - arr13) * IXHEAACE_INV_SQRT2);
  var113 = ((arr6 + arr10) * -IXHEAACE_COS_PI_DIV8);
  var114 = ((arr6 - arr10) * IXHEAACE_COS_3PI_DIV8);
  var112 = ((arr7 + arr11) * IXHEAACE_COS_PI_DIV8);
  var115 = ((arr7 - arr11) * IXHEAACE_COS_3PI_DIV8);

  arr2 = (var18 * IXHEAACE_SQRT2PLUS1) - (var112 * IXHEAACE_SQRT2MINUS1);
  arr3 = (var19 * IXHEAACE_SQRT2PLUS1) - (var113 * IXHEAACE_SQRT2MINUS1);
  arr4 = (var110 * IXHEAACE_SQRT2MINUS1) - (var114 * IXHEAACE_SQRT2PLUS1);
  arr5 = (var111 * IXHEAACE_SQRT2MINUS1) - (var115 * IXHEAACE_SQRT2PLUS1);

  var18 = var18 + var112;
  var19 = var19 + var113;
  var110 = var110 + var114;
  var111 = var111 + var115;

  arr6 = arr0 + var14;
  arr10 = arr0 - var14;
  arr7 = arr1 + var15;
  arr11 = arr1 - var15;

  arr12 = var16 - arr9;
  arr14 = var16 + arr9;
  arr13 = arr8 + var17;
  arr15 = arr8 - var17;

  var10 = arr6 - arr14;
  var12 = arr6 + arr14;
  var11 = arr7 + arr15;
  var13 = arr7 - arr15;
  var14 = arr10 + arr12;
  var16 = arr10 - arr12;
  var15 = arr11 + arr13;
  var17 = arr11 - arr13;

  arr10 = var18 + var110;
  var110 = var18 - var110;
  arr11 = var19 + var111;
  var111 = var19 - var111;

  var112 = (arr2 + arr4);
  var114 = (arr2 - arr4);
  var113 = (arr3 + arr5);
  var115 = (arr3 - arr5);

  vector[0] = var20 + var22;
  vector[1] = var21 + var23;
  vector[2] = var12 + arr10;
  vector[3] = var13 + arr11;
  vector[4] = var210 + var212;
  vector[5] = var211 + var213;
  vector[6] = var10 + var112;
  vector[7] = var11 + var113;
  vector[8] = var24 - var26;
  vector[9] = var25 - var27;
  vector[10] = var16 + var114;
  vector[11] = var17 + var115;
  vector[12] = var28 + var214;
  vector[13] = var29 + var215;
  vector[14] = var14 + var110;
  vector[15] = var15 + var111;
  vector[16] = var20 - var22;
  vector[17] = var21 - var23;
  vector[18] = var12 - arr10;
  vector[19] = var13 - arr11;
  vector[20] = var210 - var212;
  vector[21] = var211 - var213;
  vector[22] = var10 - var112;
  vector[23] = var11 - var113;
  vector[24] = var24 + var26;
  vector[25] = var25 + var27;
  vector[26] = var16 - var114;
  vector[27] = var17 - var115;
  vector[28] = var28 - var214;
  vector[29] = var29 - var215;
  vector[30] = var14 - var110;
  vector[31] = var15 - var111;
}

static VOID ixheaace_cos_sim_mod(FLOAT32 *ptr_subband,
                                 ixheaace_pstr_sbr_qmf_filter_bank pstr_qmf_bank) {
  WORD32 i;
  FLOAT32 re1, im1, re2, im2, re3, im3;
  const FLOAT32 *ptr_cos_sin_tab = pstr_qmf_bank->ptr_flt_cos_twiddle;
  const FLOAT32 *ptr_cos_sin_tab2 = &pstr_qmf_bank->ptr_flt_cos_twiddle[31];
  const FLOAT32 *ptr_alt_sin_tab;
  FLOAT32 wim, wre;
  FLOAT32 *ptr_subband2 = ptr_subband + 32;

  for (i = 0; i < 8; i++) {
    wre = *ptr_cos_sin_tab++;
    wim = *ptr_cos_sin_tab++;
    re1 = ptr_subband[2 * i];
    im1 = ptr_subband[31 - 2 * i];
    re3 = ptr_subband[30 - 2 * i];
    im3 = ptr_subband[2 * i + 1];
    ptr_subband[2 * i] = (im1 * wim) + (re1 * wre);
    ptr_subband[2 * i + 1] = (im1 * wre) - (re1 * wim);
    re1 = ptr_subband2[2 * i];
    im1 = ptr_subband2[31 - 2 * i];
    re2 = ptr_subband2[30 - 2 * i];
    im2 = ptr_subband2[2 * i + 1];
    ptr_subband2[2 * i + 1] = (im1 * wre) + (re1 * wim);
    ptr_subband2[2 * i] = (im1 * wim) - (re1 * wre);
    wim = *ptr_cos_sin_tab2--;
    wre = *ptr_cos_sin_tab2--;
    ptr_subband[30 - 2 * i] = (im3 * wim) + (re3 * wre);
    ptr_subband[31 - 2 * i] = (im3 * wre) - (re3 * wim);
    ptr_subband2[31 - 2 * i] = (im2 * wre) + (re2 * wim);
    ptr_subband2[30 - 2 * i] = (im2 * wim) - (re2 * wre);
  }

  ixheaace_fft16(ptr_subband);
  ixheaace_fft16(ptr_subband2);

  ptr_alt_sin_tab = &pstr_qmf_bank->ptr_flt_alt_sin_twiddle[0];
  wim = *ptr_alt_sin_tab++;
  wre = *ptr_alt_sin_tab++;
  for (i = 0; i < 8; i++) {
    re1 = ptr_subband[2 * i];
    im1 = ptr_subband[2 * i + 1];
    re3 = ptr_subband[30 - 2 * i];
    im3 = ptr_subband[31 - 2 * i];
    ptr_subband[2 * i] = (re1 * wre) + (im1 * wim);
    ptr_subband[31 - 2 * i] = (re1 * wim) - (im1 * wre);
    re1 = ptr_subband2[2 * i];
    im1 = ptr_subband2[2 * i + 1];
    re2 = ptr_subband2[30 - 2 * i];
    im2 = ptr_subband2[31 - 2 * i];
    ptr_subband2[31 - 2 * i] = -((re1 * wre) + (im1 * wim));
    ptr_subband2[2 * i] = -((re1 * wim) - (im1 * wre));
    wim = *ptr_alt_sin_tab++;
    wre = *ptr_alt_sin_tab++;
    ptr_subband[30 - 2 * i] = (re3 * wim) + (im3 * wre);
    ptr_subband[2 * i + 1] = (re3 * wre) - (im3 * wim);
    ptr_subband2[2 * i + 1] = -((re2 * wim) + (im2 * wre));
    ptr_subband2[30 - 2 * i] = -(re2 * wre) + (im2 * wim);
  }
}

static VOID ixheaace_inverse_modulation(const FLOAT32 *ptr_sbr_real, const FLOAT32 *ptr_sbr_imag,
                                        FLOAT32 *ptr_time_out,
                                        ixheaace_pstr_sbr_qmf_filter_bank pstr_qmf_bank) {
  WORD32 i;
  FLOAT32 gain = 0.015625f;
  FLOAT32 r1, i1, r2, i2;
  FLOAT32 *ptr_time1, *ptr_time2;
  FLOAT32 *ptr_time3, *ptr_time4;
  const FLOAT32 *ptr_re;
  const FLOAT32 *ptr_im;

  ptr_time1 = &ptr_time_out[0];
  ptr_time2 = &ptr_time_out[32];
  ptr_re = &ptr_sbr_real[0];
  ptr_im = &ptr_sbr_imag[0];

  for (i = 31; i >= 0; i--) {
    r1 = *ptr_re++;
    i1 = *ptr_im++;

    *ptr_time1++ = r1 * gain;
    *ptr_time2++ = i1 * gain;
  }

  ixheaace_cos_sim_mod(ptr_time_out, pstr_qmf_bank);

  ptr_time1 = &ptr_time_out[63];
  ptr_time2 = &ptr_time_out[31];
  ptr_time3 = &ptr_time_out[0];
  ptr_time4 = &ptr_time_out[32];
  for (i = 15; i >= 0; i--) {
    r1 = *ptr_time3;
    i2 = *ptr_time1;
    r2 = *ptr_time2;
    i1 = *ptr_time4;

    *ptr_time3++ = r1 - i1;

    *ptr_time1-- = -(r1 + i1);

    *ptr_time2-- = r2 - i2;

    *ptr_time4++ = -(r2 + i2);
  }
}

static VOID ixheaace_win_add(FLOAT32 *ptr_time_buf, FLOAT32 *ptr_work_buffer,
                             const FLOAT32 *ptr_filter) {
  WORD32 j, k;
  FLOAT32 *ptr_work_buf = &ptr_work_buffer[63];
  for (j = 0, k = 0; j < 64; j += 2, k++) {
    FLOAT32 tmp_var = *ptr_work_buf--;
    FLOAT32 temp_a = ptr_filter[2 * k];
    FLOAT32 temp_b = ptr_filter[2 * k + 1];
    FLOAT32 temp1 = *ptr_work_buf--;
    ptr_time_buf[j] += tmp_var * temp_a;
    ptr_time_buf[j + 1] += temp1 * temp_b;

    temp_a = ptr_filter[64 + 2 * k];
    temp_b = ptr_filter[64 + 2 * k + 1];
    ptr_time_buf[64 + j] += tmp_var * temp_a;
    ptr_time_buf[64 + j + 1] += temp1 * temp_b;

    temp_a = ptr_filter[128 + 2 * k];
    temp_b = ptr_filter[128 + 2 * k + 1];
    ptr_time_buf[128 + j] += tmp_var * temp_a;
    ptr_time_buf[128 + j + 1] += temp1 * temp_b;

    temp_a = ptr_filter[192 + 2 * k];
    temp_b = ptr_filter[192 + 2 * k + 1];
    ptr_time_buf[192 + j] += tmp_var * temp_a;
    ptr_time_buf[192 + j + 1] += temp1 * temp_b;

    temp_a = ptr_filter[256 + 2 * k];
    temp_b = ptr_filter[256 + 2 * k + 1];
    ptr_time_buf[256 + j] += tmp_var * temp_a;
    ptr_time_buf[256 + j + 1] += temp1 * temp_b;
  }
}

VOID ixheaace_enc_synthesis_qmf_filtering(FLOAT32 **ptr_sbr_re, FLOAT32 **ptr_sbr_im,
                                          FLOAT32 *time_float,
                                          ixheaace_pstr_sbr_qmf_filter_bank pstr_qmf_bank) {
  WORD32 k, j;
  FLOAT32 *timeBuf_flt;
  const FLOAT32 *ptr_filter = pstr_qmf_bank->ptr_flt_filter;
  for (k = 0; k < 32; k++) {
    ixheaace_inverse_modulation(*(ptr_sbr_re + k), *(ptr_sbr_im + k),
                                pstr_qmf_bank->ptr_flt_work_buf, pstr_qmf_bank);
    ixheaace_win_add(pstr_qmf_bank->ptr_flt_time_buf, pstr_qmf_bank->ptr_flt_work_buf,
                     ptr_filter);
    timeBuf_flt = &pstr_qmf_bank->ptr_flt_time_buf[288];
    {
      FLOAT32 temp1_flt, temp2_flt;
      FLOAT32 *ptr_time_out_flt = &time_float[31], temp_out_flt;
      for (j = 15; j >= 0; j--) {
        temp1_flt = *timeBuf_flt++;
        temp2_flt = *timeBuf_flt++;
        temp_out_flt = temp1_flt;
        *ptr_time_out_flt-- = temp_out_flt * -1;
        temp_out_flt = temp2_flt;
        *ptr_time_out_flt-- = temp_out_flt * -1;
      }
    }
    time_float += 32;
    memmove(pstr_qmf_bank->ptr_flt_time_buf + 32, pstr_qmf_bank->ptr_flt_time_buf,
            288 * sizeof(pstr_qmf_bank->ptr_flt_time_buf[0]));
    memset(pstr_qmf_bank->ptr_flt_time_buf, 0, 32 * sizeof(pstr_qmf_bank->ptr_flt_time_buf[0]));
  }
}
