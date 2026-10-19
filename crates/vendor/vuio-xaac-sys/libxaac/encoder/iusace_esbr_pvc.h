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
#define IXHEAACE_ESBR_PVC_MOD_NUM_TAB (2)
#define IXHEAACE_ESBR_PVC_NUM_TAB_IDX_1 (3)
#define IXHEAACE_ESBR_PVC_NUM_TAB_IDX_2 (128)
#define IXHEAACE_ESBR_PVC_NUM_PVCID (128)
#define IXHEAACE_ESBR_PVC_NUM_TS (16)
#define IXHEAACE_ESBR_PVC_NUM_QMF_BANDS (64)
#define IXHEAACE_ESBR_PVC_NUM_QMF_BANDS_CORE (64)
#define IXHEAACE_ESBR_PVC_NUM_BANDS_CORE (3)
#define IXHEAACE_ESBR_PVC_NUM_BANDS_SBR_MODE1 (8)
#define IXHEAACE_ESBR_PVC_NUM_BANDS_SBR_MODE2 (6)

#define IXHEAACE_ESBR_PVC_POW_THRS (0.1f)
#define IXHEAACE_ESBR_PVC_NS_MODE_PRD_THRS (2)

#define IXHEAACE_ESBR_PVC_RESIDUAL_VAL (1000000.0f)
#define IXHEAACE_ESBR_PVC_NTS_GRP_ID (8)

#define IXHEAACE_ESBR_PVC_DIV_MODE_BITS (3)
#define IXHEAACE_ESBR_PVC_NS_MODE_BITS (1)
#define IXHEAACE_ESBR_PVC_ID_BITS (7)
#define IXHEAACE_ESBR_PVC_GRID_INFO_BITS (1)
#define IXHEAACE_ESBR_PVC_REUSE_BITS (1)

#define IXHEAACE_ESBR_PVC_FLAG_PREV_DFLT (0)
#define IXHEAACE_ESBR_PVC_ID_PREV_DFLT (0xFF)
#define IXHEAACE_ESBR_PVC_RATE_PREV_DFLT (0xFF)
#define IXHEAACE_ESBR_PVC_STRT_BAND_PREV_DFLT (0xFF)

#define IXHEAACE_ESBR_PVC_MODE_1 (1)
#define IXHEAACE_ESBR_PVC_MODE_2 (2)

typedef struct {
  const FLOAT32 pvc_smth_win_ns_16[16];
  const FLOAT32 pvc_smth_win_ns_12[12];
  const FLOAT32 pvc_smth_win_ns_4[4];
  const FLOAT32 pvc_smth_win_ns_3[3];
  const UWORD8 pvc_idx_mode_1[IXHEAACE_ESBR_PVC_MOD_NUM_TAB];
  const UWORD8 pvc_prd_coef_kb_3_mode_1[IXHEAACE_ESBR_PVC_NUM_TAB_IDX_2]
                                       [IXHEAACE_ESBR_PVC_NUM_BANDS_SBR_MODE1];
  const UWORD8 pvc_prd_coef_kb_012_mode_1[IXHEAACE_ESBR_PVC_NUM_TAB_IDX_1]
                                         [IXHEAACE_ESBR_PVC_NUM_BANDS_CORE]
                                         [IXHEAACE_ESBR_PVC_NUM_BANDS_SBR_MODE1];
  const FLOAT32 pvc_scaling_coef_mode_1[IXHEAACE_ESBR_PVC_NUM_BANDS_CORE + 1];
  const UWORD8 pvc_idx_mode_2[IXHEAACE_ESBR_PVC_MOD_NUM_TAB];
  const UWORD8 pvc_prd_coef_kb_3_mode_2[IXHEAACE_ESBR_PVC_NUM_TAB_IDX_2]
                                       [IXHEAACE_ESBR_PVC_NUM_BANDS_SBR_MODE2];
  const UWORD8 pvc_prd_coef_kb_012_mode_2[IXHEAACE_ESBR_PVC_NUM_TAB_IDX_1]
                                         [IXHEAACE_ESBR_PVC_NUM_BANDS_CORE]
                                         [IXHEAACE_ESBR_PVC_NUM_BANDS_SBR_MODE2];
  const FLOAT32 pvc_scaling_coef_mode_2[IXHEAACE_ESBR_PVC_NUM_BANDS_CORE + 1];
} ixheaace_pvc_tabs_struct;

extern const ixheaace_pvc_tabs_struct ixheaace_pvc_tabs;

typedef struct {
  UWORD8 pvc_mode;
  UWORD8 div_mode;
  UWORD8 ns_mode;
  UWORD16 pvc_id[IXHEAACE_ESBR_PVC_NUM_TS];
  UWORD8 time_smth_ts;
  UWORD8 num_grp_core;
  UWORD8 num_grp_sbr;
  UWORD8 hbw;
  UWORD8 num_pvc_id;
  UWORD8 pvc_rate;
  WORD32 usac_indep_flag;
} ixheaace_pvc_params;

typedef struct {
  UWORD16 pvc_id;
  UWORD8 start_band;
  UWORD8 pvc_flag;
  UWORD8 pvc_mode;
  UWORD8 pvc_rate;
} ixheaace_pvc_prv_frm_params;

typedef struct {
  const FLOAT32 *smoothing_coef;
  const FLOAT32 *scaling_coef;
  const UWORD8 *pvc_pred_coef_kb_3;
  const UWORD8 *pvc_pred_coef_kb_012;
  const UWORD8 *pvc_idx_tab;
} ixheaace_pvc_coef_tabs;

typedef struct {
  UWORD8 div_mode;
  UWORD8 grid_info[IXHEAACE_ESBR_PVC_NUM_TS];
  UWORD8 ns_mode;
  WORD32 num_grid_info;
  UWORD16 pvc_id_bs[IXHEAACE_ESBR_PVC_NUM_TS];
} ixheaace_pvc_bs_info;

typedef struct {
  ixheaace_pvc_bs_info pvc_bs_info;
  ixheaace_pvc_params pvc_param;
  ixheaace_pvc_prv_frm_params pvc_prv_param;
  ixheaace_pvc_coef_tabs pvc_tabs;
  FLOAT32 sb_grp_energy[IXHEAACE_ESBR_PVC_NUM_TS + 16 - 1][3];
} ixheaace_pvc_enc;

typedef struct {
  FLOAT32 pvc_qmf_low[IXHEAACE_ESBR_PVC_NUM_TS * IXHEAACE_ESBR_PVC_NUM_QMF_BANDS_CORE];
  FLOAT32 pvc_qmf_high[IXHEAACE_ESBR_PVC_NUM_TS * IXHEAACE_ESBR_PVC_NUM_QMF_BANDS];
} ixheaace_pvc_scratch;

IA_ERRORCODE ixheaace_pvc_enc_init(ixheaace_pvc_enc *pstr_pvc_enc, WORD32 sbr_pvc_rate);

IA_ERRORCODE ixheaace_pvc_encode_frame(ixheaace_pvc_enc *pstr_pvc_enc, UWORD8 pvc_mode,
                                       FLOAT32 *ptr_qmf_low, FLOAT32 *ptr_qmf_high,
                                       UWORD8 start_band, UWORD8 stop_band);
