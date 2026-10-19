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
#ifndef IXHEAACD_PVC_ROM_H
#define IXHEAACD_PVC_ROM_H

extern const FLOAT32 ixheaacd_pvc_smoothing_wind_tab_ns4[4];
extern const FLOAT32 ixheaacd_pvc_smoothing_wind_tab_ns16[16];

extern const FLOAT32 ixheaacd_pvc_smoothing_wind_tab_ns3[3];
extern const FLOAT32 ixheaacd_pvc_smoothing_wind_tab_ns12[12];

extern const UWORD8
    ixheaacd_pred_coeff_pvc_id_boundaries_1[PVC_ID_NUM_GROUPS - 1];

extern const FLOAT32 ixheaacd_q_factor_table_mode_1[PVC_NB_LOW + 1];

extern const WORD8 ixheaacd_pred_coeff_table_1_mode_1[PVC_ID_NUM_GROUPS]
                                                     [PVC_NB_LOW]
                                                     [PVC_NB_HIGH_MODE1];

extern const WORD8 ixheaacd_pred_coeff_table_2_mode_1[PVC_NB_HIGH]
                                                     [PVC_NB_HIGH_MODE1];

extern const UWORD8
    ixheaacd_pred_coeff_pvc_id_boundaries_2[PVC_ID_NUM_GROUPS - 1];

extern const FLOAT32 ixheaacd_q_factor_table_mode_2[PVC_NB_LOW + 1];

extern const WORD8 ixheaacd_pred_coeff_table_1_mode_2[PVC_ID_NUM_GROUPS]
                                                     [PVC_NB_LOW]
                                                     [PVC_NB_HIGH_MODE2];

extern const WORD8 ixheaacd_pred_coeff_table_2_mode_2[PVC_NB_HIGH]
                                                     [PVC_NB_HIGH_MODE2];

#endif
