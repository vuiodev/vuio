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
#include "ixheaace_aac_constants.h"
#include "ixheaace_sbr_def.h"
#include "ixheaace_resampler.h"
#include "ixheaace_sbr_rom.h"
#include "ixheaace_error_codes.h"
#include "ixheaac_error_standards.h"

IA_ERRORCODE ia_enhaacplus_enc_init_iir_resampler(ixheaace_iir21_resampler *pstr_resampler,
                                                  WORD32 ratio,
                                                  ixheaace_resampler_table *pstr_resampler_table)

{
  struct ixheaace_iir_params const *current_set = NULL;
  IA_ERRORCODE error = IA_NO_ERROR;

  if (pstr_resampler == NULL) {
    return IA_EXHEAACE_INIT_FATAL_RESAMPLER_INIT_FAILED;
  }

  memset(pstr_resampler->iir_filter.ring_buf_1, 0,
         (LEN_RING_BUF * sizeof(pstr_resampler->iir_filter.ring_buf_1[0])));
  memset(pstr_resampler->iir_filter.ring_buf_2, 0,
         (LEN_RING_BUF * sizeof(pstr_resampler->iir_filter.ring_buf_2[0])));

  current_set = &(pstr_resampler_table->iir_param_set);

  pstr_resampler->iir_filter.ptr_coeff_iir_den = current_set->coeff_iir_den;
  pstr_resampler->iir_filter.ptr_coeff_iir_num = current_set->coeff_iir_num;
  pstr_resampler->iir_filter.max = current_set->max;
  pstr_resampler->delay = current_set->delay;
  pstr_resampler->ratio = ratio;
  pstr_resampler->pending = ratio - 1;

  return error;
}
IA_ERRORCODE
ia_enhaacplus_enc_init_iir_sos_resampler(ixheaace_iir_sos_resampler *pstr_resampler, WORD32 ratio,
                                         ixheaace_resampler_sos_table *pstr_resampler_table)

{
  struct ixheaace_iir_params_sos const *current_set = NULL;
  IA_ERRORCODE error = IA_NO_ERROR;

  if (pstr_resampler == NULL) {
    error = IA_EXHEAACE_INIT_FATAL_USAC_RESAMPLER_INIT_FAILED;
    return error;
  }

  memset(pstr_resampler->iir_filter.ring_buf_sos_1, 0,
         (LEN_RING_BUF_SOS_1 * sizeof(pstr_resampler->iir_filter.ring_buf_sos_1[0])));
  memset(pstr_resampler->iir_filter.ring_buf_sos_2, 0,
         (LEN_RING_BUF_SOS_2 * sizeof(pstr_resampler->iir_filter.ring_buf_sos_2[0])));

  current_set = &(pstr_resampler_table->iir_param_set_sos);

  pstr_resampler->iir_filter.ptr_coeff_iir_den = &current_set->coeff_iir_sos_den[0][0];
  pstr_resampler->iir_filter.ptr_coeff_iir_num = &current_set->coeff_iir_sos_num[0][0];
  pstr_resampler->ratio = ratio;
  pstr_resampler->pending = ratio - 1;
  pstr_resampler->iir_filter.gain_sos = current_set->gain_sos;
  pstr_resampler->delay = current_set->delay;

  return error;
}