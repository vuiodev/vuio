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

#ifndef IXHEAACE_MAX_CH_IN_BS_ELE
#define IXHEAACE_MAX_CH_IN_BS_ELE (2)
#endif

#define FRAME_LEN_BYTES_MODULO (1)
#define FRAME_LEN_BYTES_INT (2)

typedef struct {
  WORD32 ch_bitrate;
  WORD32 avg_bits;
  WORD32 max_bits;
  WORD32 bit_res_lvl;
  WORD32 max_bitres_bits;
  WORD32 static_bits;
  FLOAT32 max_bit_fac;
  WORD32 tot_avg_bits;
  WORD32 padding;
  ia_adj_thr_state_struct str_adj_thr;
  ia_adj_thr_elem_struct str_adj_thr_ele;
  WORD8 num_ch;
} ia_qc_data_struct;

typedef struct {
  WORD16 quant_spec[LEN_SUPERFRAME];
  UWORD16 max_val_in_sfb[LEN_SUPERFRAME];
  WORD16 scalefactor[LEN_SUPERFRAME];
  WORD32 global_gain;
} ia_qc_out_chan_struct;

typedef struct {
  ia_qc_out_chan_struct str_qc_out_chan[IXHEAACE_MAX_CH_IN_BS_ELE];
  WORD32 static_bits;
  WORD32 dyn_bits;
  WORD32 fill_bits;
  FLOAT32 pe;
} ia_qc_out_data_struct;

typedef struct {
  ia_qc_data_struct str_qc_data[IXHEAACE_MAX_CH_IN_BS_ELE];
  ia_qc_out_data_struct str_qc_out;
} ia_qc_main_struct;

VOID iusace_qc_create(ia_qc_main_struct *pstr_qc_data);

VOID iusace_qc_init(ia_qc_data_struct *pstr_qc_data, const WORD32 max_bits, WORD32 sample_rate,
                    WORD32 bw_limit, WORD32 channels, WORD32 ccfl);

VOID iusace_adj_bitrate(ia_qc_data_struct *pstr_qc_data, WORD32 bit_rate, WORD32 sample_rate,
                        WORD32 ccfl);

WORD32 iusace_calc_max_val_in_sfb(WORD32 sfb_count, WORD32 max_sfb_per_grp, WORD32 sfb_per_group,
                                  WORD32 *ptr_sfb_offset, WORD16 *ptr_quant_spec);
