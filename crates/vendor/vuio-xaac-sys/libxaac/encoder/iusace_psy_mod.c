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
#include "iusace_cnst.h"
#include "iusace_type_def.h"
#include "iusace_block_switch_const.h"
#include "iusace_block_switch_struct_def.h"
#include "iusace_bitbuffer.h"
#include "iusace_tns_usac.h"
#include "impd_drc_common_enc.h"
#include "impd_drc_uni_drc.h"
#include "impd_drc_api.h"
#include "impd_drc_uni_drc_eq.h"
#include "impd_drc_uni_drc_filter_bank.h"
#include "impd_drc_gain_enc.h"
#include "impd_drc_struct_def.h"
#include "iusace_cnst.h"
#include "iusace_tns_usac.h"
#include "iusace_psy_mod.h"
#include "iusace_psy_utils.h"

#include "ixheaac_error_standards.h"
#include "iusace_type_def.h"
#include "iusace_cnst.h"

#include "iusace_ms.h"

#include "iusace_psy_utils.h"
#include "ixheaace_adjust_threshold_data.h"
#include "iusace_fd_qc_util.h"
#include "ixheaace_memory_standards.h"
#include "iusace_config.h"
#include "iusace_arith_enc.h"
#include "iusace_fd_quant.h"
#include "iusace_signal_classifier.h"
#include "iusace_block_switch_const.h"
#include "ixheaace_sbr_header.h"
#include "ixheaace_config.h"
#include "ixheaace_asc_write.h"
#include "iusace_main.h"

VOID iusace_psy_mod_init(ia_psy_mod_struct *pstr_psy_mod, WORD32 sample_rate, WORD32 bit_rate,
                         WORD32 band_width, WORD32 num_channels, WORD32 ch, WORD32 ele_id,
                         WORD32 ccfl) {
  WORD32 i;

  for (i = 0; i < num_channels; i++) {
    iusace_psy_long_config_init(bit_rate / num_channels, sample_rate, band_width,
                                &(pstr_psy_mod->str_psy_long_config[ele_id]), ccfl);

    iusace_psy_short_config_init(bit_rate / num_channels, sample_rate, band_width,
                                 &(pstr_psy_mod->str_psy_short_config[ele_id]), ccfl);

    pstr_psy_mod->str_psy_data[ch].ptr_sfb_thr_long =
        (FLOAT32 *)pstr_psy_mod->str_psy_data[ch].sfb_thr_short;
    pstr_psy_mod->str_psy_data[ch].ptr_sfb_energy_long =
        (FLOAT32 *)pstr_psy_mod->str_psy_data[ch].sfb_energy_short;
    pstr_psy_mod->str_psy_data[ch].ptr_sfb_spreaded_energy_long =
        (FLOAT32 *)pstr_psy_mod->str_psy_data[ch].sfb_spreaded_energy_short;

    memcpy(pstr_psy_mod->str_psy_data[ch].sfb_thr_nm1,
           pstr_psy_mod->str_psy_long_config[ele_id].sfb_thr_quiet,
           pstr_psy_mod->str_psy_long_config[ele_id].sfb_count * sizeof(FLOAT32));

    ch++;
  }

  return;
}

VOID iusace_psy_mod_lb(ia_psy_mod_struct *pstr_psy_mod, ia_sfb_params_struct *pstr_sfb_prms,
                       FLOAT64 *ptr_spec_in, ia_tns_info *pstr_tns_info[MAX_TIME_CHANNELS],
                       WORD32 tns_select, WORD32 i_ch, WORD32 chn, WORD32 channel_type,
                       FLOAT64 *scratch_tns_filter, WORD32 elem_idx, FLOAT64 *ptr_tns_scratch,
                       WORD32 ccfl) {
  ia_psy_mod_data_struct *pstr_psy_data = &(pstr_psy_mod->str_psy_data[i_ch]);
  ia_psy_mod_long_config_struct *pstr_psy_config = &(pstr_psy_mod->str_psy_long_config[elem_idx]);
  WORD32 window_sequence = pstr_sfb_prms->window_sequence[i_ch];
  WORD32 num_sfb = pstr_sfb_prms->num_sfb[i_ch];
  ia_tns_info *ptr_tns_info = pstr_tns_info[i_ch];
  WORD32 sfb, line;
  WORD32 i;
  WORD32 frame_len_long = ccfl;
  FLOAT32 energy_shift = 0.25f;
  FLOAT32 clip_energy = pstr_psy_config->clip_energy * energy_shift;
  (VOID) channel_type;

  pstr_psy_data->window_sequence = window_sequence;
  memset(&ptr_spec_in[pstr_psy_config->low_pass_line], 0,
         (frame_len_long - pstr_psy_config->low_pass_line) * sizeof(FLOAT64));

  iusace_calc_band_energy(ptr_spec_in, pstr_psy_config->sfb_offset, pstr_psy_config->sfb_active,
                          pstr_psy_data->ptr_sfb_energy_long, pstr_psy_config->sfb_count);

  if (tns_select != 0) {
    ia_tns_info *ptr_tns_info_ch2 = pstr_tns_info[i_ch - chn];
    ptr_tns_info->number_of_bands = num_sfb;
    ptr_tns_info->block_type = window_sequence;
    ptr_tns_info->spec = ptr_spec_in;
    iusace_tns_encode(ptr_tns_info_ch2, ptr_tns_info, pstr_psy_data->ptr_sfb_energy_long, 0, chn,
                      pstr_psy_config->low_pass_line, scratch_tns_filter, 0, ptr_tns_scratch);
  }

  for (i = 0; i < pstr_psy_config->sfb_count; i++) {
    pstr_psy_data->ptr_sfb_thr_long[i] =
        pstr_psy_data->ptr_sfb_energy_long[i] * pstr_psy_config->ratio;
    pstr_psy_data->ptr_sfb_thr_long[i] = MIN(pstr_psy_data->ptr_sfb_thr_long[i], clip_energy);
  }

  if (tns_select != 0) {
    if (ptr_tns_info->tns_data_present == 1) {
      iusace_calc_band_energy(ptr_spec_in, pstr_psy_config->sfb_offset,
                              pstr_psy_config->sfb_active, pstr_psy_data->ptr_sfb_energy_long,
                              pstr_psy_config->sfb_count);
    }
  }

  iusace_find_max_spreading(pstr_psy_config->sfb_count, pstr_psy_config->sfb_mask_low_fac,
                            pstr_psy_config->sfb_mask_high_fac, pstr_psy_data->ptr_sfb_thr_long);

  for (i = 0; i < pstr_psy_config->sfb_count; i++) {
    pstr_psy_data->ptr_sfb_thr_long[i] = MAX(pstr_psy_data->ptr_sfb_thr_long[i],
                                             (pstr_psy_config->sfb_thr_quiet[i] * energy_shift));
  }

  if (pstr_psy_data->window_sequence == LONG_STOP_SEQUENCE) {
    for (i = 0; i < pstr_psy_config->sfb_count; i++) {
      pstr_psy_data->sfb_thr_nm1[i] = 1.0e20f;
    }
  }

  iusace_pre_echo_control(pstr_psy_data->sfb_thr_nm1, pstr_psy_config->sfb_count,
                          pstr_psy_config->max_allowed_inc_fac,
                          pstr_psy_config->min_remaining_thr_fac,
                          pstr_psy_data->ptr_sfb_thr_long);

  if (pstr_psy_data->window_sequence == LONG_START_SEQUENCE) {
    for (i = 0; i < pstr_psy_config->sfb_count; i++) {
      pstr_psy_data->sfb_thr_nm1[i] = 1.0e20f;
    }
  }

  for (i = 0; i < pstr_psy_config->sfb_count; i++) {
    pstr_psy_data->ptr_sfb_spreaded_energy_long[i] = pstr_psy_data->ptr_sfb_energy_long[i];
  }
  iusace_find_max_spreading(
      pstr_psy_config->sfb_count, pstr_psy_config->sfb_mask_low_fac_spr_ener,
      pstr_psy_config->sfb_mask_high_fac_spr_ener, pstr_psy_data->ptr_sfb_spreaded_energy_long);

  for (sfb = pstr_psy_config->sfb_count - 1; sfb >= 0; sfb--) {
    for (line = pstr_psy_config->sfb_offset[sfb + 1] - 1;
         line >= pstr_psy_config->sfb_offset[sfb]; line--) {
      if (ptr_spec_in[line] != 0) break;
    }
    if (line >= pstr_psy_config->sfb_offset[sfb]) break;
  }

  pstr_psy_mod->str_psy_out_data[i_ch].max_sfb_per_grp = sfb + 1;

  return;
}

VOID iusace_psy_mod_sb(ia_psy_mod_struct *pstr_psy_mod, ia_sfb_params_struct *pstr_sfb_prms,
                       FLOAT64 *ptr_spec_in, ia_tns_info *pstr_tns_info[MAX_TIME_CHANNELS],
                       WORD32 tns_select, WORD32 i_ch, WORD32 chn, WORD32 channel_type,
                       FLOAT64 *scratch_tns_filter, WORD32 elem_idx, FLOAT64 *ptr_tns_scratch,
                       WORD32 ccfl) {
  ia_psy_mod_data_struct *pstr_psy_data = &(pstr_psy_mod->str_psy_data[i_ch]);
  ia_psy_mod_short_config_struct *pstr_psy_config =
      &(pstr_psy_mod->str_psy_short_config[elem_idx]);
  WORD32 max_sfb = 0, sfb, line;
  WORD32 window_sequence = pstr_sfb_prms->window_sequence[i_ch];
  WORD32 num_sfb = pstr_sfb_prms->num_sfb[i_ch];
  ia_tns_info *ptr_tns_info = pstr_tns_info[i_ch];
  WORD32 i, w;
  WORD32 frame_len_short = (ccfl * FRAME_LEN_SHORT_128) / FRAME_LEN_LONG;
  FLOAT32 energy_shift = 0.25f;
  FLOAT32 clip_energy = pstr_psy_config->clip_energy * energy_shift;
  (VOID) channel_type;

  pstr_psy_data->window_sequence = window_sequence;

  for (w = 0; w < MAX_SHORT_WINDOWS; w++) {
    WORD32 w_offset = w * frame_len_short;
    WORD32 offset;
    FLOAT64 *pmdct_double = &ptr_spec_in[pstr_psy_config->low_pass_line + w_offset];

    offset = frame_len_short - pstr_psy_config->low_pass_line;

    memset(pmdct_double, 0, sizeof(FLOAT64) * offset);

    iusace_calc_band_energy(ptr_spec_in + w_offset, pstr_psy_config->sfb_offset,
                            pstr_psy_config->sfb_active, pstr_psy_data->sfb_energy_short[w],
                            pstr_psy_config->sfb_count);

    if (tns_select != 0) {
      ia_tns_info *ptr_tns_info_ch2 = pstr_tns_info[i_ch - chn];
      ptr_tns_info->number_of_bands = num_sfb;
      ptr_tns_info->block_type = window_sequence;
      ptr_tns_info->spec = ptr_spec_in + w_offset;
      iusace_tns_encode(ptr_tns_info_ch2, ptr_tns_info, pstr_psy_data->sfb_energy_short[w], w,
                        chn, pstr_psy_config->low_pass_line, scratch_tns_filter, 0,
                        ptr_tns_scratch);
    }

    for (i = 0; i < pstr_psy_config->sfb_count; i++) {
      pstr_psy_data->sfb_thr_short[w][i] =
          pstr_psy_data->sfb_energy_short[w][i] * pstr_psy_config->ratio;
      pstr_psy_data->sfb_thr_short[w][i] = MIN(pstr_psy_data->sfb_thr_short[w][i], clip_energy);
    }

    if (tns_select != 0) {
      if (ptr_tns_info->tns_data_present == 1) {
        iusace_calc_band_energy(ptr_spec_in + w_offset, pstr_psy_config->sfb_offset,
                                pstr_psy_config->sfb_active, pstr_psy_data->sfb_energy_short[w],
                                pstr_psy_config->sfb_count);
      }
    }

    iusace_find_max_spreading(pstr_psy_config->sfb_count, pstr_psy_config->sfb_mask_low_fac,
                              pstr_psy_config->sfb_mask_high_fac,
                              pstr_psy_data->sfb_thr_short[w]);

    for (i = 0; i < pstr_psy_config->sfb_count; i++) {
      pstr_psy_data->sfb_thr_short[w][i] =
          MAX(pstr_psy_data->sfb_thr_short[w][i], (pstr_psy_config->sfb_thr_quiet[i] * 0.25f));
    }

    iusace_pre_echo_control(pstr_psy_data->sfb_thr_nm1, pstr_psy_config->sfb_count,
                            pstr_psy_config->max_allowed_inc_fac,
                            pstr_psy_config->min_remaining_thr_fac,
                            pstr_psy_data->sfb_thr_short[w]);

    for (i = 0; i < pstr_psy_config->sfb_count; i++) {
      pstr_psy_data->sfb_spreaded_energy_short[w][i] = pstr_psy_data->sfb_energy_short[w][i];
    }
    iusace_find_max_spreading(
        pstr_psy_config->sfb_count, pstr_psy_config->sfb_mask_low_fac_spr_ener,
        pstr_psy_config->sfb_mask_high_fac_spr_ener, pstr_psy_data->sfb_spreaded_energy_short[w]);
  }

  for (WORD32 wnd = 0; wnd < MAX_SHORT_WINDOWS; wnd++) {
    for (sfb = pstr_psy_config->sfb_count - 1; sfb >= max_sfb; sfb--) {
      for (line = pstr_psy_config->sfb_offset[sfb + 1] - 1;
           line >= pstr_psy_config->sfb_offset[sfb]; line--) {
        if (ptr_spec_in[wnd * frame_len_short + line] != 0.0) break;
      }
      if (line >= pstr_psy_config->sfb_offset[sfb]) break;
    }
    max_sfb = MAX(max_sfb, sfb);
  }
  max_sfb = max_sfb > 0 ? max_sfb : 0;

  pstr_psy_mod->str_psy_out_data[i_ch].max_sfb_per_grp = max_sfb + 1;

  return;
}
