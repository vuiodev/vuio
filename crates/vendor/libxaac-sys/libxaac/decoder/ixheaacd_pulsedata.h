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
#ifndef IXHEAACD_PULSEDATA_H
#define IXHEAACD_PULSEDATA_H

#define MAX_LINES 4

typedef struct {
  FLAG pulse_data_present;
  WORD16 number_pulse;
  WORD16 pulse_start_band;
  WORD8 pulse_offset[MAX_LINES];
  WORD8 pulse_amp[MAX_LINES];
} ia_pulse_info_struct;

WORD32 ixheaacd_read_pulse_data(ia_bit_buf_struct *it_bit_buff,
                                ia_pulse_info_struct *ptr_pulse_data,
                                ia_aac_dec_tables_struct *ptr_aac_tables);

VOID ixheaacd_pulse_data_apply(ia_pulse_info_struct *ptr_pulse_data,
                               WORD8 *pulse_scratch,
                               const WORD16 *ptr_swb_offset, WORD object_type);

#endif /* #ifndef IXHEAACD_PULSEDATA_H */
