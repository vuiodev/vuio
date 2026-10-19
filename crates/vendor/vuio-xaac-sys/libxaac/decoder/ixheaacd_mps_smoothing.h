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
#ifndef IXHEAACD_MPS_SMOOTHING_H
#define IXHEAACD_MPS_SMOOTHING_H

#define SMG_TIME_64 (64)
#define SMG_TIME_128 (128)
#define SMG_TIME_256 (256)
#define SMG_TIME_512 (512)

VOID ixheaacd_smooth_m1m2(ia_heaac_mps_state_struct *pstr_mps_state);

#endif /* IXHEAACD_MPS_SMOOTHING_H */
