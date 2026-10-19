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

#include <math.h>
#include <string.h>
#include "ixheaac_type_def.h"
#include "ixheaac_error_standards.h"
#include "ixheaace_error_codes.h"

#include "impd_drc_common_enc.h"
#include "impd_drc_uni_drc.h"
#include "impd_drc_uni_drc_eq.h"

static IA_ERRORCODE impd_drc_derive_subband_center_freq(const WORD32 eq_subband_gain_count,
                                                        const WORD32 eq_subband_gain_format,
                                                        const FLOAT32 audio_sample_rate,
                                                        FLOAT32 *ptr_subband_center_freq) {
  LOOPIDX idx;
  FLOAT32 width, offset;

  switch (eq_subband_gain_format) {
    case GAINFORMAT_QMF32:
    case GAINFORMAT_QMF64:
    case GAINFORMAT_QMF128:
    case GAINFORMAT_UNIFORM:
      width = 0.5f * audio_sample_rate / (FLOAT32)eq_subband_gain_count;
      offset = 0.5f * width;
      for (idx = 0; idx < eq_subband_gain_count; idx++) {
        ptr_subband_center_freq[idx] = idx * width + offset;
      }
      break;
    case GAINFORMAT_QMFHYBRID39:
    case GAINFORMAT_QMFHYBRID71:
    case GAINFORMAT_QMFHYBRID135:
      return IA_EXHEAACE_CONFIG_FATAL_DRC_UNSUPPORTED_CONFIG;
      break;
    default:
      break;
  }

  return IA_NO_ERROR;
}

static VOID impd_drc_derive_zero_response(const FLOAT32 radius, const FLOAT32 angle_radian,
                                          const FLOAT32 frequency_radian, FLOAT32 *response) {
  *response =
      (FLOAT32)(1.0f + radius * radius - 2.0f * radius * cos(frequency_radian - angle_radian));
}

static VOID impd_drc_derive_pole_response(const FLOAT32 radius, const FLOAT32 angle_radian,
                                          const FLOAT32 frequency_radian, FLOAT32 *response) {
  *response =
      (FLOAT32)(1.0f + radius * radius - 2.0f * radius * cos(frequency_radian - angle_radian));
  *response = 1.0f / *response;
}

static VOID impd_drc_derive_fir_filter_response(const WORD32 fir_order, const WORD32 fir_symmetry,
                                                const FLOAT32 *ptr_fir_coeff,
                                                const FLOAT32 frequency_radian,
                                                FLOAT32 *response) {
  LOOPIDX idx;
  WORD32 order_2;
  FLOAT32 sum = 0.0f;

  if ((fir_order & 0x1) != 0) {
    order_2 = (fir_order + 1) / 2;
    if (fir_symmetry != 0) {
      for (idx = 1; idx <= order_2; idx++) {
        sum += (FLOAT32)(ptr_fir_coeff[order_2 - idx] * sin((idx - 0.5f) * frequency_radian));
      }
    } else {
      for (idx = 1; idx <= order_2; idx++) {
        sum += (FLOAT32)(ptr_fir_coeff[order_2 - idx] * cos((idx - 0.5f) * frequency_radian));
      }
    }
    sum *= 2.0f;
  } else {
    order_2 = fir_order / 2;
    if (fir_symmetry != 0) {
      for (idx = 1; idx <= order_2; idx++) {
        sum += (FLOAT32)(ptr_fir_coeff[order_2 - idx] * sin(idx * frequency_radian));
      }
      sum *= 2.0f;
    } else {
      sum = ptr_fir_coeff[order_2];
    }
  }

  *response = sum;
}

static VOID impd_drc_derive_filter_element_response(
    ia_drc_unique_td_filter_element_struct *pstr_unique_td_filter_element,
    const FLOAT32 frequency_radian, FLOAT32 *response) {
  LOOPIDX idx;
  FLOAT32 response_part, radius, angle_radian;
  FLOAT64 combined_response = 1.0;

  if (pstr_unique_td_filter_element->eq_filter_format != FILTER_ELEMENT_FORMAT_POLE_ZERO) {
    impd_drc_derive_fir_filter_response(pstr_unique_td_filter_element->fir_filter_order,
                                        pstr_unique_td_filter_element->fir_symmetry,
                                        pstr_unique_td_filter_element->fir_coefficient,
                                        frequency_radian, &response_part);
    combined_response *= response_part;
  } else {
    for (idx = 0; idx < pstr_unique_td_filter_element->real_zero_radius_one_count; idx++) {
      impd_drc_derive_zero_response(1.0f,
                                    (FLOAT32)(M_PI)*pstr_unique_td_filter_element->zero_sign[idx],
                                    frequency_radian, &response_part);
      combined_response *= response_part;
    }
    for (idx = 0; idx < pstr_unique_td_filter_element->real_zero_count; idx++) {
      if (pstr_unique_td_filter_element->real_zero_radius[idx] >= 0.0f) {
        radius = pstr_unique_td_filter_element->real_zero_radius[idx];
        angle_radian = 0.0f;
      } else {
        radius = -pstr_unique_td_filter_element->real_zero_radius[idx];
        angle_radian = (FLOAT32)(M_PI);
      }
      impd_drc_derive_zero_response(radius, angle_radian, frequency_radian, &response_part);
      combined_response *= response_part;
      impd_drc_derive_zero_response(1.0f / radius, angle_radian, frequency_radian,
                                    &response_part);
      combined_response *= response_part;
    }

    combined_response = sqrt(combined_response);

    for (idx = 0; idx < pstr_unique_td_filter_element->generic_zero_count; idx++) {
      radius = pstr_unique_td_filter_element->generic_zero_radius[idx];

      impd_drc_derive_zero_response(
          radius, (FLOAT32)(M_PI)*pstr_unique_td_filter_element->generic_zero_angle[idx],
          frequency_radian, &response_part);
      combined_response *= response_part;

      impd_drc_derive_zero_response(
          1.0f / radius, (FLOAT32)(M_PI)*pstr_unique_td_filter_element->generic_zero_angle[idx],
          frequency_radian, &response_part);
      combined_response *= response_part;
    }
    for (idx = 0; idx < pstr_unique_td_filter_element->real_pole_count; idx++) {
      if (pstr_unique_td_filter_element->real_pole_radius[idx] >= 0.0f) {
        radius = pstr_unique_td_filter_element->real_pole_radius[idx];
        angle_radian = 0.0f;
      } else {
        radius = -pstr_unique_td_filter_element->real_pole_radius[idx];
        angle_radian = (FLOAT32)(-M_PI);
      }
      impd_drc_derive_pole_response(radius, angle_radian, frequency_radian, &response_part);
      combined_response *= response_part;
    }
    for (idx = 0; idx < pstr_unique_td_filter_element->complex_pole_count; idx++) {
      impd_drc_derive_pole_response(
          pstr_unique_td_filter_element->real_pole_radius[idx],
          (FLOAT32)(M_PI)*pstr_unique_td_filter_element->complex_pole_angle[idx],
          frequency_radian, &response_part);
      combined_response *= response_part * response_part;
    }
  }

  *response = (FLOAT32)combined_response;
}

static VOID impd_drc_derive_filter_block_response(
    ia_drc_unique_td_filter_element_struct *pstr_unique_td_filter_element,
    ia_drc_filter_block_struct *pstr_filter_block, const FLOAT32 frequency_radian,
    FLOAT32 *response) {
  LOOPIDX idx;
  FLOAT32 response_part;
  FLOAT64 combined_response = 1.0;
  ia_drc_filter_element_struct *pstr_filter_element;

  for (idx = 0; idx < pstr_filter_block->filter_element_count; idx++) {
    pstr_filter_element = &pstr_filter_block->filter_element[idx];
    impd_drc_derive_filter_element_response(
        &(pstr_unique_td_filter_element[pstr_filter_element->filter_element_index]),
        frequency_radian, &response_part);
    combined_response *= response_part;

    if (pstr_filter_element->filter_element_gain_present == 1) {
      combined_response *= pow(10.0f, 0.05f * pstr_filter_element->filter_element_gain);
    }
  }

  *response = (FLOAT32)combined_response;
}

static IA_ERRORCODE impd_drc_derive_subband_gains_from_td_cascade(
    ia_drc_unique_td_filter_element_struct *pstr_unique_td_filter_element,
    ia_drc_filter_block_struct *pstr_filter_block,
    ia_drc_td_filter_cascade_struct *pstr_td_filter_cascade, const WORD32 eq_subband_gain_format,
    const WORD32 eq_channel_group_count, const FLOAT32 audio_sample_rate,
    const WORD32 eq_frame_size_subband, ia_drc_subband_filter_struct *pstr_subband_filter,
    VOID *ptr_scratch) {
  IA_ERRORCODE err_code = IA_NO_ERROR;
  LOOPIDX i, j, k;
  WORD32 eq_subband_gain_count = pstr_subband_filter->coeff_count;
  FLOAT32 response_part, frequency_radian;
  FLOAT32 *ptr_subband_center_freq = (FLOAT32 *)ptr_scratch;
  FLOAT64 combined_response;

  err_code = impd_drc_derive_subband_center_freq(eq_subband_gain_count, eq_subband_gain_format,
                                                 audio_sample_rate, ptr_subband_center_freq);
  if (err_code & IA_FATAL_ERROR) {
    return err_code;
  }

  for (i = 0; i < eq_channel_group_count; i++) {
    for (j = 0; j < eq_subband_gain_count; j++) {
      combined_response = pow(10.0f, 0.05f * pstr_td_filter_cascade->eq_cascade_gain[i]);
      frequency_radian = 2.0f * (FLOAT32)M_PI * ptr_subband_center_freq[j] / audio_sample_rate;

      for (k = 0; k < pstr_td_filter_cascade->str_filter_block_refs[i].filter_block_count; k++) {
        impd_drc_derive_filter_block_response(
            pstr_unique_td_filter_element,
            &(pstr_filter_block[pstr_td_filter_cascade->str_filter_block_refs[i]
                                    .filter_block_index[k]]),
            frequency_radian, &response_part);
        combined_response *= response_part;
      }
      pstr_subband_filter[i].subband_coeff[j] = (FLOAT32)combined_response;
    }
    pstr_subband_filter[i].eq_frame_size_subband = eq_frame_size_subband;
  }

  return err_code;
}

static VOID impd_drc_check_presence_and_add_cascade(
    ia_drc_cascade_alignment_group_struct *pstr_cascade_alignment_group, const WORD32 index_c1,
    const WORD32 index_c2, WORD32 *done) {
  LOOPIDX i, j;

  *done = 0;
  for (i = 0; i < pstr_cascade_alignment_group->member_count; i++) {
    if (pstr_cascade_alignment_group->member_index[i] == index_c1) {
      for (j = 0; j < pstr_cascade_alignment_group->member_count; j++) {
        if (pstr_cascade_alignment_group->member_index[j] == index_c2) {
          *done = 1;
        }
      }
      if (*done == 0) {
        pstr_cascade_alignment_group->member_index[pstr_cascade_alignment_group->member_count] =
            index_c2;
        pstr_cascade_alignment_group->member_count++;
        *done = 1;
      }
    }
  }
}

static VOID impd_drc_derive_cascade_alignment_groups(
    const WORD32 eq_channel_group_count, const WORD32 eq_phase_alignment_present,
    const WORD32 eq_phase_alignment[EQ_MAX_CHANNEL_GROUP_COUNT][EQ_MAX_CHANNEL_GROUP_COUNT],
    WORD32 *cascade_alignment_group_count,
    ia_drc_cascade_alignment_group_struct *pstr_cascade_alignment_group) {
  LOOPIDX i, j, k;
  WORD32 group_count = 0, done;

  if (eq_phase_alignment_present != 0) {
    for (i = 0; i < eq_channel_group_count; i++) {
      for (j = i + 1; j < eq_channel_group_count; j++) {
        if (eq_phase_alignment[i][j] == 1) {
          done = 0;
          for (k = 0; k < group_count; k++) {
            impd_drc_check_presence_and_add_cascade(&pstr_cascade_alignment_group[k], i, j,
                                                    &done);
            if (done == 0) {
              impd_drc_check_presence_and_add_cascade(&pstr_cascade_alignment_group[k], j, i,
                                                      &done);
            }
          }
          if (done == 0) {
            pstr_cascade_alignment_group[group_count].member_count = 2;
            pstr_cascade_alignment_group[group_count].member_index[0] = i;
            pstr_cascade_alignment_group[group_count].member_index[1] = j;
            group_count++;
          }
        }
      }
    }
  } else {
    if (eq_channel_group_count > 1) {
      for (i = 0; i < eq_channel_group_count; i++) {
        pstr_cascade_alignment_group[group_count].member_index[i] = i;
      }
      pstr_cascade_alignment_group[group_count].member_count = eq_channel_group_count;
      group_count = 1;
    }
  }

  *cascade_alignment_group_count = group_count;
}

static IA_ERRORCODE impd_drc_derive_allpass_chain(
    ia_drc_filter_cascade_t_domain_struct *pstr_filter_cascade_t_domain,
    ia_drc_allpass_chain_struct *pstr_allpass_chain) {
  LOOPIDX i, j;
  WORD32 allpass_count = 0;

  for (i = 0; i < pstr_filter_cascade_t_domain->block_count; i++) {
    ia_drc_eq_filter_element_struct *pstr_eq_filter_element =
        &pstr_filter_cascade_t_domain->str_eq_filter_block[i].str_eq_filter_element[0];

    if (pstr_filter_cascade_t_domain->str_eq_filter_block[i]
            .str_matching_phase_filter_element_0.is_valid != 1) {
      return IA_EXHEAACE_CONFIG_FATAL_DRC_INVALID_CONFIG;
    } else {
      pstr_allpass_chain->str_matching_phase_filter[allpass_count] =
          pstr_filter_cascade_t_domain->str_eq_filter_block[i]
              .str_matching_phase_filter_element_0;
      allpass_count++;
    }

    for (j = 0; j < pstr_eq_filter_element->phase_alignment_filter_count; j++) {
      if (pstr_eq_filter_element->str_phase_alignment_filter[j].is_valid != 1) {
        return IA_EXHEAACE_CONFIG_FATAL_DRC_INVALID_CONFIG;
      } else {
        pstr_allpass_chain->str_matching_phase_filter[allpass_count] =
            pstr_eq_filter_element->str_phase_alignment_filter[j];
        allpass_count++;
      }
    }
  }
  pstr_allpass_chain->allpass_count = allpass_count;

  return IA_NO_ERROR;
}

static VOID impd_drc_add_allpass_filter_chain(
    ia_drc_allpass_chain_struct *pstr_allpass_chain,
    ia_drc_filter_cascade_t_domain_struct *pstr_filter_cascade_t_domain) {
  LOOPIDX idx;

  for (idx = 0; idx < pstr_allpass_chain->allpass_count; idx++) {
    pstr_filter_cascade_t_domain
        ->str_phase_alignment_filter[pstr_filter_cascade_t_domain->phase_alignment_filter_count +
                                     idx] = pstr_allpass_chain->str_matching_phase_filter[idx];
  }
  pstr_filter_cascade_t_domain->phase_alignment_filter_count += pstr_allpass_chain->allpass_count;
}

static IA_ERRORCODE impd_drc_phase_align_cascade_group(
    const WORD32 cascade_alignment_group_count,
    ia_drc_cascade_alignment_group_struct *pstr_cascade_alignment_group,
    ia_drc_filter_cascade_t_domain_struct *pstr_filter_cascade_t_domain, VOID *ptr_scratch,
    WORD32 *scratch_used) {
  IA_ERRORCODE err_code = IA_NO_ERROR;
  LOOPIDX i, j, k;
  WORD32 cascade_index;
  ia_drc_allpass_chain_struct *pstr_allpass_chain =
      (ia_drc_allpass_chain_struct *)((pUWORD8)(ptr_scratch)) + *scratch_used;

  for (i = 0; i < cascade_alignment_group_count; i++) {
    for (j = 0; j < pstr_cascade_alignment_group[i].member_count; j++) {
      cascade_index = pstr_cascade_alignment_group[i].member_index[j];

      err_code = impd_drc_derive_allpass_chain(&pstr_filter_cascade_t_domain[cascade_index],
                                               &pstr_allpass_chain[j]);
      if (err_code & IA_FATAL_ERROR) {
        return err_code;
      }
      pstr_allpass_chain[j].matches_cascade_index = cascade_index;
    }
    for (j = 0; j < pstr_cascade_alignment_group[i].member_count; j++) {
      cascade_index = pstr_cascade_alignment_group[i].member_index[j];
      for (k = 0; k < pstr_cascade_alignment_group[i].member_count; k++) {
        if (cascade_index != pstr_allpass_chain[k].matches_cascade_index) {
          impd_drc_add_allpass_filter_chain(&pstr_allpass_chain[k],
                                            &pstr_filter_cascade_t_domain[cascade_index]);
        }
      }
    }
  }

  return err_code;
}

static VOID impd_drc_derive_matching_phase_filter_params(
    const WORD32 config, FLOAT32 radius, FLOAT32 angle,
    ia_drc_phase_alignment_filter_struct *pstr_phase_alignment_filter) {
  LOOPIDX idx;
  WORD32 section = pstr_phase_alignment_filter->section_count;
  FLOAT32 z_real, z_imag, product;
  ia_drc_filter_section_struct *pstr_filter_section =
      &pstr_phase_alignment_filter->str_filter_section[section];

  switch (config) {
    case CONFIG_COMPLEX_POLE:
      z_real = (FLOAT32)(radius * cos((FLOAT32)M_PI * angle));
      z_imag = (FLOAT32)(radius * sin((FLOAT32)M_PI * angle));
      product = z_real * z_real + z_imag * z_imag;
      pstr_phase_alignment_filter->gain *= product;
      pstr_filter_section->var_a1 = -2.0f * z_real;
      pstr_filter_section->var_a2 = product;
      pstr_filter_section->var_b1 = -2.0f * z_real / product;
      pstr_filter_section->var_b2 = 1.0f / product;
      pstr_phase_alignment_filter->section_count++;
      break;
    case CONFIG_REAL_POLE:
      pstr_phase_alignment_filter->gain *= (-radius);
      pstr_filter_section->var_a1 = -radius;
      pstr_filter_section->var_a2 = 0.0f;
      pstr_filter_section->var_b1 = -1.0f / radius;
      pstr_filter_section->var_b2 = 0.0f;
      pstr_phase_alignment_filter->section_count++;
      break;
    default:
      break;
  }
  for (idx = 0; idx < MAX_EQ_CHANNEL_COUNT; idx++) {
    pstr_filter_section->str_filter_section_state[idx].state_in_1 = 0.0f;
    pstr_filter_section->str_filter_section_state[idx].state_out_1 = 0.0f;
    pstr_filter_section->str_filter_section_state[idx].state_in_2 = 0.0f;
    pstr_filter_section->str_filter_section_state[idx].state_out_2 = 0.0f;
  }
}

static VOID impd_drc_derive_matching_phase_filter_delay(
    ia_drc_unique_td_filter_element_struct *pstr_filter_element,
    ia_drc_phase_alignment_filter_struct *pstr_phase_alignment_filter) {
  LOOPIDX i, j;
  WORD32 delay = 0;

  if (pstr_filter_element->eq_filter_format == FILTER_ELEMENT_FORMAT_POLE_ZERO) {
    if (pstr_filter_element->real_zero_radius_one_count == 0) {
      delay = pstr_filter_element->real_zero_count + 2 * pstr_filter_element->generic_zero_count -
              pstr_filter_element->real_pole_count - 2 * pstr_filter_element->complex_pole_count;
      delay = MAX(0, delay);
      pstr_phase_alignment_filter->is_valid = 1;
    }
  }

  pstr_phase_alignment_filter->str_audio_delay.delay = delay;
  for (i = 0; i < MAX_EQ_CHANNEL_COUNT; i++) {
    for (j = 0; j < delay; j++) {
      pstr_phase_alignment_filter->str_audio_delay.state[i][j] = 0.0f;
    }
  }
}

static VOID impd_drc_derive_matching_phase_filter(
    ia_drc_unique_td_filter_element_struct *pstr_filter_element, WORD32 filter_element_index,
    ia_drc_matching_phase_filter_struct *pstr_matching_phase_filter) {
  LOOPIDX idx;

  memset(pstr_matching_phase_filter, 0, sizeof(ia_drc_matching_phase_filter_struct));
  pstr_matching_phase_filter->gain = 1.0f;

  if (pstr_filter_element->eq_filter_format == FILTER_ELEMENT_FORMAT_POLE_ZERO) {
    for (idx = 0; idx < pstr_filter_element->real_pole_count; idx++) {
      impd_drc_derive_matching_phase_filter_params(CONFIG_REAL_POLE,
                                                   pstr_filter_element->real_pole_radius[idx],
                                                   0.0f, pstr_matching_phase_filter);
    }
    for (idx = 0; idx < pstr_filter_element->complex_pole_count; idx++) {
      impd_drc_derive_matching_phase_filter_params(
          CONFIG_COMPLEX_POLE, pstr_filter_element->complex_pole_radius[idx],
          pstr_filter_element->complex_pole_angle[idx], pstr_matching_phase_filter);
    }
  }
  impd_drc_derive_matching_phase_filter_delay(pstr_filter_element, pstr_matching_phase_filter);

  pstr_matching_phase_filter->matches_filter_count = 1;
  pstr_matching_phase_filter->matches_filter[0] = filter_element_index;
}

static VOID impd_drc_check_phase_filter_is_equal(
    ia_drc_matching_phase_filter_struct *pstr_matching_phase_filter_1,
    ia_drc_matching_phase_filter_struct *pstr_matching_phase_filter_2, WORD32 *is_equal) {
  LOOPIDX idx;

  *is_equal = 1;
  if (pstr_matching_phase_filter_1->section_count ==
      pstr_matching_phase_filter_2->section_count) {
    for (idx = 0; idx < pstr_matching_phase_filter_1->section_count; idx++) {
      if ((pstr_matching_phase_filter_1->str_filter_section[idx].var_a1 !=
           pstr_matching_phase_filter_2->str_filter_section[idx].var_a1) ||
          (pstr_matching_phase_filter_1->str_filter_section[idx].var_a2 !=
           pstr_matching_phase_filter_2->str_filter_section[idx].var_a2) ||
          (pstr_matching_phase_filter_1->str_filter_section[idx].var_b1 !=
           pstr_matching_phase_filter_2->str_filter_section[idx].var_b1) ||
          (pstr_matching_phase_filter_1->str_filter_section[idx].var_b2 !=
           pstr_matching_phase_filter_2->str_filter_section[idx].var_b2)) {
        *is_equal = 0;
        break;
      }
    }
  } else {
    *is_equal = 0;
  }

  if (pstr_matching_phase_filter_1->str_audio_delay.delay !=
      pstr_matching_phase_filter_2->str_audio_delay.delay) {
    *is_equal = 0;
  }
}

static IA_ERRORCODE impd_drc_add_phase_alignment_filter(
    ia_drc_matching_phase_filter_struct *pstr_matching_phase_filter,
    ia_drc_eq_filter_element_struct *pstr_eq_filter_element) {
  if (pstr_matching_phase_filter->is_valid != 1) {
    return IA_EXHEAACE_CONFIG_FATAL_DRC_INVALID_CONFIG;
  } else {
    pstr_eq_filter_element
        ->str_phase_alignment_filter[pstr_eq_filter_element->phase_alignment_filter_count] =
        *pstr_matching_phase_filter;
    pstr_eq_filter_element->phase_alignment_filter_count++;
  }

  return IA_NO_ERROR;
}

static IA_ERRORCODE impd_drc_derive_element_phase_alignment_filters(
    ia_drc_matching_phase_filter_struct *pstr_matching_phase_filter,
    ia_drc_eq_filter_block_struct *pstr_eq_filter_block, VOID *ptr_scratch,
    WORD32 *scratch_used) {
  IA_ERRORCODE err_code = IA_NO_ERROR;
  LOOPIDX i, j, k;
  WORD32 skip, is_equal;
  WORD32 optimized_phase_filter_count;
  WORD32 path_delay_min, path_delay, path_delay_new, path_delay_to_remove;

  ia_drc_matching_phase_filter_struct *pstr_matching_phase_filter_opt =
      (ia_drc_matching_phase_filter_struct *)((pUWORD8)ptr_scratch) + *scratch_used;
  ia_drc_eq_filter_element_struct *pstr_eq_filter_element;

  optimized_phase_filter_count = 0;
  for (i = 0; i < pstr_eq_filter_block->element_count; i++) {
    is_equal = 0;
    for (j = 0; j < optimized_phase_filter_count; j++) {
      impd_drc_check_phase_filter_is_equal(&pstr_matching_phase_filter[i],
                                           &pstr_matching_phase_filter_opt[j], &is_equal);
      if (is_equal == 1) {
        break;
      }
    }
    if (is_equal != 1) {
      pstr_matching_phase_filter_opt[optimized_phase_filter_count] =
          pstr_matching_phase_filter[i];
      optimized_phase_filter_count++;
    } else {
      pstr_matching_phase_filter_opt[j]
          .matches_filter[pstr_matching_phase_filter_opt[j].matches_filter_count] = i;
      pstr_matching_phase_filter_opt[j].matches_filter_count++;
    }
  }

  for (i = 0; i < pstr_eq_filter_block->element_count; i++) {
    for (j = 0; j < optimized_phase_filter_count; j++) {
      skip = 0;
      for (k = 0; k < pstr_matching_phase_filter_opt[j].matches_filter_count; k++) {
        if (pstr_matching_phase_filter_opt[j].matches_filter[k] == i) {
          skip = 1;
          break;
        }
      }
      if (skip == 0) {
        err_code = impd_drc_add_phase_alignment_filter(
            &pstr_matching_phase_filter_opt[j], &pstr_eq_filter_block->str_eq_filter_element[i]);
        if (err_code & IA_FATAL_ERROR) {
          return err_code;
        }
      }
    }
  }

  path_delay_min = 100000;
  for (i = 0; i < pstr_eq_filter_block->element_count; i++) {
    pstr_eq_filter_element = &pstr_eq_filter_block->str_eq_filter_element[i];
    path_delay = 0;
    for (k = 0; k < pstr_eq_filter_element->phase_alignment_filter_count; k++) {
      path_delay += pstr_eq_filter_element->str_phase_alignment_filter[k].str_audio_delay.delay;
    }
    if (path_delay_min > path_delay) {
      path_delay_min = path_delay;
    }
  }
  if (path_delay_min > 0) {
    for (i = 0; i < pstr_eq_filter_block->element_count; i++) {
      pstr_eq_filter_element = &pstr_eq_filter_block->str_eq_filter_element[i];
      path_delay_to_remove = path_delay_min;
      for (k = 0; k < pstr_eq_filter_element->phase_alignment_filter_count; k++) {
        path_delay = pstr_eq_filter_element->str_phase_alignment_filter[k].str_audio_delay.delay;
        path_delay_new = MAX(0, path_delay - path_delay_to_remove);
        path_delay_to_remove -= path_delay - path_delay_new;
        pstr_eq_filter_element->str_phase_alignment_filter[k].str_audio_delay.delay =
            path_delay_new;
      }
    }
  }

  return err_code;
}

static IA_ERRORCODE impd_drc_convert_pole_zero_to_filter_params(
    const WORD32 config, FLOAT32 radius, FLOAT32 angle, WORD32 *filter_param_count,
    ia_drc_second_order_filter_params_struct *pstr_second_order_filter_params) {
  FLOAT32 z_real, angle_1, angle_2;
  FLOAT32 *ptr_coeff;

  switch (config) {
    case CONFIG_REAL_POLE: {
      pstr_second_order_filter_params[0].radius = radius;
      ptr_coeff = pstr_second_order_filter_params[0].coeff;
      ptr_coeff[0] = -2.0f * radius;
      ptr_coeff[1] = radius * radius;
      *filter_param_count = 1;
    } break;
    case CONFIG_COMPLEX_POLE: {
      z_real = (FLOAT32)(radius * cos((FLOAT32)M_PI * angle));
      pstr_second_order_filter_params[0].radius = radius;
      ptr_coeff = pstr_second_order_filter_params[0].coeff;
      ptr_coeff[0] = -2.0f * z_real;
      ptr_coeff[1] = radius * radius;
      pstr_second_order_filter_params[1].radius = radius;
      pstr_second_order_filter_params[1].coeff[0] = ptr_coeff[0];
      pstr_second_order_filter_params[1].coeff[1] = ptr_coeff[1];
      *filter_param_count = 2;
    } break;
    case CONFIG_REAL_ZERO_RADIUS_ONE: {
      angle_1 = radius;
      angle_2 = angle;
      pstr_second_order_filter_params[0].radius = 1.0f;
      ptr_coeff = pstr_second_order_filter_params[0].coeff;

      if (angle_1 != angle_2) {
        ptr_coeff[0] = 0.0f;
        ptr_coeff[1] = -1.0f;
      } else if (angle_1 == 1.0f) {
        ptr_coeff[0] = -2.0f;
        ptr_coeff[1] = 1.0f;
      } else {
        ptr_coeff[0] = 2.0f;
        ptr_coeff[1] = 1.0f;
      }
      *filter_param_count = 1;
    } break;
    case CONFIG_REAL_ZERO: {
      pstr_second_order_filter_params[0].radius = radius;
      if (fabs(radius) == 1.0f) {
        return IA_EXHEAACE_CONFIG_FATAL_DRC_INVALID_CONFIG;
      } else {
        ptr_coeff = pstr_second_order_filter_params[0].coeff;
        ptr_coeff[0] = -(radius + 1.0f / radius);
        ptr_coeff[1] = 1.0f;
      }
      *filter_param_count = 1;
    } break;
    case CONFIG_GENERIC_ZERO: {
      z_real = (FLOAT32)(radius * cos((FLOAT32)M_PI * angle));
      pstr_second_order_filter_params[0].radius = radius;
      ptr_coeff = pstr_second_order_filter_params[0].coeff;
      ptr_coeff[0] = -2.0f * z_real;
      ptr_coeff[1] = (FLOAT32)(radius * radius);
      z_real = (FLOAT32)(cos((FLOAT32)M_PI * angle) / radius);
      pstr_second_order_filter_params[1].radius = radius;
      ptr_coeff = pstr_second_order_filter_params[1].coeff;
      ptr_coeff[0] = -2.0f * z_real;
      ptr_coeff[1] = 1.0f / (radius * radius);
      *filter_param_count = 2;
    } break;
    default:
      break;
  }

  return IA_NO_ERROR;
}

static VOID impd_drc_convert_fir_filter_params(const WORD32 fir_filter_order,
                                               const WORD32 fir_symmetry,
                                               FLOAT32 *fir_coefficient,
                                               ia_drc_fir_filter_struct *pstr_fir_filter) {
  LOOPIDX i, j;
  FLOAT32 *ptr_coeff = pstr_fir_filter->coeff;

  pstr_fir_filter->coeff_count = fir_filter_order + 1;
  for (i = 0; i < (fir_filter_order / 2 + 1); i++) {
    ptr_coeff[i] = fir_coefficient[i];
  }
  for (i = 0; i < (fir_filter_order + 1) / 2; i++) {
    if (fir_symmetry != 1) {
      ptr_coeff[fir_filter_order - i] = ptr_coeff[i];
    } else {
      ptr_coeff[fir_filter_order - i] = -ptr_coeff[i];
    }
  }
  if ((fir_symmetry == 1) && ((fir_filter_order & 1) == 0)) {
    ptr_coeff[fir_filter_order / 2] = 0.0f;
  }
  for (i = 0; i < MAX_EQ_CHANNEL_COUNT; i++) {
    for (j = 0; j < (fir_filter_order + 1); j++) {
      pstr_fir_filter->state[i][j] = 0.0f;
    }
  }
}

static IA_ERRORCODE impd_drc_derive_pole_zero_filter_params(
    ia_drc_unique_td_filter_element_struct *pstr_filter_element,
    ia_drc_intermediate_filter_params_struct *pstr_intermediate_filter_params) {
  IA_ERRORCODE err_code = IA_NO_ERROR;
  LOOPIDX idx;
  WORD32 param_index, filter_param_count;
  ia_drc_second_order_filter_params_struct *pstr_second_order_filter_params_for_zeros;
  ia_drc_second_order_filter_params_struct *pstr_second_order_filter_params_for_poles;

  pstr_intermediate_filter_params->filter_format = pstr_filter_element->eq_filter_format;
  if (pstr_filter_element->eq_filter_format != FILTER_ELEMENT_FORMAT_POLE_ZERO) {
    pstr_intermediate_filter_params->filter_param_count_for_zeros = 0;
    pstr_intermediate_filter_params->filter_param_count_for_poles = 0;

    impd_drc_convert_fir_filter_params(
        pstr_filter_element->fir_filter_order, pstr_filter_element->fir_symmetry,
        pstr_filter_element->fir_coefficient, &pstr_intermediate_filter_params->str_fir_filter);
  } else {
    pstr_second_order_filter_params_for_zeros =
        pstr_intermediate_filter_params->str_second_order_filter_params_for_zeros;
    pstr_second_order_filter_params_for_poles =
        pstr_intermediate_filter_params->str_second_order_filter_params_for_poles;

    param_index = 0;
    for (idx = 0; idx < pstr_filter_element->real_zero_radius_one_count; idx += 2) {
      err_code = impd_drc_convert_pole_zero_to_filter_params(
          CONFIG_REAL_ZERO_RADIUS_ONE, pstr_filter_element->zero_sign[idx],
          pstr_filter_element->zero_sign[idx + 1], &filter_param_count,
          &(pstr_second_order_filter_params_for_zeros[param_index]));
      if (err_code & IA_FATAL_ERROR) {
        return err_code;
      }
      param_index += filter_param_count;
    }
    for (idx = 0; idx < pstr_filter_element->real_zero_count; idx++) {
      err_code = impd_drc_convert_pole_zero_to_filter_params(
          CONFIG_REAL_ZERO, pstr_filter_element->real_zero_radius[idx], 0.0f, &filter_param_count,
          &(pstr_second_order_filter_params_for_zeros[param_index]));
      if (err_code & IA_FATAL_ERROR) {
        return err_code;
      }
      param_index += filter_param_count;
    }
    for (idx = 0; idx < pstr_filter_element->generic_zero_count; idx++) {
      err_code = impd_drc_convert_pole_zero_to_filter_params(
          CONFIG_GENERIC_ZERO, pstr_filter_element->generic_zero_radius[idx],
          pstr_filter_element->generic_zero_angle[idx], &filter_param_count,
          &(pstr_second_order_filter_params_for_zeros[param_index]));
      if (err_code & IA_FATAL_ERROR) {
        return err_code;
      }
      param_index += filter_param_count;
    }
    pstr_intermediate_filter_params->filter_param_count_for_zeros = param_index;

    param_index = 0;
    for (idx = 0; idx < pstr_filter_element->real_pole_count; idx++) {
      err_code = impd_drc_convert_pole_zero_to_filter_params(
          CONFIG_REAL_POLE, pstr_filter_element->real_pole_radius[idx], 0.0f, &filter_param_count,
          &(pstr_second_order_filter_params_for_poles[param_index]));
      if (err_code & IA_FATAL_ERROR) {
        return err_code;
      }
      param_index += filter_param_count;
    }
    for (idx = 0; idx < pstr_filter_element->complex_pole_count; idx++) {
      err_code = impd_drc_convert_pole_zero_to_filter_params(
          CONFIG_COMPLEX_POLE, pstr_filter_element->complex_pole_radius[idx],
          pstr_filter_element->complex_pole_angle[idx], &filter_param_count,
          &(pstr_second_order_filter_params_for_poles[param_index]));
      if (err_code & IA_FATAL_ERROR) {
        return err_code;
      }
      param_index += filter_param_count;
    }
    pstr_intermediate_filter_params->filter_param_count_for_poles = param_index;
  }

  return err_code;
}

static VOID impd_drc_derive_eq_filter_elements(
    ia_drc_intermediate_filter_params_struct *pstr_intermediate_filter_params,
    ia_drc_eq_filter_element_struct *pstr_eq_filter_element, pUWORD8 ptr_scratch) {
  LOOPIDX idx, ch_idx;
  WORD32 poles_index, zeros_index, pole_order = 0, section;
  WORD32 coeff_count, coeff_idx;
  WORD32 *ptr_poles_done = (WORD32 *)ptr_scratch;
  WORD32 *ptr_zeros_done =
      (WORD32 *)(ptr_scratch +
                 ((REAL_POLE_COUNT_MAX + COMPLEX_POLE_COUNT_MAX) * sizeof(ptr_poles_done[0])));
  FLOAT32 radius_max, radius_diff;
  FLOAT32 temp_b1, temp_b2;
  FLOAT32 *ptr_coeff;

  for (idx = 0; idx < (REAL_ZERO_COUNT_MAX + COMPLEX_ZERO_COUNT_MAX); idx++) {
    ptr_zeros_done[idx] = 0;
  }
  for (idx = 0; idx < (REAL_POLE_COUNT_MAX + COMPLEX_POLE_COUNT_MAX); idx++) {
    ptr_poles_done[idx] = 0;
  }
  section = 0;
  do {
    poles_index = -1;
    radius_max = -1.0;
    for (idx = 0; idx < pstr_intermediate_filter_params->filter_param_count_for_poles; idx++) {
      if ((ptr_poles_done[idx] == 0) && (pstr_intermediate_filter_params->filter_format == 0)) {
        if (radius_max <
            fabs(pstr_intermediate_filter_params->str_second_order_filter_params_for_poles[idx]
                     .radius)) {
          radius_max = (FLOAT32)fabs(
              pstr_intermediate_filter_params->str_second_order_filter_params_for_poles[idx]
                  .radius);
          poles_index = idx;
          if (pstr_intermediate_filter_params->str_second_order_filter_params_for_poles[idx]
                  .coeff[1] == 0.0f) {
            pole_order = 1;
          } else {
            pole_order = 2;
          }
        }
      }
    }

    if (poles_index >= 0) {
      radius_diff = 10.0f;
      zeros_index = -1;
      for (idx = 0; idx < pstr_intermediate_filter_params->filter_param_count_for_zeros; idx++) {
        if (ptr_zeros_done[idx] == 0 && pstr_intermediate_filter_params->filter_format == 0) {
          if (pole_order == 2) {
            if (pstr_intermediate_filter_params->str_second_order_filter_params_for_zeros[idx]
                    .coeff[1] != 0.0f) {
              if (radius_diff > fabs(fabs(pstr_intermediate_filter_params
                                              ->str_second_order_filter_params_for_zeros[idx]
                                              .radius) -
                                     radius_max)) {
                radius_diff =
                    (FLOAT32)fabs(fabs(pstr_intermediate_filter_params
                                           ->str_second_order_filter_params_for_zeros[idx]
                                           .radius) -
                                  radius_max);
                zeros_index = idx;
              }
            }
          } else {
            if (pstr_intermediate_filter_params->str_second_order_filter_params_for_zeros[idx]
                    .coeff[1] == 0.0f) {
              if (radius_diff > fabs(fabs(pstr_intermediate_filter_params
                                              ->str_second_order_filter_params_for_zeros[idx]
                                              .radius) -
                                     radius_max)) {
                radius_diff =
                    (FLOAT32)fabs(fabs(pstr_intermediate_filter_params
                                           ->str_second_order_filter_params_for_zeros[idx]
                                           .radius) -
                                  radius_max);
                zeros_index = idx;
              }
            }
          }
        }
      }

      if (zeros_index == -1) {
        for (idx = 0; idx < pstr_intermediate_filter_params->filter_param_count_for_zeros;
             idx++) {
          if (ptr_zeros_done[idx] == 0 && pstr_intermediate_filter_params->filter_format == 0) {
            if (pole_order == 2) {
              if (pstr_intermediate_filter_params->str_second_order_filter_params_for_zeros[idx]
                      .coeff[1] == 0.0f) {
                if (radius_diff > fabs(fabs(pstr_intermediate_filter_params
                                                ->str_second_order_filter_params_for_zeros[idx]
                                                .radius) -
                                       radius_max)) {
                  radius_diff =
                      (FLOAT32)fabs(fabs(pstr_intermediate_filter_params
                                             ->str_second_order_filter_params_for_zeros[idx]
                                             .radius) -
                                    radius_max);
                  zeros_index = idx;
                }
              }
            } else {
              if (pstr_intermediate_filter_params->str_second_order_filter_params_for_zeros[idx]
                      .coeff[1] != 0.0f) {
                if (radius_diff > fabs(fabs(pstr_intermediate_filter_params
                                                ->str_second_order_filter_params_for_zeros[idx]
                                                .radius) -
                                       radius_max)) {
                  radius_diff =
                      (FLOAT32)fabs(fabs(pstr_intermediate_filter_params
                                             ->str_second_order_filter_params_for_zeros[idx]
                                             .radius) -
                                    radius_max);
                  zeros_index = idx;
                }
              }
            }
          }
        }
      }
      pstr_eq_filter_element->str_pole_zero_filter.str_filter_section[section].var_a1 =
          pstr_intermediate_filter_params->str_second_order_filter_params_for_poles[poles_index]
              .coeff[0];
      pstr_eq_filter_element->str_pole_zero_filter.str_filter_section[section].var_a2 =
          pstr_intermediate_filter_params->str_second_order_filter_params_for_poles[poles_index]
              .coeff[1];
      if (zeros_index < 0) {
        pstr_eq_filter_element->str_pole_zero_filter.str_filter_section[section].var_b1 = 0.0f;
        pstr_eq_filter_element->str_pole_zero_filter.str_filter_section[section].var_b2 = 0.0f;
        pstr_eq_filter_element->str_pole_zero_filter.str_audio_delay.delay++;
      } else {
        pstr_eq_filter_element->str_pole_zero_filter.str_filter_section[section].var_b1 =
            pstr_intermediate_filter_params->str_second_order_filter_params_for_zeros[zeros_index]
                .coeff[0];
        pstr_eq_filter_element->str_pole_zero_filter.str_filter_section[section].var_b2 =
            pstr_intermediate_filter_params->str_second_order_filter_params_for_zeros[zeros_index]
                .coeff[1];
      }
      for (ch_idx = 0; ch_idx < MAX_EQ_CHANNEL_COUNT; ch_idx++) {
        pstr_eq_filter_element->str_pole_zero_filter.str_filter_section[section]
            .str_filter_section_state[ch_idx]
            .state_in_1 = 0.0f;
        pstr_eq_filter_element->str_pole_zero_filter.str_filter_section[section]
            .str_filter_section_state[ch_idx]
            .state_in_2 = 0.0f;
        pstr_eq_filter_element->str_pole_zero_filter.str_filter_section[section]
            .str_filter_section_state[ch_idx]
            .state_out_1 = 0.0f;
        pstr_eq_filter_element->str_pole_zero_filter.str_filter_section[section]
            .str_filter_section_state[ch_idx]
            .state_out_2 = 0.0f;
      }
      if (zeros_index >= 0) {
        ptr_zeros_done[zeros_index] = 1;
      }
      if (poles_index >= 0) {
        ptr_poles_done[poles_index] = 1;
      }
      section++;
    }
  } while (poles_index >= 0);

  pstr_eq_filter_element->str_pole_zero_filter.section_count = section;

  coeff_count = 1;
  ptr_coeff = pstr_eq_filter_element->str_pole_zero_filter.str_fir_filter.coeff;
  ptr_coeff[0] = 1.0f;
  for (idx = 0; idx < pstr_intermediate_filter_params->filter_param_count_for_zeros; idx++) {
    if (ptr_zeros_done[idx] == 0 && pstr_intermediate_filter_params->filter_format == 0) {
      temp_b1 =
          pstr_intermediate_filter_params->str_second_order_filter_params_for_zeros[idx].coeff[0];
      temp_b2 =
          pstr_intermediate_filter_params->str_second_order_filter_params_for_zeros[idx].coeff[1];

      coeff_count += 2;
      coeff_idx = coeff_count - 1;
      ptr_coeff[coeff_idx] = temp_b2 * ptr_coeff[coeff_idx - 2];
      coeff_idx--;
      if (coeff_idx > 1) {
        ptr_coeff[coeff_idx] =
            temp_b1 * ptr_coeff[coeff_idx - 1] + temp_b2 * ptr_coeff[coeff_idx - 2];
        coeff_idx--;
        for (; coeff_idx > 1; coeff_idx--) {
          ptr_coeff[coeff_idx] +=
              temp_b1 * ptr_coeff[coeff_idx - 1] + temp_b2 * ptr_coeff[coeff_idx - 2];
        }
        ptr_coeff[1] += temp_b1 * ptr_coeff[0];
      } else {
        ptr_coeff[1] = temp_b1 * ptr_coeff[0];
      }
    }
    ptr_zeros_done[idx] = 1;
  }
  if (coeff_count > 1) {
    pstr_eq_filter_element->str_pole_zero_filter.fir_coeffs_present = 1;
    pstr_eq_filter_element->str_pole_zero_filter.str_fir_filter.coeff_count = coeff_count;
  } else {
    pstr_eq_filter_element->str_pole_zero_filter.fir_coeffs_present = 0;
    pstr_eq_filter_element->str_pole_zero_filter.str_fir_filter.coeff_count = 0;
  }
}

static IA_ERRORCODE impd_drc_derive_filter_block(
    ia_drc_unique_td_filter_element_struct *pstr_unique_td_filter_element,
    ia_drc_filter_block_struct *pstr_filter_block,
    ia_drc_eq_filter_block_struct *pstr_eq_filter_block, VOID *ptr_scratch,
    WORD32 *scratch_used) {
  IA_ERRORCODE err_code = IA_NO_ERROR;
  LOOPIDX i, j;
  WORD32 filter_index;
  WORD32 temp_scratch_used = *scratch_used;
  ia_drc_intermediate_filter_params_struct str_intermediate_filter_params;
  ia_drc_eq_filter_element_struct *pstr_eq_filter_element;
  ia_drc_filter_element_struct *pstr_filter_element;
  ia_drc_matching_phase_filter_struct *pstr_matching_phase_filter =
      (ia_drc_matching_phase_filter_struct *)((pUWORD8)(ptr_scratch)) + temp_scratch_used;

  temp_scratch_used += sizeof(ia_drc_matching_phase_filter_struct) * FILTER_ELEMENT_COUNT_MAX;

  for (i = 0; i < pstr_filter_block->filter_element_count; i++) {
    if ((pstr_unique_td_filter_element[pstr_filter_block->filter_element[i].filter_element_index]
             .eq_filter_format == FILTER_ELEMENT_FORMAT_FIR) &&
        (pstr_filter_block->filter_element_count > 1)) {
      return IA_EXHEAACE_CONFIG_FATAL_DRC_INVALID_CONFIG;
    }
  }
  for (i = 0; i < pstr_filter_block->filter_element_count; i++) {
    pstr_filter_element = &pstr_filter_block->filter_element[i];
    filter_index = pstr_filter_element->filter_element_index;
    pstr_eq_filter_element = &pstr_eq_filter_block->str_eq_filter_element[i];

    if (pstr_unique_td_filter_element[filter_index].eq_filter_format ==
        FILTER_ELEMENT_FORMAT_POLE_ZERO) {
      err_code = impd_drc_derive_pole_zero_filter_params(
          &(pstr_unique_td_filter_element[filter_index]), &str_intermediate_filter_params);
      if (err_code & IA_FATAL_ERROR) {
        return err_code;
      }

      impd_drc_derive_eq_filter_elements(&str_intermediate_filter_params, pstr_eq_filter_element,
                                         (pUWORD8)(ptr_scratch) + temp_scratch_used);
      pstr_eq_filter_element->format = FILTER_ELEMENT_FORMAT_POLE_ZERO;
    } else {
      impd_drc_convert_fir_filter_params(
          pstr_unique_td_filter_element[filter_index].fir_filter_order,
          pstr_unique_td_filter_element[filter_index].fir_symmetry,
          pstr_unique_td_filter_element[filter_index].fir_coefficient,
          &pstr_eq_filter_element->str_fir_filter);
      pstr_eq_filter_element->format = FILTER_ELEMENT_FORMAT_FIR;
    }
    if (pstr_filter_element->filter_element_gain_present != 1) {
      pstr_eq_filter_element->element_gain_linear = 1.0f;
    } else {
      pstr_eq_filter_element->element_gain_linear =
          (FLOAT32)pow(10.0f, 0.05f * pstr_filter_element->filter_element_gain);
    }
    for (j = 0; j < pstr_unique_td_filter_element[filter_index].real_zero_count; j++) {
      if (pstr_unique_td_filter_element[filter_index].real_zero_radius[j] > 0.0f) {
        pstr_eq_filter_element->element_gain_linear =
            -pstr_eq_filter_element->element_gain_linear;
      }
    }
    impd_drc_derive_matching_phase_filter(&(pstr_unique_td_filter_element[filter_index]), i,
                                          &pstr_matching_phase_filter[i]);
  }
  pstr_eq_filter_block->str_matching_phase_filter_element_0 = pstr_matching_phase_filter[0];
  pstr_eq_filter_block->element_count = pstr_filter_block->filter_element_count;

  err_code = impd_drc_derive_element_phase_alignment_filters(
      pstr_matching_phase_filter, pstr_eq_filter_block, ptr_scratch, &temp_scratch_used);
  if (err_code & IA_FATAL_ERROR) {
    return err_code;
  }

  return err_code;
}

static IA_ERRORCODE impd_drc_derive_cascade_phase_alignment_filters(
    ia_drc_td_filter_cascade_struct *pstr_td_filter_cascade, const WORD32 channel_group_count,
    ia_drc_filter_cascade_t_domain_struct *pstr_filter_cascade_t_domain, VOID *ptr_scratch,
    WORD32 *scratch_used) {
  IA_ERRORCODE err_code = IA_NO_ERROR;
  WORD32 cascade_alignment_group_count = 0;
  ia_drc_cascade_alignment_group_struct *pstr_cascade_alignment_group =
      (ia_drc_cascade_alignment_group_struct *)ptr_scratch;
  *scratch_used +=
      sizeof(ia_drc_cascade_alignment_group_struct) * (EQ_MAX_CHANNEL_GROUP_COUNT / 2);

  impd_drc_derive_cascade_alignment_groups(
      channel_group_count, pstr_td_filter_cascade->eq_phase_alignment_present,
      (const WORD32(*)[EQ_MAX_CHANNEL_GROUP_COUNT])pstr_td_filter_cascade->eq_phase_alignment,
      &cascade_alignment_group_count, pstr_cascade_alignment_group);

  if (cascade_alignment_group_count > 0) {
    err_code = impd_drc_phase_align_cascade_group(
        cascade_alignment_group_count, pstr_cascade_alignment_group, pstr_filter_cascade_t_domain,
        ptr_scratch, scratch_used);
    if (err_code & IA_FATAL_ERROR) {
      return err_code;
    }
  }

  return err_code;
}

static IA_ERRORCODE impd_drc_derive_filter_cascade(
    ia_drc_unique_td_filter_element_struct *pstr_unique_td_filter_element,
    ia_drc_filter_block_struct *pstr_filter_block,
    ia_drc_td_filter_cascade_struct *pstr_td_filter_cascade, WORD32 channel_group_count,
    ia_drc_filter_cascade_t_domain_struct *pstr_filter_cascade_t_domain, VOID *ptr_scratch,
    WORD32 *scratch_used) {
  IA_ERRORCODE err_code = IA_NO_ERROR;
  LOOPIDX i, j;

  for (i = 0; i < channel_group_count; i++) {
    for (j = 0; j < pstr_td_filter_cascade->str_filter_block_refs[i].filter_block_count; j++) {
      err_code = impd_drc_derive_filter_block(
          pstr_unique_td_filter_element,
          &(pstr_filter_block[pstr_td_filter_cascade->str_filter_block_refs[i]
                                  .filter_block_index[j]]),
          &(pstr_filter_cascade_t_domain[i].str_eq_filter_block[j]), ptr_scratch, scratch_used);
      if (err_code & IA_FATAL_ERROR) {
        return err_code;
      }
    }
    pstr_filter_cascade_t_domain[i].cascade_gain_linear =
        (FLOAT32)pow(10.0f, 0.05f * pstr_td_filter_cascade->eq_cascade_gain[i]);
    pstr_filter_cascade_t_domain[i].block_count = j;
  }

  err_code = impd_drc_derive_cascade_phase_alignment_filters(
      pstr_td_filter_cascade, channel_group_count, pstr_filter_cascade_t_domain, ptr_scratch,
      scratch_used);
  if (err_code & IA_FATAL_ERROR) {
    return err_code;
  }

  return err_code;
}

static VOID impd_drc_derive_subband_eq(
    ia_drc_eq_subband_gain_vector_struct *pstr_eq_subband_gain_vector,
    const WORD32 eq_subband_gain_count, ia_drc_subband_filter_struct *pstr_subband_filter) {
  LOOPIDX idx;

  for (idx = 0; idx < eq_subband_gain_count; idx++) {
    pstr_subband_filter->subband_coeff[idx] =
        (FLOAT32)pstr_eq_subband_gain_vector->eq_subband_gain[idx];
  }
  pstr_subband_filter->coeff_count = eq_subband_gain_count;
}

static FLOAT32 impd_drc_decode_eq_node_freq(const WORD32 eq_node_freq_index) {
  FLOAT32 eq_node_frequency;

  eq_node_frequency =
      (FLOAT32)(pow(STEP_RATIO_F_LOW, 1.0f + eq_node_freq_index * STEP_RATIO_COMPUTED));

  return eq_node_frequency;
}

static FLOAT32 impd_drc_warp_freq_delta(const FLOAT32 f_subband, const FLOAT32 node_frequency_0,
                                        const WORD32 eq_node_freq_index) {
  FLOAT32 wraped_delta_frequency;

  wraped_delta_frequency =
      (FLOAT32)((log10(f_subband) / log10(node_frequency_0) - 1.0f) / STEP_RATIO_COMPUTED -
                (FLOAT32)eq_node_freq_index);

  return wraped_delta_frequency;
}

static VOID impd_drc_interpolate_eq_gain(const WORD32 band_step, const FLOAT32 eq_gain_0,
                                         const FLOAT32 eq_gain_1, const FLOAT32 eq_slope_0,
                                         const FLOAT32 eq_slope_1, const FLOAT32 wrap_delta_freq,
                                         FLOAT32 *interpolated_gain) {
  FLOAT32 k1, k2, val_a, val_b;
  FLOAT32 nodes_per_octave_count = 3.128f;
  FLOAT32 gain_left = eq_gain_0;
  FLOAT32 gain_right = eq_gain_1;
  FLOAT32 slope_left = eq_slope_0 / nodes_per_octave_count;
  FLOAT32 slope_right = eq_slope_1 / nodes_per_octave_count;
  FLOAT32 band_step_inv = (FLOAT32)(1.0 / (FLOAT32)band_step);
  FLOAT32 band_step_inv_square = band_step_inv * band_step_inv;

  k1 = (gain_right - gain_left) * band_step_inv_square;
  k2 = slope_right + slope_left;
  val_a = (FLOAT32)(band_step_inv * (band_step_inv * k2 - 2.0 * k1));
  val_b = (FLOAT32)(3.0 * k1 - band_step_inv * (k2 + slope_left));

  *interpolated_gain =
      (((val_a * wrap_delta_freq + val_b) * wrap_delta_freq + slope_left) * wrap_delta_freq) +
      gain_left;
}

static IA_ERRORCODE impd_drc_interpolate_subband_spline(
    ia_drc_eq_subband_gain_spline_struct *pstr_eq_subband_gain_spline,
    const WORD32 eq_subband_gain_count, const WORD32 eq_subband_gain_format,
    const FLOAT32 audio_sample_rate, ia_drc_subband_filter_struct *pstr_subband_filter,
    VOID *ptr_scratch) {
  IA_ERRORCODE err_code = IA_NO_ERROR;
  LOOPIDX i, j;
  WORD32 eq_node_freq_index[32] = {0};
  WORD32 n_eq_nodes = pstr_eq_subband_gain_spline->n_eq_nodes;
  WORD32 eq_node_count_max = 33;
  WORD32 eq_node_index_max = eq_node_count_max - 1;
  WORD32 *ptr_eq_freq_delta = pstr_eq_subband_gain_spline->eq_freq_delta;
  FLOAT32 eq_gain[32] = {0}, eq_node_freq[32] = {0};
  FLOAT32 freq_subband, warped_delta_freq, g_eq_subband_db;
  FLOAT32 eq_gain_initial = pstr_eq_subband_gain_spline->eq_gain_initial;
  FLOAT32 *ptr_subband_center_freq = (FLOAT32 *)ptr_scratch;
  FLOAT32 *ptr_eq_slope = pstr_eq_subband_gain_spline->eq_slope;
  FLOAT32 *ptr_eq_gain_delta = pstr_eq_subband_gain_spline->eq_gain_delta;
  FLOAT32 *ptr_subband_coeff = pstr_subband_filter->subband_coeff;

  eq_gain[0] = eq_gain_initial;
  eq_node_freq_index[0] = 0;
  eq_node_freq[0] = impd_drc_decode_eq_node_freq(eq_node_freq_index[0]);
  for (i = 1; i < n_eq_nodes; i++) {
    eq_gain[i] = eq_gain[i - 1] + ptr_eq_gain_delta[i];
    eq_node_freq_index[i] = eq_node_freq_index[i - 1] + ptr_eq_freq_delta[i];
    eq_node_freq[i] = impd_drc_decode_eq_node_freq(eq_node_freq_index[i]);
  }
  if ((eq_node_freq[n_eq_nodes - 1] < audio_sample_rate * 0.5f) &&
      (eq_node_freq_index[n_eq_nodes - 1] < eq_node_index_max)) {
    ptr_eq_slope[n_eq_nodes] = 0;
    eq_gain[n_eq_nodes] = eq_gain[n_eq_nodes - 1];
    ptr_eq_freq_delta[n_eq_nodes] = eq_node_index_max - eq_node_freq_index[n_eq_nodes - 1];
    eq_node_freq_index[n_eq_nodes] = eq_node_index_max;
    eq_node_freq[n_eq_nodes] = impd_drc_decode_eq_node_freq(eq_node_freq_index[n_eq_nodes]);
    n_eq_nodes += 1;
  }

  err_code = impd_drc_derive_subband_center_freq(eq_subband_gain_count, eq_subband_gain_format,
                                                 audio_sample_rate, ptr_subband_center_freq);
  if (err_code & IA_FATAL_ERROR) {
    return err_code;
  }

  for (i = 0; i < n_eq_nodes - 1; i++) {
    for (j = 0; j < eq_subband_gain_count; j++) {
      freq_subband = MAX(ptr_subband_center_freq[j], eq_node_freq[0]);
      freq_subband = MIN(freq_subband, eq_node_freq[n_eq_nodes - 1]);
      if ((freq_subband >= eq_node_freq[i]) && (freq_subband <= eq_node_freq[i + 1])) {
        warped_delta_freq =
            impd_drc_warp_freq_delta(freq_subband, eq_node_freq[0], eq_node_freq_index[i]);
        impd_drc_interpolate_eq_gain(ptr_eq_freq_delta[i + 1], eq_gain[i], eq_gain[i + 1],
                                     ptr_eq_slope[i], ptr_eq_slope[i + 1], warped_delta_freq,
                                     &g_eq_subband_db);

        ptr_subband_coeff[j] = (FLOAT32)pow(2.0, (FLOAT32)(g_eq_subband_db / 6.0f));
      }
    }
  }
  pstr_subband_filter->coeff_count = eq_subband_gain_count;

  return err_code;
}

static IA_ERRORCODE impd_drc_derive_subband_gains(
    ia_drc_eq_coefficients_struct *pstr_eq_coefficients, const WORD32 eq_channel_group_count,
    const WORD32 *subband_gains_index, const FLOAT32 audio_sample_rate,
    const WORD32 eq_frame_size_subband, ia_drc_subband_filter_struct *pstr_subband_filter,
    VOID *ptr_scratch) {
  IA_ERRORCODE err_code = IA_NO_ERROR;
  LOOPIDX idx;

  for (idx = 0; idx < eq_channel_group_count; idx++) {
    if (pstr_eq_coefficients->eq_subband_gain_representation != 1) {
      impd_drc_derive_subband_eq(
          &(pstr_eq_coefficients->str_eq_subband_gain_vector[subband_gains_index[idx]]),
          pstr_eq_coefficients->eq_subband_gain_count, &(pstr_subband_filter[idx]));
    } else {
      err_code = impd_drc_interpolate_subband_spline(
          &(pstr_eq_coefficients->str_eq_subband_gain_spline[subband_gains_index[idx]]),
          pstr_eq_coefficients->eq_subband_gain_count,
          pstr_eq_coefficients->eq_subband_gain_format, audio_sample_rate,
          &(pstr_subband_filter[idx]), ptr_scratch);
      if (err_code & IA_FATAL_ERROR) {
        return err_code;
      }
    }
    pstr_subband_filter[idx].eq_frame_size_subband = eq_frame_size_subband;
  }

  return err_code;
}

IA_ERRORCODE impd_drc_get_eq_complexity(ia_drc_eq_set_struct *pstr_eq_set,
                                        WORD32 *eq_complexity_level) {
  LOOPIDX idx_c, idx_b, i, j;
  WORD32 group;
  WORD32 fir_order_complexity = 0;
  WORD32 zero_pole_pair_count_complexity = 0;
  WORD32 subband_filter_complexity = 0;
  FLOAT32 complexity;
  ia_drc_filter_cascade_t_domain_struct *pstr_filter_cascade_t_domain;
  ia_drc_eq_filter_block_struct *pstr_eq_filter_block;
  ia_drc_eq_filter_element_struct *pstr_eq_filter_element;

  for (idx_c = 0; idx_c < pstr_eq_set->audio_channel_count; idx_c++) {
    group = pstr_eq_set->eq_channel_group_for_channel[idx_c];
    if (group >= 0) {
      switch (pstr_eq_set->domain) {
        case EQ_FILTER_DOMAIN_TIME: {
          pstr_filter_cascade_t_domain = &pstr_eq_set->str_filter_cascade_t_domain[group];
          for (idx_b = 0; idx_b < pstr_filter_cascade_t_domain->block_count; idx_b++) {
            pstr_eq_filter_block = &pstr_filter_cascade_t_domain->str_eq_filter_block[idx_b];
            for (i = 0; i < pstr_eq_filter_block->element_count; i++) {
              pstr_eq_filter_element = &pstr_eq_filter_block->str_eq_filter_element[i];
              switch (pstr_eq_filter_element->format) {
                case FILTER_ELEMENT_FORMAT_POLE_ZERO:
                  zero_pole_pair_count_complexity +=
                      pstr_eq_filter_element->str_pole_zero_filter.section_count * 2;
                  if (pstr_eq_filter_element->str_pole_zero_filter.fir_coeffs_present) {
                    fir_order_complexity +=
                        pstr_eq_filter_element->str_pole_zero_filter.str_fir_filter.coeff_count -
                        1;
                  }
                  break;
                case FILTER_ELEMENT_FORMAT_FIR:
                  fir_order_complexity += pstr_eq_filter_element->str_fir_filter.coeff_count - 1;
                  break;
                default:
                  break;
              }
              for (j = 0; j < pstr_eq_filter_element->phase_alignment_filter_count; j++) {
                zero_pole_pair_count_complexity +=
                    pstr_eq_filter_element->str_phase_alignment_filter[j].section_count * 2;
              }
            }
          }
          for (idx_b = 0; idx_b < pstr_filter_cascade_t_domain->phase_alignment_filter_count;
               idx_b++) {
            zero_pole_pair_count_complexity +=
                pstr_filter_cascade_t_domain->str_phase_alignment_filter[idx_b].section_count * 2;
          }
        } break;
        case EQ_FILTER_DOMAIN_SUBBAND:
          subband_filter_complexity++;
          break;
        case EQ_FILTER_DOMAIN_NONE:
        default:
          break;
      }
    }
  }
  complexity = COMPLEXITY_W_SUBBAND_EQ * subband_filter_complexity;
  complexity += COMPLEXITY_W_FIR * fir_order_complexity;
  complexity += COMPLEXITY_W_IIR * zero_pole_pair_count_complexity;
  complexity = (FLOAT32)(log10(complexity / pstr_eq_set->audio_channel_count) / log10(2.0f));
  *eq_complexity_level = (WORD32)MAX(0, ceil(complexity));
  if (*eq_complexity_level > EQ_COMPLEXITY_LEVEL_MAX) {
    return IA_EXHEAACE_CONFIG_FATAL_DRC_INVALID_CONFIG;
  }

  return IA_NO_ERROR;
}

IA_ERRORCODE impd_drc_derive_eq_set(ia_drc_eq_coefficients_struct *pstr_eq_coefficients,
                                    ia_drc_eq_instructions_struct *pstr_eq_instructions,
                                    const FLOAT32 audio_sample_rate, const WORD32 drc_frame_size,
                                    const WORD32 sub_band_domain_mode,
                                    ia_drc_eq_set_struct *pstr_eq_set, VOID *ptr_scratch,
                                    WORD32 *scratch_used) {
  IA_ERRORCODE err_code = IA_NO_ERROR;
  LOOPIDX idx;
  WORD32 eq_frame_size_subband;

  pstr_eq_set->domain = EQ_FILTER_DOMAIN_NONE;

  if (sub_band_domain_mode != SUBBAND_DOMAIN_MODE_OFF) {
    switch (sub_band_domain_mode) {
      case SUBBAND_DOMAIN_MODE_STFT256:
        if (pstr_eq_coefficients->eq_subband_gain_count != STFT256_AUDIO_CODEC_SUBBAND_COUNT) {
          return IA_EXHEAACE_CONFIG_FATAL_DRC_INVALID_CONFIG;
        }
        eq_frame_size_subband = drc_frame_size / STFT256_AUDIO_CODEC_SUBBAND_DOWNSAMPLING_FACTOR;
        break;
      case SUBBAND_DOMAIN_MODE_QMF71:
        if (pstr_eq_coefficients->eq_subband_gain_count != QMF71_AUDIO_CODEC_SUBBAND_COUNT) {
          return IA_EXHEAACE_CONFIG_FATAL_DRC_INVALID_CONFIG;
        }
        eq_frame_size_subband = drc_frame_size / QMF71_AUDIO_CODEC_SUBBAND_DOWNSAMPLING_FACTOR;
        break;
      case SUBBAND_DOMAIN_MODE_QMF64:
        if (pstr_eq_coefficients->eq_subband_gain_count != QMF64_AUDIO_CODEC_SUBBAND_COUNT) {
          return IA_EXHEAACE_CONFIG_FATAL_DRC_INVALID_CONFIG;
        }
        eq_frame_size_subband = drc_frame_size / QMF64_AUDIO_CODEC_SUBBAND_DOWNSAMPLING_FACTOR;
        break;
      default:
        return IA_EXHEAACE_CONFIG_FATAL_DRC_INVALID_CONFIG;
        break;
    }
    if (pstr_eq_instructions->subband_gains_present == 1) {
      err_code = impd_drc_derive_subband_gains(
          pstr_eq_coefficients, pstr_eq_instructions->eq_channel_group_count,
          pstr_eq_instructions->subband_gains_index, audio_sample_rate, eq_frame_size_subband,
          pstr_eq_set->str_subband_filter, ptr_scratch);
      if (err_code & IA_FATAL_ERROR) {
        return err_code;
      }
    } else {
      if (pstr_eq_instructions->td_filter_cascade_present == 1) {
        err_code = impd_drc_derive_subband_gains_from_td_cascade(
            pstr_eq_coefficients->str_unique_td_filter_element,
            pstr_eq_coefficients->str_filter_block, &pstr_eq_instructions->str_td_filter_cascade,
            pstr_eq_coefficients->eq_subband_gain_format,
            pstr_eq_instructions->eq_channel_group_count, audio_sample_rate,
            eq_frame_size_subband, pstr_eq_set->str_subband_filter, ptr_scratch);
        if (err_code & IA_FATAL_ERROR) {
          return err_code;
        }
      } else {
        err_code = IA_EXHEAACE_CONFIG_NONFATAL_DRC_MISSING_CONFIG;
      }
    }
    pstr_eq_set->domain |= EQ_FILTER_DOMAIN_SUBBAND;
  } else {
    if (pstr_eq_instructions->td_filter_cascade_present == 1) {
      err_code = impd_drc_derive_filter_cascade(
          pstr_eq_coefficients->str_unique_td_filter_element,
          pstr_eq_coefficients->str_filter_block, &pstr_eq_instructions->str_td_filter_cascade,
          pstr_eq_instructions->eq_channel_group_count, pstr_eq_set->str_filter_cascade_t_domain,
          ptr_scratch, scratch_used);
      if (err_code & IA_FATAL_ERROR) {
        return err_code;
      }
    }
    pstr_eq_set->domain |= EQ_FILTER_DOMAIN_TIME;
  }

  pstr_eq_set->audio_channel_count = pstr_eq_instructions->eq_channel_count;
  pstr_eq_set->eq_channel_group_count = pstr_eq_instructions->eq_channel_group_count;

  for (idx = 0; idx < pstr_eq_instructions->eq_channel_count; idx++) {
    pstr_eq_set->eq_channel_group_for_channel[idx] =
        pstr_eq_instructions->eq_channel_group_for_channel[idx];
  }

  return err_code;
}
