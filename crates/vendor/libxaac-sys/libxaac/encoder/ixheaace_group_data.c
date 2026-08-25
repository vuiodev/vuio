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
#include <stdlib.h>
#include "ixheaac_type_def.h"
#include "ixheaac_constants.h"
#include "ixheaace_psy_const.h"
#include "ixheaace_tns.h"
#include "ixheaace_tns_params.h"
#include "ixheaace_rom.h"
#include "ixheaace_common_rom.h"
#include "ixheaace_bitbuffer.h"
#include "ixheaace_aac_constants.h"
#include "ixheaace_block_switch.h"
#include "ixheaace_psy_data.h"
#include "ixheaace_interface.h"
#include "ixheaace_group_data.h"

#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops16.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaac_basic_ops.h"

VOID iaace_group_short_data(FLOAT32 *ptr_mdct_spectrum, FLOAT32 *ptr_tmp_spectrum,
                            ixheaace_sfb_energy *pstr_sfb_threshold,
                            ixheaace_sfb_energy *pstr_sfb_energy,
                            ixheaace_sfb_energy *pstr_sfb_energy_ms,
                            ixheaace_sfb_energy *pstr_sfb_spreaded_energy, const WORD32 sfb_cnt,
                            const WORD32 *ptr_sfb_offset, const FLOAT32 *ptr_sfb_min_snr,
                            WORD32 *ptr_grouped_sfb_offset, WORD32 *ptr_max_sfb_per_group,
                            FLOAT32 *ptr_grouped_sfb_min_snr, const WORD32 no_of_groups,
                            const WORD32 *ptr_group_len, WORD32 frame_length) {
  WORD32 i, j;
  WORD32 line;
  WORD32 sfb;
  WORD32 grp;
  WORD32 wnd;
  WORD32 offset;
  WORD32 highest_sfb;

  /* For short: regroup and cumulate energies und thresholds group-wise */
  /* Calculate sfb_cnt */

  highest_sfb = 0;

  for (wnd = 0; wnd < TRANS_FAC; wnd++) {
    for (sfb = sfb_cnt - 1; sfb >= highest_sfb; sfb--) {
      for (line = ptr_sfb_offset[sfb + 1] - 1; line >= ptr_sfb_offset[sfb]; line--) {
        if (ptr_mdct_spectrum[wnd * (frame_length / 8) + line] != 0.0) {
          break;
        }
      }

      if (line >= ptr_sfb_offset[sfb]) {
        break;
      }
    }
    highest_sfb = MAX(highest_sfb, sfb);
  }

  highest_sfb = highest_sfb > 0 ? highest_sfb : 0;

  *ptr_max_sfb_per_group = highest_sfb + 1;

  /* Calculate ptr_sfb_offset */
  i = 0;
  offset = 0;

  for (grp = 0; grp < no_of_groups; grp++) {
    for (sfb = 0; sfb < sfb_cnt; sfb++) {
      ptr_grouped_sfb_offset[i++] = offset + ptr_sfb_offset[sfb] * ptr_group_len[grp];
    }

    offset += ptr_group_len[grp] * (frame_length / 8);
  }

  ptr_grouped_sfb_offset[i++] = frame_length;

  /* Calculate min SNR */

  i = 0;
  offset = 0;

  for (grp = 0; grp < no_of_groups; grp++) {
    memcpy(&ptr_grouped_sfb_min_snr[i], &ptr_sfb_min_snr[0], sfb_cnt * sizeof(*ptr_sfb_min_snr));
    i += sfb_cnt;

    offset += ptr_group_len[grp] * (frame_length / 8);
  }

  wnd = 0;
  i = 0;

  for (grp = 0; grp < no_of_groups; grp++) {
    for (sfb = 0; sfb < sfb_cnt; sfb++) {
      FLOAT32 thresh = pstr_sfb_threshold->short_nrg[wnd][sfb];

      for (j = 1; j < ptr_group_len[grp]; j++) {
        thresh += pstr_sfb_threshold->short_nrg[wnd + j][sfb];
      }

      pstr_sfb_threshold->long_nrg[i++] = thresh;
    }
    wnd += ptr_group_len[grp];
  }

  /* Sum up sfb energies -  left/right */
  wnd = 0;
  i = 0;

  for (grp = 0; grp < no_of_groups; grp++) {
    for (sfb = 0; sfb < sfb_cnt; sfb++) {
      FLOAT32 energy = pstr_sfb_energy->short_nrg[wnd][sfb];

      for (j = 1; j < ptr_group_len[grp]; j++) {
        energy += pstr_sfb_energy->short_nrg[wnd + j][sfb];
      }

      pstr_sfb_energy->long_nrg[i++] = energy;
    }
    wnd += ptr_group_len[grp];
  }

  /* Sum up sfb energies mid/side */
  wnd = 0;
  i = 0;

  for (grp = 0; grp < no_of_groups; grp++) {
    for (sfb = 0; sfb < sfb_cnt; sfb++) {
      FLOAT32 energy = pstr_sfb_energy_ms->short_nrg[wnd][sfb];

      for (j = 1; j < ptr_group_len[grp]; j++) {
        energy += pstr_sfb_energy_ms->short_nrg[wnd + j][sfb];
      }

      pstr_sfb_energy_ms->long_nrg[i++] = energy;
    }
    wnd += ptr_group_len[grp];
  }

  /* Sum up sfb spreaded energies */
  wnd = 0;
  i = 0;

  for (grp = 0; grp < no_of_groups; grp++) {
    for (sfb = 0; sfb < sfb_cnt; sfb++) {
      FLOAT32 energy = pstr_sfb_spreaded_energy->short_nrg[wnd][sfb];

      for (j = 1; j < ptr_group_len[grp]; j++) {
        energy += pstr_sfb_spreaded_energy->short_nrg[wnd + j][sfb];
      }
      pstr_sfb_spreaded_energy->long_nrg[i++] = energy;
    }
    wnd += ptr_group_len[grp];
  }

  /* Re-group spectrum */
  wnd = 0;
  i = 0;

  for (grp = 0; grp < no_of_groups; grp++) {
    for (sfb = 0; sfb < sfb_cnt; sfb++) {
      for (j = 0; j < ptr_group_len[grp]; j++) {
        for (line = ptr_sfb_offset[sfb]; line < ptr_sfb_offset[sfb + 1]; line++) {
          ptr_tmp_spectrum[i++] = ptr_mdct_spectrum[(wnd + j) * (frame_length / 8) + line];
        }
      }
    }
    wnd += ptr_group_len[grp];
  }

  memcpy(ptr_mdct_spectrum, ptr_tmp_spectrum, frame_length * sizeof(*ptr_mdct_spectrum));
}
