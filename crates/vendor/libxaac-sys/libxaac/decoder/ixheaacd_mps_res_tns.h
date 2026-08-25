/******************************************************************************
 *
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
#ifndef IXHEAACD_MPS_RES_TNS_H
#define IXHEAACD_MPS_RES_TNS_H

VOID ixheaacd_res_ctns_apply(ia_mps_dec_residual_channel_info_struct *p_aac_decoder_channel_info,
                             WORD16 max_sfb,
                             ia_mps_dec_residual_aac_tables_struct *aac_tables_ptr);

WORD16
ixheaacd_res_c_tns_read(ia_bit_buf_struct *it_bit_buf,
                        ia_mps_dec_residual_channel_info_struct *p_aac_decoder_channel_info);

WORD32 ixheaacd_res_calc_max_spectral_line(WORD32 *p_tmp, WORD32 size);

VOID ixheaacd_res_tns_parcor_2_lpc_32x16(WORD16 *parcor, WORD16 *lpc, WORD16 *scale, WORD order);

VOID ixheaacd_res_tns_ar_filter_fixed_32x16(WORD32 *spectrum, WORD32 size, WORD32 inc,
                                            WORD16 *lpc, WORD32 order, WORD32 shift_value,
                                            WORD scale_spec);

#endif /* IXHEAACD_MPS_RES_TNS_H */
