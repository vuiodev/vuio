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

#define C70 (-0.1666667014f)  //(cos(u) + cos(2 * u) + cos(3 * u)) / 3;
#define C71 (0.7901564837f)   //(2 * cos(u) - cos(2 * u) - cos(3 * u)) / 3;
#define C72 (0.0558542535f)   //(cos(u) - 2 * cos(2 * u) + cos(3 * u)) / 3;
#define C73 (0.7343022227f)   //(cos(u) + cos(2 * u) - 2 * cos(3 * u)) / 3;
#define C74 (-0.4409585893f)  //(sin(u) + sin(2 * u) - sin(3 * u)) / 3;
#define C75 (-0.3408728838f)  //(2 * sin(u) - sin(2 * u) + sin(3 * u)) / 3;
#define C76 (0.5339693427f)   //(sin(u) - 2 * sin(2 * u) - sin(3 * u)) / 3;
#define C77 (-0.8748422265f)  //(sin(u) + sin(2 * u) + 2 * sin(3 * u)) / 3;

#define CPLX_MPY_FFT(re, im, a, b, c, d) \
  {                                      \
    re = ((a * c) - (b * d));            \
    im = ((a * d) + (b * c));            \
  }

IA_ERRORCODE iusace_fft_based_mdct(FLOAT64 *ptr_in, FLOAT64 *ptr_out, WORD32 npoints,
                                   const WORD32 tx_flag, iusace_scratch_mem *pstr_scratch);

VOID iusace_complex_fft(FLOAT32 *data, WORD32 nlength, iusace_scratch_mem *pstr_scratch);
