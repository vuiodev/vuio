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
#include <string.h>
#include "ixheaac_type_def.h"
#include "ixheaacd_mps_struct_def.h"
#include "ixheaacd_error_codes.h"
#include "ixheaacd_mps_res_rom.h"
#include "ixheaacd_mps_aac_struct.h"
#include "ixheaac_constants.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaacd_bitbuffer.h"
#include "ixheaacd_common_rom.h"
#include "ixheaacd_sbrdecsettings.h"
#include "ixheaacd_sbr_scale.h"
#include "ixheaacd_env_extr_part.h"
#include "ixheaacd_sbr_rom.h"
#include "ixheaacd_hybrid.h"
#include "ixheaacd_ps_dec.h"
#include "ixheaacd_mps_polyphase.h"
#include "ixheaacd_config.h"
#include "ixheaacd_qmf_dec.h"
#include "ixheaacd_mps_dec.h"
#include "ixheaacd_mps_defines.h"
#include "ixheaacd_mps_macro_def.h"
#include "ixheaacd_mps_basic_op.h"
#include "ixheaac_error_standards.h"

static VOID ixheaacd_fft32(WORD32 *vec, const WORD16 *fft_c) {
  WORD32 tmp0, tmp1, tmp2, tmp3, tmp4, tmp5, tmp6, tmp7, tmp8, tmp9, tmp10, tmp11, tmp12, tmp13,
      tmp14, tmp15;
  WORD32 temp10, temp11, temp12, temp13, temp14, temp15, temp16, temp17, temp18, temp19, temp110,
      temp111, temp112, temp113, temp114, temp115;
  WORD32 temp20, temp21, temp22, temp23, temp24, temp25, temp26, temp27, temp28, temp29, temp210,
      temp211, temp212, temp213, temp214, temp215;
  WORD32 temp30, temp31, temp32, temp33, temp34, temp35, temp36, temp37, temp38, temp39, temp310,
      temp311, temp312, temp313, temp314, temp315;
  WORD32 temp316, temp317, temp318, temp319, temp320, temp321, temp322, temp323, temp324, temp325,
      temp326, temp327, temp328, temp329, temp330, temp331;
  WORD32 temp40, temp41, temp42, temp43, temp44, temp45, temp46, temp47, temp48, temp49, temp410,
      temp411, temp412, temp413, temp414, temp415;

  temp20 = ixheaac_sub32_sat(vec[2], vec[34]);
  temp21 = ixheaac_sub32_sat(vec[3], vec[35]);
  temp30 = ixheaac_add32_sat(vec[0], vec[32]);
  temp31 = ixheaac_add32_sat(vec[1], vec[33]);
  temp32 = ixheaac_add32_sat(vec[2], vec[34]);
  temp33 = ixheaac_add32_sat(vec[3], vec[35]);

  temp22 = ixheaac_sub32_sat(vec[6], vec[38]);
  temp23 = ixheaac_sub32_sat(vec[7], vec[39]);
  temp34 = ixheaac_add32_sat(vec[4], vec[36]);
  temp35 = ixheaac_add32_sat(vec[5], vec[37]);
  temp36 = ixheaac_add32_sat(vec[6], vec[38]);
  temp37 = ixheaac_add32_sat(vec[7], vec[39]);

  temp24 = ixheaac_sub32_sat(vec[10], vec[42]);
  temp25 = ixheaac_sub32_sat(vec[11], vec[43]);
  temp38 = ixheaac_add32_sat(vec[8], vec[40]);
  temp39 = ixheaac_add32_sat(vec[9], vec[41]);
  temp310 = ixheaac_add32_sat(vec[10], vec[42]);
  temp311 = ixheaac_add32_sat(vec[11], vec[43]);

  temp26 = ixheaac_sub32_sat(vec[14], vec[46]);
  temp27 = ixheaac_sub32_sat(vec[15], vec[47]);
  temp312 = ixheaac_add32_sat(vec[12], vec[44]);
  temp313 = ixheaac_add32_sat(vec[13], vec[45]);
  temp314 = ixheaac_add32_sat(vec[14], vec[46]);
  temp315 = ixheaac_add32_sat(vec[15], vec[47]);

  temp28 = ixheaac_sub32_sat(vec[18], vec[50]);
  temp29 = ixheaac_sub32_sat(vec[19], vec[51]);
  temp316 = ixheaac_add32_sat(vec[16], vec[48]);
  temp317 = ixheaac_add32_sat(vec[17], vec[49]);
  temp318 = ixheaac_add32_sat(vec[18], vec[50]);
  temp319 = ixheaac_add32_sat(vec[19], vec[51]);

  temp210 = ixheaac_sub32_sat(vec[22], vec[54]);
  temp211 = ixheaac_sub32_sat(vec[23], vec[55]);
  temp320 = ixheaac_add32_sat(vec[20], vec[52]);
  temp321 = ixheaac_add32_sat(vec[21], vec[53]);
  temp322 = ixheaac_add32_sat(vec[22], vec[54]);
  temp323 = ixheaac_add32_sat(vec[23], vec[55]);

  temp212 = ixheaac_sub32_sat(vec[26], vec[58]);
  temp213 = ixheaac_sub32_sat(vec[27], vec[59]);
  temp324 = ixheaac_add32_sat(vec[24], vec[56]);
  temp325 = ixheaac_add32_sat(vec[25], vec[57]);
  temp326 = ixheaac_add32_sat(vec[26], vec[58]);
  temp327 = ixheaac_add32_sat(vec[27], vec[59]);

  temp214 = ixheaac_sub32_sat(vec[30], vec[62]);
  temp215 = ixheaac_sub32_sat(vec[31], vec[63]);
  temp328 = ixheaac_add32_sat(vec[28], vec[60]);
  temp329 = ixheaac_add32_sat(vec[29], vec[61]);
  temp330 = ixheaac_add32_sat(vec[30], vec[62]);
  temp331 = ixheaac_add32_sat(vec[31], vec[63]);

  temp41 = ixheaac_negate32_sat(ixheaac_add32_sat(temp20, temp214));
  temp42 = ixheaac_sub32_sat(temp20, temp214);
  temp40 = ixheaac_add32_sat(temp21, temp215);
  temp43 = ixheaac_sub32_sat(temp21, temp215);
  temp45 = ixheaac_negate32_sat(ixheaac_add32_sat(temp22, temp212));
  temp46 = ixheaac_sub32_sat(temp22, temp212);
  temp44 = ixheaac_add32_sat(temp23, temp213);
  temp47 = ixheaac_sub32_sat(temp23, temp213);
  temp49 = ixheaac_negate32_sat(ixheaac_add32_sat(temp24, temp210));
  temp410 = ixheaac_sub32_sat(temp24, temp210);
  temp48 = ixheaac_add32_sat(temp25, temp211);
  temp411 = ixheaac_sub32_sat(temp25, temp211);
  temp413 = ixheaac_negate32_sat(ixheaac_add32_sat(temp26, temp28));
  temp414 = ixheaac_sub32_sat(temp26, temp28);
  temp412 = ixheaac_add32_sat(temp27, temp29);
  temp415 = ixheaac_sub32_sat(temp27, temp29);

  temp20 = ixheaac_add32_sat(ixheaac_add32_sat(ixheaac_add32_sat(
           ixheaac_mult32x16in32_shl(temp40, fft_c[3]),
           ixheaac_mult32x16in32_shl(temp44, fft_c[2])),
           ixheaac_mult32x16in32_shl(temp48, fft_c[1])),
           ixheaac_mult32x16in32_shl(temp412, fft_c[0]));

  temp24 = ixheaac_sub32_sat(ixheaac_add32_sat(ixheaac_add32_sat(
           ixheaac_mult32x16in32_shl(temp40, fft_c[2]),
           ixheaac_mult32x16in32_shl(temp44, fft_c[0])),
           ixheaac_mult32x16in32_shl(temp48, fft_c[3])),
           ixheaac_mult32x16in32_shl(temp412, fft_c[1]));

  temp28 = ixheaac_add32_sat(ixheaac_sub32_sat(ixheaac_add32_sat(
           ixheaac_mult32x16in32_shl(temp40, fft_c[1]),
           ixheaac_mult32x16in32_shl(temp44, fft_c[3])),
           ixheaac_mult32x16in32_shl(temp48, fft_c[0])),
           ixheaac_mult32x16in32_shl(temp412, fft_c[2]));

  temp212 = ixheaac_sub32_sat(ixheaac_add32_sat(ixheaac_sub32_sat(
            ixheaac_mult32x16in32_shl(temp40, fft_c[0]),
            ixheaac_mult32x16in32_shl(temp44, fft_c[1])),
            ixheaac_mult32x16in32_shl(temp48, fft_c[2])),
            ixheaac_mult32x16in32_shl(temp412, fft_c[3]));

  temp21 = ixheaac_add32_sat(ixheaac_add32_sat(ixheaac_add32_sat(
           ixheaac_mult32x16in32_shl(temp41, fft_c[3]),
           ixheaac_mult32x16in32_shl(temp45, fft_c[2])),
           ixheaac_mult32x16in32_shl(temp49, fft_c[1])),
           ixheaac_mult32x16in32_shl(temp413, fft_c[0]));

  temp25 = ixheaac_sub32_sat(ixheaac_add32_sat(ixheaac_add32_sat(
           ixheaac_mult32x16in32_shl(temp41, fft_c[2]),
           ixheaac_mult32x16in32_shl(temp45, fft_c[0])),
           ixheaac_mult32x16in32_shl(temp49, fft_c[3])),
           ixheaac_mult32x16in32_shl(temp413, fft_c[1]));

  temp29 = ixheaac_add32_sat(ixheaac_sub32_sat(ixheaac_add32_sat(
           ixheaac_mult32x16in32_shl(temp41, fft_c[1]),
           ixheaac_mult32x16in32_shl(temp45, fft_c[3])),
           ixheaac_mult32x16in32_shl(temp49, fft_c[0])),
           ixheaac_mult32x16in32_shl(temp413, fft_c[2]));

  temp213 = ixheaac_sub32_sat(ixheaac_add32_sat(ixheaac_sub32_sat(
            ixheaac_mult32x16in32_shl(temp41, fft_c[0]),
            ixheaac_mult32x16in32_shl(temp45, fft_c[1])),
            ixheaac_mult32x16in32_shl(temp49, fft_c[2])),
            ixheaac_mult32x16in32_shl(temp413, fft_c[3]));

  temp22 = ixheaac_add32_sat(ixheaac_add32_sat(ixheaac_add32_sat(
           ixheaac_mult32x16in32_shl(temp42, fft_c[0]),
           ixheaac_mult32x16in32_shl(temp46, fft_c[1])),
           ixheaac_mult32x16in32_shl(temp410, fft_c[2])),
           ixheaac_mult32x16in32_shl(temp414, fft_c[3]));

  temp26 = ixheaac_sub32_sat(ixheaac_sub32_sat(ixheaac_sub32_sat(
           ixheaac_mult32x16in32_shl(temp42, fft_c[1]),
           ixheaac_mult32x16in32_shl(temp46, fft_c[3])),
           ixheaac_mult32x16in32_shl(temp410, fft_c[0])),
           ixheaac_mult32x16in32_shl(temp414, fft_c[2]));

  temp210 = ixheaac_add32_sat(ixheaac_add32_sat(ixheaac_sub32_sat(
            ixheaac_mult32x16in32_shl(temp42, fft_c[2]),
            ixheaac_mult32x16in32_shl(temp46, fft_c[0])),
            ixheaac_mult32x16in32_shl(temp410, fft_c[3])),
            ixheaac_mult32x16in32_shl(temp414, fft_c[1]));

  temp214 = ixheaac_sub32_sat(ixheaac_add32_sat(ixheaac_sub32_sat(
            ixheaac_mult32x16in32_shl(temp42, fft_c[3]),
            ixheaac_mult32x16in32_shl(temp46, fft_c[2])),
            ixheaac_mult32x16in32_shl(temp410, fft_c[1])),
            ixheaac_mult32x16in32_shl(temp414, fft_c[0]));

  temp23 = ixheaac_add32_sat(ixheaac_add32_sat(ixheaac_add32_sat(
           ixheaac_mult32x16in32_shl(temp43, fft_c[0]),
           ixheaac_mult32x16in32_shl(temp47, fft_c[1])),
           ixheaac_mult32x16in32_shl(temp411, fft_c[2])),
           ixheaac_mult32x16in32_shl(temp415, fft_c[3]));

  temp27 = ixheaac_sub32_sat(ixheaac_sub32_sat(ixheaac_sub32_sat(
           ixheaac_mult32x16in32_shl(temp43, fft_c[1]),
           ixheaac_mult32x16in32_shl(temp47, fft_c[3])),
           ixheaac_mult32x16in32_shl(temp411, fft_c[0])),
           ixheaac_mult32x16in32_shl(temp415, fft_c[2]));

  temp211 = ixheaac_add32_sat(ixheaac_add32_sat(ixheaac_sub32_sat(
            ixheaac_mult32x16in32_shl(temp43, fft_c[2]),
            ixheaac_mult32x16in32_shl(temp47, fft_c[0])),
            ixheaac_mult32x16in32_shl(temp411, fft_c[3])),
            ixheaac_mult32x16in32_shl(temp415, fft_c[1]));

  temp215 = ixheaac_sub32_sat(ixheaac_add32_sat(ixheaac_sub32_sat(
            ixheaac_mult32x16in32_shl(temp43, fft_c[3]),
            ixheaac_mult32x16in32_shl(temp47, fft_c[2])),
            ixheaac_mult32x16in32_shl(temp411, fft_c[1])),
            ixheaac_mult32x16in32_shl(temp415, fft_c[0]));

  temp40 = ixheaac_add32_sat(temp20, temp22);
  temp414 = ixheaac_sub32_sat(temp20, temp22);
  temp41 = ixheaac_add32_sat(temp21, temp23);
  temp415 = ixheaac_sub32_sat(temp21, temp23);
  temp42 = ixheaac_add32_sat(temp24, temp26);
  temp412 = ixheaac_sub32_sat(temp24, temp26);
  temp43 = ixheaac_add32_sat(temp25, temp27);
  temp413 = ixheaac_sub32_sat(temp25, temp27);
  temp44 = ixheaac_add32_sat(temp28, temp210);
  temp410 = ixheaac_sub32_sat(temp28, temp210);
  temp45 = ixheaac_add32_sat(temp29, temp211);
  temp411 = ixheaac_sub32_sat(temp29, temp211);
  temp46 = ixheaac_add32_sat(temp212, temp214);
  temp48 = ixheaac_sub32_sat(temp212, temp214);
  temp47 = ixheaac_add32_sat(temp213, temp215);
  temp49 = ixheaac_sub32_sat(temp213, temp215);

  temp10 = ixheaac_add32_sat(temp30, temp316);
  temp11 = ixheaac_add32_sat(temp31, temp317);
  temp12 = ixheaac_add32_sat(temp32, temp318);
  temp13 = ixheaac_add32_sat(temp33, temp319);
  temp14 = ixheaac_add32_sat(temp34, temp320);
  temp15 = ixheaac_add32_sat(temp35, temp321);
  temp16 = ixheaac_add32_sat(temp36, temp322);
  temp17 = ixheaac_add32_sat(temp37, temp323);
  temp18 = ixheaac_add32_sat(temp38, temp324);
  temp19 = ixheaac_add32_sat(temp39, temp325);
  temp110 = ixheaac_add32_sat(temp310, temp326);
  temp111 = ixheaac_add32_sat(temp311, temp327);
  temp112 = ixheaac_add32_sat(temp312, temp328);
  temp113 = ixheaac_add32_sat(temp313, temp329);
  temp114 = ixheaac_add32_sat(temp314, temp330);
  temp115 = ixheaac_add32_sat(temp315, temp331);

  tmp0 = ixheaac_add32_sat(temp10, temp18);
  tmp2 = ixheaac_sub32_sat(temp10, temp18);
  tmp1 = ixheaac_add32_sat(temp11, temp19);
  tmp3 = ixheaac_sub32_sat(temp11, temp19);
  tmp4 = ixheaac_add32_sat(temp12, temp110);
  tmp6 = ixheaac_sub32_sat(temp12, temp110);
  tmp5 = ixheaac_add32_sat(temp13, temp111);
  tmp7 = ixheaac_sub32_sat(temp13, temp111);
  tmp8 = ixheaac_add32_sat(temp14, temp112);
  tmp10 = ixheaac_sub32_sat(temp14, temp112);
  tmp9 = ixheaac_add32_sat(temp15, temp113);
  tmp11 = ixheaac_sub32_sat(temp15, temp113);
  tmp12 = ixheaac_add32_sat(temp16, temp114);
  tmp14 = ixheaac_sub32_sat(temp16, temp114);
  tmp13 = ixheaac_add32_sat(temp17, temp115);
  tmp15 = ixheaac_sub32_sat(temp17, temp115);

  temp20 = ixheaac_add32_sat(tmp0, tmp8);
  temp24 = ixheaac_sub32_sat(tmp0, tmp8);
  temp21 = ixheaac_add32_sat(tmp1, tmp9);
  temp25 = ixheaac_sub32_sat(tmp1, tmp9);
  temp28 = ixheaac_sub32_sat(tmp2, tmp11);
  temp210 = ixheaac_add32_sat(tmp2, tmp11);
  temp29 = ixheaac_add32_sat(tmp3, tmp10);
  temp211 = ixheaac_sub32_sat(tmp3, tmp10);
  temp22 = ixheaac_add32_sat(tmp4, tmp12);
  temp27 = ixheaac_sub32_sat(tmp4, tmp12);
  temp23 = ixheaac_add32_sat(tmp5, tmp13);
  temp26 = ixheaac_sub32_sat(tmp13, tmp5);

  tmp1 = ixheaac_add32_sat(tmp6, tmp14);
  tmp2 = ixheaac_sub32_sat(tmp6, tmp14);
  tmp0 = ixheaac_add32_sat(tmp7, tmp15);
  tmp3 = ixheaac_sub32_sat(tmp7, tmp15);

  temp212 = ixheaac_mult32x16in32_shl(ixheaac_add32_sat(tmp0, tmp2), INV_SQRT2_Q15);
  temp214 = ixheaac_mult32x16in32_shl(ixheaac_sub32_sat(tmp0, tmp2), INV_SQRT2_Q15);
  temp213 = ixheaac_mult32x16in32_shl(ixheaac_sub32_sat(tmp3, tmp1), INV_SQRT2_Q15);
  temp215 = ixheaac_mult32x16in32_shl(ixheaac_add32_sat(tmp1, tmp3), -INV_SQRT2_Q15);

  temp10 = ixheaac_sub32_sat(temp30, temp316);
  temp11 = ixheaac_sub32_sat(temp31, temp317);
  temp12 = ixheaac_sub32_sat(temp32, temp318);
  temp13 = ixheaac_sub32_sat(temp33, temp319);
  temp14 = ixheaac_sub32_sat(temp34, temp320);
  temp15 = ixheaac_sub32_sat(temp35, temp321);
  temp16 = ixheaac_sub32_sat(temp36, temp322);
  temp17 = ixheaac_sub32_sat(temp37, temp323);
  temp18 = ixheaac_sub32_sat(temp38, temp324);
  temp19 = ixheaac_sub32_sat(temp39, temp325);
  temp110 = ixheaac_sub32_sat(temp310, temp326);
  temp111 = ixheaac_sub32_sat(temp311, temp327);
  temp112 = ixheaac_sub32_sat(temp312, temp328);
  temp113 = ixheaac_sub32_sat(temp313, temp329);
  temp114 = ixheaac_sub32_sat(temp314, temp330);
  temp115 = ixheaac_sub32_sat(temp315, temp331);

  temp30 = ixheaac_add32_sat(temp20, temp22);
  temp316 = ixheaac_sub32_sat(temp20, temp22);
  temp31 = ixheaac_add32_sat(temp21, temp23);
  temp317 = ixheaac_sub32_sat(temp21, temp23);
  temp38 = ixheaac_sub32_sat(temp24, temp26);
  temp324 = ixheaac_add32_sat(temp24, temp26);
  temp39 = ixheaac_sub32_sat(temp25, temp27);
  temp325 = ixheaac_add32_sat(temp25, temp27);
  temp312 = ixheaac_add32_sat(temp28, temp214);
  temp328 = ixheaac_sub32_sat(temp28, temp214);
  temp313 = ixheaac_add32_sat(temp29, temp215);
  temp329 = ixheaac_sub32_sat(temp29, temp215);
  temp34 = ixheaac_add32_sat(temp210, temp212);
  temp320 = ixheaac_sub32_sat(temp210, temp212);
  temp35 = ixheaac_add32_sat(temp211, temp213);
  temp321 = ixheaac_sub32_sat(temp211, temp213);

  tmp9 = ixheaac_mult32x16in32_shl(ixheaac_add32_sat(temp12, temp114), -COS_3PI_BY_8_Q15);
  tmp10 = ixheaac_mult32x16in32_shl(ixheaac_sub32_sat(temp12, temp114), COS_PI_BY_8_Q15);
  tmp8 = ixheaac_mult32x16in32_shl(ixheaac_add32_sat(temp13, temp115), COS_3PI_BY_8_Q15);
  tmp11 = ixheaac_mult32x16in32_shl(ixheaac_sub32_sat(temp13, temp115), COS_PI_BY_8_Q15);
  tmp5 = ixheaac_mult32x16in32_shl(ixheaac_add32_sat(temp14, temp112), -INV_SQRT2_Q15);
  tmp6 = ixheaac_mult32x16in32_shl(ixheaac_sub32_sat(temp14, temp112), INV_SQRT2_Q15);
  tmp4 = ixheaac_mult32x16in32_shl(ixheaac_add32_sat(temp15, temp113), INV_SQRT2_Q15);
  tmp7 = ixheaac_mult32x16in32_shl(ixheaac_sub32_sat(temp15, temp113), INV_SQRT2_Q15);
  tmp13 = ixheaac_mult32x16in32_shl(ixheaac_add32_sat(temp16, temp110), -COS_PI_BY_8_Q15);
  tmp14 = ixheaac_mult32x16in32_shl(ixheaac_sub32_sat(temp16, temp110), COS_3PI_BY_8_Q15);
  tmp12 = ixheaac_mult32x16in32_shl(ixheaac_add32_sat(temp17, temp111), COS_PI_BY_8_Q15);
  tmp15 = ixheaac_mult32x16in32_shl(ixheaac_sub32_sat(temp17, temp111), COS_3PI_BY_8_Q15);

  temp12 = ixheaac_sub32_sat(ixheaac_shl32(ixheaac_mult32x16in32(tmp8, SQRT2PLUS1_Q13), 3),
           ixheaac_mult32x16in32_shl(tmp12, SQRT2MINUS1_Q15));
  temp13 = ixheaac_sub32_sat(ixheaac_shl32(ixheaac_mult32x16in32(tmp9, SQRT2PLUS1_Q13), 3),
           ixheaac_mult32x16in32_shl(tmp13, SQRT2MINUS1_Q15));
  temp14 = ixheaac_sub32_sat(ixheaac_mult32x16in32_shl(tmp10, SQRT2MINUS1_Q15),
           ixheaac_shl32(ixheaac_mult32x16in32(tmp14, SQRT2PLUS1_Q13), 3));
  temp15 = ixheaac_sub32_sat(ixheaac_mult32x16in32_shl(tmp11, SQRT2MINUS1_Q15),
           ixheaac_shl32(ixheaac_mult32x16in32(tmp15, SQRT2PLUS1_Q13), 3));

  tmp8 = ixheaac_add32_sat(tmp8, tmp12);
  tmp9 = ixheaac_add32_sat(tmp9, tmp13);
  tmp10 = ixheaac_add32_sat(tmp10, tmp14);
  tmp11 = ixheaac_add32_sat(tmp11, tmp15);
  temp16 = ixheaac_add32_sat(temp10, tmp4);
  temp110 = ixheaac_sub32_sat(temp10, tmp4);
  temp17 = ixheaac_add32_sat(temp11, tmp5);
  temp111 = ixheaac_sub32_sat(temp11, tmp5);

  temp112 = ixheaac_sub32_sat(tmp6, temp19);
  temp114 = ixheaac_add32_sat(tmp6, temp19);
  temp113 = ixheaac_add32_sat(temp18, tmp7);
  temp115 = ixheaac_sub32_sat(temp18, tmp7);

  tmp0 = ixheaac_sub32_sat(temp16, temp114);
  tmp2 = ixheaac_add32_sat(temp16, temp114);
  tmp1 = ixheaac_add32_sat(temp17, temp115);
  tmp3 = ixheaac_sub32_sat(temp17, temp115);
  tmp4 = ixheaac_add32_sat(temp110, temp112);
  tmp6 = ixheaac_sub32_sat(temp110, temp112);
  tmp5 = ixheaac_add32_sat(temp111, temp113);
  tmp7 = ixheaac_sub32_sat(temp111, temp113);

  temp110 = ixheaac_add32_sat(tmp8, tmp10);
  tmp10 = ixheaac_sub32_sat(tmp8, tmp10);
  temp111 = ixheaac_add32_sat(tmp9, tmp11);
  tmp11 = ixheaac_sub32_sat(tmp9, tmp11);

  tmp12 = ixheaac_add32_sat(temp12, temp14);
  tmp14 = ixheaac_sub32_sat(temp12, temp14);
  tmp13 = ixheaac_add32_sat(temp13, temp15);
  tmp15 = ixheaac_sub32_sat(temp13, temp15);

  temp32 = ixheaac_add32_sat(tmp2, temp110);
  temp318 = ixheaac_sub32_sat(tmp2, temp110);
  temp33 = ixheaac_add32_sat(tmp3, temp111);
  temp319 = ixheaac_sub32_sat(tmp3, temp111);
  temp36 = ixheaac_add32_sat(tmp0, tmp12);
  temp322 = ixheaac_sub32_sat(tmp0, tmp12);
  temp37 = ixheaac_add32_sat(tmp1, tmp13);
  temp323 = ixheaac_sub32_sat(tmp1, tmp13);
  temp314 = ixheaac_add32_sat(tmp4, tmp10);
  temp330 = ixheaac_sub32_sat(tmp4, tmp10);
  temp315 = ixheaac_add32_sat(tmp5, tmp11);
  temp331 = ixheaac_sub32_sat(tmp5, tmp11);
  temp310 = ixheaac_add32_sat(tmp6, tmp14);
  temp326 = ixheaac_sub32_sat(tmp6, tmp14);
  temp311 = ixheaac_add32_sat(tmp7, tmp15);
  temp327 = ixheaac_sub32_sat(tmp7, tmp15);

  temp10 = ixheaac_sub32_sat(vec[0], vec[32]);
  temp11 = ixheaac_sub32_sat(vec[1], vec[33]);
  temp12 = ixheaac_sub32_sat(vec[4], vec[36]);
  temp13 = ixheaac_sub32_sat(vec[5], vec[37]);
  temp14 = ixheaac_sub32_sat(vec[8], vec[40]);
  temp15 = ixheaac_sub32_sat(vec[9], vec[41]);
  temp16 = ixheaac_sub32_sat(vec[12], vec[44]);
  temp17 = ixheaac_sub32_sat(vec[13], vec[45]);
  temp18 = ixheaac_sub32_sat(vec[16], vec[48]);
  temp19 = ixheaac_sub32_sat(vec[17], vec[49]);
  temp110 = ixheaac_sub32_sat(vec[20], vec[52]);
  temp111 = ixheaac_sub32_sat(vec[21], vec[53]);
  temp112 = ixheaac_sub32_sat(vec[24], vec[56]);
  temp113 = ixheaac_sub32_sat(vec[25], vec[57]);
  temp114 = ixheaac_sub32_sat(vec[28], vec[60]);
  temp115 = ixheaac_sub32_sat(vec[29], vec[61]);

  tmp9 = ixheaac_mult32x16in32_shl(ixheaac_add32_sat(temp12, temp114), -COS_3PI_BY_8_Q15);
  tmp10 = ixheaac_mult32x16in32_shl(ixheaac_sub32_sat(temp12, temp114), COS_PI_BY_8_Q15);
  tmp8 = ixheaac_mult32x16in32_shl(ixheaac_add32_sat(temp13, temp115), COS_3PI_BY_8_Q15);
  tmp11 = ixheaac_mult32x16in32_shl(ixheaac_sub32_sat(temp13, temp115), COS_PI_BY_8_Q15);
  tmp5 = ixheaac_mult32x16in32_shl(ixheaac_add32_sat(temp14, temp112), -INV_SQRT2_Q15);
  tmp6 = ixheaac_mult32x16in32_shl(ixheaac_sub32_sat(temp14, temp112), INV_SQRT2_Q15);
  tmp4 = ixheaac_mult32x16in32_shl(ixheaac_add32_sat(temp15, temp113), INV_SQRT2_Q15);
  tmp7 = ixheaac_mult32x16in32_shl(ixheaac_sub32_sat(temp15, temp113), INV_SQRT2_Q15);
  tmp13 = ixheaac_mult32x16in32_shl(ixheaac_add32_sat(temp16, temp110), -COS_PI_BY_8_Q15);
  tmp14 = ixheaac_mult32x16in32_shl(ixheaac_sub32_sat(temp16, temp110), COS_3PI_BY_8_Q15);
  tmp12 = ixheaac_mult32x16in32_shl(ixheaac_add32_sat(temp17, temp111), COS_PI_BY_8_Q15);
  tmp15 = ixheaac_mult32x16in32_shl(ixheaac_sub32_sat(temp17, temp111), COS_3PI_BY_8_Q15);

  temp12 = ixheaac_sub32_sat(ixheaac_shl32(ixheaac_mult32x16in32(tmp8, SQRT2PLUS1_Q13), 3),
           ixheaac_mult32x16in32_shl(tmp12, SQRT2MINUS1_Q15));
  temp13 = ixheaac_sub32_sat(ixheaac_shl32(ixheaac_mult32x16in32(tmp9, SQRT2PLUS1_Q13), 3),
           ixheaac_mult32x16in32_shl(tmp13, SQRT2MINUS1_Q15));
  temp14 = ixheaac_sub32_sat(ixheaac_mult32x16in32_shl(tmp10, SQRT2MINUS1_Q15),
           ixheaac_shl32(ixheaac_mult32x16in32(tmp14, SQRT2PLUS1_Q13), 3));
  temp15 = ixheaac_sub32_sat(ixheaac_mult32x16in32_shl(tmp11, SQRT2MINUS1_Q15),
           ixheaac_shl32(ixheaac_mult32x16in32(tmp15, SQRT2PLUS1_Q13), 3));

  tmp8 = ixheaac_add32_sat(tmp8, tmp12);
  tmp9 = ixheaac_add32_sat(tmp9, tmp13);
  tmp10 = ixheaac_add32_sat(tmp10, tmp14);
  tmp11 = ixheaac_add32_sat(tmp11, tmp15);
  temp16 = ixheaac_add32_sat(temp10, tmp4);
  temp110 = ixheaac_sub32_sat(temp10, tmp4);
  temp17 = ixheaac_add32_sat(temp11, tmp5);
  temp111 = ixheaac_sub32_sat(temp11, tmp5);

  temp112 = ixheaac_sub32_sat(tmp6, temp19);
  temp114 = ixheaac_add32_sat(tmp6, temp19);
  temp113 = ixheaac_add32_sat(temp18, tmp7);
  temp115 = ixheaac_sub32_sat(temp18, tmp7);

  tmp0 = ixheaac_sub32_sat(temp16, temp114);
  tmp2 = ixheaac_add32_sat(temp16, temp114);
  tmp1 = ixheaac_add32_sat(temp17, temp115);
  tmp3 = ixheaac_sub32_sat(temp17, temp115);
  tmp4 = ixheaac_add32_sat(temp110, temp112);
  tmp6 = ixheaac_sub32_sat(temp110, temp112);
  tmp5 = ixheaac_add32_sat(temp111, temp113);
  tmp7 = ixheaac_sub32_sat(temp111, temp113);

  temp110 = ixheaac_add32_sat(tmp8, tmp10);
  tmp10 = ixheaac_sub32_sat(tmp8, tmp10);
  temp111 = ixheaac_add32_sat(tmp9, tmp11);
  tmp11 = ixheaac_sub32_sat(tmp9, tmp11);

  tmp12 = ixheaac_add32_sat(temp12, temp14);
  tmp14 = ixheaac_sub32_sat(temp12, temp14);
  tmp13 = ixheaac_add32_sat(temp13, temp15);
  tmp15 = ixheaac_sub32_sat(temp13, temp15);

  temp10 = ixheaac_add32_sat(tmp2, temp110);
  temp18 = ixheaac_sub32_sat(tmp2, temp110);
  temp11 = ixheaac_add32_sat(tmp3, temp111);
  temp19 = ixheaac_sub32_sat(tmp3, temp111);
  temp12 = ixheaac_add32_sat(tmp0, tmp12);
  temp110 = ixheaac_sub32_sat(tmp0, tmp12);
  temp13 = ixheaac_add32_sat(tmp1, tmp13);
  temp111 = ixheaac_sub32_sat(tmp1, tmp13);
  temp16 = ixheaac_add32_sat(tmp4, tmp10);
  temp114 = ixheaac_sub32_sat(tmp4, tmp10);
  temp17 = ixheaac_add32_sat(tmp5, tmp11);
  temp115 = ixheaac_sub32_sat(tmp5, tmp11);
  temp14 = ixheaac_add32_sat(tmp6, tmp14);
  temp112 = ixheaac_sub32_sat(tmp6, tmp14);
  temp15 = ixheaac_add32_sat(tmp7, tmp15);
  temp113 = ixheaac_sub32_sat(tmp7, tmp15);

  *vec++ = temp30;
  *vec++ = temp31;
  *vec++ = ixheaac_add32_sat(temp10, temp40);
  *vec++ = ixheaac_add32_sat(temp11, temp41);
  *vec++ = temp32;
  *vec++ = temp33;
  *vec++ = ixheaac_add32_sat(temp12, temp42);
  *vec++ = ixheaac_add32_sat(temp13, temp43);
  *vec++ = temp34;
  *vec++ = temp35;
  *vec++ = ixheaac_add32_sat(temp14, temp44);
  *vec++ = ixheaac_add32_sat(temp15, temp45);
  *vec++ = temp36;
  *vec++ = temp37;
  *vec++ = ixheaac_add32_sat(temp16, temp46);
  *vec++ = ixheaac_add32_sat(temp17, temp47);
  *vec++ = temp38;
  *vec++ = temp39;
  *vec++ = ixheaac_add32_sat(temp18, temp48);
  *vec++ = ixheaac_add32_sat(temp19, temp49);
  *vec++ = temp310;
  *vec++ = temp311;
  *vec++ = ixheaac_add32_sat(temp110, temp410);
  *vec++ = ixheaac_add32_sat(temp111, temp411);
  *vec++ = temp312;
  *vec++ = temp313;
  *vec++ = ixheaac_add32_sat(temp112, temp412);
  *vec++ = ixheaac_add32_sat(temp113, temp413);
  *vec++ = temp314;
  *vec++ = temp315;
  *vec++ = ixheaac_add32_sat(temp114, temp414);
  *vec++ = ixheaac_add32_sat(temp115, temp415);
  *vec++ = temp316;
  *vec++ = temp317;
  *vec++ = ixheaac_sub32_sat(temp10, temp40);
  *vec++ = ixheaac_sub32_sat(temp11, temp41);
  *vec++ = temp318;
  *vec++ = temp319;
  *vec++ = ixheaac_sub32_sat(temp12, temp42);
  *vec++ = ixheaac_sub32_sat(temp13, temp43);
  *vec++ = temp320;
  *vec++ = temp321;
  *vec++ = ixheaac_sub32_sat(temp14, temp44);
  *vec++ = ixheaac_sub32_sat(temp15, temp45);
  *vec++ = temp322;
  *vec++ = temp323;
  *vec++ = ixheaac_sub32_sat(temp16, temp46);
  *vec++ = ixheaac_sub32_sat(temp17, temp47);
  *vec++ = temp324;
  *vec++ = temp325;
  *vec++ = ixheaac_sub32_sat(temp18, temp48);
  *vec++ = ixheaac_sub32_sat(temp19, temp49);
  *vec++ = temp326;
  *vec++ = temp327;
  *vec++ = ixheaac_sub32_sat(temp110, temp410);
  *vec++ = ixheaac_sub32_sat(temp111, temp411);
  *vec++ = temp328;
  *vec++ = temp329;
  *vec++ = ixheaac_sub32_sat(temp112, temp412);
  *vec++ = ixheaac_sub32_sat(temp113, temp413);
  *vec++ = temp330;
  *vec++ = temp331;
  *vec++ = ixheaac_sub32_sat(temp114, temp414);
  *vec++ = ixheaac_sub32_sat(temp115, temp415);
}

static VOID ixheaacd_cos_mod(WORD32 *subband, ia_mps_dec_qmf_tables_struct *qmf_table_ptr) {
  WORD32 i, m;
  WORD16 wim, wre;
  WORD32 temp_1;
  const WORD16 *ptr1, *ptr2, *ptr3, *ptr4;
  WORD32 re1, im1, re2, im2;

  m = WORD_LENGTH;
  ptr1 = qmf_table_ptr->sbr_sin_twiddle;
  ptr2 = qmf_table_ptr->sbr_cos_twiddle;
  ptr3 = qmf_table_ptr->sbr_sin_twiddle + 31;
  ptr4 = qmf_table_ptr->sbr_cos_twiddle + 31;
  for (i = 0; i < 16; i++) {
    temp_1 = (i << 1);
    re1 = subband[temp_1];
    im2 = subband[temp_1 + 1];
    re2 = subband[62 - temp_1];
    im1 = subband[63 - temp_1];

    wim = *ptr1++;
    wre = *ptr2++;

    subband[temp_1] = ixheaac_add32_sat(
        ixheaac_mult32x16in32_shl(im1, wim), ixheaac_mult32x16in32_shl(re1, wre));
    subband[temp_1 + 1] = ixheaac_sub32_sat(
        ixheaac_mult32x16in32_shl(im1, wre), ixheaac_mult32x16in32_shl(re1, wim));

    wim = *ptr3--;
    wre = *ptr4--;

    subband[62 - temp_1] = ixheaac_add32_sat(
        ixheaac_mult32x16in32_shl(im2, wim), ixheaac_mult32x16in32_shl(re2, wre));
    subband[63 - temp_1] = ixheaac_sub32_sat(
        ixheaac_mult32x16in32_shl(im2, wre), ixheaac_mult32x16in32_shl(re2, wim));
  }

  ixheaacd_fft32(subband, qmf_table_ptr->fft_c);

  ptr1 = qmf_table_ptr->sbr_alt_sin_twiddle;
  ptr2 = qmf_table_ptr->sbr_alt_sin_twiddle + m;
  wim = *ptr1++;
  wre = *ptr2--;

  for (i = 0; i < 16; i++) {
    temp_1 = (i << 1);
    re1 = subband[temp_1];
    im1 = subband[temp_1 + 1];
    re2 = subband[62 - temp_1];
    im2 = subband[63 - temp_1];

    subband[temp_1] = ixheaac_add32_sat(
        ixheaac_mult32x16in32_shl(re1, wre), ixheaac_mult32x16in32_shl(im1, wim));
    subband[63 - temp_1] = ixheaac_sub32_sat(
        ixheaac_mult32x16in32_shl(re1, wim), ixheaac_mult32x16in32_shl(im1, wre));

    wim = *ptr1++;
    wre = *ptr2--;

    subband[62 - temp_1] = ixheaac_add32_sat(
        ixheaac_mult32x16in32_shl(re2, wim), ixheaac_mult32x16in32_shl(im2, wre));
    subband[temp_1 + 1] = ixheaac_sub32_sat(
        ixheaac_mult32x16in32_shl(re2, wre), ixheaac_mult32x16in32_shl(im2, wim));
  }
}

static VOID ixheaacd_sin_mod(WORD32 *subband, ia_mps_dec_qmf_tables_struct *qmf_table_ptr) {
  WORD32 i, m;
  WORD16 wre, wim;
  WORD32 temp_1;
  WORD32 re1, im1, re2, im2;
  const WORD16 *ptr1, *ptr2, *ptr3, *ptr4;

  ptr1 = qmf_table_ptr->sbr_sin_twiddle;
  ptr2 = qmf_table_ptr->sbr_cos_twiddle;
  ptr3 = qmf_table_ptr->sbr_sin_twiddle + 31;
  ptr4 = qmf_table_ptr->sbr_cos_twiddle + 31;

  m = WORD_LENGTH;

  for (i = 0; i < 16; i++) {
    temp_1 = (i << 1);

    re1 = subband[temp_1];
    im2 = subband[temp_1 + 1];
    re2 = subband[62 - temp_1];
    im1 = subband[63 - temp_1];

    wre = *ptr1++;
    wim = *ptr2++;

    subband[temp_1 + 1] = ixheaac_add32_sat(
        ixheaac_mult32x16in32_shl(im1, wim), ixheaac_mult32x16in32_shl(re1, wre));
    subband[temp_1] = ixheaac_sub32_sat(
        ixheaac_mult32x16in32_shl(im1, wre), ixheaac_mult32x16in32_shl(re1, wim));

    wre = *ptr3--;
    wim = *ptr4--;

    subband[63 - temp_1] = ixheaac_add32_sat(
        ixheaac_mult32x16in32_shl(im2, wim), ixheaac_mult32x16in32_shl(re2, wre));
    subband[62 - temp_1] = ixheaac_sub32_sat(
        ixheaac_mult32x16in32_shl(im2, wre), ixheaac_mult32x16in32_shl(re2, wim));
  }

  ixheaacd_fft32(subband, qmf_table_ptr->fft_c);

  ptr1 = qmf_table_ptr->sbr_alt_sin_twiddle;
  ptr2 = qmf_table_ptr->sbr_alt_sin_twiddle + m;

  wim = *ptr1++;
  wre = *ptr2--;

  for (i = 0; i < 16; i++) {
    temp_1 = (i << 1);
    re1 = subband[temp_1];
    im1 = subband[temp_1 + 1];
    re2 = subband[62 - temp_1];
    im2 = subband[63 - temp_1];

    subband[63 - temp_1] = ixheaac_negate32_sat(ixheaac_add32_sat(
        ixheaac_mult32x16in32_shl(re1, wre), ixheaac_mult32x16in32_shl(im1, wim)));
    subband[temp_1] = ixheaac_negate32_sat(ixheaac_sub32_sat(
        ixheaac_mult32x16in32_shl(re1, wim), ixheaac_mult32x16in32_shl(im1, wre)));

    wim = *ptr1++;
    wre = *ptr2--;

    subband[temp_1 + 1] = ixheaac_negate32_sat(ixheaac_add32_sat(
        ixheaac_mult32x16in32_shl(re2, wim), ixheaac_mult32x16in32_shl(im2, wre)));
    subband[62 - temp_1] = ixheaac_negate32_sat(ixheaac_sub32_sat(
        ixheaac_mult32x16in32_shl(re2, wre), ixheaac_mult32x16in32_shl(im2, wim)));
  }
}

static VOID ixheaacd_inverse_modulation(WORD32 *qmf_real, WORD32 *qmf_imag,
                                        ia_mps_dec_qmf_tables_struct *qmf_table_ptr) {
  WORD32 i;

  const WORD16 *ptr1, *ptr2, *ptr3, *ptr4;
  const WORD16 *fft = qmf_table_ptr->fft_c;
  WORD16 wre, wim;
  WORD32 re1, im1, re2, im2;
  WORD32 re12, im12, re22, im22;
  WORD32 temp_1;

  ptr1 = qmf_table_ptr->sbr_sin_twiddle;
  ptr2 = qmf_table_ptr->sbr_cos_twiddle;
  ptr3 = ptr1 + 31;
  ptr4 = ptr2 + 31;

  for (i = 0; i < 16; i++) {
    temp_1 = (i << 1);

    re1 = qmf_real[temp_1];
    im1 = qmf_real[63 - temp_1];

    wim = *ptr1++;
    wre = *ptr2++;

    qmf_real[temp_1] = ixheaac_add32_sat(
        ixheaac_mult32x16in32_shl(im1, wim), ixheaac_mult32x16in32_shl(re1, wre));

    re12 = qmf_imag[temp_1];
    im12 = qmf_imag[63 - temp_1];

    qmf_imag[temp_1] = ixheaac_sub32_sat(
        ixheaac_mult32x16in32_shl(im12, wim), ixheaac_mult32x16in32_shl(re12, wre));

    im2 = qmf_real[temp_1 + 1];

    qmf_real[temp_1 + 1] = ixheaac_sub32_sat(
        ixheaac_mult32x16in32_shl(im1, wre), ixheaac_mult32x16in32_shl(re1, wim));

    im22 = qmf_imag[temp_1 + 1];

    qmf_imag[temp_1 + 1] = ixheaac_add32_sat(
        ixheaac_mult32x16in32_shl(im12, wre), ixheaac_mult32x16in32_shl(re12, wim));

    wim = *ptr3--;
    wre = *ptr4--;

    re2 = qmf_real[62 - temp_1];

    qmf_real[62 - temp_1] = ixheaac_add32_sat(
        ixheaac_mult32x16in32_shl(im2, wim), ixheaac_mult32x16in32_shl(re2, wre));

    re22 = qmf_imag[62 - temp_1];

    qmf_imag[62 - temp_1] = ixheaac_sub32_sat(
        ixheaac_mult32x16in32_shl(im22, wim), ixheaac_mult32x16in32_shl(re22, wre));

    qmf_real[63 - temp_1] = ixheaac_sub32_sat(
        ixheaac_mult32x16in32_shl(im2, wre), ixheaac_mult32x16in32_shl(re2, wim));
    qmf_imag[63 - temp_1] = ixheaac_add32_sat(
        ixheaac_mult32x16in32_shl(im22, wre), ixheaac_mult32x16in32_shl(re22, wim));
  }

  ixheaacd_fft32(qmf_real, fft);
  ixheaacd_fft32(qmf_imag, fft);

  ptr1 = qmf_table_ptr->sbr_alt_sin_twiddle;
  ptr2 = ptr1 + 32;

  wim = *ptr1++;
  wre = *ptr2--;
  for (i = 0; i < 16; i++) {
    temp_1 = (i << 1);
    re1 = qmf_real[temp_1];
    im1 = qmf_real[temp_1 + 1];
    re12 = qmf_imag[temp_1];
    im12 = qmf_imag[temp_1 + 1];

    qmf_real[temp_1] = ixheaac_sub32_sat(ixheaac_mult32x16in32_shl(ixheaac_sub32_sat(
        im12, re1), wre), ixheaac_mult32x16in32_shl(ixheaac_add32_sat(im1, re12), wim));
    qmf_imag[temp_1] = ixheaac_add32_sat(ixheaac_mult32x16in32_shl((ixheaac_add32_sat(
        im12, re1)), wre), ixheaac_mult32x16in32_shl(ixheaac_sub32_sat(im1, re12), wim));

    im2 = qmf_real[63 - temp_1];
    im22 = qmf_imag[63 - temp_1];

    qmf_real[63 - temp_1] = ixheaac_sub32_sat(ixheaac_mult32x16in32_shl(ixheaac_sub32_sat(
        im1, re12), wre), ixheaac_mult32x16in32_shl(ixheaac_add32_sat(im12, re1), wim));
    qmf_imag[63 - temp_1] = ixheaac_sub32_sat(ixheaac_mult32x16in32_shl(ixheaac_sub32_sat(
        re1, im12), wim), ixheaac_mult32x16in32_shl(ixheaac_add32_sat(im1, re12), wre));

    wim = *ptr1++;
    wre = *ptr2--;
    re2 = qmf_real[62 - temp_1];
    re22 = qmf_imag[62 - temp_1];

    qmf_real[temp_1 + 1] = ixheaac_sub32_sat(ixheaac_mult32x16in32_shl(ixheaac_sub32_sat(
        im2, re22), wim), ixheaac_mult32x16in32_shl(ixheaac_add32_sat(im22, re2), wre));
    qmf_imag[temp_1 + 1] = ixheaac_sub32_sat(ixheaac_mult32x16in32_shl(ixheaac_sub32_sat(
        re2, im22), wre), ixheaac_mult32x16in32_shl(ixheaac_add32_sat(im2, re22), wim));

    qmf_real[62 - temp_1] = ixheaac_sub32_sat(ixheaac_mult32x16in32_shl(ixheaac_sub32_sat(
        im22, re2), wim), ixheaac_mult32x16in32_shl(ixheaac_add32_sat(im2, re22), wre));
    qmf_imag[62 - temp_1] = ixheaac_add32_sat(ixheaac_mult32x16in32_shl(ixheaac_add32_sat(
        re2, im22), wim), ixheaac_mult32x16in32_shl(ixheaac_sub32_sat(im2, re22), wre));
  }
}

VOID ixheaacd_calculate_syn_filt_bank_res64(ia_mps_dec_qmf_syn_filter_bank *syn, WORD32 *sr,
                                            WORD32 *si, WORD32 *time_sig, WORD32 channel,
                                            WORD32 resolution, WORD32 nr_samples,
                                            ia_mps_dec_qmf_tables_struct *qmf_table_ptr) {
  WORD32 j, k;
  WORD32 *synth_buf;
  WORD32 *syn_buf_p1, *syn_buf_p2, *syn_buf_p3;
  WORD32 val;
  const WORD32 *p_filter_1, *p_filter_6;
  const WORD32 *p_filter_2, *p_filter_7;
  const WORD32 *p_filter_3, *p_filter_8;
  const WORD32 *p_filter_4, *p_filter_9;
  const WORD32 *p_filter_5, *p_filter_10;

  WORD32 *p_sr, *p_si;

  WORD32 *sbr_qmf_states_synthesis = syn->sbr_qmf_states_synthesis;
  synth_buf = &(sbr_qmf_states_synthesis[channel * QMF_FILTER_STATE_SYN_SIZE_MPS]);

  p_sr = sr;
  p_si = si;
  for (k = 0; k < nr_samples; k++) {
    WORD32 *new_samp = p_si;
    WORD32 *new_samp2;

    ixheaacd_inverse_modulation(p_sr, p_si, qmf_table_ptr);

    p_filter_1 = syn->p_filter_syn;
    p_filter_2 = p_filter_1 + 64;
    p_filter_3 = p_filter_2 + 65;
    p_filter_4 = p_filter_3 + 65;
    p_filter_5 = p_filter_4 + 65;

    syn_buf_p1 = &synth_buf[63];
    val = *(p_sr);

    {
      WORD32 val1 = *(p_si + 63);
      syn_buf_p2 = &synth_buf[63];

      *time_sig++ = ixheaac_add32_sat(syn_buf_p1[512],
                        ixheaacd_mps_mult32_shr_30(*(p_filter_5 + 65), val));
      syn_buf_p1[512] = ixheaac_add32_sat(syn_buf_p2[448],
                            ixheaacd_mps_mult32_shr_30(*(p_filter_5 + 64), val1));
      syn_buf_p2[448] = ixheaac_add32_sat(syn_buf_p1[384],
                            ixheaacd_mps_mult32_shr_30(*p_filter_5++, val));
      syn_buf_p1[384] = ixheaac_add32_sat(syn_buf_p2[320],
                            ixheaacd_mps_mult32_shr_30(*(p_filter_4 + 64), val1));
      syn_buf_p2[320] = ixheaac_add32_sat(syn_buf_p1[256],
                            ixheaacd_mps_mult32_shr_30(*p_filter_4++, val));
      syn_buf_p1[256] = ixheaac_add32_sat(syn_buf_p2[192],
                            ixheaacd_mps_mult32_shr_30(*(p_filter_3 + 64), val1));
      syn_buf_p2[192] = ixheaac_add32_sat(syn_buf_p1[128],
                            ixheaacd_mps_mult32_shr_30(*p_filter_3++, val));
      syn_buf_p1[128] = ixheaac_add32_sat(syn_buf_p2[64],
                            ixheaacd_mps_mult32_shr_30(*(p_filter_2 + 64), val1));
      syn_buf_p2[64] = ixheaac_add32_sat(syn_buf_p1[0],
                            ixheaacd_mps_mult32_shr_30(*p_filter_2++, val));
      syn_buf_p1[0] = ixheaacd_mps_mult32_shr_30(*(p_filter_1 + 63), val1);
    }
    p_filter_6 = p_filter_1 + 62;
    p_filter_7 = p_filter_2 + 62;
    p_filter_8 = p_filter_3 + 62;
    p_filter_9 = p_filter_4 + 62;
    p_filter_10 = p_filter_5 + 62;
    time_sig += 62;

    syn_buf_p2 = synth_buf;
    syn_buf_p3 = syn_buf_p2;
    new_samp2 = p_sr + 63;
    for (j = 0; j < resolution - 1; j++) {
      *time_sig-- = ixheaac_add32_sat(syn_buf_p3[512],
                        ixheaacd_mps_mult32_shr_30(*p_filter_6--, (*new_samp2)));
      syn_buf_p3[512] = ixheaac_add32_sat(syn_buf_p2[448],
                            ixheaacd_mps_mult32_shr_30(*p_filter_5++, (*new_samp)));
      syn_buf_p2[448] = ixheaac_add32_sat(syn_buf_p3[384],
                            ixheaacd_mps_mult32_shr_30(*p_filter_7--, (*new_samp2)));
      syn_buf_p3[384] = ixheaac_add32_sat(syn_buf_p2[320],
                            ixheaacd_mps_mult32_shr_30(*p_filter_4++, (*new_samp)));
      syn_buf_p2[320] = ixheaac_add32_sat(syn_buf_p3[256],
                            ixheaacd_mps_mult32_shr_30(*p_filter_8--, (*new_samp2)));
      syn_buf_p3[256] = ixheaac_add32_sat(syn_buf_p2[192],
                            ixheaacd_mps_mult32_shr_30(*p_filter_3++, (*new_samp)));
      syn_buf_p2[192] = ixheaac_add32_sat(syn_buf_p3[128],
                            ixheaacd_mps_mult32_shr_30(*p_filter_9--, (*new_samp2)));
      syn_buf_p3[128] = ixheaac_add32_sat(syn_buf_p2[64],
                            ixheaacd_mps_mult32_shr_30(*p_filter_2++, (*new_samp)));
      syn_buf_p2[64] = ixheaac_add32_sat(syn_buf_p3[0],
                           ixheaacd_mps_mult32_shr_30(*p_filter_10--, (*new_samp2)));
      syn_buf_p3[0] = ixheaacd_mps_mult32_shr_30(*p_filter_1++, (*new_samp));

      new_samp++;
      syn_buf_p2++;

      new_samp2--;
      syn_buf_p3++;
    }

    time_sig += 64;

    p_sr += MAX_HYBRID_BANDS;
    p_si += MAX_HYBRID_BANDS;
  }
}

VOID ixheaacd_calculate_syn_filt_bank(ia_mps_dec_qmf_syn_filter_bank *syn, WORD32 *sr, WORD32 *si,
                                      WORD32 *time_sig, WORD32 channel, WORD32 resolution,
                                      WORD32 nr_samples,
                                      ia_mps_dec_qmf_tables_struct *qmf_table_ptr) {
  WORD32 j, k;
  WORD32 *synth_buf;
  WORD32 *p_sr, *p_si;
  WORD32 *buf_ptr;
  WORD32 resx2 = resolution << 1;
  WORD32 *sbr_qmf_states_synthesis = syn->sbr_qmf_states_synthesis;

  synth_buf = &(sbr_qmf_states_synthesis[channel * QMF_FILTER_STATE_SYN_SIZE_MPS]);

  p_sr = sr;
  p_si = si;
  for (k = 0; k < nr_samples; k++) {
    WORD32 *new_samp = p_si + 63;

    const WORD32 *p_filter_1 = syn->p_filter_syn;
    const WORD32 *p_filter_2 = p_filter_1 + 65;
    const WORD32 *p_filter_3 = p_filter_2 + 65;
    const WORD32 *p_filter_4 = p_filter_3 + 65;
    const WORD32 *p_filter_5 = p_filter_4 + 65;

    ixheaacd_inverse_modulation(p_sr, p_si, qmf_table_ptr);

    for (j = 0; j < resolution; j++) {
      synth_buf[j] = ixheaacd_mps_mult32_shr_30(*p_filter_1++, (*new_samp));
      synth_buf[resx2 + j] = ixheaac_add32_sat(synth_buf[resx2 + j],
                                 ixheaacd_mps_mult32_shr_30(*p_filter_2++, (*new_samp)));
      synth_buf[resx2 * 2 + j] = ixheaac_add32_sat(synth_buf[resx2 * 2 + j],
                                     ixheaacd_mps_mult32_shr_30(*p_filter_3++, (*new_samp)));
      synth_buf[resx2 * 3 + j] = ixheaac_add32_sat(synth_buf[resx2 * 3 + j],
                                     ixheaacd_mps_mult32_shr_30(*p_filter_4++, (*new_samp)));
      synth_buf[resx2 * 4 + j] = ixheaac_add32_sat(synth_buf[resx2 * 4 + j],
                                     ixheaacd_mps_mult32_shr_30(*p_filter_5++, (*new_samp)));

      new_samp--;
    }

    synth_buf[resx2 - 1] = ixheaac_add32_sat(synth_buf[resx2 - 1],
                               ixheaacd_mps_mult32_shr_30(*p_filter_1++, *p_sr));
    synth_buf[resx2 * 2 - 1] = ixheaac_add32_sat(synth_buf[resx2 * 2 - 1],
                                   ixheaacd_mps_mult32_shr_30(*p_filter_2++, *p_sr));
    synth_buf[3 * resx2 - 1] = ixheaac_add32_sat(synth_buf[3 * resx2 - 1],
                                   ixheaacd_mps_mult32_shr_30(*p_filter_3++, *p_sr));
    synth_buf[4 * resx2 - 1] = ixheaac_add32_sat(synth_buf[4 * resx2 - 1],
                                   ixheaacd_mps_mult32_shr_30(*p_filter_4++, *p_sr));
    *time_sig++ = ixheaac_add32_sat(synth_buf[5 * resx2 - 1],
                      ixheaacd_mps_mult32_shr_30(*p_filter_5++, *p_sr));

    p_filter_1 -= 2;
    p_filter_2 -= 2;
    p_filter_3 -= 2;
    p_filter_4 -= 2;
    p_filter_5 -= 2;

    new_samp = p_sr + resolution - 1;

    for (j = 0; j < resolution - 1; j++) {
      synth_buf[resolution + j] = ixheaac_add32_sat(synth_buf[resolution + j],
                                      ixheaacd_mps_mult32_shr_30(*--p_filter_5, (*new_samp)));
      synth_buf[resolution * (3) + j] =
          ixheaac_add32_sat(synth_buf[resolution * (3) + j],
              ixheaacd_mps_mult32_shr_30(*--p_filter_4, (*new_samp)));
      synth_buf[resolution * (5) + j] =
          ixheaac_add32_sat(synth_buf[resolution * (5) + j],
              ixheaacd_mps_mult32_shr_30(*--p_filter_3, (*new_samp)));
      synth_buf[resolution * (7) + j] =
          ixheaac_add32_sat(synth_buf[resolution * (7) + j],
              ixheaacd_mps_mult32_shr_30(*--p_filter_2, (*new_samp)));
      synth_buf[resolution * (9) + j] =
          ixheaac_add32_sat(synth_buf[resolution * (9) + j],
              ixheaacd_mps_mult32_shr_30(*--p_filter_1, (*new_samp)));
      new_samp--;
    }

    buf_ptr = synth_buf + 9 * resolution + resolution - 2;
    for (j = 0; j < resolution - 1; j++) {
      *time_sig++ = *buf_ptr--;
    }

    memmove((synth_buf + resolution), synth_buf, (9 * resolution) * sizeof(WORD32));

    p_sr += MAX_HYBRID_BANDS;
    p_si += MAX_HYBRID_BANDS;
  }
}

IA_ERRORCODE
ixheaacd_syn_filt_bank_init(ia_mps_dec_synthesis_interface_handle self, WORD32 resolution) {
  switch (resolution) {
    case QMF_BANDS_32:
      self->syn_filter_bank = ixheaacd_calculate_syn_filt_bank;
      break;
    case QMF_BANDS_64:
      self->syn_filter_bank = ixheaacd_calculate_syn_filt_bank_res64;
      break;
    case QMF_BANDS_128:
      self->syn_filter_bank = ixheaacd_calculate_syn_filt_bank;
      break;
    default:
      return IA_XHEAAC_MPS_DEC_INIT_NONFATAL_INVALID_QMF_BAND;
      break;
  }
  return IA_NO_ERROR;
}

static VOID ia_mps_enc_fwd_mod(WORD32 *time_in, WORD32 *r_subband, WORD32 *i_subband,
                               ia_mps_dec_qmf_tables_struct *qmf_table_ptr) {
  WORD32 i;

  for (i = 0; i < 64; i++) {
    r_subband[i] = time_in[i] - time_in[127 - i];
    i_subband[i] = time_in[i] + time_in[127 - i];
  }
  ixheaacd_cos_mod(r_subband, qmf_table_ptr);
  ixheaacd_sin_mod(i_subband, qmf_table_ptr);
}

VOID ixheaacd_calc_ana_filt_bank(ia_heaac_mps_state_struct *pstr_mps_state, WORD16 *time_in,
                                 WORD32 *r_analysis, WORD32 *i_analysis, WORD32 channel) {
  ia_mps_dec_qmf_ana_filter_bank *qmf_bank = &pstr_mps_state->qmf_bank[channel];
  ia_mps_dec_qmf_tables_struct *qmf_table_ptr =
      pstr_mps_state->ia_mps_dec_mps_table.qmf_table_ptr;
  WORD32 i, k, m;

  WORD32 *syn_buffer = pstr_mps_state->mps_scratch_mem_v;
  WORD64 accu1 = 0, accu2 = 0;
  WORD16 flag;
  WORD32 *fp1;
  WORD32 *fp2;
  WORD32 *temp;
  const WORD32 *start_co_eff_ptr_l;
  const WORD32 *start_co_eff_ptr_r;
  const WORD32 *ptr_pf_l, *ptr_pf_r;
  WORD32 *qmf_states_curr_pos;
  WORD32 offset = 0;
  WORD32 n_channels = pstr_mps_state->num_input_channels;
  WORD32 nr_samples = pstr_mps_state->time_slots;
  WORD32 qmf_bands = pstr_mps_state->qmf_bands;
  WORD32 shift = pstr_mps_state->bits_per_sample - 16;
  WORD32 gain = pstr_mps_state->clip_protect_gain;

  WORD32 *p_ana_real = r_analysis;
  WORD32 *p_ana_imag = i_analysis;

  WORD32 *p_ana_re, *p_ana_im;

  flag = qmf_bank->flag;
  if (!flag) {
    fp1 = qmf_bank->qmf_states_buffer;
    fp2 = qmf_bank->qmf_states_buffer + qmf_bands;
  } else {
    fp2 = qmf_bank->qmf_states_buffer;
    fp1 = qmf_bank->qmf_states_buffer + qmf_bands;
  }

  qmf_bank->qmf_states_curr_pos =
      ((WORD32 *)(qmf_bank->qmf_states_buffer) + (qmf_bank->offset * qmf_bands));

  offset = qmf_bank->offset;
  start_co_eff_ptr_l = qmf_bank->ref_co_eff_ptr_l + qmf_bank->offset_l;
  start_co_eff_ptr_r = qmf_bank->ref_co_eff_ptr_r - qmf_bank->offset_r;

  for (i = 0; i < nr_samples; i++) {
    const WORD16 *pcoz = qmf_table_ptr->ia_qmf_anl_addt_cos,
                 *psin = qmf_table_ptr->ia_qmf_anl_addt_sin;
    qmf_states_curr_pos = qmf_bank->qmf_states_curr_pos;

    p_ana_re = p_ana_real;
    p_ana_im = p_ana_imag;

    temp = fp1;
    fp1 = fp2;
    fp2 = temp;

    if (flag) {
      start_co_eff_ptr_l--;

      if (start_co_eff_ptr_l == qmf_bank->ref_co_eff_ptr_l) start_co_eff_ptr_l += 5;
    } else {
      start_co_eff_ptr_r++;
      if (start_co_eff_ptr_r == qmf_bank->ref_co_eff_ptr_r) start_co_eff_ptr_r -= 5;
    }

    flag++;
    if ((flag & ONE_BIT_MASK) == 0) flag = 0;

    if (shift == 0) {
      for (k = 0; k < qmf_bands; k++) {
        qmf_states_curr_pos[k] = ixheaacd_mps_mult32_shr_15(
            (WORD32)time_in[n_channels * (i * qmf_bands + k) + channel], gain);
      }
    } else {
      for (k = 0; k < qmf_bands; k++) {
        WORD32 temp;
        temp = ixheaacd_mps_mult32_shr_15(
            (WORD32)time_in[n_channels * (i * qmf_bands + k) + channel], gain);

        qmf_states_curr_pos[k] = temp >> shift;
      }
    }

    ptr_pf_l = start_co_eff_ptr_l;
    ptr_pf_r = start_co_eff_ptr_r;

    for (k = 0; k < qmf_bands; k++) {
      {
        accu1 = (WORD64)((WORD64)ptr_pf_l[0] * (WORD64)fp1[k]);
        accu1 += (WORD64)((WORD64)ptr_pf_l[1] * (WORD64)fp1[128 + k]);
        accu1 += (WORD64)((WORD64)ptr_pf_l[2] * (WORD64)fp1[256 + k]);
        accu1 += (WORD64)((WORD64)ptr_pf_l[3] * (WORD64)fp1[384 + k]);
        accu1 += (WORD64)((WORD64)ptr_pf_l[4] * (WORD64)fp1[512 + k]);

        accu2 = (WORD64)((WORD64)ptr_pf_r[-1] * (WORD64)fp2[k]);
        accu2 += (WORD64)((WORD64)ptr_pf_r[-2] * (WORD64)fp2[128 + k]);
        accu2 += (WORD64)((WORD64)ptr_pf_r[-3] * (WORD64)fp2[256 + k]);
        accu2 += (WORD64)((WORD64)ptr_pf_r[-4] * (WORD64)fp2[384 + k]);
        accu2 += (WORD64)((WORD64)ptr_pf_r[-5] * (WORD64)fp2[512 + k]);
      }
      syn_buffer[(qmf_bands << 1) - 1 - k] = (WORD32)((WORD64)accu1 >> 21);
      syn_buffer[qmf_bands - 1 - k] = (WORD32)((WORD64)accu2 >> 21);
      ptr_pf_l += 10;
      ptr_pf_r -= 10;
    }

    ia_mps_enc_fwd_mod(syn_buffer, p_ana_re, p_ana_im, qmf_table_ptr);

    for (m = 0; m < (qmf_bands >> 1); m++) {
      WORD32 a_cos, b_cos, a_sin, b_sin;
      WORD32 a_cos1, b_cos1, a_sin1, b_sin1;

      WORD16 coz = *pcoz++, sin = *psin++;

      a_cos = ixheaac_mult32x16in32(p_ana_re[m], coz);
      b_sin = ixheaac_mult32x16in32(p_ana_im[m], sin);
      b_cos = ixheaac_mult32x16in32(p_ana_im[m], coz);
      a_sin = ixheaac_mult32x16in32(p_ana_re[m], sin);

      p_ana_re[m] = ((a_cos + b_sin) << 1);
      p_ana_im[m] = ((b_cos - a_sin) << 1);

      a_cos1 = ixheaac_mult32x16in32(p_ana_re[qmf_bands - 1 - m], coz);
      b_sin1 = ixheaac_mult32x16in32(p_ana_im[qmf_bands - 1 - m], sin);

      a_sin1 = ixheaac_mult32x16in32(p_ana_re[qmf_bands - 1 - m], sin);
      p_ana_re[qmf_bands - 1 - m] = ((-a_cos1 + b_sin1) << 1);
      b_cos1 = ixheaac_mult32x16in32(p_ana_im[qmf_bands - 1 - m], coz);

      p_ana_im[qmf_bands - 1 - m] = ((-b_cos1 - a_sin1) << 1);
    }

    qmf_bank->qmf_states_curr_pos = qmf_bank->qmf_states_curr_pos + qmf_bands;
    offset++;
    if (offset == 10) {
      offset = 0;
      qmf_bank->qmf_states_curr_pos = qmf_bank->qmf_states_buffer;
    }

    p_ana_real += MAX_NUM_QMF_BANDS;
    p_ana_imag += MAX_NUM_QMF_BANDS;
  }

  qmf_bank->offset_l = (WORD32)(start_co_eff_ptr_l - qmf_bank->ref_co_eff_ptr_l);
  qmf_bank->offset_r = (WORD32)(qmf_bank->ref_co_eff_ptr_r - start_co_eff_ptr_r);

  qmf_bank->flag = flag;
  qmf_bank->offset = offset;
}
