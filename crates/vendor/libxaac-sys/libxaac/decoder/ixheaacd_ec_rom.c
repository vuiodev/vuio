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

#include <ixheaac_type_def.h>
#include "ixheaacd_ec_defines.h"

const WORD16 ia_ec_interpolation_fac[4] = {(0x4000), (0x4c1b), (0x5a82), (0x6ba2)};

const FLOAT32 ia_ec_fade_factors[MAX_FADE_FRAMES + 1] = {
    1.00000f, 0.875f, 0.750f, 0.625f, 0.500f, 0.375f, 0.250f, 0.125f, 0.00000f};

const WORD32 ia_ec_fade_factors_fix[MAX_FADE_FRAMES + 1] = {
    1073741824, 939524096, 805306368, 671088640, 536870912, 402653184, 268435456, 134217728, 0};

const FLOAT32 ixheaacd_exc_fade_fac[8] = { 0.8f, 0.7f, 0.6f, 0.5f, 0.4f, 0.3f, 0.2f, 0.1f };
