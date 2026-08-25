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
#include "ixheaac_error_standards.h"
#include "ixheaac_type_def.h"
#include "iusace_cnst.h"
#include "iusace_block_switch_const.h"
#include "iusace_rom.h"
#include "ixheaace_error_codes.h"

IA_ERRORCODE iusace_calc_window(FLOAT64 **win, WORD32 win_sz, WORD32 win_sel) {
  switch (win_sel) {
    case WIN_SEL_0:
      switch (win_sz) {
        case WIN_LEN_96:
          *win = (FLOAT64 *)iexheaac_sine_win_96;
          break;
        case WIN_LEN_192:
          *win = (FLOAT64 *)iexheaac_sine_win_192;
          break;
        case WIN_LEN_128:
          *win = (FLOAT64 *)iusace_sine_win_128;
          break;
        case WIN_LEN_256:
          *win = (FLOAT64 *)iusace_sine_win_256;
          break;
        case WIN_LEN_768:
          *win = (FLOAT64 *)iexheaac_sine_win_768;
          break;
        case WIN_LEN_1024:
          *win = (FLOAT64 *)iusace_sine_win_1024;
          break;
        default:
          return IA_EXHEAACE_EXE_FATAL_USAC_INVALID_WINDOW_LENGTH;
          break;
      }
      break;
    case WIN_SEL_1:
      switch (win_sz) {
        case WIN_LEN_96:
          *win = (FLOAT64 *)iexheaac_kbd_win_96;
          break;
        case WIN_LEN_128:
          *win = (FLOAT64 *)iusace_sine_win_128;
          break;
        case WIN_LEN_192:
          *win = (FLOAT64 *)iexheaac_kbd_win_192;
          break;
        case WIN_LEN_256:
          *win = (FLOAT64 *)iusace_kbd_win256;
          break;
        case WIN_LEN_768:
          *win = (FLOAT64 *)iexheaac_kbd_win_768;
          break;
        case WIN_LEN_1024:
          *win = (FLOAT64 *)iusace_kbd_win1024;
          break;
        default:
          return IA_EXHEAACE_EXE_FATAL_USAC_INVALID_WINDOW_LENGTH;
          break;
      }
      break;

    default:
      return IA_EXHEAACE_EXE_FATAL_USAC_INVALID_WINDOW_SHAPE;
      break;
  }
  return IA_NO_ERROR;
}

VOID iusace_windowing_long(FLOAT64 *ptr_overlap, FLOAT64 *ptr_win_long, FLOAT64 *ptr_win_buf,
                           FLOAT64 *ptr_in_data, WORD32 n_long) {
  WORD32 i;
  FLOAT64 *ptr_win = ptr_win_long + n_long - 1;
  WORD32 data_size = (OVERLAP_WIN_SIZE_576 * n_long) / LEN_SUPERFRAME;

  for (i = 0; i < n_long; i++) {
    ptr_win_buf[i] = ptr_overlap[i] * ptr_win_long[i];
  }

  memcpy(ptr_overlap, ptr_overlap + n_long, data_size * sizeof(ptr_overlap[0]));
  memcpy(ptr_overlap + data_size, ptr_in_data, n_long * sizeof(ptr_overlap[0]));

  for (i = 0; i < n_long; i++) {
    ptr_win_buf[i + n_long] = ptr_overlap[i] * (*ptr_win--);
  }

  return;
}

VOID iusace_windowing_long_start(FLOAT64 *ptr_overlap, FLOAT64 *ptr_win_long,
                                 FLOAT64 *ptr_win_buf, FLOAT64 *ptr_in_data, WORD32 n_long,
                                 WORD32 nflat_ls, FLOAT64 *ptr_win_med, WORD32 win_sz) {
  WORD32 i;
  FLOAT64 *ptr_win = ptr_win_buf + 2 * n_long - 1;
  WORD32 data_size = (OVERLAP_WIN_SIZE_576 * n_long) / LEN_SUPERFRAME;

  for (i = 0; i < n_long; i++) {
    ptr_win_buf[i] = ptr_overlap[i] * ptr_win_long[i];
  }

  memcpy(ptr_overlap, ptr_overlap + n_long, data_size * sizeof(ptr_overlap[0]));
  memcpy(ptr_overlap + data_size, ptr_in_data, n_long * sizeof(ptr_overlap[0]));
  memcpy(ptr_win_buf + n_long, ptr_overlap, nflat_ls * sizeof(ptr_win_buf[0]));

  ptr_win_med = ptr_win_med + win_sz - 1;
  win_sz = n_long - 2 * nflat_ls;

  for (i = 0; i < win_sz; i++) {
    ptr_win_buf[i + n_long + nflat_ls] = ptr_overlap[i + nflat_ls] * (*ptr_win_med--);
  }

  for (i = 0; i < nflat_ls; i++) {
    *ptr_win-- = 0;
  }

  return;
}

VOID iusace_windowing_long_stop(FLOAT64 *ptr_overlap, FLOAT64 *ptr_win_long, FLOAT64 *ptr_win_buf,
                                FLOAT64 *ptr_in_data, WORD32 n_long, WORD32 nflat_ls,
                                FLOAT64 *ptr_win_med, WORD32 win_sz) {
  WORD32 i;
  FLOAT64 *ptr_win = ptr_win_long + n_long - 1;
  WORD32 data_size = (OVERLAP_WIN_SIZE_576 * n_long) / LEN_SUPERFRAME;

  memset(ptr_win_buf, 0, nflat_ls * sizeof(FLOAT64));
  for (i = 0; i < win_sz; i++) {
    ptr_win_buf[i + nflat_ls] = ptr_overlap[i + nflat_ls] * ptr_win_med[i];
  }

  memcpy(ptr_win_buf + nflat_ls + win_sz, ptr_overlap + nflat_ls + win_sz,
         nflat_ls * sizeof(ptr_win_buf[0]));
  memcpy(ptr_overlap, ptr_overlap + n_long, data_size * sizeof(ptr_overlap[0]));
  memcpy(ptr_overlap + data_size, ptr_in_data, n_long * sizeof(ptr_overlap[0]));

  for (i = 0; i < n_long; i++) {
    ptr_win_buf[i + n_long] = ptr_overlap[i] * (*ptr_win--);
  }
  return;
}

VOID iusace_windowing_stop_start(FLOAT64 *ptr_overlap, FLOAT64 *ptr_win_buf, FLOAT64 *ptr_win_med,
                                 WORD32 win_sz, WORD32 n_long) {
  WORD32 i;
  FLOAT64 *win_gen;
  WORD32 wsize = (n_long - win_sz) >> 1;
  win_gen = ptr_win_med;

  for (i = 0; i < win_sz; i++) {
    ptr_win_buf[wsize + i] = ptr_overlap[wsize + i] * (*win_gen++);
  }
  memcpy(ptr_win_buf + wsize, ptr_overlap + wsize, wsize * sizeof(FLOAT64));
  memcpy(ptr_win_buf + n_long, ptr_overlap + n_long, wsize * sizeof(FLOAT64));

  win_gen = ptr_win_med + win_sz - 1;
  win_sz = n_long - 2 * wsize;

  for (i = 0; i < win_sz; i++) {
    ptr_win_buf[n_long + wsize + i] = ptr_overlap[n_long + wsize + i] * (*win_gen--);
  }
  return;
}
