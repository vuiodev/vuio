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

#include "ixheaacd_sbrqmftrans.h"

#include "ixheaacd_audioobjtypes.h"

extern const WORD32 ixheaacd_ldmps_polyphase_filter_coeff_fix[1280];

#define ALIGN_SIZE64(x) ((((x) + 7) >> 3) << 3)

#define FD_OVERSAMPLING_FAC (1.5f)

WORD32 ixheaacd_getsize_sbr_persistent() {
  return (IXHEAAC_GET_SIZE_ALIGNED(sizeof(ia_sbr_pers_struct), BYTE_ALIGN_8));
}

VOID ixheaacd_esbr_hbe_data_init(
    ia_esbr_hbe_txposer_struct *pstr_esbr_hbe_txposer,
    const WORD32 num_aac_samples, WORD32 samp_fac_4_flag,
    const WORD32 num_out_samples, VOID *persistent_hbe_mem,
    WORD32 *total_persistant) {
  WORD32 i;
  WORD32 used_persistent = 0;

  if (pstr_esbr_hbe_txposer) {
    memset(pstr_esbr_hbe_txposer, 0,
           IXHEAAC_GET_SIZE_ALIGNED(sizeof(ia_esbr_hbe_txposer_struct), BYTE_ALIGN_8));

    pstr_esbr_hbe_txposer->core_frame_length = num_aac_samples;

    pstr_esbr_hbe_txposer->no_bins = num_out_samples / NO_QMF_SYNTH_CHANNELS;
    pstr_esbr_hbe_txposer->hbe_qmf_in_len =
        pstr_esbr_hbe_txposer->no_bins;
    pstr_esbr_hbe_txposer->hbe_qmf_out_len =
        2 * pstr_esbr_hbe_txposer->hbe_qmf_in_len;

    pstr_esbr_hbe_txposer->ptr_input_buf =
        (FLOAT32 *)((WORD8 *)persistent_hbe_mem);
    used_persistent +=
        (num_aac_samples + NO_QMF_SYNTH_CHANNELS) * sizeof(FLOAT32);

    pstr_esbr_hbe_txposer->qmf_in_buf =
        (FLOAT32 **)((WORD8 *)persistent_hbe_mem + used_persistent);
    used_persistent +=
        pstr_esbr_hbe_txposer->hbe_qmf_in_len * sizeof(FLOAT32 *);

    for (i = 0; i < pstr_esbr_hbe_txposer->hbe_qmf_in_len; i++) {
      pstr_esbr_hbe_txposer->qmf_in_buf[i] =
          (FLOAT32 *)((WORD8 *)persistent_hbe_mem + used_persistent);

      used_persistent += (TWICE_QMF_SYNTH_CHANNELS_NUM * sizeof(FLOAT32));
    }

    pstr_esbr_hbe_txposer->qmf_out_buf =
        (FLOAT32 **)((WORD8 *)persistent_hbe_mem + used_persistent);
    used_persistent +=
        (pstr_esbr_hbe_txposer->hbe_qmf_out_len * sizeof(FLOAT32 *));

    for (i = 0; i < pstr_esbr_hbe_txposer->hbe_qmf_out_len; i++) {
      pstr_esbr_hbe_txposer->qmf_out_buf[i] =
          (FLOAT32 *)((WORD8 *)persistent_hbe_mem + used_persistent);
      used_persistent += (TWICE_QMF_SYNTH_CHANNELS_NUM * sizeof(FLOAT32));
    }
    pstr_esbr_hbe_txposer->upsamp_4_flag = samp_fac_4_flag;
    if (pstr_esbr_hbe_txposer) {
      pstr_esbr_hbe_txposer->fft_size[0] = num_aac_samples;
      pstr_esbr_hbe_txposer->fft_size[1] = (WORD32)(FD_OVERSAMPLING_FAC * num_aac_samples);

      pstr_esbr_hbe_txposer->ptr_spectrum = &pstr_esbr_hbe_txposer->spectrum_buf[0];
      pstr_esbr_hbe_txposer->ptr_spectrum_tx =
          &pstr_esbr_hbe_txposer->spectrum_transposed_buf[0];
      pstr_esbr_hbe_txposer->mag = &pstr_esbr_hbe_txposer->mag_buf[0];
      pstr_esbr_hbe_txposer->phase = &pstr_esbr_hbe_txposer->phase_buf[0];
      pstr_esbr_hbe_txposer->ptr_output_buf = &pstr_esbr_hbe_txposer->output_buf[0];
    }
  }
  *total_persistant = used_persistent;
  return;
}

VOID ixheaacd_set_sbr_persistent_table_pointer(
    VOID *sbr_persistent_mem_v, ia_sbr_tables_struct *sbr_tables_ptr,
    ixheaacd_misc_tables *pstr_common_tables) {
  ia_sbr_pers_struct *sbr_persistent_mem =
      (ia_sbr_pers_struct *)sbr_persistent_mem_v;
  sbr_persistent_mem->str_sbr_dec_inst.pstr_sbr_tables = sbr_tables_ptr;
  sbr_persistent_mem->str_sbr_dec_inst.pstr_common_tables = pstr_common_tables;
}

VOID ixheaacd_set_sbr_persistent_buffers(VOID *sbr_persistent_mem_v,
                                         WORD32 *persistent_used,
                                         WORD32 num_channel, WORD ps_enable) {
  WORD32 i = 0;
  WORD32 used_persistent = *persistent_used;
  WORD32 temp, temp1, temp2, temp3;
  struct ia_sbr_pers_struct *sbr_persistent_mem =
      (struct ia_sbr_pers_struct *)sbr_persistent_mem_v;

  struct ia_sbr_dec_inst_struct *p_str_sbr_dec_inst =
      &sbr_persistent_mem->str_sbr_dec_inst;

  num_channel = max(2, num_channel);
  memset(sbr_persistent_mem, 0,
         IXHEAAC_GET_SIZE_ALIGNED(sizeof(struct ia_sbr_pers_struct), BYTE_ALIGN_8));

  sbr_persistent_mem->sbr_qmf_analy_states =
      (WORD16 *)((WORD8 *)sbr_persistent_mem_v + used_persistent);
  temp = num_channel *
         IXHEAAC_GET_SIZE_ALIGNED((QMF_FILTER_STATE_ANA_SIZE + 2 * NO_ANALYSIS_CHANNELS) *
                                      sizeof(sbr_persistent_mem->sbr_qmf_analy_states[0]),
                                  BYTE_ALIGN_8);
  used_persistent += temp;

  sbr_persistent_mem->sbr_qmf_analy_states_32 =
      (WORD32 *)((WORD8 *)sbr_persistent_mem_v + used_persistent);
  temp1 = num_channel *
          IXHEAAC_GET_SIZE_ALIGNED((QMF_FILTER_STATE_ANA_SIZE + 2 * NO_ANALYSIS_CHANNELS) *
                                       sizeof(sbr_persistent_mem->sbr_qmf_analy_states_32[0]),
                                   BYTE_ALIGN_8);
  used_persistent += temp1;

  sbr_persistent_mem->sbr_qmf_synth_states =
      (WORD16 *)((WORD8 *)sbr_persistent_mem_v + used_persistent);

  temp2 = (num_channel *
           IXHEAAC_GET_SIZE_ALIGNED((QMF_FILTER_STATE_SYN_SIZE + 2 * NO_SYNTHESIS_CHANNELS) *
                                        sizeof(sbr_persistent_mem->sbr_qmf_synth_states[0]),
                                    BYTE_ALIGN_8));
  used_persistent += temp2;

  sbr_persistent_mem->sbr_qmf_synth_states_32 =
      (WORD32 *)((WORD8 *)sbr_persistent_mem_v + used_persistent);

  temp3 = (num_channel *
           IXHEAAC_GET_SIZE_ALIGNED((QMF_FILTER_STATE_SYN_SIZE + 2 * NO_SYNTHESIS_CHANNELS) *
                                        sizeof(sbr_persistent_mem->sbr_qmf_synth_states_32[0]),
                                    BYTE_ALIGN_8));
  used_persistent += temp3;

  memset(sbr_persistent_mem->sbr_qmf_analy_states, 0,
         (temp + temp1 + temp2 + temp3));

  for (i = 0; i < num_channel; i++) {
    sbr_persistent_mem->ptr_sbr_overlap_buf[i] =
        (WORD32 *)((WORD8 *)sbr_persistent_mem_v + used_persistent);

    if (ps_enable) {
      memset(
          sbr_persistent_mem->ptr_sbr_overlap_buf[i], 0,
          2 * IXHEAAC_GET_SIZE_ALIGNED(MAX_OV_COLS * NO_SYNTHESIS_CHANNELS *
                                           sizeof(sbr_persistent_mem->ptr_sbr_overlap_buf[i][0]),
                                       BYTE_ALIGN_8));
      used_persistent +=
          2 * IXHEAAC_GET_SIZE_ALIGNED(MAX_OV_COLS * NO_SYNTHESIS_CHANNELS *
                                           sizeof(sbr_persistent_mem->ptr_sbr_overlap_buf[i][0]),
                                       BYTE_ALIGN_8);
    } else {
      memset(sbr_persistent_mem->ptr_sbr_overlap_buf[i], 0,
             IXHEAAC_GET_SIZE_ALIGNED(MAX_OV_COLS * NO_SYNTHESIS_CHANNELS *
                                          sizeof(sbr_persistent_mem->ptr_sbr_overlap_buf[i][0]),
                                      BYTE_ALIGN_8));
      used_persistent +=
          IXHEAAC_GET_SIZE_ALIGNED(MAX_OV_COLS * NO_SYNTHESIS_CHANNELS *
                                       sizeof(sbr_persistent_mem->ptr_sbr_overlap_buf[i][0]),
                                   BYTE_ALIGN_8);
    }
  }

  for (i = 0; i < num_channel; i++) {
    WORD32 j;
    sbr_persistent_mem->sbr_lpc_filter_states_real[i] =
        (WORD32 **)((WORD8 *)sbr_persistent_mem_v + used_persistent);
    used_persistent += IXHEAAC_GET_SIZE_ALIGNED(
        LPC_ORDER * sizeof(sbr_persistent_mem->sbr_lpc_filter_states_real[i][0]), BYTE_ALIGN_8);
    for (j = 0; j < LPC_ORDER; j++) {
      sbr_persistent_mem->sbr_lpc_filter_states_real[i][j] =
          (WORD32 *)((WORD8 *)sbr_persistent_mem_v + used_persistent);

      used_persistent += IXHEAAC_GET_SIZE_ALIGNED(
          NO_ANALYSIS_CHANNELS * sizeof(sbr_persistent_mem->sbr_lpc_filter_states_real[i][j][0]),
          BYTE_ALIGN_8);

      memset(sbr_persistent_mem->sbr_lpc_filter_states_real[i][j], 0,
             IXHEAAC_GET_SIZE_ALIGNED(
                 NO_ANALYSIS_CHANNELS *
                     sizeof(sbr_persistent_mem->sbr_lpc_filter_states_real[i][j][0]),
                 BYTE_ALIGN_8));
    }
  }

  if (ps_enable) {
    for (i = 0; i < num_channel; i++) {
      WORD32 j;

      sbr_persistent_mem->sbr_lpc_filter_states_imag[i] =
          (WORD32 **)((WORD8 *)sbr_persistent_mem_v + used_persistent);
      used_persistent += IXHEAAC_GET_SIZE_ALIGNED(
          LPC_ORDER * sizeof(sbr_persistent_mem->sbr_lpc_filter_states_imag[i][0]), BYTE_ALIGN_8);
      for (j = 0; j < LPC_ORDER; j++) {
        sbr_persistent_mem->sbr_lpc_filter_states_imag[i][j] =
            (WORD32 *)((WORD8 *)sbr_persistent_mem_v + used_persistent);

        used_persistent += IXHEAAC_GET_SIZE_ALIGNED(
            NO_ANALYSIS_CHANNELS *
                sizeof(sbr_persistent_mem->sbr_lpc_filter_states_imag[i][j][0]),
            BYTE_ALIGN_8);

        memset(sbr_persistent_mem->sbr_lpc_filter_states_imag[i][j], 0,
               IXHEAAC_GET_SIZE_ALIGNED(
                   NO_ANALYSIS_CHANNELS *
                       sizeof(sbr_persistent_mem->sbr_lpc_filter_states_imag[i][j][0]),
                   BYTE_ALIGN_8));
      }
    }
  }
  for (i = 0; i < num_channel; i++) {
    WORD32 initial_used = used_persistent;
    WORD32 temp_used = used_persistent;

    sbr_persistent_mem->sbr_smooth_gain_buf[i] =
        (WORD16 *)((WORD8 *)sbr_persistent_mem_v + temp_used);
    temp_used += 2 * IXHEAAC_GET_SIZE_ALIGNED(
                         MAX_FREQ_COEFFS * sizeof(sbr_persistent_mem->sbr_smooth_gain_buf[i][0]),
                         BYTE_ALIGN_8);

    sbr_persistent_mem->sbr_smooth_noise_buf[i] =
        (WORD16 *)((WORD8 *)sbr_persistent_mem_v + temp_used);

    temp_used += IXHEAAC_GET_SIZE_ALIGNED(
        MAX_FREQ_COEFFS * sizeof(sbr_persistent_mem->sbr_smooth_noise_buf[i][0]), BYTE_ALIGN_8);

    p_str_sbr_dec_inst->pstr_freq_band_data[i] =
        (VOID *)((WORD8 *)sbr_persistent_mem_v + temp_used);

    temp_used += IXHEAAC_GET_SIZE_ALIGNED(sizeof(ia_freq_band_data_struct), BYTE_ALIGN_8);

    sbr_persistent_mem->pstr_prev_frame_data[i] =
        (VOID *)((WORD8 *)sbr_persistent_mem_v + temp_used);

    temp_used += IXHEAAC_GET_SIZE_ALIGNED(sizeof(ia_sbr_prev_frame_data_struct), BYTE_ALIGN_8);

    p_str_sbr_dec_inst->pstr_sbr_channel[i] =
        (VOID *)((WORD8 *)sbr_persistent_mem_v + temp_used);

    temp_used += IXHEAAC_GET_SIZE_ALIGNED(sizeof(ia_sbr_channel_struct), BYTE_ALIGN_8);

    p_str_sbr_dec_inst->pstr_sbr_header[i] =
        (VOID *)((WORD8 *)sbr_persistent_mem_v + temp_used);

    temp_used += IXHEAAC_GET_SIZE_ALIGNED(sizeof(ia_sbr_header_data_struct), BYTE_ALIGN_8);

    memset(sbr_persistent_mem->sbr_smooth_gain_buf[i], 0,
           temp_used - initial_used);

    used_persistent = temp_used;
  }

  if (ps_enable) {
    p_str_sbr_dec_inst->pstr_ps_stereo_dec =
        (ia_ps_dec_struct *)((WORD8 *)sbr_persistent_mem_v + used_persistent);

    memset(p_str_sbr_dec_inst->pstr_ps_stereo_dec, 0,
           IXHEAAC_GET_SIZE_ALIGNED(sizeof(ia_ps_dec_struct), BYTE_ALIGN_8));

    used_persistent += IXHEAAC_GET_SIZE_ALIGNED(sizeof(ia_ps_dec_struct), BYTE_ALIGN_8);
  }

  p_str_sbr_dec_inst->frame_buffer[0] =
      (VOID *)((WORD8 *)sbr_persistent_mem_v + used_persistent);
  memset(p_str_sbr_dec_inst->frame_buffer[0], 0,
         IXHEAAC_GET_SIZE_ALIGNED(
             sizeof(ia_sbr_frame_info_data_struct) + MAX_FREQ_COEFFS * sizeof(WORD32) * 2 + 8,
             BYTE_ALIGN_8));
  used_persistent =
      used_persistent + IXHEAAC_GET_SIZE_ALIGNED(sizeof(ia_sbr_frame_info_data_struct) +
                                                     MAX_FREQ_COEFFS * sizeof(WORD32) + 8,
                                                 BYTE_ALIGN_8);

  p_str_sbr_dec_inst->frame_buffer[1] =
      (VOID *)((WORD8 *)sbr_persistent_mem_v + used_persistent);
  memset(p_str_sbr_dec_inst->frame_buffer[1], 0,
         IXHEAAC_GET_SIZE_ALIGNED(
             sizeof(ia_sbr_frame_info_data_struct) + MAX_FREQ_COEFFS * sizeof(WORD32) * 2 + 8,
             BYTE_ALIGN_8));
  used_persistent =
      used_persistent + IXHEAAC_GET_SIZE_ALIGNED(sizeof(ia_sbr_frame_info_data_struct) +
                                                     MAX_FREQ_COEFFS * sizeof(WORD32) + 8,
                                                 BYTE_ALIGN_8);

  {
    WORD32 index = 0;
    p_str_sbr_dec_inst->ptr_pvc_data_str =
        (ia_pvc_data_struct *)((WORD8 *)sbr_persistent_mem_v + used_persistent);
    memset(p_str_sbr_dec_inst->ptr_pvc_data_str, 0,
           IXHEAAC_GET_SIZE_ALIGNED(sizeof(ia_pvc_data_struct), BYTE_ALIGN_8));
    used_persistent += IXHEAAC_GET_SIZE_ALIGNED(sizeof(ia_pvc_data_struct), BYTE_ALIGN_8);

    p_str_sbr_dec_inst->pstr_sbr_channel[0]->str_sbr_dec.p_hbe_txposer =
        (ia_esbr_hbe_txposer_struct *)((WORD8 *)sbr_persistent_mem_v + used_persistent);
    memset(p_str_sbr_dec_inst->pstr_sbr_channel[0]->str_sbr_dec.p_hbe_txposer, 0,
           IXHEAAC_GET_SIZE_ALIGNED(sizeof(ia_esbr_hbe_txposer_struct), BYTE_ALIGN_8));
    used_persistent += IXHEAAC_GET_SIZE_ALIGNED(sizeof(ia_esbr_hbe_txposer_struct), BYTE_ALIGN_8);

    if (num_channel == 2) {
      p_str_sbr_dec_inst->pstr_sbr_channel[1]->str_sbr_dec.p_hbe_txposer =
          (ia_esbr_hbe_txposer_struct *)((WORD8 *)sbr_persistent_mem_v + used_persistent);
      memset(p_str_sbr_dec_inst->pstr_sbr_channel[1]->str_sbr_dec.p_hbe_txposer, 0,
             IXHEAAC_GET_SIZE_ALIGNED(sizeof(ia_esbr_hbe_txposer_struct), BYTE_ALIGN_8));
      used_persistent +=
          IXHEAAC_GET_SIZE_ALIGNED(sizeof(ia_esbr_hbe_txposer_struct), BYTE_ALIGN_8);
    }

    p_str_sbr_dec_inst->hbe_txposer_buffers =
        (VOID *)((WORD8 *)sbr_persistent_mem_v + used_persistent);
    memset(p_str_sbr_dec_inst->hbe_txposer_buffers, 0,
           num_channel * IXHEAAC_GET_SIZE_ALIGNED(MAX_HBE_PERSISTENT_SIZE, BYTE_ALIGN_8));
    used_persistent +=
        num_channel * IXHEAAC_GET_SIZE_ALIGNED(MAX_HBE_PERSISTENT_SIZE, BYTE_ALIGN_8);

    p_str_sbr_dec_inst->pstr_sbr_channel[0]->str_sbr_dec.pp_qmf_buf_real =
        (FLOAT32 **)((WORD8 *)sbr_persistent_mem_v + used_persistent);
    memset(
        p_str_sbr_dec_inst->pstr_sbr_channel[0]->str_sbr_dec.pp_qmf_buf_real, 0,
        IXHEAAC_GET_SIZE_ALIGNED(
            MAX_QMF_BUF_LEN *
                sizeof(p_str_sbr_dec_inst->pstr_sbr_channel[0]->str_sbr_dec.pp_qmf_buf_real[0]),
            BYTE_ALIGN_8));
    used_persistent += IXHEAAC_GET_SIZE_ALIGNED(
        MAX_QMF_BUF_LEN *
            sizeof(p_str_sbr_dec_inst->pstr_sbr_channel[0]->str_sbr_dec.pp_qmf_buf_real[0]),
        BYTE_ALIGN_8);

    p_str_sbr_dec_inst->pstr_sbr_channel[0]->str_sbr_dec.pp_qmf_buf_imag =
        (FLOAT32 **)((WORD8 *)sbr_persistent_mem_v + used_persistent);
    memset(
        p_str_sbr_dec_inst->pstr_sbr_channel[0]->str_sbr_dec.pp_qmf_buf_imag, 0,
        IXHEAAC_GET_SIZE_ALIGNED(
            MAX_QMF_BUF_LEN *
                sizeof(p_str_sbr_dec_inst->pstr_sbr_channel[0]->str_sbr_dec.pp_qmf_buf_imag[0]),
            BYTE_ALIGN_8));
    used_persistent += IXHEAAC_GET_SIZE_ALIGNED(
        MAX_QMF_BUF_LEN *
            sizeof(p_str_sbr_dec_inst->pstr_sbr_channel[0]->str_sbr_dec.pp_qmf_buf_imag[0]),
        BYTE_ALIGN_8);

    if (num_channel == 2) {
      p_str_sbr_dec_inst->pstr_sbr_channel[1]->str_sbr_dec.pp_qmf_buf_real =
          (FLOAT32 **)((WORD8 *)sbr_persistent_mem_v + used_persistent);
      memset(
          p_str_sbr_dec_inst->pstr_sbr_channel[1]->str_sbr_dec.pp_qmf_buf_real, 0,
          IXHEAAC_GET_SIZE_ALIGNED(
              MAX_QMF_BUF_LEN *
                  sizeof(p_str_sbr_dec_inst->pstr_sbr_channel[1]->str_sbr_dec.pp_qmf_buf_real[0]),
              BYTE_ALIGN_8));
      used_persistent += IXHEAAC_GET_SIZE_ALIGNED(
          MAX_QMF_BUF_LEN *
              sizeof(p_str_sbr_dec_inst->pstr_sbr_channel[1]->str_sbr_dec.pp_qmf_buf_real[0]),
          BYTE_ALIGN_8);

      p_str_sbr_dec_inst->pstr_sbr_channel[1]->str_sbr_dec.pp_qmf_buf_imag =
          (FLOAT32 **)((WORD8 *)sbr_persistent_mem_v + used_persistent);
      memset(
          p_str_sbr_dec_inst->pstr_sbr_channel[1]->str_sbr_dec.pp_qmf_buf_imag, 0,
          IXHEAAC_GET_SIZE_ALIGNED(
              MAX_QMF_BUF_LEN *
                  sizeof(p_str_sbr_dec_inst->pstr_sbr_channel[1]->str_sbr_dec.pp_qmf_buf_imag[0]),
              BYTE_ALIGN_8));
      used_persistent += IXHEAAC_GET_SIZE_ALIGNED(
          MAX_QMF_BUF_LEN *
              sizeof(p_str_sbr_dec_inst->pstr_sbr_channel[1]->str_sbr_dec.pp_qmf_buf_imag[0]),
          BYTE_ALIGN_8);
    }

    for (index = 0; index < MAX_QMF_BUF_LEN; index++) {
      p_str_sbr_dec_inst->pstr_sbr_channel[0]
          ->str_sbr_dec.pp_qmf_buf_real[index] =
          (FLOAT32 *)((WORD8 *)sbr_persistent_mem_v + used_persistent);
      used_persistent += IXHEAAC_GET_SIZE_ALIGNED(
          MAX_QMF_BUF_LEN *
              sizeof(
                  p_str_sbr_dec_inst->pstr_sbr_channel[0]->str_sbr_dec.pp_qmf_buf_real[index][0]),
          BYTE_ALIGN_8);
    }

    for (index = 0; index < MAX_QMF_BUF_LEN; index++) {
      p_str_sbr_dec_inst->pstr_sbr_channel[0]
          ->str_sbr_dec.pp_qmf_buf_imag[index] =
          (FLOAT32 *)((WORD8 *)sbr_persistent_mem_v + used_persistent);
      used_persistent += IXHEAAC_GET_SIZE_ALIGNED(
          MAX_QMF_BUF_LEN *
              sizeof(
                  p_str_sbr_dec_inst->pstr_sbr_channel[0]->str_sbr_dec.pp_qmf_buf_imag[index][0]),
          BYTE_ALIGN_8);
    }

    if (num_channel == 2) {
      for (index = 0; index < MAX_QMF_BUF_LEN; index++) {
        p_str_sbr_dec_inst->pstr_sbr_channel[1]
            ->str_sbr_dec.pp_qmf_buf_real[index] =
            (FLOAT32 *)((WORD8 *)sbr_persistent_mem_v + used_persistent);
        used_persistent += IXHEAAC_GET_SIZE_ALIGNED(
            MAX_QMF_BUF_LEN * sizeof(p_str_sbr_dec_inst->pstr_sbr_channel[1]
                                         ->str_sbr_dec.pp_qmf_buf_real[index][0]),
            BYTE_ALIGN_8);
      }

      for (index = 0; index < MAX_QMF_BUF_LEN; index++) {
        p_str_sbr_dec_inst->pstr_sbr_channel[1]
            ->str_sbr_dec.pp_qmf_buf_imag[index] =
            (FLOAT32 *)((WORD8 *)sbr_persistent_mem_v + used_persistent);
        used_persistent += IXHEAAC_GET_SIZE_ALIGNED(
            MAX_QMF_BUF_LEN * sizeof(p_str_sbr_dec_inst->pstr_sbr_channel[1]
                                         ->str_sbr_dec.pp_qmf_buf_imag[index][0]),
            BYTE_ALIGN_8);
      }
    }

    for (i = 0; i < MAX_ENV_COLS; i++) {
      p_str_sbr_dec_inst->pstr_sbr_channel[0]->str_sbr_dec.p_arr_qmf_buf_real[i] =
          (WORD32 *)((WORD8 *)sbr_persistent_mem_v + used_persistent);
      memset(p_str_sbr_dec_inst->pstr_sbr_channel[0]->str_sbr_dec.p_arr_qmf_buf_real[i], 0,
             IXHEAAC_GET_SIZE_ALIGNED(
                 MAX_QMF_BUF_LEN * sizeof(p_str_sbr_dec_inst->pstr_sbr_channel[0]
                                              ->str_sbr_dec.p_arr_qmf_buf_real[i][0]),
                 BYTE_ALIGN_8));
      used_persistent += IXHEAAC_GET_SIZE_ALIGNED(
          MAX_QMF_BUF_LEN *
              sizeof(
                  p_str_sbr_dec_inst->pstr_sbr_channel[0]->str_sbr_dec.p_arr_qmf_buf_real[i][0]),
          BYTE_ALIGN_8);

      p_str_sbr_dec_inst->pstr_sbr_channel[0]->str_sbr_dec.p_arr_qmf_buf_imag[i] =
          (WORD32 *)((WORD8 *)sbr_persistent_mem_v + used_persistent);
      memset(p_str_sbr_dec_inst->pstr_sbr_channel[0]->str_sbr_dec.p_arr_qmf_buf_imag[i], 0,
             IXHEAAC_GET_SIZE_ALIGNED(
                 MAX_QMF_BUF_LEN * sizeof(p_str_sbr_dec_inst->pstr_sbr_channel[0]
                                              ->str_sbr_dec.p_arr_qmf_buf_imag[i][0]),
                 BYTE_ALIGN_8));
      used_persistent += IXHEAAC_GET_SIZE_ALIGNED(
          MAX_QMF_BUF_LEN *
              sizeof(
                  p_str_sbr_dec_inst->pstr_sbr_channel[0]->str_sbr_dec.p_arr_qmf_buf_imag[i][0]),
          BYTE_ALIGN_8);
    }

    if (num_channel == 2) {
      for (i = 0; i < MAX_ENV_COLS; i++) {
        p_str_sbr_dec_inst->pstr_sbr_channel[1]->str_sbr_dec.p_arr_qmf_buf_real[i] =
            (WORD32 *)((WORD8 *)sbr_persistent_mem_v + used_persistent);
        memset(p_str_sbr_dec_inst->pstr_sbr_channel[1]->str_sbr_dec.p_arr_qmf_buf_real[i], 0,
               IXHEAAC_GET_SIZE_ALIGNED(
                   MAX_QMF_BUF_LEN * sizeof(p_str_sbr_dec_inst->pstr_sbr_channel[1]
                                                ->str_sbr_dec.p_arr_qmf_buf_real[i][0]),
                   BYTE_ALIGN_8));
        used_persistent += IXHEAAC_GET_SIZE_ALIGNED(
            MAX_QMF_BUF_LEN * sizeof(p_str_sbr_dec_inst->pstr_sbr_channel[1]
                                         ->str_sbr_dec.p_arr_qmf_buf_real[i][0]),
            BYTE_ALIGN_8);

        p_str_sbr_dec_inst->pstr_sbr_channel[1]->str_sbr_dec.p_arr_qmf_buf_imag[i] =
            (WORD32 *)((WORD8 *)sbr_persistent_mem_v + used_persistent);
        memset(p_str_sbr_dec_inst->pstr_sbr_channel[1]->str_sbr_dec.p_arr_qmf_buf_imag[i], 0,
               IXHEAAC_GET_SIZE_ALIGNED(
                   MAX_QMF_BUF_LEN * sizeof(p_str_sbr_dec_inst->pstr_sbr_channel[1]
                                                ->str_sbr_dec.p_arr_qmf_buf_imag[i][0]),
                   BYTE_ALIGN_8));
        used_persistent += IXHEAAC_GET_SIZE_ALIGNED(
            MAX_QMF_BUF_LEN * sizeof(p_str_sbr_dec_inst->pstr_sbr_channel[1]
                                         ->str_sbr_dec.p_arr_qmf_buf_imag[i][0]),
            BYTE_ALIGN_8);
      }
    }
  }

  *persistent_used = used_persistent;
}

static PLATFORM_INLINE VOID ixheaacd_init_headerdata(
    ia_sbr_header_data_struct *ptr_header_data, WORD32 sample_rate_dec,
    WORD32 samp_per_frame, ia_freq_band_data_struct *freq_band_data,
    ia_sbr_tables_struct *sbr_tables, WORD audio_obj_type) {
  ia_freq_band_data_struct *pstr_freq_band_data = freq_band_data;
  WORD32 tmp;

  if (audio_obj_type != AOT_ER_AAC_ELD) {
    memcpy(ptr_header_data,
           &sbr_tables->env_extr_tables_ptr->str_sbr_default_header,
           sizeof(ia_sbr_header_data_struct));
  }

  if (audio_obj_type == AOT_ER_AAC_ELD) ptr_header_data->time_step -= 1;

  pstr_freq_band_data->freq_band_table[LOW] =
      pstr_freq_band_data->freq_band_tbl_lo;
  pstr_freq_band_data->freq_band_table[HIGH] =
      pstr_freq_band_data->freq_band_tbl_hi;
  ptr_header_data->pstr_freq_band_data = pstr_freq_band_data;

  ptr_header_data->core_frame_size = samp_per_frame;
  ptr_header_data->out_sampling_freq = sample_rate_dec << 1;

  if (audio_obj_type != AOT_ER_AAC_ELD) {
    tmp = ptr_header_data->time_step + 4;

    if (tmp < 0)
      ptr_header_data->num_time_slots =
          ixheaac_extract16l(samp_per_frame << (-tmp));
    else
      ptr_header_data->num_time_slots =
          ixheaac_extract16l(samp_per_frame >> tmp);
  } else {
    ptr_header_data->time_step = 1;

    ptr_header_data->num_time_slots =
        (samp_per_frame / 32 >> (ptr_header_data->time_step - 1));
  }
}

VOID ixheaacd_setesbr_flags(VOID *sbr_persistent_mem_v, FLAG pvc_flag,
                            FLAG hbe_flag, FLAG inter_tes_flag) {
  ia_sbr_pers_struct *sbr_persistent_mem;
  sbr_persistent_mem = (ia_sbr_pers_struct *)sbr_persistent_mem_v;
  sbr_persistent_mem->str_sbr_dec_inst.hbe_flag = hbe_flag;
  sbr_persistent_mem->str_sbr_dec_inst.pvc_flag = pvc_flag;
  sbr_persistent_mem->str_sbr_dec_inst.inter_tes_flag = inter_tes_flag;
  return;
}

ia_handle_sbr_dec_inst_struct ixheaacd_init_sbr(
    WORD32 sample_rate_dec, WORD32 samp_per_frame, FLAG *down_sample_flag,
    VOID *sbr_persistent_mem_v, WORD32 *ptr_overlap_buf, WORD32 channel,
    WORD32 ps_enable, WORD32 sbr_ratio_idx, WORD32 output_frame_size,
    WORD32 *use_hbe, VOID *p_usac_dflt_header,
    ia_sbr_header_data_struct str_sbr_config, WORD32 audio_object_type,
    WORD32 ldmps_present, WORD32 ldsbr_present) {
  WORD16 i;
  WORD16 err;
  ia_sbr_header_data_struct *ptr_header_data[MAXNRSBRCHANNELS];
  ia_sbr_dec_struct *ptr_sbr_dec[2];
  ia_qmf_dec_tables_struct *qmf_dec_tables_ptr;
  ia_sbr_pers_struct *sbr_persistent_mem;

  sbr_persistent_mem = (ia_sbr_pers_struct *)sbr_persistent_mem_v;
  ptr_sbr_dec[0] =
      &sbr_persistent_mem->str_sbr_dec_inst.pstr_sbr_channel[0]->str_sbr_dec;
  ptr_sbr_dec[1] =
      &sbr_persistent_mem->str_sbr_dec_inst.pstr_sbr_channel[1]->str_sbr_dec;

  qmf_dec_tables_ptr =
      sbr_persistent_mem->str_sbr_dec_inst.pstr_sbr_tables->qmf_dec_tables_ptr;

  if (sample_rate_dec > 48000) {
    *down_sample_flag = 1;
  }

  for (i = 0; i < channel; i++) {
    if (audio_object_type == AOT_ER_AAC_ELD) {
      memcpy(sbr_persistent_mem->str_sbr_dec_inst.pstr_sbr_header[i],
             &str_sbr_config, sizeof(ia_sbr_header_data_struct));
    }
    ptr_header_data[i] =
        sbr_persistent_mem->str_sbr_dec_inst.pstr_sbr_header[i];

    ixheaacd_init_headerdata(
        ptr_header_data[i], sample_rate_dec, samp_per_frame,
        sbr_persistent_mem->str_sbr_dec_inst.pstr_freq_band_data[i],
        sbr_persistent_mem->str_sbr_dec_inst.pstr_sbr_tables,
        audio_object_type);

    err = ixheaacd_create_sbrdec(

        sbr_persistent_mem->str_sbr_dec_inst.pstr_common_tables,
        sbr_persistent_mem->str_sbr_dec_inst.pstr_sbr_channel[i],
        ptr_header_data[i], i, *down_sample_flag, sbr_persistent_mem, ps_enable,
        audio_object_type, ldmps_present, ldsbr_present);

    ptr_header_data[i]->status = 1;
    ptr_sbr_dec[i]->band_count = 64;
    ptr_header_data[i]->pstr_freq_band_data[0].qmf_sb_prev = 64;
    ptr_header_data[i]->pstr_freq_band_data[1].qmf_sb_prev = 64;

    if (err) {
      return NULL;
    }
  }

  if (channel != 1) {
    if (ps_enable) {
      if (audio_object_type == AOT_ER_AAC_ELD)
        ixheaacd_create_psdec(
            sbr_persistent_mem->str_sbr_dec_inst.pstr_ps_stereo_dec,
            sbr_persistent_mem, &ptr_overlap_buf[512 * 4], samp_per_frame);
      else
        ixheaacd_create_psdec(
            sbr_persistent_mem->str_sbr_dec_inst.pstr_ps_stereo_dec,
            sbr_persistent_mem, ptr_overlap_buf, samp_per_frame);
    }
  }

  if ((use_hbe != NULL) && !((audio_object_type == AOT_ER_AAC_ELD) ||
     (audio_object_type == AOT_ER_AAC_LD))) {
    ia_sbr_header_data_struct *ptr_sbr_dflt_header =
        &sbr_persistent_mem->str_sbr_dec_inst.str_sbr_dflt_header;
    ia_sbr_header_data_struct *ptr_usac_dflt_header =
        (ia_sbr_header_data_struct *)p_usac_dflt_header;
    struct ia_sbr_dec_inst_struct *p_str_sbr_dec_inst =
        &sbr_persistent_mem->str_sbr_dec_inst;
    VOID *hbe_txposer_buffers = p_str_sbr_dec_inst->hbe_txposer_buffers;

    ptr_header_data[0] = p_str_sbr_dec_inst->pstr_sbr_header[0];
    ptr_header_data[1] = p_str_sbr_dec_inst->pstr_sbr_header[1];

    ptr_header_data[0]->sbr_ratio_idx = sbr_ratio_idx;
    ptr_header_data[0]->output_framesize = output_frame_size;
    ptr_header_data[0]->pstr_freq_band_data->sub_band_start = 64;
    ptr_header_data[0]->esbr_start_up = 1;
    ptr_header_data[0]->esbr_start_up_pvc = 1;

    if (channel > 1) {
      ptr_header_data[1]->sbr_ratio_idx = sbr_ratio_idx;
      ptr_header_data[1]->output_framesize = output_frame_size;
      ptr_header_data[1]->pstr_freq_band_data->sub_band_start = 64;
      ptr_header_data[1]->esbr_start_up = 1;
      ptr_header_data[1]->esbr_start_up_pvc = 1;
    }
    if (hbe_txposer_buffers != NULL) {
      WORD32 persistant_used = 0;
      ixheaacd_esbr_hbe_data_init(ptr_sbr_dec[0]->p_hbe_txposer, samp_per_frame,
                                  sbr_ratio_idx == SBR_UPSAMPLE_IDX_4_1 ? 1 : 0,
                                  output_frame_size, hbe_txposer_buffers, &persistant_used);

      hbe_txposer_buffers = (WORD8 *)hbe_txposer_buffers +
                            IXHEAAC_GET_SIZE_ALIGNED(MAX_HBE_PERSISTENT_SIZE, BYTE_ALIGN_8);

      ixheaacd_esbr_hbe_data_init(ptr_sbr_dec[1]->p_hbe_txposer, samp_per_frame,
                                  sbr_ratio_idx == SBR_UPSAMPLE_IDX_4_1 ? 1 : 0,
                                  output_frame_size, hbe_txposer_buffers, &persistant_used);
    }

    p_str_sbr_dec_inst->ptr_pvc_data_str->prev_first_bnd_idx = -1;
    p_str_sbr_dec_inst->ptr_pvc_data_str->prev_pvc_rate = -1;
    p_str_sbr_dec_inst->ptr_pvc_data_str->prev_sbr_mode = UNKNOWN_SBR;

    p_str_sbr_dec_inst->pstr_sbr_channel[0]
        ->str_sbr_dec.str_codec_qmf_bank.num_time_slots =
        output_frame_size / 64;
    p_str_sbr_dec_inst->pstr_sbr_channel[1]
        ->str_sbr_dec.str_codec_qmf_bank.num_time_slots =
        output_frame_size / 64;

    ptr_header_data[0]->core_frame_size = samp_per_frame;
    ptr_header_data[1]->core_frame_size = samp_per_frame;

    switch (sbr_ratio_idx) {
      case SBR_UPSAMPLE_IDX_0_0:
        ptr_sbr_dec[0]->str_codec_qmf_bank.no_channels = 32;
        ptr_sbr_dec[0]->str_codec_qmf_bank.esbr_cos_twiddle =
            (WORD32 *)qmf_dec_tables_ptr->esbr_sin_cos_twiddle_l32;
        ptr_sbr_dec[0]->str_codec_qmf_bank.esbr_alt_sin_twiddle =
            (WORD32 *)qmf_dec_tables_ptr->esbr_alt_sin_twiddle_l32;
        ptr_sbr_dec[0]->str_codec_qmf_bank.esbr_t_cos =
            (WORD32 *)qmf_dec_tables_ptr->esbr_t_cos_sin_l32;
        ptr_header_data[0]->is_usf_4 = 0;
        ptr_header_data[0]->upsamp_fac = 1;

        ptr_sbr_dec[1]->str_codec_qmf_bank.no_channels = 32;
        ptr_sbr_dec[1]->str_codec_qmf_bank.esbr_cos_twiddle =
            (WORD32 *)qmf_dec_tables_ptr->esbr_sin_cos_twiddle_l32;
        ptr_sbr_dec[1]->str_codec_qmf_bank.esbr_alt_sin_twiddle =
            (WORD32 *)qmf_dec_tables_ptr->esbr_alt_sin_twiddle_l32;
        ptr_sbr_dec[1]->str_codec_qmf_bank.esbr_t_cos =
            (WORD32 *)qmf_dec_tables_ptr->esbr_t_cos_sin_l32;
        ptr_header_data[1]->is_usf_4 = 0;
        ptr_header_data[1]->upsamp_fac = 1;
        break;
      case SBR_UPSAMPLE_IDX_2_1:
        ptr_sbr_dec[0]->str_codec_qmf_bank.no_channels = 32;
        ptr_sbr_dec[0]->str_codec_qmf_bank.esbr_cos_twiddle =
            (WORD32 *)qmf_dec_tables_ptr->esbr_sin_cos_twiddle_l32;
        ptr_sbr_dec[0]->str_codec_qmf_bank.esbr_alt_sin_twiddle =
            (WORD32 *)qmf_dec_tables_ptr->esbr_alt_sin_twiddle_l32;
        ptr_sbr_dec[0]->str_codec_qmf_bank.esbr_t_cos =
            (WORD32 *)qmf_dec_tables_ptr->esbr_t_cos_sin_l32;
        ptr_header_data[0]->is_usf_4 = 0;
        ptr_header_data[0]->upsamp_fac = 2;

        ptr_sbr_dec[1]->str_codec_qmf_bank.no_channels = 32;
        ptr_sbr_dec[1]->str_codec_qmf_bank.esbr_cos_twiddle =
            (WORD32 *)qmf_dec_tables_ptr->esbr_sin_cos_twiddle_l32;
        ptr_sbr_dec[1]->str_codec_qmf_bank.esbr_alt_sin_twiddle =
            (WORD32 *)qmf_dec_tables_ptr->esbr_alt_sin_twiddle_l32;
        ptr_sbr_dec[1]->str_codec_qmf_bank.esbr_t_cos =
            (WORD32 *)qmf_dec_tables_ptr->esbr_t_cos_sin_l32;
        ptr_header_data[1]->is_usf_4 = 0;
        ptr_header_data[1]->upsamp_fac = 2;
        break;
      case SBR_UPSAMPLE_IDX_8_3:
        ptr_sbr_dec[0]->str_codec_qmf_bank.no_channels = 24;
        ptr_sbr_dec[0]->str_codec_qmf_bank.filter_pos_32 =
            qmf_dec_tables_ptr->esbr_qmf_c_24;
        ptr_sbr_dec[0]->str_codec_qmf_bank.analy_win_coeff_32 =
            qmf_dec_tables_ptr->esbr_qmf_c_24;
        ptr_sbr_dec[0]->str_codec_qmf_bank.esbr_cos_twiddle =
            (WORD32 *)qmf_dec_tables_ptr->esbr_sin_cos_twiddle_l24;
        ptr_sbr_dec[0]->str_codec_qmf_bank.esbr_alt_sin_twiddle =
            (WORD32 *)qmf_dec_tables_ptr->esbr_alt_sin_twiddle_l24;
        ptr_sbr_dec[0]->str_codec_qmf_bank.esbr_t_cos =
            (WORD32 *)qmf_dec_tables_ptr->esbr_t_cos_sin_l24;

        ptr_header_data[0]->is_usf_4 = 0;
        ptr_header_data[0]->upsamp_fac = 2;

        ptr_sbr_dec[1]->str_codec_qmf_bank.no_channels = 24;
        ptr_sbr_dec[1]->str_codec_qmf_bank.filter_pos_32 =
            qmf_dec_tables_ptr->esbr_qmf_c_24;
        ptr_sbr_dec[1]->str_codec_qmf_bank.analy_win_coeff_32 =
            qmf_dec_tables_ptr->esbr_qmf_c_24;
        ptr_sbr_dec[1]->str_codec_qmf_bank.esbr_cos_twiddle =
            (WORD32 *)qmf_dec_tables_ptr->esbr_sin_cos_twiddle_l24;
        ptr_sbr_dec[1]->str_codec_qmf_bank.esbr_alt_sin_twiddle =
            (WORD32 *)qmf_dec_tables_ptr->esbr_alt_sin_twiddle_l24;
        ptr_sbr_dec[1]->str_codec_qmf_bank.esbr_t_cos =
            (WORD32 *)qmf_dec_tables_ptr->esbr_t_cos_sin_l24;

        ptr_header_data[1]->is_usf_4 = 0;
        ptr_header_data[1]->upsamp_fac = 2;
        break;
      case SBR_UPSAMPLE_IDX_4_1:
        ptr_sbr_dec[0]->str_codec_qmf_bank.no_channels = 16;
        ptr_sbr_dec[0]->str_codec_qmf_bank.esbr_cos_twiddle =
            (WORD32 *)qmf_dec_tables_ptr->esbr_sin_cos_twiddle_l16;
        ptr_sbr_dec[0]->str_codec_qmf_bank.esbr_alt_sin_twiddle =
            (WORD32 *)qmf_dec_tables_ptr->esbr_alt_sin_twiddle_l16;
        ptr_sbr_dec[0]->str_codec_qmf_bank.esbr_t_cos =
            (WORD32 *)qmf_dec_tables_ptr->esbr_t_cos_sin_l16;
        ptr_header_data[0]->is_usf_4 = 1;
        ptr_header_data[0]->upsamp_fac = 4;
        ptr_header_data[0]->out_sampling_freq =
            ptr_header_data[0]->out_sampling_freq * 2;

        ptr_sbr_dec[1]->str_codec_qmf_bank.no_channels = 16;
        ptr_sbr_dec[1]->str_codec_qmf_bank.esbr_cos_twiddle =
            (WORD32 *)qmf_dec_tables_ptr->esbr_sin_cos_twiddle_l16;
        ptr_sbr_dec[1]->str_codec_qmf_bank.esbr_alt_sin_twiddle =
            (WORD32 *)qmf_dec_tables_ptr->esbr_alt_sin_twiddle_l16;
        ptr_sbr_dec[1]->str_codec_qmf_bank.esbr_t_cos =
            (WORD32 *)qmf_dec_tables_ptr->esbr_t_cos_sin_l16;
        ptr_header_data[1]->is_usf_4 = 1;
        ptr_header_data[1]->upsamp_fac = 4;
        ptr_header_data[1]->out_sampling_freq =
            ptr_header_data[1]->out_sampling_freq * 2;
        break;
    }

    if (ptr_usac_dflt_header != NULL) {
      ptr_sbr_dflt_header->start_freq = ptr_usac_dflt_header->start_freq;
      ptr_sbr_dflt_header->stop_freq = ptr_usac_dflt_header->stop_freq;

      if (ptr_usac_dflt_header->header_extra_1) {
        ptr_sbr_dflt_header->freq_scale = ptr_usac_dflt_header->freq_scale;
        ptr_sbr_dflt_header->alter_scale = ptr_usac_dflt_header->alter_scale;
        ptr_sbr_dflt_header->noise_bands = ptr_usac_dflt_header->noise_bands;
      } else {
        ptr_sbr_dflt_header->freq_scale = SBR_FREQ_SCALE_DEFAULT;
        ptr_sbr_dflt_header->alter_scale = SBR_ALTER_SCALE_DEFAULT;
        ptr_sbr_dflt_header->noise_bands = SBR_NOISE_BANDS_DEFAULT;
      }

      if (ptr_usac_dflt_header->header_extra_2) {
        ptr_sbr_dflt_header->limiter_bands = ptr_usac_dflt_header->limiter_bands;
        ptr_sbr_dflt_header->limiter_gains = ptr_usac_dflt_header->limiter_gains;
        ptr_sbr_dflt_header->interpol_freq = ptr_usac_dflt_header->interpol_freq;
        ptr_sbr_dflt_header->smoothing_mode =
            ptr_usac_dflt_header->smoothing_mode;
      } else {
        ptr_sbr_dflt_header->limiter_bands = SBR_LIMITER_BANDS_DEFAULT;
        ptr_sbr_dflt_header->limiter_gains = SBR_LIMITER_GAINS_DEFAULT;
        ptr_sbr_dflt_header->interpol_freq = SBR_INTERPOL_FREQ_DEFAULT;
        ptr_sbr_dflt_header->smoothing_mode = SBR_SMOOTHING_LENGTH_DEFAULT;
      }
    }
  }
  return &(sbr_persistent_mem->str_sbr_dec_inst);
}

static PLATFORM_INLINE WORD16 ixheaacd_create_sbr_env_calc(

    ixheaacd_misc_tables *pstr_common_table, ia_sbr_calc_env_struct *hs,
    WORD16 chan, VOID *sbr_persistent_mem_v,
    ia_sbr_header_data_struct *ptr_header_data, WORD audio_object_type) {
  WORD16 err;
  ia_sbr_pers_struct *sbr_persistent_mem =
      (ia_sbr_pers_struct *)sbr_persistent_mem_v;

  err = 0;
  memset(&hs->harm_flags_prev[0], 0, sizeof(WORD8) * MAX_FREQ_COEFFS);

  hs->harm_index = 0;

  hs->filt_buf_me = sbr_persistent_mem->sbr_smooth_gain_buf[chan];
  hs->filt_buf_noise_m = sbr_persistent_mem->sbr_smooth_noise_buf[chan];
  hs->tansient_env_prev = -1;

  ixheaacd_reset_sbrenvelope_calc(hs);

  if ((chan == 0) && (audio_object_type == AOT_ER_AAC_ELD)) {
    err = ixheaacd_calc_frq_bnd_tbls(ptr_header_data, pstr_common_table);
  }

  return err;
}

static PLATFORM_INLINE VOID ixheaacd_init_sbr_prev_framedata(
    ia_sbr_prev_frame_data_struct *ptr_prev_data, WORD16 time_slots) {
  WORD16 *psfb_nrg_prev = ptr_prev_data->sfb_nrg_prev;
  WORD16 *psfb_noise_level = ptr_prev_data->prev_noise_level;
  WORD32 *ppsbr_invf_mode = ptr_prev_data->sbr_invf_mode;

  memset(psfb_nrg_prev, 0, sizeof(WORD16) * (MAX_FREQ_COEFFS));
  memset(psfb_noise_level, 0, sizeof(WORD16) * (MAX_NOISE_COEFFS));

  memset(ppsbr_invf_mode, 0, sizeof(WORD32) * MAX_INVF_BANDS);

  ptr_prev_data->end_position = time_slots;
  ptr_prev_data->coupling_mode = COUPLING_OFF;
  ptr_prev_data->amp_res = 0;
  ptr_prev_data->max_qmf_subband_aac = 0;
}

static PLATFORM_INLINE VOID
ixheaacd_create_hyb_filterbank(ia_hybrid_struct *ptr_hybrid, WORD32 **p_ptr,
                               ia_sbr_tables_struct *sbr_tables_ptr) {
  WORD16 i;
  WORD32 *ptr = (WORD32 *)*p_ptr;

  ptr_hybrid->ptr_resol = sbr_tables_ptr->ps_tables_ptr->hyb_resol;
  ptr_hybrid->ptr_qmf_buf = HYBRID_FILTER_LENGTH - 1;

  ptr_hybrid->ptr_temp_re = ptr;
  ptr += NO_HYBRID_CHANNELS_HIGH;
  ptr_hybrid->ptr_temp_im = ptr;
  ptr += NO_HYBRID_CHANNELS_HIGH;

  memset(ptr_hybrid->ptr_temp_re, 0,
         2 * NO_HYBRID_CHANNELS_HIGH * sizeof(WORD32));

  ptr_hybrid->ptr_work_re = ptr;
  ptr += 16;
  ptr_hybrid->ptr_work_im = ptr;
  ptr += 16;

  for (i = 0; i < NO_QMF_CHANNELS_IN_HYBRID; i++) {
    ptr_hybrid->ptr_qmf_buf_re[i] = ptr;
    ptr += ptr_hybrid->ptr_qmf_buf;

    ptr_hybrid->ptr_qmf_buf_im[i] = ptr;
    ptr += ptr_hybrid->ptr_qmf_buf;

    memset(ptr_hybrid->ptr_qmf_buf_re[i], 0,
           2 * ptr_hybrid->ptr_qmf_buf * sizeof(WORD32));
  }

  *p_ptr = ptr;

  return;
}

static PLATFORM_INLINE VOID ixheaacd_create_hf_generator(
    ia_sbr_hf_generator_struct *ptr_hf_gen_str, WORD16 num_columns, WORD16 chan,
    VOID *sbr_persistent_mem_v, WORD32 ps_enable) {
  WORD16 i;
  ia_sbr_pers_struct *sbr_persistent_mem =
      (ia_sbr_pers_struct *)sbr_persistent_mem_v;

  ptr_hf_gen_str->pstr_settings = &sbr_persistent_mem->str_sbr_tran_settings;

  ptr_hf_gen_str->lpc_filt_states_real[0] =
      sbr_persistent_mem->sbr_lpc_filter_states_real[chan][0];
  ptr_hf_gen_str->lpc_filt_states_real[1] =
      sbr_persistent_mem->sbr_lpc_filter_states_real[chan][1];

  if (ps_enable) {
    ptr_hf_gen_str->lpc_filt_states_imag[0] =
        sbr_persistent_mem->sbr_lpc_filter_states_imag[chan][0];
    ptr_hf_gen_str->lpc_filt_states_imag[1] =
        sbr_persistent_mem->sbr_lpc_filter_states_imag[chan][1];
  }

  for (i = 0; i < LPC_ORDER; i++) {
    if (ptr_hf_gen_str->lpc_filt_states_real[i] != NULL) {
      memset(ptr_hf_gen_str->lpc_filt_states_real[i], 0,
             NO_ANALYSIS_CHANNELS * sizeof(WORD32));
    }

    if (ps_enable)
      memset(ptr_hf_gen_str->lpc_filt_states_imag[i], 0,
             NO_ANALYSIS_CHANNELS * sizeof(WORD32));
  }

  if (chan == 0) {
    ptr_hf_gen_str->pstr_settings->num_columns = num_columns;
  }
  return;
}

VOID ixheaacd_create_psdec(ia_ps_dec_struct *ptr_ps_dec,
                           VOID *sbr_persistent_mem_v,
                           WORD32 *ptr_overlap_buf, WORD32 frame_size) {
  ia_sbr_pers_struct *sbr_persistent_mem =
      (ia_sbr_pers_struct *)sbr_persistent_mem_v;

  WORD16 *ptr1 = (WORD16 *)&(
      sbr_persistent_mem->ptr_sbr_overlap_buf[MAXNRSBRCHANNELS - 1][0]);
  WORD32 *ptr2 = (WORD32 *)&ptr_overlap_buf[512];
  WORD16 *initial_ptr;
  WORD16 delay;
  WORD32 temp;

  ia_sbr_tables_struct *sbr_tables_ptr =
      sbr_persistent_mem->str_sbr_dec_inst.pstr_sbr_tables;

  memset(ptr_ps_dec, 0, sizeof(ia_ps_dec_struct));

  ptr_ps_dec->ps_data_present = 0;
  ptr_ps_dec->enable_iid = 0;
  ptr_ps_dec->enable_icc = 0;
  ptr_ps_dec->enable_ext = 0;
  ptr_ps_dec->iid_mode = 0;
  ptr_ps_dec->icc_mode = 0;

  ptr_ps_dec->ptr_hyb_left_re = ptr2;
  ptr2 += 16;
  ptr_ps_dec->ptr_hyb_left_im = ptr2;
  ptr2 += 16;
  ptr_ps_dec->ptr_hyb_right_re = ptr2;
  ptr2 += 16;
  ptr_ps_dec->ptr_hyb_right_im = ptr2;
  ptr2 += 16;

  memset(ptr_ps_dec->ptr_hyb_left_re, 0, sizeof(WORD32) * 16 * 4);

  ixheaacd_create_hyb_filterbank(&ptr_ps_dec->str_hybrid, &ptr2,
                                 sbr_tables_ptr);

  ptr_ps_dec->peak_decay_diff = ptr2;
  ptr2 += NUM_OF_BINS;
  ptr_ps_dec->energy_prev = ptr2;
  ptr2 += NUM_OF_BINS;
  ptr_ps_dec->peak_decay_diff_prev = ptr2;
  ptr2 += NUM_OF_BINS;

  memset(ptr_ps_dec->peak_decay_diff, 0, 3 * sizeof(WORD32) * NUM_OF_BINS);

  ptr_ps_dec->delay_buf_idx = 0;
  ptr_ps_dec->delay_buf_idx_long = 0;

  memset(ptr_ps_dec->delay_buf_qmf_sub_re_im, 0,
         2 * 16 * DEL_ALL_PASS * sizeof(WORD16));
  memset(ptr_ps_dec->delay_buf_qmf_sub_ser_re_im, 0,
         2 * 16 * NUM_SER_AP_LINKS * 5 * sizeof(WORD16));

  initial_ptr = ptr1;
  ptr_ps_dec->delay_buf_qmf_ser_re_im = (VOID *)ptr1;
  ptr1 += 2 * NUM_SER_AP_LINKS * 32 * 5;

  delay = 2;
  ptr_ps_dec->delay_buf_qmf_ap_re_im = (VOID *)ptr1;
  ptr1 += 2 * delay * 32;

  delay = HIGH_DEL;
  ptr_ps_dec->delay_buf_qmf_ld_re_im = (VOID *)ptr1;
  ptr1 += 2 * delay * SMALL_DEL_STRT;

  delay = SMALL_DEL;
  ptr_ps_dec->delay_buf_qmf_sd_re_im = (VOID *)ptr1;
  ptr1 +=
      2 * delay * (NUM_OF_QUAD_MIRROR_FILTER_ICC_CHNLS -
                   (NUM_OF_QUAD_MIRROR_FILTER_ALL_PASS_CHNLS + SMALL_DEL_STRT));

  temp = (WORD32)(ptr1 - initial_ptr);
  memset(ptr_ps_dec->delay_buf_qmf_ser_re_im, 0, temp * sizeof(WORD16));

  memset(ptr_ps_dec->delay_buf_idx_ser, 0, NUM_SER_AP_LINKS * sizeof(WORD16));
  memcpy(ptr_ps_dec->delay_sample_ser,
         sbr_tables_ptr->ps_tables_ptr->rev_link_delay_ser,
         NUM_SER_AP_LINKS * sizeof(WORD16));

  memset(ptr_ps_dec->h11_h12_vec, 0xff,
         (NO_IID_GROUPS + 2) * 2 * sizeof(WORD16));
  memset(ptr_ps_dec->h21_h22_vec, 0, sizeof(ptr_ps_dec->h21_h22_vec));

  if (frame_size == 960)
    ptr_ps_dec->num_sub_samples = NUM_SUB_SAMPLES_960;
  else
    ptr_ps_dec->num_sub_samples = NUM_SUB_SAMPLES;


  ixheaacd_create_ps_esbr_dec(ptr_ps_dec, sbr_tables_ptr->ps_tables_ptr,
                              64, ptr_ps_dec->num_sub_samples, 0);

  return;
}

static PLATFORM_INLINE VOID ixheaacd_create_cplx_anal_qmfbank(
    ia_sbr_qmf_filter_bank_struct *ptr_sbr_qmf,
    ia_sbr_scale_fact_struct *sbr_scale_factor, WORD16 no_bins, WORD16 usb,
    WORD16 chan, WORD16 *sbr_qmf_analy_states, WORD32 *sbr_qmf_analy_states_32,
    ia_qmf_dec_tables_struct *qmf_dec_tables_ptr, WORD32 audio_object_type,
    WORD32 ldmps_present, WORD32 no_ldsbr) {
  memset(ptr_sbr_qmf, 0, sizeof(ia_sbr_qmf_filter_bank_struct));

  ptr_sbr_qmf->analy_win_coeff_32 = qmf_dec_tables_ptr->esbr_qmf_c;
  if (audio_object_type != AOT_ER_AAC_ELD &&
      audio_object_type != AOT_ER_AAC_LD) {
    ptr_sbr_qmf->analy_win_coeff = qmf_dec_tables_ptr->qmf_c;
  } else {
    ptr_sbr_qmf->analy_win_coeff = qmf_dec_tables_ptr->qmf_c_eld3;
    if (ldmps_present == 1)
      ptr_sbr_qmf->analy_win_coeff_32 = qmf_dec_tables_ptr->qmf_c_ldsbr_mps;
    if (no_ldsbr == 1)
      ptr_sbr_qmf->analy_win_coeff_32 =
          (WORD32 *)ixheaacd_ldmps_polyphase_filter_coeff_fix;
  }

  ptr_sbr_qmf->no_channels = NO_ANALYSIS_CHANNELS;
  if (no_ldsbr) ptr_sbr_qmf->no_channels = 64;
  ptr_sbr_qmf->num_time_slots = no_bins;

  ptr_sbr_qmf->lsb = 0;
  ptr_sbr_qmf->usb = usb;

  ptr_sbr_qmf->anal_filter_states =
      &(sbr_qmf_analy_states[chan ? QMF_FILTER_STATE_ANA_SIZE : 0]);

  memset(ptr_sbr_qmf->anal_filter_states, 0,
         sizeof(WORD16) * QMF_FILTER_STATE_ANA_SIZE);

  ptr_sbr_qmf->anal_filter_states_32 =
      &(sbr_qmf_analy_states_32[chan ? QMF_FILTER_STATE_ANA_SIZE : 0]);

  memset(ptr_sbr_qmf->anal_filter_states_32, 0,
         sizeof(WORD32) * QMF_FILTER_STATE_ANA_SIZE);

  ptr_sbr_qmf->core_samples_buffer = ptr_sbr_qmf->anal_filter_states;
  ptr_sbr_qmf->core_samples_buffer_32 = ptr_sbr_qmf->anal_filter_states_32;

  ptr_sbr_qmf->filter_pos_32 = (WORD32 *)qmf_dec_tables_ptr->esbr_qmf_c;
  ptr_sbr_qmf->state_new_samples_pos_low_32 =
      ptr_sbr_qmf->anal_filter_states_32;
  if (audio_object_type != AOT_ER_AAC_ELD &&
      audio_object_type != AOT_ER_AAC_LD) {
    ptr_sbr_qmf->filter_pos = (WORD16 *)qmf_dec_tables_ptr->qmf_c;
  } else {
    ptr_sbr_qmf->filter_pos = (WORD16 *)qmf_dec_tables_ptr->qmf_c_eld3;
    if (ldmps_present == 1)
      ptr_sbr_qmf->filter_pos_32 =
          (WORD32 *)qmf_dec_tables_ptr->qmf_c_ldsbr_mps;
    if (no_ldsbr == 1)
      ptr_sbr_qmf->filter_pos_32 =
          (WORD32 *)ixheaacd_ldmps_polyphase_filter_coeff_fix;
  }

  sbr_scale_factor->st_lb_scale = 0;

  sbr_scale_factor->st_syn_scale = -6;

  if (audio_object_type == AOT_ER_AAC_ELD ||
      audio_object_type == AOT_ER_AAC_LD) {
    ptr_sbr_qmf->filter_2_32 =
        ptr_sbr_qmf->filter_pos_32 + ptr_sbr_qmf->no_channels;
    ptr_sbr_qmf->fp1_anal_32 = ptr_sbr_qmf->anal_filter_states_32;
    ptr_sbr_qmf->fp2_anal_32 =
        ptr_sbr_qmf->anal_filter_states_32 + ptr_sbr_qmf->no_channels;

    ptr_sbr_qmf->filter_2 = ptr_sbr_qmf->filter_pos + ptr_sbr_qmf->no_channels;
    ptr_sbr_qmf->fp1_anal = ptr_sbr_qmf->anal_filter_states;
    ptr_sbr_qmf->fp2_anal =
        ptr_sbr_qmf->anal_filter_states + ptr_sbr_qmf->no_channels;
  }

  return;
}

static PLATFORM_INLINE VOID ixheaacd_create_cplx_synt_qmfbank(
    ia_sbr_qmf_filter_bank_struct *ptr_sbr_qmf, WORD16 no_bins, WORD16 lsb,
    WORD16 usb, WORD16 chan, FLAG down_sample_flag,
    WORD16 *sbr_qmf_synth_states, WORD32 *sbr_qmf_synth_states_32,
    ia_qmf_dec_tables_struct *qmf_dec_tables_ptr, WORD32 audio_object_type) {
  WORD32 L;

  WORD32 qmf_filter_state_size;

  memset(ptr_sbr_qmf, 0, sizeof(ia_sbr_qmf_filter_bank_struct));

  if (down_sample_flag) {
    L = NO_SYNTHESIS_CHANNELS_DOWN_SAMPLED;
    qmf_filter_state_size = QMF_FILTER_STATE_SYN_SIZE_DOWN_SAMPLED;
    ptr_sbr_qmf->usb = NO_SYNTHESIS_CHANNELS_DOWN_SAMPLED;
  } else {
    L = NO_SYNTHESIS_CHANNELS;
    qmf_filter_state_size = QMF_FILTER_STATE_SYN_SIZE;
    ptr_sbr_qmf->usb = usb;
  }

  ptr_sbr_qmf->ixheaacd_drc_offset = 0;
  if (audio_object_type != AOT_ER_AAC_ELD &&
      audio_object_type != AOT_ER_AAC_LD) {
    ptr_sbr_qmf->filter_pos_syn = (WORD16 *)qmf_dec_tables_ptr->qmf_c;
    ptr_sbr_qmf->p_filter = qmf_dec_tables_ptr->qmf_c;
  } else {
    ptr_sbr_qmf->filter_pos_syn = (WORD16 *)qmf_dec_tables_ptr->qmf_c_eld;
    ptr_sbr_qmf->p_filter = qmf_dec_tables_ptr->qmf_c_eld;
  }

  ptr_sbr_qmf->filter_pos_syn_32 = (WORD32 *)qmf_dec_tables_ptr->esbr_qmf_c;
  ptr_sbr_qmf->p_filter_32 = qmf_dec_tables_ptr->esbr_qmf_c;

  ptr_sbr_qmf->no_channels = L;
  ptr_sbr_qmf->qmf_filter_state_size = qmf_filter_state_size;
  ptr_sbr_qmf->num_time_slots = no_bins;
  ptr_sbr_qmf->lsb = lsb;

  ptr_sbr_qmf->filter_states =
      &sbr_qmf_synth_states[chan ? qmf_filter_state_size : 0];

  memset(ptr_sbr_qmf->filter_states, 0, sizeof(WORD16) * qmf_filter_state_size);

  ptr_sbr_qmf->filter_states_32 =
      &sbr_qmf_synth_states_32[chan ? qmf_filter_state_size : 0];

  memset(ptr_sbr_qmf->filter_states_32, 0,
         sizeof(WORD32) * qmf_filter_state_size);

  if (audio_object_type == AOT_ER_AAC_ELD ||
      audio_object_type == AOT_ER_AAC_LD) {
    ptr_sbr_qmf->fp1_syn = ptr_sbr_qmf->filter_states;
    ptr_sbr_qmf->fp2_syn =
        ptr_sbr_qmf->filter_states + ptr_sbr_qmf->no_channels;
    ptr_sbr_qmf->sixty4 = NO_SYNTHESIS_CHANNELS;
  }

  return;
}

WORD16 ixheaacd_create_sbrdec(ixheaacd_misc_tables *pstr_common_table,
                              ia_sbr_channel_struct *ptr_sbr_channel,
                              ia_sbr_header_data_struct *ptr_header_data,
                              WORD16 chan, FLAG down_sample_flag,
                              VOID *sbr_persistent_mem_v, WORD ps_enable,
                              WORD audio_object_type, WORD32 ldmps_present,
                              WORD32 ldsbr_present)

{
  WORD16 err;
  WORD16 time_slots;
  WORD16 no_bins;
  ia_sbr_pers_struct *sbr_persistent_mem =
      (ia_sbr_pers_struct *)sbr_persistent_mem_v;
  ia_sbr_dec_struct *hs = &(ptr_sbr_channel->str_sbr_dec);

  time_slots = ptr_header_data->num_time_slots;

  no_bins = (WORD16)(time_slots * ptr_header_data->time_step);

  hs->str_sbr_scale_fact.ov_lb_scale = INT_BITS - 1;
  hs->str_sbr_scale_fact.hb_scale = INT_BITS - 1;
  hs->str_sbr_scale_fact.ov_hb_scale = INT_BITS - 1;
  hs->str_sbr_scale_fact.st_syn_scale = INT_BITS - 1;

  ptr_sbr_channel->pstr_prev_frame_data =
      sbr_persistent_mem->pstr_prev_frame_data[chan];

  err = ixheaacd_create_sbr_env_calc(pstr_common_table, &hs->str_sbr_calc_env,
                                     chan, sbr_persistent_mem, ptr_header_data,
                                     audio_object_type);

  if (err) {
    return (-1);
  }

  ixheaacd_create_cplx_anal_qmfbank(
      &hs->str_codec_qmf_bank, &hs->str_sbr_scale_fact, no_bins,
      ptr_header_data->pstr_freq_band_data->sub_band_start, chan,
      sbr_persistent_mem->sbr_qmf_analy_states,
      sbr_persistent_mem->sbr_qmf_analy_states_32,
      sbr_persistent_mem->str_sbr_dec_inst.pstr_sbr_tables->qmf_dec_tables_ptr,
      audio_object_type, ldmps_present, ldsbr_present);

  ixheaacd_create_cplx_synt_qmfbank(
      &hs->str_synthesis_qmf_bank, no_bins,
      ptr_header_data->pstr_freq_band_data->sub_band_start,
      ptr_header_data->pstr_freq_band_data->sub_band_end, chan,
      down_sample_flag, sbr_persistent_mem->sbr_qmf_synth_states,
      sbr_persistent_mem->sbr_qmf_synth_states_32,
      sbr_persistent_mem->str_sbr_dec_inst.pstr_sbr_tables->qmf_dec_tables_ptr,
      audio_object_type);

  ixheaacd_init_sbr_prev_framedata(ptr_sbr_channel->pstr_prev_frame_data,
                                   time_slots);

  ixheaacd_create_hf_generator(&hs->str_hf_generator,
                               hs->str_codec_qmf_bank.num_time_slots, chan,
                               sbr_persistent_mem, ps_enable);

  hs->ptr_sbr_overlap_buf = sbr_persistent_mem->ptr_sbr_overlap_buf[chan];

  return 0;
}
