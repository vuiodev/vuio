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

typedef struct {
  FLOAT32 long_nrg[120];
  FLOAT32 short_nrg[TRANS_FAC][MAXIMUM_SCALE_FACTOR_BAND_SHORT];
} ixheaace_sfb_energy;

typedef struct {
  FLOAT32 long_nrg;
  FLOAT32 short_nrg[TRANS_FAC];
} ixheaace_sfb_energy_sum;

typedef struct {
  ixheaace_block_switch_control blk_switch_cntrl; /* block switching */
  FLOAT32 *ptr_mdct_delay_buf;
  FLOAT32 sfb_threshold_nm1_float[MAXIMUM_SCALE_FACTOR_BAND]; /* PreEchoControl */
  ixheaace_sfb_energy sfb_threshold;                          /* adapt           */
  ixheaace_sfb_energy sfb_energy;                             /* scale_factor_band_ Energy      */
  ixheaace_sfb_energy sfb_energy_ms;
  ixheaace_sfb_energy_sum sfb_energy_sum;
  ixheaace_sfb_energy_sum sfb_energy_sum_ms;
  ixheaace_sfb_energy sfb_sreaded_energy;
  FLOAT32 *ptr_spec_coeffs;
} ixheaace_psy_data;
