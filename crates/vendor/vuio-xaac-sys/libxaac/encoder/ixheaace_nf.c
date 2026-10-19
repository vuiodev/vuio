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

#include <float.h>
#include <string.h>
#include <math.h>
#include "iusace_cnst.h"
#include "iusace_type_def.h"
#include "ixheaac_constants.h"
#include "iusace_bitbuffer.h"
#include "iusace_tns_usac.h"
#include "iusace_fd_quant.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaac_basic_ops.h"
#include "ixheaace_nf.h"

static VOID iusace_noise_filling_limiter(FLOAT64 *energy, FLOAT64 *ptr_spec,
                                         WORD32 *ptr_quant_spec, WORD32 n0_by_4,
                                         WORD32 *ptr_sfb_offset, WORD32 sb, WORD32 cntr,
                                         FLOAT64 *ptr_highest_tone) {
  WORD32 n, i;
  FLOAT64 tone_energy;
  FLOAT64 tot_tone_energy = 0.0;

  if (!n0_by_4) return;
  if (cntr <= n0_by_4) return;

  memset(ptr_highest_tone, 0, n0_by_4 * sizeof(*ptr_highest_tone));

  /* finds the n0_by_4 strongest bins */
  for (i = ptr_sfb_offset[sb]; i < ptr_sfb_offset[sb + 1]; i++) {
    if (!ptr_quant_spec[i]) {
      tone_energy = ptr_spec[i] * ptr_spec[i];

      for (n = 0; n < n0_by_4; n++) {
        if (tone_energy > ptr_highest_tone[n]) {
          memmove(ptr_highest_tone + 1 + n, ptr_highest_tone + n,
                  (n0_by_4 - n - 1) * sizeof(*ptr_highest_tone));
          ptr_highest_tone[n] = tone_energy;
          break;
        }
      }
    }
  }
  /* remove the contribution of the highest_tone components */
  for (n = 0; n < n0_by_4; n++) tot_tone_energy += ptr_highest_tone[n];

  FLOAT64 diff = *energy - tot_tone_energy;
  //If the difference is within 1% of total energy, no need to send any energy
  if (diff < 0.01*(*energy))
  {
    *energy = 0.0;
  }
  else
  {
    *energy = diff;
  }

  /* add the average component energy */
  *energy += n0_by_4 * (*energy) / (cntr - n0_by_4);
  return;
}

VOID iusace_noise_filling(WORD32 *noise_level, WORD32 *noise_offset, FLOAT64 *ptr_quant_spec,
                          ia_usac_quant_info_struct *pstr_quant_info, WORD32 *ptr_sfb_offset,
                          WORD32 max_sfb, WORD32 window_size_samples, WORD32 num_window_groups,
                          const WORD32 *ptr_window_group_length,
                          WORD32 noise_filling_start_offset, FLOAT64 *ptr_scratch_buf) {
  FLOAT64 energy;
  FLOAT64 noise_level_temp;
  FLOAT64 noise_offset_temp;

  FLOAT64 sum_sfb_on, sum_sfb_off;
  FLOAT64 e_sfb_on, e_sfb_off;

  WORD32 n0;
  WORD32 start_sfb, sfb, i;
  WORD32 band_quantized_to_zero;

  FLOAT64 alpha = 0.15; /* prudence factor */
  WORD32 grp = 0;

  e_sfb_on = 1e-6;
  e_sfb_off = 1e-6;

  sum_sfb_on = 1e-6;
  sum_sfb_off = 1e-6;

  *noise_offset = 0;
  *noise_level = 0;

  for (sfb = 0; sfb < max_sfb; sfb++) {
    if (ptr_sfb_offset[sfb + 1] > noise_filling_start_offset) break;
  }
  start_sfb = sfb;
  for (grp = 0; grp < num_window_groups; grp++) {
    WORD32 grp_win = 0;
    for (sfb = start_sfb; sfb < max_sfb; sfb++) {
      band_quantized_to_zero = 1;
      for (grp_win = 0; grp_win < ptr_window_group_length[grp]; grp_win++) {
        WORD32 offset = grp_win * window_size_samples;
        energy = 0;
        n0 = 0;
        for (i = ptr_sfb_offset[sfb]; i < ptr_sfb_offset[sfb + 1]; i++) {
          /* calculate energy if the quantized value is non zero */
          if (!pstr_quant_info->quant_degroup[offset + i]) {
            energy += ptr_quant_spec[offset + i] * ptr_quant_spec[offset + i];
            n0++;
          } else {
            /* All quantized values are not zero */
            band_quantized_to_zero = 0;
          }
        }

        /* Remove highest (tonal) contributions */
        iusace_noise_filling_limiter(&energy, &ptr_quant_spec[offset],
                                     &pstr_quant_info->quant_degroup[offset], n0 / 4,
                                     ptr_sfb_offset, sfb, n0, ptr_scratch_buf);

        if (band_quantized_to_zero == 0) {
          e_sfb_on += energy;
          sum_sfb_on += pow(2., 0.5 * pstr_quant_info->scale_factor[sfb] - 50) * n0;
        } else
        /* subband is completely zeroed  */
        {
          e_sfb_off += energy;
          sum_sfb_off += pow(2., 0.5 * pstr_quant_info->scale_factor[sfb] - 58) *
                         (ptr_sfb_offset[sfb + 1] - ptr_sfb_offset[sfb]);
        }
      }
    }
  }

  if (num_window_groups > 1) alpha = alpha * 0.15;

  if (sum_sfb_on) {
    noise_level_temp = 1.5 * (log(alpha * e_sfb_on) - log(sum_sfb_on)) / log(2.0) + 14.0;

    /* quantize to nearest integer */
    *noise_level = (WORD32)(noise_level_temp + 0.5);

    /* noise level limited to quantization range [0,7] */
    *noise_level = MAX(*noise_level, 0);
    *noise_level = MIN(*noise_level, 7);

    if (*noise_level != 0) {
      noise_offset_temp =
          2. * log(alpha * e_sfb_off * sum_sfb_on / sum_sfb_off / e_sfb_on) / log(2.);

      /* quantize to nearest integer */
      *noise_offset = (WORD32)(noise_offset_temp + 0.5);

      /* noise offset limited to quantization range [0,31] */
      *noise_level = *noise_offset <= 0 ? 0 : *noise_level;
      *noise_offset = MIN(*noise_offset, 31);
      *noise_offset = MAX(*noise_offset, 0);
    }
  }
  return;
}
