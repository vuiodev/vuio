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
  UWORD8 use_coarse_quant_tto_cld_flag;
  UWORD8 use_coarse_quant_tto_icc_flag;
  WORD32 quant_mode;
  UWORD8 num_channels_in_max;
  UWORD8 num_hybrid_bands_max;

} ixheaace_mps_space_tree_setup;

typedef struct {
  UWORD8 num_ott_boxes;
  UWORD8 num_in_channels;
  UWORD8 num_out_channels;

} ixheaace_mps_space_tree_description;

struct ixheaace_mps_space_tree {
  ixheaace_mps_space_tree_description descr;
  ixheaace_mps_pstr_tto_box pstr_tto_box[IXHEAACE_MPS_MAX_NUM_BOXES];
  WORD32 num_param_bands;
  UWORD8 use_coarse_quant_tto_icc_flag;
  UWORD8 use_coarse_quant_tto_cld_flag;
  WORD32 quant_mode;
  WORD32 frame_count;
  WORD32 frame_keep_flag;

  UWORD8 cld_prev[IXHEAACE_MPS_MAX_NUM_BOXES][MAX_NUM_PARAM_BANDS];
  UWORD8 icc_prev[IXHEAACE_MPS_MAX_NUM_BOXES][MAX_NUM_PARAM_BANDS];

  UWORD8 num_channels_in_max;
  UWORD8 num_hybrid_bands_max;
};

typedef struct {
  UWORD8 box_id;
  UWORD8 in_ch1;
  UWORD8 in_ch2;
  UWORD8 in_ch3;
  UWORD8 in_ch4;
  UWORD8 w_ch1;
  UWORD8 w_ch2;

} ixheaace_mps_tto_descriptor;

typedef struct {
  UWORD8 num_channels_in;
  UWORD8 n_channels_out;
  UWORD8 n_tto_boxes;
  ixheaace_mps_tto_descriptor tto_descriptor[1];
} ixheaace_mps_tree_setup;

typedef struct ixheaace_mps_space_tree ixheaace_mps_space_tree, *ixheaace_mps_pstr_space_tree;

IA_ERRORCODE
ixheaace_mps_212_space_tree_init(ixheaace_mps_pstr_space_tree pstr_space_tree,
                                 const ixheaace_mps_space_tree_setup *const pstr_space_tree_setup,
                                 UWORD8 *ptr_parameter_band_2_hybrid_band_offset,
                                 const WORD32 frame_keep_flag, WORD32 aot);

IA_ERRORCODE ixheaace_mps_212_space_tree_apply(
    ixheaace_mps_pstr_space_tree pstr_space_tree, const WORD32 param_set,
    const WORD32 num_channels_in, const WORD32 num_time_slots, const WORD32 start_time_slot,
    const WORD32 num_hybrid_bands, FLOAT32 *p_frame_window_ana_mps,
    ixheaace_cmplx_str ppp_cmplx_hybrid_1[IXHEAACE_MPS_MAX_INPUT_CHANNELS][MAX_ANA_TIME_SLOT]
                                         [MAX_QMF_BANDS],
    ixheaace_cmplx_str ppp_cmplx_hybrid_2[IXHEAACE_MPS_MAX_INPUT_CHANNELS][MAX_ANA_TIME_SLOT]
                                         [MAX_QMF_BANDS],
    ixheaace_mps_spatial_frame *const pstr_spatial_frame, const WORD32 avoid_keep);
