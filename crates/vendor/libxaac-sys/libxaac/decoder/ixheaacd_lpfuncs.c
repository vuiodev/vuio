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
#include "ixheaacd_bitbuffer.h"

#include "ixheaac_basic_op.h"
#include "ixheaacd_intrinsics.h"

#include "ixheaacd_defines.h"

#include "ixheaacd_aac_rom.h"

#include "ixheaacd_definitions.h"

#include "ixheaacd_error_codes.h"

#include "ixheaacd_pulsedata.h"

#include "ixheaacd_pns.h"
#include "ixheaacd_drc_data_struct.h"

#include "ixheaacd_lt_predict.h"
#include "ixheaacd_cnst.h"
#include "ixheaacd_ec_defines.h"
#include "ixheaacd_ec_struct_def.h"
#include "ixheaacd_channelinfo.h"
#include "ixheaacd_drc_dec.h"

#include "ixheaacd_sbrdecoder.h"

#include "ixheaacd_block.h"
#include "ixheaacd_channel.h"

#include "ixheaacd_sbr_payload.h"
#include "ixheaacd_common_rom.h"
#include "ixheaacd_sbrdecsettings.h"
#include "ixheaacd_sbr_scale.h"
#include "ixheaacd_env_extr_part.h"
#include "ixheaacd_sbr_rom.h"

#include "ixheaacd_lpp_tran.h"
#include "ixheaacd_hybrid.h"
#include "ixheaacd_ps_dec.h"

#include "ixheaacd_env_extr.h"
#include "ixheaacd_adts.h"
#include "ixheaacd_audioobjtypes.h"
#include "ixheaacd_memory_standards.h"

#include "ixheaacd_latmdemux.h"

#include "ixheaacd_qmf_dec.h"
#include "ixheaacd_aacdec.h"
#include "ixheaacd_mps_polyphase.h"
#include "ixheaacd_config.h"
#include "ixheaacd_mps_macro_def.h"
#include "ixheaacd_mps_struct_def.h"
#include "ixheaacd_mps_res_rom.h"
#include "ixheaacd_mps_aac_struct.h"
#include "ixheaacd_mps_dec.h"
#include "ixheaacd_struct_def.h"

#include "ixheaacd_tns.h"
#include "ixheaacd_aac_imdct.h"

#include "ixheaacd_multichannel.h"
#include "ixheaacd_function_selector.h"

VOID ixheaacd_process_win_seq(WORD32 *coef, WORD32 *prev, WORD32 *out,
                              const WORD16 *window_long,
                              const WORD16 *window_short, WORD16 q_shift,
                              WORD16 ch_fac, WORD16 flag, WORD16 size_01) {
  WORD32 i, accu;
  WORD32 *coef_1;
  const WORD16 *temp_win_sh, *temp_win_long;
  WORD32 *out1, *out2;
  WORD32 *temp_prev;

  WORD16 size_07 = 7 * size_01;
  WORD16 size_08 = 8 * size_01;
  WORD16 size_09 = 9 * size_01;
  WORD16 size_14 = 14 * size_01;
  WORD16 size_15 = 15 * size_01;

  if (flag == 1) {
    for (i = 0; i < size_07; i++) {
      WORD32 temp1 = ixheaac_shl32_dir_sat_limit(
          ixheaac_mult32x16in32(coef[size_08 + i], window_long[2 * i]),
          (q_shift + 1));

      accu = ixheaac_add32_sat(temp1, ((WORD32)prev[i] << 16));
      out[ch_fac * i] = accu;

      accu = ixheaac_shl32_dir_sat_limit(
          ixheaac_mult32x16in32(-(coef[size_15 - 1 - i]),
                                 window_long[2 * (size_07 - i) - 1]),
          q_shift);
      out[ch_fac * (i + size_09)] = (accu << 1);
    }

    temp_win_sh = &(window_short[0]);
    coef_1 = &(coef[size_15]);
    temp_win_long = &(window_long[size_14]);
    temp_prev = &(prev[size_08 - 1]);
    out1 = &(out[ch_fac * (size_07)]);
    out2 = &(out[ch_fac * (size_09 - 1)]);

  } else {
    for (i = 0; i < size_07; i++) {
      accu = ixheaac_mult32x16in32_sat(
          prev[size_08 - 1 - i], ixheaac_negate16(window_long[2 * i + 1]));

      out[ch_fac * i] = accu;

      accu = ixheaac_sub32_sat(
          ixheaac_shl32_dir_sat_limit(-(coef[size_15 - 1 - i]), (q_shift - 1)),
          ixheaac_mult32x16in32_sat(prev[i + size_01],
                                     window_long[2 * size_07 - 2 - 2 * i]));

      out[ch_fac * (size_09 + i)] = accu;
    }

    temp_win_sh = &(window_long[size_14]);
    coef_1 = &(coef[size_15]);
    temp_win_long = &(window_short[0]);
    temp_prev = &(prev[size_01 - 1]);
    out1 = &(out[ch_fac * (size_07)]);
    out2 = &(out[ch_fac * (size_09 - 1)]);
  }

  for (i = size_01 - 1; i >= 0; i--) {
    WORD32 temp_coef = *coef_1++;
    WORD16 win1 = *temp_win_long++;
    WORD16 win2 = *temp_win_long++;
    WORD32 prev1 = *temp_prev--;
    WORD16 win4 = *temp_win_sh++;
    WORD16 win3 = *temp_win_sh++;
    accu = ixheaac_sub32_sat(
        ixheaac_shl32_dir_sat_limit(ixheaac_mult32x16in32(temp_coef, win1),
                                     q_shift),
        ixheaac_mult32x16in32_sat(prev1, win3));
    *out1 = accu << flag;
    out1 += ch_fac;

    accu = ixheaac_sub32_sat(
        ixheaac_shl32_dir_sat_limit(
            ixheaac_mult32x16in32(ixheaac_negate32_sat(temp_coef), win2),
            q_shift),
        ixheaac_mult32x16in32_sat(prev1, win4));
    *out2 = accu << flag;
    out2 -= ch_fac;
  }
}

static PLATFORM_INLINE VOID ixheaacd_long_short_win_process(
    WORD32 *current, WORD32 *prev, WORD32 *out, const WORD16 *short_window,
    const WORD16 *long_window_prev, WORD16 q_shift, WORD16 ch_fac,
    WORD32 flag, WORD16 size_01) {

  WORD16 size_02 = 2 * size_01;
  WORD16 size_03 = 3 * size_01;
  WORD i;
  WORD32 accu;
  WORD32 *current_tmp1 = &(current[(size_03 - 1)]);
  WORD32 *current_tmp2 = &(current[-size_01]);
  const WORD16 *short_ptr = &(short_window[size_02 - 1]);

  for (i = size_01 - 1; i >= 0; i--) {
    WORD32 tmp1_cur = *current_tmp1--;
    WORD32 tmp2_cur = *current_tmp2++;
    WORD16 short1 = *short_ptr--;
    WORD16 short2 = *short_ptr--;
    accu = ixheaac_sub32_sat(
        ixheaac_shl32_dir_sat_limit((ixheaac_mult32x16in32(tmp1_cur, short2) -
                                      ixheaac_mult32x16in32(tmp2_cur, short1)),
                                     q_shift),
        ixheaac_mult32x16in32_sat(prev[i], long_window_prev[0 - 2 - 2 * i]));
    out[ch_fac * (0 + i)] = accu;

    if (flag) {
      accu = ixheaac_sub32_sat(
          ixheaac_shl32_dir_sat_limit(
              (ixheaac_mult32x16in32(ixheaac_negate32_sat(tmp1_cur), short1) -
               ixheaac_mult32x16in32(tmp2_cur, short2)),
              q_shift),
          ixheaac_mult32x16in32_sat(prev[size_02 - 1 - i],
                                     long_window_prev[-2 * size_02 + 2 * i]));
      out[ch_fac * (size_02 - 1 - i)] = accu;
    }
  }
}

VOID ixheaacd_long_short_win_seq(WORD32 *current, WORD32 *prev, WORD32 *out,
                                 const WORD16 *short_window,
                                 const WORD16 *short_window_prev,
                                 const WORD16 *long_window_prev, WORD16 q_shift,
                                 WORD16 ch_fac, WORD16 size_01) {

  WORD16 size_02 = 2 * size_01;
  WORD16 size_06 = 6 * size_01;
  WORD16 size_07 = 7 * size_01;
  WORD16 size_08 = 8 * size_01;
  WORD16 size_09 = 9 * size_01;
  WORD16 size_10 = 10 * size_01;
  WORD16 size_16 = 16 * size_01;

  WORD32 i, flag;
  WORD32 accu;
  for (i = 0; i < size_07; i++) {
    accu = ixheaac_mult32x16in32_sat(
        prev[size_08 - 1 - i], ixheaac_negate16(long_window_prev[2 * i + 1]));
    out[ch_fac * i] = accu;
  }

  for (i = 0; i < size_01; i++) {
    accu = ixheaac_sub32_sat(
        ixheaac_shl32_dir_sat_limit(
            ixheaac_mult32x16in32(current[size_01 + i],
                                   short_window_prev[2 * i]),
            q_shift),
        ixheaac_mult32x16in32_sat(prev[size_01 - 1 - i],
                                   long_window_prev[2 * size_07 + 1 + 2 * i]));
    out[ch_fac * (size_07 + i)] = accu;
  }

  for (i = 0; i < size_01; i++) {
    accu = ixheaac_sub32_sat(
        ixheaac_shl32_dir_sat_limit(
            ixheaac_mult32x16in32(ixheaac_negate32_sat(current[size_02 - 1 - i]),
                                   short_window_prev[size_02 - 2 * i - 1]),
            q_shift),
        ixheaac_mult32x16in32_sat(prev[i],
                                   long_window_prev[size_16 - 2 - (2 * i)]));
    out[ch_fac * (size_08 + i)] = accu;
  }

  flag = 1;
  for (i = 0; i < 4; i++) {
    WORD32 inc = i * size_02;

    if (i == 3) {
      flag = 0;
    }

    ixheaacd_long_short_win_process(&current[size_01 + inc], &prev[size_01 + inc],
                                    &out[ch_fac * (size_09 + inc)], short_window,
                                    &long_window_prev[2 * (size_07 - inc)],
                                    q_shift, ch_fac, flag, size_01);
  }

  for (i = 0; i < size_01; i++) {
    accu = (ixheaac_mult32x16in32(-(current[size_10 - 1 - i]),
                                   short_window[size_02 - 2 * i - 1]) -
            ixheaac_mult32x16in32(current[size_06 + i],
                                   short_window[size_02 - 2 * i - 2]));
    prev[i] =
        ixheaac_round16(ixheaac_shl32_dir_sat_limit(accu, (q_shift + 1)));
  }
}

VOID ixheaacd_nolap1_32(WORD32 *coef, WORD32 *out, WORD16 q_shift,
                        WORD16 ch_fac, WORD16 size_01) {
  WORD16 size_07 = 7 * size_01;
  WORD32 i;

  for (i = 0; i < size_07; i++) {
    out[ch_fac * i] = ixheaac_shr32_sat(
        ixheaac_negate32_sat(coef[size_07 - 1 - i]), 16 - q_shift);
  }
}

VOID ixheaacd_neg_shift_spec_dec(WORD32 *coef, WORD32 *out, WORD16 q_shift,
                                 WORD16 ch_fac) {
  WORD32 i;
  for (i = 0; i < SIZE07; i++) {
    out[ch_fac * i] = (ixheaac_shl32_dir_sat_limit(
        ixheaac_negate32_sat(coef[SIZE07 - 1 - i]), q_shift));
  }
}

VOID ixheaacd_Nolap_dec(WORD32 *coef, WORD32 *out, WORD16 q_shift,
                        WORD16 ch_fac, WORD16 size_01) {
  WORD16 size_07 = 7 * size_01;
  WORD32 i;
  for (i = 0; i < size_07; i++) {
    out[ch_fac * i] = ixheaac_shl32_dir_sat_limit(
        ixheaac_negate32_sat(coef[size_07 - 1 - i]), q_shift);
  }
}

VOID ixheaacd_spec_to_overlapbuf_dec(WORD32 *ptr_overlap_buf,
                                     WORD32 *ptr_spec_coeff, WORD32 q_shift,
                                     WORD32 size) {
  WORD32 i;
  for (i = 0; i < size; i++) {
    ptr_overlap_buf[i] = ixheaac_shr32_sat(ptr_spec_coeff[i], 16 - q_shift);
  }
}

VOID ixheaacd_overlap_buf_out_dec(WORD32 *out_samples, WORD32 *ptr_overlap_buf,
                                  WORD32 size, const WORD16 ch_fac) {
  WORD32 i;

  for (i = 0; i < size; i++) {
    out_samples[ch_fac * i] =
        (ixheaac_shl32_sat((WORD16)ptr_overlap_buf[i], 15));
  }
}

VOID ixheaacd_overlap_out_copy_dec(WORD32 *out_samples, WORD32 *ptr_overlap_buf,
                                   WORD32 *ptr_overlap_buf1,
                                   const WORD16 ch_fac, WORD16 size_01) {
  WORD32 i;

  for (i = 0; i < size_01; i++) {
    out_samples[ch_fac * i] =
        ixheaac_shl32_sat((WORD16)ptr_overlap_buf[i], 15);
    ptr_overlap_buf[i] = ptr_overlap_buf1[i];
  }
}

VOID ixheaacd_imdct_process(ia_aac_dec_overlap_info *ptr_aac_dec_overlap_info,
                            WORD32 *ptr_spec_coeff,
                            ia_ics_info_struct *ptr_ics_info, VOID *out_samples,
                            const WORD16 ch_fac, WORD32 *scratch,
                            ia_aac_dec_tables_struct *ptr_aac_tables,
                            WORD32 object_type, WORD32 ld_mps_present,
                            WORD slot_element) {
  WORD32 *ptr_overlap_buf;
  const WORD16 *ptr_long_window;
  const WORD16 *ptr_short_window;
  WORD16 max_bin_long = ptr_ics_info->frame_length;
  WORD16 size_01;
  if (max_bin_long == 960)
    size_01 = (max_bin_long / 16);
  else
    size_01 = (MAX_BINS_LONG / 16);
  WORD16 size_02 = 2 * size_01;
  WORD16 size_04 = 4 * size_01;
  WORD16 size_06 = 6 * size_01;
  WORD16 size_07 = 7 * size_01;
  WORD16 size_08 = 8 * size_01;
  WORD16 size_09 = 9 * size_01;
  WORD16 size_10 = 10 * size_01;
  WORD16 size_14 = 14 * size_01;
  WORD16 size_15 = 15 * size_01;

  ptr_overlap_buf = ptr_aac_dec_overlap_info->ptr_overlap_buf;
  ptr_long_window =
      ptr_aac_dec_overlap_info
          ->ptr_long_window[(WORD32)ptr_aac_dec_overlap_info->window_shape];
  ptr_short_window =
      ptr_aac_dec_overlap_info
          ->ptr_short_window[(WORD32)ptr_aac_dec_overlap_info->window_shape];

  if (ptr_ics_info->window_sequence != EIGHT_SHORT_SEQUENCE) {
    WORD16 q_shift;
    WORD32 expo, imdct_scale;

    if ((512 == ptr_ics_info->frame_length) ||
        (480 == ptr_ics_info->frame_length)) {
      ptr_ics_info->qshift_adj = -2;

      if (512 == ptr_ics_info->frame_length) {
        WORD32 *ld_cos_sin_ptr =
            (WORD32 *)ptr_aac_tables->pstr_imdct_tables->cosine_array_1024;

        ixheaacd_inverse_transform_512(
            ptr_spec_coeff, scratch, &imdct_scale, ld_cos_sin_ptr,
            ptr_aac_tables->pstr_imdct_tables, object_type);

      } else {
        ixheaacd_mdct_480_ld(ptr_spec_coeff, scratch, &imdct_scale, 0,
                             ptr_aac_tables->pstr_imdct_tables, object_type);
      }

      if (object_type == AOT_ER_AAC_ELD) {
        int i, N = (ptr_ics_info->frame_length << 1);

        for (i = 0; i < N / 2; i++) {
          ptr_spec_coeff[i] = -ptr_spec_coeff[i + N];
          ptr_spec_coeff[i + N + N / 2] = -ptr_spec_coeff[i + N / 2];
        }
      }
    } else if (960 == ptr_ics_info->frame_length) {
      ixheaacd_mdct_960(ptr_spec_coeff, scratch, &imdct_scale, 0,
                        ptr_aac_tables->pstr_imdct_tables);
    } else {
      expo = (*ixheaacd_calc_max_spectral_line)(ptr_spec_coeff, 1024) - 1;

      expo = 8 - expo;

      imdct_scale = ixheaacd_inverse_transform(
          ptr_spec_coeff, scratch, ptr_aac_tables->pstr_imdct_tables, expo,
          1024);
    }

    q_shift = (31 + imdct_scale) + (-1 - 16 - 9);

    switch (ptr_ics_info->window_sequence) {
      case ONLY_LONG_SEQUENCE:

        switch (ptr_aac_dec_overlap_info->window_sequence) {
          case ONLY_LONG_SEQUENCE:
          case LONG_STOP_SEQUENCE:

            if (1024 == ptr_ics_info->frame_length) {
              ia_ics_info_struct *tmp_ptr_ics_info = ptr_ics_info;

              (*ixheaacd_post_twid_overlap_add)(
                  (WORD32 *)out_samples, ptr_spec_coeff,
                  ptr_aac_tables->pstr_imdct_tables, 1024, ptr_overlap_buf,
                  q_shift, ptr_long_window, ch_fac);

              ptr_ics_info->qshift_adj = 2;
              ptr_ics_info = tmp_ptr_ics_info;
            }

            if (960 == ptr_ics_info->frame_length)
            {
                ixheaacd_over_lap_add1_dec(ptr_spec_coeff, ptr_overlap_buf,
                    (WORD32*)out_samples, ptr_long_window, q_shift,
                    480, ch_fac);

                ixheaacd_spec_to_overlapbuf(ptr_overlap_buf, ptr_spec_coeff,
                                            q_shift, 480);

                ptr_ics_info->qshift_adj = 2;
            }

            if ((512 == ptr_ics_info->frame_length) ||
                (480 == ptr_ics_info->frame_length)) {
              if (object_type != AOT_ER_AAC_ELD) {
                if (512 == ptr_ics_info->frame_length) {
                  ixheaacd_lap1_512_480(ptr_spec_coeff, ptr_overlap_buf,
                                        (WORD16 *)out_samples, ptr_long_window,
                                        q_shift, size_04, ch_fac, slot_element);
                  ixheaacd_spec_to_overlapbuf(ptr_overlap_buf, ptr_spec_coeff,
                                              q_shift, size_04);
                } else if (480 == ptr_ics_info->frame_length) {
                  ixheaacd_lap1_512_480(ptr_spec_coeff, ptr_overlap_buf,
                                        (WORD16 *)out_samples, ptr_long_window,
                                        q_shift, 240, ch_fac, slot_element);
                  ixheaacd_spec_to_overlapbuf(ptr_overlap_buf, ptr_spec_coeff,
                                              q_shift, 240);
                }
              } else {
                if (ld_mps_present == 1) {
                  ixheaacd_eld_dec_windowing_32bit(
                      ptr_spec_coeff, ptr_long_window,
                      ptr_ics_info->frame_length, q_shift, ptr_overlap_buf,
                      ch_fac, (WORD32 *)out_samples);
                } else {
                  ixheaacd_eld_dec_windowing(
                      ptr_spec_coeff, ptr_long_window,
                      ptr_ics_info->frame_length, q_shift, ptr_overlap_buf,
                      ch_fac, (WORD16 *)out_samples, slot_element);
                }
                ptr_ics_info->qshift_adj = -2;
              }
            }
            break;

          case LONG_START_SEQUENCE:
          case EIGHT_SHORT_SEQUENCE:
            if (1024 == ptr_ics_info->frame_length) {
              (*ixheaacd_post_twiddle)(scratch, ptr_spec_coeff,
                                       ptr_aac_tables->pstr_imdct_tables, 1024);
              ixheaacd_process_win_seq(scratch, ptr_overlap_buf,
                                       (WORD32*)out_samples, ptr_long_window,
                                       ptr_short_window, q_shift, ch_fac, 1,
                                       size_01);
              (*ixheaacd_spec_to_overlapbuf)(ptr_overlap_buf, scratch, q_shift,
                                             size_08);
            }

            if (960 == ptr_ics_info->frame_length) {
              ixheaacd_process_win_seq(ptr_spec_coeff, ptr_overlap_buf,
                                       (WORD32*)out_samples, ptr_long_window,
                                       ptr_short_window, q_shift, ch_fac, 1,
                                       size_01);
              ixheaacd_spec_to_overlapbuf(ptr_overlap_buf, ptr_spec_coeff,
                                          q_shift, 480);
            }

            ptr_ics_info->qshift_adj = 1;

            if (512 == ptr_ics_info->frame_length) {
              ixheaacd_spec_to_overlapbuf(ptr_overlap_buf, ptr_spec_coeff,
                                          q_shift, size_04);
            }
            if (480 == ptr_ics_info->frame_length) {
              ixheaacd_spec_to_overlapbuf(ptr_overlap_buf, ptr_spec_coeff,
                                          q_shift, 240);
            }
            break;
        }

        break;

      case LONG_START_SEQUENCE:
        if (1024 == ptr_ics_info->frame_length) {
          (*ixheaacd_post_twiddle)(scratch, ptr_spec_coeff,
                                   ptr_aac_tables->pstr_imdct_tables, 1024);
        }
        switch (ptr_aac_dec_overlap_info->window_sequence) {
          case ONLY_LONG_SEQUENCE:
          case LONG_STOP_SEQUENCE:

            if (1024 == ptr_ics_info->frame_length) {
              (*ixheaacd_over_lap_add1)(scratch, ptr_overlap_buf,
                                        (WORD32*)out_samples, ptr_long_window,
                                        q_shift, size_08, ch_fac);
            } else {
              ixheaacd_over_lap_add1_dec(ptr_spec_coeff, ptr_overlap_buf,
                                         (WORD32*)out_samples, ptr_long_window,
                                         q_shift, size_08, ch_fac);
            }
            ptr_ics_info->qshift_adj = 2;
            break;

          case LONG_START_SEQUENCE:
          case EIGHT_SHORT_SEQUENCE:

            if (1024 == ptr_ics_info->frame_length) {
              ixheaacd_process_win_seq(scratch, ptr_overlap_buf,
                                       (WORD32*)out_samples, ptr_long_window,
                                       ptr_short_window, q_shift, ch_fac, 1,
                                       size_01);
            } else {
              ixheaacd_process_win_seq(ptr_spec_coeff, ptr_overlap_buf,
                                       (WORD32*)out_samples, ptr_long_window,
                                       ptr_short_window, q_shift, ch_fac, 1,
                                       size_01);
            }

            ptr_ics_info->qshift_adj = 1;

            break;
        }

        if (960 != ptr_ics_info->frame_length) {
          ixheaacd_nolap1_32(&scratch[size_01], ptr_overlap_buf, q_shift, 1,
                             size_01);

          (*ixheaacd_spec_to_overlapbuf)(&ptr_overlap_buf[size_07], scratch,
                                         q_shift, size_01);
        } else {
          ixheaacd_nolap1_32(&ptr_spec_coeff[size_01], ptr_overlap_buf,
                             q_shift, 1, size_01);

          (*ixheaacd_spec_to_overlapbuf)(&ptr_overlap_buf[size_07],
                                         ptr_spec_coeff, q_shift, size_01);
        }

        break;

      case LONG_STOP_SEQUENCE:
        if (1024 == ptr_ics_info->frame_length) {
          (*ixheaacd_post_twiddle)(scratch, ptr_spec_coeff,
                                   ptr_aac_tables->pstr_imdct_tables, 1024);
        }

        switch (ptr_aac_dec_overlap_info->window_sequence) {
          case EIGHT_SHORT_SEQUENCE:
          case LONG_START_SEQUENCE:

            if (960 != ptr_ics_info->frame_length) {
              (*ixheaacd_overlap_buf_out)((WORD32*)out_samples,
                                          ptr_overlap_buf, size_07, ch_fac);
              (*ixheaacd_over_lap_add1)(
                  &scratch[size_14], &ptr_overlap_buf[size_07],
                  ((WORD32*)out_samples + ch_fac * (size_07)),
                  ptr_short_window, q_shift, size_01, ch_fac);
            } else {
              ixheaacd_dec_copy_outsample((WORD32*)out_samples,
                                          ptr_overlap_buf, size_07, ch_fac);
              ixheaacd_over_lap_add1_dec(&ptr_spec_coeff[size_14],
                                         &ptr_overlap_buf[size_07],
                                         ((WORD32*)out_samples + ch_fac * (size_07)),
                                         ptr_short_window, q_shift, size_01,
                                         ch_fac);
            }

            {
              if (960 != ptr_ics_info->frame_length) {

                WORD16 q_shift1 = q_shift - 1;
                (*ixheaacd_neg_shift_spec)(&scratch[size_08],
                                           ((WORD32*)out_samples + ch_fac * size_09),
                                           q_shift1, ch_fac);
              } else {
                WORD16 q_shift1 = q_shift - 1;
                ixheaacd_Nolap_dec(&ptr_spec_coeff[size_08],
                                   ((WORD32*)out_samples + ch_fac * size_09),
                                   q_shift1, ch_fac, size_01);
              }
            }
            ptr_ics_info->qshift_adj = 2;

            break;
          case ONLY_LONG_SEQUENCE:
          case LONG_STOP_SEQUENCE:

            if (1024 == ptr_ics_info->frame_length) {
              ixheaacd_process_win_seq(scratch, ptr_overlap_buf,
                                       (WORD32*)out_samples, ptr_long_window,
                                       ptr_short_window, q_shift, ch_fac, 0,
                                       size_01);
            } else {
              ixheaacd_process_win_seq(ptr_spec_coeff, ptr_overlap_buf,
                                       (WORD32*)out_samples, ptr_long_window,
                                       ptr_short_window, q_shift, ch_fac, 0,
                                       size_01);
            }

            ptr_ics_info->qshift_adj = 2;
            break;
        }

        if (1024 == ptr_ics_info->frame_length) {
          (*ixheaacd_spec_to_overlapbuf)(ptr_overlap_buf, scratch, q_shift,
                                         size_08);
        } else {
          (*ixheaacd_spec_to_overlapbuf)(ptr_overlap_buf, ptr_spec_coeff,
                                         q_shift, size_08);
        }

        break;
    }

  } else {
    WORD16 q_shift, max_scale;
    WORD32 imdct_scale[8], i;
    const WORD16 *short_window;

    short_window = ptr_aac_dec_overlap_info
                       ->ptr_short_window[(WORD32)ptr_ics_info->window_shape];

    {
      WORD32 expo;

      if (1024 == ptr_ics_info->frame_length) {
        expo = (*ixheaacd_calc_max_spectral_line)(ptr_spec_coeff, 1024) - 1;

        expo = 5 - expo;

        for (i = 0; i < MAX_WINDOWS; i++) {
          imdct_scale[i] = ixheaacd_inverse_transform(
              &ptr_spec_coeff[i * size_02], &scratch[i * size_02],
              ptr_aac_tables->pstr_imdct_tables, expo, 128);

        (*ixheaacd_post_twiddle)(&scratch[i * size_02],
                                 &ptr_spec_coeff[i * size_02],
                                 ptr_aac_tables->pstr_imdct_tables, 128);
        }
        max_scale = 31 + imdct_scale[0];
        q_shift = max_scale + (-16 - 6 - 1);
      } else {
        expo = (*ixheaacd_calc_max_spectral_line)(ptr_spec_coeff, 960) - 1;
        memcpy(scratch, ptr_spec_coeff, sizeof(WORD32) * 960);

        for (i = 0; i < MAX_WINDOWS; i++) {
            ixheaacd_inverse_transform_960(
                &ptr_spec_coeff[i * size_02], &scratch[i * size_02],
                ptr_aac_tables->pstr_imdct_tables, expo, &imdct_scale[i]);

                imdct_scale[i] -= expo;
        }
        max_scale = 31 + imdct_scale[0];
        q_shift = max_scale + (-16 - 6 - 1);

      }
    }
    switch (ptr_aac_dec_overlap_info->window_sequence) {
      WORD32 overlap_buf_loc[64];

      case EIGHT_SHORT_SEQUENCE:
      case LONG_START_SEQUENCE:

        (*ixheaacd_overlap_buf_out)((WORD32 *)out_samples, ptr_overlap_buf,
                                    size_07, ch_fac);

        if (1024 == ptr_ics_info->frame_length) {
          (*ixheaacd_over_lap_add1)(&scratch[0], &ptr_overlap_buf[size_07],
                                    ((WORD32*)out_samples + ch_fac * size_07),
                                    ptr_short_window, q_shift, size_01, ch_fac);
        } else {
          ixheaacd_over_lap_add1_dec(&ptr_spec_coeff[0], &ptr_overlap_buf[size_07],
                                     ((WORD32*)out_samples + ch_fac * size_07),
                                     ptr_short_window, q_shift, size_01, ch_fac);
        }

        for (i = 0; i < 3; i++) {
          WORD32 inc = (i * size_02);

          if (1024 == ptr_ics_info->frame_length) {
            (*ixheaacd_spec_to_overlapbuf)(overlap_buf_loc, &scratch[inc],
                                           q_shift, size_01);

            (*ixheaacd_over_lap_add1)(&scratch[size_02 + inc], overlap_buf_loc,
                                      ((WORD32*)out_samples + ch_fac * (size_09 + inc)),
                                      short_window, q_shift, size_01, ch_fac);
          } else {
            (*ixheaacd_spec_to_overlapbuf)(overlap_buf_loc, &ptr_spec_coeff[inc],
                                           q_shift, size_01);

            ixheaacd_over_lap_add1_dec(&ptr_spec_coeff[size_02 + inc], overlap_buf_loc,
                                       ((WORD32*)out_samples + ch_fac * (size_09 + inc)),
                                       short_window, q_shift, size_01, ch_fac);
          }
        }

        if (1024 == ptr_ics_info->frame_length) {
          (*ixheaacd_over_lap_add2)(&scratch[size_08], &scratch[size_06],
                                    ptr_overlap_buf, short_window, q_shift,
                                    size_01, 1);
        } else {
          ixheaacd_over_lap_add2_dec(&ptr_spec_coeff[size_08], &ptr_spec_coeff[size_06],
                                     ptr_overlap_buf, short_window, q_shift,
                                     size_01, 1);
        }


        (*ixheaacd_overlap_out_copy)(((WORD32 *)out_samples + ch_fac * size_15),
                                     ptr_overlap_buf, &ptr_overlap_buf[size_01],
                                     ch_fac, size_01);

        ptr_ics_info->qshift_adj = 2;

        break;

      case ONLY_LONG_SEQUENCE:
      case LONG_STOP_SEQUENCE:

        if (1024 == ptr_ics_info->frame_length) {

          ixheaacd_long_short_win_seq(
              scratch, ptr_overlap_buf, (WORD32*)out_samples, short_window,
              ptr_short_window, ptr_long_window, q_shift, ch_fac, size_01);
        } else {
          ixheaacd_long_short_win_seq(
              ptr_spec_coeff, ptr_overlap_buf, (WORD32*)out_samples, short_window,
              ptr_short_window, ptr_long_window, q_shift, ch_fac, size_01);
        }

        ptr_ics_info->qshift_adj = 2;
        break;
    }

    for (i = 0; i < 3; i++) {
      WORD32 inc = (i * size_02);

      if (1024 == ptr_ics_info->frame_length) {
        (*ixheaacd_over_lap_add2)(&scratch[size_10 + inc], &scratch[size_08 + inc],
                                  &ptr_overlap_buf[size_01 + inc], short_window,
                                  q_shift, size_01, 1);
      } else {
        ixheaacd_over_lap_add2_dec(&ptr_spec_coeff[size_10 + inc],
                                   &ptr_spec_coeff[size_08 + inc],
                                   &ptr_overlap_buf[size_01 + inc],
                                   short_window, q_shift, size_01, 1);
      }
    }

    if (1024 == ptr_ics_info->frame_length) {
      (*ixheaacd_spec_to_overlapbuf)(&ptr_overlap_buf[size_07], &scratch[size_14],
                                     q_shift, size_01);
    } else {
      (*ixheaacd_spec_to_overlapbuf)(&ptr_overlap_buf[size_07], &ptr_spec_coeff[size_14],
                                     q_shift, size_01);
    }
  }

  ptr_aac_dec_overlap_info->window_shape = ptr_ics_info->window_shape;
  ptr_aac_dec_overlap_info->window_sequence = ptr_ics_info->window_sequence;
}

void ixheaacd_eld_dec_windowing(WORD32 *ptr_spect_coeff, const WORD16 *p_win,
                                WORD32 framesize, WORD16 q_shift,
                                WORD32 *p_overlap_buffer, const WORD16 stride,
                                VOID *out_samples_t, WORD slot_element) {
  int i = 0;
  int loop_size;
  WORD32 *ptr_z = ptr_spect_coeff;

  WORD32 *ptr_out, *p_out2;
  WORD32 *p_overlap_buffer32 = (WORD32 *)p_overlap_buffer;
  WORD32 delay = framesize >> 2;

  WORD16 *out_samples = (WORD16 *)out_samples_t - slot_element;

  ptr_z = ptr_spect_coeff + delay;
  p_win += delay;
  ptr_out = p_overlap_buffer32;

  q_shift = q_shift + 2;

  if (q_shift >= 0) {
    for (i = (delay)-1; i >= 0; i--) {
      WORD32 win_op;
      WORD32 win_ovadd_op;
      WORD16 win_val;

      win_val = *p_win++;

      win_op = ixheaac_mult32x16in32(*ptr_z++, (win_val));

      win_ovadd_op =
          ixheaac_add32_sat(ixheaac_shl32(win_op, q_shift), *ptr_out++);

      *out_samples = ixheaac_round16(ixheaac_shl32_sat(win_ovadd_op, 1));
      out_samples += stride;

      win_val = *p_win++;

      win_op = ixheaac_mult32x16in32(*ptr_z++, (win_val));

      win_ovadd_op =
          ixheaac_add32_sat(ixheaac_shl32(win_op, q_shift), *ptr_out++);

      *out_samples = ixheaac_round16(ixheaac_shl32_sat(win_ovadd_op, 1));
      out_samples += stride;
      win_val = *p_win++;

      win_op = ixheaac_mult32x16in32(*ptr_z++, (win_val));

      win_ovadd_op =
          ixheaac_add32_sat(ixheaac_shl32(win_op, q_shift), *ptr_out++);

      *out_samples = ixheaac_round16(ixheaac_shl32_sat(win_ovadd_op, 1));
      out_samples += stride;

      win_val = *p_win++;

      win_op = ixheaac_mult32x16in32(*ptr_z++, (win_val));

      win_ovadd_op =
          ixheaac_add32_sat(ixheaac_shl32(win_op, q_shift), *ptr_out++);

      *out_samples = ixheaac_round16(ixheaac_shl32_sat(win_ovadd_op, 1));
      out_samples += stride;
    }

    p_out2 = p_overlap_buffer32;
    loop_size = (((framesize * 3) - framesize) >> 2) - 1;

    for (i = loop_size; i >= 0; i--) {
      WORD32 win_op;
      WORD16 win_val;
      win_val = *p_win++;
      win_op = ixheaac_mult32x16in32(*ptr_z++, (win_val));
      *p_out2++ =
          ixheaac_add32_sat(ixheaac_shl32(win_op, q_shift), *ptr_out++);

      win_val = *p_win++;
      win_op = ixheaac_mult32x16in32(*ptr_z++, (win_val));
      *p_out2++ =
          ixheaac_add32_sat(ixheaac_shl32(win_op, q_shift), *ptr_out++);

      win_val = *p_win++;
      win_op = ixheaac_mult32x16in32(*ptr_z++, (win_val));
      *p_out2++ =
          ixheaac_add32_sat(ixheaac_shl32(win_op, q_shift), *ptr_out++);

      win_val = *p_win++;
      win_op = ixheaac_mult32x16in32(*ptr_z++, (win_val));
      *p_out2++ =
          ixheaac_add32_sat(ixheaac_shl32(win_op, q_shift), *ptr_out++);
    }

    loop_size = ((((framesize << 2) - delay) - (framesize * 3)) >> 2) - 1;
    for (i = loop_size; i >= 0; i--) {
      WORD32 win_op;
      WORD16 win_val;

      win_val = *p_win++;
      win_op = ixheaac_mult32x16in32(*ptr_z++, (win_val));
      *p_out2++ = ixheaac_shl32(win_op, q_shift);

      win_val = *p_win++;
      win_op = ixheaac_mult32x16in32(*ptr_z++, (win_val));
      *p_out2++ = ixheaac_shl32(win_op, q_shift);

      win_val = *p_win++;
      win_op = ixheaac_mult32x16in32(*ptr_z++, (win_val));
      *p_out2++ = ixheaac_shl32(win_op, q_shift);

      win_val = *p_win++;
      win_op = ixheaac_mult32x16in32(*ptr_z++, (win_val));
      *p_out2++ = ixheaac_shl32(win_op, q_shift);
    }
  } else {
    q_shift = -q_shift;

    for (i = (delay)-1; i >= 0; i--) {
      WORD32 win_op;
      WORD32 win_ovadd_op;
      WORD16 win_val;

      win_val = *p_win++;
      win_op = ixheaac_mult32x16in32(*ptr_z++, (win_val));

      win_ovadd_op =
          ixheaac_add32_sat(ixheaac_shr32(win_op, q_shift), *ptr_out++);

      *out_samples = ixheaac_round16(ixheaac_shl32(win_ovadd_op, 1));
      out_samples += stride;

      win_val = *p_win++;
      win_op = ixheaac_mult32x16in32(*ptr_z++, (win_val));

      win_ovadd_op =
          ixheaac_add32_sat(ixheaac_shr32(win_op, q_shift), *ptr_out++);

      *out_samples = ixheaac_round16(ixheaac_shl32(win_ovadd_op, 1));
      out_samples += stride;

      win_val = *p_win++;
      win_op = ixheaac_mult32x16in32(*ptr_z++, (win_val));

      win_ovadd_op =
          ixheaac_add32_sat(ixheaac_shr32(win_op, q_shift), *ptr_out++);

      *out_samples = ixheaac_round16(ixheaac_shl32(win_ovadd_op, 1));
      out_samples += stride;

      win_val = *p_win++;
      win_op = ixheaac_mult32x16in32(*ptr_z++, (win_val));

      win_ovadd_op =
          ixheaac_add32_sat(ixheaac_shr32(win_op, q_shift), *ptr_out++);

      *out_samples = ixheaac_round16(ixheaac_shl32(win_ovadd_op, 1));
      out_samples += stride;
    }

    p_out2 = p_overlap_buffer32;
    loop_size = (((framesize * 3) - framesize) >> 2) - 1;

    for (i = loop_size; i >= 0; i--) {
      WORD32 win_op;
      WORD16 win_val;
      win_val = *p_win++;
      win_op = ixheaac_mult32x16in32(*ptr_z++, (win_val));
      *p_out2++ =
          ixheaac_add32_sat(ixheaac_shr32(win_op, q_shift), *ptr_out++);

      win_val = *p_win++;
      win_op = ixheaac_mult32x16in32(*ptr_z++, (win_val));
      *p_out2++ =
          ixheaac_add32_sat(ixheaac_shr32(win_op, q_shift), *ptr_out++);

      win_val = *p_win++;
      win_op = ixheaac_mult32x16in32(*ptr_z++, (win_val));
      *p_out2++ =
          ixheaac_add32_sat(ixheaac_shr32(win_op, q_shift), *ptr_out++);

      win_val = *p_win++;
      win_op = ixheaac_mult32x16in32(*ptr_z++, (win_val));
      *p_out2++ =
          ixheaac_add32_sat(ixheaac_shr32(win_op, q_shift), *ptr_out++);
    }
    loop_size = ((((framesize << 2) - delay) - (framesize * 3)) >> 2) - 1;
    for (i = loop_size; i >= 0; i--) {
      WORD32 win_op;
      WORD16 win_val;
      win_val = *p_win++;
      win_op = ixheaac_mult32x16in32(*ptr_z++, (win_val));
      *p_out2++ = ixheaac_shr32(win_op, q_shift);

      win_val = *p_win++;
      win_op = ixheaac_mult32x16in32(*ptr_z++, (win_val));
      *p_out2++ = ixheaac_shr32(win_op, q_shift);

      win_val = *p_win++;
      win_op = ixheaac_mult32x16in32(*ptr_z++, (win_val));
      *p_out2++ = ixheaac_shr32(win_op, q_shift);

      win_val = *p_win++;
      win_op = ixheaac_mult32x16in32(*ptr_z++, (win_val));
      *p_out2++ = ixheaac_shr32(win_op, q_shift);
    }
  }
}

void ixheaacd_eld_dec_windowing_32bit(WORD32 *ptr_spect_coeff,
                                      const WORD16 *p_win, WORD32 framesize,
                                      WORD16 q_shift, WORD32 *p_overlap_buffer,
                                      const WORD16 stride, WORD32 *out_samples)

{
  WORD32 i = 0;
  WORD32 loop_size;
  WORD32 *ptr_z = ptr_spect_coeff;

  WORD32 *ptr_out, *p_out2;
  WORD32 *p_overlap_buffer32 = (WORD32 *)p_overlap_buffer;
  WORD32 delay = framesize >> 2;

  ptr_z = ptr_spect_coeff + delay;
  p_win += delay;
  ptr_out = p_overlap_buffer32;

  q_shift = q_shift + 2;

  if (q_shift >= 0) {
    for (i = (delay)-1; i >= 0; i--) {
      WORD32 win_op;
      WORD16 win_val;

      win_val = *p_win++;

      win_op = ixheaac_mult32x16in32(*ptr_z++, (win_val));

      *out_samples =
          ixheaac_add32_sat(ixheaac_shl32(win_op, q_shift), *ptr_out++);

      out_samples += stride;

      win_val = *p_win++;

      win_op = ixheaac_mult32x16in32(*ptr_z++, (win_val));

      *out_samples =
          ixheaac_add32_sat(ixheaac_shl32(win_op, q_shift), *ptr_out++);

      out_samples += stride;
      win_val = *p_win++;

      win_op = ixheaac_mult32x16in32(*ptr_z++, (win_val));

      *out_samples =
          ixheaac_add32_sat(ixheaac_shl32(win_op, q_shift), *ptr_out++);

      out_samples += stride;

      win_val = *p_win++;

      win_op = ixheaac_mult32x16in32(*ptr_z++, (win_val));

      *out_samples =
          ixheaac_add32_sat(ixheaac_shl32(win_op, q_shift), *ptr_out++);

      out_samples += stride;
    }

    p_out2 = p_overlap_buffer32;
    loop_size = (((framesize * 3) - framesize) >> 2) - 1;

    for (i = loop_size; i >= 0; i--) {
      WORD32 win_op;
      WORD16 win_val;
      win_val = *p_win++;
      win_op = ixheaac_mult32x16in32(*ptr_z++, (win_val));
      *p_out2++ =
          ixheaac_add32_sat(ixheaac_shl32(win_op, q_shift), *ptr_out++);

      win_val = *p_win++;
      win_op = ixheaac_mult32x16in32(*ptr_z++, (win_val));
      *p_out2++ =
          ixheaac_add32_sat(ixheaac_shl32(win_op, q_shift), *ptr_out++);

      win_val = *p_win++;
      win_op = ixheaac_mult32x16in32(*ptr_z++, (win_val));
      *p_out2++ =
          ixheaac_add32_sat(ixheaac_shl32(win_op, q_shift), *ptr_out++);

      win_val = *p_win++;
      win_op = ixheaac_mult32x16in32(*ptr_z++, (win_val));
      *p_out2++ =
          ixheaac_add32_sat(ixheaac_shl32(win_op, q_shift), *ptr_out++);
    }

    loop_size = ((((framesize << 2) - delay) - (framesize * 3)) >> 2) - 1;
    for (i = loop_size; i >= 0; i--) {
      WORD32 win_op;
      WORD16 win_val;

      win_val = *p_win++;
      win_op = ixheaac_mult32x16in32(*ptr_z++, (win_val));
      *p_out2++ = ixheaac_shl32(win_op, q_shift);

      win_val = *p_win++;
      win_op = ixheaac_mult32x16in32(*ptr_z++, (win_val));
      *p_out2++ = ixheaac_shl32(win_op, q_shift);

      win_val = *p_win++;
      win_op = ixheaac_mult32x16in32(*ptr_z++, (win_val));
      *p_out2++ = ixheaac_shl32(win_op, q_shift);

      win_val = *p_win++;
      win_op = ixheaac_mult32x16in32(*ptr_z++, (win_val));
      *p_out2++ = ixheaac_shl32(win_op, q_shift);
    }
  } else {
    q_shift = -q_shift;

    for (i = (delay)-1; i >= 0; i--) {
      WORD32 win_op;
      WORD16 win_val;

      win_val = *p_win++;
      win_op = ixheaac_mult32x16in32(*ptr_z++, (win_val));

      *out_samples =
          ixheaac_add32_sat(ixheaac_shr32(win_op, q_shift), *ptr_out++);

      out_samples += stride;

      win_val = *p_win++;
      win_op = ixheaac_mult32x16in32(*ptr_z++, (win_val));

      *out_samples =
          ixheaac_add32_sat(ixheaac_shr32(win_op, q_shift), *ptr_out++);

      out_samples += stride;

      win_val = *p_win++;
      win_op = ixheaac_mult32x16in32(*ptr_z++, (win_val));

      *out_samples =
          ixheaac_add32_sat(ixheaac_shr32(win_op, q_shift), *ptr_out++);

      out_samples += stride;

      win_val = *p_win++;
      win_op = ixheaac_mult32x16in32(*ptr_z++, (win_val));

      *out_samples =
          ixheaac_add32_sat(ixheaac_shr32(win_op, q_shift), *ptr_out++);

      out_samples += stride;
    }

    p_out2 = p_overlap_buffer32;
    loop_size = (((framesize * 3) - framesize) >> 2) - 1;

    for (i = loop_size; i >= 0; i--) {
      WORD32 win_op;
      WORD16 win_val;
      win_val = *p_win++;
      win_op = ixheaac_mult32x16in32(*ptr_z++, (win_val));
      *p_out2++ =
          ixheaac_add32_sat(ixheaac_shr32(win_op, q_shift), *ptr_out++);

      win_val = *p_win++;
      win_op = ixheaac_mult32x16in32(*ptr_z++, (win_val));
      *p_out2++ =
          ixheaac_add32_sat(ixheaac_shr32(win_op, q_shift), *ptr_out++);

      win_val = *p_win++;
      win_op = ixheaac_mult32x16in32(*ptr_z++, (win_val));
      *p_out2++ =
          ixheaac_add32_sat(ixheaac_shr32(win_op, q_shift), *ptr_out++);

      win_val = *p_win++;
      win_op = ixheaac_mult32x16in32(*ptr_z++, (win_val));
      *p_out2++ =
          ixheaac_add32_sat(ixheaac_shr32(win_op, q_shift), *ptr_out++);
    }
    loop_size = ((((framesize << 2) - delay) - (framesize * 3)) >> 2) - 1;
    for (i = loop_size; i >= 0; i--) {
      WORD32 win_op;
      WORD16 win_val;
      win_val = *p_win++;
      win_op = ixheaac_mult32x16in32(*ptr_z++, (win_val));
      *p_out2++ = ixheaac_shr32(win_op, q_shift);

      win_val = *p_win++;
      win_op = ixheaac_mult32x16in32(*ptr_z++, (win_val));
      *p_out2++ = ixheaac_shr32(win_op, q_shift);

      win_val = *p_win++;
      win_op = ixheaac_mult32x16in32(*ptr_z++, (win_val));
      *p_out2++ = ixheaac_shr32(win_op, q_shift);

      win_val = *p_win++;
      win_op = ixheaac_mult32x16in32(*ptr_z++, (win_val));
      *p_out2++ = ixheaac_shr32(win_op, q_shift);
    }
  }
}
