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
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "ixheaac_type_def.h"
#include "ixheaacd_interface.h"

#include "ixheaacd_defines.h"
#include "ixheaacd_aac_rom.h"

#include "ixheaacd_bitbuffer.h"

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

#include "ixheaacd_pulsedata.h"
#include "ixheaacd_pns.h"
#include "ixheaacd_lt_predict.h"
#include "ixheaacd_ec_defines.h"
#include "ixheaacd_ec_struct_def.h"
#include "ixheaacd_main.h"
#include "ixheaacd_channelinfo.h"
#include "ixheaacd_ec.h"
#include "ixheaacd_arith_dec.h"
#include "ixheaacd_windows.h"

#include "ixheaacd_vec_baisc_ops.h"
#include "ixheaac_constants.h"
#include "ixheaacd_function_selector.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops40.h"

#include "ixheaacd_func_def.h"

#include "ixheaacd_windows.h"

extern const WORD32 ixheaacd_pre_post_twid_cos_512[512];
extern const WORD32 ixheaacd_pre_post_twid_sin_512[512];
extern const WORD32 ixheaacd_pre_post_twid_cos_384[384];
extern const WORD32 ixheaacd_pre_post_twid_sin_384[384];
extern const WORD32 ixheaacd_pre_post_twid_cos_64[64];
extern const WORD32 ixheaacd_pre_post_twid_sin_64[64];
extern const WORD32 ixheaacd_pre_post_twid_cos_48[48];
extern const WORD32 ixheaacd_pre_post_twid_sin_48[48];
extern const FLOAT64 ixheaacd_power_10_table[28];
extern const ia_usac_samp_rate_info ixheaacd_samp_rate_info[];

#define ABS(A) ((A) < 0 ? (-A) : (A))

static WORD32 ixheaacd_calc_max_spectralline(WORD32 *p_in_ibuffer, WORD32 n) {
  WORD32 k, shiftp, itemp = 0;
  for (k = 0; k < n; k++) {
    if (ixheaac_abs32_sat(p_in_ibuffer[k]) > itemp)
      itemp = ixheaac_abs32_sat(p_in_ibuffer[k]);
  }

  shiftp = ixheaac_norm32(itemp);

  return (shiftp);
}

static void ixheaacd_normalize(WORD32 *buff, WORD32 shift, WORD len) {
  WORD32 i;

  for (i = 0; i < len; i++) {
    buff[i] = buff[i] << shift;
  }
}

static FLOAT32 ixheaacd_pow10(WORD32 input) {
  FLOAT32 output = 1;
  while (input > 0) {
    output *= 10;
    input--;
  }
  return (output);
}

void ixheaacd_calc_pre_twid_dec(WORD32 *ptr_x, WORD32 *r_ptr, WORD32 *i_ptr,
                                WORD32 nlength, const WORD32 *cos_ptr,
                                const WORD32 *sin_ptr) {
  WORD32 i;
  WORD32 *ptr_y;

  ptr_y = &ptr_x[2 * nlength - 1];

  for (i = 0; i < nlength; i++) {
    *r_ptr++ = ((ixheaac_mult32(ixheaac_negate32_sat(*ptr_x), (*cos_ptr)) -
                 ixheaac_mult32((*ptr_y), (*sin_ptr))));
    *i_ptr++ = ((ixheaac_mult32((*ptr_y), (*cos_ptr++)) -
                 ixheaac_mult32((*ptr_x), (*sin_ptr++))));
    ptr_x += 2;
    ptr_y -= 2;
  }
}

void ixheaacd_calc_post_twid_dec(WORD32 *xptr, WORD32 *r_ptr, WORD32 *i_ptr,
                                 WORD32 nlength, const WORD32 *cos_ptr,
                                 const WORD32 *sin_ptr

                                 ) {
  WORD32 i;
  WORD32 *yptr;

  yptr = &xptr[2 * nlength - 1];

  for (i = 0; i < nlength; i++) {
    *xptr = (-(ixheaac_mult32((r_ptr[i]), (*cos_ptr)) -
               ixheaac_mult32((i_ptr[i]), (*sin_ptr))));
    *yptr = (-(ixheaac_mult32((i_ptr[i]), (*cos_ptr++)) +
               ixheaac_mult32((r_ptr[i]), (*sin_ptr++))));
    xptr += 2;
    yptr -= 2;
  }
}

static VOID ixheaacd_fft_based_imdct(WORD32 *data, WORD32 npoints, WORD32 *preshift,
                                     WORD32 *tmp_data) {
  WORD32 *data_r;
  WORD32 *data_i;
  WORD32 nlength = npoints >> 1;
  const WORD32 *cos_ptr;
  const WORD32 *sin_ptr;

  data_r = tmp_data;
  data_i = tmp_data + 512;

  if (nlength == 512) {
    cos_ptr = ixheaacd_pre_post_twid_cos_512;
    sin_ptr = ixheaacd_pre_post_twid_sin_512;
  } else if (nlength == 384) {
    cos_ptr = ixheaacd_pre_post_twid_cos_384;
    sin_ptr = ixheaacd_pre_post_twid_sin_384;
  } else if (nlength == 64) {
    cos_ptr = ixheaacd_pre_post_twid_cos_64;
    sin_ptr = ixheaacd_pre_post_twid_sin_64;
  } else if (nlength == 48) {
    cos_ptr = ixheaacd_pre_post_twid_cos_48;
    sin_ptr = ixheaacd_pre_post_twid_sin_48;
  } else {
    cos_ptr = ixheaacd_pre_post_twid_cos_48;
    sin_ptr = ixheaacd_pre_post_twid_sin_48;
  }

  (*ixheaacd_calc_pre_twid)(data, data_r, data_i, nlength, cos_ptr, sin_ptr);
  ixheaacd_complex_fft(data_r, data_i, nlength, 1, preshift);

  (*ixheaacd_calc_post_twid)(data, data_r, data_i, nlength, cos_ptr, sin_ptr);
  return;
}

#define N_LONG_LEN_MAX 1024

VOID ixheaacd_acelp_imdct(WORD32 *imdct_in, WORD32 npoints, WORD8 *qshift,
                            WORD32 *tmp_data) {
  WORD32 preshift = 0;
  WORD32 i;
  WORD32 k = (npoints / 2);

  while (((k & 1) == 0) & (k != 1)) {
    k = k >> 1;
    preshift++;
  }

  if ((k != 1)) {
    for (i = 0; i < (npoints / 2); i++) {
      imdct_in[i] = (imdct_in[i] / 3) << 1;
    }
    preshift++;
  }

  ixheaacd_fft_based_imdct(imdct_in, npoints / 2, &preshift, tmp_data);
  preshift += 2;
  *qshift -= preshift;
  return;
}

IA_ERRORCODE ixheaacd_cal_fac_data(ia_usac_data_struct *usac_data, WORD32 i_ch,
                                   WORD32 n_long, WORD32 lfac,
                                   WORD32 *fac_idata, WORD8 *q_fac) {
  WORD32 gain_fac, scale, k, *i_aq, itemp = 0, *izir;
  WORD32 int_aq[ORDER + 1] = {0};
  WORD32 intzir[2 * LEN_FRAME] = {0};
  WORD32 x_in[FAC_LENGTH] = {0};
  FLOAT32 gain, ztemp, ftemp, pow10, rem10;
  FLOAT32 qfac1;
  WORD8 qshift1 = 0;
  WORD8 qshift2 = 0;
  WORD8 qshift3 = 0;
  WORD32 preshift = 0;

  FLOAT32 *last_lpc = usac_data->lpc_prev[i_ch];
  FLOAT32 *acelp_in = usac_data->acelp_in[i_ch];
  WORD32 *fac_data = usac_data->fac_data[i_ch];
  WORD32 *ptr_scratch = &usac_data->scratch_buffer[0];

  WORD32 quo = fac_data[0] / 28;
  WORD32 rem = fac_data[0] % 28;
  pow10 = ixheaacd_pow10(quo);
  rem10 = (FLOAT32)ixheaacd_power_10_table[rem];

  gain = pow10 * rem10;
  scale = (WORD32)(ixheaac_norm32((WORD32)((ABS(gain) + 1))));
  gain_fac = (WORD32)(gain * (FLOAT32)((WORD64)1 << scale));
  scale += 4;
  qfac1 = 1.0f / (gain);

  if (acelp_in != NULL) {
    izir = intzir;
    ftemp = 0.0;
    for (k = 0; k < n_long / 4; k++) {
      ztemp = acelp_in[k] * (qfac1);
      if (ABS(ztemp) > ftemp) ftemp = ABS(ztemp);
    }

    itemp = (WORD32)(ftemp);
    qshift3 = ixheaac_norm32(itemp);

    for (k = 0; k < n_long / 4; k++) {
      izir[k] =
          (WORD32)((acelp_in[k] * (qfac1)) * (FLOAT32)((WORD64)1 << qshift3));
    }
  } else
    izir = NULL;

  if (last_lpc != NULL) {
    ftemp = 0.0;
    i_aq = int_aq;
    for (k = 0; k < ORDER + 1; k++) {
      if (ABS(last_lpc[k]) > ftemp) ftemp = ABS(last_lpc[k]);
    }

    itemp = (WORD32)(ftemp);
    qshift2 = ixheaac_norm32(itemp);

    for (k = 0; k < ORDER + 1; k++) {
      i_aq[k] = (WORD32)(last_lpc[k] * (FLOAT32)((WORD64)1 << qshift2));
    }
  } else
    i_aq = NULL;

  for (k = 0; k < lfac; k++) {
    if (ixheaac_abs32_sat(fac_data[k + 1]) > itemp)
      itemp = ixheaac_abs32_sat(fac_data[k + 1]);
  }

  qshift1 = ixheaac_norm32(itemp);

  for (k = 0; k < lfac; k++) {
    fac_data[k + 1] =
        (WORD32)(fac_data[k + 1] * (FLOAT32)((WORD64)1 << qshift1));
  }

  for (k = 0; k < lfac / 2; k++) {
    x_in[k] = fac_data[2 * k + 1];
    x_in[lfac / 2 + k] = fac_data[lfac - 2 * k];
  }

  if (FAC_LENGTH < lfac) {
    if (usac_data->ec_flag == 0)
      return -1;
    else
      lfac = FAC_LENGTH;
  }

  if ((n_long / 8) < lfac) {
    if (usac_data->ec_flag == 0)
      return -1;
    else
      lfac = (n_long / 8);
  }

  if ((n_long / 8 + 1) > (2 * LEN_FRAME - lfac - 1)) {
    if (usac_data->ec_flag == 0)
      return -1;
    else
      lfac = (2 * LEN_FRAME) - (n_long / 8);
  }

  if (lfac & (lfac - 1)) {
    if ((lfac != 48) && (lfac != 96) && (lfac != 192) && (lfac != 384) && (lfac != 768)) {
      if (usac_data->ec_flag == 0)
        return -1;
      else
        lfac = 48;
    }
  }

  ixheaacd_fr_alias_cnx_fix(x_in, n_long / 4, lfac, i_aq, izir, fac_idata + 16, &qshift1, qshift2,
                            qshift3, &preshift, ptr_scratch);

  preshift += 4;
  *q_fac = (qshift1 - preshift);

  if (acelp_in != NULL) {
    for (k = 0; k < 2 * lfac; k++) {
      fac_idata[k] =
          ixheaac_mul32_sh(fac_idata[k + 16], gain_fac, (WORD8)(scale));
    }
  }
  return IA_NO_ERROR;
}

static IA_ERRORCODE ixheaacd_fd_imdct_short(ia_usac_data_struct *usac_data,
                                            WORD32 i_ch, WORD32 *fac_data_out,
                                            offset_lengths *ixheaacd_drc_offset,
                                            WORD8 fac_q) {
  FLOAT32 qfac;
  WORD32 overlap_data_buf[2 * N_LONG_LEN_MAX] = {0};
  WORD32 *window_short, k, *window_short_prev_ptr;
  WORD32 *overlap_data, *fp;

  WORD32 *p_overlap_ibuffer = usac_data->overlap_data_ptr[i_ch];
  WORD32 *p_in_ibuffer = usac_data->coef_fix[i_ch];
  FLOAT32 *p_out_buffer = usac_data->time_sample_vector[i_ch];
  WORD32 *p_out_ibuffer = usac_data->output_data_ptr[i_ch];
  WORD32 *scratch_mem = usac_data->scratch_buffer;
  WORD32 td_frame_prev = usac_data->td_frame_prev[i_ch];
  WORD32 fac_apply = usac_data->fac_data_present[i_ch];
  WORD8 shiftp, input_q, output_q, shift_olap = 14;
  WORD32 max_shift;

  WORD32 window_select = usac_data->window_shape[i_ch];
  WORD32 window_select_prev = usac_data->window_shape_prev[i_ch];
  ia_usac_lpd_decoder_handle st = usac_data->str_tddec[i_ch];
  WORD32 err_code = 0;

  if (usac_data->ec_flag) {
    td_frame_prev = usac_data->td_frame_prev_ec[i_ch];
  } else {
    if (ixheaacd_drc_offset->n_short & (ixheaacd_drc_offset->n_short - 1)) {
      if ((ixheaacd_drc_offset->n_short != 48) && (ixheaacd_drc_offset->n_short != 96) &&
          (ixheaacd_drc_offset->n_short != 192) && (ixheaacd_drc_offset->n_short != 384) &&
          (ixheaacd_drc_offset->n_short != 768)) {
        return -1;
      }
    }
  }

  max_shift = ixheaacd_calc_max_spectralline(p_in_ibuffer, ixheaacd_drc_offset->n_long);
  ixheaacd_normalize(p_in_ibuffer, max_shift, ixheaacd_drc_offset->n_long);
  shiftp = max_shift + 6;
  input_q = shiftp;

  memcpy(overlap_data_buf, p_overlap_ibuffer,
         sizeof(WORD32) * ixheaacd_drc_offset->n_long);
  overlap_data = overlap_data_buf;

  fp = overlap_data + ixheaacd_drc_offset->n_flat_ls;

  for (k = 0; k < 8; k++) {
    shiftp = input_q;
    ixheaacd_acelp_imdct(
        p_in_ibuffer + (k * ixheaacd_drc_offset->n_short),
        2 * ixheaacd_drc_offset->n_short, &shiftp, scratch_mem);
  }

  max_shift =
      ixheaacd_calc_max_spectralline(p_in_ibuffer, ixheaacd_drc_offset->n_long);
  ixheaacd_normalize(p_in_ibuffer, max_shift - 1, ixheaacd_drc_offset->n_long);
  shiftp += max_shift - 1;

  if ((shiftp - shift_olap) > 31) {
    shiftp = 31 + shift_olap;
  }
  err_code = ixheaacd_calc_window(&window_short, ixheaacd_drc_offset->n_short,
                                  window_select, usac_data->ec_flag);
  if (err_code == -1) return err_code;
  err_code = ixheaacd_calc_window(&window_short_prev_ptr, ixheaacd_drc_offset->n_trans_ls,
                                  window_select_prev, usac_data->ec_flag);
  if (err_code == -1) return err_code;

  if (fac_apply)
    ixheaacd_windowing_short1(p_in_ibuffer + ixheaacd_drc_offset->n_short / 2,
                              window_short_prev_ptr, fp, ixheaacd_drc_offset,
                              shiftp, shift_olap);

  else
    ixheaacd_windowing_short2(p_in_ibuffer + ixheaacd_drc_offset->n_short / 2,
                              window_short_prev_ptr, fp, ixheaacd_drc_offset,
                              shiftp, shift_olap);

  output_q = ixheaacd_windowing_short3(
      p_in_ibuffer, window_short + ixheaacd_drc_offset->n_short - 1,
      fp + ixheaacd_drc_offset->n_short, ixheaacd_drc_offset->n_short, shiftp,
      shift_olap);
  p_in_ibuffer += ixheaacd_drc_offset->n_short;
  fp += ixheaacd_drc_offset->n_short;
  window_short_prev_ptr = window_short;

  for (k = 1; k < 7; k++) {
    output_q = ixheaacd_windowing_short4(
        p_in_ibuffer, window_short_prev_ptr, fp,
        window_short_prev_ptr + ixheaacd_drc_offset->n_short - 1,
        ixheaacd_drc_offset->n_short, 1, shiftp, shift_olap, output_q);
    p_in_ibuffer += ixheaacd_drc_offset->n_short;
    fp += ixheaacd_drc_offset->n_short;
    window_short_prev_ptr = window_short;
  }

  output_q = ixheaacd_windowing_short4(
      p_in_ibuffer, window_short_prev_ptr, fp,
      window_short_prev_ptr + ixheaacd_drc_offset->n_short - 1,
      ixheaacd_drc_offset->n_short, 0, shiftp, shift_olap, output_q);
  p_in_ibuffer += ixheaacd_drc_offset->n_short;
  fp += ixheaacd_drc_offset->n_short;

  if (fac_apply) {
    ixheaacd_combine_fac(overlap_data + ixheaacd_drc_offset->n_flat_ls +
                             ixheaacd_drc_offset->lfac,
                         fac_data_out,
                         overlap_data + ixheaacd_drc_offset->n_flat_ls +
                             ixheaacd_drc_offset->lfac,
                         2 * ixheaacd_drc_offset->lfac, output_q, fac_q);
  }
  memset(overlap_data + 2 * ixheaacd_drc_offset->n_long -
             ixheaacd_drc_offset->n_flat_ls,
         0, sizeof(WORD32) * ixheaacd_drc_offset->n_flat_ls);
  ixheaacd_scale_down(overlap_data, overlap_data,
                      ixheaacd_drc_offset->n_flat_ls, shift_olap, output_q);

  ixheaacd_scale_down(p_overlap_ibuffer,
                      overlap_data + ixheaacd_drc_offset->n_long,
                      ixheaacd_drc_offset->n_long, output_q, shift_olap);
  ixheaacd_scale_down(p_out_ibuffer, overlap_data, ixheaacd_drc_offset->n_long,
                      output_q, 15);

  if (td_frame_prev) {
    qfac = 1.0f / (FLOAT32)(1 << 15);

    for (k = 0; k < ixheaacd_drc_offset->n_long; k++) {
      p_out_buffer[k] = ((FLOAT32)p_out_ibuffer[k]) * qfac;
    }
    err_code = ixheaacd_lpd_bpf_fix(usac_data, 1, p_out_buffer, st);
    if (err_code != 0) return err_code;

    for (k = 0; k < ixheaacd_drc_offset->n_long; k++) {
      p_out_ibuffer[k] = (WORD32)(p_out_buffer[k] * (1 << 15));
    }
  }

  return 0;
}

static IA_ERRORCODE ixheaacd_fd_imdct_long(ia_usac_data_struct *usac_data,
                                           WORD32 i_ch, WORD32 *fac_idata,
                                           offset_lengths *ixheaacd_drc_offset,
                                           WORD8 fac_q) {
  FLOAT32 qfac;
  WORD32 *window_long_prev = NULL, k, i, *window_short_prev_ptr = NULL;

  WORD32 *p_in_ibuffer = usac_data->coef_fix[i_ch];
  WORD32 *p_overlap_ibuffer = usac_data->overlap_data_ptr[i_ch];
  WORD32 *p_out_ibuffer = usac_data->output_data_ptr[i_ch];
  FLOAT32 *p_out_buffer = usac_data->time_sample_vector[i_ch];
  WORD32 *scratch_mem = usac_data->scratch_buffer;
  WORD32 n_long = usac_data->ccfl;
  WORD32 td_frame_prev = usac_data->td_frame_prev[i_ch];
  WORD32 fac_apply = usac_data->fac_data_present[i_ch];
  WORD8 shiftp, output_q = 0, shift_olap = 14;
  WORD32 max_shift;

  WORD32 window_sequence = usac_data->window_sequence[i_ch];
  WORD32 window_select_prev = usac_data->window_shape_prev[i_ch];
  ia_usac_lpd_decoder_handle st = usac_data->str_tddec[i_ch];

  WORD32 err_code = 0;
  if (usac_data->ec_flag) {
    td_frame_prev = usac_data->td_frame_prev_ec[i_ch];
  } else {
    if (ixheaacd_drc_offset->n_long & (ixheaacd_drc_offset->n_long - 1)) {
      if ((ixheaacd_drc_offset->n_long != 48) && (ixheaacd_drc_offset->n_long != 96) &&
          (ixheaacd_drc_offset->n_long != 192) && (ixheaacd_drc_offset->n_long != 384) &&
          (ixheaacd_drc_offset->n_long != 768)) {
        return -1;
      }
    }
  }

  max_shift =
      ixheaacd_calc_max_spectralline(p_in_ibuffer, ixheaacd_drc_offset->n_long);
  ixheaacd_normalize(p_in_ibuffer, max_shift, ixheaacd_drc_offset->n_long);
  shiftp = max_shift + 6;

  ixheaacd_acelp_imdct(p_in_ibuffer, 2 * ixheaacd_drc_offset->n_long,
                                  &shiftp, scratch_mem);

  max_shift =
      ixheaacd_calc_max_spectralline(p_in_ibuffer, ixheaacd_drc_offset->n_long);
  ixheaacd_normalize(p_in_ibuffer, max_shift - 1, ixheaacd_drc_offset->n_long);
  shiftp += max_shift - 1;
  if ((shiftp - shift_olap) > 31) {
    shiftp = 31 + shift_olap;
  }

  switch (window_sequence) {
    case ONLY_LONG_SEQUENCE:
    case LONG_START_SEQUENCE:
      err_code = ixheaacd_calc_window(
          &window_long_prev, ixheaacd_drc_offset->n_long, window_select_prev, usac_data->ec_flag);
      if (err_code == -1) return err_code;
      output_q = ixheaacd_windowing_long1(
          p_in_ibuffer + n_long / 2, p_overlap_ibuffer, window_long_prev,
          window_long_prev + ixheaacd_drc_offset->n_long - 1, p_out_ibuffer,
          ixheaacd_drc_offset->n_long, shiftp, shift_olap);
      break;

    case STOP_START_SEQUENCE:
    case LONG_STOP_SEQUENCE:
      err_code = ixheaacd_calc_window(&window_short_prev_ptr,
                                      ixheaacd_drc_offset->n_trans_ls,
                                      window_select_prev, usac_data->ec_flag);
      if (err_code == -1) return err_code;
      if (fac_apply) {
        output_q = ixheaacd_windowing_long2(
            p_in_ibuffer + n_long / 2, window_short_prev_ptr, fac_idata,
            p_overlap_ibuffer, p_out_ibuffer, ixheaacd_drc_offset, shiftp,
            shift_olap, fac_q);
      } else {
        output_q = ixheaacd_windowing_long3(
            p_in_ibuffer + n_long / 2, window_short_prev_ptr, p_overlap_ibuffer,
            p_out_ibuffer,
            window_short_prev_ptr + ixheaacd_drc_offset->n_trans_ls - 1,
            ixheaacd_drc_offset, shiftp, shift_olap);
      }
      break;
  }

  for (i = 0; i < ixheaacd_drc_offset->n_long / 2; i++) {
    if (shiftp > shift_olap) {
      p_overlap_ibuffer[ixheaacd_drc_offset->n_long / 2 + i] =
        ixheaac_negate32_sat(p_in_ibuffer[i]) >> (shiftp - shift_olap);
      p_overlap_ibuffer[ixheaacd_drc_offset->n_long / 2 - i - 1] =
        ixheaac_negate32_sat(p_in_ibuffer[i]) >> (shiftp - shift_olap);
    }
    else {
      p_overlap_ibuffer[ixheaacd_drc_offset->n_long / 2 + i] =
        ixheaac_negate32_sat(p_in_ibuffer[i]) >> (shift_olap - shiftp);
      p_overlap_ibuffer[ixheaacd_drc_offset->n_long / 2 - i - 1] =
        ixheaac_negate32_sat(p_in_ibuffer[i]) >> (shift_olap - shiftp);
    }
  }

  ixheaacd_scale_down_adj(p_out_ibuffer, p_out_ibuffer,
                          ixheaacd_drc_offset->n_long, output_q, 15);

  if (td_frame_prev) {
    qfac = 1.0f / (FLOAT32)(1 << 15);

    for (k = 0; k < ixheaacd_drc_offset->n_long; k++) {
      p_out_buffer[k] = ((FLOAT32)p_out_ibuffer[k]) * qfac;
    }
    err_code = ixheaacd_lpd_bpf_fix(usac_data, 0, p_out_buffer, st);
    if (err_code != 0) return err_code;

    for (k = 0; k < ixheaacd_drc_offset->n_long; k++) {
      p_out_ibuffer[k] = (WORD32)(p_out_buffer[k] * (1 << 15));
    }
  }

  return 0;
}

WORD32 ixheaacd_fd_frm_dec(ia_usac_data_struct *usac_data, WORD32 i_ch) {
  WORD32 fac_idata[2 * FAC_LENGTH + 16];
  offset_lengths ixheaacd_drc_offset;
  WORD8 fac_q = 0;
  IA_ERRORCODE err = IA_NO_ERROR;
  WORD32 td_frame_prev, fac_apply, window_sequence;
  if (usac_data->ec_flag) {
    usac_data->str_error_concealment[i_ch].pstr_ec_scratch =
        (ia_ec_scratch_str *)&usac_data->str_error_concealment[i_ch].str_ec_scratch;
    usac_data->core_mode = 0;
    ixheaacd_usac_apply_ec(usac_data, &ixheaacd_samp_rate_info[0], i_ch);
  }
  if (usac_data->ec_flag) {
    td_frame_prev = usac_data->td_frame_prev_ec[i_ch];
  } else {
    td_frame_prev = usac_data->td_frame_prev[i_ch];
  }
  fac_apply = usac_data->fac_data_present[i_ch];
  window_sequence = usac_data->window_sequence[i_ch];
  ixheaacd_drc_offset.n_long = usac_data->ccfl;
  ixheaacd_drc_offset.n_short = ixheaacd_drc_offset.n_long >> 3;

  memset(fac_idata, 0, sizeof(fac_idata));

  if (td_frame_prev) {
    if (window_sequence == EIGHT_SHORT_SEQUENCE) {
      ixheaacd_drc_offset.lfac = ixheaacd_drc_offset.n_long >> 4;
    } else {
      ixheaacd_drc_offset.lfac = ixheaacd_drc_offset.n_long >> 3;
    }
    ixheaacd_drc_offset.n_flat_ls =
        (ixheaacd_drc_offset.n_long - (ixheaacd_drc_offset.lfac) * 2) >> 1;

    ixheaacd_drc_offset.n_trans_ls = (ixheaacd_drc_offset.lfac) << 1;
  } else {
    ixheaacd_drc_offset.lfac = FAC_LENGTH;
    ixheaacd_drc_offset.n_flat_ls =
        (ixheaacd_drc_offset.n_long - ixheaacd_drc_offset.n_short) >> 1;
    ixheaacd_drc_offset.n_trans_ls = ixheaacd_drc_offset.n_short;
  }

  if (fac_apply && usac_data->frame_ok == 1) {
    err = ixheaacd_cal_fac_data(usac_data, i_ch, ixheaacd_drc_offset.n_long,
                                ixheaacd_drc_offset.lfac, fac_idata, &fac_q);
    if (err) return err;
  }

  if (window_sequence != EIGHT_SHORT_SEQUENCE) {
    err = ixheaacd_fd_imdct_long(usac_data, i_ch, fac_idata,
                                 &ixheaacd_drc_offset, fac_q);
    if (err) return err;
  } else {
    err = ixheaacd_fd_imdct_short(usac_data, i_ch, fac_idata,
                                  &ixheaacd_drc_offset, fac_q);
    if (err) return err;
  }

  return err;
}
