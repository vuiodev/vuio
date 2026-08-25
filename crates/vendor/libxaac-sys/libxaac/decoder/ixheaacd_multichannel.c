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

#include "ixheaac_type_def.h"
#include "ixheaacd_sbr_common.h"

#include "ixheaac_constants.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops16.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaac_basic_ops.h"

#include "ixheaacd_bitbuffer.h"

#include "ixheaacd_audioobjtypes.h"
#include "ixheaacd_sbrdecsettings.h"
#include "ixheaacd_memory_standards.h"
#include "ixheaacd_error_codes.h"

#include "ixheaacd_defines.h"
#include "ixheaacd_aac_rom.h"
#include "ixheaacd_pns.h"

#include "ixheaacd_pulsedata.h"
#include "ixheaacd_drc_data_struct.h"

#include "ixheaacd_lt_predict.h"
#include "ixheaacd_cnst.h"
#include "ixheaacd_ec_defines.h"
#include "ixheaacd_ec_struct_def.h"
#include "ixheaacd_channelinfo.h"
#include "ixheaacd_drc_dec.h"

#include "ixheaacd_sbrdecoder.h"
#include "ixheaacd_sbr_scale.h"
#include "ixheaacd_lpp_tran.h"
#include "ixheaacd_env_extr_part.h"
#include "ixheaacd_sbr_rom.h"

#include "ixheaacd_hybrid.h"
#include "ixheaacd_ps_dec.h"
#include "ixheaacd_ps_bitdec.h"

#include "ixheaacd_pulsedata.h"

#include "ixheaacd_pns.h"

#include "ixheaacd_env_extr.h"
#include "ixheaacd_common_rom.h"
#include "ixheaacd_block.h"
#include "ixheaacd_channel.h"
#include "ixheaacd_audioobjtypes.h"
#include "ixheaacd_latmdemux.h"
#include "ixheaacd_aacdec.h"
#include "ixheaacd_config.h"
#include "ixheaacd_hybrid.h"
#include "ixheaacd_ps_dec.h"
#include "ixheaacd_qmf_dec.h"
#include "ixheaacd_mps_polyphase.h"
#include "ixheaacd_mps_macro_def.h"
#include "ixheaacd_mps_struct_def.h"
#include "ixheaacd_mps_res_rom.h"
#include "ixheaacd_mps_aac_struct.h"
#include "ixheaacd_mps_dec.h"
#include "ixheaacd_mps_interface.h"
#include "ixheaacd_struct_def.h"
#include "ixheaacd_headerdecode.h"

#include "ixheaacd_multichannel.h"
#include "ixheaac_basic_op.h"

WORD cblock_decode_huff_symbol(UWORD8 *ptr_read_next, WORD32 bit_pos,
                               const UWORD16 *huff_ori, WORD16 *input,
                               WORD32 *readword)

{
  const UWORD16 *h;
  WORD tot_bits;
  {
    UWORD16 first_offset;
    WORD16 sign_ret_val;
    UWORD32 read_word1;

    read_word1 = *readword << bit_pos;

    h = (UWORD16 *)(huff_ori);
    first_offset = 7;

    h += (read_word1) >> (32 - first_offset);
    sign_ret_val = *h;
    tot_bits = 0;

    while (sign_ret_val > 0) {
      tot_bits += first_offset;
      bit_pos += first_offset;

      if ((bit_pos -= 8) >= 0) {
        *readword = (*readword << 8) | *ptr_read_next;
        ptr_read_next++;
      } else {
        bit_pos += 8;
      }

      read_word1 = (read_word1) << (first_offset);

      first_offset = (sign_ret_val >> 11);
      h += sign_ret_val & (0x07FF);

      h += (read_word1) >> (32 - first_offset);
      sign_ret_val = *h;
    }

    tot_bits += ((sign_ret_val & 0x7fff) >> 11);
    bit_pos += ((sign_ret_val & 0x7fff) >> 11);
    if ((bit_pos - 8) >= 0) {
      *readword = (*readword << 8) | *ptr_read_next;
    }

    *input = (sign_ret_val & (0x07FF)) - 60;
  }

  return tot_bits;
}

IA_ERRORCODE ixheaacd_dec_coupling_channel_element(
    ia_bit_buf_struct *it_bit_buf, ia_aac_decoder_struct *aac_handle,
    WORD32 samp_rate_idx, ia_aac_dec_tables_struct *ptr_aac_tables,
    ixheaacd_misc_tables *common_tables_ptr, WORD *element_index_order,
    ia_enhaacplus_dec_ind_cc *ind_channel_info, WORD32 total_channels,
    WORD32 frame_size, WORD32 audio_object_type,
    ia_eld_specific_config_struct eld_specific_config, WORD32 ele_type) {
  WORD32 element_instance_tag;
  LOOPIDX c;

  WORD ind_sw_cce_flag, num_coupled_elements;

  WORD num_gain_element_lists = 0;
  WORD gain_element_scale;

  const UWORD16 *hcod_sf =
      ptr_aac_tables->pstr_huffmann_tables->huffman_code_book_scl;
  const UWORD32 *table_idx =
      ptr_aac_tables->pstr_huffmann_tables->huffman_code_book_scl_index;
  WORD16 index, length;

  IA_ERRORCODE error_status = IA_NO_ERROR;

  element_instance_tag = ixheaacd_read_bits_buf(it_bit_buf, 4);
  element_index_order[0] = element_instance_tag;

  ind_sw_cce_flag = ixheaacd_read_bits_buf(it_bit_buf, 1);
  num_coupled_elements = ixheaacd_read_bits_buf(it_bit_buf, 3);

  for (c = 0; c < MAX_BS_ELEMENT; c++)
    ind_channel_info->elements_coupled[c] = -1;

  ind_channel_info->num_coupled_elements = num_coupled_elements;

  for (c = 0; c < (num_coupled_elements + 1); c++) {
    num_gain_element_lists++;

    ind_channel_info->cc_target_is_cpe[c] = ixheaacd_read_bits_buf(it_bit_buf, 1);
    ind_channel_info->cc_target_tag_select[c] = ixheaacd_read_bits_buf(it_bit_buf, 4);
    if (ind_channel_info->cc_target_is_cpe[c]) {
      ind_channel_info->cc_l[c] = ixheaacd_read_bits_buf(it_bit_buf, 1);
      ind_channel_info->cc_r[c] = ixheaacd_read_bits_buf(it_bit_buf, 1);
      if (ind_channel_info->cc_l[c] && ind_channel_info->cc_r[c])
        num_gain_element_lists++;
      ind_channel_info->elements_coupled[c] = 1;
    } else
      ind_channel_info->elements_coupled[c] = 0;
  }
  if ((ind_sw_cce_flag == 0) && (num_gain_element_lists > MAX_BS_ELEMENT)) {
    return IA_FATAL_ERROR;
  }
  ixheaacd_read_bits_buf(it_bit_buf, 1);/*cc_domain*/
  ixheaacd_read_bits_buf(it_bit_buf, 1);/*gain_element_sign*/
  gain_element_scale = ixheaacd_read_bits_buf(it_bit_buf, 2);

  aac_handle->pstr_aac_dec_ch_info[0]->str_ics_info.num_swb_window = 0;
  aac_handle->pstr_aac_dec_ch_info[0]->str_ics_info.sampling_rate_index =
      samp_rate_idx;

  aac_handle->pstr_aac_dec_ch_info[0]->common_window = 0;

  error_status = ixheaacd_individual_ch_stream(
      it_bit_buf, aac_handle, 1, frame_size, total_channels, audio_object_type,
      eld_specific_config, ele_type);

  if (error_status) return error_status;

  ind_channel_info->cc_gain[0] = 1 << 29;
  for (c = 1; c < num_gain_element_lists; c++) {
    WORD cge;
    WORD common_gain_element_present[MAX_BS_ELEMENT];
    WORD16 norm_value;

    if (ind_sw_cce_flag)
      cge = 1;
    else {
      common_gain_element_present[c] = ixheaacd_read_bits_buf(it_bit_buf, 1);
      cge = common_gain_element_present[c];
      return IA_XHEAAC_DEC_EXE_FATAL_UNIMPLEMENTED_CCE;
    }
    if (cge) {
      UWORD8 *ptr_read_next = it_bit_buf->ptr_read_next;
      WORD32 bit_pos = 7 - it_bit_buf->bit_pos;
      WORD32 read_word =
          ixheaacd_aac_showbits_32(it_bit_buf->ptr_read_next, it_bit_buf->cnt_bits, NULL);
      UWORD32 read_word1;

      read_word1 = read_word << bit_pos;
      ixheaacd_huffman_decode(read_word1, &index, &length, hcod_sf, table_idx);

      bit_pos += length;

      ixheaacd_aac_read_byte(&ptr_read_next, &bit_pos, &read_word);
      while (bit_pos > 8)
        ixheaacd_aac_read_byte(&ptr_read_next, &bit_pos, &read_word);

      it_bit_buf->ptr_read_next = ptr_read_next;
      it_bit_buf->bit_pos = 7 - bit_pos;
      it_bit_buf->cnt_bits -= length;

      norm_value = index - 60;
      if (norm_value == -1)
        ind_channel_info->cc_gain[c] =
            common_tables_ptr->cc_gain_scale[gain_element_scale];
      else {
        int i;
        ind_channel_info->cc_gain[c] =
            common_tables_ptr->cc_gain_scale[gain_element_scale];
        for (i = 0; i < (-norm_value) - 1; i++) {
          ind_channel_info->cc_gain[c] = ixheaac_mul32_sh(
              ind_channel_info->cc_gain[c],
              common_tables_ptr->cc_gain_scale[gain_element_scale], 29);
        }
      }
    } else {
      return IA_XHEAAC_DEC_EXE_FATAL_UNIMPLEMENTED_CCE;
    }
  }
  if (it_bit_buf->cnt_bits < 0) {
    return IA_XHEAAC_DEC_EXE_NONFATAL_INSUFFICIENT_INPUT_BYTES;
  }
  return error_status;
}

void ixheaacd_dec_couple_channel(WORD32 *p_time_data, WORD32 *out_samp_cc,
                                 WORD16 frame_size, WORD total_channels,
                                 WORD16 gain_cc)

{
  WORD i;
  WORD32 out_cc;
  WORD32 *ptr_out_samp = &out_samp_cc[0];
  for (i = frame_size - 1; i >= 0; i--) {
    out_cc = (ixheaac_shl32_sat(
        ixheaac_mult32x16in32(*ptr_out_samp++, gain_cc), 3));
    *p_time_data = ixheaac_add32_sat(out_cc, *p_time_data);
    p_time_data += total_channels;
  }
}

IA_ERRORCODE ixheaacd_dec_ind_coupling(
    ia_exhaacplus_dec_api_struct *p_obj_exhaacplus_dec, WORD32 *coup_ch_output,
    WORD16 frame_size, WORD total_channels, VOID *ptr_time_data_tmp)

{
  WORD c, j, k;
  WORD l;
  WORD32 *out_samp_cc, err = 0;

  ia_enhaacplus_dec_ind_cc *ind_channel_info;

  WORD32 *ptr_time_data = (WORD32 *)ptr_time_data_tmp;

  {
    ind_channel_info = &p_obj_exhaacplus_dec->p_state_aac->ind_cc_info;

    out_samp_cc = coup_ch_output;

    if (ind_channel_info->num_coupled_elements > 0 && coup_ch_output == NULL) {
      if (p_obj_exhaacplus_dec->p_state_aac->ec_enable) {
        ind_channel_info->num_coupled_elements = 0;
        p_obj_exhaacplus_dec->aac_config.frame_status = 0;
      } else {
        return IA_FATAL_ERROR;
      }
    }

    j = 0;
    for (c = 0; c < ind_channel_info->num_coupled_elements + 1; c++) {
      for (l = 0; l < MAX_BS_ELEMENT; l++) {
        if (p_obj_exhaacplus_dec->aac_config.element_type[l] ==
                ind_channel_info->elements_coupled[c] &&
            p_obj_exhaacplus_dec->aac_config.element_instance_order[l] ==
                ind_channel_info->cc_target_tag_select[c]) {
          break;
        }
      }
      if (l == MAX_BS_ELEMENT) {
        continue;
      }

      k = p_obj_exhaacplus_dec->aac_config.slot_element[l];

      if (ind_channel_info->cc_target_is_cpe[c] == 0) {
        WORD32 *p_time_data = &ptr_time_data[k];

        WORD16 gain_cc = ixheaac_round16(ind_channel_info->cc_gain[j]);

        ixheaacd_dec_couple_channel(p_time_data, out_samp_cc, frame_size,
                                    total_channels, gain_cc);
      }
      if (ind_channel_info->cc_target_is_cpe[c] == 1) {
        if (ind_channel_info->cc_l[c] == 1) {
          WORD32 *p_time_data = &ptr_time_data[k];

          WORD16 gain_cc = ixheaac_round16(ind_channel_info->cc_gain[j]);

          ixheaacd_dec_couple_channel(p_time_data, out_samp_cc, frame_size,
                                      total_channels, gain_cc);
        }

        k = p_obj_exhaacplus_dec->aac_config.slot_element[l];

        if (ind_channel_info->cc_r[c] == 1) {
          WORD32 *p_time_data = &ptr_time_data[k + 1];
          WORD16 gain_cc = ixheaac_round16(ind_channel_info->cc_gain[j + 1]);

          ixheaacd_dec_couple_channel(p_time_data, out_samp_cc, frame_size,
                                      total_channels, gain_cc);
        }
      }
      if (ind_channel_info->cc_target_is_cpe[c] == 1) {
        j += 2;
      } else {
        j += 1;
      }
    }
  }
  return err;
}

void ixheaacd_dec_downmix_to_stereo(
    ia_exhaacplus_dec_api_struct *p_obj_exhaacplus_dec, WORD16 frame_size,
    WORD total_elements, WORD16 *ptr_time_data, WORD total_channels) {
  LOOPIDX i, j;
  WORD k = 0;
  if (5 == total_channels) k = 0;
  if (6 == total_channels) k = 1;
  if (7 == total_channels) k = 2;
  if (8 == total_channels) k = 3;

  for (j = 0; j < frame_size; j++) {
    WORD16 temp_l = 0, temp_r = 0;
    for (i = 0; i < total_elements; i++) {
      if (0 == p_obj_exhaacplus_dec->aac_config.element_type[i] ||
          3 == p_obj_exhaacplus_dec->aac_config.element_type[i]) {
        temp_l += (WORD16)(
            ixheaac_mult32x16in32(
                p_obj_exhaacplus_dec->common_tables->down_mix_martix
                    [k][0][p_obj_exhaacplus_dec->aac_config.slot_element[i]],
                ptr_time_data[j * total_channels +
                              p_obj_exhaacplus_dec->aac_config
                                  .slot_element[i]]) >>
            14);

        temp_r += (WORD16)(
            ixheaac_mult32x16in32(
                p_obj_exhaacplus_dec->common_tables->down_mix_martix
                    [k][1][p_obj_exhaacplus_dec->aac_config.slot_element[i]],
                ptr_time_data[j * total_channels +
                              p_obj_exhaacplus_dec->aac_config
                                  .slot_element[i]]) >>
            14);
      }
      if (1 == p_obj_exhaacplus_dec->aac_config.element_type[i]) {
        temp_l += (WORD16)(
            ixheaac_mult32x16in32(
                p_obj_exhaacplus_dec->common_tables->down_mix_martix
                    [k][0][p_obj_exhaacplus_dec->aac_config.slot_element[i]],
                ptr_time_data[j * total_channels +
                              p_obj_exhaacplus_dec->aac_config
                                  .slot_element[i]]) >>
            14);

        temp_r += (WORD16)(
            ixheaac_mult32x16in32(
                p_obj_exhaacplus_dec->common_tables->down_mix_martix
                    [k][1]
                    [p_obj_exhaacplus_dec->aac_config.slot_element[i] + 1],
                ptr_time_data[j * total_channels +
                              p_obj_exhaacplus_dec->aac_config.slot_element[i] +
                              1]) >>
            14);
      }
    }

    ptr_time_data[2 * j] = temp_l;
    ptr_time_data[2 * j + 1] = temp_r;
  }
}
