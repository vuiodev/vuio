/******************************************************************************
 *                                                                            *
 * Copyright (C) 2018 The Android Open Source Project
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

#include "ixheaacd_bitbuffer.h"

#include "ixheaacd_interface.h"

#include "ixheaacd_tns_usac.h"
#include "ixheaacd_cnst.h"

#include "ixheaacd_acelp_info.h"

#include "ixheaacd_sbrdecsettings.h"
#include "ixheaacd_info.h"
#include "ixheaacd_sbr_common.h"
#include "ixheaacd_drc_data_struct.h"
#include "ixheaacd_drc_dec.h"
#include "ixheaacd_sbrdecoder.h"
#include "ixheaacd_mps_polyphase.h"
#include "ixheaac_sbr_const.h"

#include "ixheaacd_env_extr_part.h"
#include "ixheaacd_sbr_rom.h"
#include "ixheaacd_common_rom.h"
#include "ixheaacd_hybrid.h"
#include "ixheaacd_sbr_scale.h"
#include "ixheaacd_ps_dec.h"
#include "ixheaacd_freq_sca.h"
#include "ixheaacd_lpp_tran.h"
#include "ixheaacd_bitbuffer.h"
#include "ixheaacd_env_extr.h"
#include "ixheaacd_qmf_dec.h"
#include "ixheaacd_env_calc.h"
#include "ixheaacd_pvc_dec.h"

#include "ixheaacd_sbr_dec.h"
#include "ixheaac_error_standards.h"
#include "ixheaacd_sbrqmftrans.h"
#include "ixheaacd_qmf_poly.h"

#include "ixheaac_constants.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_op.h"

#include "ixheaac_esbr_rom.h"

static FLOAT32 *ixheaacd_map_prot_filter(WORD32 filt_length) {
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

static VOID ixheaacd_create_dft_hbe_window(FLOAT32 *win, WORD32 x_over_bin1,
                                           WORD32 x_over_bin2,
                                           WORD32 ts, WORD32 size) {
  const FLOAT32 *ptr_freq_domain_win = NULL;
  WORD32 n;
  if (ts == 12) {
    ptr_freq_domain_win = &ixheaac_dft_hbe_window_ts_12[0];
  } else {
    ptr_freq_domain_win = &ixheaac_dft_hbe_window_ts_18[0];
  }
  for (n = 0; n < (x_over_bin1 - ts / 2); n++) {
    win[n] = 0;
  }

  for (n = (x_over_bin1 - ts / 2); n <= (x_over_bin1 + ts / 2); n++) {
    win[n] = (FLOAT32)ptr_freq_domain_win[n - (x_over_bin1 - ts / 2)];
  }

  for (n = (x_over_bin1 + ts / 2 + 1); n < (x_over_bin2 - ts / 2); n++) {
    win[n] = (FLOAT32)1.0f;
  }

  for (n = (x_over_bin2 - ts / 2); n <= (x_over_bin2 + ts / 2); n++) {
    win[n] = (FLOAT32)1.0f - ptr_freq_domain_win[n - (x_over_bin2 - ts / 2)];
  }

  for (n = (x_over_bin2 + ts / 2 + 1); n < size; n++) {
    win[n] = 0.0f;
  }
}

static WORD32 ixheaacd_calc_anal_synth_window(WORD32 fft_size, FLOAT32 *ptr_window) {
  FLOAT32 sin_pi_2_N = 0.0f;
  FLOAT32 cos_pi_2_N = 0.0f;
  FLOAT32 *ptr_sin_pi_n_by_N = NULL;
  WORD32 hop_stride = 1;
  WORD32 i, j;
  WORD32 l_fft_stride = 512;
  switch (fft_size) {
    case 64:
      hop_stride = 16;
      ptr_sin_pi_n_by_N = (FLOAT32 *)&ixheaac_sine_pi_n_by_1024[0];
      sin_pi_2_N = ptr_sin_pi_n_by_N[hop_stride >> 1];
      cos_pi_2_N = ptr_sin_pi_n_by_N[512 + (hop_stride >> 1)];
      l_fft_stride = 512;
      break;
    case 128:
      hop_stride = 8;
      ptr_sin_pi_n_by_N = (FLOAT32 *)&ixheaac_sine_pi_n_by_1024[0];
      sin_pi_2_N = ptr_sin_pi_n_by_N[hop_stride >> 1];
      cos_pi_2_N = ptr_sin_pi_n_by_N[512 + (hop_stride >> 1)];
      l_fft_stride = 512;
      break;
    case 256:
      hop_stride = 4;
      ptr_sin_pi_n_by_N = (FLOAT32 *)&ixheaac_sine_pi_n_by_1024[0];
      sin_pi_2_N = ptr_sin_pi_n_by_N[hop_stride >> 1];
      cos_pi_2_N = ptr_sin_pi_n_by_N[512 + (hop_stride >> 1)];
      l_fft_stride = 512;
      break;
    case 512:
      hop_stride = 2;
      ptr_sin_pi_n_by_N = (FLOAT32 *)&ixheaac_sine_pi_n_by_1024[0];
      sin_pi_2_N = ptr_sin_pi_n_by_N[1];
      cos_pi_2_N = ptr_sin_pi_n_by_N[512 + 1];
      l_fft_stride = 512;
      break;
    case 1024:
      hop_stride = 1;
      ptr_sin_pi_n_by_N = (FLOAT32 *)&ixheaac_sine_pi_n_by_1024[0];
      sin_pi_2_N = ixheaac_sine_pi_by_2_N[0];
      cos_pi_2_N = ixheaac_sine_pi_by_2_N[1];
      l_fft_stride = 512;
      break;
    case 192:
      hop_stride = 4;
      ptr_sin_pi_n_by_N = (FLOAT32 *)&ixheaac_sine_pi_n_by_768[0];
      sin_pi_2_N = ptr_sin_pi_n_by_N[hop_stride >> 1];
      cos_pi_2_N = ptr_sin_pi_n_by_N[384 + (hop_stride >> 1)];
      l_fft_stride = 384;
      break;
    case 384:
      hop_stride = 2;
      ptr_sin_pi_n_by_N = (FLOAT32 *)&ixheaac_sine_pi_n_by_768[0];
      sin_pi_2_N = ptr_sin_pi_n_by_N[1];
      cos_pi_2_N = ptr_sin_pi_n_by_N[384 + 1];
      l_fft_stride = 384;
      break;
    case 768:
      hop_stride = 1;
      ptr_sin_pi_n_by_N = (FLOAT32 *)&ixheaac_sine_pi_n_by_768[0];
      sin_pi_2_N = ixheaac_sine_pi_by_2_N[8];
      cos_pi_2_N = ixheaac_sine_pi_by_2_N[9];
      l_fft_stride = 384;
      break;
    case 320:
      hop_stride = 3;
      ptr_sin_pi_n_by_N = (FLOAT32 *)&ixheaac_sine_pi_n_by_960[0];
      sin_pi_2_N = ixheaac_sine_pi_by_2_N[16];
      cos_pi_2_N = ixheaac_sine_pi_by_2_N[17];
      l_fft_stride = 480;
      break;
    case 960:
      hop_stride = 1;
      ptr_sin_pi_n_by_N = (FLOAT32 *)&ixheaac_sine_pi_n_by_960[0];
      sin_pi_2_N = ixheaac_sine_pi_by_2_N[2];
      cos_pi_2_N = ixheaac_sine_pi_by_2_N[3];
      l_fft_stride = 480;
      break;
    case 448:
      hop_stride = 2;
      ptr_sin_pi_n_by_N = (FLOAT32 *)&ixheaac_sine_pi_n_by_896[0];
      sin_pi_2_N = ptr_sin_pi_n_by_N[1];
      cos_pi_2_N = ptr_sin_pi_n_by_N[448 + 1];
      l_fft_stride = 448;
      break;
    case 896:
      hop_stride = 1;
      ptr_sin_pi_n_by_N = (FLOAT32 *)&ixheaac_sine_pi_n_by_896[0];
      sin_pi_2_N = ixheaac_sine_pi_by_2_N[4];
      cos_pi_2_N = ixheaac_sine_pi_by_2_N[5];
      l_fft_stride = 448;
      break;
    case 576:
      hop_stride = 1;
      ptr_sin_pi_n_by_N = (FLOAT32 *)&ixheaac_sine_pi_n_by_576[0];
      sin_pi_2_N = ixheaac_sine_pi_by_2_N[14];
      cos_pi_2_N = ixheaac_sine_pi_by_2_N[15];
      l_fft_stride = 288;
      break;
    case 640:
      hop_stride = 1;
      ptr_sin_pi_n_by_N = (FLOAT32 *)&ixheaac_sine_pi_n_by_640[0];
      sin_pi_2_N = ixheaac_sine_pi_by_2_N[12];
      cos_pi_2_N = ixheaac_sine_pi_by_2_N[13];
      l_fft_stride = 320;
      break;
    case 704:
      hop_stride = 1;
      ptr_sin_pi_n_by_N = (FLOAT32 *)&ixheaac_sine_pi_n_by_704[0];
      sin_pi_2_N = ixheaac_sine_pi_by_2_N[10];
      cos_pi_2_N = ixheaac_sine_pi_by_2_N[11];
      l_fft_stride = 352;
      break;
    case 832:
      hop_stride = 1;
      ptr_sin_pi_n_by_N = (FLOAT32 *)&ixheaac_sine_pi_n_by_832[0];
      sin_pi_2_N = ixheaac_sine_pi_by_2_N[6];
      cos_pi_2_N = ixheaac_sine_pi_by_2_N[7];
      l_fft_stride = 416;
      break;
    default:
      return -1;
  }

  for (i = 0, j = 0; j < (fft_size >> 1); i += hop_stride, j++) {
    FLOAT32 cos_val = ptr_sin_pi_n_by_N[i + l_fft_stride];
    FLOAT32 sin_val = ptr_sin_pi_n_by_N[i];
    ptr_window[j] = cos_val * sin_pi_2_N + sin_val * cos_pi_2_N;
  }

  for (; j < fft_size; j++, i += hop_stride) {
    FLOAT32 cos_val = ptr_sin_pi_n_by_N[i - l_fft_stride];
    FLOAT32 sin_val = ptr_sin_pi_n_by_N[i];
    ptr_window[j] = sin_val * cos_pi_2_N - cos_val * sin_pi_2_N;
  }
  return 0;
}

WORD32 ixheaacd_dft_hbe_data_reinit(ia_esbr_hbe_txposer_struct *ptr_hbe_txposer,
                                    WORD16 *p_freq_band_tab[2], WORD16 *p_num_sfb) {
  WORD32 sfb;
  WORD32 patch;
  WORD32 i;
  WORD32 temp_start;
  FLOAT32 fb_ratio;
  WORD32 stop_patch;
  WORD32 in_hop_size_divisor = 8;
  static const WORD32 trans_samp[2] = {12, 18};
  WORD32 err = 0;

  ptr_hbe_txposer->start_band = p_freq_band_tab[LOW][0];
  ptr_hbe_txposer->end_band = p_freq_band_tab[LOW][p_num_sfb[LOW]];
  ptr_hbe_txposer->esbr_hq = 1;

  ptr_hbe_txposer->synth_size = 4 * ((ptr_hbe_txposer->start_band + 4) / 8 + 1);
  ptr_hbe_txposer->k_start = ixheaac_start_subband2kL_tbl[ptr_hbe_txposer->start_band];

  fb_ratio = ptr_hbe_txposer->synth_size / 32.0f;

  ptr_hbe_txposer->ana_fft_size[0] = (WORD32)(fb_ratio * ptr_hbe_txposer->fft_size[0]);
  ptr_hbe_txposer->ana_fft_size[1] = (WORD32)(fb_ratio * ptr_hbe_txposer->fft_size[1]);

  ptr_hbe_txposer->in_hop_size = ptr_hbe_txposer->ana_fft_size[0] / in_hop_size_divisor;

  ptr_hbe_txposer->synth_window = (FLOAT32 *)&ptr_hbe_txposer->synthesis_window_buf[0];
  ptr_hbe_txposer->anal_window = (FLOAT32 *)&ptr_hbe_txposer->analysis_window_buf[0];

  err = ixheaacd_calc_anal_synth_window(ptr_hbe_txposer->ana_fft_size[0],
                                        ptr_hbe_txposer->anal_window);
  if (err) {
    return err;
  }

  memset(ptr_hbe_txposer->synth_buf, 0, 1280 * sizeof(ptr_hbe_txposer->synth_buf[0]));

  ptr_hbe_txposer->synth_wind_coeff = ixheaacd_map_prot_filter(ptr_hbe_txposer->synth_size);

  temp_start = 2 * ((ptr_hbe_txposer->start_band - 1) / 2);
  ptr_hbe_txposer->analy_size =
      4 * ((min(64, ptr_hbe_txposer->end_band + 1) - temp_start - 1) / 4 +
           1);
  ptr_hbe_txposer->a_start = temp_start - max(0, temp_start + ptr_hbe_txposer->analy_size - 64);

  fb_ratio = ptr_hbe_txposer->analy_size / 64.0f;

  ptr_hbe_txposer->syn_fft_size[0] = (WORD32)(fb_ratio * ptr_hbe_txposer->fft_size[0]);
  ptr_hbe_txposer->syn_fft_size[1] = (WORD32)(fb_ratio * ptr_hbe_txposer->fft_size[1]);

  ptr_hbe_txposer->out_hop_size = 2 * ptr_hbe_txposer->syn_fft_size[0] / in_hop_size_divisor;

  err = ixheaacd_calc_anal_synth_window(ptr_hbe_txposer->syn_fft_size[0],
                                        ptr_hbe_txposer->synth_window);
  if (err) {
    return err;
  }

  ptr_hbe_txposer->analy_wind_coeff = ixheaacd_map_prot_filter(ptr_hbe_txposer->analy_size);

  memset(&ptr_hbe_txposer->x_over_qmf[0], 0, sizeof(ptr_hbe_txposer->x_over_qmf));
  for (i = 0; i < MAX_STRETCH; i++) {
    memset(&ptr_hbe_txposer->x_over_bin[i][0], 0,
           2 * sizeof(ptr_hbe_txposer->x_over_bin[i][0]));
  }
  sfb = 0;
  stop_patch = MAX_STRETCH;

  switch (ptr_hbe_txposer->synth_size) {
    case 4:
      ptr_hbe_txposer->synth_cos_tab = (FLOAT32 *)ixheaac_synth_cos_table_kl_4;
      ptr_hbe_txposer->ixheaacd_real_synth_fft = &ixheaac_real_synth_fft_p2;
      break;
    case 8:
      ptr_hbe_txposer->synth_cos_tab = (FLOAT32 *)ixheaac_synth_cos_table_kl_8;
      ptr_hbe_txposer->ixheaacd_real_synth_fft = &ixheaac_real_synth_fft_p2;
      break;
    case 12:
      ptr_hbe_txposer->synth_cos_tab = (FLOAT32 *)ixheaac_synth_cos_table_kl_12;
      ptr_hbe_txposer->ixheaacd_real_synth_fft = &ixheaac_real_synth_fft_p3;
      break;
    case 16:
      ptr_hbe_txposer->synth_cos_tab = (FLOAT32 *)ixheaac_synth_cos_table_kl_16;
      ptr_hbe_txposer->ixheaacd_real_synth_fft = &ixheaac_real_synth_fft_p2;
      break;
    case 20:
      ptr_hbe_txposer->synth_cos_tab = (FLOAT32 *)ixheaac_synth_cos_table_kl_20;
      break;
    case 28:
      ptr_hbe_txposer->synth_cos_tab = (FLOAT32 *)ixheaac_synth_cos_table_kl_20;
      break;
    default:
      ptr_hbe_txposer->synth_cos_tab = (FLOAT32 *)ixheaac_synth_cos_table_kl_4;
      ptr_hbe_txposer->ixheaacd_real_synth_fft = &ixheaac_real_synth_fft_p2;
  }

  {
    WORD32 l, k, L = ptr_hbe_txposer->analy_size;
    for (k = 0; k < L; k++) {
      for (l = 0; l < 2 * L; l++) {
        ptr_hbe_txposer->str_dft_hbe_anal_coeff.real[k][l] =
            (FLOAT32)cos(PI / (2 * L) *
                         ((k + 0.5) * (2 * l - L / 64.0) - L / 64.0 *
                          ptr_hbe_txposer->a_start));
        ptr_hbe_txposer->str_dft_hbe_anal_coeff.imag[k][l] =
            (FLOAT32)sin(PI / (2 * L) *
                         ((k + 0.5) * (2 * l - L / 64.0) - L / 64.0 *
                          ptr_hbe_txposer->a_start));
      }
    }
  }

  for (patch = 1; patch <= stop_patch; patch++) {
    while (sfb <= p_num_sfb[LOW] &&
           p_freq_band_tab[LOW][sfb] <= patch * ptr_hbe_txposer->start_band)
      sfb++;
    if (sfb <= p_num_sfb[LOW]) {
      if ((patch * ptr_hbe_txposer->start_band - p_freq_band_tab[LOW][sfb - 1]) <= 3) {
        ptr_hbe_txposer->x_over_qmf[patch - 1] = p_freq_band_tab[LOW][sfb - 1];
        if (patch <= MAX_STRETCH) {
          ptr_hbe_txposer->x_over_bin[patch - 1][0] = (WORD32)(
              ptr_hbe_txposer->fft_size[0] * p_freq_band_tab[LOW][sfb - 1] / 128 + 0.5);
          ptr_hbe_txposer->x_over_bin[patch - 1][1] = (WORD32)(
              ptr_hbe_txposer->fft_size[1] * p_freq_band_tab[LOW][sfb - 1] / 128 + 0.5);
        }
      } else {
        WORD32 sfb = 0;
        while (sfb <= p_num_sfb[HIGH] &&
               p_freq_band_tab[HIGH][sfb] <= patch * ptr_hbe_txposer->start_band)
          sfb++;
        ptr_hbe_txposer->x_over_qmf[patch - 1] = p_freq_band_tab[HIGH][sfb - 1];
        if (patch <= MAX_STRETCH) {
          ptr_hbe_txposer->x_over_bin[patch - 1][0] = (WORD32)(
              ptr_hbe_txposer->fft_size[0] * p_freq_band_tab[HIGH][sfb - 1] / 128 + 0.5);
          ptr_hbe_txposer->x_over_bin[patch - 1][1] = (WORD32)(
              ptr_hbe_txposer->fft_size[1] * p_freq_band_tab[HIGH][sfb - 1] / 128 + 0.5);
        }
      }
    } else {
      ptr_hbe_txposer->x_over_qmf[patch - 1] = ptr_hbe_txposer->end_band;
      if (patch <= MAX_STRETCH) {
        ptr_hbe_txposer->x_over_bin[patch - 1][0] =
            (WORD32)(ptr_hbe_txposer->fft_size[0] * ptr_hbe_txposer->end_band / 128 + 0.5);
        ptr_hbe_txposer->x_over_bin[patch - 1][1] =
            (WORD32)(ptr_hbe_txposer->fft_size[1] * ptr_hbe_txposer->end_band / 128 + 0.5);
      }
      ptr_hbe_txposer->max_stretch = min(patch, MAX_STRETCH);
      break;
    }
  }

  for (patch = 0; patch < ptr_hbe_txposer->max_stretch - 1; patch++) {
    for (i = 0; i < 2; i++) {
      ixheaacd_create_dft_hbe_window(ptr_hbe_txposer->fd_win_buf[patch][i],
                                     ptr_hbe_txposer->x_over_bin[patch][i],
                                     ptr_hbe_txposer->x_over_bin[patch + 1][i], trans_samp[i],
                                     ptr_hbe_txposer->fft_size[i]);
    }
  }
  return 0;
}

static VOID ixheaacd_dft_hbe_apply_win(const FLOAT32 *inp1, const FLOAT32 *inp2, FLOAT32 *out,
                                       WORD32 n) {
  WORD32 i;
  for (i = 0; i < n; i++) {
    out[i] = inp1[i] * inp2[i];
  }
}

VOID ixheaacd_dft_hbe_fft_memmove(FLOAT32 *ptr_spectrum, WORD32 size) {
  WORD32 n;

  for (n = 0; n < size / 2; n++) {
    FLOAT32 tmp = ptr_spectrum[n];
    ptr_spectrum[n] = ptr_spectrum[n + size / 2];
    ptr_spectrum[n + size / 2] = tmp;
  }
}

VOID ixheaacd_karth2polar(FLOAT32 *spectrum, FLOAT32 *mag, FLOAT32 *phase, WORD32 fft_size) {
  WORD32 n;

  for (n = 1; n < fft_size / 2; n++) {
    phase[n] = (FLOAT32)atan2(spectrum[2 * n + 1], spectrum[2 * n]);
    mag[n] = (FLOAT32)sqrt(spectrum[2 * n] * spectrum[2 * n] +
                           spectrum[2 * n + 1] * spectrum[2 * n + 1]);
  }

  if (spectrum[0] < 0) {
    phase[0] = (FLOAT32)acos(-1);
    mag[0] = -spectrum[0];
  } else {
    phase[0] = 0;
    mag[0] = spectrum[0];
  }

  if (spectrum[1] < 0) {
    phase[fft_size / 2] = (FLOAT32)acos(-1);
    mag[fft_size / 2] = -spectrum[1];
  } else {
    phase[fft_size / 2] = 0;
    mag[fft_size / 2] = spectrum[1];
  }
}

VOID ixheaacd_hbe_fft_table(ia_esbr_hbe_txposer_struct *ptr_hbe_txposer) {
  WORD32 oversampling_flag = ptr_hbe_txposer->oversampling_flag;
  WORD32 ana_fft_size = ptr_hbe_txposer->ana_fft_size[oversampling_flag];
  WORD32 syn_fft_size = ptr_hbe_txposer->syn_fft_size[oversampling_flag];

  switch (ana_fft_size) {
    case 576:
      ptr_hbe_txposer->ana_cos_sin_tab = (FLOAT32 *)ixheaac_sin_cos_576;
      break;
    case 384:
      ptr_hbe_txposer->ana_cos_sin_tab = (FLOAT32 *)ixheaac_sin_cos_384;
      break;
    case 512:
      ptr_hbe_txposer->ana_cos_sin_tab = (FLOAT32 *)ixheaac_sin_cos_512;
      break;
    case 768:
      ptr_hbe_txposer->ana_cos_sin_tab = (FLOAT32 *)ixheaac_sin_cos_768;
      break;
    default:
      break;
  }

  switch (syn_fft_size) {
    case 448:
      ptr_hbe_txposer->syn_cos_sin_tab = (FLOAT32 *)ixheaac_sin_cos_448;
      break;
    case 512:
      ptr_hbe_txposer->syn_cos_sin_tab = (FLOAT32 *)ixheaac_sin_cos_512;
      break;
    case 768:
      ptr_hbe_txposer->syn_cos_sin_tab = (FLOAT32 *)ixheaac_sin_cos_768;
      break;
    case 672:
      ptr_hbe_txposer->syn_cos_sin_tab = (FLOAT32 *)ixheaac_sin_cos_672;
      break;
    default:
      break;
  }
}

IA_ERRORCODE ixheaacd_hbe_fft_map(ia_esbr_hbe_txposer_struct *ptr_hbe_txposer) {
  WORD32 oversampling_flag = ptr_hbe_txposer->oversampling_flag;
  WORD32 ana_fft_size = ptr_hbe_txposer->ana_fft_size[oversampling_flag];
  WORD32 syn_fft_size = ptr_hbe_txposer->syn_fft_size[oversampling_flag];

  switch (ana_fft_size) {
    case 576:
      ptr_hbe_txposer->ana_cos_sin_tab = (FLOAT32 *)ixheaac_sin_cos_576;
      ptr_hbe_txposer->ixheaacd_hbe_anal_fft = &ixheaacd_hbe_apply_fft_288;
      break;
    case 384:
      ptr_hbe_txposer->ana_cos_sin_tab = (FLOAT32 *)ixheaac_sin_cos_384;
      ptr_hbe_txposer->ixheaacd_hbe_anal_fft = &ixheaacd_hbe_apply_cfftn_gen;
      break;
    case 512:
      ptr_hbe_txposer->ana_cos_sin_tab = (FLOAT32 *)ixheaac_sin_cos_512;
      ptr_hbe_txposer->ixheaacd_hbe_anal_fft = &ixheaacd_hbe_apply_cfftn;
      break;
    case 768:
      ptr_hbe_txposer->ana_cos_sin_tab = (FLOAT32 *)ixheaac_sin_cos_768;
      ptr_hbe_txposer->ixheaacd_hbe_anal_fft = &ixheaacd_hbe_apply_cfftn_gen;
      break;
    default:
      return IA_FATAL_ERROR;
      break;
  }

  switch (syn_fft_size) {
    case 448:
      ptr_hbe_txposer->syn_cos_sin_tab = (FLOAT32 *)ixheaac_sin_cos_448;
      ptr_hbe_txposer->ixheaacd_hbe_synth_ifft = &ixheaacd_hbe_apply_ifft_224;
      break;
    case 512:
      ptr_hbe_txposer->syn_cos_sin_tab = (FLOAT32 *)ixheaac_sin_cos_512;
      ptr_hbe_txposer->ixheaacd_hbe_synth_ifft = &ixheaacd_hbe_apply_cfftn;
      break;
    case 768:
      ptr_hbe_txposer->syn_cos_sin_tab = (FLOAT32 *)ixheaac_sin_cos_768;
      ptr_hbe_txposer->ixheaacd_hbe_synth_ifft = &ixheaacd_hbe_apply_cfftn_gen;
      break;
    case 672:
      ptr_hbe_txposer->syn_cos_sin_tab = (FLOAT32 *)ixheaac_sin_cos_672;
      ptr_hbe_txposer->ixheaacd_hbe_synth_ifft = &ixheaacd_hbe_apply_ifft_336;
      break;
    default:
      return IA_FATAL_ERROR;
      break;
  }

  return IA_NO_ERROR;
}

VOID ixheaacd_dft_hbe_apply_polar_t2(
    WORD32 trans_fac, ia_esbr_hbe_txposer_struct *ptr_hbe_txposer,
    WORD32 pitch_in_bins, WORD out_transform_size) {
  WORD32 tr;
  WORD32 ti;
  WORD32 m_tr;
  WORD32 p, i;
  FLOAT32 mag_t;
  FLOAT32 phase_t;
  FLOAT32 m_val;
  FLOAT32(*fd_win_buf)[3][3][1536] = &ptr_hbe_txposer->fd_win_buf;
  FLOAT32 *phase = ptr_hbe_txposer->phase;
  WORD32 oversampling_flag = ptr_hbe_txposer->oversampling_flag;
  WORD32 fft_size = ptr_hbe_txposer->fft_size[oversampling_flag];
  FLOAT32 *ptr_spectrum_tx = ptr_hbe_txposer->ptr_spectrum_tx;
  FLOAT32 *mag = ptr_hbe_txposer->mag;
  FLOAT32 p_flt = fft_size * pitch_in_bins / 1536.0f;
  p = (WORD32)p_flt;
  FLOAT32 q_thr = 4.0f;
  m_tr = 0;

  for (i = 0; i <= out_transform_size; i++) {
    WORD32 utk = i;

    mag_t = (*fd_win_buf)[trans_fac - 2][oversampling_flag][i] * mag[utk];

    phase_t = trans_fac * phase[utk];

    if (phase_t == 0.0) {
      ptr_spectrum_tx[2 * i] += mag_t;
    } else {
      ptr_spectrum_tx[2 * i] += mag_t * (FLOAT32)cos(phase_t);
      ptr_spectrum_tx[2 * i + 1] += mag_t * (FLOAT32)sin(phase_t);
    }
    if (p > 0) {
      m_val = 0;
      for (tr = 1; tr < trans_fac; tr++) {
        FLOAT32 temp;
        ti = (WORD32)((2.0f * i - tr * p_flt) / trans_fac + 0.5f);
        if ((ti < 0) || (ti + p > fft_size / 2)) continue;
        temp = min(mag[ti], mag[ti + p]);
        if (temp > m_val) {
          m_val = temp;
          m_tr = tr;
          utk = ti;
        }
      }

      if (m_val > q_thr * mag[2 * i / trans_fac]) {
        mag_t = (FLOAT32)((*fd_win_buf)[trans_fac - 2][oversampling_flag][i] *
                         sqrt(mag[utk]) * sqrt(mag[utk + p]));
        phase_t = (trans_fac - m_tr) * phase[utk] + m_tr * phase[utk + p];
        ptr_spectrum_tx[2 * i] += (FLOAT32)(mag_t * cos(phase_t));
        ptr_spectrum_tx[2 * i + 1] += (FLOAT32)(mag_t * sin(phase_t));
      }
    }
  }
}

VOID ixheaacd_dft_hbe_apply_polar_t3(
    WORD32 trans_fac, ia_esbr_hbe_txposer_struct *ptr_hbe_txposer,
    WORD32 pitch_in_bins, WORD out_transform_size) {
  WORD32 tr;
  WORD32 ti;
  WORD32 m_tr = 0;
  WORD32 p, i;
  FLOAT32 mag_t = 0;
  FLOAT32 phase_t;
  FLOAT32 m_val;
  FLOAT32(*fd_win_buf)[3][3][1536] = &ptr_hbe_txposer->fd_win_buf;
  FLOAT32 *phase = ptr_hbe_txposer->phase;
  WORD32 oversampling_flag = ptr_hbe_txposer->oversampling_flag;
  WORD32 fft_size = ptr_hbe_txposer->fft_size[oversampling_flag];
  FLOAT32 *ptr_spectrum_tx = ptr_hbe_txposer->ptr_spectrum_tx;
  FLOAT32 *mag = ptr_hbe_txposer->mag;
  FLOAT32 p_flt = fft_size * pitch_in_bins / 1536.0f;
  p = (WORD32)p_flt;
  FLOAT32 q_thr = 4.0f;

  for (i = 0; i <= out_transform_size; i++) {
    WORD32 utk = 2 * i / trans_fac;
    FLOAT32 ptk = (2.0f * i / trans_fac) - utk;
    FLOAT32 k;

    if (i % 3 == 0) {
      mag_t = (*fd_win_buf)[trans_fac - 2][oversampling_flag][i] * mag[utk];
    } else if (i % 3 == 1) {
      k = (FLOAT32)cbrt(mag[utk]);
      mag_t = (*fd_win_buf)[trans_fac - 2][oversampling_flag][i] * k *
             (FLOAT32)pow(mag[utk + 1], ptk);
    } else if (i % 3 == 2) {
      k = (FLOAT32)cbrt(mag[utk + 1]);
      mag_t = (*fd_win_buf)[trans_fac - 2][oversampling_flag][i] *
             (FLOAT32)pow(mag[utk], 1.0 - ptk) * k;
    }

    phase_t = trans_fac * ((1 - ptk) * phase[utk] + ptk * phase[utk + 1]);

    ptr_spectrum_tx[2 * i] += mag_t * (FLOAT32)cos(phase_t);
    ptr_spectrum_tx[2 * i + 1] += mag_t * (FLOAT32)sin(phase_t);

    if (p > 0) {
      m_val = 0;
      for (tr = 1; tr < trans_fac; tr++) {
        FLOAT32 temp;
        ti = (WORD32)((2.0f * i - tr * p_flt) / trans_fac + 0.5f);
        if ((ti < 0) || (ti + p > fft_size / 2)) continue;
        temp = min(mag[ti], mag[ti + p]);
        if (temp > m_val) {
          m_val = temp;
          m_tr = tr;
          utk = ti;
        }
      }

      if (m_val > q_thr * mag[2 * i / trans_fac]) {
        FLOAT32 r = (FLOAT32)m_tr / trans_fac;
        if (m_tr == 1) {
          k = (FLOAT32)(cbrt((FLOAT32)mag[utk + p]));
          mag_t = (*fd_win_buf)[trans_fac - 2][oversampling_flag][i] *
                 (FLOAT32)pow(mag[utk], 1.0 - r) * k;
          phase_t = (trans_fac - m_tr) * phase[utk] + phase[utk + p];
        } else if (m_tr == 2) {
          k = (FLOAT32)(cbrt((FLOAT32)mag[utk]));
          mag_t = (*fd_win_buf)[trans_fac - 2][oversampling_flag][i] * k *
                 (FLOAT32)pow(mag[utk + p], r);
          phase_t = phase[utk] + m_tr * phase[utk + p];
        }

        ptr_spectrum_tx[2 * i] += mag_t * (FLOAT32)cos(phase_t);
        ptr_spectrum_tx[2 * i + 1] += mag_t * (FLOAT32)sin(phase_t);
      }
    }
  }
}

VOID ixheaacd_dft_hbe_apply_polar_t(
    WORD32 trans_fac, ia_esbr_hbe_txposer_struct *ptr_hbe_txposer,
    WORD32 pitch_in_bins, WORD out_transform_size) {
  WORD32 tr;
  WORD32 ti;
  WORD32 m_tr;
  WORD32 p, i;
  FLOAT32 mag_t;
  FLOAT32 phase_t;
  FLOAT32 m_val;
  FLOAT32(*fd_win_buf)[3][3][1536] = &ptr_hbe_txposer->fd_win_buf;
  FLOAT32 *phase = ptr_hbe_txposer->phase;
  WORD32 oversampling_flag = ptr_hbe_txposer->oversampling_flag;
  WORD32 fft_size = ptr_hbe_txposer->fft_size[oversampling_flag];
  FLOAT32 *ptr_spectrum_tx = ptr_hbe_txposer->ptr_spectrum_tx;
  FLOAT32 *mag = ptr_hbe_txposer->mag;
  FLOAT32 p_flt = fft_size * pitch_in_bins / 1536.0f;
  p = (WORD32)p_flt;
  FLOAT32 q_thr = 4.0f;
  m_tr = 0;

  for (i = 0; i <= out_transform_size; i++) {
    WORD32 utk = 2 * i / trans_fac;
    FLOAT32 ptk = (2.0f * i / trans_fac) - utk;

    mag_t = (*fd_win_buf)[trans_fac - 2][oversampling_flag][i] *
            (FLOAT32)pow(mag[utk], 1.0f - ptk) * (FLOAT32)pow(mag[utk + 1], ptk);

    phase_t = trans_fac * ((1 - ptk) * phase[utk] + ptk * phase[utk + 1]);

    ptr_spectrum_tx[2 * i] += mag_t * (FLOAT32)cos(phase_t);
    ptr_spectrum_tx[2 * i + 1] += mag_t * (FLOAT32)sin(phase_t);

    if (p > 0) {
      m_val = 0;
      for (tr = 1; tr < trans_fac; tr++) {
        FLOAT32 temp;
        ti = (WORD32)((2.0f * i - tr * p_flt) / trans_fac + 0.5f);
        if ((ti < 0) || (ti + p > fft_size / 2)) continue;
        temp = min(mag[ti], mag[ti + p]);
        if (temp > m_val) {
          m_val = temp;
          m_tr = tr;
          utk = ti;
        }
      }

      if (m_val > q_thr * mag[2 * i / trans_fac]) {
        FLOAT32 r = (FLOAT32)m_tr / trans_fac;
        mag_t = (*fd_win_buf)[trans_fac - 2][oversampling_flag][i] *
                (FLOAT32)pow(mag[utk], 1.0 - r) * (FLOAT32)pow(mag[utk + p], r);
        phase_t = (trans_fac - m_tr) * phase[utk] + m_tr * phase[utk + p];
        ptr_spectrum_tx[2 * i] += mag_t * (FLOAT32)cos(phase_t);
        ptr_spectrum_tx[2 * i + 1] += mag_t * (FLOAT32)sin(phase_t);
      }
    }
  }
}

WORD32 ixheaacd_dft_hbe_apply(ia_esbr_hbe_txposer_struct *ptr_hbe_txposer,
                              FLOAT32 qmf_buf_real[][64], FLOAT32 qmf_buf_imag[][64],
                              WORD32 num_columns, FLOAT32 pv_qmf_buf_real[][64],
                              FLOAT32 pv_qmf_buf_imag[][64], WORD32 pitch_in_bins,
                              FLOAT32 *dft_hbe_scratch_buf) {
  WORD32 in_offset = 0;
  WORD32 out_offset = 0;
  WORD32 in_hop_size = ptr_hbe_txposer->in_hop_size;
  WORD32 oversampling_flag = ptr_hbe_txposer->oversampling_flag;
  WORD32 fft_size = ptr_hbe_txposer->fft_size[oversampling_flag];

  WORD32 out_hop_size = ptr_hbe_txposer->out_hop_size;
  WORD32 num_in_samples = num_columns * ptr_hbe_txposer->synth_size;
  WORD32 ana_fft_size = ptr_hbe_txposer->ana_fft_size[oversampling_flag];
  WORD32 syn_fft_size = ptr_hbe_txposer->syn_fft_size[oversampling_flag];

  WORD32 ana_pad_size = (ana_fft_size - ptr_hbe_txposer->ana_fft_size[0]) / 2;
  WORD32 syn_pad_size = (syn_fft_size - ptr_hbe_txposer->syn_fft_size[0]) / 2;

  FLOAT32 *ptr_input_buf = ptr_hbe_txposer->ptr_input_buf;
  FLOAT32 *ptr_output_buf = ptr_hbe_txposer->ptr_output_buf;
  FLOAT32 *ptr_spectrum = ptr_hbe_txposer->ptr_spectrum;
  FLOAT32 *ptr_spectrum_tx = ptr_hbe_txposer->ptr_spectrum_tx;
  FLOAT32 *mag = ptr_hbe_txposer->mag;
  FLOAT32 *phase = ptr_hbe_txposer->phase;
  WORD32 i, trans_fac;

  FLOAT32 *ptr_cos_fft;
  FLOAT32 *ptr_cos_ifft;

  WORD32 ana_fft_offset = ptr_hbe_txposer->k_start * fft_size / 32;
  WORD32 syn_fft_offset = ptr_hbe_txposer->a_start * fft_size / 64;

  WORD32 err_code = IA_NO_ERROR;

  memcpy(ptr_hbe_txposer->ptr_input_buf,
         ptr_hbe_txposer->ptr_input_buf + ptr_hbe_txposer->ana_fft_size[0],
         ptr_hbe_txposer->ana_fft_size[0] * sizeof(ptr_hbe_txposer->ptr_input_buf[0]));

  ixheaacd_real_synth_filt(ptr_hbe_txposer, num_columns, qmf_buf_real, qmf_buf_imag);
  memcpy(ptr_output_buf, ptr_output_buf + 2 * ptr_hbe_txposer->syn_fft_size[0],
         2 * ptr_hbe_txposer->syn_fft_size[0] * sizeof(*ptr_output_buf));

  memset(ptr_output_buf + 2 * ptr_hbe_txposer->syn_fft_size[0], 0,
         2 * ptr_hbe_txposer->syn_fft_size[0] * sizeof(*ptr_output_buf));

  err_code = ixheaacd_hbe_fft_map(ptr_hbe_txposer);
  if (err_code) return err_code;
  while (in_offset < num_in_samples) {
    memset(ptr_spectrum, 0, fft_size * sizeof(FLOAT32));
    memset(ptr_spectrum_tx, 0, ((fft_size + 2) * sizeof(FLOAT32)));

    memset(mag, 0, (fft_size / 2 + 2) * sizeof(FLOAT32));
    memset(phase, 0, (fft_size / 2 + 2) * sizeof(FLOAT32));
    ixheaacd_dft_hbe_apply_win(ptr_input_buf + in_offset, ptr_hbe_txposer->anal_window,
                               ptr_spectrum + ana_pad_size + ana_fft_offset,
                               ptr_hbe_txposer->ana_fft_size[0]);
    ixheaacd_dft_hbe_fft_memmove(ptr_spectrum + ana_fft_offset, ana_fft_size);
    {
      WORD32 len = ana_fft_size;
      ptr_cos_fft = ptr_hbe_txposer->ana_cos_sin_tab;
      FLOAT32 *ptr_fft_data = ptr_spectrum + ana_fft_offset;
      FLOAT32 tmp1, tmp2, tmp3, tmp4;

      (*(ptr_hbe_txposer->ixheaacd_hbe_anal_fft))(ptr_fft_data, dft_hbe_scratch_buf,
                                                  len / 2, -1);

      tmp1 = ptr_fft_data[0] + ptr_fft_data[1];
      ptr_fft_data[1] = ptr_fft_data[0] - ptr_fft_data[1];
      ptr_fft_data[0] = tmp1;

      for (i = 1; i <= (len / 4 + (len % 4) / 2); ++i) {
        tmp1 = ptr_fft_data[2 * i] - ptr_fft_data[len - 2 * i];
        tmp2 = ptr_fft_data[2 * i + 1] + ptr_fft_data[len - 2 * i + 1];

        tmp3 = (*(ptr_cos_fft)) * tmp1 -
               (*(ptr_cos_fft + 1)) * tmp2;
        tmp4 = (*(ptr_cos_fft + 1)) * tmp1 +
               (*(ptr_cos_fft)) * tmp2;

        ptr_cos_fft = ptr_cos_fft + 2;

        tmp1 = ptr_fft_data[2 * i] + ptr_fft_data[len - 2 * i];
        tmp2 = ptr_fft_data[2 * i + 1] - ptr_fft_data[len - 2 * i + 1];

        ptr_fft_data[2 * i + 0] = 0.5f * (tmp1 - tmp3);
        ptr_fft_data[2 * i + 1] = 0.5f * (tmp2 - tmp4);
        ptr_fft_data[len - 2 * i + 0] = 0.5f * (tmp1 + tmp3);
        ptr_fft_data[len - 2 * i + 1] = -0.5f * (tmp2 + tmp4);
      }
    }
    ixheaacd_karth2polar(ptr_spectrum + ana_fft_offset, mag + ana_fft_offset / 2,
                         phase + ana_fft_offset / 2, ana_fft_size);

    for (trans_fac = 2; trans_fac <= ptr_hbe_txposer->max_stretch; trans_fac++) {
      WORD32 out_transform_size;

      out_transform_size = (fft_size / 2);

      switch (trans_fac) {
        case 2:
          ixheaacd_dft_hbe_apply_polar_t2(trans_fac, ptr_hbe_txposer,
                                          pitch_in_bins, out_transform_size);
          break;
        case 3:
          ixheaacd_dft_hbe_apply_polar_t3(trans_fac, ptr_hbe_txposer,
                                          pitch_in_bins, out_transform_size);
          break;
        default:
          ixheaacd_dft_hbe_apply_polar_t(trans_fac, ptr_hbe_txposer,
                                         pitch_in_bins, out_transform_size);
      }
    }

    ptr_spectrum_tx[syn_fft_offset + 1] = ptr_spectrum_tx[syn_fft_offset +
                                                          syn_fft_size];

    {
      WORD32 len = syn_fft_size;
      ptr_cos_ifft = ptr_hbe_txposer->syn_cos_sin_tab;
      FLOAT32 *ptr_fft_data = ptr_spectrum_tx + syn_fft_offset;
      FLOAT32 tmp1, tmp2, tmp3, tmp4;

      FLOAT32 scale = 1.0f / len;
      tmp1 = ptr_fft_data[0] + ptr_fft_data[1];
      ptr_fft_data[1] = scale * (ptr_fft_data[0] - ptr_fft_data[1]);
      ptr_fft_data[0] = scale * tmp1;

      for (i = 1; i <= (len / 4 + (len % 4) / 2); ++i) {
        tmp1 = ptr_fft_data[2 * i] - ptr_fft_data[len - 2 * i];
        tmp2 = ptr_fft_data[2 * i + 1] + ptr_fft_data[len - 2 * i + 1];

        tmp3 = (*(ptr_cos_ifft)) * tmp1 +
               (*(ptr_cos_ifft + 1)) * tmp2;
        tmp4 = -(*(ptr_cos_ifft + 1)) * tmp1 +
               (*(ptr_cos_ifft)) * tmp2;

        ptr_cos_ifft = ptr_cos_ifft + 2;

        tmp1 = ptr_fft_data[2 * i] + ptr_fft_data[len - 2 * i];
        tmp2 = ptr_fft_data[2 * i + 1] - ptr_fft_data[len - 2 * i + 1];

        ptr_fft_data[2 * i] = scale * (tmp1 - tmp3);
        ptr_fft_data[2 * i + 1] = scale * (tmp2 - tmp4);
        ptr_fft_data[len - 2 * i] = scale * (tmp1 + tmp3);
        ptr_fft_data[len - 2 * i + 1] = -scale * (tmp2 + tmp4);
      }

      (*(ptr_hbe_txposer->ixheaacd_hbe_synth_ifft))(ptr_fft_data, dft_hbe_scratch_buf,
                                                    len / 2, 1);
    }

    ixheaacd_dft_hbe_fft_memmove(ptr_spectrum_tx + syn_fft_offset, syn_fft_size);
    ixheaacd_dft_hbe_apply_win(
        ptr_spectrum_tx + syn_pad_size + syn_fft_offset, ptr_hbe_txposer->synth_window,
        ptr_spectrum_tx + syn_pad_size + syn_fft_offset, ptr_hbe_txposer->syn_fft_size[0]);

    for (i = 0; i < ptr_hbe_txposer->syn_fft_size[0]; i++) {
      ptr_output_buf[out_offset + i] += ptr_spectrum_tx[syn_pad_size + syn_fft_offset + i];
    }

    in_offset += in_hop_size;
    out_offset += out_hop_size;

  }

  err_code = ixheaacd_dft_hbe_cplx_anal_filt(ptr_hbe_txposer, pv_qmf_buf_real,
                                             pv_qmf_buf_imag);

  return err_code;
}
