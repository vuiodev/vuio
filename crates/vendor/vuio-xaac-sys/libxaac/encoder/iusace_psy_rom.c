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

#include "iusace_type_def.h"
#include "iusace_cnst.h"
#include "iusace_tns_usac.h"
#include "iusace_psy_mod.h"
#include "iusace_psy_utils.h"

const WORD16 iusace_sfb_96_1024[] = {4,   8,   12,  16,  20,  24,  28,  32,  36,  40,  44,
                                     48,  52,  56,  64,  72,  80,  88,  96,  108, 120, 132,
                                     144, 156, 172, 188, 212, 240, 276, 320, 384, 448, 512,
                                     576, 640, 704, 768, 832, 896, 960, 1024};

const WORD16 iexheaac_sfb_96_768[] = {
    4,   8,   12,  16,  20,  24,  28,  32,  36,  40,  44,  48,  52,  56,  64,  72,  80,  88, 96,
    108, 120, 132, 144, 156, 172, 188, 212, 240, 276, 320, 384, 448, 512, 576, 640, 704, 768};

const WORD16 iusace_sfb_96_128[] = {4, 8, 12, 16, 20, 24, 32, 40, 48, 64, 92, 128};

const WORD16 iexheaac_sfb_96_96[] = {4, 8, 12, 16, 20, 24, 32, 40, 48, 64, 92, 96};

const WORD16 iusace_sfb_64_1024[] = {4,   8,   12,  16,  20,  24,  28,  32,  36,  40,  44,  48,
                                     52,  56,  64,  72,  80,  88,  100, 112, 124, 140, 156, 172,
                                     192, 216, 240, 268, 304, 344, 384, 424, 464, 504, 544, 584,
                                     624, 664, 704, 744, 784, 824, 864, 904, 944, 984, 1024};

const WORD16 iusace_sfb_64_768[] = {4,   8,   12,  16,  20,  24,  28,  32,  36,  40,  44,
                                    48,  52,  56,  64,  72,  80,  88,  100, 112, 124, 140,
                                    156, 172, 192, 216, 240, 268, 304, 344, 384, 424, 464,
                                    504, 544, 584, 624, 664, 704, 744, 768};

const WORD16 iusace_sfb_64_128[] = {4, 8, 12, 16, 20, 24, 32, 40, 48, 64, 92, 128};

const WORD16 iusace_sfb_64_96[] = {4, 8, 12, 16, 20, 24, 32, 40, 48, 64, 92, 96};

const WORD16 iusace_sfb_48_1024[] = {
    4,   8,   12,  16,  20,  24,  28,  32,  36,  40,  48,  56,  64,  72,  80,  88,  96,
    108, 120, 132, 144, 160, 176, 196, 216, 240, 264, 292, 320, 352, 384, 416, 448, 480,
    512, 544, 576, 608, 640, 672, 704, 736, 768, 800, 832, 864, 896, 928, 1024};

const WORD16 iexheaac_sfb_48_768[] = {4,   8,   12,  16,  20,  24,  28,  32,  36,  40,  48,
                                      56,  64,  72,  80,  88,  96,  108, 120, 132, 144, 160,
                                      176, 196, 216, 240, 264, 292, 320, 352, 384, 416, 448,
                                      480, 512, 544, 576, 608, 640, 672, 704, 736, 768};

const WORD16 iusace_sfb_48_128[] = {4, 8, 12, 16, 20, 28, 36, 44, 56, 68, 80, 96, 112, 128};

const WORD16 iexheaac_sfb_48_96[] = {4, 8, 12, 16, 20, 28, 36, 44, 56, 68, 80, 96};

const WORD16 iusace_sfb_32_1024[] = {
    4,   8,   12,  16,  20,  24,  28,  32,  36,  40,  48,  56,  64,  72,  80,  88,  96,
    108, 120, 132, 144, 160, 176, 196, 216, 240, 264, 292, 320, 352, 384, 416, 448, 480,
    512, 544, 576, 608, 640, 672, 704, 736, 768, 800, 832, 864, 896, 928, 960, 992, 1024};

const WORD16 iexheaac_sfb_32_768[] = {4,   8,   12,  16,  20,  24,  28,  32,  36,  40,  48,
                                      56,  64,  72,  80,  88,  96,  108, 120, 132, 144, 160,
                                      176, 196, 216, 240, 264, 292, 320, 352, 384, 416, 448,
                                      480, 512, 544, 576, 608, 640, 672, 704, 736, 768};

const WORD16 iusace_sfb_24_1024[] = {4,   8,   12,  16,  20,  24,  28,  32,  36,  40,  44,  52,
                                     60,  68,  76,  84,  92,  100, 108, 116, 124, 136, 148, 160,
                                     172, 188, 204, 220, 240, 260, 284, 308, 336, 364, 396, 432,
                                     468, 508, 552, 600, 652, 704, 768, 832, 896, 960, 1024};

const WORD16 iexheaac_sfb_24_768[] = {4,   8,   12,  16,  20,  24,  28,  32,  36,  40,  44,
                                      52,  60,  68,  76,  84,  92,  100, 108, 116, 124, 136,
                                      148, 160, 172, 188, 204, 220, 240, 260, 284, 308, 336,
                                      364, 396, 432, 468, 508, 552, 600, 652, 704, 768};

const WORD16 iusace_sfb_24_128[] = {4, 8, 12, 16, 20, 24, 28, 36, 44, 52, 64, 76, 92, 108, 128};

const WORD16 iexheaac_sfb_24_96[] = {4, 8, 12, 16, 20, 24, 28, 36, 44, 52, 64, 76, 92, 96};

const WORD16 iusace_sfb_16_1024[] = {8,   16,  24,  32,  40,  48,  56,  64,  72,  80,  88,
                                     100, 112, 124, 136, 148, 160, 172, 184, 196, 212, 228,
                                     244, 260, 280, 300, 320, 344, 368, 396, 424, 456, 492,
                                     532, 572, 616, 664, 716, 772, 832, 896, 960, 1024};

const WORD16 iexheaac_sfb_16_768[] = {8,   16,  24,  32,  40,  48,  56,  64,  72,  80,
                                      88,  100, 112, 124, 136, 148, 160, 172, 184, 196,
                                      212, 228, 244, 260, 280, 300, 320, 344, 368, 396,
                                      424, 456, 492, 532, 572, 616, 664, 716, 768};

const WORD16 iusace_sfb_16_128[] = {4, 8, 12, 16, 20, 24, 28, 32, 40, 48, 60, 72, 88, 108, 128};

const WORD16 iexheaac_sfb_16_96[] = {4, 8, 12, 16, 20, 24, 28, 32, 40, 48, 60, 72, 88, 96};

const WORD16 iusace_sfb_8_1024[] = {12,  24,  36,  48,  60,  72,  84,  96,  108, 120,
                                    132, 144, 156, 172, 188, 204, 220, 236, 252, 268,
                                    288, 308, 328, 348, 372, 396, 420, 448, 476, 508,
                                    544, 580, 620, 664, 712, 764, 820, 880, 944, 1024};

const WORD16 iusace_sfb_8_128[] = {4, 8, 12, 16, 20, 24, 28, 36, 44, 52, 60, 72, 88, 108, 128};

const WORD16 iusace_sfb_8_768[] = {
    12,  24,  36,  48,  60,  72,  84,  96,  108, 120, 132, 144, 156, 172, 188, 204, 220, 236, 252,
    268, 288, 308, 328, 348, 372, 396, 420, 448, 476, 508, 544, 580, 620, 664, 712, 764, 768};

const WORD16 iusace_sfb_8_96[] = {4, 8, 12, 16, 20, 24, 28, 36, 44, 52, 60, 72, 88, 96};

ia_sfb_info_struct iusace_sfb_info_1024[12] = {
    {8000, 40, 15, iusace_sfb_8_1024, iusace_sfb_8_128, {0}, {0}},
    {11025, 43, 15, iusace_sfb_16_1024, iusace_sfb_16_128, {0}, {0}},
    {12000, 43, 15, iusace_sfb_16_1024, iusace_sfb_16_128, {0}, {0}},
    {16000, 43, 15, iusace_sfb_16_1024, iusace_sfb_16_128, {0}, {0}},
    {22050, 47, 15, iusace_sfb_24_1024, iusace_sfb_24_128, {0}, {0}},
    {24000, 47, 15, iusace_sfb_24_1024, iusace_sfb_24_128, {0}, {0}},
    {32000, 51, 14, iusace_sfb_32_1024, iusace_sfb_48_128, {0}, {0}},
    {44100, 49, 14, iusace_sfb_48_1024, iusace_sfb_48_128, {0}, {0}},
    {48000, 49, 14, iusace_sfb_48_1024, iusace_sfb_48_128, {0}, {0}},
    {64000, 47, 12, iusace_sfb_64_1024, iusace_sfb_64_128, {0}, {0}},
    {88200, 41, 12, iusace_sfb_64_1024, iusace_sfb_64_128, {0}, {0}},
    {96000, 41, 12, iusace_sfb_96_1024, iusace_sfb_96_128, {0}, {0}}};

#define ARRAY_SIZE(a) (sizeof(a) / sizeof(a[0]))

ia_sfb_info_struct iusace_sfb_info_768[12] = {{8000,
                                               ARRAY_SIZE(iusace_sfb_8_768),
                                               ARRAY_SIZE(iusace_sfb_8_96),
                                               iusace_sfb_8_768,
                                               iusace_sfb_8_96,
                                               {0},
                                               {0}},
                                              {11025,
                                               ARRAY_SIZE(iexheaac_sfb_16_768),
                                               ARRAY_SIZE(iexheaac_sfb_16_96),
                                               iexheaac_sfb_16_768,
                                               iexheaac_sfb_16_96,
                                               {0},
                                               {0}},
                                              {12000,
                                               ARRAY_SIZE(iexheaac_sfb_16_768),
                                               ARRAY_SIZE(iexheaac_sfb_16_96),
                                               iexheaac_sfb_16_768,
                                               iexheaac_sfb_16_96,
                                               {0},
                                               {0}},
                                              {16000,
                                               ARRAY_SIZE(iexheaac_sfb_16_768),
                                               ARRAY_SIZE(iexheaac_sfb_16_96),
                                               iexheaac_sfb_16_768,
                                               iexheaac_sfb_16_96,
                                               {0},
                                               {0}},
                                              {22050,
                                               ARRAY_SIZE(iexheaac_sfb_24_768),
                                               ARRAY_SIZE(iexheaac_sfb_24_96),
                                               iexheaac_sfb_24_768,
                                               iexheaac_sfb_24_96,
                                               {0},
                                               {0}},
                                              {24000,
                                               ARRAY_SIZE(iexheaac_sfb_24_768),
                                               ARRAY_SIZE(iexheaac_sfb_24_96),
                                               iexheaac_sfb_24_768,
                                               iexheaac_sfb_24_96,
                                               {0},
                                               {0}},
                                              {32000,
                                               ARRAY_SIZE(iexheaac_sfb_48_768),
                                               ARRAY_SIZE(iexheaac_sfb_48_96),
                                               iexheaac_sfb_48_768,
                                               iexheaac_sfb_48_96,
                                               {0},
                                               {0}},
                                              {44100,
                                               ARRAY_SIZE(iexheaac_sfb_48_768),
                                               ARRAY_SIZE(iexheaac_sfb_48_96),
                                               iexheaac_sfb_48_768,
                                               iexheaac_sfb_48_96,
                                               {0},
                                               {0}},
                                              {48000,
                                               ARRAY_SIZE(iexheaac_sfb_48_768),
                                               ARRAY_SIZE(iexheaac_sfb_48_96),
                                               iexheaac_sfb_48_768,
                                               iexheaac_sfb_48_96,
                                               {0},
                                               {0}},
                                              {64000,
                                               ARRAY_SIZE(iusace_sfb_64_768),
                                               ARRAY_SIZE(iusace_sfb_64_96),
                                               iusace_sfb_64_768,
                                               iusace_sfb_64_96,
                                               {0},
                                               {0}},
                                              {88200,
                                               ARRAY_SIZE(iexheaac_sfb_96_768),
                                               ARRAY_SIZE(iexheaac_sfb_96_96),
                                               iexheaac_sfb_96_768,
                                               iexheaac_sfb_96_96,
                                               {0},
                                               {0}},
                                              {96000,
                                               ARRAY_SIZE(iexheaac_sfb_96_768),
                                               ARRAY_SIZE(iexheaac_sfb_96_96),
                                               iexheaac_sfb_96_768,
                                               iexheaac_sfb_96_96,
                                               {0},
                                               {0}}};
