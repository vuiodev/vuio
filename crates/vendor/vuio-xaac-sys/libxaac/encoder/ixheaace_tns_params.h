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

typedef struct {
  WORD32 sampling_rate;
  WORD32 max_band_1024_long_lc;
  WORD32 max_band_1024_short_lc;
  WORD32 max_band_960_long_lc;
  WORD32 max_band_960_short_lc;
  WORD32 max_band_512_ld;
  WORD32 max_band_480_ld;

} ixheaace_temporal_noise_shaping_max_table;

typedef struct {
  WORD32 bit_rate_from;
  WORD32 bit_rate_to;
  const ixheaace_temporal_noise_shaping_config_tabulated
      param_mono_long; /* contains TNS parameters */
  const ixheaace_temporal_noise_shaping_config_tabulated param_mono_short;
  const ixheaace_temporal_noise_shaping_config_tabulated param_stereo_long;
  const ixheaace_temporal_noise_shaping_config_tabulated param_stereo_short;
} ixheaace_temporal_noise_shaping_info_tab;

IA_ERRORCODE
ia_enhaacplus_enc_get_tns_param(
    ixheaace_temporal_noise_shaping_config_tabulated *pstr_tns_config_tab, WORD32 bit_rate,
    WORD32 channels, WORD32 block_type,
    const ixheaace_temporal_noise_shaping_info_tab *pstr_tns_info_tab, WORD32 size);

VOID ia_enhaacplus_enc_get_tns_max_bands(
    WORD32 sampling_rate, WORD32 block_type, WORD32 *tns_max_sfb,
    const ixheaace_temporal_noise_shaping_max_table *pstr_tns_max_bands_tab, WORD32 size,
    WORD32 aot, WORD32 frame_length);
