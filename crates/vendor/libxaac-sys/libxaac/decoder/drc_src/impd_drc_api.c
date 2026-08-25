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
#include <string.h>
#include <stdlib.h>
#include "impd_type_def.h"
#include "impd_error_standards.h"
#include "impd_apicmd_standards.h"
#include "impd_memory_standards.h"

#include "impd_drc_bitbuffer.h"
#include "impd_drc_extr_delta_coded_info.h"
#include "ixheaac_constants.h"
#include "impd_drc_common.h"
#include "impd_drc_struct.h"
#include "impd_drc_interface.h"
#include "impd_parametric_drc_dec.h"
#include "impd_drc_gain_dec.h"
#include "impd_drc_filter_bank.h"
#include "impd_drc_multi_band.h"
#include "impd_drc_process_audio.h"
#include "impd_drc_eq.h"
#include "impd_drc_gain_decoder.h"
#include "impd_drc_config_params.h"
#include "impd_drc_api_defs.h"
#include "impd_drc_definitions.h"
#include "impd_drc_hashdefines.h"
#include "impd_drc_peak_limiter.h"

#include "impd_drc_selection_process.h"
#include "impd_drc_api_struct_def.h"
#include "impd_drc_error_codes.h"

WORD32 impd_init_process_audio_main_qmf(ia_drc_api_struct *p_obj_drc);
WORD32 impd_init_process_audio_main_stft(ia_drc_api_struct *p_obj_drc);
WORD32 impd_init_process_audio_main_td_qmf(ia_drc_api_struct *p_obj_drc);

IA_ERRORCODE impd_drc_mem_api(ia_drc_api_struct *p_obj_drc, WORD32 i_cmd,
                              WORD32 i_idx, pVOID pv_value);

IA_ERRORCODE impd_drc_fill_mem_tables(ia_drc_api_struct *p_obj_drc);

IA_ERRORCODE impd_drc_process_frame(ia_drc_api_struct *p_obj_drc);
IA_ERRORCODE impd_drc_init(ia_drc_api_struct *p_obj_drc);
IA_ERRORCODE impd_drc_set_default_config(ia_drc_api_struct *p_obj_drc);
IA_ERRORCODE impd_drc_set_struct_pointer(ia_drc_api_struct *p_obj_drc);
IA_ERRORCODE impd_process_time_domain(ia_drc_api_struct *p_obj_drc);

#define NUM_DRC_TABLES 4

IA_ERRORCODE ia_drc_dec_api(pVOID p_ia_drc_dec_obj, WORD32 i_cmd, WORD32 i_idx,
                            pVOID pv_value) {
  ia_drc_api_struct *p_obj_drc = p_ia_drc_dec_obj;
  IA_ERRORCODE error_code = IA_NO_ERROR;
  LOOPIDX i;

  pUWORD32 pui_value = pv_value;
  pUWORD32 pus_value = pv_value;
  pWORD8 pb_value = pv_value;
  SIZE_T *ps_value = pv_value;
  pWORD32 pi_value = pv_value;
  FLOAT32 *pf_value = pv_value;

  switch (i_cmd) {
    case IA_API_CMD_GET_MEM_INFO_SIZE:
    case IA_API_CMD_GET_MEM_INFO_ALIGNMENT:
    case IA_API_CMD_GET_MEM_INFO_TYPE:
    case IA_API_CMD_GET_MEM_INFO_PLACEMENT:
    case IA_API_CMD_GET_MEM_INFO_PRIORITY:
    case IA_API_CMD_SET_MEM_PTR:
    case IA_API_CMD_SET_MEM_PLACEMENT: {
      return impd_drc_mem_api(p_ia_drc_dec_obj, i_cmd, i_idx, pv_value);
    }
  };

  switch (i_cmd) {
    case IA_API_CMD_GET_LIB_ID_STRINGS: {
      switch (i_idx) {
        case IA_CMD_TYPE_LIB_NAME: {
          WORD8 lib_name[] = LIBNAME;
          for (i = 0; i < IA_API_STR_LEN && lib_name[i - 1] != 0; i++) {
            pb_value[i] = lib_name[i];
          }
          break;
        }
        case IA_CMD_TYPE_LIB_VERSION: {
          break;
        }

        case IA_CMD_TYPE_API_VERSION: {
        }
        default: { return -1; }
      };
      break;
    }
    case IA_API_CMD_GET_API_SIZE: {
      *pui_value = sizeof(ia_drc_api_struct);

      break;
    }
    case IA_API_CMD_INIT: {
      switch (i_idx) {
        case IA_CMD_TYPE_INIT_SET_BUFF_PTR: {
          p_obj_drc->p_state->persistent_ptr =
              (UWORD8 *)p_obj_drc->pp_mem[IA_DRC_PERSIST_IDX] +
              IXHEAAC_GET_SIZE_ALIGNED(sizeof(ia_drc_state_struct), BYTE_ALIGN_8);
          error_code = impd_drc_set_struct_pointer(p_obj_drc);
          if (error_code) return error_code;

          break;
        }
        case IA_CMD_TYPE_INIT_API_PRE_CONFIG_PARAMS: {
          error_code = impd_drc_set_default_config(p_obj_drc);
          if (error_code) return error_code;
          break;
        }
        case IA_CMD_TYPE_INIT_API_POST_CONFIG_PARAMS: {
          error_code = impd_drc_fill_mem_tables(p_obj_drc);
          if (error_code) return error_code;
          break;
        }
        case IA_CMD_TYPE_INIT_PROCESS: {
          IA_ERRORCODE Error = 0;

          if (p_obj_drc->pp_mem[IA_DRC_PERSIST_IDX] == 0) {
            return (-1);
          }

          Error = impd_drc_init(p_obj_drc);
          if (Error) return Error;
          p_obj_drc->p_state->ui_init_done = 1;
          return Error;
          break;
        }
        case IA_CMD_TYPE_INIT_DONE_QUERY: {
          if (p_obj_drc->p_state->ui_init_done == 1) {
            *pui_value = 1;
          } else {
            *pui_value = 0;
          }
          break;
        }

        case IA_CMD_TYPE_INIT_CPY_BSF_BUFF_OVER_QUERY: {
          *pui_value = p_obj_drc->str_bit_handler.cpy_over;
          break;
        }
        case IA_CMD_TYPE_INIT_CPY_IC_BSF_BUFF_OVER_QUERY: {
          *pui_value = p_obj_drc->str_bit_handler.cpy_over_ic;
          break;
        }

        case IA_CMD_TYPE_INIT_CPY_IL_BSF_BUFF_OVER_QUERY: {
          *pui_value = p_obj_drc->str_bit_handler.cpy_over_il;
          break;
        }
        case IA_CMD_TYPE_INIT_CPY_IN_BSF_BUFF_OVER_QUERY: {
          *pui_value = p_obj_drc->str_bit_handler.cpy_over_in;
          break;
        }
        case IA_CMD_TYPE_INIT_CPY_BSF_BUFF: {
          memcpy(p_obj_drc->str_bit_handler.it_bit_buf +
                     p_obj_drc->str_bit_handler.num_bytes_offset_bs,
                 p_obj_drc->pp_mem[2], p_obj_drc->str_bit_handler.num_byts_cur);
          p_obj_drc->str_bit_handler.num_bytes_bs =
              p_obj_drc->str_bit_handler.num_bytes_bs +
              p_obj_drc->str_bit_handler.num_byts_cur;
          p_obj_drc->str_bit_handler.num_bytes_offset_bs =
              p_obj_drc->str_bit_handler.num_bytes_bs;
          p_obj_drc->str_bit_handler.num_total_bytes =
              p_obj_drc->str_bit_handler.num_bytes_bs;

          break;
        }
        case IA_CMD_TYPE_INIT_CPY_IC_BSF_BUFF: {
          memcpy(p_obj_drc->str_bit_handler.bitstream_drc_config,
                 p_obj_drc->pp_mem[2],
                 p_obj_drc->str_bit_handler.num_byts_cur_ic);
          p_obj_drc->str_bit_handler.num_bytes_bs_drc_config =
              p_obj_drc->str_bit_handler.num_byts_cur_ic;
          break;
        }
        case IA_CMD_TYPE_INIT_CPY_IL_BSF_BUFF: {
          memcpy(p_obj_drc->str_bit_handler.bitstream_loudness_info,
                 p_obj_drc->pp_mem[2],
                 p_obj_drc->str_bit_handler.num_byts_cur_il);
          p_obj_drc->str_bit_handler.num_bytes_bs_loudness_info =
              p_obj_drc->str_bit_handler.num_byts_cur_il;
          break;
        }
        case IA_CMD_TYPE_INIT_CPY_IN_BSF_BUFF: {
          memcpy(p_obj_drc->str_bit_handler.bitstream_unidrc_interface,
                 p_obj_drc->pp_mem[2],
                 p_obj_drc->str_bit_handler.num_byts_cur_in);
          p_obj_drc->str_bit_handler.num_bytes_bs_unidrc_interface =
              p_obj_drc->str_bit_handler.num_byts_cur_in;
          break;
        }
        default: { return -1; }
      };
      break;
    }
    case IA_API_CMD_GET_CONFIG_PARAM: {
      switch (i_idx) {
        case IA_DRC_DEC_CONFIG_PARAM_SAMP_FREQ: {
          *pus_value = p_obj_drc->str_config.sampling_rate;
          break;
        }

        case IA_DRC_DEC_CONFIG_PARAM_NUM_CHANNELS: {
          *pus_value = p_obj_drc->str_config.num_ch_out;
          break;
        }
        case IA_DRC_DEC_CONFIG_PROC_OUT_PTR: {
          *ps_value = (SIZE_T)p_obj_drc->str_payload.pstr_drc_sel_proc_output;
          break;
        }
        case IA_DRC_DEC_CONFIG_DRC_TARGET_LOUDNESS: {
          *pi_value = (WORD32)p_obj_drc->str_payload.pstr_loudness_info->loudness_info
                          ->loudness_measure->method_val;
          if (*pi_value < -1)
            *pi_value = *pi_value * -4;
          else
            *pi_value = -1;
          break;
        }
      }
      break;
    }
    case IA_API_CMD_SET_CONFIG_PARAM: {
      switch (i_idx) {
        case IA_DRC_DEC_CONFIG_PARAM_SAMP_FREQ: {
          if (*pus_value == 0 || *pus_value > 96000) {
            return IA_DRC_DEC_CONFIG_NON_FATAL_INVALID_SAMP_FREQ;
          }
          p_obj_drc->str_config.sampling_rate = *pus_value;
          break;
        }
        case IA_DRC_DEC_CONFIG_PARAM_APPLY_CROSSFADE: {
          p_obj_drc->str_config.apply_crossfade = *pus_value;
          break;
        }
        case IA_DRC_DEC_CONFIG_PARAM_CONFIG_CHANGED: {
          p_obj_drc->str_config.is_config_changed = *pus_value;
          break;
        }
        case IA_DRC_DEC_CONFIG_PARAM_NUM_CHANNELS: {
          p_obj_drc->str_config.num_ch_in = *pus_value;
          if (*pus_value < 1 || *pus_value > MAX_CHANNEL_COUNT) {
            return IA_DRC_DEC_CONFIG_NON_FATAL_INVALID_NUM_OF_CHANNELS;
          }
          break;
        }

        case IA_DRC_DEC_CONFIG_PARAM_PCM_WDSZ: {
          switch (*pus_value) {
            case 16:
            case 24:
            case 32:
              break;
            default:
              return IA_DRC_DEC_CONFIG_NON_FATAL_INVALID_PCM_SIZE;
          }

          p_obj_drc->str_config.pcm_size = *pus_value;

          break;
        }

        case IA_DRC_DEC_CONFIG_PARAM_BITS_FORMAT: {
          break;
        }
        case IA_DRC_DEC_CONFIG_PARAM_INT_PRESENT: {
          break;
        }
        case IA_DRC_DEC_CONFIG_PARAM_FRAME_SIZE: {
          if (*pus_value < 1 || *pus_value > 4096) {
            return IA_DRC_DEC_CONFIG_NON_FATAL_INVALID_FRAME_SIZE;
          }

          p_obj_drc->str_config.frame_size = *pus_value;

          break;
        }
        case IA_DRC_DEC_CONFIG_GAIN_STREAM_FLAG: {
          break;
        }

        case IA_DRC_DEC_CONFIG_DRC_EFFECT_TYPE: {
          p_obj_drc->str_config.effect_type = *pus_value;
          break;
        }
        case IA_DRC_DEC_CONFIG_DRC_TARGET_LOUDNESS: {
          p_obj_drc->str_config.target_loudness = *pus_value;
          break;
        }
        case IA_DRC_DEC_CONFIG_DRC_LOUD_NORM: {
          p_obj_drc->str_config.loud_norm_flag = *pus_value;
          break;
        }
        case IA_DRC_DEC_CONFIG_DRC_ALBUM_MODE: {
          p_obj_drc->str_config.album_mode = *pus_value;
          break;
        }
        case IA_DRC_DEC_CONFIG_DRC_BOOST: {
          p_obj_drc->str_config.boost = *pf_value;
          p_obj_drc->str_config.boost_set = 1;
          break;
        }
        case IA_DRC_DEC_CONFIG_DRC_COMPRESS: {
          p_obj_drc->str_config.compress = *pf_value;
          p_obj_drc->str_config.compress_set = 1;
          break;
        }
#ifdef LOUDNESS_LEVELING_SUPPORT
        case IA_DRC_DEC_CONFIG_DRC_LOUDNESS_LEVELING: {
          p_obj_drc->str_config.loudness_leveling_flag = *pus_value;
          break;
        }
#endif
        default: { return -1; }
      }
      break;
    }
    case IA_API_CMD_GET_MEMTABS_SIZE: {
      *pui_value =
          (sizeof(ia_mem_info_struct) + sizeof(pVOID *)) * (NUM_DRC_TABLES);
      break;
    }
    case IA_API_CMD_SET_MEMTABS_PTR: {
      if (ps_value == NULL) return IA_DRC_DEC_API_FATAL_MEM_ALLOC;
      memset(ps_value, 0,
             (sizeof(ia_mem_info_struct) + sizeof(pVOID *)) * (NUM_DRC_TABLES));
      p_obj_drc->p_mem_info = (ia_mem_info_struct *)(ps_value);
      p_obj_drc->pp_mem =
          (pVOID)((SIZE_T)p_obj_drc->p_mem_info +
                  (NUM_DRC_TABLES * sizeof(*(p_obj_drc->p_mem_info))));
      break;
    }
    case IA_API_CMD_GET_N_MEMTABS: {
      *pui_value = NUM_DRC_TABLES;
      break;
    }
    case IA_API_CMD_GET_N_TABLES: {
      break;
    }

    case IA_API_CMD_EXECUTE: {
      switch (i_idx) {
        case IA_CMD_TYPE_DO_EXECUTE: {
          if (!p_obj_drc->p_state->ui_init_done) {
            error_code = IA_FATAL_ERROR;
          } else if (p_obj_drc->str_config.dec_type == DEC_TYPE_TD) {
            error_code = impd_process_time_domain(p_obj_drc);
          } else if (p_obj_drc->str_config.dec_type == DEC_TYPE_QMF64) {
            error_code = IA_FATAL_ERROR;
          } else if (p_obj_drc->str_config.dec_type == DEC_TYPE_STFT256) {
            error_code = IA_FATAL_ERROR;
          } else if (p_obj_drc->str_config.dec_type == DEC_TYPE_TD_QMF64) {
            error_code = IA_FATAL_ERROR;
          }
          p_obj_drc->str_bit_handler.byte_index_bs =
              p_obj_drc->str_bit_handler.num_total_bytes -
              p_obj_drc->str_bit_handler.num_bytes_bs;
          p_obj_drc->str_bit_handler.num_bytes_offset_bs = 0;

          break;
        }
        case IA_CMD_TYPE_DONE_QUERY: {
          *pui_value = p_obj_drc->p_state->ui_exe_done;
          break;
        }
        default: { return -1; }
      };
      break;
    }
    case IA_API_CMD_PUT_INPUT_QUERY: {
      *pui_value = 1;
      break;
    }
    case IA_API_CMD_GET_CURIDX_INPUT_BUF: {
      UWORD32 ui_in_buf_size = p_obj_drc->p_mem_info[IA_DRC_INPUT_IDX].ui_size;
      UWORD32 ui_in_bytes = p_obj_drc->p_state->ui_in_bytes;
      *pui_value = ui_in_buf_size > ui_in_bytes ? ui_in_bytes : ui_in_buf_size;
      break;
    }
    case IA_API_CMD_SET_INPUT_BYTES: {
      p_obj_drc->p_state->ui_in_bytes = *pui_value;
      break;
    }

    case IA_API_CMD_GET_OUTPUT_BYTES: {
      *pui_value = p_obj_drc->p_state->ui_out_bytes;
      break;
    }
    case IA_API_CMD_INPUT_OVER: {
      p_obj_drc->p_state->ui_exe_done = 1;
      break;
    }
    case IA_API_CMD_SET_INPUT_BYTES_BS: {
      p_obj_drc->str_bit_handler.num_byts_cur = *pus_value;
      break;
    }
    case IA_API_CMD_SET_INPUT_BYTES_IC_BS: {
      p_obj_drc->str_bit_handler.num_byts_cur_ic = *pus_value;
      break;
    }
    case IA_API_CMD_SET_INPUT_BYTES_IL_BS: {
      p_obj_drc->str_bit_handler.num_byts_cur_il = *pus_value;
      break;
    }
    case IA_API_CMD_SET_INPUT_BYTES_IN_BS: {
      p_obj_drc->str_bit_handler.num_byts_cur_in = *pus_value;
      break;
    }
    default: { return -1; }
  };
  return error_code;
}

IA_ERRORCODE impd_drc_mem_api(ia_drc_api_struct *p_obj_drc, WORD32 i_cmd,
                              WORD32 i_idx, pVOID pv_value) {
  pUWORD32 pui_value = pv_value;

  switch (i_cmd) {
    case IA_API_CMD_GET_MEM_INFO_SIZE: {
      *pui_value = p_obj_drc->p_mem_info[i_idx].ui_size;
      break;
    }
    case IA_API_CMD_GET_MEM_INFO_ALIGNMENT: {
      *pui_value = p_obj_drc->p_mem_info[i_idx].ui_alignment;
      break;
    }
    case IA_API_CMD_GET_MEM_INFO_TYPE: {
      *pui_value = p_obj_drc->p_mem_info[i_idx].ui_type;
      break;
    }
    case IA_API_CMD_GET_MEM_INFO_PLACEMENT: {
      *pui_value = p_obj_drc->p_mem_info[i_idx].ui_placement[0];
      *(pui_value + 1) = p_obj_drc->p_mem_info[i_idx].ui_placement[1];
      break;
    }
    case IA_API_CMD_GET_MEM_INFO_PRIORITY: {
      *pui_value = p_obj_drc->p_mem_info[i_idx].ui_priority;
      break;
    }
    case IA_API_CMD_SET_MEM_PTR: {
      if (pv_value == 0) {
        return (-1);
      }
      if (((SIZE_T)pv_value % p_obj_drc->p_mem_info[i_idx].ui_alignment) != 0) {
        return (-1);
      }
      p_obj_drc->pp_mem[i_idx] = pv_value;
      memset(p_obj_drc->pp_mem[i_idx], 0, p_obj_drc->p_mem_info[i_idx].ui_size);
      if (IA_MEMTYPE_PERSIST == i_idx) {
        p_obj_drc->p_state = pv_value;
      }
      break;
    }
    case IA_API_CMD_SET_MEM_PLACEMENT: {
    }
  };
  return IA_NO_ERROR;
}

static WORD32 impd_calc_scratch_size() {
  return IXHEAAC_GET_SIZE_ALIGNED(
      AUDIO_CODEC_FRAME_SIZE_MAX * MAX_CHANNEL_COUNT * NUM_ELE_IN_CPLX_NUM * sizeof(FLOAT32),
      BYTE_ALIGN_8);
}

static WORD32 impd_calc_pers_size() {
  WORD32 size = 0;

  WORD32 analysis_buf_size = IXHEAAC_GET_SIZE_ALIGNED(
      AUDIO_CODEC_FRAME_SIZE_MAX * MAX_CHANNEL_COUNT * sizeof(FLOAT32) * NUM_ELE_IN_CPLX_NUM,
      BYTE_ALIGN_8);
  WORD32 synth_buf_size = IXHEAAC_GET_SIZE_ALIGNED(
      AUDIO_CODEC_FRAME_SIZE_MAX * MAX_CHANNEL_COUNT * sizeof(FLOAT32) * NUM_ELE_IN_CPLX_NUM,
      BYTE_ALIGN_8);
  WORD32 peak_lim_buf_size = IXHEAAC_GET_SIZE_ALIGNED(
      AUDIO_CODEC_FRAME_SIZE_MAX * MAX_CHANNEL_COUNT * sizeof(FLOAT32) * NUM_ELE_IN_CPLX_NUM,
      BYTE_ALIGN_8);
  WORD32 subband_buf_size =
      NUM_ELE_IN_CPLX_NUM *
          IXHEAAC_GET_SIZE_ALIGNED(MAX_CHANNEL_COUNT * sizeof(FLOAT32 *), BYTE_ALIGN_8) +
      NUM_ELE_IN_CPLX_NUM * MAX_CHANNEL_COUNT *
          IXHEAAC_GET_SIZE_ALIGNED((MAX_SUBBAND_DELAY + MAX_DRC_FRAME_SIZE) * sizeof(FLOAT32),
                                   BYTE_ALIGN_8);

  size += IXHEAAC_GET_SIZE_ALIGNED(sizeof(ia_drc_state_struct), BYTE_ALIGN_8);
  size += IXHEAAC_GET_SIZE_ALIGNED(sizeof(ia_drc_bits_dec_struct), BYTE_ALIGN_8);
  size += 2 * IXHEAAC_GET_SIZE_ALIGNED(sizeof(ia_drc_gain_dec_struct), BYTE_ALIGN_8);
  size += IXHEAAC_GET_SIZE_ALIGNED(sizeof(ia_drc_loudness_info_set_struct), BYTE_ALIGN_8);
  size += IXHEAAC_GET_SIZE_ALIGNED(sizeof(ia_drc_gain_struct), BYTE_ALIGN_8);
  size += IXHEAAC_GET_SIZE_ALIGNED(sizeof(ia_drc_interface_struct), BYTE_ALIGN_8);
  size += IXHEAAC_GET_SIZE_ALIGNED(sizeof(ia_drc_config), BYTE_ALIGN_8);
  size += IXHEAAC_GET_SIZE_ALIGNED(sizeof(ia_drc_sel_pro_struct), BYTE_ALIGN_8);
  size += IXHEAAC_GET_SIZE_ALIGNED(sizeof(ia_drc_sel_proc_params_struct), BYTE_ALIGN_8);
  size += IXHEAAC_GET_SIZE_ALIGNED(sizeof(ia_drc_sel_proc_output_struct), BYTE_ALIGN_8);
  size += IXHEAAC_GET_SIZE_ALIGNED(sizeof(ia_drc_peak_limiter_struct), BYTE_ALIGN_8);
  size += IXHEAAC_GET_SIZE_ALIGNED(sizeof(ia_drc_qmf_filt_struct), BYTE_ALIGN_8);
  size += analysis_buf_size;
  size += synth_buf_size;
  size += peak_lim_buf_size;

  size += IXHEAAC_GET_SIZE_ALIGNED(MAX_DRC_BS_BUF_SIZE, BYTE_ALIGN_8);
  size += IXHEAAC_GET_SIZE_ALIGNED(MAX_DRC_CONFG_BUF_SIZE, BYTE_ALIGN_8); /*DRC Config Bitstream*/
  size += IXHEAAC_GET_SIZE_ALIGNED(MAX_LOUD_INFO_BUF_SIZE,
                                   BYTE_ALIGN_8); /*DRC loudness info Bitstream*/
  size +=
      IXHEAAC_GET_SIZE_ALIGNED(MAX_INTERFACE_BUF_SIZE, BYTE_ALIGN_8); /*DRC interface Bitstream*/

  size += NUM_GAIN_DEC_INSTANCES * SEL_DRC_COUNT *
          IXHEAAC_GET_SIZE_ALIGNED((sizeof(ia_interp_buf_struct) * MAX_GAIN_ELE_COUNT),
                                   BYTE_ALIGN_8); /*ia_interp_buf_struct*/
  size += NUM_GAIN_DEC_INSTANCES *
          IXHEAAC_GET_SIZE_ALIGNED((sizeof(ia_eq_set_struct)), BYTE_ALIGN_8); /*ia_eq_set_struct*/

  /*non_interleaved_audio*/
  size += NUM_GAIN_DEC_INSTANCES *
          IXHEAAC_GET_SIZE_ALIGNED((MAX_CHANNEL_COUNT * sizeof(FLOAT32 *)), BYTE_ALIGN_8);
  size += NUM_GAIN_DEC_INSTANCES * MAX_CHANNEL_COUNT *
          IXHEAAC_GET_SIZE_ALIGNED((MAX_DRC_FRAME_SIZE * sizeof(FLOAT32)), BYTE_ALIGN_8);

  /*audio_in_out_buf ptr*/
  size += NUM_GAIN_DEC_INSTANCES *
          IXHEAAC_GET_SIZE_ALIGNED((MAX_CHANNEL_COUNT * sizeof(FLOAT32 *)), BYTE_ALIGN_8);
  /*audio_io_buffer_delayed*/
  size += NUM_GAIN_DEC_INSTANCES *
          IXHEAAC_GET_SIZE_ALIGNED((MAX_CHANNEL_COUNT * sizeof(FLOAT32 *)), BYTE_ALIGN_8);
  size += NUM_GAIN_DEC_INSTANCES * MAX_CHANNEL_COUNT *
          IXHEAAC_GET_SIZE_ALIGNED((MAX_DRC_FRAME_SIZE * sizeof(FLOAT32)), BYTE_ALIGN_8);

  /*subband band buffer ptr*/
  size += NUM_GAIN_DEC_INSTANCES * NUM_ELE_IN_CPLX_NUM *
          IXHEAAC_GET_SIZE_ALIGNED(MAX_CHANNEL_COUNT * sizeof(FLOAT32 *), BYTE_ALIGN_8);
  size += NUM_GAIN_DEC_INSTANCES * subband_buf_size;

  size += NUM_GAIN_DEC_INSTANCES * MAX_CHANNEL_COUNT *
          IXHEAAC_GET_SIZE_ALIGNED((PARAM_DRC_MAX_BUF_SZ * sizeof(FLOAT32)), BYTE_ALIGN_8);

  return size;
}

IA_ERRORCODE impd_drc_fill_mem_tables(ia_drc_api_struct *p_obj_drc) {
  ia_mem_info_struct *p_mem_info;
  {
    p_mem_info = &p_obj_drc->p_mem_info[IA_DRC_PERSIST_IDX];
    memset(p_mem_info, 0, sizeof(*p_mem_info));
    p_mem_info->ui_size = impd_calc_pers_size();
    p_mem_info->ui_alignment = 8;
    p_mem_info->ui_type = IA_MEMTYPE_PERSIST;
    p_mem_info->ui_placement[0] = 0;
    p_mem_info->ui_placement[1] = 0;
    p_mem_info->ui_priority = IA_MEMPRIORITY_ANYWHERE;
    p_mem_info->ui_placed[0] = 0;
    p_mem_info->ui_placed[1] = 0;
  }
  {
    p_mem_info = &p_obj_drc->p_mem_info[IA_DRC_INPUT_IDX];
    memset(p_mem_info, 0, sizeof(*p_mem_info));
    p_mem_info->ui_size = p_obj_drc->str_config.frame_size *
                          (p_obj_drc->str_config.pcm_size >> 3) *
                          p_obj_drc->str_config.num_ch_in;
    p_mem_info->ui_alignment = 4;
    p_mem_info->ui_type = IA_MEMTYPE_INPUT;
    p_mem_info->ui_placement[0] = 0;
    p_mem_info->ui_placement[1] = 0;
    p_mem_info->ui_priority = IA_MEMPRIORITY_ANYWHERE;
    p_mem_info->ui_placed[0] = 0;
    p_mem_info->ui_placed[1] = 0;
  }
  {
    p_mem_info = &p_obj_drc->p_mem_info[IA_DRC_OUTPUT_IDX];
    memset(p_mem_info, 0, sizeof(*p_mem_info));
    p_mem_info->ui_size = p_obj_drc->str_config.frame_size *
                          (p_obj_drc->str_config.pcm_size >> 3) *
                          p_obj_drc->str_config.num_ch_in;
    p_mem_info->ui_alignment = 4;
    p_mem_info->ui_type = IA_MEMTYPE_OUTPUT;
    p_mem_info->ui_placement[0] = 0;
    p_mem_info->ui_placement[1] = 0;
    p_mem_info->ui_priority = IA_MEMPRIORITY_ANYWHERE;
    p_mem_info->ui_placed[0] = 0;
    p_mem_info->ui_placed[1] = 0;
  }
  {
    p_mem_info = &p_obj_drc->p_mem_info[IA_DRC_SCRATCH_IDX];
    memset(p_mem_info, 0, sizeof(*p_mem_info));
    p_mem_info->ui_size = impd_calc_scratch_size();
    p_mem_info->ui_alignment = 8;
    p_mem_info->ui_type = IA_MEMTYPE_SCRATCH;
    p_mem_info->ui_placement[0] = 0;
    p_mem_info->ui_placement[1] = 0;
    p_mem_info->ui_priority = IA_MEMPRIORITY_ANYWHERE;
    p_mem_info->ui_placed[0] = 0;
    p_mem_info->ui_placed[1] = 0;
  }
  return IA_NO_ERROR;
}
