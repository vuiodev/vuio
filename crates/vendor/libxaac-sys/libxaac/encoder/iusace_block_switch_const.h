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
#define TRANS_FAC 8

#define BLK_SWITCH_WIN 8
#define BLK_SWITCH_FILT_LEN 2

/* Block types */
#define LONG_WINDOW 0
#define START_WINDOW 1
#define SHORT_WINDOW 2
#define STOP_WINDOW 3

/* Window shapes */
#define SINE_WINDOW 0
#define KBD_WINDOW 1

#define MAXIMUM_NO_OF_GROUPS 4

#define ACC_WINDOW_NRG_FAC 0.3f
#define ONE_MINUS_ACC_WINDOW_NRG_FAC 0.7f
#define INV_ATTACK_RATIO_HIGH_BR 0.1f
#define INV_ATTACK_RATIO_LOW_BR 0.056f
#define USAC_MIN_ATTACK_NRG 1e+6
#define CLIP_ENERGY_VALUE_LONG (1.0e9f)
#define CLIP_ENERGY_VALUE_SHORT (15625000.0f)
