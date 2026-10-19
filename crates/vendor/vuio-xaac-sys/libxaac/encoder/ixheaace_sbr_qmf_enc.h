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

#define IXHEAACE_NUM_QMF_SYNTH_CHANNELS (64)
#define IXHEAACE_TIMESLOT_BUFFER_SIZE (78)

#define IXHEAACE_PS_BUF4_SIZE                                              \
  (sizeof(WORD32) * (IXHEAACE_QMF_TIME_SLOTS + IXHEAACE_QMF_BUFFER_MOVE) + \
   sizeof(WORD32 *) * (IXHEAACE_NUM_QMF_BANDS_IN_HYBRID +                  \
                       IXHEAACE_NUM_QMF_BANDS_IN_HYBRID * IXHEAACE_QMF_BUFFER_MOVE))

typedef struct {
  const FLOAT32 *ptr_flt_filter;
  const FLOAT32 *ptr_flt_cos_twiddle;
  const FLOAT32 *ptr_flt_alt_sin_twiddle;
  FLOAT32 *ptr_flt_time_buf;
  FLOAT32 *ptr_flt_work_buf;
  WORD16 flag;
  SIZE_T offset_l;
  SIZE_T offset_r;
  WORD16 offset;
  const FLOAT32 *ptr_filter;
  const FLOAT32 *ptr_cos_twiddle;
  const FLOAT32 *ptr_sin_twiddle;
  const FLOAT32 *ptr_alt_sin_twiddle;
  FLOAT32 *ptr_work_buf;
  FLOAT32 *ptr_sbr_qmf_states_ana;
  FLOAT32 *ptr_qmf_states_buf;
  FLOAT32 *ptr_qmf_states_curr_pos;
  const FLOAT32 *ptr_ref_coeff_l;
  const FLOAT32 *ptr_ref_coeff_r;
  const FLOAT32 *ptr_cld_filt;
  FLOAT32 *ptr_fp1;
  FLOAT32 *ptr_fp2;
  WORD32 start_coeff_cnt;
  WORD32 num_time_slots;
  WORD32 rate;
} ixheaace_str_sbr_qmf_filter_bank;

typedef ixheaace_str_sbr_qmf_filter_bank *ixheaace_pstr_sbr_qmf_filter_bank;

VOID ixheaace_sbr_analysis_filtering(const FLOAT32 *ptr_time_in, WORD32 time_sn_stride,
                                     FLOAT32 **ptr_ana_r, FLOAT32 **ptr_ana_i,
                                     ixheaace_pstr_sbr_qmf_filter_bank pstr_qmf_bank,
                                     ixheaace_str_qmf_tabs *pstr_qmf_tab, WORD32 num_qmf_subsamp,
                                     WORD32 is_ld_sbr, FLOAT32 *ptr_sbr_scratch,
                                     WORD32 is_ps_960);

VOID ixheaace_create_qmf_bank(ixheaace_pstr_sbr_qmf_filter_bank pstr_sbr_qmf_handle,
                              ixheaace_str_sbr_tabs *pstr_sbr_tab, WORD32 is_ld_sbr);

VOID ixheaace_get_energy_from_cplx_qmf(
    FLOAT32 **ptr_energy_vals, FLOAT32 **ptr_real_values, FLOAT32 **ptr_imag_values,
    WORD32 is_ld_sbr, WORD32 num_time_slots, WORD32 samp_ratio_fac,
    ixheaace_str_hbe_enc *pstr_hbe_enc, WORD32 op_delay, WORD32 harmonic_sbr);

VOID ixheaace_enc_synthesis_qmf_filtering(FLOAT32 **ptr_sbr_re, FLOAT32 **ptr_sbr_im,
                                          FLOAT32 *time_float,
                                          ixheaace_pstr_sbr_qmf_filter_bank pstr_qmf_bank);

VOID ixheaace_create_synthesis_qmf_bank(
    ixheaace_pstr_sbr_qmf_filter_bank pstr_sbr_qmf_filter_bank, WORD32 *ptr_common_buffer,
    ixheaace_str_sbr_tabs *pstr_sbr_tab);
