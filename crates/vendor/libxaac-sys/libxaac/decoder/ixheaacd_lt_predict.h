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
#ifndef IXHEAACD_LT_PREDICT_H
#define IXHEAACD_LT_PREDICT_H

#define MAX_SFB 51
#define MAX_LTP_SFB 40
#define MAX_LTP_SFB_SR_FIVE_PLUS_480 31
#define MAX_LTP_SFB_SR_FIVE_480 38
#define MAX_LTP_SFB_SR_FIVE_LESS_480 36
#define MAX_LTP_SFB_SR_FIVE_PLUS_512 32
#define MAX_LTP_SFB_SR_FIVE_512 38
#define MAX_LTP_SFB_SR_FIVE_LESS_512 37

enum { ltp_buffer_size = (4 * 1024) };

typedef struct {
  UWORD8 last_band;
  UWORD8 data_present;
  UWORD16 lag;
  UWORD8 lag_update;
  UWORD8 coef;
  UWORD8 long_used[MAX_SFB];
  UWORD8 short_used[8];
  UWORD8 short_lag_present[8];
  UWORD8 short_lag[8];
} ltp_info;

VOID ixheaacd_init_ltp_object(ltp_info *ltp);

VOID ixheaacd_lt_update_state(WORD16 *lt_pred_stat, VOID *time, WORD32 *overlap,
                              WORD32 frame_len, WORD32 object_type,
                              WORD32 stride, WORD16 window_sequence,
                              WORD16 *p_window_next, WORD slot_element);

VOID ixheaacd_filter_bank_ltp(ia_aac_dec_tables_struct *aac_tables_ptr,
                              WORD16 window_sequence, WORD16 win_shape,
                              WORD16 win_shape_prev, WORD32 *in_data,
                              WORD32 *out_mdct, UWORD32 object_type,
                              UWORD32 frame_len);

#endif /* IXHEAACD_LT_PREDICT_H */
