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
#ifndef IXHEAAC_BASIC_OPS16_H
#define IXHEAAC_BASIC_OPS16_H

static PLATFORM_INLINE WORD16 ixheaac_sat16(WORD32 op1) {
  WORD16 var_out;

  if (op1 > 0X00007fffL) {
    var_out = MAX_16;
  } else if (op1 < (WORD32)0xffff8000L) {
    var_out = (WORD16)(-32768);
  } else {
    var_out = (WORD16)(op1);
  }
  return (var_out);
}

static PLATFORM_INLINE WORD16 ixheaac_add16(WORD16 op1, WORD16 op2) {
  WORD16 var_out;

  var_out = ((WORD16)(op1 + op2));
  return (var_out);
}

static PLATFORM_INLINE WORD16 ixheaac_add16_sat(WORD16 op1, WORD16 op2) {
  WORD16 var_out;
  WORD32 sum;

  sum = (WORD32)op1 + (WORD32)op2;
  var_out = ixheaac_sat16(sum);
  return (var_out);
}

static PLATFORM_INLINE WORD16 ixheaac_sub16(WORD16 op1, WORD16 op2) {
  WORD16 var_out;

  var_out = ((WORD16)(op1 - op2));
  return (var_out);
}

static PLATFORM_INLINE WORD16 ixheaac_sub16_sat(WORD16 op1, WORD16 op2) {
  WORD16 var_out;
  WORD32 diff;

  diff = (WORD32)op1 - op2;
  var_out = ixheaac_sat16(diff);
  return (var_out);
}

static PLATFORM_INLINE WORD16 ixheaac_mult16(WORD16 op1, WORD16 op2) {
  WORD16 var_out;

  var_out = ((WORD16)(((WORD32)op1 * (WORD32)op2) >> 16));
  return (var_out);
}

static PLATFORM_INLINE WORD16 ixheaac_mult16_shl(WORD16 op1, WORD16 op2) {
  WORD16 var_out;

  var_out = ((WORD16)(((WORD32)op1 * (WORD32)op2) >> 15));
  return (var_out);
}

static PLATFORM_INLINE WORD16 ixheaac_mult16_shl_sat(WORD16 op1, WORD16 op2) {
  WORD16 var_out;
  WORD32 temp;

  temp = ((WORD32)(((WORD32)op1 * (WORD32)op2) >> 15));
  var_out = ixheaac_sat16(temp);
  return (var_out);
}

static PLATFORM_INLINE WORD16 ixheaac_shl16(WORD16 op1, WORD16 shift) {
  WORD16 var_out;

  var_out = (WORD16)(op1 << shift);
  return (var_out);
}

static PLATFORM_INLINE WORD16 ixheaac_shl16_sat(WORD16 op1, WORD16 shift) {
  WORD16 var_out;
  WORD32 temp;

  if (shift > 15) {
    shift = 15;
  }
  temp = (WORD32)(op1 << shift);
  var_out = ixheaac_sat16(temp);
  return (var_out);
}

static PLATFORM_INLINE WORD16 ixheaac_shr16(WORD16 op1, WORD16 shift) {
  WORD16 var_out;

  var_out = ((WORD16)(op1 >> shift));
  return (var_out);
}

static PLATFORM_INLINE WORD16 ixheaac_shl16_dir(WORD16 op1, WORD16 shift) {
  WORD16 var_out;
  if (shift > 0) {
    var_out = ixheaac_shl16(op1, shift);
  } else {
    var_out = ixheaac_shr16(op1, (WORD16)(-shift));
  }
  return (var_out);
}

static PLATFORM_INLINE WORD16 ixheaac_shr16_dir(WORD16 op1, WORD16 shift) {
  WORD16 var_out;

  if (shift < 0) {
    var_out = ixheaac_shl16(op1, (WORD16)(-shift));
  } else {
    var_out = ixheaac_shr16(op1, shift);
  }
  return (var_out);
}

static PLATFORM_INLINE WORD16 ixheaac_shl16_dir_sat(WORD16 op1, WORD16 shift) {
  WORD16 var_out;
  if (shift > 0) {
    var_out = ixheaac_shl16_sat(op1, shift);
  } else {
    var_out = ixheaac_shr16(op1, (WORD16)(-shift));
  }
  return (var_out);
}

static PLATFORM_INLINE WORD16 ixheaac_shr16_dir_sat(WORD16 op1, WORD16 shift) {
  WORD16 var_out;

  if (shift < 0) {
    var_out = ixheaac_shl16_sat(op1, (WORD16)(-shift));
  } else {
    var_out = ixheaac_shr16(op1, shift);
  }
  return (var_out);
}

static PLATFORM_INLINE WORD16 ixheaac_norm16(WORD16 op1) {
  WORD16 var_out;

  if (0 == op1) {
    var_out = 0;
  } else {
    if ((WORD16)0xffff == op1) {
      var_out = 15;
    } else {
      if (op1 < 0) {
        op1 = (WORD16)(~op1);
      }
      for (var_out = 0; op1 < 0x4000; var_out++) {
        op1 <<= 1;
      }
    }
  }

  return (var_out);
}

static PLATFORM_INLINE WORD16 ixheaac_abs16(WORD16 op1) {
  WORD16 var_out;

  if (op1 < 0) {
    var_out = (WORD16)(-op1);
  } else {
    var_out = op1;
  }
  return (var_out);
}

static PLATFORM_INLINE WORD16 ixheaac_abs16_sat(WORD16 op1) {
  WORD16 var_out;

  if (-32768 == op1) {
    var_out = MAX_16;
  } else {
    if (op1 < 0) {
      var_out = (WORD16)(-op1);
    } else {
      var_out = op1;
    }
  }
  return (var_out);
}

static PLATFORM_INLINE WORD16 ixheaac_negate16(WORD16 op1) {
  WORD16 var_out;

  if (-32768 == op1) {
    var_out = MAX_16;
  } else {
    var_out = (WORD16)(-op1);
  }
  return (var_out);
}

static PLATFORM_INLINE WORD16 ixheaac_min16(WORD16 op1, WORD16 op2) {
  WORD16 var_out;

  var_out = op1 < op2 ? op1 : op2;
  return (var_out);
}

static PLATFORM_INLINE WORD16 ixheaac_max16(WORD16 op1, WORD16 op2) {
  WORD16 var_out;

  var_out = op1 > op2 ? op1 : op2;
  return (var_out);
}

static PLATFORM_INLINE WORD16 ixheaac_round16(WORD32 op1) {
  WORD16 var_out;

  var_out = (WORD16)(ixheaac_add32_sat(op1, 0x8000) >> 16);
  return (var_out);
}
#endif /* IXHEAAC_BASIC_OPS16_H */
