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

#include "ixheaacd_lt_predict.h"

#include "ixheaacd_cnst.h"
#include "ixheaacd_ec_defines.h"
#include "ixheaacd_ec_struct_def.h"

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
#include "ixheaacd_dsp_fft32x32s.h"

#include "ixheaacd_interface.h"

WORD32 (*ixheaacd_fix_div)(WORD32, WORD32) = &ixheaacd_fix_div_armv7;

VOID(*ixheaacd_covariance_matrix_calc)
(WORD32 *, ia_lpp_trans_cov_matrix *,
 WORD32) = &ixheaacd_covariance_matrix_calc_armv7;

VOID(*ixheaacd_covariance_matrix_calc_2)
(ia_lpp_trans_cov_matrix *, WORD32 *, WORD32,
 WORD16) = &ixheaacd_covariance_matrix_calc_2_armv7;

VOID(*ixheaacd_over_lap_add1)
(WORD32 *, WORD32 *, WORD32 *, const WORD16 *, WORD16, WORD16,
 WORD16) = &ixheaacd_over_lap_add1_armv7;

VOID(*ixheaacd_over_lap_add2)
(WORD32 *, WORD32 *, WORD32 *, const WORD16 *, WORD16, WORD16,
 WORD16) = &ixheaacd_over_lap_add2_armv7;

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
(ia_ps_dec_struct *ptr_ps_dec, WORD32 *qmf_left_real, WORD32 *qmf_left_imag,
 WORD32 *qmf_right_real, WORD32 *qmf_right_imag,
 ia_sbr_tables_struct *sbr_tables_ptr,
 const WORD16 *ptr_resol) = &ixheaacd_apply_rot_armv7;

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
 WORD32 *ptr_qmf_matrix) = &ixheaacd_enery_calc_per_subband_armv7;

VOID(*ixheaacd_harm_idx_zerotwolp)
(WORD32 *ptr_real_buf, WORD16 *ptr_gain_buf, WORD32 scale_change,
 WORD16 *ptr_sine_level_buf, const WORD32 *ptr_rand_ph,
 WORD16 *noise_level_mant, WORD32 num_sub_bands, FLAG noise_absc_flag,
 WORD32 harm_index) = &ixheaacd_harm_idx_zerotwolp_armv7;

VOID(*ixheaacd_tns_ar_filter_fixed)
(WORD32 *spectrum, WORD32 size, WORD32 inc, WORD32 *lpc, WORD32 order,
 WORD32 shift_value, WORD scale_spec) = &ixheaacd_tns_ar_filter_fixed_armv7;

VOID(*ixheaacd_tns_ar_filter)
(WORD32 *spectrum, WORD32 size, WORD32 inc, WORD16 *lpc, WORD32 order,
 WORD32 shift_value, WORD scale_spec,
 WORD32 *ptr_filter_state) = &ixheaacd_tns_ar_filter_armv7;

VOID(*ixheaacd_tns_parcor_lpc_convert)
(WORD16 *parcor, WORD16 *lpc, WORD16 *scale,
 WORD order) = &ixheaacd_tns_parcor_lpc_convert_armv7;

WORD32(*ixheaacd_calc_max_spectral_line)
(WORD32 *ptr_tmp, WORD32 size) = &ixheaacd_calc_max_spectral_line_armv7;

VOID(*ixheaacd_post_twiddle)
(WORD32 out_ptr[], WORD32 spec_data[],
 ia_aac_dec_imdct_tables_struct *ptr_imdct_tables,
 WORD npoints) = &ixheaacd_post_twiddle_armv7;

VOID(*ixheaacd_post_twid_overlap_add)
(WORD32 pcm_out[], WORD32 spec_data[],
 ia_aac_dec_imdct_tables_struct *ptr_imdct_tables, WORD npoints,
 WORD32 *ptr_overlap_buf, WORD16 q_shift, const WORD16 *window,
 WORD16 ch_fac) = &ixheaacd_post_twid_overlap_add_armv7;

VOID(*ixheaacd_neg_shift_spec)
(WORD32 *coef, WORD32 *out, WORD16 q_shift,
 WORD16 ch_fac) = &ixheaacd_neg_shift_spec_armv7;

VOID(*ixheaacd_spec_to_overlapbuf)
(WORD32 *ptr_overlap_buf, WORD32 *ptr_spec_coeff, WORD32 q_shift,
 WORD32 size) = &ixheaacd_spec_to_overlapbuf_armv7;

VOID(*ixheaacd_overlap_buf_out)
(WORD32 *out_samples, WORD32 *ptr_overlap_buf, WORD32 size,
 const WORD16 ch_fac) = &ixheaacd_overlap_buf_out_armv7;

VOID(*ixheaacd_overlap_out_copy)
(WORD32 *out_samples, WORD32 *ptr_overlap_buf, WORD32 *ptr_overlap_buf1,
 const WORD16 ch_fac) = &ixheaacd_overlap_out_copy_armv7;

VOID(*ixheaacd_pretwiddle_compute)
(WORD32 *spec_data1, WORD32 *spec_data2, WORD32 *out_ptr,
 ia_aac_dec_imdct_tables_struct *ptr_imdct_tables, WORD npoints4,
 WORD32 neg_expo) = &ixheaacd_pretwiddle_compute_armv7;

VOID(*ixheaacd_imdct_using_fft)
(ia_aac_dec_imdct_tables_struct *ptr_imdct_tables, WORD32 npoints,
 WORD32 *ptr_x, WORD32 *ptr_y) = &ixheaacd_imdct_using_fft_armv7;

VOID(*ixheaacd_complex_fft_p2)
(WORD32 *xr, WORD32 *xi, WORD32 nlength, WORD32 fft_mode,
 WORD32 *preshift) = &ixheaacd_complex_fft_p2_armv7;

VOID(*ixheaacd_calc_pre_twid)
(WORD32 *ptr_x, WORD32 *r_ptr, WORD32 *i_ptr, WORD32 nlength,
 const WORD32 *cos_ptr, const WORD32 *sin_ptr) = &ixheaacd_calc_pre_twid_armv7;

VOID(*ixheaacd_calc_post_twid)
(WORD32 *ptr_x, WORD32 *r_ptr, WORD32 *i_ptr, WORD32 nlength,
 const WORD32 *cos_ptr, const WORD32 *sin_ptr) = &ixheaacd_calc_post_twid_armv7;

VOID(*ixheaacd_mps_synt_out_calc)
(WORD32 resolution, FLOAT32 *out, FLOAT32 *state,
 const FLOAT32 *filter_coeff) = &ixheaacd_mps_synt_out_calc_dec;

VOID(*ixheaacd_fft_15_ld)
(WORD32 *inp, WORD32 *op, WORD32 *fft3out,
 UWORD8 *re_arr_tab_sml_240_ptr) = &ixheaacd_fft_15_ld_armv7;

VOID(*ixheaacd_aac_ld_dec_rearrange)
(WORD32 *ip, WORD32 *op, WORD32 mdct_len_2,
 UWORD8 *re_arr_tab) = &ia_aac_ld_dec_rearrange_armv7;

VOID(*ixheaacd_fft32x32_ld)
(ia_aac_dec_imdct_tables_struct *imdct_tables_ptr, WORD32 npoints,
 WORD32 *ptr_x, WORD32 *ptr_y) = &ixheaacd_imdct_using_fft_armv7;

VOID(*ixheaacd_fft32x32_ld2)
(ia_aac_dec_imdct_tables_struct *imdct_tables_ptr, WORD32 npoints,
 WORD32 *ptr_x, WORD32 *ptr_y) = &ixheaacd_fft32x32_ld2_armv7;

WORD16 (*ixheaacd_neg_expo_inc)(WORD16 neg_expo) = &ixheaacd_neg_expo_inc_arm;

VOID(*ixheaacd_inv_dit_fft_8pt)
(WORD32 *x, WORD32 *real, WORD32 *imag) = &ixheaacd_inv_dit_fft_8pt_armv7;

VOID(*ixheaacd_scale_factor_process)
(WORD32 *x_invquant, WORD16 *scale_fact, WORD no_band, WORD8 *width,
 WORD32 *scale_tables_ptr, WORD32 total_channels, WORD32 object_type,
 WORD32 aac_sf_data_resil_flag) = &ixheaacd_scale_factor_process_armv7;

VOID(*ixheaacd_covariance_matrix_calc_960)
(WORD32 *, ia_lpp_trans_cov_matrix *,
 WORD32, WORD32) = &ixheaacd_covariance_matrix_calc_dec_960;

VOID(*ixheaacd_aac_ld_dec_rearrange_960)
(WORD32 *ip, WORD32 *op, WORD32 mdct_len_2,
 WORD16 *re_arr_tab) = &ixheaacd_dec_rearrange_short;

VOID(*ixheaacd_pretwiddle_compute_960)
(WORD32 *spec_data1, WORD32 *spec_data2, WORD32 *out_ptr,
 ia_aac_dec_imdct_tables_struct *ptr_imdct_tables, WORD npoints4,
 WORD32 neg_expo) = &ixheaacd_pretwiddle_compute_960_dec;
