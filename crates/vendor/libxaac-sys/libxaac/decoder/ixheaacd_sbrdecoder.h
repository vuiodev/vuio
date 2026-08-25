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
#ifndef IXHEAACD_SBRDECODER_H
#define IXHEAACD_SBRDECODER_H

#define EXT_DYNAMIC_RANGE 11

#define EXT_SAC_DATA 12
#define SBR_EXTENSION 13
#define SBR_EXTENSION_CRC 14

#define MAXNRELEMENTS 1
#define MAXNRSBRCHANNELS 2
#define MAXSBRBYTES 1024

#define SBRDEC_OK 0

typedef enum {
  SBR_ID_SCE = 0,
  SBR_ID_CPE,
  SBR_ID_CCE = 2,
  SBR_ID_LCS,
  SBR_ID_LFE,
  SBR_ID_DSE,
  SBR_ID_PCE,
  SBR_ID_FIL,
  SBR_ID_END
} SBR_ELEMENT_ID;

typedef struct {
  WORD32 sbr_ele_id;
  WORD32 extension_type;
  WORD32 size_payload;
  WORD8 *ptr_sbr_data;
  WORD8 *ptr_prev_sbr_data;
  WORD32 prev_size_payload;
  WORD32 frame_error_flag[2];
  WORD32 use_frame_slot;
  WORD32 prev_sbr_ele_id;
  WORD32 prev_extension_type;
  WORD32 size_payload_old;
  WORD8 sbr_prev_data[MAXSBRBYTES];
} ia_sbr_element_stream_struct;

typedef struct {
  WORD16 no_elements;
  ia_sbr_element_stream_struct str_sbr_ele[MAXNRELEMENTS];
} ia_aac_dec_sbr_bitstream_struct;
typedef enum { UNKNOWN_SBR = 0, ORIG_SBR, PVC_SBR } SBR_TYPE_ID;

typedef struct ia_sbr_dec_inst_struct *ia_handle_sbr_dec_inst_struct;

typedef struct {
  VOID *ptr_work_buf_core;
} ia_sbr_scr_struct;

IA_ERRORCODE ixheaacd_applysbr(
    ia_handle_sbr_dec_inst_struct self, ia_aac_dec_sbr_bitstream_struct *p_sbr_bit_stream,
    WORD16 *core_sample_buf, WORD16 *codec_num_channels, FLAG frame_status, FLAG down_samp_flag,
    FLAG down_mix_flag, ia_sbr_scr_struct *sbr_scratch_struct, WORD32 ps_enable, WORD32 ch_fac,
    WORD32 slot_element, ia_bit_buf_struct *it_bit_buff, ia_drc_dec_struct *pstr_drc_dec,
    WORD eld_sbr_flag, WORD audio_object_type, WORD32 init_flag, WORD32 ldmps_present,
    WORD32 frame_size, WORD32 heaac_mps_present, WORD32 ec_flag, FLAG first_frame);

IA_ERRORCODE ixheaacd_parse_sbr(ia_handle_sbr_dec_inst_struct self,
                                ia_aac_dec_sbr_bitstream_struct *p_sbr_bit_stream,
                                WORD16 *codec_num_channels, FLAG frame_status,
                                ia_sbr_scr_struct *sbr_scratch_struct,
                                ia_bit_buf_struct *it_bit_buff, WORD32 audio_object_type);

WORD32 ixheaacd_getsize_sbr_persistent();

VOID ixheaacd_set_sbr_persistent_buffers(VOID *aac_persistent_mem_v,
                                         WORD32 *persistent_used,
                                         WORD32 channels, WORD32 ps_enable);

#endif /* IXHEAACD_SBRDECODER_H */
