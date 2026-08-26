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

#include <math.h>
#include <string.h>
#include "ixheaac_type_def.h"
#include "ixheaace_adjust_threshold_data.h"
#include "iusace_bitbuffer.h"
#include "ixheaace_mps_common_define.h"
/* DRC */
#include "impd_drc_common_enc.h"
#include "impd_drc_uni_drc.h"
#include "impd_drc_tables.h"
#include "impd_drc_api.h"
#include "impd_drc_uni_drc_eq.h"
#include "impd_drc_uni_drc_filter_bank.h"
#include "impd_drc_gain_enc.h"
#include "impd_drc_struct_def.h"

#include "iusace_cnst.h"
#include "iusace_tns_usac.h"
#include "iusace_psy_mod.h"
#include "iusace_ms.h"
#include "iusace_fd_qc_util.h"
#include "ixheaace_memory_standards.h"
#include "iusace_config.h"
#include "iusace_tcx_mdct.h"
#include "iusace_arith_enc.h"
#include "iusace_fd_quant.h"
#include "iusace_signal_classifier.h"
#include "iusace_block_switch_const.h"
#include "iusace_block_switch_struct_def.h"
#include "ixheaace_sbr_header.h"
#include "ixheaace_config.h"
#include "ixheaace_asc_write.h"
#include "iusace_main.h"
#include "iusace_func_prototypes.h"
#include "iusace_lpd_rom.h"
#include "iusace_lpd.h"
#include "iusace_avq_enc.h"
#include "ixheaac_error_standards.h"
#include "ixheaace_error_codes.h"

static VOID iusace_decode_fd_fac(WORD32 *ptr_fac_prms, WORD32 len_subfrm, WORD32 fac_len,
                                 FLOAT32 *ptr_lpc_coeffs, FLOAT32 *zir_sig, FLOAT32 *ptr_fac_dec,
                                 iusace_scratch_mem *pstr_scratch) {
  FLOAT32 *x = pstr_scratch->p_x;
  FLOAT32 *xn2 = pstr_scratch->p_xn_2;
  FLOAT32 fac_gain;
  WORD32 i;
  const FLOAT32 *sin_window;
  FLOAT32 *fac_window = pstr_scratch->p_fac_window;
  FLOAT32 ap[ORDER + 1];

  if (fac_len == 64) {
    sin_window = iusace_sin_window_128;
  } else {
    sin_window = iusace_sin_window_256;
  }

  if (ptr_lpc_coeffs != NULL && ptr_fac_dec != NULL) {
    fac_gain = (FLOAT32)pow(10.0f, ((FLOAT32)ptr_fac_prms[0]) / 28.0f);
    for (i = 0; i < fac_len; i++) {
      x[i] = (FLOAT32)ptr_fac_prms[i + 1] * fac_gain;
    }

    iusace_tcx_mdct(x, xn2, fac_len, pstr_scratch);

    iusace_get_weighted_lpc(ptr_lpc_coeffs, ap);

    memset(xn2 + fac_len, 0, fac_len * sizeof(FLOAT32));
    iusace_synthesis_tool_float(ap, xn2, ptr_fac_dec, 2 * fac_len, xn2 + fac_len,
                                pstr_scratch->p_buf_synthesis_tool);

    if (zir_sig != NULL) {
      for (i = 0; i < fac_len; i++) {
        fac_window[i] = sin_window[i] * sin_window[(2 * fac_len) - 1 - i];
        fac_window[fac_len + i] = 1.0f - (sin_window[fac_len + i] * sin_window[fac_len + i]);
      }
      for (i = 0; i < fac_len; i++) {
        ptr_fac_dec[i] += zir_sig[1 + (len_subfrm / 2) + i] * fac_window[fac_len + i] +
                          zir_sig[1 + (len_subfrm / 2) - 1 - i] * fac_window[fac_len - 1 - i];
      }
    }
  }

  return;
}

VOID iusace_fac_apply(FLOAT32 *orig, WORD32 len_subfrm, WORD32 fac_len, WORD32 low_pass_line,
                      WORD32 target_br, FLOAT32 *synth, FLOAT32 *ptr_lpc_coeffs,
                      WORD16 *fac_bits_word, WORD32 *num_fac_bits,
                      iusace_scratch_mem *pstr_scratch) {
  FLOAT32 *xn2 = pstr_scratch->p_xn2;
  FLOAT32 *fac_dec = pstr_scratch->p_fac_dec;
  FLOAT32 *right_fac_spec = pstr_scratch->p_right_fac_spec;
  FLOAT32 *x2 = pstr_scratch->p_x2;
  WORD32 *param = pstr_scratch->p_param;
  FLOAT32 ap[ORDER + 1];
  FLOAT32 fac_gain;
  WORD32 i, index;
  WORD32 num_enc_bits = 0;
  WORD32 start_right = 2 * len_subfrm - fac_len;

  *num_fac_bits = 0;

  memset(xn2, 0, (FAC_LENGTH + ORDER) * sizeof(FLOAT32));

  memcpy(xn2 + ORDER, &orig[start_right], fac_len * sizeof(FLOAT32));
  for (i = 0; i < fac_len; i++) {
    xn2[ORDER + i] -= synth[start_right + i];
  }

  iusace_get_weighted_lpc(ptr_lpc_coeffs, ap);
  iusace_compute_lp_residual(ap, xn2 + ORDER, x2, fac_len);
  for (i = 0; i < fac_len; i++) {
    x2[i] = x2[i] * (2.0f / (FLOAT32)fac_len);
  }

  iusace_tcx_mdct(x2, right_fac_spec, fac_len, pstr_scratch);

  memset(&right_fac_spec[low_pass_line], 0, (fac_len - low_pass_line) * sizeof(FLOAT32));

  fac_gain = iusace_calc_sq_gain(right_fac_spec, target_br, fac_len, pstr_scratch->p_sq_gain_en);
  index = (WORD32)floor(0.5f + (28.0f * (FLOAT32)log10(fac_gain)));
  if (index < 0) index = 0;
  if (index > 127) index = 127;
  param[0] = index;
  fac_gain = (FLOAT32)pow(10.0f, ((FLOAT32)index) / 28.0f);
  for (i = 0; i < fac_len; i++) right_fac_spec[i] /= fac_gain;

  for (i = 0; i < fac_len; i += 8) {
    iusace_find_nearest_neighbor(&right_fac_spec[i], &param[i + 1]);
  }

  iusace_write_bits2buf(index, 7, fac_bits_word);
  num_enc_bits += 7;
  num_enc_bits += iusace_fd_encode_fac(&param[1], &fac_bits_word[7], fac_len);
  iusace_decode_fd_fac(&param[0], len_subfrm, fac_len, ptr_lpc_coeffs, NULL, fac_dec,
                       pstr_scratch);
  *num_fac_bits = num_enc_bits;

  for (i = 0; i < fac_len; i++) {
    synth[start_right + i] += fac_dec[i];
  }
  return;
}

IA_ERRORCODE iusace_fd_fac(WORD32 *sfb_offsets, WORD32 sfb_active, FLOAT64 *orig_sig_dbl,
                           WORD32 window_sequence, FLOAT64 *synth_time,
                           ia_usac_td_encoder_struct *pstr_acelp, WORD32 last_subfr_was_acelp,
                           WORD32 next_frm_lpd, WORD16 *fac_prm_out, WORD32 *num_fac_bits,
                           iusace_scratch_mem *pstr_scratch) {
  const FLOAT32 *sin_window = NULL;
  LOOPIDX i;
  FLOAT32 *zir_sig = NULL;
  FLOAT32 *lpc_coeffs_q = NULL;
  WORD32 index;
  WORD32 low_pass_line = 0;
  WORD32 fac_len;
  FLOAT64 *left_fac_time_data = pstr_scratch->p_left_fac_time_data;
  FLOAT32 *left_fac_timedata_flt = pstr_scratch->p_left_fac_timedata_flt;
  FLOAT32 *left_fac_spec = pstr_scratch->p_left_fac_spec;
  FLOAT64 *fac_win = pstr_scratch->p_fac_win;
  WORD32 *fac_prm = pstr_scratch->p_fac_prm;
  WORD16 *fac_bits_word = pstr_scratch->p_fac_bits_word;
  FLOAT32 *acelp_folded = pstr_scratch->p_acelp_folded_scratch;

  *num_fac_bits = 0;

  if (last_subfr_was_acelp || next_frm_lpd)
  {
    if (window_sequence == EIGHT_SHORT_SEQUENCE)
      fac_len = (pstr_acelp->len_frame / 16);
    else
      fac_len = (pstr_acelp->len_frame / 8);

    low_pass_line = (WORD32)(sfb_offsets[sfb_active] * fac_len /
      (FLOAT32)pstr_acelp->len_frame);
  }

  if (last_subfr_was_acelp) {
    FLOAT32 *tmp_lp_res = pstr_scratch->ptr_tmp_lp_res;
    FLOAT32 lpc_coeffs[ORDER + 1];
    FLOAT32 ener, fac_gain;
    WORD32 left_start;

    switch (fac_len) {
      case 48:
        sin_window = iusace_sin_window_96;
        break;
      case 64:
        sin_window = iusace_sin_window_128;
        break;
      case 96:
        sin_window = iusace_sin_window_192;
        break;
      case 128:
        sin_window = iusace_sin_window_256;
        break;
      default:
        return IA_EXHEAACE_EXE_FATAL_USAC_INVALID_FAC_LEN;
    }

    for (i = 0; i < fac_len; i++) {
      fac_win[i] = sin_window[i] * sin_window[(2 * fac_len) - 1 - i];
      fac_win[fac_len + i] = 1.0f - (sin_window[fac_len + i] * sin_window[fac_len + i]);
    }

    left_start = (pstr_acelp->len_frame / 2) - fac_len - ORDER;

    for (i = 0; i < 2 * fac_len + ORDER; i++) {
      left_fac_time_data[i] = orig_sig_dbl[left_start + i];
    }

    for (i = 0; i < fac_len; i++) {
      left_fac_time_data[fac_len + ORDER + i] =
          left_fac_time_data[fac_len + ORDER + i] - synth_time[left_start + fac_len + ORDER + i];
    }

    zir_sig = pstr_acelp->lpd_state.tcx_quant;

    for (i = 0; i < ORDER; i++) {
      left_fac_time_data[fac_len + i] =
          left_fac_time_data[fac_len + i] - zir_sig[1 + 128 - ORDER + i];
    }

    for (i = 0; i < fac_len; i++) {
      acelp_folded[i] = zir_sig[1 + 128 + i] * (FLOAT32)fac_win[fac_len + i] +
                        zir_sig[1 + 128 - 1 - i] * (FLOAT32)fac_win[fac_len - 1 - i];
    }

    {
      FLOAT32 ener_tmp;
      ener = 0.0f;
      ener_tmp = 0.0f;

      for (i = 0; i < fac_len; i++) {
        ener += (FLOAT32)(left_fac_time_data[i + ORDER + fac_len] *
                          left_fac_time_data[i + ORDER + fac_len]);
      }
      ener *= 2.0f;

      for (i = 0; i < fac_len; i++) {
        ener_tmp += acelp_folded[i] * acelp_folded[i];
      }

      if (ener_tmp > ener)
        fac_gain = (FLOAT32)sqrt(ener / ener_tmp);
      else
        fac_gain = 1.0f;

      for (i = 0; i < fac_len; i++) {
        left_fac_time_data[i + ORDER + fac_len] -= fac_gain * acelp_folded[i];
      }
    }

    for (i = 0; i < 2 * fac_len + ORDER; i++) {
      left_fac_timedata_flt[i] = (FLOAT32)left_fac_time_data[i];
    }

    lpc_coeffs_q = pstr_acelp->lpd_state.lpc_coeffs_quant;
    lpc_coeffs_q += ORDER + 1;
    iusace_get_weighted_lpc(lpc_coeffs_q, lpc_coeffs);
    iusace_compute_lp_residual(lpc_coeffs, left_fac_timedata_flt + ORDER + fac_len, tmp_lp_res,
                               fac_len);
    FLOAT32 coeff = (2.0f / (FLOAT32)fac_len);
    for (i = 0; i < fac_len; i++) {
      tmp_lp_res[i] = tmp_lp_res[i] * coeff;
    }

    iusace_tcx_mdct(tmp_lp_res, left_fac_spec, fac_len, pstr_scratch);
    memset(&left_fac_spec[low_pass_line], 0, (fac_len - low_pass_line) * sizeof(FLOAT32));

    fac_gain = iusace_calc_sq_gain(left_fac_spec, 240, fac_len, pstr_scratch->p_sq_gain_en);

    index = (WORD32)floor(0.5f + (28.0f * (FLOAT32)log10(fac_gain)));
    if (index < 0) index = 0;
    if (index > 127) index = 127;
    iusace_write_bits2buf(index, 7, fac_bits_word);
    *num_fac_bits += 7;
    fac_gain = (FLOAT32)pow(10.0f, ((FLOAT32)index) / 28.0f);

    for (i = 0; i < fac_len; i++) {
      left_fac_spec[i] /= fac_gain;
    }

    for (i = 0; i < fac_len; i += 8) {
      iusace_find_nearest_neighbor(&left_fac_spec[i], &fac_prm[i]);
    }

    *num_fac_bits += iusace_fd_encode_fac(fac_prm, &fac_bits_word[7], fac_len);

    for (i = 0; i < (*num_fac_bits + 7) / 8; i++) {
      fac_prm_out[i] =
          (WORD16)((fac_bits_word[8 * i + 0] & 0x1) << 7 | (fac_bits_word[8 * i + 1] & 0x1) << 6 |
                   (fac_bits_word[8 * i + 2] & 0x1) << 5 | (fac_bits_word[8 * i + 3] & 0x1) << 4 |
                   (fac_bits_word[8 * i + 4] & 0x1) << 3 | (fac_bits_word[8 * i + 5] & 0x1) << 2 |
                   (fac_bits_word[8 * i + 6] & 0x1) << 1 | (fac_bits_word[8 * i + 7] & 0x1) << 0);
    }
  } else {
    *num_fac_bits = 0;
  }

  if (next_frm_lpd) {
    for (i = 0; i < 1024 / 2 + 1 + ORDER; i++) {
      pstr_acelp->fd_synth[i] = (FLOAT32)synth_time[pstr_acelp->len_frame - 1 + i - ORDER];
      pstr_acelp->fd_orig[i] = (FLOAT32)orig_sig_dbl[pstr_acelp->len_frame + i - ORDER];
    }

    pstr_acelp->low_pass_line = low_pass_line;
  }

  return IA_NO_ERROR;
}
