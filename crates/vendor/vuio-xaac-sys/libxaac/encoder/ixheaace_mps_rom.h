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
extern const UWORD8 freq_res_bin_table_ld[MAX_FREQ_RES_INDEX];
extern const UWORD8 freq_res_bin_table_usac[8];
extern const UWORD8 freq_res_stride_table_212[4];
extern const ixheaace_mps_sac_tree_description tree_config_table[];
extern const WORD32 freq_res_bin_table[];
extern const UWORD8 valid_bands_ld[10];
extern const WORD32 freq_res_stride_table[];
extern const WORD32 temp_shape_chan_table[][7];
extern const FLOAT32 p8_13[13];
extern const FLOAT32 p2_6[6];
extern const WORD32 kernels_20[];

extern const UWORD8 lav_huff_val[4];
extern const UWORD8 lav_huff_len[4];

extern const UWORD8 lav_step_cld[];
extern const UWORD8 lav_step_icc[];

extern const ixheaace_mps_tree_setup tree_setup_table;

extern const UWORD8 subband_2_parameter_ld[NUM_QMF_BANDS];

extern const UWORD8 subband_2_parameter_usac[MAX_QMF_BANDS];

extern const FLOAT32 cld_quant_table_fine_enc[IXHEAACE_MPS_MAX_CLD_QUANT_FINE];

extern const FLOAT32 cld_quant_table_coarse_enc[IXHEAACE_MPS_MAX_CLD_QUANT_COARSE];

extern const FLOAT32 icc_quant_table_fine[IXHEAACE_MPS_MAX_ICC_QUANT_FINE];

extern const FLOAT32 icc_quant_table_coarse[IXHEAACE_MPS_MAX_ICC_QUANT_COARSE];

extern const FLOAT32 pre_gain_factor_table_flt_new[41];

extern const FLOAT32 dmx_gain_table_flt[];

extern ixheaace_mps_sac_qmf_ana_filter_bank qmf_fltbank;
extern const FLOAT32 trig_data_fct4_8[8];
extern const FLOAT32 trig_data_fct4_16[16];
extern const FLOAT32 trig_data_fct4_32[32];

extern const FLOAT32 ia_mps_enc_qmf_64_640[325];

extern const FLOAT32 sbr_cos_twiddle[];
extern const FLOAT32 sbr_sin_twiddle[];
extern const FLOAT32 sbr_alt_sin_twiddle[];
extern const FLOAT32 fft_c[4];

extern const FLOAT32 ia_qmf_anl_addt_cos[32];
extern const FLOAT32 ia_qmf_anl_addt_sin[32];
