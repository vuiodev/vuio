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
#include <stdio.h>

#define MAX_NUM_GROUPED_SFB (60)
#define MAX_BARK_VALUE (24.0f)
#define MASK_LOW_FAC (3.0f)
#define MASK_HIGH_FAC (1.5f)
#define MASK_LOW_SP_ENERGY_L (3.0f)
#define MASK_HIGH_SP_ENERGY_L (2.0f)
#define MASK_HIGH_SP_ENERGY_L_LBR (1.5f)
#define MASK_LOW_SP_ENERGY_S (2.0f)
#define MASK_HIGH_SP_ENERGY_S (1.5f)
#define C_RATIO (0.001258925f)

#define MAXIMUM_SCALE_FACTOR_BAND_LONG 51
#define MAXIMUM_SCALE_FACTOR_BAND_SHORT 15

#define MAX_GROUPED_SFB 51
#define MAX_GROUPED_SFB_TEMP 60
#define BLOCK_SWITCHING_OFFSET (1 * 1024 + 3 * 128 + 64 + 128)
#define MAX_CHANNEL_BITS 6144
#define MAX_SFB_SHORT 15

#define TRANS_FAC 8
#ifndef FRAME_LEN_SHORT_128
#define FRAME_LEN_SHORT_128 (FRAME_LEN_LONG / TRANS_FAC)
#endif

typedef struct {
  WORD32 sfb_count;
  WORD32 sfb_active;
  WORD32 sfb_offset[MAXIMUM_SCALE_FACTOR_BAND_LONG + 1];
  FLOAT32 sfb_thr_quiet[MAXIMUM_SCALE_FACTOR_BAND_LONG];
  FLOAT32 max_allowed_inc_fac;
  FLOAT32 min_remaining_thr_fac;
  WORD32 low_pass_line;
  FLOAT32 clip_energy;
  FLOAT32 ratio;
  FLOAT32 sfb_mask_low_fac[MAXIMUM_SCALE_FACTOR_BAND_LONG];
  FLOAT32 sfb_mask_high_fac[MAXIMUM_SCALE_FACTOR_BAND_LONG];
  FLOAT32 sfb_mask_low_fac_spr_ener[MAXIMUM_SCALE_FACTOR_BAND_LONG];
  FLOAT32 sfb_mask_high_fac_spr_ener[MAXIMUM_SCALE_FACTOR_BAND_LONG];
  FLOAT32 sfb_min_snr[MAXIMUM_SCALE_FACTOR_BAND_LONG];
} ia_psy_mod_long_config_struct;

typedef struct {
  WORD32 sfb_count;
  WORD32 sfb_active;
  WORD32 sfb_offset[MAXIMUM_SCALE_FACTOR_BAND_SHORT + 1];
  FLOAT32 sfb_thr_quiet[MAXIMUM_SCALE_FACTOR_BAND_SHORT];
  FLOAT32 max_allowed_inc_fac;
  FLOAT32 min_remaining_thr_fac;
  WORD32 low_pass_line;
  FLOAT32 clip_energy;
  FLOAT32 ratio;
  FLOAT32 sfb_mask_low_fac[MAXIMUM_SCALE_FACTOR_BAND_SHORT];
  FLOAT32 sfb_mask_high_fac[MAXIMUM_SCALE_FACTOR_BAND_SHORT];
  FLOAT32 sfb_mask_low_fac_spr_ener[MAXIMUM_SCALE_FACTOR_BAND_SHORT];
  FLOAT32 sfb_mask_high_fac_spr_ener[MAXIMUM_SCALE_FACTOR_BAND_SHORT];
  FLOAT32 sfb_min_snr[MAXIMUM_SCALE_FACTOR_BAND_SHORT];
} ia_psy_mod_short_config_struct;

typedef struct {
  WORD32 sfb_count;
  WORD32 max_sfb_per_grp;
  WORD32 sfb_per_group;
  WORD32 window_sequence;
  WORD32 window_shape;
  WORD32 sfb_offsets[100];
  FLOAT32 *ptr_sfb_energy;
  FLOAT32 *ptr_sfb_spread_energy;
  FLOAT32 *ptr_sfb_thr;
  FLOAT64 *ptr_spec_coeffs;
  FLOAT32 sfb_sum_lr_energy;
  FLOAT32 pe;
  FLOAT32 sfb_min_snr[100];
  WORD32 ms_used[100];
} ia_psy_mod_out_data_struct;

typedef struct {
  WORD32 window_sequence;
  FLOAT32 sfb_thr_nm1[MAX_GROUPED_SFB_TEMP];
  FLOAT32 *ptr_sfb_thr_long;
  FLOAT32 sfb_thr_short[TRANS_FAC][MAXIMUM_SCALE_FACTOR_BAND_SHORT];
  FLOAT32 *ptr_sfb_energy_long;
  FLOAT32 ptr_sfb_energy_long_ms[MAX_GROUPED_SFB_TEMP];
  FLOAT32 ptr_sfb_energy_short_ms[TRANS_FAC][MAXIMUM_SCALE_FACTOR_BAND_SHORT];
  FLOAT32 sfb_energy_short[TRANS_FAC][MAXIMUM_SCALE_FACTOR_BAND_SHORT];
  FLOAT32 *ptr_sfb_spreaded_energy_long;
  FLOAT32 sfb_spreaded_energy_short[TRANS_FAC][MAXIMUM_SCALE_FACTOR_BAND_SHORT];
} ia_psy_mod_data_struct;

typedef struct ia_psy_mod_struct {
  ia_psy_mod_long_config_struct str_psy_long_config[MAX_TIME_CHANNELS];
  ia_psy_mod_short_config_struct str_psy_short_config[MAX_TIME_CHANNELS];
  ia_psy_mod_data_struct str_psy_data[MAX_TIME_CHANNELS];
  ia_psy_mod_out_data_struct str_psy_out_data[MAX_TIME_CHANNELS];
  FLOAT32 mdct_spec_coeff_buf[MAX_TIME_CHANNELS][1024];
} ia_psy_mod_struct;

typedef struct ia_sfb_params_struct {
  WORD32 num_sfb[MAX_TIME_CHANNELS];
  WORD32 max_sfb[MAX_TIME_CHANNELS];
  WORD32 max_sfb_ste;
  WORD32 sfb_width_table[MAX_TIME_CHANNELS][MAX_SFB_LONG];
  WORD32 grouped_sfb_offset[MAX_TIME_CHANNELS][MAX_SF_BANDS + 1];
  WORD32 sfb_offset[MAX_TIME_CHANNELS][MAX_SF_BANDS + 1];
  WORD32 num_window_groups[MAX_TIME_CHANNELS];
  WORD32 window_group_length[MAX_TIME_CHANNELS][8];
  WORD32 window_shape[MAX_TIME_CHANNELS];
  WORD32 window_sequence[MAX_TIME_CHANNELS];
  WORD32 common_win[MAX_TIME_CHANNELS];

} ia_sfb_params_struct;

VOID iusace_psy_mod_init(ia_psy_mod_struct *pstr_psy_mod, WORD32 sample_rate, WORD32 bit_rate,
                         WORD32 band_width, WORD32 num_channels, WORD32 ch, WORD32 ele_id,
                         WORD32 ccfl);

VOID iusace_psy_mod_sb(ia_psy_mod_struct *pstr_psy_mod, ia_sfb_params_struct *pstr_sfb_prms,
                       FLOAT64 *ptr_spec_in, ia_tns_info *pstr_tns_info[MAX_TIME_CHANNELS],
                       WORD32 tns_select, WORD32 i_ch, WORD32 chn, WORD32 channel_type,
                       FLOAT64 *scratch_tns_filter, WORD32 elem_idx, FLOAT64 *ptr_tns_scratch,
                       WORD32 ccfl);

VOID iusace_psy_mod_lb(ia_psy_mod_struct *pstr_psy_mod, ia_sfb_params_struct *pstr_sfb_prms,
                       FLOAT64 *ptr_spec_in, ia_tns_info *pstr_tns_info[MAX_TIME_CHANNELS],
                       WORD32 tns_select, WORD32 i_ch, WORD32 chn, WORD32 channel_type,
                       FLOAT64 *scratch_tns_filter, WORD32 elem_idx, FLOAT64 *ptr_tns_scratch,
                       WORD32 ccfl);
