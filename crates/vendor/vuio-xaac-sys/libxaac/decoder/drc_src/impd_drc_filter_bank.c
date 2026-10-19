/******************************************************************************
 *
 * Copyright (C) 2018 The Android Open Source Project
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
#include <stdio.h>
#include <stdlib.h>
#include <math.h>

#include "impd_type_def.h"
#include "impd_drc_extr_delta_coded_info.h"
#include "impd_drc_common.h"
#include "impd_drc_struct.h"
#include "impd_drc_filter_bank.h"
#include "impd_drc_rom.h"

VOID impd_compute_filt_coeff(WORD32 crossover_freq_idx,
                             ia_iir_filter_struct* pstr_lp_filt_coeff,
                             ia_iir_filter_struct* pstr_hp_filt_coeff,
                             ia_iir_filter_struct* pstr_ap_filt_coeff,
                             WORD32 filter_type) {
  FLOAT32 gamma = normal_cross_freq[crossover_freq_idx].gamma;
  FLOAT32 delta = normal_cross_freq[crossover_freq_idx].delta;

  if (filter_type == 0 || filter_type == 2) {
    pstr_lp_filt_coeff->a0 = 1.0f;
    pstr_lp_filt_coeff->a1 = 2.0f * (gamma - delta);
    pstr_lp_filt_coeff->a2 = 2.0f * (gamma + delta) - 1.0f;
    pstr_lp_filt_coeff->b0 = gamma;
    pstr_lp_filt_coeff->b1 = 2.0f * gamma;
    pstr_lp_filt_coeff->b2 = gamma;

    pstr_hp_filt_coeff->a0 = 1.0f;
    pstr_hp_filt_coeff->a1 = pstr_lp_filt_coeff->a1;
    pstr_hp_filt_coeff->a2 = pstr_lp_filt_coeff->a2;
    pstr_hp_filt_coeff->b0 = delta;
    pstr_hp_filt_coeff->b1 = -2.0f * delta;
    pstr_hp_filt_coeff->b2 = delta;
  }

  if (filter_type == 1 || filter_type == 2) {
    pstr_ap_filt_coeff->a0 = 1.0f;
    pstr_ap_filt_coeff->a1 = 2.0f * (gamma - delta);
    ;
    pstr_ap_filt_coeff->a2 = 2.0f * (gamma + delta) - 1.0f;
    ;
    pstr_ap_filt_coeff->b0 = pstr_ap_filt_coeff->a2;
    pstr_ap_filt_coeff->b1 = pstr_ap_filt_coeff->a1;
    pstr_ap_filt_coeff->b2 = pstr_ap_filt_coeff->a0;
  }

  return;
}

WORD32 impd_initialize_filt_bank(WORD32 num_sub_bands,
                                 ia_gain_params_struct* gain_params,
                                 ia_drc_filter_bank_struct* drc_filter_bank) {
  ia_two_band_filt_struct* str_two_band_bank;
  ia_three_band_filt_struct* str_three_band_bank;
  ia_four_band_filt_struct* str_four_band_bank;
  drc_filter_bank->complexity = 0;
  drc_filter_bank->num_bands = num_sub_bands;

  if (num_sub_bands == 1) {
    return 0;
  } else if (num_sub_bands == 2) {
    str_two_band_bank = &drc_filter_bank->str_two_band_bank;
    impd_compute_filt_coeff(gain_params[1].crossover_freq_idx,
                            &(str_two_band_bank->low_pass),
                            &(str_two_band_bank->high_pass), NULL, 0);
  } else if (num_sub_bands == 3) {
    str_three_band_bank = &drc_filter_bank->str_three_band_bank;

    impd_compute_filt_coeff(gain_params[1].crossover_freq_idx,
                            &(str_three_band_bank->str_low_pass_stage_2),
                            &(str_three_band_bank->str_high_pass_stage_2),
                            &(str_three_band_bank->str_all_pass_stage_2), 2);
    impd_compute_filt_coeff(gain_params[2].crossover_freq_idx,
                            &(str_three_band_bank->str_low_pass_stage_1),
                            &(str_three_band_bank->str_high_pass_stage_1), NULL,
                            0);
  }

  else if (num_sub_bands == 4) {
    str_four_band_bank = &drc_filter_bank->str_four_band_bank;

    impd_compute_filt_coeff(gain_params[1].crossover_freq_idx,
                            &(str_four_band_bank->str_low_pass_stage_3_low),
                            &(str_four_band_bank->str_high_pass_stage_3_low),
                            &(str_four_band_bank->str_all_pass_stage_2_high),
                            2);
    impd_compute_filt_coeff(gain_params[2].crossover_freq_idx,
                            &(str_four_band_bank->str_low_pass_stage_1),
                            &(str_four_band_bank->str_high_pass_stage_1), NULL,
                            0);
    impd_compute_filt_coeff(gain_params[3].crossover_freq_idx,
                            &(str_four_band_bank->str_low_pass_stage_3_high),
                            &(str_four_band_bank->str_high_pass_stage_3_high),
                            &(str_four_band_bank->str_all_pass_stage_2_low), 2);
  } else {
    return -1;
  }

  return 0;
}

WORD32 impd_init_all_filter_banks(
    ia_uni_drc_coeffs_struct* str_p_loc_drc_coefficients_uni_drc,
    ia_drc_instructions_struct* str_drc_instruction_str,
    ia_filter_banks_struct* ia_filter_banks_struct) {
  WORD32 err_code = 0;
  WORD32 b, g, i, k, m, s, crossover_freq_idx, num_ch_in_groups,
      num_ph_align_ch_groups;
  WORD32 match_found = 0, num_filter;
  WORD32 cascade_cross_idx[CHANNEL_GROUP_COUNT_MAX + 1]
                          [CHANNEL_GROUP_COUNT_MAX * 3];
  WORD32 count[CHANNEL_GROUP_COUNT_MAX + 1];

  num_ch_in_groups = 0;
  num_ph_align_ch_groups = str_drc_instruction_str->num_drc_ch_groups;

  for (g = 0; g < str_drc_instruction_str->num_drc_ch_groups; g++) {
    num_ch_in_groups += str_drc_instruction_str->num_chan_per_ch_group[g];
  }

  if (num_ch_in_groups < str_drc_instruction_str->audio_num_chan) {
    num_ph_align_ch_groups++;
  }

  ia_filter_banks_struct->nfilter_banks =
      str_drc_instruction_str->num_drc_ch_groups;
  ia_filter_banks_struct->num_ph_align_ch_groups = num_ph_align_ch_groups;

  if (str_p_loc_drc_coefficients_uni_drc == NULL) {
    ia_filter_banks_struct->str_drc_filter_bank->num_bands = 1;
  } else {
    for (g = 0; g < str_drc_instruction_str->num_drc_ch_groups; g++) {
      err_code = impd_initialize_filt_bank(
          str_p_loc_drc_coefficients_uni_drc
              ->gain_set_params[str_drc_instruction_str
                                    ->gain_set_index_for_channel_group[g]]
              .band_count,
          str_p_loc_drc_coefficients_uni_drc
              ->gain_set_params[str_drc_instruction_str
                                    ->gain_set_index_for_channel_group[g]]
              .gain_params,
          &(ia_filter_banks_struct->str_drc_filter_bank[g]));
      if (err_code != 0) return (err_code);
    }
  }

  for (g = 0; g < CHANNEL_GROUP_COUNT_MAX + 1; g++) {
    count[g] = 0;
  }
  for (g = 0; g < str_drc_instruction_str->num_drc_ch_groups; g++) {
    for (b = 1;
         b < str_p_loc_drc_coefficients_uni_drc
                 ->gain_set_params[str_drc_instruction_str
                                       ->gain_set_index_for_channel_group[g]]
                 .band_count;
         b++) {
      crossover_freq_idx =
          str_p_loc_drc_coefficients_uni_drc
              ->gain_set_params[str_drc_instruction_str
                                    ->gain_set_index_for_channel_group[g]]
              .gain_params[b]
              .crossover_freq_idx;
      for (k = 0; k < num_ph_align_ch_groups; k++) {
        if (k != g) {
          cascade_cross_idx[k][count[k]] = crossover_freq_idx;
          count[k]++;
          if (count[k] > CHANNEL_GROUP_COUNT_MAX * 3) {
            return -1;
          }
        }
      }
    }
  }

  i = 0;
  while (i < count[0]) {
    crossover_freq_idx = cascade_cross_idx[0][i];
    match_found = 0;
    for (g = 1; g < num_ph_align_ch_groups; g++) {
      match_found = 0;
      for (k = 0; k < count[g]; k++) {
        if (cascade_cross_idx[g][k] == crossover_freq_idx) {
          match_found = 1;
          break;
        }
      }
      if (match_found == 0) break;
    }
    if (match_found == 1) {
      for (g = 0; g < num_ph_align_ch_groups; g++) {
        for (m = 0; m < count[g]; m++) {
          if (cascade_cross_idx[g][m] == crossover_freq_idx) {
            for (s = m + 1; s < count[g]; s++) {
              cascade_cross_idx[g][s - 1] = cascade_cross_idx[g][s];
            }
            count[g]--;
            break;
          }
        }
      }
      i = 0;
    } else {
      i++;
    }
  }

  for (g = 0; g < num_ph_align_ch_groups; g++) {
    num_filter = count[g];
    if (num_filter > 0) {
      for (i = 0; i < num_filter; i++) {
        impd_compute_filt_coeff(
            cascade_cross_idx[g][i], NULL, NULL,
            &(ia_filter_banks_struct->str_drc_filter_bank[g]
                  .str_all_pass_cascade.str_all_pass_cascade_filter[i]
                  .str_all_pass_stage),
            1);
      }
      ia_filter_banks_struct->str_drc_filter_bank[g]
          .str_all_pass_cascade.num_filter = num_filter;
    }

    if (err_code != 0) return (err_code);
  }

  return 0;
}

VOID impd_iir_second_order_filter_all_pass(ia_iir_filter_struct* filter,
                                           WORD32 chan_idx, WORD32 frame_len,
                                           FLOAT32* input, FLOAT32* output) {
  WORD32 i;
  FLOAT32 tmp;
  FLOAT32 a1 = filter->a1;
  FLOAT32 a2 = filter->a2;
  FLOAT32 b0 = filter->b0;
  FLOAT32 b1 = filter->b1;
  FLOAT32 b2 = filter->b2;

  FLOAT32 st1 = filter->x_p[chan_idx * 2];
  FLOAT32 st2 = filter->y_p[chan_idx * 2];

  for (i = 0; i < frame_len; i++) {
    tmp = input[i];
    output[i] = b0 * tmp + st1;
    st1 = b1 * tmp - a1 * output[i] + st2;
    st2 = b2 * tmp - a2 * output[i];
  }
  filter->x_p[chan_idx * 2] = st1;
  filter->y_p[chan_idx * 2] = st2;

  return;
}

VOID impd_apply_low_high_filter(ia_iir_filter_struct* pstr_lp_filt_coeff,
                                ia_iir_filter_struct* pstr_hp_filt_coeff,
                                WORD32 chan_idx, WORD32 frame_len,
                                FLOAT32* input, FLOAT32* output[]) {
  WORD32 i;
  FLOAT32 tmp, tmp1;
  FLOAT32 a1_l = pstr_lp_filt_coeff->a1;
  FLOAT32 a2_l = pstr_lp_filt_coeff->a2;
  FLOAT32 b0_l = pstr_lp_filt_coeff->b0;
  FLOAT32 b1_l = pstr_lp_filt_coeff->b1;
  FLOAT32 b2_l = pstr_lp_filt_coeff->b2;

  FLOAT32 st1_l = pstr_lp_filt_coeff->x_p[chan_idx * 2 + 0];
  FLOAT32 st2_l = pstr_lp_filt_coeff->x_p[chan_idx * 2 + 1];
  FLOAT32 st3_l = pstr_lp_filt_coeff->y_p[chan_idx * 2 + 0];
  FLOAT32 st4_l = pstr_lp_filt_coeff->y_p[chan_idx * 2 + 1];

  FLOAT32 a1_h = pstr_hp_filt_coeff->a1;
  FLOAT32 a2_h = pstr_hp_filt_coeff->a2;
  FLOAT32 b0_h = pstr_hp_filt_coeff->b0;
  FLOAT32 b1_h = pstr_hp_filt_coeff->b1;
  FLOAT32 b2_h = pstr_hp_filt_coeff->b2;

  FLOAT32 st1_h = pstr_hp_filt_coeff->x_p[chan_idx * 2 + 0];
  FLOAT32 st2_h = pstr_hp_filt_coeff->x_p[chan_idx * 2 + 1];
  FLOAT32 st3_h = pstr_hp_filt_coeff->y_p[chan_idx * 2 + 0];
  FLOAT32 st4_h = pstr_hp_filt_coeff->y_p[chan_idx * 2 + 1];

  FLOAT32* output_low = output[0];
  FLOAT32* output_high = output[1];

  for (i = 0; i < frame_len; i++) {
    tmp1 = input[i];
    tmp = b0_l * tmp1 + st1_l;
    st1_l = b1_l * tmp1 - a1_l * tmp + st2_l;
    st2_l = b2_l * tmp1 - a2_l * tmp;

    output_low[i] = b0_l * tmp + st3_l;
    st3_l = b1_l * tmp - a1_l * output_low[i] + st4_l;
    st4_l = b2_l * tmp - a2_l * output_low[i];

    tmp = b0_h * tmp1 + st1_h;
    st1_h = b1_h * tmp1 - a1_h * tmp + st2_h;
    st2_h = b2_h * tmp1 - a2_h * tmp;

    output_high[i] = b0_h * tmp + st3_h;
    st3_h = b1_h * tmp - a1_h * output_high[i] + st4_h;
    st4_h = b2_h * tmp - a2_h * output_high[i];
  }
  pstr_lp_filt_coeff->x_p[chan_idx * 2 + 0] = st1_l;
  pstr_lp_filt_coeff->x_p[chan_idx * 2 + 1] = st2_l;
  pstr_lp_filt_coeff->y_p[chan_idx * 2 + 0] = st3_l;
  pstr_lp_filt_coeff->y_p[chan_idx * 2 + 1] = st4_l;

  pstr_hp_filt_coeff->x_p[chan_idx * 2 + 0] = st1_h;
  pstr_hp_filt_coeff->x_p[chan_idx * 2 + 1] = st2_h;
  pstr_hp_filt_coeff->y_p[chan_idx * 2 + 0] = st3_h;
  pstr_hp_filt_coeff->y_p[chan_idx * 2 + 1] = st4_h;

  return;
}
VOID impd_two_band_filter_process(ia_two_band_filt_struct* str_two_band_bank,
                                  WORD32 chan_idx, WORD32 frame_len,
                                  FLOAT32* input, FLOAT32* output[]) {
  ia_iir_filter_struct* pstr_lp_filt_coeff = &str_two_band_bank->low_pass;
  ia_iir_filter_struct* pstr_hp_filt_coeff = &str_two_band_bank->high_pass;

  impd_apply_low_high_filter(pstr_lp_filt_coeff, pstr_hp_filt_coeff, chan_idx,
                             frame_len, input, output);
  return;
}

VOID impd_three_band_filter_process(
    ia_three_band_filt_struct* str_three_band_bank, WORD32 c, WORD32 size,
    FLOAT32* input, FLOAT32* output[]) {
  //    WORD32 err_code=0;
  ia_iir_filter_struct* all_pass_filter;
  ia_iir_filter_struct* pstr_lp_filt_coeff =
      &str_three_band_bank->str_low_pass_stage_1;
  ia_iir_filter_struct* pstr_hp_filt_coeff =
      &str_three_band_bank->str_high_pass_stage_1;
  FLOAT32* output1[2];
  output1[0] = output[0];
  output1[1] = output[1];

  impd_apply_low_high_filter(pstr_lp_filt_coeff, pstr_hp_filt_coeff, c, size,
                             input, output1);

  all_pass_filter = &str_three_band_bank->str_all_pass_stage_2;

  impd_iir_second_order_filter_all_pass(all_pass_filter, c, size, output1[1],
                                        output[2]);
  pstr_lp_filt_coeff = &str_three_band_bank->str_low_pass_stage_2;
  pstr_hp_filt_coeff = &str_three_band_bank->str_high_pass_stage_2;

  impd_apply_low_high_filter(pstr_lp_filt_coeff, pstr_hp_filt_coeff, c, size,
                             output1[0], output1);

  return;
}

VOID impd_four_band_filter_process(ia_four_band_filt_struct* str_four_band_bank,
                                   WORD32 cha_idx, WORD32 win_size,
                                   FLOAT32* input, FLOAT32* output[]) {
  //    WORD32 err_code=0;
  ia_iir_filter_struct* all_pass_filter;
  ia_iir_filter_struct* pstr_lp_filt_coeff =
      &str_four_band_bank->str_low_pass_stage_1;
  ia_iir_filter_struct* pstr_hp_filt_coeff =
      &str_four_band_bank->str_high_pass_stage_1;
  FLOAT32* output1[2];
  FLOAT32* output2[2];
  output1[0] = output[0];
  output1[1] = output[1];
  output2[0] = output[2];
  output2[1] = output[3];

  impd_apply_low_high_filter(pstr_lp_filt_coeff, pstr_hp_filt_coeff, cha_idx,
                             win_size, input, output1);

  all_pass_filter = &str_four_band_bank->str_all_pass_stage_2_low;

  impd_iir_second_order_filter_all_pass(all_pass_filter, cha_idx, win_size,
                                        output1[0], output1[0]);

  all_pass_filter = &str_four_band_bank->str_all_pass_stage_2_high;

  impd_iir_second_order_filter_all_pass(all_pass_filter, cha_idx, win_size,
                                        output1[1], output2[0]);

  pstr_lp_filt_coeff = &str_four_band_bank->str_low_pass_stage_3_low;
  pstr_hp_filt_coeff = &str_four_band_bank->str_high_pass_stage_3_low;

  impd_apply_low_high_filter(pstr_lp_filt_coeff, pstr_hp_filt_coeff, cha_idx,
                             win_size, output1[0], output1);

  pstr_lp_filt_coeff = &str_four_band_bank->str_low_pass_stage_3_high;
  pstr_hp_filt_coeff = &str_four_band_bank->str_high_pass_stage_3_high;

  impd_apply_low_high_filter(pstr_lp_filt_coeff, pstr_hp_filt_coeff, cha_idx,
                             win_size, output2[0], output2);

  return;
}

VOID impd_all_pass_cascade_process(
    ia_all_pass_cascade_struct* str_all_pass_cascade, WORD32 ch_idx,
    WORD32 win_size, FLOAT32* input) {
  WORD32 i;

  for (i = 0; i < str_all_pass_cascade->num_filter; i++) {
    impd_iir_second_order_filter_all_pass(
        &(str_all_pass_cascade->str_all_pass_cascade_filter[i]
              .str_all_pass_stage),
        ch_idx, win_size, input, input);
  }

  return;
}