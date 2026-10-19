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
  UWORD8 bs_smooth_mode[MAX_NUM_PARAMS];
  UWORD8 bs_smooth_time[MAX_NUM_PARAMS];
  UWORD8 bs_freq_res_stride[MAX_NUM_PARAMS];
  UWORD8 bs_smg_data[MAX_NUM_PARAMS][MAX_NUM_BINS];

} ixheaace_mps_smg_data;

typedef struct {
  UWORD8 bs_framing_type;
  WORD32 num_param_sets;
  WORD32 bs_param_slots[MAX_NUM_PARAMS];

} ixheaace_mps_framing_info;

typedef struct {
  WORD8 cld[IXHEAACE_MPS_MAX_NUM_BOXES][MAX_NUM_PARAMS][MAX_NUM_BINS];
  WORD8 icc[IXHEAACE_MPS_MAX_NUM_BOXES][MAX_NUM_PARAMS][MAX_NUM_BINS];

} ixheaace_mps_ott_data;

typedef struct {
  UWORD8 bs_env_shape_channel[MAX_NUM_OUTPUTCHANNELS];
  UWORD8 bs_env_shape_data[MAX_NUM_OUTPUTCHANNELS][MAX_TIME_SLOTS];

} ixheaace_mps_temp_shape_data;

typedef struct {
  WORD32 num_ott_boxes;
  WORD32 num_in_chan;
  WORD32 num_out_chan;

} ixheaace_mps_tree_description;

typedef struct {
  WORD32 bs_ott_bands;

} ixheaace_mps_ott_config;

typedef struct {
  WORD32 bs_sampling_frequency;
  WORD32 bs_frame_length;
  WORD32 num_bands;
  WORD32 bs_tree_config;
  WORD32 bs_quant_mode;
  WORD32 bs_fixed_gain_dmx;
  WORD32 bs_env_quant_mode;
  WORD32 bs_decorr_config;
  ixheaace_mps_tree_description tree_description;
  ixheaace_mps_ott_config ott_config[IXHEAACE_MPS_MAX_NUM_BOXES];

} ixheaace_mps_spatial_specific_config;
typedef struct {
  UWORD8 bs_xxx_data_mode[IXHEAACE_MPS_MAX_NUM_BOXES][MAX_NUM_PARAMS];
  UWORD8 bs_data_pair[IXHEAACE_MPS_MAX_NUM_BOXES][MAX_NUM_PARAMS];
  UWORD8 bs_quant_coarse_xxx[IXHEAACE_MPS_MAX_NUM_BOXES][MAX_NUM_PARAMS];
  UWORD8 bs_freq_res_stride_xxx[IXHEAACE_MPS_MAX_NUM_BOXES][MAX_NUM_PARAMS];

} ixheaace_mps_lossless_data;
typedef struct {
  ixheaace_mps_framing_info framing_info;
  WORD32 bs_independency_flag;
  ixheaace_mps_ott_data ott_data;
  ixheaace_mps_smg_data smg_data;
  ixheaace_mps_temp_shape_data temp_shape_data;
  ixheaace_mps_lossless_data cld_lossless_data;
  ixheaace_mps_lossless_data icc_lossless_data;
  UWORD8 b_use_bb_cues;

} ixheaace_mps_spatial_frame;

typedef struct {
  WORD8 cld_old[IXHEAACE_MPS_MAX_NUM_BOXES][MAX_NUM_BINS];
  WORD8 icc_old[IXHEAACE_MPS_MAX_NUM_BOXES][MAX_NUM_BINS];
  UWORD8 quant_coarse_cld_prev[IXHEAACE_MPS_MAX_NUM_BOXES][MAX_NUM_PARAMS];
  UWORD8 quant_coarse_icc_prev[IXHEAACE_MPS_MAX_NUM_BOXES][MAX_NUM_PARAMS];

} ixheaace_mps_prev_ott_data;

typedef struct {
  ixheaace_mps_prev_ott_data prev_ott_data;

} ixheaace_mps_static_spatial_frame;

typedef struct ixheaace_mps_bsf_instance {
  ixheaace_mps_spatial_specific_config spatial_specific_config;
  ixheaace_mps_spatial_frame frame;
  ixheaace_mps_static_spatial_frame prev_frame_data;

} ixheaace_mps_bsf_instance;

typedef struct ixheaace_mps_bsf_instance *ixheaace_mps_pstr_bsf_instance;

IA_ERRORCODE ixheaace_mps_212_write_spatial_specific_config(
    ixheaace_mps_spatial_specific_config *const pstr_spatial_specific_config,
    UWORD8 *const ptr_output_buffer, const WORD32 output_buffer_size,
    WORD32 *const ptr_output_bits, WORD32 aot);

IA_ERRORCODE ixheaace_mps_212_write_spatial_frame(
    UWORD8 *const ptr_output_buffer, const WORD32 output_buffer_size,
    WORD32 *const ptr_output_bits, ixheaace_mps_pstr_bsf_instance pstr_bsf_instance, WORD32 aot);
