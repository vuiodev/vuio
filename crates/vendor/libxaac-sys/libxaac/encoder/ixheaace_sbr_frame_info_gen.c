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
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops16.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaac_basic_ops.h"

#include "ixheaac_error_standards.h"
#include "ixheaace_error_codes.h"

#include "ixheaace_sbr_header.h"
#include "ixheaace_sbr_def.h"
#include "ixheaace_resampler.h"
#include "ixheaace_sbr_rom.h"
#include "ixheaace_common_rom.h"
#include "ixheaace_sbr_main.h"
#include "ixheaace_sbr_frame_info_gen.h"
#include "ixheaace_sbr_misc.h"

static VOID ixheaace_add_freq_left(ixheaace_freq_res *ptr_vector, WORD32 *ptr_length_vector,
                                   ixheaace_freq_res value) {
  memmove(ptr_vector + 1, ptr_vector, *ptr_length_vector * sizeof(ptr_vector[0]));
  ptr_vector[0] = value;
  (*ptr_length_vector)++;
}

static VOID ixheaace_add_freq_vec_left(ixheaace_freq_res *ptr_dst, WORD32 *length_dst,
                                       ixheaace_freq_res *ptr_src, WORD32 length_src) {
  WORD32 i;
  i = length_src - 1;
  while (i >= 0) {
    ixheaace_add_freq_left(ptr_dst, length_dst, ptr_src[i]);
    i--;
  }
}

static VOID ixheaace_add_freq_right(ixheaace_freq_res *ptr_vector, WORD32 *ptr_length_vector,
                                    ixheaace_freq_res value) {
  ptr_vector[*ptr_length_vector] = value;

  (*ptr_length_vector)++;
}

static VOID ixheaace_fill_frame_tran(WORD32 *ptr_v_bord, WORD32 *ptr_length_v_bord,
                                     ixheaace_freq_res *ptr_v_freq, WORD32 *ptr_length_v_freq,
                                     WORD32 *ptr_bmin, WORD32 *ptr_bmax, WORD32 tran,
                                     WORD32 *ptr_ptr_v_tuning_segm,
                                     ixheaace_freq_res *ptr_tuning_freq) {
  WORD32 bord, i;

  *ptr_length_v_bord = 0;
  *ptr_length_v_freq = 0;

  if (ptr_ptr_v_tuning_segm[0]) {
    ixheaace_add_right(ptr_v_bord, ptr_length_v_bord, (tran - ptr_ptr_v_tuning_segm[0]));

    ixheaace_add_freq_right(ptr_v_freq, ptr_length_v_freq, ptr_tuning_freq[0]);
  }

  bord = tran;

  ixheaace_add_right(ptr_v_bord, ptr_length_v_bord, tran);

  if (ptr_ptr_v_tuning_segm[1]) {
    bord += ptr_ptr_v_tuning_segm[1];

    ixheaace_add_right(ptr_v_bord, ptr_length_v_bord, bord);

    ixheaace_add_freq_right(ptr_v_freq, ptr_length_v_freq, ptr_tuning_freq[1]);
  }

  if (ptr_ptr_v_tuning_segm[2] != 0) {
    bord += ptr_ptr_v_tuning_segm[2];

    ixheaace_add_right(ptr_v_bord, ptr_length_v_bord, bord);

    ixheaace_add_freq_right(ptr_v_freq, ptr_length_v_freq, ptr_tuning_freq[2]);
  }

  ixheaace_add_freq_right(ptr_v_freq, ptr_length_v_freq, FREQ_RES_HIGH);

  *ptr_bmin = ptr_v_bord[0];
  *ptr_bmax = ptr_v_bord[0];

  for (i = 0; i < *ptr_length_v_bord; i++) {
    if (ptr_v_bord[i] < *ptr_bmin) {
      *ptr_bmin = ptr_v_bord[i];
    }
    if (ptr_v_bord[i] > *ptr_bmax) {
      *ptr_bmax = ptr_v_bord[i];
    }
  }
}

static VOID ixheaace_fill_frame_pre(WORD32 dmax, WORD32 *ptr_v_bord, WORD32 *ptr_length_v_bord,
                                    ixheaace_freq_res *ptr_v_freq, WORD32 *ptr_length_v_freq,
                                    WORD32 bmin, WORD32 rest) {
  WORD32 ptr_parts, ptr_d, j, S, s = 0, segm, bord;

  ptr_parts = 1;
  ptr_d = rest;

  while (ptr_d > dmax) {
    ptr_parts++;

    segm = rest / ptr_parts;

    S = ixheaac_shr32((segm - 2), 1);

    s = ixheaac_min32(8, 2 * S + 2);

    ptr_d = rest - (ptr_parts - 1) * s;
  }

  bord = bmin;
  j = 0;
  while (j <= ptr_parts - 2) {
    bord = bord - s;

    ixheaace_add_left(ptr_v_bord, ptr_length_v_bord, bord);

    ixheaace_add_freq_left(ptr_v_freq, ptr_length_v_freq, FREQ_RES_HIGH);
    j++;
  }
}

static VOID ixheaace_fill_Frame_Post(WORD32 *ptr_parts, WORD32 *ptr_d, WORD32 dmax,
                                     WORD32 *ptr_v_bord, WORD32 *ptr_length_v_bord,
                                     ixheaace_freq_res *ptr_v_freq, WORD32 *ptr_length_v_freq,
                                     WORD32 bmax, WORD32 fmax) {
  WORD32 j, rest, segm, S, s = 0, bord;

  rest = 32 - bmax;

  *ptr_d = rest;

  if (*ptr_d > 0) {
    *ptr_parts = 1; /* start with one envelope */

    while (*ptr_d > dmax) {
      *ptr_parts = *ptr_parts + 1;

      segm = rest / (*ptr_parts);

      S = ixheaac_shr32((segm - 2), 1);

      s = ixheaac_min32(fmax, 2 * S + 2);

      *ptr_d = rest - (*ptr_parts - 1) * s;
    }

    bord = bmax;

    for (j = 0; j <= *ptr_parts - 2; j++) {
      bord += s;

      ixheaace_add_right(ptr_v_bord, ptr_length_v_bord, bord);

      ixheaace_add_freq_right(ptr_v_freq, ptr_length_v_freq, FREQ_RES_HIGH);
    }
  } else {
    *ptr_parts = 1;

    *ptr_length_v_bord = *ptr_length_v_bord - 1;
    *ptr_length_v_freq = *ptr_length_v_freq - 1;
  }
}

static VOID ixheaace_fill_frame_inter(WORD32 *ptr_n_l, WORD32 *ptr_ptr_v_tuning_segm,
                                      WORD32 *ptr_v_bord, WORD32 *ptr_length_v_bord, WORD32 bmin,
                                      ixheaace_freq_res *ptr_v_freq, WORD32 *ptr_length_v_freq,
                                      WORD32 *ptr_v_bord_follow, WORD32 *ptr_length_v_bord_follow,
                                      ixheaace_freq_res *ptr_v_freq_follow,
                                      WORD32 *ptr_length_v_freq_follow, WORD32 fill_follow,
                                      WORD32 dmin, WORD32 dmax) {
  WORD32 middle, b_new, num_bord_follow, bord_max_follow;

  if (fill_follow >= 1) {
    *ptr_length_v_bord_follow = fill_follow;
    *ptr_length_v_freq_follow = fill_follow;
  }

  num_bord_follow = *ptr_length_v_bord_follow;

  bord_max_follow = ptr_v_bord_follow[num_bord_follow - 1];

  middle = bmin - bord_max_follow;

  while (middle < 0) {
    num_bord_follow--;

    bord_max_follow = ptr_v_bord_follow[num_bord_follow - 1];

    middle = bmin - bord_max_follow;
  }

  *ptr_length_v_bord_follow = num_bord_follow;
  *ptr_length_v_freq_follow = num_bord_follow;

  *ptr_n_l = num_bord_follow - 1;

  b_new = *ptr_length_v_bord;

  if ((middle <= dmax) && (middle >= dmin)) {
    ixheaace_add_vec_left(ptr_v_bord, ptr_length_v_bord, ptr_v_bord_follow,
                          *ptr_length_v_bord_follow);

    ixheaace_add_freq_vec_left(ptr_v_freq, ptr_length_v_freq, ptr_v_freq_follow,
                               *ptr_length_v_freq_follow);
  }

  else if ((middle <= dmax) && (middle < dmin)) {
    if (ptr_ptr_v_tuning_segm[0] != 0) {
      *ptr_length_v_bord = b_new - 1;

      ixheaace_add_vec_left(ptr_v_bord, ptr_length_v_bord, ptr_v_bord_follow,
                            *ptr_length_v_bord_follow);

      *ptr_length_v_freq = b_new - 1;

      ixheaace_add_freq_vec_left(ptr_v_freq + 1, ptr_length_v_freq, ptr_v_freq_follow,
                                 *ptr_length_v_freq_follow);
    } else {
      if (*ptr_length_v_bord_follow > 1) {
        ixheaace_add_vec_left(ptr_v_bord, ptr_length_v_bord, ptr_v_bord_follow,
                              *ptr_length_v_bord_follow - 1);

        ixheaace_add_freq_vec_left(ptr_v_freq, ptr_length_v_freq, ptr_v_freq_follow,
                                   *ptr_length_v_bord_follow - 1);

        *ptr_n_l = *ptr_n_l - 1;
      } else {
        memmove(ptr_v_bord, ptr_v_bord + 1, (*ptr_length_v_bord - 1) * sizeof(*ptr_v_bord));

        memmove(ptr_v_freq, ptr_v_freq + 1, (*ptr_length_v_freq - 1) * sizeof(*ptr_v_freq));
        *ptr_length_v_bord = b_new - 1;
        *ptr_length_v_freq = b_new - 1;

        ixheaace_add_vec_left(ptr_v_bord, ptr_length_v_bord, ptr_v_bord_follow,
                              *ptr_length_v_bord_follow);

        ixheaace_add_freq_vec_left(ptr_v_freq, ptr_length_v_freq, ptr_v_freq_follow,
                                   *ptr_length_v_freq_follow);
      }
    }
  } else {
    ixheaace_fill_frame_pre(dmax, ptr_v_bord, ptr_length_v_bord, ptr_v_freq, ptr_length_v_freq,
                            bmin, middle);

    ixheaace_add_vec_left(ptr_v_bord, ptr_length_v_bord, ptr_v_bord_follow,
                          *ptr_length_v_bord_follow);

    ixheaace_add_freq_vec_left(ptr_v_freq, ptr_length_v_freq, ptr_v_freq_follow,
                               *ptr_length_v_freq_follow);
  }
}

static VOID ixheaace_calc_frame_class(ixheaace_frame_class *ptr_frame_type,
                                      ixheaace_frame_class *ptr_frame_type_old, WORD32 tran_flag,
                                      WORD32 *ptr_spread_flag, WORD32 is_ld_sbr) {
  if (0 == is_ld_sbr) {
    switch (*ptr_frame_type_old) {
      case IXHEAACE_FIXFIX:

        *ptr_frame_type = (tran_flag ? IXHEAACE_FIXVAR : IXHEAACE_FIXFIX);
        break;

      case IXHEAACE_FIXVAR:

        if (tran_flag) {
          *ptr_frame_type = IXHEAACE_VARVAR;
          *ptr_spread_flag = 0;
        } else {
          *ptr_frame_type = (*ptr_spread_flag ? IXHEAACE_VARVAR : IXHEAACE_VARFIX);
        }
        break;

      case IXHEAACE_VARFIX:

        *ptr_frame_type = (tran_flag ? IXHEAACE_FIXVAR : IXHEAACE_FIXFIX);
        break;

      case IXHEAACE_VARVAR:

        if (tran_flag) {
          *ptr_frame_type = IXHEAACE_VARVAR;
          *ptr_spread_flag = 0;
        } else {
          *ptr_frame_type = (*ptr_spread_flag ? IXHEAACE_VARVAR : IXHEAACE_VARFIX);
        }
        break;
      default:
        break;
    }
  } else {
    *ptr_frame_type = (tran_flag ? IXHEAACE_LD_TRAN : IXHEAACE_FIXFIX);
  }
  *ptr_frame_type_old = *ptr_frame_type;
}

static VOID ixheaace_special_case(WORD32 *ptr_spread_flag, WORD32 allow_spread,
                                  WORD32 *ptr_a_borders, WORD32 *ptr_border_vec_len,
                                  ixheaace_freq_res *ptr_freq_res, WORD32 *ptr_freq_res_vec_len,
                                  WORD32 *ptr_num_parts, WORD32 ptr_d) {
  WORD32 L;

  L = *ptr_border_vec_len;

  if (allow_spread) {
    *ptr_spread_flag = 1;

    ixheaace_add_right(ptr_a_borders, ptr_border_vec_len, ptr_a_borders[L - 1] + 8);

    ixheaace_add_freq_right(ptr_freq_res, ptr_freq_res_vec_len, FREQ_RES_HIGH);

    (*ptr_num_parts)++;
  } else {
    if (ptr_d == 1) {
      *ptr_border_vec_len = L - 1;
      *ptr_freq_res_vec_len = L - 1;
    } else {
      if ((ptr_a_borders[L - 1] - ptr_a_borders[L - 2]) > 2) {
        ptr_a_borders[L - 1] = ptr_a_borders[L - 1] - 2;

        ptr_freq_res[*ptr_freq_res_vec_len - 1] = FREQ_RES_LOW;
      }
    }
  }
}

static VOID ixheaace_calc_cmon_border(WORD32 *ptr_cmon_border_idx, WORD32 *ptr_tran_idx,
                                      WORD32 *ptr_a_borders, WORD32 *ptr_border_vec_len,
                                      WORD32 tran

) {
  WORD32 i;

  for (i = 0; i < *ptr_border_vec_len; i++) {
    if (ptr_a_borders[i] >= 16) {
      *ptr_cmon_border_idx = i;
      break;
    }
  }
  i = 0;
  while (i < *ptr_border_vec_len) {
    if (ptr_a_borders[i] >= tran) {
      *ptr_tran_idx = i;
      break;
    } else {
      *ptr_tran_idx = IXHEAACE_EMPTY;
    }
    i++;
  }
}

static VOID ixheaace_keep_for_follow_up(WORD32 *ptr_a_borders_follow,
                                        WORD32 *ptr_border_vec_len_follow,
                                        ixheaace_freq_res *ptr_a_freq_res_follow,
                                        WORD32 *ptr_freq_res_vec_len_follow,
                                        WORD32 *ptr_tran_idx_follow, WORD32 *ptr_fill_idx_follow,
                                        WORD32 *ptr_a_borders, WORD32 *ptr_border_vec_len,
                                        ixheaace_freq_res *ptr_freq_res, WORD32 cmon_border_idx,
                                        WORD32 tran_idx, WORD32 num_parts

) {
  WORD32 L, i, j;

  L = *ptr_border_vec_len;

  (*ptr_border_vec_len_follow) = 0;
  (*ptr_freq_res_vec_len_follow) = 0;

  j = 0, i = cmon_border_idx;

  while (i < L) {
    ptr_a_borders_follow[j] = ptr_a_borders[i] - 16;

    ptr_a_freq_res_follow[j] = ptr_freq_res[i];

    (*ptr_border_vec_len_follow)++;
    (*ptr_freq_res_vec_len_follow)++;
    i++, j++;
  }

  *ptr_tran_idx_follow =
      (tran_idx != IXHEAACE_EMPTY ? tran_idx - cmon_border_idx : IXHEAACE_EMPTY);

  *ptr_fill_idx_follow = L - (num_parts - 1) - cmon_border_idx;
}

static VOID ixheaace_calc_ctrl_signal(ixheaace_pstr_sbr_grid pstr_sbr_grid,
                                      ixheaace_frame_class frame_type, WORD32 *ptr_v_bord,
                                      WORD32 length_v_bord, ixheaace_freq_res *ptr_v_freq,
                                      WORD32 length_v_freq, WORD32 i_cmon, WORD32 i_tran,
                                      WORD32 spread_flag, WORD32 ptr_n_l) {
  WORD32 i, r, a, n, p, b, a_l, a_r, ntot, nmax, n_r;

  ixheaace_freq_res *ptr_v_f = pstr_sbr_grid->v_f;
  ixheaace_freq_res *ptr_v_f_lr = pstr_sbr_grid->v_f_lr;
  WORD32 *ptr_v_r = pstr_sbr_grid->bs_rel_bord;
  WORD32 *ptr_v_rl = pstr_sbr_grid->bs_rel_bord_0;
  WORD32 *ptr_v_rr = pstr_sbr_grid->bs_rel_bord_1;

  WORD32 length_v_r = 0;
  WORD32 length_v_rr = 0;
  WORD32 length_v_rl = 0;

  switch (frame_type) {
    case IXHEAACE_FIXVAR:

      a = ptr_v_bord[i_cmon];

      length_v_r = 0;
      i = i_cmon;

      while (i >= 1) {
        r = ptr_v_bord[i] - ptr_v_bord[i - 1];

        ixheaace_add_right(ptr_v_r, &length_v_r, r);

        i--;
      }

      n = length_v_r;

      for (i = 0; i < i_cmon; i++) {
        ptr_v_f[i] = ptr_v_freq[i_cmon - 1 - i];
      }

      ptr_v_f[i_cmon] = FREQ_RES_HIGH;

      if ((i_cmon >= i_tran) && (i_tran != IXHEAACE_EMPTY)) {
        p = i_cmon - i_tran + 1;
      } else {
        p = 0;
      }

      pstr_sbr_grid->frame_type = frame_type;
      pstr_sbr_grid->bs_abs_bord = a;
      pstr_sbr_grid->n = n;
      pstr_sbr_grid->p = p;

      break;
    case IXHEAACE_VARFIX:

      a = ptr_v_bord[0];

      length_v_r = 0;

      i = 1;
      while (i < length_v_bord) {
        r = ptr_v_bord[i] - ptr_v_bord[i - 1];

        ixheaace_add_right(ptr_v_r, &length_v_r, r);

        i++;
      }

      n = length_v_r;

      memcpy(ptr_v_f, ptr_v_freq, length_v_freq * sizeof(ixheaace_freq_res));

      if ((i_tran >= 0) && (i_tran != IXHEAACE_EMPTY)) {
        p = i_tran + 1;
      } else {
        p = 0;
      }

      pstr_sbr_grid->frame_type = frame_type;
      pstr_sbr_grid->bs_abs_bord = a;
      pstr_sbr_grid->n = n;
      pstr_sbr_grid->p = p;

      break;
    case IXHEAACE_VARVAR:

      if (spread_flag) {
        b = length_v_bord;

        a_l = ptr_v_bord[0];

        a_r = ptr_v_bord[b - 1];

        ntot = b - 2;

        nmax = 2;

        if (ntot > nmax) {
          ptr_n_l = nmax;
          n_r = ntot - nmax;
        } else {
          ptr_n_l = ntot;
          n_r = 0;
        }

        length_v_rl = 0;

        i = 1;
        while (i <= ptr_n_l) {
          r = ptr_v_bord[i] - ptr_v_bord[i - 1];

          ixheaace_add_right(ptr_v_rl, &length_v_rl, r);

          i++;
        }

        length_v_rr = 0;

        i = b - 1;

        while (i >= b - n_r) {
          r = ptr_v_bord[i] - ptr_v_bord[i - 1];

          ixheaace_add_right(ptr_v_rr, &length_v_rr, r);

          i--;
        }

        if ((i_tran > 0) && (i_tran != IXHEAACE_EMPTY)) {
          p = b - i_tran;
        } else {
          p = 0;
        }

        for (i = 0; i < b - 1; i++) {
          ptr_v_f_lr[i] = ptr_v_freq[i];
        }
      } else {
        length_v_bord = i_cmon + 1;

        b = length_v_bord;

        a_l = ptr_v_bord[0];

        a_r = ptr_v_bord[b - 1];

        ntot = b - 2;
        n_r = ntot - ptr_n_l;

        length_v_rl = 0;

        i = 1;
        while (i <= ptr_n_l) {
          r = ptr_v_bord[i] - ptr_v_bord[i - 1];

          ixheaace_add_right(ptr_v_rl, &length_v_rl, r);

          i++;
        }

        length_v_rr = 0;

        i = b - 1;

        while (i >= b - n_r) {
          r = ptr_v_bord[i] - ptr_v_bord[i - 1];

          ixheaace_add_right(ptr_v_rr, &length_v_rr, r);

          i--;
        }

        if ((i_cmon >= i_tran) && (i_tran != IXHEAACE_EMPTY)) {
          p = i_cmon - i_tran + 1;
        } else {
          p = 0;
        }

        for (i = 0; i < b - 1; i++) {
          ptr_v_f_lr[i] = ptr_v_freq[i];
        }
      }

      pstr_sbr_grid->frame_type = frame_type;
      pstr_sbr_grid->bs_abs_bord_0 = a_l;
      pstr_sbr_grid->bs_abs_bord_1 = a_r;
      pstr_sbr_grid->bs_num_rel_0 = ptr_n_l;
      pstr_sbr_grid->bs_num_rel_1 = n_r;
      pstr_sbr_grid->p = p;

      break;

    default:
      break;
  }
}

static VOID ixheaace_create_def_frame_info(ixheaace_pstr_sbr_frame_info pstr_sbr_frame_info,
                                           WORD32 n_env, ixheaace_str_qmf_tabs *ptr_qmf_tab) {
  switch (n_env) {
    case NUM_ENVELOPE_1:

      memcpy(pstr_sbr_frame_info, &(ptr_qmf_tab->frame_info1_2048),
             sizeof(ixheaace_str_frame_info_sbr));

      break;

    case NUM_ENVELOPE_2:

      memcpy(pstr_sbr_frame_info, &(ptr_qmf_tab->frame_info2_2048),
             sizeof(ixheaace_str_frame_info_sbr));

      break;

    case NUM_ENVELOPE_4:

      memcpy(pstr_sbr_frame_info, &(ptr_qmf_tab->frame_info4_2048),
             sizeof(ixheaace_str_frame_info_sbr));

      break;
  }
}
static VOID ixheaace_create_ld_transient_frame_info(
    ixheaace_pstr_sbr_frame_info pstr_sbr_frame_info, ixheaace_pstr_sbr_grid pstr_sbr_grid,
    WORD32 tran_pos, WORD32 num_time_slots, WORD32 low_tran_flag) {
  WORD32 num_env, i, diff;
  const WORD32 *env_tab = NULL;

  switch (num_time_slots)

  {
    case TIME_SLOTS_15:
      env_tab = ixheaace_ld_env_tab_480[tran_pos];
      break;

    default:
      env_tab = ixheaace_ld_env_tab_512[tran_pos];
      break;
  }

  num_env = env_tab[0];

  for (i = 1; i < num_env; i++) {
    pstr_sbr_frame_info->borders[i] = env_tab[i];
  }

  pstr_sbr_frame_info->borders[0] = 0;
  pstr_sbr_frame_info->borders[num_env] = num_time_slots;

  i = 0;
  while (i < num_env) {
    diff = pstr_sbr_frame_info->borders[i + 1] - pstr_sbr_frame_info->borders[i];

    if (low_tran_flag) {
      pstr_sbr_frame_info->freq_res[i] = FREQ_RES_LOW;
    } else {
      pstr_sbr_frame_info->freq_res[i] = (diff <= 5) ? FREQ_RES_LOW : FREQ_RES_HIGH;
    }

    pstr_sbr_grid->v_f[i] = pstr_sbr_frame_info->freq_res[i];

    i++;
  }

  pstr_sbr_frame_info->n_envelopes = num_env;
  pstr_sbr_frame_info->short_env = env_tab[IXHEAACE_SBR_ENVT_TRANIDX];
  pstr_sbr_frame_info->borders_noise[0] = pstr_sbr_frame_info->borders[0];
  pstr_sbr_frame_info->borders_noise[1] = pstr_sbr_frame_info->borders[1];
  pstr_sbr_frame_info->borders_noise[2] = num_time_slots;
  pstr_sbr_frame_info->n_noise_envelopes = 2;
  pstr_sbr_grid->frame_type = IXHEAACE_LD_TRAN;
  pstr_sbr_grid->bs_transient_position = tran_pos;
  pstr_sbr_grid->bs_num_env = num_env;
}
static VOID ixheaace_create_def_frame_480_info(ixheaace_pstr_sbr_frame_info pstr_sbr_frame_info,
                                               WORD32 n_env, ixheaace_str_qmf_tabs *ptr_qmf_tab) {
  switch (n_env) {
    case NUM_ENVELOPE_1:

      memcpy(pstr_sbr_frame_info, &(ptr_qmf_tab->frame_480_info1_2048),
             sizeof(ixheaace_str_frame_info_sbr));

      break;

    case NUM_ENVELOPE_2:

      memcpy(pstr_sbr_frame_info, &(ptr_qmf_tab->frame_480_info2_2048),
             sizeof(ixheaace_str_frame_info_sbr));

      break;

    case NUM_ENVELOPE_4:

      memcpy(pstr_sbr_frame_info, &(ptr_qmf_tab->frame_480_info4_2048),
             sizeof(ixheaace_str_frame_info_sbr));

      break;
  }
}

static IA_ERRORCODE ixheaace_ctrl_signal2_frame_info(
    ixheaace_pstr_sbr_grid pstr_sbr_grid, ixheaace_pstr_sbr_frame_info pstr_sbr_frame_info,
    ixheaace_freq_res freq_res_fix, ixheaace_str_qmf_tabs *ptr_qmf_tab, WORD32 frame_length_480) {
  WORD32 n_env = 0, border = 0, i, k, p;
  WORD32 *ptr_v_r = pstr_sbr_grid->bs_rel_bord;
  ixheaace_freq_res *ptr_v_f = pstr_sbr_grid->v_f;

  ixheaace_frame_class frame_type = pstr_sbr_grid->frame_type;

  switch (frame_type) {
    case IXHEAACE_FIXFIX:
      if (frame_length_480) {
        ixheaace_create_def_frame_480_info(pstr_sbr_frame_info, pstr_sbr_grid->bs_num_env,
                                           ptr_qmf_tab);
      } else {
        ixheaace_create_def_frame_info(pstr_sbr_frame_info, pstr_sbr_grid->bs_num_env,
                                       ptr_qmf_tab);
      }

      if (freq_res_fix == FREQ_RES_LOW) {
        for (i = 0; i < pstr_sbr_frame_info->n_envelopes; i++) {
          pstr_sbr_frame_info->freq_res[i] = FREQ_RES_LOW;
        }
      }
      break;

    case IXHEAACE_FIXVAR:
    case IXHEAACE_VARFIX:

      n_env = pstr_sbr_grid->n + 1;

      if ((n_env <= 0) || (n_env > IXHEAACE_MAX_ENV_FIXVAR_VARFIX)) {
        return IA_EXHEAACE_EXE_FATAL_INVALID_SBR_NUM_ENVELOPES;
      }

      pstr_sbr_frame_info->n_envelopes = n_env;

      border = pstr_sbr_grid->bs_abs_bord;

      pstr_sbr_frame_info->n_noise_envelopes = (n_env == 1 ? 1 : 2);

      break;

    case IXHEAACE_VARVAR:
      break;
    default:
      return IA_EXHEAACE_EXE_FATAL_INVALID_SBR_FRAME_TYPE;
      break;
  }

  switch (frame_type) {
    case IXHEAACE_FIXVAR:

      pstr_sbr_frame_info->borders[0] = 0;

      pstr_sbr_frame_info->borders[n_env] = border;

      k = 0;
      i = n_env - 1;
      while (k < n_env - 1) {
        border -= ptr_v_r[k];

        pstr_sbr_frame_info->borders[i] = border;
        k++;
        i--;
      }

      p = pstr_sbr_grid->p;

      if (p == 0) {
        pstr_sbr_frame_info->short_env = 0;
      } else {
        pstr_sbr_frame_info->short_env = n_env + 1 - p;
      }

      for (k = 0, i = n_env - 1; k < n_env; k++, i--) {
        pstr_sbr_frame_info->freq_res[i] = ptr_v_f[k];
      }

      if (p == 0 || p == 1) {
        pstr_sbr_frame_info->borders_noise[1] = pstr_sbr_frame_info->borders[n_env - 1];
      } else {
        pstr_sbr_frame_info->borders_noise[1] =
            pstr_sbr_frame_info->borders[pstr_sbr_frame_info->short_env];
      }

      break;

    case IXHEAACE_VARFIX:
      pstr_sbr_frame_info->borders[0] = border;

      for (k = 0; k < n_env - 1; k++) {
        border += ptr_v_r[k];

        pstr_sbr_frame_info->borders[k + 1] = border;
      }

      pstr_sbr_frame_info->borders[n_env] = 16;

      p = pstr_sbr_grid->p;

      if (p == 0 || p == 1) {
        pstr_sbr_frame_info->short_env = 0;
      } else {
        pstr_sbr_frame_info->short_env = p - 1;
      }

      for (k = 0; k < n_env; k++) {
        pstr_sbr_frame_info->freq_res[k] = ptr_v_f[k];
      }

      switch (p) {
        case 0:
          pstr_sbr_frame_info->borders_noise[1] = pstr_sbr_frame_info->borders[1];
          break;
        case 1:
          pstr_sbr_frame_info->borders_noise[1] = pstr_sbr_frame_info->borders[n_env - 1];
          break;
        default:
          pstr_sbr_frame_info->borders_noise[1] =
              pstr_sbr_frame_info->borders[pstr_sbr_frame_info->short_env];
          break;
      }
      break;

    case IXHEAACE_VARVAR:
      n_env = pstr_sbr_grid->bs_num_rel_0 + pstr_sbr_grid->bs_num_rel_1 + 1;

      if ((n_env < 1) || (n_env > IXHEAACE_MAX_ENV_VARVAR)) {
        return IA_EXHEAACE_EXE_FATAL_INVALID_SBR_NUM_ENVELOPES;
      }
      pstr_sbr_frame_info->n_envelopes = n_env;

      pstr_sbr_frame_info->borders[0] = border = pstr_sbr_grid->bs_abs_bord_0;

      for (k = 0, i = 1; k < pstr_sbr_grid->bs_num_rel_0; k++, i++) {
        border += pstr_sbr_grid->bs_rel_bord_0[k];

        pstr_sbr_frame_info->borders[i] = border;
      }

      border = pstr_sbr_grid->bs_abs_bord_1;

      pstr_sbr_frame_info->borders[n_env] = border;

      for (k = 0, i = n_env - 1; k < pstr_sbr_grid->bs_num_rel_1; k++, i--) {
        border -= pstr_sbr_grid->bs_rel_bord_1[k];

        pstr_sbr_frame_info->borders[i] = border;
      }

      p = pstr_sbr_grid->p;

      pstr_sbr_frame_info->short_env = (p == 0 ? 0 : n_env + 1 - p);

      for (k = 0; k < n_env; k++) {
        pstr_sbr_frame_info->freq_res[k] = pstr_sbr_grid->v_f_lr[k];
      }

      if (n_env == 1) {
        pstr_sbr_frame_info->n_noise_envelopes = 1;
        pstr_sbr_frame_info->borders_noise[0] = pstr_sbr_grid->bs_abs_bord_0;
        pstr_sbr_frame_info->borders_noise[1] = pstr_sbr_grid->bs_abs_bord_1;
      } else {
        pstr_sbr_frame_info->n_noise_envelopes = 2;
        pstr_sbr_frame_info->borders_noise[0] = pstr_sbr_grid->bs_abs_bord_0;

        if (p == 0 || p == 1) {
          pstr_sbr_frame_info->borders_noise[1] = pstr_sbr_frame_info->borders[n_env - 1];
        } else {
          pstr_sbr_frame_info->borders_noise[1] =
              pstr_sbr_frame_info->borders[pstr_sbr_frame_info->short_env];
        }

        pstr_sbr_frame_info->borders_noise[2] = pstr_sbr_grid->bs_abs_bord_1;
      }
      break;

    default:
      break;
  }

  if (frame_type == IXHEAACE_VARFIX || frame_type == IXHEAACE_FIXVAR) {
    pstr_sbr_frame_info->borders_noise[0] = pstr_sbr_frame_info->borders[0];

    if (n_env == 1) {
      pstr_sbr_frame_info->borders_noise[1] = pstr_sbr_frame_info->borders[n_env];
    } else {
      pstr_sbr_frame_info->borders_noise[2] = pstr_sbr_frame_info->borders[n_env];
    }
  }
  return IA_NO_ERROR;
}

IA_ERRORCODE
ixheaace_frame_info_generator(ixheaace_pstr_sbr_env_frame pstr_sbr_env_frame,
                              WORD32 *ptr_v_pre_transient_info, WORD32 *ptr_v_transient_info,
                              WORD32 *ptr_v_tuning, ixheaace_str_qmf_tabs *ptr_qmf_tab,
                              WORD32 num_time_slots, WORD32 is_ld_sbr,
                              ixheaace_pstr_sbr_frame_info *ptr_frame_info,
                              WORD32 flag_framelength_small) {
  IA_ERRORCODE err_code = IA_NO_ERROR;
  WORD32 num_env, tran = 0, bmin = 0, bmax = 0;
  WORD32 ptr_parts, ptr_d, i_cmon = 0, i_tran = IXHEAACE_EMPTY, ptr_n_l;
  WORD32 fmax = 0;

  WORD32 *ptr_v_bord = pstr_sbr_env_frame->v_bord;
  ixheaace_freq_res *ptr_v_freq = pstr_sbr_env_frame->v_freq;
  WORD32 *ptr_v_bord_follow = pstr_sbr_env_frame->v_bord_follow;
  ixheaace_freq_res *ptr_v_freq_follow = pstr_sbr_env_frame->v_freq_follow;

  WORD32 *ptr_length_v_bord_follow = &pstr_sbr_env_frame->length_v_bord_follow;
  WORD32 *ptr_length_v_freq_follow = &pstr_sbr_env_frame->length_v_freq_follow;
  WORD32 *ptr_length_v_bord = &pstr_sbr_env_frame->length_v_bord;
  WORD32 *ptr_length_v_freq = &pstr_sbr_env_frame->length_v_freq;
  WORD32 *ptr_spread_flag = &pstr_sbr_env_frame->spread_flag;
  WORD32 *ptr_tran_follow = &pstr_sbr_env_frame->i_tran_follow;
  WORD32 *ptr_fill_follow = &pstr_sbr_env_frame->i_fill_follow;
  ixheaace_frame_class *ptr_frame_type_old = &pstr_sbr_env_frame->frame_type_old;
  ixheaace_frame_class frame_type = IXHEAACE_FIXFIX;

  WORD32 allow_spread = pstr_sbr_env_frame->allow_spread;
  WORD32 num_env_static = pstr_sbr_env_frame->num_env_static;
  WORD32 static_framing = pstr_sbr_env_frame->static_framing;
  WORD32 dmin = pstr_sbr_env_frame->dmin;
  WORD32 dmax = pstr_sbr_env_frame->dmax;

  WORD32 tran_pos = ptr_v_transient_info[0];
  WORD32 tran_flag = ptr_v_transient_info[1];

  WORD32 *ptr_ptr_v_tuning_segm = ptr_v_tuning;
  ixheaace_freq_res *ptr_tuning_freq = (ixheaace_freq_res *)(ptr_v_tuning + 3);

  ixheaace_freq_res freq_res_fix = pstr_sbr_env_frame->freq_res_fix;

  if (is_ld_sbr) {
    if ((!tran_flag && ptr_v_pre_transient_info[1]) &&
        (num_time_slots - ptr_v_pre_transient_info[0] < 4)) {
      tran_flag = 1;
      tran_pos = 0;
    }
  }
  if (static_framing) {
    frame_type = IXHEAACE_FIXFIX;
    num_env = num_env_static;
    *ptr_frame_type_old = IXHEAACE_FIXFIX;
    pstr_sbr_env_frame->sbr_grid.bs_num_env = num_env;
    pstr_sbr_env_frame->sbr_grid.frame_type = frame_type;
  } else {
    ixheaace_calc_frame_class(&frame_type, ptr_frame_type_old, tran_flag, ptr_spread_flag,
                              is_ld_sbr);
    if (is_ld_sbr && tran_flag) {
      frame_type = IXHEAACE_LD_TRAN;
      *ptr_frame_type_old = IXHEAACE_FIXFIX;
    }
    if (tran_flag) {
      if (tran_pos < 4) {
        fmax = 6;
      } else if (tran_pos == 4 || tran_pos == 5) {
        fmax = 4;
      } else
        fmax = 8;

      tran = tran_pos + 4;

      ixheaace_fill_frame_tran(ptr_v_bord, ptr_length_v_bord, ptr_v_freq, ptr_length_v_freq,
                               &bmin, &bmax, tran, ptr_ptr_v_tuning_segm, ptr_tuning_freq);
    }
    if (0 == is_ld_sbr) {
      switch (frame_type) {
        case IXHEAACE_FIXVAR:

          ixheaace_fill_frame_pre(dmax, ptr_v_bord, ptr_length_v_bord, ptr_v_freq,
                                  ptr_length_v_freq, bmin, bmin);

          ixheaace_fill_Frame_Post(&ptr_parts, &ptr_d, dmax, ptr_v_bord, ptr_length_v_bord,
                                   ptr_v_freq, ptr_length_v_freq, bmax, fmax);

          if (ptr_parts == 1 && ptr_d < dmin) {
            ixheaace_special_case(ptr_spread_flag, allow_spread, ptr_v_bord, ptr_length_v_bord,
                                  ptr_v_freq, ptr_length_v_freq, &ptr_parts, ptr_d);
          }

          ixheaace_calc_cmon_border(&i_cmon, &i_tran, ptr_v_bord, ptr_length_v_bord, tran);
          ixheaace_keep_for_follow_up(ptr_v_bord_follow, ptr_length_v_bord_follow,
                                      ptr_v_freq_follow, ptr_length_v_freq_follow,
                                      ptr_tran_follow, ptr_fill_follow, ptr_v_bord,
                                      ptr_length_v_bord, ptr_v_freq, i_cmon, i_tran, ptr_parts);

          ixheaace_calc_ctrl_signal(&pstr_sbr_env_frame->sbr_grid, frame_type, ptr_v_bord,
                                    *ptr_length_v_bord, ptr_v_freq, *ptr_length_v_freq, i_cmon,
                                    i_tran, *ptr_spread_flag, IXHEAACE_DC);
          break;
        case IXHEAACE_VARFIX:
          ixheaace_calc_ctrl_signal(&pstr_sbr_env_frame->sbr_grid, frame_type, ptr_v_bord_follow,
                                    *ptr_length_v_bord_follow, ptr_v_freq_follow,
                                    *ptr_length_v_freq_follow, IXHEAACE_DC, *ptr_tran_follow,
                                    *ptr_spread_flag, IXHEAACE_DC);
          break;
        case IXHEAACE_VARVAR:

          if (*ptr_spread_flag) {
            ixheaace_calc_ctrl_signal(&pstr_sbr_env_frame->sbr_grid, frame_type,
                                      ptr_v_bord_follow, *ptr_length_v_bord_follow,
                                      ptr_v_freq_follow, *ptr_length_v_freq_follow, IXHEAACE_DC,
                                      *ptr_tran_follow, *ptr_spread_flag, IXHEAACE_DC);

            *ptr_spread_flag = 0;

            ptr_v_bord_follow[0] = pstr_sbr_env_frame->sbr_grid.bs_abs_bord_1 - 16;

            ptr_v_freq_follow[0] = FREQ_RES_HIGH;
            *ptr_length_v_bord_follow = 1;
            *ptr_length_v_freq_follow = 1;

            *ptr_tran_follow = -IXHEAACE_DC;
            *ptr_fill_follow = -IXHEAACE_DC;
          } else {
            ixheaace_fill_frame_inter(
                &ptr_n_l, ptr_ptr_v_tuning_segm, ptr_v_bord, ptr_length_v_bord, bmin, ptr_v_freq,
                ptr_length_v_freq, ptr_v_bord_follow, ptr_length_v_bord_follow, ptr_v_freq_follow,
                ptr_length_v_freq_follow, *ptr_fill_follow, dmin, dmax);

            ixheaace_fill_Frame_Post(&ptr_parts, &ptr_d, dmax, ptr_v_bord, ptr_length_v_bord,
                                     ptr_v_freq, ptr_length_v_freq, bmax, fmax);

            if (ptr_parts == 1 && ptr_d < dmin) {
              ixheaace_special_case(ptr_spread_flag, allow_spread, ptr_v_bord, ptr_length_v_bord,
                                    ptr_v_freq, ptr_length_v_freq, &ptr_parts, ptr_d);
            }

            ixheaace_calc_cmon_border(&i_cmon, &i_tran, ptr_v_bord, ptr_length_v_bord, tran);

            ixheaace_keep_for_follow_up(ptr_v_bord_follow, ptr_length_v_bord_follow,
                                        ptr_v_freq_follow, ptr_length_v_freq_follow,
                                        ptr_tran_follow, ptr_fill_follow, ptr_v_bord,
                                        ptr_length_v_bord, ptr_v_freq, i_cmon, i_tran, ptr_parts);

            ixheaace_calc_ctrl_signal(&pstr_sbr_env_frame->sbr_grid, frame_type, ptr_v_bord,
                                      *ptr_length_v_bord, ptr_v_freq, *ptr_length_v_freq, i_cmon,
                                      i_tran, 0, ptr_n_l);
          }
          break;
        case IXHEAACE_FIXFIX:

          num_env = (tran_pos == 0 ? 1 : 2);

          pstr_sbr_env_frame->sbr_grid.bs_num_env = num_env;
          pstr_sbr_env_frame->sbr_grid.frame_type = frame_type;

          break;
        default:
          break;
      }
      err_code = ixheaace_ctrl_signal2_frame_info(
          &pstr_sbr_env_frame->sbr_grid, &pstr_sbr_env_frame->sbr_frame_info, freq_res_fix,
          ptr_qmf_tab, flag_framelength_small);
      if (err_code) {
        return err_code;
      }
    } else {
      WORD32 i;

      switch (frame_type) {
        case IXHEAACE_FIXFIX: {
          pstr_sbr_env_frame->sbr_grid.frame_type = frame_type;
          pstr_sbr_env_frame->sbr_grid.bs_transient_position = tran_pos;
          pstr_sbr_env_frame->sbr_frame_info.n_envelopes = 1;
          pstr_sbr_env_frame->sbr_grid.bs_num_env = 1;
          if (tran_pos == 1) {
            pstr_sbr_env_frame->sbr_grid.bs_num_env = 2;
          }
          pstr_sbr_env_frame->sbr_frame_info.short_env = 0;
          if (flag_framelength_small) {
            ixheaace_create_def_frame_480_info(&pstr_sbr_env_frame->sbr_frame_info,
                                               pstr_sbr_env_frame->sbr_grid.bs_num_env,
                                               ptr_qmf_tab);
          } else {
            ixheaace_create_def_frame_info(&pstr_sbr_env_frame->sbr_frame_info,
                                           pstr_sbr_env_frame->sbr_grid.bs_num_env, ptr_qmf_tab);
          }

          if (pstr_sbr_env_frame->sbr_frame_info.n_envelopes > 1) {
            for (i = 0; i < pstr_sbr_env_frame->sbr_frame_info.n_envelopes; i++) {
              pstr_sbr_env_frame->sbr_frame_info.freq_res[i] = FREQ_RES_LOW;
              pstr_sbr_env_frame->sbr_grid.v_f[i] = FREQ_RES_LOW;
            }
          } else {
            for (i = 0; i < pstr_sbr_env_frame->sbr_frame_info.n_envelopes; i++) {
              pstr_sbr_env_frame->sbr_frame_info.freq_res[i] = FREQ_RES_HIGH;
              pstr_sbr_env_frame->sbr_grid.v_f[i] = FREQ_RES_HIGH;
            }
          }
        } break;

        case IXHEAACE_LD_TRAN: {
          ixheaace_create_ld_transient_frame_info(&pstr_sbr_env_frame->sbr_frame_info,
                                                  &pstr_sbr_env_frame->sbr_grid, tran_pos,
                                                  num_time_slots,
                                                  pstr_sbr_env_frame->use_low_freq_res);
        } break;
        default:
          break;
      }
    }
  }

  *ptr_frame_info = &pstr_sbr_env_frame->sbr_frame_info;
  return err_code;
}

VOID ixheaace_create_frame_info_generator(ixheaace_pstr_sbr_env_frame pstr_sbr_env_frame,
                                          WORD32 allow_spread, WORD32 num_env_static,
                                          WORD32 static_framing, ixheaace_freq_res freq_res_fix,
                                          WORD32 use_low_freq_res) {
  memset(pstr_sbr_env_frame, 0, sizeof(ixheaace_str_sbr_env_frame));

  pstr_sbr_env_frame->frame_type_old = IXHEAACE_FIXFIX;
  pstr_sbr_env_frame->spread_flag = 0;

  pstr_sbr_env_frame->allow_spread = allow_spread;
  pstr_sbr_env_frame->num_env_static = num_env_static;
  pstr_sbr_env_frame->static_framing = static_framing;
  pstr_sbr_env_frame->freq_res_fix = freq_res_fix;
  pstr_sbr_env_frame->use_low_freq_res = use_low_freq_res;

  pstr_sbr_env_frame->length_v_bord = 0;
  pstr_sbr_env_frame->length_v_bord_follow = 0;

  pstr_sbr_env_frame->length_v_freq = 0;
  pstr_sbr_env_frame->length_v_freq_follow = 0;

  pstr_sbr_env_frame->i_tran_follow = 0;
  pstr_sbr_env_frame->i_fill_follow = 0;

  pstr_sbr_env_frame->dmin = 4;
  pstr_sbr_env_frame->dmax = 12;
}
