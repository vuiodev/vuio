/******************************************************************************
 *
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
#include "ixheaac_type_def.h"
#include "ixheaacd_bitbuffer.h"
#include "ixheaacd_mps_aac_struct.h"
#include "ixheaacd_mps_res_rom.h"
#include "ixheaacd_error_codes.h"

WORD32 ixheaacd_res_c_pulse_data_read(ia_bit_buf_struct *it_bit_buf,
                                      ia_mps_dec_residual_pulse_data_struct *pulse_data,
                                      ia_mps_dec_residual_aac_tables_struct *aac_tables_ptr) {
  WORD32 i, k;
  WORD32 error = 0;
  pulse_data->pulse_data_present = (FLAG)ixheaacd_read_bits_buf(it_bit_buf, 1);
  if (pulse_data->pulse_data_present) {
    WORD32 tmp = ixheaacd_read_bits_buf(it_bit_buf, 8);
    pulse_data->number_pulse = tmp >> 6;
    pulse_data->pulse_start_band = tmp & 0x3F;

    if (pulse_data->pulse_start_band >= 52) {
      return (WORD32)IA_XHEAAC_DEC_EXE_NONFATAL_PULSEDATA_ERROR;
    }

    k = aac_tables_ptr->sfb_index_long[pulse_data->pulse_start_band];

    for (i = 0; i <= pulse_data->number_pulse; i++) {
      WORD32 tmp = ixheaacd_read_bits_buf(it_bit_buf, 9);
      pulse_data->pulse_offset[i] = tmp >> 4;
      pulse_data->pulse_amp[i] = tmp & 0xF;

      k += pulse_data->pulse_offset[i];
      if (k >= 1024) error = (WORD32)IA_XHEAAC_DEC_EXE_NONFATAL_PULSEDATA_ERROR;
    }
  }

  return error;
}
