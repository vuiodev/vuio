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
#include "ixheaac_type_def.h"
#include "ixheaace_adjust_threshold_data.h"
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

#include "iusace_cnst.h"
#include "iusace_tns_usac.h"
#include "iusace_psy_mod.h"
#include "iusace_config.h"
#include "iusace_arith_enc.h"
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
#include "iusace_write_bitstream.h"
#include "iusace_func_prototypes.h"
#include "iusace_avq_enc.h"
#include "iusace_lpd_rom.h"

static WORD32 iusace_unary_code(WORD32 idx, WORD16 *ptr_bit_buf) {
  WORD32 num_bits;

  num_bits = 1;

  idx -= 1;
  while (idx-- > 0) {
    *ptr_bit_buf++ = 1;
    num_bits++;
  }

  *ptr_bit_buf = 0;

  return (num_bits);
}

static VOID iusace_get_nk_mode(WORD32 mode_lpc, ia_bit_buf_struct *pstr_it_bit_buff,
                               WORD32 *nk_mode, WORD32 lpc_set) {
  switch (lpc_set) {
    case 4:
      break;
    case 0:
    case 2:
      *nk_mode = 3;
      iusace_write_bits_buf(pstr_it_bit_buff, mode_lpc, 1);
      break;
    case 1:
      *nk_mode = mode_lpc;
      if (mode_lpc == 2) {
        iusace_write_bits_buf(pstr_it_bit_buff, 0, 1);
      } else if (mode_lpc == 1) {
        iusace_write_bits_buf(pstr_it_bit_buff, 1, 1);
        iusace_write_bits_buf(pstr_it_bit_buff, 1, 1);
      } else if (mode_lpc == 0) {
        iusace_write_bits_buf(pstr_it_bit_buff, 1, 1);
        iusace_write_bits_buf(pstr_it_bit_buff, 0, 1);
      }
      break;
    case 3:
      if (mode_lpc == 0) {
        *nk_mode = 0;
        iusace_write_bits_buf(pstr_it_bit_buff, 1, 1);
        iusace_write_bits_buf(pstr_it_bit_buff, 0, 1);
      } else if (mode_lpc == 1) {
        *nk_mode = 1;
        iusace_write_bits_buf(pstr_it_bit_buff, 0, 1);
      } else if (mode_lpc == 2) {
        *nk_mode = 2;
        iusace_write_bits_buf(pstr_it_bit_buff, 1, 1);
        iusace_write_bits_buf(pstr_it_bit_buff, 1, 1);
        iusace_write_bits_buf(pstr_it_bit_buff, 0, 1);
      } else {
        *nk_mode = 2;
        iusace_write_bits_buf(pstr_it_bit_buff, 1, 1);
        iusace_write_bits_buf(pstr_it_bit_buff, 1, 1);
        iusace_write_bits_buf(pstr_it_bit_buff, 1, 1);
      }
      break;
  }
  return;
}

static VOID iusace_write_qn_data(WORD32 *qn, ia_bit_buf_struct *pstr_it_bit_buff, WORD32 nk_mode,
                                 WORD32 num_frames) {
  WORD32 k, i;
  switch (nk_mode) {
    case 1:
      for (k = 0; k < 2; k++) {
        for (i = 0; i < qn[k] - 1; i++) {
          iusace_write_bits_buf(pstr_it_bit_buff, 1, 1);
        }
        iusace_write_bits_buf(pstr_it_bit_buff, 0, 1);
      }
      break;
    case 0:
    case 2:
    case 3:
      for (k = 0; k < 2; k++) {
        WORD32 qn1 = qn[k] - 2;
        if (qn1 < 0 || qn1 > 3) {
          qn1 = 3;
        }
        iusace_write_bits_buf(pstr_it_bit_buff, qn1, 2);
      }
      if ((nk_mode == 2) && num_frames != 2) {
        for (k = 0; k < 2; k++) {
          if (qn[k] > 4) {
            for (i = 0; i < qn[k] - 4; i++) {
              iusace_write_bits_buf(pstr_it_bit_buff, 1, 1);
            }
            iusace_write_bits_buf(pstr_it_bit_buff, 0, 1);
          }
          if (qn[k] == 0) {
            iusace_write_bits_buf(pstr_it_bit_buff, 0, 1);
          }
        }
      } else {
        for (k = 0; k < 2; k++) {
          if (qn[k] == 5) {
            iusace_write_bits_buf(pstr_it_bit_buff, 0, 1);
          } else if (qn[k] == 6) {
            iusace_write_bits_buf(pstr_it_bit_buff, 1, 1);
            iusace_write_bits_buf(pstr_it_bit_buff, 0, 1);
          } else if (qn[k] == 0) {
            iusace_write_bits_buf(pstr_it_bit_buff, 1, 1);
            iusace_write_bits_buf(pstr_it_bit_buff, 1, 1);
            iusace_write_bits_buf(pstr_it_bit_buff, 0, 1);
          } else {
            WORD32 qn_ext = qn[k] - 4;
            if (qn_ext > 0) {
              for (i = 0; i < qn_ext; i++) {
                iusace_write_bits_buf(pstr_it_bit_buff, 1, 1);
              }
              iusace_write_bits_buf(pstr_it_bit_buff, 0, 1);
            }
          }
        }
      }
      break;
  }
  return;
}

static VOID iusace_write_cb_indices(WORD32 *qn, WORD32 *ptr_params, WORD32 *idx,
                                    ia_bit_buf_struct *pstr_it_bit_buff, WORD32 nk_mode,
                                    WORD32 num_frames) {
  WORD32 k;
  WORD32 j = *idx;

  iusace_write_qn_data(qn, pstr_it_bit_buff, nk_mode, num_frames);

  for (k = 0; k < 2; k++) {
    if (qn[k] > 0) {
      WORD32 n, nk, i;
      if (qn[k] > 4) {
        nk = (qn[k] - 3) >> 1;
        n = qn[k] - nk * 2;
      } else {
        nk = 0;
        n = qn[k];
      }

      iusace_write_bits_buf(pstr_it_bit_buff, ptr_params[j++], (UWORD8)(4 * n));

      for (i = 0; i < 8; i++) {
        iusace_write_bits_buf(pstr_it_bit_buff, ptr_params[j++], (UWORD8)nk);
      }
    }
  }

  *idx = j;

  return;
}

static VOID iusace_write_lpc_data(ia_bit_buf_struct *pstr_it_bit_buff, WORD32 *param_lpc,
                                  WORD32 first_lpd_flag, WORD32 *mod, WORD32 num_frames) {
  WORD32 nk_mode = 0;
  WORD32 j = 0, k;
  WORD32 mode_lpc = 0;
  WORD32 qn[2] = {0};

  iusace_get_nk_mode(mode_lpc, pstr_it_bit_buff, &nk_mode, 4);

  iusace_write_bits_buf(pstr_it_bit_buff, param_lpc[j++], 8);

  for (k = 0; k < 2; k++) {
    qn[k] = param_lpc[j++];
  }

  iusace_write_cb_indices(qn, param_lpc, &j, pstr_it_bit_buff, nk_mode, num_frames);

  if (first_lpd_flag) {
    mode_lpc = param_lpc[j++];
    iusace_get_nk_mode(mode_lpc, pstr_it_bit_buff, &nk_mode, 0);

    if (mode_lpc == 0) {
      iusace_write_bits_buf(pstr_it_bit_buff, param_lpc[j++], 8);
    }

    for (k = 0; k < 2; k++) {
      qn[k] = param_lpc[j++];
    }

    iusace_write_cb_indices(qn, param_lpc, &j, pstr_it_bit_buff, nk_mode, num_frames);
  }

  mode_lpc = param_lpc[j++];

  if (num_frames == 4 && mod[0] < 3) {
    iusace_get_nk_mode(mode_lpc, pstr_it_bit_buff, &nk_mode, 2);

    if (mode_lpc == 0) {
      iusace_write_bits_buf(pstr_it_bit_buff, param_lpc[j++], 8);
    }

    for (k = 0; k < 2; k++) {
      qn[k] = param_lpc[j++];
    }

    iusace_write_cb_indices(qn, param_lpc, &j, pstr_it_bit_buff, nk_mode, num_frames);
  }
  mode_lpc = param_lpc[j++];
  if (mod[0] < 2) {
    iusace_get_nk_mode(mode_lpc, pstr_it_bit_buff, &nk_mode, 1);

    if (mode_lpc != 1) {
      if (mode_lpc == 0) {
        iusace_write_bits_buf(pstr_it_bit_buff, param_lpc[j++], 8);
      }

      for (k = 0; k < 2; k++) {
        qn[k] = param_lpc[j++];
      }

      iusace_write_cb_indices(qn, param_lpc, &j, pstr_it_bit_buff, nk_mode, num_frames);
    }
  } else if (mode_lpc != 1) {
    if (mode_lpc == 0) {
      j++;
    }
    for (k = 0; k < 2; k++) {
      qn[k] = param_lpc[j++];
    }
    j += ((qn[0] > 0) ? 9 : 0) + ((qn[1] > 0) ? 9 : 0);
  }

  mode_lpc = param_lpc[j++];
  if (num_frames != 2 && mod[2] < 2) {
    iusace_get_nk_mode(mode_lpc, pstr_it_bit_buff, &nk_mode, 3);
    if (mode_lpc == 0) {
      iusace_write_bits_buf(pstr_it_bit_buff, param_lpc[j++], 8);
    }

    for (k = 0; k < 2; k++) {
      qn[k] = param_lpc[j++];
    }
    iusace_write_cb_indices(qn, param_lpc, &j, pstr_it_bit_buff, nk_mode, num_frames);
  }
  return;
}

VOID iusace_encode_fac_params(WORD32 *mod, WORD32 *n_param_tcx, ia_usac_data_struct *usac_data,
                              WORD32 const usac_independency_flag,
                              ia_bit_buf_struct *pstr_it_bit_buff, WORD32 ch_idx) {
  WORD32 *total_nbbits = &usac_data->total_nbbits[ch_idx];
  ia_usac_td_encoder_struct *pstr_td = usac_data->td_encoder[ch_idx];
  WORD32 codec_mode = pstr_td->acelp_core_mode;
  WORD16 *bit_buf = usac_data->td_serial_out[ch_idx];
  WORD32 is_bass_post_filter = 1;
  WORD32 first_lpd_flag = (usac_data->core_mode_prev[ch_idx] == CORE_MODE_FD);
  WORD32 *param_lpc = usac_data->param_buf + (NUM_FRAMES * MAX_NUM_TCX_PRM_PER_DIV);
  WORD32 *param = usac_data->param_buf;
  WORD32 j, k, n, sfr, lpd_mode, num_bits, sq_bits, *prm;
  WORD16 first_tcx_flag = 1;
  WORD32 nbits_fac, nb_bits_lpc;
  WORD32 core_mode_last = (first_lpd_flag) ? 0 : 1;
  WORD32 fac_data_present;
  WORD32 num_frames = NUM_FRAMES;
  WORD16 *ptr_bit_buf = bit_buf;

  pstr_td->num_bits_per_supfrm = 0;
  *total_nbbits = 0;

  iusace_write_bits_buf(pstr_it_bit_buff, pstr_td->acelp_core_mode, 3);

  if (mod[0] == 3) {
    lpd_mode = 25;
  } else if ((mod[0] == 2) && (mod[2] == 2)) {
    lpd_mode = 24;
  } else {
    if (mod[0] == 2) {
      lpd_mode = 16 + mod[2] + 2 * mod[3];
    } else if (mod[2] == 2) {
      lpd_mode = 20 + mod[0] + 2 * mod[1];
    } else {
      lpd_mode = mod[0] + 2 * mod[1] + 4 * mod[2] + 8 * mod[3];
    }
  }
  iusace_write_bits_buf(pstr_it_bit_buff, lpd_mode, 5);
  pstr_td->num_bits_per_supfrm = 5;
  *total_nbbits += 5;

  iusace_write_bits_buf(pstr_it_bit_buff, is_bass_post_filter, 1);
  *total_nbbits += 1;

  iusace_write_bits_buf(pstr_it_bit_buff, core_mode_last, 1);
  *total_nbbits += 1;

  if (((mod[0] == 0) && (mod[-1] != 0)) || ((mod[0] > 0) && (mod[-1] == 0))) {
    fac_data_present = 1;
  } else {
    fac_data_present = 0;
  }

  iusace_write_bits_buf(pstr_it_bit_buff, fac_data_present, 1);
  *total_nbbits += 1;

  num_bits = (iusace_acelp_core_numbits_1024[codec_mode] / 4) - 2;

  k = 0;
  while (k < num_frames) {
    lpd_mode = mod[k];
    prm = param + (k * MAX_NUM_TCX_PRM_PER_DIV);
    j = 0;

    if (((mod[k - 1] == 0) && (mod[k] > 0)) || ((mod[k - 1] > 0) && (mod[k] == 0))) {
      nbits_fac = iusace_fd_encode_fac(&prm[j], ptr_bit_buf, (pstr_td->len_subfrm) / 2);
      j += (pstr_td->len_subfrm) / 2;
      *total_nbbits += nbits_fac;
      for (WORD32 i = 0; i < nbits_fac; i++) {
        iusace_write_bits_buf(pstr_it_bit_buff, ptr_bit_buf[i], 1);
      }
    }

    if (lpd_mode == 0) {
      iusace_write_bits_buf(pstr_it_bit_buff, prm[j++], 2);

      for (sfr = 0; sfr < (pstr_td->num_subfrm); sfr++) {
        n = 6;
        if ((sfr == 0) || (((pstr_td->len_subfrm) == 256) && (sfr == 2))) n = 9;
        iusace_write_bits_buf(pstr_it_bit_buff, prm[j], (UWORD8)n);
        j++;
        iusace_write_bits_buf(pstr_it_bit_buff, prm[j], 1);
        j++;
        if (codec_mode == ACELP_CORE_MODE_9k6) {
          iusace_write_bits_buf(pstr_it_bit_buff, prm[j], 5);
          j++;
          iusace_write_bits_buf(pstr_it_bit_buff, prm[j], 5);
          j++;
          iusace_write_bits_buf(pstr_it_bit_buff, prm[j], 5);
          j++;
          iusace_write_bits_buf(pstr_it_bit_buff, prm[j], 5);
          j++;
        } else if (codec_mode == ACELP_CORE_MODE_11k2) {
          iusace_write_bits_buf(pstr_it_bit_buff, prm[j], 9);
          j++;
          iusace_write_bits_buf(pstr_it_bit_buff, prm[j], 9);
          j++;
          iusace_write_bits_buf(pstr_it_bit_buff, prm[j], 5);
          j++;
          iusace_write_bits_buf(pstr_it_bit_buff, prm[j], 5);
          j++;
        } else if (codec_mode == ACELP_CORE_MODE_12k8) {
          iusace_write_bits_buf(pstr_it_bit_buff, prm[j], 9);
          j++;
          iusace_write_bits_buf(pstr_it_bit_buff, prm[j], 9);
          j++;
          iusace_write_bits_buf(pstr_it_bit_buff, prm[j], 9);
          j++;
          iusace_write_bits_buf(pstr_it_bit_buff, prm[j], 9);
          j++;
        } else if (codec_mode == ACELP_CORE_MODE_14k4) {
          iusace_write_bits_buf(pstr_it_bit_buff, prm[j], 13);
          j++;
          iusace_write_bits_buf(pstr_it_bit_buff, prm[j], 13);
          j++;
          iusace_write_bits_buf(pstr_it_bit_buff, prm[j], 9);
          j++;
          iusace_write_bits_buf(pstr_it_bit_buff, prm[j], 9);
          j++;
        } else if (codec_mode == ACELP_CORE_MODE_16k) {
          iusace_write_bits_buf(pstr_it_bit_buff, prm[j], 13);
          j++;
          iusace_write_bits_buf(pstr_it_bit_buff, prm[j], 13);
          j++;
          iusace_write_bits_buf(pstr_it_bit_buff, prm[j], 13);
          j++;
          iusace_write_bits_buf(pstr_it_bit_buff, prm[j], 13);
          j++;
        } else if (codec_mode == ACELP_CORE_MODE_18k4) {
          iusace_write_bits_buf(pstr_it_bit_buff, prm[j], 2);
          j++;
          iusace_write_bits_buf(pstr_it_bit_buff, prm[j], 2);
          j++;
          iusace_write_bits_buf(pstr_it_bit_buff, prm[j], 2);
          j++;
          iusace_write_bits_buf(pstr_it_bit_buff, prm[j], 2);
          j++;
          iusace_write_bits_buf(pstr_it_bit_buff, prm[j], 14);
          j++;
          iusace_write_bits_buf(pstr_it_bit_buff, prm[j], 14);
          j++;
          iusace_write_bits_buf(pstr_it_bit_buff, prm[j], 14);
          j++;
          iusace_write_bits_buf(pstr_it_bit_buff, prm[j], 14);
          j++;
        }
        iusace_write_bits_buf(pstr_it_bit_buff, prm[j], 7);
        j++;
      }
      *total_nbbits += (num_bits - NBITS_LPC);
      pstr_td->num_bits_per_supfrm += (num_bits - NBITS_LPC);
      k++;
    } else {
      iusace_write_bits_buf(pstr_it_bit_buff, prm[j++], 3);
      *total_nbbits += 3;
      pstr_td->num_bits_per_supfrm += 3;
      iusace_write_bits_buf(pstr_it_bit_buff, prm[j++], 7);
      *total_nbbits += 7;
      pstr_td->num_bits_per_supfrm += 7;

      if (first_tcx_flag) {
        first_tcx_flag = 0;
        if (usac_independency_flag) {
          pstr_td->arith_reset_flag = 1;
          memset(pstr_td->c_pres, 0, 516 * sizeof(WORD32));
          memset(pstr_td->c_prev, 0, 516 * sizeof(WORD32));
        } else {
          if (pstr_td->arith_reset_flag) {
            memset(pstr_td->c_pres, 0, 516 * sizeof(WORD32));
            memset(pstr_td->c_prev, 0, 516 * sizeof(WORD32));
          }
          iusace_write_bits_buf(pstr_it_bit_buff, pstr_td->arith_reset_flag, 1);
          *total_nbbits += 1;
          pstr_td->num_bits_per_supfrm += 1;
        }
      }

      sq_bits = iusace_tcx_coding(pstr_it_bit_buff, n_param_tcx[k], pstr_td->len_frame, prm + j,
                                  pstr_td->c_pres, pstr_td->c_prev);

      *total_nbbits += sq_bits;
      pstr_td->num_bits_per_supfrm += sq_bits;

      k += (1 << (lpd_mode - 1));
    }
  }

  nb_bits_lpc = pstr_it_bit_buff->cnt_bits;

  iusace_write_lpc_data(pstr_it_bit_buff, param_lpc, first_lpd_flag, mod, num_frames);

  nb_bits_lpc = pstr_it_bit_buff->cnt_bits - nb_bits_lpc;
  *total_nbbits += nb_bits_lpc;
  pstr_td->num_bits_per_supfrm += nb_bits_lpc;

  if ((core_mode_last == 0) && (fac_data_present == 1)) {
    WORD32 short_fac_flag = (mod[-1] == -2) ? 1 : 0;
    iusace_write_bits_buf(pstr_it_bit_buff, short_fac_flag, 1);
    *total_nbbits += 1;
  }

  return;
}

WORD32 iusace_fd_encode_fac(WORD32 *prm, WORD16 *ptr_bit_buf, WORD32 fac_length) {
  WORD32 i, j, n, nb, qn, kv[8], nk, fac_bits;
  WORD32 I;

  fac_bits = 0;

  for (i = 0; i < fac_length; i += 8) {
    iusace_apply_voronoi_ext(&prm[i], &qn, &I, kv);

    nb = iusace_unary_code(qn, ptr_bit_buf);
    ptr_bit_buf += nb;

    fac_bits += nb;

    nk = 0;
    n = qn;
    if (qn > 4) {
      nk = (qn - 3) >> 1;
      n = qn - nk * 2;
    }

    if (qn != 0) {
      iusace_write_bits2buf(I, 4 * n, ptr_bit_buf);
      ptr_bit_buf += 4 * n;
    }
    if (qn > 4) {
      for (j = 0; j < 8; j++) {
        iusace_write_bits2buf(kv[j], nk, ptr_bit_buf);
        ptr_bit_buf += nk;
      }
    }
    fac_bits += 4 * qn;
  }

  return fac_bits;
}
