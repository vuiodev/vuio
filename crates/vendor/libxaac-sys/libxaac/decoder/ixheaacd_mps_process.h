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
#ifndef IXHEAACD_MPS_PROCESS_H
#define IXHEAACD_MPS_PROCESS_H

VOID ixheaacd_mps_qmf_hyb_analysis(ia_mps_dec_state_struct *self);
VOID ixheaacd_mps_qmf_hyb_analysis_no_pre_mix(ia_mps_dec_state_struct *self);
VOID ixheaacd_mps_qmf_hyb_synthesis(ia_mps_dec_state_struct *self);
VOID ixheaacd_mps_decor(ia_mps_dec_state_struct *self);
VOID ixheaacd_mps_create_w(ia_mps_dec_state_struct *self);
VOID ixheaacd_mps_mix_res_decor(ia_mps_dec_state_struct *self);
VOID ixheaacd_mps_synt_calc(ia_mps_dec_state_struct *self);

VOID ixheaacd_mdct_2_qmf(ia_heaac_mps_state_struct *pstr_mps_state);
VOID ixheaacd_hybrid_qmf_analysis(ia_heaac_mps_state_struct *pstr_mps_state);
VOID ixheaacd_create_w(ia_heaac_mps_state_struct *pstr_mps_state);
VOID ixheaacd_merge_res_decor(ia_heaac_mps_state_struct *pstr_mps_state);
VOID ixheaacd_update_buffers(ia_heaac_mps_state_struct *pstr_mps_state);

#endif /* IXHEAACD_MPS_PROCESS_H */
