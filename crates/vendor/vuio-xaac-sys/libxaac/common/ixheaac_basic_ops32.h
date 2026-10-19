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
#ifndef IXHEAAC_BASIC_OPS32_H
#define IXHEAAC_BASIC_OPS32_H

static PLATFORM_INLINE WORD32 ixheaac_min32(WORD32 a, WORD32 b) {
  WORD32 min_val;

  min_val = (a < b) ? a : b;

  return min_val;
}

static PLATFORM_INLINE WORD32 ixheaac_max32(WORD32 a, WORD32 b) {
  WORD32 max_val;

  max_val = (a > b) ? a : b;

  return max_val;
}

static PLATFORM_INLINE WORD32 ixheaac_shl32(WORD32 a, WORD b) {
  WORD32 out_val;

  b = ((UWORD32)(b << 24) >> 24);
  if (b > 31)
    out_val = 0;
  else
    out_val = (WORD32)a << b;

  return out_val;
}

static PLATFORM_INLINE WORD32 ixheaac_shr32(WORD32 a, WORD b) {
  WORD32 out_val;

  b = ((UWORD32)(b << 24) >> 24);
  if (b >= 31) {
    if (a < 0)
      out_val = -1;
    else
      out_val = 0;
  } else {
    out_val = (WORD32)a >> b;
  }

  return out_val;
}

static PLATFORM_INLINE WORD32 ixheaac_shl32_sat(WORD32 a, WORD b) {
  WORD32 out_val;
  if (a > (MAX_32 >> b))
    out_val = MAX_32;
  else if (a < (MIN_32 >> b))
    out_val = MIN_32;
  else
    out_val = a << b;
  return (out_val);
}

static PLATFORM_INLINE WORD32 ixheaac_shl32_dir(WORD32 a, WORD b) {
  WORD32 out_val;

  if (b < 0) {
    out_val = ixheaac_shr32(a, -b);
  } else {
    out_val = ixheaac_shl32(a, b);
  }

  return out_val;
}

static PLATFORM_INLINE WORD32 ixheaac_shl32_dir_sat(WORD32 a, WORD b) {
  WORD32 out_val;

  if (b < 0) {
    out_val = ixheaac_shr32(a, -b);
  } else {
    out_val = ixheaac_shl32_sat(a, b);
  }

  return out_val;
}

static PLATFORM_INLINE WORD32 ixheaac_shr32_dir(WORD32 a, WORD b) {
  WORD32 out_val;

  if (b < 0) {
    out_val = ixheaac_shl32(a, -b);
  } else {
    out_val = ixheaac_shr32(a, b);
  }

  return out_val;
}

static PLATFORM_INLINE WORD32 ixheaac_shr32_dir_sat(WORD32 a, WORD b) {
  WORD32 out_val;

  if (b < 0) {
    out_val = ixheaac_shl32_sat(a, -b);
  } else {
    out_val = ixheaac_shr32(a, b);
  }

  return out_val;
}

static PLATFORM_INLINE WORD32 ixheaac_mult16x16in32(WORD16 a, WORD16 b) {
  WORD32 product;

  product = (WORD32)a * (WORD32)b;

  return product;
}

static PLATFORM_INLINE WORD32 ixheaac_mult32x16hin32(WORD32 a, WORD32 b) {
  WORD32 result;
  WORD64 temp_result;

  temp_result = (WORD64)(a) * (WORD64)(b >> 16);
  result = (WORD32)(temp_result >> 16);

  return (result);
}

static PLATFORM_INLINE WORD32 ixheaac_mult32x16in32_sat(WORD32 a, WORD16 b) {
  WORD32 result;
  WORD64 temp_result;

  temp_result = (WORD64)a * (WORD64)b;

  if (temp_result < (WORD64)MIN_32)
    result = MIN_32;

  else if (temp_result > (WORD64)MAX_32)
    result = MAX_32;

  else
    result = (WORD32)(temp_result);

  return (result);
}

static PLATFORM_INLINE WORD32 ixheaac_mult16x16in32_shl(WORD16 a, WORD16 b) {
  WORD32 product;

  product = ixheaac_shl32(ixheaac_mult16x16in32(a, b), 1);

  return product;
}

static PLATFORM_INLINE WORD32 ixheaac_mult16x16in32_shl_sat(WORD16 a, WORD16 b) {
  WORD32 product;
  product = (WORD32)a * (WORD32)b;
  if (product != (WORD32)0x40000000L) {
    product = ixheaac_shl32(product, 1);
  } else {
    product = MAX_32;
  }
  return product;
}

static PLATFORM_INLINE WORD32 ixheaac_add32(WORD32 a, WORD32 b) {
  WORD32 sum;

  sum = (WORD32)a + (WORD32)b;

  return sum;
}

static PLATFORM_INLINE WORD32 ixheaac_sub32(WORD32 a, WORD32 b) {
  WORD32 diff;

  diff = (WORD32)a - (WORD32)b;

  return diff;
}

static PLATFORM_INLINE WORD32 ixheaac_add32_sat(WORD32 a, WORD32 b) {
  WORD64 sum;

  sum = (WORD64)a + (WORD64)b;

  if (sum >= MAX_32) return MAX_32;
  if (sum <= MIN_32) return MIN_32;

  return (WORD32)sum;
}

static PLATFORM_INLINE WORD32 ixheaac_add32_sat3(WORD32 a, WORD32 b, WORD32 c) {
  WORD64 sum;

  sum = (WORD64)a + (WORD64)b;

  sum = (WORD64)sum + (WORD64)c;

  if (sum > MAX_32) {
    sum = MAX_32;
  }
  if (sum < MIN_32) {
    sum = MIN_32;
  }

  return (WORD32)sum;
}

static PLATFORM_INLINE WORD32 ixheaac_sub32_sat(WORD32 a, WORD32 b) {
  WORD64 diff;

  diff = (WORD64)a - (WORD64)b;

  if (diff >= MAX_32) return MAX_32;
  if (diff <= MIN_32) return MIN_32;

  return (WORD32)diff;
}

static PLATFORM_INLINE WORD ixheaac_norm32(WORD32 a) {
  WORD norm_val;

  if (a == 0) {
    norm_val = 31;
  } else {
    if (a == (WORD32)0xffffffffL) {
      norm_val = 31;
    } else {
      if (a < 0) {
        a = ~a;
      }
      for (norm_val = 0; a < (WORD32)0x40000000L; norm_val++) {
        a <<= 1;
      }
    }
  }

  return norm_val;
}

static PLATFORM_INLINE WORD ixheaac_pnorm32(WORD32 a) {
  WORD norm_val;

  if (a == 0) {
    norm_val = 31;
  } else {
    for (norm_val = 0; a < (WORD32)0x40000000L; norm_val++) {
      a <<= 1;
    }
  }

  return norm_val;
}

static PLATFORM_INLINE WORD32 ixheaac_abs32(WORD32 a) {
  WORD32 abs_val;

  abs_val = a;

  if (a < 0) {
    abs_val = -a;
  }

  return abs_val;
}

static PLATFORM_INLINE WORD32 ixheaac_abs32_nrm(WORD32 a) {
  WORD32 abs_val;

  abs_val = a;

  if (a < 0) {
    abs_val = ~a;
  }

  return abs_val;
}

static PLATFORM_INLINE WORD32 ixheaac_abs32_sat(WORD32 a) {
  WORD32 abs_val;

  abs_val = a;

  if (a == MIN_32) {
    abs_val = MAX_32;
  } else if (a < 0) {
    abs_val = -a;
  }

  return abs_val;
}

static PLATFORM_INLINE WORD32 ixheaac_negate32(WORD32 a) {
  WORD32 neg_val;

  neg_val = -a;

  return neg_val;
}

static PLATFORM_INLINE WORD32 ixheaac_negate32_sat(WORD32 a) {
  WORD32 neg_val;

  if (a == MIN_32) {
    neg_val = MAX_32;
  } else {
    neg_val = -a;
  }
  return neg_val;
}

static PLATFORM_INLINE WORD32 ixheaac_div32(WORD32 a, WORD32 b, WORD *q_format) {
  WORD32 quotient;
  UWORD32 mantissa_nr, mantissa_dr;
  WORD16 sign = 0;

  LOOPINDEX i;
  WORD q_nr, q_dr;

  if ((a < 0) && (0 != b)) {
    a = -a;
    sign = (WORD16)(sign ^ -1);
  }

  if (b < 0) {
    b = -b;
    sign = (WORD16)(sign ^ -1);
  }

  if (0 == b) {
    *q_format = 0;
    return (a);
  }

  quotient = 0;

  q_nr = ixheaac_norm32(a);
  mantissa_nr = (UWORD32)a << (q_nr);
  q_dr = ixheaac_norm32(b);
  mantissa_dr = (UWORD32)b << (q_dr);
  *q_format = (WORD)(30 + q_nr - q_dr);

  for (i = 0; i < 31; i++) {
    quotient = quotient << 1;

    if (mantissa_nr >= mantissa_dr) {
      mantissa_nr = mantissa_nr - mantissa_dr;
      quotient += 1;
    }

    mantissa_nr = (UWORD32)mantissa_nr << 1;
  }

  if (sign < 0) {
    quotient = -quotient;
  }

  return quotient;
}

static PLATFORM_INLINE WORD32 ixheaac_shr32_sat(WORD32 a, WORD32 b) {
  WORD32 out_val;

  b = ((UWORD32)(b << 24) >> 24);
  if (b >= 31) {
    if (a < 0)
      out_val = -1;
    else
      out_val = 0;
  } else if (b <= 0) {
    return a;
  } else {
    a = ixheaac_add32_sat(a, (1 << (b - 1)));
    out_val = (WORD32)a >> b;
  }

  return out_val;
}

static PLATFORM_INLINE WORD32 ixheaac_mac16x16in32_sat(WORD32 a, WORD16 b, WORD16 c) {
  WORD32 acc;

  acc = ixheaac_mult16x16in32(b, c);

  acc = ixheaac_add32_sat(a, acc);

  return acc;
}

static PLATFORM_INLINE WORD32 ixheaac_mac16x16in32_shl(WORD32 a, WORD16 b, WORD16 c) {
  WORD32 acc;

  acc = ixheaac_mult16x16in32_shl(b, c);

  acc = ixheaac_add32(a, acc);

  return acc;
}

static PLATFORM_INLINE WORD32 ixheaac_mac16x16in32_shl_sat(WORD32 a, WORD16 b, WORD16 c) {
  WORD32 acc;

  acc = ixheaac_mult16x16in32_shl_sat(b, c);

  acc = ixheaac_add32_sat(a, acc);

  return acc;
}

static PLATFORM_INLINE WORD32 ixheaac_msu16x16in32(WORD32 a, WORD16 b, WORD16 c) {
  WORD32 acc;

  acc = ixheaac_mult16x16in32(b, c);

  acc = ixheaac_sub32(a, acc);

  return acc;
}

#endif /* IXHEAAC_BASIC_OPS32_H */
