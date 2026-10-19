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
VOID ixheaace_mps_515_calculate_sbr_syn_filterbank(
    FLOAT32 *sr, FLOAT32 *si, FLOAT32 *time_sig, WORD32 channel,
    ixheaace_mps_sac_pstr_qmf_synth_filter_bank qmf_bank, WORD32 nr_samples,
    FLOAT32 *sbr_qmf_states_synthesis_per);

typedef struct {
  FLOAT32 x[640];

} ixheaace_mps_sac_sbr_encoder_ana_filter_bank;

VOID ixheaace_mps_515_calculate_ana_filterbank(
    ixheaace_mps_sac_sbr_encoder_ana_filter_bank *filterbank, FLOAT32 *time_sig, FLOAT32 *sr,
    FLOAT32 *si, ixheaace_mps_sac_pstr_qmf_ana_filter_bank qmf_bank);
