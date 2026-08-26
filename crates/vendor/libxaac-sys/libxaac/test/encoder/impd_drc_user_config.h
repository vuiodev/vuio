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
VOID ixheaace_read_drc_config_params(FILE *fp, ia_drc_enc_params_struct *pstr_enc_params,
                                     ia_drc_uni_drc_config_struct *pstr_uni_drc_config,
                                     ia_drc_loudness_info_set_struct *pstr_enc_loudness_info_set,
                                     ia_drc_uni_drc_gain_ext_struct *pstr_enc_gain_extension,
                                     WORD32 in_ch);
