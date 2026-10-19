/******************************************************************************
 *
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
#ifndef IXHEAACD_MPS_TP_PROCESS_H
#define IXHEAACD_MPS_TP_PROCESS_H

#define STP_LPF_COEFF1_FIX (31130)
#define STP_LPF_COEFF2_FIX (14746)
#define ONE_MINUS_STP_LPF_COEFF2 (18022)
#define STP_SCALE_LIMIT_FIX (92406)
#define ONE_BY_STP_SCALE_LIMIT (11620)
#define QMF_TO_HYB_OFFSET (7)
#define DMX_OFFSET (48)
#define DMX_OFFSET_MINUS_ONE (47)
#define QMF_OUT_START_IDX (5)
#define QMF_OUT_OFFSET (20)
#define HYBRID_BAND_BORDER (12)
#define FIVE (5)

VOID ixheaacd_tp_process(ia_heaac_mps_state_struct *pstr_mps_state);

#endif /* IXHEAACD_MPS_TP_PROCESS_H */
