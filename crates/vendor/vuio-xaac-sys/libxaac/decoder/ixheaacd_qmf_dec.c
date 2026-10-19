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
#include <string.h>

#include "ixheaacd_sbr_common.h"
#include "ixheaac_type_def.h"

#include "ixheaac_constants.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops16.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaac_basic_ops.h"

#include "ixheaacd_intrinsics.h"
#include "ixheaacd_common_rom.h"
#include "ixheaacd_bitbuffer.h"
#include "ixheaacd_sbrdecsettings.h"
#include "ixheaacd_sbr_scale.h"
#include "ixheaacd_lpp_tran.h"
#include "ixheaacd_env_extr_part.h"
#include "ixheaacd_sbr_rom.h"
#include "ixheaacd_hybrid.h"
#include "ixheaacd_ps_dec.h"
#include "ixheaacd_env_extr.h"
#include "ixheaacd_qmf_dec.h"

#include "ixheaac_basic_op.h"
#include "ixheaacd_env_calc.h"
#include "ixheaacd_interface.h"

#include "ixheaacd_function_selector.h"
#include "ixheaacd_audioobjtypes.h"

#define DCT3_LEN (32)
#define DCT2_LEN (64)

#define LP_SHIFT_VAL 7
#define HQ_SHIFT_64 4
#define RADIXSHIFT 1
#define ROUNDING_SPECTRA 1
#define HQ_SHIFT_VAL 4

static PLATFORM_INLINE WORD32 ixheaacd_mult32x32in32_shift25(WORD32 a,
                                                             WORD32 b) {
  WORD32 result;
  WORD64 temp_result;

  temp_result = (WORD64)a * (WORD64)b;

  result = (WORD32)(temp_result >> 25);

  return (result);
}

VOID ixheaacd_pretwdct2(WORD32 *inp, WORD32 *out_fwd) {
  WORD32 n;
  WORD32 *out_rev = out_fwd + DCT2_LEN - 1;

  for (n = 0; n < DCT2_LEN / 2; n++) {
    *out_fwd = *inp;
    inp++;
    *out_rev = *inp;
    out_fwd++;

    out_rev--;
    inp++;
  }

  return;
}

static PLATFORM_INLINE VOID ixheaacd_pretwdct2_32(WORD32 *inp, WORD32 *out_fwd,
                                                  int dct2_len) {
  WORD32 n;

  WORD32 *out_rev = out_fwd + dct2_len - 1;
  for (n = dct2_len / 2 - 1; n >= 0; n--) {
    *out_fwd = *inp;
    inp++;
    *out_rev = *inp;
    out_fwd++;

    out_rev--;
    inp++;
  }

  return;
}

VOID ixheaacd_fftposttw(WORD32 *out,
                        ia_qmf_dec_tables_struct *qmf_dec_tables_ptr) {
  int k;
  WORD32 *p_out_fwd, *ptr_out_rev;
  const WORD16 *twidle_fwd, *twidle_rev;
  WORD32 in1, in2, val1, val2;

  twidle_fwd = qmf_dec_tables_ptr->post_fft_tbl + 1;
  twidle_rev = qmf_dec_tables_ptr->post_fft_tbl + 15;

  p_out_fwd = out;
  ptr_out_rev = out + DCT2_LEN - 1;

  in1 = ((*p_out_fwd++) << 1);
  val1 = ((*p_out_fwd--) << 1);

  *p_out_fwd++ = in1;
  *p_out_fwd++ = val1;

  for (k = 1; k <= DCT2_LEN / 4; k++) {
    WORD32 temp[4];
    WORD16 twid_re, twid_im;

    temp[0] = *p_out_fwd++;
    temp[1] = *p_out_fwd--;
    temp[3] = *ptr_out_rev--;
    temp[2] = *ptr_out_rev++;

    in2 = ixheaac_sub32_sat(temp[3], temp[1]);
    in1 = ixheaac_add32_sat(temp[3], temp[1]);

    temp[1] = ixheaac_sub32_sat(temp[0], temp[2]);
    temp[3] = ixheaac_add32_sat(temp[0], temp[2]);

    twid_re = *twidle_fwd++;
    twid_im = *twidle_rev--;
    val1 = ixheaac_mult32x16in32(in1, twid_re) -
           ixheaac_mult32x16in32(temp[1], twid_im);
    val2 = ixheaac_mult32x16in32(temp[1], twid_re) +
           ixheaac_mult32x16in32(in1, twid_im);
    val1 = val1 << 1;
    val2 = val2 << 1;

    *p_out_fwd++ = ixheaac_add32_sat(temp[3], val1);
    *p_out_fwd++ = ixheaac_add32_sat(in2, val2);

    *ptr_out_rev-- = ixheaac_sub32_sat(val2, in2);
    *ptr_out_rev-- = ixheaac_sub32_sat(temp[3], val1);
  }

  return;
}

VOID ixheaacd_posttwdct2(WORD32 *inp, WORD16 *out_fwd,
                         ia_qmf_dec_tables_struct *qmf_dec_tables_ptr) {
  WORD32 k;
  WORD32 inp_re, inp_im, out_re, out_im, last_val, out_re1;
  WORD16 *out_fwd2, *out_rev2, *out_rev;
  WORD16 twid_re, twid_im;
  const WORD16 *twidle_fwd;
  WORD16 re1, im1, im2;

  out_rev = out_fwd + DCT2_LEN - 1;
  out_rev2 = out_fwd - 1;
  out_fwd2 = out_fwd + 65;
  out_re = *inp++;
  out_im = *inp++;
  out_re1 =
      ixheaac_sat64_32(ixheaac_add64((WORD64)out_re, (WORD64)out_im) >> 1);
  re1 = ixheaac_round16(ixheaac_shl32(out_re1, (5 - 1)));

  *out_fwd++ = re1;

  last_val = ixheaac_sub32_sat(out_re, out_im);

  twidle_fwd = qmf_dec_tables_ptr->dct23_tw + 2;
  for (k = DCT2_LEN / 2 - 2; k >= 0; k--) {
    inp_re = *inp++;
    inp_im = *inp++;

    twid_re = *twidle_fwd++;
    twid_im = *twidle_fwd++;
    out_re = ixheaac_sub32_sat(ixheaac_mult32x16in32(inp_re, twid_re),
                                ixheaac_mult32x16in32(inp_im, twid_im));
    out_im = ixheaac_add32_sat(ixheaac_mult32x16in32(inp_im, twid_re),
                                ixheaac_mult32x16in32(inp_re, twid_im));
    re1 = ixheaac_round16(ixheaac_shl32(out_re, (5 - 1)));
    im1 = ixheaac_round16(ixheaac_shl32(out_im, (5 - 1)));
    im2 = ixheaac_negate16(im1);

    *out_fwd++ = re1;
    *out_rev2-- = re1;
    *out_rev-- = im1;
    *out_fwd2++ = im2;
  }
  twid_re = *twidle_fwd++;

  out_re = ixheaac_mult32x16in32(last_val, twid_re);
  re1 = ixheaac_round16(ixheaac_shl32(out_re, (5 - 1)));

  *out_fwd++ = re1;
  *out_rev2-- = re1;

  return;
}

static PLATFORM_INLINE VOID ixheaacd_fftposttw_32(
    WORD32 *out, int dct2_len, ia_qmf_dec_tables_struct *qmf_dec_tables_ptr) {
  int k;
  WORD32 *ptr_out_fwd, *ptr_out_rev;
  const WORD16 *twidle_fwd, *twidle_rev;
  WORD32 in1, in2, val1, val2;

  twidle_fwd = qmf_dec_tables_ptr->post_fft_tbl + 2;
  twidle_rev = qmf_dec_tables_ptr->post_fft_tbl + 14;

  ptr_out_fwd = out;
  ptr_out_rev = out + dct2_len - 1;

  in1 = ((*ptr_out_fwd++) << 1);
  val1 = ((*ptr_out_fwd--) << 1);

  *ptr_out_fwd++ = in1;
  *ptr_out_fwd++ = val1;

  for (k = dct2_len / 4 - 1; k >= 0; k--) {
    WORD32 temp0, temp1, temp2, temp3;
    WORD16 twid_re, twid_im;

    temp0 = *ptr_out_fwd++;
    temp1 = *ptr_out_fwd--;
    temp3 = *ptr_out_rev--;
    temp2 = *ptr_out_rev++;

    in1 = ixheaac_add32_sat(temp1, temp3);
    in2 = ixheaac_sub32_sat(temp3, temp1);

    temp1 = ixheaac_sub32_sat(temp0, temp2);
    temp3 = ixheaac_add32_sat(temp0, temp2);

    twid_re = *twidle_fwd;
    twidle_fwd += 2;

    twid_im = *twidle_rev;
    twidle_rev -= 2;

    val1 = ixheaac_mult32x16in32(in1, twid_re) -
           ixheaac_mult32x16in32(temp1, twid_im);
    val2 = ixheaac_mult32x16in32(temp1, twid_re) +
           ixheaac_mult32x16in32(in1, twid_im);

    val1 = val1 << 1;
    val2 = val2 << 1;

    *ptr_out_fwd++ = ixheaac_add32_sat(temp3, val1);
    *ptr_out_fwd++ = ixheaac_add32_sat(in2, val2);

    *ptr_out_rev-- = ixheaac_sub32_sat(val2, in2);
    *ptr_out_rev-- = ixheaac_sub32_sat(temp3, val1);
  }

  return;
}

static PLATFORM_INLINE VOID
ixheaacd_posttwdct2_32(WORD32 *inp, WORD16 *out_fwd,
                       ia_qmf_dec_tables_struct *qmf_dec_tables_ptr) {
  int k;
  WORD32 inp_re, out_re, out_im, last_val, out_re1;
  WORD16 *out_rev, *out_rev2, *out_fwd2;
  WORD16 twid_re, twid_im;
  const WORD16 *twidle_fwd;
  WORD16 re1, im1, im2;
  WORD32 rounding_fac = 0x8000;

  out_rev = out_fwd + 32 - 1;
  out_rev2 = out_fwd - 1;
  out_fwd2 = out_fwd + 32 + 1;
  out_fwd[32] = 0;

  out_re = *inp++;
  out_im = *inp++;

  out_re1 =
      ixheaac_sat64_32(ixheaac_add64((WORD64)out_re, (WORD64)out_im) >> 1);
  re1 = ixheaac_round16(ixheaac_shl32_sat(out_re1, (5 - 1)));
  *out_fwd++ = re1;
  last_val = ixheaac_sub32_sat(out_re, out_im);

  twidle_fwd = qmf_dec_tables_ptr->dct23_tw + 4;
  for (k = 14; k >= 0; k--) {
    WORD32 temp1, temp2;
    inp_re = *inp++;
    twid_re = *twidle_fwd++;
    twid_im = *twidle_fwd;
    twidle_fwd += 3;

    temp1 = ixheaac_mult32x16in32(inp_re, twid_re);
    temp2 = ixheaac_mult32x16in32(inp_re, twid_im);

    inp_re = *inp++;

    out_re = ixheaac_sub32_sat(temp1, ixheaac_mult32x16in32(inp_re, twid_im));
    out_im = ixheaac_add32_sat(ixheaac_mult32x16in32(inp_re, twid_re), temp2);

    out_re = ixheaac_add32_sat(out_re, out_re);
    out_im = ixheaac_add32_sat(out_im, out_im);
    out_re = ixheaac_add32_sat(out_re, out_re);
    out_im = ixheaac_add32_sat(out_im, out_im);
    out_re = ixheaac_add32_sat(out_re, out_re);
    out_im = ixheaac_add32_sat(out_im, out_im);
    out_re = ixheaac_add32_sat(out_re, out_re);
    out_im = ixheaac_add32_sat(out_im, out_im);
    out_re = ixheaac_add32_sat(out_re, rounding_fac);
    out_im = ixheaac_add32_sat(out_im, rounding_fac);
    re1 = (out_re >> 16);
    im1 = (out_im >> 16);
    im2 = ixheaac_negate16(im1);

    *out_fwd++ = re1;
    *out_rev2-- = re1;
    *out_rev-- = im1;
    *out_fwd2++ = im2;
  }
  twid_re = *twidle_fwd++;

  out_re = ixheaac_mult32x16in32(last_val, twid_re);
  re1 = ixheaac_round16(ixheaac_shl32_sat(out_re, (5 - 1)));
  *out_fwd++ = re1;
  *out_rev2-- = re1;

  return;
}

VOID ixheaacd_dct2_32(WORD32 *inp, WORD32 *out,
                      ia_qmf_dec_tables_struct *qmf_dec_tables_ptr,
                      WORD16 *filter_states) {
  WORD32 *output;

  output = out + 16;
  filter_states = filter_states + 16;
  ixheaacd_pretwdct2_32(inp, output, 32);

  ixheaacd_radix4bfly(qmf_dec_tables_ptr->w_16, output, 1, 4);
  ixheaacd_postradixcompute4(inp, output, qmf_dec_tables_ptr->dig_rev_table4_16,
                             16);
  ixheaacd_fftposttw_32(inp, 32, qmf_dec_tables_ptr);

  ixheaacd_posttwdct2_32(inp, filter_states, qmf_dec_tables_ptr);

  return;
}

VOID ixheaacd_sbr_qmfanal32_winadd_eld_mps(WORD32 *inp1, WORD32 *inp2,
                                           const WORD32 *p_qmf1,
                                           const WORD32 *p_qmf2,
                                           WORD32 *p_out) {
  WORD32 n;
  WORD32 resolution = 64;

  for (n = 0; n < 64; n += 2) {
    WORD32 accu;
    accu = ixheaac_mul32_sh(inp1[n + 0], p_qmf1[(n + 0)], 31);
    accu = ixheaac_add32_sat(
        accu, ixheaac_mul32_sh(inp1[n + 2 * resolution],
                                p_qmf1[(n + 2 * resolution)], 31));
    accu = ixheaac_add32_sat(
        accu, ixheaac_mul32_sh(inp1[n + 4 * resolution],
                                p_qmf1[(n + 4 * resolution)], 31));
    accu = ixheaac_add32_sat(
        accu, ixheaac_mul32_sh(inp1[n + 6 * resolution],
                                p_qmf1[(n + 6 * resolution)], 31));
    accu = ixheaac_add32_sat(
        accu, ixheaac_mul32_sh(inp1[n + 8 * resolution],
                                p_qmf1[(n + 8 * resolution)], 31));
    p_out[n] = accu;

    accu = ixheaac_mul32_sh(inp1[n + 1 + 0], p_qmf1[(n + 1 + 0)], 31);
    accu = ixheaac_add32_sat(
        accu, ixheaac_mul32_sh(inp1[n + 1 + 2 * resolution],
                                p_qmf1[(n + 1 + 2 * resolution)], 31));
    accu = ixheaac_add32_sat(
        accu, ixheaac_mul32_sh(inp1[n + 1 + 4 * resolution],
                                p_qmf1[(n + 1 + 4 * resolution)], 31));
    accu = ixheaac_add32_sat(
        accu, ixheaac_mul32_sh(inp1[n + 1 + 6 * resolution],
                                p_qmf1[(n + 1 + 6 * resolution)], 31));
    accu = ixheaac_add32_sat(
        accu, ixheaac_mul32_sh(inp1[n + 1 + 8 * resolution],
                                p_qmf1[(n + 1 + 8 * resolution)], 31));
    p_out[n + 1] = accu;

    accu = ixheaac_mul32_sh(inp2[n + 0], p_qmf2[(n + 0)], 31);
    accu = ixheaac_add32_sat(
        accu, ixheaac_mul32_sh(inp2[n + 2 * resolution],
                                p_qmf2[(n + 2 * resolution)], 31));
    accu = ixheaac_add32_sat(
        accu, ixheaac_mul32_sh(inp2[n + 4 * resolution],
                                p_qmf2[(n + 4 * resolution)], 31));
    accu = ixheaac_add32_sat(
        accu, ixheaac_mul32_sh(inp2[n + 6 * resolution],
                                p_qmf2[(n + 6 * resolution)], 31));
    accu = ixheaac_add32_sat(
        accu, ixheaac_mul32_sh(inp2[n + 8 * resolution],
                                p_qmf2[(n + 8 * resolution)], 31));
    p_out[n + 64] = accu;

    accu = ixheaac_mul32_sh(inp2[n + 1 + 0], p_qmf2[(n + 1 + 0)], 31);
    accu = ixheaac_add32_sat(
        accu, ixheaac_mul32_sh(inp2[n + 1 + 2 * resolution],
                                p_qmf2[(n + 1 + 2 * resolution)], 31));
    accu = ixheaac_add32_sat(
        accu, ixheaac_mul32_sh(inp2[n + 1 + 4 * resolution],
                                p_qmf2[(n + 1 + 4 * resolution)], 31));
    accu = ixheaac_add32_sat(
        accu, ixheaac_mul32_sh(inp2[n + 1 + 6 * resolution],
                                p_qmf2[(n + 1 + 6 * resolution)], 31));
    accu = ixheaac_add32_sat(
        accu, ixheaac_mul32_sh(inp2[n + 1 + 8 * resolution],
                                p_qmf2[(n + 1 + 8 * resolution)], 31));
    p_out[n + 1 + 64] = accu;
  }
}

VOID ixheaacd_sbr_qmfanal32_winadd_eld_32(WORD32 *inp1, WORD32 *inp2,
                                          const WORD32 *p_qmf1,
                                          const WORD32 *p_qmf2, WORD32 *p_out) {
  WORD32 n;

  for (n = 0; n < 32; n += 2) {
    WORD32 accu;
    accu = ixheaac_mul32_sh(inp1[n + 0], p_qmf1[(n + 0)], 31);
    accu = ixheaac_add32_sat(
        accu, ixheaac_mul32_sh(inp1[n + 64], p_qmf1[(n + 64)], 31));
    accu = ixheaac_add32_sat(
        accu, ixheaac_mul32_sh(inp1[n + 128], p_qmf1[(n + 128)], 31));
    accu = ixheaac_add32_sat(
        accu, ixheaac_mul32_sh(inp1[n + 192], p_qmf1[(n + 192)], 31));
    accu = ixheaac_add32_sat(
        accu, ixheaac_mul32_sh(inp1[n + 256], p_qmf1[(n + 256)], 31));
    p_out[n] = accu;

    accu = ixheaac_mul32_sh(inp1[n + 1 + 0], p_qmf1[(n + 1 + 0)], 31);
    accu = ixheaac_add32_sat(
        accu, ixheaac_mul32_sh(inp1[n + 1 + 64], p_qmf1[(n + 1 + 64)], 31));
    accu = ixheaac_add32_sat(
        accu, ixheaac_mul32_sh(inp1[n + 1 + 128], p_qmf1[(n + 1 + 128)], 31));
    accu = ixheaac_add32_sat(
        accu, ixheaac_mul32_sh(inp1[n + 1 + 192], p_qmf1[(n + 1 + 192)], 31));
    accu = ixheaac_add32_sat(
        accu, ixheaac_mul32_sh(inp1[n + 1 + 256], p_qmf1[(n + 1 + 256)], 31));
    p_out[n + 1] = accu;

    accu = ixheaac_mul32_sh(inp2[n + 0], p_qmf2[(n + 0)], 31);
    accu = ixheaac_add32_sat(
        accu, ixheaac_mul32_sh(inp2[n + 64], p_qmf2[(n + 64)], 31));
    accu = ixheaac_add32_sat(
        accu, ixheaac_mul32_sh(inp2[n + 128], p_qmf2[(n + 128)], 31));
    accu = ixheaac_add32_sat(
        accu, ixheaac_mul32_sh(inp2[n + 192], p_qmf2[(n + 192)], 31));
    accu = ixheaac_add32_sat(
        accu, ixheaac_mul32_sh(inp2[n + 256], p_qmf2[(n + 256)], 31));
    p_out[n + 32] = accu;

    accu = ixheaac_mul32_sh(inp2[n + 1 + 0], p_qmf2[(n + 1 + 0)], 31);
    accu = ixheaac_add32_sat(
        accu, ixheaac_mul32_sh(inp2[n + 1 + 64], p_qmf2[(n + 1 + 64)], 31));
    accu = ixheaac_add32_sat(
        accu, ixheaac_mul32_sh(inp2[n + 1 + 128], p_qmf2[(n + 1 + 128)], 31));
    accu = ixheaac_add32_sat(
        accu, ixheaac_mul32_sh(inp2[n + 1 + 192], p_qmf2[(n + 1 + 192)], 31));
    accu = ixheaac_add32_sat(
        accu, ixheaac_mul32_sh(inp2[n + 1 + 256], p_qmf2[(n + 1 + 256)], 31));
    p_out[n + 1 + 32] = accu;
  }
}

VOID ixheaacd_sbr_qmfanal32_winadd_eld(WORD16 *inp1, WORD16 *inp2,
                                       const WORD16 *p_qmf1,
                                       const WORD16 *p_qmf2, WORD32 *p_out) {
  WORD32 n;

  for (n = 0; n < 32; n += 2) {
    WORD32 accu;
    accu = ixheaac_mult16x16in32(inp1[n + 0], p_qmf1[(n + 0)]);
    accu = ixheaac_add32_sat(
        accu, ixheaac_mult16x16in32(inp1[n + 64], p_qmf1[(n + 64)]));
    accu = ixheaac_add32_sat(
        accu, ixheaac_mult16x16in32(inp1[n + 128], p_qmf1[(n + 128)]));
    accu = ixheaac_add32_sat(
        accu, ixheaac_mult16x16in32(inp1[n + 192], p_qmf1[(n + 192)]));
    accu = ixheaac_add32_sat(
        accu, ixheaac_mult16x16in32(inp1[n + 256], p_qmf1[(n + 256)]));
    p_out[n] = accu;

    accu = ixheaac_mult16x16in32(inp1[n + 1 + 0], p_qmf1[(n + 1 + 0)]);
    accu = ixheaac_add32_sat(
        accu, ixheaac_mult16x16in32(inp1[n + 1 + 64], p_qmf1[(n + 1 + 64)]));
    accu = ixheaac_add32_sat(
        accu, ixheaac_mult16x16in32(inp1[n + 1 + 128], p_qmf1[(n + 1 + 128)]));
    accu = ixheaac_add32_sat(
        accu, ixheaac_mult16x16in32(inp1[n + 1 + 192], p_qmf1[(n + 1 + 192)]));
    accu = ixheaac_add32_sat(
        accu, ixheaac_mult16x16in32(inp1[n + 1 + 256], p_qmf1[(n + 1 + 256)]));
    p_out[n + 1] = accu;

    accu = ixheaac_mult16x16in32(inp2[n + 0], p_qmf2[(n + 0)]);
    accu = ixheaac_add32_sat(
        accu, ixheaac_mult16x16in32(inp2[n + 64], p_qmf2[(n + 64)]));
    accu = ixheaac_add32_sat(
        accu, ixheaac_mult16x16in32(inp2[n + 128], p_qmf2[(n + 128)]));
    accu = ixheaac_add32_sat(
        accu, ixheaac_mult16x16in32(inp2[n + 192], p_qmf2[(n + 192)]));
    accu = ixheaac_add32_sat(
        accu, ixheaac_mult16x16in32(inp2[n + 256], p_qmf2[(n + 256)]));
    p_out[n + 32] = accu;

    accu = ixheaac_mult16x16in32(inp2[n + 1 + 0], p_qmf2[(n + 1 + 0)]);
    accu = ixheaac_add32_sat(
        accu, ixheaac_mult16x16in32(inp2[n + 1 + 64], p_qmf2[(n + 1 + 64)]));
    accu = ixheaac_add32_sat(
        accu, ixheaac_mult16x16in32(inp2[n + 1 + 128], p_qmf2[(n + 1 + 128)]));
    accu = ixheaac_add32_sat(
        accu, ixheaac_mult16x16in32(inp2[n + 1 + 192], p_qmf2[(n + 1 + 192)]));
    accu = ixheaac_add32_sat(
        accu, ixheaac_mult16x16in32(inp2[n + 1 + 256], p_qmf2[(n + 1 + 256)]));
    p_out[n + 1 + 32] = accu;
  }
}

VOID ixheaacd_esbr_qmfanal32_winadd(WORD32 *inp1, WORD32 *inp2, WORD32 *p_qmf1,
                                    WORD32 *p_qmf2, WORD32 *p_out,
                                    WORD32 num_band_anal_qmf) {
  WORD32 n;
  WORD64 accu;

  if (num_band_anal_qmf == 32) {
    for (n = 0; n < num_band_anal_qmf; n += 2) {
      accu = ixheaac_mult64(inp1[n + 0], p_qmf1[2 * (n + 0)]);
      accu = ixheaac_add64(
          accu, ixheaac_mult64(inp1[n + 2 * num_band_anal_qmf],
                                p_qmf1[2 * (n + 2 * num_band_anal_qmf)]));
      accu = ixheaac_add64(
          accu, ixheaac_mult64(inp1[n + 4 * num_band_anal_qmf],
                                p_qmf1[2 * (n + 4 * num_band_anal_qmf)]));
      accu = ixheaac_add64(
          accu, ixheaac_mult64(inp1[n + 6 * num_band_anal_qmf],
                                p_qmf1[2 * (n + 6 * num_band_anal_qmf)]));
      accu = ixheaac_add64(
          accu, ixheaac_mult64(inp1[n + 8 * num_band_anal_qmf],
                                p_qmf1[2 * (n + 8 * num_band_anal_qmf)]));
      p_out[n] = (WORD32)(accu >> 31);

      accu = ixheaac_mult64(inp1[n + 1 + 0], p_qmf1[2 * (n + 1 + 0)]);
      accu = ixheaac_add64(
          accu, ixheaac_mult64(inp1[n + 1 + 2 * num_band_anal_qmf],
                                p_qmf1[2 * (n + 1 + 2 * num_band_anal_qmf)]));
      accu = ixheaac_add64(
          accu, ixheaac_mult64(inp1[n + 1 + 4 * num_band_anal_qmf],
                                p_qmf1[2 * (n + 1 + 4 * num_band_anal_qmf)]));
      accu = ixheaac_add64(
          accu, ixheaac_mult64(inp1[n + 1 + 6 * num_band_anal_qmf],
                                p_qmf1[2 * (n + 1 + 6 * num_band_anal_qmf)]));
      accu = ixheaac_add64(
          accu, ixheaac_mult64(inp1[n + 1 + 8 * num_band_anal_qmf],
                                p_qmf1[2 * (n + 1 + 8 * num_band_anal_qmf)]));
      p_out[n + 1] = (WORD32)(accu >> 31);

      accu = ixheaac_mult64(inp2[n + 0], p_qmf2[2 * (n + 0)]);
      accu = ixheaac_add64(
          accu, ixheaac_mult64(inp2[n + 2 * num_band_anal_qmf],
                                p_qmf2[2 * (n + 2 * num_band_anal_qmf)]));
      accu = ixheaac_add64(
          accu, ixheaac_mult64(inp2[n + 4 * num_band_anal_qmf],
                                p_qmf2[2 * (n + 4 * num_band_anal_qmf)]));
      accu = ixheaac_add64(
          accu, ixheaac_mult64(inp2[n + 6 * num_band_anal_qmf],
                                p_qmf2[2 * (n + 6 * num_band_anal_qmf)]));
      accu = ixheaac_add64(
          accu, ixheaac_mult64(inp2[n + 8 * num_band_anal_qmf],
                                p_qmf2[2 * (n + 8 * num_band_anal_qmf)]));
      p_out[n + num_band_anal_qmf] = (WORD32)(accu >> 31);

      accu = ixheaac_mult64(inp2[n + 1 + 0], p_qmf2[2 * (n + 1 + 0)]);
      accu = ixheaac_add64(
          accu, ixheaac_mult64(inp2[n + 1 + 2 * num_band_anal_qmf],
                                p_qmf2[2 * (n + 1 + 2 * num_band_anal_qmf)]));
      accu = ixheaac_add64(
          accu, ixheaac_mult64(inp2[n + 1 + 4 * num_band_anal_qmf],
                                p_qmf2[2 * (n + 1 + 4 * num_band_anal_qmf)]));
      accu = ixheaac_add64(
          accu, ixheaac_mult64(inp2[n + 1 + 6 * num_band_anal_qmf],
                                p_qmf2[2 * (n + 1 + 6 * num_band_anal_qmf)]));
      accu = ixheaac_add64(
          accu, ixheaac_mult64(inp2[n + 1 + 8 * num_band_anal_qmf],
                                p_qmf2[2 * (n + 1 + 8 * num_band_anal_qmf)]));
      p_out[n + 1 + num_band_anal_qmf] = (WORD32)(accu >> 31);
    }
  } else if (num_band_anal_qmf == 24) {
    for (n = 0; n < num_band_anal_qmf; n += 2) {
      accu = ixheaac_mult64(inp1[n + 0], p_qmf1[(n + 0)]);
      accu = ixheaac_add64(
          accu, ixheaac_mult64(inp1[n + 2 * num_band_anal_qmf],
                                p_qmf1[(n + 2 * num_band_anal_qmf)]));
      accu = ixheaac_add64(
          accu, ixheaac_mult64(inp1[n + 4 * num_band_anal_qmf],
                                p_qmf1[(n + 4 * num_band_anal_qmf)]));
      accu = ixheaac_add64(
          accu, ixheaac_mult64(inp1[n + 6 * num_band_anal_qmf],
                                p_qmf1[(n + 6 * num_band_anal_qmf)]));
      accu = ixheaac_add64(
          accu, ixheaac_mult64(inp1[n + 8 * num_band_anal_qmf],
                                p_qmf1[(n + 8 * num_band_anal_qmf)]));
      p_out[n] = (WORD32)(accu >> 31);

      accu = ixheaac_mult64(inp1[n + 1 + 0], p_qmf1[(n + 1 + 0)]);
      accu = ixheaac_add64(
          accu, ixheaac_mult64(inp1[n + 1 + 2 * num_band_anal_qmf],
                                p_qmf1[(n + 1 + 2 * num_band_anal_qmf)]));
      accu = ixheaac_add64(
          accu, ixheaac_mult64(inp1[n + 1 + 4 * num_band_anal_qmf],
                                p_qmf1[(n + 1 + 4 * num_band_anal_qmf)]));
      accu = ixheaac_add64(
          accu, ixheaac_mult64(inp1[n + 1 + 6 * num_band_anal_qmf],
                                p_qmf1[(n + 1 + 6 * num_band_anal_qmf)]));
      accu = ixheaac_add64(
          accu, ixheaac_mult64(inp1[n + 1 + 8 * num_band_anal_qmf],
                                p_qmf1[(n + 1 + 8 * num_band_anal_qmf)]));
      p_out[n + 1] = (WORD32)(accu >> 31);

      accu = ixheaac_mult64(inp2[n + 0], p_qmf2[(n + 0)]);
      accu = ixheaac_add64(
          accu, ixheaac_mult64(inp2[n + 2 * num_band_anal_qmf],
                                p_qmf2[(n + 2 * num_band_anal_qmf)]));
      accu = ixheaac_add64(
          accu, ixheaac_mult64(inp2[n + 4 * num_band_anal_qmf],
                                p_qmf2[(n + 4 * num_band_anal_qmf)]));
      accu = ixheaac_add64(
          accu, ixheaac_mult64(inp2[n + 6 * num_band_anal_qmf],
                                p_qmf2[(n + 6 * num_band_anal_qmf)]));
      accu = ixheaac_add64(
          accu, ixheaac_mult64(inp2[n + 8 * num_band_anal_qmf],
                                p_qmf2[(n + 8 * num_band_anal_qmf)]));
      p_out[n + num_band_anal_qmf] = (WORD32)(accu >> 31);

      accu = ixheaac_mult64(inp2[n + 1 + 0], p_qmf2[(n + 1 + 0)]);
      accu = ixheaac_add64(
          accu, ixheaac_mult64(inp2[n + 1 + 2 * num_band_anal_qmf],
                                p_qmf2[(n + 1 + 2 * num_band_anal_qmf)]));
      accu = ixheaac_add64(
          accu, ixheaac_mult64(inp2[n + 1 + 4 * num_band_anal_qmf],
                                p_qmf2[(n + 1 + 4 * num_band_anal_qmf)]));
      accu = ixheaac_add64(
          accu, ixheaac_mult64(inp2[n + 1 + 6 * num_band_anal_qmf],
                                p_qmf2[(n + 1 + 6 * num_band_anal_qmf)]));
      accu = ixheaac_add64(
          accu, ixheaac_mult64(inp2[n + 1 + 8 * num_band_anal_qmf],
                                p_qmf2[(n + 1 + 8 * num_band_anal_qmf)]));
      p_out[n + 1 + num_band_anal_qmf] = (WORD32)(accu >> 31);
    }

  } else {
    for (n = 0; n < num_band_anal_qmf; n += 2) {
      accu = ixheaac_mult64(inp1[n + 0], p_qmf1[4 * (n + 0)]);
      accu = ixheaac_add64(
          accu, ixheaac_mult64(inp1[n + 2 * num_band_anal_qmf],
                                p_qmf1[4 * (n + 2 * num_band_anal_qmf)]));
      accu = ixheaac_add64(
          accu, ixheaac_mult64(inp1[n + 4 * num_band_anal_qmf],
                                p_qmf1[4 * (n + 4 * num_band_anal_qmf)]));
      accu = ixheaac_add64(
          accu, ixheaac_mult64(inp1[n + 6 * num_band_anal_qmf],
                                p_qmf1[4 * (n + 6 * num_band_anal_qmf)]));
      accu = ixheaac_add64(
          accu, ixheaac_mult64(inp1[n + 8 * num_band_anal_qmf],
                                p_qmf1[4 * (n + 8 * num_band_anal_qmf)]));
      p_out[n] = (WORD32)(accu >> 31);

      accu = ixheaac_mult64(inp1[n + 1 + 0], p_qmf1[4 * (n + 1 + 0)]);
      accu = ixheaac_add64(
          accu, ixheaac_mult64(inp1[n + 1 + 2 * num_band_anal_qmf],
                                p_qmf1[4 * (n + 1 + 2 * num_band_anal_qmf)]));
      accu = ixheaac_add64(
          accu, ixheaac_mult64(inp1[n + 1 + 4 * num_band_anal_qmf],
                                p_qmf1[4 * (n + 1 + 4 * num_band_anal_qmf)]));
      accu = ixheaac_add64(
          accu, ixheaac_mult64(inp1[n + 1 + 6 * num_band_anal_qmf],
                                p_qmf1[4 * (n + 1 + 6 * num_band_anal_qmf)]));
      accu = ixheaac_add64(
          accu, ixheaac_mult64(inp1[n + 1 + 8 * num_band_anal_qmf],
                                p_qmf1[4 * (n + 1 + 8 * num_band_anal_qmf)]));
      p_out[n + 1] = (WORD32)(accu >> 31);

      accu = ixheaac_mult64(inp2[n + 0], p_qmf2[4 * (n + 0)]);
      accu = ixheaac_add64(
          accu, ixheaac_mult64(inp2[n + 2 * num_band_anal_qmf],
                                p_qmf2[4 * (n + 2 * num_band_anal_qmf)]));
      accu = ixheaac_add64(
          accu, ixheaac_mult64(inp2[n + 4 * num_band_anal_qmf],
                                p_qmf2[4 * (n + 4 * num_band_anal_qmf)]));
      accu = ixheaac_add64(
          accu, ixheaac_mult64(inp2[n + 6 * num_band_anal_qmf],
                                p_qmf2[4 * (n + 6 * num_band_anal_qmf)]));
      accu = ixheaac_add64(
          accu, ixheaac_mult64(inp2[n + 8 * num_band_anal_qmf],
                                p_qmf2[4 * (n + 8 * num_band_anal_qmf)]));
      p_out[n + num_band_anal_qmf] = (WORD32)(accu >> 31);

      accu = ixheaac_mult64(inp2[n + 1 + 0], p_qmf2[4 * (n + 1 + 0)]);
      accu = ixheaac_add64(
          accu, ixheaac_mult64(inp2[n + 1 + 2 * num_band_anal_qmf],
                                p_qmf2[4 * (n + 1 + 2 * num_band_anal_qmf)]));
      accu = ixheaac_add64(
          accu, ixheaac_mult64(inp2[n + 1 + 4 * num_band_anal_qmf],
                                p_qmf2[4 * (n + 1 + 4 * num_band_anal_qmf)]));
      accu = ixheaac_add64(
          accu, ixheaac_mult64(inp2[n + 1 + 6 * num_band_anal_qmf],
                                p_qmf2[4 * (n + 1 + 6 * num_band_anal_qmf)]));
      accu = ixheaac_add64(
          accu, ixheaac_mult64(inp2[n + 1 + 8 * num_band_anal_qmf],
                                p_qmf2[4 * (n + 1 + 8 * num_band_anal_qmf)]));
      p_out[n + 1 + num_band_anal_qmf] = (WORD32)(accu >> 31);
    }
  }
}

VOID ixheaacd_esbr_inv_modulation(
    WORD32 *qmf_real, ia_sbr_qmf_filter_bank_struct *syn_qmf,
    ia_qmf_dec_tables_struct *qmf_dec_tables_ptr, WORD32 no_synthesis_channels) {

    if (no_synthesis_channels == NO_SYNTHESIS_CHANNELS_DOWN_SAMPLED)
    {
      ixheaacd_esbr_cos_sin_mod(qmf_real, syn_qmf, qmf_dec_tables_ptr->esbr_w_16,
        qmf_dec_tables_ptr->dig_rev_table4_16);
    }
    else
    {
      ixheaacd_esbr_cos_sin_mod(qmf_real, syn_qmf, qmf_dec_tables_ptr->esbr_w_32,
        qmf_dec_tables_ptr->dig_rev_table2_32);
    }
}

VOID ixheaacd_sbr_qmfsyn32_winadd(WORD16 *tmp1, WORD16 *tmp2, WORD16 *inp1,
                                  WORD16 *sample_buffer, FLAG shift,
                                  WORD32 ch_fac) {
  WORD32 k;
  WORD32 rounding_fac = 0x8000;
  rounding_fac = rounding_fac >> shift;

  for (k = 0; k < 32; k++) {
    WORD32 syn_out = rounding_fac;

    syn_out = ixheaac_add32(
        syn_out, ixheaac_mult16x16in32(tmp1[0 + k], inp1[2 * (k + 0)]));
    syn_out = ixheaac_add32(
        syn_out, ixheaac_mult16x16in32(tmp1[128 + k], inp1[2 * (k + 64)]));
    syn_out = ixheaac_add32(
        syn_out, ixheaac_mult16x16in32(tmp1[256 + k], inp1[2 * (k + 128)]));
    syn_out = ixheaac_add32(
        syn_out, ixheaac_mult16x16in32(tmp1[384 + k], inp1[2 * (k + 192)]));
    syn_out = ixheaac_add32(
        syn_out, ixheaac_mult16x16in32(tmp1[512 + k], inp1[2 * (k + 256)]));

    syn_out = ixheaac_add32(
        syn_out, ixheaac_mult16x16in32(tmp2[64 + k], inp1[2 * (k + 32)]));
    syn_out = ixheaac_add32(
        syn_out, ixheaac_mult16x16in32(tmp2[192 + k], inp1[2 * (k + 96)]));
    syn_out = ixheaac_add32(
        syn_out, ixheaac_mult16x16in32(tmp2[320 + k], inp1[2 * (k + 160)]));
    syn_out = ixheaac_add32(
        syn_out, ixheaac_mult16x16in32(tmp2[448 + k], inp1[2 * (k + 224)]));
    syn_out = ixheaac_add32(
        syn_out, ixheaac_mult16x16in32(tmp2[576 + k], inp1[2 * (k + 288)]));
    syn_out = ixheaac_add32_sat(syn_out, syn_out);
    if (shift == 2) {
      syn_out = ixheaac_add32_sat(syn_out, syn_out);
    }
    sample_buffer[ch_fac * k] = (syn_out >> 16);
  }
}

void ixheaacd_sbr_pre_twiddle(WORD32 *p_xre, WORD32 *p_xim,
                              WORD16 *p_twiddles) {
  int k;

  for (k = 62; k >= 0; k--) {
    WORD32 x_re = *p_xre;
    WORD32 x_im = *p_xim;

    WORD16 ixheaacd_cosine = *p_twiddles++;
    WORD16 ixheaacd_sine = *p_twiddles++;

    WORD32 re, im;

    re = ixheaac_mac32x16in32_shl_sat(
        ixheaac_mult32x16in32_shl(x_re, ixheaacd_cosine), x_im, ixheaacd_sine);
    im = ixheaac_sub32_sat(ixheaac_mult32x16in32_shl(x_im, ixheaacd_cosine),
                            ixheaac_mult32x16in32_shl(x_re, ixheaacd_sine));

    *p_xre++ = re;
    *p_xim++ = im;
  }
}

VOID ixheaacd_cplx_synt_qmffilt(
    WORD32 **qmf_real, WORD32 **qmf_imag, WORD32 split,
    WORD32 *qmf_real_out[MAX_ENV_COLS], WORD32 *qmf_imag_out[MAX_ENV_COLS],
    ia_sbr_scale_fact_struct *sbr_scale_factor, WORD16 *time_out,
    ia_sbr_qmf_filter_bank_struct *qmf_bank, ia_ps_dec_struct *ptr_ps_dec,
    FLAG active, FLAG low_pow_flag, ia_sbr_tables_struct *sbr_tables_ptr,
    ixheaacd_misc_tables *pstr_common_tables, WORD32 ch_fac, FLAG drc_on,
    WORD32 drc_sbr_factors[][64], WORD32 audio_object_type) {
  WORD32 i, j;

  WORD32 code_scale_factor;
  WORD32 scale_factor;
  WORD32 out_scale_factor;
  WORD32 low_band_scale_factor;
  WORD32 high_band_scale_factor;
  WORD16 *filter_states = qmf_bank->filter_states;
  WORD32 **ptr_qmf_imag_temp;
  WORD32 qmf_real2[2 * NO_SYNTHESIS_CHANNELS];

  WORD32 no_synthesis_channels = qmf_bank->no_channels;
  WORD32 p1;

  WORD16 *fp1;
  WORD16 *fp2;

  WORD32 sixty4 = NO_SYNTHESIS_CHANNELS;
  WORD32 thirty2 = qmf_bank->no_channels;

  WORD16 *filter_coeff;
  WORD32 num_time_slots = qmf_bank->num_time_slots;
  WORD32 ixheaacd_drc_offset;
  WORD32 ov_lb_scale = sbr_scale_factor->ov_lb_scale;
  WORD32 lb_scale = sbr_scale_factor->lb_scale;
  WORD32 st_syn_scale = sbr_scale_factor->st_syn_scale;
  WORD32 ov_lb_shift, lb_shift, hb_shift;

  WORD32 *qmf_real_tmp = qmf_real2;
  WORD32 *qmf_imag_tmp = &qmf_real2[NO_SYNTHESIS_CHANNELS];
  WORD32 env = 0;

  WORD32 common_shift = 0;

  if (no_synthesis_channels == 32) {
    qmf_bank->cos_twiddle =
        (WORD16 *)sbr_tables_ptr->qmf_dec_tables_ptr->sbr_sin_cos_twiddle_l32;
    qmf_bank->alt_sin_twiddle =
        (WORD16 *)sbr_tables_ptr->qmf_dec_tables_ptr->sbr_alt_sin_twiddle_l32;
    qmf_bank->t_cos =
        (WORD16 *)
            sbr_tables_ptr->qmf_dec_tables_ptr->sbr_cos_sin_twiddle_ds_l32;
  } else {
    qmf_bank->cos_twiddle =
        (WORD16 *)sbr_tables_ptr->qmf_dec_tables_ptr->sbr_sin_cos_twiddle_l64;
    qmf_bank->alt_sin_twiddle =
        (WORD16 *)sbr_tables_ptr->qmf_dec_tables_ptr->sbr_alt_sin_twiddle_l64;
  }
  if (audio_object_type != AOT_ER_AAC_ELD &&
      audio_object_type != AOT_ER_AAC_LD) {
    qmf_bank->filter_pos_syn +=
        (sbr_tables_ptr->qmf_dec_tables_ptr->qmf_c - qmf_bank->p_filter);
    qmf_bank->p_filter = sbr_tables_ptr->qmf_dec_tables_ptr->qmf_c;
  } else {
    qmf_bank->filter_pos_syn +=
        (sbr_tables_ptr->qmf_dec_tables_ptr->qmf_c_eld - qmf_bank->p_filter);
    qmf_bank->p_filter = sbr_tables_ptr->qmf_dec_tables_ptr->qmf_c_eld;
  }

  fp1 = &filter_states[0];
  fp2 = fp1 + no_synthesis_channels;

  if (audio_object_type == AOT_ER_AAC_ELD ||
      audio_object_type == AOT_ER_AAC_LD) {
    fp1 = qmf_bank->fp1_syn;
    fp2 = qmf_bank->fp2_syn;
    sixty4 = qmf_bank->sixty4;
  }

  filter_coeff = qmf_bank->filter_pos_syn;

  if (active) {
    code_scale_factor = scale_factor = sbr_scale_factor->ps_scale;
  } else {
    code_scale_factor = ixheaac_min32(lb_scale, ov_lb_scale);
    scale_factor = sbr_scale_factor->hb_scale;
  }

  low_band_scale_factor = (st_syn_scale - code_scale_factor);
  high_band_scale_factor = (st_syn_scale - scale_factor);

  p1 = 0;

  if (low_pow_flag)

  {
    ov_lb_shift = (st_syn_scale - ov_lb_scale) - 4;
    lb_shift = (st_syn_scale - lb_scale) - 4;
    hb_shift = high_band_scale_factor - 4;
    out_scale_factor = -((sbr_scale_factor->st_syn_scale - 1));
    ptr_qmf_imag_temp = 0;

  }

  else {
    out_scale_factor = -((sbr_scale_factor->st_syn_scale - 3));
    if (active) {
      ov_lb_shift = (sbr_scale_factor->ps_scale - ov_lb_scale);
      lb_shift = (sbr_scale_factor->ps_scale - lb_scale);
      hb_shift = (sbr_scale_factor->ps_scale - sbr_scale_factor->hb_scale);
      common_shift = low_band_scale_factor - 8;

    } else {
      if (audio_object_type != AOT_ER_AAC_ELD &&
          audio_object_type != AOT_ER_AAC_LD) {
        ov_lb_shift = (st_syn_scale - ov_lb_scale) - 8;
        lb_shift = (st_syn_scale - lb_scale) - 8;
        hb_shift = high_band_scale_factor - 8;
      } else {
        ov_lb_shift = (st_syn_scale - ov_lb_scale) - 7;
        lb_shift = (st_syn_scale - lb_scale) - 7;
        hb_shift = high_band_scale_factor - 7;
      }
      common_shift = 0;
    }
    ptr_qmf_imag_temp = qmf_imag;
  }

  {
    if (ov_lb_shift == lb_shift) {
      (*ixheaacd_adjust_scale)(qmf_real, ptr_qmf_imag_temp, 0, qmf_bank->lsb, 0,
                               num_time_slots, ov_lb_shift, low_pow_flag);

    } else {
      (*ixheaacd_adjust_scale)(qmf_real, ptr_qmf_imag_temp, 0, qmf_bank->lsb, 0,
                               split, ov_lb_shift, low_pow_flag);

      (*ixheaacd_adjust_scale)(qmf_real, ptr_qmf_imag_temp, 0, qmf_bank->lsb,
                               split, num_time_slots, lb_shift, low_pow_flag);
    }

    (*ixheaacd_adjust_scale)(qmf_real, ptr_qmf_imag_temp, qmf_bank->lsb,
                             qmf_bank->usb, 0, num_time_slots, hb_shift,
                             low_pow_flag);
  }

  ixheaacd_drc_offset = qmf_bank->ixheaacd_drc_offset;

  if (1 == drc_on) {
    for (i = 0; i < num_time_slots; i++) {
      WORD32 loop_val;
      for (loop_val = 0; loop_val < 64; loop_val++) {
        qmf_real[i][loop_val] = ixheaacd_mult32x32in32_shift25(
            qmf_real[i][loop_val], drc_sbr_factors[6 + i][loop_val]);
      }
    }
  }
  for (i = 0; i < num_time_slots; i++)
  {
    for (j = 0; j < no_synthesis_channels; j++)
    {
      qmf_real_out[i][j] = qmf_real[i][j];
      if (!low_pow_flag)
      {
        qmf_imag_out[i][j] = qmf_imag[i][j];
      }
    }
  }
  if (low_pow_flag)
  {
    WORD16 *fptemp;

    VOID(*sbr_qmf_syn_winadd)
    (WORD16 *, WORD16 *, WORD16 *, WORD16 *, FLAG, WORD32);
    ia_qmf_dec_tables_struct *qmf_tab_ptr = sbr_tables_ptr->qmf_dec_tables_ptr;

    if (no_synthesis_channels == NO_SYNTHESIS_CHANNELS_DOWN_SAMPLED)
      sbr_qmf_syn_winadd = ixheaacd_sbr_qmfsyn32_winadd;
    else
      sbr_qmf_syn_winadd = ixheaacd_sbr_qmfsyn64_winadd;

    for (i = 0; i < num_time_slots; i++) {
      ixheaacd_inv_modulation_lp(qmf_real[i],
                                 &filter_states[ixheaacd_drc_offset], qmf_bank,
                                 qmf_tab_ptr);

      sbr_qmf_syn_winadd(fp1, fp2, filter_coeff, &time_out[ch_fac * p1], 2,
                         ch_fac);

      ixheaacd_drc_offset -= no_synthesis_channels << 1;

      if (ixheaacd_drc_offset < 0)
        ixheaacd_drc_offset += ((no_synthesis_channels << 1) * 10);

      fptemp = fp1;
      fp1 = fp2;
      fp2 = fptemp;

      filter_coeff += 64;

      if (filter_coeff == qmf_bank->p_filter + 640)
        filter_coeff = (WORD16 *)qmf_bank->p_filter;

      p1 += no_synthesis_channels;
    }

  } else {
    for (i = 0; i < num_time_slots; i++) {
      WORD32 *t_qmf_imag;
      t_qmf_imag = qmf_imag[i];

      if (active) {
        if (i == ptr_ps_dec->border_position[env]) {
          ixheaacd_init_rot_env(ptr_ps_dec, (WORD16)env, qmf_bank->usb,
                                sbr_tables_ptr, pstr_common_tables->trig_data);
          env++;
        }

        ixheaacd_apply_ps(ptr_ps_dec, &qmf_real[i], &qmf_imag[i], qmf_real_tmp,
                          qmf_imag_tmp, sbr_scale_factor, (WORD16)i,
                          sbr_tables_ptr, num_time_slots);
      }
      if (1 == drc_on) {
        WORD32 loop_val;
        for (loop_val = 0; loop_val < 64; loop_val++) {
          qmf_real[i][loop_val] = ixheaacd_mult32x32in32_shift25(
              qmf_real[i][loop_val], drc_sbr_factors[6 + i][loop_val]);
        }
      }

      if (active) {
        if (common_shift)
          ixheaacd_shiftrountine(qmf_real[i], t_qmf_imag, no_synthesis_channels,
                                 common_shift);
      }

      if (audio_object_type == AOT_ER_AAC_ELD ||
          audio_object_type == AOT_ER_AAC_LD)
        ixheaacd_sbr_pre_twiddle(
            qmf_real[i], t_qmf_imag,
            sbr_tables_ptr->qmf_dec_tables_ptr->ixheaacd_sbr_synth_cos_sin_l32);

      ixheaacd_inv_emodulation(qmf_real[i], qmf_bank,
                               sbr_tables_ptr->qmf_dec_tables_ptr);

      {
        WORD32 temp_out_scale_fac = out_scale_factor + 1;
        if (audio_object_type == AOT_ER_AAC_LD ||
            audio_object_type == AOT_ER_AAC_ELD) {
          temp_out_scale_fac = temp_out_scale_fac - 1;

          ixheaacd_shiftrountine_with_rnd_eld(
              qmf_real[i], t_qmf_imag, &filter_states[ixheaacd_drc_offset],
              no_synthesis_channels, temp_out_scale_fac);

        }

        else {
          ixheaacd_shiftrountine_with_rnd(
              qmf_real[i], t_qmf_imag, &filter_states[ixheaacd_drc_offset],
              no_synthesis_channels, temp_out_scale_fac);
        }
      }

      if (no_synthesis_channels == NO_SYNTHESIS_CHANNELS_DOWN_SAMPLED) {
        WORD32 temp = 1;
        if (audio_object_type == AOT_ER_AAC_LD ||
            audio_object_type == AOT_ER_AAC_ELD) {
          temp = 2;
        }
        ixheaacd_sbr_qmfsyn32_winadd(fp1, fp2, filter_coeff,
                                     &time_out[ch_fac * p1], temp, ch_fac);

        fp1 += thirty2;
        fp2 -= thirty2;
        thirty2 = -thirty2;

        ixheaacd_drc_offset -= 64;

        if (ixheaacd_drc_offset < 0) ixheaacd_drc_offset += 640;

      } else {
        WORD32 temp = 1;
        if (audio_object_type == AOT_ER_AAC_LD ||
            audio_object_type == AOT_ER_AAC_ELD) {
          temp = 2;
        }
        ixheaacd_sbr_qmfsyn64_winadd(fp1, fp2, filter_coeff,
                                     &time_out[ch_fac * p1], temp, ch_fac);

        fp1 += sixty4;
        fp2 -= sixty4;
        sixty4 = -sixty4;
        ixheaacd_drc_offset -= 128;

        if (ixheaacd_drc_offset < 0) ixheaacd_drc_offset += 1280;
      }

      filter_coeff += 64;

      if (filter_coeff == qmf_bank->p_filter + 640)
        filter_coeff = (WORD16 *)qmf_bank->p_filter;

      p1 += no_synthesis_channels;

      if (active)
        memcpy(qmf_real[i], qmf_real_tmp,
               2 * no_synthesis_channels * sizeof(WORD32));
    }
  }

  if (audio_object_type == AOT_ER_AAC_LD ||
      audio_object_type == AOT_ER_AAC_ELD) {
    qmf_bank->fp1_syn = fp1;
    qmf_bank->fp2_syn = fp2;
    qmf_bank->sixty4 = sixty4;
  }

  qmf_bank->filter_pos_syn = filter_coeff;
  qmf_bank->ixheaacd_drc_offset = ixheaacd_drc_offset;
}
