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
#include "ixheaacd_cnst.h"

#include "ixheaac_constants.h"
#include "ixheaac_type_def.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops40.h"

#include "ixheaacd_windows.h"

WORD32 ixheaacd_calc_window(WORD32 **win, WORD32 win_sz, WORD32 win_sel, WORD32 ec_flag) {
  switch (win_sel) {
    case WIN_SEL_0:
      switch (win_sz) {
        case WIN_LEN_128:
          *win = (WORD32 *)ixheaacd_sine_win_128;
          break;
        case WIN_LEN_1024:
          *win = (WORD32 *)ixheaacd_sine_win_1024;
          break;
        case WIN_LEN_64:
          *win = (WORD32 *)ixheaacd_sine_win_64;
          break;
        case WIN_LEN_768:
          *win = (WORD32 *)ixheaacd_sine_win_768;
          break;
        case WIN_LEN_192:
          *win = (WORD32 *)ixheaacd_sine_win_192;
          break;
        case WIN_LEN_96:
          *win = (WORD32 *)ixheaacd_sine_win_96;
          break;
        case WIN_LEN_256:
          *win = (WORD32 *)ixheaacd_sine_win_256;
          break;
        default:
          if (ec_flag)
            *win = (WORD32 *)ixheaacd_sine_win_1024;
          else
            return -1;
          break;
      }
      break;

    case WIN_SEL_1:
      switch (win_sz) {
        case WIN_LEN_120:
          *win = (WORD32 *)ixheaacd_kbd_win120;
          break;
        case WIN_LEN_128:
          *win = (WORD32 *)ixheaacd_kbd_win128;
          break;
        case WIN_LEN_960:
          *win = (WORD32 *)ixheaacd_kbd_win960;
          break;
        case WIN_LEN_1024:
          *win = (WORD32 *)ixheaacd_kbd_win1024;
          break;
        case WIN_LEN_4:
          *win = (WORD32 *)ixheaacd_kbd_win4;
          break;
        case WIN_LEN_16:
          *win = (WORD32 *)ixheaacd_kbd_win16;
          break;
        case WIN_LEN_64:
          *win = (WORD32 *)ixheaacd_kbd_win_64;
          break;
        case WIN_LEN_768:
          *win = (WORD32 *)ixheaacd_kbd_win768;
          break;
        case WIN_LEN_192:
          *win = (WORD32 *)ixheaacd_kbd_win192;
          break;
        case WIN_LEN_96:
          *win = (WORD32 *)ixheaacd_kbd_win96;
          break;
        case WIN_LEN_48:
          *win = (WORD32 *)ixheaacd_kbd_win48;
          break;
        default:
          if (ec_flag)
            *win = (WORD32 *)ixheaacd_kbd_win1024;
          else
            return -1;
          break;
      }
      break;

    default:
      return -1;
      break;
  }
  return 0;
}
