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

#pragma once
struct ixheaace_mps_enhanced_time_domain_dmx {
  WORD32 max_frame_length;

  WORD32 frame_length;
  FLOAT32 prev_gain[2];
  FLOAT32 prev_h1[2];
  FLOAT32 *sinus_window;
  FLOAT32 prev_left_energy;
  FLOAT32 prev_right_energy;
  FLOAT32 prev_x_energy;
  FLOAT32 cld_weight;
  FLOAT32 gain_weight[2];
};

typedef struct ixheaace_mps_enhanced_time_domain_dmx ixheaace_mps_enhanced_time_domain_dmx,
    *ixheaace_mps_pstr_enhanced_time_domain_dmx;

IA_ERRORCODE ixheaace_mps_212_init_enhanced_time_domain_dmx(
    ixheaace_mps_pstr_enhanced_time_domain_dmx pstr_enhanced_time_domain_dmx,
    const FLOAT32 *const ptr_input_gain_m_flt, const FLOAT32 output_gain_m_flt,
    const WORD32 frame_length);

IA_ERRORCODE ixheaace_mps_212_apply_enhanced_time_domain_dmx(
    ixheaace_mps_pstr_enhanced_time_domain_dmx pstr_enhanced_time_domain_dmx,
    FLOAT32 input_time[2][MPS_MAX_FRAME_LENGTH + MAX_DELAY_SURROUND_ANALYSIS],
    FLOAT32 *const ptr_output_time_dmx, const WORD32 input_delay);
