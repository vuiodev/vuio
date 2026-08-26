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
#ifndef IXHEAAC_BASIC_OPS40_H
#define IXHEAAC_BASIC_OPS40_H

static PLATFORM_INLINE WORD32 ixheaac_mult32x16in32_shl(WORD32 a, WORD16 b) {
  WORD32 result;
  WORD64 temp_result;

  temp_result = (WORD64)a * (WORD64)b;

  result = (WORD32)(temp_result >> 16);

  return (result << 1);
}

static PLATFORM_INLINE WORD32 ixheaac_mult32x16in32(WORD32 a, WORD16 b) {
  WORD32 result;
  WORD64 temp_result;

  temp_result = (WORD64)a * (WORD64)b;

  result = (WORD32)(temp_result >> 16);

  return (result);
}

static PLATFORM_INLINE WORD32 ixheaac_mult32x32in32(WORD32 a, WORD32 b) {
  WORD32 result;
  WORD64 temp_result;

  temp_result = (WORD64)a * (WORD64)b;

  result = (WORD32)(temp_result >> 16);

  return (result);
}

static PLATFORM_INLINE WORD32 ixheaac_mult32x16in32_shl_sat(WORD32 a, WORD16 b) {
  WORD32 result;

  if (a == (WORD32)0x80000000 && b == (WORD16)0x8000) {
    result = (WORD32)0x7fffffff;
  } else {
    result = ixheaac_mult32x16in32_shl(a, b);
  }

  return (result);
}

static PLATFORM_INLINE WORD32 ixheaac_mult32_shl(WORD32 a, WORD32 b) {
  WORD32 result;
  WORD64 temp_result;

  temp_result = (WORD64)a * (WORD64)b;
  result = (WORD32)(temp_result >> 32);

  return (result << 1);
}

static PLATFORM_INLINE WORD32 ixheaac_mult32(WORD32 a, WORD32 b) {
  WORD32 result;
  WORD64 temp_result;

  temp_result = (WORD64)a * (WORD64)b;
  result = (WORD32)(temp_result >> 32);

  return (result);
}

static PLATFORM_INLINE WORD32 ixheaac_mult32_shl_sat(WORD32 a, WORD32 b) {
  WORD32 result;

  if (a == (WORD32)0x80000000 && b == (WORD32)0x80000000) {
    result = 0x7fffffff;
  } else {
    result = ixheaac_mult32_shl(a, b);
  }

  return (result);
}

static PLATFORM_INLINE WORD32 ixheaac_mac32x16in32(WORD32 a, WORD32 b, WORD16 c) {
  WORD32 result;

  result = a + ixheaac_mult32x16in32(b, c);

  return (result);
}

static PLATFORM_INLINE WORD32 ixheaac_mac32x16in32_shl(WORD32 a, WORD32 b, WORD16 c) {
  WORD32 result;

  result = a + ixheaac_mult32x16in32_shl(b, c);

  return (result);
}

static PLATFORM_INLINE WORD32 ixheaac_mac32x16in32_shl_sat(WORD32 a, WORD32 b, WORD16 c) {
  WORD32 result;

  result = ixheaac_add32_sat(a, ixheaac_mult32x16in32_shl_sat(b, c));

  return (result);
}

static PLATFORM_INLINE WORD32 ixheaac_mac32(WORD32 a, WORD32 b, WORD32 c) {
  WORD32 result;

  result = a + ixheaac_mult32(b, c);

  return (result);
}

static PLATFORM_INLINE WORD64 ixheaac_mult32x32in64(WORD32 a, WORD32 b) {
  WORD64 result;

  result = (WORD64)a * (WORD64)b;

  return (result);
}

static PLATFORM_INLINE WORD64 ixheaac_mac32x32in64(WORD64 sum, WORD32 a, WORD32 b) {
  sum += (WORD64)a * (WORD64)b;

  return (sum);
}

static PLATFORM_INLINE WORD64 ixheaac_mac32x32in64_7(const WORD32 *a, const WORD16 *b) {
  WORD64 sum;
  sum = (WORD64)a[0] * (WORD64)b[0];
  sum += (WORD64)a[1] * (WORD64)b[1];
  sum += (WORD64)a[2] * (WORD64)b[2];
  sum += (WORD64)a[3] * (WORD64)b[3];
  sum += (WORD64)a[4] * (WORD64)b[4];
  sum += (WORD64)a[5] * (WORD64)b[5];
  sum += (WORD64)a[6] * (WORD64)b[6];

  return (sum);
}

static PLATFORM_INLINE WORD64 ixheaac_mac32x32in64_n(WORD64 sum, const WORD32 *a, const WORD16 *b,
                                                     WORD32 n) {
  WORD32 k;

  sum += (WORD64)a[0] * (WORD64)b[0];
  for (k = 1; k < n; k++) sum += (WORD64)a[k] * (WORD64)b[k];
  return (sum);
}

static PLATFORM_INLINE WORD64 ixheaac_mult64(WORD32 a, WORD32 b) {
  WORD64 result;
  result = (WORD64)a * (WORD64)b;
  return (result);
}

static PLATFORM_INLINE WORD64 ixheaac_mult64_sat(WORD64 a, WORD64 b) {
  WORD64 result;

  if (a > 0 && b > 0 && a > MAX_64 / b) return MAX_64;
  if (a < 0 && b > 0 && a < MIN_64 / b) return MIN_64;
  if (a > 0 && b < 0 && b < MIN_64 / a) return MIN_64;
  if (a < 0 && b < 0 && a < MAX_64 / b) return MAX_64;

  result = a * b;
  return (result);
}

static PLATFORM_INLINE WORD64 ixheaac_add64_sat(WORD64 a, WORD64 b) {
  WORD64 result, comp;
  result = (a < 0) ? MIN_64 : MAX_64;
  comp = result - a;
  if ((a < 0) == (b > comp)) result = a + b;

  return (result);
}

static PLATFORM_INLINE WORD32 ixheaac_sat64_32(WORD64 a) {
  WORD32 result;
  if (a >= MAX_32) {
    result = MAX_32;
  } else if (a <= MIN_32) {
    result = MIN_32;
  } else {
    result = (WORD32)a;
  }
  return (result);
}

static PLATFORM_INLINE WORD64 ixheaac_add64(WORD64 a, WORD64 b) {
  WORD64 result;
  result = a + b;
  return (result);
}

static PLATFORM_INLINE WORD64 ixheaac_sub64(WORD64 a, WORD64 b) {
  WORD64 diff;

  diff = (WORD64)a - (WORD64)b;

  return diff;
}

static PLATFORM_INLINE WORD64 ixheaac_sub64_sat(WORD64 a, WORD64 b) {
  WORD64 diff;

  diff = ixheaac_sub64(a, b);

  if ((((WORD64)a ^ (WORD64)b) & (WORD64)MIN_64) != 0) {
    if (((WORD64)diff ^ (WORD64)a) & (WORD64)MIN_64) {
      diff = (a < 0L) ? MIN_64 : MAX_64;
    }
  }

  return (diff);
}

static PLATFORM_INLINE WORD32 ixheaac_mul32_sh(WORD32 a, WORD32 b, WORD8 shift) {
  WORD32 result;
  WORD64 temp_result;

  temp_result = (WORD64)a * (WORD64)b;
  result = (WORD32)(temp_result >> shift);

  return (result);
}

static PLATFORM_INLINE WORD32 ixheaac_mult32x16hin32_shl(WORD32 a, WORD32 b) {
  WORD32 product;
  WORD64 temp_product;

  temp_product = (WORD64)a * (WORD64)(b >> 16);
  product = (WORD32)(temp_product >> 16);

  return (product << 1);
}

#endif /* IXHEAAC_BASIC_OPS40_H */
