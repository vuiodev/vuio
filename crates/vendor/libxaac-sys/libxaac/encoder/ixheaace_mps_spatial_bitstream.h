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
  WORD32 num_ott_boxes;
  WORD32 ott_mode_lfe[MAX_NUM_BOXES];
  WORD32 num_ttt_boxes;
  WORD32 num_in_chan;
  WORD32 num_out_chan;
  WORD32 arbitrary_tree;
} ixheaace_mps_sac_tree_description;

typedef struct {
  WORD32 bs_ott_bands;
} ixheaace_mps_sac_ott_config;

typedef struct {
  WORD32 bs_ttt_dual_mode;
  WORD32 bs_ttt_mode_low;
  WORD32 bs_ttt_mode_high;
  WORD32 bs_ttt_bands_low;
} ixheaace_mps_sac_ttt_config;

typedef struct {
  UWORD32 bs_sampling_frequency;
  WORD32 bs_smooth_config;
  WORD32 bs_tree_config;
  ixheaace_mps_sac_tree_description tree_description;
  WORD32 bs_frame_length;
  WORD32 bs_freq_res;
  WORD32 bs_quant_mode;
  WORD32 bs_fixed_gain_sur;
  WORD32 bs_fixed_gain_lfe;
  WORD32 bs_fixed_gain_dmx;
  WORD32 bs_matrix_mode;
  WORD32 bs_temp_shape_config;
  WORD32 bs_decorr_config;
  WORD32 bs_3d_audio_mode;
  WORD32 bs_one_icc;
  WORD32 bs_arbitrary_downmix;
  WORD32 bs_residual_coding;
  ixheaace_mps_sac_ott_config ott_config[MAX_NUM_BOXES];
  ixheaace_mps_sac_ttt_config ttt_config[MAX_NUM_BOXES];
  WORD32 aac_residual_fs;
  WORD32 aac_residual_frames_per_spatial_frame;
  WORD32 aac_residual_use_tns;
  FLOAT32 aac_residual_bwhz[MAX_NUM_BOXES];
  WORD32 aac_residual_bit_rate[MAX_NUM_BOXES];
} ixheaace_mps_sac_specific_config;

typedef struct {
  WORD32 bs_framing_type;
  WORD32 bs_num_param_sets;
  WORD32 bs_param_slots[MAX_NUM_PARAMS];
} ixheaace_mps_sac_framing_info;

typedef struct {
  WORD32 cld[MAX_NUM_BOXES][MAX_NUM_PARAMS][MAX_NUM_BINS];
  WORD32 icc[MAX_NUM_BOXES][MAX_NUM_PARAMS][MAX_NUM_BINS];
  WORD32 cld_old[MAX_NUM_BOXES][MAX_NUM_BINS];
  WORD32 icc_old[MAX_NUM_BOXES][MAX_NUM_BINS];
} ixheaace_mps_sac_ott_data;

typedef struct {
  WORD32 cpc_cld1[MAX_NUM_BOXES][MAX_NUM_PARAMS][MAX_NUM_BINS];
  WORD32 cpc_cld2[MAX_NUM_BOXES][MAX_NUM_PARAMS][MAX_NUM_BINS];
  WORD32 icc[MAX_NUM_BOXES][MAX_NUM_PARAMS][MAX_NUM_BINS];
  WORD32 cpc_cld1_old[MAX_NUM_BOXES][MAX_NUM_BINS];
  WORD32 cpc_cld2_old[MAX_NUM_BOXES][MAX_NUM_BINS];
  WORD32 icc_old[MAX_NUM_BOXES][MAX_NUM_BINS];
} ixheaace_mps_sac_ttt_data;

typedef struct {
  WORD32 bs_smooth_control;
  WORD32 bs_smooth_mode[MAX_NUM_PARAMS];
  WORD32 bs_smooth_time[MAX_NUM_PARAMS];
  WORD32 bs_freq_res_stride[MAX_NUM_PARAMS];
  WORD32 bs_smg_data[MAX_NUM_PARAMS][MAX_NUM_BINS];
} ixheaace_mps_sac_smg_data;

typedef struct {
  WORD32 bs_temp_shape_enable;
  WORD32 bs_temp_shape_enable_channel[MAX_NUM_OUTPUTCHANNELS];
} ixheaace_mps_sac_temp_shape_data;

typedef struct {
  WORD32 bs_xxx_data_mode[MAX_NUM_BOXES][MAX_NUM_PARAMS];
  WORD32 bs_data_pair[MAX_NUM_BOXES][MAX_NUM_PARAMS];
  WORD32 bs_quant_coarse_xxx[MAX_NUM_BOXES][MAX_NUM_PARAMS];
  WORD32 bs_freq_res_stride_xxx[MAX_NUM_BOXES][MAX_NUM_PARAMS];
} ixheaace_mps_sac_lossless_data;

typedef struct {
  ixheaace_mps_sac_framing_info framing_info;
  WORD32 bs_independency_flag;
  WORD32 bs_independency_flag_count;
  ixheaace_mps_sac_ott_data ott_data;
  ixheaace_mps_sac_ttt_data ttt_data;
  ixheaace_mps_sac_smg_data smg_data;
  ixheaace_mps_sac_temp_shape_data temp_shape_data;
  ixheaace_mps_sac_lossless_data cld_lossless_data;
  ixheaace_mps_sac_lossless_data icc_lossless_data;
  ixheaace_mps_sac_lossless_data cpc_lossless_data;
  WORD32 aac_window_grouping[MAX_AAC_FRAMES][MAX_AAC_SHORTWINDOWGROUPS];
  WORD32 aac_window_sequence;
} ixheaace_mps_sac_spatial_frame;

typedef struct {
  ixheaace_mps_sac_specific_config spatial_specific_config;
  ixheaace_mps_sac_spatial_frame current_frame;

  WORD32 num_bins;
  WORD32 total_bits;
} ixheaace_mps_sac_bsf_instance;

typedef struct {
  WORD32 tree_config;
  WORD32 output_channels;
  WORD32 time_slots;
  WORD32 frame_size;
  ixheaace_mps_sac_sbr_encoder_ana_filter_bank filterbank[6];
  ixheaace_mps_hyb_filter_state hyb_state[6];
  ixheaace_mps_sac_bsf_instance *bitstream_formatter;
  ixheaace_mps_sac_qmf_ana_filter_bank qmf_fltbank;
  ixheaace_mps_sac_qmf_synth_filter_bank qmf_synth_fltbank;
  FLOAT32 in1[MAX_INPUT_CHANNELS][MAX_BUFFER_SIZE + DELAY_COMPENSATION];
  FLOAT32 sbr_qmf_states_synthesis[2 * QMF_FILTER_STATE_SYN_SIZE];
} ixheaace_mps_sac_enc, *ixheaace_mps_sac_pstr_enc;

WORD32 ixheaace_mps_515_icc_quant(FLOAT32 val);

WORD32 ixheaace_mps_515_cld_quant(FLOAT32 val);

VOID ixheaace_mps_515_ttt_box(WORD32 slots, FLOAT32 *ptr_real1, FLOAT32 *ptr_imag1,
                              FLOAT32 *ptr_real2, FLOAT32 *ptr_imag2, FLOAT32 *ptr_real3,
                              FLOAT32 *ptr_imag3, WORD32 *ptr_qclds1, WORD32 *ptr_qclds2);

VOID ixheaace_mps_515_ott_box(WORD32 slots, FLOAT32 *ptr_real1, FLOAT32 *ptr_imag1,
                              FLOAT32 *ptr_real2, FLOAT32 *ptr_imag2, WORD32 *ptr_p_qclds,
                              WORD32 *ptr_qiccs);

IA_ERRORCODE
ixheaace_mps_515_write_spatial_specific_config(ixheaace_bit_buf_handle pstr_bit_buf,
                                               ixheaace_mps_sac_bsf_instance *pstr_bsf_instance);

IA_ERRORCODE
ixheaace_mps_515_write_spatial_frame(ixheaace_bit_buf_handle pstr_bit_buf,
                                     ixheaace_mps_sac_bsf_instance *pstr_bsf_instance);
