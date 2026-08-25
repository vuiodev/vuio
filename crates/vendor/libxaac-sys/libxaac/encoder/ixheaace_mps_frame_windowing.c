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

#include <stdlib.h>
#include <string.h>
#include "ixheaac_type_def.h"
#include "ixheaac_error_standards.h"
#include "ixheaace_error_codes.h"
#include "ixheaace_mps_common_fix.h"
#include "ixheaace_mps_defines.h"

#include "ixheaace_mps_common_define.h"
#include "ixheaace_mps_bitstream.h"
#include "ixheaace_mps_frame_windowing.h"

static IA_ERRORCODE ixheaace_mps_212_frame_window_list_limit(
    ixheaace_mps_pstr_frame_win_list const pstr_frame_win_list, const WORD32 lower_limit,
    const WORD32 upper_limit) {
  IA_ERRORCODE error = IA_NO_ERROR;
  WORD32 list_cnt = 0;
  for (list_cnt = 0; list_cnt < pstr_frame_win_list->win_list_cnt; list_cnt++) {
    if (pstr_frame_win_list->dat[list_cnt].slot < lower_limit ||
        pstr_frame_win_list->dat[list_cnt].slot > upper_limit) {
      if (list_cnt == MAX_NUM_PARAMS - 1) {
        pstr_frame_win_list->dat[list_cnt].hold = IXHEAACE_MPS_FRAME_WINDOWING_INTP;
        pstr_frame_win_list->dat[list_cnt].slot = -1;
      } else {
        WORD32 param = 0;
        for (param = list_cnt; param < MAX_NUM_PARAMS - 1; param++) {
          pstr_frame_win_list->dat[param] = pstr_frame_win_list->dat[param + 1];
        }
      }

      pstr_frame_win_list->win_list_cnt--;
      --list_cnt;
    }
  }
  return error;
}

static IA_ERRORCODE ixheaace_mps_212_frame_window_list_add(
    ixheaace_mps_pstr_frame_win_list const pstr_frame_win_list, const WORD32 slot,
    const WORD32 hold) {
  IA_ERRORCODE error = IA_NO_ERROR;

  if (pstr_frame_win_list->win_list_cnt >= MAX_NUM_PARAMS) {
    return IA_EXHEAACE_CONFIG_NONFATAL_MPS_PARAM_ERROR;
  } else if (pstr_frame_win_list->win_list_cnt > 0 &&
             pstr_frame_win_list->dat[pstr_frame_win_list->win_list_cnt - 1].slot - slot > 0) {
    return IA_EXHEAACE_CONFIG_NONFATAL_MPS_PARAM_ERROR;
  } else {
    pstr_frame_win_list->dat[pstr_frame_win_list->win_list_cnt].hold = hold;
    pstr_frame_win_list->dat[pstr_frame_win_list->win_list_cnt].slot = slot;
    pstr_frame_win_list->win_list_cnt++;
  }
  return error;
}

IA_ERRORCODE
ixheaace_mps_212_frame_window_init(
    ixheaace_mps_pstr_frame_win pstr_frame_win,
    const ixheaace_mps_pstr_frame_win_config pstr_frame_win_config) {
  IA_ERRORCODE error = IA_NO_ERROR;
  WORD32 slot;
  WORD32 time_slots = pstr_frame_win_config->num_time_slots_max;
  pstr_frame_win->frame_keep_flag = pstr_frame_win_config->frame_keep_flag;
  pstr_frame_win->num_time_slots_max = pstr_frame_win_config->num_time_slots_max;
  pstr_frame_win->start_slope = 0;
  pstr_frame_win->start_rect = time_slots >> 1;
  pstr_frame_win->stop_slope = ((3 * time_slots) >> 1) - 1;
  pstr_frame_win->stop_rect = time_slots;
  for (slot = 0; slot<time_slots>> 1; slot++) {
    pstr_frame_win->p_tapper_sync_flt[slot] = (FLOAT32)slot / time_slots;
  }
  pstr_frame_win->p_tapper_sync_flt[time_slots >> 1] = 1.0f;
  pstr_frame_win->taper_syn_len = time_slots >> 1;
  pstr_frame_win->taper_ana_len = pstr_frame_win->start_rect - pstr_frame_win->start_slope;
  for (slot = 0; slot < pstr_frame_win->taper_ana_len; slot++) {
    pstr_frame_win->p_taper_ana_flt[slot] = 1.0f;
  }

  return error;
}

IA_ERRORCODE ixheaace_mps_212_frame_window_get_window(
    ixheaace_mps_pstr_frame_win pstr_frame_win, WORD32 tr_pos[MAX_NUM_PARAMS],
    const WORD32 time_slots, ixheaace_mps_framing_info *const pstr_framing_info,
    FLOAT32 *ptr_window_ana[MAX_NUM_PARAMS],
    ixheaace_mps_pstr_frame_win_list const pstr_frame_win_list, const WORD32 avoid_keep) {
  IA_ERRORCODE error = IA_NO_ERROR;
  WORD32 win_cnt = 0;
  WORD32 w, ps;
  WORD32 idx;
  WORD32 param;
  WORD32 start_slope = pstr_frame_win->start_slope;
  WORD32 start_rect = pstr_frame_win->start_rect;
  WORD32 stop_slope = pstr_frame_win->stop_slope;
  WORD32 stop_rect = pstr_frame_win->stop_rect;
  WORD32 taper_ana_len = pstr_frame_win->taper_ana_len;
  FLOAT32 apply_right_window_gain[MAX_NUM_PARAMS];
  FLOAT32 *p_taper_ana = pstr_frame_win->p_taper_ana_flt;

  if ((time_slots > pstr_frame_win->num_time_slots_max) || (time_slots < 0)) {
    return IA_EXHEAACE_CONFIG_NONFATAL_MPS_INVALID_CONFIG;
  }

  pstr_frame_win_list->win_list_cnt = 0;
  for (param = 0; param < MAX_NUM_PARAMS; param++) {
    pstr_frame_win_list->dat[param].slot = -1;
    pstr_frame_win_list->dat[param].hold = IXHEAACE_MPS_FRAME_WINDOWING_INTP;
  }
  memset(apply_right_window_gain, 0, sizeof(apply_right_window_gain));
  if (tr_pos[0] <= 0) {
    win_cnt = 0;
    error = ixheaace_mps_212_frame_window_list_add(pstr_frame_win_list, time_slots - 1,
                                                   IXHEAACE_MPS_FRAME_WINDOWING_INTP);
    if (error) {
      return error;
    }
    for (idx = 0; idx < start_slope; idx++) {
      ptr_window_ana[win_cnt][idx] = 0.0f;
    }
    for (idx = 0; idx < taper_ana_len; idx++) {
      ptr_window_ana[win_cnt][start_slope + idx] = p_taper_ana[idx];
    }

    for (idx = 0; idx < time_slots - start_rect; idx++) {
      ptr_window_ana[win_cnt][start_rect + idx] = 1.0f;
    }

    apply_right_window_gain[win_cnt] = 1.0f;
    win_cnt++;
  } else {
    WORD32 p_l = tr_pos[0];
    win_cnt = 0;
    error = ixheaace_mps_212_frame_window_list_add(pstr_frame_win_list, p_l - 1,
                                                   IXHEAACE_MPS_FRAME_WINDOWING_HOLD);
    if (error) {
      return error;
    }
    error = ixheaace_mps_212_frame_window_list_add(pstr_frame_win_list, p_l,
                                                   IXHEAACE_MPS_FRAME_WINDOWING_INTP);
    if (error) {
      return error;
    }

    error = ixheaace_mps_212_frame_window_list_limit(pstr_frame_win_list, 0, time_slots - 1);
    if (error) {
      return error;
    }

    error = ixheaace_mps_212_frame_window_list_add(pstr_frame_win_list, time_slots - 1,
                                                   IXHEAACE_MPS_FRAME_WINDOWING_HOLD);
    if (error) {
      return error;
    }

    if (pstr_frame_win_list->win_list_cnt > MAX_NUM_PARAMS) {
      return IA_EXHEAACE_CONFIG_NONFATAL_MPS_INVALID_CONFIG;
    }
    for (ps = 0; ps < pstr_frame_win_list->win_list_cnt - 1; ps++) {
      if (IXHEAACE_MPS_FRAME_WINDOWING_HOLD != pstr_frame_win_list->dat[ps].hold) {
        WORD32 start = pstr_frame_win_list->dat[ps].slot;
        WORD32 stop = pstr_frame_win_list->dat[ps + 1].slot;

        for (idx = 0; idx < start; idx++) {
          ptr_window_ana[win_cnt][idx] = 0.0f;
        }
        for (idx = 0; idx < stop - start + 1; idx++) {
          ptr_window_ana[win_cnt][start + idx] = 1.0f;
        }
        for (idx = 0; idx < time_slots - stop - 1; idx++) {
          ptr_window_ana[win_cnt][stop + 1 + idx] = 0.0f;
        }

        apply_right_window_gain[win_cnt] = ptr_window_ana[win_cnt][time_slots - 1];
        win_cnt++;
      }
    }

    if (pstr_frame_win_list->win_list_cnt - 1 < 0 ||
        pstr_frame_win_list->win_list_cnt - 1 >= MAX_NUM_PARAMS) {
      return IA_EXHEAACE_CONFIG_NONFATAL_MPS_PARAM_ERROR;
    } else if (pstr_frame_win_list->win_list_cnt > 0) {
      if (pstr_frame_win_list->win_list_cnt - 1 == MAX_NUM_PARAMS - 1) {
        pstr_frame_win_list->dat[pstr_frame_win_list->win_list_cnt - 1].slot = -1;
        pstr_frame_win_list->dat[pstr_frame_win_list->win_list_cnt - 1].hold =
            IXHEAACE_MPS_FRAME_WINDOWING_INTP;
      } else {
        for (param = pstr_frame_win_list->win_list_cnt - 1; param < MAX_NUM_PARAMS - 1; param++) {
          pstr_frame_win_list->dat[param] = pstr_frame_win_list->dat[param + 1];
        }
      }
      pstr_frame_win_list->win_list_cnt--;
    }
    if (error) {
      return error;
    }
  }

  for (w = 0; w < win_cnt; w++) {
    if (apply_right_window_gain[w] <= 0) {
      for (idx = 0; idx < time_slots; idx++) {
        ptr_window_ana[w][time_slots + idx] = 0.0f;
      }
    } else {
      if (tr_pos[1] < 0) {
        for (idx = 0; idx < stop_rect - time_slots + 1; idx++) {
          ptr_window_ana[w][time_slots + idx] = 1.0f;
        }
        for (idx = 0; idx < taper_ana_len; idx++) {
          ptr_window_ana[w][stop_rect + idx] = p_taper_ana[taper_ana_len - 1 - idx];
        }
        for (idx = 0; idx < 2 * time_slots - stop_slope - 1; idx++) {
          ptr_window_ana[w][stop_slope + 1 + idx] = 0.0f;
        }
      } else {
        WORD32 p_r = tr_pos[1];
        for (idx = 0; idx < p_r - time_slots; idx++) {
          ptr_window_ana[w][time_slots + idx] = 1.0f;
        }
        for (idx = 0; idx < 2 * time_slots - p_r; idx++) {
          ptr_window_ana[w][p_r + idx] = 0.0f;
        }
      }
      if (apply_right_window_gain[w] < 1.0f) {
        WORD32 slot;
        for (slot = 0; slot < time_slots; slot++) {
          ptr_window_ana[w][time_slots + slot] =
              ptr_window_ana[w][time_slots + slot] * apply_right_window_gain[w];
        }
      }
    }
  }
  if (pstr_frame_win->frame_keep_flag == 1) {
    for (idx = 0; idx < time_slots; idx++) {
      ptr_window_ana[0][2 * time_slots + idx] = ptr_window_ana[0][time_slots + idx];
      ptr_window_ana[0][time_slots + idx] = ptr_window_ana[0][idx];
    }
    if (avoid_keep == 0) {
      for (idx = 0; idx < time_slots; idx++) {
        ptr_window_ana[0][idx] = 1.0f;
      }
    } else {
      for (idx = 0; idx < time_slots; idx++) {
        ptr_window_ana[0][idx] = 0.0f;
      }
    }
  }
  pstr_framing_info->num_param_sets = pstr_frame_win_list->win_list_cnt;
  pstr_framing_info->bs_framing_type = 1;
  for (ps = 0; ps < pstr_framing_info->num_param_sets; ps++) {
    pstr_framing_info->bs_param_slots[ps] = pstr_frame_win_list->dat[ps].slot;
  }
  if ((pstr_framing_info->num_param_sets == 1) &&
      (pstr_framing_info->bs_param_slots[0] == time_slots - 1)) {
    pstr_framing_info->bs_framing_type = 0;
  }
  return error;
}

VOID ixheaace_mps_212_analysis_windowing(
    const WORD32 num_time_slots, const WORD32 start_time_slot, FLOAT32 *ptr_window_ana,
    ixheaace_cmplx_str pp_cmplx_data_in[MAX_ANA_TIME_SLOT][MAX_QMF_BANDS],
    ixheaace_cmplx_str pp_cmplx_data_out[MAX_ANA_TIME_SLOT][MAX_QMF_BANDS],
    const WORD32 num_hybrid_bands) {
  WORD32 band, slot;
  FLOAT32 win;

  for (slot = start_time_slot; slot < num_time_slots; slot++) {
    win = ptr_window_ana[slot];
    if (win != 1.0f) {
      for (band = 0; band < num_hybrid_bands; band++) {
        pp_cmplx_data_out[slot][band].re = win * pp_cmplx_data_in[slot][band].re;
        pp_cmplx_data_out[slot][band].im = win * pp_cmplx_data_in[slot][band].im;
      }
    } else {
      for (band = 0; band < num_hybrid_bands; band++) {
        pp_cmplx_data_out[slot][band].re = pp_cmplx_data_in[slot][band].re;
        pp_cmplx_data_out[slot][band].im = pp_cmplx_data_in[slot][band].im;
      }
    }
  }
}
