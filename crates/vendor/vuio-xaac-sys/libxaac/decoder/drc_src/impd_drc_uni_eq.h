/******************************************************************************
 *
 * Copyright (C) 2015 The Android Open Source Project
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
#ifndef IMPD_DRC_UNI_EQ_H
#define IMPD_DRC_UNI_EQ_H

#ifndef COMPILE_FOR_DRC_ENCODER
#endif

#define EQ_CHANNEL_COUNT_MAX 8
#define EQ_AUDIO_DELAY_MAX 1024
#define EQ_FIR_FILTER_SIZE_MAX 128
#define EQ_SUBBAND_COUNT_MAX 256
#define EQ_INTERMEDIATE_2ND_ORDER_PARAMS_COUNT_MAX 32
#define EQ_INTERMEDIATE_PARAMETER_COUNT_MAX 32
#define EQ_FILTER_SECTION_COUNT_MAX 8
#define EQ_FILTER_ELEMENT_COUNT_MAX 4
#define EQ_FILTER_COUNT_MAX 4
#define MATCHING_PHASE_FILTER_COUNT_MAX 32

#define EQ_FILTER_DOMAIN_NONE 0
#define EQ_FILTER_DOMAIN_TIME (1 << 0)
#define EQ_FILTER_DOMAIN_SUBBAND (1 << 1)

#ifdef __cplusplus
extern "C" {
#endif

#ifdef __cplusplus
}
#endif
#endif
