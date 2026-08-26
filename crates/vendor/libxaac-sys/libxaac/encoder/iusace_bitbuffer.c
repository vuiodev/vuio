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
#include "iusace_cnst.h"
#include "iusace_bitbuffer.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaac_basic_ops.h"

ia_bit_buf_struct *iusace_create_bit_buffer(ia_bit_buf_struct *it_bit_buf,
                                            UWORD8 *ptr_bit_buf_base, UWORD32 bit_buffer_size,
                                            WORD32 init) {
  it_bit_buf->ptr_bit_buf_base = ptr_bit_buf_base;
  it_bit_buf->ptr_bit_buf_end = ptr_bit_buf_base + bit_buffer_size - 1;
  it_bit_buf->ptr_read_next = ptr_bit_buf_base;
  it_bit_buf->ptr_write_next = ptr_bit_buf_base;

  if (init) {
    it_bit_buf->write_position = 7;
    it_bit_buf->read_position = 7;
    it_bit_buf->cnt_bits = 0;
    it_bit_buf->size = bit_buffer_size * 8;
  }

  return (it_bit_buf);
}

VOID iusace_reset_bit_buffer(ia_bit_buf_struct *it_bit_buf) {
  it_bit_buf->ptr_read_next = it_bit_buf->ptr_bit_buf_base;
  it_bit_buf->ptr_write_next = it_bit_buf->ptr_bit_buf_base;

  it_bit_buf->write_position = 7;
  it_bit_buf->read_position = 7;
  it_bit_buf->cnt_bits = 0;

  return;
}

UWORD8 iusace_write_bits_buf(ia_bit_buf_struct *it_bit_buf, UWORD32 write_val, UWORD8 num_bits) {
  WORD8 bits_to_write;
  WORD32 write_position;
  UWORD8 *ptr_write_next;
  UWORD8 *ptr_bit_buf_end;
  UWORD8 *ptr_bit_buf_base;
  UWORD8 bits_written = num_bits;
  if (it_bit_buf) {
    it_bit_buf->cnt_bits += num_bits;

    write_position = it_bit_buf->write_position;
    ptr_write_next = it_bit_buf->ptr_write_next;
    ptr_bit_buf_end = it_bit_buf->ptr_bit_buf_end;
    ptr_bit_buf_base = it_bit_buf->ptr_bit_buf_base;
    while (num_bits) {
      UWORD8 tmp, msk;

      bits_to_write = (WORD8)MIN(write_position + 1, num_bits);

      tmp = (UWORD8)(write_val << (32 - num_bits) >> (32 - bits_to_write)
                                                         << (write_position + 1 - bits_to_write));

      msk = ~(((1 << bits_to_write) - 1) << (write_position + 1 - bits_to_write));

      *ptr_write_next &= msk;
      *ptr_write_next |= tmp;

      write_position -= bits_to_write;

      num_bits -= bits_to_write;

      if (write_position < 0) {
        write_position += 8;
        ptr_write_next++;

        if (ptr_write_next > ptr_bit_buf_end) {
          ptr_write_next = ptr_bit_buf_base;
        }
      }
    }

    it_bit_buf->write_position = write_position;
    it_bit_buf->ptr_write_next = ptr_write_next;
  }

  return (bits_written);
}

WORD32 iusace_write_escape_value(ia_bit_buf_struct *pstr_it_bit_buff, UWORD32 value,
                                 UWORD8 no_bits1, UWORD8 no_bits2, UWORD8 no_bits3) {
  WORD32 bit_cnt = 0;
  UWORD32 esc_val = 0;
  UWORD32 max_val1 = (1 << no_bits1) - 1;
  UWORD32 max_val2 = (1 << no_bits2) - 1;
  UWORD32 max_val3 = (1 << no_bits3) - 1;

  esc_val = MIN(value, max_val1);
  bit_cnt += iusace_write_bits_buf(pstr_it_bit_buff, esc_val, no_bits1);

  if (esc_val == max_val1) {
    value = value - esc_val;

    esc_val = MIN(value, max_val2);
    bit_cnt += iusace_write_bits_buf(pstr_it_bit_buff, esc_val, no_bits2);

    if (esc_val == max_val2) {
      value = value - esc_val;

      esc_val = MIN(value, max_val3);
      bit_cnt += iusace_write_bits_buf(pstr_it_bit_buff, esc_val, no_bits3);
    }
  }

  return bit_cnt;
}
