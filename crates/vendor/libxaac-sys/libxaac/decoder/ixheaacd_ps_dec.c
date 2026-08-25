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
#include "ixheaac_basic_ops_arr.h"
#include "ixheaac_basic_ops.h"

#include "ixheaacd_defines.h"
#include "ixheaac_basic_op.h"
#include "ixheaacd_intrinsics.h"
#include "ixheaacd_common_rom.h"
#include "ixheaacd_basic_funcs.h"
#include "ixheaacd_bitbuffer.h"
#include "ixheaacd_sbrdecsettings.h"
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

#include "ixheaacd_ps_dec.h"

#include "ixheaacd_qmf_dec.h"
#include "ixheaacd_env_calc.h"
#include "ixheaac_sbr_const.h"

#include "ixheaacd_pvc_dec.h"
#include "ixheaacd_sbr_dec.h"
#include "ixheaacd_function_selector.h"

static PLATFORM_INLINE WORD16 ixheaacd_shl16_saturate(WORD16 op1,
                                                      WORD16 shift) {
  WORD16 var_out;
  WORD32 temp;

  temp = (WORD32)(op1 << shift);
  var_out = ixheaac_sat16(temp);
  return (var_out);
}

static PLATFORM_INLINE VOID ixheaacd_shl16_arr_saturate(WORD16 *word16_arr,
                                                        WORD16 shift, WORD n) {
  WORD i;

  for (i = n - 1; i >= 0; i--) {
    *word16_arr = ixheaacd_shl16_saturate(*word16_arr, shift);
    word16_arr++;
  }

  return;
}

VOID ixheaacd_scale_short_vec_left(WORD16 *word16_arr, WORD32 n, WORD16 shift) {
  ixheaacd_shl16_arr_saturate(word16_arr, shift, n);
}

VOID ixheaacd_scale_int_vec_left(WORD32 *word32_arr, WORD32 n, WORD16 shift) {
  ixheaac_shl32_arr_sat(word32_arr, shift, n);
}

VOID ixheaacd_scale_int_vec_right(WORD32 *word32_arr, WORD32 n, WORD16 shift) {
  ixheaac_shr32_arr(word32_arr, shift, n);
}

VOID ixheaacd_scale_short_vec_right(WORD16 *word16_arr, WORD32 n,
                                    WORD16 shift) {
  ixheaac_shr16_arr(word16_arr, shift, n);
}

WORD32 ixheaacd_calc_max(WORD16 *array, WORD32 size) {
  WORD n;
  WORD32 max_val = 0;
  WORD16 temp1, temp2;

  for (n = size; n != 0; n--) {
    temp1 = *array++;
    temp2 = *array++;

    max_val = max_val | ixheaac_abs32_nrm(temp1);
    max_val = max_val | ixheaac_abs32_nrm(temp2);
  }

  return max_val;
}

static WORD ixheaacd_get_ps_scale(ia_ps_dec_struct *ptr_ps_dec) {
  WORD i, m, n, len;
  WORD32 max_val = 0;
  WORD16 *ptr_re;
  WORD32 *ptr_re_temp, *ptr_im;

  for (m = 0; m < 2; m++) {
    ptr_re = &ptr_ps_dec->delay_buf_qmf_ap_re_im[m][2 * 3];
    max_val |=
        ixheaacd_calc_max(ptr_re, NUM_OF_QUAD_MIRROR_FILTER_ALL_PASS_CHNLS);
  }

  ptr_re = (WORD16 *)ptr_ps_dec->delay_buf_qmf_ld_re_im;

  max_val |= ixheaacd_calc_max(ptr_re, HIGH_DEL * SMALL_DEL_STRT);

  ptr_re = (WORD16 *)ptr_ps_dec->delay_buf_qmf_sd_re_im;

  max_val |= ixheaacd_calc_max(
      ptr_re, (SMALL_DEL *
               (NUM_OF_QUAD_MIRROR_FILTER_ICC_CHNLS -
                (NUM_OF_QUAD_MIRROR_FILTER_ALL_PASS_CHNLS + SMALL_DEL_STRT))));

  ptr_re = (WORD16 *)ptr_ps_dec->delay_buf_qmf_sub_re_im;

  max_val |= ixheaacd_calc_max(ptr_re, 16 * DEL_ALL_PASS);

  for (i = 0; i < NUM_SER_AP_LINKS; i++) {
    for (m = 0; m < ptr_ps_dec->delay_sample_ser[i]; m++) {
      ptr_re = &ptr_ps_dec->delay_buf_qmf_ser_re_im[m][i][2 * 3];

      max_val |=
          ixheaacd_calc_max(ptr_re, NUM_OF_QUAD_MIRROR_FILTER_ALL_PASS_CHNLS);
    }
  }

  ptr_re = (WORD16 *)ptr_ps_dec->delay_buf_qmf_sub_ser_re_im;
  max_val |= ixheaacd_calc_max(ptr_re, NUM_SER_AP_LINKS * 5 * 16);

  max_val = max_val << 16;

  len = ptr_ps_dec->str_hybrid.ptr_qmf_buf;

  for (i = 0; i < NO_QMF_CHANNELS_IN_HYBRID; i++) {
    ptr_re_temp = &ptr_ps_dec->str_hybrid.ptr_qmf_buf_re[i][0];
    ptr_im = &ptr_ps_dec->str_hybrid.ptr_qmf_buf_im[i][0];

    for (n = len; n != 0; n--) {
      WORD32 temp3 = *ptr_re_temp++;
      WORD32 temp4 = *ptr_im++;

      max_val = max_val | ixheaac_abs32_nrm(temp3);
      max_val = max_val | ixheaac_abs32_nrm(temp4);
    }
  }

  return ixheaac_pnorm32(max_val);
}

VOID ixheaacd_init_ps_scale(ia_ps_dec_struct *ptr_ps_dec,
                            ia_sbr_scale_fact_struct *sbr_scale_factor)

{
  WORD32 reserve, change;
  WORD16 temp;

  reserve = ixheaacd_get_ps_scale(ptr_ps_dec);

  ptr_ps_dec->delay_buffer_scale = (ptr_ps_dec->delay_buffer_scale + reserve);
  temp =
      ixheaac_min16(sbr_scale_factor->lb_scale, sbr_scale_factor->ov_lb_scale);
  temp = ixheaac_min16(temp, sbr_scale_factor->hb_scale);
  temp = ixheaac_min16(temp, ptr_ps_dec->delay_buffer_scale);
  sbr_scale_factor->ps_scale = (temp - 1);

  change = (sbr_scale_factor->ps_scale - ptr_ps_dec->delay_buffer_scale);
  change = (change + reserve);

  ixheaacd_scale_ps_states(ptr_ps_dec, (WORD16)change);

  ptr_ps_dec->delay_buffer_scale = sbr_scale_factor->ps_scale;
}

WORD32 ixheaacd_divide16_pos_dec(WORD32 op1, WORD32 op2) {
  UWORD32 v;
  UWORD32 u;
  WORD k, nrm;

  nrm = ixheaac_norm32(op2);
  u = (op1 << nrm);
  v = (op2 << nrm);
  u = (u & 0xffff0000);
  v = (v & 0xffff0000);

  if (u != 0) {
    for (k = 16; k > 0; k--) {
      if (u >= (UWORD32)v) {
        u = ((u - v) << 1) + 1;
      } else {
        u = (u << 1);
      }
    }
  }

  return (u);
}

VOID ixheaacd_decorr_filter1_dec(ia_ps_dec_struct *ptr_ps_dec,
                                 ia_ps_tables_struct *ps_tables_ptr,
                                 WORD16 *transient_ratio) {
  WORD sb;
  WORD m;

  WORD16 delay_buf_idx;
  WORD16 *p_delay_buf_sub_re_im;
  WORD16 *p_frac_delay_phase_fac_ser_re_im, *p_frac_delay_phase_fac_ser_re_im1;

  const WORD16 *p_frac_delay_phase_fac_re_im;
  REVERB_BUFFERS_CH_RI *p_delay_buf_ser_sub_re_im;

  WORD32 *p_left_real;
  WORD32 *p_left_imag;
  WORD32 *p_right_real;
  WORD32 *p_right_imag;

  p_frac_delay_phase_fac_re_im =
      &ps_tables_ptr->frac_delay_phase_fac_qmf_sub_re_im[0];
  p_delay_buf_ser_sub_re_im = &ptr_ps_dec->delay_buf_qmf_sub_ser_re_im;
  p_frac_delay_phase_fac_ser_re_im =
      &ps_tables_ptr->frac_delay_phase_fac_qmf_sub_ser_re_im[0][0];

  p_left_real = ptr_ps_dec->ptr_hyb_left_re;
  p_left_imag = ptr_ps_dec->ptr_hyb_left_im;
  p_right_real = ptr_ps_dec->ptr_hyb_right_re;
  p_right_imag = ptr_ps_dec->ptr_hyb_right_im;

  delay_buf_idx = ptr_ps_dec->delay_buf_idx;
  p_delay_buf_sub_re_im =
      &ptr_ps_dec->delay_buf_qmf_sub_re_im[delay_buf_idx][0];
  for (sb = 0; sb < SUBQMF_GROUPS; sb++) {
    WORD16 real_tmp, imag_tmp, real_tmp0, imag_tmp0, real_in, imag_in, bin;

    real_tmp0 = p_delay_buf_sub_re_im[0];
    imag_tmp0 = p_delay_buf_sub_re_im[1];

    real_in = (WORD16)(
        ixheaac_sub32_sat(
            ixheaac_mult16x16in32(real_tmp0, p_frac_delay_phase_fac_re_im[0]),
            ixheaac_mult16x16in32(imag_tmp0,
                                   p_frac_delay_phase_fac_re_im[1])) >>
        15);
    imag_in = (WORD16)(
        ixheaac_add32_sat(
            ixheaac_mult16x16in32(real_tmp0, p_frac_delay_phase_fac_re_im[1]),
            ixheaac_mult16x16in32(imag_tmp0,
                                   p_frac_delay_phase_fac_re_im[0])) >>
        15);
    *p_delay_buf_sub_re_im++ = ixheaac_round16(p_left_real[sb]);
    *p_delay_buf_sub_re_im++ = ixheaac_round16(p_left_imag[sb]);
    p_frac_delay_phase_fac_re_im += 2;

    p_frac_delay_phase_fac_ser_re_im1 = p_frac_delay_phase_fac_ser_re_im;
    p_frac_delay_phase_fac_ser_re_im += 2;

    for (m = 0; m < NUM_SER_AP_LINKS; m++) {
      WORD16 decay;
      WORD16 delay_buf_idx_ser;
      delay_buf_idx_ser = ptr_ps_dec->delay_buf_idx_ser[m];
      real_tmp0 = (*p_delay_buf_ser_sub_re_im)[delay_buf_idx_ser][m][2 * sb];
      imag_tmp0 =
          (*p_delay_buf_ser_sub_re_im)[delay_buf_idx_ser][m][2 * sb + 1];

      real_tmp =
          (WORD16)(ixheaac_sub32_sat(
                       ixheaac_mult16x16in32(
                           real_tmp0, p_frac_delay_phase_fac_ser_re_im1[0]),
                       ixheaac_mult16x16in32(
                           imag_tmp0, p_frac_delay_phase_fac_ser_re_im1[1])) >>
                   15);
      imag_tmp =
          (WORD16)(ixheaac_add32_sat(
                       ixheaac_mult16x16in32(
                           real_tmp0, p_frac_delay_phase_fac_ser_re_im1[1]),
                       ixheaac_mult16x16in32(
                           imag_tmp0, p_frac_delay_phase_fac_ser_re_im1[0])) >>
                   15);

      decay = ps_tables_ptr->rev_link_decay_ser[m];

      real_tmp = ixheaac_sub16(real_tmp, ixheaac_mult16_shl(real_in, decay));
      imag_tmp = ixheaac_sub16(imag_tmp, ixheaac_mult16_shl(imag_in, decay));

      (*p_delay_buf_ser_sub_re_im)[delay_buf_idx_ser][m][sb * 2] =
          ixheaac_add16(real_in, ixheaac_mult16_shl(real_tmp, decay));
      (*p_delay_buf_ser_sub_re_im)[delay_buf_idx_ser][m][sb * 2 + 1] =
          ixheaac_add16(imag_in, ixheaac_mult16_shl(imag_tmp, decay));

      real_in = real_tmp;
      imag_in = imag_tmp;
      p_frac_delay_phase_fac_ser_re_im1 += 32;
    }

    bin = ps_tables_ptr->hybrid_to_bin[sb];
    p_right_real[sb] =
        ixheaac_mult16x16in32_shl(real_in, transient_ratio[bin]);
    p_right_imag[sb] =
        ixheaac_mult16x16in32_shl(imag_in, transient_ratio[bin]);
  }
}

VOID ixheaacd_decorr_filter2_dec(
    ia_ps_dec_struct *ptr_ps_dec, WORD32 *p_buf_left_real,
    WORD32 *p_buf_left_imag, WORD32 *p_buf_right_real, WORD32 *p_buf_right_imag,
    ia_ps_tables_struct *ps_tables_ptr, WORD16 *transient_ratio) {
  WORD sb, di, sb_delay;
  WORD m, bin;
  WORD32 *p_left_real;
  WORD32 *p_left_imag;
  WORD32 *p_right_real;
  WORD32 *p_right_imag;
  WORD16 delay_buf_idx;
  REVERB_BUFFERS_RI *p_delay_buf_ser_re_im;
  WORD16 *p_delay_buf_ap_re_im;
  const WORD16 *p_frac_delay_phase_fac_re_im;
  WORD16 *p_frac_delay_phase_fac_ser_ap_re_im,
      *p_frac_delay_phase_fac_ser_ap_re_im_temp;

  p_left_real = p_buf_left_real;
  p_left_imag = p_buf_left_imag;
  p_right_real = p_buf_right_real;
  p_right_imag = p_buf_right_imag;

  p_delay_buf_ser_re_im = &ptr_ps_dec->delay_buf_qmf_ser_re_im;
  p_frac_delay_phase_fac_re_im = ps_tables_ptr->frac_delay_phase_fac_qmf_re_im;
  p_frac_delay_phase_fac_ser_ap_re_im =
      &ps_tables_ptr->frac_delay_phase_fac_qmf_ser_re_im[0][0];

  delay_buf_idx = ptr_ps_dec->delay_buf_idx;

  p_delay_buf_ap_re_im = &ptr_ps_dec->delay_buf_qmf_ap_re_im[delay_buf_idx][0];
  p_frac_delay_phase_fac_re_im += 6;
  p_frac_delay_phase_fac_ser_ap_re_im += 6;
  p_delay_buf_ap_re_im += 6;

  for (sb = 3, di = 9; sb < 23; sb++) {
    WORD16 real_tmp, imag_tmp, real_tmp0, imag_tmp0, real_in, imag_in;

    sb_delay = sb;

    real_tmp0 = p_delay_buf_ap_re_im[0];
    imag_tmp0 = p_delay_buf_ap_re_im[1];

    real_in = (WORD16)(
        ixheaac_sub32_sat(
            ixheaac_mult16x16in32(real_tmp0, p_frac_delay_phase_fac_re_im[0]),
            ixheaac_mult16x16in32(imag_tmp0,
                                   p_frac_delay_phase_fac_re_im[1])) >>
        15);
    imag_in = (WORD16)(
        ixheaac_add32_sat(
            ixheaac_mult16x16in32(real_tmp0, p_frac_delay_phase_fac_re_im[1]),
            ixheaac_mult16x16in32(imag_tmp0,
                                   p_frac_delay_phase_fac_re_im[0])) >>
        15);

    *p_delay_buf_ap_re_im++ = ixheaac_round16(p_left_real[sb]);
    *p_delay_buf_ap_re_im++ = ixheaac_round16(p_left_imag[sb]);

    p_frac_delay_phase_fac_re_im += 2;

    p_frac_delay_phase_fac_ser_ap_re_im_temp =
        p_frac_delay_phase_fac_ser_ap_re_im;
    p_frac_delay_phase_fac_ser_ap_re_im += 2;

    for (m = 0; m < NUM_SER_AP_LINKS; m++, di++) {
      WORD16 decay;
      WORD16 delay_buf_idx_ser;
      delay_buf_idx_ser = ptr_ps_dec->delay_buf_idx_ser[m];

      real_tmp0 = (*p_delay_buf_ser_re_im)[delay_buf_idx_ser][m][sb_delay * 2];
      imag_tmp0 =
          (*p_delay_buf_ser_re_im)[delay_buf_idx_ser][m][sb_delay * 2 + 1];

      real_tmp = (WORD16)(
          ixheaac_sub32_sat(
              ixheaac_mult16x16in32(
                  real_tmp0, p_frac_delay_phase_fac_ser_ap_re_im_temp[0]),
              ixheaac_mult16x16in32(
                  imag_tmp0, p_frac_delay_phase_fac_ser_ap_re_im_temp[1])) >>
          15);
      imag_tmp = (WORD16)(
          ixheaac_add32_sat(
              ixheaac_mult16x16in32(
                  real_tmp0, p_frac_delay_phase_fac_ser_ap_re_im_temp[1]),
              ixheaac_mult16x16in32(
                  imag_tmp0, p_frac_delay_phase_fac_ser_ap_re_im_temp[0])) >>
          15);

      decay = ps_tables_ptr->decay_scale_factor[di];

      real_tmp = ixheaac_sub16(real_tmp, ixheaac_mult16_shl(real_in, decay));
      imag_tmp = ixheaac_sub16(imag_tmp, ixheaac_mult16_shl(imag_in, decay));

      (*p_delay_buf_ser_re_im)[delay_buf_idx_ser][m][sb_delay * 2] =
          ixheaac_add16(real_in, ixheaac_mult16_shl(real_tmp, decay));
      (*p_delay_buf_ser_re_im)[delay_buf_idx_ser][m][sb_delay * 2 + 1] =
          ixheaac_add16(imag_in, ixheaac_mult16_shl(imag_tmp, decay));

      real_in = real_tmp;
      imag_in = imag_tmp;
      p_frac_delay_phase_fac_ser_ap_re_im_temp += 64;
    }

    bin = ps_tables_ptr->delay_to_bin[sb_delay];
    p_right_real[sb] =
        ixheaac_mult16x16in32_shl(real_in, transient_ratio[bin]);
    p_right_imag[sb] =
        ixheaac_mult16x16in32_shl(imag_in, transient_ratio[bin]);
  }
}

VOID ixheaacd_decorrelation_dec(ia_ps_dec_struct *ptr_ps_dec,
                                WORD32 *p_buf_left_real,
                                WORD32 *p_buf_left_imag,
                                WORD32 *p_buf_right_real,
                                WORD32 *p_buf_right_imag,
                                ia_ps_tables_struct *ps_tables_ptr) {
  WORD sb;

  WORD gr, bin, sband, maxsband;

  WORD32 peak_diff, nrg;
  WORD32 power_buf[NUM_OF_BINS];
  WORD16 transient_ratio[NUM_OF_BINS + 1];

  WORD32 *p_left_real;
  WORD32 *p_left_imag;
  WORD32 *p_right_real;
  WORD32 *p_right_imag;

  WORD16 *p_delay_buf_re_im_ld;
  WORD16 *p_delay_buf_re_im_sd;

  WORD usb = ptr_ps_dec->usb;
  WORD16 delay_buf_idx;

  p_left_real = ptr_ps_dec->ptr_hyb_left_re;
  p_left_imag = ptr_ps_dec->ptr_hyb_left_im;
  p_right_real = ptr_ps_dec->ptr_hyb_right_re;
  p_right_imag = ptr_ps_dec->ptr_hyb_right_im;

  {
    WORD32 re0, im0, re1, im1;

    re0 = (p_left_real[0]);
    im0 = (p_left_imag[0]);
    re1 = (p_left_real[5]);
    im1 = (p_left_imag[5]);

    power_buf[0] = ixheaac_mult32x16in32(re0, (WORD16)(re0 >> 16));
    power_buf[0] = ixheaac_add32_sat(
        power_buf[0], ixheaac_mult32x16in32(im0, (WORD16)(im0 >> 16)));
    power_buf[0] = ixheaac_add32_sat(
        power_buf[0], ixheaac_mult32x16in32(re1, (WORD16)(re1 >> 16)));
    power_buf[0] = ixheaac_add32_sat(
        power_buf[0], ixheaac_mult32x16in32(im1, (WORD16)(im1 >> 16)));

    re0 = (p_left_real[4]);
    im0 = (p_left_imag[4]);
    re1 = (p_left_real[1]);
    im1 = (p_left_imag[1]);

    power_buf[1] = ixheaac_mult32x16in32(re0, (WORD16)(re0 >> 16));
    power_buf[1] = ixheaac_add32_sat(
        power_buf[1], ixheaac_mult32x16in32(im0, (WORD16)(im0 >> 16)));
    power_buf[1] = ixheaac_add32_sat(
        power_buf[1], ixheaac_mult32x16in32(re1, (WORD16)(re1 >> 16)));
    power_buf[1] = ixheaac_add32_sat(
        power_buf[1], ixheaac_mult32x16in32(im1, (WORD16)(im1 >> 16)));
  }

  bin = 4 - 2;
  for (gr = 4; gr < SUBQMF_GROUPS; gr++) {
    WORD32 re, im;
    sb = ps_tables_ptr->borders_group[gr];
    re = (p_left_real[sb]);
    im = (p_left_imag[sb]);
    power_buf[bin] = ixheaac_mult32x16in32(re, (WORD16)(re >> 16));
    power_buf[bin] = ixheaac_add32_sat(
        power_buf[bin], ixheaac_mult32x16in32(im, (WORD16)(im >> 16)));
    bin++;
  }

  p_left_real = p_buf_left_real;
  p_left_imag = p_buf_left_imag;

  bin = NO_QMF_CHANNELS_IN_HYBRID + 5;
  for (sband = NO_QMF_CHANNELS_IN_HYBRID; sband < NO_QMF_CHANNELS_IN_HYBRID + 6;
       sband++) {
    WORD32 re = (p_left_real[sband]);
    WORD32 im = (p_left_imag[sband]);
    power_buf[bin] = ixheaac_mult32x16in32(re, (WORD16)(re >> 16));
    power_buf[bin] = ixheaac_add32_sat(
        power_buf[bin], ixheaac_mult32x16in32(im, (WORD16)(im >> 16)));
    bin++;
  }

  bin = 16 - 2;
  for (gr = 16; gr < NO_IID_GROUPS; gr++) {
    WORD32 accu = 0, tmp;
    WORD32 re, im;

    maxsband = ixheaac_min32(usb, ps_tables_ptr->borders_group[gr + 1]);

    for (sband = ps_tables_ptr->borders_group[gr]; sband < maxsband; sband++) {
      re = (p_left_real[sband]);
      im = (p_left_imag[sband]);

      tmp = ixheaac_mult32x16in32(re, (WORD16)(re >> 16));
      tmp = ixheaac_add32_sat(tmp,
                               ixheaac_mult32x16in32(im, (WORD16)(im >> 16)));
      tmp = (tmp >> ps_tables_ptr->group_shift[gr - (SUBQMF_GROUPS + 6)]);

      accu = ixheaac_add32_sat(accu, tmp);
    }
    power_buf[bin] = accu;
    bin++;
  }

  p_left_real = ptr_ps_dec->ptr_hyb_left_re;
  p_left_imag = ptr_ps_dec->ptr_hyb_left_im;

  for (bin = 0; bin < NUM_OF_BINS; bin++) {
    power_buf[bin] = ixheaac_shl32(power_buf[bin], 1);

    power_buf[bin] = ixheaac_max32(0, power_buf[bin]);

    ptr_ps_dec->peak_decay_diff[bin] = ixheaac_mult32x16in32_shl(
        ptr_ps_dec->peak_decay_diff[bin], PEAK_DECAYING_FACT);

    ptr_ps_dec->peak_decay_diff[bin] =
        ixheaac_max32(ptr_ps_dec->peak_decay_diff[bin], power_buf[bin]);

    peak_diff = ixheaac_add32_sat(
        ixheaac_mult32x16in32_shl(ptr_ps_dec->peak_decay_diff_prev[bin],
                                   0x6000),
        ((ixheaac_sub32_sat(ptr_ps_dec->peak_decay_diff[bin],
                             power_buf[bin]) >>
          2)));

    ptr_ps_dec->peak_decay_diff_prev[bin] = peak_diff;

    nrg = ixheaac_add32_sat(
        ixheaac_mult32x16in32_shl(ptr_ps_dec->energy_prev[bin], 0x6000),
        (power_buf[bin] >> 2));
    ptr_ps_dec->energy_prev[bin] = nrg;

    peak_diff = ixheaac_add32_sat(peak_diff, (peak_diff >> 1));

    if (peak_diff <= nrg) {
      transient_ratio[bin] = 0x7fff;
    } else {
      transient_ratio[bin] =
          ixheaac_extract16l((*ixheaacd_divide16_pos)(nrg, peak_diff));
    }
  }

  (*ixheaacd_decorr_filter1)(ptr_ps_dec, ps_tables_ptr, transient_ratio);

  transient_ratio[20] = 0;

  (*ixheaacd_decorr_filter2)(ptr_ps_dec, p_buf_left_real, p_buf_left_imag,
                             p_buf_right_real, p_buf_right_imag, ps_tables_ptr,
                             transient_ratio);

  {
    WORD16 trans_ratio = transient_ratio[18];

    p_left_real = p_buf_left_real;
    p_left_imag = p_buf_left_imag;
    p_right_real = p_buf_right_real;
    p_right_imag = p_buf_right_imag;

    maxsband = ixheaac_min32((WORD16)usb, ps_tables_ptr->borders_group[21]);
    delay_buf_idx = ptr_ps_dec->delay_buf_idx_long;
    p_delay_buf_re_im_ld =
        &ptr_ps_dec->delay_buf_qmf_ld_re_im[delay_buf_idx][0];

    for (sband = ps_tables_ptr->borders_group[20]; sband < maxsband; sband++) {
      WORD16 real_in, imag_in;

      real_in = p_delay_buf_re_im_ld[0];
      imag_in = p_delay_buf_re_im_ld[1];
      *p_delay_buf_re_im_ld++ = ixheaac_round16(p_left_real[sband]);
      *p_delay_buf_re_im_ld++ = ixheaac_round16(p_left_imag[sband]);

      p_right_real[sband] = ixheaac_mult16x16in32_shl(real_in, trans_ratio);
      p_right_imag[sband] = ixheaac_mult16x16in32_shl(imag_in, trans_ratio);
    }

    ptr_ps_dec->delay_buf_idx_long =
        ixheaac_add16(ptr_ps_dec->delay_buf_idx_long, 1);

    if (ptr_ps_dec->delay_buf_idx_long >= 14) {
      ptr_ps_dec->delay_buf_idx_long = 0;
    }

    p_delay_buf_re_im_sd = &ptr_ps_dec->delay_buf_qmf_sd_re_im[0][0];

    trans_ratio = transient_ratio[19];
    maxsband = ixheaac_min32((WORD16)usb, ps_tables_ptr->borders_group[22]);
    for (sband = ps_tables_ptr->borders_group[21]; sband < maxsband; sband++) {
      WORD16 real_in, imag_in;

      real_in = p_delay_buf_re_im_sd[0];
      imag_in = p_delay_buf_re_im_sd[1];
      *p_delay_buf_re_im_sd++ = ixheaac_round16(p_left_real[sband]);
      *p_delay_buf_re_im_sd++ = ixheaac_round16(p_left_imag[sband]);

      p_right_real[sband] = ixheaac_mult16x16in32_shl(real_in, trans_ratio);
      p_right_imag[sband] = ixheaac_mult16x16in32_shl(imag_in, trans_ratio);
    }
  }

  for (sband = usb; sband < NO_SYNTHESIS_CHANNELS; sband++) {
    p_right_real[sband] = 0;
    p_right_imag[sband] = 0;
  }

  ptr_ps_dec->delay_buf_idx = (WORD16)(ptr_ps_dec->delay_buf_idx + 1);
  if (ptr_ps_dec->delay_buf_idx >= DEL_ALL_PASS) {
    ptr_ps_dec->delay_buf_idx = 0;
  }

  {
    WORD delay_m;

    for (delay_m = 0; delay_m < NUM_SER_AP_LINKS; delay_m++) {
      ptr_ps_dec->delay_buf_idx_ser[delay_m] =
          (ptr_ps_dec->delay_buf_idx_ser[delay_m] + 1);
      if (ptr_ps_dec->delay_buf_idx_ser[delay_m] >=
          ptr_ps_dec->delay_sample_ser[delay_m]) {
        ptr_ps_dec->delay_buf_idx_ser[delay_m] = 0;
      }
    }
  }
}

static WORD16 ixheaacd_cos512(WORD phi_by_4, const WORD16 *cos_sin_lookup_tab) {
  WORD index;
  index = ixheaac_round16(ixheaac_abs32_sat(phi_by_4));

  index = (index & 0x3FF);

  if (index < 512) {
    return cos_sin_lookup_tab[512 - index];
  } else {
    return (WORD16)(-(cos_sin_lookup_tab[index - 512]));
  }
}

static WORD16 ixheaacd_sin512(WORD phi_by_4, const WORD16 *cos_sin_lookup_tab) {
  WORD index;

  index = ixheaac_round16(phi_by_4);

  if (index < 0) {
    index = (-(index)&0x3FF);

    if (index < 512) {
      return (WORD16)(-cos_sin_lookup_tab[index]);
    } else {
      return (WORD16)(-cos_sin_lookup_tab[1024 - index]);
    }
  } else {
    index = (index & 0x3FF);

    if (index < 512) {
      return cos_sin_lookup_tab[index];
    } else {
      return cos_sin_lookup_tab[1024 - index];
    }
  }
}

VOID ixheaacd_init_rot_env(ia_ps_dec_struct *ptr_ps_dec, WORD16 env, WORD16 usb,
                           ia_sbr_tables_struct *sbr_tables_ptr,
                           const WORD16 *cos_sin_lookup_tab) {
  WORD group, bin, num_iid_steps;
  WORD16 c2, c1;
  WORD32 alpha, beta;
  WORD16 h11, h12, h21, h22;
  WORD16 inv_env_len;
  const WORD16 *p_scale_factors;
  WORD16 *p_iid_idx;
  WORD indexplusa, indexminusa;

  const WORD32 rescale = (0x0517cc1b << 1);

  if (env == 0) {
    WORD usb_prev = ptr_ps_dec->usb;
    WORD16 *ptr_tmp;
    ptr_ps_dec->usb = usb;

    if ((usb > usb_prev) && usb_prev) {
      WORD i, j, delay, offset1;
      WORD ixheaacd_drc_offset =
          (usb < NUM_OF_QUAD_MIRROR_FILTER_ALL_PASS_CHNLS
               ? usb
               : NUM_OF_QUAD_MIRROR_FILTER_ALL_PASS_CHNLS);

      if (ixheaacd_drc_offset > usb_prev) {
        for (i = 0; i < NUM_SER_AP_LINKS; i++) {
          for (j = 0; j < ptr_ps_dec->delay_sample_ser[i]; j++) {
            ptr_tmp = &ptr_ps_dec->delay_buf_qmf_ser_re_im[j][i][usb_prev * 2];

            memset(ptr_tmp, 0,
                   sizeof(WORD16) * (ixheaacd_drc_offset - usb_prev) * 2);
          }
        }
      }

      offset1 =
          (usb < (NUM_OF_QUAD_MIRROR_FILTER_ALL_PASS_CHNLS + SMALL_DEL_STRT)
               ? usb
               : (NUM_OF_QUAD_MIRROR_FILTER_ALL_PASS_CHNLS + SMALL_DEL_STRT));
      delay = HIGH_DEL;

      if ((offset1 >= ixheaacd_drc_offset) && (offset1 <= SMALL_DEL_STRT)) {
        for (i = 0; i < delay; i++) {
          ptr_tmp =
              &ptr_ps_dec->delay_buf_qmf_ld_re_im[i][ixheaacd_drc_offset * 2];

          memset(ptr_tmp, 0,
                 sizeof(WORD16) * 2 * (offset1 - ixheaacd_drc_offset));
        }
      }

      delay = SMALL_DEL;

      if ((usb >= offset1) && (usb <= 16)) {
        for (i = 0; i < delay; i++) {
          ptr_tmp = &ptr_ps_dec->delay_buf_qmf_sd_re_im[i][offset1 * 2];

          memset(ptr_tmp, 0, sizeof(WORD16) * 2 * (usb - offset1));
        }
      }
    }
  }

  if (ptr_ps_dec->iid_quant) {
    num_iid_steps = NUM_IID_LEVELS_FINE;
    p_scale_factors = sbr_tables_ptr->ps_tables_ptr->scale_factors_fine;
  } else {
    num_iid_steps = NUM_IID_LEVELS;
    p_scale_factors = sbr_tables_ptr->ps_tables_ptr->scale_factors;
  }

  inv_env_len =
      sbr_tables_ptr->env_calc_tables_ptr->sbr_inv_int_table[ixheaac_abs16(
          ixheaac_sub16_sat(ptr_ps_dec->border_position[env + 1],
                             ptr_ps_dec->border_position[env]))];

  p_iid_idx = &ptr_ps_dec->iid_par_table[env][0];

  for (group = 0; group < NO_IID_GROUPS; group++) {
    WORD16 bplusa, bminusa;
    WORD num_iid_idx, num_icc_idx;

    bin = sbr_tables_ptr->ps_tables_ptr->group_to_bin[group];

    num_iid_idx = p_iid_idx[bin];
    num_icc_idx = p_iid_idx[bin + 238];

    c1 = p_scale_factors[(num_iid_steps + num_iid_idx)];
    c2 = p_scale_factors[(num_iid_steps - num_iid_idx)];

    beta = ixheaac_mult32x16in32_shl(
        ixheaac_mult16x16in32_shl(
            sbr_tables_ptr->ps_tables_ptr->alpha_values[num_icc_idx],
            ixheaac_sub16(c1, c2)),
        PSC_SQRT05F);
    alpha = ixheaac_shr32_dir_sat_limit(
        ixheaac_deposit16h_in32(
            sbr_tables_ptr->ps_tables_ptr->alpha_values[num_icc_idx]),
        1);

    bplusa = ixheaac_round16(ixheaac_add32_sat(beta, alpha));
    bminusa = ixheaac_round16(ixheaac_sub32_sat(beta, alpha));

    indexplusa = ixheaac_mult32x16in32(rescale, bplusa);
    indexminusa = ixheaac_mult32x16in32(rescale, bminusa);

    h11 = ixheaac_mult16_shl(ixheaacd_cos512(indexplusa, cos_sin_lookup_tab),
                              c2);
    h12 = ixheaac_mult16_shl(ixheaacd_cos512(indexminusa, cos_sin_lookup_tab),
                              c1);
    h21 = ixheaac_mult16_shl(ixheaacd_sin512(indexplusa, cos_sin_lookup_tab),
                              c2);
    h22 = ixheaac_mult16_shl(ixheaacd_sin512(indexminusa, cos_sin_lookup_tab),
                              c1);

    ptr_ps_dec->delta_h11_h12[2 * group + 0] = ixheaac_mult16_shl(
        inv_env_len,
        ixheaac_sub16(h11, ptr_ps_dec->h11_h12_vec[2 * group + 0]));
    ptr_ps_dec->delta_h11_h12[2 * group + 1] = ixheaac_mult16_shl(
        inv_env_len,
        ixheaac_sub16(h12, ptr_ps_dec->h11_h12_vec[2 * group + 1]));
    ptr_ps_dec->delta_h21_h22[2 * group + 0] = ixheaac_mult16_shl(
        inv_env_len,
        ixheaac_sub16(h21, ptr_ps_dec->h21_h22_vec[2 * group + 0]));
    ptr_ps_dec->delta_h21_h22[2 * group + 1] = ixheaac_mult16_shl(
        inv_env_len,
        ixheaac_sub16(h22, ptr_ps_dec->h21_h22_vec[2 * group + 1]));

    ptr_ps_dec->H11_H12[2 * group + 0] = ptr_ps_dec->h11_h12_vec[2 * group + 0];
    ptr_ps_dec->H11_H12[2 * group + 1] = ptr_ps_dec->h11_h12_vec[2 * group + 1];
    ptr_ps_dec->H21_H22[2 * group + 0] = ptr_ps_dec->h21_h22_vec[2 * group + 0];
    ptr_ps_dec->H21_H22[2 * group + 1] = ptr_ps_dec->h21_h22_vec[2 * group + 1];

    ptr_ps_dec->h11_h12_vec[2 * group + 0] = h11;
    ptr_ps_dec->h11_h12_vec[2 * group + 1] = h12;
    ptr_ps_dec->h21_h22_vec[2 * group + 0] = h21;
    ptr_ps_dec->h21_h22_vec[2 * group + 1] = h22;
  }
}

VOID ixheaacd_apply_rot_dec(ia_ps_dec_struct *ptr_ps_dec, WORD32 *p_qmf_left_re,
                            WORD32 *p_qmf_left_im, WORD32 *p_qmf_right_re,
                            WORD32 *p_qmf_right_im,
                            ia_sbr_tables_struct *sbr_tables_ptr,
                            const WORD16 *ptr_res) {
  WORD group, subband, max_subband, usb, k;
  WORD32 *p_hyb_left_re, *p_hyb_left_re1;
  WORD32 *p_hyb_left_im, *p_hyb_left_im1;
  WORD32 *p_hyb_right_re, *p_hyb_right_re1;
  WORD32 *p_hyb_right_im, *p_hyb_right_im1;
  WORD32 temp_left_real, temp_left_imag;
  WORD32 temp_right_real, temp_right_imag;
  WORD16 hybrid_resol;
  WORD32 tmp_real, tmp_img;
  WORD32 tmp_real1, tmp_img1;
  WORD16 H11_H12[128 * 2] = {0};

  usb = ptr_ps_dec->usb;

  p_hyb_left_re1 = ptr_ps_dec->ptr_hyb_left_re;
  p_hyb_left_im1 = ptr_ps_dec->ptr_hyb_left_im;
  p_hyb_right_re1 = ptr_ps_dec->ptr_hyb_right_re;
  p_hyb_right_im1 = ptr_ps_dec->ptr_hyb_right_im;

  for (group = 0; group < NO_IID_GROUPS; group++) {
    ptr_ps_dec->H11_H12[2 * group + 0] =
        ixheaac_add16(ptr_ps_dec->H11_H12[2 * group + 0],
                       ptr_ps_dec->delta_h11_h12[2 * group + 0]);
    ptr_ps_dec->H11_H12[2 * group + 1] =
        ixheaac_add16(ptr_ps_dec->H11_H12[2 * group + 1],
                       ptr_ps_dec->delta_h11_h12[2 * group + 1]);

    ptr_ps_dec->H21_H22[2 * group + 0] =
        ixheaac_add16(ptr_ps_dec->H21_H22[2 * group + 0],
                       ptr_ps_dec->delta_h21_h22[2 * group + 0]);
    ptr_ps_dec->H21_H22[2 * group + 1] =
        ixheaac_add16(ptr_ps_dec->H21_H22[2 * group + 1],
                       ptr_ps_dec->delta_h21_h22[2 * group + 1]);
  }

  for (subband = 0; subband < SUBQMF_GROUPS; subband++) {
    temp_left_real = ixheaac_add32_sat(
        ixheaac_mult32x16in32(p_hyb_left_re1[subband],
                               ptr_ps_dec->H11_H12[2 * subband + 0]),
        ixheaac_mult32x16in32(p_hyb_right_re1[subband],
                               ptr_ps_dec->H21_H22[2 * subband + 0]));
    temp_left_imag = ixheaac_add32_sat(
        ixheaac_mult32x16in32(p_hyb_left_im1[subband],
                               ptr_ps_dec->H11_H12[2 * subband + 0]),
        ixheaac_mult32x16in32(p_hyb_right_im1[subband],
                               ptr_ps_dec->H21_H22[2 * subband + 0]));
    temp_right_real = ixheaac_add32_sat(
        ixheaac_mult32x16in32(p_hyb_left_re1[subband],
                               ptr_ps_dec->H11_H12[2 * subband + 1]),
        ixheaac_mult32x16in32(p_hyb_right_re1[subband],
                               ptr_ps_dec->H21_H22[2 * subband + 1]));
    temp_right_imag = ixheaac_add32_sat(
        ixheaac_mult32x16in32(p_hyb_left_im1[subband],
                               ptr_ps_dec->H11_H12[2 * subband + 1]),
        ixheaac_mult32x16in32(p_hyb_right_im1[subband],
                               ptr_ps_dec->H21_H22[2 * subband + 1]));
    p_hyb_left_re1[subband] = ixheaac_shl32(temp_left_real, 2);
    p_hyb_left_im1[subband] = ixheaac_shl32(temp_left_imag, 2);
    p_hyb_right_re1[subband] = ixheaac_shl32(temp_right_real, 2);
    p_hyb_right_im1[subband] = ixheaac_shl32(temp_right_imag, 2);
  }

  p_hyb_left_re = p_qmf_left_re;
  p_hyb_left_im = p_qmf_left_im;
  p_hyb_right_re = p_qmf_right_re;
  p_hyb_right_im = p_qmf_right_im;

  {
    WORD32 *h11_h12_src = (WORD32 *)ptr_ps_dec->H11_H12;
    WORD32 *h21_h22_src = (WORD32 *)ptr_ps_dec->H21_H22;
    WORD32 *h11_h12_dst = (WORD32 *)H11_H12;

    for (group = SUBQMF_GROUPS; group < NO_IID_GROUPS; group++) {
      max_subband = ixheaac_min32(
          usb, sbr_tables_ptr->ps_tables_ptr->borders_group[group + 1]);
      for (subband = sbr_tables_ptr->ps_tables_ptr->borders_group[group];
           subband < max_subband; subband++) {
        h11_h12_dst[2 * subband] = h11_h12_src[group];
        h11_h12_dst[2 * subband + 1] = h21_h22_src[group];
      }
    }
  }

  for (subband = 0; subband < NO_QMF_CHANNELS_IN_HYBRID; subband++) {
    tmp_real = *p_hyb_left_re1++;
    tmp_img = *p_hyb_left_im1++;
    tmp_real1 = *p_hyb_right_re1++;
    tmp_img1 = *p_hyb_right_im1++;

    hybrid_resol = ixheaac_min16(*ptr_res++, 6);

    for (k = hybrid_resol - 2; k >= 0; k--) {
      tmp_real = ixheaac_add32_sat(tmp_real, *p_hyb_left_re1++);
      tmp_img = ixheaac_add32_sat(tmp_img, *p_hyb_left_im1++);
      tmp_real1 = ixheaac_add32_sat(tmp_real1, *p_hyb_right_re1++);
      tmp_img1 = ixheaac_add32_sat(tmp_img1, *p_hyb_right_im1++);
    }

    p_hyb_left_re[subband] = tmp_real;
    p_hyb_left_im[subband] = tmp_img;
    p_hyb_right_re[subband] = tmp_real1;
    p_hyb_right_im[subband] = tmp_img1;
  }

  for (; subband < usb; subband++) {
    temp_left_real =
        ixheaac_add32_sat(ixheaac_mult32x16in32(p_hyb_left_re[subband],
                                                  H11_H12[4 * subband + 0]),
                           ixheaac_mult32x16in32(p_hyb_right_re[subband],
                                                  H11_H12[4 * subband + 2]));
    temp_left_imag =
        ixheaac_add32_sat(ixheaac_mult32x16in32(p_hyb_left_im[subband],
                                                  H11_H12[4 * subband + 0]),
                           ixheaac_mult32x16in32(p_hyb_right_im[subband],
                                                  H11_H12[4 * subband + 2]));
    temp_right_real =
        ixheaac_add32_sat(ixheaac_mult32x16in32(p_hyb_left_re[subband],
                                                  H11_H12[4 * subband + 1]),
                           ixheaac_mult32x16in32(p_hyb_right_re[subband],
                                                  H11_H12[4 * subband + 3]));
    temp_right_imag =
        ixheaac_add32_sat(ixheaac_mult32x16in32(p_hyb_left_im[subband],
                                                  H11_H12[4 * subband + 1]),
                           ixheaac_mult32x16in32(p_hyb_right_im[subband],
                                                  H11_H12[4 * subband + 3]));
    p_hyb_left_re[subband] = ixheaac_shl32(temp_left_real, 2);
    p_hyb_left_im[subband] = ixheaac_shl32(temp_left_imag, 2);
    p_hyb_right_re[subband] = ixheaac_shl32(temp_right_real, 2);
    p_hyb_right_im[subband] = ixheaac_shl32(temp_right_imag, 2);
  }
}
