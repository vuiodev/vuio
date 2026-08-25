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
#include <stdlib.h>
#include <string.h>
#include <limits.h>

#include "ixheaac_type_def.h"
#include "ixheaac_constants.h"
#include "ixheaace_aac_constants.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops16.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaac_basic_ops.h"

#include "ixheaace_psy_const.h"
#include "ixheaace_block_switch.h"
#include "ixheaace_tns.h"
#include "ixheaace_tns_params.h"
#include "ixheaace_rom.h"
#include "ixheaace_common_rom.h"
#include "ixheaace_bitbuffer.h"
#include "ixheaace_psy_data.h"
#include "ixheaace_interface.h"
#include "ixheaace_adjust_threshold_data.h"
#include "ixheaace_dynamic_bits.h"
#include "ixheaace_qc_data.h"

#include "ixheaace_sf_estimation.h"
#include "ixheaace_quant.h"
#include "ixheaace_bits_count.h"
#include "ixheaace_tns_params.h"
#include "ixheaace_common_utils.h"

VOID iaace_calc_form_fac_per_chan(FLOAT32 *ptr_sfb_form_factor,
                                  FLOAT32 *ptr_sfb_num_relevant_lines,
                                  ixheaace_psy_out_channel *pstr_psy_out_chan,
                                  FLOAT32 *ptr_sfb_ld_energy) {
  WORD32 i, j, sfb_offs;
  WORD32 sfb, sfb_width;

  memset(ptr_sfb_num_relevant_lines, 0,
         sizeof(*ptr_sfb_num_relevant_lines) * pstr_psy_out_chan->sfb_count);

  memset(ptr_sfb_ld_energy, 0, sizeof(*ptr_sfb_ld_energy) * pstr_psy_out_chan->sfb_count);

  for (sfb_offs = 0; sfb_offs < pstr_psy_out_chan->sfb_count;
       sfb_offs += pstr_psy_out_chan->sfb_per_group) {
    i = sfb_offs;
    for (sfb = 0; sfb < pstr_psy_out_chan->max_sfb_per_grp; sfb++, i++) {
      ptr_sfb_form_factor[i] = MIN_FLT_VAL;
      if (pstr_psy_out_chan->ptr_sfb_energy[i] > pstr_psy_out_chan->ptr_sfb_thr[i]) {
        FLOAT32 avg_form_factor;

        for (j = pstr_psy_out_chan->sfb_offsets[i]; j < pstr_psy_out_chan->sfb_offsets[i + 1];
             j++) {
          ptr_sfb_form_factor[i] += (FLOAT32)sqrt(fabs(pstr_psy_out_chan->ptr_spec_coeffs[j]));
        }

        sfb_width = pstr_psy_out_chan->sfb_offsets[i + 1] - pstr_psy_out_chan->sfb_offsets[i];
        avg_form_factor =
            (FLOAT32)pow(pstr_psy_out_chan->ptr_sfb_energy[i] / (FLOAT32)sfb_width, 0.25f);
        ptr_sfb_num_relevant_lines[i] = ptr_sfb_form_factor[i] / avg_form_factor;
        ptr_sfb_ld_energy[i] = (FLOAT32)(log(pstr_psy_out_chan->ptr_sfb_energy[i]) * LOG2_1);
      }
    }
  }
}

static VOID iaace_calculate_exp_spec(const WORD32 num_lines, FLOAT32 *ptr_exp_spec,
                                     FLOAT32 *ptr_ptr_mdct_spec) {
  WORD32 line;

  for (line = 0; line < num_lines; line++) {
    FLOAT32 tmp = ptr_ptr_mdct_spec[line];
    ptr_exp_spec[line] = (FLOAT32)sqrt(fabs(tmp));
    ptr_exp_spec[line] *= (FLOAT32)sqrt(ptr_exp_spec[line]);
  }
}

static WORD32 iaace_scf_delta_bit_count(WORD32 delta) {
  if (delta > 60) {
    return (ixheaace_huffman_code_table[120][0]);
  }
  if (delta < -60) {
    return (ixheaace_huffman_code_table[0][0]);
  }
  return (ixheaace_huffman_code_table[delta + 60][0]);
}

static WORD32 iaace_count_single_scf_bits(WORD32 scf, WORD32 left_scf, WORD32 right_scf) {
  WORD32 scf_bits;

  scf_bits =
      iaace_scf_delta_bit_count(left_scf - scf) + iaace_scf_delta_bit_count(scf - right_scf);

  return scf_bits;
}

static FLOAT32 iaace_calc_single_spec_pe(WORD32 scf, FLOAT32 sfb_const_pe_part,
                                         FLOAT32 num_lines) {
  FLOAT32 spec_pe;
  FLOAT32 ld_ratio;

  ld_ratio = sfb_const_pe_part - (FLOAT32)0.375f * (FLOAT32)scf;

  if (ld_ratio >= PE_C1) {
    spec_pe = (FLOAT32)0.7f * num_lines * ld_ratio;
  } else {
    spec_pe = (FLOAT32)0.7f * num_lines * (PE_C2 + PE_C3 * ld_ratio);
  }

  return spec_pe;
}

static WORD32 iaace_count_scf_bits_diff(WORD16 *ptr_sfb_prev, WORD16 *ptr_sfb_new,
                                        WORD32 sfb_count, WORD32 start_sfb, WORD32 stop_sfb) {
  WORD32 scf_bits_diff = 0;
  WORD32 sfb = 0, sfb_last;
  WORD32 sfb_prev, sfb_next;

  sfb_last = start_sfb;

  while ((sfb_last < stop_sfb) && (ptr_sfb_prev[sfb_last] == SHRT_MIN)) {
    sfb_last++;
  }

  sfb_prev = start_sfb - 1;

  while ((sfb_prev >= 0) && (ptr_sfb_prev[sfb_prev] == SHRT_MIN)) {
    sfb_prev--;
  }

  if (sfb_prev >= 0) {
    scf_bits_diff += iaace_scf_delta_bit_count(ptr_sfb_new[sfb_prev] - ptr_sfb_new[sfb_last]) -
                     iaace_scf_delta_bit_count(ptr_sfb_prev[sfb_prev] - ptr_sfb_prev[sfb_last]);
  }

  for (sfb = sfb_last + 1; sfb < stop_sfb; sfb++) {
    if (ptr_sfb_prev[sfb] != SHRT_MIN) {
      scf_bits_diff += iaace_scf_delta_bit_count(ptr_sfb_new[sfb_last] - ptr_sfb_new[sfb]) -
                       iaace_scf_delta_bit_count(ptr_sfb_prev[sfb_last] - ptr_sfb_prev[sfb]);

      sfb_last = sfb;
    }
  }

  sfb_next = stop_sfb;

  while ((sfb_next < sfb_count) && (ptr_sfb_prev[sfb_next] == SHRT_MIN)) {
    sfb_next++;
  }

  if (sfb_next < sfb_count) {
    scf_bits_diff += iaace_scf_delta_bit_count(ptr_sfb_new[sfb_last] - ptr_sfb_new[sfb_next]) -
                     iaace_scf_delta_bit_count(ptr_sfb_prev[sfb_last] - ptr_sfb_prev[sfb_next]);
  }

  return scf_bits_diff;
}

static FLOAT32 iaace_calc_spec_pe_diff(ixheaace_psy_out_channel *pstr_psy_out,
                                       WORD16 *ptr_scf_prev, WORD16 *ptr_scf_new,
                                       FLOAT32 *ptr_sfb_const_pe_part, FLOAT32 *ptr_sfb_form_fac,
                                       FLOAT32 *ptr_sfb_num_rel_lines, WORD32 start_sfb,
                                       WORD32 stop_sfb) {
  FLOAT32 spec_pe_diff = 0.0f;
  WORD32 sfb;

  for (sfb = start_sfb; sfb < stop_sfb; sfb++) {
    if (ptr_scf_prev[sfb] != SHRT_MIN) {
      FLOAT32 ld_ratio_prev, ld_ratio_new, pe_prev, pe_new;

      if (ptr_sfb_const_pe_part[sfb] == MIN_FLT_VAL) {
        ptr_sfb_const_pe_part[sfb] = (FLOAT32)log(pstr_psy_out->ptr_sfb_energy[sfb] *
                                                  (FLOAT32)6.75f / ptr_sfb_form_fac[sfb]) *
                                     LOG2_1;
      }

      ld_ratio_prev = ptr_sfb_const_pe_part[sfb] - 0.375f * ptr_scf_prev[sfb];
      ld_ratio_new = ptr_sfb_const_pe_part[sfb] - 0.375f * ptr_scf_new[sfb];

      if (ld_ratio_prev >= PE_C1) {
        pe_prev = ld_ratio_prev;
      } else {
        pe_prev = PE_C2 + PE_C3 * ld_ratio_prev;
      }

      if (ld_ratio_new >= PE_C1) {
        pe_new = ld_ratio_new;
      } else {
        pe_new = PE_C2 + PE_C3 * ld_ratio_new;
      }

      spec_pe_diff += (FLOAT32)0.7f * ptr_sfb_num_rel_lines[sfb] * (pe_new - pe_prev);
    }
  }

  return spec_pe_diff;
}

static FLOAT32 iaace_calc_sfb_dist(const FLOAT32 *ptr_spec, const FLOAT32 *ptr_exp_spec,
                                   WORD16 *ptr_quant_spec, WORD32 sfb_width, WORD32 gain) {
  WORD32 i = 0;
  FLOAT32 dist = 0;
  FLOAT32 k = -0.0946f + 0.5f;
  FLOAT32 quantizer = ixheaace_fd_quant_table[gain + 128];
  FLOAT32 inv_quantizer = ixheaace_fd_inv_quant_table[gain + 128];

  while (i < sfb_width) {
    FLOAT32 iq_val;
    FLOAT32 diff;

    ptr_quant_spec[i] = (WORD16)(k + quantizer * ptr_exp_spec[i]);

    if (ptr_quant_spec[i] < 64) {
      iq_val = ixheaace_pow_4_3_table[ptr_quant_spec[i]] * inv_quantizer;
    } else {
      iq_val = (FLOAT32)((pow((FLOAT32)abs(ptr_quant_spec[i]), 4.0f / 3.0f)) * inv_quantizer);
    }

    diff = (FLOAT32)fabs(ptr_spec[i]) - iq_val;

    dist += diff * diff;

    i++;
  }

  return dist;
}

static WORD16 iaace_improve_scf(FLOAT32 *ptr_spec, FLOAT32 *ptr_exp_spec, WORD16 *ptr_quant_spec,
                                WORD16 *ptr_quant_spec_temp, WORD32 sfb_width, FLOAT32 threshold,
                                WORD16 scf, WORD16 min_scf, FLOAT32 *dist,
                                WORD16 *ptr_min_calc_scf) {
  FLOAT32 sfb_dist;
  WORD16 best_scf = scf;
  WORD32 j;

  sfb_dist = iaace_calc_sfb_dist(ptr_spec, ptr_exp_spec, ptr_quant_spec, sfb_width, scf);

  *ptr_min_calc_scf = scf;

  if (sfb_dist > (1.25 * threshold)) {
    FLOAT32 best_sfb_dist = sfb_dist;

    if (scf > min_scf) {
      scf--;

      sfb_dist = iaace_calc_sfb_dist(ptr_spec, ptr_exp_spec, ptr_quant_spec_temp, sfb_width, scf);

      if (sfb_dist < best_sfb_dist) {
        best_scf = scf;
        best_sfb_dist = sfb_dist;

        for (j = 0; j < sfb_width; j++) {
          ptr_quant_spec[j] = ptr_quant_spec_temp[j];
        }
      }

      *ptr_min_calc_scf = scf;
    }
    *dist = best_sfb_dist;
  } else {
    FLOAT32 best_sfb_dist = sfb_dist;
    FLOAT32 allowed_sfb_dist = MIN(sfb_dist * 1.25f, threshold);
    WORD32 count;

    for (count = SCF_COUNT_LIMIT_AAC; count >= 0; count--) {
      scf++;

      sfb_dist = iaace_calc_sfb_dist(ptr_spec, ptr_exp_spec, ptr_quant_spec_temp, sfb_width, scf);

      if (sfb_dist < allowed_sfb_dist) {
        *ptr_min_calc_scf = best_scf + 1;

        best_scf = scf;
        best_sfb_dist = sfb_dist;

        for (j = 0; j < sfb_width; j++) {
          ptr_quant_spec[j] = ptr_quant_spec_temp[j];
        }
      }
    }
    *dist = best_sfb_dist;
  }

  for (j = 0; j < sfb_width; j++) {
    if (ptr_spec[j] < 0) {
      ptr_quant_spec[j] = -ptr_quant_spec[j];
    }
  }

  return best_scf;
}

static VOID iaace_assimilate_single_scf(ixheaace_psy_out_channel *pstr_psy_out,
                                        FLOAT32 *ptr_exp_spec, WORD16 *ptr_quant_spec,
                                        WORD16 *ptr_quant_spec_temp, WORD16 *ptr_scf,
                                        WORD16 *ptr_min_scf, FLOAT32 *ptr_sfb_dist,
                                        FLOAT32 *ptr_sfb_const_pe_part, FLOAT32 *ptr_sfb_form_fac,
                                        FLOAT32 *ptr_sfb_num_lines, WORD16 *ptr_min_calc_scf,
                                        FLOAT32 *ptr_ptr_mdct_spec) {
  WORD32 sfb_prev, sfb_act, sfb_next;
  WORD16 scf_act = 0, *scf_prev, *scf_next, min_scf, max_scf;
  WORD32 sfb_width, sfb_offs;
  FLOAT32 energy;
  FLOAT32 sfb_pe_prev, sfb_pe_new;
  FLOAT32 sfb_dist_new;
  WORD32 j;
  WORD32 success = 0;
  FLOAT32 delta_pe = 0.0f, delta_pe_new, delta_pe_temp;
  WORD16 prev_scf_last[MAXIMUM_GROUPED_SCALE_FACTOR_BAND],
      prev_scf_next[MAXIMUM_GROUPED_SCALE_FACTOR_BAND];
  FLOAT32 delta_pe_last[MAXIMUM_GROUPED_SCALE_FACTOR_BAND];
  WORD32 update_min_scf;

  for (j = 0; j < pstr_psy_out->sfb_count; j++) {
    prev_scf_last[j] = SHRT_MAX;
    prev_scf_next[j] = SHRT_MAX;
    delta_pe_last[j] = MAX_FLT_VAL;
  }

  sfb_prev = -1;
  sfb_act = -1;
  sfb_next = -1;
  scf_prev = 0;
  scf_next = 0;
  min_scf = SHRT_MAX;
  max_scf = SHRT_MAX;

  do {
    sfb_next++;

    while ((sfb_next < pstr_psy_out->sfb_count) && (ptr_scf[sfb_next] == SHRT_MIN)) {
      sfb_next++;
    }

    if ((sfb_prev >= 0) && (sfb_act >= 0) && (sfb_next < pstr_psy_out->sfb_count)) {
      scf_act = ptr_scf[sfb_act];

      scf_prev = ptr_scf + sfb_prev;
      scf_next = ptr_scf + sfb_next;

      min_scf = MIN(*scf_prev, *scf_next);

      max_scf = MAX(*scf_prev, *scf_next);
    } else {
      if ((sfb_prev == -1) && (sfb_act >= 0) && (sfb_next < pstr_psy_out->sfb_count)) {
        scf_act = ptr_scf[sfb_act];

        scf_prev = &scf_act;

        scf_next = ptr_scf + sfb_next;

        min_scf = *scf_next;

        max_scf = *scf_next;
      } else {
        if ((sfb_prev >= 0) && (sfb_act >= 0) && (sfb_next == pstr_psy_out->sfb_count)) {
          scf_act = ptr_scf[sfb_act];

          scf_prev = ptr_scf + sfb_prev;

          scf_next = &scf_act;

          min_scf = *scf_prev;

          max_scf = *scf_prev;
        }
      }
    }

    if (sfb_act >= 0) {
      min_scf = MAX(min_scf, ptr_min_scf[sfb_act]);
    }

    if ((sfb_act >= 0) && (sfb_prev >= 0 || sfb_next < pstr_psy_out->sfb_count) &&
        (scf_act > min_scf) && (scf_act <= min_scf + MAX_SCF_DELTA) &&
        (scf_act >= max_scf - MAX_SCF_DELTA) &&
        (*scf_prev != prev_scf_last[sfb_act] || *scf_next != prev_scf_next[sfb_act] ||
         delta_pe < delta_pe_last[sfb_act])) {
      success = 0;

      sfb_width = pstr_psy_out->sfb_offsets[sfb_act + 1] - pstr_psy_out->sfb_offsets[sfb_act];

      sfb_offs = pstr_psy_out->sfb_offsets[sfb_act];

      energy = pstr_psy_out->ptr_sfb_energy[sfb_act];

      if (ptr_sfb_const_pe_part[sfb_act] == MIN_FLT_VAL) {
        ptr_sfb_const_pe_part[sfb_act] =
            (FLOAT32)log(energy * (FLOAT32)6.75f / ptr_sfb_form_fac[sfb_act]) * LOG2_1;
      }

      sfb_pe_prev = iaace_calc_single_spec_pe(scf_act, ptr_sfb_const_pe_part[sfb_act],
                                              ptr_sfb_num_lines[sfb_act]) +
                    (FLOAT32)iaace_count_single_scf_bits(scf_act, *scf_prev, *scf_next);

      delta_pe_new = delta_pe;
      update_min_scf = 1;

      do {
        scf_act--;

        if (scf_act < ptr_min_calc_scf[sfb_act] && scf_act >= max_scf - MAX_SCF_DELTA) {
          sfb_pe_new = iaace_calc_single_spec_pe(scf_act, ptr_sfb_const_pe_part[sfb_act],
                                                 ptr_sfb_num_lines[sfb_act]) +
                       (FLOAT32)iaace_count_single_scf_bits(scf_act, *scf_prev, *scf_next);

          delta_pe_temp = delta_pe + sfb_pe_new - sfb_pe_prev;

          if (delta_pe_temp < (FLOAT32)10.0f) {
            sfb_dist_new =
                iaace_calc_sfb_dist(ptr_ptr_mdct_spec + sfb_offs, ptr_exp_spec + sfb_offs,
                                    ptr_quant_spec_temp + sfb_offs, sfb_width, scf_act);

            if (sfb_dist_new < ptr_sfb_dist[sfb_act]) {
              ptr_scf[sfb_act] = scf_act;
              ptr_sfb_dist[sfb_act] = sfb_dist_new;

              for (j = sfb_offs; j < sfb_offs + sfb_width; j++) {
                ptr_quant_spec[j] = ptr_quant_spec_temp[j];

                if (ptr_ptr_mdct_spec[j] < 0.0f) {
                  ptr_quant_spec[j] = -ptr_quant_spec[j];
                }
              }
              delta_pe_new = delta_pe_temp;
              success = 1;
            }

            if (update_min_scf) {
              ptr_min_calc_scf[sfb_act] = scf_act;
            }
          } else {
            update_min_scf = 0;
          }
        }
      } while (scf_act > min_scf);

      delta_pe = delta_pe_new;

      prev_scf_last[sfb_act] = *scf_prev;
      prev_scf_next[sfb_act] = *scf_next;
      delta_pe_last[sfb_act] = delta_pe;
    }

    if (success) {
      sfb_prev = -1;
      sfb_act = -1;
      sfb_next = -1;
      scf_prev = 0;
      scf_next = 0;
      min_scf = SHRT_MAX;
      max_scf = SHRT_MAX;
      success = 0;
    } else {
      sfb_prev = sfb_act;
      sfb_act = sfb_next;
    }
  } while (sfb_next < pstr_psy_out->sfb_count);
}

static VOID iaace_assimilate_multiple_scf(ixheaace_psy_out_channel *pstr_psy_out,
                                          FLOAT32 *ptr_exp_spec, WORD16 *ptr_quant_spec,
                                          WORD16 *ptr_quant_spec_temp, WORD16 *ptr_scf,
                                          WORD16 *ptr_min_scf, FLOAT32 *ptr_sfb_dist,
                                          FLOAT32 *ptr_sfb_const_pe_part,
                                          FLOAT32 *ptr_sfb_form_fac, FLOAT32 *ptr_sfb_num_lines,
                                          FLOAT32 *ptr_ptr_mdct_spec) {
  WORD32 sfb, start_sfb, stop_sfb;
  WORD16 scf_temp[MAXIMUM_GROUPED_SCALE_FACTOR_BAND], min_scf, max_scf, scf_act;
  WORD32 possible_region_found;
  WORD32 sfb_width, sfb_offs, j;
  FLOAT32 sfb_dist_new[MAXIMUM_GROUPED_SCALE_FACTOR_BAND] = {0};
  FLOAT32 prev_dist_sum, new_dist_sum;
  WORD32 delta_scf_bits;
  FLOAT32 delta_spec_pe;
  FLOAT32 delta_pe = 0.0f, delta_pe_new;
  WORD32 sfb_count = pstr_psy_out->sfb_count;

  min_scf = SHRT_MAX;
  max_scf = SHRT_MIN;

  for (sfb = 0; sfb < sfb_count; sfb++) {
    if (ptr_scf[sfb] != SHRT_MIN) {
      min_scf = MIN(min_scf, ptr_scf[sfb]);

      max_scf = MAX(max_scf, ptr_scf[sfb]);
    }
  }

  if (max_scf != SHRT_MIN && max_scf <= min_scf + MAX_SCF_DELTA) {
    scf_act = max_scf;
    do {
      scf_act = scf_act - 2;

      memcpy(scf_temp, ptr_scf, MAXIMUM_GROUPED_SCALE_FACTOR_BAND * sizeof(*ptr_scf));

      stop_sfb = 0;

      do {
        sfb = stop_sfb;

        while (sfb < sfb_count && (ptr_scf[sfb] == SHRT_MIN || ptr_scf[sfb] <= scf_act)) {
          sfb++;
        }

        start_sfb = sfb;

        sfb++;

        while (sfb < sfb_count && (ptr_scf[sfb] == SHRT_MIN || ptr_scf[sfb] > scf_act)) {
          sfb++;
        }

        stop_sfb = sfb;

        possible_region_found = 0;

        if (start_sfb < sfb_count) {
          possible_region_found = 1;

          for (sfb = start_sfb; sfb < stop_sfb; sfb++) {
            if (ptr_scf[sfb] != SHRT_MIN) {
              if (scf_act < ptr_min_scf[sfb]) {
                possible_region_found = 0;
                break;
              }
            }
          }
        }

        if (possible_region_found) {
          for (sfb = start_sfb; sfb < stop_sfb; sfb++) {
            if (scf_temp[sfb] != SHRT_MIN) {
              scf_temp[sfb] = scf_act;
            }
          }

          delta_scf_bits =
              iaace_count_scf_bits_diff(ptr_scf, scf_temp, sfb_count, start_sfb, stop_sfb);

          delta_spec_pe =
              iaace_calc_spec_pe_diff(pstr_psy_out, ptr_scf, scf_temp, ptr_sfb_const_pe_part,
                                      ptr_sfb_form_fac, ptr_sfb_num_lines, start_sfb, stop_sfb);

          delta_pe_new = delta_pe + (FLOAT32)delta_scf_bits + delta_spec_pe;

          if (delta_pe_new < (FLOAT32)10.0f) {
            prev_dist_sum = new_dist_sum = 0.0f;

            for (sfb = start_sfb; sfb < stop_sfb; sfb++) {
              if (scf_temp[sfb] != SHRT_MIN) {
                prev_dist_sum += ptr_sfb_dist[sfb];

                sfb_width = pstr_psy_out->sfb_offsets[sfb + 1] - pstr_psy_out->sfb_offsets[sfb];

                sfb_offs = pstr_psy_out->sfb_offsets[sfb];

                sfb_dist_new[sfb] =
                    iaace_calc_sfb_dist(ptr_ptr_mdct_spec + sfb_offs, ptr_exp_spec + sfb_offs,
                                        ptr_quant_spec_temp + sfb_offs, sfb_width, scf_act);

                if (sfb_dist_new[sfb] > pstr_psy_out->ptr_sfb_thr[sfb]) {
                  new_dist_sum = (FLOAT32)2.0f * prev_dist_sum;
                  break;
                }

                new_dist_sum += sfb_dist_new[sfb];
              }
            }

            if (new_dist_sum < prev_dist_sum) {
              delta_pe = delta_pe_new;

              for (sfb = start_sfb; sfb < stop_sfb; sfb++) {
                if (ptr_scf[sfb] != SHRT_MIN) {
                  sfb_width = pstr_psy_out->sfb_offsets[sfb + 1] - pstr_psy_out->sfb_offsets[sfb];

                  sfb_offs = pstr_psy_out->sfb_offsets[sfb];
                  ptr_scf[sfb] = scf_act;
                  ptr_sfb_dist[sfb] = sfb_dist_new[sfb];

                  for (j = sfb_offs; j < sfb_offs + sfb_width; j++) {
                    ptr_quant_spec[j] = ptr_quant_spec_temp[j];

                    if (ptr_ptr_mdct_spec[j] < 0.0f) {
                      ptr_quant_spec[j] = -ptr_quant_spec[j];
                    }
                  }
                }
              }
            }
          }
        }

      } while (stop_sfb <= sfb_count);

    } while (scf_act > min_scf);
  }
}

VOID iaace_estimate_scfs_chan(
    ixheaace_psy_out_channel **pstr_psy_out,
    ixheaace_qc_out_channel **pstr_qc_out_chan,
    FLOAT32 sfb_form_factor_ch[IXHEAACE_MAX_CH_IN_BS_ELE][MAXIMUM_GROUPED_SCALE_FACTOR_BAND],
    FLOAT32 sfb_num_relevant_lines_ch[][MAXIMUM_GROUPED_SCALE_FACTOR_BAND], WORD32 num_channels,
    WORD32 chn, WORD32 frame_len_long) {
  WORD16 *ptr_scalefactor;
  WORD32 *global_gain;
  FLOAT32 *ptr_sfb_form_factor;
  FLOAT32 *ptr_sfb_num_relevant_lines_ch;
  WORD16 *ptr_quant_spec;
  WORD32 i, ch, j;
  FLOAT32 thresh, energy, energy_part, thr_part;
  FLOAT32 scf_float;
  WORD16 scf_int = 0, min_scf = 0, max_scf = 0;
  FLOAT32 max_spec = 0.0f;
  WORD16 min_sf_max_quant[MAXIMUM_GROUPED_SCALE_FACTOR_BAND] = {0};
  FLOAT32 sfb_dist[MAXIMUM_GROUPED_SCALE_FACTOR_BAND] = {0};
  WORD16 min_calc_scf[MAXIMUM_GROUPED_SCALE_FACTOR_BAND] = {0};
  WORD16 quant_spec_temp[FRAME_LEN_1024];
  FLOAT32 ptr_exp_spec[FRAME_LEN_1024];
  FLOAT32 ptr_mdct_spec[FRAME_LEN_1024];

  memset(quant_spec_temp, 0, frame_len_long * sizeof(quant_spec_temp[0]));
  memset(ptr_mdct_spec, 0, frame_len_long * sizeof(ptr_mdct_spec[0]));
  memset(ptr_exp_spec, 0, frame_len_long * sizeof(ptr_exp_spec[0]));

  for (ch = chn; ch < chn + num_channels; ch++) {
    ixheaace_psy_out_channel *pstr_psy_out_chan = pstr_psy_out[ch];
    pstr_qc_out_chan[ch]->global_gain = 0;

    memset(pstr_qc_out_chan[ch]->scalefactor, 0,
           sizeof(*pstr_qc_out_chan[ch]->scalefactor) * pstr_psy_out[ch]->sfb_count);
    memset(pstr_qc_out_chan[ch]->quant_spec, 0,
           sizeof(*pstr_qc_out_chan[ch]->quant_spec) * frame_len_long);

    ptr_scalefactor = pstr_qc_out_chan[ch]->scalefactor;
    global_gain = &pstr_qc_out_chan[ch]->global_gain;
    ptr_sfb_form_factor = &sfb_form_factor_ch[ch][0];
    ptr_sfb_num_relevant_lines_ch = &sfb_num_relevant_lines_ch[ch][0];
    ptr_quant_spec = pstr_qc_out_chan[ch]->quant_spec;

    for (i = 0; i < pstr_psy_out_chan->sfb_count; i++) {
      thresh = pstr_psy_out_chan->ptr_sfb_thr[i];
      energy = pstr_psy_out_chan->ptr_sfb_energy[i];
      max_spec = 0.0f;

      for (j = pstr_psy_out_chan->sfb_offsets[i]; j < pstr_psy_out_chan->sfb_offsets[i + 1];
           j++) {
        max_spec = (FLOAT32)MAX(max_spec, fabsf(pstr_psy_out_chan->ptr_spec_coeffs[j]));
      }

      ptr_scalefactor[i] = MIN_SHRT_VAL;
      min_sf_max_quant[i] = MIN_SHRT_VAL;

      if ((max_spec > 0.0) && (energy > thresh) && (ptr_sfb_form_factor[i] != MIN_FLT_VAL)) {
        energy_part = (FLOAT32)log10(ptr_sfb_form_factor[i]);

        thr_part = (FLOAT32)log10(6.75 * thresh + MIN_FLT_VAL);
        scf_float = 8.8585f * (thr_part - energy_part);
        scf_int = (WORD16)floor(scf_float);
        min_sf_max_quant[i] = (WORD16)floor(C1_SF + C2_SF * log(max_spec));
        scf_int = MAX(scf_int, min_sf_max_quant[i]);
        scf_int = MAX(scf_int, MIN_GAIN_INDEX_AAC);
        scf_int = MIN(scf_int, (MAX_GAIN_INDEX_AAC - SCF_COUNT_LIMIT_AAC - 1));
        for (j = 0; j < pstr_psy_out_chan->sfb_offsets[i + 1] - pstr_psy_out_chan->sfb_offsets[i];
             j++) {
          ptr_exp_spec[pstr_psy_out_chan->sfb_offsets[i] + j] = (FLOAT32)(
              pstr_psy_out_chan->ptr_spec_coeffs[pstr_psy_out_chan->sfb_offsets[i] + j]);
          ptr_mdct_spec[pstr_psy_out_chan->sfb_offsets[i] + j] = (FLOAT32)(
              pstr_psy_out_chan->ptr_spec_coeffs[pstr_psy_out_chan->sfb_offsets[i] + j]);
        }

        iaace_calculate_exp_spec(
            pstr_psy_out_chan->sfb_offsets[i + 1] - pstr_psy_out_chan->sfb_offsets[i],
            ptr_exp_spec + pstr_psy_out_chan->sfb_offsets[i],
            ptr_mdct_spec + pstr_psy_out_chan->sfb_offsets[i]);

        scf_int = iaace_improve_scf(
            ptr_mdct_spec + pstr_psy_out_chan->sfb_offsets[i],
            ptr_exp_spec + pstr_psy_out_chan->sfb_offsets[i],
            ptr_quant_spec + pstr_psy_out_chan->sfb_offsets[i],
            quant_spec_temp + pstr_psy_out_chan->sfb_offsets[i],
            pstr_psy_out_chan->sfb_offsets[i + 1] - pstr_psy_out_chan->sfb_offsets[i], thresh,
            scf_int, min_sf_max_quant[i], &sfb_dist[i], &min_calc_scf[i]);

        ptr_scalefactor[i] = scf_int;
      }
    }

    {
      FLOAT32 sfb_const_pe_part[MAXIMUM_GROUPED_SCALE_FACTOR_BAND];

      for (i = 0; i < pstr_psy_out_chan->sfb_count; i++) {
        sfb_const_pe_part[i] = MIN_FLT_VAL;
      }

      iaace_assimilate_single_scf(pstr_psy_out_chan, ptr_exp_spec, ptr_quant_spec,
                                  quant_spec_temp, ptr_scalefactor, min_sf_max_quant, sfb_dist,
                                  sfb_const_pe_part, ptr_sfb_form_factor,
                                  ptr_sfb_num_relevant_lines_ch, min_calc_scf, ptr_mdct_spec);

      iaace_assimilate_multiple_scf(pstr_psy_out_chan, ptr_exp_spec, ptr_quant_spec,
                                    quant_spec_temp, ptr_scalefactor, min_sf_max_quant, sfb_dist,
                                    sfb_const_pe_part, ptr_sfb_form_factor,
                                    ptr_sfb_num_relevant_lines_ch, ptr_mdct_spec);
    }

    max_scf = MIN_SHRT_VAL;
    min_scf = MAX_SHRT_VAL;
    for (i = 0; i < pstr_psy_out_chan->sfb_count; i++) {
      if (max_scf < ptr_scalefactor[i]) {
        max_scf = ptr_scalefactor[i];
      }
      if ((ptr_scalefactor[i] != MIN_SHRT_VAL) && (min_scf > ptr_scalefactor[i])) {
        min_scf = ptr_scalefactor[i];
      }
    }

    for (i = 0; i < pstr_psy_out[ch]->sfb_count; i++) {
      if ((ptr_scalefactor[i] != MIN_SHRT_VAL) &&
          (min_scf + MAX_SCF_DELTA) < ptr_scalefactor[i]) {
        ptr_scalefactor[i] = min_scf + MAX_SCF_DELTA;

        iaace_calc_sfb_dist(
            ptr_mdct_spec + pstr_psy_out_chan->sfb_offsets[i],
            ptr_exp_spec + pstr_psy_out_chan->sfb_offsets[i],
            ptr_quant_spec + pstr_psy_out_chan->sfb_offsets[i],
            pstr_psy_out_chan->sfb_offsets[i + 1] - pstr_psy_out_chan->sfb_offsets[i],
            ptr_scalefactor[i]);
      }
    }

    max_scf = MIN((min_scf + MAX_SCF_DELTA), max_scf);

    if (max_scf > MIN_SHRT_VAL) {
      *global_gain = max_scf;
      for (i = 0; i < pstr_psy_out_chan->sfb_count; i++) {
        if (ptr_scalefactor[i] == MIN_SHRT_VAL) {
          ptr_scalefactor[i] = 0;
          for (j = pstr_psy_out_chan->sfb_offsets[i]; j < pstr_psy_out_chan->sfb_offsets[i + 1];
               j++) {
            pstr_psy_out_chan->ptr_spec_coeffs[j] = 0.0f;
          }
        } else {
          ptr_scalefactor[i] = max_scf - ptr_scalefactor[i];
        }
      }
    } else {
      *global_gain = 0;
      for (i = 0; i < pstr_psy_out_chan->sfb_count; i++) {
        ptr_scalefactor[i] = 0;
        for (j = pstr_psy_out_chan->sfb_offsets[i]; j < pstr_psy_out_chan->sfb_offsets[i + 1];
             j++) {
          pstr_psy_out_chan->ptr_spec_coeffs[j] = 0.0f;
        }
      }
    }
  }
}
