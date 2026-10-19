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
#include "ixheaac_constants.h"
#include "impd_drc_common_enc.h"
#include "impd_drc_uni_drc.h"
#include "impd_drc_tables.h"
#include "impd_drc_api.h"
#include "ixheaace_api.h"
#include "ixheaace_aac_constants.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops16.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaac_basic_ops.h"
#include "ixheaac_error_standards.h"
#include "ixheaace_error_codes.h"
#include "ixheaace_psy_const.h"
#include "ixheaace_tns.h"
#include "ixheaace_tns_params.h"
#include "ixheaace_rom.h"
#include "ixheaace_common_rom.h"
#include "ixheaace_bitbuffer.h"
#include "ixheaace_psy_configuration.h"

#include "ixheaace_adjust_threshold_data.h"
#include "ixheaace_tns_params.h"
#include "ixheaace_dynamic_bits.h"
#include "ixheaace_qc_data.h"
#include "ixheaace_block_switch.h"
#include "ixheaace_psy_data.h"
#include "ixheaace_interface.h"
#include "ixheaace_adjust_threshold.h"
#include "ixheaace_common_utils.h"

static IA_ERRORCODE ia_enhaacplus_enc_init_sfb_table(ixheaace_psycho_tables *pstr_psycho_tab,
                                                     WORD32 sample_rate, WORD32 aot,
                                                     WORD32 block_type, WORD32 *ptr_sfb_offset,
                                                     WORD32 *sfb_cnt, WORD32 long_frame_len) {
  const UWORD8 *ptr_sfb_param = NULL;
  UWORD32 i;
  WORD32 spec_start_offset, spec_lines = 0;

  /* sfb_info_tab[] */
  for (i = 0; i < sizeof(pstr_psycho_tab->sfb_info_tab) / sizeof(ixheaace_sfb_info_tab); i++) {
    if (pstr_psycho_tab->sfb_info_tab[i].sample_rate == sample_rate) {
      switch (block_type) {
        case LONG_WINDOW:
        case START_WINDOW:
        case STOP_WINDOW:
          if (aot == AOT_AAC_LC || aot == AOT_SBR || aot == AOT_PS) {
            if (long_frame_len == FRAME_LEN_960) {
              ptr_sfb_param = pstr_psycho_tab->sfb_info_tab[i].param_long_960;
            } else {
              ptr_sfb_param = pstr_psycho_tab->sfb_info_tab[i].param_long;
            }
            spec_lines = long_frame_len;
            break;
          } else if (aot == AOT_AAC_LD || aot == AOT_AAC_ELD) {
            if (long_frame_len == FRAME_LEN_480)
              ptr_sfb_param = pstr_psycho_tab->sfb_info_tab[i].param_long_480_ld;
            else
              ptr_sfb_param = pstr_psycho_tab->sfb_info_tab[i].param_long_512_ld;
            spec_lines = long_frame_len;
            break;
          }

        case SHORT_WINDOW:
          if (long_frame_len == FRAME_LEN_SHORT_120) {
            ptr_sfb_param = pstr_psycho_tab->sfb_info_tab[i].param_short_120;
          } else {
            ptr_sfb_param = pstr_psycho_tab->sfb_info_tab[i].param_short;
          }
          spec_lines = long_frame_len;
          break;
      }
      break;
    }
  }

  if (ptr_sfb_param == NULL) {
    return IA_EXHEAACE_INIT_FATAL_SCALE_FACTOR_BAND_NOT_SUPPORTED;
  }

  *sfb_cnt = 0;
  spec_start_offset = 0;

  do {
    ptr_sfb_offset[*sfb_cnt] = spec_start_offset;

    spec_start_offset += ptr_sfb_param[*sfb_cnt];

    (*sfb_cnt)++;
  } while (spec_start_offset < spec_lines);

  if (spec_start_offset != spec_lines) {
    return IA_EXHEAACE_INIT_FATAL_SFB_TABLE_INIT_FAILED;
  }

  ptr_sfb_offset[*sfb_cnt] = spec_start_offset;

  return IA_NO_ERROR;
}

FLOAT32 iaace_atan_approx(FLOAT32 val) {
  if (val < (FLOAT32)1.0f) {
    return (val / ((FLOAT32)1.0f + (FLOAT32)0.280872f * val * val));
  } else {
    return ((FLOAT32)1.57079633f - val / ((FLOAT32)0.280872f + val * val));
  }
}

static FLOAT32 iaace_calc_bark_line_value(WORD32 num_lines, WORD32 fft_line, WORD32 sample_rate) {
  FLOAT32 center_freq, temp, b_value;
  center_freq = (FLOAT32)fft_line * ((FLOAT32)sample_rate * (FLOAT32)0.5f) / (FLOAT32)num_lines;
  temp = (FLOAT32)iaace_atan_approx((FLOAT32)1.3333333e-4f * center_freq);
  b_value = (FLOAT32)13.3f * iaace_atan_approx((FLOAT32)0.00076f * center_freq) +
            (FLOAT32)3.5f * temp * temp;
  return (b_value);
}

static VOID iaace_thr_quiet_init(WORD32 sfb_count, WORD32 *ptr_sfb_offset, FLOAT32 *ptr_bark_val,
                                 FLOAT32 *ptr_thr_quiet, const FLOAT32 *ptr_bark_quiet_thr_val,
                                 WORD32 aot) {
  WORD32 i;
  FLOAT32 bark_thr_quiet;

  for (i = 0; i < sfb_count; i++) {
    if (aot == AOT_AAC_ELD) {
      ptr_thr_quiet[i] = (FLOAT32)(ptr_sfb_offset[i + 1] - ptr_sfb_offset[i]) * 42;
    } else {
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
      bark_thr_quiet = MIN(ptr_bark_quiet_thr_val[b_val1], ptr_bark_quiet_thr_val[b_val2]);

      ptr_thr_quiet[i] = (FLOAT32)db_lin_scale(bark_thr_quiet - 20.0f) * 16887.8f *
                         (FLOAT32)(ptr_sfb_offset[i + 1] - ptr_sfb_offset[i]);
    }
  }
}

static VOID iaace_spreading_init(WORD32 sfb_count, FLOAT32 *ptr_bark_val,
                                 FLOAT32 *ptr_mask_low_fac, FLOAT32 *ptr_mask_high_fac,
                                 FLOAT32 *ptr_mask_low_fac_spr_energy,
                                 FLOAT32 *ptr_mask_high_fac_spr_energy, const WORD32 bit_rate,
                                 WORD32 block_type) {
  WORD32 i;
  FLOAT32 mask_low_spr_energy, mask_high_spr_energy;

  if (block_type != SHORT_WINDOW) {
    mask_low_spr_energy = MASK_LOW_SP_ENERGY_L;
    mask_high_spr_energy = (bit_rate > MASK_HIGH_SP_BITRATE_THRESH) ? MASK_HIGH_SP_ENERGY_L
                                                                    : MASK_HIGH_SP_ENERGY_L_LBR;
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

static VOID iaace_bark_values_init(WORD32 sfb_count, WORD32 *ptr_sfb_offset, WORD32 num_lines,
                                   WORD32 sample_rate, FLOAT32 *ptr_b_value, WORD32 aot) {
  WORD32 i;
  FLOAT32 b_val0, b_val1;
  b_val0 = 0.0f;

  for (i = 0; i < sfb_count; i++) {
    b_val1 = iaace_calc_bark_line_value(num_lines, ptr_sfb_offset[i + 1], sample_rate);
    ptr_b_value[i] = (b_val0 + b_val1) * (FLOAT32)0.5f;
    if (aot == AOT_AAC_ELD) {
      if (ptr_b_value[i] > MAX_BARK_VALUE) {
        ptr_b_value[i] = MAX_BARK_VALUE;
      }
    }

    b_val0 = b_val1;
  }
}

static FLOAT32 iaace_bits_to_pe(const FLOAT32 bits) { return (bits * 1.18f); }
static VOID iaace_min_snr_init(const WORD32 bit_rate, const WORD32 sample_rate,
                               const WORD32 num_lines, const WORD32 *ptr_sfb_offset,
                               const FLOAT32 *ptr_bark_value, const WORD32 sfb_active,
                               FLOAT32 *ptr_sfb_min_snr) {
  WORD32 sfb;
  FLOAT32 bark_fac;
  FLOAT32 bark_width;
  FLOAT32 pe_per_window, pe_part;
  FLOAT32 snr;
  FLOAT32 b_val0, b_val1;

  bark_fac = (FLOAT32)1.0 / MIN(ptr_bark_value[sfb_active - 1] / MAX_BARK_VALUE, (FLOAT32)1.0);
  pe_per_window = iaace_bits_to_pe((FLOAT32)bit_rate / (FLOAT32)sample_rate * (FLOAT32)num_lines);
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
}

IA_ERRORCODE ia_enhaacplus_enc_init_psy_configuration(
    WORD32 bit_rate, WORD32 sample_rate, WORD32 bandwidth, WORD32 aot,
    ixheaace_psy_configuration_long *pstr_psy_conf, ixheaace_aac_tables *pstr_aac_tables,
    WORD32 long_frame_len) {
  WORD32 sfb;
  FLOAT32 sfb_bark_val[MAXIMUM_SCALE_FACTOR_BAND_LONG];
  IA_ERRORCODE error;
  error = ia_enhaacplus_enc_init_sfb_table(pstr_aac_tables->pstr_psycho_tab, sample_rate, aot,
                                           LONG_WINDOW, pstr_psy_conf->sfb_offsets,
                                           &(pstr_psy_conf->sfb_cnt), long_frame_len);
  if (error != IA_NO_ERROR) {
    return error;
  }

  /*   calculate bark values for each pb */
  iaace_bark_values_init(pstr_psy_conf->sfb_cnt, pstr_psy_conf->sfb_offsets,
                         pstr_psy_conf->sfb_offsets[pstr_psy_conf->sfb_cnt], sample_rate,
                         sfb_bark_val, aot);

  /*init thresholds in quiet  */

  iaace_thr_quiet_init(pstr_psy_conf->sfb_cnt, pstr_psy_conf->sfb_offsets, sfb_bark_val,
                       pstr_psy_conf->sfb_threshold_quiet,
                       pstr_aac_tables->pstr_psycho_tab->ixheaace_bark_quiet_thr_val, aot);

  /*  calculate spreading function */

  iaace_spreading_init(pstr_psy_conf->sfb_cnt, sfb_bark_val, pstr_psy_conf->sfb_mask_low_factor,
                       pstr_psy_conf->sfb_mask_high_factor,
                       pstr_psy_conf->sfb_mask_low_factor_spread_nrg,
                       pstr_psy_conf->sfb_mask_high_factor_spread_nrg, bit_rate, LONG_WINDOW);

  /*
    init ratio
  */

  pstr_psy_conf->min_remaining_threshold_factor = MIN_THRESH_FAC;

  pstr_psy_conf->clip_energy = CLIP_ENERGY_VALUE_LONG;
  if (aot == AOT_AAC_LC || aot == AOT_SBR || aot == AOT_PS) {
    pstr_psy_conf->ratio_float = C_RATIO;
  } else if (aot == AOT_AAC_LD || aot == AOT_AAC_ELD) {
    if (aot == AOT_AAC_ELD) {
      pstr_psy_conf->ratio_float = C_RATIO;
    } else {
      pstr_psy_conf->ratio_float = SNR_FLOAT;
    }
  }

  pstr_psy_conf->lowpass_line = (WORD32)((2 * bandwidth * long_frame_len) / sample_rate);

  /* pstr_psy_conf->pstr_sfb_offset[] */
  for (sfb = 0; sfb < pstr_psy_conf->sfb_cnt; sfb++) {
    if (pstr_psy_conf->sfb_offsets[sfb] >= pstr_psy_conf->lowpass_line) break;
  }
  pstr_psy_conf->sfb_active = sfb;

  /*
    calculate minSnr
  */
  iaace_min_snr_init(bit_rate, sample_rate, pstr_psy_conf->sfb_offsets[pstr_psy_conf->sfb_cnt],
                     pstr_psy_conf->sfb_offsets, sfb_bark_val, pstr_psy_conf->sfb_active,
                     pstr_psy_conf->sfb_min_snr);

  return IA_NO_ERROR;
}

IA_ERRORCODE ia_enhaacplus_enc_init_psy_configuration_short(
    WORD32 bit_rate, WORD32 sample_rate, WORD32 bandwidth, WORD32 aot,
    ixheaace_psy_configuration_short *pstr_psy_conf, ixheaace_aac_tables *pstr_aac_tables,
    WORD32 long_frame_len) {
  WORD32 sfb;
  FLOAT32 sfb_bark_val[MAXIMUM_SCALE_FACTOR_BAND_SHORT] = {0};
  IA_ERRORCODE error;
  /*
    init sfb table
  */
  error = ia_enhaacplus_enc_init_sfb_table(pstr_aac_tables->pstr_psycho_tab, sample_rate, aot,
                                           SHORT_WINDOW, pstr_psy_conf->sfb_offsets,
                                           &(pstr_psy_conf->sfb_cnt), long_frame_len);

  if (error != IA_NO_ERROR) {
    return error;
  }

  iaace_bark_values_init(pstr_psy_conf->sfb_cnt, pstr_psy_conf->sfb_offsets,
                         pstr_psy_conf->sfb_offsets[pstr_psy_conf->sfb_cnt], sample_rate,
                         sfb_bark_val, aot);

  iaace_thr_quiet_init(pstr_psy_conf->sfb_cnt, pstr_psy_conf->sfb_offsets, sfb_bark_val,
                       pstr_psy_conf->sfb_threshold_quiet,
                       pstr_aac_tables->pstr_psycho_tab->ixheaace_bark_quiet_thr_val, aot);

  /*
    calculate spreading function
  */
  iaace_spreading_init(pstr_psy_conf->sfb_cnt, sfb_bark_val, pstr_psy_conf->sfb_mask_low_factor,
                       pstr_psy_conf->sfb_mask_high_factor,
                       pstr_psy_conf->sfb_mask_low_factor_spread_nrg,
                       pstr_psy_conf->sfb_mask_high_factor_spread_nrg, bit_rate, SHORT_WINDOW);

  /*
    init ratio
  */

  pstr_psy_conf->min_remaining_threshold_factor = 0.01f;

  pstr_psy_conf->clip_energy = CLIP_ENERGY_VALUE_LONG / (TRANS_FAC * TRANS_FAC);
  switch (aot) {
    case AOT_AAC_LC:
    case AOT_SBR:
    case AOT_PS:
      pstr_psy_conf->ratio_float = C_RATIO;
      break;

    case AOT_AAC_LD:
    case AOT_AAC_ELD:
      pstr_psy_conf->ratio_float = SNR_FLOAT;
      break;
  }

  pstr_psy_conf->lowpass_line = (WORD32)((2 * bandwidth * long_frame_len) / sample_rate);
  for (sfb = 0; sfb < pstr_psy_conf->sfb_cnt; sfb++) {
    if (pstr_psy_conf->sfb_offsets[sfb] >= pstr_psy_conf->lowpass_line) break;
  }
  pstr_psy_conf->sfb_active = sfb;

  iaace_min_snr_init(bit_rate, sample_rate, pstr_psy_conf->sfb_offsets[pstr_psy_conf->sfb_cnt],
                     pstr_psy_conf->sfb_offsets, sfb_bark_val, pstr_psy_conf->sfb_active,
                     pstr_psy_conf->sfb_min_snr);

  return IA_NO_ERROR;
}

WORD32 ia_enhaacplus_enc_power2(WORD32 power, WORD q_power, WORD *q_result,
                                const WORD32 *power_of_2_table_neg,
                                const WORD32 *power_of_2_table_pos) {
  WORD32 int_part, frac_part;
  WORD32 result;
  WORD16 sign = 0;

  if (power < 0) {
    sign = 1;
    power = ixheaac_negate32(power);
  }

  if (q_power < 32)
    int_part = ixheaac_shr32(power, q_power);
  else
    int_part = 0;

  if (q_power < 39) {
    power = ixheaac_shr32_dir(power, q_power - 8);
    frac_part = power & 0xFF;
  } else
    frac_part = 0;

  if (!sign) {
    result = power_of_2_table_pos[frac_part];
    *q_result = Q_POWER2_TABLE - int_part;
  } else {
    result = power_of_2_table_neg[frac_part];
    *q_result = Q_POWER2_TABLE + int_part;
  }

  return (result);
}
