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
#define IXHEAACE_MPS_SAC_DATA_TYPE_CLD (0x0)
#define IXHEAACE_MPS_SAC_DATA_TYPE_ICC (0x1)
#define IXHEAACE_MPS_SAC_DATA_TYPE_CPC (0x2)

#define IXHEAACE_MPS_SAC_DIRECTION_BACKWARDS (0x0)
#define IXHEAACE_MPS_SAC_DIRECTION_FORWARDS (0x1)

#define IXHEAACE_MPS_SAC_DIFF_FREQ (0x0)
#define IXHEAACE_MPS_SAC_DIFF_TIME (0x1)

#define IXHEAACE_MPS_SAC_FREQ_PAIR (0x0)

#define IXHEAACE_MPS_SAC_HUFF_1D (0x0)
#define IXHEAACE_MPS_SAC_HUFF_2D (0x1)

#ifndef PI
#define PI (3.1415926535897932f)
#endif
#define EIGHT_SHORT_SEQUENCE (2)
#define MAX_NUM_BINS (56)
#define MAX_NUM_PARAMS (7)
#define MAX_NUM_OUTPUTCHANNELS (16)
#define MAX_TIME_SLOTS (64)
