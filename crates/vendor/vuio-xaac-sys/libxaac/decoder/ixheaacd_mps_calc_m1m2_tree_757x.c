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
#include <string.h>
#include "ixheaac_type_def.h"
#include "ixheaacd_mps_struct_def.h"
#include "ixheaacd_mps_res_rom.h"
#include "ixheaacd_mps_aac_struct.h"
#include "ixheaac_constants.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaacd_bitbuffer.h"
#include "ixheaacd_common_rom.h"
#include "ixheaacd_sbrdecsettings.h"
#include "ixheaacd_sbr_scale.h"
#include "ixheaacd_env_extr_part.h"
#include "ixheaacd_sbr_rom.h"
#include "ixheaacd_hybrid.h"
#include "ixheaacd_ps_dec.h"
#include "ixheaac_error_standards.h"
#include "ixheaacd_mps_polyphase.h"
#include "ixheaacd_config.h"
#include "ixheaacd_qmf_dec.h"
#include "ixheaacd_mps_dec.h"
#include "ixheaacd_mps_macro_def.h"
#include "ixheaacd_mps_basic_op.h"
#include "ixheaacd_mps_calc_m1m2_common.h"

VOID ixheaacd_calc_m1m2_7571(ia_heaac_mps_state_struct *pstr_mps_state) {
  ia_mps_dec_auxilary_struct *p_aux_struct = pstr_mps_state->aux_struct;
  ia_mps_dec_m2_param_struct *m2_param = p_aux_struct->m2_param;
  ia_mps_dec_m1_param_struct *m1_param = pstr_mps_state->array_struct->m1_param;
  WORD32 ps, pb;

  WORD32 *h11_l, *h11_r, *h12_l, *h12_r, *h21_l, *h21_r, *h22_l, *h22_r, *h12_res_l, *h12_res_r,
      *h22_res_l, *h22_res_r;
  WORD16 *c_f_l, *c_f_r, *dummy;

  WORD32 idx;
  WORD32 residual_coding = pstr_mps_state->residual_coding;
  WORD32 num_parameter_sets = pstr_mps_state->num_parameter_sets;
  WORD32 num_parameter_bands = pstr_mps_state->num_parameter_bands;

  h11_l = pstr_mps_state->mps_scratch_mem_v;
  h11_r =
      h11_l + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h11_r), BYTE_ALIGN_8);
  h12_l =
      h11_r + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h12_l), BYTE_ALIGN_8);
  h12_r =
      h12_l + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h12_r), BYTE_ALIGN_8);
  h21_l =
      h12_r + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h21_l), BYTE_ALIGN_8);
  h21_r =
      h21_l + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h21_r), BYTE_ALIGN_8);
  h22_l =
      h21_r + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h22_l), BYTE_ALIGN_8);
  h22_r =
      h22_l + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h22_r), BYTE_ALIGN_8);
  h12_res_l = h22_r + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h12_res_l),
                                                    BYTE_ALIGN_8);
  h12_res_r = h12_res_l + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h12_res_r),
                                                        BYTE_ALIGN_8);
  h22_res_l = h12_res_r + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h22_res_l),
                                                        BYTE_ALIGN_8);
  h22_res_r = h22_res_l + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h22_res_r),
                                                        BYTE_ALIGN_8);

  c_f_l = (WORD16 *)pstr_mps_state->mps_scratch_mem_v +
          IXHEAAC_GET_SIZE_ALIGNED_TYPE(PARAMETER_BANDSX24, sizeof(*c_f_l), BYTE_ALIGN_8);
  c_f_r =
      c_f_l + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*c_f_r), BYTE_ALIGN_8);
  dummy =
      c_f_r + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*dummy), BYTE_ALIGN_8);

  for (ps = 0; ps < num_parameter_sets; ps++) {
    ixheaacd_param_2_umx_ps(pstr_mps_state, h11_l, h12_l, h21_l, h22_l, h12_res_l, h22_res_l,
                            c_f_l, dummy, 0, ps, pstr_mps_state->res_bands[0]);
    ixheaacd_param_2_umx_ps(pstr_mps_state, h11_r, h12_r, h21_r, h22_r, h12_res_r, h22_res_r,
                            c_f_r, dummy, 1, ps, pstr_mps_state->res_bands[1]);

    for (pb = 0; pb < num_parameter_bands; pb++) {
      m1_param->m1_param_real[0][0][ps][pb] = ONE_IN_Q15;
      m1_param->m1_param_real[1][1][ps][pb] = ONE_IN_Q15;
      m1_param->m1_param_real[2][2][ps][pb] = ONE_IN_Q15;
      m1_param->m1_param_real[3][3][ps][pb] = ONE_IN_Q15;
      m1_param->m1_param_real[4][4][ps][pb] = ONE_IN_Q15;
      m1_param->m1_param_real[5][5][ps][pb] = ONE_IN_Q15;
      m1_param->m1_param_real[6][0][ps][pb] = ONE_IN_Q15;
      m1_param->m1_param_real[7][1][ps][pb] = ONE_IN_Q15;
    }

    memcpy(m2_param->m2_decor_real[0][ps], h12_l, num_parameter_bands * sizeof(h12_l[0]));
    memcpy(m2_param->m2_decor_real[1][ps], h22_l, num_parameter_bands * sizeof(h22_l[0]));
    memcpy(m2_param->m2_decor_real[2][ps], h12_r, num_parameter_bands * sizeof(h12_r[0]));
    memcpy(m2_param->m2_decor_real[3][ps], h22_r, num_parameter_bands * sizeof(h22_r[0]));

    for (pb = 0; pb < num_parameter_bands; pb++) {
      idx = 0;

      m2_param->m2_resid_real[idx++][ps][pb] = h11_l[pb];

      if (residual_coding) m2_param->m2_resid_real[idx++][ps][pb] = h12_res_l[pb];

      m2_param->m2_resid_real[idx++][ps][pb] = h21_l[pb];

      if (residual_coding) m2_param->m2_resid_real[idx++][ps][pb] = h22_res_l[pb];

      m2_param->m2_resid_real[idx++][ps][pb] = ONE_IN_Q15;

      m2_param->m2_resid_real[idx++][ps][pb] = h11_r[pb];

      if (residual_coding) m2_param->m2_resid_real[idx++][ps][pb] = h12_res_r[pb];

      m2_param->m2_resid_real[idx++][ps][pb] = h21_r[pb];

      if (residual_coding) m2_param->m2_resid_real[idx++][ps][pb] = h22_res_r[pb];

      m2_param->m2_resid_real[idx++][ps][pb] = ONE_IN_Q15;

      m2_param->m2_resid_real[idx++][ps][pb] = ONE_IN_Q15;

      m2_param->m2_resid_real[idx++][ps][pb] = ONE_IN_Q15;
    }
  }
  return;
}

VOID ixheaacd_calc_m1m2_7572(ia_heaac_mps_state_struct *pstr_mps_state) {
  ia_mps_dec_auxilary_struct *p_aux_struct = pstr_mps_state->aux_struct;
  ia_mps_dec_m2_param_struct *m2_param = p_aux_struct->m2_param;
  ia_mps_dec_m1_param_struct *m1_param = pstr_mps_state->array_struct->m1_param;
  WORD32 ps, pb;
  WORD32 *h11_l, *h11_r, *h12_l, *h12_r, *h21_l, *h21_r, *h22_l, *h22_r, *h12_res_l, *h12_res_r,
      *h22_res_l, *h22_res_r;
  WORD16 *c_f_l, *c_f_r, *dummy;

  WORD32 idx;
  WORD32 residual_coding = pstr_mps_state->residual_coding;
  WORD32 num_parameter_sets = pstr_mps_state->num_parameter_sets;
  WORD32 num_parameter_bands = pstr_mps_state->num_parameter_bands;

  h11_l = pstr_mps_state->mps_scratch_mem_v;
  h11_r =
      h11_l + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h11_r), BYTE_ALIGN_8);
  h12_l =
      h11_r + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h12_l), BYTE_ALIGN_8);
  h12_r =
      h12_l + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h12_r), BYTE_ALIGN_8);
  h21_l =
      h12_r + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h21_l), BYTE_ALIGN_8);
  h21_r =
      h21_l + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h21_r), BYTE_ALIGN_8);
  h22_l =
      h21_r + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h22_l), BYTE_ALIGN_8);
  h22_r =
      h22_l + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h22_r), BYTE_ALIGN_8);
  h12_res_l = h22_r + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h12_res_l),
                                                    BYTE_ALIGN_8);
  h12_res_r = h12_res_l + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h12_res_r),
                                                        BYTE_ALIGN_8);
  h22_res_l = h12_res_r + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h22_res_l),
                                                        BYTE_ALIGN_8);
  h22_res_r = h22_res_l + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h22_res_r),
                                                        BYTE_ALIGN_8);

  c_f_l = (WORD16 *)pstr_mps_state->mps_scratch_mem_v +
          IXHEAAC_GET_SIZE_ALIGNED_TYPE(PARAMETER_BANDSX24, sizeof(*c_f_l), BYTE_ALIGN_8);
  c_f_r =
      c_f_l + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*c_f_r), BYTE_ALIGN_8);
  dummy =
      c_f_r + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*dummy), BYTE_ALIGN_8);

  for (ps = 0; ps < num_parameter_sets; ps++) {
    ixheaacd_param_2_umx_ps(pstr_mps_state, h11_l, h12_l, h21_l, h22_l, h12_res_l, h22_res_l,
                            c_f_l, dummy, 0, ps, pstr_mps_state->res_bands[0]);
    ixheaacd_param_2_umx_ps(pstr_mps_state, h11_r, h12_r, h21_r, h22_r, h12_res_r, h22_res_r,
                            c_f_r, dummy, 1, ps, pstr_mps_state->res_bands[1]);

    for (pb = 0; pb < num_parameter_bands; pb++) {
      m1_param->m1_param_real[0][0][ps][pb] = ONE_IN_Q15;
      m1_param->m1_param_real[1][1][ps][pb] = ONE_IN_Q15;
      m1_param->m1_param_real[2][2][ps][pb] = ONE_IN_Q15;
      m1_param->m1_param_real[3][3][ps][pb] = ONE_IN_Q15;
      m1_param->m1_param_real[4][4][ps][pb] = ONE_IN_Q15;
      m1_param->m1_param_real[5][5][ps][pb] = ONE_IN_Q15;
      m1_param->m1_param_real[6][4][ps][pb] = ONE_IN_Q15;
      m1_param->m1_param_real[7][5][ps][pb] = ONE_IN_Q15;
    }

    memcpy(m2_param->m2_decor_real[0][ps], h22_l, num_parameter_bands * sizeof(h22_l[0]));
    memcpy(m2_param->m2_decor_real[1][ps], h12_l, num_parameter_bands * sizeof(h12_l[0]));
    memcpy(m2_param->m2_decor_real[2][ps], h22_r, num_parameter_bands * sizeof(h22_r[0]));
    memcpy(m2_param->m2_decor_real[3][ps], h12_r, num_parameter_bands * sizeof(h12_r[0]));

    for (pb = 0; pb < num_parameter_bands; pb++) {
      idx = 0;

      m2_param->m2_resid_real[idx++][ps][pb] = ONE_IN_Q15;

      m2_param->m2_resid_real[idx++][ps][pb] = h21_l[pb];

      if (residual_coding) m2_param->m2_resid_real[idx++][ps][pb] = h22_res_l[pb];

      m2_param->m2_resid_real[idx++][ps][pb] = h11_l[pb];

      if (residual_coding) m2_param->m2_resid_real[idx++][ps][pb] = h12_res_l[pb];

      m2_param->m2_resid_real[idx++][ps][pb] = ONE_IN_Q15;

      m2_param->m2_resid_real[idx++][ps][pb] = h21_r[pb];

      if (residual_coding) m2_param->m2_resid_real[idx++][ps][pb] = h22_res_r[pb];

      m2_param->m2_resid_real[idx++][ps][pb] = h11_r[pb];

      if (residual_coding) m2_param->m2_resid_real[idx++][ps][pb] = h12_res_r[pb];

      m2_param->m2_resid_real[idx++][ps][pb] = ONE_IN_Q15;

      m2_param->m2_resid_real[idx++][ps][pb] = ONE_IN_Q15;
    }
  }
  return;
}
