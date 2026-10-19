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
#include <string.h>
#include "ixheaacd_sbr_common.h"
#include "ixheaac_type_def.h"

#include "ixheaac_constants.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops16.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaacd_bitbuffer.h"
#include "ixheaacd_defines.h"
#include "ixheaacd_aac_rom.h"
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

#include "ixheaacd_audioobjtypes.h"
#include "ixheaacd_sbrdecsettings.h"
#include "ixheaacd_memory_standards.h"
#include "ixheaacd_error_codes.h"

#include "ixheaacd_defines.h"

#include "ixheaacd_sbr_scale.h"
#include "ixheaacd_lpp_tran.h"
#include "ixheaacd_env_extr_part.h"
#include "ixheaacd_sbr_rom.h"

#include "ixheaacd_hybrid.h"
#include "ixheaacd_ps_dec.h"
#include "ixheaacd_ps_bitdec.h"

#include "ixheaacd_pulsedata.h"

#include "ixheaacd_env_extr.h"
#include "ixheaacd_common_rom.h"
#include "ixheaacd_block.h"
#include "ixheaacd_channel.h"

#include "ixheaacd_audioobjtypes.h"
#include "ixheaacd_latmdemux.h"

#include "ixheaacd_aacdec.h"
#include "ixheaacd_hybrid.h"
#include "ixheaacd_ps_dec.h"
#include "ixheaacd_mps_polyphase.h"
#include "ixheaacd_config.h"
#include "ixheaacd_qmf_dec.h"
#include "ixheaacd_mps_macro_def.h"
#include "ixheaacd_mps_struct_def.h"
#include "ixheaacd_mps_res_rom.h"
#include "ixheaacd_mps_aac_struct.h"
#include "ixheaacd_mps_dec.h"
#include "ixheaacd_struct_def.h"
#include "ixheaacd_headerdecode.h"

#include "ixheaacd_multichannel.h"

#include "ixheaac_basic_op.h"
#include "ixheaacd_adts_crc_check.h"
#include "ixheaacd_function_selector.h"

VOID ixheaacd_allocate_mem_persistent(
    ia_exhaacplus_dec_api_struct *p_obj_enhaacplus_dec,
    ia_aac_dec_state_struct *p_state_enhaacplus_dec, WORD channels,
    WORD *persistent_used_total, WORD *sbr_persistent_start, WORD ps_enable) {
  WORD persistent_used;
  WORD8 **temp_persistent =
      (WORD8 **)&(p_state_enhaacplus_dec->aac_persistent_mem_v);
  *temp_persistent += *persistent_used_total;

  persistent_used = ixheaacd_set_aac_persistent_buffers(
      p_state_enhaacplus_dec->aac_persistent_mem_v, channels);

  *persistent_used_total += persistent_used;

  *sbr_persistent_start = *persistent_used_total;

  p_state_enhaacplus_dec->sbr_persistent_mem_v =
      (pVOID)((pWORD8)p_state_enhaacplus_dec->aac_persistent_mem_v +
              IXHEAAC_GET_SIZE_ALIGNED(persistent_used, BYTE_ALIGN_8));

  persistent_used = ixheaacd_getsize_sbr_persistent();

  ixheaacd_set_sbr_persistent_buffers(
      p_state_enhaacplus_dec->sbr_persistent_mem_v, &persistent_used, channels,
      ps_enable);

  *persistent_used_total += persistent_used;

  {
    struct ia_aac_persistent_struct *aac_persistent_mem =
        (struct ia_aac_persistent_struct *)
            p_obj_enhaacplus_dec->p_state_aac->aac_persistent_mem_v;
    aac_persistent_mem->str_aac_decoder.pstr_aac_tables =
        &p_obj_enhaacplus_dec->aac_tables;
    aac_persistent_mem->str_aac_decoder.pstr_common_tables =
        p_obj_enhaacplus_dec->common_tables;
  }

  ixheaacd_set_sbr_persistent_table_pointer(
      p_obj_enhaacplus_dec->p_state_aac->sbr_persistent_mem_v,
      &p_obj_enhaacplus_dec->str_sbr_tables,
      p_obj_enhaacplus_dec->common_tables);
}

ia_bit_buf_struct *ixheaacd_create_bit_buf(ia_bit_buf_struct *it_bit_buff,
                                           UWORD8 *ptr_bit_buf_base,
                                           WORD32 bit_buf_size) {
  it_bit_buff->ptr_bit_buf_base = ptr_bit_buf_base;
  it_bit_buff->ptr_bit_buf_end = ptr_bit_buf_base + bit_buf_size - 1;

  it_bit_buff->ptr_read_next = ptr_bit_buf_base;
  it_bit_buff->bit_pos = 7;

  it_bit_buff->cnt_bits = 0;
  it_bit_buff->size = bit_buf_size << 3;

  it_bit_buff->adts_header_present = 0;
  it_bit_buff->protection_absent = 0;
  it_bit_buff->pstr_adts_crc_info = &it_bit_buff->str_adts_crc_info;

  it_bit_buff->max_size = it_bit_buff->size;

  ixheaacd_adts_crc_open(it_bit_buff->pstr_adts_crc_info);

  return it_bit_buff;
}

VOID ixheaacd_create_init_bit_buf(ia_bit_buf_struct *it_bit_buff,
                                  UWORD8 *ptr_bit_buf_base,
                                  WORD32 bit_buf_size) {
  ixheaacd_create_bit_buf(it_bit_buff, ptr_bit_buf_base, bit_buf_size);
  it_bit_buff->cnt_bits = (bit_buf_size << 3);
  return;
}

VOID ixheaacd_read_bidirection(ia_bit_buf_struct *it_bit_buff,
                               WORD32 ixheaacd_drc_offset) {
  if (ixheaacd_drc_offset != 0) {
    WORD32 byte_offset;
    if ((it_bit_buff->cnt_bits < 0) ||
        (it_bit_buff->cnt_bits - ixheaacd_drc_offset < 0) ||
        (it_bit_buff->cnt_bits - ixheaacd_drc_offset > it_bit_buff->size)) {
      longjmp(*(it_bit_buff->xaac_jmp_buf),
              IA_XHEAAC_DEC_EXE_NONFATAL_INSUFFICIENT_INPUT_BYTES);
    }
    it_bit_buff->cnt_bits = it_bit_buff->cnt_bits - ixheaacd_drc_offset;
    it_bit_buff->bit_pos = it_bit_buff->bit_pos - ixheaacd_drc_offset;
    byte_offset = it_bit_buff->bit_pos >> 3;
    it_bit_buff->bit_pos = it_bit_buff->bit_pos - (byte_offset << 3);

    if (byte_offset) {
      UWORD8 *ptr_read_next;

      ptr_read_next = it_bit_buff->ptr_read_next;

      ptr_read_next = ptr_read_next - (byte_offset);

      it_bit_buff->ptr_read_next = ptr_read_next;
    }
  }
}
