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
#include "impd_parametric_drc_dec.h"
#include "impd_drc_filter_bank.h"
#include "impd_drc_multi_band.h"
#include "impd_drc_gain_dec.h"
#include "impd_drc_process_audio.h"
#include "impd_drc_interface.h"
#include "impd_drc_gain_dec.h"
#include "impd_drc_eq.h"
#include "impd_drc_gain_decoder.h"
#include "impd_drc_rom.h"

VOID impd_shape_filt_block_adapt(const FLOAT32 drc_gain,
                                 shape_filter_block* shape_filter_block) {
  //    WORD32 err = 0;
  WORD32 i;
  FLOAT32 warped_gain = 0, x1 = 0, y1;
  shape_filter_block->drc_gain_last = drc_gain;
  for (i = 0; i < 4; i++) {
    if (shape_filter_block->shape_filter[i].type == SHAPE_FILTER_TYPE_OFF)
      continue;
    else if (shape_filter_block->shape_filter[i].type ==
                 SHAPE_FILTER_TYPE_LF_CUT ||
             shape_filter_block->shape_filter[i].type ==
                 SHAPE_FILTER_TYPE_HF_CUT) {
      if (drc_gain < 1.0f)
        warped_gain = -1.0f;
      else
        warped_gain =
            (drc_gain - 1.0f) /
            (drc_gain - 1.0f + shape_filter_block->shape_filter[i].gain_offset);
      x1 = shape_filter_block->shape_filter[i].a1;
    } else if (shape_filter_block->shape_filter[i].type ==
                   SHAPE_FILTER_TYPE_LF_BOOST ||
               shape_filter_block->shape_filter[i].type ==
                   SHAPE_FILTER_TYPE_HF_BOOST) {
      if (drc_gain >= 1.0f)
        warped_gain = -1.0f;
      else
        warped_gain =
            (1.0f - drc_gain) /
            (1.0f +
             drc_gain *
                 (shape_filter_block->shape_filter[i].gain_offset - 1.0f));
      x1 = shape_filter_block->shape_filter[i].b1;
    }

    if (warped_gain <= 0.0f) {
      y1 = x1;
    } else if (warped_gain <
               shape_filter_block->shape_filter[i].warped_gain_max) {
      y1 = x1 + shape_filter_block->shape_filter[i].factor * warped_gain;
    } else {
      y1 = shape_filter_block->shape_filter[i].y1_bound;
    }
    if (shape_filter_block->shape_filter[i].type == SHAPE_FILTER_TYPE_LF_CUT) {
      shape_filter_block->shape_filter[i].b1 = y1;
    } else if (shape_filter_block->shape_filter[i].type ==
               SHAPE_FILTER_TYPE_HF_CUT) {
      shape_filter_block->shape_filter[i].g_norm =
          shape_filter_block->shape_filter[i].coeff_sum /
          (shape_filter_block->shape_filter[i].partial_coeff_sum + y1);
      shape_filter_block->shape_filter[i].b1 = y1;
    } else if (shape_filter_block->shape_filter[i].type ==
               SHAPE_FILTER_TYPE_HF_BOOST) {
      shape_filter_block->shape_filter[i].g_norm =
          (shape_filter_block->shape_filter[i].partial_coeff_sum + y1) /
          shape_filter_block->shape_filter[i].coeff_sum;
      shape_filter_block->shape_filter[i].a1 = y1;
    } else if (shape_filter_block->shape_filter[i].type ==
               SHAPE_FILTER_TYPE_LF_BOOST) {
      shape_filter_block->shape_filter[i].a1 = y1;
    }
  }
  return;
}

VOID resetshape_flter_block(shape_filter_block* shape_filter_block) {
  WORD32 i, c;
  shape_filter_block->drc_gain_last = -1.0f;
  impd_shape_filt_block_adapt(1.0f, shape_filter_block);
  for (i = 0; i < 4; i++) {
    for (c = 0; c < MAX_CHANNEL_COUNT; c++) {
      shape_filter_block->shape_filter[i].audio_in_state_1[c] = 0.0f;
      shape_filter_block->shape_filter[i].audio_in_state_2[c] = 0.0f;
      shape_filter_block->shape_filter[i].audio_out_state_1[c] = 0.0f;
      shape_filter_block->shape_filter[i].audio_out_state_2[c] = 0.0f;
    }
  }
  return;
}

VOID impd_shape_filt_block_init(
    ia_shape_filter_block_params_struct* shape_flter_block_params,
    shape_filter_block* shape_filter_block) {
  // WORD32 err = 0;
  FLOAT32 x1;
  FLOAT32 x2 = 0.0f;
  FLOAT32 radius;
  if (shape_flter_block_params->lf_cut_filter_present) {
    ia_shape_filter_params_struct* params =
        &shape_flter_block_params->str_lf_cut_params;
    shape_filter_block->shape_filter[0].type = SHAPE_FILTER_TYPE_LF_CUT;
    shape_filter_block->shape_filter[0].gain_offset =
        shape_filt_lf_gain_offset_tbl[params->corner_freq_index]
                                     [params->filter_strength_index];
    shape_filter_block->shape_filter[0].y1_bound =
        shape_filt_lf_y1_bound_tbl[params->corner_freq_index]
                                  [params->filter_strength_index];
    x1 = -shape_filt_lf_radius_tbl[params->corner_freq_index];
    shape_filter_block->shape_filter[0].warped_gain_max =
        SHAPE_FILTER_DRC_GAIN_MAX_MINUS_ONE /
        (SHAPE_FILTER_DRC_GAIN_MAX_MINUS_ONE +
         shape_filter_block->shape_filter[0].gain_offset);
    shape_filter_block->shape_filter[0].factor =
        (shape_filter_block->shape_filter[0].y1_bound - x1) /
        shape_filter_block->shape_filter[0].warped_gain_max;
    shape_filter_block->shape_filter[0].a1 = x1;

  } else {
    shape_filter_block->shape_filter[0].type = SHAPE_FILTER_TYPE_OFF;
  }
  if (shape_flter_block_params->lf_boost_filter_present) {
    ia_shape_filter_params_struct* params =
        &shape_flter_block_params->str_lf_boost_params;
    shape_filter_block->shape_filter[1].type = SHAPE_FILTER_TYPE_LF_BOOST;
    shape_filter_block->shape_filter[1].gain_offset =
        shape_filt_lf_gain_offset_tbl[params->corner_freq_index]
                                     [params->filter_strength_index];
    shape_filter_block->shape_filter[1].y1_bound =
        shape_filt_lf_y1_bound_tbl[params->corner_freq_index]
                                  [params->filter_strength_index];
    x1 = -shape_filt_lf_radius_tbl[params->corner_freq_index];
    shape_filter_block->shape_filter[1].warped_gain_max =
        SHAPE_FILTER_DRC_GAIN_MAX_MINUS_ONE /
        (SHAPE_FILTER_DRC_GAIN_MAX_MINUS_ONE +
         shape_filter_block->shape_filter[1].gain_offset);
    shape_filter_block->shape_filter[1].factor =
        (shape_filter_block->shape_filter[1].y1_bound - x1) /
        shape_filter_block->shape_filter[1].warped_gain_max;
    shape_filter_block->shape_filter[1].b1 = x1;

  } else {
    shape_filter_block->shape_filter[1].type = SHAPE_FILTER_TYPE_OFF;
  }
  if (shape_flter_block_params->hf_cut_filter_present) {
    ia_shape_filter_params_struct* params =
        &shape_flter_block_params->str_hfCutParams;
    shape_filter_block->shape_filter[2].type = SHAPE_FILTER_TYPE_HF_CUT;
    shape_filter_block->shape_filter[2].gain_offset =
        shape_filt_hf_gain_offset_tbl[params->corner_freq_index]
                                     [params->filter_strength_index];
    shape_filter_block->shape_filter[2].y1_bound =
        shape_filt_hf_y1_bound_tbl[params->corner_freq_index]
                                  [params->filter_strength_index];
    radius = shape_filt_hf_radius_tbl[params->corner_freq_index];
    x1 = (FLOAT32)(
        -2.0f * radius *
        cos(2.0f * M_PI *
            shape_filt_cutoff_freq_norm_hf_tbl[params->corner_freq_index]));
    x2 = radius * radius;
    shape_filter_block->shape_filter[2].warped_gain_max =
        SHAPE_FILTER_DRC_GAIN_MAX_MINUS_ONE /
        (SHAPE_FILTER_DRC_GAIN_MAX_MINUS_ONE +
         shape_filter_block->shape_filter[2].gain_offset);
    shape_filter_block->shape_filter[2].factor =
        (shape_filter_block->shape_filter[2].y1_bound - x1) /
        shape_filter_block->shape_filter[2].warped_gain_max;
    shape_filter_block->shape_filter[2].coeff_sum = 1.0f + x1 + x2;
    shape_filter_block->shape_filter[2].partial_coeff_sum = 1.0f + x2;
    shape_filter_block->shape_filter[2].a1 = x1;
    shape_filter_block->shape_filter[2].a2 = x2;
    shape_filter_block->shape_filter[2].b2 = x2;
  } else {
    shape_filter_block->shape_filter[2].type = SHAPE_FILTER_TYPE_OFF;
  }
  if (shape_flter_block_params->hf_boost_filter_present) {
    ia_shape_filter_params_struct* params =
        &shape_flter_block_params->str_hf_boost_params;
    shape_filter_block->shape_filter[3].type = SHAPE_FILTER_TYPE_HF_BOOST;
    shape_filter_block->shape_filter[3].gain_offset =
        shape_filt_hf_gain_offset_tbl[params->corner_freq_index]
                                     [params->filter_strength_index];
    shape_filter_block->shape_filter[3].y1_bound =
        shape_filt_hf_y1_bound_tbl[params->corner_freq_index]
                                  [params->filter_strength_index];
    radius = shape_filt_hf_radius_tbl[params->corner_freq_index];
    x1 = (FLOAT32)(
        -2.0f * radius *
        cos(2.0f * M_PI *
            shape_filt_cutoff_freq_norm_hf_tbl[params->corner_freq_index]));
    x2 = radius * radius;
    shape_filter_block->shape_filter[3].warped_gain_max =
        SHAPE_FILTER_DRC_GAIN_MAX_MINUS_ONE /
        (SHAPE_FILTER_DRC_GAIN_MAX_MINUS_ONE +
         shape_filter_block->shape_filter[3].gain_offset);
    shape_filter_block->shape_filter[3].factor =
        (shape_filter_block->shape_filter[3].y1_bound - x1) /
        shape_filter_block->shape_filter[3].warped_gain_max;
    shape_filter_block->shape_filter[3].coeff_sum = 1.0f + x1 + x2;
    shape_filter_block->shape_filter[3].partial_coeff_sum = 1.0f + x2;
    shape_filter_block->shape_filter[3].b1 = x1;
    shape_filter_block->shape_filter[3].b2 = x2;
    shape_filter_block->shape_filter[3].a2 = x2;

  } else {
    shape_filter_block->shape_filter[3].type = SHAPE_FILTER_TYPE_OFF;
  }
  resetshape_flter_block(shape_filter_block);
  shape_filter_block->shape_flter_block_flag = 1;
  return;
}

VOID impd_shape_filt_block_time_process(shape_filter_block* shape_filter_block,
                                        FLOAT32* drc_gain, const WORD32 channel,
                                        FLOAT32* audio_in, WORD32 start,
                                        WORD32 end) {
  WORD32 i, j;
  FLOAT32 audio_out;

  if (shape_filter_block->shape_flter_block_flag) {
    for (i = start; i < end; i++) {
      FLOAT32 tmp = audio_in[i];
      for (j = 0; j < 4; j++) {
        if (shape_filter_block->shape_filter[j].type ==
                SHAPE_FILTER_TYPE_LF_CUT ||
            shape_filter_block->shape_filter[j].type ==
                SHAPE_FILTER_TYPE_LF_BOOST) {
          audio_out = tmp +
                      shape_filter_block->shape_filter[j].b1 *
                          shape_filter_block->shape_filter[j]
                              .audio_in_state_1[channel] -
                      shape_filter_block->shape_filter[j].a1 *
                          shape_filter_block->shape_filter[j]
                              .audio_out_state_1[channel];
          shape_filter_block->shape_filter[j].audio_in_state_1[channel] = tmp;
          shape_filter_block->shape_filter[j].audio_out_state_1[channel] =
              audio_out;

        } else if (shape_filter_block->shape_filter[j].type ==
                       SHAPE_FILTER_TYPE_HF_CUT ||
                   shape_filter_block->shape_filter[j].type ==
                       SHAPE_FILTER_TYPE_HF_BOOST) {
          audio_out = shape_filter_block->shape_filter[j].g_norm * tmp +
                      shape_filter_block->shape_filter[j].b1 *
                          shape_filter_block->shape_filter[j]
                              .audio_in_state_1[channel] +
                      shape_filter_block->shape_filter[j].b2 *
                          shape_filter_block->shape_filter[j]
                              .audio_in_state_2[channel] -
                      shape_filter_block->shape_filter[j].a1 *
                          shape_filter_block->shape_filter[j]
                              .audio_out_state_1[channel] -
                      shape_filter_block->shape_filter[j].a2 *
                          shape_filter_block->shape_filter[j]
                              .audio_out_state_2[channel];
          shape_filter_block->shape_filter[j].audio_in_state_2[channel] =
              shape_filter_block->shape_filter[j].audio_in_state_1[channel];
          shape_filter_block->shape_filter[j].audio_in_state_1[channel] =
              shape_filter_block->shape_filter[j].g_norm * tmp;
          shape_filter_block->shape_filter[j].audio_out_state_2[channel] =
              shape_filter_block->shape_filter[j].audio_out_state_1[channel];
          shape_filter_block->shape_filter[j].audio_out_state_1[channel] =
              audio_out;
        } else {
          audio_out = tmp;
        }
        tmp = audio_out;
      }

      audio_in[i] = audio_out * drc_gain[i];
    }

  } else {
    for (i = start; i < end; i++) {
      audio_in[i] = audio_in[i] * drc_gain[i];
    }
  }

  return;
}
