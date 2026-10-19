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
#ifndef IXHEAACD_ENV_DEC_H
#define IXHEAACD_ENV_DEC_H

IA_ERRORCODE ixheaacd_dec_sbrdata(ia_sbr_header_data_struct *ptr_header_data_ch_0,
                                  ia_sbr_header_data_struct *ptr_header_data_ch_1,
                                  ia_sbr_frame_info_data_struct *ptr_sbr_data_ch_0,
                                  ia_sbr_prev_frame_data_struct *ptr_prev_data_ch_0,
                                  ia_sbr_frame_info_data_struct *ptr_sbr_data_ch_1,
                                  ia_sbr_prev_frame_data_struct *ptr_prev_data_ch_1,
                                  ixheaacd_misc_tables *ptr_common_tables, WORD32 ldmps_present,
                                  WORD32 audio_object_type, WORD32 ec_flag);

IA_ERRORCODE ixheaacd_dec_sbrdata_for_pvc(
    ia_sbr_header_data_struct *ptr_header_data,
    ia_sbr_frame_info_data_struct *ptr_sbr_data,
    ia_sbr_prev_frame_data_struct *ptr_prev_data,
    WORD32 audio_object_type);

VOID ixheaacd_harm_idx_onethreelp(WORD32 *ptr_real_buf, WORD16 *ptr_gain_buf,
                                  WORD scale_change, WORD16 *ptr_sine_level_buf,
                                  const WORD32 *ptr_rand_ph,
                                  WORD16 *noise_lvl_me, WORD num_subband,
                                  FLAG noise_absc_flag, WORD freq_inv_flag,
                                  WORD noise_e, WORD sub_band_start);

VOID ixheaacd_harm_idx_zerotwo(FLAG noise_absc_flag, WORD16 num_subband,
                               WORD32 *ptr_real_buf, WORD32 *ptr_im,
                               WORD16 *smoothed_gain, WORD16 *smoothed_noise,
                               WORD factor, WORD16 *ptr_gain_buf,
                               WORD16 scale_change, const WORD32 *ptr_rand_ph,
                               WORD16 *ptr_sine_level_buf, WORD16 noise_e,
                               WORD32 harm_index);

VOID ixheaacd_harm_idx_onethree(FLAG noise_absc_flag, WORD16 num_subband,
                                WORD32 *ptr_real_buf, WORD32 *ptr_im,
                                WORD16 *smoothed_gain, WORD16 *smoothed_noise,
                                WORD factor, WORD16 *ptr_gain_buf,
                                WORD16 scale_change, const WORD32 *ptr_rand_ph,
                                WORD16 *ptr_sine_level_buf, WORD16 noise_e,
                                WORD freq_inv_flag, WORD32 harm_index);

IA_ERRORCODE ixheaacd_dec_envelope(ia_sbr_header_data_struct *ptr_header_data,
                                   ia_sbr_frame_info_data_struct *ptr_sbr_data,
                                   ia_sbr_prev_frame_data_struct *ptr_prev_data,
                                   ia_sbr_prev_frame_data_struct *ptr_prev_data_ch_1,
                                   ixheaacd_misc_tables *pstr_common_tables,
                                   WORD32 audio_object_type, WORD32 ec_flag);

VOID ixheaacd_lean_sbrconcealment(ia_sbr_header_data_struct *ptr_header_data,
                                  ia_sbr_frame_info_data_struct *ptr_sbr_data,
                                  ia_sbr_prev_frame_data_struct *ptr_prev_data);

#endif
