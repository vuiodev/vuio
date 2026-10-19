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
#include <stdlib.h>
#include <string.h>
#include <math.h>

#include "ixheaac_type_def.h"
#include "ixheaacd_bitbuffer.h"
#include "ixheaacd_interface.h"
#include "ixheaacd_tns_usac.h"
#include "ixheaacd_cnst.h"
#include "ixheaacd_acelp_info.h"
#include "ixheaacd_td_mdct.h"
#include "ixheaacd_sbrdecsettings.h"
#include "ixheaacd_info.h"
#include "ixheaacd_sbr_common.h"
#include "ixheaacd_drc_data_struct.h"
#include "ixheaacd_drc_dec.h"
#include "ixheaacd_sbrdecoder.h"
#include "ixheaacd_mps_polyphase.h"
#include "ixheaac_sbr_const.h"
#include "ixheaacd_ec_defines.h"
#include "ixheaacd_ec_struct_def.h"
#include "ixheaacd_main.h"
#include "ixheaacd_arith_dec.h"
#include "ixheaacd_windows.h"
#include "ixheaac_constants.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaacd_func_def.h"
#include "ixheaacd_acelp_com.h"

static PLATFORM_INLINE WORD32 ixheaacd_mult32_m(WORD32 a, WORD32 b) {
  WORD32 result;
  WORD64 temp_result;

  temp_result = (WORD64)a * (WORD64)b;
  result = (WORD32)(temp_result >> 31);

  return (result);
}

static VOID ixheaacd_weighted_synthesis_filter(WORD32 *a, WORD32 *ap) {
  WORD32 f;
  WORD32 i;
  ap[0] = a[0];
  f = IGAMMA1;
  for (i = 1; i <= ORDER; i++) {
    ap[i] = ixheaacd_mult32_m(f, a[i]);
    f = ixheaacd_mult32_m(f, IGAMMA1);
  }
  return;
}

static VOID ixheaacd_synthesis_tool(WORD32 a[], WORD32 x[], WORD32 l,
                                    WORD32 qshift, WORD32 *preshift) {
  WORD32 s;
  WORD32 i, j;

  for (i = 0; i < l; i++) {
    s = x[i];
    for (j = 1; j <= ORDER; j += 4) {
      s = ixheaac_sub32_sat(
          s, ixheaac_mul32_sh(a[j], x[i - j], (WORD8)(qshift)));
      s = ixheaac_sub32_sat(
          s, ixheaac_mul32_sh(a[j + 1], x[i - (j + 1)], (WORD8)(qshift)));
      s = ixheaac_sub32_sat(
          s, ixheaac_mul32_sh(a[j + 2], x[i - (j + 2)], (WORD8)(qshift)));
      s = ixheaac_sub32_sat(
          s, ixheaac_mul32_sh(a[j + 3], x[i - (j + 3)], (WORD8)(qshift)));
    }
    x[i] = s;
  }

  (*preshift)++;
  return;
}

VOID ixheaacd_fwd_alias_cancel_tool(
    ia_usac_data_struct *usac_data, ia_td_frame_data_struct *pstr_td_frame_data,
    WORD32 fac_length, FLOAT32 *lp_filt_coeff, WORD32 gain) {
  WORD32 i;
  FLOAT32 lp_filt_coeff_a[ORDER + 1];
  WORD32 qshift = 0;

  WORD32 *x_in = pstr_td_frame_data->fac_data;
  WORD32 *ptr_scratch = &usac_data->scratch_buffer[0];
  WORD32 *fac_signal = &usac_data->x_ac_dec[16];
  FLOAT32 fac_signal_flt[128 + 16];
  FLOAT32 *ptr_fac_signal_flt = &fac_signal_flt[16];
  WORD32 *ptr_overlap_buf =
      &(usac_data->overlap_data_ptr[usac_data->present_chan]
                                   [(usac_data->ccfl / 2) - fac_length]);

  memset(fac_signal - 16, 0, ORDER * sizeof(WORD32));

  ixheaacd_acelp_mdct(x_in, fac_signal, &qshift, fac_length, ptr_scratch);

  ixheaacd_lpc_coeff_wt_apply(lp_filt_coeff, lp_filt_coeff_a);

  for (i = 0; i < fac_length; i++)
    ptr_fac_signal_flt[i] =
        (FLOAT32)((FLOAT32)fac_signal[i] / (1 << (16 - qshift)));

  memset(ptr_fac_signal_flt - 16, 0, 16 * sizeof(FLOAT32));

  ixheaacd_synthesis_tool_float1(lp_filt_coeff_a, ptr_fac_signal_flt,
                                 fac_length);

  for (i = 0; i < fac_length; i++)
    fac_signal[i] = (WORD32)(ptr_fac_signal_flt[i] * (1 << (16 - qshift)));

  for (i = 0; i < fac_length; i++)
    ptr_overlap_buf[i] = ixheaac_add32_sat(
        ptr_overlap_buf[i],
        (WORD32)ixheaac_mul32_sh(fac_signal[i], gain, (WORD8)(16 - qshift)));

  return;
}

VOID ixheaacd_fr_alias_cnx_fix(WORD32 *x_in, WORD32 len, WORD32 fac_length,
                                 WORD32 *lp_filt_coeff, WORD32 *izir,
                                 WORD32 *fac_data_out, WORD8 *qshift1,
                                 WORD8 qshift2, WORD8 qshift3, WORD32 *preshift,
                                 WORD32 *ptr_scratch) {
  WORD32 i;
  const WORD32 *sine_window;
  WORD32 fac_window[2 * FAC_LENGTH];
  WORD32 lp_filt_coeff_a[ORDER + 1];

  if (fac_length == 48) {
    sine_window = ixheaacd_sine_win_96;
  } else if (fac_length == 64) {
    sine_window = ixheaacd_sine_win_128;
  } else if (fac_length == 96) {
    sine_window = ixheaacd_sine_win_192;
  } else {
    sine_window = ixheaacd_sine_win_256;
  }

  if (lp_filt_coeff != NULL && fac_data_out != NULL) {
    memset(fac_data_out - 16, 0, ORDER * sizeof(WORD32));
    ixheaacd_acelp_mdct(x_in, fac_data_out, preshift, fac_length,
                              ptr_scratch);

    ixheaacd_weighted_synthesis_filter(lp_filt_coeff, lp_filt_coeff_a);

    memset(fac_data_out + fac_length, 0, fac_length * sizeof(WORD32));

    ixheaacd_synthesis_tool(lp_filt_coeff_a, fac_data_out, 2 * fac_length,
                            qshift2, preshift);

    if (izir != NULL) {
      for (i = 0; i < fac_length; i++) {
        fac_window[i] = ixheaacd_mult32_m(
            sine_window[i], sine_window[(2 * fac_length) - 1 - i]);
        fac_window[fac_length + i] =
            2147483647 - ixheaacd_mult32_m(sine_window[fac_length + i],
                                           sine_window[fac_length + i]);
      }
      for (i = 0; i < fac_length; i++) {
        WORD32 temp1;
        WORD32 temp2;

        temp1 = ixheaac_mul32_sh(
            izir[1 + (len / 2) + i], fac_window[fac_length + i],
            (char)((qshift3 - *qshift1 + 31 + (WORD8)(*preshift))));

        temp2 = ixheaac_mul32_sh(
            izir[1 + (len / 2) - 1 - i], fac_window[fac_length - 1 - i],
            (char)((qshift3 - *qshift1 + 31 + (WORD8)(*preshift))));

        fac_data_out[i] =
            ixheaac_add32_sat3((fac_data_out[i] / 2), temp1, temp2);

        fac_data_out[fac_length + i] = (fac_data_out[fac_length + i] / 2);
      }
    }
  }

  return;
}
