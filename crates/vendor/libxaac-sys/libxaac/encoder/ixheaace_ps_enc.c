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
#include "ixheaac_error_standards.h"
#include "ixheaace_sbr_def.h"
#include "ixheaace_resampler.h"
#include "ixheaace_sbr_rom.h"
#include "ixheaace_common_rom.h"
#include "ixheaace_bitbuffer.h"
#include "ixheaace_sbr_hybrid.h"
#include "ixheaace_sbr_ps_enc.h"
#include "ixheaac_constants.h"
#include "ixheaace_aac_constants.h"

static VOID ixheaace_memcpy(void *dest, const void *src, WORD32 count) {
  memcpy(dest, src, count);
}

static VOID ia_enhaacplus_enc_downmix_to_mono(ixheaace_pstr_ps_enc pms, FLOAT32 **qmf_left_real,
                                              FLOAT32 **qmf_left_imag, FLOAT32 **qmf_right_real,
                                              FLOAT32 **qmf_right_imag,
                                              ixheaace_str_ps_tab *ps_tables) {
  WORD32 i;
  WORD32 group;
  WORD32 subband;
  WORD32 max_subband;
  WORD32 subband_grp;

  FLOAT32 temp_left_real;
  FLOAT32 temp_left_imag;
  FLOAT32 temp_right_real;
  FLOAT32 temp_right_imag;
  FLOAT32 temp, temp2;

  for (i = 0; i < IXHEAACE_HYBRID_FILTER_DELAY; i++) {
    ixheaace_memcpy(pms->temp_qmf_left_real[i],
                    qmf_left_real[NUMBER_OF_SUBSAMPLES - IXHEAACE_HYBRID_FILTER_DELAY + i],
                    NUMBER_OF_QMF_BANDS * sizeof(**qmf_left_real));
    ixheaace_memcpy(pms->temp_qmf_left_imag[i],
                    qmf_left_imag[NUMBER_OF_SUBSAMPLES - IXHEAACE_HYBRID_FILTER_DELAY + i],
                    NUMBER_OF_QMF_BANDS * sizeof(**qmf_left_imag));
  }

  {
    FLOAT32 *hybrid_left_real_init, *hybrid_right_real_init;
    hybrid_left_real_init = pms->m_hybrid_real_left[0];
    hybrid_right_real_init = pms->m_hybrid_real_right[0];

    for (group = 0; group < SUBQMF_GROUPS_MIX; group++) {
      FLOAT32 *hybrid_left_real, *hybrid_right_real;

      subband = ps_tables->grp_borders_mix[group];
      hybrid_left_real = hybrid_left_real_init + subband;
      hybrid_right_real = hybrid_right_real_init + subband;

      for (i = NUMBER_OF_SUBSAMPLES; i != 0; i--) {
        FLOAT32 temp_left_real_1, temp_left_imag_1;

        temp_left_real = hybrid_left_real[0];
        hybrid_left_real += 32;
        temp_left_imag = hybrid_left_real[-16];
        temp_right_real = hybrid_right_real[0];
        hybrid_right_real += 32;
        temp_right_imag = hybrid_right_real[-16];

        temp_left_real_1 = temp_right_real + temp_left_real;
        temp_left_imag_1 = temp_right_imag + temp_left_imag;

        temp2 = temp_left_real_1 * temp_left_real_1;
        temp2 += temp_left_imag_1 * temp_left_imag_1;
        temp2 /= 2.0f;

        temp = temp_left_real * temp_right_real;
        temp += temp_left_imag * temp_right_imag;

        temp = temp2 - temp;

        if ((temp / 8.0f) >= temp2) {
          temp_left_real = temp_left_real_1 * 2.0f;
          temp_left_imag = temp_left_imag_1 * 2.0f;
        } else {
          temp = (FLOAT32)sqrt(temp / (temp2 * 2.0f));
          temp_left_real = temp_left_real_1 * temp;
          temp_left_imag = temp_left_imag_1 * temp;
        }

        hybrid_left_real[-32] = temp_left_real;
        hybrid_left_real[-16] = temp_left_imag;
      }
    }
  }

  for (; group < NUMBER_OF_IPD_GROUPS; group++) {
    FLOAT32 *hybrid_left_real;
    FLOAT32 *qmf_lre = qmf_left_real[NUMBER_OF_SUBSAMPLES - 1];

    subband_grp = ps_tables->grp_borders_mix[group];
    max_subband = ps_tables->grp_borders_mix[group + 1];

    hybrid_left_real =
        (qmf_left_real - IXHEAACE_HYBRID_FILTER_DELAY)[NUMBER_OF_SUBSAMPLES - 1] + 0X1000;
    hybrid_left_real += subband_grp;
    qmf_lre += subband_grp;

    for (i = NUMBER_OF_SUBSAMPLES - 1; i >= IXHEAACE_HYBRID_FILTER_DELAY; i--) {
      for (subband = max_subband - subband_grp; subband != 0; subband--) {
        FLOAT32 temp_left_real_1, temp_left_imag_1;

        temp_left_real = hybrid_left_real[-0X1000];
        temp_left_imag = hybrid_left_real[0];
        temp_right_real = hybrid_left_real[-0X800];
        temp_right_imag = hybrid_left_real[0X800];

        hybrid_left_real += 1;

        temp_left_real_1 = temp_right_real + temp_left_real;
        temp_left_imag_1 = temp_right_imag + temp_left_imag;

        temp2 = temp_left_real_1 * temp_left_real_1;
        temp2 += temp_left_imag_1 * temp_left_imag_1;
        temp2 /= 2.0f;

        temp = temp_left_real * temp_right_real;
        temp += temp_left_imag * temp_right_imag;

        temp = temp2 - temp;

        if ((temp / 8.0f) >= temp2) {
          temp_left_real = temp_left_real_1 * 2.0f;
          temp_left_imag = temp_left_imag_1 * 2.0f;
        } else {
          temp = (FLOAT32)sqrt(temp / (2.0f * temp2));
          temp_left_real = temp_left_real_1 * temp;
          temp_left_imag = temp_left_imag_1 * temp;
        }

        qmf_lre[0] = temp_left_real;
        qmf_lre[0 + 0x1000] = temp_left_imag;
        qmf_lre += 1;
      }

      hybrid_left_real -= max_subband - subband_grp;
      qmf_lre -= max_subband - subband_grp;
      qmf_lre -= 64;
      hybrid_left_real -= 64;
    }

    hybrid_left_real = pms->hist_qmf_left_real[IXHEAACE_HYBRID_FILTER_DELAY - 1];
    hybrid_left_real += subband_grp;

    for (i = IXHEAACE_HYBRID_FILTER_DELAY - 1; i >= 0; i--) {
      for (subband = max_subband - subband_grp; subband != 0; subband--) {
        FLOAT32 temp_left_real_1, temp_left_imag_1;

        temp_left_real = hybrid_left_real[0];
        temp_left_imag = hybrid_left_real[0 + 0x40];
        temp_right_real = hybrid_left_real[0 + 0x80];
        temp_right_imag = hybrid_left_real[0 + 0xc0];

        temp_left_real_1 = temp_right_real + temp_left_real;
        temp_left_imag_1 = temp_right_imag + temp_left_imag;

        temp2 = temp_left_real_1 * temp_left_real_1;
        temp2 += temp_left_imag_1 * temp_left_imag_1;
        temp2 /= 2.0f;

        temp = temp_left_real * temp_right_real;
        temp += temp_left_imag * temp_right_imag;

        temp = temp2 - temp;

        if ((temp / 8.0f) >= temp2) {
          temp_left_real = temp_left_real_1 * 2.0f;
          temp_left_imag = temp_left_imag_1 * 2.0f;
        } else {
          temp = (FLOAT32)sqrt(temp / (2.0f * temp2));
          temp_left_real = temp_left_real_1 * temp;
          temp_left_imag = temp_left_imag_1 * temp;
        }

        qmf_lre[0] = temp_left_real;
        qmf_lre[0x1000] = temp_left_imag;
        qmf_lre += 1;
        hybrid_left_real += 1;
      }
      qmf_lre -= max_subband - subband_grp;
      hybrid_left_real -= max_subband - subband_grp;
      qmf_lre -= 64;
      hybrid_left_real -= 0x100;
    }
  }

  for (i = 0; i < IXHEAACE_HYBRID_FILTER_DELAY; i++) {
    ixheaace_memcpy(pms->hist_qmf_left_real[i], pms->temp_qmf_left_real[i],
                    NUMBER_OF_QMF_BANDS * sizeof(**qmf_left_real));
    ixheaace_memcpy(pms->hist_qmf_left_imag[i], pms->temp_qmf_left_imag[i],
                    NUMBER_OF_QMF_BANDS * sizeof(**qmf_left_imag));
    ixheaace_memcpy(pms->hist_qmf_right_real[i],
                    qmf_right_real[NUMBER_OF_SUBSAMPLES - IXHEAACE_HYBRID_FILTER_DELAY + i],
                    NUMBER_OF_QMF_BANDS * sizeof(**qmf_right_real));
    ixheaace_memcpy(pms->hist_qmf_right_imag[i],
                    qmf_right_imag[NUMBER_OF_SUBSAMPLES - IXHEAACE_HYBRID_FILTER_DELAY + i],
                    NUMBER_OF_QMF_BANDS * sizeof(**qmf_right_imag));
  }

  ixheaace_hybrid_synthesis((const FLOAT32 **)pms->m_hybrid_real_left,
                            (const FLOAT32 **)pms->m_hybrid_imag_left, qmf_left_real,
                            qmf_left_imag, ps_tables->a_hyb_res);
}

IA_ERRORCODE ixheaace_encode_ps_frame(ixheaace_pstr_ps_enc pms, FLOAT32 **i_buffer_left,
                                      FLOAT32 **r_buffer_left, FLOAT32 **i_buffer_right,
                                      FLOAT32 **r_buffer_right, ixheaace_str_ps_tab *ps_tables,
                                      ixheaace_comm_tables *common_tab) {
  IA_ERRORCODE err_code;
  WORD32 i;
  WORD32 bin;
  WORD32 subband, max_subband;

  FLOAT32 band_hist_power_left;
  FLOAT32 band_hist_power_right;
  FLOAT32 band_hist_power_corr_real;
  FLOAT32 band_hist_power_corr_imag;

  FLOAT32 band_power_left_env1;
  FLOAT32 band_power_right_env1;
  FLOAT32 band_power_corr_real_env1;
  FLOAT32 band_power_corr_imag_env1;
  FLOAT32 scratch_power_left_right_env2[2 * NUMBER_OF_BINS];
  FLOAT32 scratch_power_corr_real_imag_env2[2 * NUMBER_OF_BINS];

  FLOAT32 **hybrid_left_imag_env1;
  FLOAT32 **hybrid_left_real_env1;
  FLOAT32 **hybrid_right_imag_env1;
  FLOAT32 **hybrid_right_real_env1;

  FLOAT32 **hybrid_left_imag_env2;
  FLOAT32 **hybrid_left_real_env2;
  FLOAT32 **hybr_right_imag_env2;
  FLOAT32 **hybr_right_real_env2;

  FLOAT32 **hist_left_imag;
  FLOAT32 **hist_left_real;
  FLOAT32 **hist_right_imag;
  FLOAT32 **hist_right_real;

  FLOAT32 temp1;
  FLOAT32 temp2;

  FLOAT32 temp_hist_pow_left;
  FLOAT32 temp_hist_pow_right;
  FLOAT32 temp_hist_pow_corr_real;
  FLOAT32 temp_hist_pow_corr_imag;

  FLOAT32 temp_power_left_env1;
  FLOAT32 temp_power_right_env1;
  FLOAT32 temp_power_corr_real_env1;
  FLOAT32 temp_power_corr_imag_env1;

  FLOAT32 temp_power_left_env2;
  FLOAT32 temp_power_right_env2;
  FLOAT32 temp_power_corr_real_env2;
  FLOAT32 temp_power_corr_imag_env2;

  err_code = ixheaace_hybrid_analysis((const FLOAT32 **)(r_buffer_left),
                                      (const FLOAT32 **)(i_buffer_left), pms->m_hybrid_real_left,
                                      pms->m_hybrid_imag_left, pms->ptr_hybrid_left, ps_tables,
                                      common_tab->pstr_common_tab);
  if (err_code != IA_NO_ERROR) {
    return err_code;
  }

  err_code = ixheaace_hybrid_analysis(
      (const FLOAT32 **)(r_buffer_right), (const FLOAT32 **)(i_buffer_right),
      pms->m_hybrid_real_right, pms->m_hybrid_imag_right, pms->ptr_hybrid_right, ps_tables,
      common_tab->pstr_common_tab);
  if (err_code != IA_NO_ERROR) {
    return err_code;
  }

  {
    hybrid_left_real_env1 = pms->m_hybrid_real_left;
    hybrid_left_imag_env1 = pms->m_hybrid_imag_left;
    hybrid_right_real_env1 = pms->m_hybrid_real_right;
    hybrid_right_imag_env1 = pms->m_hybrid_imag_right;

    hybrid_left_real_env2 = &(pms->m_hybrid_real_left[NUMBER_OF_SUBSAMPLES / 2]);
    hybrid_left_imag_env2 = &(pms->m_hybrid_imag_left[NUMBER_OF_SUBSAMPLES / 2]);
    hybr_right_real_env2 = &(pms->m_hybrid_real_right[NUMBER_OF_SUBSAMPLES / 2]);
    hybr_right_imag_env2 = &(pms->m_hybrid_imag_right[NUMBER_OF_SUBSAMPLES / 2]);

    for (bin = 0; bin < SUBQMF_BINS_ENERGY; bin++) {
      band_power_left_env1 = 0;
      band_power_right_env1 = 0;
      band_power_corr_real_env1 = 0;
      band_power_corr_imag_env1 = 0;

      temp_power_left_env2 = 0;
      temp_power_right_env2 = 0;
      temp_power_corr_real_env2 = 0;
      temp_power_corr_imag_env2 = 0;

      max_subband = ps_tables->hi_res_band_borders[bin] + 1;
      for (subband = ps_tables->hi_res_band_borders[bin]; subband < max_subband; subband++) {
        FLOAT32 t_left_real, t_left_imag, t_right_real, t_right_imag;

        for (i = 0; i < NUMBER_OF_SUBSAMPLES / 2; i++) {
          // Envelope 1
          t_left_real = hybrid_left_real_env1[i][subband];
          t_right_real = hybrid_right_real_env1[i][subband];
          t_left_imag = hybrid_left_imag_env1[i][subband];
          t_right_imag = hybrid_right_imag_env1[i][subband];

          temp1 = t_left_real * t_left_real;
          temp2 = t_left_imag * t_left_imag;
          band_power_left_env1 += (temp1 + temp2);

          temp1 = t_right_real * t_right_real;
          temp2 = t_right_imag * t_right_imag;
          band_power_right_env1 += (temp1 + temp2);

          temp1 = t_left_real * t_right_real;
          temp2 = t_left_imag * t_right_imag;
          band_power_corr_real_env1 += (temp1 + temp2);

          temp1 = t_left_imag * t_right_real;
          temp2 = t_left_real * t_right_imag;
          band_power_corr_imag_env1 += (temp1 - temp2);

          // Envelope 2
          t_left_real = hybrid_left_real_env2[i][subband];
          t_right_real = hybr_right_real_env2[i][subband];
          t_left_imag = hybrid_left_imag_env2[i][subband];
          t_right_imag = hybr_right_imag_env2[i][subband];

          temp1 = t_left_real * t_left_real;
          temp2 = t_left_imag * t_left_imag;
          temp_power_left_env2 += (temp1 + temp2);

          temp1 = t_right_real * t_right_real;
          temp2 = t_right_imag * t_right_imag;
          temp_power_right_env2 += (temp1 + temp2);

          temp1 = t_left_real * t_right_real;
          temp2 = t_left_imag * t_right_imag;
          temp_power_corr_real_env2 += (temp1 + temp2);

          temp1 = t_left_imag * t_right_real;
          temp2 = t_left_real * t_right_imag;
          temp_power_corr_imag_env2 += (temp1 - temp2);
        }
      }

      scratch_power_left_right_env2[2 * bin] = temp_power_left_env2;
      scratch_power_left_right_env2[2 * bin + 1] = temp_power_right_env2;
      scratch_power_corr_real_imag_env2[2 * bin] = temp_power_corr_real_env2;
      scratch_power_corr_real_imag_env2[2 * bin + 1] = temp_power_corr_imag_env2;

      pms->pow_left_right[2 * bin] = pms->pow_left_right[2 * bin] + band_power_left_env1;
      pms->pow_left_right[2 * bin + 1] = pms->pow_left_right[2 * bin + 1] + band_power_right_env1;
      pms->pow_corr_real_imag[2 * bin] =
          pms->pow_corr_real_imag[2 * bin] + band_power_corr_real_env1;
      pms->pow_corr_real_imag[2 * bin + 1] =
          pms->pow_corr_real_imag[2 * bin + 1] + band_power_corr_imag_env1;
    }

    hist_left_real = pms->hist_qmf_left_real;
    hist_left_imag = pms->hist_qmf_left_imag;
    hist_right_real = pms->hist_qmf_right_real;
    hist_right_imag = pms->hist_qmf_right_imag;

    hybrid_left_real_env1 = r_buffer_left;
    hybrid_left_imag_env1 = i_buffer_left;
    hybrid_right_real_env1 = r_buffer_right;
    hybrid_right_imag_env1 = i_buffer_right;

    hybrid_left_real_env2 =
        &r_buffer_left[NUMBER_OF_SUBSAMPLES / 2 - IXHEAACE_HYBRID_FILTER_DELAY];
    hybrid_left_imag_env2 =
        &i_buffer_left[NUMBER_OF_SUBSAMPLES / 2 - IXHEAACE_HYBRID_FILTER_DELAY];
    hybr_right_real_env2 =
        &r_buffer_right[NUMBER_OF_SUBSAMPLES / 2 - IXHEAACE_HYBRID_FILTER_DELAY];
    hybr_right_imag_env2 =
        &i_buffer_right[NUMBER_OF_SUBSAMPLES / 2 - IXHEAACE_HYBRID_FILTER_DELAY];

    for (bin = SUBQMF_BINS_ENERGY; bin < NUMBER_OF_BINS; bin++) {
      band_power_left_env1 = 0;
      band_power_right_env1 = 0;
      band_power_corr_real_env1 = 0;
      band_power_corr_imag_env1 = 0;

      scratch_power_left_right_env2[2 * bin] = 0;
      scratch_power_left_right_env2[2 * bin + 1] = 0;
      scratch_power_corr_real_imag_env2[2 * bin] = 0;
      scratch_power_corr_real_imag_env2[2 * bin + 1] = 0;

      band_hist_power_left = 0;
      band_hist_power_right = 0;
      band_hist_power_corr_real = 0;
      band_hist_power_corr_imag = 0;

      for (subband = ps_tables->hi_res_band_borders[bin];
           subband < ps_tables->hi_res_band_borders[bin + 1]; subband++) {
        FLOAT32 t_left_real, t_left_imag, t_right_real, t_right_imag;
        temp_hist_pow_left = 0;
        temp_hist_pow_right = 0;
        temp_hist_pow_corr_real = 0;
        temp_hist_pow_corr_imag = 0;

        temp_power_left_env1 = 0;
        temp_power_right_env1 = 0;
        temp_power_corr_real_env1 = 0;
        temp_power_corr_imag_env1 = 0;

        temp_power_left_env2 = 0;
        temp_power_right_env2 = 0;
        temp_power_corr_real_env2 = 0;
        temp_power_corr_imag_env2 = 0;

        for (i = 0; i < 6; i++) {
          t_left_real = hist_left_real[i][subband];
          t_left_imag = hist_left_imag[i][subband];
          t_right_real = hist_right_real[i][subband];
          t_right_imag = hist_right_imag[i][subband];

          temp1 = t_left_real * t_left_real;
          temp2 = t_left_imag * t_left_imag;
          temp_hist_pow_left += (temp1 + temp2);

          temp1 = t_right_real * t_right_real;
          temp2 = t_right_imag * t_right_imag;
          temp_hist_pow_right += (temp1 + temp2);

          temp1 = t_left_real * t_right_real;
          temp2 = t_left_imag * t_right_imag;
          temp_hist_pow_corr_real += (temp1 + temp2);

          temp1 = t_left_imag * t_right_real;
          temp2 = t_left_real * t_right_imag;
          temp_hist_pow_corr_imag += (temp1 - temp2);

          t_left_real = hybrid_left_real_env1[i][subband];
          t_left_imag = hybrid_left_imag_env1[i][subband];
          t_right_real = hybrid_right_real_env1[i][subband];
          t_right_imag = hybrid_right_imag_env1[i][subband];

          temp1 = t_left_real * t_left_real;
          temp2 = t_left_imag * t_left_imag;
          temp_power_left_env1 += (temp1 + temp2);

          temp1 = t_right_real * t_right_real;
          temp2 = t_right_imag * t_right_imag;
          temp_power_right_env1 += (temp1 + temp2);

          temp1 = t_left_real * t_right_real;
          temp2 = t_left_imag * t_right_imag;
          temp_power_corr_real_env1 += (temp1 + temp2);

          temp1 = t_left_imag * t_right_real;
          temp2 = t_left_real * t_right_imag;
          temp_power_corr_imag_env1 += (temp1 - temp2);

          t_left_real = hybrid_left_real_env2[i][subband];
          t_left_imag = hybrid_left_imag_env2[i][subband];
          t_right_real = hybr_right_real_env2[i][subband];
          t_right_imag = hybr_right_imag_env2[i][subband];

          temp1 = t_left_real * t_left_real;
          temp2 = t_left_imag * t_left_imag;
          temp_power_left_env2 += (temp1 + temp2);

          temp1 = t_right_real * t_right_real;
          temp2 = t_right_imag * t_right_imag;
          temp_power_right_env2 += (temp1 + temp2);

          temp1 = t_left_real * t_right_real;
          temp2 = t_left_imag * t_right_imag;
          temp_power_corr_real_env2 += (temp1 + temp2);

          temp1 = t_left_imag * t_right_real;
          temp2 = t_left_real * t_right_imag;
          temp_power_corr_imag_env2 += (temp1 - temp2);
        }

        for (i = 6; i < 10; i++) {
          t_left_real = hybrid_left_real_env1[i][subband];
          t_left_imag = hybrid_left_imag_env1[i][subband];
          t_right_real = hybrid_right_real_env1[i][subband];
          t_right_imag = hybrid_right_imag_env1[i][subband];

          temp1 = t_left_real * t_left_real;
          temp2 = t_left_imag * t_left_imag;
          temp_power_left_env1 += (temp1 + temp2);

          temp1 = t_right_real * t_right_real;
          temp2 = t_right_imag * t_right_imag;
          temp_power_right_env1 += (temp1 + temp2);

          temp1 = t_left_real * t_right_real;
          temp2 = t_left_imag * t_right_imag;
          temp_power_corr_real_env1 += (temp1 + temp2);

          temp1 = t_left_imag * t_right_real;
          temp2 = t_left_real * t_right_imag;
          temp_power_corr_imag_env1 += (temp1 - temp2);

          t_left_real = hybrid_left_real_env2[i][subband];
          t_left_imag = hybrid_left_imag_env2[i][subband];
          t_right_real = hybr_right_real_env2[i][subband];
          t_right_imag = hybr_right_imag_env2[i][subband];

          temp1 = t_left_real * t_left_real;
          temp2 = t_left_imag * t_left_imag;
          temp_power_left_env2 += (temp1 + temp2);

          temp1 = t_right_real * t_right_real;
          temp2 = t_right_imag * t_right_imag;
          temp_power_right_env2 += (temp1 + temp2);

          temp1 = t_left_real * t_right_real;
          temp2 = t_left_imag * t_right_imag;
          temp_power_corr_real_env2 += (temp1 + temp2);

          temp1 = t_left_imag * t_right_real;
          temp2 = t_left_real * t_right_imag;
          temp_power_corr_imag_env2 += (temp1 - temp2);
        }

        for (i = 10; i < 16; i++) {
          t_left_real = hybrid_left_real_env2[i][subband];
          t_left_imag = hybrid_left_imag_env2[i][subband];
          t_right_real = hybr_right_real_env2[i][subband];
          t_right_imag = hybr_right_imag_env2[i][subband];

          temp1 = t_left_real * t_left_real;
          temp2 = t_left_imag * t_left_imag;
          temp_power_left_env2 += (temp1 + temp2);

          temp1 = t_right_real * t_right_real;
          temp2 = t_right_imag * t_right_imag;
          temp_power_right_env2 += (temp1 + temp2);

          temp1 = t_left_real * t_right_real;
          temp2 = t_left_imag * t_right_imag;
          temp_power_corr_real_env2 += (temp1 + temp2);

          temp1 = t_left_imag * t_right_real;
          temp2 = t_left_real * t_right_imag;
          temp_power_corr_imag_env2 += (temp1 - temp2);
        }

        scratch_power_left_right_env2[2 * bin] =
            scratch_power_left_right_env2[2 * bin] + temp_power_left_env2;
        scratch_power_left_right_env2[2 * bin + 1] =
            scratch_power_left_right_env2[2 * bin + 1] + temp_power_right_env2;
        scratch_power_corr_real_imag_env2[2 * bin] =
            scratch_power_corr_real_imag_env2[2 * bin] + temp_power_corr_real_env2;
        scratch_power_corr_real_imag_env2[2 * bin + 1] =
            scratch_power_corr_real_imag_env2[2 * bin + 1] + temp_power_corr_imag_env2;

        band_power_left_env1 = band_power_left_env1 + temp_power_left_env1;
        band_power_right_env1 = band_power_right_env1 + temp_power_right_env1;
        band_power_corr_real_env1 = band_power_corr_real_env1 + temp_power_corr_real_env1;
        band_power_corr_imag_env1 = band_power_corr_imag_env1 + temp_power_corr_imag_env1;

        band_hist_power_left = band_hist_power_left + temp_hist_pow_left;
        band_hist_power_right = band_hist_power_right + temp_hist_pow_right;
        band_hist_power_corr_real = band_hist_power_corr_real + temp_hist_pow_corr_real;
        band_hist_power_corr_imag = band_hist_power_corr_imag + temp_hist_pow_corr_imag;
      }

      pms->pow_left_right[2 * bin] += (band_power_left_env1 + band_hist_power_left);
      pms->pow_left_right[2 * bin + 1] += (band_power_right_env1 + band_hist_power_right);
      pms->pow_corr_real_imag[2 * bin] += (band_power_corr_real_env1 + band_hist_power_corr_real);
      pms->pow_corr_real_imag[2 * bin + 1] +=
          (band_power_corr_imag_env1 + band_hist_power_corr_imag);
    }
  }

  {
    FLOAT32 temp_left;
    FLOAT32 temp_right;
    FLOAT32 temp_corr_re;
    FLOAT32 temp_corr_im;

    for (bin = 0; bin < pms->iid_icc_bins; bin++) {
      if (pms->b_hi_freq_res_iid_icc) {
        temp_left = pms->pow_left_right[2 * bin];
        temp_right = pms->pow_left_right[2 * bin + 1];
        temp_corr_re = pms->pow_corr_real_imag[2 * bin];
        temp_corr_im = pms->pow_corr_real_imag[2 * bin + 1];
      } else {
        temp_left = pms->pow_left_right[2 * 2 * bin] + pms->pow_left_right[2 * 2 * bin + 2];
        temp_right =
            pms->pow_left_right[2 * 2 * bin + 1] + pms->pow_left_right[2 * (2 * bin + 1) + 1];
        temp_corr_re =
            pms->pow_corr_real_imag[2 * 2 * bin] + pms->pow_corr_real_imag[2 * 2 * bin + 2];
        temp_corr_im = pms->pow_corr_real_imag[2 * 2 * bin + 1] +
                       pms->pow_corr_real_imag[2 * (2 * bin + 1) + 1];
      }

      if (temp_left == 0) {
        temp_left = 0.0625f;
      }
      if (temp_right == 0) {
        temp_right = 0.0625f;
      }
      if (temp_corr_re == 0) {
        temp_corr_re = 0.0625f;
      }
      if (temp_corr_im == 0) {
        temp_corr_im = 0.0625f;
      }

      pms->aaa_ICC_data_buf[bin][1] = pms->aaa_ICC_data_buf[bin][0];
      if (bin > NUMBER_OF_IPD_BINS) {
        temp1 = temp_corr_re * temp_corr_re;
        temp2 = temp_corr_im * temp_corr_im;
        temp_corr_re = temp1 + temp2;

        temp1 = temp_left * temp_right;
        pms->aaa_ICC_data_buf[bin][0] = (FLOAT32)sqrt(temp_corr_re / temp1);
      } else {
        temp1 = temp_left * temp_right;
        pms->aaa_ICC_data_buf[bin][0] = temp_corr_re / (FLOAT32)sqrt(temp1);
      }
      if (pms->aaa_ICC_data_buf[bin][0] > 1.0f) {
        pms->aaa_ICC_data_buf[bin][0] = 0;
      } else {
        pms->aaa_ICC_data_buf[bin][0] =
            (FLOAT32)sqrt(0.5f - (pms->aaa_ICC_data_buf[bin][0] / 2.0f));
      }
      temp1 = temp_left / temp_right;
      temp1 = (FLOAT32)sqrt(temp1);
      pms->aaa_IID_data_buf[bin][1] = pms->aaa_IID_data_buf[bin][0];
      pms->aaa_IID_data_buf[bin][0] = SBR_INV_LOG_2 * (FLOAT32)log(temp1);
    }
  }
  {
    FLOAT32 *pow_left = &pms->pow_left_right[0];
    FLOAT32 *corr_real = &pms->pow_corr_real_imag[0];

    memcpy(pow_left, scratch_power_left_right_env2, 2 * NUMBER_OF_BINS * sizeof(FLOAT32));
    memcpy(corr_real, scratch_power_corr_real_imag_env2, 2 * NUMBER_OF_BINS * sizeof(FLOAT32));
  }

  ia_enhaacplus_enc_downmix_to_mono(pms, r_buffer_left, i_buffer_left, r_buffer_right,
                                    i_buffer_right, ps_tables);
  return IA_NO_ERROR;
}
