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
#ifndef IXHEAACD_ENV_CALC_H
#define IXHEAACD_ENV_CALC_H

typedef struct {
  WORD16 *filt_buf_me;
  WORD16 *filt_buf_noise_m;
  WORD32 filt_buf_noise_e;
  FLAG start_up;
  WORD16 ph_index;
  WORD16 tansient_env_prev;
  WORD8 harm_flags_prev[MAX_FREQ_COEFFS];
  WORD16 harm_index;
} ia_sbr_calc_env_struct;

IA_ERRORCODE ixheaacd_calc_sbrenvelope(
    ia_sbr_scale_fact_struct *sbr_scale_factor,
    ia_sbr_calc_env_struct *ptr_sbr_calc_env,
    ia_sbr_header_data_struct *ptr_header_data,
    ia_sbr_frame_info_data_struct *ptr_frame_data,
    ia_sbr_prev_frame_data_struct *ptr_prev_frame_data,
    WORD32 **anal_buf_real_mant, WORD32 **anal_buf_imag_mant,
    WORD16 *degree_alias, FLAG low_pow_flag,
    ia_sbr_tables_struct *ptr_sbr_tables,
    ixheaacd_misc_tables *pstr_common_tables, WORD32 *ptr_qmf_matrix,
    WORD32 audio_object_type);

VOID ixheaacd_reset_sbrenvelope_calc(ia_sbr_calc_env_struct *ptr_calc_env);

VOID ixheaacd_derive_lim_band_tbl(
    ia_sbr_header_data_struct *ptr_header_data,
    const ia_patch_param_struct *p_str_patch_param, WORD16 num_patches,
    ixheaacd_misc_tables *pstr_common_tables);

VOID ixheaacd_filt_buf_update(WORD16 *filt_buf_mant,
                              WORD16 *ptr_filt_buf_noise,
                              WORD16 *nrg_gain_mant,
                              WORD16 *noise_level_mant,
                              WORD32 num_sub_bands);

VOID ixheaacd_noise_level_rescaling(WORD16 *noise_level_mant, WORD32 diff,
                                    WORD32 num_sub_bands,
                                    WORD32 ixheaacd_drc_offset);

VOID ixheaacd_enery_calc_persfb(WORD32 **anal_buf_real, WORD32 **anal_buf_imag,
                                WORD32 num_sf_bands, WORD16 *freq_band_table,
                                WORD32 start_pos, WORD32 next_pos,
                                WORD32 max_qmf_subband_aac, WORD32 frame_exp,
                                WORD16 *nrg_est_m, FLAG low_pow_flag,
                                ia_sbr_tables_struct *ptr_sbr_tables);

VOID ixheaacd_avggain_calc(WORD16 *ptr_enrg_orig, WORD16 *nrg_est,
                           WORD32 sub_band_start, WORD32 sub_band_end,
                           WORD16 *sum_orig_mant, WORD16 *sum_orig_exp,
                           WORD16 *ptr_avg_gain_mant, WORD16 *ptr_avg_gain_exp,
                           ixheaacd_misc_tables *pstr_common_tables,
                           WORD32 flag);

VOID ixheaacd_adj_timeslot(WORD32 *ptr_buf_real, WORD32 *ptr_buf_imag,
                           WORD16 *ptr_filt_buf, WORD16 *ptr_filt_buf_noise,
                           WORD16 *ptr_gain_buf, WORD16 *ptr_noise_floor,
                           WORD16 *ptr_sine_lvl_buf, WORD16 noise_floor_exp,
                           WORD16 *ptr_harm_index, WORD16 sub_band_start,
                           WORD16 num_sub_bands, WORD16 scale_change,
                           WORD16 smooth_ratio, FLAG num_noise_flg,
                           WORD16 *ptr_phase_index,
                           ia_sbr_tables_struct *ptr_sbr_tables);

VOID ixheaacd_map_sineflags(WORD16 *freq_band_table, WORD16 num_sf_bands,
                            FLAG *add_harmonics, WORD8 *harm_flags_prev,
                            WORD16 transient_env, WORD8 *sine_mapped_matrix);

VOID ixheaacd_adjust_scale_dec(WORD32 **re, WORD32 **im, WORD sub_band_start,
                               WORD num_sub_bands, WORD start_pos,
                               WORD next_pos, WORD shift, FLAG low_pow_flag);

VOID ixheaacd_adjust_scale_armv7(WORD32 **re, WORD32 **im, WORD sub_band_start,
                                 WORD num_sub_bands, WORD start_pos,
                                 WORD next_pos, WORD shift, FLAG low_pow_flag);

WORD16 ixheaacd_expsubbandsamples_dec(WORD32 **anal_buf_real_mant,
                                      WORD32 **anal_buf_imag_mant,
                                      WORD sub_band_start, WORD sub_band_end,
                                      WORD start_pos, WORD end_pos,
                                      FLAG low_pow_flag);

WORD16 ixheaacd_expsubbandsamples_armv7(WORD32 **anal_buf_real_mant,
                                        WORD32 **anal_buf_imag_mant,
                                        WORD sub_band_start, WORD sub_band_end,
                                        WORD start_pos, WORD end_pos,
                                        FLAG low_pow_flag);

VOID ixheaacd_enery_calc_per_subband_dec(WORD32 start_pos, WORD32 next_pos,
                                         WORD32 sub_band_start,
                                         WORD32 sub_band_end, WORD32 frame_exp,
                                         WORD16 *nrg_est_mant,
                                         FLAG low_pow_flag,
                                         ia_sbr_tables_struct *ptr_sbr_tables,
                                         WORD32 *ptr_qmf_matrix);

VOID ixheaacd_enery_calc_per_subband_armv7(
    WORD32 start_pos, WORD32 next_pos, WORD32 sub_band_start,
    WORD32 sub_band_end, WORD32 frame_exp, WORD16 *nrg_est_mant,
    FLAG low_pow_flag, ia_sbr_tables_struct *ptr_sbr_tables,
    WORD32 *ptr_qmf_matrix);

WORD32 ixheaacd_reset_hf_generator(ia_sbr_hf_generator_struct *hf_generator,
                                   ia_sbr_header_data_struct *ptr_header_data,
                                   WORD audio_obj_type);

VOID ixheaacd_harm_idx_zerotwolp_dec(WORD32 *ptr_real_buf, WORD16 *ptr_gain_buf,
                                     WORD scale_change,
                                     WORD16 *ptr_sine_level_buf,
                                     const WORD32 *ptr_rand_ph,
                                     WORD16 *noise_lvl_me, WORD num_sub_bands,
                                     FLAG noise_absc_flag, WORD32 harm_index);

VOID ixheaacd_harm_idx_zerotwolp_armv7(WORD32 *ptr_real_buf,
                                       WORD16 *ptr_gain_buf, WORD scale_change,
                                       WORD16 *ptr_sine_level_buf,
                                       const WORD32 *ptr_rand_ph,
                                       WORD16 *noise_lvl_me, WORD num_sub_bands,
                                       FLAG noise_absc_flag, WORD32 harm_index);

VOID ixheaacd_harm_idx_onethreelp(WORD32 *ptr_real_buf, WORD16 *ptr_gain_buf,
                                  WORD scale_change, WORD16 *ptr_sine_level_buf,
                                  const WORD32 *ptr_rand_ph,
                                  WORD16 *noise_lvl_me, WORD num_sub_bands,
                                  FLAG noise_absc_flag, WORD freq_inv_flag,
                                  WORD noise_e, WORD sub_band_start);

VOID ixheaacd_conv_ergtoamplitudelp_dec(WORD32 bands, WORD16 noise_e,
                                        WORD16 *nrg_sine, WORD16 *nrg_gain,
                                        WORD16 *noise_level_mant,
                                        WORD16 *sqrt_table);

VOID ixheaacd_conv_ergtoamplitudelp_armv7(WORD32 bands, WORD16 noise_e,
                                          WORD16 *nrg_sine, WORD16 *nrg_gain,
                                          WORD16 *noise_level_mant,
                                          WORD16 *sqrt_table);

VOID ixheaacd_conv_ergtoamplitude_dec(WORD32 bands, WORD16 noise_e,
                                      WORD16 *nrg_sine, WORD16 *nrg_gain,
                                      WORD16 *noise_level_mant,
                                      WORD16 *sqrt_table);

VOID ixheaacd_conv_ergtoamplitude_armv7(WORD32 bands, WORD16 noise_e,
                                        WORD16 *nrg_sine, WORD16 *nrg_gain,
                                        WORD16 *noise_level_mant,
                                        WORD16 *sqrt_table);

VOID ixheaacd_subbandgain_calc(WORD16 e_orig_mant_matrix, WORD16 tmp_noise_mant,
                               WORD16 nrg_est_mant, WORD16 nrg_est_exp,
                               WORD16 tmp_noise_exp, WORD16 nrg_ref_exp,
                               FLAG sine_present_flag, FLAG sine_mapped_matrix,
                               FLAG noise_absc_flag, WORD16 *ptr_nrg_gain_mant,
                               WORD16 *ptr_noise_floor_mant,
                               WORD16 *ptr_nrg_sine_m,
                               ixheaacd_misc_tables *pstr_common_tables);

VOID ixheaacd_equalize_filt_buff_exp(WORD16 *ptr_filt_buf,
                                                     WORD16 *nrg_gain,
                                                     WORD32 subbands);

#endif
