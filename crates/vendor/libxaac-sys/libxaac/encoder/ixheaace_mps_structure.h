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
  WORD32 num_param_bands;
  WORD32 quant_mode;
  UWORD8 b_use_coarse_quant;
  UWORD8 b_ld_mode;
  UWORD32 sample_rate;
  UWORD32 frame_time_slots;
  UWORD32 independency_factor;
  WORD32 time_alignment;

} ixheaace_mps_space_enc_setup, *ixheaace_mps_pstr_spece_enc_setup;

typedef struct {
  UWORD8 b_enc_mode_212;
  UWORD8 max_hybrid_in_static_slots;
  WORD32 max_sampling_rate;
  WORD32 max_analysis_length_time_slots;
  WORD32 max_hybrid_bands;
  WORD32 max_qmf_bands;
  WORD32 max_ch_in;
  WORD32 max_frame_time_slots;
  WORD32 max_frame_length;
  WORD32 max_ch_out;
  WORD32 max_ch_tot_out;
} ixheaace_mps_enc_config_setup, *ixheaace_mps_pstr_enc_config_setup;

struct ixheaace_mps_space_structure {
  ixheaace_mps_space_enc_setup user;
  ixheaace_mps_pstr_enc_config_setup setup;
  ixheaace_mps_pstr_frame_win h_frame_window;
  WORD32 n_samples_valid;
  WORD32 num_param_bands;
  UWORD8 b_enc_mode_212_only;

  UWORD8 use_frame_keep;
  UWORD32 independency_factor;
  UWORD32 num_sample_rate;
  UWORD8 n_input_channels;
  UWORD8 n_output_channels;
  WORD32 num_frame_time_slots;
  UWORD8 num_qmf_bands;
  UWORD8 num_hybrid_bands;
  UWORD32 n_frame_length;

  WORD32 n_samples_next;
  WORD32 num_analysis_length_time_slots;
  WORD32 n_analysis_lookahead_time_slots;
  WORD32 n_update_hybrid_position_time_slots;
  WORD32 *pn_output_bits;
  WORD32 n_input_delay;
  WORD32 n_output_buffer_delay;
  WORD32 n_surround_analysis_buffer_delay;
  WORD32 n_bitstream_delay_buffer;
  WORD32 n_bitstream_buffer_read;
  WORD32 n_bitstream_buffer_write;
  WORD32 num_discard_out_frames;
  WORD32 avoid_keep;

  UWORD8 use_coarse_quant_cld;
  UWORD8 use_coarse_quant_icc;
  UWORD8 use_coarse_quant_cpc;
  UWORD8 use_coarse_quant_arb_dmx;
  WORD32 quant_mode;
  WORD32 core_coder_delay;
  WORD32 time_alignment;

  WORD32 independency_count;
  WORD32 independency_flag;
  WORD32 pp_tr_curr_pos[IXHEAACE_MPS_MAX_INPUT_CHANNELS][MAX_NUM_TRANS];
  WORD32 tr_prev_pos[MAX_NUM_PARAMS];

  ixheaace_mps_frame_win_list frame_win_list;
  ixheaace_mps_spatial_frame save_frame;

  ixheaace_mps_space_tree_setup space_tree_setup;
  ixheaace_mps_space_ssc_buf ssc_buf;

  FLOAT32 *ptr_frame_window_ana[MAX_NUM_PARAMS];

  ixheaace_mps_pstr_qmf_filter_bank *pstr_qmf_filter_in;
  ixheaace_mps_pstr_dc_filter pstr_dc_filter[IXHEAACE_MPS_MAX_INPUT_CHANNELS];
  ixheaace_mps_pstr_onset_detect pstr_onset_detect[IXHEAACE_MPS_MAX_INPUT_CHANNELS];
  ixheaace_mps_pstr_space_tree pstr_space_tree;
  ixheaace_mps_pstr_bsf_instance pstr_bitstream_formatter;
  ixheaace_mps_pstr_static_gain_config pstr_static_gain_config;
  ixheaace_mps_pstr_static_gain pstr_static_gain;
  ixheaace_mps_pstr_delay pstr_delay;
  ixheaace_mps_pstr_enhanced_time_domain_dmx pstr_enhanced_time_dmx;

  FLOAT32 time_signal_in[2][MPS_MAX_FRAME_LENGTH + MAX_DELAY_SURROUND_ANALYSIS];
  FLOAT32 time_signal_delay_in[2][MAX_DELAY_SURROUND_ANALYSIS];

  ixheaace_cmplx_str cmplx_hybrid_in[IXHEAACE_MPS_MAX_INPUT_CHANNELS][MAX_ANA_TIME_SLOT]
                                    [MAX_HYBRID_BAND];
  ixheaace_cmplx_str cmplx_hybrid_in_static[MAX_SPACE_TREE_CHANNELS][MAX_HYBRID_STATIC_SLOT]
                                           [MAX_HYBRID_BAND];
  UWORD8 bit_stream_delay_buffer[MAX_BITSTREAM_DELAY][MAX_MPEGS_BYTES];
  UWORD8 *ptr_parameter_band_2_hybrid_band_offset;
};
