/******************************************************************************
 *
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
#ifndef IXHEAACD_MPS_RES_BLOCK_H
#define IXHEAACD_MPS_RES_BLOCK_H

#define IQ_TABLE_SIZE_HALF 128

VOID ixheaacd_res_inverse_quant_lb(WORD32 *x_invquant, WORD t_bands, WORD32 *pow_table_q17,
                                   WORD8 *pulse_data);

VOID ixheaacd_res_apply_scfs(WORD32 *x_invquant, WORD16 *sc_factor, WORD t_bands, WORD8 *offset,
                             WORD32 *scale_tables_ptr);

WORD ixheaacd_res_c_block_decode_huff_word_all(
    ia_bit_buf_struct *it_bit_buf, WORD32 code_no, WORD32 *qp, WORD16 *offsets, WORD start_band,
    WORD endBand, WORD group_no, ia_mps_dec_residual_aac_tables_struct *aac_tables_ptr,
    WORD32 maximum_bins_short);

WORD ixheaacd_res_c_block_decode_huff_word_all_lb(
    ia_bit_buf_struct *it_bit_buf, WORD32 code_no, WORD32 len,
    ia_mps_dec_residual_aac_tables_struct *aac_tables_ptr, WORD32 *x_invquant, WORD8 *p_pul_arr);

VOID ixheaacd_res_c_block_read_scf_data(
    ia_bit_buf_struct *it_bit_buf,
    ia_mps_dec_residual_channel_info_struct *p_aac_decoder_channel_info, WORD16 global_gain,
    ia_mps_dec_residual_aac_tables_struct *aac_tables_ptr);

WORD16 ixheaacd_res_c_block_read_spec_data(
    ia_bit_buf_struct *it_bit_buf,
    ia_mps_dec_residual_channel_info_struct *p_aac_decoder_channel_info,
    ia_mps_dec_residual_aac_tables_struct *aac_tables_ptr);

WORD16 ixheaacd_c_block_read_section_data(
    ia_bit_buf_struct *it_bit_buf,
    ia_mps_dec_residual_channel_info_struct *p_aac_decoder_channel_info);

WORD32 ixheaacd_res_inv_quant(WORD32 *x_quant, WORD32 *pow_table_q17);

#endif /* IXHEAACD_MPS_RES_BLOCK_H */
