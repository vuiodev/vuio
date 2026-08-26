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
#include <math.h>
#include "ixheaac_type_def.h"
#include "impd_drc_common_enc.h"
#include "impd_drc_uni_drc.h"
#include "impd_drc_tables.h"
#include "impd_drc_api.h"
#include "ixheaace_api.h"
#include "ixheaac_error_standards.h"
#include "ixheaace_error_codes.h"
#include "ixheaace_mps_common_fix.h"
#include "ixheaace_mps_defines.h"
#include "ixheaace_bitbuffer.h"
#include "ixheaace_mps_common_define.h"
#include "ixheaace_mps_param_extract.h"
#include "ixheaace_mps_buf.h"
#include "ixheaace_mps_lib.h"
#include "ixheaace_mps_bitstream.h"
#include "ixheaace_mps_struct_def.h"
#include "ixheaace_mps_sac_polyphase.h"
#include "ixheaace_mps_sac_nlc_enc.h"
#include "ixheaace_mps_sac_hybfilter.h"
#include "ixheaace_mps_spatial_bitstream.h"
#include "ixheaace_mps_tree.h"
#include "ixheaace_mps_rom.h"
#include "ixheaace_mps_vector_functions.h"
#include "ixheaac_constants.h"
#include "ixheaace_aac_constants.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaac_basic_ops.h"

static UWORD8 ixheaace_mps_212_get_icc_correlation_coherence_border(
    const WORD32 aot, const WORD32 use_coherence_only) {
  ixheaace_mps_box_subband_setup pstr_box_subband_setup;

  if (aot == AOT_AAC_ELD) {
    pstr_box_subband_setup.p_subband_2_parameter_index_ld = (UWORD8 *)subband_2_parameter_ld;
    pstr_box_subband_setup.icc_correlation_coherence_border = 5;
  } else {
    pstr_box_subband_setup.p_subband_2_parameter_index_ld = (UWORD8 *)subband_2_parameter_usac;
    pstr_box_subband_setup.icc_correlation_coherence_border = 8;
  }

  return ((use_coherence_only) ? 0 : pstr_box_subband_setup.icc_correlation_coherence_border);
}

static VOID ixheaace_mps_212_calc_correlation_vec(FLOAT32 *const data,
                                                  const FLOAT32 *const data_real,
                                                  const FLOAT32 *const power_data_1,
                                                  const FLOAT32 *const power_data_2,
                                                  const WORD32 icc_correlation_coherence_border) {
  WORD32 idx;
  FLOAT32 p_12;

  for (idx = 0; idx < icc_correlation_coherence_border; idx++) {
    p_12 = power_data_1[idx] * power_data_2[idx];
    if (p_12 > 0.0f) {
      p_12 = 1.0f / ((FLOAT32)sqrt(p_12));
      data[idx] = data_real[idx] * p_12;
    } else {
      data[idx] = 0.9995f;
    }
  }
}

static VOID ixheaace_mps_212_calc_coherence_vec(FLOAT32 *const data,
                                                const FLOAT32 *const data_real,
                                                const FLOAT32 *const data_imag,
                                                const FLOAT32 *const power_data_1,
                                                const FLOAT32 *const power_data_2,
                                                const WORD32 icc_correlation_coherence_border) {
  WORD32 idx;
  FLOAT32 coh, p_12, p_12_ri;

  for (idx = 0; idx < icc_correlation_coherence_border; idx++) {
    p_12_ri = (FLOAT32)(sqrt(data_real[idx] * data_real[idx] + data_imag[idx] * data_imag[idx]));
    p_12 = power_data_1[idx] * power_data_2[idx];

    if (p_12 > 0.0f) {
      p_12 = 1.0f / ((FLOAT32)sqrt(p_12));
      coh = p_12_ri * p_12;
      data[idx] = coh;
    } else {
      data[idx] = 0.9995f;
    }
  }
}

static VOID ixheaace_mps_212_quantize_coef(const FLOAT32 *const input, const WORD32 num_bands,
                                           const FLOAT32 *const quant_table,
                                           const WORD32 idx_offset, const WORD32 num_quant_steps,
                                           WORD8 *const quant_out) {
  WORD32 band;
  WORD32 forward = (quant_table[1] >= quant_table[0]);

  for (band = 0; band < num_bands; band++) {
    FLOAT32 q_val;
    FLOAT32 cur_val = input[band];
    WORD32 upper = num_quant_steps - 1;
    WORD32 lower = 0;
    if (forward) {
      while (upper - lower > 1) {
        WORD32 idx = (lower + upper) >> 1;
        q_val = quant_table[idx];
        if (cur_val <= q_val) {
          upper = idx;
        } else {
          lower = idx;
        }
      }

      if ((cur_val - quant_table[lower]) > (quant_table[upper] - cur_val)) {
        quant_out[band] = (WORD8)(upper - idx_offset);
      } else {
        quant_out[band] = (WORD8)(lower - idx_offset);
      }
    } else {
      while (upper - lower > 1) {
        WORD32 idx = (lower + upper) >> 1;
        q_val = quant_table[idx];
        if (cur_val >= q_val) {
          upper = idx;
        } else {
          lower = idx;
        }
      }

      if ((cur_val - quant_table[lower]) < (quant_table[upper] - cur_val)) {
        quant_out[band] = (WORD8)(upper - idx_offset);
      } else {
        quant_out[band] = (WORD8)(lower - idx_offset);
      }
    }
    if (quant_out[band] != 0) {
      quant_out[band] = quant_out[band];
    }
  }
}

VOID ixheaace_mps_212_calc_parameter_band_to_hybrid_band_offset(
    const WORD32 num_hybrid_bands, UWORD8 *ptr_parameter_band_2_hybrid_band_offset, WORD32 aot) {
  ixheaace_mps_box_subband_setup pstr_box_subband_setup;
  if (aot == AOT_AAC_ELD) {
    pstr_box_subband_setup.p_subband_2_parameter_index_ld = (UWORD8 *)subband_2_parameter_ld;
    pstr_box_subband_setup.icc_correlation_coherence_border = 5;
  } else {
    pstr_box_subband_setup.p_subband_2_parameter_index_ld = (UWORD8 *)subband_2_parameter_usac;
    pstr_box_subband_setup.icc_correlation_coherence_border = 8;
  }

  const UWORD8 *p_subband_2_parameter_index;

  UWORD8 idx;
  WORD32 band;

  p_subband_2_parameter_index = pstr_box_subband_setup.p_subband_2_parameter_index_ld;

  for (band = 0, idx = NUM_QMF_BANDS - 1; idx > NUM_QMF_BANDS - num_hybrid_bands; idx--) {
    if (p_subband_2_parameter_index[idx - 1] - p_subband_2_parameter_index[idx]) {
      ptr_parameter_band_2_hybrid_band_offset[band++] = (NUM_QMF_BANDS - idx);
    }
  }
  ptr_parameter_band_2_hybrid_band_offset[band++] = (NUM_QMF_BANDS - idx);
}

IA_ERRORCODE
ixheaace_mps_212_init_tto_box(ixheaace_mps_pstr_tto_box pstr_tto_box,
                              const ixheaace_mps_tto_box_config *const pstr_tto_box_config,
                              UWORD8 *ptr_parameter_band_2_hybrid_band_offset, WORD32 aot) {
  IA_ERRORCODE error = IA_NO_ERROR;

  {
    memset(pstr_tto_box, 0, sizeof(ixheaace_mps_tto_box));
    pstr_tto_box->use_coarse_quant_cld_flag = pstr_tto_box_config->use_coarse_quant_cld_flag;
    pstr_tto_box->use_coarse_quant_icc_flag = pstr_tto_box_config->use_coarse_quant_icc_flag;
    pstr_tto_box->box_quant_mode = pstr_tto_box_config->box_quant_mode;
    pstr_tto_box->icc_correlation_coherence_border =
        ixheaace_mps_212_get_icc_correlation_coherence_border(
            aot, pstr_tto_box_config->b_use_coherence_icc_only);
    pstr_tto_box->num_hybrid_bands_max = pstr_tto_box_config->num_hybrid_bands_max;
    if (aot == 39) {
      pstr_tto_box->num_parameter_bands = IXHEAACE_MPS_SAC_BANDS_ld;
    } else {
      pstr_tto_box->num_parameter_bands = IXHEAACE_MPS_SAC_BANDS_usac;
    }
    pstr_tto_box->ptr_parameter_band_2_hybrid_band_offset =
        ptr_parameter_band_2_hybrid_band_offset;
    pstr_tto_box->frame_keep_flag = pstr_tto_box_config->frame_keep_flag;
    pstr_tto_box->n_cld_quant_steps = pstr_tto_box->use_coarse_quant_cld_flag
                                          ? IXHEAACE_MPS_MAX_CLD_QUANT_COARSE
                                          : IXHEAACE_MPS_MAX_CLD_QUANT_FINE;
    pstr_tto_box->n_cld_quant_offset = pstr_tto_box->use_coarse_quant_cld_flag
                                           ? IXHEAACE_MPS_OFFSET_CLD_QUANT_COARSE
                                           : IXHEAACE_MPS_OFFSET_CLD_QUANT_FINE;
    pstr_tto_box->n_icc_quant_steps = pstr_tto_box->use_coarse_quant_icc_flag
                                          ? IXHEAACE_MPS_MAX_ICC_QUANT_COARSE
                                          : IXHEAACE_MPS_MAX_ICC_QUANT_FINE;
    pstr_tto_box->n_icc_quant_offset = pstr_tto_box->use_coarse_quant_icc_flag
                                           ? IXHEAACE_MPS_OFFSET_ICC_QUANT_COARSE
                                           : IXHEAACE_MPS_OFFSET_ICC_QUANT_FINE;
    pstr_tto_box->p_icc_quant_table =
        pstr_tto_box->use_coarse_quant_icc_flag ? icc_quant_table_coarse : icc_quant_table_fine;
    pstr_tto_box->p_cld_quant_table_enc = pstr_tto_box->use_coarse_quant_cld_flag
                                              ? cld_quant_table_coarse_enc
                                              : cld_quant_table_fine_enc;

    if ((pstr_tto_box->box_quant_mode != IXHEAACE_MPS_QUANTMODE_FINE) &&
        (pstr_tto_box->box_quant_mode != IXHEAACE_MPS_QUANTMODE_EBQ1) &&
        (pstr_tto_box->box_quant_mode != IXHEAACE_MPS_QUANTMODE_EBQ2)) {
      return IA_EXHEAACE_INIT_FATAL_MPS_INIT_FAILED;
    }
  }
  return error;
}

IA_ERRORCODE ixheaace_mps_212_apply_tto_box(
    ixheaace_mps_pstr_tto_box pstr_tto_box, const WORD32 num_time_slots,
    const WORD32 start_time_slot, const WORD32 num_hybrid_bands,
    ixheaace_cmplx_str pp_hybrid_data_1[MAX_ANA_TIME_SLOT][MAX_QMF_BANDS],
    ixheaace_cmplx_str pp_hybrid_data_2[MAX_ANA_TIME_SLOT][MAX_QMF_BANDS], WORD8 *const ptr_icc,
    UWORD8 *const pb_icc_quant_coarse, WORD8 *const ptr_cld, UWORD8 *const pb_cld_quant_coarse,
    const WORD32 b_use_bb_cues) {
  IA_ERRORCODE error = IA_NO_ERROR;

  WORD32 j, band;
  FLOAT32 power_hybrid_data_1[MAX_NUM_PARAM_BANDS] = {0};
  FLOAT32 power_hybrid_data_2[MAX_NUM_PARAM_BANDS] = {0};
  FLOAT32 prod_hybrid_data_real[MAX_NUM_PARAM_BANDS] = {0};
  FLOAT32 prod_hybrid_data_imag[MAX_NUM_PARAM_BANDS] = {0};
  const WORD32 num_param_bands = pstr_tto_box->num_parameter_bands;
  const WORD32 use_box_quant_mode =
      (pstr_tto_box->box_quant_mode == IXHEAACE_MPS_QUANTMODE_EBQ1) ||
      (pstr_tto_box->box_quant_mode == IXHEAACE_MPS_QUANTMODE_EBQ2);

  if ((num_hybrid_bands < 0) || (num_hybrid_bands > pstr_tto_box->num_hybrid_bands_max)) {
    return IA_EXHEAACE_CONFIG_NONFATAL_MPS_INVALID_CONFIG;
  }

  for (j = 0, band = 0; band < num_param_bands; band++) {
    FLOAT32 data_1, data_2;
    data_1 = 0;
    data_2 = 0;
    for (; j < pstr_tto_box->ptr_parameter_band_2_hybrid_band_offset[band]; j++) {
      data_1 += ixheaace_mps_212_sum_up_cplx_pow_2_dim_2(pp_hybrid_data_1, start_time_slot,
                                                         num_time_slots, j, j + 1);
      data_2 += ixheaace_mps_212_sum_up_cplx_pow_2_dim_2(pp_hybrid_data_2, start_time_slot,
                                                         num_time_slots, j, j + 1);
    }

    power_hybrid_data_1[band] = data_1;
    power_hybrid_data_2[band] = data_2;
  }
  for (j = 0, band = 0; band < num_param_bands; band++) {
    FLOAT32 data_real, data_imag;
    data_real = data_imag = 0;
    for (; j < pstr_tto_box->ptr_parameter_band_2_hybrid_band_offset[band]; j++) {
      ixheaace_cmplx_str scalar_prod;
      ixheaace_mps_212_cplx_scalar_product(&scalar_prod, pp_hybrid_data_1, pp_hybrid_data_2,
                                           start_time_slot, num_time_slots, j, j + 1);
      data_real += scalar_prod.re;
      data_imag += scalar_prod.im;
    }
    prod_hybrid_data_real[band] = data_real;
    prod_hybrid_data_imag[band] = data_imag;
  }

  ixheaace_mps_212_calc_correlation_vec(pstr_tto_box->icc, prod_hybrid_data_real,
                                        power_hybrid_data_1, power_hybrid_data_2,
                                        pstr_tto_box->icc_correlation_coherence_border);

  ixheaace_mps_212_calc_coherence_vec(
      &pstr_tto_box->icc[pstr_tto_box->icc_correlation_coherence_border],
      &prod_hybrid_data_real[pstr_tto_box->icc_correlation_coherence_border],
      &prod_hybrid_data_imag[pstr_tto_box->icc_correlation_coherence_border],
      &power_hybrid_data_1[pstr_tto_box->icc_correlation_coherence_border],
      &power_hybrid_data_2[pstr_tto_box->icc_correlation_coherence_border],
      num_param_bands - pstr_tto_box->icc_correlation_coherence_border);
  if (error) {
    return error;
  }
  if (!use_box_quant_mode) {
    FLOAT32 power_1, power_2, cld;
    FLOAT32 max_pow = 30.0f;

    for (band = 0; band < num_param_bands; band++) {
      power_1 = (FLOAT32)log(power_hybrid_data_1[band] / 2.0f);
      power_2 = (FLOAT32)log(power_hybrid_data_2[band] / 2.0f);
      power_1 = MAX(MIN(power_1, max_pow), -max_pow);
      power_2 = MAX(MIN(power_2, max_pow), -max_pow);
      cld = (INV_LN_10_10 * (power_1 - power_2));
      pstr_tto_box->cld[band] = cld;
    }
  }

  if (b_use_bb_cues) {
    FLOAT32 temp;
    temp = 0;
    for (band = 0; band < num_param_bands; band++) {
      temp += pstr_tto_box->cld[band];
    }
    temp /= num_param_bands;
    for (band = 0; band < num_param_bands; band++) {
      pstr_tto_box->cld[band] = temp;
    }
  }

  ixheaace_mps_212_quantize_coef(
      pstr_tto_box->icc, num_param_bands, pstr_tto_box->p_icc_quant_table,
      pstr_tto_box->n_icc_quant_offset, pstr_tto_box->n_icc_quant_steps, ptr_icc);

  *pb_icc_quant_coarse = pstr_tto_box->use_coarse_quant_icc_flag;

  if (!use_box_quant_mode) {
    ixheaace_mps_212_quantize_coef(
        pstr_tto_box->cld, num_param_bands, pstr_tto_box->p_cld_quant_table_enc,
        pstr_tto_box->n_cld_quant_offset, pstr_tto_box->n_cld_quant_steps, ptr_cld);
  } else {
    memset(ptr_cld, 0, num_param_bands * sizeof(WORD8));
  }
  *pb_cld_quant_coarse = pstr_tto_box->use_coarse_quant_cld_flag;

  return error;
}
