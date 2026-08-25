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

#pragma once
#define NUMBER_OF_BINS (20)
#define NUMBER_OF_LOW_RES_BINS (NUMBER_OF_IID_BINS / 2)

#define NUMBER_OF_IID_BINS (NUMBER_OF_BINS)
#define NUMBER_OF_ICC_BINS (NUMBER_OF_BINS)
#define NUMBER_OF_IPD_BINS (11)
#define NUMBER_OF_IPD_BINS_EST (NUMBER_OF_BINS)

#define NUMBER_OF_LOW_RES_IID_BINS (NUMBER_OF_LOW_RES_BINS)

#define NUMBER_OF_IPD_GROUPS (NUMBER_OF_IPD_BINS_EST + 6 + 2)
#define IXHEAACE_IPD_MASK_NEGATED (0x00001000)

#define NUMBER_OF_SUBSAMPLES (32)
#define NUMBER_OF_QMF_BANDS (64)
#define SYSTEMLOOKAHEAD (1)
#define PS_MODE_LOW_FREQ_RES_IID_ICC (0x00020000)
#define SUBQMF_BINS_ENERGY (8)
#define SUBQMF_GROUPS_MIX (16)

struct ixheaace_ps_enc {
  WORD32 b_enable_header;

  WORD32 b_hi_freq_res_iid_icc;
  WORD32 iid_icc_bins;

  UWORD32 b_prev_zero_iid;
  UWORD32 b_prev_zero_icc;

  WORD32 ps_mode;
  WORD32 hdr_bits_prev_frame;

  WORD8 a_last_iid_index[NUMBER_OF_IID_BINS];
  WORD8 a_last_icc_index[NUMBER_OF_ICC_BINS];

  ixheaace_str_hybrid hybrid_left;
  ixheaace_str_hybrid hybrid_right;

  ixheaace_pstr_hybrid ptr_hybrid_left;
  ixheaace_pstr_hybrid ptr_hybrid_right;

  WORD32 bit_buf_write_offset;
  WORD32 bit_buf_read_offset;

  FLOAT32 **aaa_IID_data_buf;
  FLOAT32 **aaa_ICC_data_buf;

  FLOAT32 *m_hybrid_real_left[NUMBER_OF_SUBSAMPLES];
  FLOAT32 *m_hybrid_imag_left[NUMBER_OF_SUBSAMPLES];
  FLOAT32 *m_hybrid_real_right[NUMBER_OF_SUBSAMPLES];
  FLOAT32 *m_hybrid_imag_right[NUMBER_OF_SUBSAMPLES];

  FLOAT32 pow_left_right[2 * NUMBER_OF_BINS];
  FLOAT32 pow_corr_real_imag[2 * NUMBER_OF_BINS];

  FLOAT32 **temp_qmf_left_real;
  FLOAT32 **temp_qmf_left_imag;
  FLOAT32 **hist_qmf_left_real;
  FLOAT32 **hist_qmf_left_imag;
  FLOAT32 **hist_qmf_right_real;
  FLOAT32 **hist_qmf_right_imag;

  FLOAT32
  ps_buf2[(IXHEAACE_HYBRID_FILTER_DELAY * sizeof(FLOAT32 *) * (1 + NUMBER_OF_QMF_BANDS))];
  ixheaace_bit_buf ps_bit_buf;
};

typedef struct ixheaace_ps_enc *ixheaace_pstr_ps_enc;

WORD32 ixheaace_get_ps_mode(WORD32 bit_rate);

IA_ERRORCODE
ixheaace_create_ps_enc(ixheaace_pstr_ps_enc pstr_ps_enc, WORD32 ps_mode,
                       FLOAT32 *ptr_common_buffer, FLOAT32 *ptr_common_buffer2,
                       FLOAT32 *ptr_ps_buf3);

IA_ERRORCODE ixheaace_encode_ps_frame(ixheaace_pstr_ps_enc pms, FLOAT32 **i_buffer_left,
                                      FLOAT32 **r_buffer_left, FLOAT32 **i_buffer_right,
                                      FLOAT32 **r_buffer_right, ixheaace_str_ps_tab *ps_tabs,
                                      ixheaace_comm_tables *common_tab);
