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
#include "ixheaacd_mps_bitdec.h"

#undef ABS_THR
#define ABS_THR 1

VOID ixheaacd_calc_m1m2_51s1(ia_heaac_mps_state_struct *pstr_mps_state) {
  ia_mps_dec_auxilary_struct *p_aux_struct = pstr_mps_state->aux_struct;
  ia_mps_dec_m2_param_struct *m2_param = p_aux_struct->m2_param;
  ia_mps_dec_m1_param_struct *m1_param = pstr_mps_state->array_struct->m1_param;
  WORD32 ps, pb;

  WORD32 *iid;
  WORD32 *icc;
  WORD32 *h11;
  WORD32 *h12;
  WORD32 *h21;
  WORD32 *h22;
  WORD32 *h12_res;
  WORD32 *h22_res;
  WORD16 *c_l;
  WORD16 *c_r;

  const WORD32 *sqrt_tab = pstr_mps_state->ia_mps_dec_mps_table.common_table_ptr->sqrt_tab;
  WORD32 num_parameter_sets = pstr_mps_state->num_parameter_sets;
  WORD32 num_parameter_bands = pstr_mps_state->num_parameter_bands;
  iid = pstr_mps_state->mps_scratch_mem_v;
  icc = iid + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*icc), BYTE_ALIGN_8);
  h11 = icc + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h11), BYTE_ALIGN_8);
  h12 = h11 + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h12), BYTE_ALIGN_8);
  h21 = h12 + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h21), BYTE_ALIGN_8);
  h22 = h21 + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h22), BYTE_ALIGN_8);
  h12_res =
      h22 + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h12_res), BYTE_ALIGN_8);
  h22_res = h12_res +
            IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h22_res), BYTE_ALIGN_8);
  c_l = (WORD16 *)pstr_mps_state->mps_scratch_mem_v +
        IXHEAAC_GET_SIZE_ALIGNED_TYPE(PARAMETER_BANDSX16, sizeof(*c_l), BYTE_ALIGN_8);
  c_r = c_l + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*c_r), BYTE_ALIGN_8);

  for (ps = 0; ps < num_parameter_sets; ps++) {
    for (pb = 0; pb < num_parameter_bands; pb++) {
      WORD32 p_l_fs, p_r_fs;
      WORD32 p_l_c, p_r_c;
      WORD32 p_l_s, p_r_s;
      WORD32 p_l_f, p_r_f;
      WORD32 left_f;
      WORD32 right_f;
      WORD32 center;
      WORD32 left_s;
      WORD32 right_s;
      WORD32 left;
      WORD32 right;
      WORD32 cross;
      WORD32 temp_1, temp_2;
      WORD16 qtemp1, qtemp2;
      ia_mps_dec_spatial_bs_frame_struct *p_cur_bs = pstr_mps_state->bs_frame;

      p_l_fs = pstr_mps_state->ia_mps_dec_mps_table.m1_m2_table_ptr
                   ->cld_tab_1[p_cur_bs->ott_cld_idx[0][ps][pb] + 15];
      p_r_fs = pstr_mps_state->ia_mps_dec_mps_table.m1_m2_table_ptr
                   ->cld_tab_1[15 - p_cur_bs->ott_cld_idx[0][ps][pb]];

      p_l_c = pstr_mps_state->ia_mps_dec_mps_table.m1_m2_table_ptr
                  ->cld_tab_1[p_cur_bs->ott_cld_idx[1][ps][pb] + 15];
      p_r_c = pstr_mps_state->ia_mps_dec_mps_table.m1_m2_table_ptr
                  ->cld_tab_1[15 - p_cur_bs->ott_cld_idx[1][ps][pb]];

      p_l_s = pstr_mps_state->ia_mps_dec_mps_table.m1_m2_table_ptr
                  ->cld_tab_1[p_cur_bs->ott_cld_idx[2][ps][pb] + 15];
      p_r_s = pstr_mps_state->ia_mps_dec_mps_table.m1_m2_table_ptr
                  ->cld_tab_1[15 - p_cur_bs->ott_cld_idx[2][ps][pb]];

      p_l_f = pstr_mps_state->ia_mps_dec_mps_table.m1_m2_table_ptr
                  ->cld_tab_1[p_cur_bs->ott_cld_idx[3][ps][pb] + 15];
      p_r_f = pstr_mps_state->ia_mps_dec_mps_table.m1_m2_table_ptr
                  ->cld_tab_1[15 - p_cur_bs->ott_cld_idx[3][ps][pb]];

      left_f = ixheaacd_mps_mult32_shr_15(ixheaacd_mps_mult32_shr_15(p_l_fs, p_l_c), p_l_f);
      right_f = ixheaacd_mps_mult32_shr_15(ixheaacd_mps_mult32_shr_15(p_l_fs, p_l_c), p_r_f);
      center = ixheaacd_mps_mult32_shr_16(p_l_fs, p_r_c);
      left_s = ixheaacd_mps_mult32_shr_15(p_r_fs, p_l_s);
      right_s = ixheaacd_mps_mult32_shr_15(p_r_fs, p_r_s);

      left = center + left_f + left_s;
      right = center + right_f + right_s;

      temp_1 = ixheaacd_mps_mult32_shr_15(left_f, right_f);
      temp_1 = ixheaacd_mps_mult32_shr_15(temp_1, p_aux_struct->ott_icc[3][ps][pb]);

      temp_2 = ixheaacd_mps_mult32_shr_15(left_f, right_f);
      temp_2 = ixheaacd_mps_mult32_shr_15(temp_2, p_aux_struct->ott_icc[3][ps][pb]);

      temp_1 = ixheaac_add32_sat(temp_1, temp_2);
      cross = ixheaac_add32_sat(center, temp_1);

      temp_1 = ixheaacd_mps_div32_in_q15(left, right);
      qtemp1 = 15;

      iid[pb] = (10 * ixheaacd_mps_log10(temp_1, qtemp1)) >> 1;

      temp_1 = ixheaacd_mps_mult32_shr_15(left, right);
      qtemp1 = 15;
      temp_1 = ixheaacd_mps_add32(temp_1, ABS_THR, &qtemp1, 30);
      temp_1 = ixheaacd_mps_sqrt(temp_1, &qtemp1, sqrt_tab);
      temp_2 = ixheaacd_mps_div_32(cross, temp_1, &qtemp2);
      qtemp2 = qtemp2 + 15 - qtemp1;
      icc[pb] = ixheaacd_mps_convert_to_qn(temp_2, qtemp2, 15);

      if (icc[pb] > ONE_IN_Q15) {
        icc[pb] = ONE_IN_Q15;
      } else {
        if (icc[pb] < MINUS_POINT_NINE_NINE_Q15) {
          icc[pb] = MINUS_POINT_NINE_NINE_Q15;
        }
      }

      iid[pb] =
          ixheaacd_quantize_cld(iid[pb], pstr_mps_state->ia_mps_dec_mps_table.bitdec_table_ptr);
      icc[pb] =
          ixheaacd_quantize_icc(icc[pb], pstr_mps_state->ia_mps_dec_mps_table.bitdec_table_ptr);
    }

    ixheaacd_param_2_umx_ps_core_tables(iid, icc, num_parameter_bands, 0, h11, h12, h21, h22,
                                        h12_res, h22_res, c_l, c_r,
                                        pstr_mps_state->ia_mps_dec_mps_table.m1_m2_table_ptr);

    for (pb = 0; pb < num_parameter_bands; pb++) {
      m1_param->m1_param_real[0][0][ps][pb] = ONE_IN_Q15;
      m1_param->m1_param_real[3][0][ps][pb] = ONE_IN_Q15;
    }

    memcpy(m2_param->m2_decor_real[0][ps], h12, num_parameter_bands * sizeof(h12[0]));
    memcpy(m2_param->m2_decor_real[1][ps], h22, num_parameter_bands * sizeof(h22[0]));
    memcpy(m2_param->m2_resid_real[0][ps], h11, num_parameter_bands * sizeof(h11[0]));
    memcpy(m2_param->m2_resid_real[1][ps], h21, num_parameter_bands * sizeof(h21[0]));
  }
  return;
}

VOID ixheaacd_calc_m1m2_51s2(ia_heaac_mps_state_struct *pstr_mps_state) {
  ia_mps_dec_auxilary_struct *p_aux_struct = pstr_mps_state->aux_struct;
  ia_mps_dec_m2_param_struct *m2_param = p_aux_struct->m2_param;
  ia_mps_dec_m1_param_struct *m1_param = pstr_mps_state->array_struct->m1_param;
  WORD32 ps, pb;

  WORD32 *iid;
  WORD32 *icc;
  WORD32 *h11;
  WORD32 *h12;
  WORD32 *h21;
  WORD32 *h22;
  WORD32 *h12_res;
  WORD32 *h22_res;
  WORD32 *g_s;
  WORD16 *c_l;
  WORD16 *c_r;

  const WORD32 *sqrt_tab = pstr_mps_state->ia_mps_dec_mps_table.common_table_ptr->sqrt_tab;
  WORD32 num_parameter_sets = pstr_mps_state->num_parameter_sets;
  WORD32 num_parameter_bands = pstr_mps_state->num_parameter_bands;

  iid = pstr_mps_state->mps_scratch_mem_v;
  icc = iid + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*icc), BYTE_ALIGN_8);
  h11 = icc + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h11), BYTE_ALIGN_8);
  h12 = h11 + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h12), BYTE_ALIGN_8);
  h21 = h12 + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h21), BYTE_ALIGN_8);
  h22 = h21 + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h22), BYTE_ALIGN_8);
  h12_res =
      h22 + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h12_res), BYTE_ALIGN_8);
  h22_res = h12_res +
            IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h22_res), BYTE_ALIGN_8);
  g_s = h22_res + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*g_s), BYTE_ALIGN_8);
  c_l = (WORD16 *)pstr_mps_state->mps_scratch_mem_v +
        IXHEAAC_GET_SIZE_ALIGNED_TYPE(PARAMETER_BANDSX18, sizeof(*c_l), BYTE_ALIGN_8);
  c_r = c_l + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*c_r), BYTE_ALIGN_8);

  for (ps = 0; ps < num_parameter_sets; ps++) {
    for (pb = 0; pb < num_parameter_bands; pb++) {
      WORD32 p_l_c, p_r_c;
      WORD32 p_l_lr, p_r_lr;
      WORD32 left;
      WORD32 right;
      WORD32 center;
      WORD32 cross;
      WORD32 temp_1;
      WORD16 qtemp1;
      ia_mps_dec_spatial_bs_frame_struct *p_cur_bs = pstr_mps_state->bs_frame;

      p_l_c = pstr_mps_state->ia_mps_dec_mps_table.m1_m2_table_ptr
                  ->cld_tab_1[p_cur_bs->ott_cld_idx[0][ps][pb] + 15];
      p_r_c = pstr_mps_state->ia_mps_dec_mps_table.m1_m2_table_ptr
                  ->cld_tab_1[15 - p_cur_bs->ott_cld_idx[0][ps][pb]];

      p_l_lr = pstr_mps_state->ia_mps_dec_mps_table.m1_m2_table_ptr
                   ->cld_tab_1[p_cur_bs->ott_cld_idx[1][ps][pb] + 15];
      p_r_lr = pstr_mps_state->ia_mps_dec_mps_table.m1_m2_table_ptr
                   ->cld_tab_1[15 - p_cur_bs->ott_cld_idx[1][ps][pb]];

      left = ixheaacd_mps_mult32_shr_15(p_l_c, p_l_lr);
      right = ixheaacd_mps_mult32_shr_15(p_l_c, p_r_lr);
      center = (p_r_c) >> 1;

      temp_1 = ixheaacd_mps_mult32_shr_15(left, right);
      qtemp1 = 15;
      temp_1 = ixheaacd_mps_sqrt(temp_1, &qtemp1, sqrt_tab);
      temp_1 = ixheaacd_mps_convert_to_qn(temp_1, qtemp1, 15);
      cross = ixheaacd_mps_mult32_shr_15(temp_1, p_aux_struct->ott_icc[1][ps][pb]);

      temp_1 = ixheaac_add32_sat((left + right), cross);
      temp_1 = ixheaacd_mps_mult32_shr_15(temp_1, center);
      qtemp1 = 15;
      temp_1 = ixheaacd_mps_sqrt(temp_1, &qtemp1, sqrt_tab);
      temp_1 = ixheaacd_mps_convert_to_qn(temp_1, qtemp1, 15);
      temp_1 = ixheaacd_mps_mult32_shr_15(temp_1, p_aux_struct->ott_icc[0][ps][pb]);
      temp_1 = ixheaac_add32_sat(temp_1, center);
      cross = ixheaac_add32_sat(cross, temp_1);

      temp_1 = ixheaacd_mps_mult32_shr_15(left, center);
      qtemp1 = 15;
      temp_1 = ixheaacd_mps_sqrt(temp_1, &qtemp1, sqrt_tab);
      temp_1 = ixheaacd_mps_convert_to_qn(2 * temp_1, qtemp1, 15);
      temp_1 = ixheaacd_mps_mult32_shr_15(temp_1, p_aux_struct->ott_icc[0][ps][pb]);
      temp_1 = ixheaac_add32_sat(temp_1, center);
      left = ixheaac_add32_sat(left, temp_1);

      temp_1 = ixheaacd_mps_mult32_shr_15(right, center);
      qtemp1 = 15;
      temp_1 = ixheaacd_mps_sqrt(temp_1, &qtemp1, sqrt_tab);
      temp_1 = ixheaacd_mps_convert_to_qn(2 * temp_1, qtemp1, 15);
      temp_1 = ixheaacd_mps_mult32_shr_15(temp_1, p_aux_struct->ott_icc[0][ps][pb]);
      temp_1 = ixheaac_add32_sat(temp_1, center);
      right = ixheaac_add32_sat(right, temp_1);

      temp_1 = ixheaacd_mps_div32_in_q15(left, right);
      qtemp1 = 15;
      iid[pb] = ((10 * ixheaacd_mps_log10(temp_1, qtemp1)) >> 1);

      temp_1 = ixheaacd_mps_mult32_shr_15(left, right);
      qtemp1 = 15;
      temp_1 = ixheaacd_mps_sqrt(temp_1, &qtemp1, sqrt_tab);
      temp_1 = ixheaacd_mps_convert_to_qn(temp_1, qtemp1, 15);
      if (temp_1 == 0) {
        temp_1 = 1;
      }

      icc[pb] = ixheaacd_mps_div32_in_q15(cross, temp_1);

      temp_1 = ixheaac_add32_sat(left, right);
      qtemp1 = 15;
      temp_1 = ixheaacd_mps_sqrt(temp_1, &qtemp1, sqrt_tab);
      g_s[pb] = ixheaacd_mps_convert_to_qn(temp_1, qtemp1, 15);

      if (icc[pb] > ONE_IN_Q15) {
        icc[pb] = ONE_IN_Q15;
      } else {
        if (icc[pb] < MINUS_POINT_NINE_NINE_Q15) {
          icc[pb] = MINUS_POINT_NINE_NINE_Q15;
        }
      }

      iid[pb] =
          ixheaacd_quantize_cld(iid[pb], pstr_mps_state->ia_mps_dec_mps_table.bitdec_table_ptr);
      icc[pb] =
          ixheaacd_quantize_icc(icc[pb], pstr_mps_state->ia_mps_dec_mps_table.bitdec_table_ptr);
    }

    ixheaacd_param_2_umx_ps_core_tables(iid, icc, num_parameter_bands, 0, h11, h12, h21, h22,
                                        h12_res, h22_res, c_l, c_r,
                                        pstr_mps_state->ia_mps_dec_mps_table.m1_m2_table_ptr);

    for (pb = 0; pb < num_parameter_bands; pb++) {
      m1_param->m1_param_real[0][0][ps][pb] = ONE_IN_Q15;
      m1_param->m1_param_real[2][0][ps][pb] = ONE_IN_Q15;
    }

    for (pb = 0; pb < num_parameter_bands; pb++) {
      m2_param->m2_decor_real[0][ps][pb] = ixheaacd_mps_mult32_shr_15(g_s[pb], h12[pb]);
      m2_param->m2_decor_real[1][ps][pb] = ixheaacd_mps_mult32_shr_15(g_s[pb], h22[pb]);
    }

    for (pb = 0; pb < num_parameter_bands; pb++) {
      m2_param->m2_resid_real[0][ps][pb] = ixheaacd_mps_mult32_shr_15(g_s[pb], h11[pb]);
      m2_param->m2_resid_real[1][ps][pb] = ixheaacd_mps_mult32_shr_15(g_s[pb], h21[pb]);
    }
  }
  return;
}
