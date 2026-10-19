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
struct ixheaace_mps_tto_box {
  FLOAT32 cld[MAX_NUM_PARAM_BANDS];
  FLOAT32 icc[MAX_NUM_PARAM_BANDS];

  const FLOAT32 *p_icc_quant_table;
  const FLOAT32 *p_cld_quant_table_enc;

  UWORD8 *ptr_parameter_band_2_hybrid_band_offset;
  UWORD8 num_hybrid_bands_max;
  UWORD8 num_parameter_bands;
  WORD32 frame_keep_flag;

  UWORD8 icc_correlation_coherence_border;
  WORD32 box_quant_mode;

  UWORD8 n_icc_quant_steps;
  UWORD8 n_icc_quant_offset;

  UWORD8 n_cld_quant_steps;
  UWORD8 n_cld_quant_offset;

  UWORD8 use_coarse_quant_cld_flag;
  UWORD8 use_coarse_quant_icc_flag;
};

struct ixheaace_mps_box_subband_setup {
  UWORD8 *p_subband_2_parameter_index_ld;
  UWORD8 icc_correlation_coherence_border;
};

typedef struct ixheaace_mps_tto_box_config {
  UWORD8 use_coarse_quant_cld_flag;
  UWORD8 use_coarse_quant_icc_flag;
  UWORD8 b_use_coherence_icc_only;

  WORD32 subband_config;
  WORD32 box_quant_mode;

  UWORD8 num_hybrid_bands_max;
  WORD32 frame_keep_flag;

} ixheaace_mps_tto_box_config;

typedef struct ixheaace_mps_tto_box ixheaace_mps_tto_box, *ixheaace_mps_pstr_tto_box;
typedef struct ixheaace_mps_box_subband_setup ixheaace_mps_box_subband_setup,
    *ixheaace_mps_pstr_box_subband_setup;

IA_ERRORCODE
ixheaace_mps_212_init_tto_box(ixheaace_mps_pstr_tto_box pstr_tto_box,
                              const ixheaace_mps_tto_box_config *const pstr_tto_box_config,
                              UWORD8 *ptr_parameter_band_2_hybrid_band_offset, WORD32 aot);

IA_ERRORCODE ixheaace_mps_212_apply_tto_box(
    ixheaace_mps_pstr_tto_box pstr_tto_box, const WORD32 num_time_slots,
    const WORD32 start_time_slot, const WORD32 num_hybrid_bands,
    ixheaace_cmplx_str pp_cmplx_hybrid_data_1[MAX_ANA_TIME_SLOT][MAX_QMF_BANDS],
    ixheaace_cmplx_str pp_cmplx_hybrid_data_2[MAX_ANA_TIME_SLOT][MAX_QMF_BANDS],
    WORD8 *const p_icc_idx, UWORD8 *const pb_icc_quant_coarse, WORD8 *const p_cld_idx,
    UWORD8 *const pb_cld_quant_coarse, const WORD32 b_use_bb_cues);

VOID ixheaace_mps_212_calc_parameter_band_to_hybrid_band_offset(
    const WORD32 num_hybrid_bands, UWORD8 *ptr_parameter_band_2_hybrid_band_offset, WORD32 aot);
