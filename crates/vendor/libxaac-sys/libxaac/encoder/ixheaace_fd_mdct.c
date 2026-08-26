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
#include "iusace_type_def.h"

#include "ixheaace_mps_common_define.h"
#include "iusace_cnst.h"
#include "iusace_bitbuffer.h"
#include "impd_drc_common_enc.h"
#include "impd_drc_uni_drc.h"
#include "impd_drc_api.h"
#include "impd_drc_uni_drc_eq.h"
#include "impd_drc_uni_drc_filter_bank.h"
#include "impd_drc_gain_enc.h"
#include "impd_drc_struct_def.h"

#include "iusace_cnst.h"
#include "iusace_tns_usac.h"
#include "iusace_psy_mod.h"
#include "iusace_ms.h"

#include "ixheaace_adjust_threshold_data.h"
#include "iusace_fd_qc_util.h"
#include "ixheaace_memory_standards.h"
#include "iusace_config.h"
#include "iusace_fft.h"
#include "iusace_arith_enc.h"
#include "iusace_fd_quant.h"
#include "iusace_signal_classifier.h"
#include "iusace_block_switch_const.h"
#include "iusace_block_switch_struct_def.h"
#include "ixheaace_sbr_header.h"
#include "ixheaace_config.h"
#include "ixheaace_asc_write.h"
#include "iusace_main.h"
#include "iusace_windowing.h"
#include "ixheaac_error_standards.h"

static IA_ERRORCODE iusace_fd_mdct_short(ia_usac_data_struct *pstr_usac_data,
                                         ia_usac_encoder_config_struct *pstr_usac_config,
                                         WORD32 ch_idx) {
  IA_ERRORCODE err_code = 0;
  iusace_scratch_mem *pstr_scratch = &pstr_usac_data->str_scratch;
  IA_ERRORCODE err_code_2 = 0;
  FLOAT64 *ptr_windowed_buf = pstr_scratch->p_fd_mdct_windowed_short_buf;
  WORD32 n_long = pstr_usac_config->ccfl;
  WORD32 n_short = pstr_usac_config->ccfl >> 3;
  FLOAT64 *ptr_in_data = pstr_usac_data->ptr_time_data[ch_idx];
  FLOAT64 *ptr_out_mdct = pstr_usac_data->spectral_line_vector[ch_idx];
  FLOAT64 *ptr_out_mdst = pstr_usac_data->mdst_spectrum[ch_idx];
  WORD32 window_shape = pstr_usac_config->window_shape_prev[ch_idx];
  FLOAT64 *ptr_win_gen_medium = NULL, *ptr_win_gen_short = NULL;
  FLOAT64 *ptr_overlap = pstr_usac_data->overlap_buf[ch_idx];
  WORD32 nflat_ls;
  WORD32 i, k;
  WORD32 data_size = (OVERLAP_WIN_SIZE_576 * n_long) / LEN_SUPERFRAME;

  memset(ptr_windowed_buf, 0, 2 * n_short * sizeof(FLOAT64));
  nflat_ls = (n_long - n_short) >> 1;
  err_code = iusace_calc_window(&ptr_win_gen_short, n_short, window_shape);
  if (err_code) return err_code;
  err_code = iusace_calc_window(&ptr_win_gen_medium, n_short, 0);
  if (err_code) return err_code;
  ptr_overlap += nflat_ls;

  for (k = MAX_SHORT_WINDOWS - 1; k-- >= 0;) {
    for (i = 0; i < n_short; i++) {
      ptr_windowed_buf[i] = ptr_win_gen_short[i] * ptr_overlap[i];
    }
    for (i = 0; i < n_short; i++) {
      ptr_windowed_buf[i + n_short] =
          ptr_win_gen_medium[n_short - 1 - i] * ptr_overlap[i + n_short];
    }

    ptr_win_gen_medium = ptr_win_gen_short;

    // Compute MDCT
    err_code = iusace_fft_based_mdct(ptr_windowed_buf, ptr_out_mdct, n_short, MDCT_TX_FLAG,
                                     pstr_scratch);

    if (err_code) {
      return err_code;
    }

    // Compute MDST
    err_code_2 = iusace_fft_based_mdct(ptr_windowed_buf, ptr_out_mdst, n_short, MDST_TX_FLAG,
                                       pstr_scratch);

    if (err_code_2) {
      return err_code_2;
    }

    ptr_out_mdct += n_short;
    ptr_out_mdst += n_short;
    ptr_overlap += n_short;
  }

  ptr_overlap = pstr_usac_data->overlap_buf[ch_idx];
  memcpy(ptr_overlap, ptr_overlap + n_long, data_size * sizeof(*ptr_overlap));
  memcpy(ptr_overlap + data_size, ptr_in_data, n_long * sizeof(*ptr_overlap));

  return err_code;
}

static IA_ERRORCODE iusace_fd_mdct_long(ia_usac_data_struct *pstr_usac_data,
                                        ia_usac_encoder_config_struct *pstr_usac_config,
                                        WORD32 ch_idx, WORD32 window_sequence) {
  IA_ERRORCODE err_code = 0;
  iusace_scratch_mem *pstr_scratch = &pstr_usac_data->str_scratch;
  IA_ERRORCODE err_code_2 = 0;
  FLOAT64 *ptr_windowed_buf = pstr_scratch->p_fd_mdct_windowed_long_buf;
  WORD32 n_long = pstr_usac_config->ccfl;
  WORD32 n_short = pstr_usac_config->ccfl >> 3;
  WORD32 prev_mode = (pstr_usac_data->core_mode_prev[ch_idx] == CORE_MODE_TD);
  WORD32 next_mode = (pstr_usac_data->core_mode_next[ch_idx] == CORE_MODE_TD);
  FLOAT64 *ptr_in_data = pstr_usac_data->ptr_time_data[ch_idx];
  FLOAT64 *ptr_out_mdct = pstr_usac_data->spectral_line_vector[ch_idx];
  FLOAT64 *ptr_out_mdst = pstr_usac_data->mdst_spectrum[ch_idx];
  WORD32 window_shape = pstr_usac_config->window_shape_prev[ch_idx];
  FLOAT64 *ptr_win_long = NULL, *ptr_win_med = NULL;
  WORD32 win_len;
  FLOAT64 *ptr_overlap = pstr_usac_data->overlap_buf[ch_idx];

  WORD32 nflat_ls;

  memset(ptr_windowed_buf, 0, 2 * n_long * sizeof(*ptr_windowed_buf));

  switch (window_sequence) {
    case ONLY_LONG_SEQUENCE:
      err_code = iusace_calc_window(&ptr_win_long, n_long, window_shape);
      if (err_code) return err_code;
      iusace_windowing_long(ptr_overlap, ptr_win_long, ptr_windowed_buf, ptr_in_data, n_long);
      break;

    case LONG_START_SEQUENCE:
      win_len = n_short << next_mode;
      nflat_ls = (n_long - win_len) >> 1;
      err_code = iusace_calc_window(&ptr_win_long, n_long, window_shape);
      if (err_code) return err_code;
      err_code = iusace_calc_window(&ptr_win_med, win_len, 0);
      if (err_code) return err_code;

      iusace_windowing_long_start(ptr_overlap, ptr_win_long, ptr_windowed_buf, ptr_in_data,
                                  n_long, nflat_ls, ptr_win_med, win_len);
      break;

    case LONG_STOP_SEQUENCE:
      win_len = n_short << prev_mode;
      nflat_ls = (n_long - win_len) >> 1;
      err_code = iusace_calc_window(&ptr_win_long, n_long, 1);
      if (err_code) return err_code;
      err_code = iusace_calc_window(&ptr_win_med, win_len, window_shape);
      if (err_code) return err_code;
      iusace_windowing_long_stop(ptr_overlap, ptr_win_long, ptr_windowed_buf, ptr_in_data, n_long,
                                 nflat_ls, ptr_win_med, win_len);
      break;

    case STOP_START_SEQUENCE:
      win_len = n_short << (prev_mode | next_mode);
      err_code = iusace_calc_window(&ptr_win_med, win_len, window_shape);
      if (err_code) return err_code;

      iusace_windowing_stop_start(ptr_overlap, ptr_windowed_buf, ptr_win_med, win_len, n_long);
      break;
  }

  // Compute MDCT
  err_code =
      iusace_fft_based_mdct(ptr_windowed_buf, ptr_out_mdct, n_long, MDCT_TX_FLAG, pstr_scratch);
  if (err_code) {
    return err_code;
  }

  // Compute MDST
  err_code_2 =
      iusace_fft_based_mdct(ptr_windowed_buf, ptr_out_mdst, n_long, MDST_TX_FLAG, pstr_scratch);

  if (err_code_2) {
    return err_code_2;
  }

  return IA_NO_ERROR;
}

WORD32 iusace_fd_mdct(ia_usac_data_struct *pstr_usac_data,
                      ia_usac_encoder_config_struct *pstr_usac_config, WORD32 ch_idx) {
  IA_ERRORCODE err_code = 0;
  WORD32 window_sequence = pstr_usac_config->window_sequence[ch_idx];

  if (window_sequence != EIGHT_SHORT_SEQUENCE) {
    err_code = iusace_fd_mdct_long(pstr_usac_data, pstr_usac_config, ch_idx, window_sequence);
  } else {
    err_code = iusace_fd_mdct_short(pstr_usac_data, pstr_usac_config, ch_idx);
  }

  return err_code;
}
