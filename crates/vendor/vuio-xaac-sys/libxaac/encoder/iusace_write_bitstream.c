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
#include <math.h>
#include "ixheaac_type_def.h"
#include "iusace_bitbuffer.h"

/* DRC */
#include "impd_drc_common_enc.h"
#include "impd_drc_uni_drc.h"
#include "impd_drc_tables.h"
#include "impd_drc_api.h"
#include "impd_drc_uni_drc_eq.h"
#include "impd_drc_uni_drc_filter_bank.h"
#include "impd_drc_gain_enc.h"
#include "impd_drc_struct_def.h"

#include "ixheaace_mps_common_define.h"
#include "iusace_cnst.h"
#include "iusace_tns_usac.h"
#include "iusace_psy_mod.h"
#include "iusace_tns_usac.h"
#include "iusace_config.h"
#include "iusace_arith_enc.h"
#include "ixheaace_adjust_threshold_data.h"
#include "iusace_block_switch_const.h"
#include "iusace_block_switch_struct_def.h"
#include "iusace_fd_qc_util.h"
#include "iusace_fd_quant.h"
#include "iusace_ms.h"
#include "iusace_signal_classifier.h"
#include "ixheaace_sbr_header.h"
#include "ixheaace_config.h"
#include "ixheaace_asc_write.h"
#include "iusace_main.h"
#include "iusace_rom.h"

WORD32 iusace_write_scf_data(ia_bit_buf_struct *it_bit_buf, WORD32 max_sfb, WORD32 num_sfb,
                             const WORD32 *ptr_scale_factors, WORD32 num_win_grps,
                             WORD32 global_gain, const WORD32 huff_tab[CODE_BOOK_ALPHA_LAV][2]) {
  WORD32 write_flag = (it_bit_buf != NULL);
  WORD32 i, j, bit_count = 0;
  WORD32 diff, length, codeword;
  WORD32 index = 0;
  WORD32 previous_scale_factor = global_gain;

  for (j = 0; j < num_win_grps; j++) {
    for (i = 0; i < max_sfb; i++) {
      if (!((i == 0) && (j == 0))) {
        diff = ptr_scale_factors[index] - previous_scale_factor;
        length = huff_tab[diff + 60][0];
        bit_count += length;
        previous_scale_factor = ptr_scale_factors[index];
        if (write_flag == 1) {
          codeword = huff_tab[diff + 60][1];
          iusace_write_bits_buf(it_bit_buf, codeword, (UWORD8)length);
        }
      }
      index++;
    }
    for (; i < num_sfb; i++) {
      index++;
    }
  }

  return (bit_count);
}

WORD32 iusace_write_ms_data(ia_bit_buf_struct *it_bit_buf, WORD32 ms_mask,
                            WORD32 ms_used[MAX_SHORT_WINDOWS][MAX_SFB_LONG], WORD32 num_win_grps,
                            WORD32 nr_of_sfb) {
  WORD32 write_flag = (it_bit_buf != NULL);
  WORD32 bit_count = 0;
  WORD32 i, j;
  WORD32 ms_mask_write = ms_mask;

  if (write_flag) iusace_write_bits_buf(it_bit_buf, ms_mask_write, 2);
  bit_count += 2;

  if (ms_mask_write == 1) {
    for (i = 0; i < num_win_grps; i++) {
      for (j = 0; j < nr_of_sfb; j++) {
        if (write_flag) iusace_write_bits_buf(it_bit_buf, ms_used[i][j], 1);
        bit_count += 1;
      }
    }
  }

  return bit_count;
}

WORD32 iusace_write_tns_data(ia_bit_buf_struct *it_bit_buf, ia_tns_info *pstr_tns_info,
                             WORD32 window_sequence, WORD32 core_mode) {
  WORD32 write_flag = (it_bit_buf != NULL);
  WORD32 bit_count = 0;
  WORD32 num_windows;
  WORD32 len_tns_nfilt;
  WORD32 len_tns_length;
  WORD32 len_tns_order;
  WORD32 filt;
  WORD32 res_bits;
  UWORD32 coeff;
  WORD32 w;

  if (window_sequence == EIGHT_SHORT_SEQUENCE) {
    num_windows = MAX_SHORT_WINDOWS;
    len_tns_nfilt = 1;
    len_tns_length = 4;
    len_tns_order = 3;
  } else {
    num_windows = 1;
    len_tns_nfilt = 2;
    len_tns_length = 6;
    len_tns_order = 4;
  }
  if (core_mode == 1) {
    num_windows = 1;
  }

  for (w = 0; w < num_windows; w++) {
    ia_tns_window_data *ptr_win_data = &pstr_tns_info->window_data[w];
    WORD32 n_filt = ptr_win_data->n_filt;
    if (write_flag) {
      iusace_write_bits_buf(it_bit_buf, n_filt, (UWORD8)len_tns_nfilt);
    }
    bit_count += len_tns_nfilt;
    if (n_filt) {
      res_bits = ptr_win_data->coef_res;
      if (write_flag) {
        iusace_write_bits_buf(it_bit_buf, res_bits - DEF_TNS_RES_OFFSET, 1);
      }
      bit_count += 1;
      for (filt = 0; filt < n_filt; filt++) {
        ia_tns_filter_data *ptr_tns_filt = &ptr_win_data->tns_filter[filt];
        WORD32 order = ptr_tns_filt->order;
        if (write_flag) {
          iusace_write_bits_buf(it_bit_buf, ptr_tns_filt->length, (UWORD8)len_tns_length);
          iusace_write_bits_buf(it_bit_buf, order, (UWORD8)len_tns_order);
        }
        bit_count += (len_tns_length + len_tns_order);
        if (order) {
          WORD32 i;
          if (write_flag) {
            iusace_write_bits_buf(it_bit_buf, ptr_tns_filt->direction, 1);
            iusace_write_bits_buf(it_bit_buf, ptr_tns_filt->coef_compress, 1);
          }
          bit_count += 2;
          for (i = 1; i <= order; i++) {
            if (write_flag) {
              coeff = (UWORD32)(ptr_tns_filt->index[i]) & ((1 << res_bits) - 1);
              iusace_write_bits_buf(it_bit_buf, coeff, (UWORD8)res_bits);
            }
            bit_count += res_bits;
          }
        }
      }
    }
  }

  return bit_count;
}

static WORD32 iusace_calc_grouping_bits(const WORD32 *ptr_win_grp_len, WORD32 num_win_grps) {
  WORD32 grouping_bits = 0;
  WORD32 tmp[8] = {0};
  WORD32 i, j;
  WORD32 index = 0;

  for (i = 0; i < num_win_grps; i++) {
    for (j = 0; j < ptr_win_grp_len[i]; j++) {
      tmp[index++] = i;
    }
  }

  for (i = 1; i < 8; i++) {
    grouping_bits = grouping_bits << 1;
    if (tmp[i] == tmp[i - 1]) {
      grouping_bits++;
    }
  }

  return (grouping_bits);
}

WORD32 iusace_write_ics_info(ia_bit_buf_struct *it_bit_buf, ia_sfb_params_struct *pstr_sfb_prms,
                             WORD32 ch) {
  WORD32 write_flag = (it_bit_buf != NULL);
  WORD32 bit_count = 0;
  WORD32 win_seq = 0;
  WORD32 grouping_bits = 0;
  WORD32 max_sfb = pstr_sfb_prms->max_sfb[ch];
  WORD32 window_sequence = pstr_sfb_prms->window_sequence[ch];
  WORD32 window_shape = pstr_sfb_prms->window_shape[ch];
  WORD32 num_win_grps = pstr_sfb_prms->num_window_groups[ch];

  switch (window_sequence) {
    case EIGHT_SHORT_SEQUENCE:
      win_seq = 2;
      break;
    case ONLY_LONG_SEQUENCE:
      win_seq = 0;
      break;
    case LONG_START_SEQUENCE:
    case STOP_START_SEQUENCE:
      win_seq = 1;
      break;
    case LONG_STOP_SEQUENCE:
      win_seq = 3;
      break;
    default:
      win_seq = 3;
      break;
  }
  if (write_flag) iusace_write_bits_buf(it_bit_buf, win_seq, 2);
  bit_count += 2;

  if (write_flag) iusace_write_bits_buf(it_bit_buf, window_shape, 1);
  bit_count += 1;

  if (window_sequence == EIGHT_SHORT_SEQUENCE) {
    if (write_flag) iusace_write_bits_buf(it_bit_buf, max_sfb, 4);
    bit_count += 4;

    grouping_bits =
        iusace_calc_grouping_bits(pstr_sfb_prms->window_group_length[ch], num_win_grps);
    if (write_flag) iusace_write_bits_buf(it_bit_buf, grouping_bits, 7);
    bit_count += 7;
  } else {
    if (write_flag) iusace_write_bits_buf(it_bit_buf, max_sfb, 6);
    bit_count += 6;
  }

  return (bit_count);
}

WORD32 iusace_write_cplx_pred_data(ia_bit_buf_struct *it_bit_buf, WORD32 num_win_grps,
                                   WORD32 num_sfb, WORD32 complex_coef,
                                   WORD32 pred_coeffs_re[MAX_SHORT_WINDOWS][MAX_SFB_LONG],
                                   WORD32 pred_coeffs_im[MAX_SHORT_WINDOWS][MAX_SFB_LONG],
                                   const WORD32 huff_tab[CODE_BOOK_ALPHA_LAV][2],
                                   WORD32 const usac_independency_flg, WORD32 pred_dir,
                                   WORD32 cplx_pred_used[MAX_SHORT_WINDOWS][MAX_SFB_LONG],
                                   WORD32 cplx_pred_all, WORD32 *ptr_prev_alpha_coeff_re,
                                   WORD32 *ptr_prev_alpha_coeff_im, WORD32 *delta_code_time) {
  WORD32 write_flag = (it_bit_buf != NULL);
  WORD32 bit_count = 0;
  WORD32 i, j;
  WORD32 g;
  WORD32 sfb;
  const WORD32 sfb_per_pred_band = 2;
  WORD32 length_temp1_re[MAX_SHORT_WINDOWS][MAX_SFB_LONG],
      length_temp2_re[MAX_SHORT_WINDOWS][MAX_SFB_LONG],
      length_temp1_im[MAX_SHORT_WINDOWS][MAX_SFB_LONG],
      length_temp2_im[MAX_SHORT_WINDOWS][MAX_SFB_LONG];
  WORD32 code_word_temp1_re[MAX_SHORT_WINDOWS][MAX_SFB_LONG],
      code_word_temp2_re[MAX_SHORT_WINDOWS][MAX_SFB_LONG],
      code_word_temp1_im[MAX_SHORT_WINDOWS][MAX_SFB_LONG],
      code_word_temp2_im[MAX_SHORT_WINDOWS][MAX_SFB_LONG];
  WORD32 length_tot1 = 0, length_tot2 = 0;

  if (write_flag) iusace_write_bits_buf(it_bit_buf, cplx_pred_all, 1);
  bit_count += 1;

  if (cplx_pred_all == 0) {
    for (g = 0; g < num_win_grps; g++) {
      for (sfb = 0; sfb < num_sfb; sfb += sfb_per_pred_band) {
        iusace_write_bits_buf(it_bit_buf, cplx_pred_used[g][sfb], 1);
        bit_count += 1;
      }
    }
  }

  if (write_flag) iusace_write_bits_buf(it_bit_buf, pred_dir, 1);
  bit_count += 1;

  if (write_flag) iusace_write_bits_buf(it_bit_buf, complex_coef, 1);
  bit_count += 1;

  if (complex_coef) {
    if (!usac_independency_flg) {
      if (write_flag) iusace_write_bits_buf(it_bit_buf, 1, 1); /* use_prev_frame */
      bit_count += 1;
    }
  }

  if (usac_independency_flg) {
    *delta_code_time = 0;
  }

  /* Switching mechanism for delta_code_time */
  WORD32 prev_pred_coeff_re_temp1 = 0, prev_pred_coeff_re_temp2 = 0;
  WORD32 diff_pred_coeff_re_temp1 = 0, diff_pred_coeff_re_temp2 = 0;
  WORD32 prev_pred_coeff_im_temp1 = 0, prev_pred_coeff_im_temp2 = 0;
  WORD32 diff_pred_coeff_im_temp1 = 0, diff_pred_coeff_im_temp2 = 0;

  for (i = 0; i < num_win_grps; i++) {
    /* delta_code_time = 0*/
    prev_pred_coeff_re_temp1 = 0;
    if (complex_coef == 1) {
      prev_pred_coeff_im_temp1 = 0;
    }

    for (j = 0; j < num_sfb; j += 2) {
      if (!usac_independency_flg) {
        /* delta_code_time = 1*/
        if (i > 0) {
          prev_pred_coeff_re_temp2 = pred_coeffs_re[i - 1][j];
          if (complex_coef == 1) {
            prev_pred_coeff_im_temp2 = pred_coeffs_im[i - 1][j];
          }
        } else {
          prev_pred_coeff_re_temp2 = ptr_prev_alpha_coeff_re[j];
          if (complex_coef == 1) {
            prev_pred_coeff_im_temp2 = ptr_prev_alpha_coeff_im[j];
          }
        }
      }

      if (cplx_pred_used[i][j] == 1) {
        /*Differential Huffman coding of real prediction coefficients*/
        diff_pred_coeff_re_temp1 =
            pred_coeffs_re[i][j] - prev_pred_coeff_re_temp1; /* delta_code_time = 0 */
        prev_pred_coeff_re_temp1 = pred_coeffs_re[i][j];     /* delta_code_time = 0 */
        if (!usac_independency_flg) {
          diff_pred_coeff_re_temp2 =
              pred_coeffs_re[i][j] - prev_pred_coeff_re_temp2; /* delta_code_time = 1 */
        }

        /* delta_code_time = 0 */
        length_temp1_re[i][j] = huff_tab[diff_pred_coeff_re_temp1 + 60][0];
        code_word_temp1_re[i][j] = huff_tab[diff_pred_coeff_re_temp1 + 60][1];

        length_tot1 += length_temp1_re[i][j];

        if (!usac_independency_flg) {
          /*delta_code_time = 1 */
          length_temp2_re[i][j] = huff_tab[diff_pred_coeff_re_temp2 + 60][0];
          code_word_temp2_re[i][j] = huff_tab[diff_pred_coeff_re_temp2 + 60][1];

          length_tot2 += length_temp2_re[i][j];
        }

        if (complex_coef == 1) {
          /*Differential Huffman coding of imaginary prediction coefficients*/
          diff_pred_coeff_im_temp1 =
              pred_coeffs_im[i][j] - prev_pred_coeff_im_temp1; /* delta_code_time = 0 */
          prev_pred_coeff_im_temp1 = pred_coeffs_im[i][j];     /* delta_code_time = 0*/

          if (!usac_independency_flg) {
            diff_pred_coeff_im_temp2 =
                pred_coeffs_im[i][j] - prev_pred_coeff_im_temp2; /* delta_code_time = 1 */
          }

          /*delta_code_time = 0*/
          length_temp1_im[i][j] = huff_tab[diff_pred_coeff_im_temp1 + 60][0];
          code_word_temp1_im[i][j] = huff_tab[diff_pred_coeff_im_temp1 + 60][1];

          length_tot1 += length_temp1_im[i][j];

          if (!usac_independency_flg) {
            /*delta_code_time = 1*/
            length_temp2_im[i][j] = huff_tab[diff_pred_coeff_im_temp2 + 60][0];
            code_word_temp2_im[i][j] = huff_tab[diff_pred_coeff_im_temp2 + 60][1];

            length_tot2 += length_temp2_im[i][j];
          }
        }
      } else {
        pred_coeffs_re[i][j] = 0;
        /*delta_code_time = 0*/
        prev_pred_coeff_re_temp1 = pred_coeffs_re[i][j];
        if (complex_coef == 1) {
          pred_coeffs_im[i][j] = 0;
          /*delta_code_time = 0*/
          prev_pred_coeff_im_temp1 = pred_coeffs_im[i][j];
        }
      }

      ptr_prev_alpha_coeff_re[j] = pred_coeffs_re[i][j];
      if (complex_coef == 1) {
        ptr_prev_alpha_coeff_im[j] = pred_coeffs_im[i][j];
      }
    }

    for (j = num_sfb; j < MAX_SFB_LONG; j++) {
      pred_coeffs_re[i][j] = 0;
      ptr_prev_alpha_coeff_re[j] = 0;
      if (complex_coef == 1) {
        pred_coeffs_im[i][j] = 0;
        ptr_prev_alpha_coeff_im[j] = 0;
      }
    }
  }

  /*Make a decison on the value of delta_code_time per frame */
  if (!usac_independency_flg) {
    // Compare the code-word lengths
    if (length_tot1 <= length_tot2) {
      *delta_code_time = 0;
    } else {
      *delta_code_time = 1;
    }

    /* Write the value of delta_code_time to bitstream */
    if (write_flag) iusace_write_bits_buf(it_bit_buf, *delta_code_time, 1);
    bit_count += 1;
  }

  if (*delta_code_time == 0) {
    for (i = 0; i < num_win_grps; i++) {
      for (j = 0; j < num_sfb; j += 2) {
        if (cplx_pred_used[i][j] == 1) {
          if (write_flag)
            iusace_write_bits_buf(it_bit_buf, code_word_temp1_re[i][j],
                                  (UWORD8)length_temp1_re[i][j]);
          bit_count += length_temp1_re[i][j];

          if (complex_coef == 1) {
            if (write_flag)
              iusace_write_bits_buf(it_bit_buf, code_word_temp1_im[i][j],
                                    (UWORD8)length_temp1_im[i][j]);
            bit_count += length_temp1_im[i][j];
          }
        }
      }
    }
  } else {
    for (i = 0; i < num_win_grps; i++) {
      for (j = 0; j < num_sfb; j += 2) {
        if (cplx_pred_used[i][j] == 1) {
          if (write_flag)
            iusace_write_bits_buf(it_bit_buf, code_word_temp2_re[i][j],
                                  (UWORD8)length_temp2_re[i][j]);
          bit_count += length_temp2_re[i][j];

          if (complex_coef == 1) {
            if (write_flag)
              iusace_write_bits_buf(it_bit_buf, code_word_temp2_im[i][j],
                                    (UWORD8)length_temp2_im[i][j]);
            bit_count += length_temp2_im[i][j];
          }
        }
      }
    }
  }

  return bit_count;
}

WORD32 iusace_write_cpe(ia_sfb_params_struct *pstr_sfb_prms, ia_bit_buf_struct *it_bit_buf,
                        WORD32 *tns_data_present, WORD32 const usac_independency_flg,
                        ia_usac_encoder_config_struct *pstr_usac_config,
                        ia_usac_data_struct *pstr_usac_data, WORD32 ch) {
  WORD32 bit_count = 0;
  WORD32 ms_mask = pstr_usac_data->str_ms_info[ch].ms_mask;
  WORD32 common_max_sfb = 1;
  WORD32 tns_active = tns_data_present[0] || tns_data_present[1];
  ia_tns_info *pstr_tns_info = pstr_usac_data->pstr_tns_info[ch];
  (VOID) pstr_usac_config;

  iusace_write_bits_buf(it_bit_buf, tns_active, 1);
  bit_count += 1;

  iusace_write_bits_buf(it_bit_buf, pstr_sfb_prms->common_win[ch], 1);
  bit_count += 1;

  if (pstr_sfb_prms->max_sfb[ch] != pstr_sfb_prms->max_sfb[ch + 1]) {
    common_max_sfb = 0;
  }

  if (pstr_sfb_prms->common_win[ch]) {
    bit_count += iusace_write_ics_info(it_bit_buf, pstr_sfb_prms, ch);

    iusace_write_bits_buf(it_bit_buf, common_max_sfb, 1);
    bit_count += 1;

    if (common_max_sfb == 0) {
      if (pstr_sfb_prms->window_sequence[ch] != EIGHT_SHORT_SEQUENCE) {
        iusace_write_bits_buf(it_bit_buf, pstr_sfb_prms->max_sfb[ch + 1], 6);
        bit_count += 6;
      } else {
        iusace_write_bits_buf(it_bit_buf, pstr_sfb_prms->max_sfb[ch + 1], 4);
        bit_count += 4;
      }
    }

    pstr_sfb_prms->max_sfb_ste = MAX(pstr_sfb_prms->max_sfb[ch], pstr_sfb_prms->max_sfb[ch + 1]);

    bit_count +=
        iusace_write_ms_data(it_bit_buf, ms_mask, pstr_usac_data->str_ms_info[ch].ms_used,
                             pstr_sfb_prms->num_window_groups[ch], pstr_sfb_prms->max_sfb_ste);

    {
      if ((ms_mask == 3) && (pstr_usac_data->stereo_config_index == 0)) {
        bit_count += iusace_write_cplx_pred_data(
            it_bit_buf, pstr_sfb_prms->num_window_groups[ch], pstr_sfb_prms->max_sfb_ste,
            pstr_usac_data->complex_coef[ch], pstr_usac_data->pred_coef_re[ch],
            pstr_usac_data->pred_coef_im[ch], iusace_huffman_code_table, usac_independency_flg,
            pstr_usac_data->pred_dir_idx[ch], pstr_usac_data->cplx_pred_used[ch],
            pstr_usac_data->cplx_pred_all[ch], pstr_usac_data->pred_coef_re_prev[ch],
            pstr_usac_data->pred_coef_im_prev[ch], &pstr_usac_data->delta_code_time[ch]);
      }
    }
  }

  if (tns_active) {
    WORD32 common_tns = 0;
    WORD32 tns_on_lr = 1;
    WORD32 tns_present_both = tns_data_present[0] && tns_data_present[1];
    WORD32 tns_data_present1 = tns_data_present[1];

    if (pstr_sfb_prms->common_win[ch]) {
      iusace_write_bits_buf(it_bit_buf, common_tns, 1);
      bit_count += 1;
    }

    iusace_write_bits_buf(it_bit_buf, tns_on_lr, 1);
    bit_count += 1;

    if (common_tns) {
      bit_count +=
          iusace_write_tns_data(it_bit_buf, pstr_tns_info, pstr_sfb_prms->window_sequence[ch], 0);
    } else {
      iusace_write_bits_buf(it_bit_buf, tns_present_both, 1);
      bit_count += 1;

      if (!tns_present_both) {
        iusace_write_bits_buf(it_bit_buf, tns_data_present1, 1);
        bit_count += 1;
      }
    }
  }

  return (bit_count);
}

WORD32 iusace_write_fd_data(ia_bit_buf_struct *it_bit_buf, ia_sfb_params_struct *pstr_sfb_prms,
                            WORD32 num_fac_bits, WORD32 usac_independency_flg,
                            ia_usac_data_struct *pstr_usac_data,
                            ia_usac_encoder_config_struct *pstr_usac_config, WORD32 ch_idx,
                            WORD32 ele_id, WORD32 idx) {
  WORD32 bit_count = 0;
  WORD32 fac_data_present = (num_fac_bits > 0) ? 1 : 0;
  WORD16 *ptr_fac_data = pstr_usac_data->fac_out_stream[ch_idx];

  WORD32 is_noise_filling = pstr_usac_data->noise_filling[ele_id];
  WORD32 common_window = pstr_sfb_prms->common_win[ch_idx];
  ia_usac_quant_info_struct *pstr_quant_info = &(pstr_usac_data->str_quant_info[idx]);
  ia_tns_info *pstr_tns_info = pstr_usac_data->pstr_tns_info[ch_idx];
  WORD32 global_gain = pstr_usac_data->str_quant_info[idx].scale_factor[0];

  iusace_write_bits_buf(it_bit_buf, global_gain, 8);
  bit_count += 8;

  if (is_noise_filling) {
    iusace_write_bits_buf(it_bit_buf, pstr_usac_data->noise_level[idx], 3);

    iusace_write_bits_buf(it_bit_buf, pstr_usac_data->noise_offset[idx], 5);
    bit_count += 8;
  }

  if (!common_window) {
    bit_count += iusace_write_ics_info(it_bit_buf, pstr_sfb_prms, ch_idx);
  }

  bit_count += iusace_write_scf_data(
      it_bit_buf, pstr_sfb_prms->max_sfb[ch_idx], pstr_sfb_prms->num_sfb[ch_idx],
      pstr_quant_info->scale_factor, pstr_sfb_prms->num_window_groups[ch_idx], global_gain,
      iusace_huffman_code_table);

  if (pstr_tns_info != NULL && pstr_tns_info->tns_data_present == 1) {
    bit_count += iusace_write_tns_data(it_bit_buf, pstr_tns_info,
                                       pstr_sfb_prms->window_sequence[ch_idx], 0);
  }

  if (!usac_independency_flg) {
    iusace_write_bits_buf(it_bit_buf, pstr_quant_info->reset, 1);
    bit_count += 1;
  }

  if (pstr_quant_info->max_spec_coeffs == FRAME_LEN_SHORT_768) {
    pstr_quant_info->max_spec_coeffs = pstr_quant_info->max_spec_coeffs;
  }
  bit_count += iusace_arith_enc_spec(
      it_bit_buf, pstr_sfb_prms->window_sequence[ch_idx], pstr_quant_info->quant_degroup,
      pstr_quant_info->max_spec_coeffs, pstr_quant_info->c_pres, pstr_quant_info->c_prev,
      &(pstr_quant_info->arith_size_prev), usac_independency_flg || pstr_quant_info->reset,
      pstr_usac_config->ccfl);

  iusace_write_bits_buf(it_bit_buf, fac_data_present, 1);
  bit_count += 1;

  if (fac_data_present) {
    WORD32 i;
    for (i = 0; i < num_fac_bits; i += 8) {
      WORD32 bits_to_write = MIN(8, num_fac_bits - i);
      iusace_write_bits_buf(it_bit_buf, ptr_fac_data[i / 8] >> (8 - bits_to_write),
                            (UWORD8)bits_to_write);
    }
    bit_count += num_fac_bits;
  }

  return (bit_count);
}

WORD32 iusace_count_fd_bits(ia_sfb_params_struct *pstr_sfb_prms,
                            ia_usac_data_struct *pstr_usac_data, WORD32 usac_independency_flg,
                            ia_usac_encoder_config_struct *pstr_usac_config, WORD32 ch_idx,
                            WORD32 idx) {
  WORD32 bit_count = 0;
  ia_usac_quant_info_struct *pstr_quant_info = &pstr_usac_data->str_quant_info[idx];
  WORD32 window_sequence = pstr_sfb_prms->window_sequence[ch_idx];
  WORD32 global_gain = pstr_quant_info->scale_factor[0];
  WORD32 max_sfb = pstr_sfb_prms->max_sfb[ch_idx];
  WORD32 num_sfb = pstr_sfb_prms->num_sfb[ch_idx];
  WORD32 num_win_grps = pstr_sfb_prms->num_window_groups[ch_idx];

  bit_count += iusace_write_scf_data(NULL, max_sfb, num_sfb, pstr_quant_info->scale_factor,
                                     num_win_grps, global_gain, iusace_huffman_code_table);

  WORD32 temp_c_pres[516], temp_c_prev[516], temp_size = pstr_quant_info->arith_size_prev;
  memcpy(temp_c_pres, pstr_quant_info->c_pres, 516 * sizeof(pstr_quant_info->c_pres[0]));
  memcpy(temp_c_prev, pstr_quant_info->c_prev, 516 * sizeof(pstr_quant_info->c_prev[0]));
  bit_count += iusace_arith_enc_spec(
      NULL, window_sequence, pstr_quant_info->quant_degroup, pstr_quant_info->max_spec_coeffs,
      temp_c_pres, temp_c_prev, &(temp_size), usac_independency_flg || pstr_quant_info->reset,
      pstr_usac_config->ccfl);

  return (bit_count);
}

WORD32 iusace_write_fill_ele(ia_bit_buf_struct *it_bit_buf, WORD32 num_bits) {
  WORD32 write_flag = (it_bit_buf != NULL);
  WORD32 bit_count = 0;

  if (num_bits <= 8) {
    if (write_flag) {
      iusace_write_bits_buf(it_bit_buf, 0, 1);
    }
    bit_count++;
    num_bits--;
  } else {
    if (write_flag) {
      iusace_write_bits_buf(it_bit_buf, 1, 1);
    }
    bit_count++;
    num_bits--;

    if (num_bits <= 8) {
      if (write_flag) {
        iusace_write_bits_buf(it_bit_buf, 1, 1);
      }
      bit_count++;
      num_bits--;
    } else {
      WORD32 bytes_to_write = 0;
      if (write_flag) {
        iusace_write_bits_buf(it_bit_buf, 0, 1);
      }
      bit_count++;
      num_bits--;
      bytes_to_write = num_bits >> 3;

      if (bytes_to_write > 255) {
        bytes_to_write -= 3;
        if (write_flag) {
          iusace_write_bits_buf(it_bit_buf, 255, 8);
        }
        if (write_flag) {
          iusace_write_bits_buf(it_bit_buf, bytes_to_write - 253, 16);
        }
        bit_count += 24;
        num_bits -= 24;
      } else {
        bytes_to_write--;
        if (write_flag) {
          iusace_write_bits_buf(it_bit_buf, bytes_to_write, 8);
        }
        bit_count += 8;
        num_bits -= 8;
      }

      while (bytes_to_write > 0) {
        if (write_flag) {
          iusace_write_bits_buf(it_bit_buf, 0xA9, 8);
        }
        bit_count += 8;
        num_bits -= 8;
        bytes_to_write--;
      }
    }
  }
  return bit_count;
}
