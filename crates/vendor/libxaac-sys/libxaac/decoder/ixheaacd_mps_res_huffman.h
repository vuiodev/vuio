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
#ifndef IXHEAACD_MPS_RES_HUFFMAN_H
#define IXHEAACD_MPS_RES_HUFFMAN_H

#define _SWAP(a, b) \
  (b = (((WORD32)a[0] << 24) | ((WORD32)a[1] << 16) | ((WORD32)a[2] << 8) | ((WORD32)a[3])))

extern UWORD32 ixheaacd_res_aac_showbits_32(UWORD8 *p_read_next);

static PLATFORM_INLINE WORD32 ixheaacd_res_exts(UWORD32 a, WORD32 shift_left,
                                                WORD32 shift_right) {
  WORD32 x;
  x = (UWORD32)a << shift_left;
  x = (WORD32)x >> shift_right;

  return x;
}

static PLATFORM_INLINE UWORD32 ixheaacd_aac_read_2bytes(UWORD8 **p_read_next, WORD32 *r_bit_pos,
                                                        WORD32 *readword) {
  UWORD8 *v = *p_read_next;
  WORD32 bits_consumed = *r_bit_pos;

  if ((bits_consumed - 16) >= 0) {
    *readword = (*readword << 8) | *v;
    v++;
    *readword = (*readword << 8) | *v;
    v++;
    bits_consumed -= 16;
  } else if ((bits_consumed - 8) >= 0) {
    *readword = (*readword << 8) | *v;
    v++;
    bits_consumed -= 8;
  }

  *r_bit_pos = bits_consumed;
  *p_read_next = v;
  return 1;
}

#endif /* IXHEAACD_MPS_RES_HUFFMAN_H */
