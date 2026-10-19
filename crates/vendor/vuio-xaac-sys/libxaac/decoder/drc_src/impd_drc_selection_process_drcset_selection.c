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
#include <string.h>
#include <math.h>
#include "impd_type_def.h"
#include "impd_drc_extr_delta_coded_info.h"
#include "impd_drc_common.h"
#include "impd_drc_struct.h"
#include "impd_drc_interface.h"
#include "impd_drc_selection_process.h"
#include "impd_drc_sel_proc_drc_set_sel.h"
#include "impd_drc_loudness_control.h"
#include "impd_drc_filter_bank.h"
#include "impd_drc_rom.h"
#ifdef LOUDNESS_LEVELING_SUPPORT
#include "ixheaac_error_standards.h"
#endif

static const WORD32 effect_types_request_table[] = {
    EFFECT_BIT_NIGHT,    EFFECT_BIT_NOISY,   EFFECT_BIT_LIMITED,
    EFFECT_BIT_LOWLEVEL, EFFECT_BIT_DIALOG,  EFFECT_BIT_GENERAL_COMPR,
    EFFECT_BIT_EXPAND,   EFFECT_BIT_ARTISTIC};

WORD32 impd_validate_requested_drc_feature(
    ia_drc_sel_proc_params_struct* pstr_drc_sel_proc_params_struct) {
  WORD32 i, j;

  for (i = 0; i < pstr_drc_sel_proc_params_struct->num_drc_feature_requests;
       i++) {
    switch (pstr_drc_sel_proc_params_struct->drc_feature_req_type[i]) {
      case MATCH_EFFECT_TYPE:
        for (j = 0; j < pstr_drc_sel_proc_params_struct
                            ->desired_num_drc_effects_of_requested[i];
             j++) {
          if (pstr_drc_sel_proc_params_struct
                  ->requested_drc_effect_type[i][j] ==
              EFFECT_TYPE_REQUESTED_NONE) {
            if (pstr_drc_sel_proc_params_struct
                    ->desired_num_drc_effects_of_requested[i] > 1) {
              return (UNEXPECTED_ERROR);
            }
          }
        }
        break;
      case MATCH_DYNAMIC_RANGE:
        break;
      case MATCH_DRC_CHARACTERISTIC:
        break;
      default:
        return (UNEXPECTED_ERROR);
        break;
    }
  }
  return (0);
}

WORD32 impd_find_drc_instructions_uni_drc(
    ia_drc_config* drc_config, WORD32 drc_set_id_requested,
    ia_drc_instructions_struct** str_drc_instruction_str) {
  WORD32 i;
  for (i = 0; i < drc_config->drc_instructions_uni_drc_count; i++) {
    if (drc_set_id_requested ==
        drc_config->str_drc_instruction_str[i].drc_set_id)
      break;
  }
  if (i == drc_config->drc_instructions_uni_drc_count) {
    return (UNEXPECTED_ERROR);
  }
  *str_drc_instruction_str = &drc_config->str_drc_instruction_str[i];
  return (0);
}

WORD32 impd_map_requested_effect_bit_idx(WORD32 requested_effect_type,
                                         WORD32* effect_bit_idx) {
  switch (requested_effect_type) {
    case EFFECT_TYPE_REQUESTED_NONE:
      *effect_bit_idx = EFFECT_BIT_NONE;
      break;
    case EFFECT_TYPE_REQUESTED_NIGHT:
      *effect_bit_idx = EFFECT_BIT_NIGHT;
      break;
    case EFFECT_TYPE_REQUESTED_NOISY:
      *effect_bit_idx = EFFECT_BIT_NOISY;
      break;
    case EFFECT_TYPE_REQUESTED_LIMITED:
      *effect_bit_idx = EFFECT_BIT_LIMITED;
      break;
    case EFFECT_TYPE_REQUESTED_LOWLEVEL:
      *effect_bit_idx = EFFECT_BIT_LOWLEVEL;
      break;
    case EFFECT_TYPE_REQUESTED_DIALOG:
      *effect_bit_idx = EFFECT_BIT_DIALOG;
      break;
    case EFFECT_TYPE_REQUESTED_GENERAL_COMPR:
      *effect_bit_idx = EFFECT_BIT_GENERAL_COMPR;
      break;
    case EFFECT_TYPE_REQUESTED_EXPAND:
      *effect_bit_idx = EFFECT_BIT_EXPAND;
      break;
    case EFFECT_TYPE_REQUESTED_ARTISTIC:
      *effect_bit_idx = EFFECT_BIT_ARTISTIC;
      break;

    default:
      return (UNEXPECTED_ERROR);

      break;
  }
  return (0);
}

WORD32 impd_get_fading_drc_set(ia_drc_sel_pro_struct* pstr_drc_uni_sel_proc) {
  pstr_drc_uni_sel_proc->drc_instructions_index[2] = -1;
  if (pstr_drc_uni_sel_proc->uni_drc_sel_proc_params.album_mode == 0) {
    WORD32 n;
    ia_drc_instructions_struct* str_drc_instruction_str = NULL;
    for (n = 0;
         n < pstr_drc_uni_sel_proc->drc_config.drc_instructions_uni_drc_count;
         n++) {
      str_drc_instruction_str =
          &(pstr_drc_uni_sel_proc->drc_config.str_drc_instruction_str[n]);

      if (str_drc_instruction_str->drc_set_effect & EFFECT_BIT_FADE) {
        if (str_drc_instruction_str->downmix_id[0] == ID_FOR_ANY_DOWNMIX) {
          pstr_drc_uni_sel_proc->drc_instructions_index[2] = n;

        } else {
          return (UNEXPECTED_ERROR);
        }
      }
    }
  }
  return (0);
}

WORD32 impd_get_ducking_drc_set(ia_drc_sel_pro_struct* pstr_drc_uni_sel_proc) {
  WORD32 drc_instructions_index;
  WORD32 n, k;
  ia_drc_instructions_struct* str_drc_instruction_str;

  WORD32 requested_dwnmix_id =
      pstr_drc_uni_sel_proc->uni_drc_sel_proc_output.active_downmix_id;

  pstr_drc_uni_sel_proc->drc_instructions_index[3] = -1;
  drc_instructions_index = -1;
  str_drc_instruction_str = NULL;

  for (n = 0;
       n < pstr_drc_uni_sel_proc->drc_config.drc_instructions_uni_drc_count;
       n++) {
    str_drc_instruction_str =
        &(pstr_drc_uni_sel_proc->drc_config.str_drc_instruction_str[n]);

    if (str_drc_instruction_str->drc_set_effect &
        (EFFECT_BIT_DUCK_OTHER | EFFECT_BIT_DUCK_SELF)) {
      for (k = 0; k < str_drc_instruction_str->dwnmix_id_count; k++) {
        if (str_drc_instruction_str->downmix_id[k] == requested_dwnmix_id) {
#ifdef LOUDNESS_LEVELING_SUPPORT
          if (drc_instructions_index != -1) break;
          if (pstr_drc_uni_sel_proc->uni_drc_sel_proc_params.loudness_leveling_on == 0 &&
              str_drc_instruction_str->leveling_present == 1)
            continue;
#endif
          drc_instructions_index = n;
        }
      }
    }
  }
  if (drc_instructions_index == -1) {
    for (n = 0;
         n < pstr_drc_uni_sel_proc->drc_config.drc_instructions_uni_drc_count;
         n++) {
      str_drc_instruction_str =
          &(pstr_drc_uni_sel_proc->drc_config.str_drc_instruction_str[n]);

      if (str_drc_instruction_str->drc_set_effect &
          (EFFECT_BIT_DUCK_OTHER | EFFECT_BIT_DUCK_SELF)) {
        for (k = 0; k < str_drc_instruction_str->dwnmix_id_count; k++) {
          if (str_drc_instruction_str->downmix_id[k] == ID_FOR_BASE_LAYOUT) {
#ifdef LOUDNESS_LEVELING_SUPPORT
            if (drc_instructions_index != -1) break;
            if (pstr_drc_uni_sel_proc->uni_drc_sel_proc_params.loudness_leveling_on == 0 &&
                str_drc_instruction_str->leveling_present == 1)
              continue;
#endif
            drc_instructions_index = n;
          }
        }
      }
    }
  }
  if (drc_instructions_index > -1) {
    pstr_drc_uni_sel_proc->drc_instructions_index[2] = -1;
    pstr_drc_uni_sel_proc->drc_instructions_index[3] = drc_instructions_index;
  }
  return (0);
}

WORD32 impd_get_selected_drc_set(ia_drc_sel_pro_struct* pstr_drc_uni_sel_proc,
                                 WORD32 drc_set_id_selected) {
  WORD32 n;

  for (n = 0; n < pstr_drc_uni_sel_proc->drc_config.drc_instructions_count_plus;
       n++) {
    if (pstr_drc_uni_sel_proc->drc_config.str_drc_instruction_str[n]
            .drc_set_id == drc_set_id_selected)
      break;
  }
  if (n == pstr_drc_uni_sel_proc->drc_config.drc_instructions_count_plus) {
    return (EXTERNAL_ERROR);
  }
  pstr_drc_uni_sel_proc->drc_inst_index_sel = n;

  pstr_drc_uni_sel_proc->drc_instructions_index[0] =
      pstr_drc_uni_sel_proc->drc_inst_index_sel;
  return (0);
}

WORD32 impd_get_dependent_drc_set(
    ia_drc_sel_pro_struct* pstr_drc_uni_sel_proc) {
  ia_drc_instructions_struct* str_drc_instruction_str = NULL;
  str_drc_instruction_str = &(
      pstr_drc_uni_sel_proc->drc_config
          .str_drc_instruction_str[pstr_drc_uni_sel_proc->drc_inst_index_sel]);

  if (str_drc_instruction_str->depends_on_drc_set_present == 1) {
    WORD32 n;
    WORD32 drc_dependent_set_id = str_drc_instruction_str->depends_on_drc_set;

    for (n = 0;
         n < pstr_drc_uni_sel_proc->drc_config.drc_instructions_count_plus;
         n++) {
      if (pstr_drc_uni_sel_proc->drc_config.str_drc_instruction_str[n]
              .drc_set_id == drc_dependent_set_id)
        break;
    }
    if (n == pstr_drc_uni_sel_proc->drc_config.drc_instructions_count_plus) {
      return (UNEXPECTED_ERROR);
    }
    pstr_drc_uni_sel_proc->drc_instructions_index[1] = n;
  } else {
    pstr_drc_uni_sel_proc->drc_instructions_index[1] = -1;
  }
  return (0);
}

WORD32 impd_get_dependent_drc_instructions(
    const ia_drc_config* drc_config,
    const ia_drc_instructions_struct* str_drc_instruction_str,
    ia_drc_instructions_struct** drc_instructions_dependent) {
  WORD32 j;
  ia_drc_instructions_struct* dependent_drc = NULL;
  for (j = 0; j < drc_config->drc_instructions_uni_drc_count; j++) {
    dependent_drc =
        (ia_drc_instructions_struct*)&(drc_config->str_drc_instruction_str[j]);
    if (dependent_drc->drc_set_id ==
        str_drc_instruction_str->depends_on_drc_set) {
      break;
    }
  }
  if (j == drc_config->drc_instructions_uni_drc_count) {
    return (UNEXPECTED_ERROR);
  }
  if (dependent_drc->depends_on_drc_set_present == 1) {
    return (UNEXPECTED_ERROR);
  }
  *drc_instructions_dependent = dependent_drc;
  return (0);
}

WORD32 impd_select_drcs_without_compr_effects(
    ia_drc_config* pstr_drc_config, WORD32* match_found_flag,
    WORD32* selection_candidate_count,
    ia_selection_candidate_info_struct* selection_candidate_info) {
  WORD32 i, k, n;
  WORD32 selection_candidate_step_2_count = 0;
  ia_selection_candidate_info_struct
      selection_candidate_info_step_2[SELECTION_CANDIDATE_COUNT_MAX];
  WORD32 effect_types_request_table_size;
  WORD32 match;
  ia_drc_instructions_struct* str_drc_instruction_str;

  effect_types_request_table_size =
      sizeof(effect_types_request_table) / sizeof(WORD32);

  k = 0;
  for (i = 0; i < *selection_candidate_count; i++) {
    str_drc_instruction_str = &(
        pstr_drc_config->str_drc_instruction_str[selection_candidate_info[i]
                                                     .drc_instructions_index]);

    match = 1;
    for (n = 0; n < effect_types_request_table_size; n++) {
      if ((str_drc_instruction_str->drc_set_effect &
           effect_types_request_table[n]) != 0x0) {
        match = 0;
      }
    }
    if (match == 1) {
      if (k >= SELECTION_CANDIDATE_COUNT_MAX) return UNEXPECTED_ERROR;
      memcpy(&selection_candidate_info_step_2[k], &selection_candidate_info[i],
             sizeof(ia_selection_candidate_info_struct));
      k++;
    }
  }
  selection_candidate_step_2_count = k;

  if (selection_candidate_step_2_count > 0) {
    *match_found_flag = 1;
    for (i = 0; i < selection_candidate_step_2_count; i++) {
      memcpy(&selection_candidate_info[i], &selection_candidate_info_step_2[i],
             sizeof(ia_selection_candidate_info_struct));
      *selection_candidate_count = selection_candidate_step_2_count;
    }
  } else {
    *match_found_flag = 0;
  }

  return (0);
}

WORD32 impd_match_effect_type_attempt(
    ia_drc_config* pstr_drc_config, WORD32 requested_effect_type,
    WORD32 state_requested, WORD32* match_found_flag,
    WORD32* selection_candidate_count,
    ia_selection_candidate_info_struct* selection_candidate_info) {
  WORD32 i, k, err;
  WORD32 selection_candidate_step_2_count = 0;
  ia_selection_candidate_info_struct
      selection_candidate_info_step_2[SELECTION_CANDIDATE_COUNT_MAX];
  ia_drc_instructions_struct* str_drc_instruction_str;
  ia_drc_instructions_struct* drc_instructions_dependent;
  WORD32 effect_bit_idx;

  err =
      impd_map_requested_effect_bit_idx(requested_effect_type, &effect_bit_idx);
  if (err) return (err);

  if (effect_bit_idx == EFFECT_BIT_NONE) {
    err = impd_select_drcs_without_compr_effects(
        pstr_drc_config, match_found_flag, selection_candidate_count,
        selection_candidate_info);
    if (err) return (err);
  } else {
    k = 0;
    for (i = 0; i < *selection_candidate_count; i++) {
      str_drc_instruction_str =
          &(pstr_drc_config->str_drc_instruction_str
                [selection_candidate_info[i].drc_instructions_index]);
      if (str_drc_instruction_str->depends_on_drc_set_present == 1) {
        err = impd_get_dependent_drc_instructions(pstr_drc_config,
                                                  str_drc_instruction_str,
                                                  &drc_instructions_dependent);
        if (err) return (err);

        if (state_requested == 1) {
          if (((str_drc_instruction_str->drc_set_effect & effect_bit_idx) !=
               0x0) ||
              ((drc_instructions_dependent->drc_set_effect & effect_bit_idx) !=
               0x0)) {
            if (k >= SELECTION_CANDIDATE_COUNT_MAX) return UNEXPECTED_ERROR;
            memcpy(&selection_candidate_info_step_2[k],
                   &selection_candidate_info[i],
                   sizeof(ia_selection_candidate_info_struct));
            k++;
          }
        } else {
          if (((str_drc_instruction_str->drc_set_effect & effect_bit_idx) ==
               0x0) &&
              ((drc_instructions_dependent->drc_set_effect & effect_bit_idx) ==
               0x0)) {
            if (k >= SELECTION_CANDIDATE_COUNT_MAX) return UNEXPECTED_ERROR;
            memcpy(&selection_candidate_info_step_2[k],
                   &selection_candidate_info[i],
                   sizeof(ia_selection_candidate_info_struct));
            k++;
          }
        }
      } else {
        if (state_requested == 1) {
          if ((str_drc_instruction_str->drc_set_effect & effect_bit_idx) !=
              0x0) {
            if (k >= SELECTION_CANDIDATE_COUNT_MAX) return UNEXPECTED_ERROR;
            memcpy(&selection_candidate_info_step_2[k],
                   &selection_candidate_info[i],
                   sizeof(ia_selection_candidate_info_struct));
            k++;
          }
        } else {
          if ((str_drc_instruction_str->drc_set_effect & effect_bit_idx) ==
              0x0) {
            if (k >= SELECTION_CANDIDATE_COUNT_MAX) return UNEXPECTED_ERROR;
            memcpy(&selection_candidate_info_step_2[k],
                   &selection_candidate_info[i],
                   sizeof(ia_selection_candidate_info_struct));
            k++;
          }
        }
      }
    }
    selection_candidate_step_2_count = k;

    if (selection_candidate_step_2_count > 0) {
      *match_found_flag = 1;
      for (i = 0; i < selection_candidate_step_2_count; i++) {
        *selection_candidate_count = selection_candidate_step_2_count;
        memcpy(&selection_candidate_info[i],
               &selection_candidate_info_step_2[i],
               sizeof(ia_selection_candidate_info_struct));
      }
    } else {
      *match_found_flag = 0;
    }
  }
  return (0);
}

WORD32 impd_match_effect_types(
    ia_drc_config* pstr_drc_config, WORD32 effect_type_requested_total_count,
    WORD32 effect_type_requested_desired_count, WORD32* requested_effect_type,
    WORD32* selection_candidate_count,
    ia_selection_candidate_info_struct* selection_candidate_info) {
  WORD32 k, err;
  WORD32 match_found_flag = 0;
  WORD32 state_requested;
  WORD32 desired_effect_type_found;

  desired_effect_type_found = 0;
  k = 0;
  while (k < effect_type_requested_desired_count) {
    state_requested = 1;
    err = impd_match_effect_type_attempt(
        pstr_drc_config, requested_effect_type[k], state_requested,
        &match_found_flag, selection_candidate_count, selection_candidate_info);
    if (err) return (err);
    if (match_found_flag) desired_effect_type_found = 1;
    k++;
  }
  if (desired_effect_type_found == 0) {
    while ((k < effect_type_requested_total_count) && (match_found_flag == 0)) {
      state_requested = 1;
      err = impd_match_effect_type_attempt(
          pstr_drc_config, requested_effect_type[k], state_requested,
          &match_found_flag, selection_candidate_count,
          selection_candidate_info);
      if (err) return (err);
      k++;
    }
  }

  return (0);
}

WORD32 impd_match_dynamic_range(
    ia_drc_config* pstr_drc_config,
    ia_drc_loudness_info_set_struct* pstr_loudness_info,
    ia_drc_sel_proc_params_struct* pstr_drc_sel_proc_params_struct,
    WORD32 num_drc_requests, WORD32* selection_candidate_count,
    ia_selection_candidate_info_struct* selection_candidate_info) {
  ia_drc_instructions_struct* str_drc_instruction_str;
  WORD32 err, i, k;
  WORD32 lp_avg_present_val;
  FLOAT32 lp_avg_val;
  FLOAT32 deviation_min = 1000.0f;
  WORD32 selected[DRC_INSTRUCTIONS_COUNT_MAX];
  WORD32 dynamic_range_measurement_type =
      pstr_drc_sel_proc_params_struct
          ->requested_dyn_range_measur_type[num_drc_requests];

  WORD32 requested_dyn_range_range_flag =
      pstr_drc_sel_proc_params_struct
          ->requested_dyn_range_range_flag[num_drc_requests];

  FLOAT32 dynamic_range_requested =
      pstr_drc_sel_proc_params_struct
          ->requested_dyn_range_value[num_drc_requests];

  FLOAT32 dynamic_range_min_requested =
      pstr_drc_sel_proc_params_struct
          ->requested_dyn_range_min_val[num_drc_requests];

  FLOAT32 dynamic_range_max_requested =
      pstr_drc_sel_proc_params_struct
          ->requested_dyn_range_max_val[num_drc_requests];

  WORD32* requested_dwnmix_id =
      pstr_drc_sel_proc_params_struct->requested_dwnmix_id;

  WORD32 album_mode = pstr_drc_sel_proc_params_struct->album_mode;

  k = 0;
  for (i = 0; i < *selection_candidate_count; i++) {
    str_drc_instruction_str = &(
        pstr_drc_config->str_drc_instruction_str[selection_candidate_info[i]
                                                     .drc_instructions_index]);

    err = impd_loudness_peak_to_average_info(
        pstr_loudness_info, str_drc_instruction_str,
        requested_dwnmix_id[selection_candidate_info[i]
                                .downmix_id_request_index],
        dynamic_range_measurement_type, album_mode, &lp_avg_present_val,
        &lp_avg_val);
    if (err) return (err);

    if (lp_avg_present_val == 1) {
      if (requested_dyn_range_range_flag == 1) {
        if ((lp_avg_val >= dynamic_range_min_requested) &&
            (lp_avg_val <= dynamic_range_max_requested)) {
          if (k >= DRC_INSTRUCTIONS_COUNT_MAX) return UNEXPECTED_ERROR;
          selected[k] = i;
          k++;
        }
      } else {
        FLOAT32 deviation =
            (FLOAT32)fabs((FLOAT64)(dynamic_range_requested - lp_avg_val));
        if (deviation_min >= deviation) {
          if (deviation_min > deviation) {
            deviation_min = deviation;
            k = 0;
          }
          if (k >= DRC_INSTRUCTIONS_COUNT_MAX) return UNEXPECTED_ERROR;
          selected[k] = i;
          k++;
        }
      }
    }
  }
  if (k > 0) {
    for (i = 0; i < k; i++) {
      memcpy(&selection_candidate_info[i],
             &selection_candidate_info[selected[i]],
             sizeof(ia_selection_candidate_info_struct));
    }
    *selection_candidate_count = k;
  }

  return (0);
}

WORD32 impd_match_drc_characteristic_attempt(
    ia_drc_config* pstr_drc_config, WORD32 requested_drc_characteristic,
    WORD32* match_found_flag, WORD32* selection_candidate_count,
    ia_selection_candidate_info_struct* selection_candidate_info) {
  WORD32 i, k, n, b, m;
  WORD32 ref_count;
  WORD32 drc_characteristic;
  FLOAT32 match_count;
  WORD32 drc_characteristic_request_1;
  WORD32 drc_characteristic_request_2;
  WORD32 drc_characteristic_request_3;

  ia_drc_instructions_struct* str_drc_instruction_str = NULL;
  ia_uni_drc_coeffs_struct* str_p_loc_drc_coefficients_uni_drc = NULL;
  ia_gain_set_params_struct* gain_set_params = NULL;
  *match_found_flag = 0;

  if (requested_drc_characteristic < 1) {
    return (UNEXPECTED_ERROR);
  }
  if (requested_drc_characteristic < 12) {
    drc_characteristic_request_1 =
        drc_characteristic_order_default[requested_drc_characteristic - 1][0];
    drc_characteristic_request_2 =
        drc_characteristic_order_default[requested_drc_characteristic - 1][1];
    drc_characteristic_request_3 =
        drc_characteristic_order_default[requested_drc_characteristic - 1][2];
  } else {
    drc_characteristic_request_1 = requested_drc_characteristic;
    drc_characteristic_request_2 = -1;
    drc_characteristic_request_3 = -1;
  }

  if (pstr_drc_config->drc_coefficients_drc_count) {
    for (i = 0; i < pstr_drc_config->drc_coefficients_drc_count; i++) {
      str_p_loc_drc_coefficients_uni_drc =
          &(pstr_drc_config->str_p_loc_drc_coefficients_uni_drc[i]);
      if (str_p_loc_drc_coefficients_uni_drc->drc_location == LOCATION_SELECTED)
        break;
    }

    if (i == pstr_drc_config->drc_coefficients_drc_count) {
      return (UNEXPECTED_ERROR);
    }
  }

  n = 0;
  for (i = 0; i < *selection_candidate_count; i++) {
    ref_count = 0;
    match_count = 0;

    str_drc_instruction_str = &(
        pstr_drc_config->str_drc_instruction_str[selection_candidate_info[i]
                                                     .drc_instructions_index]);
    for (k = 0; k < str_drc_instruction_str->num_drc_ch_groups; k++) {
      gain_set_params =
          &(str_p_loc_drc_coefficients_uni_drc->gain_set_params
                [str_drc_instruction_str->gain_set_index_for_channel_group[k]]);
      for (b = 0; b < gain_set_params->band_count; b++) {
        ref_count++;
        drc_characteristic = gain_set_params->gain_params[b].drc_characteristic;
        if (drc_characteristic == drc_characteristic_request_1)
          match_count += 1.0f;
        else if (drc_characteristic == drc_characteristic_request_2)
          match_count += 0.75f;
        else if (drc_characteristic == drc_characteristic_request_3)
          match_count += 0.5f;
      }
    }
    if (str_drc_instruction_str->depends_on_drc_set_present == 1) {
      WORD32 depends_on_drc_set = str_drc_instruction_str->depends_on_drc_set;
      for (m = 0; m < pstr_drc_config->drc_instructions_uni_drc_count; m++) {
        if (pstr_drc_config->str_drc_instruction_str[m].drc_set_id ==
            depends_on_drc_set)
          break;
      }
      if (m == pstr_drc_config->drc_instructions_uni_drc_count) {
        return (UNEXPECTED_ERROR);
      }
      str_drc_instruction_str = &(pstr_drc_config->str_drc_instruction_str[m]);
      if ((str_drc_instruction_str->drc_set_effect &
           (EFFECT_BIT_FADE | EFFECT_BIT_DUCK_OTHER | EFFECT_BIT_DUCK_SELF)) ==
          0) {
        if (str_drc_instruction_str->drc_set_effect != EFFECT_BIT_CLIPPING) {
          for (k = 0; k < str_drc_instruction_str->num_drc_ch_groups; k++) {
            gain_set_params =
                &(str_p_loc_drc_coefficients_uni_drc->gain_set_params
                      [str_drc_instruction_str
                           ->gain_set_index_for_channel_group[k]]);
            for (b = 0; b < gain_set_params->band_count; b++) {
              ref_count++;
              drc_characteristic =
                  gain_set_params->gain_params[b].drc_characteristic;
              if (drc_characteristic == drc_characteristic_request_1)
                match_count += 1.0f;
              else if (drc_characteristic == drc_characteristic_request_2)
                match_count += 0.75f;
              else if (drc_characteristic == drc_characteristic_request_3)
                match_count += 0.5;
            }
          }
        }
      }
    }
    if ((ref_count > 0) && (((FLOAT32)match_count) > 0.5f * ref_count)) {
      if (n >= SELECTION_CANDIDATE_COUNT_MAX) return UNEXPECTED_ERROR;
      memcpy(&selection_candidate_info[n], &selection_candidate_info[i],
             sizeof(ia_selection_candidate_info_struct));
      n++;
    }
  }
  if (n > 0) {
    *selection_candidate_count = n;
    *match_found_flag = 1;
  }

  return (0);
}

WORD32 impd_match_drc_characteristic(
    ia_drc_config* pstr_drc_config, WORD32 requested_drc_characteristic,
    WORD32* selection_candidate_count,
    ia_selection_candidate_info_struct* selection_candidate_info) {
  WORD32 k, err;
  WORD32 match_found_flag = 0;

  const WORD32* drc_characteristic_order =
      drc_characteristic_order_default[requested_drc_characteristic - 1];
  const WORD32 drc_characteristic_order_count =
      sizeof(drc_characteristic_order_default[requested_drc_characteristic]) /
      sizeof(WORD32);
  k = 0;
  while ((k < drc_characteristic_order_count) && (match_found_flag == 0) &&
         (drc_characteristic_order[k] > 0)) {
    err = impd_match_drc_characteristic_attempt(
        pstr_drc_config, drc_characteristic_order[k], &match_found_flag,
        selection_candidate_count, selection_candidate_info);
    if (err) return (err);
    k++;
  }
  return (0);
}

WORD32 impd_drc_set_preselection(
    ia_drc_sel_proc_params_struct* pstr_drc_sel_proc_params_struct,
    ia_drc_config* pstr_drc_config,
    ia_drc_loudness_info_set_struct* pstr_loudness_info,
    WORD32 restrict_to_drc_with_album_loudness,
    ia_drc_sel_pro_struct* pstr_drc_uni_sel_proc,
    WORD32* selection_candidate_count,
    ia_selection_candidate_info_struct* selection_candidate_info) {
  WORD32 i, j, k, l, d, n, err;
  WORD32 downmix_id_match = 0;

  WORD32 selection_candidate_step_2_count;
  ia_selection_candidate_info_struct
      selection_candidate_info_step_2[SELECTION_CANDIDATE_COUNT_MAX];

  WORD32 num_downmix_id_requests =
      pstr_drc_sel_proc_params_struct->num_downmix_id_requests;
  WORD32* requested_dwnmix_id =
      pstr_drc_sel_proc_params_struct->requested_dwnmix_id;
  FLOAT32 output_peak_level_max =
      pstr_drc_sel_proc_params_struct->output_peak_level_max;
  WORD32 loudness_deviation_max =
      pstr_drc_sel_proc_params_struct->loudness_deviation_max;
  WORD32* drc_set_id_valid_flag = pstr_drc_uni_sel_proc->drc_set_id_valid_flag;
  WORD32* eq_set_id_valid_flag = pstr_drc_uni_sel_proc->eq_set_id_valid_flag;

  FLOAT32 output_peak_level_min = 1000.0f;
  FLOAT32 adjustment;
  WORD32 loudness_drc_set_id_requested;

  WORD32 num_compression_eq_count = 0;
  WORD32 num_compression_eq_id[16];

  WORD32 loudness_info_count = 0;
  WORD32 eq_set_id_loudness[16];
  FLOAT32 loudness_normalization_gain_db[16];
  FLOAT32 loudness[16];
  WORD32 peak_info_count;
  WORD32 eq_set_id_Peak[16];
  FLOAT32 signal_peak_level[16];
  WORD32 explicit_peak_information_present[16] = { 0 };

  ia_uni_drc_coeffs_struct* str_p_loc_drc_coefficients_uni_drc = NULL;
  ia_drc_instructions_struct* str_drc_instruction_str = NULL;

  impd_select_drc_coeff3(pstr_drc_config, &str_p_loc_drc_coefficients_uni_drc);
  if (str_p_loc_drc_coefficients_uni_drc == NULL) return UNEXPECTED_ERROR;
  k = 0;
  for (d = 0; d < num_downmix_id_requests; d++) {
    err = impd_find_eq_set_no_compression(
        pstr_drc_config, requested_dwnmix_id[d], &num_compression_eq_count,
        num_compression_eq_id);
    if (err) return (err);
    for (i = 0; i < pstr_drc_config->drc_instructions_count_plus; i++) {
      downmix_id_match = 0;
      str_drc_instruction_str = &(pstr_drc_config->str_drc_instruction_str[i]);

      for (j = 0; j < str_drc_instruction_str->dwnmix_id_count; j++) {
        if ((str_drc_instruction_str->downmix_id[j] ==
             requested_dwnmix_id[d]) ||
            ((str_drc_instruction_str->downmix_id[j] == ID_FOR_BASE_LAYOUT) &&
             (str_drc_instruction_str->drc_set_id > 0)) ||
            (str_drc_instruction_str->downmix_id[j] == ID_FOR_ANY_DOWNMIX)) {
          downmix_id_match = 1;
        }
      }
      if (downmix_id_match == 1) {
        if (pstr_drc_sel_proc_params_struct->dynamic_range_control_on == 1) {
          if ((str_drc_instruction_str->drc_set_effect != EFFECT_BIT_FADE) &&
              (str_drc_instruction_str->drc_set_effect !=
               EFFECT_BIT_DUCK_OTHER) &&
              (str_drc_instruction_str->drc_set_effect !=
               EFFECT_BIT_DUCK_SELF) &&
              (str_drc_instruction_str->drc_set_effect != 0 ||
               str_drc_instruction_str->drc_set_id < 0) &&
              (((str_drc_instruction_str->depends_on_drc_set_present == 0) &&
                (str_drc_instruction_str->no_independent_use == 0)) ||
               (str_drc_instruction_str->depends_on_drc_set_present == 1))) {
            WORD32 drc_is_permitted = 1;
            if (str_drc_instruction_str->drc_set_id > 0) {
              drc_is_permitted =
                  drc_set_id_valid_flag[str_drc_instruction_str->drc_set_id];
            }
            if (drc_is_permitted == 1) {
              err = impd_init_loudness_control(
                  pstr_drc_sel_proc_params_struct, pstr_loudness_info,
                  requested_dwnmix_id[d], str_drc_instruction_str->drc_set_id,

                  num_compression_eq_count, num_compression_eq_id,
                  &loudness_info_count, eq_set_id_loudness,
                  loudness_normalization_gain_db, loudness);
              if (err) return (err);

              if (loudness_info_count > MAX_LOUDNESS_INFO_COUNT)
                return UNEXPECTED_ERROR;

              impd_signal_peak_level_info(
                  pstr_drc_config, pstr_loudness_info, str_drc_instruction_str,
                  requested_dwnmix_id[d],
                  pstr_drc_sel_proc_params_struct->album_mode,
                  num_compression_eq_count, num_compression_eq_id,
                  &peak_info_count, eq_set_id_Peak, signal_peak_level,
                  explicit_peak_information_present);

              for (l = 0; l < loudness_info_count; l++) {
                WORD32 match_found_flag = 0;
                WORD32 p;
                if (k >= SELECTION_CANDIDATE_COUNT_MAX) return UNEXPECTED_ERROR;
                selection_candidate_info[k].loudness_norm_db_gain_adjusted =
                    loudness_normalization_gain_db[l];

                selection_candidate_info[k]
                    .loudness_norm_db_gain_adjusted = min(
                    selection_candidate_info[k].loudness_norm_db_gain_adjusted,
                    pstr_drc_sel_proc_params_struct->loudness_norm_gain_db_max);

                if (loudness[l] != UNDEFINED_LOUDNESS_VALUE) {
                  selection_candidate_info[k].output_loudness =
                      loudness[l] +
                      selection_candidate_info[k]
                          .loudness_norm_db_gain_adjusted;
                } else {
                  selection_candidate_info[k].output_loudness =
                      UNDEFINED_LOUDNESS_VALUE;
                }

                for (p = 0; p < peak_info_count; p++) {
                  if (eq_set_id_Peak[p] == eq_set_id_loudness[l]) {
                    if (eq_set_id_valid_flag[eq_set_id_Peak[p]] == 1)

                    {
                      match_found_flag = 1;
                      break;
                    }
                  }
                }
                if (match_found_flag == 1) {
                  selection_candidate_info[k].output_peak_level =
                      signal_peak_level[p] +
                      selection_candidate_info[k]
                          .loudness_norm_db_gain_adjusted;
                } else {
                  selection_candidate_info[k].output_peak_level =
                      selection_candidate_info[k]
                          .loudness_norm_db_gain_adjusted;
                }
                if ((str_drc_instruction_str->requires_eq == 1) &&
                    (eq_set_id_valid_flag[eq_set_id_loudness[l]] == 0))
                  continue;
                selection_candidate_info[k].drc_instructions_index = i;
                selection_candidate_info[k].downmix_id_request_index = d;
                selection_candidate_info[k].eq_set_id = eq_set_id_loudness[l];
                if (explicit_peak_information_present[p] == 1) {
                  selection_candidate_info[k].selection_flags =
                      SELECTION_FLAG_EXPLICIT_PEAK_INFO_PRESENT;
                } else {
                  selection_candidate_info[k].selection_flags = 0;
                }
                impd_mixing_level_info(
                    pstr_drc_sel_proc_params_struct, pstr_loudness_info,
                    requested_dwnmix_id[d], str_drc_instruction_str->drc_set_id,
                    eq_set_id_loudness[l],
                    &selection_candidate_info[k].mixing_level);
                if (str_drc_instruction_str->drc_set_target_loudness_present &&
                    ((pstr_drc_sel_proc_params_struct
                          ->loudness_normalization_on &&
                      str_drc_instruction_str
                              ->drc_set_target_loudness_value_upper >=
                          pstr_drc_sel_proc_params_struct->target_loudness &&
                      str_drc_instruction_str
                              ->drc_set_target_loudness_value_lower <
                          pstr_drc_sel_proc_params_struct->target_loudness) ||
                     !pstr_drc_sel_proc_params_struct
                          ->loudness_normalization_on)) {
                  selection_candidate_info[k].selection_flags |=
                      SELECTION_FLAG_DRC_TARGET_LOUDNESS_MATCH;
                  if (!explicit_peak_information_present[p]) {
                    if (pstr_drc_sel_proc_params_struct
                            ->loudness_normalization_on) {
                      selection_candidate_info[k].output_peak_level =
                          pstr_drc_sel_proc_params_struct->target_loudness -
                          str_drc_instruction_str
                              ->drc_set_target_loudness_value_upper;
                    } else {
                      selection_candidate_info[k].output_peak_level = 0.0f;
                    }
                  }
                }
                if ((selection_candidate_info[k].selection_flags &
                         (SELECTION_FLAG_DRC_TARGET_LOUDNESS_MATCH |
                          SELECTION_FLAG_EXPLICIT_PEAK_INFO_PRESENT) ||
                     !str_drc_instruction_str
                          ->drc_set_target_loudness_present)) {
                  k++;
                }
              }
            }
          }
        } else {
          if (str_drc_instruction_str->drc_set_id < 0) {
            err = impd_init_loudness_control(
                pstr_drc_sel_proc_params_struct, pstr_loudness_info,
                requested_dwnmix_id[d], str_drc_instruction_str->drc_set_id,
                num_compression_eq_count, num_compression_eq_id,
                &loudness_info_count, eq_set_id_loudness,
                loudness_normalization_gain_db, loudness);
            if (err) return (err);

            impd_signal_peak_level_info(
                pstr_drc_config, pstr_loudness_info, str_drc_instruction_str,
                requested_dwnmix_id[d],
                pstr_drc_sel_proc_params_struct->album_mode,
                num_compression_eq_count, num_compression_eq_id,
                &peak_info_count, eq_set_id_Peak, signal_peak_level,
                explicit_peak_information_present);
            for (l = 0; l < loudness_info_count; l++) {
              WORD32 match_found_flag = 0;
              WORD32 p;
              if (k >= SELECTION_CANDIDATE_COUNT_MAX) return UNEXPECTED_ERROR;
              for (p = 0; p < peak_info_count; p++) {
                if (eq_set_id_Peak[p] == eq_set_id_loudness[l]) {
                  if (eq_set_id_valid_flag[eq_set_id_Peak[p]] == 1) {
                    match_found_flag = 1;
                    break;
                  }
                }
              }
              if (match_found_flag == 1) {
                adjustment = max(
                    0.0f,
                    signal_peak_level[p] + loudness_normalization_gain_db[l] -
                        pstr_drc_sel_proc_params_struct->output_peak_level_max);
                adjustment = min(adjustment, max(0.0f, loudness_deviation_max));
                selection_candidate_info[k].loudness_norm_db_gain_adjusted =
                    loudness_normalization_gain_db[l] - adjustment;

                selection_candidate_info[k]
                    .loudness_norm_db_gain_adjusted = min(
                    selection_candidate_info[k].loudness_norm_db_gain_adjusted,
                    pstr_drc_sel_proc_params_struct->loudness_norm_gain_db_max);

                selection_candidate_info[k].output_peak_level =
                    signal_peak_level[p] +
                    selection_candidate_info[k].loudness_norm_db_gain_adjusted;
                if (loudness[l] != UNDEFINED_LOUDNESS_VALUE) {
                  selection_candidate_info[k].output_loudness =
                      loudness[l] +
                      selection_candidate_info[k]
                          .loudness_norm_db_gain_adjusted;
                } else {
                  selection_candidate_info[k].output_loudness =
                      UNDEFINED_LOUDNESS_VALUE;
                }
                selection_candidate_info[k].drc_instructions_index = i;
                selection_candidate_info[k].downmix_id_request_index = d;
                selection_candidate_info[k].eq_set_id = eq_set_id_loudness[l];
                if (explicit_peak_information_present[p] == 1) {
                  selection_candidate_info[k].selection_flags =
                      SELECTION_FLAG_EXPLICIT_PEAK_INFO_PRESENT;
                } else {
                  selection_candidate_info[k].selection_flags = 0;
                }
                impd_mixing_level_info(
                    pstr_drc_sel_proc_params_struct, pstr_loudness_info,
                    requested_dwnmix_id[d], str_drc_instruction_str->drc_set_id,
                    eq_set_id_loudness[l],
                    &selection_candidate_info[k].mixing_level);
                k++;
              }
            }
          }
        }
      }
    }
  }
  if (k > SELECTION_CANDIDATE_COUNT_MAX) return UNEXPECTED_ERROR;
  *selection_candidate_count = k;
  if (pstr_drc_sel_proc_params_struct->dynamic_range_control_on == 1) {
    n = 0;
    for (k = 0; k < *selection_candidate_count; k++) {
      str_drc_instruction_str =
          &(pstr_drc_config->str_drc_instruction_str
                [selection_candidate_info[k].drc_instructions_index]);

      if (pstr_drc_sel_proc_params_struct->eq_set_purpose_request !=
          EQ_PURPOSE_EQ_OFF) {
        WORD32 matching_eq_set_count = 0;
        WORD32 matching_eq_instrucions_index[64];
        err = impd_match_eq_set(
            pstr_drc_config, requested_dwnmix_id[selection_candidate_info[k]
                                                     .downmix_id_request_index],
            str_drc_instruction_str->drc_set_id, eq_set_id_valid_flag,
            &matching_eq_set_count, matching_eq_instrucions_index);
        if (err) return (err);
        for (j = 0; j < matching_eq_set_count; j++) {
          if (n >= SELECTION_CANDIDATE_COUNT_MAX) return UNEXPECTED_ERROR;
          memcpy(&selection_candidate_info_step_2[n],
                 &selection_candidate_info[k],
                 sizeof(ia_selection_candidate_info_struct));
          selection_candidate_info_step_2[n].eq_set_id =
              pstr_drc_config->str_drc_config_ext
                  .str_eq_instructions[matching_eq_instrucions_index[j]]
                  .eq_set_id;
          n++;
        }
      }
      if (str_drc_instruction_str->requires_eq == 0) {
        if (n >= SELECTION_CANDIDATE_COUNT_MAX) return UNEXPECTED_ERROR;
        memcpy(&selection_candidate_info_step_2[n],
               &selection_candidate_info[k],
               sizeof(ia_selection_candidate_info_struct));
        selection_candidate_info_step_2[n].eq_set_id = 0;
        n++;
      }
    }
    if (n > SELECTION_CANDIDATE_COUNT_MAX) return UNEXPECTED_ERROR;
    memcpy(selection_candidate_info, selection_candidate_info_step_2,
           n * sizeof(ia_selection_candidate_info_struct));
    *selection_candidate_count = n;
    n = 0;
    for (k = 0; k < *selection_candidate_count; k++) {
      if ((selection_candidate_info[k].selection_flags &
           SELECTION_FLAG_DRC_TARGET_LOUDNESS_MATCH) &&
          !(selection_candidate_info[k].selection_flags &
            SELECTION_FLAG_EXPLICIT_PEAK_INFO_PRESENT)) {
        memcpy(&selection_candidate_info_step_2[n],
               &selection_candidate_info[k],
               sizeof(ia_selection_candidate_info_struct));
        n++;
      } else {
        if (selection_candidate_info[k].output_peak_level <=
            output_peak_level_max) {
          memcpy(&selection_candidate_info_step_2[n],
                 &selection_candidate_info[k],
                 sizeof(ia_selection_candidate_info_struct));
          n++;
        }
        if (selection_candidate_info[k].output_peak_level <
            output_peak_level_min) {
          output_peak_level_min = selection_candidate_info[k].output_peak_level;
        }
      }
    }
    selection_candidate_step_2_count = n;
    if (selection_candidate_step_2_count == 0) {
      n = 0;
      for (k = 0; k < *selection_candidate_count; k++) {
        if ((selection_candidate_info[k].selection_flags &
             SELECTION_FLAG_DRC_TARGET_LOUDNESS_MATCH) &&
            (selection_candidate_info[k].selection_flags &
             SELECTION_FLAG_EXPLICIT_PEAK_INFO_PRESENT)) {
          memcpy(&selection_candidate_info_step_2[n],
                 &selection_candidate_info[k],
                 sizeof(ia_selection_candidate_info_struct));
          n++;
        }
      }
      selection_candidate_step_2_count = n;
    }
    if (selection_candidate_step_2_count == 0) {
      n = 0;
      for (k = 0; k < *selection_candidate_count; k++) {
        if (selection_candidate_info_step_2[k].output_peak_level <
            output_peak_level_min + 1.0f) {
          memcpy(&selection_candidate_info_step_2[n],
                 &selection_candidate_info[k],
                 sizeof(ia_selection_candidate_info_struct));
          adjustment =
              max(0.0f, selection_candidate_info_step_2[n].output_peak_level -
                            output_peak_level_max);
          adjustment = min(adjustment, max(0.0f, loudness_deviation_max));
          selection_candidate_info_step_2[n].loudness_norm_db_gain_adjusted -=
              adjustment;
          selection_candidate_info_step_2[n].output_peak_level -= adjustment;
          selection_candidate_info_step_2[n].output_loudness -= adjustment;
          n++;
        }
      }
      selection_candidate_step_2_count = n;
    }

    for (n = 0; n < selection_candidate_step_2_count; n++) {
      memcpy(&selection_candidate_info[n], &selection_candidate_info_step_2[n],
             sizeof(ia_selection_candidate_info_struct));
    }
    *selection_candidate_count = selection_candidate_step_2_count;
  }

  if (restrict_to_drc_with_album_loudness == 1) {
    j = 0;
    for (k = 0; k < *selection_candidate_count; k++) {
      loudness_drc_set_id_requested =
          max(0, pstr_drc_config
                     ->str_drc_instruction_str[selection_candidate_info[k]
                                                   .drc_instructions_index]
                     .drc_set_id);
      for (n = 0; n < pstr_loudness_info->loudness_info_album_count; n++) {
        if (loudness_drc_set_id_requested ==
            pstr_loudness_info->str_loudness_info_album[n].drc_set_id) {
          if (j >= SELECTION_CANDIDATE_COUNT_MAX) return UNEXPECTED_ERROR;
          memcpy(&selection_candidate_info[j], &selection_candidate_info[k],
                 sizeof(ia_selection_candidate_info_struct));
          j++;
          break;
        }
      }
    }
    *selection_candidate_count = j;
  }
  return (0);
}

WORD32 impd_drc_set_final_selection(
    ia_drc_config* pstr_drc_config,
    ia_drc_sel_proc_params_struct* pstr_drc_sel_proc_params_struct,
    WORD32* selection_candidate_count,
    ia_selection_candidate_info_struct* selection_candidate_info,
    WORD32* eq_set_id_valid_flag) {
  WORD32 k, i, n, err;
  WORD32 selection_candidate_step_2_count;
  ia_selection_candidate_info_struct
      selection_candidate_info_step_2[SELECTION_CANDIDATE_COUNT_MAX];
  WORD32 drc_set_id_max;
  FLOAT32 output_level_max;
  FLOAT32 output_level_min;
  WORD32 effect_count, effect_count_min;
  WORD32 effect_types_request_table_size;
  WORD32 drc_set_target_loudness_val_upper_min;
  ia_drc_instructions_struct* str_drc_instruction_str;
  ia_drc_instructions_struct* drc_instructions_dependent;

  if (pstr_drc_sel_proc_params_struct->eq_set_purpose_request > 0) {
    WORD32 eq_purpose_requested =
        pstr_drc_sel_proc_params_struct->eq_set_purpose_request;

    err = impd_match_eq_set_purpose(
        pstr_drc_config, eq_purpose_requested, eq_set_id_valid_flag,
        selection_candidate_count, selection_candidate_info,
        selection_candidate_info_step_2);
    if (err) return (err);
  }

  output_level_min = 10000.0f;
  k = 0;
  for (i = 0; i < *selection_candidate_count; i++) {
    if (output_level_min >= selection_candidate_info[i].output_peak_level) {
      if (output_level_min > selection_candidate_info[i].output_peak_level) {
        output_level_min = selection_candidate_info[i].output_peak_level;
        k = 0;
      }
      memcpy(&selection_candidate_info_step_2[k], &selection_candidate_info[i],
             sizeof(ia_selection_candidate_info_struct));
      k++;
    }
  }
  selection_candidate_step_2_count = k;

  if (output_level_min <= 0.0f) {
    selection_candidate_step_2_count = *selection_candidate_count;
    k = 0;
    for (i = 0; i < selection_candidate_step_2_count; i++) {
      if (selection_candidate_info[i].output_peak_level <= 0.0f) {
        memcpy(&selection_candidate_info_step_2[k],
               &selection_candidate_info[i],
               sizeof(ia_selection_candidate_info_struct));
        k++;
      }
    }
    selection_candidate_step_2_count = k;

    k = 0;
    for (i = 0; i < selection_candidate_step_2_count; i++) {
      str_drc_instruction_str =
          &(pstr_drc_config->str_drc_instruction_str
                [selection_candidate_info_step_2[i].drc_instructions_index]);
      for (n = 0; n < str_drc_instruction_str->dwnmix_id_count; n++) {
        if (pstr_drc_sel_proc_params_struct->requested_dwnmix_id
                [selection_candidate_info_step_2[i].downmix_id_request_index] ==
            str_drc_instruction_str->downmix_id[n]) {
          if (k >= SELECTION_CANDIDATE_COUNT_MAX) return UNEXPECTED_ERROR;
          memcpy(&selection_candidate_info_step_2[k],
                 &selection_candidate_info_step_2[i],
                 sizeof(ia_selection_candidate_info_struct));
          k++;
        }
      }
    }
    if (k > 0) {
      selection_candidate_step_2_count = k;
    }

    effect_types_request_table_size =
        sizeof(effect_types_request_table) / sizeof(WORD32);
    effect_count_min = 100;
    k = 0;
    for (i = 0; i < selection_candidate_step_2_count; i++) {
      str_drc_instruction_str =
          &(pstr_drc_config->str_drc_instruction_str
                [selection_candidate_info_step_2[i].drc_instructions_index]);
      effect_count = 0;
      if (str_drc_instruction_str->depends_on_drc_set_present == 1) {
        err = impd_get_dependent_drc_instructions(pstr_drc_config,
                                                  str_drc_instruction_str,
                                                  &drc_instructions_dependent);
        if (err) return (err);

        for (n = 0; n < effect_types_request_table_size; n++) {
          if (effect_types_request_table[n] != EFFECT_BIT_GENERAL_COMPR) {
            if (((str_drc_instruction_str->drc_set_effect &
                  effect_types_request_table[n]) != 0x0) ||
                ((drc_instructions_dependent->drc_set_effect &
                  effect_types_request_table[n]) != 0x0)) {
              effect_count++;
            }
          }
        }
      } else {
        for (n = 0; n < effect_types_request_table_size; n++) {
          if (effect_types_request_table[n] != EFFECT_BIT_GENERAL_COMPR) {
            if ((str_drc_instruction_str->drc_set_effect &
                 effect_types_request_table[n]) != 0x0) {
              effect_count++;
            }
          }
        }
      }
      if (effect_count_min >= effect_count) {
        if (effect_count_min > effect_count) {
          effect_count_min = effect_count;
          k = 0;
        }
        memcpy(&selection_candidate_info_step_2[k],
               &selection_candidate_info_step_2[i],
               sizeof(ia_selection_candidate_info_struct));
        k++;
      }
    }
    selection_candidate_step_2_count = k;

    drc_set_target_loudness_val_upper_min = 100;
    k = 0;
    for (i = 0; i < selection_candidate_step_2_count; i++) {
      if (selection_candidate_info_step_2[i].selection_flags &
          SELECTION_FLAG_DRC_TARGET_LOUDNESS_MATCH) {
        k++;
      }
    }
    if (k != 0 && k != selection_candidate_step_2_count) {
      k = 0;
      for (i = 0; i < selection_candidate_step_2_count; i++) {
        if (!(selection_candidate_info_step_2[i].selection_flags &
              SELECTION_FLAG_DRC_TARGET_LOUDNESS_MATCH)) {
          memcpy(&selection_candidate_info_step_2[k],
                 &selection_candidate_info_step_2[i],
                 sizeof(ia_selection_candidate_info_struct));
          k++;
        }
      }
      selection_candidate_step_2_count = k;
    } else if (k == selection_candidate_step_2_count) {
      k = 0;
      for (i = 0; i < selection_candidate_step_2_count; i++) {
        str_drc_instruction_str =
            &(pstr_drc_config->str_drc_instruction_str
                  [selection_candidate_info_step_2[i].drc_instructions_index]);
        if (str_drc_instruction_str->drc_set_target_loudness_present != 1) {
          return UNEXPECTED_ERROR;
        }
        if (drc_set_target_loudness_val_upper_min >=
            str_drc_instruction_str->drc_set_target_loudness_value_upper) {
          if (drc_set_target_loudness_val_upper_min >
              str_drc_instruction_str->drc_set_target_loudness_value_upper) {
            drc_set_target_loudness_val_upper_min =
                str_drc_instruction_str->drc_set_target_loudness_value_upper;
            k = 0;
          }
          memcpy(&selection_candidate_info_step_2[k],
                 &selection_candidate_info_step_2[i],
                 sizeof(ia_selection_candidate_info_struct));
          k++;
        }
      }
      selection_candidate_step_2_count = k;
    }

    k = 0;
    for (i = 0; i < selection_candidate_step_2_count; i++) {
      str_drc_instruction_str =
          &(pstr_drc_config->str_drc_instruction_str
                [selection_candidate_info_step_2[i].drc_instructions_index]);
      if (str_drc_instruction_str->drc_set_target_loudness_present &&
          pstr_drc_sel_proc_params_struct->loudness_normalization_on &&
          str_drc_instruction_str->drc_set_target_loudness_value_upper >=
              pstr_drc_sel_proc_params_struct->target_loudness &&
          str_drc_instruction_str->drc_set_target_loudness_value_lower <
              pstr_drc_sel_proc_params_struct->target_loudness) {
        k++;
      }
    }
    if (k != 0 && k != selection_candidate_step_2_count) {
      k = 0;
      for (i = 0; i < selection_candidate_step_2_count; i++) {
        str_drc_instruction_str =
            &(pstr_drc_config->str_drc_instruction_str
                  [selection_candidate_info_step_2[i].drc_instructions_index]);
        if (str_drc_instruction_str->drc_set_target_loudness_present &&
            pstr_drc_sel_proc_params_struct->loudness_normalization_on &&
            str_drc_instruction_str->drc_set_target_loudness_value_upper >=
                pstr_drc_sel_proc_params_struct->target_loudness &&
            str_drc_instruction_str->drc_set_target_loudness_value_lower <
                pstr_drc_sel_proc_params_struct->target_loudness) {
          memcpy(&selection_candidate_info_step_2[k],
                 &selection_candidate_info_step_2[i],
                 sizeof(ia_selection_candidate_info_struct));
          k++;
        }
      }
      selection_candidate_step_2_count = k;
      drc_set_target_loudness_val_upper_min = 100;
      k = 0;
      for (i = 0; i < selection_candidate_step_2_count; i++) {
        str_drc_instruction_str =
            &(pstr_drc_config->str_drc_instruction_str
                  [selection_candidate_info_step_2[i].drc_instructions_index]);
        if (str_drc_instruction_str->drc_set_target_loudness_present != 1) {
          return UNEXPECTED_ERROR;
        }
        if (drc_set_target_loudness_val_upper_min >=
            str_drc_instruction_str->drc_set_target_loudness_value_upper) {
          if (drc_set_target_loudness_val_upper_min >
              str_drc_instruction_str->drc_set_target_loudness_value_upper) {
            drc_set_target_loudness_val_upper_min =
                str_drc_instruction_str->drc_set_target_loudness_value_upper;
            k = 0;
          }
          memcpy(&selection_candidate_info_step_2[k],
                 &selection_candidate_info_step_2[i],
                 sizeof(ia_selection_candidate_info_struct));
          k++;
        }
      }
      selection_candidate_step_2_count = k;
    } else if (k == selection_candidate_step_2_count) {
      drc_set_target_loudness_val_upper_min = 100;
      k = 0;
      for (i = 0; i < selection_candidate_step_2_count; i++) {
        str_drc_instruction_str =
            &(pstr_drc_config->str_drc_instruction_str
                  [selection_candidate_info_step_2[i].drc_instructions_index]);
        if (str_drc_instruction_str->drc_set_target_loudness_present != 1) {
          return UNEXPECTED_ERROR;
        }
        if (drc_set_target_loudness_val_upper_min >=
            str_drc_instruction_str->drc_set_target_loudness_value_upper) {
          if (drc_set_target_loudness_val_upper_min >
              str_drc_instruction_str->drc_set_target_loudness_value_upper) {
            drc_set_target_loudness_val_upper_min =
                str_drc_instruction_str->drc_set_target_loudness_value_upper;
            k = 0;
          }
          memcpy(&selection_candidate_info_step_2[k],
                 &selection_candidate_info_step_2[i],
                 sizeof(ia_selection_candidate_info_struct));
          k++;
        }
      }
      selection_candidate_step_2_count = k;
    }
    k = 0;
    output_level_max = -1000.0f;
    for (i = 0; i < selection_candidate_step_2_count; i++) {
      if ((selection_candidate_info_step_2[i].output_peak_level <= 0.0f) &&
          (output_level_max <=
           selection_candidate_info_step_2[i].output_peak_level)) {
        if (output_level_max <
            selection_candidate_info_step_2[i].output_peak_level) {
          output_level_max =
              selection_candidate_info_step_2[i].output_peak_level;
          k = 0;
        }
        memcpy(&selection_candidate_info_step_2[k],
               &selection_candidate_info_step_2[i],
               sizeof(ia_selection_candidate_info_struct));
        k++;
        output_level_max = selection_candidate_info_step_2[i].output_peak_level;
      }
    }
    selection_candidate_step_2_count = k;
  }

  drc_set_id_max = -1000;
  for (i = 0; i < selection_candidate_step_2_count; i++) {
    str_drc_instruction_str =
        &(pstr_drc_config->str_drc_instruction_str
              [selection_candidate_info_step_2[i].drc_instructions_index]);
    if (drc_set_id_max < str_drc_instruction_str->drc_set_id) {
      drc_set_id_max = str_drc_instruction_str->drc_set_id;
      memcpy(&selection_candidate_info_step_2[0],
             &selection_candidate_info_step_2[i],
             sizeof(ia_selection_candidate_info_struct));
    }
  }
  memcpy(&selection_candidate_info[0], &selection_candidate_info_step_2[0],
         sizeof(ia_selection_candidate_info_struct));
  *selection_candidate_count = 1;

  return 0;
}

WORD32 impd_select_drc_set(ia_drc_sel_pro_struct* pstr_drc_uni_sel_proc,
                           WORD32* drc_set_id_selected,
                           WORD32* eq_set_id_selected, WORD32* loud_eq_id_sel) {
  WORD32 i, err;

  ia_drc_sel_proc_params_struct* pstr_drc_sel_proc_params_struct =
      &pstr_drc_uni_sel_proc->uni_drc_sel_proc_params;
  ia_drc_config* pstr_drc_config = &pstr_drc_uni_sel_proc->drc_config;
  ia_drc_loudness_info_set_struct* pstr_loudness_info =
      &pstr_drc_uni_sel_proc->loudness_info_set;

  WORD32 selection_candidate_count = 0;
  WORD32 restrict_to_drc_with_album_loudness = 0;
  ia_selection_candidate_info_struct
      selection_candidate_info[SELECTION_CANDIDATE_COUNT_MAX];

  //    WORD32 selected_eq_set_count = 0;

  if (pstr_drc_sel_proc_params_struct->album_mode == 1) {
    restrict_to_drc_with_album_loudness = 1;
  }

  while (!selection_candidate_count) {
    err = impd_drc_set_preselection(
        pstr_drc_sel_proc_params_struct, pstr_drc_config, pstr_loudness_info,
        restrict_to_drc_with_album_loudness, pstr_drc_uni_sel_proc,
        &selection_candidate_count, selection_candidate_info);
    if (err) return err;

    if (selection_candidate_count == 0) {
      if (restrict_to_drc_with_album_loudness == 1) {
        restrict_to_drc_with_album_loudness = 0;
        continue;
      } else {
        return (UNEXPECTED_ERROR);
      }
    }

    err = impd_validate_requested_drc_feature(pstr_drc_sel_proc_params_struct);
    if (err) return (err);

    if (pstr_drc_sel_proc_params_struct->dynamic_range_control_on == 1) {
      if (pstr_drc_sel_proc_params_struct->num_drc_feature_requests > 0) {
        for (i = 0;
             i < pstr_drc_sel_proc_params_struct->num_drc_feature_requests;
             i++) {
          switch (pstr_drc_sel_proc_params_struct->drc_feature_req_type[i]) {
            case MATCH_EFFECT_TYPE:
              err = impd_match_effect_types(
                  pstr_drc_config,
                  pstr_drc_sel_proc_params_struct->requested_num_drc_effects[i],
                  pstr_drc_sel_proc_params_struct
                      ->desired_num_drc_effects_of_requested[i],
                  pstr_drc_sel_proc_params_struct->requested_drc_effect_type[i],
                  &selection_candidate_count, selection_candidate_info);
              if (err) return (err);
              break;
            case MATCH_DYNAMIC_RANGE:
              err = impd_match_dynamic_range(
                  pstr_drc_config, pstr_loudness_info,
                  pstr_drc_sel_proc_params_struct, i,
                  &selection_candidate_count, selection_candidate_info);
              if (err) return (err);
              break;
            case MATCH_DRC_CHARACTERISTIC:
              err = impd_match_drc_characteristic(
                  pstr_drc_config, pstr_drc_sel_proc_params_struct
                                       ->requested_drc_characteristic[i],
                  &selection_candidate_count, selection_candidate_info);
              if (err) return (err);
              break;

            default:
              return (UNEXPECTED_ERROR);
              break;
          }
        }
      } else {
        WORD32 match_found_flag = 0;

        err = impd_select_drcs_without_compr_effects(
            pstr_drc_config, &match_found_flag, &selection_candidate_count,
            selection_candidate_info);
        if (err) return (err);

        if (match_found_flag == 0) {
          WORD32 requested_num_drc_effects = 5;
          WORD32 desired_num_drc_effects_of_requested = 1;
          WORD32 requested_drc_effect_type[5] = {
              EFFECT_TYPE_REQUESTED_GENERAL_COMPR, EFFECT_TYPE_REQUESTED_NIGHT,
              EFFECT_TYPE_REQUESTED_NOISY, EFFECT_TYPE_REQUESTED_LIMITED,
              EFFECT_TYPE_REQUESTED_LOWLEVEL};

          err = impd_match_effect_types(
              pstr_drc_config, requested_num_drc_effects,
              desired_num_drc_effects_of_requested, requested_drc_effect_type,
              &selection_candidate_count, selection_candidate_info);
          if (err) return (err);
        }
      }

      if (selection_candidate_count > 0) {
        err = impd_drc_set_final_selection(
            pstr_drc_config, pstr_drc_sel_proc_params_struct,
            &selection_candidate_count, selection_candidate_info,
            pstr_drc_uni_sel_proc->eq_set_id_valid_flag);
        if (err) return (err);
      } else {
        selection_candidate_count = 0;
        return (UNEXPECTED_ERROR);
      }
    }

    if (selection_candidate_count == 0) {
      if (restrict_to_drc_with_album_loudness == 1) {
        restrict_to_drc_with_album_loudness = 0;
      } else {
        return (UNEXPECTED_ERROR);
      }
    }
  }
  *drc_set_id_selected =
      pstr_drc_config
          ->str_drc_instruction_str[selection_candidate_info[0]
                                        .drc_instructions_index]
          .drc_set_id;
  *eq_set_id_selected = selection_candidate_info[0].eq_set_id;

  err = impd_select_loud_eq(
      pstr_drc_config,
      pstr_drc_sel_proc_params_struct->requested_dwnmix_id
          [selection_candidate_info[0].downmix_id_request_index],
      *drc_set_id_selected, *eq_set_id_selected, loud_eq_id_sel);
  if (err) return (err);
  if (selection_candidate_count > 0) {
    pstr_drc_uni_sel_proc->uni_drc_sel_proc_output
        .loudness_normalization_gain_db =
        selection_candidate_info[0].loudness_norm_db_gain_adjusted;
    pstr_drc_uni_sel_proc->uni_drc_sel_proc_output.output_peak_level_db =
        selection_candidate_info[0].output_peak_level;
    pstr_drc_uni_sel_proc->uni_drc_sel_proc_output.output_loudness =
        selection_candidate_info[0].output_loudness;
    pstr_drc_uni_sel_proc->uni_drc_sel_proc_output.active_downmix_id =
        pstr_drc_sel_proc_params_struct->requested_dwnmix_id
            [selection_candidate_info[0].downmix_id_request_index];
    pstr_drc_uni_sel_proc->uni_drc_sel_proc_output.mixing_level =
        selection_candidate_info[0].mixing_level;
  }
  return (0);
}
