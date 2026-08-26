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
#include "iusace_block_switch_const.h"
#include "iusace_cnst.h"
#include "iusace_rom.h"
#include "iusace_bitbuffer.h"

/* DRC */
#include "impd_drc_common_enc.h"
#include "impd_drc_uni_drc.h"
#include "impd_drc_tables.h"
#include "impd_drc_api.h"
#include "impd_drc_uni_drc_eq.h"
#include "impd_drc_uni_drc_filter_bank.h"
#include "impd_drc_gain_enc.h"
#include "impd_drc_struct_def.h"

#include "iusace_tns_usac.h"
#include "iusace_psy_mod.h"
#include "iusace_tns_usac.h"
#include "iusace_config.h"
#include "iusace_fft.h"
#include "iusace_tcx_mdct.h"

VOID iusace_tcx_mdct_main(FLOAT32 *ptr_in, FLOAT32 *ptr_out, WORD32 l, WORD32 m, WORD32 r,
                          iusace_scratch_mem *pstr_scratch) {
  WORD32 length = l / 2 + m + r / 2;
  FLOAT32 *input = pstr_scratch->p_tcx_input;
  WORD32 i;

  for (i = 0; i < m / 2; i++) {
    input[m / 2 + r / 2 + i] = -ptr_in[l + m / 2 - 1 - i];
  }
  for (i = 0; i < l / 2; i++) {
    input[m / 2 + r / 2 + m / 2 + i] = ptr_in[i] - ptr_in[l - 1 - i];
  }
  for (i = 0; i < m / 2; i++) {
    input[m / 2 + r / 2 - 1 - i] = -ptr_in[l + m / 2 + i];
  }
  for (i = 0; i < r / 2; i++) {
    input[m / 2 + r / 2 - 1 - m / 2 - i] = -ptr_in[l + m + i] - ptr_in[l + m + r - 1 - i];
  }

  iusace_tcx_mdct(input, ptr_out, length, pstr_scratch);

  return;
}

VOID iusace_tcx_imdct(FLOAT32 *ptr_in, FLOAT32 *ptr_out, WORD32 l, WORD32 m, WORD32 r,
                      iusace_scratch_mem *pstr_scratch) {
  WORD32 length = l / 2 + m + r / 2;
  FLOAT32 *output = pstr_scratch->p_tcx_output;
  iusace_tcx_mdct(ptr_in, output, length, pstr_scratch);

  WORD32 i;

  for (i = 0; i < m / 2; i++) {
    ptr_out[l + m / 2 - 1 - i] = -1.0f * output[m / 2 + r / 2 + i];
  }
  for (i = 0; i < l / 2; i++) {
    ptr_out[i] = output[m / 2 + r / 2 + m / 2 + i];
    ptr_out[l - 1 - i] = -1.0f * output[m / 2 + r / 2 + m / 2 + i];
  }
  for (i = 0; i < m / 2; i++) {
    ptr_out[l + m / 2 + i] = -1.0f * output[m / 2 + r / 2 - 1 - i];
  }
  for (i = 0; i < r / 2; i++) {
    ptr_out[l + m + i] = -1.0f * output[m / 2 + r / 2 - 1 - m / 2 - i];
    ptr_out[l + m + r - 1 - i] = -1.0f * output[m / 2 + r / 2 - 1 - m / 2 - i];
  }
  return;
}

static VOID iusace_get_pre_post_twid(FLOAT32 **ptr_pre_twid_re, FLOAT32 **ptr_pre_twid_im,
                                     FLOAT32 **ptr_post_twid_re, FLOAT32 **ptr_post_twid_im,
                                     WORD32 length) {
  switch (length) {
    case 512:
      *ptr_pre_twid_re = (FLOAT32 *)&iusace_pre_post_twid_cos_sin_256[0][0];
      *ptr_pre_twid_im = (FLOAT32 *)&iusace_pre_post_twid_cos_sin_256[1][0];
      *ptr_post_twid_re = (FLOAT32 *)&iusace_pre_post_twid_cos_sin_256[2][0];
      *ptr_post_twid_im = (FLOAT32 *)&iusace_pre_post_twid_cos_sin_256[3][0];
      break;
    case 256:
      *ptr_pre_twid_re = (FLOAT32 *)&iusace_pre_post_twid_cos_sin_128[0][0];
      *ptr_pre_twid_im = (FLOAT32 *)&iusace_pre_post_twid_cos_sin_128[1][0];
      *ptr_post_twid_re = (FLOAT32 *)&iusace_pre_post_twid_cos_sin_128[2][0];
      *ptr_post_twid_im = (FLOAT32 *)&iusace_pre_post_twid_cos_sin_128[3][0];
      break;
    case 128:
      *ptr_pre_twid_re = (FLOAT32 *)&iusace_pre_post_twid_cos_sin_64[0][0];
      *ptr_pre_twid_im = (FLOAT32 *)&iusace_pre_post_twid_cos_sin_64[1][0];
      *ptr_post_twid_re = (FLOAT32 *)&iusace_pre_post_twid_cos_sin_64[2][0];
      *ptr_post_twid_im = (FLOAT32 *)&iusace_pre_post_twid_cos_sin_64[3][0];
      break;
    case 64:
      *ptr_pre_twid_re = (FLOAT32 *)&iusace_pre_post_twid_cos_sin_32[0][0];
      *ptr_pre_twid_im = (FLOAT32 *)&iusace_pre_post_twid_cos_sin_32[1][0];
      *ptr_post_twid_re = (FLOAT32 *)&iusace_pre_post_twid_cos_sin_32[2][0];
      *ptr_post_twid_im = (FLOAT32 *)&iusace_pre_post_twid_cos_sin_32[3][0];
      break;
    default:
      *ptr_pre_twid_re = (FLOAT32 *)&iusace_pre_post_twid_cos_sin_512[0][0];
      *ptr_pre_twid_im = (FLOAT32 *)&iusace_pre_post_twid_cos_sin_512[1][0];
      *ptr_post_twid_re = (FLOAT32 *)&iusace_pre_post_twid_cos_sin_512[2][0];
      *ptr_post_twid_im = (FLOAT32 *)&iusace_pre_post_twid_cos_sin_512[3][0];
  }

  return;
}

static VOID iusace_pre_twid(FLOAT32 *ptr_twid_re, FLOAT32 *ptr_twid_im, FLOAT32 *ptr_in,
                            WORD32 length) {
  WORD32 i;
  for (i = 0; i < length; i++) {
    FLOAT32 temp = ptr_in[2 * i] * ptr_twid_re[i] - ptr_in[2 * i + 1] * ptr_twid_im[i];

    ptr_in[2 * i + 1] = ptr_in[2 * i] * ptr_twid_im[i] + ptr_in[2 * i + 1] * ptr_twid_re[i];

    ptr_in[2 * i] = temp;
  }
  return;
}

static VOID iusace_post_twid(FLOAT32 *ptr_twid_re, FLOAT32 *ptr_twid_im, FLOAT32 *ptr_in,
                             WORD32 length) {
  WORD32 i;
  for (i = 0; i < length; i++) {
    FLOAT32 temp = ptr_in[2 * i] * ptr_twid_re[i] - ptr_in[2 * i + 1] * ptr_twid_im[i];

    ptr_in[2 * i + 1] = -ptr_in[2 * i] * ptr_twid_im[i] - ptr_in[2 * i + 1] * ptr_twid_re[i];
    ptr_in[2 * i] = temp;
  }
  return;
}

VOID iusace_tcx_mdct(FLOAT32 *ptr_in, FLOAT32 *ptr_out, WORD32 length,
                     iusace_scratch_mem *pstr_scratch) {
  FLOAT32 *ptr_pre_twid_re, *ptr_pre_twid_im;
  FLOAT32 *ptr_post_twid_re, *ptr_post_twid_im;
  WORD32 i;
  FLOAT32 *ptr_real = pstr_scratch->p_temp_mdct;
  WORD32 len_by_2 = length >> 1;

  iusace_get_pre_post_twid(&ptr_pre_twid_re, &ptr_pre_twid_im, &ptr_post_twid_re,
                           &ptr_post_twid_im, length);

  for (i = 0; i < len_by_2; i++) {
    ptr_real[2 * i] = ptr_in[2 * i];
    ptr_real[2 * i + 1] = ptr_in[length - 1 - 2 * i];
  }

  /* pre twiddle */
  iusace_pre_twid(ptr_pre_twid_re, ptr_pre_twid_im, ptr_real, len_by_2);

  iusace_complex_fft(ptr_real, len_by_2, pstr_scratch);

  /* post twiddle */
  iusace_post_twid(ptr_post_twid_re, ptr_post_twid_im, ptr_real, len_by_2);

  for (i = 0; i < len_by_2; i++) {
    ptr_out[2 * i] = ptr_real[2 * i];
    ptr_out[length - 1 - 2 * i] = ptr_real[2 * i + 1];
  }

  return;
}
