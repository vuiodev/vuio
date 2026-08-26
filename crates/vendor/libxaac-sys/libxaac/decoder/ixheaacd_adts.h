/******************************************************************************
 *                                                                            *
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
#ifndef IXHEAACD_ADTS_H
#define IXHEAACD_ADTS_H

typedef struct {
  WORD16 sync_word;
  WORD32 id;
  WORD32 layer;
  WORD32 protection_absent;
  WORD32 profile;
  WORD32 samp_freq_index;
  WORD32 channel_configuration;
  WORD16 aac_frame_length;
  WORD32 no_raw_data_blocks;
  WORD32 crc_check;
} ia_adts_header_struct;

WORD32 ixheaacd_find_syncword(ia_adts_header_struct *adts,
                              struct ia_bit_buf_struct *it_bit_buff);

WORD32 ixheaacd_adtsframe(ia_adts_header_struct *adts,
                          struct ia_bit_buf_struct *it_bit_buff);

WORD32 ixheaacd_check_if_adts(ia_adts_header_struct *adts,
                              struct ia_bit_buf_struct *it_bit_buff,
                              WORD32 usr_max_ch);

#endif /* IXHEAACD_ADTS_H */
