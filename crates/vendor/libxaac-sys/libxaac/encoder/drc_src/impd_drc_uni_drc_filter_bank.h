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
#define IMPD_DRCMAX_PHASE_ALIGN_CH_GROUP (32)

typedef struct {
  WORD32 num_bands;
  WORD32 complexity;
} ia_drc_filter_bank_struct;

typedef struct {
  WORD32 num_filter_banks;
  WORD32 num_phase_alignment_ch_groups;
  WORD32 complexity;
  ia_drc_filter_bank_struct str_drc_filter_bank[IMPD_DRCMAX_PHASE_ALIGN_CH_GROUP];
} ia_drc_filter_banks_struct;

IA_ERRORCODE impd_drc_init_all_filter_banks(
    const ia_drc_coefficients_uni_drc_struct *pstr_drc_coefficients_uni_drc,
    const ia_drc_instructions_uni_drc *pstr_drc_instructions_uni_drc,
    ia_drc_filter_banks_struct *pstr_filter_banks, VOID *ptr_scratch);
