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
  const FLOAT32 *p_filter;
} ixheaace_mps_sac_qmf_ana_filter_bank;

typedef struct {
  const FLOAT32 *p_filter;
  const FLOAT32 *cos_twiddle;
  const FLOAT32 *sin_twiddle;
  const FLOAT32 *alt_sin_twiddle;
} ixheaace_mps_sac_qmf_synth_filter_bank;

typedef ixheaace_mps_sac_qmf_ana_filter_bank *ixheaace_mps_sac_pstr_qmf_ana_filter_bank;
typedef ixheaace_mps_sac_qmf_synth_filter_bank *ixheaace_mps_sac_pstr_qmf_synth_filter_bank;
