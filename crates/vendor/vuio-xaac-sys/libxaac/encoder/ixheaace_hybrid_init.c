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

#include <string.h>

#include "ixheaac_type_def.h"
#include "ixheaac_error_standards.h"
#include "ixheaace_common_rom.h"
#include "ixheaace_sbr_def.h"
#include "ixheaace_resampler.h"
#include "ixheaace_sbr_rom.h"
#include "ixheaace_sbr_hybrid.h"
#include "ixheaac_constants.h"
#include "ixheaace_aac_constants.h"

WORD32
ixheaace_create_hybrid_filter_bank(ixheaace_pstr_hybrid pstr_hybrid, FLOAT32 **pptr_flt) {
  LOOPINDEX i;
  FLOAT32 *ptr_flt = *pptr_flt;

  pstr_hybrid->ptr_work_real = ptr_flt;
  ptr_flt += (IXHEAACE_QMF_TIME_SLOTS + IXHEAACE_QMF_BUFFER_MOVE);
  pstr_hybrid->ptr_work_imag = ptr_flt;
  ptr_flt += (IXHEAACE_QMF_TIME_SLOTS + IXHEAACE_QMF_BUFFER_MOVE);

  pstr_hybrid->ptr_qmf_buf_real = (FLOAT32 **)ptr_flt;
  ptr_flt += IXHEAACE_NUM_QMF_BANDS_IN_HYBRID * sizeof(FLOAT32 *) / sizeof(FLOAT32);

  pstr_hybrid->ptr_qmf_buf_imag = (FLOAT32 **)ptr_flt;
  ptr_flt += IXHEAACE_NUM_QMF_BANDS_IN_HYBRID * sizeof(FLOAT32 *) / sizeof(FLOAT32);

  for (i = 0; i < IXHEAACE_NUM_QMF_BANDS_IN_HYBRID; i++) {
    pstr_hybrid->ptr_qmf_buf_real[i] = ptr_flt;
    ptr_flt += IXHEAACE_QMF_BUFFER_MOVE;

    memset(pstr_hybrid->ptr_qmf_buf_real[i], 0,
           IXHEAACE_QMF_BUFFER_MOVE * sizeof(pstr_hybrid->ptr_qmf_buf_real[0]));

    pstr_hybrid->ptr_qmf_buf_imag[i] = ptr_flt;
    ptr_flt += IXHEAACE_QMF_BUFFER_MOVE;

    memset(pstr_hybrid->ptr_qmf_buf_imag[i], 0,
           IXHEAACE_QMF_BUFFER_MOVE * sizeof(pstr_hybrid->ptr_qmf_buf_imag[0]));
  }

  *pptr_flt = ptr_flt;
  return IA_NO_ERROR;
}
