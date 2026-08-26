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
#ifndef IXHEAACD_MPS_AAC_STRUCT_H
#define IXHEAACD_MPS_AAC_STRUCT_H

#include "ixheaacd_defines.h"

#define PNS_BAND_FLAGS_SIZE 16
#define MAX_WINDOWS 8
#define MAX_SFB_SHORT 16
#define MAXIMUM_LINES 4

typedef struct {
  WORD16 a_scale_factor[MAX_WINDOWS * MAX_SFB_SHORT];
  WORD8 a_code_book[MAX_WINDOWS * MAX_SFB_SHORT];
} ia_mps_dec_residual_dynamic_data_struct;

typedef struct {
  WORD16 window_sequence;
  WORD16 max_sf_bands;
  WORD16 total_sf_bands;
  WORD16 sampling_rate_index;
  WORD16 window_groups;
  WORD8 window_group_length[8];
  WORD16 frame_length;

} ia_mps_dec_residual_ics_info_struct;

typedef struct {
  WORD16 start_band;
  WORD16 stop_band;
  WORD8 direction;
  WORD8 resolution;
  WORD8 order;
  WORD8 coeff[MAX_ORDER];
} ia_mps_dec_residual_filter_struct;

typedef struct {
  FLAG tns_data_present;
  WORD8 number_of_filters[MAX_WINDOWS];
  ia_mps_dec_residual_filter_struct filter[MAX_WINDOWS][MAX_FILTERS];
} ia_mps_dec_residual_tns_data;

typedef struct {
  FLAG pulse_data_present;
  WORD16 number_pulse;
  WORD16 pulse_start_band;
  WORD8 pulse_offset[MAXIMUM_LINES];
  WORD8 pulse_amp[MAXIMUM_LINES];
} ia_mps_dec_residual_pulse_data_struct;

typedef struct {
  UWORD8 pns_used[PNS_BAND_FLAGS_SIZE * 8];
  WORD16 current_energy;
  UWORD16 pns_active;
} ia_mps_dec_residual_pns_data_struct;

typedef struct {
  WORD16 *p_scale_factor;
  WORD8 *p_code_book;
  WORD32 *p_spectral_coefficient;
  ia_mps_dec_residual_ics_info_struct ics_info;
  ia_mps_dec_residual_tns_data tns_data;
  ia_mps_dec_residual_pulse_data_struct pulse_data;
  ia_mps_dec_residual_pns_data_struct pns_data;
  WORD16 common_window;
  WORD16 global_gain;
  WORD32 *p_tns_scratch;
} ia_mps_dec_residual_channel_info_struct;

typedef struct {
  WORD16 sfb_long_idx[52];
  WORD16 sfb_short_idx[16];

} ia_mps_dec_residual_sfband_info_struct;

#endif /* IXHEAACD_MPS_AAC_STRUCT_H */
