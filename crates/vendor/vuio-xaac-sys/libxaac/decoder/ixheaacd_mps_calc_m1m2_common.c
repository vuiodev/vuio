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

ATTR_NO_SANITIZE_INTEGER
VOID ixheaacd_get_matrix_inversion_weights(
    WORD32 iid_lf_ls_idx, WORD32 iid_rf_rs_idx, WORD32 prediction_mode, WORD32 c1, WORD32 c2,
    WORD32 *weight1, WORD32 *weight2, ia_mps_dec_mps_tables_struct *ia_mps_dec_mps_table_ptr) {
  WORD32 temp, temp_1, temp_2, temp_one;
  WORD16 qtemp;
  WORD32 w1 = ia_mps_dec_mps_table_ptr->m1_m2_table_ptr->cld_tab_2[iid_lf_ls_idx + 15];
  WORD32 w2 = ia_mps_dec_mps_table_ptr->m1_m2_table_ptr->cld_tab_2[iid_rf_rs_idx + 15];

  if (prediction_mode == 1) {
    if (abs(c1) >= ONE_IN_Q15) {
      c1 = ONE_IN_Q15;
    } else if ((c1 < MINUS_ONE_IN_Q14) && (c1 > MINUS_ONE_IN_Q15)) {
      c1 = MINUS_ONE_IN_Q15 - (c1 << 1);
    } else {
      qtemp = 15;
      temp = ixheaacd_mps_mult32(TWO_BY_THREE_Q15, c1, &qtemp, 15);
      temp = ixheaacd_mps_convert_to_qn(temp, qtemp, 15);
      c1 = ONE_BY_THREE_Q15 + temp;
    }

    if (abs(c2) >= ONE_IN_Q15) {
      c2 = ONE_IN_Q15;
    } else if ((c2 < MINUS_ONE_IN_Q14) && (c2 > MINUS_ONE_IN_Q15)) {
      c2 = MINUS_ONE_IN_Q15 - (c2 << 1);
    } else {
      qtemp = 15;
      temp = ixheaacd_mps_mult32(TWO_BY_THREE_Q15, c2, &qtemp, 15);
      temp = ixheaacd_mps_convert_to_qn(temp, qtemp, 15);
      qtemp = 15;
      temp = ixheaacd_mps_add32(temp, ONE_BY_THREE_Q15, &qtemp, 15);
      c2 = ixheaacd_mps_convert_to_qn(temp, qtemp, 15);
    }
  } else {
    WORD32 c1p, c2p;
    WORD64 acc;
    const WORD32 *cld_tab_3 = ia_mps_dec_mps_table_ptr->m1_m2_table_ptr->cld_tab_3;
    const WORD32 *sqrt_tab = ia_mps_dec_mps_table_ptr->common_table_ptr->sqrt_tab;

    c1p = cld_tab_3[c1 + 15];
    c2p = cld_tab_3[c2 + 15];

    acc = (WORD64)((WORD64)c1p * (WORD64)c2p);
    acc >>= 15;
    temp = (WORD32)acc;
    temp_1 = (ONE_IN_Q15 + c2p) << 1;
    acc += temp_1;
    temp_2 = (WORD32)acc;

    temp = ixheaacd_mps_div_32(temp, temp_2, &qtemp);

    c1 = ixheaacd_mps_sqrt(temp, &qtemp, sqrt_tab);
    c1 = ixheaacd_mps_convert_to_qn(c1, qtemp, 15);

    temp_2 = ixheaac_add32_sat(c1p, temp_1);
    temp = ixheaacd_mps_div_32(c1p, temp_2, &qtemp);
    c2 = ixheaacd_mps_sqrt(temp, &qtemp, sqrt_tab);
    c2 = ixheaacd_mps_convert_to_qn(c2, qtemp, 15);
  }
  temp_one = ONE_IN_Q15;
  if (ixheaac_norm32(w1) == 0) {
    temp_one = ONE_IN_Q14;
    w1 = w1 >> 1;
  }
  temp_1 = temp_one + w1;
  temp_2 = ixheaacd_mps_mult32_shr_15(c1, w1);
  *weight1 = ixheaacd_mps_div32_in_q15(temp_2, temp_1);

  if (ixheaac_norm32(w2) == 0) {
    temp_one = ONE_IN_Q14;
    w2 = w2 >> 1;
  }
  temp_1 = temp_one + w2;
  temp_2 = ixheaacd_mps_mult32_shr_15(c2, w2);
  *weight2 = ixheaacd_mps_div32_in_q15(temp_2, temp_1);
}

VOID ixheaacd_invert_matrix(WORD32 weight1, WORD32 weight2, WORD32 h_real[][2],
                            WORD32 h_imag[][2],
                            const ia_mps_dec_common_tables_struct *common_tab_ptr) {
  WORD32 h11_f_real, h12_f_real, h21_f_real, h22_f_real;
  WORD32 h11_f_imag, h12_f_imag, h21_f_imag, h22_f_imag;

  WORD32 inv_norm_real, inv_norm_imag, inv_norm;

  WORD32 len1, len2;
  WORD16 q_len1 = 0, q_len2 = 0;

  WORD64 acc1, acc2;

  len1 = ixheaacd_mps_sqrt(
      (ONE_IN_Q15 - (weight1 << 1) + ixheaacd_mps_mult32_shr_n(weight1, weight1, 14)), &q_len1,
      common_tab_ptr->sqrt_tab);

  len2 = ixheaacd_mps_sqrt(
      (ONE_IN_Q15 - (weight2 << 1) + ixheaacd_mps_mult32_shr_n(weight2, weight2, 14)), &q_len2,
      common_tab_ptr->sqrt_tab);

  len1 = ixheaacd_mps_convert_to_qn(len1, q_len1, 15);
  len2 = ixheaacd_mps_convert_to_qn(len2, q_len2, 15);

  h11_f_real = ixheaacd_mps_div32_in_q15((ONE_IN_Q15 - weight1), len1);

  h11_f_imag = ixheaacd_mps_div32_in_q15(weight1, len1);

  h22_f_imag = ixheaac_negate32_sat(ixheaacd_mps_div32_in_q15(weight2, len2));

  h12_f_real = 0;

  h12_f_imag = ixheaacd_mps_mult32_shr_15(h22_f_imag, ONE_BY_SQRT_3_Q15);

  h21_f_real = 0;

  h21_f_imag = ixheaacd_mps_mult32_shr_15(h11_f_imag, -(ONE_BY_SQRT_3_Q15));

  h22_f_real = ixheaacd_mps_div32_in_q15((ONE_IN_Q15 - weight2), len2);

  acc1 =
      (WORD64)((WORD64)h11_f_real * (WORD64)h22_f_real - (WORD64)h11_f_imag * (WORD64)h22_f_imag);
  acc1 >>= 15;

  acc2 =
      (WORD64)((WORD64)h12_f_real * (WORD64)h21_f_real - (WORD64)h12_f_imag * (WORD64)h21_f_imag);
  acc2 >>= 15;
  inv_norm_real = (WORD32)(acc1 - acc2);

  acc1 =
      (WORD64)((WORD64)h11_f_real * (WORD64)h22_f_imag + (WORD64)h11_f_imag * (WORD64)h22_f_real);
  acc1 >>= 15;

  acc2 =
      (WORD64)((WORD64)h12_f_real * (WORD64)h21_f_imag + (WORD64)h12_f_imag * (WORD64)h21_f_real);
  acc2 >>= 15;
  inv_norm_imag = (WORD32)(acc1 + acc2);

  acc1 = (WORD64)((WORD64)inv_norm_real * (WORD64)inv_norm_real +
                  (WORD64)inv_norm_imag * (WORD64)inv_norm_imag);
  acc1 >>= 15;
  inv_norm = (WORD32)acc1;

  inv_norm_real = ixheaacd_mps_div32_in_q15(inv_norm_real, inv_norm);
  inv_norm_imag = -(ixheaacd_mps_div32_in_q15(inv_norm_imag, inv_norm));

  acc1 = (WORD64)((WORD64)h22_f_real * (WORD64)inv_norm_real -
                  (WORD64)h22_f_imag * (WORD64)inv_norm_imag);
  acc1 >>= 15;
  h_real[0][0] = (WORD32)acc1;

  acc1 = (WORD64)((WORD64)h22_f_real * (WORD64)inv_norm_imag +
                  (WORD64)h22_f_imag * (WORD64)inv_norm_real);
  acc1 >>= 15;
  h_imag[0][0] = (WORD32)acc1;

  acc1 = (WORD64)((WORD64)h12_f_imag * (WORD64)inv_norm_imag -
                  (WORD64)h12_f_real * (WORD64)inv_norm_real);
  acc1 >>= 15;
  h_real[0][1] = (WORD32)acc1;

  acc1 = (WORD64)((WORD64)h12_f_real * (WORD64)inv_norm_imag +
                  (WORD64)h12_f_imag * (WORD64)inv_norm_real);
  acc1 = -(acc1 >> 15);
  h_imag[0][1] = (WORD32)acc1;

  acc1 = (WORD64)((WORD64)h21_f_imag * (WORD64)inv_norm_imag -
                  (WORD64)h21_f_real * (WORD64)inv_norm_real);
  acc1 >>= 15;
  h_real[1][0] = (WORD32)acc1;

  acc1 = (WORD64)((WORD64)h21_f_real * (WORD64)inv_norm_imag +
                  (WORD64)h21_f_imag * (WORD64)inv_norm_real);
  acc1 = -(acc1 >> 15);
  h_imag[1][0] = (WORD32)acc1;

  acc1 = (WORD64)((WORD64)h11_f_real * (WORD64)inv_norm_real -
                  (WORD64)h11_f_imag * (WORD64)inv_norm_imag);
  acc1 >>= 15;
  h_real[1][1] = (WORD32)acc1;

  acc1 = (WORD64)((WORD64)h11_f_real * (WORD64)inv_norm_imag +
                  (WORD64)h11_f_imag * (WORD64)inv_norm_real);
  acc1 >>= 15;
  h_imag[1][1] = (WORD32)acc1;
}

WORD32 ixheaacd_dequant_icc_band(WORD32 iccband, WORD32 cldband) {
  if (iccband < 6) {
    return iccband;
  }
  if (iccband == 6) {
    if (cldband > 9 && cldband < 21) {
      switch (cldband) {
        case 10:
        case 20:
          return 10;
        case 11:
        case 19:
          return 11;
        case 12:
        case 18:
          return 12;
        case 13:
        case 17:
          return 13;
        case 14:
        case 16:
          return 14;
        case 15:
          return 15;
        default:
          return iccband;
      }
    } else {
      return iccband;
    }
  }
  if (7 == iccband) {
    if (cldband > 7 && cldband < 23) {
      switch (cldband) {
        case 8:
        case 22:
          return 8;
        case 9:
        case 21:
          return 9;
        case 10:
        case 20:
          return 10;
        case 11:
        case 19:
          return 11;
        case 12:
        case 18:
          return 12;
        case 13:
        case 17:
          return 13;
        case 14:
        case 16:
          return 14;
        case 15:
          return 15;
        default:
          return iccband;
      }
    } else {
      return iccband;
    }
  } else {
    return iccband;
  }
}

WORD32 ixheaacd_dequant_cld_band(WORD32 cld) {
  switch (cld) {
    case -4915200:
      return 0;
    case -1474560:
      return 1;
    case -1310720:
      return 2;
    case -1146880:
      return 3;
    case -983040:
      return 4;
    case -819200:
      return 5;
    case -720896:
      return 6;
    case -622592:
      return 7;
    case -524288:
      return 8;
    case -425984:
      return 9;
    case -327680:
      return 10;
    case -262144:
      return 11;
    case -196608:
      return 12;
    case -131072:
      return 13;
    case -65536:
      return 14;
    case 0:
      return 15;
    case 65536:
      return 16;
    case 131072:
      return 17;
    case 196608:
      return 18;
    case 262144:
      return 19;
    case 327680:
      return 20;
    case 425984:
      return 21;
    case 524288:
      return 22;
    case 622592:
      return 23;
    case 720896:
      return 24;
    case 819200:
      return 25;
    case 983040:
      return 26;
    case 1146880:
      return 27;
    case 1310720:
      return 28;
    case 1474560:
      return 29;
    case 4915200:
      return 30;
    default:
      return 0;
  }
}

VOID ixheaacd_param_2_umx_ps_core_tables(
    WORD32 cld[MAX_PARAMETER_BANDS], WORD32 icc[MAX_PARAMETER_BANDS], WORD32 num_ott_bands,
    WORD32 res_bands, WORD32 h11[MAX_PARAMETER_BANDS], WORD32 h12[MAX_PARAMETER_BANDS],
    WORD32 h21[MAX_PARAMETER_BANDS], WORD32 h22[MAX_PARAMETER_BANDS],
    WORD32 h12_res[MAX_PARAMETER_BANDS], WORD32 h22_res[MAX_PARAMETER_BANDS],
    WORD16 c_l[MAX_PARAMETER_BANDS], WORD16 c_r[MAX_PARAMETER_BANDS],
    const ia_mps_dec_m1_m2_tables_struct *ixheaacd_mps_dec_m1_m2_tables) {
  WORD32 band;
  WORD32 quant_band_cld, quant_band_icc;

  for (band = 0; band < num_ott_bands; band++) {
    quant_band_cld = ixheaacd_dequant_cld_band(cld[band]);

    c_l[band] = (WORD16)ixheaacd_mps_dec_m1_m2_tables->c_l_table[quant_band_cld];
    c_r[band] = (WORD16)ixheaacd_mps_dec_m1_m2_tables->c_l_table[30 - quant_band_cld];
  }

  for (band = 0; band < num_ott_bands; band++) {
    if (band < res_bands) {
      quant_band_cld = ixheaacd_dequant_cld_band(cld[band]);
      quant_band_icc = ixheaacd_dequant_icc_band(icc[band], quant_band_cld);

      h11[band] = ixheaacd_mps_dec_m1_m2_tables->cos_table[quant_band_icc][quant_band_cld];
      h11[band] = ixheaacd_mps_mult32_shr_15(h11[band], c_l[band]);
      h21[band] = ixheaacd_mps_dec_m1_m2_tables->cos_table[quant_band_icc][30 - quant_band_cld];
      h21[band] = ixheaacd_mps_mult32_shr_15(h21[band], c_r[band]);

      h12[band] = 0;
      h22[band] = 0;
      h12_res[band] = ONE_IN_Q15;
      h22_res[band] = MINUS_ONE_IN_Q15;
    } else {
      quant_band_cld = ixheaacd_dequant_cld_band(cld[band]);
      if (quant_band_cld < 0 || quant_band_cld >= 31) {
        quant_band_cld = 30;
      }

      quant_band_icc = icc[band];

      if (quant_band_icc < 0 || quant_band_icc >= 8) {
        quant_band_icc = 7;
      }
      h11[band] = ixheaacd_mps_dec_m1_m2_tables->cos_table[quant_band_icc][quant_band_cld];
      h11[band] = ixheaacd_mps_mult32_shr_15(h11[band], c_l[band]);
      h21[band] = ixheaacd_mps_dec_m1_m2_tables->cos_table[quant_band_icc][30 - quant_band_cld];
      h21[band] = ixheaacd_mps_mult32_shr_15(h21[band], c_r[band]);
      h12[band] = ixheaacd_mps_dec_m1_m2_tables->sin_table[quant_band_icc][quant_band_cld];
      h12[band] = ixheaacd_mps_mult32_shr_15(h12[band], c_l[band]);
      h22[band] = -ixheaacd_mps_dec_m1_m2_tables->sin_table[quant_band_icc][30 - quant_band_cld];
      h22[band] = ixheaacd_mps_mult32_shr_15(h22[band], c_r[band]);

      h12_res[band] = 0;
      h22_res[band] = 0;
    }
  }
  return;
}

VOID ixheaacd_param_2_umx_ps(ia_heaac_mps_state_struct *pstr_mps_state, WORD32 *h11, WORD32 *h12,
                             WORD32 *h21, WORD32 *h22, WORD32 *h12_res, WORD32 *h22_res,
                             WORD16 *c_l, WORD16 *c_r, WORD32 ott_box_indx,
                             WORD32 parameter_set_indx, WORD32 res_bands) {
  WORD32 band;
  ia_mps_dec_spatial_bs_frame_struct *p_cur_bs = pstr_mps_state->bs_frame;
  ia_mps_dec_auxilary_struct *p_aux_struct = pstr_mps_state->aux_struct;
  WORD32 num_parameter_bands = pstr_mps_state->num_parameter_bands;

  ixheaacd_param_2_umx_ps_core_tables(p_aux_struct->ott_cld[ott_box_indx][parameter_set_indx],
                                      p_cur_bs->ott_icc_idx[ott_box_indx][parameter_set_indx],
                                      p_aux_struct->num_ott_bands[ott_box_indx], res_bands, h11,
                                      h12, h21, h22, h12_res, h22_res, c_l, c_r,
                                      pstr_mps_state->ia_mps_dec_mps_table.m1_m2_table_ptr);

  for (band = p_aux_struct->num_ott_bands[ott_box_indx]; band < num_parameter_bands; band++) {
    h11[band] = h21[band] = h12[band] = h22[band] = h12_res[band] = h22_res[band] = 0;
  }
  return;
}

static WORD32 ixheaacd_dequant_one_by_icc(WORD32 icc) {
  switch (icc) {
    case 32768:
      return 32768;
    case 30704:
      return 34971;
    case 27564:
      return 38955;
    case 19691:
      return 54530;
    case 12047:
      return 89131;
    case 0:
      return 0;
    case -19300:
      return -55633;
    case -32440:
      return -33099;
    default:
      return 0;
  }
}

static WORD16 ixheaacd_map_cld_index(WORD32 cld_val) {
  WORD32 temp = cld_val;
  WORD16 idx = 0;
  if (cld_val == 0) {
    return 15;
  } else {
    if (cld_val < 0) {
      temp = -cld_val;
    }
    switch (temp) {
      case 150:
        idx = 15;
        break;
      case 45:
        idx = 14;
        break;
      case 40:
        idx = 13;
        break;
      case 35:
        idx = 12;
        break;
      case 30:
        idx = 11;
        break;
      case 25:
        idx = 10;
        break;
      case 22:
        idx = 9;
        break;
      case 19:
        idx = 8;
        break;
      case 16:
        idx = 7;
        break;
      case 13:
        idx = 6;
        break;
      case 10:
        idx = 5;
        break;
      case 8:
        idx = 4;
        break;
      case 6:
        idx = 3;
        break;
      case 4:
        idx = 2;
        break;
      case 2:
        idx = 1;
        break;
      default:
        idx = 0;
        break;
    }
  }

  return (cld_val >= 0) ? idx + 15 : 15 - idx;
}

VOID ixheaacd_calculate_ttt(ia_heaac_mps_state_struct *pstr_mps_state, WORD32 ps, WORD32 pb,
                            WORD32 ttt_mode, WORD32 m_ttt[][3]) {
  ia_mps_dec_auxilary_struct *p_aux_struct = pstr_mps_state->aux_struct;
  WORD32 col;

  if (ttt_mode < 2) {
    m_ttt[0][0] = (p_aux_struct->ttt_cpc_1[0][ps][pb] + ONE_IN_Q16);
    m_ttt[0][1] = (p_aux_struct->ttt_cpc_2[0][ps][pb] - ONE_IN_Q15);
    m_ttt[1][0] = (p_aux_struct->ttt_cpc_1[0][ps][pb] - ONE_IN_Q15);
    m_ttt[1][1] = (p_aux_struct->ttt_cpc_2[0][ps][pb] + ONE_IN_Q16);
    m_ttt[2][0] = (ONE_IN_Q15 - p_aux_struct->ttt_cpc_1[0][ps][pb]);
    m_ttt[2][1] = (ONE_IN_Q15 - p_aux_struct->ttt_cpc_2[0][ps][pb]);

    if (pb >= pstr_mps_state->res_bands[3]) {
      WORD32 one_by_icc;
      one_by_icc = ixheaacd_dequant_one_by_icc(p_aux_struct->ttt_icc[0][ps][pb]);

      m_ttt[0][0] = ixheaacd_mps_mult32_shr_15(m_ttt[0][0], one_by_icc);
      m_ttt[0][1] = ixheaacd_mps_mult32_shr_15(m_ttt[0][1], one_by_icc);
      m_ttt[1][0] = ixheaacd_mps_mult32_shr_15(m_ttt[1][0], one_by_icc);
      m_ttt[1][1] = ixheaacd_mps_mult32_shr_15(m_ttt[1][1], one_by_icc);
      m_ttt[2][0] = ixheaacd_mps_mult32_shr_15(m_ttt[2][0], one_by_icc);
      m_ttt[2][1] = ixheaacd_mps_mult32_shr_15(m_ttt[2][1], one_by_icc);
    }

    m_ttt[0][0] = ixheaac_mult32x16in32(m_ttt[0][0], TWO_BY_THREE_Q15);
    m_ttt[0][1] = ixheaac_mult32x16in32(m_ttt[0][1], TWO_BY_THREE_Q15);
    m_ttt[1][0] = ixheaac_mult32x16in32(m_ttt[1][0], TWO_BY_THREE_Q15);
    m_ttt[1][1] = ixheaac_mult32x16in32(m_ttt[1][1], TWO_BY_THREE_Q15);
    m_ttt[2][0] = ixheaac_mult32x16in32(m_ttt[2][0], TWO_BY_THREE_Q15);
    m_ttt[2][1] = ixheaac_mult32x16in32(m_ttt[2][1], TWO_BY_THREE_Q15);
  } else {
    WORD32 center_wiener;
    WORD32 center_subtraction;
    WORD32 c1d, c2d;
    WORD64 prod;
    WORD32 w11, w00, w20, w21;
    WORD16 q_w11, q_w00, q_w20, q_w21;

    const WORD32 *ten_cld_by_10 =
        pstr_mps_state->ia_mps_dec_mps_table.m1_m2_table_ptr->ten_cld_by_10;

    ia_mps_dec_spatial_bs_frame_struct *p_cur_bs = pstr_mps_state->bs_frame;
    WORD16 index = ixheaacd_map_cld_index(p_aux_struct->ttt_cld_1[0][ps][pb] >> 15);

    c1d = ten_cld_by_10[index];

    index = ixheaacd_map_cld_index(p_aux_struct->ttt_cld_2[0][ps][pb] >> 15);
    c2d = ten_cld_by_10[index];

    if (p_cur_bs->cmp_ttt_cld_1_idx[0][ps][pb] == 15 ||
        p_cur_bs->cmp_ttt_cld_2_idx[0][ps][pb] == 15) {
      if (p_cur_bs->cmp_ttt_cld_1_idx[0][ps][pb] == 15) {
        if (p_cur_bs->cmp_ttt_cld_2_idx[0][ps][pb] == -15) {
          w00 = ONE_BY_SQRT_2_Q15;
          w20 = ONE_BY_SQRT_8_Q15;
        } else {
          w00 = ONE_IN_Q15;
          w20 = 0;
        }

        if (p_cur_bs->cmp_ttt_cld_2_idx[0][ps][pb] == 15) {
          w11 = ONE_BY_SQRT_2_Q15;
          w21 = ONE_BY_SQRT_8_Q15;
        } else {
          w11 = ONE_IN_Q15;
          w21 = 0;
        }

        m_ttt[0][0] = w00;
        m_ttt[2][0] = w20;
        m_ttt[2][1] = w21;
        m_ttt[1][1] = w11;
      }

      if (p_cur_bs->cmp_ttt_cld_2_idx[0][ps][pb] == 15) {
        const WORD32 *w00_cld2_15 =
            pstr_mps_state->ia_mps_dec_mps_table.m1_m2_table_ptr->w00_cld2_15;

        if (p_cur_bs->cmp_ttt_cld_1_idx[0][ps][pb] == 15) {
          w11 = ONE_BY_SQRT_2_Q15;
          w21 = ONE_BY_SQRT_8_Q15;
        } else {
          w11 = 0;
          w21 = ONE_IN_Q14;
        }

        w00 = w00_cld2_15[p_cur_bs->cmp_ttt_cld_1_idx[0][ps][pb] + 15];
        w20 = w00_cld2_15[15 - p_cur_bs->cmp_ttt_cld_1_idx[0][ps][pb]] / 2;
        m_ttt[0][0] = w00;
        m_ttt[2][0] = w20;
        m_ttt[2][1] = w21;
        m_ttt[1][1] = w11;
      }

      m_ttt[0][1] = 0;
      m_ttt[1][0] = 0;
    } else {
      WORD32 temporary;
      const WORD32 *sqrt_tab = pstr_mps_state->ia_mps_dec_mps_table.common_table_ptr->sqrt_tab;
      prod = ixheaacd_mps_mult32_shr_15(c1d, c2d);

      temporary = ixheaac_add32_sat(ONE_IN_Q15, c2d);
      temporary = ixheaac_add32_sat(temporary, (WORD32)prod);
      w00 = ixheaacd_mps_div_32((WORD32)prod, temporary, &q_w00);

      w11 = ixheaacd_mps_div_32(c1d, (ixheaac_add32_sat3(c1d, c2d, ONE_IN_Q15)), &q_w11);

      w20 = ixheaacd_mps_div_32((ixheaac_add32_sat(c2d, ONE_IN_Q15)),
                                ixheaac_add32_sat3(ONE_IN_Q15, (WORD32)prod, c2d), &q_w20);

      w21 = ixheaacd_mps_div_32(ixheaac_add32_sat(c2d, ONE_IN_Q15),
                                (ixheaac_add32_sat3(c1d, c2d, ONE_IN_Q15)), &q_w21);

      m_ttt[0][0] = ixheaacd_mps_sqrt(w00, &q_w00, sqrt_tab);
      m_ttt[0][0] = ixheaacd_mps_convert_to_qn(m_ttt[0][0], q_w00, 15);

      m_ttt[0][1] = 0;
      m_ttt[1][0] = 0;

      m_ttt[1][1] = ixheaacd_mps_sqrt(w11, &q_w11, sqrt_tab);
      m_ttt[1][1] = ixheaacd_mps_convert_to_qn(m_ttt[1][1], q_w11, 15);

      m_ttt[2][0] = ixheaacd_mps_sqrt(w20, &q_w20, sqrt_tab) >> 1;

      m_ttt[2][0] = ixheaacd_mps_convert_to_qn(m_ttt[2][0], q_w20, 15);

      m_ttt[2][1] = ixheaacd_mps_sqrt(w21, &q_w21, sqrt_tab) >> 1;

      m_ttt[2][1] = ixheaacd_mps_convert_to_qn(m_ttt[2][1], q_w21, 15);
      if (p_aux_struct->ttt_cld_1[0][ps][pb] == 4915200) {
        m_ttt[0][0] = 32767;
        m_ttt[1][1] = 32767;
        m_ttt[2][0] = 0;
        m_ttt[2][1] = 0;
      }
    }

    center_wiener = 0;
    center_subtraction = (ttt_mode == 2 || ttt_mode == 3);

    if (center_wiener) {
      WORD32 cld_1_idx = p_cur_bs->cmp_ttt_cld_1_idx[0][ps][pb];
      WORD32 cld_2_idx = p_cur_bs->cmp_ttt_cld_2_idx[0][ps][pb];

      if (cld_1_idx == 15 && cld_2_idx == 15) {
        m_ttt[2][0] = 0;
        m_ttt[2][1] = ONE_BY_SQRT_2_Q15;
      } else if (cld_1_idx == 15) {
        if (cld_2_idx == -15)
          m_ttt[2][0] = ONE_BY_SQRT_2_Q15;
        else
          m_ttt[2][0] = 0;
        m_ttt[2][1] = 0;
      } else if (cld_2_idx == 15) {
        m_ttt[2][0] = 0;
        m_ttt[2][1] = ONE_IN_Q15;
      } else {
        WORD32 temp;
        WORD16 q_temp;
        const WORD32 *sqrt_tab = pstr_mps_state->ia_mps_dec_mps_table.common_table_ptr->sqrt_tab;

        prod = ixheaacd_mps_mult32_shr_15(c2d, (c2d + c1d + ONE_IN_Q16)) + ONE_IN_Q15;

        temp = ixheaacd_mps_div_32((WORD32)ONE_IN_Q15, (WORD32)prod, &q_temp);

        m_ttt[2][0] = ixheaacd_mps_sqrt(temp, &q_temp, sqrt_tab);
        m_ttt[2][0] = ixheaacd_mps_convert_to_qn(m_ttt[2][0], q_temp, 15);

        m_ttt[2][1] = ixheaacd_mps_mult32_shr_15(c2d, m_ttt[2][0]);
      }
    }

    if (center_subtraction) {
      WORD32 wl1, wl2, wr1, wr2;
      WORD16 q_wl1, q_wr1;
      WORD32 cld_1_idx = p_cur_bs->cmp_ttt_cld_1_idx[0][ps][pb];
      WORD32 cld_2_idx = p_cur_bs->cmp_ttt_cld_2_idx[0][ps][pb];

      if (cld_1_idx == 15 && cld_2_idx == 15) {
        m_ttt[0][0] = ONE_IN_Q15;
        m_ttt[0][1] = MINUS_ONE_IN_Q14;
        m_ttt[1][1] = ONE_BY_SQRT_2_Q15;
        m_ttt[1][0] = 0;
      } else if (cld_1_idx == 15) {
        if (cld_2_idx == -15) {
          m_ttt[0][0] = ONE_BY_SQRT_2_Q15;
          m_ttt[1][0] = MINUS_ONE_IN_Q14;
        } else {
          m_ttt[0][0] = ONE_IN_Q15;
          m_ttt[1][0] = 0;
        }

        m_ttt[0][1] = 0;
        m_ttt[1][1] = ONE_IN_Q15;
      } else if (cld_2_idx == 15) {
        m_ttt[0][0] = ONE_IN_Q15;
        m_ttt[0][1] = MINUS_ONE_IN_Q15;
        m_ttt[1][1] = 0;
        m_ttt[1][0] = 0;
      } else {
        WORD32 temp, temp_1, q_a;
        WORD16 q_c, q_l, q_r, q_temp, q_temp1;
        WORD32 c;
        WORD32 r;
        WORD32 l;
        const WORD32 *sqrt_tab = pstr_mps_state->ia_mps_dec_mps_table.common_table_ptr->sqrt_tab;

        c = ixheaacd_mps_div_32(ONE_IN_Q15, (ixheaac_add32_sat(c1d, ONE_IN_Q15)), &q_c);
        r = ixheaacd_mps_div_32(c1d, (ixheaac_add32_sat(c2d, ONE_IN_Q15)), &q_r);
        r = ixheaacd_mps_mult32_shr_30(r, c);
        q_r = q_r + q_c - 30;

        l = ixheaacd_mps_mult32_shr_30(c2d, r);
        q_l = q_r - 15;

        temp = ixheaacd_mps_div_32(r, l, &q_temp);
        q_temp += (q_r - q_l);

        if (q_temp > 28) {
          temp = temp >> (q_temp - 28);
          q_temp = 28;
        }

        temp += ((1 << q_temp) - 1);

        temp = ixheaac_add32_sat(
            ixheaacd_mps_mult32_shr_n(c, temp, (WORD16)(q_c + q_temp - q_r)), r);
        q_temp = q_r;

        if (q_c > q_r) {
          temp_1 = r + (c >> (q_c - q_r));
          q_temp1 = q_r;
        } else {
          temp_1 = ixheaac_add32_sat((r >> (q_r - q_c)), c);
          q_temp1 = q_c;
        }

        temp = ixheaac_div32(temp_1, temp, &q_a);
        q_wl1 = q_a + q_temp1 - q_temp;
        wl1 = ixheaacd_mps_sqrt(temp, &q_wl1, sqrt_tab);
        m_ttt[0][0] = ixheaacd_mps_convert_to_qn(wl1, q_wl1, 15);

        temp = ixheaac_div32(wl1, temp_1, &q_a);
        q_temp = q_a + (q_wl1 - q_temp1);
        wl2 = ixheaacd_mps_mult32_shr_n(c, temp, (WORD16)(q_c + q_temp - 15));
        m_ttt[0][1] = ixheaac_negate32_sat(wl2);

        temp = ixheaacd_mps_div_32(l, r, &q_temp);
        q_temp += (q_l - q_r);

        if (q_temp > 28) {
          temp = temp >> (q_temp - 28);
          q_temp = 28;
        }

        temp = ixheaac_add32_sat((1 << q_temp) - 1, temp);

        temp = ixheaac_add32_sat(
                   ixheaacd_mps_mult32_shr_n(c, temp, (WORD16)(q_c + q_temp - q_l)), l);

        q_temp = q_l;

        if (q_c > q_l) {
          temp_1 = l + (c >> (q_c - q_l));
          q_temp1 = q_l;
        } else {
          temp_1 = ixheaac_add32_sat((l >> (q_l - q_c)), c);
          q_temp1 = q_c;
        }

        temp = ixheaac_div32(temp_1, temp, &q_a);
        q_wr1 = q_a + q_temp1 - q_temp;
        wr1 = ixheaacd_mps_sqrt(temp, &q_wr1, sqrt_tab);
        m_ttt[1][1] = ixheaacd_mps_convert_to_qn(wr1, q_wr1, 15);

        temp = ixheaac_div32(wr1, temp_1, &q_a);
        q_temp = q_a + (q_wl1 - q_temp1);
        wr2 = ixheaacd_mps_mult32_shr_n(c, temp, (WORD16)(q_c + q_temp - 15));
        m_ttt[1][0] = ixheaac_negate32_sat(wr2);
      }
    }
  }

  m_ttt[0][2] = ONE_BY_THREE_Q15;
  m_ttt[1][2] = ONE_BY_THREE_Q15;
  m_ttt[2][2] = MINUS_ONE_BY_THREE_Q15;

  for (col = 0; col < 3; col++) {
    m_ttt[2][col] = ixheaacd_mps_mult32_shr_15(m_ttt[2][col], SQRT_TWO_Q15);
  }
}

VOID ixheaacd_calculate_mtx_inv(ia_heaac_mps_state_struct *pstr_mps_state, WORD32 ps, WORD32 pb,
                                WORD32 mode, WORD32 h_real[][2], WORD32 h_imag[][2]) {
  ia_mps_dec_auxilary_struct *p_aux_struct = pstr_mps_state->aux_struct;

  WORD32 weight1;
  WORD32 weight2;
  ia_mps_dec_spatial_bs_frame_struct *p_cur_bs = pstr_mps_state->bs_frame;

  if (mode < 2) {
    ixheaacd_get_matrix_inversion_weights(
        p_cur_bs->ott_cld_idx[1][ps][pb], p_cur_bs->ott_cld_idx[2][ps][pb], 1,
        p_aux_struct->ttt_cpc_1[0][ps][pb], p_aux_struct->ttt_cpc_2[0][ps][pb], &weight1,
        &weight2, &(pstr_mps_state->ia_mps_dec_mps_table));
  } else {
    ixheaacd_get_matrix_inversion_weights(
        p_cur_bs->ott_cld_idx[1][ps][pb], p_cur_bs->ott_cld_idx[2][ps][pb], 0,
        p_cur_bs->cmp_ttt_cld_1_idx[0][ps][pb], p_cur_bs->cmp_ttt_cld_2_idx[0][ps][pb], &weight1,
        &weight2, &(pstr_mps_state->ia_mps_dec_mps_table));
  }

  ixheaacd_invert_matrix(weight1, weight2, h_real, h_imag,
                         pstr_mps_state->ia_mps_dec_mps_table.common_table_ptr);
}

VOID ixheaacd_calculate_arb_dmx_mtx(ia_heaac_mps_state_struct *pstr_mps_state, WORD32 ps,
                                    WORD32 pb, WORD32 g_real[]) {
  WORD32 ch;
  WORD32 gain;

  WORD32 *arbdmx_alpha_prev = pstr_mps_state->mps_persistent_mem.arbdmx_alpha_prev;
  WORD32 *arbdmx_alpha_upd_set = pstr_mps_state->aux_struct->arbdmx_alpha_upd_set;
  WORD32 *arbdmx_alpha = pstr_mps_state->aux_struct->arbdmx_alpha;

  WORD32 n_ch_in = pstr_mps_state->num_input_channels;
  WORD32 temp_1;
  for (ch = 0; ch < n_ch_in; ch++) {
    temp_1 = pstr_mps_state->bs_frame->arbdmx_gain_idx[ch][ps][pb] + 15;
    gain = pstr_mps_state->ia_mps_dec_mps_table.m1_m2_table_ptr->dec_pow[temp_1];

    if (pb < pstr_mps_state->arbdmx_residual_bands) {
      if ((ps == 0) && (arbdmx_alpha_upd_set[ch] == 1)) {
        g_real[ch] = ixheaacd_mps_mult32_shr_15(*arbdmx_alpha_prev, gain);
      } else {
        g_real[ch] = ixheaacd_mps_mult32_shr_15(arbdmx_alpha[ch], gain);
      }
    } else {
      g_real[ch] = gain;
    }
    arbdmx_alpha_prev++;
  }
}

WORD32 ixheaacd_quantize(WORD32 cld) {
  switch (cld) {
    case -150:
      return -15;
    case -45:
      return -14;
    case -40:
      return -13;
    case -35:
      return -12;
    case -30:
      return -11;
    case -25:
      return -10;
    case -22:
      return -9;
    case -19:
      return -8;
    case -16:
      return -7;
    case -13:
      return -6;
    case -10:
      return -5;
    case -8:
      return -4;
    case -6:
      return -3;
    case -4:
      return -2;
    case -2:
      return -1;
    case 0:
      return 0;
    case 2:
      return 1;
    case 4:
      return 2;
    case 6:
      return 3;
    case 8:
      return 4;
    case 10:
      return 5;
    case 13:
      return 6;
    case 16:
      return 7;
    case 19:
      return 8;
    case 22:
      return 9;
    case 25:
      return 10;
    case 30:
      return 11;
    case 35:
      return 12;
    case 40:
      return 13;
    case 45:
      return 14;
    case 150:
      return 15;
    default:
      return 0;
  }
}
