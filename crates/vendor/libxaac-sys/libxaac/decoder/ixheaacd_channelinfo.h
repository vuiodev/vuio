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
#ifndef IXHEAACD_CHANNELINFO_H
#define IXHEAACD_CHANNELINFO_H

#include "ixheaacd_lt_predict.h"

#define MAX_SFB_SHORT 16
#define MAX_QUANTIZED_VALUE 8191

#define OVERLAP_BUFFER_SIZE 512

#define JOINT_STEREO_MAX_GROUPS 8
#define JOINT_STEREO_MAX_BANDS 64

typedef struct {
  WORD16 window_shape;
  WORD16 window_sequence;
  WORD16 max_sfb;
  WORD16 num_swb_window;
  WORD16 sampling_rate_index;
  WORD16 num_window_groups;
  WORD8 window_group_length[8];
  WORD16 frame_length;
  WORD32 frame_size;
  WORD16 predictor_data_present;
  ltp_info ltp;
  ltp_info ltp2;
  WORD8 qshift_adj;
} ia_ics_info_struct;

typedef struct {
  WORD32 aac_sect_data_resil_flag;
  WORD32 aac_sf_data_resil_flag;
  WORD32 aac_spect_data_resil_flag;
  WORD32 ep_config;
} ia_aac_err_config_struct;

typedef struct {
  WORD32 ld_sbr_flag_present;
  WORD32 ld_sbr_samp_rate;
  WORD32 ld_sbr_crc_flag;
  WORD32 ldSbrHeaderPresent;

  WORD32 aac_sect_data_resil_flag;
  WORD32 aac_sf_data_resil_flag;
  WORD32 aac_spect_data_resil_flag;
  WORD32 ep_config;

} ia_eld_specific_config_struct;

typedef struct {
  UWORD8 ms_used[JOINT_STEREO_MAX_GROUPS][JOINT_STEREO_MAX_BANDS];
} ia_stereo_info_struct;

typedef struct {
  WORD16 start_band;
  WORD16 stop_band;
  WORD8 direction;
  WORD8 resolution;
  WORD8 order;
  WORD8 coef[MAX_ORDER];
} ia_filter_info_struct;

typedef struct {
  FLAG tns_data_present;
  WORD8 n_filt[MAX_WINDOWS];
  ia_filter_info_struct str_filter[MAX_WINDOWS][MAX_FILTERS];
} ia_tns_info_aac_struct;

typedef struct {
  const WORD16 *ptr_long_window[2];
  const WORD16 *ptr_short_window[2];
  WORD16 window_shape;
  WORD16 window_sequence;
  WORD32 *ptr_overlap_buf;

  WORD16 rvlc_prev_sf[128];
  WORD16 rvlc_prev_cb[128];
  WORD8 rvlc_prev_blk_type;
  WORD8 rvlc_prev_sf_ok;

} ia_aac_dec_overlap_info;

typedef struct {
  WORD32 sf_concealment;
  WORD32 rev_global_gain;
  WORD16 rvlc_sf_len;
  WORD32 dpcm_noise_nrg;
  WORD32 sf_esc_present;
  WORD16 rvlc_esc_len;
  WORD32 dpcm_noise_last_pos;

  WORD32 dpcm_is_last_pos;

  WORD16 rvlc_sf_fwd_len;
  WORD16 rvlc_sf_bwd_len;

  WORD16 *ptr_rvl_bit_cnt;
  UWORD16 *ptr_rvl_bit_str_idx;

  WORD16 num_wind_grps;
  WORD16 max_sfb_transmitted;
  UWORD8 first_noise_group;
  UWORD8 first_noise_band;
  UWORD8 direction;

  UWORD16 rvl_fwd_bit_str_idx;
  UWORD16 rvl_bwd_bit_str_idx;
  UWORD16 esc_bit_str_idx;

  const UWORD32 *ptr_huff_tree_rvl_cw;
  const UWORD32 *ptr_huff_tree_rvl_esc;

  UWORD8 num_fwd_esc_words_decoded;
  UWORD8 num_bwd_esc_words_decoded;
  UWORD8 num_esc_words_decoded;

  WORD8 noise_used;
  WORD8 intensity_used;
  WORD8 sf_used;

  WORD16 firt_scale_fac;
  WORD16 last_scale_fac;
  WORD16 first_nrg;
  WORD16 last_nrg;
  WORD16 is_first;
  WORD16 is_last;

  UWORD32 rvlc_err_log;
  WORD16 conceal_min;
  WORD16 conceal_max;
  WORD16 conceal_min_esc;
  WORD16 conceal_max_esc;
} ia_rvlc_info_struct;

#define LINES_PER_UNIT 4

#define MAX_SFB_HCR (((1024 / 8) / LINES_PER_UNIT) * 8)
#define NUMBER_OF_UNIT_GROUPS (LINES_PER_UNIT * 8)
#define LINES_PER_UNIT_GROUP (1024 / NUMBER_OF_UNIT_GROUPS)

#define FROM_LEFT_TO_RIGHT 0
#define FROM_RIGHT_TO_LEFT 1

#define MAX_CB_PAIRS 23
#define MAX_HCR_SETS 14

#define ESCAPE_VALUE 16
#define POSITION_OF_FLAG_A 21
#define POSITION_OF_FLAG_B 20

#define MAX_CB 32

#define MAX_CB_CHECK 32
#define WORD_BITS 32

#define THIRTYTWO_LOG_DIV_TWO_LOG 5
#define EIGHT_LOG_DIV_TWO_LOG 3
#define FOUR_LOG_DIV_TWO_LOG 2

#define CPE_TOP_LENGTH 12288
#define SCE_TOP_LENGTH 6144
#define LEN_OF_LONGEST_CW_TOP_LENGTH 49
#define Q_VALUE_INVALID 8192
#define NODE_MASK 0x400

#define ERROR_POS 0x00000001
#define HCR_FATAL_PCW_ERROR_MASK 0x100E01FC

typedef enum { PCW, PCW_SIGN, PCW_ESC_SIGN } ia_pcw_type_struct;

typedef struct {
  UWORD32 err_log;
  WORD32 *ptr_quant_spec_coeff_base;
  WORD32 quant_spec_coeff_idx;
  WORD16 reordered_spec_data_len;
  WORD16 num_sect;
  WORD16 *ptr_num_line_in_sect;
  UWORD16 bit_str_idx;
  WORD8 longest_cw_len;
  UWORD8 *ptr_cb;
} ia_huff_code_reorder_io_struct;

typedef struct {
  const UWORD8 *ptr_min_cb_pair_tbl;
  const UWORD8 *ptr_max_cb_pair_tbl;
} ia_huff_code_reorder_cb_pairs_struct;

typedef struct {
  const UWORD16 *ptr_lav_tbl;
  const UWORD8 *ptr_max_cw_len_tbl;
  const UWORD8 *ptr_cb_dimension_tbl;
  const UWORD8 *ptr_cb_dim_shift_tbl;
  const UWORD8 *ptr_cb_sign_tbl;
  const UWORD8 *ptr_cb_priority;
} ia_huff_code_reorder_tbl_struct;

typedef struct {
  WORD32 num_segment;
  UWORD32 segment_offset;
  WORD32 arr_temp_values[1024];
  UWORD16 arr_seg_start_l[1024 >> 1];
  UWORD16 arr_seg_start_r[1024 >> 1];
  WORD8 p_remaining_bits_in_seg[1024 >> 1];
  WORD32 code[512];
  WORD32 code_extra[512];
  WORD8 p_num_bits[512];
  UWORD8 read_direction;
  WORD32 is_decoded[512];
} ia_huff_code_reord_seg_info_struct;

typedef struct {
  UWORD32 num_code_word;
  UWORD32 current_codeword;
  UWORD32 num_sorted_section;
  UWORD16 ptr_num_cw_in_sect[MAX_SFB_HCR];
  UWORD16 ptr_num_sorted_cw_in_sect[MAX_SFB_HCR];
  UWORD16 ptr_num_ext_sorted_cw_in_sect[MAX_SFB_HCR + MAX_HCR_SETS];
  WORD32 num_ext_sorted_cw_in_sect_idx;
  UWORD16 ptr_num_ext_sorted_sect_in_sets[MAX_HCR_SETS];
  WORD32 num_ext_sorted_sect_in_sets_idx;
  UWORD16 ptr_reorder_offset[MAX_SFB_HCR];
  UWORD8 ptr_sorted_cb[MAX_SFB_HCR];

  UWORD8 ptr_ext_sorted_cw[MAX_SFB_HCR + MAX_HCR_SETS];
  WORD32 ext_sorted_cw_idx;
  UWORD8 ptr_ext_sorted_sect_max_cb_len[MAX_SFB_HCR + MAX_HCR_SETS];
  WORD32 ext_sorted_sect_max_cb_len_idx;
  UWORD8 ptr_cb_switch[MAX_SFB_HCR];
} ia_huff_code_reord_sect_info_struct;

typedef UWORD32 (*ixheaacd_ptr_state_func)(ia_bit_buf_struct *, pVOID);

typedef struct {
  WORD32 *ptr_result_base;
  UWORD16 res_ptr_idx[1024 >> 2];
  UWORD32 cw_offset;
  UWORD8 ptr_cb[1024 >> 2];
} ia_hcr_non_pcw_sideinfo_struct;

typedef struct {
  ia_huff_code_reorder_io_struct str_dec_io;
  ia_huff_code_reorder_cb_pairs_struct codebook_pairs;
  ia_huff_code_reorder_tbl_struct table_info;
  ia_huff_code_reord_seg_info_struct str_segment_info;
  ia_huff_code_reord_sect_info_struct sect_info;
  ia_hcr_non_pcw_sideinfo_struct str_non_pcw_side_info;

  WORD32 global_hcr_type;
} ia_hcr_info_struct;

typedef struct {
  WORD16 scale_factor[MAX_WINDOWS * MAX_SFB_SHORT];
  WORD8 code_book[MAX_WINDOWS * MAX_SFB_SHORT];
} ia_aac_sfb_code_book_struct;

typedef struct {
  ia_stereo_info_struct str_stereo_info;
  ia_pns_correlation_info_struct str_pns_corr_info;
} ia_pns_stereo_data_struct;

typedef struct {
  WORD16 win_shape;
  WORD16 win_seq;

  WORD32 *ptr_overlap_buf;

} ia_aac_dec_ola_data;

typedef struct {
  const WORD16 *ptr_long_window[2];
  const WORD16 *ptr_short_window[2];

  ia_aac_dec_ola_data overlap_add_data;

  WORD16 *ltp_buf;
  UWORD16 ltp_lag_1;
  UWORD16 ltp_lag_2;
  ia_ec_state_str str_ec_state;
} ia_aac_dec_channel_info;

typedef struct {
  WORD16 *ptr_scale_factor;
  WORD8 *ptr_code_book;
  WORD32 *ptr_spec_coeff;
  ia_stereo_info_struct *pstr_stereo_info;
  ia_pns_correlation_info_struct *pstr_pns_corr_info;
  ia_pns_rand_vec_struct *pstr_pns_rand_vec_data;
  ia_ics_info_struct str_ics_info;
  ia_tns_info_aac_struct str_tns_info;
  ia_pulse_info_struct str_pulse_info;
  ia_pns_info_struct str_pns_info;
  WORD16 common_window;
  WORD16 element_instance_tag;
  WORD16 global_gain;
  WORD32 *scratch_buf_ptr;
  WORD32 *pulse_scratch;
  ia_rvlc_info_struct ptr_rvlc_info;
  ia_hcr_info_struct str_hcr_info;
  WORD16 reorder_spect_data_len;
  WORD8 longest_cw_len;
  WORD16 rvlc_scf_esc_arr[128];
  WORD16 rvlc_scf_fwd_arr[128];
  WORD16 rvlc_scf_bwd_arr[128];
  WORD8 rvlc_intensity_used;
  WORD16 num_line_in_sec4_hcr_arr[32 * 8];
  UWORD8 cb4_hcr_arr[32 * 8];
  WORD32 number_sect;
  WORD32 granule_len;
  WORD16 rvlc_curr_sf_flag;
  WORD16 *ltp_buf;
  UWORD16 ltp_lag;
} ia_aac_dec_channel_info_struct;
WORD16 ixheaacd_ics_read(ia_bit_buf_struct *it_bit_buff,
                         ia_ics_info_struct *ptr_ics_info,
                         WORD8 num_swb_window[2], WORD32 object_type,
                         WORD32 common_window, WORD32 frame_size);

IA_ERRORCODE ixheaacd_ltp_decode(ia_bit_buf_struct *it_bit_buff,
                                 ia_ics_info_struct *ptr_ics_info,
                                 WORD32 object_type, WORD32 frame_size,
                                 WORD32 ch);

#endif /* #ifndef IXHEAACD_CHANNELINFO_H */
