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
#define MAX_DRC_GAIN_BAND_COUNT (50)
#define MAX_DRC_FRAME_SIZE (4096)
#define DRC_OUT_BITBUFFER_SIZE (4096)

typedef struct {
  ia_drc_enc_params_struct str_enc_params;
  ia_drc_uni_drc_config_struct str_uni_drc_config;
  ia_drc_loudness_info_set_struct str_enc_loudness_info_set;
  ia_drc_uni_drc_gain_ext_struct str_enc_gain_extension;

  ia_drc_gain_enc_struct str_gain_enc;
  UWORD8 bit_buf_base_cfg[MAX_DRC_PAYLOAD_BYTES];
  ia_bit_buf_struct str_bit_buf_cfg;
  WORD32 drc_config_data_size_bit;
  UWORD8 bit_buf_base_cfg_ext[MAX_DRC_PAYLOAD_BYTES];
  ia_bit_buf_struct str_bit_buf_cfg_ext;
  WORD32 drc_config_ext_data_size_bit;
  UWORD8 bit_buf_base_cfg_tmp[MAX_DRC_PAYLOAD_BYTES];
  ia_bit_buf_struct str_bit_buf_cfg_tmp;

  UWORD8 drc_payload_data[MAX_DRC_PAYLOAD_BYTES];
  FLOAT32 gain_buffer[MAX_DRC_GAIN_BAND_COUNT][MAX_DRC_FRAME_SIZE];

  UWORD8 bit_buf_base_out[DRC_OUT_BITBUFFER_SIZE];
  ia_bit_buf_struct str_bit_buf_out;
  UWORD8 is_first_drc_process_complete;

  VOID *drc_scratch_mem;
  WORD32 drc_scratch_used;
} ia_drc_enc_state;
