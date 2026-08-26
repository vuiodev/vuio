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
enum _avoid_hole_state { NO_AH = 0, AH_INACTIVE = 1, AH_ACTIVE = 2 };

typedef struct {
  FLOAT32 *sfb_ld_energy;
  FLOAT32 *sfb_lines;
  FLOAT32 sfb_pe[MAXIMUM_GROUPED_SCALE_FACTOR_BAND];
  FLOAT32 sfb_const_part[MAXIMUM_GROUPED_SCALE_FACTOR_BAND];
  FLOAT32 num_sfb_active_lines[MAXIMUM_GROUPED_SCALE_FACTOR_BAND];
  FLOAT32 pe;
  FLOAT32 const_part;
  FLOAT32 num_active_lines;
} ia_qc_pe_chan_data_struct;

typedef struct {
  ia_qc_pe_chan_data_struct pe_ch_data[30];
  FLOAT32 pe;
  FLOAT32 const_part;
  FLOAT32 num_active_lines;
  FLOAT32 offset;
} ia_qc_pe_data_struct;

VOID iaace_adj_thr_init(ia_adj_thr_state_struct *pstr_adj_thr_state, const FLOAT32 mean_pe,
                        WORD32 ch_bitrate, WORD32 aot);

VOID iaace_adjust_threshold(ia_adj_thr_state_struct *pstr_adj_thr_state,
                            ia_adj_thr_elem_struct *pstr_adj_thr_elem,
                            ixheaace_psy_out_channel **pstr_psy_out,
                            FLOAT32 *ptr_ch_bit_dist, ixheaace_qc_out_element *pstr_qc_out_el,
                            const WORD32 avg_bits, const WORD32 bitres_bits,
                            const WORD32 max_bitres_bits, const WORD32 side_info_bits,
                            FLOAT32 *max_bit_fac, FLOAT32 *ptr_sfb_num_relevant_lines,
                            FLOAT32 *ptr_sfb_ld_energy, WORD32 num_chans, WORD32 chn, WORD32 aot,
                            WORD8 *ptr_scratch);
