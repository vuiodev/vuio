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
struct ixheaace_mps_static_gain_config {
  WORD32 fixed_gain_dmx;
  WORD32 pre_gain_factor_db;
};

struct ixheaace_mps_static_gain {
  WORD32 fixed_gain_dmx;
  WORD32 pre_gain_factor_db;

  FLOAT32 post_gain;
  FLOAT32 pre_gain[IXHEAACE_MPS_MAX_INPUT_CHANNELS];
};

typedef struct ixheaace_mps_static_gain_config ixheaace_mps_static_gain_config,
    *ixheaace_mps_pstr_static_gain_config;
typedef struct ixheaace_mps_static_gain ixheaace_mps_static_gain, *ixheaace_mps_pstr_static_gain;

IA_ERRORCODE ixheaace_mps_212_static_gain_init(
    ixheaace_mps_pstr_static_gain pstr_static_gain,
    const ixheaace_mps_pstr_static_gain_config pstr_static_gain_config);
