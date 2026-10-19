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
#ifndef IXHEAAC_BASIC_OPS_ARR_H
#define IXHEAAC_BASIC_OPS_ARR_H

static PLATFORM_INLINE VOID ixheaac_shr32_arr(WORD32 *word32_arr, WORD16 shift, WORD32 n) {
  WORD32 i;

  for (i = 0; i < n; i++) {
    *word32_arr = ixheaac_shr32(*word32_arr, shift);
    word32_arr++;
  }

  return;
}

static PLATFORM_INLINE VOID ixheaac_shl32_arr_sat(WORD32 *word32_arr, WORD16 shift, WORD32 n) {
  WORD32 i;

  for (i = 0; i < n; i++) {
    *word32_arr = ixheaac_shl32_sat(*word32_arr, shift);
    word32_arr++;
  }

  return;
}

static PLATFORM_INLINE VOID ixheaac_shr16_arr(WORD16 *word16_arr, WORD16 shift, WORD32 n) {
  WORD32 i;

  for (i = 0; i < n; i++) {
    *word16_arr = ixheaac_shr16(*word16_arr, shift);
    word16_arr++;
  }

  return;
}

#endif /* IXHEAAC_BASIC_OPS_ARR_H */
