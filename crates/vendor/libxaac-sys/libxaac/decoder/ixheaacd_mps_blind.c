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
#include "ixheaacd_error_codes.h"
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
#include "ixheaacd_mps_bitdec.h"
#include "ixheaacd_mps_macro_def.h"
#include "ixheaacd_mps_basic_op.h"
#include "ixheaacd_mps_blind.h"

static IA_ERRORCODE ixheaacd_get_sampling_freq_idx(
    WORD32 sampling_freq, ia_mps_dec_bitdec_tables_struct *ixheaacd_mps_dec_bitdec_tables,
    WORD32 *idx) {
  WORD32 i;
  for (i = 0; i < 13; i++) {
    if (ixheaacd_mps_dec_bitdec_tables->sampling_freq_table[i] == sampling_freq) {
      *idx = i;
      return IA_NO_ERROR;
    }
  }
  *idx = 3;
  return IA_XHEAAC_DEC_CONFIG_FATAL_INVALID_SAMPLE_RATE;
}

VOID ixheaacd_init_blind(ia_heaac_mps_state_struct *pstr_mps_state) {
  ia_mps_dec_blind_decoder_struct *blind = pstr_mps_state->mps_persistent_mem.blind_decoder;
  ia_mps_dec_blind_tables_struct *p_blind_table =
      pstr_mps_state->ia_mps_dec_mps_table.blind_table_ptr;
  ia_mps_dec_auxilary_struct *p_aux_struct = pstr_mps_state->aux_struct;
  WORD32 temp_1, temp_2;
  WORD32 q_64, q_32, q_16, q_8, q_4, q_2;
  WORD32 r_64, r_32, r_16, r_8, r_4, r_2;
  WORD32 i;
  WORD32 qmf_bands = pstr_mps_state->qmf_bands;

  ixheaacd_get_sampling_freq_idx(pstr_mps_state->sampling_freq,
                                 pstr_mps_state->ia_mps_dec_mps_table.bitdec_table_ptr, &temp_1);
  if (pstr_mps_state->qmf_bands == 128)
    blind->filter_coeff = p_blind_table->exp_128[temp_1];
  else {
    q_64 = (WORD32)(qmf_bands >> 6) ? p_blind_table->exp_64[temp_1] : ONE_IN_Q15;
    r_64 = (WORD32)(qmf_bands & SIX_BIT_MASK);

    q_32 = (WORD32)(r_64 >> 5) ? p_blind_table->exp_32[temp_1] : ONE_IN_Q15;
    r_32 = (WORD32)(r_64 & FIVE_BIT_MASK);

    q_16 = (WORD32)(r_32 >> 4) ? p_blind_table->exp_16[temp_1] : ONE_IN_Q15;
    r_16 = (WORD32)(r_32 & FOUR_BIT_MASK);

    q_8 = (WORD32)(r_16 >> 3) ? p_blind_table->exp_8[temp_1] : ONE_IN_Q15;
    r_8 = (WORD32)(r_16 & THREE_BIT_MASK);

    q_4 = (WORD32)(r_8 >> 2) ? p_blind_table->exp_4[temp_1] : ONE_IN_Q15;
    r_4 = (WORD32)(r_8 & TWO_BIT_MASK);

    q_2 = (WORD32)(r_4 >> 1) ? p_blind_table->exp_2[temp_1] : ONE_IN_Q15;
    r_2 = (WORD32)(r_4 & ONE_BIT_MASK) ? p_blind_table->exp_1[temp_1] : ONE_IN_Q15;

    temp_1 = ixheaacd_mps_mult32_shr_15(ixheaacd_mps_mult32_shr_15(q_64, q_32), q_16);
    temp_2 = ixheaacd_mps_mult32_shr_15(ixheaacd_mps_mult32_shr_15(q_8, q_4), q_2);
    blind->filter_coeff =
        ixheaacd_mps_mult32_shr_15(ixheaacd_mps_mult32_shr_15(temp_1, temp_2), r_2);
  }
  for (i = 0; i < MAX_PARAMETER_BANDS; i++) {
    blind->excitation[0][i] = ABS_THR_FIX;
    blind->excitation[1][i] = ABS_THR_FIX;
    blind->excitation[2][i] = ABS_THR_FIX;

    blind->q_excitation[0][i] = 15;
    blind->q_excitation[1][i] = 15;
    blind->q_excitation[2][i] = 15;
  }

  memset(p_aux_struct->temp_shape_enable_channel_stp, 0,
         MAX_OUTPUT_CHANNELS_MPS * sizeof(p_aux_struct->temp_shape_enable_channel_stp[0]));
  memset(p_aux_struct->temp_shape_enable_channel_ges, 0,
         MAX_OUTPUT_CHANNELS_MPS * sizeof(p_aux_struct->temp_shape_enable_channel_ges[0]));
}

static VOID ixheaacd_signal_2_parameters(ia_heaac_mps_state_struct *pstr_mps_state, WORD32 ps) {
  ia_mps_dec_blind_decoder_struct *blind = pstr_mps_state->mps_persistent_mem.blind_decoder;
  ia_mps_dec_blind_tables_struct *p_blind_table =
      pstr_mps_state->ia_mps_dec_mps_table.blind_table_ptr;
  ia_mps_dec_auxilary_struct *p_aux_struct = pstr_mps_state->aux_struct;

  WORD32 cld_index;
  WORD32 icc_index;
  WORD32 mesh[2][2];
  WORD32 pb;
  WORD32 *dequant_cld;
  WORD32 *dequant_icc;
  WORD32 *dequant_cpc;
  WORD32 cld;
  WORD32 icc;
  WORD16 q_icc;
  WORD32 cld_delta;
  WORD32 icc_delta;
  WORD16 q_icc_delta;
  WORD32 temp_1;
  WORD16 qtemp1, qtemp;
  const WORD32 *sqrt_tab = pstr_mps_state->ia_mps_dec_mps_table.common_table_ptr->sqrt_tab;

  WORD32 num_parameter_bands = pstr_mps_state->num_parameter_bands;

  ixheaacd_get_dequant_tables(&dequant_cld, &dequant_icc, &dequant_cpc,
                              pstr_mps_state->ia_mps_dec_mps_table.bitdec_table_ptr);

  for (pb = 0; pb < num_parameter_bands; pb++) {
    temp_1 = ixheaacd_mps_div_32(blind->excitation[0][pb], blind->excitation[1][pb], &qtemp1);
    qtemp1 = qtemp1 + blind->q_excitation[0][pb] - blind->q_excitation[1][pb];
    cld = 10 * ixheaacd_mps_log10(temp_1, qtemp1);

    qtemp1 = blind->q_excitation[0][pb];
    temp_1 = ixheaacd_mps_mult32(blind->excitation[0][pb], blind->excitation[1][pb], &qtemp1,
                                 blind->q_excitation[1][pb]);
    temp_1 = ixheaacd_mps_sqrt(temp_1, &qtemp1, sqrt_tab);
    icc = ixheaacd_mps_div_32(blind->excitation[2][pb], temp_1, &q_icc);
    q_icc = q_icc + blind->q_excitation[2][pb] - qtemp1;

    if (abs(cld) > THIRTY_IN_Q16)
      cld_delta = THIRTY_IN_Q16;
    else
      cld_delta = ixheaac_abs32(cld);

    q_icc_delta = q_icc;
    icc_delta = ixheaacd_mps_add32(icc, ONE_IN_Q15, &q_icc_delta, 15);
    icc_delta = ixheaacd_mps_mult32(icc_delta, 10, &q_icc_delta, 0);

    temp_1 = cld_delta >> 16;
    if (temp_1 > 29)
      cld_index = 29;
    else
      cld_index = temp_1;

    temp_1 = icc_delta >> q_icc_delta;
    if (temp_1 > 19)
      icc_index = 19;
    else
      icc_index = temp_1;

    cld_delta -= (cld_index << 16);
    icc_delta -= (icc_index << q_icc_delta);

    mesh[0][0] = p_blind_table->mesh_table.blind_cld_mesh[cld_index][icc_index] + 15;
    mesh[0][1] = p_blind_table->mesh_table.blind_cld_mesh[cld_index][icc_index + 1] + 15;
    mesh[1][0] = p_blind_table->mesh_table.blind_cld_mesh[cld_index + 1][icc_index] + 15;
    mesh[1][1] = p_blind_table->mesh_table.blind_cld_mesh[cld_index + 1][icc_index + 1] + 15;

    qtemp1 = q_icc_delta;
    temp_1 = ixheaacd_mps_mult32(icc_delta, (mesh[0][1] - mesh[0][0]), &qtemp1, 0);
    qtemp = 15;
    p_aux_struct->ott_cld[0][ps][pb] =
        ixheaacd_mps_add32((mesh[0][0] << 15), temp_1, &qtemp, qtemp1);
    qtemp1 = 16;
    temp_1 = ixheaacd_mps_mult32(cld_delta, (mesh[1][0] - mesh[0][0]), &qtemp1, 0);
    p_aux_struct->ott_cld[0][ps][pb] =
        ixheaacd_mps_add32(p_aux_struct->ott_cld[0][ps][pb], temp_1, &qtemp, qtemp1);
    qtemp1 = q_icc_delta;
    temp_1 = ixheaacd_mps_mult32(icc_delta, cld_delta, &qtemp1, 16);
    temp_1 = ixheaacd_mps_mult32(temp_1, (mesh[1][1] - mesh[0][1] - mesh[1][0] + mesh[0][0]),
                                 &qtemp1, 0);
    p_aux_struct->ott_cld[0][ps][pb] =
        ixheaacd_mps_add32(p_aux_struct->ott_cld[0][ps][pb], temp_1, &qtemp, qtemp1);

    p_aux_struct->ott_cld[0][ps][pb] =
        ixheaacd_mps_add32(p_aux_struct->ott_cld[0][ps][pb], ONE_IN_Q14, &qtemp, 15);
    p_aux_struct->ott_cld[0][ps][pb] = dequant_cld[((p_aux_struct->ott_cld[0][ps][pb]) >> qtemp)];

    mesh[0][0] = p_blind_table->mesh_table.blind_icc_mesh[cld_index][icc_index];
    mesh[0][1] = p_blind_table->mesh_table.blind_icc_mesh[cld_index][icc_index + 1];
    mesh[1][0] = p_blind_table->mesh_table.blind_icc_mesh[cld_index + 1][icc_index];
    mesh[1][1] = p_blind_table->mesh_table.blind_icc_mesh[cld_index + 1][icc_index + 1];

    qtemp1 = q_icc_delta;
    temp_1 = ixheaacd_mps_mult32(icc_delta, (mesh[0][1] - mesh[0][0]), &qtemp1, 0);
    qtemp = 15;
    p_aux_struct->ott_icc[0][ps][pb] =
        ixheaacd_mps_add32((mesh[0][0] << 15), temp_1, &qtemp, qtemp1);
    qtemp1 = 16;
    temp_1 = ixheaacd_mps_mult32(cld_delta, (mesh[1][0] - mesh[0][0]), &qtemp1, 0);
    p_aux_struct->ott_icc[0][ps][pb] =
        ixheaacd_mps_add32(p_aux_struct->ott_icc[0][ps][pb], temp_1, &qtemp, qtemp1);
    qtemp1 = q_icc_delta;
    temp_1 = ixheaacd_mps_mult32(icc_delta, cld_delta, &qtemp1, 16);
    temp_1 = ixheaacd_mps_mult32(temp_1, (mesh[1][1] - mesh[0][1] - mesh[1][0] + mesh[0][0]),
                                 &qtemp1, 0);
    p_aux_struct->ott_icc[0][ps][pb] =
        ixheaacd_mps_add32(p_aux_struct->ott_icc[0][ps][pb], temp_1, &qtemp, qtemp1);

    p_aux_struct->ott_icc[0][ps][pb] =
        ixheaacd_mps_add32(p_aux_struct->ott_icc[0][ps][pb], ONE_IN_Q14, &qtemp, 15);
    p_aux_struct->ott_icc[0][ps][pb] = dequant_icc[((p_aux_struct->ott_icc[0][ps][pb]) >> qtemp)];

    mesh[0][0] = p_blind_table->mesh_table.blind_cpc_1_mesh[cld_index][icc_index] + 20;
    mesh[0][1] = p_blind_table->mesh_table.blind_cpc_1_mesh[cld_index][icc_index + 1] + 20;
    mesh[1][0] = p_blind_table->mesh_table.blind_cpc_1_mesh[cld_index + 1][icc_index] + 20;
    mesh[1][1] = p_blind_table->mesh_table.blind_cpc_1_mesh[cld_index + 1][icc_index + 1] + 20;

    qtemp1 = q_icc_delta;
    temp_1 = ixheaacd_mps_mult32(icc_delta, (mesh[0][1] - mesh[0][0]), &qtemp1, 0);
    qtemp = 15;
    p_aux_struct->ttt_cpc_1[0][ps][pb] =
        ixheaacd_mps_add32((mesh[0][0] << 15), temp_1, &qtemp, qtemp1);
    qtemp1 = 16;
    temp_1 = ixheaacd_mps_mult32(cld_delta, (mesh[1][0] - mesh[0][0]), &qtemp1, 0);
    p_aux_struct->ttt_cpc_1[0][ps][pb] =
        ixheaacd_mps_add32(p_aux_struct->ttt_cpc_1[0][ps][pb], temp_1, &qtemp, qtemp1);
    qtemp1 = q_icc_delta;
    temp_1 = ixheaacd_mps_mult32(icc_delta, cld_delta, &qtemp1, 16);
    temp_1 = ixheaacd_mps_mult32(temp_1, (mesh[1][1] - mesh[0][1] - mesh[1][0] + mesh[0][0]),
                                 &qtemp1, 0);
    p_aux_struct->ttt_cpc_1[0][ps][pb] =
        ixheaacd_mps_add32(p_aux_struct->ttt_cpc_1[0][ps][pb], temp_1, &qtemp, qtemp1);

    p_aux_struct->ttt_cpc_1[0][ps][pb] =
        ixheaacd_mps_add32(p_aux_struct->ttt_cpc_1[0][ps][pb], ONE_IN_Q14, &qtemp, 15);
    p_aux_struct->ttt_cpc_1[0][ps][pb] =
        dequant_cpc[((p_aux_struct->ttt_cpc_1[0][ps][pb]) >> qtemp)];

    mesh[0][0] = p_blind_table->mesh_table.blind_cpc_2_mesh[cld_index][icc_index] + 20;
    mesh[0][1] = p_blind_table->mesh_table.blind_cpc_2_mesh[cld_index][icc_index + 1] + 20;
    mesh[1][0] = p_blind_table->mesh_table.blind_cpc_2_mesh[cld_index + 1][icc_index] + 20;
    mesh[1][1] = p_blind_table->mesh_table.blind_cpc_2_mesh[cld_index + 1][icc_index + 1] + 20;

    qtemp1 = q_icc_delta;
    temp_1 = ixheaacd_mps_mult32(icc_delta, (mesh[0][1] - mesh[0][0]), &qtemp1, 0);
    qtemp = 15;
    p_aux_struct->ttt_cpc_2[0][ps][pb] =
        ixheaacd_mps_add32((mesh[0][0] < 15), temp_1, &qtemp, qtemp1);
    qtemp1 = 16;
    temp_1 = ixheaacd_mps_mult32(cld_delta, (mesh[1][0] - mesh[0][0]), &qtemp1, 0);
    p_aux_struct->ttt_cpc_2[0][ps][pb] =
        ixheaacd_mps_add32(p_aux_struct->ttt_cpc_2[0][ps][pb], temp_1, &qtemp, qtemp1);
    qtemp1 = q_icc_delta;
    temp_1 = ixheaacd_mps_mult32(icc_delta, cld_delta, &qtemp1, 16);
    temp_1 = ixheaacd_mps_mult32(temp_1, (mesh[1][1] - mesh[0][1] - mesh[1][0] + mesh[0][0]),
                                 &qtemp1, 0);
    p_aux_struct->ttt_cpc_2[0][ps][pb] =
        ixheaacd_mps_add32(p_aux_struct->ttt_cpc_2[0][ps][pb], temp_1, &qtemp, qtemp1);

    p_aux_struct->ttt_cpc_2[0][ps][pb] =
        ixheaacd_mps_add32(p_aux_struct->ttt_cpc_2[0][ps][pb], ONE_IN_Q14, &qtemp, 15);
    p_aux_struct->ttt_cpc_2[0][ps][pb] =
        dequant_cpc[((p_aux_struct->ttt_cpc_2[0][ps][pb]) >> qtemp)];

    if (cld < 0) {
      cld = p_aux_struct->ttt_cpc_2[0][ps][pb];
      p_aux_struct->ttt_cpc_2[0][ps][pb] = p_aux_struct->ttt_cpc_1[0][ps][pb];
      p_aux_struct->ttt_cpc_1[0][ps][pb] = cld;
    }
  }
}

static VOID ixheaacd_update_down_mix_state(ia_heaac_mps_state_struct *pstr_mps_state,
                                           WORD32 offset) {
  ia_mps_dec_blind_decoder_struct *blind = pstr_mps_state->mps_persistent_mem.blind_decoder;
  WORD32 ts;
  WORD32 hb;
  WORD32 pb;
  WORD32 *excitation_0, *excitation_1, *excitation_2;
  WORD16 *q_excitation_0, *q_excitation_1, *q_excitation_2;
  WORD32 temp_1, temp_2;
  WORD16 qtemp1, qtemp2;
  WORD32 *p_x_real, *p_x_imag;
  WORD32 num_parameter_bands = pstr_mps_state->num_parameter_bands;
  WORD32 hybrid_bands = pstr_mps_state->hybrid_bands;
  excitation_0 = pstr_mps_state->mps_scratch_mem_v;
  q_excitation_0 =
      (WORD16 *)pstr_mps_state->mps_scratch_mem_v +
      IXHEAAC_GET_SIZE_ALIGNED_TYPE(PARAMETER_BANDSX2, sizeof(*q_excitation_0), BYTE_ALIGN_8);
  excitation_1 = excitation_0 + IXHEAAC_GET_SIZE_ALIGNED_TYPE(
                                    PARAMETER_BANDSX1_5, sizeof(*excitation_1), BYTE_ALIGN_8);
  q_excitation_1 = q_excitation_0 + IXHEAAC_GET_SIZE_ALIGNED_TYPE(
                                        PARAMETER_BANDSX3, sizeof(*q_excitation_1), BYTE_ALIGN_8);
  excitation_2 = excitation_1 + IXHEAAC_GET_SIZE_ALIGNED_TYPE(
                                    PARAMETER_BANDSX1_5, sizeof(*excitation_2), BYTE_ALIGN_8);
  q_excitation_2 = q_excitation_1 + IXHEAAC_GET_SIZE_ALIGNED_TYPE(
                                        PARAMETER_BANDSX3, sizeof(*q_excitation_2), BYTE_ALIGN_8);

  p_x_real = &pstr_mps_state->array_struct->x_real[offset * MAX_HYBRID_BANDS];
  p_x_imag = &pstr_mps_state->array_struct->x_imag[offset * MAX_HYBRID_BANDS];
  for (ts = 0; ts < HOP_SLOTS; ts++) {
    WORD32 *x_real_0 = p_x_real;
    WORD32 *x_imag_0 = p_x_imag;

    WORD32 *x_real_1 = p_x_real + TSXHB;
    WORD32 *x_imag_1 = p_x_imag + TSXHB;

    for (pb = 0; pb < num_parameter_bands; pb++) {
      excitation_0[pb] = ABS_THR_FIX;
      excitation_1[pb] = ABS_THR_FIX;
      excitation_2[pb] = ABS_THR_FIX;

      q_excitation_0[pb] = 15;
      q_excitation_1[pb] = 15;
      q_excitation_2[pb] = 15;
    }

    for (hb = 0; hb < hybrid_bands; hb++) {
      WORD64 temp;
      pb = (WORD32)pstr_mps_state->kernels[hb];

      temp =
          (WORD64)((WORD64)*x_real_0 * (WORD64)*x_real_0 + (WORD64)*x_imag_0 * (WORD64)*x_imag_0);
      temp >>= 10;
      temp_1 = (WORD32)temp;
      qtemp1 = 10;
      excitation_0[pb] =
          ixheaacd_mps_add32(excitation_0[pb], temp_1, &(q_excitation_0[pb]), qtemp1);

      temp =
          (WORD64)((WORD64)*x_real_1 * (WORD64)*x_real_1 + (WORD64)*x_imag_1 * (WORD64)*x_imag_1);
      temp >>= 10;
      temp_1 = (WORD32)temp;
      qtemp1 = 10;
      excitation_1[pb] =
          ixheaacd_mps_add32(excitation_1[pb], temp_1, &(q_excitation_1[pb]), qtemp1);

      temp =
          (WORD64)((WORD64)*x_real_0 * (WORD64)*x_real_1 + (WORD64)*x_imag_0 * (WORD64)*x_imag_1);
      temp >>= 10;
      temp_1 = (WORD32)temp;
      qtemp1 = 10;
      excitation_2[pb] =
          ixheaacd_mps_add32(excitation_2[pb], temp_1, &(q_excitation_2[pb]), qtemp1);

      x_real_0++;
      x_imag_0++;
      x_real_1++;
      x_imag_1++;
    }

    for (pb = 0; pb < num_parameter_bands; pb++) {
      blind->excitation[0][pb] =
          ixheaacd_mps_mult32_shr_15(blind->excitation[0][pb], blind->filter_coeff);
      blind->excitation[1][pb] =
          ixheaacd_mps_mult32_shr_15(blind->excitation[1][pb], blind->filter_coeff);
      blind->excitation[2][pb] =
          ixheaacd_mps_mult32_shr_15(blind->excitation[2][pb], blind->filter_coeff);

      temp_1 = ONE_IN_Q15 - blind->filter_coeff;

      qtemp2 = *q_excitation_0++;
      temp_2 = ixheaacd_mps_mult32_shr_15(temp_1, *excitation_0);
      excitation_0++;

      blind->excitation[0][pb] = ixheaacd_mps_add32(blind->excitation[0][pb], temp_2,
                                                    &(blind->q_excitation[0][pb]), qtemp2);

      qtemp2 = *q_excitation_1++;
      temp_2 = ixheaacd_mps_mult32_shr_15(temp_1, *excitation_1);
      excitation_1++;

      blind->excitation[1][pb] = ixheaacd_mps_add32(blind->excitation[1][pb], temp_2,
                                                    &(blind->q_excitation[1][pb]), qtemp2);

      qtemp2 = *q_excitation_2++;
      temp_2 = ixheaacd_mps_mult32_shr_15(temp_1, *excitation_2);
      excitation_2++;

      blind->excitation[2][pb] = ixheaacd_mps_add32(blind->excitation[2][pb], temp_2,
                                                    &(blind->q_excitation[2][pb]), qtemp2);
    }
    excitation_0[pb] -= num_parameter_bands;
    excitation_1[pb] -= num_parameter_bands;
    excitation_2[pb] -= num_parameter_bands;

    q_excitation_0[pb] -= num_parameter_bands;
    q_excitation_1[pb] -= num_parameter_bands;
    q_excitation_2[pb] -= num_parameter_bands;

    p_x_real += MAX_HYBRID_BANDS;
    p_x_imag += MAX_HYBRID_BANDS;
  }
}

VOID ixheaacd_apply_blind(ia_heaac_mps_state_struct *pstr_mps_state) {
  ia_mps_dec_spatial_bs_frame_struct *frame = pstr_mps_state->bs_frame;
  WORD32 ts;
  WORD32 ps;
  WORD32 time_slots = pstr_mps_state->time_slots;
  WORD32 *param_slot = pstr_mps_state->aux_struct->param_slot;

  for (ts = 0, ps = 0; ts < time_slots; ts += HOP_SLOTS, ps++) {
    param_slot[ps] = ts + HOP_SLOTS - 1;

    ixheaacd_signal_2_parameters(pstr_mps_state, ps);
    ixheaacd_update_down_mix_state(pstr_mps_state, ts);
  }

  pstr_mps_state->num_parameter_sets_prev = ps;
  pstr_mps_state->num_parameter_sets = ps;
  frame->bs_independency_flag = 0;
  pstr_mps_state->aux_struct->num_ott_bands[0] = pstr_mps_state->num_parameter_bands;

  return;
}
