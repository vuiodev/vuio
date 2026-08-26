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

#define GLOBAL_GAIN_OFFSET 100
#define ICS_RESERVED_BIT 0
#define EXT_FIL 0
#define NUM_COUPLED_ELE 0
#define SCALE_COUPLING_LEVEL0 0

typedef struct {
  WORD32 num_channels;
  WORD32 bitrate;
  WORD32 sample_rate;
  WORD32 profile;
} ixheaace_bitstream_enc_init;

IA_ERRORCODE ia_enhaacplus_enc_write_bitstream(
    ixheaace_bit_buf_handle pstr_bit_stream_handle, ixheaace_element_info pstr_element_info,
    ixheaace_qc_out *pstr_qc_out, ixheaace_psy_out *pstr_psy_out, WORD32 *glob_used_bits,
    const UWORD8 *ptr_anc_bytes, ixheaace_aac_tables *pstr_aac_tables, FLAG flag_last_element,
    WORD32 *write_program_config_element, WORD32 i_num_coup_channels, WORD32 i_channels_mask,
    WORD32 i_samp_freq, WORD32 ele_idx, WORD32 aot, WORD32 *total_fill_bits);
