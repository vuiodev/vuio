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
#ifndef IXHEAACD_TNS_H
#define IXHEAACD_TNS_H

VOID ixheaacd_aac_tns_process(
    ia_aac_dec_channel_info_struct *ptr_aac_dec_channel_info, WORD32 num_ch,
    ia_aac_dec_tables_struct *ptr_aac_tables, WORD object_type, WORD32 ar_flag,
    WORD32 *predicted_spectrum);

WORD16 ixheaacd_read_tns_data(
    ia_bit_buf_struct *it_bit_buff,
    ia_aac_dec_channel_info_struct *ptr_aac_dec_channel_info);

WORD32 ixheaacd_calc_max_spectral_line_dec(WORD32 *ptr_tmp, WORD32 size);

WORD32 ixheaacd_calc_max_spectral_line_armv7(WORD32 *ptr_tmp, WORD32 size);

WORD32 ixheaacd_calc_max_spectral_line_armv8(WORD32 *ptr_tmp, WORD32 size);

VOID ixheaacd_tns_decode_coef(const ia_filter_info_struct *filter,
                              WORD16 *parcor_coeff,
                              ia_aac_dec_tables_struct *ptr_aac_tables);

VOID ixheaacd_tns_parcor_lpc_convert_dec(WORD16 *parcor, WORD16 *lpc,
                                         WORD16 *scale, WORD order);

void ixheaacd_tns_parcor_lpc_convert_dec_ld(WORD32 *parcor, WORD32 *lpc,
                                            WORD16 *scale, WORD32 order);

VOID ixheaacd_tns_parcor_lpc_convert_armv7(WORD16 *parcor, WORD16 *lpc,
                                           WORD16 *scale, WORD order);

VOID ixheaacd_tns_ar_filter_dec(WORD32 *spectrum, WORD32 size, WORD32 inc,
                                WORD16 *lpc, WORD32 order, WORD32 shift_value,
                                WORD scale_spec, WORD32 *ptr_filter_state);

void ixheaacd_tns_ma_filter_fixed_ld(WORD32 *spectrum, WORD32 size, WORD32 inc,
                                     WORD32 *lpc, WORD32 order,
                                     WORD16 shift_value);

VOID ixheaacd_tns_ar_filter_armv7(WORD32 *spectrum, WORD32 size, WORD32 inc,
                                  WORD16 *lpc, WORD32 order, WORD32 shift_value,
                                  WORD scale_spec, WORD32 *ptr_filter_state);

VOID ixheaacd_tns_decode_coefficients(const ia_filter_info_struct *filter,
                                      WORD32 *a,
                                      ia_aac_dec_tables_struct *ptr_aac_tables);
VOID ixheaacd_tns_parcor_to_lpc(WORD32 *parcor, WORD32 *lpc, WORD16 *scale,
                                WORD32 order);

VOID ixheaacd_tns_ar_filter_fixed_dec(WORD32 *spectrum, WORD32 size, WORD32 inc,
                                      WORD32 *lpc, WORD32 order,
                                      WORD32 shift_value, WORD scale_spec);

VOID ixheaacd_tns_ar_filter_fixed_armv7(WORD32 *spectrum, WORD32 size,
                                        WORD32 inc, WORD32 *lpc, WORD32 order,
                                        WORD32 shift_value, WORD scale_spec);

VOID ixheaacd_tns_ar_filter_fixed_non_neon_armv7(WORD32 *spectrum, WORD32 size,
                                                 WORD32 inc, WORD32 *lpc,
                                                 WORD32 order,
                                                 WORD32 shift_value,
                                                 WORD scale_spec);

VOID ixheaacd_tns_ar_filter_fixed_armv8(WORD32 *spectrum, WORD32 size,
                                        WORD32 inc, WORD32 *lpc, WORD32 order,
                                        WORD32 shift_value, WORD scale_spec);

#endif /* #ifndef IXHEAACD_TNS_H */
