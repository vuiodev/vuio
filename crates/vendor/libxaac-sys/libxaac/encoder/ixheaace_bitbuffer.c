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

#include <stdlib.h>

#include "ixheaac_type_def.h"
#include "ixheaac_constants.h"
#include "ixheaace_aac_constants.h"
#include "ixheaace_bitbuffer.h"

static VOID ia_enhaacplus_enc_update_bitbuf_word_ptr(ixheaace_bit_buf_handle pstr_bit_buf,
                                                     UWORD8 **p_bitbuf_word, WORD32 cnt) {
  *p_bitbuf_word += cnt;

  if (*p_bitbuf_word > pstr_bit_buf->ptr_bit_buf_end) {
    *p_bitbuf_word -= (pstr_bit_buf->ptr_bit_buf_end - pstr_bit_buf->ptr_bit_buf_base + 1);
  }

  if (*p_bitbuf_word < pstr_bit_buf->ptr_bit_buf_base) {
    *p_bitbuf_word += (pstr_bit_buf->ptr_bit_buf_end - pstr_bit_buf->ptr_bit_buf_base + 1);
  }
}

ixheaace_bit_buf_handle ia_enhaacplus_enc_create_bitbuffer(ixheaace_bit_buf_handle pstr_bit_buf,
                                                           UWORD8 *ptr_bit_buf_base,
                                                           UWORD32 bitbuf_size) {
  pstr_bit_buf->ptr_bit_buf_base = ptr_bit_buf_base;

  pstr_bit_buf->ptr_bit_buf_end = ptr_bit_buf_base + bitbuf_size - 1;

  pstr_bit_buf->ptr_read_next = ptr_bit_buf_base;
  pstr_bit_buf->ptr_write_next = ptr_bit_buf_base;

  pstr_bit_buf->write_position = 7;
  pstr_bit_buf->read_position = 7;

  pstr_bit_buf->cnt_bits = 0;

  pstr_bit_buf->size = bitbuf_size * 8;

  return pstr_bit_buf;
}

VOID ia_enhaacplus_enc_delete_bitbuffer(ixheaace_bit_buf_handle pstr_bit_buf) {
  pstr_bit_buf = NULL;
}

VOID ixheaace_reset_bitbuf(ixheaace_bit_buf_handle pstr_bit_buf, UWORD8 *ptr_bit_buf_base,
                           UWORD32 bitbuf_size) {
  pstr_bit_buf->ptr_bit_buf_base = ptr_bit_buf_base;

  pstr_bit_buf->ptr_bit_buf_end = ptr_bit_buf_base + bitbuf_size - 1;

  pstr_bit_buf->ptr_read_next = ptr_bit_buf_base;
  pstr_bit_buf->ptr_write_next = ptr_bit_buf_base;

  pstr_bit_buf->read_position = 7;
  pstr_bit_buf->write_position = 7;

  pstr_bit_buf->cnt_bits = 0;
}
VOID ixheaace_copy_bitbuf_to_and_fro(ixheaace_bit_buf_handle h_bitbuf_src,
                                     ixheaace_bit_buf_handle h_bitbuf_dst)

{
  WORD32 i;
  WORD32 bytes_to_go_src =
      (WORD32)(h_bitbuf_src->ptr_bit_buf_end - h_bitbuf_src->ptr_bit_buf_base);

  UWORD8 *dst = &h_bitbuf_dst->ptr_bit_buf_base[0];
  UWORD8 *src = &h_bitbuf_src->ptr_bit_buf_base[0];
  WORD32 temp321, temp322;
  UWORD8 temp1, temp2, temp3;
  WORD32 count;
  WORD32 remaining;

  count = bytes_to_go_src >> 1;
  remaining = bytes_to_go_src - (count << 1);

  for (i = count - 1; i >= 0; i--) {
    temp1 = *src;
    temp2 = *dst;
    temp3 = *(src + 1);
    *dst++ = temp1;
    temp1 = *dst;
    *src++ = temp2;
    *dst++ = temp3;
    *src++ = temp1;
  }
  if (remaining)
    for (i = remaining - 1; i >= 0; i--) {
      temp1 = *src;
      temp2 = *dst;
      *dst++ = temp1;
      *src++ = temp2;
    }

  src = h_bitbuf_src->ptr_read_next;
  dst = h_bitbuf_dst->ptr_read_next;

  h_bitbuf_dst->ptr_read_next = src;
  h_bitbuf_src->ptr_read_next = dst;

  src = h_bitbuf_src->ptr_write_next;
  dst = h_bitbuf_dst->ptr_write_next;

  h_bitbuf_dst->ptr_write_next = src;
  h_bitbuf_src->ptr_write_next = dst;

  temp321 = h_bitbuf_dst->read_position;
  temp322 = h_bitbuf_src->read_position;

  h_bitbuf_dst->read_position = temp322;
  h_bitbuf_src->read_position = temp321;

  temp321 = h_bitbuf_dst->write_position;
  temp322 = h_bitbuf_src->write_position;

  h_bitbuf_dst->write_position = temp322;
  h_bitbuf_src->write_position = temp321;

  temp321 = h_bitbuf_dst->cnt_bits;
  temp322 = h_bitbuf_src->cnt_bits;

  h_bitbuf_dst->cnt_bits = temp322;
  h_bitbuf_src->cnt_bits = temp321;
}

VOID ia_enhaacplus_enc_copy_bitbuf(ixheaace_bit_buf_handle h_bitbuf_src,
                                   ixheaace_bit_buf_handle h_bitbuf_dst) {
  WORD32 i;
  WORD32 bytes_to_go_src =
      (WORD32)(h_bitbuf_src->ptr_bit_buf_end - h_bitbuf_src->ptr_bit_buf_base);

  for (i = 0; i < bytes_to_go_src; i++) {
    h_bitbuf_dst->ptr_bit_buf_base[i] = h_bitbuf_src->ptr_bit_buf_base[i];
  }

  h_bitbuf_dst->ptr_read_next = h_bitbuf_src->ptr_read_next;
  h_bitbuf_dst->ptr_write_next = h_bitbuf_src->ptr_write_next;

  h_bitbuf_dst->read_position = h_bitbuf_src->read_position;
  h_bitbuf_dst->write_position = h_bitbuf_src->write_position;

  h_bitbuf_dst->cnt_bits = h_bitbuf_src->cnt_bits;
}

WORD32 ia_enhaacplus_enc_get_bits_available(ixheaace_bit_buf_handle pstr_bit_buf_handle) {
  return pstr_bit_buf_handle->cnt_bits;
}
UWORD32
ixheaace_readbits(ixheaace_bit_buf_handle pstr_bit_buf, UWORD8 no_bits_to_read) {
  UWORD32 return_value;

  if (no_bits_to_read >= 25) {
    return 0;
  }

  pstr_bit_buf->cnt_bits -= no_bits_to_read;
  pstr_bit_buf->read_position -= no_bits_to_read;

  return_value = (UWORD32)*pstr_bit_buf->ptr_read_next;

  while (pstr_bit_buf->read_position < 0) {
    pstr_bit_buf->read_position += 8;
    pstr_bit_buf->ptr_read_next++;

    if (pstr_bit_buf->ptr_read_next > pstr_bit_buf->ptr_bit_buf_end) {
      pstr_bit_buf->ptr_read_next = pstr_bit_buf->ptr_bit_buf_base;
    }

    return_value <<= 8;

    return_value |= (UWORD32)*pstr_bit_buf->ptr_read_next;
  }
  return_value = return_value << (31 - no_bits_to_read - pstr_bit_buf->read_position) >>
                 (32 - no_bits_to_read);

  return (return_value);
}

VOID ia_enhaacplus_enc_wind_bitbuffer_bidirectional(ixheaace_bit_buf_handle pstr_bit_buf,
                                                    WORD32 offset) {
  if (offset != 0) {
    WORD32 buff_offset;
    pstr_bit_buf->read_position -= offset;
    buff_offset = (pstr_bit_buf->read_position) >> 3;
    pstr_bit_buf->read_position -= buff_offset << 3;

    if (buff_offset) {
      ia_enhaacplus_enc_update_bitbuf_word_ptr(pstr_bit_buf, &pstr_bit_buf->ptr_read_next,
                                               -buff_offset);
    }

    pstr_bit_buf->cnt_bits -= offset;
  }
}
