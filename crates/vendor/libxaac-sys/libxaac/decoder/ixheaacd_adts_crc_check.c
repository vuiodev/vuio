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
#include "ixheaac_constants.h"
#include "ixheaacd_memory_standards.h"
#include "ixheaacd_mps_struct_def.h"
#include "ixheaacd_mps_res_rom.h"
#include "ixheaacd_defines.h"
#include "ixheaacd_mps_aac_struct.h"

#include "ixheaacd_sbr_common.h"
#include "ixheaacd_bitbuffer.h"
#include "ixheaacd_defines.h"
#include "ixheaacd_aac_rom.h"

#include "ixheaacd_sbrdecsettings.h"
#include "ixheaacd_sbr_scale.h"
#include "ixheaacd_env_extr_part.h"
#include "ixheaacd_sbr_rom.h"

#include "ixheaacd_common_rom.h"
#include "ixheaacd_pulsedata.h"

#include "ixheaacd_pns.h"
#include "ixheaacd_drc_data_struct.h"

#include "ixheaacd_lt_predict.h"
#include "ixheaacd_cnst.h"
#include "ixheaacd_ec_defines.h"
#include "ixheaacd_ec_struct_def.h"

#include "ixheaacd_channelinfo.h"
#include "ixheaacd_drc_dec.h"
#include "ixheaacd_sbrdecoder.h"

#include "ixheaacd_channel.h"

#include "ixheaacd_audioobjtypes.h"
#include "ixheaacd_memory_standards.h"
#include "ixheaacd_adts.h"
#include "ixheaacd_audioobjtypes.h"
#include "ixheaacd_latmdemux.h"
#include "ixheaacd_aacdec.h"

#include "ixheaacd_hybrid.h"
#include "ixheaacd_ps_dec.h"

#include "ixheaacd_mps_polyphase.h"
#include "ixheaacd_config.h"
#include "ixheaacd_qmf_dec.h"
#include "ixheaacd_mps_dec.h"

#include "ixheaacd_struct_def.h"
#include "ixheaacd_error_codes.h"

#include "ixheaacd_adts_crc_check.h"

VOID ixheaacd_adts_crc_open(ia_adts_crc_info_struct *ptr_adts_crc_info) {
  WORD32 i, j;
  UWORD16 val;

  ptr_adts_crc_info->no_reg = 0;
  ptr_adts_crc_info->crc_active = 0;

  for (i = 0; i <= 255; ++i) {
    for (val = i << 8, j = 8; --j >= 0;) {
      val = (val & 0x8000) ? (val << 1) ^ 0x8005 : val << 1;
    }

    ptr_adts_crc_info->crc_lookup[i] = val;
  }
}

VOID ixheaacd_copy_bit_buf_state(
    ia_bit_buf_struct *it_bit_buff_src,
    ia_crc_bit_buf_struct_handle it_crc_bit_buff_dst) {
  it_crc_bit_buff_dst->ptr_bit_buf_base = it_bit_buff_src->ptr_bit_buf_base;
  it_crc_bit_buff_dst->ptr_bit_buf_end = it_bit_buff_src->ptr_bit_buf_end;
  it_crc_bit_buff_dst->ptr_read_next = it_bit_buff_src->ptr_read_next;
  it_crc_bit_buff_dst->bit_pos = it_bit_buff_src->bit_pos;
  it_crc_bit_buff_dst->cnt_bits = it_bit_buff_src->cnt_bits;
  it_crc_bit_buff_dst->size = it_bit_buff_src->size;
}

WORD32 ixheaacd_adts_crc_start_reg(ia_adts_crc_info_struct *ptr_adts_crc_info,
                                   ia_bit_buf_struct *it_bit_buff_src,
                                   WORD32 no_bits) {
  UWORD32 no_bytes;

  ptr_adts_crc_info->str_crc_reg_data[ptr_adts_crc_info->no_reg].bit_cnt = 0;
  ptr_adts_crc_info->str_crc_reg_data[ptr_adts_crc_info->no_reg].max_bits =
      no_bits;

  if (no_bits < 0) {
    no_bits = -no_bits;
  }

  if (no_bits == 0) {
    no_bits = 16 << 3;
  }

  no_bytes = no_bits >> 3;

  if (no_bytes << 3 < (UWORD32)no_bits) {
    no_bytes++;
  }

  ptr_adts_crc_info->str_crc_reg_data[ptr_adts_crc_info->no_reg].buf_size =
      no_bytes;
  ptr_adts_crc_info->str_crc_reg_data[ptr_adts_crc_info->no_reg].active = 1;

  ixheaacd_copy_bit_buf_state(
      it_bit_buff_src,
      &(ptr_adts_crc_info->str_crc_reg_data[ptr_adts_crc_info->no_reg]
            .str_bit_buf));

  ptr_adts_crc_info->no_reg += 1;

  return (ptr_adts_crc_info->no_reg - 1);
}

VOID ixheaacd_adts_crc_end_reg(ia_adts_crc_info_struct *ptr_adts_crc_info,
                               ia_bit_buf_struct *it_bit_buff_src, WORD32 reg) {
  ptr_adts_crc_info->str_crc_reg_data[reg].active = 0;
  ptr_adts_crc_info->str_crc_reg_data[reg].bit_buf_cnt =
      ptr_adts_crc_info->str_crc_reg_data[reg].str_bit_buf.cnt_bits -
      it_bit_buff_src->cnt_bits;
}

VOID ixheaacd_adts_crc_fast_crc(ia_adts_crc_info_struct *ptr_adts_crc_info,
                                UWORD16 *crc_reg, UWORD8 feed) {
  *crc_reg =
      (*crc_reg << 8) ^ ptr_adts_crc_info->crc_lookup[(*crc_reg >> 8) ^ feed];
}

VOID ixheaacd_adts_crc_slow_crc(UWORD16 *crc_reg, UWORD8 feed,
                                UWORD32 no_bits) {
  UWORD32 i;
  UWORD16 tmp;
  for (i = 0; i < no_bits; i++) {
    tmp = (feed & (1 << (7 - i))) >> (7 - i);
    tmp ^= (*crc_reg & (1 << 15)) >> 15;
    tmp *= 32773;
    *crc_reg <<= 1;
    *crc_reg ^= tmp;
  }
}

WORD32 ixheaacd_adts_crc_check_crc(ia_adts_crc_info_struct *ptr_adts_crc_info) {
  WORD32 error_code = AAC_DEC_OK;
  UWORD16 crc = 65535;
  WORD32 reg;
  ia_crc_reg_data_struct *ptr_reg_data;

  for (reg = 0; reg < ptr_adts_crc_info->no_reg; reg++) {
    UWORD8 bits;
    WORD32 bits_remaining;

    ptr_reg_data = &ptr_adts_crc_info->str_crc_reg_data[reg];

    if (ptr_reg_data->max_bits > 0) {
      if (ptr_adts_crc_info->str_crc_reg_data[reg].bit_buf_cnt >
          ptr_reg_data->max_bits)
        bits_remaining = ptr_reg_data->max_bits;
      else
        bits_remaining = ptr_adts_crc_info->str_crc_reg_data[reg].bit_buf_cnt;
    } else {
      bits_remaining = ptr_adts_crc_info->str_crc_reg_data[reg].bit_buf_cnt;
    }

    while (bits_remaining >= 8) {
      if (ptr_reg_data->str_bit_buf.cnt_bits < 8) {
        return IA_XHEAAC_DEC_EXE_NONFATAL_INSUFFICIENT_INPUT_BYTES;
      }
      bits = (UWORD8)ixheaacd_read_bits_buf(
          (ia_bit_buf_struct *)(&ptr_adts_crc_info->str_crc_reg_data[reg]
                                     .str_bit_buf),
          8);
      ixheaacd_adts_crc_fast_crc(ptr_adts_crc_info, &crc, bits);
      bits_remaining -= 8;
    }

    if (ptr_reg_data->str_bit_buf.cnt_bits < bits_remaining) {
      return IA_XHEAAC_DEC_EXE_NONFATAL_INSUFFICIENT_INPUT_BYTES;
    }
    bits = (UWORD8)ixheaacd_read_bits_buf(
        (ia_bit_buf_struct *)(&ptr_adts_crc_info->str_crc_reg_data[reg]
                                   .str_bit_buf),
        bits_remaining);
    ixheaacd_adts_crc_slow_crc(&crc, (UWORD8)(bits << (8 - bits_remaining)),
                               bits_remaining);

    if (ptr_reg_data->max_bits >
        ptr_adts_crc_info->str_crc_reg_data[reg].bit_buf_cnt) {
      bits_remaining = ptr_reg_data->max_bits -
                       ptr_adts_crc_info->str_crc_reg_data[reg].bit_buf_cnt;

      for (; bits_remaining >= 8; bits_remaining -= 8) {
        ixheaacd_adts_crc_fast_crc(ptr_adts_crc_info, &crc, 0);
      }

      ixheaacd_adts_crc_slow_crc(&crc, 0, bits_remaining);
    }
  }

  ptr_adts_crc_info->no_reg = 0;

  if (crc != ptr_adts_crc_info->file_value) {
    return (IA_XHEAAC_DEC_EXE_NONFATAL_ADTS_HDR_CRC_FAIL);
  }

  return (error_code);
}
