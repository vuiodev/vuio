/******************************************************************************
 *                                                                            *
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
#ifndef IXHEAACD_HEADERDECODE_H
#define IXHEAACD_HEADERDECODE_H

typedef struct {
  WORD32 bit_stream_type;
  WORD32 prog_config_present;
  ia_program_config_struct str_prog_config;
} ia_adif_header_struct;

WORD32 ixheaacd_aac_headerdecode(
    ia_exhaacplus_dec_api_struct *p_obj_enhaacplus_dec, UWORD8 *buffer,
    WORD32 *bytes_consumed,
    const ia_aac_dec_huffman_tables_struct *ptr_huffmann_tables);

ia_aac_decoder_struct *ixheaacd_aac_decoder_init(
    ia_aac_dec_state_struct *p_state_enhaacplus_dec,
    ia_aac_dec_sbr_bitstream_struct *ptr_sbr_bitstream, WORD channels,
    VOID *aac_persistent_mem_v, WORD32 frame_length);

VOID ixheaacd_fill_prog_config_slots(
    ia_aac_dec_state_struct *p_state_enhaacplus_dec);

WORD ixheaacd_decode_pce(struct ia_bit_buf_struct *it_bit_buff,
                         UWORD32 *ui_pce_found_in_hdr,
                         ia_program_config_struct *pce_t);

WORD32 ixheaacd_ga_hdr_dec(ia_aac_dec_state_struct *pstate_aac_dec,
                           WORD32 header_len, WORD32 *bytes_consumed,
                           ia_sampling_rate_info_struct *pstr_samp_rate_info,
                           struct ia_bit_buf_struct *it_bit_buff);

WORD32 ixheaacd_latm_audio_mux_element(
    struct ia_bit_buf_struct *it_bit_buff, ixheaacd_latm_struct *latm_element,
    ia_aac_dec_state_struct *aac_state_struct,
    ia_sampling_rate_info_struct *sample_rate_info);

WORD32 ixheaacd_get_element_index_tag(
    ia_exhaacplus_dec_api_struct *p_obj_exhaacplus_dec, WORD ch_idx1,
    WORD *ch_idx, WORD *channel, WORD *element_index_order, WORD total_elements,
    WORD8 *element_used, WORD total_channels, ia_drc_dec_struct *pstr_drc_dec,
    ia_drc_dec_struct *drc_dummy
    ,
    UWORD8 *mps_buffer,
    WORD32 *mps_header,
    WORD32 *mps_bytes
);

WORD32 ixheaacd_latm_audio_mux_element(
    struct ia_bit_buf_struct *it_bit_buff, ixheaacd_latm_struct *latm_element,
    ia_aac_dec_state_struct *aac_state_struct,
    ia_sampling_rate_info_struct *ptr_samp_rate_info);

VOID ixheaacd_set_sbr_persistent_table_pointer(
    VOID *sbr_persistent_mem_v, ia_sbr_tables_struct *ptr_sbr_tables,
    ixheaacd_misc_tables *ptr_common_tables);

#endif /* IXHEAACD_HEADERDECODE_H */
