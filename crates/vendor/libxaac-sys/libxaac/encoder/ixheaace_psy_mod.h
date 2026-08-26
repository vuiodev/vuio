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
  ixheaace_psy_configuration_long psy_conf_long;
  ixheaace_psy_configuration_short psy_conf_short;
  ixheaace_psy_data *psy_data[IXHEAACE_MAX_CH_IN_BS_ELE];
  ixheaace_temporal_noise_shaping_data *temporal_noise_shaping_data[IXHEAACE_MAX_CH_IN_BS_ELE];
  FLOAT32 *p_scratch_tns_float;
} ixheaace_psy_kernel;

WORD32 ia_enhaacplus_enc_psy_new(ixheaace_psy_kernel *pstr_h_psy, WORD32 num_chan,
                                 WORD32 *ptr_shared_buffer_2, WORD32 frame_len_long);

IA_ERRORCODE ia_enhaacplus_enc_psy_main_init(ixheaace_psy_kernel *pstr_h_psy, WORD32 sample_rate,
                                             WORD32 bit_rate, WORD32 channels, WORD32 tns_mask,
                                             WORD32 bandwidth, WORD32 aot,
                                             ixheaace_aac_tables *pstr_aac_tables,
                                             WORD32 frame_length);

IA_ERRORCODE ia_enhaacplus_enc_psy_main(
    WORD32 time_sn_stride, /* total number of channels */
    ixheaace_element_info *pstr_elem_info, const FLOAT32 *ptr_time_signal, WORD32 aot,
    ixheaace_psy_data **psy_data,
    ixheaace_temporal_noise_shaping_data **tns_data,
    ixheaace_psy_configuration_long *pstr_psy_conf_long,
    ixheaace_psy_configuration_short *pstr_psy_conf_short,
    ixheaace_psy_out_channel **psy_out_ch,
    ixheaace_psy_out_element *pstr_psy_out_element, FLOAT32 *ptr_scratch_tns,
    FLOAT32 *ptr_shared_buffer1, WORD8 *ptr_shared_buffer5, ixheaace_aac_tables *pstr_aac_tables,
    WORD32 frame_len_long);
