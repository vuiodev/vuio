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
#include <stdio.h>
#include <string.h>
#include <stdlib.h>
#include <setjmp.h>

#include "ixheaac_type_def.h"

#include "ixheaacd_cnst.h"

#include "ixheaacd_bitbuffer.h"

#include "ixheaacd_interface.h"
#include "ixheaacd_acelp_info.h"

#include "ixheaacd_tns_usac.h"
#include "ixheaacd_acelp_info.h"

#include "ixheaacd_sbrdecsettings.h"
#include "ixheaacd_info.h"
#include "ixheaacd_sbr_common.h"
#include "ixheaacd_drc_data_struct.h"
#include "ixheaacd_drc_dec.h"

#include "ixheaacd_sbrdecoder.h"
#include "ixheaacd_mps_polyphase.h"
#include "ixheaac_sbr_const.h"

#include "ixheaacd_ec_defines.h"
#include "ixheaacd_ec_rom.h"
#include "ixheaacd_ec_struct_def.h"
#include "ixheaacd_main.h"
#include "ixheaacd_arith_dec.h"
#include "ixheaacd_tns_usac.h"

#include "ixheaacd_bit_extract.h"

#include "ixheaac_constants.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops40.h"

#include "ixheaacd_func_def.h"

#include "ixheaacd_defines.h"
#include "ixheaacd_windows.h"

#include "ixheaacd_vec_baisc_ops.h"
#include "ixheaacd_config.h"
#include "ixheaacd_defines.h"
#include "ixheaacd_aac_rom.h"
#include "ixheaacd_pulsedata.h"
#include "ixheaacd_pns.h"
#include "ixheaacd_channelinfo.h"
#include "ixheaacd_ec.h"
#include "ixheaacd_error_codes.h"

const WORD16 ixheaacd_mdst_fcoeff_long_sin[] = {0, 0, -16384, 0, 16384, 0, 0};
const WORD16 ixheaacd_mdst_fcoeff_long_kbd[] = {-2998, 0, -19052, 0,
                                                19052, 0, 2998};
const WORD16 ixheaacd_mdst_fcoeff_long_sin_kbd[] = {-1499, -1876, -17718, 0,
                                                    17718, 1876,  1499};
const WORD16 ixheaacd_mdst_fcoeff_long_kbd_sin[] = {-1499, 1876,  -17718, 0,
                                                    17718, -1876, 1499};

const WORD16 *const ixheaacd_mdst_fcoeff_longshort_curr[2][2] = {
    {ixheaacd_mdst_fcoeff_long_sin, ixheaacd_mdst_fcoeff_long_sin_kbd},
    {ixheaacd_mdst_fcoeff_long_kbd_sin, ixheaacd_mdst_fcoeff_long_kbd}};

const WORD16 ixheaacd_mdst_fcoeff_start_sin[] = {-3364, -3401, -18584, 0,
                                                 18584, 3401,  3364};
const WORD16 ixheaacd_mdst_fcoeff_start_kbd[] = {-4932, -1572, -19942, 0,
                                                 19942, 1572,  4932};
const WORD16 ixheaacd_mdst_fcoeff_start_sin_kbd[] = {-3433, -3447, -18608, 0,
                                                     18608, 3447,  3433};
const WORD16 ixheaacd_mdst_fcoeff_start_kbd_sin[] = {-4863, -1525, -19918, 0,
                                                     19918, 1525,  4863};

const WORD16 *const ixheaacd_mdst_fcoeff_start_curr[2][2] = {
    {ixheaacd_mdst_fcoeff_start_sin, ixheaacd_mdst_fcoeff_start_sin_kbd},
    {ixheaacd_mdst_fcoeff_start_kbd_sin, ixheaacd_mdst_fcoeff_start_kbd}};

const WORD16 ixheaacd_mdst_fcoeff_stop_sin[] = {-3364, 3401,  -18584, 0,
                                                18584, -3401, 3364};
const WORD16 ixheaacd_mdst_fcoeff_stop_kbd[] = {-4932, 1572,  -19942, 0,
                                                19942, -1572, 4932};
const WORD16 ixheaacd_mdst_fcoeff_stop_sin_kbd[] = {-4863, 1525,  -19918, 0,
                                                    19918, -1525, 4863};
const WORD16 ixheaacd_mdst_fcoeff_stop_kbd_sin[] = {-3433, 3447,  -18608, 0,
                                                    18608, -3447, 3433};

const WORD16 *const ixheaacd_mdst_fcoeff_stop_cur[2][2] = {
    {ixheaacd_mdst_fcoeff_stop_sin, ixheaacd_mdst_fcoeff_stop_sin_kbd},
    {ixheaacd_mdst_fcoeff_stop_kbd_sin, ixheaacd_mdst_fcoeff_stop_kbd}};

const WORD16 ixheaacd_mdst_fcoeff_stopstart_sin[] = {-6728, 0, -20785, 0,
                                                     20785, 0, 6728};
const WORD16 ixheaacd_mdst_fcoeff_stopstart_kbd[] = {-6866, -0, -20831, 0,
                                                     20831, 0,  6866};
const WORD16 ixheaacd_mdst_fcoeff_stopstart_sin_kbd[] = {-6797, -46, -20808, 0,
                                                         20808, 46,  6797};
const WORD16 ixheaacd_mdst_fcoeff_stopstart_kbd_sin[] = {-6797, 46, -20808, 0,
                                                         20808, 46, 6797};

const WORD16 *const ixheaacd_mdst_fcoeff_stopstart_cur[2][2] = {
    {ixheaacd_mdst_fcoeff_stopstart_sin,
     ixheaacd_mdst_fcoeff_stopstart_sin_kbd},
    {ixheaacd_mdst_fcoeff_stopstart_kbd_sin,
     ixheaacd_mdst_fcoeff_stopstart_kbd}};

const WORD16 ixheaacd_mdst_fcoeff_l_s_start_left_sin[] = {
    -0, 3477, 8192, 10430, 8192, 3477, -0};
const WORD16 ixheaacd_mdst_fcoeff_l_s_start_left_kbd[] = {
    1950, 4054, 6114, 6982, 6114, 4054, 1950};

const WORD16 ixheaacd_mdst_fcoeff_stop_stopstart_left_sin[] = {
    1262, 1285, 1299, 1304, 1299, 1285, 1262};
const WORD16 ixheaacd_mdst_fcoeff_stop_stopstart_left_kbd[] = {
    857, 866, 871, 873, 871, 866, 857};

const WORD16 *const ixheaacd_mdst_fcoeff_l_s_start_left_prev[2] = {
    ixheaacd_mdst_fcoeff_l_s_start_left_sin,
    ixheaacd_mdst_fcoeff_l_s_start_left_kbd};
const WORD16 *const ixheaacd_mdst_fcoeff_stop_stopstart_left_prev[2] = {
    ixheaacd_mdst_fcoeff_stop_stopstart_left_sin,
    ixheaacd_mdst_fcoeff_stop_stopstart_left_kbd};

#define ONE_BY_TWO_POW_14 0.00006103515625
#define ONE_BY_TWO_POW_15 0.000030517578125

void ixheaacd_usac_cplx_save_prev(ia_sfb_info_struct *info, WORD32 *l_spec,
                                  WORD32 *r_spec, WORD32 *l_spec_prev,
                                  WORD32 *r_spec_prev) {
  WORD32 ixheaacd_drc_offset;

  ixheaacd_drc_offset = info->samp_per_bk - info->bins_per_sbk;

  memcpy(l_spec_prev + ixheaacd_drc_offset, l_spec + ixheaacd_drc_offset,
         sizeof(WORD32) * info->bins_per_sbk);
  memcpy(r_spec_prev + ixheaacd_drc_offset, r_spec + ixheaacd_drc_offset,
         sizeof(WORD32) * info->bins_per_sbk);
}

static VOID ixheaacd_cplx_pred_data(
    ia_usac_data_struct *usac_data,
    ia_usac_tmp_core_coder_struct *pstr_core_coder, WORD32 num_window_groups,
    ia_bit_buf_struct *it_bit_buff) {
  ia_huff_code_book_struct *ptr_huff_code_book = &ixheaacd_book;
  const ia_huff_code_word_struct *ptr_huff_code_word =
      ptr_huff_code_book->pstr_huff_code_word;
  WORD32 cplx_pred_all;
  WORD32 delta_code_time;
  WORD32 g, sfb;
  WORD32 dpcm_alpha, last_alpha_q_re, last_alpha_q_im;
  UWORD8 max_sfb_ste = pstr_core_coder->max_sfb_ste;

  WORD32(*alpha_q_re)[SFB_NUM_MAX] = usac_data->alpha_q_re;
  WORD32(*alpha_q_im)[SFB_NUM_MAX] = usac_data->alpha_q_im;
  WORD32 *alpha_q_re_prev = usac_data->alpha_q_re_prev;
  WORD32 *alpha_q_im_prev = usac_data->alpha_q_im_prev;
  UWORD8(*cplx_pred_used)[SFB_NUM_MAX] = usac_data->cplx_pred_used;

  cplx_pred_all = ixheaacd_read_bits_buf(it_bit_buff, 1);

  if (cplx_pred_all == 0) {
    for (g = 0; g < num_window_groups; g++) {
      for (sfb = 0; sfb < max_sfb_ste; sfb += SFB_PER_PRED_BAND) {
        cplx_pred_used[g][sfb] = ixheaacd_read_bits_buf(it_bit_buff, 1);

        if (sfb + 1 < max_sfb_ste)
          cplx_pred_used[g][sfb + 1] = cplx_pred_used[g][sfb];
      }
      for (sfb = max_sfb_ste; sfb < SFB_NUM_MAX; sfb++)
        cplx_pred_used[g][sfb] = 0;
    }
  } else {
    for (g = 0; g < num_window_groups; g++) {
      for (sfb = 0; sfb < max_sfb_ste; sfb++) cplx_pred_used[g][sfb] = 1;

      for (sfb = max_sfb_ste; sfb < SFB_NUM_MAX; sfb++)
        cplx_pred_used[g][sfb] = 0;
    }
  }

  pstr_core_coder->pred_dir = ixheaacd_read_bits_buf(it_bit_buff, 1);

  pstr_core_coder->complex_coef = ixheaacd_read_bits_buf(it_bit_buff, 1);

  if (pstr_core_coder->complex_coef) {
    if (usac_data->usac_independency_flg)
      pstr_core_coder->use_prev_frame = 0;
    else
      pstr_core_coder->use_prev_frame = ixheaacd_read_bits_buf(it_bit_buff, 1);
  }

  if (usac_data->usac_independency_flg)
    delta_code_time = 0;
  else
    delta_code_time = ixheaacd_read_bits_buf(it_bit_buff, 1);

  for (g = 0; g < num_window_groups; g++) {
    for (sfb = 0; sfb < max_sfb_ste; sfb += SFB_PER_PRED_BAND) {
      if (delta_code_time == 1) {
        last_alpha_q_re = alpha_q_re_prev[sfb];
        last_alpha_q_im = alpha_q_im_prev[sfb];
      } else {
        if (sfb > 0) {
          last_alpha_q_re = alpha_q_re[g][sfb - 1];
          last_alpha_q_im = alpha_q_im[g][sfb - 1];
        } else {
          last_alpha_q_re = last_alpha_q_im = 0;
        }
      }

      if (cplx_pred_used[g][sfb] == 1) {
        dpcm_alpha =
            -ixheaacd_huff_codeword(ptr_huff_code_word, 0, it_bit_buff) + 60;
        alpha_q_re[g][sfb] = dpcm_alpha + last_alpha_q_re;

        if (pstr_core_coder->complex_coef) {
          dpcm_alpha =
              -ixheaacd_huff_codeword(ptr_huff_code_word, 0, it_bit_buff) + 60;
          alpha_q_im[g][sfb] = dpcm_alpha + last_alpha_q_im;
        } else {
          alpha_q_im[g][sfb] = 0;
        }
      } else {
        alpha_q_re[g][sfb] = 0;
        alpha_q_im[g][sfb] = 0;
      }

      if ((sfb + 1) < max_sfb_ste) {
        alpha_q_re[g][sfb + 1] = alpha_q_re[g][sfb];
        alpha_q_im[g][sfb + 1] = alpha_q_im[g][sfb];
      }

      alpha_q_re_prev[sfb] = alpha_q_re[g][sfb];
      alpha_q_im_prev[sfb] = alpha_q_im[g][sfb];
    }
    for (sfb = max_sfb_ste; sfb < SFB_NUM_MAX; sfb++) {
      alpha_q_re[g][sfb] = 0;
      alpha_q_im[g][sfb] = 0;
      alpha_q_re_prev[sfb] = 0;
      alpha_q_im_prev[sfb] = 0;
    }
  }

  return;
}

static WORD32 ixheaacd_read_ms_mask(
    ia_usac_data_struct *usac_data,
    ia_usac_tmp_core_coder_struct *pstr_core_coder,
    ia_bit_buf_struct *it_bit_buff, WORD32 chn) {
  WORD32 g, sfb;
  WORD32 ms_mask_present;

  UWORD8 *sfb_group = usac_data->group_dis[chn];
  UWORD8 max_sfb = pstr_core_coder->max_sfb_ste;
  UWORD8 *ms_used = usac_data->ms_used[chn];
  ia_sfb_info_struct *info = usac_data->pstr_sfb_info[chn];

  ms_mask_present = ixheaacd_read_bits_buf(it_bit_buff, 2);

  switch (ms_mask_present) {
    case 0:

      break;

    case 1:
      for (g = 0; g < info->max_win_len; g = *sfb_group++) {
        for (sfb = 0; sfb < max_sfb; sfb++) {
          *ms_used = ixheaacd_read_bits_buf(it_bit_buff, 1);
          ms_used++;
        }
        for (; sfb < info->sfb_per_sbk; sfb++) {
          *ms_used = 0;
          ms_used++;
        }
      }

      break;
    case 2:
      for (g = 0; g < info->max_win_len; g = *sfb_group++)
        for (sfb = 0; sfb < info->sfb_per_sbk; sfb++) *ms_used++ = 1;
      break;

    case 3:

      ixheaacd_cplx_pred_data(usac_data, pstr_core_coder, info->num_groups,
                              it_bit_buff);
      return 3;
  }

  for (sfb = 0; sfb < SFB_NUM_MAX; sfb++) {
    usac_data->alpha_q_re_prev[sfb] = 0;
    usac_data->alpha_q_im_prev[sfb] = 0;
  }
  return ms_mask_present;
}

VOID ixheaacd_ms_stereo(ia_usac_data_struct *usac_data, WORD32 *r_spec,
                        WORD32 *l_spec, WORD32 chn, WORD32 nband) {
  WORD32 temp_r, temp_l;
  WORD32 sfb, k, grp, grp_len;
  ia_sfb_info_struct *ptr_sfb_info = usac_data->pstr_sfb_info[chn];
  UWORD8 *ms_used = usac_data->ms_used[chn];
  WORD32 ixheaacd_drc_offset = 0;

  for (grp = 0; grp < ptr_sfb_info->num_groups; grp++) {
    for (grp_len = 0; grp_len < ptr_sfb_info->group_len[grp]; grp_len++) {
      ixheaacd_drc_offset = 0;
      for (sfb = 0; sfb < nband; sfb++) {
        ixheaacd_drc_offset += ptr_sfb_info->sfb_width[sfb];
        if (ms_used[sfb]) {
          for (k = 0; k < ptr_sfb_info->sfb_width[sfb]; k++) {
            temp_r = *r_spec;
            temp_l = *l_spec;
            *l_spec = ixheaac_add32_sat(temp_r, temp_l);
            *r_spec = ixheaac_sub32_sat(temp_l, temp_r);
            r_spec++;
            l_spec++;
          }
        } else {
          r_spec += ptr_sfb_info->sfb_width[sfb];
          l_spec += ptr_sfb_info->sfb_width[sfb];
        }
      }

      l_spec = l_spec + ptr_sfb_info->bins_per_sbk - ixheaacd_drc_offset;
      r_spec = r_spec + ptr_sfb_info->bins_per_sbk - ixheaacd_drc_offset;
    }

    ms_used += ptr_sfb_info->sfb_per_sbk;
  }
}

static VOID ixheaacd_filter_and_add(const WORD32 *in, const WORD32 length,
                                    const WORD16 *filter, WORD32 *out,
                                    const WORD32 factor_even,
                                    const WORD32 factor_odd) {
  WORD32 i;
  WORD64 sum;

  sum = ixheaac_mult32x32in64(in[2], filter[0]);
  sum = ixheaac_mac32x32in64(sum, in[1], filter[1]);
  sum = ixheaac_mac32x32in64(sum, in[0], filter[2]);
  sum = ixheaac_mac32x32in64_n(sum, &in[0], &filter[3], 4);
  *out = ixheaac_add32_sat(
      *out, ixheaac_sat64_32((((WORD64)sum * (WORD64)factor_even) >> 15)));
  out++;

  sum = ixheaac_mult32x32in64(in[1], filter[0]);
  sum = ixheaac_mac32x32in64(sum, in[0], filter[1]);
  sum = ixheaac_mac32x32in64_n(sum, &in[0], &filter[2], 5);
  *out = ixheaac_add32_sat(
      *out, ixheaac_sat64_32((((WORD64)sum * (WORD64)factor_odd) >> 15)));
  out++;

  sum = ixheaac_mult32x32in64(in[0], filter[0]);
  sum = ixheaac_mac32x32in64_n(sum, &in[0], &filter[1], 6);
  *out = ixheaac_add32_sat(
      *out, ixheaac_sat64_32((((WORD64)sum * (WORD64)factor_even) >> 15)));
  out++;

  for (i = 3; i < length - 4; i += 2) {
    sum = 0;
    sum = ixheaac_mac32x32in64_7(&in[i - 3], filter);
    *out = ixheaac_add32_sat(
        *out, ixheaac_sat64_32((((WORD64)sum * (WORD64)factor_odd) >> 15)));
    out++;

    sum = 0;
    sum = ixheaac_mac32x32in64_7(&in[i - 2], filter);
    *out = ixheaac_add32_sat(
        *out, ixheaac_sat64_32((((WORD64)sum * (WORD64)factor_even) >> 15)));
    out++;
  }
  i = length - 3;
  sum = 0;
  sum = ixheaac_mac32x32in64_n(sum, &in[i - 3], filter, 6);
  sum = ixheaac_mac32x32in64(sum, in[i + 2], filter[6]);
  *out = ixheaac_add32_sat(
      *out, ixheaac_sat64_32((((WORD64)sum * (WORD64)factor_odd) >> 15)));

  out++;
  i = length - 2;
  sum = 0;
  sum = ixheaac_mac32x32in64_n(sum, &in[i - 3], filter, 5);
  sum = ixheaac_mac32x32in64(sum, in[i + 1], filter[5]);
  sum = ixheaac_mac32x32in64(sum, in[i], filter[6]);

  *out = ixheaac_add32_sat(
      *out, ixheaac_sat64_32((((WORD64)sum * (WORD64)factor_even) >> 15)));
  out++;

  i = length - 1;
  sum = 0;
  sum = ixheaac_mac32x32in64_n(sum, &in[i - 3], filter, 4);
  sum = ixheaac_mac32x32in64(sum, in[i], filter[4]);
  sum = ixheaac_mac32x32in64(sum, in[i - 1], filter[5]);
  sum = ixheaac_mac32x32in64(sum, in[i - 2], filter[6]);

  *out = ixheaac_add32_sat(
      *out, ixheaac_sat64_32((((WORD64)sum * (WORD64)factor_odd) >> 15)));
}

static VOID ixheaacd_estimate_dmx_im(const WORD32 *dmx_re,
                                     const WORD32 *dmx_re_prev, WORD32 *dmx_im,
                                     ia_sfb_info_struct *pstr_sfb_info,
                                     WORD32 window, const WORD32 w_shape,
                                     const WORD32 prev_w_shape) {
  WORD32 i;
  const WORD16 *mdst_fcoeff_curr, *mdst_fcoeff_prev;

  switch (window) {
    case ONLY_LONG_SEQUENCE:
    case EIGHT_SHORT_SEQUENCE:
      mdst_fcoeff_curr =
          ixheaacd_mdst_fcoeff_longshort_curr[prev_w_shape][w_shape];
      mdst_fcoeff_prev = ixheaacd_mdst_fcoeff_l_s_start_left_prev[prev_w_shape];
      break;
    case LONG_START_SEQUENCE:
      mdst_fcoeff_curr = ixheaacd_mdst_fcoeff_start_curr[prev_w_shape][w_shape];
      mdst_fcoeff_prev = ixheaacd_mdst_fcoeff_l_s_start_left_prev[prev_w_shape];
      break;
    case LONG_STOP_SEQUENCE:
      mdst_fcoeff_curr = ixheaacd_mdst_fcoeff_stop_cur[prev_w_shape][w_shape];
      mdst_fcoeff_prev =
          ixheaacd_mdst_fcoeff_stop_stopstart_left_prev[prev_w_shape];
      break;
    case STOP_START_SEQUENCE:
      mdst_fcoeff_curr =
          ixheaacd_mdst_fcoeff_stopstart_cur[prev_w_shape][w_shape];
      mdst_fcoeff_prev =
          ixheaacd_mdst_fcoeff_stop_stopstart_left_prev[prev_w_shape];
      break;
    default:
      mdst_fcoeff_curr =
          ixheaacd_mdst_fcoeff_stopstart_cur[prev_w_shape][w_shape];
      mdst_fcoeff_prev =
          ixheaacd_mdst_fcoeff_stop_stopstart_left_prev[prev_w_shape];
      break;
  }

  for (i = 0; i < pstr_sfb_info->max_win_len; i++) {
    ixheaacd_filter_and_add(dmx_re, pstr_sfb_info->bins_per_sbk,
                            mdst_fcoeff_curr, dmx_im, 1, 1);

    if (dmx_re_prev)
      ixheaacd_filter_and_add(dmx_re_prev, pstr_sfb_info->bins_per_sbk,
                              mdst_fcoeff_prev, dmx_im, -1, 1);

    dmx_re_prev = dmx_re;
    dmx_re += pstr_sfb_info->bins_per_sbk;
    dmx_im += pstr_sfb_info->bins_per_sbk;
  }
  return;
}

static VOID ixheaacd_cplx_pred_upmixing(
    ia_usac_data_struct *usac_data, WORD32 *l_spec, WORD32 *r_spec,
    ia_usac_tmp_core_coder_struct *pstr_core_coder, WORD32 chn) {
  ia_sfb_info_struct *pstr_sfb_info = usac_data->pstr_sfb_info[chn];
  WORD32 *dmx_re = &usac_data->scratch_buffer[0];
  WORD32 *dmx_im = &usac_data->x_ac_dec[0];

  WORD32 grp, sfb, grp_len, i = 0, k;
  WORD32 *dmx_re_prev = usac_data->dmx_re_prev;
  WORD32(*alpha_q_re)[SFB_NUM_MAX] = usac_data->alpha_q_re;
  WORD32(*alpha_q_im)[SFB_NUM_MAX] = usac_data->alpha_q_im;

  UWORD8(*cplx_pred_used)[SFB_NUM_MAX] = usac_data->cplx_pred_used;

  WORD32 alpha_q_re_temp;
  WORD32 alpha_q_im_temp;
  WORD32 factor = 1;

  if (pstr_core_coder->pred_dir) factor = -1;

  for (grp = 0; grp < pstr_sfb_info->num_groups; grp++) {
    for (grp_len = 0; grp_len < pstr_sfb_info->group_len[grp]; grp_len++) {
      for (sfb = 0; sfb < pstr_sfb_info->sfb_per_sbk; sfb++) {
        if (cplx_pred_used[grp][sfb] == 1) {
          memcpy(&dmx_re[i], &l_spec[i],
                 pstr_sfb_info->sfb_width[sfb] * sizeof(WORD32));
          i += pstr_sfb_info->sfb_width[sfb];
        }

        else {
          for (k = 0; k < pstr_sfb_info->sfb_width[sfb]; k++, i++) {
            dmx_re[i] = (WORD32)(
                ((WORD64)l_spec[i] + ((WORD64)factor * (WORD64)r_spec[i])) >>
                1);
          }
        }
      }
    }
  }

  memset(dmx_im, 0, sizeof(WORD32) * BLOCK_LEN_LONG);

  if (pstr_core_coder->complex_coef) {
    WORD32 *p_dmx_re_prev =
        pstr_core_coder->use_prev_frame ? dmx_re_prev : NULL;
    ixheaacd_estimate_dmx_im(dmx_re, p_dmx_re_prev, dmx_im, pstr_sfb_info,
                             usac_data->window_sequence[chn],
                             usac_data->window_shape[chn],
                             usac_data->window_shape_prev[chn]);

    for (grp = 0, i = 0; grp < pstr_sfb_info->num_groups; grp++) {
      for (grp_len = 0; grp_len < pstr_sfb_info->group_len[grp]; grp_len++) {
        for (sfb = 0; sfb < pstr_sfb_info->sfb_per_sbk; sfb++) {
          alpha_q_re_temp = ixheaac_sat64_32(ixheaac_mult32x32in64(alpha_q_re[grp][sfb], 1677722));
          alpha_q_im_temp = ixheaac_sat64_32(ixheaac_mult32x32in64(alpha_q_im[grp][sfb], 1677722));
          if (cplx_pred_used[grp][sfb]) {
            for (k = 0; k < pstr_sfb_info->sfb_width[sfb]; k++, i++) {
              WORD32 mid_side = ixheaac_sub32_sat(
                  ixheaac_sub32_sat(r_spec[i],
                                     (WORD32)((WORD64)ixheaac_mult32x32in64(
                                                  alpha_q_re_temp, l_spec[i]) >>
                                              24)),
                  (WORD32)((WORD64)ixheaac_mult32x32in64(alpha_q_im_temp,
                                                          dmx_im[i]) >>
                           24));
              r_spec[i] = ixheaac_sat64_32((WORD64)factor) *
                          (WORD64)(ixheaac_sub32_sat(l_spec[i], mid_side));
              l_spec[i] = ixheaac_add32_sat(l_spec[i], mid_side);
            }
          } else {
            i += pstr_sfb_info->sfb_width[sfb];
          }
        }
      }
    }
  } else {
    for (grp = 0, i = 0; grp < pstr_sfb_info->num_groups; grp++) {
      for (grp_len = 0; grp_len < pstr_sfb_info->group_len[grp]; grp_len++) {
        for (sfb = 0; sfb < pstr_sfb_info->sfb_per_sbk; sfb++) {
          alpha_q_re_temp = ixheaac_sat64_32(ixheaac_mult32x32in64(alpha_q_re[grp][sfb], 1677722));
          if (cplx_pred_used[grp][sfb]) {
            for (k = 0; k < pstr_sfb_info->sfb_width[sfb]; k++, i++) {
              WORD32 mid_side = ixheaac_sub32_sat(
                  r_spec[i], (WORD32)((WORD64)ixheaac_mult32x32in64(
                                          alpha_q_re_temp, l_spec[i]) >>
                                      24));

              r_spec[i] = ixheaac_sat64_32((WORD64)factor) *
                          (WORD64)(ixheaac_sub32_sat(l_spec[i], mid_side));
              l_spec[i] = ixheaac_add32_sat(l_spec[i], mid_side);
            }
          } else {
            i += pstr_sfb_info->sfb_width[sfb];
          }
        }
      }
    }
  }

  return;
}

static VOID ixheaacd_cplx_prev_mdct_dmx(ia_sfb_info_struct *pstr_sfb_info,
                                        WORD32 *l_spec, WORD32 *r_spec,
                                        WORD32 *dmx_re_prev, WORD32 pred_dir) {
  WORD32 offs, i;
  WORD32 factor = 1;
  if (pred_dir) factor = -1;

  offs = pstr_sfb_info->samp_per_bk - pstr_sfb_info->bins_per_sbk;

  for (i = 0; i < pstr_sfb_info->bins_per_sbk; i++)
    dmx_re_prev[i] = (WORD32)(((WORD64)l_spec[i + offs] +
                               ((WORD64)factor * (WORD64)r_spec[i + offs])) >>
                              1);
}

WORD32 ixheaacd_ics_info(ia_usac_data_struct *usac_data, WORD32 chn,
                         UWORD8 *max_sfb, ia_bit_buf_struct *it_bit_buff,
                         WORD32 window_sequence_last

                         )

{
  WORD32 win;
  WORD32 mask = 0x40;

  UWORD8 *scf_group_ptr = usac_data->group_dis[chn];

  win = ixheaacd_read_bits_buf(it_bit_buff, 2);

  win = usac_data->window_sequence[chn] =
      ixheaacd_win_seq_select(win, window_sequence_last);
  if (win == -1) return -1;

  usac_data->pstr_sfb_info[chn] =
      usac_data->pstr_usac_winmap[usac_data->window_sequence[chn]];

  usac_data->window_shape[chn] = ixheaacd_read_bits_buf(it_bit_buff, 1);

  if (usac_data->pstr_usac_winmap[win]->islong) {
    *max_sfb = ixheaacd_read_bits_buf(it_bit_buff, 6);
    *scf_group_ptr = 1;
  } else {
    WORD32 i, scale_factor_grouping;

    *max_sfb = ixheaacd_read_bits_buf(it_bit_buff, 4);

    scale_factor_grouping = ixheaacd_read_bits_buf(it_bit_buff, 7);

    for (i = 1; i < 8; i++) {
      if (!(scale_factor_grouping & mask)) *scf_group_ptr++ = i;

      mask = mask >> 1;
    }
    *scf_group_ptr++ = i;

    ixheaacd_calc_grp_offset(usac_data->pstr_usac_winmap[win],
                             &usac_data->group_dis[chn][0]);
  }

  if (*max_sfb > usac_data->pstr_sfb_info[chn]->sfb_per_sbk) {
    *max_sfb = usac_data->pstr_sfb_info[chn]->sfb_per_sbk;
    return -1;
  }

  return 0;
}

WORD32 ixheaacd_core_coder_data(WORD32 id, ia_usac_data_struct *usac_data,
                                WORD32 elem_idx, WORD32 chan_offset,
                                ia_bit_buf_struct *it_bit_buff,
                                WORD32 nr_core_coder_channels) {
  WORD32 err_code = 0;
  WORD32 k = 0, ch = 0, chn = 0, left = 0, right = 0;

  ia_usac_tmp_core_coder_struct str_tmp_core_coder = {0};
  ia_usac_tmp_core_coder_struct *pstr_core_coder = &str_tmp_core_coder;
  ia_td_frame_data_struct td_frame;
  jmp_buf local;

  if (usac_data->ec_flag) {
    err_code = setjmp(local);
    it_bit_buff->xaac_jmp_buf = &local;
  }
  if (err_code == 0 &&
      ((usac_data->ec_flag == 0) || (usac_data->frame_ok == 1 && usac_data->ec_flag == 1))) {
    memset(&td_frame, 0, sizeof(td_frame));
    pstr_core_coder->tns_on_lr = 0;
    pstr_core_coder->pred_dir = 0;
    if (id != ID_USAC_LFE) {
      for (ch = 0; ch < nr_core_coder_channels; ch++)
        pstr_core_coder->core_mode[ch] = ixheaacd_read_bits_buf(it_bit_buff, 1);
    } else {
      for (ch = 0; ch < nr_core_coder_channels; ch++) pstr_core_coder->core_mode[ch] = 0;
    }

    if (nr_core_coder_channels == 2 && pstr_core_coder->core_mode[0] == 0 &&
        pstr_core_coder->core_mode[1] == 0) {
      pstr_core_coder->tns_active = ixheaacd_read_bits_buf(it_bit_buff, 1);
      pstr_core_coder->common_window = ixheaacd_read_bits_buf(it_bit_buff, 1);

      if (pstr_core_coder->common_window) {
        left = chan_offset;
        right = chan_offset + 1;

        err_code = ixheaacd_ics_info(usac_data, left, &pstr_core_coder->max_sfb[left],
                                     it_bit_buff, usac_data->window_sequence_last[left]);

        if (err_code == -1) {
          if (usac_data->ec_flag) {
            memcpy(usac_data->max_sfb, pstr_core_coder->max_sfb,
                   sizeof(pstr_core_coder->max_sfb));
            longjmp(*(it_bit_buff->xaac_jmp_buf),
                    IA_XHEAAC_DEC_EXE_NONFATAL_INSUFFICIENT_INPUT_BYTES);
          } else {
            return err_code;
          }
        }

        pstr_core_coder->common_max_sfb = ixheaacd_read_bits_buf(it_bit_buff, 1);

        if (pstr_core_coder->common_max_sfb == 0) {
          if (usac_data->window_sequence[left] == EIGHT_SHORT_SEQUENCE)
            pstr_core_coder->max_sfb[right] = ixheaacd_read_bits_buf(it_bit_buff, 4);
          else
            pstr_core_coder->max_sfb[right] = ixheaacd_read_bits_buf(it_bit_buff, 6);
        } else {
          pstr_core_coder->max_sfb[right] = pstr_core_coder->max_sfb[left];
        }

        pstr_core_coder->max_sfb_ste =
            max(pstr_core_coder->max_sfb[left], pstr_core_coder->max_sfb[right]);

        usac_data->window_sequence[right] = usac_data->window_sequence[left];
        usac_data->window_shape[right] = usac_data->window_shape[left];
        memcpy(&usac_data->group_dis[right][0], &usac_data->group_dis[left][0], 8);
        usac_data->pstr_sfb_info[right] = usac_data->pstr_sfb_info[left];
        if (pstr_core_coder->max_sfb[right] > usac_data->pstr_sfb_info[right]->sfb_per_sbk)
          pstr_core_coder->max_sfb[right] = usac_data->pstr_sfb_info[right]->sfb_per_sbk;

        pstr_core_coder->ms_mask_present[0] =
            ixheaacd_read_ms_mask(usac_data, pstr_core_coder, it_bit_buff, left);
      } else {
        left = chan_offset;
        right = chan_offset + 1;

        pstr_core_coder->ms_mask_present[0] = 0;
        pstr_core_coder->ms_mask_present[1] = 0;

        for (k = 0; k < SFB_NUM_MAX; k++) {
          usac_data->alpha_q_re_prev[k] = 0;
          usac_data->alpha_q_im_prev[k] = 0;
        }
      }

      if (usac_data->tw_mdct[elem_idx] == 1) {
        pstr_core_coder->common_tw = ixheaacd_read_bits_buf(it_bit_buff, 1);

        if (pstr_core_coder->common_tw == 1) {
          usac_data->tw_data_present[left] = ixheaacd_read_bits_buf(it_bit_buff, 1);
          usac_data->tw_data_present[right] = usac_data->tw_data_present[left];
          if (usac_data->tw_data_present[left]) {
            for (k = 0; k < NUM_TW_NODES; k++) {
              usac_data->tw_ratio[left][k] = ixheaacd_read_bits_buf(it_bit_buff, 3);
              usac_data->tw_ratio[right][k] = usac_data->tw_ratio[left][k];
            }
          }
        }
      }

      if (pstr_core_coder->tns_active) {
        if (pstr_core_coder->common_window) {
          pstr_core_coder->common_tns = ixheaacd_read_bits_buf(it_bit_buff, 1);

        } else {
          pstr_core_coder->common_tns = 0;
        }

        pstr_core_coder->tns_on_lr = ixheaacd_read_bits_buf(it_bit_buff, 1);

        if (pstr_core_coder->common_tns) {
          ixheaacd_read_tns_u(usac_data->pstr_sfb_info[0], &usac_data->pstr_tns[left][0],
                              it_bit_buff);
          memcpy(&usac_data->pstr_tns[right][0], &usac_data->pstr_tns[left][0],
                 sizeof(ia_tns_frame_info_struct));

          pstr_core_coder->tns_data_present[0] = 2;
          pstr_core_coder->tns_data_present[1] = 2;
        } else {
          pstr_core_coder->tns_present_both = ixheaacd_read_bits_buf(it_bit_buff, 1);

          if (pstr_core_coder->tns_present_both) {
            pstr_core_coder->tns_data_present[0] = 1;
            pstr_core_coder->tns_data_present[1] = 1;
          } else {
            pstr_core_coder->tns_data_present[1] = ixheaacd_read_bits_buf(it_bit_buff, 1);
            pstr_core_coder->tns_data_present[0] = 1 - pstr_core_coder->tns_data_present[1];
          }
        }
      } else {
        pstr_core_coder->common_tns = 0;
        pstr_core_coder->tns_data_present[0] = 0;
        pstr_core_coder->tns_data_present[1] = 0;
      }

    } else {
      pstr_core_coder->common_window = 0;
      pstr_core_coder->common_tw = 0;
      left = chan_offset;
      right = chan_offset;
      if (nr_core_coder_channels == 2) right = chan_offset + 1;
    }

    for (ch = 0, chn = chan_offset; ch < nr_core_coder_channels; ch++, chn++) {
      if (pstr_core_coder->core_mode[chn] == CORE_MODE_LPD &&
          usac_data->td_frame_prev[chn] == CORE_MODE_FD && usac_data->ec_flag) {
        memcpy(usac_data->coef_fix[chn], usac_data->str_error_concealment[chn].spectral_coeff,
               sizeof(usac_data->str_error_concealment[chn].spectral_coeff));
        memcpy(usac_data->spec_scale[chn], usac_data->str_error_concealment[chn].q_spec_coeff,
               sizeof(usac_data->spec_scale[chn]));
        err_code = ixheaacd_fd_frm_dec(usac_data, chn);
        if (err_code == -1) return err_code;
        for (k = 0; k < usac_data->ccfl; k++) {
          usac_data->time_sample_vector[chn][k] = (FLOAT32)(
              (FLOAT32)usac_data->output_data_ptr[chn][k] * (FLOAT32)(ONE_BY_TWO_POW_15));
        }
        memcpy(usac_data->time_sample_vector_prev[chn], usac_data->time_sample_vector[chn],
               usac_data->ccfl * sizeof(usac_data->time_sample_vector_prev[chn][0]));

        usac_data->window_sequence[ch] = usac_data->str_error_concealment[ch].win_seq;
        usac_data->window_shape[ch] = usac_data->str_error_concealment[ch].win_shape;
        usac_data->window_shape_prev[ch] = usac_data->window_shape[ch];
        usac_data->window_sequence_last[ch] = usac_data->window_sequence[ch];
      }
      if (pstr_core_coder->core_mode[ch] == 1) {
        err_code = ixheaacd_tw_buff_update(usac_data, chn, usac_data->str_tddec[chn]);
        if (err_code == -1) return err_code;

        if (!usac_data->td_frame_prev[chn]) {
          ixheaacd_fix2flt_data(usac_data, usac_data->str_tddec[chn], chn);
        }

        for (k = 0; k < usac_data->ccfl; k++) {
          usac_data->time_sample_vector[chn][k] = (FLOAT32)(
              (FLOAT32)usac_data->output_data_ptr[chn][k] * (FLOAT32)(ONE_BY_TWO_POW_15));
        }
        usac_data->present_chan = chn;
        err_code = ixheaacd_lpd_channel_stream(usac_data, &td_frame, it_bit_buff,
                                               usac_data->time_sample_vector[chn]);
        if (err_code == -1) return err_code;
        if (usac_data->ec_flag) {
          it_bit_buff->xaac_jmp_buf = &local;
        }
        if (usac_data->ec_flag && usac_data->frame_ok) {
          memcpy(&usac_data->td_frame_data_prev[chn], &td_frame, sizeof(td_frame));
          usac_data->core_mode = CORE_MODE_LPD;
        }
        for (k = 0; k < usac_data->ccfl; k++) {
          usac_data->output_data_ptr[chn][k] =
              (WORD32)(usac_data->time_sample_vector[chn][k] * (FLOAT32)((WORD64)1 << 15));
        }

        usac_data->window_shape[chn] = WIN_SEL_0;

        ixheaacd_td_frm_dec(usac_data, chn, td_frame.mod[0]);

        usac_data->window_shape_prev[chn] = usac_data->window_shape[chn];
        usac_data->window_sequence_last[chn] = EIGHT_SHORT_SEQUENCE;

      } else {
        memset(usac_data->coef_fix[chn], 0, LN2 * sizeof(*usac_data->coef_fix[0]));

        if (usac_data->str_tddec[chn] && usac_data->td_frame_prev[chn]) {
          if (usac_data->ec_flag) {
            memcpy(usac_data->time_sample_vector[chn], usac_data->time_sample_vector_prev[chn],
                   usac_data->ccfl * sizeof(usac_data->time_sample_vector[chn][0]));
          }
          ixheaacd_lpd_dec_update(usac_data->str_tddec[chn], usac_data, chn);
        }

        if (id != ID_USAC_LFE) {
          if ((nr_core_coder_channels == 1) ||
              (pstr_core_coder->core_mode[0] != pstr_core_coder->core_mode[1]))
            pstr_core_coder->tns_data_present[ch] = ixheaacd_read_bits_buf(it_bit_buff, 1);
        }

        err_code = ixheaacd_fd_channel_stream(
            usac_data, pstr_core_coder, &pstr_core_coder->max_sfb[ch],
            usac_data->window_sequence_last[chn], chn, usac_data->noise_filling_config[elem_idx],
            ch, it_bit_buff);
        if (err_code == -1) return err_code;
      }
    }

    if (pstr_core_coder->core_mode[0] == CORE_MODE_FD &&
        pstr_core_coder->core_mode[1] == CORE_MODE_FD && nr_core_coder_channels == 2) {
      ixheaacd_cplx_prev_mdct_dmx(usac_data->pstr_sfb_info[left], usac_data->coef_save[left],
                                  usac_data->coef_save[right], usac_data->dmx_re_prev,
                                  pstr_core_coder->pred_dir);
    }

    if (pstr_core_coder->tns_on_lr == 0 && (id != ID_USAC_LFE)) {
      for (ch = 0, chn = left; chn <= right; ch++, chn++) {
        if (pstr_core_coder->core_mode[ch] == CORE_MODE_FD) {
          err_code = ixheaacd_tns_apply(usac_data, usac_data->coef_fix[chn],
                                        pstr_core_coder->max_sfb[ch],
                                        usac_data->pstr_sfb_info[chn], usac_data->pstr_tns[chn]);
          if (err_code) return err_code;
        }
      }
    }

    if (nr_core_coder_channels == 2 && pstr_core_coder->core_mode[0] == 0 &&
        pstr_core_coder->core_mode[1] == 0) {
      if (pstr_core_coder->ms_mask_present[0] == 3) {
        ixheaacd_cplx_pred_upmixing(usac_data, usac_data->coef_fix[left],
                                    usac_data->coef_fix[right], pstr_core_coder, left);

      } else if (pstr_core_coder->ms_mask_present[0] > 0) {
        ixheaacd_ms_stereo(usac_data, usac_data->coef_fix[right], usac_data->coef_fix[left], left,
                           pstr_core_coder->max_sfb[right] > pstr_core_coder->max_sfb[left]
                               ? pstr_core_coder->max_sfb[right]
                               : pstr_core_coder->max_sfb[left]);
      }

      if (pstr_core_coder->tns_on_lr) {
        for (ch = 0, chn = left; chn <= right; ch++, chn++) {
          if (pstr_core_coder->core_mode[ch] == CORE_MODE_FD) {
            err_code = ixheaacd_tns_apply(
                usac_data, usac_data->coef_fix[chn], pstr_core_coder->max_sfb[ch],
                usac_data->pstr_sfb_info[chn], usac_data->pstr_tns[chn]);
            if (err_code) return err_code;
          }
        }
      }

      ixheaacd_usac_cplx_save_prev(usac_data->pstr_sfb_info[left], usac_data->coef_fix[left],
                                   usac_data->coef_fix[right], usac_data->coef_save[left],
                                   usac_data->coef_save[right]);
    }
    if (usac_data->ec_flag) {
      for (chn = left; chn <= right; chn++) {
        if (pstr_core_coder->core_mode[chn] == CORE_MODE_FD &&
            usac_data->td_frame_prev[chn] == CORE_MODE_LPD) {
          memcpy(usac_data->str_error_concealment[chn].spectral_coeff, usac_data->coef_fix[chn],
                 sizeof(usac_data->str_error_concealment[chn].spectral_coeff));
          memcpy(usac_data->str_error_concealment[chn].q_spec_coeff, usac_data->spec_scale[chn],
                 sizeof(usac_data->spec_scale[chn]));
          usac_data->str_error_concealment[chn].win_seq = usac_data->window_sequence[chn];
          usac_data->str_error_concealment[chn].win_shape = usac_data->window_shape[chn];
          usac_data->str_error_concealment[chn].win_shape_prev =
              usac_data->window_shape_prev[chn];
          usac_data->str_error_concealment[chn].td_frame_prev = usac_data->td_frame_prev[chn];
          usac_data->str_error_concealment[chn].fac_data_present =
              usac_data->fac_data_present[chn];
        }
      }
      if (usac_data->frame_ok && usac_data->ec_flag) {
        memcpy(usac_data->max_sfb, pstr_core_coder->max_sfb, sizeof(pstr_core_coder->max_sfb));
      }
    }
  } else {
    left = chan_offset;
    right = chan_offset;
    if (nr_core_coder_channels == 2) right = chan_offset + 1;
    if (usac_data->ec_flag == 1) {
      WORD32 err = 0;
      usac_data->frame_ok = 0;
      for (ch = left; ch <= right; ch++) {
        if (usac_data->td_frame_prev[ch] == CORE_MODE_LPD) {
          usac_data->fac_data_present[ch] = 0;
          usac_data->str_error_concealment[ch].pstr_ec_scratch =
              (ia_ec_scratch_str *)&usac_data->str_error_concealment[ch].str_ec_scratch;
          usac_data->core_mode = usac_data->td_frame_prev[ch];
          usac_data->present_chan = ch;
          ixheaacd_usac_apply_ec(usac_data, NULL, ch);
          err = ixheaacd_lpd_dec(usac_data, usac_data->str_tddec[ch],
                                 &usac_data->td_frame_data_prev[ch],
                                 usac_data->time_sample_vector[ch], usac_data->first_lpd_flag, 0,
                                 usac_data->bpf_control_info);

          if (err) return err;

          for (k = 0; k < usac_data->ccfl; k++) {
            usac_data->output_data_ptr[ch][k] =
                (WORD32)(usac_data->time_sample_vector[ch][k] * (FLOAT32)((WORD64)1 << 15));
          }

          usac_data->window_shape[ch] = WIN_SEL_0;
          usac_data->window_shape_prev[ch] = usac_data->window_shape[ch];
          usac_data->window_sequence_last[ch] = EIGHT_SHORT_SEQUENCE;
        } else {
          pstr_core_coder->core_mode[ch] = CORE_MODE_FD;
        }
      }
    }
  }

  for (ch = left; ch <= right; ch++) {
    FLOAT32 *ptr_scratch =
        (FLOAT32 *)usac_data->str_error_concealment[ch].str_ec_scratch.spec_coeff;
    if ((pstr_core_coder->core_mode[ch] != CORE_MODE_LPD &&
         usac_data->td_frame_prev[ch] != CORE_MODE_LPD && usac_data->ec_flag) ||
        (pstr_core_coder->core_mode[ch] == CORE_MODE_FD && usac_data->ec_flag == 0)) {
      if (usac_data->tw_mdct[elem_idx]) {
        err_code = -1;
        return err_code;

      } else {
        if (usac_data->frame_ok == 0) {
          usac_data->fac_data_present[ch] = 0;
        }
        err_code = ixheaacd_fd_frm_dec(usac_data, ch);
        if (err_code == -1) return err_code;
        if (usac_data->ec_flag) {
          if (usac_data->str_error_concealment[ch].fade_idx < MAX_FADE_FRAMES) {
            FLOAT32 fade_fac = (FLOAT32)(ONE_BY_TWO_POW_15)*ia_ec_fade_factors
                [usac_data->str_error_concealment[ch].fade_idx];
            for (k = 0; k < usac_data->ccfl; k++) {
              usac_data->time_sample_vector[ch][k] =
                  (FLOAT32)((FLOAT32)usac_data->output_data_ptr[ch][k] * fade_fac);
            }
          } else {
            memset(&usac_data->time_sample_vector[ch][0], 0,
                   usac_data->ccfl * sizeof(usac_data->time_sample_vector[ch][0]));
          }
        } else {
          for (k = 0; k < usac_data->ccfl; k++) {
            usac_data->time_sample_vector[ch][k] =
                (FLOAT32)((FLOAT32)usac_data->output_data_ptr[ch][k] *
                          (FLOAT32)(ONE_BY_TWO_POW_15));
          }
        }
      }
      usac_data->window_shape_prev[ch] = usac_data->window_shape[ch];
      usac_data->window_sequence_last[ch] = usac_data->window_sequence[ch];
    } else {
      if (usac_data->ec_flag) {
        usac_data->str_error_concealment[ch].prev_frame_ok[0] =
            usac_data->str_error_concealment[ch].prev_frame_ok[1];
        usac_data->str_error_concealment[ch].prev_frame_ok[1] = usac_data->frame_ok;

        if (usac_data->str_error_concealment[ch].fade_idx < MAX_FADE_FRAMES) {
          FLOAT32 fade_fac =
              (FLOAT32)(ONE_BY_TWO_POW_15)*ia_ec_fade_factors[usac_data->str_error_concealment[ch]
                                                                  .fade_idx];
          for (k = 0; k < usac_data->ccfl; k++) {
            usac_data->time_sample_vector[ch][k] =
                (FLOAT32)((FLOAT32)usac_data->output_data_ptr[ch][k] * fade_fac);
          }
        } else {
          memset(&usac_data->time_sample_vector[ch][0], 0,
                 usac_data->ccfl * sizeof(usac_data->time_sample_vector[ch][0]));
        }

        memcpy(ptr_scratch, usac_data->time_sample_vector[ch],
               usac_data->ccfl * sizeof(ptr_scratch[0]));
        memcpy(usac_data->time_sample_vector[ch], usac_data->time_sample_vector_prev[ch],
               usac_data->ccfl * sizeof(usac_data->time_sample_vector[ch][0]));
        memcpy(usac_data->time_sample_vector_prev[ch], ptr_scratch,
               usac_data->ccfl * sizeof(ptr_scratch[0]));
      } else {
        for (k = 0; k < usac_data->ccfl; k++) {
          usac_data->time_sample_vector[ch][k] =
              (FLOAT32)((FLOAT32)usac_data->output_data_ptr[ch][k] *
                        (FLOAT32)(ONE_BY_TWO_POW_15));
        }
      }
    }
    if (usac_data->ec_flag) {
      usac_data->window_sequence[ch] = usac_data->str_error_concealment[ch].win_seq;
      usac_data->window_shape[ch] = usac_data->str_error_concealment[ch].win_shape;
      if (usac_data->first_frame == 0) {
        usac_data->window_shape_prev[ch] = usac_data->window_shape[ch];
        usac_data->window_sequence_last[ch] = usac_data->window_sequence[ch];
      }
    }
  }
  if (usac_data->ec_flag) {
    usac_data->first_frame = 0;
    if (usac_data->frame_ok == 1) {
      for (ch = 0, chn = left; chn <= right; chn++, ch++)
        usac_data->td_frame_prev[chn] = pstr_core_coder->core_mode[ch];
    }
  } else {
    for (ch = 0, chn = left; chn <= right; chn++, ch++)
      usac_data->td_frame_prev[chn] = pstr_core_coder->core_mode[ch];
  }

  return 0;
}
