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
#include "ixheaacd_sbr_common.h"
#include "ixheaac_type_def.h"

#include "ixheaac_constants.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops16.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaac_basic_op.h"
#include "ixheaac_basic_ops.h"
#include "ixheaacd_intrinsics.h"
#include "ixheaacd_common_rom.h"
#include "ixheaacd_basic_funcs.h"

#define sat16_m(a) ixheaac_sat16(a)

VOID ixheaacd_fix_mant_exp_add(WORD16 op1_mant, WORD16 op1_exp, WORD16 op2_mant,
                               WORD16 op2_exp, WORD16 *ptr_result_mant,
                               WORD16 *ptr_result_exp) {
  WORD32 new_mant;
  WORD32 new_exp;
  new_exp = op1_exp - op2_exp;
  if (new_exp < 0) {
    if (new_exp < -31) {
      new_exp = -31;
    }
    op1_mant = op1_mant >> (-new_exp);
    new_exp = op2_exp;
  } else {
    if (new_exp > 31) {
      new_exp = 31;
    }
    op2_mant = op2_mant >> new_exp;
    new_exp = op1_exp;
  }

  new_mant = op1_mant + op2_mant;

  if (ixheaac_abs32(new_mant) >= 0x8000) {
    new_mant = new_mant >> 1;
    new_exp++;
  }

  *ptr_result_mant = new_mant;
  *ptr_result_exp = (WORD16)(new_exp);
}

WORD32 ixheaacd_fix_mant_div(WORD16 op1_mant, WORD16 op2_mant,
                             WORD16 *ptr_result_mant,
                             ixheaacd_misc_tables *pstr_common_tables)

{
  WORD32 pre_shift_val, post_shift_val;
  WORD32 index;
  WORD16 one_by_op2_mant;

  pre_shift_val = ixheaac_norm32(op2_mant) - 16;

  index = (op2_mant << pre_shift_val) >> (SHORT_BITS - 3 - 8);

  index &= (1 << (8 + 1)) - 1;

  if (index == 0) {
    post_shift_val = ixheaac_norm32(op1_mant) - 16;

    *ptr_result_mant = (op1_mant << post_shift_val);
  } else {
    WORD32 ratio_m;

    index = ((index - 1) >> 1);

    one_by_op2_mant = pstr_common_tables->inv_table[index];

    ratio_m = ixheaac_mult16x16in32(one_by_op2_mant, op1_mant);

    post_shift_val = ixheaac_norm32(ratio_m) - 1;

    *ptr_result_mant = (WORD16)((ratio_m << post_shift_val) >> 15);
  }
  return (pre_shift_val - post_shift_val);
}

VOID ixheaacd_fix_mant_exp_sqrt(WORD16 *ptr_in_out, WORD16 *sqrt_table) {
  WORD32 index;
  WORD32 pre_shift_val;
  WORD32 op_mant = *ptr_in_out;
  WORD32 op_exp = *(ptr_in_out + 1);
  WORD32 result_m;
  WORD32 result_e;

  if (op_mant > 0) {
    pre_shift_val = (ixheaac_norm32((WORD16)op_mant) - 16);
    op_exp = (op_exp - pre_shift_val);
    index = (op_mant << pre_shift_val) >> (SHORT_BITS - 3 - 8);
    index &= (1 << (8 + 1)) - 1;
    result_m = sqrt_table[index >> 1];
    if ((op_exp & 1) != 0) {
      result_m = (result_m * 0x5a82) >> 16;
      op_exp += 3;
    }
    result_e = (op_exp >> 1);

  } else {
    result_m = 0;
    result_e = -SHORT_BITS;
  }

  *ptr_in_out++ = (WORD16)result_m;
  *ptr_in_out = (WORD16)result_e;
}

WORD32 ixheaacd_fix_div_dec(WORD32 op1, WORD32 op2) {
  WORD32 quotient = 0;
  UWORD32 abs_num, abs_den;
  WORD32 k;
  WORD32 sign;

  abs_num = ixheaac_abs32(op1 >> 1);
  abs_den = ixheaac_abs32(op2 >> 1);
  sign = op1 ^ op2;

  if (abs_num != 0) {
    for (k = 15; k > 0; k--) {
      quotient = (quotient << 1);
      abs_num = (abs_num << 1);
      if (abs_num >= abs_den) {
        abs_num -= abs_den;
        quotient++;
      }
    }
  }
  if (sign < 0) quotient = -(quotient);

  return quotient;
}

#define ONE_IN_Q30 0x40000000

static PLATFORM_INLINE WORD32 ixheaacd_one_by_sqrt_calc(WORD32 op) {
  WORD32 a = ixheaac_add32_sat(0x900ebee0,
                                ixheaac_mult32x16in32_shl_sat(op, 0x39d9));
  WORD32 iy =
      ixheaac_add32_sat(0x573b645a, ixheaac_mult32x16h_in32_shl_sat(op, a));

  iy = ixheaac_shl32_dir_sat_limit(iy, 1);

  a = ixheaac_mult32_shl_sat(op, iy);
  a = ixheaac_sub32_sat(ONE_IN_Q30, ixheaac_shl32_dir_sat_limit(
                                         ixheaac_mult32_shl_sat(a, iy), 1));
  iy = ixheaac_add32_sat(iy, ixheaac_mult32_shl_sat(a, iy));

  a = ixheaac_mult32_shl_sat(op, iy);
  a = ixheaac_sub32_sat(ONE_IN_Q30, ixheaac_shl32_dir_sat_limit(
                                         ixheaac_mult32_shl_sat(a, iy), 1));
  iy = ixheaac_add32_sat(iy, ixheaac_mult32_shl_sat(a, iy));

  a = ixheaac_mult32_shl_sat(op, iy);
  a = ixheaac_sub32_sat(ONE_IN_Q30, ixheaac_shl32_dir_sat_limit(
                                         ixheaac_mult32_shl_sat(a, iy), 1));
  iy = ixheaac_add32_sat(iy, ixheaac_mult32_shl_sat(a, iy));

  return iy;
}

WORD32 ixheaacd_sqrt(WORD32 op) {
  WORD32 result = 0;
  WORD16 shift;

  if (op != 0) {
    shift = (WORD16)(ixheaac_norm32(op) & ~1);
    op = ixheaac_shl32_dir_sat_limit(op, shift);
    shift = ixheaac_shr32_dir_sat_limit(shift, 1);
    op = ixheaac_mult32_shl_sat(ixheaacd_one_by_sqrt_calc(op), op);
    result = ixheaac_shr32_dir_sat_limit(op, ixheaac_sat16(shift - 1));
  }

  return result;
}
