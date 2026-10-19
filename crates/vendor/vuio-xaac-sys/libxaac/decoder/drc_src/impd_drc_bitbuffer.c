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
#include <stdio.h>
#include <stdlib.h>
#include <math.h>
#include <assert.h>
#include <string.h>

#include "impd_type_def.h"
#include "impd_drc_bitbuffer.h"
#include "impd_drc_extr_delta_coded_info.h"
#include "impd_drc_common.h"
#include "impd_drc_struct.h"
#include "impd_drc_parser.h"

WORD32 impd_read_bits_buf(ia_bit_buf_struct* it_bit_buff, WORD no_of_bits) {
  UWORD32 ret_val;
  UWORD8* ptr_read_next = it_bit_buff->ptr_read_next;
  WORD bit_pos = it_bit_buff->bit_pos;

  if (it_bit_buff->cnt_bits <= 0) {
    it_bit_buff->error = 1;
    return -1;
  }

  if (no_of_bits == 0) {
    return 0;
  }

  it_bit_buff->cnt_bits -= no_of_bits;
  ret_val = (UWORD32)*ptr_read_next;

  bit_pos -= no_of_bits;
  while (bit_pos < 0) {
    bit_pos += 8;
    ptr_read_next++;

    if (ptr_read_next > it_bit_buff->ptr_bit_buf_end) {
      ptr_read_next = it_bit_buff->ptr_bit_buf_base;
    }

    ret_val <<= 8;

    ret_val |= (UWORD32)*ptr_read_next;
  }

  ret_val = ret_val << ((31 - no_of_bits) - bit_pos) >> (32 - no_of_bits);
  it_bit_buff->ptr_read_next = ptr_read_next;
  it_bit_buff->bit_pos = (WORD16)bit_pos;
  return ret_val;
}

WORD32 impd_skip_bits_buf(ia_bit_buf_struct* it_bit_buff, WORD no_of_bits) {
  UWORD8* ptr_read_next = it_bit_buff->ptr_read_next;
  WORD bit_pos = it_bit_buff->bit_pos;

  if (it_bit_buff->cnt_bits < no_of_bits) {
    it_bit_buff->error = 1;
    return -1;
  }

  it_bit_buff->cnt_bits -= no_of_bits;

  bit_pos -= no_of_bits;
  while (bit_pos < 0) {
    bit_pos += 8;
    ptr_read_next++;
  }
  it_bit_buff->ptr_read_next = ptr_read_next;
  it_bit_buff->bit_pos = (WORD16)bit_pos;
  return no_of_bits;
}
VOID impd_create_bit_buf(ia_bit_buf_struct* it_bit_buff,
                         UWORD8* ptr_bit_buf_base, WORD32 bit_buf_size) {
  it_bit_buff->ptr_bit_buf_base = ptr_bit_buf_base;
  it_bit_buff->ptr_bit_buf_end = ptr_bit_buf_base + bit_buf_size - 1;

  it_bit_buff->ptr_read_next = ptr_bit_buf_base;
  it_bit_buff->bit_pos = 7;

  it_bit_buff->cnt_bits = 0;
  it_bit_buff->size = bit_buf_size << 3;
  it_bit_buff->error = 0;

  return;
}

VOID impd_create_init_bit_buf(ia_bit_buf_struct* it_bit_buff,
                              UWORD8* ptr_bit_buf_base, WORD32 bit_buf_size) {
  impd_create_bit_buf(it_bit_buff, ptr_bit_buf_base, bit_buf_size);
  it_bit_buff->cnt_bits = (bit_buf_size << 3);
  return;
}

WORD32 impd_init_drc_bitstream_dec(ia_drc_bits_dec_struct* p_drc_bs_dec_struct,
                                   WORD32 sample_rate, WORD32 frame_size,
                                   WORD32 delay_mode,
                                   WORD32 lfe_channel_map_count,
                                   WORD32* lfe_channel_map) {
  WORD32 i, err_code = 0;

  ia_drc_params_bs_dec_struct* ia_drc_params_struct =
      &p_drc_bs_dec_struct->ia_drc_params_struct;
  ia_drc_params_struct->drc_frame_size = frame_size;
  if (sample_rate < MIN_DRC_SAMP_FREQ) {
    return -1;
  }
  ia_drc_params_struct->delta_tmin_default = impd_get_delta_tmin(sample_rate);
  ia_drc_params_struct->num_gain_values_max_default =
      ia_drc_params_struct->drc_frame_size /
      ia_drc_params_struct->delta_tmin_default;
  ia_drc_params_struct->delay_mode = delay_mode;

  if ((frame_size < 1) || (frame_size > AUDIO_CODEC_FRAME_SIZE_MAX) ||
      (ia_drc_params_struct->drc_frame_size < 0.001f * sample_rate)) {
    return -1;
  }

  if (ia_drc_params_struct->delta_tmin_default >
      ia_drc_params_struct->drc_frame_size) {
    return -1;
  }

  if (lfe_channel_map_count >= 0) {
    if ((lfe_channel_map == NULL) ||
        (lfe_channel_map_count > MAX_CHANNEL_COUNT)) {
      return (-1);
    }

    ia_drc_params_struct->lfe_channel_map_count = lfe_channel_map_count;

    for (i = 0; i < lfe_channel_map_count; i++) {
      ia_drc_params_struct->lfe_channel_map[i] = lfe_channel_map[i];
    }
  } else {
    ia_drc_params_struct->lfe_channel_map_count = -1;

    for (i = 0; i < MAX_CHANNEL_COUNT; i++) {
      ia_drc_params_struct->lfe_channel_map[i] = 0;
    }
  }

  impd_init_tbls(ia_drc_params_struct->num_gain_values_max_default,
                 &p_drc_bs_dec_struct->tables_default);

  return err_code;
}

WORD32 impd_process_drc_bitstream_dec_config(
    ia_drc_bits_dec_struct* p_drc_bs_dec_struct, ia_bit_buf_struct* it_bit_buff,
    ia_drc_config* pstr_drc_config, UWORD8* bitstream_config,
    WORD32 num_bytes) {
  WORD32 err_code = 0;

  impd_create_init_bit_buf(it_bit_buff, bitstream_config, num_bytes);

  err_code = impd_parse_drc_config(
      it_bit_buff, &p_drc_bs_dec_struct->ia_drc_params_struct, pstr_drc_config);
  if (err_code) return (err_code);

  return err_code;
}

WORD32 impd_process_drc_bitstream_dec_gain(
    ia_drc_bits_dec_struct* p_drc_bs_dec_struct, ia_bit_buf_struct* it_bit_buff,
    ia_drc_config* pstr_drc_config, ia_drc_gain_struct* pstr_drc_gain,
    UWORD8* bitstream_gain, WORD32 num_bytes, WORD32 num_bits_offset,
    WORD32* num_bits_read) {
  WORD32 err_code = 0;

  impd_create_init_bit_buf(it_bit_buff, bitstream_gain, num_bytes);

  impd_read_bits_buf(it_bit_buff, num_bits_offset);
  if (it_bit_buff->error) return it_bit_buff->error;

  err_code = impd_drc_uni_gain_read(it_bit_buff, p_drc_bs_dec_struct,
                                    pstr_drc_config, pstr_drc_gain);

  if (err_code > PROC_COMPLETE) return (err_code);

  *num_bits_read = (it_bit_buff->size) - it_bit_buff->cnt_bits;

  if (err_code == PROC_COMPLETE) {
    return err_code;
  }

  return 0;
}

WORD32 impd_process_drc_bitstream_dec_loudness_info_set(
    ia_bit_buf_struct* it_bit_buff,
    ia_drc_loudness_info_set_struct* pstr_loudness_info,
    UWORD8* bit_stream_loudness, WORD32 num_bytes_loudness) {
  WORD32 err_code = 0;

  impd_create_init_bit_buf(it_bit_buff, bit_stream_loudness,
                           num_bytes_loudness);

  err_code = impd_parse_loudness_info_set(it_bit_buff, pstr_loudness_info);
  return err_code;
}
