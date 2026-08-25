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
#include "ixheaac_type_def.h"
#include "ixheaacd_mps_struct_def.h"
#include "ixheaacd_error_codes.h"
#include "ixheaacd_mps_res_rom.h"
#include "ixheaacd_mps_aac_struct.h"
#include "ixheaac_constants.h"
#include "ixheaacd_bitbuffer.h"
#include "ixheaacd_common_rom.h"
#include "ixheaacd_sbrdecsettings.h"
#include "ixheaacd_sbr_scale.h"
#include "ixheaacd_env_extr_part.h"
#include "ixheaacd_sbr_rom.h"
#include "ixheaacd_hybrid.h"
#include "ixheaacd_ps_dec.h"
#include "ixheaacd_mps_polyphase.h"
#include "ixheaac_error_standards.h"
#include "ixheaacd_config.h"
#include "ixheaacd_qmf_dec.h"
#include "ixheaacd_mps_dec.h"
#include "ixheaacd_mps_reshape_bb_env.h"

VOID ixheaacd_get_ch_idx(ia_heaac_mps_state_struct *pstr_mps_state, WORD32 row, WORD32 *index) {
  switch (pstr_mps_state->temp_shape_config) {
    case SHAPE_STP:
      *index = pstr_mps_state->ia_mps_dec_mps_table.m1_m2_table_ptr->idx_table
                   .row_2_channel_stp[pstr_mps_state->tree_config][row];
      break;
    case SHAPE_GES:
      *index = pstr_mps_state->ia_mps_dec_mps_table.m1_m2_table_ptr->idx_table
                   .row_2_channel_ges[pstr_mps_state->tree_config][row];
      break;
    default:
      break;
  }

  return;
}

WORD32 ixheaacd_get_res_idx(ia_heaac_mps_state_struct *pstr_mps_state, WORD32 row) {
  return pstr_mps_state->ia_mps_dec_mps_table.m1_m2_table_ptr->idx_table
      .row_2_residual[pstr_mps_state->tree_config][row];
}
