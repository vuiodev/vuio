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
#ifndef IXHEAACD_MPS_APPLY_COMMON_H
#define IXHEAACD_MPS_APPLY_COMMON_H

VOID ixheaacd_dec_interp_umx(WORD32 m[MAX_PARAMETER_SETS][MAX_PARAMETER_BANDS], WORD32 *ptr_r,
                             WORD32 *ptr_m_prev, ia_heaac_mps_state_struct *pstr_mps_state);

VOID ixheaacd_apply_abs_kernels(WORD32 *ptr_r_in, WORD32 *ptr_r_out, SIZE_T *ptr_params);

#endif /* IXHEAACD_MPS_APPLY_COMMON_H */
