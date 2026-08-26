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

#include "ixheaacd_pns.h"

#include "ixheaacd_channelinfo.h"

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
#include "ixheaacd_intrinsics.h"

static PLATFORM_INLINE UWORD32
ixheaacd_aac_showbits_7(ia_bit_buf_struct *it_bit_buff) {
  UWORD8 *v = it_bit_buff->ptr_read_next;
  UWORD32 b = 0;
  UWORD32 x;
  b = ((WORD32)v[0] << 8);
  if (it_bit_buff->bit_pos < 6) {
    b |= (WORD32)(v[1]);
  }
  x = (UWORD32)b << (15 + 8 - it_bit_buff->bit_pos);
  x = (UWORD32)x >> (25);

  return x;
}

WORD ixheaacd_get_channel_mask(
    ia_exhaacplus_dec_api_struct *p_obj_enhaacplus_dec) {
  WORD ixheaacd_drc_offset = 0, channel_mask = 0;
  WORD flag1 = 0, flag2 = 0;
  WORD ch_idx;
  WORD *ptr_slot_element = p_obj_enhaacplus_dec->aac_config.slot_element;
  WORD *ptr_element_type = p_obj_enhaacplus_dec->aac_config.element_type;

  memset(ptr_slot_element, 0, sizeof(WORD) * MAX_BS_ELEMENT);

  for (ch_idx = 0; ch_idx < MAX_BS_ELEMENT; ch_idx++) {
    if (ptr_element_type[ch_idx] == 1) {
      channel_mask += 0x3;
      ptr_slot_element[ch_idx] = ixheaacd_drc_offset;
      ixheaacd_drc_offset += 2;
      flag1 = ch_idx + 1;
      break;
    }
  }

  for (ch_idx = 0; ch_idx < MAX_BS_ELEMENT; ch_idx++) {
    if (ptr_element_type[ch_idx] == 0) {
      channel_mask += 0x4;
      ptr_slot_element[ch_idx] = ixheaacd_drc_offset;
      ixheaacd_drc_offset += 1;
      flag2 = ch_idx + 1;
      break;
    }
  }
  for (ch_idx = 0; ch_idx < MAX_BS_ELEMENT; ch_idx++) {
    if (ptr_element_type[ch_idx] == 3) {
      channel_mask += 0x8;
      ptr_slot_element[ch_idx] = ixheaacd_drc_offset;
      ixheaacd_drc_offset += 1;
      break;
    }
  }
  for (ch_idx = flag1; ch_idx < MAX_BS_ELEMENT; ch_idx++) {
    if (ptr_element_type[ch_idx] == 1) {
      channel_mask += 0x30;
      ptr_slot_element[ch_idx] = ixheaacd_drc_offset;
      ixheaacd_drc_offset += 2;
      flag1 = ch_idx + 1;
      break;
    }
  }
  for (ch_idx = flag2; ch_idx < MAX_BS_ELEMENT; ch_idx++) {
    if (ptr_element_type[ch_idx] == 0) {
      channel_mask += 0x100;
      ptr_slot_element[ch_idx] = ixheaacd_drc_offset;
      ixheaacd_drc_offset += 1;
      break;
    }
  }
  for (ch_idx = flag1; ch_idx < MAX_BS_ELEMENT; ch_idx++) {
    if (ptr_element_type[ch_idx] == 1) {
      {
        channel_mask += (0x40 + 0x80);
        ptr_slot_element[ch_idx] = ixheaacd_drc_offset;
        ixheaacd_drc_offset += 2;
        break;
      }
    }
  }

  return channel_mask;
}

VOID ixheaacd_read_data_stream_element(ia_bit_buf_struct *it_bit_buff,
                                       WORD32 *byte_align_bits,
                                       ia_drc_dec_struct *drc_handle) {
  ia_bit_buf_struct temp_bs = {0};
  WORD32 count = ixheaacd_read_bits_buf(it_bit_buff, 13);
  WORD32 cnt = (count & 0xff);
  WORD32 start_pos = 0;

  if (cnt == 255) {
    cnt += ixheaacd_read_bits_buf(it_bit_buff, 8);
  }

  if ((count & 0x0100) >> 8) {
    ixheaacd_byte_align(it_bit_buff, byte_align_bits);
  }

  {
    memcpy(&temp_bs, it_bit_buff, sizeof(ia_bit_buf_struct));
    start_pos = temp_bs.cnt_bits;

    if (ixheaacd_read_bits_buf(&temp_bs, 8) == DVB_ANC_DATA_SYNC_BYTE) {
      int dmx_level_present, compression_present;
      int coarse_gain_present, fine_grain_present;

      ixheaacd_read_bits_buf(&temp_bs, 8);

      ixheaacd_read_bits_buf(&temp_bs, 3);
      dmx_level_present = ixheaacd_read_bits_buf(&temp_bs, 1);
      ixheaacd_read_bits_buf(&temp_bs, 1);
      compression_present = ixheaacd_read_bits_buf(&temp_bs, 1);
      coarse_gain_present = ixheaacd_read_bits_buf(&temp_bs, 1);
      fine_grain_present = ixheaacd_read_bits_buf(&temp_bs, 1);

      if (dmx_level_present) ixheaacd_read_bits_buf(&temp_bs, 8);

      if (compression_present) ixheaacd_read_bits_buf(&temp_bs, 16);

      if (coarse_gain_present) ixheaacd_read_bits_buf(&temp_bs, 16);

      if (fine_grain_present) ixheaacd_read_bits_buf(&temp_bs, 16);

      if (!drc_handle->dvb_anc_data_present && temp_bs.cnt_bits >= 0) {
        drc_handle->dvb_anc_data_pos = start_pos;
        drc_handle->dvb_anc_data_present = 1;
      }
    }
  }

  if (it_bit_buff->cnt_bits < (cnt << 3)) {
    longjmp(*(it_bit_buff->xaac_jmp_buf),
            IA_XHEAAC_DEC_EXE_NONFATAL_INSUFFICIENT_INPUT_BYTES);
  }
  it_bit_buff->ptr_read_next += cnt;
  it_bit_buff->cnt_bits -= ((cnt) << 3);
}

VOID ixheaacd_read_fill_element(
    ia_bit_buf_struct *it_bit_buff, ia_drc_dec_struct *drc_dummy,
    ia_drc_dec_struct *ptr_drc_dec, UWORD8 *mps_buffer, WORD32 *mps_header,
    WORD32 *mps_bytes) {
  WORD32 count;
  count = ixheaacd_read_bits_buf(it_bit_buff, 4);

  if ((count - 15) == 0) {
    count = ixheaacd_read_bits_buf(it_bit_buff, 8);
    count = (count + 14);
  }

  if (count > 0) {
    WORD32 extension_type;

    extension_type = ixheaacd_read_bits_buf(it_bit_buff, 4);

    if (extension_type == EXT_DYNAMIC_RANGE) {
      ptr_drc_dec->drc_element_found = 1;
      count -=
          ixheaacd_dec_drc_read_element(ptr_drc_dec, drc_dummy, it_bit_buff);
    }
    if (EXT_SAC_DATA == extension_type) {
      WORD32 anc_type, i;
      anc_type = ixheaacd_read_bits_buf(it_bit_buff, 2);
      *mps_header = anc_type;

      ixheaacd_read_bits_buf(it_bit_buff, 1);

      ixheaacd_read_bits_buf(it_bit_buff, 1);

      for (i = 0; i < count - 1; i++) {
        mps_buffer[i] = ixheaacd_read_bits_buf(it_bit_buff, 8);
      }

      *mps_bytes = count - 1;
    }
    else {
      ixheaacd_read_bits_buf(it_bit_buff, 4);

      if (it_bit_buff->cnt_bits < ((count - 1) << 3)) {
        longjmp(*(it_bit_buff->xaac_jmp_buf),
                IA_XHEAAC_DEC_EXE_NONFATAL_INSUFFICIENT_INPUT_BYTES);
      }
      it_bit_buff->ptr_read_next += count - 1;
      it_bit_buff->cnt_bits -= ((count - 1) << 3);
    }
  }
}

WORD32 ixheaacd_get_element_index_tag(
    ia_exhaacplus_dec_api_struct *p_obj_enhaacplus_dec, WORD ch_idx1,
    WORD *ch_idx, WORD *channel, WORD *ele_idx_order, WORD total_elements,
    WORD8 *element_used, WORD total_channels, ia_drc_dec_struct *pstr_drc_dec,
    ia_drc_dec_struct *drc_dummy
    ,
    UWORD8 *mps_buffer, WORD32 *mps_header, WORD32 *mps_bytes
) {
  WORD element_tag, j;
  ia_aac_dec_state_struct *p_state_enhaacplus_dec =
      p_obj_enhaacplus_dec->p_state_aac;

  ia_bit_buf_struct *it_bit_buff = p_state_enhaacplus_dec->ptr_bit_stream;
  WORD element_idx;
  WORD element_type;

  ia_aac_decoder_struct *aac_dec_handle =
      p_state_enhaacplus_dec->pstr_aac_dec_info[ch_idx1];

  *ch_idx = ch_idx1;

  if (p_state_enhaacplus_dec->bs_format != LOAS_BSFORMAT) {
    if (ch_idx1 == 0) {
      ixheaacd_byte_align(it_bit_buff, &aac_dec_handle->byte_align_bits);
    }
  }
  {
    if (ch_idx1 == 0) {
      aac_dec_handle->byte_align_bits = it_bit_buff->cnt_bits;
    }
  }

  if (it_bit_buff->cnt_bits < 3) {
    it_bit_buff->cnt_bits = -1;
    return (WORD16)(
        (WORD32)IA_XHEAAC_DEC_EXE_NONFATAL_INSUFFICIENT_INPUT_BYTES);
  }

  element_tag = (WORD)ixheaacd_read_bits_buf(it_bit_buff, 7);
  ixheaacd_read_bidirection(it_bit_buff, -7);

  element_idx = (element_tag & 0xF);
  element_type = (element_tag >> 4) & 0x7;

  p_obj_enhaacplus_dec->aac_config.str_prog_config.alignment_bits =
      it_bit_buff->bit_pos;

  while (element_type == 4 || element_type == 5 || element_type == 6) {
    WORD type = (WORD)ixheaacd_read_bits_buf(it_bit_buff, 3);

    if (it_bit_buff->cnt_bits < 3) {
      it_bit_buff->cnt_bits = -1;
      return (WORD16)(
          (WORD32)IA_XHEAAC_DEC_EXE_NONFATAL_INSUFFICIENT_INPUT_BYTES);
    }

    if (type == 4) {
      ixheaacd_read_data_stream_element(
          it_bit_buff, &aac_dec_handle->byte_align_bits, pstr_drc_dec);
    }
    if (type == 5) {
      WORD32 error_code = 0;
      error_code = ixheaacd_decode_pce(
          it_bit_buff, &p_obj_enhaacplus_dec->aac_config.ui_pce_found_in_hdr,
          &p_obj_enhaacplus_dec->aac_config.str_prog_config);
      if (error_code != 0) {
        if (error_code < 0) return error_code;
        return IA_XHEAAC_DEC_EXE_NONFATAL_DECODE_FRAME_ERROR;
      }
    }
    if (type == 6) {
      ixheaacd_read_fill_element(it_bit_buff, drc_dummy, pstr_drc_dec,
                                 mps_buffer, mps_header, mps_bytes);
    }

    if (it_bit_buff->cnt_bits < 7) {
      it_bit_buff->cnt_bits = -1;
      return (WORD16)(
          (WORD32)IA_XHEAAC_DEC_EXE_NONFATAL_INSUFFICIENT_INPUT_BYTES);
    }

    element_tag = (WORD)ixheaacd_aac_showbits_7(it_bit_buff);
    element_idx = (element_tag & 0xF);
    element_type = (element_tag >> 4) & 0x7;
  }

  if (total_elements == 2 && total_channels == 2 &&
      p_state_enhaacplus_dec->p_config->ui_pce_found_in_hdr == 2 &&
      ch_idx1 == 0) {
    ixheaacd_fill_prog_config_slots(p_state_enhaacplus_dec);
  }

  *channel = 1;
  if (element_type == 1) {
    *channel = 2;
  }

  for (j = 0; j < total_elements; j++) {
    if (p_obj_enhaacplus_dec->aac_config.element_type[j] == element_type &&
        (element_idx == ele_idx_order[j]) && (element_used[j] == 0)) {
      *ch_idx = j;
      element_used[j] = 1;
      break;
    }
  }

  if (j == total_elements) {
    if (it_bit_buff->cnt_bits < 0) {
      return (WORD16)(
          (WORD32)IA_XHEAAC_DEC_EXE_NONFATAL_INSUFFICIENT_INPUT_BYTES);
    }

    ixheaacd_read_bidirection(
        it_bit_buff, (WORD16)(it_bit_buff->cnt_bits - it_bit_buff->size));
    return IA_XHEAAC_DEC_EXE_NONFATAL_ELE_INSTANCE_TAG_NOT_FOUND;
  } else
    return 0;
}
