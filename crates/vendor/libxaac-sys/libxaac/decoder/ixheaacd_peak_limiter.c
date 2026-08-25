/******************************************************************************
 *                                                                            *
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
#include <stdlib.h>
#include <math.h>
#include "ixheaac_type_def.h"
#include "ixheaacd_cnst.h"
#include "ixheaacd_peak_limiter_struct_def.h"
#include "ixheaac_constants.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops16.h"

#define MAX(x, y) ((x) > (y) ? (x) : (y))
#define MIN(x, y) ((x) > (y) ? (y) : (x))

/**
*  ixheaacd_peak_limiter_init
*
*  \brief Peak Limiter initialization
*
*  \param [in/out] peak_limiter Pointer to peak_limiter struct
*  \param [in] num_channels Number of ouptut channels
*  \param [in] sample_rate Sampling rate value
*  \param [in] buffer Peak limiter buffer of size PEAK_LIM_BUFFER_SIZE
*
*  \return WORD32
*
*/
WORD32 ixheaacd_peak_limiter_init(ia_peak_limiter_struct *peak_limiter,
                                  UWORD32 num_channels, UWORD32 sample_rate,
                                  FLOAT32 *buffer, UWORD32 *delay_in_samples) {
  UWORD32 attack;

  attack = (UWORD32)(DEFAULT_ATTACK_TIME_MS * sample_rate / 1000);
  *delay_in_samples = attack;

  if (attack < 1) return 0;

  peak_limiter->max_buf = buffer;
  peak_limiter->max_idx = 0;
  peak_limiter->cir_buf_pnt = 0;
  peak_limiter->delayed_input = buffer + attack * 4 + 32;

  peak_limiter->delayed_input_index = 0;
  peak_limiter->attack_time = DEFAULT_ATTACK_TIME_MS;
  peak_limiter->release_time = DEFAULT_RELEASE_TIME_MS;
  peak_limiter->attack_time_samples = attack;
  peak_limiter->attack_constant = (FLOAT32)pow(0.1, 1.0 / (attack + 1));
  peak_limiter->release_constant = (FLOAT32)pow(
      0.1, 1.0 / (DEFAULT_RELEASE_TIME_MS * sample_rate / 1000 + 1));
  peak_limiter->num_channels = num_channels;
  peak_limiter->sample_rate = sample_rate;
  peak_limiter->min_gain = 1.0f;
  peak_limiter->limiter_on = 1;
  peak_limiter->pre_smoothed_gain = 1.0f;
  peak_limiter->gain_modified = 1.0f;

  return 0;
}
VOID ixheaacd_peak_limiter_process_float(ia_peak_limiter_struct *peak_limiter,
                                         FLOAT32 samples[MAX_NUM_CHANNELS][4096],
                                         UWORD32 frame_len) {
  UWORD32 i, j;
  FLOAT32 tmp, gain;
  FLOAT32 min_gain = 1.0f;
  FLOAT32 maximum;
  UWORD32 num_channels = peak_limiter->num_channels;
  UWORD32 attack_time_samples = peak_limiter->attack_time_samples;
  FLOAT32 attack_constant = peak_limiter->attack_constant;
  FLOAT32 release_constant = peak_limiter->release_constant;
  FLOAT32 *max_buf = peak_limiter->max_buf;
  FLOAT32 gain_modified = peak_limiter->gain_modified;
  FLOAT32 *delayed_input = peak_limiter->delayed_input;
  UWORD32 delayed_input_index = peak_limiter->delayed_input_index;
  FLOAT64 pre_smoothed_gain = peak_limiter->pre_smoothed_gain;
  FLOAT32 limit_threshold = PEAK_LIM_THR_FLOAT;

  if (peak_limiter->limiter_on || (FLOAT32)pre_smoothed_gain) {
    for (i = 0; i < frame_len; i++) {
      tmp = 0.0f;
      for (j = 0; j < num_channels; j++) {
        tmp = (FLOAT32)MAX(tmp, fabs(samples[j][i]));
      }
      max_buf[peak_limiter->cir_buf_pnt] = tmp;

      if (peak_limiter->max_idx == peak_limiter->cir_buf_pnt) {
        peak_limiter->max_idx = 0;
        for (j = 1; j < (attack_time_samples); j++) {
          if (max_buf[j] > max_buf[peak_limiter->max_idx]) peak_limiter->max_idx = j;
        }
      } else if (tmp >= max_buf[peak_limiter->max_idx]) {
        peak_limiter->max_idx = peak_limiter->cir_buf_pnt;
      }

      peak_limiter->cir_buf_pnt++;

      if (peak_limiter->cir_buf_pnt == (WORD32)(attack_time_samples))
        peak_limiter->cir_buf_pnt = 0;
      maximum = max_buf[peak_limiter->max_idx];

      if (maximum > limit_threshold) {
        gain = limit_threshold / maximum;
      } else {
        gain = 1;
      }

      if (gain < pre_smoothed_gain) {
        gain_modified =
            MIN(gain_modified, (gain - 0.1f * (FLOAT32)pre_smoothed_gain) * 1.11111111f);
      } else {
        gain_modified = gain;
      }

      if (gain_modified < pre_smoothed_gain) {
        pre_smoothed_gain = attack_constant * (pre_smoothed_gain - gain_modified) + gain_modified;
        pre_smoothed_gain = MAX(pre_smoothed_gain, gain);
      } else {
        pre_smoothed_gain =
            release_constant * (pre_smoothed_gain - gain_modified) + gain_modified;
      }

      gain = (FLOAT32)pre_smoothed_gain;

      for (j = 0; j < num_channels; j++) {
        tmp = delayed_input[delayed_input_index * num_channels + j];
        delayed_input[delayed_input_index * num_channels + j] = samples[j][i];

        tmp *= gain;

        if (tmp > limit_threshold)
          tmp = limit_threshold;
        else if (tmp < -limit_threshold)
          tmp = -limit_threshold;

        samples[j][i] = tmp;
      }

      delayed_input_index++;
      if (delayed_input_index >= attack_time_samples) delayed_input_index = 0;

      if (gain < min_gain) min_gain = gain;
    }
  } else {
    for (i = 0; i < frame_len; i++) {
      for (j = 0; j < num_channels; j++) {
        tmp = delayed_input[delayed_input_index * num_channels + j];
        delayed_input[delayed_input_index * num_channels + j] = samples[j][i];
        samples[j][i] = tmp;
      }

      delayed_input_index++;
      if (delayed_input_index >= attack_time_samples) delayed_input_index = 0;
    }
  }

  peak_limiter->gain_modified = gain_modified;
  peak_limiter->delayed_input_index = delayed_input_index;
  peak_limiter->pre_smoothed_gain = pre_smoothed_gain;
  peak_limiter->min_gain = min_gain;

  return;
}

/**
*  ixheaacd_peak_limiter_process
*
*  \brief Peak Limiter process
*
*  \param [in/out] peak_limiter
*  \param [in] samples
*  \param [in] frame_len
*
*  \return WORD32
*
*/
VOID ixheaacd_peak_limiter_process(ia_peak_limiter_struct *peak_limiter,
                                   VOID *samples_t, UWORD32 frame_len,
                                   UWORD8 *qshift_adj) {
  UWORD32 i, j;
  FLOAT32 tmp, gain;
  FLOAT32 min_gain = 1.0f;
  FLOAT32 maximum;
  UWORD32 num_channels = peak_limiter->num_channels;
  UWORD32 attack_time_samples = peak_limiter->attack_time_samples;
  FLOAT32 attack_constant = peak_limiter->attack_constant;
  FLOAT32 release_constant = peak_limiter->release_constant;
  FLOAT32 *max_buf = peak_limiter->max_buf;
  FLOAT32 gain_modified = peak_limiter->gain_modified;
  FLOAT32 *delayed_input = peak_limiter->delayed_input;
  UWORD32 delayed_input_index = peak_limiter->delayed_input_index;
  FLOAT64 pre_smoothed_gain = peak_limiter->pre_smoothed_gain;
  WORD32 limit_threshold = PEAK_LIM_THR_FIX;

  WORD32 *samples = (WORD32 *)samples_t;

  if (peak_limiter->limiter_on || (FLOAT32)pre_smoothed_gain) {
    for (i = 0; i < frame_len; i++) {
      tmp = 0.0f;
      for (j = 0; j < num_channels; j++) {
        FLOAT32 gain_t = (FLOAT32)(1 << *(qshift_adj + j));
        tmp = (FLOAT32)MAX(tmp, fabs((samples[i * num_channels + j] * gain_t)));
      }
      max_buf[peak_limiter->cir_buf_pnt] = tmp;

      if (peak_limiter->max_idx == peak_limiter->cir_buf_pnt) {
        peak_limiter->max_idx = 0;
        for (j = 1; j < (attack_time_samples); j++) {
          if (max_buf[j] > max_buf[peak_limiter->max_idx])
            peak_limiter->max_idx = j;
        }
      } else if (tmp >= max_buf[peak_limiter->max_idx]) {
        peak_limiter->max_idx = peak_limiter->cir_buf_pnt;
      }
      peak_limiter->cir_buf_pnt++;

      if (peak_limiter->cir_buf_pnt == (WORD32)(attack_time_samples))
        peak_limiter->cir_buf_pnt = 0;
      maximum = max_buf[peak_limiter->max_idx];

      if (maximum > limit_threshold) {
        gain = limit_threshold / maximum;
      } else {
        gain = 1;
      }

      if (gain < pre_smoothed_gain) {
        gain_modified =
            MIN(gain_modified,
                (gain - 0.1f * (FLOAT32)pre_smoothed_gain) * 1.11111111f);

      } else {
        gain_modified = gain;
      }

      if (gain_modified < pre_smoothed_gain) {
        pre_smoothed_gain =
            attack_constant * (pre_smoothed_gain - gain_modified) +
            gain_modified;
        pre_smoothed_gain = MAX(pre_smoothed_gain, gain);
      } else {
        pre_smoothed_gain =
            release_constant * (pre_smoothed_gain - gain_modified) +
            gain_modified;
      }

      gain = (FLOAT32)pre_smoothed_gain;

      for (j = 0; j < num_channels; j++) {
        WORD64 tmp_fix;
        tmp = delayed_input[delayed_input_index * num_channels + j];
        FLOAT32 gain_t = (FLOAT32)(1 << *(qshift_adj + j));
        delayed_input[delayed_input_index * num_channels + j] =
            samples[i * num_channels + j] * gain_t;

        tmp *= gain;

        tmp_fix = (WORD64)tmp;

        if (tmp_fix > limit_threshold)
          tmp_fix = limit_threshold;
        else if (tmp_fix < -limit_threshold)
          tmp_fix = -limit_threshold;

        samples[i * num_channels + j] = (WORD32)tmp_fix;
      }

      delayed_input_index++;
      if (delayed_input_index >= attack_time_samples) delayed_input_index = 0;

      if (gain < min_gain) min_gain = gain;
    }
  } else {
    for (i = 0; i < frame_len; i++) {
      for (j = 0; j < num_channels; j++) {
        tmp = delayed_input[delayed_input_index * num_channels + j];
        FLOAT32 gain_t = (FLOAT32)(1 << *(qshift_adj + j));
        delayed_input[delayed_input_index * num_channels + j] =
            samples[i * num_channels + j] * gain_t;
        samples[i * num_channels + j] = (WORD32)tmp;
      }

      delayed_input_index++;
      if (delayed_input_index >= attack_time_samples) delayed_input_index = 0;
    }
  }

  peak_limiter->gain_modified = gain_modified;
  peak_limiter->delayed_input_index = delayed_input_index;
  peak_limiter->pre_smoothed_gain = pre_smoothed_gain;
  peak_limiter->min_gain = min_gain;

  return;
}

/**
 *  ixheaacd_scale_adjust
 *
 *  \brief Scale adjust process
 *
 *  \param [in/out] samples
 *  \param [in] qshift_adj
 *  \param [in] frame_len
 *
 *  \return WORD32
 *
 */

VOID ixheaacd_scale_adjust(WORD32 *samples, UWORD32 frame_len,
                           WORD8 *qshift_adj, WORD num_channels) {
  UWORD32 i;
  WORD32 j;
  for (i = 0; i < frame_len; i++) {
    for (j = 0; j < num_channels; j++) {
      samples[i * num_channels + j] = ixheaac_shl32_sat(samples[i * num_channels + j], qshift_adj[j]);
    }
  }
}