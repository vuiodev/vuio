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
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops16.h"
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

VOID ixheaacd_calc_m1m2_5151(ia_heaac_mps_state_struct *pstr_mps_state) {
  ia_mps_dec_auxilary_struct *p_aux_struct = pstr_mps_state->aux_struct;
  ia_mps_dec_m2_param_struct *m2_param = p_aux_struct->m2_param;
  ia_mps_dec_m1_param_struct *m1_param = pstr_mps_state->array_struct->m1_param;
  ia_heaac_mps_state_struct *curr_state = pstr_mps_state;
  WORD32 ps, pb;
  WORD32 *h11_fs, *h11_c;
  WORD32 *h12_fs, *h12_c;
  WORD32 *h21_fs, *h21_c;
  WORD32 *h22_fs, *h22_c;
  WORD32 *h12_res_fs, *h12_res_c;
  WORD32 *h22_res_fs, *h22_res_c;
  WORD16 *c_l_fs, *c_l_c;
  WORD16 *c_r_fs, *c_r_c;

  WORD32 *h11_f, *h11_s;
  WORD32 *h12_f, *h12_s;
  WORD32 *h21_f, *h21_s;
  WORD32 *h22_f, *h22_s;
  WORD32 *h12_res_f, *h12_res_s, *c_r_clfe;
  WORD32 *h22_res_f, *h22_res_s, *c_l_clfe;
  WORD16 *c_l_f, *c_l_s;
  WORD16 *c_r_f, *c_r_s;

  WORD32 idx;
  WORD32 residual_coding = pstr_mps_state->residual_coding;
  WORD32 num_parameter_sets = pstr_mps_state->num_parameter_sets;
  WORD32 num_parameter_bands = pstr_mps_state->num_parameter_bands;
  WORD32 arbitrary_downmix = pstr_mps_state->arbitrary_downmix;
  WORD32 *res_bands = pstr_mps_state->res_bands;

  h11_fs = pstr_mps_state->mps_scratch_mem_v;
  h11_c =
      h11_fs + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h11_c), BYTE_ALIGN_8);
  h12_fs =
      h11_c + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h12_fs), BYTE_ALIGN_8);
  h12_c =
      h12_fs + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h12_c), BYTE_ALIGN_8);
  h21_fs =
      h12_c + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h21_fs), BYTE_ALIGN_8);
  h21_c =
      h21_fs + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h21_c), BYTE_ALIGN_8);
  h22_fs =
      h21_c + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h22_fs), BYTE_ALIGN_8);
  h22_c =
      h22_fs + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h22_c), BYTE_ALIGN_8);
  h12_res_fs = h22_c + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h12_res_fs),
                                                     BYTE_ALIGN_8);
  h12_res_c = h12_res_fs + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h12_res_c),
                                                         BYTE_ALIGN_8);
  h22_res_fs = h12_res_c + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h22_res_fs),
                                                         BYTE_ALIGN_8);
  h22_res_c = h22_res_fs + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h22_res_c),
                                                         BYTE_ALIGN_8);
  h11_f = h22_res_c +
          IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h11_f), BYTE_ALIGN_8);
  h11_s =
      h11_f + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h11_s), BYTE_ALIGN_8);
  h12_f =
      h11_s + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h12_f), BYTE_ALIGN_8);
  h12_s =
      h12_f + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h12_s), BYTE_ALIGN_8);
  h21_f =
      h12_s + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h21_f), BYTE_ALIGN_8);
  h21_s =
      h21_f + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h21_s), BYTE_ALIGN_8);
  h22_f =
      h21_s + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h22_f), BYTE_ALIGN_8);
  h22_s =
      h22_f + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h22_s), BYTE_ALIGN_8);
  h12_res_f = h22_s + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h12_res_f),
                                                    BYTE_ALIGN_8);
  h12_res_s = h12_res_f + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h12_res_s),
                                                        BYTE_ALIGN_8);
  h22_res_f = h12_res_s + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h22_res_f),
                                                        BYTE_ALIGN_8);
  h22_res_s = h22_res_f + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h22_res_s),
                                                        BYTE_ALIGN_8);
  c_r_clfe = h22_res_s +
             IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*c_r_clfe), BYTE_ALIGN_8);
  c_l_clfe = c_r_clfe +
             IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*c_l_clfe), BYTE_ALIGN_8);

  c_l_fs = (WORD16 *)pstr_mps_state->mps_scratch_mem_v +
           IXHEAAC_GET_SIZE_ALIGNED_TYPE(PARAMETER_BANDSX52, sizeof(*c_l_fs), BYTE_ALIGN_8);
  c_l_c =
      c_l_fs + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*c_l_c), BYTE_ALIGN_8);
  c_r_fs =
      c_l_c + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*c_r_fs), BYTE_ALIGN_8);
  c_r_c =
      c_r_fs + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*c_r_c), BYTE_ALIGN_8);
  c_l_f =
      c_r_c + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*c_l_f), BYTE_ALIGN_8);
  c_l_s =
      c_l_f + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*c_l_s), BYTE_ALIGN_8);
  c_r_f =
      c_l_s + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*c_r_f), BYTE_ALIGN_8);
  c_r_s =
      c_r_f + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*c_r_s), BYTE_ALIGN_8);

  for (ps = 0; ps < num_parameter_sets; ps++) {
    ixheaacd_param_2_umx_ps(pstr_mps_state, h11_fs, h12_fs, h21_fs, h22_fs, h12_res_fs,
                            h22_res_fs, c_l_fs, c_r_fs, 0, ps, res_bands[0]);
    ixheaacd_param_2_umx_ps(pstr_mps_state, h11_c, h12_c, h21_c, h22_c, h12_res_c, h22_res_c,
                            c_l_c, c_r_c, 1, ps, res_bands[1]);
    ixheaacd_param_2_umx_ps(pstr_mps_state, h11_s, h12_s, h21_s, h22_s, h12_res_s, h22_res_s,
                            c_l_s, c_r_s, 2, ps, res_bands[2]);
    ixheaacd_param_2_umx_ps(pstr_mps_state, h11_f, h12_f, h21_f, h22_f, h12_res_f, h22_res_f,
                            c_l_f, c_r_f, 3, ps, res_bands[3]);

    for (pb = 0; pb < p_aux_struct->num_ott_bands[4]; pb++) {
      ia_mps_dec_spatial_bs_frame_struct *p_cur_bs = pstr_mps_state->bs_frame;
      c_l_clfe[pb] = pstr_mps_state->ia_mps_dec_mps_table.m1_m2_table_ptr
                         ->r1_matrix_l[p_cur_bs->ott_cld_idx[4][ps][pb] + 15];
      c_r_clfe[pb] = pstr_mps_state->ia_mps_dec_mps_table.m1_m2_table_ptr
                         ->r1_matrix_l[15 - p_cur_bs->ott_cld_idx[4][ps][pb]];
    }

    for (pb = p_aux_struct->num_ott_bands[4]; pb < num_parameter_bands; pb++) {
      c_l_clfe[pb] = ONE_IN_Q15;
      c_r_clfe[pb] = 0;
    }

    for (pb = 0; pb < num_parameter_bands; pb++) {
      m1_param->m1_param_real[0][0][ps][pb] = ONE_IN_Q15;
      m1_param->m1_param_real[1][0][ps][pb] = ONE_IN_Q15;
      m1_param->m1_param_real[2][0][ps][pb] = c_l_fs[pb];
      m1_param->m1_param_real[3][0][ps][pb] = ixheaac_mult16_shl(c_l_fs[pb], c_l_c[pb]);
      m1_param->m1_param_real[4][0][ps][pb] = c_r_fs[pb];
    }

    for (pb = 0; pb < num_parameter_bands; pb++) {
      m2_param->m2_decor_real[0][ps][pb] = ixheaacd_mps_mult32_shr_15(
          h11_f[pb], ixheaacd_mps_mult32_shr_15(h11_c[pb], h12_fs[pb]));
      m2_param->m2_decor_real[1][ps][pb] = ixheaacd_mps_mult32_shr_15(h11_f[pb], h12_c[pb]);
      m2_param->m2_decor_real[2][ps][pb] = h12_f[pb];

      m2_param->m2_decor_real[3][ps][pb] = ixheaacd_mps_mult32_shr_15(
          h21_f[pb], ixheaacd_mps_mult32_shr_15(h11_c[pb], h12_fs[pb]));
      m2_param->m2_decor_real[4][ps][pb] = ixheaacd_mps_mult32_shr_15(h21_f[pb], h12_c[pb]);
      m2_param->m2_decor_real[5][ps][pb] = h22_f[pb];

      m2_param->m2_decor_real[6][ps][pb] = ixheaacd_mps_mult32_shr_15(
          c_l_clfe[pb], ixheaacd_mps_mult32_shr_15(h21_c[pb], h12_fs[pb]));
      m2_param->m2_decor_real[7][ps][pb] = ixheaacd_mps_mult32_shr_15(c_l_clfe[pb], h22_c[pb]);

      m2_param->m2_decor_real[8][ps][pb] = ixheaacd_mps_mult32_shr_15(h11_s[pb], h22_fs[pb]);
      m2_param->m2_decor_real[9][ps][pb] = h12_s[pb];

      m2_param->m2_decor_real[10][ps][pb] = ixheaacd_mps_mult32_shr_15(h21_s[pb], h22_fs[pb]);
      m2_param->m2_decor_real[11][ps][pb] = h22_s[pb];
    }

    for (pb = 0; pb < num_parameter_bands; pb++) {
      idx = 0;

      m2_param->m2_resid_real[idx++][ps][pb] = ixheaacd_mps_mult32_shr_15(
          h11_f[pb], ixheaacd_mps_mult32_shr_15(h11_c[pb], h11_fs[pb]));

      if (residual_coding) {
        m2_param->m2_resid_real[idx++][ps][pb] = ixheaacd_mps_mult32_shr_15(
            h11_f[pb], ixheaacd_mps_mult32_shr_15(h11_c[pb], h12_res_fs[pb]));
        m2_param->m2_resid_real[idx++][ps][pb] =
            ixheaacd_mps_mult32_shr_15(h11_f[pb], h12_res_c[pb]);
        m2_param->m2_resid_real[idx++][ps][pb] = h12_res_f[pb];
      }

      m2_param->m2_resid_real[idx++][ps][pb] = ixheaacd_mps_mult32_shr_15(
          h21_f[pb], ixheaacd_mps_mult32_shr_15(h11_c[pb], h11_fs[pb]));

      if (residual_coding) {
        m2_param->m2_resid_real[idx++][ps][pb] = ixheaacd_mps_mult32_shr_15(
            h21_f[pb], ixheaacd_mps_mult32_shr_15(h11_c[pb], h12_res_fs[pb]));
        m2_param->m2_resid_real[idx++][ps][pb] =
            ixheaacd_mps_mult32_shr_15(h21_f[pb], h12_res_c[pb]);
        m2_param->m2_resid_real[idx++][ps][pb] = h22_res_f[pb];
      }

      m2_param->m2_resid_real[idx++][ps][pb] = ixheaacd_mps_mult32_shr_15(
          c_l_clfe[pb], ixheaacd_mps_mult32_shr_15(h21_c[pb], h11_fs[pb]));

      if (residual_coding) {
        m2_param->m2_resid_real[idx++][ps][pb] = ixheaacd_mps_mult32_shr_15(
            c_l_clfe[pb], ixheaacd_mps_mult32_shr_15(h21_c[pb], h12_res_fs[pb]));
        m2_param->m2_resid_real[idx++][ps][pb] =
            ixheaacd_mps_mult32_shr_15(c_l_clfe[pb], h22_res_c[pb]);
      }

      m2_param->m2_resid_real[idx++][ps][pb] = ixheaacd_mps_mult32_shr_15(
          c_r_clfe[pb], ixheaacd_mps_mult32_shr_15(c_r_c[pb], c_l_fs[pb]));

      m2_param->m2_resid_real[idx++][ps][pb] = ixheaacd_mps_mult32_shr_15(h11_s[pb], h21_fs[pb]);

      if (residual_coding) {
        m2_param->m2_resid_real[idx++][ps][pb] =
            ixheaacd_mps_mult32_shr_15(h11_s[pb], h22_res_fs[pb]);
        m2_param->m2_resid_real[idx++][ps][pb] = h12_res_s[pb];
      }

      m2_param->m2_resid_real[idx++][ps][pb] = ixheaacd_mps_mult32_shr_15(h21_s[pb], h21_fs[pb]);

      if (residual_coding) {
        m2_param->m2_resid_real[idx++][ps][pb] =
            ixheaacd_mps_mult32_shr_15(h21_s[pb], h22_res_fs[pb]);
        m2_param->m2_resid_real[idx++][ps][pb] = h22_res_s[pb];
      }
    }
  }

  if (arbitrary_downmix > 0) {
    for (ps = 0; ps < num_parameter_sets; ps++) {
      for (pb = 0; pb < num_parameter_bands; pb++) {
        WORD32 g_real[1];
        ixheaacd_calculate_arb_dmx_mtx(pstr_mps_state, ps, pb, g_real);

        if (arbitrary_downmix == 2) {
          m1_param->m1_param_real[0][1][ps][pb] = m1_param->m1_param_real[0][0][ps][pb];
          m1_param->m1_param_real[1][1][ps][pb] = m1_param->m1_param_real[1][0][ps][pb];
          m1_param->m1_param_real[2][1][ps][pb] = m1_param->m1_param_real[2][0][ps][pb];
          m1_param->m1_param_real[3][1][ps][pb] = m1_param->m1_param_real[3][0][ps][pb];
          m1_param->m1_param_real[4][1][ps][pb] = m1_param->m1_param_real[4][0][ps][pb];

          curr_state->m1_param_present[0][1] = 1;
          curr_state->m1_param_present[1][1] = 1;
          curr_state->m1_param_present[2][1] = 1;
          curr_state->m1_param_present[3][1] = 1;
          curr_state->m1_param_present[4][1] = 1;
        }

        m1_param->m1_param_real[0][0][ps][pb] =
            ixheaacd_mps_mult32_shr_15(m1_param->m1_param_real[0][0][ps][pb], g_real[0]);
        m1_param->m1_param_real[1][0][ps][pb] =
            ixheaacd_mps_mult32_shr_15(m1_param->m1_param_real[1][0][ps][pb], g_real[0]);
        m1_param->m1_param_real[2][0][ps][pb] =
            ixheaacd_mps_mult32_shr_15(m1_param->m1_param_real[2][0][ps][pb], g_real[0]);
        m1_param->m1_param_real[3][0][ps][pb] =
            ixheaacd_mps_mult32_shr_15(m1_param->m1_param_real[3][0][ps][pb], g_real[0]);
        m1_param->m1_param_real[4][0][ps][pb] =
            ixheaacd_mps_mult32_shr_15(m1_param->m1_param_real[4][0][ps][pb], g_real[0]);
      }
    }
  }
  return;
}

VOID ixheaacd_calc_m1m2_5152(ia_heaac_mps_state_struct *pstr_mps_state) {
  ia_mps_dec_auxilary_struct *p_aux_struct = pstr_mps_state->aux_struct;
  ia_mps_dec_m2_param_struct *m2_param = p_aux_struct->m2_param;
  ia_mps_dec_m1_param_struct *m1_param = pstr_mps_state->array_struct->m1_param;
  ia_heaac_mps_state_struct *curr_state = pstr_mps_state;
  WORD32 ps, pb;

  WORD32 *h11_lr, *h11_c, *h12_lr, *h12_c, *h21_lr, *h21_c, *h22_lr, *h22_c, *h12_res_lr,
      *h12_res_c, *h22_res_lr, *h22_res_c;
  WORD16 *c_l_lr, *c_l_c, *c_r_lr, *c_r_c;

  WORD32 *h11_l, *h11_r, *h12_l, *h12_r, *h21_l, *h21_r, *h22_l, *h22_r, *h12_res_l, *h12_res_r,
      *h22_res_l, *h22_res_r, *c_l_clfe, *c_r_clfe;
  WORD16 *c_l_l, *c_l_r, *c_r_l, *c_r_r;

  WORD32 idx;
  WORD32 num_parameter_sets = pstr_mps_state->num_parameter_sets;
  WORD32 residual_coding = pstr_mps_state->residual_coding;
  WORD32 num_parameter_bands = pstr_mps_state->num_parameter_bands;
  WORD32 arbitrary_downmix = pstr_mps_state->arbitrary_downmix;
  WORD32 *res_bands = pstr_mps_state->res_bands;

  h11_lr = pstr_mps_state->mps_scratch_mem_v;
  h11_c =
      h11_lr + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h11_c), BYTE_ALIGN_8);
  h12_lr =
      h11_c + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h12_lr), BYTE_ALIGN_8);
  h12_c =
      h12_lr + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h12_c), BYTE_ALIGN_8);
  h21_lr =
      h12_c + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h21_lr), BYTE_ALIGN_8);
  h21_c =
      h21_lr + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h21_c), BYTE_ALIGN_8);
  h22_lr =
      h21_c + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h22_lr), BYTE_ALIGN_8);
  h22_c =
      h22_lr + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h22_c), BYTE_ALIGN_8);
  h12_res_lr = h22_c + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h12_res_lr),
                                                     BYTE_ALIGN_8);
  h12_res_c = h12_res_lr + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h12_res_c),
                                                         BYTE_ALIGN_8);
  h22_res_lr = h12_res_c + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h22_res_lr),
                                                         BYTE_ALIGN_8);
  h22_res_c = h22_res_lr + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h22_res_c),
                                                         BYTE_ALIGN_8);
  h11_l = h22_res_c +
          IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h11_l), BYTE_ALIGN_8);
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
  c_r_clfe = h22_res_r +
             IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*c_r_clfe), BYTE_ALIGN_8);
  c_l_clfe = c_r_clfe +
             IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*c_l_clfe), BYTE_ALIGN_8);

  c_l_lr = (WORD16 *)pstr_mps_state->mps_scratch_mem_v +
           IXHEAAC_GET_SIZE_ALIGNED_TYPE(PARAMETER_BANDSX52, sizeof(*c_l_lr), BYTE_ALIGN_8);
  c_l_c =
      c_l_lr + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*c_l_c), BYTE_ALIGN_8);
  c_r_lr =
      c_l_c + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*c_r_lr), BYTE_ALIGN_8);
  c_r_c =
      c_r_lr + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*c_r_c), BYTE_ALIGN_8);
  c_l_l =
      c_r_c + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*c_l_l), BYTE_ALIGN_8);
  c_l_r =
      c_l_l + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*c_l_r), BYTE_ALIGN_8);
  c_r_l =
      c_l_r + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*c_r_l), BYTE_ALIGN_8);
  c_r_r =
      c_r_l + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*c_r_r), BYTE_ALIGN_8);

  for (ps = 0; ps < num_parameter_sets; ps++) {
    ixheaacd_param_2_umx_ps(pstr_mps_state, h11_c, h12_c, h21_c, h22_c, h12_res_c, h22_res_c,
                            c_l_c, c_r_c, 0, ps, res_bands[0]);
    ixheaacd_param_2_umx_ps(pstr_mps_state, h11_lr, h12_lr, h21_lr, h22_lr, h12_res_lr,
                            h22_res_lr, c_l_lr, c_r_lr, 1, ps, res_bands[1]);
    ixheaacd_param_2_umx_ps(pstr_mps_state, h11_l, h12_l, h21_l, h22_l, h12_res_l, h22_res_l,
                            c_l_l, c_r_l, 3, ps, res_bands[3]);
    ixheaacd_param_2_umx_ps(pstr_mps_state, h11_r, h12_r, h21_r, h22_r, h12_res_r, h22_res_r,
                            c_l_r, c_r_r, 4, ps, res_bands[4]);

    for (pb = 0; pb < p_aux_struct->num_ott_bands[2]; pb++) {
      ia_mps_dec_spatial_bs_frame_struct *p_cur_bs = pstr_mps_state->bs_frame;
      c_l_clfe[pb] = pstr_mps_state->ia_mps_dec_mps_table.m1_m2_table_ptr
                         ->r1_matrix_l[p_cur_bs->ott_cld_idx[2][ps][pb] + 15];
      c_r_clfe[pb] = pstr_mps_state->ia_mps_dec_mps_table.m1_m2_table_ptr
                         ->r1_matrix_l[15 - p_cur_bs->ott_cld_idx[2][ps][pb]];
    }

    for (pb = p_aux_struct->num_ott_bands[2]; pb < num_parameter_bands; pb++) {
      c_l_clfe[pb] = ONE_IN_Q15;
      c_r_clfe[pb] = 0;
    }

    for (pb = 0; pb < num_parameter_bands; pb++) {
      m1_param->m1_param_real[0][0][ps][pb] = ONE_IN_Q15;
      m1_param->m1_param_real[1][0][ps][pb] = ONE_IN_Q15;
      m1_param->m1_param_real[2][0][ps][pb] = c_l_c[pb];
      m1_param->m1_param_real[3][0][ps][pb] = ixheaac_mult16_shl(c_l_c[pb], c_l_lr[pb]);
      m1_param->m1_param_real[4][0][ps][pb] = ixheaac_mult16_shl(c_l_c[pb], c_r_lr[pb]);
    }

    for (pb = 0; pb < num_parameter_bands; pb++) {
      m2_param->m2_decor_real[0][ps][pb] = ixheaacd_mps_mult32_shr_15(
          h11_l[pb], ixheaacd_mps_mult32_shr_15(h11_lr[pb], h12_c[pb]));
      m2_param->m2_decor_real[1][ps][pb] = ixheaacd_mps_mult32_shr_15(h11_l[pb], h12_lr[pb]);
      m2_param->m2_decor_real[2][ps][pb] = h12_l[pb];

      m2_param->m2_decor_real[3][ps][pb] = ixheaacd_mps_mult32_shr_15(
          h21_l[pb], ixheaacd_mps_mult32_shr_15(h11_lr[pb], h12_c[pb]));
      m2_param->m2_decor_real[4][ps][pb] = ixheaacd_mps_mult32_shr_15(h21_l[pb], h12_lr[pb]);
      m2_param->m2_decor_real[5][ps][pb] = h22_l[pb];

      m2_param->m2_decor_real[6][ps][pb] = ixheaacd_mps_mult32_shr_15(
          h11_r[pb], ixheaacd_mps_mult32_shr_15(h21_lr[pb], h12_c[pb]));
      m2_param->m2_decor_real[7][ps][pb] = ixheaacd_mps_mult32_shr_15(h11_r[pb], h22_lr[pb]);
      m2_param->m2_decor_real[8][ps][pb] = h12_r[pb];

      m2_param->m2_decor_real[9][ps][pb] = ixheaacd_mps_mult32_shr_15(
          h21_r[pb], ixheaacd_mps_mult32_shr_15(h21_lr[pb], h12_c[pb]));
      m2_param->m2_decor_real[10][ps][pb] = ixheaacd_mps_mult32_shr_15(h21_r[pb], h22_lr[pb]);
      m2_param->m2_decor_real[11][ps][pb] = h22_r[pb];

      m2_param->m2_decor_real[12][ps][pb] = ixheaacd_mps_mult32_shr_15(c_l_clfe[pb], h22_c[pb]);
    }

    for (pb = 0; pb < num_parameter_bands; pb++) {
      idx = 0;

      m2_param->m2_resid_real[idx++][ps][pb] = ixheaacd_mps_mult32_shr_15(
          h11_l[pb], ixheaacd_mps_mult32_shr_15(h11_lr[pb], h11_c[pb]));

      if (residual_coding) {
        m2_param->m2_resid_real[idx++][ps][pb] = ixheaacd_mps_mult32_shr_15(
            h11_l[pb], ixheaacd_mps_mult32_shr_15(h11_lr[pb], h12_res_c[pb]));
        m2_param->m2_resid_real[idx++][ps][pb] =
            ixheaacd_mps_mult32_shr_15(h11_l[pb], h12_res_lr[pb]);
        m2_param->m2_resid_real[idx++][ps][pb] = h12_res_l[pb];
      }

      m2_param->m2_resid_real[idx++][ps][pb] = ixheaacd_mps_mult32_shr_15(
          h21_l[pb], ixheaacd_mps_mult32_shr_15(h11_lr[pb], h11_c[pb]));

      if (residual_coding) {
        m2_param->m2_resid_real[idx++][ps][pb] = ixheaacd_mps_mult32_shr_15(
            h21_l[pb], ixheaacd_mps_mult32_shr_15(h11_lr[pb], h12_res_c[pb]));
        m2_param->m2_resid_real[idx++][ps][pb] =
            ixheaacd_mps_mult32_shr_15(h21_l[pb], h12_res_lr[pb]);
        m2_param->m2_resid_real[idx++][ps][pb] = h22_res_l[pb];
      }

      m2_param->m2_resid_real[idx++][ps][pb] = ixheaacd_mps_mult32_shr_15(
          h11_r[pb], ixheaacd_mps_mult32_shr_15(h21_lr[pb], h11_c[pb]));

      if (residual_coding) {
        m2_param->m2_resid_real[idx++][ps][pb] = ixheaacd_mps_mult32_shr_15(
            h11_r[pb], ixheaacd_mps_mult32_shr_15(h21_lr[pb], h12_res_c[pb]));
        m2_param->m2_resid_real[idx++][ps][pb] =
            ixheaacd_mps_mult32_shr_15(h11_r[pb], h22_res_lr[pb]);
        m2_param->m2_resid_real[idx++][ps][pb] = h12_res_r[pb];
      }

      m2_param->m2_resid_real[idx++][ps][pb] = ixheaacd_mps_mult32_shr_15(
          h21_r[pb], ixheaacd_mps_mult32_shr_15(h21_lr[pb], h11_c[pb]));

      if (residual_coding) {
        m2_param->m2_resid_real[idx++][ps][pb] = ixheaacd_mps_mult32_shr_15(
            h21_r[pb], ixheaacd_mps_mult32_shr_15(h21_lr[pb], h12_res_c[pb]));
        m2_param->m2_resid_real[idx++][ps][pb] =
            ixheaacd_mps_mult32_shr_15(h21_r[pb], h22_res_lr[pb]);
        m2_param->m2_resid_real[idx++][ps][pb] = h22_res_r[pb];
      }

      m2_param->m2_resid_real[idx++][ps][pb] =
          ixheaacd_mps_mult32_shr_15(c_l_clfe[pb], h21_c[pb]);

      if (residual_coding)
        m2_param->m2_resid_real[idx++][ps][pb] =
            ixheaacd_mps_mult32_shr_15(c_l_clfe[pb], h22_res_c[pb]);

      m2_param->m2_resid_real[idx++][ps][pb] =
          ixheaacd_mps_mult32_shr_15(c_r_c[pb], c_r_clfe[pb]);
    }
  }

  if (arbitrary_downmix > 0) {
    for (ps = 0; ps < num_parameter_sets; ps++) {
      for (pb = 0; pb < num_parameter_bands; pb++) {
        WORD32 g_real[1];
        ixheaacd_calculate_arb_dmx_mtx(pstr_mps_state, ps, pb, g_real);

        if (arbitrary_downmix == 2) {
          m1_param->m1_param_real[0][1][ps][pb] = m1_param->m1_param_real[0][0][ps][pb];
          m1_param->m1_param_real[1][1][ps][pb] = m1_param->m1_param_real[1][0][ps][pb];
          m1_param->m1_param_real[2][1][ps][pb] = m1_param->m1_param_real[2][0][ps][pb];
          m1_param->m1_param_real[3][1][ps][pb] = m1_param->m1_param_real[3][0][ps][pb];
          m1_param->m1_param_real[4][1][ps][pb] = m1_param->m1_param_real[4][0][ps][pb];

          curr_state->m1_param_present[0][1] = 1;
          curr_state->m1_param_present[1][1] = 1;
          curr_state->m1_param_present[2][1] = 1;
          curr_state->m1_param_present[3][1] = 1;
          curr_state->m1_param_present[4][1] = 1;
        }

        m1_param->m1_param_real[0][0][ps][pb] =
            ixheaacd_mps_mult32_shr_15(m1_param->m1_param_real[0][0][ps][pb], g_real[0]);
        m1_param->m1_param_real[1][0][ps][pb] =
            ixheaacd_mps_mult32_shr_15(m1_param->m1_param_real[1][0][ps][pb], g_real[0]);
        m1_param->m1_param_real[2][0][ps][pb] =
            ixheaacd_mps_mult32_shr_15(m1_param->m1_param_real[2][0][ps][pb], g_real[0]);
        m1_param->m1_param_real[3][0][ps][pb] =
            ixheaacd_mps_mult32_shr_15(m1_param->m1_param_real[3][0][ps][pb], g_real[0]);
        m1_param->m1_param_real[4][0][ps][pb] =
            ixheaacd_mps_mult32_shr_15(m1_param->m1_param_real[4][0][ps][pb], g_real[0]);
      }
    }
  }
  return;
}
