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
#ifndef IXHEAACD_BLOCK_H
#define IXHEAACD_BLOCK_H

#define IQ_TABLE_SIZE_HALF 128

VOID ixheaacd_inverse_quantize(WORD32 *x_invquant, WORD no_band,
                               WORD32 *ixheaacd_pow_table_Q13,
                               WORD8 *scratch_in);

VOID ixheaacd_scale_factor_process_dec(WORD32 *x_invquant, WORD16 *scale_fact,
                                       WORD no_band, WORD8 *width,
                                       WORD32 *scale_tables_ptr,
                                       WORD32 total_channels,
                                       WORD32 object_type,
                                       WORD32 aac_sf_data_resil_flag);

VOID ixheaacd_scale_factor_process_armv7(WORD32 *x_invquant, WORD16 *scale_fact,
                                         WORD no_band, WORD8 *width,
                                         WORD32 *scale_tables_ptr,
                                         WORD32 total_channels,
                                         WORD32 object_type,
                                         WORD32 aac_sf_data_resil_flag);

VOID ixheaacd_scale_factor_process_armv8(WORD32 *x_invquant, WORD16 *scale_fact,
                                         WORD no_band, WORD8 *width,
                                         WORD32 *scale_tables_ptr,
                                         WORD32 total_channels,
                                         WORD32 object_type,
                                         WORD32 aac_sf_data_resil_flag);

VOID ixheaacd_right_shift_block(WORD32 *p_spectrum, WORD length,
                                WORD shift_val);

WORD ixheaacd_decode_huffman(ia_bit_buf_struct *it_bit_buff, WORD32 cb_no,
                             WORD32 *spec_coef, WORD16 *sfb_offset, WORD start,
                             WORD sfb, WORD group_len,
                             ia_aac_dec_tables_struct *ptr_aac_tables,
                             WORD32 maximum_bins_short);

WORD ixheaacd_huffman_dec_word2(ia_bit_buf_struct *it_bit_buff, WORD32 cb_no,
                                WORD32 width,
                                ia_aac_dec_tables_struct *ptr_aac_tables,
                                WORD32 *x_invquant, WORD8 *scratch_ptr);

VOID ixheaacd_read_scale_factor_data(
    ia_bit_buf_struct *it_bit_buff,
    ia_aac_dec_channel_info_struct *ptr_aac_dec_channel_info,
    ia_aac_dec_tables_struct *ptr_aac_tables, WORD32 object_type);

IA_ERRORCODE ixheaacd_read_spectral_data(
    ia_bit_buf_struct *it_bit_buff,
    ia_aac_dec_channel_info_struct *ptr_aac_dec_channel_info,
    ia_aac_dec_tables_struct *ptr_aac_tables, WORD32 total_channels,
    WORD32 frame_size, WORD32 object_type, WORD32 aac_spect_data_resil_flag,
    WORD32 aac_sf_data_resil_flag);

IA_ERRORCODE ixheaacd_read_section_data(
    ia_bit_buf_struct *it_bit_buff,
    ia_aac_dec_channel_info_struct *ptr_aac_dec_channel_info,
    WORD32 aac_spect_data_resil_flag, WORD32 aac_sect_data_resil_flag,
    ia_aac_dec_tables_struct *ptr_aac_tables);

VOID ixheaacd_over_lap_add1_dec(WORD32 *coef, WORD32 *prev, WORD32 *out,
                                const WORD16 *window, WORD16 q_shift,
                                WORD16 size, WORD16 ch_fac);

VOID ixheaacd_over_lap_add1_armv7(WORD32 *coef, WORD32 *prev, WORD32 *out,
                                  const WORD16 *window, WORD16 q_shift,
                                  WORD16 size, WORD16 ch_fac);

VOID ixheaacd_over_lap_add1_armv8(WORD32 *coef, WORD32 *prev, WORD32 *out,
                                  const WORD16 *window, WORD16 q_shift,
                                  WORD16 size, WORD16 ch_fac);

VOID ixheaacd_over_lap_add2_dec(WORD32 *coef, WORD32 *prev, WORD32 *out,
                                const WORD16 *window, WORD16 q_shift,
                                WORD16 size, WORD16 ch_fac);

VOID ixheaacd_over_lap_add2_armv7(WORD32 *coef, WORD32 *prev, WORD32 *out,
                                  const WORD16 *window, WORD16 q_shift,
                                  WORD16 size, WORD16 ch_fac);

VOID ixheaacd_over_lap_add2_armv8(WORD32 *coef, WORD32 *prev, WORD32 *out,
                                  const WORD16 *window, WORD16 q_shift,
                                  WORD16 size, WORD16 ch_fac);

VOID ixheaacd_set_corr_info(
    ia_aac_dec_channel_info_struct *ptr_aac_dec_channel_info, WORD16 pns_band);

VOID ixheaacd_gen_rand_vec(WORD32 scale, WORD shift, WORD32 *spec,
                           WORD32 sfb_width, WORD32 *random_vec);

VOID ixheaacd_pns_process(ia_aac_dec_channel_info_struct *ptr_aac_dec_channel_info[CHANNELS],
                          WORD32 channel, ia_aac_dec_tables_struct *ptr_aac_tables);

VOID ixheaacd_spec_to_overlapbuf_dec(WORD32 *ptr_overlap_buf,
                                     WORD32 *ptr_spec_coeff, WORD32 q_shift,
                                     WORD32 size);

VOID ixheaacd_spec_to_overlapbuf_armv7(WORD32 *ptr_overlap_buf,
                                       WORD32 *ptr_spec_coeff, WORD32 q_shift,
                                       WORD32 size);

VOID ixheaacd_overlap_buf_out_dec(WORD32 *out_samples, WORD32 *ptr_overlap_buf,
                                  WORD32 size, const WORD16 ch_fac);

VOID ixheaacd_overlap_buf_out_armv7(WORD32 *out_samples,
                                    WORD32 *ptr_overlap_buf, WORD32 size,
                                    const WORD16 ch_fac);

WORD32 ixheaacd_inv_quant(WORD32 *x_quant, WORD32 *ixheaacd_pow_table_Q13);

VOID ixheaacd_imdct_process(ia_aac_dec_overlap_info *ptr_aac_dec_overlap_info,
                            WORD32 *ptr_spec_coeff,
                            ia_ics_info_struct *ptr_ics_info, VOID *out_samples,
                            const WORD16 ch_fac, WORD32 *scratch,
                            ia_aac_dec_tables_struct *ptr_aac_tables,
                            WORD32 object_type, WORD32 ld_mps_present,
                            WORD slot_element);

VOID ixheaacd_neg_shift_spec_dec(WORD32 *coef, WORD32 *out, WORD16 q_shift,
                                 WORD16 ch_fac);

VOID ixheaacd_nolap_dec(WORD32 *coef, WORD32 *out, WORD16 q_shift,
                        WORD16 ch_fac, WORD16 size_01);

VOID ixheaacd_neg_shift_spec_armv7(WORD32 *coef, WORD32 *out, WORD16 q_shift,
                                   WORD16 ch_fac);

VOID ixheaacd_neg_shift_spec_armv8(WORD32 *coef, WORD32 *out, WORD16 q_shift,
                                   WORD16 ch_fac);

VOID ixheaacd_nolap1_32(WORD32 *coef, WORD32 *out, WORD16 cu_scale,
                        WORD16 stride, WORD16 size_01);

VOID ixheaacd_overlap_out_copy_dec(WORD32 *out_samples, WORD32 *ptr_overlap_buf,
                                   WORD32 *ptr_overlap_buf1,
                                   const WORD16 ch_fac, WORD16 size_01);

VOID ixheaacd_overlap_out_copy_armv7(WORD32 *out_samples,
                                     WORD32 *ptr_overlap_buf,
                                     WORD32 *ptr_overlap_buf1,
                                     const WORD16 ch_fac);

VOID ixheaacd_long_short_win_seq(WORD32 *current, WORD32 *prev, WORD32 *out,
                                 const WORD16 *short_window,
                                 const WORD16 *short_window_prev,
                                 const WORD16 *long_window_prev, WORD16 q_shift,
                                 WORD16 ch_fac, WORD16 size_01);

WORD32 ixheaacd_cnt_leading_ones(WORD32 a);

VOID ixheaacd_huffman_decode(WORD32 it_bit_buff, WORD16 *huff_index,
                             WORD16 *len, const UWORD16 *input_table,
                             const UWORD32 *idx_table);

VOID ixheaacd_eld_dec_windowing(WORD32 *ptr_spect_coeff, const WORD16 *ptr_win,
                                WORD32 framesize, WORD16 q_shift,
                                WORD32 *ptr_overlap_buf, const WORD16 stride,
                                VOID *out_samples, WORD slot_element);

VOID ixheaacd_eld_dec_windowing_32bit(WORD32 *ptr_spect_coeff,
                                      const WORD16 *ptr_win, WORD32 framesize,
                                      WORD16 q_shift, WORD32 *ptr_overlap_buf,
                                      const WORD16 stride, WORD32 *out_samples);

VOID ixheaacd_process_single_scf(WORD32 scale_factor, WORD32 *x_invquant,
                                 WORD32 width, WORD32 *ptr_scale_table,
                                 WORD32 total_channels, WORD32 object_type,
                                 WORD32 aac_sf_data_resil_flag);

VOID ixheaacd_lap1_512_480(WORD32 *coef, WORD32 *prev, VOID *out_tmp,
                           const WORD16 *window, WORD16 q_shift, WORD16 size,
                           WORD16 stride, WORD slot_element);

VOID ixheaacd_dec_copy_outsample(WORD32 *out_samples, WORD32 *p_overlap_buffer,
                                 WORD32 size, WORD16 stride);

#endif /* #ifndef IXHEAACD_BLOCK_H */
