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
#include "ixheaac_constants.h"
#include "ixheaace_aac_constants.h"
#include <stdlib.h>
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops16.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaac_basic_ops.h"

#include "ixheaace_bitbuffer.h"
#include "ixheaace_common_utils.h"

UWORD8
ixheaace_write_bits(ixheaace_bit_buf_handle pstr_bit_buf, UWORD32 write_value,
                    UWORD8 num_bits_to_write) {
  WORD8 bits_to_write;
  WORD32 write_position;
  UWORD8 *ptr_write_next;
  UWORD8 *ptr_bit_buf_end;
  UWORD8 *ptr_bit_buf_base;

  UWORD8 bits_written = num_bits_to_write;

  if (pstr_bit_buf == NULL) {
    return num_bits_to_write;
  }

  pstr_bit_buf->cnt_bits += num_bits_to_write;

  write_position = pstr_bit_buf->write_position;
  ptr_write_next = pstr_bit_buf->ptr_write_next;
  ptr_bit_buf_end = pstr_bit_buf->ptr_bit_buf_end;
  ptr_bit_buf_base = pstr_bit_buf->ptr_bit_buf_base;

  while (num_bits_to_write) {
    UWORD8 tmp, msk;
    bits_to_write = (WORD8)MIN(write_position + 1, num_bits_to_write);

    tmp = (UWORD8)(write_value << (32 - num_bits_to_write) >>
                   (32 - bits_to_write) << (write_position + 1 - bits_to_write));

    msk = ~(((1 << bits_to_write) - 1) << (write_position + 1 - bits_to_write));

    *ptr_write_next &= msk;
    *ptr_write_next |= tmp;

    write_position -= bits_to_write;

    num_bits_to_write -= bits_to_write;

    if (write_position < 0) {
      write_position += 8;
      ptr_write_next++;

      if (ptr_write_next > ptr_bit_buf_end) {
        ptr_write_next = ptr_bit_buf_base;
      }
    }
  }

  pstr_bit_buf->write_position = write_position;
  pstr_bit_buf->ptr_write_next = ptr_write_next;

  return bits_written;
}

UWORD32 ixheaace_byte_align_buffer(ixheaace_bit_buf_handle pstr_it_bit_buff) {
  WORD alignment;
  alignment = (WORD)((pstr_it_bit_buff->cnt_bits) & 0x07);

  if (alignment) {
    ixheaace_write_bits(pstr_it_bit_buff, 0, (UWORD8)(8 - alignment));
    return (8 - alignment);
  }
  return 0;
}