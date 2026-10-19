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
#include "ixheaac_type_def.h"
#include "ixheaacd_bitbuffer.h"
#include "ixheaacd_interface.h"
#include "ixheaacd_tns_usac.h"
#include "ixheaacd_cnst.h"
#include "ixheaacd_acelp_info.h"
#include "ixheaacd_sbrdecsettings.h"
#include "ixheaacd_info.h"
#include "ixheaacd_sbr_common.h"
#include "ixheaacd_drc_data_struct.h"
#include "ixheaacd_drc_dec.h"
#include "ixheaacd_sbrdecoder.h"
#include "ixheaacd_mps_polyphase.h"
#include "ixheaacd_ec_defines.h"
#include "ixheaacd_ec_struct_def.h"
#include "ixheaacd_main.h"
#include "ixheaacd_arith_dec.h"
#include "ixheaacd_func_def.h"
#include "ixheaacd_acelp_com.h"

#include "ixheaac_basic_op.h"
#include "ixheaac_constants.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops40.h"

#define LSF_GAP 50.0f
#define FREQ_MAX 6400.0f
#define FREQ_DIV 400.0f

static const FLOAT32 factor_table[4] = {60.0f, 65.0f, 64.0f, 63.0f};

VOID ixheaacd_lsf_weight_2st_flt(FLOAT32 *lsfq, FLOAT32 *w, WORD32 mode) {
  WORD32 i;
  FLOAT32 d[ORDER + 1];

  d[0] = lsfq[0];
  d[ORDER] = FREQ_MAX - lsfq[ORDER - 1];
  for (i = 1; i < ORDER; i++) {
    d[i] = lsfq[i] - lsfq[i - 1];
  }

  for (i = 0; i < ORDER; i++) {
    w[i] = (FLOAT32)((factor_table[mode] * sqrt(d[i] * d[i + 1])) / FREQ_DIV);
  }

  return;
}

static WORD32 ixheaacd_decoding_avq_tool(WORD32 *read_arr, WORD32 *nvecq) {
  WORD32 i, k, qn, kv[8] = {0};
  WORD32 code_book_idx;
  WORD32 *ptr_kv = &kv[0];

  WORD32 position = 2;

  for (k = 0; k < 2; k++) {
    qn = read_arr[k];

    if (qn > 0) {
      code_book_idx = read_arr[position++];

      ptr_kv = &read_arr[position];
      position += 8;

    } else {
      code_book_idx = 0;
      for (i = 0; i < 8; i++) ptr_kv = &kv[0];
    }

    ixheaacd_rotated_gosset_mtx_dec(qn, code_book_idx, ptr_kv, &nvecq[k * 8]);
  }

  return position;
}

static WORD32 ixheaacd_avq_first_approx_abs(FLOAT32 *lsf, WORD32 *indx) {
  WORD32 i;
  extern const FLOAT32 ixheaacd_dico_lsf_abs_8b_flt[];
  extern const FLOAT32 ixheaacd_weight_table_avq[];
  WORD32 position = 0;
  WORD32 avq[ORDER];
  FLOAT32 lsf_min;
  const FLOAT32 *ptr_w;

  ptr_w = &ixheaacd_weight_table_avq[(indx[0] * ORDER)];

  position++;

  for (i = 0; i < ORDER; i++) {
    lsf[i] = ixheaacd_dico_lsf_abs_8b_flt[indx[0] * ORDER + i];
  }

  position += ixheaacd_decoding_avq_tool(&indx[position], avq);

  lsf_min = LSF_GAP;
  for (i = 0; i < ORDER; i++) {
    lsf[i] += (ptr_w[i] * avq[i]);

    if (lsf[i] < lsf_min) lsf[i] = lsf_min;

    lsf_min = lsf[i] + LSF_GAP;
  }

  lsf_min = FREQ_MAX - LSF_GAP;
  for (i = ORDER - 1; i >= 0; i--) {
    if (lsf[i] > lsf_min) {
      lsf[i] = lsf_min;
    }

    lsf_min = lsf[i] - LSF_GAP;
  }

  return position;
}

WORD32 ixheaacd_avq_first_approx_rel(FLOAT32 *lsf, WORD32 *indx, WORD32 mode) {
  WORD32 i;
  FLOAT32 w[ORDER];
  WORD32 avq[ORDER];
  WORD32 position = 0;
  FLOAT32 lsf_min;

  ixheaacd_lsf_weight_2st_flt(lsf, w, mode);

  position = ixheaacd_decoding_avq_tool(indx, avq);

  lsf_min = LSF_GAP;

  for (i = 0; i < ORDER; i++) {
    lsf[i] += (w[i] * avq[i]);

    if (lsf[i] < lsf_min) lsf[i] = lsf_min;

    lsf_min = lsf[i] + LSF_GAP;
  }

  return position;
}

VOID ixheaacd_alg_vec_dequant(ia_td_frame_data_struct *pstr_td_frame_data, WORD32 first_lpd_flag,
                              FLOAT32 *lsf, WORD32 mod[], WORD32 ec_flag) {
  WORD32 i;
  WORD32 *lpc_index, mode_lpc, pos = 0;
  WORD32 lpc_present[5] = {0, 0, 0, 0, 0};
  lpc_index = pstr_td_frame_data->lpc_first_approx_idx;
  lpc_present[4] = 1;
  pos = ixheaacd_avq_first_approx_abs(&lsf[4 * ORDER], &lpc_index[0]);

  lpc_index += pos;

  if (first_lpd_flag) {
    mode_lpc = lpc_index[0];
    lpc_index++;

    if (mode_lpc == 0) {
      pos = ixheaacd_avq_first_approx_abs(&lsf[0], &lpc_index[0]);

    } else if (mode_lpc == 1) {
      for (i = 0; i < ORDER; i++) lsf[i] = lsf[4 * ORDER + i];
      pos = ixheaacd_avq_first_approx_rel(&lsf[0], &lpc_index[0], 3);
    }

    lpc_index += pos;
  }
  lpc_present[0] = 1;

  if (mod[0] < 3) {
    mode_lpc = lpc_index[0];
    lpc_index++;
    lpc_present[2] = 1;

    if (mode_lpc == 0) {
      pos = ixheaacd_avq_first_approx_abs(&lsf[2 * ORDER], &lpc_index[0]);
    } else if (mode_lpc == 1) {
      for (i = 0; i < ORDER; i++) lsf[2 * ORDER + i] = lsf[4 * ORDER + i];
      pos = ixheaacd_avq_first_approx_rel(&lsf[2 * ORDER], &lpc_index[0], 3);
    }

    lpc_index += pos;
  }

  if (mod[0] < 2) {
    mode_lpc = lpc_index[0];
    lpc_index++;
    lpc_present[1] = 1;

    if (mode_lpc == 1) {
      for (i = 0; i < ORDER; i++)
        lsf[ORDER + i] = 0.5f * (lsf[i] + lsf[2 * ORDER + i]);
    } else {
      if (mode_lpc == 0) {
        pos = ixheaacd_avq_first_approx_abs(&lsf[ORDER], &lpc_index[0]);
      } else if (mode_lpc == 2) {
        for (i = 0; i < ORDER; i++) lsf[ORDER + i] = lsf[2 * ORDER + i];
        pos = ixheaacd_avq_first_approx_rel(&lsf[ORDER], &lpc_index[0], 2);
      }

      lpc_index += pos;
    }
  }

  if (mod[2] < 2) {
    mode_lpc = lpc_index[0];
    lpc_index++;
    lpc_present[3] = 1;

    if (mode_lpc == 0) {
      pos = ixheaacd_avq_first_approx_abs(&lsf[3 * ORDER], &lpc_index[0]);
    } else if (mode_lpc == 1) {
      for (i = 0; i < ORDER; i++)
        lsf[3 * ORDER + i] = 0.5f * (lsf[2 * ORDER + i] + lsf[4 * ORDER + i]);
      pos = ixheaacd_avq_first_approx_rel(&lsf[3 * ORDER], &lpc_index[0], 1);
    } else if (mode_lpc == 2) {
      for (i = 0; i < ORDER; i++) lsf[3 * ORDER + i] = lsf[2 * ORDER + i];
      pos = ixheaacd_avq_first_approx_rel(&lsf[3 * ORDER], &lpc_index[0], 2);
    } else if (mode_lpc == 3) {
      for (i = 0; i < ORDER; i++) lsf[3 * ORDER + i] = lsf[4 * ORDER + i];
      pos = ixheaacd_avq_first_approx_rel(&lsf[3 * ORDER], &lpc_index[0], 2);
    }

    lpc_index += pos;
  }
  if (ec_flag) {
    WORD32 last, k;
    WORD32 num_lpc = 0, num_div = 4;
    FLOAT32 div_fac;
    FLOAT32 *lsf4 = &lsf[4 * ORDER];
    for (i = 0; i < ORDER; i++) {
      pstr_td_frame_data->lpc4_lsf[i] = lsf4[i];
    }
    i = num_div;
    do {
      num_lpc += lpc_present[i--];
    } while (i >= 0 && num_lpc < 3);

    last = i;

    switch (num_lpc) {
      case 3:
        div_fac = (1.0f / 3.0f);
        break;
      case 2:
        div_fac = (1.0f / 2.0f);
        break;
      default:
        div_fac = (1.0f);
        break;
    }
    for (k = 0; k < ORDER; k++) {
      FLOAT32 temp = 0;
      for (i = 4; i > last; i--) {
        if (lpc_present[i]) {
          temp = temp + (lsf[i * ORDER + k] * div_fac);
        }
      }
      pstr_td_frame_data->lsf_adaptive_mean_cand[k] = temp;
    }
  }
}
