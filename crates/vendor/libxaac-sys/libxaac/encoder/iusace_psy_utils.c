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

#include "ixheaac_type_def.h"
#include "ixheaace_adjust_threshold_data.h"
#include "iusace_block_switch_const.h"
#include "iusace_cnst.h"
#include "iusace_bitbuffer.h"
#include "ixheaace_mps_common_define.h"

/* DRC */
#include "impd_drc_common_enc.h"
#include "impd_drc_uni_drc.h"
#include "impd_drc_tables.h"
#include "impd_drc_api.h"
#include "impd_drc_uni_drc_eq.h"
#include "impd_drc_uni_drc_filter_bank.h"
#include "impd_drc_gain_enc.h"
#include "impd_drc_struct_def.h"

#include "iusace_tns_usac.h"
#include "iusace_psy_mod.h"
#include "iusace_tns_usac.h"
#include "iusace_config.h"
#include "iusace_psy_utils.h"
#include "iusace_fd_qc_util.h"
#include "iusace_fd_qc_adjthr.h"
#include "ixheaac_error_standards.h"
#include "ixheaace_error_codes.h"

extern ia_sfb_info_struct iusace_sfb_info_1024[12];
extern ia_sfb_info_struct iusace_sfb_info_768[12];

static const FLOAT32 iusace_bark_quiet_thr_val[] = {
    15.0f, 10.0f, 7.0f, 2.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f,  0.0f,  0.0f, 0.0f,
    0.0f,  0.0f,  0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 3.0f, 5.0f, 10.0f, 20.0f, 30.0f};

VOID iusace_calc_band_energy(const FLOAT64 *ptr_spec_coeffs, const WORD32 *band_offset,
                             const WORD32 num_bands, FLOAT32 *ptr_band_energy, WORD32 sfb_count) {
  WORD32 i, j;

  j = 0;
  memset(ptr_band_energy, 0, sfb_count * sizeof(FLOAT32));
  for (i = 0; i < num_bands; i++) {
    while (j < band_offset[i + 1]) {
      ptr_band_energy[i] += (FLOAT32)(ptr_spec_coeffs[j] * ptr_spec_coeffs[j]);
      j++;
    }
  }
  return;
}

VOID iusace_find_max_spreading(const WORD32 sfb_count, const FLOAT32 *ptr_mask_low_fac,
                               const FLOAT32 *ptr_mask_high_fac, FLOAT32 *ptr_spreaded_enegry) {
  WORD32 i;

  for (i = 1; i < sfb_count; i++) {
    ptr_spreaded_enegry[i] =
        MAX(ptr_spreaded_enegry[i], ptr_mask_high_fac[i] * ptr_spreaded_enegry[i - 1]);
  }

  for (i = sfb_count - 2; i >= 0; i--) {
    ptr_spreaded_enegry[i] =
        MAX(ptr_spreaded_enegry[i], ptr_mask_low_fac[i] * ptr_spreaded_enegry[i + 1]);
  }
  return;
}

VOID iusace_pre_echo_control(FLOAT32 *ptr_thr_nm1, WORD32 sfb_count, FLOAT32 max_allowed_inc_fac,
                             FLOAT32 min_remaining_thr_fac, FLOAT32 *ptr_threshold) {
  WORD32 i;
  FLOAT32 thr1, thr2;

  for (i = 0; i < sfb_count; i++) {
    thr1 = max_allowed_inc_fac * (ptr_thr_nm1[i]);
    thr2 = min_remaining_thr_fac * ptr_threshold[i];

    ptr_thr_nm1[i] = ptr_threshold[i];

    if (ptr_threshold[i] > thr1) {
      ptr_threshold[i] = thr1;
    }
    if (thr2 > ptr_threshold[i]) {
      ptr_threshold[i] = thr2;
    }
  }
  return;
}

static VOID iusace_sfb_init(WORD32 sample_rate, WORD32 block_type, WORD32 *ptr_sfb_offset,
                            WORD32 *ptr_sfb_count, WORD32 ccfl) {
  const WORD16 *ptr_sfb_params = 0;
  WORD32 start_offset, block_len = 0;
  const ia_sfb_info_struct *pstr_sfb_info_tbls = &iusace_sfb_info_1024[0];
  WORD32 sampling_rate_mapped = iusace_map_sample_rate(sample_rate);
  WORD16 prev_val = 0;
  if (ccfl == LEN_SUPERFRAME_768) {
    pstr_sfb_info_tbls = &iusace_sfb_info_768[0];
  }


  if (block_type == ONLY_LONG_SEQUENCE) {
    block_len = ccfl;
    switch (sampling_rate_mapped) {
      case 96000:
        ptr_sfb_params = pstr_sfb_info_tbls[11].cb_offset_long;
        break;
      case 88200:
        ptr_sfb_params = pstr_sfb_info_tbls[10].cb_offset_long;
        break;
      case 64000:
        ptr_sfb_params = pstr_sfb_info_tbls[9].cb_offset_long;
        break;
      case 48000:
        ptr_sfb_params = pstr_sfb_info_tbls[8].cb_offset_long;
        break;
      case 44100:
        ptr_sfb_params = pstr_sfb_info_tbls[7].cb_offset_long;
        break;
      case 32000:
      case 29400:
        ptr_sfb_params = pstr_sfb_info_tbls[6].cb_offset_long;
        break;
      case 24000:
        ptr_sfb_params = pstr_sfb_info_tbls[5].cb_offset_long;
        break;
      case 22050:
        ptr_sfb_params = pstr_sfb_info_tbls[4].cb_offset_long;
        break;
      case 16000:
      case 14700:
        ptr_sfb_params = pstr_sfb_info_tbls[3].cb_offset_long;
        break;
      case 12000:
        ptr_sfb_params = pstr_sfb_info_tbls[2].cb_offset_long;
        break;
      case 11025:
        ptr_sfb_params = pstr_sfb_info_tbls[1].cb_offset_long;
        break;
      case 8000:
        ptr_sfb_params = pstr_sfb_info_tbls[0].cb_offset_long;
        break;
      default:
        ptr_sfb_params = pstr_sfb_info_tbls[8].cb_offset_long;
        break;
    }
  } else {
    block_len = ccfl >> 3;
    switch (sampling_rate_mapped) {
      case 96000:
        ptr_sfb_params = pstr_sfb_info_tbls[11].cb_offset_short;
        break;
      case 88200:
        ptr_sfb_params = pstr_sfb_info_tbls[10].cb_offset_short;
        break;
      case 64000:
        ptr_sfb_params = pstr_sfb_info_tbls[9].cb_offset_short;
        break;
      case 48000:
        ptr_sfb_params = pstr_sfb_info_tbls[8].cb_offset_short;
        break;
      case 44100:
        ptr_sfb_params = pstr_sfb_info_tbls[7].cb_offset_short;
        break;
      case 32000:
      case 29400:
        ptr_sfb_params = pstr_sfb_info_tbls[6].cb_offset_short;
        break;
      case 24000:
        ptr_sfb_params = pstr_sfb_info_tbls[5].cb_offset_short;
        break;
      case 22050:
        ptr_sfb_params = pstr_sfb_info_tbls[4].cb_offset_short;
        break;
      case 16000:
      case 14700:
        ptr_sfb_params = pstr_sfb_info_tbls[3].cb_offset_short;
        break;
      case 12000:
        ptr_sfb_params = pstr_sfb_info_tbls[2].cb_offset_short;
        break;
      case 11025:
        ptr_sfb_params = pstr_sfb_info_tbls[1].cb_offset_short;
        break;
      case 8000:
        ptr_sfb_params = pstr_sfb_info_tbls[0].cb_offset_short;
        break;
      default:
        ptr_sfb_params = pstr_sfb_info_tbls[8].cb_offset_short;
        break;
    }
  }

  *ptr_sfb_count = 0;
  start_offset = 0;

  do {
    ptr_sfb_offset[*ptr_sfb_count] = start_offset;
    if (*ptr_sfb_count == 0)
      prev_val = 0;
    else
      prev_val = ptr_sfb_params[*ptr_sfb_count - 1];
    start_offset += ptr_sfb_params[*ptr_sfb_count] - prev_val;
    (*ptr_sfb_count)++;
  } while (start_offset < block_len);

  ptr_sfb_offset[*ptr_sfb_count] = start_offset;

  return;
}

static FLOAT32 iusace_atan_approx(FLOAT32 val) {
  if (val < (FLOAT32)1.0) {
    return (val / ((FLOAT32)1.0f + (FLOAT32)0.280872f * val * val));
  } else {
    return ((FLOAT32)1.57079633f - val / ((FLOAT32)0.280872f + val * val));
  }
}

static FLOAT32 iusace_calc_bark_line_value(WORD32 num_lines, WORD32 fft_line,
                                           WORD32 sample_rate) {
  FLOAT32 center_freq, temp, b_value;

  center_freq = (FLOAT32)fft_line * ((FLOAT32)sample_rate * (FLOAT32)0.5f) / (FLOAT32)num_lines;
  temp = (FLOAT32)iusace_atan_approx((FLOAT32)1.3333333e-4f * center_freq);
  b_value = (FLOAT32)13.3f * iusace_atan_approx((FLOAT32)0.00076f * center_freq) +
            (FLOAT32)3.5f * temp * temp;

  return (b_value);
}

static VOID iusace_bark_values_init(WORD32 sfb_count, WORD32 *ptr_sfb_offset, WORD32 num_lines,
                                    WORD32 sample_rate, FLOAT32 *ptr_b_value) {
  WORD32 i;
  FLOAT32 b_val0, b_val1;
  b_val0 = 0.0f;

  for (i = 0; i < sfb_count; i++) {
    b_val1 = iusace_calc_bark_line_value(num_lines, ptr_sfb_offset[i + 1], sample_rate);
    ptr_b_value[i] = (b_val0 + b_val1) * (FLOAT32)0.5f;
    b_val0 = b_val1;
  }
  return;
}

static VOID iusace_thr_quiet_init(WORD32 sfb_count, WORD32 *ptr_sfb_offset, FLOAT32 *ptr_bark_val,
                                  FLOAT32 *ptr_thr_quiet) {
  WORD32 i;
  FLOAT32 bark_thr_quiet;

  for (i = 0; i < sfb_count; i++) {
    WORD32 b_val1, b_val2;

    if (i > 0) {
      b_val1 = (WORD32)(ptr_bark_val[i] + ptr_bark_val[i - 1]) >> 1;
    } else {
      b_val1 = (WORD32)(ptr_bark_val[i]) >> 1;
    }

    if (i < sfb_count - 1) {
      b_val2 = (WORD32)(ptr_bark_val[i] + ptr_bark_val[i + 1]) >> 1;
    } else {
      b_val2 = (WORD32)(ptr_bark_val[i]);
    }
    b_val1 = MIN(b_val1, (WORD32)MAX_BARK_VALUE);
    b_val2 = MIN(b_val2, (WORD32)MAX_BARK_VALUE);
    bark_thr_quiet = MIN(iusace_bark_quiet_thr_val[b_val1], iusace_bark_quiet_thr_val[b_val2]);

    ptr_thr_quiet[i] = (FLOAT32)pow(10.0f, (bark_thr_quiet - 20.0f) * (FLOAT32)0.1f) * 16887.8f *
                       (FLOAT32)(ptr_sfb_offset[i + 1] - ptr_sfb_offset[i]);
  }
  return;
}

static VOID iusace_spreading_init(WORD32 sfb_count, FLOAT32 *ptr_bark_val,
                                  FLOAT32 *ptr_mask_low_fac, FLOAT32 *ptr_mask_high_fac,
                                  FLOAT32 *ptr_mask_low_fac_spr_energy,
                                  FLOAT32 *ptr_mask_high_fac_spr_energy, const WORD32 bit_rate,
                                  WORD32 block_type) {
  WORD32 i;
  FLOAT32 mask_low_spr_energy, mask_high_spr_energy;

  if (block_type != EIGHT_SHORT_SEQUENCE) {
    mask_low_spr_energy = MASK_LOW_SP_ENERGY_L;
    mask_high_spr_energy = (bit_rate > 22000) ? MASK_HIGH_SP_ENERGY_L : MASK_HIGH_SP_ENERGY_L_LBR;
  } else {
    mask_low_spr_energy = MASK_LOW_SP_ENERGY_S;
    mask_high_spr_energy = MASK_HIGH_SP_ENERGY_S;
  }

  for (i = 0; i < sfb_count; i++) {
    if (i > 0) {
      FLOAT32 db_val;
      FLOAT32 diff_val = (ptr_bark_val[i] - ptr_bark_val[i - 1]);

      db_val = MASK_HIGH_FAC * diff_val;
      ptr_mask_high_fac[i] = (FLOAT32)pow(10.0f, -db_val);
      db_val = MASK_LOW_FAC * diff_val;
      ptr_mask_low_fac[i - 1] = (FLOAT32)pow(10.0f, -db_val);
      db_val = mask_high_spr_energy * diff_val;
      ptr_mask_high_fac_spr_energy[i] = (FLOAT32)pow(10.0f, -db_val);
      db_val = mask_low_spr_energy * diff_val;
      ptr_mask_low_fac_spr_energy[i - 1] = (FLOAT32)pow(10.0f, -db_val);
    } else {
      ptr_mask_high_fac[i] = 0.0f;
      ptr_mask_low_fac[sfb_count - 1] = 0.0f;
      ptr_mask_high_fac_spr_energy[i] = 0.0f;
      ptr_mask_low_fac_spr_energy[sfb_count - 1] = 0.0f;
    }
  }
  return;
}

static VOID iusace_min_snr_init(const WORD32 bit_rate, const WORD32 sample_rate,
                                const WORD32 num_lines, const WORD32 *ptr_sfb_offset,
                                const FLOAT32 *ptr_bark_value, const WORD32 sfb_active,
                                FLOAT32 *ptr_sfb_min_snr) {
  WORD32 sfb;
  FLOAT32 bark_fac;
  FLOAT32 bark_width;
  FLOAT32 pe_per_window, pe_part;
  FLOAT32 snr;
  FLOAT32 b_val0, b_val1;

  if (sfb_active == 0) {
    bark_fac = 1.0f;
  } else {
    bark_fac = (FLOAT32)1.0 / MIN(ptr_bark_value[sfb_active - 1] / MAX_BARK_VALUE, (FLOAT32)1.0);
  }

  pe_per_window =
      iusace_bits_to_pe((FLOAT32)bit_rate / (FLOAT32)sample_rate * (FLOAT32)num_lines);

  b_val0 = (FLOAT32)0.0f;

  for (sfb = 0; sfb < sfb_active; sfb++) {
    b_val1 = (FLOAT32)2.0 * ptr_bark_value[sfb] - b_val0;
    bark_width = b_val1 - b_val0;
    b_val0 = b_val1;

    pe_part = pe_per_window * (FLOAT32)0.024f * bark_fac;
    pe_part *= bark_width;
    pe_part /= (FLOAT32)(ptr_sfb_offset[sfb + 1] - ptr_sfb_offset[sfb]);
    snr = (FLOAT32)pow(2.0f, pe_part) - 1.5f;
    snr = 1.0f / MAX(snr, 1.0f);
    snr = MIN(snr, 0.8f);
    snr = MAX(snr, 0.003f);
    ptr_sfb_min_snr[sfb] = snr;
  }
  return;
}

VOID iusace_psy_long_config_init(WORD32 bit_rate, WORD32 sample_rate, WORD32 band_width,
                                 ia_psy_mod_long_config_struct *pstr_psy_config, WORD32 ccfl) {
  WORD32 sfb;
  FLOAT32 sfb_bark_val[MAX_NUM_GROUPED_SFB];

  iusace_sfb_init(sample_rate, ONLY_LONG_SEQUENCE, pstr_psy_config->sfb_offset,
                  &(pstr_psy_config->sfb_count), ccfl);

  iusace_bark_values_init(pstr_psy_config->sfb_count, pstr_psy_config->sfb_offset,
                          pstr_psy_config->sfb_offset[pstr_psy_config->sfb_count], sample_rate,
                          sfb_bark_val);

  iusace_thr_quiet_init(pstr_psy_config->sfb_count, pstr_psy_config->sfb_offset, sfb_bark_val,
                        pstr_psy_config->sfb_thr_quiet);

  iusace_spreading_init(
      pstr_psy_config->sfb_count, sfb_bark_val, pstr_psy_config->sfb_mask_low_fac,
      pstr_psy_config->sfb_mask_high_fac, pstr_psy_config->sfb_mask_low_fac_spr_ener,
      pstr_psy_config->sfb_mask_high_fac_spr_ener, bit_rate, ONLY_LONG_SEQUENCE);

  pstr_psy_config->ratio = C_RATIO;
  pstr_psy_config->max_allowed_inc_fac = 2.0f;
  pstr_psy_config->min_remaining_thr_fac = 0.01f;

  pstr_psy_config->clip_energy = (CLIP_ENERGY_VALUE_LONG * ccfl) / FRAME_LEN_LONG;
  pstr_psy_config->low_pass_line = (WORD32)((2 * band_width * ccfl) / sample_rate);

  for (sfb = 0; sfb < pstr_psy_config->sfb_count; sfb++) {
    if (pstr_psy_config->sfb_offset[sfb] >= pstr_psy_config->low_pass_line) break;
  }
  pstr_psy_config->sfb_active = sfb;

  iusace_min_snr_init(bit_rate, sample_rate,
                      pstr_psy_config->sfb_offset[pstr_psy_config->sfb_count],
                      pstr_psy_config->sfb_offset, sfb_bark_val, pstr_psy_config->sfb_active,
                      pstr_psy_config->sfb_min_snr);

  return;
}

VOID iusace_psy_short_config_init(WORD32 bit_rate, WORD32 sample_rate, WORD32 band_width,
                                  ia_psy_mod_short_config_struct *pstr_psy_config, WORD32 ccfl) {
  WORD32 sfb;
  WORD32 frame_len_short = (ccfl * FRAME_LEN_SHORT_128) / FRAME_LEN_LONG;
  FLOAT32 sfb_bark_val[MAX_NUM_GROUPED_SFB];

  iusace_sfb_init(sample_rate, EIGHT_SHORT_SEQUENCE, pstr_psy_config->sfb_offset,
                  &(pstr_psy_config->sfb_count), ccfl);

  iusace_bark_values_init(pstr_psy_config->sfb_count, pstr_psy_config->sfb_offset,
                          pstr_psy_config->sfb_offset[pstr_psy_config->sfb_count], sample_rate,
                          sfb_bark_val);

  iusace_thr_quiet_init(pstr_psy_config->sfb_count, pstr_psy_config->sfb_offset, sfb_bark_val,
                        pstr_psy_config->sfb_thr_quiet);

  iusace_spreading_init(
      pstr_psy_config->sfb_count, sfb_bark_val, pstr_psy_config->sfb_mask_low_fac,
      pstr_psy_config->sfb_mask_high_fac, pstr_psy_config->sfb_mask_low_fac_spr_ener,
      pstr_psy_config->sfb_mask_high_fac_spr_ener, bit_rate, EIGHT_SHORT_SEQUENCE);

  pstr_psy_config->ratio = C_RATIO;
  pstr_psy_config->max_allowed_inc_fac = 2.0f;
  pstr_psy_config->min_remaining_thr_fac = 0.01f;

  pstr_psy_config->clip_energy =
      (CLIP_ENERGY_VALUE_SHORT * frame_len_short) / FRAME_LEN_SHORT_128;
  pstr_psy_config->low_pass_line = (WORD32)((2 * band_width * frame_len_short) / sample_rate);

  for (sfb = 0; sfb < pstr_psy_config->sfb_count; sfb++) {
    if (pstr_psy_config->sfb_offset[sfb] >= pstr_psy_config->low_pass_line) break;
  }
  pstr_psy_config->sfb_active = sfb;

  iusace_min_snr_init(bit_rate, sample_rate,
                      pstr_psy_config->sfb_offset[pstr_psy_config->sfb_count],
                      pstr_psy_config->sfb_offset, sfb_bark_val, pstr_psy_config->sfb_active,
                      pstr_psy_config->sfb_min_snr);

  return;
}

IA_ERRORCODE iusace_sfb_params_init(WORD32 sample_rate, WORD32 frame_len, WORD32 *ptr_sfb_width,
                                    WORD32 *num_sfb, WORD32 win_seq) {
  WORD32 i, j, k;
  ia_sfb_info_struct *ptr_sr_info = NULL;
  WORD32 sampling_rate_mapped = 0;

  if (frame_len == 1024) {
    ptr_sr_info = &iusace_sfb_info_1024[0];
  } else {
    ptr_sr_info = &iusace_sfb_info_768[0];
  }

  if (sample_rate < 0)
  {
    return IA_EXHEAACE_INIT_FATAL_USAC_INVALID_CORE_SAMPLE_RATE;
  }

  sampling_rate_mapped = iusace_map_sample_rate(sample_rate);

  while (ptr_sr_info->sample_rate != sampling_rate_mapped) {
    if (ptr_sr_info->sample_rate == -1) {
      return IA_EXHEAACE_INIT_FATAL_USAC_INVALID_CORE_SAMPLE_RATE;
    }
    ptr_sr_info++;
  }

  j = 0;
  for (i = 0; i < ptr_sr_info->num_sfb_long; i++) {
    k = ptr_sr_info->cb_offset_long[i];
    ptr_sr_info->sfb_width_long[i] = k - j;
    j = k;
  }
  j = 0;
  for (i = 0; i < ptr_sr_info->num_sfb_short; i++) {
    k = ptr_sr_info->cb_offset_short[i];
    ptr_sr_info->sfb_width_short[i] = k - j;
    j = k;
  }

  switch (win_seq) {
    case EIGHT_SHORT_SEQUENCE:
      memcpy(ptr_sfb_width, ptr_sr_info->sfb_width_short, (MAX_SFB_SHORT) * sizeof(WORD32));
      *num_sfb = ptr_sr_info->num_sfb_short;
      break;
    case ONLY_LONG_SEQUENCE:
    case LONG_START_SEQUENCE:
    case STOP_START_SEQUENCE:
    case LONG_STOP_SEQUENCE:
    default:
      memcpy(ptr_sfb_width, ptr_sr_info->sfb_width_long, MAX_SFB_LONG * sizeof(WORD32));
      *num_sfb = ptr_sr_info->num_sfb_long;
      break;
  }

  return IA_NO_ERROR;
}
