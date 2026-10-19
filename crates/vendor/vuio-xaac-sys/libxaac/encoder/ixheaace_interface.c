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
#include "ixheaac_type_def.h"
#include "ixheaac_constants.h"
#include "ixheaace_aac_constants.h"

#include "ixheaace_psy_const.h"
#include "ixheaace_tns.h"
#include "ixheaace_tns_params.h"
#include "ixheaace_rom.h"
#include "ixheaace_common_rom.h"
#include "ixheaace_bitbuffer.h"
#include "ixheaace_block_switch.h"
#include "ixheaace_psy_data.h"
#include "ixheaace_interface.h"

VOID ia_enhaacplus_enc_build_interface(
    FLOAT32 *ptr_grouped_mdct_spec, ixheaace_sfb_energy *pstr_grouped_sfb_thres,
    ixheaace_sfb_energy *pstr_grouped_sfb_nrg, ixheaace_sfb_energy *pstr_grouped_sfb_spreaded_nrg,
    const ixheaace_sfb_energy_sum sfb_nrg_sum_lr, const ixheaace_sfb_energy_sum sfb_nrg_sum_ms,
    const WORD32 win_seq, const WORD32 win_shape, const WORD32 grouped_sfb_cnt,
    const WORD32 *ptr_grouped_sfb_offset, const WORD32 max_sfb_per_grp,
    const FLOAT32 *ptr_grouped_sfb_min_snr, const WORD32 total_groups_cnt,
    const WORD32 *ptr_group_len, ixheaace_psy_out_channel *pstr_psy_out_ch) {
  WORD32 j;
  WORD32 grp;
  WORD32 mask;

  /* Copy values to psy_out */
  pstr_psy_out_ch->max_sfb_per_grp = max_sfb_per_grp;
  pstr_psy_out_ch->sfb_count = grouped_sfb_cnt;
  pstr_psy_out_ch->sfb_per_group = grouped_sfb_cnt / total_groups_cnt;
  pstr_psy_out_ch->window_sequence = win_seq;
  pstr_psy_out_ch->window_shape = win_shape;
  pstr_psy_out_ch->ptr_spec_coeffs = ptr_grouped_mdct_spec;

  pstr_psy_out_ch->ptr_sfb_energy = pstr_grouped_sfb_nrg->long_nrg;
  pstr_psy_out_ch->ptr_sfb_thr = pstr_grouped_sfb_thres->long_nrg;
  pstr_psy_out_ch->ptr_sfb_spread_energy = pstr_grouped_sfb_spreaded_nrg->long_nrg;

  memcpy(&pstr_psy_out_ch->sfb_offsets[0], &ptr_grouped_sfb_offset[0],
         sizeof(pstr_psy_out_ch->sfb_offsets[0]) * (grouped_sfb_cnt + 1));
  memcpy(&pstr_psy_out_ch->sfb_min_snr[0], &ptr_grouped_sfb_min_snr[0],
         sizeof(pstr_psy_out_ch->sfb_min_snr[0]) * (grouped_sfb_cnt));

  /* Generate grouping mask */
  mask = 0;

  for (grp = 0; grp < total_groups_cnt; grp++) {
    mask <<= 1;

    for (j = 1; j < ptr_group_len[grp]; j++) {
      mask <<= 1;
      mask |= 1;
    }
  }
  /* copy energy sums to psy_out for stereo preprocessing */
  if (win_seq != SHORT_WINDOW) {
    pstr_psy_out_ch->sfb_sum_lr_energy = sfb_nrg_sum_lr.long_nrg;
    pstr_psy_out_ch->sfb_sum_ms_energy = sfb_nrg_sum_ms.long_nrg;
  } else {
    int i;

    pstr_psy_out_ch->sfb_sum_ms_energy = 0;
    pstr_psy_out_ch->sfb_sum_lr_energy = 0;

    for (i = 0; i < TRANS_FAC; i++) {
      pstr_psy_out_ch->sfb_sum_lr_energy += sfb_nrg_sum_lr.short_nrg[i];
      pstr_psy_out_ch->sfb_sum_ms_energy += sfb_nrg_sum_ms.short_nrg[i];
    }
  }

  pstr_psy_out_ch->grouping_mask = mask;
}
