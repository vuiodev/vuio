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
#include <stdio.h>
#include <stdlib.h>
#include <float.h>
#include <math.h>
#include <assert.h>
#include <string.h>
#include "ixheaac_type_def.h"
#include "ixheaac_constants.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaacd_acelp_com.h"

extern const WORD32 ixheaacd_factorial_7[8];
extern const WORD32 ixheaacd_iso_code_index_table[LEN_ABS_LEADER];
extern const UWORD8 ixheaacd_iso_code_data_table[LEN_SIGN_LEADER];
extern const UWORD32 ixheaacd_signed_leader_is[LEN_SIGN_LEADER];
extern const WORD32 ixheaacd_iso_code_num_table[],
    ixheaacd_pos_abs_leaders_a3[], ixheaacd_pos_abs_leaders_a4[];
extern const UWORD8 ixheaacd_absolute_leader_tab_da[][8];
extern const UWORD32 ixheaacd_cardinality_offset_table_i3[],
    ixheaacd_cardinality_offset_tab_i4[];

static VOID ixheaacd_nearest_neighbor_2d(WORD32 x[], WORD32 y[], WORD32 count,
                                         WORD32 *rem) {
  WORD32 i, j, sum;
  WORD32 s, e[8], em;
  WORD32 rem_temp[8];

  memcpy(rem_temp, rem, 8 * sizeof(WORD32));

  sum = 0;
  for (i = 0; i < 8; i++) {
    if (x[i] < 0) {
      y[i] = ixheaac_negate32_sat(
          ixheaac_shl32_sat((ixheaac_sub32_sat(1, x[i]) >> 1), 1));
    } else {
      y[i] = ixheaac_shl32_sat((ixheaac_add32_sat(1, x[i]) >> 1), 1);
    }
    sum = ixheaac_add32_sat(sum, y[i]);

    if (x[i] % 2 != 0) {
      if (x[i] < 0) {
        rem_temp[i] = ixheaac_negate32_sat(
            ixheaac_sub32_sat(rem_temp[i], (1 << count)));
      } else {
        rem_temp[i] = ixheaac_sub32_sat(rem_temp[i], (1 << count));
      }
    }
  }

  if (sum % 4) {
    em = 0;
    j = 0;
    for (i = 0; i < 8; i++) {
      e[i] = rem_temp[i];
    }
    for (i = 0; i < 8; i++) {
      if (e[i] < 0) {
        s = -e[i];
      } else {
        s = e[i];
      }

      if (em < s) {
        em = s;
        j = i;
      }
    }

    if (e[j] < 0) {
      y[j] -= 2;
      rem_temp[j] = ixheaac_add32_sat(rem_temp[j], (2 << count));
    } else {
      y[j] = ixheaac_add32_sat(y[j], 2);
      rem_temp[j] = ixheaac_sub32_sat(rem_temp[j], (2 << count));
    }
  }

  memcpy(rem, rem_temp, 8 * sizeof(WORD32));
  return;
}

VOID ixheaacd_voronoi_search(WORD32 x[], WORD32 y[], WORD32 count, WORD32 *rem1,
                             WORD32 *rem2) {
  WORD32 i, y0[8], y1[8];
  WORD32 x1[8], tmp;
  WORD32 e0, e1;

  ixheaacd_nearest_neighbor_2d(x, y0, count, rem1);
  for (i = 0; i < 8; i++) {
    if (x[i] == 0) {
      if (rem2[i] == 0) {
        x1[i] = x[i] - 1;
      } else {
        x1[i] = 0;
        rem2[i] = ixheaac_sub32_sat(rem2[i], (1 << count));
      }
    } else {
      x1[i] = ixheaac_sub32_sat(x[i], 1);
    }
  }

  ixheaacd_nearest_neighbor_2d(x1, y1, count, rem2);

  for (i = 0; i < 8; i++) {
    y1[i] = ixheaac_add32_sat(y1[i], 1);
  }

  e0 = e1 = 0;
  for (i = 0; i < 8; i++) {
    tmp = rem1[i];
    e0 = ixheaac_add32_sat(ixheaac_sat64_32((WORD64)tmp * (WORD64)tmp), e0);
    tmp = rem2[i];
    e1 = ixheaac_add32_sat(ixheaac_sat64_32((WORD64)tmp * (WORD64)tmp), e1);
  }

  if (e0 < e1) {
    for (i = 0; i < 8; i++) {
      y[i] = y0[i];
    }
  } else {
    for (i = 0; i < 8; i++) {
      y[i] = y1[i];
    }
  }
  return;
}

VOID ixheaacd_voronoi_idx_dec(WORD32 *kv, WORD32 m, WORD32 *y, WORD32 count) {
  WORD32 i, v[8], tmp, sum, *ptr1, *ptr2;
  WORD32 z[8];
  WORD32 rem1[8], rem2[8];

  for (i = 0; i < 8; i++) y[i] = kv[7];

  z[7] = y[7] >> count;
  rem1[7] = y[7] & (m - 1);
  sum = 0;
  for (i = 6; i >= 1; i--) {
    tmp = ixheaac_shl32_sat(kv[i], 1);
    sum = ixheaac_add32_sat(sum, tmp);
    y[i] = ixheaac_add32_sat(y[i], tmp);
    z[i] = y[i] >> count;
    rem1[i] = y[i] & (m - 1);
  }
  y[0] = ixheaac_add32_sat(
      y[0],
      ixheaac_add32_sat(ixheaac_sat64_32((WORD64)4 * (WORD64)kv[0]), sum));
  z[0] = (ixheaac_sub32_sat(y[0], 2)) >> count;
  if (m != 0)
    rem1[0] = (ixheaac_sub32_sat(y[0], 2)) % m;
  else
    rem1[0] = ixheaac_sub32_sat(y[0], 2);

  memcpy(rem2, rem1, 8 * sizeof(WORD32));

  ixheaacd_voronoi_search(z, v, count, rem1, rem2);

  ptr1 = y;
  ptr2 = v;
  for (i = 0; i < 8; i++) {
    *ptr1 = ixheaac_sub32_sat(*ptr1,
                               ixheaac_sat64_32((WORD64)m * (WORD64)*ptr2++));
    ptr1++;
  }
}

static VOID ixheaacd_gosset_rank_of_permutation(WORD32 rank, WORD32 *xs) {
  WORD32 i, j, a[8], w[8], base, fac, fac_b, target;

  j = 0;
  w[j] = 1;
  a[j] = xs[0];
  base = 1;
  for (i = 1; i < 8; i++) {
    if (xs[i] != xs[i - 1]) {
      j++;
      w[j] = 1;
      a[j] = xs[i];
    } else {
      w[j]++;
      base *= w[j];
    }
  }

  if (w[0] == 8) {
    for (i = 0; i < 8; i++) xs[i] = a[0];
  } else {
    target = rank * base;
    fac_b = 1;

    for (i = 0; i < 8; i++) {
      fac = fac_b * ixheaacd_factorial_7[i];
      j = -1;
      do {
        target -= w[++j] * fac;
      } while (target >= 0);
      xs[i] = a[j];

      target += w[j] * fac;
      fac_b *= w[j];
      w[j]--;
    }
  }

  return;
}

static WORD32 ixheaacd_get_abs_leader_tbl(const UWORD32 *table,
                                          UWORD32 code_book_ind, WORD32 size) {
  WORD32 i;

  for (i = 4; i < size; i += 4) {
    if (code_book_ind < table[i]) break;
  }
  if (i > size) i = size;

  if (code_book_ind < table[i - 2]) i -= 2;
  if (code_book_ind < table[i - 1]) i--;
  i--;

  return (i);
}

static VOID ixheaacd_gosset_decode_base_index(WORD32 n, UWORD32 code_book_ind,
                                              WORD32 *ya) {
  WORD32 i, im, t, sign_code, idx = 0, ks, rank;

  if (n < 2) {
    for (i = 0; i < 8; i++) ya[i] = 0;
  } else {
    switch (n) {
      case 2:
      case 3:
        i = ixheaacd_get_abs_leader_tbl(ixheaacd_cardinality_offset_table_i3,
                                        code_book_ind, LEN_I3);
        idx = ixheaacd_pos_abs_leaders_a3[i];
        break;
      case 4:
        i = ixheaacd_get_abs_leader_tbl(ixheaacd_cardinality_offset_tab_i4,
                                        code_book_ind, LEN_I4);
        idx = ixheaacd_pos_abs_leaders_a4[i];
        break;
    }

    for (i = 0; i < 8; i++) ya[i] = ixheaacd_absolute_leader_tab_da[idx][i];

    t = ixheaacd_iso_code_index_table[idx];
    im = ixheaacd_iso_code_num_table[idx];
    ks = ixheaacd_get_abs_leader_tbl(ixheaacd_signed_leader_is + t,
                                     code_book_ind, im);

    sign_code = 2 * ixheaacd_iso_code_data_table[t + ks];
    for (i = 7; i >= 0; i--) {
      ya[i] *= (1 - (sign_code & 2));
      sign_code >>= 1;
    }

    rank = code_book_ind - ixheaacd_signed_leader_is[t + ks];

    ixheaacd_gosset_rank_of_permutation(rank, ya);
  }
  return;
}

VOID ixheaacd_rotated_gosset_mtx_dec(WORD32 qn, WORD32 code_book_idx,
                                     WORD32 *kv, WORD32 *b) {
  if (qn <= 4) {
    ixheaacd_gosset_decode_base_index(qn, code_book_idx, b);
  } else {
    WORD32 i, m, c[8];
    WORD32 count = 0;
    while (qn > 4) {
      count++;
      qn -= 2;
    }

    if (count >= 31)
      m = MAX_32;
    else
      m = 1 << count;

    ixheaacd_gosset_decode_base_index(qn, code_book_idx, b);

    ixheaacd_voronoi_idx_dec(kv, m, c, count);

    for (i = 0; i < 8; i++) {
      b[i] =
          ixheaac_add32_sat(ixheaac_sat64_32((WORD64)m * (WORD64)b[i]), c[i]);
    }
  }
  return;
}