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

#include "ixheaac_type_def.h"
#include "ixheaace_error_codes.h"
#include "ixheaac_error_standards.h"
#include "ixheaace_mps_common_fix.h"
#include "ixheaace_mps_defines.h"
#include "ixheaace_mps_common_define.h"
#include "ixheaace_bitbuffer.h"

#include "ixheaace_psy_const.h"
#include "ixheaace_tns.h"
#include "ixheaace_tns_params.h"
#include "ixheaace_rom.h"
#include "ixheaace_common_rom.h"
#include "ixheaace_fft.h"

#include "ixheaace_mps_dct.h"
#include "ixheaace_mps_buf.h"
#include "ixheaace_mps_lib.h"
#include "ixheaace_mps_main_structure.h"

#include "ixheaace_mps_bitstream.h"
#include "ixheaace_mps_frame_windowing.h"
#include "ixheaace_mps_param_extract.h"
#include "ixheaace_mps_static_gain.h"
#include "ixheaace_mps_filter.h"
#include "ixheaace_mps_delay.h"
#include "ixheaace_mps_dmx_tdom_enh.h"
#include "ixheaace_mps_tools_rom.h"

IA_ERRORCODE ixheaace_mps_212_dct_iv(FLOAT32 *ptr_data, WORD32 length, WORD8 *ptr_scratch) {
  WORD32 sin_step = 0;
  WORD32 length_by_2 = length >> 1;
  WORD32 step, idx;
  FLOAT32 accu_1, accu_2, accu_3, accu_4;
  FLOAT32 *ptr_data_0;
  FLOAT32 *ptr_data_1;
  const ixheaace_cmplx_str *ptr_cmplx_twiddle;
  const ixheaace_cmplx_str *ptr_cmplx_sin_twiddle;
  ixheaace_scratch_mem *pstr_scratch = (ixheaace_scratch_mem *)ptr_scratch;

  if (length == 64) {
    ptr_cmplx_twiddle = sine_window_64;
    ptr_cmplx_sin_twiddle = sine_table_1024;
    sin_step = 32;
  } else {
    ptr_cmplx_twiddle = sine_window_32;
    ptr_cmplx_sin_twiddle = sine_table_1024;
    sin_step = 64;
  }

  ptr_data_0 = &ptr_data[0];
  ptr_data_1 = &ptr_data[length - 2];
  for (idx = 0; idx < length_by_2 - 1; idx += 2, ptr_data_0 += 2, ptr_data_1 -= 2) {
    accu_1 =
        (ptr_data_1[1] * ptr_cmplx_twiddle[idx].re) - (ptr_data_0[0] * ptr_cmplx_twiddle[idx].im);
    accu_2 =
        (ptr_data_1[1] * ptr_cmplx_twiddle[idx].im) + (ptr_data_0[0] * ptr_cmplx_twiddle[idx].re);
    accu_3 = (ptr_data_1[0] * ptr_cmplx_twiddle[idx + 1].re) -
             (ptr_data_0[1] * ptr_cmplx_twiddle[idx + 1].im);
    accu_4 = (ptr_data_1[0] * ptr_cmplx_twiddle[idx + 1].im) +
             (ptr_data_0[1] * ptr_cmplx_twiddle[idx + 1].re);

    ptr_data_0[0] = accu_2 / 4;
    ptr_data_0[1] = accu_1 / 4;
    ptr_data_1[0] = accu_4 / 4;
    ptr_data_1[1] = -(accu_3 / 4);
  }
  ia_enhaacplus_enc_complex_fft(ptr_data, length_by_2, pstr_scratch);
  if (length_by_2 == 32) {
    for (idx = 0; idx < length; idx++) {
      ptr_data[idx] = (ptr_data[idx] / (1 << (4)));
    }
  } else {
    for (idx = 0; idx < length; idx++) {
      ptr_data[idx] = (ptr_data[idx] / (1 << (3)));
    }
  }

  ptr_data_0 = &ptr_data[0];
  ptr_data_1 = &ptr_data[length - 2];
  accu_1 = ptr_data_1[0];
  accu_2 = ptr_data_1[1];
  ptr_data_1[1] = -ptr_data_0[1];

  for (step = sin_step, idx = 1; idx<(length_by_2 + 1)>> 1; idx++, step += sin_step) {
    ixheaace_cmplx_str twd = ptr_cmplx_sin_twiddle[step];
    accu_3 = (accu_1 * twd.re) - (accu_2 * twd.im);
    accu_4 = (accu_1 * twd.im) + (accu_2 * twd.re);
    ptr_data_0[1] = accu_3;
    ptr_data_1[0] = accu_4;

    ptr_data_0 += 2;
    ptr_data_1 -= 2;

    accu_3 = (ptr_data_0[1] * twd.re) - (ptr_data_0[0] * twd.im);
    accu_4 = (ptr_data_0[1] * twd.im) + (ptr_data_0[0] * twd.re);
    accu_1 = ptr_data_1[0];
    accu_2 = ptr_data_1[1];

    ptr_data_1[1] = -accu_3;
    ptr_data_0[0] = accu_4;
  }
  accu_1 = (accu_1 * SQUARE_ROOT_TWO);
  accu_2 = (accu_2 * SQUARE_ROOT_TWO);

  ptr_data_1[0] = (accu_1 + accu_2) / 2;
  ptr_data_0[1] = (accu_1 - accu_2) / 2;

  return IA_NO_ERROR;
}

IA_ERRORCODE ixheaace_mps_212_dst_iv(FLOAT32 *ptr_data, WORD32 length, WORD8 *ptr_scratch) {
  WORD32 sin_step = 0;
  WORD32 step, idx;
  WORD32 length_by_2 = length >> 1;
  FLOAT32 accu_1, accu_2, accu_3, accu_4;
  FLOAT32 *ptr_data_0;
  FLOAT32 *ptr_data_1;
  const ixheaace_cmplx_str *ptr_cmplx_twiddle;
  const ixheaace_cmplx_str *ptr_cmplx_sin_twiddle;
  ixheaace_scratch_mem *pstr_scratch = (ixheaace_scratch_mem *)ptr_scratch;
  if (length == 64) {
    ptr_cmplx_twiddle = sine_window_64;
    ptr_cmplx_sin_twiddle = sine_table_1024;
    sin_step = 32;
  } else {
    ptr_cmplx_twiddle = sine_window_32;
    ptr_cmplx_sin_twiddle = sine_table_1024;
    sin_step = 64;
  }

  ptr_data_0 = &ptr_data[0];
  ptr_data_1 = &ptr_data[length - 2];
  for (idx = 0; idx < length_by_2 - 1; idx += 2, ptr_data_0 += 2, ptr_data_1 -= 2) {
    accu_1 = (ptr_data_1[1] * ptr_cmplx_twiddle[idx].re) -
             ((-ptr_data_0[0]) * ptr_cmplx_twiddle[idx].im);
    accu_2 = (ptr_data_1[1] * ptr_cmplx_twiddle[idx].im) +
             ((-ptr_data_0[0]) * ptr_cmplx_twiddle[idx].re);
    accu_3 = ((-ptr_data_1[0]) * ptr_cmplx_twiddle[idx + 1].re) -
             (ptr_data_0[1] * ptr_cmplx_twiddle[idx + 1].im);
    accu_4 = ((-ptr_data_1[0]) * ptr_cmplx_twiddle[idx + 1].im) +
             (ptr_data_0[1] * ptr_cmplx_twiddle[idx + 1].re);
    ptr_data_0[0] = accu_2 / 4;
    ptr_data_0[1] = accu_1 / 4;
    ptr_data_1[0] = accu_4 / 4;
    ptr_data_1[1] = -(accu_3) / 4;
  }
  ia_enhaacplus_enc_complex_fft(ptr_data, length_by_2, pstr_scratch);
  if (length_by_2 == 32) {
    for (idx = 0; idx < length; idx++) {
      ptr_data[idx] = (ptr_data[idx] / (1 << (4)));
    }
  } else {
    for (idx = 0; idx < length; idx++) {
      ptr_data[idx] = (ptr_data[idx] / (1 << (3)));
    }
  }
  ptr_data_0 = &ptr_data[0];
  ptr_data_1 = &ptr_data[length - 2];
  accu_1 = ptr_data_1[0];
  accu_2 = ptr_data_1[1];
  ptr_data_1[1] = -ptr_data_0[0];
  ptr_data_0[0] = ptr_data_0[1];

  for (step = sin_step, idx = 1; idx<(length_by_2 + 1)>> 1; idx++, step += sin_step) {
    ixheaace_cmplx_str twd = ptr_cmplx_sin_twiddle[step];

    accu_3 = (accu_1 * twd.re) - (accu_2 * twd.im);
    accu_4 = (accu_1 * twd.im) + (accu_2 * twd.re);
    ptr_data_1[0] = -accu_3;
    ptr_data_0[1] = -accu_4;

    ptr_data_0 += 2;
    ptr_data_1 -= 2;

    accu_3 = (ptr_data_0[1] * twd.re) - (ptr_data_0[0] * twd.im);
    accu_4 = (ptr_data_0[1] * twd.im) + (ptr_data_0[0] * twd.re);
    accu_1 = ptr_data_1[0];
    accu_2 = ptr_data_1[1];

    ptr_data_0[0] = accu_3;
    ptr_data_1[1] = -accu_4;
  }

  accu_1 = (accu_1 * SQUARE_ROOT_TWO);
  accu_2 = (accu_2 * SQUARE_ROOT_TWO);
  ptr_data_1[0] = (accu_1 + accu_2) / 2;
  ptr_data_0[1] = (accu_1 - accu_2) / 2;

  return IA_NO_ERROR;
}
