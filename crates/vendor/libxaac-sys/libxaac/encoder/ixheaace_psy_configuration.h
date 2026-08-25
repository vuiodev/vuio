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

#pragma once

#define db_lin_scale(a) pow(10.0f, (a) * (FLOAT32)0.1f)

#ifndef C_RATIO
#define C_RATIO 0.001258925f /* pow(10.0f, -(29.0f/10.0f)) */
#endif

#define SNR_FLOAT .0001f /* 0.0001 ie. pow(10.0f, -(40.0f/10.0f)) */

#define MAX_BARK_VALUE (24.0f)
#define MASK_LOW_FAC (3.0f)
#define MASK_HIGH_FAC (1.5f)
#define MASK_LOW_SP_ENERGY_L (3.0f)
#define MASK_HIGH_SP_ENERGY_L (2.0f)
#define MASK_HIGH_SP_ENERGY_L_LBR (1.5f)
#define MASK_LOW_SP_ENERGY_S (2.0f)
#define MASK_HIGH_SP_ENERGY_S (1.5f)
#define MASK_HIGH_SP_BITRATE_THRESH (22000)

typedef struct {
  WORD32 sfb_cnt;
  WORD32 sfb_active;
  WORD32 sfb_offsets[MAXIMUM_SCALE_FACTOR_BAND_LONG + 1];
  FLOAT32 sfb_threshold_quiet[MAXIMUM_SCALE_FACTOR_BAND_LONG];
  FLOAT32 min_remaining_threshold_factor;
  WORD32 lowpass_line;
  FLOAT32 clip_energy;
  FLOAT32 ratio_float;
  FLOAT32 sfb_mask_low_factor[MAXIMUM_SCALE_FACTOR_BAND_LONG];
  FLOAT32 sfb_mask_high_factor[MAXIMUM_SCALE_FACTOR_BAND_LONG];
  FLOAT32 sfb_mask_low_factor_spread_nrg[MAXIMUM_SCALE_FACTOR_BAND_LONG];
  FLOAT32 sfb_mask_high_factor_spread_nrg[MAXIMUM_SCALE_FACTOR_BAND_LONG];
  FLOAT32 sfb_min_snr[MAXIMUM_SCALE_FACTOR_BAND_LONG];
  ixheaace_temporal_noise_shaping_config str_tns_conf;
} ixheaace_psy_configuration_long;

typedef struct {
  WORD32 sfb_cnt;
  WORD32 sfb_active;
  WORD32 sfb_offsets[MAXIMUM_SCALE_FACTOR_BAND_SHORT + 1];
  FLOAT32 sfb_threshold_quiet[MAXIMUM_SCALE_FACTOR_BAND_SHORT];
  FLOAT32 min_remaining_threshold_factor;
  WORD32 lowpass_line;
  FLOAT32 clip_energy;
  FLOAT32 ratio_float;
  FLOAT32 sfb_mask_low_factor[MAXIMUM_SCALE_FACTOR_BAND_SHORT];
  FLOAT32 sfb_mask_high_factor[MAXIMUM_SCALE_FACTOR_BAND_SHORT];
  FLOAT32 sfb_mask_low_factor_spread_nrg[MAXIMUM_SCALE_FACTOR_BAND_SHORT];
  FLOAT32 sfb_mask_high_factor_spread_nrg[MAXIMUM_SCALE_FACTOR_BAND_SHORT];
  FLOAT32 sfb_min_snr[MAXIMUM_SCALE_FACTOR_BAND_SHORT];
  ixheaace_temporal_noise_shaping_config str_tns_conf;
} ixheaace_psy_configuration_short;

IA_ERRORCODE ia_enhaacplus_enc_init_psy_configuration(
    WORD32 bit_rate, WORD32 sample_rate, WORD32 bandwidth, WORD32 aot,
    ixheaace_psy_configuration_long *pstr_psy_conf, ixheaace_aac_tables *pstr_aac_tables,
    WORD32 long_frame_len);

IA_ERRORCODE ia_enhaacplus_enc_init_psy_configuration_short(
    WORD32 bit_rate, WORD32 sample_rate, WORD32 bandwidth, WORD32 aot,
    ixheaace_psy_configuration_short *pstr_psy_conf, ixheaace_aac_tables *pstr_aac_tables,
    WORD32 long_frame_len);
