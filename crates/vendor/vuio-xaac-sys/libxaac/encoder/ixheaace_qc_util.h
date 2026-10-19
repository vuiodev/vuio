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

IA_ERRORCODE ia_enhaacplus_enc_qc_out_new(ixheaace_qc_out *pstr_qc_out, WORD32 num_channels,
                                          WORD32 *ptr_shared_buffer1, WORD32 *ptr_shared_buffer3,
                                          WORD32 frame_len_long);

IA_ERRORCODE ia_enhaacplus_enc_qc_new(ixheaace_qc_state *pstr_qc_state,
                                      WORD32 *ptr_shared_buffer_2, WORD32 frame_len_long);

IA_ERRORCODE ia_enhaacplus_enc_qc_init(ixheaace_qc_state *pstr_qc_state, WORD32 aot,
                                       ixheaace_qc_init *pstr_init, FLAG flag_framelength_small);

IA_ERRORCODE ia_enhaacplus_enc_qc_main(
    ixheaace_qc_state *pstr_qc_state, WORD32 num_channels, ixheaace_element_bits *pstr_el_bits,
    ixheaace_psy_out_channel **psy_out_ch,
    ixheaace_psy_out_element *pstr_psy_out_element,
    ixheaace_qc_out_channel **pstr_qc_out_ch,
    ixheaace_qc_out_element *pstr_qc_out_element, WORD32 ancillary_data_bytes,
    ixheaace_aac_tables *pstr_aac_tables, WORD32 adts_flag, WORD32 aot, WORD32 stat_bits_flag,
    WORD32 flag_last_element, WORD32 frame_len_long, WORD8 *ptr_scratch,
    WORD32 *is_quant_spec_zero, WORD32 *is_gain_limited);

VOID ia_enhaacplus_enc_update_bit_reservoir(ixheaace_qc_state *pstr_qc_kernel,
                                            ixheaace_qc_out *pstr_qc_out);

VOID ia_enhaacplus_enc_adjust_bitrate(ixheaace_qc_state *pstr_qc_state, WORD32 bit_rate,
                                      WORD32 sample_rate, WORD32 flag_last_element,
                                      WORD32 frame_len_long);

WORD32 ia_enhaacplus_aac_limitbitrate(WORD32 core_sampling_rate, WORD32 frame_length,
                                      WORD32 num_channels, WORD32 bit_rate);

WORD32 iaace_calc_max_val_in_sfb(WORD32 sfb_count, WORD32 max_sfb_per_grp, WORD32 ptr_sfb_per_grp,
                                 WORD32 *ptr_sfb_offset, WORD16 *ptr_quant_spec,
                                 UWORD16 *ptr_max_value);

IA_ERRORCODE ia_enhaacplus_enc_finalize_bit_consumption(ixheaace_qc_state *pstr_qc_kernel,
                                                        ixheaace_qc_out *pstr_qc_out,
                                                        WORD32 flag_last_element, WORD32 cnt_bits,
                                                        WORD32 *tot_fill_bits,
                                                        iexheaac_encoder_str **pstr_aac_enc,
                                                        WORD32 num_bs_elements, WORD32 aot);