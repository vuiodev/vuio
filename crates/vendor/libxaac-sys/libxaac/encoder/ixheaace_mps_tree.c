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

#include <stdlib.h>
#include "ixheaac_type_def.h"
#include "ixheaac_error_standards.h"
#include "ixheaace_error_codes.h"
#include "ixheaace_mps_common_fix.h"
#include "ixheaace_mps_defines.h"
#include "ixheaace_mps_common_define.h"
#include "ixheaace_bitbuffer.h"

#include "ixheaace_mps_bitstream.h"
#include "ixheaace_mps_param_extract.h"
#include "ixheaace_mps_frame_windowing.h"
#include "ixheaace_mps_tree.h"
#include "ixheaace_mps_buf.h"
#include "ixheaace_mps_lib.h"
#include "ixheaace_mps_onset_detect.h"
#include "ixheaace_mps_static_gain.h"
#include "ixheaace_mps_filter.h"
#include "ixheaace_mps_delay.h"
#include "ixheaace_mps_dmx_tdom_enh.h"
#include "ixheaace_mps_main_structure.h"
#include "ixheaace_mps_tools_rom.h"
#include "ixheaace_mps_qmf.h"
#include "ixheaace_mps_structure.h"
#include "ixheaace_mps_struct_def.h"
#include "ixheaace_mps_sac_polyphase.h"
#include "ixheaace_mps_sac_nlc_enc.h"
#include "ixheaace_mps_sac_hybfilter.h"
#include "ixheaace_mps_spatial_bitstream.h"
#include "ixheaace_mps_rom.h"

static VOID ixheaace_mps_212_space_tree_frame_keep_212(
    const ixheaace_mps_pstr_space_tree pstr_space_tree,
    ixheaace_mps_spatial_frame *const pstr_spatial_frame, const WORD32 avoid_keep) {
  WORD32 band;

  if (avoid_keep) {
    for (band = 0; band < pstr_space_tree->num_param_bands; band++) {
      pstr_space_tree->icc_prev[0][band] = pstr_spatial_frame->ott_data.icc[0][0][band];
      pstr_space_tree->cld_prev[0][band] = pstr_spatial_frame->ott_data.cld[0][0][band];
    }
  } else {
    if (pstr_space_tree->frame_count % 2) {
      for (band = 0; band < pstr_space_tree->num_param_bands; band++) {
        pstr_spatial_frame->ott_data.icc[0][0][band] = pstr_space_tree->icc_prev[0][band];
        pstr_space_tree->cld_prev[0][band] = pstr_spatial_frame->ott_data.cld[0][0][band];
      }
    } else {
      for (band = 0; band < pstr_space_tree->num_param_bands; band++) {
        pstr_space_tree->icc_prev[0][band] = pstr_spatial_frame->ott_data.icc[0][0][band];
        pstr_spatial_frame->ott_data.cld[0][0][band] = pstr_space_tree->cld_prev[0][band];
      }
    }
  }
  pstr_space_tree->frame_count++;
  if (pstr_space_tree->frame_count == MAX_KEEP_FRAMECOUNT) {
    pstr_space_tree->frame_count = 0;
  }
}

IA_ERRORCODE
ixheaace_mps_212_space_tree_init(ixheaace_mps_pstr_space_tree pstr_space_tree,
                                 const ixheaace_mps_space_tree_setup *const pstr_space_tree_setup,
                                 UWORD8 *ptr_parameter_band_2_hybrid_band_offset,
                                 const WORD32 frame_keep_flag, WORD32 aot) {
  IA_ERRORCODE error = IA_NO_ERROR;
  WORD32 box = 0;

  pstr_space_tree->frame_count = 0;
  pstr_space_tree->frame_keep_flag = frame_keep_flag;
  pstr_space_tree->num_param_bands = pstr_space_tree_setup->num_param_bands;
  pstr_space_tree->use_coarse_quant_tto_icc_flag =
      pstr_space_tree_setup->use_coarse_quant_tto_icc_flag;
  pstr_space_tree->use_coarse_quant_tto_cld_flag =
      pstr_space_tree_setup->use_coarse_quant_tto_cld_flag;
  pstr_space_tree->quant_mode = pstr_space_tree_setup->quant_mode;
  pstr_space_tree->num_channels_in_max = pstr_space_tree_setup->num_channels_in_max;
  pstr_space_tree->num_hybrid_bands_max = pstr_space_tree_setup->num_hybrid_bands_max;
  pstr_space_tree->descr.num_ott_boxes = 1;
  pstr_space_tree->descr.num_in_channels = 1;
  pstr_space_tree->descr.num_out_channels = 2;

  if (pstr_space_tree->descr.num_ott_boxes > IXHEAACE_MPS_MAX_NUM_BOXES) {
    return IA_EXHEAACE_INIT_FATAL_MPS_INIT_FAILED;
  }

  for (box = 0; box < pstr_space_tree->descr.num_ott_boxes; box++) {
    ixheaace_mps_tto_box_config box_config;
    box_config.b_use_coherence_icc_only = 0;
    box_config.subband_config = pstr_space_tree->num_param_bands;
    box_config.use_coarse_quant_cld_flag = pstr_space_tree->use_coarse_quant_tto_cld_flag;
    box_config.use_coarse_quant_icc_flag = pstr_space_tree->use_coarse_quant_tto_icc_flag;
    box_config.box_quant_mode = pstr_space_tree->quant_mode;
    box_config.num_hybrid_bands_max = pstr_space_tree->num_hybrid_bands_max;
    box_config.frame_keep_flag = pstr_space_tree->frame_keep_flag;

    error = ixheaace_mps_212_init_tto_box(pstr_space_tree->pstr_tto_box[box], &box_config,
                                          ptr_parameter_band_2_hybrid_band_offset, aot);
    if (error) {
      return error;
    }
  }

  return error;
}

IA_ERRORCODE ixheaace_mps_212_space_tree_apply(
    ixheaace_mps_pstr_space_tree pstr_space_tree, const WORD32 param_set,
    const WORD32 num_channels_in, const WORD32 num_time_slots, const WORD32 start_time_slot,
    const WORD32 num_hybrid_bands, FLOAT32 *p_frame_window_ana_mps,
    ixheaace_cmplx_str ppp_cmplx_hybrid_1[IXHEAACE_MPS_MAX_INPUT_CHANNELS][MAX_ANA_TIME_SLOT]
                                         [MAX_QMF_BANDS],
    ixheaace_cmplx_str ppp_cmplx_hybrid_2[IXHEAACE_MPS_MAX_INPUT_CHANNELS][MAX_ANA_TIME_SLOT]
                                         [MAX_QMF_BANDS],
    ixheaace_mps_spatial_frame *const pstr_spatial_frame, const WORD32 avoid_keep) {
  IA_ERRORCODE error = IA_NO_ERROR;
  WORD32 box;
  const ixheaace_mps_tree_setup *pstr_tree_setup = NULL;
  pstr_tree_setup = &tree_setup_table;

  if ((num_channels_in != pstr_tree_setup->num_channels_in) ||
      (num_channels_in > pstr_space_tree->num_channels_in_max) ||
      (num_hybrid_bands > pstr_space_tree->num_hybrid_bands_max)) {
    return IA_EXHEAACE_CONFIG_NONFATAL_MPS_INVALID_CONFIG;
  }

  for (box = 0; box < pstr_tree_setup->n_tto_boxes; box++) {
    const ixheaace_mps_tto_descriptor *pstr_tto_descriptor =
        &pstr_tree_setup->tto_descriptor[box];
    WORD32 i;
    WORD32 in_ch[2], out_ch[2], win[2];

    in_ch[0] = pstr_tto_descriptor->in_ch1;
    in_ch[1] = pstr_tto_descriptor->in_ch2;
    out_ch[0] = pstr_tto_descriptor->in_ch3;
    out_ch[1] = pstr_tto_descriptor->in_ch4;
    win[0] = pstr_tto_descriptor->w_ch1;
    win[1] = pstr_tto_descriptor->w_ch2;

    for (i = 0; i < 2; i++) {
      if (win[i] == WIN_ACTIV) {
        ixheaace_mps_212_analysis_windowing(num_time_slots, start_time_slot,
                                            p_frame_window_ana_mps, ppp_cmplx_hybrid_1[in_ch[i]],
                                            ppp_cmplx_hybrid_2[out_ch[i]], num_hybrid_bands);
      }
    }

    error = ixheaace_mps_212_apply_tto_box(
        pstr_space_tree->pstr_tto_box[pstr_tto_descriptor->box_id], num_time_slots,
        start_time_slot, num_hybrid_bands, ppp_cmplx_hybrid_2[pstr_tto_descriptor->in_ch3],
        ppp_cmplx_hybrid_2[pstr_tto_descriptor->in_ch4],
        pstr_spatial_frame->ott_data.icc[pstr_tto_descriptor->box_id][param_set],
        &(pstr_spatial_frame->icc_lossless_data
              .bs_quant_coarse_xxx[pstr_tto_descriptor->box_id][param_set]),
        pstr_spatial_frame->ott_data.cld[pstr_tto_descriptor->box_id][param_set],
        &(pstr_spatial_frame->cld_lossless_data
              .bs_quant_coarse_xxx[pstr_tto_descriptor->box_id][param_set]),
        pstr_spatial_frame->b_use_bb_cues);
    if (error) {
      return error;
    }
  }

  if (pstr_space_tree->frame_keep_flag == 1) {
    ixheaace_mps_212_space_tree_frame_keep_212(pstr_space_tree, pstr_spatial_frame, avoid_keep);
  }
  return error;
}
