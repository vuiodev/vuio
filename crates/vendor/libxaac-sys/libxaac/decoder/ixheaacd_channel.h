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
#ifndef IXHEAACD_CHANNEL_H
#define IXHEAACD_CHANNEL_H

enum {

  ID_SCE = 0,
  ID_CPE,
  ID_CCE,
  ID_LFE,
  ID_DSE,
  ID_PCE,
  ID_FIL,
  ID_END,
  ID_HDR,
  ID_NULL,
  ID_IIND_ICS,
  CRC_LEVEL_FIN,
  END_HDR,
};

#define LEFT 0
#define RIGHT 1

IA_ERRORCODE ixheaacd_channel_pair_process(
    ia_aac_dec_channel_info_struct *ptr_aac_dec_channel_info[CHANNELS], WORD32 num_ch,
    ia_aac_dec_tables_struct *ptr_aac_tables, WORD32 total_channels, WORD32 object_type,
    WORD32 aac_spect_data_resil_flag, WORD32 aac_sf_data_resil_flag, WORD32 *in_data,
    WORD32 *out_data, void *self_ptr);

VOID ixheaacd_map_ms_mask_pns(
    ia_aac_dec_channel_info_struct *ptr_aac_dec_channel_info[CHANNELS]);

VOID ixheaacd_read_fill_element(ia_bit_buf_struct *it_bit_buff,
                                ia_drc_dec_struct *drc_dummy,
                                ia_drc_dec_struct *pstr_drc_dec,
                                UWORD8 *mps_buffer, WORD32 *mps_header,
                                WORD32 *mps_bytes);

VOID ixheaacd_read_data_stream_element(ia_bit_buf_struct *it_bit_buff,
                                       WORD32 *byte_align_bits,
                                       ia_drc_dec_struct *drc_handle);

WORD16 *ixheaacd_getscalefactorbandoffsets(
    ia_ics_info_struct *ptr_ics_info, ia_aac_dec_tables_struct *ptr_aac_tables);
#endif /* #ifndef IXHEAACD_CHANNEL_H */
