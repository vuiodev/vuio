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
#include <stdlib.h>
#include <string.h>
#include "ixheaac_type_def.h"
#include "ixheaac_error_standards.h"
#include "ixheaacd_memory_standards.h"
#include "ixheaacd_sbrdecsettings.h"
#include "ixheaacd_sbr_scale.h"
#include "ixheaacd_env_extr_part.h"
#include "ixheaacd_defines.h"
#include "ixheaacd_aac_rom.h"
#include "ixheaacd_common_rom.h"
#include "ixheaacd_sbr_rom.h"
#include "ixheaacd_bitbuffer.h"
#include "ixheaacd_pulsedata.h"
#include "ixheaacd_pns.h"
#include "ixheaacd_interface.h"
#include "ixheaacd_info.h"
#include "ixheaacd_lt_predict.h"
#include "ixheaacd_cnst.h"
#include "ixheaacd_ec_defines.h"
#include "ixheaacd_ec_struct_def.h"
#include "ixheaacd_channelinfo.h"
#include "ixheaacd_sbr_common.h"
#include "ixheaacd_drc_data_struct.h"
#include "ixheaacd_drc_dec.h"
#include "ixheaacd_channel.h"
#include "ixheaacd_sbrdecoder.h"
#include "ixheaacd_audioobjtypes.h"
#include "ixheaacd_latmdemux.h"
#include "ixheaacd_aacdec.h"
#include "ixheaacd_sbr_common.h"
#include "ixheaacd_hybrid.h"
#include "ixheaacd_ps_dec.h"
#include "ixheaacd_mps_polyphase.h"
#include "ixheaacd_config.h"
#include "ixheaacd_qmf_dec.h"
#include "ixheaacd_mps_macro_def.h"
#include "ixheaacd_mps_struct_def.h"
#include "ixheaacd_mps_res_rom.h"
#include "ixheaacd_mps_aac_struct.h"
#include "ixheaac_constants.h"
#include "ixheaacd_mps_dec.h"
#include "ixheaacd_struct_def.h"
#include "ixheaacd_bitbuffer.h"
#include "ixheaacd_interface.h"
#include "ixheaacd_tns_usac.h"

#include "ixheaacd_acelp_info.h"
#include "ixheaacd_sbrdecsettings.h"
#include "ixheaacd_info.h"
#include "ixheaacd_sbrdecoder.h"
#include "ixheaacd_mps_polyphase.h"
#include "ixheaac_sbr_const.h"
#include "ixheaacd_main.h"
#include "ixheaacd_arith_dec.h"
#include "ixheaacd_config.h"
#include "ixheaacd_struct.h"
#include "ixheaacd_create.h"
#include "ixheaacd_dec_main.h"
#include "ixheaac_error_standards.h"
#include "ixheaacd_headerdecode.h"
#include "ixheaacd_error_codes.h"
VOID ixheaacd_samples_sat(WORD8 *outbuffer, WORD32 num_samples_out,
                          WORD32 pcmsize, FLOAT32 (*out_samples)[4096],
                          WORD32 *out_bytes, WORD32 num_channel_out) {
  WORD32 num;
  WORD32 i;
  WORD32 write_local;
  FLOAT32 write_local_float;

  WORD16 *out_buf = (WORD16 *)outbuffer;

  num = num_channel_out * num_samples_out;

  if (pcmsize == 16) {
    for (i = 0; i < num; i++) {
      write_local_float =
          (out_samples[i % num_channel_out][i / num_channel_out]);

      if (write_local_float > 32767.0f) {
        write_local_float = 32767.0f;
      } else if (write_local_float < -32768.0f) {
        write_local_float = -32768.0f;
      }
      out_buf[i] = (WORD16)write_local_float;
    }

    *out_bytes = num * sizeof(WORD16);
  } else {
    WORD8 *out_24bit = (WORD8 *)out_buf;
    for (i = 0; i < num; i++) {
      write_local_float =
          (out_samples[i % num_channel_out][i / num_channel_out] * 256);

      if (write_local_float > 8388607.0f) {
        write_local_float = 8388607.0f;
      } else if (write_local_float < -8388608.0f) {
        write_local_float = -8388608.0f;
      }
      write_local = (WORD32)write_local_float;

      *out_24bit++ = (WORD32)write_local & 0xff;
      *out_24bit++ = ((WORD32)write_local >> 8) & 0xff;
      *out_24bit++ = ((WORD32)write_local >> 16) & 0xff;
    }

    *out_bytes = num * 3 * sizeof(WORD8);
  }
}

VOID ixheaacd_samples_sat_mc(WORD8* outbuffer, WORD32 num_samples_out,
    FLOAT32(*out_samples)[4096], WORD32* out_bytes,
    WORD32 num_channel_out, WORD32 ch_fac) {
  WORD32 num;
  WORD32 i;
  FLOAT32 write_local_float;

  WORD16* out_buf = (WORD16*)outbuffer;

  num = num_channel_out * num_samples_out;
  if (num_channel_out == 1) {
    for (i = 0; i < num; i++) {
      write_local_float =
          (out_samples[i % num_channel_out][i / num_channel_out]);

      if (write_local_float > 32767.0f) {
        write_local_float = 32767.0f;
      } else if (write_local_float < -32768.0f) {
        write_local_float = -32768.0f;
      }
      out_buf[i * ch_fac] = (WORD16)write_local_float;
    }
  } else if (num_channel_out == 2) {
    for (i = 0; i < num_samples_out; i++) {
      write_local_float =
          (out_samples[(2*i) % num_channel_out][(2 * i) / num_channel_out]);

      if (write_local_float > 32767.0f) {
          write_local_float = 32767.0f;
      } else if (write_local_float < -32768.0f) {
          write_local_float = -32768.0f;
      }
      out_buf[i * ch_fac] = (WORD16)write_local_float;

      write_local_float =
          (out_samples[((2 * i) + 1) % num_channel_out][((2 * i) + 1) / num_channel_out]);

      if (write_local_float > 32767.0f) {
          write_local_float = 32767.0f;
      } else if (write_local_float < -32768.0f) {
          write_local_float = -32768.0f;
      }
      out_buf[i * ch_fac + 1] = (WORD16)write_local_float;
    }
  }
  *out_bytes = num * sizeof(WORD16);
}

/* audio pre roll frame parsing*/
static WORD32 ixheaacd_audio_preroll_parsing(
    ia_dec_data_struct *pstr_dec_data, UWORD8 *conf_buf, WORD32 *preroll_units,
    WORD32 *preroll_frame_offset, ia_aac_dec_state_struct *aac_dec_handle,
    WORD32 *config_changed, WORD32 *apply_crossfade) {
  ia_bit_buf_struct *temp_buff =
      (ia_bit_buf_struct *)&(pstr_dec_data->dec_bit_buf);

  WORD32 ext_ele_present = 0;
  WORD32 ext_ele_use_dflt_len = 0;
  WORD32 ext_ele_payload_len = 0;
  WORD32 num_pre_roll_frames = 0;

  WORD32 frame_idx = 0;
  WORD32 temp = 0;

  WORD32 config_len = 0;
  WORD32 loop;

  if (pstr_dec_data->str_frame_data.str_audio_specific_config.str_usac_config
          .str_usac_dec_config.usac_element_type[0] == ID_USAC_EXT) {
    temp = ixheaacd_show_bits_buf(temp_buff, 3);
    ext_ele_present = (temp >> 1) & 0x1;

    if (ext_ele_present) {
      ext_ele_use_dflt_len = temp & 0x1;
      if (ext_ele_use_dflt_len != 0) return 0;

      ixheaacd_read_bits_buf(temp_buff, 3);

      ext_ele_payload_len = ixheaacd_read_bits_buf(temp_buff, 8);

      if (ext_ele_payload_len == 255) {
        WORD32 val_add = 0;
        val_add = ixheaacd_read_bits_buf(temp_buff, 16);
        ext_ele_payload_len =
            (UWORD32)((WORD32)ext_ele_payload_len + val_add - 2);
      }

      config_len = ixheaacd_read_bits_buf(temp_buff, 4);
      if (config_len == 15) {
        WORD32 val_add = 0;
        val_add = ixheaacd_read_bits_buf(temp_buff, 4);
        config_len += val_add;
        if (val_add == 15) {
          WORD32 val_add1 = 0;
          val_add1 = ixheaacd_read_bits_buf(temp_buff, 8);
          config_len += val_add1;
        }
      }

      for (loop = 0; loop < config_len; loop++)
        conf_buf[loop] = ixheaacd_read_bits_buf(temp_buff, 8);

      if (aac_dec_handle->preroll_config_present == 1) {
        if (!(memcmp(aac_dec_handle->preroll_config_prev, conf_buf,
                     sizeof(UWORD8) * config_len))) {
          config_len = 0;
        }
        if (memcmp(aac_dec_handle->preroll_config_prev, conf_buf,
                   sizeof(UWORD8) * config_len) != 0) {
          *config_changed = 1;
        } else {
          *config_changed = 0;
        }
      }
      aac_dec_handle->preroll_config_present = 1;
      memcpy(aac_dec_handle->preroll_config_prev, conf_buf,
             sizeof(UWORD8) * config_len);

      *apply_crossfade = ixheaacd_read_bits_buf(temp_buff, 1);
      ixheaacd_read_bits_buf(temp_buff, 1);

      num_pre_roll_frames = ixheaacd_read_bits_buf(temp_buff, 2);
      if (num_pre_roll_frames == 3) {
        WORD32 val_add = 0;
        val_add = ixheaacd_read_bits_buf(temp_buff, 4);
        num_pre_roll_frames += val_add;
      }

      if (num_pre_roll_frames > MAX_AUDIO_PREROLLS) {
        if (pstr_dec_data->str_usac_data.ec_flag) {
          num_pre_roll_frames = 0;
          longjmp(*(pstr_dec_data->xaac_jmp_buf),
                  IA_XHEAAC_DEC_EXE_NONFATAL_INSUFFICIENT_INPUT_BYTES);
        } else {
          return IA_FATAL_ERROR;
        }
      }

      for (frame_idx = 0; frame_idx < num_pre_roll_frames; frame_idx++) {
        WORD32 au_len = 0;
        au_len = ixheaacd_read_bits_buf(temp_buff, 16);
        if (au_len == 65535) {
          WORD32 val_add = ixheaacd_read_bits_buf(temp_buff, 16);
          au_len += val_add;
        }
        if (config_len != 0) {
          preroll_frame_offset[frame_idx] =
              temp_buff->size - temp_buff->cnt_bits;
        }
        temp_buff->ptr_read_next += au_len;
        temp_buff->cnt_bits -= au_len * 8;
        if (temp_buff->cnt_bits < 0) {
          if (pstr_dec_data->str_usac_data.ec_flag) {
            temp_buff->cnt_bits = 0;
            longjmp(*(pstr_dec_data->xaac_jmp_buf),
                    IA_XHEAAC_DEC_EXE_NONFATAL_INSUFFICIENT_INPUT_BYTES);
          } else {
            return IA_FATAL_ERROR;
          }
        }
      }
    }
  }
  if (config_len == 0)
    *preroll_units = 0;
  else
    *preroll_units = num_pre_roll_frames;

  return config_len;
}

WORD32 ixheaacd_dec_main(VOID *temp_handle, WORD8 *inbuffer, WORD8 *outbuffer,
                         WORD32 *out_bytes, WORD32 frames_done, WORD32 pcmsize,
                         WORD32 *num_channel_out) {
  WORD32 err = 0;
  ia_exhaacplus_dec_api_struct *handle =
      (ia_exhaacplus_dec_api_struct *)temp_handle;
  ia_aac_dec_state_struct *aac_dec_handle = handle->p_state_aac;

  WORD32 tmp;
  ia_audio_specific_config_struct *pstr_audio_specific_config =
      (ia_audio_specific_config_struct *)
          aac_dec_handle->ia_audio_specific_config;
  WORD32 suitable_tracks = 1;
  WORD32 num_samples_out;
  ia_dec_data_struct *pstr_dec_data;
  UWORD8 config[MAX_PREROLL_SIZE];
  WORD32 config_len;
  WORD32 delay;
  WORD preroll_frame_offset[MAX_PREROLL_FRAME_OFFSET] = {0};
  WORD preroll_units = -1;
  WORD32 access_units = 0;
  WORD32 bits_consumed = 0;

  if (frames_done == 0) {
    if ((pstr_audio_specific_config->channel_configuration > 2) ||
        (pstr_audio_specific_config->channel_configuration == 0)) {
      return -1;
    }

    pstr_dec_data = (ia_dec_data_struct *)aac_dec_handle->pstr_dec_data;

    tmp = pstr_audio_specific_config->channel_configuration;

    suitable_tracks =
        ixheaacd_frm_data_init(pstr_audio_specific_config, pstr_dec_data);

    pstr_audio_specific_config->channel_configuration = tmp;

    if (suitable_tracks <= 0) {
      return -1;
    }
  }

  {
    WORD32 tot_out_bytes = 0;
    jmp_buf local;
    pstr_dec_data = (ia_dec_data_struct *)aac_dec_handle->pstr_dec_data;
    pstr_dec_data->str_usac_data.frame_ok = 1;
    pstr_dec_data->str_usac_data.ec_flag = aac_dec_handle->p_config->ui_err_conceal;
    if (pstr_dec_data->str_usac_data.ec_flag) {
      err = setjmp(local);
    }

    if (aac_dec_handle->p_config->ui_err_conceal) {
      if (err == 0) {
        if (pstr_dec_data->dec_bit_buf.cnt_bits) {
          aac_dec_handle->ui_in_bytes += (pstr_dec_data->dec_bit_buf.cnt_bits >> 3);
          if (aac_dec_handle->ui_in_bytes > IA_MAX_INP_BUFFER_SIZE) {
            aac_dec_handle->ui_in_bytes = 0;
          }
        }
      } else {
        pstr_dec_data->str_usac_data.frame_ok = 0;
      }
    }

    if (frames_done == 0) {
      WORD32 delay;
      pstr_dec_data->str_usac_data.first_frame = 1;
      if (aac_dec_handle->decode_create_done == 0) {
        delay = ixheaacd_decode_create(
            handle, pstr_dec_data,
            pstr_dec_data->str_frame_data.scal_out_select + 1);
        if (delay == -1) return -1;
      }
      pstr_dec_data->dec_bit_buf.max_size =
          handle->p_mem_info_aac[IA_MEMTYPE_INPUT].ui_size;
      *num_channel_out = pstr_dec_data->str_frame_data.scal_out_num_channels;
      return 0;
    }

    pstr_dec_data->dec_bit_buf.ptr_bit_buf_base = (UWORD8 *)inbuffer;
    pstr_dec_data->dec_bit_buf.size = aac_dec_handle->ui_in_bytes << 3;
    pstr_dec_data->dec_bit_buf.ptr_bit_buf_end =
        (UWORD8 *)inbuffer + aac_dec_handle->ui_in_bytes - 1;
    pstr_dec_data->dec_bit_buf.ptr_read_next = (UWORD8 *)inbuffer;
    pstr_dec_data->dec_bit_buf.bit_pos = 7;
    pstr_dec_data->dec_bit_buf.cnt_bits = pstr_dec_data->dec_bit_buf.size;
    pstr_dec_data->dec_bit_buf.xaac_jmp_buf = &(aac_dec_handle->xaac_jmp_buf);
    if (pstr_dec_data->str_usac_data.ec_flag) {
      pstr_dec_data->xaac_jmp_buf = &local;
    }
    pstr_dec_data->str_usac_data.usac_flag = aac_dec_handle->usac_flag;
    pstr_dec_data->str_usac_data.esbr_hq = handle->aac_config.ui_hq_esbr;
    pstr_dec_data->str_usac_data.enh_sbr = 1;
    pstr_dec_data->str_usac_data.enh_sbr_ps = handle->aac_config.ui_enh_sbr_ps;
    if (pstr_dec_data->dec_bit_buf.size > pstr_dec_data->dec_bit_buf.max_size)
      pstr_dec_data->dec_bit_buf.max_size = pstr_dec_data->dec_bit_buf.size;
    /* audio pre roll frame parsing*/

    if (aac_dec_handle->bs_format == LOAS_BSFORMAT && pstr_dec_data->str_usac_data.frame_ok) {
      WORD32 sync = ixheaacd_read_bits_buf(&pstr_dec_data->dec_bit_buf, 11);
      if (sync == 0x2b7) {
        WORD32 result = ixheaacd_latm_audio_mux_element(
          &pstr_dec_data->dec_bit_buf, &aac_dec_handle->latm_struct_element,
          aac_dec_handle,
          (ia_sampling_rate_info_struct *)&handle->aac_tables
          .pstr_huffmann_tables->str_sample_rate_info[0]);
        if (result < 0) {
          if (aac_dec_handle->p_config->ui_err_conceal)
            pstr_dec_data->str_usac_data.frame_ok = 0;
          else
            return result;
        }
      }
      bits_consumed = pstr_dec_data->dec_bit_buf.size - pstr_dec_data->dec_bit_buf.cnt_bits;
    }

    do {
      config_len = 0;
      if (err == 0 || aac_dec_handle->p_config->ui_err_conceal == 0) {
        if (access_units == 0 &&
            pstr_audio_specific_config->str_usac_config.str_usac_dec_config.preroll_flag) {
          config_len = ixheaacd_audio_preroll_parsing(
              pstr_dec_data, &config[0], &preroll_units, &preroll_frame_offset[0], aac_dec_handle,
              &aac_dec_handle->drc_config_changed, &aac_dec_handle->apply_crossfade);

          if (config_len == IA_FATAL_ERROR) return IA_FATAL_ERROR;
        }

        if (config_len != 0) {
          ia_bit_buf_struct config_bit_buf = {0};

          config_bit_buf.ptr_bit_buf_base = config;
          config_bit_buf.size = config_len << 3;
          config_bit_buf.ptr_read_next = config_bit_buf.ptr_bit_buf_base;
          config_bit_buf.ptr_bit_buf_end = (UWORD8 *)config + config_len;
          config_bit_buf.bit_pos = 7;
          config_bit_buf.cnt_bits = config_bit_buf.size;
          if (pstr_dec_data->str_usac_data.ec_flag) {
            config_bit_buf.xaac_jmp_buf = &local;
          } else {
            config_bit_buf.xaac_jmp_buf = &(aac_dec_handle->xaac_jmp_buf);
          }

          suitable_tracks = ixheaacd_frm_data_init(pstr_audio_specific_config, pstr_dec_data);

          if (suitable_tracks <= 0) return -1;

          aac_dec_handle->decode_create_done = 0;
          if (aac_dec_handle->p_config->ui_err_conceal) {
            if (pstr_dec_data->str_usac_data.frame_ok == 1 && err == 0) {
              err = ixheaacd_config(
                  &config_bit_buf,
                  &(pstr_dec_data->str_frame_data.str_audio_specific_config.str_usac_config),
                  &(pstr_audio_specific_config->channel_configuration),
                  aac_dec_handle->p_config->ui_err_conceal);
              if (err != 0) {
                if (frames_done == 0)
                  return -1;
                else
                  pstr_dec_data->str_usac_data.frame_ok = 0;
              }
            }
          } else {
            err = ixheaacd_config(
                &config_bit_buf,
                &(pstr_dec_data->str_frame_data.str_audio_specific_config.str_usac_config),
                &(pstr_audio_specific_config->channel_configuration),
                aac_dec_handle->p_config->ui_err_conceal);
            if (err != 0) {
              return err;
            }
          }

          pstr_dec_data->str_frame_data.str_audio_specific_config.sampling_frequency =
              pstr_dec_data->str_frame_data.str_audio_specific_config.str_usac_config
                  .usac_sampling_frequency;
          delay = ixheaacd_decode_create(handle, pstr_dec_data,
                                         pstr_dec_data->str_frame_data.scal_out_select + 1);
          if (delay == -1) return -1;
          *num_channel_out = pstr_dec_data->str_frame_data.scal_out_num_channels;
        }
      } else {
        pstr_dec_data->str_usac_data.frame_ok = 0;
      }

      pstr_dec_data->dec_bit_buf.ptr_bit_buf_base = (UWORD8 *)inbuffer;
      pstr_dec_data->dec_bit_buf.size = aac_dec_handle->ui_in_bytes << 3;
      pstr_dec_data->dec_bit_buf.ptr_bit_buf_end =
          (UWORD8 *)inbuffer + aac_dec_handle->ui_in_bytes - 1;
      pstr_dec_data->dec_bit_buf.ptr_read_next = (UWORD8 *)inbuffer;
      pstr_dec_data->dec_bit_buf.bit_pos = 7;
      pstr_dec_data->dec_bit_buf.cnt_bits = pstr_dec_data->dec_bit_buf.size;
      pstr_dec_data->dec_bit_buf.xaac_jmp_buf = &(aac_dec_handle->xaac_jmp_buf);

      pstr_dec_data->str_usac_data.usac_flag = aac_dec_handle->usac_flag;
      pstr_dec_data->str_usac_data.esbr_hq = handle->aac_config.ui_hq_esbr;
      pstr_dec_data->str_usac_data.enh_sbr = 1;
      pstr_dec_data->str_usac_data.enh_sbr_ps = handle->aac_config.ui_enh_sbr_ps;

      if (preroll_frame_offset[access_units] &&
          ((pstr_dec_data->str_usac_data.ec_flag && pstr_dec_data->str_usac_data.frame_ok == 1) ||
           pstr_dec_data->str_usac_data.ec_flag == 0)) {
        pstr_dec_data->dec_bit_buf.cnt_bits =
            pstr_dec_data->dec_bit_buf.size -
            preroll_frame_offset[access_units];
        pstr_dec_data->dec_bit_buf.bit_pos =
            7 - preroll_frame_offset[access_units] % 8;
        pstr_dec_data->dec_bit_buf.ptr_read_next =
            pstr_dec_data->dec_bit_buf.ptr_read_next +
            (preroll_frame_offset[access_units] / 8);
      } else {
        pstr_dec_data->dec_bit_buf.cnt_bits =
          pstr_dec_data->dec_bit_buf.size -
          (bits_consumed);
        pstr_dec_data->dec_bit_buf.bit_pos =
          7 - (bits_consumed) % 8;
        pstr_dec_data->dec_bit_buf.ptr_read_next =
          pstr_dec_data->dec_bit_buf.ptr_read_next +
          (bits_consumed / 8);
      }

      if (pstr_dec_data->str_usac_data.ec_flag) {
        if (!aac_dec_handle->decode_create_done && pstr_dec_data->str_usac_data.frame_ok == 1 &&
            config_len != 0)
          return IA_FATAL_ERROR;
      } else {
        if (!aac_dec_handle->decode_create_done) return IA_FATAL_ERROR;
      }

      err =
          ixheaacd_usac_process(pstr_dec_data, num_channel_out, aac_dec_handle);

      switch (pstr_dec_data->str_usac_data.sbr_ratio_idx) {
        case 0:
          handle->aac_config.ui_sbr_mode = 0;
          break;
        case 1:
          handle->aac_config.ui_sbr_mode = 1;
          break;
        case 2:
          handle->aac_config.ui_sbr_mode = 1;
          break;
        case 3:
          handle->aac_config.ui_sbr_mode = 3;
          break;

        default:
          handle->aac_config.ui_sbr_mode = 0;
      }

      if (err == -1) return err;

      num_samples_out = pstr_dec_data->str_usac_data.output_samples;
      if (!handle->aac_config.peak_limiter_off && pstr_dec_data->str_usac_data.ec_flag) {
        aac_dec_handle->peak_limiter.num_channels = *num_channel_out;

        ixheaacd_peak_limiter_process_float(&aac_dec_handle->peak_limiter,
                                            pstr_dec_data->str_usac_data.time_sample_vector,
                                            num_samples_out);
      }

      ixheaacd_samples_sat((WORD8 *)outbuffer + tot_out_bytes, num_samples_out,
                           pcmsize,
                           pstr_dec_data->str_usac_data.time_sample_vector,
                           out_bytes, *num_channel_out);
      {
        WORD32 preroll_counter =
            pstr_dec_data->str_frame_data.str_audio_specific_config
                .str_usac_config.str_usac_dec_config.preroll_counter;

        UWORD8 i;  // for looping index used for payload calculation
        WORD32 payload_buffer_offset = 0;
        WORD32 copy_bytes =
            pstr_dec_data->str_frame_data.str_audio_specific_config
                .str_usac_config.str_usac_dec_config
                .usac_ext_gain_payload_len[preroll_counter] *
            sizeof(WORD8);

        pstr_audio_specific_config->str_usac_config.str_usac_dec_config
            .usac_ext_gain_payload_len[preroll_counter] =
            pstr_dec_data->str_frame_data.str_audio_specific_config
                .str_usac_config.str_usac_dec_config
                .usac_ext_gain_payload_len[preroll_counter];

        for (i = 0; i < preroll_counter; i++)
          payload_buffer_offset +=
              pstr_dec_data->str_frame_data.str_audio_specific_config
                  .str_usac_config.str_usac_dec_config
                  .usac_ext_gain_payload_len[i] *
              sizeof(WORD8);

        memcpy(pstr_audio_specific_config->str_usac_config.str_usac_dec_config
                       .usac_ext_gain_payload_buf +
                   payload_buffer_offset,
               pstr_dec_data->str_frame_data.str_audio_specific_config
                       .str_usac_config.str_usac_dec_config
                       .usac_ext_gain_payload_buf +
                   payload_buffer_offset,
               copy_bytes);

        pstr_audio_specific_config->str_usac_config.str_usac_dec_config
            .preroll_bytes[preroll_counter] = *out_bytes;

        preroll_counter++;

        if (preroll_counter > (MAX_AUDIO_PREROLLS + 1)) return IA_FATAL_ERROR;

        pstr_dec_data->str_frame_data.str_audio_specific_config.str_usac_config
            .str_usac_dec_config.preroll_counter = preroll_counter;

        ia_usac_decoder_config_struct *pstr_usac_dec_config_state =
            &pstr_audio_specific_config->str_usac_config.str_usac_dec_config;
        ia_usac_decoder_config_struct *pstr_usac_dec_config_dec_data =
            &pstr_dec_data->str_frame_data.str_audio_specific_config.str_usac_config
            .str_usac_dec_config;
        pstr_usac_dec_config_state->num_config_extensions =
            pstr_usac_dec_config_dec_data->num_config_extensions;
        pstr_usac_dec_config_state->num_elements =
            pstr_usac_dec_config_dec_data->num_elements;
        memcpy(pstr_usac_dec_config_state->usac_cfg_ext_info_buf,
            pstr_usac_dec_config_dec_data->usac_cfg_ext_info_buf,
            sizeof(pstr_usac_dec_config_state->usac_cfg_ext_info_buf));
        memcpy(pstr_usac_dec_config_state->usac_ext_ele_payload_present,
            pstr_usac_dec_config_dec_data->usac_ext_ele_payload_present,
            sizeof(pstr_usac_dec_config_dec_data->usac_ext_ele_payload_present));
        memcpy(pstr_usac_dec_config_state->usac_ext_ele_payload_buf,
            pstr_usac_dec_config_dec_data->usac_ext_ele_payload_buf,
            sizeof(pstr_usac_dec_config_state->usac_ext_ele_payload_buf));
      }

      access_units++;
      preroll_units--;
      tot_out_bytes += (*out_bytes);
    } while (preroll_units >= 0);
    *out_bytes = tot_out_bytes;
  }

  if (aac_dec_handle->bs_format == LOAS_BSFORMAT) {
    pstr_dec_data->dec_bit_buf.ptr_bit_buf_base = (UWORD8 *)inbuffer;
    pstr_dec_data->dec_bit_buf.size = aac_dec_handle->ui_in_bytes << 3;
    pstr_dec_data->dec_bit_buf.ptr_bit_buf_end =
      (UWORD8 *)inbuffer + aac_dec_handle->ui_in_bytes - 1;
    pstr_dec_data->dec_bit_buf.ptr_read_next = (UWORD8 *)inbuffer;
    pstr_dec_data->dec_bit_buf.bit_pos = 7;
    pstr_dec_data->dec_bit_buf.cnt_bits = pstr_dec_data->dec_bit_buf.size;
    pstr_dec_data->dec_bit_buf.xaac_jmp_buf = &(aac_dec_handle->xaac_jmp_buf);

    ixheaacd_read_bits_buf(&pstr_dec_data->dec_bit_buf, 11);
    aac_dec_handle->i_bytes_consumed =
        ixheaacd_read_bits_buf(&pstr_dec_data->dec_bit_buf, 13) + 3;
  }

  return err;
}
