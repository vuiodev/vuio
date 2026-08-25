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

#pragma once
IA_ERRORCODE iusace_calc_window(FLOAT64 **win, WORD32 win_sz, WORD32 win_sel);
VOID iusace_windowing_long(FLOAT64 *ptr_out_buf, FLOAT64 *ptr_win_long, FLOAT64 *ptr_win_buf,
                           FLOAT64 *ptr_in_data, WORD32 n_long);
VOID iusace_windowing_long_start(FLOAT64 *ptr_out_buf, FLOAT64 *ptr_win_long,
                                 FLOAT64 *ptr_win_buf, FLOAT64 *ptr_in_data, WORD32 n_long,
                                 WORD32 nflat_ls, FLOAT64 *ptr_win_med, WORD32 win_sz);
VOID iusace_windowing_long_stop(FLOAT64 *ptr_out_buf, FLOAT64 *ptr_win_long, FLOAT64 *ptr_win_buf,
                                FLOAT64 *ptr_in_data, WORD32 n_long, WORD32 nflat_ls,
                                FLOAT64 *ptr_win_med, WORD32 win_sz);
VOID iusace_windowing_stop_start(FLOAT64 *ptr_out_buf, FLOAT64 *ptr_win_buf, FLOAT64 *ptr_win_med,
                                 WORD32 win_sz, WORD32 n_long);
WORD32 iusace_fd_mdct(ia_usac_data_struct *pstr_usac_data,
                      ia_usac_encoder_config_struct *pstr_usac_config, WORD32 ch_idx);
