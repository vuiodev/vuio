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
IA_ERRORCODE
ia_enhaacplus_enc_init_tns_configuration(WORD32 bit_rate, WORD32 sample_rate, WORD32 channels,
                                         ixheaace_temporal_noise_shaping_config *pstr_tns_config,
                                         ixheaace_psy_configuration_long *pstr_psy_config,
                                         WORD32 active,
                                         ixheaace_temporal_noise_shaping_tables *pstr_tns_tab,
                                         WORD32 frame_len_long, WORD32 aot);

IA_ERRORCODE ia_enhaacplus_enc_init_tns_configuration_short(
    WORD32 bit_rate, WORD32 sample_rate, WORD32 channels,
    ixheaace_temporal_noise_shaping_config *pstr_tns_config,
    ixheaace_psy_configuration_short *pstr_psy_config, WORD32 active,
    ixheaace_temporal_noise_shaping_tables *pstr_tns_tab, WORD32 frame_len_long, WORD32 aot);

IA_ERRORCODE ia_enhaacplus_enc_tns_detect(ixheaace_temporal_noise_shaping_data *pstr_tns_data,
                                          ixheaace_temporal_noise_shaping_config tns_config,
                                          FLOAT32 *ptr_scratch_tns, const WORD32 *ptr_sfb_offset,
                                          FLOAT32 *ptr_spectrum, WORD32 sub_blk_num,
                                          WORD32 block_type, WORD32 aot, FLOAT32 *ptr_sfb_energy,
                                          FLOAT32 *ptr_shared_buffer1, WORD32 long_frame_len);

VOID ia_enhaacplus_enc_tns_sync(ixheaace_temporal_noise_shaping_data *pstr_tns_data_dst,
                                const ixheaace_temporal_noise_shaping_data *pstr_tns_data_src,
                                const ixheaace_temporal_noise_shaping_config tns_config,
                                const WORD32 sub_blk_num, const WORD32 block_type);

VOID ia_enhaacplus_enc_tns_encode(ixheaace_temporal_noise_shaping_params *pstr_tns_info,
                                    ixheaace_temporal_noise_shaping_data *pstr_tns_data,
                                    WORD32 num_sfb,
                                    ixheaace_temporal_noise_shaping_config tns_config,
                                    WORD32 lowpass_line, FLOAT32 *ptr_spectrum,
                                    WORD32 sub_blk_num, WORD32 block_type,
                                    ixheaace_temporal_noise_shaping_tables *pstr_tns_tab);

VOID ia_enhaacplus_enc_apply_tns_mult_table_to_ratios(WORD32 startCb, WORD32 stopCb,
                                                      FLOAT32 *thresholds);

VOID ia_enhaacplus_enc_calc_weighted_spectrum(FLOAT32 *ptr_spectrum, FLOAT32 *ptr_weighted_spec,
                                              FLOAT32 *ptr_sfb_energy,
                                              const WORD32 *ptr_sfb_offset, WORD32 lpc_start_line,
                                              WORD32 lpc_stop_line, WORD32 lpc_start_band,
                                              WORD32 lpc_stop_band, FLOAT32 *ptr_shared_buffer1,
                                              WORD32 aot);

VOID ia_enhaacplus_enc_auto_correlation(const FLOAT32 *ptr_input, FLOAT32 *ptr_corr,
                                        WORD32 samples, WORD32 corr_coeff);

VOID ia_enhaacplus_enc_analysis_filter_lattice(const FLOAT32 *ptr_signal, WORD32 num_lines,
                                               const FLOAT32 *ptr_par_coeff, WORD32 order,
                                               FLOAT32 *ptr_output);
