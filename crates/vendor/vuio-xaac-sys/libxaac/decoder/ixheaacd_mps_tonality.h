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
#ifndef IXHEAACD_MPS_TONALITY_H
#define IXHEAACD_MPS_TONALITY_H

VOID ixheaacd_init_tonality(ia_heaac_mps_state_struct *pstr_mps_state);

VOID ixheaacd_measure_tonality(ia_heaac_mps_state_struct *pstr_mps_state, WORD32 *tonality);

#endif /* IXHEAACD_MPS_TONALITY_H */
