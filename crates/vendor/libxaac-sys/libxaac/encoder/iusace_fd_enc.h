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
IA_ERRORCODE iusace_fd_encode(ia_sfb_params_struct *pstr_sfb_prms, WORD32 usac_independancy_flag,
                              ia_usac_data_struct *pstr_usac_data,
                              ia_usac_encoder_config_struct *pstr_usac_config,
                              ia_bit_buf_struct *pstr_it_bit_buff, WORD32 nr_core_coder_ch,
                              WORD32 chn, WORD32 ele_id, WORD32 *bit_written,
                              WORD32 *is_quant_spec_zero, WORD32* is_gain_limited);
