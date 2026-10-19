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
  UWORD8 *p_data;
  UWORD32 data_size;
  WORD32 data_type;
  WORD32 associated_ch_element;
} ixheaace_mps_enc_ext_payload;

IA_ERRORCODE ixheaace_mps_212_initialise(VOID *pstr_mps_enc, const WORD32 audio_object_type,
                                         const UWORD32 sampling_rate, WORD32 *ptr_bitrate,
                                         const UWORD32 sbr_ratio, const UWORD32 frame_length,
                                         const UWORD32 input_buffer_size_per_channel,
                                         const UWORD32 core_coder_delay, WORD8 *ptr_scratch);

IA_ERRORCODE
ixheaace_mps_212_process(VOID *pstr_mps_enc, FLOAT32 *const ptr_audio_samples,
                         const WORD32 num_audio_samples,
                         ixheaace_mps_enc_ext_payload *pstr_mps_ext_payload);

WORD32 ixheaace_mps_212_get_spatial_specific_config(VOID *pstr_mps_enc, WORD8 *ptr_out_buffer,
                                                    WORD32 buf_size, WORD32 aot);

VOID ixheaace_mps_212_open(VOID **pstr_mps_enc, ixheaace_mps_212_memory_struct *pstr_mps_memory);

VOID ixheaace_mps_212_close(VOID **pstr_mps_enc);

IA_ERRORCODE
ixheaace_mps_515_open(VOID **pstr_mps_enc, WORD32 sample_freq, WORD32 tree_config,
                      ixheaace_bit_buf_handle pstr_bitstream, WORD32 *ptr_bits_written,
                      ixheaace_mps_515_memory_struct *pstr_mps_memory, WORD32 flag_480);

IA_ERRORCODE ixheaace_mps_515_apply(ixheaace_mps_sac_enc *pstr_mps_enc, FLOAT32 *ptr_audio_input,
                                    FLOAT32 *ptr_audio_output,
                                    ixheaace_bit_buf_handle pstr_bitstream, VOID *pstr_scratch);

VOID ixheaace_mps_515_close(ixheaace_mps_sac_enc *pstr_mps_enc);

WORD32 ixheaace_mps_515_scratch_size(VOID);
