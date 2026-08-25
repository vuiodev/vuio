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
#ifndef IXHEAACD_AUDIOOBJTYPES_H
#define IXHEAACD_AUDIOOBJTYPES_H

typedef enum AUDIO_OBJECT_TYPE {
  AOT_NULL_OBJECT = 0,
  AOT_AAC_MAIN = 1,
  AOT_AAC_LC = 2,
  AOT_AAC_SSR = 3,
  AOT_AAC_LTP = 4,
  AOT_SBR = 5,
  AOT_AAC_SCAL = 6,
  AOT_TWIN_VQ = 7,
  AOT_CELP = 8,
  AOT_HVXC = 9,
  AOT_RSVD_10 = 10,
  AOT_RSVD_11 = 11,

  AOT_TTSI = 12,
  AOT_MAIN_SYNTH = 13,
  AOT_WAV_TAB_SYNTH = 14,
  AOT_GEN_MIDI = 15,
  AOT_ALG_SYNTH_AUD_FX = 16,
  AOT_ER_AAC_LC = 17,
  AOT_RSVD_18 = 18,
  AOT_ER_AAC_LTP = 19,
  AOT_ER_AAC_SCAL = 20,
  AOT_ER_TWIN_VQ = 21,
  AOT_ER_BSAC = 22,
  AOT_ER_AAC_LD = 23,
  AOT_ER_CELP = 24,
  AOT_ER_HVXC = 25,
  AOT_ER_HILN = 26,
  AOT_ER_PARA = 27,
  AOT_RSVD_28 = 28,
  AOT_PS = 29,
  AOT_RSVD_30 = 30,
  AOT_RSVD_31 = 31,
  AOT_ESC = 31,
  AOT_ER_AAC_ELD = 39,

  AOT_USAC = 42

} AUDIO_OBJECT_TYPE;

#define ER_OBJECT_START 17

#endif
