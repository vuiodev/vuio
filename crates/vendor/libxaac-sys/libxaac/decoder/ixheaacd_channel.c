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
#include "ixheaac_basic_ops.h"

#include "ixheaacd_bitbuffer.h"
#include "ixheaacd_error_codes.h"
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

#include "ixheaacd_block.h"
#include "ixheaacd_channel.h"

#include "ixheaacd_common_rom.h"
#include "ixheaacd_basic_funcs.h"
#include "ixheaac_basic_op.h"
#include "ixheaacd_intrinsics.h"
#include "ixheaacd_stereo.h"

#include "ixheaacd_tns.h"

#include "ixheaacd_audioobjtypes.h"
#include "ixheaacd_latmdemux.h"

#include "ixheaacd_aacdec.h"
#include "ixheaacd_definitions.h"

#include "ixheaacd_error_codes.h"

#include "ixheaacd_pulsedata.h"

#include "ixheaacd_sbrdecsettings.h"
#include "ixheaacd_sbr_scale.h"
#include "ixheaacd_env_extr_part.h"
#include "ixheaacd_sbr_rom.h"
#include "ixheaacd_audioobjtypes.h"
#include "ixheaacd_sbrdecoder.h"
#include "ixheaacd_memory_standards.h"
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
#include "ixheaacd_adts_crc_check.h"
#include "ixheaacd_rvlc.h"
#include "ixheaacd_hcr.h"
#include "ixheaacd_function_selector.h"

#define SPEC(ptr, w, gl) ((ptr) + ((w) * (gl)))

UWORD32 ixheaacd_aac_showbits_32(UWORD8 *ptr_read_next, WORD32 cnt_bits,
                                 WORD32 *increment) {
  UWORD8 *v = ptr_read_next;
  UWORD32 b = 0;
  WORD32 i;
  WORD32 bumped = 0;

  for (i = 0; i < 4; i++) {
    b = b << 8;
    if (cnt_bits > 0) {
      b = b | *v;
      v++;
      bumped++;
    }
    cnt_bits -= 8;
  }
  if (increment != NULL) {
    *increment = bumped;
  }
  return b;
}

WORD16 *ixheaacd_getscalefactorbandoffsets(
    ia_ics_info_struct *ptr_ics_info,
    ia_aac_dec_tables_struct *ptr_aac_tables) {
  if (ptr_ics_info->window_sequence != EIGHT_SHORT_SEQUENCE) {
    return ptr_aac_tables->sfb_long_table;
  } else {
    return ptr_aac_tables->sfb_short_table;
  }
}

WORD8 *ixheaacd_getscalefactorbandwidth(
    ia_ics_info_struct *ptr_ics_info,
    ia_aac_dec_tables_struct *ptr_aac_tables) {
  if (ptr_ics_info->frame_length == 512) {
    return (
        WORD8 *)(&ptr_aac_tables
                      ->scale_fac_bands_512[ptr_ics_info->sampling_rate_index]
                                           [0]);
  } else if (ptr_ics_info->frame_length == 1024) {
    return (
        WORD8 *)(&ptr_aac_tables
                      ->scale_factor_bands_long[ptr_ics_info->sampling_rate_index]
                                               [0]);
  } else {
    return (
        WORD8 *)(&ptr_aac_tables
                      ->scale_fac_bands_480[ptr_ics_info->sampling_rate_index]
                                           [0]);
  }
}

WORD32 ixheaacd_cblock_inv_quant_spect_data(
    ia_aac_dec_channel_info_struct *ptr_aac_dec_channel_info,
    ia_aac_dec_tables_struct *ptr_aac_tables);

void ixheaacd_cblock_scale_spect_data(
    ia_aac_dec_channel_info_struct *ptr_aac_dec_channel_info,
    ia_aac_dec_tables_struct *ptr_aac_tables, WORD32 total_channels,
    WORD32 object_type, WORD32 aac_sf_data_resil_flag) {
  WORD32 *ptr_spect_coeff = ptr_aac_dec_channel_info->ptr_spec_coeff;
  WORD8 *ptr_sfb_width = (WORD8 *)(ixheaacd_getscalefactorbandwidth(
      &(ptr_aac_dec_channel_info->str_ics_info), ptr_aac_tables));
  WORD16 *ptr_scale_fac = ptr_aac_dec_channel_info->ptr_scale_factor;
  WORD tot_bands = ptr_aac_dec_channel_info->str_ics_info.max_sfb;

  WORD num_win_grp, group_len;
  WORD32 *ptr_scale_table = ptr_aac_tables->pstr_block_tables->scale_table;
  ia_ics_info_struct *ptr_ics_info = &ptr_aac_dec_channel_info->str_ics_info;

  if (object_type == AOT_ER_AAC_LC)
    ptr_sfb_width = ptr_aac_tables->str_aac_sfb_info[ptr_ics_info->window_sequence]
      .sfb_width;
  for (num_win_grp = 0; num_win_grp < ptr_ics_info->num_window_groups;
      num_win_grp++) {
    for (group_len = 0;
        group_len < ptr_ics_info->window_group_length[num_win_grp];
        group_len++) {
      (*ixheaacd_scale_factor_process)(
          &ptr_spect_coeff[0], &ptr_scale_fac[0], tot_bands,
          (WORD8 *)ptr_sfb_width, ptr_scale_table, total_channels,
          object_type, aac_sf_data_resil_flag);

      ptr_spect_coeff += MAX_BINS_SHORT;
    }
    ptr_scale_fac += MAX_SCALE_FACTOR_BANDS_SHORT;
  }
}

WORD32 ixheaacd_read_pulse_data(ia_bit_buf_struct *it_bit_buff,
                                ia_pulse_info_struct *ptr_pulse_info,
                                ia_aac_dec_tables_struct *ptr_aac_tables) {
  WORD32 i, total_offset;
  WORD32 error_code = 0;

  WORD32 value = ixheaacd_read_bits_buf(it_bit_buff, 8);
  ptr_pulse_info->number_pulse = value >> 6;
  ptr_pulse_info->pulse_start_band = value & 0x3F;

  if (ptr_pulse_info->pulse_start_band >= 52) {
    return (WORD32)IA_XHEAAC_DEC_EXE_NONFATAL_PULSEDATA_ERROR;
  }

  total_offset = ptr_aac_tables->str_aac_sfb_info[0]
                     .sfb_index[ptr_pulse_info->pulse_start_band];

  for (i = 0; i < ptr_pulse_info->number_pulse + 1; i++) {
    WORD32 value = ixheaacd_read_bits_buf(it_bit_buff, 9);
    ptr_pulse_info->pulse_offset[i] = value >> 4;
    ptr_pulse_info->pulse_amp[i] = value & 0xF;
    total_offset += ptr_pulse_info->pulse_offset[i];

    if (total_offset >= 1024) {
      error_code = (WORD32)IA_XHEAAC_DEC_EXE_NONFATAL_PULSEDATA_ERROR;
    }
  }

  return error_code;
}

static IA_ERRORCODE ixheaacd_read_block_data(
    ia_bit_buf_struct *it_bit_buff,
    ia_aac_dec_channel_info_struct *ptr_aac_dec_channel_info,
    ia_aac_dec_tables_struct *ptr_aac_tables, WORD32 total_channels,
    WORD32 frame_size, WORD32 object_type, WORD32 aac_spect_data_resil_flag,
    WORD32 aac_sect_data_resil_flag, WORD32 aac_sf_data_resil_flag,
    WORD32 ele_type, ia_aac_dec_overlap_info *ptr_aac_dec_static_channel_info)

{
  FLAG gain_control_data_present;
  WORD16 error_code = AAC_DEC_OK;

  if (ptr_aac_dec_channel_info->str_ics_info.window_sequence ==
      EIGHT_SHORT_SEQUENCE) {
    memset(ptr_aac_dec_channel_info->ptr_scale_factor, 0,
           MAX_WINDOWS * MAX_SCALE_FACTOR_BANDS_SHORT * 3);
  }

  error_code = ixheaacd_read_section_data(
      it_bit_buff, ptr_aac_dec_channel_info, aac_spect_data_resil_flag,
      aac_sect_data_resil_flag, ptr_aac_tables);

  if (error_code) {
    return error_code;
  }
  if (aac_sf_data_resil_flag &&
      ((object_type == AOT_ER_AAC_ELD) || (object_type == AOT_ER_AAC_LD)))
    ixheaacd_rvlc_read(it_bit_buff, ptr_aac_dec_channel_info);
  else
    ixheaacd_read_scale_factor_data(it_bit_buff, ptr_aac_dec_channel_info,
                                    ptr_aac_tables, object_type);

  error_code = 0;
  if (object_type != AOT_ER_AAC_ELD) {
    ptr_aac_dec_channel_info->str_pulse_info.pulse_data_present =
        ixheaacd_read_bits_buf(it_bit_buff, 1);
    if (ptr_aac_dec_channel_info->str_pulse_info.pulse_data_present) {
      error_code = ixheaacd_read_pulse_data(
          it_bit_buff, &ptr_aac_dec_channel_info->str_pulse_info,
          ptr_aac_tables);
    }

    if (error_code) {
      return error_code;
    }
  }

  ptr_aac_dec_channel_info->str_tns_info.tns_data_present =
      (FLAG)ixheaacd_read_bits_buf(it_bit_buff, 1);

  if (object_type < ER_OBJECT_START) {
    error_code = 0;
    if (ptr_aac_dec_channel_info->str_tns_info.tns_data_present) {
      error_code =
          ixheaacd_read_tns_data(it_bit_buff, ptr_aac_dec_channel_info);
    }

    if (error_code) {
      return error_code;
    }
  }

  if (object_type != AOT_ER_AAC_ELD) {
    gain_control_data_present = ixheaacd_read_bits_buf(it_bit_buff, 1);

    if (gain_control_data_present) {
      return (WORD16)(
          (WORD32)IA_XHEAAC_DEC_EXE_NONFATAL_GAIN_CONTROL_DATA_PRESENT);
    }
  }

  if (object_type == AOT_ER_AAC_ELD) {
    if (ptr_aac_dec_channel_info->str_tns_info.tns_data_present)
      error_code =
          ixheaacd_read_tns_data(it_bit_buff, ptr_aac_dec_channel_info);
    if (error_code) return error_code;
  }

  if (aac_spect_data_resil_flag &&
      ((object_type == AOT_ER_AAC_ELD) || (object_type == AOT_ER_AAC_LD)
      || (object_type == AOT_ER_AAC_LC)))
    ixheaacd_hcr_read(it_bit_buff, ptr_aac_dec_channel_info, ele_type);

  if (aac_sf_data_resil_flag &&
      ((object_type == AOT_ER_AAC_ELD) || (object_type == AOT_ER_AAC_LD))) {
    error_code = ixheaacd_rvlc_dec(
        ptr_aac_dec_channel_info, ptr_aac_dec_static_channel_info, it_bit_buff);
    if (error_code) return error_code;

    it_bit_buff->bit_pos = 7 - it_bit_buff->bit_pos;
  }

  if (object_type == AOT_ER_AAC_LD || object_type == AOT_ER_AAC_LC) {
    if (ptr_aac_dec_channel_info->str_tns_info.tns_data_present)
      error_code =
          ixheaacd_read_tns_data(it_bit_buff, ptr_aac_dec_channel_info);
    if (error_code) return error_code;
  }

  { it_bit_buff->bit_pos = 7 - it_bit_buff->bit_pos; }

  error_code = ixheaacd_read_spectral_data(
      it_bit_buff, ptr_aac_dec_channel_info, ptr_aac_tables, total_channels,
      frame_size, object_type, aac_spect_data_resil_flag,
      aac_sf_data_resil_flag);

  it_bit_buff->bit_pos = (7 - it_bit_buff->bit_pos);

  return error_code;
}

IA_ERRORCODE ixheaacd_ltp_decode(ia_bit_buf_struct *it_bit_buff,
                                 ia_ics_info_struct *ptr_ics_info,
                                 WORD32 object_type, WORD32 frame_size,
                                 WORD32 ch) {
  IA_ERRORCODE retval = AAC_DEC_OK;

  if (ptr_ics_info->predictor_data_present) {
    if (ch == 0) {
      ixheaacd_init_ltp_object(&(ptr_ics_info->ltp));
      ptr_ics_info->ltp.data_present = ixheaacd_read_bits_buf(it_bit_buff, 1);
      if (ptr_ics_info->ltp.data_present) {
        if ((retval = ixheaacd_ltp_data(object_type, ptr_ics_info,
                                        &(ptr_ics_info->ltp), it_bit_buff,
                                        frame_size)) > 0) {
          return retval;
        }
      }
    } else {
      ixheaacd_init_ltp_object(&(ptr_ics_info->ltp2));
      ptr_ics_info->ltp2.data_present = ixheaacd_read_bits_buf(it_bit_buff, 1);
      if (ptr_ics_info->ltp2.data_present) {
        if ((retval = ixheaacd_ltp_data(object_type, ptr_ics_info,
                                        &(ptr_ics_info->ltp2), it_bit_buff,
                                        frame_size)) > 0) {
          return retval;
        }
      }
    }
  }
  return retval;
}
WORD16 ixheaacd_ics_read(ia_bit_buf_struct *it_bit_buff,
                         ia_ics_info_struct *ptr_ics_info,
                         WORD8 num_swb_window[2], WORD32 object_type,
                         WORD32 common_window, WORD32 frame_size) {
  WORD i;
  WORD mask;
  WORD value = 0;

  if (object_type == AOT_ER_AAC_ELD) {
    ptr_ics_info->window_sequence = 0;
    ptr_ics_info->window_shape = 1;
  } else {
    if (object_type != AOT_ER_AAC_LD)
    {
      if (frame_size == 960)
        ptr_ics_info->frame_length = 960;
      else
        ptr_ics_info->frame_length = 1024;
    }
    value = ixheaacd_read_bits_buf(it_bit_buff, 4);
    ptr_ics_info->window_sequence = (WORD16)((value & 0x6) >> 1);
    ptr_ics_info->window_shape = (WORD16)((value & 0x1));
  }

  if (ptr_ics_info->window_sequence != EIGHT_SHORT_SEQUENCE) {
    ptr_ics_info->num_swb_window = num_swb_window[0];

    ptr_ics_info->num_window_groups = 1;
    ptr_ics_info->window_group_length[0] = 1;

    if (object_type == AOT_ER_AAC_ELD) {
      ptr_ics_info->max_sfb = ixheaacd_read_bits_buf(it_bit_buff, 6);
      if (ptr_ics_info->max_sfb == 0) ptr_ics_info->num_swb_window = 0;
    } else {
      value = ixheaacd_read_bits_buf(it_bit_buff, 7);
      ptr_ics_info->max_sfb = (value & 0x7E) >> 1;
    }

    if ((object_type != AOT_ER_AAC_LD) && (object_type != AOT_AAC_LTP)) {
      if (value & 1) {
        return (WORD16)(
            (WORD32)IA_XHEAAC_DEC_EXE_NONFATAL_PREDICTION_DATA_PRESENT);
      }

    } else {
      ptr_ics_info->predictor_data_present = value & 1;

      if (ptr_ics_info->predictor_data_present) {
        WORD32 retval = AAC_DEC_OK;

        ixheaacd_init_ltp_object(&(ptr_ics_info->ltp));
        if (object_type < ER_OBJECT_START) {
          if ((ptr_ics_info->ltp.data_present =
                   ixheaacd_read_bits_buf(it_bit_buff, 1)) &
              1) {
            if ((retval = ixheaacd_ltp_data(object_type, ptr_ics_info,
                                            &(ptr_ics_info->ltp), it_bit_buff,
                                            frame_size)) > 0) {
              return retval;
            }
          }
          if (common_window) {
            ixheaacd_init_ltp_object(&(ptr_ics_info->ltp2));
            if ((ptr_ics_info->ltp2.data_present =
                     ixheaacd_read_bits_buf(it_bit_buff, 1)) &
                1) {
              if ((retval = ixheaacd_ltp_data(object_type, ptr_ics_info,
                                              &(ptr_ics_info->ltp2),
                                              it_bit_buff, frame_size)) > 0) {
                return retval;
              }
            }
          }
        }
        if ((object_type == AOT_ER_AAC_ELD) || (object_type == AOT_ER_AAC_LD)) {
          if (!common_window && (object_type >= ER_OBJECT_START)) {
            if ((ptr_ics_info->ltp.data_present =
                     ixheaacd_read_bits_buf(it_bit_buff, 1)) &
                1) {
              if ((retval = ixheaacd_ltp_data(object_type, ptr_ics_info,
                                              &(ptr_ics_info->ltp), it_bit_buff,
                                              frame_size)) < 0) {
                return retval;
              }
            }
          }
        }
      }
    }

  } else {
    WORD32 num_groups = 0, scale_factor_grouping;
    ptr_ics_info->num_swb_window = num_swb_window[1];

    value = ixheaacd_read_bits_buf(it_bit_buff, 11);
    ptr_ics_info->max_sfb = (value & 0x780) >> 7;

    scale_factor_grouping = (value & 0x7F);

    mask = 0x40;
    for (i = 0; i < 7; i++) {
      ptr_ics_info->window_group_length[i] = 1;

      if (scale_factor_grouping & mask) {
        ptr_ics_info->window_group_length[num_groups] =
            ptr_ics_info->window_group_length[num_groups] + 1;

      } else {
        num_groups = num_groups + 1;
      }

      mask = mask >> 1;
    }

    ptr_ics_info->window_group_length[7] = 1;
    ptr_ics_info->num_window_groups = num_groups + 1;
  }

  if (ptr_ics_info->max_sfb > ptr_ics_info->num_swb_window) {
    return (WORD16)IA_XHEAAC_DEC_EXE_NONFATAL_EXCEEDS_SFB_TRANSMITTED;
  }

  return AAC_DEC_OK;
}

WORD16 ixheaacd_individual_ch_stream(
    ia_bit_buf_struct *it_bit_buff, ia_aac_decoder_struct *aac_dec_handle,
    WORD32 num_ch, WORD32 frame_size, WORD32 total_channels, WORD32 object_type,
    ia_eld_specific_config_struct eld_specific_config, WORD32 ele_type) {
  WORD16 error_code = AAC_DEC_OK;
  WORD32 ch;
  WORD32 crc_reg = 0;

  for (ch = 0; ch < num_ch; ch++) {
    ia_aac_dec_channel_info_struct *ptr_aac_dec_ch_info =
        aac_dec_handle->pstr_aac_dec_ch_info[ch];
    ia_ics_info_struct *ptr_ics_info = &ptr_aac_dec_ch_info->str_ics_info;

    if (ch == 1) {
      if (it_bit_buff->pstr_adts_crc_info->crc_active == 1 &&
          (it_bit_buff->pstr_adts_crc_info->no_reg < 7)) {
        crc_reg =
            ixheaacd_adts_crc_start_reg(it_bit_buff->pstr_adts_crc_info,
                                        it_bit_buff, CRC_ADTS_RAW_IIND_ICS);
      }
    }
    ptr_aac_dec_ch_info->global_gain =
        (WORD16)ixheaacd_read_bits_buf(it_bit_buff, 8);

    if (!(aac_dec_handle->pstr_aac_dec_ch_info[LEFT]->common_window)) {
      error_code = ixheaacd_ics_read(
          it_bit_buff, ptr_ics_info, aac_dec_handle->num_swb_window,
          object_type,
          aac_dec_handle->pstr_aac_dec_ch_info[LEFT]->common_window,
          aac_dec_handle->samples_per_frame);
      if (ch == 1)
        aac_dec_handle->pstr_aac_dec_ch_info[ch - 1]->str_ics_info.ltp2.lag =
            ptr_ics_info->ltp.lag;

      if (error_code) {
        if (it_bit_buff->cnt_bits < 0) {
          error_code = (WORD16)(
              (WORD32)IA_XHEAAC_DEC_EXE_NONFATAL_INSUFFICIENT_INPUT_BYTES);
        }
        return error_code;
      }
    }

    error_code = ixheaacd_read_block_data(
        it_bit_buff, ptr_aac_dec_ch_info, aac_dec_handle->pstr_aac_tables,
        total_channels, frame_size, object_type,
        eld_specific_config.aac_spect_data_resil_flag,
        eld_specific_config.aac_sect_data_resil_flag,
        eld_specific_config.aac_sf_data_resil_flag, ele_type,
        aac_dec_handle->pstr_aac_dec_overlap_info[ch]);
    if (error_code) {
      if (it_bit_buff->cnt_bits < 0) {
        error_code = (WORD16)(
            (WORD32)IA_XHEAAC_DEC_EXE_NONFATAL_INSUFFICIENT_INPUT_BYTES);
      }

      return error_code;
    }

    if (ch == 0) {
      if ((object_type == AOT_ER_AAC_LD) &&
          (aac_dec_handle->pstr_aac_dec_ch_info[LEFT]->common_window) &&
          (ele_type == ID_CPE)) {
        IA_ERRORCODE temp =
            ixheaacd_ltp_decode(it_bit_buff, ptr_ics_info, object_type,
                                aac_dec_handle->samples_per_frame, 1);

        if (temp != 0) {
          return temp;
        }
        aac_dec_handle->pstr_aac_dec_ch_info[ch + 1]->str_ics_info.ltp.lag =
            ptr_ics_info->ltp2.lag;
      }
    }
    if (ch == 1) {
      if (it_bit_buff->pstr_adts_crc_info->crc_active == 1) {
        ixheaacd_adts_crc_end_reg(it_bit_buff->pstr_adts_crc_info, it_bit_buff,
                                  crc_reg);
      }
    }
  }

  return error_code;
}

VOID ixheaacd_read_ms_data(
    ia_bit_buf_struct *it_bit_buff,
    ia_aac_dec_channel_info_struct *ptr_aac_dec_ch_info) {
  WORD32 num_win_group, sfb;
  WORD32 ms_mask_present;
  UWORD8 *ptr_ms_used = &ptr_aac_dec_ch_info->pstr_stereo_info->ms_used[0][0];
  WORD32 num_window_groups =
      ptr_aac_dec_ch_info->str_ics_info.num_window_groups;
  WORD16 max_sfb = ptr_aac_dec_ch_info->str_ics_info.max_sfb;

  ms_mask_present = ixheaacd_read_bits_buf(it_bit_buff, 2);

  if (ms_mask_present < 1) {
    memset(ptr_ms_used, 0,
           sizeof(UWORD8) * JOINT_STEREO_MAX_BANDS * JOINT_STEREO_MAX_GROUPS);
  }

  else if (ms_mask_present == 1) {
    for (num_win_group = 0; num_win_group < num_window_groups;
         num_win_group++) {
      for (sfb = 0; sfb < max_sfb; sfb++) {
        ptr_aac_dec_ch_info->pstr_stereo_info->ms_used[num_win_group][sfb] =
            (UWORD8)ixheaacd_read_bits_buf(it_bit_buff, 1);
      }
    }

  } else {
    for (num_win_group = 0; num_win_group < num_window_groups;
         num_win_group++) {
      ptr_ms_used =
          &ptr_aac_dec_ch_info->pstr_stereo_info->ms_used[num_win_group][0];
      memset(ptr_ms_used, 1, (max_sfb) * sizeof(UWORD8));
    }
  }
}

IA_ERRORCODE ixheaacd_channel_pair_process(
    ia_aac_dec_channel_info_struct *ptr_aac_dec_channel_info[CHANNELS], WORD32 num_ch,
    ia_aac_dec_tables_struct *ptr_aac_tables, WORD32 total_channels, WORD32 object_type,
    WORD32 aac_spect_data_resil_flag, WORD32 aac_sf_data_resil_flag, WORD32 *in_data,
    WORD32 *out_data, void *self_ptr) {
  WORD32 channel;
  IA_ERRORCODE err = IA_NO_ERROR;
  ia_aac_decoder_struct *self = self_ptr;
  if (aac_spect_data_resil_flag &&
      ((object_type == AOT_ER_AAC_LD) || (object_type == AOT_ER_AAC_ELD) ||
      (object_type == AOT_ER_AAC_LC))) {
    for (channel = 0; channel < num_ch; channel++) {
      err = ixheaacd_cblock_inv_quant_spect_data(
          ptr_aac_dec_channel_info[channel], ptr_aac_tables);
      if (err) return err;
      ixheaacd_cblock_scale_spect_data(ptr_aac_dec_channel_info[channel],
                                       ptr_aac_tables, num_ch, object_type,
                                       aac_sf_data_resil_flag);
    }
  }

  if (num_ch > 1) {
    if (ptr_aac_dec_channel_info[LEFT]->common_window) {
      if (ptr_aac_dec_channel_info[LEFT]->str_pns_info.pns_active ||
          ptr_aac_dec_channel_info[RIGHT]->str_pns_info.pns_active) {
        ixheaacd_map_ms_mask_pns(ptr_aac_dec_channel_info);
      }

      ixheaacd_ms_stereo_process(ptr_aac_dec_channel_info, ptr_aac_tables);
    }

    ixheaacd_intensity_stereo_process(
        ptr_aac_dec_channel_info, ptr_aac_tables, object_type,
        aac_sf_data_resil_flag,
        ptr_aac_dec_channel_info[LEFT]->str_ics_info.frame_length);
  }

  for (channel = 0; channel < num_ch; channel++) {
    WORD32 *p_spectrum = ptr_aac_dec_channel_info[channel]->ptr_spec_coeff;

    if (total_channels > 2) {
      if (ptr_aac_dec_channel_info[channel]->str_ics_info.window_sequence !=
          EIGHT_SHORT_SEQUENCE) {
        WORD16 *band_offsets = ptr_aac_tables->sfb_long_table;
        WORD32 no_spec_coeff = band_offsets[ptr_aac_dec_channel_info[channel]
                                                ->str_ics_info.max_sfb];
        ixheaacd_right_shift_block(p_spectrum, no_spec_coeff, 3);
      } else {
        ixheaacd_right_shift_block(p_spectrum, 1024, 3);
      }
    }

    ixheaacd_pns_process(ptr_aac_dec_channel_info, channel, ptr_aac_tables);

    if ((object_type == AOT_ER_AAC_LD) || (object_type == AOT_AAC_LTP)) {
      {
        if (channel == 0) {
          ltp_info *ltp1 =
              &(ptr_aac_dec_channel_info[channel]->str_ics_info.ltp);
          ixheaacd_lt_prediction(ptr_aac_dec_channel_info[channel], ltp1,
                                 p_spectrum, ptr_aac_tables,
                                 self->ptr_aac_dec_static_channel_info[LEFT]
                                     ->overlap_add_data.win_shape,
                                 self->sampling_rate_index, object_type,
                                 self->samples_per_frame, in_data, out_data);

        } else {
          ltp_info *ltp2 =
              (self->pstr_aac_dec_ch_info[0]->common_window)
                  ? &(ptr_aac_dec_channel_info[0]->str_ics_info.ltp2)
                  :

                  &(ptr_aac_dec_channel_info[1]->str_ics_info.ltp);
          ixheaacd_lt_prediction(ptr_aac_dec_channel_info[channel], ltp2,
                                 p_spectrum, ptr_aac_tables,
                                 self->ptr_aac_dec_static_channel_info[RIGHT]
                                     ->overlap_add_data.win_shape,
                                 self->sampling_rate_index, object_type,
                                 self->samples_per_frame, in_data, out_data);
        }
      }
    }

    if (ptr_aac_dec_channel_info[channel]->str_tns_info.tns_data_present) {
      ixheaacd_aac_tns_process(ptr_aac_dec_channel_info[channel],
                               total_channels, ptr_aac_tables, object_type, 1,
                               NULL);
    }
  }
  return err;
}

VOID ixheaacd_set_corr_info(
    ia_aac_dec_channel_info_struct *ptr_aac_dec_channel_info, WORD16 pns_band) {
  ia_pns_correlation_info_struct *ptr_corr_info =
      ptr_aac_dec_channel_info->pstr_pns_corr_info;
  ptr_corr_info->correlated[(pns_band >> PNS_BAND_FLAGS_SHIFT)] |=
      (1 << (pns_band & PNS_BAND_FLAGS_MASK));
}

VOID ixheaacd_map_ms_mask_pns(
    ia_aac_dec_channel_info_struct *ptr_aac_dec_channel_info[CHANNELS]) {
  WORD32 num_win_group, sfb;

  for (num_win_group = 0;
       num_win_group <
       ptr_aac_dec_channel_info[LEFT]->str_ics_info.num_window_groups;
       num_win_group++) {
    for (sfb = 0; sfb < ptr_aac_dec_channel_info[LEFT]->str_ics_info.max_sfb;
         sfb++) {
      if (ptr_aac_dec_channel_info[LEFT]
              ->pstr_stereo_info->ms_used[num_win_group][sfb]) {
        WORD16 pns_band = (num_win_group << 4) + sfb;
        ixheaacd_set_corr_info(ptr_aac_dec_channel_info[LEFT], pns_band);

        if (ptr_aac_dec_channel_info[LEFT]->str_pns_info.pns_used[pns_band] &&
            ptr_aac_dec_channel_info[RIGHT]->str_pns_info.pns_used[pns_band]) {
          ptr_aac_dec_channel_info[LEFT]
              ->pstr_stereo_info->ms_used[num_win_group][sfb] ^= 1;
        }
      }
    }
  }
}

VOID ixheaacd_pulse_data_apply(ia_pulse_info_struct *ptr_pulse_info,
                               WORD8 *pulse_scratch,
                               const WORD16 *ptr_swb_offset, WORD object_type) {
  WORD i;
  WORD32 k;

  memset(pulse_scratch, 0, sizeof(WORD32) * 256);

  if (object_type != AOT_ER_AAC_ELD) {
    if (ptr_pulse_info->pulse_data_present) {
      k = ptr_swb_offset[ptr_pulse_info->pulse_start_band];

      for (i = 0; i <= ptr_pulse_info->number_pulse; i++) {
        k = k + ptr_pulse_info->pulse_offset[i];
        pulse_scratch[k] = ptr_pulse_info->pulse_amp[i];
      }
    }
  } else {
    ptr_pulse_info->pulse_data_present = 0;
  }
}

IA_ERRORCODE ixheaacd_read_spectral_data(
    ia_bit_buf_struct *it_bit_buff,
    ia_aac_dec_channel_info_struct *ptr_aac_dec_channel_info,
    ia_aac_dec_tables_struct *ptr_aac_tables, WORD32 total_channels,
    WORD32 frame_size, WORD32 object_type, WORD32 aac_spect_data_resil_flag,
    WORD32 aac_sf_data_resil_flag) {
  WORD sfb, max_sfb;
  WORD num_win_grp, group_len, grp_offset;

  WORD index;
  WORD8 *ptr_code_book;
  WORD16 *ptr_scale_factor;
  WORD32 *ptr_spec_coef;
  ia_ics_info_struct *ptr_ics_info = &ptr_aac_dec_channel_info->str_ics_info;
  WORD16 *swb_offset;

  WORD32 maximum_bins_short = ptr_ics_info->frame_length >> 3;

  WORD32 *ptr_spec_coef_out;

  ptr_code_book = ptr_aac_dec_channel_info->ptr_code_book;
  ptr_scale_factor = ptr_aac_dec_channel_info->ptr_scale_factor;
  ptr_spec_coef = ptr_aac_dec_channel_info->ptr_spec_coeff;
  max_sfb = ptr_ics_info->max_sfb;

  swb_offset =
      ptr_aac_tables->str_aac_sfb_info[ptr_ics_info->window_sequence].sfb_index;

  if (!aac_spect_data_resil_flag) {
    if (ptr_aac_dec_channel_info->str_ics_info.window_sequence !=
        EIGHT_SHORT_SEQUENCE) {
      WORD8 *ptr_scratch;

      if (object_type == AOT_ER_AAC_ELD)
        ptr_scratch = (WORD8 *)ptr_aac_dec_channel_info->pulse_scratch;
      else
        ptr_scratch = (WORD8 *)ptr_aac_dec_channel_info->scratch_buf_ptr;

      memset(ptr_spec_coef, 0, sizeof(WORD32) * 1024);

      ixheaacd_pulse_data_apply(&ptr_aac_dec_channel_info->str_pulse_info,
                                ptr_scratch, swb_offset, object_type);

      ptr_spec_coef_out = &ptr_spec_coef[0];
      for (sfb = 0; sfb < max_sfb;) {
        WORD ret_val;
        WORD32 sfb_width;
        WORD32 sect_cb = ptr_code_book[sfb];
        WORD start = sfb;
        if ((object_type == AOT_ER_AAC_ELD) || (object_type == AOT_ER_AAC_LD)
            || (object_type == AOT_ER_AAC_LC)) {
          if ((sect_cb >= 16) && (sect_cb <= 31)) {
            ptr_code_book[sfb] = sect_cb = 11;
          }
        }
        for (; sfb < max_sfb && (ptr_code_book[sfb] == sect_cb); sfb++)
          ;

        sfb_width = swb_offset[sfb] - swb_offset[start];

        if (sect_cb > ZERO_HCB && (sect_cb < NOISE_HCB)) {
          ret_val = ixheaacd_huffman_dec_word2(it_bit_buff, sect_cb, sfb_width,
                                               ptr_aac_tables,
                                               ptr_spec_coef_out, ptr_scratch);

          if (ret_val != 0) {
            return (WORD16)(
                (WORD32)IA_XHEAAC_DEC_EXE_NONFATAL_EXCEEDS_MAX_HUFFDEC_VAL);
          }
        }

        else {
          if (ptr_aac_dec_channel_info->str_pulse_info.pulse_data_present) {
            ixheaacd_inverse_quantize(
                ptr_spec_coef_out, sfb_width,
                (WORD32 *)
                    ptr_aac_tables->pstr_block_tables->ixheaacd_pow_table_Q13,
                ptr_scratch);
          }
        }
        ptr_scratch += sfb_width;
        ptr_spec_coef_out += sfb_width;
      }

      if ((object_type != AOT_ER_AAC_ELD) && (object_type != AOT_ER_AAC_LD))
        index = 1024 - swb_offset[max_sfb];
      else
        index = frame_size - swb_offset[max_sfb];

      if (index < 0) return -1;

    } else {
      memset(ptr_spec_coef, 0, sizeof(WORD32) * 1024);

      grp_offset = 0;

      for (num_win_grp = 0; num_win_grp < ptr_ics_info->num_window_groups;
           num_win_grp++) {
        WORD grp_len = ptr_ics_info->window_group_length[num_win_grp];

        if (maximum_bins_short == 120)
          ptr_spec_coef_out = &ptr_spec_coef[grp_offset * maximum_bins_short];
        else
          ptr_spec_coef_out = &ptr_spec_coef[grp_offset * MAX_BINS_SHORT];

        WORD bands = num_win_grp * MAX_SCALE_FACTOR_BANDS_SHORT;
        for (sfb = 0; sfb < max_sfb;) {
          WORD sect_cb = ptr_code_book[bands];
          WORD start = sfb;
          WORD ret_val;

          if ((object_type == AOT_ER_AAC_ELD) || (object_type == AOT_ER_AAC_LD)
              || (object_type == AOT_ER_AAC_LC)) {
            if ((sect_cb >= 16) && (sect_cb <= 31)) {
              ptr_code_book[bands] = sect_cb = 11;
            }
          }

          for (; sfb < max_sfb && (ptr_code_book[bands] == sect_cb);
               sfb++, bands++)
            ;

          if (sect_cb > ZERO_HCB && (sect_cb < NOISE_HCB)) {
            ret_val = ixheaacd_decode_huffman(
                it_bit_buff, sect_cb, ptr_spec_coef_out, (WORD16 *)swb_offset,
                start, sfb, grp_len, ptr_aac_tables, maximum_bins_short);

            if (ret_val != 0) {
              return (WORD16)(
                  (WORD32)
                      IA_XHEAAC_DEC_EXE_NONFATAL_EXCEEDS_MAX_HUFFDEC_VAL);
            }
          }
        }
        grp_offset = (grp_offset + grp_len);
      }
    }
    {
      WORD32 *ptr_scale_table;

      if (maximum_bins_short != 120)
        ptr_scale_table = ptr_aac_tables->pstr_block_tables->scale_table;
      else
        ptr_scale_table = ptr_aac_tables->pstr_block_tables->scale_table_960;

      WORD8 *ptr_sfb_width =
          ptr_aac_tables->str_aac_sfb_info[ptr_ics_info->window_sequence]
              .sfb_width;

      for (num_win_grp = 0; num_win_grp < ptr_ics_info->num_window_groups;
           num_win_grp++) {
        for (group_len = 0;
             group_len < ptr_ics_info->window_group_length[num_win_grp];
             group_len++) {
          (*ixheaacd_scale_factor_process)(
              &ptr_spec_coef[0], &ptr_scale_factor[0], max_sfb,
              (WORD8 *)ptr_sfb_width, ptr_scale_table, total_channels,
              object_type, aac_sf_data_resil_flag);

          if (maximum_bins_short == 120)
            ptr_spec_coef += maximum_bins_short;
          else
            ptr_spec_coef += MAX_BINS_SHORT;
        }

        ptr_scale_factor += MAX_SCALE_FACTOR_BANDS_SHORT;
      }
    }
  } else {
    ia_hcr_info_struct *pstr_hcr_info = &ptr_aac_dec_channel_info->str_hcr_info;
    WORD32 error = 0;

    memset(ptr_spec_coef, 0, sizeof(WORD32) * 1024);

    if (ptr_aac_dec_channel_info->reorder_spect_data_len != 0) {
      error = ixheaacd_huff_code_reorder_init(
          pstr_hcr_info, ptr_aac_dec_channel_info, ptr_aac_tables, it_bit_buff);

      if (error != 0) {
        return IA_XHEAAC_DEC_EXE_NONFATAL_DECODE_FRAME_ERROR;
      }
      error = ixheaacd_hcr_decoder(pstr_hcr_info, ptr_aac_dec_channel_info,
                                   ptr_aac_tables, it_bit_buff);

      if (error != 0) {
        ixheaacd_huff_mute_erroneous_lines(pstr_hcr_info);
      }

      if (it_bit_buff->cnt_bits <
          ptr_aac_dec_channel_info->reorder_spect_data_len) {
        longjmp(*(it_bit_buff->xaac_jmp_buf),
                IA_XHEAAC_DEC_EXE_NONFATAL_INSUFFICIENT_INPUT_BYTES);
      }

      it_bit_buff->cnt_bits +=
          -ptr_aac_dec_channel_info->reorder_spect_data_len;
      it_bit_buff->ptr_read_next =
          it_bit_buff->ptr_bit_buf_base +
          ((it_bit_buff->size - it_bit_buff->cnt_bits) >> 3);
      it_bit_buff->bit_pos = (it_bit_buff->size - it_bit_buff->cnt_bits) & 7;

    } else {
      it_bit_buff->ptr_read_next =
          it_bit_buff->ptr_bit_buf_base +
          ((it_bit_buff->size - it_bit_buff->cnt_bits) >> 3);
      it_bit_buff->bit_pos = (it_bit_buff->size - it_bit_buff->cnt_bits) & 7;
    }
  }

  return AAC_DEC_OK;
}

WORD16 ixheaacd_read_tns_data(
    ia_bit_buf_struct *it_bit_buff,
    ia_aac_dec_channel_info_struct *ptr_aac_dec_channel_info) {
  WORD win_size, window_per_frame;
  WORD n_filt_bits, start_band_bits, order_bits;
  WORD32 bottom;

  ia_ics_info_struct *ptr_ics_info = &ptr_aac_dec_channel_info->str_ics_info;
  ia_tns_info_aac_struct *ptr_tns_info =
      &ptr_aac_dec_channel_info->str_tns_info;

  if (ptr_ics_info->window_sequence != EIGHT_SHORT_SEQUENCE) {
    n_filt_bits = 2;
    start_band_bits = 6;
    order_bits = 5;
    window_per_frame = 1;

  } else {
    n_filt_bits = 1;
    start_band_bits = 4;
    order_bits = 3;
    window_per_frame = 8;
  }

  bottom = ptr_ics_info->num_swb_window;

  for (win_size = 0; win_size < window_per_frame; win_size++) {
    WORD n_filt;
    WORD start_band, coef_res;
    ptr_tns_info->n_filt[win_size] = n_filt =
        (WORD16)ixheaacd_read_bits_buf(it_bit_buff, n_filt_bits);

    if (n_filt) {
      WORD filt;
      WORD top;

      coef_res = ixheaacd_read_bits_buf(it_bit_buff, 1);

      top = bottom;
      for (filt = 0; filt < n_filt; filt++) {
        WORD order;
        ia_filter_info_struct *filter =
            &ptr_tns_info->str_filter[win_size][filt];

        start_band = ixheaacd_read_bits_buf(it_bit_buff, start_band_bits);

        if (top < start_band) {
          top = start_band;
        }
        filter->start_band = top - start_band;
        filter->stop_band = top;

        top = filter->start_band;

        if (filter->start_band < 0) {
          filter->order = -1;
          return (WORD16)((WORD32)IA_XHEAAC_DEC_EXE_FATAL_TNS_RANGE_ERROR);
        }

        filter->order = order = ixheaacd_read_bits_buf(it_bit_buff, order_bits);

        if ((order - MAX_ORDER_LONG) > 0) {
          return (WORD16)(
              (WORD32)IA_XHEAAC_DEC_EXE_NONFATAL_TNS_ORDER_ERROR);
        }

        if (order) {
          WORD i;
          WORD32 coef, coef_compress;
          WORD resolution, shift;

          filter->direction =
              (WORD8)(ixheaacd_read_bits_buf(it_bit_buff, 1) ? -1 : 1);

          coef_compress = ixheaacd_read_bits_buf(it_bit_buff, 1);

          filter->resolution = coef_res;

          resolution = coef_res + 3 - coef_compress;

          shift = 32 - resolution;

          for (i = 0; i < order; i++) {
            coef = ixheaacd_read_bits_buf(it_bit_buff, resolution);
            coef = coef << shift;
            filter->coef[i] = (WORD8)(coef >> shift);
          }
        }
      }
    }
  }
  return AAC_DEC_OK;
}

WORD32 ixheaacd_inv_quant(WORD32 *px_quant, WORD32 *ixheaacd_pow_table_Q13)

{
  WORD32 q1;
  WORD32 temp;
  WORD32 q_abs;
  WORD16 interp;
  WORD32 shift;

  q_abs = *px_quant;

  if (q_abs > (8191 + 32))
    return IA_XHEAAC_DEC_EXE_NONFATAL_EXCEEDS_MAX_HUFFDEC_VAL;

  if (q_abs < 1024) {
    shift = 3;
  } else {
    shift = 6;
  }

  q1 = (q_abs) >> shift;

  interp = q_abs - (q1 << shift);

  temp = ixheaacd_pow_table_Q13[q1 + 1] - ixheaacd_pow_table_Q13[q1];

  temp = (WORD32)(temp * (WORD32)interp);

  temp = temp + (ixheaacd_pow_table_Q13[q1] << shift);

  if (shift == 3)
    temp = temp << 1;
  else
    temp = temp << 2;

  *px_quant = temp;

  return 0;
}

void ixheaacd_scale_value_in_place(WORD32 *value, WORD32 scalefactor) {
  WORD32 newscale;

  if ((newscale = (scalefactor)) >= 0) {
    *(value) <<= newscale;
  } else {
    *(value) >>= -newscale;
  }
}

WORD32 ixheaacd_cblock_inv_quant_spect_data(
    ia_aac_dec_channel_info_struct *ptr_aac_dec_channel_info,
    ia_aac_dec_tables_struct *ptr_aac_tables) {
  int window, group, grp_win, band;
  IA_ERRORCODE err = IA_NO_ERROR;
  int sf_bands_transmitted = ptr_aac_dec_channel_info->str_ics_info.max_sfb;
  WORD8 *ptr_code_book = ptr_aac_dec_channel_info->ptr_code_book;
  const WORD16 *band_offsets = (WORD16 *)ixheaacd_getscalefactorbandoffsets(
      &(ptr_aac_dec_channel_info->str_ics_info), ptr_aac_tables);
  WORD32 *ptr_pow_table_Q13 =
      (WORD32 *)ptr_aac_tables->pstr_block_tables->ixheaacd_pow_table_Q13;

  for (window = 0, group = 0;
       group < ptr_aac_dec_channel_info->str_ics_info.num_window_groups;
       group++) {
    for (grp_win = 0;
         grp_win <
         ptr_aac_dec_channel_info->str_ics_info.window_group_length[group];
         grp_win++, window++) {
      for (band = 0; band < sf_bands_transmitted; band++) {
        WORD32 *ptr_spec_coef =
            SPEC(ptr_aac_dec_channel_info->ptr_spec_coeff, window,
                 ptr_aac_dec_channel_info->granule_len) +
            band_offsets[band];
        int num_lines = band_offsets[band + 1] - band_offsets[band];
        int bnds = group * 16 + band;
        int i;

        if ((ptr_code_book[bnds] == ZERO_HCB) ||
            (ptr_code_book[bnds] == INTENSITY_HCB) ||
            (ptr_code_book[bnds] == INTENSITY_HCB2))
          continue;

        if (ptr_code_book[bnds] == NOISE_HCB) {
          continue;
        }

        for (i = 0; i < num_lines; i++) {
          WORD8 temp = 0;
          WORD32 out1 = ptr_spec_coef[i];
          if (out1 <= 0) {
            out1 = sub_d(temp, out1);
            if (out1 > 127) {
              err = ixheaacd_inv_quant(&out1, ptr_pow_table_Q13);
              if (err) return err;
            } else
              out1 = ptr_pow_table_Q13[out1];
            ptr_spec_coef[i] = -out1;

          } else {
            if (out1 > 127) {
              err = ixheaacd_inv_quant(&out1, ptr_pow_table_Q13);
              if (err) return err;
            } else
              out1 = ptr_pow_table_Q13[out1];

            ptr_spec_coef[i] = out1;
          }
        }
      }
    }
  }

  return AAC_DEC_OK;
}

void ixheaacd_init_ltp_object(ltp_info *ltp) {
  ltp->data_present = 0;
  ltp->last_band = 0;

  ltp->lag_update = 0;
  ltp->coef = 0;
}

WORD32 ixheaacd_ltp_data(WORD32 object_type, ia_ics_info_struct *ics,
                         ltp_info *ltp, ia_bit_buf_struct *it_bit_buf,
                         WORD32 frame_len) {
  UWORD8 sfb, w;

  if (object_type == AOT_ER_AAC_LD) {
    ltp->lag_update = ixheaacd_read_bits_buf(it_bit_buf, 1);

    if (ltp->lag_update) {
      ltp->lag = (UWORD16)ixheaacd_read_bits_buf(it_bit_buf, 10);
    }
  } else {
    ltp->lag = (UWORD16)ixheaacd_read_bits_buf(it_bit_buf, 11);
  }

  if (ltp->lag > (frame_len << 1)) return -1;

  ltp->coef = (UWORD8)ixheaacd_read_bits_buf(it_bit_buf, 3);

  if (ics->window_sequence == EIGHT_SHORT_SEQUENCE) {
    for (w = 0; w < 8; w++) {
      if ((ltp->short_used[w] = ixheaacd_read_bits_buf(it_bit_buf, 1)) & 1) {
        ltp->short_lag_present[w] = ixheaacd_read_bits_buf(it_bit_buf, 1);
        if (ltp->short_lag_present[w]) {
          ltp->short_lag[w] = (UWORD8)ixheaacd_read_bits_buf(it_bit_buf, 4);
        }
      }
    }
  } else {
    ltp->last_band = (ics->max_sfb < MAX_LTP_SFB ? ics->max_sfb : MAX_LTP_SFB);

    for (sfb = 0; sfb < ltp->last_band; sfb++) {
      ltp->long_used[sfb] = ixheaacd_read_bits_buf(it_bit_buf, 1);
    }
  }
  if (ics->frame_length == 480) {
    if ((ics->sampling_rate_index > 5) &&
        (ltp->last_band > MAX_LTP_SFB_SR_FIVE_PLUS_480))
      ltp->last_band = MAX_LTP_SFB_SR_FIVE_PLUS_480;
    else if ((ics->sampling_rate_index == 5) &&
             (ltp->last_band > MAX_LTP_SFB_SR_FIVE_480))
      ltp->last_band = MAX_LTP_SFB_SR_FIVE_480;
    else if ((ics->sampling_rate_index < 5) &&
             (ltp->last_band > MAX_LTP_SFB_SR_FIVE_LESS_480))
      ltp->last_band = MAX_LTP_SFB_SR_FIVE_LESS_480;
  } else if (ics->frame_length == 512) {
    if ((ics->sampling_rate_index > 5) &&
        (ltp->last_band > MAX_LTP_SFB_SR_FIVE_PLUS_512))
      ltp->last_band = MAX_LTP_SFB_SR_FIVE_PLUS_512;
    else if ((ics->sampling_rate_index == 5) &&
             (ltp->last_band > MAX_LTP_SFB_SR_FIVE_512))
      ltp->last_band = MAX_LTP_SFB_SR_FIVE_512;
    else if ((ics->sampling_rate_index < 5) &&
             (ltp->last_band > MAX_LTP_SFB_SR_FIVE_LESS_512))
      ltp->last_band = MAX_LTP_SFB_SR_FIVE_LESS_512;
  }
  return 0;
}
