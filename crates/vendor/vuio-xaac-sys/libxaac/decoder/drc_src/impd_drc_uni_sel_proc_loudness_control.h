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
#ifndef IMPD_DRC_UNI_SEL_PROC_LOUDNESS_CONTROL_H
#define IMPD_DRC_UNI_SEL_PROC_LOUDNESS_CONTROL_H

#ifdef __cplusplus
extern "C" {
#endif /* __cplusplus */

WORD32
impd_signal_peak_level_info(ia_drc_config* pstr_drc_config,
                            ia_drc_loudness_info_set_struct* pstr_loudness_info,
                            ia_drc_instructions_struct* str_drc_instruction_str,
                            const WORD32 requested_dwnmix_id,
                            const WORD32 album_mode,
                            const WORD32 num_compression_eq_count,
                            const WORD32* num_compression_eq_id,
                            WORD32* peak_info_count, WORD32 eq_set_id[],
                            FLOAT32 signal_peak_level[],
                            WORD32 explicit_peak_information_present[]);

WORD32
impd_extract_loudness_peak_to_average_info(
    ia_loudness_info_struct* loudness_info,
    const WORD32 dyn_range_measurement_type,
    WORD32* loudness_peak_2_avg_value_present,
    FLOAT32* loudness_peak_2_avg_value);

WORD32
impd_overall_loudness_present(const ia_loudness_info_struct* loudness_info,
                              WORD32* loudness_info_present);

WORD32
impd_find_overall_loudness_info(
    ia_drc_sel_proc_params_struct* pstr_drc_sel_proc_params_struct,
    ia_drc_loudness_info_set_struct* pstr_loudness_info,
    const WORD32 requested_dwnmix_id, const WORD32 drc_set_id_requested,
    WORD32* overall_loudness_info_present, WORD32* loudness_info_count,
    ia_loudness_info_struct* loudness_info[]);

WORD32
impd_high_pass_loudness_adjust_info(
    const ia_loudness_info_struct* loudness_info,
    WORD32* loudness_hp_adjust_present, FLOAT32* loudness_hp_adjust);

WORD32
impd_find_high_pass_loudness_adjust(
    ia_drc_loudness_info_set_struct* pstr_loudness_info,
    const WORD32 requested_dwnmix_id, const WORD32 drc_set_id_requested,
    const WORD32 album_mode, const FLOAT32 device_cutoff_freq,
    WORD32* loudness_hp_adjust_present, FLOAT32* loudness_hp_adjust);

WORD32
impd_init_loudness_control(
    ia_drc_sel_proc_params_struct* pstr_drc_sel_proc_params_struct,
    ia_drc_loudness_info_set_struct* pstr_loudness_info,
    const WORD32 requested_dwnmix_id, const WORD32 drc_set_id_requested,
    const WORD32 num_compression_eq_count, const WORD32* num_compression_eq_id,
    WORD32* loudness_info_count, WORD32 eq_set_id[],
    FLOAT32 loudness_normalization_gain_db[], FLOAT32 loudness[]);

#ifdef __cplusplus
}
#endif /* __cplusplus */

#endif
