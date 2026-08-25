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
#ifndef IXHEAACD_COMMON_ROM_H
#define IXHEAACD_COMMON_ROM_H

#define LOG_2_TABLE_SIZE 65
#define INV_TABLE_SIZE 256
#define SQRT_TABLE_SIZE 256

typedef struct {
  const WORD16 trig_data[513];
  const WORD16 sine_table8_16[8];
  const WORD16 log_dual_is_table[LOG_2_TABLE_SIZE];

  const WORD32 down_mix_martix[4][2][8];
  const WORD32 cc_gain_scale[4];

  WORD16 inv_table[INV_TABLE_SIZE];
  const WORD16 sqrt_table[SQRT_TABLE_SIZE + 1];

  WORD32 dummy;
  WORD32 start_band[10][16];
  WORD32 stop_band[10][16];
  WORD32 stop_freq_table_fs40k_2[14];
  WORD32 stop_freq_table_fs40k_4[14];
} ixheaacd_misc_tables;

extern const ixheaacd_misc_tables ixheaacd_str_fft_n_transcendent_tables;

#endif
