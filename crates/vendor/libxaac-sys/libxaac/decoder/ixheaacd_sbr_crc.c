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
#include "ixheaac_type_def.h"

#include "ixheaacd_bitbuffer.h"
#include "ixheaacd_sbr_crc.h"
#include "ixheaac_sbr_const.h"

static VOID ixheaacd_calc_chk_sum(WORD16* crc_state, WORD32 stream_data,
                                  WORD32 num_bits) {
  WORD32 i;
  WORD32 data_bit_mask = (1 << (num_bits - 1));
  WORD16 crc10 = SBR_CRC_POLY;
  WORD16 crc_mask = (1 << 9);
  WORD16 crc_state_local = *crc_state;

  for (i = 0; i < num_bits; i++) {
    WORD32 bit0, bit1;
    bit0 = ((crc_state_local & crc_mask) ? 1 : 0);
    bit1 = ((data_bit_mask & stream_data) ? 1 : 0);
    bit0 ^= bit1;
    crc_state_local = (WORD16)((WORD32)(crc_state_local & 0x0000FFFF) << 1);
    if (bit0) {
      crc_state_local ^= crc10;
    }
    data_bit_mask = (data_bit_mask >> 1);
  }
  *crc_state = crc_state_local;
  return;
}

static PLATFORM_INLINE WORD32 ixheaacd_sbr_crc(ia_bit_buf_struct* it_bit_buff,
                                               WORD32 num_crc_bits) {
  WORD32 i;
  WORD32 num_full_bytes, rem_bits;
  WORD32 byte_value;
  WORD16 crc_state = 0;

  num_full_bytes = (num_crc_bits >> 3);
  rem_bits = (num_crc_bits & 0x7);

  for (i = 0; i < num_full_bytes; i++) {
    byte_value = ixheaacd_read_bits_buf(it_bit_buff, 8);
    ixheaacd_calc_chk_sum(&crc_state, byte_value, 8);
  }

  byte_value = ixheaacd_read_bits_buf(it_bit_buff, rem_bits);
  ixheaacd_calc_chk_sum(&crc_state, byte_value, rem_bits);

  return (crc_state & 0x03FF);
}

FLAG ixheaacd_sbr_crccheck(ia_bit_buf_struct* it_bit_buff,
                           WORD32 crc_bits_len) {
  struct ia_bit_buf_struct it_bit_buff_local = {0};
  WORD32 num_crc_bits;
  WORD32 calc_crc_sum;
  WORD32 bits_available;
  WORD32 crc_check_sum;

  crc_check_sum = ixheaacd_read_bits_buf(it_bit_buff, SBR_CYC_REDCY_CHK_BITS);

  it_bit_buff_local = *it_bit_buff;

  bits_available = it_bit_buff->cnt_bits;

  if (bits_available <= 0) {
    return 0;
  }

  num_crc_bits =
      (crc_bits_len > bits_available) ? bits_available : crc_bits_len;

  calc_crc_sum = ixheaacd_sbr_crc(&it_bit_buff_local, num_crc_bits);

  if (calc_crc_sum != crc_check_sum) {
    return 0;
  }
  return 1;
}
