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
#ifndef IXHEAACD_DRC_DEC_H

#define IXHEAACD_DRC_DEC_H

#define DD_BLOCKSIZE (256)

#define ININTERBUF_SIZE 256

#define MAX_METADATA_SETS (32)

VOID ixheaacd_drc_dec_create(ia_drc_dec_struct *pstr_hdrc_dec,
                             WORD16 drc_ref_level, WORD16 drc_def_level);

WORD32 ixheaacd_dec_drc_read_element(ia_drc_dec_struct *pstr_drc_dec,
                                     ia_drc_dec_struct *drc_dummy,
                                     ia_bit_buf_struct *it_bit_buf);

WORD32 ixheaacd_drc_map_channels(ia_drc_dec_struct *drc_dec, WORD32 num_ch,
                                 WORD32 frame_size);

VOID ixheaacd_drc_apply(ia_drc_dec_struct *pstr_drc_dec,
                        WORD32 *ptr_spectral_coef, WORD32 win_seq,
                        WORD32 channel, WORD32 frame_size,
                        WORD32 esbr_flag, WORD32 audio_object_type);
#endif
