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

#include <stdlib.h>
#include <stdio.h>
#include <string.h>

#include "ixheaac_type_def.h"
#include "ixheaac_constants.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops16.h"
#include "ixheaac_basic_ops40.h"

#include "ixheaacd_defines.h"
#include "ixheaacd_cnst.h"
#include "ixheaacd_aac_rom.h"
#include "ixheaacd_lt_predict.h"
#include "ixheaacd_ec_defines.h"
#include "ixheaacd_ec_struct_def.h"
#include "ixheaacd_audioobjtypes.h"

#include "ixheaacd_bitbuffer.h"
#include "ixheaacd_pulsedata.h"
#include "ixheaacd_pns.h"
#include "ixheaacd_channelinfo.h"
#include "ixheaacd_tns.h"
#include "ixheaacd_aac_imdct.h"

static const WORD32 ixheaacd_codebook_Q30[8] = {
    612922971,  747985734,  872956397,  978505219,
    1057528322, 1146642451, 1282693056, 1470524861};

#define SHIFT_VAL 8
#define SHIFT_VAL1 (15 - SHIFT_VAL)

VOID ixheaacd_lt_prediction(
    ia_aac_dec_channel_info_struct *ptr_aac_dec_channel_info, ltp_info *ltp,
    WORD32 *spec, ia_aac_dec_tables_struct *aac_tables_ptr,
    UWORD16 win_shape_prev, UWORD32 sr_index, UWORD32 object_type,
    UWORD32 frame_len, WORD32 *in_data, WORD32 *out_data) {
  ia_ics_info_struct *ptr_ics_info = &ptr_aac_dec_channel_info->str_ics_info;
  WORD16 *lt_pred_stat = ptr_aac_dec_channel_info->ltp_buf;
  UWORD16 win_shape = ptr_aac_dec_channel_info->str_ics_info.window_shape;
  WORD16 sfb;
  WORD16 bin, i, num_samples;
  const WORD8 *swb_offset = aac_tables_ptr->scale_factor_bands_long[sr_index];
  WORD32 *ptr_spec = &spec[0];
  WORD32 *ptr_x_est = &out_data[0];

  if (512 == ptr_ics_info->frame_length) {
    swb_offset = aac_tables_ptr->scale_fac_bands_512[sr_index];
  } else if (480 == ptr_ics_info->frame_length) {
    swb_offset = aac_tables_ptr->scale_fac_bands_480[sr_index];
  }

  if (ptr_ics_info->window_sequence != EIGHT_SHORT_SEQUENCE) {
    if (ltp->data_present) {
      num_samples = frame_len << 1;

      for (i = 0; i < num_samples; i++) {
        in_data[i] =
            ixheaac_shr32(ixheaac_mult32x16in32_shl_sat(
                               ixheaacd_codebook_Q30[ltp->coef],
                               lt_pred_stat[num_samples + i - ltp->lag]),
                           SHIFT_VAL);
      }

      ixheaacd_filter_bank_ltp(aac_tables_ptr, ptr_ics_info->window_sequence,
                               win_shape, win_shape_prev, in_data, out_data,
                               object_type, frame_len);

      if (ptr_aac_dec_channel_info->str_tns_info.tns_data_present == 1)
        ixheaacd_aac_tns_process(ptr_aac_dec_channel_info, 1, aac_tables_ptr,
                                 object_type, 0, out_data);

      for (sfb = 0; sfb < ltp->last_band; sfb++) {
        WORD8 sfb_width = swb_offset[sfb];
        if (ltp->long_used[sfb]) {
          for (bin = sfb_width - 1; bin >= 0; bin--) {
            WORD32 temp = *ptr_spec;
            temp = ixheaac_add32_sat(temp,
                                      ixheaac_shr32(*ptr_x_est++, SHIFT_VAL1));
            *ptr_spec++ = temp;
          }
        } else {
          ptr_spec += sfb_width;
          ptr_x_est += sfb_width;
        }
      }
    }
  }
}

VOID ixheaacd_filter_bank_ltp(ia_aac_dec_tables_struct *aac_tables_ptr,
                              WORD16 window_sequence, WORD16 window_shape,
                              WORD16 window_shape_prev, WORD32 *in_data,
                              WORD32 *out_mdct, UWORD32 object_type,
                              UWORD32 frame_len) {
  WORD32 i;

  const WORD16 *window_long = NULL;
  const WORD16 *window_long_prev = NULL;
  const WORD16 *window_short = NULL;
  const WORD16 *window_short_prev = NULL;

  UWORD16 nlong = frame_len;
  UWORD16 nlong2 = frame_len << 1;
  UWORD16 nshort = frame_len / 8;
  UWORD16 nflat_ls = (nlong - nshort) / 2;
  WORD32 imdct_scale = 0;
  WORD32 expo = 0;

  if (object_type == AOT_ER_AAC_LD) {
    if (!window_shape) {
      if (512 == frame_len) {
        window_long =
            (WORD16 *)aac_tables_ptr->pstr_imdct_tables->window_sine_512;
      } else {
        window_long =
            (WORD16 *)aac_tables_ptr->pstr_imdct_tables->window_sine_480;
      }
    } else {
      if (512 == frame_len) {
        window_long =
            (WORD16 *)aac_tables_ptr->pstr_imdct_tables->low_overlap_win;
      } else {
        window_long =
            (WORD16 *)aac_tables_ptr->pstr_imdct_tables->low_overlap_win_480;
      }
    }

    if (!window_shape_prev) {
      if (512 == frame_len) {
        window_long_prev =
            (WORD16 *)aac_tables_ptr->pstr_imdct_tables->window_sine_512;
      } else {
        window_long_prev =
            (WORD16 *)aac_tables_ptr->pstr_imdct_tables->window_sine_480;
      }
    } else {
      if (512 == frame_len) {
        window_long_prev =
            (WORD16 *)aac_tables_ptr->pstr_imdct_tables->low_overlap_win;
      } else {
        window_long_prev =
            (WORD16 *)aac_tables_ptr->pstr_imdct_tables->low_overlap_win_480;
      }
    }

    if (!window_shape)
      window_short = aac_tables_ptr->pstr_imdct_tables->only_short_window_sine;
    else
      window_short = aac_tables_ptr->pstr_imdct_tables->only_short_window_kbd;
    if (!window_shape_prev)
      window_short_prev =
          aac_tables_ptr->pstr_imdct_tables->only_short_window_sine;
    else
      window_short_prev =
          aac_tables_ptr->pstr_imdct_tables->only_short_window_kbd;

  } else {
    if (!window_shape)
      window_long = aac_tables_ptr->pstr_imdct_tables->only_long_window_sine;
    else
      window_long = aac_tables_ptr->pstr_imdct_tables->only_long_window_kbd;
    if (!window_shape_prev)
      window_long_prev =
          aac_tables_ptr->pstr_imdct_tables->only_long_window_sine;
    else
      window_long_prev =
          aac_tables_ptr->pstr_imdct_tables->only_long_window_kbd;

    if (!window_shape)
      window_short = aac_tables_ptr->pstr_imdct_tables->only_short_window_sine;
    else
      window_short = aac_tables_ptr->pstr_imdct_tables->only_short_window_kbd;
    if (!window_shape_prev)
      window_short_prev =
          aac_tables_ptr->pstr_imdct_tables->only_short_window_sine;
    else
      window_short_prev =
          aac_tables_ptr->pstr_imdct_tables->only_short_window_kbd;
  }

  switch (window_sequence) {
    case ONLY_LONG_SEQUENCE:

      if ((512 != nlong) && (480 != nlong)) {
        for (i = 0; i<nlong>> 1; i++) {
          in_data[i] =
              ixheaac_mult32x16in32_shl(in_data[i], window_long_prev[2 * i]);

          in_data[i + nlong] = ixheaac_mult32x16in32_shl(
              in_data[i + nlong], window_long[2 * i + 1]);
        }
        for (i = 0; i<nlong>> 1; i++) {
          in_data[i + (nlong >> 1)] = ixheaac_mult32x16in32_shl(
              in_data[i + (nlong >> 1)], window_long_prev[nlong - 1 - 2 * i]);

          in_data[i + nlong + (nlong >> 1)] =
              ixheaac_mult32x16in32_shl(in_data[i + nlong + (nlong >> 1)],
                                         window_long[nlong - 1 - 2 * i - 1]);
        }

      } else {
        WORD32 *win1, *win2;
        WORD32 *ptr_in1, *ptr_in2;
        win1 = (WORD32 *)window_long_prev;
        win2 = (WORD32 *)window_long;
        ptr_in1 = &in_data[0];
        ptr_in2 = &in_data[nlong];

        for (i = nlong - 1; i >= 0; i--) {
          WORD32 temp1 = ixheaac_mult32_shl(*ptr_in1, *win1++);
          WORD32 temp2 = ixheaac_mult32_shl(*ptr_in2, win2[i]);

          *ptr_in1++ = temp1;
          *ptr_in2++ = temp2;
        }
      }

      for (i = 0; i < nlong / 2; i++) {
        out_mdct[nlong / 2 + i] =
            ixheaac_sub32(in_data[i], in_data[nlong - 1 - i]);
        out_mdct[i] = (-ixheaac_add32(in_data[nlong + i + nlong / 2],
                                       in_data[nlong2 - nlong / 2 - 1 - i]));
      }

      if (512 == nlong || (480 == nlong)) {
        if (512 == nlong)
          ixheaacd_inverse_transform_512(
              out_mdct, in_data, &imdct_scale,
              aac_tables_ptr->pstr_imdct_tables->cosine_array_1024,
              aac_tables_ptr->pstr_imdct_tables, object_type);

        else
          ixheaacd_mdct_480_ld(out_mdct, in_data, &imdct_scale, 1,
                               aac_tables_ptr->pstr_imdct_tables, object_type);

        imdct_scale += 1;

        if (imdct_scale > 0) {
          WORD32 *ptr_out_mdct = &out_mdct[0];

          for (i = nlong - 1; i >= 0; i -= 4) {
            *ptr_out_mdct = ixheaac_shl32(*ptr_out_mdct, imdct_scale);
            ptr_out_mdct++;
            *ptr_out_mdct = ixheaac_shl32(*ptr_out_mdct, imdct_scale);
            ptr_out_mdct++;
            *ptr_out_mdct = ixheaac_shl32(*ptr_out_mdct, imdct_scale);
            ptr_out_mdct++;
            *ptr_out_mdct = ixheaac_shl32(*ptr_out_mdct, imdct_scale);
            ptr_out_mdct++;
          }
        } else if (imdct_scale < 0) {
          WORD32 *ptr_out_mdct = &out_mdct[0];
          imdct_scale = -imdct_scale;
          for (i = nlong - 1; i >= 0; i -= 4) {
            *ptr_out_mdct = ixheaac_shr32(*ptr_out_mdct, imdct_scale);
            ptr_out_mdct++;
            *ptr_out_mdct = ixheaac_shr32(*ptr_out_mdct, imdct_scale);
            ptr_out_mdct++;
            *ptr_out_mdct = ixheaac_shr32(*ptr_out_mdct, imdct_scale);
            ptr_out_mdct++;
            *ptr_out_mdct = ixheaac_shr32(*ptr_out_mdct, imdct_scale);
            ptr_out_mdct++;
          }
        }
      }

      else if (1024 == nlong) {
        expo = ixheaacd_calc_max_spectral_line_dec(out_mdct, 1024) - 1;

        expo = 8 - expo;

        imdct_scale = ixheaacd_inverse_transform(
            out_mdct, in_data, aac_tables_ptr->pstr_imdct_tables, expo, 1024);

        ixheaacd_post_twiddle_dec(in_data, out_mdct,
                                  aac_tables_ptr->pstr_imdct_tables, 1024);

        imdct_scale += 1;

        for (i = 0; i < nlong; i++) {
          out_mdct[i] = ixheaac_shl32_dir(in_data[i], imdct_scale);
        }
      }

      break;

    case LONG_START_SEQUENCE:

      for (i = 0; i<nlong>> 1; i++)
        in_data[i] =
            ixheaac_mult32x16in32_shl(in_data[i], window_long_prev[2 * i]);

      for (i = 0; i<nlong>> 1; i++)
        in_data[i + (nlong >> 1)] = ixheaac_mult32x16in32_shl(
            in_data[i + (nlong >> 1)], window_long_prev[nlong - 1 - 2 * i - 1]);

      for (i = 0; i<nshort>> 1; i++)
        in_data[i + nlong + nflat_ls + (nshort >> 1)] =
            ixheaac_mult32x16in32_shl(
                in_data[i + nlong + nflat_ls + (nshort >> 1)],
                window_short[nshort - 1 - 2 * i - 1]);

      for (i = 0; i < nflat_ls; i++) in_data[i + nlong + nflat_ls + nshort] = 0;

      for (i = 0; i < nlong / 2; i++) {
        out_mdct[nlong / 2 + i] =
            ixheaac_sub32(in_data[i], in_data[nlong - 1 - i]);
        out_mdct[nlong / 2 - 1 - i] =
            -ixheaac_add32(in_data[nlong + i], in_data[nlong2 - 1 - i]);
      }

      {
        expo = ixheaacd_calc_max_spectral_line_dec(out_mdct, 1024) - 1;

        expo = 8 - expo;
        imdct_scale = ixheaacd_inverse_transform(
            out_mdct, in_data, aac_tables_ptr->pstr_imdct_tables, expo, 1024);

        ixheaacd_post_twiddle_dec(in_data, out_mdct,
                                  aac_tables_ptr->pstr_imdct_tables, 1024);
      }

      imdct_scale += 1;

      for (i = 0; i < nlong; i++) {
        out_mdct[i] = ixheaac_shl32_dir(in_data[i], imdct_scale);
      }
      break;

    case LONG_STOP_SEQUENCE:
      for (i = 0; i < nflat_ls; i++) in_data[i] = 0;

      for (i = 0; i<nshort>> 1; i++)
        in_data[i + nflat_ls] = ixheaac_mult32x16in32_shl(
            in_data[i + nflat_ls], window_short_prev[2 * i]);

      for (i = 0; i<nshort>> 1; i++)
        in_data[i + nflat_ls + (nshort >> 1)] =
            ixheaac_mult32x16in32_shl(in_data[i + nflat_ls + (nshort >> 1)],
                                       window_short_prev[127 - 2 * i]);

      for (i = 0; i<nlong>> 1; i++)
        in_data[i + nlong] = ixheaac_mult32x16in32_shl(in_data[i + nlong],
                                                        window_long[2 * i + 1]);

      for (i = 0; i<nlong>> 1; i++)
        in_data[i + nlong + (nlong >> 1)] =
            ixheaac_mult32x16in32_shl(in_data[i + nlong + (nlong >> 1)],
                                       window_long[nlong - 1 - 2 * i - 1]);

      for (i = 0; i < nlong / 2; i++) {
        out_mdct[nlong / 2 + i] =
            ixheaac_sub32(in_data[i], in_data[nlong - 1 - i]);
        out_mdct[nlong / 2 - 1 - i] =
            -ixheaac_add32(in_data[nlong + i], in_data[nlong2 - 1 - i]);
      }

      {
        expo = ixheaacd_calc_max_spectral_line_dec(out_mdct, 1024) - 1;

        expo = 8 - expo;
        imdct_scale = ixheaacd_inverse_transform(
            out_mdct, in_data, aac_tables_ptr->pstr_imdct_tables, expo, 1024);

        ixheaacd_post_twiddle_dec(in_data, out_mdct,
                                  aac_tables_ptr->pstr_imdct_tables, 1024);
      }

      imdct_scale += 1;

      for (i = 0; i < nlong; i++) {
        out_mdct[i] = ixheaac_shl32_dir(in_data[i], imdct_scale);
      }

      break;
  }
}

VOID ixheaacd_lt_update_state(WORD16 *lt_pred_stat, VOID *time_t,
                              WORD32 *overlap, WORD32 frame_len,
                              WORD32 object_type, WORD32 stride,
                              WORD16 window_sequence, WORD16 *p_window_next,
                              WORD slot_element) {
  WORD32 i;

  if (object_type == AOT_ER_AAC_LD) {
    WORD16 *ptr_ltp_state0 = &lt_pred_stat[0];
    WORD16 *ptr_ltp_state_fl = &lt_pred_stat[frame_len + 0];
    WORD16 *ptr_ltp_state_2fl = &lt_pred_stat[(frame_len * 2) + 0];
    WORD16 *time = (WORD16 *)time_t - slot_element;
    WORD16 *ptr_time_in = &time[0 * stride];

    for (i = 0; i < frame_len; i++) {
      *ptr_ltp_state0++ = *ptr_ltp_state_fl;
      *ptr_ltp_state_fl++ = *ptr_ltp_state_2fl;
      *ptr_ltp_state_2fl++ = *ptr_time_in;
      ptr_time_in += stride;
    }

  } else {
    WORD16 *ptr_ltp_state0 = &lt_pred_stat[0];
    WORD16 *ptr_ltp_state_fl = &lt_pred_stat[frame_len + 0];
    WORD32 *time = (WORD32 *)time_t;
    WORD32 *ptr_time_in = &time[0 * stride];

    time = (WORD32 *)time_t;

    for (i = 0; i < frame_len; i++) {
      *ptr_ltp_state0++ = *ptr_ltp_state_fl;
      *ptr_ltp_state_fl++ =
          ixheaac_round16(ixheaac_shl32_sat(*ptr_time_in, 2));
      ptr_time_in += stride;
    }
  }

  if ((window_sequence == ONLY_LONG_SEQUENCE) ||
      (window_sequence == LONG_STOP_SEQUENCE)) {
    if (512 == frame_len) {
      WORD32 *window = (WORD32 *)p_window_next;

      for (i = 0; i < 256; i++) {
        lt_pred_stat[(frame_len * 3) + i] =
            ixheaac_round16(ixheaac_mult16x16in32_shl(
                (WORD16)ixheaac_shl16(
                    (WORD16)-ixheaac_sat16(overlap[255 - i]), 1),
                (WORD16)ixheaac_shr32(window[511 - i], 15)));

        lt_pred_stat[(frame_len * 3) + 256 + i] =
            ixheaac_round16(ixheaac_mult16x16in32_shl(
                (WORD16)ixheaac_shl16((WORD16)-ixheaac_sat16(overlap[i]), 1),
                (WORD16)ixheaac_shr32(window[255 - i], 15)));
      }
    } else if (480 == frame_len) {
      WORD32 *window = (WORD32 *)p_window_next;

      for (i = 0; i < 240; i++) {
        lt_pred_stat[(frame_len * 3) + i] =
            ixheaac_round16(ixheaac_mult16x16in32_shl(
                (WORD16)ixheaac_shl16(
                    (WORD16)-ixheaac_sat16(overlap[239 - i]), 1),
                (WORD16)ixheaac_shr32(window[479 - i], 15)));

        lt_pred_stat[(frame_len * 3) + 240 + i] =
            ixheaac_round16(ixheaac_mult16x16in32_shl(
                (WORD16)ixheaac_shl16((WORD16)-ixheaac_sat16(overlap[i]), 1),
                (WORD16)ixheaac_shr32(window[239 - i], 15)));
      }
    } else {
      for (i = 0; i < 512; i++) {
        lt_pred_stat[(frame_len * 2) + i] = ixheaac_round16(
            ixheaac_shl32_sat(ixheaac_mult16x16in32_shl(
                                   (WORD16)-ixheaac_sat16(overlap[511 - i]),
                                   p_window_next[2 * i + 1]),
                               1));

        lt_pred_stat[(frame_len * 2) + 512 + i] =
            ixheaac_round16(ixheaac_shl32_sat(
                ixheaac_mult16x16in32_shl((WORD16)-ixheaac_sat16(overlap[i]),
                                           p_window_next[1023 - 2 * i - 1]),
                1));
      }
    }

  } else if (window_sequence == LONG_START_SEQUENCE) {
    for (i = 0; i < 448; i++) {
      lt_pred_stat[(frame_len * 2) + i] =
          ixheaac_shl16((WORD16)-ixheaac_sat16(overlap[511 - i]), 1);
    }
    for (i = 0; i < 64; i++) {
      lt_pred_stat[(frame_len * 2) + 448 + i] =
          ixheaac_round16(ixheaac_shl32_sat(
              ixheaac_mult16x16in32_shl(
                  (WORD16)-ixheaac_sat16(overlap[511 - 448 - i]),
                  p_window_next[2 * i + 1]),
              1));
    }
    for (i = 0; i < 64; i++) {
      lt_pred_stat[(frame_len * 2) + 512 + i] =
          ixheaac_round16(ixheaac_shl32_sat(
              ixheaac_mult16x16in32_shl((WORD16)-ixheaac_sat16(overlap[i]),
                                         p_window_next[127 - 2 * i - 1]),
              1));
    }
    for (i = 576; i < 1024; i++) {
      lt_pred_stat[(frame_len * 2) + i] = 0;
    }
  } else {
    for (i = 0; i < 448; i++) {
      lt_pred_stat[(frame_len * 2) + i] =
          ixheaac_shl16(ixheaac_sat16(overlap[i]), 1);
    }
    for (i = 0; i < 64; i++) {
      lt_pred_stat[(frame_len * 2) + 448 + i] = ixheaac_round16(
          ixheaac_shl32_sat(ixheaac_mult16x16in32_shl(
                                 (WORD16)-ixheaac_sat16(overlap[511 - i]),
                                 p_window_next[2 * i + 1]),
                             1));
    }
    for (i = 0; i < 64; i++) {
      lt_pred_stat[(frame_len * 2) + 512 + i] = ixheaac_round16(
          ixheaac_shl32_sat(ixheaac_mult16x16in32_shl(
                                 (WORD16)-ixheaac_sat16(overlap[448 + i]),
                                 p_window_next[127 - 2 * i - 1]),
                             1));
    }
    for (i = 576; i < 1024; i++) {
      lt_pred_stat[(frame_len * 2) + i] = 0;
    }
  }
}
