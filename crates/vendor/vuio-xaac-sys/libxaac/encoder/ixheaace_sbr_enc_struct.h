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

#pragma once

#define IXHEAACE_TABLE_IDX_NOT_FOUND (-1)
#define IXHEAACE_TABLE_IDX_FOUND (0)
#define IXHEAACE_HYBRID_BANDS (16)
#define IXHEAACE_SUBSAMPLES (32)

#define ALIGNMENT_DEFINE __attribute__((aligned(8)))

#ifndef max
#define max(a, b) (a > b ? a : b)
#endif

// 4 is for sizeof FLOAT32 data type
#define IXHEAACE_SBR_SCR_SIZE_PVC                                      \
  (((IXHEAACE_ESBR_PVC_NUM_TS * IXHEAACE_ESBR_PVC_NUM_QMF_BANDS_CORE + \
     IXHEAACE_ESBR_PVC_NUM_TS * IXHEAACE_ESBR_PVC_NUM_QMF_BANDS) *     \
    4) +                                                               \
   128)

// 4 is for sizeof FLOAT32 data type and 2 is for two-channels
#define IXHEAACE_SBR_SCR_SIZE_TES \
  ((IXHEAACE_TIMESLOT_BUFFER_SIZE * IXHEAACE_QMF_CHANNELS * 2 * 4) + 128)

#define IXHEAACE_SBR_SCR_SIZE \
  MAX(IXHEAACE_SBR_SCR_SIZE_PVC, MAX(IXHEAACE_SBR_SCR_SIZE_TES, (2 * 1024)))

typedef struct {
  WORD32 ps_buf3[IXHEAACE_HYBRID_BANDS * IXHEAACE_SUBSAMPLES * 2];
  FLOAT32
  sbr_env_r_buf[IXHEAACE_MAX_CH_IN_BS_ELE * IXHEAACE_QMF_TIME_SLOTS * IXHEAACE_QMF_CHANNELS];
  FLOAT32
  sbr_env_i_buf[IXHEAACE_MAX_CH_IN_BS_ELE * IXHEAACE_QMF_TIME_SLOTS * IXHEAACE_QMF_CHANNELS];
  FLOAT32
  sbr_env_r_buffer[IXHEAACE_MAX_CH_IN_BS_ELE * MAX_QMF_TIME_SLOTS * IXHEAACE_QMF_CHANNELS];
  FLOAT32
  sbr_env_i_buffer[IXHEAACE_MAX_CH_IN_BS_ELE * MAX_QMF_TIME_SLOTS * IXHEAACE_QMF_CHANNELS];
  WORD8 sbr_scratch[IXHEAACE_SBR_SCR_SIZE];
} ixheaace_str_sbr_enc_scratch;

struct ixheaace_str_sbr_enc {
  struct ixheaace_str_sbr_config_data str_sbr_cfg;
  struct ixheaace_str_sbr_hdr_data str_sbr_hdr;
  struct ixheaace_str_sbr_bitstream_data str_sbr_bs;
  struct ixheaace_str_enc_channel *pstr_env_channel[IXHEAACE_MAX_CH_IN_BS_ELE];
  struct ixheaace_str_common_data str_cmon_data;
  struct ixheaace_ps_enc *pstr_ps_enc;
  ixheaace_str_sbr_qmf_filter_bank *pstr_synthesis_qmf_bank;
  UWORD32 sbr_payload_prev[IXHEAACE_MAX_PAYLOAD_SIZE / (sizeof(WORD32))];
  UWORD32 sbr_payload[IXHEAACE_MAX_PAYLOAD_SIZE / (sizeof(WORD32))];
  UWORD32 sbr_payload_size;
  WORD32 *ptr_common_buffer1;
  WORD32 *ptr_common_buffer2;
  ixheaace_str_sbr_enc_scratch *ptr_sbr_enc_scr;
  ixheaace_pvc_enc *pstr_pvc_enc;
  FLOAT32 *ptr_hbe_resample_buf;
};
