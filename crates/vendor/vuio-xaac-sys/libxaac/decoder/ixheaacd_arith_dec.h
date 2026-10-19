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
#ifndef IXHEAACD_ARITH_DEC_H
#define IXHEAACD_ARITH_DEC_H

WORD32 ixheaacd_ac_spectral_data(ia_usac_data_struct *usac_data,
                                 WORD32 max_spec_coefficients,
                                 WORD32 noise_level, WORD32 noise_offset,
                                 WORD32 arth_size,
                                 ia_bit_buf_struct *it_bit_buff, UWORD8 max_sfb,
                                 WORD32 reset, WORD32 noise_filling, WORD32 ch);

VOID ixheaacd_arith_data(ia_td_frame_data_struct *pstr_td_frame_data,
                           WORD32 *quant, ia_usac_data_struct *usac_data,
                           ia_bit_buf_struct *it_bit_buff,
                           WORD32 first_tcx_flag, WORD32 k);

#endif
