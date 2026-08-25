/******************************************************************************
 *
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
#ifndef IMPD_DRC_BITBUFFER_H
#define IMPD_DRC_BITBUFFER_H

typedef struct ia_bit_buf_struct {
  UWORD8 *ptr_bit_buf_base;
  UWORD8 *ptr_bit_buf_end;

  UWORD8 *ptr_read_next;

  WORD32 bit_pos;
  WORD32 cnt_bits;

  WORD32 size;
  WORD32 error;

} ia_bit_buf_struct;

VOID impd_create_bit_buf(ia_bit_buf_struct *it_bit_buff,
                         UWORD8 *ptr_bit_buf_base, WORD32 bit_buf_size);

VOID impd_create_init_bit_buf(ia_bit_buf_struct *it_bit_buff,
                              UWORD8 *ptr_bit_buf_base, WORD32 bit_buf_size);

WORD32 impd_read_bits_buf(ia_bit_buf_struct *it_bit_buff, WORD no_of_bits);

WORD32 impd_skip_bits_buf(ia_bit_buf_struct *it_bit_buff, WORD no_of_bits);
#endif
