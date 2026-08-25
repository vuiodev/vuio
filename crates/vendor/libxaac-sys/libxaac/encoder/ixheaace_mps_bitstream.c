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

#include <stdlib.h>
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
#include "ixheaace_mps_common_define.h"
#include "ixheaace_bitbuffer.h"

#include "ixheaace_mps_nlc_enc.h"
#include "ixheaac_constants.h"
#include "ixheaace_aac_constants.h"
#include "ixheaace_common_rom.h"

#include "ixheaace_mps_struct_def.h"
#include "ixheaace_mps_sac_polyphase.h"
#include "ixheaace_mps_sac_nlc_enc.h"
#include "ixheaace_mps_sac_hybfilter.h"
#include "ixheaace_mps_bitstream.h"
#include "ixheaace_mps_spatial_bitstream.h"
#include "ixheaace_mps_param_extract.h"
#include "ixheaace_mps_tree.h"
#include "ixheaace_mps_rom.h"

static UWORD8 ixheaace_mps_212_get_bs_freq_res_stride(const WORD32 index) {
  WORD32 freq_res_stride_table_size = 0;
  const UWORD8 *ptr_freq_res_stride_table = NULL;

  ptr_freq_res_stride_table = freq_res_stride_table_212;
  freq_res_stride_table_size =
      sizeof(freq_res_stride_table_212) / sizeof(*freq_res_stride_table_212);

  return (((index >= 0) && (index < freq_res_stride_table_size))
              ? ptr_freq_res_stride_table[index]
              : 1);
}

static IA_ERRORCODE ixheaace_mps_212_get_bs_freq_res_index(const WORD32 num_bands,
                                                           WORD32 *const ptr_bs_freq_res_index,
                                                           WORD32 aot) {
  IA_ERRORCODE error = IA_NO_ERROR;
  WORD32 idx;
  const UWORD8 *p_freq_res_bin_table;
  *ptr_bs_freq_res_index = -1;

  if (aot == AOT_AAC_ELD) {
    p_freq_res_bin_table = freq_res_bin_table_ld;
  } else {
    p_freq_res_bin_table = freq_res_bin_table_usac;
  }
  for (idx = 0; idx < MAX_FREQ_RES_INDEX; idx++) {
    if (num_bands == p_freq_res_bin_table[idx]) {
      *ptr_bs_freq_res_index = idx;
      break;
    }
  }
  if (*ptr_bs_freq_res_index < 0 || *ptr_bs_freq_res_index >= MAX_FREQ_RES_INDEX) {
    return IA_EXHEAACE_CONFIG_NONFATAL_MPS_INVALID_CONFIG;
  }
  return error;
}

static VOID ixheaace_mps_212_get_sampling_frequency_index(
    const UWORD32 bs_sampling_frequency, WORD32 *const ptr_bs_sampling_frequency_index) {
  WORD32 idx;
  *ptr_bs_sampling_frequency_index = SAMPLING_FREQUENCY_INDEX_ESCAPE;

  for (idx = 0; idx < MAX_SAMPLING_FREQUENCY_INDEX; idx++) {
    if (bs_sampling_frequency == ia_sampl_freq_table[idx]) {
      *ptr_bs_sampling_frequency_index = idx;
      break;
    }
  }
}

static IA_ERRORCODE ixheaace_mps_212_ec_data(
    ixheaace_bit_buf_handle pstr_bit_buf, WORD8 data[MAX_NUM_PARAMS][MAX_NUM_BINS],
    WORD8 old_data[MAX_NUM_BINS], UWORD8 quant_coarse_xxx_prev[MAX_NUM_PARAMS],
    ixheaace_mps_lossless_data *const pstr_lossless_data, const WORD32 data_type,
    const WORD32 param_idx, const WORD32 num_param_sets, const WORD32 independency_flag,
    const WORD32 start_band, const WORD32 stop_band, const WORD32 default_value) {
  IA_ERRORCODE error;
  WORD32 ps, pb, str_offset, pb_stride, i;
  WORD16 data_bands;
  WORD32 a_strides[MAX_NUM_BINS + 1] = {0};
  WORD16 cmp_idx_data[2][MAX_NUM_BINS] = {{0}};
  WORD16 cmp_old_data[MAX_NUM_BINS] = {0};

  if (num_param_sets > MAX_NUM_PARAMS) {
    return IA_EXHEAACE_EXE_FATAL_MPS_INVALID_NUM_PARAM_SETS;
  }

  if (independency_flag || (pstr_lossless_data->bs_quant_coarse_xxx[param_idx][0] !=
                            quant_coarse_xxx_prev[param_idx])) {
    pstr_lossless_data->bs_xxx_data_mode[param_idx][0] = IXHEAACE_MPS_DATA_MODE_FINECOARSE;
  } else {
    pstr_lossless_data->bs_xxx_data_mode[param_idx][0] = IXHEAACE_MPS_DATA_MODE_KEEP;
    for (i = start_band; i < stop_band; i++) {
      if (data[0][i] != old_data[i]) {
        pstr_lossless_data->bs_xxx_data_mode[param_idx][0] = IXHEAACE_MPS_DATA_MODE_FINECOARSE;
        break;
      }
    }
  }

  ixheaace_write_bits(pstr_bit_buf, pstr_lossless_data->bs_xxx_data_mode[param_idx][0], 2);

  for (ps = 1; ps < num_param_sets; ps++) {
    if (pstr_lossless_data->bs_quant_coarse_xxx[param_idx][ps] !=
        pstr_lossless_data->bs_quant_coarse_xxx[param_idx][ps - 1]) {
      pstr_lossless_data->bs_xxx_data_mode[param_idx][ps] = IXHEAACE_MPS_DATA_MODE_FINECOARSE;
    } else {
      pstr_lossless_data->bs_xxx_data_mode[param_idx][ps] = IXHEAACE_MPS_DATA_MODE_KEEP;
      for (i = start_band; i < stop_band; i++) {
        if (data[ps][i] != data[ps - 1][i]) {
          pstr_lossless_data->bs_xxx_data_mode[param_idx][ps] = IXHEAACE_MPS_DATA_MODE_FINECOARSE;
          break;
        }
      }
    }

    ixheaace_write_bits(pstr_bit_buf, pstr_lossless_data->bs_xxx_data_mode[param_idx][ps], 2);
  }

  for (ps = 0; ps < (num_param_sets - 1); ps++) {
    if (pstr_lossless_data->bs_xxx_data_mode[param_idx][ps] ==
        IXHEAACE_MPS_DATA_MODE_FINECOARSE) {
      if (pstr_lossless_data->bs_xxx_data_mode[param_idx][ps + 1] ==
          IXHEAACE_MPS_DATA_MODE_FINECOARSE) {
        if ((pstr_lossless_data->bs_quant_coarse_xxx[param_idx][ps + 1] ==
             pstr_lossless_data->bs_quant_coarse_xxx[param_idx][ps]) &&
            (pstr_lossless_data->bs_freq_res_stride_xxx[param_idx][ps + 1] ==
             pstr_lossless_data->bs_freq_res_stride_xxx[param_idx][ps])) {
          pstr_lossless_data->bs_data_pair[param_idx][ps] = 1;
          pstr_lossless_data->bs_data_pair[param_idx][ps + 1] = 1;
          ps++;
          continue;
        }
      }

      pstr_lossless_data->bs_data_pair[param_idx][ps] = 0;

      pstr_lossless_data->bs_data_pair[param_idx][ps + 1] = 0;
    } else {
      pstr_lossless_data->bs_data_pair[param_idx][ps] = 0;
      pstr_lossless_data->bs_data_pair[param_idx][ps + 1] = 0;
    }
  }

  for (ps = 0; ps < num_param_sets; ps++) {
    if (pstr_lossless_data->bs_xxx_data_mode[param_idx][ps] == IXHEAACE_MPS_DATA_MODE_DEFAULT) {
      for (i = start_band; i < stop_band; i++) {
        old_data[i] = (WORD8)default_value;
      }
      quant_coarse_xxx_prev[param_idx] = 0;
    }

    if (pstr_lossless_data->bs_xxx_data_mode[param_idx][ps] ==
        IXHEAACE_MPS_DATA_MODE_FINECOARSE) {
      ixheaace_write_bits(pstr_bit_buf, pstr_lossless_data->bs_data_pair[param_idx][ps], 1);
      ixheaace_write_bits(pstr_bit_buf, pstr_lossless_data->bs_quant_coarse_xxx[param_idx][ps],
                          1);
      ixheaace_write_bits(pstr_bit_buf, pstr_lossless_data->bs_freq_res_stride_xxx[param_idx][ps],
                          2);

      if (pstr_lossless_data->bs_quant_coarse_xxx[param_idx][ps] !=
          quant_coarse_xxx_prev[param_idx]) {
        if (quant_coarse_xxx_prev[param_idx]) {
          for (i = start_band; i < stop_band; i++) {
            old_data[i] *= 2;
          }

          if (data_type == IXHEAACE_MPS_SAC_DATA_TYPE_CLD) {
            for (i = start_band; i < stop_band; i++) {
              if (old_data[i] == -14) {
                old_data[i] = -15;
              } else if (old_data[i] == 14) {
                old_data[i] = 15;
              }
            }
          }
        } else {
          for (i = start_band; i < stop_band; i++) {
            old_data[i] /= 2;
          }
        }
      }

      pb_stride = ixheaace_mps_212_get_bs_freq_res_stride(
          pstr_lossless_data->bs_freq_res_stride_xxx[param_idx][ps]);
      data_bands = (WORD16)((stop_band - start_band - 1) / pb_stride + 1);

      a_strides[0] = start_band;
      for (pb = 1; pb <= data_bands; pb++) {
        a_strides[pb] = a_strides[pb - 1] + pb_stride;
      }

      str_offset = 0;
      while (a_strides[data_bands] > stop_band) {
        if (str_offset < data_bands) {
          str_offset++;
        }
        for (i = str_offset; i <= data_bands; i++) {
          a_strides[i]--;
        }
      }

      for (pb = 0; pb < data_bands; pb++) {
        cmp_old_data[start_band + pb] = old_data[a_strides[pb]];
        cmp_idx_data[0][start_band + pb] = data[ps][a_strides[pb]];

        if (pstr_lossless_data->bs_data_pair[param_idx][ps]) {
          cmp_idx_data[1][start_band + pb] = data[ps + 1][a_strides[pb]];
        }
      }

      if (pstr_lossless_data->bs_data_pair[param_idx][ps]) {
        error = ixheaace_mps_212_ec_data_pair_enc(
            pstr_bit_buf, cmp_idx_data, cmp_old_data, data_type, 0, start_band, data_bands,
            pstr_lossless_data->bs_quant_coarse_xxx[param_idx][ps],
            independency_flag && (ps == 0));
        if (error != IA_NO_ERROR) {
          return error;
        }
      } else {
        error = ixheaace_mps_212_ec_data_single_enc(
            pstr_bit_buf, cmp_idx_data, cmp_old_data, data_type, 0, start_band, data_bands,
            pstr_lossless_data->bs_quant_coarse_xxx[param_idx][ps],
            independency_flag && (ps == 0));
        if (error != IA_NO_ERROR) {
          return error;
        }
      }
      for (i = start_band; i < stop_band; i++) {
        if (pstr_lossless_data->bs_data_pair[param_idx][ps]) {
          old_data[i] = data[ps + 1][i];
        } else {
          old_data[i] = data[ps][i];
        }
      }

      quant_coarse_xxx_prev[param_idx] = pstr_lossless_data->bs_quant_coarse_xxx[param_idx][ps];

      if (pstr_lossless_data->bs_data_pair[param_idx][ps]) {
        ps++;
      }
    }
  }
  return IA_NO_ERROR;
}

static VOID ixheaace_mps_212_write_smg_data(ixheaace_bit_buf_handle pstr_bit_buf,
                                            const ixheaace_mps_smg_data *const pstr_smg_data,
                                            const WORD32 num_param_sets,
                                            const WORD32 data_bands) {
  WORD32 param, band;

  for (param = 0; param < num_param_sets; param++) {
    ixheaace_write_bits(pstr_bit_buf, pstr_smg_data->bs_smooth_mode[param], 2);
    if (pstr_smg_data->bs_smooth_mode[param] >= 2) {
      ixheaace_write_bits(pstr_bit_buf, pstr_smg_data->bs_smooth_time[param], 2);
    }
    if (pstr_smg_data->bs_smooth_mode[param] == 3) {
      WORD32 stride =
          ixheaace_mps_212_get_bs_freq_res_stride(pstr_smg_data->bs_freq_res_stride[param]);
      ixheaace_write_bits(pstr_bit_buf, pstr_smg_data->bs_freq_res_stride[param], 2);
      for (band = 0; band < data_bands; band += stride) {
        ixheaace_write_bits(pstr_bit_buf, pstr_smg_data->bs_smg_data[param][band], 1);
      }
    }
  }
}

static IA_ERRORCODE ixheaace_mps_212_write_ott_data(
    ixheaace_bit_buf_handle pstr_bit_buf, ixheaace_mps_prev_ott_data *const pstr_prev_ott_data,
    ixheaace_mps_ott_data *const pstr_ott_data,
    const ixheaace_mps_ott_config ott_config[IXHEAACE_MPS_MAX_NUM_BOXES],
    ixheaace_mps_lossless_data *const pstr_cld_lossless_data,
    ixheaace_mps_lossless_data *const pstr_icc_lossless_data, const WORD32 num_ott_boxes,
    const WORD32 num_bands, const WORD32 num_param_sets, const WORD32 bs_independency_flag) {
  IA_ERRORCODE error = IA_NO_ERROR;
  WORD32 box;
  for (box = 0; box < num_ott_boxes; box++) {
    error = ixheaace_mps_212_ec_data(
        pstr_bit_buf, pstr_ott_data->cld[box], pstr_prev_ott_data->cld_old[box],
        pstr_prev_ott_data->quant_coarse_cld_prev[box], pstr_cld_lossless_data,
        IXHEAACE_MPS_SAC_DATA_TYPE_CLD, box, num_param_sets, bs_independency_flag, 0,
        ott_config[box].bs_ott_bands, 15);
    if (error != IA_NO_ERROR) {
      return error;
    }
  }
  for (box = 0; box < num_ott_boxes; box++) {
    error = ixheaace_mps_212_ec_data(pstr_bit_buf, pstr_ott_data->icc[box],
                                     pstr_prev_ott_data->icc_old[box],
                                     pstr_prev_ott_data->quant_coarse_icc_prev[box],
                                     pstr_icc_lossless_data, IXHEAACE_MPS_SAC_DATA_TYPE_ICC, box,
                                     num_param_sets, bs_independency_flag, 0, num_bands, 0);
    if (error != IA_NO_ERROR) {
      return error;
    }
  }

  return IA_NO_ERROR;
}

static VOID ixheaace_mps_212_write_framing_info(
    ixheaace_bit_buf_handle pstr_bit_buf,
    const ixheaace_mps_framing_info *const pstr_framing_info, const WORD32 frame_length) {
  ixheaace_write_bits(pstr_bit_buf, pstr_framing_info->bs_framing_type, 1);
  ixheaace_write_bits(pstr_bit_buf, pstr_framing_info->num_param_sets - 1, 1);

  if (pstr_framing_info->bs_framing_type) {
    WORD32 ps = 0;
    UWORD8 bits_param_slot = 0;
    WORD32 num_param_sets = pstr_framing_info->num_param_sets;
    while ((1 << bits_param_slot) < (frame_length + 1)) {
      bits_param_slot++;
    }
    if (bits_param_slot > 0) {
      for (ps = 0; ps < num_param_sets; ps++) {
        ixheaace_write_bits(pstr_bit_buf, pstr_framing_info->bs_param_slots[ps], bits_param_slot);
      }
    }
  }
}

static IA_ERRORCODE ixheaace_mps_515_ec_data(ixheaace_bit_buf_handle pstr_bit_buf,
                                             WORD32 data[MAX_NUM_PARAMS][MAX_NUM_BINS],
                                             WORD32 old_data[MAX_NUM_BINS],
                                             ixheaace_mps_sac_lossless_data *pstr_lossless_data,
                                             WORD32 data_type, WORD32 param_idx,
                                             WORD32 num_param_sets, WORD32 independency_flag,
                                             WORD32 start_band, WORD32 stop_band) {
  WORD32 param_set, set_idx, pb_stride, data_bands, bin, data_sets;
  WORD32 param_set_index[MAX_NUM_PARAMS] = {0};

  for (param_set = 0; param_set < num_param_sets; param_set++) {
    pstr_lossless_data->bs_xxx_data_mode[param_idx][param_set] =
        IXHEAACE_MPS_DATA_MODE_FINECOARSE;
    ixheaace_write_bits(pstr_bit_buf, pstr_lossless_data->bs_xxx_data_mode[param_idx][param_set],
                        2);
  }

  for (param_set = 0, set_idx = 0; param_set < num_param_sets; param_set++) {
    param_set_index[set_idx] = param_set;
    if (param_set != num_param_sets - 1) {
      pstr_lossless_data->bs_data_pair[param_idx][set_idx] = 1;
      pstr_lossless_data->bs_data_pair[param_idx][set_idx + 1] = 1;
      param_set++;
      set_idx += 2;
    } else {
      pstr_lossless_data->bs_data_pair[param_idx][set_idx] = 0;
      set_idx++;
    }
  }
  data_sets = set_idx;

  for (set_idx = 0; set_idx < data_sets;) {
    ixheaace_write_bits(pstr_bit_buf, pstr_lossless_data->bs_data_pair[param_idx][set_idx], 1);
    ixheaace_write_bits(pstr_bit_buf, pstr_lossless_data->bs_quant_coarse_xxx[param_idx][set_idx],
                        1);
    ixheaace_write_bits(pstr_bit_buf,
                        pstr_lossless_data->bs_freq_res_stride_xxx[param_idx][set_idx], 2);

    pb_stride =
        freq_res_stride_table[pstr_lossless_data->bs_freq_res_stride_xxx[param_idx][set_idx]];
    data_bands = (stop_band - start_band - 1) / pb_stride + 1;

    ixheaace_mps_515_ec_data_pair_enc(
        pstr_bit_buf, data, old_data, data_type, param_set_index[set_idx], start_band, data_bands,
        pstr_lossless_data->bs_data_pair[param_idx][set_idx],
        pstr_lossless_data->bs_quant_coarse_xxx[param_idx][set_idx], independency_flag);

    if (pstr_lossless_data->bs_data_pair[param_idx][set_idx]) {
      if (pstr_lossless_data->bs_freq_res_stride_xxx[param_idx][set_idx + 1] !=
          pstr_lossless_data->bs_freq_res_stride_xxx[param_idx][set_idx]) {
        return IA_EXHEAACE_EXE_FATAL_MPS_INVALID_RES_STRIDE;
      }
      if (pstr_lossless_data->bs_quant_coarse_xxx[param_idx][set_idx + 1] !=
          pstr_lossless_data->bs_quant_coarse_xxx[param_idx][set_idx]) {
        return IA_EXHEAACE_EXE_FATAL_MPS_INVALID_QUANT_COARSE;
      }
    }

    for (bin = 0; bin < MAX_NUM_BINS; bin++) {
      old_data[bin] = data[param_set_index[set_idx] +
                           pstr_lossless_data->bs_data_pair[param_idx][set_idx]][bin];
    }

    set_idx += pstr_lossless_data->bs_data_pair[param_idx][set_idx] + 1;
  }
  return IA_NO_ERROR;
}

WORD32 ixheaace_mps_515_icc_quant(FLOAT32 val) {
  FLOAT32 p_qsteps[7] = {0.9685f, 0.88909f, 0.72105f, 0.48428f, 0.18382f, -0.2945f, -0.7895f};
  WORD32 i;

  if (val >= p_qsteps[0]) {
    return 0;
  }
  for (i = 1; i < 6; i++) {
    if ((val >= p_qsteps[i]) && (val <= p_qsteps[i - 1])) {
      return i;
    }
  }
  return 7;
}

WORD32 ixheaace_mps_515_cld_quant(FLOAT32 val) {
  FLOAT32 p_qsteps[30] = {-47.5, -42.5, -37.5, -32.5, -27.5, -23.5, -20.5, -17.5, -14.5, -11.5,
                          -9.0,  -7.0,  -5.0,  -3.0,  -1.0,  1.0,   3.0,   5.0,   7.0,   9.0,
                          11.5,  14.5,  17.5,  20.5,  23.5,  27.5,  32.5,  37.5,  42.5,  47.5};

  WORD32 i;

  if (val < p_qsteps[0]) {
    return 0 - 15;
  }
  for (i = 1; i < 30; i++) {
    if ((val <= p_qsteps[i]) && (val >= p_qsteps[i - 1])) {
      return i - 15;
    }
  }
  return 30 - 15;
}

VOID ixheaace_mps_515_ttt_box(WORD32 slots, FLOAT32 *ptr_real1, FLOAT32 *ptr_imag1,
                              FLOAT32 *ptr_real2, FLOAT32 *ptr_imag2, FLOAT32 *ptr_real3,
                              FLOAT32 *ptr_imag3, WORD32 *ptr_qclds1, WORD32 *ptr_qclds2) {
  WORD32 i, j;

  FLOAT32 cld_s1[PARAMETER_BANDS] = {0};
  FLOAT32 cld_s2[PARAMETER_BANDS] = {0};
  FLOAT32 p_pow1[MAX_HYBRID_BANDS] = {0};
  FLOAT32 p_pow2[MAX_HYBRID_BANDS] = {0};
  FLOAT32 p_pow3[MAX_HYBRID_BANDS] = {0};

  FLOAT32 p_pow_par_band1[PARAMETER_BANDS] = {0};
  FLOAT32 p_pow_par_band2[PARAMETER_BANDS] = {0};
  FLOAT32 p_pow_par_band3[PARAMETER_BANDS] = {0};

  for (i = 0; i < slots; i++) {
    for (j = 0; j < MAX_HYBRID_BANDS; j++) {
      p_pow1[j] += ptr_real1[i * MAX_HYBRID_BANDS + j] * ptr_real1[i * MAX_HYBRID_BANDS + j] +
                   ptr_imag1[i * MAX_HYBRID_BANDS + j] * ptr_imag1[i * MAX_HYBRID_BANDS + j];
      p_pow2[j] += ptr_real2[i * MAX_HYBRID_BANDS + j] * ptr_real2[i * MAX_HYBRID_BANDS + j] +
                   ptr_imag2[i * MAX_HYBRID_BANDS + j] * ptr_imag2[i * MAX_HYBRID_BANDS + j];
      p_pow3[j] += ptr_real3[i * MAX_HYBRID_BANDS + j] * ptr_real3[i * MAX_HYBRID_BANDS + j] +
                   ptr_imag3[i * MAX_HYBRID_BANDS + j] * ptr_imag3[i * MAX_HYBRID_BANDS + j];

      ptr_real1[i * MAX_HYBRID_BANDS + j] =
          (ptr_real1[i * MAX_HYBRID_BANDS + j] + ptr_real3[i * MAX_HYBRID_BANDS + j] * 0.7071f);
      ptr_imag1[i * MAX_HYBRID_BANDS + j] =
          (ptr_imag1[i * MAX_HYBRID_BANDS + j] + ptr_imag3[i * MAX_HYBRID_BANDS + j] * 0.7071f);

      ptr_real2[i * MAX_HYBRID_BANDS + j] =
          (ptr_real2[i * MAX_HYBRID_BANDS + j] + ptr_real3[i * MAX_HYBRID_BANDS + j] * 0.7071f);
      ptr_imag2[i * MAX_HYBRID_BANDS + j] =
          (ptr_imag2[i * MAX_HYBRID_BANDS + j] + ptr_imag3[i * MAX_HYBRID_BANDS + j] * 0.7071f);
    }
  }
  for (i = 0; i < MAX_HYBRID_BANDS; i++) {
    p_pow_par_band1[kernels_20[i]] += p_pow1[i];
    p_pow_par_band2[kernels_20[i]] += p_pow2[i];
    p_pow_par_band3[kernels_20[i]] += p_pow3[i];
  }
  for (i = 0; i < PARAMETER_BANDS; i++) {
    if (p_pow_par_band3[i]) {
      cld_s1[i] = ((p_pow_par_band1[i] + p_pow_par_band2[i]) / (p_pow_par_band3[i]));
      cld_s1[i] = (FLOAT32)(10.0 * log(cld_s1[i] + 1e-10) / log(10.0));
    } else if ((p_pow_par_band1[i] + p_pow_par_band2[i]))
      cld_s1[i] = 50.0;
    else
      cld_s1[i] = -50;
    ptr_qclds1[i] = ixheaace_mps_515_cld_quant(cld_s1[i]);

    if (p_pow_par_band2[i]) {
      cld_s2[i] = (p_pow_par_band1[i] / (p_pow_par_band2[i]));
      cld_s2[i] = (FLOAT32)(10 * log(cld_s2[i] + 1e-10f) / log(10.0));
    } else if (p_pow_par_band1[i])
      cld_s2[i] = 50.0;
    else
      cld_s2[i] = -50;
    ptr_qclds2[i] = ixheaace_mps_515_cld_quant(cld_s2[i]);
  }
}

VOID ixheaace_mps_515_ott_box(WORD32 slots, FLOAT32 *ptr_real1, FLOAT32 *ptr_imag1,
                              FLOAT32 *ptr_real2, FLOAT32 *ptr_imag2, WORD32 *ptr_p_qclds,
                              WORD32 *ptr_qiccs) {
  WORD32 i, j;

  FLOAT32 clds[PARAMETER_BANDS] = {0};
  FLOAT32 iccs[PARAMETER_BANDS] = {0};
  FLOAT32 p_pow1[MAX_HYBRID_BANDS] = {0};
  FLOAT32 p_pow2[MAX_HYBRID_BANDS] = {0};
  FLOAT32 p_xcor_real[MAX_HYBRID_BANDS] = {0};

  FLOAT32 p_pow_par_band1[PARAMETER_BANDS] = {0};
  FLOAT32 p_pow_par_band2[PARAMETER_BANDS] = {0};
  FLOAT32 p_xcor_par_band[PARAMETER_BANDS] = {0};

  for (i = 0; i < slots; i++) {
    for (j = 0; j < MAX_HYBRID_BANDS; j++) {
      p_pow1[j] += ptr_real1[i * MAX_HYBRID_BANDS + j] * ptr_real1[i * MAX_HYBRID_BANDS + j] +
                   ptr_imag1[i * MAX_HYBRID_BANDS + j] * ptr_imag1[i * MAX_HYBRID_BANDS + j];
      p_pow2[j] += ptr_real2[i * MAX_HYBRID_BANDS + j] * ptr_real2[i * MAX_HYBRID_BANDS + j] +
                   ptr_imag2[i * MAX_HYBRID_BANDS + j] * ptr_imag2[i * MAX_HYBRID_BANDS + j];
      p_xcor_real[j] +=
          ptr_real1[i * MAX_HYBRID_BANDS + j] * ptr_real2[i * MAX_HYBRID_BANDS + j] +
          ptr_imag1[i * MAX_HYBRID_BANDS + j] * ptr_imag2[i * MAX_HYBRID_BANDS + j];

      ptr_real1[i * MAX_HYBRID_BANDS + j] =
          (ptr_real1[i * MAX_HYBRID_BANDS + j] + ptr_real2[i * MAX_HYBRID_BANDS + j]);
      ptr_imag1[i * MAX_HYBRID_BANDS + j] =
          (ptr_imag1[i * MAX_HYBRID_BANDS + j] + ptr_imag2[i * MAX_HYBRID_BANDS + j]);
    }
  }
  for (i = 0; i < MAX_HYBRID_BANDS; i++) {
    p_pow_par_band1[kernels_20[i]] += p_pow1[i];
    p_pow_par_band2[kernels_20[i]] += p_pow2[i];
    p_xcor_par_band[kernels_20[i]] += p_xcor_real[i];
  }
  for (i = 0; i < PARAMETER_BANDS; i++) {
    if (p_pow_par_band2[i]) {
      clds[i] = (p_pow_par_band1[i] / (p_pow_par_band2[i]));
      clds[i] = (FLOAT32)(10 * log(clds[i] + 1e-10) / log(10.0));
    } else if (p_pow_par_band1[i])
      clds[i] = 50.0;
    else
      clds[i] = -50;  // 0.0;
    iccs[i] =
        p_xcor_par_band[i] / (FLOAT32)sqrt((p_pow_par_band1[i] * p_pow_par_band2[i] + 1e-10));

    ptr_p_qclds[i] = ixheaace_mps_515_cld_quant(clds[i]);
    ptr_qiccs[i] = ixheaace_mps_515_icc_quant(iccs[i]);
  }
}

IA_ERRORCODE ixheaace_mps_212_write_spatial_specific_config(
    ixheaace_mps_spatial_specific_config *const pstr_spatial_specific_config,
    UWORD8 *const ptr_output_buffer, const WORD32 output_buffer_size,
    WORD32 *const ptr_output_bits, WORD32 aot) {
  IA_ERRORCODE error = IA_NO_ERROR;
  WORD32 bs_sampling_frequency_index = 0;
  WORD32 bs_freq_res = 0;
  ixheaace_bit_buf bit_buf;
  ixheaace_bit_buf_handle pstr_bit_buf =
      ia_enhaacplus_enc_create_bitbuffer(&bit_buf, ptr_output_buffer, output_buffer_size);

  error = ixheaace_mps_212_get_bs_freq_res_index(pstr_spatial_specific_config->num_bands,
                                                 &bs_freq_res, aot);
  if (error) {
    return error;
  }

  if (aot == AOT_AAC_ELD) {
    ixheaace_mps_212_get_sampling_frequency_index(
        pstr_spatial_specific_config->bs_sampling_frequency, &bs_sampling_frequency_index);
    ixheaace_write_bits(pstr_bit_buf, bs_sampling_frequency_index, 4);

    if (bs_sampling_frequency_index == 15) {
      ixheaace_write_bits(pstr_bit_buf, pstr_spatial_specific_config->bs_sampling_frequency, 24);
    }

    ixheaace_write_bits(pstr_bit_buf, pstr_spatial_specific_config->bs_frame_length, 5);

    ixheaace_write_bits(pstr_bit_buf, bs_freq_res, 3);
    ixheaace_write_bits(pstr_bit_buf, pstr_spatial_specific_config->bs_tree_config, 4);
    ixheaace_write_bits(pstr_bit_buf, pstr_spatial_specific_config->bs_quant_mode, 2);

    ixheaace_write_bits(pstr_bit_buf, 0, 1);

    ixheaace_write_bits(pstr_bit_buf, pstr_spatial_specific_config->bs_fixed_gain_dmx, 3);

    ixheaace_write_bits(pstr_bit_buf, 0, 2);
    ixheaace_write_bits(pstr_bit_buf, pstr_spatial_specific_config->bs_decorr_config, 2);

    ixheaace_byte_align_buffer(pstr_bit_buf);

    if ((*ptr_output_bits = ia_enhaacplus_enc_get_bits_available(pstr_bit_buf)) >
        (output_buffer_size * 8)) {
      return IA_EXHEAACE_CONFIG_NONFATAL_MPS_INVALID_CONFIG;
    }

    ixheaace_byte_align_buffer(pstr_bit_buf);
  } else {
    ixheaace_mps_212_get_sampling_frequency_index(
        pstr_spatial_specific_config->bs_sampling_frequency, &bs_sampling_frequency_index);
    ixheaace_write_bits(pstr_bit_buf, bs_freq_res, 3);

    ixheaace_write_bits(pstr_bit_buf, pstr_spatial_specific_config->bs_fixed_gain_dmx, 3);

    ixheaace_write_bits(pstr_bit_buf, 0, 2);
    ixheaace_write_bits(pstr_bit_buf, pstr_spatial_specific_config->bs_decorr_config, 2);

    ixheaace_write_bits(pstr_bit_buf, 0, 1);
    ixheaace_write_bits(pstr_bit_buf, 0, 1);
    ixheaace_write_bits(pstr_bit_buf, 0, 1);

    ixheaace_write_bits(pstr_bit_buf, 0, 5);
    ixheaace_write_bits(pstr_bit_buf, 0, 1);

    *ptr_output_bits = ia_enhaacplus_enc_get_bits_available(pstr_bit_buf);
  }
  return error;
}

IA_ERRORCODE ixheaace_mps_212_write_spatial_frame(
    UWORD8 *const ptr_output_buffer, const WORD32 output_buffer_size,
    WORD32 *const ptr_output_bits, ixheaace_mps_pstr_bsf_instance pstr_bsf_instance, WORD32 aot) {
  IA_ERRORCODE error = IA_NO_ERROR;
  WORD32 i, j, num_param_sets, num_ott_boxes;
  ixheaace_mps_spatial_frame *pstr_spatial_frame = NULL;
  ixheaace_mps_spatial_specific_config *pstr_specific_config = NULL;
  ixheaace_bit_buf bit_buf;
  ixheaace_bit_buf_handle pstr_bit_buf =
      ia_enhaacplus_enc_create_bitbuffer(&bit_buf, ptr_output_buffer, output_buffer_size);
  pstr_spatial_frame = &pstr_bsf_instance->frame;
  pstr_specific_config = &pstr_bsf_instance->spatial_specific_config;
  num_ott_boxes = pstr_bsf_instance->spatial_specific_config.tree_description.num_ott_boxes;
  num_param_sets = pstr_spatial_frame->framing_info.num_param_sets;

  if (pstr_spatial_frame->b_use_bb_cues) {
    for (i = 0; i < IXHEAACE_MPS_MAX_NUM_BOXES; i++) {
      if (num_param_sets == 1) {
        pstr_spatial_frame->cld_lossless_data.bs_freq_res_stride_xxx[i][0] = 3;
        pstr_spatial_frame->icc_lossless_data.bs_freq_res_stride_xxx[i][0] = 3;
      } else {
        for (j = 1; j < MAX_NUM_PARAMS; j++) {
          pstr_spatial_frame->cld_lossless_data.bs_freq_res_stride_xxx[i][j] = 3;
          pstr_spatial_frame->icc_lossless_data.bs_freq_res_stride_xxx[i][j] = 3;
        }
      }
    }
  }

  if (aot == AOT_AAC_ELD) {
    ixheaace_mps_212_write_framing_info(
        pstr_bit_buf, &(pstr_spatial_frame->framing_info),
        pstr_bsf_instance->spatial_specific_config.bs_frame_length);
    ixheaace_write_bits(pstr_bit_buf, pstr_spatial_frame->bs_independency_flag, 1);
  } else if (aot == AOT_USAC) {
    if (!pstr_spatial_frame->bs_independency_flag) {
      ixheaace_write_bits(pstr_bit_buf, pstr_spatial_frame->bs_independency_flag, 1);
    }
  }
  error = ixheaace_mps_212_write_ott_data(
      pstr_bit_buf, &pstr_bsf_instance->prev_frame_data.prev_ott_data,
      &pstr_spatial_frame->ott_data, pstr_specific_config->ott_config,
      &pstr_spatial_frame->cld_lossless_data, &pstr_spatial_frame->icc_lossless_data,
      num_ott_boxes, pstr_specific_config->num_bands, num_param_sets,
      pstr_spatial_frame->bs_independency_flag);
  if (error != IA_NO_ERROR) {
    return error;
  }
  if (aot == AOT_AAC_ELD) {
    ixheaace_mps_212_write_smg_data(pstr_bit_buf, &pstr_spatial_frame->smg_data, num_param_sets,
                                    pstr_specific_config->num_bands);
  }

  *ptr_output_bits = ia_enhaacplus_enc_get_bits_available(pstr_bit_buf);
  if ((*ptr_output_bits) > (output_buffer_size * 8)) {
    return IA_EXHEAACE_CONFIG_NONFATAL_MPS_INVALID_CONFIG;
  }
  return error;
}

IA_ERRORCODE
ixheaace_mps_515_write_spatial_specific_config(ixheaace_bit_buf_handle pstr_bit_buf,
                                               ixheaace_mps_sac_bsf_instance *pstr_bsf_instance) {
  WORD32 idx, box, bin;
  WORD32 bs_sampling_frequency_index;
  ixheaace_mps_sac_spatial_frame *pstr_spatial_frame = &(pstr_bsf_instance->current_frame);
  ixheaace_mps_sac_specific_config *pstr_specific_config =
      &(pstr_bsf_instance->spatial_specific_config);
  ixheaace_mps_sac_tree_description *pstr_tree_description =
      &(pstr_specific_config->tree_description);

  pstr_tree_description->num_ott_boxes =
      tree_config_table[pstr_specific_config->bs_tree_config].num_ott_boxes;
  pstr_tree_description->num_ttt_boxes =
      tree_config_table[pstr_specific_config->bs_tree_config].num_ttt_boxes;
  pstr_tree_description->num_in_chan =
      tree_config_table[pstr_specific_config->bs_tree_config].num_in_chan;
  pstr_tree_description->num_out_chan =
      tree_config_table[pstr_specific_config->bs_tree_config].num_out_chan;

  if (pstr_specific_config->bs_temp_shape_config == 2) {
    return IA_EXHEAACE_EXE_FATAL_MPS_UNSUPPORTED_GUIDED_ENV_SHAPE;
  }

  if (pstr_specific_config->bs_3d_audio_mode > 0) {
    return IA_EXHEAACE_EXE_FATAL_MPS_3D_STEREO_MODE_NOT_SUPPORTED;
  }

  if (pstr_specific_config->bs_residual_coding == 1) {
    return IA_EXHEAACE_EXE_FATAL_MPS_UNSUPPORTED_RESIDUAL_CODING;
  }
  if (pstr_specific_config->bs_arbitrary_downmix == 2) {
    return IA_EXHEAACE_EXE_FATAL_MPS_UNSUPPORTED_ARBITARY_DOWNMIX_CODING;
  }
  if (pstr_specific_config->tree_description.arbitrary_tree) {
    return IA_EXHEAACE_EXE_FATAL_MPS_ARBITARY_TREE_NOT_SUPPORTED;
  }

  for (box = 0; box < MAX_NUM_BOXES; box++) {
    pstr_tree_description->ott_mode_lfe[box] =
        tree_config_table[pstr_specific_config->bs_tree_config].ott_mode_lfe[box];
  }
  pstr_bsf_instance->num_bins = freq_res_bin_table[pstr_specific_config->bs_freq_res];

  bs_sampling_frequency_index = 15;
  for (idx = 0; idx < 15; idx++) {
    if (pstr_specific_config->bs_sampling_frequency == ia_sampl_freq_table[idx]) {
      bs_sampling_frequency_index = idx;
      break;
    }
  }

  ixheaace_write_bits(pstr_bit_buf, bs_sampling_frequency_index, 4);
  if (bs_sampling_frequency_index == 15) {
    ixheaace_write_bits(pstr_bit_buf, pstr_specific_config->bs_sampling_frequency, 24);
  }
  ixheaace_write_bits(pstr_bit_buf, pstr_specific_config->bs_frame_length, 5);
  ixheaace_write_bits(pstr_bit_buf, pstr_specific_config->bs_freq_res, 3);
  ixheaace_write_bits(pstr_bit_buf, pstr_specific_config->bs_tree_config, 4);
  ixheaace_write_bits(pstr_bit_buf, pstr_specific_config->bs_quant_mode, 2);
  ixheaace_write_bits(pstr_bit_buf, pstr_specific_config->bs_one_icc, 1);
  ixheaace_write_bits(pstr_bit_buf, pstr_specific_config->bs_arbitrary_downmix, 1);
  ixheaace_write_bits(pstr_bit_buf, pstr_specific_config->bs_fixed_gain_sur, 3);
  ixheaace_write_bits(pstr_bit_buf, pstr_specific_config->bs_fixed_gain_lfe, 3);
  ixheaace_write_bits(pstr_bit_buf, pstr_specific_config->bs_fixed_gain_dmx, 3);
  ixheaace_write_bits(pstr_bit_buf, pstr_specific_config->bs_matrix_mode, 1);
  ixheaace_write_bits(pstr_bit_buf, pstr_specific_config->bs_temp_shape_config, 2);
  ixheaace_write_bits(pstr_bit_buf, pstr_specific_config->bs_decorr_config, 2);
  ixheaace_write_bits(pstr_bit_buf, pstr_specific_config->bs_3d_audio_mode, 1);

  for (box = 0; box < pstr_tree_description->num_ott_boxes; box++) {
    if (pstr_tree_description->ott_mode_lfe[box]) {
      ixheaace_write_bits(pstr_bit_buf, pstr_specific_config->ott_config[box].bs_ott_bands, 5);
    }
  }
  for (box = 0; box < pstr_tree_description->num_ttt_boxes; box++) {
    ixheaace_write_bits(pstr_bit_buf, pstr_specific_config->ttt_config[box].bs_ttt_dual_mode, 1);
    ixheaace_write_bits(pstr_bit_buf, pstr_specific_config->ttt_config[box].bs_ttt_mode_low, 3);
    if (pstr_specific_config->ttt_config[box].bs_ttt_dual_mode == 1) {
      ixheaace_write_bits(pstr_bit_buf, pstr_specific_config->ttt_config[box].bs_ttt_mode_high,
                          3);
      ixheaace_write_bits(pstr_bit_buf, pstr_specific_config->ttt_config[box].bs_ttt_bands_low,
                          5);
    }
  }
  ixheaace_byte_align_buffer(pstr_bit_buf);

  for (box = 0; box < MAX_NUM_BOXES; box++) {
    pstr_specific_config->ttt_config[box].bs_ttt_bands_low = pstr_bsf_instance->num_bins;
    for (bin = 0; bin < MAX_NUM_BINS; bin++) {
      pstr_spatial_frame->ott_data.cld_old[box][bin] = 0;
      pstr_spatial_frame->ott_data.icc_old[box][bin] = 0;
      pstr_spatial_frame->ttt_data.cpc_cld1_old[box][bin] = 0;
      pstr_spatial_frame->ttt_data.cpc_cld2_old[box][bin] = 0;
      pstr_spatial_frame->ttt_data.icc_old[box][bin] = 0;
    }
  }

  for (box = 0; box < pstr_tree_description->num_ott_boxes; box++) {
    if (!pstr_tree_description->ott_mode_lfe[box]) {
      pstr_specific_config->ott_config[box].bs_ott_bands = pstr_bsf_instance->num_bins;
    }
    if (!pstr_specific_config->ttt_config[box].bs_ttt_dual_mode) {
      pstr_specific_config->ttt_config[box].bs_ttt_bands_low = pstr_bsf_instance->num_bins;
    }
  }

  return IA_NO_ERROR;
}

IA_ERRORCODE
ixheaace_mps_515_write_spatial_frame(ixheaace_bit_buf_handle pstr_bit_buf,
                                     ixheaace_mps_sac_bsf_instance *pstr_bsf_instance) {
  IA_ERRORCODE error = IA_NO_ERROR;
  UWORD8 bits_param_slot;
  WORD32 param, box, ch, bin;
  WORD32 prev_bs_param_slot, num_temp_shape_chan;
  ixheaace_mps_sac_spatial_frame *pstr_spatial_frame = &(pstr_bsf_instance->current_frame);
  ixheaace_mps_sac_specific_config *pstr_specific_config =
      &(pstr_bsf_instance->spatial_specific_config);
  WORD32 bs_independency_flag = pstr_spatial_frame->bs_independency_flag;
  WORD32 num_param_sets = pstr_spatial_frame->framing_info.bs_num_param_sets;
  WORD32 num_ott_boxes =
      pstr_bsf_instance->spatial_specific_config.tree_description.num_ott_boxes;
  WORD32 num_ttt_boxes =
      pstr_bsf_instance->spatial_specific_config.tree_description.num_ttt_boxes;
  WORD32 *ptr_ott_mode_lfe =
      pstr_bsf_instance->spatial_specific_config.tree_description.ott_mode_lfe;

  if (pstr_specific_config->bs_arbitrary_downmix) {
    return IA_EXHEAACE_EXE_FATAL_MPS_UNSUPPORTED_ARBITARY_DOWNMIX_CODING;
  }
  if (pstr_specific_config->bs_residual_coding == 1) {
    return IA_EXHEAACE_EXE_FATAL_MPS_UNSUPPORTED_RESIDUAL_CODING;
  }
  if (pstr_specific_config->bs_arbitrary_downmix == 2) {
    return IA_EXHEAACE_EXE_FATAL_MPS_UNSUPPORTED_ARBITARY_DOWNMIX_CODING;
  }
  if (pstr_specific_config->tree_description.arbitrary_tree) {
    return IA_EXHEAACE_EXE_FATAL_MPS_ARBITARY_TREE_NOT_SUPPORTED;
  }
  ixheaace_write_bits(pstr_bit_buf, pstr_spatial_frame->framing_info.bs_framing_type, 1);
  ixheaace_write_bits(pstr_bit_buf, num_param_sets - 1, 1);

  if (pstr_spatial_frame->framing_info.bs_framing_type) {
    prev_bs_param_slot = -1;

    for (param = 0; param < num_param_sets; param++) {
      bits_param_slot = 0;
      while ((1 << bits_param_slot) < (pstr_specific_config->bs_frame_length + 1 -
                                       num_param_sets + param - prev_bs_param_slot)) {
        bits_param_slot++;
      }

      if (bits_param_slot > 0) {
        ixheaace_write_bits(
            pstr_bit_buf,
            pstr_spatial_frame->framing_info.bs_param_slots[param] - prev_bs_param_slot - 1,
            bits_param_slot);
      }
      prev_bs_param_slot = pstr_spatial_frame->framing_info.bs_param_slots[param];
    }
  }

  ixheaace_write_bits(pstr_bit_buf, pstr_spatial_frame->bs_independency_flag, 1);

  for (box = 0; box < num_ott_boxes; box++) {
    error = ixheaace_mps_515_ec_data(
        pstr_bit_buf, pstr_spatial_frame->ott_data.cld[box],
        pstr_spatial_frame->ott_data.cld_old[box], &(pstr_spatial_frame->cld_lossless_data),
        IXHEAACE_MPS_SAC_DATA_TYPE_CLD, box, num_param_sets, bs_independency_flag, 0,
        pstr_specific_config->ott_config[box].bs_ott_bands);
    if (error) {
      return error;
    }
  }
  if (!pstr_bsf_instance->spatial_specific_config.bs_one_icc) {
    for (box = 0; box < num_ott_boxes; box++) {
      if (!ptr_ott_mode_lfe[box]) {
        error = ixheaace_mps_515_ec_data(pstr_bit_buf, pstr_spatial_frame->ott_data.icc[box],
                                         pstr_spatial_frame->ott_data.icc_old[box],
                                         &(pstr_spatial_frame->cld_lossless_data),
                                         IXHEAACE_MPS_SAC_DATA_TYPE_ICC, box, num_param_sets,
                                         bs_independency_flag, 0, pstr_bsf_instance->num_bins);
        if (error) {
          return error;
        }
      }
    }
  } else {
    error = ixheaace_mps_515_ec_data(pstr_bit_buf, pstr_spatial_frame->ott_data.icc[0],
                                     pstr_spatial_frame->ott_data.icc_old[0],
                                     &(pstr_spatial_frame->cld_lossless_data),
                                     IXHEAACE_MPS_SAC_DATA_TYPE_ICC, 0, num_param_sets,
                                     bs_independency_flag, 0, pstr_bsf_instance->num_bins);
    if (error) {
      return error;
    }
  }
  for (box = 0; box < num_ttt_boxes; box++) {
    if (pstr_specific_config->ttt_config[box].bs_ttt_mode_low >= 2) {
      error = ixheaace_mps_515_ec_data(pstr_bit_buf, pstr_spatial_frame->ttt_data.cpc_cld1[box],
                                       pstr_spatial_frame->ttt_data.cpc_cld1_old[box],
                                       &(pstr_spatial_frame->cld_lossless_data),
                                       IXHEAACE_MPS_SAC_DATA_TYPE_CLD, box, num_param_sets,
                                       bs_independency_flag, 0,
                                       pstr_specific_config->ttt_config[box].bs_ttt_bands_low);
      if (error) {
        return error;
      }
      error = ixheaace_mps_515_ec_data(pstr_bit_buf, pstr_spatial_frame->ttt_data.cpc_cld2[box],
                                       pstr_spatial_frame->ttt_data.cpc_cld2_old[box],
                                       &(pstr_spatial_frame->cld_lossless_data),
                                       IXHEAACE_MPS_SAC_DATA_TYPE_CLD, box, num_param_sets,
                                       bs_independency_flag, 0,
                                       pstr_specific_config->ttt_config[box].bs_ttt_bands_low);
      if (error) {
        return error;
      }
    } else {
      error = ixheaace_mps_515_ec_data(pstr_bit_buf, pstr_spatial_frame->ttt_data.cpc_cld1[box],
                                       pstr_spatial_frame->ttt_data.cpc_cld1_old[box],
                                       &(pstr_spatial_frame->cpc_lossless_data),
                                       IXHEAACE_MPS_SAC_DATA_TYPE_CPC, box, num_param_sets,
                                       bs_independency_flag, 0,
                                       pstr_specific_config->ttt_config[box].bs_ttt_bands_low);
      if (error) {
        return error;
      }
      error = ixheaace_mps_515_ec_data(pstr_bit_buf, pstr_spatial_frame->ttt_data.cpc_cld2[box],
                                       pstr_spatial_frame->ttt_data.cpc_cld2_old[box],
                                       &(pstr_spatial_frame->cpc_lossless_data),
                                       IXHEAACE_MPS_SAC_DATA_TYPE_CPC, box, num_param_sets,
                                       bs_independency_flag, 0,
                                       pstr_specific_config->ttt_config[box].bs_ttt_bands_low);
      if (error) {
        return error;
      }
      error = ixheaace_mps_515_ec_data(
          pstr_bit_buf, pstr_spatial_frame->ttt_data.icc[box],
          pstr_spatial_frame->ttt_data.icc_old[box], &(pstr_spatial_frame->icc_lossless_data),
          IXHEAACE_MPS_SAC_DATA_TYPE_ICC, box, num_param_sets, bs_independency_flag, 0,
          pstr_specific_config->ttt_config[box].bs_ttt_bands_low);
      if (error) {
        return error;
      }
    }

    if (pstr_specific_config->ttt_config[box].bs_ttt_dual_mode) {
      if (pstr_specific_config->ttt_config[box].bs_ttt_mode_high >= 2) {
        error = ixheaace_mps_515_ec_data(
            pstr_bit_buf, pstr_spatial_frame->ttt_data.cpc_cld1[box],
            pstr_spatial_frame->ttt_data.cpc_cld1_old[box],
            &(pstr_spatial_frame->cld_lossless_data), IXHEAACE_MPS_SAC_DATA_TYPE_CLD, box,
            num_param_sets, bs_independency_flag,
            pstr_specific_config->ttt_config[box].bs_ttt_bands_low, pstr_bsf_instance->num_bins);
        if (error) {
          return error;
        }
        error = ixheaace_mps_515_ec_data(
            pstr_bit_buf, pstr_spatial_frame->ttt_data.cpc_cld2[box],
            pstr_spatial_frame->ttt_data.cpc_cld2_old[box],
            &(pstr_spatial_frame->cld_lossless_data), IXHEAACE_MPS_SAC_DATA_TYPE_CLD, box,
            num_param_sets, bs_independency_flag,
            pstr_specific_config->ttt_config[box].bs_ttt_bands_low, pstr_bsf_instance->num_bins);
        if (error) {
          return error;
        }
      } else {
        error = ixheaace_mps_515_ec_data(
            pstr_bit_buf, pstr_spatial_frame->ttt_data.cpc_cld1[box],
            pstr_spatial_frame->ttt_data.cpc_cld1_old[box],
            &(pstr_spatial_frame->cpc_lossless_data), IXHEAACE_MPS_SAC_DATA_TYPE_CPC, box,
            num_param_sets, bs_independency_flag,
            pstr_specific_config->ttt_config[box].bs_ttt_bands_low, pstr_bsf_instance->num_bins);
        if (error) {
          return error;
        }
        error = ixheaace_mps_515_ec_data(
            pstr_bit_buf, pstr_spatial_frame->ttt_data.cpc_cld2[box],
            pstr_spatial_frame->ttt_data.cpc_cld2_old[box],
            &(pstr_spatial_frame->cpc_lossless_data), IXHEAACE_MPS_SAC_DATA_TYPE_CPC, box,
            num_param_sets, bs_independency_flag,
            pstr_specific_config->ttt_config[box].bs_ttt_bands_low, pstr_bsf_instance->num_bins);
        if (error) {
          return error;
        }
        error = ixheaace_mps_515_ec_data(
            pstr_bit_buf, pstr_spatial_frame->ttt_data.icc[box],
            pstr_spatial_frame->ttt_data.icc_old[box], &(pstr_spatial_frame->icc_lossless_data),
            IXHEAACE_MPS_SAC_DATA_TYPE_ICC, box, num_param_sets, bs_independency_flag,
            pstr_specific_config->ttt_config[box].bs_ttt_bands_low, pstr_bsf_instance->num_bins);
        if (error) {
          return error;
        }
      }
    }
  }
  for (param = 0; param < num_param_sets; param++) {
    ixheaace_write_bits(pstr_bit_buf, pstr_spatial_frame->smg_data.bs_smooth_mode[param], 2);

    if (pstr_spatial_frame->smg_data.bs_smooth_mode[param] >= 2) {
      ixheaace_write_bits(pstr_bit_buf, pstr_spatial_frame->smg_data.bs_smooth_time[param], 2);
    }
    if (pstr_spatial_frame->smg_data.bs_smooth_mode[param] == 3) {
      ixheaace_write_bits(pstr_bit_buf, pstr_spatial_frame->smg_data.bs_freq_res_stride[param],
                          2);
      for (bin = 0; bin < pstr_bsf_instance->num_bins;
           bin += pstr_spatial_frame->smg_data.bs_freq_res_stride[param]) {
        ixheaace_write_bits(pstr_bit_buf, pstr_spatial_frame->smg_data.bs_smg_data[param][bin],
                            1);
      }
    }
  }
  if (pstr_specific_config->bs_temp_shape_config != 0) {
    ixheaace_write_bits(pstr_bit_buf, pstr_spatial_frame->temp_shape_data.bs_temp_shape_enable,
                        1);
    if (pstr_spatial_frame->temp_shape_data.bs_temp_shape_enable) {
      num_temp_shape_chan = temp_shape_chan_table[pstr_specific_config->bs_temp_shape_config - 1]
                                                 [pstr_specific_config->bs_tree_config];
      for (ch = 0; ch < num_temp_shape_chan; ch++) {
        ixheaace_write_bits(pstr_bit_buf,
                            pstr_spatial_frame->temp_shape_data.bs_temp_shape_enable_channel[ch],
                            1);
      }
      if (pstr_specific_config->bs_temp_shape_config == 2) {
        return IA_EXHEAACE_EXE_FATAL_MPS_UNSUPPORTED_GUIDED_ENV_SHAPE;
      }
    }
  }

  ixheaace_byte_align_buffer(pstr_bit_buf);

  return IA_NO_ERROR;
}