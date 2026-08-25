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
  WORD32 sample_rate;
  WORD32 num_sfb_long;
  WORD32 num_sfb_short;
  const WORD16 *cb_offset_long;
  const WORD16 *cb_offset_short;
  WORD32 sfb_width_long[MAX_SFB_LONG];
  WORD32 sfb_width_short[MAX_SFB_SHORT];

} ia_sfb_info_struct;

VOID iusace_psy_long_config_init(WORD32 bit_rate, WORD32 sample_rate, WORD32 band_width,
                                 ia_psy_mod_long_config_struct *pstr_psy_config, WORD32 ccfl);
VOID iusace_psy_short_config_init(WORD32 bit_rate, WORD32 sample_rate, WORD32 band_width,
                                  ia_psy_mod_short_config_struct *pstr_psy_config, WORD32 ccfl);
VOID iusace_calc_band_energy(const FLOAT64 *ptr_spec_coeffs, const WORD32 *band_offset,
                             const WORD32 num_bands, FLOAT32 *ptr_band_energy, WORD32 sfb_count);
VOID iusace_find_max_spreading(const WORD32 sfb_count, const FLOAT32 *ptr_mask_low_fac,
                               const FLOAT32 *ptr_mask_high_fac, FLOAT32 *ptr_spreaded_enegry);
VOID iusace_pre_echo_control(FLOAT32 *ptr_thr_nm1, WORD32 sfb_count, FLOAT32 max_allowed_inc_fac,
                             FLOAT32 min_remaining_thr_fac, FLOAT32 *ptr_threshold);

IA_ERRORCODE iusace_sfb_params_init(WORD32 sample_rate, WORD32 frame_len, WORD32 *ptr_sfb_width,
                                    WORD32 *num_sfb, WORD32 win_seq);

WORD32 iusace_map_sample_rate(WORD32 sample_rate);

IA_ERRORCODE iusace_validate_baseline_profile_sample_rate(WORD32 sample_rate);
