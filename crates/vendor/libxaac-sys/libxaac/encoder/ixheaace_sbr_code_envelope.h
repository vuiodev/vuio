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
#define COUPLING_1 (1)

typedef struct {
  WORD32 offset;
  WORD32 update;
  WORD32 num_scf[2];
  WORD32 sfb_nrg_prev[MAXIMUM_FREQ_COEFFS];
  WORD32 delta_t_across_frames;
  FLOAT32 df_edge_1st_env;
  FLOAT32 df_edge_incr;
  WORD32 df_edge_incr_fac;

  WORD32 code_book_scf_lav_time;
  WORD32 code_book_scf_lav_freq;

  WORD32 code_book_scf_lav_lvl_time;
  WORD32 code_book_scf_lav_lvl_freq;
  WORD32 code_book_scf_lav_bal_time;
  WORD32 code_book_scf_lav_bal_freq;

  WORD32 start_bits;
  WORD32 start_bits_balance;

  const UWORD8 *ptr_huff_tab_time_l;
  const UWORD8 *ptr_huff_tab_freq_l;

  const UWORD8 *ptr_huff_tab_lvl_time_l;
  const UWORD8 *ptr_huff_tab_bal_time_l;
  const UWORD8 *ptr_huff_tab_lvl_freq_l;
  const UWORD8 *ptr_huff_tab_bal_freq_l;
} ixheaace_str_sbr_code_envelope;

typedef ixheaace_str_sbr_code_envelope *ixheaace_pstr_sbr_code_envelope;
struct ixheaace_str_sbr_env_data;

IA_ERRORCODE ixheaace_code_envelope(WORD32 *ptr_sfb_energy, const ixheaace_freq_res *freq_res,
                                    ixheaace_str_sbr_code_envelope *pstr_code_env,
                                    WORD32 *ptr_dir_vec, WORD32 coupling, WORD32 num_envelopes,
                                    WORD32 channel, WORD32 header_active, WORD32 usac_indep_flag,
                                    WORD32 is_ld_sbr);

VOID ixheaace_create_sbr_code_envelope(ixheaace_pstr_sbr_code_envelope pstr_code_env,
                                       WORD32 *num_sfb, WORD32 delta_t_across_frames,
                                       FLOAT32 df_edge_first_env, FLOAT32 df_edge_incr);

IA_ERRORCODE
ixheaace_init_sbr_huffman_tabs(struct ixheaace_str_sbr_env_data *pstr_sbr_env,
                               ixheaace_pstr_sbr_code_envelope pstr_code_env,
                               ixheaace_pstr_sbr_code_envelope pstr_noise,
                               ixheaace_amp_res amp_res,
                               ixheaace_str_sbr_huff_tabs *pstr_sbr_huff_tabs);

VOID ixheaace_map_low_res_energy_value(WORD32 curr_val, WORD32 *ptr_prev_data, WORD32 offset,
                                       WORD32 index, ixheaace_freq_res res);

IA_ERRORCODE
ixheaace_compute_bits(WORD32 delta, WORD32 code_book_scf_lav_lvl,
                      WORD32 code_book_scf_lav_balance, const UWORD8 *ptr_huff_tbl_lvl,
                      const UWORD8 *ptr_huff_tbl_bal, WORD32 coupling, WORD32 ch,
                      WORD32 *ptr_delta_bits);
