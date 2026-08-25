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
#include <string.h>
#include "ixheaac_type_def.h"
#include "ixheaac_constants.h"
#include "ixheaace_aac_constants.h"

#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops16.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaac_basic_ops.h"
#include "ixheaac_error_standards.h"

#include "ixheaace_common_rom.h"
#include "ixheaace_psy_const.h"
#include "ixheaace_tns.h"
#include "ixheaace_tns_params.h"
#include "ixheaace_rom.h"
#include "ixheaace_bitbuffer.h"
#include "ixheaace_psy_configuration.h"
#include "ixheaace_tns_func.h"
#include "ixheaace_error_codes.h"
#include "ixheaace_common_utils.h"

static VOID ia_enhaacplus_enc_auto_to_parcor(const FLOAT32 *ptr_input, FLOAT32 *ptr_refl_coeff,
                                             WORD32 num_coeff, FLOAT32 *ptr_work_buffer,
                                             FLOAT32 *prediction_gain) {
  WORD32 i, j;
  FLOAT32 *ptr_work_buf_tmp, tmp_var;

  memset(ptr_refl_coeff, 0, num_coeff * sizeof(*ptr_refl_coeff));

  if (ptr_input[0] == 0) {
    return;
  }

  ptr_work_buffer[0] = ptr_input[0];
  for (i = 1; i < num_coeff; i++) {
    tmp_var = ptr_input[i];
    ptr_work_buffer[i] = tmp_var;
    ptr_work_buffer[i + num_coeff - 1] = tmp_var;
  }
  ptr_work_buffer[i + num_coeff - 1] = ptr_input[i];

  for (i = 0; i < num_coeff; i++) {
    FLOAT32 refc, tmp;

    tmp = ptr_work_buffer[num_coeff + i];

    if (tmp < 0.f) {
      tmp = -tmp;
    }

    if ((ptr_work_buffer[0]) < tmp) {
      break;
    }

    if (ptr_work_buffer[0] == 0.f) {
      refc = 0;
    } else {
      refc = (tmp / ptr_work_buffer[0]);
    }

    if (ptr_work_buffer[num_coeff + i] > 0.f) {
      refc = -refc;
    }

    ptr_refl_coeff[i] = refc;

    ptr_work_buf_tmp = &(ptr_work_buffer[num_coeff]);

    for (j = i; j < num_coeff; j++) {
      FLOAT32 accu1, accu2;

      accu1 = (refc * ptr_work_buffer[j - i]);
      accu1 = (accu1) + ptr_work_buf_tmp[j];

      accu2 = (refc * ptr_work_buf_tmp[j]);
      accu2 = accu2 + ptr_work_buffer[j - i];

      ptr_work_buf_tmp[j] = accu1;
      ptr_work_buffer[j - i] = accu2;
    }
  }

  *prediction_gain = (ptr_input[0] / ptr_work_buffer[0]);
}

static IA_ERRORCODE ia_enhaacplus_enc_calc_tns_filter(const FLOAT32 *ptr_signal,
                                                      const FLOAT32 *ptr_window, WORD32 num_lines,
                                                      WORD32 tns_order, FLOAT32 *ptr_parcor,
                                                      FLOAT32 *prediction_gain) {
  FLOAT32 auto_corr_buf[TEMPORAL_NOISE_SHAPING_MAX_ORDER + 1];
  FLOAT32 par_cor_buf[2 * TEMPORAL_NOISE_SHAPING_MAX_ORDER];
  WORD32 i;

  if (tns_order > TEMPORAL_NOISE_SHAPING_MAX_ORDER) {
    return IA_EXHEAACE_EXE_FATAL_INVALID_TNS_FILT_ORDER;
  }

  memset(&auto_corr_buf[0], 0, (tns_order + 1) * sizeof(auto_corr_buf[0]));

  ia_enhaacplus_enc_auto_correlation(ptr_signal, auto_corr_buf, num_lines, tns_order + 1);

  if (ptr_window) {
    for (i = 0; i < tns_order + 1; i++) {
      auto_corr_buf[i] = (auto_corr_buf[i] * ptr_window[i]);
    }
  }

  ia_enhaacplus_enc_auto_to_parcor(auto_corr_buf, ptr_parcor, tns_order, par_cor_buf,
                                   prediction_gain);

  return IA_NO_ERROR;
}

IA_ERRORCODE ia_enhaacplus_enc_tns_detect(ixheaace_temporal_noise_shaping_data *pstr_tns_data,
                                          ixheaace_temporal_noise_shaping_config tns_config,
                                          FLOAT32 *ptr_scratch_tns, const WORD32 *ptr_sfb_offset,
                                          FLOAT32 *ptr_spectrum, WORD32 sub_blk_num,
                                          WORD32 block_type, WORD32 aot, FLOAT32 *ptr_sfb_energy,
                                          FLOAT32 *ptr_shared_buffer1, WORD32 long_frame_len)

{
  FLOAT32 prediction_gain = 0;
  FLOAT32 *ptr_weighted_spec = ptr_scratch_tns + sub_blk_num * long_frame_len;
  WORD i;
  FLOAT32 *ptr_weighted_spec_new = ptr_weighted_spec;
  IA_ERRORCODE error_code = IA_NO_ERROR;

  if (tns_config.tns_active) {
    ia_enhaacplus_enc_calc_weighted_spectrum(ptr_spectrum, ptr_weighted_spec, ptr_sfb_energy,
                                             ptr_sfb_offset, tns_config.lpc_start_line,
                                             tns_config.lpc_stop_line, tns_config.lpc_start_band,
                                             tns_config.lpc_stop_band, ptr_shared_buffer1, aot);

    for (i = tns_config.lpc_stop_line; i < (tns_config.lpc_stop_line + 12); i++) {
      ptr_weighted_spec_new[i] = 0;
    }

    if (block_type != SHORT_WINDOW) {
      error_code = ia_enhaacplus_enc_calc_tns_filter(
          &ptr_weighted_spec_new[tns_config.lpc_start_line], tns_config.acf_window_float,
          tns_config.lpc_stop_line - tns_config.lpc_start_line, tns_config.max_order,
          pstr_tns_data->data_raw.tns_data_long.sub_block_info.parcor, &prediction_gain);

      if (error_code != IA_NO_ERROR) {
        return error_code;
      }
      pstr_tns_data->data_raw.tns_data_long.sub_block_info.tns_active =
          ((prediction_gain) > tns_config.threshold) ? 1 : 0;

      pstr_tns_data->data_raw.tns_data_long.sub_block_info.prediction_gain = prediction_gain;
    } else {
      error_code = ia_enhaacplus_enc_calc_tns_filter(
          &ptr_weighted_spec_new[tns_config.lpc_start_line], tns_config.acf_window_float,
          tns_config.lpc_stop_line - tns_config.lpc_start_line, tns_config.max_order,
          pstr_tns_data->data_raw.tns_data_short.sub_block_info[sub_blk_num].parcor,
          &prediction_gain);

      if (error_code != IA_NO_ERROR) {
        return error_code;
      }

      pstr_tns_data->data_raw.tns_data_short.sub_block_info[sub_blk_num].tns_active =
          ((prediction_gain) > tns_config.threshold) ? 1 : 0;

      pstr_tns_data->data_raw.tns_data_short.sub_block_info[sub_blk_num].prediction_gain =
          prediction_gain;
    }
  } else {
    if (block_type != SHORT_WINDOW) {
      pstr_tns_data->data_raw.tns_data_long.sub_block_info.tns_active = 0;
      pstr_tns_data->data_raw.tns_data_long.sub_block_info.prediction_gain = 0.f;
    } else {
      pstr_tns_data->data_raw.tns_data_short.sub_block_info[sub_blk_num].tns_active = 0;
      pstr_tns_data->data_raw.tns_data_short.sub_block_info[sub_blk_num].prediction_gain = 0.f;
    }
  }
  return error_code;
}

VOID ia_enhaacplus_enc_tns_sync(ixheaace_temporal_noise_shaping_data *pstr_tns_data_dst,
                                const ixheaace_temporal_noise_shaping_data *pstr_tns_data_src,
                                const ixheaace_temporal_noise_shaping_config tns_config,
                                const WORD32 sub_blk_num, const WORD32 block_type) {
  if (block_type != SHORT_WINDOW) {
    ixheaace_temporal_noise_shaping_subblock_info_long *pstr_sfb_info_dst;
    const ixheaace_temporal_noise_shaping_subblock_info_long *pstr_sfb_info_src;

    pstr_sfb_info_dst = &pstr_tns_data_dst->data_raw.tns_data_long.sub_block_info;
    pstr_sfb_info_src = &pstr_tns_data_src->data_raw.tns_data_long.sub_block_info;
    FLOAT32 tmp_var = pstr_sfb_info_dst->prediction_gain;
    if (fabs(tmp_var - pstr_sfb_info_src->prediction_gain) < (tmp_var * 0.03f)) {
      pstr_sfb_info_dst->tns_active = pstr_sfb_info_src->tns_active;

      memcpy(pstr_sfb_info_dst->parcor, pstr_sfb_info_src->parcor,
             tns_config.max_order * sizeof(pstr_sfb_info_dst->parcor[0]));
    }
  } else {
    ixheaace_temporal_noise_shaping_subblock_info_short *pstr_sfb_info_dst;
    const ixheaace_temporal_noise_shaping_subblock_info_short *pstr_sfb_info_src;
    pstr_sfb_info_dst = &pstr_tns_data_dst->data_raw.tns_data_short.sub_block_info[sub_blk_num];
    pstr_sfb_info_src = &pstr_tns_data_src->data_raw.tns_data_short.sub_block_info[sub_blk_num];

    FLOAT32 tmp_var = pstr_sfb_info_dst->prediction_gain;
    if (fabs(tmp_var - pstr_sfb_info_src->prediction_gain) < (tmp_var * 0.03f)) {
      pstr_sfb_info_dst->tns_active = pstr_sfb_info_src->tns_active;

      memcpy(pstr_sfb_info_dst->parcor, pstr_sfb_info_src->parcor,
             tns_config.max_order * sizeof(pstr_sfb_info_dst->parcor[0]));
    }
  }
}

static WORD32 ia_enhaacplus_enc_index_search3(
    FLOAT32 parcor, ixheaace_temporal_noise_shaping_tables *pstr_tns_tab) {
  WORD32 index = 0;
  WORD32 i;

  for (i = 0; i < 8; i++) {
    if (parcor > pstr_tns_tab->tns_coeff_3_borders[i]) {
      index = i;
    }
  }

  return (index - 4);
}
static WORD32 ia_enhaacplus_enc_index_search4(
    FLOAT32 parcor, ixheaace_temporal_noise_shaping_tables *pstr_tns_tab) {
  WORD32 index = 0;
  WORD32 i;
  for (i = 0; i < 16; i++) {
    if (parcor > pstr_tns_tab->tns_coeff_4_borders[i]) {
      index = i;
    }
  }

  return (index - 8);
}

static VOID ia_enhaacplus_enc_parcor_to_index(
    const FLOAT32 *ptr_parcor, WORD32 *ptr_index, WORD32 order, WORD32 bits_per_coeff,
    ixheaace_temporal_noise_shaping_tables *pstr_tns_tab) {
  WORD32 i;

  if (bits_per_coeff == 3) {
    for (i = order - 1; i >= 0; i--) {
      ptr_index[i] = ia_enhaacplus_enc_index_search3(ptr_parcor[i], pstr_tns_tab);
    }
  } else {
    for (i = order - 1; i >= 0; i--) {
      ptr_index[i] = ia_enhaacplus_enc_index_search4(ptr_parcor[i], pstr_tns_tab);
    }
  }
}

static VOID ia_enhaacplus_enc_index_to_parcor(
    const WORD32 *ptr_index, FLOAT32 *ptr_parcor, WORD32 order, WORD32 bits_per_coeff,
    ixheaace_temporal_noise_shaping_tables *pstr_tns_tab) {
  WORD32 i;

  if (bits_per_coeff == 4) {
    for (i = order - 1; i >= 0; i--) {
      ptr_parcor[i] = pstr_tns_tab->tns_coeff_4[ptr_index[i] + 8];
    }
  } else {
    for (i = order - 1; i >= 0; i--) {
      ptr_parcor[i] = pstr_tns_tab->tns_coeff_3[ptr_index[i] + 4];
    }
  }
}

VOID ia_enhaacplus_enc_tns_encode(ixheaace_temporal_noise_shaping_params *pstr_tns_info,
                                    ixheaace_temporal_noise_shaping_data *pstr_tns_data,
                                    WORD32 num_sfb,
                                    ixheaace_temporal_noise_shaping_config tns_config,
                                    WORD32 lowpass_line, FLOAT32 *ptr_spectrum,
                                    WORD32 sub_blk_num, WORD32 block_type,
                                    ixheaace_temporal_noise_shaping_tables *pstr_tns_tab) {
  WORD32 i;
  FLOAT32 *ptr_parcor;

  if (block_type != SHORT_WINDOW) {
    if (pstr_tns_data->data_raw.tns_data_long.sub_block_info.tns_active == 0) {
      pstr_tns_info->tns_active[sub_blk_num] = 0;
      return;
    } else {
      ia_enhaacplus_enc_parcor_to_index(
          pstr_tns_data->data_raw.tns_data_long.sub_block_info.parcor, &pstr_tns_info->coef[0],
          tns_config.max_order, tns_config.coef_res, pstr_tns_tab);

      ia_enhaacplus_enc_index_to_parcor(
          &pstr_tns_info->coef[0], pstr_tns_data->data_raw.tns_data_long.sub_block_info.parcor,
          tns_config.max_order, tns_config.coef_res, pstr_tns_tab);

      ptr_parcor =
          &pstr_tns_data->data_raw.tns_data_long.sub_block_info.parcor[tns_config.max_order - 1];

      for (i = tns_config.max_order - 1; i >= 0; i--) {
        FLOAT32 tmp_var = *ptr_parcor--;

        if (tmp_var > 0.1f || tmp_var < -0.1f) {
          break;
        }
      }
      pstr_tns_info->order[sub_blk_num] = i + 1;

      pstr_tns_info->tns_active[sub_blk_num] = 1;

      for (i = sub_blk_num + 1; i < TRANS_FAC; i++) {
        pstr_tns_info->tns_active[i] = 0;
      }

      pstr_tns_info->coef_res[sub_blk_num] = (WORD8)tns_config.coef_res;

      pstr_tns_info->length[sub_blk_num] = num_sfb - tns_config.tns_start_band;

      ia_enhaacplus_enc_analysis_filter_lattice(
          &(ptr_spectrum[tns_config.tns_start_line]),
          MIN(tns_config.tns_stop_line, lowpass_line) - tns_config.tns_start_line,
          pstr_tns_data->data_raw.tns_data_long.sub_block_info.parcor,
          pstr_tns_info->order[sub_blk_num], &(ptr_spectrum[tns_config.tns_start_line]));
    }
  } else /*short block*/
  {
    if (pstr_tns_data->data_raw.tns_data_short.sub_block_info[sub_blk_num].tns_active == 0) {
      pstr_tns_info->tns_active[sub_blk_num] = 0;

      return;
    } else {
      ia_enhaacplus_enc_parcor_to_index(
          pstr_tns_data->data_raw.tns_data_short.sub_block_info[sub_blk_num].parcor,
          &pstr_tns_info->coef[sub_blk_num * TEMPORAL_NOISE_SHAPING_MAX_ORDER_SHORT],
          tns_config.max_order, tns_config.coef_res, pstr_tns_tab);

      ia_enhaacplus_enc_index_to_parcor(
          &pstr_tns_info->coef[sub_blk_num * TEMPORAL_NOISE_SHAPING_MAX_ORDER_SHORT],
          pstr_tns_data->data_raw.tns_data_short.sub_block_info[sub_blk_num].parcor,
          tns_config.max_order, tns_config.coef_res, pstr_tns_tab);

      ptr_parcor = &pstr_tns_data->data_raw.tns_data_short.sub_block_info[sub_blk_num]
                        .parcor[tns_config.max_order - 1];
      for (i = tns_config.max_order - 1; i >= 0; i--) {
        FLOAT32 tmp_var = *ptr_parcor--;
        if (tmp_var > 0.1f || tmp_var < -0.1f) {
          break;
        }
      }

      pstr_tns_info->order[sub_blk_num] = i + 1;

      pstr_tns_info->tns_active[sub_blk_num] = 1;

      pstr_tns_info->coef_res[sub_blk_num] = (WORD8)tns_config.coef_res;

      pstr_tns_info->length[sub_blk_num] = num_sfb - tns_config.tns_start_band;

      ia_enhaacplus_enc_analysis_filter_lattice(
          &(ptr_spectrum[tns_config.tns_start_line]),
          tns_config.tns_stop_line - tns_config.tns_start_line,
          pstr_tns_data->data_raw.tns_data_short.sub_block_info[sub_blk_num].parcor,
          pstr_tns_info->order[sub_blk_num], &(ptr_spectrum[tns_config.tns_start_line]));
    }
  }

  return;
}

VOID ia_enhaacplus_enc_apply_tns_mult_table_to_ratios(WORD32 start_cb, WORD32 stop_cb,
                                                      FLOAT32 *ptr_thresholds) {
  WORD32 i;

  for (i = start_cb; i < stop_cb; i++) {
    ptr_thresholds[i] = (FLOAT32)(ptr_thresholds[i] * 0.25f);
  }
}
