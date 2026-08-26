/******************************************************************************
 *
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
#ifndef IMPD_DRC_PARSER_INTERFACE_H
#define IMPD_DRC_PARSER_INTERFACE_H

WORD32
impd_unidrc_interface_signature_read(
    ia_bit_buf_struct* it_bit_buff,
    ia_drc_uni_interface_signat_struct* drc_uni_interface_signature);

WORD32
impd_sys_interface_read(ia_bit_buf_struct* it_bit_buff,
                        ia_system_interface_struct* system_interface);

WORD32
impd_loudness_norm_control_interface_read(
    ia_bit_buf_struct* it_bit_buff,
    ia_loudness_norm_ctrl_interface_struct* loudness_norm_ctrl_interface);

WORD32
impd_loudness_norm_param_interface_read(
    ia_bit_buf_struct* it_bit_buff,
    ia_loudness_norm_parameter_interface_struct* loudness_norm_param_interface);

WORD32
impd_drc_interface_read(ia_bit_buf_struct* it_bit_buff,
                        ia_dyn_rng_ctrl_interface_struct* drc_ctrl_interface);

WORD32
impd_drc_param_interface_read(
    ia_bit_buf_struct* it_bit_buff,
    ia_drc_parameter_interface_struct* drc_parameter_interface);

WORD32
impd_unidrc_interface_extension_read(
    ia_bit_buf_struct* it_bit_buff,
    ia_drc_interface_struct* impd_drc_uni_interface,
    ia_drc_uni_interface_ext_struct* drc_uni_interface_ext);

WORD32
impd_unidrc_interface_read(ia_bit_buf_struct* it_bit_buff,
                           ia_drc_interface_struct* impd_drc_uni_interface);

#endif
