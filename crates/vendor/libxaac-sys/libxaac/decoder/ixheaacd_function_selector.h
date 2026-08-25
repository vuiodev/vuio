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
#ifndef _IXHEAACD_FUNCTION_SELECTOR_H_
#define _IXHEAACD_FUNCTION_SELECTOR_H_

#include <stdio.h>
#include <string.h>
#include "ixheaacd_sbr_common.h"
#include "ixheaac_type_def.h"

#include "ixheaac_constants.h"
#include "ixheaacd_intrinsics.h"
#include "ixheaacd_common_rom.h"
#include "ixheaacd_sbrdecsettings.h"
#include "ixheaacd_bitbuffer.h"
#include "ixheaacd_defines.h"

#include "ixheaacd_pns.h"

#include "ixheaacd_aac_rom.h"
#include "ixheaacd_pulsedata.h"

#include "ixheaacd_drc_data_struct.h"

#include "ixheaacd_lt_predict.h"

#include "ixheaacd_cnst.h"
#include "ixheaacd_ec_defines.h"
#include "ixheaacd_ec_struct_def.h"
#include "ixheaacd_channelinfo.h"
#include "ixheaacd_drc_dec.h"

#include "ixheaacd_sbrdecoder.h"
#include "ixheaacd_sbr_scale.h"
#include "ixheaacd_lpp_tran.h"
#include "ixheaacd_env_extr_part.h"
#include "ixheaacd_sbr_rom.h"
#include "ixheaacd_hybrid.h"
#include "ixheaacd_ps_dec.h"
#include "ixheaacd_env_extr.h"

#include "ixheaacd_basic_funcs.h"

extern WORD32 (*ixheaacd_fix_div)(WORD32, WORD32);
extern VOID (*ixheaacd_covariance_matrix_calc)(WORD32 *,
                                               ia_lpp_trans_cov_matrix *,
                                               WORD32, WORD32);

extern VOID(*ixheaacd_covariance_matrix_calc_960)(WORD32 *,
                                                  ia_lpp_trans_cov_matrix *,
                                                  WORD32, WORD32);

extern VOID (*ixheaacd_covariance_matrix_calc_2)(
    ia_lpp_trans_cov_matrix *, WORD32 *, WORD32, WORD16);
extern VOID (*ixheaacd_over_lap_add1)(WORD32 *, WORD32 *, WORD32 *,
                                      const WORD16 *, WORD16, WORD16, WORD16);
extern VOID (*ixheaacd_over_lap_add2)(WORD32 *, WORD32 *, WORD32 *,
                                      const WORD16 *, WORD16, WORD16, WORD16);

extern VOID (*ixheaacd_decorr_filter1)(ia_ps_dec_struct *,
                                       ia_ps_tables_struct *, WORD16 *);

extern VOID (*ixheaacd_decorr_filter2)(ia_ps_dec_struct *, WORD32 *, WORD32 *,
                                       WORD32 *, WORD32 *,
                                       ia_ps_tables_struct *, WORD16 *);

extern WORD32 (*ixheaacd_divide16_pos)(WORD32, WORD32);

extern VOID (*ixheaacd_decorrelation)(ia_ps_dec_struct *, WORD32 *, WORD32 *,
                                      WORD32 *, WORD32 *,
                                      ia_ps_tables_struct *);

extern VOID (*ixheaacd_apply_rot)(ia_ps_dec_struct *, WORD32 *, WORD32 *,
                                  WORD32 *, WORD32 *, ia_sbr_tables_struct *,
                                  const WORD16 *);

extern VOID (*ixheaacd_conv_ergtoamplitudelp)(WORD32, WORD16, WORD16 *,
                                              WORD16 *, WORD16 *, WORD16 *);

extern VOID (*ixheaacd_conv_ergtoamplitude)(WORD32, WORD16, WORD16 *, WORD16 *,
                                            WORD16 *, WORD16 *);

extern VOID (*ixheaacd_adjust_scale)(WORD32 **, WORD32 **, WORD32, WORD32,
                                     WORD32, WORD32, WORD32, FLAG);

extern WORD16 (*ixheaacd_ixheaacd_expsubbandsamples)(WORD32 **, WORD32 **,
                                                     WORD32, WORD32, WORD32,
                                                     WORD32, FLAG);

extern VOID (*ixheaacd_enery_calc_per_subband)(WORD32, WORD32, WORD32, WORD32,
                                               WORD32, WORD16 *, FLAG,
                                               ia_sbr_tables_struct *,
                                               WORD32 *);

extern VOID (*ixheaacd_harm_idx_zerotwolp)(WORD32 *, WORD16 *, WORD, WORD16 *,
                                           const WORD32 *, WORD16 *, WORD, FLAG,
                                           WORD32);

extern VOID (*ixheaacd_tns_ar_filter_fixed)(WORD32 *, WORD32, WORD32, WORD32 *,
                                            WORD32, WORD32, WORD);

extern VOID (*ixheaacd_tns_ar_filter)(WORD32 *, WORD32, WORD32, WORD16 *,
                                      WORD32, WORD32, WORD, WORD32 *);

extern VOID (*ixheaacd_tns_parcor_lpc_convert)(WORD16 *, WORD16 *, WORD16 *,
                                               WORD);

extern WORD32 (*ixheaacd_calc_max_spectral_line)(WORD32 *, WORD32);

extern VOID (*ixheaacd_post_twiddle)(WORD32[], WORD32[],
                                     ia_aac_dec_imdct_tables_struct *, WORD);

extern VOID (*ixheaacd_post_twid_overlap_add)(WORD32[], WORD32[],
                                              ia_aac_dec_imdct_tables_struct *,
                                              WORD, WORD32 *, WORD16,
                                              const WORD16 *, WORD16);

extern VOID (*ixheaacd_neg_shift_spec)(WORD32 *, WORD32 *, WORD16, WORD16);

extern VOID (*ixheaacd_spec_to_overlapbuf)(WORD32 *, WORD32 *, WORD32, WORD32);

extern VOID (*ixheaacd_overlap_buf_out)(WORD32 *, WORD32 *, WORD32,
                                        const WORD16);

extern VOID (*ixheaacd_overlap_out_copy)(WORD32 *, WORD32 *, WORD32 *,
                                         const WORD16, WORD16);

extern VOID (*ixheaacd_pretwiddle_compute)(WORD32 *, WORD32 *, WORD32 *,
                                           ia_aac_dec_imdct_tables_struct *,
                                           WORD, WORD32);

extern VOID(*ixheaacd_pretwiddle_compute_960)(WORD32 *, WORD32 *, WORD32 *,
                                              ia_aac_dec_imdct_tables_struct *,
                                              WORD, WORD32);

extern VOID (*ixheaacd_imdct_using_fft)(ia_aac_dec_imdct_tables_struct *,
                                        WORD32, WORD32 *, WORD32 *);

extern VOID (*ixheaacd_complex_fft_p2)(WORD32 *xr, WORD32 *xi, WORD32 nlength,
                                       WORD32 fft_mode, WORD32 *preshift);

extern VOID(*ixheaacd_mps_complex_fft_64)(WORD32 *ptr_x, WORD32 *fin_re,
                                          WORD32 *fin_im, WORD32 nlength);

extern VOID(*ixheaacd_mps_synt_pre_twiddle)(FLOAT32 *ptr_in,
                                            const FLOAT32 *table_re,
                                            const FLOAT32 *table_im,
                                            WORD32 resolution);

extern VOID(*ixheaacd_mps_synt_post_twiddle)(WORD32 *ptr_in,
                                             const WORD32 *table_re,
                                             const WORD32 *table_im,
                                             WORD32 resolution);

extern VOID (*ixheaacd_calc_pre_twid)(WORD32 *ptr_x, WORD32 *r_ptr,
                                      WORD32 *i_ptr, WORD32 nlength,
                                      const WORD32 *cos_ptr,
                                      const WORD32 *sin_ptr);

extern VOID (*ixheaacd_calc_post_twid)(WORD32 *ptr_x, WORD32 *r_ptr,
                                       WORD32 *i_ptr, WORD32 nlength,
                                       const WORD32 *cos_ptr,
                                       const WORD32 *sin_ptr);

extern VOID(*ixheaacd_mps_synt_post_fft_twiddle)(
    WORD32 resolution, FLOAT32 *fin_re, FLOAT32 *fin_im, const FLOAT32 *table_re,
    const FLOAT32 *table_im, FLOAT32 *state);

extern VOID (*ixheaacd_mps_synt_out_calc)(WORD32 resolution, FLOAT32 *out,
                                          FLOAT32 *state,
                                          const FLOAT32 *filter_coeff);

extern VOID (*ixheaacd_fft_15_ld)(WORD32 *inp, WORD32 *op, WORD32 *fft3out,
                                  UWORD8 *re_arr_tab_sml_240_ptr);

extern VOID(*ixheaacd_fft_15_960_dec)(WORD32 *inp, WORD32 *op, WORD32 *fft3out,
                                      UWORD8 *re_arr_tab_sml_480_ptr);

extern VOID (*ixheaacd_aac_ld_dec_rearrange)(WORD32 *ip, WORD32 *op,
                                             WORD32 mdct_len_2,
                                             UWORD8 *re_arr_tab);

extern VOID(*ixheaacd_aac_ld_dec_rearrange_960)(WORD32 *ip, WORD32 *op,
                                                WORD32 mdct_len_2,
                                                WORD16 *re_arr_tab);

extern VOID (*ixheaacd_fft32x32_ld)(
    ia_aac_dec_imdct_tables_struct *imdct_tables_ptr, WORD32 npoints,
    WORD32 *ptr_x, WORD32 *ptr_y);

extern VOID (*ixheaacd_fft32x32_ld2)(
    ia_aac_dec_imdct_tables_struct *imdct_tables_ptr, WORD32 npoints,
    WORD32 *ptr_x, WORD32 *ptr_y);

extern VOID(*ixheaacd_fft32_points_960)(
    ia_aac_dec_imdct_tables_struct *imdct_tables_ptr, WORD32 npoints,
    WORD32 *ptr_x, WORD32 *ptr_y);

extern WORD16 (*ixheaacd_neg_expo_inc)(WORD16 neg_expo);

extern VOID (*ixheaacd_inv_dit_fft_8pt)(WORD32 *x, WORD32 *real, WORD32 *imag);

extern VOID (*ixheaacd_scale_factor_process)(
    WORD32 *x_invquant, WORD16 *scale_fact, WORD no_band, WORD8 *width,
    WORD32 *scale_tables_ptr, WORD32 total_channels, WORD32 object_type,
    WORD32 aac_sf_data_resil_flag);

#endif /* _IXHEAACD_FUNCTION_SELECTOR_H_ */
