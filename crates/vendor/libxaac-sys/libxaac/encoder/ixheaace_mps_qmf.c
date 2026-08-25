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
#include <stdlib.h>
#include "ixheaac_type_def.h"
#include "ixheaac_error_standards.h"
#include "ixheaace_error_codes.h"
#include "ixheaace_mps_common_fix.h"
#include "ixheaace_mps_defines.h"
#include "ixheaace_mps_common_define.h"
#include "ixheaace_mps_buf.h"
#include "ixheaace_mps_lib.h"
#include "ixheaace_mps_main_structure.h"
#include "ixheaace_mps_tools_rom.h"
#include "ixheaace_mps_dct.h"
#include "ixheaace_mps_qmf.h"
#include "ixheaac_constants.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaac_basic_ops.h"

static IA_ERRORCODE ixheaace_mps_212_qmf_forward_modulation_hq(
    ixheaace_mps_pstr_qmf_filter_bank pstr_qmf_filter_bank, const FLOAT32 *time_in,
    FLOAT32 *real_subband, FLOAT32 *imag_subband, WORD8 *ptr_scratch) {
  IA_ERRORCODE error;
  WORD32 idx;
  WORD32 qmf_bands = pstr_qmf_filter_bank->no_channels;
  WORD32 qmf_bands_by_2 = qmf_bands << 1;
  FLOAT32 intermediate_band;
  FLOAT32 x0, x1, y0, y1;
  if (qmf_bands == 64) {
    x0 = time_in[1];
    y0 = time_in[0];
    real_subband[0] = (x0 + y0) / 2;
    imag_subband[0] = (x0 - y0) / 2;
    for (idx = 1; idx < qmf_bands; idx++) {
      x0 = time_in[idx + 1];
      y0 = time_in[qmf_bands_by_2 - idx];
      real_subband[idx] = (x0 - y0) / 2;
      imag_subband[idx] = (x0 + y0) / 2;
    }
  } else {
    for (idx = 0; idx < qmf_bands; idx += 2) {
      x0 = time_in[idx + 0];
      x1 = time_in[idx + 1];
      y0 = time_in[qmf_bands_by_2 - 1 - idx];
      y1 = time_in[qmf_bands_by_2 - 2 - idx];

      real_subband[idx + 0] = (x0 - y0) / 2;
      real_subband[idx + 1] = (x1 - y1) / 2;
      imag_subband[idx + 0] = (x0 + y0) / 2;
      imag_subband[idx + 1] = (x1 + y1) / 2;
    }
  }
  error = ixheaace_mps_212_dct_iv(real_subband, qmf_bands, ptr_scratch);
  if (error != IA_NO_ERROR) {
    return error;
  }
  error = ixheaace_mps_212_dst_iv(imag_subband, qmf_bands, ptr_scratch);
  if (error != IA_NO_ERROR) {
    return error;
  }

  for (idx = 0; idx < MIN(pstr_qmf_filter_bank->lsb, qmf_bands); idx += 2) {
    intermediate_band = real_subband[idx];
    real_subband[idx] = -imag_subband[idx];
    imag_subband[idx] = intermediate_band;

    intermediate_band = -real_subband[idx + 1];
    real_subband[idx + 1] = imag_subband[idx + 1];
    imag_subband[idx + 1] = intermediate_band;
  }
  return IA_NO_ERROR;
}

static VOID ixheaace_mps_212_qmf_ana_prototype_fir_slot(FLOAT32 *ptr_analysis_buffer,
                                                        WORD32 qmf_bands,
                                                        const FLOAT32 *ptr_filter, WORD32 stride,
                                                        FLOAT32 *ptr_filter_states) {
  const FLOAT32 *p_flt = ptr_filter;
  WORD32 poly, band;
  FLOAT32 accumlate;

  for (band = 0; band < 2 * qmf_bands; band++) {
    accumlate = 0;
    p_flt += QMF_NO_POLY * (stride - 1);
    for (poly = 0; poly < QMF_NO_POLY; poly++) {
      accumlate += (((*p_flt++) * ptr_filter_states[2 * qmf_bands * poly]));
    }
    ptr_analysis_buffer[2 * qmf_bands - 1 - band] = accumlate;
    ptr_filter_states++;
  }
}

IA_ERRORCODE ixheaace_mps_212_qmf_analysis_filtering_slot(
    ixheaace_mps_pstr_qmf_filter_bank pstr_qmf_filter_bank, FLOAT32 *real_subband,
    FLOAT32 *imag_subband, const FLOAT32 *time_in, const WORD32 stride, FLOAT32 *p_work_buffer,
    WORD8 *ptr_scratch) {
  IA_ERRORCODE error = IA_NO_ERROR;
  WORD32 idx;
  WORD32 offset = pstr_qmf_filter_bank->no_channels * (QMF_NO_POLY * 2 - 1);
  FLOAT32 *pstr_filter_states_ana_tmp =
      ((FLOAT32 *)pstr_qmf_filter_bank->ptr_filter_states) + offset;
  for (idx = 0; idx < pstr_qmf_filter_bank->no_channels; idx++) {
    *pstr_filter_states_ana_tmp++ = (FLOAT32)*time_in;
    time_in += stride;
  }
  ixheaace_mps_212_qmf_ana_prototype_fir_slot(
      p_work_buffer, pstr_qmf_filter_bank->no_channels, pstr_qmf_filter_bank->pstr_filter,
      pstr_qmf_filter_bank->p_stride, (FLOAT32 *)pstr_qmf_filter_bank->ptr_filter_states);

  error = ixheaace_mps_212_qmf_forward_modulation_hq(pstr_qmf_filter_bank, p_work_buffer,
                                                     real_subband, imag_subband, ptr_scratch);
  if (error != IA_NO_ERROR) {
    return error;
  }

  memmove(pstr_qmf_filter_bank->ptr_filter_states,
          (FLOAT32 *)pstr_qmf_filter_bank->ptr_filter_states + pstr_qmf_filter_bank->no_channels,
          offset * sizeof(FLOAT32));
  return error;
}

IA_ERRORCODE
ixheaace_mps_212_qmf_init_filter_bank(ixheaace_mps_pstr_qmf_filter_bank pstr_qmf_filter_bank,
                                      VOID *ptr_filter_states, WORD32 num_cols, WORD32 lsb,
                                      WORD32 usb, WORD32 no_channels) {
  memset(pstr_qmf_filter_bank, 0, sizeof(ixheaace_mps_qmf_filter_bank));
  pstr_qmf_filter_bank->no_channels = no_channels;
  pstr_qmf_filter_bank->no_col = num_cols;
  pstr_qmf_filter_bank->lsb = MIN(lsb, pstr_qmf_filter_bank->no_channels);
  pstr_qmf_filter_bank->usb = MIN(usb, pstr_qmf_filter_bank->no_channels);
  pstr_qmf_filter_bank->ptr_filter_states = (VOID *)ptr_filter_states;
  memset(pstr_qmf_filter_bank->ptr_filter_states, 0,
         (2 * QMF_NO_POLY - 1) * pstr_qmf_filter_bank->no_channels * sizeof(FLOAT32));
  pstr_qmf_filter_bank->p_stride = 1;
  if (no_channels == 64) {
    pstr_qmf_filter_bank->pstr_filter = qmf_mps_ld_fb_640;
    pstr_qmf_filter_bank->filter_size = 640;
  } else {
    pstr_qmf_filter_bank->pstr_filter = qmf_mps_ld_fb_320;
    pstr_qmf_filter_bank->filter_size = 320;
  }

  return IA_NO_ERROR;
}
