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

#include <float.h>
#include <math.h>
#include <stdlib.h>

#include <string.h>

#include "ixheaac_type_def.h"
#include "ixheaac_constants.h"
#include "impd_drc_common_enc.h"
#include "impd_drc_uni_drc.h"
#include "impd_drc_tables.h"
#include "impd_drc_api.h"
#include "ixheaace_api.h"
#include "ixheaace_aac_constants.h"
#include "ixheaace_psy_const.h"
#include "ixheaace_tns.h"
#include "ixheaace_tns_params.h"
#include "ixheaace_rom.h"

#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops16.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaac_basic_ops.h"

#include "ixheaace_common_rom.h"
#include "ixheaace_bitbuffer.h"
#include "ixheaace_adjust_threshold_data.h"
#include "ixheaace_dynamic_bits.h"
#include "ixheaace_qc_data.h"
#include "ixheaace_block_switch.h"
#include "ixheaace_psy_data.h"
#include "ixheaace_interface.h"
#include "ixheaace_adjust_threshold.h"
#include "ixheaace_common_utils.h"

VOID iaace_adj_thr_init(ia_adj_thr_state_struct *pstr_adj_thr_state, const FLOAT32 mean_pe,
                        WORD32 ch_bitrate, WORD32 aot) {
  ia_adj_thr_elem_struct *pstr_adj_thr_ele = &pstr_adj_thr_state->str_adj_thr_ele;
  ia_min_snr_adapt_param_struct *pstr_min_snr_params =
      &pstr_adj_thr_ele->str_min_snr_adapt_params;

  pstr_adj_thr_state->str_bitres_params_long.clip_save_low = CLIP_SAVE_LO_LONG;
  pstr_adj_thr_state->str_bitres_params_long.clip_save_high = CLIP_SAVE_HI_LONG;
  pstr_adj_thr_state->str_bitres_params_long.min_bit_save = MIN_BITS_SAVE_LONG;
  pstr_adj_thr_state->str_bitres_params_long.max_bit_save = MAX_BITS_SAVE_LONG;
  pstr_adj_thr_state->str_bitres_params_long.clip_spend_low = CLIP_SPEND_LO_LONG;
  pstr_adj_thr_state->str_bitres_params_long.clip_spend_high = CLIP_SPEND_HI_LONG;
  pstr_adj_thr_state->str_bitres_params_long.min_bits_spend = MIN_BITS_SPEND_LONG;
  pstr_adj_thr_state->str_bitres_params_long.max_bits_spend = MAX_BITS_SPEND_LONG;

  pstr_adj_thr_state->str_bitres_params_short.clip_save_low = CLIP_SAVE_LO_SHORT;
  pstr_adj_thr_state->str_bitres_params_short.clip_save_high = CLIP_SAVE_HI_SHORT;
  pstr_adj_thr_state->str_bitres_params_short.min_bit_save = MIN_BITS_SAVE_SHORT;
  pstr_adj_thr_state->str_bitres_params_short.max_bit_save = MAX_BITS_SAVE_SHORT;
  pstr_adj_thr_state->str_bitres_params_short.clip_spend_low = CLIP_SPEND_LO_SHORT;
  pstr_adj_thr_state->str_bitres_params_short.clip_spend_high = CLIP_SPEND_HI_SHORT;
  pstr_adj_thr_state->str_bitres_params_short.min_bits_spend = MIN_BITS_SPEND_SHORT;
  pstr_adj_thr_state->str_bitres_params_short.max_bits_spend = MAX_BITS_SPEND_SHORT;

  pstr_adj_thr_ele->pe_min = (FLOAT32)0.8f * mean_pe;
  pstr_adj_thr_ele->pe_max = (FLOAT32)1.2f * mean_pe;
  pstr_adj_thr_ele->pe_offset = 0.0f;
  switch (aot) {
    case AOT_AAC_LC:
    case AOT_SBR:
    case AOT_PS:
      if (ch_bitrate < 32000) {
        pstr_adj_thr_ele->pe_offset = MAX(
            (FLOAT32)50.0f, (FLOAT32)(100.0f) - (FLOAT32)(100.0f / 32000) * (FLOAT32)ch_bitrate);
      }
      break;

    case AOT_AAC_LD:
    case AOT_AAC_ELD:

      if (ch_bitrate <= 32000) {
        pstr_adj_thr_ele->pe_offset = MAX(
            (FLOAT32)50.0f, (FLOAT32)(100.0f) - (FLOAT32)(100.0f / 32000) * (FLOAT32)ch_bitrate);
      }
      break;
  }

  if (ch_bitrate > 20000) {
    pstr_adj_thr_ele->str_ah_param.modify_min_snr = TRUE;
    pstr_adj_thr_ele->str_ah_param.start_sfb_long = 15;
    pstr_adj_thr_ele->str_ah_param.start_sfb_short = 3;
  } else {
    pstr_adj_thr_ele->str_ah_param.modify_min_snr = FALSE;
    pstr_adj_thr_ele->str_ah_param.start_sfb_long = 0;
    pstr_adj_thr_ele->str_ah_param.start_sfb_short = 0;
  }

  pstr_min_snr_params->max_red = (FLOAT32)0.25f;

  pstr_min_snr_params->start_ratio = (FLOAT32)1.e1f;

  pstr_min_snr_params->max_ratio = (FLOAT32)1.e3f;

  pstr_min_snr_params->red_ratio_fac =
      (1.0f - pstr_min_snr_params->max_red) /
      (10.0f * (FLOAT32)log10(pstr_min_snr_params->start_ratio / pstr_min_snr_params->max_ratio));

  pstr_min_snr_params->red_offs = 1.0f - pstr_min_snr_params->red_ratio_fac * 10.0f *
                                             (FLOAT32)log10(pstr_min_snr_params->start_ratio);

  pstr_adj_thr_ele->pe_last = (FLOAT32)0.0f;
  pstr_adj_thr_ele->dyn_bits_last = 0;
  pstr_adj_thr_ele->pe_correction_fac = (FLOAT32)1.0f;
}
FLOAT32 iaace_bits_to_pe(const FLOAT32 bits) { return (bits * 1.18f); }
static VOID iaace_calc_sfb_pe_data(ia_qc_pe_data_struct *pstr_qc_pe_data,
                                   ixheaace_psy_out_channel **pstr_psy_out, WORD32 num_channels,
                                   WORD32 chn) {
  WORD32 ch;
  WORD32 scf_band_grp;
  FLOAT32 num_lines;
  FLOAT32 ld_thr, ld_ratio;
  WORD32 i = 0, scf;
  WORD32 sfb_count;
  WORD32 scf_band_per_grp;
  WORD32 max_sfb_per_grp;
  FLOAT32 *ptr_sfb_energy;
  FLOAT32 *ptr_sfb_thr;
  ia_qc_pe_chan_data_struct *pstr_qc_pe_chan_data;

  pstr_qc_pe_data->pe = pstr_qc_pe_data->offset;
  pstr_qc_pe_data->const_part = 0.0f;
  pstr_qc_pe_data->num_active_lines = 0.0f;

  for (ch = chn; ch < chn + num_channels; ch++) {
    sfb_count = pstr_psy_out[ch]->sfb_count;
    scf_band_per_grp = pstr_psy_out[ch]->sfb_per_group;
    max_sfb_per_grp = pstr_psy_out[ch]->max_sfb_per_grp;
    ptr_sfb_energy = pstr_psy_out[ch]->ptr_sfb_energy;
    ptr_sfb_thr = pstr_psy_out[ch]->ptr_sfb_thr;
    pstr_qc_pe_chan_data = &pstr_qc_pe_data->pe_ch_data[ch];
    pstr_qc_pe_chan_data->pe = 0;
    pstr_qc_pe_chan_data->num_active_lines = 0;
    pstr_qc_pe_chan_data->const_part = 0;

    for (scf_band_grp = 0; scf_band_grp < sfb_count; scf_band_grp += scf_band_per_grp) {
      i = scf_band_grp;
      for (scf = max_sfb_per_grp - 1; scf >= 0; scf--, i++) {
        if (ptr_sfb_energy[i] > ptr_sfb_thr[i]) {
          ld_thr = (FLOAT32)log(ptr_sfb_thr[i]) * LOG2_1;
          ld_ratio = pstr_qc_pe_chan_data->sfb_ld_energy[i] - ld_thr;
          num_lines = pstr_qc_pe_chan_data->sfb_lines[i];
          if (ld_ratio >= PE_C1) {
            pstr_qc_pe_chan_data->sfb_pe[i] = num_lines * ld_ratio;
            pstr_qc_pe_chan_data->sfb_const_part[i] =
                num_lines * pstr_qc_pe_chan_data->sfb_ld_energy[i];
          } else {
            pstr_qc_pe_chan_data->sfb_pe[i] = num_lines * (PE_C2 + PE_C3 * ld_ratio);
            pstr_qc_pe_chan_data->sfb_const_part[i] =
                num_lines * (PE_C2 + PE_C3 * pstr_qc_pe_chan_data->sfb_ld_energy[i]);
            num_lines = num_lines * PE_C3;
          }
          pstr_qc_pe_chan_data->num_sfb_active_lines[i] = num_lines;
        } else {
          pstr_qc_pe_chan_data->sfb_pe[i] = 0.0f;
          pstr_qc_pe_chan_data->sfb_const_part[i] = 0.0f;
          pstr_qc_pe_chan_data->num_sfb_active_lines[i] = 0.0;
        }

        pstr_qc_pe_chan_data->pe += pstr_qc_pe_chan_data->sfb_pe[i];
        pstr_qc_pe_chan_data->const_part += pstr_qc_pe_chan_data->sfb_const_part[i];
        pstr_qc_pe_chan_data->num_active_lines += pstr_qc_pe_chan_data->num_sfb_active_lines[i];
      }
    }
    pstr_qc_pe_data->pe += pstr_qc_pe_chan_data->pe;
    pstr_qc_pe_data->const_part += pstr_qc_pe_chan_data->const_part;
    pstr_qc_pe_data->num_active_lines += pstr_qc_pe_chan_data->num_active_lines;
    pstr_psy_out[ch]->pe = pstr_qc_pe_data->pe;
  }
}
static FLOAT32 iaace_calc_bit_save(ia_bitres_param_struct *pstr_bitres_params, FLOAT32 fill_lvl) {
  FLOAT32 bit_save;
  const FLOAT32 clip_low = pstr_bitres_params->clip_save_low;
  const FLOAT32 clip_high = pstr_bitres_params->clip_save_high;
  const FLOAT32 min_bit_save = pstr_bitres_params->min_bit_save;
  const FLOAT32 max_bit_save = pstr_bitres_params->max_bit_save;

  fill_lvl = MAX(fill_lvl, clip_low);
  fill_lvl = MIN(fill_lvl, clip_high);
  bit_save = max_bit_save -
             ((max_bit_save - min_bit_save) / (clip_high - clip_low)) * (fill_lvl - clip_low);

  return bit_save;
}

static FLOAT32 iaace_calc_bit_spend(ia_bitres_param_struct *pstr_bitres_params,
                                    FLOAT32 fill_lvl) {
  FLOAT32 bit_spend;
  const FLOAT32 clip_low = pstr_bitres_params->clip_spend_low;
  const FLOAT32 clip_high = pstr_bitres_params->clip_spend_high;
  const FLOAT32 min_bits_spend = pstr_bitres_params->min_bits_spend;
  const FLOAT32 max_bits_spend = pstr_bitres_params->max_bits_spend;

  fill_lvl = MAX(fill_lvl, clip_low);
  fill_lvl = MIN(fill_lvl, clip_high);
  bit_spend = min_bits_spend + ((max_bits_spend - min_bits_spend) / (clip_high - clip_low)) *
                                   (fill_lvl - clip_low);

  return bit_spend;
}

static VOID iaace_adjust_pe_minmax(const FLOAT32 curr_pe, FLOAT32 *ptr_pe_min,
                                   FLOAT32 *ptr_pe_max) {
  FLOAT32 min_hi_fac = 0.3f, max_hi_fac = 1.0f, min_low_fac = 0.14f, max_low_fac = 0.07f;
  FLOAT32 diff;
  FLOAT32 min_diff = curr_pe * (FLOAT32)0.1666666667f;

  if (curr_pe > *ptr_pe_max) {
    diff = (curr_pe - *ptr_pe_max);
    *ptr_pe_min += diff * min_hi_fac;
    *ptr_pe_max += diff * max_hi_fac;
  } else {
    if (curr_pe < *ptr_pe_min) {
      diff = (*ptr_pe_min - curr_pe);
      *ptr_pe_min -= diff * min_low_fac;
      *ptr_pe_max -= diff * max_low_fac;
    } else {
      *ptr_pe_min += (curr_pe - *ptr_pe_min) * min_hi_fac;
      *ptr_pe_max -= (*ptr_pe_max - curr_pe) * max_low_fac;
    }
  }

  if ((*ptr_pe_max - *ptr_pe_min) < min_diff) {
    FLOAT32 low_part, high_part;
    low_part = MAX((FLOAT32)0.0f, curr_pe - *ptr_pe_min);
    high_part = MAX((FLOAT32)0.0f, *ptr_pe_max - curr_pe);
    *ptr_pe_max = curr_pe + high_part / (low_part + high_part) * min_diff;
    *ptr_pe_min = curr_pe - low_part / (low_part + high_part) * min_diff;
    *ptr_pe_min = MAX((FLOAT32)0.0f, *ptr_pe_min);
  }
}

static FLOAT32 iaace_bitres_calc_bitfac(const WORD32 bitres_bits, const WORD32 max_bitres_bits,
                                        const FLOAT32 pe, const WORD32 win_seq,
                                        const WORD32 avg_bits, const FLOAT32 max_bit_fac,
                                        ia_adj_thr_state_struct *pstr_adj_thr_state,
                                        ia_adj_thr_elem_struct *pstr_adj_the_elem) {
  ia_bitres_param_struct *pstr_bitres_params;
  FLOAT32 pex;
  FLOAT32 fill_lvl = 0.0f;
  FLOAT32 bit_save, bit_spend, bitres_factor;

  if (max_bitres_bits) {
    fill_lvl = (FLOAT32)bitres_bits / max_bitres_bits;
  }

  if (win_seq != SHORT_WINDOW) {
    pstr_bitres_params = &(pstr_adj_thr_state->str_bitres_params_long);
  } else {
    pstr_bitres_params = &(pstr_adj_thr_state->str_bitres_params_short);
  }

  pex = MAX(pe, pstr_adj_the_elem->pe_min);
  pex = MIN(pex, pstr_adj_the_elem->pe_max);

  bit_save = iaace_calc_bit_save(pstr_bitres_params, fill_lvl);
  bit_spend = iaace_calc_bit_spend(pstr_bitres_params, fill_lvl);

  bitres_factor =
      (FLOAT32)1.0f - bit_save +
      ((bit_spend + bit_save) / (pstr_adj_the_elem->pe_max - pstr_adj_the_elem->pe_min)) *
          (pex - pstr_adj_the_elem->pe_min);
  if (avg_bits)
  {
    bitres_factor = MIN(bitres_factor,
      (FLOAT32)1.0f - (FLOAT32)0.3f + (FLOAT32)bitres_bits / (FLOAT32)avg_bits);
  }

  bitres_factor = MIN(bitres_factor, max_bit_fac);

  iaace_adjust_pe_minmax(pe, &pstr_adj_the_elem->pe_min, &pstr_adj_the_elem->pe_max);

  return bitres_factor;
}

static VOID iaace_calc_pe_correction(FLOAT32 *ptr_correction_fac, const FLOAT32 pe_act,
                                     const FLOAT32 pe_last, const WORD32 bits_prev) {
  if ((bits_prev > 0) && (pe_act < (FLOAT32)1.5f * pe_last) &&
      (pe_act > (FLOAT32)0.7f * pe_last) &&
      ((FLOAT32)1.2f * iaace_bits_to_pe((FLOAT32)bits_prev) > pe_last) &&
      ((FLOAT32)0.65f * iaace_bits_to_pe((FLOAT32)bits_prev) < pe_last)) {
    FLOAT32 new_fac = pe_last / iaace_bits_to_pe((FLOAT32)bits_prev);

    if (new_fac < (FLOAT32)1.0f) {
      new_fac = MIN((FLOAT32)1.1f * new_fac, (FLOAT32)1.0f);
      new_fac = MAX(new_fac, (FLOAT32)0.85f);
    } else {
      new_fac = MAX((FLOAT32)0.9f * new_fac, (FLOAT32)1.0f);
      new_fac = MIN(new_fac, (FLOAT32)1.15f);
    }
    if (((new_fac > (FLOAT32)1.0f) && (*ptr_correction_fac < (FLOAT32)1.0f)) ||
        ((new_fac < (FLOAT32)1.0f) && (*ptr_correction_fac > (FLOAT32)1.0f))) {
      *ptr_correction_fac = (FLOAT32)1.0f;
    }

    if ((*ptr_correction_fac < (FLOAT32)1.0f && new_fac < *ptr_correction_fac) ||
        (*ptr_correction_fac > (FLOAT32)1.0f && new_fac > *ptr_correction_fac)) {
      *ptr_correction_fac = (FLOAT32)0.85f * (*ptr_correction_fac) + (FLOAT32)0.15f * new_fac;
    } else {
      *ptr_correction_fac = (FLOAT32)0.7f * (*ptr_correction_fac) + (FLOAT32)0.3f * new_fac;
    }

    *ptr_correction_fac = MIN(*ptr_correction_fac, (FLOAT32)1.15f);
    *ptr_correction_fac = MAX(*ptr_correction_fac, (FLOAT32)0.85f);
  } else {
    *ptr_correction_fac = (FLOAT32)1.0f;
  }
}

static VOID iaace_calc_thr_exp(
    FLOAT32 thr_exp[IXHEAACE_MAX_CH_IN_BS_ELE][MAXIMUM_GROUPED_SCALE_FACTOR_BAND],
    ixheaace_psy_out_channel **pstr_psy_out, WORD32 num_chans, WORD32 chn) {
  WORD32 sfb, ch, scf_band_grp;
  ixheaace_psy_out_channel *pstr_psy_chan_out;
  FLOAT32 *ptr_scf_band_thr;
  FLOAT32 *ptr_thr_exp;
  for (ch = chn; ch < chn + num_chans; ch++) {
    pstr_psy_chan_out = pstr_psy_out[ch];
    ptr_thr_exp = thr_exp[ch];
    for (scf_band_grp = 0; scf_band_grp < pstr_psy_chan_out->sfb_count;
         scf_band_grp += pstr_psy_chan_out->sfb_per_group) {
      FLOAT32 *ptr_thr_exp1 = &ptr_thr_exp[scf_band_grp];
      ptr_scf_band_thr = &pstr_psy_chan_out->ptr_sfb_thr[scf_band_grp];
      for (sfb = 0; sfb < pstr_psy_chan_out->max_sfb_per_grp; sfb++) {
        ptr_thr_exp1[sfb] = (FLOAT32)pow(*ptr_scf_band_thr++, RED_EXP_VAL);
      }
    }
  }
}

static VOID iaace_adapt_min_snr(ixheaace_psy_out_channel **pstr_psy_out,
                                ia_min_snr_adapt_param_struct *pstr_min_snr_params,
                                WORD32 num_chans, WORD32 chn) {
  WORD32 num_scf_band = 0, ch, scf_band_cnt, scf_band_offs, sfb;
  FLOAT32 avg_energy = 0.0f, db_ratio, min_snr_red;
  WORD32 i;

  for (ch = chn; ch < chn + num_chans; ch++) {
    ixheaace_psy_out_channel *pstr_psy_chan_out = pstr_psy_out[ch];
    num_scf_band = 0;
    avg_energy = 0;
    scf_band_cnt = pstr_psy_chan_out->max_sfb_per_grp;

    for (scf_band_offs = 0; scf_band_offs < pstr_psy_chan_out->sfb_count;
         scf_band_offs += pstr_psy_chan_out->sfb_per_group) {
      FLOAT32 *ptr_sfb_energy = &pstr_psy_chan_out->ptr_sfb_energy[scf_band_offs];
      for (sfb = scf_band_cnt - 1; sfb >= 0; sfb--) {
        avg_energy += ptr_sfb_energy[sfb];
      }
      num_scf_band += scf_band_cnt;
    }

    if (num_scf_band > 0) {
      avg_energy /= num_scf_band;
    }

    for (scf_band_offs = 0; scf_band_offs < pstr_psy_chan_out->sfb_count;
         scf_band_offs += pstr_psy_chan_out->sfb_per_group) {
      i = scf_band_offs;
      for (sfb = scf_band_cnt - 1; sfb >= 0; sfb--, i++) {
        if (pstr_min_snr_params->start_ratio * pstr_psy_chan_out->ptr_sfb_energy[i] <
            avg_energy) {
          db_ratio =
              (FLOAT32)(10.0f * log10((MIN_FLT_VAL + avg_energy) /
                                      (MIN_FLT_VAL + pstr_psy_chan_out->ptr_sfb_energy[i])));
          min_snr_red =
              pstr_min_snr_params->red_offs + pstr_min_snr_params->red_ratio_fac * db_ratio;
          min_snr_red = MAX(min_snr_red, pstr_min_snr_params->max_red);
          pstr_psy_chan_out->sfb_min_snr[i] =
              (FLOAT32)pow(pstr_psy_out[ch]->sfb_min_snr[i], min_snr_red);
          pstr_psy_chan_out->sfb_min_snr[i] =
              MIN(MIN_SNR_LIMIT, pstr_psy_out[ch]->sfb_min_snr[i]);
        }
      }
    }
  }
}

static VOID iaace_init_avoid_hole_flag(
    WORD32 ah_flag[IXHEAACE_MAX_CH_IN_BS_ELE][MAXIMUM_GROUPED_SCALE_FACTOR_BAND],
    ixheaace_psy_out_channel **pstr_psy_out, ia_ah_param_struct *pstr_ah_param, WORD32 num_chans,
    WORD32 chn, WORD32 aot) {
  WORD32 ch;
  FLOAT32 sfb_energy;
  FLOAT32 scale_spread_energy = 0.0f;
  WORD32 scf_band_grp, scf_band_cnt, scf_band;
  FLOAT32 *ptr_scf_band_spread_energy, *ptr_scf_band_energy, *ptr_scf_band_min_snr;
  for (ch = chn; ch < chn + num_chans; ch++) {
    ixheaace_psy_out_channel *pstr_psy_chan_out = pstr_psy_out[ch];

    if (pstr_psy_chan_out->window_sequence != SHORT_WINDOW) {
      switch (aot) {
        case AOT_AAC_LC:
        case AOT_SBR:
        case AOT_PS:
          scale_spread_energy = 0.5f;
          break;

        case AOT_AAC_LD:
        case AOT_AAC_ELD:
          scale_spread_energy = 0.56f;
          break;
      }
    } else {
      scale_spread_energy = 0.63f;
    }

    for (scf_band_grp = 0; scf_band_grp < pstr_psy_chan_out->sfb_count;
         scf_band_grp += pstr_psy_chan_out->sfb_per_group) {
      ptr_scf_band_spread_energy = &pstr_psy_chan_out->ptr_sfb_spread_energy[scf_band_grp];
      scf_band_cnt = pstr_psy_chan_out->max_sfb_per_grp;
      for (scf_band = scf_band_cnt - 1; scf_band >= 0; scf_band--) {
        *ptr_scf_band_spread_energy = *ptr_scf_band_spread_energy * scale_spread_energy;
        ptr_scf_band_spread_energy++;
      }
    }
  }

  if (pstr_ah_param->modify_min_snr) {
    for (ch = chn; ch < chn + num_chans; ch++) {
      ixheaace_psy_out_channel *pstr_psy_chan_out = pstr_psy_out[ch];
      ptr_scf_band_energy = pstr_psy_chan_out->ptr_sfb_energy;

      ptr_scf_band_min_snr = pstr_psy_chan_out->sfb_min_snr;

      for (scf_band_grp = 0; scf_band_grp < pstr_psy_chan_out->sfb_count;
           scf_band_grp += pstr_psy_chan_out->sfb_per_group) {
        for (scf_band = 0; scf_band < pstr_psy_chan_out->max_sfb_per_grp; scf_band++) {
          FLOAT32 sfb_en_m1, sfb_en_p1, avg_energy;
          if (scf_band > 0) {
            sfb_en_m1 = ptr_scf_band_energy[scf_band_grp + scf_band - 1];
          } else {
            sfb_en_m1 = ptr_scf_band_energy[scf_band_grp];
          }
          if (scf_band < pstr_psy_chan_out->max_sfb_per_grp - 1) {
            sfb_en_p1 = ptr_scf_band_energy[scf_band_grp + scf_band + 1];
          } else {
            sfb_en_p1 = ptr_scf_band_energy[scf_band_grp + scf_band];
          }

          avg_energy = (sfb_en_m1 + sfb_en_p1) / (FLOAT32)2.0f;
          sfb_energy = ptr_scf_band_energy[scf_band_grp + scf_band];
          FLOAT32 temp_min_snr = 0.0f;
          if (sfb_energy > avg_energy) {
            switch (aot) {
              case AOT_AAC_LC:
              case AOT_SBR:
              case AOT_PS:
                temp_min_snr = MAX((FLOAT32)0.8f * avg_energy / sfb_energy, (FLOAT32)0.316f);
                break;

              case AOT_AAC_LD:
              case AOT_AAC_ELD:
                temp_min_snr = MAX((FLOAT32)0.8f * avg_energy / sfb_energy, (FLOAT32)0.408f);
                break;
            }
            if (pstr_psy_chan_out->window_sequence != SHORT_WINDOW) {
              switch (aot) {
                case AOT_AAC_LC:
                case AOT_PS:
                  temp_min_snr = MAX(temp_min_snr, (FLOAT32)0.316f);
                  break;

                case AOT_AAC_LD:
                  temp_min_snr = MAX(temp_min_snr, (FLOAT32)0.408f);
                  break;
              }
            } else {
              temp_min_snr = MAX(temp_min_snr, (FLOAT32)0.5f);
            }
            ptr_scf_band_min_snr[scf_band_grp + scf_band] =
                MIN(ptr_scf_band_min_snr[scf_band_grp + scf_band], temp_min_snr);
          }

          if (((FLOAT32)2.0f * sfb_energy < avg_energy) && (sfb_energy > (FLOAT32)0.0f)) {
            temp_min_snr = avg_energy / ((FLOAT32)2.0f * sfb_energy) *
                           ptr_scf_band_min_snr[scf_band_grp + scf_band];
            temp_min_snr = MIN((FLOAT32)0.8f, temp_min_snr);
            ptr_scf_band_min_snr[scf_band_grp + scf_band] =
                MIN(temp_min_snr, ptr_scf_band_min_snr[scf_band_grp + scf_band] * (FLOAT32)3.16f);
          }
        }
      }
    }
  }

  if (num_chans == 2) {
    ixheaace_psy_out_channel *pstr_psy_out_mid = pstr_psy_out[chn];
    ixheaace_psy_out_channel *pstr_psy_out_side = pstr_psy_out[chn + 1];
    WORD32 sfb;

    for (sfb = 0; sfb < pstr_psy_out_mid->sfb_count; sfb++) {
      if (pstr_psy_out[chn]->ms_used[sfb]) {
        FLOAT32 sfb_en_mid = pstr_psy_out_mid->ptr_sfb_energy[sfb];
        FLOAT32 sfb_en_side = pstr_psy_out_side->ptr_sfb_energy[sfb];
        FLOAT32 max_sfb_en = MAX(sfb_en_mid, sfb_en_side);
        FLOAT32 max_thr = 0.25f * pstr_psy_out_mid->sfb_min_snr[sfb] * max_sfb_en;

        pstr_psy_out_mid->sfb_min_snr[sfb] = (FLOAT32)MAX(
            pstr_psy_out_mid->sfb_min_snr[sfb],
            MIN(MAX_FLT_VAL, ((FLOAT32)max_thr / (MIN_FLT_VAL + (FLOAT32)sfb_en_mid))));

        if (pstr_psy_out_mid->ptr_sfb_energy[sfb] <= 1.0f) {
          pstr_psy_out_mid->ptr_sfb_energy[sfb] =
              MIN(pstr_psy_out_mid->ptr_sfb_energy[sfb], 0.8f);
        }

        pstr_psy_out_side->sfb_min_snr[sfb] = (FLOAT32)MAX(
            pstr_psy_out_side->sfb_min_snr[sfb],
            MIN(MAX_FLT_VAL, ((FLOAT32)max_thr / (MIN_FLT_VAL + (FLOAT32)sfb_en_side))));

        if (pstr_psy_out_side->sfb_min_snr[sfb] <= 1.0f) {
          pstr_psy_out_side->sfb_min_snr[sfb] = MIN(pstr_psy_out_side->sfb_min_snr[sfb], 0.8f);
        }
      }
    }
  }

  for (ch = chn; ch < chn + num_chans; ch++) {
    ixheaace_psy_out_channel *pstr_psy_chan_out = pstr_psy_out[ch];
    for (scf_band_grp = 0; scf_band_grp < pstr_psy_chan_out->sfb_count;
         scf_band_grp += pstr_psy_chan_out->sfb_per_group) {
      for (scf_band = 0; scf_band < pstr_psy_chan_out->max_sfb_per_grp; scf_band++) {
        if (pstr_psy_chan_out->ptr_sfb_spread_energy[scf_band_grp + scf_band] >
                pstr_psy_chan_out->ptr_sfb_energy[scf_band_grp + scf_band] ||
            pstr_psy_chan_out->sfb_min_snr[scf_band_grp + scf_band] > 1.0f) {
          ah_flag[ch][scf_band_grp + scf_band] = NO_AH;
        } else {
          ah_flag[ch][scf_band_grp + scf_band] = AH_INACTIVE;
        }
      }

      for (scf_band = pstr_psy_chan_out->max_sfb_per_grp;
           scf_band < pstr_psy_chan_out->sfb_per_group; scf_band++) {
        ah_flag[ch][scf_band_grp + scf_band] = NO_AH;
      }
    }
  }
}

static VOID iaace_reduce_thr(
    ixheaace_psy_out_channel **pstr_psy_out,
    WORD32 ah_flag[IXHEAACE_MAX_CH_IN_BS_ELE][MAXIMUM_GROUPED_SCALE_FACTOR_BAND],
    FLOAT32 thr_exp[IXHEAACE_MAX_CH_IN_BS_ELE][MAXIMUM_GROUPED_SCALE_FACTOR_BAND],
    const FLOAT32 red_value, WORD32 num_channels, WORD32 chn) {
  WORD32 ch, sfb_group, sfb;
  FLOAT32 sfb_energy, sfb_threshold, sfb_thr_reduced;
  FLOAT32 *ptr_sfb_energy_fix, *ptr_sfb_threshold_fix, *ptr_sfb_min_snr_fix, *ptr_thr_exp_fix;

  for (ch = chn; ch < chn + num_channels; ch++) {
    ixheaace_psy_out_channel *pstr_psy_chan_out = pstr_psy_out[ch];
    ptr_sfb_energy_fix = pstr_psy_chan_out->ptr_sfb_energy;
    ptr_sfb_threshold_fix = pstr_psy_chan_out->ptr_sfb_thr;
    ptr_sfb_min_snr_fix = pstr_psy_chan_out->sfb_min_snr;
    ptr_thr_exp_fix = &thr_exp[ch][0];
    for (sfb_group = 0; sfb_group < pstr_psy_chan_out->sfb_count;
         sfb_group += pstr_psy_chan_out->sfb_per_group) {
      for (sfb = 0; sfb < pstr_psy_chan_out->max_sfb_per_grp; sfb++) {
        sfb_energy = ptr_sfb_energy_fix[sfb_group + sfb];
        sfb_threshold = ptr_sfb_threshold_fix[sfb_group + sfb];
        if (sfb_energy > sfb_threshold) {
          sfb_thr_reduced =
              (FLOAT32)pow((ptr_thr_exp_fix[sfb_group + sfb] + red_value), INV_RED_EXP_VAL);

          if ((sfb_thr_reduced > ptr_sfb_min_snr_fix[sfb_group + sfb] * sfb_energy) &&
              (ah_flag[ch][sfb_group + sfb] != NO_AH)) {
            sfb_thr_reduced =
                MAX(ptr_sfb_min_snr_fix[sfb_group + sfb] * sfb_energy, sfb_threshold);
            ah_flag[ch][sfb_group + sfb] = AH_ACTIVE;
          }
          ptr_sfb_threshold_fix[sfb_group + sfb] = sfb_thr_reduced;
        }
      }
    }
  }
}

static VOID iaace_calc_pe_no_active_holes(
    FLOAT32 *ptr_pe, FLOAT32 *ptr_const_part, FLOAT32 *ptr_num_active_lines,
    ia_qc_pe_data_struct *pstr_qs_pe_data,
    WORD32 ah_flag[IXHEAACE_MAX_CH_IN_BS_ELE][MAXIMUM_GROUPED_SCALE_FACTOR_BAND],
    ixheaace_psy_out_channel **pstr_psy_out, WORD32 num_channels, WORD32 chn) {
  WORD32 ch, sfb_group, sfb;
  *ptr_pe = 0.0f;
  *ptr_const_part = 0.0f;
  *ptr_num_active_lines = 0;

  for (ch = chn; ch < chn + num_channels; ch++) {
    ixheaace_psy_out_channel *pstr_psy_chan_out = pstr_psy_out[ch];
    ia_qc_pe_chan_data_struct *ptr_pe_chan_data = &pstr_qs_pe_data->pe_ch_data[ch];

    for (sfb_group = 0; sfb_group < pstr_psy_chan_out->sfb_count;
         sfb_group += pstr_psy_chan_out->sfb_per_group) {
      for (sfb = 0; sfb < pstr_psy_chan_out->max_sfb_per_grp; sfb++) {
        if (ah_flag[ch][sfb_group + sfb] < AH_ACTIVE) {
          *ptr_pe += ptr_pe_chan_data->sfb_pe[sfb_group + sfb];
          *ptr_const_part += ptr_pe_chan_data->sfb_const_part[sfb_group + sfb];
          *ptr_num_active_lines += ptr_pe_chan_data->num_sfb_active_lines[sfb_group + sfb];
        }
      }
    }
  }
}

static VOID iaace_correct_thr(
    ixheaace_psy_out_channel **pstr_psy_out,
    WORD32 ah_flag[IXHEAACE_MAX_CH_IN_BS_ELE][MAXIMUM_GROUPED_SCALE_FACTOR_BAND],
    ia_qc_pe_data_struct *pstr_qs_pe_data,
    FLOAT32 thr_exp[IXHEAACE_MAX_CH_IN_BS_ELE][MAXIMUM_GROUPED_SCALE_FACTOR_BAND],
    const FLOAT32 red_value, const FLOAT32 delta_pe, WORD32 num_channels, WORD32 chn) {
  WORD32 i, ch, sfb_group, sfb;
  FLOAT32 delta_sfb_pe;
  FLOAT32 thr_factor;
  FLOAT32 sfb_pe_factors[IXHEAACE_MAX_CH_IN_BS_ELE][MAXIMUM_GROUPED_SCALE_FACTOR_BAND],
      norm_factor[IXHEAACE_MAX_CH_IN_BS_ELE] = {MIN_FLT_VAL};
  FLOAT32 sfb_en, sfb_thr, sfb_thr_reduced;
  FLOAT32 *ptr_thr_exp;
  FLOAT32 *ptr_sfb_energy, *ptr_sfb_thr, *ptr_sfb_min_snr;
  ixheaace_psy_out_channel *pstr_psy_chan_out = NULL;
  ia_qc_pe_chan_data_struct *pstr_pe_chan_data = NULL;

  for (ch = chn; ch < chn + num_channels; ch++) {
    pstr_psy_chan_out = pstr_psy_out[ch];
    pstr_pe_chan_data = &pstr_qs_pe_data->pe_ch_data[ch];
    norm_factor[ch] = MIN_FLT_VAL;
    ptr_thr_exp = thr_exp[ch];

    for (sfb_group = 0; sfb_group < pstr_psy_chan_out->sfb_count;
         sfb_group += pstr_psy_chan_out->sfb_per_group) {
      for (sfb = 0; sfb < pstr_psy_chan_out->max_sfb_per_grp; sfb++) {
        if ((ah_flag[ch][sfb_group + sfb] < AH_ACTIVE) || (delta_pe > 0)) {
          sfb_pe_factors[ch][sfb_group + sfb] =
              pstr_pe_chan_data->num_sfb_active_lines[sfb_group + sfb] /
              (ptr_thr_exp[sfb_group + sfb] + red_value);
          norm_factor[ch] += sfb_pe_factors[ch][sfb_group + sfb];
        } else {
          sfb_pe_factors[ch][sfb_group + sfb] = 0.0f;
        }
      }
    }
  }
  if (num_channels > 1) {
    norm_factor[chn] = norm_factor[chn] + norm_factor[chn + 1];
  }
  norm_factor[chn] = 1.0f / norm_factor[chn];

  for (ch = chn; ch < chn + num_channels; ch++) {
    pstr_psy_chan_out = pstr_psy_out[ch];
    pstr_pe_chan_data = &pstr_qs_pe_data->pe_ch_data[ch];
    ptr_sfb_energy = pstr_psy_chan_out->ptr_sfb_energy;
    ptr_sfb_thr = pstr_psy_chan_out->ptr_sfb_thr;
    ptr_sfb_min_snr = pstr_psy_chan_out->sfb_min_snr;

    for (sfb_group = 0; sfb_group < pstr_psy_chan_out->sfb_count;
         sfb_group += pstr_psy_chan_out->sfb_per_group) {
      i = sfb_group;
      for (sfb = pstr_psy_chan_out->max_sfb_per_grp - 1; sfb >= 0; sfb--, i++) {
        delta_sfb_pe = sfb_pe_factors[ch][i] * norm_factor[chn] * delta_pe;
        if (pstr_pe_chan_data->num_sfb_active_lines[i] > (FLOAT32)0.5f) {
          sfb_en = ptr_sfb_energy[i];
          sfb_thr = ptr_sfb_thr[i];
          thr_factor = MIN(-delta_sfb_pe / pstr_pe_chan_data->num_sfb_active_lines[i], 20.f);
          thr_factor = (FLOAT32)pow(2.0f, thr_factor);
          sfb_thr_reduced = sfb_thr * thr_factor;

          if ((sfb_thr_reduced > ptr_sfb_min_snr[i] * sfb_en) &&
              (ah_flag[ch][i] == AH_INACTIVE)) {
            sfb_thr_reduced = MAX(ptr_sfb_min_snr[i] * sfb_en, sfb_thr);
            ah_flag[ch][i] = AH_ACTIVE;
          }
          ptr_sfb_thr[i] = sfb_thr_reduced;
        }
      }
    }
  }
}

static VOID iaace_reduce_min_snr(
    ixheaace_psy_out_channel **pstr_psy_out, ia_qc_pe_data_struct *pstr_qs_pe_data,
    WORD32 ah_flag[IXHEAACE_MAX_CH_IN_BS_ELE][MAXIMUM_GROUPED_SCALE_FACTOR_BAND],
    const FLOAT32 desired_pe, WORD32 num_channels, WORD32 chn) {
  WORD32 sfb, sfb_sub_win, ch;
  FLOAT32 delta_pe;

  sfb_sub_win = pstr_psy_out[chn]->max_sfb_per_grp;

  while (pstr_qs_pe_data->pe > desired_pe && sfb_sub_win > 0) {
    sfb_sub_win--;
    for (sfb = sfb_sub_win; sfb < pstr_psy_out[chn]->sfb_count;
         sfb += pstr_psy_out[chn]->sfb_per_group) {
      for (ch = chn; ch < chn + num_channels; ch++) {
        if (ah_flag[ch][sfb] != NO_AH && pstr_psy_out[ch]->sfb_min_snr[sfb] < MIN_SNR_LIMIT) {
          pstr_psy_out[ch]->sfb_min_snr[sfb] = MIN_SNR_LIMIT;
          pstr_psy_out[ch]->ptr_sfb_thr[sfb] =
              pstr_psy_out[ch]->ptr_sfb_energy[sfb] * pstr_psy_out[ch]->sfb_min_snr[sfb];
          delta_pe = pstr_qs_pe_data->pe_ch_data[ch].sfb_lines[sfb] * 1.5f -
                     pstr_qs_pe_data->pe_ch_data[ch].sfb_pe[sfb];
          pstr_qs_pe_data->pe += delta_pe;
          pstr_qs_pe_data->pe_ch_data[ch].pe += delta_pe;
        }
      }
      if (pstr_qs_pe_data->pe <= desired_pe) {
        break;
      }
    }
  }
}

static VOID iaace_allow_more_holes(
    ixheaace_psy_out_channel **pstr_psy_out, ia_qc_pe_data_struct *pstr_qs_pe_data,
    WORD32 ah_flag[IXHEAACE_MAX_CH_IN_BS_ELE][MAXIMUM_GROUPED_SCALE_FACTOR_BAND],
    const ia_ah_param_struct *pstr_str_ah_param, const FLOAT32 desired_pe, WORD32 num_channels,
    WORD32 chn) {
  WORD32 sfb, ch;
  FLOAT32 act_pe = pstr_qs_pe_data->pe;

  if (num_channels == 2 &&
      pstr_psy_out[chn]->window_sequence == pstr_psy_out[chn + 1]->window_sequence) {
    ixheaace_psy_out_channel *pstr_psy_out_left = pstr_psy_out[chn];
    ixheaace_psy_out_channel *pstr_psy_out_right = pstr_psy_out[chn + 1];

    for (sfb = 0; sfb < pstr_psy_out_left->sfb_count; sfb++) {
      if (pstr_psy_out[chn]->ms_used[sfb]) {
        if (ah_flag[chn + 1][sfb] != NO_AH &&
            0.4f * pstr_psy_out_left->sfb_min_snr[sfb] * pstr_psy_out_left->ptr_sfb_energy[sfb] >
                pstr_psy_out_right->ptr_sfb_energy[sfb]) {
          ah_flag[chn + 1][sfb] = NO_AH;

          pstr_psy_out_right->ptr_sfb_thr[sfb] = 2.0f * pstr_psy_out_right->ptr_sfb_energy[sfb];

          act_pe -= pstr_qs_pe_data->pe_ch_data[chn + 1].sfb_pe[sfb];
        } else {
          if (ah_flag[chn][sfb] != NO_AH && 0.4f * pstr_psy_out_right->sfb_min_snr[sfb] *
                                                    pstr_psy_out_right->ptr_sfb_energy[sfb] >
                                                pstr_psy_out_left->ptr_sfb_energy[sfb]) {
            ah_flag[chn][sfb] = NO_AH;

            pstr_psy_out_left->ptr_sfb_thr[sfb] = 2.0f * pstr_psy_out_left->ptr_sfb_energy[sfb];

            act_pe -= pstr_qs_pe_data->pe_ch_data[chn].sfb_pe[sfb];
          }
        }
        if (act_pe < desired_pe) {
          break;
        }
      }
    }
  }
  if (act_pe > desired_pe) {
    WORD32 start_sfb[IXHEAACE_MAX_CH_IN_BS_ELE] = {0};
    FLOAT32 average_energy, min_energy;
    WORD32 ah_cnt;
    WORD32 en_idx;
    FLOAT32 energy[4];
    WORD32 min_sfb, max_sfb;
    WORD32 done;

    for (ch = chn; ch < chn + num_channels; ch++) {
      if (pstr_psy_out[ch]->window_sequence != SHORT_WINDOW) {
        start_sfb[ch] = pstr_str_ah_param->start_sfb_long;
      } else {
        start_sfb[ch] = pstr_str_ah_param->start_sfb_short;
      }
    }

    average_energy = 0.0f;
    min_energy = MAX_FLT_VAL;
    ah_cnt = 0;
    for (ch = chn; ch < chn + num_channels; ch++) {
      ixheaace_psy_out_channel *pstr_psy_chan_out = pstr_psy_out[ch];
      for (sfb = start_sfb[ch]; sfb < pstr_psy_chan_out->sfb_count; sfb++) {
        if ((ah_flag[ch][sfb] != NO_AH) &&
            (pstr_psy_chan_out->ptr_sfb_energy[sfb] > pstr_psy_chan_out->ptr_sfb_thr[sfb])) {
          min_energy = MIN(min_energy, pstr_psy_chan_out->ptr_sfb_energy[sfb]);
          average_energy += pstr_psy_chan_out->ptr_sfb_energy[sfb];
          ah_cnt++;
        }
      }
    }

    average_energy = MIN(MAX_FLT_VAL, average_energy / (ah_cnt + MIN_FLT_VAL));

    for (en_idx = 0; en_idx < 4; en_idx++) {
      energy[en_idx] = min_energy * (FLOAT32)pow(average_energy / (min_energy + MIN_FLT_VAL),
                                                 (2 * en_idx + 1) / 7.0f);
    }
    max_sfb = pstr_psy_out[chn]->sfb_count - 1;
    min_sfb = start_sfb[chn];

    if (num_channels == 2) {
      max_sfb = MAX(max_sfb, pstr_psy_out[chn + 1]->sfb_count - 1);

      min_sfb = MIN(min_sfb, start_sfb[chn + 1]);
    }

    sfb = max_sfb;
    en_idx = 0;
    done = 0;
    while (!done) {
      for (ch = chn; ch < chn + num_channels; ch++) {
        ixheaace_psy_out_channel *pstr_psy_chan_out = pstr_psy_out[ch];
        if (sfb >= start_sfb[ch] && sfb < pstr_psy_chan_out->sfb_count) {
          if (ah_flag[ch][sfb] != NO_AH &&
              pstr_psy_chan_out->ptr_sfb_energy[sfb] < energy[en_idx]) {
            ah_flag[ch][sfb] = NO_AH;
            pstr_psy_chan_out->ptr_sfb_thr[sfb] = 2.0f * pstr_psy_chan_out->ptr_sfb_energy[sfb];
            act_pe -= pstr_qs_pe_data->pe_ch_data[ch].sfb_pe[sfb];
          }

          if (act_pe < desired_pe) {
            done = 1;
            break;
          }
        }
      }
      sfb--;
      if (sfb < min_sfb) {
        sfb = max_sfb;
        en_idx++;
        if (en_idx >= 4) {
          done = 1;
        }
      }
    }
  }
}

static VOID iaace_adapt_thr_to_pe(
    ixheaace_psy_out_channel **pstr_psy_out,
    ia_qc_pe_data_struct *pstr_qs_pe_data, const FLOAT32 desired_pe,
    ia_ah_param_struct *pstr_ah_param, ia_min_snr_adapt_param_struct *pstr_msa_param,
    WORD32 num_channels, WORD32 chn, WORD32 aot) {
  FLOAT32 no_red_pe, red_pe, red_pe_no_ah;
  FLOAT32 const_part, const_part_no_ah;
  FLOAT32 num_active_lines, num_active_lines_no_ah;
  FLOAT32 desired_pe_no_ah;
  FLOAT32 redval = 0.0f;
  WORD32 ah_flag[IXHEAACE_MAX_CH_IN_BS_ELE][MAXIMUM_GROUPED_SCALE_FACTOR_BAND];
  FLOAT32 thr_exp[IXHEAACE_MAX_CH_IN_BS_ELE][MAXIMUM_GROUPED_SCALE_FACTOR_BAND];
  WORD32 iteration;

  iaace_calc_thr_exp(thr_exp, pstr_psy_out, num_channels, chn);
  iaace_adapt_min_snr(pstr_psy_out, pstr_msa_param, num_channels, chn);
  iaace_init_avoid_hole_flag(ah_flag, pstr_psy_out, pstr_ah_param, num_channels, chn, aot);

  no_red_pe = pstr_qs_pe_data->pe;
  const_part = pstr_qs_pe_data->const_part;
  num_active_lines = pstr_qs_pe_data->num_active_lines;
  if (num_active_lines > FLT_EPSILON) {
    FLOAT32 avg_thr_exp =
        (FLOAT32)pow(2.0f, (const_part - no_red_pe) / (INV_RED_EXP_VAL * num_active_lines));
    redval = (FLOAT32)pow(2.0f, (const_part - desired_pe) / (INV_RED_EXP_VAL * num_active_lines))
             - avg_thr_exp;
    redval = MAX(0.0f, redval);
    iaace_reduce_thr(pstr_psy_out, ah_flag, thr_exp, redval, num_channels, chn);
  }

  iaace_calc_sfb_pe_data(pstr_qs_pe_data, pstr_psy_out, num_channels, chn);
  red_pe = pstr_qs_pe_data->pe;

  iteration = 0;
  do {
    iaace_calc_pe_no_active_holes(&red_pe_no_ah, &const_part_no_ah, &num_active_lines_no_ah,
                                  pstr_qs_pe_data, ah_flag, pstr_psy_out, num_channels, chn);

    desired_pe_no_ah = MAX(desired_pe - (red_pe - red_pe_no_ah), 0);

    if (num_active_lines_no_ah > FLT_EPSILON) {
      FLOAT32 avg_thr_exp = (FLOAT32)pow(
          2.0f, (const_part_no_ah - red_pe_no_ah) / (INV_RED_EXP_VAL * num_active_lines_no_ah));
      redval += (FLOAT32)pow(2.0f, (const_part_no_ah - desired_pe_no_ah) /
                                       (INV_RED_EXP_VAL * num_active_lines_no_ah)) -
                avg_thr_exp;
      redval = MAX(0.0f, redval);
      iaace_reduce_thr(pstr_psy_out, ah_flag, thr_exp, redval, num_channels, chn);
    }

    iaace_calc_sfb_pe_data(pstr_qs_pe_data, pstr_psy_out, num_channels, chn);

    red_pe = pstr_qs_pe_data->pe;
    iteration++;
  } while ((fabs(red_pe - desired_pe) > (0.05f) * desired_pe) && (iteration < 2));

  if (red_pe < 1.15f * desired_pe) {
    iaace_correct_thr(pstr_psy_out, ah_flag, pstr_qs_pe_data, thr_exp, redval,
                      desired_pe - red_pe, num_channels, chn);
  } else {
    iaace_reduce_min_snr(pstr_psy_out, pstr_qs_pe_data, ah_flag, 1.05f * desired_pe, num_channels,
                         chn);
    iaace_allow_more_holes(pstr_psy_out, pstr_qs_pe_data, ah_flag, pstr_ah_param,
                           1.05f * desired_pe, num_channels, chn);
  }
}

VOID iaace_adjust_threshold(ia_adj_thr_state_struct *pstr_adj_thr_state,
                            ia_adj_thr_elem_struct *pstr_adj_thr_elem,
                            ixheaace_psy_out_channel **pstr_psy_out,
                            FLOAT32 *ptr_ch_bit_dist, ixheaace_qc_out_element *pstr_qc_out_el,
                            const WORD32 avg_bits, const WORD32 bitres_bits,
                            const WORD32 max_bitres_bits, const WORD32 side_info_bits,
                            FLOAT32 *max_bit_fac, FLOAT32 *ptr_sfb_n_relevant_lines,
                            FLOAT32 *ptr_sfb_ld_energy, WORD32 num_channels, WORD32 chn,
                            WORD32 aot, WORD8 *ptr_scratch) {
  FLOAT32 no_red_pe, granted_pe, granted_pe_corr;
  WORD32 curr_win_sequence;
  ia_qc_pe_data_struct *pstr_qc_pe_data = (ia_qc_pe_data_struct *)ptr_scratch;
  FLOAT32 bit_factor;
  WORD32 ch;

  for (ch = chn; ch < chn + num_channels; ch++) {
    pstr_qc_pe_data->pe_ch_data[ch].sfb_lines =
        ptr_sfb_n_relevant_lines + MAXIMUM_GROUPED_SCALE_FACTOR_BAND * ch;
    pstr_qc_pe_data->pe_ch_data[ch].sfb_ld_energy =
        ptr_sfb_ld_energy + MAXIMUM_GROUPED_SCALE_FACTOR_BAND * ch;
  }
  pstr_qc_pe_data->offset = pstr_adj_thr_elem->pe_offset;

  iaace_calc_sfb_pe_data(pstr_qc_pe_data, pstr_psy_out, num_channels, chn);

  no_red_pe = pstr_qc_pe_data->pe;

  curr_win_sequence = LONG_WINDOW;
  if (num_channels == 2) {
    if ((pstr_psy_out[chn]->window_sequence == SHORT_WINDOW) ||
        (pstr_psy_out[chn + 1]->window_sequence == SHORT_WINDOW)) {
      curr_win_sequence = SHORT_WINDOW;
    }
  } else {
    curr_win_sequence = pstr_psy_out[chn]->window_sequence;
  }

  bit_factor = iaace_bitres_calc_bitfac(
      bitres_bits, max_bitres_bits, no_red_pe + 5.0f * side_info_bits, curr_win_sequence,
      avg_bits, *max_bit_fac, pstr_adj_thr_state, pstr_adj_thr_elem);

  granted_pe = bit_factor * iaace_bits_to_pe((FLOAT32)avg_bits);

  iaace_calc_pe_correction(&(pstr_adj_thr_elem->pe_correction_fac), MIN(granted_pe, no_red_pe),
                           pstr_adj_thr_elem->pe_last, pstr_adj_thr_elem->dyn_bits_last);

  granted_pe_corr = granted_pe * pstr_adj_thr_elem->pe_correction_fac;

  if (granted_pe_corr < no_red_pe) {
    iaace_adapt_thr_to_pe(pstr_psy_out, pstr_qc_pe_data, granted_pe_corr,
                          &pstr_adj_thr_elem->str_ah_param,
                          &pstr_adj_thr_elem->str_min_snr_adapt_params, num_channels, chn, aot);
  }

  for (ch = chn; ch < chn + num_channels; ch++) {
    FLOAT32 tmp_var, temp1;
    if (pstr_qc_pe_data->pe) {
      tmp_var = 1.0f - num_channels * 0.2f;
      temp1 = pstr_qc_pe_data->pe_ch_data[ch].pe / pstr_qc_pe_data->pe;
      temp1 = temp1 * tmp_var;
      ptr_ch_bit_dist[ch] = temp1 + 0.2f;
      if (ptr_ch_bit_dist[ch] < 0.2f) {
        ptr_ch_bit_dist[ch] = 0.2f;
      }
    } else {
      ptr_ch_bit_dist[ch] = 0.2f;
    }
  }

  pstr_qc_out_el->pe = no_red_pe;
  pstr_adj_thr_elem->pe_last = granted_pe;
}
