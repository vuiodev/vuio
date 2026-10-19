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
#ifndef IXHEAACD_AACDEC_H
#define IXHEAACD_AACDEC_H

#define AAC_DEC_OK IA_XHEAAC_DEC_API_NONFATAL_NO_ERROR

#define FRAME_SIZE 1024
#define FRAME_SIZE_SMALL 960

#define ADTS_BSFORMAT 2
#define LOAS_BSFORMAT 3

typedef struct ia_aac_decoder_struct {
  FLAG frame_status;
  WORD32 byte_align_bits;
  ia_aac_dec_sbr_bitstream_struct *pstr_sbr_bitstream;
  ia_aac_dec_channel_info_struct *pstr_aac_dec_ch_info[CHANNELS];

  ia_aac_dec_channel_info *ptr_aac_dec_static_channel_info[CHANNELS];

  ia_aac_dec_overlap_info *pstr_aac_dec_overlap_info[CHANNELS];
  ia_pns_rand_vec_struct *pstr_pns_rand_vec_data;
  void *p_ind_channel_info;
  WORD32 block_number;
  WORD16 sampling_rate_index;
  WORD32 sampling_rate;
  WORD32 samples_per_frame;
  WORD16 channels;
  WORD8 num_swb_window[2];
  ia_aac_dec_tables_struct *pstr_aac_tables;
  ixheaacd_misc_tables *pstr_common_tables;
  WORD32 is_first;
  WORD32 conceal_count;
  WORD16 sbr_num_elements;
} ia_aac_decoder_struct;

struct ia_aac_persistent_struct {
  WORD32 *overlap_buffer;
  ia_aac_dec_overlap_info str_aac_dec_overlap_info[CHANNELS];
  ia_pns_rand_vec_struct str_pns_rand_vec_data;
  struct ia_aac_decoder_struct str_aac_decoder;
  WORD8 *sbr_payload_buffer;

  ia_aac_dec_channel_info *ptr_aac_dec_static_channel_info[CHANNELS];
  WORD16 *ltp_buf[CHANNELS];
  WORD8 *prev_sbr_payload_buffer;
};

typedef struct {
  VOID *base_scr_8k;
  VOID *extra_scr_4k[4];

  WORD32 *in_data;
  WORD32 *out_data;

} ia_aac_dec_scratch_struct;

WORD16 ixheaacd_individual_ch_stream(
    ia_bit_buf_struct *it_bit_buff, ia_aac_decoder_struct *aac_dec_handle,
    WORD32 num_ch, WORD32 frame_size, WORD32 total_channels, WORD32 object_type,
    ia_eld_specific_config_struct eld_specific_config, WORD32 ele_type);

VOID ixheaacd_huff_tables_create(ia_aac_dec_tables_struct *);

WORD32 ixheaacd_set_aac_persistent_buffers(VOID *aac_persistent_mem_v,
                                           WORD32 channels);

VOID ixheaacd_huffman_decode(WORD32 it_bit_buff, WORD16 *h_index, WORD16 *len,
                             const UWORD16 *input_table,
                             const UWORD32 *idx_table);

#endif /* #ifndef IXHEAACD_AACDEC_H */
