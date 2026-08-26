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
#ifndef IXHEAACD_DEFINES_H
#define IXHEAACD_DEFINES_H

#define MAX_WINDOWS 8
#define MAX_ORDER 31
#define MAX_ORDER_LONG 12
#define MAX_FILTERS 3

#define MAX_BINS_LONG 1024
#define MAX_BINS_SHORT 128
#define MAX_SCALE_FACTOR_BANDS_SHORT 16
#define MAX_SCALE_FACTOR_BANDS_LONG (52)

#define ZERO_HCB 0

#define NOISE_OFFSET 90

#define ESC_HCB 11
#define NOISE_HCB 13
#define INTENSITY_HCB2 14
#define INTENSITY_HCB 15
#define BOOKSCL 12

#define CHANNELS 2

#define SIZE01 (MAX_BINS_LONG / 16)
#define SIZE07 7 * SIZE01

typedef struct { WORD32 sampling_frequency; } ia_sampling_rate_info_struct;

#define USAC_MAX_SAMPLE_RATE (96000)

#endif
