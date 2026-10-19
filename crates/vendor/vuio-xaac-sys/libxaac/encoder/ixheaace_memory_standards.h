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

/*****************************************************************************/
/* Constant hash defines                                                     */
/*****************************************************************************/

/* ittiam standard memory types to be used inter frames */
#define IA_MEMTYPE_PERSIST 0x00
/* read write, to be used intra frames */
#define IA_MEMTYPE_SCRATCH 0x01

/* ittiam standard memory priorities */
#define IA_MEMPRIORITY_ANYWHERE 0x00
#define IA_MEMPRIORITY_HIGH 0x05

/* ittiam standard memory placements */
/* placement is defined by 64 bits */

#define IA_MEMPLACE_EXT_RAM_0 0x010000

/* the simple common PC RAM */