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
#ifndef IXHEAACD_MPS_STRUCT_DEF_H
#define IXHEAACD_MPS_STRUCT_DEF_H

typedef struct {
  const WORD32 *p_filter_ana;
  const WORD32 *ref_co_eff_ptr_l;
  const WORD32 *ref_co_eff_ptr_r;
  WORD32 offset_l;
  WORD32 offset_r;
  WORD32 *qmf_states_buffer;
  WORD16 flag;
  WORD16 offset;
  WORD32 *qmf_states_curr_pos;
} ia_mps_dec_qmf_ana_filter_bank;

typedef struct {
  const WORD32 *p_filter_syn;
  WORD32 *sbr_qmf_states_synthesis;
} ia_mps_dec_qmf_syn_filter_bank;

typedef struct {
  WORD32 ia_mps_enc_qmf_64_640[650];
  WORD16 sbr_alt_sin_twiddle[33];
  WORD16 sbr_cos_twiddle[32];
  WORD16 sbr_sin_twiddle[32];
  WORD16 fft_c[4];
  WORD16 ia_qmf_anl_addt_cos[32];
  WORD16 ia_qmf_anl_addt_sin[32];
} ia_mps_dec_qmf_tables_struct;

typedef struct { WORD32 sqrt_tab[513]; } ia_mps_dec_common_tables_struct;

typedef struct {
  WORD16 p8_13[19];
  WORD16 p2_6[6];
  WORD32 sine_array[2048];
  WORD32 cosine_array[2048];
} ia_mps_dec_hybrid_tables_struct;

typedef struct {
  WORD32 row_2_channel_stp[7][8];
  WORD32 row_2_channel_ges[7][8];
  WORD32 row_2_residual[7][8];
} ia_mps_dec_index_tables_struct;

typedef struct {
  ia_mps_dec_index_tables_struct idx_table;
  WORD32 hybrid_2_param_28[71];
  WORD32 r1_matrix_l[31];
  WORD32 ten_cld_by_10[31];
  WORD32 w00_cld2_15[31];
  WORD32 table_kappa[8];
  WORD32 dec_pow[31];
  WORD32 cld_tab_1[31];
  WORD32 cld_tab_2[31];
  WORD32 cld_tab_3[31];
  WORD32 reciprocal[576];
  WORD32 c_l_table[31];
  WORD32 cos_table[16][31];
  WORD32 sin_table[8][31];
} ia_mps_dec_m1_m2_tables_struct;

typedef struct {
  WORD32 rev_delay[4][10];
  WORD32 rev_split_freq_0[4];
  WORD32 rev_split_freq_1[4];
  WORD32 rev_split_freq_2[4];
} ia_mps_dec_rev_tables_struct;

typedef struct {
  ia_mps_dec_rev_tables_struct rev_table;
  WORD32 lattice_coeff_0[10][20];
  WORD32 lattice_coeff_1[10][15];
  WORD32 lattice_coeff_2[10][6];
  WORD32 lattice_coeff_3[10][3];
  WORD32 den_coef_0[10][21];
  WORD32 den_coef_1[10][16];
  WORD32 den_coef_2[10][7];
  WORD32 den_coef_3[10][4];
  WORD32 lattice_delta_phi[10][20];
} ia_mps_dec_decorr_tables_struct;

typedef struct {
  WORD32 bp[25];
  WORD32 bpxgf[25];
  WORD32 bp2xgf2[25];
  WORD32 ia_mps_dec_qmf_64_640[325];
  WORD32 time_out_idx_5xxx[6];
  WORD32 time_out_idx_7xxx[8];
} ia_mps_dec_tp_process_tables_struct;

typedef struct {
  WORD32 wf_02[02];
  WORD32 wf_03[03];
  WORD32 wf_04[04];
  WORD32 wf_15[15];
  WORD32 wf_16[16];
  WORD32 wf_18[18];
  WORD32 wf_24[24];
  WORD32 wf_30[30];
  WORD32 wf_32[32];
} ia_mps_dec_wf_tables_struct;

typedef struct { const WORD32 *wf[32]; } ia_mps_dec_wf_ptr_table_struct;

typedef struct {
  WORD32 twi_post_cos[64];
  WORD32 twi_post_sin[64];
  WORD32 hybrid_2_qmf[71];
  WORD32 local_sin_4[4];
  WORD32 local_sin_15[16];
  WORD32 local_sin_16[16];
  WORD32 local_sin_18[18];
  WORD32 local_sin_24[24];
  WORD32 local_sin_30[30];
  WORD32 local_sin_32[32];
} ia_mps_dec_mdct2qmf_table_struct;

typedef struct {
  WORD16 gmax_fix[72];
  WORD32 dwin_fix[72];
  WORD32 nstart_fix[5][72];
  WORD32 dfrac_fix[5][56];
  WORD32 part4[4];
  WORD32 part5[5];
  WORD32 part7[7];
  WORD32 part10[10];
  WORD32 part14[14];
  WORD32 part20[20];
  WORD32 part28[28];
  WORD32 part40[40];
  WORD32 w_real[16];
  WORD32 w_imag[16];
  WORD32 bitrev[16];
} ia_mps_dec_tonality_tables_struct;

typedef struct {
  WORD32 kernels_4_to_71[71];
  WORD32 kernels_5_to_71[71];
  WORD32 kernels_7_to_71[71];
  WORD32 kernels_10_to_71[71];
  WORD32 kernels_14_to_71[71];
  WORD32 kernels_20_to_71[71];
  WORD32 kernels_28_to_71[71];
  WORD32 bb_env_kernels[71];
} ia_mps_dec_kernels_table_struct;

typedef struct {
  WORD32 mapping_4_to_28[28];
  WORD32 mapping_5_to_28[28];
  WORD32 mapping_7_to_28[28];
  WORD32 mapping_10_to_28[28];
  WORD32 mapping_14_to_28[28];
  WORD32 mapping_20_to_28[28];
} ia_mps_dec_mapping_table_struct;

typedef struct {
  WORD32 num_input_channels;
  WORD32 num_output_channels;
  WORD32 num_ott_boxes;
  WORD32 num_ttt_boxes;
  WORD32 ott_mode_lfe[5];
} ia_mps_dec_tree_properties_struct;

typedef struct {
  ia_mps_dec_kernels_table_struct kernel_table;
  ia_mps_dec_mapping_table_struct map_table;
  ia_mps_dec_tree_properties_struct tree_property_table[7];
  WORD32 sampling_freq_table[15];
  WORD32 freq_res_table[8];
  WORD32 temp_shape_chan_table[2][7];
  WORD32 surround_gain_table[5];
  WORD32 lfe_gain_table[5];
  WORD32 clip_gain_table[8];
  WORD32 pb_stride_table[4];
  WORD32 smg_time_table[4];
  WORD32 dequant_cld[31];
  WORD32 dequant_cld_coarse[15];
  WORD32 dequant_cpc[52];
  WORD32 dequant_cpc_coarse[26];
  WORD32 dequant_icc[8];
  WORD32 factor_cld_tab_1[31];
  WORD32 hrtf_power[64];
  WORD32 envshape_data[2][5];
  WORD32 pcm_chnksz_level_3[5];
  WORD32 pcm_chnksz_level_4;
  WORD32 pcm_chnksz_level_7[6];
  WORD32 pcm_chnksz_level_8;
  WORD32 pcm_chnksz_level_11[2];
  WORD32 pcm_chnksz_level_13[4];
  WORD32 pcm_chnksz_level_15;
  WORD32 pcm_chnksz_level_19[4];
  WORD32 pcm_chnksz_level_25[3];
  WORD32 pcm_chnksz_level_26;
  WORD32 pcm_chnksz_level_31;
  WORD32 pcm_chnksz_level_51[4];
} ia_mps_dec_bitdec_tables_struct;

typedef struct {
  WORD32 blind_cld_mesh[31][21];
  WORD32 blind_icc_mesh[31][21];
  WORD32 blind_cpc_1_mesh[31][21];
  WORD32 blind_cpc_2_mesh[31][21];
} ia_mps_dec_mesh_tables_struct;

typedef struct {
  ia_mps_dec_mesh_tables_struct mesh_table;
  WORD32 exp_1[13];
  WORD32 exp_2[13];
  WORD32 exp_4[13];
  WORD32 exp_8[13];
  WORD32 exp_16[13];
  WORD32 exp_32[13];
  WORD32 exp_64[13];
  WORD32 exp_128[13];
} ia_mps_dec_blind_tables_struct;

typedef struct {
  WORD16 *cos_table_long[64];
  WORD16 *cos_table_short[10];
} ia_mps_dec_mdct2qmf_cos_table_struct;

typedef struct {
  WORD16 cos_table_long_32_00[32];
  WORD16 cos_table_long_32_01[32];
  WORD16 cos_table_long_32_02[32];
  WORD16 cos_table_long_32_03[32];
  WORD16 cos_table_long_32_04[32];
  WORD16 cos_table_long_32_05[32];
  WORD16 cos_table_long_32_06[32];
  WORD16 cos_table_long_32_07[32];
  WORD16 cos_table_long_32_08[32];
  WORD16 cos_table_long_32_09[32];
  WORD16 cos_table_long_32_10[32];
  WORD16 cos_table_long_32_11[32];
  WORD16 cos_table_long_32_12[32];
  WORD16 cos_table_long_32_13[32];
  WORD16 cos_table_long_32_14[32];
  WORD16 cos_table_long_32_15[32];
  WORD16 cos_table_long_32_16[32];
  WORD16 cos_table_long_32_17[32];
  WORD16 cos_table_long_32_18[32];
  WORD16 cos_table_long_32_19[32];
  WORD16 cos_table_long_32_20[32];
  WORD16 cos_table_long_32_21[32];
  WORD16 cos_table_long_32_22[32];
  WORD16 cos_table_long_32_23[32];
  WORD16 cos_table_long_32_24[32];
  WORD16 cos_table_long_32_25[32];
  WORD16 cos_table_long_32_26[32];
  WORD16 cos_table_long_32_27[32];
  WORD16 cos_table_long_32_28[32];
  WORD16 cos_table_long_32_29[32];
  WORD16 cos_table_long_32_30[32];
  WORD16 cos_table_long_32_31[32];

  WORD16 cos_table_long_30_00[30];
  WORD16 cos_table_long_30_01[30];
  WORD16 cos_table_long_30_02[30];
  WORD16 cos_table_long_30_03[30];
  WORD16 cos_table_long_30_04[30];
  WORD16 cos_table_long_30_05[30];
  WORD16 cos_table_long_30_06[30];
  WORD16 cos_table_long_30_07[30];
  WORD16 cos_table_long_30_08[30];
  WORD16 cos_table_long_30_09[30];
  WORD16 cos_table_long_30_10[30];
  WORD16 cos_table_long_30_11[30];
  WORD16 cos_table_long_30_12[30];
  WORD16 cos_table_long_30_13[30];
  WORD16 cos_table_long_30_14[30];
  WORD16 cos_table_long_30_15[30];
  WORD16 cos_table_long_30_16[30];
  WORD16 cos_table_long_30_17[30];
  WORD16 cos_table_long_30_18[30];
  WORD16 cos_table_long_30_19[30];
  WORD16 cos_table_long_30_20[30];
  WORD16 cos_table_long_30_21[30];
  WORD16 cos_table_long_30_22[30];
  WORD16 cos_table_long_30_23[30];
  WORD16 cos_table_long_30_24[30];
  WORD16 cos_table_long_30_25[30];
  WORD16 cos_table_long_30_26[30];
  WORD16 cos_table_long_30_27[30];
  WORD16 cos_table_long_30_28[30];
  WORD16 cos_table_long_30_29[30];

  WORD16 cos_table_long_24_00[24];
  WORD16 cos_table_long_24_01[24];
  WORD16 cos_table_long_24_02[24];
  WORD16 cos_table_long_24_03[24];
  WORD16 cos_table_long_24_04[24];
  WORD16 cos_table_long_24_05[24];
  WORD16 cos_table_long_24_06[24];
  WORD16 cos_table_long_24_07[24];
  WORD16 cos_table_long_24_08[24];
  WORD16 cos_table_long_24_09[24];
  WORD16 cos_table_long_24_10[24];
  WORD16 cos_table_long_24_11[24];
  WORD16 cos_table_long_24_12[24];
  WORD16 cos_table_long_24_13[24];
  WORD16 cos_table_long_24_14[24];
  WORD16 cos_table_long_24_15[24];
  WORD16 cos_table_long_24_16[24];
  WORD16 cos_table_long_24_17[24];
  WORD16 cos_table_long_24_18[24];
  WORD16 cos_table_long_24_19[24];
  WORD16 cos_table_long_24_20[24];
  WORD16 cos_table_long_24_21[24];
  WORD16 cos_table_long_24_22[24];
  WORD16 cos_table_long_24_23[24];

  WORD16 cos_table_long_18_00[18];
  WORD16 cos_table_long_18_01[18];
  WORD16 cos_table_long_18_02[18];
  WORD16 cos_table_long_18_03[18];
  WORD16 cos_table_long_18_04[18];
  WORD16 cos_table_long_18_05[18];
  WORD16 cos_table_long_18_06[18];
  WORD16 cos_table_long_18_07[18];
  WORD16 cos_table_long_18_08[18];
  WORD16 cos_table_long_18_09[18];
  WORD16 cos_table_long_18_10[18];
  WORD16 cos_table_long_18_11[18];
  WORD16 cos_table_long_18_12[18];
  WORD16 cos_table_long_18_13[18];
  WORD16 cos_table_long_18_14[18];
  WORD16 cos_table_long_18_15[18];
  WORD16 cos_table_long_18_16[18];
  WORD16 cos_table_long_18_17[18];

  WORD16 cos_table_long_16_00[16];
  WORD16 cos_table_long_16_01[16];
  WORD16 cos_table_long_16_02[16];
  WORD16 cos_table_long_16_03[16];
  WORD16 cos_table_long_16_04[16];
  WORD16 cos_table_long_16_05[16];
  WORD16 cos_table_long_16_06[16];
  WORD16 cos_table_long_16_07[16];
  WORD16 cos_table_long_16_08[16];
  WORD16 cos_table_long_16_09[16];
  WORD16 cos_table_long_16_10[16];
  WORD16 cos_table_long_16_11[16];
  WORD16 cos_table_long_16_12[16];
  WORD16 cos_table_long_16_13[16];
  WORD16 cos_table_long_16_14[16];
  WORD16 cos_table_long_16_15[16];

  WORD16 cos_table_long_15_00[15];
  WORD16 cos_table_long_15_01[15];
  WORD16 cos_table_long_15_02[15];
  WORD16 cos_table_long_15_03[15];
  WORD16 cos_table_long_15_04[15];
  WORD16 cos_table_long_15_05[15];
  WORD16 cos_table_long_15_06[15];
  WORD16 cos_table_long_15_07[15];
  WORD16 cos_table_long_15_08[15];
  WORD16 cos_table_long_15_09[15];
  WORD16 cos_table_long_15_10[15];
  WORD16 cos_table_long_15_11[15];
  WORD16 cos_table_long_15_12[15];
  WORD16 cos_table_long_15_13[15];
  WORD16 cos_table_long_15_14[15];

  WORD16 cos_table_short_4_00[4];
  WORD16 cos_table_short_4_01[4];
  WORD16 cos_table_short_4_02[4];
  WORD16 cos_table_short_4_03[4];

  WORD16 cos_table_short_3_00[3];
  WORD16 cos_table_short_3_01[3];
  WORD16 cos_table_short_3_02[3];

  WORD16 cos_table_short_2_00[2];
  WORD16 cos_table_short_2_01[2];
} ia_mps_dec_mdct2qmf_tables_struct;

#endif /* IXHEAACD_MPS_STRUCT_DEF_H */
