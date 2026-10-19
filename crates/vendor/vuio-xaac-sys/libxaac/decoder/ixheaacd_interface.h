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
#ifndef IXHEAACD_INTERFACE_H
#define IXHEAACD_INTERFACE_H

enum {
  LN = 2048,
  SN = 256,
  LN2 = LN / 2,
  SN2 = SN / 2,
  LN4 = LN / 4,
  SN4 = SN / 4,
  NSHORT = LN / SN,
  MAX_SBK = NSHORT,

  MAXBANDS = 16 * NSHORT,
  MAXFAC = 121,
  MIDFAC = (MAXFAC - 1) / 2,
  SF_OFFSET = 100,

  LEN_TAG = 4,
  LEN_MAX_SFBS = 4,
  LEN_MAX_SFBL = 6,
  LEN_SAMP_IDX = 4,
  LEN_PC_COMM = 8,
};

WORD32 ixheaacd_dec_main(VOID *handle, WORD8 *inbuffer, WORD8 *outbuffer,
                         WORD32 *out_bytes, WORD32 frames_done, WORD32 pcmsize,
                         WORD32 *num_channel_out);

VOID ixheaacd_complex_fft_p3(WORD32 *xr, WORD32 *xi, WORD32 nlength,
                             WORD32 fft_mode, WORD32 *preshift);

VOID ixheaacd_complex_fft_p2_dec(WORD32 *xr, WORD32 *xi, WORD32 nlength,
                                 WORD32 fft_mode, WORD32 *preshift);

VOID ixheaacd_complex_fft_p2_armv7(WORD32 *xr, WORD32 *xi, WORD32 nlength,
                                   WORD32 fft_mode, WORD32 *preshift);

VOID ixheaacd_mps_synth_calc_fft(FLOAT32 *ptr_x, FLOAT32 *ptr_y, WORD32 nPass);

VOID ixheaacd_mps_complex_fft_64_armv7(WORD32 *ptr_x, WORD32 *fin_re,
                                       WORD32 *fin_im, WORD32 nlength);

VOID ixheaacd_mps_complex_fft_64_asm(const WORD32 *table, WORD32 nlength,
                                     WORD32 *ptr_x, WORD32 *ptr_y,
                                     const WORD8 *table2);

VOID ixheaacd_complex_ifft_p2_asm(const WORD32 *table, WORD32 nlength,
                                  WORD32 *ptr_x, WORD32 *ptr_y);

VOID ixheaacd_complex_fft_p2_asm(const WORD32 *table, WORD32 nlength,
                                 WORD32 *ptr_x, WORD32 *ptr_y);

VOID ixheaacd_mps_synt_pre_twiddle_armv7(WORD32 *ptr_in, const WORD32 *table_re,
                                         const WORD32 *table_im,
                                         WORD32 resolution);

VOID ixheaacd_mps_synt_post_twiddle_armv7(WORD32 *ptr_in,
                                          const WORD32 *table_re,
                                          const WORD32 *table_im,
                                          WORD32 resolution);

VOID ixheaacd_calc_pre_twid_dec(WORD32 *ptr_x, WORD32 *r_ptr, WORD32 *i_ptr,
                                WORD32 nlength, const WORD32 *cos_ptr,
                                const WORD32 *sin_ptr);

VOID ixheaacd_calc_pre_twid_armv7(WORD32 *ptr_x, WORD32 *r_ptr, WORD32 *i_ptr,
                                  WORD32 nlength, const WORD32 *cos_ptr,
                                  const WORD32 *sin_ptr);

VOID ixheaacd_calc_post_twid_dec(WORD32 *ptr_x, WORD32 *r_ptr, WORD32 *i_ptr,
                                 WORD32 nlength, const WORD32 *cos_ptr,
                                 const WORD32 *sin_ptr);

VOID ixheaacd_calc_post_twid_armv7(WORD32 *ptr_x, WORD32 *r_ptr, WORD32 *i_ptr,
                                   WORD32 nlength, const WORD32 *cos_ptr,
                                   const WORD32 *sin_ptr);

VOID ixheaacd_mps_synt_out_calc_dec(WORD32 resolution, FLOAT32 *out,
                                    FLOAT32 *state,
                                    const FLOAT32 *filter_coeff);

VOID ixheaacd_mps_synt_post_fft_twiddle_dec(WORD32 resolution, FLOAT32 *fin_re,
                                            FLOAT32 *fin_im, const FLOAT32 *table_re,
                                            const FLOAT32 *table_im, FLOAT32 *state);

VOID ixheaacd_mps_synt_out_calc_armv7(WORD32 resolution, WORD32 *out,
                                      WORD32 *state,
                                      const WORD32 *filter_coeff);
#endif /* #ifndef IXHEAACD_INTERFACE_H */
