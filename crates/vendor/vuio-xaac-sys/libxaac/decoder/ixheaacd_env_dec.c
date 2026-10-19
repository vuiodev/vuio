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
#include <math.h>
#include "ixheaacd_sbr_common.h"
#include "ixheaac_type_def.h"
#include "ixheaac_error_standards.h"
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
#include "ixheaacd_common_rom.h"
#include "ixheaacd_env_dec.h"
#include "ixheaac_sbr_const.h"
#include "ixheaacd_basic_funcs.h"
#include "ixheaacd_audioobjtypes.h"

#define add16_m(a, b) ((a) + (b))
#define sub16_m(a, b) ((a) - (b))

#define mult16x16_16(a, b) ixheaac_mult16((a), (b))

static VOID ixheaacd_dequant_esbr_env_data(FLOAT32 *ptr_env_sf,
                                           WORD32 num_env_sf,
                                           WORD32 num_noise_fac, WORD32 amp_res,
                                           FLOAT32 *ptr_noise_floor) {
  WORD32 i;
  const FLOAT32 array[2] = {0.5f, 1.0f};
  FLOAT32 a_flt = array[amp_res];

  for (i = 0; i < num_env_sf; i++) {
    ptr_env_sf[i] = (FLOAT32)(pow(2, ptr_env_sf[i] * a_flt) * 64);
  }

  for (i = 0; i < num_noise_fac; i++) {
    FLOAT32 temp = ptr_noise_floor[i];

    temp = NOISE_FLOOR_OFFSET - temp;
    temp = (FLOAT32)pow(2.0f, temp);

    ptr_noise_floor[i] = temp;
  }
}

static VOID ixheaacd_dequant_pvc_env_data(WORD32 num_noise_fac,
                                          FLOAT32 *ptr_noise_floor) {
  WORD32 i;

  for (i = 0; i < num_noise_fac; i++) {
    FLOAT32 temp = ptr_noise_floor[i];

    temp = NOISE_FLOOR_OFFSET - temp;
    temp = (FLOAT32)pow(2.0f, temp);

    ptr_noise_floor[i] = temp;
  }
}

VOID ixheaacd_map_res_energy(WORD16 curr_val, WORD16 *prev_data,
                             WORD32 ixheaacd_drc_offset, WORD32 index,
                             WORD32 res) {
  if (res == LOW) {
    if (ixheaacd_drc_offset >= 0) {
      if (index < ixheaacd_drc_offset) {
        prev_data[index] = curr_val;
      } else {
        WORD32 index_2;
        index_2 = ((index + index) - ixheaacd_drc_offset);
        prev_data[index_2] = curr_val;
        prev_data[index_2 + 1] = curr_val;
      }
    } else {
      ixheaacd_drc_offset = -(ixheaacd_drc_offset);

      if (index < ixheaacd_drc_offset) {
        WORD32 index_3;
        index_3 = ((index + index) + index);
        prev_data[index_3] = curr_val;
        prev_data[index_3 + 1] = curr_val;
        prev_data[index_3 + 2] = curr_val;
      } else {
        WORD32 index_2;
        index_2 = ((index + index) + ixheaacd_drc_offset);
        prev_data[index_2] = curr_val;
        prev_data[index_2 + 1] = curr_val;
      }
    }
  } else {
    prev_data[index] = curr_val;
  }
}

VOID ixheaacd_process_del_cod_env_data(
    ia_sbr_header_data_struct *ptr_header_data,
    ia_sbr_frame_info_data_struct *ptr_sbr_data,
    ia_sbr_prev_frame_data_struct *ptr_prev_data) {
  WORD32 i, dtdf_dir, num_sf_bands, band, freq_res;
  WORD16 temp_val;
  WORD16 *ptr_prev_env_sf = ptr_prev_data->sfb_nrg_prev;
  WORD16 *ptr_env_sf = ptr_sbr_data->int_env_sf_arr;

  FLOAT32 *ptr_env_sf_float = ptr_sbr_data->flt_env_sf_arr;

  WORD32 ixheaacd_drc_offset;

  ixheaacd_drc_offset =
      ((ptr_header_data->pstr_freq_band_data->num_sf_bands[LOW] << 1) -
       ptr_header_data->pstr_freq_band_data->num_sf_bands[HIGH]);

  for (i = 0; i < ptr_sbr_data->str_frame_info_details.num_env; i++) {
    dtdf_dir = ptr_sbr_data->del_cod_dir_arr[i];
    freq_res = ptr_sbr_data->str_frame_info_details.freq_res[i];

    num_sf_bands = ptr_header_data->pstr_freq_band_data->num_sf_bands[freq_res];
    band = num_sf_bands;

    if (dtdf_dir == DTDF_DIR_FREQ) {
      ixheaacd_map_res_energy(*ptr_env_sf, ptr_prev_env_sf, ixheaacd_drc_offset,
                              0, freq_res);
      ptr_env_sf++;

      ptr_env_sf_float++;

      for (band = 1; band < num_sf_bands; band++) {
        *ptr_env_sf = *ptr_env_sf + *(ptr_env_sf - 1);
        ixheaacd_map_res_energy(*ptr_env_sf, ptr_prev_env_sf,
                                ixheaacd_drc_offset, band, freq_res);

        *ptr_env_sf_float = (FLOAT32)(*ptr_env_sf);
        ptr_env_sf_float++;

        ptr_env_sf++;
      }

    } else {
      if (freq_res == LOW) {
        if (ixheaacd_drc_offset < 0) {
          WORD32 tar, index_3;
          ixheaacd_drc_offset = -ixheaacd_drc_offset;
          tar = ixheaac_min32(ixheaacd_drc_offset, band);

          for (band = 0; band < tar; band++) {
            index_3 = ((band + band) + band);
            temp_val = add16_m(*ptr_env_sf, ptr_prev_env_sf[index_3]);

            ptr_prev_env_sf[index_3] = temp_val;
            ptr_prev_env_sf[index_3 + 1] = temp_val;
            ptr_prev_env_sf[index_3 + 2] = temp_val;
            *ptr_env_sf++ = temp_val;

            *ptr_env_sf_float = (FLOAT32)temp_val;
            ptr_env_sf_float++;
          }

          for (; band < num_sf_bands; band++) {
            index_3 = (band << 1) + ixheaacd_drc_offset;
            temp_val = add16_m(*ptr_env_sf, ptr_prev_env_sf[index_3]);
            ptr_prev_env_sf[index_3] = temp_val;
            ptr_prev_env_sf[index_3 + 1] = temp_val;
            *ptr_env_sf++ = temp_val;
            *ptr_env_sf_float = (FLOAT32)temp_val;
            ptr_env_sf_float++;
          }
        } else {
          WORD32 tar, index_2;
          WORD16 *ptr2 = ptr_prev_env_sf;
          tar = ixheaac_min32(ixheaacd_drc_offset, band);
          for (band = 0; band < tar; band++) {
            *ptr_env_sf = add16_m(*ptr_env_sf, *ptr2);
            *ptr2 = *ptr_env_sf;

            *ptr_env_sf_float = (FLOAT32)(*ptr_env_sf);
            ptr_env_sf_float++;

            ptr2++;
            ptr_env_sf++;
          }

          for (; band < num_sf_bands; band++) {
            index_2 = (band < ixheaacd_drc_offset)
                          ? band
                          : ((band << 1) - ixheaacd_drc_offset);
            temp_val = add16_m(*ptr_env_sf, ptr_prev_env_sf[index_2]);
            ptr_prev_env_sf[index_2] = temp_val;
            ptr_prev_env_sf[index_2 + 1] = temp_val;
            *ptr_env_sf++ = temp_val;

            *ptr_env_sf_float = (FLOAT32)temp_val;
            ptr_env_sf_float++;
          }
        }

      } else {
        WORD16 *ptr2 = ptr_prev_env_sf;
        for (band = num_sf_bands - 1; band >= 0; band--) {
          *ptr_env_sf = add16_m(*ptr_env_sf, *ptr2);
          *ptr2 = *ptr_env_sf;
          *ptr_env_sf_float = (FLOAT32)(*ptr_env_sf);
          ptr_env_sf_float++;
          ptr2++;
          ptr_env_sf++;
        }
        band = num_sf_bands;
      }
    }
  }
}

static PLATFORM_INLINE WORD32
ixheaacd_wrong_timing_compensate(ia_sbr_header_data_struct *ptr_header_data,
                                 ia_sbr_frame_info_data_struct *ptr_sbr_data,
                                 ia_sbr_prev_frame_data_struct *ptr_prev_data,
                                 ixheaacd_misc_tables *pstr_common_tables) {
  WORD32 i, num_env_sf;
  ia_frame_info_struct *p_frame_info = &ptr_sbr_data->str_frame_info_details;
  WORD16 *num_sf_bands = ptr_header_data->pstr_freq_band_data->num_sf_bands;
  WORD32 start_pos_est;
  WORD32 ref_len, new_len, shift;
  WORD16 delta_exp;

  start_pos_est =
      (ptr_prev_data->end_position - ptr_header_data->num_time_slots);

  ref_len = (p_frame_info->border_vec[1] - p_frame_info->border_vec[0]);

  new_len = (p_frame_info->border_vec[1] - start_pos_est);

  if (new_len <= 0) {
    new_len = ref_len;
    start_pos_est = p_frame_info->border_vec[0];
  }

  delta_exp = pstr_common_tables->log_dual_is_table[ref_len];
  delta_exp -= pstr_common_tables->log_dual_is_table[new_len];

  shift = (SHORT_BITS - ENV_EXP_FRACT - 3 - ptr_sbr_data->amp_res);
  delta_exp = ixheaac_shr16(delta_exp, (WORD16)shift);
  p_frame_info->border_vec[0] = start_pos_est;
  p_frame_info->noise_border_vec[0] = start_pos_est;

  if (start_pos_est < 0) return -1;

  if (ptr_sbr_data->coupling_mode != COUPLING_BAL) {
    num_env_sf =
        ((p_frame_info->freq_res[0]) ? num_sf_bands[HIGH] : num_sf_bands[LOW]);

    for (i = 0; i < num_env_sf; i++) {
      ptr_sbr_data->int_env_sf_arr[i] =
          add16_m(ptr_sbr_data->int_env_sf_arr[i], delta_exp);
      ptr_sbr_data->flt_env_sf_arr[i] =
          (FLOAT32)(ptr_sbr_data->int_env_sf_arr[i]);
    }
  }

  return 0;
}

WORD16 ixheaacd_check_env_data(ia_sbr_header_data_struct *ptr_header_data,
                               ia_sbr_frame_info_data_struct *ptr_sbr_data,
                               ia_sbr_prev_frame_data_struct *ptr_prev_data) {
  WORD16 *ptr_evn_sf = ptr_sbr_data->int_env_sf_arr;
  FLOAT32 *ptr_evn_sf_float = ptr_sbr_data->flt_env_sf_arr;
  WORD16 *ptr_prev_evn_sf = ptr_prev_data->sfb_nrg_prev;
  WORD32 i;
  FLAG error_code = 0;
  WORD16 sbr_max_env_sf;
  WORD32 amp_res = ptr_sbr_data->amp_res;

  sbr_max_env_sf = (SBR_ENV_SF_MAX_VAL_1_5 >> amp_res);

  for (i = 0; i < ptr_sbr_data->num_env_sfac; i++) {
    if (ptr_evn_sf[i] > sbr_max_env_sf) {
      error_code = 1;
    }
    if (ptr_evn_sf[i] < 0) {
      ptr_evn_sf[i] = 0;
      ptr_evn_sf_float[i] = 0;
    }
  }

  for (i = 0; i < ptr_header_data->pstr_freq_band_data->num_sf_bands[HIGH];
       i++) {
    if (ptr_prev_evn_sf[i] < 0) {
      ptr_prev_evn_sf[i] = 0;
    } else {
      if (ptr_prev_evn_sf[i] > sbr_max_env_sf)
        ptr_prev_evn_sf[i] = sbr_max_env_sf;
    }
  }
  return (WORD16)(error_code);
}

VOID ixheaacd_dequant_env_data(ia_sbr_frame_info_data_struct *ptr_sbr_data,
                               WORD32 amp_res) {
  WORD32 i;
  WORD32 num_env_sf = ptr_sbr_data->num_env_sfac;
  WORD32 mantissa;
  WORD32 amp_res_1;
  WORD32 exponent;
  WORD32 exp_add = (7 + NRG_EXP_OFFSET);
  WORD16 *ptr_env_sf = ptr_sbr_data->int_env_sf_arr;
  const WORD32 mant_arr[2] = {0x4000, 0x5a80};

  amp_res_1 = (1 - amp_res);

  for (i = num_env_sf - 1; i >= 0; i--) {
    exponent = *ptr_env_sf;
    mantissa = mant_arr[(exponent & amp_res_1)];
    exponent = (exponent >> amp_res_1);
    exponent = (exponent + exp_add);
    *ptr_env_sf++ = (WORD16)(mantissa | (exponent & MASK_FOR_EXP));
  }
}

static PLATFORM_INLINE VOID
ixheaacd_limit_noise_floor_fac(ia_sbr_header_data_struct *ptr_header_data,
                               ia_sbr_frame_info_data_struct *ptr_sbr_data) {
  WORD32 i, tot_nf_bands;
  WORD32 value;
  WORD32 num_nf_bands;
  WORD16 *ptr_noise_floor;
  FLOAT32 *ptr_noise_floor_flt;

  num_nf_bands = ptr_header_data->pstr_freq_band_data->num_nf_bands;

  tot_nf_bands =
      ptr_sbr_data->str_frame_info_details.num_noise_env * num_nf_bands;

  ptr_noise_floor = ptr_sbr_data->int_noise_floor;

  ptr_noise_floor_flt = &ptr_sbr_data->flt_noise_floor[0];

  for (i = tot_nf_bands - 1; i >= 0; i--) {
    value = *ptr_noise_floor;
    if (value > MAX_NOISE_FLOOR_FAC_VAL) {
      *ptr_noise_floor = MAX_NOISE_FLOOR_FAC_VAL;
      *ptr_noise_floor_flt = MAX_NOISE_FLOOR_FAC_VAL;
    } else {
      if (value < MIN_NOISE_FLOOR_FAC_VAL) {
        *ptr_noise_floor = MIN_NOISE_FLOOR_FAC_VAL;
        *ptr_noise_floor_flt = MIN_NOISE_FLOOR_FAC_VAL;
      }
    }
    ptr_noise_floor++;
    ptr_noise_floor_flt++;
  }
}

VOID ixheaacd_add_arr(WORD16 *ptr1, WORD16 *ptr2, WORD32 num) {
  WORD32 i;
  for (i = num - 1; i >= 0; i--) {
    *ptr2 = (*ptr2 + *ptr1);
    ptr2++;
    ptr1++;
  }
}

VOID ixheaacd_add_farr(FLOAT32 *ptr1, FLOAT32 *ptr2, WORD32 num) {
  WORD32 i;
  for (i = num - 1; i >= 0; i--) {
    *ptr2 = (*ptr2 + *ptr1);
    ptr2++;
    ptr1++;
  }
}

IA_ERRORCODE ixheaacd_calc_noise_floor(
    ia_sbr_header_data_struct *ptr_header_data,
    ia_sbr_frame_info_data_struct *ptr_sbr_data,
    ia_sbr_prev_frame_data_struct *ptr_prev_data,
    WORD32 audio_object_type) {
  WORD32 i;
  WORD32 num_nf_bands;
  WORD32 num_noise_env;
  WORD32 ixheaacd_drc_offset;
  WORD16 *ptr_noise_floor = ptr_sbr_data->int_noise_floor;

  WORD16 *ptr_prev_noise_floor = ptr_prev_data->prev_noise_level;
  WORD16 *ptr1, *ptr2;
  FLOAT32 *f_ptr1, *f_ptr2;
  WORD32 num;
  FLOAT32 *ptr_noise_floor_float = ptr_sbr_data->flt_noise_floor;

  num_nf_bands = ptr_header_data->pstr_freq_band_data->num_nf_bands;
  num_noise_env = ptr_sbr_data->str_frame_info_details.num_noise_env;

  if (ptr_sbr_data->del_cod_dir_noise_arr[0] == DTDF_DIR_FREQ) {
    ptr1 = ptr_noise_floor++;
    ptr2 = ptr_noise_floor;
    num = num_nf_bands - 1;
    f_ptr1 = &ptr_noise_floor_float[0];
    f_ptr2 = &ptr_noise_floor_float[1];
  } else {
    ptr1 = ptr_prev_noise_floor;
    ptr2 = ptr_sbr_data->int_noise_floor;
    f_ptr1 = &ptr_sbr_data->prev_noise_level[0];
    f_ptr2 = &ptr_noise_floor_float[0];
    num = num_nf_bands;
  }

  ixheaacd_add_arr(ptr1, ptr2, num);
  ixheaacd_add_farr(f_ptr1, f_ptr2, num);

  if (num_noise_env > 1) {
    if (ptr_sbr_data->del_cod_dir_noise_arr[1] == DTDF_DIR_FREQ) {
      ptr1 = &ptr_sbr_data->int_noise_floor[num_nf_bands];
      ptr2 = &ptr_sbr_data->int_noise_floor[(num_nf_bands + 1)];
      f_ptr1 = &ptr_sbr_data->flt_noise_floor[num_nf_bands];
      f_ptr2 = &ptr_sbr_data->flt_noise_floor[(num_nf_bands + 1)];
      num = num_nf_bands - 1;
    } else {
      ptr1 = &ptr_sbr_data->int_noise_floor[0];
      ptr2 = &ptr_sbr_data->int_noise_floor[num_nf_bands];
      f_ptr1 = &ptr_sbr_data->flt_noise_floor[0];
      f_ptr2 = &ptr_sbr_data->flt_noise_floor[num_nf_bands];
      num = num_nf_bands;
    }
    ixheaacd_add_arr(ptr1, ptr2, num);
    ixheaacd_add_farr(f_ptr1, f_ptr2, num);
  }

  ixheaacd_limit_noise_floor_fac(ptr_header_data, ptr_sbr_data);

  ixheaacd_drc_offset = num_nf_bands * (num_noise_env - 1);
  if (ixheaacd_drc_offset < 0 || ixheaacd_drc_offset >= MAX_NUM_NOISE_VALUES)
    return IA_FATAL_ERROR;
  ptr1 = &ptr_sbr_data->int_noise_floor[ixheaacd_drc_offset];
  ptr2 = ptr_prev_noise_floor;

  memcpy(ptr2, ptr1, sizeof(WORD16) * (num_nf_bands));
  if (!ptr_header_data->usac_flag) {
    for (i = 0; i < num_nf_bands; i++) {
      ptr_sbr_data->prev_noise_level[i] = ptr_prev_noise_floor[i];
    }
  }
  if (audio_object_type != AOT_ER_AAC_ELD && ptr_header_data->enh_sbr) {
    WORD32 noise_floor_exp, tot_nf_bands;

    tot_nf_bands = (num_nf_bands * num_noise_env);
    ptr_noise_floor = &ptr_sbr_data->int_noise_floor[0];

    for (i = 0; i < tot_nf_bands; i++) {
      noise_floor_exp =
          (NOISE_FLOOR_OFFSET_INT + 1 + NOISE_EXP_OFFSET - *ptr_noise_floor);

      *ptr_noise_floor_float++ = *ptr_noise_floor;
      *ptr_noise_floor++ = (WORD16)(0x4000 + (noise_floor_exp & MASK_FOR_EXP));
    }
  } else {
    if (ptr_sbr_data->coupling_mode != COUPLING_BAL) {
      WORD32 noise_floor_exp, tot_nf_bands;

      tot_nf_bands = (num_nf_bands * num_noise_env);
      ptr_noise_floor = &ptr_sbr_data->int_noise_floor[0];

      for (i = 0; i < tot_nf_bands; i++) {
        noise_floor_exp =
            (NOISE_FLOOR_OFFSET_INT + 1 + NOISE_EXP_OFFSET - *ptr_noise_floor);
        *ptr_noise_floor_float++ = *ptr_noise_floor;
        *ptr_noise_floor++ = (WORD16)(0x4000 + (noise_floor_exp & MASK_FOR_EXP));
      }
    }
  }
  return IA_NO_ERROR;
}

IA_ERRORCODE ixheaacd_dec_sbrdata_for_pvc(
    ia_sbr_header_data_struct *ptr_header_data,
    ia_sbr_frame_info_data_struct *ptr_sbr_data,
    ia_sbr_prev_frame_data_struct *ptr_prev_data,
    WORD32 audio_object_type) {
  WORD32 err = 0;
  err = ixheaacd_calc_noise_floor(ptr_header_data, ptr_sbr_data, ptr_prev_data,
                                  audio_object_type);
  if (err) return err;

  if (!ptr_sbr_data->coupling_mode) {
    ptr_sbr_data->num_noise_sfac =
        ptr_header_data->pstr_freq_band_data->num_nf_bands *
        ptr_sbr_data->str_frame_info_details.num_noise_env;
    ixheaacd_dequant_pvc_env_data(ptr_sbr_data->num_noise_sfac,
                                  ptr_sbr_data->flt_noise_floor);
  }
  return IA_NO_ERROR;
}

VOID ixheaacd_sbr_env_dequant_coup_fix(
    ia_sbr_header_data_struct *ptr_header_data,
    ia_sbr_frame_info_data_struct *ptr_data_left,
    ia_sbr_frame_info_data_struct *ptr_data_right,
    ixheaacd_misc_tables *pstr_common_tables) {
  WORD32 i;
  WORD32 num_env_sf = ptr_data_left->num_env_sfac;
  WORD16 temp_left_mant, temp_right_mant, temp_right_plus1_mant, new_left_mant,
      new_right_mant;
  WORD16 temp_left_exp, temp_right_exp, temp_rightplus1_exp, new_left_exp,
      new_right_exp;
  WORD32 i_end;
  WORD16 *r_data = ptr_data_right->int_env_sf_arr;
  WORD16 *l_data = ptr_data_left->int_env_sf_arr;

  for (i = 0; i < num_env_sf; i++) {
    temp_right_mant = (WORD16)(*r_data & MASK_M);
    temp_right_exp = (WORD16)(*r_data & MASK_FOR_EXP);

    temp_right_exp = sub16_m(temp_right_exp, add16_m(18, NRG_EXP_OFFSET));
    temp_left_mant = (WORD16)(*l_data & MASK_M);
    temp_left_exp = (WORD16)(*l_data & MASK_FOR_EXP);

    temp_left_exp = sub16_m(temp_left_exp, NRG_EXP_OFFSET);

    ixheaacd_fix_mant_exp_add(temp_right_mant, temp_right_exp, 0x4000, 1,
                              &temp_right_plus1_mant, &temp_rightplus1_exp);

    new_right_exp = ixheaacd_fix_mant_div(temp_left_mant, temp_right_plus1_mant,
                                          &new_right_mant, pstr_common_tables);

    new_right_exp += temp_left_exp - temp_rightplus1_exp + 2;

    new_left_mant = ixheaac_mult16_shl(temp_right_mant, new_right_mant);

    new_left_exp = add16_m(temp_right_exp, new_right_exp);

    *r_data++ = (WORD16)(((new_right_mant + ROUNDING) & MASK_M) +
                         ((new_right_exp + NRG_EXP_OFFSET) & MASK_FOR_EXP));
    *l_data++ = (WORD16)(((new_left_mant + ROUNDING) & MASK_M) +
                         ((new_left_exp + NRG_EXP_OFFSET) & MASK_FOR_EXP));
  }

  i_end = ptr_header_data->pstr_freq_band_data->num_nf_bands *
          ptr_data_left->str_frame_info_details.num_noise_env;
  r_data = ptr_data_right->int_noise_floor;
  l_data = ptr_data_left->int_noise_floor;

  for (i = i_end - 1; i >= 0; i--) {
    temp_left_exp =
        sub16_m((WORD16)(*l_data & (WORD16)MASK_FOR_EXP), NOISE_EXP_OFFSET);
    temp_right_exp = sub16_m(*r_data, 12);

    ixheaacd_fix_mant_exp_add(0x4000, ixheaac_add16(1, temp_right_exp), 0x4000,
                              1, &temp_right_plus1_mant, &temp_rightplus1_exp);

    new_right_exp = ixheaacd_fix_mant_div(0x4000, temp_right_plus1_mant,
                                          &new_right_mant, pstr_common_tables);

    new_right_exp += temp_left_exp - temp_rightplus1_exp + 2;

    new_left_mant = new_right_mant;
    new_left_exp = add16_m(new_right_exp, temp_right_exp);
    *r_data++ = (WORD16)(((new_right_mant + ROUNDING) & MASK_M) +
                         ((new_right_exp + NOISE_EXP_OFFSET) & MASK_FOR_EXP));
    *l_data++ = (WORD16)(((new_left_mant + ROUNDING) & MASK_M) +
                         ((new_left_exp + NOISE_EXP_OFFSET) & MASK_FOR_EXP));
  }
}

VOID ixheaacd_sbr_env_dequant_coup(
    ia_sbr_frame_info_data_struct *ptr_data_ch_0,
    ia_sbr_frame_info_data_struct *ptr_data_ch_1) {
  FLOAT32 *ptr_env_sf_left = ptr_data_ch_0->flt_env_sf_arr;
  FLOAT32 *ptr_env_sf_right = ptr_data_ch_1->flt_env_sf_arr;
  FLOAT32 *ptr_noise_floor_left = ptr_data_ch_0->flt_noise_floor;
  FLOAT32 *ptr_noise_floor_right = ptr_data_ch_1->flt_noise_floor;
  WORD32 num_env_sf = ptr_data_ch_0->num_env_sfac;
  WORD32 num_noise_fac = ptr_data_ch_0->num_noise_sfac;
  WORD32 amp_res = ptr_data_ch_0->amp_res;

  WORD32 i;
  FLOAT32 temp_l, temp_r;
  const FLOAT32 pan_offset[2] = {24.0f, 12.0f};
  const FLOAT32 a_arr[2] = {0.5f, 1.0f};

  FLOAT32 a = a_arr[amp_res];

  for (i = 0; i < num_env_sf; i++) {
    temp_l = ptr_env_sf_left[i];
    temp_r = ptr_env_sf_right[i];

    ptr_env_sf_left[i] =
        (FLOAT32)(64 * (pow(2, temp_l * a + 1) /
                        (1 + pow(2, (pan_offset[amp_res] - temp_r) * a))));
    ptr_env_sf_right[i] =
        (FLOAT32)(64 * (pow(2, temp_l * a + 1) /
                        (1 + pow(2, (temp_r - pan_offset[amp_res]) * a))));
  }

  for (i = 0; i < num_noise_fac; i++) {
    temp_l = ptr_noise_floor_left[i];
    temp_r = ptr_noise_floor_right[i];

    ptr_noise_floor_left[i] =
        (FLOAT32)(pow(2, NOISE_FLOOR_OFFSET - temp_l + 1) /
                  (1 + pow(2, pan_offset[1] - temp_r)));
    ptr_noise_floor_right[i] =
        (FLOAT32)(pow(2, NOISE_FLOOR_OFFSET - temp_l + 1) /
                  (1 + pow(2, temp_r - pan_offset[1])));
  }
}
IA_ERRORCODE ixheaacd_dec_sbrdata(ia_sbr_header_data_struct *ptr_header_data_ch_0,
                                  ia_sbr_header_data_struct *ptr_header_data_ch_1,
                                  ia_sbr_frame_info_data_struct *ptr_sbr_data_ch_0,
                                  ia_sbr_prev_frame_data_struct *ptr_prev_data_ch_0,
                                  ia_sbr_frame_info_data_struct *ptr_sbr_data_ch_1,
                                  ia_sbr_prev_frame_data_struct *ptr_prev_data_ch_1,
                                  ixheaacd_misc_tables *ptr_common_tables, WORD32 ldmps_present,
                                  WORD32 audio_object_type, WORD32 ec_flag) {
  FLAG error_code;
  WORD32 err = 0;
  WORD16 temp_sfb_nrg_prev[MAX_FREQ_COEFFS];
  WORD32 usac_flag = ptr_header_data_ch_0->usac_flag |
                     ptr_header_data_ch_0->enh_sbr;

  if (ec_flag) {
    memcpy(temp_sfb_nrg_prev, ptr_prev_data_ch_0->sfb_nrg_prev, MAX_FREQ_COEFFS * sizeof(WORD16));
  }

  err = ixheaacd_dec_envelope(ptr_header_data_ch_0, ptr_sbr_data_ch_0, ptr_prev_data_ch_0,
                              ptr_prev_data_ch_1, ptr_common_tables, audio_object_type, ec_flag);

  if (err) return err;

  err = ixheaacd_calc_noise_floor(ptr_header_data_ch_0, ptr_sbr_data_ch_0,
                                  ptr_prev_data_ch_0, audio_object_type);
  if (err) return err;
  if ((!ptr_sbr_data_ch_0->coupling_mode &&
     ((usac_flag) && (audio_object_type != AOT_ER_AAC_ELD))) || ldmps_present) {
    ptr_sbr_data_ch_0->num_noise_sfac =
        ptr_header_data_ch_0->pstr_freq_band_data->num_nf_bands *
        ptr_sbr_data_ch_0->str_frame_info_details.num_noise_env;

    ixheaacd_dequant_esbr_env_data(
        ptr_sbr_data_ch_0->flt_env_sf_arr, ptr_sbr_data_ch_0->num_env_sfac,
        ptr_sbr_data_ch_0->num_noise_sfac, ptr_sbr_data_ch_0->amp_res,
        ptr_sbr_data_ch_0->flt_noise_floor);
  }

  if (ptr_sbr_data_ch_1 != NULL) {
    error_code = ptr_header_data_ch_0->err_flag;
    err =
        ixheaacd_dec_envelope(ptr_header_data_ch_1, ptr_sbr_data_ch_1, ptr_prev_data_ch_1,
                              ptr_prev_data_ch_0, ptr_common_tables, audio_object_type, ec_flag);

    if (err) return err;

    err = ixheaacd_calc_noise_floor(ptr_header_data_ch_1, ptr_sbr_data_ch_1,
                                    ptr_prev_data_ch_1, audio_object_type);

    if (err) return err;

    if (!ptr_sbr_data_ch_1->coupling_mode &&
        ((usac_flag) && (audio_object_type != AOT_ER_AAC_ELD))) {
      ptr_sbr_data_ch_1->num_noise_sfac =
          ptr_header_data_ch_1->pstr_freq_band_data->num_nf_bands *
          ptr_sbr_data_ch_1->str_frame_info_details.num_noise_env;

      ixheaacd_dequant_esbr_env_data(
          ptr_sbr_data_ch_1->flt_env_sf_arr, ptr_sbr_data_ch_1->num_env_sfac,
          ptr_sbr_data_ch_1->num_noise_sfac, ptr_sbr_data_ch_1->amp_res,
          ptr_sbr_data_ch_1->flt_noise_floor);
    }
    if (ec_flag) {
      if ((!ptr_header_data_ch_0->usac_flag) || (audio_object_type == AOT_ER_AAC_ELD)) {
        if (!error_code && ptr_header_data_ch_0->err_flag) {
          memcpy(ptr_prev_data_ch_0->sfb_nrg_prev, temp_sfb_nrg_prev,
                 MAX_FREQ_COEFFS * sizeof(WORD16));
          err = ixheaacd_dec_envelope(ptr_header_data_ch_0, ptr_sbr_data_ch_0, ptr_prev_data_ch_0,
                                      ptr_prev_data_ch_1, ptr_common_tables, audio_object_type,
                                      ec_flag);

          if (err) return err;
        }
      }
    } else {
      if ((!usac_flag) || (audio_object_type == AOT_ER_AAC_ELD)) {
        if (!error_code && ptr_header_data_ch_0->err_flag) {
          err = ixheaacd_dec_envelope(ptr_header_data_ch_0, ptr_sbr_data_ch_0, ptr_prev_data_ch_0,
                                      ptr_prev_data_ch_1, ptr_common_tables, audio_object_type,
                                      ec_flag);

          if (err) return err;
        }
      }
    }

    if (ptr_sbr_data_ch_0->coupling_mode) {
      ptr_sbr_data_ch_0->num_noise_sfac =
          ptr_header_data_ch_1->pstr_freq_band_data->num_nf_bands *
          ptr_sbr_data_ch_1->str_frame_info_details.num_noise_env;
      ixheaacd_sbr_env_dequant_coup_fix(ptr_header_data_ch_0, ptr_sbr_data_ch_0,
                                        ptr_sbr_data_ch_1, ptr_common_tables);

      ixheaacd_sbr_env_dequant_coup(ptr_sbr_data_ch_0, ptr_sbr_data_ch_1);
    }
  }

  return 0;
}
IA_ERRORCODE ixheaacd_dec_envelope(ia_sbr_header_data_struct *ptr_header_data,
                                   ia_sbr_frame_info_data_struct *ptr_sbr_data,
                                   ia_sbr_prev_frame_data_struct *ptr_prev_data_ch_0,
                                   ia_sbr_prev_frame_data_struct *ptr_prev_data_ch_1,
                                   ixheaacd_misc_tables *pstr_common_tables,
                                   WORD32 audio_object_type, WORD32 ec_flag) {
  FLAG error_code;
  WORD32 err;
  WORD16 env_sf_local_arr[MAX_FREQ_COEFFS];
  WORD32 enh_sbr = ptr_header_data->enh_sbr;
  WORD32 usac_flag = enh_sbr | ptr_header_data->usac_flag;
  WORD32 temp_1 =
      ptr_prev_data_ch_0->end_position - ptr_header_data->num_time_slots;

  if (temp_1 < 0) return IA_FATAL_ERROR;
  temp_1 = ptr_sbr_data->str_frame_info_details.border_vec[0] - temp_1;

  if ((!ptr_header_data->err_flag_prev) && (!ptr_header_data->err_flag) &&
      (temp_1 != 0)) {
    if (ptr_sbr_data->del_cod_dir_arr[0] == DTDF_DIR_TIME) {
      ptr_header_data->err_flag = 1;
    } else {
      ptr_header_data->err_flag_prev = 1;
    }
  }
  if (ec_flag) {
    if (ptr_header_data->err_flag_prev && (!ptr_header_data->err_flag)) {
      if (ptr_sbr_data->del_cod_dir_arr[0] != 0) {
        ptr_header_data->err_flag = 1;
      }
    }
  }
  if (ec_flag && ptr_header_data->err_flag &&
      ((!ptr_header_data->usac_flag) || (audio_object_type == AOT_ER_AAC_ELD))) {
    ixheaacd_lean_sbrconcealment(ptr_header_data, ptr_sbr_data, ptr_prev_data_ch_0);

    ixheaacd_process_del_cod_env_data(ptr_header_data, ptr_sbr_data, ptr_prev_data_ch_0);
  } else if (ptr_header_data->err_flag &&
             ((!usac_flag) || (audio_object_type == AOT_ER_AAC_ELD))) {
    ixheaacd_lean_sbrconcealment(ptr_header_data, ptr_sbr_data, ptr_prev_data_ch_0);

    ixheaacd_process_del_cod_env_data(ptr_header_data, ptr_sbr_data, ptr_prev_data_ch_0);
  } else {
    WORD32 num = ptr_header_data->pstr_freq_band_data->num_sf_bands[HIGH];
    if (ptr_header_data->err_flag_prev && ((!usac_flag) ||
        (audio_object_type == AOT_ER_AAC_ELD))) {
      WORD16 *ptr1, *ptr2;
      WORD32 i;

      err = ixheaacd_wrong_timing_compensate(ptr_header_data, ptr_sbr_data,
                                             ptr_prev_data_ch_0,
                                             pstr_common_tables);

      if (err) return err;

      if (ptr_sbr_data->coupling_mode !=
          (WORD16)ptr_prev_data_ch_0->coupling_mode) {
        if (ptr_prev_data_ch_0->coupling_mode == COUPLING_BAL) {
          memcpy(ptr_prev_data_ch_0->sfb_nrg_prev,
                 ptr_prev_data_ch_1->sfb_nrg_prev, sizeof(WORD16) * num);
        } else {
          if (ptr_sbr_data->coupling_mode == COUPLING_LEVEL) {
            ptr1 = ptr_prev_data_ch_0->sfb_nrg_prev;
            ptr2 = ptr_prev_data_ch_1->sfb_nrg_prev;

            for (i = 0; i < num; i++) {
              *ptr1 = (add16_m(*ptr1, *ptr2) >> 1);
              ptr2++;
              ptr1++;
            }
          } else {
            if (ptr_sbr_data->coupling_mode == COUPLING_BAL) {
              memset(ptr_prev_data_ch_0->sfb_nrg_prev, SBR_ENERGY_PAN_OFFSET,
                     sizeof(WORD16) * num);
            }
          }
        }
      }
    }

    memcpy(env_sf_local_arr, ptr_prev_data_ch_0->sfb_nrg_prev,
           sizeof(WORD16) * MAX_FREQ_COEFFS);

    ixheaacd_process_del_cod_env_data(ptr_header_data, ptr_sbr_data,
                                      ptr_prev_data_ch_0);

    if ((!usac_flag)||(audio_object_type == AOT_ER_AAC_ELD)) {
      error_code = ixheaacd_check_env_data(ptr_header_data, ptr_sbr_data,
                                           ptr_prev_data_ch_0);

      if (error_code) {
        ptr_header_data->err_flag = 1;

        memcpy(ptr_prev_data_ch_0->sfb_nrg_prev, env_sf_local_arr,
               sizeof(WORD16) * MAX_FREQ_COEFFS);

        err = ixheaacd_dec_envelope(ptr_header_data, ptr_sbr_data, ptr_prev_data_ch_0,
                                    ptr_prev_data_ch_1, pstr_common_tables, audio_object_type,
                                    ec_flag);

        if (err) return err;
        return 0;
      }
    }
  }
  if (ec_flag) {
    if ((!ptr_header_data->usac_flag) || (audio_object_type == AOT_ER_AAC_ELD)) {
      ixheaacd_dequant_env_data(ptr_sbr_data, ptr_sbr_data->amp_res);
    }
  } else {
    if ((!usac_flag) || (audio_object_type == AOT_ER_AAC_ELD)) {
      ixheaacd_dequant_env_data(ptr_sbr_data, ptr_sbr_data->amp_res);
    }
  }

  return 0;
}

VOID ixheaacd_adj_timeslot(WORD32 *ptr_buf_real, WORD32 *ptr_buf_imag,
                           WORD16 *ptr_filt_buf, WORD16 *ptr_filt_buf_noise,
                           WORD16 *ptr_gain_buf, WORD16 *ptr_noise_floor,
                           WORD16 *ptr_sine_lvl_buf, WORD16 noise_floor_exp,
                           WORD16 *ptr_harm_index, WORD16 sub_band_start,
                           WORD16 num_sub_bands, WORD16 scale_change,
                           WORD16 smooth_ratio, FLAG num_noise_flg,
                           WORD16 *ptr_phase_index,
                           ia_sbr_tables_struct *ptr_sbr_tables) {
  WORD16 k;
  WORD16 *ptr_smoothed_gain, *ptr_smoothed_noise;
  WORD16 direct_ratio;
  WORD32 index = *ptr_phase_index;
  WORD32 harm_idx = *ptr_harm_index;
  WORD32 freq_inv_flag;
  const WORD32 *ptr_rand_ph_buf;
  WORD32 factor = 0;

  direct_ratio = ixheaac_sub16_sat(0x7fff, smooth_ratio);
  freq_inv_flag = (sub_band_start & 1);

  scale_change = scale_change - 1;

  ptr_rand_ph_buf = &ptr_sbr_tables->sbr_rand_ph[index];
  *ptr_phase_index =
      (WORD16)((index + num_sub_bands) & (SBR_NF_NO_RANDOM_VAL - 1));

  if (smooth_ratio) {
    WORD16 *ptr_filt_buf_local = &ptr_filt_buf[0];
    WORD16 *ptr_gain_buf_local = &ptr_gain_buf[0];
    WORD16 *ptr_filt_noise_local = &ptr_filt_buf_noise[0];
    WORD16 *ptr_noise_floor_local = &ptr_noise_floor[0];

    WORD16 tmp, tmp1;

    for (k = 0; k < num_sub_bands; k++) {
      tmp = add16_m(mult16x16_16(smooth_ratio, *ptr_filt_buf_local),
                    mult16x16_16(direct_ratio, *ptr_gain_buf_local++));

      ptr_gain_buf_local++;

      tmp1 = add16_m(mult16x16_16(smooth_ratio, *ptr_filt_noise_local),
                     mult16x16_16(direct_ratio, *ptr_noise_floor_local++));

      ptr_noise_floor_local++;

      *ptr_filt_buf_local++ = tmp << 1;
      ptr_filt_buf_local++;
      *ptr_filt_noise_local++ = tmp1 << 1;
    }
    ptr_smoothed_gain = ptr_filt_buf;
    ptr_smoothed_noise = ptr_filt_buf_noise;
    factor = 1;
  } else {
    ptr_smoothed_gain = ptr_gain_buf;
    ptr_smoothed_noise = ptr_noise_floor;
    factor = 2;
  }

  switch (harm_idx) {
    case 0:
    case 2:
      ixheaacd_harm_idx_zerotwo(num_noise_flg, num_sub_bands, ptr_buf_real,
                                ptr_buf_imag, ptr_smoothed_gain,
                                ptr_smoothed_noise, factor, ptr_gain_buf,
                                scale_change, ptr_rand_ph_buf, ptr_sine_lvl_buf,
                                noise_floor_exp, harm_idx);
      break;
    case 1:
    case 3:
      ixheaacd_harm_idx_onethree(
          num_noise_flg, num_sub_bands, ptr_buf_real, ptr_buf_imag,
          ptr_smoothed_gain, ptr_smoothed_noise, factor, ptr_gain_buf,
          scale_change, ptr_rand_ph_buf, ptr_sine_lvl_buf, noise_floor_exp,
          freq_inv_flag, harm_idx);
      break;
  }
  *ptr_harm_index = (WORD16)((harm_idx + 1) & 3);
}
