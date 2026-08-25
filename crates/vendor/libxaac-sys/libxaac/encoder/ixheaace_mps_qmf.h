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
struct ixheaace_mps_qmf_filter_bank {
  const FLOAT32 *pstr_filter;
  VOID *ptr_filter_states;
  WORD32 filter_size;
  WORD32 no_channels;
  WORD32 no_col;
  WORD32 lsb;
  WORD32 usb;
  UWORD8 p_stride;
};

typedef struct ixheaace_mps_qmf_filter_bank ixheaace_mps_qmf_filter_bank,
    *ixheaace_mps_pstr_qmf_filter_bank;

IA_ERRORCODE
ixheaace_mps_212_qmf_init_filter_bank(ixheaace_mps_pstr_qmf_filter_bank pstr_qmf_filter_bank,
                                      VOID *p_filter_states, WORD32 num_cols, WORD32 lsb,
                                      WORD32 usb, WORD32 no_channels);

IA_ERRORCODE ixheaace_mps_212_qmf_analysis_filtering_slot(
    ixheaace_mps_pstr_qmf_filter_bank pstr_qmf_filter_bank, FLOAT32 *qmf_real, FLOAT32 *qmf_imag,
    const FLOAT32 *time_in, const WORD32 stride, FLOAT32 *p_work_buffer, WORD8 *ptr_scratch);
