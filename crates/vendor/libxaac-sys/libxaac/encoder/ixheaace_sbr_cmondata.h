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

struct ixheaace_str_common_data {
  WORD32 sbr_hdr_bits;
  WORD32 sbr_crc_len;
  WORD32 sbr_data_bits;
  WORD32 sbr_fill_bits;
  ixheaace_bit_buf str_sbr_bit_buf;
  ixheaace_bit_buf str_tmp_write_bit_buf;
  WORD32 sbr_num_channels;
  ixheaace_bit_buf str_sbr_bit_buf_prev;
  WORD32 prev_bit_buf_write_offset;
  WORD32 prev_bit_buf_read_offset;
};

typedef struct ixheaace_str_common_data *ixheaace_pstr_common_data;
