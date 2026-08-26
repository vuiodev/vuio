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
#include <stdio.h>
#include <string.h>
#include <math.h>
#include "ixheaac_constants.h"
#include "ixheaac_type_def.h"
#include "ixheaacd_memory_standards.h"
#include "ixheaacd_sbrdecsettings.h"
#include "ixheaacd_env_extr_part.h"
#include "ixheaacd_defines.h"
#include "ixheaacd_aac_rom.h"
#include "ixheaacd_common_rom.h"
#include "ixheaacd_sbr_rom.h"
#include "ixheaacd_bitbuffer.h"
#include "ixheaacd_pulsedata.h"
#include "ixheaacd_pns.h"

#include "ixheaacd_sbr_common.h"
#include "ixheaacd_drc_data_struct.h"
#include "ixheaacd_drc_dec.h"

#include "ixheaacd_lt_predict.h"
#include "ixheaacd_cnst.h"
#include "ixheaacd_ec_defines.h"
#include "ixheaacd_ec_struct_def.h"
#include "ixheaacd_channelinfo.h"
#include "ixheaacd_channel.h"
#include "ixheaacd_sbrdecoder.h"
#include "ixheaacd_sbr_scale.h"
#include "ixheaacd_audioobjtypes.h"
#include "ixheaacd_latmdemux.h"
#include "ixheaacd_aacdec.h"
#include "ixheaacd_sbr_common.h"

#include "ixheaacd_mps_polyphase.h"
#include "ixheaacd_config.h"
#include "ixheaacd_hybrid.h"
#include "ixheaacd_ps_dec.h"
#include "ixheaacd_qmf_dec.h"
#include "ixheaacd_mps_macro_def.h"
#include "ixheaacd_mps_struct_def.h"
#include "ixheaacd_mps_res_rom.h"
#include "ixheaacd_mps_aac_struct.h"
#include "ixheaacd_mps_dec.h"
#include "ixheaacd_mps_interface.h"
#include "ixheaacd_struct_def.h"
#include "ixheaacd_config.h"
#include "ixheaacd_bitbuffer.h"
#include "ixheaacd_interface.h"

#include "ixheaacd_tns_usac.h"
#include "ixheaacd_cnst.h"
#include "ixheaacd_acelp_info.h"
#include "ixheaacd_sbrdecsettings.h"
#include "ixheaacd_info.h"
#include "ixheaacd_struct.h"

#include "ixheaacd_sbrdecoder.h"
#include "ixheaacd_mps_polyphase.h"
#include "ixheaac_sbr_const.h"
#include "ixheaacd_main.h"
#include "ixheaacd_ec.h"
#include "ixheaacd_arith_dec.h"
#include "ixheaacd_bit_extract.h"
#include "ixheaacd_create.h"

#include "ixheaacd_func_def.h"
#include "ixheaacd_mps_interface.h"
#include "ixheaacd_mps_polyphase.h"

#include "ixheaacd_defines.h"

#include "ixheaacd_sbr_scale.h"
#include "ixheaacd_lpp_tran.h"
#include "ixheaacd_hybrid.h"
#include "ixheaacd_ps_dec.h"
#include "ixheaacd_env_extr.h"
#include "ixheaacd_env_calc.h"
#include "ixheaacd_qmf_dec.h"

#include "ixheaacd_pvc_dec.h"
#include "ixheaacd_sbr_dec.h"

#include "ixheaacd_error_codes.h"

#define MAXNRSBRELEMENTS 6

VOID ixheaacd_allocate_sbr_scr(ia_sbr_scr_struct *sbr_scratch_struct, VOID *base_scratch_ptr,
                               VOID *output_ptr, WORD32 total_channels, WORD8 *p_qshift_arr,
                               UWORD8 slot_element, WORD32 channel);

IA_ERRORCODE ixheaacd_esbr_process(ia_usac_data_struct *usac_data,
                                   ia_bit_buf_struct *it_bit_buff,
                                   WORD32 stereo_config_idx,
                                   WORD16 num_channels,
                                   WORD32 audio_object_type,
                                   WORD32 heaac_mps_present) {
  WORD32 ch;
  WORD32 err_code = 0;
  ia_aac_dec_sbr_bitstream_struct *esbr_bit_str = &usac_data->esbr_bit_str[0];
  ia_handle_sbr_dec_inst_struct self = usac_data->pstr_esbr_dec;

  ia_sbr_scr_struct sbr_scratch_struct;
  ixheaacd_allocate_sbr_scr(&sbr_scratch_struct, usac_data->sbr_scratch_mem_base, NULL, 0, NULL,
                            0, 0);

  self->usac_independency_flag = usac_data->usac_independency_flg;

  self->time_sample_buf[0] = usac_data->time_sample_vector[0];
  self->time_sample_buf[1] = usac_data->time_sample_vector[1];
  self->stereo_config_idx = stereo_config_idx;

  self->sbr_mode = usac_data->sbr_mode;
  self->aot_usac_flag = usac_data->usac_flag;
  self->esbr_hq = usac_data->esbr_hq;
  self->enh_sbr = usac_data->enh_sbr;
  self->enh_sbr_ps = usac_data->enh_sbr_ps;
  self->ec_flag = usac_data->ec_flag;
  self->first_frame = usac_data->first_frame;
  self->frame_ok = usac_data->frame_ok;
  self->sbr_parse_complete = usac_data->sbr_parse_complete;

  for (ch = 0; ch < num_channels; ch++) {
    if ((self->pstr_sbr_channel[ch]
             ->str_sbr_dec.str_synthesis_qmf_bank.qmf_filter_state_size) <
        QMF_FILTER_STATE_SYN_SIZE)
      return IA_FATAL_ERROR;
  }

  err_code = ixheaacd_applysbr(self, esbr_bit_str, NULL, &num_channels, 1, 0, 0,
                               &sbr_scratch_struct, 0, 1, 0, it_bit_buff, NULL,
                               0, audio_object_type, 0, 0, 1024,
                               heaac_mps_present, self->ec_flag, self->first_frame);

  usac_data->sbr_mode = self->sbr_mode;

  return err_code;
}

IA_ERRORCODE ixheaacd_esbr_parse(ia_usac_data_struct *usac_data, ia_bit_buf_struct *it_bit_buff,
                                 WORD32 stereo_config_idx, WORD16 num_channels,
                                 WORD32 audio_object_type)
{
  WORD32 err_code = 0;
  ia_aac_dec_sbr_bitstream_struct *esbr_bit_str = &usac_data->esbr_bit_str[0];
  ia_handle_sbr_dec_inst_struct self = usac_data->pstr_esbr_dec;

  ia_sbr_scr_struct sbr_scratch_struct;
  jmp_buf local;
  ixheaacd_allocate_sbr_scr(&sbr_scratch_struct, usac_data->sbr_scratch_mem_base, NULL, 0, NULL,
                            0, 0);

  self->usac_independency_flag = usac_data->usac_independency_flg;

  self->time_sample_buf[0] = usac_data->time_sample_vector[0];
  self->time_sample_buf[1] = usac_data->time_sample_vector[1];
  self->stereo_config_idx = stereo_config_idx;
  self->pstr_sbr_channel[0]->str_sbr_dec.time_sample_buf = self->time_sample_buf[0];
  self->pstr_sbr_channel[1]->str_sbr_dec.time_sample_buf = self->time_sample_buf[1];

  self->sbr_mode = usac_data->sbr_mode;
  self->aot_usac_flag = usac_data->usac_flag;
  self->esbr_hq = usac_data->esbr_hq;
  self->enh_sbr = usac_data->enh_sbr;
  self->enh_sbr_ps = usac_data->enh_sbr_ps;
  self->ec_flag = usac_data->ec_flag;
  self->first_frame = usac_data->first_frame;
  self->frame_ok = usac_data->frame_ok;
  self->sbr_parse_err_flag = usac_data->sbr_parse_err_flag;

  if (usac_data->ec_flag) {
    err_code = setjmp(local);
  }
  if (err_code == 0) {
    if (usac_data->ec_flag) {
      it_bit_buff->xaac_jmp_buf = &local;
    }
    err_code = ixheaacd_parse_sbr(self, esbr_bit_str, &num_channels, self->frame_ok,
                                  &sbr_scratch_struct, it_bit_buff, audio_object_type);
  } else {
    usac_data->frame_ok = 0;
    self->frame_ok = usac_data->frame_ok;
  }

  usac_data->sbr_mode = self->sbr_mode;
  usac_data->sbr_parse_err_flag = self->sbr_parse_err_flag;
  usac_data->sbr_parse_complete = self->sbr_parse_complete;

  return err_code;
}

static VOID ixheaacd_sbr_ext_data_read(ia_bit_buf_struct *it_bit_buff,
    ia_aac_dec_sbr_bitstream_struct *esbr_bit_str) {
  WORD32 count = 0;
  WORD32 read_bits = 0;
  WORD32 unaligned_bits = 0;
  WORD32 cnt_bits_in;

  cnt_bits_in = it_bit_buff->cnt_bits;
  count = (it_bit_buff->cnt_bits) >> 3;
  if (count > 0) {
    if ((esbr_bit_str->no_elements < MAXNRSBRELEMENTS)) {
      esbr_bit_str->str_sbr_ele[esbr_bit_str->no_elements].extension_type =
          EXT_SBR_DATA;
      esbr_bit_str->str_sbr_ele[esbr_bit_str->no_elements].size_payload = count;

      read_bits = count << 3;

      unaligned_bits = (cnt_bits_in - read_bits);
      if (unaligned_bits > 0 && unaligned_bits < 8) {
        count++;
        esbr_bit_str->str_sbr_ele[esbr_bit_str->no_elements].size_payload =
            count;
      }
      esbr_bit_str->no_elements += 1;
    }
  }
}

static WORD32 ixheaacd_read_ext_element(UWORD32 usac_ext_element_default_length,
                                        UWORD32 usac_ext_element_payload_frag,
                                        ia_bit_buf_struct *it_bit_buff
                                       ,
                                        ia_usac_decoder_config_struct *pstr_usac_dec_config,
                                        WORD32 elem_idx,
                                        WORD32 *frame_ok) {
  UWORD32 usac_ext_element_present;
  UWORD32 usac_ext_element_use_dft_length;
  UWORD32 pay_load_length;
  WORD32 i;
  usac_ext_element_present = ixheaacd_read_bits_buf(it_bit_buff, 1);

  if (usac_ext_element_present) {
    usac_ext_element_use_dft_length = ixheaacd_read_bits_buf(it_bit_buff, 1);

    if (usac_ext_element_use_dft_length) {
      pay_load_length = usac_ext_element_default_length;
    } else {
      pay_load_length = ixheaacd_read_bits_buf(it_bit_buff, 8);

      if (pay_load_length == 255) {
        WORD32 val_add = 0;
        val_add = ixheaacd_read_bits_buf(it_bit_buff, 16);
        pay_load_length = (UWORD32)((WORD32)pay_load_length + val_add - 2);
      }
    }
    if ((it_bit_buff->cnt_bits >> 3) < (WORD32)pay_load_length) {
      *frame_ok = 0;
      longjmp(*(it_bit_buff->xaac_jmp_buf), IA_XHEAAC_DEC_EXE_NONFATAL_INSUFFICIENT_INPUT_BYTES);
    }
    if (pay_load_length > 0) {
      if (usac_ext_element_payload_frag)
        ixheaacd_read_bits_buf(it_bit_buff, 2);

      if (pstr_usac_dec_config->usac_ext_ele_payload_present[elem_idx]) {
        WORD32 preroll_counter = pstr_usac_dec_config->preroll_counter;
        int payload_buffer_offeset = 0;
        for (i = 0; i < preroll_counter; i++)
          payload_buffer_offeset +=
              pstr_usac_dec_config->usac_ext_gain_payload_len[i] *
              sizeof(WORD8);
        if ((pay_load_length + payload_buffer_offeset) >
            (MAX_AUDIO_PREROLLS * 768))
          return IA_FATAL_ERROR;
        for (i = 0; i < ((WORD32)pay_load_length); i++) {
          pstr_usac_dec_config
              ->usac_ext_gain_payload_buf[i + payload_buffer_offeset] =
              ixheaacd_read_bits_buf(it_bit_buff, 8);
        }
        pstr_usac_dec_config->usac_ext_gain_payload_len[preroll_counter] +=
            pay_load_length;
      } else {
        if (it_bit_buff->cnt_bits < (WORD32)(pay_load_length << 3)) {
          longjmp(*(it_bit_buff->xaac_jmp_buf),
                  IA_XHEAAC_DEC_EXE_NONFATAL_INSUFFICIENT_INPUT_BYTES);
        }
        it_bit_buff->ptr_read_next =
            it_bit_buff->ptr_read_next + pay_load_length;
        it_bit_buff->cnt_bits =
            it_bit_buff->cnt_bits - (WORD32)(pay_load_length << 3);
      }
    }
  }
  return 0;
}

static VOID ixheaacd_sbr_ele_type_set(
    ia_aac_dec_sbr_bitstream_struct *esbr_bit_str0,
    ia_aac_dec_sbr_bitstream_struct *esbr_bit_str1, WORD32 ele_id,
    WORD32 st_config_idx) {
  if (ele_id == ID_USAC_SCE) {
    esbr_bit_str0->str_sbr_ele[esbr_bit_str0->no_elements].sbr_ele_id =
        SBR_ID_SCE;
    esbr_bit_str1->str_sbr_ele[esbr_bit_str1->no_elements].sbr_ele_id =
        SBR_ID_SCE;
  }
  if (ele_id == ID_USAC_CPE) {
    if ((st_config_idx == 0) || (st_config_idx == 3)) {
      esbr_bit_str0->str_sbr_ele[esbr_bit_str0->no_elements].sbr_ele_id =
          SBR_ID_CPE;
      esbr_bit_str1->str_sbr_ele[esbr_bit_str1->no_elements].sbr_ele_id =
          SBR_ID_CPE;
    } else {
      esbr_bit_str0->str_sbr_ele[esbr_bit_str0->no_elements].sbr_ele_id =
          SBR_ID_SCE;
      esbr_bit_str1->str_sbr_ele[esbr_bit_str1->no_elements].sbr_ele_id =
          SBR_ID_SCE;
    }
  }
  if (ele_id == ID_USAC_LFE) {
    esbr_bit_str0->str_sbr_ele[esbr_bit_str0->no_elements].sbr_ele_id =
        SBR_ID_SCE;
    esbr_bit_str1->str_sbr_ele[esbr_bit_str1->no_elements].sbr_ele_id =
        SBR_ID_SCE;

    esbr_bit_str0->no_elements++;
    esbr_bit_str0->str_sbr_ele[0].size_payload = 0;
  }
}

VOID ixheaacd_ms_processing(ia_usac_data_struct *pstr_usac_data) {
  WORD32 i;

  FLOAT32 tmp, tmp1;
  FLOAT32 ms_factor = (FLOAT32)0.7071067812;
  for (i = 0; i < pstr_usac_data->ccfl; i++) {
    tmp = (FLOAT32)((pstr_usac_data->time_sample_vector[0][i] +
                     pstr_usac_data->time_sample_vector[1][i]) *
                    ms_factor);
    tmp1 = (FLOAT32)((pstr_usac_data->time_sample_vector[0][i] -
                      pstr_usac_data->time_sample_vector[1][i]) *
                     ms_factor);
    pstr_usac_data->time_sample_vector[1][i] = tmp1;
    pstr_usac_data->time_sample_vector[0][i] = tmp;
  }
}

WORD32 ixheaacd_usac_process(ia_dec_data_struct *pstr_dec_data,
                             WORD32 *num_out_channels, VOID *codec_handle) {
  WORD32 ele_id = 0;
  WORD32 err_code = 0;

  ia_aac_dec_state_struct *p_state_aac_dec =
      (ia_aac_dec_state_struct *)codec_handle;

  ia_usac_data_struct *pstr_usac_data = &(pstr_dec_data->str_usac_data);
  ia_bit_buf_struct *it_bit_buff = &pstr_dec_data->dec_bit_buf;

  ia_frame_data_struct *fd = &(pstr_dec_data->str_frame_data);

  ia_usac_config_struct *pstr_usac_config =
      &(fd->str_audio_specific_config.str_usac_config);
  ia_usac_decoder_config_struct *pstr_usac_dec_config =
      &(fd->str_audio_specific_config.str_usac_config.str_usac_dec_config);

  WORD32 err = 0;
  WORD16 nr_core_coder_channels = 0;
  WORD32 ch_offset = 0;

  WORD32 elem_idx = 0;
  WORD32 num_ch_out = 0;
  WORD32 num_elements = pstr_usac_dec_config->num_elements;
  UWORD32 ec_flag = p_state_aac_dec->p_config->ui_err_conceal;

  if (ec_flag) {
    if (it_bit_buff->cnt_bits < 16) {
      pstr_dec_data->str_usac_data.frame_ok = 0;
    }
    if (pstr_dec_data->str_usac_data.frame_ok == 1) {
      pstr_usac_data->usac_independency_flg = ixheaacd_read_bits_buf(it_bit_buff, 1);
    }
  } else {
    pstr_usac_data->usac_independency_flg = ixheaacd_read_bits_buf(it_bit_buff, 1);
  }

  if (pstr_usac_data->ec_flag && pstr_dec_data->str_usac_data.frame_ok == 1) {
    if (!(pstr_usac_data->last_frame_ok) && !(pstr_usac_data->usac_independency_flg)) {
      pstr_dec_data->str_usac_data.frame_ok = 0;
    } else {
      pstr_dec_data->str_usac_data.frame_ok = 1;
    }
  }

  for (elem_idx = 0; elem_idx < num_elements; elem_idx++) {
    WORD32 stereo_config_index =
        pstr_usac_config->str_usac_dec_config.str_usac_element_config[elem_idx]
            .stereo_config_index;
    jmp_buf local;
    if (ec_flag) {
      err = setjmp(local);
    }

    pstr_usac_data->esbr_bit_str[0].no_elements = 0;
    pstr_usac_data->esbr_bit_str[1].no_elements = 0;

    if (!err) {
      if (ec_flag) {
        it_bit_buff->xaac_jmp_buf = &local;
      }
      switch (ele_id = pstr_usac_dec_config->usac_element_type[elem_idx]) {
        case ID_USAC_SCE:
          nr_core_coder_channels = 1;
          num_ch_out += 1;
          goto core_data_extracting;

        case ID_USAC_CPE:
          nr_core_coder_channels = (stereo_config_index == 1) ? 1 : 2;
          if (((stereo_config_index > 1) || (stereo_config_index == 0)) &&
              (p_state_aac_dec->num_of_output_ch < 2))
            return -1;
          num_ch_out += 2;
          goto core_data_extracting;
        case ID_USAC_LFE:
          nr_core_coder_channels = 1;
          num_ch_out += 1;

        core_data_extracting:
          if (!ec_flag) {
            if (ch_offset >= MAX_NUM_CHANNELS_USAC_LVL2) return -1;
            if (num_ch_out > MAX_NUM_CHANNELS_USAC_LVL2) return -1;
          } else {
            if ((ch_offset >= MAX_NUM_CHANNELS_USAC_LVL2) ||
                (num_ch_out > MAX_NUM_CHANNELS_USAC_LVL2)) {
              pstr_usac_data->frame_ok = 0;
            }
          }

          err = ixheaacd_core_coder_data(ele_id, pstr_usac_data, elem_idx, ch_offset, it_bit_buff,
                                         nr_core_coder_channels);
          if (!ec_flag) {
            if (err != 0) return -1;
          } else {
            if (err != 0) pstr_usac_data->frame_ok = 0;
          }

          ixheaacd_sbr_ele_type_set(&pstr_usac_data->esbr_bit_str[0],
                                    &pstr_usac_data->esbr_bit_str[1], ele_id,
                                    stereo_config_index);

          if (pstr_usac_data->mps_pseudo_lr[elem_idx]) ixheaacd_ms_processing(pstr_usac_data);

          if (ele_id != ID_USAC_LFE) {
            if (pstr_usac_data->sbr_ratio_idx > 0)
              ixheaacd_sbr_ext_data_read(it_bit_buff, &pstr_usac_data->esbr_bit_str[0]);
          }

          if ((pstr_usac_data->sbr_ratio_idx > 0) &&
              (pstr_usac_data->esbr_bit_str[0].no_elements != 0)) {
            if (ec_flag) {
              pstr_usac_data->first_frame = p_state_aac_dec->first_frame;
            }
            err_code = ixheaacd_esbr_process(
                pstr_usac_data, it_bit_buff, stereo_config_index, nr_core_coder_channels,
                pstr_dec_data->str_frame_data.str_audio_specific_config.audio_object_type,
                p_state_aac_dec->heaac_mps_handle.heaac_mps_present);
            if (!ec_flag) {
              if (err_code < 0) return err_code;
            } else {
              if (err_code < 0) pstr_usac_data->frame_ok = 0;
            }
          }

          if (stereo_config_index > 0 && pstr_usac_data->sbr_ratio_idx > 0) {
            FLOAT32 **ptr_inp[2 * 2];
            WORD32 ch;

            *num_out_channels = p_state_aac_dec->mps_dec_handle.out_ch_count;

            if (!ec_flag && pstr_usac_data->frame_ok == 1) {
              if (ec_flag) {
                it_bit_buff->xaac_jmp_buf = &local;
              }
              err =
                  ixheaacd_mps_frame_parsing(&p_state_aac_dec->mps_dec_handle,
                                             pstr_usac_data->usac_independency_flg, it_bit_buff);
              if (err) return err;
            }

            p_state_aac_dec->mps_dec_handle.band_count[0] =
                pstr_usac_data->pstr_esbr_dec->pstr_sbr_channel[0]->str_sbr_dec.band_count;
            p_state_aac_dec->mps_dec_handle.band_count[1] =
                pstr_usac_data->pstr_esbr_dec->pstr_sbr_channel[1]->str_sbr_dec.band_count;

            for (ch = 0; ch < nr_core_coder_channels; ch++) {
              ptr_inp[2 * ch] = pstr_usac_data->pstr_esbr_dec->pstr_sbr_channel[ch]
                                    ->str_sbr_dec.pp_qmf_buf_real;
              ptr_inp[2 * ch + 1] = pstr_usac_data->pstr_esbr_dec->pstr_sbr_channel[ch]
                                        ->str_sbr_dec.pp_qmf_buf_imag;
              p_state_aac_dec->mps_dec_handle.p_sbr_dec[ch] =
                  (VOID *)(&pstr_usac_data->pstr_esbr_dec->pstr_sbr_channel[ch]->str_sbr_dec);
              p_state_aac_dec->mps_dec_handle.p_sbr_header[ch] =
                  (VOID *)(pstr_usac_data->pstr_esbr_dec->pstr_sbr_header[ch]);
              p_state_aac_dec->mps_dec_handle.p_sbr_frame[ch] =
                  (VOID *)(pstr_usac_data->pstr_esbr_dec->frame_buffer[ch]);
            }
            if (nr_core_coder_channels == 1) {
              if (p_state_aac_dec->mps_dec_handle.res_ch_count != 0) {
                ptr_inp[2] = pstr_usac_data->pstr_esbr_dec->pstr_sbr_channel[1]
                                 ->str_sbr_dec.pp_qmf_buf_real;
                ptr_inp[2 + 1] = pstr_usac_data->pstr_esbr_dec->pstr_sbr_channel[1]
                                     ->str_sbr_dec.pp_qmf_buf_imag;
              }
              p_state_aac_dec->mps_dec_handle.p_sbr_dec[1] =
                  (VOID *)(&pstr_usac_data->pstr_esbr_dec->pstr_sbr_channel[1]->str_sbr_dec);
              p_state_aac_dec->mps_dec_handle.p_sbr_header[1] =
                  (VOID *)(pstr_usac_data->pstr_esbr_dec->pstr_sbr_header[1]);
              p_state_aac_dec->mps_dec_handle.p_sbr_frame[1] =
                  (VOID *)(pstr_usac_data->pstr_esbr_dec->frame_buffer[1]);
            }
            if ((!ec_flag || !p_state_aac_dec->first_frame) &&
                p_state_aac_dec->mps_dec_handle.mps_init_done == 1) {
              err = ixheaacd_mps_apply(&p_state_aac_dec->mps_dec_handle, ptr_inp,
                                       pstr_usac_data->time_sample_vector);
              if (!ec_flag) {
                if (err) return err;
              } else {
                if (err) pstr_usac_data->frame_ok = 0;
              }
            }
            if (ec_flag) {
              pstr_usac_data->first_frame = p_state_aac_dec->first_frame;
              err_code = ixheaacd_esbr_parse(
                  pstr_usac_data, it_bit_buff, stereo_config_index, nr_core_coder_channels,
                  pstr_dec_data->str_frame_data.str_audio_specific_config.audio_object_type);

              if (err_code < 0) pstr_usac_data->frame_ok = 0;
              if (ec_flag) {
                err = setjmp(local);
              }
              if (err == 0 && pstr_usac_data->frame_ok == 1) {
                if (ec_flag) {
                  it_bit_buff->xaac_jmp_buf = &local;
                }
                err = ixheaacd_mps_frame_parsing(&p_state_aac_dec->mps_dec_handle,
                                                 pstr_usac_data->usac_independency_flg,
                                                 it_bit_buff);
                if (err) pstr_usac_data->frame_ok = 0;
              } else {
                pstr_usac_data->frame_ok = 0;
                if (p_state_aac_dec->mps_dec_handle.num_parameter_sets_prev < 1) {
                  p_state_aac_dec->mps_dec_handle.num_parameter_sets_prev = 1;
                }
              }
            }
          } else if ((pstr_usac_data->sbr_ratio_idx > 0) &&
                     (pstr_usac_data->esbr_bit_str[0].no_elements != 0) && ec_flag) {
            pstr_usac_data->first_frame = p_state_aac_dec->first_frame;
            err_code = ixheaacd_esbr_parse(
                pstr_usac_data, it_bit_buff, stereo_config_index, nr_core_coder_channels,
                pstr_dec_data->str_frame_data.str_audio_specific_config.audio_object_type);
            if (err_code < 0) pstr_usac_data->frame_ok = 0;
          }
          if (ec_flag) {
            p_state_aac_dec->first_frame = 0;
          }

          ch_offset += nr_core_coder_channels;
          break;

        case ID_USAC_EXT: {
          ia_usac_dec_element_config_struct *pusac_element_config =
              &pstr_usac_dec_config->str_usac_element_config[elem_idx];
          if (pstr_usac_data->frame_ok) {
            err = ixheaacd_read_ext_element(pusac_element_config->usac_ext_eleme_def_len,
                                            pusac_element_config->usac_ext_elem_pld_frag,
                                            it_bit_buff, pstr_usac_dec_config, elem_idx,
                                            &pstr_usac_data->frame_ok);
            if (!ec_flag) {
              if (err != 0) return err;
            } else {
              if (err) pstr_usac_data->frame_ok = 0;
            }
          }
          break;
        }

        default:

          return -1;

          break;
      }
    }
  }
  pstr_usac_data->last_frame_ok = pstr_usac_data->frame_ok;

  return 0;
}
