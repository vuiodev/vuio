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
#ifndef IXHEAACD_CREATE_H
#define IXHEAACD_CREATE_H

typedef struct {
  struct ia_bit_buf_struct dec_bit_buf;
  ia_frame_data_struct str_frame_data;
  ia_usac_data_struct str_usac_data;
  jmp_buf *xaac_jmp_buf;
} ia_dec_data_struct;

WORD32 ixheaacd_frm_data_init(ia_audio_specific_config_struct *pstr_audio_conf,
                              ia_dec_data_struct *pstr_dec_data);

WORD32 ixheaacd_decode_create(ia_exhaacplus_dec_api_struct *dec_handle,
                              ia_dec_data_struct *pstr_dec_data,
                              WORD32 tracks_for_decoder);

WORD32 ixheaacd_decode_free(pVOID codec_handle);
ia_handle_sbr_dec_inst_struct ixheaacd_init_sbr(
    WORD32 sample_rate_dec, WORD32 samp_per_frame, FLAG *down_sample_flag,
    VOID *sbr_persistent_mem_v, WORD32 *ptr_overlap_buf, WORD channel,
    WORD ps_enable, WORD sbr_ratio_idx, WORD output_frame_size, WORD *use_hbe,
    VOID *p_usac_dflt_header, ia_sbr_header_data_struct str_sbr_config,
    WORD audio_object_type, WORD32 ldmps_present, WORD32 ldsbr_present);

VOID ixheaacd_setesbr_flags(VOID *sbr_persistent_mem_v, FLAG pvc_flag,
                            FLAG hbe_flag, FLAG inter_tes_flag);

#endif
