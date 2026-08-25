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
#ifndef IXHEAACD_BASIC_FUNCS_H
#define IXHEAACD_BASIC_FUNCS_H

#define LOG_2_TABLE_SIZE 65
#define INV_TABLE_SIZE 256
#define SQRT_TABLE_SIZE 256

WORD32 ixheaacd_sqrt(WORD32 op);

VOID ixheaacd_fix_mant_exp_add(WORD16 a_m, WORD16 a_e, WORD16 b_m, WORD16 b_e,
                              WORD16 *ptr_sum_mant, WORD16 *ptr_sum_exp);

WORD32 ixheaacd_fix_mant_div(WORD16 a_m, WORD16 b_m, WORD16 *ptr_result,
                            ixheaacd_misc_tables *pstr_common_tables);

VOID ixheaacd_fix_mant_exp_sqrt(WORD16 *mant, WORD16 *sqrt_table);

WORD32 ixheaacd_fix_div_dec(WORD32 divident, WORD32 divisor);

WORD32 ixheaacd_fix_div_armv7(WORD32 divident, WORD32 divisor);

extern VOID ixheaacd_scale_short_vec_left(WORD16 *word16_arr, WORD32 n,
                                          WORD16 shift);

extern VOID ixheaacd_scale_int_vec_left(WORD32 *word32_arr, WORD32 n,
                                        WORD16 shift);

extern VOID ixheaacd_scale_int_vec_right(WORD32 *word32_arr, WORD32 n,
                                         WORD16 shift);

extern VOID ixheaacd_scale_short_vec_right(WORD16 *word16_arr, WORD32 n,
                                           WORD16 shift);

#endif /* IXHEAACD_BASIC_FUNCS_H */
