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

#include <math.h>
#include "ixheaac_type_def.h"
#include "ixheaac_constants.h"
#include "ixheaace_aac_constants.h"
#include "ixheaace_psy_const.h"
#include "ixheaace_tns.h"
#include "ixheaace_tns_params.h"
#include "ixheaace_rom.h"
#include "ixheaace_common_rom.h"
#include "ixheaace_bitbuffer.h"

static VOID ia_enhaacplus_enc_ps_fft(complex *out, LOOPINDEX N, WORD32 nob,
                                     ixheaace_common_tables *comm_tab_ptr) {
  LOOPINDEX block_per_stage, stage_num, inner;

  WORD32 index_1, index_2, tab_modifier;
  WORD32 len, increment, i;

  FLOAT32 cos_val;
  FLOAT32 sin_val;

  WORD16 index1;
  FLOAT32 re_temp, re_temp2;
  FLOAT32 im_temp, im_temp2;
  FLOAT32 *out1_w32, *out2_w32;

  len = N >> 1;
  index_1 = 0;

  out1_w32 = (FLOAT32 *)&out[index_1];
  out2_w32 = (FLOAT32 *)&out[index_1 + 1];

  for (block_per_stage = len - 1; block_per_stage >= 0; block_per_stage--) {
    re_temp = out1_w32[0];
    im_temp = out1_w32[1];

    re_temp2 = out2_w32[0];
    im_temp2 = out2_w32[1];

    out1_w32[0] = re_temp + re_temp2;
    out1_w32[1] = im_temp + im_temp2;

    out2_w32[0] = re_temp - re_temp2;
    out2_w32[1] = im_temp - im_temp2;

    out1_w32 += 4;
    out2_w32 += 4;
  }

  if (nob == 3) {
    i = 2; /* used for dist calculation*/
    increment = 1;
    tab_modifier = 2;
    for (stage_num = 1; stage_num >= 0; stage_num--) {
      len = len >> 1;
      index_1 = 0;
      increment += 1;

      tab_modifier--;

      for (block_per_stage = len - 1; block_per_stage >= 0; block_per_stage--) {
        index_2 = index_1 + i;

        out1_w32 = (FLOAT32 *)&out[index_1];
        out2_w32 = (FLOAT32 *)&out[index_2];

        re_temp = out1_w32[0];
        im_temp = out1_w32[1];

        re_temp2 = out2_w32[0];
        im_temp2 = out2_w32[1];

        out1_w32[0] = re_temp + re_temp2;
        out1_w32[1] = im_temp + im_temp2;

        out2_w32[0] = re_temp - re_temp2;
        out2_w32[1] = im_temp - im_temp2;

        index1 = (WORD16)tab_modifier;

        out1_w32 += 2;
        out2_w32 += 2;

        for (inner = 0; inner < (2 * i - 2); inner += 2) {
          cos_val = comm_tab_ptr->cos_arr[index1];
          sin_val = comm_tab_ptr->sin_arr[index1];

          index1++;

          re_temp = (out2_w32[inner] * cos_val) + (out2_w32[inner + 1] * sin_val);
          im_temp = (out2_w32[inner + 1] * cos_val) - (out2_w32[inner] * sin_val);

          re_temp2 = out1_w32[inner];
          im_temp2 = out1_w32[inner + 1];

          out1_w32[inner] = re_temp2 + re_temp;
          out1_w32[inner + 1] = im_temp2 + im_temp;

          out2_w32[inner] = re_temp2 - re_temp;
          out2_w32[inner + 1] = im_temp2 - im_temp;
        }

        index_1 += (WORD32)pow(2, increment);
      }
      i <<= 1;
    }
  } else {
    out1_w32 = (FLOAT32 *)&out[0];
    out2_w32 = (FLOAT32 *)&out[2];

    re_temp = out1_w32[0];
    im_temp = out1_w32[1];

    re_temp2 = out2_w32[0];
    im_temp2 = out2_w32[1];

    out1_w32[0] = re_temp + re_temp2;
    out1_w32[1] = im_temp + im_temp2;

    out2_w32[0] = re_temp - re_temp2;
    out2_w32[1] = im_temp - im_temp2;

    out1_w32 += 2;
    out2_w32 += 2;

    sin_val = comm_tab_ptr->sin_arr[1];

    re_temp = out2_w32[1] * sin_val;
    im_temp = out2_w32[0] * sin_val;

    re_temp2 = out1_w32[0];
    im_temp2 = out1_w32[1];

    out1_w32[0] = re_temp2 + re_temp;
    out1_w32[1] = im_temp2 - im_temp;

    out2_w32[0] = re_temp2 - re_temp;
    out2_w32[1] = im_temp2 + im_temp;
  }
}

VOID ia_enhaacplus_enc_fft(complex *out, WORD32 N, ixheaace_common_tables *pstr_common_tab) {
  WORD32 nob;
  WORD32 len;

  /* time domain samples obtained with bit reversal*/
  len = N;

  if (len == 8) {
    nob = 3;
  } else {
    nob = 2;
  }
  ia_enhaacplus_enc_ps_fft(out, N, nob, pstr_common_tab);
}
