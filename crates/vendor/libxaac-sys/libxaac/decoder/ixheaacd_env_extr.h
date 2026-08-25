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
#ifndef IXHEAACD_ENV_EXTR_H
#define IXHEAACD_ENV_EXTR_H
#include "ixheaac_error_standards.h"
#define ENV_EXP_FRACT 0

#define EXP_BITS 6

#define MASK_M (((1 << (SHORT_BITS - EXP_BITS)) - 1) << EXP_BITS)
#define MASK_FOR_EXP ((1 << EXP_BITS) - 1)

#define SIGN_EXT (((WORD8)-1) ^ MASK_FOR_EXP)
#define ROUNDING (1 << (EXP_BITS - 1))
#define NRG_EXP_OFFSET 16
#define NOISE_EXP_OFFSET 38
#define MAX_QMF_SUB_BANDS 64

typedef const UWORD16 *ia_huffman_data_type;

#define COUPLING_OFF 0
#define COUPLING_LEVEL 1
#define COUPLING_BAL 2

#define MAX_INVF_BANDS MAX_NOISE_COEFFS

typedef struct {
  WORD16 sfb_nrg_prev[MAX_FREQ_COEFFS];
  WORD16 prev_noise_level[MAX_NOISE_COEFFS];
  WORD16 amp_res;
  WORD16 end_position;
  WORD32 max_qmf_subband_aac;
  WORD32 coupling_mode;
  WORD32 sbr_invf_mode[MAX_NUM_NOISE_VALUES];
} ia_sbr_prev_frame_data_struct;

typedef struct {
  WORD16 num_env_sfac;
  ia_frame_info_struct str_frame_info_details;
  WORD16 del_cod_dir_arr[MAX_ENVELOPES];
  WORD16 del_cod_dir_noise_arr[MAX_NOISE_ENVELOPES];
  WORD32 sbr_invf_mode[MAX_NUM_NOISE_VALUES];
  WORD32 coupling_mode;
  WORD16 amp_res;
  WORD32 max_qmf_subband_aac;
  FLAG add_harmonics[MAX_FREQ_COEFFS];
  WORD16 int_env_sf_arr[MAX_NUM_ENVELOPE_VALUES];
  WORD16 int_noise_floor[MAX_NUM_NOISE_VALUES];
  WORD32 num_noise_sfac;
  ia_frame_info_struct str_pvc_frame_info;
  WORD32 env_short_flag_prev;
  ia_sbr_header_data_struct *pstr_sbr_header;
  WORD32 num_time_slots;
  WORD32 rate;
  WORD32 sbr_patching_mode;
  WORD32 prev_sbr_patching_mode;
  WORD32 over_sampling_flag;
  WORD32 pitch_in_bins;
  WORD32 pvc_mode;
  WORD32 cov_count;
  WORD32 sbr_invf_mode_prev[MAX_NUM_NOISE_VALUES];
  FLOAT32 flt_env_sf_arr[MAX_NUM_ENVELOPE_VALUES];
  FLOAT32 flt_noise_floor[MAX_NUM_NOISE_VALUES];
  FLOAT32 sfb_nrg_prev[MAX_FREQ_COEFFS];
  FLOAT32 prev_noise_level[MAX_NUM_NOISE_VALUES];
  WORD32 inter_temp_shape_mode[MAX_ENVELOPES];
  WORD32 var_len;
  WORD32 bs_sin_pos_present;
  WORD32 sine_position;
  WORD32 sin_start_for_next_top;
  WORD32 sin_len_for_next_top;
  WORD32 sin_start_for_cur_top;
  WORD32 sin_len_for_cur_top;
  WORD32 var_len_id_prev;
  ia_frame_info_struct str_frame_info_prev;
  FLOAT32 bw_array_prev[MAX_NUM_PATCHES];
  struct ixheaacd_lpp_trans_patch patch_param;
  WORD32 harm_index;
  WORD32 phase_index;
  WORD8 harm_flag_prev[64];
  FLOAT32 e_gain[5][64];
  FLOAT32 noise_buf[5][64];
  WORD32 lim_table[4][12 + 1];
  WORD32 gate_mode[4];
  WORD8 harm_flag_varlen_prev[64];
  WORD8 harm_flag_varlen[64];
  FLOAT32 qmapped_pvc[MAX_QMF_SUB_BANDS][MAX_FREQ_COEFFS_SBR];
  FLOAT32 env_tmp[MAX_QMF_SUB_BANDS][MAX_FREQ_COEFFS_SBR];
  FLOAT32 noise_level_pvc[MAX_QMF_SUB_BANDS][MAX_FREQ_COEFFS_SBR];
  FLOAT32 nrg_est_pvc[MAX_QMF_SUB_BANDS][MAX_FREQ_COEFFS_SBR];
  FLOAT32 nrg_ref_pvc[MAX_QMF_SUB_BANDS][MAX_FREQ_COEFFS_SBR];
  FLOAT32 nrg_gain_pvc[MAX_QMF_SUB_BANDS][MAX_FREQ_COEFFS_SBR];
  FLOAT32 nrg_tone_pvc[MAX_QMF_SUB_BANDS][MAX_FREQ_COEFFS_SBR];
  WORD32 stereo_config_idx;
  FLAG reset_flag;
  FLAG mps_sbr_flag;
  FLAG usac_independency_flag;
  FLAG inter_tes_flag;
  FLAG sbr_mode;
  FLAG prev_sbr_mode;
  WORD32 eld_sbr_flag;

} ia_sbr_frame_info_data_struct;

IA_ERRORCODE ixheaacd_sbr_read_sce(ia_sbr_header_data_struct *ptr_header_data,
                                   ia_sbr_frame_info_data_struct *ptr_frame_data,
                                   ia_ps_dec_struct *ptr_ps_dec, ia_bit_buf_struct *it_bit_buff,
                                   ia_sbr_tables_struct *ptr_sbr_tables, WORD audio_object_type,
                                   WORD32 ec_flag);

IA_ERRORCODE ixheaacd_sbr_read_cpe(
    ia_sbr_header_data_struct *ptr_header_data,
    ia_sbr_frame_info_data_struct **ptr_frame_data,
    ia_bit_buf_struct *itt_bit_buf, ia_sbr_tables_struct *ptr_sbr_tables,
    WORD audio_object_type);

WORD32 ixheaacd_sbr_read_header_data(
    ia_sbr_header_data_struct *ptr_sbr_header, ia_bit_buf_struct *it_bit_buf,
    FLAG stereo_flag, ia_sbr_header_data_struct *ptr_sbr_dflt_header);

WORD32 ixheaacd_ssc_huff_dec(ia_huffman_data_type h,
                             ia_bit_buf_struct *it_bit_buff);

IA_ERRORCODE ixheaacd_extract_frame_info_ld(
    ia_bit_buf_struct *it_bit_buff,
    ia_sbr_frame_info_data_struct *h_frame_data);

WORD32 ixheaacd_pvc_time_freq_grid_info(
    ia_bit_buf_struct *it_bit_buff,
    ia_sbr_frame_info_data_struct *ptr_frame_data);

WORD16 ixheaacd_sbr_time_freq_grid_info(
    ia_bit_buf_struct *it_bit_buff,
    ia_sbr_frame_info_data_struct *ptr_frame_data,
    ia_env_extr_tables_struct *env_extr_tables_ptr, WORD16 number_of_timeslots);

WORD16 ixheaacd_read_sbr_env_data(
    ia_sbr_header_data_struct *ptr_header_data,
    ia_sbr_frame_info_data_struct *ptr_frame_data,
    ia_bit_buf_struct *it_bit_buff,
    ia_env_extr_tables_struct *env_extr_tables_ptr, WORD audio_object_type);

VOID ixheaacd_sbr_env_dtdf_data(ia_sbr_frame_info_data_struct *ptr_frame_data,
                                ia_bit_buf_struct *it_bit_buff,
                                WORD32 usac_flag);

VOID ixheaacd_read_sbr_noise_floor_data(
    ia_sbr_header_data_struct *ptr_header_data,
    ia_sbr_frame_info_data_struct *ptr_frame_data,
    ia_bit_buf_struct *it_bit_buff,
    ia_env_extr_tables_struct *env_extr_tables_ptr);

VOID ixheaacd_huffman_decode(WORD32 it_bit_buff, WORD16 *h_index, WORD16 *len,
                             const UWORD16 *input_table,
                             const UWORD32 *idx_table);

IA_ERRORCODE ixheaacd_createlimiterbands(WORD32 lim_table[4][12 + 1], WORD32 gate_mode[4],
                                         WORD16 *freq_band_tbl, WORD32 ixheaacd_num_bands,
                                         WORD32 x_over_qmf[MAX_NUM_PATCHES],
                                         WORD32 b_patching_mode, WORD32 upsamp_4_flag,
                                         struct ixheaacd_lpp_trans_patch *patch_param,
                                         WORD32 ec_flag);

WORD32 ixheaacd_apply_inter_tes(FLOAT32 *qmf_real1, FLOAT32 *qmf_imag1,
                              FLOAT32 *qmf_real, FLOAT32 *qmf_imag,
                              WORD32 num_sample, WORD32 sub_band_start,
                              WORD32 num_subband, WORD32 gamma_idx);

#endif
