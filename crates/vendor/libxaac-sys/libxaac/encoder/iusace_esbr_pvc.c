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

#include <string.h>
#include <math.h>

#include "ixheaac_type_def.h"
#include "ixheaac_constants.h"
#include "ixheaace_aac_constants.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops16.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaac_error_standards.h"
#include "ixheaace_error_codes.h"

#include "ixheaace_bitbuffer.h"
#include "iusace_esbr_pvc.h"
#include "ixheaace_common_utils.h"
#include "ixheaace_sbr_cmondata.h"

IA_ERRORCODE ixheaace_pvc_enc_init(ixheaace_pvc_enc *pstr_pvc_enc, WORD32 sbr_pvc_rate) {
  pstr_pvc_enc->pvc_prv_param.pvc_flag = IXHEAACE_ESBR_PVC_FLAG_PREV_DFLT;
  pstr_pvc_enc->pvc_prv_param.pvc_id = IXHEAACE_ESBR_PVC_ID_PREV_DFLT;
  pstr_pvc_enc->pvc_prv_param.pvc_rate = IXHEAACE_ESBR_PVC_RATE_PREV_DFLT;
  pstr_pvc_enc->pvc_prv_param.start_band = IXHEAACE_ESBR_PVC_STRT_BAND_PREV_DFLT;
  pstr_pvc_enc->pvc_param.pvc_rate = (UWORD8)sbr_pvc_rate;

  return IA_NO_ERROR;
}

static VOID ixheaace_pvc_sb_grouping(ixheaace_pvc_enc *pstr_pvc_enc, UWORD8 start_band,
                                     FLOAT32 *ptr_qmf_low, FLOAT32 *ptr_sb_grp_energy,
                                     WORD32 first_pvc_ts) {
  WORD32 ksg, ts, band;
  FLOAT32 tmp_sb_grp_energy;
  WORD32 lbw, sb;
  WORD32 nqmf_lb;
  FLOAT32 *ptr_tmp_qmfl;

  ixheaace_pvc_params *pstr_params = &pstr_pvc_enc->pvc_param;
  ixheaace_pvc_prv_frm_params *pstr_prv_params = &pstr_pvc_enc->pvc_prv_param;

  nqmf_lb = IXHEAACE_ESBR_PVC_NUM_QMF_BANDS / pstr_params->pvc_rate;
  lbw = 8 / pstr_params->pvc_rate;

  for (ts = 0; ts < IXHEAACE_ESBR_PVC_NUM_TS; ts++) {
    sb = start_band - lbw * pstr_params->num_grp_core;
    ksg = 0;
    while (ksg < pstr_params->num_grp_core) {
      tmp_sb_grp_energy = 0.0f;
      if (sb >= 0) {
        ptr_tmp_qmfl = &ptr_qmf_low[ts * nqmf_lb + sb];
        band = 0;
        while (band < lbw) {
          tmp_sb_grp_energy += ptr_tmp_qmfl[band];
          band++;
        }
        tmp_sb_grp_energy /= lbw;
      }

      tmp_sb_grp_energy = max(IXHEAACE_ESBR_PVC_POW_THRS, tmp_sb_grp_energy);
      ptr_sb_grp_energy[(ts + IXHEAACE_ESBR_PVC_NUM_TS - 1) * 3 + ksg] =
          10 * (FLOAT32)log10(tmp_sb_grp_energy);
      sb += lbw;
      ksg++;
    }
  }

  if ((pstr_prv_params->pvc_flag == 0) ||
      ((start_band * pstr_params->pvc_rate) !=
       (pstr_prv_params->start_band * pstr_prv_params->pvc_rate))) {
    for (ts = 0; ts < IXHEAACE_ESBR_PVC_NUM_TS - 1 + first_pvc_ts; ts++) {
      memcpy(&ptr_sb_grp_energy[ts * 3],
             &ptr_sb_grp_energy[(IXHEAACE_ESBR_PVC_NUM_TS - 1 + first_pvc_ts) * 3],
             pstr_params->num_grp_core * sizeof(ptr_sb_grp_energy[0]));
    }
  }

  return;
}

static IA_ERRORCODE ixheaace_set_pvc_mode_param(ixheaace_pvc_params *pstr_pvc_param,
                                                ixheaace_pvc_coef_tabs *pstr_pvc_tabs) {
  pstr_pvc_param->num_grp_core = IXHEAACE_ESBR_PVC_NUM_BANDS_CORE;
  pstr_pvc_param->num_pvc_id = IXHEAACE_ESBR_PVC_NUM_PVCID;

  switch (pstr_pvc_param->pvc_mode) {
    case IXHEAACE_ESBR_PVC_MODE_1:
      pstr_pvc_param->num_grp_sbr = IXHEAACE_ESBR_PVC_NUM_BANDS_SBR_MODE1;
      pstr_pvc_param->hbw = 8 / pstr_pvc_param->pvc_rate;
      pstr_pvc_tabs->pvc_pred_coef_kb_012 =
          (UWORD8 *)ixheaace_pvc_tabs.pvc_prd_coef_kb_012_mode_1;
      pstr_pvc_tabs->pvc_pred_coef_kb_3 = (UWORD8 *)ixheaace_pvc_tabs.pvc_prd_coef_kb_3_mode_1;
      pstr_pvc_tabs->pvc_idx_tab = ixheaace_pvc_tabs.pvc_idx_mode_1;
      pstr_pvc_tabs->scaling_coef = ixheaace_pvc_tabs.pvc_scaling_coef_mode_1;
      break;

    case IXHEAACE_ESBR_PVC_MODE_2:
      pstr_pvc_param->num_grp_sbr = IXHEAACE_ESBR_PVC_NUM_BANDS_SBR_MODE2;
      pstr_pvc_param->hbw = 12 / pstr_pvc_param->pvc_rate;
      pstr_pvc_tabs->pvc_pred_coef_kb_012 =
          (UWORD8 *)ixheaace_pvc_tabs.pvc_prd_coef_kb_012_mode_2;
      pstr_pvc_tabs->pvc_pred_coef_kb_3 = (UWORD8 *)ixheaace_pvc_tabs.pvc_prd_coef_kb_3_mode_2;
      pstr_pvc_tabs->pvc_idx_tab = ixheaace_pvc_tabs.pvc_idx_mode_2;
      pstr_pvc_tabs->scaling_coef = ixheaace_pvc_tabs.pvc_scaling_coef_mode_2;
      break;

    default:
      return IA_EXHEAACE_EXE_NONFATAL_ESBR_INVALID_PVC_MODE;
  }
  return IA_NO_ERROR;
}

static VOID ixheaace_pvc_sb_grouping_ref(ixheaace_pvc_params *pstr_pvc_param, UWORD8 start_band,
                                         UWORD8 stop_band, FLOAT32 *ptr_qmf_high,
                                         FLOAT32 *ptr_sb_grp_energy) {
  WORD32 ksg, ts, band;
  UWORD8 min_sb;
  WORD32 sb, eb;
  FLOAT32 tmp_sb_grp_energy;

  min_sb = start_band + pstr_pvc_param->num_grp_sbr * pstr_pvc_param->hbw - 1;
  stop_band = max(stop_band, min_sb);

  for (ts = 0; ts < IXHEAACE_ESBR_PVC_NUM_TS; ts++) {
    sb = start_band;
    eb = min((sb + pstr_pvc_param->hbw - 1), (IXHEAACE_ESBR_PVC_NUM_QMF_BANDS - 1));

    for (ksg = 0; ksg < (pstr_pvc_param->num_grp_sbr - 2); ksg++) {
      tmp_sb_grp_energy = 0.0f;
      band = sb;
      while (band <= eb) {
        tmp_sb_grp_energy += ptr_qmf_high[ts * IXHEAACE_ESBR_PVC_NUM_QMF_BANDS + band];
        band++;
      }
      tmp_sb_grp_energy = tmp_sb_grp_energy / (eb - sb + 1);

      ptr_sb_grp_energy[ts * pstr_pvc_param->num_grp_sbr + ksg] =
          max(IXHEAACE_ESBR_PVC_POW_THRS, tmp_sb_grp_energy);
      sb += pstr_pvc_param->hbw;
      eb = min((sb + pstr_pvc_param->hbw - 1), stop_band);
      eb = min(eb, (IXHEAACE_ESBR_PVC_NUM_QMF_BANDS - 1));
    }

    while (ksg < pstr_pvc_param->num_grp_sbr) {
      tmp_sb_grp_energy = 0.0f;
      band = sb;
      while (band <= eb) {
        tmp_sb_grp_energy += ptr_qmf_high[ts * IXHEAACE_ESBR_PVC_NUM_QMF_BANDS + band];
        band++;
      }
      tmp_sb_grp_energy = tmp_sb_grp_energy / (eb - sb + 1);

      ptr_sb_grp_energy[ts * pstr_pvc_param->num_grp_sbr + ksg] =
          max(IXHEAACE_ESBR_PVC_POW_THRS, tmp_sb_grp_energy);
      sb += pstr_pvc_param->hbw;
      eb = min(stop_band, (IXHEAACE_ESBR_PVC_NUM_QMF_BANDS - 1));
      ksg++;
    }
  }
  return;
}

static VOID ixheaace_pvc_calc_grp_energy_below_sbr(ixheaace_pvc_enc *pstr_pvc_enc, WORD32 ts,
                                                   FLOAT32 *ptr_sb_grp_ene_blsbr) {
  WORD32 ksg = 0, idx;
  ixheaace_pvc_params *pstr_pvc_params = &pstr_pvc_enc->pvc_param;

  memset(ptr_sb_grp_ene_blsbr, 0,
         pstr_pvc_params->num_grp_core * sizeof(ptr_sb_grp_ene_blsbr[0]));

  while (ksg < pstr_pvc_params->num_grp_core) {
    for (idx = 0; idx < pstr_pvc_params->time_smth_ts; idx++) {
      ptr_sb_grp_ene_blsbr[ksg] +=
          pstr_pvc_enc->sb_grp_energy[ts + IXHEAACE_ESBR_PVC_NUM_TS - 1 - idx][ksg] *
          pstr_pvc_enc->pvc_tabs.smoothing_coef[idx];
    }
    ksg++;
  }
  return;
}

static VOID ixheaace_pvc_predict(ixheaace_pvc_enc *pstr_pvc_enc, WORD32 ts,
                                 FLOAT32 *ptr_sb_grp_ene_blsbr, FLOAT32 *ptr_sb_grp_energy_high) {
  ixheaace_pvc_params *pstr_pvc_params = &(pstr_pvc_enc->pvc_param);
  ixheaace_pvc_coef_tabs *pstr_pvc_tabs = &(pstr_pvc_enc->pvc_tabs);

  WORD32 ksg, kb, idx_tab_1, idx_tab_2;
  const UWORD8 *ptr_tab;
  FLOAT32 prod;

  idx_tab_2 = pstr_pvc_params->pvc_id[ts];

  if (idx_tab_2 < pstr_pvc_tabs->pvc_idx_tab[0]) {
    idx_tab_1 = 0;
  } else if (idx_tab_2 < pstr_pvc_tabs->pvc_idx_tab[1]) {
    idx_tab_1 = 1;
  } else {
    idx_tab_1 = 2;
  }

  memset(ptr_sb_grp_energy_high, 0,
         pstr_pvc_params->num_grp_sbr * sizeof(ptr_sb_grp_energy_high[0]));

  ptr_tab = &(pstr_pvc_tabs->pvc_pred_coef_kb_012[idx_tab_1 * pstr_pvc_params->num_grp_core *
                                                  pstr_pvc_params->num_grp_sbr]);

  for (kb = 0; kb < pstr_pvc_params->num_grp_core; kb++) {
    prod = pstr_pvc_tabs->scaling_coef[kb] * ptr_sb_grp_ene_blsbr[kb];
    for (ksg = 0; ksg < pstr_pvc_params->num_grp_sbr; ksg++) {
      ptr_sb_grp_energy_high[ksg] += ((FLOAT32)(WORD8)(*(ptr_tab++)) * prod);
    }
  }

  ptr_tab = &(pstr_pvc_tabs->pvc_pred_coef_kb_3[idx_tab_2 * pstr_pvc_params->num_grp_sbr]);
  prod = pstr_pvc_tabs->scaling_coef[pstr_pvc_params->num_grp_core];

  for (ksg = 0; ksg < pstr_pvc_params->num_grp_sbr; ksg++) {
    ptr_sb_grp_energy_high[ksg] += (FLOAT32)(WORD8)(*(ptr_tab++)) * prod;
  }

  return;
}

static VOID ixheaace_pvc_packing(ixheaace_pvc_enc *pstr_pvc_enc, WORD32 const usac_indep_flag) {
  UWORD16 *ptr_pvc_bs;
  ixheaace_pvc_params *pstr_params = &pstr_pvc_enc->pvc_param;
  ixheaace_pvc_prv_frm_params *pstr_prv_params = &pstr_pvc_enc->pvc_prv_param;
  ixheaace_pvc_bs_info *pstr_pvc_bs_info = &pstr_pvc_enc->pvc_bs_info;

  ptr_pvc_bs = pstr_pvc_bs_info->pvc_id_bs;

  pstr_pvc_bs_info->ns_mode = pstr_params->ns_mode;

  if ((usac_indep_flag == 1) || (pstr_params->pvc_id[1] != pstr_prv_params->pvc_id)) {
    pstr_pvc_bs_info->grid_info[0] = 1;
    *(ptr_pvc_bs++) = pstr_params->pvc_id[0];
  } else {
    pstr_pvc_bs_info->grid_info[0] = 0;
  }

  if (pstr_params->pvc_id[8] == pstr_params->pvc_id[7]) {
    pstr_pvc_bs_info->div_mode = 0;
  } else {
    pstr_pvc_bs_info->div_mode = 4;
    pstr_pvc_bs_info->num_grid_info = 2;
    pstr_pvc_bs_info->grid_info[1] = 1;
    *(ptr_pvc_bs++) = pstr_params->pvc_id[8];
  }
  return;
}

static FLOAT32 ixheaace_pvc_calc_res(FLOAT32 *ptr_org, FLOAT32 *ptr_prd, UWORD8 ng_sb_sbr) {
  FLOAT32 residual = 0, diff;
  WORD32 band = 0;

  while (band < ng_sb_sbr) {
    diff = ptr_org[band] - ptr_prd[band];
    residual += diff * diff;
    band++;
  }
  residual = (FLOAT32)sqrt(residual);

  return residual;
}

VOID ixheaace_pvc_calc_grp_energy(ixheaace_pvc_enc *pstr_pvc_enc,
                                  FLOAT32 *ptr_sb_grp_energy_hi_ref) {
  WORD32 ts, res_init_flg = 1, pvc_id, i;
  FLOAT32 res_all[128] = {0}, res_min, sb_grp_energy_blwsbr[3], sb_grp_energy_hi[8], res = 0;
  UWORD16 *ptr_pvc_id, tmp = 0;

  ptr_pvc_id = pstr_pvc_enc->pvc_param.pvc_id;
  for (ts = 0; ts < IXHEAACE_ESBR_PVC_NUM_TS; ts++) {
    ixheaace_pvc_calc_grp_energy_below_sbr(pstr_pvc_enc, ts, sb_grp_energy_blwsbr);
    for (pvc_id = 0; pvc_id < pstr_pvc_enc->pvc_param.num_pvc_id; pvc_id++) {
      pstr_pvc_enc->pvc_param.pvc_id[ts] = (UWORD16)pvc_id;
      ixheaace_pvc_predict(pstr_pvc_enc, ts, sb_grp_energy_blwsbr, sb_grp_energy_hi);

      res = ixheaace_pvc_calc_res(
          &(ptr_sb_grp_energy_hi_ref[ts * pstr_pvc_enc->pvc_param.num_grp_sbr]), sb_grp_energy_hi,
          pstr_pvc_enc->pvc_param.num_grp_sbr);
      if (res_init_flg) {
        res_all[pvc_id] = res;
      } else {
        res_all[pvc_id] += res;
      }
    }

    res_init_flg = 0;
    if ((ts & (IXHEAACE_ESBR_PVC_NTS_GRP_ID - 1)) == (IXHEAACE_ESBR_PVC_NTS_GRP_ID - 1)) {
      res_min = IXHEAACE_ESBR_PVC_RESIDUAL_VAL;
      pvc_id = 0;
      while (pvc_id < pstr_pvc_enc->pvc_param.num_pvc_id) {
        if (res_all[pvc_id] < res_min) {
          tmp = (UWORD16)pvc_id;
          res_min = res_all[pvc_id];
        }
        pvc_id++;
      }
      for (i = 0; i < IXHEAACE_ESBR_PVC_NTS_GRP_ID; i++) {
        *(ptr_pvc_id++) = tmp;
      }
      res_init_flg = 1;
    }
  }

  for (ts = 0; ts < 15; ts++) {
    memcpy(&pstr_pvc_enc->sb_grp_energy[ts][0],
           &pstr_pvc_enc->sb_grp_energy[ts + IXHEAACE_ESBR_PVC_NUM_TS][0],
           IXHEAACE_ESBR_PVC_NUM_BANDS_CORE * sizeof(pstr_pvc_enc->sb_grp_energy[ts][0]));
  }
}

VOID ixheaace_pvc_calc_ref_ene_update_tabs(ixheaace_pvc_enc *pstr_pvc_enc,
                                           FLOAT32 *ptr_sb_grp_energy_hi_ref) {
  WORD32 ts, band;
  FLOAT32 sum, prev_sum = 0;
  pstr_pvc_enc->pvc_param.ns_mode = 0;

  switch (pstr_pvc_enc->pvc_param.pvc_mode) {
    case 1: {
      pstr_pvc_enc->pvc_param.time_smth_ts = IXHEAACE_ESBR_PVC_NUM_TS;
      pstr_pvc_enc->pvc_tabs.smoothing_coef = ixheaace_pvc_tabs.pvc_smth_win_ns_16;
      break;
    }
    case 2: {
      pstr_pvc_enc->pvc_param.time_smth_ts = 12;
      pstr_pvc_enc->pvc_tabs.smoothing_coef = ixheaace_pvc_tabs.pvc_smth_win_ns_12;
      break;
    }
    default: {
      // No assignment
      break;
    }
  }

  for (ts = 0; ts < IXHEAACE_ESBR_PVC_NUM_TS; ts++) {
    FLOAT32 *ptr_grp_energy =
        &(ptr_sb_grp_energy_hi_ref[ts * pstr_pvc_enc->pvc_param.num_grp_sbr]);
    sum = 0.0f;
    for (band = 0; band < pstr_pvc_enc->pvc_param.num_grp_sbr; band++) {
      sum += *ptr_grp_energy;

      *ptr_grp_energy = 10 * (FLOAT32)log10(*ptr_grp_energy);
      ptr_grp_energy++;
    }
    if (ts && (sum > prev_sum * IXHEAACE_ESBR_PVC_NS_MODE_PRD_THRS)) {
      pstr_pvc_enc->pvc_param.ns_mode = 1;
    }
    prev_sum = sum;
  }

  if (pstr_pvc_enc->pvc_param.ns_mode == 1) {
    switch (pstr_pvc_enc->pvc_param.pvc_mode) {
      case 1: {
        pstr_pvc_enc->pvc_param.time_smth_ts = 4;
        pstr_pvc_enc->pvc_tabs.smoothing_coef = ixheaace_pvc_tabs.pvc_smth_win_ns_4;
        break;
      }
      case 2: {
        pstr_pvc_enc->pvc_param.time_smth_ts = 3;
        pstr_pvc_enc->pvc_tabs.smoothing_coef = ixheaace_pvc_tabs.pvc_smth_win_ns_3;
        break;
      }
      default: {
        // No assignment
        break;
      }
    }
  }
}

IA_ERRORCODE ixheaace_pvc_encode_frame(ixheaace_pvc_enc *pstr_pvc_enc, UWORD8 pvc_mode,
                                       FLOAT32 *ptr_qmf_low, FLOAT32 *ptr_qmf_high,
                                       UWORD8 start_band, UWORD8 stop_band) {
  IA_ERRORCODE ret;
  FLOAT32 sb_grp_energy_hi_ref[IXHEAACE_ESBR_PVC_NUM_TS * 8];

  pstr_pvc_enc->pvc_param.pvc_mode = pvc_mode;
  /* PVC encoding process */
  if (pstr_pvc_enc->pvc_param.pvc_mode) {
    ret = ixheaace_set_pvc_mode_param(&(pstr_pvc_enc->pvc_param), &(pstr_pvc_enc->pvc_tabs));
    if (IA_NO_ERROR != ret) {
      return ret;
    }

    ixheaace_pvc_sb_grouping(pstr_pvc_enc, start_band, ptr_qmf_low,
                             (FLOAT32 *)pstr_pvc_enc->sb_grp_energy, 0);

    ixheaace_pvc_sb_grouping_ref(&(pstr_pvc_enc->pvc_param), start_band, stop_band, ptr_qmf_high,
                                 sb_grp_energy_hi_ref);
    ixheaace_pvc_calc_ref_ene_update_tabs(pstr_pvc_enc, sb_grp_energy_hi_ref);

    ixheaace_pvc_calc_grp_energy(pstr_pvc_enc, sb_grp_energy_hi_ref);

    ixheaace_pvc_packing(pstr_pvc_enc, pstr_pvc_enc->pvc_param.usac_indep_flag);
  }

  pstr_pvc_enc->pvc_prv_param.pvc_id =
      (pstr_pvc_enc->pvc_param.pvc_mode == 0)
          ? 0xFF
          : pstr_pvc_enc->pvc_param.pvc_id[IXHEAACE_ESBR_PVC_NUM_TS - 1];
  pstr_pvc_enc->pvc_prv_param.start_band = start_band;
  pstr_pvc_enc->pvc_prv_param.pvc_flag = (pstr_pvc_enc->pvc_param.pvc_mode == 0) ? 0 : 1;
  pstr_pvc_enc->pvc_prv_param.pvc_rate = pstr_pvc_enc->pvc_param.pvc_rate;

  return IA_NO_ERROR;
}
