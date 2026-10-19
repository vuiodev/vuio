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
#ifndef IXHEAACD_PNS_H
#define IXHEAACD_PNS_H

#define PNS_BAND_FLAGS_SIZE 16
#define PNS_BAND_FLAGS_MASK \
  ((WORD16)((((WORD16)1) << PNS_BAND_FLAGS_SHIFT) - 1))
#define PNS_BAND_FLAGS_SHIFT 3

#define PNS_SCALE_MANT_TAB_SIZE 4
#define PNS_SCALE_MANT_TAB_MASK (PNS_SCALE_MANT_TAB_SIZE - 1)
#define PNS_SCALE_MANT_TAB_SCALING -4

#define PNS_SCALEFACTOR_SCALING 2

#define PNS_SPEC_SCALE 3

typedef struct {
  UWORD8 correlated[PNS_BAND_FLAGS_SIZE];
  WORD32 random_vector[PNS_BAND_FLAGS_SIZE * 8];
} ia_pns_correlation_info_struct;

typedef struct {
  WORD32 current_seed;
  WORD16 pns_frame_number;
} ia_pns_rand_vec_struct;

typedef struct {
  UWORD8 pns_used[PNS_BAND_FLAGS_SIZE * 8];
  WORD16 noise_energy;
  UWORD16 pns_active;
} ia_pns_info_struct;

#endif /* #ifndef IXHEAACD_PNS_H */
