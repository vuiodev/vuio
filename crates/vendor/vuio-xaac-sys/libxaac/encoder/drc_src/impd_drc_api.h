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
typedef struct {
  WORD32 frame_size;
  WORD32 sample_rate;
  WORD32 delay_mode;
  WORD32 domain;
  WORD32 parametric_drc_only;
  WORD32 frame_count;
  WORD32 gain_sequence_present;
} ia_drc_enc_params_struct;

typedef struct {
  ia_drc_enc_params_struct str_enc_params;
  ia_drc_uni_drc_config_struct str_uni_drc_config;
  ia_drc_loudness_info_set_struct str_enc_loudness_info_set;
  ia_drc_uni_drc_gain_ext_struct str_enc_gain_extension;
} ia_drc_input_config;

typedef struct {
  ia_drc_enc_params_struct str_enc_params;
  ia_drc_uni_drc_config_struct str_uni_drc_config;
  ia_drc_loudness_info_set_struct str_enc_loudness_info_set;
  ia_drc_uni_drc_gain_ext_struct str_enc_gain_extension;
} ia_drc_internal_config;

IA_ERRORCODE impd_drc_enc_init(VOID *pstr_drc_state, VOID *ptr_drc_scratch,
                               ia_drc_input_config *pstr_inp_config);

IA_ERRORCODE impd_drc_enc(VOID *pstr_drc_state, FLOAT32 **pptr_input, UWORD32 inp_offset,
                          WORD32 *ptr_bits_written, VOID *pstr_scratch);

IA_ERRORCODE impd_loudness_info_init(VOID *pstr_drc_state, ia_drc_input_config *pstr_inp_config);

IA_ERRORCODE impd_drc_validate_config_params(ia_drc_input_config *pstr_inp_config);