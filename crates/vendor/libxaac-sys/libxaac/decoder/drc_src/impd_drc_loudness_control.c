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
#include <math.h>

#include "impd_type_def.h"
#include "impd_drc_extr_delta_coded_info.h"
#include "impd_drc_common.h"
#include "impd_drc_struct.h"
#include "impd_drc_interface.h"
#include "impd_drc_selection_process.h"
#include "impd_drc_filter_bank.h"
#include "impd_drc_rom.h"
VOID impd_signal_peak_level_info(
    ia_drc_config* pstr_drc_config,
    ia_drc_loudness_info_set_struct* pstr_loudness_info,
    ia_drc_instructions_struct* str_drc_instruction_str,
    WORD32 requested_dwnmix_id, WORD32 album_mode,
    WORD32 num_compression_eq_count, WORD32* num_compression_eq_id,
    WORD32* peak_info_count, WORD32 eq_set_id[], FLOAT32 signal_peak_level[],
    WORD32 explicit_peak_information_present[]) {
  WORD32 c, d, i, k, n, base_channel_count, mode;
  WORD32 pre_lim_count;
  WORD32 peak_count = 0;
  WORD32 loudness_info_count = 0;
  ia_loudness_info_struct* loudness_info;
  FLOAT32 sum, max_sum;
  WORD32 drc_set_id_requested = str_drc_instruction_str->drc_set_id;
  WORD32 loudness_drc_set_id_requested;
  WORD32 match_found_flag = 0;

  FLOAT32 signal_peak_level_tmp;
  eq_set_id[0] = 0;
  signal_peak_level[0] = 0.0f;
  explicit_peak_information_present[0] = 0;

  k = 0;
  if (drc_set_id_requested < 0) {
    for (k = 0; k < num_compression_eq_count; k++) {
      eq_set_id[k] = num_compression_eq_id[k];
      signal_peak_level[k] = 0.0f;
      explicit_peak_information_present[k] = 0;
    }
  }
  eq_set_id[k] = 0;
  signal_peak_level[k] = 0.0f;
  explicit_peak_information_present[k] = 0;
  k++;

  pre_lim_count = k;

  if (drc_set_id_requested < 0) {
    loudness_drc_set_id_requested = 0;
  } else {
    loudness_drc_set_id_requested = drc_set_id_requested;
  }

  if (album_mode == 1) {
    mode = 1;
    loudness_info_count = pstr_loudness_info->loudness_info_album_count;
  } else {
    mode = 0;
    loudness_info_count = pstr_loudness_info->loudness_info_count;
  }

  for (n = 0; n < loudness_info_count; n++) {
    if (mode == 1) {
      loudness_info = &(pstr_loudness_info->str_loudness_info_album[n]);
    } else {
      loudness_info = &(pstr_loudness_info->loudness_info[n]);
    }
    if (loudness_drc_set_id_requested == loudness_info->drc_set_id &&
        requested_dwnmix_id == loudness_info->downmix_id) {
      if (loudness_info->true_peak_level_present) {
        eq_set_id[peak_count] = loudness_info->eq_set_id;

        signal_peak_level[peak_count] = loudness_info->true_peak_level;
        explicit_peak_information_present[peak_count] = 1;

        match_found_flag = 1;
        peak_count++;
      }
      if (match_found_flag == 0) {
        if (loudness_info->sample_peak_level_present) {
          eq_set_id[peak_count] = loudness_info->eq_set_id;

          signal_peak_level[peak_count] = loudness_info->sample_peak_level;
          explicit_peak_information_present[peak_count] = 1;

          match_found_flag = 1;
          peak_count++;
        }
      }
    }
  }
  if (match_found_flag == 0) {
    for (n = 0; n < loudness_info_count; n++) {
      if (mode == 1) {
        loudness_info = &(pstr_loudness_info->str_loudness_info_album[n]);
      } else {
        loudness_info = &(pstr_loudness_info->loudness_info[n]);
      }
      if (ID_FOR_ANY_DRC == loudness_info->drc_set_id &&
          requested_dwnmix_id == loudness_info->downmix_id) {
        if (loudness_info->true_peak_level_present) {
          eq_set_id[peak_count] = loudness_info->eq_set_id;

          signal_peak_level[peak_count] = loudness_info->true_peak_level;
          explicit_peak_information_present[peak_count] = 1;

          match_found_flag = 1;
          peak_count++;
        }
        if (match_found_flag == 0) {
          if (loudness_info->sample_peak_level_present) {
            eq_set_id[peak_count] = loudness_info->eq_set_id;

            signal_peak_level[peak_count] = loudness_info->sample_peak_level;
            explicit_peak_information_present[peak_count] = 1;

            match_found_flag = 1;
            peak_count++;
          }
        }
      }
    }
  }
  if (match_found_flag == 0) {
    for (i = 0; i < str_drc_instruction_str->dwnmix_id_count; i++) {
      if (requested_dwnmix_id == str_drc_instruction_str->downmix_id[0] ||
          ID_FOR_ANY_DOWNMIX == str_drc_instruction_str->downmix_id[0]) {
        if (str_drc_instruction_str->limiter_peak_target_present) {
          if (str_drc_instruction_str->requires_eq == 1) {
            for (d = 0;
                 d < pstr_drc_config->str_drc_config_ext.eq_instructions_count;
                 d++) {
              ia_eq_instructions_struct* eq_instructions =
                  &pstr_drc_config->str_drc_config_ext.str_eq_instructions[d];
              for (c = 0; c < eq_instructions->drc_set_id_count; c++) {
                if ((eq_instructions->drc_set_id[c] ==
                     loudness_drc_set_id_requested) ||
                    (eq_instructions->drc_set_id[c] == ID_FOR_ANY_DRC)) {
                  for (i = 0; i < eq_instructions->dwnmix_id_count; i++) {
                    if ((eq_instructions->downmix_id[i] ==
                         requested_dwnmix_id) ||
                        (eq_instructions->downmix_id[i] ==
                         ID_FOR_ANY_DOWNMIX)) {
                      eq_set_id[peak_count] = eq_instructions->eq_set_id;
                      signal_peak_level[peak_count] =
                          str_drc_instruction_str->limiter_peak_target;
                      explicit_peak_information_present[peak_count] = 1;
                      match_found_flag = 1;
                      peak_count++;
                    }
                  }
                }
              }
            }
          } else

          {
            eq_set_id[peak_count] = 0;
            signal_peak_level[peak_count] =
                str_drc_instruction_str->limiter_peak_target;
            explicit_peak_information_present[peak_count] = 1;
            match_found_flag = 1;
            peak_count++;
          }
        }
      }
    }
  }
  if (match_found_flag == 0) {
    for (i = 1; i < str_drc_instruction_str->dwnmix_id_count; i++) {
      if (requested_dwnmix_id == str_drc_instruction_str->downmix_id[i]) {
        if (str_drc_instruction_str->limiter_peak_target_present) {
          {
            eq_set_id[peak_count] = 0;
            signal_peak_level[peak_count] =
                str_drc_instruction_str->limiter_peak_target;
            explicit_peak_information_present[peak_count] = 1;
            match_found_flag = 1;
            peak_count++;
          }
        }
      }
    }
  }
  if (match_found_flag == 0) {
    if (requested_dwnmix_id != ID_FOR_BASE_LAYOUT) {
      signal_peak_level_tmp = 0.f;
      for (i = 0; i < pstr_drc_config->dwnmix_instructions_count; i++) {
        if (pstr_drc_config->dwnmix_instructions[i].downmix_id ==
            requested_dwnmix_id) {
          if (pstr_drc_config->dwnmix_instructions[i]
                  .downmix_coefficients_present) {
            base_channel_count =
                pstr_drc_config->channel_layout.base_channel_count;
            max_sum = 0.0f;
            for (c = 0;
                 c <
                 pstr_drc_config->dwnmix_instructions[i].target_channel_count;
                 c++) {
              sum = 0.0f;
              for (d = 0; d < base_channel_count; d++) {
                sum += pstr_drc_config->dwnmix_instructions[i]
                           .downmix_coefficient[c * base_channel_count + d];
              }
              if (max_sum < sum) max_sum = sum;
            }
            signal_peak_level_tmp = 20.0f * (FLOAT32)log10(max_sum);
          } else {
          }
          break;
        }
      }
      for (n = 0; n < loudness_info_count; n++) {
        if (mode == 1) {
          loudness_info = &(pstr_loudness_info->str_loudness_info_album[n]);
        } else {
          loudness_info = &(pstr_loudness_info->loudness_info[n]);
        }
        if (loudness_drc_set_id_requested == loudness_info->drc_set_id &&
            ID_FOR_BASE_LAYOUT == loudness_info->downmix_id) {
          if (loudness_info->true_peak_level_present) {
            eq_set_id[peak_count] = loudness_info->eq_set_id;

            signal_peak_level[peak_count] =
                loudness_info->true_peak_level + signal_peak_level_tmp;
            explicit_peak_information_present[peak_count] = 0;

            match_found_flag = 1;
            peak_count++;
          }
          if (match_found_flag == 0) {
            if (loudness_info->sample_peak_level_present) {
              eq_set_id[peak_count] = loudness_info->eq_set_id;

              signal_peak_level[peak_count] =
                  loudness_info->sample_peak_level + signal_peak_level_tmp;
              explicit_peak_information_present[peak_count] = 0;

              match_found_flag = 1;
              peak_count++;
            }
          }
        }
      }
      if (match_found_flag == 0) {
        for (n = 0; n < loudness_info_count; n++) {
          if (mode == 1) {
            loudness_info = &(pstr_loudness_info->str_loudness_info_album[n]);
          } else {
            loudness_info = &(pstr_loudness_info->loudness_info[n]);
          }
          if (ID_FOR_ANY_DRC == loudness_info->drc_set_id &&
              ID_FOR_BASE_LAYOUT == loudness_info->downmix_id) {
            if (loudness_info->true_peak_level_present) {
              eq_set_id[peak_count] = loudness_info->eq_set_id;

              signal_peak_level[peak_count] =
                  loudness_info->true_peak_level + signal_peak_level_tmp;
              explicit_peak_information_present[peak_count] = 0;

              match_found_flag = 1;
              peak_count++;
            }
            if (match_found_flag == 0) {
              if (loudness_info->sample_peak_level_present) {
                eq_set_id[peak_count] = loudness_info->eq_set_id;

                signal_peak_level[peak_count] =
                    loudness_info->sample_peak_level + signal_peak_level_tmp;
                explicit_peak_information_present[peak_count] = 0;

                match_found_flag = 1;
                peak_count++;
              }
            }
          }
        }
      }
      if (match_found_flag == 0) {
        ia_drc_instructions_struct* drc_instructions_drc_tmp;
        for (n = 0; n < pstr_drc_config->drc_instructions_count_plus; n++) {
          drc_instructions_drc_tmp =
              &pstr_drc_config->str_drc_instruction_str[n];
          if (loudness_drc_set_id_requested ==
              drc_instructions_drc_tmp->drc_set_id) {
            if (ID_FOR_BASE_LAYOUT == drc_instructions_drc_tmp->downmix_id[0]) {
              if (drc_instructions_drc_tmp->limiter_peak_target_present) {
                eq_set_id[peak_count] = -1;
                signal_peak_level[peak_count] =
                    drc_instructions_drc_tmp->limiter_peak_target +
                    signal_peak_level_tmp;
                explicit_peak_information_present[peak_count] = 0;
                match_found_flag = 1;
                peak_count++;
              }
            }
          }
        }
      }
    }
  }
  if (peak_count > 0) {
    *peak_info_count = peak_count;
  } else {
    *peak_info_count = pre_lim_count;
  }
  return;
}

WORD32
impd_extract_loudness_peak_to_average_info(
    ia_loudness_info_struct* loudness_info, WORD32 dyn_range_measurement_type,
    WORD32* loudness_peak_2_avg_value_present,
    FLOAT32* loudness_peak_2_avg_value) {
  WORD32 k;
  WORD32 program_loudness_present = 0;
  WORD32 peak_loudness_present = 0;
  WORD32 match_measure_program_loudness = 0;
  WORD32 match_measure_peak_loudness = 0;
  FLOAT32 program_loudness = 0.0f;
  FLOAT32 peak_loudness = 0.0f;
  ia_loudness_measure_struct* loudness_measure = NULL;

  for (k = 0; k < loudness_info->measurement_count; k++) {
    loudness_measure = &(loudness_info->loudness_measure[k]);
    if (loudness_measure->method_def == METHOD_DEFINITION_PROGRAM_LOUDNESS) {
      if (match_measure_program_loudness <
          measurement_method_prog_loudness_tbl[loudness_measure
                                                   ->measurement_system]) {
        program_loudness = loudness_measure->method_val;
        program_loudness_present = 1;
        match_measure_program_loudness =
            measurement_method_prog_loudness_tbl[loudness_measure
                                                     ->measurement_system];
      }
    }
    switch (dyn_range_measurement_type) {
      case SHORT_TERM_LOUDNESS_TO_AVG:
        if (loudness_measure->method_def ==
            METHOD_DEFINITION_SHORT_TERM_LOUDNESS_MAX) {
          if (match_measure_peak_loudness <
              measurement_method_peak_loudness_tbl[loudness_measure
                                                       ->measurement_system]) {
            peak_loudness = loudness_measure->method_val;
            peak_loudness_present = 1;
            match_measure_peak_loudness =
                measurement_method_peak_loudness_tbl[loudness_measure
                                                         ->measurement_system];
          }
        }
        break;

      case MOMENTARY_LOUDNESS_TO_AVG:
        if (loudness_measure->method_def ==
            METHOD_DEFINITION_MOMENTARY_LOUDNESS_MAX) {
          if (match_measure_peak_loudness <
              measurement_method_peak_loudness_tbl[loudness_measure
                                                       ->measurement_system]) {
            peak_loudness = loudness_measure->method_val;
            peak_loudness_present = 1;
            match_measure_peak_loudness =
                measurement_method_peak_loudness_tbl[loudness_measure
                                                         ->measurement_system];
          }
        }
        break;

      case TOP_OF_LOUDNESS_RANGE_TO_AVG:
        if (loudness_measure->method_def ==
            METHOD_DEFINITION_MAX_OF_LOUDNESS_RANGE) {
          if (match_measure_peak_loudness <
              measurement_method_peak_loudness_tbl[loudness_measure
                                                       ->measurement_system]) {
            peak_loudness = loudness_measure->method_val;
            peak_loudness_present = 1;
            match_measure_peak_loudness =
                measurement_method_peak_loudness_tbl[loudness_measure
                                                         ->measurement_system];
          }
        }
        break;

      default:
        return (UNEXPECTED_ERROR);

        break;
    }
  }
  if ((program_loudness_present == 1) && (peak_loudness_present == 1)) {
    *loudness_peak_2_avg_value = peak_loudness - program_loudness;
    *loudness_peak_2_avg_value_present = 1;
  }
  return (0);
}

WORD32 impd_loudness_peak_to_average_info(
    ia_drc_loudness_info_set_struct* pstr_loudness_info,
    ia_drc_instructions_struct* str_drc_instruction_str,
    WORD32 requested_dwnmix_id, WORD32 dyn_range_measurement_type,
    WORD32 album_mode, WORD32* loudness_peak_2_avg_value_present,
    FLOAT32* loudness_peak_2_avg_value) {
  WORD32 n, err;
  WORD32 drc_set_id = max(0, str_drc_instruction_str->drc_set_id);

  *loudness_peak_2_avg_value_present = 0;

  if (album_mode == 1) {
    for (n = 0; n < pstr_loudness_info->loudness_info_album_count; n++) {
      ia_loudness_info_struct* loudness_info =
          &(pstr_loudness_info->str_loudness_info_album[n]);
      if (drc_set_id == loudness_info->drc_set_id) {
        if (requested_dwnmix_id == loudness_info->downmix_id) {
          err = impd_extract_loudness_peak_to_average_info(
              loudness_info, dyn_range_measurement_type,
              loudness_peak_2_avg_value_present, loudness_peak_2_avg_value);
          if (err) return (err);
        }
      }
    }
  }
  if (*loudness_peak_2_avg_value_present == 0) {
    for (n = 0; n < pstr_loudness_info->loudness_info_count; n++) {
      ia_loudness_info_struct* loudness_info =
          &(pstr_loudness_info->loudness_info[n]);
      if (drc_set_id == loudness_info->drc_set_id) {
        if (requested_dwnmix_id == loudness_info->downmix_id) {
          err = impd_extract_loudness_peak_to_average_info(
              loudness_info, dyn_range_measurement_type,
              loudness_peak_2_avg_value_present, loudness_peak_2_avg_value);
          if (err) return (err);
        }
      }
    }
  }
  return (0);
}

VOID impd_overall_loudness_present(ia_loudness_info_struct* loudness_info,
                                   WORD32* loudness_info_present) {
  WORD32 m;

  *loudness_info_present = 0;
  for (m = 0; m < loudness_info->measurement_count; m++) {
    if ((loudness_info->loudness_measure[m].method_def ==
         METHOD_DEFINITION_PROGRAM_LOUDNESS) ||
        (loudness_info->loudness_measure[m].method_def ==
         METHOD_DEFINITION_ANCHOR_LOUDNESS)) {
      *loudness_info_present = 1;
    }
  }
  return;
}

WORD32 impd_check_loud_info(WORD32 loudness_info_count,
                            ia_loudness_info_struct* loudness_info,
                            WORD32 requested_dwnmix_id,
                            WORD32 drc_set_id_requested, WORD32* info_count,
                            ia_loudness_info_struct* loudness_info_matching[]) {
  WORD32 n;
  WORD32 loudness_info_present;
  for (n = 0; n < loudness_info_count; n++) {
    if (requested_dwnmix_id == loudness_info[n].downmix_id) {
      if (drc_set_id_requested == loudness_info[n].drc_set_id) {
        impd_overall_loudness_present(&(loudness_info[n]),
                                      &loudness_info_present);

        if (loudness_info_present) {
          loudness_info_matching[*info_count] = &(loudness_info[n]);
          (*info_count)++;
        }
      }
    }
  }

  return (0);
}

WORD32 impd_check_loud_payload(
    WORD32 loudness_info_count, ia_loudness_info_struct* loudness_info,
    WORD32 requested_dwnmix_id, WORD32 drc_set_id_requested, WORD32* info_count,
    ia_loudness_info_struct* loudness_info_matching[]) {
  WORD32 err = 0;

  err = impd_check_loud_info(loudness_info_count, loudness_info,
                             requested_dwnmix_id, drc_set_id_requested,
                             info_count, loudness_info_matching);
  if (err || *info_count) goto matchEnd;
  err = impd_check_loud_info(loudness_info_count, loudness_info,
                             ID_FOR_ANY_DOWNMIX, drc_set_id_requested,
                             info_count, loudness_info_matching);
  if (err || *info_count) goto matchEnd;
  err = impd_check_loud_info(loudness_info_count, loudness_info,
                             requested_dwnmix_id, ID_FOR_ANY_DRC, info_count,
                             loudness_info_matching);
  if (err || *info_count) goto matchEnd;
  err = impd_check_loud_info(loudness_info_count, loudness_info,
                             requested_dwnmix_id, ID_FOR_NO_DRC, info_count,
                             loudness_info_matching);
  if (err || *info_count) goto matchEnd;
  err = impd_check_loud_info(loudness_info_count, loudness_info,
                             ID_FOR_ANY_DOWNMIX, ID_FOR_ANY_DRC, info_count,
                             loudness_info_matching);
  if (err || *info_count) goto matchEnd;
  err = impd_check_loud_info(loudness_info_count, loudness_info,
                             ID_FOR_ANY_DOWNMIX, ID_FOR_NO_DRC, info_count,
                             loudness_info_matching);
  if (err || *info_count) goto matchEnd;
  err = impd_check_loud_info(loudness_info_count, loudness_info,
                             ID_FOR_BASE_LAYOUT, drc_set_id_requested,
                             info_count, loudness_info_matching);
  if (err || *info_count) goto matchEnd;
  err = impd_check_loud_info(loudness_info_count, loudness_info,
                             ID_FOR_BASE_LAYOUT, ID_FOR_ANY_DRC, info_count,
                             loudness_info_matching);
  if (err || *info_count) goto matchEnd;
  err = impd_check_loud_info(loudness_info_count, loudness_info,
                             ID_FOR_BASE_LAYOUT, ID_FOR_NO_DRC, info_count,
                             loudness_info_matching);
  if (err || *info_count) goto matchEnd;
matchEnd:
  return (err);
}

WORD32 impd_find_overall_loudness_info(
    ia_drc_sel_proc_params_struct* pstr_drc_sel_proc_params_struct,
    ia_drc_loudness_info_set_struct* pstr_loudness_info,
    WORD32 requested_dwnmix_id, WORD32 drc_set_id_requested,
    WORD32* overall_loudness_info_present, WORD32* info_count,
    ia_loudness_info_struct* loudness_info_matching[]) {
  WORD32 err;
  WORD32 loudness_drc_set_id_requested;

  *info_count = 0;
  if (drc_set_id_requested < 0) {
    loudness_drc_set_id_requested = ID_FOR_NO_DRC;
  } else {
    loudness_drc_set_id_requested = drc_set_id_requested;
  }
  if (pstr_drc_sel_proc_params_struct->album_mode == 1) {
    err = impd_check_loud_payload(
        pstr_loudness_info->loudness_info_album_count,
        pstr_loudness_info->str_loudness_info_album, requested_dwnmix_id,
        loudness_drc_set_id_requested, info_count, loudness_info_matching);
    if (err) return (err);
  }
  if (*info_count == 0) {
    err = impd_check_loud_payload(pstr_loudness_info->loudness_info_count,
                                  pstr_loudness_info->loudness_info,
                                  requested_dwnmix_id,
                                  loudness_drc_set_id_requested,

                                  info_count, loudness_info_matching);
    if (err) return (err);
  }
  *overall_loudness_info_present = (*info_count > 0);
  return (0);
}

WORD32
impd_high_pass_loudness_adjust_info(ia_loudness_info_struct* loudness_info,
                                    WORD32* loudness_hp_adjust_present,
                                    FLOAT32* loudness_hp_adjust) {
  WORD32 m, k;

  *loudness_hp_adjust_present = 0;
  *loudness_hp_adjust = 0.0f;
  for (m = 0; m < loudness_info->measurement_count; m++) {
    if (loudness_info->loudness_measure[m].measurement_system ==
        MEASUREMENT_SYSTEM_BS_1770_4_PRE_PROCESSING) {
      for (k = 0; k < loudness_info->measurement_count; k++) {
        if (loudness_info->loudness_measure[k].measurement_system ==
            MEASUREMENT_SYSTEM_BS_1770_4) {
          if (loudness_info->loudness_measure[m].method_def ==
              loudness_info->loudness_measure[k].method_def) {
            *loudness_hp_adjust_present = 1;
            *loudness_hp_adjust =
                loudness_info->loudness_measure[m].method_val -
                loudness_info->loudness_measure[k].method_val;
          }
        }
      }
    }
  }
  return (0);
}

WORD32 impd_find_high_pass_loudness_adjust(
    ia_drc_loudness_info_set_struct* pstr_loudness_info,
    WORD32 requested_dwnmix_id, WORD32 drc_set_id_requested, WORD32 album_mode,
    FLOAT32 device_cutoff_freq, WORD32* loudness_hp_adjust_present,
    FLOAT32* loudness_hp_adjust) {
  WORD32 n, err;
  WORD32 loudness_drc_set_id_requested;

  if (drc_set_id_requested < 0) {
    loudness_drc_set_id_requested = 0;
  } else {
    loudness_drc_set_id_requested = drc_set_id_requested;
  }

  *loudness_hp_adjust_present = 0;

  if (album_mode == 1) {
    for (n = 0; n < pstr_loudness_info->loudness_info_album_count; n++) {
      if ((requested_dwnmix_id ==
           pstr_loudness_info->str_loudness_info_album[n].downmix_id) ||
          (ID_FOR_ANY_DOWNMIX ==
           pstr_loudness_info->str_loudness_info_album[n].downmix_id)) {
        if (loudness_drc_set_id_requested ==
            pstr_loudness_info->str_loudness_info_album[n].drc_set_id) {
          err = impd_high_pass_loudness_adjust_info(
              &(pstr_loudness_info->loudness_info[n]),
              loudness_hp_adjust_present, loudness_hp_adjust);
          if (err) return (err);
        }
      }
    }
  }
  if (*loudness_hp_adjust_present == 0) {
    for (n = 0; n < pstr_loudness_info->loudness_info_count; n++) {
      if ((requested_dwnmix_id ==
           pstr_loudness_info->loudness_info[n].downmix_id) ||
          (ID_FOR_ANY_DOWNMIX ==
           pstr_loudness_info->loudness_info[n].downmix_id)) {
        if (loudness_drc_set_id_requested ==
            pstr_loudness_info->loudness_info[n].drc_set_id) {
          err = impd_high_pass_loudness_adjust_info(
              &(pstr_loudness_info->loudness_info[n]),
              loudness_hp_adjust_present, loudness_hp_adjust);
          if (err) return (err);
        }
      }
    }
  }
  if (*loudness_hp_adjust_present == 0) {
    for (n = 0; n < pstr_loudness_info->loudness_info_count; n++) {
      if (ID_FOR_BASE_LAYOUT ==
          pstr_loudness_info->loudness_info[n].downmix_id) /* base layout */
      {
        if (loudness_drc_set_id_requested ==
            pstr_loudness_info->loudness_info[n].drc_set_id) {
          err = impd_high_pass_loudness_adjust_info(
              &(pstr_loudness_info->loudness_info[n]),
              loudness_hp_adjust_present, loudness_hp_adjust);
          if (err) return (err);
        }
      }
    }
  }
  if (*loudness_hp_adjust_present == 0) {
    for (n = 0; n < pstr_loudness_info->loudness_info_count; n++) {
      if (ID_FOR_BASE_LAYOUT ==
          pstr_loudness_info->loudness_info[n].downmix_id) /* base layout */
      {
        if (0 == pstr_loudness_info->loudness_info[n].drc_set_id) {
          err = impd_high_pass_loudness_adjust_info(
              &(pstr_loudness_info->loudness_info[n]),
              loudness_hp_adjust_present, loudness_hp_adjust);
          if (err) return (err);
        }
      }
    }
  }
  if (*loudness_hp_adjust_present == 0) {
    *loudness_hp_adjust = 0.0f;
  } else {
    *loudness_hp_adjust *=
        (max(20.0f, min(500.0f, device_cutoff_freq)) - 20.0f) /
        (500.0f - 20.0f);
  }
  return (0);
}

WORD32 impd_init_loudness_control(
    ia_drc_sel_proc_params_struct* pstr_drc_sel_proc_params_struct,
    ia_drc_loudness_info_set_struct* pstr_loudness_info,
    WORD32 requested_dwnmix_id, WORD32 drc_set_id_requested,
    WORD32 num_compression_eq_count, WORD32* num_compression_eq_id,
    WORD32* loudness_info_count, WORD32 eq_set_id[],
    FLOAT32 loudness_normalization_gain_db[], FLOAT32 loudness[]) {
  WORD32 err, k, info_count = 0, pre_lim_count;
  WORD32 loudness_hp_adjust_present;
  WORD32 overall_loudness_info_present;
  FLOAT32 pre_proc_adjust;

  k = 0;
  if (drc_set_id_requested < 0) {
    for (k = 0; k < num_compression_eq_count; k++) {
      eq_set_id[k] = num_compression_eq_id[k];
      loudness[k] = UNDEFINED_LOUDNESS_VALUE;
      loudness_normalization_gain_db[k] = 0.0f;
    }
  }
  if (k >= MAX_NUM_COMPRESSION_EQ) return UNEXPECTED_ERROR;
  eq_set_id[k] = 0;

  loudness[k] = UNDEFINED_LOUDNESS_VALUE;

  loudness_normalization_gain_db[k] = 0.0f;
  k++;

  pre_lim_count = k;

  if (pstr_drc_sel_proc_params_struct->loudness_normalization_on == 1) {
    WORD32 n;
    ia_loudness_info_struct* loudness_info[16];
    err = impd_find_overall_loudness_info(
        pstr_drc_sel_proc_params_struct, pstr_loudness_info,
        requested_dwnmix_id, drc_set_id_requested,
        &overall_loudness_info_present, &info_count, loudness_info);
    if (err) return (err);

    if (overall_loudness_info_present == 1) {
      WORD32 requested_method_definition = METHOD_DEFINITION_PROGRAM_LOUDNESS;
      WORD32 other_method_definition = METHOD_DEFINITION_PROGRAM_LOUDNESS;
      WORD32 requested_preprocessing = 0;

      const WORD32* system_bonus = measurement_system_default_tbl;

      WORD32 match_measure;
      FLOAT32 method_val = 0;

      switch (pstr_drc_sel_proc_params_struct->loudness_measurement_method) {
        case USER_METHOD_DEFINITION_DEFAULT:
        case USER_METHOD_DEFINITION_PROGRAM_LOUDNESS:
          requested_method_definition = METHOD_DEFINITION_PROGRAM_LOUDNESS;
          other_method_definition = METHOD_DEFINITION_ANCHOR_LOUDNESS;
          break;
        case USER_METHOD_DEFINITION_ANCHOR_LOUDNESS:
          requested_method_definition = METHOD_DEFINITION_ANCHOR_LOUDNESS;
          other_method_definition = METHOD_DEFINITION_PROGRAM_LOUDNESS;
          break;

        default:
          return (UNEXPECTED_ERROR);
          break;
      }

      switch (pstr_drc_sel_proc_params_struct->loudness_measurement_system) {
        case USER_MEASUREMENT_SYSTEM_DEFAULT:
        case USER_MEASUREMENT_SYSTEM_BS_1770_4:
          system_bonus = measurement_system_bs1770_3_tbl;
          break;
        case USER_MEASUREMENT_SYSTEM_USER:
          system_bonus = measurement_system_user_tbl;
          break;
        case USER_MEASUREMENT_SYSTEM_EXPERT_PANEL:
          system_bonus = measurement_system_expert_tbl;
          break;
        case USER_MEASUREMENT_SYSTEM_RESERVED_A:
          system_bonus = measurement_system_rms_a_tbl;
          break;
        case USER_MEASUREMENT_SYSTEM_RESERVED_B:
          system_bonus = measurement_system_rms_b_tbl;
          break;
        case USER_MEASUREMENT_SYSTEM_RESERVED_C:
          system_bonus = measurement_system_rms_c_tbl;
          break;
        case USER_MEASUREMENT_SYSTEM_RESERVED_D:
          system_bonus = measurement_system_rms_d_tbl;
          break;
        case USER_MEASUREMENT_SYSTEM_RESERVED_E:
          system_bonus = measurement_system_rms_e_tbl;
          break;

        default:
          return (UNEXPECTED_ERROR);
          break;
      }

      switch (pstr_drc_sel_proc_params_struct->loudness_measurement_pre_proc) {
        case USER_LOUDNESS_PREPROCESSING_DEFAULT:
        case USER_LOUDNESS_PREPROCESSING_OFF:
          requested_preprocessing = 0;
          break;
        case USER_LOUDNESS_PREPROCESSING_HIGHPASS:
          requested_preprocessing = 1;
          break;

        default:
          return (UNEXPECTED_ERROR);
          break;
      }

      for (k = 0; k < info_count; k++) {
        match_measure = -1;
        for (n = 0; n < loudness_info[k]->measurement_count; n++) {
          ia_loudness_measure_struct* loudness_measure =
              &(loudness_info[k]->loudness_measure[n]);
          if (match_measure <
                  system_bonus[loudness_measure->measurement_system] &&
              requested_method_definition == loudness_measure->method_def) {
            method_val = loudness_measure->method_val;
            match_measure = system_bonus[loudness_measure->measurement_system];
          }
        }
        if (match_measure == -1) {
          for (n = 0; n < loudness_info[k]->measurement_count; n++) {
            ia_loudness_measure_struct* loudness_measure =
                &(loudness_info[k]->loudness_measure[n]);
            if (match_measure <
                    system_bonus[loudness_measure->measurement_system] &&
                other_method_definition == loudness_measure->method_def) {
              method_val = loudness_measure->method_val;
              match_measure =
                  system_bonus[loudness_measure->measurement_system];
            }
          }
        }

        if (requested_preprocessing == 1) {
          err = impd_find_high_pass_loudness_adjust(
              pstr_loudness_info, requested_dwnmix_id, drc_set_id_requested,
              pstr_drc_sel_proc_params_struct->album_mode,
              (FLOAT32)
                  pstr_drc_sel_proc_params_struct->device_cut_off_frequency,
              &loudness_hp_adjust_present, &pre_proc_adjust);
          if (err) return (err);

          if (loudness_hp_adjust_present == 0) {
            pre_proc_adjust = -2.0f;
          }
          method_val += pre_proc_adjust;
        }

        eq_set_id[k] = 0;

        loudness_normalization_gain_db[k] =
            pstr_drc_sel_proc_params_struct->target_loudness - method_val;
        loudness[k] = method_val;
      }
    }
  }
  if (info_count > 0) {
    *loudness_info_count = info_count;
  } else {
    *loudness_info_count = pre_lim_count;
  }

  return (0);
}

#define MIXING_LEVEL_DEFAULT 85.0f
WORD32
impd_mixing_level_info(
    ia_drc_sel_proc_params_struct* pstr_drc_sel_proc_params_struct,
    ia_drc_loudness_info_set_struct* pstr_loudness_info,
    WORD32 requested_dwnmix_id, WORD32 drc_set_id_requested,
    WORD32 eq_set_id_requested, FLOAT32* mixing_level) {
  WORD32 n, k, info_count;
  WORD32 album_mode = pstr_drc_sel_proc_params_struct->album_mode;
  WORD32 loudness_drc_set_id_requested;
  ia_loudness_info_struct* loudness_info;

  *mixing_level = MIXING_LEVEL_DEFAULT;
  if (drc_set_id_requested < 0) {
    loudness_drc_set_id_requested = 0;
  } else {
    loudness_drc_set_id_requested = drc_set_id_requested;
  }
  if (album_mode == 1) {
    info_count = pstr_loudness_info->loudness_info_album_count;
    loudness_info = pstr_loudness_info->str_loudness_info_album;
  } else {
    info_count = pstr_loudness_info->loudness_info_count;
    loudness_info = pstr_loudness_info->loudness_info;
  }
  for (n = 0; n < info_count; n++) {
    if ((requested_dwnmix_id == loudness_info[n].downmix_id) ||
        (ID_FOR_ANY_DOWNMIX == loudness_info[n].downmix_id)) {
      if (loudness_drc_set_id_requested == loudness_info[n].drc_set_id) {
        if (eq_set_id_requested == loudness_info[n].eq_set_id) {
          for (k = 0; k < loudness_info[n].measurement_count; k++) {
            if (loudness_info[n].loudness_measure[k].method_def ==
                METHOD_DEFINITION_MIXING_LEVEL) {
              *mixing_level = loudness_info[n].loudness_measure[k].method_val;
              break;
            }
          }
        }
      }
    }
  }
  return (0);
}
