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

#include "ixheaacd_func_def.h"
#include "ixheaac_constants.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops40.h"

extern const WORD32 ixheaacd_pre_post_twid_cos_sin_512[4][512];
extern const WORD32 ixheaacd_pre_post_twid_cos_sin_384[4][384];
extern const WORD32 ixheaacd_pre_post_twid_cos_sin_256[4][256];
extern const WORD32 ixheaacd_pre_post_twid_cos_sin_192[4][192];
extern const WORD32 ixheaacd_pre_post_twid_cos_sin_128[4][128];
extern const WORD32 ixheaacd_pre_post_twid_cos_sin_96[4][96];
extern const WORD32 ixheaacd_pre_post_twid_cos_sin_64[4][64];
extern const WORD32 ixheaacd_pre_post_twid_cos_sin_48[4][48];
extern const WORD32 ixheaacd_pre_post_twid_cos_sin_32[4][32];
extern const WORD32 ixheaacd_pre_post_twid_cos_sin_24[4][24];

static PLATFORM_INLINE WORD32 ixheaacd_mul_sub64_sat_32(WORD32 a, WORD32 b,
                                                        WORD32 c, WORD32 d) {
  WORD64 diff;
  WORD64 temp_result1;
  WORD64 temp_result2;

  temp_result1 = (WORD64)a * (WORD64)c;
  temp_result2 = (WORD64)b * (WORD64)d;

  diff = (temp_result1 - temp_result2) >> 32;

  if (diff >= 2147483647)
    diff = 2147483647;
  else if (diff <= -2147483647 - 1)
    diff = -2147483647 - 1;

  return ((WORD32)diff);
}

static PLATFORM_INLINE WORD32 ixheaacd_mul_add64_sat_32(WORD32 a, WORD32 b,
                                                        WORD32 c, WORD32 d) {
  WORD64 sum;
  WORD64 temp_result1;
  WORD64 temp_result2;

  temp_result1 = (WORD64)a * (WORD64)c;
  temp_result2 = (WORD64)b * (WORD64)d;

  sum = (temp_result1 + temp_result2) >> 32;

  if (sum >= 2147483647)
    sum = 2147483647;
  else if (sum <= -2147483647 - 1)
    sum = -2147483647 - 1;

  return ((WORD32)sum);
}

static void ixheaacd_pre_twid(WORD32 *in, WORD32 *r_ptr, WORD32 *i_ptr,
                              WORD32 nlength, const WORD32 *ptr_pre_cos_sin) {
  WORD32 i;

  const WORD32 *cos_ptr = &ptr_pre_cos_sin[0];
  const WORD32 *sin_ptr = &ptr_pre_cos_sin[nlength];

  for (i = 0; i < nlength; i += 4) {
    *r_ptr++ = ixheaacd_mul_sub64_sat_32(in[i], in[nlength + i], cos_ptr[i],
                                         sin_ptr[i]);
    *i_ptr++ = ixheaacd_mul_add64_sat_32(in[i], in[nlength + i], sin_ptr[i],
                                         cos_ptr[i]);

    *r_ptr++ = ixheaacd_mul_sub64_sat_32(in[i + 1], in[nlength + i + 1],
                                         cos_ptr[i + 1], sin_ptr[i + 1]);
    *i_ptr++ = ixheaacd_mul_add64_sat_32(in[i + 1], in[nlength + i + 1],
                                         sin_ptr[i + 1], cos_ptr[i + 1]);

    *r_ptr++ = ixheaacd_mul_sub64_sat_32(in[i + 2], in[nlength + i + 2],
                                         cos_ptr[i + 2], sin_ptr[i + 2]);
    *i_ptr++ = ixheaacd_mul_add64_sat_32(in[i + 2], in[nlength + i + 2],
                                         sin_ptr[i + 2], cos_ptr[i + 2]);

    *r_ptr++ = ixheaacd_mul_sub64_sat_32(in[i + 3], in[nlength + i + 3],
                                         cos_ptr[i + 3], sin_ptr[i + 3]);
    *i_ptr++ = ixheaacd_mul_add64_sat_32(in[i + 3], in[nlength + i + 3],
                                         sin_ptr[i + 3], cos_ptr[i + 3]);
  }
}

static void ixheaacd_post_twid(WORD32 *data_re, WORD32 *data_im, WORD32 *out,
                               WORD32 nlength, const WORD32 *ptr_post_cos_sin) {
  WORD32 i;

  const WORD32 *cos_ptr = &ptr_post_cos_sin[nlength * 2];
  const WORD32 *sin_ptr = &ptr_post_cos_sin[nlength * 3];

  WORD32 *out_ptr = &out[2 * nlength - 1];
  for (i = 0; i < nlength; i += 4) {
    out[0] = ixheaacd_mul_sub64_sat_32(data_re[i], data_im[i], cos_ptr[i],
                                       sin_ptr[i]);
    out_ptr[0] = -ixheaacd_mul_add64_sat_32(data_re[i], data_im[i], sin_ptr[i],
                                            cos_ptr[i]);

    out[2] = ixheaacd_mul_sub64_sat_32(data_re[i + 1], data_im[i + 1],
                                       cos_ptr[i + 1], sin_ptr[i + 1]);
    out_ptr[-2] = -ixheaacd_mul_add64_sat_32(data_re[i + 1], data_im[i + 1],
                                             sin_ptr[i + 1], cos_ptr[i + 1]);

    out[4] = ixheaacd_mul_sub64_sat_32(data_re[i + 2], data_im[i + 2],
                                       cos_ptr[i + 2], sin_ptr[i + 2]);
    out_ptr[-4] = -ixheaacd_mul_add64_sat_32(data_re[i + 2], data_im[i + 2],
                                             sin_ptr[i + 2], cos_ptr[i + 2]);

    out[6] = ixheaacd_mul_sub64_sat_32(data_re[i + 3], data_im[i + 3],
                                       cos_ptr[i + 3], sin_ptr[i + 3]);
    out_ptr[-6] = -ixheaacd_mul_add64_sat_32(data_re[i + 3], data_im[i + 3],
                                             sin_ptr[i + 3], cos_ptr[i + 3]);
    out += 8;
    out_ptr -= 8;
  }
}

VOID ixheaacd_acelp_mdct(WORD32 *ptr_in, WORD32 *ptr_out, WORD32 *preshift, WORD32 length,
                         WORD32 *ptr_scratch) {
  WORD32 *ptr_data_r = ptr_scratch;
  WORD32 *ptr_data_i = ptr_scratch + 512;
  const WORD32 *ptr_pre_post_twid;

  switch (length) {
    case 1024:
      ptr_pre_post_twid = &ixheaacd_pre_post_twid_cos_sin_512[0][0];
      break;
    case 768:
      ptr_pre_post_twid = &ixheaacd_pre_post_twid_cos_sin_384[0][0];
      break;
    case 512:
      ptr_pre_post_twid = &ixheaacd_pre_post_twid_cos_sin_256[0][0];
      break;
    case 384:
      ptr_pre_post_twid = &ixheaacd_pre_post_twid_cos_sin_192[0][0];
      break;
    case 256:
      ptr_pre_post_twid = &ixheaacd_pre_post_twid_cos_sin_128[0][0];
      break;
    case 192:
      ptr_pre_post_twid = &ixheaacd_pre_post_twid_cos_sin_96[0][0];
      break;
    case 128:
      ptr_pre_post_twid = &ixheaacd_pre_post_twid_cos_sin_64[0][0];
      break;
    case 96:
      ptr_pre_post_twid = &ixheaacd_pre_post_twid_cos_sin_48[0][0];
      break;
    case 64:
      ptr_pre_post_twid = &ixheaacd_pre_post_twid_cos_sin_32[0][0];
      break;
    case 48:
      ptr_pre_post_twid = &ixheaacd_pre_post_twid_cos_sin_24[0][0];
      break;
    default:
      ptr_pre_post_twid = &ixheaacd_pre_post_twid_cos_sin_24[0][0];
      break;
  }

  ixheaacd_pre_twid(ptr_in, ptr_data_r, ptr_data_i, length / 2,
                    ptr_pre_post_twid);

  ixheaacd_complex_fft(ptr_data_r, ptr_data_i, length / 2, -1, preshift);
  *preshift += 1;

  ixheaacd_post_twid(ptr_data_r, ptr_data_i, ptr_out, length / 2,
                     ptr_pre_post_twid);
  *preshift += 1;
  return;
}

VOID ixheaacd_acelp_mdct_main(ia_usac_data_struct *usac_data, WORD32 *in, WORD32 *out, WORD32 l,
                              WORD32 m, WORD32 *preshift) {
  WORD32 i;
  WORD32 *ptr_scratch = &usac_data->scratch_buffer[0];
  WORD32 *output_buffer = &usac_data->x_ac_dec[0];

  ixheaacd_acelp_mdct(in, output_buffer, preshift, l + m, ptr_scratch);

  for (i = 0; i < m / 2; i++) {
    out[l + m / 2 - 1 - i] = -output_buffer[m / 2 + l / 2 + i];
  }
  for (i = 0; i < l / 2; i++) {
    out[i] = output_buffer[m + l / 2 + i];
    out[l - 1 - i] = -output_buffer[m + l / 2 + i];
  }
  for (i = 0; i < m / 2; i++) {
    out[l + m / 2 + i] = -output_buffer[m / 2 + l / 2 - 1 - i];
  }
  for (i = 0; i < l / 2; i++) {
    out[l + m + i] = -output_buffer[l / 2 - 1 - i];
    out[2 * l + m - 1 - i] = -output_buffer[l / 2 - 1 - i];
  }
  return;
}
