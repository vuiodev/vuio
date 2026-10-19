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

#include <string.h>

#include "ixheaac_type_def.h"
#include "ixheaac_constants.h"
#include "ixheaace_psy_const.h"
#include "ixheaace_tns.h"
#include "ixheaace_tns_params.h"
#include "ixheaace_rom.h"
#include "ixheaace_common_rom.h"
#include "ixheaace_bitbuffer.h"
#include "ixheaace_aac_constants.h"
#include "ixheaace_fft.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaac_basic_ops.h"
#include "iusace_basic_ops_flt.h"

static VOID ia_enhaacplus_enc_shift_mdct_delay_buffer(FLOAT32 *ptr_mdct_delay_buffer,
                                                      const FLOAT32 *ptr_time_signal,
                                                      WORD32 ch_increment,
                                                      WORD32 long_frame_len) {
  WORD32 i;
  FLOAT32 *ptr_mdct_buff = ptr_mdct_delay_buffer;
  if (ch_increment == 2) {
    const FLOAT32 *ptr_input = ptr_time_signal;
    FLOAT32 temp1, temp2, temp3, temp4;
    temp1 = *ptr_input++;
    ptr_input++;
    temp2 = *ptr_input++;
    ptr_input++;
    temp3 = *ptr_input++;
    ptr_input++;
    for (i = ((long_frame_len >> 2) - 2); i >= 0; i--) {
      *ptr_mdct_buff++ = temp1;
      temp4 = *ptr_input++;
      ptr_input++;

      *ptr_mdct_buff++ = temp2;
      *ptr_mdct_buff++ = temp3;
      *ptr_mdct_buff++ = temp4;

      temp1 = *ptr_input++;
      ptr_input++;
      temp2 = *ptr_input++;
      ptr_input++;
      temp3 = *ptr_input++;
      ptr_input++;
    }
    *ptr_mdct_buff++ = temp1;
    temp4 = *ptr_input;
    *ptr_mdct_buff++ = temp2;
    *ptr_mdct_buff++ = temp3;
    *ptr_mdct_buff++ = temp4;
  } else {
    for (i = 0; i < long_frame_len; i += 2) {
      *ptr_mdct_buff++ = ptr_time_signal[i * ch_increment];
      *ptr_mdct_buff++ = ptr_time_signal[(i + 1) * ch_increment];
    }
  }
}

static VOID ia_eaacp_enc_inverse_transform_512(FLOAT32 *ptr_data, FLOAT32 *ptr_win_buf,
                                               const FLOAT32 *ptr_cos_sin_tbl,
                                               WORD8 *ptr_scratch) {
  WORD32 n = FRAME_LEN_512;
  WORD32 n_by_2 = n >> 1;

  ixheaace_scratch_mem *pstr_scratch = (ixheaace_scratch_mem *)ptr_scratch;

  ia_eaacp_enc_pre_twiddle_aac(ptr_win_buf, ptr_data, n, ptr_cos_sin_tbl);

  ia_enhaacplus_enc_complex_fft(ptr_win_buf, n_by_2, pstr_scratch);

  ia_enhaacplus_enc_post_twiddle(ptr_data, ptr_win_buf, ptr_cos_sin_tbl, n);
}

static VOID ixheaace_pre_mdct(FLOAT32 *ptr_x, WORD32 m, const FLOAT32 *ptr_sine_window) {
  WORD32 i;
  FLOAT32 wre, wim, re1, re2, im1, im2;

  for (i = 0; i < m / 4; i++) {
    re1 = ptr_x[2 * i];
    im2 = ptr_x[2 * i + 1];
    re2 = ptr_x[m - 2 - 2 * i];
    im1 = ptr_x[m - 1 - 2 * i];

    wim = ptr_sine_window[i * 2];
    wre = ptr_sine_window[m - 1 - 2 * i];

    ptr_x[2 * i] = im1 * wim + re1 * wre;

    ptr_x[2 * i + 1] = im1 * wre - re1 * wim;

    wim = ptr_sine_window[m - 2 - 2 * i];
    wre = ptr_sine_window[2 * i + 1];

    ptr_x[m - 2 - 2 * i] = im2 * wim + re2 * wre;

    ptr_x[m - 1 - 2 * i] = im2 * wre - re2 * wim;
  }
}

static VOID ia_enhaacplus_enc_tranform_mac4(FLOAT32 *ptr_op, const FLOAT32 *ptr_win,
                                            FLOAT32 *ptr_buf1, FLOAT32 *ptr_buf2,
                                            FLOAT32 *ptr_buf3, FLOAT32 *ptr_buf4, UWORD32 len,
                                            WORD32 increment) {
  WORD32 i;

  if (increment > 0) {
    for (i = len >> 2; i > 0; i--) {
      *ptr_op = ((ptr_win[0] * (*ptr_buf1++)) + (ptr_win[1] * (*ptr_buf2++)));
      *ptr_op = (*ptr_op + (ptr_win[2] * (*ptr_buf3--)));
      *ptr_op = (*ptr_op + (ptr_win[3] * (*ptr_buf4--)));
      ptr_op++;

      *ptr_op = ((ptr_win[4] * (*ptr_buf1++)) + (ptr_win[5] * (*ptr_buf2++)));
      *ptr_op = (*ptr_op + (ptr_win[6] * (*ptr_buf3--)));
      *ptr_op = (*ptr_op + (ptr_win[7] * (*ptr_buf4--)));
      ptr_op++;

      *ptr_op = ((ptr_win[8] * (*ptr_buf1++)) + (ptr_win[9] * (*ptr_buf2++)));
      *ptr_op = (*ptr_op + (ptr_win[10] * (*ptr_buf3--)));
      *ptr_op = (*ptr_op + (ptr_win[11] * (*ptr_buf4--)));
      ptr_op++;

      *ptr_op = ((ptr_win[12] * (*ptr_buf1++)) + (ptr_win[13] * (*ptr_buf2++)));
      *ptr_op = (*ptr_op + (ptr_win[14] * (*ptr_buf3--)));
      *ptr_op = (*ptr_op + (ptr_win[15] * (*ptr_buf4--)));
      ptr_op++;
      ptr_win += 16;
    }
  } else {
    for (i = len >> 2; i > 0; i--) {
      *ptr_op = ((ptr_win[0] * (*ptr_buf1++)) + (ptr_win[1] * (*ptr_buf2++)));
      *ptr_op = (*ptr_op + (ptr_win[2] * (*ptr_buf3--)));
      *ptr_op = (*ptr_op + (ptr_win[3] * (*ptr_buf4--)));
      ptr_op--;

      *ptr_op = ((ptr_win[4] * (*ptr_buf1++)) + (ptr_win[5] * (*ptr_buf2++)));
      *ptr_op = (*ptr_op + (ptr_win[6] * (*ptr_buf3--)));
      *ptr_op = (*ptr_op + (ptr_win[7] * (*ptr_buf4--)));
      ptr_op--;

      *ptr_op = ((ptr_win[8] * (*ptr_buf1++)) + (ptr_win[9] * (*ptr_buf2++)));
      *ptr_op = (*ptr_op + (ptr_win[10] * (*ptr_buf3--)));
      *ptr_op = (*ptr_op + (ptr_win[11] * (*ptr_buf4--)));
      ptr_op--;

      *ptr_op = ((ptr_win[12] * (*ptr_buf1++)) + (ptr_win[13] * (*ptr_buf2++)));
      *ptr_op = (*ptr_op + (ptr_win[14] * (*ptr_buf3--)));
      *ptr_op = (*ptr_op + (ptr_win[15] * (*ptr_buf4--)));
      ptr_op--;
      ptr_win += 16;
    }
  }
}

static VOID ia_enhaacplus_enc_tranform_mac3(FLOAT32 *ptr_op, const FLOAT32 *ptr_win,
                                            FLOAT32 *ptr_buf1, FLOAT32 *ptr_buf2,
                                            FLOAT32 *ptr_buf3, UWORD32 len, WORD32 increment) {
  WORD32 i;

  if (increment > 0) {
    for (i = len >> 2; i > 0; i--) {
      *ptr_op = ((ptr_win[0] * (*ptr_buf1++)) + (ptr_win[1] * (*ptr_buf2--)));
      *ptr_op = (*ptr_op + (ptr_win[2] * (*ptr_buf3--)));
      ptr_op++;

      *ptr_op = ((ptr_win[3] * (*ptr_buf1++)) + (ptr_win[4] * (*ptr_buf2--)));
      *ptr_op = (*ptr_op + (ptr_win[5] * (*ptr_buf3--)));
      ptr_op++;

      *ptr_op = ((ptr_win[6] * (*ptr_buf1++)) + (ptr_win[7] * (*ptr_buf2--)));
      *ptr_op = (*ptr_op + (ptr_win[8] * (*ptr_buf3--)));
      ptr_op++;

      *ptr_op = ((ptr_win[9] * (*ptr_buf1++)) + (ptr_win[10] * (*ptr_buf2--)));
      *ptr_op = (*ptr_op + (ptr_win[11] * (*ptr_buf3--)));
      ptr_op++;
      ptr_win += 12;
    }
  } else {
    for (i = len >> 2; i > 0; i--) {
      *ptr_op = ((ptr_win[0] * (*ptr_buf1++)) + (ptr_win[1] * (*ptr_buf2--)));
      *ptr_op = (*ptr_op + (ptr_win[2] * (*ptr_buf3--)));
      ptr_op--;

      *ptr_op = ((ptr_win[3] * (*ptr_buf1++)) + (ptr_win[4] * (*ptr_buf2--)));
      *ptr_op = (*ptr_op + (ptr_win[5] * (*ptr_buf3--)));
      ptr_op--;

      *ptr_op = ((ptr_win[6] * (*ptr_buf1++)) + (ptr_win[7] * (*ptr_buf2--)));
      *ptr_op = (*ptr_op + (ptr_win[8] * (*ptr_buf3--)));
      ptr_op--;

      *ptr_op = ((ptr_win[9] * (*ptr_buf1++)) + (ptr_win[10] * (*ptr_buf2--)));
      *ptr_op = (*ptr_op + (ptr_win[11] * (*ptr_buf3--)));
      ptr_op--;
      ptr_win += 12;
    }
  }
}

VOID ia_enhaacplus_enc_transform_real(FLOAT32 *ptr_mdct_delay_buffer,
                                      const FLOAT32 *ptr_time_signal, WORD32 ch_increment,
                                      FLOAT32 *ptr_real_out, ixheaace_mdct_tables *pstr_mdct_tab,
                                      FLOAT32 *ptr_shared_buffer1, WORD8 *ptr_shared_buffer5,
                                      WORD32 long_frame_len) {
  WORD32 n, n1;
  FLOAT32 *ptr_windowed_buf = ptr_shared_buffer1;
  const FLOAT32 *ptr_ws1;
  WORD32 i, len = long_frame_len;
  FLOAT32 *ptr_real_in;
  FLOAT32 *ptr_data1, *ptr_data2, *ptr_data3, *ptr_data4;
  FLOAT32 *ptr_op1;

  ptr_real_in = ptr_mdct_delay_buffer;

  n = long_frame_len << 1;
  n1 = long_frame_len >> 1;

  ptr_ws1 =
      (long_frame_len == FRAME_LEN_512) ? pstr_mdct_tab->win_512_ld : pstr_mdct_tab->win_480_ld;

  ptr_op1 = ptr_real_out;
  ptr_data1 = &ptr_real_in[n1];
  ptr_data2 = &ptr_real_in[n + n1];
  ptr_data3 = &ptr_real_in[n1 - 1];
  ptr_data4 = &ptr_real_in[n + n1 - 1];

  ia_enhaacplus_enc_tranform_mac4(ptr_op1, ptr_ws1, ptr_data1, ptr_data2, ptr_data3, ptr_data4,
                                  n1, 1);
  ptr_ws1 += ((SIZE_T)n1 << 2);

  for (i = 0; i < long_frame_len << 1; i++) {
    ptr_mdct_delay_buffer[i] = ptr_mdct_delay_buffer[long_frame_len + i];
  }
  ia_enhaacplus_enc_shift_mdct_delay_buffer(&ptr_mdct_delay_buffer[2 * long_frame_len],
                                            ptr_time_signal, ch_increment, long_frame_len);

  ptr_op1 = &ptr_real_out[long_frame_len - 1];
  ptr_data1 = &ptr_real_in[n + len - n1];
  ptr_data2 = &ptr_real_in[len - n1];
  ptr_data3 = &ptr_real_in[len - n1 - 1];
  ptr_data4 = &ptr_real_in[n + len - n1 - 1];

  ia_enhaacplus_enc_tranform_mac4(ptr_op1, ptr_ws1, ptr_data1, ptr_data2, ptr_data3, ptr_data4,
                                  (n1 >> 1), -1);
  ptr_op1 -= (n1 >> 1);
  ptr_ws1 += ((SIZE_T)n1 << 1);
  ptr_data2 += (n1 >> 1);
  ptr_data3 -= (n1 >> 1);
  ptr_data4 -= (n1 >> 1);
  ia_enhaacplus_enc_tranform_mac3(ptr_op1, ptr_ws1, ptr_data2, ptr_data3, ptr_data4, (n1 >> 1),
                                  -1);

  if (long_frame_len == FRAME_LEN_480) {
    ia_aac_ld_enc_mdct_480(ptr_real_out, ptr_windowed_buf, 1, pstr_mdct_tab);
  } else {
    ia_eaacp_enc_inverse_transform_512(ptr_real_out, ptr_windowed_buf,
                                       pstr_mdct_tab->cosine_array_1024, ptr_shared_buffer5);
  }
}

static VOID ia_eaacp_enc_pre_twiddle_compute(FLOAT32 *ptr_in1, FLOAT32 *ptr_in2, FLOAT32 *ptr_x,
                                             const FLOAT32 *ptr_cos_sin, WORD n_by_4) {
  WORD32 i;
  FLOAT32 temp_r, temp_i;
  FLOAT32 temp_r1, temp_i1;
  FLOAT32 *ptr_x1 = ptr_x + (SIZE_T)((n_by_4 << 2) - 1);
  FLOAT32 c, c1, s, s1;

  for (i = 0; i < n_by_4; i++) {
    c = *ptr_cos_sin++;
    s = *ptr_cos_sin++;
    s1 = *ptr_cos_sin++;
    c1 = *ptr_cos_sin++;

    temp_r = *ptr_in1++;
    temp_i1 = *ptr_in1++;
    temp_i = *ptr_in2--;
    temp_r1 = *ptr_in2--;
    *ptr_x = ((temp_r * c) + (temp_i * s));
    ptr_x++;

    *ptr_x = ((temp_i * c) - (temp_r * s));
    ptr_x++;

    *ptr_x1 = ((temp_i1 * c1) - (temp_r1 * s1));
    ptr_x1--;

    *ptr_x1 = ((temp_r1 * c1) + (temp_i1 * s1));
    ptr_x1--;
  }
}

VOID ia_enhaacplus_enc_post_twiddle(FLOAT32 *ptr_out, FLOAT32 *ptr_x,
                                    const FLOAT32 *ptr_cos_sin_tbl, WORD m) {
  WORD i;
  FLOAT32 c, c1, s, s1;
  FLOAT32 tmp_var;
  FLOAT32 tempr, tempr1, tempi, tempi1;
  FLOAT32 *ptr_out1 = ptr_out + m - 1;
  FLOAT32 *ptr_x1 = ptr_x + m - 1;

  for (i = 0; i < (m >> 2); i++) {
    c = *ptr_cos_sin_tbl++;
    s = *ptr_cos_sin_tbl++;
    s1 = *ptr_cos_sin_tbl++;
    c1 = *ptr_cos_sin_tbl++;
    tempr = *ptr_x++;
    tempi = *ptr_x++;
    tempi1 = *ptr_x1--;
    tempr1 = *ptr_x1--;

    tmp_var = ((tempr * c) + (tempi * s));
    *ptr_out++ = tmp_var;

    tmp_var = ((tempr * s) - (tempi * c));
    *ptr_out1-- = tmp_var;

    tmp_var = ((tempr1 * c1) + (tempi1 * s1));
    *ptr_out1-- = tmp_var;

    tmp_var = ((tempr1 * s1) - (tempi1 * c1));
    *ptr_out++ = tmp_var;
  }
}

VOID ia_eaacp_enc_pre_twiddle_aac(FLOAT32 *ptr_x, FLOAT32 *ptr_data, WORD32 n,
                                  const FLOAT32 *ptr_cos_array) {
  WORD n_by_4;
  FLOAT32 *ptr_in1, *ptr_in2;

  n_by_4 = n >> 2;

  ptr_in1 = ptr_data;
  ptr_in2 = ptr_data + n - 1;

  ia_eaacp_enc_pre_twiddle_compute(ptr_in1, ptr_in2, ptr_x, ptr_cos_array, n_by_4);
}

static PLATFORM_INLINE WORD8 ia_enhaacplus_enc_calc_norm(WORD32 a) {
  WORD8 norm_val;

  if (a == 0) {
    norm_val = 31;
  } else {
    if (a == (WORD32)0xffffffffL) {
      norm_val = 31;
    } else {
      if (a < 0) {
        a = ~a;
      }
      for (norm_val = 0; a < (WORD32)0x40000000L; norm_val++) {
        a <<= 1;
      }
    }
  }

  return norm_val;
}

static PLATFORM_INLINE VOID ia_enhaacplus_enc_complex_3point_fft(FLOAT32 *ptr_in,
                                                                 FLOAT32 *ptr_out) {
  FLOAT32 add_r, sub_r;
  FLOAT32 add_i, sub_i;
  FLOAT32 x_01_r, x_01_i, temp;
  FLOAT32 p1, p2, p3, p4;
  FLOAT64 sin_mu = 0.866025403784439f;

  x_01_r = ptr_in[0] + ptr_in[2];
  x_01_i = ptr_in[1] + ptr_in[3];

  add_r = ptr_in[2] + ptr_in[4];
  add_i = ptr_in[3] + ptr_in[5];

  sub_r = ptr_in[2] - ptr_in[4];
  sub_i = ptr_in[3] - ptr_in[5];

  p1 = add_r / (FLOAT32)2.0f;
  p4 = add_i / (FLOAT32)2.0f;
  p2 = (FLOAT32)((FLOAT64)sub_i * sin_mu);
  p3 = (FLOAT32)((FLOAT64)sub_r * sin_mu);

  temp = ptr_in[0] - p1;

  ptr_out[0] = x_01_r + ptr_in[4];
  ptr_out[1] = x_01_i + ptr_in[5];
  ptr_out[2] = temp + p2;
  ptr_out[3] = (ptr_in[1] - p3) - p4;
  ptr_out[4] = temp - p2;
  ptr_out[5] = (ptr_in[1] + p3) - p4;
}

VOID ia_enhaacplus_enc_complex_fft_p2(FLOAT32 *ptr_x, WORD32 nlength,
                                      FLOAT32 *ptr_scratch_fft_p2_y) {
  WORD32 i, j, k, n_stages, h2;
  FLOAT32 x0_r, x0_i, x1_r, x1_i, x2_r, x2_i, x3_r, x3_i;
  WORD32 del, nodespacing, in_loop_cnt;
  WORD32 not_power_4;
  WORD32 dig_rev_shift;
  FLOAT32 *ptr_p2_y = ptr_scratch_fft_p2_y;
  WORD32 mpass = nlength;
  WORD32 npoints = nlength;
  FLOAT32 *ptr_y = ptr_p2_y;
  const FLOAT64 *ptr_w;
  FLOAT32 *ptr_inp;
  FLOAT32 tmk;
  const FLOAT64 *ptr_twiddles;
  FLOAT32 *ptr_data;
  FLOAT64 w_1, w_2, w_3, w_4, w_5, w_6;
  WORD32 sec_loop_cnt;
  FLOAT32 tmp;

  memset(ptr_y, 0, nlength * 2 * sizeof(*ptr_y));

  dig_rev_shift = ia_enhaacplus_enc_calc_norm(mpass) + 1 - 16;
  n_stages = 30 - ia_enhaacplus_enc_calc_norm(mpass);
  not_power_4 = n_stages & 1;

  n_stages = n_stages >> 1;

  ptr_w = ia_enhaacplus_enc_twiddle_table_fft_32x32;

  dig_rev_shift = MAX(dig_rev_shift, 0);

  for (i = 0; i < npoints; i += 4) {
    ptr_inp = ptr_x;
    DIG_REV_NEW(i, dig_rev_shift, h2);
    if (not_power_4) {
      h2 += 1;
      h2 &= ~1;
    }
    ptr_inp += (h2);

    x0_r = *ptr_inp;
    x0_i = *(ptr_inp + 1);
    ptr_inp += (npoints >> 1);

    x1_r = *ptr_inp;
    x1_i = *(ptr_inp + 1);
    ptr_inp += (npoints >> 1);

    x2_r = *ptr_inp;
    x2_i = *(ptr_inp + 1);
    ptr_inp += (npoints >> 1);

    x3_r = *ptr_inp;
    x3_i = *(ptr_inp + 1);

    x0_r = x0_r + x2_r;
    x0_i = x0_i + x2_i;

    tmk = x0_r - x2_r;
    x2_r = tmk - x2_r;
    tmk = x0_i - x2_i;
    x2_i = tmk - x2_i;

    x1_r = x1_r + x3_r;
    x1_i = x1_i + x3_i;

    tmk = x1_r - x3_r;
    x3_r = tmk - x3_r;
    tmk = x1_i - x3_i;
    x3_i = tmk - x3_i;

    x0_r = x0_r + x1_r;
    x0_i = x0_i + x1_i;

    tmk = x0_r - x1_r;
    x1_r = tmk - x1_r;
    tmk = x0_i - x1_i;
    x1_i = tmk - x1_i;

    x2_r = x2_r + x3_i;
    x2_i = x2_i - x3_r;

    tmk = x2_r - x3_i;
    x3_i = tmk - x3_i;
    tmk = x2_i + x3_r;
    x3_r = tmk + x3_r;

    *ptr_y++ = x0_r;
    *ptr_y++ = x0_i;
    *ptr_y++ = x2_r;
    *ptr_y++ = x2_i;
    *ptr_y++ = x1_r;
    *ptr_y++ = x1_i;
    *ptr_y++ = x3_i;
    *ptr_y++ = x3_r;
  }
  ptr_y -= 2 * npoints;
  del = 4;
  nodespacing = 64;
  in_loop_cnt = npoints >> 4;
  for (i = n_stages - 1; i > 0; i--) {
    ptr_twiddles = ptr_w;
    ptr_data = ptr_y;
    for (k = in_loop_cnt; k != 0; k--) {
      x0_r = (*ptr_data);
      x0_i = (*(ptr_data + 1));
      ptr_data += ((SIZE_T)del << 1);

      x1_r = (*ptr_data);
      x1_i = (*(ptr_data + 1));
      ptr_data += ((SIZE_T)del << 1);

      x2_r = (*ptr_data);
      x2_i = (*(ptr_data + 1));
      ptr_data += ((SIZE_T)del << 1);

      x3_r = (*ptr_data);
      x3_i = (*(ptr_data + 1));
      ptr_data -= 3 * (del << 1);

      x0_r = x0_r + x2_r;
      x0_i = x0_i + x2_i;
      x2_r = x0_r - (x2_r * 2);
      x2_i = x0_i - (x2_i * 2);
      x1_r = x1_r + x3_r;
      x1_i = x1_i + x3_i;
      x3_r = x1_r - (x3_r * 2);
      x3_i = x1_i - (x3_i * 2);

      x0_r = x0_r + x1_r;
      x0_i = x0_i + x1_i;
      x1_r = x0_r - (x1_r * 2);
      x1_i = x0_i - (x1_i * 2);
      x2_r = x2_r + x3_i;
      x2_i = x2_i - x3_r;
      x3_i = x2_r - (x3_i * 2);
      x3_r = x2_i + (x3_r * 2);

      *ptr_data = x0_r;
      *(ptr_data + 1) = x0_i;
      ptr_data += ((SIZE_T)del << 1);

      *ptr_data = x2_r;
      *(ptr_data + 1) = x2_i;
      ptr_data += ((SIZE_T)del << 1);

      *ptr_data = x1_r;
      *(ptr_data + 1) = x1_i;
      ptr_data += ((SIZE_T)del << 1);

      *ptr_data = x3_i;
      *(ptr_data + 1) = x3_r;
      ptr_data += ((SIZE_T)del << 1);
    }
    ptr_data = ptr_y + 2;

    sec_loop_cnt = (nodespacing * del);
    sec_loop_cnt = (sec_loop_cnt / 4) + (sec_loop_cnt / 8) - (sec_loop_cnt / 16) +
                   (sec_loop_cnt / 32) - (sec_loop_cnt / 64) + (sec_loop_cnt / 128) -
                   (sec_loop_cnt / 256);

    for (j = nodespacing; j <= sec_loop_cnt; j += nodespacing) {
      w_1 = *(ptr_twiddles + j);
      w_4 = *(ptr_twiddles + j + 257);
      w_2 = *(ptr_twiddles + ((SIZE_T)j << 1));
      w_5 = *(ptr_twiddles + ((SIZE_T)j << 1) + 257);
      w_3 = *(ptr_twiddles + j + ((SIZE_T)j << 1));
      w_6 = *(ptr_twiddles + j + ((SIZE_T)j << 1) + 257);

      for (k = in_loop_cnt; k != 0; k--) {
        ptr_data += ((SIZE_T)del << 1);

        x1_r = *ptr_data;
        x1_i = *(ptr_data + 1);
        ptr_data += ((SIZE_T)del << 1);

        x2_r = *ptr_data;
        x2_i = *(ptr_data + 1);
        ptr_data += ((SIZE_T)del << 1);

        x3_r = *ptr_data;
        x3_i = *(ptr_data + 1);
        ptr_data -= 3 * (del << 1);

        tmp = (FLOAT32)(ixheaace_dmult((FLOAT64)x1_r, w_1) - ixheaace_dmult((FLOAT64)x1_i, w_4));
        x1_i = (FLOAT32)ixheaace_dmac(ixheaace_dmult((FLOAT64)x1_r, w_4), (FLOAT64)x1_i, w_1);
        x1_r = tmp;

        tmp = (FLOAT32)(ixheaace_dmult((FLOAT64)x2_r, w_2) - ixheaace_dmult((FLOAT64)x2_i, w_5));
        x2_i = (FLOAT32)ixheaace_dmac(ixheaace_dmult((FLOAT64)x2_r, w_5), (FLOAT64)x2_i, w_2);
        x2_r = tmp;

        tmp = (FLOAT32)(ixheaace_dmult((FLOAT64)x3_r, w_3) - ixheaace_dmult((FLOAT64)x3_i, w_6));
        x3_i = (FLOAT32)ixheaace_dmac(ixheaace_dmult((FLOAT64)x3_r, w_6), (FLOAT64)x3_i, w_3);
        x3_r = tmp;

        x0_r = (*ptr_data);
        x0_i = (*(ptr_data + 1));

        x0_r = x0_r + (x2_r);
        x0_i = x0_i + (x2_i);
        x2_r = x0_r - (x2_r * 2);
        x2_i = x0_i - (x2_i * 2);
        x1_r = x1_r + x3_r;
        x1_i = x1_i + x3_i;
        x3_r = x1_r - (x3_r * 2);
        x3_i = x1_i - (x3_i * 2);

        x0_r = x0_r + (x1_r);
        x0_i = x0_i + (x1_i);
        x1_r = x0_r - (x1_r * 2);
        x1_i = x0_i - (x1_i * 2);
        x2_r = x2_r + (x3_i);
        x2_i = x2_i - (x3_r);
        x3_i = x2_r - (x3_i * 2);
        x3_r = x2_i + (x3_r * 2);

        *ptr_data = x0_r;
        *(ptr_data + 1) = x0_i;
        ptr_data += ((SIZE_T)del << 1);

        *ptr_data = x2_r;
        *(ptr_data + 1) = x2_i;
        ptr_data += ((SIZE_T)del << 1);

        *ptr_data = x1_r;
        *(ptr_data + 1) = x1_i;
        ptr_data += ((SIZE_T)del << 1);

        *ptr_data = x3_i;
        *(ptr_data + 1) = x3_r;
        ptr_data += ((SIZE_T)del << 1);
      }
      ptr_data -= 2 * npoints;
      ptr_data += 2;
    }
    for (; j <= (nodespacing * del) >> 1; j += nodespacing) {
      w_1 = *(ptr_twiddles + j);
      w_4 = *(ptr_twiddles + j + 257);
      w_2 = *(ptr_twiddles + ((SIZE_T)j << 1));
      w_5 = *(ptr_twiddles + ((SIZE_T)j << 1) + 257);
      w_3 = *(ptr_twiddles + j + ((SIZE_T)j << 1) - 256);
      w_6 = *(ptr_twiddles + j + ((SIZE_T)j << 1) + 1);

      for (k = in_loop_cnt; k != 0; k--) {
        ptr_data += ((SIZE_T)del << 1);

        x1_r = *ptr_data;
        x1_i = *(ptr_data + 1);
        ptr_data += ((SIZE_T)del << 1);

        x2_r = *ptr_data;
        x2_i = *(ptr_data + 1);
        ptr_data += ((SIZE_T)del << 1);

        x3_r = *ptr_data;
        x3_i = *(ptr_data + 1);
        ptr_data -= 3 * (del << 1);

        tmp = (FLOAT32)(ixheaace_dmult((FLOAT64)x1_r, w_1) - ixheaace_dmult((FLOAT64)x1_i, w_4));
        x1_i = (FLOAT32)ixheaace_dmac(ixheaace_dmult((FLOAT64)x1_r, w_4), (FLOAT64)x1_i, w_1);
        x1_r = tmp;

        tmp = (FLOAT32)(ixheaace_dmult((FLOAT64)x2_r, w_2) - ixheaace_dmult((FLOAT64)x2_i, w_5));
        x2_i = (FLOAT32)ixheaace_dmac(ixheaace_dmult((FLOAT64)x2_r, w_5), (FLOAT64)x2_i, w_2);
        x2_r = tmp;

        tmp = (FLOAT32)(ixheaace_dmult((FLOAT64)x3_r, w_6) + ixheaace_dmult((FLOAT64)x3_i, w_3));
        x3_i =
            (FLOAT32)(-ixheaace_dmult((FLOAT64)x3_r, w_3) + ixheaace_dmult((FLOAT64)x3_i, w_6));
        x3_r = tmp;

        x0_r = (*ptr_data);
        x0_i = (*(ptr_data + 1));

        x0_r = x0_r + (x2_r);
        x0_i = x0_i + (x2_i);
        x2_r = x0_r - (x2_r * 2);
        x2_i = x0_i - (x2_i * 2);
        x1_r = x1_r + x3_r;
        x1_i = x1_i + x3_i;
        x3_r = x1_r - (x3_r * 2);
        x3_i = x1_i - (x3_i * 2);

        x0_r = x0_r + (x1_r);
        x0_i = x0_i + (x1_i);
        x1_r = x0_r - (x1_r * 2);
        x1_i = x0_i - (x1_i * 2);
        x2_r = x2_r + (x3_i);
        x2_i = x2_i - (x3_r);
        x3_i = x2_r - (x3_i * 2);
        x3_r = x2_i + (x3_r * 2);

        *ptr_data = x0_r;
        *(ptr_data + 1) = x0_i;
        ptr_data += ((SIZE_T)del << 1);

        *ptr_data = x2_r;
        *(ptr_data + 1) = x2_i;
        ptr_data += ((SIZE_T)del << 1);

        *ptr_data = x1_r;
        *(ptr_data + 1) = x1_i;
        ptr_data += ((SIZE_T)del << 1);

        *ptr_data = x3_i;
        *(ptr_data + 1) = x3_r;
        ptr_data += ((SIZE_T)del << 1);
      }
      ptr_data -= 2 * npoints;
      ptr_data += 2;
    }
    for (; j <= sec_loop_cnt * 2; j += nodespacing) {
      w_1 = *(ptr_twiddles + j);
      w_4 = *(ptr_twiddles + j + 257);
      w_2 = *(ptr_twiddles + (SIZE_T)((j << 1) - 256));
      w_5 = *(ptr_twiddles + (SIZE_T)((j << 1) + 1));
      w_3 = *(ptr_twiddles + (SIZE_T)(j + (j << 1) - 256));
      w_6 = *(ptr_twiddles + (SIZE_T)(j + (j << 1) + 1));

      for (k = in_loop_cnt; k != 0; k--) {
        ptr_data += ((SIZE_T)del << 1);

        x1_r = *ptr_data;
        x1_i = *(ptr_data + 1);
        ptr_data += ((SIZE_T)del << 1);

        x2_r = *ptr_data;
        x2_i = *(ptr_data + 1);
        ptr_data += ((SIZE_T)del << 1);

        x3_r = *ptr_data;
        x3_i = *(ptr_data + 1);
        ptr_data -= 3 * (del << 1);

        tmp = (FLOAT32)(ixheaace_dmult((FLOAT64)x1_r, w_1) - ixheaace_dmult((FLOAT64)x1_i, w_4));
        x1_i = (FLOAT32)ixheaace_dmac(ixheaace_dmult(x1_r, w_4), x1_i, w_1);
        x1_r = tmp;

        tmp = (FLOAT32)(ixheaace_dmult((FLOAT64)x2_r, w_5) + ixheaace_dmult((FLOAT64)x2_i, w_2));
        x2_i = (FLOAT32)(-ixheaace_dmult(x2_r, w_2) + ixheaace_dmult(x2_i, w_5));
        x2_r = tmp;

        tmp = (FLOAT32)(ixheaace_dmult((FLOAT64)x3_r, w_6) + ixheaace_dmult((FLOAT64)x3_i, w_3));
        x3_i =
            (FLOAT32)(-ixheaace_dmult((FLOAT64)x3_r, w_3) + ixheaace_dmult((FLOAT64)x3_i, w_6));
        x3_r = tmp;

        x0_r = (*ptr_data);
        x0_i = (*(ptr_data + 1));

        x0_r = x0_r + (x2_r);
        x0_i = x0_i + (x2_i);
        x2_r = x0_r - (x2_r * 2);
        x2_i = x0_i - (x2_i * 2);
        x1_r = x1_r + x3_r;
        x1_i = x1_i + x3_i;
        x3_r = x1_r - (x3_r * 2);
        x3_i = x1_i - (x3_i * 2);

        x0_r = x0_r + (x1_r);
        x0_i = x0_i + (x1_i);
        x1_r = x0_r - (x1_r * 2);
        x1_i = x0_i - (x1_i * 2);
        x2_r = x2_r + (x3_i);
        x2_i = x2_i - (x3_r);
        x3_i = x2_r - (x3_i * 2);
        x3_r = x2_i + (x3_r * 2);

        *ptr_data = x0_r;
        *(ptr_data + 1) = x0_i;
        ptr_data += ((SIZE_T)del << 1);

        *ptr_data = x2_r;
        *(ptr_data + 1) = x2_i;
        ptr_data += ((SIZE_T)del << 1);

        *ptr_data = x1_r;
        *(ptr_data + 1) = x1_i;
        ptr_data += ((SIZE_T)del << 1);

        *ptr_data = x3_i;
        *(ptr_data + 1) = x3_r;
        ptr_data += ((SIZE_T)del << 1);
      }
      ptr_data -= 2 * npoints;
      ptr_data += 2;
    }
    for (; j < nodespacing * del; j += nodespacing) {
      w_1 = *(ptr_twiddles + j);
      w_4 = *(ptr_twiddles + j + 257);
      w_2 = *(ptr_twiddles + (SIZE_T)((j << 1) - 256));
      w_5 = *(ptr_twiddles + (SIZE_T)((j << 1) + 1));
      w_3 = *(ptr_twiddles + (SIZE_T)(j + (j << 1) - 512));
      w_6 = *(ptr_twiddles + (SIZE_T)(j + (j << 1) - 512 + 257));

      for (k = in_loop_cnt; k != 0; k--) {
        ptr_data += ((SIZE_T)del << 1);

        x1_r = *ptr_data;
        x1_i = *(ptr_data + 1);
        ptr_data += ((SIZE_T)del << 1);

        x2_r = *ptr_data;
        x2_i = *(ptr_data + 1);
        ptr_data += ((SIZE_T)del << 1);

        x3_r = *ptr_data;
        x3_i = *(ptr_data + 1);
        ptr_data -= 3 * (del << 1);

        tmp = (FLOAT32)(ixheaace_dmult((FLOAT64)x1_r, w_1) - ixheaace_dmult((FLOAT64)x1_i, w_4));
        x1_i = (FLOAT32)ixheaace_dmac(ixheaace_dmult((FLOAT64)x1_r, w_4), (FLOAT64)x1_i, w_1);
        x1_r = tmp;

        tmp = (FLOAT32)(ixheaace_dmult((FLOAT64)x2_r, w_5) + ixheaace_dmult((FLOAT64)x2_i, w_2));
        x2_i =
            (FLOAT32)(-ixheaace_dmult((FLOAT64)x2_r, w_2) + ixheaace_dmult((FLOAT64)x2_i, w_5));
        x2_r = tmp;

        tmp = (FLOAT32)(-ixheaace_dmult((FLOAT64)x3_r, w_3) + ixheaace_dmult((FLOAT64)x3_i, w_6));
        x3_i = (FLOAT32)ixheaace_dmac(ixheaace_dmult((FLOAT64)x3_r, w_6), (FLOAT64)x3_i, w_3);
        x3_r = tmp;

        x0_r = (*ptr_data);
        x0_i = (*(ptr_data + 1));

        x0_r = x0_r + (x2_r);
        x0_i = x0_i + (x2_i);
        x2_r = x0_r - (x2_r * 2);
        x2_i = x0_i - (x2_i * 2);
        x1_r = x1_r + x3_r;
        x1_i = x1_i - x3_i;
        x3_r = x1_r - (x3_r * 2);
        x3_i = x1_i + (x3_i * 2);

        x0_r = x0_r + (x1_r);
        x0_i = x0_i + (x1_i);
        x1_r = x0_r - (x1_r * 2);
        x1_i = x0_i - (x1_i * 2);
        x2_r = x2_r + (x3_i);
        x2_i = x2_i - (x3_r);
        x3_i = x2_r - (x3_i * 2);
        x3_r = x2_i + (x3_r * 2);

        *ptr_data = x0_r;
        *(ptr_data + 1) = x0_i;
        ptr_data += ((SIZE_T)del << 1);

        *ptr_data = x2_r;
        *(ptr_data + 1) = x2_i;
        ptr_data += ((SIZE_T)del << 1);

        *ptr_data = x1_r;
        *(ptr_data + 1) = x1_i;
        ptr_data += ((SIZE_T)del << 1);

        *ptr_data = x3_i;
        *(ptr_data + 1) = x3_r;
        ptr_data += ((SIZE_T)del << 1);
      }
      ptr_data -= 2 * npoints;
      ptr_data += 2;
    }
    nodespacing >>= 2;
    del <<= 2;
    in_loop_cnt >>= 2;
  }
  if (not_power_4) {
    ptr_twiddles = ptr_w;
    nodespacing <<= 1;

    for (j = del / 2; j != 0; j--) {
      w_1 = *ptr_twiddles;
      w_4 = *(ptr_twiddles + 257);
      ptr_twiddles += nodespacing;

      x0_r = *ptr_y;
      x0_i = *(ptr_y + 1);
      ptr_y += ((SIZE_T)del << 1);

      x1_r = *ptr_y;
      x1_i = *(ptr_y + 1);

      tmp = (FLOAT32)(ixheaace_dmult((FLOAT64)x1_r, w_1) - ixheaace_dmult((FLOAT64)x1_i, w_4));
      x1_i = (FLOAT32)ixheaace_dmac(ixheaace_dmult((FLOAT64)x1_r, w_4), (FLOAT64)x1_i, w_1);
      x1_r = tmp;

      *ptr_y = (x0_r) - (x1_r);
      *(ptr_y + 1) = (x0_i) - (x1_i);
      ptr_y -= ((SIZE_T)del << 1);

      *ptr_y = (x0_r) + (x1_r);
      *(ptr_y + 1) = (x0_i) + (x1_i);
      ptr_y += 2;
    }
    ptr_twiddles = ptr_w;
    for (j = del / 2; j != 0; j--) {
      w_1 = *ptr_twiddles;
      w_4 = *(ptr_twiddles + 257);
      ptr_twiddles += nodespacing;

      x0_r = *ptr_y;
      x0_i = *(ptr_y + 1);
      ptr_y += ((SIZE_T)del << 1);

      x1_r = *ptr_y;
      x1_i = *(ptr_y + 1);

      tmp = (FLOAT32)(ixheaace_dmult((FLOAT64)x1_r, w_4) +
                      ixheaace_dmult((FLOAT64)x1_i, w_1)) /*/2*/;
      x1_i = (FLOAT32)(-ixheaace_dmult((FLOAT64)x1_r, w_1) +
                       ixheaace_dmult((FLOAT64)x1_i, w_4)) /*/2*/;
      x1_r = tmp;

      *ptr_y = (x0_r) - (x1_r);
      *(ptr_y + 1) = (x0_i) - (x1_i);
      ptr_y -= ((SIZE_T)del << 1);

      *ptr_y = (x0_r) + (x1_r);
      *(ptr_y + 1) = (x0_i) + (x1_i);
      ptr_y += 2;
    }
  }

  for (i = 0; i < nlength; i++) {
    *(ptr_x + 2 * i) = ptr_p2_y[2 * i];
    *(ptr_x + 2 * i + 1) = ptr_p2_y[2 * i + 1];
  }
}

static VOID ia_enhaacplus_enc_complex_fft_p3(FLOAT32 *ptr_data, WORD32 nlength,
                                             ixheaace_scratch_mem *pstr_scratch) {
  WORD32 i, j;
  FLOAT32 *ptr_data_3 = pstr_scratch->p_fft_p3_data_3;
  FLOAT32 *ptr_p3_y = pstr_scratch->p_fft_p3_y;
  WORD32 cnfac;
  WORD32 mpass = nlength;
  FLOAT32 *ptr_x = ptr_data;
  FLOAT32 *ptr_y = ptr_p3_y;
  cnfac = 0;
  const FLOAT64 *ptr_w1_r, *ptr_w1_i;
  FLOAT32 tmp;
  ptr_w1_r = ia_enhaacplus_enc_twiddle_table_3pr;
  ptr_w1_i = ia_enhaacplus_enc_twiddle_table_3pi;

  while (mpass % 3 == 0) {
    mpass /= 3;
    cnfac++;
  }

  for (i = 0; i < 3 * cnfac; i++) {
    for (j = 0; j < mpass; j++) {
      ptr_data_3[2 * j] = ptr_data[3 * (2 * j) + (2 * i)];
      ptr_data_3[2 * j + 1] = ptr_data[3 * (2 * j) + 1 + (2 * i)];
    }
    ia_enhaacplus_enc_complex_fft_p2(ptr_data_3, mpass, pstr_scratch->p_fft_p2_y);

    for (j = 0; j < mpass; j++) {
      ptr_data[3 * (2 * j) + (2 * i)] = ptr_data_3[2 * j];
      ptr_data[3 * (2 * j) + 1 + (2 * i)] = ptr_data_3[2 * j + 1];
    }
  }

  {
    for (i = 0; i < nlength; i += 3) {
      tmp = (FLOAT32)((FLOAT64)ptr_data[2 * i] * (*ptr_w1_r) -
                      (FLOAT64)ptr_data[2 * i + 1] * (*ptr_w1_i));
      ptr_data[2 * i + 1] = (FLOAT32)((FLOAT64)ptr_data[2 * i] * (*ptr_w1_i) +
                                      (FLOAT64)ptr_data[2 * i + 1] * (*ptr_w1_r));
      ptr_data[2 * i] = tmp;

      ptr_w1_r++;
      ptr_w1_i++;

      tmp = (FLOAT32)((FLOAT64)ptr_data[2 * (i + 1)] * (*ptr_w1_r) -
                      (FLOAT64)ptr_data[2 * (i + 1) + 1] * (*ptr_w1_i));
      ptr_data[2 * (i + 1) + 1] = (FLOAT32)((FLOAT64)ptr_data[2 * (i + 1)] * (*ptr_w1_i) +
                                            (FLOAT64)ptr_data[2 * (i + 1) + 1] * (*ptr_w1_r));
      ptr_data[2 * (i + 1)] = tmp;

      ptr_w1_r++;
      ptr_w1_i++;

      tmp = (FLOAT32)((FLOAT64)ptr_data[2 * (i + 2)] * (*ptr_w1_r) -
                      (FLOAT64)ptr_data[2 * (i + 2) + 1] * (*ptr_w1_i));
      ptr_data[2 * (i + 2) + 1] = (FLOAT32)((FLOAT64)ptr_data[2 * (i + 2)] * (*ptr_w1_i) +
                                            (FLOAT64)ptr_data[2 * (i + 2) + 1] * (*ptr_w1_r));
      ptr_data[2 * (i + 2)] = tmp;

      ptr_w1_r += 3 * (128 / mpass - 1) + 1;
      ptr_w1_i += 3 * (128 / mpass - 1) + 1;
    }
  }

  for (i = 0; i < mpass; i++) {
    ia_enhaacplus_enc_complex_3point_fft(ptr_x, ptr_y);

    ptr_x = ptr_x + 6;
    ptr_y = ptr_y + 6;
  }

  for (i = 0; i < mpass; i++) {
    ptr_data[2 * i] = ptr_p3_y[6 * i];
    ptr_data[2 * i + 1] = ptr_p3_y[6 * i + 1];
  }

  for (i = 0; i < mpass; i++) {
    ptr_data[2 * (i + mpass)] = ptr_p3_y[6 * i + 2];
    ptr_data[2 * (i + mpass) + 1] = ptr_p3_y[6 * i + 3];
  }

  for (i = 0; i < mpass; i++) {
    ptr_data[2 * (i + 2 * mpass)] = ptr_p3_y[6 * i + 4];
    ptr_data[2 * (i + 2 * mpass) + 1] = ptr_p3_y[6 * i + 5];
  }
}

VOID ia_enhaacplus_enc_complex_fft(FLOAT32 *ptr_data, WORD32 len,
                                   ixheaace_scratch_mem *pstr_scratch) {
  if (len & (len - 1)) {
    ia_enhaacplus_enc_complex_fft_p3(ptr_data, len, pstr_scratch);
  } else {
    ia_enhaacplus_enc_complex_fft_p2(ptr_data, len, pstr_scratch->p_fft_p2_y);
  }
}

static VOID ixheaace_post_mdct(FLOAT32 *ptr_x, WORD32 m, const FLOAT32 *ptr_trig_data,
                               WORD32 step, WORD32 trig_data_size) {
  WORD32 i;
  FLOAT32 w_re, w_im, re1, re2, im1, im2;
  const FLOAT32 *ptr_sin = ptr_trig_data;
  const FLOAT32 *ptr_cos = ptr_trig_data + trig_data_size;

  w_im = *ptr_sin;
  w_re = *ptr_cos;

  for (i = 0; i < m / 4; i++) {
    re1 = ptr_x[2 * i];
    im1 = ptr_x[2 * i + 1];
    re2 = ptr_x[m - 2 - 2 * i];
    im2 = ptr_x[m - 1 - 2 * i];

    ptr_x[2 * i] = (re1 * w_re + im1 * w_im);

    ptr_x[m - 1 - 2 * i] = (re1 * w_im - im1 * w_re);

    ptr_sin += step;
    ptr_cos -= step;

    w_im = *ptr_sin;
    w_re = *ptr_cos;

    ptr_x[m - 2 - 2 * i] = (re2 * w_im + im2 * w_re);

    ptr_x[2 * i + 1] = (re2 * w_re - im2 * w_im);
  }
}

static VOID ixheaace_cplx_mult_twid(FLOAT32 *ptr_re, FLOAT32 *ptr_im, FLOAT32 a, FLOAT32 b,
                                    FLOAT32 twid_table, FLOAT32 twid_table_h) {
  *ptr_re = (a * twid_table) - (b * twid_table_h);
  *ptr_im = (a * twid_table_h) + (b * twid_table);
}

static VOID ixheaace_cfft_15_twiddle(FLOAT32 *ptr_inp) {
  const FLOAT32 *ptr_tw_flt = &ixheaace_mix_rad_twid_tbl[0];
  const FLOAT32 *ptr_tw_flt_h = &ixheaace_mix_rad_twid_tbl_h[0];
  FLOAT32 accu1, accu2;
  WORD32 i, j;
  ptr_inp += 12;

  for (j = 0; j < 2; j++) {
    for (i = 0; i < 4; i++) {
      ixheaace_cplx_mult_twid(&accu1, &accu2, ptr_inp[2 * i + 0], ptr_inp[2 * i + 1],
                              ptr_tw_flt[i], ptr_tw_flt_h[i]);
      ptr_inp[2 * i + 0] = accu1;
      ptr_inp[2 * i + 1] = accu2;
    }
    ptr_inp += 10;
    ptr_tw_flt += 4;
    ptr_tw_flt_h += 4;
  }
}

static VOID ixheaace_cfft_15_480(FLOAT32 *ptr_inp, FLOAT32 *ptr_op, FLOAT32 *ptr_fft3_out) {
  WORD32 i, idx;
  FLOAT32 *ptr_buf1, *ptr_buf2, *ptr_buf3;
  FLOAT32 add_r, sub_r;
  FLOAT32 add_i, sub_i;
  FLOAT32 x_01_r, x_01_i, temp;
  FLOAT32 p1, p2, p3, p4;

  FLOAT32 sin_mu_flt = 0.866027832f;
  FLOAT32 c51_flt = 0.951049805f;
  FLOAT32 c52_flt = -0.76940918f;
  FLOAT32 c53_flt = -0.36328125f;
  FLOAT32 c54_flt = 0.559020996f;
  FLOAT32 c55_flt = -0.625f;

  FLOAT32 r1, r2, r3, r4;
  FLOAT32 s1, s2, s3, s4, t, temp1, temp2;
  FLOAT32 *ptr_out_fft3 = ptr_fft3_out;

  FLOAT32 xr_0, xr_1, xr_2;
  FLOAT32 xi_0, xi_1, xi_2;

  ptr_buf2 = ptr_fft3_out;
  ptr_buf1 = ptr_buf3 = ptr_fft3_out;

  for (i = 0; i < FFT3; i++) {
    *ptr_buf1++ = ptr_inp[0 + 64 * i];
    *ptr_buf1++ = ptr_inp[1 + 64 * i];

    *ptr_buf1++ = ptr_inp[192 + 64 * i];
    *ptr_buf1++ = ptr_inp[193 + 64 * i];

    *ptr_buf1++ = ptr_inp[384 + 64 * i];
    *ptr_buf1++ = ptr_inp[385 + 64 * i];

    *ptr_buf1++ = ptr_inp[576 + 64 * i];
    *ptr_buf1++ = ptr_inp[577 + 64 * i];

    *ptr_buf1++ = ptr_inp[768 + 64 * i];
    *ptr_buf1++ = ptr_inp[769 + 64 * i];

    r1 = ptr_buf3[2] + ptr_buf3[8];
    r4 = ptr_buf3[2] - ptr_buf3[8];
    r3 = ptr_buf3[4] + ptr_buf3[6];
    r2 = ptr_buf3[4] - ptr_buf3[6];
    t = ((r1 - r3) * c54_flt);

    r1 = r1 + r3;

    temp1 = ptr_buf3[0] + r1;

    r1 = temp1 + ((r1 * c55_flt) * 2);

    r3 = r1 - t;
    r1 = r1 + t;

    t = ((r4 + r2) * c51_flt);
    r4 = t + ((r4 * c52_flt) * 2);
    r2 = t + (r2 * c53_flt);

    s1 = ptr_buf3[3] + ptr_buf3[9];
    s4 = ptr_buf3[3] - ptr_buf3[9];
    s3 = ptr_buf3[5] + ptr_buf3[7];
    s2 = ptr_buf3[5] - ptr_buf3[7];

    t = ((s1 - s3) * c54_flt);

    s1 = s1 + s3;

    temp2 = ptr_buf3[1] + s1;

    s1 = temp2 + (((s1 * c55_flt)) * 2);

    s3 = s1 - t;
    s1 = s1 + t;

    t = ((s4 + s2) * c51_flt);
    s4 = t + (((s4 * c52_flt)) * 2);
    s2 = t + ((s2 * c53_flt));

    *ptr_buf2++ = temp1;
    *ptr_buf2++ = temp2;
    *ptr_buf2++ = r1 + s2;
    *ptr_buf2++ = s1 - r2;
    *ptr_buf2++ = r3 - s4;
    *ptr_buf2++ = s3 + r4;
    *ptr_buf2++ = r3 + s4;
    *ptr_buf2++ = s3 - r4;
    *ptr_buf2++ = r1 - s2;
    *ptr_buf2++ = s1 + r2;
    ptr_buf3 = ptr_buf1;
  }

  idx = 0;
  ixheaace_cfft_15_twiddle(ptr_out_fft3);

  for (i = 0; i < FFT5; i++) {
    xr_0 = ptr_out_fft3[0];
    xi_0 = ptr_out_fft3[1];

    xr_1 = ptr_out_fft3[10];
    xi_1 = ptr_out_fft3[11];

    xr_2 = ptr_out_fft3[20];
    xi_2 = ptr_out_fft3[21];

    x_01_r = (xr_0 + xr_1);
    x_01_i = (xi_0 + xi_1);

    add_r = (xr_1 + xr_2);
    add_i = (xi_1 + xi_2);

    sub_r = (xr_1 - xr_2);
    sub_i = (xi_1 - xi_2);

    p1 = add_r / 2;

    p2 = (sub_i * sin_mu_flt);
    p3 = (sub_r * sin_mu_flt);

    p4 = add_i / 2;

    temp = (xr_0 - p1);
    temp1 = (xi_0 + p3);
    temp2 = (xi_0 - p3);

    ptr_op[idx] = (x_01_r + xr_2);
    ptr_op[idx + 1] = (x_01_i + xi_2);

    idx = idx + 320;
    ptr_op[idx] = (temp + p2);
    ptr_op[idx + 1] = (temp2 - p4);

    idx = idx + 320;
    ptr_op[idx] = (temp - p2);
    ptr_op[idx + 1] = (temp1 - p4);
    ptr_out_fft3 += 2;
    idx = idx - 576;
  }
}

static VOID ixheaace_cfft_twiddle_mult(FLOAT32 *ptr_inp, FLOAT32 *ptr_op, WORD32 dim1,
                                       WORD32 dim2, const FLOAT32 *ptr_tw_flt,
                                       const FLOAT32 *ptr_tw_h_flt) {
  FLOAT32 accu1, accu2;
  WORD32 i, j;
  WORD32 step_val = (dim2 - 1) << 1;
  for (i = 0; i < dim2; i++) {
    ptr_op[0] = ptr_inp[0];
    ptr_op[1] = ptr_inp[1];
    ptr_op += 2;
    ptr_inp += 2;
  }

  for (j = 0; j < (dim1 - 1); j++) {
    ptr_op[0] = ptr_inp[0];
    ptr_op[1] = ptr_inp[1];
    ptr_inp += 2;
    ptr_op += 2;
    for (i = 0; i < (dim2 - 1); i++) {
      ixheaace_cplx_mult_twid(&accu1, &accu2, ptr_inp[2 * i + 0], ptr_inp[2 * i + 1],
                              ptr_tw_flt[i], ptr_tw_h_flt[i]);
      ptr_op[2 * i + 0] = accu1;
      ptr_op[2 * i + 1] = accu2;
    }
    ptr_inp += step_val;
    ptr_op += step_val;
    ptr_tw_flt += (dim2 - 1);
    ptr_tw_h_flt += (dim2 - 1);
  }
}

static VOID ixheaace_cfft_32_480(FLOAT32 *ptr_in, FLOAT32 *ptr_out) {
  WORD32 i, l1, l2, h2;
  FLOAT32 xh0_0, xh1_0, xl0_0, xl1_0;
  FLOAT32 xh0_1, xh1_1, xl0_1, xl1_1;
  FLOAT32 x_0, x_1, x_2, x_3;
  FLOAT32 x_4, x_5, x_6, x_7;
  FLOAT32 *ptr_x;
  FLOAT32 *ptr_y;
  FLOAT32 interm_y[FFT32X2];
  FLOAT32 n00, n10, n20, n30, n01, n11, n21, n31;

  FLOAT32 inp_0qi, inp_0qr;
  FLOAT32 inp_1qi, inp_1qr;
  FLOAT32 inp_2qi, inp_2qr;
  FLOAT32 inp_3qi, inp_3qr;
  FLOAT32 mul_0qi, mul_0qr;
  FLOAT32 mul_1qi, mul_1qr;
  FLOAT32 mul_2qi, mul_2qr;
  FLOAT32 mul_3qi, mul_3qr;
  FLOAT32 sum_0qi, sum_0qr;
  FLOAT32 sum_1qi, sum_1qr;
  FLOAT32 sum_2qi, sum_2qr;
  FLOAT32 sum_3qi, sum_3qr;
  WORD32 idx1 = 0, idx2 = FFT15 * FFT32;
  FLOAT32 mul_i, mul_r;

  ptr_x = ptr_in;

  // This computes first and second stage butterflies. So, 4-point FFT is done.
  for (i = 0; i < 8; i++) {
    x_0 = ptr_x[0];
    x_1 = ptr_x[1];
    x_2 = ptr_x[16];
    x_3 = ptr_x[16 + 1];
    x_4 = ptr_x[32];
    x_5 = ptr_x[32 + 1];
    x_6 = ptr_x[48];
    x_7 = ptr_x[48 + 1];

    xh0_0 = x_0 + x_4;
    xh1_0 = x_1 + x_5;
    xl0_0 = x_0 - x_4;
    xl1_0 = x_1 - x_5;
    xh0_1 = x_2 + x_6;
    xh1_1 = x_3 + x_7;
    xl0_1 = x_2 - x_6;
    xl1_1 = x_3 - x_7;

    n00 = xh0_0 + xh0_1;
    n01 = xh1_0 + xh1_1;
    n10 = xl0_0 + xl1_1;
    n11 = xl1_0 - xl0_1;
    n20 = xh0_0 - xh0_1;
    n21 = xh1_0 - xh1_1;
    n30 = xl0_0 - xl1_1;
    n31 = xl1_0 + xl0_1;

    ptr_x[0] = n00;
    ptr_x[1] = n01;
    ptr_x[16] = n10;
    ptr_x[16 + 1] = n11;
    ptr_x[32] = n20;
    ptr_x[32 + 1] = n21;
    ptr_x[48] = n30;
    ptr_x[48 + 1] = n31;

    ptr_x += 2;
  }

  // This computes third and fourth stage butterflies. So, next 4-point FFT is done.
  {
    h2 = 16 >> 1;
    l1 = 16;
    l2 = 16 + (16 >> 1);

    ptr_x = ptr_in;
    ptr_y = &interm_y[0];

    /* Butter fly summation in 2 steps */
    inp_0qr = ptr_x[0];
    inp_0qi = ptr_x[1];
    inp_1qr = ptr_x[4];
    inp_1qi = ptr_x[5];
    inp_2qr = ptr_x[8];
    inp_2qi = ptr_x[9];
    inp_3qr = ptr_x[12];
    inp_3qi = ptr_x[13];

    mul_0qr = inp_0qr;
    mul_0qi = inp_0qi;
    mul_1qr = inp_1qr;
    mul_1qi = inp_1qi;
    mul_2qr = inp_2qr;
    mul_2qi = inp_2qi;
    mul_3qr = inp_3qr;
    mul_3qi = inp_3qi;

    sum_0qr = mul_0qr + mul_2qr;
    sum_0qi = mul_0qi + mul_2qi;
    sum_1qr = mul_0qr - mul_2qr;
    sum_1qi = mul_0qi - mul_2qi;
    sum_2qr = mul_1qr + mul_3qr;
    sum_2qi = mul_1qi + mul_3qi;
    sum_3qr = mul_1qr - mul_3qr;
    sum_3qi = mul_1qi - mul_3qi;

    ptr_y[0] = sum_0qr + sum_2qr;
    ptr_y[1] = sum_0qi + sum_2qi;
    ptr_y[h2] = sum_1qr + sum_3qi;
    ptr_y[h2 + 1] = sum_1qi - sum_3qr;
    ptr_y[l1] = sum_0qr - sum_2qr;
    ptr_y[l1 + 1] = sum_0qi - sum_2qi;
    ptr_y[l2] = sum_1qr - sum_3qi;
    ptr_y[l2 + 1] = sum_1qi + sum_3qr;

    ptr_y += 2;
    ptr_x += 16;

    /* 2nd butter fly */

    inp_0qr = ptr_x[0];
    inp_0qi = ptr_x[1];
    inp_1qr = ptr_x[4];
    inp_1qi = ptr_x[5];
    inp_2qr = ptr_x[8];
    inp_2qi = ptr_x[9];
    inp_3qr = ptr_x[12];
    inp_3qi = ptr_x[13];

    mul_0qr = inp_0qr;
    mul_0qi = inp_0qi;

    mul_1qr = (inp_1qr * 0.461929321f) + (inp_1qi * 0.191329956f);
    mul_1qi = (inp_1qr * -0.191329956f) + (inp_1qi * 0.461929321f);

    mul_2qr = ((inp_2qr + inp_2qi) * 0.353546143f);
    mul_2qi = ((-inp_2qr + inp_2qi) * 0.353546143f);

    mul_3qr = (inp_3qr * 0.191329956f) + (inp_3qi * 0.461929321f);
    mul_3qi = (inp_3qr * -0.461929321f) + (inp_3qi * 0.191329956f);

    sum_0qr = mul_0qr + (mul_2qr * 2);
    sum_0qi = mul_0qi + (mul_2qi * 2);
    sum_1qr = mul_0qr - (mul_2qr * 2);
    sum_1qi = mul_0qi - (mul_2qi * 2);

    sum_2qr = mul_1qr + mul_3qr;
    sum_2qi = mul_1qi + mul_3qi;
    sum_3qr = mul_1qr - mul_3qr;
    sum_3qi = mul_1qi - mul_3qi;

    ptr_y[0] = sum_0qr + (sum_2qr * 2);
    ptr_y[1] = sum_0qi + (sum_2qi * 2);
    ptr_y[h2] = sum_1qr + (sum_3qi * 2);
    ptr_y[h2 + 1] = sum_1qi - (sum_3qr * 2);
    ptr_y[l1] = sum_0qr - (sum_2qr * 2);
    ptr_y[l1 + 1] = sum_0qi - (sum_2qi * 2);
    ptr_y[l2] = sum_1qr - (sum_3qi * 2);
    ptr_y[l2 + 1] = sum_1qi + (sum_3qr * 2);

    ptr_y += 2;
    ptr_x += 16;

    /* 3rd butter fly */

    inp_0qr = ptr_x[0];
    inp_0qi = ptr_x[1];
    inp_1qr = ptr_x[4];
    inp_1qi = ptr_x[5];
    inp_2qr = ptr_x[8];
    inp_2qi = ptr_x[9];
    inp_3qr = ptr_x[12];
    inp_3qi = ptr_x[13];

    mul_0qr = inp_0qr;
    mul_0qi = inp_0qi;

    mul_1qr = ((inp_1qr + inp_1qi) * 0.353546143f);
    mul_1qi = ((-inp_1qr + inp_1qi) * 0.353546143f);

    mul_2qr = inp_2qi;
    mul_2qi = inp_2qr;

    mul_3qr = ((-inp_3qr + inp_3qi) * 0.353546143f);
    mul_3qi = ((inp_3qr + inp_3qi) * -0.353546143f);

    sum_0qr = mul_0qr + mul_2qr;
    sum_0qi = mul_0qi - mul_2qi;
    sum_1qr = mul_0qr - mul_2qr;
    sum_1qi = mul_0qi + mul_2qi;
    sum_2qr = mul_1qr + mul_3qr;
    sum_2qi = mul_1qi + mul_3qi;
    sum_3qr = mul_1qr - mul_3qr;
    sum_3qi = mul_1qi - mul_3qi;

    ptr_y[0] = sum_0qr + (sum_2qr * 2);
    ptr_y[1] = sum_0qi + (sum_2qi * 2);
    ptr_y[h2] = sum_1qr + (sum_3qi * 2);
    ptr_y[h2 + 1] = sum_1qi - (sum_3qr * 2);
    ptr_y[l1] = sum_0qr - (sum_2qr * 2);
    ptr_y[l1 + 1] = sum_0qi - (sum_2qi * 2);
    ptr_y[l2] = sum_1qr - (sum_3qi * 2);
    ptr_y[l2 + 1] = sum_1qi + (sum_3qr * 2);

    ptr_y += 2;
    ptr_x += 16;

    /* 4th butter fly */

    inp_0qr = ptr_x[0];
    inp_0qi = ptr_x[1];
    inp_1qr = ptr_x[4];
    inp_1qi = ptr_x[5];
    inp_2qr = ptr_x[8];
    inp_2qi = ptr_x[9];
    inp_3qr = ptr_x[12];
    inp_3qi = ptr_x[13];

    mul_0qr = inp_0qr;
    mul_0qi = inp_0qi;

    mul_1qr = (inp_1qr * 0.191329956f) + (inp_1qi * 0.461929321f);
    mul_1qi = (inp_1qr * -0.461929321f) + (inp_1qi * 0.191329956f);

    mul_2qr = ((-inp_2qr + inp_2qi) * 0.353546143f);
    mul_2qi = ((inp_2qr + inp_2qi) * -0.353546143f);

    mul_3qr = (inp_3qr * -0.461929321f) + (inp_3qi * -0.191329956f);
    mul_3qi = (inp_3qr * 0.191329956f) + (inp_3qi * -0.461929321f);

    sum_0qr = mul_0qr + (mul_2qr * 2);
    sum_0qi = mul_0qi + (mul_2qi * 2);
    sum_1qr = mul_0qr - (mul_2qr * 2);
    sum_1qi = mul_0qi - (mul_2qi * 2);

    sum_2qr = mul_1qr + mul_3qr;
    sum_2qi = mul_1qi + mul_3qi;
    sum_3qr = mul_1qr - mul_3qr;
    sum_3qi = mul_1qi - mul_3qi;

    ptr_y[0] = sum_0qr + (sum_2qr * 2);
    ptr_y[1] = sum_0qi + (sum_2qi * 2);
    ptr_y[h2] = sum_1qr + (sum_3qi * 2);
    ptr_y[h2 + 1] = sum_1qi - (sum_3qr * 2);
    ptr_y[l1] = sum_0qr - (sum_2qr * 2);
    ptr_y[l1 + 1] = sum_0qi - (sum_2qi * 2);
    ptr_y[l2] = sum_1qr - (sum_3qi * 2);
    ptr_y[l2 + 1] = sum_1qi + (sum_3qr * 2);

    ptr_x = ptr_in;
    ptr_y = &interm_y[32];

    /* Butter fly summation in 2 steps */
    inp_0qr = ptr_x[2];
    inp_0qi = ptr_x[3];
    inp_1qr = ptr_x[6];
    inp_1qi = ptr_x[7];
    inp_2qr = ptr_x[10];
    inp_2qi = ptr_x[11];
    inp_3qr = ptr_x[14];
    inp_3qi = ptr_x[15];

    mul_0qr = inp_0qr;
    mul_0qi = inp_0qi;
    mul_1qr = inp_1qr;
    mul_1qi = inp_1qi;
    mul_2qr = inp_2qr;
    mul_2qi = inp_2qi;
    mul_3qr = inp_3qr;
    mul_3qi = inp_3qi;

    sum_0qr = mul_0qr + mul_2qr;
    sum_0qi = mul_0qi + mul_2qi;
    sum_1qr = mul_0qr - mul_2qr;
    sum_1qi = mul_0qi - mul_2qi;
    sum_2qr = mul_1qr + mul_3qr;
    sum_2qi = mul_1qi + mul_3qi;
    sum_3qr = mul_1qr - mul_3qr;
    sum_3qi = mul_1qi - mul_3qi;

    ptr_y[0] = sum_0qr + sum_2qr;
    ptr_y[1] = sum_0qi + sum_2qi;
    ptr_y[h2] = sum_1qr + sum_3qi;
    ptr_y[h2 + 1] = sum_1qi - sum_3qr;
    ptr_y[l1] = sum_0qr - sum_2qr;
    ptr_y[l1 + 1] = sum_0qi - sum_2qi;
    ptr_y[l2] = sum_1qr - sum_3qi;
    ptr_y[l2 + 1] = sum_1qi + sum_3qr;

    ptr_y += 2;
    ptr_x += 16;

    /* 2nd butter fly */

    inp_0qr = ptr_x[2];
    inp_0qi = ptr_x[3];
    inp_1qr = ptr_x[6];
    inp_1qi = ptr_x[7];
    inp_2qr = ptr_x[10];
    inp_2qi = ptr_x[11];
    inp_3qr = ptr_x[14];
    inp_3qi = ptr_x[15];

    mul_0qr = inp_0qr;
    mul_0qi = inp_0qi;

    mul_1qr = (inp_1qr * 0.461929321f) + (inp_1qi * 0.191329956f);
    mul_1qi = (inp_1qr * -0.191329956f) + (inp_1qi * 0.461929321f);

    mul_2qr = ((inp_2qr + inp_2qi) * 0.353546143f);
    mul_2qi = ((-inp_2qr + inp_2qi) * 0.353546143f);

    mul_3qr = (inp_3qr * 0.191329956f) + (inp_3qi * 0.461929321f);
    mul_3qi = (inp_3qr * -0.461929321f) + (inp_3qi * 0.191329956f);

    sum_0qr = mul_0qr + (mul_2qr * 2);
    sum_0qi = mul_0qi + (mul_2qi * 2);
    sum_1qr = mul_0qr - (mul_2qr * 2);
    sum_1qi = mul_0qi - (mul_2qi * 2);

    sum_2qr = mul_1qr + mul_3qr;
    sum_2qi = mul_1qi + mul_3qi;
    sum_3qr = mul_1qr - mul_3qr;
    sum_3qi = mul_1qi - mul_3qi;

    ptr_y[0] = sum_0qr + (sum_2qr * 2);
    ptr_y[1] = sum_0qi + (sum_2qi * 2);
    ptr_y[h2] = sum_1qr + (sum_3qi * 2);
    ptr_y[h2 + 1] = sum_1qi - (sum_3qr * 2);
    ptr_y[l1] = sum_0qr - (sum_2qr * 2);
    ptr_y[l1 + 1] = sum_0qi - (sum_2qi * 2);
    ptr_y[l2] = sum_1qr - (sum_3qi * 2);
    ptr_y[l2 + 1] = sum_1qi + (sum_3qr * 2);

    ptr_y += 2;
    ptr_x += 16;

    /* 3rd butter fly */

    inp_0qr = ptr_x[2];
    inp_0qi = ptr_x[3];
    inp_1qr = ptr_x[6];
    inp_1qi = ptr_x[7];
    inp_2qr = ptr_x[10];
    inp_2qi = ptr_x[11];
    inp_3qr = ptr_x[14];
    inp_3qi = ptr_x[15];

    mul_0qr = inp_0qr;
    mul_0qi = inp_0qi;

    mul_1qr = ((inp_1qr + inp_1qi) * 0.353546143f);
    mul_1qi = ((-inp_1qr + inp_1qi) * 0.353546143f);

    mul_2qr = inp_2qi;
    mul_2qi = inp_2qr;

    mul_3qr = ((-inp_3qr + inp_3qi) * 0.353546143f);
    mul_3qi = ((inp_3qr + inp_3qi) * -0.353546143f);

    sum_0qr = mul_0qr + mul_2qr;
    sum_0qi = mul_0qi - mul_2qi;
    sum_1qr = mul_0qr - mul_2qr;
    sum_1qi = mul_0qi + mul_2qi;
    sum_2qr = mul_1qr + mul_3qr;
    sum_2qi = mul_1qi + mul_3qi;
    sum_3qr = mul_1qr - mul_3qr;
    sum_3qi = mul_1qi - mul_3qi;

    ptr_y[0] = sum_0qr + (sum_2qr * 2);
    ptr_y[1] = sum_0qi + (sum_2qi * 2);
    ptr_y[h2] = sum_1qr + (sum_3qi * 2);
    ptr_y[h2 + 1] = sum_1qi - (sum_3qr * 2);
    ptr_y[l1] = sum_0qr - (sum_2qr * 2);
    ptr_y[l1 + 1] = sum_0qi - (sum_2qi * 2);
    ptr_y[l2] = sum_1qr - (sum_3qi * 2);
    ptr_y[l2 + 1] = sum_1qi + (sum_3qr * 2);

    ptr_y += 2;
    ptr_x += 16;

    /* 4th butter fly */

    inp_0qr = ptr_x[2];
    inp_0qi = ptr_x[3];
    inp_1qr = ptr_x[6];
    inp_1qi = ptr_x[7];
    inp_2qr = ptr_x[10];
    inp_2qi = ptr_x[11];
    inp_3qr = ptr_x[14];
    inp_3qi = ptr_x[15];

    mul_0qr = inp_0qr;
    mul_0qi = inp_0qi;

    mul_1qr = (inp_1qr * 0.191329956f) + (inp_1qi * 0.461929321f);
    mul_1qi = (inp_1qr * -0.461929321f) + (inp_1qi * 0.191329956f);

    mul_2qr = ((-inp_2qr + inp_2qi) * 0.353546143f);
    mul_2qi = ((inp_2qr + inp_2qi) * -0.353546143f);

    mul_3qr = (inp_3qr * -0.461929321f) + (inp_3qi * -0.191329956f);
    mul_3qi = (inp_3qr * 0.191329956f) + (inp_3qi * -0.461929321f);

    sum_0qr = mul_0qr + (mul_2qr * 2);
    sum_0qi = mul_0qi + (mul_2qi * 2);
    sum_1qr = mul_0qr - (mul_2qr * 2);
    sum_1qi = mul_0qi - (mul_2qi * 2);

    sum_2qr = mul_1qr + mul_3qr;
    sum_2qi = mul_1qi + mul_3qi;
    sum_3qr = mul_1qr - mul_3qr;
    sum_3qi = mul_1qi - mul_3qi;

    ptr_y[0] = sum_0qr + (sum_2qr * 2);
    ptr_y[1] = sum_0qi + (sum_2qi * 2);
    ptr_y[h2] = sum_1qr + (sum_3qi * 2);
    ptr_y[h2 + 1] = sum_1qi - (sum_3qr * 2);
    ptr_y[l1] = sum_0qr - (sum_2qr * 2);
    ptr_y[l1 + 1] = sum_0qi - (sum_2qi * 2);
    ptr_y[l2] = sum_1qr - (sum_3qi * 2);
    ptr_y[l2 + 1] = sum_1qi + (sum_3qr * 2);
  }

  // Last stage of 32 point FFT
  {
    ptr_y = ptr_out;
    ptr_y[idx1] = interm_y[0] + interm_y[32];
    ptr_y[idx1 + 1] = interm_y[1] + interm_y[33];
    ptr_y[idx2] = interm_y[0] - interm_y[32];
    ptr_y[idx2 + 1] = interm_y[1] - interm_y[33];
    idx1 += FFT15X2;
    idx2 += FFT15X2;
    for (i = 1; i < FFT16; i++) {
      mul_r = (interm_y[FFT32 + 2 * i + 0] * ixheaace_fft_mix_rad_twid_tbl_32[i - 1]) -
              (interm_y[FFT32 + 2 * i + 1] * ixheaace_fft_mix_rad_twid_tbl_h_32[i - 1]);
      mul_i = (interm_y[FFT32 + 2 * i + 0] * ixheaace_fft_mix_rad_twid_tbl_h_32[i - 1]) +
              (interm_y[FFT32 + 2 * i + 1] * ixheaace_fft_mix_rad_twid_tbl_32[i - 1]);

      mul_r = mul_r / 2;
      mul_i = mul_i / 2;
      ptr_y[idx1] = interm_y[2 * i + 0] + (mul_r * 2);
      ptr_y[idx1 + 1] = interm_y[2 * i + 1] + (mul_i * 2);
      ptr_y[idx2] = interm_y[2 * i + 0] - (mul_r * 2);
      ptr_y[idx2 + 1] = interm_y[2 * i + 1] - (mul_i * 2);
      idx1 += FFT15X2;
      idx2 += FFT15X2;
    }
  }
}

static VOID ixheaace_dec_rearrange_short_flt(FLOAT32 *ptr_in, FLOAT32 *ptr_out, WORD32 N,
                                             const WORD16 *ptr_re_arr_tab) {
  WORD32 n, i = 0;

  for (n = 0; n < N; n++) {
    WORD32 idx = ptr_re_arr_tab[n] << 1;
    ptr_out[i++] = ptr_in[idx];
    ptr_out[i++] = ptr_in[idx + 1];
  }
}

static VOID ixheaace_fft_5_flt(FLOAT32 *ptr_in, FLOAT32 *ptr_out) {
  FLOAT32 C51 = 0.951056516f;
  FLOAT32 C52 = -0.769420885f;
  FLOAT32 C53 = -0.363271264f;
  FLOAT32 C54 = 0.559016994f;
  FLOAT32 C55 = -0.625f;

  FLOAT32 r1, r2, r3, r4;
  FLOAT32 s1, s2, s3, s4, t, temp1, temp2;

  r1 = (ptr_in[2] + ptr_in[8]);
  r4 = (ptr_in[2] - ptr_in[8]);
  r3 = (ptr_in[4] + ptr_in[6]);
  r2 = (ptr_in[4] - ptr_in[6]);

  t = ((r1 - r3) * C54);
  r1 = (r1 + r3);

  temp1 = (ptr_in[0] + r1);
  r1 = (temp1 + (((r1 * C55)) * 2));

  r3 = (r1 - t);
  r1 = (r1 + t);

  t = ((r4 + r2) * C51);
  r4 = (t + ((r4 * C52) * 2));
  r2 = (t + (r2 * C53));

  s1 = (ptr_in[3] + ptr_in[9]);
  s4 = (ptr_in[3] - ptr_in[9]);
  s3 = (ptr_in[5] + ptr_in[7]);
  s2 = (ptr_in[5] - ptr_in[7]);

  t = ((s1 - s3) * C54);
  s1 = (s1 + s3);

  temp2 = (ptr_in[1] + s1);

  s1 = (temp2 + (((s1 * C55)) * 2));

  s3 = (s1 - t);
  s1 = (s1 + t);

  t = ((s4 + s2) * C51);
  s4 = (t + (((s4 * C52)) * 2));
  s2 = (t + ((s2 * C53)));

  ptr_out[0] = temp1;
  ptr_out[1] = temp2;
  ptr_out[2] = (r1 + s2);
  ptr_out[3] = (s1 - r2);
  ptr_out[4] = (r3 - s4);
  ptr_out[5] = (s3 + r4);
  ptr_out[6] = (r3 + s4);
  ptr_out[7] = (s3 - r4);
  ptr_out[8] = (r1 - s2);
  ptr_out[9] = (s1 + r2);
}

static VOID ixheaace_fft_3_flt(FLOAT32 *ptr_in, FLOAT32 *ptr_out) {
  FLOAT32 add_r, sub_r;
  FLOAT32 add_i, sub_i;
  FLOAT32 x_01_r, x_01_i, temp;

  FLOAT32 p1, p2, p3, p4;
  FLOAT32 sinmu = 0.866025404f;

  x_01_r = (ptr_in[0] + ptr_in[2]);
  x_01_i = (ptr_in[1] + ptr_in[3]);

  add_r = (ptr_in[2] + ptr_in[4]);
  add_i = (ptr_in[3] + ptr_in[5]);

  sub_r = (ptr_in[2] - ptr_in[4]);
  sub_i = (ptr_in[3] - ptr_in[5]);

  p1 = add_r / 2;
  p2 = (sub_i * sinmu);
  p3 = (sub_r * sinmu);
  p4 = add_i / 2;

  temp = (ptr_in[0] - p1);

  ptr_out[0] = (x_01_r + ptr_in[4]);
  ptr_out[1] = (x_01_i + ptr_in[5]);
  ptr_out[2] = (temp + p2);
  ptr_out[3] = ((ptr_in[1] - p3) - p4);
  ptr_out[4] = (temp - p2);
  ptr_out[5] = ((ptr_in[1] + p3) - p4);
}

static VOID ixheaace_pre_twiddle_120(FLOAT32 *ptr_in, FLOAT32 *ptr_data, WORD32 n,
                                     const FLOAT32 *ptr_cos_sin_tbl) {
  WORD npoints_4, i;
  FLOAT32 tempr, tempi, temp;
  FLOAT32 c, c1, s, s1;
  FLOAT32 *ptr_in1, *ptr_in2;
  FLOAT32 *ptr_x = ptr_in + (n - 1);

  npoints_4 = n >> 2;

  ptr_in1 = ptr_data;
  ptr_in2 = ptr_data + n - 1;

  for (i = 0; i < npoints_4; i++) {
    c = *ptr_cos_sin_tbl++;
    s = *ptr_cos_sin_tbl++;

    tempr = *ptr_in1++;
    tempi = *ptr_in2--;

    temp = -((tempr * c) + (tempi * s));
    *ptr_in++ = temp;

    temp = -((tempi * c) - (tempr * s));
    *ptr_in++ = temp;

    c1 = *ptr_cos_sin_tbl++;
    s1 = *ptr_cos_sin_tbl++;

    tempi = *ptr_in1++;
    tempr = *ptr_in2--;

    temp = -((tempi * c1) - (tempr * s1));
    *ptr_x-- = temp;

    temp = -((tempr * c1) + (tempi * s1));
    *ptr_x-- = temp;
  }
}

static VOID ixheaace_post_twiddle_120(FLOAT32 *ptr_out, FLOAT32 *ptr_x,
                                      const FLOAT32 *ptr_cos_sin_tbl, WORD m) {
  WORD i;
  FLOAT32 c, c1, s, s1;
  FLOAT32 tempr, tempi, temp;
  FLOAT32 *ptr_in2 = ptr_x + (m - 1);
  FLOAT32 *ptr_in1 = ptr_x;
  FLOAT32 *ptr_x1 = ptr_out;
  FLOAT32 *ptr_x2 = ptr_out + (m - 1);

  for (i = 0; i < m; i += 4) {
    c = *ptr_cos_sin_tbl++;
    s = *ptr_cos_sin_tbl++;
    c1 = *ptr_cos_sin_tbl++;
    s1 = *ptr_cos_sin_tbl++;

    tempr = *ptr_in1++;
    tempi = *ptr_in1++;

    temp = -((tempr * s) - (tempi * c));
    *ptr_x2-- = temp;

    temp = -((tempr * c) + (tempi * s));
    *ptr_x1++ = temp;

    tempi = *ptr_in2--;
    tempr = *ptr_in2--;

    temp = -((tempr * s1) - (tempi * c1));
    *ptr_x1++ = temp;

    temp = -((tempr * c1) + (tempi * s1));
    *ptr_x2-- = temp;
  }
}

static VOID ixheaace_fft_960_15(FLOAT32 *ptr_in_flt, FLOAT32 *ptr_out_flt) {
  WORD32 i;
  FLOAT32 *ptr_buf1_flt, *ptr_buf2_flt;
  ixheaace_dec_rearrange_short_flt(ptr_in_flt, ptr_out_flt, FFT15, re_arr_tab_5);

  ptr_buf1_flt = ptr_out_flt;
  ptr_buf2_flt = ptr_in_flt;
  for (i = 0; i < FFT3; i++) {
    ixheaace_fft_5_flt(ptr_buf1_flt, ptr_buf2_flt);

    ptr_buf1_flt += (FFT5 * 2);
    ptr_buf2_flt += (FFT5 * 2);
  }

  ixheaace_dec_rearrange_short_flt(ptr_in_flt, ptr_out_flt, FFT15, re_arr_tab_3);
  ptr_buf1_flt = ptr_out_flt;
  ptr_buf2_flt = ptr_in_flt;
  for (i = 0; i < FFT5; i++) {
    ixheaace_fft_3_flt(ptr_buf1_flt, ptr_buf2_flt);

    ptr_buf1_flt += (FFT3 * 2);
    ptr_buf2_flt += (FFT3 * 2);
  }

  ixheaace_dec_rearrange_short_flt(ptr_in_flt, ptr_out_flt, FFT15, re_arr_tab_sml);
}

static VOID ixheaace_fft_120(WORD32 npoints, FLOAT32 *ptr_x_flt, FLOAT32 *ptr_y_flt) {
  WORD32 i;
  FLOAT32 *ptr_buf1_flt, *ptr_buf2_flt;
  FLOAT32 *ptr_in_flt, *ptr_out_flt;

  ptr_in_flt = ptr_x_flt;
  ptr_out_flt = ptr_y_flt;
  ixheaace_dec_rearrange_short_flt(ptr_in_flt, ptr_out_flt, 60, re_arr_tab_4);

  ptr_buf1_flt = ptr_out_flt;
  ptr_buf2_flt = ptr_in_flt;

  for (i = 0; i < FFT15; i++) {
    {
      FLOAT32 x_0, x_1, x_2, x_3, x_4, x_5, x_6, x_7;
      FLOAT32 *y0, *y1, *y2, *y3;
      FLOAT32 *x0;
      FLOAT32 xh0_0, xh1_0, xh0_1, xh1_1, xl0_0, xl1_0, xl0_1, xl1_1;
      WORD32 h2;
      FLOAT32 n00, n01, n10, n11, n20, n21, n30, n31;

      ptr_x_flt = ptr_buf1_flt;
      ptr_y_flt = ptr_buf2_flt;
      npoints = 4;
      h2 = 0;

      y0 = ptr_y_flt;
      y2 = ptr_y_flt + (WORD32)npoints;
      x0 = ptr_x_flt;
      y1 = y0 + (WORD32)(npoints >> 1);
      y3 = y2 + (WORD32)(npoints >> 1);

      x_0 = x0[0];
      x_1 = x0[1];
      x_2 = x0[2];
      x_3 = x0[3];
      x_4 = x0[4];
      x_5 = x0[5];
      x_6 = x0[6];
      x_7 = x0[7];

      xh0_0 = x_0 + x_4;
      xh1_0 = x_1 + x_5;
      xl0_0 = x_0 - x_4;
      xl1_0 = x_1 - x_5;
      xh0_1 = x_2 + x_6;
      xh1_1 = x_3 + x_7;
      xl0_1 = x_2 - x_6;
      xl1_1 = x_3 - x_7;

      n00 = xh0_0 + xh0_1;
      n01 = xh1_0 + xh1_1;
      n10 = xl0_0 + xl1_1;
      n11 = xl1_0 - xl0_1;
      n20 = xh0_0 - xh0_1;
      n21 = xh1_0 - xh1_1;
      n30 = xl0_0 - xl1_1;
      n31 = xl1_0 + xl0_1;

      y0[2 * h2] = n00;
      y0[2 * h2 + 1] = n01;
      y1[2 * h2] = n10;
      y1[2 * h2 + 1] = n11;
      y2[2 * h2] = n20;
      y2[2 * h2 + 1] = n21;
      y3[2 * h2] = n30;
      y3[2 * h2 + 1] = n31;
    }

    ptr_buf1_flt += (FFT4 * 2);
    ptr_buf2_flt += (FFT4 * 2);
  }

  ixheaace_dec_rearrange_short_flt(ptr_in_flt, ptr_out_flt, 60, re_arr_tab_15_4);

  ptr_buf1_flt = ptr_out_flt;
  ptr_buf2_flt = ptr_in_flt;
  for (i = 0; i < FFT4; i++) {
    ixheaace_fft_960_15(ptr_buf1_flt, ptr_buf2_flt);
    ptr_buf1_flt += (FFT15 * 2);
    ptr_buf2_flt += (FFT15 * 2);
  }

  ixheaace_dec_rearrange_short_flt(ptr_in_flt, ptr_out_flt, 60, re_arr_tab_120);
}

static VOID ixheaace_cfft_480(FLOAT32 *ptr_inp, FLOAT32 *ptr_op) {
  WORD32 i;
  FLOAT32 *ptr_buf1, *ptr_buf2;
  FLOAT32 fft5_out[FFT15X2] = {0};

  ptr_buf1 = ptr_inp;
  ptr_buf2 = ptr_op;

  for (i = 0; i < FFT32; i++) {
    ixheaace_cfft_15_480(ptr_buf1, ptr_buf2, &fft5_out[0]);
    ptr_buf1 += 2;
    ptr_buf2 += 2;
  }

  ixheaace_cfft_twiddle_mult(ptr_op, ptr_inp, FFT15, FFT32, ixheaace_fft_mix_rad_twid_tbl_480,
                             ixheaace_fft_mix_rad_twid_h_tbl_480);

  ptr_buf1 = ptr_inp;
  ptr_buf2 = ptr_op;

  for (i = 0; i < FFT15; i++) {
    ixheaace_cfft_32_480(ptr_buf1, ptr_buf2);
    ptr_buf1 += (FFT32X2);
    ptr_buf2 += 2;
  }
}

static VOID ixheaace_pre_twiddle_960(FLOAT32 *ptr_x, FLOAT32 *ptr_data, WORD32 n,
                                     const FLOAT32 *ptr_cos_sin_tbl) {
  WORD npoints_4, i;
  FLOAT32 tempr, tempi, temp;
  FLOAT32 c, c1, s, s1;
  FLOAT32 *ptr_in_1, *ptr_in_2;
  FLOAT32 *ptr_x_1 = ptr_x + (n - 1);

  npoints_4 = n >> 2;

  ptr_in_1 = ptr_data;
  ptr_in_2 = ptr_data + n - 1;

  for (i = 0; i < npoints_4; i++) {
    c = *ptr_cos_sin_tbl++;
    s = *ptr_cos_sin_tbl++;

    tempr = *ptr_in_1++;
    tempi = *ptr_in_2--;

    temp = -((tempr * c) + (tempi * s));
    *ptr_x++ = temp;

    temp = -((tempi * c) - (tempr * s));
    *ptr_x++ = temp;

    c1 = *ptr_cos_sin_tbl++;
    s1 = *ptr_cos_sin_tbl++;

    tempi = *ptr_in_1++;
    tempr = *ptr_in_2--;

    temp = -((tempi * c1) - (tempr * s1));
    *ptr_x_1-- = temp;

    temp = -((tempr * c1) + (tempi * s1));
    *ptr_x_1-- = temp;
  }
}

static VOID ixheaace_post_twiddle_960(FLOAT32 *ptr_out, FLOAT32 *ptr_x,
                                      const FLOAT32 *ptr_cos_sin_tbl, WORD m) {
  WORD i;
  FLOAT32 c, c1, s, s1;
  FLOAT32 tempr, tempi, temp;
  FLOAT32 *ptr_in2 = ptr_x + (m - 1);
  FLOAT32 *ptr_in1 = ptr_x;
  FLOAT32 *ptr_x1 = ptr_out;
  FLOAT32 *ptr_x2 = ptr_out + (m - 1);

  for (i = 0; i < m; i += 4) {
    c = *ptr_cos_sin_tbl++;
    s = *ptr_cos_sin_tbl++;
    c1 = *ptr_cos_sin_tbl++;
    s1 = *ptr_cos_sin_tbl++;

    tempr = *ptr_in1++;
    tempi = *ptr_in1++;

    temp = -((tempr * s) - (tempi * c));
    *ptr_x2-- = temp;

    temp = -((tempr * c) + (tempi * s));
    *ptr_x1++ = temp;

    tempi = *ptr_in2--;
    tempr = *ptr_in2--;

    temp = -((tempr * s1) - (tempi * c1));
    *ptr_x1++ = temp;

    temp = -((tempr * c1) + (tempi * s1));
    *ptr_x2-- = temp;
  }
}

static VOID ixheaace_mdct_960(FLOAT32 *ptr_input_flt, WORD8 *ptr_scratch) {
  FLOAT32 *ptr_scratch_flt = (FLOAT32 *)ptr_scratch;
  FLOAT32 const_mult_fac = 3.142857143f;
  FLOAT32 *ptr_data = ptr_input_flt;
  WORD32 k;

  memcpy(ptr_scratch_flt, ptr_input_flt, sizeof(*ptr_scratch_flt) * FRAME_LEN_960);
  ixheaace_pre_twiddle_960(ptr_input_flt, ptr_scratch_flt, FRAME_LEN_960, cos_sin_table_flt);

  ixheaace_cfft_480(ptr_input_flt, ptr_scratch_flt);

  ixheaace_post_twiddle_960(ptr_input_flt, ptr_scratch_flt, cos_sin_table_flt, FRAME_LEN_960);

  for (k = FRAME_LEN_960 - 1; k >= 0; k -= 2) {
    *ptr_data = (*ptr_data * const_mult_fac);
    ptr_data++;
    *ptr_data = (*ptr_data * const_mult_fac);
    ptr_data++;
  }
}

static VOID ixheaace_mdct_120(FLOAT32 *ptr_input_flt, WORD8 *ptr_scratch) {
  WORD32 n, k;
  WORD32 n_by_2;
  FLOAT32 *ptr_scratch_flt = (FLOAT32 *)ptr_scratch;
  FLOAT32 const_mltfac = 3.142857143f;
  FLOAT32 *ptr_data = ptr_input_flt;
  n = 120;
  n_by_2 = n >> 1;
  memcpy(ptr_scratch_flt, ptr_input_flt, sizeof(*ptr_scratch_flt) * n);

  ixheaace_pre_twiddle_120(ptr_input_flt, ptr_scratch_flt, n, ixheaace_cosine_array_240);

  ixheaace_fft_120(n_by_2, ptr_input_flt, ptr_scratch_flt);

  ixheaace_post_twiddle_120(ptr_input_flt, ptr_scratch_flt, ixheaace_cosine_array_240, n);

  for (k = n - 1; k >= 0; k -= 2) {
    *ptr_data = (*ptr_data * const_mltfac);
    ptr_data++;
    *ptr_data = (*ptr_data * const_mltfac);
    ptr_data++;
  }
}

static VOID ixheaace_mdct(FLOAT32 *ptr_dct_data, const FLOAT32 *ptr_trig_data,
                          const FLOAT32 *ptr_sine_window, WORD32 n, WORD32 ld_n,
                          WORD8 *ptr_scratch) {
  ixheaace_pre_mdct(ptr_dct_data, n, ptr_sine_window);

  ixheaace_scratch_mem *pstr_scratch = (ixheaace_scratch_mem *)ptr_scratch;
  ia_enhaacplus_enc_complex_fft(ptr_dct_data, n / 2, pstr_scratch);

  ixheaace_post_mdct(ptr_dct_data, n, ptr_trig_data,
                     1 << (LD_FFT_TWIDDLE_TABLE_SIZE - (ld_n - 1)), FFT_TWIDDLE_TABLE_SIZE);
}

static VOID ixheaace_shift_mdct_delay_buffer(FLOAT32 *ptr_mdct_delay_buffer,
                                             const FLOAT32 *ptr_time_signal, WORD32 ch_increment,
                                             WORD32 frame_len) {
  WORD32 i;
  WORD32 blk_switch_offset = frame_len;
  switch (frame_len) {
    case FRAME_LEN_1024:
      blk_switch_offset = BLK_SWITCH_OFFSET_LC_128;
      memmove(ptr_mdct_delay_buffer, ptr_mdct_delay_buffer + frame_len,
              (blk_switch_offset - frame_len) * sizeof(*ptr_mdct_delay_buffer));
      break;

    case FRAME_LEN_960:
      blk_switch_offset = BLK_SWITCH_OFFSET_LC_120;
      memmove(ptr_mdct_delay_buffer, ptr_mdct_delay_buffer + frame_len,
              (blk_switch_offset - frame_len) * sizeof(*ptr_mdct_delay_buffer));
      break;

    case FRAME_LEN_512:
    case FRAME_LEN_480:
      blk_switch_offset = frame_len;
      break;
  }

  for (i = 0; i < frame_len; i++) {
    ptr_mdct_delay_buffer[blk_switch_offset - frame_len + i] = ptr_time_signal[i * ch_increment];
  }
}

VOID ixheaace_transform_real_lc_ld(FLOAT32 *ptr_mdct_delay_buffer, const FLOAT32 *ptr_time_signal,
                                   WORD32 ch_increment, FLOAT32 *ptr_real_out, WORD32 block_type,
                                   WORD32 frame_len, WORD8 *ptr_scratch) {
  WORD32 i, w;
  FLOAT32 ws1, ws2;
  FLOAT32 *ptr_dct_in;
  WORD32 frame_len_short = FRAME_LEN_SHORT_128;
  WORD32 ls_trans = LS_TRANS_128;
  WORD32 trans_offset = TRANSFORM_OFFSET_SHORT_128;
  const FLOAT32 *ptr_window;
  if (frame_len == FRAME_LEN_960) {
    ls_trans = LS_TRANS_120;
    trans_offset = TRANSFORM_OFFSET_SHORT_120;
    frame_len_short = FRAME_LEN_SHORT_120;
  }
  switch (block_type) {
    case LONG_WINDOW:
      ptr_dct_in = ptr_real_out;
      ptr_window = &long_window_KBD[0];
      switch (frame_len) {
        case FRAME_LEN_1024:
          ptr_window = &long_window_KBD[0];
          break;

        case FRAME_LEN_960:
          ptr_window = &long_window_sine_960[0];
          break;

        case FRAME_LEN_512:
          ptr_window = &long_window_sine_ld[0];
          break;

        case FRAME_LEN_480:
          ptr_window = &long_window_sine_ld_480[0];
          break;
      }
      for (i = 0; i < frame_len / 2; i++) {
        ws1 = ptr_mdct_delay_buffer[i] * ptr_window[i];

        ws2 = ptr_mdct_delay_buffer[(frame_len - i - 1)] * ptr_window[frame_len - i - 1];

        ptr_dct_in[frame_len / 2 + i] = ws1 - ws2;
      }

      ixheaace_shift_mdct_delay_buffer(ptr_mdct_delay_buffer, ptr_time_signal, ch_increment,
                                       frame_len);

      for (i = 0; i < frame_len / 2; i++) {
        ws1 = ptr_mdct_delay_buffer[i] * ptr_window[frame_len - i - 1];

        ws2 = ptr_mdct_delay_buffer[(frame_len - i - 1)] * ptr_window[i];

        ptr_dct_in[frame_len / 2 - i - 1] = -(ws1 + ws2);
      }
      switch (frame_len) {
        case FRAME_LEN_1024:
          ixheaace_mdct(ptr_dct_in, fft_twiddle_tab, long_window_sine, frame_len, 10,
                        ptr_scratch);
          break;

        case FRAME_LEN_960:
          ixheaace_mdct_960(ptr_dct_in, ptr_scratch);
          break;

        case FRAME_LEN_512:
        case FRAME_LEN_480:
          ixheaace_mdct(ptr_dct_in, fft_twiddle_tab, ptr_window, frame_len, 9, ptr_scratch);
          break;
      }
      break;

    case START_WINDOW:
      ptr_dct_in = ptr_real_out;
      ptr_window = &long_window_KBD[0];
      switch (frame_len) {
        case FRAME_LEN_1024:
          ptr_window = &long_window_KBD[0];
          break;

        case FRAME_LEN_960:
          ptr_window = &long_window_sine_960[0];
          break;
      }
      for (i = 0; i < frame_len / 2; i++) {
        ws1 = ptr_mdct_delay_buffer[i] * ptr_window[i];

        ws2 = ptr_mdct_delay_buffer[(frame_len - i - 1)] * ptr_window[frame_len - i - 1];

        ptr_dct_in[frame_len / 2 + i] = ws1 - ws2;
      }

      ixheaace_shift_mdct_delay_buffer(ptr_mdct_delay_buffer, ptr_time_signal, ch_increment,
                                       frame_len);

      if (frame_len == FRAME_LEN_1024) {
        ptr_window = &short_window_sine[0];
      } else if (frame_len == FRAME_LEN_960) {
        ptr_window = &short_window_sine_120[0];
      }
      for (i = 0; i < ls_trans; i++) {
        ws1 = ptr_mdct_delay_buffer[i];
        ws2 = 0.0f;

        ptr_dct_in[frame_len / 2 - i - 1] = -(ws1 + ws2);
      }

      for (i = 0; i < frame_len_short / 2; i++) {
        ws1 = ptr_mdct_delay_buffer[i + ls_trans] * ptr_window[frame_len_short - i - 1];

        ws2 = ptr_mdct_delay_buffer[(frame_len - i - 1 - ls_trans)] * ptr_window[i];

        ptr_dct_in[frame_len / 2 - i - 1 - ls_trans] = -(ws1 + ws2);
      }
      if (frame_len == FRAME_LEN_960) {
        ixheaace_mdct_960(ptr_dct_in, ptr_scratch);
      } else {
        ixheaace_mdct(ptr_dct_in, fft_twiddle_tab, long_window_sine, frame_len, 10, ptr_scratch);
      }

      break;

    case STOP_WINDOW:
      ptr_window = &long_window_KBD[0];
      ptr_dct_in = ptr_real_out;
      if (frame_len == FRAME_LEN_1024) {
        ptr_window = &short_window_sine[0];
      } else if (frame_len == FRAME_LEN_960) {
        ptr_window = &short_window_sine_120[0];
      }
      for (i = 0; i < ls_trans; i++) {
        ws1 = 0.0f;
        ws2 = ptr_mdct_delay_buffer[(frame_len - i - 1)];
        ptr_dct_in[frame_len / 2 + i] = ws1 - ws2;
      }

      for (i = 0; i < frame_len_short / 2; i++) {
        ws1 = ptr_mdct_delay_buffer[(i + ls_trans)] * ptr_window[i];

        ws2 = ptr_mdct_delay_buffer[(frame_len - ls_trans - i - 1)] *
              ptr_window[frame_len_short - i - 1];

        ptr_dct_in[frame_len / 2 + i + ls_trans] = ws1 - ws2;
      }

      ixheaace_shift_mdct_delay_buffer(ptr_mdct_delay_buffer, ptr_time_signal, ch_increment,
                                       frame_len);

      if (frame_len == FRAME_LEN_1024) {
        ptr_window = &long_window_KBD[0];
      } else if (frame_len == FRAME_LEN_960) {
        ptr_window = &long_window_sine_960[0];
      }
      for (i = 0; i < frame_len / 2; i++) {
        ws1 = ptr_mdct_delay_buffer[i] * ptr_window[frame_len - i - 1];

        ws2 = ptr_mdct_delay_buffer[(frame_len - i - 1)] * ptr_window[i];

        ptr_dct_in[frame_len / 2 - i - 1] = -(ws1 + ws2);
      }

      if (frame_len == FRAME_LEN_960) {
        ixheaace_mdct_960(ptr_dct_in, ptr_scratch);
      } else {
        ixheaace_mdct(ptr_dct_in, fft_twiddle_tab, long_window_sine, frame_len, 10, ptr_scratch);
      }

      break;

    case SHORT_WINDOW:
      ptr_window = &short_window_sine[0];
      if (frame_len == FRAME_LEN_1024) {
        ptr_window = &short_window_sine[0];
      } else if (frame_len == FRAME_LEN_960) {
        ptr_window = &short_window_sine_120[0];
      }
      for (w = 0; w < TRANS_FAC; w++) {
        ptr_dct_in = ptr_real_out + w * frame_len_short;

        for (i = 0; i < frame_len_short / 2; i++) {
          ws1 = ptr_mdct_delay_buffer[trans_offset + w * frame_len_short + i] * ptr_window[i];

          ws2 = ptr_mdct_delay_buffer[trans_offset + w * frame_len_short + frame_len_short - i -
                                      1] *
                ptr_window[frame_len_short - i - 1];

          ptr_dct_in[frame_len_short / 2 + i] = ws1 - ws2;

          ws1 = ptr_mdct_delay_buffer[trans_offset + w * frame_len_short + frame_len_short + i] *
                ptr_window[frame_len_short - i - 1];

          ws2 = ptr_mdct_delay_buffer[trans_offset + w * frame_len_short + frame_len_short * 2 -
                                      i - 1] *
                ptr_window[i];

          ptr_dct_in[frame_len_short / 2 - i - 1] = -(ws1 + ws2);
        }
        if (frame_len == FRAME_LEN_960) {
          ixheaace_mdct_120(ptr_dct_in, ptr_scratch);
        } else {
          ixheaace_mdct(ptr_dct_in, fft_twiddle_tab, short_window_sine, frame_len_short, 7,
                        ptr_scratch);
        }
      }

      ixheaace_shift_mdct_delay_buffer(ptr_mdct_delay_buffer, ptr_time_signal, ch_increment,
                                       frame_len);
      break;
  }
}

VOID ia_enhaacplus_enc_transform_real_eld(FLOAT32 *ptr_mdct_delay_buffer,
                                          const FLOAT32 *ptr_time_signal, WORD32 ch_increment,
                                          FLOAT32 *ptr_real_out, WORD8 *ptr_shared_buffer5,
                                          WORD32 frame_len) {
  WORD32 i, loop_len;
  FLOAT32 w1, w2;
  FLOAT32 *ptr_curr_data, *ptr_prev1_data, *ptr_prev2_data, *ptr_prev3_data;
  const FLOAT32 *ptr_win0, *ptr_win1, *ptr_win2, *ptr_win3;

  loop_len = frame_len / 4;

  ptr_curr_data = &ptr_mdct_delay_buffer[3 * frame_len];
  ptr_prev1_data = &ptr_mdct_delay_buffer[2 * frame_len];
  ptr_prev2_data = &ptr_mdct_delay_buffer[frame_len];
  ptr_prev3_data = &ptr_mdct_delay_buffer[0];

  ptr_win0 = &low_delay_window_eld[0];
  ptr_win1 = &low_delay_window_eld[frame_len];
  ptr_win2 = &low_delay_window_eld[2 * frame_len];
  ptr_win3 = &low_delay_window_eld[3 * frame_len];

  memmove(&ptr_mdct_delay_buffer[0], &ptr_mdct_delay_buffer[frame_len],
          (3 * frame_len) * sizeof(ptr_mdct_delay_buffer[0]));

  for (i = 0; i < frame_len; i++) {
    ptr_curr_data[i] = ptr_time_signal[i * ch_increment];
  }

  for (i = 0; i < loop_len; i++) {
    w1 = ptr_prev3_data[(frame_len / 2) + loop_len + i] * ptr_win3[(frame_len / 2) - 1 - i];
    w1 += ptr_prev3_data[(frame_len / 2) + loop_len - 1 - i] * ptr_win3[(frame_len / 2) + i];

    w2 = (-ptr_prev1_data[(frame_len / 2) + loop_len + i] * ptr_win1[(frame_len / 2) - 1 - i]);
    w2 += (-ptr_prev1_data[(frame_len / 2) + loop_len - 1 - i] * ptr_win1[(frame_len / 2) + i]);

    ptr_real_out[i] = w1 + w2;
  }

  for (i = 0; i < loop_len; i++) {
    w1 = (-ptr_prev2_data[(frame_len / 2) + loop_len + i] * ptr_win2[(frame_len / 2) - 1 - i]);
    w1 += ptr_prev2_data[(frame_len / 2) + loop_len - 1 - i] * ptr_win2[(frame_len / 2) + i];

    w2 = ptr_curr_data[(frame_len / 2) + loop_len + i] * ptr_win0[(frame_len / 2) - 1 - i];
    w2 += (-ptr_curr_data[(frame_len / 2) + loop_len - 1 - i] * ptr_win0[(frame_len / 2) + i]);

    ptr_real_out[frame_len - 1 - i] = w1 + w2;
  }

  for (i = 0; i < loop_len; i++) {
    w1 = ptr_prev2_data[loop_len - 1 - i] * ptr_win3[i];
    w1 += ptr_prev3_data[loop_len + i] * ptr_win3[frame_len - 1 - i];

    w2 = (-ptr_curr_data[loop_len - 1 - i] * ptr_win1[i]);
    w2 += (-ptr_prev1_data[loop_len + i] * ptr_win1[frame_len - 1 - i]);

    ptr_real_out[(frame_len / 2) - 1 - i] = w1 + w2;
  }

  for (i = 0; i < loop_len; i++) {
    w1 = -(ptr_prev1_data[loop_len - 1 - i] * ptr_win2[i]);
    w1 += ptr_prev2_data[loop_len + i] * ptr_win2[frame_len - 1 - i];

    /* First 128 coeffcients are zeros in the window table so they are not used in the code here*/
    w2 = (-ptr_curr_data[loop_len + i] * ptr_win0[frame_len - 1 - i]);

    ptr_real_out[(frame_len / 2) + i] = w1 + w2;
  }

  ixheaace_mdct(ptr_real_out, fft_twiddle_tab, long_window_sine_ld, frame_len, 9,
                ptr_shared_buffer5);
}
