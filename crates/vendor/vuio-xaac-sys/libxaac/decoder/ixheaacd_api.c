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
#include <stdio.h>
#include <stdlib.h>
#include "ixheaacd_sbr_common.h"
#include "ixheaac_type_def.h"
#include "ixheaacd_memory_standards.h"
#include "ixheaacd_mps_struct_def.h"
#include "ixheaacd_mps_res_rom.h"
#include "ixheaacd_mps_aac_struct.h"
#include "ixheaac_constants.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops16.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaac_basic_ops.h"

#include "ixheaac_error_standards.h"
#include "ixheaacd_apicmd_standards.h"
#include "ixheaacd_aac_config.h"
#include "ixheaacd_api_defs.h"

#include "ixheaacd_definitions.h"
#include "ixheaacd_error_codes.h"
#include "ixheaacd_bitbuffer.h"

#include "ixheaacd_audioobjtypes.h"
#include "ixheaacd_sbrdecsettings.h"
#include "ixheaacd_memory_standards.h"

#include "ixheaacd_bitbuffer.h"
#include "ixheaacd_adts.h"
#include "ixheaacd_defines.h"
#include "ixheaacd_cnst.h"
#include "ixheaacd_aac_rom.h"
#include "ixheaacd_ec_defines.h"
#include "ixheaacd_ec_struct_def.h"

#include "ixheaacd_sbr_scale.h"
#include "ixheaacd_lpp_tran.h"
#include "ixheaacd_env_extr_part.h"
#include "ixheaacd_sbr_rom.h"

#include "ixheaacd_hybrid.h"
#include "ixheaacd_ps_dec.h"
#include "ixheaacd_ps_bitdec.h"

#include "ixheaacd_pulsedata.h"

#include "ixheaacd_pns.h"
#include "ixheaacd_drc_data_struct.h"

#include "ixheaacd_lt_predict.h"

#include "ixheaacd_channelinfo.h"
#include "ixheaacd_drc_dec.h"
#include "ixheaac_sbr_const.h"
#include "ixheaacd_sbrdecoder.h"
#include "ixheaacd_env_extr.h"
#include "ixheaacd_common_rom.h"
#include "ixheaacd_freq_sca.h"
#include "ixheaacd_qmf_dec.h"
#include "ixheaacd_env_calc.h"

#include "ixheaacd_pvc_dec.h"
#include "ixheaacd_sbr_dec.h"
#include "ixheaacd_block.h"
#include "ixheaacd_channel.h"

#include "ixheaacd_audioobjtypes.h"
#include "ixheaacd_latmdemux.h"

#include "ixheaacd_aacdec.h"
#include "ixheaacd_mps_polyphase.h"
#include "ixheaacd_config.h"
#include "ixheaacd_mps_dec.h"
#include "ixheaacd_struct_def.h"
#include "ixheaacd_headerdecode.h"
#include "ixheaacd_adts_crc_check.h"

#include "ixheaacd_multichannel.h"
#include "ixheaacd_ver_number.h"

#include "ixheaacd_interface.h"
#include "ixheaacd_info.h"

#include "ixheaacd_config.h"

#include "ixheaacd_struct.h"
#include "ixheaacd_mps_polyphase.h"
#include "ixheaacd_tns_usac.h"
#include "ixheaacd_acelp_info.h"

#include "ixheaacd_main.h"
#include "ixheaacd_arith_dec.h"
#include "ixheaacd_create.h"
#include "ixheaacd_function_selector.h"
#include "ixheaacd_ld_mps_dec.h"
#include "ixheaacd_mps_tables.h"
#define MAX_TRACKS_PER_LAYER 50

#define ALIGN_SIZE64(x) ((((x) + 7) >> 3) << 3)

#define IA_ENHAACPDEC_NUM_MEMTABS (4)

#define NUM_AAC_TABLES 8

#define LD_OBJ -2

#define SCR_BASE_SCR_8K_SIZE \
  (IXHEAAC_GET_SIZE_ALIGNED((2 * CHANNELS * MAX_BINS_LONG * sizeof(WORD32)), BYTE_ALIGN_8))
#define SCR_EXTRA_SCR_4K_0_SIZE                                                           \
  (2 * IXHEAAC_GET_SIZE_ALIGNED(sizeof(ia_aac_dec_channel_info_struct), sizeof(WORD32)) + \
   2 * IXHEAAC_GET_SIZE_ALIGNED(sizeof(ia_aac_sfb_code_book_struct), sizeof(WORD32)) +    \
   IXHEAAC_GET_SIZE_ALIGNED(sizeof(ia_pns_stereo_data_struct), sizeof(WORD32)))
#define SCR_EXTRA_SCR_4K_2_SIZE \
  (IXHEAAC_GET_SIZE_ALIGNED((IA_ENHAACPLUS_DEC_SAMPLES_PER_FRAME * sizeof(WORD32)), BYTE_ALIGN_8))
#define SCR_EXTRA_SCR_4K_3_SIZE \
  (IXHEAAC_GET_SIZE_ALIGNED((IA_ENHAACPLUS_DEC_SAMPLES_PER_FRAME * sizeof(WORD32)), BYTE_ALIGN_8))
#define SCR_OUT_DATA_SIZE \
  (IXHEAAC_GET_SIZE_ALIGNED((IA_ENHAACPLUS_DEC_SAMPLES_PER_FRAME * sizeof(WORD32)), BYTE_ALIGN_8))
#define SCR_IN_DATA_SIZE                                                                \
  (2 * IXHEAAC_GET_SIZE_ALIGNED((IA_ENHAACPLUS_DEC_SAMPLES_PER_FRAME * sizeof(WORD32)), \
                                BYTE_ALIGN_8))
#define SCR_INTER_SCR_SIZE                                                          \
  (MAX_CHANNEL_COUNT *                                                              \
   IXHEAAC_GET_SIZE_ALIGNED((IA_ENHAACPLUS_DEC_SAMPLES_PER_FRAME * sizeof(WORD16)), \
                            BYTE_ALIGN_8))
#define SCR_COUP_CH_OUT_SIZE                                                        \
  (MAX_CHANNEL_COUNT *                                                              \
   IXHEAAC_GET_SIZE_ALIGNED((IA_ENHAACPLUS_DEC_SAMPLES_PER_FRAME * sizeof(WORD16)), \
                            BYTE_ALIGN_8))

#define P_IND_CH_INFO_OFFSET \
  (SCR_BASE_SCR_8K_SIZE + SCR_EXTRA_SCR_4K_0_SIZE + SCR_EXTRA_SCR_4K_2_SIZE)

#define HEAACV2_MAX_SIZE                                                          \
  (max(SCR_BASE_SCR_8K_SIZE + SCR_EXTRA_SCR_4K_0_SIZE + SCR_EXTRA_SCR_4K_2_SIZE + \
           SCR_INTER_SCR_SIZE + SCR_COUP_CH_OUT_SIZE,                             \
       MPS_SCRATCH_MEM_SIZE))
#define ELDV2_MAX_SIZE                                                            \
  (max(SCR_BASE_SCR_8K_SIZE + SCR_EXTRA_SCR_4K_0_SIZE + SCR_EXTRA_SCR_4K_2_SIZE + \
           SCR_EXTRA_SCR_4K_3_SIZE + SCR_INTER_SCR_SIZE + SCR_COUP_CH_OUT_SIZE,   \
       MPS_SCRATCH_MEM_SIZE))
#define LD_MAX_SIZE                                                                          \
  (max(SCR_BASE_SCR_8K_SIZE + SCR_EXTRA_SCR_4K_0_SIZE + SCR_EXTRA_SCR_4K_2_SIZE +            \
           SCR_OUT_DATA_SIZE + SCR_IN_DATA_SIZE + SCR_INTER_SCR_SIZE + SCR_COUP_CH_OUT_SIZE, \
       MPS_SCRATCH_MEM_SIZE))

#define MAX_SCR_SIZE (max(max(HEAACV2_MAX_SIZE, ELDV2_MAX_SIZE), LD_MAX_SIZE))

IA_ERRORCODE ixheaacd_dec_mem_api(ia_exhaacplus_dec_api_struct *p_obj_exhaacplus_dec,
                                  WORD32 i_cmd, WORD32 i_idx, VOID *pv_value) {
  pUWORD32 pui_value = pv_value;

  if (i_idx < 0 || i_idx >= IA_ENHAACPDEC_NUM_MEMTABS) {
    return IA_XHEAAC_DEC_API_FATAL_INVALID_MEMTAB_INDEX;
  }

  if (i_cmd == IA_API_CMD_SET_MEM_PTR) {
    if (pv_value == 0) {
      return (IA_XHEAAC_DEC_API_FATAL_MEM_ALLOC);
    }
    if (((SIZE_T)pv_value %
         p_obj_exhaacplus_dec->p_mem_info_aac[i_idx].ui_alignment) != 0) {
      return (IA_XHEAAC_DEC_API_FATAL_MEM_ALIGN);
    }
    p_obj_exhaacplus_dec->pp_mem_aac[i_idx] = pv_value;
    memset(p_obj_exhaacplus_dec->pp_mem_aac[i_idx], 0,
           p_obj_exhaacplus_dec->p_mem_info_aac[i_idx].ui_size);

    if (i_idx == IA_ENHAACPLUS_DEC_PERSIST_IDX) {
      pUWORD8 p_temp = pv_value;
      UWORD32 *meminfo =
          (UWORD32 *)p_obj_exhaacplus_dec->p_mem_info_aac + i_idx;
      UWORD32 pers_size = meminfo[0];
      p_temp = p_temp + pers_size -
               (IXHEAAC_GET_SIZE_ALIGNED(sizeof(ia_dec_data_struct), BYTE_ALIGN_8) +
                IXHEAAC_GET_SIZE_ALIGNED(sizeof(ia_audio_specific_config_struct), BYTE_ALIGN_8) +
                IXHEAAC_GET_SIZE_ALIGNED(MAX_HEADER_BUF_SIZE, BYTE_ALIGN_8));
      p_obj_exhaacplus_dec->p_state_aac = pv_value;
      memset(p_obj_exhaacplus_dec->p_state_aac, 0,
             IXHEAAC_GET_SIZE_ALIGNED(sizeof(ia_aac_dec_state_struct), BYTE_ALIGN_8));
      p_obj_exhaacplus_dec->p_state_aac->pstr_dec_data = p_temp;
      p_obj_exhaacplus_dec->p_state_aac->ia_audio_specific_config =
          p_temp + IXHEAAC_GET_SIZE_ALIGNED(sizeof(ia_dec_data_struct), BYTE_ALIGN_8);
      p_obj_exhaacplus_dec->p_state_aac->header_ptr =
          p_temp + IXHEAAC_GET_SIZE_ALIGNED(sizeof(ia_dec_data_struct), BYTE_ALIGN_8) +
          IXHEAAC_GET_SIZE_ALIGNED(sizeof(ia_audio_specific_config_struct), BYTE_ALIGN_8);
    }

  } else {
    UWORD32 *meminfo =
        (UWORD32 *)(p_obj_exhaacplus_dec->p_mem_info_aac + i_idx);
    *pui_value = *(meminfo + (i_cmd - IA_API_CMD_GET_MEM_INFO_SIZE));
  }

  return IA_NO_ERROR;
}

static PLATFORM_INLINE VOID
ixheaacd_init_sbr_tables(ia_sbr_tables_struct *ptr_sbr_tables) {
  ptr_sbr_tables->env_calc_tables_ptr =
      (ia_env_calc_tables_struct *)&ixheaacd_aac_dec_env_calc_tables;
  ptr_sbr_tables->qmf_dec_tables_ptr =
      (ia_qmf_dec_tables_struct *)&ixheaacd_aac_qmf_dec_tables;
  ptr_sbr_tables->env_extr_tables_ptr =
      (ia_env_extr_tables_struct *)&ixheaacd_aac_dec_env_extr_tables;
  ptr_sbr_tables->ps_tables_ptr =
      (ia_ps_tables_struct *)&ixheaacd_aac_dec_ps_tables;
}

VOID ixheaacd_updatebytesconsumed(
    ia_aac_dec_state_struct *p_state_enhaacplus_dec,
    struct ia_bit_buf_struct *it_bit_buff) {
  p_state_enhaacplus_dec->i_bytes_consumed =
      (WORD32)(it_bit_buff->ptr_read_next - it_bit_buff->ptr_bit_buf_base);
  if ((p_state_enhaacplus_dec->i_bytes_consumed == 0) &&
      (it_bit_buff->cnt_bits == 0)) {
    p_state_enhaacplus_dec->i_bytes_consumed =
        p_state_enhaacplus_dec->ui_in_bytes;
  }
  if (it_bit_buff->cnt_bits < 0) {
    p_state_enhaacplus_dec->i_bytes_consumed = 0;
    p_state_enhaacplus_dec->ui_out_bytes = 0;
    p_state_enhaacplus_dec->ui_mps_out_bytes = 0;
    p_state_enhaacplus_dec->b_n_raw_data_blk = 0;
  }
}

static VOID copy_qmf_to_ldmps(ia_mps_dec_state_struct *mps_dec_handle,
                              VOID *sbr_persistent_mem_v) {
  ia_sbr_pers_struct *sbr_persistent_mem =
      (ia_sbr_pers_struct *)sbr_persistent_mem_v;
  memcpy(&mps_dec_handle->str_mps_qmf_bank,
         &sbr_persistent_mem->str_sbr_dec_inst.pstr_sbr_channel[0]
              ->str_sbr_dec.str_codec_qmf_bank,
         sizeof(ia_sbr_qmf_filter_bank_struct));

  mps_dec_handle->sbr_tables_ptr =
      sbr_persistent_mem->str_sbr_dec_inst.pstr_sbr_tables;
  mps_dec_handle->qmf_dec_tables_ptr =
      sbr_persistent_mem->str_sbr_dec_inst.pstr_sbr_tables->qmf_dec_tables_ptr;
  mps_dec_handle->str_sbr_scale_fact =
      &sbr_persistent_mem->str_sbr_dec_inst.pstr_sbr_channel[0]
           ->str_sbr_dec.str_sbr_scale_fact;
}

WORD32 ixheaacd_readifadts(ia_aac_dec_state_struct *p_state_enhaacplus_dec,
                           struct ia_bit_buf_struct *it_bit_buff,
                           ia_adts_header_struct *adts) {
  WORD error;

  if ((error = ixheaacd_find_syncword(adts, it_bit_buff)) != 0) {
    ixheaacd_updatebytesconsumed(p_state_enhaacplus_dec, it_bit_buff);
    return IA_XHEAAC_DEC_EXE_NONFATAL_ADTS_SYNC_LOST;
  }
  if ((error = ixheaacd_check_if_adts(
           adts, it_bit_buff,
           p_state_enhaacplus_dec->p_config->ui_max_channels)) != 0) {
    p_state_enhaacplus_dec->i_bytes_consumed = 1;

    if (it_bit_buff->cnt_bits < 0) {
      p_state_enhaacplus_dec->i_bytes_consumed = 0;
      p_state_enhaacplus_dec->ui_out_bytes = 0;
      p_state_enhaacplus_dec->ui_mps_out_bytes = 0;
      error = IA_XHEAAC_DEC_EXE_NONFATAL_INSUFFICIENT_INPUT_BYTES;
      return error;
    }
    return IA_XHEAAC_DEC_EXE_NONFATAL_ADTS_SYNC_LOST;
  }
  p_state_enhaacplus_dec->b_n_raw_data_blk =
      (WORD8)(adts->no_raw_data_blocks + 1);
  return 0;
}

static VOID ixheaacd_allocate_aac_scr(
    ia_aac_dec_scratch_struct *aac_scratch_struct, VOID *base_scratch_ptr,
    VOID *output_ptr, WORD channel, WORD max_channel,
    WORD32 audio_object_type) {
  WORD32 scratch_used = 0;
  aac_scratch_struct->base_scr_8k = base_scratch_ptr;
  aac_scratch_struct->extra_scr_4k[1] = (WORD8 *)base_scratch_ptr;
  scratch_used += SCR_BASE_SCR_8K_SIZE;
  if (channel == 1) {
    aac_scratch_struct->extra_scr_4k[0] = (WORD8 *)base_scratch_ptr + scratch_used;
    scratch_used += SCR_EXTRA_SCR_4K_0_SIZE;
    aac_scratch_struct->extra_scr_4k[2] = (WORD8 *)base_scratch_ptr + scratch_used;
    scratch_used += SCR_EXTRA_SCR_4K_2_SIZE;
  } else {
    aac_scratch_struct->extra_scr_4k[0] = output_ptr;

    if (max_channel > 2) {
      aac_scratch_struct->extra_scr_4k[0] = (WORD8 *)base_scratch_ptr + scratch_used;
      scratch_used += SCR_EXTRA_SCR_4K_0_SIZE;
    }
    aac_scratch_struct->extra_scr_4k[2] = (WORD8 *)base_scratch_ptr + scratch_used;
    scratch_used += SCR_EXTRA_SCR_4K_2_SIZE;
  }

  if (audio_object_type == AOT_ER_AAC_ELD || audio_object_type == AOT_ER_AAC_LD) {
    aac_scratch_struct->extra_scr_4k[0] = (WORD8 *)base_scratch_ptr + scratch_used;
    scratch_used += SCR_EXTRA_SCR_4K_0_SIZE;

    aac_scratch_struct->extra_scr_4k[2] = (WORD8 *)base_scratch_ptr + scratch_used;
    scratch_used += SCR_EXTRA_SCR_4K_2_SIZE;

    aac_scratch_struct->extra_scr_4k[3] = (WORD8 *)base_scratch_ptr + scratch_used;
    scratch_used += SCR_EXTRA_SCR_4K_3_SIZE;
  }
  if ((audio_object_type == AOT_ER_AAC_LD) || (audio_object_type == AOT_AAC_LTP)) {
    aac_scratch_struct->out_data = (WORD32 *)((WORD8 *)base_scratch_ptr + scratch_used);
    scratch_used += SCR_OUT_DATA_SIZE;

    aac_scratch_struct->in_data = (WORD32 *)((WORD8 *)base_scratch_ptr + scratch_used);
    scratch_used += SCR_IN_DATA_SIZE;
  }
}

VOID ixheaacd_allocate_sbr_scr(ia_sbr_scr_struct *sbr_scratch_struct, VOID *base_scratch_ptr,
                               VOID *output_ptr, WORD32 total_channels, WORD8 *p_qshift_arr,
                               UWORD8 slot_pos, UWORD8 num_ch) {
  WORD32 j, i;
  sbr_scratch_struct->ptr_work_buf_core = base_scratch_ptr;

  if (p_qshift_arr != NULL && *p_qshift_arr != LD_OBJ) {
    WORD32 *tmp_buf = (WORD32 *)output_ptr;

    for (j = 1; j < num_ch; j++) {
      if ((*p_qshift_arr + j) == 0)
        *(p_qshift_arr + j) = *(p_qshift_arr + j - 1);
    }

    if (total_channels > 2) {
      for (j = 0; j < num_ch; j++) {
        for (i = 0; i < 1024; i++) {
          *((WORD16 *)tmp_buf + slot_pos + total_channels * i + j) =
              ixheaac_round16(ixheaac_shl32_sat(
                  *(tmp_buf + slot_pos + total_channels * i + j),
                  *(p_qshift_arr + j)));
        }
      }
    } else {
      for (j = 0; j < num_ch; j++) {
        for (i = 0; i < 1024; i++) {
          *((WORD16 *)tmp_buf + total_channels * i + j) =
              ixheaac_round16(ixheaac_shl32_sat(
                  *(tmp_buf + total_channels * i + j), *(p_qshift_arr + j)));
        }
      }
    }
  }
}

VOID ixheaacd_get_lib_id_strings(pVOID pv_output) {
  ia_lib_info_struct *pstr_lib_info = (ia_lib_info_struct *)pv_output;

  pstr_lib_info->p_lib_name = (WORD8 *)LIBNAME;
  pstr_lib_info->p_version_num = (WORD8 *)xHE_AAC_DEC_ITTIAM_VER;

  return;
}

IA_ERRORCODE ixheaacd_dec_api(pVOID p_ia_xheaac_dec_obj, WORD32 i_cmd,
                              WORD32 i_idx, pVOID pv_value) {
  ia_exhaacplus_dec_api_struct *p_obj_exhaacplus_dec = p_ia_xheaac_dec_obj;
  pUWORD32 pui_value = pv_value;
  pWORD32 pui_value_signed = pv_value;
  pWORD8 pb_value = pv_value;
  pVOID *pp_value = (pVOID *)pv_value;
  float *pf_value = pv_value;

  if ((i_cmd != IA_API_CMD_GET_API_SIZE) &&
      (i_cmd != IA_API_CMD_GET_LIB_ID_STRINGS)) {
    if (p_ia_xheaac_dec_obj == 0) {
      return (IA_XHEAAC_DEC_API_FATAL_MEM_ALLOC);
    }
    if (((SIZE_T)p_ia_xheaac_dec_obj & 3) != 0) {
      return (IA_XHEAAC_DEC_API_FATAL_MEM_ALIGN);
    }
  }

  switch (i_cmd) {
    case IA_API_CMD_GET_MEM_INFO_SIZE:
    case IA_API_CMD_GET_MEM_INFO_ALIGNMENT:
    case IA_API_CMD_GET_MEM_INFO_TYPE:
    case IA_API_CMD_SET_MEM_PTR: {
      return ixheaacd_dec_mem_api(p_ia_xheaac_dec_obj, i_cmd, i_idx,
                                  pv_value);
    }

    case IA_API_CMD_GET_TABLE_INFO_SIZE:
    case IA_API_CMD_GET_TABLE_INFO_ALIGNMENT:
    case IA_API_CMD_SET_TABLE_PTR:
    case IA_API_CMD_GET_TABLE_PTR: {
      return ixheaacd_dec_table_api(p_ia_xheaac_dec_obj, i_cmd, i_idx,
                                    pv_value);
    }
  };

  switch (i_cmd) {
    case IA_API_CMD_GET_LIB_ID_STRINGS: {
      WORD8 *i1_ver;
      WORD8 ver_char;

      if (i_idx == IA_CMD_TYPE_LIB_NAME)
        i1_ver = (WORD8 *)LIBNAME;
      else if (i_idx == IA_CMD_TYPE_LIB_VERSION)
        i1_ver = (WORD8 *)xHE_AAC_DEC_ITTIAM_VER;
      else
        return IA_XHEAAC_DEC_API_FATAL_INVALID_LIB_ID_STRINGS_IDX;

      ver_char = *i1_ver++;

      for (; ver_char != '\0';) {
        if (ver_char != '$') {
          *pb_value++ = ver_char;
        }
        ver_char = *i1_ver++;
      }
      *pb_value = ver_char;

      break;
    }
    case IA_API_CMD_GET_API_SIZE: {
      *pui_value = sizeof(ia_exhaacplus_dec_api_struct);
      break;
    }
    case IA_API_CMD_INIT: {
      switch (i_idx) {
        case IA_CMD_TYPE_INIT_API_PRE_CONFIG_PARAMS: {
          memset(p_obj_exhaacplus_dec, 0, sizeof(*p_obj_exhaacplus_dec));
          p_obj_exhaacplus_dec->aac_config.ui_pcm_wdsz = 16;
          p_obj_exhaacplus_dec->aac_config.flag_downmix = 0;
          p_obj_exhaacplus_dec->aac_config.flag_08khz_out = 0;
          p_obj_exhaacplus_dec->aac_config.flag_16khz_out = 0;
          p_obj_exhaacplus_dec->aac_config.flag_to_stereo = 0;
          p_obj_exhaacplus_dec->aac_config.down_sample_flag = 0;
          p_obj_exhaacplus_dec->aac_config.header_dec_done = 0;
          p_obj_exhaacplus_dec->aac_config.frame_status = 1;
          p_obj_exhaacplus_dec->aac_config.ui_mp4_flag = 0;
          p_obj_exhaacplus_dec->aac_config.ui_disable_sync = 0;
          p_obj_exhaacplus_dec->aac_config.ui_auto_sbr_upsample = 1;
          p_obj_exhaacplus_dec->aac_config.ui_samp_freq = 0;
          p_obj_exhaacplus_dec->aac_config.ui_frame_size = 0;

          p_obj_exhaacplus_dec->aac_config.ui_n_channels = 2;
          p_obj_exhaacplus_dec->aac_config.i_channel_mask = 3;
          p_obj_exhaacplus_dec->aac_config.ui_channel_mode = 3;
          p_obj_exhaacplus_dec->aac_config.ui_sbr_mode = 0;
          p_obj_exhaacplus_dec->aac_config.ui_effect_type = 0;
          p_obj_exhaacplus_dec->aac_config.ui_target_loudness = -24;
          p_obj_exhaacplus_dec->aac_config.ui_loud_norm_flag = 0;
          p_obj_exhaacplus_dec->aac_config.ui_hq_esbr = 0;
          p_obj_exhaacplus_dec->aac_config.ui_enh_sbr = 1;
          p_obj_exhaacplus_dec->aac_config.ui_enh_sbr_ps = 0;
          p_obj_exhaacplus_dec->aac_config.ui_pce_found_in_hdr = 0;
          p_obj_exhaacplus_dec->aac_config.loas_present = 0;
          p_obj_exhaacplus_dec->aac_config.ld_decoder = 0;
          p_obj_exhaacplus_dec->aac_config.ui_drc_boost = 100;
          p_obj_exhaacplus_dec->aac_config.ui_drc_cut = 100;
          p_obj_exhaacplus_dec->aac_config.ui_drc_mode_cut = 0;
          p_obj_exhaacplus_dec->aac_config.ui_drc_mode_boost = 0;
          p_obj_exhaacplus_dec->aac_config.ui_drc_target_level = 108;
          p_obj_exhaacplus_dec->aac_config.ui_drc_set = 0;
          p_obj_exhaacplus_dec->aac_config.ui_flush_cmd = 0;
          p_obj_exhaacplus_dec->aac_config.output_level = -1;
#ifdef LOUDNESS_LEVELING_SUPPORT
          p_obj_exhaacplus_dec->aac_config.ui_loudness_leveling_flag = 1;
#endif
          p_obj_exhaacplus_dec->aac_config.ui_max_channels = 6;

          p_obj_exhaacplus_dec->aac_config.ui_coupling_channel = 0;
          p_obj_exhaacplus_dec->aac_config.downmix = 0;

          p_obj_exhaacplus_dec->aac_config.ui_err_conceal = 0;

          {
            ia_aac_dec_tables_struct *pstr_aac_tables =
                &p_obj_exhaacplus_dec->aac_tables;
            pstr_aac_tables->pstr_huffmann_tables =
                (ia_aac_dec_huffman_tables_struct
                     *)&ixheaacd_aac_huffmann_tables;
            pstr_aac_tables->pstr_block_tables =
                (ia_aac_dec_block_tables_struct *)&ixheaacd_aac_block_tables;
            pstr_aac_tables->pstr_imdct_tables =
                (ia_aac_dec_imdct_tables_struct *)&ixheaacd_imdct_tables;

            ixheaacd_huff_tables_create(pstr_aac_tables);
          }
          ixheaacd_init_sbr_tables(&p_obj_exhaacplus_dec->str_sbr_tables);
          p_obj_exhaacplus_dec->common_tables =
              (ixheaacd_misc_tables *)&ixheaacd_str_fft_n_transcendent_tables;
          p_obj_exhaacplus_dec->aac_config.ui_qmf_bands = 64;

          break;
        }
        case IA_CMD_TYPE_INIT_API_POST_CONFIG_PARAMS: {
          ixheaacd_fill_aac_mem_tables(p_obj_exhaacplus_dec);
          break;
        }
        case IA_CMD_TYPE_INIT_PROCESS: {
          WORD32 err_code = 0;
          if (p_obj_exhaacplus_dec->p_state_aac->fatal_err_present) {
            err_code = IA_FATAL_ERROR;
          } else {
            err_code = ixheaacd_dec_init(p_obj_exhaacplus_dec);
            if (err_code && p_obj_exhaacplus_dec->p_state_aac->s_adts_hdr_present) {
              p_obj_exhaacplus_dec->p_state_aac->header_dec_done = 0;
            }
            if (p_obj_exhaacplus_dec->aac_config.ui_err_conceal && err_code) {
              if (err_code & IA_FATAL_ERROR) {
                err_code = IA_XHEAAC_DEC_INIT_FATAL_EC_INIT_FAIL;
              } else {
                err_code = IA_XHEAAC_DEC_INIT_NONFATAL_EC_INIT_FAIL;
              }
            }
          }
          if (err_code != 0) {
            if (err_code < 0)
              p_obj_exhaacplus_dec->p_state_aac->fatal_err_present = 1;
            p_obj_exhaacplus_dec->p_state_aac->i_bytes_consumed =
                p_obj_exhaacplus_dec->p_state_aac->ui_in_bytes;
          }
          return err_code;
          break;
        }
        case IA_CMD_TYPE_INIT_DONE_QUERY: {
          if (p_obj_exhaacplus_dec->p_state_aac->ui_init_done == 1) {
            *pui_value = 1;
          } else {
            *pui_value = 0;
          }
          break;
        }

        case IA_CMD_TYPE_GA_HDR: {
          return ixheaacd_decoder_2_ga_hdr(p_obj_exhaacplus_dec);
          break;
        }

        case IA_CMD_TYPE_FLUSH_MEM: {
          return ixheaacd_decoder_flush_api(p_obj_exhaacplus_dec);
          break;
        }

        default: {
          return IA_XHEAAC_DEC_API_NONFATAL_CMD_TYPE_NOT_SUPPORTED;
        }
      };
      break;
    }
    case IA_API_CMD_SET_CONFIG_PARAM: {
      switch (i_idx) {
        case IA_XHEAAC_DEC_CONFIG_PARAM_SAMP_FREQ: {
          if ((*pui_value < 8000) || (*pui_value > 96000)) {
            return (IA_XHEAAC_DEC_CONFIG_FATAL_INVALID_SAMPLE_RATE);
          }
          p_obj_exhaacplus_dec->aac_config.ui_samp_freq = *pui_value;
          break;
        }
        case IA_XHEAAC_DEC_CONFIG_PARAM_FRAMELENGTH_FLAG: {
          if ((*pui_value != 1) && (*pui_value != 0)) {
            return (IA_XHEAAC_DEC_CONFIG_NONFATAL_INVALID_FRAMELENGTHFLAG);
          }
          p_obj_exhaacplus_dec->aac_config.ui_frame_size = *pui_value;
          break;
        }
        case IA_XHEAAC_DEC_CONFIG_PARAM_PCM_WDSZ: {
          if ((*pui_value != 16) && (*pui_value != 24)) {
            p_obj_exhaacplus_dec->aac_config.ui_pcm_wdsz = 16;
            return (IA_XHEAAC_DEC_CONFIG_NONFATAL_INVALID_PCM_WDSZ);
          }
          p_obj_exhaacplus_dec->aac_config.ui_pcm_wdsz = *pui_value;
          break;
        }
        case IA_XHEAAC_DEC_CONFIG_PARAM_DOWNMIX: {
          if ((*pui_value != 1) && (*pui_value != 0)) {
            p_obj_exhaacplus_dec->aac_config.flag_downmix = 0;
            return (IA_XHEAAC_DEC_CONFIG_NONFATAL_INVALID_DOWNMIX);
          }
          p_obj_exhaacplus_dec->aac_config.flag_downmix = *pui_value;
          p_obj_exhaacplus_dec->aac_config.downmix = *pui_value;
          break;
        }

        case IA_XHEAAC_DEC_CONFIG_PARAM_TOSTEREO: {
          if ((*pui_value != 1) && (*pui_value != 0)) {
            p_obj_exhaacplus_dec->aac_config.flag_to_stereo = 1;
            return (IA_XHEAAC_DEC_CONFIG_NONFATAL_INVALID_TOSTEREO);
          }
          p_obj_exhaacplus_dec->aac_config.flag_to_stereo = *pui_value;
          break;
        }
        case IA_XHEAAC_DEC_CONFIG_PARAM_DSAMPLE: {
          if ((*pui_value != 1) && (*pui_value != 0)) {
            p_obj_exhaacplus_dec->aac_config.down_sample_flag = 0;
            return (IA_XHEAAC_DEC_CONFIG_NONFATAL_INVALID_DSAMPLE);
          }
          p_obj_exhaacplus_dec->aac_config.down_sample_flag = *pui_value;
          break;
        }

        case IA_XHEAAC_DEC_CONFIG_PARAM_MP4FLAG: {
          if ((*pui_value != 1) && (*pui_value != 0)) {
            p_obj_exhaacplus_dec->aac_config.ui_mp4_flag = 0;
            return (IA_XHEAAC_DEC_CONFIG_NONFATAL_INVALID_MP4FLAG);
          }
          p_obj_exhaacplus_dec->aac_config.ui_mp4_flag = *pui_value;
          break;
        }
        case IA_XHEAAC_DEC_CONFIG_PARAM_DRC_CUT: {
          p_obj_exhaacplus_dec->aac_config.ui_drc_set = 1;
          if (*pf_value > 1 || *pf_value < 0) {
            p_obj_exhaacplus_dec->aac_config.ui_drc_cut = 0;
            return (IA_XHEAAC_DEC_CONFIG_NONFATAL_INVALID_DRC_CUT);
          }

          p_obj_exhaacplus_dec->aac_config.ui_drc_mode_cut = 1;
          p_obj_exhaacplus_dec->aac_config.ui_drc_cut =
              (WORD32)((*pf_value) * 100);
          break;
        }

        case IA_XHEAAC_DEC_CONFIG_PARAM_DRC_BOOST: {
          p_obj_exhaacplus_dec->aac_config.ui_drc_set = 1;
          if (*pf_value > 1 || *pf_value < 0) {
            p_obj_exhaacplus_dec->aac_config.ui_drc_boost = 0;
            return (IA_XHEAAC_DEC_CONFIG_NONFATAL_INVALID_DRC_BOOST);
          }

          p_obj_exhaacplus_dec->aac_config.ui_drc_mode_boost = 1;
          p_obj_exhaacplus_dec->aac_config.ui_drc_boost =
              (WORD32)((*pf_value) * 100);
          break;
        }

        case IA_XHEAAC_DEC_CONFIG_PARAM_DRC_TARGET_LEVEL: {
          p_obj_exhaacplus_dec->aac_config.ui_drc_set = 1;
          p_obj_exhaacplus_dec->aac_config.i_loud_ref_level = *pui_value_signed;
          if (*pui_value > 127) {
            p_obj_exhaacplus_dec->aac_config.ui_drc_target_level = 108;
            return (IA_XHEAAC_DEC_CONFIG_NONFATAL_INVALID_DRC_TARGET);
          }
          p_obj_exhaacplus_dec->aac_config.ui_drc_target_level = *pui_value;
          break;
        }
        case IA_XHEAAC_DEC_CONFIG_PARAM_DRC_HEAVY_COMP: {
          p_obj_exhaacplus_dec->aac_config.ui_drc_set = 1;
          if ((*pui_value != 1) && (*pui_value != 0)) {
            p_obj_exhaacplus_dec->aac_config.ui_drc_heavy_comp = 0;
            return (IA_XHEAAC_DEC_CONFIG_NONFATAL_INVALID_DRCFLAG);
          }
          p_obj_exhaacplus_dec->aac_config.ui_drc_heavy_comp = *pui_value;
          break;
        }
        case IA_XHEAAC_DEC_CONFIG_DISABLE_SYNC: {
          if ((*pui_value != 1) && (*pui_value != 0)) {
            p_obj_exhaacplus_dec->aac_config.ui_disable_sync = 0;
            return (IA_XHEAAC_DEC_CONFIG_NONFATAL_INVALID_SYNCFLAG);
          }
          p_obj_exhaacplus_dec->aac_config.ui_disable_sync = *pui_value;
          break;
        }
        case IA_XHEAAC_DEC_CONFIG_PARAM_AUTO_SBR_UPSAMPLE: {
          if ((*pui_value != 1) && (*pui_value != 0)) {
            p_obj_exhaacplus_dec->aac_config.ui_auto_sbr_upsample = 0;
            return (IA_XHEAAC_DEC_CONFIG_NONFATAL_INVALID_SBRUPFLAG);
          }
          p_obj_exhaacplus_dec->aac_config.ui_auto_sbr_upsample = *pui_value;
          break;
        }
        case IA_XHEAAC_DEC_CONFIG_PARAM_MAX_CHANNEL: {
          if (*pui_value > 8) {
            p_obj_exhaacplus_dec->aac_config.ui_max_channels = 8;
            return (IA_XHEAAC_DEC_CONFIG_NONFATAL_INVALID_MAX_CHANNEL);
          }
          if (*pui_value < 2) {
            p_obj_exhaacplus_dec->aac_config.ui_max_channels = 2;
            return (IA_XHEAAC_DEC_CONFIG_NONFATAL_INVALID_MAX_CHANNEL);
          }
          p_obj_exhaacplus_dec->aac_config.ui_max_channels = *pui_value;
          break;
        }
        case IA_XHEAAC_DEC_CONFIG_PARAM_FRAMESIZE: {
          if (*pui_value == 1) {
            p_obj_exhaacplus_dec->aac_config.framesize_480 = 1;
          } else if (*pui_value == 0) {
            p_obj_exhaacplus_dec->aac_config.framesize_480 = 0;
          } else {
            return (IA_XHEAAC_DEC_CONFIG_NONFATAL_INVALID_FRAMSZ);
          }
          break;
        }

        case IA_XHEAAC_DEC_CONFIG_PARAM_LD_TESTING: {
          if (*pui_value == 1) {
            p_obj_exhaacplus_dec->aac_config.ld_decoder = 1;
          } else if (*pui_value == 0) {
            p_obj_exhaacplus_dec->aac_config.ld_decoder = 0;
          } else {
            return (IA_XHEAAC_DEC_CONFIG_NONFATAL_INVALID_LD_CONFIG);
          }
          break;
        }

        case IA_XHEAAC_DEC_CONFIG_PARAM_COUP_CHANNEL: {
          if (*pui_value > 16) {
            p_obj_exhaacplus_dec->aac_config.ui_coupling_channel = 1;
            return (IA_XHEAAC_DEC_CONFIG_NONFATAL_INVALID_COUP_CHANNEL);
          }
          p_obj_exhaacplus_dec->aac_config.ui_coupling_channel = *pui_value;
          break;
        }

        case IA_XHEAAC_DEC_CONFIG_PARAM_DOWNMIX_STEREO: {
          if ((*pui_value != 1) && (*pui_value != 0)) {
            p_obj_exhaacplus_dec->aac_config.downmix = 0;
            return (IA_XHEAAC_DEC_CONFIG_NONFATAL_INVALID_DOWNMIX_STEREO);
          }
          p_obj_exhaacplus_dec->aac_config.downmix = *pui_value;
          break;
        }

        case IA_XHEAAC_DEC_CONFIG_PARAM_DRC_EFFECT_TYPE: {
          if (((*pui_value_signed) > 8) || ((*pui_value_signed) < -1)) {
            p_obj_exhaacplus_dec->aac_config.ui_effect_type = -1;
            return (IA_XHEAAC_DEC_CONFIG_NONFATAL_INVALID_EFFECT_TYPE);
          }
          p_obj_exhaacplus_dec->aac_config.ui_effect_type = *pui_value_signed;
          break;
        }
        case IA_XHEAAC_DEC_CONFIG_PARAM_DRC_TARGET_LOUDNESS: {
          if (*pui_value_signed >= 0) {
            p_obj_exhaacplus_dec->aac_config.ui_loud_norm_flag = 1;
          }
          *pui_value_signed = -(*pui_value_signed >> 2);
          if (((*pui_value_signed) > 0) || ((*pui_value_signed) < -63)) {
            p_obj_exhaacplus_dec->aac_config.ui_target_loudness = 0;
            return (IA_XHEAAC_DEC_CONFIG_NONFATAL_INVALID_TARGET_LOUDNESS);
          }
          p_obj_exhaacplus_dec->aac_config.ui_target_loudness =
              *pui_value_signed;
          break;
        }
        case IA_XHEAAC_DEC_CONFIG_PARAM_HQ_ESBR: {
          if (((*pui_value_signed) != 0) && ((*pui_value_signed) != 1)) {
            p_obj_exhaacplus_dec->aac_config.ui_hq_esbr = 0;
            return (IA_XHEAAC_DEC_CONFIG_NONFATAL_INVALID_ESBR_HQ_FLAG);
          }
          p_obj_exhaacplus_dec->aac_config.ui_hq_esbr = *pui_value_signed;
          break;
        }
        case IA_XHEAAC_DEC_CONFIG_PARAM_PS_ENABLE: {
          if (((*pui_value_signed) != 0) && ((*pui_value_signed) != 1)) {
            p_obj_exhaacplus_dec->aac_config.ui_enh_sbr_ps = 0;
            return (IA_XHEAAC_DEC_CONFIG_NONFATAL_INVALID_ESBR_PS_FLAG);
          }
          p_obj_exhaacplus_dec->aac_config.ui_enh_sbr_ps = *pui_value_signed;
          break;
        }
        case IA_XHEAAC_DEC_CONFIG_PARAM_PEAK_LIMITER: {
          if (((*pui_value_signed) != 0) && ((*pui_value_signed) != 1)) {
            p_obj_exhaacplus_dec->aac_config.peak_limiter_off = 0;
            return (IA_XHEAAC_DEC_CONFIG_NONFATAL_INVALID_PEAK_LIM_FLAG_TYPE);
          }
          p_obj_exhaacplus_dec->aac_config.peak_limiter_off = *pui_value_signed;
          break;
        }
        case IA_XHEAAC_DEC_CONFIG_ERROR_CONCEALMENT: {
          if (((*pui_value_signed) != 0) && ((*pui_value_signed) != 1)) {
            p_obj_exhaacplus_dec->aac_config.ui_err_conceal = 0;
            return (IA_XHEAAC_DEC_CONFIG_NONFATAL_INVALID_ERR_CONCEAL_FLAG_TYPE);
          }
          p_obj_exhaacplus_dec->aac_config.ui_err_conceal = *pui_value_signed;
          break;
        }
        case IA_XHEAAC_DEC_CONFIG_PARAM_ESBR: {
          if (((*pui_value_signed) != 0) && ((*pui_value_signed) != 1)) {
            p_obj_exhaacplus_dec->aac_config.ui_enh_sbr = 1;
            return (IA_XHEAAC_DEC_CONFIG_NONFATAL_INVALID_ESBR_FLAG);
          }
          p_obj_exhaacplus_dec->aac_config.ui_enh_sbr = *pui_value_signed;
          break;
        }
#ifdef LOUDNESS_LEVELING_SUPPORT
        case IA_XHEAAC_DEC_CONFIG_PARAM_DRC_LOUDNESS_LEVELING: {
          if (((*pui_value_signed) != 0) && ((*pui_value_signed) != 1)) {
            p_obj_exhaacplus_dec->aac_config.ui_loudness_leveling_flag = 1;
            return (IA_XHEAAC_DEC_CONFIG_NONFATAL_INVALID_LOUDNESS_LEVELING_FLAG);
          }
          p_obj_exhaacplus_dec->aac_config.ui_loudness_leveling_flag = *pui_value_signed;
          break;
        }
#endif
        default: { return IA_XHEAAC_DEC_API_FATAL_INVALID_CONFIG_PARAM; }
      }
      break;
    }

    case IA_API_CMD_GET_CONFIG_PARAM: {
      UWORD32 i;
      WORD32 *pvalue =
          (WORD32 *)(&p_obj_exhaacplus_dec->aac_config.ui_pcm_wdsz);
      if (IA_XHEAAC_DEC_CONFIG_PARAM_NUM_CHANNELS == i_idx) {
        if (p_obj_exhaacplus_dec->p_state_aac != NULL &&
            p_obj_exhaacplus_dec->p_state_aac->heaac_mps_handle
                    .heaac_mps_present == 1) {
          *pui_value = p_obj_exhaacplus_dec->p_state_aac->heaac_mps_handle
                           .num_output_channels_at;
        } else {
          *pui_value = pvalue[i_idx];
        }
      } else if (IA_XHEAAC_DEC_CONFIG_PARAM_CHANNEL_MASK == i_idx) {
        if (p_obj_exhaacplus_dec->p_state_aac != NULL &&
            p_obj_exhaacplus_dec->p_state_aac->heaac_mps_handle
                    .heaac_mps_present == 1) {
          *pui_value = p_obj_exhaacplus_dec->p_state_aac->heaac_mps_handle
                           .bs_config.ui_channel_mask;
        } else {
          *pui_value = pvalue[i_idx];
        }
      } else
      if (i_idx >= 0 && i_idx <= 8) {
        *pui_value = pvalue[i_idx];
      } else if (IA_ENHAACPLUS_DEC_CONFIG_GET_NUM_PRE_ROLL_FRAMES == i_idx) {
        WORD32 *ptri_value = (WORD32 *)pv_value;
        ia_audio_specific_config_struct *ptr_audio_specific_config =
            ((ia_audio_specific_config_struct *)
                 p_obj_exhaacplus_dec->p_state_aac->ia_audio_specific_config);

        for (i = 0; i < MAX_AUDIO_PREROLLS + 1; i++) {
          if (ptr_audio_specific_config->str_usac_config.str_usac_dec_config
                  .preroll_bytes[i] == 0) {
            break;
          }
        }

        *ptri_value = i;
      } else if (IA_ENHAACPLUS_DEC_CONFIG_EXT_ELE_PTR == i_idx) {
        ia_audio_specific_config_struct *ptr_audio_specific_config =
            ((ia_audio_specific_config_struct *)
                p_obj_exhaacplus_dec->p_state_aac->ia_audio_specific_config);

        for (i = 0; i < ptr_audio_specific_config->str_usac_config
                            .str_usac_dec_config.num_config_extensions;
             i++) {
          pp_value[i] = ptr_audio_specific_config->str_usac_config
                            .str_usac_dec_config.usac_cfg_ext_info_buf[i];
        }

        for (i = 0; i < ptr_audio_specific_config->str_usac_config
                            .str_usac_dec_config.num_elements;
             i++) {
          if (ptr_audio_specific_config->str_usac_config.str_usac_dec_config
                  .usac_ext_ele_payload_present[i]) {
            pp_value[i + 16] =
                ptr_audio_specific_config->str_usac_config.str_usac_dec_config
                    .usac_ext_ele_payload_buf[i];
          }
        }
      } else if (IA_ENHAACPLUS_DEC_CONFIG_EXT_ELE_BUF_SIZES == i_idx) {
        WORD32 *ptri_value = (WORD32 *)pv_value;
        ia_audio_specific_config_struct *ptr_audio_specific_config =
            ((ia_audio_specific_config_struct *)
                 p_obj_exhaacplus_dec->p_state_aac->ia_audio_specific_config);
        for (i = 0; i < ptr_audio_specific_config->str_usac_config
                            .str_usac_dec_config.num_config_extensions;
             i++) {
          ptri_value[i] = ptr_audio_specific_config->str_usac_config
                              .str_usac_dec_config.usac_cfg_ext_info_len[i];
        }
        for (i = 0; i < ptr_audio_specific_config->str_usac_config
                            .str_usac_dec_config.num_elements;
             i++) {
          ptri_value[i + 16] =
              ptr_audio_specific_config->str_usac_config.str_usac_dec_config
                  .usac_ext_ele_payload_len[i];
        }

      } else if (IA_ENHAACPLUS_DEC_DRC_IS_CONFIG_CHANGED == i_idx) {
        *pui_value = p_obj_exhaacplus_dec->p_state_aac->drc_config_changed;
      } else if (IA_ENHAACPLUS_DEC_DRC_APPLY_CROSSFADE == i_idx) {
        *pui_value = p_obj_exhaacplus_dec->p_state_aac->apply_crossfade;
      } else if (IA_ENHAACPLUS_DEC_CONFIG_NUM_ELE == i_idx) {
        UWORD32 *ptri_value = (UWORD32 *)pv_value;
        ia_audio_specific_config_struct *ptr_audio_specific_config =
            ((ia_audio_specific_config_struct *)
                 p_obj_exhaacplus_dec->p_state_aac->ia_audio_specific_config);
        *ptri_value = ptr_audio_specific_config->str_usac_config
                          .str_usac_dec_config.num_elements;

      } else if (IA_ENHAACPLUS_DEC_CONFIG_NUM_CONFIG_EXT == i_idx) {
        UWORD32 *ptri_value = (UWORD32 *)pv_value;
        ia_audio_specific_config_struct *ptr_audio_specific_config =
            ((ia_audio_specific_config_struct *)
                 p_obj_exhaacplus_dec->p_state_aac->ia_audio_specific_config);
        *ptri_value = ptr_audio_specific_config->str_usac_config
                          .str_usac_dec_config.num_config_extensions;
      } else if (IA_ENHAACPLUS_DEC_CONFIG_GAIN_PAYLOAD_LEN == i_idx) {
        UWORD32 *ptri_value = (UWORD32 *)pv_value;
        ia_audio_specific_config_struct *ptr_audio_specific_config =
            ((ia_audio_specific_config_struct *)
                 p_obj_exhaacplus_dec->p_state_aac->ia_audio_specific_config);
        WORD32 preroll_counter = ptr_audio_specific_config->str_usac_config
                                     .str_usac_dec_config.preroll_counter;
        *ptri_value =
            ptr_audio_specific_config->str_usac_config.str_usac_dec_config
                .usac_ext_gain_payload_len[preroll_counter];
      } else if (IA_ENHAACPLUS_DEC_CONFIG_GAIN_PAYLOAD_BUF == i_idx) {
        WORD32 payload_buffer_offeset = 0;
        ia_audio_specific_config_struct *ptr_audio_specific_config =
            ((ia_audio_specific_config_struct *)
                 p_obj_exhaacplus_dec->p_state_aac->ia_audio_specific_config);
        WORD32 preroll_counter = ptr_audio_specific_config->str_usac_config
                                     .str_usac_dec_config.preroll_counter;
        for (i = 0; i < (UWORD32)preroll_counter; i++)
          payload_buffer_offeset +=
              ptr_audio_specific_config->str_usac_config.str_usac_dec_config
                  .usac_ext_gain_payload_len[i] *
              sizeof(WORD8);
        *pp_value = ptr_audio_specific_config->str_usac_config
                        .str_usac_dec_config.usac_ext_gain_payload_buf +
                    payload_buffer_offeset;
      } else if (IA_XHEAAC_DEC_CONFIG_PARAM_AOT == i_idx) {
        if (p_obj_exhaacplus_dec->p_state_aac != NULL) {
          if (p_obj_exhaacplus_dec->aac_config.ui_mp4_flag == 1) {
            ia_audio_specific_config_struct *ptr_audio_specific_config =
                ((ia_audio_specific_config_struct *)
                    p_obj_exhaacplus_dec->p_state_aac->ia_audio_specific_config);
            *pui_value = ptr_audio_specific_config->audio_object_type;
          } else {
            *pui_value = p_obj_exhaacplus_dec->p_state_aac->audio_object_type;
          }
        } else {
          *pui_value = AOT_AAC_LC;
        }
      }
#ifdef LOUDNESS_LEVELING_SUPPORT
      else if (IA_XHEAAC_DEC_CONFIG_PARAM_DRC_LOUDNESS_LEVELING == i_idx) {
        WORD32 *ui_value =
            (WORD32 *)(&p_obj_exhaacplus_dec->aac_config.ui_loudness_leveling_flag);
        *pui_value = *ui_value;
      }
#endif
      else if (IA_XHEAAC_DEC_CONFIG_PARAM_DRC_CUT == i_idx) {
        UWORD32 *ui_value = (UWORD32 *)(&p_obj_exhaacplus_dec->aac_config.ui_drc_cut);
        *pf_value = (*ui_value) / 100.0f;
      } else if (IA_XHEAAC_DEC_CONFIG_PARAM_DRC_BOOST == i_idx) {
        UWORD32 *ui_value = (UWORD32 *)(&p_obj_exhaacplus_dec->aac_config.ui_drc_boost);
        *pf_value = (*ui_value) / 100.0f;
      } else if (IA_XHEAAC_DEC_CONFIG_PARAM_DRC_MODE_CUT == i_idx) {
        UWORD8 *ui_value = (&p_obj_exhaacplus_dec->aac_config.ui_drc_mode_cut);
        *pb_value = (*ui_value);
      } else if (IA_XHEAAC_DEC_CONFIG_PARAM_DRC_MODE_BOOST == i_idx) {
        UWORD8 *ui_value = (&p_obj_exhaacplus_dec->aac_config.ui_drc_mode_boost);
        *pb_value = *ui_value;
      }
      else {
        return IA_XHEAAC_DEC_API_FATAL_INVALID_CONFIG_PARAM;
      }
      break;
    }

    case IA_API_CMD_GET_MEMTABS_SIZE: {
      *pui_value = (sizeof(ia_mem_info_struct) + sizeof(pVOID *)) *
                   (IA_ENHAACPDEC_NUM_MEMTABS);
      break;
    }
    case IA_API_CMD_GET_LOUDNESS_VAL: {
      *pui_value = p_obj_exhaacplus_dec->aac_config.output_level;
      break;
    }
    case IA_API_CMD_SET_MEMTABS_PTR: {
      if (pv_value == NULL) return IA_XHEAAC_DEC_API_FATAL_MEM_ALLOC;
      memset(pv_value, 0, (sizeof(ia_mem_info_struct) + sizeof(pVOID *)) *
                              (IA_ENHAACPDEC_NUM_MEMTABS));

      p_obj_exhaacplus_dec->p_mem_info_aac = pv_value;
      p_obj_exhaacplus_dec->pp_mem_aac =
          (pVOID *)((WORD8 *)pv_value +
                    sizeof(ia_mem_info_struct) * IA_ENHAACPDEC_NUM_MEMTABS);

      break;
    }
    case IA_API_CMD_GET_N_MEMTABS: {
      *pui_value = IA_ENHAACPDEC_NUM_MEMTABS;
      break;
    }

    case IA_API_CMD_GET_N_TABLES: {
      *pui_value = NUM_AAC_TABLES + NUM_MPS_TABLES;
      break;
    }
    case IA_API_CMD_EXECUTE: {
      switch (i_idx) {
        case IA_CMD_TYPE_DO_EXECUTE: {
          WORD32 err_code = 0;
          if (!p_obj_exhaacplus_dec->p_state_aac->ui_init_done ||
              p_obj_exhaacplus_dec->p_state_aac->fatal_err_present) {
            err_code = IA_FATAL_ERROR;
          } else {
            memset(p_obj_exhaacplus_dec->p_state_aac->qshift_adj, 0,
                   sizeof(p_obj_exhaacplus_dec->p_state_aac->qshift_adj));
            p_obj_exhaacplus_dec->p_state_aac->qshift_cnt = 0;
            err_code = ixheaacd_dec_execute(p_obj_exhaacplus_dec);
          }
          if (err_code != IA_NO_ERROR) {
            if (err_code < 0) {
              p_obj_exhaacplus_dec->p_state_aac->fatal_err_present = 1;
            }
            p_obj_exhaacplus_dec->p_state_aac->i_bytes_consumed =
                p_obj_exhaacplus_dec->p_state_aac->ui_in_bytes;
          }
          return err_code;
          break;
        }
        case IA_CMD_TYPE_DONE_QUERY: {
          if (p_obj_exhaacplus_dec->p_state_aac->ui_input_over == 1) {
            *pui_value = 1;
          } else {
            *pui_value = 0;
          }

          break;
        }
        default: { return IA_XHEAAC_DEC_API_FATAL_INVALID_EXECUTE_TYPE; }
      };
      break;
    }
    case IA_API_CMD_GET_CURIDX_INPUT_BUF: {
      *pui_value = p_obj_exhaacplus_dec->p_state_aac->i_bytes_consumed;
      break;
    }
    case IA_API_CMD_SET_INPUT_BYTES: {
      p_obj_exhaacplus_dec->p_state_aac->ui_in_bytes = *pui_value;
      break;
    }
    case IA_API_CMD_GET_OUTPUT_BYTES: {
      if (1 == i_idx) {
        *pui_value = p_obj_exhaacplus_dec->p_state_aac->ui_mps_out_bytes;
      } else {
        *pui_value = p_obj_exhaacplus_dec->p_state_aac->ui_out_bytes;
      }
      if (p_obj_exhaacplus_dec->p_state_aac->audio_object_type == AOT_USAC) {
        ia_audio_specific_config_struct *ptr_audio_specific_config =
            ((ia_audio_specific_config_struct *)
                 p_obj_exhaacplus_dec->p_state_aac->ia_audio_specific_config);

        WORD32 preroll_counter = ptr_audio_specific_config->str_usac_config
                                     .str_usac_dec_config.preroll_counter;

        *pui_value = ptr_audio_specific_config->str_usac_config
                         .str_usac_dec_config.preroll_bytes[preroll_counter];

        preroll_counter++;
        if (preroll_counter > (MAX_AUDIO_PREROLLS + 1)) return IA_FATAL_ERROR;
        ptr_audio_specific_config->str_usac_config.str_usac_dec_config
            .preroll_counter = preroll_counter;
      }
      break;
    }
    case IA_API_CMD_INPUT_OVER: {
      p_obj_exhaacplus_dec->p_state_aac->ui_input_over = 1;
      break;
    }
    default: { return IA_XHEAAC_DEC_API_FATAL_INVALID_CMD; }
  };
  return IA_NO_ERROR;
}

IA_ERRORCODE ixheaacd_decoder_2_ga_hdr(ia_exhaacplus_dec_api_struct *p_obj_exhaacplus_dec) {
  IA_ERRORCODE err_code = IA_NO_ERROR;
  if (p_obj_exhaacplus_dec->aac_config.ui_flush_cmd == 0) {
    p_obj_exhaacplus_dec->aac_config.ui_pcm_wdsz = 16;
    p_obj_exhaacplus_dec->aac_config.flag_downmix = 0;
    p_obj_exhaacplus_dec->aac_config.flag_08khz_out = 0;
    p_obj_exhaacplus_dec->aac_config.flag_16khz_out = 0;
    p_obj_exhaacplus_dec->aac_config.flag_to_stereo = 0;
    p_obj_exhaacplus_dec->aac_config.down_sample_flag = 0;
    p_obj_exhaacplus_dec->aac_config.header_dec_done = 0;
    p_obj_exhaacplus_dec->aac_config.frame_status = 1;
    p_obj_exhaacplus_dec->aac_config.ui_mp4_flag = 1;
    p_obj_exhaacplus_dec->aac_config.ui_disable_sync = 0;
    p_obj_exhaacplus_dec->aac_config.ui_auto_sbr_upsample = 1;
    p_obj_exhaacplus_dec->aac_config.ui_samp_freq = 0;
    p_obj_exhaacplus_dec->aac_config.ui_channel_mode = 3;
    p_obj_exhaacplus_dec->aac_config.ui_sbr_mode = 2;
    p_obj_exhaacplus_dec->aac_config.ui_pce_found_in_hdr = 0;
    p_obj_exhaacplus_dec->aac_config.loas_present = 0;
    p_obj_exhaacplus_dec->aac_config.ui_drc_boost = 0;
    p_obj_exhaacplus_dec->aac_config.ui_drc_cut = 0;
    p_obj_exhaacplus_dec->aac_config.ui_drc_mode_cut = 0;
    p_obj_exhaacplus_dec->aac_config.ui_drc_mode_boost = 0;
    p_obj_exhaacplus_dec->aac_config.ui_drc_target_level = 108;
    p_obj_exhaacplus_dec->aac_config.ui_drc_set = 0;
    p_obj_exhaacplus_dec->aac_config.ui_flush_cmd = 1;

    p_obj_exhaacplus_dec->aac_config.ui_max_channels = 6;

    p_obj_exhaacplus_dec->aac_config.ui_coupling_channel = 0;
    p_obj_exhaacplus_dec->aac_config.downmix = 0;

    {
      ia_aac_dec_tables_struct *pstr_aac_tables =
          &p_obj_exhaacplus_dec->aac_tables;
      pstr_aac_tables->pstr_huffmann_tables =
          (ia_aac_dec_huffman_tables_struct *)&ixheaacd_aac_huffmann_tables;
      pstr_aac_tables->pstr_block_tables =
          (ia_aac_dec_block_tables_struct *)&ixheaacd_aac_block_tables;
      pstr_aac_tables->pstr_imdct_tables =
          (ia_aac_dec_imdct_tables_struct *)&ixheaacd_imdct_tables;

      ixheaacd_huff_tables_create(pstr_aac_tables);
    }
    ixheaacd_init_sbr_tables(&p_obj_exhaacplus_dec->str_sbr_tables);
    p_obj_exhaacplus_dec->common_tables =
        (ixheaacd_misc_tables *)&ixheaacd_str_fft_n_transcendent_tables;
    p_obj_exhaacplus_dec->aac_config.ui_qmf_bands = 64;
    p_obj_exhaacplus_dec->p_state_aac->ui_init_done = 0;

    err_code = ixheaacd_dec_init(p_obj_exhaacplus_dec);
  } else {
    p_obj_exhaacplus_dec->aac_config.ui_flush_cmd = 0;
    err_code = ixheaacd_dec_init(p_obj_exhaacplus_dec);
  }
  if (err_code && p_obj_exhaacplus_dec->p_state_aac->s_adts_hdr_present) {
    p_obj_exhaacplus_dec->p_state_aac->header_dec_done = 0;
  }
  if (p_obj_exhaacplus_dec->aac_config.ui_err_conceal && err_code) {
    if (err_code & IA_FATAL_ERROR) {
      return IA_XHEAAC_DEC_INIT_FATAL_EC_INIT_FAIL;
    } else {
      return IA_XHEAAC_DEC_INIT_NONFATAL_EC_INIT_FAIL;
    }
  } else {
    return err_code;
  }
}

IA_ERRORCODE ixheaacd_decoder_flush_api(ia_exhaacplus_dec_api_struct *p_obj_exhaacplus_dec) {
  IA_ERRORCODE err_code = IA_NO_ERROR;
  UWORD8 *header_temp_ptr;
  WORD32 header_length;
  if (p_obj_exhaacplus_dec->aac_config.ui_flush_cmd == 0) {
    header_temp_ptr = p_obj_exhaacplus_dec->p_state_aac->header_ptr;
    header_length = p_obj_exhaacplus_dec->p_state_aac->header_length;
    memset(p_obj_exhaacplus_dec->p_state_aac, 0,
           IXHEAAC_GET_SIZE_ALIGNED(sizeof(ia_aac_dec_state_struct), BYTE_ALIGN_8));
    {
      pUWORD8 p_temp = (pUWORD8)p_obj_exhaacplus_dec->p_state_aac;
      UWORD32 *meminfo = (UWORD32 *)p_obj_exhaacplus_dec->p_mem_info_aac;
      UWORD32 pers_size = meminfo[0];
      p_temp = p_temp + pers_size -
               (IXHEAAC_GET_SIZE_ALIGNED(sizeof(ia_dec_data_struct), BYTE_ALIGN_8) +
                IXHEAAC_GET_SIZE_ALIGNED(sizeof(ia_audio_specific_config_struct), BYTE_ALIGN_8) +
                IXHEAAC_GET_SIZE_ALIGNED(MAX_HEADER_BUF_SIZE, BYTE_ALIGN_8));

      p_obj_exhaacplus_dec->p_state_aac->pstr_dec_data = p_temp;
      p_obj_exhaacplus_dec->p_state_aac->ia_audio_specific_config =
          p_temp + IXHEAAC_GET_SIZE_ALIGNED(sizeof(ia_dec_data_struct), BYTE_ALIGN_8);
      p_obj_exhaacplus_dec->p_state_aac->header_ptr =
          p_temp + IXHEAAC_GET_SIZE_ALIGNED(sizeof(ia_dec_data_struct), BYTE_ALIGN_8) +
          IXHEAAC_GET_SIZE_ALIGNED(sizeof(ia_audio_specific_config_struct), BYTE_ALIGN_8);
    }
    memset(&(p_obj_exhaacplus_dec->aac_config), 0,
           sizeof(ia_aac_dec_config_struct));

    p_obj_exhaacplus_dec->aac_config.ui_pcm_wdsz = 16;
    p_obj_exhaacplus_dec->aac_config.flag_downmix = 0;
    p_obj_exhaacplus_dec->aac_config.flag_08khz_out = 0;
    p_obj_exhaacplus_dec->aac_config.flag_16khz_out = 0;
    p_obj_exhaacplus_dec->aac_config.flag_to_stereo = 0;
    p_obj_exhaacplus_dec->aac_config.down_sample_flag = 0;
    p_obj_exhaacplus_dec->aac_config.header_dec_done = 0;
    p_obj_exhaacplus_dec->aac_config.frame_status = 1;
    p_obj_exhaacplus_dec->aac_config.ui_mp4_flag = 1;
    p_obj_exhaacplus_dec->aac_config.ui_disable_sync = 0;
    p_obj_exhaacplus_dec->aac_config.ui_auto_sbr_upsample = 1;
    p_obj_exhaacplus_dec->aac_config.ui_samp_freq = 0;
    p_obj_exhaacplus_dec->aac_config.ui_channel_mode = 3;
    p_obj_exhaacplus_dec->aac_config.ui_sbr_mode = 2;
    p_obj_exhaacplus_dec->aac_config.ui_pce_found_in_hdr = 0;
    p_obj_exhaacplus_dec->aac_config.loas_present = 0;
    p_obj_exhaacplus_dec->aac_config.ui_drc_boost = 0;
    p_obj_exhaacplus_dec->aac_config.ui_drc_cut = 0;
    p_obj_exhaacplus_dec->aac_config.ui_drc_mode_cut = 0;
    p_obj_exhaacplus_dec->aac_config.ui_drc_mode_boost = 0;
    p_obj_exhaacplus_dec->aac_config.ui_drc_target_level = 108;
    p_obj_exhaacplus_dec->aac_config.ui_drc_set = 0;
    p_obj_exhaacplus_dec->aac_config.ui_flush_cmd = 1;

    p_obj_exhaacplus_dec->aac_config.ui_max_channels = 6;

    p_obj_exhaacplus_dec->aac_config.ui_coupling_channel = 0;
    p_obj_exhaacplus_dec->aac_config.downmix = 0;
    p_obj_exhaacplus_dec->p_state_aac->peak_lim_init = 0;

    {
      ia_aac_dec_tables_struct *pstr_aac_tables =
          &p_obj_exhaacplus_dec->aac_tables;
      pstr_aac_tables->pstr_huffmann_tables =
          (ia_aac_dec_huffman_tables_struct *)&ixheaacd_aac_huffmann_tables;
      pstr_aac_tables->pstr_block_tables =
          (ia_aac_dec_block_tables_struct *)&ixheaacd_aac_block_tables;
      pstr_aac_tables->pstr_imdct_tables =
          (ia_aac_dec_imdct_tables_struct *)&ixheaacd_imdct_tables;

      ixheaacd_huff_tables_create(pstr_aac_tables);
    }
    ixheaacd_init_sbr_tables(&p_obj_exhaacplus_dec->str_sbr_tables);
    p_obj_exhaacplus_dec->common_tables =
        (ixheaacd_misc_tables *)&ixheaacd_str_fft_n_transcendent_tables;
    p_obj_exhaacplus_dec->aac_config.ui_qmf_bands = 64;
    p_obj_exhaacplus_dec->p_state_aac->header_ptr = header_temp_ptr;
    p_obj_exhaacplus_dec->p_state_aac->ui_in_bytes = header_length;
    p_obj_exhaacplus_dec->p_state_aac->header_length = header_length;

    err_code = ixheaacd_dec_init(p_obj_exhaacplus_dec);
  } else {
    p_obj_exhaacplus_dec->aac_config.ui_flush_cmd = 0;
    err_code = ixheaacd_dec_init(p_obj_exhaacplus_dec);
  }
  if (err_code && p_obj_exhaacplus_dec->p_state_aac->s_adts_hdr_present) {
    p_obj_exhaacplus_dec->p_state_aac->header_dec_done = 0;
  }
  if (p_obj_exhaacplus_dec->aac_config.ui_err_conceal && err_code) {
    if (err_code & IA_FATAL_ERROR) {
      return IA_XHEAAC_DEC_INIT_FATAL_EC_INIT_FAIL;
    } else {
      return IA_XHEAAC_DEC_INIT_NONFATAL_EC_INIT_FAIL;
    }
  } else {
    return err_code;
  }
}

static PLATFORM_INLINE WORD32
ixheaacd_persistent_buffer_sizes(WORD32 num_channel) {
  WORD32 size_buffers = 0;

  WORD32 temp;
  WORD32 max_channels;

  size_buffers += IXHEAAC_GET_SIZE_ALIGNED(4 * 512 * num_channel * sizeof(WORD32), BYTE_ALIGN_8);

  size_buffers +=
      num_channel * IXHEAAC_GET_SIZE_ALIGNED(ltp_buffer_size * sizeof(WORD16), BYTE_ALIGN_8);

  size_buffers +=
      num_channel * IXHEAAC_GET_SIZE_ALIGNED(sizeof(ia_aac_dec_channel_info), BYTE_ALIGN_8);

  if (num_channel > 2) {
    max_channels = MAX_BS_ELEMENT;
  } else {
    max_channels = 2;
  }
  size_buffers += (max_channels)*2 *
                  IXHEAAC_GET_SIZE_ALIGNED(sizeof(ia_aac_dec_sbr_bitstream_struct), BYTE_ALIGN_8);

  size_buffers +=
      num_channel * IXHEAAC_GET_SIZE_ALIGNED(MAXSBRBYTES * sizeof(WORD8), BYTE_ALIGN_8);

  size_buffers +=
      num_channel * IXHEAAC_GET_SIZE_ALIGNED(MAXSBRBYTES * sizeof(WORD8), BYTE_ALIGN_8);

  size_buffers +=
      2 * num_channel *
      IXHEAAC_GET_SIZE_ALIGNED(
          (QMF_FILTER_STATE_ANA_SIZE + 2 * NO_ANALYSIS_CHANNELS) * sizeof(WORD16), BYTE_ALIGN_8);

  size_buffers +=
      2 * num_channel *
      IXHEAAC_GET_SIZE_ALIGNED(
          (QMF_FILTER_STATE_ANA_SIZE + 2 * NO_ANALYSIS_CHANNELS) * sizeof(WORD32), BYTE_ALIGN_8);

  size_buffers +=
      2 * num_channel *
      IXHEAAC_GET_SIZE_ALIGNED(
          (QMF_FILTER_STATE_SYN_SIZE + 2 * NO_SYNTHESIS_CHANNELS) * sizeof(WORD16), BYTE_ALIGN_8);

  size_buffers +=
      2 * num_channel *
      IXHEAAC_GET_SIZE_ALIGNED(
          (QMF_FILTER_STATE_SYN_SIZE + 2 * NO_SYNTHESIS_CHANNELS) * sizeof(WORD32), BYTE_ALIGN_8);

  size_buffers += num_channel * 2 *
                  IXHEAAC_GET_SIZE_ALIGNED(MAX_OV_COLS * NO_SYNTHESIS_CHANNELS * sizeof(WORD32),
                                           BYTE_ALIGN_8);

  size_buffers += 2 * LPC_ORDER * num_channel *
                  IXHEAAC_GET_SIZE_ALIGNED(NO_ANALYSIS_CHANNELS * sizeof(WORD32), BYTE_ALIGN_8);

  size_buffers += 2 * IXHEAAC_GET_SIZE_ALIGNED(LPC_ORDER * sizeof(WORD32 *), BYTE_ALIGN_8);

  size_buffers += 2 * LPC_ORDER * num_channel *
                  IXHEAAC_GET_SIZE_ALIGNED(NO_ANALYSIS_CHANNELS * sizeof(WORD32), BYTE_ALIGN_8);

  size_buffers += 2 * num_channel * 3 *
                  IXHEAAC_GET_SIZE_ALIGNED(MAX_FREQ_COEFFS * sizeof(WORD16), BYTE_ALIGN_8);

  temp = IXHEAAC_GET_SIZE_ALIGNED(sizeof(ia_freq_band_data_struct), BYTE_ALIGN_8) +
         IXHEAAC_GET_SIZE_ALIGNED(sizeof(ia_sbr_prev_frame_data_struct), BYTE_ALIGN_8) +
         IXHEAAC_GET_SIZE_ALIGNED(sizeof(ia_sbr_channel_struct), BYTE_ALIGN_8) +
         IXHEAAC_GET_SIZE_ALIGNED(sizeof(ia_sbr_header_data_struct), BYTE_ALIGN_8);
  size_buffers += 2 * num_channel * temp;

  size_buffers += MAX_BS_ELEMENT *
                  IXHEAAC_GET_SIZE_ALIGNED(sizeof(ixheaac_drc_bs_data_struct *), BYTE_ALIGN_8);

  size_buffers += num_channel * IXHEAAC_GET_SIZE_ALIGNED(sizeof(ia_ps_dec_struct), BYTE_ALIGN_8);

  {
    WORD32 temp_size = 0;
    size_buffers += (num_channel * 2 *
                     IXHEAAC_GET_SIZE_ALIGNED(sizeof(ia_sbr_frame_info_data_struct) +
                                                  MAX_FREQ_COEFFS * sizeof(WORD32) * 2 + 8,
                                              BYTE_ALIGN_8));
    temp_size += IXHEAAC_GET_SIZE_ALIGNED(sizeof(ia_pvc_data_struct), BYTE_ALIGN_8);
    temp_size += 2 * IXHEAAC_GET_SIZE_ALIGNED(sizeof(ia_esbr_hbe_txposer_struct), BYTE_ALIGN_8);
    temp_size += 2 * IXHEAAC_GET_SIZE_ALIGNED(MAX_HBE_PERSISTENT_SIZE, BYTE_ALIGN_8);
    temp_size +=
        2 * 2 * IXHEAAC_GET_SIZE_ALIGNED(MAX_QMF_BUF_LEN * sizeof(FLOAT32 *), BYTE_ALIGN_8);
    temp_size += 2 * 2 * MAX_QMF_BUF_LEN *
                 IXHEAAC_GET_SIZE_ALIGNED(MAX_QMF_BUF_LEN * sizeof(FLOAT32), BYTE_ALIGN_8);
    temp_size += 2 * 2 * MAX_ENV_COLS *
                 IXHEAAC_GET_SIZE_ALIGNED(MAX_QMF_BUF_LEN * sizeof(WORD32), BYTE_ALIGN_8);
    size_buffers += num_channel * temp_size;
  }

  return (size_buffers);
}

VOID ixheaacd_fill_aac_mem_tables(
    ia_exhaacplus_dec_api_struct *p_obj_exhaacplus_dec) {
  ia_mem_info_struct *p_mem_info_aac;

  WORD32 num_channels;
  WORD32 channels;
  WORD32 buffer_size;

  if (p_obj_exhaacplus_dec->aac_config.ui_max_channels > 2) {
    num_channels = (p_obj_exhaacplus_dec->aac_config.ui_max_channels + 1);
  } else

  {
    num_channels = p_obj_exhaacplus_dec->aac_config.ui_max_channels;
  }

  channels = num_channels;
  buffer_size = ixheaacd_persistent_buffer_sizes(num_channels);

  {
    p_mem_info_aac =
        &p_obj_exhaacplus_dec->p_mem_info_aac[IA_ENHAACPLUS_DEC_PERSIST_IDX];
    p_mem_info_aac->ui_size =
        IXHEAAC_GET_SIZE_ALIGNED(sizeof(ia_aac_dec_state_struct), BYTE_ALIGN_8) +
        channels *
            IXHEAAC_GET_SIZE_ALIGNED(sizeof(struct ia_aac_persistent_struct), BYTE_ALIGN_8) +
        buffer_size + channels * ixheaacd_getsize_sbr_persistent() + channels * 16 +
        ixheaacd_mps_persistent_buffer_sizes();
    p_mem_info_aac->ui_size += IXHEAAC_GET_SIZE_ALIGNED(sizeof(ia_dec_data_struct), BYTE_ALIGN_8);
    p_mem_info_aac->ui_size +=
        IXHEAAC_GET_SIZE_ALIGNED(sizeof(ia_audio_specific_config_struct), BYTE_ALIGN_8);
    p_mem_info_aac->ui_size += IXHEAAC_GET_SIZE_ALIGNED(MAX_HEADER_BUF_SIZE, BYTE_ALIGN_8);
    p_mem_info_aac->ui_alignment = 16;
    p_mem_info_aac->ui_type = IA_MEMTYPE_PERSIST;
  }

  {
    p_mem_info_aac =
        &p_obj_exhaacplus_dec->p_mem_info_aac[IA_ENHAACPLUS_DEC_SCRATCH_IDX];

    p_mem_info_aac->ui_size = MAX_SCR_SIZE;

    p_mem_info_aac->ui_alignment = 8;
    p_mem_info_aac->ui_type = IA_MEMTYPE_SCRATCH;
  }
  {
    p_mem_info_aac =
        &p_obj_exhaacplus_dec->p_mem_info_aac[IA_ENHAACPLUS_DEC_INPUT_IDX];

    p_mem_info_aac->ui_size = IA_MAX_INP_BUFFER_SIZE;

    p_mem_info_aac->ui_alignment = 8;
    p_mem_info_aac->ui_type = IA_MEMTYPE_INPUT;
  }
  {
    p_mem_info_aac =
        &p_obj_exhaacplus_dec->p_mem_info_aac[IA_ENHAACPLUS_DEC_OUTPUT_IDX];
    p_mem_info_aac->ui_size = IA_ENHAACPLUS_DEC_OUT_BUF_SIZE;
    p_mem_info_aac->ui_alignment = 8;
    p_mem_info_aac->ui_type = IA_MEMTYPE_OUTPUT;
  }
  return;
}

IA_ERRORCODE ixheaacd_dec_table_api(
    ia_exhaacplus_dec_api_struct *p_obj_exhaacplus_dec, WORD32 i_cmd,
    WORD32 i_idx, pVOID pv_value) {
  ia_heaac_mps_state_struct *pstr_mps_state =
      &p_obj_exhaacplus_dec->p_state_aac->heaac_mps_handle;
  pUWORD32 pui_value = pv_value;
  pUWORD32 *p_pui_value = pv_value;
  SIZE_T ui_get_vals[5];

  pVOID *table_ptrs[8 + NUM_MPS_TABLES];
  UWORD32 table_sizes[8 + NUM_MPS_TABLES] = {
      sizeof(ixheaacd_aac_huffmann_tables),
      sizeof(ixheaacd_aac_block_tables),
      sizeof(ixheaacd_imdct_tables),
      sizeof(ixheaacd_str_fft_n_transcendent_tables),
      sizeof(ixheaacd_aac_dec_env_calc_tables),
      sizeof(ixheaacd_aac_qmf_dec_tables),
      sizeof(ixheaacd_aac_dec_env_extr_tables),
      sizeof(ixheaacd_aac_dec_ps_tables),
      sizeof(ixheaacd_mps_dec_qmf_tables),
      sizeof(ixheaacd_mps_dec_common_tables),
      sizeof(ixheaacd_mps_dec_hybrid_tables),
      sizeof(ixheaacd_mps_dec_m1_m2_tables),
      sizeof(ixheaacd_mps_dec_decorr_tables),
      sizeof(ixheaacd_mps_dec_tp_process_tables),
      sizeof(ixheaacd_mps_dec_mdct2qmf_table),
      sizeof(ixheaacd_mps_dec_tonality_tables),
      sizeof(ixheaacd_mps_dec_bitdec_tables),
      sizeof(ixheaacd_mps_dec_blind_tables),
      sizeof(ixheaacd_mps_dec_mdct2qmf_tables),
      sizeof(ia_mps_dec_mdct2qmf_cos_table_struct),
      sizeof(ia_mps_dec_residual_aac_tables_struct)};
  table_ptrs[0] =
      (pVOID *)&(p_obj_exhaacplus_dec->aac_tables.pstr_huffmann_tables);
  table_ptrs[1] =
      (pVOID *)&(p_obj_exhaacplus_dec->aac_tables.pstr_block_tables);
  table_ptrs[2] =
      (pVOID *)&(p_obj_exhaacplus_dec->aac_tables.pstr_imdct_tables);
  table_ptrs[3] = (pVOID *)&(p_obj_exhaacplus_dec->common_tables);
  table_ptrs[4] =
      (pVOID *)&p_obj_exhaacplus_dec->str_sbr_tables.env_calc_tables_ptr;
  table_ptrs[5] =
      (pVOID *)&p_obj_exhaacplus_dec->str_sbr_tables.qmf_dec_tables_ptr;
  table_ptrs[6] =
      (pVOID *)&p_obj_exhaacplus_dec->str_sbr_tables.env_extr_tables_ptr;
  table_ptrs[7] = (pVOID *)&p_obj_exhaacplus_dec->str_sbr_tables.ps_tables_ptr;
  table_ptrs[8] = (pVOID *)&(pstr_mps_state->ia_mps_dec_mps_table.qmf_table_ptr);
  table_ptrs[9] = (pVOID *)&(pstr_mps_state->ia_mps_dec_mps_table.common_table_ptr);
  table_ptrs[10] = (pVOID *)&(pstr_mps_state->ia_mps_dec_mps_table.hybrid_table_ptr);
  table_ptrs[11] = (pVOID *)&(pstr_mps_state->ia_mps_dec_mps_table.m1_m2_table_ptr);
  table_ptrs[12] = (pVOID *)&(pstr_mps_state->ia_mps_dec_mps_table.decor_table_ptr);
  table_ptrs[13] = (pVOID *)&(pstr_mps_state->ia_mps_dec_mps_table.tp_process_table_ptr);
  table_ptrs[14] = (pVOID *)&(pstr_mps_state->ia_mps_dec_mps_table.mdct2qmf_table_ptr);
  table_ptrs[15] = (pVOID *)&(pstr_mps_state->ia_mps_dec_mps_table.tonality_table_ptr);
  table_ptrs[16] = (pVOID *)&(pstr_mps_state->ia_mps_dec_mps_table.bitdec_table_ptr);
  table_ptrs[17] = (pVOID *)&(pstr_mps_state->ia_mps_dec_mps_table.blind_table_ptr);
  table_ptrs[18] = (pVOID *)&(pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr);
  table_ptrs[19] = (pVOID *)&(pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_tab_ptr);
  table_ptrs[20] = (pVOID *)&(pstr_mps_state->ia_mps_dec_mps_table.aac_tab);

  if (i_idx < 0 || i_idx >= (NUM_AAC_TABLES + NUM_MPS_TABLES)) {
    return IA_XHEAAC_DEC_API_FATAL_INVALID_MEMTAB_INDEX;
  }

  ui_get_vals[0] = table_sizes[i_idx];
  ui_get_vals[1] = 4;
  ui_get_vals[4] = (SIZE_T)(*table_ptrs[i_idx]);

  if (i_cmd == IA_API_CMD_SET_TABLE_PTR) {
    if (pv_value == 0) {
      return (IA_XHEAAC_DEC_API_FATAL_MEM_ALLOC);
    }
    if (((SIZE_T)pv_value) & 3) {
      return IA_XHEAAC_DEC_API_FATAL_MEM_ALIGN;
    }

    *table_ptrs[i_idx] = pv_value;

    if (i_idx == 0) {
      ixheaacd_huff_tables_create(&p_obj_exhaacplus_dec->aac_tables);
    }

  }

  else if (i_cmd == IA_API_CMD_GET_TABLE_PTR) {
    *p_pui_value = (UWORD32 *)((SIZE_T)(
        ui_get_vals[i_cmd - IA_API_CMD_GET_TABLE_INFO_SIZE]));
  } else {
    *pui_value = (WORD32)(ui_get_vals[i_cmd - IA_API_CMD_GET_TABLE_INFO_SIZE]);
  }

  return IA_NO_ERROR;
}

VOID ixheaacd_mps_payload(ia_handle_sbr_dec_inst_struct self,
                          ia_exhaacplus_dec_api_struct *p_obj_exhaacplus_dec) {
  struct ia_bit_buf_struct local_bit_buff;
  struct ia_bit_buf_struct *it_bit_buff;
  if (self->ptr_mps_data != NULL) {
    ixheaacd_create_init_bit_buf(&local_bit_buff, (UWORD8 *)self->ptr_mps_data,
                                 (self->left_mps_bits >> 3) + 1);
  }

  local_bit_buff.xaac_jmp_buf =
      &p_obj_exhaacplus_dec->p_state_aac->xaac_jmp_buf;

  it_bit_buff = &local_bit_buff;

  it_bit_buff->bit_pos = self->mps_bits_pos;
  it_bit_buff->cnt_bits = self->left_mps_bits;

  while (self->left_mps_bits >= 8) {
    ixheaacd_extension_payload(
        it_bit_buff, &self->left_mps_bits,
        &p_obj_exhaacplus_dec->p_state_aac->mps_dec_handle);
  }
}

IA_ERRORCODE ixheaacd_dec_init(
    ia_exhaacplus_dec_api_struct *p_obj_exhaacplus_dec) {
  FLAG frame_status = 1;
  WORD32 frame_size_1;
  WORD32 sample_rate_1;
  WORD16 num_channels_1;
  WORD32 ps_detected = 0;
  UWORD8 *in_buffer;
  WORD16 *time_data;
  WORD ch_idx;
  WORD sbr_present_flag = 0;
  UWORD8 *mps_buffer;
  ia_aac_dec_state_struct *p_state_enhaacplus_dec;

  WORD32 error_code = IA_NO_ERROR;
  WORD32 persistent_used = 0;
  IA_ERRORCODE err_code = IA_NO_ERROR;
  struct ia_aac_persistent_struct *aac_persistent_mem;
  struct ia_sbr_pers_struct *sbr_persistent_mem;
  WORD32 ret_val;

  p_obj_exhaacplus_dec->p_state_aac =
      p_obj_exhaacplus_dec->pp_mem_aac[IA_ENHAACPLUS_DEC_PERSIST_IDX];

  if (p_obj_exhaacplus_dec->p_state_aac->ui_init_done)
  {
    return IA_NO_ERROR;
  }

  p_obj_exhaacplus_dec->p_state_aac->preroll_config_present = 0;

  if (p_obj_exhaacplus_dec->p_state_aac != NULL) {
    ret_val = setjmp(p_obj_exhaacplus_dec->p_state_aac->xaac_jmp_buf);
    if (ret_val != 0) {
      p_obj_exhaacplus_dec->p_state_aac->i_bytes_consumed =
          p_obj_exhaacplus_dec->p_state_aac->ui_in_bytes;
      p_obj_exhaacplus_dec->p_state_aac->ui_out_bytes = 0;
      return IA_NO_ERROR;
    }
  }

  time_data = (WORD16 *)(p_obj_exhaacplus_dec
                             ->pp_mem_aac[IA_ENHAACPLUS_DEC_OUTPUT_IDX]);

  if (p_obj_exhaacplus_dec->aac_config.ui_flush_cmd == 0) {
    in_buffer = p_obj_exhaacplus_dec->pp_mem_aac[IA_ENHAACPLUS_DEC_INPUT_IDX];
  } else {
    in_buffer = p_obj_exhaacplus_dec->p_state_aac->header_ptr;
  }

  p_state_enhaacplus_dec = p_obj_exhaacplus_dec->p_state_aac;

  p_state_enhaacplus_dec->aac_scratch_mem_v =
      p_obj_exhaacplus_dec->pp_mem_aac[IA_ENHAACPLUS_DEC_SCRATCH_IDX];
  p_obj_exhaacplus_dec->p_state_aac->huffman_code_book_scl =
      p_obj_exhaacplus_dec->aac_tables.pstr_huffmann_tables
          ->huffman_code_book_scl;
  mps_buffer = p_obj_exhaacplus_dec->p_state_aac->mps_buffer;
  p_state_enhaacplus_dec->mps_header = -1;
  p_obj_exhaacplus_dec->p_state_aac->huffman_code_book_scl_index =
      p_obj_exhaacplus_dec->aac_tables.pstr_huffmann_tables
          ->huffman_code_book_scl_index;

  p_state_enhaacplus_dec->pstr_aac_tables = &p_obj_exhaacplus_dec->aac_tables;
  if (p_obj_exhaacplus_dec->p_state_aac->header_dec_done == 0)
  {
    p_obj_exhaacplus_dec->aac_config.header_dec_done = 0;
    p_state_enhaacplus_dec->mps_dec_handle.ldmps_config.ldmps_present_flag = 0;
  }
  if (p_obj_exhaacplus_dec->aac_config.header_dec_done == 0) {
    WORD32 channels;
    UWORD32 total_persistent_used = 0;

    p_obj_exhaacplus_dec->p_state_aac->p_config =
        &p_obj_exhaacplus_dec->aac_config;

    p_obj_exhaacplus_dec->p_state_aac->pstr_stream_sbr =
        (pVOID)((pWORD8)p_obj_exhaacplus_dec->p_state_aac +
                IXHEAAC_GET_SIZE_ALIGNED(sizeof(ia_aac_dec_state_struct), BYTE_ALIGN_8));
    if (p_obj_exhaacplus_dec->aac_config.ui_max_channels > 2) {
      p_state_enhaacplus_dec->aac_persistent_mem_v =
          (pVOID)((pWORD8)p_obj_exhaacplus_dec->p_state_aac->pstr_stream_sbr +
                  (MAX_BS_ELEMENT)*2 *
                      IXHEAAC_GET_SIZE_ALIGNED(sizeof(ia_aac_dec_sbr_bitstream_struct),
                                               BYTE_ALIGN_8));

      memset(p_obj_exhaacplus_dec->p_state_aac->pstr_stream_sbr, 0,
             (MAX_BS_ELEMENT)*2 *
                 IXHEAAC_GET_SIZE_ALIGNED(sizeof(ia_aac_dec_sbr_bitstream_struct), BYTE_ALIGN_8));
    } else {
      p_state_enhaacplus_dec->aac_persistent_mem_v =
          (pVOID)((pWORD8)p_obj_exhaacplus_dec->p_state_aac->pstr_stream_sbr +
                  (2) * 2 *
                      IXHEAAC_GET_SIZE_ALIGNED(sizeof(ia_aac_dec_sbr_bitstream_struct),
                                               BYTE_ALIGN_8));

      memset(p_obj_exhaacplus_dec->p_state_aac->pstr_stream_sbr, 0,
             (2) * 2 *
                 IXHEAAC_GET_SIZE_ALIGNED(sizeof(ia_aac_dec_sbr_bitstream_struct), BYTE_ALIGN_8));
    }
    if (1 == p_obj_exhaacplus_dec->aac_config.ui_max_channels)
      channels = 1;
    else
      channels = 2;

    persistent_used = ixheaacd_set_aac_persistent_buffers(
        p_state_enhaacplus_dec->aac_persistent_mem_v, channels);

    p_state_enhaacplus_dec->sbr_persistent_mem_v =
        (pVOID)((pWORD8)p_state_enhaacplus_dec->aac_persistent_mem_v +
                IXHEAAC_GET_SIZE_ALIGNED(persistent_used, BYTE_ALIGN_8));
    total_persistent_used += IXHEAAC_GET_SIZE_ALIGNED(persistent_used, BYTE_ALIGN_8);

    persistent_used = ixheaacd_getsize_sbr_persistent();
    ixheaacd_set_sbr_persistent_buffers(
        p_state_enhaacplus_dec->sbr_persistent_mem_v, &persistent_used,
        channels, 1);

    p_state_enhaacplus_dec->heaac_mps_handle.mps_persistent_mem_v =
        (pVOID)((pWORD8)p_state_enhaacplus_dec->sbr_persistent_mem_v +
                IXHEAAC_GET_SIZE_ALIGNED(persistent_used, BYTE_ALIGN_8));
    total_persistent_used += IXHEAAC_GET_SIZE_ALIGNED(persistent_used, BYTE_ALIGN_8);

    persistent_used = ixheaacd_getsize_mps_persistent();

    ixheaacd_set_mps_persistent_buffers(
        &p_state_enhaacplus_dec->heaac_mps_handle, &persistent_used, channels,
        p_state_enhaacplus_dec->heaac_mps_handle.mps_persistent_mem_v);
    total_persistent_used += IXHEAAC_GET_SIZE_ALIGNED(persistent_used, BYTE_ALIGN_8);

    if (total_persistent_used >
        p_obj_exhaacplus_dec->p_mem_info_aac[IA_ENHAACPLUS_DEC_PERSIST_IDX].ui_size) {
      return IA_XHEAAC_DEC_INIT_FATAL_DEC_INIT_FAIL;
    }

    aac_persistent_mem = (struct ia_aac_persistent_struct *)
                             p_state_enhaacplus_dec->aac_persistent_mem_v;
    if (p_state_enhaacplus_dec->audio_object_type == AOT_ER_AAC_LD ||
        p_state_enhaacplus_dec->audio_object_type == AOT_ER_AAC_ELD)
      p_state_enhaacplus_dec->frame_len_flag =
          p_obj_exhaacplus_dec->aac_config.framesize_480;

    p_state_enhaacplus_dec->ptr_overlap_buf =
        aac_persistent_mem->overlap_buffer;

    p_state_enhaacplus_dec->bit_count = 0;
    p_state_enhaacplus_dec->ec_enable = p_obj_exhaacplus_dec->aac_config.ui_err_conceal;
    p_state_enhaacplus_dec->sync_status = 0;
    p_state_enhaacplus_dec->bs_format = ADTS_BSFORMAT;
    p_state_enhaacplus_dec->latm_initialized = 0;
    p_state_enhaacplus_dec->frame_size = 0;
    memset(&p_state_enhaacplus_dec->latm_struct_element, 0,
           sizeof(ixheaacd_latm_struct));
    memset(&p_state_enhaacplus_dec->b_n_raw_data_blk, 0,
           sizeof(WORD32) * (9 + MAX_BS_ELEMENT));

    p_state_enhaacplus_dec->sbr_present_flag = 0;

    for (ch_idx = 0; ch_idx < MAX_BS_ELEMENT; ch_idx++) {
      p_state_enhaacplus_dec->str_sbr_dec_info[ch_idx] = 0;
    }

    memset(&p_state_enhaacplus_dec->ind_cc_info, 0,
           sizeof(ia_enhaacplus_dec_ind_cc));

    p_state_enhaacplus_dec->last_frame_ok = 1;
    p_obj_exhaacplus_dec->aac_config.header_dec_done = 1;

    aac_persistent_mem->str_aac_decoder.pstr_aac_tables =
        &p_obj_exhaacplus_dec->aac_tables;
    aac_persistent_mem->str_aac_decoder.pstr_common_tables =
        p_obj_exhaacplus_dec->common_tables;

    p_obj_exhaacplus_dec->p_state_aac->sbr_persistent_mem_u =
        p_obj_exhaacplus_dec->p_state_aac->sbr_persistent_mem_v;

    p_obj_exhaacplus_dec->p_state_aac->sbr_scratch_mem_u =
        p_obj_exhaacplus_dec->p_state_aac->aac_scratch_mem_v;

    ixheaacd_set_sbr_persistent_table_pointer(
        p_obj_exhaacplus_dec->p_state_aac->sbr_persistent_mem_v,
        &p_obj_exhaacplus_dec->str_sbr_tables,
        p_obj_exhaacplus_dec->common_tables);
    ixheaacd_set_scratch_buffers(
        &p_obj_exhaacplus_dec->p_state_aac->heaac_mps_handle,
        p_state_enhaacplus_dec->aac_scratch_mem_v);
  }

  if (p_obj_exhaacplus_dec->p_state_aac->ui_input_over == 1) {
    return IA_XHEAAC_DEC_INIT_FATAL_EO_INPUT_REACHED;
  }

  if (p_obj_exhaacplus_dec->p_state_aac->header_dec_done == 0) {
    if (p_state_enhaacplus_dec->audio_object_type == AOT_ER_AAC_LD ||
        p_state_enhaacplus_dec->audio_object_type == AOT_ER_AAC_ELD)
      p_state_enhaacplus_dec->frame_len_flag =
          p_obj_exhaacplus_dec->aac_config.framesize_480;

    aac_persistent_mem = (struct ia_aac_persistent_struct *)
                             p_state_enhaacplus_dec->aac_persistent_mem_v;
    sbr_persistent_mem = (struct ia_sbr_pers_struct *)
                             p_state_enhaacplus_dec->sbr_persistent_mem_v;
    if (p_obj_exhaacplus_dec->aac_config.ui_err_conceal) {
      p_obj_exhaacplus_dec->p_state_aac->first_frame = 1;
    }

    if (p_obj_exhaacplus_dec->aac_config.ui_samp_freq == 0) {
      WORD32 header_bytes_consumed, return_val;

      if (p_state_enhaacplus_dec->ui_in_bytes == 0) {
        p_state_enhaacplus_dec->i_bytes_consumed = 0;
        return IA_NO_ERROR;
      }

      if (1 == p_obj_exhaacplus_dec->aac_config.ui_frame_size) {
        p_state_enhaacplus_dec->frame_len_flag = 1;
        p_state_enhaacplus_dec->frame_length = 960;
      } else {
        p_state_enhaacplus_dec->frame_len_flag = 0;
        p_state_enhaacplus_dec->frame_length = 1024;
      }

      p_state_enhaacplus_dec->ui_init_done = 0;
      memset(&(p_state_enhaacplus_dec->eld_specific_config), 0,
             sizeof(ia_eld_specific_config_struct));
      return_val = ixheaacd_aac_headerdecode(
          p_obj_exhaacplus_dec, (UWORD8 *)in_buffer, &header_bytes_consumed,
          aac_persistent_mem->str_aac_decoder.pstr_aac_tables
              ->pstr_huffmann_tables);
      if (p_state_enhaacplus_dec->audio_object_type == AOT_ER_AAC_LD ||
          p_state_enhaacplus_dec->audio_object_type == AOT_ER_AAC_ELD ||
          p_state_enhaacplus_dec->audio_object_type == AOT_ER_AAC_LC) {
        *sbr_persistent_mem->str_sbr_dec_inst.pstr_sbr_header[0] =
            p_obj_exhaacplus_dec->p_state_aac->str_sbr_config;
        *sbr_persistent_mem->str_sbr_dec_inst.pstr_sbr_header[1] =
            p_obj_exhaacplus_dec->p_state_aac->str_sbr_config;
      } else {
        memset(&(p_state_enhaacplus_dec->eld_specific_config), 0,
               sizeof(ia_eld_specific_config_struct));
      }

      if (return_val < 0) {
        if (return_val ==
            (WORD32)IA_XHEAAC_DEC_INIT_FATAL_STREAM_CHAN_GT_MAX) {
          p_state_enhaacplus_dec->i_bytes_consumed = header_bytes_consumed;
          return return_val;
        }
        p_state_enhaacplus_dec->i_bytes_consumed = 1;

        return return_val;
      }

      if (return_val ==
          IA_XHEAAC_DEC_EXE_NONFATAL_INSUFFICIENT_INPUT_BYTES) {
        p_state_enhaacplus_dec->i_bytes_consumed = header_bytes_consumed;
        return return_val;
      }

      p_state_enhaacplus_dec->i_bytes_consumed = header_bytes_consumed;

      if ((return_val == 0) &&
          (p_obj_exhaacplus_dec->p_state_aac->audio_object_type == AOT_USAC)) {
        {
          WORD32 pcm_size = p_obj_exhaacplus_dec->aac_config.ui_pcm_wdsz;
          WORD8 *inbuffer =
              p_obj_exhaacplus_dec->pp_mem_aac[IA_ENHAACPLUS_DEC_INPUT_IDX];
          WORD8 *outbuffer =
              p_obj_exhaacplus_dec->pp_mem_aac[IA_ENHAACPLUS_DEC_OUTPUT_IDX];
          WORD32 out_bytes = 0;
          WORD32 frames_done = p_obj_exhaacplus_dec->p_state_aac->frame_counter;
          p_obj_exhaacplus_dec->p_state_aac->decode_create_done = 0;

          if (p_obj_exhaacplus_dec->p_state_aac->ui_input_over == 0) {
            error_code = ixheaacd_dec_main(
                p_obj_exhaacplus_dec, inbuffer, outbuffer, &out_bytes,
                frames_done, pcm_size,
                &p_obj_exhaacplus_dec->p_state_aac->num_of_output_ch);
            if (error_code) return error_code;
            p_obj_exhaacplus_dec->p_state_aac->frame_counter++;
          } else {
            out_bytes = 0;
          }

          p_obj_exhaacplus_dec->aac_config.ui_n_channels =
              p_obj_exhaacplus_dec->p_state_aac->num_of_output_ch;
        }
        if (return_val == 0)
          p_obj_exhaacplus_dec->p_state_aac->ui_init_done = 1;
        return return_val;
      }

      if (return_val == 0) {
        p_obj_exhaacplus_dec->p_state_aac->header_dec_done = 1;
        if (p_obj_exhaacplus_dec->aac_config.ui_flush_cmd == 0) {
          memcpy(p_state_enhaacplus_dec->header_ptr, in_buffer,
                 header_bytes_consumed * sizeof(UWORD8));
          p_state_enhaacplus_dec->header_length = header_bytes_consumed;
        }
      }

      if (p_obj_exhaacplus_dec->p_state_aac->header_dec_done != 1)
        return IA_XHEAAC_DEC_INIT_NONFATAL_HEADER_NOT_AT_START;

      if (p_state_enhaacplus_dec->dwnsmp_signal == 1 &&
          p_state_enhaacplus_dec->audio_object_type == AOT_ER_AAC_ELD)
        p_obj_exhaacplus_dec->aac_config.down_sample_flag = 1;

      if (p_state_enhaacplus_dec->sampling_rate ==
          p_state_enhaacplus_dec->extension_samp_rate) {
        p_obj_exhaacplus_dec->aac_config.down_sample_flag = 1;
      }

    } else {
      p_obj_exhaacplus_dec->p_state_aac->header_dec_done = 1;
      p_state_enhaacplus_dec->i_bytes_consumed = 0;

      p_state_enhaacplus_dec->sampling_rate =
          p_obj_exhaacplus_dec->aac_config.ui_samp_freq;

      if (1 == p_obj_exhaacplus_dec->aac_config.ui_frame_size) {
        p_state_enhaacplus_dec->frame_len_flag = 1;
        p_state_enhaacplus_dec->frame_length = 960;
      } else {
        p_state_enhaacplus_dec->frame_len_flag = 0;
        p_state_enhaacplus_dec->frame_length = 1024;
      }
    }

    p_state_enhaacplus_dec->pstr_bit_buf = ixheaacd_create_bit_buf(
        &p_state_enhaacplus_dec->str_bit_buf, (UWORD8 *)in_buffer,
        p_obj_exhaacplus_dec->p_mem_info_aac[IA_ENHAACPLUS_DEC_INPUT_IDX]
            .ui_size);
    p_state_enhaacplus_dec->pstr_bit_buf->xaac_jmp_buf =
        &(p_state_enhaacplus_dec->xaac_jmp_buf);

    p_state_enhaacplus_dec->ptr_bit_stream =
        p_state_enhaacplus_dec->pstr_bit_buf;

    if (p_state_enhaacplus_dec->s_adts_hdr_present) {
      if (p_obj_exhaacplus_dec->aac_config.ld_decoder == 1)
        p_state_enhaacplus_dec->audio_object_type = 23;
    }

    if ((p_state_enhaacplus_dec->audio_object_type == AOT_ER_AAC_ELD) ||
        (p_state_enhaacplus_dec->audio_object_type == AOT_ER_AAC_LD))
      if (p_state_enhaacplus_dec->s_adts_hdr_present) {
        if (p_state_enhaacplus_dec->audio_object_type == AOT_ER_AAC_ELD) {
          p_state_enhaacplus_dec->eld_specific_config.ld_sbr_samp_rate = 1;
          p_state_enhaacplus_dec->eld_specific_config.ld_sbr_crc_flag = 0;
          p_state_enhaacplus_dec->eld_specific_config.ld_sbr_flag_present = 0;

          if (p_obj_exhaacplus_dec->aac_config.eld_sbr_present == 1) {
            p_state_enhaacplus_dec->eld_specific_config.ld_sbr_flag_present = 1;
          }
        }
        if (p_obj_exhaacplus_dec->aac_config.framesize_480)
          p_state_enhaacplus_dec->frame_length = 480;
        else
          p_state_enhaacplus_dec->frame_length = 512;
      }

    {
      for (ch_idx = 0; ch_idx < MAX_BS_ELEMENT; ch_idx++) {
        WORD32 channels;
        channels = 2;

        p_state_enhaacplus_dec->pstr_aac_dec_info[ch_idx] =
            ixheaacd_aac_decoder_init(
                p_state_enhaacplus_dec,

                p_state_enhaacplus_dec->pstr_stream_sbr[0], channels,
                p_state_enhaacplus_dec->aac_persistent_mem_v,
                p_state_enhaacplus_dec->frame_length);

        if (!p_state_enhaacplus_dec->pstr_aac_dec_info[ch_idx]) {
          p_state_enhaacplus_dec->i_bytes_consumed = 1;
          return IA_XHEAAC_DEC_INIT_FATAL_DEC_INIT_FAIL;
        }
      }

      {
        p_state_enhaacplus_dec->pstr_drc_dec =
            &p_state_enhaacplus_dec->str_drc_dec_info;
        ixheaacd_drc_dec_create(p_state_enhaacplus_dec->pstr_drc_dec, 127, 127);
      }
      p_state_enhaacplus_dec->pstr_drc_dec->cut_factor =
          p_obj_exhaacplus_dec->aac_config.ui_drc_cut;
      p_state_enhaacplus_dec->pstr_drc_dec->boost_factor =
          p_obj_exhaacplus_dec->aac_config.ui_drc_boost;
      p_state_enhaacplus_dec->pstr_drc_dec->target_ref_level =
          p_obj_exhaacplus_dec->aac_config.ui_drc_target_level;
      p_state_enhaacplus_dec->pstr_drc_dec->prog_ref_level =
          p_obj_exhaacplus_dec->aac_config.ui_drc_target_level;

      if (1 == p_obj_exhaacplus_dec->aac_config.ui_drc_set) {
        if (p_obj_exhaacplus_dec->aac_config.ui_drc_heavy_comp == 1) {
          p_state_enhaacplus_dec->pstr_drc_dec->drc_on = 1;
          p_state_enhaacplus_dec->pstr_drc_dec->heavy_mode = 1;
        } else {
          p_state_enhaacplus_dec->pstr_drc_dec->heavy_mode = 0;
          if (p_state_enhaacplus_dec->pstr_drc_dec->target_ref_level > 127)
            p_state_enhaacplus_dec->pstr_drc_dec->target_ref_level = 127;
          if (p_state_enhaacplus_dec->pstr_drc_dec->target_ref_level < 0) {
            if (p_state_enhaacplus_dec->pstr_drc_dec->cut_factor > 0 ||
                p_state_enhaacplus_dec->pstr_drc_dec->boost_factor > 0)
              p_state_enhaacplus_dec->pstr_drc_dec->drc_on = 1;
            else
              p_state_enhaacplus_dec->pstr_drc_dec->drc_on = 0;
            p_state_enhaacplus_dec->pstr_drc_dec->drc_dig_norm = 0;
            p_state_enhaacplus_dec->pstr_drc_dec->target_ref_level = 108;
          } else {
            p_state_enhaacplus_dec->pstr_drc_dec->drc_on = 1;
            p_state_enhaacplus_dec->pstr_drc_dec->drc_dig_norm = 1;
          }
        }
      }
    }
  } else {
    struct ia_bit_buf_struct temp_bit_buff = {0};
    ia_adts_header_struct adts;
    struct ia_bit_buf_struct *it_bit_buff;

    WORD16 frame_size_2 = 0;
    WORD32 sample_rate_2 = 0;
    WORD32 sample_rate = 0;
    WORD type, i;
    WORD elements_number;

    if (p_state_enhaacplus_dec->audio_object_type == AOT_USAC)
      return IA_FATAL_ERROR;

    memset(&adts, 0, sizeof(ia_adts_header_struct));

    for (i = 0; i < MAX_BS_ELEMENT + 1; i++) {
      p_obj_exhaacplus_dec->aac_config.element_type[i] = -1;
    }

    it_bit_buff = p_state_enhaacplus_dec->pstr_bit_buf;

    p_obj_exhaacplus_dec->aac_config.ui_sbr_mode = 0;
    p_obj_exhaacplus_dec->p_state_aac->ui_out_bytes = 0;
    p_state_enhaacplus_dec->ui_mps_out_bytes = 0;
    if (p_state_enhaacplus_dec->ui_in_bytes == 0) {
      p_state_enhaacplus_dec->i_bytes_consumed = 0;
      return IA_NO_ERROR;
    }

    if (p_state_enhaacplus_dec->audio_object_type == AOT_ER_AAC_LD ||
        p_state_enhaacplus_dec->audio_object_type == AOT_ER_AAC_ELD) {
      if (p_obj_exhaacplus_dec->aac_config.ui_mp4_flag)
        p_state_enhaacplus_dec->frame_size =
            p_state_enhaacplus_dec->ui_in_bytes;
    }

    ixheaacd_create_init_bit_buf(it_bit_buff, in_buffer,
                                 p_state_enhaacplus_dec->ui_in_bytes);
    p_state_enhaacplus_dec->pstr_bit_buf->xaac_jmp_buf =
        &(p_state_enhaacplus_dec->xaac_jmp_buf);

    it_bit_buff->adts_header_present =
        p_state_enhaacplus_dec->s_adts_hdr_present;
    it_bit_buff->no_raw_data_blocks =
        (WORD8)p_state_enhaacplus_dec->b_n_raw_data_blk;
    it_bit_buff->protection_absent = p_state_enhaacplus_dec->protection_absent;

    memcpy(&temp_bit_buff, it_bit_buff, sizeof(struct ia_bit_buf_struct));

    if (p_obj_exhaacplus_dec->aac_config.ui_max_channels > 2)
      elements_number = MAX_BS_ELEMENT;
    else
      elements_number = 2;

    for (i = 0; i < elements_number; i++)
      p_state_enhaacplus_dec->pstr_stream_sbr[i][0].no_elements = 0;

    { it_bit_buff->initial_cnt_bits = it_bit_buff->cnt_bits; }

    ixheaacd_byte_align(
        p_state_enhaacplus_dec->ptr_bit_stream,
        &p_state_enhaacplus_dec->pstr_aac_dec_info[0]->byte_align_bits);

    if (p_state_enhaacplus_dec->s_adts_hdr_present) {
      WORD32 error;

      if (p_state_enhaacplus_dec->b_n_raw_data_blk == 0) {
        error = ixheaacd_readifadts(p_state_enhaacplus_dec, it_bit_buff, &adts);

        if (error) return error;

        p_state_enhaacplus_dec->protection_absent = adts.protection_absent;

        if (p_state_enhaacplus_dec->audio_object_type == AOT_ER_AAC_LD ||
            p_state_enhaacplus_dec->audio_object_type == AOT_ER_AAC_ELD) {
          p_state_enhaacplus_dec->frame_size = adts.aac_frame_length;
          if (p_obj_exhaacplus_dec->aac_config.framesize_480)
            p_state_enhaacplus_dec->frame_length = 480;
          else
            p_state_enhaacplus_dec->frame_length = 512;
        }
      }
    }

    if (p_state_enhaacplus_dec->bs_format == LOAS_BSFORMAT) {
      WORD32 result;
      WORD32 sync;
      WORD32 cnt_bits;

      sync = ixheaacd_read_bits_buf(it_bit_buff, 11);
      cnt_bits = it_bit_buff->cnt_bits;
      if (it_bit_buff->cnt_bits <= 24) {
        return IA_XHEAAC_DEC_INIT_NONFATAL_INSUFFICIENT_INPUT_BYTES;
      }

      while (sync != 0x2b7) {
        sync = ((sync & 0x3ff) << 1) | ixheaacd_read_bits_buf(it_bit_buff, 1);
        if (it_bit_buff->cnt_bits < 11) {
          ixheaacd_read_bidirection(it_bit_buff, -11);
          p_state_enhaacplus_dec->i_bytes_consumed =
              (cnt_bits - it_bit_buff->cnt_bits) / 8;
          return (IA_XHEAAC_DEC_INIT_NONFATAL_HEADER_NOT_AT_START);
        }
      }

      it_bit_buff->audio_mux_align = it_bit_buff->cnt_bits - 13;

      if (sync == 0x2b7) {
        result = ixheaacd_latm_audio_mux_element(
            it_bit_buff, &p_state_enhaacplus_dec->latm_struct_element,
            p_state_enhaacplus_dec,
            (ia_sampling_rate_info_struct *)&p_obj_exhaacplus_dec->aac_tables
                .pstr_huffmann_tables->str_sample_rate_info[0]);
        if (result < 0) {
          return result;
        }
      }
    }

    p_state_enhaacplus_dec->pstr_aac_dec_info[0]->byte_align_bits =
        it_bit_buff->cnt_bits;

    type = -1;
    ch_idx = 0;

    while ((type != 7)) {
      ia_aac_dec_scratch_struct aac_scratch_struct;
      memset(&aac_scratch_struct, 0, sizeof(aac_scratch_struct));

      if (ch_idx >= elements_number) {
        p_state_enhaacplus_dec->i_bytes_consumed = 1;

        return IA_XHEAAC_DEC_INIT_FATAL_STREAM_CHAN_GT_MAX;
      }

      ixheaacd_allocate_aac_scr(
          &aac_scratch_struct, p_state_enhaacplus_dec->aac_scratch_mem_v,
          time_data, 1, p_obj_exhaacplus_dec->aac_config.ui_max_channels,
          p_state_enhaacplus_dec->audio_object_type);

      p_state_enhaacplus_dec->pstr_aac_dec_info[ch_idx]->p_ind_channel_info =
          &p_state_enhaacplus_dec->ind_cc_info;
      if (p_obj_exhaacplus_dec->aac_config.ui_err_conceal) {
        p_obj_exhaacplus_dec->aac_config.first_frame = 1;
      }

      error_code = ixheaacd_aacdec_decodeframe(
          p_obj_exhaacplus_dec, &aac_scratch_struct, time_data, frame_status, &type, &ch_idx, 1,
          2, p_obj_exhaacplus_dec->aac_config.element_instance_order, 0, 1, 0,
          p_obj_exhaacplus_dec->aac_config.ui_max_channels, 2,
          p_obj_exhaacplus_dec->p_state_aac->frame_length,
          p_obj_exhaacplus_dec->p_state_aac->frame_size, p_state_enhaacplus_dec->pstr_drc_dec,
          p_state_enhaacplus_dec->audio_object_type, p_state_enhaacplus_dec->ch_config,
          p_state_enhaacplus_dec->eld_specific_config, p_state_enhaacplus_dec->s_adts_hdr_present,
          &p_state_enhaacplus_dec->drc_dummy, p_state_enhaacplus_dec->ldmps_present,
          &p_state_enhaacplus_dec->slot_pos, mps_buffer, &p_state_enhaacplus_dec->mps_header,
          &p_state_enhaacplus_dec->ui_mps_out_bytes, 1,
          p_obj_exhaacplus_dec->aac_config.first_frame);

      if (p_state_enhaacplus_dec->pstr_drc_dec->drc_element_found == 1) {
        if (p_obj_exhaacplus_dec->aac_config.i_loud_ref_level < 0) {
          p_obj_exhaacplus_dec->aac_config.output_level =
              p_state_enhaacplus_dec->pstr_drc_dec->prog_ref_level;
        } else {
          p_obj_exhaacplus_dec->aac_config.output_level =
              p_obj_exhaacplus_dec->aac_config.i_loud_ref_level;
        }
      }

      memset(&(p_obj_exhaacplus_dec->p_state_aac->pstr_aac_dec_info[ch_idx]
                   ->pstr_aac_dec_ch_info[0]
                   ->str_ics_info.ltp),
             0, sizeof(ltp_info));
      memset(&(p_obj_exhaacplus_dec->p_state_aac->pstr_aac_dec_info[ch_idx]
                   ->pstr_aac_dec_ch_info[0]
                   ->str_ics_info.ltp2),
             0, sizeof(ltp_info));
      memset(&(p_obj_exhaacplus_dec->p_state_aac->pstr_aac_dec_info[ch_idx]
                   ->pstr_aac_dec_ch_info[1]
                   ->str_ics_info.ltp),
             0, sizeof(ltp_info));
      memset(&(p_obj_exhaacplus_dec->p_state_aac->pstr_aac_dec_info[ch_idx]
                   ->pstr_aac_dec_ch_info[1]
                   ->str_ics_info.ltp2),
             0, sizeof(ltp_info));

      {

        frame_size_1 = p_state_enhaacplus_dec->frame_length;
        sample_rate_1 =
            p_state_enhaacplus_dec->pstr_aac_dec_info[ch_idx]->sampling_rate;
        num_channels_1 =
            p_state_enhaacplus_dec->pstr_aac_dec_info[ch_idx]->channels;
      }

      if ((p_obj_exhaacplus_dec->aac_config.ui_max_channels <= 2) &&
          (p_obj_exhaacplus_dec->aac_config.element_type[ch_idx] == 2)) {
        p_state_enhaacplus_dec->i_bytes_consumed = 1;
        return IA_XHEAAC_DEC_INIT_FATAL_UNIMPLEMENTED_CCE;
      }

      if (p_state_enhaacplus_dec->pstr_stream_sbr[0][0].no_elements) {
        sbr_present_flag = 1;
        p_obj_exhaacplus_dec->aac_config.ui_sbr_mode = 1;
      }

      if (error_code) {
        if (p_state_enhaacplus_dec->ui_input_over) {
          return IA_XHEAAC_DEC_INIT_FATAL_EO_INPUT_REACHED;
        }

        ixheaacd_updatebytesconsumed(p_state_enhaacplus_dec, it_bit_buff);
        return error_code;
      }

      if (p_state_enhaacplus_dec->s_adts_hdr_present) {
        if (adts.no_raw_data_blocks != 0) {
          if (adts.protection_absent == 0 && it_bit_buff->cnt_bits >= 16) {
            adts.crc_check = ixheaacd_read_bits_buf(it_bit_buff, 16);
          }
        }
        p_state_enhaacplus_dec->b_n_raw_data_blk--;
      }

      sample_rate_2 = sample_rate_1;
      frame_size_2 = frame_size_1;

      if (!p_state_enhaacplus_dec->str_sbr_dec_info[ch_idx] &&
          p_state_enhaacplus_dec->pstr_stream_sbr[0][0].no_elements) {
        WORD32 harmonic_sbr_flag = 0;
        if ((p_obj_exhaacplus_dec->aac_config.flag_16khz_out == 1) &&
            (sample_rate_1 == 8000)) {
          p_obj_exhaacplus_dec->aac_config.flag_16khz_out = 0;
        }

        p_state_enhaacplus_dec->str_sbr_dec_info[ch_idx] = ixheaacd_init_sbr(
            sample_rate_1, frame_size_1,
            (FLAG *)&p_obj_exhaacplus_dec->aac_config.down_sample_flag,
            p_state_enhaacplus_dec->sbr_persistent_mem_v,
            p_state_enhaacplus_dec->ptr_overlap_buf, MAXNRSBRCHANNELS, (WORD)1,
            1, frame_size_1 * 2, &harmonic_sbr_flag, NULL,
            p_state_enhaacplus_dec->str_sbr_config,
            p_state_enhaacplus_dec->audio_object_type,
            p_state_enhaacplus_dec->mps_dec_handle.ldmps_config
                .ldmps_present_flag,
            p_state_enhaacplus_dec->mps_dec_handle.ldmps_config
                .no_ldsbr_present);
        if (p_state_enhaacplus_dec->str_sbr_dec_info[ch_idx]) {
          p_state_enhaacplus_dec->str_sbr_dec_info[ch_idx]->xaac_jmp_buf =
              &(p_state_enhaacplus_dec->xaac_jmp_buf);
        }
      } else {
      }

      if (p_state_enhaacplus_dec->str_sbr_dec_info[ch_idx] &&
          p_state_enhaacplus_dec->pstr_stream_sbr[0][0].no_elements) {
        ia_sbr_scr_struct sbr_scratch_struct;
        WORD16 num_channels_1_t = num_channels_1;
        ixheaacd_allocate_sbr_scr(&sbr_scratch_struct, p_state_enhaacplus_dec->aac_scratch_mem_v,
                                  time_data, 0, NULL, 0, 0);
        {
          WORD32 audio_object_type = p_state_enhaacplus_dec->audio_object_type;

          if (audio_object_type != AOT_ER_AAC_ELD && audio_object_type != AOT_ER_AAC_LD) {
            WORD32 i = 0;
            ia_dec_data_struct *pstr_dec_data =
                (ia_dec_data_struct *)p_state_enhaacplus_dec->pstr_dec_data;
            if (num_channels_1 == 1) {
              for (; i < 1024; i++) {
                pstr_dec_data->str_usac_data.time_sample_vector[0][i] =
                    ((FLOAT32)time_data[i]);
              }
              p_state_enhaacplus_dec->str_sbr_dec_info[ch_idx]->time_sample_buf[0] =
                  &pstr_dec_data->str_usac_data.time_sample_vector[0][0];

              p_state_enhaacplus_dec->str_sbr_dec_info[ch_idx]->time_sample_buf[1] =
                  &pstr_dec_data->str_usac_data.time_sample_vector[1][0];
            } else if (num_channels_1 == 2) {
              for (; i < 1024; i++) {
                pstr_dec_data->str_usac_data.time_sample_vector[0][i] =
                    ((FLOAT32)time_data[2 * i + 0]);
                pstr_dec_data->str_usac_data.time_sample_vector[1][i] =
                    ((FLOAT32)time_data[2 * i + 1]);
              }
              p_state_enhaacplus_dec->str_sbr_dec_info[ch_idx]->time_sample_buf[0] =
                  &pstr_dec_data->str_usac_data.time_sample_vector[0][0];

              p_state_enhaacplus_dec->str_sbr_dec_info[ch_idx]->time_sample_buf[1] =
                  &pstr_dec_data->str_usac_data.time_sample_vector[1][0];
            }
          }
        }
        p_state_enhaacplus_dec->str_sbr_dec_info[ch_idx]->ec_flag =
            p_obj_exhaacplus_dec->aac_config.ui_err_conceal;
        p_state_enhaacplus_dec->str_sbr_dec_info[ch_idx]->esbr_hq =
            p_obj_exhaacplus_dec->aac_config.ui_hq_esbr;
        p_state_enhaacplus_dec->str_sbr_dec_info[ch_idx]->enh_sbr =
            p_obj_exhaacplus_dec->aac_config.ui_enh_sbr;
        p_state_enhaacplus_dec->str_sbr_dec_info[ch_idx]->enh_sbr_ps =
            p_obj_exhaacplus_dec->aac_config.ui_enh_sbr_ps;

        if (p_state_enhaacplus_dec->str_sbr_dec_info[ch_idx]) {
          p_state_enhaacplus_dec->str_sbr_dec_info[ch_idx]->xaac_jmp_buf =
              &(p_state_enhaacplus_dec->xaac_jmp_buf);
        }

        if (ixheaacd_applysbr(p_state_enhaacplus_dec->str_sbr_dec_info[ch_idx],
                              &p_state_enhaacplus_dec->pstr_stream_sbr[0][0], time_data,
                              &num_channels_1, frame_status,
                              p_obj_exhaacplus_dec->aac_config.down_sample_flag, 0,
                              &sbr_scratch_struct, 1, 1, 0, NULL, NULL,
                              p_state_enhaacplus_dec->eld_specific_config.ld_sbr_flag_present,
                              p_state_enhaacplus_dec->audio_object_type, 1,
                              p_state_enhaacplus_dec->ldmps_present, frame_size_1,
                              p_state_enhaacplus_dec->heaac_mps_handle.heaac_mps_present,
                              p_obj_exhaacplus_dec->aac_config.ui_err_conceal,
                              p_obj_exhaacplus_dec->aac_config.first_frame) != SBRDEC_OK) {
          p_state_enhaacplus_dec->str_sbr_dec_info[ch_idx] = 0;
          return -1;
        } else {
          if (!p_obj_exhaacplus_dec->aac_config.down_sample_flag) {
            sample_rate_1 *= 2;
          }
          if (p_state_enhaacplus_dec->eld_specific_config.ld_sbr_flag_present == 1) {
            p_obj_exhaacplus_dec->p_state_aac->mps_dec_handle.pre_mix_req = 1;
            ixheaacd_mps_payload(
              p_state_enhaacplus_dec->str_sbr_dec_info[ch_idx],
              p_obj_exhaacplus_dec);
          }
        }
        {
          WORD32 audio_object_type = p_state_enhaacplus_dec->audio_object_type;

          if (audio_object_type != AOT_ER_AAC_ELD && audio_object_type != AOT_ER_AAC_LD) {
            WORD32 out_bytes = 0;
            ia_dec_data_struct *pstr_dec_data =
                (ia_dec_data_struct *)p_state_enhaacplus_dec->pstr_dec_data;
            ixheaacd_samples_sat((WORD8 *)time_data, 2048, 16,
                                 pstr_dec_data->str_usac_data.time_sample_vector, &out_bytes, 1);
          }
        }
        if (p_obj_exhaacplus_dec->aac_config.flag_downmix) {
          num_channels_1 = 1;
        }
        if (num_channels_1_t == 1 && num_channels_1 == 2) ps_detected = 1;
      } else {
        p_state_enhaacplus_dec->mps_dec_handle.ldmps_config.no_ldsbr_present = 1;
      }

      p_state_enhaacplus_dec->i_bytes_consumed = 0;
      p_state_enhaacplus_dec->pstr_bit_buf = it_bit_buff;

      {
        p_state_enhaacplus_dec->pstr_aac_dec_info[ch_idx] =
            ixheaacd_aac_decoder_init(
                p_state_enhaacplus_dec,
                p_state_enhaacplus_dec->pstr_stream_sbr[0], 2,
                p_state_enhaacplus_dec->aac_persistent_mem_v,
                p_state_enhaacplus_dec->frame_length);

        if (!p_state_enhaacplus_dec->pstr_aac_dec_info[ch_idx]) {
          p_state_enhaacplus_dec->i_bytes_consumed = 1;
          return IA_XHEAAC_DEC_INIT_FATAL_DEC_INIT_FAIL;
        }

        if (p_state_enhaacplus_dec->str_sbr_dec_info[ch_idx]) {
          WORD32 harmonic_sbr_flag = 0;
          p_state_enhaacplus_dec->str_sbr_dec_info[ch_idx] = ixheaacd_init_sbr(
              sample_rate_2, frame_size_2,
              (FLAG *)&p_obj_exhaacplus_dec->aac_config.down_sample_flag,
              p_state_enhaacplus_dec->sbr_persistent_mem_v,
              p_state_enhaacplus_dec->ptr_overlap_buf, MAXNRSBRCHANNELS, 1, 1,
              frame_size_2 * 2, &harmonic_sbr_flag, NULL,
              p_state_enhaacplus_dec->str_sbr_config,
              p_state_enhaacplus_dec->audio_object_type,
              p_state_enhaacplus_dec->mps_dec_handle.ldmps_config
                .ldmps_present_flag,
              p_state_enhaacplus_dec->mps_dec_handle.ldmps_config
                .no_ldsbr_present);
        }
        if (p_state_enhaacplus_dec->str_sbr_dec_info[ch_idx]) {
          p_state_enhaacplus_dec->str_sbr_dec_info[ch_idx]->xaac_jmp_buf =
              &(p_state_enhaacplus_dec->xaac_jmp_buf);
        }
      }

      if (sample_rate < sample_rate_1) sample_rate = sample_rate_1;

      ch_idx++;

      if (p_state_enhaacplus_dec->audio_object_type >= ER_OBJECT_START &&
          (p_state_enhaacplus_dec->audio_object_type == AOT_ER_AAC_ELD ||
          p_state_enhaacplus_dec->audio_object_type == AOT_ER_AAC_LD ||
          p_state_enhaacplus_dec->audio_object_type == AOT_ER_AAC_LC))
        break;
    }

    {
      ia_adts_crc_info_struct *ptr_adts_crc_info =
          p_state_enhaacplus_dec->ptr_bit_stream->pstr_adts_crc_info;
      if (ptr_adts_crc_info->crc_active == 1) {
        if ((error_code = ixheaacd_adts_crc_check_crc(ptr_adts_crc_info))) {
          return error_code;
        }
      }
    }

    {
      VOID *temp;
      WORD prev_persistent_used_t;
      WORD prev_sbrpersistent_used_t;
      WORD ps_enable;
      WORD ch_idx_err = 0;
      WORD persistent_used_t = 0;
      WORD channel_check = 0;
      WORD cc_channel_check = 0;
      WORD max_ch_num = p_obj_exhaacplus_dec->aac_config.ui_max_channels;
      WORD32 harmonic_sbr_flag = 0;
      i = 0;

      p_obj_exhaacplus_dec->aac_config.ui_n_channels = ch_idx;
      while (p_obj_exhaacplus_dec->aac_config.element_type[ch_idx_err] <= 3 &&
             p_obj_exhaacplus_dec->aac_config.element_type[ch_idx_err] >= 0) {
        ch_idx_err++;
      }

      if (ch_idx_err == 0) {
        p_obj_exhaacplus_dec->p_state_aac->header_dec_done = 0;
        p_state_enhaacplus_dec->i_bytes_consumed = (WORD32)(
            it_bit_buff->ptr_read_next - it_bit_buff->ptr_bit_buf_base);
        return IA_XHEAAC_DEC_INIT_NONFATAL_DECODE_FRAME_ERROR;
      }

      if (ch_idx_err == 1)
        ps_enable = 1;
      else
        ps_enable = 0;

      while (p_obj_exhaacplus_dec->aac_config.element_type[i] >= 0 &&
             p_obj_exhaacplus_dec->aac_config.element_type[i] <= 3) {
        WORD32 channel = 0;
        switch (p_obj_exhaacplus_dec->aac_config.element_type[i]) {
          case 0:
          case 3:
            channel = 1;
            break;
          case 1:
            channel = 2;
            break;
          case 2:
            if (max_ch_num > 2) {
              if (p_obj_exhaacplus_dec->aac_config.element_instance_order[i] !=
                  p_obj_exhaacplus_dec->aac_config.ui_coupling_channel) {
                i++;
                continue;
              }
              channel = 1;
            } else {
              i++;
              continue;
            }
            cc_channel_check++;
            break;
          default:
            return -1;
        }

        if (cc_channel_check > MAX_CC_CHANNEL_NUM)
          return IA_XHEAAC_DEC_EXE_FATAL_UNIMPLEMENTED_CCE;
        if (ps_enable == 1) {
          channel = 2;
        }

        if (p_obj_exhaacplus_dec->aac_config.element_type[i] != 2) {
          channel_check += channel;
        }

        if (channel_check > max_ch_num) {
          p_state_enhaacplus_dec->i_bytes_consumed = 1;
          return IA_XHEAAC_DEC_INIT_FATAL_STREAM_CHAN_GT_MAX;
        }

        temp = p_state_enhaacplus_dec->aac_persistent_mem_v;

        prev_persistent_used_t = persistent_used_t;

        ixheaacd_allocate_mem_persistent(
            p_obj_exhaacplus_dec, p_state_enhaacplus_dec, channel,
            &persistent_used_t, &prev_sbrpersistent_used_t, ps_enable);

        p_state_enhaacplus_dec->aac_persistent_mem_v = temp;
        p_state_enhaacplus_dec->last_frame_ok = 1;

        p_state_enhaacplus_dec->num_channel_last = 0;
        p_state_enhaacplus_dec->ui_init_done = 0;
        p_state_enhaacplus_dec->ui_input_over = 0;
        p_state_enhaacplus_dec->ptr_bit_stream =
            p_state_enhaacplus_dec->pstr_bit_buf;

        p_state_enhaacplus_dec->pstr_aac_dec_info[i] = 0;

        p_state_enhaacplus_dec->pstr_aac_dec_info[i] =
            ixheaacd_aac_decoder_init(
                p_state_enhaacplus_dec,
                p_state_enhaacplus_dec->pstr_stream_sbr[i], channel,
                (WORD8 *)p_state_enhaacplus_dec->aac_persistent_mem_v +
                    prev_persistent_used_t,
                p_state_enhaacplus_dec->frame_length);

        if (!p_state_enhaacplus_dec->pstr_aac_dec_info[i]) {
          p_state_enhaacplus_dec->i_bytes_consumed = 1;
          return IA_XHEAAC_DEC_INIT_FATAL_DEC_INIT_FAIL;
        }

        p_state_enhaacplus_dec->str_sbr_dec_info[i] = ixheaacd_init_sbr(
            sample_rate_2, frame_size_2,
            (FLAG *)&p_obj_exhaacplus_dec->aac_config.down_sample_flag,
            p_state_enhaacplus_dec->sbr_persistent_mem_v,
            p_state_enhaacplus_dec->ptr_overlap_buf, channel, ps_enable, 1,
            frame_size_2 * 2, &harmonic_sbr_flag, NULL,
            p_state_enhaacplus_dec->str_sbr_config,
            p_state_enhaacplus_dec->audio_object_type,
            p_state_enhaacplus_dec->mps_dec_handle.ldmps_config
                .ldmps_present_flag,
            p_state_enhaacplus_dec->mps_dec_handle.ldmps_config
                .no_ldsbr_present);
        if (p_state_enhaacplus_dec->str_sbr_dec_info[i]) {
          p_state_enhaacplus_dec->str_sbr_dec_info[i]->xaac_jmp_buf =
              &(p_state_enhaacplus_dec->xaac_jmp_buf);
        }
        if ((sbr_present_flag &&
            ((p_obj_exhaacplus_dec->p_state_aac->audio_object_type ==
              AOT_AAC_LC) ||
             (p_obj_exhaacplus_dec->p_state_aac->audio_object_type ==
              AOT_SBR) ||
             (p_obj_exhaacplus_dec->p_state_aac->audio_object_type ==
               AOT_PS))) ||
            ((p_state_enhaacplus_dec->mps_dec_handle.ldmps_config
                  .ldmps_present_flag == 1) &&
             (p_obj_exhaacplus_dec->p_state_aac->audio_object_type ==
              AOT_ER_AAC_ELD)))
          p_obj_exhaacplus_dec->aac_config.flag_to_stereo = 1;
        copy_qmf_to_ldmps(&p_obj_exhaacplus_dec->p_state_aac->mps_dec_handle,
                          p_state_enhaacplus_dec->sbr_persistent_mem_v);
        if (p_state_enhaacplus_dec->audio_object_type == AOT_AAC_LC &&
            p_state_enhaacplus_dec->ui_mps_out_bytes != 0) {
          p_state_enhaacplus_dec->heaac_mps_handle.heaac_mps_present = 1;
          if (p_state_enhaacplus_dec->pstr_stream_sbr[0][0].no_elements) {
            p_obj_exhaacplus_dec->p_state_aac->heaac_mps_handle.mps_with_sbr = 1;
          }
          error_code =
              ixheaacd_aac_mps_init(p_obj_exhaacplus_dec, mps_buffer,
                                    p_state_enhaacplus_dec->ui_mps_out_bytes, sample_rate_1);
          if (error_code) return error_code;
          p_obj_exhaacplus_dec->aac_config.ui_n_channels =
              p_state_enhaacplus_dec->heaac_mps_handle.num_output_channels_at;
          if (p_obj_exhaacplus_dec->p_state_aac->heaac_mps_handle.mps_with_sbr == 1) {
            p_obj_exhaacplus_dec->aac_config.ui_sbr_mode = 1;
          }
          if (p_obj_exhaacplus_dec->aac_config.element_type[i + 1] >= 0 &&
              p_obj_exhaacplus_dec->aac_config.element_type[i + 1] <= 3) {
            return IA_FATAL_ERROR;
          }
          break;
        }
        i++;
      }
      p_state_enhaacplus_dec->pers_mem_ptr =
          (WORD8 *)p_state_enhaacplus_dec->aac_persistent_mem_v +
          persistent_used_t;

      p_obj_exhaacplus_dec->aac_config.i_channel_mask =
          ixheaacd_get_channel_mask(p_obj_exhaacplus_dec);

      {
        num_channels_1 = 0;
        ch_idx = 0;
        while (p_obj_exhaacplus_dec->aac_config.element_type[ch_idx] >= 0 &&
               p_obj_exhaacplus_dec->aac_config.element_type[ch_idx] <= 3) {
          if (p_obj_exhaacplus_dec->aac_config.element_type[ch_idx] == 0 ||
              p_obj_exhaacplus_dec->aac_config.element_type[ch_idx] == 3)
            num_channels_1 += 1;
          if (p_obj_exhaacplus_dec->aac_config.element_type[ch_idx] == 1)
            num_channels_1 += 2;
          ch_idx++;
        }

        if (ch_idx == 2 && num_channels_1 == 2) {
          p_obj_exhaacplus_dec->aac_config.ui_channel_mode = 2;
        }
        if (ch_idx == 1) {
          if (num_channels_1 == 1)
            p_obj_exhaacplus_dec->aac_config.ui_channel_mode = 0;
          if (num_channels_1 == 2)
            p_obj_exhaacplus_dec->aac_config.ui_channel_mode = 1;
        }

        if (ps_detected == 1 && num_channels_1 == 1) num_channels_1 = 2;
      }
    }
    if (1 == p_obj_exhaacplus_dec->aac_config.downmix) num_channels_1 = 2;

    if (p_obj_exhaacplus_dec->aac_config.flag_downmix == 1) {
      num_channels_1 = 1;
    }

    if ((p_obj_exhaacplus_dec->aac_config.flag_to_stereo == 1) &&
        (ch_idx == 1 || num_channels_1 <= 2)) {
      num_channels_1 = 2;
    }

    p_obj_exhaacplus_dec->aac_config.ui_n_channels = num_channels_1;
    p_obj_exhaacplus_dec->aac_config.ui_samp_freq = sample_rate;
    p_state_enhaacplus_dec->ui_init_done = 1;

    memcpy(it_bit_buff, &temp_bit_buff, sizeof(struct ia_bit_buf_struct));

    p_state_enhaacplus_dec->b_n_raw_data_blk = 0;

    if (p_obj_exhaacplus_dec->p_state_aac->header_dec_done == 1) {
      p_obj_exhaacplus_dec->p_state_aac->header_dec_done = 0;
    }
  }
  return err_code;
}

VOID ixheaacd_fill_slot_order(ia_aac_dec_state_struct *p_state_enhaacplus_dec,
                              WORD32 ch, WORD8 *ptr_is_cpe,
                              WORD8 *ptr_tag_select, WORD32 *ptr_idx_no) {
  WORD32 i;
  WORD32 idx_no = *ptr_idx_no;
  WORD *p_slot_element = p_state_enhaacplus_dec->p_config->slot_element;
  WORD *p_element_type = p_state_enhaacplus_dec->p_config->element_type;
  WORD *p_element_instance_order =
      p_state_enhaacplus_dec->p_config->element_instance_order;

  for (i = 0; i < ch; i++) {
    if (ptr_is_cpe[i] == 0) {
      *p_slot_element++ = idx_no++;
      *p_element_type++ = 0;
      *p_element_instance_order++ = ptr_tag_select[i];
    }
  }
  *ptr_idx_no = idx_no;
}

VOID ixheaacd_fill_prog_config_slots(
    ia_aac_dec_state_struct *p_state_enhaacplus_dec) {
  WORD32 idx_no = 0;

  ixheaacd_fill_slot_order(
      p_state_enhaacplus_dec, p_state_enhaacplus_dec->p_config->str_prog_config
                                  .num_front_channel_elements,
      p_state_enhaacplus_dec->p_config->str_prog_config.front_element_is_cpe,
      p_state_enhaacplus_dec->p_config->str_prog_config
          .front_element_tag_select,
      &idx_no);

  ixheaacd_fill_slot_order(
      p_state_enhaacplus_dec, p_state_enhaacplus_dec->p_config->str_prog_config
                                  .num_side_channel_elements,
      p_state_enhaacplus_dec->p_config->str_prog_config.side_element_is_cpe,
      p_state_enhaacplus_dec->p_config->str_prog_config.side_element_tag_select,
      &idx_no);

  ixheaacd_fill_slot_order(
      p_state_enhaacplus_dec, p_state_enhaacplus_dec->p_config->str_prog_config
                                  .num_back_channel_elements,
      p_state_enhaacplus_dec->p_config->str_prog_config.back_element_is_cpe,
      p_state_enhaacplus_dec->p_config->str_prog_config.back_element_tag_select,
      &idx_no);
}

IA_ERRORCODE ixheaacd_dec_execute(
    ia_exhaacplus_dec_api_struct *p_obj_exhaacplus_dec) {
  ia_adts_header_struct adts = {0};
  ia_aac_dec_state_struct *p_state_enhaacplus_dec;

  UWORD8 *in_buffer;
  WORD16 *time_data;
  WORD16 num_of_out_samples = 0;
  WORD16 frame_size = 0;
  WORD32 sample_rate_dec = 0;
  WORD32 sample_rate = 0;
  WORD16 num_ch = 0;
  struct ia_bit_buf_struct *it_bit_buff;
  UWORD8 *mps_buffer;
  WORD32 error_code = IA_NO_ERROR;
  WORD ch_idx1;
  WORD type;
  WORD total_channels = 0;
  WORD total_elements = 0;
  WORD16 *actual_out_buffer = NULL;
  WORD ps_enable;
  WORD esbr_mono_downmix = 0;
  WORD8 element_used[MAX_BS_ELEMENT];
  WORD32 channel_coupling_flag = 0;

  SIZE_T bytes_for_sync;
  WORD32 audio_mux_length_bytes_last = 0;
  WORD32 ret_val;
  WORD32 mps_out_samples;

  p_obj_exhaacplus_dec->aac_config.ui_sbr_mode = 0;
  p_obj_exhaacplus_dec->aac_config.frame_status = 1;

  if (p_obj_exhaacplus_dec->p_state_aac != NULL) {
    ret_val = setjmp(p_obj_exhaacplus_dec->p_state_aac->xaac_jmp_buf);
    if (ret_val != 0) {
      p_obj_exhaacplus_dec->p_state_aac->i_bytes_consumed =
          p_obj_exhaacplus_dec->p_state_aac->ui_in_bytes;
      p_obj_exhaacplus_dec->p_state_aac->ui_out_bytes = 0;
      p_obj_exhaacplus_dec->aac_config.frame_status = 0;
      return IA_NO_ERROR;
    }
  }

  time_data = (WORD16 *)(p_obj_exhaacplus_dec
                             ->pp_mem_aac[IA_ENHAACPLUS_DEC_OUTPUT_IDX]);
  in_buffer = p_obj_exhaacplus_dec->pp_mem_aac[IA_ENHAACPLUS_DEC_INPUT_IDX];
  p_state_enhaacplus_dec = p_obj_exhaacplus_dec->p_state_aac;
  p_state_enhaacplus_dec->aac_scratch_mem_v =
      p_obj_exhaacplus_dec->pp_mem_aac[IA_ENHAACPLUS_DEC_SCRATCH_IDX];
  p_state_enhaacplus_dec->mps_header = -1;
  mps_buffer = p_state_enhaacplus_dec->mps_buffer;
  it_bit_buff = p_state_enhaacplus_dec->pstr_bit_buf;

  ch_idx1 = 0;
  p_state_enhaacplus_dec->i_bytes_consumed = 0;

  if (p_state_enhaacplus_dec->audio_object_type == AOT_USAC) {
    WORD32 pcm_size = p_obj_exhaacplus_dec->aac_config.ui_pcm_wdsz;
    WORD8 *inbuffer = (WORD8 *)(p_obj_exhaacplus_dec
                                    ->pp_mem_aac[IA_ENHAACPLUS_DEC_INPUT_IDX]);
    WORD8 *outbuffer =
        (WORD8 *)(p_obj_exhaacplus_dec
                      ->pp_mem_aac[IA_ENHAACPLUS_DEC_OUTPUT_IDX]);
    WORD32 out_bytes = 0;

    WORD32 frames_done = p_obj_exhaacplus_dec->p_state_aac->frame_counter;

    ia_dec_data_struct *pstr_dec_data =
        (ia_dec_data_struct *)(p_obj_exhaacplus_dec->p_state_aac
                                   ->pstr_dec_data);

    if (pstr_dec_data->str_usac_data.down_samp_sbr != 0) return IA_FATAL_ERROR;

    if (p_obj_exhaacplus_dec->p_state_aac->ui_input_over == 0) {
      ia_audio_specific_config_struct *ptr_audio_specific_config =
          ((ia_audio_specific_config_struct *)
               p_obj_exhaacplus_dec->p_state_aac->ia_audio_specific_config);

      ptr_audio_specific_config->str_usac_config.str_usac_dec_config
          .preroll_counter = 0;
      {
        WORD32 iii = 0;
        for (iii = 0; iii < (MAX_AUDIO_PREROLLS + 1); iii++) {
          ((ia_dec_data_struct *)(p_obj_exhaacplus_dec->p_state_aac
                                      ->pstr_dec_data))
              ->str_frame_data.str_audio_specific_config.str_usac_config
              .str_usac_dec_config.usac_ext_gain_payload_len[iii] = 0;
          ptr_audio_specific_config->str_usac_config.str_usac_dec_config
              .usac_ext_gain_payload_len[iii] =
              0;  // reinitilize the payload len buff
          ptr_audio_specific_config->str_usac_config.str_usac_dec_config
              .preroll_bytes[iii] = 0;
        }
      }

      ((ia_dec_data_struct *)(p_obj_exhaacplus_dec->p_state_aac->pstr_dec_data))
          ->str_frame_data.str_audio_specific_config.str_usac_config
          .str_usac_dec_config.preroll_counter = 0;
      error_code = ixheaacd_dec_main(
          p_obj_exhaacplus_dec, inbuffer, outbuffer, &out_bytes, frames_done,
          pcm_size, &p_obj_exhaacplus_dec->p_state_aac->num_of_output_ch);
      if (error_code) {
        if (p_obj_exhaacplus_dec->aac_config.ui_err_conceal) {
          p_obj_exhaacplus_dec->aac_config.frame_status = 0;
        } else {
          return error_code;
        }
      }
      p_obj_exhaacplus_dec->p_state_aac->frame_counter++;
    } else {
      out_bytes = 0;
    }

    if (pstr_dec_data->str_usac_data.ec_flag == 0) {
      if (p_state_enhaacplus_dec->bs_format != LOAS_BSFORMAT) {
        p_obj_exhaacplus_dec->p_state_aac->i_bytes_consumed =
            p_obj_exhaacplus_dec->p_state_aac->ui_in_bytes;
      }
    } else {
      if (p_state_enhaacplus_dec->bs_format != LOAS_BSFORMAT) {
        if (pstr_dec_data->str_usac_data.frame_ok == 0) {
          p_obj_exhaacplus_dec->p_state_aac->i_bytes_consumed =
              p_obj_exhaacplus_dec->p_state_aac->ui_in_bytes;
          pstr_dec_data->dec_bit_buf.cnt_bits = 0;
        } else {
          ia_dec_data_struct *pstr_dec_data =
              (ia_dec_data_struct *)p_obj_exhaacplus_dec->p_state_aac->pstr_dec_data;

          if (pstr_dec_data->dec_bit_buf.cnt_bits & 7) {
            pstr_dec_data->dec_bit_buf.cnt_bits -= (pstr_dec_data->dec_bit_buf.cnt_bits & 7);
          }
          if (pstr_dec_data->dec_bit_buf.cnt_bits == 0) {
            p_obj_exhaacplus_dec->p_state_aac->i_bytes_consumed =
                p_obj_exhaacplus_dec->p_state_aac->ui_in_bytes;
          } else {
            p_obj_exhaacplus_dec->p_state_aac->i_bytes_consumed =
                p_obj_exhaacplus_dec->p_state_aac->ui_in_bytes -
                (pstr_dec_data->dec_bit_buf.cnt_bits >> 3);
          }
        }
      }
    }

    p_obj_exhaacplus_dec->p_state_aac->ui_out_bytes = out_bytes;
    p_obj_exhaacplus_dec->aac_config.ui_n_channels =
        p_obj_exhaacplus_dec->p_state_aac->num_of_output_ch;
    pstr_dec_data->str_usac_data.sbr_parse_err_flag = 0;

    return 0;
  }

  while (p_obj_exhaacplus_dec->aac_config.element_type[ch_idx1] <= 3 &&
         p_obj_exhaacplus_dec->aac_config.element_type[ch_idx1] >= 0) {
    if (p_obj_exhaacplus_dec->aac_config.element_type[ch_idx1] == 0 ||
        p_obj_exhaacplus_dec->aac_config.element_type[ch_idx1] == 3) {
      total_channels += 1;
      total_elements += 1;
    }
    if (p_obj_exhaacplus_dec->aac_config.element_type[ch_idx1] == 1) {
      total_elements += 1;
      total_channels += 2;
    }
    if (p_obj_exhaacplus_dec->aac_config.element_type[ch_idx1] == 2) {
      total_elements += 1;
    }

    ch_idx1++;
    if (ch_idx1 > MAX_BS_ELEMENT) {
      if (p_obj_exhaacplus_dec->aac_config.ui_err_conceal)
        break;
      else
        return IA_FATAL_ERROR;
    }
  }

  if (ch_idx1 != 1) {
    ps_enable = 0;
    if (p_obj_exhaacplus_dec->aac_config.ui_max_channels > 2) {
      WORD32 scratch_pointer;

      scratch_pointer = (MAX_SCR_SIZE - SCR_INTER_SCR_SIZE);

      p_state_enhaacplus_dec->coup_ch_output =
          (WORD32 *)((WORD8 *)
                         p_obj_exhaacplus_dec->p_state_aac->aac_scratch_mem_v +
                     scratch_pointer);
    }

  }

  else {
    if (total_channels < (WORD)p_obj_exhaacplus_dec->aac_config.ui_n_channels)
      total_channels = p_obj_exhaacplus_dec->aac_config.ui_n_channels;
    ps_enable = 1;
  }

  p_obj_exhaacplus_dec->p_state_aac->ui_out_bytes = 0;
  p_obj_exhaacplus_dec->p_state_aac->ui_mps_out_bytes = 0;
  if (p_state_enhaacplus_dec->ui_in_bytes == 0) {
    UWORD32 i;
    WORD32 j;
    if (p_state_enhaacplus_dec->peak_lim_init == 1) {
      p_obj_exhaacplus_dec->p_state_aac->ui_out_bytes =
          (p_state_enhaacplus_dec->peak_limiter.attack_time_samples) *
          total_channels * sizeof(WORD16);

      for (j = 0; j < total_channels; j++) {
        for (i = 0;
             i < (p_state_enhaacplus_dec->peak_limiter.attack_time_samples -
                  p_state_enhaacplus_dec->peak_limiter.delayed_input_index);
             i++) {
          *(time_data + total_channels * i + j) = ixheaac_round16(
              (WORD32)*(p_state_enhaacplus_dec->peak_limiter.delayed_input +
                (p_state_enhaacplus_dec->peak_limiter.delayed_input_index) *
                    total_channels +
                total_channels * i + j));
        }
      }

      for (j = 0; j < total_channels; j++) {
        for (i = 0;
             i < p_state_enhaacplus_dec->peak_limiter.delayed_input_index;
             i++) {
          *(time_data +
            (p_state_enhaacplus_dec->peak_limiter.attack_time_samples -
             p_state_enhaacplus_dec->peak_limiter.delayed_input_index) *
                total_channels +
            total_channels * i + j) =
              ixheaac_round16(
                  (WORD32)*(p_state_enhaacplus_dec->peak_limiter.delayed_input +
                   total_channels * i + j));
        }
      }

      if (p_obj_exhaacplus_dec->aac_config.dup_stereo_flag) {
        for (i = 0;
             i < (p_state_enhaacplus_dec->peak_limiter.attack_time_samples);
             i++) {
          time_data[2 * i + 1] = time_data[2 * i + 0];
        }
      }
    } else {
      p_obj_exhaacplus_dec->p_state_aac->ui_out_bytes = 0;
    }

    p_state_enhaacplus_dec->i_bytes_consumed = 0;
    return IA_NO_ERROR;
  }

  if (ch_idx1 == 0) {
    p_state_enhaacplus_dec->i_bytes_consumed = 1;
    if (p_obj_exhaacplus_dec->aac_config.ui_err_conceal) {
      p_obj_exhaacplus_dec->aac_config.frame_status = 0;
    } else {
      return IA_XHEAAC_DEC_EXE_NONFATAL_DECODE_FRAME_ERROR;
    }
  }
  if (total_channels > (WORD)p_obj_exhaacplus_dec->aac_config.ui_max_channels) {
    p_state_enhaacplus_dec->i_bytes_consumed = 1;
    if (p_obj_exhaacplus_dec->aac_config.ui_err_conceal) {
      p_obj_exhaacplus_dec->aac_config.frame_status = 0;
    } else {
      return IA_XHEAAC_DEC_CONFIG_NONFATAL_INVALID_MAX_CHANNEL;
    }
  }

  if (p_state_enhaacplus_dec->audio_object_type == AOT_ER_AAC_LD ||
      p_state_enhaacplus_dec->audio_object_type == AOT_ER_AAC_ELD) {
    if (p_obj_exhaacplus_dec->aac_config.ui_mp4_flag)
      p_state_enhaacplus_dec->frame_size = p_state_enhaacplus_dec->ui_in_bytes;
  }

  if (p_state_enhaacplus_dec->audio_object_type == AOT_ER_AAC_LC) {
    if (p_obj_exhaacplus_dec->aac_config.ui_mp4_flag)
      p_state_enhaacplus_dec->frame_size = 1024;
  }

  {
    ixheaacd_create_init_bit_buf(it_bit_buff, in_buffer,
                                 p_state_enhaacplus_dec->ui_in_bytes);
    it_bit_buff->xaac_jmp_buf = &(p_state_enhaacplus_dec->xaac_jmp_buf);

    it_bit_buff->adts_header_present =
        p_state_enhaacplus_dec->s_adts_hdr_present;
    it_bit_buff->no_raw_data_blocks =
        (WORD8)p_state_enhaacplus_dec->b_n_raw_data_blk;
    it_bit_buff->protection_absent = p_state_enhaacplus_dec->protection_absent;

    if (p_state_enhaacplus_dec->s_adts_hdr_present) {
      if (p_state_enhaacplus_dec->b_n_raw_data_blk == 0) {
        WORD32 error;

        error = ixheaacd_readifadts(p_state_enhaacplus_dec, it_bit_buff, &adts);
        if (error) {
          if (p_obj_exhaacplus_dec->aac_config.ui_err_conceal) {
            p_obj_exhaacplus_dec->aac_config.frame_status = 0;
            if (adts.samp_freq_index > 11) {
              adts.samp_freq_index = 11;
            }
          } else {
            return error;
          }
        }

        if ((WORD32)p_state_enhaacplus_dec->sampling_rate !=
            (WORD32)((p_obj_exhaacplus_dec->aac_tables.pstr_huffmann_tables
                          ->str_sample_rate_info[adts.samp_freq_index]
                          .sampling_frequency))) {
          p_state_enhaacplus_dec->i_bytes_consumed = 0;
          if (p_obj_exhaacplus_dec->aac_config.ui_err_conceal) {
            p_obj_exhaacplus_dec->aac_config.frame_status = 0;
          } else {
            return IA_XHEAAC_DEC_EXE_NONFATAL_CHANGED_ADTS_SF;
          }
        }
      }
    }

    bytes_for_sync = (SIZE_T)it_bit_buff->ptr_read_next;

    if (p_state_enhaacplus_dec->bs_format == LOAS_BSFORMAT) {
      WORD32 result, audio_mux_len_bytes_last;
      WORD32 cnt_bits = it_bit_buff->cnt_bits;
      WORD32 sync = ixheaacd_read_bits_buf(it_bit_buff, 11);
      UWORD32 curr_samp_rate = 0;

      if (p_state_enhaacplus_dec->latm_initialized)
        curr_samp_rate =
            p_state_enhaacplus_dec->latm_struct_element.layer_info[0][0]
                .asc.sampling_freq;

      while (sync != 0x2b7) {
        sync = ((sync & 0x3ff) << 1) | ixheaacd_read_bits_buf(it_bit_buff, 1);
        if (it_bit_buff->cnt_bits < 13) {
          ixheaacd_read_bidirection(it_bit_buff, -11);
          p_state_enhaacplus_dec->i_bytes_consumed =
              (cnt_bits - it_bit_buff->cnt_bits) / 8;

          if (p_state_enhaacplus_dec->i_bytes_consumed == 0)
            p_state_enhaacplus_dec->i_bytes_consumed = 1;
          if (p_obj_exhaacplus_dec->aac_config.ui_err_conceal) {
            p_obj_exhaacplus_dec->aac_config.frame_status = 0;
          } else {
            return (IA_XHEAAC_DEC_INIT_NONFATAL_HEADER_NOT_AT_START);
          }
        }
      }

      it_bit_buff->audio_mux_align = it_bit_buff->cnt_bits - 13;

      audio_mux_len_bytes_last = ixheaacd_read_bits_buf(it_bit_buff, 13);

      audio_mux_length_bytes_last = audio_mux_len_bytes_last;

      bytes_for_sync = (SIZE_T)it_bit_buff->ptr_read_next - bytes_for_sync;

      if (it_bit_buff->cnt_bits < (audio_mux_len_bytes_last << 3)) {
        ixheaacd_read_bidirection(it_bit_buff, -(13 + 11));
        p_state_enhaacplus_dec->i_bytes_consumed = (cnt_bits - it_bit_buff->cnt_bits) / 8;
        if (p_obj_exhaacplus_dec->aac_config.ui_err_conceal) {
          p_obj_exhaacplus_dec->aac_config.frame_status = 0;
        } else {
          return IA_XHEAAC_DEC_EXE_NONFATAL_INSUFFICIENT_INPUT_BYTES;
        }
      } else {
        ixheaacd_read_bidirection(it_bit_buff, -(13));
      }

      if (sync == 0x2b7) {
        result = ixheaacd_latm_audio_mux_element(
            it_bit_buff, &p_state_enhaacplus_dec->latm_struct_element,
            p_state_enhaacplus_dec,
            (ia_sampling_rate_info_struct *)&p_obj_exhaacplus_dec->aac_tables
                .pstr_huffmann_tables->str_sample_rate_info[0]);
        if (result < 0) {
          if (p_obj_exhaacplus_dec->aac_config.ui_err_conceal) {
            p_obj_exhaacplus_dec->aac_config.frame_status = 0;
          } else {
            return result;
          }
        }
        if (!p_state_enhaacplus_dec->latm_initialized) {
          p_state_enhaacplus_dec->sampling_rate =
              p_state_enhaacplus_dec->latm_struct_element.layer_info[0][0].asc.sampling_freq;
          p_state_enhaacplus_dec->latm_initialized = 1;
        } else {
          if (p_state_enhaacplus_dec->sampling_rate != curr_samp_rate) {
            p_state_enhaacplus_dec->i_bytes_consumed = 0;
            if (p_obj_exhaacplus_dec->aac_config.ui_err_conceal) {
              p_obj_exhaacplus_dec->aac_config.frame_status = 0;
            } else {
              return IA_XHEAAC_DEC_EXE_NONFATAL_CHANGED_ADTS_SF;
            }
          }
        }
      }
    }
  }

  if (total_elements == 2 && total_channels == 2 &&
      (p_state_enhaacplus_dec->p_config->ui_pce_found_in_hdr == 1 ||
       p_state_enhaacplus_dec->p_config->ui_pce_found_in_hdr == 3)) {
    ixheaacd_fill_prog_config_slots(p_state_enhaacplus_dec);
  }

  memset(element_used, 0, sizeof(WORD8) * MAX_BS_ELEMENT);

  if (it_bit_buff->cnt_bits <= 0) {
    it_bit_buff->cnt_bits = -1;
    ixheaacd_updatebytesconsumed(p_state_enhaacplus_dec, it_bit_buff);
    if (p_obj_exhaacplus_dec->aac_config.ui_err_conceal) {
      p_obj_exhaacplus_dec->aac_config.frame_status = 0;
    } else {
      return (WORD16)((WORD32)IA_XHEAAC_DEC_EXE_NONFATAL_INSUFFICIENT_INPUT_BYTES);
    }
  }

  { it_bit_buff->initial_cnt_bits = it_bit_buff->cnt_bits; }

  if (p_state_enhaacplus_dec->audio_object_type == AOT_ER_AAC_LD) {
    if (p_state_enhaacplus_dec->s_adts_hdr_present)
      p_state_enhaacplus_dec->frame_size = adts.aac_frame_length;
  }

  if (p_state_enhaacplus_dec->audio_object_type == AOT_ER_AAC_LC) {
    if (p_state_enhaacplus_dec->s_adts_hdr_present)
      p_state_enhaacplus_dec->frame_size = 1024;
  }

  if (p_state_enhaacplus_dec->pstr_drc_dec) {
    p_state_enhaacplus_dec->pstr_drc_dec->num_drc_elements = 0;

    p_state_enhaacplus_dec->pstr_drc_dec->state = 1;
  }
  if (p_obj_exhaacplus_dec->aac_config.ui_err_conceal) {
    if (total_elements > MAX_BS_ELEMENT) {
      total_elements = MAX_BS_ELEMENT;
    }
  }

  WORD16 *intermediate_scr = (WORD16 *)(WORD8 *)p_state_enhaacplus_dec->aac_scratch_mem_v +
                             (MAX_SCR_SIZE - SCR_INTER_SCR_SIZE - SCR_COUP_CH_OUT_SIZE);

  for (ch_idx1 = 0; ch_idx1 < total_elements; ch_idx1++) {
    WORD32 skip_full_decode = 0;
    WORD32 ch_idx = ch_idx1;
    WORD32 channel = 0;
    WORD ch_fac, slot_ele;

    if (p_state_enhaacplus_dec->audio_object_type < ER_OBJECT_START ||
        (p_state_enhaacplus_dec->audio_object_type != AOT_ER_AAC_LD &&
         p_state_enhaacplus_dec->audio_object_type != AOT_ER_AAC_ELD &&
         p_state_enhaacplus_dec->audio_object_type != AOT_ER_AAC_LC)) {
      jmp_buf local;

      if (p_obj_exhaacplus_dec->aac_config.ui_err_conceal == 1) {
        ret_val = setjmp(local);
      }
      if (ret_val == 0) {
        if (p_obj_exhaacplus_dec->aac_config.ui_err_conceal == 1) {
          p_obj_exhaacplus_dec->p_state_aac->ptr_bit_stream->xaac_jmp_buf = &local;
        }
        error_code = ixheaacd_get_element_index_tag(
            p_obj_exhaacplus_dec, ch_idx1, &ch_idx, &channel,
            p_obj_exhaacplus_dec->aac_config.element_instance_order, total_elements, element_used,
            total_channels, p_state_enhaacplus_dec->pstr_drc_dec,
            &p_state_enhaacplus_dec->drc_dummy, mps_buffer, &p_state_enhaacplus_dec->mps_header,
            &p_state_enhaacplus_dec->ui_mps_out_bytes);
      }

      if (error_code || ret_val) {
        ixheaacd_updatebytesconsumed(p_state_enhaacplus_dec, it_bit_buff);
        if (it_bit_buff->cnt_bits < 0) {
          p_state_enhaacplus_dec->ui_out_bytes = 0;
          p_state_enhaacplus_dec->ui_mps_out_bytes = 0;
          p_state_enhaacplus_dec->b_n_raw_data_blk = 0;
        }
        p_state_enhaacplus_dec->i_bytes_consumed = 1;
        p_state_enhaacplus_dec->b_n_raw_data_blk = 0;
        if (p_obj_exhaacplus_dec->aac_config.ui_err_conceal) {
          p_obj_exhaacplus_dec->aac_config.frame_status = 0;
        } else {
          return error_code;
        }
      }
      if (p_obj_exhaacplus_dec->aac_config.element_type[ch_idx] == ID_CPE)
      {
        if (channel != 2)
        {
          if (p_obj_exhaacplus_dec->aac_config.ui_err_conceal)
          {
            p_obj_exhaacplus_dec->aac_config.frame_status = 0;
            channel = 2;
          }
          else
          {
            return IA_FATAL_ERROR;
          }
        }
      }
      else
      {
        if (channel != 1)
        {
          if (p_obj_exhaacplus_dec->aac_config.ui_err_conceal)
          {
            p_obj_exhaacplus_dec->aac_config.frame_status = 0;
            channel = 1;
          }
          else
          {
            return IA_FATAL_ERROR;
          }
        }
      }
    } else {
      if (p_obj_exhaacplus_dec->aac_config.element_type[ch_idx] == ID_SCE)
        channel = 1;
      else
        channel = 2;
    }

    if (p_obj_exhaacplus_dec->aac_config.ui_err_conceal && (error_code || ret_val)) {
      if (p_obj_exhaacplus_dec->aac_config.element_type[ch_idx] == 0 ||
          p_obj_exhaacplus_dec->aac_config.element_type[ch_idx] == 3) {
        if (channel > 1) {
          channel = 1;
        }
      }
      if (p_obj_exhaacplus_dec->aac_config.element_type[ch_idx] == 1) {
        if (channel > 2) {
          channel = 2;
        }
      }
      if (p_obj_exhaacplus_dec->aac_config.element_type[ch_idx] == 2) {
        if (p_obj_exhaacplus_dec->aac_config.ui_max_channels > 2) {
          if (!(p_obj_exhaacplus_dec->aac_config.element_instance_order[ch_idx] !=
                p_obj_exhaacplus_dec->aac_config.ui_coupling_channel)) {
            if (channel > 1) {
              channel = 1;
            }
          }
        }
      }
      if (ps_enable == 1) {
        if (channel > 2) {
          channel = 2;
        }
      }
    }

    ch_fac = total_channels;
    slot_ele = p_obj_exhaacplus_dec->aac_config.slot_element[ch_idx];
    actual_out_buffer = time_data;
    if (p_obj_exhaacplus_dec->aac_config.element_type[ch_idx] == 2) {
      p_state_enhaacplus_dec->pstr_aac_dec_info[ch_idx]->p_ind_channel_info =
          &p_state_enhaacplus_dec->ind_cc_info;
      if (p_obj_exhaacplus_dec->aac_config.element_instance_order[ch_idx] !=
          p_obj_exhaacplus_dec->aac_config.ui_coupling_channel) {
        skip_full_decode = 1;
        ixheaacd_set_aac_persistent_buffers(
            p_state_enhaacplus_dec->pers_mem_ptr, channel);

        {
          struct ia_aac_persistent_struct *aac_persistent_mem =
              (struct ia_aac_persistent_struct *)
                  p_state_enhaacplus_dec->pers_mem_ptr;
          aac_persistent_mem->str_aac_decoder.pstr_aac_tables =
              &p_obj_exhaacplus_dec->aac_tables;
          aac_persistent_mem->str_aac_decoder.pstr_common_tables =
              p_obj_exhaacplus_dec->common_tables;
        }

        p_state_enhaacplus_dec->pstr_aac_dec_info[ch_idx] = 0;

        p_state_enhaacplus_dec->str_sbr_dec_info[ch_idx] = 0;

        p_state_enhaacplus_dec->pstr_aac_dec_info[ch_idx] =
            ixheaacd_aac_decoder_init(
                p_state_enhaacplus_dec,

                p_state_enhaacplus_dec->pstr_stream_sbr[ch_idx],

                channel, p_state_enhaacplus_dec->pers_mem_ptr,
                p_state_enhaacplus_dec->frame_length

                );
        if (!p_state_enhaacplus_dec->pstr_aac_dec_info[ch_idx]) {
          p_state_enhaacplus_dec->i_bytes_consumed = 1;
          if (p_obj_exhaacplus_dec->aac_config.ui_err_conceal) {
            p_obj_exhaacplus_dec->aac_config.frame_status = 0;
          } else {
            return IA_XHEAAC_DEC_INIT_FATAL_DEC_INIT_FAIL;
          }
        }
        p_state_enhaacplus_dec->pstr_aac_dec_info[ch_idx]->p_ind_channel_info =
            (WORD8 *)p_state_enhaacplus_dec->aac_scratch_mem_v + (P_IND_CH_INFO_OFFSET);
      }
      if (p_obj_exhaacplus_dec->aac_config.element_type[1] < 3 &&
          p_obj_exhaacplus_dec->aac_config.element_type[1] > 0 &&
          p_obj_exhaacplus_dec->aac_config.ui_max_channels > 2) {
        actual_out_buffer =
            (WORD16 *)(VOID *)p_state_enhaacplus_dec->coup_ch_output;
      }
      ch_fac = 1;
      slot_ele = 0;
    }

    type = -1;
    p_state_enhaacplus_dec->pstr_stream_sbr[ch_idx][0].no_elements = 0;

    {
      WORD element_index_order1[MAX_BS_ELEMENT];
      ia_aac_dec_scratch_struct aac_scratch_struct;
      memset(&aac_scratch_struct, 0, sizeof(aac_scratch_struct));

      ixheaacd_allocate_aac_scr(
          &aac_scratch_struct, p_state_enhaacplus_dec->aac_scratch_mem_v,
          time_data, channel, p_obj_exhaacplus_dec->aac_config.ui_max_channels,
          p_state_enhaacplus_dec->audio_object_type);

      if (p_state_enhaacplus_dec->ch_config == 2 && channel == 1) {
        if (p_obj_exhaacplus_dec->aac_config.ui_err_conceal) {
          p_obj_exhaacplus_dec->aac_config.frame_status = 0;
        } else {
          return IA_XHEAAC_DEC_EXE_NONFATAL_DECODE_FRAME_ERROR;
        }
      }

      error_code = ixheaacd_aacdec_decodeframe(
          p_obj_exhaacplus_dec, &aac_scratch_struct, actual_out_buffer,
          p_obj_exhaacplus_dec->aac_config.frame_status, &type, &ch_idx, 0, channel,
          element_index_order1, skip_full_decode, ch_fac, slot_ele,
          p_obj_exhaacplus_dec->aac_config.ui_max_channels, total_channels,
          p_obj_exhaacplus_dec->p_state_aac->frame_length,
          p_obj_exhaacplus_dec->p_state_aac->frame_size, p_state_enhaacplus_dec->pstr_drc_dec,
          p_state_enhaacplus_dec->audio_object_type, p_state_enhaacplus_dec->ch_config,
          p_state_enhaacplus_dec->eld_specific_config, p_state_enhaacplus_dec->s_adts_hdr_present,
          &p_state_enhaacplus_dec->drc_dummy, p_state_enhaacplus_dec->ldmps_present,
          &p_state_enhaacplus_dec->slot_pos, mps_buffer, &p_state_enhaacplus_dec->mps_header,
          &p_state_enhaacplus_dec->ui_mps_out_bytes, 0,
          p_obj_exhaacplus_dec->aac_config.first_frame);

      p_state_enhaacplus_dec->slot_pos -= (channel - 1);
      p_state_enhaacplus_dec->sbr_present = 0;

      if (p_obj_exhaacplus_dec->p_state_aac->qshift_adj[0] != LD_OBJ &&
          p_state_enhaacplus_dec->frame_counter == 0) {
        ixheaacd_peak_limiter_init(
            &p_state_enhaacplus_dec->peak_limiter, total_channels,
            p_obj_exhaacplus_dec->p_state_aac->p_config->ui_samp_freq,
            &p_state_enhaacplus_dec->peak_limiter.buffer[0],
            &p_obj_exhaacplus_dec->p_state_aac->delay_in_samples);
        p_obj_exhaacplus_dec->p_state_aac->peak_lim_init = 1;
      }

      if (p_state_enhaacplus_dec->audio_object_type < ER_OBJECT_START ||
          (p_state_enhaacplus_dec->audio_object_type != AOT_ER_AAC_LD &&
           p_state_enhaacplus_dec->audio_object_type != AOT_ER_AAC_ELD &&
           p_state_enhaacplus_dec->audio_object_type != AOT_ER_AAC_LC)) {
        if ((error_code == 0) && ((ch_idx1 + 1) == total_elements) &&
            (type != ID_END)) {
          {
            p_state_enhaacplus_dec->i_bytes_consumed = (WORD32)(
                it_bit_buff->ptr_read_next - it_bit_buff->ptr_bit_buf_base);
            p_state_enhaacplus_dec->b_n_raw_data_blk = 0;
            if (p_obj_exhaacplus_dec->aac_config.ui_err_conceal) {
              p_obj_exhaacplus_dec->aac_config.frame_status = 0;
            } else {
            return IA_XHEAAC_DEC_EXE_NONFATAL_ELE_INSTANCE_TAG_NOT_FOUND;
          }
          }
        }
      }

      num_ch = p_state_enhaacplus_dec->pstr_aac_dec_info[ch_idx]->channels;
      if (skip_full_decode == 0) {
        if (p_state_enhaacplus_dec->audio_object_type == AOT_ER_AAC_ELD ||
            p_state_enhaacplus_dec->audio_object_type == AOT_ER_AAC_LD)
          frame_size = p_state_enhaacplus_dec->frame_length;
        else {
          frame_size = p_state_enhaacplus_dec->frame_length;
        }

        sample_rate_dec =
            p_state_enhaacplus_dec->pstr_aac_dec_info[ch_idx]->sampling_rate;
      }
    }

    if (skip_full_decode == 1) {
      p_state_enhaacplus_dec->pstr_stream_sbr[ch_idx][0].no_elements = 0;
    }

    if (p_state_enhaacplus_dec->pstr_stream_sbr[ch_idx][0].no_elements != 0) {
      p_obj_exhaacplus_dec->aac_config.ui_sbr_mode = 1;
    }

    if (error_code) {
      if (p_state_enhaacplus_dec->ui_input_over) {
        if (p_obj_exhaacplus_dec->aac_config.ui_err_conceal) {
          p_obj_exhaacplus_dec->aac_config.frame_status = 0;
        } else {
          return IA_XHEAAC_DEC_INIT_FATAL_EO_INPUT_REACHED;
        }
      }
      ixheaacd_updatebytesconsumed(p_state_enhaacplus_dec, it_bit_buff);
      p_obj_exhaacplus_dec->p_state_aac->ui_out_bytes +=
          p_state_enhaacplus_dec->num_of_out_samples * num_ch * sizeof(WORD16);
      if (error_code) {
        if (p_obj_exhaacplus_dec->aac_config.ui_err_conceal) {
          p_obj_exhaacplus_dec->aac_config.frame_status = 0;
        } else {
          return error_code;
        }
      }
    }

    error_code = IA_NO_ERROR;

    if (p_obj_exhaacplus_dec->aac_config.ui_auto_sbr_upsample == 0) {
      if (p_state_enhaacplus_dec->pstr_stream_sbr[ch_idx][0].no_elements == 0 &&
          p_state_enhaacplus_dec->str_sbr_dec_info[ch_idx]) {
        p_state_enhaacplus_dec->str_sbr_dec_info[ch_idx] = 0;
        error_code = IA_XHEAAC_DEC_EXE_NONFATAL_SBR_TURNED_OFF;
      }
    }
    if ((!p_state_enhaacplus_dec->str_sbr_dec_info[ch_idx]) &&
        p_state_enhaacplus_dec->pstr_stream_sbr[ch_idx][0].no_elements) {
      WORD32 harmonic_sbr_flag = 0;
      error_code = IA_XHEAAC_DEC_EXE_NONFATAL_SBR_TURNED_ON;
      p_state_enhaacplus_dec->str_sbr_dec_info[ch_idx] = ixheaacd_init_sbr(
          sample_rate_dec, frame_size,
          (FLAG *)&p_obj_exhaacplus_dec->aac_config.down_sample_flag,
          p_state_enhaacplus_dec->sbr_persistent_mem_v,
          p_state_enhaacplus_dec->ptr_overlap_buf, ps_enable ? 2 : channel,
          ps_enable, 1, frame_size * 2, &harmonic_sbr_flag, NULL,
          p_state_enhaacplus_dec->str_sbr_config,
          p_state_enhaacplus_dec->audio_object_type,
          p_state_enhaacplus_dec->mps_dec_handle.ldmps_config
              .ldmps_present_flag,
          p_state_enhaacplus_dec->mps_dec_handle.ldmps_config.no_ldsbr_present);
      if (p_state_enhaacplus_dec->str_sbr_dec_info[ch_idx]) {
        p_state_enhaacplus_dec->str_sbr_dec_info[ch_idx]->xaac_jmp_buf =
            &(p_state_enhaacplus_dec->xaac_jmp_buf);
      }
    }

    {
      if (p_state_enhaacplus_dec->str_sbr_dec_info[ch_idx] &&
          p_state_enhaacplus_dec->pstr_stream_sbr[0][0].no_elements) {
        ia_sbr_scr_struct sbr_scratch_struct;
        ixheaacd_allocate_sbr_scr(&sbr_scratch_struct, p_state_enhaacplus_dec->aac_scratch_mem_v,
                                  time_data, total_channels,
                                  p_obj_exhaacplus_dec->p_state_aac->qshift_adj,
                                  p_state_enhaacplus_dec->slot_pos, channel);

        p_state_enhaacplus_dec->sbr_present = 1;
        p_state_enhaacplus_dec->peak_lim_init = 0;

        if (p_obj_exhaacplus_dec->aac_config.ui_enh_sbr)
        {
          WORD32 audio_object_type = p_state_enhaacplus_dec->audio_object_type;

          if (audio_object_type != AOT_ER_AAC_ELD && audio_object_type != AOT_ER_AAC_LD) {
            WORD32 i = 0;
            ia_dec_data_struct* pstr_dec_data =
                (ia_dec_data_struct*)p_state_enhaacplus_dec->pstr_dec_data;
            if (ch_fac == 1) {
              for (; i < 1024; i++) {
                pstr_dec_data->str_usac_data.time_sample_vector[0][i] =
                    (FLOAT32)time_data[i];
              }
              p_state_enhaacplus_dec->str_sbr_dec_info[ch_idx]->time_sample_buf[0] =
                  &pstr_dec_data->str_usac_data.time_sample_vector[0][0];

              p_state_enhaacplus_dec->str_sbr_dec_info[ch_idx]->time_sample_buf[1] =
                  &pstr_dec_data->str_usac_data.time_sample_vector[1][0];
            } else if (ch_fac == 2) {
              for (; i < 1024; i++) {
                pstr_dec_data->str_usac_data.time_sample_vector[0][i] =
                    (FLOAT32)time_data[2 * i + 0];
                pstr_dec_data->str_usac_data.time_sample_vector[1][i] =
                    (FLOAT32)time_data[2 * i + 1];
              }
              p_state_enhaacplus_dec->str_sbr_dec_info[ch_idx]->time_sample_buf[0] =
                  &pstr_dec_data->str_usac_data.time_sample_vector[0][0];

              p_state_enhaacplus_dec->str_sbr_dec_info[ch_idx]->time_sample_buf[1] =
                  &pstr_dec_data->str_usac_data.time_sample_vector[1][0];
            } else if (ch_fac > 2) {
              if (channel == 1) {
                for (; i < 1024; i++) {
                  pstr_dec_data->str_usac_data.time_sample_vector[0][i] =
                      (FLOAT32)(time_data + slot_ele)[i* ch_fac];
                }
                p_state_enhaacplus_dec->str_sbr_dec_info[ch_idx]->time_sample_buf[0] =
                    &pstr_dec_data->str_usac_data.time_sample_vector[0][0];
              } else if (channel == 2) {
                for (; i < 1024; i++) {
                  pstr_dec_data->str_usac_data.time_sample_vector[0][i] =
                      (FLOAT32)(time_data + slot_ele)[ch_fac * i + 0];
                  pstr_dec_data->str_usac_data.time_sample_vector[1][i] =
                      (FLOAT32)(time_data + slot_ele)[ch_fac * i + 1];
                }
                p_state_enhaacplus_dec->str_sbr_dec_info[ch_idx]->time_sample_buf[0] =
                    &pstr_dec_data->str_usac_data.time_sample_vector[0][0];

                p_state_enhaacplus_dec->str_sbr_dec_info[ch_idx]->time_sample_buf[1] =
                    &pstr_dec_data->str_usac_data.time_sample_vector[1][0];
              }
            }
          }
        }
        p_state_enhaacplus_dec->str_sbr_dec_info[ch_idx]->esbr_hq =
            p_obj_exhaacplus_dec->aac_config.ui_hq_esbr;
        p_state_enhaacplus_dec->str_sbr_dec_info[ch_idx]->enh_sbr =
            p_obj_exhaacplus_dec->aac_config.ui_enh_sbr;
        p_state_enhaacplus_dec->str_sbr_dec_info[ch_idx]->enh_sbr_ps =
            p_obj_exhaacplus_dec->aac_config.ui_enh_sbr_ps;

        if (p_state_enhaacplus_dec->str_sbr_dec_info[ch_idx]) {
          p_state_enhaacplus_dec->str_sbr_dec_info[ch_idx]->xaac_jmp_buf =
              &(p_state_enhaacplus_dec->xaac_jmp_buf);
        }

        if (ixheaacd_applysbr(p_state_enhaacplus_dec->str_sbr_dec_info[ch_idx],
                              &p_state_enhaacplus_dec->pstr_stream_sbr[ch_idx][0],
                              actual_out_buffer, &num_ch,
                              p_obj_exhaacplus_dec->aac_config.frame_status,
                              p_obj_exhaacplus_dec->aac_config.down_sample_flag,
                              esbr_mono_downmix, &sbr_scratch_struct, ps_enable, ch_fac, slot_ele,
                              NULL, &p_state_enhaacplus_dec->str_drc_dec_info,
                              p_state_enhaacplus_dec->eld_specific_config.ld_sbr_flag_present,
                              p_state_enhaacplus_dec->audio_object_type, 0,
                              p_state_enhaacplus_dec->ldmps_present, frame_size,
                              p_state_enhaacplus_dec->heaac_mps_handle.heaac_mps_present,
                              p_obj_exhaacplus_dec->aac_config.ui_err_conceal,
                              p_obj_exhaacplus_dec->aac_config.first_frame) != SBRDEC_OK) {
          p_state_enhaacplus_dec->str_sbr_dec_info[ch_idx] = 0;
          if (p_obj_exhaacplus_dec->aac_config.ui_err_conceal) {
            p_obj_exhaacplus_dec->aac_config.frame_status = 0;
          } else {
            return -1;
          }
        } else {
          if (!p_obj_exhaacplus_dec->aac_config.down_sample_flag) {
            frame_size = (WORD16)(frame_size * 2);
            sample_rate_dec *= 2;
          }
          if (p_state_enhaacplus_dec->mps_dec_handle.ldmps_config
                  .ldmps_present_flag == 1) {
            ixheaacd_mps_payload(
                p_state_enhaacplus_dec->str_sbr_dec_info[ch_idx],
                p_obj_exhaacplus_dec);
          }
        }

        if (p_obj_exhaacplus_dec->aac_config.ui_enh_sbr)
        {
          WORD32 audio_object_type = p_state_enhaacplus_dec->audio_object_type;

          if (audio_object_type != AOT_ER_AAC_ELD && audio_object_type != AOT_ER_AAC_LD) {
            WORD32 out_bytes = 0;
            ia_dec_data_struct *pstr_dec_data =
                (ia_dec_data_struct *)p_state_enhaacplus_dec->pstr_dec_data;
            if (ch_fac <= 2) {
              ixheaacd_samples_sat((WORD8*)time_data, 2048, 16,
                                   pstr_dec_data->str_usac_data.time_sample_vector,
                                   &out_bytes, ch_fac);
            } else {
              ixheaacd_samples_sat_mc((WORD8*)(time_data + slot_ele), 2048,
                                      pstr_dec_data->str_usac_data.time_sample_vector,
                                      &out_bytes, channel, ch_fac);
            }
          }
        }
        p_state_enhaacplus_dec->mps_dec_handle.ldmps_config.no_ldsbr_present =
            0;
        if (p_state_enhaacplus_dec->ui_mps_out_bytes > 0) {
          p_state_enhaacplus_dec->heaac_mps_handle.heaac_mps_present = 1;
        }
      } else {
        p_state_enhaacplus_dec->mps_dec_handle.ldmps_config.no_ldsbr_present = 1;
      }
    }

    if (p_state_enhaacplus_dec->mps_dec_handle.ldmps_config
                .ldmps_present_flag == 1 &&
         p_state_enhaacplus_dec->str_sbr_dec_info[ch_idx] &&
         p_state_enhaacplus_dec->mps_dec_handle.mps_init_done == 1) {
      if (p_state_enhaacplus_dec->ec_enable) {
        if (!p_obj_exhaacplus_dec->aac_config.first_frame) {
          error_code = ixheaacd_ld_mps_apply(p_obj_exhaacplus_dec, actual_out_buffer);
          if (error_code) p_obj_exhaacplus_dec->aac_config.frame_status = 0;
        }
      } else {
        error_code = ixheaacd_ld_mps_apply(p_obj_exhaacplus_dec, actual_out_buffer);

        if (error_code)
          return error_code;
      }
    }
    if (sample_rate < sample_rate_dec) {
      sample_rate = sample_rate_dec;
    }

    if (p_state_enhaacplus_dec->sbr_present ||
        p_obj_exhaacplus_dec->p_state_aac->qshift_adj[0] == LD_OBJ) {
      num_of_out_samples = frame_size;

    } else {
      num_of_out_samples =
          frame_size -
          MIN((WORD16)p_obj_exhaacplus_dec->p_state_aac->delay_in_samples, frame_size);
    }

    if (p_obj_exhaacplus_dec->aac_config.ui_err_conceal &&
        p_obj_exhaacplus_dec->aac_config.first_frame &&
        (p_obj_exhaacplus_dec->p_state_aac->audio_object_type == AOT_ER_AAC_ELD ||
         p_obj_exhaacplus_dec->p_state_aac->audio_object_type == AOT_ER_AAC_LD)) {
      num_of_out_samples = frame_size;
    }

    p_obj_exhaacplus_dec->aac_config.ui_samp_freq = sample_rate;

    p_state_enhaacplus_dec->num_channel_last = num_ch;
    p_state_enhaacplus_dec->num_of_out_samples = num_of_out_samples;

    if (p_state_enhaacplus_dec->mps_dec_handle.ldmps_config
            .ldmps_present_flag == 1 &&
        p_state_enhaacplus_dec->mps_dec_handle.mps_init_done == 1 &&
        p_state_enhaacplus_dec->str_sbr_dec_info[ch_idx]) {
      if (p_obj_exhaacplus_dec->p_state_aac->mps_dec_handle.output_buffer) {
        ixheaacd_samples_sat((WORD8 *)actual_out_buffer, num_of_out_samples,
                             p_obj_exhaacplus_dec->aac_config.ui_pcm_wdsz,
                             p_obj_exhaacplus_dec->p_state_aac->mps_dec_handle.output_buffer,
                             &mps_out_samples, 2);
        p_obj_exhaacplus_dec->p_state_aac->ui_out_bytes = mps_out_samples;
      }
      num_ch = p_obj_exhaacplus_dec->p_state_aac->mps_dec_handle.out_ch_count;
      if (p_state_enhaacplus_dec->ec_enable) {
        if (p_obj_exhaacplus_dec->aac_config.first_frame) {
          p_obj_exhaacplus_dec->p_state_aac->ui_out_bytes =
              p_state_enhaacplus_dec->num_of_out_samples * num_ch * sizeof(WORD16);
        }
      }
    } else {
      if (p_obj_exhaacplus_dec->aac_config.element_type[ch_idx] != 2) {
        if (p_obj_exhaacplus_dec->aac_config.flag_to_stereo == 1 &&
          channel == 1 && total_elements == 1 && num_ch == 1) {
        num_ch = 2;
        p_obj_exhaacplus_dec->aac_config.dup_stereo_flag = 1;

      } else {
        p_obj_exhaacplus_dec->aac_config.dup_stereo_flag = 0;
      }

      p_obj_exhaacplus_dec->aac_config.ui_n_channels = 2;

      p_obj_exhaacplus_dec->p_state_aac->ui_out_bytes +=
          p_state_enhaacplus_dec->num_of_out_samples * num_ch * sizeof(WORD16);

    } else {
      channel_coupling_flag = 1;
    }
  }

    if (p_state_enhaacplus_dec->sbr_present && total_channels > 2) {
      for (WORD32 j = 0; j < channel; j++) {
        for (WORD32 i = 0; i < frame_size; i++) {
          intermediate_scr[total_channels * i + j +
                           p_state_enhaacplus_dec->slot_pos] =
              actual_out_buffer[total_channels * i + j +
                                p_state_enhaacplus_dec->slot_pos];
        }
      }
    }
  }

  if (p_state_enhaacplus_dec->sbr_present && total_channels > 2) {
    memcpy(time_data, intermediate_scr,
           sizeof(WORD16) * frame_size * total_channels);
  }

  {
    ia_adts_crc_info_struct *ptr_adts_crc_info =
        p_state_enhaacplus_dec->ptr_bit_stream->pstr_adts_crc_info;
    if (ptr_adts_crc_info->crc_active == 1) {
      if ((error_code = ixheaacd_adts_crc_check_crc(ptr_adts_crc_info))) {
        if (error_code) {
          if (p_obj_exhaacplus_dec->aac_config.ui_err_conceal) {
            p_obj_exhaacplus_dec->aac_config.frame_status = 0;
          } else {
            return error_code;
          }
        }
      }
    }
  }

  p_obj_exhaacplus_dec->aac_config.ui_n_channels = total_channels;

  p_state_enhaacplus_dec->frame_counter++;

  WORD32 i, j;

  if (channel_coupling_flag) {
    error_code = ixheaacd_dec_ind_coupling(p_obj_exhaacplus_dec,
                                           p_state_enhaacplus_dec->coup_ch_output,
                                           num_of_out_samples, total_channels, time_data);
    if (error_code)
      return error_code;
  }

  for (i = 0; i < total_channels; i++) {
    if (p_obj_exhaacplus_dec->p_state_aac->qshift_adj[i + 1] == 0)
      p_obj_exhaacplus_dec->p_state_aac->qshift_adj[i + 1] =
          p_obj_exhaacplus_dec->p_state_aac->qshift_adj[0];
  }

  if (p_obj_exhaacplus_dec->aac_config.flag_to_stereo == 1 &&
      total_elements == 1 && num_ch == 2 &&
      p_obj_exhaacplus_dec->aac_config.dup_stereo_flag == 1) {
    WORD i;

    if (!p_state_enhaacplus_dec->sbr_present &&
        p_obj_exhaacplus_dec->p_state_aac->qshift_adj[0] != LD_OBJ) {
      for (i = 0; i < frame_size; i++) {
        *((WORD32 *)actual_out_buffer + 2 * i + 1) =
            *((WORD32 *)actual_out_buffer + 2 * i);
      }
    } else {
      for (i = 0; i < frame_size; i++) {
        *(actual_out_buffer + 2 * i + 1) = *(actual_out_buffer + 2 * i);
      }
    }
  }

  if (!p_state_enhaacplus_dec->sbr_present &&
      p_obj_exhaacplus_dec->p_state_aac->peak_lim_init == 1 &&
      p_obj_exhaacplus_dec->p_state_aac->qshift_adj[0] != LD_OBJ) {
    if (!p_obj_exhaacplus_dec->aac_config.peak_limiter_off) {
      ixheaacd_peak_limiter_process(
          &p_state_enhaacplus_dec->peak_limiter, time_data, frame_size,
          p_obj_exhaacplus_dec->p_state_aac->qshift_adj);
    } else {
      ixheaacd_scale_adjust(time_data, frame_size,
                            p_obj_exhaacplus_dec->p_state_aac->qshift_adj,
                            total_channels);
    }

    for (i = 0; i < frame_size * 2; i++) {
      for (j = 0; j < total_channels; j++) {
        *((WORD16 *)time_data + total_channels * i + j) =
            ixheaac_round16(*((WORD32 *)time_data + total_channels * i + j));
      }
    }

    memmove(
        time_data,
        (time_data +
         total_channels * p_obj_exhaacplus_dec->p_state_aac->delay_in_samples),
        sizeof(WORD16) * num_of_out_samples * total_channels);

    p_obj_exhaacplus_dec->p_state_aac->delay_in_samples =
        p_obj_exhaacplus_dec->p_state_aac->delay_in_samples -
        MIN(p_obj_exhaacplus_dec->p_state_aac->delay_in_samples, (UWORD16)frame_size);
  }
  if (p_state_enhaacplus_dec->heaac_mps_handle.heaac_mps_present == 1) {
    ia_heaac_mps_state_struct *pstr_mps_state =
        &p_state_enhaacplus_dec->heaac_mps_handle;
    if (p_state_enhaacplus_dec->sbr_present == 0) {
      p_state_enhaacplus_dec->heaac_mps_handle.mps_decode = 1;
    } else {
      p_state_enhaacplus_dec->heaac_mps_handle.mps_with_sbr = 1;
    }
    if (!p_obj_exhaacplus_dec->aac_config.ui_enh_sbr)
    {
      p_state_enhaacplus_dec->heaac_mps_handle.mps_decode = 1;
    }
    if (p_state_enhaacplus_dec->heaac_mps_handle.mps_init_done == 1) {
      p_obj_exhaacplus_dec->p_state_aac->heaac_mps_handle.frame_ok =
          p_obj_exhaacplus_dec->aac_config.frame_status;
      p_obj_exhaacplus_dec->p_state_aac->heaac_mps_handle.ec_flag =
          p_obj_exhaacplus_dec->aac_config.ui_err_conceal;

      error_code = ixheaacd_heaac_mps_apply(p_obj_exhaacplus_dec, actual_out_buffer, mps_buffer,
                                            p_state_enhaacplus_dec->ui_mps_out_bytes);

      if (error_code != IA_NO_ERROR) {
        return error_code;
      }

      p_state_enhaacplus_dec->heaac_mps_handle.mps_decode = 1;
      p_obj_exhaacplus_dec->p_state_aac->ui_out_bytes =
          (pstr_mps_state->num_output_channels_at *
           pstr_mps_state->frame_length *
           (p_obj_exhaacplus_dec->aac_config.ui_pcm_wdsz >> 3));
      p_obj_exhaacplus_dec->p_state_aac->heaac_mps_handle.first_frame = 0;
    }
  }
  if ((total_channels > 2) && (1 == p_obj_exhaacplus_dec->aac_config.downmix)) {
    ixheaacd_dec_downmix_to_stereo(p_obj_exhaacplus_dec, num_of_out_samples,
                                   total_elements, time_data, total_channels);

    total_channels = 2;
    p_obj_exhaacplus_dec->p_state_aac->ui_out_bytes =
        p_state_enhaacplus_dec->num_of_out_samples * 2 * sizeof(WORD16);
  }

  if (p_obj_exhaacplus_dec->aac_config.flag_downmix && total_channels == 2) {
    WORD32 out_ch = 1;
    WORD i;
    if (p_obj_exhaacplus_dec->aac_config.flag_to_stereo == 1) {
      out_ch = 2;
    }

    p_obj_exhaacplus_dec->aac_config.ui_n_channels = out_ch;
    p_obj_exhaacplus_dec->p_state_aac->ui_out_bytes =
        p_state_enhaacplus_dec->num_of_out_samples * out_ch * sizeof(WORD16);

    for (i = 0; i < num_of_out_samples; i++) {
      WORD16 temp;

      temp = (time_data[2 * i + 0] >> 1) + (time_data[2 * i + 1] >> 1);

      if (out_ch == 2) {
        time_data[2 * i + 0] = temp;
        time_data[2 * i + 1] = time_data[2 * i + 0];
      } else {
        time_data[i] = temp;
      }
    }
  }

  if (p_state_enhaacplus_dec->s_adts_hdr_present) {
    if (adts.no_raw_data_blocks != 0) {
      if (adts.protection_absent == 0 && it_bit_buff->cnt_bits >= 16) {
        adts.crc_check = ixheaacd_read_bits_buf(it_bit_buff, 16);
      }
    }
    p_state_enhaacplus_dec->b_n_raw_data_blk--;
  }

  ixheaacd_updatebytesconsumed(p_state_enhaacplus_dec, it_bit_buff);

  if (p_state_enhaacplus_dec->bs_format == LOAS_BSFORMAT)
    p_state_enhaacplus_dec->i_bytes_consumed =
        (WORD32)(audio_mux_length_bytes_last + bytes_for_sync);

  if (p_obj_exhaacplus_dec->aac_config.ui_err_conceal &&
      p_obj_exhaacplus_dec->aac_config.first_frame) {
    p_obj_exhaacplus_dec->aac_config.first_frame = 0;
  }

  if (p_obj_exhaacplus_dec->aac_config.ui_err_conceal) {
    if (p_obj_exhaacplus_dec->aac_config.frame_status != 1) {
      p_state_enhaacplus_dec->i_bytes_consumed = p_state_enhaacplus_dec->ui_in_bytes;
    }
    return IA_NO_ERROR;
  } else {
    return error_code;
  }
}
