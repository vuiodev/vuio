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
#include "ixheaac_constants.h"
#include "ixheaace_sbr_def.h"
#include "ixheaace_resampler.h"
#include "ixheaace_sbr_rom.h"
#include "ixheaace_common_rom.h"
#include "ixheaace_sbr_hbe.h"
#include "ixheaace_sbr_qmf_enc.h"

#include "ixheaace_sbr_hybrid.h"
#include "ixheaace_aac_constants.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops16.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaac_basic_ops.h"
#include "ixheaac_error_standards.h"

VOID ixheaace_create_synthesis_qmf_bank(
    ixheaace_pstr_sbr_qmf_filter_bank pstr_sbr_qmf_filter_bank, WORD32 *ptr_common_buffer,
    ixheaace_str_sbr_tabs *pstr_sbr_tab) {
  WORD32 *ptr_temp;
  ptr_temp =
      &ptr_common_buffer[5 * NO_OF_ESTIMATES * MAXIMUM_FREQ_COEFFS + IXHEAACE_PS_BUF4_SIZE];
  memset(pstr_sbr_qmf_filter_bank, 0, sizeof(ixheaace_str_sbr_qmf_filter_bank));

  pstr_sbr_qmf_filter_bank->ptr_flt_filter = pstr_sbr_tab->ptr_qmf_tab->sbr_p_64_640_qmf;
  pstr_sbr_qmf_filter_bank->ptr_flt_alt_sin_twiddle =
      pstr_sbr_tab->ptr_qmf_tab->sbr_alt_sin_twiddle;
  pstr_sbr_qmf_filter_bank->ptr_flt_cos_twiddle = pstr_sbr_tab->ptr_qmf_tab->sbr_cos_sin_twiddle;

  pstr_sbr_qmf_filter_bank->ptr_flt_work_buf = (FLOAT32 *)ptr_temp;
  ptr_temp += 64;
  pstr_sbr_qmf_filter_bank->ptr_flt_time_buf = (FLOAT32 *)ptr_temp;
}

VOID ixheaace_create_qmf_bank(ixheaace_pstr_sbr_qmf_filter_bank pstr_sbr_qmf_filter_bank,
                              ixheaace_str_sbr_tabs *pstr_sbr_tab, WORD32 is_ld_sbr) {
  pstr_sbr_qmf_filter_bank->offset = 0;
  pstr_sbr_qmf_filter_bank->flag = 0;

  pstr_sbr_qmf_filter_bank->ptr_filter =
      (is_ld_sbr) ? &cld_fb_64_640[0] : pstr_sbr_tab->ptr_qmf_tab->sbr_qmf_64_640;

  pstr_sbr_qmf_filter_bank->ptr_qmf_states_buf =
      (FLOAT32 *)pstr_sbr_qmf_filter_bank->ptr_sbr_qmf_states_ana;

  if (is_ld_sbr) {
    pstr_sbr_qmf_filter_bank->ptr_qmf_states_curr_pos =
        (FLOAT32 *)pstr_sbr_qmf_filter_bank->ptr_qmf_states_buf;
    pstr_sbr_qmf_filter_bank->ptr_fp1 = pstr_sbr_qmf_filter_bank->ptr_qmf_states_buf;
    pstr_sbr_qmf_filter_bank->ptr_fp2 = pstr_sbr_qmf_filter_bank->ptr_qmf_states_buf + 64;

    pstr_sbr_qmf_filter_bank->start_coeff_cnt = 0;
  }

  pstr_sbr_qmf_filter_bank->ptr_ref_coeff_l = pstr_sbr_tab->ptr_qmf_tab->sbr_qmf_64_640 + 10;
  pstr_sbr_qmf_filter_bank->ptr_ref_coeff_r = pstr_sbr_tab->ptr_qmf_tab->sbr_qmf_64_640 + 640;
  pstr_sbr_qmf_filter_bank->ptr_cld_filt = pstr_sbr_tab->ptr_qmf_tab->sbr_cld_fb;

  pstr_sbr_qmf_filter_bank->offset_l = 5;
  pstr_sbr_qmf_filter_bank->offset_r = 5;

  memset(pstr_sbr_qmf_filter_bank->ptr_qmf_states_buf, 0,
         QMF_FILTER_LENGTH * sizeof(pstr_sbr_qmf_filter_bank->ptr_qmf_states_buf[0]));
}
