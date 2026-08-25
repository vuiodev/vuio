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
#include <string.h>
#include "ixheaac_type_def.h"
#include "ixheaacd_mps_struct_def.h"
#include "ixheaacd_error_codes.h"
#include "ixheaacd_mps_res_rom.h"
#include "ixheaacd_mps_aac_struct.h"
#include "ixheaac_constants.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaacd_bitbuffer.h"
#include "ixheaacd_cnst.h"
#include "ixheaacd_common_rom.h"
#include "ixheaacd_sbrdecsettings.h"
#include "ixheaacd_sbr_scale.h"
#include "ixheaacd_env_extr_part.h"
#include "ixheaacd_sbr_rom.h"
#include "ixheaacd_hybrid.h"
#include "ixheaacd_ps_dec.h"
#include "ixheaac_error_standards.h"
#include "ixheaacd_mps_polyphase.h"
#include "ixheaacd_config.h"
#include "ixheaacd_qmf_dec.h"
#include "ixheaacd_mps_dec.h"
#include "ixheaacd_mps_mdct_2_qmf.h"
#include "ixheaacd_mps_macro_def.h"
#include "ixheaacd_mps_basic_op.h"

IA_ERRORCODE ixheaacd_mdct2qmf_tables_init(ia_heaac_mps_state_struct *pstr_mps_state) {
  WORD32 time_slots;

  time_slots = pstr_mps_state->upd_qmf;

  if (32 == time_slots) {
    pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[0] =
        pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_32_00;
    pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[1] =
        pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_32_01;
    pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[2] =
        pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_32_02;
    pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[3] =
        pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_32_03;
    pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[4] =
        pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_32_04;
    pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[5] =
        pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_32_05;
    pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[6] =
        pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_32_06;
    pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[7] =
        pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_32_07;
    pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[8] =
        pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_32_08;
    pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[9] =
        pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_32_09;
    pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[10] =
        pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_32_10;
    pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[11] =
        pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_32_11;
    pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[12] =
        pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_32_12;
    pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[13] =
        pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_32_13;
    pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[14] =
        pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_32_14;
    pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[15] =
        pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_32_15;
    pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[16] =
        pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_32_15;
    pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[17] =
        pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_32_14;
    pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[18] =
        pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_32_13;
    pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[19] =
        pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_32_12;
    pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[20] =
        pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_32_11;
    pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[21] =
        pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_32_10;
    pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[22] =
        pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_32_09;
    pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[23] =
        pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_32_08;
    pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[24] =
        pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_32_07;
    pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[25] =
        pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_32_06;
    pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[26] =
        pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_32_05;
    pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[27] =
        pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_32_04;
    pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[28] =
        pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_32_03;
    pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[29] =
        pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_32_02;
    pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[30] =
        pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_32_01;
    pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[31] =
        pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_32_00;
    pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[32] =
        pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_32_16;
    pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[33] =
        pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_32_17;
    pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[34] =
        pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_32_18;
    pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[35] =
        pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_32_19;
    pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[36] =
        pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_32_20;
    pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[37] =
        pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_32_21;
    pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[38] =
        pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_32_22;
    pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[39] =
        pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_32_23;
    pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[40] =
        pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_32_24;
    pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[41] =
        pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_32_25;
    pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[42] =
        pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_32_26;
    pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[43] =
        pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_32_27;
    pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[44] =
        pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_32_28;
    pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[45] =
        pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_32_29;
    pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[46] =
        pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_32_30;
    pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[47] =
        pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_32_31;
    pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[48] =
        pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_32_31;
    pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[49] =
        pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_32_30;
    pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[50] =
        pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_32_29;
    pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[51] =
        pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_32_28;
    pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[52] =
        pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_32_27;
    pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[53] =
        pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_32_26;
    pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[54] =
        pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_32_25;
    pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[55] =
        pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_32_24;
    pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[56] =
        pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_32_23;
    pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[57] =
        pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_32_22;
    pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[58] =
        pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_32_21;
    pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[59] =
        pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_32_20;
    pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[60] =
        pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_32_19;
    pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[61] =
        pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_32_18;
    pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[62] =
        pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_32_17;
    pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[63] =
        pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_32_16;

    pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_short[0] =
        pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_short_4_00;
    pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_short[1] =
        pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_short_4_01;
    pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_short[2] =
        pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_short_4_01;
    pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_short[3] =
        pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_short_4_00;
    pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_short[4] =
        pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_short_4_02;
    pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_short[5] =
        pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_short_4_03;
    pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_short[6] =
        pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_short_4_03;
    pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_short[7] =
        pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_short_4_02;
    pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_short[8] = NULL;
    pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_short[9] = NULL;
  } else {
    pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_short[0] =
        pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_short_2_00;
    pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_short[1] =
        pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_short_2_00;
    pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_short[2] =
        pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_short_2_01;
    pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_short[3] =
        pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_short_2_01;
    pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_short[4] = NULL;
    pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_short[5] = NULL;
    pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_short[6] = NULL;
    pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_short[7] = NULL;
    pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_short[8] = NULL;
    pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_short[9] = NULL;

    if (30 == time_slots) {
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[0] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_30_00;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[1] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_30_01;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[2] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_30_02;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[3] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_30_03;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[4] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_30_04;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[5] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_30_05;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[6] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_30_06;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[7] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_30_07;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[8] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_30_08;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[9] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_30_09;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[10] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_30_10;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[11] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_30_11;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[12] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_30_12;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[13] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_30_13;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[14] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_30_14;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[15] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_30_14;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[16] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_30_13;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[17] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_30_12;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[18] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_30_11;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[19] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_30_10;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[20] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_30_09;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[21] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_30_08;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[22] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_30_07;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[23] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_30_06;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[24] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_30_05;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[25] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_30_04;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[26] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_30_03;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[27] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_30_02;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[28] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_30_01;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[29] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_30_00;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[30] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_30_15;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[31] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_30_16;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[32] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_30_17;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[33] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_30_18;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[34] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_30_19;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[35] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_30_20;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[36] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_30_21;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[37] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_30_22;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[38] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_30_23;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[39] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_30_24;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[40] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_30_25;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[41] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_30_26;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[42] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_30_27;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[43] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_30_28;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[44] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_30_29;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[45] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_30_29;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[46] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_30_28;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[47] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_30_27;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[48] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_30_26;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[49] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_30_25;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[50] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_30_24;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[51] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_30_23;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[52] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_30_22;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[53] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_30_21;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[54] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_30_20;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[55] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_30_19;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[56] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_30_18;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[57] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_30_17;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[58] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_30_16;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[59] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_30_15;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[60] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[61] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[62] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[63] = NULL;
    } else if (24 == time_slots) {
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[0] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_24_00;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[1] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_24_01;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[2] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_24_02;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[3] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_24_03;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[4] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_24_04;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[5] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_24_05;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[6] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_24_06;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[7] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_24_07;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[8] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_24_08;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[9] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_24_09;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[10] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_24_10;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[11] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_24_11;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[12] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_24_11;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[13] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_24_10;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[14] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_24_09;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[15] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_24_08;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[16] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_24_07;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[17] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_24_06;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[18] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_24_05;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[19] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_24_04;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[20] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_24_03;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[21] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_24_02;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[22] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_24_01;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[23] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_24_00;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[24] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_24_12;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[25] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_24_13;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[26] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_24_14;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[27] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_24_15;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[28] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_24_16;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[29] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_24_17;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[30] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_24_18;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[31] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_24_19;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[32] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_24_20;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[33] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_24_21;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[34] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_24_22;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[35] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_24_23;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[36] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_24_23;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[37] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_24_22;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[38] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_24_21;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[39] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_24_20;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[40] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_24_19;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[41] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_24_18;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[42] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_24_17;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[43] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_24_16;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[44] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_24_15;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[45] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_24_14;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[46] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_24_13;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[47] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_24_12;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[48] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[49] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[50] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[51] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[52] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[53] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[54] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[55] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[56] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[57] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[58] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[59] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[60] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[61] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[62] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[63] = NULL;
    } else if (18 == time_slots) {
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[0] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_18_00;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[1] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_18_01;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[2] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_18_02;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[3] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_18_03;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[4] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_18_04;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[5] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_18_05;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[6] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_18_06;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[7] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_18_07;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[8] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_18_08;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[9] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_18_08;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[10] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_18_07;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[11] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_18_06;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[12] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_18_05;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[13] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_18_04;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[14] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_18_03;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[15] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_18_02;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[16] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_18_01;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[17] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_18_00;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[18] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_18_09;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[19] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_18_10;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[20] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_18_11;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[21] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_18_12;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[22] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_18_13;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[23] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_18_14;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[24] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_18_15;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[25] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_18_16;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[26] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_18_17;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[27] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_18_17;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[28] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_18_16;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[29] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_18_15;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[30] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_18_14;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[31] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_18_13;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[32] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_18_12;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[33] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_18_11;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[34] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_18_10;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[35] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_18_09;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[36] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[37] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[38] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[39] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[40] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[41] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[42] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[43] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[44] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[45] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[46] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[47] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[48] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[49] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[50] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[51] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[52] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[53] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[54] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[55] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[56] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[57] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[58] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[59] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[60] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[61] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[62] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[63] = NULL;
    } else if (16 == time_slots) {
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[0] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_16_00;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[1] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_16_01;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[2] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_16_02;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[3] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_16_03;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[4] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_16_04;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[5] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_16_05;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[6] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_16_06;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[7] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_16_07;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[8] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_16_07;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[9] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_16_06;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[10] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_16_05;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[11] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_16_04;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[12] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_16_03;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[13] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_16_02;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[14] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_16_01;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[15] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_16_00;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[16] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_16_08;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[17] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_16_09;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[18] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_16_10;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[19] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_16_11;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[20] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_16_12;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[21] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_16_13;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[22] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_16_14;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[23] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_16_15;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[24] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_16_15;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[25] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_16_14;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[26] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_16_13;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[27] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_16_12;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[28] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_16_11;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[29] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_16_10;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[30] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_16_09;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[31] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_16_08;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[32] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[33] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[34] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[35] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[36] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[37] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[38] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[39] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[40] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[41] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[42] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[43] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[44] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[45] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[46] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[47] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[48] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[49] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[50] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[51] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[52] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[53] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[54] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[55] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[56] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[57] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[58] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[59] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[60] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[61] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[62] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[63] = NULL;
    } else if (15 == time_slots) {
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[0] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_15_00;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[1] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_15_01;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[2] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_15_02;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[3] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_15_03;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[4] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_15_04;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[5] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_15_05;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[6] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_15_06;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[7] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_15_06;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[8] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_15_05;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[9] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_15_04;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[10] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_15_03;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[11] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_15_02;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[12] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_15_01;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[13] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_15_00;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[14] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_15_07;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[15] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_15_08;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[16] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_15_09;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[17] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_15_10;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[18] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_15_11;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[19] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_15_12;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[20] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_15_13;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[21] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_15_14;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[22] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_15_14;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[23] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_15_13;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[24] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_15_12;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[25] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_15_11;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[26] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_15_10;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[27] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_15_09;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[28] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_15_08;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[29] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_long_15_07;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[30] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[31] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[32] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[33] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[34] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[35] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[36] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[37] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[38] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[39] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[40] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[41] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[42] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[43] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[44] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[45] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[46] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[47] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[48] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[49] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[50] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[51] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[52] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[53] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[54] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[55] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[56] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[57] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[58] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[59] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[60] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[61] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[62] = NULL;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_long[63] = NULL;

      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_short[4] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_short_3_00;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_short[5] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_short_3_00;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_short[6] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_short_3_01;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_short[7] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_short_3_02;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_short[8] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_short_3_02;
      pstr_mps_state->ia_mps_dec_mdct2qmfcos_table.cos_table_short[9] =
          pstr_mps_state->ia_mps_dec_mps_table.mdct2qmfcos_table_ptr->cos_table_short_3_01;
    } else {
      if (pstr_mps_state->residual_coding) return IA_XHEAAC_MPS_DEC_EXE_FATAL_INVALID_TIMESLOTS;
    }
  }

  return IA_NO_ERROR;
}

IA_ERRORCODE ixheaacd_mdct2qmf_create(ia_heaac_mps_state_struct *pstr_mps_state) {
  IA_ERRORCODE error_code = IA_NO_ERROR;

  WORD32 *qmf_residual_real = pstr_mps_state->array_struct->qmf_residual_real_pre;
  WORD32 *qmf_residual_imag = pstr_mps_state->array_struct->qmf_residual_imag_pre;

  memset(qmf_residual_real, 0, RES_CHXQMFXTSX4);
  memset(qmf_residual_imag, 0, RES_CHXQMFXTSX4);

  error_code = ixheaacd_mdct2qmf_tables_init(pstr_mps_state);

  return error_code;
}

static VOID ixheaacd_local_zero(WORD32 const l, WORD32 *const b) {
  WORD32 i;

  for (i = 0; i < l; i++) {
    b[i] = 0;
  }
}

ATTR_NO_SANITIZE_INTEGER
static VOID ixheaacd_local_fold_out(WORD32 *const s, WORD32 const lv, WORD32 *const w,
                                    WORD32 const l_w, WORD32 *const v_main,
                                    WORD32 *const v_slave) {
  WORD32 n;
  WORD32 i;
  WORD32 j;
  WORD32 k;

  WORD32 temp_1;

  WORD32 *w1;
  WORD32 *w2;
  WORD32 *w3;
  WORD32 *w4;
  WORD32 *ptr1, *ptr2, *ptr3;

  WORD32 m = l_w >> 1;
  WORD32 l = lv / m;
  WORD32 m2w = m >> 1;
  WORD32 m2a = m - m2w;

  for (i = m; i < lv; i += l_w) {
    for (n = i + l_w; i < n; i++) {
      s[i] = -s[i];
    }
  }

  w1 = &w[-m2a];
  w2 = &w[m2w];
  w3 = &w[m2w];
  w4 = &w[m + m2w];

  for (n = 0, j = 0, k = m; n < l - 1; n++) {
    for (i = 0; i < m2a; i++, j++, k++) {
      v_main[k] = ixheaacd_mps_mult32_shr_30(w2[i], s[k]);
      v_slave[j] = ixheaacd_mps_mult32_shr_30(w4[i], s[k]);
    }

    for (; i < m; i++, j++, k++) {
      v_main[j] = ixheaacd_mps_mult32_shr_30(w3[i], s[j]);
      v_slave[k] = ixheaacd_mps_mult32_shr_30(w1[i], s[j]);
    }
  }

  ptr1 = v_main;
  ptr2 = v_slave + m - 1;
  ptr3 = s;
  for (i = 0; i < m2a; i++) {
    temp_1 = *ptr3++;
    *ptr1++ = ixheaacd_mps_mult32_shr_30(*w2++, temp_1);
    *ptr2-- = ixheaacd_mps_mult32_shr_30(*w4++, temp_1);
  }

  j = l * m - m2w;
  k = l * m - m2a - 1;
  ptr3 = s + l * m - m;
  ptr1 = v_main + j;
  ptr2 = v_slave + k;

  w1 += m2a;
  for (; i < m; i++, j++) {
    temp_1 = *ptr3++;
    *ptr1++ = ixheaacd_mps_mult32_shr_30(*w2++, temp_1);
    *ptr2-- = ixheaacd_mps_mult32_shr_30(*w1++, temp_1);
  }
}

static VOID ixheaacd_local_imdet(
    WORD32 *x1, WORD32 *x2, WORD32 *const scale1, WORD32 const val, WORD32 *z_real,
    WORD32 *z_imag, const ia_mps_dec_mdct2qmf_cos_table_struct *ia_mps_dec_mdct2qmfcos_tab,
    WORD32 is_long, VOID *scratch) {
  WORD32 lw = val << 1;
  WORD32 offset = val - (val >> 1);

  WORD32 temp_1, temp3, temp_2;
  WORD32 *z_real_2, *z_imag_2;
  WORD32 *px1, *px2, *px3, *px4;
  const WORD16 *cp, *sp;
  WORD32 *scale;
  WORD32 cnt = val + (val >> 1);

  WORD32 k;
  WORD32 n, j;

  WORD32 l;

  WORD32 *p_sum = scratch;
  WORD32 *p_diff =
      (WORD32 *)scratch + IXHEAAC_GET_SIZE_ALIGNED_TYPE(SUM_SIZE, sizeof(*p_diff), BYTE_ALIGN_8);

  z_real_2 = z_real + lw;
  z_imag_2 = z_imag + lw;

  for (l = 0; l < LOOP_COUNTER; l++) {
    WORD32 *sum = p_sum;
    WORD32 *diff = p_diff;

    px1 = x1;
    px2 = x2 + val - 1;
    px3 = x2 + val;
    px4 = x1 + 2 * val - 1;

    for (n = 0; n < val; n++) {
      *sum++ = ixheaac_add32_sat(*px1, *px2);
      *sum++ = ixheaac_add32_sat(*px3, *px4);
      *diff++ = ixheaac_sub32_sat(*px1++, *px2--);
      *diff++ = ixheaac_sub32_sat(*px3++, *px4--);
    }

    scale = scale1;
    for (k = 0; k < cnt; k++) {
      if (1 == is_long) {
        cp = ia_mps_dec_mdct2qmfcos_tab->cos_table_long[k];
        sp = cp + val;
      } else {
        cp = ia_mps_dec_mdct2qmfcos_tab->cos_table_short[k + is_long];
        sp = cp + val;
      }

      sum = p_sum;
      diff = p_diff;

      temp_1 = *cp++;
      temp_2 = ixheaacd_mps_mult32_shr_15(temp_1, *sum);
      sum++;
      *z_real = temp_2;

      temp_2 = ixheaacd_mps_mult32_shr_15(temp_1, *sum);
      sum++;
      *z_real_2 = temp_2;

      temp_1 = *--sp;
      temp_2 = ixheaacd_mps_mult32_shr_15(temp_1, *diff);
      diff++;
      *z_imag = temp_2;

      temp_2 = ixheaacd_mps_mult32_shr_15(temp_1, *diff);
      diff++;
      *z_imag_2 = temp_2;
      for (n = 1; n < val; n++) {
        temp_1 = *cp++;
        temp_2 = ixheaacd_mps_mult32_shr_15(temp_1, *sum);
        sum++;
        *z_real = ixheaac_add32_sat(*z_real, temp_2);

        temp_2 = ixheaacd_mps_mult32_shr_15(temp_1, *sum);
        sum++;
        *z_real_2 = ixheaac_add32_sat(*z_real_2, temp_2);

        temp_1 = *--sp;
        temp_2 = ixheaacd_mps_mult32_shr_15(temp_1, *diff);
        diff++;
        *z_imag = ixheaac_add32_sat(*z_imag, temp_2);

        temp_2 = ixheaacd_mps_mult32_shr_15(temp_1, *diff);
        diff++;
        *z_imag_2 = ixheaac_add32_sat(*z_imag_2, temp_2);
      }
      z_real++;
      z_imag++;
      z_real_2++;
      z_imag_2++;
    }
    z_real -= cnt;
    z_real_2 -= cnt;
    z_imag -= cnt;
    z_imag_2 -= cnt;

    for (j = 0; j < (cnt); j++) {
      *z_real = ixheaacd_mps_mult32_shr_15(*z_real, *scale);
      z_real++;
      *z_imag = ixheaacd_mps_mult32_shr_15(*z_imag, *scale);
      z_imag++;
      *z_real_2 = ixheaacd_mps_mult32_shr_15(*z_real_2, *scale);
      z_real_2++;
      *z_imag_2 = ixheaacd_mps_mult32_shr_15(*z_imag_2, *scale);
      scale++;
      z_imag_2++;
    }

    for (; k < lw; k++) {
      if (1 == is_long) {
        cp = ia_mps_dec_mdct2qmfcos_tab->cos_table_long[k];
        sp = cp + val;
      } else {
        cp = ia_mps_dec_mdct2qmfcos_tab->cos_table_short[k + is_long];
        sp = cp + val;
      }

      sum = p_sum;
      diff = p_diff;

      temp_1 = *cp++;

      temp3 = ixheaacd_mps_mult32_shr_15(temp_1, *sum);
      sum++;
      *z_real = temp3;

      temp3 = ixheaacd_mps_mult32_shr_15(temp_1, *sum);
      sum++;
      *z_real_2 = temp3;

      temp_1 = *--sp;
      temp3 = ixheaacd_mps_mult32_shr_15(temp_1, *diff);
      diff++;
      *z_imag = temp3;

      temp3 = ixheaacd_mps_mult32_shr_15(temp_1, *diff);
      diff++;
      *z_imag_2 = temp3;

      for (n = 1; n < val; n++) {
        temp_1 = *cp++;
        temp3 = ixheaacd_mps_mult32_shr_15(temp_1, *sum);
        sum++;
        *z_real = ixheaac_add32_sat(*z_real, temp3);

        temp3 = ixheaacd_mps_mult32_shr_15(temp_1, *sum);
        sum++;
        *z_real_2 = ixheaac_add32_sat(*z_real_2, temp3);

        temp_1 = *--sp;
        temp3 = ixheaacd_mps_mult32_shr_15(temp_1, *diff);
        diff++;
        *z_imag = ixheaac_add32_sat(*z_imag, temp3);

        temp3 = ixheaacd_mps_mult32_shr_15(temp_1, *diff);
        diff++;
        *z_imag_2 = ixheaac_add32_sat(*z_imag_2, temp3);
      }

      z_real++;
      z_imag++;
      z_real_2++;
      z_imag_2++;
    }
    z_real -= offset;
    z_real_2 -= offset;
    z_imag -= offset;
    z_imag_2 -= offset;

    for (j = 0; j < (offset); j++) {
      temp_1 = (*scale++) * -1;
      *z_real = ixheaacd_mps_mult32_shr_15(*z_real, temp_1);
      z_real++;
      *z_imag = ixheaacd_mps_mult32_shr_15(*z_imag, temp_1);
      z_imag++;
      *z_real_2 = ixheaacd_mps_mult32_shr_15(*z_real_2, temp_1);
      z_real_2++;
      *z_imag_2 = ixheaacd_mps_mult32_shr_15(*z_imag_2, temp_1);
      z_imag_2++;
    }

    x1 += lw;
    x2 += lw;

    z_real += lw;
    z_imag += lw;
    z_imag_2 += lw;
    z_real_2 += lw;
  }
}

static VOID ixheaacd_local_hybcmdct2qmf(
    WORD32 *const v_main, WORD32 *const v_slave, WORD32 *const w, WORD32 const lw, WORD32 *z_real,
    WORD32 *z_imag, const ia_mps_dec_mdct2qmf_cos_table_struct *ia_mps_dec_mdct2qmfcos_tab,
    VOID *scratch, WORD32 is_long) {
  WORD32 i, start = 0;
  WORD32 m = lw >> 1;

  switch (lw) {
    case TSX2_4:
    case TSX2_6:
    case TSX2_30:
    case TSX2_36:
    case TSX2_60:
      start = 1;
      break;

    case TSX2_8:
    case TSX2_32:
    case TSX2_48:
    case TSX2_64:
      start = 0;
      break;

    default:
      break;
  }

  ixheaacd_local_imdet(v_slave, v_main, w, m, z_real, z_imag, ia_mps_dec_mdct2qmfcos_tab, is_long,
                       scratch);

  for (i = start; i < (m << 7); i += 2) {
    z_imag[i] = -(z_imag[i]);
  }

  return;
}

static VOID ixheaacd_local_p_zero_ts15(WORD32 *const b, WORD32 *src, WORD32 l) {
  WORD32 i;

  for (i = 0; i < 15; i++) {
    b[i] = *src++;
  }
  if (l != 15) {
    src--;
    for (; i < l; i++) {
      b[i] = *--src;
    }
  }
}

static VOID ixheaacd_local_p_nonzero(WORD32 *const b, WORD32 *src, WORD32 l) {
  WORD32 i;

  for (i = 0; i < l; i++) {
    b[i] = *src--;
  }
}

static VOID ixheaacd_local_p_zero(WORD32 *const b, WORD32 *src, WORD32 l, WORD32 upd_qmf) {
  WORD32 i;

  for (i = 0; i < upd_qmf; i++) {
    b[i] = *src++;
  }
  if (l != upd_qmf) {
    for (; i < l; i++) {
      b[i] = *--src;
    }
  }
}

static VOID ixheaacd_local_sin(WORD32 const t, WORD32 const p, WORD32 const l, WORD32 *const b,
                               ia_mps_dec_mdct2qmf_table_struct *mdct2qmf_tab) {
  WORD32 *sin_ptr;

  switch (t) {
    case TS_2:
      switch (p) {
        case ZERO:
          b[0] = 12540;
          b[1] = 30274;
          if (l == TSX2_4) {
            b[2] = b[1];
            b[3] = b[0];
          }
          break;
        case TS_2:
          b[0] = 30274;
          b[1] = 12540;
          break;
        default:
          break;
      }
      break;
    case TS_4:
      switch (p) {
        case ZERO:
          sin_ptr = mdct2qmf_tab->local_sin_4;
          b[0] = *sin_ptr++;
          b[1] = *sin_ptr++;
          b[2] = *sin_ptr++;
          b[3] = *sin_ptr;
          if (l == TSX2_8) {
            b[4] = *sin_ptr--;
            b[5] = *sin_ptr--;
            b[6] = *sin_ptr--;
            b[7] = *sin_ptr;
          }
          break;
        case TS_4:
          sin_ptr = &(mdct2qmf_tab->local_sin_4[TS_MINUS_ONE_4]);
          b[0] = *sin_ptr--;
          b[1] = *sin_ptr--;
          b[2] = *sin_ptr--;
          b[3] = *sin_ptr;
          break;
        default:
          break;
      }
      break;
    case UPD_QMF_15:
      switch (p) {
        case ZERO:
          sin_ptr = &(mdct2qmf_tab->local_sin_15[1]);
          ixheaacd_local_p_zero_ts15(b, sin_ptr, l);
          break;
        case UPD_QMF_15:
          sin_ptr = &(mdct2qmf_tab->local_sin_15[TS_MINUS_ONE_15]);
          ixheaacd_local_p_nonzero(b, sin_ptr, l);
          break;
        default:
          break;
      }
      break;
    case UPD_QMF_16:
      switch (p) {
        case ZERO:
          sin_ptr = mdct2qmf_tab->local_sin_16;
          ixheaacd_local_p_zero(b, sin_ptr, l, UPD_QMF_16);
          break;
        case UPD_QMF_16:
          sin_ptr = &(mdct2qmf_tab->local_sin_16[TS_MINUS_ONE_16]);
          ixheaacd_local_p_nonzero(b, sin_ptr, l);
          break;
        default:
          break;
      }
      break;
    case UPD_QMF_18:
      switch (p) {
        case ZERO:
          sin_ptr = mdct2qmf_tab->local_sin_18;
          ixheaacd_local_p_zero(b, sin_ptr, l, UPD_QMF_18);
          break;
        case UPD_QMF_18:
          sin_ptr = &(mdct2qmf_tab->local_sin_18[TS_MINUS_ONE_18]);
          ixheaacd_local_p_nonzero(b, sin_ptr, l);
          break;
        default:
          break;
      }
      break;
    case UPD_QMF_24:
      switch (p) {
        case ZERO:
          sin_ptr = mdct2qmf_tab->local_sin_24;
          ixheaacd_local_p_zero(b, sin_ptr, l, UPD_QMF_24);
          break;
        case UPD_QMF_24:
          sin_ptr = &(mdct2qmf_tab->local_sin_24[TS_MINUS_ONE_24]);
          ixheaacd_local_p_nonzero(b, sin_ptr, l);
          break;
        default:
          break;
      }
      break;
    case UPD_QMF_30:
      switch (p) {
        case ZERO:
          sin_ptr = mdct2qmf_tab->local_sin_30;
          ixheaacd_local_p_zero(b, sin_ptr, l, UPD_QMF_30);
          break;
        case UPD_QMF_30:
          sin_ptr = &(mdct2qmf_tab->local_sin_30[TS_MINUS_ONE_30]);
          ixheaacd_local_p_nonzero(b, sin_ptr, l);
          break;
        default:
          break;
      }
      break;
    case UPD_QMF_32:
      switch (p) {
        case ZERO:
          sin_ptr = mdct2qmf_tab->local_sin_32;
          ixheaacd_local_p_zero(b, sin_ptr, l, UPD_QMF_32);
          break;
        case UPD_QMF_32:
          sin_ptr = &(mdct2qmf_tab->local_sin_32[TS_MINUS_ONE_32]);
          ixheaacd_local_p_nonzero(b, sin_ptr, l);

          break;
        default:
          break;
      }
      break;
    default:
      break;
  }
  return;
}

static VOID ixheaacd_local_one(WORD32 const l, WORD32 *const b) {
  WORD32 i;

  for (i = 0; i < l; i++) {
    b[i] = ONE_IN_Q15;
  }
}

static VOID ixheaacd_local_freq_win(WORD32 const l, WORD32 *const b, const WORD32 **wf) {
  WORD32 i;
  WORD32 odd_length = (l & ONE_BIT_MASK);
  WORD32 temp_1;
  WORD32 *b_start, *b_end;
  b_start = b;
  b_end = b + 2 * l - 1 - odd_length;

  for (i = 0; i < l - odd_length; i++) {
    temp_1 = wf[l - 1][i];
    *b_start++ = temp_1;
    *b_end-- = temp_1;
  }

  if (odd_length == 1) {
    *b_start = wf[l - 1][l - 1];
    *(b_start + l) = 0;
  }
}

static VOID ixheaacd_local_mdct_win(WORD32 upd_qmf, WORD32 const window_type, WORD32 *const wf,
                                    WORD32 const **wf_tab, WORD32 *const wt,
                                    ia_mps_dec_mdct2qmf_table_struct *mdct2qmf_tab) {
  WORD32 length = 0;
  WORD32 length_right = 0;
  WORD32 length_left = 0;
  WORD32 length_const1 = 0;
  WORD32 length_const2 = 0;

  switch (window_type) {
    case ONLY_LONG_SEQUENCE:
      length = upd_qmf;
      ixheaacd_local_sin(upd_qmf, 0, (length << 1), &wt[0], mdct2qmf_tab);
      break;
    case LONG_START_SEQUENCE:
      switch (upd_qmf) {
        case UPD_QMF_15:
          length_const1 = 6;
          length_const2 = 7;
          length_right = 2;
          break;
        case UPD_QMF_16:
        case UPD_QMF_32:
          length_const1 = 7 * (upd_qmf >> 4);
          length_const2 = length_const1;
          length_right = upd_qmf >> 3;
          break;
        case UPD_QMF_18:
          length_const1 = 8;
          length_const2 = length_const1;
          length_right = 2;
          break;
        case UPD_QMF_24:
          length_const1 = 11;
          length_const2 = length_const1;
          length_right = 2;
          break;
        case UPD_QMF_30:
          length_const1 = 14;
          length_const2 = length_const1;
          length_right = 2;
          break;
        default:
          length_const1 = 6;
          length_const2 = 7;
          length_right = 2;
          break;
      }

      ixheaacd_local_sin(upd_qmf, 0, upd_qmf, &wt[0], mdct2qmf_tab);
      ixheaacd_local_one(length_const1, &wt[upd_qmf]);
      ixheaacd_local_sin(length_right, length_right, length_right, &wt[upd_qmf + length_const1],
                         mdct2qmf_tab);

      ixheaacd_local_zero(length_const2, &wt[upd_qmf + length_const1 + length_right]);
      length = upd_qmf;
      break;
    case EIGHT_SHORT_SEQUENCE:
      switch (upd_qmf) {
        case UPD_QMF_16:
        case UPD_QMF_32:
          length = upd_qmf >> 3;
          break;
        case UPD_QMF_15:
        case UPD_QMF_18:
        case UPD_QMF_30:
        case UPD_QMF_24:
          length = 2;
          break;
        default:
          break;
      }
      ixheaacd_local_sin(length, 0, 2 * length, &wt[0], mdct2qmf_tab);
      break;
    case LONG_STOP_SEQUENCE:
      switch (upd_qmf) {
        case UPD_QMF_15:
          length_const1 = 6;
          length_const2 = 7;
          length_left = 2;
          break;
        case UPD_QMF_16:
        case UPD_QMF_32:
          length_const1 = 7 * (upd_qmf >> 4);
          length_const2 = length_const1;
          length_left = upd_qmf >> 3;
          break;
        case UPD_QMF_18:
          length_const1 = 8;
          length_const2 = length_const1;
          length_left = 2;
          break;
        case UPD_QMF_24:
          length_const1 = 11;
          length_const2 = length_const1;
          length_left = 2;
          break;
        case UPD_QMF_30:
          length_const1 = 14;
          length_const2 = length_const1;
          length_left = 2;
          break;
        default:
          break;
      }

      ixheaacd_local_zero(length_const1, &wt[0]);
      ixheaacd_local_sin(length_left, 0, length_left, &wt[length_const1], mdct2qmf_tab);
      ixheaacd_local_one(length_const2, &wt[length_const1 + length_left]);
      ixheaacd_local_sin(upd_qmf, upd_qmf, upd_qmf,
                         &wt[length_const1 + length_left + length_const2], mdct2qmf_tab);
      length = upd_qmf;
      break;
    default:
      break;
  }

  ixheaacd_local_freq_win(length, &wf[0], wf_tab);

  if ((upd_qmf == UPD_QMF_15) && (window_type == EIGHT_SHORT_SEQUENCE)) {
    WORD32 length2 = 3;

    ixheaacd_local_sin(length, 0, length, &wt[(length << 1)], mdct2qmf_tab);
    ixheaacd_local_one(1, &wt[3 * length]);
    ixheaacd_local_sin(length, length, length, &wt[3 * length + 1], mdct2qmf_tab);
    ixheaacd_local_zero(1, &wt[(length << 2) + 1]);

    ixheaacd_local_freq_win(length2, &wf[(length << 1)], wf_tab);
  }
  return;
}

static VOID ixheaacd_get_gain(WORD32 l, WORD32 *gain) {
  switch (l) {
    case TSX2_4:
      *gain = 16384;
      break;

    case TSX2_6:
      *gain = 13377;
      break;

    case TSX2_8:
      *gain = 11585;
      break;

    case TSX2_30:
      *gain = 5982;
      break;

    case TSX2_32:
      *gain = 5792;
      break;

    case TSX2_36:
      *gain = 5461;
      break;

    case TSX2_48:
      *gain = 4729;
      break;

    case TSX2_60:
      *gain = 4230;
      break;

    case TSX2_64:
      *gain = 4096;
      break;

    default:
      break;
  }

  return;
}

VOID ixheaacd_mdct2qmf_process(WORD32 upd_qmf, WORD32 *const mdct_in, WORD32 *qmf_real_pre,
                               WORD32 *qmf_real_post, WORD32 *qmf_imag_pre, WORD32 *qmf_imag_post,
                               WORD32 const window_type, WORD32 qmf_global_offset,
                               ia_mps_dec_mps_tables_struct *ia_mps_dec_mps_table_ptr,
                               VOID *scratch, WORD32 time_slots) {
  WORD32 i;
  WORD32 j;
  WORD32 k;

  WORD32 l = (upd_qmf << 1);

  WORD32 n = 0;
  WORD32 *wf;
  WORD32 *wt;

  WORD32 *v1;
  WORD32 *v2;

  WORD32 *z1_real;
  WORD32 *z1_imag;

  WORD32 *twipost_real;
  WORD32 *twipost_imag;

  WORD32 *mdct_sf, *mdct_sf_ptr;
  WORD32 *mdct_p, *mdct_p2;

  WORD32 temp_2, temp3, temp4;

  WORD32 *p_qmf_real_pre = qmf_real_pre;
  WORD32 *p_qmf_real_post = qmf_real_post;
  WORD32 *p_qmf_imag_pre = qmf_imag_pre;
  WORD32 *p_qmf_imag_post = qmf_imag_post;

  VOID *free_scratch;

  WORD32 const **wf_tab = ia_mps_dec_mps_table_ptr->wf_tab_ptr->wf;

  WORD32 window_offset = 0;

  WORD32 mdct_offset = 0;
  WORD32 mdct_shift = AAC_SHORT_FRAME_LENGTH;

  WORD32 qmf_offset = 0;
  WORD32 qmf_shift = 0;

  WORD32 n_windows = 0;

  WORD32 mdct_length = MDCT_LENGTH_LO;

  WORD32 qmf_bands = 64;
  const WORD32 *ptr1, *ptr2;
  WORD32 is_long;
  WORD32 *zr, *zi;

  WORD32 *a, *scale;
  WORD32 gain = 0;
  WORD32 *wp;
  WORD64 temp_prod = 0;

  wf = scratch;
  wt = wf + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_TIMESLOTSX2, sizeof(*wt), BYTE_ALIGN_8);
  v1 = wt + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_TIMESLOTSX2, sizeof(*v1), BYTE_ALIGN_8);
  v2 = v1 + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MDCT_LENGTH_HI, sizeof(*v2), BYTE_ALIGN_8);
  twipost_real =
      v2 + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MDCT_LENGTH_HI, sizeof(*twipost_real), BYTE_ALIGN_8);
  twipost_imag = twipost_real + IXHEAAC_GET_SIZE_ALIGNED_TYPE(
                                    MAX_NUM_QMF_BANDS, sizeof(*twipost_imag), BYTE_ALIGN_8);
  mdct_sf = twipost_imag +
            IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_NUM_QMF_BANDS, sizeof(*mdct_sf), BYTE_ALIGN_8);
  z1_real =
      mdct_sf + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MDCT_LENGTH_SF, sizeof(*z1_real), BYTE_ALIGN_8);
  z1_imag = z1_real + IXHEAAC_GET_SIZE_ALIGNED_TYPE(QBXTSX2, sizeof(*z1_imag), BYTE_ALIGN_8);
  a = z1_imag + IXHEAAC_GET_SIZE_ALIGNED_TYPE(QBXTSX2, sizeof(*a), BYTE_ALIGN_8);
  free_scratch = (VOID *)((WORD32 *)a + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_NUM_QMF_BANDS,
                                                                      sizeof(*a), BYTE_ALIGN_8));

  scale = a;

  ixheaacd_local_mdct_win(upd_qmf, window_type, wf, wf_tab, wt,
                          ia_mps_dec_mps_table_ptr->mdct2qmf_table_ptr);

  switch (window_type) {
    case ONLY_LONG_SEQUENCE:
    case LONG_START_SEQUENCE:
    case LONG_STOP_SEQUENCE:

      n = upd_qmf * qmf_bands - MDCT_LENGTH_LO;

      if (n > 0) {
        ixheaacd_local_zero(n, &mdct_in[MDCT_LENGTH_LO]);
      }
      mdct_length += n;

      ixheaacd_local_fold_out(mdct_in, mdct_length, wf, l, v1, v2);

      wp = wt;

      ixheaacd_get_gain(l, &gain);

      for (k = 0; k < l; k++) {
        *scale++ = ixheaacd_mps_mult32_shr_15(gain, *wp);
        wp++;
      }
      ixheaacd_local_hybcmdct2qmf(v1, v2, a, l, z1_real, z1_imag,
                                  ia_mps_dec_mps_table_ptr->mdct2qmfcos_tab_ptr, free_scratch, 1);

      ptr1 = ia_mps_dec_mps_table_ptr->mdct2qmf_table_ptr->twi_post_cos;
      ptr2 = ia_mps_dec_mps_table_ptr->mdct2qmf_table_ptr->twi_post_sin;

      if (qmf_global_offset < time_slots) {
        if (qmf_global_offset + l < time_slots) {
          for (i = 0; i < qmf_bands; i++) {
            WORD32 cos_twi = *ptr1++;
            WORD32 sin_twi = *ptr2++;
            for (j = 0; j < l; j++) {
              temp3 = *z1_real++;
              temp4 = *z1_imag++;

              temp_2 = j + qmf_global_offset;
              temp_prod = (WORD64)(p_qmf_real_pre[temp_2]);
              temp_prod = ixheaac_mac32x32in64(temp_prod, cos_twi, temp3);
              temp_prod = ixheaac_mac32x32in64(temp_prod, -sin_twi, temp4);
              p_qmf_real_pre[temp_2] = ixheaac_sat64_32(temp_prod);
              temp_prod = (WORD64)(p_qmf_imag_pre[temp_2]);
              temp_prod = ixheaac_mac32x32in64(temp_prod, cos_twi, temp4);
              temp_prod = ixheaac_mac32x32in64(temp_prod, sin_twi, temp3);
              p_qmf_imag_pre[temp_2] = ixheaac_sat64_32(temp_prod);
            }
            p_qmf_real_pre += MAX_TIME_SLOTS;
            p_qmf_imag_pre += MAX_TIME_SLOTS;
          }
        } else {
          for (i = 0; i < qmf_bands; i++) {
            WORD32 cos_twi = *ptr1++;
            WORD32 sin_twi = *ptr2++;
            for (j = 0; j < l; j++) {
              temp3 = *z1_real++;
              temp4 = *z1_imag++;

              temp_2 = j + qmf_global_offset;
              if (temp_2 < time_slots) {
                temp_prod = (WORD64)(p_qmf_real_pre[temp_2]);
                temp_prod = ixheaac_mac32x32in64(temp_prod, cos_twi, temp3);
                temp_prod = ixheaac_mac32x32in64(temp_prod, -sin_twi, temp4);
                p_qmf_real_pre[temp_2] = ixheaac_sat64_32(temp_prod);
                temp_prod = (WORD64)(p_qmf_imag_pre[temp_2]);
                temp_prod = ixheaac_mac32x32in64(temp_prod, cos_twi, temp4);
                temp_prod = ixheaac_mac32x32in64(temp_prod, sin_twi, temp3);
                p_qmf_imag_pre[temp_2] = ixheaac_sat64_32(temp_prod);
              } else {
                temp_prod = (WORD64)(p_qmf_real_post[temp_2 - time_slots]);
                temp_prod = ixheaac_mac32x32in64(temp_prod, cos_twi, temp3);
                temp_prod = ixheaac_mac32x32in64(temp_prod, -sin_twi, temp4);
                p_qmf_real_post[temp_2 - time_slots] = ixheaac_sat64_32(temp_prod);
                temp_prod = (WORD64)(p_qmf_imag_post[temp_2 - time_slots]);
                temp_prod = ixheaac_mac32x32in64(temp_prod, cos_twi, temp4);
                temp_prod = ixheaac_mac32x32in64(temp_prod, sin_twi, temp3);
                p_qmf_imag_post[temp_2 - time_slots] = ixheaac_sat64_32(temp_prod);
              }
            }
            p_qmf_real_pre += MAX_TIME_SLOTS;
            p_qmf_real_post += MAX_TIME_SLOTS;

            p_qmf_imag_pre += MAX_TIME_SLOTS;
            p_qmf_imag_post += MAX_TIME_SLOTS;
          }
        }
      } else {
        for (i = 0; i < qmf_bands; i++) {
          WORD32 cos_twi = *ptr1++;
          WORD32 sin_twi = *ptr2++;
          for (j = 0; j < l; j++) {
            temp3 = *z1_real++;
            temp4 = *z1_imag++;

            temp_2 = j + qmf_global_offset;

            temp_prod = (WORD64)(p_qmf_real_post[temp_2 - time_slots]);
            temp_prod = ixheaac_mac32x32in64(temp_prod, cos_twi, temp3);
            temp_prod = ixheaac_mac32x32in64(temp_prod, -sin_twi, temp4);
            p_qmf_real_post[temp_2 - time_slots] = ixheaac_sat64_32(temp_prod);
            temp_prod = (WORD64)(p_qmf_imag_post[temp_2 - time_slots]);
            temp_prod = ixheaac_mac32x32in64(temp_prod, cos_twi, temp4);
            temp_prod = ixheaac_mac32x32in64(temp_prod, sin_twi, temp3);
            p_qmf_imag_post[temp_2 - time_slots] = ixheaac_sat64_32(temp_prod);
          }
          p_qmf_real_post += MAX_TIME_SLOTS;
          p_qmf_imag_post += MAX_TIME_SLOTS;
        }
      }
      break;
    case EIGHT_SHORT_SEQUENCE:

      switch (upd_qmf) {
        case UPD_QMF_15:
          l = 4;
          mdct_length = AAC_SHORT_FRAME_LENGTH;
          qmf_offset = 6;
          qmf_shift = 2;
          n_windows = 7;
          break;
        case UPD_QMF_16:
        case UPD_QMF_32:
          n = (upd_qmf - UPD_QMF_16) * 8;
          mdct_length = AAC_SHORT_FRAME_LENGTH + n;
          l = 2 * (upd_qmf >> 3);
          qmf_offset = 7 * upd_qmf >> 4;
          qmf_shift = upd_qmf >> 3;
          n_windows = 8;
          break;
        case UPD_QMF_18:
          l = 4;
          mdct_length = AAC_SHORT_FRAME_LENGTH;
          qmf_offset = 8;
          qmf_shift = 2;
          n_windows = 9;
          break;
        case UPD_QMF_24:
          l = 4;
          mdct_length = AAC_SHORT_FRAME_LENGTH;
          qmf_offset = 11;
          qmf_shift = 2;
          n_windows = 12;
          break;
        case UPD_QMF_30:
          l = 4;
          mdct_length = AAC_SHORT_FRAME_LENGTH;
          qmf_offset = 14;
          qmf_shift = 2;
          n_windows = 15;
          break;
        default:
          l = 4;
          mdct_length = AAC_SHORT_FRAME_LENGTH;
          qmf_offset = 6;
          qmf_shift = 2;
          n_windows = 7;
          break;
      }

      wp = wt;
      ixheaacd_get_gain(l, &gain);

      for (k = 0; k < l; k++) {
        *scale++ = ixheaacd_mps_mult32_shr_15(gain, *wp);
        wp++;
      }

      for (k = 0; k < n_windows; k++) {
        is_long = 0;
        mdct_sf_ptr = mdct_sf;
        switch (upd_qmf) {
          case UPD_QMF_16:
          case UPD_QMF_32:
            mdct_p = mdct_in + mdct_offset;
            for (i = 0; i < AAC_SHORT_FRAME_LENGTH; i++) {
              *mdct_sf_ptr++ = *mdct_p++;
            }

            ixheaacd_local_zero(n, &mdct_sf[AAC_SHORT_FRAME_LENGTH]);
            break;
          case UPD_QMF_15:
            if (k < n_windows - 1) {
              mdct_p = mdct_in + mdct_offset;
              for (i = 0; i < AAC_SHORT_FRAME_LENGTH; i++) {
                *mdct_sf_ptr++ = *mdct_p++;
              }
            } else {
              window_offset = l;
              l = 6;
              mdct_length = 192;
              is_long = 4;
              gain = 13377;
              scale = a;
              wp = wt + window_offset;

              for (k = 0; k < l; k++) {
                *scale++ = ixheaacd_mps_mult32_shr_15(gain, *wp);
                wp++;
              }

              mdct_p = mdct_in + mdct_offset;
              mdct_p2 = mdct_in + mdct_offset + AAC_SHORT_FRAME_LENGTH;

              for (i = 0, j = 0; i < mdct_length / 2; i++) {
                *mdct_sf_ptr++ = *mdct_p++;
                *mdct_sf_ptr++ = *mdct_p2++;
              }
            }
            break;
          case UPD_QMF_18:
          case UPD_QMF_24:
          case UPD_QMF_30:

            mdct_p = mdct_in + mdct_offset;
            for (i = 0; i < AAC_SHORT_FRAME_LENGTH; i++) {
              *mdct_sf_ptr++ = *mdct_p++;
            }
            break;
          default:
            break;
        }
        ixheaacd_local_fold_out(mdct_sf, mdct_length, &wf[window_offset], l, v1, v2);

        ixheaacd_local_hybcmdct2qmf(v1, v2, a, l, z1_real, z1_imag,
                                    ia_mps_dec_mps_table_ptr->mdct2qmfcos_tab_ptr, free_scratch,
                                    is_long);

        zr = z1_real;
        zi = z1_imag;

        ptr1 = ia_mps_dec_mps_table_ptr->mdct2qmf_table_ptr->twi_post_cos;
        ptr2 = ia_mps_dec_mps_table_ptr->mdct2qmf_table_ptr->twi_post_sin;
        temp_2 = qmf_offset + qmf_global_offset;

        if (temp_2 < time_slots) {
          if ((temp_2 + l) < time_slots) {
            p_qmf_real_pre = qmf_real_pre;
            p_qmf_imag_pre = qmf_imag_pre;

            for (i = 0; i < qmf_bands; i++) {
              WORD32 cos_twi = *ptr1++;
              WORD32 sin_twi = *ptr2++;

              for (j = 0; j < l; j++) {
                temp3 = *zr++;
                temp4 = (*zi++);

                temp_prod = (WORD64)(p_qmf_real_pre[temp_2 + j]);
                temp_prod = ixheaac_mac32x32in64(temp_prod, cos_twi, temp3);
                temp_prod = ixheaac_mac32x32in64(temp_prod, -sin_twi, temp4);
                p_qmf_real_pre[temp_2 + j] = ixheaac_sat64_32(temp_prod);
                temp_prod = (WORD64)(p_qmf_imag_pre[temp_2 + j]);
                temp_prod = ixheaac_mac32x32in64(temp_prod, cos_twi, temp4);
                temp_prod = ixheaac_mac32x32in64(temp_prod, sin_twi, temp3);
                p_qmf_imag_pre[temp_2 + j] = ixheaac_sat64_32(temp_prod);
              }
              p_qmf_real_pre += MAX_TIME_SLOTS;
              p_qmf_imag_pre += MAX_TIME_SLOTS;
            }
          } else {
            p_qmf_real_pre = qmf_real_pre;
            p_qmf_real_post = qmf_real_post;
            p_qmf_imag_pre = qmf_imag_pre;
            p_qmf_imag_post = qmf_imag_post;

            for (i = 0; i < qmf_bands; i++) {
              WORD32 cos_twi = *ptr1++;
              WORD32 sin_twi = *ptr2++;

              for (j = 0; j < l; j++) {
                temp3 = *zr++;
                temp4 = (*zi++);

                if ((temp_2 + j) < time_slots) {
                  temp_prod = (WORD64)(p_qmf_real_pre[temp_2 + j]);
                  temp_prod = ixheaac_mac32x32in64(temp_prod, cos_twi, temp3);
                  temp_prod = ixheaac_mac32x32in64(temp_prod, -sin_twi, temp4);
                  p_qmf_real_pre[temp_2 + j] = ixheaac_sat64_32(temp_prod);
                  temp_prod = (WORD64)(p_qmf_imag_pre[temp_2 + j]);
                  temp_prod = ixheaac_mac32x32in64(temp_prod, cos_twi, temp4);
                  temp_prod = ixheaac_mac32x32in64(temp_prod, sin_twi, temp3);
                  p_qmf_imag_pre[temp_2 + j] = ixheaac_sat64_32(temp_prod);
                } else {
                  temp_prod = (WORD64)(p_qmf_real_post[temp_2 + j - time_slots]);
                  temp_prod = ixheaac_mac32x32in64(temp_prod, cos_twi, temp3);
                  temp_prod = ixheaac_mac32x32in64(temp_prod, -sin_twi, temp4);
                  p_qmf_real_post[temp_2 + j - time_slots] = ixheaac_sat64_32(temp_prod);
                  temp_prod = (WORD64)(p_qmf_imag_post[temp_2 + j - time_slots]);
                  temp_prod = ixheaac_mac32x32in64(temp_prod, cos_twi, temp4);
                  temp_prod = ixheaac_mac32x32in64(temp_prod, sin_twi, temp3);
                  p_qmf_imag_post[temp_2 + j - time_slots] = ixheaac_sat64_32(temp_prod);
                }
              }
              p_qmf_real_pre += MAX_TIME_SLOTS;
              p_qmf_imag_pre += MAX_TIME_SLOTS;

              p_qmf_real_post += MAX_TIME_SLOTS;
              p_qmf_imag_post += MAX_TIME_SLOTS;
            }
          }
        } else {
          p_qmf_real_post = qmf_real_post;
          p_qmf_imag_post = qmf_imag_post;

          for (i = 0; i < qmf_bands; i++) {
            WORD32 cos_twi = *ptr1++;
            WORD32 sin_twi = *ptr2++;

            for (j = 0; j < l; j++) {
              temp3 = *zr++;
              temp4 = (*zi++);
              {
                temp_prod = (WORD64)(p_qmf_real_post[temp_2 + j - time_slots]);
                temp_prod = ixheaac_mac32x32in64(temp_prod, cos_twi, temp3);
                temp_prod = ixheaac_mac32x32in64(temp_prod, -sin_twi, temp4);
                p_qmf_real_post[temp_2 + j - time_slots] = ixheaac_sat64_32(temp_prod);
                temp_prod = (WORD64)(p_qmf_imag_post[temp_2 + j - time_slots]);
                temp_prod = ixheaac_mac32x32in64(temp_prod, cos_twi, temp4);
                temp_prod = ixheaac_mac32x32in64(temp_prod, sin_twi, temp3);
                p_qmf_imag_post[temp_2 + j - time_slots] = ixheaac_sat64_32(temp_prod);
              }
            }
            p_qmf_real_post += MAX_TIME_SLOTS;
            p_qmf_imag_post += MAX_TIME_SLOTS;
          }
        }

        mdct_offset += mdct_shift;
        qmf_offset += qmf_shift;
      }
      break;
    default:
      break;
  }
  return;
}
