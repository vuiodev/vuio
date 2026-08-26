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
#ifndef IXHEAACD_LD_MPS_DEC_H
#define IXHEAACD_LD_MPS_DEC_H

WORD32 ixheaacd_ld_mps_apply(ia_exhaacplus_dec_api_struct *p_obj_exhaacplus_dec,
                             WORD16 *output_buf);

IA_ERRORCODE ixheaacd_ld_mps_frame_parsing(
    ia_mps_dec_state_struct *self, ia_bit_buf_struct *it_bit_buff);

WORD32 ixheaacd_extension_payload(ia_bit_buf_struct *it_bit_buff, WORD32 *cnt,
                                  ia_mps_dec_state_struct *self);

IA_ERRORCODE ixheaacd_ld_spatial_specific_config(
    ia_usac_dec_mps_config_struct *config, ia_bit_buf_struct *it_bit_buff);

IA_ERRORCODE ixheaacd_heaac_mps_apply(ia_exhaacplus_dec_api_struct *self,
                                      WORD16 *output_buf, UWORD8 *mps_buffer,
                                      WORD32 mps_bytes);

#endif /* IXHEAACD_LD_MPS_DEC_H */
