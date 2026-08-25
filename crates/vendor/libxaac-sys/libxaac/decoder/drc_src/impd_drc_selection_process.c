/******************************************************************************
 *
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
#include <stdio.h>
#include <stdlib.h>
#include <math.h>
#include <string.h>
#include "impd_type_def.h"
#include "impd_drc_extr_delta_coded_info.h"
#include "impd_drc_common.h"
#include "impd_drc_struct.h"
#include "impd_drc_interface.h"
#include "impd_drc_selection_process.h"
#include "impd_drc_sel_proc_drc_set_sel.h"

WORD32 impd_drc_uni_selction_proc_init(
    ia_drc_sel_pro_struct* pstr_drc_uni_sel_proc,
    ia_drc_sel_proc_params_struct* pstr_drc_sel_proc_params_struct,
    ia_drc_interface_struct* pstr_drc_interface, WORD32 subband_domain_mode) {
  WORD32 err = 0;

  if (pstr_drc_uni_sel_proc == NULL) {
    return 1;
  }

  if (pstr_drc_uni_sel_proc->first_frame == 1) {
    err = impd_drc_sel_proc_init_dflt(pstr_drc_uni_sel_proc);
    if (err) return (err);
  }

  err = impd_drc_sel_proc_init_sel_proc_params(pstr_drc_uni_sel_proc,
                                               pstr_drc_sel_proc_params_struct);
  if (err) return (err);
  {
    WORD32 i;
    pstr_drc_uni_sel_proc->drc_set_id_valid_flag[0] = 1;
    for (i = 1; i < DRC_INSTRUCTIONS_COUNT_MAX; i++) {
      pstr_drc_uni_sel_proc->drc_set_id_valid_flag[i] = 0;
    }

    pstr_drc_uni_sel_proc->eq_set_id_valid_flag[0] = 1;
    for (i = 1; i < EQ_INSTRUCTIONS_COUNT_MAX; i++) {
      pstr_drc_uni_sel_proc->eq_set_id_valid_flag[i] = 0;
    }
  }
  impd_drc_sel_proc_init_interface_params(pstr_drc_uni_sel_proc,
                                          pstr_drc_interface);

  pstr_drc_uni_sel_proc->subband_domain_mode = subband_domain_mode;

  return 0;
}

WORD32
impd_drc_uni_sel_proc_process(
    ia_drc_sel_pro_struct* pstr_drc_uni_sel_proc,
    ia_drc_config* pstr_drc_config,
    ia_drc_loudness_info_set_struct* pstr_loudness_info,
    ia_drc_sel_proc_output_struct* hia_drc_sel_proc_output_struct) {
  WORD32 i, err, drc_set_id_selected, activeDrcSetIndex;
  WORD32 eq_set_id_selected;
  WORD32 loudEqSetIdSelected;

  if (pstr_drc_config != NULL) {
    if (memcmp(&pstr_drc_uni_sel_proc->drc_config, pstr_drc_config,
               sizeof(ia_drc_config))) {
      pstr_drc_uni_sel_proc->drc_config = *pstr_drc_config;
      pstr_drc_uni_sel_proc->drc_config_flag = 1;

      if (pstr_drc_uni_sel_proc->uni_drc_sel_proc_params.base_channel_count !=
          pstr_drc_uni_sel_proc->drc_config.channel_layout.base_channel_count) {
        pstr_drc_uni_sel_proc->uni_drc_sel_proc_params.base_channel_count =
            pstr_drc_uni_sel_proc->drc_config.channel_layout.base_channel_count;
      }
      if (pstr_drc_uni_sel_proc->drc_config.channel_layout
                  .layout_signaling_present == 1 &&
          pstr_drc_uni_sel_proc->uni_drc_sel_proc_params.base_layout !=
              pstr_drc_uni_sel_proc->drc_config.channel_layout.defined_layout) {
        pstr_drc_uni_sel_proc->uni_drc_sel_proc_params.base_layout =
            pstr_drc_uni_sel_proc->drc_config.channel_layout.defined_layout;
      }
    } else {
      pstr_drc_uni_sel_proc->drc_config_flag = 0;
    }
  }
  if (pstr_loudness_info != NULL) {
    if (memcmp(&pstr_drc_uni_sel_proc->loudness_info_set, pstr_loudness_info,
               sizeof(ia_drc_loudness_info_set_struct))) {
      pstr_drc_uni_sel_proc->loudness_info_set = *pstr_loudness_info;
      pstr_drc_uni_sel_proc->loudness_info_set_flag = 1;
    } else {
      pstr_drc_uni_sel_proc->loudness_info_set_flag = 0;
    }
  }

  if ((pstr_drc_uni_sel_proc->drc_config_flag &&
       pstr_drc_uni_sel_proc->uni_drc_sel_proc_params
               .target_config_request_type != 0) ||
      (pstr_drc_uni_sel_proc->sel_proc_request_flag &&
       pstr_drc_uni_sel_proc->uni_drc_sel_proc_params
               .target_config_request_type != 0) ||
      (pstr_drc_uni_sel_proc->uni_drc_sel_proc_params
               .target_config_request_type == 0 &&
       pstr_drc_uni_sel_proc->uni_drc_sel_proc_params.num_downmix_id_requests ==
           0)) {
    err = impd_map_target_config_req_downmix_id(
        pstr_drc_uni_sel_proc, &pstr_drc_uni_sel_proc->drc_config);
    if (err) return (err);
  }

  if (pstr_drc_uni_sel_proc->drc_config_flag ||
      pstr_drc_uni_sel_proc->loudness_info_set_flag ||
      pstr_drc_uni_sel_proc->sel_proc_request_flag) {
    WORD32 repeat_selection = 1;

    err = impd_manage_drc_complexity(pstr_drc_uni_sel_proc, pstr_drc_config);
    if (err) return (err);
    err = impd_manage_eq_complexity(pstr_drc_uni_sel_proc, pstr_drc_config);
    if (err) return (err);
    while (repeat_selection == 1) {
      err = impd_select_drc_set(pstr_drc_uni_sel_proc, &drc_set_id_selected,
                                &eq_set_id_selected, &loudEqSetIdSelected);
      if (err) return (err);

      err =
          impd_get_selected_drc_set(pstr_drc_uni_sel_proc, drc_set_id_selected);
      if (err) return (err);

      err = impd_get_dependent_drc_set(pstr_drc_uni_sel_proc);
      if (err) return (err);

      err = impd_get_fading_drc_set(pstr_drc_uni_sel_proc);
      if (err) return (err);

      err = impd_get_ducking_drc_set(pstr_drc_uni_sel_proc);
      if (err) return (err);

      pstr_drc_uni_sel_proc->eq_inst_index[0] = -1;
      pstr_drc_uni_sel_proc->eq_inst_index[1] = -1;

      err = impd_get_selected_eq_set(pstr_drc_uni_sel_proc, eq_set_id_selected);
      if (err) return (err);

      err = impd_get_dependent_eq_set(pstr_drc_uni_sel_proc);
      if (err) return (err);

      err = impd_get_selected_loud_eq_set(pstr_drc_uni_sel_proc,
                                          loudEqSetIdSelected);
      if (err) return (err);

      activeDrcSetIndex = 0;
      for (i = SUB_DRC_COUNT - 1; i >= 0; i--) {
        WORD32 drc_instructions_index =
            pstr_drc_uni_sel_proc->drc_instructions_index[i];
        ia_drc_instructions_struct* str_drc_instruction_str;
        if (drc_instructions_index < 0) continue;

        str_drc_instruction_str =
            &(pstr_drc_uni_sel_proc->drc_config
                  .str_drc_instruction_str[drc_instructions_index]);

        if (str_drc_instruction_str->drc_set_id > 0) {
          pstr_drc_uni_sel_proc->uni_drc_sel_proc_output
              .sel_drc_set_ids[activeDrcSetIndex] =
              str_drc_instruction_str->drc_set_id;

          if ((i == 3) && (str_drc_instruction_str->drc_set_effect &
                           (EFFECT_BIT_DUCK_SELF | EFFECT_BIT_DUCK_OTHER))) {
            pstr_drc_uni_sel_proc->uni_drc_sel_proc_output
                .sel_downmix_ids[activeDrcSetIndex] = 0;
          } else {
            if (str_drc_instruction_str->drc_apply_to_dwnmix == 1) {
              pstr_drc_uni_sel_proc->uni_drc_sel_proc_output
                  .sel_downmix_ids[activeDrcSetIndex] =
                  str_drc_instruction_str->downmix_id[0];
            } else {
              pstr_drc_uni_sel_proc->uni_drc_sel_proc_output
                  .sel_downmix_ids[activeDrcSetIndex] = 0;
            }
          }

          activeDrcSetIndex++;
        }
      }
      if (activeDrcSetIndex <= 3) {
        pstr_drc_uni_sel_proc->uni_drc_sel_proc_output.num_sel_drc_sets =
            activeDrcSetIndex;
      } else {
        pstr_drc_uni_sel_proc->uni_drc_sel_proc_output.num_sel_drc_sets = -1;
        return (UNEXPECTED_ERROR);
      }

      impd_sel_downmix_matrix(pstr_drc_uni_sel_proc,
                              &pstr_drc_uni_sel_proc->drc_config);

      err = impd_manage_complexity(pstr_drc_uni_sel_proc, pstr_drc_config,
                                   &repeat_selection);
      if (err) return (err);
    }

    pstr_drc_uni_sel_proc->sel_proc_request_flag = 0;
    pstr_drc_uni_sel_proc->uni_drc_sel_proc_output.boost =
        pstr_drc_uni_sel_proc->uni_drc_sel_proc_params.boost;
    pstr_drc_uni_sel_proc->uni_drc_sel_proc_output.compress =
        pstr_drc_uni_sel_proc->uni_drc_sel_proc_params.compress;
    pstr_drc_uni_sel_proc->uni_drc_sel_proc_output.drc_characteristic_target =
        pstr_drc_uni_sel_proc->uni_drc_sel_proc_params
            .drc_characteristic_target;
    pstr_drc_uni_sel_proc->uni_drc_sel_proc_output
        .loudness_normalization_gain_db +=
        pstr_drc_uni_sel_proc->uni_drc_sel_proc_params
            .loudness_norm_gain_modification_db;
  }
  for (i = 0; i < 2; i++) {
    if (pstr_drc_uni_sel_proc->eq_inst_index[i] >= 0) {
      pstr_drc_uni_sel_proc->uni_drc_sel_proc_output.sel_eq_set_ids[i] =
          pstr_drc_uni_sel_proc->drc_config.str_drc_config_ext
              .str_eq_instructions[pstr_drc_uni_sel_proc->eq_inst_index[i]]
              .eq_set_id;
    }
  }
  if (pstr_drc_uni_sel_proc->loud_eq_inst_index_sel >= 0) {
    pstr_drc_uni_sel_proc->uni_drc_sel_proc_output.sel_loud_eq_id =
        pstr_drc_uni_sel_proc->drc_config.str_drc_config_ext
            .loud_eq_instructions[pstr_drc_uni_sel_proc->loud_eq_inst_index_sel]
            .loud_eq_set_id;
  } else {
    pstr_drc_uni_sel_proc->uni_drc_sel_proc_output.sel_loud_eq_id = 0;
  }
  *hia_drc_sel_proc_output_struct =
      pstr_drc_uni_sel_proc->uni_drc_sel_proc_output;

  return 0;
}

WORD32 impd_map_target_config_req_downmix_id(
    ia_drc_sel_pro_struct* pstr_drc_uni_sel_proc,
    ia_drc_config* pstr_drc_config) {
  WORD32 i, dwnmix_instructions_count;
  WORD32 target_ch_count_prelim =
      pstr_drc_uni_sel_proc->uni_drc_sel_proc_params.base_channel_count;

  pstr_drc_uni_sel_proc->uni_drc_sel_proc_params.num_downmix_id_requests = 0;
  switch (pstr_drc_uni_sel_proc->uni_drc_sel_proc_params
              .target_config_request_type) {
    case 0:
      if (pstr_drc_uni_sel_proc->uni_drc_sel_proc_params
              .num_downmix_id_requests == 0) {
        pstr_drc_uni_sel_proc->uni_drc_sel_proc_params.requested_dwnmix_id[0] =
            0;
        pstr_drc_uni_sel_proc->uni_drc_sel_proc_params.num_downmix_id_requests =
            1;
      }
      break;
    case 1:
      if (pstr_drc_uni_sel_proc->uni_drc_sel_proc_params
              .requested_target_layout ==
          pstr_drc_uni_sel_proc->uni_drc_sel_proc_params.base_layout) {
        pstr_drc_uni_sel_proc->uni_drc_sel_proc_params.requested_dwnmix_id[0] =
            0;
        pstr_drc_uni_sel_proc->uni_drc_sel_proc_params.num_downmix_id_requests =
            1;
      }
      if (pstr_drc_uni_sel_proc->uni_drc_sel_proc_params
              .num_downmix_id_requests == 0) {
        dwnmix_instructions_count =
            pstr_drc_uni_sel_proc->drc_config.dwnmix_instructions_count;
        for (i = 0; i < dwnmix_instructions_count; i++) {
          ia_downmix_instructions_struct* dwnmix_instructions =
              &(pstr_drc_config->dwnmix_instructions[i]);
          if (pstr_drc_uni_sel_proc->uni_drc_sel_proc_params
                  .requested_target_layout ==
              dwnmix_instructions->target_layout) {
            pstr_drc_uni_sel_proc->uni_drc_sel_proc_params.requested_dwnmix_id
                [pstr_drc_uni_sel_proc->uni_drc_sel_proc_params
                     .num_downmix_id_requests] =
                dwnmix_instructions->downmix_id;
            pstr_drc_uni_sel_proc->uni_drc_sel_proc_params
                .num_downmix_id_requests += 1;
            target_ch_count_prelim = dwnmix_instructions->target_channel_count;
          }
        }
      }

      if (pstr_drc_uni_sel_proc->uni_drc_sel_proc_params
              .num_downmix_id_requests == 0) {
        pstr_drc_uni_sel_proc->uni_drc_sel_proc_params.requested_dwnmix_id[0] =
            0;
        pstr_drc_uni_sel_proc->uni_drc_sel_proc_params.num_downmix_id_requests =
            1;
      }
      break;
    case 2:
      if (pstr_drc_uni_sel_proc->uni_drc_sel_proc_params
              .requested_target_ch_count ==
          pstr_drc_uni_sel_proc->uni_drc_sel_proc_params.base_channel_count) {
        pstr_drc_uni_sel_proc->uni_drc_sel_proc_params.requested_dwnmix_id[0] =
            0;
        pstr_drc_uni_sel_proc->uni_drc_sel_proc_params.num_downmix_id_requests =
            1;
      }
      if (pstr_drc_uni_sel_proc->uni_drc_sel_proc_params
              .num_downmix_id_requests == 0) {
        dwnmix_instructions_count =
            pstr_drc_uni_sel_proc->drc_config.dwnmix_instructions_count;
        for (i = 0; i < dwnmix_instructions_count; i++) {
          ia_downmix_instructions_struct* dwnmix_instructions =
              &(pstr_drc_config->dwnmix_instructions[i]);
          if (pstr_drc_uni_sel_proc->uni_drc_sel_proc_params
                  .requested_target_ch_count ==
              dwnmix_instructions->target_channel_count) {
            pstr_drc_uni_sel_proc->uni_drc_sel_proc_params.requested_dwnmix_id
                [pstr_drc_uni_sel_proc->uni_drc_sel_proc_params
                     .num_downmix_id_requests] =
                dwnmix_instructions->downmix_id;
            pstr_drc_uni_sel_proc->uni_drc_sel_proc_params
                .num_downmix_id_requests += 1;
            target_ch_count_prelim = dwnmix_instructions->target_channel_count;
          }
        }
      }

      if (pstr_drc_uni_sel_proc->uni_drc_sel_proc_params
              .num_downmix_id_requests == 0) {
        pstr_drc_uni_sel_proc->uni_drc_sel_proc_params.requested_dwnmix_id[0] =
            0;
        pstr_drc_uni_sel_proc->uni_drc_sel_proc_params.num_downmix_id_requests =
            1;
      }
      break;
    default:
      return UNEXPECTED_ERROR;
      break;
  }
  pstr_drc_uni_sel_proc->uni_drc_sel_proc_params.target_ch_count_prelim =
      target_ch_count_prelim;

  return 0;
}

VOID impd_sel_downmix_matrix(ia_drc_sel_pro_struct* pstr_drc_uni_sel_proc,
                             ia_drc_config* pstr_drc_config) {
  WORD32 i, j, n;

  pstr_drc_uni_sel_proc->uni_drc_sel_proc_output.base_channel_count =
      pstr_drc_config->channel_layout.base_channel_count;
  pstr_drc_uni_sel_proc->uni_drc_sel_proc_output.target_channel_count =
      pstr_drc_config->channel_layout.base_channel_count;
  pstr_drc_uni_sel_proc->uni_drc_sel_proc_output.target_layout = -1;
  pstr_drc_uni_sel_proc->uni_drc_sel_proc_output.downmix_matrix_present = 0;
  pstr_drc_uni_sel_proc->downmix_inst_index_sel = -1;

  if (pstr_drc_uni_sel_proc->uni_drc_sel_proc_output.active_downmix_id != 0) {
    for (n = 0; n < pstr_drc_config->dwnmix_instructions_count; n++) {
      ia_downmix_instructions_struct* dwnmix_instructions =
          &(pstr_drc_config->dwnmix_instructions[n]);

      if (pstr_drc_uni_sel_proc->uni_drc_sel_proc_output.active_downmix_id ==
          dwnmix_instructions->downmix_id) {
        pstr_drc_uni_sel_proc->downmix_inst_index_sel = n;
        pstr_drc_uni_sel_proc->uni_drc_sel_proc_output.target_channel_count =
            dwnmix_instructions->target_channel_count;
        pstr_drc_uni_sel_proc->uni_drc_sel_proc_output.target_layout =
            dwnmix_instructions->target_layout;
        if (dwnmix_instructions->downmix_coefficients_present) {
          for (i = 0; i < pstr_drc_uni_sel_proc->uni_drc_sel_proc_output
                              .base_channel_count;
               i++) {
            for (j = 0; j < pstr_drc_uni_sel_proc->uni_drc_sel_proc_output
                                .target_channel_count;
                 j++) {
              pstr_drc_uni_sel_proc->uni_drc_sel_proc_output
                  .downmix_matrix[i][j] =
                  dwnmix_instructions->downmix_coefficient
                      [i +
                       j *
                           pstr_drc_uni_sel_proc->uni_drc_sel_proc_output
                               .base_channel_count];
            }
          }
          pstr_drc_uni_sel_proc->uni_drc_sel_proc_output
              .downmix_matrix_present = 1;
        }
        break;
      }
    }
  }
  return;
}

WORD32 impd_get_selected_eq_set(ia_drc_sel_pro_struct* pstr_drc_uni_sel_proc,
                                WORD32 eq_set_id_selected) {
  WORD32 n;

  pstr_drc_uni_sel_proc->eq_inst_index_sel = -1;

  if (eq_set_id_selected > 0) {
    for (n = 0; n < pstr_drc_uni_sel_proc->drc_config.str_drc_config_ext
                        .eq_instructions_count;
         n++) {
      if (pstr_drc_uni_sel_proc->drc_config.str_drc_config_ext
              .str_eq_instructions[n]
              .eq_set_id == eq_set_id_selected)
        break;
    }
    if (n ==
        pstr_drc_uni_sel_proc->drc_config.str_drc_config_ext
            .eq_instructions_count) {
      return (EXTERNAL_ERROR);
    }
    pstr_drc_uni_sel_proc->eq_inst_index_sel = n;
    if (pstr_drc_uni_sel_proc->drc_config.str_drc_config_ext
            .str_eq_instructions[n]
            .eq_apply_to_downmix == 1) {
      pstr_drc_uni_sel_proc->eq_inst_index[1] =
          pstr_drc_uni_sel_proc->eq_inst_index_sel;
    } else {
      pstr_drc_uni_sel_proc->eq_inst_index[0] =
          pstr_drc_uni_sel_proc->eq_inst_index_sel;
    }
  }
  return (0);
}

WORD32 impd_get_dependent_eq_set(ia_drc_sel_pro_struct* pstr_drc_uni_sel_proc) {
  ia_eq_instructions_struct* str_eq_instructions = NULL;

  if (pstr_drc_uni_sel_proc->eq_inst_index_sel >= 0) {
    str_eq_instructions =
        &(pstr_drc_uni_sel_proc->drc_config.str_drc_config_ext
              .str_eq_instructions[pstr_drc_uni_sel_proc->eq_inst_index_sel]);

    if (str_eq_instructions->depends_on_eq_set_present == 1) {
      WORD32 n;
      WORD32 dependsOnEqSetID = str_eq_instructions->depends_on_eq_set;

      for (n = 0; n < pstr_drc_uni_sel_proc->drc_config.str_drc_config_ext
                          .eq_instructions_count;
           n++) {
        if (pstr_drc_uni_sel_proc->drc_config.str_drc_config_ext
                .str_eq_instructions[n]
                .eq_set_id == dependsOnEqSetID)
          break;
      }
      if (n ==
          pstr_drc_uni_sel_proc->drc_config.str_drc_config_ext
              .eq_instructions_count) {
        return (UNEXPECTED_ERROR);
      }
      if (pstr_drc_uni_sel_proc->drc_config.str_drc_config_ext
              .str_eq_instructions[n]
              .eq_apply_to_downmix == 1) {
        pstr_drc_uni_sel_proc->eq_inst_index[1] = n;
      } else {
        pstr_drc_uni_sel_proc->eq_inst_index[0] = n;
      }
    }
  }
  return (0);
}

WORD32 impd_get_selected_loud_eq_set(
    ia_drc_sel_pro_struct* pstr_drc_uni_sel_proc, WORD32 loudEqSetIdSelected) {
  WORD32 n;

  pstr_drc_uni_sel_proc->loud_eq_inst_index_sel = -1;

  if (loudEqSetIdSelected > 0) {
    for (n = 0; n < pstr_drc_uni_sel_proc->drc_config.str_drc_config_ext
                        .loud_eq_instructions_count;
         n++) {
      if (pstr_drc_uni_sel_proc->drc_config.str_drc_config_ext
              .loud_eq_instructions[n]
              .loud_eq_set_id == loudEqSetIdSelected)
        break;
    }
    if (n ==
        pstr_drc_uni_sel_proc->drc_config.str_drc_config_ext
            .loud_eq_instructions_count) {
      return (EXTERNAL_ERROR);
    }
    pstr_drc_uni_sel_proc->loud_eq_inst_index_sel = n;
  }
  return (0);
}

WORD32 impd_select_loud_eq(ia_drc_config* pstr_drc_config,
                           WORD32 requested_dwnmix_id,
                           WORD32 drc_set_id_requested,
                           WORD32 eq_set_id_requested, WORD32* loud_eq_id_sel) {
  WORD32 i, c, d, e;

  *loud_eq_id_sel = 0;
  for (i = 0;
       i < pstr_drc_config->str_drc_config_ext.loud_eq_instructions_count;
       i++) {
    ia_loud_eq_instructions_struct* loud_eq_instructions =
        &pstr_drc_config->str_drc_config_ext.loud_eq_instructions[i];
    if (loud_eq_instructions->drc_location == LOCATION_SELECTED) {
      for (d = 0; d < loud_eq_instructions->dwnmix_id_count; d++) {
        if ((loud_eq_instructions->downmix_id[d] == requested_dwnmix_id) ||
            (loud_eq_instructions->downmix_id[d] == ID_FOR_ANY_DOWNMIX)) {
          for (c = 0; c < loud_eq_instructions->drc_set_id_count; c++) {
            if ((loud_eq_instructions->drc_set_id[c] == drc_set_id_requested) ||
                (loud_eq_instructions->drc_set_id[c] == ID_FOR_ANY_DRC)) {
              for (e = 0; e < loud_eq_instructions->eq_set_id_count; e++) {
                if ((loud_eq_instructions->eq_set_id[e] ==
                     eq_set_id_requested) ||
                    (loud_eq_instructions->eq_set_id[e] == ID_FOR_ANY_EQ)) {
                  *loud_eq_id_sel = loud_eq_instructions->loud_eq_set_id;
                }
              }
            }
          }
        }
      }
    }
  }
  return (0);
}

WORD32 impd_match_eq_set(ia_drc_config* drc_config, WORD32 downmix_id,
                         WORD32 drc_set_id, WORD32* eq_set_id_valid_flag,
                         WORD32* matching_eq_set_count,
                         WORD32* matching_eq_set_idx) {
  ia_eq_instructions_struct* str_eq_instructions = NULL;
  WORD32 i, k, n;
  *matching_eq_set_count = 0;
  for (i = 0; i < drc_config->str_drc_config_ext.eq_instructions_count; i++) {
    str_eq_instructions =
        &drc_config->str_drc_config_ext.str_eq_instructions[i];

    if (str_eq_instructions->depends_on_eq_set_present == 0) {
      if (str_eq_instructions->no_independent_eq_use == 1) continue;
    }
    if (eq_set_id_valid_flag[str_eq_instructions->eq_set_id] == 0) continue;
    for (k = 0; k < str_eq_instructions->dwnmix_id_count; k++) {
      if ((str_eq_instructions->downmix_id[k] == ID_FOR_ANY_DOWNMIX) ||
          (downmix_id == str_eq_instructions->downmix_id[k])) {
        for (n = 0; n < str_eq_instructions->drc_set_id_count; n++) {
          if ((str_eq_instructions->drc_set_id[n] == ID_FOR_ANY_DRC) ||
              (drc_set_id == str_eq_instructions->drc_set_id[n])) {
            matching_eq_set_idx[*matching_eq_set_count] = i;
            (*matching_eq_set_count)++;
          }
        }
      }
    }
  }
  return (0);
}

WORD32 impd_match_eq_set_purpose(
    ia_drc_config* drc_config, WORD32 eq_set_purpose_requested,
    WORD32* eq_set_id_valid_flag, WORD32* selection_candidate_count,
    ia_selection_candidate_info_struct* selection_candidate_info,
    ia_selection_candidate_info_struct* selection_candidate_info_step_2) {
  WORD32 i, j, k;
  WORD32 match_found_flag;
  WORD32 loop_cnt = 0;
  ia_eq_instructions_struct* str_eq_instructions = NULL;
  match_found_flag = 0;

  k = 0;
  while ((k == 0) && (loop_cnt < 2)) {
    for (j = 0; j < *selection_candidate_count; j++) {
      WORD32 eq_set_id_requested = selection_candidate_info[j].eq_set_id;

      for (i = 0; i < drc_config->str_drc_config_ext.eq_instructions_count;
           i++) {
        str_eq_instructions =
            &drc_config->str_drc_config_ext.str_eq_instructions[i];

        if (str_eq_instructions->depends_on_eq_set_present == 0) {
          if (eq_set_id_valid_flag[str_eq_instructions->eq_set_id] == 0)
            continue;
        }
        if (eq_set_id_valid_flag[str_eq_instructions->eq_set_id] == 0) continue;
        if ((str_eq_instructions->eq_set_id == eq_set_id_requested) &&
            (str_eq_instructions->eq_set_purpose & eq_set_purpose_requested)) {
          match_found_flag = 1;
        }
      }

      if (match_found_flag > 0) {
        memcpy(&selection_candidate_info_step_2[k],
               &selection_candidate_info[j],
               sizeof(ia_selection_candidate_info_struct));
        k++;
      }
    }
    eq_set_purpose_requested = EQ_PURPOSE_DEFAULT;
    loop_cnt++;
  }

  if (k > 0) {
    memcpy(&selection_candidate_info[0], &selection_candidate_info_step_2[0],
           k * sizeof(ia_selection_candidate_info_struct));
    *selection_candidate_count = k;
  }

  return (0);
}

WORD32 impd_find_eq_set_no_compression(ia_drc_config* pstr_drc_config,
                                       WORD32 requested_dwnmix_id,
                                       WORD32* num_compression_eq_count,
                                       WORD32* num_compression_eq_id) {
  WORD32 i, d, k, c;
  k = 0;
  for (i = 0; i < pstr_drc_config->str_drc_config_ext.eq_instructions_count;
       i++) {
    ia_eq_instructions_struct* str_eq_instructions =
        &pstr_drc_config->str_drc_config_ext.str_eq_instructions[i];
    for (d = 0; d < str_eq_instructions->dwnmix_id_count; d++) {
      if (requested_dwnmix_id == str_eq_instructions->downmix_id[d]) {
        for (c = 0; c < str_eq_instructions->drc_set_id_count; c++) {
          if ((str_eq_instructions->drc_set_id[c] == ID_FOR_ANY_DRC) ||
              (str_eq_instructions->drc_set_id[c] == 0)) {
            if (k >= MAX_NUM_COMPRESSION_EQ) return UNEXPECTED_ERROR;
            num_compression_eq_id[k] = str_eq_instructions->eq_set_id;
            k++;
          }
        }
      }
    }
  }
  *num_compression_eq_count = k;
  return (0);
}

VOID impd_select_drc_coeff3(
    ia_drc_config* drc_config,
    ia_uni_drc_coeffs_struct** str_p_loc_drc_coefficients_uni_drc) {
  WORD32 n;
  WORD32 cV1 = -1;
  WORD32 cV0 = -1;
  for (n = 0; n < drc_config->drc_coefficients_drc_count; n++) {
    if (drc_config->str_p_loc_drc_coefficients_uni_drc[n].drc_location == 1) {
      if (drc_config->str_p_loc_drc_coefficients_uni_drc[n].version == 0) {
        cV0 = n;
      } else {
        cV1 = n;
      }
    }
  }
  if (cV1 >= 0) {
    *str_p_loc_drc_coefficients_uni_drc =
        &(drc_config->str_p_loc_drc_coefficients_uni_drc[cV1]);
  } else if (cV0 >= 0) {
    *str_p_loc_drc_coefficients_uni_drc =
        &(drc_config->str_p_loc_drc_coefficients_uni_drc[cV0]);
  } else {
    *str_p_loc_drc_coefficients_uni_drc = NULL;
  }
  return;
}

WORD32 impd_manage_drc_complexity(ia_drc_sel_pro_struct* pstr_drc_uni_sel_proc,
                                  ia_drc_config* pstr_drc_config) {
  WORD32 i, j, err, channel_count;
  WORD32 numBandsTooLarge = 0;
  FLOAT32 complexityDrcPrelim;
  ia_uni_drc_coeffs_struct* str_p_loc_drc_coefficients_uni_drc;
  FLOAT32 complexitySupportedTotal =
      (FLOAT32)(pow(2.0f, pstr_drc_uni_sel_proc->compl_level_supported_total));
  ia_drc_instructions_struct* str_drc_instruction_str;
  ia_drc_instructions_struct* drc_inst_uni_drc_dependent;
  ia_drc_sel_proc_output_struct* uni_drc_sel_proc_output =
      &pstr_drc_uni_sel_proc->uni_drc_sel_proc_output;
  ia_drc_sel_proc_params_struct* pstr_drc_sel_proc_params_struct =
      &pstr_drc_uni_sel_proc->uni_drc_sel_proc_params;

  impd_select_drc_coeff3(pstr_drc_config, &str_p_loc_drc_coefficients_uni_drc);
  if (str_p_loc_drc_coefficients_uni_drc == NULL) return UNEXPECTED_ERROR;
  for (i = 0; i < pstr_drc_config->drc_instructions_uni_drc_count; i++) {
    str_drc_instruction_str = &pstr_drc_config->str_drc_instruction_str[i];
    if (str_drc_instruction_str->no_independent_use) continue;

    numBandsTooLarge = 0;
    if (str_drc_instruction_str->drc_apply_to_dwnmix == 1) {
      channel_count = uni_drc_sel_proc_output->target_channel_count;
    } else {
      channel_count = uni_drc_sel_proc_output->base_channel_count;
    }
    if (pstr_drc_uni_sel_proc->subband_domain_mode == SUBBAND_DOMAIN_MODE_OFF) {
      for (j = 0; j < str_drc_instruction_str->num_drc_ch_groups; j++) {
        ia_gain_set_params_struct* gain_set_params = &(
            str_p_loc_drc_coefficients_uni_drc->gain_set_params
                [str_drc_instruction_str->gain_set_index_for_channel_group[j]]);
        if (gain_set_params->band_count >
            pstr_drc_sel_proc_params_struct->num_bands_supported) {
          numBandsTooLarge = 1;
        } else {
          if (gain_set_params->band_count > 4) {
            /* Add complexity for analysis and synthesis QMF bank here, if
             * supported */
          }
        }
      }
    }
    complexityDrcPrelim =
        (FLOAT32)(channel_count *
                  (1 << str_drc_instruction_str->drc_set_complexity_level));

    if (str_drc_instruction_str->depends_on_drc_set > 0) {
      err = impd_find_drc_instructions_uni_drc(
          pstr_drc_config, str_drc_instruction_str->depends_on_drc_set,
          &drc_inst_uni_drc_dependent);
      if (err) return (err);
      if (drc_inst_uni_drc_dependent->drc_apply_to_dwnmix == 1) {
        channel_count = uni_drc_sel_proc_output->target_channel_count;
      } else {
        channel_count = uni_drc_sel_proc_output->base_channel_count;
      }
      if (pstr_drc_uni_sel_proc->subband_domain_mode ==
          SUBBAND_DOMAIN_MODE_OFF) {
        for (j = 0; j < str_drc_instruction_str->num_drc_ch_groups; j++) {
          ia_gain_set_params_struct* gain_set_params = &(
              str_p_loc_drc_coefficients_uni_drc
                  ->gain_set_params[drc_inst_uni_drc_dependent
                                        ->gain_set_index_for_channel_group[j]]);
          if (gain_set_params->band_count >
              pstr_drc_sel_proc_params_struct->num_bands_supported) {
            numBandsTooLarge = 1;
          } else {
            if (gain_set_params->band_count > 4) {
              /* Add complexity for analysis and synthesis QMF bank here, if
               * supported */
            }
          }
        }
      }
      complexityDrcPrelim +=
          channel_count *
          (1 << drc_inst_uni_drc_dependent->drc_set_complexity_level);
    }

    complexityDrcPrelim *= pstr_drc_config->sampling_rate / 48000.0f;

    if ((complexityDrcPrelim <= complexitySupportedTotal) &&
        (numBandsTooLarge == 0)) {
      pstr_drc_uni_sel_proc
          ->drc_set_id_valid_flag[str_drc_instruction_str->drc_set_id] = 1;
    }
  }
  return (0);
}

WORD32 impd_manage_eq_complexity(ia_drc_sel_pro_struct* pstr_drc_uni_sel_proc,
                                 ia_drc_config* pstr_drc_config) {
  WORD32 k, n, m, err;
  WORD32 eqComplexityPrimary = 0;
  WORD32 eqComplexityDependent = 0;
  WORD32 eqChannelCountPrimary = 0, eqChannelCountDependent = 0;
  FLOAT32 complexityTotalEq;
  ia_drc_config* drc_config = &pstr_drc_uni_sel_proc->drc_config;
  ia_drc_sel_proc_params_struct* pstr_drc_sel_proc_params_struct =
      &pstr_drc_uni_sel_proc->uni_drc_sel_proc_params;
  FLOAT32 complexitySupportedTotal =
      (FLOAT32)(pow(2.0f, pstr_drc_uni_sel_proc->compl_level_supported_total));

  for (n = 0; n < drc_config->str_drc_config_ext.eq_instructions_count; n++) {
    ia_eq_instructions_struct* str_eq_instructions =
        &pstr_drc_config->str_drc_config_ext.str_eq_instructions[n];

    eqChannelCountPrimary = pstr_drc_sel_proc_params_struct->base_channel_count;
    eqChannelCountDependent =
        pstr_drc_sel_proc_params_struct->base_channel_count;

    eqComplexityPrimary = 1 << str_eq_instructions->eq_set_complexity_level;
    if (pstr_drc_uni_sel_proc->subband_domain_mode == SUBBAND_DOMAIN_MODE_OFF) {
      if (str_eq_instructions->td_filter_cascade_present == 0) {
        eqComplexityPrimary = 0;
      }
    } else {
      if (str_eq_instructions->td_filter_cascade_present == 1) {
        eqComplexityPrimary = (WORD32)2.5f;
      }
    }
    if (str_eq_instructions->eq_apply_to_downmix == 1) {
      if (str_eq_instructions->downmix_id[0] == ID_FOR_ANY_DOWNMIX) {
        eqChannelCountPrimary =
            pstr_drc_sel_proc_params_struct->target_ch_count_prelim;
      } else {
        for (k = 0; k < pstr_drc_config->dwnmix_instructions_count; k++) {
          for (m = 0; m < str_eq_instructions->dwnmix_id_count; m++) {
            if (pstr_drc_config->dwnmix_instructions[k].downmix_id ==
                str_eq_instructions->downmix_id[m]) {
              if (eqChannelCountPrimary >
                  pstr_drc_config->dwnmix_instructions[k]
                      .target_channel_count) {
                eqChannelCountPrimary = pstr_drc_config->dwnmix_instructions[k]
                                            .target_channel_count;
              }
            }
          }
        }
      }
    }
    complexityTotalEq = (FLOAT32)(eqChannelCountPrimary * eqComplexityPrimary);

    if (str_eq_instructions->depends_on_eq_set_present > 0) {
      ia_eq_instructions_struct* eq_instructionsDependent;
      err = impd_find_eq_instructions(drc_config,
                                      str_eq_instructions->depends_on_eq_set,
                                      &eq_instructionsDependent);
      if (err) return (err);
      eqComplexityDependent =
          1 << eq_instructionsDependent->eq_set_complexity_level;
      if (pstr_drc_uni_sel_proc->subband_domain_mode ==
          SUBBAND_DOMAIN_MODE_OFF) {
        if (str_eq_instructions->td_filter_cascade_present == 0) {
          eqComplexityDependent = 0;
        }
      } else {
        if (str_eq_instructions->td_filter_cascade_present == 1) {
          eqComplexityDependent = (WORD32)2.5f;
        }
      }
      if (eq_instructionsDependent->eq_apply_to_downmix == 1) {
        if (eq_instructionsDependent->downmix_id[0] == ID_FOR_ANY_DOWNMIX) {
          eqChannelCountDependent =
              pstr_drc_sel_proc_params_struct->target_ch_count_prelim;
        } else {
          for (k = 0; k < pstr_drc_config->dwnmix_instructions_count; k++) {
            for (m = 0; m < str_eq_instructions->dwnmix_id_count; m++) {
              if (pstr_drc_config->dwnmix_instructions[k].downmix_id ==
                  eq_instructionsDependent->downmix_id[m]) {
                if (eqChannelCountDependent >
                    pstr_drc_config->dwnmix_instructions[k]
                        .target_channel_count) {
                  eqChannelCountDependent =
                      pstr_drc_config->dwnmix_instructions[k]
                          .target_channel_count;
                }
              }
            }
          }
        }
      }
      complexityTotalEq += eqChannelCountDependent * eqComplexityDependent;
    }

    pstr_drc_uni_sel_proc
        ->eq_set_id_valid_flag[str_eq_instructions->eq_set_id] = 0;
    complexityTotalEq *= pstr_drc_config->sampling_rate / 48000.0f;

    if (complexityTotalEq <= complexitySupportedTotal) {
      pstr_drc_uni_sel_proc
          ->eq_set_id_valid_flag[str_eq_instructions->eq_set_id] = 1;
    }
  }
  return 0;
}

WORD32 impd_manage_complexity(ia_drc_sel_pro_struct* pstr_drc_uni_sel_proc,
                              ia_drc_config* pstr_drc_config,
                              WORD32* repeat_selection) {
  WORD32 i, j, p, err;
  WORD32 channel_count;
  WORD32 numBandsTooLarge = 0;
  WORD32 drcRequiresEq;
  FLOAT32 complexityEq;
  FLOAT32 complexityDrcTotal = 0.0f;
  FLOAT32 complexityEqTotal = 0.0f;
  FLOAT32 freqNorm = pstr_drc_config->sampling_rate / 48000.0f;
  ia_uni_drc_coeffs_struct* str_p_loc_drc_coefficients_uni_drc;
  ia_drc_instructions_struct* str_drc_instruction_str =
      &pstr_drc_config->str_drc_instruction_str[0];
  ia_drc_sel_proc_output_struct* uni_drc_sel_proc_output =
      &pstr_drc_uni_sel_proc->uni_drc_sel_proc_output;
  ia_drc_sel_proc_params_struct* pstr_drc_sel_proc_params_struct =
      &pstr_drc_uni_sel_proc->uni_drc_sel_proc_params;
  FLOAT32 complexitySupportedTotal =
      (FLOAT32)(pow(2.0f, pstr_drc_uni_sel_proc->compl_level_supported_total));

  impd_select_drc_coeff3(pstr_drc_config, &str_p_loc_drc_coefficients_uni_drc);

  if (str_p_loc_drc_coefficients_uni_drc == NULL) return UNEXPECTED_ERROR;

  for (p = 0; p < 4; p++) {
    if (pstr_drc_uni_sel_proc->uni_drc_sel_proc_output.sel_drc_set_ids[p] <= 0)
      continue;
    err = impd_find_drc_instructions_uni_drc(
        pstr_drc_config,
        pstr_drc_uni_sel_proc->uni_drc_sel_proc_output.sel_drc_set_ids[p],
        &str_drc_instruction_str);
    if (err) return (err);

    if (str_drc_instruction_str->drc_apply_to_dwnmix == 1) {
      channel_count = uni_drc_sel_proc_output->target_channel_count;
    } else {
      channel_count = uni_drc_sel_proc_output->base_channel_count;
    }
    if (pstr_drc_uni_sel_proc->subband_domain_mode == SUBBAND_DOMAIN_MODE_OFF) {
      for (j = 0; j < str_drc_instruction_str->num_drc_ch_groups; j++) {
        ia_gain_set_params_struct* gain_set_params = &(
            str_p_loc_drc_coefficients_uni_drc->gain_set_params
                [str_drc_instruction_str->gain_set_index_for_channel_group[j]]);
        if (gain_set_params->band_count >
            pstr_drc_sel_proc_params_struct->num_bands_supported) {
          if (p < 2) {
            numBandsTooLarge = 1;
          } else {
            pstr_drc_uni_sel_proc
                ->drc_set_id_valid_flag[str_drc_instruction_str->drc_set_id] =
                0;
            pstr_drc_uni_sel_proc->uni_drc_sel_proc_output.sel_drc_set_ids[p] =
                0;
          }
        } else {
          if (gain_set_params->band_count > 4) {
            /* Add complexity for analysis and synthesis QMF bank here, if
             * supported */
          }
        }
      }
    }
    complexityDrcTotal +=
        channel_count *
        (1 << str_drc_instruction_str->drc_set_complexity_level);
  }

  if (uni_drc_sel_proc_output->active_downmix_id > 0) {
    FLOAT32 complexityPerCoeff;
    ia_downmix_instructions_struct* dwnmix_instructions;

    if (pstr_drc_uni_sel_proc->subband_domain_mode == SUBBAND_DOMAIN_MODE_OFF) {
      complexityPerCoeff = 1.0f;
    } else {
      complexityPerCoeff = 2.0f;
    }
    err = impd_find_downmix(pstr_drc_config,
                            uni_drc_sel_proc_output->active_downmix_id,
                            &dwnmix_instructions);
    if (err) return (err);
    if (dwnmix_instructions->downmix_coefficients_present == 1) {
      for (i = 0; i < uni_drc_sel_proc_output->base_channel_count; i++) {
        for (j = 0; j < uni_drc_sel_proc_output->target_channel_count; j++) {
          if (uni_drc_sel_proc_output->downmix_matrix[i][j] != 0.0f) {
            complexityDrcTotal += complexityPerCoeff;
          }
        }
      }
    } else {
      /* add standard downmix here */
    }
  }

  for (p = 0; p < 2; p++) {
    if (pstr_drc_uni_sel_proc->eq_inst_index[p] >= 0) {
      ia_eq_instructions_struct* str_eq_instructions =
          &pstr_drc_config->str_drc_config_ext
               .str_eq_instructions[pstr_drc_uni_sel_proc->eq_inst_index[p]];
      if (p == 0) {
        channel_count = uni_drc_sel_proc_output->base_channel_count;
      } else {
        channel_count = uni_drc_sel_proc_output->target_channel_count;
      }

      complexityEq =
          (FLOAT32)(1 << str_eq_instructions->eq_set_complexity_level);
      if (pstr_drc_uni_sel_proc->subband_domain_mode ==
          SUBBAND_DOMAIN_MODE_OFF) {
        if (str_eq_instructions->td_filter_cascade_present == 0) {
          complexityEq = 0.0;
        }
      } else {
        if (str_eq_instructions->td_filter_cascade_present == 1) {
          complexityEq = 2.5;
        }
      }

      complexityEqTotal += channel_count * complexityEq;
    }
  }

  complexityDrcTotal *= freqNorm;
  complexityEqTotal *= freqNorm;

  if ((complexityDrcTotal > complexitySupportedTotal) ||
      (complexityEqTotal > complexitySupportedTotal))
    return UNEXPECTED_ERROR;

  if (numBandsTooLarge == 1) {
    if (pstr_drc_uni_sel_proc->uni_drc_sel_proc_output.sel_drc_set_ids[0] > 0) {
      err = impd_find_drc_instructions_uni_drc(
          pstr_drc_config,
          pstr_drc_uni_sel_proc->uni_drc_sel_proc_output.sel_drc_set_ids[0],
          &str_drc_instruction_str);
      if (err) return (err);

      pstr_drc_uni_sel_proc
          ->drc_set_id_valid_flag[str_drc_instruction_str->drc_set_id] = 0;
    }
    *repeat_selection = 1;
  } else {
    if (complexityDrcTotal + complexityEqTotal <= complexitySupportedTotal) {
      *repeat_selection = 0;
    } else {
      drcRequiresEq = 0;
      for (p = 0; p < 2; p++) {
        if (pstr_drc_uni_sel_proc->uni_drc_sel_proc_output.sel_drc_set_ids[p] <=
            0)
          continue;
        err = impd_find_drc_instructions_uni_drc(
            pstr_drc_config,
            pstr_drc_uni_sel_proc->uni_drc_sel_proc_output.sel_drc_set_ids[p],
            &str_drc_instruction_str);
        if (err) return (err);
        if (str_drc_instruction_str->requires_eq == 1) {
          drcRequiresEq = 1;
        }
      }
      if ((drcRequiresEq == 0) &&
          (complexityDrcTotal <= complexitySupportedTotal)) {
        for (p = 0; p < 2; p++) {
          pstr_drc_uni_sel_proc->eq_inst_index[p] = 0;
        }
        *repeat_selection = 0;
      } else {
        if (pstr_drc_uni_sel_proc->uni_drc_sel_proc_output.sel_drc_set_ids[0] >
            0) {
          err = impd_find_drc_instructions_uni_drc(
              pstr_drc_config,
              pstr_drc_uni_sel_proc->uni_drc_sel_proc_output.sel_drc_set_ids[0],
              &str_drc_instruction_str);
          if (err) return (err);

          pstr_drc_uni_sel_proc
              ->drc_set_id_valid_flag[str_drc_instruction_str->drc_set_id] = 0;
        } else {
          for (p = 2; p < 4; p++) {
            pstr_drc_uni_sel_proc
                ->drc_set_id_valid_flag[str_drc_instruction_str->drc_set_id] =
                0;
            pstr_drc_uni_sel_proc->uni_drc_sel_proc_output.sel_drc_set_ids[p] =
                0;
          }
        }
        *repeat_selection = 1;
      }
    }
  }

  if (*repeat_selection == 1) {
    memset(&pstr_drc_uni_sel_proc->uni_drc_sel_proc_output, 0,
           sizeof(ia_drc_sel_proc_output_struct));
  }
  return (0);
}

WORD32 impd_find_loud_eq_instructions_idx_for_id(
    ia_drc_config* drc_config, WORD32 loud_eq_set_id_requested,
    WORD32* instructions_idx) {
  WORD32 i;
  if (loud_eq_set_id_requested > 0) {
    for (i = 0; i < drc_config->str_drc_config_ext.loud_eq_instructions_count;
         i++) {
      if (drc_config->str_drc_config_ext.loud_eq_instructions[i]
              .loud_eq_set_id == loud_eq_set_id_requested)
        break;
    }
    if (i == drc_config->str_drc_config_ext.loud_eq_instructions_count) {
      return (UNEXPECTED_ERROR);
    }
    *instructions_idx = i;
  } else {
    *instructions_idx = -1;
  }
  return (0);
}

WORD32 impd_find_eq_instructions(
    ia_drc_config* drc_config, WORD32 eq_set_id_requested,
    ia_eq_instructions_struct** str_eq_instructions) {
  WORD32 i;
  for (i = 0; i < drc_config->str_drc_config_ext.eq_instructions_count; i++) {
    if (eq_set_id_requested ==
        drc_config->str_drc_config_ext.str_eq_instructions[i].eq_set_id)
      break;
  }
  if (i == drc_config->str_drc_config_ext.eq_instructions_count) {
    return (UNEXPECTED_ERROR);
  }
  *str_eq_instructions = &drc_config->str_drc_config_ext.str_eq_instructions[i];
  return (0);
}

WORD32 impd_find_downmix(ia_drc_config* drc_config, WORD32 requested_dwnmix_id,
                         ia_downmix_instructions_struct** dwnmix_instructions) {
  WORD32 i;
  for (i = 0; i < drc_config->dwnmix_instructions_count; i++) {
    if (requested_dwnmix_id == drc_config->dwnmix_instructions[i].downmix_id)
      break;
  }
  if (i == drc_config->dwnmix_instructions_count) {
    return (UNEXPECTED_ERROR);
  }
  *dwnmix_instructions = &drc_config->dwnmix_instructions[i];
  return (0);
}
