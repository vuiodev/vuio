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
typedef struct ia_bit_buf_struct {
  UWORD8 *ptr_bit_buf_base;
  UWORD8 *ptr_bit_buf_end;
  UWORD8 *ptr_read_next;
  UWORD8 *ptr_write_next;
  WORD32 read_position;
  WORD32 write_position;
  WORD32 cnt_bits;
  WORD32 size;
} ia_bit_buf_struct;

ia_bit_buf_struct *iusace_create_bit_buffer(ia_bit_buf_struct *it_bit_buf,
                                            UWORD8 *ptr_bit_buf_base, UWORD32 bit_buffer_size,
                                            WORD32 init);

VOID iusace_reset_bit_buffer(ia_bit_buf_struct *it_bit_buf);

UWORD8 iusace_write_bits_buf(ia_bit_buf_struct *it_bit_buf, UWORD32 write_val, UWORD8 num_bits);

WORD32 iusace_write_escape_value(ia_bit_buf_struct *pstr_it_bit_buff, UWORD32 value,
                                 UWORD8 no_bits1, UWORD8 no_bits2, UWORD8 no_bits3);
