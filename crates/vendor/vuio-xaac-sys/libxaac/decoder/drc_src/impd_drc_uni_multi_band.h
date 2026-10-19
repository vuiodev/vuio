/******************************************************************************
 *
 * Copyright (C) 2015 The Android Open Source Project
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
#ifndef IMPD_DRC_UNI_MULTI_BAND_H
#define IMPD_DRC_UNI_MULTI_BAND_H

#define DRC_SUBBAND_COUNT_WITH_AUDIO_CODEC_FILTERBANK_MAX \
  FILTER_BANK_PARAMETER_COUNT

#ifdef __cplusplus
extern "C" {
#endif

typedef struct {
  FLOAT32 overlap_weight[AUDIO_CODEC_SUBBAND_COUNT_MAX];
} ia_band_overlap_params_struct;

typedef struct {
  ia_band_overlap_params_struct str_band_overlap_params[BAND_COUNT_MAX];
} ia_group_overlap_params_struct;

typedef struct {
  ia_group_overlap_params_struct
      str_group_overlap_params[CHANNEL_GROUP_COUNT_MAX];
} ia_overlap_params_struct;

WORD32
impd_fcenter_norm_sb_init(WORD32 num_subbands, FLOAT32* fcenter_norm_subband);

WORD32
impd_generate_slope(WORD32 num_subbands, FLOAT32* fcenter_norm_subband,
                    FLOAT32 fcross_norm_lo, FLOAT32 fcross_norm_hi,
                    FLOAT32* response);

WORD32
impd_generate_overlap_weights(
    WORD32 num_drc_bands, WORD32 drc_band_type,
    ia_gain_params_struct* gain_params, WORD32 dec_subband_count,
    ia_group_overlap_params_struct* pstr_group_overlap_params);

WORD32
impd_init_overlap_weight(
    ia_uni_drc_coeffs_struct* str_p_loc_drc_coefficients_uni_drc,
    ia_drc_instructions_struct* str_drc_instruction_str,
    WORD32 sub_band_domain_mode, ia_overlap_params_struct* pstr_overlap_params);

#ifdef __cplusplus
}
#endif
#endif
