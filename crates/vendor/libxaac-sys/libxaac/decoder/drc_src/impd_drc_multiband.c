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
#include "impd_type_def.h"
#include "impd_error_standards.h"
#include "impd_drc_extr_delta_coded_info.h"
#include "impd_drc_common.h"
#include "impd_drc_struct.h"
#include "impd_drc_filter_bank.h"
#include "impd_drc_multi_band.h"
#include "impd_drc_rom.h"

VOID impd_fcenter_norm_sb_init(WORD32 num_subbands,
                               FLOAT32* fcenter_norm_subband) {
  WORD32 s;
  for (s = 0; s < num_subbands; s++) {
    fcenter_norm_subband[s] = (s + 0.5f) / (2.0f * num_subbands);
  }
  return;
}

VOID impd_generate_slope(WORD32 num_sub_bands, FLOAT32* fcenter_norm_subband,
                         FLOAT32 fcross_norm_lo, FLOAT32 fcross_norm_hi,
                         FLOAT32* response) {
  WORD32 i;
  FLOAT32 filter_slope = -24.0f;
  FLOAT32 inv_log10_2 = 3.32192809f;
  FLOAT32 norm = 0.05f * filter_slope * inv_log10_2;

  for (i = 0; i < num_sub_bands; i++) {
    if (fcenter_norm_subband[i] < fcross_norm_lo) {
      response[i] = (FLOAT32)pow(
          10.0, norm * log10(fcross_norm_lo / fcenter_norm_subband[i]));
    } else if (fcenter_norm_subband[i] < fcross_norm_hi) {
      response[i] = 1.0f;
    } else {
      response[i] = (FLOAT32)pow(
          10.0, norm * log10(fcenter_norm_subband[i] / fcross_norm_hi));
    }
  }
  return;
}

VOID impd_generate_overlap_weights(
    WORD32 num_drc_bands, WORD32 drc_band_type,
    ia_gain_params_struct* gain_params, WORD32 dec_subband_count,
    ia_group_overlap_params_struct* pstr_group_overlap_params) {
  FLOAT32 fcenter_norm_subband[AUDIO_CODEC_SUBBAND_COUNT_MAX];
  FLOAT32 w_norm[AUDIO_CODEC_SUBBAND_COUNT_MAX];
  FLOAT32 fcross_norm_lo, fcross_norm_hi;
  WORD32 b, s, start_subband_index = 0, stop_sub_band_index = 0;
  impd_fcenter_norm_sb_init(dec_subband_count, fcenter_norm_subband);

  if (drc_band_type == 1) {
    fcross_norm_lo = 0.0f;
    for (b = 0; b < num_drc_bands; b++) {
      if (b < num_drc_bands - 1) {
        fcross_norm_hi =
            normal_cross_freq[gain_params[b + 1].crossover_freq_idx]
                .f_cross_norm;
      } else {
        fcross_norm_hi = 0.5f;
      }
      impd_generate_slope(
          dec_subband_count, fcenter_norm_subband, fcross_norm_lo,
          fcross_norm_hi,
          pstr_group_overlap_params->str_band_overlap_params[b].overlap_weight);

      fcross_norm_lo = fcross_norm_hi;
    }
    for (s = 0; s < dec_subband_count; s++) {
      w_norm[s] = pstr_group_overlap_params->str_band_overlap_params[0]
                      .overlap_weight[s];
      for (b = 1; b < num_drc_bands; b++) {
        w_norm[s] += pstr_group_overlap_params->str_band_overlap_params[b]
                         .overlap_weight[s];
      }
    }

    for (s = 0; s < dec_subband_count; s++) {
      for (b = 0; b < num_drc_bands; b++) {
        pstr_group_overlap_params->str_band_overlap_params[b]
            .overlap_weight[s] /= w_norm[s];
      }
    }
  } else {
    start_subband_index = 0;
    for (b = 0; b < num_drc_bands; b++) {
      if (b < num_drc_bands - 1) {
        stop_sub_band_index = gain_params[b + 1].start_subband_index - 1;
      } else {
        stop_sub_band_index = dec_subband_count - 1;
      }
      for (s = 0; s < dec_subband_count; s++) {
        if (s >= start_subband_index && s <= stop_sub_band_index) {
          pstr_group_overlap_params->str_band_overlap_params[b]
              .overlap_weight[s] = 1.0;
        } else {
          pstr_group_overlap_params->str_band_overlap_params[b]
              .overlap_weight[s] = 0.0;
        }
      }
      start_subband_index = stop_sub_band_index + 1;
    }
  }

  return;
}

VOID impd_init_overlap_weight(
    ia_uni_drc_coeffs_struct* str_p_loc_drc_coefficients_uni_drc,
    ia_drc_instructions_struct* str_drc_instruction_str,
    WORD32 sub_band_domain_mode,
    ia_overlap_params_struct* pstr_overlap_params) {
  WORD32 g;
  WORD32 dec_subband_count = 0;
  switch (sub_band_domain_mode) {
    case SUBBAND_DOMAIN_MODE_QMF64:
      dec_subband_count = AUDIO_CODEC_SUBBAND_COUNT_QMF64;
      break;
    case SUBBAND_DOMAIN_MODE_QMF71:
      dec_subband_count = AUDIO_CODEC_SUBBAND_COUNT_QMF71;
      break;
    case SUBBAND_DOMAIN_MODE_STFT256:
      dec_subband_count = AUDIO_CODEC_SUBBAND_COUNT_STFT256;
      break;
  }

  for (g = 0; g < str_drc_instruction_str->num_drc_ch_groups; g++) {
    if (str_drc_instruction_str->band_count_of_ch_group[g] > 1) {
      impd_generate_overlap_weights(
          str_drc_instruction_str->band_count_of_ch_group[g],
          str_p_loc_drc_coefficients_uni_drc
              ->gain_set_params[str_drc_instruction_str
                                    ->gain_set_index_for_channel_group[g]]
              .drc_band_type,
          str_p_loc_drc_coefficients_uni_drc
              ->gain_set_params[str_drc_instruction_str
                                    ->gain_set_index_for_channel_group[g]]
              .gain_params,
          dec_subband_count,
          &(pstr_overlap_params->str_group_overlap_params[g]));
    }
  }

  return;
}
