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

VOID ixheaacd_calc_m1m2_5227(ia_heaac_mps_state_struct *pstr_mps_state) {
  ia_mps_dec_auxilary_struct *p_aux_struct = pstr_mps_state->aux_struct;
  ia_mps_dec_m1_param_struct *m1_param = pstr_mps_state->array_struct->m1_param;
  WORD32 ps, pb, i;
  WORD32 row, col, ch;
  WORD32 *a_prediction_mode;

  WORD32 *m_real;
  WORD32 *m_imag;

  WORD32 *lf;
  WORD32 *ls;
  WORD32 *rf;
  WORD32 *rs;
  WORD32 *a_c1;
  WORD32 *a_c2;
  WORD32 *a_icc_c;

  WORD64 acc;
  WORD32 num_parameter_sets = pstr_mps_state->num_parameter_sets;
  WORD32 num_parameter_bands = pstr_mps_state->num_parameter_bands;
  WORD32 arbitrary_downmix = pstr_mps_state->arbitrary_downmix;
  WORD32 arbdmx_residual_bands = pstr_mps_state->arbdmx_residual_bands;
  WORD32 in_ch = pstr_mps_state->num_input_channels;

  const WORD32 *cld_tab_1 = pstr_mps_state->ia_mps_dec_mps_table.m1_m2_table_ptr->cld_tab_1;

  lf = pstr_mps_state->mps_scratch_mem_v;
  ls = lf + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*ls), BYTE_ALIGN_8);
  rf = ls + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*rf), BYTE_ALIGN_8);
  rs = rf + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*rs), BYTE_ALIGN_8);
  a_c1 = rs + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*a_c1), BYTE_ALIGN_8);
  a_c2 = a_c1 + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*a_c2), BYTE_ALIGN_8);
  a_icc_c =
      a_c2 + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*a_icc_c), BYTE_ALIGN_8);
  a_prediction_mode =
      a_icc_c + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*a_prediction_mode),
                                              BYTE_ALIGN_8);
  m_real = a_prediction_mode +
           IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*m_real), BYTE_ALIGN_8);
  m_imag =
      m_real + IXHEAAC_GET_SIZE_ALIGNED_TYPE(PARAMETER_BANDSX15, sizeof(*m_imag), BYTE_ALIGN_8);

  for (i = 0; i < PARAMETER_BANDSX15; i++) {
    m_real[i] = 0;
    m_imag[i] = 0;
  }

  for (ps = 0; ps < num_parameter_sets; ps++) {
    for (i = 0; i < 2; i++) {
      for (pb = p_aux_struct->ttt_config[i][0].start_band;
           pb < p_aux_struct->ttt_config[i][0].stop_band; pb++) {
        WORD32 m_pre[3][5];
        WORD32 m_ttt[3][3];
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
          WORD32 g_real[2];
          ixheaacd_calculate_arb_dmx_mtx(pstr_mps_state, ps, pb, g_real);

          if (arbitrary_downmix == 2 && pb < arbdmx_residual_bands) {
            for (ch = 0; ch < in_ch; ch++) {
              for (row = 0; row < 3; row++) {
                m_pre[row][ch] = ixheaacd_mps_mult32_shr_15(m_pre[row][ch], g_real[ch]);

                m_pre[row][3 + ch] = m_ttt[row][ch];
              }
            }
          } else {
            for (ch = 0; ch < in_ch; ch++) {
              for (row = 0; row < 3; row++) {
                m_pre[row][ch] = ixheaacd_mps_mult32_shr_15(m_pre[row][ch], g_real[ch]);
              }
            }
          }
        }

        if (mtx_inversion) {
          WORD32 h_real[2][2];
          WORD32 h_imag[2][2];

          ixheaacd_calculate_mtx_inv(pstr_mps_state, ps, pb, p_aux_struct->ttt_config[i][0].mode,
                                     h_real, h_imag);

          acc = (WORD64)((WORD64)m_pre[0][0] * (WORD64)h_real[0][0] +
                         (WORD64)m_pre[0][1] * (WORD64)h_real[1][0]);
          acc >>= 15;
          m_real[0] = (WORD32)acc;

          acc = (WORD64)((WORD64)m_pre[0][0] * (WORD64)h_real[0][1] +
                         (WORD64)m_pre[0][1] * (WORD64)h_real[1][1]);
          acc >>= 15;
          m_real[1] = (WORD32)acc;

          acc = (WORD64)((WORD64)m_pre[1][0] * (WORD64)h_real[0][0] +
                         (WORD64)m_pre[1][1] * (WORD64)h_real[1][0]);
          acc >>= 15;
          m_real[5] = (WORD32)acc;

          acc = (WORD64)((WORD64)m_pre[1][0] * (WORD64)h_real[0][1] +
                         (WORD64)m_pre[1][1] * (WORD64)h_real[1][1]);
          acc >>= 15;
          m_real[6] = (WORD32)acc;

          acc = (WORD64)((WORD64)m_pre[2][0] * (WORD64)h_real[0][0] +
                         (WORD64)m_pre[2][1] * (WORD64)h_real[1][0]);
          acc >>= 15;
          m_real[10] = (WORD32)acc;

          acc = (WORD64)((WORD64)m_pre[2][0] * (WORD64)h_real[0][1] +
                         (WORD64)m_pre[2][1] * (WORD64)h_real[1][1]);
          acc >>= 15;
          m_real[11] = (WORD32)acc;

          acc = (WORD64)((WORD64)m_pre[0][0] * (WORD64)h_imag[0][0] +
                         (WORD64)m_pre[0][1] * (WORD64)h_imag[1][0]);
          acc >>= 15;
          m_imag[0] = (WORD32)acc;

          acc = (WORD64)((WORD64)m_pre[0][0] * (WORD64)h_imag[0][1] +
                         (WORD64)m_pre[0][1] * (WORD64)h_imag[1][1]);
          acc >>= 15;
          m_imag[1] = (WORD32)acc;

          acc = (WORD64)((WORD64)m_pre[1][0] * (WORD64)h_imag[0][0] +
                         (WORD64)m_pre[1][1] * (WORD64)h_imag[1][0]);
          acc >>= 15;
          m_imag[5] = (WORD32)acc;

          acc = (WORD64)((WORD64)m_pre[1][0] * (WORD64)h_imag[0][1] +
                         (WORD64)m_pre[1][1] * (WORD64)h_imag[1][1]);
          acc >>= 15;
          m_imag[6] = (WORD32)acc;

          acc = (WORD64)((WORD64)m_pre[2][0] * (WORD64)h_imag[0][0] +
                         (WORD64)m_pre[2][1] * (WORD64)h_imag[1][0]);
          acc >>= 15;
          m_imag[10] = (WORD32)acc;

          acc = (WORD64)((WORD64)m_pre[2][0] * (WORD64)h_imag[0][1] +
                         (WORD64)m_pre[2][1] * (WORD64)h_imag[1][1]);
          acc >>= 15;
          m_imag[11] = (WORD32)acc;
        } else if (pstr_mps_state->_3d_stereo_inversion) {
        } else {
          m_real[0] = m_pre[0][0];
          m_real[1] = m_pre[0][1];
          m_real[5] = m_pre[1][0];
          m_real[6] = m_pre[1][1];
          m_real[10] = m_pre[2][0];
          m_real[11] = m_pre[2][1];

          m_imag[0] = 0;
          m_imag[1] = 0;
          m_imag[5] = 0;
          m_imag[6] = 0;
          m_imag[10] = 0;
          m_imag[11] = 0;
        }

        m_real[2] = m_pre[0][3];
        m_real[3] = m_pre[0][4];
        m_real[4] = m_pre[0][2];
        m_real[7] = m_pre[1][3];
        m_real[8] = m_pre[1][4];
        m_real[9] = m_pre[1][2];
        m_real[12] = m_pre[2][3];
        m_real[13] = m_pre[2][4];
        m_real[14] = m_pre[2][2];
      }
    }

    for (i = 0; i < 2; i++) {
      for (pb = p_aux_struct->ttt_config[i][0].start_band;
           pb < p_aux_struct->ttt_config[i][0].stop_band; pb++) {
        ia_mps_dec_spatial_bs_frame_struct *p_cur_bs = pstr_mps_state->bs_frame;
        if (p_aux_struct->ttt_config[i][0].mode < 2) {
          a_prediction_mode[pb] = 1;
          a_c1[pb] = p_aux_struct->ttt_cpc_1[0][ps][pb];
          a_c2[pb] = p_aux_struct->ttt_cpc_2[0][ps][pb];
          a_icc_c[pb] = p_aux_struct->ttt_icc[0][ps][pb];
        } else {
          a_prediction_mode[pb] = 0;
          a_c1[pb] = p_aux_struct->ttt_cld_1[0][ps][pb];
          a_c2[pb] = p_aux_struct->ttt_cld_2[0][ps][pb];
          a_icc_c[pb] = 0;
        }
        lf[pb] = cld_tab_1[p_cur_bs->ott_cld_idx[1][ps][pb] + 15];
        ls[pb] = cld_tab_1[15 - p_cur_bs->ott_cld_idx[1][ps][pb]];

        rf[pb] = cld_tab_1[p_cur_bs->ott_cld_idx[1][ps][pb] + 15];
        rs[pb] = cld_tab_1[15 - p_cur_bs->ott_cld_idx[2][ps][pb]];
      }
    }

    for (pb = 0; pb < num_parameter_bands; pb++) {
      m1_param->m1_param_real[0][0][ps][pb] = ONE_IN_Q15;
      m1_param->m1_param_real[0][1][ps][pb] = 0;
      m1_param->m1_param_real[1][0][ps][pb] = 0;
      m1_param->m1_param_real[1][1][ps][pb] = ONE_IN_Q15;

      m1_param->m1_param_real[2][3][ps][pb] = ONE_IN_Q15;
      m1_param->m1_param_real[2][4][ps][pb] = 0;
      m1_param->m1_param_real[3][3][ps][pb] = 0;
      m1_param->m1_param_real[3][4][ps][pb] = ONE_IN_Q15;
    }
  }
}

VOID ixheaacd_calc_m1m2_5251(ia_heaac_mps_state_struct *pstr_mps_state) {
  ia_mps_dec_auxilary_struct *p_aux_struct = pstr_mps_state->aux_struct;
  ia_mps_dec_m2_param_struct *m2_param = p_aux_struct->m2_param;
  ia_mps_dec_m1_param_struct *m1_param = pstr_mps_state->array_struct->m1_param;
  WORD32 ch, ps, pb, col, row, i;

  WORD32 temp_1;
  WORD32 *h11_l, *h11_r, *h12_l, *h12_r, *h21_l, *h21_r, *h22_l, *h22_r, *h12_res_l, *h12_res_r,
      *h22_res_l, *h22_res_r;
  WORD32 *c_l_clfe, *c_r_clfe, *kappa, *g_dd;
  WORD16 *c_f_l, *c_f_r, *dummy1, *dummy2;

  WORD32 idx, index2 = 0, index3 = 0;
  WORD32 mode_0 = p_aux_struct->ttt_config[0][0].mode;
  WORD32 mode_1 = p_aux_struct->ttt_config[1][0].mode;
  WORD32 enable_additionals = ((mode_0 == 0) || (mode_1 == 0));

  WORD32 num_decor_signals = pstr_mps_state->num_decor_signals;
  WORD32 residual_coding = pstr_mps_state->residual_coding;
  WORD32 num_parameter_sets = pstr_mps_state->num_parameter_sets;
  WORD32 num_parameter_bands = pstr_mps_state->num_parameter_bands;
  WORD32 m1_param_imag_present = pstr_mps_state->m1_param_imag_present;
  WORD32 num_x_channels = pstr_mps_state->num_x_channels;
  WORD32 arbitrary_downmix = pstr_mps_state->arbitrary_downmix;
  WORD32 arbdmx_residual_bands = pstr_mps_state->arbdmx_residual_bands;
  WORD32 in_ch = pstr_mps_state->num_input_channels;
  WORD32 res_bands = pstr_mps_state->res_bands[3];

  WORD32 decorr_present;
  WORD32 pos[3];
  WORD64 acc;

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

  c_f_l = (WORD16 *)pstr_mps_state->mps_scratch_mem_v +
          IXHEAAC_GET_SIZE_ALIGNED_TYPE(PARAMETER_BANDSX32, sizeof(*c_f_l), BYTE_ALIGN_8);
  c_f_r =
      c_f_l + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*c_f_r), BYTE_ALIGN_8);
  dummy1 =
      c_f_r + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*dummy1), BYTE_ALIGN_8);
  dummy2 =
      dummy1 + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*dummy2), BYTE_ALIGN_8);

  if (enable_additionals) {
    if (mode_1 == 0 &&
        (p_aux_struct->ttt_config[1][0].start_band >= p_aux_struct->ttt_config[1][0].stop_band))
      enable_additionals = 0;
    else if (mode_0 == 0 && (p_aux_struct->ttt_config[0][0].start_band >=
                             p_aux_struct->ttt_config[0][0].stop_band))
      enable_additionals = 0;
  }

  decorr_present = enable_additionals && num_decor_signals == 3;

  pstr_mps_state->num_decor_signals = 2;

  for (ps = 0; ps < num_parameter_sets; ps++) {
    for (i = 0; i < 2; i++) {
      for (pb = p_aux_struct->ttt_config[i][0].start_band;
           pb < p_aux_struct->ttt_config[i][0].stop_band; pb++) {
        WORD32 m_ttt[3][3];
        WORD32 m_pre[3][5];
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
          WORD32 g_real[2];
          ixheaacd_calculate_arb_dmx_mtx(pstr_mps_state, ps, pb, g_real);

          if (arbitrary_downmix == 2 && pb < arbdmx_residual_bands) {
            for (ch = 0; ch < in_ch; ch++) {
              for (row = 0; row < 3; row++) {
                m_pre[row][ch] = ixheaacd_mps_mult32_shr_15(m_pre[row][ch], g_real[ch]);
                m_pre[row][3 + ch] = m_ttt[row][ch];
              }
            }
          } else {
            for (ch = 0; ch < in_ch; ch++) {
              for (row = 0; row < 3; row++) {
                m_pre[row][ch] = ixheaacd_mps_mult32_shr_15(m_pre[row][ch], g_real[ch]);
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
          m1_param->m1_param_imag[0][0][ps][pb] = 0;
          m1_param->m1_param_imag[0][1][ps][pb] = 0;
          m1_param->m1_param_imag[1][0][ps][pb] = 0;
          m1_param->m1_param_imag[1][1][ps][pb] = 0;
          m1_param->m1_param_imag[2][0][ps][pb] = 0;
          m1_param->m1_param_imag[2][1][ps][pb] = 0;
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

        m1_param->m1_param_imag[0][2][ps][pb] = 0;
        m1_param->m1_param_imag[0][3][ps][pb] = 0;
        m1_param->m1_param_imag[0][4][ps][pb] = 0;
        m1_param->m1_param_imag[1][2][ps][pb] = 0;
        m1_param->m1_param_imag[1][3][ps][pb] = 0;
        m1_param->m1_param_imag[1][4][ps][pb] = 0;
        m1_param->m1_param_imag[2][2][ps][pb] = 0;
        m1_param->m1_param_imag[2][3][ps][pb] = 0;
        m1_param->m1_param_imag[2][4][ps][pb] = 0;

        for (col = 0; col < num_x_channels; col++) {
          m1_param->m1_param_real[3][col][ps][pb] = m1_param->m1_param_real[0][col][ps][pb];
          m1_param->m1_param_real[4][col][ps][pb] = m1_param->m1_param_real[1][col][ps][pb];

          if (m1_param_imag_present) {
            m1_param->m1_param_imag[3][col][ps][pb] = m1_param->m1_param_imag[0][col][ps][pb];
            m1_param->m1_param_imag[4][col][ps][pb] = m1_param->m1_param_imag[1][col][ps][pb];
          }
          pstr_mps_state->m1_param_present[3][col] = 1;
          pstr_mps_state->m1_param_present[4][col] = 1;

          if (p_aux_struct->ttt_config[i][0].use_ttt_decorr) {
            m1_param->m1_param_real[5][col][ps][pb] = m1_param->m1_param_real[2][col][ps][pb];
            if (m1_param_imag_present)
              m1_param->m1_param_imag[5][col][ps][pb] = m1_param->m1_param_imag[2][col][ps][pb];

            pstr_mps_state->m1_param_present[5][col] = 1;
          }
        }
      }
    }
  }

  for (ps = 0; ps < num_parameter_sets; ps++) {
    ixheaacd_param_2_umx_ps(pstr_mps_state, h11_l, h12_l, h21_l, h22_l, h12_res_l, h22_res_l,
                            c_f_l, dummy1, 1, ps, pstr_mps_state->res_bands[1]);

    ixheaacd_param_2_umx_ps(pstr_mps_state, h11_r, h12_r, h21_r, h22_r, h12_res_r, h22_res_r,
                            c_f_r, dummy2, 2, ps, pstr_mps_state->res_bands[2]);

    for (pb = 0; pb < p_aux_struct->num_ott_bands[0]; pb++) {
      WORD32 temp = ixheaacd_quantize((p_aux_struct->ott_cld[0][ps][pb]) >> 15);

      c_l_clfe[pb] = pstr_mps_state->ia_mps_dec_mps_table.m1_m2_table_ptr->r1_matrix_l[temp + 15];
      c_r_clfe[pb] = pstr_mps_state->ia_mps_dec_mps_table.m1_m2_table_ptr->r1_matrix_l[15 - temp];
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
      m2_param->m2_decor_real[idx++][ps][pb] = h22_l[pb];
      m2_param->m2_decor_real[idx++][ps][pb] = h12_r[pb];
      if (enable_additionals) {
        pos[1] = idx++;
      }
      m2_param->m2_decor_real[idx++][ps][pb] = h22_r[pb];
      if (enable_additionals) {
        pos[2] = idx++;
      }
    }

    for (pb = 0; pb < num_parameter_bands; pb++) {
      idx = 1;

      m2_param->m2_resid_real[0][ps][pb] = h11_l[pb];

      if (residual_coding) m2_param->m2_resid_real[idx++][ps][pb] = h12_res_l[pb];

      if (decorr_present) idx++;

      m2_param->m2_resid_real[idx++][ps][pb] = h21_l[pb];

      if (residual_coding) m2_param->m2_resid_real[idx++][ps][pb] = h22_res_l[pb];

      m2_param->m2_resid_real[idx][ps][pb] = h11_r[pb];
      index2 = idx++;

      if (residual_coding) m2_param->m2_resid_real[idx++][ps][pb] = h12_res_r[pb];

      if (decorr_present) idx++;

      m2_param->m2_resid_real[idx++][ps][pb] = h21_r[pb];

      if (residual_coding) m2_param->m2_resid_real[idx++][ps][pb] = h22_res_r[pb];

      m2_param->m2_resid_real[idx][ps][pb] = c_l_clfe[pb];
      index3 = idx++;

      if (decorr_present) idx++;

      m2_param->m2_resid_real[idx++][ps][pb] = c_r_clfe[pb];
    }

    for (pb = 0; pb < num_parameter_bands; pb++) {
      ia_mps_dec_spatial_bs_frame_struct *p_cur_bs = pstr_mps_state->bs_frame;

      kappa[pb] = p_aux_struct->ttt_icc[0][ps][pb];
      g_dd[pb] = pstr_mps_state->ia_mps_dec_mps_table.m1_m2_table_ptr
                     ->table_kappa[p_cur_bs->ttt_icc_idx[0][ps][pb]];
    }

    for (i = 0; i < 2; i++) {
      if (p_aux_struct->ttt_config[i][0].use_ttt_decorr) {
        for (pb = p_aux_struct->ttt_config[i][0].start_band;
             pb < p_aux_struct->ttt_config[i][0].stop_band; pb++) {
          if (p_aux_struct->ttt_config[i][0].mode == 0 && pb >= res_bands) {
            if (p_aux_struct->ttt_config[i][0].use_ttt_decorr && enable_additionals) {
              WORD32 temp;
              pstr_mps_state->num_decor_signals = 3;

              temp_1 = MINUS_SQRT_2_Q30;
              m2_param->m2_decor_real[pos[0]][ps][pb] =
                  ixheaacd_mps_mult32_shr_15(g_dd[pb], c_f_l[pb]);
              m2_param->m2_decor_real[pos[1]][ps][pb] =
                  ixheaacd_mps_mult32_shr_15(g_dd[pb], c_f_r[pb]);

              temp = ixheaacd_mps_mult32_shr_15(g_dd[pb], c_l_clfe[pb]);
              m2_param->m2_decor_real[pos[2]][ps][pb] = ixheaacd_mps_mult32_shr_30(temp, temp_1);

              pstr_mps_state->m2_param_present[0][5] = 3;
              pstr_mps_state->m2_param_present[2][5] = 3;
              pstr_mps_state->m2_param_present[4][5] = 3;

              m2_param->m2_resid_real[0][ps][pb] =
                  ixheaacd_mps_mult32_shr_15(m2_param->m2_resid_real[0][ps][pb], kappa[pb]);
              m2_param->m2_resid_real[index2][ps][pb] =
                  ixheaacd_mps_mult32_shr_15(m2_param->m2_resid_real[index2][ps][pb], kappa[pb]);
              m2_param->m2_resid_real[index3][ps][pb] =
                  ixheaacd_mps_mult32_shr_15(m2_param->m2_resid_real[index3][ps][pb], kappa[pb]);

              m2_param->m2_resid_real[pos[0]][ps][pb] =
                  ixheaacd_mps_mult32_shr_15(g_dd[pb], c_f_l[pb]);
              m2_param->m2_resid_real[pos[1]][ps][pb] =
                  ixheaacd_mps_mult32_shr_15(g_dd[pb], c_f_r[pb]);

              temp = ixheaacd_mps_mult32_shr_15(g_dd[pb], c_l_clfe[pb]);
              m2_param->m2_resid_real[pos[2]][ps][pb] = ixheaacd_mps_mult32_shr_30(temp, temp_1);
            }
          }
        }
      }
    }
  }
  return;
}
