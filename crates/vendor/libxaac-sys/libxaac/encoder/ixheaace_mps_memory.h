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
  ixheaace_mps_struct mps_encoder_instance;
  ixheaace_mps_space_structure mp4_space_encoder_instance;
  UWORD8 parameter_band_2_hybrid_band_offset[MAX_HYBRID_BAND_OFFSET];
  ixheaace_mps_pstr_qmf_filter_bank pstr_qmf_filter_bank[IXHEAACE_MPS_MAX_INPUT_CHANNELS];
  UWORD8 ssc[MAX_SSC_BYTES];
  FLOAT32 frame_window_ana_flt[MAX_NUM_PARAMS][MAX_ANA_TIME_SLOT];
  WORD32 n_output_bits[MAX_BITSTREAM_DELAY];

  FLOAT32 ptr_filter_states[IXHEAACE_MPS_MAX_INPUT_CHANNELS][2 * 5 * MAX_QMF_BANDS];

  ixheaace_mps_qmf_filter_bank qmf_filter_bank[IXHEAACE_MPS_MAX_INPUT_CHANNELS];
  ixheaace_mps_space_tree spacce_tree;
  ixheaace_mps_tto_box tto_box;
  ixheaace_mps_onset_detect onset_detect[IXHEAACE_MPS_MAX_INPUT_CHANNELS];
  FLOAT32 energy_hist_float[IXHEAACE_MPS_MAX_INPUT_CHANNELS][16 + MAX_FRAME_TIME_SLOT];
  ixheaace_mps_static_gain_config static_gain_config;
  ixheaace_mps_static_gain static_gain;
  ixheaace_mps_frame_win frame_window;
  ixheaace_mps_delay delay;
  ixheaace_mps_enhanced_time_domain_dmx enhanced_time_dmx;
  FLOAT32 sinus_window_flt[4 * (MPS_MAX_FRAME_LENGTH + 1)];
  ixheaace_mps_bsf_instance bitstream;
  ixheaace_mps_dc_filter dc_filter[IXHEAACE_MPS_MAX_INPUT_CHANNELS];
  ixheaace_mps_enc_config_setup setup;
} ixheaace_mps_212_memory_struct;

typedef struct {
  ixheaace_mps_sac_enc spatial_enc_instance;
  ixheaace_mps_sac_bsf_instance bsf_memory_instance;
} ixheaace_mps_515_memory_struct;
