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
#include <stdio.h>
#include <string.h>
#include "ixheaacd_sbr_common.h"
#include "ixheaac_type_def.h"

#include "ixheaac_constants.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops16.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaac_basic_ops.h"

#include "ixheaac_basic_op.h"
#include "ixheaacd_intrinsics.h"
#include "ixheaacd_common_rom.h"
#include "ixheaacd_basic_funcs.h"
#include "ixheaacd_sbr_scale.h"
#include "ixheaacd_sbrdecsettings.h"
#include "ixheaacd_lpp_tran.h"
#include "ixheaacd_bitbuffer.h"
#include "ixheaacd_defines.h"

#include "ixheaacd_pns.h"

#include "ixheaacd_aac_rom.h"
#include "ixheaacd_pulsedata.h"

#include "ixheaacd_drc_data_struct.h"

#include "ixheaacd_lt_predict.h"
#include "ixheaacd_cnst.h"
#include "ixheaacd_ec_defines.h"
#include "ixheaacd_ec_struct_def.h"
#include "ixheaacd_channelinfo.h"
#include "ixheaacd_drc_dec.h"

#include "ixheaacd_sbrdecoder.h"
#include "ixheaacd_sbr_scale.h"
#include "ixheaacd_lpp_tran.h"
#include "ixheaacd_env_extr_part.h"
#include "ixheaacd_sbr_rom.h"
#include "ixheaacd_hybrid.h"
#include "ixheaacd_ps_dec.h"
#include "ixheaacd_env_extr.h"

#include "ixheaacd_intrinsics.h"
#include "ixheaacd_basic_funcs.h"

#include "ixheaacd_qmf_dec.h"
#include "ixheaacd_env_calc.h"
#include "ixheaac_sbr_const.h"

#include "ixheaacd_pvc_dec.h"
#include "ixheaacd_sbr_dec.h"
#include "ixheaacd_function_selector.h"

#include "ixheaacd_audioobjtypes.h"

#define LPC_SCALE_FACTOR 2

#define AUTO_CORR_LEN_1024 38
#define AUTO_CORR_LEN_960  36

#define SHIFT 5

static PLATFORM_INLINE WORD32 ixheaacd_mac32x16hin32(WORD32 a, WORD32 b,
                                                     WORD32 c) {
  WORD32 result;

  result = a + ixheaac_mult32x16hin32(b, c);

  return (result);
}

static PLATFORM_INLINE WORD32 ixheaacd_macn32x16hin32(WORD32 a, WORD32 b,
                                                      WORD32 c) {
  WORD32 result;

  result = a - ixheaac_mult32x16hin32(b, c);

  return (result);
}

VOID ixheaacd_filterstep3(WORD16 a0r, WORD16 a0i, WORD16 a1r, WORD16 a1i,
                          WORD32 start_indx, WORD32 stop_idx, WORD32 low_band,
                          WORD32 high_band, WORD32 *qmf_buffer) {
  WORD32 i;
  WORD32 prev1r, prev1i;
  WORD32 prev2r, prev2i;
  WORD16 coef1r = (a0r);
  WORD16 coef1i = (a0i);
  WORD16 coef2r = (a1r);
  WORD16 coef2i = (a1i);
  WORD32 *p_src, *p_dst;
  WORD32 qmf_real, qmf_imag;

  WORD32 curr, curi;
  p_src = qmf_buffer + low_band + ((start_indx) << 7);
  prev2r = *p_src;
  p_src += 64;

  prev2i = *p_src;
  p_src += 64;

  prev1r = *p_src;
  p_src += 64;

  prev1i = *p_src;
  p_src += 64;

  p_dst = qmf_buffer + high_band + ((start_indx + 2) << 7);

  for (i = stop_idx - start_indx; i != 0; i--) {
    WORD32 accu;

    curr = *p_src;
    p_src += 64;

    curi = *p_src;
    p_src += 64;

    qmf_real = (curr >> LPC_SCALE_FACTOR);
    qmf_imag = (curi >> LPC_SCALE_FACTOR);

    accu = ixheaac_sub32(
        ixheaac_add32(ixheaac_sub32(ixheaac_mult32x16in32(prev1r, coef1r),
                                      ixheaac_mult32x16in32(prev1i, coef1i)),
                       ixheaac_mult32x16in32(prev2r, coef2r)),
        ixheaac_mult32x16in32(prev2i, coef2i));

    *p_dst = ixheaac_add32(qmf_real, (accu << 1));
    p_dst += 64;

    accu = ixheaac_add32(
        ixheaac_add32_sat(
            ixheaac_add32_sat(ixheaac_mult32x16in32(prev1r, coef1i),
                               ixheaac_mult32x16in32(prev1i, coef1r)),
            ixheaac_mult32x16in32(prev2r, coef2i)),
        ixheaac_mult32x16in32(prev2i, coef2r));

    *p_dst = ixheaac_add32(qmf_imag, (accu << 1));
    p_dst += 64;

    prev2r = prev1r;
    prev1r = curr;
    prev2i = prev1i;
    prev1i = curi;
  }
}

VOID ixheaacd_covariance_matrix_calc_dec_960(
    WORD32 *sub_sign_xlow, ia_lpp_trans_cov_matrix *cov_matrix,
    WORD32 count, WORD32 len) {
  WORD32 j, k;
  WORD32 ixheaacd_drc_offset = 2;
  WORD32 factor;
  WORD32 max_val, q_factor;
  WORD32 temp1, temp2, temp3, temp4;
  WORD32 *temp_buf_ptr = sub_sign_xlow;

  temp3 = 0;
  for (k = count; k > 0; k--) {
    WORD32 t_phi_01 = 0, t_phi_02 = 0, t_phi_11 = 0;
    WORD32 t_phi_12 = 0, t_phi_22 = 0;

    factor = -3;
    j = ixheaacd_drc_offset;
    sub_sign_xlow = temp_buf_ptr;

    temp1 = ixheaac_shl32_dir(*sub_sign_xlow, factor);
    sub_sign_xlow += 64;

    temp2 = ixheaac_shl32_dir(*sub_sign_xlow, factor);
    sub_sign_xlow += 64;

    for (; (j = j + 3) <= ixheaacd_drc_offset + len;) {
      temp3 = ixheaac_shl32_dir(*sub_sign_xlow, factor);
      sub_sign_xlow += 64;

      t_phi_01 += ixheaac_mult32x16hin32(temp3, temp2);
      t_phi_02 += ixheaac_mult32x16hin32(temp3, temp1);
      t_phi_11 += ixheaac_mult32x16hin32(temp2, temp2);

      temp1 = ixheaac_shl32_dir(*sub_sign_xlow, factor);
      sub_sign_xlow += 64;

      t_phi_01 += ixheaac_mult32x16hin32(temp1, temp3);
      t_phi_02 += ixheaac_mult32x16hin32(temp1, temp2);
      t_phi_11 += ixheaac_mult32x16hin32(temp3, temp3);

      temp2 = ixheaac_shl32_dir(*sub_sign_xlow, factor);
      sub_sign_xlow += 64;

      t_phi_01 += ixheaac_mult32x16hin32(temp2, temp1);
      t_phi_02 += ixheaac_mult32x16hin32(temp2, temp3);
      t_phi_11 += ixheaac_mult32x16hin32(temp1, temp1);
    }

    if (AUTO_CORR_LEN_1024 == len) {
      temp3 = ixheaac_shl32_dir(*sub_sign_xlow, factor);
      sub_sign_xlow += 64;

      t_phi_01 += ixheaac_mult32x16hin32(temp3, temp2);
      t_phi_02 += ixheaac_mult32x16hin32(temp3, temp1);
      t_phi_11 += ixheaac_mult32x16hin32(temp2, temp2);

      temp1 = ixheaac_shl32_dir(*sub_sign_xlow, factor);
      sub_sign_xlow += 64;

      t_phi_01 += ixheaac_mult32x16hin32(temp1, temp3);
      t_phi_02 += ixheaac_mult32x16hin32(temp1, temp2);
      t_phi_11 += ixheaac_mult32x16hin32(temp3, temp3);
    }

    temp2 = ixheaac_shl32_dir(*temp_buf_ptr, factor);
    temp4 = ixheaac_shl32_dir(*(temp_buf_ptr + 64), factor);

    if (AUTO_CORR_LEN_960 == len) {
      temp3 = ixheaac_shl32_dir(sub_sign_xlow[-128], factor);
      temp1 = ixheaac_shl32_dir(sub_sign_xlow[-64], factor);
    }

    t_phi_12 = (t_phi_01 - ixheaac_mult32x16hin32(temp1, temp3) +
        ixheaac_mult32x16hin32(temp4, temp2));

    t_phi_22 = (t_phi_11 - ixheaac_mult32x16hin32(temp3, temp3) +
        ixheaac_mult32x16hin32(temp2, temp2));

    max_val = ixheaac_abs32_nrm(t_phi_01);
    max_val = max_val | ixheaac_abs32_nrm(t_phi_02);
    max_val = max_val | ixheaac_abs32_nrm(t_phi_12);
    max_val = max_val | (t_phi_11);
    max_val = max_val | (t_phi_22);

    q_factor = ixheaac_pnorm32(max_val);

    cov_matrix->phi_11 = (t_phi_11 << q_factor);
    cov_matrix->phi_22 = (t_phi_22 << q_factor);
    cov_matrix->phi_01 = (t_phi_01 << q_factor);
    cov_matrix->phi_02 = (t_phi_02 << q_factor);
    cov_matrix->phi_12 = (t_phi_12 << q_factor);

    cov_matrix->d = ixheaac_sub32_sat(
        ixheaac_mult32(cov_matrix->phi_22, cov_matrix->phi_11),
        ixheaac_mult32(cov_matrix->phi_12, cov_matrix->phi_12));

    cov_matrix++;
    temp_buf_ptr++;
  }
  return;
}

VOID ixheaacd_covariance_matrix_calc_dec(
    WORD32 *sub_sign_xlow, ia_lpp_trans_cov_matrix *cov_matrix,
    WORD32 count, WORD32 len) {
  WORD32 j, k;
  WORD32 ixheaacd_drc_offset = 2;
  WORD32 factor;
  WORD32 max_val, q_factor;
  WORD32 temp1, temp2, temp3, temp4;
  WORD32 *temp_buf_ptr = sub_sign_xlow;

  for (k = count; k > 0; k--) {
    WORD32 t_phi_01 = 0, t_phi_02 = 0, t_phi_11 = 0;
    WORD32 t_phi_12 = 0, t_phi_22 = 0;

    factor = -3;
    j = ixheaacd_drc_offset;
    sub_sign_xlow = temp_buf_ptr;

    temp1 = ixheaac_shl32_dir(*sub_sign_xlow, factor);
    sub_sign_xlow += 64;

    temp2 = ixheaac_shl32_dir(*sub_sign_xlow, factor);
    sub_sign_xlow += 64;

    for (; (j = j + 3) <= ixheaacd_drc_offset + len;) {
      temp3 = ixheaac_shl32_dir(*sub_sign_xlow, factor);
      sub_sign_xlow += 64;

      t_phi_01 += ixheaac_mult32x16hin32(temp3, temp2);
      t_phi_02 += ixheaac_mult32x16hin32(temp3, temp1);
      t_phi_11 += ixheaac_mult32x16hin32(temp2, temp2);

      temp1 = ixheaac_shl32_dir(*sub_sign_xlow, factor);
      sub_sign_xlow += 64;

      t_phi_01 += ixheaac_mult32x16hin32(temp1, temp3);
      t_phi_02 += ixheaac_mult32x16hin32(temp1, temp2);
      t_phi_11 += ixheaac_mult32x16hin32(temp3, temp3);

      temp2 = ixheaac_shl32_dir(*sub_sign_xlow, factor);
      sub_sign_xlow += 64;

      t_phi_01 += ixheaac_mult32x16hin32(temp2, temp1);
      t_phi_02 += ixheaac_mult32x16hin32(temp2, temp3);
      t_phi_11 += ixheaac_mult32x16hin32(temp1, temp1);
    }

    if (AUTO_CORR_LEN_960 != len) {
      temp3 = ixheaac_shl32_dir(*sub_sign_xlow, factor);
      sub_sign_xlow += 64;

      t_phi_01 += ixheaac_mult32x16hin32(temp3, temp2);
      t_phi_02 += ixheaac_mult32x16hin32(temp3, temp1);
      t_phi_11 += ixheaac_mult32x16hin32(temp2, temp2);

      temp1 = ixheaac_shl32_dir(*sub_sign_xlow, factor);
      sub_sign_xlow += 64;

      t_phi_01 += ixheaac_mult32x16hin32(temp1, temp3);
      t_phi_02 += ixheaac_mult32x16hin32(temp1, temp2);
      t_phi_11 += ixheaac_mult32x16hin32(temp3, temp3);
    }
    if (AUTO_CORR_LEN_960 == len) {
      temp3 = ixheaac_shl32_dir(sub_sign_xlow[-128], factor);

      temp3 = ixheaac_shl32_dir(sub_sign_xlow[-64], factor);

    }

    temp2 = ixheaac_shl32_dir(*temp_buf_ptr, factor);
    temp4 = ixheaac_shl32_dir(*(temp_buf_ptr + 64), factor);

    t_phi_12 = (t_phi_01 - ixheaac_mult32x16hin32(temp1, temp3) +
                ixheaac_mult32x16hin32(temp4, temp2));

    t_phi_22 = (t_phi_11 - ixheaac_mult32x16hin32(temp3, temp3) +
                ixheaac_mult32x16hin32(temp2, temp2));

    max_val = ixheaac_abs32_nrm(t_phi_01);
    max_val = max_val | ixheaac_abs32_nrm(t_phi_02);
    max_val = max_val | ixheaac_abs32_nrm(t_phi_12);
    max_val = max_val | (t_phi_11);
    max_val = max_val | (t_phi_22);

    q_factor = ixheaac_pnorm32(max_val);

    cov_matrix->phi_11 = (t_phi_11 << q_factor);
    cov_matrix->phi_22 = (t_phi_22 << q_factor);
    cov_matrix->phi_01 = (t_phi_01 << q_factor);
    cov_matrix->phi_02 = (t_phi_02 << q_factor);
    cov_matrix->phi_12 = (t_phi_12 << q_factor);

    cov_matrix->d = ixheaac_sub32_sat(
        ixheaac_mult32(cov_matrix->phi_22, cov_matrix->phi_11),
        ixheaac_mult32(cov_matrix->phi_12, cov_matrix->phi_12));

    cov_matrix++;
    temp_buf_ptr++;
  }

  return;
}

VOID ixheaacd_covariance_matrix_calc_2_dec(
    ia_lpp_trans_cov_matrix *cov_matrix,

    WORD32 *real_buffer, WORD32 num_bands, WORD16 slots) {
  WORD32 k;
  WORD32 *img_buffer;
  WORD32 *ptr_real = real_buffer;
  ia_lpp_trans_cov_matrix *pac_arr = cov_matrix;

  for (k = 0; k < num_bands; k++) {
    WORD32 t_phi_11 = 0, t_phi_01 = 0, t_phi_01_i = 0;
    WORD32 prev_real, prev_imag, curr_real, curr_imag;

    real_buffer = ptr_real;
    img_buffer = real_buffer + 64;
    cov_matrix = pac_arr;

    prev_real = real_buffer[-128];
    prev_imag = img_buffer[-128];

    curr_real = real_buffer[0];
    curr_imag = img_buffer[0];

    curr_real = ixheaac_shr32(curr_real, 3);
    curr_imag = ixheaac_shr32(curr_imag, 3);
    prev_real = ixheaac_shr32(prev_real, 3);
    prev_imag = ixheaac_shr32(prev_imag, 3);

    t_phi_01 = ixheaac_mult32x16hin32(curr_real, prev_real);
    t_phi_01 = ixheaacd_mac32x16hin32(t_phi_01, curr_imag, prev_imag);

    t_phi_01_i = ixheaac_mult32x16hin32(curr_imag, prev_real);
    t_phi_01_i = ixheaacd_macn32x16hin32(t_phi_01_i, curr_real, prev_imag);

    t_phi_11 = ixheaac_mult32x16hin32(prev_real, prev_real);
    t_phi_11 = ixheaacd_mac32x16hin32(t_phi_11, prev_imag, prev_imag);

    {
      WORD n;
      WORD32 *real1 = &real_buffer[128];
      WORD32 *imag1 = &img_buffer[128];

      prev_real = curr_real;
      prev_imag = curr_imag;

      for (n = ((slots - 2) >> 1); n; n--) {
        curr_real = *real1;
        real1 += 128;
        curr_imag = *imag1;
        imag1 += 128;

        curr_real = ixheaac_shr32(curr_real, 3);
        curr_imag = ixheaac_shr32(curr_imag, 3);

        t_phi_01 = ixheaacd_mac32x16hin32(t_phi_01, curr_real, prev_real);
        t_phi_01 = ixheaacd_mac32x16hin32(t_phi_01, curr_imag, prev_imag);

        t_phi_01_i = ixheaacd_mac32x16hin32(t_phi_01_i, curr_imag, prev_real);
        t_phi_01_i = ixheaacd_macn32x16hin32(t_phi_01_i, curr_real, prev_imag);

        t_phi_11 = ixheaacd_mac32x16hin32(t_phi_11, prev_real, prev_real);
        t_phi_11 = ixheaacd_mac32x16hin32(t_phi_11, prev_imag, prev_imag);

        prev_real = *real1;
        real1 += 128;
        prev_imag = *imag1;
        imag1 += 128;

        prev_real = ixheaac_shr32(prev_real, 3);
        prev_imag = ixheaac_shr32(prev_imag, 3);

        t_phi_01 = ixheaacd_mac32x16hin32(t_phi_01, prev_real, curr_real);
        t_phi_01 = ixheaacd_mac32x16hin32(t_phi_01, prev_imag, curr_imag);

        t_phi_01_i = ixheaacd_mac32x16hin32(t_phi_01_i, prev_imag, curr_real);
        t_phi_01_i = ixheaacd_macn32x16hin32(t_phi_01_i, prev_real, curr_imag);

        t_phi_11 = ixheaacd_mac32x16hin32(t_phi_11, curr_real, curr_real);
        t_phi_11 = ixheaacd_mac32x16hin32(t_phi_11, curr_imag, curr_imag);
      }

      if (slots & 0x01) {
        curr_real = *real1;
        curr_imag = *imag1;

        curr_real = ixheaac_shr32(curr_real, 3);
        curr_imag = ixheaac_shr32(curr_imag, 3);

        t_phi_01 = ixheaacd_mac32x16hin32(t_phi_01, curr_real, prev_real);
        t_phi_01 = ixheaacd_mac32x16hin32(t_phi_01, curr_imag, prev_imag);

        t_phi_01_i = ixheaacd_mac32x16hin32(t_phi_01_i, curr_imag, prev_real);
        t_phi_01_i = ixheaacd_macn32x16hin32(t_phi_01_i, curr_real, prev_imag);

        t_phi_11 = ixheaacd_mac32x16hin32(t_phi_11, prev_real, prev_real);
        t_phi_11 = ixheaacd_mac32x16hin32(t_phi_11, prev_imag, prev_imag);
      }
    }

    {
      WORD32 t_phi_22 = t_phi_11;
      WORD32 curr_real = real_buffer[-2 * 128];
      WORD32 curr_imag = img_buffer[-2 * 128];

      curr_real = ixheaac_shr32(curr_real, 3);
      curr_imag = ixheaac_shr32(curr_imag, 3);

      t_phi_22 = ixheaacd_mac32x16hin32(t_phi_22, curr_real, curr_real);
      t_phi_22 = ixheaacd_mac32x16hin32(t_phi_22, curr_imag, curr_imag);

      curr_real = real_buffer[(slots - 2) * 128];
      curr_imag = img_buffer[(slots - 2) * 128];

      curr_real = ixheaac_shr32(curr_real, 3);
      curr_imag = ixheaac_shr32(curr_imag, 3);

      t_phi_11 = ixheaacd_mac32x16hin32(t_phi_11, curr_real, curr_real);
      t_phi_11 = ixheaacd_mac32x16hin32(t_phi_11, curr_imag, curr_imag);

      cov_matrix->phi_11 = t_phi_11;
      cov_matrix->phi_22 = t_phi_22;
    }

    {
      WORD32 t_phi_12 = t_phi_01;

      t_phi_12 = ixheaacd_mac32x16hin32(t_phi_12, real_buffer[-128] >> 3,
                                        real_buffer[-2 * 128] >> 3);
      t_phi_12 = ixheaacd_mac32x16hin32(t_phi_12, img_buffer[-128] >> 3,
                                        img_buffer[-2 * 128] >> 3);
      t_phi_01 =
          ixheaacd_mac32x16hin32(t_phi_01, real_buffer[(slots - 1) * 128] >> 3,
                                 real_buffer[(slots - 2) * 128] >> 3);
      t_phi_01 =
          ixheaacd_mac32x16hin32(t_phi_01, img_buffer[(slots - 1) * 128] >> 3,
                                 img_buffer[(slots - 2) * 128] >> 3);

      cov_matrix->phi_01 = t_phi_01;
      cov_matrix->phi_12 = t_phi_12;
    }

    {
      WORD32 t_phi_12_i = t_phi_01_i;

      t_phi_12_i = ixheaacd_mac32x16hin32(t_phi_12_i, img_buffer[-128] >> 3,
                                          real_buffer[-2 * 128] >> 3);
      t_phi_12_i = ixheaacd_macn32x16hin32(t_phi_12_i, real_buffer[-128] >> 3,
                                           img_buffer[-2 * 128] >> 3);
      t_phi_01_i =
          ixheaacd_mac32x16hin32(t_phi_01_i, img_buffer[(slots - 1) * 128] >> 3,
                                 real_buffer[(slots - 2) * 128] >> 3);
      t_phi_01_i = ixheaacd_macn32x16hin32(t_phi_01_i,
                                           real_buffer[(slots - 1) * 128] >> 3,
                                           img_buffer[(slots - 2) * 128] >> 3);

      cov_matrix->phi_01_im = t_phi_01_i;
      cov_matrix->phi_12_im = t_phi_12_i;
    }

    {
      WORD16 n, len_by_4, p;
      WORD32 t_phi_02 = 0x00, t_phi_02_i = 0x00;

      len_by_4 = (slots >> 2);
      p = 0;
      for (n = 0; n < len_by_4; n++) {
        WORD32 real1, real2, imag1, imag2;
        real1 = real_buffer[p * 128];
        real2 = real_buffer[(p - 2) * 128];
        imag1 = img_buffer[p * 128];
        imag2 = img_buffer[(p - 2) * 128];

        real1 = ixheaac_shr32(real1, 3);
        real2 = ixheaac_shr32(real2, 3);
        imag1 = ixheaac_shr32(imag1, 3);
        imag2 = ixheaac_shr32(imag2, 3);

        t_phi_02 = ixheaacd_mac32x16hin32(t_phi_02, real1, real2);
        t_phi_02 = ixheaacd_mac32x16hin32(t_phi_02, imag1, imag2);
        t_phi_02_i = ixheaacd_mac32x16hin32(t_phi_02_i, imag1, real2);
        t_phi_02_i = ixheaacd_macn32x16hin32(t_phi_02_i, real1, imag2);

        real1 = real_buffer[(p + 1) * 128];
        real2 = real_buffer[(p - 1) * 128];
        imag1 = img_buffer[(p + 1) * 128];
        imag2 = img_buffer[(p - 1) * 128];

        real1 = ixheaac_shr32(real1, 3);
        real2 = ixheaac_shr32(real2, 3);
        imag1 = ixheaac_shr32(imag1, 3);
        imag2 = ixheaac_shr32(imag2, 3);

        t_phi_02 = ixheaacd_mac32x16hin32(t_phi_02, real1, real2);
        t_phi_02 = ixheaacd_mac32x16hin32(t_phi_02, imag1, imag2);
        t_phi_02_i = ixheaacd_mac32x16hin32(t_phi_02_i, imag1, real2);
        t_phi_02_i = ixheaacd_macn32x16hin32(t_phi_02_i, real1, imag2);

        real1 = real_buffer[(p + 2) * 128];
        real2 = real_buffer[p * 128];
        imag1 = img_buffer[(p + 2) * 128];
        imag2 = img_buffer[p * 128];

        real1 = ixheaac_shr32(real1, 3);
        real2 = ixheaac_shr32(real2, 3);
        imag1 = ixheaac_shr32(imag1, 3);
        imag2 = ixheaac_shr32(imag2, 3);

        t_phi_02 = ixheaacd_mac32x16hin32(t_phi_02, real1, real2);
        t_phi_02 = ixheaacd_mac32x16hin32(t_phi_02, imag1, imag2);
        t_phi_02_i = ixheaacd_mac32x16hin32(t_phi_02_i, imag1, real2);
        t_phi_02_i = ixheaacd_macn32x16hin32(t_phi_02_i, real1, imag2);

        real1 = real_buffer[(p + 3) * 128];
        real2 = real_buffer[(p + 1) * 128];
        imag1 = img_buffer[(p + 3) * 128];
        imag2 = img_buffer[(p + 1) * 128];

        real1 = ixheaac_shr32(real1, 3);
        real2 = ixheaac_shr32(real2, 3);
        imag1 = ixheaac_shr32(imag1, 3);
        imag2 = ixheaac_shr32(imag2, 3);

        t_phi_02 = ixheaacd_mac32x16hin32(t_phi_02, real1, real2);
        t_phi_02 = ixheaacd_mac32x16hin32(t_phi_02, imag1, imag2);
        t_phi_02_i = ixheaacd_mac32x16hin32(t_phi_02_i, imag1, real2);
        t_phi_02_i = ixheaacd_macn32x16hin32(t_phi_02_i, real1, imag2);
        p += 4;
      }
      n = ixheaac_shl16(len_by_4, 2);
      for (; n < slots; n++) {
        WORD32 real1, real2, imag1, imag2;
        real1 = real_buffer[(n * 128)];
        real2 = real_buffer[(n - 2) * 128];
        imag1 = img_buffer[n * 128];
        imag2 = img_buffer[(n - 2) * 128];

        real1 = ixheaac_shr32(real1, 3);
        real2 = ixheaac_shr32(real2, 3);
        imag1 = ixheaac_shr32(imag1, 3);
        imag2 = ixheaac_shr32(imag2, 3);

        t_phi_02 = ixheaacd_mac32x16hin32(t_phi_02, real1, real2);
        t_phi_02 = ixheaacd_mac32x16hin32(t_phi_02, imag1, imag2);
        t_phi_02_i = ixheaacd_mac32x16hin32(t_phi_02_i, imag1, real2);
        t_phi_02_i = ixheaacd_macn32x16hin32(t_phi_02_i, real1, imag2);
      }

      cov_matrix->phi_02 = t_phi_02;
      cov_matrix->phi_02_im = t_phi_02_i;
    }
    ptr_real++;
    pac_arr++;
  }
}

static PLATFORM_INLINE VOID ixheaacd_filt_step3_lp(WORD len, WORD32 coef1,
                                                   WORD32 coef2,
                                                   WORD32 *pqmf_real_low,
                                                   WORD32 *pqmf_real_high) {
  WORD32 prev1;
  WORD32 prev2;
  WORD32 i;

  prev2 = *pqmf_real_low;
  pqmf_real_low += 64;

  prev1 = *pqmf_real_low;
  pqmf_real_low += 64;

  for (i = len; i >= 0; i -= 2) {
    WORD32 curr = *pqmf_real_low;
    WORD32 temp = ixheaac_mult32x16hin32(prev2, coef2);
    pqmf_real_low += 64;

    *pqmf_real_high = ixheaac_add32_sat((curr >> LPC_SCALE_FACTOR),
                                         ((ixheaacd_mac32x16hin32(temp, prev1, coef1)) << 1));
    pqmf_real_high += 64;

    prev2 = *pqmf_real_low;
    temp = ixheaac_mult32x16hin32(prev1, coef2);
    pqmf_real_low += 64;

    *pqmf_real_high = ixheaac_add32_sat((prev2 >> LPC_SCALE_FACTOR),
                                         ((ixheaacd_mac32x16hin32(temp, curr, coef1)) << 1));
    pqmf_real_high += 64;

    prev1 = prev2;
    prev2 = curr;
  }
}

VOID ixheaacd_filter1_lp(ia_sbr_hf_generator_struct *hf_generator,
                         ia_lpp_trans_cov_matrix *cov_matrix_seq,
                         WORD32 *bw_array, WORD16 *degree_alias,
                         WORD32 start_idx, WORD32 stop_idx,
                         WORD32 max_qmf_subband, WORD32 start_patch,
                         WORD32 stop_patch, WORD32 *sub_sig_x) {
  WORD16 k1, k1_below = 0, k1_below2 = 0;
  WORD32 i;
  WORD16 alpha_real[LPC_ORDER];
  WORD32 low_band, high_band;
  WORD32 patch;
  WORD16 bw = 0;
  WORD32 a0r, a1r;

  WORD num_patches = hf_generator->pstr_settings->num_patches;
  ia_patch_param_struct *patch_param =
      hf_generator->pstr_settings->str_patch_param;
  WORD32 bw_index[MAX_NUM_PATCHES];

  memset(bw_index, 0, sizeof(WORD32) * num_patches);

  for (low_band = start_patch; low_band < stop_patch; low_band++) {
    ia_lpp_trans_cov_matrix *p_cov_matrix = &cov_matrix_seq[low_band];

    alpha_real[1] = 0;
    alpha_real[0] = 0;

    if (p_cov_matrix->d != 0) {
      WORD32 tmp_r, temp_real, modulus_d;
      WORD16 inverse_d;
      WORD32 norm_d;

      norm_d = ixheaac_norm32(p_cov_matrix->d);

      inverse_d =
          (WORD16)(*ixheaacd_fix_div)(0x40000000, (p_cov_matrix->d << norm_d));
      modulus_d = ixheaac_abs32(p_cov_matrix->d);

      tmp_r =
          (ixheaac_sub32_sat(
               ixheaac_mult32(p_cov_matrix->phi_01, p_cov_matrix->phi_12),
               ixheaac_mult32(p_cov_matrix->phi_02, p_cov_matrix->phi_11)) >>
           LPC_SCALE_FACTOR);
      temp_real = ixheaac_abs32(tmp_r);

      if (temp_real < modulus_d) {
        alpha_real[1] = (WORD16)(
            (ixheaac_mult32x16in32_shl_sat(tmp_r, inverse_d) << norm_d) >> 15);
      }

      tmp_r =
          (ixheaac_sub32_sat(
               ixheaac_mult32(p_cov_matrix->phi_02, p_cov_matrix->phi_12),
               ixheaac_mult32(p_cov_matrix->phi_01, p_cov_matrix->phi_22)) >>
           LPC_SCALE_FACTOR);
      temp_real = ixheaac_abs32(tmp_r);

      if (temp_real < modulus_d) {
        alpha_real[0] = (WORD16)(
            (ixheaac_mult32x16in32_shl_sat(tmp_r, inverse_d) << norm_d) >> 15);
      }
    }

    if (p_cov_matrix->phi_11 == 0) {
      k1 = 0;
    } else {
      if (ixheaac_abs32_sat(p_cov_matrix->phi_01) >= p_cov_matrix->phi_11) {
        if (p_cov_matrix->phi_01 < 0) {
          k1 = 0x7fff;
        } else {
          k1 = (WORD16)-0x8000;
        }
      } else {
        k1 = -((WORD16)(
            (*ixheaacd_fix_div)(p_cov_matrix->phi_01, p_cov_matrix->phi_11)));
      }
    }

    if (low_band > 1) {
      WORD16 deg = ixheaac_sub16_sat(
          0x7fff, ixheaac_mult16_shl_sat(k1_below, k1_below));
      degree_alias[low_band] = 0;

      if (((low_band & 1) == 0) && (k1 < 0)) {
        if (k1_below < 0) {
          degree_alias[low_band] = 0x7fff;

          if (k1_below2 > 0) {
            degree_alias[low_band - 1] = deg;
          }
        } else {
          if (k1_below2 > 0) {
            degree_alias[low_band] = deg;
          }
        }
      }

      if (((low_band & 1) != 0) && (k1 > 0)) {
        if (k1_below > 0) {
          degree_alias[low_band] = 0x7fff;

          if (k1_below2 < 0) {
            degree_alias[low_band - 1] = deg;
          }
        } else {
          if (k1_below2 < 0) {
            degree_alias[low_band] = deg;
          }
        }
      }
    }

    k1_below2 = k1_below;
    k1_below = k1;

    patch = 0;
    while (patch < num_patches) {
      ia_patch_param_struct *p_loc_patch_param = &patch_param[patch];
      WORD32 bw_vec, bw_idx;
      WORD16 alpha1, alpha2;

      high_band = (((low_band + p_loc_patch_param->dst_end_band) << 8) >> 8);

      if ((low_band < p_loc_patch_param->src_start_band) ||
          (low_band >= p_loc_patch_param->src_end_band) ||
          (high_band < max_qmf_subband)) {
        patch++;
        continue;
      }

      bw_idx = bw_index[patch];
      while (high_band >= hf_generator->pstr_settings->bw_borders[bw_idx]) {
        bw_idx++;
        bw_index[patch] = bw_idx;
      }

      bw_vec = bw_array[bw_idx];
      alpha1 = alpha_real[0];
      alpha2 = alpha_real[1];

      bw = ixheaac_extract16h(bw_vec);
      a0r = ixheaac_mult16x16in32_shl(bw, alpha1);
      bw = ixheaac_mult16_shl_sat(bw, bw);
      a1r = ixheaac_mult16x16in32_shl(bw, alpha2);

      {
        WORD32 *p_sub_signal_xlow = sub_sig_x + low_band + ((start_idx) << 6);
        WORD32 *p_sub_signal_xhigh =
            sub_sig_x + high_band + ((start_idx + 2) << 6);
        WORD32 len = stop_idx - start_idx - 1;

        if (bw > 0) {
          ixheaacd_filt_step3_lp(len, a0r, a1r, p_sub_signal_xlow,
                                 p_sub_signal_xhigh);

        } else {
          p_sub_signal_xlow += 128;
          for (i = len; i >= 0; i--) {
            *p_sub_signal_xhigh = *p_sub_signal_xlow >> LPC_SCALE_FACTOR;
            p_sub_signal_xlow += 64;
            p_sub_signal_xhigh += 64;
          }
        }
      }

      patch++;
    }
  }
}

VOID ixheaacd_clr_subsamples(WORD32 *ptr_qmf_buf, WORD32 num, WORD32 size) {
  WORD32 i;
  for (i = num; i >= 0; i--) {
    memset(ptr_qmf_buf, 0, sizeof(WORD32) * (size));
    ptr_qmf_buf += 64;
  }
}

VOID ixheaacd_low_pow_hf_generator(ia_sbr_hf_generator_struct *hf_generator,
                                   WORD32 **qmf_real, WORD16 *degree_alias,
                                   WORD32 start_idx, WORD32 stop_idx,
                                   WORD32 num_if_bands, WORD32 max_qmf_subband,
                                   WORD32 *sbr_invf_mode,
                                   WORD32 *sbr_invf_mode_prev, WORD32 norm_max,
                                   WORD32 *sub_sig_x) {
  WORD32 bw_array[MAX_NUM_PATCHES];
  WORD32 i;
  WORD32 start_patch, stop_patch, low_band, high_band;
  ia_patch_param_struct *patch_param =
      hf_generator->pstr_settings->str_patch_param;
  WORD32 patch;
  ia_lpp_trans_cov_matrix cov_matrix_seq[MAX_COLS];

  WORD32 actual_stop_band;
  WORD32 num_patches = hf_generator->pstr_settings->num_patches;
  WORD32 auto_corr_length = hf_generator->pstr_settings->num_columns + 6;

  stop_idx = (hf_generator->pstr_settings->num_columns + stop_idx);

  ixheaacd_invfilt_level_emphasis(hf_generator, num_if_bands, sbr_invf_mode,
                                  sbr_invf_mode_prev, bw_array);

  actual_stop_band =
      ixheaac_add16(patch_param[num_patches - 1].dst_start_band,
                     patch_param[num_patches - 1].num_bands_in_patch);

  {
    WORD32 *p_qmf_real;
    WORD32 len = 6, num;

    if (len > stop_idx) len = stop_idx;

    p_qmf_real = &qmf_real[start_idx][actual_stop_band];
    num = (len - start_idx - 1);
    ixheaacd_clr_subsamples(p_qmf_real, num,
                            (NO_SYNTHESIS_CHANNELS - actual_stop_band));

    if (actual_stop_band < 32) {
      num = (stop_idx - len - 1);
      p_qmf_real = &qmf_real[len][actual_stop_band];
      ixheaacd_clr_subsamples(p_qmf_real, num,
                              (NO_ANALYSIS_CHANNELS - actual_stop_band));
    }
  }

  start_patch = ixheaac_max16(
      1, ixheaac_sub16(hf_generator->pstr_settings->start_patch, 2));
  stop_patch = patch_param[0].dst_start_band;

  {
    WORD32 *ptr = &sub_sig_x[0];
    WORD32 *plpc_filt_states_real = &hf_generator->lpc_filt_states_real[0][0];
    for (i = LPC_ORDER; i != 0; i--) {
      memcpy(ptr, plpc_filt_states_real, sizeof(WORD32) * (stop_patch));
      ptr += NO_SYNTHESIS_CHANNELS;
      plpc_filt_states_real += 32;
    }
  }
  if (norm_max != 30) {
    if (30 == hf_generator->pstr_settings->num_columns) {
      (*ixheaacd_covariance_matrix_calc_960)(sub_sig_x + start_patch,
                                             &cov_matrix_seq[start_patch],
                                             (stop_patch - start_patch),
                                             auto_corr_length);
     } else
      (*ixheaacd_covariance_matrix_calc)(sub_sig_x + start_patch,
                                         &cov_matrix_seq[start_patch],
                                         (stop_patch - start_patch),
                                         auto_corr_length);
  } else {
    memset(&cov_matrix_seq[0], 0,
           sizeof(ia_lpp_trans_cov_matrix) * stop_patch);
  }

  ixheaacd_filter1_lp(hf_generator, cov_matrix_seq, bw_array, degree_alias,
                      start_idx, stop_idx, max_qmf_subband, start_patch,
                      stop_patch, sub_sig_x);

  start_patch = hf_generator->pstr_settings->start_patch;
  stop_patch = hf_generator->pstr_settings->stop_patch;

  for (low_band = start_patch; low_band < stop_patch; low_band++) {
    WORD32 src_start_band, src_end_band, dst_start_band;
    patch = 0;

    while (patch < num_patches) {
      ia_patch_param_struct *ptr_loc_patch_param = &patch_param[patch];

      src_start_band = ptr_loc_patch_param->src_start_band;
      src_end_band = ptr_loc_patch_param->src_end_band;
      dst_start_band = ptr_loc_patch_param->dst_start_band;

      high_band = (low_band + ptr_loc_patch_param->dst_end_band);

      if ((low_band < src_start_band) || (low_band >= src_end_band) ||
          (high_band >= NO_SYNTHESIS_CHANNELS)) {
        patch++;
        continue;
      }

      if ((high_band != dst_start_band)) {
        degree_alias[high_band] = degree_alias[low_band];
      }

      patch++;
    }
  }

  memcpy(hf_generator->bw_array_prev, bw_array, sizeof(WORD32) * num_if_bands);
}

VOID ixheaacd_hf_generator(ia_sbr_hf_generator_struct *hf_generator,
                           ia_sbr_scale_fact_struct *scale_factor,
                           WORD32 **qmf_real, WORD32 **qmf_imag, WORD32 factor,
                           WORD32 start_idx, WORD32 stop_idx,
                           WORD32 num_if_bands, WORD32 max_qmf_subband,
                           WORD32 *sbr_invf_mode, WORD32 *sbr_invf_mode_prev,
                           WORD32 *sub_sig_x, WORD audio_object_type) {
  WORD32 bw_index[MAX_NUM_PATCHES];
  WORD32 bw_array[MAX_NUM_PATCHES];

  WORD32 i, j;
  WORD32 start_patch, stop_patch, low_band, high_band;
  ia_patch_param_struct *patch_param =
      hf_generator->pstr_settings->str_patch_param;
  WORD32 patch;

  WORD16 alpha_real[LPC_ORDER];
  WORD16 a0r, a1r;
  WORD16 alpha_imag[LPC_ORDER];
  WORD16 a0i = 0, a1i = 0;

  WORD16 bw = 0;

  ia_lpp_trans_cov_matrix cov_matrix;
  ia_lpp_trans_cov_matrix cov_matrix_seq[MAX_COLS];

  WORD32 common_scale;
  WORD32 actual_stop_band;
  WORD32 num_patches = hf_generator->pstr_settings->num_patches;

  WORD32  auto_corr_length = hf_generator->pstr_settings->num_columns + 6;

  start_idx = (start_idx * factor);

  stop_idx = (hf_generator->pstr_settings->num_columns + (stop_idx * factor));

  ixheaacd_invfilt_level_emphasis(hf_generator, num_if_bands, sbr_invf_mode,
                                  sbr_invf_mode_prev, bw_array);

  actual_stop_band =
      ixheaac_add16(patch_param[num_patches - 1].dst_start_band,
                     patch_param[num_patches - 1].num_bands_in_patch);

  for (i = start_idx; i < stop_idx; i++) {
    WORD32 *p_qmf_real = &qmf_real[i][actual_stop_band];
    WORD32 *p_qmf_imag = &qmf_imag[i][actual_stop_band];

    for (j = NO_SYNTHESIS_CHANNELS - actual_stop_band; j != 0; j--) {
      *p_qmf_real++ = 0;
      *p_qmf_imag++ = 0;
    }
  }

  memset(bw_index, 0, sizeof(WORD32) * num_patches);

  common_scale =
      ixheaac_min32(scale_factor->ov_lb_scale, scale_factor->lb_scale);

  start_patch = hf_generator->pstr_settings->start_patch;
  stop_patch = hf_generator->pstr_settings->stop_patch;

  {
    WORD32 *ptr;
    for (i = 0; i < LPC_ORDER; i++) {
      ptr = sub_sig_x + (start_patch) + i * 128;
      memcpy(ptr, &hf_generator->lpc_filt_states_real[i][start_patch],
             sizeof(WORD32) * (stop_patch - start_patch));
      memcpy(ptr + 64, &hf_generator->lpc_filt_states_imag[i][start_patch],
             sizeof(WORD32) * (stop_patch - start_patch));
    }
  }
  if (audio_object_type != AOT_ER_AAC_ELD &&
      audio_object_type != AOT_ER_AAC_LD) {
    if (auto_corr_length == 36) {
      (*ixheaacd_covariance_matrix_calc_2)(
          &cov_matrix_seq[start_patch],
          (sub_sig_x + start_patch + LPC_ORDER * 128),
          (stop_patch - start_patch), auto_corr_length);
     } else {
      (*ixheaacd_covariance_matrix_calc_2)(
          &cov_matrix_seq[start_patch],
          (sub_sig_x + start_patch + LPC_ORDER * 128),
          (stop_patch - start_patch), 38);
    }
  } else {
    if (hf_generator->pstr_settings->num_columns == 15) {
      (*ixheaacd_covariance_matrix_calc_2)(
          &cov_matrix_seq[start_patch],
          (sub_sig_x + start_patch + LPC_ORDER * 128),
          (stop_patch - start_patch),
          hf_generator->pstr_settings->num_columns);
     } else {
      (*ixheaacd_covariance_matrix_calc_2)(
          &cov_matrix_seq[start_patch],
          (sub_sig_x + start_patch + LPC_ORDER * 128),
          (stop_patch - start_patch), 16);
    }
  }

  for (low_band = start_patch; low_band < stop_patch; low_band++) {
    FLAG reset_lpc_coeff = 0;
    WORD32 max_val;
    WORD32 q_shift;
    WORD32 v;
    max_val = ixheaac_abs32_nrm(cov_matrix_seq[low_band].phi_01);
    max_val = max_val | ixheaac_abs32_nrm(cov_matrix_seq[low_band].phi_02);
    max_val = max_val | ixheaac_abs32_nrm(cov_matrix_seq[low_band].phi_12);

    max_val = max_val | (cov_matrix_seq[low_band].phi_11);
    max_val = max_val | (cov_matrix_seq[low_band].phi_22);
    max_val = max_val | ixheaac_abs32_nrm(cov_matrix_seq[low_band].phi_01_im);
    max_val = max_val | ixheaac_abs32_nrm(cov_matrix_seq[low_band].phi_02_im);
    max_val = max_val | ixheaac_abs32_nrm(cov_matrix_seq[low_band].phi_12_im);

    q_shift = ixheaac_pnorm32(max_val);

    cov_matrix.phi_11 = (cov_matrix_seq[low_band].phi_11 << q_shift);
    cov_matrix.phi_22 = (cov_matrix_seq[low_band].phi_22 << q_shift);
    cov_matrix.phi_01 = (cov_matrix_seq[low_band].phi_01 << q_shift);
    cov_matrix.phi_02 = (cov_matrix_seq[low_band].phi_02 << q_shift);
    cov_matrix.phi_12 = (cov_matrix_seq[low_band].phi_12 << q_shift);
    cov_matrix.phi_01_im = (cov_matrix_seq[low_band].phi_01_im << q_shift);
    cov_matrix.phi_02_im = (cov_matrix_seq[low_band].phi_02_im << q_shift);
    cov_matrix.phi_12_im = (cov_matrix_seq[low_band].phi_12_im << q_shift);

    max_val = ixheaac_mult32(cov_matrix.phi_12, cov_matrix.phi_12);
    max_val = ixheaac_add32_sat(
        max_val, ixheaac_mult32(cov_matrix.phi_12_im, cov_matrix.phi_12_im));

    v = ixheaac_sub32_sat(
            ixheaac_mult32(cov_matrix.phi_11, cov_matrix.phi_22), max_val)
        << 1;
    cov_matrix.d = v;

    alpha_real[1] = 0;
    alpha_imag[1] = 0;

    if (cov_matrix.d != 0) {
      WORD32 tmp_r, temp_real, modulus_d;
      WORD32 tmp_i, temp_imag;
      WORD16 inverse_d;
      WORD32 norm_d;

      norm_d = ixheaac_norm32(cov_matrix.d);

      inverse_d =
          (WORD16)(*ixheaacd_fix_div)(0x40000000, (cov_matrix.d << norm_d));

      modulus_d = ixheaac_abs32_sat(cov_matrix.d);
      tmp_r =
          (ixheaac_sub32_sat(
              ixheaac_sub32_sat(
                  ixheaac_mult32(cov_matrix.phi_01, cov_matrix.phi_12),
                  ixheaac_mult32(cov_matrix.phi_01_im, cov_matrix.phi_12_im)),
              ixheaac_mult32(cov_matrix.phi_02, cov_matrix.phi_11))) >>
          (LPC_SCALE_FACTOR - 1);
      tmp_i = (ixheaac_sub32_sat(
                  ixheaac_add32_sat(
                      ixheaac_mult32(cov_matrix.phi_01_im, cov_matrix.phi_12),
                      ixheaac_mult32(cov_matrix.phi_01, cov_matrix.phi_12_im)),
                  ixheaac_mult32(cov_matrix.phi_02_im, cov_matrix.phi_11))) >>
              (LPC_SCALE_FACTOR - 1);
      temp_imag = ixheaac_abs32_sat(tmp_i);
      temp_real = ixheaac_abs32_sat(tmp_r);

      if (temp_real >= modulus_d) {
        reset_lpc_coeff = 1;
      } else {
        alpha_real[1] = (WORD16)(
            (ixheaac_mult32x16in32(tmp_r, inverse_d) << (norm_d + 1)) >> 15);
      }

      if (temp_imag >= modulus_d) {
        reset_lpc_coeff = 1;
      } else {
        alpha_imag[1] = (WORD16)(
            (ixheaac_mult32x16in32(tmp_i, inverse_d) << (norm_d + 1)) >> 15);
      }
    }

    alpha_real[0] = 0;
    alpha_imag[0] = 0;

    if (cov_matrix.phi_11 != 0) {
      WORD32 tmp_r, temp_real;
      WORD32 tmp_i = 0, temp_imag = 0;
      WORD16 inverse_r11;
      WORD32 norm_r11;

      norm_r11 = ixheaac_norm32(cov_matrix.phi_11);

      inverse_r11 = (WORD16)(*ixheaacd_fix_div)(
          0x40000000, (cov_matrix.phi_11 << norm_r11));

      tmp_r = ixheaac_add32_sat(
          ixheaac_add32(
              (cov_matrix.phi_01 >> (LPC_SCALE_FACTOR + 1)),
              ixheaac_mult32x16in32(cov_matrix.phi_12, alpha_real[1])),
          ixheaac_mult32x16in32(cov_matrix.phi_12_im, alpha_imag[1]));
      tmp_i = ixheaac_sub32_sat(
          ixheaac_add32(
              (cov_matrix.phi_01_im >> (LPC_SCALE_FACTOR + 1)),
              ixheaac_mult32x16in32(cov_matrix.phi_12, alpha_imag[1])),
          ixheaac_mult32x16in32(cov_matrix.phi_12_im, alpha_real[1]));

      tmp_r = tmp_r << 1;
      tmp_i = tmp_i << 1;

      temp_imag = ixheaac_abs32_sat(tmp_i);
      temp_real = ixheaac_abs32_sat(tmp_r);

      if (temp_real >= cov_matrix.phi_11) {
        reset_lpc_coeff = 1;
      } else {
        alpha_real[0] = (WORD16)(
            (ixheaac_mult32x16in32(ixheaac_sub32_sat(0, tmp_r), inverse_r11)
             << (norm_r11 + 1)) >>
            15);
      }

      if (temp_imag >= cov_matrix.phi_11) {
        reset_lpc_coeff = 1;
      } else {
        alpha_imag[0] = (WORD16)(
            (ixheaac_mult32x16in32(ixheaac_sub32_sat(0, tmp_i), inverse_r11)
             << (norm_r11 + 1)) >>
            15);
      }
    }

    if (ixheaac_add32_sat((alpha_real[0] * alpha_real[0]),
                           (alpha_imag[0] * alpha_imag[0])) >= 0x40000000L) {
      reset_lpc_coeff = 1;
    }

    if (ixheaac_add32_sat((alpha_real[1] * alpha_real[1]),
                           (alpha_imag[1] * alpha_imag[1])) >= 0x40000000L) {
      reset_lpc_coeff = 1;
    }

    if (reset_lpc_coeff) {
      alpha_real[0] = 0;
      alpha_real[1] = 0;
      alpha_imag[0] = 0;
      alpha_imag[1] = 0;
    }

    patch = 0;

    while (patch < num_patches) {
      high_band = (low_band + patch_param[patch].dst_end_band);

      if ((low_band < patch_param[patch].src_start_band) ||
          (low_band >= patch_param[patch].src_end_band)) {
        patch++;
        continue;
      }

      if (high_band < max_qmf_subband) {
        patch++;
        continue;
      }

      while ((bw_index[patch] < MAX_NUM_PATCHES - 1) &&
             (bw_index[patch] < MAX_NUM_NOISE_VALUES) &&
             (high_band >=
              hf_generator->pstr_settings->bw_borders[bw_index[patch]])) {
        bw_index[patch]++;
      }

      bw = ixheaac_extract16h(bw_array[bw_index[patch]]);
      a0r = ixheaac_mult16_shl_sat(bw, alpha_real[0]);
      a0i = ixheaac_mult16_shl_sat(bw, alpha_imag[0]);
      bw = ixheaac_mult16_shl_sat(bw, bw);
      a1r = ixheaac_mult16_shl_sat(bw, alpha_real[1]);
      a1i = ixheaac_mult16_shl_sat(bw, alpha_imag[1]);

      if (bw > 0) {
        ixheaacd_filterstep3(a0r, a0i, a1r, a1i, start_idx, stop_idx, low_band,
                             high_band, sub_sig_x);

      } else {
        WORD32 *p_src = sub_sig_x + low_band + ((start_idx + 2) << 7);
        WORD32 *p_dst = sub_sig_x + high_band + ((start_idx + 2) << 7);

        for (i = stop_idx - start_idx; i != 0; i--) {
          *(p_dst) = *(p_src) >> LPC_SCALE_FACTOR;
          p_src += 64;
          p_dst += 64;
          *(p_dst) = *(p_src) >> LPC_SCALE_FACTOR;
          p_src += 64;
          p_dst += 64;
        }
      }

      patch++;
    }
  }

  memcpy(hf_generator->bw_array_prev, bw_array, sizeof(WORD32) * num_if_bands);

  scale_factor->hb_scale = (WORD16)(common_scale - LPC_SCALE_FACTOR);
}
