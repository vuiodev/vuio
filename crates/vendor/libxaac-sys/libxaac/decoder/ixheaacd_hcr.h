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

#ifndef IXHEAACD_HCR_H
#define IXHEAACD_HCR_H
VOID ixheaacd_huff_code_reorder_tbl_init(ia_hcr_info_struct *ptr_hcr_info);
UWORD32 ixheaacd_huff_code_reorder_init(
    ia_hcr_info_struct* ptr_hcr_info,
    ia_aac_dec_channel_info_struct *ptr_aac_dec_channel_info,
    ia_aac_dec_tables_struct* ptr_aac_tables, ia_bit_buf_struct* itt_bit_buff);
UWORD32 ixheaacd_hcr_decoder(
    ia_hcr_info_struct* ptr_hcr_info,
    ia_aac_dec_channel_info_struct *ptr_aac_dec_channel_info,
    ia_aac_dec_tables_struct* ptr_aac_tables, ia_bit_buf_struct* itt_bit_buff);
VOID ixheaacd_huff_mute_erroneous_lines(ia_hcr_info_struct *ptr_hcr_info);

void ixheaacd_lt_prediction(
    ia_aac_dec_channel_info_struct* ptr_aac_dec_channel_info, ltp_info* ltp,
    WORD32* spec, ia_aac_dec_tables_struct* aac_tables_ptr,
    UWORD16 win_shape_prev, UWORD32 sr_index, UWORD32 object_type,
    UWORD32 frame_len, WORD32 *in_data, WORD32 *out_data);

WORD32 ixheaacd_ltp_data(WORD32 object_type, ia_ics_info_struct* ics,
                         ltp_info* ltp, ia_bit_buf_struct *it_bit_buf,
                         WORD32 frame_len);

#endif
