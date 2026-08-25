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
#ifndef IXHEAACD_TD_MDCT_H
#define IXHEAACD_TD_MDCT_H

WORD8 ixheaacd_float2fix(FLOAT32 *x, WORD32 *int_x, WORD32 length);

VOID ixheaacd_fix2float(WORD32 *int_xn1, FLOAT32 *xn1, WORD32 length,
                        WORD8 *shiftp, WORD32 *preshift);

VOID ixheaacd_complex_fft(WORD32 *data_r, WORD32 *data_i, WORD32 len, WORD32 fft_mode,
                          WORD32 *preshift);

#endif
