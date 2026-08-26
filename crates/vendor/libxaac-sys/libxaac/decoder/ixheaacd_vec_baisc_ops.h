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
#ifndef IXHEAACD_VEC_BAISC_OPS_H
#define IXHEAACD_VEC_BAISC_OPS_H

VOID ixheaacd_combine_fac(WORD32 *src1, WORD32 *src2, WORD32 *dest, WORD32 len,
                          WORD8 shift1, WORD8 shift2);

WORD8 ixheaacd_windowing_long1(WORD32 *src1, WORD32 *src2,
                               const WORD32 *win_fwd, const WORD32 *win_rev,
                               WORD32 *dest, WORD32 vlen, WORD8 shift1,
                               WORD8 shift2);

WORD8 ixheaacd_windowing_long2(WORD32 *src1, const WORD32 *win_fwd,
                               WORD32 *fac_data_out, WORD32 *over_lap,
                               WORD32 *p_out_buffer,
                               offset_lengths *ixheaacd_drc_offset,
                               WORD8 shift1, WORD8 shift2, WORD8 shift3);

WORD8 ixheaacd_windowing_long3(WORD32 *src1, const WORD32 *win_fwd,
                               WORD32 *over_lap, WORD32 *p_out_buffer,
                               const WORD32 *win_rev,
                               offset_lengths *ixheaacd_drc_offset,
                               WORD8 shift1, WORD8 shift2);

VOID ixheaacd_windowing_short1(WORD32 *src1, WORD32 *src2, WORD32 *fp,
                               offset_lengths *ixheaacd_drc_offset,
                               WORD8 shiftp, WORD8 shift_olap);

VOID ixheaacd_windowing_short2(WORD32 *src1, WORD32 *win_fwd, WORD32 *fp,
                               offset_lengths *ixheaacd_drc_offset,
                               WORD8 shiftp, WORD8 shift_olap);

WORD8 ixheaacd_windowing_short3(WORD32 *src1, WORD32 *win_rev, WORD32 *fp,
                                WORD32 nshort, WORD8 shiftp, WORD8 shift_olap);

WORD8 ixheaacd_windowing_short4(WORD32 *src1, WORD32 *win_fwd, WORD32 *fp,
                                WORD32 *win_fwd1, WORD32 nshort, WORD32 flag,
                                WORD8 shiftp, WORD8 shift_olap, WORD8 output_q);

VOID ixheaacd_scale_down(WORD32 *dest, WORD32 *src, WORD32 len, WORD8 shift1,
                         WORD8 shift2);

VOID ixheaacd_scale_down_adj(WORD32 *dest, WORD32 *src, WORD32 len,
                             WORD8 shift1, WORD8 shift2);

#endif
