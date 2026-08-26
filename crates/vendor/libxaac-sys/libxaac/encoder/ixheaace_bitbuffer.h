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

#pragma once

#undef ARIB_COMPLIANCE
#undef ADTS_CRC_ENABLE

typedef struct {
  UWORD8 *ptr_bit_buf_base; /* pointer points to first position in bitstream buffer */
  UWORD8 *ptr_bit_buf_end;  /* pointer points to last position in bitstream buffer */
  UWORD8 *ptr_read_next;  /* pointer points to next available word in bitstream buffer to read */
  UWORD8 *ptr_write_next; /* pointer points to next available word in bitstream buffer to write */
  WORD32 read_position;   /* 7 <= read_position <= 0 */
  WORD32 write_position;  /* 7 <= write_position <= 0 */
  WORD32 cnt_bits;        /* number of available bits in the bitstream buffer
                         write bits to bitstream buffer  => increment cnt_bits
                         read bits from bitstream buffer => decrement cnt_bits */
  WORD32 size;            /* size of bitbuffer in bits */
} ixheaace_bit_buf;

/* Define pointer to bit buffer structure */
typedef ixheaace_bit_buf *ixheaace_bit_buf_handle;

ixheaace_bit_buf_handle ia_enhaacplus_enc_create_bitbuffer(ixheaace_bit_buf_handle pstr_bit_buf,
                                                           UWORD8 *ptr_bit_buf_base,
                                                           UWORD32 bitbuf_size);

VOID ia_enhaacplus_enc_delete_bitbuffer(ixheaace_bit_buf_handle pstr_bit_buf);

UWORD32 ixheaace_readbits(ixheaace_bit_buf_handle pstr_bit_buf, UWORD8 no_bits_to_read);

UWORD8 ixheaace_write_bits(ixheaace_bit_buf_handle pstr_bit_buf, UWORD32 write_value,
                           UWORD8 num_bits_to_write);

VOID ixheaace_reset_bitbuf(ixheaace_bit_buf_handle pstr_bit_buf, UWORD8 *ptr_bit_buf_base,
                           UWORD32 bitbuf_size);

VOID ixheaace_copy_bitbuf_to_and_fro(ixheaace_bit_buf_handle h_bitbuf_src,
                                     ixheaace_bit_buf_handle h_bitbuf_dst);

VOID ia_enhaacplus_enc_copy_bitbuf(ixheaace_bit_buf_handle h_bitbuf_src,
                                   ixheaace_bit_buf_handle h_bitbuf_dst);

VOID ia_enhaacplus_enc_wind_bitbuffer_bidirectional(ixheaace_bit_buf_handle pstr_bit_buf,
                                                    WORD32 offset);

WORD32 ia_enhaacplus_enc_get_bits_available(ixheaace_bit_buf_handle pstr_bit_buf_handle);

UWORD32 ixheaace_byte_align_buffer(ixheaace_bit_buf_handle pstr_it_bit_buff);
