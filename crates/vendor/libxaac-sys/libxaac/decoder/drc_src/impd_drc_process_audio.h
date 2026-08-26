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
#ifndef IMPD_DRC_PROCESS_AUDIO_H
#define IMPD_DRC_PROCESS_AUDIO_H

#ifdef __cplusplus
extern "C" {
#endif

typedef struct {
  WORD32 multiband_audio_sig_count;
  WORD32 frame_size;
  FLOAT32** non_interleaved_audio;
} ia_audio_band_buffer_struct;

typedef struct {
  WORD32 audio_num_chan;
  WORD32 frame_size;
  WORD32 audio_sub_band_count;
  WORD32 audio_sub_band_frame_size;
  WORD32 audio_delay_samples;
  WORD32 audio_delay_sub_band_samples;
  FLOAT32** audio_io_buffer_delayed;
  FLOAT32** audio_buffer_delayed_real;
  FLOAT32** audio_buffer_delayed_imag;
  FLOAT32** audio_in_out_buf;
  FLOAT32** audio_real_buff;
  FLOAT32** audio_imag_buff;
} ia_audio_in_out_buf;

WORD32
impd_filter_banks_process(ia_drc_instructions_struct* pstr_drc_instruction_arr,
                          const WORD32 drc_instructions_index,
                          ia_drc_params_struct* ia_drc_params_struct,
                          FLOAT32* audio_io_buf[],
                          ia_audio_band_buffer_struct* audio_band_buffer,
                          ia_filter_banks_struct* ia_filter_banks_struct,
                          const WORD32 passThru);

VOID impd_store_audio_io_buffer_time(
    FLOAT32* audio_in_out_buf[], ia_audio_in_out_buf* audio_io_buf_internal);

VOID impd_store_audio_io_buffer_freq(
    FLOAT32* audio_real_buff[], FLOAT32* audio_imag_buff[],
    ia_audio_in_out_buf* audio_io_buf_internal);

VOID impd_retrieve_audio_io_buffer_time(
    FLOAT32* audio_in_out_buf[], ia_audio_in_out_buf* audio_io_buf_internal);

VOID impd_retrieve_audio_buffer_freq(
    FLOAT32* audio_real_buff[], FLOAT32* audio_imag_buff[],
    ia_audio_in_out_buf* audio_io_buf_internal);

VOID impd_advance_audio_io_buffer_time(
    ia_audio_in_out_buf* audio_io_buf_internal);

#ifdef __cplusplus
}
#endif
#endif
