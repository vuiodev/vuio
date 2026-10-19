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
#include "ixheaac_type_def.h"
#include "ixheaace_psy_const.h"
#include "ixheaace_tns.h"
#include "ixheaace_tns_params.h"
#include "ixheaace_rom.h"
#include "ixheaace_common_rom.h"
#include "ixheaac_constants.h"
#include "ixheaace_aac_constants.h"

#include "ixheaace_fft.h"

void ia_aac_ld_enc_rearrange(WORD32 *ip, WORD32 *op, WORD32 N, UWORD8 *re_arr_tab) {
  WORD32 n, i = 0;

  for (n = 0; n < N; n++) {
    WORD32 idx = re_arr_tab[n] << 1;

    op[i++] = ip[idx];
    op[i++] = ip[idx + 1];
  }

  return;
}

static VOID ia_enhaacplus_enc_fft15(FLOAT32 *ptr_vec) {
  FLOAT32 r0, r1, r2, r3, r4, r5, r6, r7, r8, r9, r10, r11, r12, r13, r14, r15, r16, r17, i0, i1,
      i2, i3, i4, i5, i6, i7, i8, i9, i10, i11, i12, i13, i14, i15, i16, i17, tmp0, tmp1, tmp2,
      tmp3, tmp4, tmp5, tmp6, tmp7, tmp8, tmp9, tmp10, tmp11, tmp12, tmp13, tmp14, tmp15, tmp16,
      tmp17, tmp18, tmp19, tmp20, tmp21, tmp22, tmp23, tmp24, tmp25, tmp26, tmp27, tmp28, tmp29;

  /* Pre-additions real part */
  r1 = ptr_vec[2] + ptr_vec[8];
  r2 = ptr_vec[2] - ptr_vec[8];
  r3 = ptr_vec[4] + ptr_vec[16];
  r4 = ptr_vec[4] - ptr_vec[16];
  r5 = ptr_vec[6] + ptr_vec[24];
  r6 = ptr_vec[6] - ptr_vec[24];
  r7 = ptr_vec[10] + ptr_vec[20];
  r8 = ptr_vec[10] - ptr_vec[20];
  r9 = ptr_vec[12] + ptr_vec[18];
  r10 = ptr_vec[12] - ptr_vec[18];
  r11 = ptr_vec[14] + ptr_vec[26];
  r12 = ptr_vec[14] - ptr_vec[26];
  r13 = ptr_vec[22] + ptr_vec[28];
  r14 = ptr_vec[22] - ptr_vec[28];

  tmp2 = r1 + r3;
  tmp4 = r1 - r3;
  tmp6 = r2 + r14;
  tmp8 = r2 - r14;
  tmp10 = r4 + r12;
  tmp12 = r4 - r12;
  tmp14 = r5 + r9;
  tmp16 = r5 - r9;
  tmp18 = r11 + r13;
  tmp20 = r11 - r13;

  /* Pre-additions imaginary part */
  i1 = ptr_vec[3] + ptr_vec[9];
  i2 = ptr_vec[3] - ptr_vec[9];
  i3 = ptr_vec[5] + ptr_vec[17];
  i4 = ptr_vec[5] - ptr_vec[17];
  i5 = ptr_vec[7] + ptr_vec[25];
  i6 = ptr_vec[7] - ptr_vec[25];
  i7 = ptr_vec[11] + ptr_vec[21];
  i8 = ptr_vec[11] - ptr_vec[21];
  i9 = ptr_vec[13] + ptr_vec[19];
  i10 = ptr_vec[13] - ptr_vec[19];
  i11 = ptr_vec[15] + ptr_vec[27];
  i12 = ptr_vec[15] - ptr_vec[27];
  i13 = ptr_vec[23] + ptr_vec[29];
  i14 = ptr_vec[23] - ptr_vec[29];

  tmp3 = i1 + i3;
  tmp5 = i1 - i3;
  tmp7 = i2 + i14;
  tmp9 = i2 - i14;
  tmp11 = i4 + i12;
  tmp13 = i4 - i12;
  tmp15 = i5 + i9;
  tmp17 = i5 - i9;
  tmp19 = i11 + i13;
  tmp21 = i11 - i13;

  /* Pre-additions and core multiplications */
  tmp28 = tmp4 + tmp20;
  tmp29 = tmp5 + tmp21;
  r4 = tmp2 + tmp18;
  i4 = tmp3 + tmp19;
  r3 = (FLOAT32)((r4 + tmp14) * -1.25);
  i3 = (FLOAT32)((i4 + tmp15) * -1.25);
  r2 = (FLOAT32)((tmp29 - i8) * -8.660254037844387e-1);
  i2 = (FLOAT32)((tmp28 - r8) * 8.660254037844387e-1);
  r1 = r4 + r7;
  i1 = i4 + i7;
  r0 = r1 + ptr_vec[0] + tmp14;
  i0 = i1 + ptr_vec[1] + tmp15;
  r7 = tmp4 - tmp20;
  i7 = tmp5 - tmp21;
  r8 = (FLOAT32)((tmp3 - tmp19) * -4.841229182759272e-1);
  i8 = (FLOAT32)((tmp2 - tmp18) * 4.841229182759272e-1);
  tmp0 = tmp6 + r10;
  tmp1 = tmp7 + i10;
  tmp2 = r6 - tmp10;
  tmp3 = i6 - tmp11;
  r10 = (FLOAT32)(tmp7 * -2.308262652881440);
  i10 = (FLOAT32)(tmp6 * 2.308262652881440);
  r11 = (FLOAT32)(tmp8 * 1.332676064001459);
  i11 = (FLOAT32)(tmp9 * 1.332676064001459);
  r6 = (FLOAT32)((r7 - tmp16) * 5.590169943749475e-1);
  i6 = (FLOAT32)((i7 - tmp17) * 5.590169943749475e-1);
  r12 = (FLOAT32)((tmp1 + tmp3) * 5.877852522924733e-1);
  i12 = (FLOAT32)((tmp0 + tmp2) * -5.877852522924733e-1);
  r13 = (FLOAT32)((tmp7 - tmp11) * -8.816778784387098e-1);
  i13 = (FLOAT32)((tmp6 - tmp10) * 8.816778784387098e-1);
  r14 = (FLOAT32)((tmp8 + tmp12) * 5.090369604551274e-1);
  i14 = (FLOAT32)((tmp9 + tmp13) * 5.090369604551274e-1);
  r16 = (FLOAT32)(tmp11 * 5.449068960040204e-1);
  i16 = (FLOAT32)(tmp10 * -5.449068960040204e-1);
  r17 = (FLOAT32)(tmp12 * 3.146021430912046e-1);
  i17 = (FLOAT32)(tmp13 * 3.146021430912046e-1);

  r4 *= 1.875;
  i4 *= 1.875;
  r1 *= -1.5;
  i1 *= -1.5;
  r7 *= (FLOAT32)(-8.385254915624212e-1);
  i7 *= (FLOAT32)(-8.385254915624212e-1);
  r5 = (FLOAT32)(tmp29 * 1.082531754730548);
  i5 = (FLOAT32)(tmp28 * -1.082531754730548);
  r9 = (FLOAT32)(tmp1 * 1.5388417685876270);
  i9 = (FLOAT32)(tmp0 * -1.538841768587627);
  r15 = (FLOAT32)(tmp3 * 3.632712640026803e-1);
  i15 = (FLOAT32)(tmp2 * -3.632712640026803e-1);

  /* Post-additions real part */
  tmp2 = r0 + r1;
  tmp4 = r3 + r6;
  tmp6 = r3 - r6;
  tmp8 = r4 + r5;
  tmp10 = r4 - r5;
  tmp12 = r7 + r8;
  tmp14 = r7 - r8;
  tmp16 = r13 + r16;
  tmp18 = r14 + r17;
  tmp20 = r10 - r13;
  tmp22 = r11 - r14;
  tmp24 = r12 + r15;
  tmp26 = r12 - r9;

  r1 = tmp2 + r2;
  r2 = tmp2 - r2;
  r3 = tmp4 + tmp26;
  r4 = tmp4 - tmp26;
  r5 = tmp6 + tmp24;
  r6 = tmp6 - tmp24;
  r7 = tmp16 + tmp18;
  r8 = tmp16 - tmp18;
  r9 = tmp20 - tmp22;
  r10 = tmp20 + tmp22;
  r11 = r1 + tmp8;
  r12 = r2 + tmp10;
  r13 = r11 - tmp12;
  r14 = r12 - tmp14;
  r15 = r12 + tmp14;
  r16 = r11 + tmp12;

  /* Post-additions imaginary part */
  tmp3 = i0 + i1;
  tmp5 = i3 + i6;
  tmp7 = i3 - i6;
  tmp9 = i4 + i5;
  tmp11 = i4 - i5;
  tmp13 = i7 + i8;
  tmp15 = i7 - i8;
  tmp17 = i13 + i16;
  tmp19 = i14 + i17;
  tmp21 = i10 - i13;
  tmp23 = i11 - i14;
  tmp25 = i12 + i15;
  tmp27 = i12 - i9;

  i1 = tmp3 + i2;
  i2 = tmp3 - i2;
  i3 = tmp5 + tmp27;
  i4 = tmp5 - tmp27;
  i5 = tmp7 + tmp25;
  i6 = tmp7 - tmp25;
  i7 = tmp17 + tmp19;
  i8 = tmp17 - tmp19;
  i9 = tmp21 - tmp23;
  i10 = tmp21 + tmp23;
  i11 = i1 + tmp9;
  i12 = i2 + tmp11;
  i13 = i11 - tmp13;
  i14 = i12 - tmp15;
  i15 = i12 + tmp15;
  i16 = i11 + tmp13;

  *ptr_vec++ = r0;
  *ptr_vec++ = i0;
  *ptr_vec++ = r13 + r5 + r7;
  *ptr_vec++ = i13 + i5 + i7;
  *ptr_vec++ = r15 + r3 - r9;
  *ptr_vec++ = i15 + i3 - i9;
  *ptr_vec++ = r0 + r4;
  *ptr_vec++ = i0 + i4;
  *ptr_vec++ = r13 + r6 - r7;
  *ptr_vec++ = i13 + i6 - i7;
  *ptr_vec++ = r2;
  *ptr_vec++ = i2;
  *ptr_vec++ = r0 + r5;
  *ptr_vec++ = i0 + i5;
  *ptr_vec++ = r16 + r3 - r10;
  *ptr_vec++ = i16 + i3 - i10;
  *ptr_vec++ = r15 + r4 + r9;
  *ptr_vec++ = i15 + i4 + i9;
  *ptr_vec++ = r0 + r6;
  *ptr_vec++ = i0 + i6;
  *ptr_vec++ = r1;
  *ptr_vec++ = i1;
  *ptr_vec++ = r14 + r5 + r8;
  *ptr_vec++ = i14 + i5 + i8;
  *ptr_vec++ = r0 + r3;
  *ptr_vec++ = i0 + i3;
  *ptr_vec++ = r16 + r4 + r10;
  *ptr_vec++ = i16 + i4 + i10;
  *ptr_vec++ = r14 + r6 - r8;
  *ptr_vec++ = i14 + i6 - i8;
}

static VOID ia_enhaacplus_enc_fft16(FLOAT32 *ptr_vec) {
  FLOAT32 var10, var11, var12, var13, var14, var15, var16, var17, var18, var19, var110, var111,
      var112, var113, var114, var115, var20, var21, var22, var23, var24, var25, var26, var27,
      var28, var29, var210, var211, var212, var213, var214, var215, arr0, arr1, arr2, arr3, arr4,
      arr5, arr6, arr7, arr8, arr9, arr10, arr11, arr12, arr13, arr14, arr15;

  /* Pre-additions */
  arr0 = ptr_vec[0] + ptr_vec[16];
  arr8 = ptr_vec[8] + ptr_vec[24];
  var10 = arr0 + arr8;
  var12 = arr0 - arr8;
  arr1 = ptr_vec[1] + ptr_vec[17];
  arr9 = ptr_vec[9] + ptr_vec[25];
  var11 = arr1 + arr9;
  var13 = arr1 - arr9;
  arr2 = ptr_vec[2] + ptr_vec[18];
  arr10 = ptr_vec[10] + ptr_vec[26];
  var14 = arr2 + arr10;
  var16 = arr2 - arr10;
  arr3 = ptr_vec[3] + ptr_vec[19];
  arr11 = ptr_vec[11] + ptr_vec[27];
  var15 = arr3 + arr11;
  var17 = arr3 - arr11;
  arr4 = ptr_vec[4] + ptr_vec[20];
  arr12 = ptr_vec[12] + ptr_vec[28];
  var18 = arr4 + arr12;
  var110 = arr4 - arr12;
  arr5 = ptr_vec[5] + ptr_vec[21];
  arr13 = ptr_vec[13] + ptr_vec[29];
  var19 = arr5 + arr13;
  var111 = arr5 - arr13;
  arr6 = ptr_vec[6] + ptr_vec[22];
  arr14 = ptr_vec[14] + ptr_vec[30];
  var112 = arr6 + arr14;
  var114 = arr6 - arr14;
  arr7 = ptr_vec[7] + ptr_vec[23];
  arr15 = ptr_vec[15] + ptr_vec[31];
  var113 = arr7 + arr15;
  var115 = arr7 - arr15;

  /* Pre-additions and core multiplications */
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

  /* odd */
  arr0 = ptr_vec[0] - ptr_vec[16];
  arr1 = ptr_vec[1] - ptr_vec[17];
  arr2 = ptr_vec[2] - ptr_vec[18];
  arr3 = ptr_vec[3] - ptr_vec[19];
  arr4 = ptr_vec[4] - ptr_vec[20];
  arr5 = ptr_vec[5] - ptr_vec[21];
  arr6 = ptr_vec[6] - ptr_vec[22];
  arr7 = ptr_vec[7] - ptr_vec[23];
  arr8 = ptr_vec[8] - ptr_vec[24];
  arr9 = ptr_vec[9] - ptr_vec[25];
  arr10 = ptr_vec[10] - ptr_vec[26];
  arr11 = ptr_vec[11] - ptr_vec[27];
  arr12 = ptr_vec[12] - ptr_vec[28];
  arr13 = ptr_vec[13] - ptr_vec[29];
  arr14 = ptr_vec[14] - ptr_vec[30];
  arr15 = ptr_vec[15] - ptr_vec[31];

  /* Pre-additions and core multiplications */
  var19 = (arr2 + arr14) * -IXHEAACE_COS_3PI_DIV8;
  var110 = (arr2 - arr14) * IXHEAACE_COS_PI_DIV8;
  var18 = (arr3 + arr15) * IXHEAACE_COS_3PI_DIV8;
  var111 = (arr3 - arr15) * IXHEAACE_COS_PI_DIV8;
  var15 = (arr4 + arr12) * -IXHEAACE_INV_SQRT2;
  var16 = (arr4 - arr12) * IXHEAACE_INV_SQRT2;
  var14 = (arr5 + arr13) * IXHEAACE_INV_SQRT2;
  var17 = (arr5 - arr13) * IXHEAACE_INV_SQRT2;
  var113 = (arr6 + arr10) * -IXHEAACE_COS_PI_DIV8;
  var114 = (arr6 - arr10) * IXHEAACE_COS_3PI_DIV8;
  var112 = (arr7 + arr11) * IXHEAACE_COS_PI_DIV8;
  var115 = (arr7 - arr11) * IXHEAACE_COS_3PI_DIV8;

  /* Core multiplications */
  arr2 = var18 * IXHEAACE_SQRT2PLUS1 - var112 * IXHEAACE_SQRT2MINUS1;
  arr3 = var19 * IXHEAACE_SQRT2PLUS1 - var113 * IXHEAACE_SQRT2MINUS1;
  arr4 = var110 * IXHEAACE_SQRT2MINUS1 - var114 * IXHEAACE_SQRT2PLUS1;
  arr5 = var111 * IXHEAACE_SQRT2MINUS1 - var115 * IXHEAACE_SQRT2PLUS1;

  /* Post-additions */
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

  var112 = arr2 + arr4;
  var114 = arr2 - arr4;
  var113 = arr3 + arr5;
  var115 = arr3 - arr5;

  /* Post-additions */
  ptr_vec[0] = var20 + var22;
  ptr_vec[1] = var21 + var23;
  ptr_vec[2] = var12 + arr10;
  ptr_vec[3] = var13 + arr11;
  ptr_vec[4] = var210 + var212;
  ptr_vec[5] = var211 + var213;
  ptr_vec[6] = var10 + var112;
  ptr_vec[7] = var11 + var113;
  ptr_vec[8] = var24 - var26;
  ptr_vec[9] = var25 - var27;
  ptr_vec[10] = var16 + var114;
  ptr_vec[11] = var17 + var115;
  ptr_vec[12] = var28 + var214;
  ptr_vec[13] = var29 + var215;
  ptr_vec[14] = var14 + var110;
  ptr_vec[15] = var15 + var111;
  ptr_vec[16] = var20 - var22;
  ptr_vec[17] = var21 - var23;
  ptr_vec[18] = var12 - arr10;
  ptr_vec[19] = var13 - arr11;
  ptr_vec[20] = var210 - var212;
  ptr_vec[21] = var211 - var213;
  ptr_vec[22] = var10 - var112;
  ptr_vec[23] = var11 - var113;
  ptr_vec[24] = var24 + var26;
  ptr_vec[25] = var25 + var27;
  ptr_vec[26] = var16 - var114;
  ptr_vec[27] = var17 - var115;
  ptr_vec[28] = var28 - var214;
  ptr_vec[29] = var29 - var215;
  ptr_vec[30] = var14 - var110;
  ptr_vec[31] = var15 - var111;
}

static VOID ia_enhaacplus_enc_fft240(FLOAT32 *ptr_in) {
  const WORD32 n1 = 240;
  const WORD32 n2 = 15;
  const WORD32 n3 = 16;
  const WORD32 *ptr_idx1 = ia_enhaacplus_enc_fft240_table1;
  const WORD32 *ptr_idx2 = ia_enhaacplus_enc_fft240_table2;

  WORD32 k, l;
  FLOAT32 temp[32], out[480];

  for (k = 0; k < n2; k++) {
    for (l = 0; l < n3; l++) {
      temp[2 * l] = ptr_in[2 * *ptr_idx1];
      temp[2 * l + 1] = ptr_in[2 * *ptr_idx1 + 1];
      ptr_idx1 += n2;
    }

    ia_enhaacplus_enc_fft16(temp); /* 16-point FFT */
    ptr_idx1 -= n1;

    for (l = 0; l < n3; l++) {
      ptr_in[2 * *ptr_idx1] = temp[2 * l];
      ptr_in[2 * *ptr_idx1 + 1] = temp[2 * l + 1];
      ptr_idx1 += n2;
    }

    ptr_idx1 -= n1 - 1;
  }

  ptr_idx1 -= n2;

  for (k = 0; k < n3; k++) {
    for (l = 0; l < n2; l++) {
      temp[2 * l] = ptr_in[2 * *ptr_idx1];
      temp[2 * l + 1] = ptr_in[2 * *ptr_idx1++ + 1];
    }

    ia_enhaacplus_enc_fft15(temp); /* 15-point FFT */

    for (l = 0; l < n2; l++) {
      out[2 * *ptr_idx2] = temp[2 * l];
      out[2 * *ptr_idx2++ + 1] = temp[2 * l + 1];
    }
  }

  memcpy(ptr_in, out, (2 * n1) * sizeof(out[0]));
}

VOID ia_aac_ld_enc_mdct_480(FLOAT32 *ptr_inp, FLOAT32 *ptr_scratch, WORD32 mdct_flag,
                            ixheaace_mdct_tables *pstr_mdct_tables) {
  WORD32 k;
  FLOAT32 const_mltfac = ((FLOAT32)FRAME_LEN_512) / FRAME_LEN_480;

  ia_eaacp_enc_pre_twiddle_aac(ptr_scratch, ptr_inp, FRAME_LEN_480,
                               pstr_mdct_tables->cosine_array_960);

  ia_enhaacplus_enc_fft240(ptr_scratch);

  ia_enhaacplus_enc_post_twiddle(ptr_inp, ptr_scratch, pstr_mdct_tables->cosine_array_960,
                                 FRAME_LEN_480);

  if (0 == mdct_flag) {
    for (k = 0; k < MDCT_LEN; k++) {
      ptr_inp[k] *= const_mltfac;
    }
  }
}
