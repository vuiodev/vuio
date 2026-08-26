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

#include <math.h>
#include <stdlib.h>

#include "ixheaac_type_def.h"
#include "ixheaac_constants.h"
#include "ixheaace_aac_constants.h"
#include "ixheaac_error_standards.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops16.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaac_basic_ops.h"

#include "ixheaace_common_rom.h"
#include "ixheaace_psy_const.h"
#include "ixheaace_tns.h"
#include "ixheaace_tns_params.h"
#include "ixheaace_rom.h"
#include "ixheaace_bitbuffer.h"
#include "ixheaace_psy_configuration.h"
#include "ixheaace_tns_func.h"
#include "ixheaace_common_utils.h"

static WORD32 ia_enhaacplus_enc_freq_to_band_with_rounding(WORD32 freq, WORD32 fs,
                                                           WORD32 num_bands,
                                                           const WORD32 *ptr_band_start_offset) {
  WORD32 line_num, band;

  line_num = (freq * ptr_band_start_offset[num_bands] * 4 / fs + 1) / 2;

  /* freq > fs/2 */
  if (line_num >= ptr_band_start_offset[num_bands]) {
    return num_bands;
  }

  /* find band the line number lies in */
  /* bandStartOffset[] */
  for (band = 0; band < num_bands; band++) {
    if (ptr_band_start_offset[band + 1] > line_num) break;
  }

  /* round to nearest band border */
  if (line_num - ptr_band_start_offset[band] > ptr_band_start_offset[band + 1] - line_num) {
    band++;
  }

  return band;
}

static VOID ia_enhaacplus_enc_calc_gauss_window(FLOAT32 *ptr_win, const WORD16 win_size,
                                                const WORD32 sampling_rate,
                                                const WORD16 block_type,
                                                const FLOAT32 time_resolution) {
  WORD16 i;

  FLOAT32 accu_gauss_exp;
  accu_gauss_exp = (sampling_rate * time_resolution) * PI_BY_1000;

  if (block_type != SHORT_WINDOW) {
    accu_gauss_exp = (FLOAT32)(accu_gauss_exp / FRAME_LEN_1024);
  } else {
    accu_gauss_exp = (FLOAT32)(accu_gauss_exp / FRAME_LEN_SHORT_128);
  }

  accu_gauss_exp = -(accu_gauss_exp * 0.5f * accu_gauss_exp);

  for (i = 0; i < win_size; i++) {
    ptr_win[i] = (FLOAT32)exp(accu_gauss_exp * (i + 0.5) * (i + 0.5));
  }
}

IA_ERRORCODE
ia_enhaacplus_enc_init_tns_configuration(WORD32 bit_rate, WORD32 sample_rate, WORD32 channels,
                                         ixheaace_temporal_noise_shaping_config *pstr_tns_config,
                                         ixheaace_psy_configuration_long *pstr_psy_config,
                                         WORD32 active,
                                         ixheaace_temporal_noise_shaping_tables *pstr_tns_tab,
                                         WORD32 frame_length, WORD32 aot)

{
  IA_ERRORCODE error;
  pstr_tns_config->max_order = TEMPORAL_NOISE_SHAPING_MAX_ORDER;
  pstr_tns_config->tns_start_freq = TEMPORAL_NOISE_SHAPING_START_FREQ;
  pstr_tns_config->coef_res = TEMPORAL_NOISE_SHAPING_COEF_RES;

  error = ia_enhaacplus_enc_get_tns_param(&pstr_tns_config->conf_tab, bit_rate / channels,
                                          channels, LONG_WINDOW, pstr_tns_tab->tns_info_tab,
                                          sizeof(pstr_tns_tab->tns_info_tab));

  if (error != IA_NO_ERROR) {
    return error;
  }

  ia_enhaacplus_enc_calc_gauss_window(pstr_tns_config->acf_window_float,
                                      (const WORD16)(pstr_tns_config->max_order + 1), sample_rate,
                                      LONG_WINDOW, pstr_tns_config->conf_tab.tns_time_resolution

  );

  ia_enhaacplus_enc_get_tns_max_bands(
      sample_rate, LONG_WINDOW, &pstr_tns_config->tns_max_sfb, pstr_tns_tab->tns_max_bands_table,
      sizeof(pstr_tns_tab->tns_max_bands_table), aot, frame_length);

  pstr_tns_config->tns_active = 1;

  if (active == 0) {
    pstr_tns_config->tns_active = 0;
  }

  /*now calc band and line borders */
  pstr_tns_config->tns_stop_band = MIN(pstr_psy_config->sfb_cnt, pstr_tns_config->tns_max_sfb);

  pstr_tns_config->tns_stop_line = pstr_psy_config->sfb_offsets[pstr_tns_config->tns_stop_band];

  pstr_tns_config->tns_start_band = ia_enhaacplus_enc_freq_to_band_with_rounding(
      pstr_tns_config->tns_start_freq, sample_rate, pstr_psy_config->sfb_cnt,
      pstr_psy_config->sfb_offsets);

  pstr_tns_config->tns_modify_begin_cb = ia_enhaacplus_enc_freq_to_band_with_rounding(
      TEMPORAL_NOISE_SHAPING_MODIFY_BEGIN, sample_rate, pstr_psy_config->sfb_cnt,
      pstr_psy_config->sfb_offsets);

  pstr_tns_config->tns_ratio_patch_lowest_cb = ia_enhaacplus_enc_freq_to_band_with_rounding(
      RATIO_PATCH_LOWER_BORDER, sample_rate, pstr_psy_config->sfb_cnt,
      pstr_psy_config->sfb_offsets);

  pstr_tns_config->tns_start_line = pstr_psy_config->sfb_offsets[pstr_tns_config->tns_start_band];

  pstr_tns_config->lpc_stop_band = ia_enhaacplus_enc_freq_to_band_with_rounding(
      pstr_tns_config->conf_tab.lpc_stop_freq, sample_rate, pstr_psy_config->sfb_cnt,
      pstr_psy_config->sfb_offsets);

  pstr_tns_config->lpc_stop_band =
      MIN(pstr_tns_config->lpc_stop_band, pstr_psy_config->sfb_active);

  pstr_tns_config->lpc_stop_line = pstr_psy_config->sfb_offsets[pstr_tns_config->lpc_stop_band];
  pstr_tns_config->lpc_start_band = ia_enhaacplus_enc_freq_to_band_with_rounding(
      pstr_tns_config->conf_tab.lpc_start_freq, sample_rate, pstr_psy_config->sfb_cnt,
      pstr_psy_config->sfb_offsets);

  pstr_tns_config->lpc_start_line = pstr_psy_config->sfb_offsets[pstr_tns_config->lpc_start_band];
  pstr_tns_config->threshold = pstr_tns_config->conf_tab.thresh_on;

  return IA_NO_ERROR;
}

IA_ERRORCODE ia_enhaacplus_enc_init_tns_configuration_short(
    WORD32 bit_rate, WORD32 sample_rate, WORD32 channels,
    ixheaace_temporal_noise_shaping_config *pstr_tns_config,
    ixheaace_psy_configuration_short *pstr_psy_config, WORD32 active,
    ixheaace_temporal_noise_shaping_tables *pstr_tns_tab, WORD32 frame_length, WORD32 aot) {
  IA_ERRORCODE error = IA_NO_ERROR;
  pstr_tns_config->max_order = TEMPORAL_NOISE_SHAPING_MAX_ORDER_SHORT;
  pstr_tns_config->tns_start_freq = TEMPORAL_NOISE_SHAPING_START_FREQ_SHORT;
  pstr_tns_config->coef_res = TEMPORAL_NOISE_SHAPING_COEF_RES_SHORT;

  error = ia_enhaacplus_enc_get_tns_param(&pstr_tns_config->conf_tab, bit_rate / channels,
                                          channels, SHORT_WINDOW, pstr_tns_tab->tns_info_tab,
                                          sizeof(pstr_tns_tab->tns_info_tab));
  if (error != IA_NO_ERROR) {
    return error;
  }
  ia_enhaacplus_enc_calc_gauss_window(
      pstr_tns_config->acf_window_float, (const WORD16)(pstr_tns_config->max_order + 1),
      sample_rate, SHORT_WINDOW, pstr_tns_config->conf_tab.tns_time_resolution);

  ia_enhaacplus_enc_get_tns_max_bands(
      sample_rate, SHORT_WINDOW, &pstr_tns_config->tns_max_sfb, pstr_tns_tab->tns_max_bands_table,
      sizeof(pstr_tns_tab->tns_max_bands_table), aot, frame_length);

  pstr_tns_config->tns_active = 1;

  if (active == 0) {
    pstr_tns_config->tns_active = 0;
  }

  /*now calc band and line borders */

  pstr_tns_config->tns_stop_band = MIN(pstr_psy_config->sfb_cnt, pstr_tns_config->tns_max_sfb);

  pstr_tns_config->tns_stop_line = pstr_psy_config->sfb_offsets[pstr_tns_config->tns_stop_band];

  pstr_tns_config->tns_start_band = ia_enhaacplus_enc_freq_to_band_with_rounding(
      pstr_tns_config->tns_start_freq, sample_rate, pstr_psy_config->sfb_cnt,
      pstr_psy_config->sfb_offsets);

  pstr_tns_config->tns_modify_begin_cb = ia_enhaacplus_enc_freq_to_band_with_rounding(
      TEMPORAL_NOISE_SHAPING_MODIFY_BEGIN, sample_rate, pstr_psy_config->sfb_cnt,
      pstr_psy_config->sfb_offsets);

  pstr_tns_config->tns_ratio_patch_lowest_cb = ia_enhaacplus_enc_freq_to_band_with_rounding(
      RATIO_PATCH_LOWER_BORDER, sample_rate, pstr_psy_config->sfb_cnt,
      pstr_psy_config->sfb_offsets);

  pstr_tns_config->tns_start_line = pstr_psy_config->sfb_offsets[pstr_tns_config->tns_start_band];

  pstr_tns_config->lpc_stop_band = ia_enhaacplus_enc_freq_to_band_with_rounding(
      pstr_tns_config->conf_tab.lpc_stop_freq, sample_rate, pstr_psy_config->sfb_cnt,
      pstr_psy_config->sfb_offsets);

  pstr_tns_config->lpc_stop_band =
      MIN(pstr_tns_config->lpc_stop_band, pstr_psy_config->sfb_active);

  pstr_tns_config->lpc_stop_line = pstr_psy_config->sfb_offsets[pstr_tns_config->lpc_stop_band];

  pstr_tns_config->lpc_start_band = ia_enhaacplus_enc_freq_to_band_with_rounding(
      pstr_tns_config->conf_tab.lpc_start_freq, sample_rate, pstr_psy_config->sfb_cnt,
      pstr_psy_config->sfb_offsets);

  pstr_tns_config->lpc_start_line = pstr_psy_config->sfb_offsets[pstr_tns_config->lpc_start_band];
  pstr_tns_config->threshold = pstr_tns_config->conf_tab.thresh_on;

  return IA_NO_ERROR;
}

const WORD32 ia_enhaacplus_enc_m_log2_table[INT_BITS] = {
    0x00000000, 0x4ae00d00, 0x2934f080, 0x15c01a3f, 0x0b31fb80, 0x05aeb4e0, 0x02dcf2d0,
    0x016fe50c, 0x00b84e23, 0x005c3e10, 0x002e24ca, 0x001713d6, 0x000b8a47, 0x0005c53b,
    0x0002e2a3, 0x00017153, 0x0000b8aa, 0x00005c55, 0x00002e2b, 0x00001715, 0x00000b8b,
    0x000005c5, 0x000002e3, 0x00000171, 0x000000b9, 0x0000005c, 0x0000002e, 0x00000017,
    0x0000000c, 0x00000006, 0x00000003, 0x00000001};
