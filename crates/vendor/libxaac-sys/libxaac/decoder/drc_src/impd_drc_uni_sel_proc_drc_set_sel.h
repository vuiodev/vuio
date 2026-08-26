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
#ifndef IMPD_DRC_UNI_SEL_PROC_DRC_SET_SEL_H
#define IMPD_DRC_UNI_SEL_PROC_DRC_SET_SEL_H

#ifdef __cplusplus
extern "C" {
#endif /* __cplusplus */

#define SELECTION_FLAG_DRC_TARGET_LOUDNESS_MATCH (1 << 0)
#define SELECTION_FLAG_EXPLICIT_PEAK_INFO_PRESENT (1 << 1)

typedef struct {
  WORD32 drc_instructions_index;
  WORD32 downmix_id_request_index;
  WORD32 eq_set_id;
  FLOAT32 output_peak_level;
  FLOAT32 loudness_norm_db_gain_adjusted;
  FLOAT32 output_loudness;
  FLOAT32 mixing_level;
  WORD32 selection_flags;
} ia_selection_candidate_info_struct;

WORD32
impd_validate_requested_drc_feature(
    ia_drc_sel_proc_params_struct* pstr_drc_sel_proc_params_struct);

WORD32
impd_map_requested_effect_bit_idx(WORD32 requested_effect_type,
                                  WORD32* effect_bit_idx);

WORD32
impd_get_fading_drc_set(ia_drc_sel_pro_struct* pstr_drc_uni_sel_proc);

WORD32
impd_get_ducking_drc_set(ia_drc_sel_pro_struct* pstr_drc_uni_sel_proc);

WORD32
impd_get_selected_drc_set(ia_drc_sel_pro_struct* pstr_drc_uni_sel_proc,
                          WORD32 drc_set_id_selected);

WORD32
impd_get_dependent_drc_set(ia_drc_sel_pro_struct* pstr_drc_uni_sel_proc);

WORD32
impd_get_dependent_drc_instructions(
    const ia_drc_config* drc_config,
    const ia_drc_instructions_struct* str_drc_instruction_str,
    ia_drc_instructions_struct** ppstr_drc_instructions_dependent);

WORD32
impd_select_drcs_without_compr_effects(
    ia_drc_config* pstr_drc_config, WORD32* match_found_flag,
    WORD32* selection_candidate_count,
    ia_selection_candidate_info_struct* selection_candidate_info);

WORD32
impd_match_effect_type_attempt(
    ia_drc_config* pstr_drc_config, WORD32 requested_effect_type,
    WORD32 stateRequested, WORD32* match_found_flag,
    WORD32* selection_candidate_count,
    ia_selection_candidate_info_struct* selection_candidate_info);

WORD32
impd_match_effect_types(
    ia_drc_config* pstr_drc_config, WORD32 total_effect_type_requested,
    WORD32 desired_effect_type_requested, WORD32* requested_effect_type,
    WORD32* selection_candidate_count,
    ia_selection_candidate_info_struct* selection_candidate_info);

WORD32 impd_match_dynamic_range(
    ia_drc_config* pstr_drc_config,
    ia_drc_loudness_info_set_struct* pstr_loudness_info,
    ia_drc_sel_proc_params_struct* pstr_drc_sel_proc_params_struct,
    WORD32 num_drc_requests, WORD32* selection_candidate_count,
    ia_selection_candidate_info_struct* selection_candidate_info);

WORD32
impd_match_drc_characteristic_attempt(
    ia_drc_config* pstr_drc_config, WORD32 requested_drc_characteristic,
    WORD32* match_found_flag, WORD32* selection_candidate_count,
    ia_selection_candidate_info_struct* selection_candidate_info);

WORD32
impd_match_drc_characteristic(
    ia_drc_config* pstr_drc_config, WORD32 requested_drc_characteristic,
    WORD32* selection_candidate_count,
    ia_selection_candidate_info_struct* selection_candidate_info);

VOID impd_select_drc_coeff3(
    ia_drc_config* drc_config,
    ia_uni_drc_coeffs_struct** str_p_loc_drc_coefficients_uni_drc);

WORD32
impd_drc_set_preselection(
    ia_drc_sel_proc_params_struct* pstr_drc_sel_proc_params_struct,
    ia_drc_config* pstr_drc_config,
    ia_drc_loudness_info_set_struct* pstr_loudness_info,
    WORD32 restrict_to_drc_with_album_loudness,
    WORD32* selection_candidate_count,
    ia_selection_candidate_info_struct* selection_candidate_info);

WORD32 impd_drc_set_final_selection(
    ia_drc_config* pstr_drc_config,
    ia_drc_sel_proc_params_struct* pstr_drc_sel_proc_params_struct,
    WORD32* selection_candidate_count,
    ia_selection_candidate_info_struct* selection_candidate_info);

WORD32 impd_match_eq_set_purpose(
    ia_drc_config* drc_config, WORD32 eq_set_purpose_requested,
    WORD32* eq_set_id_valid_flag, WORD32* selection_candidate_count,
    ia_selection_candidate_info_struct* selection_candidate_info,
    ia_selection_candidate_info_struct* selection_candidate_info_step_2);

WORD32
impd_select_drc_set(ia_drc_sel_pro_struct* pstr_drc_uni_sel_proc,
                    WORD32* drc_set_id_selected);

#ifdef __cplusplus
}
#endif /* __cplusplus */

#endif
