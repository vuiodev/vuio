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
#ifndef IXHEAACD_EC_DEFINES_H
#define IXHEAACD_EC_DEFINES_H

#define NO_TRANSITION (0)
#define TRANS_SHORT_LONG (1)

#define FRAME_OKAY (0)
#define FRAME_CONCEAL_SINGLE (1)
#define FRAME_FADE (2)
#define FRAME_MUTE (3)
#define MAX_FADE_FRAMES (8)

#define MAX_SFB_EC (51)
#define MAX_SPEC_SCALE_LEN (8)
#define MAX_SPEC_SCALE_LEN_EC (128)

#define BETA (0.25f)
#define ONE_BETA (0.75f)
#define BFI_FAC (0.90f)
#define ONE_BFI_FAC (0.10f)

#define FRAME_OK (0)
#define FRAME_ERROR (1)
#define FRAME_ERROR_ALLSLOTS (2)

#define CONCEAL_NOT_DEFINED ((UWORD8)-1)

#endif /* IXHEAACD_EC_DEFINES_H */
