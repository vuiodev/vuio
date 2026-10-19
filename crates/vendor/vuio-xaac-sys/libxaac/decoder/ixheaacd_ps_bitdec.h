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
#ifndef IXHEAACD_PS_BITDEC_H
#define IXHEAACD_PS_BITDEC_H

#define EXTENSION_ID_PS_CODING 2

#define EXTENSION_ID_ENHSBR_CODING 3

IA_ERRORCODE ixheaacd_read_ps_data(ia_ps_dec_struct *ptr_ps_dec, ia_bit_buf_struct *it_bit_buff,
  WORD16 n_bits_left, ia_ps_tables_struct *ps_tables_ptr);

VOID ixheaacd_decode_ps_data(ia_ps_dec_struct *ptr_ps_dec, WORD32 frame_size);

extern VOID ixheaacd_map_34_params_to_20(WORD16 *a_idx);

#endif
