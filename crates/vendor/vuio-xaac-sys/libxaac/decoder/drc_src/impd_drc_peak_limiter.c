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
#include <stdlib.h>
#include <math.h>
#include <stdio.h>
#include <string.h>

#include "impd_type_def.h"
#include "impd_drc_peak_limiter.h"

#ifndef max
#define max(a, b) (((a) > (b)) ? (a) : (b))
#endif
#ifndef min
#define min(a, b) (((a) < (b)) ? (a) : (b))
#endif

WORD32 impd_peak_limiter_init(ia_drc_peak_limiter_struct *peak_limiter,
                              FLOAT32 attack_time, FLOAT32 release_time,
                              FLOAT32 limit_threshold, UWORD32 num_channels,
                              UWORD32 sample_rate, FLOAT32 *buffer) {
  UWORD32 attack;
  attack = (UWORD32)(attack_time * sample_rate / 1000);

  if (attack < 1) return 0;

  peak_limiter->max_buf = buffer;
  peak_limiter->delayed_input = buffer + attack * 4 + 32;

  peak_limiter->delayed_input_index = 0;
  peak_limiter->attack_time = attack_time;
  peak_limiter->release_time = release_time;
  peak_limiter->attack_time_samples = attack;
  peak_limiter->attack_constant = (FLOAT32)pow(0.1, 1.0 / (attack + 1));
  peak_limiter->release_constant =
      (FLOAT32)pow(0.1, 1.0 / (release_time * sample_rate / 1000 + 1));
  peak_limiter->limit_threshold = limit_threshold;
  peak_limiter->num_channels = num_channels;
  peak_limiter->sample_rate = sample_rate;
  peak_limiter->min_gain = 1.0f;
  peak_limiter->limiter_on = 1;
  peak_limiter->pre_smoothed_gain = 1.0f;
  peak_limiter->gain_modified = 1.0f;

  return 0;
}

VOID impd_peak_limiter_reinit(ia_drc_peak_limiter_struct *peak_limiter) {
  if (peak_limiter) {
    peak_limiter->delayed_input_index = 0;
    peak_limiter->pre_smoothed_gain = 1.0f;
    peak_limiter->gain_modified = 1.0f;
    peak_limiter->min_gain = 1.0f;
    memset(peak_limiter->max_buf, 0,
           (peak_limiter->attack_time_samples + 1) * sizeof(FLOAT32));
    memset(peak_limiter->delayed_input, 0, peak_limiter->attack_time_samples *
                                               peak_limiter->num_channels *
                                               sizeof(FLOAT32));
  }

  return;
}

VOID impd_limiter_process(ia_drc_peak_limiter_struct *peak_limiter,
                          FLOAT32 *samples, UWORD32 frame_len) {
  UWORD32 i, j;
  FLOAT32 tmp, gain;
  FLOAT32 min_gain = 1;
  FLOAT32 maximum, sectionMaximum;
  UWORD32 num_channels = peak_limiter->num_channels;
  UWORD32 attack_time_samples = peak_limiter->attack_time_samples;
  FLOAT32 attack_constant = peak_limiter->attack_constant;
  FLOAT32 release_constant = peak_limiter->release_constant;
  FLOAT32 limit_threshold = peak_limiter->limit_threshold;
  FLOAT32 *max_buf = peak_limiter->max_buf;
  FLOAT32 gain_modified = peak_limiter->gain_modified;
  FLOAT32 *delayed_input = peak_limiter->delayed_input;
  UWORD32 delayed_input_index = peak_limiter->delayed_input_index;
  FLOAT64 pre_smoothed_gain = peak_limiter->pre_smoothed_gain;

  if (peak_limiter->limiter_on || (FLOAT32)pre_smoothed_gain < 1.0f) {
    for (i = 0; i < frame_len; i++) {
      tmp = 0.0f;
      for (j = 0; j < num_channels; j++) {
        tmp = max(tmp, (FLOAT32)fabs(samples[i * num_channels + j]));
      }

      for (j = attack_time_samples; j > 0; j--) {
        max_buf[j] = max_buf[j - 1];
      }
      max_buf[0] = tmp;
      sectionMaximum = tmp;
      for (j = 1; j < (attack_time_samples + 1); j++) {
        if (max_buf[j] > sectionMaximum) sectionMaximum = max_buf[j];
      }
      maximum = sectionMaximum;

      if (maximum > limit_threshold) {
        gain = limit_threshold / maximum;
      } else {
        gain = 1;
      }

      if (gain < pre_smoothed_gain) {
        gain_modified =
            min(gain_modified,
                (gain - 0.1f * (FLOAT32)pre_smoothed_gain) * 1.11111111f);
      } else {
        gain_modified = gain;
      }

      if (gain_modified < pre_smoothed_gain) {
        pre_smoothed_gain =
            attack_constant * (pre_smoothed_gain - gain_modified) +
            gain_modified;
        pre_smoothed_gain = max(pre_smoothed_gain, gain);
      } else {
        pre_smoothed_gain =
            release_constant * (pre_smoothed_gain - gain_modified) +
            gain_modified;
      }

      gain = (FLOAT32)pre_smoothed_gain;

      for (j = 0; j < num_channels; j++) {
        tmp = delayed_input[delayed_input_index * num_channels + j];
        delayed_input[delayed_input_index * num_channels + j] =
            samples[i * num_channels + j];

        tmp *= gain;
        if (tmp > limit_threshold)
          tmp = limit_threshold;
        else if (tmp < -limit_threshold)
          tmp = -limit_threshold;

        samples[i * num_channels + j] = tmp;
      }

      delayed_input_index++;
      if (delayed_input_index >= attack_time_samples) delayed_input_index = 0;

      if (gain < min_gain) min_gain = gain;
    }
  } else {
    for (i = 0; i < frame_len; i++) {
      for (j = 0; j < num_channels; j++) {
        tmp = delayed_input[delayed_input_index * num_channels + j];
        delayed_input[delayed_input_index * num_channels + j] =
            samples[i * num_channels + j];
        samples[i * num_channels + j] = tmp;
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
