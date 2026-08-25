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
#ifndef IXHEAACD_EC_H
#define IXHEAACD_EC_H

VOID ixheaacd_aac_ec_init(ia_ec_state_str *pstr_ec_state);

VOID ixheaacd_aac_apply_ec(ia_ec_state_str *pstr_ec_state,
                           ia_aac_dec_channel_info_struct *pstr_aac_dec_channel_info,
                           const ia_usac_samp_rate_info *pstr_samp_rate_info,
                           const WORD32 num_samples, ia_ics_info_struct *pstr_ics_info,
                           const WORD32 frame_status);

VOID ixheaacd_usac_ec_init(ia_ec_state_str *pstr_ec_state, WORD32 core_coder_mode);

VOID ixheaacd_usac_ec_save_states(ia_ec_state_str *pstr_ec_state,
                                  ia_usac_data_struct *pstr_usac_data, WORD32 ch);

VOID ixheaacd_usac_apply_ec(ia_usac_data_struct *pstr_usac_data,
                            const ia_usac_samp_rate_info *pstr_samp_rate_info, WORD32 ch);

VOID ixheaacd_usac_lpc_ec(FLOAT32 lsp[][ORDER], FLOAT32 *lpc4_lsf, FLOAT32 *lsf_adaptive_mean,
                          const WORD32 first_lpd_flag);

VOID ixheaacd_usac_tcx_ec(ia_usac_data_struct *pstr_usac_data, ia_usac_lpd_decoder_handle st,
                          FLOAT32 *ptr_lsp_curr, WORD32 frame_idx, FLOAT32 *lp_flt_coff_a);

#endif /* IXHEAACD_EC_H */
