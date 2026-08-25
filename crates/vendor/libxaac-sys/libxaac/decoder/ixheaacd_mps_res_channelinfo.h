/******************************************************************************
 *
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
#ifndef IXHEAACD_MPS_RES_CHANNELINFO_H
#define IXHEAACD_MPS_RES_CHANNELINFO_H

WORD16 ixheaacd_res_ics_read(ia_bit_buf_struct *it_bit_buf,
                             ia_mps_dec_residual_ics_info_struct *p_ics_info,
                             WORD8 tot_sf_bands_ls[2]);

const WORD16 *ixheaacd_res_get_sfb_offsets(ia_mps_dec_residual_ics_info_struct *p_ics_info,
                                           ia_mps_dec_residual_aac_tables_struct *aac_tables_ptr);

const WORD8 *ixheaacd_res_get_sfb_width(ia_mps_dec_residual_ics_info_struct *p_ics_info,
                                        ia_mps_dec_residual_aac_tables_struct *aac_tables_ptr);

#endif /* IXHEAACD_MPS_RES_CHANNELINFO_H */
