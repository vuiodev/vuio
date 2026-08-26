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
#include "ixheaacd_defines.h"
#include "ixheaacd_error_codes.h"

#include "ixheaacd_intrinsics.h"
#include "ixheaac_sbr_const.h"
#include "ixheaac_basic_op.h"
#include "ixheaacd_defines.h"
#include "ixheaacd_bitbuffer.h"
#include "ixheaacd_pns.h"

#include "ixheaacd_aac_rom.h"
#include "ixheaacd_pulsedata.h"

#include "ixheaacd_drc_data_struct.h"
#include "ixheaacd_lt_predict.h"
#include "ixheaacd_cnst.h"
#include "ixheaacd_ec_defines.h"
#include "ixheaacd_ec_struct_def.h"
#include "ixheaacd_channelinfo.h"
#include "ixheaacd_drc_dec.h"

#include "ixheaacd_sbrdecoder.h"

#include "ixheaacd_sbrdecsettings.h"
#include "ixheaacd_sbr_scale.h"
#include "ixheaacd_lpp_tran.h"
#include "ixheaacd_env_extr_part.h"
#include "ixheaacd_sbr_rom.h"
#include "ixheaacd_hybrid.h"
#include "ixheaacd_ps_dec.h"
#include "ixheaacd_ps_bitdec.h"
#include "ixheaacd_env_extr.h"
#include "ixheaacd_common_rom.h"
#include "ixheaacd_freq_sca.h"

#include "ixheaacd_qmf_dec.h"

#include "ixheaacd_env_calc.h"

#include "ixheaacd_pvc_dec.h"
#include "ixheaacd_sbr_dec.h"
#include "ixheaacd_env_dec.h"
#include "ixheaacd_basic_funcs.h"
#include "ixheaacd_sbr_crc.h"

#include "ixheaacd_audioobjtypes.h"

static VOID ixheaacd_ec_set_frame_error_flag(ia_sbr_element_stream_struct *pstr_sbr_element,
                                             WORD32 value) {
  if (pstr_sbr_element != NULL) {
    switch (value) {
      case FRAME_ERROR_ALLSLOTS:
        pstr_sbr_element->frame_error_flag[0] = FRAME_ERROR;
        pstr_sbr_element->frame_error_flag[1] = FRAME_ERROR;
        break;
      default:
        pstr_sbr_element->frame_error_flag[pstr_sbr_element->use_frame_slot] = value;
    }
  }
}

VOID ixheaacd_downmix_to_monosbr(WORD16 *core_sample_buf, WORD32 ch_fac) {
  WORD32 i;
  WORD16 *ptr1 = &core_sample_buf[0];
  WORD16 *ptr2 = &core_sample_buf[1];

  for (i = MAX_FRAME_SIZE - 1; i >= 0; i--) {
    *ptr1 = ((*ptr1 >> 1) + (*ptr2 >> 1));

    ptr1 += ch_fac;
    ptr2 += ch_fac;
  }
}

static WORD32 ixheaacd_sbr_dec_reset(ia_sbr_dec_struct *ptr_sbr_dec,
                                     ia_sbr_header_data_struct *ptr_header_data,
                                     FLAG low_pow_flag,
                                     ixheaacd_misc_tables *pstr_common_tables,
                                     WORD32 pitch_in_bins,
                                     WORD32 audio_object_type,
                                     WORD32 *ptr_work_buf_core) {
  WORD32 old_lsb, new_lsb;
  WORD32 l;
  WORD32 err = 0;
  WORD32 num_time_slots = ptr_sbr_dec->str_codec_qmf_bank.num_time_slots;
  WORD32 upsample_ratio_idx = ptr_header_data->sbr_ratio_idx;
  WORD32 op_delay = 6 + SBR_HF_ADJ_OFFSET;
  WORD32 hbe_flag = ptr_header_data->hbe_flag;
  WORD32 usac_flag = ptr_header_data->usac_flag;

  if (ptr_header_data->is_usf_4) {
    op_delay = op_delay + 6;
  }

  ixheaacd_reset_sbrenvelope_calc(&ptr_sbr_dec->str_sbr_calc_env);

  new_lsb = ptr_header_data->pstr_freq_band_data->sub_band_start;
  ptr_sbr_dec->str_synthesis_qmf_bank.lsb = new_lsb;
  ptr_sbr_dec->str_synthesis_qmf_bank.usb =
      ptr_header_data->pstr_freq_band_data->sub_band_end;

  old_lsb = ptr_sbr_dec->str_synthesis_qmf_bank.lsb;
  ptr_sbr_dec->str_codec_qmf_bank.lsb = 0;
  ptr_sbr_dec->str_codec_qmf_bank.usb = old_lsb;

  {
    WORD32 *plpc_filt_states_real =
        &ptr_sbr_dec->str_hf_generator.lpc_filt_states_real[0][old_lsb];
    WORD32 *plpc_filt_states_real_1 =
        &ptr_sbr_dec->str_hf_generator.lpc_filt_states_real[1][old_lsb];

    WORD32 *plpc_filt_states_imag =
        &ptr_sbr_dec->str_hf_generator.lpc_filt_states_imag[0][old_lsb];
    WORD32 *plpc_filt_states_imag_1 =
        &ptr_sbr_dec->str_hf_generator.lpc_filt_states_imag[1][old_lsb];

    for (l = new_lsb - old_lsb - 1; l >= 0; l--) {
      *plpc_filt_states_real++ = *plpc_filt_states_real_1++ = 0L;

      if (!low_pow_flag) {
        *plpc_filt_states_imag++ = *plpc_filt_states_imag_1++ = 0L;
      }
    }
  }
  if (1) {
    WORD32 start_band;
    WORD32 stop_band;
    WORD32 start_slot = SBR_HF_ADJ_OFFSET;
    WORD32 k;

    start_band = ptr_header_data->pstr_freq_band_data->qmf_sb_prev;
    stop_band = ptr_header_data->pstr_freq_band_data->sub_band_start;
    if (usac_flag && !hbe_flag) {
      for (l = 0; l < SBR_HF_ADJ_OFFSET; l++) {
        for (k = start_band; k < stop_band; k++) {
          ptr_sbr_dec->qmf_buf_real[l][k] = 0.0;
          ptr_sbr_dec->qmf_buf_imag[l][k] = 0.0;
        }
      }
      for (l = start_slot; l < op_delay; l++) {
        for (k = start_band; k < stop_band; k++) {
          ptr_sbr_dec->qmf_buf_real[l][k] = 0.0;
          ptr_sbr_dec->qmf_buf_imag[l][k] = 0.0;
        }
      }
    }
    if (ptr_sbr_dec->p_hbe_txposer != NULL && (usac_flag || hbe_flag)) {
      WORD32 k, i;
      WORD32 dft_hbe_flag = ptr_header_data->esbr_hq;
      if (dft_hbe_flag == 1) {
        err = ixheaacd_dft_hbe_data_reinit(
            ptr_sbr_dec->p_hbe_txposer,
            ptr_header_data->pstr_freq_band_data->freq_band_table,
            ptr_header_data->pstr_freq_band_data->num_sf_bands);
      } else {
        err = ixheaacd_qmf_hbe_data_reinit(
            ptr_sbr_dec->p_hbe_txposer,
            ptr_header_data->pstr_freq_band_data->freq_band_table,
            ptr_header_data->pstr_freq_band_data->num_sf_bands,
            ptr_header_data->is_usf_4);
      }
      if (err) return err;

      for (k = 0; k < 2; k++) {
        if (!((upsample_ratio_idx == SBR_UPSAMPLE_IDX_4_1) && (k == 0))) {
          WORD32 xpos_delay = num_time_slots * k;
          if (upsample_ratio_idx == SBR_UPSAMPLE_IDX_4_1) {
            xpos_delay = num_time_slots * k - 32;
          }

          for (i = 0; i < 8; i++) {
            memmove(ptr_sbr_dec->ph_vocod_qmf_real[i],
                    ptr_sbr_dec->ph_vocod_qmf_real[num_time_slots + i],
                    64 * sizeof(FLOAT32));
            memmove(ptr_sbr_dec->ph_vocod_qmf_imag[i],
                    ptr_sbr_dec->ph_vocod_qmf_imag[num_time_slots + i],
                    64 * sizeof(FLOAT32));
          }
          if (dft_hbe_flag == 1) {
            err = ixheaacd_dft_hbe_apply(
                ptr_sbr_dec->p_hbe_txposer,
                ptr_sbr_dec->qmf_buf_real + op_delay + xpos_delay,
                ptr_sbr_dec->qmf_buf_imag + op_delay + xpos_delay, num_time_slots,
                ptr_sbr_dec->ph_vocod_qmf_real + op_delay,
                ptr_sbr_dec->ph_vocod_qmf_imag + op_delay, pitch_in_bins,
                (FLOAT32 *)ptr_work_buf_core);
            if (err)
                return err;
          } else {
            err = ixheaacd_qmf_hbe_apply(
                ptr_sbr_dec->p_hbe_txposer,
                ptr_sbr_dec->qmf_buf_real + op_delay + xpos_delay,
                ptr_sbr_dec->qmf_buf_imag + op_delay + xpos_delay, num_time_slots,
                ptr_sbr_dec->ph_vocod_qmf_real + op_delay,
                ptr_sbr_dec->ph_vocod_qmf_imag + op_delay, pitch_in_bins,
                ptr_header_data);
          if (err) return err;
          }

          if (upsample_ratio_idx == SBR_UPSAMPLE_IDX_4_1) {
            ixheaacd_hbe_repl_spec(&ptr_sbr_dec->p_hbe_txposer->x_over_qmf[0],
                                   ptr_sbr_dec->ph_vocod_qmf_real + op_delay,
                                   ptr_sbr_dec->ph_vocod_qmf_imag + op_delay,
                                   num_time_slots,
                                   ptr_sbr_dec->p_hbe_txposer->max_stretch);
          }
        }
      }
    }
  }

  if (!usac_flag) {
    err |= ixheaacd_reset_hf_generator(&ptr_sbr_dec->str_hf_generator,
                                       ptr_header_data, audio_object_type);

    ixheaacd_derive_lim_band_tbl(
        ptr_header_data,
        ptr_sbr_dec->str_hf_generator.pstr_settings->str_patch_param,
        ptr_sbr_dec->str_hf_generator.pstr_settings->num_patches,
        pstr_common_tables);
  }

  return err;
}

VOID ixheaacd_prepare_upsamp(ia_sbr_header_data_struct **ptr_header_data,
                             ia_sbr_channel_struct *pstr_sbr_channel[2],
                             WORD32 num_channels) {
  WORD32 lr;
  ia_sbr_qmf_filter_bank_struct *sbr_qmf_bank;

  for (lr = 0; lr < num_channels; lr++) {
    ptr_header_data[lr]->pstr_freq_band_data->sub_band_start =
        NO_ANALYSIS_CHANNELS;
    ptr_header_data[lr]->pstr_freq_band_data->sub_band_end =
        NO_SYNTHESIS_CHANNELS;

    sbr_qmf_bank = &pstr_sbr_channel[lr]->str_sbr_dec.str_synthesis_qmf_bank;
    sbr_qmf_bank->lsb = NO_ANALYSIS_CHANNELS;
    sbr_qmf_bank->usb = NO_SYNTHESIS_CHANNELS;

    sbr_qmf_bank = &pstr_sbr_channel[lr]->str_sbr_dec.str_codec_qmf_bank;
    sbr_qmf_bank->lsb = 0;
    sbr_qmf_bank->usb = NO_ANALYSIS_CHANNELS;
    ptr_header_data[lr]->sync_state = UPSAMPLING;
  }
  return;
}

static VOID ixheaacd_copy_prev_ps_params(ia_ps_dec_struct *ps_config_curr,
                                         ia_ps_dec_config_struct *ps_config_prev,
                                         WORD32 frame_status) {
  if (frame_status == 0) {
    ps_config_curr->enable_iid = ps_config_prev->enable_iid;
    ps_config_curr->iid_mode = ps_config_prev->iid_mode;
    ps_config_curr->enable_icc = ps_config_prev->enable_icc;
    ps_config_curr->icc_mode = ps_config_prev->icc_mode;
    ps_config_curr->frame_class = ps_config_prev->frame_class;
    ps_config_curr->freq_res_ipd = ps_config_prev->freq_res_ipd;
    memcpy(ps_config_curr->border_position, ps_config_prev->border_position,
           sizeof(ps_config_curr->border_position));
    memcpy(ps_config_curr->iid_dt, ps_config_prev->iid_dt, sizeof(ps_config_curr->iid_dt));
    memcpy(ps_config_curr->iid_par_table, ps_config_prev->iid_par_table,
           sizeof(ps_config_curr->iid_par_table));
    memcpy(ps_config_curr->icc_dt, ps_config_prev->icc_dt, sizeof(ps_config_curr->icc_dt));
    memcpy(ps_config_curr->icc_par_table, ps_config_prev->icc_par_table,
           sizeof(ps_config_curr->icc_par_table));
  } else {
    ps_config_prev->enable_iid = ps_config_curr->enable_iid;
    ps_config_prev->iid_mode = ps_config_curr->iid_mode;
    ps_config_prev->enable_icc = ps_config_curr->enable_icc;
    ps_config_prev->icc_mode = ps_config_curr->icc_mode;
    ps_config_prev->frame_class = ps_config_curr->frame_class;
    ps_config_prev->freq_res_ipd = ps_config_curr->freq_res_ipd;
    memcpy(ps_config_prev->border_position, ps_config_curr->border_position,
           sizeof(ps_config_prev->border_position));
    memcpy(ps_config_prev->iid_dt, ps_config_curr->iid_dt, sizeof(ps_config_prev->iid_dt));
    memcpy(ps_config_prev->iid_par_table, ps_config_curr->iid_par_table,
           sizeof(ps_config_prev->iid_par_table));
    memcpy(ps_config_prev->icc_dt, ps_config_curr->icc_dt, sizeof(ps_config_prev->icc_dt));
    memcpy(ps_config_prev->icc_par_table, ps_config_curr->icc_par_table,
           sizeof(ps_config_prev->icc_par_table));
  }
}
IA_ERRORCODE ixheaacd_applysbr(
    ia_handle_sbr_dec_inst_struct self, ia_aac_dec_sbr_bitstream_struct *p_sbr_bit_stream,
    WORD16 *core_sample_buf, WORD16 *codec_num_channels, FLAG frame_status, FLAG down_samp_flag,
    FLAG down_mix_flag, ia_sbr_scr_struct *sbr_scratch_struct, WORD32 ps_enable, WORD32 ch_fac,
    WORD32 slot_element, ia_bit_buf_struct *it_bit_buff, ia_drc_dec_struct *pstr_drc_dec,
    WORD eld_sbr_flag, WORD32 audio_object_type, WORD32 init_flag, WORD32 ldmps_present,
    WORD32 frame_size, WORD32 heaac_mps_present, WORD32 ec_flag, FLAG first_frame) {
  WORD32 k;
  FLAG prev_ps_flag = 0;
  FLAG ps_flag = 0;
  FLAG stereo = 0;
  FLAG low_pow_flag = 0;
  FLAG header_flag = 1;
  FLAG dual_mono = 0;
  WORD32 err = 0;
  WORD32 num_channels = *codec_num_channels;
  FLAG prev_stereo;
  WORD32 ele_channels = 0;
  WORD32 num_elements = p_sbr_bit_stream->no_elements;
  WORD32 usac_flag = self->aot_usac_flag;
  WORD32 total_bits_left = 0;

  ia_sbr_channel_struct *pstr_sbr_channel[2];
  ia_sbr_header_data_struct *ptr_header_data[MAXNRSBRCHANNELS];

  WORD32 initial_sync_state;

  ia_sbr_header_data_struct *ptr_sbr_dflt_header =
      (ia_sbr_header_data_struct *)(&self->str_sbr_dflt_header);

  ia_sbr_frame_info_data_struct *ptr_frame_data[2];
  self->num_delay_frames = 1;
  self->ptr_mps_data = NULL;

  if (!ec_flag || !usac_flag) {
    for (k = 0; k < 2; k++) {
      ptr_frame_data[k] = (ia_sbr_frame_info_data_struct *)self->frame_buffer[k];

      pstr_sbr_channel[k] = self->pstr_sbr_channel[k];

      ptr_header_data[k] = self->pstr_sbr_header[k];

      if (audio_object_type == AOT_ER_AAC_ELD) {
        ptr_frame_data[k]->eld_sbr_flag = eld_sbr_flag;
        ptr_frame_data[k]->num_time_slots = ptr_header_data[0]->num_time_slots;
      }

      ptr_frame_data[k]->usac_independency_flag = self->usac_independency_flag;
      ptr_frame_data[k]->mps_sbr_flag = (self->stereo_config_idx == 3) ? 1 : 0;
      ptr_frame_data[k]->stereo_config_idx = self->stereo_config_idx;
      ptr_frame_data[k]->inter_tes_flag = self->inter_tes_flag;
      ptr_frame_data[k]->sbr_mode = self->sbr_mode;

      if (!usac_flag) {
        ptr_frame_data[k]->usac_independency_flag = 0;
        ptr_frame_data[k]->mps_sbr_flag = 0;
        ptr_frame_data[k]->stereo_config_idx = -1;
        ptr_frame_data[k]->inter_tes_flag = 0;
        ptr_frame_data[k]->sbr_mode = ORIG_SBR;
      }
    }

    if (init_flag) {
      ptr_frame_data[1]->reset_flag = 1;
      ptr_frame_data[0]->reset_flag = 1;
    }

    for (k = 0; k < *codec_num_channels; k++) {
      ptr_header_data[k]->usac_flag = self->aot_usac_flag;
      ptr_header_data[k]->enh_sbr = self->enh_sbr;
      ptr_header_data[k]->enh_sbr_ps =
          ((self->enh_sbr_ps) | (ptr_header_data[k]->channel_mode == PS_STEREO));

      ptr_header_data[k]->usac_independency_flag = self->usac_independency_flag;
      ptr_header_data[k]->hbe_flag = self->hbe_flag;
      ptr_header_data[k]->pvc_flag = self->pvc_flag;

      if (!usac_flag) {
        ptr_header_data[k]->usac_independency_flag = 0;
        ptr_header_data[k]->hbe_flag = 0;
        ptr_header_data[k]->pvc_flag = 0;
      }
      if (self->enh_sbr)
      {
        ptr_header_data[k]->esbr_hq = self->esbr_hq;
      }

      if (!usac_flag && (!(audio_object_type == AOT_ER_AAC_ELD ||
          audio_object_type == AOT_ER_AAC_LD)) && self->enh_sbr) {
        ptr_header_data[k]->hbe_flag = 1;
      }
    }

    initial_sync_state = ptr_header_data[0]->sync_state;

    low_pow_flag = !usac_flag;
    self->pstr_sbr_tables->sbr_rand_ph = self->pstr_sbr_tables->env_calc_tables_ptr->sbr_rand_ph;

    if (ps_enable) {
      if (num_channels == 1) {
        low_pow_flag = 0;
      }
    }

    if (audio_object_type == AOT_ER_AAC_ELD || (ps_enable && heaac_mps_present)) {
      low_pow_flag = 0;
    }

    prev_stereo = (ptr_header_data[0]->channel_mode == SBR_STEREO);

    if (ps_enable) prev_ps_flag = (ptr_header_data[0]->channel_mode == PS_STEREO);

    ptr_header_data[0]->err_flag_prev = ptr_header_data[0]->err_flag;

    if (p_sbr_bit_stream->no_elements == 0) {
      frame_status = 0;
      ptr_header_data[0]->sync_state = UPSAMPLING;
      if (num_channels == 2) ptr_header_data[1]->sync_state = UPSAMPLING;
    }

    if ((usac_flag)) {
      if ((p_sbr_bit_stream->no_elements) && (p_sbr_bit_stream->str_sbr_ele->size_payload > 0)) {
        num_elements = p_sbr_bit_stream->no_elements;
      } else {
        num_elements = 0;
      }
    }

    for (k = 0; k < num_elements; k++) {
      struct ia_bit_buf_struct local_bit_buf = {0};
      ia_sbr_element_stream_struct *ptr_bit_str_ele = &p_sbr_bit_stream->str_sbr_ele[k];
      ele_channels = (p_sbr_bit_stream->str_sbr_ele[0].sbr_ele_id == SBR_ID_CPE) ? 2 : 1;
      if (!frame_status && ptr_header_data[k]->sync_state == SBR_ACTIVE && ec_flag) {
        ixheaacd_ec_set_frame_error_flag(ptr_bit_str_ele, FRAME_ERROR_ALLSLOTS);
      }

      switch (ptr_bit_str_ele->sbr_ele_id) {
        case SBR_ID_SCE:
        case SBR_ID_CCE:
          if (num_channels == 2) {
            dual_mono = 1;
          }
          stereo = 0;
          break;
        case SBR_ID_CPE:
          stereo = 1;
          ptr_header_data[1] = ptr_header_data[0];

          memcpy(self->pstr_sbr_header[1], self->pstr_sbr_header[0],
                 sizeof(ia_sbr_header_data_struct));
          break;
        default:
          frame_status = 0;
      }

      if (frame_status) {
        if (!usac_flag) {
          if (!(audio_object_type == AOT_ER_AAC_LD || audio_object_type == AOT_ER_AAC_ELD)
              && self->enh_sbr) {
            WORD8 tmp[1024];
            WORD32 tmp_payload;
            memcpy(&tmp[0], ptr_bit_str_ele->ptr_sbr_data, ptr_bit_str_ele->size_payload);
            memcpy(ptr_bit_str_ele->ptr_sbr_data, ptr_bit_str_ele->ptr_prev_sbr_data,
                   ptr_bit_str_ele->prev_size_payload);
            memcpy(ptr_bit_str_ele->ptr_prev_sbr_data, &tmp[0], ptr_bit_str_ele->size_payload);
            tmp_payload = ptr_bit_str_ele->size_payload;
            ptr_bit_str_ele->size_payload = ptr_bit_str_ele->prev_size_payload;
            ptr_bit_str_ele->prev_size_payload = tmp_payload;
          }
          if (!ptr_bit_str_ele->size_payload) {
            continue;
          }

          ixheaacd_create_init_bit_buf(&local_bit_buf, (UWORD8 *)ptr_bit_str_ele->ptr_sbr_data,
                                       ptr_bit_str_ele->size_payload);
          it_bit_buff = &local_bit_buf;
          it_bit_buff->xaac_jmp_buf = self->xaac_jmp_buf;
          total_bits_left = it_bit_buff->cnt_bits;

          if (audio_object_type == AOT_ER_AAC_ELD) {
            if (eld_sbr_flag != 1) {
              ixheaacd_read_bits_buf(&local_bit_buf, LEN_NIBBLE);
            }
          } else {
            ixheaacd_read_bits_buf(&local_bit_buf, LEN_NIBBLE);
          }
        }
        if (ptr_bit_str_ele->extension_type == SBR_EXTENSION_CRC) {
          WORD32 crc_bits = 0;
          WORD32 crc_check_flag = 0;
          crc_check_flag = 1;
          crc_bits = (((ptr_bit_str_ele->size_payload - 1) << 3) + (4 - SBR_CYC_REDCY_CHK_BITS));

          if (crc_bits < 0) {
            crc_check_flag = 0;
            frame_status = 0;
          }
          if (crc_check_flag) frame_status = ixheaacd_sbr_crccheck(it_bit_buff, crc_bits);
        }

        if (!usac_flag) header_flag = ixheaacd_read_bits_buf(it_bit_buff, 1);

        if (audio_object_type != AOT_ER_AAC_ELD) {
          if (header_flag) {
            header_flag = ixheaacd_sbr_read_header_data(ptr_header_data[k], it_bit_buff, stereo,
                                                        ptr_sbr_dflt_header);
            if (usac_flag) {
              if ((self->ptr_pvc_data_str->prev_pvc_mode == 0) &&
                  (ptr_header_data[k]->pvc_mode != 0)) {
                self->ptr_pvc_data_str->prev_pvc_id = 0;
              }
              self->ptr_pvc_data_str->prev_pvc_mode = ptr_header_data[k]->pvc_mode;
              if (ptr_header_data[k]->pvc_mode == 0) {
                ptr_frame_data[k]->sbr_mode = ORIG_SBR;
              } else {
                ptr_frame_data[k]->sbr_mode = PVC_SBR;
              }
            }
            if (header_flag == SBR_RESET) {
              err = ixheaacd_calc_frq_bnd_tbls(ptr_header_data[k],

                                               self->pstr_common_tables);
              if (!err) {
                WORD32 lr;
                WORD32 lr1 = ps_enable ? 2 : num_channels;
                for (lr = 0; lr < lr1; lr++) {
                  if (ldmps_present != 1) ptr_frame_data[lr]->reset_flag = 1;
                  if ((SBR_NOT_INITIALIZED == ptr_header_data[lr]->sync_state) && !usac_flag) {
                    ptr_frame_data[lr]->sbr_patching_mode = 1;
                    ptr_frame_data[lr]->over_sampling_flag = 0;
                    ptr_frame_data[lr]->pitch_in_bins = 0;
                    ptr_header_data[lr]->pre_proc_flag = 0;
                  }
                  err |= ixheaacd_sbr_dec_reset(
                      &(pstr_sbr_channel[lr]->str_sbr_dec), ptr_header_data[k], low_pow_flag,
                      self->pstr_common_tables, ptr_frame_data[k]->pitch_in_bins,
                      audio_object_type, sbr_scratch_struct->ptr_work_buf_core);
                  if (err < 0) return err;
                }
              }

              if (err == 0) {
                ptr_header_data[k]->sync_state = SBR_ACTIVE;
              }
            }
          }
        } else {
          if (header_flag) {
            header_flag = ixheaacd_sbr_read_header_data(ptr_header_data[k], it_bit_buff, stereo,
                                                        ptr_sbr_dflt_header);
            if (usac_flag) {
              if ((self->ptr_pvc_data_str->prev_pvc_mode == 0) &&
                  (ptr_header_data[k]->pvc_mode != 0)) {
                self->ptr_pvc_data_str->prev_pvc_id = 0;
              }
              self->ptr_pvc_data_str->prev_pvc_mode = ptr_header_data[k]->pvc_mode;
              if (ptr_header_data[k]->pvc_mode == 0) {
                ptr_frame_data[k]->sbr_mode = ORIG_SBR;
              } else {
                ptr_frame_data[k]->sbr_mode = PVC_SBR;
              }
            }

            if (header_flag == SBR_RESET) {
              err = ixheaacd_calc_frq_bnd_tbls(ptr_header_data[k],

                                               self->pstr_common_tables);
              if (err) {
                return err;
              }
            }
          }

          {
            WORD32 lr;
            WORD32 lr1 = ps_enable ? 2 : num_channels;
            for (lr = 0; lr < lr1; lr++) {
              if (ldmps_present != 1) ptr_frame_data[lr]->reset_flag = 1;
              if (ptr_header_data[k]->status) {
                err |= ixheaacd_sbr_dec_reset(
                    &(pstr_sbr_channel[lr]->str_sbr_dec), ptr_header_data[k], low_pow_flag,
                    self->pstr_common_tables, ptr_frame_data[k]->pitch_in_bins, audio_object_type,
                    sbr_scratch_struct->ptr_work_buf_core);
                if (err < 0) return err;
              }
            }
            ptr_header_data[k]->status = 0;
          }

          if (err == 0) {
            ptr_header_data[k]->sync_state = SBR_ACTIVE;
          }
        }
      }

      if (err || (ptr_header_data[k]->sync_state == SBR_NOT_INITIALIZED)) {
        WORD32 lr1 = ps_enable ? 2 : num_channels;
        ixheaacd_prepare_upsamp(ptr_header_data, pstr_sbr_channel, lr1);
        if (err) return err;
      }

      if (frame_status && (ptr_header_data[k]->sync_state == SBR_ACTIVE)) {
        if (stereo) {
          frame_status = ixheaacd_sbr_read_cpe(ptr_header_data[0], ptr_frame_data, it_bit_buff,
                                               self->pstr_sbr_tables, audio_object_type);
          if (usac_flag && (frame_status == 0)) return -1;
          if (frame_status < 0) return frame_status;
        } else {
          if (ps_enable) {
            if (down_mix_flag) {
              self->pstr_ps_stereo_dec->force_mono = 1;
            } else {
              self->pstr_ps_stereo_dec->force_mono = 0;
            }
          } else {
            self->pstr_ps_stereo_dec = 0;
          }
          if (ptr_frame_data[k]->sbr_mode == ORIG_SBR) {
            frame_status = ixheaacd_sbr_read_sce(
                ptr_header_data[k], ptr_frame_data[k], self->pstr_ps_stereo_dec, it_bit_buff,
                self->pstr_sbr_tables, audio_object_type, ec_flag);
            if (usac_flag && (frame_status == 0)) return -1;
            if (frame_status < 0) return frame_status;
            if (ec_flag && self->pstr_ps_stereo_dec != NULL) {
              ixheaacd_copy_prev_ps_params(self->pstr_ps_stereo_dec, &self->str_ps_config_prev,
                                           frame_status);
            }
          } else if (ptr_frame_data[k]->sbr_mode == PVC_SBR) {
            frame_status = ixheaacd_sbr_read_pvc_sce(
                ptr_frame_data[k], it_bit_buff, ptr_header_data[k]->hbe_flag,
                self->ptr_pvc_data_str, self->pstr_sbr_tables, ptr_header_data[k]);
            if (frame_status < 0) return frame_status;
          }
        }
        ptr_header_data[k]->enh_sbr_ps =
            ((self->enh_sbr_ps) | (ptr_header_data[0]->channel_mode == PS_STEREO));
        if ((audio_object_type != AOT_ER_AAC_ELD) && (audio_object_type != AOT_USAC)) {
          WORD32 total_bits_read;
          total_bits_read = ixheaacd_no_bits_read(it_bit_buff);
          if (total_bits_read > (ptr_bit_str_ele->size_payload << 3) ||
              total_bits_read < ((ptr_bit_str_ele->size_payload << 3) - 8)) {
            frame_status = 0;
          }
          if (ec_flag) {
            if (!frame_status) {
              ixheaacd_ec_set_frame_error_flag(ptr_bit_str_ele, FRAME_ERROR);
            } else {
              ixheaacd_ec_set_frame_error_flag(ptr_bit_str_ele, FRAME_OK);
            }
          }
        }
      }
      if ((ldmps_present == 1) && (it_bit_buff)) {
        WORD32 bits_decoded = (it_bit_buff->size - it_bit_buff->cnt_bits);
        self->ptr_mps_data = (WORD8 *)it_bit_buff->ptr_read_next;
        self->left_mps_bits = (total_bits_left - bits_decoded);
        self->mps_bits_pos = it_bit_buff->bit_pos;
      }
      if (ec_flag) {
        if (frame_status && !init_flag) {
          ptr_bit_str_ele->use_frame_slot =
              (ptr_bit_str_ele->use_frame_slot + 1) % (self->num_delay_frames + 1);
        }
        if (ptr_header_data[0]->sync_state == SBR_ACTIVE) {
          ptr_header_data[k]->err_flag =
              ptr_bit_str_ele->frame_error_flag[ptr_bit_str_ele->use_frame_slot];
        }
      }
    }

    if (!usac_flag) {
      if (!frame_status || (ptr_header_data[0]->sync_state != SBR_ACTIVE) ||
          ptr_header_data[0]->err_flag) {
        ptr_header_data[0]->err_flag = 1;
        stereo = (num_channels == 2) ? 1 : 0;
        if (ptr_header_data[0]->channel_mode == 0) {
          ptr_header_data[0]->channel_mode = stereo ? SBR_STEREO : SBR_MONO;
        }
      }

      if (!(stereo || dual_mono)) {
        ptr_frame_data[0]->coupling_mode = COUPLING_OFF;
        ptr_frame_data[1]->coupling_mode = COUPLING_OFF;
      }

      if (ptr_header_data[0]->sync_state == SBR_NOT_INITIALIZED && (!ec_flag || init_flag)) {
        WORD32 lr1 = ps_enable ? 2 : num_channels;
        ixheaacd_prepare_upsamp(ptr_header_data, pstr_sbr_channel, lr1);
      }
    }

    if (ptr_header_data[0]->sync_state == SBR_ACTIVE) {
      if (ptr_frame_data[0]->sbr_mode == PVC_SBR) {
        err = ixheaacd_dec_sbrdata_for_pvc(ptr_header_data[0], ptr_frame_data[0],
                                           pstr_sbr_channel[0]->pstr_prev_frame_data,
                                           audio_object_type);
        if (err) return err;
      } else if (ptr_frame_data[0]->sbr_mode == ORIG_SBR) {
        err = ixheaacd_dec_sbrdata(
            ptr_header_data[0], ptr_header_data[1], ptr_frame_data[0],
            pstr_sbr_channel[0]->pstr_prev_frame_data,
            (stereo || dual_mono) ? ptr_frame_data[1] : NULL,
            (stereo || dual_mono) ? pstr_sbr_channel[1]->pstr_prev_frame_data : NULL,
            self->pstr_common_tables, ldmps_present, audio_object_type, ec_flag);

        if (err) return err;
      }

      if (ptr_header_data[0]->channel_mode == PS_STEREO &&
          (audio_object_type != AOT_ER_AAC_ELD && audio_object_type != AOT_ER_AAC_LD)) {
        ixheaacd_decode_ps_data(self->pstr_ps_stereo_dec, frame_size);
        ps_flag = 1;
        self->ps_present = ps_flag;
      }
      if (ptr_header_data[0]->enh_sbr_ps && self->enh_sbr) {
        ps_flag = 1;
        self->ps_present = ps_flag;
      }
      ptr_frame_data[0]->max_qmf_subband_aac =
          ptr_header_data[0]->pstr_freq_band_data->sub_band_start;
      if (stereo) {
        ptr_frame_data[1]->max_qmf_subband_aac =
            ptr_header_data[1]->pstr_freq_band_data->sub_band_start;
      }
      if (ldmps_present == 1) {
        ptr_frame_data[0]->rate = 1;
        if (stereo) {
          ptr_frame_data[1]->rate = 1;
        }
      }
    }
    if (audio_object_type == AOT_ER_AAC_ELD && ec_flag) {
      if ((initial_sync_state == SBR_NOT_INITIALIZED) && ptr_header_data[0]->err_flag) {
        ptr_header_data[0]->sync_state = SBR_NOT_INITIALIZED;
      }
    } else {
      if (audio_object_type != AOT_ER_AAC_ELD) {
        if ((initial_sync_state == SBR_NOT_INITIALIZED) && ptr_header_data[0]->err_flag) {
          ptr_header_data[0]->sync_state = SBR_NOT_INITIALIZED;
        }
      } else {
        ptr_header_data[0]->sync_state = SBR_ACTIVE;
      }
    }

    if ((num_channels == 2) && !(stereo || dual_mono)) {
      ixheaacd_downmix_to_monosbr(&core_sample_buf[slot_element], ch_fac);
    }

    if ((!prev_stereo && !prev_ps_flag) && (ps_flag)) {
      WORD32 copy_size;
      if (down_samp_flag)
        copy_size = QMF_FILTER_STATE_SYN_SIZE_DOWN_SAMPLED;
      else
        copy_size = QMF_FILTER_STATE_SYN_SIZE;

      memcpy(pstr_sbr_channel[1]->str_sbr_dec.str_synthesis_qmf_bank.filter_states,
             pstr_sbr_channel[0]->str_sbr_dec.str_synthesis_qmf_bank.filter_states,
             copy_size * sizeof(WORD16));

      pstr_sbr_channel[1]->str_sbr_dec.str_sbr_scale_fact.st_syn_scale =
          pstr_sbr_channel[0]->str_sbr_dec.str_sbr_scale_fact.st_syn_scale;
    }

    if ((!prev_stereo && stereo && (num_channels == 2)) &&
        (audio_object_type != AOT_ER_AAC_ELD)) {
      WORD32 copy_size;
      if (down_samp_flag)
        copy_size = QMF_FILTER_STATE_SYN_SIZE_DOWN_SAMPLED;
      else
        copy_size = QMF_FILTER_STATE_SYN_SIZE;

      memcpy(pstr_sbr_channel[1]->str_sbr_dec.str_synthesis_qmf_bank.filter_states,
             pstr_sbr_channel[0]->str_sbr_dec.str_synthesis_qmf_bank.filter_states,
             copy_size * sizeof(WORD16));

      pstr_sbr_channel[1]->str_sbr_dec.str_sbr_scale_fact.st_syn_scale =
          pstr_sbr_channel[0]->str_sbr_dec.str_sbr_scale_fact.st_syn_scale;

      memcpy(pstr_sbr_channel[1]->str_sbr_dec.str_codec_qmf_bank.anal_filter_states,
             pstr_sbr_channel[0]->str_sbr_dec.str_codec_qmf_bank.anal_filter_states,
             QMF_FILTER_STATE_ANA_SIZE * sizeof(WORD16));

      pstr_sbr_channel[1]->str_sbr_dec.str_sbr_scale_fact.st_lb_scale =
          pstr_sbr_channel[0]->str_sbr_dec.str_sbr_scale_fact.st_lb_scale;

      memcpy(pstr_sbr_channel[1]->str_sbr_dec.ptr_sbr_overlap_buf,
             pstr_sbr_channel[0]->str_sbr_dec.ptr_sbr_overlap_buf,
             MAX_OV_COLS * NO_SYNTHESIS_CHANNELS * sizeof(WORD32));

      pstr_sbr_channel[1]->str_sbr_dec.str_sbr_scale_fact.ov_lb_scale =
          pstr_sbr_channel[0]->str_sbr_dec.str_sbr_scale_fact.ov_lb_scale;
      pstr_sbr_channel[1]->str_sbr_dec.str_sbr_scale_fact.ov_hb_scale =
          pstr_sbr_channel[0]->str_sbr_dec.str_sbr_scale_fact.ov_hb_scale;
    }
    pstr_sbr_channel[0]->str_sbr_dec.time_sample_buf = self->time_sample_buf[0];
    if (self->pstr_ps_stereo_dec != NULL &&
        (ps_enable || self->enh_sbr_ps) && self->enh_sbr) {
      self->pstr_ps_stereo_dec->pp_qmf_buf_real[0] =
          pstr_sbr_channel[0]->str_sbr_dec.pp_qmf_buf_real;
      self->pstr_ps_stereo_dec->pp_qmf_buf_imag[0] =
          pstr_sbr_channel[0]->str_sbr_dec.pp_qmf_buf_imag;
      self->pstr_ps_stereo_dec->pp_qmf_buf_real[1] =
          pstr_sbr_channel[1]->str_sbr_dec.pp_qmf_buf_real;
      self->pstr_ps_stereo_dec->pp_qmf_buf_imag[1] =
          pstr_sbr_channel[1]->str_sbr_dec.pp_qmf_buf_imag;
      self->pstr_ps_stereo_dec->time_sample_buf[0] = self->time_sample_buf[0];
      self->pstr_ps_stereo_dec->time_sample_buf[1] = self->time_sample_buf[1];
    }
  } else {
    for (k = 0; k < 2; k++) {
      ptr_frame_data[k] = (ia_sbr_frame_info_data_struct *)self->frame_buffer[k];

      pstr_sbr_channel[k] = self->pstr_sbr_channel[k];

      ptr_header_data[k] = self->pstr_sbr_header[k];
      ptr_header_data[k]->usac_flag = self->aot_usac_flag;
    }

    if ((p_sbr_bit_stream->no_elements) && (p_sbr_bit_stream->str_sbr_ele->size_payload > 0)) {
      num_elements = p_sbr_bit_stream->no_elements;
    } else {
      num_elements = 0;
    }

    for (k = 0; k < num_elements; k++) {
      ia_sbr_element_stream_struct *ptr_bit_str_ele = &p_sbr_bit_stream->str_sbr_ele[k];
      ele_channels = (p_sbr_bit_stream->str_sbr_ele[0].sbr_ele_id == SBR_ID_CPE) ? 2 : 1;
      {
        switch (ptr_bit_str_ele->sbr_ele_id) {
          case SBR_ID_SCE:
          case SBR_ID_CCE:
            if (num_channels == 2) {
              dual_mono = 1;
            }
            stereo = 0;
            break;
          case SBR_ID_CPE:
            stereo = 1;
            ptr_header_data[1] = ptr_header_data[0];

            memcpy(self->pstr_sbr_header[1], self->pstr_sbr_header[0],
                   sizeof(ia_sbr_header_data_struct));
            break;
          default:
            frame_status = 0;
        }
      }
    }
  }
  if (ec_flag) {
    for (k = 0; k < 2; k++) {
      if (pstr_sbr_channel[k]->str_sbr_dec.band_count == 0) {
        pstr_sbr_channel[k]->str_sbr_dec.band_count =
            pstr_sbr_channel[k]->str_sbr_dec.str_codec_qmf_bank.no_channels;
      }
    }
  }
  if (ec_flag && usac_flag && !first_frame && !self->sbr_parse_complete) {
    return IA_FATAL_ERROR;
  }
  if (!ec_flag || !first_frame || init_flag) {
    if (pstr_drc_dec == NULL) {
      WORD32 err_code = 0;
      err_code = ixheaacd_sbr_dec(
          &pstr_sbr_channel[0]->str_sbr_dec, core_sample_buf + slot_element, ptr_header_data[0],
          ptr_frame_data[0], pstr_sbr_channel[0]->pstr_prev_frame_data, self->pstr_ps_stereo_dec,
          &pstr_sbr_channel[1]->str_sbr_dec.str_synthesis_qmf_bank,
          &pstr_sbr_channel[1]->str_sbr_dec.str_sbr_scale_fact,
          (ptr_header_data[0]->sync_state == SBR_ACTIVE), low_pow_flag,
          sbr_scratch_struct->ptr_work_buf_core, self->pstr_sbr_tables, self->pstr_common_tables,
          ch_fac, self->ptr_pvc_data_str, 0, NULL, audio_object_type, ldmps_present, self,
          heaac_mps_present, ec_flag);
      if (err_code) return err_code;
      if (self->enh_sbr)
      {
        if (!self->enh_sbr_ps) {
          if ((ptr_header_data[0]->sync_state == SBR_ACTIVE) && !ptr_frame_data[0]->mps_sbr_flag
              && ch_fac != 2) {
            ptr_header_data[0]->pstr_freq_band_data[0].qmf_sb_prev =
                ptr_header_data[0]->pstr_freq_band_data->sub_band_start;
          }
        } else {
          if (ptr_header_data[0]->sync_state == SBR_ACTIVE) {
            ptr_header_data[0]->pstr_freq_band_data[0].qmf_sb_prev =
                ptr_header_data[0]->pstr_freq_band_data->sub_band_start;
          }
        }
      }
    } else {
      WORD32 err_code = 0;
      err_code = ixheaacd_sbr_dec(
          &pstr_sbr_channel[0]->str_sbr_dec, core_sample_buf + slot_element, ptr_header_data[0],
          ptr_frame_data[0], pstr_sbr_channel[0]->pstr_prev_frame_data, self->pstr_ps_stereo_dec,
          &pstr_sbr_channel[1]->str_sbr_dec.str_synthesis_qmf_bank,
          &pstr_sbr_channel[1]->str_sbr_dec.str_sbr_scale_fact,
          (ptr_header_data[0]->sync_state == SBR_ACTIVE), low_pow_flag,
          sbr_scratch_struct->ptr_work_buf_core, self->pstr_sbr_tables, self->pstr_common_tables,
          ch_fac, self->ptr_pvc_data_str, pstr_drc_dec->drc_on,
          pstr_drc_dec->str_drc_channel_data[0].drc_factors_sbr, audio_object_type, ldmps_present,
          self, heaac_mps_present, ec_flag);
      if (err_code) return err_code;
      if (self->enh_sbr)
      {
        if (!self->enh_sbr_ps) {
          if ((ptr_header_data[0]->sync_state == SBR_ACTIVE) && !ptr_frame_data[0]->mps_sbr_flag
              && num_channels != 2) {
            ptr_header_data[0]->pstr_freq_band_data[0].qmf_sb_prev =
                ptr_header_data[0]->pstr_freq_band_data->sub_band_start;
          }
        } else {
          if (ptr_header_data[0]->sync_state == SBR_ACTIVE) {
            ptr_header_data[0]->pstr_freq_band_data[0].qmf_sb_prev =
                ptr_header_data[0]->pstr_freq_band_data->sub_band_start;
          }
        }
      }
    }

    if (!down_mix_flag && (stereo || dual_mono) && (num_channels == 2)) {
      pstr_sbr_channel[1]->str_sbr_dec.time_sample_buf = self->time_sample_buf[1];

      if (ele_channels == 1 && usac_flag) {
        WORD32 err_code =
            ixheaacd_esbr_dec(&pstr_sbr_channel[1]->str_sbr_dec, ptr_header_data[1],
                              ptr_frame_data[1], (ptr_header_data[1]->sync_state == SBR_ACTIVE),
                              low_pow_flag, self->pstr_sbr_tables, ch_fac);
        if (err_code) return err_code;
      } else {
        if (pstr_drc_dec == NULL) {
          WORD32 err_code = ixheaacd_sbr_dec(
              &pstr_sbr_channel[1]->str_sbr_dec, core_sample_buf + slot_element + 1,
              ptr_header_data[1], ptr_frame_data[1], pstr_sbr_channel[1]->pstr_prev_frame_data,
              NULL, NULL, NULL, (ptr_header_data[1]->sync_state == SBR_ACTIVE), low_pow_flag,
              sbr_scratch_struct->ptr_work_buf_core, self->pstr_sbr_tables,
              self->pstr_common_tables, ch_fac, self->ptr_pvc_data_str, 0, NULL,
              audio_object_type, ldmps_present, self, heaac_mps_present, ec_flag);
          if (err_code) return err_code;
          if (self->enh_sbr)
          {
            if (!self->enh_sbr_ps) {
              if ((ptr_header_data[1]->sync_state == SBR_ACTIVE) &&
                  !ptr_frame_data[0]->mps_sbr_flag) {
                ptr_header_data[1]->pstr_freq_band_data[0].qmf_sb_prev =
                    ptr_header_data[1]->pstr_freq_band_data->sub_band_start;
              }
            } else {
              if (ptr_header_data[1]->sync_state == SBR_ACTIVE) {
                ptr_header_data[1]->pstr_freq_band_data[0].qmf_sb_prev =
                    ptr_header_data[1]->pstr_freq_band_data->sub_band_start;
              }
            }
          }
        } else {
          WORD32 err_code = ixheaacd_sbr_dec(
              &pstr_sbr_channel[1]->str_sbr_dec, core_sample_buf + slot_element + 1,
              ptr_header_data[1], ptr_frame_data[1], pstr_sbr_channel[1]->pstr_prev_frame_data,
              NULL, NULL, NULL, (ptr_header_data[1]->sync_state == SBR_ACTIVE), low_pow_flag,
              sbr_scratch_struct->ptr_work_buf_core, self->pstr_sbr_tables,
              self->pstr_common_tables, ch_fac, self->ptr_pvc_data_str, pstr_drc_dec->drc_on,
              pstr_drc_dec->str_drc_channel_data[1].drc_factors_sbr, audio_object_type,
              ldmps_present, self, heaac_mps_present, ec_flag);
          if (err_code) return err_code;
          if (self->enh_sbr)
          {
            if (!self->enh_sbr_ps) {
              if ((ptr_header_data[1]->sync_state == SBR_ACTIVE) &&
                  !ptr_frame_data[0]->mps_sbr_flag) {
                ptr_header_data[1]->pstr_freq_band_data[0].qmf_sb_prev =
                    ptr_header_data[1]->pstr_freq_band_data->sub_band_start;
              }
            } else {
              if (ptr_header_data[1]->sync_state == SBR_ACTIVE) {
                ptr_header_data[1]->pstr_freq_band_data[0].qmf_sb_prev =
                    ptr_header_data[1]->pstr_freq_band_data->sub_band_start;
              }
            }
          }
        }
      }

    } else {
      if (audio_object_type != AOT_ER_AAC_ELD && audio_object_type != AOT_ER_AAC_LD)

      {
        if (sub_d((WORD16)ptr_header_data[0]->channel_mode, PS_STEREO) == 0) {
          num_channels = 2;
        }
        if (ptr_header_data[0]->enh_sbr_ps && self->enh_sbr) {
          num_channels = 2;
        }
      }
    }
    *codec_num_channels = num_channels;
    self->sbr_mode = ptr_frame_data[0]->sbr_mode;

    if ((audio_object_type == AOT_ER_AAC_ELD) || (audio_object_type == AOT_ER_AAC_LD) ||
        !self->enh_sbr) {
      if (pstr_drc_dec != NULL) {
        WORD32 i, j;
        for (i = 0; i < *codec_num_channels; i++) {
          for (j = 0; j < 32; j++) {
            memcpy(pstr_drc_dec->str_drc_channel_data[i].drc_factors_sbr[j],
                   pstr_drc_dec->str_drc_channel_data[i].drc_factors_sbr[j + 32],
                   64 * sizeof(WORD32));
          }
        }
      }
    }
    if (ec_flag) {
      self->band_count[0] = pstr_sbr_channel[0]->str_sbr_dec.band_count;
      self->band_count[1] = pstr_sbr_channel[1]->str_sbr_dec.band_count;
    }
  }

  return SBRDEC_OK;
}

IA_ERRORCODE ixheaacd_parse_sbr(ia_handle_sbr_dec_inst_struct self,
                                ia_aac_dec_sbr_bitstream_struct *p_sbr_bit_stream,
                                WORD16 *codec_num_channels, FLAG frame_status,
                                ia_sbr_scr_struct *sbr_scratch_struct,
                                ia_bit_buf_struct *it_bit_buff, WORD32 audio_object_type) {
  WORD32 k;
  FLAG ps_flag = 0;
  FLAG stereo = 0;
  FLAG low_pow_flag = 0;
  FLAG header_flag = 1;
  FLAG dual_mono = 0;
  WORD32 err = 0;
  WORD32 num_channels = *codec_num_channels;
  FLAG prev_stereo;
  WORD32 num_elements = p_sbr_bit_stream->no_elements;
  WORD32 usac_flag = self->aot_usac_flag;

  ia_sbr_channel_struct *pstr_sbr_channel[2];
  ia_sbr_header_data_struct *ptr_header_data[MAXNRSBRCHANNELS];

  WORD32 initial_sync_state;

  ia_sbr_header_data_struct *ptr_sbr_dflt_header =
      (ia_sbr_header_data_struct *)(&self->str_sbr_dflt_header);

  ia_sbr_frame_info_data_struct *ptr_frame_data[2];

  self->num_delay_frames = 1;

  self->ptr_mps_data = NULL;

  if (usac_flag && self->ec_flag) {
    for (k = 0; k < 2; k++) {
      ptr_frame_data[k] = (ia_sbr_frame_info_data_struct *)self->frame_buffer[k];

      pstr_sbr_channel[k] = self->pstr_sbr_channel[k];

      ptr_header_data[k] = self->pstr_sbr_header[k];

      ptr_frame_data[k]->usac_independency_flag = self->usac_independency_flag;
      ptr_frame_data[k]->mps_sbr_flag = (self->stereo_config_idx == 3) ? 1 : 0;
      ptr_frame_data[k]->stereo_config_idx = self->stereo_config_idx;
      ptr_frame_data[k]->inter_tes_flag = self->inter_tes_flag;
      ptr_frame_data[k]->sbr_mode = self->sbr_mode;
    }

    for (k = 0; k < *codec_num_channels; k++) {
      ptr_header_data[k]->usac_flag = self->aot_usac_flag;

      ptr_header_data[k]->enh_sbr = self->enh_sbr;
      ptr_header_data[k]->enh_sbr_ps =
          ((self->enh_sbr_ps) | (ptr_header_data[k]->channel_mode == PS_STEREO));
      ptr_header_data[k]->usac_independency_flag = self->usac_independency_flag;
      ptr_header_data[k]->hbe_flag = self->hbe_flag;
      ptr_header_data[k]->pvc_flag = self->pvc_flag;

      ptr_header_data[k]->esbr_hq = self->esbr_hq;
    }

    initial_sync_state = ptr_header_data[0]->sync_state;

    low_pow_flag = 0;
    self->pstr_sbr_tables->sbr_rand_ph = self->pstr_sbr_tables->env_calc_tables_ptr->sbr_rand_ph;

    prev_stereo = (ptr_header_data[0]->channel_mode == SBR_STEREO);

    ptr_header_data[0]->err_flag_prev = ptr_header_data[0]->err_flag;

    if (p_sbr_bit_stream->no_elements == 0) {
      frame_status = 0;
      ptr_header_data[0]->sync_state = UPSAMPLING;
      if (num_channels == 2) ptr_header_data[1]->sync_state = UPSAMPLING;
    }

    if ((p_sbr_bit_stream->no_elements) && (p_sbr_bit_stream->str_sbr_ele->size_payload > 0)) {
      num_elements = p_sbr_bit_stream->no_elements;
    } else {
      num_elements = 0;
    }

    for (k = 0; k < num_elements; k++) {
      ia_sbr_element_stream_struct *ptr_bit_str_ele = &p_sbr_bit_stream->str_sbr_ele[k];

      switch (ptr_bit_str_ele->sbr_ele_id) {
        case SBR_ID_SCE:
        case SBR_ID_CCE:
          if (num_channels == 2) {
            dual_mono = 1;
          }
          stereo = 0;
          break;
        case SBR_ID_CPE:
          stereo = 1;
          ptr_header_data[1] = ptr_header_data[0];

          memcpy(self->pstr_sbr_header[1], self->pstr_sbr_header[0],
                 sizeof(*(self->pstr_sbr_header[1])));
          break;
        default:
          frame_status = 0;
      }

      if (frame_status) {
        if (ptr_bit_str_ele->extension_type == SBR_EXTENSION_CRC) {
          WORD32 crc_bits = 0;
          WORD32 crc_check_flag = 0;
          crc_check_flag = 1;
          crc_bits = (((ptr_bit_str_ele->size_payload - 1) << 3) + (4 - SBR_CYC_REDCY_CHK_BITS));

          if (crc_bits < 0) {
            crc_check_flag = 0;
            frame_status = 0;
          }
          if (crc_check_flag && frame_status == 1)
            frame_status = ixheaacd_sbr_crccheck(it_bit_buff, crc_bits);
        }

        if (frame_status == 1) {
          header_flag = ixheaacd_sbr_read_header_data(ptr_header_data[k], it_bit_buff, stereo,
                                                      ptr_sbr_dflt_header);
        }
        if (usac_flag) {
          if ((self->ptr_pvc_data_str->prev_pvc_mode == 0) &&
              (ptr_header_data[k]->pvc_mode != 0)) {
            self->ptr_pvc_data_str->prev_pvc_id = 0;
          }
          self->ptr_pvc_data_str->prev_pvc_mode = ptr_header_data[k]->pvc_mode;
          if (ptr_header_data[k]->pvc_mode == 0) {
            ptr_frame_data[k]->sbr_mode = ORIG_SBR;
          } else {
            ptr_frame_data[k]->sbr_mode = PVC_SBR;
          }
        }
        if (header_flag == SBR_RESET) {
          err = ixheaacd_calc_frq_bnd_tbls(ptr_header_data[k],

                                           self->pstr_common_tables);
          if (!err) {
            WORD32 lr;
            WORD32 lr1 = num_channels;
            for (lr = 0; lr < lr1; lr++) {
              ptr_frame_data[lr]->reset_flag = 1;
              if ((SBR_NOT_INITIALIZED == ptr_header_data[lr]->sync_state) && !usac_flag) {
                ptr_frame_data[lr]->sbr_patching_mode = 1;
                ptr_frame_data[lr]->over_sampling_flag = 0;
                ptr_frame_data[lr]->pitch_in_bins = 0;
                ptr_header_data[lr]->pre_proc_flag = 0;
              }

              err |= ixheaacd_sbr_dec_reset(
                  &(pstr_sbr_channel[lr]->str_sbr_dec), ptr_header_data[k], low_pow_flag,
                  self->pstr_common_tables, ptr_frame_data[k]->pitch_in_bins, audio_object_type,
                  sbr_scratch_struct->ptr_work_buf_core);
              if (err < 0) {
                if (self->ec_flag) {
                  self->frame_ok = 0;
                } else {
                  return err;
                }
              }
            }
          }

          if (err == 0) {
            ptr_header_data[k]->sync_state = SBR_ACTIVE;
          }
        }
      }

      if (err || (ptr_header_data[k]->sync_state == SBR_NOT_INITIALIZED)) {
        WORD32 lr1 = num_channels;
        ixheaacd_prepare_upsamp(ptr_header_data, pstr_sbr_channel, lr1);
        if (err && !self->ec_flag) {
          return err;
        }
      }

      if (frame_status && (ptr_header_data[k]->sync_state == SBR_ACTIVE)) {
        if (stereo) {
          frame_status = ixheaacd_sbr_read_cpe(ptr_header_data[0], ptr_frame_data, it_bit_buff,
                                               self->pstr_sbr_tables, audio_object_type);
          if (frame_status < 0) return frame_status;
        } else {
          self->pstr_ps_stereo_dec = 0;

          if (ptr_frame_data[k]->sbr_mode == ORIG_SBR) {
            frame_status = ixheaacd_sbr_read_sce(ptr_header_data[k], ptr_frame_data[k],
                                                 self->pstr_ps_stereo_dec, it_bit_buff,
                                                 self->pstr_sbr_tables, audio_object_type, 0);
            if (frame_status < 0) return frame_status;
          } else if (ptr_frame_data[k]->sbr_mode == PVC_SBR) {
            frame_status = ixheaacd_sbr_read_pvc_sce(ptr_frame_data[k], it_bit_buff, 0,
                                                     self->ptr_pvc_data_str,
                                                     self->pstr_sbr_tables, ptr_header_data[k]);
            if (frame_status < 0) return frame_status;
          }
        }
        ptr_header_data[k]->enh_sbr_ps =
            ((self->enh_sbr_ps) | (ptr_header_data[0]->channel_mode == PS_STEREO));
        {
          WORD32 total_bits_read;
          total_bits_read = ixheaacd_no_bits_read(it_bit_buff);
          if (total_bits_read > (ptr_bit_str_ele->size_payload << 3) ||
              total_bits_read < ((ptr_bit_str_ele->size_payload << 3) - 8)) {
            frame_status = 0;
          }
        }
      } else {
        if (frame_status && self->ec_flag) {
          err = IA_XHEAAC_DEC_EXE_NONFATAL_SBR_PARSE_ERROR;
          self->sbr_parse_err_flag = 1;
        }
        if (!frame_status) {
          ixheaacd_ec_set_frame_error_flag(ptr_bit_str_ele, FRAME_ERROR);
        } else {
          ixheaacd_ec_set_frame_error_flag(ptr_bit_str_ele, FRAME_OK);
        }
      }
    }
    if (ptr_header_data[0]->sync_state == SBR_ACTIVE) {
      if (ptr_frame_data[0]->sbr_mode == PVC_SBR) {
        err = ixheaacd_dec_sbrdata_for_pvc(ptr_header_data[0], ptr_frame_data[0],
                                           pstr_sbr_channel[0]->pstr_prev_frame_data,
                                           audio_object_type);
        if (err) {
          if (self->ec_flag) {
            self->frame_ok = 0;
          } else {
            return err;
          }
        }
      } else if (ptr_frame_data[0]->sbr_mode == ORIG_SBR) {
        err = ixheaacd_dec_sbrdata(
            ptr_header_data[0], ptr_header_data[1], ptr_frame_data[0],
            pstr_sbr_channel[0]->pstr_prev_frame_data,
            (stereo || dual_mono) ? ptr_frame_data[1] : NULL,
            (stereo || dual_mono) ? pstr_sbr_channel[1]->pstr_prev_frame_data : NULL,
            self->pstr_common_tables, 0, audio_object_type, self->ec_flag);

        if (err) {
          if (self->ec_flag) {
            self->frame_ok = 0;
          } else {
            return err;
          }
        }
      }

      if (ptr_header_data[0]->channel_mode == PS_STEREO &&
          (audio_object_type != AOT_ER_AAC_ELD && audio_object_type != AOT_ER_AAC_LD)) {
        ixheaacd_decode_ps_data(self->pstr_ps_stereo_dec, 1024);
        ps_flag = 1;
        self->ps_present = ps_flag;
      }

      if (ptr_header_data[0]->enh_sbr_ps) {
        ps_flag = 1;
        self->ps_present = ps_flag;
      }

      ptr_frame_data[0]->max_qmf_subband_aac =
          ptr_header_data[0]->pstr_freq_band_data->sub_band_start;
      if (stereo) {
        ptr_frame_data[1]->max_qmf_subband_aac =
            ptr_header_data[1]->pstr_freq_band_data->sub_band_start;
      }
    }
    if (audio_object_type != AOT_ER_AAC_ELD) {
      if ((initial_sync_state == SBR_NOT_INITIALIZED) && ptr_header_data[0]->err_flag) {
        ptr_header_data[0]->sync_state = SBR_NOT_INITIALIZED;
      }
    } else {
      ptr_header_data[0]->sync_state = SBR_ACTIVE;
    }

    if ((!prev_stereo && stereo && (num_channels == 2)) &&
        (audio_object_type != AOT_ER_AAC_ELD)) {
      memcpy(pstr_sbr_channel[1]->str_sbr_dec.str_synthesis_qmf_bank.filter_states,
             pstr_sbr_channel[0]->str_sbr_dec.str_synthesis_qmf_bank.filter_states,
             QMF_FILTER_STATE_SYN_SIZE * sizeof(WORD16));

      pstr_sbr_channel[1]->str_sbr_dec.str_sbr_scale_fact.st_syn_scale =
          pstr_sbr_channel[0]->str_sbr_dec.str_sbr_scale_fact.st_syn_scale;

      memcpy(pstr_sbr_channel[1]->str_sbr_dec.str_codec_qmf_bank.anal_filter_states,
             pstr_sbr_channel[0]->str_sbr_dec.str_codec_qmf_bank.anal_filter_states,
             QMF_FILTER_STATE_ANA_SIZE * sizeof(WORD16));

      pstr_sbr_channel[1]->str_sbr_dec.str_sbr_scale_fact.st_lb_scale =
          pstr_sbr_channel[0]->str_sbr_dec.str_sbr_scale_fact.st_lb_scale;

      memcpy(pstr_sbr_channel[1]->str_sbr_dec.ptr_sbr_overlap_buf,
             pstr_sbr_channel[0]->str_sbr_dec.ptr_sbr_overlap_buf,
             MAX_OV_COLS * NO_SYNTHESIS_CHANNELS * sizeof(WORD32));

      pstr_sbr_channel[1]->str_sbr_dec.str_sbr_scale_fact.ov_lb_scale =
          pstr_sbr_channel[0]->str_sbr_dec.str_sbr_scale_fact.ov_lb_scale;
      pstr_sbr_channel[1]->str_sbr_dec.str_sbr_scale_fact.ov_hb_scale =
          pstr_sbr_channel[0]->str_sbr_dec.str_sbr_scale_fact.ov_hb_scale;
    }

    pstr_sbr_channel[0]->str_sbr_dec.time_sample_buf = self->time_sample_buf[0];
    self->sbr_parse_complete = 1;
  }

  return err;
}