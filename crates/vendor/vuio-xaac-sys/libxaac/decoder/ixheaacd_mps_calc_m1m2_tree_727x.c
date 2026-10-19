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

VOID ixheaacd_calc_m1m2_7271(ia_heaac_mps_state_struct *pstr_mps_state) {
  ia_mps_dec_auxilary_struct *p_aux_struct = pstr_mps_state->aux_struct;
  ia_mps_dec_m2_param_struct *m2_param = p_aux_struct->m2_param;
  ia_mps_dec_m1_param_struct *m1_param = pstr_mps_state->array_struct->m1_param;
  ia_heaac_mps_state_struct *curr_state = pstr_mps_state;
  WORD32 ps, pb, col, row, i;

  WORD32 m_pre[3][5];
  WORD32 m_ttt[3][3];

  WORD32 g_real[2];

  WORD32 temp_1, temp_2;
  WORD64 acc;

  WORD32 *h11_l, *h11_r, *h12_l, *h12_r, *h21_l, *h21_r, *h22_l, *h22_r, *h12_res_l, *h12_res_r,
      *h22_res_l, *h22_res_r;
  WORD32 *c_l_clfe, *c_r_clfe, *kappa, *g_dd;
  WORD16 *c_f_l, *c_f_r, *dummy;

  WORD32 *h11_lc, *h11_rc, *h12_lc, *h12_rc, *h21_lc, *h21_rc, *h22_lc, *h22_rc, *h12_res_lc,
      *h12_res_rc, *h22_res_lc, *h22_res_rc;
  WORD16 *c_f_lc, *c_f_rc;

  WORD32 idx;
  WORD32 residual_coding = pstr_mps_state->residual_coding;
  WORD32 *res_bands = pstr_mps_state->res_bands;
  WORD32 num_parameter_bands = pstr_mps_state->num_parameter_bands;
  WORD32 m1_param_imag_present = pstr_mps_state->m1_param_imag_present;
  WORD32 num_input_channels = pstr_mps_state->num_input_channels;
  WORD32 num_parameter_sets = pstr_mps_state->num_parameter_sets;
  WORD32 arbitrary_downmix = pstr_mps_state->arbitrary_downmix;
  WORD32 mode_0 = p_aux_struct->ttt_config[0][0].mode;
  WORD32 mode_1 = p_aux_struct->ttt_config[1][0].mode;
  WORD32 enable_additionals = ((mode_0 == 0) || (mode_1 == 0));
  WORD32 pos[5] = {0};
  WORD32 pos_resid[5] = {0};

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
  c_l_clfe = h22_res_r +
             IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*c_l_clfe), BYTE_ALIGN_8);
  c_r_clfe = c_l_clfe +
             IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*c_r_clfe), BYTE_ALIGN_8);
  kappa =
      c_r_clfe + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*kappa), BYTE_ALIGN_8);
  g_dd = kappa + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*g_dd), BYTE_ALIGN_8);

  h11_lc =
      g_dd + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h11_lc), BYTE_ALIGN_8);
  h11_rc =
      h11_lc + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h11_rc), BYTE_ALIGN_8);
  h12_lc =
      h11_rc + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h12_lc), BYTE_ALIGN_8);
  h12_rc =
      h12_lc + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h12_rc), BYTE_ALIGN_8);
  h21_lc =
      h12_rc + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h21_lc), BYTE_ALIGN_8);
  h21_rc =
      h21_lc + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h21_rc), BYTE_ALIGN_8);
  h22_lc =
      h21_rc + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h22_lc), BYTE_ALIGN_8);
  h22_rc =
      h22_lc + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h22_rc), BYTE_ALIGN_8);
  h12_res_lc = h22_rc + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h12_res_lc),
                                                      BYTE_ALIGN_8);
  h12_res_rc = h12_res_lc + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS,
                                                          sizeof(*h12_res_rc), BYTE_ALIGN_8);
  h22_res_lc = h12_res_rc + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS,
                                                          sizeof(*h22_res_lc), BYTE_ALIGN_8);
  h22_res_rc = h22_res_lc + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS,
                                                          sizeof(*h22_res_rc), BYTE_ALIGN_8);

  c_f_l = (WORD16 *)pstr_mps_state->mps_scratch_mem_v +
          IXHEAAC_GET_SIZE_ALIGNED_TYPE(PARAMETER_BANDSX56, sizeof(*c_f_l), BYTE_ALIGN_8);
  c_f_r =
      c_f_l + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*c_f_r), BYTE_ALIGN_8);
  dummy =
      c_f_r + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*dummy), BYTE_ALIGN_8);
  c_f_lc =
      dummy + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*c_f_lc), BYTE_ALIGN_8);
  c_f_rc =
      c_f_lc + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*c_f_rc), BYTE_ALIGN_8);

  if (enable_additionals) {
    if (mode_1 == 0 &&
        (p_aux_struct->ttt_config[1][0].start_band >= p_aux_struct->ttt_config[1][0].stop_band))
      enable_additionals = 0;
    else if (mode_0 == 0 && (p_aux_struct->ttt_config[0][0].start_band >=
                             p_aux_struct->ttt_config[0][0].stop_band))
      enable_additionals = 0;
  }

  for (ps = 0; ps < num_parameter_sets; ps++) {
    ixheaacd_param_2_umx_ps(pstr_mps_state, h11_l, h12_l, h21_l, h22_l, h12_res_l, h22_res_l,
                            c_f_l, dummy, 1, ps, res_bands[1]);
    ixheaacd_param_2_umx_ps(pstr_mps_state, h11_r, h12_r, h21_r, h22_r, h12_res_r, h22_res_r,
                            c_f_r, dummy, 2, ps, res_bands[2]);
    ixheaacd_param_2_umx_ps(pstr_mps_state, h11_lc, h12_lc, h21_lc, h22_lc, h12_res_lc,
                            h22_res_lc, c_f_lc, dummy, 3, ps, res_bands[3]);
    ixheaacd_param_2_umx_ps(pstr_mps_state, h11_rc, h12_rc, h21_rc, h22_rc, h12_res_rc,
                            h22_res_rc, c_f_rc, dummy, 4, ps, res_bands[4]);

    for (i = 0; i < 2; i++) {
      for (pb = p_aux_struct->ttt_config[i][0].start_band;
           pb < p_aux_struct->ttt_config[i][0].stop_band; pb++) {
        WORD32 mtx_inversion = pstr_mps_state->mtx_inversion;

        memset(m_pre, 0, sizeof(m_pre));

        if (p_aux_struct->ttt_config[i][0].mode >= 2) {
          mtx_inversion = mtx_inversion && (p_aux_struct->ttt_config[i][0].mode == 2 ||
                                            p_aux_struct->ttt_config[i][0].mode == 4);
        }

        ixheaacd_calculate_ttt(pstr_mps_state, ps, pb, p_aux_struct->ttt_config[i][0].mode,
                               m_ttt);

        for (row = 0; row < 3; row++) {
          for (col = 0; col < 3; col++) {
            m_pre[row][col] = m_ttt[row][col];
          }
        }

        if (arbitrary_downmix != 0) {
          WORD32 ch;
          ixheaacd_calculate_arb_dmx_mtx(pstr_mps_state, ps, pb, g_real);

          for (ch = 0; ch < num_input_channels; ch++) {
            for (row = 0; row < 3; row++) {
              m_pre[row][col] = ixheaacd_mps_mult32_shr_15(m_pre[row][col], g_real[ch]);

              if (arbitrary_downmix == 2 && pb < pstr_mps_state->arbdmx_residual_bands) {
                m_pre[row][3 + ch] = m_ttt[row][ch];
              }
            }
          }
        }

        if (mtx_inversion) {
          WORD32 h_real[2][2], h_imag[2][2];

          ixheaacd_calculate_mtx_inv(pstr_mps_state, ps, pb, p_aux_struct->ttt_config[i][0].mode,
                                     h_real, h_imag);

          acc = (WORD64)((WORD64)m_pre[0][0] * (WORD64)h_real[0][0] +
                         (WORD64)m_pre[0][1] * (WORD64)h_real[1][0]);
          acc >>= 15;
          m1_param->m1_param_real[0][0][ps][pb] = (WORD32)acc;

          acc = (WORD64)((WORD64)m_pre[0][0] * (WORD64)h_real[0][1] +
                         (WORD64)m_pre[0][1] * (WORD64)h_real[1][1]);
          acc >>= 15;
          m1_param->m1_param_real[0][1][ps][pb] = (WORD32)acc;

          acc = (WORD64)((WORD64)m_pre[1][0] * (WORD64)h_real[0][0] +
                         (WORD64)m_pre[1][1] * (WORD64)h_real[1][0]);
          acc >>= 15;
          m1_param->m1_param_real[1][0][ps][pb] = (WORD32)acc;

          acc = (WORD64)((WORD64)m_pre[1][0] * (WORD64)h_real[0][1] +
                         (WORD64)m_pre[1][1] * (WORD64)h_real[1][1]);
          acc >>= 15;
          m1_param->m1_param_real[1][1][ps][pb] = (WORD32)acc;

          acc = (WORD64)((WORD64)m_pre[2][0] * (WORD64)h_real[0][0] +
                         (WORD64)m_pre[2][1] * (WORD64)h_real[1][0]);
          acc >>= 15;
          m1_param->m1_param_real[2][0][ps][pb] = (WORD32)acc;

          acc = (WORD64)((WORD64)m_pre[2][0] * (WORD64)h_real[0][1] +
                         (WORD64)m_pre[2][1] * (WORD64)h_real[1][1]);
          acc >>= 15;
          m1_param->m1_param_real[2][1][ps][pb] = (WORD32)acc;

          acc = (WORD64)((WORD64)m_pre[0][0] * (WORD64)h_imag[0][0] +
                         (WORD64)m_pre[0][1] * (WORD64)h_imag[1][0]);
          acc >>= 15;
          m1_param->m1_param_imag[0][0][ps][pb] = (WORD32)acc;

          acc = (WORD64)((WORD64)m_pre[0][0] * (WORD64)h_imag[0][1] +
                         (WORD64)m_pre[0][1] * (WORD64)h_imag[1][1]);
          acc >>= 15;
          m1_param->m1_param_imag[0][1][ps][pb] = (WORD32)acc;

          acc = (WORD64)((WORD64)m_pre[1][0] * (WORD64)h_imag[0][0] +
                         (WORD64)m_pre[1][1] * (WORD64)h_imag[1][0]);
          acc >>= 15;
          m1_param->m1_param_imag[1][0][ps][pb] = (WORD32)acc;

          acc = (WORD64)((WORD64)m_pre[1][0] * (WORD64)h_imag[0][1] +
                         (WORD64)m_pre[1][1] * (WORD64)h_imag[1][1]);
          acc >>= 15;
          m1_param->m1_param_imag[1][1][ps][pb] = (WORD32)acc;

          acc = (WORD64)((WORD64)m_pre[2][0] * (WORD64)h_imag[0][0] +
                         (WORD64)m_pre[2][1] * (WORD64)h_imag[1][0]);
          acc >>= 15;
          m1_param->m1_param_imag[2][0][ps][pb] = (WORD32)acc;

          acc = (WORD64)((WORD64)m_pre[2][0] * (WORD64)h_imag[0][1] +
                         (WORD64)m_pre[2][1] * (WORD64)h_imag[1][1]);
          acc >>= 15;
          m1_param->m1_param_imag[2][1][ps][pb] = (WORD32)acc;
        } else if (pstr_mps_state->_3d_stereo_inversion) {
        } else {
          m1_param->m1_param_real[0][0][ps][pb] = m_pre[0][0];
          m1_param->m1_param_real[0][1][ps][pb] = m_pre[0][1];
          m1_param->m1_param_real[1][0][ps][pb] = m_pre[1][0];
          m1_param->m1_param_real[1][1][ps][pb] = m_pre[1][1];
          m1_param->m1_param_real[2][0][ps][pb] = m_pre[2][0];
          m1_param->m1_param_real[2][1][ps][pb] = m_pre[2][1];
        }

        m1_param->m1_param_real[0][2][ps][pb] = m_pre[0][2];
        m1_param->m1_param_real[0][3][ps][pb] = m_pre[0][3];
        m1_param->m1_param_real[0][4][ps][pb] = m_pre[0][4];
        m1_param->m1_param_real[1][2][ps][pb] = m_pre[1][2];
        m1_param->m1_param_real[1][3][ps][pb] = m_pre[1][3];
        m1_param->m1_param_real[1][4][ps][pb] = m_pre[1][4];
        m1_param->m1_param_real[2][2][ps][pb] = m_pre[2][2];
        m1_param->m1_param_real[2][3][ps][pb] = m_pre[2][3];
        m1_param->m1_param_real[2][4][ps][pb] = m_pre[2][4];

        for (col = 0; col < pstr_mps_state->num_x_channels; col++) {
          m1_param->m1_param_real[3][col][ps][pb] = m1_param->m1_param_real[0][col][ps][pb];
          m1_param->m1_param_real[4][col][ps][pb] = m1_param->m1_param_real[1][col][ps][pb];

          curr_state->m1_param_present[3][col] = 1;
          curr_state->m1_param_present[4][col] = 1;

          if (m1_param_imag_present) {
            m1_param->m1_param_imag[3][col][ps][pb] = m1_param->m1_param_imag[0][col][ps][pb];
            m1_param->m1_param_imag[4][col][ps][pb] = m1_param->m1_param_imag[1][col][ps][pb];
          }

          if (p_aux_struct->ttt_config[i][0].use_ttt_decorr) {
            m1_param->m1_param_real[5][col][ps][pb] = m1_param->m1_param_real[2][col][ps][pb];
            if (m1_param_imag_present)
              m1_param->m1_param_imag[5][col][ps][pb] = m1_param->m1_param_imag[2][col][ps][pb];

            curr_state->m1_param_present[5][col] = 1;
          } else {
            m1_param->m1_param_real[5][col][ps][pb] = 0;
            if (m1_param_imag_present) m1_param->m1_param_imag[5][col][ps][pb] = 0;
          }

          m1_param->m1_param_real[6][col][ps][pb] =
              ixheaacd_mps_mult32_shr_15(c_f_l[pb], m1_param->m1_param_real[0][col][ps][pb]);
          m1_param->m1_param_real[7][col][ps][pb] =
              ixheaacd_mps_mult32_shr_15(c_f_r[pb], m1_param->m1_param_real[1][col][ps][pb]);

          if (m1_param_imag_present) {
            m1_param->m1_param_imag[6][col][ps][pb] =
                ixheaacd_mps_mult32_shr_15(c_f_l[pb], m1_param->m1_param_imag[0][col][ps][pb]);
            m1_param->m1_param_imag[7][col][ps][pb] =
                ixheaacd_mps_mult32_shr_15(c_f_r[pb], m1_param->m1_param_imag[1][col][ps][pb]);
          }

          curr_state->m1_param_present[6][col] = 1;
          curr_state->m1_param_present[7][col] = 1;
        }
      }
    }

    for (pb = 0; pb < p_aux_struct->num_ott_bands[0]; pb++) {
      ia_mps_dec_spatial_bs_frame_struct *p_cur_bs = pstr_mps_state->bs_frame;
      c_l_clfe[pb] = pstr_mps_state->ia_mps_dec_mps_table.m1_m2_table_ptr
                         ->r1_matrix_l[p_cur_bs->ott_cld_idx[0][ps][pb] + 15];
      c_r_clfe[pb] = pstr_mps_state->ia_mps_dec_mps_table.m1_m2_table_ptr
                         ->r1_matrix_l[15 - p_cur_bs->ott_cld_idx[0][ps][pb]];
    }
    for (pb = p_aux_struct->num_ott_bands[0]; pb < num_parameter_bands; pb++) {
      c_l_clfe[pb] = ONE_IN_Q15;
      c_r_clfe[pb] = 0;
    }

    for (pb = 0; pb < num_parameter_bands; pb++) {
      idx = 0;

      m2_param->m2_decor_real[idx++][ps][pb] = ixheaacd_mps_mult32_shr_15(h11_lc[pb], h12_l[pb]);
      if (enable_additionals) {
        pos[0] = idx++;
      }
      m2_param->m2_decor_real[idx++][ps][pb] = h12_lc[pb];
      m2_param->m2_decor_real[idx++][ps][pb] = ixheaacd_mps_mult32_shr_15(h21_lc[pb], h12_l[pb]);
      if (enable_additionals) {
        pos[1] = idx++;
      }
      m2_param->m2_decor_real[idx++][ps][pb] = h22_lc[pb];
      m2_param->m2_decor_real[idx++][ps][pb] = h22_l[pb];
      m2_param->m2_decor_real[idx++][ps][pb] = ixheaacd_mps_mult32_shr_15(h11_rc[pb], h12_r[pb]);
      if (enable_additionals) {
        pos[2] = idx++;
      }
      m2_param->m2_decor_real[idx++][ps][pb] = h12_rc[pb];
      m2_param->m2_decor_real[idx++][ps][pb] = ixheaacd_mps_mult32_shr_15(h21_rc[pb], h12_r[pb]);
      if (enable_additionals) {
        pos[3] = idx++;
      }
      m2_param->m2_decor_real[idx++][ps][pb] = h22_rc[pb];
      m2_param->m2_decor_real[idx++][ps][pb] = h22_r[pb];
      if (enable_additionals) {
        pos[4] = idx++;
      }
    }

    for (pb = 0; pb < num_parameter_bands; pb++) {
      idx = 0;

      m2_param->m2_resid_real[idx][ps][pb] = ixheaacd_mps_mult32_shr_15(h11_lc[pb], h11_l[pb]);
      pos_resid[0] = idx++;

      if (residual_coding) {
        m2_param->m2_resid_real[idx++][ps][pb] =
            ixheaacd_mps_mult32_shr_15(h11_lc[pb], h12_res_l[pb]);
        m2_param->m2_resid_real[idx++][ps][pb] = h12_res_lc[pb];
      }

      m2_param->m2_resid_real[idx][ps][pb] = ixheaacd_mps_mult32_shr_15(h21_lc[pb], h11_l[pb]);
      pos_resid[1] = idx++;

      if (residual_coding) {
        m2_param->m2_resid_real[idx++][ps][pb] =
            ixheaacd_mps_mult32_shr_15(h21_lc[pb], h12_res_l[pb]);
        m2_param->m2_resid_real[idx++][ps][pb] = h22_res_lc[pb];
      }

      m2_param->m2_resid_real[idx++][ps][pb] = h21_l[pb];

      if (residual_coding) m2_param->m2_resid_real[idx++][ps][pb] = h22_res_l[pb];

      m2_param->m2_resid_real[idx][ps][pb] = ixheaacd_mps_mult32_shr_15(h11_rc[pb], h11_r[pb]);
      pos_resid[2] = idx++;

      if (residual_coding) {
        m2_param->m2_resid_real[idx++][ps][pb] =
            ixheaacd_mps_mult32_shr_15(h11_rc[pb], h12_res_r[pb]);
        m2_param->m2_resid_real[idx++][ps][pb] = h12_res_rc[pb];
      }

      m2_param->m2_resid_real[idx][ps][pb] = ixheaacd_mps_mult32_shr_15(h21_rc[pb], h11_r[pb]);
      pos_resid[3] = idx++;

      if (residual_coding) {
        m2_param->m2_resid_real[idx++][ps][pb] =
            ixheaacd_mps_mult32_shr_15(h21_rc[pb], h12_res_r[pb]);
        m2_param->m2_resid_real[idx++][ps][pb] = h22_res_rc[pb];
      }

      m2_param->m2_resid_real[idx++][ps][pb] = h21_r[pb];

      if (residual_coding) m2_param->m2_resid_real[idx++][ps][pb] = h22_res_r[pb];

      m2_param->m2_resid_real[idx][ps][pb] = c_l_clfe[pb];
      pos_resid[4] = idx++;

      m2_param->m2_resid_real[idx][ps][pb] = c_r_clfe[pb];
    }

    for (pb = 0; pb < num_parameter_bands; pb++) {
      ia_mps_dec_spatial_bs_frame_struct *p_cur_bs = pstr_mps_state->bs_frame;

      kappa[pb] = p_aux_struct->ttt_icc[0][ps][pb];
      g_dd[pb] = pstr_mps_state->ia_mps_dec_mps_table.m1_m2_table_ptr
                     ->table_kappa[p_cur_bs->ttt_icc_idx[0][ps][pb]];
    }

    for (i = 0; i < 2; i++) {
      for (pb = p_aux_struct->ttt_config[i][0].start_band;
           pb < p_aux_struct->ttt_config[i][0].stop_band; pb++) {
        if (p_aux_struct->ttt_config[i][0].mode == 0 && pb >= pstr_mps_state->res_bands[5]) {
          if (p_aux_struct->ttt_config[i][0].use_ttt_decorr) {
            temp_1 = ixheaacd_mps_mult32_shr_15(g_dd[pb], c_f_l[pb]);
            m2_param->m2_decor_real[pos[0]][ps][pb] =
                ixheaacd_mps_mult32_shr_15(temp_1, h11_lc[pb]);

            temp_1 = ixheaacd_mps_mult32_shr_15(g_dd[pb], c_f_l[pb]);
            m2_param->m2_decor_real[pos[1]][ps][pb] =
                ixheaacd_mps_mult32_shr_15(temp_1, h21_lc[pb]);

            temp_1 = ixheaacd_mps_mult32_shr_15(g_dd[pb], c_f_r[pb]);
            m2_param->m2_decor_real[pos[2]][ps][pb] =
                ixheaacd_mps_mult32_shr_15(temp_1, h11_rc[pb]);

            temp_1 = ixheaacd_mps_mult32_shr_15(g_dd[pb], c_f_r[pb]);
            m2_param->m2_decor_real[pos[3]][ps][pb] =
                ixheaacd_mps_mult32_shr_15(temp_1, h21_rc[pb]);

            temp_1 = ixheaacd_mps_mult32_shr_15(g_dd[pb], c_l_clfe[pb]);
            temp_2 = MINUS_SQRT_2_Q30;
            m2_param->m2_decor_real[pos[4]][ps][pb] = ixheaacd_mps_mult32_shr_30(temp_1, temp_2);

            pstr_mps_state->m2_param_present[0][5] = 1;
            pstr_mps_state->m2_param_present[1][5] = 1;
            pstr_mps_state->m2_param_present[3][5] = 1;
            pstr_mps_state->m2_param_present[4][5] = 1;
            pstr_mps_state->m2_param_present[6][5] = 1;

            m2_param->m2_resid_real[pos_resid[0]][ps][pb] = ixheaacd_mps_mult32_shr_15(
                m2_param->m2_resid_real[pos_resid[0]][ps][pb], kappa[pb]);
            m2_param->m2_resid_real[pos_resid[1]][ps][pb] = ixheaacd_mps_mult32_shr_15(
                m2_param->m2_resid_real[pos_resid[1]][ps][pb], kappa[pb]);
            m2_param->m2_resid_real[pos_resid[2]][ps][pb] = ixheaacd_mps_mult32_shr_15(
                m2_param->m2_resid_real[pos_resid[2]][ps][pb], kappa[pb]);
            m2_param->m2_resid_real[pos_resid[3]][ps][pb] = ixheaacd_mps_mult32_shr_15(
                m2_param->m2_resid_real[pos_resid[3]][ps][pb], kappa[pb]);
            m2_param->m2_resid_real[pos_resid[4]][ps][pb] = ixheaacd_mps_mult32_shr_15(
                m2_param->m2_resid_real[pos_resid[4]][ps][pb], kappa[pb]);
          }
        }
      }
    }
  }
  return;
}

VOID ixheaacd_calc_m1m2_7272(ia_heaac_mps_state_struct *pstr_mps_state) {
  ia_mps_dec_auxilary_struct *p_aux_struct = pstr_mps_state->aux_struct;
  ia_mps_dec_m2_param_struct *m2_param = p_aux_struct->m2_param;
  ia_mps_dec_m1_param_struct *m1_param = pstr_mps_state->array_struct->m1_param;
  ia_heaac_mps_state_struct *curr_state = pstr_mps_state;
  WORD32 ps, pb, col, row, i;

  WORD32 m_pre[3][5];
  WORD32 m_ttt[3][3];

  WORD32 g_real[2];

  WORD32 temp_1, temp_2;
  WORD64 acc;

  WORD32 *h11_l, *h11_r, *h12_l, *h12_r, *h21_l, *h21_r, *h22_l, *h22_r, *h12_res_l, *h12_res_r,
      *h22_res_l, *h22_res_r;
  WORD32 *c_l_clfe, *c_r_clfe, *kappa, *g_dd;
  WORD16 *c_1_L, *c_1_R, *c_2_L, *c_2_R;
  WORD32 *h11_ls, *h11_rs, *h12_ls, *h12_rs, *h21_ls, *h21_rs, *h22_ls, *h22_rs, *h12_res_ls,
      *h12_res_rs, *h22_res_ls, *h22_res_rs;
  WORD16 *c_f_ls, *c_f_rs, *dummy;

  WORD32 idx;
  WORD32 mode_0 = p_aux_struct->ttt_config[0][0].mode;
  WORD32 mode_1 = p_aux_struct->ttt_config[1][0].mode;
  WORD32 num_parameter_bands = pstr_mps_state->num_parameter_bands;
  WORD32 m1_param_imag_present = pstr_mps_state->m1_param_imag_present;
  WORD32 num_parameter_sets = pstr_mps_state->num_parameter_sets;
  WORD32 arbitrary_downmix = pstr_mps_state->arbitrary_downmix;

  WORD32 enable_additionals = ((mode_0 == 0) || (mode_1 == 0));
  WORD32 pos[3] = {0};
  WORD32 pos_resid[3] = {0};

  WORD32 residual_coding = pstr_mps_state->residual_coding;

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
  c_l_clfe = h22_res_r +
             IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*c_l_clfe), BYTE_ALIGN_8);
  c_r_clfe = c_l_clfe +
             IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*c_r_clfe), BYTE_ALIGN_8);
  kappa =
      c_r_clfe + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*kappa), BYTE_ALIGN_8);
  g_dd = kappa + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*g_dd), BYTE_ALIGN_8);

  h11_ls =
      g_dd + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h11_ls), BYTE_ALIGN_8);
  h11_rs =
      h11_ls + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h11_rs), BYTE_ALIGN_8);
  h12_ls =
      h11_rs + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h12_ls), BYTE_ALIGN_8);
  h12_rs =
      h12_ls + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h12_rs), BYTE_ALIGN_8);
  h21_ls =
      h12_rs + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h21_ls), BYTE_ALIGN_8);
  h21_rs =
      h21_ls + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h21_rs), BYTE_ALIGN_8);
  h22_ls =
      h21_rs + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h22_ls), BYTE_ALIGN_8);
  h22_rs =
      h22_ls + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h22_rs), BYTE_ALIGN_8);
  h12_res_ls = h22_rs + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*h12_res_ls),
                                                      BYTE_ALIGN_8);
  h12_res_rs = h12_res_ls + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS,
                                                          sizeof(*h12_res_rs), BYTE_ALIGN_8);
  h22_res_ls = h12_res_rs + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS,
                                                          sizeof(*h22_res_ls), BYTE_ALIGN_8);
  h22_res_rs = h22_res_ls + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS,
                                                          sizeof(*h22_res_rs), BYTE_ALIGN_8);

  c_1_L = (WORD16 *)pstr_mps_state->mps_scratch_mem_v +
          IXHEAAC_GET_SIZE_ALIGNED_TYPE(PARAMETER_BANDSX56, sizeof(*c_1_L), BYTE_ALIGN_8);
  c_1_R =
      c_1_L + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*c_1_R), BYTE_ALIGN_8);
  c_2_L =
      c_1_R + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*c_2_L), BYTE_ALIGN_8);
  c_2_R =
      c_2_L + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*c_2_R), BYTE_ALIGN_8);
  c_f_ls =
      c_2_R + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*c_f_ls), BYTE_ALIGN_8);
  c_f_rs =
      c_f_ls + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*c_f_rs), BYTE_ALIGN_8);
  dummy =
      c_f_rs + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*dummy), BYTE_ALIGN_8);

  if (enable_additionals) {
    if (mode_1 == 0 &&
        (p_aux_struct->ttt_config[1][0].start_band >= p_aux_struct->ttt_config[1][0].stop_band))
      enable_additionals = 0;
    else if (mode_0 == 0 && (p_aux_struct->ttt_config[0][0].start_band >=
                             p_aux_struct->ttt_config[0][0].stop_band))
      enable_additionals = 0;
  }

  for (ps = 0; ps < num_parameter_sets; ps++) {
    ixheaacd_param_2_umx_ps(pstr_mps_state, h11_l, h12_l, h21_l, h22_l, h12_res_l, h22_res_l,
                            c_1_L, c_2_L, 1, ps, pstr_mps_state->res_bands[1]);
    ixheaacd_param_2_umx_ps(pstr_mps_state, h11_r, h12_r, h21_r, h22_r, h12_res_r, h22_res_r,
                            c_1_R, c_2_R, 2, ps, pstr_mps_state->res_bands[2]);
    ixheaacd_param_2_umx_ps(pstr_mps_state, h11_ls, h12_ls, h21_ls, h22_ls, h12_res_ls,
                            h22_res_ls, dummy, c_f_ls, 3, ps, pstr_mps_state->res_bands[3]);
    ixheaacd_param_2_umx_ps(pstr_mps_state, h11_rs, h12_rs, h21_rs, h22_rs, h12_res_rs,
                            h22_res_rs, dummy, c_f_rs, 4, ps, pstr_mps_state->res_bands[4]);

    for (i = 0; i < 2; i++) {
      for (pb = p_aux_struct->ttt_config[i][0].start_band;
           pb < p_aux_struct->ttt_config[i][0].stop_band; pb++) {
        WORD32 mtx_inversion = pstr_mps_state->mtx_inversion;

        memset(m_pre, 0, sizeof(m_pre));

        if (p_aux_struct->ttt_config[i][0].mode >= 2) {
          mtx_inversion = mtx_inversion && (p_aux_struct->ttt_config[i][0].mode == 2 ||
                                            p_aux_struct->ttt_config[i][0].mode == 4);
        }

        ixheaacd_calculate_ttt(pstr_mps_state, ps, pb, p_aux_struct->ttt_config[i][0].mode,
                               m_ttt);

        for (row = 0; row < 3; row++) {
          for (col = 0; col < 3; col++) {
            m_pre[row][col] = m_ttt[row][col];
          }
        }

        if (arbitrary_downmix != 0) {
          WORD32 ch;
          ixheaacd_calculate_arb_dmx_mtx(pstr_mps_state, ps, pb, g_real);

          for (ch = 0; ch < pstr_mps_state->num_input_channels; ch++) {
            for (row = 0; row < 3; row++) {
              m_pre[row][col] = ixheaacd_mps_mult32_shr_15(m_pre[row][col], g_real[ch]);

              if (arbitrary_downmix == 2 && pb < pstr_mps_state->arbdmx_residual_bands) {
                m_pre[row][3 + ch] = m_ttt[row][ch];
              }
            }
          }
        }

        if (mtx_inversion) {
          WORD32 h_real[2][2], h_imag[2][2];

          ixheaacd_calculate_mtx_inv(pstr_mps_state, ps, pb, p_aux_struct->ttt_config[i][0].mode,
                                     h_real, h_imag);

          acc = (WORD64)((WORD64)m_pre[0][0] * (WORD64)h_real[0][0] +
                         (WORD64)m_pre[0][1] * (WORD64)h_real[1][0]);
          acc >>= 15;
          m1_param->m1_param_real[0][0][ps][pb] = (WORD32)acc;

          acc = (WORD64)((WORD64)m_pre[0][0] * (WORD64)h_real[0][1] +
                         (WORD64)m_pre[0][1] * (WORD64)h_real[1][1]);
          acc >>= 15;
          m1_param->m1_param_real[0][1][ps][pb] = (WORD32)acc;

          acc = (WORD64)((WORD64)m_pre[1][0] * (WORD64)h_real[0][0] +
                         (WORD64)m_pre[1][1] * (WORD64)h_real[1][0]);
          acc >>= 15;
          m1_param->m1_param_real[1][0][ps][pb] = (WORD32)acc;

          acc = (WORD64)((WORD64)m_pre[1][0] * (WORD64)h_real[0][1] +
                         (WORD64)m_pre[1][1] * (WORD64)h_real[1][1]);
          acc >>= 15;
          m1_param->m1_param_real[1][1][ps][pb] = (WORD32)acc;

          acc = (WORD64)((WORD64)m_pre[2][0] * (WORD64)h_real[0][0] +
                         (WORD64)m_pre[2][1] * (WORD64)h_real[1][0]);
          acc >>= 15;
          m1_param->m1_param_real[2][0][ps][pb] = (WORD32)acc;

          acc = (WORD64)((WORD64)m_pre[2][0] * (WORD64)h_real[0][1] +
                         (WORD64)m_pre[2][1] * (WORD64)h_real[1][1]);
          acc >>= 15;
          m1_param->m1_param_real[2][1][ps][pb] = (WORD32)acc;

          acc = (WORD64)((WORD64)m_pre[0][0] * (WORD64)h_imag[0][0] +
                         (WORD64)m_pre[0][1] * (WORD64)h_imag[1][0]);
          acc >>= 15;
          m1_param->m1_param_imag[0][0][ps][pb] = (WORD32)acc;

          acc = (WORD64)((WORD64)m_pre[0][0] * (WORD64)h_imag[0][1] +
                         (WORD64)m_pre[0][1] * (WORD64)h_imag[1][1]);
          acc >>= 15;
          m1_param->m1_param_imag[0][1][ps][pb] = (WORD32)acc;

          acc = (WORD64)((WORD64)m_pre[1][0] * (WORD64)h_imag[0][0] +
                         (WORD64)m_pre[1][1] * (WORD64)h_imag[1][0]);
          acc >>= 15;
          m1_param->m1_param_imag[1][0][ps][pb] = (WORD32)acc;

          acc = (WORD64)((WORD64)m_pre[1][0] * (WORD64)h_imag[0][1] +
                         (WORD64)m_pre[1][1] * (WORD64)h_imag[1][1]);
          acc >>= 15;
          m1_param->m1_param_imag[1][1][ps][pb] = (WORD32)acc;

          acc = (WORD64)((WORD64)m_pre[2][0] * (WORD64)h_imag[0][0] +
                         (WORD64)m_pre[2][1] * (WORD64)h_imag[1][0]);
          acc >>= 15;
          m1_param->m1_param_imag[2][0][ps][pb] = (WORD32)acc;

          acc = (WORD64)((WORD64)m_pre[2][0] * (WORD64)h_imag[0][1] +
                         (WORD64)m_pre[2][1] * (WORD64)h_imag[1][1]);
          acc >>= 15;
          m1_param->m1_param_imag[2][1][ps][pb] = (WORD32)acc;
        } else if (pstr_mps_state->_3d_stereo_inversion) {
        } else {
          m1_param->m1_param_real[0][0][ps][pb] = m_pre[0][0];
          m1_param->m1_param_real[0][1][ps][pb] = m_pre[0][1];
          m1_param->m1_param_real[1][0][ps][pb] = m_pre[1][0];
          m1_param->m1_param_real[1][1][ps][pb] = m_pre[1][1];
          m1_param->m1_param_real[2][0][ps][pb] = m_pre[2][0];
          m1_param->m1_param_real[2][1][ps][pb] = m_pre[2][1];
        }

        m1_param->m1_param_real[0][2][ps][pb] = m_pre[0][2];
        m1_param->m1_param_real[0][3][ps][pb] = m_pre[0][3];
        m1_param->m1_param_real[0][4][ps][pb] = m_pre[0][4];
        m1_param->m1_param_real[1][2][ps][pb] = m_pre[1][2];
        m1_param->m1_param_real[1][3][ps][pb] = m_pre[1][3];
        m1_param->m1_param_real[1][4][ps][pb] = m_pre[1][4];
        m1_param->m1_param_real[2][2][ps][pb] = m_pre[2][2];
        m1_param->m1_param_real[2][3][ps][pb] = m_pre[2][3];
        m1_param->m1_param_real[2][4][ps][pb] = m_pre[2][4];

        for (col = 0; col < pstr_mps_state->num_x_channels; col++) {
          m1_param->m1_param_real[3][col][ps][pb] = m1_param->m1_param_real[0][col][ps][pb];
          m1_param->m1_param_real[4][col][ps][pb] = m1_param->m1_param_real[1][col][ps][pb];

          if (m1_param_imag_present) {
            m1_param->m1_param_imag[3][col][ps][pb] = m1_param->m1_param_imag[0][col][ps][pb];
            m1_param->m1_param_imag[4][col][ps][pb] = m1_param->m1_param_imag[1][col][ps][pb];
          }

          curr_state->m1_param_present[3][col] = 1;
          curr_state->m1_param_present[4][col] = 1;

          if (p_aux_struct->ttt_config[i][0].use_ttt_decorr) {
            m1_param->m1_param_real[5][col][ps][pb] = m1_param->m1_param_real[2][col][ps][pb];
            if (m1_param_imag_present)
              m1_param->m1_param_imag[5][col][ps][pb] = m1_param->m1_param_imag[2][col][ps][pb];

            curr_state->m1_param_present[5][col] = 1;
          } else {
            m1_param->m1_param_real[5][col][ps][pb] = 0;
            if (m1_param_imag_present) m1_param->m1_param_imag[5][col][ps][pb] = 0;
          }

          m1_param->m1_param_real[6][col][ps][pb] =
              ixheaacd_mps_mult32_shr_15(m1_param->m1_param_real[0][col][ps][pb], c_2_L[pb]);
          m1_param->m1_param_real[7][col][ps][pb] =
              ixheaacd_mps_mult32_shr_15(m1_param->m1_param_real[1][col][ps][pb], c_2_R[pb]);

          if (m1_param_imag_present) {
            m1_param->m1_param_imag[6][col][ps][pb] =
                ixheaacd_mps_mult32_shr_15(m1_param->m1_param_imag[0][col][ps][pb], c_2_L[pb]);
            m1_param->m1_param_imag[7][col][ps][pb] =
                ixheaacd_mps_mult32_shr_15(m1_param->m1_param_imag[1][col][ps][pb], c_2_R[pb]);
          }

          curr_state->m1_param_present[6][col] = 1;
          curr_state->m1_param_present[7][col] = 1;
        }
      }
    }

    for (pb = 0; pb < p_aux_struct->num_ott_bands[0]; pb++) {
      ia_mps_dec_spatial_bs_frame_struct *p_cur_bs = pstr_mps_state->bs_frame;
      c_l_clfe[pb] = pstr_mps_state->ia_mps_dec_mps_table.m1_m2_table_ptr
                         ->r1_matrix_l[p_cur_bs->ott_cld_idx[0][ps][pb] + 15];
      c_r_clfe[pb] = pstr_mps_state->ia_mps_dec_mps_table.m1_m2_table_ptr
                         ->r1_matrix_l[15 - p_cur_bs->ott_cld_idx[0][ps][pb]];
    }
    for (pb = p_aux_struct->num_ott_bands[0]; pb < num_parameter_bands; pb++) {
      c_l_clfe[pb] = ONE_IN_Q15;
      c_r_clfe[pb] = 0;
    }

    for (pb = 0; pb < num_parameter_bands; pb++) {
      idx = 0;

      m2_param->m2_decor_real[idx++][ps][pb] = h12_l[pb];
      if (enable_additionals) {
        pos[0] = idx++;
      }
      m2_param->m2_decor_real[idx++][ps][pb] = ixheaacd_mps_mult32_shr_15(h11_ls[pb], h22_l[pb]);
      m2_param->m2_decor_real[idx++][ps][pb] = h12_ls[pb];
      m2_param->m2_decor_real[idx++][ps][pb] = ixheaacd_mps_mult32_shr_15(h21_ls[pb], h22_l[pb]);
      m2_param->m2_decor_real[idx++][ps][pb] = h22_ls[pb];
      m2_param->m2_decor_real[idx++][ps][pb] = h12_r[pb];
      if (enable_additionals) {
        pos[1] = idx++;
      }
      m2_param->m2_decor_real[idx++][ps][pb] = ixheaacd_mps_mult32_shr_15(h11_rs[pb], h22_r[pb]);
      m2_param->m2_decor_real[idx++][ps][pb] = h12_rs[pb];
      m2_param->m2_decor_real[idx++][ps][pb] = ixheaacd_mps_mult32_shr_15(h21_rs[pb], h22_r[pb]);
      m2_param->m2_decor_real[idx++][ps][pb] = h22_rs[pb];
      if (enable_additionals) {
        pos[2] = idx++;
      }
    }

    for (pb = 0; pb < num_parameter_bands; pb++) {
      idx = 0;
      m2_param->m2_resid_real[idx][ps][pb] = h11_l[pb];
      pos_resid[0] = idx++;

      if (residual_coding) m2_param->m2_resid_real[idx++][ps][pb] = h12_res_l[pb];

      m2_param->m2_resid_real[idx++][ps][pb] = ixheaacd_mps_mult32_shr_15(h11_ls[pb], h21_l[pb]);

      if (residual_coding) {
        m2_param->m2_resid_real[idx++][ps][pb] =
            ixheaacd_mps_mult32_shr_15(h11_ls[pb], h22_res_l[pb]);
        m2_param->m2_resid_real[idx++][ps][pb] = h12_res_ls[pb];
      }

      m2_param->m2_resid_real[idx++][ps][pb] = ixheaacd_mps_mult32_shr_15(h21_ls[pb], h21_l[pb]);

      if (residual_coding) {
        m2_param->m2_resid_real[idx++][ps][pb] =
            ixheaacd_mps_mult32_shr_15(h21_ls[pb], h22_res_l[pb]);
        m2_param->m2_resid_real[idx++][ps][pb] = h22_res_ls[pb];
      }

      m2_param->m2_resid_real[idx][ps][pb] = h11_r[pb];
      pos_resid[1] = idx++;

      if (residual_coding) m2_param->m2_resid_real[idx++][ps][pb] = h12_res_r[pb];

      m2_param->m2_resid_real[idx++][ps][pb] = ixheaacd_mps_mult32_shr_15(h11_rs[pb], h21_r[pb]);

      if (residual_coding) {
        m2_param->m2_resid_real[idx++][ps][pb] =
            ixheaacd_mps_mult32_shr_15(h11_rs[pb], h22_res_r[pb]);
        m2_param->m2_resid_real[idx++][ps][pb] = h12_res_rs[pb];
      }

      m2_param->m2_resid_real[idx++][ps][pb] = ixheaacd_mps_mult32_shr_15(h21_rs[pb], h21_r[pb]);

      if (residual_coding) {
        m2_param->m2_resid_real[idx++][ps][pb] =
            ixheaacd_mps_mult32_shr_15(h21_rs[pb], h22_res_r[pb]);
        m2_param->m2_resid_real[idx++][ps][pb] = h22_res_rs[pb];
      }

      m2_param->m2_resid_real[idx][ps][pb] = c_l_clfe[pb];
      pos_resid[2] = idx++;

      m2_param->m2_resid_real[idx++][ps][pb] = c_r_clfe[pb];
    }

    for (pb = 0; pb < num_parameter_bands; pb++) {
      ia_mps_dec_spatial_bs_frame_struct *p_cur_bs = pstr_mps_state->bs_frame;

      kappa[pb] = p_aux_struct->ttt_icc[0][ps][pb];
      g_dd[pb] = pstr_mps_state->ia_mps_dec_mps_table.m1_m2_table_ptr
                     ->table_kappa[p_cur_bs->ttt_icc_idx[0][ps][pb]];
    }

    for (i = 0; i < 2; i++) {
      for (pb = p_aux_struct->ttt_config[i][0].start_band;
           pb < p_aux_struct->ttt_config[i][0].stop_band; pb++) {
        if (p_aux_struct->ttt_config[i][0].mode == 0 && pb >= pstr_mps_state->res_bands[5]) {
          if (p_aux_struct->ttt_config[i][0].use_ttt_decorr) {
            m2_param->m2_decor_real[pos[0]][ps][pb] =
                ixheaacd_mps_mult32_shr_15(g_dd[pb], c_1_L[pb]);

            m2_param->m2_decor_real[pos[1]][ps][pb] =
                ixheaacd_mps_mult32_shr_15(g_dd[pb], c_1_R[pb]);

            temp_1 = ixheaacd_mps_mult32_shr_15(g_dd[pb], c_l_clfe[pb]);
            temp_2 = MINUS_SQRT_2_Q30;
            m2_param->m2_decor_real[pos[2]][ps][pb] = ixheaacd_mps_mult32_shr_30(temp_1, temp_2);

            pstr_mps_state->m2_param_present[0][5] = 1;
            pstr_mps_state->m2_param_present[3][5] = 1;
            pstr_mps_state->m2_param_present[6][5] = 1;

            m2_param->m2_resid_real[pos_resid[0]][ps][pb] = ixheaacd_mps_mult32_shr_15(
                m2_param->m2_resid_real[pos_resid[0]][ps][pb], kappa[pb]);
            m2_param->m2_resid_real[pos_resid[1]][ps][pb] = ixheaacd_mps_mult32_shr_15(
                m2_param->m2_resid_real[pos_resid[1]][ps][pb], kappa[pb]);
            m2_param->m2_resid_real[pos_resid[2]][ps][pb] = ixheaacd_mps_mult32_shr_15(
                m2_param->m2_resid_real[pos_resid[2]][ps][pb], kappa[pb]);
          }
        }
      }
    }
  }
  return;
}
