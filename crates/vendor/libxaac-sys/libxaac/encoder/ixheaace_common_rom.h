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
  const WORD32 sqrt_tab[513];
  const WORD32 sine_array[3];
  const WORD32 cosine_array[3];
  const FLOAT32 sin_arr[3];
  const FLOAT32 cos_arr[3];
  const WORD32 power_of_2_table_pos[256];
  const WORD32 power_of_2_table_neg[256];
  const WORD32 log_natural_Q25[128];
  const WORD32 sfb_width_pow_point_25_Q28[25];
  const short ia_enhaacplus_enc_w1024[768];
} ixheaace_common_tables;

extern const ixheaace_common_tables ia_enhaacplus_enc_common_tab;

typedef struct {
  ixheaace_common_tables *pstr_common_tab;
} ixheaace_comm_tables;

extern const FLOAT64 ia_enhaacplus_enc_twiddle_table_3pr[1155];

extern const FLOAT64 ia_enhaacplus_enc_twiddle_table_3pi[1155];

extern const FLOAT64 ia_enhaacplus_enc_twiddle_table_fft_32x32[514];

extern const WORD32 ia_enhaacplus_enc_fft240_table1[240];

extern const WORD32 ia_enhaacplus_enc_fft240_table2[240];

extern const UWORD32 ia_sampl_freq_table[16];

extern const UWORD32 ia_usac_sampl_freq_table[32];
