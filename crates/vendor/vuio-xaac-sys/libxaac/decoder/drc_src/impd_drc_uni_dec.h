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
#ifndef IMPD_DRC_UNI_DEC_H
#define IMPD_DRC_UNI_DEC_H

#ifdef __cplusplus
extern "C" {
#endif

WORD32 impd_select_drc_coefficients(
    ia_drc_config* drc_config, ia_uni_drc_coeffs_struct** drc_coefficients_drc,
    WORD32* drc_coefficients_selected);

WORD32 impd_init_selected_drc_set(
    ia_drc_config* drc_config, ia_drc_params_struct* ia_drc_params_struct,
    WORD32 audio_num_chan, WORD32 drc_set_id_selected,
    WORD32 downmix_id_selected, ia_filter_banks_struct* ia_filter_banks_struct,
    ia_overlap_params_struct* pstr_overlap_params);

#ifdef __cplusplus
}
#endif
#endif
