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
#include "ixheaac_error_standards.h"
#include "ixheaac_type_def.h"
#include "ixheaace_sbr_def.h"
#include "ixheaace_resampler.h"
#include "ixheaace_sbr_rom.h"
#include "ixheaace_common_rom.h"
#include "ixheaace_bitbuffer.h"
#include "ixheaace_sbr_hybrid.h"
#include "ixheaace_sbr_ps_enc.h"
#include "ixheaac_constants.h"
#include "ixheaace_aac_constants.h"
#include "ixheaace_error_codes.h"
#include "ixheaace_bitbuffer.h"

WORD32 ixheaace_get_ps_mode(WORD32 bitrate) {
  WORD32 ps_mode = 0;

  if (bitrate < 21000) {
    ps_mode = PS_MODE_LOW_FREQ_RES_IID_ICC;
  }

  return ps_mode;
}

IA_ERRORCODE
ixheaace_create_ps_enc(ixheaace_pstr_ps_enc pstr_ps_enc, WORD32 ps_mode,
                       FLOAT32 *ptr_common_buffer, FLOAT32 *ptr_common_buffer2,
                       FLOAT32 *ptr_ps_buf3) {
  WORD32 i;
  IA_ERRORCODE err = IA_NO_ERROR;

  FLOAT32 *ptr1, *ptr2, *ptr3, *ptr4;

  if (pstr_ps_enc == NULL) {
    return IA_EXHEAACE_INIT_FATAL_PS_INIT_FAILED;
  }
  ptr1 = &ptr_common_buffer2[IXHEAACE_QMF_TIME_SLOTS * IXHEAACE_QMF_CHANNELS];
  ptr2 = pstr_ps_enc->ps_buf2;
  ptr3 = ptr_ps_buf3;
  ptr4 = &ptr_common_buffer[5 * NO_OF_ESTIMATES * MAXIMUM_FREQ_COEFFS];

  pstr_ps_enc->ps_mode = ps_mode;
  pstr_ps_enc->b_prev_zero_iid = 0;
  pstr_ps_enc->b_prev_zero_icc = 0;
  pstr_ps_enc->b_hi_freq_res_iid_icc = ((ps_mode & PS_MODE_LOW_FREQ_RES_IID_ICC) != 0) ? 0 : 1;
  pstr_ps_enc->iid_icc_bins =
      (pstr_ps_enc->b_hi_freq_res_iid_icc) ? NUMBER_OF_IID_BINS : NUMBER_OF_LOW_RES_IID_BINS;

  pstr_ps_enc->aaa_ICC_data_buf = (FLOAT32 **)ptr1;
  ptr1 += NUMBER_OF_BINS * sizeof(FLOAT32 *) / sizeof(FLOAT32);

  pstr_ps_enc->aaa_IID_data_buf = (FLOAT32 **)ptr1;
  ptr1 += NUMBER_OF_BINS * sizeof(FLOAT32 *) / sizeof(FLOAT32);

  for (i = 0; i < NUMBER_OF_BINS; i++) {
    pstr_ps_enc->aaa_ICC_data_buf[i] = ptr1;
    ptr1 += SYSTEMLOOKAHEAD + 1;

    memset(pstr_ps_enc->aaa_ICC_data_buf[i], 0,
           (SYSTEMLOOKAHEAD + 1) * sizeof(pstr_ps_enc->aaa_ICC_data_buf[0]));

    pstr_ps_enc->aaa_IID_data_buf[i] = ptr1;
    ptr1 += SYSTEMLOOKAHEAD + 1;

    memset(pstr_ps_enc->aaa_IID_data_buf[i], 0,
           (SYSTEMLOOKAHEAD + 1) * sizeof(pstr_ps_enc->aaa_IID_data_buf[0]));
  }

  pstr_ps_enc->ptr_hybrid_left = &pstr_ps_enc->hybrid_left;
  pstr_ps_enc->ptr_hybrid_right = &pstr_ps_enc->hybrid_right;

  err = ixheaace_create_hybrid_filter_bank(pstr_ps_enc->ptr_hybrid_left, &ptr4);

  if (err) {
    return err;
  }

  err = ixheaace_create_hybrid_filter_bank(pstr_ps_enc->ptr_hybrid_right, &ptr4);

  if (err) {
    return err;
  }

  for (i = 0; i < NUMBER_OF_SUBSAMPLES; i++) {
    pstr_ps_enc->m_hybrid_real_left[i] = ptr3;
    ptr3 += IXHEAACE_NUM_HYBRID_BANDS;

    memset(pstr_ps_enc->m_hybrid_real_left[i], 0,
           IXHEAACE_NUM_HYBRID_BANDS * sizeof(pstr_ps_enc->m_hybrid_real_left[0]));

    pstr_ps_enc->m_hybrid_imag_left[i] = ptr3;
    ptr3 += IXHEAACE_NUM_HYBRID_BANDS;

    memset(pstr_ps_enc->m_hybrid_imag_left[i], 0,
           IXHEAACE_NUM_HYBRID_BANDS * sizeof(pstr_ps_enc->m_hybrid_imag_left[0]));

    pstr_ps_enc->m_hybrid_real_right[i] = ptr1;
    ptr1 += IXHEAACE_NUM_HYBRID_BANDS;

    memset(pstr_ps_enc->m_hybrid_real_right[i], 0,
           IXHEAACE_NUM_HYBRID_BANDS * sizeof(pstr_ps_enc->m_hybrid_real_right[0]));

    pstr_ps_enc->m_hybrid_imag_right[i] = ptr1;
    ptr1 += IXHEAACE_NUM_HYBRID_BANDS;

    memset(pstr_ps_enc->m_hybrid_imag_right[i], 0,
           IXHEAACE_NUM_HYBRID_BANDS * sizeof(pstr_ps_enc->m_hybrid_imag_right[0]));
  }

  pstr_ps_enc->temp_qmf_left_real = (FLOAT32 **)ptr1;
  ptr1 += IXHEAACE_HYBRID_FILTER_DELAY * sizeof(FLOAT32 *) / sizeof(FLOAT32);

  pstr_ps_enc->temp_qmf_left_imag = (FLOAT32 **)ptr1;
  ptr1 += IXHEAACE_HYBRID_FILTER_DELAY * sizeof(WORD32 *) / sizeof(WORD32);

  pstr_ps_enc->hist_qmf_left_real = (FLOAT32 **)ptr2;
  ptr2 += IXHEAACE_HYBRID_FILTER_DELAY * sizeof(FLOAT32 *) / sizeof(FLOAT32);

  pstr_ps_enc->hist_qmf_left_imag = (FLOAT32 **)ptr2;
  ptr2 += IXHEAACE_HYBRID_FILTER_DELAY * sizeof(FLOAT32 *) / sizeof(FLOAT32);

  pstr_ps_enc->hist_qmf_right_real = (FLOAT32 **)ptr2;
  ptr2 += IXHEAACE_HYBRID_FILTER_DELAY * sizeof(FLOAT32 *) / sizeof(FLOAT32);

  pstr_ps_enc->hist_qmf_right_imag = (FLOAT32 **)ptr2;
  ptr2 += IXHEAACE_HYBRID_FILTER_DELAY * sizeof(FLOAT32 *) / sizeof(FLOAT32);

  for (i = 0; i < IXHEAACE_HYBRID_FILTER_DELAY; i++) {
    pstr_ps_enc->temp_qmf_left_real[i] = ptr1;
    ptr1 += NUMBER_OF_QMF_BANDS;
    memset(pstr_ps_enc->temp_qmf_left_real[i], 0,
           NUMBER_OF_QMF_BANDS * sizeof(pstr_ps_enc->temp_qmf_left_real[0]));

    pstr_ps_enc->temp_qmf_left_imag[i] = ptr1;
    ptr1 += NUMBER_OF_QMF_BANDS;

    memset(pstr_ps_enc->temp_qmf_left_imag[i], 0,
           NUMBER_OF_QMF_BANDS * sizeof(pstr_ps_enc->temp_qmf_left_imag[0]));

    pstr_ps_enc->hist_qmf_left_real[i] = ptr2;
    ptr2 += NUMBER_OF_QMF_BANDS;

    memset(pstr_ps_enc->hist_qmf_left_real[i], 0,
           NUMBER_OF_QMF_BANDS * sizeof(pstr_ps_enc->hist_qmf_left_real[0]));

    pstr_ps_enc->hist_qmf_left_imag[i] = ptr2;
    ptr2 += NUMBER_OF_QMF_BANDS;

    memset(pstr_ps_enc->hist_qmf_left_imag[i], 0,
           NUMBER_OF_QMF_BANDS * sizeof(pstr_ps_enc->hist_qmf_left_imag[0]));

    pstr_ps_enc->hist_qmf_right_real[i] = ptr2;
    ptr2 += NUMBER_OF_QMF_BANDS;

    memset(pstr_ps_enc->hist_qmf_right_real[i], 0,
           NUMBER_OF_QMF_BANDS * sizeof(pstr_ps_enc->hist_qmf_right_real[0]));

    pstr_ps_enc->hist_qmf_right_imag[i] = ptr2;
    ptr2 += NUMBER_OF_QMF_BANDS;

    memset(pstr_ps_enc->hist_qmf_right_imag[i], 0,
           NUMBER_OF_QMF_BANDS * sizeof(pstr_ps_enc->hist_qmf_right_imag[0]));
  }

  memset(pstr_ps_enc->pow_left_right, 0, sizeof(pstr_ps_enc->pow_left_right));
  memset(pstr_ps_enc->pow_corr_real_imag, 0, sizeof(pstr_ps_enc->pow_corr_real_imag));

  if ((pstr_ps_enc->hist_qmf_left_real == NULL) || (pstr_ps_enc->hist_qmf_left_imag == NULL) ||
      (pstr_ps_enc->hist_qmf_right_real == NULL) || (pstr_ps_enc->hist_qmf_right_imag == NULL)) {
    return IA_EXHEAACE_INIT_FATAL_PS_INIT_FAILED;
  }

  for (i = 0; i < pstr_ps_enc->iid_icc_bins; i++) {
    pstr_ps_enc->aaa_IID_data_buf[i][0] = 0;
    pstr_ps_enc->aaa_ICC_data_buf[i][0] = -1.0f;
  }

  pstr_ps_enc->bit_buf_read_offset = 0;
  pstr_ps_enc->bit_buf_write_offset = 0;

  ia_enhaacplus_enc_create_bitbuffer(&pstr_ps_enc->ps_bit_buf, (UWORD8 *)ptr1, 255 + 15);

  pstr_ps_enc->ps_bit_buf.ptr_read_next =
      pstr_ps_enc->ps_bit_buf.ptr_bit_buf_base + pstr_ps_enc->bit_buf_read_offset;
  pstr_ps_enc->ps_bit_buf.ptr_write_next =
      pstr_ps_enc->ps_bit_buf.ptr_bit_buf_base + pstr_ps_enc->bit_buf_write_offset;

  return err;
}
