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
#include "impd_drc_gain_dec.h"
#include "impd_drc_eq.h"

#define CONFIG_REAL_POLE 0
#define CONFIG_COMPLEX_POLE 1
#define CONFIG_REAL_ZERO_RADIUS_ONE 2
#define CONFIG_REAL_ZERO 3
#define CONFIG_GENERIC_ZERO 4

#define STEP_RATIO_F_LO 20.0f
#define STEP_RATIO_F_HI 24000.0f
#define STEP_RATIO_EQ_NODE_COUNT_MAX 33

#define FILTER_ELEMENT_FORMAT_POLE_ZERO 0
#define FILTER_ELEMENT_FORMAT_FIR 1

#ifndef M_PI
#define M_PI (3.14159265358979323846)
#endif

WORD32 impd_derive_subband_center_freq(WORD32 eq_subband_gain_count,
                                       WORD32 eq_subband_gain_format,
                                       FLOAT32 sample_rate,
                                       FLOAT32* subband_center_freq) {
  WORD32 i;
  FLOAT32 width, offset;
  switch (eq_subband_gain_format) {
    case GAINFORMAT_QMF32:
    case GAINFORMAT_QMF64:
    case GAINFORMAT_QMF128:
    case GAINFORMAT_UNIFORM:
      width = 0.5f * sample_rate / (FLOAT32)eq_subband_gain_count;
      offset = 0.5f * width;
      for (i = 0; i < eq_subband_gain_count; i++) {
        subband_center_freq[i] = offset;
        offset = offset + width;
      }
      break;
    case GAINFORMAT_QMFHYBRID39:
    case GAINFORMAT_QMFHYBRID71:
    case GAINFORMAT_QMFHYBRID135:
      return (UNEXPECTED_ERROR);
      break;
    default:
      break;
  }
  return (0);
}

VOID impd_calc_fir_filt_response(WORD32 fir_order, WORD32 fir_symmetry,
                                 FLOAT32* coeff, FLOAT32 frequency_radian,
                                 FLOAT32* response) {
  WORD32 m;
  FLOAT32 sum = 0.0f;
  WORD32 o2;

  if ((fir_order & 0x1) == 0) {
    o2 = fir_order / 2;
    if (fir_symmetry == 0) {
      /*ITTIAM: sum is been over written after the loop
                None of the conformance streams with us entering this function*/
      for (m = 1; m <= o2; m++) {
        sum += coeff[o2 - m] * (FLOAT32)cos(m * frequency_radian);
      }
      sum += sum;
      sum = coeff[o2];
    } else {
      for (m = 1; m <= o2; m++) {
        sum += coeff[o2 - m] * (FLOAT32)sin(m * frequency_radian);
      }
      sum += sum;
    }
  } else {
    o2 = (fir_order + 1) / 2;
    if (fir_symmetry == 0) {
      for (m = 1; m <= o2; m++) {
        sum += coeff[o2 - m] * (FLOAT32)cos((m - 0.5f) * frequency_radian);
      }
    } else {
      for (m = 1; m <= o2; m++) {
        sum += coeff[o2 - m] * (FLOAT32)sin((m - 0.5f) * frequency_radian);
      }
    }
    sum += sum;
  }
  *response = sum;
  return;
}

VOID impd_calc_filt_ele_response(ia_unique_td_filt_element* element,
                                 FLOAT32 frequency_radian, FLOAT32* response) {
  WORD32 i;
  FLOAT32 part_response, radius, angle_radian;
  FLOAT64 total_response = 1.0;

  if (element->eq_filter_format == FILTER_ELEMENT_FORMAT_POLE_ZERO) {
    for (i = 0; i < element->bs_real_zero_radius_one_count; i++) {
      part_response =
          1.0f + 1.0f -
          2.0f * 1.0f *
              (FLOAT32)cos(frequency_radian - (FLOAT32)element->zero_sign[i]);
      total_response *= part_response;
    }
    for (i = 0; i < element->real_zero_count; i++) {
      if (element->real_zero_radius[i] < 0.0f) {
        radius = -element->real_zero_radius[i];
        angle_radian = (FLOAT32)M_PI;
      } else {
        radius = element->real_zero_radius[i];
        angle_radian = 0.0f;
      }
      part_response =
          1.0f + radius * radius -
          2.0f * radius * (FLOAT32)cos(frequency_radian - angle_radian);
      total_response *= part_response;
      part_response =
          1.0f + radius * radius -
          2.0f * radius * (FLOAT32)cos(frequency_radian - angle_radian);
      total_response *= part_response;
    }

    total_response = sqrt(total_response);

    for (i = 0; i < element->generic_zero_count; i++) {
      radius = element->generic_zero_radius[i];
      part_response =
          1.0f + radius * radius -
          2.0f * radius *
              (FLOAT32)cos(frequency_radian - element->generic_zero_angle[i]);
      total_response *= part_response;
      part_response =
          1.0f + radius * radius -
          2.0f * radius *
              (FLOAT32)cos(frequency_radian - element->generic_zero_angle[i]);
      total_response *= part_response;
    }
    for (i = 0; i < element->real_pole_count; i++) {
      if (element->real_pole_radius[i] < 0.0f) {
        radius = -element->real_pole_radius[i];
        angle_radian = (FLOAT32)(-M_PI);
      } else {
        radius = element->real_pole_radius[i];
        angle_radian = 0.0f;
      }
      part_response =
          1 / (1.0f + radius * radius -
               2.0f * radius * (FLOAT32)cos(frequency_radian - angle_radian));
      total_response *= part_response;
    }
    for (i = 0; i < element->cmplx_pole_count; i++) {
      part_response =
          1 /
          (1.0f + element->real_pole_radius[i] * element->real_pole_radius[i] -
           2.0f * element->real_pole_radius[i] *
               (FLOAT32)cos(frequency_radian - element->complex_pole_angle[i]));
      total_response *= part_response * part_response;
    }
  } else {
    impd_calc_fir_filt_response(element->fir_filt_order, element->fir_symmetry,
                                element->fir_coeff, frequency_radian,
                                &part_response);

    total_response *= part_response;
  }
  *response = (FLOAT32)total_response;
  return;
}

VOID impd_calc_filt_block_response(
    ia_unique_td_filt_element* unique_td_filt_ele,
    ia_filt_block_struct* str_filter_block, FLOAT32 frequency_radian,
    FLOAT32* response) {
  WORD32 i;
  FLOAT32 part_response;
  FLOAT64 total_response = 1.0;
  for (i = 0; i < str_filter_block->filter_element_count; i++) {
    ia_filt_ele_struct* str_filter_element =
        &str_filter_block->str_filter_element[i];

    impd_calc_filt_ele_response(
        &(unique_td_filt_ele[str_filter_element->filt_ele_idx]),
        frequency_radian, &part_response);
    total_response *= part_response;

    if (str_filter_element->filt_ele_gain_flag == 1) {
      total_response *= pow(10.0f, 0.05f * str_filter_element->filt_ele_gain);
    }
  }
  *response = (FLOAT32)total_response;
  return;
}

WORD32 impd_calc_subband_gains_td_cascade(
    ia_unique_td_filt_element* unique_td_filt_ele,
    ia_filt_block_struct* str_filter_block,
    ia_td_filter_cascade_struct* str_td_filter_cascade,
    WORD32 eq_subband_gain_format, WORD32 eq_ch_group_count,
    FLOAT32 sample_rate, WORD32 eq_frame_size_subband,
    ia_subband_filt_struct* subband_filt) {
  WORD32 i, err = 0, g, b;
  FLOAT32 response, frequency_radian;
  FLOAT32 subband_center_freq[256];
  FLOAT64 total_response;

  WORD32 eq_subband_gain_count = subband_filt->coeff_count;

  err = impd_derive_subband_center_freq(eq_subband_gain_count,
                                        eq_subband_gain_format, sample_rate,
                                        subband_center_freq);
  if (err) return (err);

  for (g = 0; g < eq_ch_group_count; g++) {
    for (b = 0; b < eq_subband_gain_count; b++) {
      total_response =
          pow(10.0f, 0.05f * str_td_filter_cascade->eq_cascade_gain[g]);
      frequency_radian =
          (FLOAT32)(2.0f * M_PI * subband_center_freq[b] / sample_rate);
      for (i = 0;
           i <
           str_td_filter_cascade->str_filter_block_refs[g].filter_block_count;
           i++) {
        impd_calc_filt_block_response(
            unique_td_filt_ele,
            &(str_filter_block[str_td_filter_cascade->str_filter_block_refs[g]
                                   .filter_block_index[i]]),
            frequency_radian, &response);
        total_response *= response;
      }
      subband_filt[g].subband_coeff[b] = (FLOAT32)total_response;
    }
    subband_filt[g].eq_frame_size_subband = eq_frame_size_subband;
  }
  return (0);
}

VOID impd_add_cascade(ia_cascade_align_group_struct* pstr_cascade_align_grp,
                      WORD32 c1, WORD32 c2, WORD32* done) {
  WORD32 m, n;

  *done = 0;
  for (m = 0; m < pstr_cascade_align_grp->member_count; m++) {
    if (pstr_cascade_align_grp->member_idx[m] == c1) {
      for (n = 0; n < pstr_cascade_align_grp->member_count; n++) {
        if (pstr_cascade_align_grp->member_idx[n] == c2) {
          *done = 1;
        }
      }
      if (*done == 0) {
        pstr_cascade_align_grp
            ->member_idx[pstr_cascade_align_grp->member_count] = c2;
        pstr_cascade_align_grp->member_count++;
        *done = 1;
      }
    }
  }
  return;
}

VOID impd_calc_cascade_align_groups(
    WORD32 eq_ch_group_count, WORD32 eq_phase_alignment_present,
    WORD32 eq_phase_alignment[][EQ_CHANNEL_GROUP_COUNT_MAX],
    WORD32* cascade_align_grp_cnt,
    ia_cascade_align_group_struct* pstr_cascade_align_grp) {
  WORD32 i, k, g, group_count, done;

  group_count = 0;

  if (eq_phase_alignment_present == 0) {
    if (eq_ch_group_count > 1) {
      for (i = 0; i < eq_ch_group_count; i++) {
        pstr_cascade_align_grp[group_count].member_idx[i] = i;
      }
      pstr_cascade_align_grp[group_count].member_count = eq_ch_group_count;
      group_count = 1;
    }
  } else {
    for (i = 0; i < eq_ch_group_count; i++) {
      for (k = i + 1; k < eq_ch_group_count; k++) {
        if (eq_phase_alignment[i][k] == 1) {
          done = 0;
          for (g = 0; g < group_count; g++) {
            impd_add_cascade(&pstr_cascade_align_grp[g], i, k, &done);

            if (done == 0) {
              impd_add_cascade(&pstr_cascade_align_grp[g], k, i, &done);
            }
          }
          if (done == 0) {
            pstr_cascade_align_grp[group_count].member_idx[0] = i;
            pstr_cascade_align_grp[group_count].member_idx[1] = k;
            pstr_cascade_align_grp[group_count].member_count = 2;
            group_count++;
          }
        }
      }
    }
  }
  *cascade_align_grp_cnt = group_count;
  return;
}

VOID impd_calc_phase_filt_params(
    WORD32 config, FLOAT32 radius, FLOAT32 angle,
    ia_ph_alignment_filt_struct* ph_alignment_filt) {
  WORD32 channel;
  FLOAT32 zReal, zImag;
  FLOAT32 prod;
  WORD32 section = ph_alignment_filt->section_count;
  ia_filt_sect_struct* filt_section = &ph_alignment_filt->filt_section[section];
  switch (config) {
    case CONFIG_REAL_POLE:
      ph_alignment_filt->gain *= (-radius);
      filt_section->a1 = -radius;
      filt_section->a2 = 0.0f;
      filt_section->b1 = -1.0f / radius;
      filt_section->b2 = 0.0f;
      ph_alignment_filt->section_count++;
      break;
    case CONFIG_COMPLEX_POLE:
      zReal = radius * (FLOAT32)cos(M_PI * angle);
      zImag = radius * (FLOAT32)sin(M_PI * angle);
      prod = zReal * zReal + zImag * zImag;
      ph_alignment_filt->gain *= prod;
      filt_section->a1 = -2.0f * zReal;
      filt_section->a2 = prod;
      filt_section->b1 = -2.0f * zReal / prod;
      filt_section->b2 = 1.0f / prod;
      ph_alignment_filt->section_count++;
      break;
    default:
      break;
  }
  for (channel = 0; channel < EQ_CHANNEL_COUNT_MAX; channel++) {
    filt_section->filt_sect_state[channel].in_state_1 = 0.0f;
    filt_section->filt_sect_state[channel].in_state_2 = 0.0f;
    filt_section->filt_sect_state[channel].out_state_1 = 0.0f;
    filt_section->filt_sect_state[channel].out_state_2 = 0.0f;
  }

  return;
}

VOID impd_calc_phase_filt_delay(
    ia_unique_td_filt_element* element,
    ia_ph_alignment_filt_struct* ph_alignment_filt) {
  WORD32 i, delay = 0, channel;
  if (element->eq_filter_format == FILTER_ELEMENT_FORMAT_POLE_ZERO) {
    if (element->bs_real_zero_radius_one_count == 0) {
      delay = element->real_zero_count + 2 * element->generic_zero_count -
              element->real_pole_count - 2 * element->cmplx_pole_count;
      delay = max(0, delay);
      ph_alignment_filt->validity_flag = 1;
    }
  }
  ph_alignment_filt->audio_delay.delay = delay;
  for (channel = 0; channel < EQ_CHANNEL_COUNT_MAX; channel++) {
    for (i = 0; i < delay; i++) {
      ph_alignment_filt->audio_delay.state[channel][i] = 0.0f;
    }
  }

  return;
}

VOID impd_calc_phase_filt(ia_unique_td_filt_element* element,
                          WORD32 filt_ele_idx,
                          ia_matching_ph_filt_struct* matching_ph_filt) {
  WORD32 i;

  memset(matching_ph_filt, 0, sizeof(ia_matching_ph_filt_struct));
  matching_ph_filt->gain = 1.0f;

  if (element->eq_filter_format == FILTER_ELEMENT_FORMAT_POLE_ZERO) {
    for (i = 0; i < element->real_pole_count; i++) {
      impd_calc_phase_filt_params(CONFIG_REAL_POLE,
                                  element->real_pole_radius[i], 0.0f,
                                  matching_ph_filt);
    }
    for (i = 0; i < element->cmplx_pole_count; i++) {
      impd_calc_phase_filt_params(
          CONFIG_COMPLEX_POLE, element->complex_pole_radius[i],
          element->complex_pole_angle[i], matching_ph_filt);
    }
  }
  impd_calc_phase_filt_delay(element, matching_ph_filt);

  matching_ph_filt->num_matches_filter = 1;
  matching_ph_filt->matches_filter[0] = filt_ele_idx;

  return;
}

WORD32 impd_calc_filt_params(ia_unique_td_filt_element* element,
                             ia_interm_filt_params_struct* interm_filt_params) {
  FLOAT32 zReal;
  FLOAT32* coeff;
  // WORD32   offset_idx = 0;
  WORD32 i;
  WORD32 param_idx = 0;

  ia_2nd_order_filt_params_struct* pstr_2nd_oder_filt_params =
      &interm_filt_params->ord_2_filt_params_of_zeros[0];

  for (i = 0; i < element->bs_real_zero_radius_one_count; i += 2) {
    FLOAT32 radius = (FLOAT32)element->zero_sign[i + 0];
    FLOAT32 angle = (FLOAT32)element->zero_sign[i + 1];
    FLOAT32 angle1 = radius;
    FLOAT32 angle2 = angle;
    pstr_2nd_oder_filt_params->radius = 1.0f;
    coeff = pstr_2nd_oder_filt_params->coeff;

    if (angle1 != angle2) {
      coeff[0] = 0.0f;
      coeff[1] = -1.0f;
    } else if (angle1 == 1.0f) {
      coeff[0] = -2.0f;
      coeff[1] = 1.0f;
    } else {
      coeff[0] = 2.0f;
      coeff[1] = 1.0f;
    }
    pstr_2nd_oder_filt_params += 1;
    param_idx += 1;
  }
  for (i = 0; i < element->real_zero_count; i++) {
    FLOAT32 radius = element->real_zero_radius[i];
    // FLOAT32  angle  = 0.0f;

    pstr_2nd_oder_filt_params->radius = radius;
    if (fabs(radius) == 1.0f) {
      return (-1);
    } else {
      coeff = pstr_2nd_oder_filt_params->coeff;
      coeff[0] = -(radius + 1.0f / radius);
      coeff[1] = 1.0f;
    }
    pstr_2nd_oder_filt_params += 1;
    param_idx += 1;
  }

  for (i = 0; i < element->generic_zero_count; i++) {
    FLOAT32 radius = element->generic_zero_radius[i];
    FLOAT32 angle = element->generic_zero_angle[i];
    zReal = radius * (FLOAT32)cos(M_PI * angle);
    pstr_2nd_oder_filt_params->radius = radius;
    coeff = pstr_2nd_oder_filt_params->coeff;
    coeff[0] = -2.0f * zReal;
    coeff[1] = radius * radius;

    pstr_2nd_oder_filt_params += 1;

    zReal = (FLOAT32)cos(M_PI * angle) / radius;
    pstr_2nd_oder_filt_params->radius = radius;
    coeff = pstr_2nd_oder_filt_params->coeff;
    coeff[0] = -2.0f * zReal;
    coeff[1] = 1.0f / (radius * radius);

    pstr_2nd_oder_filt_params += 1;

    param_idx += 2;
  }

  interm_filt_params->filter_param_count_of_zeros = param_idx;
  param_idx = 0;

  pstr_2nd_oder_filt_params =
      &interm_filt_params->ord_2_filt_params_of_poles[0];

  for (i = 0; i < element->real_pole_count; i++) {
    FLOAT32 radius = element->real_pole_radius[i];
    pstr_2nd_oder_filt_params->radius = radius;
    coeff = pstr_2nd_oder_filt_params->coeff;
    coeff[0] = -2.0f * radius;
    coeff[1] = radius * radius;
    param_idx += 1;
    pstr_2nd_oder_filt_params += 1;
  }

  for (i = 0; i < element->cmplx_pole_count; i++) {
    FLOAT32 radius = element->complex_pole_radius[i];
    FLOAT32 angle = element->complex_pole_angle[i];

    zReal = radius * (FLOAT32)cos(M_PI * angle);
    pstr_2nd_oder_filt_params->radius = radius;
    coeff = pstr_2nd_oder_filt_params->coeff;
    coeff[0] = -2.0f * zReal;
    coeff[1] = radius * radius;

    pstr_2nd_oder_filt_params += 1;

    pstr_2nd_oder_filt_params->radius = radius;
    pstr_2nd_oder_filt_params->coeff[0] = coeff[0];
    pstr_2nd_oder_filt_params->coeff[1] = coeff[1];

    pstr_2nd_oder_filt_params += 1;
    param_idx += 2;
  }
  interm_filt_params->filter_param_count_of_poles = param_idx;
  return 0;
}

VOID impd_convert_fir_filt_params(WORD32 fir_filt_order, WORD32 fir_symmetry,
                                  FLOAT32* fir_coeff,
                                  ia_fir_filter_struct* fir_filter) {
  WORD32 i, channel;
  FLOAT32* coeff = fir_filter->coeff;

  fir_filter->coeff_count = fir_filt_order + 1;
  for (i = 0; i < fir_filt_order / 2 + 1; i++) {
    coeff[i] = fir_coeff[i];
  }

  if (fir_symmetry == 1) {
    for (i = 0; i < (fir_filt_order + 1) / 2; i++) {
      coeff[fir_filt_order - i] = -coeff[i];
    }

    if ((fir_filt_order & 1) == 0) {
      coeff[fir_filt_order / 2] = 0.0f;
    }
  } else {
    for (i = 0; i < (fir_filt_order + 1) / 2; i++) {
      coeff[fir_filt_order - i] = coeff[i];
    }
  }

  for (channel = 0; channel < EQ_CHANNEL_COUNT_MAX; channel++) {
    for (i = 0; i < fir_filt_order + 1; i++) {
      fir_filter->state[channel][i] = 0.0f;
    }
  }
  return;
}

WORD32 impd_calc_filt_params_all(
    ia_unique_td_filt_element* element,
    ia_interm_filt_params_struct* interm_filt_params) {
  WORD32 err = 0;

  interm_filt_params->filter_format = element->eq_filter_format;
  if (element->eq_filter_format == FILTER_ELEMENT_FORMAT_POLE_ZERO) {
    err = impd_calc_filt_params(element, interm_filt_params);
    if (err) return err;
  } else {
    interm_filt_params->filter_param_count_of_zeros = 0;
    interm_filt_params->filter_param_count_of_poles = 0;

    impd_convert_fir_filt_params(element->fir_filt_order, element->fir_symmetry,
                                 element->fir_coeff,
                                 &interm_filt_params->fir_filter);
  }
  return (0);
}

VOID impd_calc_eq_filt_elements(
    ia_interm_filt_params_struct* interm_filt_params,
    ia_eq_filt_ele_struct* eq_filt_element) {
  WORD32 i, poles_idx, zeros_idx, pole_order = 0, section, channel;
  WORD32 poles_over[REAL_POLE_COUNT_MAX + COMPLEX_POLE_COUNT_MAX];
  WORD32 zeros_over[REAL_ZERO_COUNT_MAX + COMPLEX_ZERO_COUNT_MAX];
  FLOAT32 max_radius, diff_radius;
  WORD32 coeff_count;
  FLOAT32* coeff;

  for (i = 0; i < REAL_POLE_COUNT_MAX + COMPLEX_POLE_COUNT_MAX; i++) {
    poles_over[i] = 0;
  }
  for (i = 0; i < REAL_ZERO_COUNT_MAX + COMPLEX_ZERO_COUNT_MAX; i++) {
    zeros_over[i] = 0;
  }
  section = 0;
  do {
    max_radius = -1.0;
    poles_idx = -1;
    for (i = 0; i < interm_filt_params->filter_param_count_of_poles; i++) {
      if (poles_over[i] == 0) {
        if (interm_filt_params->filter_format == 0) {
          if (max_radius <
              fabs(interm_filt_params->ord_2_filt_params_of_poles[i].radius)) {
            max_radius = (FLOAT32)fabs(
                interm_filt_params->ord_2_filt_params_of_poles[i].radius);
            poles_idx = i;
            if (interm_filt_params->ord_2_filt_params_of_poles[i].coeff[1] !=
                0.0f) {
              pole_order = 2;
            } else {
              pole_order = 1;
            }
          }
        }
      }
    }
    if (poles_idx >= 0) {
      diff_radius = 10.0f;
      zeros_idx = -1;
      for (i = 0; i < interm_filt_params->filter_param_count_of_zeros; i++) {
        if (zeros_over[i] == 0) {
          if (interm_filt_params->filter_format == 0) {
            if (pole_order == 2) {
              if (interm_filt_params->ord_2_filt_params_of_zeros[i].coeff[1] !=
                  0.0f) {
                if (diff_radius >
                    fabs(fabs(interm_filt_params->ord_2_filt_params_of_zeros[i]
                                  .radius) -
                         max_radius)) {
                  diff_radius = (FLOAT32)fabs(
                      fabs(interm_filt_params->ord_2_filt_params_of_zeros[i]
                               .radius) -
                      max_radius);
                  zeros_idx = i;
                }
              }
            } else {
              if (interm_filt_params->ord_2_filt_params_of_zeros[i].coeff[1] ==
                  0.0f) {
                if (diff_radius >
                    (FLOAT32)(fabs(
                        fabs(interm_filt_params->ord_2_filt_params_of_zeros[i]
                                 .radius) -
                        max_radius))) {
                  diff_radius = (FLOAT32)(fabs(
                      fabs(interm_filt_params->ord_2_filt_params_of_zeros[i]
                               .radius) -
                      max_radius));
                  zeros_idx = i;
                }
              }
            }
          }
        }
      }
      if (zeros_idx == -1) {
        for (i = 0; i < interm_filt_params->filter_param_count_of_zeros; i++) {
          if (zeros_over[i] == 0) {
            if (interm_filt_params->filter_format == 0) {
              if (pole_order == 2) {
                if (interm_filt_params->ord_2_filt_params_of_zeros[i]
                        .coeff[1] == 0.0f) {
                  if (diff_radius >
                      (FLOAT32)(fabs(
                          fabs(interm_filt_params->ord_2_filt_params_of_zeros[i]
                                   .radius) -
                          max_radius))) {
                    diff_radius = (FLOAT32)(fabs(
                        fabs(interm_filt_params->ord_2_filt_params_of_zeros[i]
                                 .radius) -
                        max_radius));
                    zeros_idx = i;
                  }
                }
              } else {
                if (interm_filt_params->ord_2_filt_params_of_zeros[i]
                        .coeff[1] != 0.0f) {
                  if (diff_radius >
                      (FLOAT32)(fabs(
                          fabs(interm_filt_params->ord_2_filt_params_of_zeros[i]
                                   .radius) -
                          max_radius))) {
                    diff_radius = (FLOAT32)(fabs(
                        fabs(interm_filt_params->ord_2_filt_params_of_zeros[i]
                                 .radius) -
                        max_radius));
                    zeros_idx = i;
                  }
                }
              }
            }
          }
        }
      }
      eq_filt_element->pstr_pole_zero_filt.filt_section[section].a1 =
          interm_filt_params->ord_2_filt_params_of_poles[poles_idx].coeff[0];
      eq_filt_element->pstr_pole_zero_filt.filt_section[section].a2 =
          interm_filt_params->ord_2_filt_params_of_poles[poles_idx].coeff[1];
      if (zeros_idx >= 0) {
        eq_filt_element->pstr_pole_zero_filt.filt_section[section].b1 =
            interm_filt_params->ord_2_filt_params_of_zeros[zeros_idx].coeff[0];
        eq_filt_element->pstr_pole_zero_filt.filt_section[section].b2 =
            interm_filt_params->ord_2_filt_params_of_zeros[zeros_idx].coeff[1];
      } else {
        eq_filt_element->pstr_pole_zero_filt.filt_section[section].b1 = 0.0f;
        eq_filt_element->pstr_pole_zero_filt.filt_section[section].b2 = 0.0f;
        eq_filt_element->pstr_pole_zero_filt.audio_delay.delay++;
      }
      for (channel = 0; channel < EQ_CHANNEL_COUNT_MAX; channel++) {
        eq_filt_element->pstr_pole_zero_filt.filt_section[section]
            .filt_sect_state[channel]
            .in_state_1 = 0.0f;
        eq_filt_element->pstr_pole_zero_filt.filt_section[section]
            .filt_sect_state[channel]
            .in_state_2 = 0.0f;
        eq_filt_element->pstr_pole_zero_filt.filt_section[section]
            .filt_sect_state[channel]
            .out_state_1 = 0.0f;
        eq_filt_element->pstr_pole_zero_filt.filt_section[section]
            .filt_sect_state[channel]
            .out_state_2 = 0.0f;
      }
      if (zeros_idx >= 0) zeros_over[zeros_idx] = 1;
      if (poles_idx >= 0) poles_over[poles_idx] = 1;
      section++;
    }
  } while (poles_idx >= 0);

  eq_filt_element->pstr_pole_zero_filt.section_count = section;

  coeff_count = 1;
  coeff = eq_filt_element->pstr_pole_zero_filt.fir_filter.coeff;
  coeff[0] = 1.0f;
  for (i = 0; i < interm_filt_params->filter_param_count_of_zeros; i++) {
    if (zeros_over[i] == 0) {
      if (interm_filt_params->filter_format == 0) {
        WORD32 k;
        FLOAT32 b1, b2;
        b1 = interm_filt_params->ord_2_filt_params_of_zeros[i].coeff[0];
        b2 = interm_filt_params->ord_2_filt_params_of_zeros[i].coeff[1];

        coeff_count += 2;
        k = coeff_count - 1;
        coeff[k] = b2 * coeff[k - 2];
        k--;
        if (k > 1) {
          coeff[k] = b1 * coeff[k - 1] + b2 * coeff[k - 2];
          k--;
          for (; k > 1; k--) {
            coeff[k] += b1 * coeff[k - 1] + b2 * coeff[k - 2];
          }
          coeff[1] += b1 * coeff[0];
        } else {
          coeff[1] = b1 * coeff[0];
        }
      }
    }
    zeros_over[i] = 1;
  }
  if (coeff_count > 1) {
    eq_filt_element->pstr_pole_zero_filt.filt_coeffs_flag = 1;
    eq_filt_element->pstr_pole_zero_filt.fir_filter.coeff_count = coeff_count;
  } else {
    eq_filt_element->pstr_pole_zero_filt.filt_coeffs_flag = 0;
    eq_filt_element->pstr_pole_zero_filt.fir_filter.coeff_count = 0;
  }

  return;
}

WORD32 impd_calc_filt_block(ia_unique_td_filt_element* unique_td_filt_ele,
                            ia_filt_block_struct* str_filter_block,
                            ia_eq_filt_block_struct* pstr_eq_filt_block) {
  WORD32 i, k, err;
  ia_interm_filt_params_struct interm_filt_params;
  ia_matching_ph_filt_struct matching_ph_filt[FILTER_ELEMENT_COUNT_MAX];

  for (i = 0; i < str_filter_block->filter_element_count; i++) {
    if ((unique_td_filt_ele[str_filter_block->str_filter_element[i]
                                .filt_ele_idx]
             .eq_filter_format == FILTER_ELEMENT_FORMAT_FIR) &&
        (str_filter_block->filter_element_count > 1)) {
      return (-1);
    }
  }
  for (i = 0; i < str_filter_block->filter_element_count; i++) {
    ia_eq_filt_ele_struct* eq_filt_element =
        &pstr_eq_filt_block->eq_filt_element[i];
    ia_filt_ele_struct* str_filter_element =
        &str_filter_block->str_filter_element[i];
    WORD32 filterIndex = str_filter_element->filt_ele_idx;

    if (unique_td_filt_ele[filterIndex].eq_filter_format ==
        FILTER_ELEMENT_FORMAT_POLE_ZERO) {
      err = impd_calc_filt_params_all(&(unique_td_filt_ele[filterIndex]),
                                      &interm_filt_params);
      if (err) return (err);

      impd_calc_eq_filt_elements(&interm_filt_params, eq_filt_element);

      eq_filt_element->format = FILTER_ELEMENT_FORMAT_POLE_ZERO;
    } else {
      impd_convert_fir_filt_params(
          unique_td_filt_ele[filterIndex].fir_filt_order,
          unique_td_filt_ele[filterIndex].fir_symmetry,
          unique_td_filt_ele[filterIndex].fir_coeff,
          &eq_filt_element->fir_filter);

      eq_filt_element->format = FILTER_ELEMENT_FORMAT_FIR;
    }
    if (str_filter_element->filt_ele_gain_flag == 1) {
      eq_filt_element->elementGainLinear =
          (FLOAT32)(pow(10.0f, 0.05f * str_filter_element->filt_ele_gain));
    } else {
      eq_filt_element->elementGainLinear = 1.0f;
    }
    for (k = 0; k < unique_td_filt_ele[filterIndex].real_zero_count; k++) {
      if (unique_td_filt_ele[filterIndex].real_zero_radius[k] > 0.0f) {
        eq_filt_element->elementGainLinear =
            -eq_filt_element->elementGainLinear;
      }
    }
    impd_calc_phase_filt(&(unique_td_filt_ele[filterIndex]), i,
                         &matching_ph_filt[i]);
  }
  pstr_eq_filt_block->element_count = str_filter_block->filter_element_count;

  pstr_eq_filt_block->matching_ph_filt_ele_0 = matching_ph_filt[0];

  return (0);
}

VOID impd_calc_cascade_phase_align_filt(
    ia_td_filter_cascade_struct* str_td_filter_cascade, WORD32 ch_group_cnt) {
  // WORD32 err = 0;
  WORD32 cascade_align_grp_cnt = 0;
  ia_cascade_align_group_struct
      pstr_cascade_align_grp[EQ_CHANNEL_GROUP_COUNT_MAX / 2];

  impd_calc_cascade_align_groups(
      ch_group_cnt, str_td_filter_cascade->eq_phase_alignment_present,
      str_td_filter_cascade->eq_phase_alignment, &cascade_align_grp_cnt,
      pstr_cascade_align_grp);
  return;
}

WORD32 impd_calc_filt_cascade(
    ia_unique_td_filt_element* unique_td_filt_ele,
    ia_filt_block_struct* str_filter_block,
    ia_td_filter_cascade_struct* str_td_filter_cascade, WORD32 ch_group_cnt,
    ia_filt_cascade_td_struct filt_cascade_td[]) {
  WORD32 i, err, g;

  for (g = 0; g < ch_group_cnt; g++) {
    for (i = 0;
         i < str_td_filter_cascade->str_filter_block_refs[g].filter_block_count;
         i++) {
      err = impd_calc_filt_block(
          unique_td_filt_ele,
          &(str_filter_block[str_td_filter_cascade->str_filter_block_refs[g]
                                 .filter_block_index[i]]),
          &(filt_cascade_td[g].pstr_eq_filt_block[i]));
      if (err) return (err);
    }
    filt_cascade_td[g].block_count = i;
    filt_cascade_td[g].cascade_gain_linear = (FLOAT32)(
        pow(10.0f, 0.05f * str_td_filter_cascade->eq_cascade_gain[g]));
  }

  impd_calc_cascade_phase_align_filt(str_td_filter_cascade, ch_group_cnt);
  return (0);
}

VOID impd_calc_subband_eq(ia_eq_subband_gain_vector* str_eq_subband_gain_vector,
                          WORD32 eq_subband_gain_count,
                          ia_subband_filt_struct* subband_filt) {
  WORD32 i;

  for (i = 0; i < eq_subband_gain_count; i++) {
    subband_filt->subband_coeff[i] =
        str_eq_subband_gain_vector->eq_subband_gain[i];
  }
  subband_filt->coeff_count = eq_subband_gain_count;
  return;
}

FLOAT32 impd_decode_eq_node_freq(WORD32 eq_node_freq_idx) {
  /*((FLOAT32)((log10(STEP_RATIO_F_HI) / log10(STEP_RATIO_F_LO) - 1.0f) /
   * (STEP_RATIO_EQ_NODE_COUNT_MAX - 1.0f)))*/
  FLOAT32 step_ratio = 0.0739601809794f;
  return (
      (FLOAT32)(pow(STEP_RATIO_F_LO, 1.0f + eq_node_freq_idx * step_ratio)));
}

FLOAT32 impd_calc_warp_freq_delta(FLOAT32 fsubband, FLOAT32 node_freq,
                                  WORD32 eq_node_freq_idx) {
  /*((FLOAT32)((log10(STEP_RATIO_F_HI) / log10(STEP_RATIO_F_LO) - 1.0f) /
   * (STEP_RATIO_EQ_NODE_COUNT_MAX - 1.0f)))*/
  FLOAT32 step_ratio = 0.0739601809794f;
  return ((FLOAT32)((log10(fsubband) / log10(node_freq) - 1.0f) / step_ratio -
                    (FLOAT32)eq_node_freq_idx));
}

VOID impd_interpolate_eq_gain(WORD32 band_step, FLOAT32 left_gain,
                              FLOAT32 right_gain, FLOAT32 left_slope,
                              FLOAT32 right_slope, FLOAT32 f,
                              FLOAT32* interpolated_gain) {
  FLOAT32 k1, k2, a, b, c, d;
  FLOAT32 inv_band_step = (FLOAT32)(1.0 / (FLOAT32)band_step);
  FLOAT32 inv_band_step_sqr = inv_band_step * inv_band_step;
  k1 = (right_gain - left_gain) * inv_band_step_sqr;
  left_slope = (FLOAT32)(left_slope / 3.128f);
  right_slope = (FLOAT32)(right_slope / 3.128f);

  k2 = right_slope + left_slope;
  a = inv_band_step * (inv_band_step * k2 - 2.0f * k1);
  b = 3.0f * k1 - inv_band_step * (k2 + left_slope);
  c = left_slope;
  d = left_gain;
  *interpolated_gain = (((a * f + b) * f + c) * f) + d;
  return;
}

WORD32 impd_interpolate_subband_spline(
    ia_eq_subband_gain_spline_struct* str_eq_subband_gain_spline,
    WORD32 eq_subband_gain_count, WORD32 eq_subband_gain_format,
    FLOAT32 sample_rate, ia_subband_filt_struct* subband_filt) {
  WORD32 b, n, err;

  FLOAT32 eq_gain[32];
  WORD32 eq_node_freq_idx[32];
  FLOAT32 eq_node_freq[32];
  FLOAT32 subband_center_freq[256];
  WORD32 num_eq_nodes = str_eq_subband_gain_spline->num_eq_nodes;

  FLOAT32* eq_slope = str_eq_subband_gain_spline->eq_slope;
  WORD32* eq_freq_delta = str_eq_subband_gain_spline->eq_freq_delta;
  FLOAT32 eq_gain_initial = str_eq_subband_gain_spline->eq_gain_initial;
  FLOAT32* eq_gain_delta = str_eq_subband_gain_spline->eq_gain_delta;

  FLOAT32* subband_coeff = subband_filt->subband_coeff;
  WORD32 max_eq_node_idx = 32;

  eq_gain[0] = eq_gain_initial;
  eq_node_freq_idx[0] = 0;
  eq_node_freq[0] = impd_decode_eq_node_freq(eq_node_freq_idx[0]);
  for (n = 1; n < num_eq_nodes; n++) {
    eq_gain[n] = eq_gain[n - 1] + eq_gain_delta[n];
    eq_node_freq_idx[n] = eq_node_freq_idx[n - 1] + eq_freq_delta[n];
    eq_node_freq[n] = impd_decode_eq_node_freq(eq_node_freq_idx[n]);
  }
  if ((eq_node_freq[num_eq_nodes - 1] < sample_rate * 0.5f) &&
      (eq_node_freq_idx[num_eq_nodes - 1] < max_eq_node_idx)) {
    eq_slope[num_eq_nodes] = 0;
    eq_gain[num_eq_nodes] = eq_gain[num_eq_nodes - 1];
    eq_freq_delta[num_eq_nodes] =
        max_eq_node_idx - eq_node_freq_idx[num_eq_nodes - 1];
    eq_node_freq_idx[num_eq_nodes] = max_eq_node_idx;
    eq_node_freq[num_eq_nodes] =
        impd_decode_eq_node_freq(eq_node_freq_idx[num_eq_nodes]);
    num_eq_nodes += 1;
  }

  err = impd_derive_subband_center_freq(eq_subband_gain_count,
                                        eq_subband_gain_format, sample_rate,
                                        subband_center_freq);
  if (err) return (err);

  for (n = 0; n < num_eq_nodes - 1; n++) {
    for (b = 0; b < eq_subband_gain_count; b++) {
      FLOAT32 fSub;
      fSub = max(subband_center_freq[b], eq_node_freq[0]);
      fSub = min(fSub, eq_node_freq[num_eq_nodes - 1]);
      if ((fSub >= eq_node_freq[n]) && (fSub <= eq_node_freq[n + 1])) {
        FLOAT32 warpedDeltaFreq = impd_calc_warp_freq_delta(
            fSub, eq_node_freq[0], eq_node_freq_idx[n]);
        FLOAT32 gEqSubbandDb;
        impd_interpolate_eq_gain(eq_freq_delta[n + 1], eq_gain[n],
                                 eq_gain[n + 1], eq_slope[n], eq_slope[n + 1],
                                 warpedDeltaFreq, &gEqSubbandDb);

        subband_coeff[b] = (FLOAT32)pow(2.0, gEqSubbandDb / 6.0f);
      }
    }
  }
  subband_filt->coeff_count = eq_subband_gain_count;
  return (0);
}

WORD32 impd_calc_subband_gains(ia_eq_coeff_struct* str_eq_coeff,
                               WORD32 eq_ch_group_count,
                               WORD32* subband_gains_index, FLOAT32 sample_rate,
                               WORD32 eq_frame_size_subband,
                               ia_subband_filt_struct* subband_filt) {
  WORD32 g, err;
  WORD32 eq_subband_gain_representation =
      str_eq_coeff->eq_subband_gain_representation;
  WORD32 eq_subband_gain_count = str_eq_coeff->eq_subband_gain_count;
  WORD32 eq_subband_gain_format = str_eq_coeff->eq_subband_gain_format;

  for (g = 0; g < eq_ch_group_count; g++) {
    if (eq_subband_gain_representation == 1) {
      err = impd_interpolate_subband_spline(
          &(str_eq_coeff->str_eq_subband_gain_spline[subband_gains_index[g]]),
          eq_subband_gain_count, eq_subband_gain_format, sample_rate,
          &(subband_filt[g]));
      if (err) return (err);
    } else {
      impd_calc_subband_eq(
          &(str_eq_coeff->str_eq_subband_gain_vector[subband_gains_index[g]]),
          eq_subband_gain_count, &(subband_filt[g]));
    }
    subband_filt[g].eq_frame_size_subband = eq_frame_size_subband;
  }
  return (0);
}

VOID impd_calc_filt_sect_delay(WORD32 section_count,
                               ia_filt_sect_struct* filt_section,
                               FLOAT32* delay) {
  WORD32 i;
  FLOAT32 d = 0.0f;
  for (i = 0; i < section_count; i++) {
    if (filt_section[i].b2 != 0.0f) {
      d += 1.0f;
    } else if (filt_section[i].b1 != 0.0f) {
      d += 0.5f;
    }
  }
  *delay = d;
  return;
}

VOID impd_get_eq_set_delay(ia_eq_set_struct* eq_set, WORD32* cascade_delay) {
  FLOAT32 delay, sect_delay;
  WORD32 k, g, b;

  delay = 0;
  g = eq_set->eq_ch_group_of_channel[0];
  if (g >= 0) {
    switch (eq_set->domain) {
      case EQ_FILTER_DOMAIN_TIME: {
        ia_filt_cascade_td_struct* filt_cascade_td =
            &eq_set->filt_cascade_td[g];
        for (b = 0; b < filt_cascade_td->block_count; b++) {
          ia_eq_filt_ele_struct* eq_filt_element =
              &filt_cascade_td->pstr_eq_filt_block[b].eq_filt_element[0];
          switch (eq_filt_element->format) {
            case FILTER_ELEMENT_FORMAT_POLE_ZERO:
              impd_calc_filt_sect_delay(
                  eq_filt_element->pstr_pole_zero_filt.section_count,
                  eq_filt_element->pstr_pole_zero_filt.filt_section,
                  &sect_delay);
              delay += sect_delay;
              if (eq_filt_element->pstr_pole_zero_filt.filt_coeffs_flag) {
                delay += 0.5f * (eq_filt_element->pstr_pole_zero_filt.fir_filter
                                     .coeff_count -
                                 1);
              }
              break;
            case FILTER_ELEMENT_FORMAT_FIR:
              delay += 0.5f * (eq_filt_element->fir_filter.coeff_count - 1);
              break;
            default:
              break;
          }
          for (k = 0; k < eq_filt_element->num_ph_align_filt; k++) {
            ia_ph_alignment_filt_struct* ph_alignment_filt =
                &eq_filt_element->ph_alignment_filt[k];
            impd_calc_filt_sect_delay(ph_alignment_filt->section_count,
                                      ph_alignment_filt->filt_section,
                                      &sect_delay);
            delay += sect_delay;
          }
        }
        for (b = 0; b < filt_cascade_td->num_ph_align_filt; b++) {
          ia_ph_alignment_filt_struct* ph_alignment_filt =
              &filt_cascade_td->ph_alignment_filt[b];
          impd_calc_filt_sect_delay(ph_alignment_filt->section_count,
                                    ph_alignment_filt->filt_section,
                                    &sect_delay);
          delay += sect_delay;
        }
      } break;
      case EQ_FILTER_DOMAIN_SUBBAND:
      case EQ_FILTER_DOMAIN_NONE:
      default:
        break;
    }
  }
  *cascade_delay = (WORD32)delay;
  return;
}

WORD32 impd_derive_eq_set(ia_eq_coeff_struct* str_eq_coeff,
                          ia_eq_instructions_struct* str_eq_instructions,
                          FLOAT32 sample_rate, WORD32 drc_frame_size,
                          WORD32 sub_band_domain_mode,
                          ia_eq_set_struct* eq_set) {
  WORD32 err, i, eq_frame_size_subband;

  eq_set->domain = EQ_FILTER_DOMAIN_NONE;

  if (sub_band_domain_mode == SUBBAND_DOMAIN_MODE_OFF) {
    if (str_eq_instructions->td_filter_cascade_present == 1) {
      err = impd_calc_filt_cascade(
          str_eq_coeff->unique_td_filt_ele, str_eq_coeff->str_filter_block,
          &str_eq_instructions->str_td_filter_cascade,
          str_eq_instructions->eq_ch_group_count, eq_set->filt_cascade_td);
      if (err) return (err);
    }

    eq_set->domain |= EQ_FILTER_DOMAIN_TIME;
  }
  if (sub_band_domain_mode != SUBBAND_DOMAIN_MODE_OFF) {
    switch (sub_band_domain_mode) {
      case SUBBAND_DOMAIN_MODE_QMF64:
        if (str_eq_coeff->eq_subband_gain_count !=
            AUDIO_CODEC_SUBBAND_COUNT_QMF64) {
          return (-1);
        }
        eq_frame_size_subband =
            drc_frame_size / AUDIO_CODEC_SUBBAND_DOWNSAMPLING_FACTOR_QMF64;
        break;
      case SUBBAND_DOMAIN_MODE_QMF71:
        if (str_eq_coeff->eq_subband_gain_count !=
            AUDIO_CODEC_SUBBAND_COUNT_QMF71) {
          return (-1);
        }
        eq_frame_size_subband =
            drc_frame_size / AUDIO_CODEC_SUBBAND_DOWNSAMPLING_FACTOR_QMF71;
        break;
      case SUBBAND_DOMAIN_MODE_STFT256:
        if (str_eq_coeff->eq_subband_gain_count !=
            AUDIO_CODEC_SUBBAND_COUNT_STFT256) {
          return (-1);
        }
        eq_frame_size_subband =
            drc_frame_size / AUDIO_CODEC_SUBBAND_DOWNSAMPLING_FACTOR_STFT256;
        break;
      default:
        return (-1);
        break;
    }
    if (str_eq_instructions->subband_gains_present == 1) {
      err = impd_calc_subband_gains(
          str_eq_coeff, str_eq_instructions->eq_ch_group_count,
          str_eq_instructions->subband_gains_index, sample_rate,
          eq_frame_size_subband, eq_set->subband_filt);
      if (err) return (err);
    } else {
      if (str_eq_instructions->td_filter_cascade_present == 1) {
        err = impd_calc_subband_gains_td_cascade(
            str_eq_coeff->unique_td_filt_ele, str_eq_coeff->str_filter_block,
            &str_eq_instructions->str_td_filter_cascade,
            str_eq_coeff->eq_subband_gain_format,
            str_eq_instructions->eq_ch_group_count, sample_rate,
            eq_frame_size_subband, eq_set->subband_filt);
        if (err) return (err);
      }
    }
    eq_set->domain |= EQ_FILTER_DOMAIN_SUBBAND;
  }
  eq_set->audio_num_chan = str_eq_instructions->eq_channel_count;
  eq_set->eq_ch_group_count = str_eq_instructions->eq_ch_group_count;

  for (i = 0; i < str_eq_instructions->eq_channel_count; i++) {
    eq_set->eq_ch_group_of_channel[i] =
        str_eq_instructions->eq_ch_group_of_channel[i];
  }

  return (0);
}

VOID impd_process_filt_sect(
    ia_filt_sect_struct filt_section[EQ_FILTER_SECTION_COUNT_MAX],
    WORD32 channel, FLOAT32* audio_out, WORD32 section_count) {
  WORD32 i;

  for (i = 0; i < section_count; i++) {
    ia_filt_sect_state_struct* filt_sect_state =
        &filt_section[i].filt_sect_state[channel];
    FLOAT32 audio_in = *audio_out;
    *audio_out = audio_in + filt_section[i].b1 * filt_sect_state->in_state_1 +
                 filt_section[i].b2 * filt_sect_state->in_state_2 -
                 filt_section[i].a1 * filt_sect_state->out_state_1 -
                 filt_section[i].a2 * filt_sect_state->out_state_2;

    filt_sect_state->in_state_2 = filt_sect_state->in_state_1;
    filt_sect_state->in_state_1 = audio_in;
    filt_sect_state->out_state_2 = filt_sect_state->out_state_1;
    filt_sect_state->out_state_1 = *audio_out;
  }
  return;
}

VOID impd_fir_filt_process(ia_fir_filter_struct* fir_filter, WORD32 channel,
                           FLOAT32 audio_in, FLOAT32* audio_out) {
  WORD32 i;
  FLOAT32* coeff = fir_filter->coeff;
  FLOAT32* state = fir_filter->state[channel];
  FLOAT32 sum;
  sum = coeff[0] * audio_in;
  for (i = 1; i < fir_filter->coeff_count; i++) {
    sum += coeff[i] * state[i - 1];
  }
  *audio_out = sum;
  for (i = fir_filter->coeff_count - 2; i > 0; i--) {
    state[i] = state[i - 1];
  }
  state[0] = audio_in;
  return;
}

VOID impd_audio_delay_process(ia_audio_delay_struct* audio_delay,
                              WORD32 channel, FLOAT32 audio_in,
                              FLOAT32* ptr_audio_out) {
  WORD32 i;
  FLOAT32* state = audio_delay->state[channel];
  if (audio_delay->delay > 0) {
    *ptr_audio_out = state[audio_delay->delay - 1];
    for (i = audio_delay->delay - 1; i > 0; i--) {
      state[i] = state[i - 1];
    }
    state[0] = audio_in;
  } else {
    *ptr_audio_out = audio_in;
  }
  return;
}

VOID impd_pole_zero_filt_process(ia_pole_zero_filt_struct* pstr_pole_zero_filt,
                                 WORD32 channel, FLOAT32 audio_in,
                                 FLOAT32* ptr_audio_out) {
  FLOAT32 inp = audio_in;
  FLOAT32 out = inp;

  impd_process_filt_sect(pstr_pole_zero_filt->filt_section, channel, &out,
                         pstr_pole_zero_filt->section_count);
  inp = out;

  if (pstr_pole_zero_filt->filt_coeffs_flag == 1) {
    impd_fir_filt_process(&pstr_pole_zero_filt->fir_filter, channel, inp, &out);
    inp = out;
  }
  impd_audio_delay_process(&pstr_pole_zero_filt->audio_delay, channel, inp,
                           &out);

  *ptr_audio_out = out;
  return;
}

VOID impd_phase_align_filt_process(
    ia_ph_alignment_filt_struct* ph_alignment_filt, WORD32 channel,
    FLOAT32* ptr_audio_out) {
  FLOAT32 audio_in = *ptr_audio_out;
  FLOAT32 inp = audio_in;
  FLOAT32 out = inp;

  impd_process_filt_sect(ph_alignment_filt->filt_section, channel, &out,
                         ph_alignment_filt->section_count);
  inp = out;

  impd_audio_delay_process(&ph_alignment_filt->audio_delay, channel, inp, &out);

  *ptr_audio_out = out * ph_alignment_filt->gain;
  return;
}

VOID impd_eq_filt_element_process(
    ia_eq_filt_block_struct str_eq_filt_block[EQ_FILTER_BLOCK_COUNT_MAX],
    WORD32 channel, FLOAT32 audio_in, FLOAT32* ptr_audio_out,
    WORD32 block_count) {
  WORD32 i;
  FLOAT32 inp = audio_in;
  FLOAT32 out = inp;
  WORD32 k, j;
  WORD32 element_count;
  for (j = 0; j < block_count; j++) {
    FLOAT32 sum = 0.0f;
    element_count = str_eq_filt_block[j].element_count;
    for (k = 0; k < element_count; k++) {
      switch (str_eq_filt_block[j].eq_filt_element[k].format) {
        case FILTER_ELEMENT_FORMAT_POLE_ZERO:
          impd_pole_zero_filt_process(
              &str_eq_filt_block[j].eq_filt_element[k].pstr_pole_zero_filt,
              channel, inp, &out);
          break;
        case FILTER_ELEMENT_FORMAT_FIR:
          impd_fir_filt_process(
              &str_eq_filt_block[j].eq_filt_element[k].fir_filter, channel, inp,
              &out);
          break;
        default:
          break;
      }
      out *= str_eq_filt_block[j].eq_filt_element[k].elementGainLinear;

      for (i = 0; i < str_eq_filt_block[j].eq_filt_element[k].num_ph_align_filt;
           i++) {
        inp = out;
        impd_phase_align_filt_process(
            &str_eq_filt_block[j].eq_filt_element[k].ph_alignment_filt[i],
            channel, &out);
      }
      sum += out;
    }
    inp = sum;
  }
  *ptr_audio_out = inp;
  return;
}

WORD32 impd_process_eq_set_time_domain(ia_eq_set_struct* pstr_eq_set,
                                       WORD32 channel, FLOAT32* ptr_audio_in,
                                       FLOAT32* ptr_audio_out,
                                       WORD32 frame_size) {
  WORD32 i, j, g = 0;

  if (pstr_eq_set == NULL) return 0;

  g = pstr_eq_set->eq_ch_group_of_channel[channel];

  if (g < 0) return 0;

  for (i = 0; i < frame_size; i++) {
    impd_eq_filt_element_process(
        (pstr_eq_set->filt_cascade_td[g].pstr_eq_filt_block), channel,
        ptr_audio_in[i], &ptr_audio_out[i],
        pstr_eq_set->filt_cascade_td[g].block_count);

    for (j = 0; j < pstr_eq_set->filt_cascade_td[g].num_ph_align_filt; j++) {
      impd_phase_align_filt_process(
          &pstr_eq_set->filt_cascade_td[g].ph_alignment_filt[j], channel,
          &ptr_audio_out[i]);
    }

    ptr_audio_out[i] =
        ptr_audio_out[i] * pstr_eq_set->filt_cascade_td[g].cascade_gain_linear;
  }
  return 0;
}

