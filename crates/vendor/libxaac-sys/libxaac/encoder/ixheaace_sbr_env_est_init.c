/******************************************************************************
 *                                                                            *
 * Copyright (C) 2023 The Android Open Source Project
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
#include <limits.h>

#include "ixheaac_type_def.h"
#include "ixheaac_constants.h"
#include "ixheaac_error_standards.h"
#include "ixheaace_error_codes.h"
#include "ixheaace_aac_constants.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops16.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaac_basic_ops.h"
#include "ixheaace_sbr_header.h"
#include "ixheaace_sbr_def.h"
#include "ixheaace_resampler.h"
#include "ixheaace_sbr_rom.h"
#include "ixheaace_common_rom.h"
#include "ixheaace_sbr_hbe.h"
#include "ixheaace_sbr_qmf_enc.h"
#include "ixheaace_sbr_tran_det.h"
#include "ixheaace_sbr_frame_info_gen.h"
#include "ixheaace_sbr_env_est.h"
#include "ixheaace_sbr_code_envelope.h"
#include "ixheaace_sbr_main.h"
#include "ixheaace_sbr_missing_harmonics_det.h"
#include "ixheaace_sbr_inv_filtering_estimation.h"
#include "ixheaace_sbr_noise_floor_est.h"

#include "ixheaace_common_rom.h"
#include "ixheaace_sbr_ton_corr.h"
#include "iusace_esbr_pvc.h"
#include "iusace_esbr_inter_tes.h"
#include "ixheaace_sbr.h"
#include "ixheaace_bitbuffer.h"
#include "ixheaace_sbr_cmondata.h"
#include "ixheaace_sbr_write_bitstream.h"
#include "ixheaace_sbr_hybrid.h"
#include "ixheaace_sbr_ps_enc.h"

IA_ERRORCODE
ixheaace_create_extract_sbr_envelope(WORD32 ch,
                                     ixheaace_pstr_sbr_extract_envelope pstr_sbr_ext_env,
                                     WORD32 start_index, WORD32 *ptr_common_buffer2,
                                     FLOAT32 *ptr_sbr_env_r_buf, FLOAT32 *ptr_sbr_env_i_buf,
                                     WORD32 frame_flag_480, ixheaace_sbr_codec_type sbr_codec) {
  WORD32 i;
  WORD32 y_buffer_length, r_buffer_length;
  WORD32 offset = 0;
  WORD32 y_buffer_write_offset = 32;
  WORD32 no_cols = 32;
  WORD32 time_slots = 16;
  WORD32 sbr_ratio_idx = pstr_sbr_ext_env->sbr_ratio_idx;
  WORD32 qmf_time_slots = IXHEAACE_QMF_TIME_SLOTS;
  FLOAT32 *ptr_buffer = NULL;
  FLOAT32 *ptr_i_buffer = NULL;
  memset(pstr_sbr_ext_env, 0, sizeof(ixheaace_str_sbr_extr_env));

  pstr_sbr_ext_env->pre_transient_info[0] = 0;
  pstr_sbr_ext_env->pre_transient_info[1] = 0;

  if (sbr_codec == ELD_SBR) {
    if (frame_flag_480) {
      no_cols = 30;
      time_slots = 15;
    }

    y_buffer_write_offset = time_slots / 2;
    pstr_sbr_ext_env->y_buffer_write_offset = y_buffer_write_offset;
    pstr_sbr_ext_env->no_cols = no_cols;
    pstr_sbr_ext_env->no_rows = 64;
    pstr_sbr_ext_env->start_index = start_index;
    pstr_sbr_ext_env->time_slots = time_slots;
    pstr_sbr_ext_env->time_step = 1;

    y_buffer_length = y_buffer_write_offset + time_slots;
    r_buffer_length = time_slots;
  } else {
    if ((sbr_codec == USAC_SBR) && (USAC_SBR_RATIO_INDEX_4_1 == sbr_ratio_idx)) {
      qmf_time_slots = QMF_TIME_SLOTS_USAC_4_1;
      y_buffer_write_offset = QMF_TIME_SLOTS_USAC_4_1;
      no_cols = qmf_time_slots;
    }

    pstr_sbr_ext_env->y_buffer_write_offset = y_buffer_write_offset;

    y_buffer_length = pstr_sbr_ext_env->y_buffer_write_offset + y_buffer_write_offset;

    r_buffer_length = y_buffer_write_offset;

    pstr_sbr_ext_env->pre_transient_info[0] = 0;
    pstr_sbr_ext_env->pre_transient_info[1] = 0;

    pstr_sbr_ext_env->no_cols = no_cols;
    pstr_sbr_ext_env->no_rows = 64;
    pstr_sbr_ext_env->start_index = start_index;

    pstr_sbr_ext_env->time_slots = time_slots;
    pstr_sbr_ext_env->time_step = no_cols / time_slots;

    if ((r_buffer_length != qmf_time_slots) || (y_buffer_length != 2 * qmf_time_slots)) {
      return IA_EXHEAACE_INIT_FATAL_SBR_INVALID_BUFFER_LENGTH;
    }

    y_buffer_length /= 2;

    if ((sbr_codec == USAC_SBR) && (USAC_SBR_RATIO_INDEX_4_1 == sbr_ratio_idx)) {
      pstr_sbr_ext_env->y_buffer_write_offset /= 4;
    } else {
      pstr_sbr_ext_env->y_buffer_write_offset /= 2;
    }
  }

  pstr_sbr_ext_env->buffer_flag = 0;

  ptr_buffer =
      (FLOAT32 *)&ptr_common_buffer2[ch * y_buffer_length * 64 + offset * IXHEAACE_QMF_CHANNELS];

  i = 0;
  while (i < pstr_sbr_ext_env->y_buffer_write_offset) {
    pstr_sbr_ext_env->ptr_y_buffer[i] = ptr_buffer + i * IXHEAACE_QMF_CHANNELS;
    memset(pstr_sbr_ext_env->ptr_y_buffer[i], 0,
           IXHEAACE_QMF_CHANNELS * sizeof(pstr_sbr_ext_env->ptr_y_buffer[0]));
    i++;
  }

  ptr_buffer =
      (FLOAT32 *)&ptr_common_buffer2[ch * y_buffer_length * 64 - offset * IXHEAACE_QMF_CHANNELS];
  for (i = pstr_sbr_ext_env->y_buffer_write_offset; i < y_buffer_length; i++) {
    pstr_sbr_ext_env->ptr_y_buffer[i] = ptr_buffer + i * IXHEAACE_QMF_CHANNELS;
    memset(pstr_sbr_ext_env->ptr_y_buffer[i], 0,
           IXHEAACE_QMF_CHANNELS * sizeof(pstr_sbr_ext_env->ptr_y_buffer[0]));
  }

  ptr_buffer = &ptr_sbr_env_r_buf[ch * qmf_time_slots * IXHEAACE_QMF_CHANNELS];
  ptr_i_buffer = &ptr_sbr_env_i_buf[ch * qmf_time_slots * IXHEAACE_QMF_CHANNELS];
  for (i = 0; i < r_buffer_length; i++) {
    pstr_sbr_ext_env->ptr_r_buffer[i] = ptr_buffer + i * IXHEAACE_QMF_CHANNELS;
    memset(pstr_sbr_ext_env->ptr_r_buffer[i], 0,
           IXHEAACE_QMF_CHANNELS * sizeof(pstr_sbr_ext_env->ptr_r_buffer[0]));
    pstr_sbr_ext_env->ptr_i_buffer[i] = ptr_i_buffer + i * IXHEAACE_QMF_CHANNELS;
    memset(pstr_sbr_ext_env->ptr_i_buffer[i], 0,
           IXHEAACE_QMF_CHANNELS * sizeof(pstr_sbr_ext_env->ptr_i_buffer[0]));
  }

  memset(pstr_sbr_ext_env->envelope_compensation, 0,
         sizeof(pstr_sbr_ext_env->envelope_compensation[0]) * MAXIMUM_FREQ_COEFFS);

  return IA_NO_ERROR;
}
