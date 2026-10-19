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
#include <stdio.h>
#include <string.h>
#include "ixheaacd_sbr_common.h"
#include "ixheaac_type_def.h"

#include "ixheaac_constants.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops16.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaac_basic_ops.h"

#include "ixheaac_basic_op.h"
#include "ixheaacd_intrinsics.h"
#include "ixheaacd_common_rom.h"
#include "ixheaacd_sbrdecsettings.h"
#include "ixheaacd_bitbuffer.h"
#include "ixheaacd_defines.h"

#include "ixheaacd_pns.h"

#include "ixheaacd_aac_rom.h"
#include "ixheaacd_aac_imdct.h"
#include "ixheaacd_pulsedata.h"

#include "ixheaacd_drc_data_struct.h"
#include "ixheaacd_channelinfo.h"
#include "ixheaacd_drc_dec.h"

#include "ixheaacd_sbrdecoder.h"
#include "ixheaacd_tns.h"
#include "ixheaacd_sbr_scale.h"
#include "ixheaacd_lpp_tran.h"
#include "ixheaacd_env_extr_part.h"
#include "ixheaacd_sbr_rom.h"
#include "ixheaacd_block.h"
#include "ixheaacd_hybrid.h"
#include "ixheaacd_ps_dec.h"
#include "ixheaacd_env_extr.h"
#include "ixheaacd_basic_funcs.h"
#include "ixheaacd_env_calc.h"

WORD32 (*ixheaacd_fix_div)(WORD32, WORD32) = &ixheaacd_fix_div_armv7;

VOID(*ixheaacd_covariance_matrix_calc)
(WORD32 *, ia_lpp_trans_cov_matrix *,
 WORD32) = &ixheaacd_covariance_matrix_calc_armv7;

VOID(*ixheaacd_covariance_matrix_calc_2)
(ia_lpp_trans_cov_matrix *, WORD32 *, WORD32,
 WORD16) = &ixheaacd_covariance_matrix_calc_2_armv7;

VOID(*ixheaacd_over_lap_add1)
(WORD32 *, WORD32 *, WORD32 *, const WORD16 *, WORD16, WORD16,
 WORD16) = &ixheaacd_over_lap_add1_dec;

VOID(*ixheaacd_over_lap_add2)
(WORD32 *, WORD32 *, WORD32 *, const WORD16 *, WORD16, WORD16,
 WORD16) = &ixheaacd_over_lap_add2_dec;

VOID(*ixheaacd_decorr_filter2)
(ia_ps_dec_struct *ptr_ps_dec, WORD32 *p_buf_left_real, WORD32 *p_buf_left_imag,
 WORD32 *p_buf_right_real, WORD32 *p_buf_right_imag,
 ia_ps_tables_struct *ps_tables_ptr,
 WORD16 *transient_ratio) = &ixheaacd_decorr_filter2_armv7;

VOID(*ixheaacd_decorr_filter1)
(ia_ps_dec_struct *ptr_ps_dec, ia_ps_tables_struct *ps_tables_ptr,
 WORD16 *transient_ratio) = &ixheaacd_decorr_filter1_armv7;

WORD32(*ixheaacd_divide16_pos)
(WORD32 op1, WORD32 op2) = &ixheaacd_divide16_pos_armv7;

VOID(*ixheaacd_decorrelation)
(ia_ps_dec_struct *ptr_ps_dec, WORD32 *p_buf_left_real, WORD32 *p_buf_left_imag,
 WORD32 *p_buf_right_real, WORD32 *p_buf_right_imag,
 ia_ps_tables_struct *ps_tables_ptr) = &ixheaacd_decorrelation_armv7;

VOID(*ixheaacd_apply_rot)
(ia_ps_dec_struct *ptr_ps_dec, WORD32 *p_qmf_left_re, WORD32 *p_qmf_left_im,
 WORD32 *p_qmf_right_re, WORD32 *p_qmf_right_im,
 ia_sbr_tables_struct *sbr_tables_ptr,
 const WORD16 *ptr_res) = &ixheaacd_apply_rot_armv7;

VOID(*ixheaacd_conv_ergtoamplitudelp)
(WORD32 bands, WORD16 noise_e, WORD16 *nrg_sine, WORD16 *nrg_gain,
 WORD16 *noise_level_mant,
 WORD16 *sqrt_table) = &ixheaacd_conv_ergtoamplitudelp_armv7;

VOID(*ixheaacd_conv_ergtoamplitude)
(WORD32 bands, WORD16 noise_e, WORD16 *nrg_sine, WORD16 *nrg_gain,
 WORD16 *noise_level_mant,
 WORD16 *sqrt_table) = &ixheaacd_conv_ergtoamplitude_armv7;

VOID(*ixheaacd_adjust_scale)
(WORD32 **re, WORD32 **im, WORD32 sub_band_start, WORD32 sub_band_end,
 WORD32 start_pos, WORD32 next_pos, WORD32 shift,
 FLAG low_pow_flag) = &ixheaacd_adjust_scale_armv7;

WORD16(*ixheaacd_ixheaacd_expsubbandsamples)
(WORD32 **re, WORD32 **im, WORD32 sub_band_start, WORD32 sub_band_end,
 WORD32 start_pos, WORD32 next_pos,
 FLAG low_pow_flag) = &ixheaacd_expsubbandsamples_armv7;

VOID(*ixheaacd_enery_calc_per_subband)
(WORD32 start_pos, WORD32 next_pos, WORD32 sub_band_start, WORD32 sub_band_end,
 WORD32 frame_exp, WORD16 *nrg_est_mant, FLAG low_pow_flag,
 ia_sbr_tables_struct *ptr_sbr_tables,
 WORD32 *ptr_qmf_matrix) = &ixheaacd_enery_calc_per_subband_dec;

VOID(*ixheaacd_harm_idx_zerotwolp)
(WORD32 *ptr_real_buf, WORD16 *ptr_gain_buf, WORD32 scale_change,
 WORD16 *ptr_sine_level_buf, const WORD32 *ptr_rand_ph,
 WORD16 *noise_level_mant, WORD32 num_sub_bands, FLAG noise_absc_flag,
 WORD32 harm_index) = &ixheaacd_harm_idx_zerotwolp_armv7;

VOID(*ixheaacd_tns_ar_filter_fixed)
(WORD32 *spectrum, WORD32 size, WORD32 inc, WORD32 *lpc, WORD32 order,
 WORD32 shift_value,
 WORD scale_spec) = &ixheaacd_tns_ar_filter_fixed_non_neon_armv7;

VOID(*ixheaacd_tns_ar_filter)
(WORD32 *spectrum, WORD32 size, WORD32 inc, WORD16 *lpc, WORD32 order,
 WORD32 shift_value, WORD scale_spec,
 WORD32 *ptr_filter_state) = &ixheaacd_tns_ar_filter_armv7;

VOID(*ixheaacd_tns_parcor_lpc_convert)
(WORD16 *parcor, WORD16 *lpc, WORD16 *scale,
 WORD order) = &ixheaacd_tns_parcor_lpc_convert_armv7;

WORD32(*ixheaacd_calc_max_spectral_line)
(WORD32 *ptr_tmp, WORD32 size) = &ixheaacd_calc_max_spectral_line_dec;

VOID(*ixheaacd_post_twiddle)
(WORD32 out_ptr[], WORD32 spec_data[],
 ia_aac_dec_imdct_tables_struct *ptr_imdct_tables,
 WORD npoints) = &ixheaacd_post_twiddle_dec;

VOID(*ixheaacd_post_twid_overlap_add)
(WORD32 pcm_out[], WORD32 spec_data[],
 ia_aac_dec_imdct_tables_struct *ptr_imdct_tables, WORD npoints,
 WORD32 *ptr_overlap_buf, WORD16 q_shift, const WORD16 *window,
 WORD16 ch_fac) = &ixheaacd_post_twid_overlap_add_dec;

VOID(*ixheaacd_neg_shift_spec)
(WORD32 *coef, WORD32 *out, WORD16 q_shift,
 WORD16 ch_fac) = &ixheaacd_neg_shift_spec_dec;

VOID(*ixheaacd_spec_to_overlapbuf)
(WORD32 *ptr_overlap_buf, WORD32 *ptr_spec_coeff, WORD32 q_shift,
 WORD32 size) = &ixheaacd_spec_to_overlapbuf_armv7;

VOID(*ixheaacd_overlap_buf_out)
(WORD32 *out_samples, WORD32 *ptr_overlap_buf, WORD32 size,
 const WORD16 ch_fac) = &ixheaacd_overlap_buf_out_armv7;

VOID(*ixheaacd_overlap_out_copy)
(WORD16 *out_samples, WORD32 *ptr_overlap_buf, WORD32 *ptr_overlap_buf1,
 const WORD16 ch_fac) = &ixheaacd_overlap_out_copy_armv7;

VOID(*ixheaacd_pretwiddle_compute)
(WORD32 *spec_data1, WORD32 *spec_data2, WORD32 *out_ptr,
 ia_aac_dec_imdct_tables_struct *ptr_imdct_tables, WORD npoints4,
 WORD32 neg_expo) = &ixheaacd_pretwiddle_compute_dec;

VOID(*ixheaacd_imdct_using_fft)
(ia_aac_dec_imdct_tables_struct *ptr_imdct_tables, WORD32 npoints,
 WORD32 *ptr_x, WORD32 *ptr_y) = &ixheaacd_imdct_using_fft_dec;
