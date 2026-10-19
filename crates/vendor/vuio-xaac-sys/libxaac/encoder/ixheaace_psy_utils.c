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

#include <string.h>
#include "ixheaac_type_def.h"
#include "ixheaac_constants.h"
#include "ixheaace_aac_constants.h"
#include "ixheaace_psy_utils.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops16.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaac_basic_ops.h"

VOID ia_enhaacplus_enc_init_pre_echo_control(FLOAT32 *ptr_pb_threshold_nm1, WORD32 num_pb,
                                             FLOAT32 *ptr_pb_threshold_quiet) {
  memcpy(ptr_pb_threshold_nm1, ptr_pb_threshold_quiet, num_pb * sizeof(*ptr_pb_threshold_nm1));
}

VOID ia_enhaacplus_enc_pre_echo_control(FLOAT32 *ptr_threshold_nm1, WORD32 num_sfb,
                                        FLOAT32 min_rem_thr_factor, FLOAT32 *ptr_threshold) {
  WORD32 i;
  FLOAT32 tmp_threshold1, tmp_threshold2;
  FLOAT32 *ptr_tmp_threshold = &ptr_threshold[0];
  FLOAT32 *ptr_tmp_threshold_nm1 = &ptr_threshold_nm1[0];

  for (i = num_sfb - 1; i >= 0; i--) {
    FLOAT32 tmp_var;

    tmp_var = *ptr_tmp_threshold;

    tmp_threshold1 = *ptr_tmp_threshold_nm1 * 2.0f;

    tmp_threshold2 = (min_rem_thr_factor * tmp_var);

    /* copy thresholds to internal memory */
    *ptr_tmp_threshold_nm1++ = tmp_var;

    tmp_var = MIN(tmp_var, tmp_threshold1);

    tmp_var = MAX(tmp_var, tmp_threshold2);
    *ptr_tmp_threshold++ = tmp_var;
  }
}
