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

#define IXHEAACE_HYBRID_FILTER_LENGTH (13)
#define IXHEAACE_QMF_BUFFER_MOVE (IXHEAACE_HYBRID_FILTER_LENGTH - 1)
#define IXHEAACE_HYBRID_FILTER_DELAY (6)
#define IXHEAACE_NUM_QMF_BANDS_IN_HYBRID (3)
#define IXHEAACE_NUM_HYBRID_BANDS (16)

typedef enum {

  IXHEAACE_HYBRID_2_REAL = 2,
  IXHEAACE_HYBRID_4_CPLX = 4,
  IXHEAACE_HYBRID_8_CPLX = 8

} ixheaace_hybrid_res;

typedef struct {
  FLOAT32 *ptr_work_real;
  FLOAT32 *ptr_work_imag;
  FLOAT32 **ptr_qmf_buf_real;
  FLOAT32 **ptr_qmf_buf_imag;

} ixheaace_str_hybrid;

typedef ixheaace_str_hybrid *ixheaace_pstr_hybrid;

VOID ixheaace_hybrid_synthesis(const FLOAT32 **ptr_hybrid_real_flt,
                               const FLOAT32 **ptr_hybrid_imag_flt, FLOAT32 **ptr_qmf_real_flt,
                               FLOAT32 **ptr_qmf_imag_flt, const WORD32 *ptr_hyb_res);

IA_ERRORCODE ixheaace_hybrid_analysis(const FLOAT32 **ptr_qmf_real_in,
                                      const FLOAT32 **ptr_qmf_imag_in, FLOAT32 **ptr_hyb_real_in,
                                      FLOAT32 **ptr_hyb_imag_in, ixheaace_pstr_hybrid pstr_hybrid,
                                      ixheaace_str_ps_tab *pstr_ps_tab,
                                      ixheaace_common_tables *pstr_common_tab);

WORD32
ixheaace_create_hybrid_filter_bank(ixheaace_pstr_hybrid pstr_hybrid, FLOAT32 **pptr_flt);
