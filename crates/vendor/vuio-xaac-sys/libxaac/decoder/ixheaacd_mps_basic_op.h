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
#ifndef IXHEAACD_MPS_BASIC_OP_H
#define IXHEAACD_MPS_BASIC_OP_H

#define NORM32 (0x40000000)
#define INV_SQRT_2_Q31 (1518500250)
#define Q_SQRT_TAB (15)
#define LOG2XQ17 (5171707904LL)
#define LOG_COEFF1 (27890)
#define LOG_COEFF2 (16262)
#define LOG_COEFF3 (7574)
#define LOG_COEFF4 (1786)

#define TRIG_TABLE_CONV_FAC 326

static PLATFORM_INLINE WORD32 ixheaacd_mps_get_rshift_bits(WORD64 a) {
  WORD32 temp_1, temp_2;
  temp_1 = (WORD32)(a >> 32);
  temp_2 = ixheaac_norm32(temp_1);
  if (temp_2 < 31) {
    return (32 - temp_2);
  } else {
    temp_2 = (WORD32)(a);
    if ((temp_1 ^ temp_2) < 0) {
      return 1;
    } else {
      return 0;
    }
  }
}

static PLATFORM_INLINE WORD32 ixheaacd_mps_narrow(WORD64 a, WORD16 *qfac) {
  WORD32 x;
  x = ixheaacd_mps_get_rshift_bits(a);
  *qfac = 20 - x;
  return (WORD32)((WORD64)a >> x);
}

static PLATFORM_INLINE WORD32 ixheaacd_mps_sqrt(WORD32 num, WORD16 *q, const WORD32 *sqrt_tab) {
  WORD32 index, answer, temp;
  WORD k;

  if (num == 0) return 0;

  k = ixheaac_norm32(num);
  temp = ixheaac_shr32(ixheaac_shl32(num, k), 21);
  *q += k;
  index = temp & 0x1FF;
  answer = sqrt_tab[index];
  if (*q & 1) {
    *q -= 1;
    answer = ixheaac_mult32_shl(answer, INV_SQRT_2_Q31);
  }
  *q = *q >> 1;
  *q += Q_SQRT_TAB;
  return answer;
}

static PLATFORM_INLINE WORD32 ixheaacd_mps_reshape_add32(WORD32 op1, WORD32 op2, WORD16 *qop1,
                                                         WORD16 qop2) {
  WORD64 tempresult;
  if (0 == op2) {
    return op1;
  }
  if (0 == op1) {
    *qop1 = qop2;
    return op2;
  }
  if (*qop1 < qop2) {
    if ((qop2 - *qop1) > 31)
      op2 = 0;
    else
      op2 = op2 >> (qop2 - *qop1);
    tempresult = (WORD64)op1 + (WORD64)op2;
  } else {
    if ((*qop1 - qop2) > 31)
      op1 = 0;
    else
      op1 = op1 >> (*qop1 - qop2);
    *qop1 = qop2;
    tempresult = (WORD64)op1 + (WORD64)op2;
  }
  if (tempresult > (WORD32)0x7fffffff || tempresult < (WORD32)0x80000000) {
    tempresult = tempresult >> 1;
    *qop1 -= 1;
  }
  return (WORD32)tempresult;
}

static PLATFORM_INLINE WORD32 ixheaacd_mps_add32(WORD32 a, WORD32 b, WORD16 *q_a, WORD16 q_b) {
  WORD64 temp_result;

  if (a == 0 || b == 0) {
    if (b == 0) {
      return a;
    } else {
      *q_a = q_b;
      return b;
    }
  }
  if (*q_a > q_b) {
    if (((*q_a) - q_b) > 31) {
      a = 0;
      *q_a = q_b;
    } else {
      a = (a >> ((*q_a) - q_b));
      *q_a = q_b;
    }
  } else {
    if ((q_b - (*q_a)) > 31) {
      b = 0;
    } else {
      b = (b >> (q_b - (*q_a)));
      q_b = *q_a;
    }
  }
  temp_result = (WORD64)a + (WORD64)b;
  if (temp_result > (WORD32)0x7fffffff || temp_result < (WORD32)0x80000000) {
    temp_result = temp_result >> 1;
    *q_a -= 1;
  }

  return (WORD32)temp_result;
}

static PLATFORM_INLINE WORD32 ixheaacd_mps_mult32(WORD32 a, WORD32 b, WORD16 *q_a, WORD16 q_b) {
  WORD64 temp_result;
  WORD32 temp;

  if (a == 0 || b == 0) {
    temp_result = 0;
    *q_a = 15;
    return (WORD32)temp_result;
  }

  *q_a = *q_a + q_b;

  temp_result = (WORD64)a * (WORD64)b;
  temp = ixheaacd_mps_get_rshift_bits(temp_result);
  if (0 != temp) {
    *q_a -= temp;
    temp_result = temp_result >> temp;
  }

  return (WORD32)temp_result;
}

static PLATFORM_INLINE WORD32 ixheaacd_mps_mult32x32(WORD32 a, WORD32 b, WORD16 *q_a,
                                                     WORD16 q_b) {
  WORD64 temp_result;
  if (a == 0 || b == 0) {
    temp_result = 0;
    *q_a = 15;
    return (WORD32)temp_result;
  }
  *q_a = *q_a + q_b;

  temp_result = (WORD64)a * (WORD64)b;
  while (temp_result > (WORD32)0x7fffffff || temp_result < (WORD32)0x80000000) {
    temp_result = temp_result >> 1;
    *q_a -= 1;
  }

  return (WORD32)temp_result;
}

static PLATFORM_INLINE WORD32 ixheaacd_mps_mult32_shr_n(WORD32 a, WORD32 b, WORD16 n) {
  WORD32 result;
  WORD64 temp_result;

  temp_result = (WORD64)a * (WORD64)b;
  result = (WORD32)(temp_result >> n);

  return (result);
}

static PLATFORM_INLINE WORD32 ixheaacd_mps_mult32_shr_30(WORD32 a, WORD32 b) {
  WORD32 result;
  WORD64 temp_result;

  temp_result = (WORD64)a * (WORD64)b;
  result = (WORD32)(temp_result >> 30);

  return (result);
}

static PLATFORM_INLINE WORD32 ixheaacd_mps_mult32_shr_16(WORD32 a, WORD32 b) {
  WORD32 result;
  WORD64 temp_result;

  temp_result = (WORD64)a * (WORD64)b;
  result = (WORD32)(temp_result >> 16);
  return (result);
}

static PLATFORM_INLINE WORD32 ixheaacd_mps_mult32x16_shr_16(WORD32 a, WORD32 b) {
  WORD32 result;
  WORD64 temp_result;
  temp_result = (WORD64)a * (WORD64)b;
  result = (WORD32)(temp_result >> 16);
  return (result);
}

static PLATFORM_INLINE WORD32 ixheaacd_mps_mult32_shr_15(WORD32 a, WORD32 b) {
  WORD32 result;
  WORD64 temp_result;
  temp_result = (WORD64)a * (WORD64)b;
  result = (WORD32)ixheaac_sat64_32(temp_result >> 15);

  return (result);
}

static PLATFORM_INLINE WORD32 ixheaacd_mps_mult32_shr_14(WORD32 a, WORD32 b) {
  WORD32 result;
  WORD64 temp_result;

  temp_result = (WORD64)a * (WORD64)b;
  result = (WORD32)(temp_result >> 16);
  result = result << 2;

  return (result);
}

static PLATFORM_INLINE WORD32 ixheaacd_mps_div_32(WORD32 a, WORD32 b, WORD16 *q_format) {
  WORD32 quotient;
  UWORD32 mantissa_nr, mantissa_dr;
  LOOPINDEX i;
  WORD q_nr, q_dr;

  quotient = 0;

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
    quotient <<= 1;

    if (mantissa_nr >= mantissa_dr) {
      mantissa_nr -= mantissa_dr;
      quotient += 1;
    }
    mantissa_nr <<= 1;
  }

  if ((a ^ b) < 0) {
    return -(quotient);
  }

  return quotient;
}

static WORD32 ixheaacd_mps_convert_to_qn(WORD32 temp, WORD16 qtemp, WORD16 n) {
  WORD64 result;
  if (qtemp == n)
    return temp;
  else if (qtemp > n)
    temp = (WORD32)((WORD64)temp >> (qtemp - n));
  else {
    result = (WORD32)((WORD64)temp << (n - qtemp));
    if (result > (WORD32)0x7fffffff || result < (WORD32)0x80000000) {
      return 0;
    } else
      temp = (WORD32)result;
  }
  return temp;
}

static PLATFORM_INLINE WORD32 ixheaacd_mps_div32_in_q15(WORD32 num, WORD32 den) {
  WORD32 quotient;
  WORD16 q_quotient;

  quotient = ixheaacd_mps_div_32(num, den, &q_quotient);
  quotient = ixheaacd_mps_convert_to_qn(quotient, q_quotient, 15);
  return quotient;
}

static PLATFORM_INLINE WORD32 ixheaacd_mps_log10(WORD32 a, WORD16 q_a) {
  WORD32 x;
  WORD16 q_x;
  WORD32 j, k, temp;
  WORD16 q_num;
  q_num = ixheaac_norm32(a);
  a = a << q_num;
  x = ixheaacd_mps_div_32(a, NORM32, &q_x);

  if (q_x > 16)
    x = x >> (q_x - 16);
  else
    x = x << (16 - q_x);

  q_num = 30 - (q_num + q_a);

  j = x - ONE_IN_Q16;
  k = ixheaacd_mps_mult32_shr_16(SQRT_THREE_Q15, j);
  temp = ixheaacd_mps_mult32_shr_16(j, j);
  k -= ixheaacd_mps_mult32_shr_16(LOG_COEFF1, temp);
  temp = ixheaacd_mps_mult32_shr_16(temp, j);
  k += ixheaacd_mps_mult32_shr_16(LOG_COEFF2, temp);
  temp = ixheaacd_mps_mult32_shr_16(temp, j);
  k -= ixheaacd_mps_mult32_shr_16(LOG_COEFF3, temp);
  temp = ixheaacd_mps_mult32_shr_16(temp, j);
  k += ixheaacd_mps_mult32_shr_16(LOG_COEFF4, temp);

  k += (WORD32)(q_num * ((WORD32)LOG2XQ17));

  return (k >> 1);
}

static PLATFORM_INLINE WORD32 ixheaacd_mps_cos(WORD32 a, const WORD32 *cosine_tab) {
  WORD32 temp_result;

  if (a < 0) {
    a = -a;
  }

  a = a % TWO_PI_IN_Q15;

  temp_result = cosine_tab[((a * TRIG_TABLE_CONV_FAC) >> 15)];
  return temp_result;
}

static PLATFORM_INLINE WORD32 ixheaacd_mps_sin(WORD32 a, const WORD32 *sine_tab) {
  WORD32 temp_result, flag = 0;

  if (a < 0) {
    a = -a;
    flag = 1;
  }

  a = a % TWO_PI_IN_Q15;

  temp_result = sine_tab[((a * TRIG_TABLE_CONV_FAC) >> 15)];
  if (flag) temp_result = -temp_result;

  return temp_result;
}

static PLATFORM_INLINE WORD32 ixheaacd_mps_comp(WORD32 a, WORD32 b, WORD16 *q_a, WORD16 q_b) {
  if (a == 0 || b == 0) {
    if (a == 0) {
      if (b < 0)
        return 1;
      else
        return 0;
    } else if (b == 0) {
      if (a > 0)
        return 1;
      else
        return 0;
    }
  }

  if (*q_a > q_b) {
    a = (a >> ((*q_a) - q_b));
  } else {
    b = (b >> (q_b - (*q_a)));
  }

  if (a > b)
    return 1;
  else
    return 0;
}

#endif /* IXHEAACD_MPS_BASIC_OP_H */
