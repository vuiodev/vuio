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
IA_ERRORCODE ia_enhaacplus_enc_init_element_info(WORD32 num_channels,
                                                 ixheaace_element_info *pstr_element_info,
                                                 WORD32 ele_type, WORD32 element_instance_tag);

IA_ERRORCODE ia_enhaacplus_enc_init_element_bits(ixheaace_element_bits *element_bits,
                                                 ixheaace_element_info pstr_element_info,
                                                 WORD32 bitrate_tot, WORD32 average_bits_tot,
                                                 WORD32 aot, WORD32 static_bits_tot,
                                                 WORD32 bit_res, FLAG flag_framelength_small);
