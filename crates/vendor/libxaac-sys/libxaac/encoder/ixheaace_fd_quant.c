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
#include "iusace_type_def.h"
#include "ixheaac_error_standards.h"
#include "ixheaace_error_codes.h"
#include "ixheaace_mps_common_define.h"
#include "iusace_cnst.h"
#include "iusace_block_switch_const.h"
#include "iusace_bitbuffer.h"
#include "impd_drc_common_enc.h"
#include "impd_drc_uni_drc.h"
#include "impd_drc_api.h"
#include "impd_drc_uni_drc_eq.h"
#include "impd_drc_uni_drc_filter_bank.h"
#include "impd_drc_gain_enc.h"
#include "impd_drc_struct_def.h"

#include "ixheaace_memory_standards.h"
#include "iusace_tns_usac.h"
#include "iusace_psy_mod.h"
#include "iusace_config.h"
#include "iusace_ms.h"
#include "ixheaace_adjust_threshold_data.h"
#include "iusace_fd_qc_util.h"
#include "iusace_arith_enc.h"
#include "iusace_fd_quant.h"
#include "iusace_signal_classifier.h"
#include "iusace_block_switch_struct_def.h"
#include "ixheaace_sbr_header.h"
#include "ixheaace_config.h"
#include "ixheaace_asc_write.h"
#include "iusace_main.h"
#include "iusace_write_bitstream.h"
#include "ixheaace_nf.h"
#include "iusace_fd_qc_adjthr.h"
#include "iusace_block_switch_const.h"
#include "iusace_rom.h"
#include "ixheaace_cplx_pred.h"
#include "ixheaace_aac_constants.h"

static WORD32 iusace_window_shape[5] = {WIN_SEL_1, WIN_SEL_0, WIN_SEL_0, WIN_SEL_1, WIN_SEL_0};

static WORD32 iusace_count_ms_bits(WORD32 sfb_count, WORD32 sfb_per_grp, WORD32 max_sfb_per_grp) {
  WORD32 ms_bits = 0, sfb_offs, sfb;
  for (sfb_offs = 0; sfb_offs < sfb_count; sfb_offs += sfb_per_grp) {
    for (sfb = 0; sfb < max_sfb_per_grp; sfb++) {
      ms_bits++;
    }
  }
  return (ms_bits);
}

static WORD32 iusace_count_static_bits(ia_usac_data_struct *ptr_usac_data,
                                       ia_usac_encoder_config_struct *ptr_usac_config,
                                       ia_sfb_params_struct *pstr_sfb_params,
                                       ia_psy_mod_out_data_struct *pstr_psy_out, WORD32 channels,
                                       WORD32 chn, WORD32 usac_independency_flag, WORD32 ele_id) {
  WORD32 ms_mask = ptr_usac_data->str_ms_info[chn].ms_mask;
  WORD32 noise_filling = ptr_usac_data->noise_filling[ele_id];
  WORD32 stat_bits = 0, i;
  WORD32 tns_active = 0, tns_present_both = 0;
  WORD32 max_sfb = pstr_psy_out[chn].max_sfb_per_grp;
  (VOID) ptr_usac_config;

  if (channels == 1) {
    stat_bits += 1;  // core mode
    stat_bits += 1;  // tns active

    switch (pstr_psy_out[chn].window_sequence) {
      case ONLY_LONG_SEQUENCE:
      case LONG_START_SEQUENCE:
      case LONG_STOP_SEQUENCE:
        stat_bits += SI_ICS_INFO_BITS_LONG;
        break;
      case EIGHT_SHORT_SEQUENCE:
        stat_bits += SI_ICS_INFO_BITS_SHORT;
        break;
    }
  } else {
    stat_bits += 2;  // core mode
    stat_bits += 2;  // tns and common window

    switch (pstr_psy_out[chn].window_sequence) {
      case ONLY_LONG_SEQUENCE:
      case LONG_START_SEQUENCE:
      case LONG_STOP_SEQUENCE:
        stat_bits += SI_ICS_INFO_BITS_LONG;
        break;
      case EIGHT_SHORT_SEQUENCE:
        stat_bits += SI_ICS_INFO_BITS_SHORT;
        break;
    }

    stat_bits += 1;  // common_max_sfb
    stat_bits += SI_CPE_MS_MASK_BITS;

    if (ms_mask != 3) {
      if (ms_mask == 1) {
        stat_bits += iusace_count_ms_bits(pstr_psy_out[chn].sfb_count,
                                          pstr_psy_out[chn].sfb_per_group, max_sfb);
      }
    } else {
      stat_bits += iusace_write_cplx_pred_data(
          NULL, pstr_sfb_params->num_window_groups[chn], max_sfb,
          ptr_usac_data->complex_coef[chn], ptr_usac_data->pred_coef_re[chn],
          ptr_usac_data->pred_coef_im[chn], iusace_huffman_code_table, usac_independency_flag,
          ptr_usac_data->pred_dir_idx[chn], ptr_usac_data->cplx_pred_used[chn],
          ptr_usac_data->cplx_pred_all[chn], ptr_usac_data->temp_pred_coef_re_prev[chn],
          ptr_usac_data->temp_pred_coef_im_prev[chn], &ptr_usac_data->delta_code_time[chn]);
    }

    if (ptr_usac_data->pstr_tns_info[chn] != NULL &&
        ptr_usac_data->pstr_tns_info[chn + 1] != NULL) {
      tns_active = ptr_usac_data->pstr_tns_info[chn]->tns_data_present ||
                   ptr_usac_data->pstr_tns_info[chn + 1]->tns_data_present;
      tns_present_both = ptr_usac_data->pstr_tns_info[chn]->tns_data_present &&
                         ptr_usac_data->pstr_tns_info[chn + 1]->tns_data_present;
    }
    if (tns_active) {
      stat_bits += 1;  // common_tns

      stat_bits += 1;  // tns_present_both

      if (!tns_present_both) {
        stat_bits += 1;  // tns_data_present1
      }
    }
  }

  for (i = chn; i < chn + channels; i++) {
    stat_bits += 8;  // global_gain

    if (noise_filling) {
      stat_bits += 8;
    }
  }

  for (i = chn; i < chn + channels; i++) {
    if (ptr_usac_data->pstr_tns_info[i] != NULL &&
        ptr_usac_data->pstr_tns_info[i]->tns_data_present == 1) {
      stat_bits += iusace_write_tns_data(NULL, ptr_usac_data->pstr_tns_info[i],
                                         pstr_psy_out[i].window_sequence, 0);
    }

    if (!usac_independency_flag) {
      stat_bits += 1;  // arith_reset_flag
    }

    stat_bits += 1;  // fac_data_present
    stat_bits += ptr_usac_data->str_scratch.ptr_num_fac_bits[i];
  }

  stat_bits += (ptr_usac_data->num_sbr_bits + ptr_usac_data->num_drc_bits);

  return stat_bits;
}

static VOID iusace_sort_for_grouping(WORD32 *sfb_offset, const WORD32 *sfb_width_table,
                                     FLOAT64 *ptr_scratch, FLOAT64 *ptr_spec,
                                     WORD32 num_window_groups, const WORD32 *window_group_length,
                                     WORD32 nr_of_sfb, WORD32 ccfl) {
  WORD32 i, j, ii;
  WORD32 index = 0;
  WORD32 group_offset = 0;
  WORD32 k = 0;
  WORD32 frame_len_short = (ccfl * FRAME_LEN_SHORT_128) / FRAME_LEN_LONG;

  sfb_offset[k] = 0;
  for (k = 1; k < nr_of_sfb + 1; k++) {
    sfb_offset[k] = sfb_offset[k - 1] + sfb_width_table[k - 1];
  }

  index = 0;
  group_offset = 0;
  for (i = 0; i < num_window_groups; i++) {
    for (k = 0; k < nr_of_sfb; k++) {
      for (j = 0; j < window_group_length[i]; j++) {
        for (ii = 0; ii < sfb_width_table[k]; ii++) {
          ptr_scratch[index++] =
              ptr_spec[ii + sfb_offset[k] + frame_len_short * j + group_offset];
        }
      }
    }
    group_offset += frame_len_short * window_group_length[i];
  }

  memcpy(ptr_spec, ptr_scratch, ccfl * sizeof(ptr_spec[0]));

  index = 0;
  sfb_offset[index++] = 0;
  for (i = 0; i < num_window_groups; i++) {
    for (k = 0; k < nr_of_sfb; k++) {
      sfb_offset[index] = sfb_offset[index - 1] + sfb_width_table[k] * window_group_length[i];
      index++;
    }
  }

  return;
}

static VOID iusace_degroup_int(const WORD32 *ptr_grouped_sfb_offsets, WORD32 sfb_per_group,
                               WORD32 *ptr_scratch, WORD32 *ptr_spec, WORD32 num_window_groups,
                               const WORD32 *window_group_length, WORD32 ccfl)

{
  WORD32 i, j, k, n;
  WORD32 index, group_offset;
  WORD32 loop1, loop2;
  WORD32 frame_len_short = (ccfl * FRAME_LEN_SHORT_128) / FRAME_LEN_LONG;
  index = 0;
  group_offset = 0;

  memset(ptr_scratch, 0, ccfl * sizeof(WORD32));

  for (i = 0; i < num_window_groups; i++) {
    for (j = 0; j < sfb_per_group; j++) {
      WORD32 idx = i * sfb_per_group + j;
      loop1 = ((ptr_grouped_sfb_offsets[idx + 1] - ptr_grouped_sfb_offsets[idx]) /
               window_group_length[i]);

      for (k = 0; k < window_group_length[i]; k++) {
        loop2 = ((ptr_grouped_sfb_offsets[idx] - group_offset) / window_group_length[i]) +
                frame_len_short * k + group_offset;

        for (n = 0; n < loop1; n++) {
          ptr_scratch[n + loop2] = ptr_spec[index++];
        }
      }
    }
    group_offset += frame_len_short * window_group_length[i];
  }

  memcpy(ptr_spec, ptr_scratch, ccfl * sizeof(WORD32));

  return;
}

IA_ERRORCODE iusace_stereo_proc(ia_sfb_params_struct *pstr_sfb_prms,
                                WORD32 usac_independancy_flag, ia_usac_data_struct *ptr_usac_data,
                                ia_usac_encoder_config_struct *ptr_usac_config, WORD32 chn) {
  IA_ERRORCODE err_code;
  WORD32 i = 0;
  WORD32 j = 0;
  iusace_scratch_mem *pstr_scratch = &ptr_usac_data->str_scratch;
  ia_psy_mod_data_struct *pstr_psy_data = ptr_usac_data->str_psy_mod.str_psy_data;
  ia_psy_mod_out_data_struct *pstr_psy_out = ptr_usac_data->str_psy_mod.str_psy_out_data;
  WORD32 *ptr_num_sfb = pstr_sfb_prms->num_sfb;
  WORD32 *ptr_num_window_groups = pstr_sfb_prms->num_window_groups;
  WORD32 ccfl = ptr_usac_config->ccfl;
  ia_ms_info_struct *pstr_ms_info = &ptr_usac_data->str_ms_info[chn];

  FLOAT64 tmp = 0.0f;
  FLOAT32 nrg_mid = 0.0f, nrg_side = 0.0f, nrg_left = 0.0f, nrg_right = 0.0f;
  FLOAT64 *ptr_scratch_spec = pstr_scratch->p_quant_spectrum_spec_scratch;
  FLOAT32 ratio_mid = 0.0f, ratio_side = 0.0f;
  FLOAT32 eps = 1.0e-6f;
  /* Save a copy of left and right channel MDCT spectra before they are modified */
  memcpy(ptr_usac_data->left_chan_save[chn], ptr_usac_data->spectral_line_vector[chn],
         ccfl * sizeof(FLOAT64));
  memcpy(ptr_usac_data->right_chan_save[chn], ptr_usac_data->spectral_line_vector[chn + 1],
         ccfl * sizeof(FLOAT64));

  if (ptr_usac_config->cmplx_pred_flag == 1) {
    /* Refinement - decision on whether to use complex prediction or MS */
    for (i = 0; i < ccfl; i++) {
      tmp = ptr_usac_data->spectral_line_vector[chn][i];
      ptr_scratch_spec[i] = 0.5f * (ptr_usac_data->spectral_line_vector[chn][i] +
                                    ptr_usac_data->spectral_line_vector[chn + 1][i]);
      ptr_scratch_spec[ccfl + i] = 0.5f * (tmp - ptr_usac_data->spectral_line_vector[chn + 1][i]);
    }

    for (i = 0; i < ccfl; i++) {
      nrg_mid += (FLOAT32)(ptr_scratch_spec[i] * ptr_scratch_spec[i]);
      nrg_side += (FLOAT32)(ptr_scratch_spec[ccfl + i] * ptr_scratch_spec[ccfl + i]);
      nrg_left += (FLOAT32)(ptr_usac_data->spectral_line_vector[chn][i] *
                            ptr_usac_data->spectral_line_vector[chn][i]);
      nrg_right += (FLOAT32)(ptr_usac_data->spectral_line_vector[chn + 1][i] *
                             ptr_usac_data->spectral_line_vector[chn + 1][i]);
    }

    ratio_mid = nrg_mid / (MAX(nrg_left, nrg_right) + eps);
    ratio_side = nrg_side / (MAX(nrg_left, nrg_right) + eps);

    if (ratio_mid >= 0.8f || ratio_side >= 0.8f || nrg_mid == 0.f || nrg_side == 0.f) {
      pstr_ms_info->ms_mask = 0;
    } else {
      pstr_ms_info->ms_mask = 3;
    }
  }

  if (pstr_ms_info->ms_mask != 3) {
    WORD32 idx = 0;
    WORD32 sfb;

    for (sfb = 0; sfb < ptr_num_sfb[chn]; sfb++) {
      ptr_usac_data->pred_coef_re_prev[chn][sfb] = 0;
      ptr_usac_data->pred_coef_im_prev[chn][sfb] = 0;
      ptr_usac_data->temp_pred_coef_re_prev[chn][sfb] = 0;
      ptr_usac_data->temp_pred_coef_im_prev[chn][sfb] = 0;
    }

    memset(ptr_usac_data->str_ms_info[chn].ms_used, 0,
           MAX_SFB_LONG * MAX_SHORT_WINDOWS * sizeof(WORD32));

    iusace_ms_apply(pstr_psy_data, ptr_usac_data->spectral_line_vector[chn],
                    ptr_usac_data->spectral_line_vector[chn + 1],
                    &ptr_usac_data->str_ms_info[chn].ms_mask,
                    ptr_usac_data->str_ms_info[chn].ms_used,
                    ptr_num_window_groups[chn] * ptr_num_sfb[chn], ptr_num_sfb[chn],
                    pstr_psy_out[chn].max_sfb_per_grp, pstr_sfb_prms->grouped_sfb_offset[chn],
                    chn, ptr_usac_config->cmplx_pred_flag == 1 ? ptr_scratch_spec : NULL);

    for (i = 0; i < ptr_num_window_groups[chn]; i++) {
      for (j = 0; j < ptr_num_sfb[chn]; j++) {
        pstr_psy_out[chn].ms_used[idx++] = ptr_usac_data->str_ms_info[chn].ms_used[i][j];
      }
    }
  } else {
    /* Reset buffer to zero */
    for (WORD32 group = 0; group < MAX_SHORT_WINDOWS; group++) {
      memset(ptr_usac_data->cplx_pred_used[chn][group], 0, MAX_SFB_LONG * sizeof(WORD32));
    }

    ptr_usac_data->cplx_pred_all[chn] = 1; /* Disable bandwise switching to L/R */
    for (i = 0; i < ptr_num_window_groups[chn]; i++) {
      for (j = 0; j < ptr_num_sfb[chn]; j += 2) {
        ptr_usac_data->cplx_pred_used[chn][i][j] = 1;
        if ((j + 1) < ptr_num_sfb[chn]) {
          ptr_usac_data->cplx_pred_used[chn][i][j + 1] = ptr_usac_data->cplx_pred_used[chn][i][j];
        }
      }
    }

    err_code = iusace_cplx_pred_proc(
        ptr_usac_data, ptr_usac_config, usac_independancy_flag, pstr_sfb_prms, chn, pstr_psy_data,
        pstr_sfb_prms->grouped_sfb_offset[chn], pstr_scratch->p_cmpx_mdct_temp_buf,
        ptr_scratch_spec, nrg_mid, nrg_side);
    if (err_code != IA_NO_ERROR) {
      return err_code;
    }
  }
  return IA_NO_ERROR;
}

IA_ERRORCODE iusace_grouping(ia_sfb_params_struct *pstr_sfb_prms, WORD32 num_chans,
                             ia_usac_data_struct *ptr_usac_data,
                             ia_usac_encoder_config_struct *ptr_usac_config, WORD32 chn,
                             WORD32 ele_id) {
  WORD32 i = 0, grp, sfb, wnd;
  WORD32 j = 0;
  WORD32 k;
  WORD32 ch;
  ia_psy_mod_struct *pstr_psy_config = &ptr_usac_data->str_psy_mod;
  ia_psy_mod_data_struct *pstr_psy_data = ptr_usac_data->str_psy_mod.str_psy_data;
  WORD32 *ptr_window_sequence = pstr_sfb_prms->window_sequence;
  WORD32 *ptr_num_sfb = pstr_sfb_prms->num_sfb;
  WORD32 *ptr_num_window_groups = pstr_sfb_prms->num_window_groups;
  ia_psy_mod_out_data_struct *pstr_psy_out = ptr_usac_data->str_psy_mod.str_psy_out_data;

  for (ch = chn; ch < chn + num_chans; ch++) {
    if (ptr_window_sequence[ch] == EIGHT_SHORT_SEQUENCE) {
      iusace_sort_for_grouping(
          pstr_sfb_prms->grouped_sfb_offset[ch], pstr_sfb_prms->sfb_width_table[ch],
          ptr_usac_data->str_scratch.p_sort_grouping_scratch,
          ptr_usac_data->spectral_line_vector[ch], ptr_num_window_groups[ch],
          pstr_sfb_prms->window_group_length[ch], ptr_num_sfb[ch], ptr_usac_config->ccfl);
    } else if ((ptr_window_sequence[ch] == ONLY_LONG_SEQUENCE) ||
               (ptr_window_sequence[ch] == LONG_START_SEQUENCE) ||
               (ptr_window_sequence[ch] == LONG_STOP_SEQUENCE) ||
               (ptr_window_sequence[ch] == STOP_START_SEQUENCE)) {
      pstr_sfb_prms->grouped_sfb_offset[ch][0] = 0;
      k = 0;
      for (i = 0; i < ptr_num_sfb[ch]; i++) {
        pstr_sfb_prms->grouped_sfb_offset[ch][i] = k;
        k += pstr_sfb_prms->sfb_width_table[ch][i];
      }
      pstr_sfb_prms->grouped_sfb_offset[ch][i] = k;
    } else {
      return IA_EXHEAACE_EXE_FATAL_USAC_INVALID_WINDOW_TYPE;
    }
  }

  for (ch = chn; ch < chn + num_chans; ch++) {
    if (pstr_psy_data[ch].window_sequence == 2) {
      i = 0;
      for (grp = 0; grp < ptr_num_window_groups[ch]; grp++) {
        for (sfb = 0; sfb < ptr_num_sfb[ch]; sfb++) {
          pstr_psy_out[ch].sfb_min_snr[i++] =
              pstr_psy_config->str_psy_short_config[ele_id].sfb_min_snr[sfb];
        }
      }
      wnd = 0;
      i = 0;
      for (grp = 0; grp < ptr_num_window_groups[ch]; grp++) {
        for (sfb = 0; sfb < ptr_num_sfb[ch]; sfb++) {
          FLOAT32 threshold = pstr_psy_data[ch].sfb_thr_short[wnd][sfb];
          FLOAT32 energy = pstr_psy_data[ch].sfb_energy_short[wnd][sfb];
          FLOAT32 energy_ms = pstr_psy_data[ch].ptr_sfb_energy_short_ms[wnd][sfb];
          FLOAT32 spread_energy = pstr_psy_data[ch].sfb_spreaded_energy_short[wnd][sfb];
          for (j = 1; j < pstr_sfb_prms->window_group_length[ch][grp]; j++) {
            threshold = threshold + pstr_psy_data[ch].sfb_thr_short[wnd + j][sfb];
            energy = energy + pstr_psy_data[ch].sfb_energy_short[wnd + j][sfb];
            spread_energy =
                spread_energy + pstr_psy_data[ch].sfb_spreaded_energy_short[wnd + j][sfb];
            energy_ms = energy_ms + pstr_psy_data[ch].ptr_sfb_energy_short_ms[wnd + j][sfb];
          }
          pstr_psy_data[ch].ptr_sfb_thr_long[i] = threshold;
          pstr_psy_data[ch].ptr_sfb_energy_long[i] = energy;
          pstr_psy_data[ch].ptr_sfb_energy_long_ms[i] = energy_ms;
          pstr_psy_data[ch].ptr_sfb_spreaded_energy_long[i++] = spread_energy;
        }
        wnd += pstr_sfb_prms->window_group_length[ch][grp];
      }
    } else {
      for (sfb = 0; sfb < ptr_num_sfb[ch]; sfb++) {
        pstr_psy_out[ch].sfb_min_snr[sfb] =
            pstr_psy_config->str_psy_long_config[ele_id].sfb_min_snr[sfb];
      }
    }
  }
  return IA_NO_ERROR;
}

IA_ERRORCODE iusace_quantize_spec(ia_sfb_params_struct *pstr_sfb_prms,
                                  WORD32 usac_independancy_flag, WORD32 num_chans,
                                  ia_usac_data_struct *ptr_usac_data,
                                  ia_usac_encoder_config_struct *ptr_usac_config, WORD32 chn,
                                  WORD32 ele_id, WORD32 *is_quant_spec_zero,
                                  WORD32 *is_gain_limited) {
  IA_ERRORCODE err_code;
  WORD32 i = 0, sfb;
  WORD32 j = 0;
  WORD32 k;
  WORD32 max_bits;
  WORD32 ch;
  iusace_scratch_mem *pstr_scratch = &ptr_usac_data->str_scratch;
  WORD32 num_scfs[2];
  FLOAT32 **sfb_form_fac = &pstr_scratch->ptr_sfb_form_fac[0];
  WORD32 max_ch_dyn_bits[2] = {0};
  FLOAT32 ch_bit_dist[2];
  WORD32 constraints_fulfilled;
  WORD32 iterations = 0;
  WORD32 max_val;
  WORD32 kk, idx = 0;

  FLOAT32 *ptr_exp_spec = pstr_scratch->p_exp_spec;
  FLOAT32 *ptr_mdct_spec_float = pstr_scratch->p_mdct_spec_float;
  ia_psy_mod_data_struct *pstr_psy_data = ptr_usac_data->str_psy_mod.str_psy_data;
  ia_qc_out_data_struct *pstr_qc_out = &ptr_usac_data->str_qc_main.str_qc_out;
  ia_psy_mod_out_data_struct *pstr_psy_out = ptr_usac_data->str_psy_mod.str_psy_out_data;
  ia_qc_data_struct *pstr_qc_data = &ptr_usac_data->str_qc_main.str_qc_data[ele_id];
  ia_adj_thr_elem_struct *pstr_adj_thr_elem = &pstr_qc_data->str_adj_thr_ele;
  WORD32 *ptr_window_sequence = pstr_sfb_prms->window_sequence;
  WORD32 *ptr_max_sfb = pstr_sfb_prms->max_sfb;
  WORD32 *ptr_num_sfb = pstr_sfb_prms->num_sfb;
  WORD32 *ptr_num_window_groups = pstr_sfb_prms->num_window_groups;
  WORD32 bitres_bits, bitres_diff;
  WORD32 gain;

  memset(num_scfs, 0, 2 * sizeof(num_scfs[0]));

  for (ch = chn; ch < chn + num_chans; ch++) {
    num_scfs[idx] = ptr_num_sfb[ch] * ptr_num_window_groups[ch];

    pstr_psy_out[ch].sfb_count = num_scfs[idx];
    pstr_psy_out[ch].sfb_per_group = num_scfs[idx] / ptr_num_window_groups[ch];
    pstr_psy_out[ch].window_sequence = pstr_psy_data[ch].window_sequence;
    pstr_psy_out[ch].window_shape = iusace_window_shape[pstr_psy_data[ch].window_sequence];
    pstr_psy_out[ch].ptr_spec_coeffs = ptr_usac_data->spectral_line_vector[ch];
    pstr_psy_out[ch].ptr_sfb_energy = pstr_psy_data[ch].ptr_sfb_energy_long;
    pstr_psy_out[ch].ptr_sfb_thr = pstr_psy_data[ch].ptr_sfb_thr_long;
    pstr_psy_out[ch].ptr_sfb_spread_energy = pstr_psy_data[ch].ptr_sfb_spreaded_energy_long;

    for (j = 0; j < num_scfs[idx]; j++) {
      pstr_psy_out[ch].sfb_offsets[j] = pstr_sfb_prms->grouped_sfb_offset[ch][j];
    }
    pstr_psy_out[ch].sfb_offsets[num_scfs[idx]] =
        pstr_sfb_prms->grouped_sfb_offset[ch][num_scfs[idx]];

    for (j = 0; j < MAX_NUM_GROUPED_SFB; j++) {
      sfb_form_fac[idx][j] = MIN_FLT_VAL;
    }

    iusace_calc_form_fac_per_chan(&pstr_psy_out[ch], pstr_scratch, idx);
    idx++;
  }

  pstr_qc_out->static_bits =
      iusace_count_static_bits(ptr_usac_data, ptr_usac_config, pstr_sfb_prms, pstr_psy_out,
                               num_chans, chn, usac_independancy_flag, ele_id);

  iusace_adj_bitrate(pstr_qc_data, pstr_qc_data->ch_bitrate, ptr_usac_config->core_sample_rate,
                     ptr_usac_config->ccfl);
  err_code =
      iusace_adj_thr(pstr_adj_thr_elem, pstr_psy_out, ch_bit_dist, pstr_qc_out,
                     pstr_qc_data->avg_bits - pstr_qc_out->static_bits, pstr_qc_data->bit_res_lvl,
                     pstr_qc_data->max_bitres_bits, pstr_qc_out->static_bits,
                     &pstr_qc_data->max_bit_fac, num_chans, chn, pstr_scratch);
  if (err_code != IA_NO_ERROR) {
    return err_code;
  }

  iusace_estimate_scfs_chan(pstr_psy_out, pstr_qc_out->str_qc_out_chan, num_chans, chn,
                            pstr_scratch);
  idx = 0;
  for (ch = 0; ch < num_chans; ch++) {
    max_ch_dyn_bits[ch] = (WORD32)floor(
        ch_bit_dist[ch] * (FLOAT32)(pstr_qc_data->avg_bits + pstr_qc_data->bit_res_lvl - 7 -
                                    pstr_qc_out->static_bits));
    idx++;
  }

  pstr_qc_out->dyn_bits = 0;
  idx = 0;
  for (ch = chn; ch < chn + num_chans; ch++) {
    iterations = 0;
    gain = 0;
    for (kk = 0; kk < ptr_usac_config->ccfl; kk++) {
      ptr_exp_spec[kk] = (FLOAT32)pstr_psy_out[ch].ptr_spec_coeffs[kk];
      ptr_mdct_spec_float[kk] = (FLOAT32)pstr_psy_out[ch].ptr_spec_coeffs[kk];
    }
    do {
      constraints_fulfilled = 1;
      WORD32 quant_spec_is_zero = 1;
      if (iterations > 0) {
        for (WORD32 sfb_offs = 0; sfb_offs < pstr_psy_out[ch].sfb_count;
             sfb_offs += pstr_psy_out[ch].sfb_per_group) {
          for (sfb = 0; sfb < pstr_psy_out[ch].max_sfb_per_grp; sfb++) {
            WORD32 scalefactor = pstr_qc_out->str_qc_out_chan[idx].scalefactor[sfb + sfb_offs];
            gain = MAX(gain, pstr_qc_out->str_qc_out_chan[idx].global_gain - scalefactor);
            iusace_quantize_lines(
                pstr_qc_out->str_qc_out_chan[idx].global_gain - scalefactor,
                pstr_psy_out[ch].sfb_offsets[sfb_offs + sfb + 1] -
                    pstr_psy_out[ch].sfb_offsets[sfb_offs + sfb],
                ptr_exp_spec + pstr_psy_out[ch].sfb_offsets[sfb_offs + sfb],
                pstr_qc_out->str_qc_out_chan[idx].quant_spec +
                    pstr_psy_out[ch].sfb_offsets[sfb_offs + sfb],
                ptr_mdct_spec_float + pstr_psy_out[ch].sfb_offsets[sfb_offs + sfb]);
          }
        }
      }
      max_val =
          iusace_calc_max_val_in_sfb(pstr_psy_out[ch].sfb_count, pstr_psy_out[ch].max_sfb_per_grp,
                                     pstr_psy_out[ch].sfb_per_group, pstr_psy_out[ch].sfb_offsets,
                                     pstr_qc_out->str_qc_out_chan[idx].quant_spec);
      if (max_val > MAX_QUANT) {
        constraints_fulfilled = 0;
      }

      for (k = 0; k < num_scfs[idx]; k++) {
        for (i = pstr_sfb_prms->grouped_sfb_offset[ch][k];
             i < pstr_sfb_prms->grouped_sfb_offset[ch][k + 1]; i++) {
          ptr_usac_data->str_quant_info[idx].quant_degroup[i] =
              (WORD32)pstr_qc_out->str_qc_out_chan[idx].quant_spec[i];
          if (ptr_usac_data->str_quant_info[idx].quant_degroup[i] != 0) {
            quant_spec_is_zero = 0;
          }
        }
      }

      if (ptr_window_sequence[ch] == EIGHT_SHORT_SEQUENCE) {
        iusace_degroup_int(pstr_sfb_prms->grouped_sfb_offset[ch], ptr_num_sfb[ch],
                           ptr_usac_data->str_scratch.p_degroup_scratch,
                           ptr_usac_data->str_quant_info[idx].quant_degroup,
                           ptr_num_window_groups[ch], pstr_sfb_prms->window_group_length[ch],
                           ptr_usac_config->ccfl);
      }

      ptr_usac_data->str_quant_info[idx].max_spec_coeffs = 0;
      for (k = 0; k < ptr_max_sfb[ch]; k++) {
        ptr_usac_data->str_quant_info[idx].max_spec_coeffs +=
            pstr_sfb_prms->sfb_width_table[ch][k];
      }

      for (i = 0; i < num_scfs[idx]; i++) {
        ptr_usac_data->str_quant_info[idx].scale_factor[i] =
            pstr_qc_out->str_qc_out_chan[idx].global_gain -
            pstr_qc_out->str_qc_out_chan[idx].scalefactor[i] + SF_OFFSET;
      }

      max_bits = iusace_count_fd_bits(pstr_sfb_prms, ptr_usac_data, usac_independancy_flag,
                                      ptr_usac_config, ch, idx);

      if (max_bits > max_ch_dyn_bits[idx]) {
        constraints_fulfilled = 0;
      }
      if (quant_spec_is_zero == 1) {
        constraints_fulfilled = 1;
        /*Bit consuption is exceding bit reservoir, there is no scope left for bit consumption
          reduction, as spectrum is zero. Hence breaking the quantization loop. */
        if (iterations > 0) {
          *is_quant_spec_zero = 1;
          max_bits = max_ch_dyn_bits[idx];
        }
      }
      if ((gain == MAX_GAIN_INDEX) && (constraints_fulfilled == 0)) {
        /* Bit consuption is exceding bit reservoir, there is no scope left for bit consumption
           reduction, as gain has reached the maximum value. Hence breaking the quantization
           loop. */
        constraints_fulfilled = 1;
        *is_gain_limited = 1;
        max_bits = max_ch_dyn_bits[idx];
      }
      if (!constraints_fulfilled) {
        pstr_qc_out->str_qc_out_chan[idx].global_gain++;
      }
      iterations++;
    } while (!constraints_fulfilled);

    pstr_qc_out->dyn_bits += max_bits;

    if (ptr_usac_data->noise_filling[ele_id]) {
      WORD32 max_nf_sfb = ptr_max_sfb[ch];

      if (ptr_window_sequence[ch] != EIGHT_SHORT_SEQUENCE) {
        iusace_noise_filling(
            &ptr_usac_data->noise_level[idx], &ptr_usac_data->noise_offset[idx],
            ptr_usac_data->spectral_line_vector[ch], &ptr_usac_data->str_quant_info[idx],
            pstr_sfb_prms->grouped_sfb_offset[ch], max_nf_sfb, ptr_usac_config->ccfl,
            ptr_num_window_groups[ch], pstr_sfb_prms->window_group_length[ch], 160,
            ptr_usac_data->str_scratch.p_noise_filling_highest_tone);
      } else {
        iusace_noise_filling(
            &ptr_usac_data->noise_level[idx], &ptr_usac_data->noise_offset[idx],
            ptr_usac_data->spectral_line_vector[ch], &ptr_usac_data->str_quant_info[idx],
            pstr_sfb_prms->grouped_sfb_offset[ch], max_nf_sfb, ptr_usac_config->ccfl >> 3,
            ptr_num_window_groups[ch], pstr_sfb_prms->window_group_length[ch], 20,
            (FLOAT64 *)ptr_usac_data->str_scratch.p_noise_filling_highest_tone);
      }

      if (ptr_usac_data->noise_level[idx] == 0 && ptr_usac_data->noise_offset[idx] != 0 &&
          pstr_sfb_prms->common_win[ch]) {
        ptr_usac_data->complex_coef[ch] = 0;
      }
    }
    idx++;
  }

  pstr_adj_thr_elem->dyn_bits_last = pstr_qc_out->dyn_bits;

  bitres_bits = pstr_qc_data->max_bitres_bits - pstr_qc_data->bit_res_lvl;
  bitres_diff = pstr_qc_data->avg_bits - (pstr_qc_out->static_bits + pstr_qc_out->dyn_bits);
  pstr_qc_out->fill_bits = MAX(0, (bitres_diff - bitres_bits));

  if (pstr_qc_data->avg_bits > 0) {
    pstr_qc_data->bit_res_lvl +=
        pstr_qc_data->avg_bits -
        (pstr_qc_out->static_bits + pstr_qc_out->dyn_bits + pstr_qc_out->fill_bits);
  } else {
    pstr_qc_data->bit_res_lvl = pstr_qc_data->max_bits;
  }

  if (pstr_qc_data->bit_res_lvl < 0 ||
      pstr_qc_data->bit_res_lvl > pstr_qc_data->max_bitres_bits) {
    return IA_EXHEAACE_EXE_FATAL_USAC_INVALID_BIT_RSVR_LVL;
  }

  return IA_NO_ERROR;
}
