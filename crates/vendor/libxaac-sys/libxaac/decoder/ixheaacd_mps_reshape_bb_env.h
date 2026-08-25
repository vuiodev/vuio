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
#ifndef IXHEAACD_MPS_RESHAPE_BB_ENV_H
#define IXHEAACD_MPS_RESHAPE_BB_ENV_H

#define INP_DRY_WET (0)
#define INP_DMX (1)

#define ALPHA_Q15 (32649)
#define BETA_Q15 (31600)
#define ONE_MINUS_ALPHA_Q16 (238)
#define ONE_MINUS_BETA_Q16 (2336)

#define SHAPE_STP (1)
#define SHAPE_GES (2)

VOID ixheaacd_init_bb_env(ia_heaac_mps_state_struct *pstr_mps_state);

VOID ixheaacd_reshape_bb_env(ia_heaac_mps_state_struct *pstr_mps_state);

VOID ixheaacd_pre_reshape_bb_env(ia_heaac_mps_state_struct *pstr_mps_state);

#endif /* IXHEAACD_MPS_RESHAPE_BB_ENV_H */
