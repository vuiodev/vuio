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
#ifndef IXHEAACD_BITBUFFER_H
#define IXHEAACD_BITBUFFER_H

#include <setjmp.h>

#define CRC_ADTS_HEADER_LEN 56
#define CRC_ADTS_RAW_DATA_BLK_LEN 192
#define CRC_ADTS_RAW_IIND_ICS 128
#define CRC_ADTS_LEN_ALL -1

#define MAX_REG_SIZE 192
#define MAX_CRC_REGS 7

struct ia_crc_bit_buf_struct {
  UWORD8 *ptr_bit_buf_base;
  UWORD8 *ptr_bit_buf_end;

  UWORD8 *ptr_read_next;
  WORD16 bit_pos;
  WORD32 cnt_bits;

  WORD32 size;
};

typedef struct {
  UWORD8 active;
  WORD32 buf_size;
  WORD32 max_bits;
  UWORD32 bit_cnt;
  WORD32 bit_buf_cnt;
  struct ia_crc_bit_buf_struct str_bit_buf;
} ia_crc_reg_data_struct;

typedef struct {
  UWORD8 crc_active;
  UWORD16 no_reg;
  UWORD16 file_value;
  UWORD16 crc_lookup[256];
  ia_crc_reg_data_struct str_crc_reg_data[MAX_CRC_REGS];
} ia_adts_crc_info_struct;

typedef struct ia_bit_buf_struct {
  UWORD8 *ptr_bit_buf_base;
  UWORD8 *ptr_bit_buf_end;

  UWORD8 *ptr_read_next;

  WORD32 bit_pos;
  WORD32 cnt_bits;

  WORD32 size;

  WORD32 adts_header_present;
  WORD32 crc_check;
  WORD8 protection_absent;
  WORD8 no_raw_data_blocks;
  ia_adts_crc_info_struct str_adts_crc_info;
  ia_adts_crc_info_struct *pstr_adts_crc_info;

  WORD32 initial_cnt_bits;
  WORD32 audio_mux_align;
  WORD32 bit_count;
  WORD32 valid_bits;
  UWORD8 byte;
  UWORD8 *byte_ptr;
  UWORD8 *ptr_start;
  WORD32 write_bit_count;
  WORD32 max_size;
  jmp_buf *xaac_jmp_buf;

} ia_bit_buf_struct;

typedef struct ia_crc_bit_buf_struct *ia_crc_bit_buf_struct_handle;

VOID ixheaacd_byte_align(ia_bit_buf_struct *it_bit_buff,
                         WORD32 *ptr_byte_align_bits);

ia_bit_buf_struct *ixheaacd_create_bit_buf(ia_bit_buf_struct *it_bit_buff,
                                           UWORD8 *ptr_bit_buf_base,
                                           WORD32 bit_buf_size);

VOID ixheaacd_create_init_bit_buf(ia_bit_buf_struct *it_bit_buff,
                                  UWORD8 *ptr_bit_buf_base,
                                  WORD32 bit_buf_size);

WORD32 ixheaacd_read_bits_buf(ia_bit_buf_struct *it_bit_buff, WORD no_of_bits);

WORD32 ixheaacd_skip_bits_buf(ia_bit_buf_struct *it_bit_buff, WORD no_of_bits);

WORD32 ixheaacd_show_bits_buf(ia_bit_buf_struct *it_bit_buff, WORD no_of_bits);

VOID ixheaacd_read_bidirection(ia_bit_buf_struct *it_bit_buff,
                               WORD32 ixheaacd_drc_offset);

UWORD32 ixheaacd_aac_showbits_32(UWORD8 *ptr_read_next, WORD32 cnt_bits,
                                 WORD32 *increment);

VOID ixheaacd_aac_read_byte(UWORD8 **ptr_read_next, WORD32 *bit_pos,
                            WORD32 *readword);

VOID ixheaacd_aac_read_byte_corr(UWORD8 **ptr_read_next, WORD32 *ptr_bit_pos,
                                 WORD32 *readword, UWORD8 *p_bit_buf_end);

VOID ixheaacd_aac_read_byte_corr1(UWORD8 **ptr_read_next, WORD32 *ptr_bit_pos,
                                  WORD32 *readword, UWORD8 *p_bit_buf_end);

#define get_no_bits_available(it_bit_buff) ((it_bit_buff)->cnt_bits)
#define ixheaacd_no_bits_read(it_bit_buff) \
  ((it_bit_buff)->size - (it_bit_buff)->cnt_bits)

WORD32 ixheaacd_aac_read_bit_rev(ia_bit_buf_struct *it_bit_buff);
WORD32 ixheaacd_aac_read_bit(ia_bit_buf_struct *it_bit_buff);

VOID ixheaacd_write_bit(ia_bit_buf_struct *it_bit_buff, WORD32 value,
                        WORD32 no_of_bits);

WORD32 ixheaacd_read_bit(ia_bit_buf_struct *data, WORD32 no_of_bits);

#endif /* IXHEAACD_BITBUFFER_H */
