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
#include <string.h>

#include "ixheaac_type_def.h"
#include "ixheaace_bitbuffer.h"
#include "iusace_tns_usac.h"
#include "iusace_cnst.h"
#include "ixheaace_sbr_def.h"
#include "ixheaace_resampler.h"
#include "ixheaace_sbr_hbe.h"
#include "ixheaace_sbr_hbe_fft.h"
#include "ixheaace_error_codes.h"
#include "ixheaace_common_rom.h"
#include "ixheaac_error_standards.h"
#include "ixheaac_constants.h"
#include "ixheaac_esbr_rom.h"

static FLOAT32 *ixheaace_map_prot_filter(WORD32 filt_length) {
  switch (filt_length) {
    case 4:
      return (FLOAT32 *)&ixheaac_sub_samp_qmf_window_coeff[0];
      break;
    case 8:
      return (FLOAT32 *)&ixheaac_sub_samp_qmf_window_coeff[40];
      break;
    case 12:
      return (FLOAT32 *)&ixheaac_sub_samp_qmf_window_coeff[120];
      break;
    case 16:
      return (FLOAT32 *)&ixheaac_sub_samp_qmf_window_coeff[240];
      break;
    case 20:
      return (FLOAT32 *)&ixheaac_sub_samp_qmf_window_coeff[400];
      break;
    case 24:
      return (FLOAT32 *)&ixheaac_sub_samp_qmf_window_coeff[600];
      break;
    case 28:
      return (FLOAT32 *)&ixheaac_sub_samp_qmf_window_coeff_28_36[0];
      break;
    case 32:
      return (FLOAT32 *)&ixheaac_sub_samp_qmf_window_coeff[840];
      break;
    case 36:
      return (FLOAT32 *)&ixheaac_sub_samp_qmf_window_coeff_28_36[280];
      break;
    case 40:
      return (FLOAT32 *)&ixheaac_sub_samp_qmf_window_coeff[1160];
      break;
    case 44:
      return (FLOAT32 *)&ixheaac_sub_samp_qmf_window_coeff[1560];
      break;
    default:
      return (FLOAT32 *)&ixheaac_sub_samp_qmf_window_coeff[0];
  }
}

/**
 * Calculate frequency domain window according to 23003-3:2012, 7.5.3.1 eSBR - Tool description
 */
static VOID ixheaace_create_dft_hbe_window(FLOAT32 *ptr_win, WORD32 x_over_bin1,
                                           WORD32 x_over_bin2, WORD32 ts, WORD32 size) {
  const FLOAT32 *ptr_freq_domain_win = NULL;
  WORD32 n;
  if (ts == 12) {
    ptr_freq_domain_win = &ixheaac_dft_hbe_window_ts_12[0];
  } else {
    ptr_freq_domain_win = &ixheaac_dft_hbe_window_ts_18[0];
  }
  for (n = 0; n < (x_over_bin1 - ts / 2); n++) {
    ptr_win[n] = 0;
  }

  for (n = (x_over_bin1 - ts / 2); n <= (x_over_bin1 + ts / 2); n++) {
    ptr_win[n] = (FLOAT32)ptr_freq_domain_win[n - (x_over_bin1 - ts / 2)];
  }

  for (n = (x_over_bin1 + ts / 2 + 1); n < (x_over_bin2 - ts / 2); n++) {
    ptr_win[n] = (FLOAT32)1.0f;
  }

  for (n = (x_over_bin2 - ts / 2); n <= (x_over_bin2 + ts / 2); n++) {
    ptr_win[n] = (FLOAT32)1.0f - ptr_freq_domain_win[n - (x_over_bin2 - ts / 2)];
  }

  for (n = (x_over_bin2 + ts / 2 + 1); n < size; n++) {
    ptr_win[n] = 0.0f;
  }
}

static IA_ERRORCODE ixheaace_calc_anal_synth_window(WORD32 fft_size, FLOAT32 *ptr_window) {
  FLOAT32 sin_pi_2_n = 0.0f;
  FLOAT32 cos_pi_2_n = 0.0f;
  FLOAT32 *ptr_sin_pi_n_by_n = NULL;
  WORD32 hop_stride = 1;
  WORD32 i, j;
  WORD32 l_fft_stride = 512;
  switch (fft_size) {
    case 64:
      hop_stride = 16;
      ptr_sin_pi_n_by_n = (FLOAT32 *)&ixheaac_sine_pi_n_by_1024[0];
      sin_pi_2_n = ptr_sin_pi_n_by_n[hop_stride >> 1];
      cos_pi_2_n = ptr_sin_pi_n_by_n[512 + (hop_stride >> 1)];
      l_fft_stride = 512;
      break;
    case 128:
      hop_stride = 8;
      ptr_sin_pi_n_by_n = (FLOAT32 *)&ixheaac_sine_pi_n_by_1024[0];
      sin_pi_2_n = ptr_sin_pi_n_by_n[hop_stride >> 1];
      cos_pi_2_n = ptr_sin_pi_n_by_n[512 + (hop_stride >> 1)];
      l_fft_stride = 512;
      break;
    case 256:
      hop_stride = 4;
      ptr_sin_pi_n_by_n = (FLOAT32 *)&ixheaac_sine_pi_n_by_1024[0];
      sin_pi_2_n = ptr_sin_pi_n_by_n[hop_stride >> 1];
      cos_pi_2_n = ptr_sin_pi_n_by_n[512 + (hop_stride >> 1)];
      l_fft_stride = 512;
      break;
    case 512:
      hop_stride = 2;
      ptr_sin_pi_n_by_n = (FLOAT32 *)&ixheaac_sine_pi_n_by_1024[0];
      sin_pi_2_n = ptr_sin_pi_n_by_n[1];
      cos_pi_2_n = ptr_sin_pi_n_by_n[512 + 1];
      l_fft_stride = 512;
      break;
    case 1024:
      hop_stride = 1;
      ptr_sin_pi_n_by_n = (FLOAT32 *)&ixheaac_sine_pi_n_by_1024[0];
      sin_pi_2_n = ixheaac_sine_pi_by_2_N[0];
      cos_pi_2_n = ixheaac_sine_pi_by_2_N[1];
      l_fft_stride = 512;
      break;
    case 192:
      hop_stride = 4;
      ptr_sin_pi_n_by_n = (FLOAT32 *)&ixheaac_sine_pi_n_by_768[0];
      sin_pi_2_n = ptr_sin_pi_n_by_n[hop_stride >> 1];
      cos_pi_2_n = ptr_sin_pi_n_by_n[384 + (hop_stride >> 1)];
      l_fft_stride = 384;
      break;
    case 384:
      hop_stride = 2;
      ptr_sin_pi_n_by_n = (FLOAT32 *)&ixheaac_sine_pi_n_by_768[0];
      sin_pi_2_n = ptr_sin_pi_n_by_n[1];
      cos_pi_2_n = ptr_sin_pi_n_by_n[384 + 1];
      l_fft_stride = 384;
      break;
    case 768:
      hop_stride = 1;
      ptr_sin_pi_n_by_n = (FLOAT32 *)&ixheaac_sine_pi_n_by_768[0];
      sin_pi_2_n = ixheaac_sine_pi_by_2_N[8];
      cos_pi_2_n = ixheaac_sine_pi_by_2_N[9];
      l_fft_stride = 384;
      break;
    case 320:
      hop_stride = 3;
      ptr_sin_pi_n_by_n = (FLOAT32 *)&ixheaac_sine_pi_n_by_960[0];
      sin_pi_2_n = ixheaac_sine_pi_by_2_N[16];
      cos_pi_2_n = ixheaac_sine_pi_by_2_N[17];
      l_fft_stride = 480;
      break;
    case 960:
      hop_stride = 1;
      ptr_sin_pi_n_by_n = (FLOAT32 *)&ixheaac_sine_pi_n_by_960[0];
      sin_pi_2_n = ixheaac_sine_pi_by_2_N[2];
      cos_pi_2_n = ixheaac_sine_pi_by_2_N[3];
      l_fft_stride = 480;
      break;
    case 448:
      hop_stride = 2;
      ptr_sin_pi_n_by_n = (FLOAT32 *)&ixheaac_sine_pi_n_by_896[0];
      sin_pi_2_n = ptr_sin_pi_n_by_n[1];
      cos_pi_2_n = ptr_sin_pi_n_by_n[448 + 1];
      l_fft_stride = 448;
      break;
    case 896:
      hop_stride = 1;
      ptr_sin_pi_n_by_n = (FLOAT32 *)&ixheaac_sine_pi_n_by_896[0];
      sin_pi_2_n = ixheaac_sine_pi_by_2_N[4];
      cos_pi_2_n = ixheaac_sine_pi_by_2_N[5];
      l_fft_stride = 448;
      break;
    case 576:
      hop_stride = 1;
      ptr_sin_pi_n_by_n = (FLOAT32 *)&ixheaac_sine_pi_n_by_576[0];
      sin_pi_2_n = ixheaac_sine_pi_by_2_N[14];
      cos_pi_2_n = ixheaac_sine_pi_by_2_N[15];
      l_fft_stride = 288;
      break;
    case 640:
      hop_stride = 1;
      ptr_sin_pi_n_by_n = (FLOAT32 *)&ixheaac_sine_pi_n_by_640[0];
      sin_pi_2_n = ixheaac_sine_pi_by_2_N[12];
      cos_pi_2_n = ixheaac_sine_pi_by_2_N[13];
      l_fft_stride = 320;
      break;
    case 704:
      hop_stride = 1;
      ptr_sin_pi_n_by_n = (FLOAT32 *)&ixheaac_sine_pi_n_by_704[0];
      sin_pi_2_n = ixheaac_sine_pi_by_2_N[10];
      cos_pi_2_n = ixheaac_sine_pi_by_2_N[11];
      l_fft_stride = 352;
      break;
    case 832:
      hop_stride = 1;
      ptr_sin_pi_n_by_n = (FLOAT32 *)&ixheaac_sine_pi_n_by_832[0];
      sin_pi_2_n = ixheaac_sine_pi_by_2_N[6];
      cos_pi_2_n = ixheaac_sine_pi_by_2_N[7];
      l_fft_stride = 416;
      break;
    default:
      return IA_EXHEAACE_EXE_FATAL_USAC_INVALID_MAPPING;
  }

  /*calculate Window*/
  for (i = 0, j = 0; j < (fft_size >> 1); i += hop_stride, j++) {
    FLOAT32 cos_val = ptr_sin_pi_n_by_n[i + l_fft_stride];
    FLOAT32 sin_val = ptr_sin_pi_n_by_n[i];
    ptr_window[j] = cos_val * sin_pi_2_n + sin_val * cos_pi_2_n;
  }

  for (; j < fft_size; j++, i += hop_stride) {
    FLOAT32 cos_val = ptr_sin_pi_n_by_n[i - l_fft_stride];
    FLOAT32 sin_val = ptr_sin_pi_n_by_n[i];
    ptr_window[j] = sin_val * cos_pi_2_n - cos_val * sin_pi_2_n;
  }
  return IA_NO_ERROR;
}

VOID ixheaace_esbr_hbe_data_init(ixheaace_str_esbr_hbe_txposer *pstr_esbr_hbe_txposer,
                                 const WORD32 num_aac_samples, WORD32 samp_fac_4_flag,
                                 const WORD32 num_out_samples, VOID *ptr_persistent_hbe_mem,
                                 WORD32 *ptr_total_persistant) {
  WORD32 used_persistent = 0;

  if (pstr_esbr_hbe_txposer) {
    memset(pstr_esbr_hbe_txposer, 0, sizeof(ixheaace_str_esbr_hbe_txposer));

    pstr_esbr_hbe_txposer->core_frame_length = num_aac_samples;

    pstr_esbr_hbe_txposer->no_bins = num_out_samples / IXHEAACE_NUM_QMF_SYNTH_CHANNELS;

    pstr_esbr_hbe_txposer->hbe_qmf_in_len = pstr_esbr_hbe_txposer->no_bins;

    pstr_esbr_hbe_txposer->hbe_qmf_out_len = 2 * pstr_esbr_hbe_txposer->hbe_qmf_in_len;

    pstr_esbr_hbe_txposer->ptr_input_buf = (FLOAT32 *)ptr_persistent_hbe_mem;
    used_persistent += (num_aac_samples + IXHEAACE_NUM_QMF_SYNTH_CHANNELS) *
                       sizeof(pstr_esbr_hbe_txposer->ptr_input_buf[0]);

    pstr_esbr_hbe_txposer->upsamp_4_flag = samp_fac_4_flag;

    if (pstr_esbr_hbe_txposer) {
      pstr_esbr_hbe_txposer->fft_size[0] = num_aac_samples;
      pstr_esbr_hbe_txposer->fft_size[1] = (int)(IXHEAACE_FD_OVERSAMPLING_FAC * num_aac_samples);

      /*
        Worst Case Memory requirements for DFT based HBE.
        analysis ptr_win = num_aac_samples * sizeof(float)          = 1024 * sizeof(FLOAT32)
        synthesis ptr_win = num_aac_samples * sizeof(float)         = 1024 * sizeof(FLOAT32)
        ptr_spectrum = num_aac_samples * sizeof(float) * 1.5        = 1536 * sizeof(FLOAT32)
        spectrumTransposed = num_aac_samples * sizeof(float)    = 1536 * sizeof(FLOAT32)
        ptr_mag                                                     = 1536 * sizeof(FLOAT32)
        ptr_phase                                                   = 1536 * sizeof(FLOAT32)
        pstr_esbr_hbe_txposer->inBuf                            = 2048 * sizeof(FLOAT32)
        pstr_esbr_hbe_txposer->outBuf                           = 4096 * sizeof(FLOAT32)
        fd_win_buf                                                   = 2560 * 3 * sizeof(FLOAT32)
        Total ~ (1024 * 2 + 1536 * 4 + 2048 * 1 + 4096 * 1 + 2560 * 3) * sizeof(FLOAT32)
        = 22016 * sizeof(FLOAT32)
       */

      pstr_esbr_hbe_txposer->ptr_spectrum = &pstr_esbr_hbe_txposer->spectrum_buf[0];
      pstr_esbr_hbe_txposer->ptr_spectrum_tx = &pstr_esbr_hbe_txposer->spectrum_transposed_buf[0];
      pstr_esbr_hbe_txposer->ptr_mag = &pstr_esbr_hbe_txposer->mag_buf[0];
      pstr_esbr_hbe_txposer->ptr_phase = &pstr_esbr_hbe_txposer->phase_buf[0];
      pstr_esbr_hbe_txposer->ptr_output_buf = &pstr_esbr_hbe_txposer->output_buf[0];
    }
  }
  *ptr_total_persistant = used_persistent;
}

IA_ERRORCODE ixheaace_dft_hbe_data_reinit(ixheaace_str_esbr_hbe_txposer *pstr_hbe_txposer) {
  WORD32 sfb;
  WORD32 patch;
  WORD32 i;
  WORD32 temp_start;
  FLOAT32 fb_ratio;
  WORD32 stop_patch;
  WORD32 in_hop_size_divisor = 8;
  static const WORD32 trans_samp[2] = {12, 18}; /* FD transition samples */
  WORD32 err = IA_NO_ERROR;

  pstr_hbe_txposer->start_band = pstr_hbe_txposer->ptr_freq_band_tab[IXHEAACE_LOW][0];
  pstr_hbe_txposer->end_band =
      pstr_hbe_txposer
          ->ptr_freq_band_tab[IXHEAACE_LOW][pstr_hbe_txposer->num_sf_bands[IXHEAACE_LOW]];
  pstr_hbe_txposer->esbr_hq = 1;

  pstr_hbe_txposer->synth_size = 4 * ((pstr_hbe_txposer->start_band + 4) / 8 + 1);
  pstr_hbe_txposer->k_start = ixheaac_start_subband2kL_tbl[pstr_hbe_txposer->start_band];

  fb_ratio = pstr_hbe_txposer->synth_size / 32.0f;

  pstr_hbe_txposer->ana_fft_size[0] = (WORD32)(fb_ratio * pstr_hbe_txposer->fft_size[0]);
  pstr_hbe_txposer->ana_fft_size[1] = (WORD32)(fb_ratio * pstr_hbe_txposer->fft_size[1]);

  pstr_hbe_txposer->in_hop_size = pstr_hbe_txposer->ana_fft_size[0] / in_hop_size_divisor;

  pstr_hbe_txposer->ptr_syn_win = (FLOAT32 *)&pstr_hbe_txposer->synthesis_window_buf[0];
  pstr_hbe_txposer->ptr_ana_win = (FLOAT32 *)&pstr_hbe_txposer->analysis_window_buf[0];

  err = ixheaace_calc_anal_synth_window(pstr_hbe_txposer->ana_fft_size[0],
                                        pstr_hbe_txposer->ptr_ana_win);
  if (err) {
    return err;
  }

  memset(pstr_hbe_txposer->synth_buf, 0, sizeof(pstr_hbe_txposer->synth_buf));

  pstr_hbe_txposer->ptr_syn_win_coeff = ixheaace_map_prot_filter(pstr_hbe_txposer->synth_size);

  temp_start = 2 * ((pstr_hbe_txposer->start_band - 1) / 2); /* Largest start band */
  pstr_hbe_txposer->analy_size =
      4 * ((min(64, pstr_hbe_txposer->end_band + 1) - temp_start - 1) / 4 +
           1); /* Quantize in steps of 4 */
  pstr_hbe_txposer->a_start = temp_start - max(0, temp_start + pstr_hbe_txposer->analy_size - 64);

  fb_ratio = pstr_hbe_txposer->analy_size / 64.0f;

  pstr_hbe_txposer->syn_fft_size[0] = (WORD32)(fb_ratio * pstr_hbe_txposer->fft_size[0]);
  pstr_hbe_txposer->syn_fft_size[1] = (WORD32)(fb_ratio * pstr_hbe_txposer->fft_size[1]);

  pstr_hbe_txposer->out_hop_size = 2 * pstr_hbe_txposer->syn_fft_size[0] / in_hop_size_divisor;

  err = ixheaace_calc_anal_synth_window(pstr_hbe_txposer->syn_fft_size[0],
                                        pstr_hbe_txposer->ptr_syn_win);
  if (err) {
    return err;
  }

  pstr_hbe_txposer->ptr_ana_win_coeff = ixheaace_map_prot_filter(pstr_hbe_txposer->analy_size);

  /* calculation of x_over_bin array and x_over_qmf array */

  memset(&pstr_hbe_txposer->x_over_qmf[0], 0, sizeof(pstr_hbe_txposer->x_over_qmf));

  for (i = 0; i < IXHEAACE_MAX_STRETCH; i++) {
    memset(&pstr_hbe_txposer->x_over_bin[i][0], 0,
           2 * sizeof(pstr_hbe_txposer->x_over_bin[i][0]));
  }
  sfb = 0;
  stop_patch = IXHEAACE_MAX_STRETCH;

  switch (pstr_hbe_txposer->synth_size) {
    case 4:
      pstr_hbe_txposer->ptr_syn_cos_tab = (FLOAT32 *)ixheaac_synth_cos_table_kl_4;
      pstr_hbe_txposer->ixheaace_real_synth_fft = &ixheaac_real_synth_fft_p2;
      break;
    case 8:
      pstr_hbe_txposer->ptr_syn_cos_tab = (FLOAT32 *)ixheaac_synth_cos_table_kl_8;
      pstr_hbe_txposer->ixheaace_real_synth_fft = &ixheaac_real_synth_fft_p2;
      break;
    case 12:
      pstr_hbe_txposer->ptr_syn_cos_tab = (FLOAT32 *)ixheaac_synth_cos_table_kl_12;
      pstr_hbe_txposer->ixheaace_real_synth_fft = &ixheaac_real_synth_fft_p3;
      break;
    case 16:
      pstr_hbe_txposer->ptr_syn_cos_tab = (FLOAT32 *)ixheaac_synth_cos_table_kl_16;
      pstr_hbe_txposer->ixheaace_real_synth_fft = &ixheaac_real_synth_fft_p2;
      break;
    case 20:
      pstr_hbe_txposer->ptr_syn_cos_tab = (FLOAT32 *)ixheaac_synth_cos_table_kl_20;
      break;
    case 28:
      pstr_hbe_txposer->ptr_syn_cos_tab = (FLOAT32 *)ixheaac_synth_cos_table_kl_20;
      break;
    default:
      pstr_hbe_txposer->ptr_syn_cos_tab = (FLOAT32 *)ixheaac_synth_cos_table_kl_4;
      pstr_hbe_txposer->ixheaace_real_synth_fft = &ixheaac_real_synth_fft_p2;
  }

  {
    WORD32 l, k, L = pstr_hbe_txposer->analy_size;
    for (k = 0; k < L; k++) {
      for (l = 0; l < 2 * L; l++) {
        pstr_hbe_txposer->str_dft_hbe_anal_coeff.real[k][l] =
            (FLOAT32)cos(PI / (2 * L) *
                         ((k + 0.5) * (2 * l - L / 64.0) - L / 64.0 * pstr_hbe_txposer->a_start));
        pstr_hbe_txposer->str_dft_hbe_anal_coeff.imag[k][l] =
            (FLOAT32)sin(PI / (2 * L) *
                         ((k + 0.5) * (2 * l - L / 64.0) - L / 64.0 * pstr_hbe_txposer->a_start));
      }
    }
  }

  for (patch = 1; patch <= stop_patch; patch++) {
    while (sfb <= pstr_hbe_txposer->num_sf_bands[IXHEAACE_LOW] &&
           pstr_hbe_txposer->ptr_freq_band_tab[IXHEAACE_LOW][sfb] <=
               patch * pstr_hbe_txposer->start_band)
      sfb++;
    if (sfb <= pstr_hbe_txposer->num_sf_bands[IXHEAACE_LOW]) {
      /* If the distance is larger than three QMF bands - try aligning to high resolution
       * frequency bands instead. */
      if ((patch * pstr_hbe_txposer->start_band -
           pstr_hbe_txposer->ptr_freq_band_tab[IXHEAACE_LOW][sfb - 1]) <= 3) {
        pstr_hbe_txposer->x_over_qmf[patch - 1] =
            pstr_hbe_txposer->ptr_freq_band_tab[IXHEAACE_LOW][sfb - 1];
        if (patch <= IXHEAACE_MAX_STRETCH) {
          pstr_hbe_txposer->x_over_bin[patch - 1][0] =
              (WORD32)(pstr_hbe_txposer->fft_size[0] *
                           pstr_hbe_txposer->ptr_freq_band_tab[IXHEAACE_LOW][sfb - 1] / 128 +
                       0.5);
          pstr_hbe_txposer->x_over_bin[patch - 1][1] =
              (WORD32)(pstr_hbe_txposer->fft_size[1] *
                           pstr_hbe_txposer->ptr_freq_band_tab[IXHEAACE_LOW][sfb - 1] / 128 +
                       0.5);
        }
      } else {
        WORD32 sfb_idx = 0;
        while (sfb_idx <= pstr_hbe_txposer->num_sf_bands[IXHEAACE_HIGH] &&
               pstr_hbe_txposer->ptr_freq_band_tab[IXHEAACE_HIGH][sfb_idx] <=
                   patch * pstr_hbe_txposer->start_band)
          sfb_idx++;
        pstr_hbe_txposer->x_over_qmf[patch - 1] =
            pstr_hbe_txposer->ptr_freq_band_tab[IXHEAACE_HIGH][sfb_idx - 1];
        if (patch <= IXHEAACE_MAX_STRETCH) {
          pstr_hbe_txposer->x_over_bin[patch - 1][0] =
              (WORD32)(pstr_hbe_txposer->fft_size[0] *
                           pstr_hbe_txposer->ptr_freq_band_tab[IXHEAACE_HIGH][sfb_idx - 1] / 128 +
                       0.5);
          pstr_hbe_txposer->x_over_bin[patch - 1][1] =
              (WORD32)(pstr_hbe_txposer->fft_size[1] *
                           pstr_hbe_txposer->ptr_freq_band_tab[IXHEAACE_HIGH][sfb_idx - 1] / 128 +
                       0.5);
        }
      }
    } else {
      pstr_hbe_txposer->x_over_qmf[patch - 1] = pstr_hbe_txposer->end_band;
      if (patch <= IXHEAACE_MAX_STRETCH) {
        pstr_hbe_txposer->x_over_bin[patch - 1][0] =
            (WORD32)(pstr_hbe_txposer->fft_size[0] * pstr_hbe_txposer->end_band / 128 + 0.5);
        pstr_hbe_txposer->x_over_bin[patch - 1][1] =
            (WORD32)(pstr_hbe_txposer->fft_size[1] * pstr_hbe_txposer->end_band / 128 + 0.5);
      }
      pstr_hbe_txposer->max_stretch = min(patch, IXHEAACE_MAX_STRETCH);
      break;
    }
  }

  /* calculation of frequency domain windows */
  for (patch = 0; patch < pstr_hbe_txposer->max_stretch - 1; patch++) {
    for (i = 0; i < 2; i++) {
      ixheaace_create_dft_hbe_window(pstr_hbe_txposer->fd_win_buf[patch][i],
                                     pstr_hbe_txposer->x_over_bin[patch][i],
                                     pstr_hbe_txposer->x_over_bin[patch + 1][i], trans_samp[i],
                                     pstr_hbe_txposer->fft_size[i]);
    }
  }
  return err;
}

static VOID ixheaace_dft_hbe_apply_win(const FLOAT32 *ptr_x, const FLOAT32 *ptr_y, FLOAT32 *ptr_z,
                                       WORD32 n) {
  WORD32 i;
  for (i = 0; i < n; i++) {
    ptr_z[i] = ptr_x[i] * ptr_y[i];
  }
}

VOID ixheaace_dft_hbe_fft_memmove(FLOAT32 *ptr_spectrum, WORD32 size) {
  WORD32 n = 0;

  while (n < size / 2) {
    FLOAT32 tmp = ptr_spectrum[n];
    ptr_spectrum[n] = ptr_spectrum[n + size / 2];
    ptr_spectrum[n + size / 2] = tmp;
    n++;
  }
}
VOID ixheaace_karth2polar(FLOAT32 *ptr_spectrum, FLOAT32 *ptr_mag, FLOAT32 *ptr_phase,
                          WORD32 fft_size) {
  WORD32 n;

  for (n = 1; n < fft_size / 2; n++) {
    ptr_phase[n] = (FLOAT32)atan2(ptr_spectrum[2 * n + 1], ptr_spectrum[2 * n]);
    ptr_mag[n] = (FLOAT32)sqrt(ptr_spectrum[2 * n] * ptr_spectrum[2 * n] +
                               ptr_spectrum[2 * n + 1] * ptr_spectrum[2 * n + 1]);
  }

  if (ptr_spectrum[0] < 0) {
    ptr_phase[0] = (FLOAT32)acos(-1);
    ptr_mag[0] = -ptr_spectrum[0];
  } else {
    ptr_phase[0] = 0;
    ptr_mag[0] = ptr_spectrum[0];
  }

  if (ptr_spectrum[1] < 0) {
    ptr_phase[fft_size / 2] = (FLOAT32)acos(-1);
    ptr_mag[fft_size / 2] = -ptr_spectrum[1];
  } else {
    ptr_phase[fft_size / 2] = 0;
    ptr_mag[fft_size / 2] = ptr_spectrum[1];
  }
}
VOID ixheaace_hbe_fft_tab(ixheaace_str_esbr_hbe_txposer *pstr_hbe_txposer) {
  WORD32 oversampling_flag = pstr_hbe_txposer->oversampling_flag;
  WORD32 ana_fft_size = pstr_hbe_txposer->ana_fft_size[oversampling_flag];
  WORD32 syn_fft_size = pstr_hbe_txposer->syn_fft_size[oversampling_flag];

  switch (ana_fft_size) {
    case 576:
      pstr_hbe_txposer->ptr_ana_cos_sin_tab = (FLOAT32 *)ixheaac_sin_cos_576;
      break;
    case 384:
      pstr_hbe_txposer->ptr_ana_cos_sin_tab = (FLOAT32 *)ixheaac_sin_cos_384;
      break;
    case 512:
      pstr_hbe_txposer->ptr_ana_cos_sin_tab = (FLOAT32 *)ixheaac_sin_cos_512;
      break;
    case 768:
      pstr_hbe_txposer->ptr_ana_cos_sin_tab = (FLOAT32 *)ixheaac_sin_cos_768;
      break;
    default:
      break;
  }

  switch (syn_fft_size) {
    case 448:
      pstr_hbe_txposer->ptr_syn_cos_sin_tab = (FLOAT32 *)ixheaac_sin_cos_448;
      break;
    case 512:
      pstr_hbe_txposer->ptr_syn_cos_sin_tab = (FLOAT32 *)ixheaac_sin_cos_512;
      break;
    case 768:
      pstr_hbe_txposer->ptr_syn_cos_sin_tab = (FLOAT32 *)ixheaac_sin_cos_768;
      break;
    case 672:
      pstr_hbe_txposer->ptr_syn_cos_sin_tab = (FLOAT32 *)ixheaac_sin_cos_672;
      break;
    default:
      break;
  }
}

IA_ERRORCODE ixheaace_hbe_fft_map(ixheaace_str_esbr_hbe_txposer *pstr_hbe_txposer) {
  WORD32 oversampling_flag = pstr_hbe_txposer->oversampling_flag;
  WORD32 ana_fft_size = pstr_hbe_txposer->ana_fft_size[oversampling_flag];
  WORD32 syn_fft_size = pstr_hbe_txposer->syn_fft_size[oversampling_flag];

  switch (ana_fft_size) {
    case 576:
      pstr_hbe_txposer->ptr_ana_cos_sin_tab = (FLOAT32 *)ixheaac_sin_cos_576;
      pstr_hbe_txposer->ixheaace_hbe_anal_fft = &ixheaace_hbe_apply_fft_288;
      break;
    case 384:
      pstr_hbe_txposer->ptr_ana_cos_sin_tab = (FLOAT32 *)ixheaac_sin_cos_384;
      pstr_hbe_txposer->ixheaace_hbe_anal_fft = &ixheaace_hbe_apply_cfftn_gen;
      break;
    case 512:
      pstr_hbe_txposer->ptr_ana_cos_sin_tab = (FLOAT32 *)ixheaac_sin_cos_512;
      pstr_hbe_txposer->ixheaace_hbe_anal_fft = &ixheaace_hbe_apply_cfftn;
      break;
    case 768:
      pstr_hbe_txposer->ptr_ana_cos_sin_tab = (FLOAT32 *)ixheaac_sin_cos_768;
      pstr_hbe_txposer->ixheaace_hbe_anal_fft = &ixheaace_hbe_apply_cfftn_gen;
      break;
    default:
      return IA_EXHEAACE_EXE_FATAL_USAC_INVALID_MAPPING;
      break;
  }

  switch (syn_fft_size) {
    case 448:
      pstr_hbe_txposer->ptr_syn_cos_sin_tab = (FLOAT32 *)ixheaac_sin_cos_448;
      pstr_hbe_txposer->ixheaace_hbe_synth_ifft = &ixheaace_hbe_apply_ifft_224;
      break;
    case 512:
      pstr_hbe_txposer->ptr_syn_cos_sin_tab = (FLOAT32 *)ixheaac_sin_cos_512;
      pstr_hbe_txposer->ixheaace_hbe_synth_ifft = &ixheaace_hbe_apply_cfftn;
      break;
    case 576:
      pstr_hbe_txposer->ptr_syn_cos_sin_tab = (FLOAT32 *)ixheaac_sin_cos_576;
      pstr_hbe_txposer->ixheaace_hbe_synth_ifft = &ixheaace_hbe_apply_fft_288;
      break;
    case 768:
      pstr_hbe_txposer->ptr_syn_cos_sin_tab = (FLOAT32 *)ixheaac_sin_cos_768;
      pstr_hbe_txposer->ixheaace_hbe_synth_ifft = &ixheaace_hbe_apply_cfftn_gen;
      break;
    case 672:
      pstr_hbe_txposer->ptr_syn_cos_sin_tab = (FLOAT32 *)ixheaac_sin_cos_672;
      pstr_hbe_txposer->ixheaace_hbe_synth_ifft = &ixheaace_hbe_apply_ifft_336;
      break;
    default:
      return IA_EXHEAACE_EXE_FATAL_USAC_INVALID_MAPPING;
      break;
  }

  return IA_NO_ERROR;
}

VOID ia_dft_hbe_apply_polar_t2(WORD32 T, ixheaace_str_esbr_hbe_txposer *pstr_hbe_txposer,
                               WORD32 pitch_in_bins, WORD out_transform_size) {
  WORD32 tr;
  WORD32 ti;
  WORD32 m_tr = 0;
  WORD32 p, i;
  FLOAT32 mag_t;
  FLOAT32 phase_t;
  FLOAT32 m_val;
  FLOAT32(*fd_win_buf)[3][3][1536] = &pstr_hbe_txposer->fd_win_buf;
  FLOAT32 *ptr_phase = pstr_hbe_txposer->ptr_phase;
  WORD32 oversampling_flag = pstr_hbe_txposer->oversampling_flag;
  WORD32 fft_size = pstr_hbe_txposer->fft_size[oversampling_flag];
  FLOAT32 *ptr_spectrum_tx = pstr_hbe_txposer->ptr_spectrum_tx;
  FLOAT32 *ptr_mag = pstr_hbe_txposer->ptr_mag;
  /* pitch_in_bins is given with the resolution of a 1536 point FFT */
  FLOAT32 p_flt = fft_size * pitch_in_bins / 1536.0f;
  p = (WORD32)p_flt;
  FLOAT32 q_thr = 4.0f;

  if (T < 2) {
    // To avoid invalid access by fd_win_buf
    T = 2;
  }
  i = 0;
  while (i <= out_transform_size) {
    WORD32 utk = i;

    mag_t = (*fd_win_buf)[T - 2][oversampling_flag][i] * ptr_mag[utk];

    phase_t = T * ptr_phase[utk];

    if (phase_t == 0.0) {
      ptr_spectrum_tx[2 * i] += mag_t;
    } else {
      ptr_spectrum_tx[2 * i] += mag_t * (FLOAT32)cos(phase_t);
      ptr_spectrum_tx[2 * i + 1] += mag_t * (FLOAT32)sin(phase_t);
    }
    if (p > 0) {
      m_val = 0;
      for (tr = 1; tr < T; tr++) {
        FLOAT32 temp;
        ti = (WORD32)((2.0f * i - tr * p_flt) / T + 0.5f);
        if ((ti < 0) || (ti + p > fft_size / 2)) continue;
        temp = min(ptr_mag[ti], ptr_mag[ti + p]);
        if (temp > m_val) {
          m_val = temp;
          m_tr = tr;
          utk = ti;
        }
      } /* for tr */

      if (m_val > q_thr * ptr_mag[2 * i / T]) {
        mag_t = (*fd_win_buf)[T - 2][oversampling_flag][i] * ((FLOAT32)sqrt(ptr_mag[utk])) *
                ((FLOAT32)sqrt(ptr_mag[utk + p]));
        phase_t = ((FLOAT32)(T - m_tr)) * ptr_phase[utk] + ((FLOAT32)m_tr) * ptr_phase[utk + p];
        ptr_spectrum_tx[2 * i] += mag_t * ((FLOAT32)cos(phase_t));
        ptr_spectrum_tx[2 * i + 1] += mag_t * ((FLOAT32)sin(phase_t));
      }
    }
    i++;
  }
}

VOID ia_dft_hbe_apply_polar_t_3(WORD32 T, ixheaace_str_esbr_hbe_txposer *pstr_hbe_txposer,
                                WORD32 pitch_in_bins, WORD out_transform_size) {
  WORD32 tr;
  WORD32 ti;
  WORD32 m_tr = 0;
  WORD32 p, i;
  FLOAT32 mag_t = 0.0f;
  FLOAT32 phase_t;
  FLOAT32 m_val;
  FLOAT32(*fd_win_buf)[3][3][1536] = &pstr_hbe_txposer->fd_win_buf;
  FLOAT32 *ptr_phase = pstr_hbe_txposer->ptr_phase;
  WORD32 oversampling_flag = pstr_hbe_txposer->oversampling_flag;
  WORD32 fft_size = pstr_hbe_txposer->fft_size[oversampling_flag];
  FLOAT32 *ptr_spectrum_tx = pstr_hbe_txposer->ptr_spectrum_tx;
  FLOAT32 *ptr_mag = pstr_hbe_txposer->ptr_mag;
  /* pitch_in_bins is given with the resolution of a 1536 point FFT */
  FLOAT32 p_flt = fft_size * pitch_in_bins / 1536.0f;
  p = (WORD32)p_flt;
  FLOAT32 q_thr = 4.0f;

  if (T < 3) {
    // To avoid invalid access by fd_win_buf
    T = 3;
  }

  i = 0;
  while (i <= out_transform_size) {
    WORD32 utk = 2 * i / T;
    FLOAT32 ptk = (2.0f * i / T) - utk;
    FLOAT32 k;

    if (i % 3 == 0) {
      mag_t = (*fd_win_buf)[T - 2][oversampling_flag][i] * ptr_mag[utk];
    } else if (i % 3 == 1) {
      k = (FLOAT32)cbrt(ptr_mag[utk]);
      mag_t =
          (*fd_win_buf)[T - 2][oversampling_flag][i] * k * (FLOAT32)pow(ptr_mag[utk + 1], ptk);
    } else if (i % 3 == 2) {
      k = (FLOAT32)cbrt(ptr_mag[utk + 1]);
      mag_t =
          (*fd_win_buf)[T - 2][oversampling_flag][i] * (FLOAT32)pow(ptr_mag[utk], 1.0 - ptk) * k;
    }

    phase_t = T * ((1 - ptk) * ptr_phase[utk] + ptk * ptr_phase[utk + 1]);

    ptr_spectrum_tx[2 * i] += mag_t * (FLOAT32)cos(phase_t);
    ptr_spectrum_tx[2 * i + 1] += mag_t * (FLOAT32)sin(phase_t);

    if (p > 0) {
      m_val = 0;
      for (tr = 1; tr < T; tr++) {
        FLOAT32 temp;
        ti = (WORD32)((2.0f * i - tr * p_flt) / T + 0.5f);
        if ((ti < 0) || (ti + p > fft_size / 2)) continue;
        temp = min(ptr_mag[ti], ptr_mag[ti + p]);
        if (temp > m_val) {
          m_val = temp;
          m_tr = tr;
          utk = ti;
        }
      } /* for tr */

      if (m_val > q_thr * ptr_mag[2 * i / T]) {
        FLOAT32 r = (FLOAT32)m_tr / T;
        switch (m_tr) {
          case 1:
            k = (FLOAT32)(cbrt((FLOAT32)ptr_mag[utk + p]));
            mag_t = (*fd_win_buf)[T - 2][oversampling_flag][i] *
                    (FLOAT32)pow(ptr_mag[utk], 1.0 - r) * k;
            phase_t = (T - m_tr) * ptr_phase[utk] + ptr_phase[utk + p];
            break;

          case 2:
            k = (FLOAT32)(cbrt((FLOAT32)ptr_mag[utk]));
            mag_t = (*fd_win_buf)[T - 2][oversampling_flag][i] * k *
                    (FLOAT32)pow(ptr_mag[utk + p], r);
            phase_t = ptr_phase[utk] + m_tr * ptr_phase[utk + p];
            break;
        }

        ptr_spectrum_tx[2 * i] += mag_t * (FLOAT32)cos(phase_t);
        ptr_spectrum_tx[2 * i + 1] += mag_t * (FLOAT32)sin(phase_t);
      }
    }
    i++;
  }
}

VOID ia_dft_hbe_apply_polar_t(WORD32 T, ixheaace_str_esbr_hbe_txposer *pstr_hbe_txposer,
                              WORD32 pitch_in_bins, WORD out_transform_size) {
  WORD32 tr;
  WORD32 ti;
  WORD32 m_tr = 0;
  WORD32 p, i;
  FLOAT32 mag_t;
  FLOAT32 phase_t;
  FLOAT32 m_val;
  FLOAT32(*fd_win_buf)[3][3][1536] = &pstr_hbe_txposer->fd_win_buf;
  FLOAT32 *ptr_phase = pstr_hbe_txposer->ptr_phase;
  WORD32 oversampling_flag = pstr_hbe_txposer->oversampling_flag;
  WORD32 fft_size = pstr_hbe_txposer->fft_size[oversampling_flag];
  FLOAT32 *ptr_spectrum_tx = pstr_hbe_txposer->ptr_spectrum_tx;
  FLOAT32 *ptr_mag = pstr_hbe_txposer->ptr_mag;
  /* pitch_in_bins is given with the resolution of a 1536 point FFT */
  FLOAT32 p_flt = fft_size * pitch_in_bins / 1536.0f;
  p = (WORD32)p_flt;
  FLOAT32 q_thr = 4.0f;

  if (T < 2) {
    // To avoid invalid access by fd_win_buf
    T = 2;
  }
  for (i = 0; i <= out_transform_size; i++) {
    WORD32 utk = 2 * i / T;
    FLOAT32 ptk = (2.0f * i / T) - utk;

    mag_t = (*fd_win_buf)[T - 2][oversampling_flag][i] * (FLOAT32)pow(ptr_mag[utk], 1.0f - ptk) *
            (FLOAT32)pow(ptr_mag[utk + 1], ptk);

    phase_t = T * ((1 - ptk) * ptr_phase[utk] + ptk * ptr_phase[utk + 1]);

    ptr_spectrum_tx[2 * i] += mag_t * (FLOAT32)cos(phase_t);
    ptr_spectrum_tx[2 * i + 1] += mag_t * (FLOAT32)sin(phase_t);

    if (p > 0) {
      m_val = 0;
      for (tr = 1; tr < T; tr++) {
        FLOAT32 temp;
        ti = (WORD32)((2.0f * i - tr * p_flt) / T + 0.5f);
        if ((ti < 0) || (ti + p > fft_size / 2)) continue;
        temp = min(ptr_mag[ti], ptr_mag[ti + p]);
        if (temp > m_val) {
          m_val = temp;
          m_tr = tr;
          utk = ti;
        }
      } /* for tr */

      if (m_val > q_thr * ptr_mag[2 * i / T]) {
        FLOAT32 r = (FLOAT32)m_tr / T;
        mag_t = (*fd_win_buf)[T - 2][oversampling_flag][i] * (FLOAT32)pow(ptr_mag[utk], 1.0 - r) *
                (FLOAT32)pow(ptr_mag[utk + p], r);
        phase_t = (T - m_tr) * ptr_phase[utk] + m_tr * ptr_phase[utk + p];
        ptr_spectrum_tx[2 * i] += mag_t * (FLOAT32)cos(phase_t);
        ptr_spectrum_tx[2 * i + 1] += mag_t * (FLOAT32)sin(phase_t);
      }
    }
  }
}

IA_ERRORCODE ixheaace_dft_hbe_apply(ixheaace_str_esbr_hbe_txposer *pstr_hbe_txposer,
                                    FLOAT32 qmf_buf_real[][64], FLOAT32 qmf_buf_imag[][64],
                                    WORD32 num_columns, FLOAT32 pv_qmf_buf_real[][64],
                                    FLOAT32 pv_qmf_buf_imag[][64], WORD32 pitch_in_bins,
                                    FLOAT32 *ptr_dft_hbe_scratch_buf) {
  WORD32 in_offset = 0;
  WORD32 out_offset = 0;
  WORD32 in_hop_size = pstr_hbe_txposer->in_hop_size;
  WORD32 oversampling_flag = pstr_hbe_txposer->oversampling_flag;
  WORD32 fft_size = pstr_hbe_txposer->fft_size[oversampling_flag];

  WORD32 out_hop_size = pstr_hbe_txposer->out_hop_size;
  WORD32 num_in_samples = num_columns * pstr_hbe_txposer->synth_size;
  WORD32 ana_fft_size = pstr_hbe_txposer->ana_fft_size[oversampling_flag];
  WORD32 syn_fft_size = pstr_hbe_txposer->syn_fft_size[oversampling_flag];

  WORD32 ana_pad_size = (ana_fft_size - pstr_hbe_txposer->ana_fft_size[0]) / 2;
  WORD32 syn_pad_size = (syn_fft_size - pstr_hbe_txposer->syn_fft_size[0]) / 2;

  FLOAT32 *ptr_input_buf = pstr_hbe_txposer->ptr_input_buf;
  FLOAT32 *ptr_output_buf = pstr_hbe_txposer->ptr_output_buf;
  FLOAT32 *ptr_spectrum = pstr_hbe_txposer->ptr_spectrum;
  FLOAT32 *ptr_spectrum_tx = pstr_hbe_txposer->ptr_spectrum_tx;
  FLOAT32 *ptr_mag = pstr_hbe_txposer->ptr_mag;
  FLOAT32 *ptr_phase = pstr_hbe_txposer->ptr_phase;
  WORD32 i, T;
  FLOAT32 *ptr_cos_fft;
  FLOAT32 *ptr_cos_ifft;

  WORD32 ana_fft_offset = pstr_hbe_txposer->k_start * fft_size / 32;
  WORD32 syn_fft_offset = pstr_hbe_txposer->a_start * fft_size / 64;
  /* pitch_in_bins is given with the resolution of a 1536 point FFT */
  WORD32 err_code = IA_NO_ERROR;

  memcpy(pstr_hbe_txposer->ptr_input_buf,
         pstr_hbe_txposer->ptr_input_buf + pstr_hbe_txposer->ana_fft_size[0],
         pstr_hbe_txposer->ana_fft_size[0] * sizeof(pstr_hbe_txposer->ptr_input_buf[0]));

  err_code = ixheaace_real_synth_filt(pstr_hbe_txposer, num_columns, qmf_buf_real, qmf_buf_imag);
  if (err_code) {
    return err_code;
  }

  memcpy(ptr_output_buf, ptr_output_buf + 2 * pstr_hbe_txposer->syn_fft_size[0],
         2 * pstr_hbe_txposer->syn_fft_size[0] * sizeof(ptr_output_buf[0]));

  memset(ptr_output_buf + 2 * pstr_hbe_txposer->syn_fft_size[0], 0,
         2 * pstr_hbe_txposer->syn_fft_size[0] * sizeof(ptr_output_buf[0]));

  err_code = ixheaace_hbe_fft_map(pstr_hbe_txposer);
  if (err_code) {
    return err_code;
  }

  while (in_offset < num_in_samples) {
    memset(ptr_spectrum, 0, fft_size * sizeof(ptr_spectrum[0]));
    memset(ptr_spectrum_tx, 0, ((fft_size + 2) * sizeof(ptr_spectrum_tx[0])));

    memset(ptr_mag, 0, (fft_size / 2 + 2) * sizeof(ptr_mag[0]));
    memset(ptr_phase, 0, (fft_size / 2 + 2) * sizeof(ptr_phase[0]));
    ixheaace_dft_hbe_apply_win(ptr_input_buf + in_offset, pstr_hbe_txposer->ptr_ana_win,
                               ptr_spectrum + ana_pad_size + ana_fft_offset,
                               pstr_hbe_txposer->ana_fft_size[0]);
    ixheaace_dft_hbe_fft_memmove(ptr_spectrum + ana_fft_offset, ana_fft_size);
    {
      WORD32 len = ana_fft_size;
      ptr_cos_fft = pstr_hbe_txposer->ptr_ana_cos_sin_tab;
      FLOAT32 *ptr_fft_data = ptr_spectrum + ana_fft_offset;
      FLOAT32 tmp1, tmp2, tmp3, tmp4;
      (*(pstr_hbe_txposer->ixheaace_hbe_anal_fft))(ptr_fft_data, ptr_dft_hbe_scratch_buf, len / 2,
                                                   -1);
      tmp1 = ptr_fft_data[0] + ptr_fft_data[1];
      ptr_fft_data[1] = ptr_fft_data[0] - ptr_fft_data[1];
      ptr_fft_data[0] = tmp1;

      i = 1;
      while (i <= (len / 4 + (len % 4) / 2)) {
        tmp1 = ptr_fft_data[2 * i] - ptr_fft_data[len - 2 * i];
        tmp2 = ptr_fft_data[2 * i + 1] + ptr_fft_data[len - 2 * i + 1];

        tmp3 = (*(ptr_cos_fft)) * tmp1 - (*(ptr_cos_fft + 1)) * tmp2;
        tmp4 = (*(ptr_cos_fft + 1)) * tmp1 + (*(ptr_cos_fft)) * tmp2;

        ptr_cos_fft = ptr_cos_fft + 2;

        tmp1 = ptr_fft_data[2 * i] + ptr_fft_data[len - 2 * i];
        tmp2 = ptr_fft_data[2 * i + 1] - ptr_fft_data[len - 2 * i + 1];

        ptr_fft_data[2 * i + 0] = 0.5f * (tmp1 - tmp3);
        ptr_fft_data[2 * i + 1] = 0.5f * (tmp2 - tmp4);
        ptr_fft_data[len - 2 * i + 0] = 0.5f * (tmp1 + tmp3);
        ptr_fft_data[len - 2 * i + 1] = -0.5f * (tmp2 + tmp4);
        ++i;
      }
    }
    ixheaace_karth2polar(ptr_spectrum + ana_fft_offset, ptr_mag + ana_fft_offset / 2,
                         ptr_phase + ana_fft_offset / 2, ana_fft_size);

    for (T = 2; T <= pstr_hbe_txposer->max_stretch; T++) {
      /* max_stretch cannot be greater than 4. So, T can be 2 to 4*/

      WORD32 out_transform_size;

      /* 0<i<fft_size/2 */
      out_transform_size = (fft_size / 2);
      switch (T) {
        case 2:
          ia_dft_hbe_apply_polar_t2(T, pstr_hbe_txposer, pitch_in_bins, out_transform_size);
          break;
        case 3:
          ia_dft_hbe_apply_polar_t_3(T, pstr_hbe_txposer, pitch_in_bins, out_transform_size);
          break;
        default:
          ia_dft_hbe_apply_polar_t(T, pstr_hbe_txposer, pitch_in_bins, out_transform_size);
      }
    } /* for T */

    ptr_spectrum_tx[syn_fft_offset + 1] =
        ptr_spectrum_tx[syn_fft_offset + syn_fft_size]; /* Move Nyquist bin to bin 1 for RFFTN */

    {
      WORD32 len = syn_fft_size;
      ptr_cos_ifft = pstr_hbe_txposer->ptr_syn_cos_sin_tab;
      FLOAT32 *ptr_fft_data = ptr_spectrum_tx + syn_fft_offset;
      FLOAT32 tmp1, tmp2, tmp3, tmp4;

      FLOAT32 scale = 1.0f / len;
      tmp1 = ptr_fft_data[0] + ptr_fft_data[1];
      ptr_fft_data[1] = scale * (ptr_fft_data[0] - ptr_fft_data[1]);
      ptr_fft_data[0] = scale * tmp1;

      for (i = 1; i <= (len / 4 + (len % 4) / 2); ++i) {
        tmp1 = ptr_fft_data[2 * i] - ptr_fft_data[len - 2 * i];
        tmp2 = ptr_fft_data[2 * i + 1] + ptr_fft_data[len - 2 * i + 1];

        tmp3 = (*(ptr_cos_ifft)) * tmp1 + (*(ptr_cos_ifft + 1)) * tmp2;
        tmp4 = -(*(ptr_cos_ifft + 1)) * tmp1 + (*(ptr_cos_ifft)) * tmp2;

        ptr_cos_ifft = ptr_cos_ifft + 2;

        tmp1 = ptr_fft_data[2 * i] + ptr_fft_data[len - 2 * i];
        tmp2 = ptr_fft_data[2 * i + 1] - ptr_fft_data[len - 2 * i + 1];

        ptr_fft_data[2 * i] = scale * (tmp1 - tmp3);
        ptr_fft_data[2 * i + 1] = scale * (tmp2 - tmp4);
        ptr_fft_data[len - 2 * i] = scale * (tmp1 + tmp3);
        ptr_fft_data[len - 2 * i + 1] = -scale * (tmp2 + tmp4);
      }

      (*(pstr_hbe_txposer->ixheaace_hbe_synth_ifft))(ptr_fft_data, ptr_dft_hbe_scratch_buf,
                                                     len / 2, 1);
    }

    ixheaace_dft_hbe_fft_memmove(ptr_spectrum_tx + syn_fft_offset, syn_fft_size);
    ixheaace_dft_hbe_apply_win(
        ptr_spectrum_tx + syn_pad_size + syn_fft_offset, pstr_hbe_txposer->ptr_syn_win,
        ptr_spectrum_tx + syn_pad_size + syn_fft_offset, pstr_hbe_txposer->syn_fft_size[0]);

    for (i = 0; i < pstr_hbe_txposer->syn_fft_size[0]; i++) {
      ptr_output_buf[out_offset + i] += ptr_spectrum_tx[syn_pad_size + syn_fft_offset + i];
    }

    in_offset += in_hop_size;
    out_offset += out_hop_size;

  } /* while(in_offset<num_in_samples) */

  ixheaace_dft_hbe_cplx_anal_filt(pstr_hbe_txposer, pv_qmf_buf_real, pv_qmf_buf_imag);

  return err_code;
}
