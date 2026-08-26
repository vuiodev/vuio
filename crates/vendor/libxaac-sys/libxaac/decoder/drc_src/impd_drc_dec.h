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
#ifndef IMPD_DRC_DEC_H
#define IMPD_DRC_DEC_H

WORD32 impd_init_drc_params(WORD32 frame_size, WORD32 sample_rate,
                            WORD32 gain_delay_samples, WORD32 delay_mode,
                            WORD32 sub_band_domain_mode,
                            ia_drc_params_struct* ia_drc_params_struct);

WORD32 impd_init_selected_drc_set(
    ia_drc_config* drc_config, ia_drc_params_struct* ia_drc_params_struct,
    ia_parametric_drc_params_struct* p_parametricdrc_params,
    WORD32 audio_num_chan, WORD32 drc_set_id_selected,
    WORD32 downmix_id_selected, ia_filter_banks_struct* ia_filter_banks_struct,
    ia_overlap_params_struct* pstr_overlap_params

    ,
    shape_filter_block* shape_filter_block);

VOID impd_apply_gains_and_add(
    ia_drc_instructions_struct* pstr_drc_instruction_arr,
    const WORD32 drc_instructions_index,
    ia_drc_params_struct* ia_drc_params_struct,
    ia_gain_buffer_struct* pstr_gain_buf,
    shape_filter_block shape_filter_block[], FLOAT32* deinterleaved_audio[],

    FLOAT32* channel_audio[], WORD32 impd_apply_gains);

#endif
