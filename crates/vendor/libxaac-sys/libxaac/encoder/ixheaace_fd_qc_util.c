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
#include <math.h>
#include <stdlib.h>
#include "iusace_type_def.h"
#include "iusace_cnst.h"
#include "iusace_bitbuffer.h"
#include "impd_drc_common_enc.h"
#include "impd_drc_uni_drc.h"
#include "impd_drc_api.h"
#include "impd_drc_uni_drc_eq.h"
#include "impd_drc_uni_drc_filter_bank.h"
#include "impd_drc_gain_enc.h"
#include "impd_drc_struct_def.h"

#include "ixheaace_memory_standards.h"
#include "iusace_tns_usac.h"
#include "iusace_psy_mod.h"
#include "iusace_config.h"
#include "ixheaace_adjust_threshold_data.h"
#include "iusace_fd_qc_util.h"
#include "iusace_fd_qc_adjthr.h"

VOID iusace_qc_create(ia_qc_main_struct *pstr_qc_main) {
  WORD32 i = 0;
  memset(pstr_qc_main, 0, sizeof(ia_qc_main_struct));

  for (i = 0; i < 2; i++) {
    memset(pstr_qc_main->str_qc_out.str_qc_out_chan[i].quant_spec, 0,
           sizeof(WORD16) * FRAME_LEN_LONG);
    memset(pstr_qc_main->str_qc_out.str_qc_out_chan[i].scalefactor, 0,
           sizeof(WORD16) * FRAME_LEN_LONG);
  }
  return;
}

VOID iusace_qc_init(ia_qc_data_struct *pstr_qc_data, const WORD32 max_bits, WORD32 sample_rate,
                    WORD32 bw_limit, WORD32 channels, WORD32 ccfl) {
  FLOAT32 mean_pe;
  pstr_qc_data->tot_avg_bits = pstr_qc_data->avg_bits;
  pstr_qc_data->static_bits = 1;
  pstr_qc_data->avg_bits = (pstr_qc_data->avg_bits - pstr_qc_data->static_bits);
  pstr_qc_data->max_bits = channels * max_bits;
  pstr_qc_data->max_bitres_bits = channels * max_bits - pstr_qc_data->tot_avg_bits;
  pstr_qc_data->max_bitres_bits =
      pstr_qc_data->max_bitres_bits - (pstr_qc_data->max_bitres_bits % 8);
  pstr_qc_data->bit_res_lvl = pstr_qc_data->max_bitres_bits;
  pstr_qc_data->padding = sample_rate;

  pstr_qc_data->max_bit_fac =
      (FLOAT32)channels * (max_bits - 744) /
      (FLOAT32)(pstr_qc_data->tot_avg_bits ? pstr_qc_data->tot_avg_bits : 1);
  mean_pe = 10.0f * ccfl * bw_limit / (sample_rate / 2.0f);

  iusace_adj_thr_init(&pstr_qc_data->str_adj_thr_ele, mean_pe,
                      (channels > 0) ? pstr_qc_data->ch_bitrate / channels : 0);

  return;
}

static WORD32 iusace_calc_frame_len(WORD32 bit_rate, WORD32 sample_rate, WORD32 mode,
                                    WORD32 ccfl) {
  WORD32 result;

  result = ((ccfl) >> 3) * (bit_rate);
  switch (mode) {
    case FRAME_LEN_BYTES_MODULO:
      result %= sample_rate;
      break;
    case FRAME_LEN_BYTES_INT:
      result /= sample_rate;
      break;
  }

  return (result);
}

static WORD32 iusace_get_frame_padding(WORD32 bit_rate, WORD32 sample_rate, WORD32 *padding,
                                       WORD32 ccfl) {
  WORD32 padding_on = 0;
  WORD32 difference;

  difference = iusace_calc_frame_len(bit_rate, sample_rate, FRAME_LEN_BYTES_MODULO, ccfl);

  *padding -= difference;

  if (*padding <= 0) {
    padding_on = 1;
    *padding += sample_rate;
  }

  return padding_on;
}

VOID iusace_adj_bitrate(ia_qc_data_struct *pstr_qc_data, WORD32 bit_rate, WORD32 sample_rate,
                        WORD32 ccfl) {
  WORD32 padding_on;
  WORD32 frame_len;
  WORD32 code_bits;
  WORD32 code_bits_prev;
  WORD32 total_bits = 0;

  padding_on = iusace_get_frame_padding(bit_rate, sample_rate, &pstr_qc_data->padding, ccfl);
  frame_len =
      padding_on + iusace_calc_frame_len(bit_rate, sample_rate, FRAME_LEN_BYTES_INT, ccfl);

  frame_len <<= 3;
  code_bits_prev = pstr_qc_data->tot_avg_bits - pstr_qc_data->static_bits;
  code_bits = frame_len - pstr_qc_data->static_bits;

  if (code_bits != code_bits_prev) {
    pstr_qc_data->avg_bits = (WORD32)code_bits;
    total_bits += pstr_qc_data->avg_bits;
    pstr_qc_data->avg_bits += code_bits - total_bits;
  }

  pstr_qc_data->tot_avg_bits = frame_len;

  return;
}

WORD32 iusace_calc_max_val_in_sfb(WORD32 sfb_count, WORD32 max_sfb_per_grp, WORD32 sfb_per_group,
                                  WORD32 *ptr_sfb_offset, WORD16 *ptr_quant_spec) {
  WORD32 sfb;
  WORD32 max = 0;
  WORD32 sfb_offs;

  for (sfb_offs = 0; sfb_offs < sfb_count; sfb_offs += sfb_per_group) {
    for (sfb = 0; sfb < max_sfb_per_grp; sfb++) {
      WORD32 line;
      WORD32 local_max = 0;
      for (line = ptr_sfb_offset[sfb + sfb_offs]; line < ptr_sfb_offset[sfb + sfb_offs + 1];
           line++) {
        if (abs(ptr_quant_spec[line]) > local_max) {
          local_max = abs(ptr_quant_spec[line]);
        }
      }
      if (local_max > max) {
        max = local_max;
      }
    }
  }

  return max;
}
