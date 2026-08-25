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
#include <string.h>
#include "ixheaace_mps_common_define.h"
#include "ixheaac_constants.h"
#include "iusace_cnst.h"
#include "ixheaac_type_def.h"
#include "iusace_bitbuffer.h"
#include "iusace_tns_usac.h"
#include "iusace_psy_mod.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaac_basic_ops.h"
#include "ixheaac_error_standards.h"
#include "ixheaace_error_codes.h"

static const WORD32 iusace_tns_supported_sampling_rates[13] = {
    96000, 88200, 64000, 48000, 44100, 32000, 24000, 22050, 16000, 12000, 11025, 8000, 0};

static const UWORD16 iusace_tns_min_band_number_long[12] = {11, 12, 15, 16, 17, 20,
                                                            25, 26, 24, 28, 30, 31};

static const UWORD16 iusace_tns_min_band_number_short[12] = {2, 2, 2, 3,  3,  4,
                                                             6, 6, 8, 10, 10, 12};

static const WORD32 iusace_tns_max_bands_table[16][2] = {{31, 9},  /**< 96000 */
                                                         {31, 9},  /**< 88200 */
                                                         {34, 10}, /**< 64000 */
                                                         {40, 14}, /**< 48000 */
                                                         {42, 14}, /**< 44100 */
                                                         {51, 14}, /**< 32000 */
                                                         {47, 15}, /**< 24000 */
                                                         {47, 15}, /**< 22050 */
                                                         {43, 15}, /**< 16000 */
                                                         {43, 15}, /**< 12000 */
                                                         {43, 15}, /**< 11025 */
                                                         {40, 15}, /**< 8000  */
                                                         {40, 15}, /**< 7350  */
                                                         {0, 0},   {0, 0}, {0, 0}};

static WORD32 iusace_freq_to_band_mapping(WORD32 freq, WORD32 sample_rate, WORD32 num_bands,
                                          const WORD32 *ptr_band_start_offset) {
  WORD32 line_num, band;

  line_num = (freq * ptr_band_start_offset[num_bands] * 4 / sample_rate + 1) / 2;

  if (line_num >= ptr_band_start_offset[num_bands]) {
    return num_bands;
  }

  for (band = 0; band < num_bands; band++) {
    if (ptr_band_start_offset[band + 1] > line_num) break;
  }

  if (line_num - ptr_band_start_offset[band] > ptr_band_start_offset[band + 1] - line_num) {
    band++;
  }

  return band;
};

static VOID iusace_calc_gauss_win(FLOAT64 *ptr_win, const WORD32 length, const WORD32 sample_rate,
                                  const WORD32 win_seq, const FLOAT32 time_resolution) {
  WORD32 i;
  FLOAT32 gauss_exp = 3.14159265358979323f * sample_rate * 0.001f * (FLOAT32)time_resolution /
                      (win_seq != EIGHT_SHORT_SEQUENCE ? 1024.0f : 128.0f);

  gauss_exp = -0.5f * gauss_exp * gauss_exp;

  for (i = 0; i < length; i++) {
    ptr_win[i] = (FLOAT32)exp(gauss_exp * (i + 0.5) * (i + 0.5));
  }
  return;
}

IA_ERRORCODE iusace_tns_init(WORD32 sampling_rate, WORD32 bit_rate, ia_tns_info *tns_info,
                             WORD32 num_channels) {
  IA_ERRORCODE err_code = IA_NO_ERROR;
  WORD32 fs_index = 0;
  WORD32 lpc_stop_freq = 16000;
  WORD32 lpc_start_freq_long = 2500, lpc_start_freq_short = 3750;
  tns_info->threshold = 1.41f;
  tns_info->tns_time_res_short = 0.6f;
  tns_info->tns_time_res_long = 0.6f;

  if (sampling_rate == 14700) {
    sampling_rate = 16000;
  }
  if (sampling_rate == 29400) {
    sampling_rate = 32000;
  }

  if (bit_rate < 32000) {
    if (num_channels == 1) {
      tns_info->threshold = 1.2f;
      lpc_start_freq_long = 2000;
    }
  } else if (bit_rate < 36000) {
    if (num_channels == 1) {
      tns_info->tns_time_res_long = 0.8f;
    } else {
      tns_info->tns_time_res_long = 0.5f;
    }
    tns_info->tns_time_res_short = 0.3f;
  } else {
    tns_info->tns_time_res_long = 0.5f;
    tns_info->tns_time_res_short = 0.3f;
  }

  /** Determine if sampling rate is supported
   */
  while (sampling_rate != iusace_tns_supported_sampling_rates[fs_index]) {
    if (!iusace_tns_supported_sampling_rates[fs_index]) {
      return IA_EXHEAACE_INIT_FATAL_USAC_INVALID_CORE_SAMPLE_RATE;
    }
    fs_index++;
  }

  tns_info->tns_max_bands_long = iusace_tns_max_bands_table[fs_index][0];
  tns_info->tns_max_bands_short = iusace_tns_max_bands_table[fs_index][1];
  tns_info->tns_max_order_long = 15;
  tns_info->tns_max_order_short = 7;

  tns_info->tns_min_band_number_long = iusace_tns_min_band_number_long[fs_index];
  tns_info->tns_min_band_number_short = iusace_tns_min_band_number_short[fs_index];

  tns_info->lpc_start_band_long =
      iusace_freq_to_band_mapping(lpc_start_freq_long, sampling_rate, tns_info->max_sfb_long,
                                  tns_info->sfb_offset_table_long);

  tns_info->lpc_start_band_short =
      iusace_freq_to_band_mapping(lpc_start_freq_short, sampling_rate, tns_info->max_sfb_short,
                                  tns_info->sfb_offset_table_short);

  tns_info->lpc_stop_band_long = iusace_freq_to_band_mapping(
      lpc_stop_freq, sampling_rate, tns_info->max_sfb_long, tns_info->sfb_offset_table_long);

  tns_info->lpc_stop_band_short = iusace_freq_to_band_mapping(
      lpc_stop_freq, sampling_rate, tns_info->max_sfb_short, tns_info->sfb_offset_table_short);

  iusace_calc_gauss_win(tns_info->win_long, tns_info->tns_max_order_long + 1, sampling_rate,
                        ONLY_LONG_SEQUENCE, tns_info->tns_time_res_long);

  iusace_calc_gauss_win(tns_info->win_short, tns_info->tns_max_order_short + 1, sampling_rate,
                        EIGHT_SHORT_SEQUENCE, tns_info->tns_time_res_short);
  return err_code;
}

VOID iusace_tns_filter(WORD32 length, FLOAT64 *spec, ia_tns_filter_data *filter,
                       FLOAT64 *scratch_tns_filter) {
  WORD32 i, j, k = 0;
  WORD32 order = filter->order;
  FLOAT64 *a = filter->a_coeffs;
  FLOAT64 *temp = scratch_tns_filter;

  /** Determine loop parameters for given direction
   */
  if (filter->direction) {
    /** Startup, initial state is zero
     */
    temp[length - 1] = spec[length - 1];
    for (i = length - 2; i > (length - 1 - order); i--) {
      temp[i] = spec[i];
      k++;
      for (j = 1; j <= k; j++) {
        spec[i] += temp[i + j] * a[j];
      }
    }

    /** Now filter the rest
     */
    for (i = length - 1 - order; i >= 0; i--) {
      temp[i] = spec[i];
      for (j = 1; j <= order; j++) {
        spec[i] += temp[i + j] * a[j];
      }
    }
  } else {
    /** Startup, initial state is zero
     */
    temp[0] = spec[0];
    for (i = 1; i < order; i++) {
      temp[i] = spec[i];
      for (j = 1; j <= i; j++) {
        spec[i] += temp[i - j] * a[j];
      }
    }

    /** Now filter the rest
     */
    for (i = order; i < length; i++) {
      temp[i] = spec[i];
      for (j = 1; j <= order; j++) {
        spec[i] += temp[i - j] * a[j];
      }
    }
  }

  return;
}

static WORD32 iusace_truncate_coeffs(WORD32 f_order, FLOAT64 threshold, FLOAT64 *k_array) {
  WORD32 i;
  for (i = f_order; i >= 0; i--) {
    k_array[i] = (fabs(k_array[i]) > threshold) ? k_array[i] : 0.0;
    if (k_array[i] != 0.0) {
      return i;
    }
  }
  return 0;
}

VOID iusace_quantize_reflection_coeffs(WORD32 f_order, WORD32 coeff_res, FLOAT64 *k_array,
                                       WORD32 *index_array) {
  FLOAT64 iqfac, iqfac_m;
  WORD32 i;

  iqfac = (((SIZE_T)1 << (coeff_res - 1)) - 0.5) / (PI / 2);
  iqfac_m = (((SIZE_T)1 << (coeff_res - 1)) + 0.5) / (PI / 2);

  /* Quantize and inverse quantize */
  for (i = 1; i <= f_order; i++) {
    index_array[i] = (WORD32)(0.5 + (asin(k_array[i]) * ((k_array[i] >= 0) ? iqfac : iqfac_m)));
    k_array[i] = sin((FLOAT64)index_array[i] / ((index_array[i] >= 0) ? iqfac : iqfac_m));
  }
  return;
}

VOID iusace_tns_auto_corr(WORD32 max_order, WORD32 data_size, FLOAT64 *data, FLOAT64 *r_array) {
  WORD32 i, j;
  FLOAT64 tmp_var;
  for (i = 0; i < data_size; i += 2) {
    const FLOAT64 *input1 = &data[i];
    FLOAT64 temp1 = *input1;
    FLOAT64 temp2 = *(input1 + 1);
    FLOAT64 inp_tmp1 = *input1++;
    for (j = 0; j <= max_order; j++) {
      FLOAT64 inp_tmp2;
      tmp_var = temp1 * inp_tmp1;
      inp_tmp2 = *input1++;
      tmp_var += temp2 * inp_tmp2;
      r_array[j] += tmp_var;
      j++;
      tmp_var = temp1 * inp_tmp2;
      inp_tmp1 = *input1++;
      tmp_var += temp2 * inp_tmp1;
      r_array[j] += tmp_var;
    }
  }
  return;
}

static FLOAT64 iusace_levinson_durbin(WORD32 order, WORD32 data_size, FLOAT64 *ptr_data,
                                      FLOAT64 *ptr_k, FLOAT64 *ptr_win, FLOAT64 *ptr_scratch) {
  WORD32 i, j;
  FLOAT64 *ptr_work_buffer_temp;
  FLOAT64 *ptr_work_buffer = ptr_scratch;
  FLOAT64 *ptr_input = ptr_scratch + TNS_MAX_ORDER + 1;
  memset(ptr_input, 0, (TNS_MAX_ORDER + 1) * sizeof(ptr_input[0]));
  iusace_tns_auto_corr(order, data_size, ptr_data, ptr_input);

  WORD32 num_of_coeff = order;
  FLOAT64 *ptr_refl_coeff = ptr_k;
  ptr_k[0] = 1.0;

  if (ptr_input[0] == 0) {
    return 0;
  }

  for (i = 0; i < num_of_coeff + 1; i++) {
    ptr_input[i] = ptr_input[i] * ptr_win[i];
  }

  FLOAT64 tmp_var;
  ptr_work_buffer[0] = ptr_input[0];

  for (i = 1; i < num_of_coeff; i++) {
    tmp_var = ptr_input[i];
    ptr_work_buffer[i] = tmp_var;
    ptr_work_buffer[i + num_of_coeff - 1] = tmp_var;
  }
  ptr_work_buffer[i + num_of_coeff - 1] = ptr_input[i];

  for (i = 0; i < num_of_coeff; i++) {
    FLOAT64 refc, tmp;
    tmp = ptr_work_buffer[num_of_coeff + i];
    if (tmp < 0) {
      tmp = -tmp;
    } else {
      if (ptr_work_buffer[0] < tmp) {
        break;
      }
    }
    if (ptr_work_buffer[0] == 0) {
      refc = 0;
    } else {
      refc = tmp / ptr_work_buffer[0];
    }

    if (ptr_work_buffer[num_of_coeff + i] > 0) {
      refc = -refc;
    }
    ptr_refl_coeff[i + 1] = refc;
    ptr_work_buffer_temp = &(ptr_work_buffer[num_of_coeff]);

    for (j = i; j < num_of_coeff; j++) {
      FLOAT64 accu1, accu2;
      accu1 = refc * ptr_work_buffer[j - i];
      accu1 += ptr_work_buffer_temp[j];
      accu2 = refc * ptr_work_buffer_temp[j];
      accu2 += ptr_work_buffer[j - i];
      ptr_work_buffer_temp[j] = accu1;
      ptr_work_buffer[j - i] = accu2;
    }
  }
  return (ptr_input[0] / ptr_work_buffer[0]);
}

static VOID iusace_step_up(WORD32 f_order, FLOAT64 *ptr_k, FLOAT64 *ptr_a, FLOAT64 *ptr_scratch) {
  FLOAT64 *ptr_a_temp = ptr_scratch;
  WORD32 i, order;

  ptr_a[0] = 1.0;
  ptr_a_temp[0] = 1.0;
  for (order = 1; order <= f_order; order++) {
    ptr_a[order] = 0.0;
    for (i = 1; i <= order; i++) {
      ptr_a_temp[i] = ptr_a[i] + ptr_k[order] * ptr_a[order - i];
    }
    for (i = 1; i <= order; i++) {
      ptr_a[i] = ptr_a_temp[i];
    }
  }
  return;
}

static VOID iusace_calc_weighted_spec(FLOAT64 *ptr_spec, FLOAT64 *ptr_wgt_spec,
                                      FLOAT32 *ptr_sfb_en, WORD32 *ptr_sfb_offset,
                                      WORD32 lpc_start_band, WORD32 lpc_stop_band,
                                      FLOAT64 *ptr_scratch) {
  WORD32 i, sfb;
  FLOAT32 temp;
  FLOAT32 *ptr_tns_sfb_mean = (FLOAT32 *)ptr_scratch;
  memset(ptr_scratch, 0, MAX_NUM_GROUPED_SFB * sizeof(ptr_tns_sfb_mean[0]));
  WORD32 lpc_stop_line = ptr_sfb_offset[lpc_stop_band];
  WORD32 lpc_start_line = ptr_sfb_offset[lpc_start_band];

  for (sfb = lpc_start_band; sfb < lpc_stop_band; sfb++) {
    ptr_tns_sfb_mean[sfb] = (FLOAT32)(1.0 / sqrt(ptr_sfb_en[sfb] + 1e-30f));
  }

  sfb = lpc_start_band;
  temp = ptr_tns_sfb_mean[sfb];

  for (i = lpc_start_line; i < lpc_stop_line; i++) {
    if (ptr_sfb_offset[sfb + 1] == i) {
      sfb++;

      if (sfb + 1 < lpc_stop_band) {
        temp = ptr_tns_sfb_mean[sfb];
      }
    }
    ptr_wgt_spec[i] = temp;
  }

  for (i = lpc_stop_line - 2; i >= lpc_start_line; i--) {
    ptr_wgt_spec[i] = (ptr_wgt_spec[i] + ptr_wgt_spec[i + 1]) * 0.5f;
  }

  for (i = lpc_start_line + 1; i < lpc_stop_line; i++) {
    ptr_wgt_spec[i] = (ptr_wgt_spec[i] + ptr_wgt_spec[i - 1]) * 0.5f;
  }

  for (i = lpc_start_line; i < lpc_stop_line; i++) {
    ptr_wgt_spec[i] = ptr_wgt_spec[i] * ptr_spec[i];
  }
  return;
}

VOID iusace_tns_data_sync(ia_tns_info *ptr_tns_dest, ia_tns_info *ptr_tns_src, const WORD32 w,
                          WORD32 order) {
  ia_tns_window_data *win_data_src = &ptr_tns_src->window_data[w];
  ia_tns_window_data *win_data_dest = &ptr_tns_dest->window_data[w];
  WORD32 i;
  if (fabs(win_data_dest->tns_pred_gain - win_data_src->tns_pred_gain) <
      ((FLOAT32)0.03f * win_data_dest->tns_pred_gain)) {
    win_data_dest->tns_active = win_data_src->tns_active;

    for (i = 0; i < order; i++) {
      win_data_dest->tns_filter->k_coeffs[i] = win_data_src->tns_filter->k_coeffs[i];
    }
  }
  return;
}

VOID iusace_tns_encode(ia_tns_info *pstr_tns_info_ch2, ia_tns_info *pstr_tns_info,
                       FLOAT32 *ptr_sfb_energy, WORD32 w, WORD32 i_ch, WORD32 low_pass_line,
                       FLOAT64 *ptr_scratch_tns_filter, WORD32 core_mode,
                       FLOAT64 *ptr_tns_scratch) {
  WORD32 number_of_bands = pstr_tns_info->number_of_bands;
  WORD32 block_type = pstr_tns_info->block_type;
  FLOAT64 *ptr_spec = pstr_tns_info->spec;
  WORD32 start_band, stop_band, order; /**< bands over which to apply TNS */
  WORD32 length_in_bands;              /**< Length to filter, in bands */
  WORD32 start_index, length;
  WORD32 nbands;
  WORD32 coeff_res;
  FLOAT64 *ptr_weighted_spec = ptr_tns_scratch;
  memset(ptr_weighted_spec, 0, 4096 * sizeof(ptr_weighted_spec[0]));
  FLOAT64 *ptr_scratch = ptr_tns_scratch + 4096;
  FLOAT64 *ptr_window = NULL;
  WORD32 lpc_start_band, lpc_stop_band;
  WORD32 *ptr_sfb_offset_table;

  switch (block_type) {
    case EIGHT_SHORT_SEQUENCE:
      start_band = pstr_tns_info->tns_min_band_number_short;
      stop_band = number_of_bands;
      length_in_bands = stop_band - start_band;
      order = pstr_tns_info->tns_max_order_short;
      start_band = MIN(start_band, pstr_tns_info->tns_max_bands_short);
      stop_band = MIN(stop_band, pstr_tns_info->tns_max_bands_short);
      coeff_res = 3;
      ptr_window = pstr_tns_info->win_short;
      nbands = pstr_tns_info->max_sfb_short;
      lpc_start_band = pstr_tns_info->lpc_start_band_short;
      lpc_stop_band = pstr_tns_info->lpc_stop_band_short;
      if (core_mode == CORE_MODE_FD) {
        ptr_sfb_offset_table = pstr_tns_info->sfb_offset_table_short;
      } else {
        ptr_sfb_offset_table = pstr_tns_info->sfb_offset_table_short_tcx;
      }
      break;

    default:
      start_band = pstr_tns_info->tns_min_band_number_long;
      stop_band = number_of_bands;
      length_in_bands = stop_band - start_band;
      order = pstr_tns_info->tns_max_order_long;
      start_band = MIN(start_band, pstr_tns_info->tns_max_bands_long);
      stop_band = MIN(stop_band, pstr_tns_info->tns_max_bands_long);
      coeff_res = 4;
      ptr_window = pstr_tns_info->win_long;
      nbands = pstr_tns_info->max_sfb_long;
      lpc_start_band = pstr_tns_info->lpc_start_band_long;
      lpc_stop_band = pstr_tns_info->lpc_stop_band_long;
      ptr_sfb_offset_table = pstr_tns_info->sfb_offset_table_long;
      break;
  }

  /** Make sure that start and stop bands < max_sfb
   * Make sure that start and stop bands >= 0
   */
  start_band = MIN(start_band, nbands);
  stop_band = MIN(stop_band, nbands);
  start_band = MAX(start_band, 0);
  stop_band = MAX(stop_band, 0);

  pstr_tns_info->tns_data_present = 0; /**< default TNS not used */

  /** Perform analysis and filtering for each window
   */
  {
    ia_tns_window_data *window_data = &pstr_tns_info->window_data[w];
    ia_tns_filter_data *tns_filter = window_data->tns_filter;
    FLOAT64 *k = tns_filter->k_coeffs; /**< reflection coeffs */
    FLOAT64 *a = tns_filter->a_coeffs; /**< prediction coeffs */

    iusace_calc_weighted_spec(ptr_spec, ptr_weighted_spec, ptr_sfb_energy, ptr_sfb_offset_table,
                              lpc_start_band, lpc_stop_band, ptr_scratch);

    window_data->n_filt = 0;
    window_data->coef_res = coeff_res;

    start_index = ptr_sfb_offset_table[lpc_start_band];
    length =
        ptr_sfb_offset_table[lpc_stop_band] -
        ptr_sfb_offset_table[lpc_start_band]; /**< The length of the spectral data to be
                                                                                         processed
                                               */

    window_data->tns_pred_gain = iusace_levinson_durbin(
        order, length, &ptr_weighted_spec[start_index], k, ptr_window, ptr_scratch);

    window_data->tns_active = 0;
    if (window_data->tns_pred_gain > DEF_TNS_GAIN_THRESH) {
      window_data->tns_active = 1;
    }

    if (i_ch == 1) {
      iusace_tns_data_sync(pstr_tns_info, pstr_tns_info_ch2, w, order);
    }

    if (window_data->tns_pred_gain > DEF_TNS_GAIN_THRESH) {
      /** Use TNS
       */
      WORD32 truncated_order;
      window_data->n_filt++;
      pstr_tns_info->tns_data_present = 1;
      tns_filter->direction = 0;
      tns_filter->coef_compress = 0;
      tns_filter->length = length_in_bands;
      iusace_quantize_reflection_coeffs(order, coeff_res, k, tns_filter->index);
      truncated_order = iusace_truncate_coeffs(order, DEF_TNS_COEFF_THRESH, k);
      tns_filter->order = truncated_order;
      iusace_step_up(truncated_order, k, a, ptr_scratch); /**< Compute prediction coefficients */
      start_index = ptr_sfb_offset_table[start_band];
      length = MIN(ptr_sfb_offset_table[stop_band], low_pass_line) - start_index;
      if (block_type == EIGHT_SHORT_SEQUENCE) {
        length = ptr_sfb_offset_table[stop_band] - start_index;
      }
      iusace_tns_filter(length, &ptr_spec[start_index], tns_filter,
                        ptr_scratch_tns_filter); /**< filter */
    }
  }
  return;
}
