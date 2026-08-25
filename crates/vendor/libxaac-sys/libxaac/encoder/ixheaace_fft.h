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
#ifndef PI
#define PI (3.141592653589795)
#endif

#define LS_TRANS_128 448  //((long_frame_len - FRAME_LEN_SHORT_128) / 2)
#define LS_TRANS_120 420

#define DIG_REV(i, m, j)                                      \
  do {                                                        \
    unsigned _ = (i);                                         \
    _ = ((_ & 0x33333333) << 2) | ((_ & ~0x33333333) >> 2);   \
    _ = ((_ & 0x0F0F0F0F) << 4) | ((_ & ~0x0F0F0F0F) >> 4);   \
    _ = ((_ & 0x00FF00FF) << 8) | ((_ & ~0x00FF00FF) >> 8);   \
    _ = ((_ & 0x0000FFFF) << 16) | ((_ & ~0x0000FFFF) >> 16); \
    (j) = _ >> (m);                                           \
  } while (0)

#define DIG_REV_NEW(i, m, j)                                \
  do {                                                      \
    unsigned _ = (i);                                       \
    _ = ((_ & 0x33333333) << 2) | ((_ & ~0x33333333) >> 2); \
    _ = ((_ & 0x0F0F0F0F) << 4) | ((_ & ~0x0F0F0F0F) >> 4); \
    _ = ((_ & 0x00FF00FF) << 8) | ((_ & ~0x00FF00FF) >> 8); \
    (j) = _ >> (m);                                         \
  } while (0)

typedef struct {
  FLOAT32 p_fft_p2_y[2048];
  FLOAT32 p_fft_p3_data_3[800];
  FLOAT32 p_fft_p3_y[2048];
} ixheaace_scratch_mem;

VOID ia_enhaacplus_enc_complex_fft(FLOAT32 *ptr_data, WORD32 len,
                                   ixheaace_scratch_mem *pstr_scratch);

VOID ia_eaacp_enc_pre_twiddle_aac(FLOAT32 *ptr_x, FLOAT32 *ptr_data, WORD32 n,
                                  const FLOAT32 *ptr_cos_array);

VOID ia_enhaacplus_enc_post_twiddle(FLOAT32 *ptr_out, FLOAT32 *ptr_x,
                                    const FLOAT32 *ptr_cos_sin_tbl, WORD m);

VOID ia_aac_ld_enc_mdct_480(FLOAT32 *ptr_inp, FLOAT32 *ptr_scratch, WORD32 mdct_flag,
                            ixheaace_mdct_tables *pstr_mdct_tables);

VOID ia_enhaacplus_enc_complex_fft_p2(FLOAT32 *ptr_x, WORD32 nlength,
                                      FLOAT32 *ptr_scratch_fft_p2_y);

VOID ixheaace_transform_real_lc_ld(FLOAT32 *ptr_mdct_delay_buffer, const FLOAT32 *ptr_time_signal,
                                   WORD32 ch_increment, FLOAT32 *ptr_real_out, WORD32 block_type,
                                   WORD32 frame_len, WORD8 *ptr_scratch);

VOID ia_enhaacplus_enc_transform_real_eld(FLOAT32 *ptr_mdct_delay_buffer,
                                          const FLOAT32 *ptr_time_signal, WORD32 ch_increment,
                                          FLOAT32 *ptr_real_out, WORD8 *ptr_shared_buffer5,
                                          WORD32 frame_len);

VOID ia_enhaacplus_enc_transform_real(FLOAT32 *ptr_mdct_delay_buffer,
                                      const FLOAT32 *ptr_time_signal, WORD32 ch_increment,
                                      FLOAT32 *ptr_real_out, ixheaace_mdct_tables *pstr_mdct_tab,
                                      FLOAT32 *ptr_shared_buffer1, WORD8 *ptr_shared_buffer5,
                                      WORD32 long_frame_len);