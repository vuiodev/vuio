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
#include "ixheaacd_sbr_common.h"
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

VOID ixheaacd_calc_m1m2_emm(ia_heaac_mps_state_struct *pstr_mps_state) {
  ia_mps_dec_auxilary_struct *p_aux_struct = pstr_mps_state->aux_struct;
  ia_mps_dec_m2_param_struct *m2_param = p_aux_struct->m2_param;
  ia_mps_dec_m1_param_struct *m1_param = pstr_mps_state->array_struct->m1_param;
  WORD32 ps;
  WORD32 pb;
  WORD32 col;
  WORD32 row;

  WORD64 acc;
  WORD32 *h11, *h12, *h21, *h22, *dummy1, *dummy2;
  WORD16 *dummy3, *dummy4;

  h11 = pstr_mps_state->mps_scratch_mem_v;
  h12 = h11 + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h12), BYTE_ALIGN_8);
  h21 = h12 + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h21), BYTE_ALIGN_8);
  h22 = h21 + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h22), BYTE_ALIGN_8);
  dummy1 =
      h22 + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*dummy1), BYTE_ALIGN_8);
  dummy2 =
      dummy1 + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*dummy2), BYTE_ALIGN_8);

  dummy3 = (WORD16 *)pstr_mps_state->mps_scratch_mem_v +
           IXHEAAC_GET_SIZE_ALIGNED_TYPE(PARAMETER_BANDSX12, sizeof(*dummy3), BYTE_ALIGN_8);
  dummy4 =
      dummy3 + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*dummy4), BYTE_ALIGN_8);

  for (ps = 0; ps < pstr_mps_state->num_parameter_sets; ps++) {
    ixheaacd_param_2_umx_ps(pstr_mps_state, h11, h12, h21, h22, dummy1, dummy2, dummy3, dummy4, 0,
                            ps, 0);

    for (pb = 0; pb < pstr_mps_state->num_parameter_bands; pb++) {
      WORD32 m11 = p_aux_struct->ttt_cpc_1[0][ps][pb] + ONE_IN_Q16;
      WORD32 m12 = p_aux_struct->ttt_cpc_2[0][ps][pb] - ONE_IN_Q15;
      WORD32 m21 = p_aux_struct->ttt_cpc_1[0][ps][pb] - ONE_IN_Q15;
      WORD32 m22 = p_aux_struct->ttt_cpc_2[0][ps][pb] + ONE_IN_Q16;
      WORD32 m31 = ONE_IN_Q15 - p_aux_struct->ttt_cpc_1[0][ps][pb];
      WORD32 m32 = ONE_IN_Q15 - p_aux_struct->ttt_cpc_2[0][ps][pb];
      WORD32 weight1;
      WORD32 weight2;
      WORD32 h_real[2][2];
      WORD32 h_imag[2][2];

      ia_mps_dec_spatial_bs_frame_struct *p_cur_bs = pstr_mps_state->bs_frame;

      ixheaacd_get_matrix_inversion_weights(
          p_cur_bs->ott_cld_idx[0][ps][pb], p_cur_bs->ott_cld_idx[0][ps][pb], 1,
          p_aux_struct->ttt_cpc_1[0][ps][pb], p_aux_struct->ttt_cpc_2[0][ps][pb], &weight1,
          &weight2, &(pstr_mps_state->ia_mps_dec_mps_table));

      ixheaacd_invert_matrix(weight1, weight2, h_real, h_imag,
                             pstr_mps_state->ia_mps_dec_mps_table.common_table_ptr);

      pstr_mps_state->m1_param_imag_present = 1;

      acc = (WORD64)((WORD64)h_real[0][0] * (WORD64)m11 + (WORD64)h_real[1][0] * (WORD64)m12);
      acc >>= 15;
      m1_param->m1_param_real[0][0][ps][pb] = (WORD32)acc;

      acc = (WORD64)((WORD64)h_real[0][1] * (WORD64)m11 + (WORD64)h_real[1][1] * (WORD64)m12);
      acc >>= 15;
      m1_param->m1_param_real[0][1][ps][pb] = (WORD32)acc;

      acc = (WORD64)((WORD64)h_imag[0][0] * (WORD64)m11 + (WORD64)h_imag[1][0] * (WORD64)m12);
      acc >>= 15;
      m1_param->m1_param_imag[0][0][ps][pb] = (WORD32)acc;

      acc = (WORD64)((WORD64)h_imag[0][1] * (WORD64)m11 + (WORD64)h_imag[1][1] * (WORD64)m12);
      acc >>= 15;
      m1_param->m1_param_imag[0][1][ps][pb] = (WORD32)acc;

      acc = (WORD64)((WORD64)h_real[0][0] * (WORD64)m21 + (WORD64)h_real[1][0] * (WORD64)m22);
      acc >>= 15;
      m1_param->m1_param_real[1][0][ps][pb] = (WORD32)acc;

      acc = (WORD64)((WORD64)h_real[0][1] * (WORD64)m21 + (WORD64)h_real[1][1] * (WORD64)m22);
      acc >>= 15;
      m1_param->m1_param_real[1][1][ps][pb] = (WORD32)acc;

      acc = (WORD64)((WORD64)h_imag[0][0] * (WORD64)m21 + (WORD64)h_imag[1][0] * (WORD64)m22);
      acc >>= 15;
      m1_param->m1_param_imag[1][0][ps][pb] = (WORD32)acc;

      acc = (WORD64)((WORD64)h_imag[0][1] * (WORD64)m21 + (WORD64)h_imag[1][1] * (WORD64)m22);
      acc >>= 15;
      m1_param->m1_param_imag[1][1][ps][pb] = (WORD32)acc;

      acc = (WORD64)((WORD64)h_real[0][0] * (WORD64)m31 + (WORD64)h_real[1][0] * (WORD64)m32);
      acc >>= 15;
      m1_param->m1_param_real[2][0][ps][pb] = (WORD32)acc;

      acc = (WORD64)((WORD64)h_real[0][1] * (WORD64)m31 + (WORD64)h_real[1][1] * (WORD64)m32);
      acc >>= 15;
      m1_param->m1_param_real[2][1][ps][pb] = (WORD32)acc;

      acc = (WORD64)((WORD64)h_imag[0][0] * (WORD64)m31 + (WORD64)h_imag[1][0] * (WORD64)m32);
      acc >>= 15;
      m1_param->m1_param_imag[2][0][ps][pb] = (WORD32)acc;

      acc = (WORD64)((WORD64)h_imag[0][1] * (WORD64)m31 + (WORD64)h_imag[1][1] * (WORD64)m32);
      acc >>= 15;
      m1_param->m1_param_imag[2][1][ps][pb] = (WORD32)acc;

      m1_param->m1_param_real[0][2][ps][pb] = ONE_IN_Q15;
      m1_param->m1_param_real[1][2][ps][pb] = ONE_IN_Q15;
      m1_param->m1_param_real[2][2][ps][pb] = -32768;

      m1_param->m1_param_imag[0][2][ps][pb] = 0;
      m1_param->m1_param_imag[1][2][ps][pb] = 0;
      m1_param->m1_param_imag[2][2][ps][pb] = 0;

      for (row = 0; row < 3; row++) {
        for (col = 0; col < 3; col++) {
          m1_param->m1_param_real[row][col][ps][pb] = ixheaacd_mps_mult32_shr_15(
              m1_param->m1_param_real[row][col][ps][pb], ONE_BY_THREE_Q15);
          m1_param->m1_param_imag[row][col][ps][pb] = ixheaacd_mps_mult32_shr_15(
              m1_param->m1_param_imag[row][col][ps][pb], ONE_BY_THREE_Q15);
        }

        m1_param->m1_param_real[2][col][ps][pb] =
            ixheaacd_mps_mult32_shr_15(m1_param->m1_param_real[2][col][ps][pb], SQRT_TWO_Q15);
        m1_param->m1_param_imag[2][col][ps][pb] =
            ixheaacd_mps_mult32_shr_15(m1_param->m1_param_imag[2][col][ps][pb], SQRT_TWO_Q15);

        m1_param->m1_param_real[3][col][ps][pb] = m1_param->m1_param_real[0][col][ps][pb];
        m1_param->m1_param_imag[3][col][ps][pb] = m1_param->m1_param_imag[0][col][ps][pb];
        m1_param->m1_param_real[4][col][ps][pb] = m1_param->m1_param_real[1][col][ps][pb];
        m1_param->m1_param_imag[4][col][ps][pb] = m1_param->m1_param_imag[1][col][ps][pb];
        m1_param->m1_param_real[5][col][ps][pb] = 0;
        m1_param->m1_param_imag[5][col][ps][pb] = 0;
      }
    }

    for (pb = 0; pb < pstr_mps_state->num_parameter_bands; pb++) {
      m2_param->m2_decor_real[0][ps][pb] = h12[pb];
      m2_param->m2_decor_real[1][ps][pb] = h22[pb];
      m2_param->m2_decor_real[2][ps][pb] = h12[pb];
      m2_param->m2_decor_real[3][ps][pb] = h22[pb];
    }

    for (pb = 0; pb < pstr_mps_state->num_parameter_bands; pb++) {
      m2_param->m2_resid_real[0][ps][pb] = h11[pb];
      m2_param->m2_resid_real[1][ps][pb] = h21[pb];
      m2_param->m2_resid_real[2][ps][pb] = h21[pb];
      m2_param->m2_resid_real[3][ps][pb] = 32768;
    }
  }
  return;
}
