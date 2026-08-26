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

#pragma once

#define LIB_APIVERSION_MAJOR 1
#define LIB_APIVERSION_MINOR 10

#define LIB_APIVERSION IA_MAKE_VERSION_STR(LIB_APIVERSION_MAJOR, LIB_APIVERSION_MINOR)

/* Index to the different Memory Tables */
#define IA_ENHAACPLUSENC_PERSIST_IDX (0)
#define IA_ENHAACPLUSENC_SCRATCH_IDX (1)
#define IA_ENHAACPLUSENC_INPUT_IDX (2)
#define IA_ENHAACPLUSENC_OUTPUT_IDX (3)
