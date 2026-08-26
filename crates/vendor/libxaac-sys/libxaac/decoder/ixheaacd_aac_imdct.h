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
#ifndef IXHEAACD_AAC_IMDCT_H
#define IXHEAACD_AAC_IMDCT_H

WORD32 ixheaacd_inverse_transform(
    WORD32 spec_data[], WORD32 scratch[],
    ia_aac_dec_imdct_tables_struct *ptr_imdct_tables, WORD32 expo,
    WORD32 npoints);

VOID ixheaacd_inverse_transform_960(
    WORD32 spec_data[], WORD32 scratch[],
    ia_aac_dec_imdct_tables_struct *ptr_imdct_tables, WORD32 expo,
    WORD32 *imdct_scale);

VOID ixheaacd_post_twiddle_dec(WORD32 out_ptr[], WORD32 spec_data[],
                               ia_aac_dec_imdct_tables_struct *ptr_imdct_tables,
                               WORD32 npoints);

VOID ixheaacd_post_twiddle_armv7(
    WORD32 out_ptr[], WORD32 spec_data[],
    ia_aac_dec_imdct_tables_struct *ptr_imdct_tables, WORD32 npoints);

VOID ixheaacd_post_twiddle_armv8(
    WORD32 out_ptr[], WORD32 spec_data[],
    ia_aac_dec_imdct_tables_struct *ptr_imdct_tables, WORD32 npoints);

VOID ixheaacd_post_twid_overlap_add_dec(
    WORD32 pcm_out[], WORD32 spec_data[],
    ia_aac_dec_imdct_tables_struct *ptr_imdct_tables, WORD npoints,
    WORD32 *ptr_overlap_buf, WORD16 q_shift, const WORD16 *window,
    WORD16 ch_fac);

VOID ixheaacd_post_twid_overlap_add_armv7(
    WORD32 pcm_out[], WORD32 spec_data[],
    ia_aac_dec_imdct_tables_struct *ptr_imdct_tables, WORD npoints,
    WORD32 *ptr_overlap_buf, WORD16 q_shift, const WORD16 *window,
    WORD16 ch_fac);

VOID ixheaacd_post_twid_overlap_add_armv8(
    WORD32 pcm_out[], WORD32 spec_data[],
    ia_aac_dec_imdct_tables_struct *ptr_imdct_tables, WORD npoints,
    WORD32 *ptr_overlap_buf, WORD16 q_shift, const WORD16 *window,
    WORD16 ch_fac);

VOID ixheaacd_pretwiddle_compute_dec(
    WORD32 *spec_data1, WORD32 *spec_data2, WORD32 *out_ptr,
    ia_aac_dec_imdct_tables_struct *ptr_imdct_tables, WORD npoints4,
    WORD32 neg_expo);

VOID ixheaacd_pretwiddle_compute_960_dec(
    WORD32 *spec_data1, WORD32 *spec_data2, WORD32 *out_ptr,
    ia_aac_dec_imdct_tables_struct *ptr_imdct_tables, WORD npoints4,
    WORD32 neg_expo);

VOID ixheaacd_pretwiddle_compute_armv7(
    WORD32 *spec_data1, WORD32 *spec_data2, WORD32 *out_ptr,
    ia_aac_dec_imdct_tables_struct *ptr_imdct_tables, WORD npoints4,
    WORD32 neg_expo);

VOID ixheaacd_pretwiddle_compute_armv8(
    WORD32 *spec_data1, WORD32 *spec_data2, WORD32 *out_ptr,
    ia_aac_dec_imdct_tables_struct *ptr_imdct_tables, WORD npoints4,
    WORD32 neg_expo);

VOID ixheaacd_imdct_using_fft_dec(
    ia_aac_dec_imdct_tables_struct *ptr_imdct_tables, WORD32 npoints,
    WORD32 *ptr_x, WORD32 *ptr_y);

VOID ixheaacd_imdct_using_fft_armv7(
    ia_aac_dec_imdct_tables_struct *ptr_imdct_tables, WORD32 npoints,
    WORD32 *ptr_x, WORD32 *ptr_y);

VOID ixheaacd_imdct_using_fft_armv8(
    ia_aac_dec_imdct_tables_struct *ptr_imdct_tables, WORD32 npoints,
    WORD32 *ptr_x, WORD32 *ptr_y);

VOID ixheaacd_fft_480_ld(WORD32 *inp, WORD32 *op,
                         ia_aac_dec_imdct_tables_struct *imdct_tables_ptr);

VOID ixheaacd_fft_5(WORD32 *inp, WORD32 *op);

VOID ixheaacd_fft_3(WORD32 *inp, WORD32 *op);

VOID ixheaacd_fft_960_15(WORD32 *inp, WORD32 *op,
                         ia_aac_dec_imdct_tables_struct *imdct_tables_ptr);

VOID ixheaacd_fft_960(WORD32 *inp, WORD32 *op,
                      ia_aac_dec_imdct_tables_struct *imdct_tables_ptr);

VOID ixheaacd_fft_120(ia_aac_dec_imdct_tables_struct *imdct_tables_ptr,
                      WORD32 npoints, WORD32* ptr_x, WORD32* ptr_y);

VOID ixheaacd_fft_32_points(WORD16 *ptr_w, WORD32 npoints,
                            WORD32* ptr_x, WORD32* ptr_y);

VOID ixheaacd_ld_dec_fft_15_opt(WORD32 *inp, WORD32 *op, WORD32 *fft3out,
                                WORD16 *re_arr_tab_sml_240_ptr);

VOID ixheaacd_dec_rearrange_short(WORD32 *ip, WORD32 *op, WORD32 mdct_len_2,
                                  WORD16 *re_arr_tab);

VOID ixheaacd_pre_twiddle(WORD32 *xptr, WORD32 *data, WORD32 n,
                          WORD32 *cos_array_ptr, WORD32 neg_expo);

VOID ixheaacd_pre_twiddle_960(WORD32 *xptr, WORD32 *data, WORD32 n,
                              WORD32 *cos_array_ptr, WORD32 neg_expo);

VOID ixheaacd_pre_twiddle_120(WORD32 *xptr, WORD32 *data, WORD32 n,
                              WORD16 *cos_array_ptr, WORD32 neg_expo);

VOID ixheaacd_post_twiddle_ld(WORD32 out[], WORD32 x[],
                              const WORD32 *cos_sin_ptr, WORD m);

VOID ixheaacd_post_twiddle_960(WORD32 out[], WORD32 x[],
                               const WORD32 *cos_sin_ptr, WORD m);

VOID ixheaacd_post_twiddle_120(WORD32 out[], WORD32 x[],
                               const WORD16 *cos_sin_ptr, WORD m);

VOID ixheaacd_post_twiddle_eld(WORD32 out[], WORD32 x[],
                               const WORD32 *cos_sin_ptr, WORD m);

VOID ixheaacd_fft32x32_ld_dec(ia_aac_dec_imdct_tables_struct *imdct_tables_ptr,
                              WORD32 npoints, WORD32 *ptr_x, WORD32 *ptr_y);

VOID ixheaacd_fft32x32_ld2_armv7(
    ia_aac_dec_imdct_tables_struct *imdct_tables_ptr, WORD32 npoints,
    WORD32 *ptr_x, WORD32 *ptr_y);

VOID ixheaacd_fft32x32_ld2_armv8(
    ia_aac_dec_imdct_tables_struct *imdct_tables_ptr, WORD32 npoints,
    WORD32 *ptr_x, WORD32 *ptr_y);

WORD16 ixheaacd_neg_expo_inc_dec(WORD16 neg_expo);

WORD16 ixheaacd_neg_expo_inc_arm(WORD16 neg_expo);

VOID ixheaacd_rearrange_dec(WORD32 *ip, WORD32 *op, WORD32 mdct_len_2,
                            UWORD8 *re_arr_tab);

VOID ia_aac_ld_dec_rearrange_armv7(WORD32 *ip, WORD32 *op, WORD32 mdct_len_2,
                                   UWORD8 *re_arr_tab);

VOID ixheaacd_fft_15_ld_dec(WORD32 *inp, WORD32 *op, WORD32 *fft3out,
                            UWORD8 *re_arr_tab_sml_240_ptr);

VOID ixheaacd_fft_15_ld_armv7(WORD32 *inp, WORD32 *op, WORD32 *fft3out,
                              UWORD8 *re_arr_tab_sml_240_ptr);

VOID ixheaacd_inverse_transform_512(
    WORD32 data[], WORD32 temp[], WORD32 *imdct_scale, WORD32 *cos_sin_ptr,
    ia_aac_dec_imdct_tables_struct *imdct_tables_ptr, WORD32 object_type);

VOID ixheaacd_mdct_960(WORD32 *inp, WORD32 *scratch, WORD32 *mdct_scale,
                       WORD32 mdct_flag,
                       ia_aac_dec_imdct_tables_struct *imdct_tables_ptr);

VOID ixheaacd_mdct_480_ld(WORD32 *inp, WORD32 *scratch, WORD32 *mdct_scale,
                          WORD32 mdct_flag,
                          ia_aac_dec_imdct_tables_struct *imdct_tables_ptr,
                          WORD32 object_type);
#endif
