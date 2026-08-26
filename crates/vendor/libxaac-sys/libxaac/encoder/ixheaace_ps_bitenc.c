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

#include "ixheaac_type_def.h"

#include "ixheaace_sbr_header.h"
#include "ixheaace_sbr_def.h"
#include "ixheaace_resampler.h"
#include "ixheaace_sbr_rom.h"
#include "ixheaace_common_rom.h"
#include "ixheaace_bitbuffer.h"
#include "ixheaace_sbr_main.h"

#include "ixheaace_sbr_hybrid.h"
#include "ixheaace_sbr_ps_enc.h"
#include "ixheaace_sbr_ps_bitenc.h"
#include "ixheaace_env_bit.h"

static WORD32 iexheaax_append_bitstream(ixheaace_bit_buf_handle hdl_bitbuf_write,
                                        ixheaace_bit_buf_handle hdl_bitbuf_read,
                                        UWORD8 num_bits) {
  WORD32 idx;
  UWORD32 value;

  if (num_bits > 16) {
    WORD32 cnt;
    UWORD8 rem;
    cnt = num_bits >> 4;
    rem = num_bits % 16;

    for (idx = 0; idx < cnt; idx++) {
      value = ixheaace_readbits(hdl_bitbuf_read, 16);
      ixheaace_write_bits(hdl_bitbuf_write, value, 16);
    }
    if (rem) {
      value = ixheaace_readbits(hdl_bitbuf_read, rem);
      ixheaace_write_bits(hdl_bitbuf_write, value, rem);
    }
  } else {
    value = ixheaace_readbits(hdl_bitbuf_read, num_bits);
    ixheaace_write_bits(hdl_bitbuf_write, value, num_bits);
  }

  return num_bits;
}

WORD32
ixheaace_enc_write_ps_data(ixheaace_pstr_ps_enc pstr_ps_handle, WORD32 b_header_active,
                           ixheaace_str_ps_tab *ps_tables) {
  WORD32 tmp_var, gr;
  const WORD32 *aa_huff_book_iid_c;
  const WORD16 *aa_huff_book_icc_c;
  const WORD8 *aa_huff_book_iid_l;
  const WORD8 *aa_huff_book_icc_l;
  WORD32 *aa_delta_iid;
  WORD32 *aa_delta_icc;

  WORD8 index, last_index;
  WORD32 no_bits_f = 0;
  WORD32 no_bits_t = 0;

  WORD32 aa_delta_iid_t[NUMBER_OF_IID_BINS] = {0};
  WORD32 aa_delta_icc_t[NUMBER_OF_ICC_BINS] = {0};

  WORD32 aa_delta_iid_f[NUMBER_OF_IID_BINS] = {0};
  WORD32 aa_delta_icc_f[NUMBER_OF_ICC_BINS] = {0};

  WORD32 ab_dt_flag_iid;
  WORD32 ab_dt_flag_icc;

  WORD32 b_send_header;

  UWORD32 b_zero_iid = 1;
  UWORD32 b_zero_icc = 1;
  UWORD32 b_keep_params = 1;

  ixheaace_bit_buf_handle bb = &pstr_ps_handle->ps_bit_buf;

  tmp_var = ia_enhaacplus_enc_get_bits_available(bb);

  /* bit buffer shall be empty */
  if (tmp_var != 0) {
    return -1;
  }

  if (b_header_active) {
    b_keep_params = 0;
  }

  last_index = 0;

  for (gr = 0; gr < pstr_ps_handle->iid_icc_bins; gr++) {
    WORD IID_flag;
    FLOAT32 pan_value = pstr_ps_handle->aaa_IID_data_buf[gr][SYSTEMLOOKAHEAD];
    IID_flag = 0;
    if ((pan_value >= -ps_tables->pan_class[0]) && (pan_value <= ps_tables->pan_class[0])) {
      IID_flag = 1;
    }

    if (IID_flag) {
      index = 0;
    } else {
      if (pan_value < 0) {
        for (index = NO_IID_STEPS - 1; pan_value > -ps_tables->pan_class[index]; index--) {
        }
        index = -index - 1;
      } else {
        for (index = NO_IID_STEPS - 1; pan_value < ps_tables->pan_class[index]; index--) {
        }
        index++;
      }

      b_zero_iid = 0;
    }

    if (gr == 0) {
      aa_delta_iid_f[gr] = index;
      no_bits_t = 0;

      no_bits_f = ps_tables->a_book_ps_iid_freq_length[index + CODE_BCK_LAV_IID];
    } else {
      aa_delta_iid_f[gr] = index - last_index;

      no_bits_f += ps_tables->a_book_ps_iid_freq_length[aa_delta_iid_f[gr] + CODE_BCK_LAV_IID];
    }

    last_index = index;

    aa_delta_iid_t[gr] = index - pstr_ps_handle->a_last_iid_index[gr];

    pstr_ps_handle->a_last_iid_index[gr] = index;

    no_bits_t += ps_tables->a_book_ps_iid_time_length[aa_delta_iid_t[gr] + CODE_BCK_LAV_IID];

    if (aa_delta_iid_t[gr] != 0) {
      b_keep_params = 0;
    }
  }

  if (no_bits_t < no_bits_f && !b_header_active) {
    ab_dt_flag_iid = 1;
    aa_delta_iid = aa_delta_iid_t;
    aa_huff_book_iid_c = ps_tables->a_book_ps_iid_time_code;
    aa_huff_book_iid_l = ps_tables->a_book_ps_iid_time_length;
  } else {
    ab_dt_flag_iid = 0;
    aa_delta_iid = aa_delta_iid_f;
    aa_huff_book_iid_c = ps_tables->a_book_ps_iid_freq_code;
    aa_huff_book_iid_l = ps_tables->a_book_ps_iid_freq_length;
  }

  last_index = 0;

  for (gr = 0; gr < pstr_ps_handle->iid_icc_bins; gr++) {
    WORD ICC_flag;
    FLOAT32 sa_value = pstr_ps_handle->aaa_ICC_data_buf[gr][SYSTEMLOOKAHEAD];
    ICC_flag = 0;
    if (sa_value <= ps_tables->sa_class[0]) {
      ICC_flag = 1;
    }
    if (ICC_flag) {
      index = 0;
    } else {
      for (index = NO_ICC_STEPS - 2; sa_value < ps_tables->sa_class[index]; index--) {
      }
      index++;

      b_zero_icc = 0;
    }

    if (gr == 0) {
      aa_delta_icc_f[gr] = index;

      no_bits_f = ps_tables->a_book_ps_icc_freq_length[index + CODE_BCK_LAV_ICC];

      no_bits_t = 0;
    } else {
      aa_delta_icc_f[gr] = index - last_index;

      no_bits_f += ps_tables->a_book_ps_icc_freq_length[aa_delta_icc_f[gr] + CODE_BCK_LAV_ICC];
    }

    last_index = index;

    aa_delta_icc_t[gr] = index - pstr_ps_handle->a_last_icc_index[gr];

    pstr_ps_handle->a_last_icc_index[gr] = index;

    no_bits_t += ps_tables->a_book_ps_icc_time_length[aa_delta_icc_t[gr] + CODE_BCK_LAV_ICC];

    if (aa_delta_icc_t[gr] != 0) {
      b_keep_params = 0;
    }
  }

  if (no_bits_t < no_bits_f && !b_header_active) {
    ab_dt_flag_icc = 1;
    aa_delta_icc = aa_delta_icc_t;
    aa_huff_book_icc_c = ps_tables->a_book_ps_icc_time_code;
    aa_huff_book_icc_l = ps_tables->a_book_ps_icc_time_length;
  } else {
    ab_dt_flag_icc = 0;
    aa_delta_icc = aa_delta_icc_f;
    aa_huff_book_icc_c = ps_tables->a_book_ps_icc_freq_code;
    aa_huff_book_icc_l = ps_tables->a_book_ps_icc_freq_length;
  }

  {
    static WORD32 initheader = 0;

    if (!initheader || b_header_active) {
      initheader = 1;
      pstr_ps_handle->b_enable_header = 1;
    } else {
      pstr_ps_handle->b_enable_header = 0;
    }
  }

  b_send_header = pstr_ps_handle->b_enable_header ||
                  pstr_ps_handle->b_prev_zero_iid != b_zero_iid ||
                  pstr_ps_handle->b_prev_zero_icc != b_zero_icc;

  ixheaace_write_bits(bb, b_send_header, 1);

  if (b_send_header) {
    ixheaace_write_bits(bb, !b_zero_iid, 1);

    if (!b_zero_iid) {
      ixheaace_write_bits(bb, (pstr_ps_handle->b_hi_freq_res_iid_icc) ? 1 : 0, 3);
    }

    ixheaace_write_bits(bb, !b_zero_icc, 1);

    if (!b_zero_icc) {
      ixheaace_write_bits(bb, (pstr_ps_handle->b_hi_freq_res_iid_icc) ? 1 : 0, 3);
    }

    ixheaace_write_bits(bb, 0, 1);
  }

  ixheaace_write_bits(bb, 0, 1);

  ixheaace_write_bits(bb, 1 - b_keep_params, 2);

  if (!b_keep_params) {
    if (!b_zero_iid) {
      ixheaace_write_bits(bb, ab_dt_flag_iid, 1);

      for (gr = 0; gr < pstr_ps_handle->iid_icc_bins; gr++) {
        ixheaace_write_bits(bb, aa_huff_book_iid_c[aa_delta_iid[gr] + CODE_BCK_LAV_IID],
                            aa_huff_book_iid_l[aa_delta_iid[gr] + CODE_BCK_LAV_IID]);
      }
    }
  }

  if (!b_keep_params) {
    if (!b_zero_icc) {
      ixheaace_write_bits(bb, ab_dt_flag_icc, 1);

      for (gr = 0; gr < pstr_ps_handle->iid_icc_bins; gr++) {
        ixheaace_write_bits(bb, aa_huff_book_icc_c[aa_delta_icc[gr] + CODE_BCK_LAV_ICC],
                            aa_huff_book_icc_l[aa_delta_icc[gr] + CODE_BCK_LAV_ICC]);
      }
    }
  }

  pstr_ps_handle->b_prev_zero_iid = b_zero_iid;
  pstr_ps_handle->b_prev_zero_icc = b_zero_icc;

  return ia_enhaacplus_enc_get_bits_available(bb);
}

WORD32
ixheaace_append_ps_bitstream(ixheaace_pstr_ps_enc pstr_ps_handle, ixheaace_bit_buf_handle hdl_bs,
                             WORD32 *sbr_hdr_bits) {
  if (!pstr_ps_handle) {
    return 0;
  }
  if (!hdl_bs) {
    return ia_enhaacplus_enc_get_bits_available(&pstr_ps_handle->ps_bit_buf);
  } else {
    UWORD8 num_bits = (UWORD8)ia_enhaacplus_enc_get_bits_available(&pstr_ps_handle->ps_bit_buf);

    ixheaace_write_bits(hdl_bs, EXTENSION_ID_PS_CODING, SI_SBR_EXTENSION_ID_BITS);
    iexheaax_append_bitstream(hdl_bs, &pstr_ps_handle->ps_bit_buf, num_bits);

    pstr_ps_handle->bit_buf_read_offset = (WORD32)(pstr_ps_handle->ps_bit_buf.ptr_read_next -
                                                   pstr_ps_handle->ps_bit_buf.ptr_bit_buf_base);
    pstr_ps_handle->bit_buf_write_offset = (WORD32)(pstr_ps_handle->ps_bit_buf.ptr_write_next -
                                                    pstr_ps_handle->ps_bit_buf.ptr_bit_buf_base);

    return ia_enhaacplus_enc_get_bits_available(hdl_bs) - (*sbr_hdr_bits) -
           SI_FILL_EXTENTION_BITS;
  }
}
