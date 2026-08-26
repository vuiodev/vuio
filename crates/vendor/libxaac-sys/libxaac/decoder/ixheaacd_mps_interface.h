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
#ifndef IXHEAACD_MPS_INTERFACE_H
#define IXHEAACD_MPS_INTERFACE_H

#define FIVE_POINT_ONE_CHANNEL_MASK (0x3F)
#define SEVEN_POINT_ONE_CHANNEL_MASK1 (0xFF)
#define SEVEN_POINT_ONE_CHANNEL_MASK2 (0x33F)

IA_ERRORCODE ixheaacd_parse_frame(ia_heaac_mps_state_struct *pstr_mps_state);

WORD32 ixheaacd_mps_create(ia_mps_dec_state_struct *self, WORD32 bs_frame_len,
                           WORD32 residual_coding,
                           ia_usac_dec_mps_config_struct *usac_mps_config);

IA_ERRORCODE ixheaacd_mps_frame_parsing(ia_mps_dec_state_struct* self,
                                        WORD32 independency_flag,
                                        ia_bit_buf_struct *it_bit_buff);

WORD32 ixheaacd_mps_apply(ia_mps_dec_state_struct *self, FLOAT32 **pointers[4],
                          FLOAT32 (*out_samples)[4096]);

#endif /* IXHEAACD_MPS_INTERFACE_H */
