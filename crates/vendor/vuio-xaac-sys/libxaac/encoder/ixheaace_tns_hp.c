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
#include <stddef.h>
#include "ixheaac_type_def.h"
#include "ixheaac_constants.h"
#include "impd_drc_common_enc.h"
#include "impd_drc_uni_drc.h"
#include "impd_drc_tables.h"
#include "impd_drc_api.h"
#include "ixheaace_api.h"
#include "ixheaace_aac_constants.h"
#include "ixheaace_common_rom.h"
#include "ixheaace_psy_const.h"
#include "ixheaace_tns.h"
#include "ixheaace_tns_params.h"
#include "ixheaace_rom.h"
#include "ixheaace_bitbuffer.h"
#include "ixheaace_psy_configuration.h"
#include "ixheaace_tns_func.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops16.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaac_basic_ops.h"

VOID ia_enhaacplus_enc_calc_weighted_spectrum(FLOAT32 *ptr_spectrum, FLOAT32 *ptr_weighted_spec,
                                              FLOAT32 *ptr_sfb_energy,
                                              const WORD32 *ptr_sfb_offset, WORD32 lpc_start_line,
                                              WORD32 lpc_stop_line, WORD32 lpc_start_band,
                                              WORD32 lpc_stop_band, FLOAT32 *ptr_shared_buffer1,
                                              WORD32 aot) {
  WORD32 i, sfb, tempcnt;
  FLOAT32 tmp;
  FLOAT32 *ptr_tns_sfb_mean = ptr_shared_buffer1;
  FLOAT32 temp1, temp2;
  FLOAT32 *ptr_spec;
  FLOAT32 *ptr_ws1;
  WORD sfb_width, j;

  for (sfb = lpc_start_band; sfb < lpc_stop_band; sfb++) {
    FLOAT32 sfb_nrg_tmp = ptr_sfb_energy[sfb];
    ptr_tns_sfb_mean[sfb] = 1 / ((FLOAT32)sqrt(sfb_nrg_tmp) + 1e-30f);
  }

  sfb = lpc_start_band;

  tmp = ptr_tns_sfb_mean[sfb];

  for (i = lpc_start_line; i < lpc_stop_line; i += sfb_width) {
    ptr_spec = &ptr_weighted_spec[i];
    WORD start = i, stop = ptr_sfb_offset[sfb + 1];

    stop = MIN(stop, lpc_stop_line);
    sfb_width = stop - start;

    for (j = (sfb_width >> 1) - 1; j >= 0; j--) {
      *ptr_spec++ = tmp;
      *ptr_spec++ = tmp;
    }
    sfb++;

    if ((sfb + 1) < lpc_stop_band) {
      tmp = ptr_tns_sfb_mean[sfb];
    }
  }

  /* Filter down */
  if (aot == AOT_AAC_LC || aot == AOT_SBR || aot == AOT_PS || aot == AOT_AAC_ELD) {
    for (i = lpc_stop_line - 2; i >= lpc_start_line; i--) {
      ptr_weighted_spec[i] = (ptr_weighted_spec[i] + ptr_weighted_spec[i + 1]) * 0.5f;
    }
    for (i = lpc_start_line + 1; i < lpc_stop_line; i++) {
      ptr_weighted_spec[i] = (ptr_weighted_spec[i] + ptr_weighted_spec[i - 1]) * 0.5f;
    }

    /* Weight and normalize */
    for (i = lpc_start_line; i < lpc_stop_line; i++) {
      ptr_weighted_spec[i] = ptr_weighted_spec[i] * ptr_spectrum[i];
    }
  } else if (aot == AOT_AAC_LD || aot == AOT_AAC_ELD) {
    WORD32 remaining;
    FLOAT32 multout_temp;

    ptr_ws1 = &ptr_weighted_spec[lpc_stop_line - 1];
    tempcnt = (lpc_stop_line - lpc_start_line) >> 2;
    remaining = lpc_stop_line - lpc_start_line - (tempcnt << 2);

    temp1 = *ptr_ws1--;
    temp2 = (*ptr_ws1 + temp1);
    for (i = tempcnt - 1; i >= 0; i--) {
      *ptr_ws1-- = temp2;
      temp1 = (*ptr_ws1 + temp2 * 0.5f);
      *ptr_ws1-- = temp1;
      temp2 = (*ptr_ws1 + temp1 * 0.5f);
      *ptr_ws1-- = temp2;
      temp1 = (*ptr_ws1 + temp2 * 0.5f);
      *ptr_ws1-- = temp1;
      temp2 = (*ptr_ws1 + temp1 * 0.5f);
    }
    ptr_ws1++;
    if (remaining) {
      for (i = remaining - 1; i >= 0; i--) {
        temp1 = *ptr_ws1--;
        *ptr_ws1 = (*ptr_ws1 + temp1 * 0.5f);
      }
    }

    ptr_weighted_spec[lpc_start_line + 1] = (FLOAT32)(
        ((ptr_weighted_spec[lpc_start_line + 1]) + (ptr_weighted_spec[lpc_start_line])) * 0.5f);
    multout_temp = (ptr_weighted_spec[lpc_start_line] * ptr_spectrum[lpc_start_line]);
    ptr_weighted_spec[lpc_start_line] = multout_temp;

    /* Weight and normalize */
    ptr_spec = &ptr_spectrum[lpc_start_line + 1];
    ptr_ws1 = &ptr_weighted_spec[lpc_start_line + 1];

    tempcnt = (lpc_stop_line - lpc_start_line - 2) >> 2;
    remaining = (lpc_stop_line - lpc_start_line - 2) - (tempcnt << 2);
    temp2 = *ptr_ws1;

    for (i = tempcnt - 1; i >= 0; i--) {
      temp1 = *(ptr_ws1 + 1);
      temp1 = (FLOAT32)((temp1 + temp2) * 0.5f);
      multout_temp = (temp2 * *ptr_spec++);
      *ptr_ws1++ = multout_temp;

      temp2 = *(ptr_ws1 + 1);
      temp2 = (FLOAT32)((temp2 + temp1) * 0.5f);
      multout_temp = (temp1 * *ptr_spec++);
      *ptr_ws1++ = multout_temp;

      temp1 = *(ptr_ws1 + 1);
      temp1 = (FLOAT32)((temp2 + temp1) * 0.5f);
      multout_temp = (temp2 * *ptr_spec++);
      *ptr_ws1++ = multout_temp;

      temp2 = *(ptr_ws1 + 1);
      temp2 = (FLOAT32)((temp2 + temp1) * 0.5f);
      multout_temp = (temp1 * *ptr_spec++);
      *ptr_ws1++ = multout_temp;
    }

    if (remaining) {
      for (i = remaining - 1; i >= 0; i--) {
        temp1 = *(ptr_ws1 + 1);

        multout_temp = (temp2 * *ptr_spec++);
        *ptr_ws1++ = multout_temp;
        temp2 = (FLOAT32)((temp1 + temp2) * 0.5f);
      }
    }

    multout_temp = (temp2 + ptr_spectrum[lpc_stop_line - 1]);

    ptr_weighted_spec[lpc_stop_line - 1] = multout_temp;
  }
}

VOID ia_enhaacplus_enc_auto_correlation(const FLOAT32 *ptr_input, FLOAT32 *ptr_corr,
                                        WORD32 samples, WORD32 corr_coeff) {
  WORD32 i, j;
  FLOAT32 tmp_var;
  WORD32 remaining;
  remaining = corr_coeff - ((corr_coeff >> 1) << 1);

  for (i = 0; i < samples; i += 2) {
    const FLOAT32 *ptr_input1 = &ptr_input[i];
    FLOAT32 temp1 = *ptr_input1;
    FLOAT32 temp2 = *(ptr_input1 + 1);
    FLOAT32 inp_tmp1 = *ptr_input1++;
    for (j = 0; j < (corr_coeff >> 1) << 1; j++) {
      FLOAT32 inp_tmp2;
      tmp_var = (temp1 * inp_tmp1);
      inp_tmp2 = *ptr_input1++;
      tmp_var += (temp2 * inp_tmp2);
      ptr_corr[j] += tmp_var;
      j++;
      tmp_var = (temp1 * inp_tmp2);
      inp_tmp1 = *ptr_input1++;
      tmp_var += (temp2 * inp_tmp1);
      ptr_corr[j] += (tmp_var);
    }
    if (remaining) {
      tmp_var = (temp1 * inp_tmp1);
      tmp_var += (temp2 * *ptr_input1);
      ptr_corr[j] += (tmp_var);
    }
  }
}

VOID ia_enhaacplus_enc_analysis_filter_lattice(const FLOAT32 *ptr_signal, WORD32 num_lines,
                                               const FLOAT32 *ptr_par_coeff, WORD32 order,
                                               FLOAT32 *ptr_output) {
  FLOAT32 state_par[TEMPORAL_NOISE_SHAPING_MAX_ORDER] = {0};
  WORD32 j;

  if (order <= 0) {
    return;
  }

  for (j = 0; j < num_lines; j++) {
    WORD32 i;
    FLOAT32 x = ptr_signal[j];
    FLOAT32 accu, tmp, tmp_save;

    tmp_save = x;
    accu = x;

    for (i = 0; i < order - 1; i++) {
      tmp = (accu * ptr_par_coeff[i]);

      tmp += state_par[i];

      accu += (state_par[i] * ptr_par_coeff[i]);

      state_par[i] = tmp_save;
      tmp_save = tmp;
    }

    /* last stage: only need half operations */
    accu += (state_par[order - 1] * ptr_par_coeff[order - 1]);

    state_par[order - 1] = tmp_save;

    ptr_output[j] = accu;
  }
}
