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
#include "ixheaac_sbr_const.h"

#include "ixheaacd_ec_defines.h"
#include "ixheaacd_ec_struct_def.h"
#include "ixheaacd_main.h"
#include "ixheaacd_arith_dec.h"
#include "ixheaacd_function_selector.h"
#include "ixheaac_constants.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops40.h"

#define sfb_offset(x) (((x) > 0) ? sfb_top[(x)-1] : 0)

static VOID ixheaacd_tns_dec_coef_usac(ia_usac_data_struct *usac_data,
                                       ia_tns_filter_struct *filter,
                                       WORD32 coef_res, WORD32 *par_coeff) {
  WORD32 resolution;
  WORD32 *ptr_par_coeff = par_coeff;
  const WORD32 *tns_coeff_ptr;
  WORD32 ixheaacd_drc_offset = 4;
  WORD16 *ptr_coeff = filter->coef;
  WORD32 order;

  resolution = coef_res - 3;
  tns_coeff_ptr = usac_data->tns_coeff3_32;
  if (resolution) {
    tns_coeff_ptr = usac_data->tns_coeff4_32;
    ixheaacd_drc_offset = ixheaacd_drc_offset << 1;
  }
  order = filter->order;

  do {
    WORD16 temp = *ptr_coeff++;
    *ptr_par_coeff++ = tns_coeff_ptr[temp + ixheaacd_drc_offset];
    order--;
  } while (order != 0);
}

static VOID ixheaacd_tns_parcor_lpc_convert_usac(WORD32 *parcor,
                                                 WORD32 *lpc_coeff,
                                                 WORD32 *scale, WORD order)

{
  WORD i, j, status;
  WORD32 accu;
  WORD32 temp_buf1[TNS_MAX_ORDER + 1];
  WORD32 temp_buf2[TNS_MAX_ORDER + 1];
  WORD32 accu1, accu2;

  status = 1;
  *scale = 1;

  while (status) {
    status = 0;

    for (i = TNS_MAX_ORDER; i >= 0; i--) {
      temp_buf1[i] = 0;
      temp_buf2[i] = 0;
    }

    accu1 = (0x40000000 >> (*scale - 1));

    for (i = 0; i <= order; i++) {
      accu = accu1;

      for (j = 0; j < order; j++) {
        temp_buf2[j] = (accu1);

        accu1 = ixheaac_add32_sat(
            accu1, ixheaac_mult32_shl_sat(parcor[j], temp_buf1[j]));
        if (ixheaac_abs32_sat(accu1) == 0x7fffffff) status = 1;
      }

      for (j = (order - 1); j >= 0; j--) {
        accu2 = (temp_buf1[j]);
        accu2 = ixheaac_add32_sat(
            accu2, ixheaac_mult32_shl_sat(parcor[j], temp_buf2[j]));
        temp_buf1[j + 1] = (accu2);

        if (ixheaac_abs32_sat(accu2) == 0x7fffffff) status = 1;
      }

      temp_buf1[0] = (accu);
      lpc_coeff[i] = (accu1);
      accu1 = 0;
    }

    accu1 = (status - 1);

    if (accu1 == 0) {
      *scale = *scale + 1;
    }
  }
}

static VOID ixheaacd_tns_ar_filter_usac(WORD32 *spectrum, WORD32 size,
                                        WORD32 inc, WORD32 *lpc_coeff,
                                        WORD32 order, WORD32 shift_value,
                                        WORD32 *ptr_filter_state) {
  WORD32 i, j;
  WORD32 y;
  WORD64 acc;

  if ((order & 3) != 0) {
    for (i = order + 1; i < ((WORD32)(order & 0xfffffffc) + 4); i++) {
      lpc_coeff[i] = 0;
    }
    lpc_coeff[i] = 0;
    order = ((order & 0xfffffffc) + 4);
  }

  for (i = 0; i < order; i++) {
    y = *spectrum;
    acc = 0;

    for (j = i; j > 0; j--) {
      acc = ixheaac_add64_sat(
          acc, ixheaac_mult64(ptr_filter_state[j - 1], lpc_coeff[j]));
      ptr_filter_state[j] = ptr_filter_state[j - 1];
    }

    y = ixheaac_sub32_sat(y, (WORD32)(acc >> 31));
    ptr_filter_state[0] = ixheaac_shl32(y, shift_value);
    *spectrum = y;
    spectrum += inc;
  }

  for (i = order; i < size; i++) {
    y = *spectrum;
    acc = 0;
    for (j = order; j > 0; j--) {
      acc = ixheaac_add64_sat(
          acc, ixheaac_mult64(ptr_filter_state[j - 1], lpc_coeff[j]));
      ;
      ptr_filter_state[j] = ptr_filter_state[j - 1];
    }
    y = ixheaac_sub32_sat(y, (WORD32)(acc >> 31));
    ptr_filter_state[0] = ixheaac_shl32(y, shift_value);
    *spectrum = y;
    spectrum += inc;
  }
}

IA_ERRORCODE ixheaacd_tns_apply(ia_usac_data_struct *usac_data, WORD32 *spec,
                                WORD32 nbands,
                                ia_sfb_info_struct *pstr_sfb_info,
                                ia_tns_frame_info_struct *pstr_tns) {
  WORD32 f, start, stop, size, inc;
  WORD32 n_filt, coef_res, order, direction;
  WORD32 *ptr_spec;
  WORD32 scale_spec;
  WORD32 scale_lpc;
  WORD32 guard_band;
  WORD32 shift;
  WORD32 lpc_coeff[TNS_MAX_ORDER + 1];
  WORD32 par_coeff[TNS_MAX_ORDER + 1];
  ia_tns_filter_struct *filt;

  const WORD16 *sfb_top;

  WORD32 nbins = (pstr_sfb_info->islong) ? 1024 : 128;
  WORD32 i, j, idx;

  idx = (pstr_sfb_info->islong) ? 0 : 1;

  ptr_spec = &usac_data->scratch_buffer[0];

  for (j = 0; j < pstr_tns->n_subblocks; j++) {
    sfb_top = pstr_sfb_info->ptr_sfb_tbl;

    for (i = 0; i < nbins; i++) {
      ptr_spec[i] = spec[i];
    }

    if (pstr_tns->str_tns_info[j].n_filt) {
      n_filt = pstr_tns->str_tns_info[j].n_filt;

      for (f = 0; f < n_filt; f++) {
        WORD32 tmp;

        coef_res = pstr_tns->str_tns_info[j].coef_res;
        filt = &pstr_tns->str_tns_info[j].str_filter[f];
        order = filt->order;
        direction = filt->direction;
        start = filt->start_band;
        stop = filt->stop_band;

        if (!order) continue;

        ixheaacd_tns_dec_coef_usac(usac_data, filt, coef_res,
                                   (WORD32 *)par_coeff);

        ixheaacd_tns_parcor_lpc_convert_usac(par_coeff, lpc_coeff, &scale_lpc,
                                             filt->order);

        tmp = (*usac_data->tns_max_bands_tbl_usac)[usac_data->sampling_rate_idx]
                                                  [idx];

        start = ixheaac_min32(start, tmp);

        start = ixheaac_min32(start, nbands);
        if (start > pstr_sfb_info->sfb_per_sbk) return -1;
        start = sfb_offset(start);

        stop = ixheaac_min32(stop, tmp);
        stop = ixheaac_min32(stop, nbands);
        if (stop > pstr_sfb_info->sfb_per_sbk) return -1;
        stop = sfb_offset(stop);

        guard_band = 31 - ixheaac_norm32(filt->order);

        if ((size = stop - start) <= 0) continue;

        if (direction) {
          inc = -1;
          shift = stop - 1;
        }

        else {
          inc = 1;
          shift = start;
        }

        {
          WORD32 *ptr_temp = ptr_spec + start;
          scale_spec = (*ixheaacd_calc_max_spectral_line)(ptr_temp, size);
        }

        scale_spec = ((scale_spec - guard_band) - scale_lpc);

        if (scale_spec > 0) {
          ixheaacd_tns_ar_filter_usac(&ptr_spec[shift], size, inc, lpc_coeff,
                                      filt->order, scale_lpc,
                                      usac_data->x_ac_dec);
        }

        else {
          WORD32 *ptr_temp = ptr_spec + start;

          scale_spec = -scale_spec;
          scale_spec = ixheaac_min32(scale_spec, 31);

          for (i = size; i != 0; i--) {
            *ptr_temp = *ptr_temp >> scale_spec;
            ptr_temp++;
          }

          {
            ixheaacd_tns_ar_filter_usac(&ptr_spec[shift], size, inc, lpc_coeff,
                                        filt->order, scale_lpc,
                                        usac_data->x_ac_dec);
          }

          {
            ptr_temp = ptr_spec + start;
            i = size;

            do {
              *ptr_temp = *ptr_temp << scale_spec;
              ptr_temp++;
              i--;
            } while (i != 0);
          }
        }

        for (i = start; i <= stop - 1; i++) {
          spec[i] = ptr_spec[i];
        }
      }
    }

    spec += pstr_sfb_info->bins_per_sbk;
  }
  return 0;
}
