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
IA_ERRORCODE iusace_cplx_pred_proc(
    ia_usac_data_struct *ptr_usac_data, ia_usac_encoder_config_struct *ptr_usac_config,
    WORD32 usac_independancy_flag, ia_sfb_params_struct *pstr_sfb_prms, WORD32 chn,
    ia_psy_mod_data_struct *pstr_psy_data, const WORD32 *ptr_sfb_offsets,
    FLOAT64 *scratch_cmpx_mdct_temp_buf, FLOAT64 *ptr_ms_spec, FLOAT32 nrg_mid, FLOAT32 nrg_side);
