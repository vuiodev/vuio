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
#include "ixheaac_type_def.h"

#include "ixheaacd_acelp_com.h"
#include "ixheaacd_bitbuffer.h"

#include "ixheaacd_interface.h"
#include "ixheaacd_cnst.h"
#include "ixheaacd_info.h"
#include "ixheaacd_sbr_common.h"
#include "ixheaacd_drc_data_struct.h"
#include "ixheaacd_defines.h"
#include "ixheaacd_aac_rom.h"
#include "ixheaacd_pulsedata.h"
#include "ixheaacd_pns.h"
#include "ixheaac_constants.h"
#include "ixheaacd_sbrdecsettings.h"
#include "ixheaacd_sbrdecoder.h"
#include "ixheaacd_acelp_info.h"
#include "ixheaacd_tns_usac.h"
#include "ixheaacd_ec_defines.h"
#include "ixheaacd_ec_struct_def.h"
#include "ixheaacd_main.h"
#include "ixheaacd_channelinfo.h"
#include "ixheaacd_ec.h"
#include "ixheaacd_arith_dec.h"
#include "ixheaacd_func_def.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_error_standards.h"
#include "ixheaacd_error_codes.h"

extern const ia_usac_samp_rate_info ixheaacd_samp_rate_info[];

WORD32 ixheaacd_get_mode_lpc(WORD32 lpc_set, ia_bit_buf_struct *it_bit_buff, WORD32 *nk_mode) {
  WORD32 mode_lpc = 0;
  switch (lpc_set) {
    case 4:
      mode_lpc = 0;
      break;
    case 0:
    case 2:
      mode_lpc = ixheaacd_read_bits_buf(it_bit_buff, 1);
      if (mode_lpc == 1) *nk_mode = 3;
      break;
    case 1:
      if (ixheaacd_read_bits_buf(it_bit_buff, 1) == 0)
        mode_lpc = *nk_mode = 2;
      else {
        if (ixheaacd_read_bits_buf(it_bit_buff, 1) == 0)
          mode_lpc = *nk_mode = 0;
        else
          mode_lpc = *nk_mode = 1;
      }
      break;
    case 3:
      if (ixheaacd_read_bits_buf(it_bit_buff, 1) == 0)
        mode_lpc = *nk_mode = 1;
      else {
        if (ixheaacd_read_bits_buf(it_bit_buff, 1) == 0)
          mode_lpc = *nk_mode = 0;
        else {
          if (ixheaacd_read_bits_buf(it_bit_buff, 1) == 0)
            mode_lpc = 2;
          else
            mode_lpc = 3;
          *nk_mode = 2;
        }
      }
      break;
  }
  return mode_lpc;
}

VOID ixheaacd_qn_data(WORD32 nk_mode, WORD32 *qn, ia_bit_buf_struct *it_bit_buff) {
  WORD32 k;
  switch (nk_mode) {
    case 1:
      for (k = 0; k < 2; k++) {
        while (ixheaacd_read_bits_buf(it_bit_buff, 1) == 1) {
          qn[k] += 1;
        }
        if (qn[k] > 0) qn[k] += 1;
      }
      break;
    case 0:
    case 2:
    case 3:
      for (k = 0; k < 2; k++) qn[k] = 2 + ixheaacd_read_bits_buf(it_bit_buff, 2);

      if (nk_mode == 2) {
        for (k = 0; k < 2; k++) {
          if (qn[k] > 4) {
            qn[k] = 0;

            while (ixheaacd_read_bits_buf(it_bit_buff, 1) == 1) qn[k] += 1;

            if (qn[k] > 0) qn[k] += 4;
          }
        }
      } else {
        for (k = 0; k < 2; k++) {
          if (qn[k] > 4) {
            WORD32 qn_ext = 0;
            while (ixheaacd_read_bits_buf(it_bit_buff, 1) == 1) qn_ext += 1;

            switch (qn_ext) {
              case 0:
                qn[k] = 5;
                break;
              case 1:
                qn[k] = 6;
                break;
              case 2:
                qn[k] = 0;
                break;
              default:
                qn[k] = qn_ext + 4;
                break;
            }
          }
        }
      }
      break;
  }
  return;
}

VOID ixheaacd_code_book_indices(ia_td_frame_data_struct *pstr_td_frame_data, WORD32 nk_mode,
                                WORD32 *pos, ia_bit_buf_struct *it_bit_buff) {
  WORD32 k, qn[2] = {0, 0}, nk, n, i;

  ixheaacd_qn_data(nk_mode, &qn[0], it_bit_buff);

  pstr_td_frame_data->lpc_first_approx_idx[(*pos)++] = qn[0];
  pstr_td_frame_data->lpc_first_approx_idx[(*pos)++] = qn[1];

  for (k = 0; k < 2; k++) {
    if (qn[k] > 0) {
      if (qn[k] > 4) {
        nk = (qn[k] - 3) / 2;
        n = qn[k] - nk * 2;
      } else {
        nk = 0;
        n = qn[k];
      }
      pstr_td_frame_data->lpc_first_approx_idx[(*pos)++] =
          ixheaacd_read_bits_buf(it_bit_buff, 4 * n);

      for (i = 0; i < 8; i++)
        pstr_td_frame_data->lpc_first_approx_idx[(*pos)++] =
            ixheaacd_read_bits_buf(it_bit_buff, nk);
    }
  }
  return;
}

VOID ixheaacd_lpc_data(WORD32 first_lpd_flag, WORD32 mod[],
                       ia_td_frame_data_struct *pstr_td_frame_data,
                       ia_bit_buf_struct *it_bit_buff) {
  WORD32 mode_lpc, nk_mode = 0, j = 0;

  mode_lpc = ixheaacd_get_mode_lpc(4, it_bit_buff, &nk_mode);

  pstr_td_frame_data->lpc_first_approx_idx[j++] = ixheaacd_read_bits_buf(it_bit_buff, 8);

  ixheaacd_code_book_indices(pstr_td_frame_data, nk_mode, &j, it_bit_buff);
  if (first_lpd_flag) {
    mode_lpc = ixheaacd_get_mode_lpc(0, it_bit_buff, &nk_mode);
    pstr_td_frame_data->lpc_first_approx_idx[j++] = mode_lpc;

    if (mode_lpc == 0)
      pstr_td_frame_data->lpc_first_approx_idx[j++] = ixheaacd_read_bits_buf(it_bit_buff, 8);

    ixheaacd_code_book_indices(pstr_td_frame_data, nk_mode, &j, it_bit_buff);
  }
  if (mod[0] < 3) {
    mode_lpc = ixheaacd_get_mode_lpc(2, it_bit_buff, &nk_mode);
    pstr_td_frame_data->lpc_first_approx_idx[j++] = mode_lpc;

    if (mode_lpc == 0)
      pstr_td_frame_data->lpc_first_approx_idx[j++] = ixheaacd_read_bits_buf(it_bit_buff, 8);

    ixheaacd_code_book_indices(pstr_td_frame_data, nk_mode, &j, it_bit_buff);
  }
  if (mod[0] < 2) {
    mode_lpc = ixheaacd_get_mode_lpc(1, it_bit_buff, &nk_mode);
    pstr_td_frame_data->lpc_first_approx_idx[j++] = mode_lpc;

    if (mode_lpc == 0)
      pstr_td_frame_data->lpc_first_approx_idx[j++] = ixheaacd_read_bits_buf(it_bit_buff, 8);

    if (mode_lpc != 1) ixheaacd_code_book_indices(pstr_td_frame_data, nk_mode, &j, it_bit_buff);
  }
  if (mod[2] < 2) {
    mode_lpc = ixheaacd_get_mode_lpc(3, it_bit_buff, &nk_mode);
    pstr_td_frame_data->lpc_first_approx_idx[j++] = mode_lpc;

    if (mode_lpc == 0)
      pstr_td_frame_data->lpc_first_approx_idx[j++] = ixheaacd_read_bits_buf(it_bit_buff, 8);

    ixheaacd_code_book_indices(pstr_td_frame_data, nk_mode, &j, it_bit_buff);
  }
  return;
}

VOID ixheaacd_fac_decoding(WORD32 fac_length, WORD32 k, WORD32 *fac_prm,
                           ia_bit_buf_struct *it_bit_buff) {
  WORD32 i, j, n, qn, nk, kv[8];
  long code_book_index;

  for (i = 0; i < fac_length; i += 8) {
    qn = 0;
    while (ixheaacd_read_bits_buf(it_bit_buff, 1) == 1) {
      qn += 1;
    }
    if (qn != 0) qn += 1;

    nk = 0;
    n = qn;
    if (qn > 4) {
      nk = (qn - 3) >> 1;
      n = qn - nk * 2;
    }

    code_book_index = ixheaacd_read_bits_buf(it_bit_buff, 4 * n);

    for (j = 0; j < 8; j++) {
      kv[j] = ixheaacd_read_bits_buf(it_bit_buff, nk);
    }

    ixheaacd_rotated_gosset_mtx_dec(qn, code_book_index, kv, &fac_prm[k * FAC_LENGTH + i]);
  }
}

const UWORD8 ixheaacd_num_bites_celp_coding[8][4] = {
    {5, 5, 5, 5},     {9, 9, 5, 5},     {9, 9, 9, 9}, {13, 13, 9, 9},
    {13, 13, 13, 13}, {16, 16, 16, 16}, {1, 5, 1, 5}, {1, 5, 5, 5}};

VOID ixheaacd_acelp_decoding(WORD32 k, ia_usac_data_struct *usac_data,
                             ia_td_frame_data_struct *pstr_td_frame_data,
                             ia_bit_buf_struct *it_bit_buff, WORD32 chan) {
  WORD32 sfr, kk;
  WORD32 nb_subfr = usac_data->num_subfrm;
  const UWORD8 *ptr_num_bits =
      &ixheaacd_num_bites_celp_coding[pstr_td_frame_data->acelp_core_mode][0];

  chan = 0;
  pstr_td_frame_data->mean_energy[k] = ixheaacd_read_bits_buf(it_bit_buff, 2);

  for (sfr = 0; sfr < nb_subfr; sfr++) {
    kk = k * 4 + sfr;

    if ((sfr == 0) || ((nb_subfr == 4) && (sfr == 2)))
      pstr_td_frame_data->acb_index[kk] = ixheaacd_read_bits_buf(it_bit_buff, 9);

    else
      pstr_td_frame_data->acb_index[kk] = ixheaacd_read_bits_buf(it_bit_buff, 6);

    pstr_td_frame_data->ltp_filtering_flag[kk] = ixheaacd_read_bits_buf(it_bit_buff, 1);

    if (pstr_td_frame_data->acelp_core_mode == 5) {
      pstr_td_frame_data->icb_index[kk][0] =
          ixheaacd_read_bits_buf(it_bit_buff, 2);
      pstr_td_frame_data->icb_index[kk][1] =
          ixheaacd_read_bits_buf(it_bit_buff, 2);
      pstr_td_frame_data->icb_index[kk][2] =
          ixheaacd_read_bits_buf(it_bit_buff, 2);
      pstr_td_frame_data->icb_index[kk][3] =
          ixheaacd_read_bits_buf(it_bit_buff, 2);
      pstr_td_frame_data->icb_index[kk][4] =
          ixheaacd_read_bits_buf(it_bit_buff, 14);
      pstr_td_frame_data->icb_index[kk][5] =
          ixheaacd_read_bits_buf(it_bit_buff, 14);
      pstr_td_frame_data->icb_index[kk][6] =
          ixheaacd_read_bits_buf(it_bit_buff, 14);
      pstr_td_frame_data->icb_index[kk][7] =
          ixheaacd_read_bits_buf(it_bit_buff, 14);
    } else {
      pstr_td_frame_data->icb_index[kk][0] =
          ixheaacd_read_bits_buf(it_bit_buff, ptr_num_bits[0]);
      pstr_td_frame_data->icb_index[kk][1] =
          ixheaacd_read_bits_buf(it_bit_buff, ptr_num_bits[1]);
      pstr_td_frame_data->icb_index[kk][2] =
          ixheaacd_read_bits_buf(it_bit_buff, ptr_num_bits[2]);
      pstr_td_frame_data->icb_index[kk][3] =
          ixheaacd_read_bits_buf(it_bit_buff, ptr_num_bits[3]);
    }

    pstr_td_frame_data->gains[kk] = ixheaacd_read_bits_buf(it_bit_buff, 7);
  }
}

VOID ixheaacd_tcx_coding(ia_usac_data_struct *usac_data, pWORD32 quant, WORD32 k,
                         WORD32 first_tcx_flag, ia_td_frame_data_struct *pstr_td_frame_data,
                         ia_bit_buf_struct *it_bit_buff) {
  pstr_td_frame_data->noise_factor[k] = ixheaacd_read_bits_buf(it_bit_buff, 3);

  pstr_td_frame_data->global_gain[k] = ixheaacd_read_bits_buf(it_bit_buff, 7);

  switch (pstr_td_frame_data->mod[k]) {
    case 1:
      pstr_td_frame_data->tcx_lg[k] = usac_data->len_subfrm;
      break;
    case 2:
      pstr_td_frame_data->tcx_lg[k] = 2 * (usac_data->len_subfrm);
      break;
    case 3:
      pstr_td_frame_data->tcx_lg[k] = 4 * (usac_data->len_subfrm);
      break;
  }

  if (first_tcx_flag) {
    if (usac_data->usac_independency_flg) {
      pstr_td_frame_data->arith_reset_flag = 1;
    } else {
      pstr_td_frame_data->arith_reset_flag = ixheaacd_read_bits_buf(it_bit_buff, 1);
    }
  }

  ixheaacd_arith_data(pstr_td_frame_data, quant, usac_data, it_bit_buff, (first_tcx_flag), k);

  return;
}

WORD32 ixheaacd_lpd_channel_stream(ia_usac_data_struct *usac_data,
                                   ia_td_frame_data_struct *pstr_td_frame_data,
                                   ia_bit_buf_struct *it_bit_buff, FLOAT32 *synth) {
  WORD32 lpd_mode, k, cnt, ii;
  WORD32 first_tcx_flag;
  WORD32 *quant;
  WORD32 core_mode_last = 0, fac_data_present;
  WORD32 *fac_data;
  WORD32 first_lpd_flag = 0;
  WORD32 short_fac_flag;
  WORD32 bpf_control_info = 0;
  WORD32 chan = usac_data->present_chan;
  WORD32 last_lpd_mode = usac_data->str_tddec[chan]->mode_prev;
  WORD32 err = 0;
  short_fac_flag = 0;

  jmp_buf local;

  if (usac_data->ec_flag) {
    err = setjmp(local);
  }

  if (usac_data->sampling_rate_idx <= 5 && usac_data->ec_flag) {
    usac_data->frame_ok = 0;
  }

  if (err == 0 && usac_data->frame_ok == 1) {
    if (usac_data->ec_flag) {
      it_bit_buff->xaac_jmp_buf = &local;
    }
    pstr_td_frame_data->acelp_core_mode = ixheaacd_read_bits_buf(it_bit_buff, 3);

    lpd_mode = ixheaacd_read_bits_buf(it_bit_buff, 5);

    if ((lpd_mode > 25 || lpd_mode < 0)) {
      if (usac_data->ec_flag) {
        longjmp(*(it_bit_buff->xaac_jmp_buf),
                IA_XHEAAC_DEC_EXE_NONFATAL_INSUFFICIENT_INPUT_BYTES);
      } else {
        return IA_FATAL_ERROR;
      }
    }

    if (lpd_mode == 25) {
      pstr_td_frame_data->mod[0] = pstr_td_frame_data->mod[1] = pstr_td_frame_data->mod[2] =
          pstr_td_frame_data->mod[3] = 3;
    } else if (lpd_mode == 24) {
      pstr_td_frame_data->mod[0] = pstr_td_frame_data->mod[1] = pstr_td_frame_data->mod[2] =
          pstr_td_frame_data->mod[3] = 2;
    } else {
      if (lpd_mode >= 20) {
        pstr_td_frame_data->mod[0] = lpd_mode & 1;
        pstr_td_frame_data->mod[1] = (lpd_mode >> 1) & 1;
        pstr_td_frame_data->mod[2] = pstr_td_frame_data->mod[3] = 2;
      } else if (lpd_mode >= 16) {
        pstr_td_frame_data->mod[0] = pstr_td_frame_data->mod[1] = 2;
        pstr_td_frame_data->mod[2] = lpd_mode & 1;
        pstr_td_frame_data->mod[3] = (lpd_mode >> 1) & 1;
      } else {
        pstr_td_frame_data->mod[0] = lpd_mode & 1;
        pstr_td_frame_data->mod[1] = (lpd_mode >> 1) & 1;
        pstr_td_frame_data->mod[2] = (lpd_mode >> 2) & 1;
        pstr_td_frame_data->mod[3] = (lpd_mode >> 3) & 1;
      }
    }

    bpf_control_info = ixheaacd_read_bits_buf(it_bit_buff, 1);

    core_mode_last = ixheaacd_read_bits_buf(it_bit_buff, 1);

    fac_data_present = ixheaacd_read_bits_buf(it_bit_buff, 1);

    first_lpd_flag = (core_mode_last == 0) ? 1 : 0;

    quant = pstr_td_frame_data->x_tcx_invquant;
    first_tcx_flag = 1;
    k = 0;
    while (k < 4) {
      if (k == 0) {
        if ((core_mode_last == 1) && (fac_data_present == 1))
          ixheaacd_fac_decoding((usac_data->len_subfrm) / 2, k, pstr_td_frame_data->fac,
                                it_bit_buff);
      } else {
        if (((last_lpd_mode == 0) && (pstr_td_frame_data->mod[k] > 0)) ||
            ((last_lpd_mode > 0) && (pstr_td_frame_data->mod[k] == 0)))
          ixheaacd_fac_decoding((usac_data->len_subfrm) / 2, k, pstr_td_frame_data->fac,
                                it_bit_buff);
      }

      if (pstr_td_frame_data->mod[k] == 0) {
        ixheaacd_acelp_decoding(k, usac_data, pstr_td_frame_data, it_bit_buff, chan);
        last_lpd_mode = 0;
        pstr_td_frame_data->tcx_lg[k] = 0;
        k += 1;
      } else {
        ixheaacd_tcx_coding(usac_data, quant, k, first_tcx_flag, pstr_td_frame_data, it_bit_buff);
        last_lpd_mode = pstr_td_frame_data->mod[k];
        quant += pstr_td_frame_data->tcx_lg[k];

        cnt = 1 << (pstr_td_frame_data->mod[k] - 1);

        for (ii = 0; ii < cnt - 1; ii++) pstr_td_frame_data->tcx_lg[k + 1 + ii] = 0;

        k += cnt;
        first_tcx_flag = 0;
      }
    }

    ixheaacd_lpc_data(first_lpd_flag, pstr_td_frame_data->mod, pstr_td_frame_data, it_bit_buff);

    if ((core_mode_last == 0) && (fac_data_present == 1)) {
      WORD32 fac_length;
      short_fac_flag = ixheaacd_read_bits_buf(it_bit_buff, 1);

      fac_length = (short_fac_flag) ? ((usac_data->ccfl) / 16) : ((usac_data->ccfl) / 8);

      fac_data = pstr_td_frame_data->fac_data;
      fac_data[0] = ixheaacd_read_bits_buf(it_bit_buff, 7);
      if ((pstr_td_frame_data->fac_data[0] < 0) || (pstr_td_frame_data->fac_data[0] > 128)) {
        if (usac_data->ec_flag) {
          longjmp(*(it_bit_buff->xaac_jmp_buf),
                  IA_XHEAAC_DEC_EXE_NONFATAL_INSUFFICIENT_INPUT_BYTES);
        } else {
          return IA_FATAL_ERROR;
        }
      }
      ixheaacd_fac_decoding(fac_length, 0, &fac_data[1], it_bit_buff);
    }
    if (fac_data_present == 0) {
      memset(&pstr_td_frame_data->fac_data[0], 0, sizeof(pstr_td_frame_data->fac_data));
    }
    if (usac_data->ec_flag && usac_data->frame_ok) {
      usac_data->bpf_control_info = bpf_control_info;
      usac_data->core_mode_last = core_mode_last;
      usac_data->first_lpd_flag = first_lpd_flag;
    }
  } else {
    usac_data->frame_ok = 0;
  }

  if (usac_data->ec_flag) {
    usac_data->str_error_concealment[chan].pstr_ec_scratch =
        (ia_ec_scratch_str *)&usac_data->str_error_concealment[chan].str_ec_scratch;

    ixheaacd_usac_apply_ec(usac_data, &ixheaacd_samp_rate_info[0], chan);

    if (usac_data->frame_ok == 0) {
      bpf_control_info = usac_data->bpf_control_info;
      core_mode_last = usac_data->core_mode_last;
      first_lpd_flag = usac_data->first_lpd_flag;
    }
  }
  if (usac_data->frame_ok) {
    err = ixheaacd_lpd_dec(usac_data, usac_data->str_tddec[chan], pstr_td_frame_data, synth,
                           first_lpd_flag, short_fac_flag, bpf_control_info);
  } else {
    if (usac_data->ec_flag) {
      usac_data->fac_data_present[chan] = 0;
      err = ixheaacd_lpd_dec(usac_data, usac_data->str_tddec[chan],
                             &usac_data->td_frame_data_prev[chan], synth, first_lpd_flag,
                             short_fac_flag, bpf_control_info);
    }
  }

  return (err);
}

WORD32 ixheaacd_tw_buff_update(ia_usac_data_struct *usac_data, WORD32 i,
                               ia_usac_lpd_decoder_handle st) {
  WORD32 *p_ioverlap = usac_data->overlap_data_ptr[i];
  WORD32 td_frame_prev = usac_data->td_frame_prev[i];
  WORD32 window_sequence_last = usac_data->window_sequence_last[i];
  WORD32 tw_mdct = usac_data->tw_mdct[0];

  if (!td_frame_prev) {
    if (tw_mdct) {
      if (usac_data->ec_flag == 0) {
        return -1;
      } else {
        tw_mdct = 0;
      }
    } else {
      ixheaacd_reset_acelp_data_fix(usac_data, st, p_ioverlap,
                                    (window_sequence_last == EIGHT_SHORT_SEQUENCE), 0);
    }
  }
  return 0;
}

VOID ixheaacd_td_frm_dec(ia_usac_data_struct *usac_data, WORD32 k, WORD32 mod0) {
  WORD32 i;
  WORD32 lfac = 0;

  WORD32 *p_out_idata = usac_data->output_data_ptr[k];
  WORD32 *p_ioverlap = usac_data->overlap_data_ptr[k];
  WORD32 nlong = usac_data->ccfl;
  WORD32 window_sequence_last = usac_data->window_sequence_last[k];
  WORD32 td_frame_prev = usac_data->td_frame_prev[k];
  WORD32 tw_mdct = usac_data->tw_mdct[0];
  WORD32 nshort = nlong / 8;
  WORD32 *p_in_idata = p_out_idata;

  if (!td_frame_prev) {
    if (window_sequence_last == EIGHT_SHORT_SEQUENCE) {
      lfac = nshort / 2;
    } else {
      lfac = nshort;
    }
  }

  if (!td_frame_prev && (mod0 == 0)) {
    for (i = 0; i < (nlong / 2) - lfac - (LEN_SUBFR); i++) {
      p_in_idata[i] = 0;
    }
  } else if (!td_frame_prev && (mod0 > 0)) {
    for (i = 0; i < (nlong / 2) - lfac; i++) {
      p_in_idata[i] = 0;
    }
  }

  if (tw_mdct) {
    if (!td_frame_prev && (mod0 == 0)) {
      for (i = (nlong / 2) - lfac - (LEN_SUBFR); i < nlong / 2; i++) {
        p_ioverlap[i + (nlong / 2)] = 0;
      }
    }
    for (i = 0; i < nlong / 2; i++) {
      p_out_idata[i] = p_ioverlap[i] << 1;
      p_out_idata[i + nlong / 2] =
          ixheaac_add32_sat(p_ioverlap[i + nlong / 2] << 1, p_in_idata[i]);
      p_ioverlap[i] = ixheaac_add32_sat(p_in_idata[i + (nlong / 2)] >> 1, p_ioverlap[i + nlong]);
      p_ioverlap[i + (nlong / 2)] = 0;
      p_ioverlap[i + nlong] = 0;
      p_ioverlap[i + nlong + (nlong / 2)] = 0;
    }
  } else {
    if (!td_frame_prev && (mod0 == 0)) {
      for (i = (nlong / 2) - lfac - (LEN_SUBFR); i < nlong / 2; i++) {
        p_ioverlap[i] = 0;
      }
    } else if (!td_frame_prev) {
      for (i = (nlong / 2) - lfac; i < nlong; i++) {
        p_ioverlap[i] = 0;
      }
    }
    for (i = 0; i < nlong; i++) {
      p_out_idata[i] = ixheaac_add32_sat(p_ioverlap[i] << 1, p_in_idata[i]);
      p_ioverlap[i] = 0;
    }
  }
}
