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
#ifndef IXHEAACD_PVC_DEC_H
#define IXHEAACD_PVC_DEC_H

typedef struct {
  UWORD8 pvc_mode;
  UWORD8 div_mode;
  UWORD8 ns_mode;
  UWORD8 num_time_slots;
  UWORD8 nb_high;
  UWORD8 nb_high_per_grp;
  UWORD8 prev_pvc_flg;
  UWORD8 prev_pvc_mode;
  UWORD8 pvc_rate;
  UWORD8 prev_pvc_rate;
  WORD16 dummy;

  UWORD16 pvc_id[PVC_NUM_TIME_SLOTS];
  UWORD16 prev_pvc_id;
  WORD8 *p_pred_coeff_tab_1;
  WORD8 *p_pred_coeff_tab_2;
  UWORD8 *p_pvc_id_boundary;
  WORD16 prev_first_bnd_idx;
  WORD32 prev_sbr_mode;
  FLOAT32 *p_smth_wind_coeff;
  FLOAT32 *p_q_fac;
  FLOAT32 esg[2 * PVC_NUM_TIME_SLOTS - 1][3];
  FLOAT32 smooth_esg_arr[PVC_NUM_TIME_SLOTS * 3];
  FLOAT32 sbr_range_esg_arr[PVC_NUM_TIME_SLOTS * 8];

} ia_pvc_data_struct;

WORD32 ixheaacd_pvc_process(ia_pvc_data_struct *ptr_pvc_data,
                            WORD16 first_bnd_idx, WORD32 first_pvc_timelost,
                            FLOAT32 *a_qmfl, FLOAT32 *a_qmfh);

#endif /* IXHEAACD_PVC_DEC_H */
