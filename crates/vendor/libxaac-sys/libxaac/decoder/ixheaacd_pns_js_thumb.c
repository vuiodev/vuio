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
#include <string.h>
#include <stdio.h>
#include <stdlib.h>
#include "ixheaacd_sbr_common.h"
#include "ixheaac_type_def.h"
#include "ixheaac_constants.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops16.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaac_basic_ops.h"

#include "ixheaacd_bitbuffer.h"

#include "ixheaacd_error_codes.h"
#include "ixheaacd_defines.h"
#include "ixheaacd_aac_rom.h"
#include "ixheaacd_common_rom.h"
#include "ixheaacd_basic_funcs.h"
#include "ixheaacd_aac_imdct.h"
#include "ixheaac_basic_op.h"
#include "ixheaacd_intrinsics.h"

#include "ixheaacd_pulsedata.h"

#include "ixheaacd_pns.h"
#include "ixheaacd_drc_data_struct.h"

#include "ixheaacd_lt_predict.h"
#include "ixheaacd_cnst.h"
#include "ixheaacd_ec_defines.h"
#include "ixheaacd_ec_struct_def.h"
#include "ixheaacd_channelinfo.h"
#include "ixheaacd_drc_dec.h"
#include "ixheaacd_sbrdecoder.h"
#include "ixheaacd_block.h"

#include "ixheaacd_channel.h"

#include "ixheaacd_audioobjtypes.h"
#include "ixheaacd_latmdemux.h"
#include "ixheaacd_aacdec.h"
#include "ixheaacd_tns.h"
#include "ixheaacd_function_selector.h"

static PLATFORM_INLINE WORD16 ixheaacd_is_correlation(
    ia_aac_dec_channel_info_struct *ptr_aac_dec_channel_info, WORD16 pns_band) {
  ia_pns_correlation_info_struct *ptr_corr_info =
      ptr_aac_dec_channel_info->pstr_pns_corr_info;

  return ((ptr_corr_info->correlated[(pns_band >> PNS_BAND_FLAGS_SHIFT)] >>
           (pns_band & PNS_BAND_FLAGS_MASK)) &
          1);
}

VOID ixheaacd_gen_rand_vec(WORD32 scale, WORD shift, WORD32 *ptr_spec_coef,
                           WORD32 sfb_width, WORD32 *seed) {
  WORD nrg_scale;
  WORD32 nrg = 0;
  WORD32 *spec = ptr_spec_coef;
  WORD32 sfb;

  for (sfb = 0; sfb <= sfb_width; sfb++) {
    *seed = (WORD32)(((WORD64)1664525 * (WORD64)(*seed)) + (WORD64)1013904223);

    *spec = (*seed >> 3);

    nrg = ixheaac_add32_sat(nrg, ixheaac_mult32_shl_sat(*spec, *spec));

    spec++;
  }

  nrg_scale = ixheaac_norm32(nrg);

  if (nrg_scale > 0) {
    nrg_scale &= ~1;
    nrg = ixheaac_shl32_sat(nrg, nrg_scale);
    shift = shift - (nrg_scale >> 1);
  }

  nrg = ixheaacd_sqrt(nrg);
  scale = ixheaac_div32_pos_normb(scale, nrg);

  spec = ptr_spec_coef;

  if (shift < -31) {
    shift = -31;
  }
  for (sfb = 0; sfb <= sfb_width; sfb++) {
    *spec = ixheaac_shr32_dir_sat_limit(ixheaac_mult32_shl_sat(*spec, scale),
                                         shift);
    spec++;
  }
}

VOID ixheaacd_pns_process(ia_aac_dec_channel_info_struct *ptr_aac_dec_channel_info[CHANNELS],
                          WORD32 channel, ia_aac_dec_tables_struct *ptr_aac_tables) {
  ia_pns_info_struct *ptr_pns_info =
      &ptr_aac_dec_channel_info[channel]->str_pns_info;
  ia_ics_info_struct *ptr_ics_info =
      &ptr_aac_dec_channel_info[channel]->str_ics_info;
  WORD16 maximum_bins_short = ptr_ics_info->frame_length >> 3;
  WORD32 *ptr_scale_mant_tab =
      ptr_aac_tables->pstr_block_tables->scale_mant_tab;

  if (ptr_pns_info->pns_active) {
    const WORD16 *swb_offset =
        ptr_aac_tables->str_aac_sfb_info[ptr_ics_info->window_sequence]
            .sfb_index;

    WORD num_win_group, grp_len, sfb;
    WORD32 *spec = &ptr_aac_dec_channel_info[channel]->ptr_spec_coeff[0];

    for (num_win_group = 0; num_win_group < ptr_ics_info->num_window_groups;
         num_win_group++) {
      grp_len = ptr_ics_info->window_group_length[num_win_group];

      for (grp_len = 0;
           grp_len < ptr_ics_info->window_group_length[num_win_group];
           grp_len++) {
        for (sfb = 0; sfb < ptr_ics_info->max_sfb; sfb++) {
          WORD16 pns_band = ((num_win_group << 4) + sfb);

          if (ptr_aac_dec_channel_info[channel]
                  ->str_pns_info.pns_used[pns_band]) {
            WORD32 scale_mant;
            WORD32 scale_exp;
            WORD32 sfb_width = swb_offset[sfb + 1] - swb_offset[sfb] - 1;
            WORD32 *ptr_spec = &spec[swb_offset[sfb]];

            scale_mant = ptr_scale_mant_tab[ptr_aac_dec_channel_info[channel]
                                                ->ptr_scale_factor[pns_band] &
                                            PNS_SCALE_MANT_TAB_MASK];
            scale_exp = add_d(sub_d(31, (ptr_aac_dec_channel_info[channel]
                                             ->ptr_scale_factor[pns_band] >>
                                         PNS_SCALEFACTOR_SCALING)),
                              PNS_SCALE_MANT_TAB_SCALING);

            if (ixheaacd_is_correlation(ptr_aac_dec_channel_info[LEFT],
                                        pns_band)) {
              if (channel == 0) {
                ptr_aac_dec_channel_info[LEFT]
                    ->pstr_pns_corr_info->random_vector[pns_band] =
                    ptr_aac_dec_channel_info[LEFT]
                        ->pstr_pns_rand_vec_data->current_seed;

                ixheaacd_gen_rand_vec(
                    scale_mant, scale_exp, ptr_spec, sfb_width,
                    &(ptr_aac_dec_channel_info[LEFT]
                          ->pstr_pns_rand_vec_data->current_seed));
              }

              else {
                ixheaacd_gen_rand_vec(
                    scale_mant, scale_exp, ptr_spec, sfb_width,
                    &(ptr_aac_dec_channel_info[LEFT]
                          ->pstr_pns_corr_info->random_vector[pns_band]));
              }

            }

            else {
              ixheaacd_gen_rand_vec(
                  scale_mant, scale_exp, ptr_spec, sfb_width,
                  &(ptr_aac_dec_channel_info[LEFT]
                        ->pstr_pns_rand_vec_data->current_seed));
            }
          }
        }

        if (maximum_bins_short == 120)
          spec += maximum_bins_short;
        else
          spec += 128;
      }
    }
  }

  if (channel == 0) {
    ptr_aac_dec_channel_info[0]->pstr_pns_rand_vec_data->pns_frame_number++;
  }
}

VOID ixheaacd_tns_decode_coef(const ia_filter_info_struct *filter,
                              WORD16 *parcor_coef,
                              ia_aac_dec_tables_struct *ptr_aac_tables) {
  WORD order, resolution;
  WORD16 *ptr_par_coef = parcor_coef;
  WORD16 *tns_coeff_ptr;
  WORD8 ixheaacd_drc_offset = 4;
  WORD8 *ptr_coef = (WORD8 *)filter->coef;

  resolution = filter->resolution;
  tns_coeff_ptr = ptr_aac_tables->pstr_block_tables->tns_coeff3_16;

  if (resolution) {
    tns_coeff_ptr = ptr_aac_tables->pstr_block_tables->tns_coeff4_16;
    ixheaacd_drc_offset = ixheaacd_drc_offset << 1;
  }

  for (order = 0; order < filter->order; order++) {
    WORD8 temp = *ptr_coef++;
    *ptr_par_coef++ = tns_coeff_ptr[temp + ixheaacd_drc_offset];
  }
}

VOID ixheaacd_tns_decode_coef_ld(const ia_filter_info_struct *filter,
                                 WORD32 *parcor_coef,
                                 ia_aac_dec_tables_struct *ptr_aac_tables) {
  WORD order, resolution;
  WORD32 *ptr_par_coef = parcor_coef;
  WORD32 *tns_coeff_ptr;
  WORD8 offset = 4;
  WORD8 *ptr_coef = (WORD8 *)filter->coef;

  resolution = filter->resolution;
  tns_coeff_ptr = ptr_aac_tables->pstr_block_tables->tns_coeff3;

  if (resolution) {
    tns_coeff_ptr = ptr_aac_tables->pstr_block_tables->tns_coeff4;
    offset = offset << 1;
  }

  for (order = 0; order < filter->order; order++) {
    WORD8 temp = *ptr_coef++;
    *ptr_par_coef++ = tns_coeff_ptr[temp + offset];
  }
}

VOID ixheaacd_aac_tns_process(
    ia_aac_dec_channel_info_struct *ptr_aac_dec_channel_info, WORD32 num_ch,
    ia_aac_dec_tables_struct *ptr_aac_tables, WORD32 object_type,
    WORD32 ar_flag, WORD32 *predicted_spectrum) {
  WORD i;
  WORD16 scale_lpc;

  ia_tns_info_aac_struct *ptr_tns_info =
      &ptr_aac_dec_channel_info->str_tns_info;
  WORD32 *spec = ptr_aac_dec_channel_info->ptr_spec_coeff;
  WORD32 *scratch_buf = ptr_aac_dec_channel_info->scratch_buf_ptr;

  WORD win, filt, start, stop, size, scale_spec;
  ia_ics_info_struct *ptr_ics_info = &ptr_aac_dec_channel_info->str_ics_info;
  WORD num_window, tns_max_bands, win_seq;
  WORD16 maximum_bins_short = ptr_ics_info->frame_length >> 3;
  WORD position;

  WORD32 parcor_coef[MAX_ORDER + 1];
  WORD16 parcor_coef_16[MAX_ORDER + 1];

  WORD32 lpc_coef[MAX_ORDER + 1];
  WORD16 lpc_coef_16[MAX_ORDER + 1];

  const WORD16 *ptr_sfb_table;

  WORD16 max_bin_long = ptr_ics_info->frame_length;

  win_seq = ptr_ics_info->window_sequence == 0
                ? 0
                : (ptr_ics_info->window_sequence % 2 == 0);

  if (ar_flag)
    spec = ptr_aac_dec_channel_info->ptr_spec_coeff;
  else {
    spec = predicted_spectrum;
  }

  if (object_type == AOT_ER_AAC_ELD || object_type == AOT_ER_AAC_LD ||
      object_type == AOT_AAC_LTP) {
    if (512 == ptr_ics_info->frame_length) {
      tns_max_bands =
          ptr_aac_tables->pstr_block_tables
              ->tns_max_bands_tbl_ld[ptr_ics_info->sampling_rate_index];
      win_seq = 1;
      num_window = win_seq;
    } else if (480 == ptr_ics_info->frame_length) {
      tns_max_bands =
          ptr_aac_tables->pstr_block_tables
              ->tns_max_bands_tbl_480[ptr_ics_info->sampling_rate_index];
      win_seq = 1;
      num_window = win_seq;
    } else {
      tns_max_bands =
          ptr_aac_tables->pstr_block_tables
              ->tns_max_bands_tbl[ptr_ics_info->sampling_rate_index][win_seq];

      num_window = win_seq ? 8 : 1;
    }
  } else {
    tns_max_bands =
        ptr_aac_tables->pstr_block_tables
            ->tns_max_bands_tbl[ptr_ics_info->sampling_rate_index][win_seq];

    num_window = win_seq ? 8 : 1;
  }

  ptr_sfb_table =
      ptr_aac_tables->str_aac_sfb_info[ptr_ics_info->window_sequence].sfb_index;

  for (win = 0; win < num_window; win++) {
    WORD n_filt = ptr_tns_info->n_filt[win];

    for (filt = 0; filt < n_filt; filt++) {
      ia_filter_info_struct *filter = &ptr_tns_info->str_filter[win][filt];

      if (filter->order <= 0) {
        continue;
      }

      if ((object_type == AOT_ER_AAC_LD) || (object_type == AOT_AAC_LTP) ||
          (num_ch > 2)) {
        ixheaacd_tns_decode_coefficients(filter, parcor_coef, ptr_aac_tables);

      } else {
        ixheaacd_tns_decode_coef(filter, parcor_coef_16, ptr_aac_tables);
      }

      start = ixheaac_min32(ixheaac_min32(filter->start_band, tns_max_bands),
                             ptr_ics_info->max_sfb);

      start = ptr_sfb_table[start];

      stop = ixheaac_min32(ixheaac_min32(filter->stop_band, tns_max_bands),
                            ptr_ics_info->max_sfb);

      stop = ptr_sfb_table[stop];

      size = (stop - start);

      if (size <= 0) {
        continue;
      }
      if ((object_type == AOT_ER_AAC_LD) || (object_type == AOT_AAC_LTP) ||
          (num_ch > 2)) {
        ixheaacd_tns_parcor_to_lpc(parcor_coef, lpc_coef, &scale_lpc,
                                   filter->order);

      } else {
        (*ixheaacd_tns_parcor_lpc_convert)(parcor_coef_16, lpc_coef_16,
                                           &scale_lpc, filter->order);
      }

      {
        WORD32 *ptr_tmp;

        if (maximum_bins_short == 120)
          ptr_tmp = spec + (win * maximum_bins_short) + start;
        else
          ptr_tmp = spec + (win << 7) + start;

        scale_spec = (*ixheaacd_calc_max_spectral_line)(ptr_tmp, size);
      }

      if (filter->direction == -1) {
        position = stop - 1;

        if (maximum_bins_short == 120) {
          if (((win * maximum_bins_short) + position) < filter->order) continue;
        } else {
          if (((win << 7) + position) < filter->order) continue;
        }

      } else {
        position = start;
        if (maximum_bins_short == 120) {
          if ((((win * maximum_bins_short) + position) + filter->order) > max_bin_long)
            continue;
        } else {
          if ((((win << 7) + position) + filter->order) > MAX_BINS_LONG) continue;
        }
      }

      if ((num_ch <= 2) &&
          ((object_type != AOT_ER_AAC_LD) && (object_type != AOT_AAC_LTP)))
        scale_spec = ((scale_spec - 4) - scale_lpc);
      else {
        if (scale_spec > 17)
          scale_spec = ((scale_spec - 6) - scale_lpc);
        else if (scale_spec > 11)
          scale_spec = ((scale_spec - 5) - scale_lpc);
        else
          scale_spec = ((scale_spec - 4) - scale_lpc);
      }

      if (scale_spec > 0) {
        scale_spec = ixheaac_min32(scale_spec, 31);

        if ((object_type == AOT_ER_AAC_LD) || (object_type == AOT_AAC_LTP) ||
            (num_ch > 2)) {
          if (ar_flag)
          {
            if (maximum_bins_short == 120) {
              (*ixheaacd_tns_ar_filter_fixed)(&spec[(win * maximum_bins_short) + position],
                                              size, filter->direction,
                                              (WORD32 *)lpc_coef, filter->order,
                                              (WORD32)scale_lpc, scale_spec);
            } else {
              (*ixheaacd_tns_ar_filter_fixed)(&spec[(win << 7) + position], size,
                                              filter->direction,
                                              (WORD32 *)lpc_coef, filter->order,
                                              (WORD32)scale_lpc, scale_spec);
            }
          } else {
            if (maximum_bins_short == 120) {
              ixheaacd_tns_ma_filter_fixed_ld(&spec[(win * maximum_bins_short) + position],
                                              size, filter->direction, lpc_coef,
                                              filter->order, scale_lpc);
            } else {
              ixheaacd_tns_ma_filter_fixed_ld(&spec[(win << 7) + position], size,
                                              filter->direction, lpc_coef,
                                              filter->order, scale_lpc);
            }
          }
        } else {
          if (object_type == AOT_ER_AAC_ELD) scale_spec = scale_spec - 1;
          if (maximum_bins_short == 120) {
            (*ixheaacd_tns_ar_filter)(&spec[(win * maximum_bins_short) + position], size,
                                      filter->direction, lpc_coef_16,
                                      filter->order, (WORD32)scale_lpc,
                                      scale_spec, scratch_buf);
          } else {
            (*ixheaacd_tns_ar_filter)(&spec[(win << 7) + position], size,
                                      filter->direction, lpc_coef_16,
                                      filter->order, (WORD32)scale_lpc,
                                      scale_spec, scratch_buf);
          }
        }
      }

      else {
        WORD32 *ptr_tmp;

        if (maximum_bins_short == 120)
          ptr_tmp = spec + (win * maximum_bins_short) + start;
        else
          ptr_tmp = spec + (win >> 7) + start;

        scale_spec = -scale_spec;
        scale_spec = ixheaac_min32(scale_spec, 31);

        for (i = size; i != 0; i--) {
          *ptr_tmp = (*ptr_tmp >> scale_spec);
          ptr_tmp++;
        }

        if ((object_type == AOT_ER_AAC_LD) || (object_type == AOT_AAC_LTP) ||
            num_ch > 2) {
          if (ar_flag) {
            if (maximum_bins_short == 120) {
              (*ixheaacd_tns_ar_filter_fixed)(
                  &spec[(win * maximum_bins_short) + position], size, filter->direction,
                  (WORD32 *)lpc_coef, filter->order, scale_lpc, 0);
            } else {
              (*ixheaacd_tns_ar_filter_fixed)(
                  &spec[(win << 7) + position], size, filter->direction,
                  (WORD32 *)lpc_coef, filter->order, scale_lpc, 0);
            }
          } else {
            if (maximum_bins_short == 120) {
              ixheaacd_tns_ma_filter_fixed_ld(&spec[(win * maximum_bins_short) + position],
                                              size, filter->direction, lpc_coef,
                                              filter->order, scale_lpc);
            } else {
              ixheaacd_tns_ma_filter_fixed_ld(&spec[(win << 7) + position], size,
                                              filter->direction, lpc_coef,
                                              filter->order, scale_lpc);
            }
          }
        } else {
          if (object_type == AOT_ER_AAC_ELD) {
            scale_lpc = scale_lpc - 1;
          }

          if (maximum_bins_short == 120) {
            (*ixheaacd_tns_ar_filter)(&spec[(win * maximum_bins_short) + position], size,
                                      filter->direction, lpc_coef_16,
                                      filter->order, scale_lpc, 0, scratch_buf);
          } else {
            (*ixheaacd_tns_ar_filter)(&spec[(win << 7) + position], size,
                                      filter->direction, lpc_coef_16,
                                      filter->order, scale_lpc, 0, scratch_buf);
          }
        }

        if (maximum_bins_short == 120)
          ptr_tmp = spec + (win * maximum_bins_short) + start;
        else
          ptr_tmp = spec + (win << 7) + start;

        for (i = size; i != 0; i--) {
          *ptr_tmp = (*ptr_tmp << scale_spec);
          ptr_tmp++;
        }
      }
    }
  }
}