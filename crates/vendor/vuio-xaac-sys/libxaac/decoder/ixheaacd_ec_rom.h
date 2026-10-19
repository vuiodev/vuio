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
#ifndef IXHEAACD_EC_ROM_H
#define IXHEAACD_EC_ROM_H
extern const WORD16 ia_ec_interpolation_fac[4];
extern const FLOAT32 ia_ec_fade_factors[MAX_FADE_FRAMES + 1];
extern const WORD32 ia_ec_fade_factors_fix[MAX_FADE_FRAMES + 1];
extern const FLOAT32 ixheaacd_exc_fade_fac[8];
extern const FLOAT32 lsf_init[ORDER];
extern const FLOAT32 ixheaacd_gamma_table[17];
#endif /* IXHEAACD_EC_ROM_H */
