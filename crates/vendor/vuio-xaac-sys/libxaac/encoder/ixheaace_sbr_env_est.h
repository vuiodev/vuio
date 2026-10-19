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

#define M_32 (32)
#define M_16 (16)
#define M_12 (12)
#define IXHEAACE_OP_DELAY_OFFSET (6)

typedef struct {
  WORD32 pre_transient_info[2];
  FLOAT32 *ptr_r_buffer[MAX_QMF_TIME_SLOTS];
  FLOAT32 *ptr_i_buffer[MAX_QMF_TIME_SLOTS];
  FLOAT32 *ptr_y_buffer[MAX_QMF_TIME_SLOTS * 2];
  WORD8 envelope_compensation[MAXIMUM_FREQ_COEFFS];
  WORD32 y_buffer_write_offset;
  WORD32 no_cols;
  WORD32 no_rows;
  WORD32 start_index;
  WORD32 time_slots;
  WORD32 time_step;
  WORD32 buffer_flag;
  WORD32 sbr_ratio_idx;
} ixheaace_str_sbr_extr_env;
typedef ixheaace_str_sbr_extr_env *ixheaace_pstr_sbr_extract_envelope;

WORD32
ixheaace_create_extract_sbr_envelope(WORD32 ch,
                                     ixheaace_pstr_sbr_extract_envelope pstr_sbr_ext_env,
                                     WORD32 start_index, WORD32 *ptr_common_buffer2,
                                     FLOAT32 *ptr_sbr_env_r_buf, FLOAT32 *ptr_sbr_env_i_buf,
                                     WORD32 frame_flag_480, ixheaace_sbr_codec_type sbr_codec);

struct ixheaace_str_sbr_config_data;
struct ixheaace_str_sbr_bitstream_data;
struct ixheaace_str_enc_channel;
struct ixheaace_str_common_data;
struct ixheaace_ps_enc;

VOID iusace_complex_fft_2048(FLOAT32 *ptr_x, FLOAT32 *ptr_scratch_fft);
VOID iusace_complex_fft_4096(FLOAT32 *ptr_x_r, FLOAT32 *ptr_x_i, FLOAT32 *ptr_scratch_fft);
