/******************************************************************************
 *
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
#ifndef IMPD_DRC_FILTER_BANK_H
#define IMPD_DRC_FILTER_BANK_H

#define FILTER_BANK_PARAMETER_COUNT 16
#define CASCADE_ALLPASS_COUNT_MAX 9
#define QMF_NUM_FILT_BANDS 64
#define QMF_FILT_RESOLUTION 64

typedef struct ia_drc_qmf_filt_struct {
  FLOAT64* ana_buff;
  FLOAT64* syn_buff;
  FLOAT64 ana_tab_real[QMF_NUM_FILT_BANDS][2 * QMF_NUM_FILT_BANDS];
  FLOAT64 ana_tab_imag[QMF_NUM_FILT_BANDS][2 * QMF_NUM_FILT_BANDS];
  FLOAT64 syn_tab_real[2 * QMF_NUM_FILT_BANDS][QMF_NUM_FILT_BANDS];
  FLOAT64 syn_tab_imag[2 * QMF_NUM_FILT_BANDS][QMF_NUM_FILT_BANDS];

} ia_drc_qmf_filt_struct;

typedef struct {
  FLOAT32 f_cross_norm;
  FLOAT32 gamma;
  FLOAT32 delta;
} ia_filter_bank_params_struct;

typedef struct {
  FLOAT32 s00;
  FLOAT32 s01;
  FLOAT32 s10;
  FLOAT32 s11;
} ia_lr_filter_state_struct;

typedef struct {
  FLOAT32 s0;
  FLOAT32 s1;
} ia_all_pass_filter_state_struct;

typedef struct {
  FLOAT32 a0;
  FLOAT32 a1;
  FLOAT32 a2;
  FLOAT32 b0;
  FLOAT32 b1;
  FLOAT32 b2;
  FLOAT32 x_p[MAX_CHANNEL_COUNT * 2];
  FLOAT32 y_p[MAX_CHANNEL_COUNT * 2];
} ia_iir_filter_struct;
typedef struct {
  ia_iir_filter_struct low_pass;
  ia_iir_filter_struct high_pass;
} ia_two_band_filt_struct;

typedef struct {
  ia_iir_filter_struct str_low_pass_stage_1;
  ia_iir_filter_struct str_high_pass_stage_1;
  ia_iir_filter_struct str_low_pass_stage_2;
  ia_iir_filter_struct str_high_pass_stage_2;
  ia_iir_filter_struct str_all_pass_stage_2;
} ia_three_band_filt_struct;

typedef struct {
  ia_iir_filter_struct str_low_pass_stage_1;
  ia_iir_filter_struct str_high_pass_stage_1;
  ia_iir_filter_struct str_all_pass_stage_2_high;
  ia_iir_filter_struct str_all_pass_stage_2_low;
  ia_iir_filter_struct str_low_pass_stage_3_high;
  ia_iir_filter_struct str_high_pass_stage_3_high;
  ia_iir_filter_struct str_low_pass_stage_3_low;
  ia_iir_filter_struct str_high_pass_stage_3_low;
} ia_four_band_filt_struct;

typedef struct {
  ia_iir_filter_struct str_all_pass_stage;
} ia_all_pass_filter_sturct;

typedef struct {
  ia_all_pass_filter_sturct
      str_all_pass_cascade_filter[CASCADE_ALLPASS_COUNT_MAX];
  WORD32 num_filter;
} ia_all_pass_cascade_struct;

typedef struct {
  WORD32 num_bands;
  WORD32 complexity;
  ia_two_band_filt_struct str_two_band_bank;
  ia_three_band_filt_struct str_three_band_bank;
  ia_four_band_filt_struct str_four_band_bank;
  ia_all_pass_cascade_struct str_all_pass_cascade;
} ia_drc_filter_bank_struct;

typedef struct {
  WORD32 nfilter_banks;
  WORD32 num_ph_align_ch_groups;
  WORD32 complexity;
  ia_drc_filter_bank_struct str_drc_filter_bank[8];
} ia_filter_banks_struct;

WORD32
impd_init_all_filter_banks(
    ia_uni_drc_coeffs_struct* str_p_loc_drc_coefficients_uni_drc,
    ia_drc_instructions_struct* str_drc_instruction_str,
    ia_filter_banks_struct* ia_filter_banks_struct);

VOID impd_two_band_filter_process(ia_two_band_filt_struct* str_two_band_bank,
                                  WORD32 c, WORD32 size, FLOAT32* audio_in,
                                  FLOAT32* audio_out[]);

VOID impd_three_band_filter_process(
    ia_three_band_filt_struct* str_three_band_bank, WORD32 c, WORD32 size,
    FLOAT32* audio_in, FLOAT32* audio_out[]);

VOID impd_four_band_filter_process(ia_four_band_filt_struct* str_four_band_bank,
                                   WORD32 c, WORD32 size, FLOAT32* audio_in,
                                   FLOAT32* audio_out[]);

VOID impd_all_pass_cascade_process(
    ia_all_pass_cascade_struct* str_all_pass_cascade, WORD32 c, WORD32 size,
    FLOAT32* audio_in);

VOID impd_shape_filt_block_init(
    ia_shape_filter_block_params_struct* pstr_shape_filter_block_params,
    shape_filter_block* shape_filter_block);

VOID impd_shape_filt_block_adapt(const FLOAT32 drc_gain,
                                 shape_filter_block* shape_filter_block);

VOID impd_shape_filt_block_time_process(shape_filter_block* shape_filter_block,
                                        FLOAT32* drc_gain, const WORD32 channel,
                                        FLOAT32* audio_in, WORD32 start,
                                        WORD32 end);
#endif
