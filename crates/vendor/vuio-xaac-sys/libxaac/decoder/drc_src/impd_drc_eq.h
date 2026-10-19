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
#ifndef IMPD_DRC_EQ_H
#define IMPD_DRC_EQ_H

#ifndef COMPILE_FOR_DRC_ENCODER
#endif

#define EQ_CHANNEL_COUNT_MAX 8
#define EQ_FIR_FILTER_SIZE_MAX 128
#define EQ_SUBBAND_COUNT_MAX 256
#define EQ_INTERMEDIATE_2ND_ORDER_PARAMS_COUNT_MAX 32
#define EQ_INTERMEDIATE_PARAMETER_COUNT_MAX 32
#define EQ_FILTER_SECTION_COUNT_MAX 8
#define EQ_FILTER_ELEMENT_COUNT_MAX 4
#define EQ_FILTER_COUNT_MAX 4
#define MATCHING_PHASE_FILTER_COUNT_MAX 32

#define EQ_FILTER_DOMAIN_NONE 0
#define EQ_FILTER_DOMAIN_TIME (1 << 0)
#define EQ_FILTER_DOMAIN_SUBBAND (1 << 1)

#define EQ_REAL_ZERO_CNT 63    /*Max Value of a 6 bit number*/
#define EQ_GENERIC_ZERO_CNT 63 /*Max Value of a 6 bit number*/
#define EQ_AUDIO_DELAY_MAX (EQ_REAL_ZERO_CNT + (EQ_GENERIC_ZERO_CNT * 2))

#ifdef __cplusplus
extern "C" {
#endif

typedef struct {
  WORD32 delay;
  FLOAT32 state[EQ_CHANNEL_COUNT_MAX][EQ_AUDIO_DELAY_MAX];
} ia_audio_delay_struct;

typedef struct {
  FLOAT32 radius;
  FLOAT32 coeff[2];
} ia_2nd_order_filt_params_struct;

typedef struct {
  WORD32 coeff_count;
  FLOAT32 coeff[EQ_FIR_FILTER_SIZE_MAX];
  FLOAT32 state[EQ_CHANNEL_COUNT_MAX][EQ_FIR_FILTER_SIZE_MAX];
} ia_fir_filter_struct;

typedef struct {
  WORD32 eq_frame_size_subband;
  WORD32 coeff_count;
  FLOAT32 subband_coeff[EQ_SUBBAND_COUNT_MAX];
} ia_subband_filt_struct;

typedef struct {
  WORD32 filter_format;
  WORD32 filter_param_count_of_zeros;
  ia_2nd_order_filt_params_struct
      ord_2_filt_params_of_zeros[EQ_INTERMEDIATE_2ND_ORDER_PARAMS_COUNT_MAX];
  WORD32 filter_param_count_of_poles;
  ia_2nd_order_filt_params_struct
      ord_2_filt_params_of_poles[EQ_INTERMEDIATE_2ND_ORDER_PARAMS_COUNT_MAX];
  WORD32 filter_param_count_of_fir;
  ia_fir_filter_struct fir_filter;
} ia_interm_filt_params_struct;

typedef struct {
  WORD32 interm_filt_param_count;
  ia_interm_filt_params_struct
      interm_filt_params[EQ_INTERMEDIATE_PARAMETER_COUNT_MAX];
} IntermediateParams;

typedef struct {
  FLOAT32 in_state_1;
  FLOAT32 in_state_2;
  FLOAT32 out_state_1;
  FLOAT32 out_state_2;
} ia_filt_sect_state_struct;

typedef struct {
  FLOAT32 a1;
  FLOAT32 a2;
  FLOAT32 b1;
  FLOAT32 b2;
  ia_filt_sect_state_struct filt_sect_state[EQ_CHANNEL_COUNT_MAX];
} ia_filt_sect_struct;

typedef struct {
  WORD32 member_count;
  WORD32 member_idx[EQ_CHANNEL_GROUP_COUNT_MAX];
} ia_cascade_align_group_struct;

typedef struct {
  WORD32 validity_flag;
  WORD32 num_matches_filter;
  WORD32 matches_filter[EQ_FILTER_SECTION_COUNT_MAX];
  FLOAT32 gain;
  WORD32 section_count;
  ia_filt_sect_struct filt_section[EQ_FILTER_SECTION_COUNT_MAX];
  ia_audio_delay_struct audio_delay;
} ia_ph_alignment_filt_struct;

typedef ia_ph_alignment_filt_struct ia_matching_ph_filt_struct;

typedef struct {
  WORD32 matches_cascade_idx;
  WORD32 all_pass_count;
  ia_matching_ph_filt_struct matching_ph_filt[MATCHING_PHASE_FILTER_COUNT_MAX];
} ia_all_pass_chain_struct;

typedef struct {
  WORD32 section_count;
  ia_filt_sect_struct filt_section[EQ_FILTER_SECTION_COUNT_MAX];
  WORD32 filt_coeffs_flag;
  ia_fir_filter_struct fir_filter;
  ia_audio_delay_struct audio_delay;
} ia_pole_zero_filt_struct;

typedef struct {
  FLOAT32 elementGainLinear;
  WORD32 format;
  ia_pole_zero_filt_struct pstr_pole_zero_filt;
  ia_fir_filter_struct fir_filter;
  WORD32 num_ph_align_filt;
  ia_ph_alignment_filt_struct ph_alignment_filt[EQ_FILTER_ELEMENT_COUNT_MAX];
} ia_eq_filt_ele_struct;

typedef struct {
  WORD32 element_count;
  ia_eq_filt_ele_struct eq_filt_element[EQ_FILTER_ELEMENT_COUNT_MAX];
  ia_matching_ph_filt_struct matching_ph_filt_ele_0;
} ia_eq_filt_block_struct;

typedef struct {
  FLOAT32 cascade_gain_linear;
  WORD32 block_count;
  ia_eq_filt_block_struct pstr_eq_filt_block[EQ_FILTER_BLOCK_COUNT_MAX];
  WORD32 num_ph_align_filt;
  ia_ph_alignment_filt_struct
      ph_alignment_filt[EQ_FILTER_BLOCK_COUNT_MAX * EQ_FILTER_BLOCK_COUNT_MAX];
} ia_filt_cascade_td_struct;

typedef struct {
  WORD32 domain;
  WORD32 audio_num_chan;
  WORD32 eq_ch_group_count;
  WORD32 eq_ch_group_of_channel[EQ_CHANNEL_COUNT_MAX];
  ia_filt_cascade_td_struct filt_cascade_td[EQ_CHANNEL_GROUP_COUNT_MAX];
  ia_subband_filt_struct subband_filt[EQ_CHANNEL_GROUP_COUNT_MAX];
} ia_eq_set_struct;

WORD32
impd_derive_eq_set(ia_eq_coeff_struct* str_eq_coeff,
                   ia_eq_instructions_struct* str_eq_instructions,
                   FLOAT32 sample_rate, WORD32 drc_frame_size,
                   WORD32 sub_band_domain_mode, ia_eq_set_struct* eq_set);

VOID impd_get_eq_set_delay(ia_eq_set_struct* eq_set, WORD32* cascade_delay);
WORD32
impd_process_eq_set_td(ia_eq_set_struct* eq_set, WORD32 channel,
                       FLOAT32 audio_in, FLOAT32* audio_out);

WORD32 impd_process_eq_set_time_domain(ia_eq_set_struct* eq_set, WORD32 channel,
                                       FLOAT32* audio_in, FLOAT32* audio_out,
                                       WORD32 frame_size);
#ifdef __cplusplus
}
#endif
#endif
