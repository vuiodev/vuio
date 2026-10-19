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
#ifndef IXHEAACD_WINDOWS_H
#define IXHEAACD_WINDOWS_H

extern const WORD32 ixheaacd_sine_win_128[128];
extern const WORD32 ixheaacd_sine_win_1024[1024];
extern const WORD32 ixheaacd_sine_win_64[64];
extern const WORD32 ixheaacd_sine_win_768[768];
extern const WORD32 ixheaacd_sine_win_192[192];
extern const WORD32 ixheaacd_sine_win_96[96];
extern const WORD32 ixheaacd_sine_win_256[256];
extern const WORD32 ixheaacd_kbd_win120[120];
extern const WORD32 ixheaacd_kbd_win128[128];
extern const WORD32 ixheaacd_kbd_win960[960];
extern const WORD32 ixheaacd_kbd_win1024[1024];
extern const WORD32 kbd_win256[256];
extern const WORD32 ixheaacd_kbd_win4[4];
extern const WORD32 ixheaacd_kbd_win16[16];
extern const WORD32 ixheaacd_kbd_win_64[64];
extern const WORD32 ixheaacd_kbd_win768[768];
extern const WORD32 ixheaacd_kbd_win192[192];
extern const WORD32 ixheaacd_kbd_win96[96];
extern const WORD32 ixheaacd_kbd_win48[48];

extern const FLOAT32 ixheaacd_sine_window96[96];
extern const FLOAT32 ixheaacd_sine_window128[128];
extern const FLOAT32 ixheaacd_sine_window192[192];
extern const FLOAT32 ixheaacd__sine_window256[256];

WORD32 ixheaacd_calc_window(WORD32 **win, WORD32 len, WORD32 wfun_select, WORD32 ec_flag);

VOID ixheaacd_acelp_imdct(WORD32 *imdct_in, WORD32 npoints, WORD8 *qshift, WORD32 *scratch);

typedef struct {
  WORD32 lfac;
  WORD32 n_flat_ls;
  WORD32 n_trans_ls;
  WORD32 n_long;
  WORD32 n_short;

} offset_lengths;

#endif
