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
#include <string.h>
#include "ixheaacd_sbr_common.h"
#include "ixheaac_type_def.h"

#include "ixheaac_constants.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops16.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaac_basic_ops.h"

#include "ixheaac_basic_op.h"
#include "ixheaacd_intrinsics.h"
#include "ixheaacd_bitbuffer.h"
#include "ixheaacd_sbrdecsettings.h"
#include "ixheaacd_sbr_scale.h"
#include "ixheaacd_lpp_tran.h"
#include "ixheaacd_env_extr_part.h"
#include "ixheaacd_sbr_rom.h"

#include "ixheaacd_hybrid.h"
#include "ixheaacd_ps_dec.h"

#include "ixheaacd_env_extr.h"
#include "ixheaacd_ps_bitdec.h"

const WORD16 ixheaacd_num_bands[3] = {10, 20, 34};

static WORD32 ixheaacd_clamp(WORD32 i, WORD16 min, WORD16 max) {
  WORD32 result = i;

  if (i < min) {
    result = min;
  } else {
    if (i > max) {
      result = max;
    }
  }

  return result;
}

WORD16 ixheaacd_divideby2(WORD op) {
  FLAG sign = (op < 0);

  if (sign) {
    op = -(op);
  }

  op = (op >> 1);

  if (sign) {
    op = -op;
  }

  return (WORD16)op;
}

WORD16 ixheaacd_divideby3(WORD op) {
  WORD16 temp, ret;
  FLAG sign = (op < 0);

  if (sign) {
    op = -(op);
  }

  temp = (WORD16)(op << 2);

  temp = ixheaac_mult16_shl(temp, 0x2aab);

  ret = (temp >> 2);

  if (sign) {
    ret = -(ret);
  }

  return (WORD16)ret;
}

VOID ixheaacd_decode_ps_data(ia_ps_dec_struct *ptr_ps_dec, WORD32 frame_size) {
  WORD e, i, temp;
  WORD16 iid_mode = (WORD16)((ptr_ps_dec->iid_mode) ? 1 : 2);
  WORD16 icc_mode = (WORD16)((ptr_ps_dec->icc_mode) ? 1 : 2);
  WORD16 num_iid_levels =
      (WORD16)(ptr_ps_dec->iid_quant ? NUM_IID_LEVELS_FINE : NUM_IID_LEVELS);
  WORD32 max_num_columns;

  if (frame_size == 960)
    max_num_columns = MAX_NUM_COLUMNS_960;
  else
    max_num_columns = MAX_NUM_COLUMNS;

  if (!ptr_ps_dec->ps_data_present) {
    ptr_ps_dec->num_env = 0;
  }

  for (e = 0; e < ptr_ps_dec->num_env; e++) {
    WORD16 *p_iid_par_prev;
    WORD16 *p_icc_par_prev;

    if (e == 0) {
      p_iid_par_prev = ptr_ps_dec->iid_par_prev;
      p_icc_par_prev = ptr_ps_dec->icc_par_prev;
    } else {
      p_iid_par_prev = ptr_ps_dec->iid_par_table[e - 1];
      p_icc_par_prev = ptr_ps_dec->icc_par_table[e - 1];
    }

    if (ptr_ps_dec->enable_iid) {
      if (ptr_ps_dec->iid_dt[e]) {
        for (i = 0; i < ixheaacd_num_bands[ptr_ps_dec->iid_mode]; i++) {
          temp =
              ixheaac_add16(*p_iid_par_prev, ptr_ps_dec->iid_par_table[e][i]);
          ptr_ps_dec->iid_par_table[e][i] = ixheaacd_clamp(
              temp, ixheaac_negate16(num_iid_levels), num_iid_levels);
          p_iid_par_prev += iid_mode;
        }
      } else {
        ptr_ps_dec->iid_par_table[e][0] =
            ixheaacd_clamp(ptr_ps_dec->iid_par_table[e][0],
                           ixheaac_negate16(num_iid_levels), num_iid_levels);
        for (i = 1; i < ixheaacd_num_bands[ptr_ps_dec->iid_mode]; i++) {
          temp = ixheaac_add16(ptr_ps_dec->iid_par_table[e][i - 1],
                                ptr_ps_dec->iid_par_table[e][i]);
          ptr_ps_dec->iid_par_table[e][i] = ixheaacd_clamp(
              temp, ixheaac_negate16(num_iid_levels), num_iid_levels);
        }
      }
    } else {
      memset(ptr_ps_dec->iid_par_table[e], 0,
             sizeof(WORD16) * ixheaacd_num_bands[ptr_ps_dec->iid_mode]);
    }

    if (iid_mode == 2) {
      for (i = (ixheaacd_num_bands[ptr_ps_dec->iid_mode] * iid_mode - 1);
           i != 0; i--) {
        ptr_ps_dec->iid_par_table[e][i] =
            ptr_ps_dec->iid_par_table[e][ixheaac_shr32(i, 1)];
      }
    }

    if (ptr_ps_dec->enable_icc) {
      if (ptr_ps_dec->icc_dt[e]) {
        for (i = 0; i < ixheaacd_num_bands[ptr_ps_dec->icc_mode]; i++) {
          temp =
              ixheaac_add16(*p_icc_par_prev, ptr_ps_dec->icc_par_table[e][i]);
          ptr_ps_dec->icc_par_table[e][i] =
              ixheaacd_clamp(temp, 0, (WORD16)(NUM_ICC_LEVELS - 1));
          p_icc_par_prev += icc_mode;
        }
      } else {
        ptr_ps_dec->icc_par_table[e][0] = ixheaacd_clamp(
            ptr_ps_dec->icc_par_table[e][0], 0, (WORD16)(NUM_ICC_LEVELS - 1));
        for (i = 1; i < ixheaacd_num_bands[ptr_ps_dec->icc_mode]; i++) {
          temp = ixheaac_add16(ptr_ps_dec->icc_par_table[e][i - 1],
                                ptr_ps_dec->icc_par_table[e][i]);
          ptr_ps_dec->icc_par_table[e][i] =
              ixheaacd_clamp(temp, 0, (WORD16)(NUM_ICC_LEVELS - 1));
        }
      }
    } else {
      memset(ptr_ps_dec->icc_par_table[e], 0,
             sizeof(WORD16) * ixheaacd_num_bands[ptr_ps_dec->icc_mode]);
    }

    if (icc_mode == 2) {
      for (i = (ixheaacd_num_bands[ptr_ps_dec->icc_mode] * icc_mode - 1);
           i != 0; i--) {
        ptr_ps_dec->icc_par_table[e][i] =
            ptr_ps_dec->icc_par_table[e][ixheaac_shr32(i, 1)];
      }
    }
  }

  if (ptr_ps_dec->num_env == 0) {
    ptr_ps_dec->num_env = 1;

    if (ptr_ps_dec->enable_iid) {
      memcpy(ptr_ps_dec->iid_par_table[0], ptr_ps_dec->iid_par_prev,
             sizeof(WORD16) * NUM_BANDS_FINE);
    } else {
      memset(ptr_ps_dec->iid_par_table[0], 0, sizeof(WORD16) * NUM_BANDS_FINE);
    }

    if (ptr_ps_dec->enable_icc) {
      memcpy(ptr_ps_dec->icc_par_table[0], ptr_ps_dec->icc_par_prev,
             sizeof(WORD16) * NUM_BANDS_FINE);
    } else {
      memset(ptr_ps_dec->icc_par_table[0], 0, sizeof(WORD16) * NUM_BANDS_FINE);
    }
  }

  memcpy(ptr_ps_dec->iid_par_prev,
         ptr_ps_dec->iid_par_table[ptr_ps_dec->num_env - 1],
         sizeof(WORD16) * NUM_BANDS_FINE);

  memcpy(ptr_ps_dec->icc_par_prev,
         ptr_ps_dec->icc_par_table[ptr_ps_dec->num_env - 1],
         sizeof(WORD16) * NUM_BANDS_FINE);

  ptr_ps_dec->ps_data_present = 0;

  if (ptr_ps_dec->frame_class == 0) {
    WORD env_count;
    WORD16 shift = 0;

    switch (ptr_ps_dec->num_env) {
      case 1:
        shift = 0;
        break;
      case 2:
        shift = 1;
        break;
      case 4:
        shift = 2;
        break;
    }
    ptr_ps_dec->border_position[0] = 0;
    env_count = 0;

    for (e = 1; e < ptr_ps_dec->num_env; e++) {
      env_count = add_d(env_count, max_num_columns);
      ptr_ps_dec->border_position[e] = (WORD16)(env_count >> shift);
    }
    ptr_ps_dec->border_position[ptr_ps_dec->num_env] = max_num_columns;
  } else {
    ptr_ps_dec->border_position[0] = 0;

    if (ptr_ps_dec->border_position[ptr_ps_dec->num_env] < max_num_columns) {
      ptr_ps_dec->num_env++;
      ptr_ps_dec->border_position[ptr_ps_dec->num_env] = max_num_columns;

      memcpy(ptr_ps_dec->iid_par_table[ptr_ps_dec->num_env - 1],
             ptr_ps_dec->iid_par_table[ptr_ps_dec->num_env - 2],
             sizeof(WORD16) * NUM_BANDS_FINE);

      memcpy(ptr_ps_dec->icc_par_table[ptr_ps_dec->num_env - 1],
             ptr_ps_dec->icc_par_table[ptr_ps_dec->num_env - 2],
             sizeof(WORD16) * NUM_BANDS_FINE);
    }

    for (e = 1; e < ptr_ps_dec->num_env; e++) {
      WORD threshold;
      threshold = sub_d(max_num_columns, sub_d(ptr_ps_dec->num_env, e));

      if (ptr_ps_dec->border_position[e] > threshold) {
        ptr_ps_dec->border_position[e] = threshold;
      } else {
        threshold = add_d(ptr_ps_dec->border_position[e - 1], 1);

        if (ptr_ps_dec->border_position[e] < threshold) {
          ptr_ps_dec->border_position[e] = threshold;
        }
      }
    }
  }

  for (e = 0; e < ptr_ps_dec->num_env; e++) {
    if (ptr_ps_dec->iid_mode == 2)
      ixheaacd_map_34_params_to_20(ptr_ps_dec->iid_par_table[e]);

    if (ptr_ps_dec->icc_mode == 2)
      ixheaacd_map_34_params_to_20(ptr_ps_dec->icc_par_table[e]);
  }
}