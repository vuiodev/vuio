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
#ifndef IXHEAACD_EC_STRUCT_DEF_H
#define IXHEAACD_EC_STRUCT_DEF_H

typedef struct {
  WORD32 num_sfb_long;
  WORD32 num_sfb_short;
  WORD16 *ptr_sfb_long;
  WORD16 *ptr_sfb_short;
} ia_ec_sfb_str;

typedef struct {
  WORD32 prev_sfb_nrg[MAX_SFB_EC];
  WORD32 pres_sfb_nrg[MAX_SFB_EC];
  WORD32 spec_coeff[BLOCK_LEN_LONG];
} ia_ec_scratch_str;

typedef struct {
  WORD32 spectral_coeff[BLOCK_LEN_LONG];
  WORD16 q_spec_coeff[MAX_SPEC_SCALE_LEN_EC];
  WORD32 prev_frame_ok[2];
  UWORD8 win_shape;
  UWORD8 win_shape_prev;
  WORD32 win_seq;
  WORD32 td_frame_prev;
  WORD32 fac_data_present;
  UWORD8 prev_win_group_len;
  WORD32 conceal_state;
  WORD32 prev_core_mode;
  WORD32 fade_idx;
  FLOAT32 lsf4[ORDER];
  ia_ec_sfb_str str_ec_sfb;
  ia_ec_scratch_str *pstr_ec_scratch;
  ia_ec_scratch_str str_ec_scratch;
} ia_ec_state_str;

#endif /* IXHEAACD_EC_STRUCT_DEF_H */
