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
#include <stdlib.h>
#include <stdio.h>

#include "ixheaac_type_def.h"
#include "ixheaacd_interface.h"

#include "ixheaacd_bitbuffer.h"
#include "ixheaacd_info.h"
#include "ixheaacd_bitbuffer.h"

VOID ixheaacd_hufftab(ia_huff_code_book_struct *ptr_huff_code_book,
                      const ia_huff_code_word_struct *ptr_huff_code_word,
                      const WORD16 *code_book_tbl, const WORD32 *index,
                      WORD32 dim, WORD32 lav, WORD32 lav_incr_esc,
                      WORD32 sign_code_book, UWORD8 max_code_word_len) {
  WORD32 i, num;

  if (!sign_code_book) {
    ptr_huff_code_book->huff_mode = lav + 1;
    ptr_huff_code_book->off = 0;
  } else {
    ptr_huff_code_book->huff_mode = 2 * lav + 1;
    ptr_huff_code_book->off = lav;
  }
  num = 1;
  for (i = 0; i < dim; i++) num *= ptr_huff_code_book->huff_mode;

  ptr_huff_code_book->num = num;
  ptr_huff_code_book->dim = dim;
  ptr_huff_code_book->lav = lav;
  ptr_huff_code_book->lav_incr_esc = lav_incr_esc;
  ptr_huff_code_book->sign_code_book = sign_code_book;
  ptr_huff_code_book->pstr_huff_code_word = ptr_huff_code_word;
  ptr_huff_code_book->code_book_tbl = code_book_tbl;
  ptr_huff_code_book->idx_tbl = index;
  ptr_huff_code_book->max_code_word_len = max_code_word_len;
}

WORD32 ixheaacd_huff_codeword(
    const ia_huff_code_word_struct *ptr_huff_code_word, UWORD16 data_present,
    ia_bit_buf_struct *it_bit_buff)

{
  WORD32 i, j;
  UWORD32 code_word = 0;
  UWORD32 tmp = 0;

  i = ptr_huff_code_word->len;
  if (data_present == 0) {
    code_word = ixheaacd_read_bits_buf(it_bit_buff, i);
  }

  if (data_present == 1) {
    code_word = ixheaacd_read_bits_buf(it_bit_buff, i);
  }
  while (code_word != ptr_huff_code_word->code_word) {
    ptr_huff_code_word++;
    j = ptr_huff_code_word->len - i;
    if (j < 0) {
      return ptr_huff_code_word->index;
    }

    i += j;
    code_word <<= j;

    if (data_present == 0) {
      tmp = ixheaacd_read_bits_buf(it_bit_buff, j);
    }

    if (data_present == 1) {
      tmp = ixheaacd_read_bits_buf(it_bit_buff, j);
    }

    code_word |= tmp;
  }
  return (ptr_huff_code_word->index);
}
