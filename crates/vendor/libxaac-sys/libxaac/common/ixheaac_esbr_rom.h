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
#ifndef IXHEAAC_ESBR_ROM_H
#define IXHEAAC_ESBR_ROM_H

extern const FLOAT32
    ixheaac_sub_samp_qmf_window_coeff[40 + 80 + 120 + 160 + 200 + 240 + 320 + 400 + 440];
extern const FLOAT32 ixheaac_random_phase[512][2];
extern const FLOAT32 ixheaac_hphase_tbl[2][8];

extern const FLOAT32 ixheaac_g_lim_gains[4];

typedef FLOAT32 ia_fir_table_struct[5];
extern const ia_fir_table_struct ixheaac_fir_0;
extern const ia_fir_table_struct ixheaac_fir_1;
extern const ia_fir_table_struct ixheaac_fir_2;
extern const ia_fir_table_struct ixheaac_fir_3;
extern const ia_fir_table_struct ixheaac_fir_4;
extern const ia_fir_table_struct *ixheaac_fir_table[5];
extern const FLOAT32 ixheaac_q_gamma_table[4];
extern const WORD32 ixheaac_start_subband2kL_tbl[33];
extern const FLOAT32 ixheaac_cos_table_trans_qmf[7][32 * 2];

extern const FLOAT32 ixheaac_phase_vocoder_cos_table[64];
extern const FLOAT32 ixheaac_phase_vocoder_sin_table[64];
extern const FLOAT32 ixheaac_hbe_post_anal_proc_interp_coeff[4][2];

extern const FLOAT32 ixheaac_hbe_x_prod_cos_table_trans_2[2 * (128 + 128)];
extern const FLOAT32 ixheaac_hbe_x_prod_cos_table_trans_3[2 * (128 + 128)];
extern const FLOAT32 ixheaac_hbe_x_prod_cos_table_trans_4[2 * (128 + 128)];
extern const FLOAT32 ixheaac_hbe_x_prod_cos_table_trans_4_1[2 * (128 + 128)];

extern const FLOAT32 ixheaac_synth_cos_table_kl_4[16];
extern const FLOAT32 ixheaac_synth_cos_table_kl_8[32];
extern const FLOAT32 ixheaac_synth_cos_table_kl_12[48];
extern const FLOAT32 ixheaac_synth_cos_table_kl_16[64];
extern const FLOAT32 ixheaac_synth_cos_table_kl_20[40 * 20];
extern const FLOAT32 ixheaac_analy_cos_sin_table_kl_8[32];
extern const FLOAT32 ixheaac_analy_cos_sin_table_kl_16[64];
extern const FLOAT32 ixheaac_analy_cos_sin_table_kl_24[96];
extern const FLOAT32 ixheaac_analy_cos_sin_table_kl_32[128];
extern const FLOAT32 ixheaac_analy_cos_sin_table_kl_40[40 * 40 * 2];

extern const FLOAT32 ixheaac_dft_hbe_window_ts_12[13];
extern const FLOAT32 ixheaac_dft_hbe_window_ts_18[19];

extern const FLOAT32 ixheaac_sine_pi_n_by_1024[1024];
extern const FLOAT32 ixheaac_sine_pi_n_by_960[960];
extern const FLOAT32 ixheaac_sine_pi_n_by_896[896];
extern const FLOAT32 ixheaac_sine_pi_n_by_832[832];
extern const FLOAT32 ixheaac_sine_pi_n_by_768[768];
extern const FLOAT32 ixheaac_sine_pi_n_by_704[704];
extern const FLOAT32 ixheaac_sine_pi_n_by_640[640];
extern const FLOAT32 ixheaac_sine_pi_n_by_576[576];
extern const FLOAT32 ixheaac_sine_pi_by_2_N[];
extern const FLOAT32 ixheaac_sin_cos_448[];
extern const FLOAT32 ixheaac_sin_cos_672[];
extern const FLOAT32 ixheaac_sin_cos_512[];
extern const FLOAT32 ixheaac_sin_cos_576[];
extern const FLOAT32 ixheaac_sin_cos_384[];
extern const FLOAT32 ixheaac_sin_cos_768[];

extern const FLOAT32 ixheaac_sel_case[5][8];
extern const FLOAT32 ixheaac_sub_samp_qmf_window_coeff_28_36[280 + 360];
extern const FLOAT32 ixheaac_analy_cos_sin_table_kl_56[56 * 56 * 2];
#endif /* IXHEAAC_ESBR_ROM_H */
