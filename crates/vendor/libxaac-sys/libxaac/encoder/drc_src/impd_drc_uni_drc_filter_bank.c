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
#include "ixheaac_error_standards.h"
#include "ixheaace_error_codes.h"

#include "iusace_cnst.h"
#include "impd_drc_common_enc.h"
#include "impd_drc_uni_drc.h"
#include "impd_drc_uni_drc_filter_bank.h"

static IA_ERRORCODE impd_drc_filter_bank_complexity(
    const WORD32 num_bands, ia_drc_filter_bank_struct *pstr_drc_filter_bank) {
  pstr_drc_filter_bank->complexity = 0;
  pstr_drc_filter_bank->num_bands = num_bands;
  switch (num_bands) {
    case 1:
      break;
    case 2:
      pstr_drc_filter_bank->complexity = 8;
      break;
    case 3:
      pstr_drc_filter_bank->complexity = 18;
      break;
    case 4:
      pstr_drc_filter_bank->complexity = 28;
      break;
    default:
      return IA_EXHEAACE_CONFIG_FATAL_DRC_PARAM_OUT_OF_RANGE;
      break;
  }

  return IA_NO_ERROR;
}

IA_ERRORCODE impd_drc_init_all_filter_banks(
    const ia_drc_coefficients_uni_drc_struct *pstr_drc_coefficients_uni_drc,
    const ia_drc_instructions_uni_drc *pstr_drc_instructions_uni_drc,
    ia_drc_filter_banks_struct *pstr_filter_banks, VOID *ptr_scratch) {
  IA_ERRORCODE err_code = IA_NO_ERROR;
  LOOPIDX band_idx, group_idx, i, j, k;
  WORD32 crossover_freq_index, num_ch_in_groups, num_phase_alignment_ch_groups;
  WORD32 index_found = FALSE;
  WORD32 group_count[MAX_CHANNEL_GROUP_COUNT + 1] = {0};
  WORD32 *ptr_cascade_crossover_indices[MAX_CHANNEL_GROUP_COUNT + 1];

  for (i = 0; i < MAX_CHANNEL_GROUP_COUNT + 1; i++) {
    ptr_cascade_crossover_indices[i] = (WORD32 *)(ptr_scratch);
    ptr_scratch = (UWORD8 *)ptr_scratch +
                  (MAX_CHANNEL_GROUP_COUNT * 3) * sizeof(ptr_cascade_crossover_indices[i][0]);
  }

  num_ch_in_groups = 0;
  for (group_idx = 0; group_idx < pstr_drc_instructions_uni_drc->num_drc_channel_groups;
       group_idx++) {
    num_ch_in_groups += pstr_drc_instructions_uni_drc->num_channels_per_channel_group[group_idx];
  }
  num_phase_alignment_ch_groups = pstr_drc_instructions_uni_drc->num_drc_channel_groups;
  if (num_ch_in_groups < pstr_drc_instructions_uni_drc->drc_channel_count) {
    num_phase_alignment_ch_groups++;
  }
  if (num_phase_alignment_ch_groups > IMPD_DRCMAX_PHASE_ALIGN_CH_GROUP) {
    return IA_EXHEAACE_CONFIG_FATAL_DRC_PARAM_OUT_OF_RANGE;
  }

  memset(pstr_filter_banks->str_drc_filter_bank, 0,
         sizeof(pstr_filter_banks->str_drc_filter_bank));
  pstr_filter_banks->num_phase_alignment_ch_groups = num_phase_alignment_ch_groups;
  pstr_filter_banks->num_filter_banks = pstr_drc_instructions_uni_drc->num_drc_channel_groups;
  if (pstr_drc_coefficients_uni_drc != NULL) {
    for (group_idx = 0; group_idx < pstr_drc_instructions_uni_drc->num_drc_channel_groups;
         group_idx++) {
      err_code = impd_drc_filter_bank_complexity(
          pstr_drc_coefficients_uni_drc
              ->str_gain_set_params[pstr_drc_instructions_uni_drc
                                        ->gain_set_index_for_channel_group[group_idx]]
              .band_count,
          &(pstr_filter_banks->str_drc_filter_bank[group_idx]));
      if (err_code) {
        return err_code;
      }
    }
  } else {
    pstr_filter_banks->str_drc_filter_bank->num_bands = 1;
  }

  if (pstr_drc_coefficients_uni_drc != NULL) {
    for (group_idx = 0; group_idx < pstr_drc_instructions_uni_drc->num_drc_channel_groups;
         group_idx++) {
      for (band_idx = 1;
           band_idx < pstr_drc_coefficients_uni_drc
                          ->str_gain_set_params[pstr_drc_instructions_uni_drc
                                                    ->gain_set_index_for_channel_group[group_idx]]
                          .band_count;
           band_idx++) {
        crossover_freq_index =
            pstr_drc_coefficients_uni_drc
                ->str_gain_set_params[pstr_drc_instructions_uni_drc
                                          ->gain_set_index_for_channel_group[group_idx]]
                .gain_params[band_idx]
                .crossover_freq_index;

        for (j = 0; j < num_phase_alignment_ch_groups; j++) {
          if (j != group_idx) {
            ptr_cascade_crossover_indices[j][group_count[j]] = crossover_freq_index;
            group_count[j]++;
            if (group_count[j] > MAX_CHANNEL_GROUP_COUNT * 3) {
              return IA_EXHEAACE_CONFIG_FATAL_DRC_PARAM_OUT_OF_RANGE;
            }
          }
        }
      }
    }
  }

  i = 0;
  while (i < group_count[0]) {
    crossover_freq_index = ptr_cascade_crossover_indices[0][i];
    index_found = FALSE;
    for (group_idx = 1; group_idx < num_phase_alignment_ch_groups; group_idx++) {
      index_found = FALSE;
      for (j = 0; j < group_count[group_idx]; j++) {
        if (ptr_cascade_crossover_indices[group_idx][j] == crossover_freq_index) {
          index_found = TRUE;
          break;
        }
      }
      if (index_found == FALSE) {
        break;
      }
    }
    if (index_found == FALSE) {
      i++;
    } else {
      for (group_idx = 0; group_idx < num_phase_alignment_ch_groups; group_idx++) {
        for (j = 0; j < group_count[group_idx]; j++) {
          if (ptr_cascade_crossover_indices[group_idx][j] == crossover_freq_index) {
            for (k = j + 1; k < group_count[group_idx]; k++) {
              ptr_cascade_crossover_indices[group_idx][k - 1] =
                  ptr_cascade_crossover_indices[group_idx][k];
            }
            group_count[group_idx]--;
            break;
          }
        }
      }
      i = 0;
    }
  }
  for (group_idx = 0; group_idx < num_phase_alignment_ch_groups; group_idx++) {
    if (group_count[group_idx] > 0) {
      pstr_filter_banks->str_drc_filter_bank[group_idx].complexity +=
          (group_count[group_idx] << 1);
    }
  }
  pstr_filter_banks->complexity = 0;
  for (group_idx = 0; group_idx < pstr_drc_instructions_uni_drc->num_drc_channel_groups;
       group_idx++) {
    pstr_filter_banks->complexity +=
        pstr_drc_instructions_uni_drc->num_channels_per_channel_group[group_idx] *
        pstr_filter_banks->str_drc_filter_bank[group_idx].complexity;
  }

  return err_code;
}
