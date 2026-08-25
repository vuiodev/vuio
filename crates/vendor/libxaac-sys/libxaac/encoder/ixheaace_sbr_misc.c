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

#include "ixheaac_type_def.h"
#include "ixheaace_sbr_misc.h"

VOID ixheaace_shellsort_int(WORD32 *ptr_in, WORD32 n) {
  WORD32 i, j, v;
  WORD32 inc = 1;

  do {
    inc = 3 * inc + 1;
  } while (inc <= n);

  do {
    inc = inc / 3;

    for (i = inc + 1; i <= n; i++) {
      v = ptr_in[i - 1];
      j = i;

      while (ptr_in[j - inc - 1] > v) {
        ptr_in[j - 1] = ptr_in[j - inc - 1];
        j -= inc;

        if (j <= inc) {
          break;
        }
      }

      ptr_in[j - 1] = v;
    }
  } while (inc > 1);
}

VOID ixheaace_add_vec_left(WORD32 *ptr_dst, WORD32 *ptr_length_dst, WORD32 *ptr_src,
                           WORD32 length_src) {
  WORD32 i;

  for (i = length_src - 1; i >= 0; i--) {
    ixheaace_add_left(ptr_dst, ptr_length_dst, ptr_src[i]);
  }
}

VOID ixheaace_add_left(WORD32 *ptr_vector, WORD32 *ptr_length_vector, WORD32 value) {
  WORD32 i;

  for (i = *ptr_length_vector; i > 0; i--) {
    ptr_vector[i] = ptr_vector[i - 1];
  }

  ptr_vector[0] = value;

  (*ptr_length_vector)++;
}

VOID ixheaace_add_right(WORD32 *ptr_vector, WORD32 *ptr_length_vector, WORD32 value) {
  ptr_vector[*ptr_length_vector] = value;

  (*ptr_length_vector)++;
}
