/******************************************************************************
 *
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
#include "ixheaac_constants.h"
#include "ixheaacd_cnst.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops16.h"
#include "ixheaacd_bitbuffer.h"
#include "ixheaacd_mps_aac_struct.h"
#include "ixheaacd_mps_res_rom.h"
#include "ixheaacd_mps_res_channelinfo.h"
#include "ixheaacd_mps_res_tns.h"

static PLATFORM_INLINE WORD16 ixheaacd_res_get_maximum_tns_bands(
    ia_mps_dec_residual_ics_info_struct *p_ics_info,
    ia_mps_dec_residual_aac_tables_struct *aac_tables_ptr, WORD32 *win_len) {
  WORD32 i = 0;
  *win_len = 1;

  if (p_ics_info->window_sequence == EIGHT_SHORT_SEQUENCE) {
    *win_len = 8;
    i = 1;
  }

  return aac_tables_ptr->res_block_tables_ptr
      ->tns_max_bands_tbl[p_ics_info->sampling_rate_index][i];
}

VOID ixheaacd_res_tns_decode_coeffs_32x16(const ia_mps_dec_residual_filter_struct *filter,
                                          WORD16 *a,
                                          ia_mps_dec_residual_aac_tables_struct *aac_tables_ptr) {
  WORD tmp;
  WORD16 *aptr = a;
  WORD16 *tns_coeff_ptr;
  WORD8 offset = 4;
  WORD8 *p_coeff = (WORD8 *)&filter->coeff[0];
  WORD32 tmp1;

  tmp = filter->resolution;
  tns_coeff_ptr = aac_tables_ptr->res_block_tables_ptr->tns_coeff3_16;
  if (tmp) {
    tns_coeff_ptr = aac_tables_ptr->res_block_tables_ptr->tns_coeff4_16;
    offset = offset << 1;
  }
  tmp1 = filter->order;
  do {
    WORD8 temp = *p_coeff++;
    *aptr++ = tns_coeff_ptr[temp + offset];
    tmp1--;
  } while (tmp1 != 0);
}

VOID ixheaacd_res_ctns_apply(ia_mps_dec_residual_channel_info_struct *p_aac_decoder_channel_info,
                             WORD16 max_sfb,
                             ia_mps_dec_residual_aac_tables_struct *aac_tables_ptr) {
  WORD i;
  WORD16 scale_lpc;

  ia_mps_dec_residual_tns_data *p_tns_data = &p_aac_decoder_channel_info->tns_data;
  WORD32 *p_spectrum = p_aac_decoder_channel_info->p_spectral_coefficient;

  WORD window, index, start, stop, size, scale_spec;
  ia_mps_dec_residual_ics_info_struct *p_ics_info = &p_aac_decoder_channel_info->ics_info;
  WORD win_len, tns_max_bands;
  WORD16 maximum_bins_short = ixheaac_shr16_dir_sat(p_ics_info->frame_length, 3);

  WORD32 coeff_parc[MAX_ORDER + 1];
  WORD32 lpc[MAX_ORDER + 1];

  const WORD16 *scale_factor_bands_tbl;

  if (!p_tns_data->tns_data_present) return;

  tns_max_bands = ixheaacd_res_get_maximum_tns_bands(p_ics_info, aac_tables_ptr, &win_len);

  scale_factor_bands_tbl =
      ixheaacd_res_get_sfb_offsets(&p_aac_decoder_channel_info->ics_info, aac_tables_ptr);

  for (window = 0; window < win_len; window++) {
    WORD ind_len = p_tns_data->number_of_filters[window];

    for (index = 0; index < ind_len; index++) {
      ia_mps_dec_residual_filter_struct *filter = &p_tns_data->filter[window][index];

      if (filter->order <= 0 || filter->order > MAX_ORDER_LONG) continue;

      ixheaacd_res_tns_decode_coeffs_32x16(filter, (WORD16 *)coeff_parc, aac_tables_ptr);

      start = ixheaac_min32(ixheaac_min32(filter->start_band, tns_max_bands), max_sfb);

      start = scale_factor_bands_tbl[start];

      stop = ixheaac_min32(ixheaac_min32(filter->stop_band, tns_max_bands), max_sfb);

      stop = scale_factor_bands_tbl[stop];

      size = (stop - start);
      if (size <= 0) continue;

      ixheaacd_res_tns_parcor_2_lpc_32x16((WORD16 *)coeff_parc, (WORD16 *)lpc, &scale_lpc,
                                          filter->order);
      {
        WORD32 *p_tmp = p_spectrum + (window * maximum_bins_short) + start;
        scale_spec = ixheaacd_res_calc_max_spectral_line(p_tmp, size);
      }

      scale_spec = ((scale_spec - 4) - scale_lpc);

      if (scale_spec > 0) {
        WORD shift;

        scale_spec = ixheaac_min32(scale_spec, 31);

        if (filter->direction == -1)
          shift = stop - 1;
        else
          shift = start;

        ixheaacd_res_tns_ar_filter_fixed_32x16(&p_spectrum[(window * maximum_bins_short) + shift],
                                               size, filter->direction, (WORD16 *)lpc,
                                               filter->order, (WORD32)scale_lpc, scale_spec);
      } else {
        WORD shift;
        WORD32 *p_tmp = p_spectrum + (window * maximum_bins_short) + start;

        scale_spec = -scale_spec;
        scale_spec = ixheaac_min32(scale_spec, 31);

        for (i = size; i != 0; i--) {
          *p_tmp = (*p_tmp >> scale_spec);
          p_tmp++;
        }

        if (filter->direction == -1)
          shift = stop - 1;
        else
          shift = start;

        {
          WORD32 shift_val = scale_lpc;

          ixheaacd_res_tns_ar_filter_fixed_32x16(
              &p_spectrum[(window * maximum_bins_short) + shift], size, filter->direction,
              (WORD16 *)lpc, filter->order, shift_val, 0);
        }
        {
          p_tmp = p_spectrum + (window * maximum_bins_short) + start;
          i = size;
          do {
            *p_tmp = (*p_tmp << scale_spec);
            p_tmp++;
            i--;
          } while (i != 0);
        }
      }
    }
  }
}
