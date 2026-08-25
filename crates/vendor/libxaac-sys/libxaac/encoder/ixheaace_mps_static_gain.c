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

#include <math.h>
#include <stdlib.h>
#include <string.h>
#include "ixheaac_type_def.h"
#include "ixheaac_error_standards.h"
#include "ixheaace_error_codes.h"
#include "ixheaace_mps_common_fix.h"
#include "ixheaace_mps_defines.h"
#include "ixheaace_mps_common_define.h"
#include "ixheaace_bitbuffer.h"

#include "ixheaace_mps_static_gain.h"
#include "ixheaace_mps_bitstream.h"
#include "ixheaace_mps_struct_def.h"
#include "ixheaace_mps_sac_polyphase.h"
#include "ixheaace_mps_sac_nlc_enc.h"
#include "ixheaace_mps_sac_hybfilter.h"
#include "ixheaace_mps_spatial_bitstream.h"
#include "ixheaace_mps_param_extract.h"
#include "ixheaace_mps_tree.h"
#include "ixheaace_mps_rom.h"

IA_ERRORCODE ixheaace_mps_212_static_gain_init(
    ixheaace_mps_pstr_static_gain pstr_static_gain,
    const ixheaace_mps_pstr_static_gain_config pstr_static_gain_config) {
  IA_ERRORCODE error = IA_NO_ERROR;
  FLOAT32 pre_gain_factor;
  pstr_static_gain->fixed_gain_dmx = pstr_static_gain_config->fixed_gain_dmx;
  pstr_static_gain->pre_gain_factor_db = pstr_static_gain_config->pre_gain_factor_db;

  if ((pstr_static_gain->pre_gain_factor_db < -20) ||
      (pstr_static_gain->pre_gain_factor_db > 20)) {
    return IA_EXHEAACE_CONFIG_NONFATAL_MPS_INVALID_CONFIG;
  }

  if (pstr_static_gain->pre_gain_factor_db) {
    pre_gain_factor = pre_gain_factor_table_flt_new[pstr_static_gain->pre_gain_factor_db + 20];
  } else {
    pre_gain_factor = 1.0f;
  }

  if (pstr_static_gain->fixed_gain_dmx) {
    pstr_static_gain->post_gain = dmx_gain_table_flt[pstr_static_gain->fixed_gain_dmx - 1];
  } else {
    pstr_static_gain->post_gain = 1.0f;
  }

  memset(pstr_static_gain->pre_gain, 0, sizeof(pstr_static_gain->pre_gain));

  pstr_static_gain->pre_gain[0] = pre_gain_factor;
  pstr_static_gain->pre_gain[1] = pre_gain_factor;

  return error;
}
