/******************************************************************************
 *                                                                            *
 * Copyright (C) 2018 The Android Open Source Project
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
#include <stdio.h>
#include <stdlib.h>
#include <math.h>
#include <string.h>
#include "ixheaac_type_def.h"
#include "ixheaac_sbr_const.h"

#include "ixheaacd_pvc_dec.h"
#include "ixheaacd_pvc_rom.h"

static VOID ixheaacd_pvc_sb_parsing(ia_pvc_data_struct *ptr_pvc_data,
                                    WORD16 first_bnd_idx, FLOAT32 *p_qmfh) {
  WORD32 ksg, k;
  WORD32 start_band, end_band;
  WORD32 time_slot;

  FLOAT32 *p_sbr_range_esg = &ptr_pvc_data->sbr_range_esg_arr[0];

  for (time_slot = 0; time_slot < PVC_NUM_TIME_SLOTS; time_slot++) {
    start_band = first_bnd_idx;
    end_band = start_band + ptr_pvc_data->nb_high_per_grp - 1;

    for (ksg = 0; ksg < ptr_pvc_data->nb_high; ksg++) {
      for (k = start_band; k <= end_band; k++) {
        p_qmfh[k] = (FLOAT32)pow(10.0f, (p_sbr_range_esg[ksg] / 10.0));
      }
      start_band += ptr_pvc_data->nb_high_per_grp;
      if (ksg >= ptr_pvc_data->nb_high - 2) {
        end_band = SBR_NUM_QMF_BANDS - 1;
      } else {
        end_band = start_band + ptr_pvc_data->nb_high_per_grp - 1;
        if (end_band >= SBR_NUM_QMF_BANDS - 1) {
          end_band = SBR_NUM_QMF_BANDS - 1;
        }
      }
    }
    p_sbr_range_esg = p_sbr_range_esg + 8;
    p_qmfh = p_qmfh + SBR_NUM_QMF_BANDS;
  }
  return;
}

static VOID ixheaacd_pvc_qmf_grouping(ia_pvc_data_struct *ptr_pvc_data,
                                      WORD16 first_bnd_idx, FLOAT32 *p_qmf_ener,
                                      WORD32 first_pvc_timeslot) {
  WORD32 ksg, time_slot, ib;
  WORD32 lbw, start_band, end_band;
  FLOAT32 esg;
  FLOAT32 *p_esg = (FLOAT32 *)ptr_pvc_data->esg;

  lbw = 8 / ptr_pvc_data->pvc_rate;

  for (time_slot = 0; time_slot < PVC_NUM_TIME_SLOTS; time_slot++) {
    for (ksg = 0; ksg < PVC_NB_LOW; ksg++) {
      start_band = first_bnd_idx - lbw * PVC_NB_LOW + lbw * ksg;
      end_band = start_band + lbw - 1;
      if (start_band >= 0) {
        esg = 0.0f;
        for (ib = start_band; ib <= end_band; ib++) {
          esg += p_qmf_ener[ib];
        }
        esg = esg / lbw;
      } else {
        esg = PVC_ESG_MIN_VAL;
      }

      if (esg > PVC_ESG_MIN_VAL) {
        p_esg[(time_slot + 16 - 1) * 3 + ksg] = 10 * ((FLOAT32)log10(esg));
      } else {
        p_esg[(time_slot + 16 - 1) * 3 + ksg] = PVC_10LOG10_ESG_MIN_VAL;
      }
    }
    p_qmf_ener = p_qmf_ener + SBR_NUM_QMF_BANDS_2;
  }

  if ((ptr_pvc_data->prev_pvc_flg == 0) ||
      ((first_bnd_idx * ptr_pvc_data->pvc_rate) !=
       (ptr_pvc_data->prev_first_bnd_idx * ptr_pvc_data->prev_pvc_rate))) {
    for (time_slot = 0; time_slot < 16 - 1 + first_pvc_timeslot; time_slot++) {
      for (ksg = 0; ksg < PVC_NB_LOW; ksg++) {
        p_esg[time_slot * 3 + ksg] =
            p_esg[(16 - 1 + first_pvc_timeslot) * 3 + ksg];
      }
    }
  }

  return;
}

static VOID ixheaacd_pvc_time_smoothing(ia_pvc_data_struct *ptr_pvc_data) {
  WORD32 time_slot;
  FLOAT32 *p_smooth_esg = (FLOAT32 *)&ptr_pvc_data->smooth_esg_arr[0];
  for (time_slot = 0; time_slot < PVC_NUM_TIME_SLOTS; time_slot++) {
    WORD32 ksg, time_slot_idx;
    FLOAT32 *p_esg = (FLOAT32 *)&ptr_pvc_data->esg[time_slot + 16 - 1][2];
    FLOAT32 *p_smth_wind_coeff = (FLOAT32 *)&ptr_pvc_data->p_smth_wind_coeff[0];
    memset(p_smooth_esg, (WORD32)0.f, sizeof(FLOAT32) * PVC_NB_LOW);
    for (time_slot_idx = 0; time_slot_idx < ptr_pvc_data->num_time_slots;
         time_slot_idx++) {
      ksg = PVC_NB_LOW - 1;
      for (; ksg >= 0; ksg--) {
        p_smooth_esg[ksg] += (*p_esg) * (*p_smth_wind_coeff);
        p_esg--;
      }
      p_smth_wind_coeff++;
    }
    p_smooth_esg = p_smooth_esg + 3;
  }
  return;
}

static VOID ixheaacd_pvc_pred_env_sf(ia_pvc_data_struct *ptr_pvc_data) {
  WORD32 ksg, kb;
  WORD32 tab_1_index, tab_2_index;
  WORD32 time_slot;
  WORD8 *pred_tab_1, *pred_tab_2;

  FLOAT32 temp;
  FLOAT32 *p_smooth_esg = &ptr_pvc_data->smooth_esg_arr[0];
  FLOAT32 *p_sbr_range_esg = &ptr_pvc_data->sbr_range_esg_arr[0];

  for (time_slot = 0; time_slot < PVC_NUM_TIME_SLOTS; time_slot++) {
    tab_2_index = ptr_pvc_data->pvc_id[time_slot];

    if (tab_2_index < ptr_pvc_data->p_pvc_id_boundary[0]) {
      tab_1_index = 0;
    } else if (tab_2_index < ptr_pvc_data->p_pvc_id_boundary[1]) {
      tab_1_index = 1;
    } else {
      tab_1_index = 2;
    }

    pred_tab_1 =
        (WORD8 *)(&(ptr_pvc_data->p_pred_coeff_tab_1[tab_1_index * PVC_NB_LOW *
                                                     ptr_pvc_data->nb_high]));
    pred_tab_2 = (WORD8 *)(&(
        ptr_pvc_data->p_pred_coeff_tab_2[tab_2_index * ptr_pvc_data->nb_high]));

    for (ksg = 0; ksg < ptr_pvc_data->nb_high; ksg++) {
      temp =
          (FLOAT32)(WORD8)(*(pred_tab_2++)) * ptr_pvc_data->p_q_fac[PVC_NB_LOW];
      p_sbr_range_esg[ksg] = temp;
    }
    for (kb = 0; kb < PVC_NB_LOW; kb++) {
      for (ksg = 0; ksg < ptr_pvc_data->nb_high; ksg++) {
        temp = (FLOAT32)(WORD8)(*(pred_tab_1++)) * ptr_pvc_data->p_q_fac[kb];
        p_sbr_range_esg[ksg] += temp * p_smooth_esg[kb];
      }
    }
    p_smooth_esg = p_smooth_esg + 3;
    p_sbr_range_esg = p_sbr_range_esg + 8;
  }

  return;
}

WORD32 ixheaacd_pvc_process(ia_pvc_data_struct *ptr_pvc_data,
                            WORD16 first_bnd_idx, WORD32 first_pvc_timeslot,
                            FLOAT32 *p_qmf_ener, FLOAT32 *p_qmfh) {
  switch (ptr_pvc_data->pvc_mode) {
    case 1:
      ptr_pvc_data->nb_high = PVC_NB_HIGH_MODE1;
      ptr_pvc_data->nb_high_per_grp = 8 / ptr_pvc_data->pvc_rate;
      ptr_pvc_data->p_pred_coeff_tab_1 =
          (WORD8 *)ixheaacd_pred_coeff_table_1_mode_1;
      ptr_pvc_data->p_pred_coeff_tab_2 =
          (WORD8 *)ixheaacd_pred_coeff_table_2_mode_1;
      ptr_pvc_data->p_pvc_id_boundary =
          (UWORD8 *)ixheaacd_pred_coeff_pvc_id_boundaries_1;
      ptr_pvc_data->p_q_fac = (FLOAT32 *)ixheaacd_q_factor_table_mode_1;
      if (ptr_pvc_data->ns_mode) {
        ptr_pvc_data->num_time_slots = 4;
        ptr_pvc_data->p_smth_wind_coeff =
            (FLOAT32 *)ixheaacd_pvc_smoothing_wind_tab_ns4;
      } else {
        ptr_pvc_data->num_time_slots = 16;
        ptr_pvc_data->p_smth_wind_coeff =
            (FLOAT32 *)ixheaacd_pvc_smoothing_wind_tab_ns16;
      }
      break;
    case 2:
      ptr_pvc_data->nb_high = PVC_NB_HIGH_MODE2;
      ptr_pvc_data->nb_high_per_grp = 12 / ptr_pvc_data->pvc_rate;
      ptr_pvc_data->p_pred_coeff_tab_1 =
          (WORD8 *)ixheaacd_pred_coeff_table_1_mode_2;
      ptr_pvc_data->p_pred_coeff_tab_2 =
          (WORD8 *)ixheaacd_pred_coeff_table_2_mode_2;
      ptr_pvc_data->p_pvc_id_boundary =
          (UWORD8 *)ixheaacd_pred_coeff_pvc_id_boundaries_2;
      ptr_pvc_data->p_q_fac = (FLOAT32 *)ixheaacd_q_factor_table_mode_2;
      if (ptr_pvc_data->ns_mode) {
        ptr_pvc_data->num_time_slots = 3;
        ptr_pvc_data->p_smth_wind_coeff =
            (FLOAT32 *)ixheaacd_pvc_smoothing_wind_tab_ns3;
      } else {
        ptr_pvc_data->num_time_slots = 12;
        ptr_pvc_data->p_smth_wind_coeff =
            (FLOAT32 *)ixheaacd_pvc_smoothing_wind_tab_ns12;
      }
      break;
    default:
      return -1;
  }
  ptr_pvc_data->prev_pvc_id = ptr_pvc_data->pvc_id[PVC_NUM_TIME_SLOTS - 1];

  ixheaacd_pvc_qmf_grouping(ptr_pvc_data, first_bnd_idx, p_qmf_ener,
                            first_pvc_timeslot);

  ixheaacd_pvc_time_smoothing(ptr_pvc_data);

  ixheaacd_pvc_pred_env_sf(ptr_pvc_data);

  ixheaacd_pvc_sb_parsing(ptr_pvc_data, first_bnd_idx, p_qmfh);

  memcpy((FLOAT32 *)(&ptr_pvc_data->esg[0][0]),
         (FLOAT32 *)(&ptr_pvc_data->esg[PVC_NUM_TIME_SLOTS][0]),
         sizeof(FLOAT32) * (PVC_NUM_TIME_SLOTS - 1) * 3);

  return 0;
}
