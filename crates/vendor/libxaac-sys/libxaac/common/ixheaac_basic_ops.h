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
#ifndef IXHEAAC_BASIC_OPS_H
#define IXHEAAC_BASIC_OPS_H

#define MIN(a, b) ((a) < (b) ? (a) : (b))
#define MAX(a, b) ((a) > (b) ? (a) : (b))

static PLATFORM_INLINE WORD16 ixheaac_extract16h(WORD32 var) {
  WORD16 var_out;

  var_out = (WORD16)(var >> 16);
  return (var_out);
}

static PLATFORM_INLINE WORD16 ixheaac_extract16l(WORD32 var) {
  WORD16 var_out;

  var_out = (WORD16)var;
  return (var_out);
}

static PLATFORM_INLINE WORD32 ixheaac_deposit16h_in32(WORD16 var) {
  WORD32 var_out;

  var_out = (WORD32)var << 16;
  return (var_out);
}

static PLATFORM_INLINE WORD32 ixheaac_deposit16l_in32(WORD16 var) {
  WORD32 var_out;

  var_out = (WORD32)var;
  return (var_out);
}

static PLATFORM_INLINE UWORD32 ixheaac_extu(UWORD32 a, WORD32 shift_left, WORD32 shift_right) {
  UWORD32 x;
  x = (UWORD32)a << shift_left;
  x = (UWORD32)x >> shift_right;

  return x;
}

static PLATFORM_INLINE WORD32 ixheaac_mult32x16h_in32_shl_sat(WORD32 a, WORD32 b) {
  WORD32 result;

  if (a == (WORD32)0x80000000 && b == (WORD16)0x8000) {
    result = (WORD32)0x7fffffff;
  } else {
    result = ixheaac_mult32x16in32_shl(a, ixheaac_extract16h(b));
  }

  return (result);
}

static PLATFORM_INLINE WORD32 ixheaac_div32_pos_normb(WORD32 a, WORD32 b) {
  WORD32 quotient;
  UWORD32 mantissa_nr = a;
  UWORD32 mantissa_dr = b;

  LOOPINDEX i;

  if (a == b) {
    quotient = MAX_32;
  } else {
    quotient = 0;

    for (i = 0; i < 32; i++) {
      quotient = quotient << 1;

      if (mantissa_nr >= mantissa_dr) {
        mantissa_nr = mantissa_nr - mantissa_dr;
        quotient += 1;
      }

      mantissa_nr = (UWORD32)mantissa_nr << 1;
    }
  }

  return quotient;
}

static PLATFORM_INLINE WORD32 ixheaac_shr32_dir_sat_limit(WORD32 a, WORD b) {
  WORD32 out_val;

  if (b < 0) {
    out_val = ixheaac_shl32_sat(a, -b);
  } else {
    b = ixheaac_min32(b, 31);
    out_val = ixheaac_shr32(a, b);
  }

  return out_val;
}

static PLATFORM_INLINE WORD32 ixheaac_shl32_dir_sat_limit(WORD32 a, WORD b) {
  WORD32 out_val;

  if (b < 0) {
    b = -b;
    b = ixheaac_min32(b, 31);
    out_val = ixheaac_shr32(a, b);
  } else {
    out_val = ixheaac_shl32_sat(a, b);
  }

  return out_val;
}

static PLATFORM_INLINE WORD64 ixheaac_mac32x32in64_dual(WORD32 a, WORD32 b, WORD64 c) {
  WORD64 result;
  WORD64 temp_result;

  temp_result = (WORD64)a * (WORD64)b;
  result = ixheaac_add64_sat(c, temp_result);
  return (result);
}

#endif /* IXHEAAC_BASIC_OPS_H */
