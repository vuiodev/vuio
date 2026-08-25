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
#include "ixheaace_adjust_threshold_data.h"
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

#include "iusace_cnst.h"
#include "iusace_tns_usac.h"
#include "iusace_psy_mod.h"
#include "iusace_config.h"
#include "iusace_arith_enc.h"
#include "iusace_block_switch_const.h"
#include "iusace_block_switch_struct_def.h"
#include "iusace_fd_qc_util.h"
#include "iusace_fd_quant.h"
#include "iusace_ms.h"
#include "iusace_signal_classifier.h"
#include "ixheaace_sbr_header.h"
#include "ixheaace_config.h"
#include "ixheaace_asc_write.h"
#include "iusace_main.h"
#include "iusace_func_prototypes.h"
#include "iusace_avq_enc.h"

static VOID iusace_lsf_weight_2st_flt(FLOAT32 *lsfq, FLOAT32 *w, WORD32 mode) {
  WORD32 i;
  FLOAT32 d[ORDER + 1];

  d[0] = lsfq[0];
  d[ORDER] = FREQ_MAX - lsfq[ORDER - 1];
  for (i = 1; i < ORDER; i++) {
    d[i] = lsfq[i] - lsfq[i - 1];
  }

  for (i = 0; i < ORDER; i++) {
    w[i] = (FLOAT32)(iusace_wlsf_factor_table[mode] / (FREQ_DIV / sqrt(d[i] * d[i + 1])));
  }
}

static VOID iusace_lsf_weight(FLOAT32 *lsf, FLOAT32 *ptr_w) {
  WORD32 i;
  FLOAT32 d[ORDER + 1];

  d[0] = lsf[0];
  d[ORDER] = FREQ_MAX - lsf[ORDER - 1];
  for (i = 1; i < ORDER; i++) {
    d[i] = lsf[i] - lsf[i - 1];
  }

  for (i = 0; i < ORDER; i++) {
    ptr_w[i] = (1.0f / d[i]) + (1.0f / d[i + 1]);
  }

  return;
}

static WORD32 iusace_avq_first_approx_abs(FLOAT32 *lsf, FLOAT32 *lsfq) {
  WORD32 i, j, index;
  FLOAT32 w[ORDER];
  FLOAT32 dist_min, dist, temp;
  const FLOAT32 *p_dico;

  iusace_lsf_weight(lsf, w);

  dist_min = 1.0e30f;
  p_dico = iusace_dico_lsf_abs_8b_flt;
  index = 0;

  for (i = 0; i < 256; i++) {
    dist = 0.0;
    for (j = 0; j < ORDER; j++) {
      temp = lsf[j] - *p_dico++;
      dist += w[j] * temp * temp;
    }
    if (dist < dist_min) {
      dist_min = dist;
      index = i;
    }
  }

  for (j = 0; j < ORDER; j++) {
    lsfq[j] = iusace_dico_lsf_abs_8b_flt[index * ORDER + j];
  }

  return index;
}

static WORD32 iusace_avq_first_approx_rel(FLOAT32 *ptr_lsf, FLOAT32 *ptr_lsfq, WORD32 *idx,
                                          WORD32 mode) {
  WORD32 i, num_bits;
  FLOAT32 w[ORDER], x[ORDER], temp;
  WORD32 nq, avq[ORDER];
  FLOAT32 lsf_min;

  iusace_lsf_weight_2st_flt(ptr_lsf, w, 1);

  temp = 0.0f;
  for (i = 0; i < ORDER; i++) {
    x[i] = (ptr_lsf[i] - ptr_lsfq[i]) / w[i];
    temp += x[i] * x[i];
  }

  if (temp < 8.0f) {
    idx[0] = 0;
    idx[1] = 0;
    if ((mode == 0) || (mode == 3)) {
      return (10);
    } else if (mode == 1) {
      return (2);
    } else {
      return (6);
    }
  }

  iusace_lsf_weight_2st_flt(ptr_lsfq, w, mode);

  for (i = 0; i < ORDER; i++) {
    x[i] = (ptr_lsf[i] - ptr_lsfq[i]) / w[i];
  }

  iusace_alg_vec_quant(x, avq, idx);

  for (i = 0; i < ORDER; i++) {
    ptr_lsfq[i] += (w[i] * (FLOAT32)avq[i]);
  }

  num_bits = 0;
  for (i = 0; i < 2; i++) {
    nq = idx[i];

    if ((mode == 0) || (mode == 3)) {
      num_bits += (2 + (nq * 4));
      if (nq > 6) {
        num_bits += nq - 3;
      } else if (nq > 4) {
        num_bits += nq - 4;
      } else if (nq == 0) {
        num_bits += 3;
      }
    } else if (mode == 1) {
      num_bits += nq * 5;
      if (nq == 0) {
        num_bits += 1;
      }
    } else {
      num_bits += (2 + (nq * 4));
      if (nq == 0) {
        num_bits += 1;
      } else if (nq > 4) {
        num_bits += nq - 3;
      }
    }
  }

  lsf_min = LSF_GAP;
  for (i = 0; i < ORDER; i++) {
    if (ptr_lsfq[i] < lsf_min) {
      ptr_lsfq[i] = lsf_min;
    }

    lsf_min = ptr_lsfq[i] + LSF_GAP;
  }

  lsf_min = FREQ_MAX - LSF_GAP;
  for (i = ORDER - 1; i >= 0; i--) {
    if (ptr_lsfq[i] > lsf_min) {
      ptr_lsfq[i] = lsf_min;
    }

    lsf_min = ptr_lsfq[i] - LSF_GAP;
  }

  return (num_bits);
}

VOID iusace_quantize_lpc_avq(FLOAT32 *ptr_lsf, FLOAT32 *ptr_lsfq, WORD32 lpc0,
                             WORD32 *ptr_lpc_idx, WORD32 *nb_indices, WORD32 *nbbits) {
  WORD32 i;
  FLOAT32 lsfq[ORDER];
  WORD32 *ptr_index, indxt[100], num_bits, nbt, nit;

  ptr_index = &ptr_lpc_idx[0];
  *nb_indices = 0;
  *nbbits = 0;

  ptr_index[0] = iusace_avq_first_approx_abs(&ptr_lsf[3 * ORDER], &ptr_lsfq[3 * ORDER]);

  nbt = iusace_avq_first_approx_rel(&ptr_lsf[3 * ORDER], &ptr_lsfq[3 * ORDER], &ptr_index[1], 0);
  nit = 1 + iusace_get_num_params(&ptr_lpc_idx[1]);

  ptr_index += nit;
  *nb_indices += nit;
  *nbbits += 8 + nbt;

  if (lpc0) {
    *ptr_index = 0;
    ptr_index++;
    *nb_indices += 1;
    *nbbits += 1;

    ptr_index[0] = iusace_avq_first_approx_abs(&ptr_lsf[-ORDER], &ptr_lsfq[-ORDER]);

    num_bits = iusace_avq_first_approx_rel(&ptr_lsf[-ORDER], &ptr_lsfq[-ORDER], &ptr_index[1], 0);
    nbt = 8 + num_bits;
    nit = 1 + iusace_get_num_params(&ptr_index[1]);

    for (i = 0; i < ORDER; i++) lsfq[i] = ptr_lsfq[3 * ORDER + i];

    num_bits = iusace_avq_first_approx_rel(&ptr_lsf[-ORDER], &lsfq[0], indxt, 3);

    if (num_bits < nbt) {
      nbt = num_bits;
      nit = iusace_get_num_params(&indxt[0]);
      ptr_index[-1] = 1;
      for (i = 0; i < ORDER; i++) ptr_lsfq[-ORDER + i] = lsfq[i];
      for (i = 0; i < nit; i++) ptr_index[i] = indxt[i];
    }

    ptr_index += nit;
    *nb_indices += nit;
    *nbbits += nbt;
  }

  *ptr_index = 0;
  ptr_index++;
  *nb_indices += 1;
  *nbbits += 1;

  ptr_index[0] = iusace_avq_first_approx_abs(&ptr_lsf[ORDER], &ptr_lsfq[ORDER]);

  num_bits = iusace_avq_first_approx_rel(&ptr_lsf[ORDER], &ptr_lsfq[ORDER], &ptr_index[1], 0);
  nbt = 8 + num_bits;
  nit = 1 + iusace_get_num_params(&ptr_index[1]);

  for (i = 0; i < ORDER; i++) lsfq[i] = ptr_lsfq[3 * ORDER + i];

  num_bits = iusace_avq_first_approx_rel(&ptr_lsf[ORDER], &lsfq[0], indxt, 3);

  if (num_bits < nbt) {
    nbt = num_bits;
    nit = iusace_get_num_params(&indxt[0]);
    ptr_index[-1] = 1;
    for (i = 0; i < ORDER; i++) ptr_lsfq[ORDER + i] = lsfq[i];
    for (i = 0; i < nit; i++) ptr_index[i] = indxt[i];
  }

  ptr_index += nit;
  *nb_indices += nit;
  *nbbits += nbt;

  *ptr_index = 0;
  ptr_index++;
  *nb_indices += 1;

  ptr_index[0] = iusace_avq_first_approx_abs(&ptr_lsf[0], &ptr_lsfq[0]);

  num_bits = iusace_avq_first_approx_rel(&ptr_lsf[0], &ptr_lsfq[0], &ptr_index[1], 0);
  nbt = 2 + 8 + num_bits;
  nit = 1 + iusace_get_num_params(&ptr_index[1]);

  for (i = 0; i < ORDER; i++) lsfq[i] = 0.5f * (ptr_lsfq[-ORDER + i] + ptr_lsfq[ORDER + i]);

  num_bits = iusace_avq_first_approx_rel(&ptr_lsf[0], lsfq, indxt, 1);

  if (num_bits < 10) {
    nbt = 2;
    nit = 0;
    ptr_index[-1] = 1;
    for (i = 0; i < ORDER; i++) ptr_lsfq[i] = lsfq[i];
  }

  for (i = 0; i < ORDER; i++) lsfq[i] = ptr_lsfq[ORDER + i];

  num_bits = iusace_avq_first_approx_rel(&ptr_lsf[0], lsfq, indxt, 2);
  num_bits += 1;

  if (num_bits < nbt) {
    nbt = num_bits;
    nit = iusace_get_num_params(&indxt[0]);
    ptr_index[-1] = 2;
    for (i = 0; i < ORDER; i++) ptr_lsfq[i] = lsfq[i];
    for (i = 0; i < nit; i++) ptr_index[i] = indxt[i];
  }

  ptr_index += nit;
  *nb_indices += nit;
  *nbbits += nbt;

  *ptr_index = 0;
  ptr_index++;
  *nb_indices += 1;

  ptr_index[0] = iusace_avq_first_approx_abs(&ptr_lsf[2 * ORDER], &ptr_lsfq[2 * ORDER]);

  num_bits =
      iusace_avq_first_approx_rel(&ptr_lsf[2 * ORDER], &ptr_lsfq[2 * ORDER], &ptr_index[1], 0);
  nbt = 2 + 8 + num_bits;
  nit = 1 + iusace_get_num_params(&ptr_index[1]);

  for (i = 0; i < ORDER; i++) lsfq[i] = 0.5f * (ptr_lsfq[ORDER + i] + ptr_lsfq[3 * ORDER + i]);

  num_bits = iusace_avq_first_approx_rel(&ptr_lsf[2 * ORDER], lsfq, indxt, 1);
  num_bits += 1;

  if (num_bits < nbt) {
    nbt = num_bits;
    nit = iusace_get_num_params(&indxt[0]);
    ptr_index[-1] = 1;
    for (i = 0; i < ORDER; i++) ptr_lsfq[2 * ORDER + i] = lsfq[i];
    for (i = 0; i < nit; i++) ptr_index[i] = indxt[i];
  }

  for (i = 0; i < ORDER; i++) lsfq[i] = ptr_lsfq[ORDER + i];

  num_bits = iusace_avq_first_approx_rel(&ptr_lsf[2 * ORDER], lsfq, indxt, 2);
  num_bits += 3;

  if (num_bits < nbt) {
    nbt = num_bits;
    nit = iusace_get_num_params(&indxt[0]);
    ptr_index[-1] = 2;
    for (i = 0; i < ORDER; i++) ptr_lsfq[2 * ORDER + i] = lsfq[i];
    for (i = 0; i < nit; i++) ptr_index[i] = indxt[i];
  }

  for (i = 0; i < ORDER; i++) lsfq[i] = ptr_lsfq[3 * ORDER + i];

  num_bits = iusace_avq_first_approx_rel(&ptr_lsf[2 * ORDER], lsfq, indxt, 2);
  num_bits += 3;

  if (num_bits < nbt) {
    nbt = num_bits;
    nit = iusace_get_num_params(&indxt[0]);
    ptr_index[-1] = 3;
    for (i = 0; i < ORDER; i++) ptr_lsfq[2 * ORDER + i] = lsfq[i];
    for (i = 0; i < nit; i++) ptr_index[i] = indxt[i];
  }

  *nb_indices += nit;
  *nbbits += nbt;

  return;
}
