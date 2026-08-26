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
#include <stdlib.h>
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
#include "iusace_config.h"
#include "iusace_arith_enc.h"
#include "iusace_block_switch_const.h"
#include "iusace_rom.h"

#define ARITH_ESCAPE (16)

static VOID iusace_arith_map_context(WORD32 pres_n, WORD32 prev_n, WORD32 *ptr_c_prev,
                                     WORD32 *ptr_c_pres, WORD32 arith_reset_flag) {
  WORD32 i, k;
  FLOAT32 ratio;
  WORD32 c_prev[516];
  WORD32 c_pres[516];

  if (arith_reset_flag) {
    memset(ptr_c_pres, 0, 516 * sizeof(WORD32));
    memset(ptr_c_prev, 0, 516 * sizeof(WORD32));
  } else {
    memcpy(&c_prev[2], &ptr_c_prev[2], (prev_n / 2 + 2) * sizeof(WORD32));
    memcpy(&c_pres[2], &ptr_c_pres[2], (prev_n / 2 + 2) * sizeof(WORD32));

    ratio = (FLOAT32)(prev_n) / (FLOAT32)(pres_n);
    for (i = 0; i < (pres_n / 2); i++) {
      k = (WORD32)((FLOAT32)(i)*ratio);
      ptr_c_pres[2 + i] = c_pres[2 + k];
      ptr_c_prev[2 + i] = c_prev[2 + k];
    }

    ptr_c_pres[(pres_n / 2) + 2] = c_pres[(prev_n / 2) + 2];
    ptr_c_pres[(pres_n / 2) + 3] = c_pres[(prev_n / 2) + 3];
    ptr_c_prev[(pres_n / 2) + 2] = c_prev[(prev_n / 2) + 2];
    ptr_c_prev[(pres_n / 2) + 3] = c_prev[(prev_n / 2) + 3];
  }
  return;
}

static WORD32 iusace_arith_get_state(WORD32 *c_pres, WORD32 *c_prev, WORD32 *s, WORD32 idx) {
  WORD32 s_tmp = *s;

  s_tmp = s_tmp >> 4;
  s_tmp = s_tmp + (c_prev[idx + 1] << 12);
  s_tmp = (s_tmp & 0xFFF0) + c_pres[idx - 1];

  *s = s_tmp;

  if (idx > 3) {
    if ((c_pres[idx - 1] + c_pres[idx - 2] + c_pres[idx - 3]) < 5) {
      return (s_tmp + 0x10000);
    }
  }

  return (s_tmp);
}

static UWORD16 iusace_arith_get_pk(WORD32 c) {
  WORD32 j;
  WORD32 i, i_min, i_max;

  i_min = -1;
  i_max = (sizeof(iusace_ari_lookup_m) / sizeof(iusace_ari_lookup_m[0])) - 1;
  while ((i_max - i_min) > 1) {
    i = i_min + ((i_max - i_min) / 2);
    j = iusace_ari_hash_m[i];
    if (c < j)
      i_max = i;
    else if (c > j)
      i_min = i;
    else
      return (iusace_ari_hash_m_lsb[i]);
  }

  return (iusace_ari_lookup_m[i_max]);
}

static VOID iusace_copy_bit_buf(ia_bit_buf_struct *it_bit_buff_dest,
                                ia_bit_buf_struct *it_bit_buff_src) {
  if (it_bit_buff_src != NULL && it_bit_buff_dest != NULL) {
    it_bit_buff_dest->cnt_bits = it_bit_buff_src->cnt_bits;
    it_bit_buff_dest->ptr_write_next = it_bit_buff_src->ptr_write_next;
    it_bit_buff_dest->write_position = it_bit_buff_src->write_position;
  }
  return;
}

static WORD32 iusace_arith_encode_level2(ia_bit_buf_struct *pstr_it_bit_buff, WORD32 bp,
                                         WORD32 *ptr_c_pres, WORD32 *ptr_c_prev, WORD32 *quant,
                                         WORD32 n, WORD32 nt, WORD32 use_stop) {
  WORD32 qs[32];
  iusace_state_arith as, as_stop;

  WORD32 a, b, a1, b1, m;
  WORD32 s, t, i, l, lev, esc_nb;
  UWORD16 pki;
  WORD32 bp_start = bp;
  WORD32 bp_stop = bp;
  WORD32 stop = 0;
  WORD32 sopt;
  WORD32 a2, b2;
  ia_bit_buf_struct it_bit_buff_temp;
  memset(&it_bit_buff_temp, 0, sizeof(it_bit_buff_temp));
  iusace_copy_bit_buf(&it_bit_buff_temp, pstr_it_bit_buff);

  as.low = 0;
  as.high = 65535;
  as.value = 0;

  sopt = ptr_c_prev[0] << 12;

  for (i = 0; i < n; i++) {
    if ((use_stop == 1 || use_stop == 2) && (stop == 0)) {
      WORD32 j;

      stop = 1;
      for (j = i; j < n; j++) {
        if (quant[2 * j] != 0 || quant[2 * j + 1] != 0) {
          stop = 0;
          break;
        }
      }

      if (stop) {
        s = iusace_arith_get_state(ptr_c_pres, ptr_c_prev, &sopt, i);
        t = s & 0xFFFFF;

        pki = iusace_arith_get_pk(t);

        if (use_stop == 1) {
          bp = iusace_arith_encode(pstr_it_bit_buff, bp, &as, ARITH_ESCAPE, iusace_ari_cf_m[pki]);
          pki = iusace_arith_get_pk(t + (1 << 17));
          bp = iusace_arith_encode(pstr_it_bit_buff, bp, &as, 0, iusace_ari_cf_m[pki]);

          break;
        } else {
          bp_stop = bp;
          as_stop.low = as.low;
          as_stop.high = as.high;
          as_stop.value = as.value;

          bp_stop =
              iusace_arith_encode(NULL, bp_stop, &as_stop, ARITH_ESCAPE, iusace_ari_cf_m[pki]);

          pki = iusace_arith_get_pk(t + (1 << 17));
          bp_stop = iusace_arith_encode(NULL, bp_stop, &as_stop, (0), iusace_ari_cf_m[pki]);
        }
      }
    }
    s = iusace_arith_get_state(ptr_c_pres, ptr_c_prev, &sopt, i);
    t = s & 0xFFFFF;

    a = quant[2 * i];
    b = quant[2 * i + 1];
    a1 = abs(a);
    b1 = abs(b);

    ptr_c_pres[i] = a1 + b1 + 1;
    if (ptr_c_pres[i] > 0xF) {
      ptr_c_pres[i] = 0xF;
    }

    lev = 0;
    esc_nb = 0;

    while ((a1) > 3 || (b1) > 3) {
      pki = iusace_arith_get_pk(t + (esc_nb << 17));

      bp = iusace_arith_encode(pstr_it_bit_buff, bp, &as, ARITH_ESCAPE, iusace_ari_cf_m[pki]);

      qs[lev++] = (a1 & 1) | ((b1 & 1) << 1);
      a1 >>= 1;
      b1 >>= 1;
      esc_nb++;

      if (esc_nb > 7) {
        esc_nb = 7;
      }
    }
    m = a1 + (b1 << 2);
    pki = iusace_arith_get_pk(t + (esc_nb << 17));
    bp = iusace_arith_encode(pstr_it_bit_buff, bp, &as, m, iusace_ari_cf_m[pki]);

    a2 = a1;
    b2 = b1;

    for (l = lev - 1; l >= 0; l--) {
      WORD32 lsbidx = (a2 == 0) ? 1 : ((b2 == 0) ? 0 : 2);
      bp = iusace_arith_encode(pstr_it_bit_buff, bp, &as, qs[l], iusace_ari_cf_r[lsbidx]);

      a2 = (a2 << 1) | (qs[l] & 1);
      b2 = (b2 << 1) | ((qs[l] >> 1) & 1);
    }
  }

  if (use_stop == 2) {
    bp = iusace_arith_done(pstr_it_bit_buff, bp, &as);
    if (stop) {
      bp_stop = iusace_arith_done(NULL, bp_stop, &as_stop);

      if (bp_stop < bp) {
        iusace_copy_bit_buf(pstr_it_bit_buff, &it_bit_buff_temp);
        bp = iusace_arith_encode_level2(pstr_it_bit_buff, bp_start, ptr_c_pres, ptr_c_prev, quant,
                                        n, nt, 1);
      } else {
        iusace_copy_bit_buf(pstr_it_bit_buff, &it_bit_buff_temp);
        bp = iusace_arith_encode_level2(pstr_it_bit_buff, bp_start, ptr_c_pres, ptr_c_prev, quant,
                                        n, nt, 0);
      }
    } else {
      iusace_copy_bit_buf(pstr_it_bit_buff, &it_bit_buff_temp);
      bp = iusace_arith_encode_level2(pstr_it_bit_buff, bp_start, ptr_c_pres, ptr_c_prev, quant,
                                      n, nt, 0);
    }
  } else {
    bp = iusace_arith_done(pstr_it_bit_buff, bp, &as);

    for (; i < nt; i++) {
      ptr_c_pres[i] = 1;
    }

    for (i = 0; i < n; i++) {
      if (quant[2 * i] != 0) {
        if (quant[2 * i] > 0) {
          iusace_write_bits_buf(pstr_it_bit_buff, 1, 1);
          bp++;
        } else {
          iusace_write_bits_buf(pstr_it_bit_buff, 0, 1);
          bp++;
        }
      }

      if (quant[2 * i + 1] != 0) {
        if (quant[2 * i + 1] > 0) {
          iusace_write_bits_buf(pstr_it_bit_buff, 1, 1);
          bp++;
        } else {
          iusace_write_bits_buf(pstr_it_bit_buff, 0, 1);
          bp++;
        }
      }
    }

    for (i = 0; i < nt; i++) {
      ptr_c_prev[i] = ptr_c_pres[i];
      ptr_c_pres[i] = 1;
    }
  }

  return bp;
}

WORD32 iusace_arith_enc_spec(ia_bit_buf_struct *it_bit_buf, WORD32 window_sequence,
                             WORD32 *ptr_x_ac_enc, WORD32 max_spec_coefficients,
                             WORD32 *ptr_c_pres, WORD32 *ptr_c_prev, WORD32 *ptr_size_prev,
                             WORD32 arith_reset_flag, WORD32 ccfl) {
  LOOPIDX i;
  WORD32 write_flag = (it_bit_buf != NULL);
  WORD32 size;
  WORD32 num_wins = (window_sequence == EIGHT_SHORT_SEQUENCE) ? MAX_SHORT_WINDOWS : 1;
  WORD32 bits_data_written = 0;

  switch (window_sequence) {
    case ONLY_LONG_SEQUENCE:
    case LONG_START_SEQUENCE:
    case STOP_START_SEQUENCE:
    case LONG_STOP_SEQUENCE:
      size = ccfl;
      break;
    case EIGHT_SHORT_SEQUENCE:
      size = ccfl >> 3;
      break;
    default:
      size = ccfl >> 3;
      break;
  }

  iusace_arith_map_context(size, *ptr_size_prev, ptr_c_pres, ptr_c_prev, arith_reset_flag);

  if (max_spec_coefficients > 0) {
    for (i = 0; i < num_wins; i++) {
      bits_data_written = iusace_arith_encode_level2(
          it_bit_buf, bits_data_written, ptr_c_pres + 2, ptr_c_prev + 2, &ptr_x_ac_enc[i * size],
          max_spec_coefficients / 2, size / 2, 2);
    }
  }

  if (write_flag) {
    *ptr_size_prev = size;
  }

  return bits_data_written;
}

WORD32 iusace_tcx_coding(ia_bit_buf_struct *pstr_it_bit_buff, WORD32 tcx_size,
                         WORD32 max_tcx_size, WORD32 *ptr_quant, WORD32 *c_pres, WORD32 *c_prev) {
  WORD32 bits_written = 0;

  iusace_arith_map_context(tcx_size, max_tcx_size, c_pres, c_prev, 0);

  bits_written =
      iusace_arith_encode_level2(pstr_it_bit_buff, bits_written, c_pres + 2, c_prev + 2,
                                 &ptr_quant[0], tcx_size / 2, tcx_size / 2, 2);

  iusace_arith_map_context(max_tcx_size, tcx_size, c_pres, c_prev, 0);

  return bits_written;
}

WORD32 iusace_arith_done(ia_bit_buf_struct *pstr_it_bit_buff, WORD32 bp, iusace_state_arith *s) {
  WORD32 low, high;
  WORD32 bits_to_follow;

  low = s->low;
  high = s->high;
  bits_to_follow = s->value + 1;

  if (low < 16384) {
    iusace_write_bits_buf(pstr_it_bit_buff, 0, 1);
    bp++;
    while (bits_to_follow) {
      iusace_write_bits_buf(pstr_it_bit_buff, 1, 1);
      bp++;
      bits_to_follow--;
    }
  } else {
    iusace_write_bits_buf(pstr_it_bit_buff, 1, 1);
    bp++;
    while (bits_to_follow) {
      iusace_write_bits_buf(pstr_it_bit_buff, 0, 1);
      bp++;
      bits_to_follow--;
    }
  }

  s->low = low;
  s->high = high;
  s->value = bits_to_follow;

  return bp;
}

WORD32 iusace_arith_encode(ia_bit_buf_struct *pstr_it_bit_buff, WORD32 bp, iusace_state_arith *s,
                           WORD32 symbol, UWORD16 const *cum_freq) {
  WORD32 low, high, range;
  WORD32 bits_to_follow;

  high = s->high;
  low = s->low;
  range = high - low + 1;

  if (symbol > 0) {
    high = low + ((range * cum_freq[symbol - 1]) >> 14) - 1;
  }

  low = low + ((range * cum_freq[symbol]) >> 14);

  bits_to_follow = s->value;

  for (;;) {
    if (high < 32768) {
      iusace_write_bits_buf(pstr_it_bit_buff, 0, 1);
      bp++;
      while (bits_to_follow) {
        iusace_write_bits_buf(pstr_it_bit_buff, 1, 1);
        bp++;
        bits_to_follow--;
      }
    } else if (low >= 32768) {
      iusace_write_bits_buf(pstr_it_bit_buff, 1, 1);
      bp++;
      while (bits_to_follow) {
        iusace_write_bits_buf(pstr_it_bit_buff, 0, 1);
        bp++;
        bits_to_follow--;
      }
      low -= 32768;
      high -= 32768;
    } else if (low >= 16384 && high < 49152) {
      bits_to_follow += 1;
      low -= 16384;
      high -= 16384;
    } else
      break;

    low += low;
    high += high + 1;
  }

  s->low = low;
  s->high = high;
  s->value = bits_to_follow;

  return bp;
}
