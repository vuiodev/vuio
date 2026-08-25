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
#ifndef IXHEAACD_ACELP_COM_H
#define IXHEAACD_ACELP_COM_H

#define LEN_ABS_LEADER 37
#define LEN_SIGN_LEADER 226
#define LEN_I3 9
#define LEN_I4 28

extern const FLOAT32 ixheaacd_int_leave_gain_table[256];

VOID ixheaacd_rotated_gosset_mtx_dec(WORD32 qn, WORD32 code_book_idx,
                                     WORD32 kv[], WORD32 y[]);

VOID ixheaacd_residual_tool(WORD32 *a, WORD32 *x, WORD32 *y, WORD32 l,
                            WORD32 n);

VOID ixheaacd_synthesis_tool_float(FLOAT32 a[], FLOAT32 x[], FLOAT32 y[],
                                   WORD32 l, FLOAT32 mem[]);

VOID ixheaacd_synthesis_tool_float1(FLOAT32 a[], FLOAT32 x[], WORD32 l);

VOID ixheaacd_lpc_wt_synthesis_tool(FLOAT32 a[], FLOAT32 x[], WORD32 l);

WORD16 ixheaacd_rand_gen(WORD16 *seed);

VOID ixheaacd_preemphsis_tool(WORD32 *signal, WORD32 mu, WORD32 len,
                              WORD32 mem);

VOID ixheaacd_deemphsis_tool(FLOAT32 *signal, WORD32 len, FLOAT32 mem);

VOID ixheaacd_residual_tool_float(FLOAT32 *a, FLOAT32 *x, FLOAT32 *y, WORD32 l,
                                  WORD32 loop_count);

VOID ixheaacd_residual_tool_float1(FLOAT32 *a, FLOAT32 *x, FLOAT32 *y, WORD32 l,
                                   WORD32 loop_count);

VOID ixheaacd_preemphsis_tool_float(FLOAT32 *signal, FLOAT32 mu, WORD32 len,
                                    FLOAT32 mem);

VOID ixheaacd_lsp_to_lp_conversion(FLOAT32 *lsp, FLOAT32 *a);

VOID ixheaacd_lpc_coeff_wt_apply(FLOAT32 *a, FLOAT32 *ap);

VOID ixheaacd_acelp_pitch_sharpening(FLOAT32 *x, WORD32 pit_lag);

VOID ixheaacd_acelp_decode_pulses_per_track(WORD32 index[], const WORD16 nbbits,
                                            FLOAT32 code[]);

#endif
