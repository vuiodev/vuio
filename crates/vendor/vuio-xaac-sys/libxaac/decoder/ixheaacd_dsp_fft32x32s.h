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
#ifndef IXHEAACD_DSP_FFT32X32S_H
#define IXHEAACD_DSP_FFT32X32S_H

VOID ixheaacd_inv_dit_fft_8pt_dec(WORD32 *x, WORD32 *real, WORD32 *imag);

VOID ixheaacd_inv_dit_fft_8pt_armv7(WORD32 *x, WORD32 *real, WORD32 *imag);

VOID ixheaacd_inv_dit_fft_8pt_armv8(WORD32 *x, WORD32 *real, WORD32 *imag);

#define CPLX_MPY_FFT(re, im, a, b, c, d) \
  do {                                   \
    re = ((a * c) - (b * d));            \
    im = ((a * d) + (b * c));            \
  } while (0)

#define CPLX_MPY_IFFT(re, im, a, b, c, d) \
  do {                                    \
    re = ((a * c) + (b * d));             \
    im = (-(a * d) + (b * c));            \
  } while (0)

#endif /* IXHEAACD_DSP_FFT32X32S_H */
