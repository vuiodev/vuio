/******************************************************************************
 *
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
#include "ixheaacd_mps_struct_def.h"
#include "ixheaacd_mps_res_rom.h"
#include "ixheaacd_mps_aac_struct.h"
#include "ixheaac_constants.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaacd_bitbuffer.h"
#include "ixheaacd_error_codes.h"
#include "ixheaacd_common_rom.h"
#include "ixheaacd_sbrdecsettings.h"
#include "ixheaacd_sbr_scale.h"
#include "ixheaacd_env_extr_part.h"
#include "ixheaacd_sbr_rom.h"
#include "ixheaacd_lpp_tran.h"
#include "ixheaacd_hybrid.h"
#include "ixheaacd_ps_dec.h"
#include "ixheaacd_env_extr.h"
#include "ixheaacd_mps_polyphase.h"
#include "ixheaacd_config.h"
#include "ixheaacd_qmf_dec.h"
#include "ixheaacd_mps_dec.h"
#include "ixheaacd_mps_bitdec.h"
#include "ixheaacd_mps_macro_def.h"
#include "ixheaacd_mps_basic_op.h"

VOID ixheaacd_init_tonality(ia_heaac_mps_state_struct *pstr_mps_state) {
  ia_mps_dec_tonality_state_struct *ton_state = pstr_mps_state->mps_persistent_mem.ton_state;
  WORD32 cnt = pstr_mps_state->qmf_bands * 8;
  WORD32 qmf_bands = pstr_mps_state->qmf_bands;

  memset(ton_state->spec_prev_real, 0, cnt * sizeof(ton_state->spec_prev_real[0]));
  memset(ton_state->spec_prev_imag, 0, cnt * sizeof(ton_state->spec_prev_imag[0]));
  memset(ton_state->p_cross_real, 0, cnt * sizeof(ton_state->p_cross_real[0]));
  memset(ton_state->p_cross_imag, 0, cnt * sizeof(ton_state->p_cross_imag[0]));
  memset(ton_state->p_sum, 0, cnt * sizeof(ton_state->p_sum[0]));
  memset(ton_state->p_sum_prev, 0, cnt * sizeof(ton_state->p_sum_prev[0]));

  memset(ton_state->buf_real, 0, qmf_bands * 6 * sizeof(ton_state->buf_real[0][0]));
  memset(ton_state->buf_imag, 0, qmf_bands * 6 * sizeof(ton_state->buf_imag[0][0]));
  memset(ton_state->win_buf_real, 0, qmf_bands * 16 * sizeof(ton_state->win_buf_real[0][0]));
  memset(ton_state->win_buf_imag, 0, qmf_bands * 16 * sizeof(ton_state->win_buf_imag[0][0]));
}

VOID ixheaacd_zoom_fft16(WORD32 *in_real, WORD32 *in_imag, WORD32 *out_real, WORD32 *out_imag,
                         WORD32 qmf_band, WORD32 dfrac,
                         ia_mps_dec_mps_tables_struct *ia_mps_dec_mps_table_ptr) {
  WORD32 blackman[16];

  WORD32 v_real[16], v_imag[16];
  WORD32 t_real, t_imag;
  WORD32 e_real, e_imag;

  WORD32 temp_1, temp_2, temp3, temp4;
  const WORD32 *bitrev = ia_mps_dec_mps_table_ptr->tonality_table_ptr->bitrev;
  const WORD32 *w_real = ia_mps_dec_mps_table_ptr->tonality_table_ptr->w_real;
  const WORD32 *w_imag = ia_mps_dec_mps_table_ptr->tonality_table_ptr->w_imag;
  const WORD32 *cos_tab = ia_mps_dec_mps_table_ptr->hybrid_table_ptr->cosine_array;
  const WORD32 *sin_tab = ia_mps_dec_mps_table_ptr->hybrid_table_ptr->sine_array;

  WORD32 i, j, s1, s2;

  temp3 = TWO_PI_BY_FIFTEEN_Q15;

  for (i = 0; i < 16; i++) {
    temp_1 = (i << 15) + dfrac;
    temp_2 = ixheaacd_mps_mult32_shr_15(temp_1, temp3);
    temp_2 = ixheaacd_mps_cos(temp_2, cos_tab);
    temp_2 >>= 1;

    temp_1 <<= 1;
    temp_1 = ixheaacd_mps_cos(temp_1, cos_tab);

    temp_1 = ixheaacd_mps_mult32x16_shr_16(temp_1, TWO_BY_TWENTYFIVE_Q16);

    temp4 = POINT_FOUR_TWO_Q15 - temp_2;

    blackman[i] = temp_1 + temp4;
  }

  for (i = 0; i < 16; i++) {
    WORD32 idx = bitrev[i];
    temp_1 = ixheaacd_mps_mult32_shr_30(in_real[i], w_real[i]) -
             ixheaacd_mps_mult32_shr_30(in_imag[i], w_imag[i]);
    v_real[idx] = ixheaacd_mps_mult32_shr_30(temp_1, blackman[i]);

    temp_1 = ixheaacd_mps_mult32_shr_30(in_real[i], w_imag[i]) +
             ixheaacd_mps_mult32_shr_30(in_imag[i], w_real[i]);
    v_imag[idx] = ixheaacd_mps_mult32_shr_30(temp_1, blackman[i]);
  }

  for (s1 = 1, s2 = 16; s1 < 8; s1 <<= 1, s2 >>= 1) {
    for (i = 0; i < 16; i += 2 * s1) {
      for (j = 0; j < s1; j++) {
        t_real = ixheaacd_mps_mult32_shr_30(v_real[i + j + s1], w_real[j * s2]) -
                 ixheaacd_mps_mult32_shr_30(v_imag[i + j + s1], w_imag[j * s2]);
        t_imag = ixheaacd_mps_mult32_shr_30(v_real[i + j + s1], w_imag[j * s2]) +
                 ixheaacd_mps_mult32_shr_30(v_imag[i + j + s1], w_real[j * s2]);

        v_real[i + j + s1] = v_real[i + j] - t_real;
        v_imag[i + j + s1] = v_imag[i + j] - t_imag;

        v_real[i + j] += t_real;
        v_imag[i + j] += t_imag;
      }
    }
  }

  for (j = 0; j < 8; j++) {
    WORD32 idx = j << 1;
    t_real = ixheaacd_mps_mult32_shr_30(v_real[j + 8], w_real[idx]) -
             ixheaacd_mps_mult32_shr_30(v_imag[j + 8], w_imag[idx]);
    t_imag = ixheaacd_mps_mult32_shr_30(v_real[j + 8], w_imag[idx]) +
             ixheaacd_mps_mult32_shr_30(v_imag[j + 8], w_real[idx]);

    if ((qmf_band & ONE_BIT_MASK) == 0) {
      out_real[j] = v_real[j] + t_real;
      out_imag[j] = v_imag[j] + t_imag;
    } else {
      out_real[j] = v_real[j] - t_real;
      out_imag[j] = v_imag[j] - t_imag;
    }
  }

  temp3 = MINUS_PI_BY_EIGHT_Q15;
  for (i = 0; i < 8; i++) {
    if ((qmf_band & ONE_BIT_MASK) == 0) {
      temp_1 = dfrac * i;
      temp_1 = ixheaacd_mps_mult32_shr_15(temp_1, temp3);
      e_real = ixheaacd_mps_cos(temp_1, cos_tab);
      e_imag = ixheaacd_mps_sin(temp_1, sin_tab);
    } else {
      temp_1 = dfrac * (i - 8);
      temp_1 = ixheaacd_mps_mult32_shr_15(temp_1, temp3);
      e_real = ixheaacd_mps_cos(temp_1, cos_tab);
      e_imag = ixheaacd_mps_sin(temp_1, sin_tab);
    }

    t_real = ixheaacd_mps_mult32_shr_15(out_real[i], e_real) -
             ixheaacd_mps_mult32_shr_15(out_imag[i], e_imag);
    out_imag[i] = ixheaacd_mps_mult32_shr_15(out_real[i], e_imag) +
                  ixheaacd_mps_mult32_shr_15(out_imag[i], e_real);
    out_real[i] = t_real;
  }
}

VOID ixheaacd_measure_tonality(ia_heaac_mps_state_struct *pstr_mps_state, WORD32 *tonality) {
  ia_mps_dec_tonality_state_struct *ton_state = pstr_mps_state->mps_persistent_mem.ton_state;

  WORD32 *qmf_real;
  WORD32 *qmf_imag;

  WORD32 *spec_zoom_real;
  WORD32 *spec_zoom_imag;

  WORD32 *spec_prev_real = ton_state->spec_prev_real;
  WORD32 *spec_prev_imag = ton_state->spec_prev_imag;

  WORD32 *p_cross_real = ton_state->p_cross_real;
  WORD32 *p_cross_imag = ton_state->p_cross_imag;

  WORD32 *p_sum = ton_state->p_sum;
  WORD32 *p_sum_prev = ton_state->p_sum_prev;

  WORD32 *p_max;

  WORD32 *coh_spec;
  WORD32 *pow_spec;

  WORD32 *p_buf_real, *p_buf_imag, *p_buf_re, *p_buf_im;
  WORD32 *buf_real, *buf_imag;
  WORD32 g, gmax;
  WORD32 i, j, q, s, c, cnt;

  WORD32 const *part;
  WORD32 pstart;
  WORD32 pstop = 0;
  WORD32 pqmf, num, den, tmp_ton, beta, dwin, dfrac;
  WORD16 q_beta, q_tmp_ton;

  WORD32 qmf_bands = pstr_mps_state->qmf_bands;
  WORD32 time_slots = pstr_mps_state->time_slots;
  WORD32 num_input_channels = pstr_mps_state->num_input_channels;
  WORD32 num_parameter_bands = pstr_mps_state->num_parameter_bands;
  WORD32 sampling_freq = pstr_mps_state->sampling_freq;
  const WORD32 *sqrt_tab = pstr_mps_state->ia_mps_dec_mps_table.common_table_ptr->sqrt_tab;

  WORD32 nstart;

  WORD32 tmp_real, tmp_imag;

  WORD32 temp_1, temp;
  WORD16 qtemp1, qtemp2;

  spec_zoom_real = (WORD32 *)((WORD8 *)pstr_mps_state->mps_scratch_mem_v +
                              IXHEAAC_GET_SIZE_ALIGNED(SCRATCH_OFFSET_SMOOTHING, BYTE_ALIGN_8));
  spec_zoom_imag = spec_zoom_real + IXHEAAC_GET_SIZE_ALIGNED_TYPE(
                                        QMF_BANDSX8, sizeof(*spec_zoom_imag), BYTE_ALIGN_8);
  p_max =
      spec_zoom_imag + IXHEAAC_GET_SIZE_ALIGNED_TYPE(QMF_BANDSX8, sizeof(*p_max), BYTE_ALIGN_8);
  coh_spec = p_max + IXHEAAC_GET_SIZE_ALIGNED_TYPE(QMF_BANDSX8, sizeof(*coh_spec), BYTE_ALIGN_8);
  pow_spec =
      coh_spec + IXHEAAC_GET_SIZE_ALIGNED_TYPE(QMF_BANDSX8, sizeof(*pow_spec), BYTE_ALIGN_8);

  qmf_real =
      pow_spec + IXHEAAC_GET_SIZE_ALIGNED_TYPE(QMF_BANDSX8, sizeof(*qmf_real), BYTE_ALIGN_8);
  qmf_imag = qmf_real + IXHEAAC_GET_SIZE_ALIGNED_TYPE(QBXTS, sizeof(*qmf_imag), BYTE_ALIGN_8);

  switch (num_parameter_bands) {
    case PARAMETER_BANDS_4:
      part = pstr_mps_state->ia_mps_dec_mps_table.tonality_table_ptr->part4;
      break;
    case PARAMETER_BANDS_5:
      part = pstr_mps_state->ia_mps_dec_mps_table.tonality_table_ptr->part5;
      break;
    case PARAMETER_BANDS_7:
      part = pstr_mps_state->ia_mps_dec_mps_table.tonality_table_ptr->part7;
      break;
    case PARAMETER_BANDS_10:
      part = pstr_mps_state->ia_mps_dec_mps_table.tonality_table_ptr->part10;
      break;
    case PARAMETER_BANDS_14:
      part = pstr_mps_state->ia_mps_dec_mps_table.tonality_table_ptr->part14;
      break;
    case PARAMETER_BANDS_20:
      part = pstr_mps_state->ia_mps_dec_mps_table.tonality_table_ptr->part20;
      break;
    case PARAMETER_BANDS_28:
      part = pstr_mps_state->ia_mps_dec_mps_table.tonality_table_ptr->part28;
      break;
    case PARAMETER_BANDS_40:
      part = pstr_mps_state->ia_mps_dec_mps_table.tonality_table_ptr->part40;
      break;
    default:
      part = pstr_mps_state->ia_mps_dec_mps_table.tonality_table_ptr->part4;
      break;
  }

  temp = time_slots - 6;

  p_buf_real = pstr_mps_state->array_struct->buf_real;
  p_buf_imag = pstr_mps_state->array_struct->buf_imag;

  for (q = 0; q < qmf_bands; q++) {
    qmf_real += 6;
    qmf_imag += 6;

    p_buf_re = p_buf_real;
    p_buf_im = p_buf_imag;
    for (s = 0; s < time_slots; s++) {
      tmp_real = 0;
      tmp_imag = 0;

      buf_real = p_buf_re;
      buf_imag = p_buf_im;

      for (c = 0; c < num_input_channels; c++) {
        tmp_real += *buf_real;
        tmp_imag += *buf_imag;

        buf_real += TSXHB;
        buf_imag += TSXHB;
      }

      if (s == temp) {
        qmf_real -= time_slots;
        qmf_imag -= time_slots;
      }

      if (s + 6 < time_slots) {
        *qmf_real++ = tmp_real;
        *qmf_imag++ = tmp_imag;
      } else {
        *qmf_real++ = ton_state->buf_real[q][s + 6 - time_slots];
        *qmf_imag++ = ton_state->buf_imag[q][s + 6 - time_slots];

        ton_state->buf_real[q][s + 6 - time_slots] = tmp_real;
        ton_state->buf_imag[q][s + 6 - time_slots] = tmp_imag;
      }
      p_buf_re += MAX_HYBRID_BANDS;
      p_buf_re += MAX_HYBRID_BANDS;
    }
    qmf_real += temp;
    qmf_imag += temp;

    p_buf_real++;
    p_buf_imag++;
  }

  gmax = pstr_mps_state->ia_mps_dec_mps_table.tonality_table_ptr->gmax_fix[time_slots];
  dwin = pstr_mps_state->ia_mps_dec_mps_table.tonality_table_ptr->dwin_fix[time_slots];

  qtemp1 = 15;
  temp_1 = ixheaacd_mps_mult32(dwin, (40 * (qmf_bands)), &qtemp1, 0);
  beta = ixheaacd_mps_div_32(temp_1, sampling_freq, &q_beta);
  q_beta = q_beta + qtemp1;
  beta = ixheaacd_mps_convert_to_qn(beta, q_beta, 15);

  for (i = 0; i < num_parameter_bands; i++) {
    tonality[i] = ONE_IN_Q15;
  }

  for (g = 0; g < gmax; g++) {
    nstart = pstr_mps_state->ia_mps_dec_mps_table.tonality_table_ptr->nstart_fix[g][time_slots];
    if (time_slots <= 16)
      dfrac = 0;
    else
      dfrac =
          pstr_mps_state->ia_mps_dec_mps_table.tonality_table_ptr->dfrac_fix[g][time_slots - 16];

    qmf_real = pow_spec + QBX48;
    qmf_imag = qmf_real + QMF_BANDSXTSX6;
    for (q = 0; q < qmf_bands; q++) {
      for (i = 0; i < 16; i++) {
        if (nstart + i < 0) {
          ton_state->win_buf_real[q][i] = ton_state->win_buf_real[q][16 + nstart + i];
          ton_state->win_buf_imag[q][i] = ton_state->win_buf_imag[q][16 + nstart + i];
        } else {
          ton_state->win_buf_real[q][i] = qmf_real[nstart + i];
          ton_state->win_buf_imag[q][i] = qmf_imag[nstart + i];
        }
      }
      qmf_real += time_slots;
      qmf_imag += time_slots;
    }

    for (q = 0; q < qmf_bands; q++) {
      ixheaacd_zoom_fft16(&(ton_state->win_buf_real[q][0]), &(ton_state->win_buf_imag[q][0]),
                          &(spec_zoom_real[q * 8]), &(spec_zoom_imag[q * 8]), q, dfrac,
                          &(pstr_mps_state->ia_mps_dec_mps_table));
    }

    cnt = 8 * qmf_bands;
    for (i = 0; i < cnt; i++) {
      WORD64 temp;
      WORD32 one_minus_beta = ONE_IN_Q15 - beta;
      WORD32 x = *spec_zoom_real;
      WORD32 y = *spec_zoom_imag;

      temp = x * spec_prev_real[i] + y * spec_prev_imag[i];
      temp_1 = (WORD32)(temp >> 10);
      temp_1 = ixheaacd_mps_mult32_shr_15(temp_1, beta);

      p_cross_real[i] = ixheaacd_mps_mult32_shr_15(p_cross_real[i], one_minus_beta);
      p_cross_real[i] += temp_1;

      temp = y * spec_prev_real[i] - x * spec_prev_imag[i];
      temp_1 = (WORD32)(temp >> 10);
      temp_1 = ixheaacd_mps_mult32_shr_15(temp_1, beta);

      p_cross_imag[i] = ixheaacd_mps_mult32_shr_15(p_cross_imag[i], one_minus_beta);
      p_cross_imag[i] += temp_1;

      temp = x * x + y * y;
      temp_1 = (WORD32)(temp >> 10);
      temp_1 = ixheaacd_mps_mult32_shr_15(temp_1, beta);

      p_sum[i] = ixheaacd_mps_mult32_shr_15(p_sum[i], one_minus_beta);
      p_sum[i] += temp_1;

      *p_max = (p_sum[i] > p_sum_prev[i]) ? p_sum[i] : p_sum_prev[i];

      p_sum_prev[i] = p_sum[i];

      temp = p_cross_real[i] * p_cross_real[i] + p_cross_imag[i] * p_cross_imag[i];
      temp_1 = (WORD32)(temp >> 10);
      qtemp1 = 10;
      temp_1 = ixheaacd_mps_sqrt(temp_1, &qtemp1, sqrt_tab);
      *coh_spec = ixheaacd_mps_div_32(temp_1, *p_max++, &qtemp2);
      qtemp2 = qtemp2 + qtemp1 - 10;
      *coh_spec = ixheaacd_mps_convert_to_qn(*coh_spec, qtemp2, 10);
      coh_spec++;

      temp = x * x + y * y + spec_prev_real[i] * spec_prev_real[i] +
             spec_prev_imag[i] * spec_prev_imag[i];
      *pow_spec = (WORD32)(temp >> 10);

      spec_prev_real[i] = *spec_zoom_real++;
      spec_prev_imag[i] = *spec_zoom_imag++;
    }
    spec_zoom_real -= i;
    spec_zoom_imag -= i;
    p_max -= i;
    coh_spec -= i;
    pow_spec -= i;

    pstart = 0;
    pqmf = 0;
    for (i = 0; i < num_parameter_bands; i++) {
      pqmf += part[i];
      pstop = ((pqmf << 3) + ONE_IN_Q14) >> 15;

      num = 0;
      den = 0;
      for (j = pstart; j < pstop; j++) {
        num += ixheaacd_mps_mult32_shr_n(*pow_spec, *coh_spec, 10);
        coh_spec++;
        den += *pow_spec++;
      }

      tmp_ton = ixheaacd_mps_div_32(num, den, &q_tmp_ton);
      ixheaacd_mps_convert_to_qn(tmp_ton, q_tmp_ton, 15);

      if (tmp_ton > 32767) {
        tmp_ton = 32767;
      }

      if (tmp_ton < tonality[i]) tonality[i] = tmp_ton;

      pstart = pstop;
    }
    coh_spec -= pstop;
    pow_spec -= pstop;
  }
  return;
}
