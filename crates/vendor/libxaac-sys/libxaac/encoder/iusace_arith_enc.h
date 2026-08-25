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
#ifndef MIN
#define MIN(a, b) ((a) < (b) ? (a) : (b))
#endif

#ifndef MAX
#define MAX(a, b) ((a) > (b) ? (a) : (b))
#endif

typedef struct {
  WORD32 low;
  WORD32 high;
  WORD32 value;
} iusace_state_arith;

WORD32 iusace_arith_enc_spec(ia_bit_buf_struct *it_bit_buf, WORD32 window_sequence,
                             WORD32 *ptr_x_ac_enc, WORD32 max_spec_coefficients,
                             WORD32 *ptr_c_pres, WORD32 *ptr_c_prev, WORD32 *ptr_size_prev,
                             WORD32 arith_reset_flag, WORD32 ccfl);

WORD32 iusace_tcx_coding(ia_bit_buf_struct *pstr_it_bit_buff, WORD32 tcx_size,
                         WORD32 max_tcx_size, WORD32 *ptr_quant, WORD32 *c_pres, WORD32 *c_prev);

WORD32 iusace_arith_done(ia_bit_buf_struct *pstr_it_bit_buff, WORD32 bp, iusace_state_arith *s);

WORD32 iusace_arith_encode(ia_bit_buf_struct *pstr_it_bit_buff, WORD32 bp, iusace_state_arith *s,
                           WORD32 symbol, UWORD16 const *cum_freq);
