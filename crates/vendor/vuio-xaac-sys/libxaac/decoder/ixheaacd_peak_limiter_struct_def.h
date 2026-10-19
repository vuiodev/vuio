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
#ifndef IXHEAACD_PEAK_LIMITER_STRUCT_DEF_H
#define IXHEAACD_PEAK_LIMITER_STRUCT_DEF_H

#define PEAK_LIM_SIZE (1024 * 16)
#define DEFAULT_ATTACK_TIME_MS (5.0f)
#define DEFAULT_RELEASE_TIME_MS (50.0f)
#define PEAK_LIM_THR_FLOAT (29203.6f)
#define PEAK_LIM_THR_FIX (2147483647)

typedef struct ia_peak_limiter_struct {
  FLOAT32 attack_time;
  FLOAT32 release_time;
  FLOAT32 attack_constant;
  FLOAT32 release_constant;
  FLOAT32 limit_threshold;
  UWORD32 num_channels;
  UWORD32 sample_rate;
  UWORD32 attack_time_samples;
  UWORD32 limiter_on;
  FLOAT32 gain_modified;
  FLOAT64 pre_smoothed_gain;
  FLOAT32 *delayed_input;
  UWORD32 delayed_input_index;
  FLOAT32 *max_buf;
  FLOAT32 min_gain;
  FLOAT32 buffer[PEAK_LIM_SIZE];
  WORD32 max_idx;
  WORD32 cir_buf_pnt;
} ia_peak_limiter_struct;

#endif /* IXHEAACD_PEAK_LIMITER_STRUCT_DEF_H */
