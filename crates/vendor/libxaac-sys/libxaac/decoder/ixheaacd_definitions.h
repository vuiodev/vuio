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
#ifndef IXHEAACD_DEFINITIONS_H
#define IXHEAACD_DEFINITIONS_H

#define LIBNAME "IA_XHEAAC_DEC"
#define LIBVERSION "1.0"

#define LIB_APIVERSION_MAJOR 1
#define LIB_APIVERSION_MINOR 10

#define LIB_APIVERSION \
  IA_MAKE_VERSION_STR(LIB_APIVERSION_MAJOR, LIB_APIVERSION_MINOR)

#define IA_ENHAACPLUS_DEC_PERSIST_IDX (0)
#define IA_ENHAACPLUS_DEC_SCRATCH_IDX (1)
#define IA_ENHAACPLUS_DEC_INPUT_IDX (2)
#define IA_ENHAACPLUS_DEC_OUTPUT_IDX (3)

#define IA_MAX_PREROLL_FRAMES (4)
#define IA_MAX_OUTPUT_PCM_SIZE (3)
#define IA_MAX_USAC_CH (2)
#define IA_MAX_OUT_SAMPLES_PER_FRAME (4096)

#define IA_ENHAACPLUS_DEC_INP_BUF_SIZE (6144 / 8)

#define IA_ENHAACPLUS_DEC_SAMPLES_PER_FRAME (1024)

#define IA_ENHAACPLUS_DEC_OUT_BUF_SIZE                                     \
  (IA_MAX_USAC_CH * IA_MAX_PREROLL_FRAMES * IA_MAX_OUT_SAMPLES_PER_FRAME * \
   IA_MAX_OUTPUT_PCM_SIZE)

#define IA_ENHAACPLUS_DEC_MAX_CHANNEL (2)
#define IA_ENHAACPLUS_DEC_FRAME_LENGTH (1024)

#endif /* IXHEAACD_DEFINITIONS_H */
