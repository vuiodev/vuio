/******************************************************************************
 *
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
#include "ixheaac_type_def.h"
#include "ixheaacd_mps_struct_def.h"
#include "ixheaacd_mps_res_rom.h"
#include "ixheaacd_mps_aac_struct.h"
#include "ixheaac_constants.h"
#include "ixheaacd_bitbuffer.h"
#include "ixheaacd_common_rom.h"
#include "ixheaacd_sbrdecsettings.h"
#include "ixheaacd_sbr_scale.h"
#include "ixheaacd_env_extr_part.h"
#include "ixheaacd_sbr_rom.h"
#include "ixheaacd_hybrid.h"
#include "ixheaacd_ps_dec.h"
#include "ixheaacd_mps_polyphase.h"
#include "ixheaacd_config.h"
#include "ixheaacd_qmf_dec.h"
#include "ixheaacd_mps_dec.h"
#include "ixheaacd_mps_macro_def.h"

VOID ixheaacd_dec_interp_umx(WORD32 m[MAX_PARAMETER_SETS][MAX_PARAMETER_BANDS], WORD32 *ptr_r,
                             WORD32 *ptr_m_prev, ia_heaac_mps_state_struct *pstr_mps_state) {
  WORD32 ts, ps = 0, pb;
  WORD32 *r_out;
  const WORD32 *reciprocal_tab = pstr_mps_state->ia_mps_dec_mps_table.m1_m2_table_ptr->reciprocal;

  WORD32 num_parameter_bands = pstr_mps_state->num_parameter_bands;
  WORD32 num_parameter_sets = pstr_mps_state->num_parameter_sets;

  WORD32 prev_slot = -1;
  WORD32 *prm_slot = pstr_mps_state->aux_struct->param_slot;
  WORD32 curr_slot = *prm_slot;

  WORD32 temp = reciprocal_tab[curr_slot];
  r_out = ptr_r;

  for (ts = prev_slot + 1; ts <= curr_slot; ts++) {
    for (pb = 0; pb < num_parameter_bands; pb++) {
      WORD32 alpha = temp * (ts + 1);
      WORD32 one_minus_alpha;
      WORD64 result;

      one_minus_alpha = (ONE_IN_Q28 - alpha);

      result = ((WORD64)(*ptr_m_prev) * (WORD64)one_minus_alpha);
      result += ((WORD64)(m[ps][pb]) * (WORD64)alpha);

      *r_out++ = (WORD32)((WORD64)result >> 28);
      ptr_m_prev++;
    }
    ptr_m_prev -= num_parameter_bands;
  }

  for (ps = 1; ps < num_parameter_sets; ps++) {
    WORD32 prev_slot = prm_slot[ps - 1];
    WORD32 curr_slot = prm_slot[ps];

    temp = reciprocal_tab[curr_slot - prev_slot - 1];

    for (ts = (prev_slot) + 1; ts <= (curr_slot); ts++) {
      for (pb = 0; pb < num_parameter_bands; pb++) {
        WORD32 alpha = (ts - prev_slot) * temp;
        WORD64 result;
        WORD32 one_minus_alpha;
        one_minus_alpha = (ONE_IN_Q28 - alpha);

        result = ((WORD64)(m[ps - 1][pb]) * (WORD64)one_minus_alpha);
        result += ((WORD64)(m[ps][pb]) * (WORD64)alpha);

        *r_out++ = (WORD32)((WORD64)result >> 28);
      }
    }
  }
  return;
}

VOID ixheaacd_apply_abs_kernels(WORD32 *ptr_r_in, WORD32 *ptr_r_out, SIZE_T *ptr_params) {
  WORD32 ts, qb;
  SIZE_T *idx_ptr = (SIZE_T *)ptr_params[0];
  WORD32 time_slots = (WORD32)ptr_params[1];
  WORD32 num_parameter_bands = (WORD32)ptr_params[2];
  WORD32 hybrid_bands = (WORD32)ptr_params[3];

  for (ts = 0; ts < time_slots; ts++) {
    SIZE_T *idx = idx_ptr;
    for (qb = 0; qb < hybrid_bands; qb++) {
      WORD32 idx_v = (WORD32)(*idx);
      *ptr_r_out++ = *(ptr_r_in + idx_v);
      idx++;
    }
    ptr_r_in += num_parameter_bands;
  }
  return;
}
