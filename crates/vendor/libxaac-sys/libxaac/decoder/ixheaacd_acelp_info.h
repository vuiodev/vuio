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
#ifndef IXHEAACD_ACELP_INFO_H
#define IXHEAACD_ACELP_INFO_H

typedef struct {
  WORD32 acelp_core_mode;
  WORD32 mod[NUM_FRAMES];

  WORD32 fac[NUM_FRAMES * FAC_LENGTH];
  WORD32 fac_data[FAC_LENGTH + 1];
  WORD32 mean_energy[NUM_FRAMES];
  WORD32 acb_index[NUM_SUBFR_SUPERFRAME];
  WORD32 noise_factor[NUM_FRAMES];
  WORD32 global_gain[NUM_FRAMES];
  WORD32 arith_reset_flag;
  WORD32 x_tcx_invquant[LEN_SUPERFRAME];
  WORD32 tcx_lg[4 * NUM_FRAMES];
  WORD32 ltp_filtering_flag[NUM_SUBFR_SUPERFRAME];
  WORD32 icb_index[NUM_SUBFR_SUPERFRAME][8];
  WORD32 gains[NUM_SUBFR_SUPERFRAME];
  WORD32 mode_lpc[NUM_FRAMES];
  WORD32 lpc_first_approx_idx[110];
  FLOAT32 lsp_coeff[5][ORDER];
  FLOAT32 lsf_adaptive_mean_cand[ORDER];
  FLOAT32 lsf_adaptive_mean[ORDER];
  FLOAT32 lpc4_lsf[ORDER];
} ia_td_frame_data_struct;

typedef struct {
  WORD32 islong;
  WORD32 max_win_len;
  WORD32 samp_per_bk;
  WORD32 sfb_per_bk;
  WORD32 bins_per_sbk;
  WORD32 sfb_per_sbk;

  const WORD16 *ptr_sfb_tbl;
  pWORD16 sfb_width;
  WORD16 sfb_idx_tbl[125];
  WORD32 num_groups;
  WORD16 group_len[8];

} ia_sfb_info_struct;

#endif
