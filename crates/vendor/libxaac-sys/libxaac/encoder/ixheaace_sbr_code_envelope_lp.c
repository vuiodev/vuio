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

#include "ixheaac_type_def.h"
#include "ixheaac_constants.h"
#include "ixheaace_aac_constants.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops16.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaac_basic_ops.h"
#include "ixheaac_error_standards.h"

#include "ixheaace_sbr_header.h"
#include "ixheaace_sbr_def.h"
#include "ixheaace_resampler.h"
#include "ixheaace_sbr_rom.h"
#include "ixheaace_common_rom.h"
#include "ixheaace_sbr_main.h"
#include "ixheaace_sbr_frame_info_gen.h"
#include "ixheaace_sbr_hbe.h"
#include "ixheaace_sbr_code_envelope.h"
#include "ixheaace_sbr_qmf_enc.h"
#include "ixheaace_sbr_tran_det.h"
#include "ixheaace_sbr_env_est.h"
#include "ixheaace_sbr_missing_harmonics_det.h"
#include "ixheaace_sbr_inv_filtering_estimation.h"
#include "ixheaace_sbr_noise_floor_est.h"

#include "ixheaace_sbr_ton_corr.h"
#include "iusace_esbr_pvc.h"
#include "iusace_esbr_inter_tes.h"
#include "ixheaace_sbr.h"
#include "ixheaace_common_utils.h"

static WORD32 ixheaace_index_low2_high(WORD32 offset, WORD32 index, ixheaace_freq_res res) {
  switch (res) {
    case FREQ_RES_LOW:
      if (offset >= 0) {
        return (index < offset) ? (index) : (2 * index - offset);
      } else {
        offset = -offset;
        return (index < offset) ? (3 * index) : (2 * index + offset);
      }
    default:
      return (index);
  }
}

IA_ERRORCODE ixheaace_code_envelope(WORD32 *ptr_sfb_energy, const ixheaace_freq_res *freq_res,
                                    ixheaace_str_sbr_code_envelope *pstr_code_env,
                                    WORD32 *ptr_dir_vec, WORD32 coupling, WORD32 num_envelopes,
                                    WORD32 channel, WORD32 header_active, WORD32 usac_indep_flag,
                                    WORD32 is_ld_sbr) {
  IA_ERRORCODE err_code = IA_NO_ERROR;
  WORD32 i, no_of_bands, band, last_nrg, current_energy;
  WORD32 *ptr_energy;

  WORD32 code_book_scf_lav_lvl_time;
  WORD32 code_book_scf_lav_lvl_freq;
  WORD32 code_book_scf_lav_bal_time;
  WORD32 code_book_scf_lav_bal_freq;
  const UWORD8 *ptr_huff_tab_lvl_time_l;
  const UWORD8 *ptr_huff_tab_bal_time_l;
  const UWORD8 *ptr_huff_tab_lvl_freq_l;
  const UWORD8 *ptr_huff_tab_bal_freq_l;

  WORD32 offset = pstr_code_env->offset;
  WORD32 env_data_tab_comp_factor;

  WORD32 delta_f_bits = 0, delta_t_bits = 0;

  WORD32 use_dt;

  WORD32 delta_f[MAXIMUM_FREQ_COEFFS];
  WORD32 delta_t[MAXIMUM_FREQ_COEFFS];

  FLOAT32 df_edge_first_ennv;

  df_edge_first_ennv = pstr_code_env->df_edge_1st_env +
                       pstr_code_env->df_edge_incr * pstr_code_env->df_edge_incr_fac;

  switch (coupling) {
    case COUPLING_1:
      code_book_scf_lav_lvl_time = pstr_code_env->code_book_scf_lav_lvl_time;
      code_book_scf_lav_lvl_freq = pstr_code_env->code_book_scf_lav_lvl_freq;
      code_book_scf_lav_bal_time = pstr_code_env->code_book_scf_lav_bal_time;
      code_book_scf_lav_bal_freq = pstr_code_env->code_book_scf_lav_bal_freq;
      ptr_huff_tab_lvl_time_l = pstr_code_env->ptr_huff_tab_lvl_time_l;
      ptr_huff_tab_bal_time_l = pstr_code_env->ptr_huff_tab_bal_time_l;
      ptr_huff_tab_lvl_freq_l = pstr_code_env->ptr_huff_tab_lvl_freq_l;
      ptr_huff_tab_bal_freq_l = pstr_code_env->ptr_huff_tab_bal_freq_l;
      break;
    default:
      code_book_scf_lav_lvl_time = pstr_code_env->code_book_scf_lav_time;
      code_book_scf_lav_lvl_freq = pstr_code_env->code_book_scf_lav_freq;
      code_book_scf_lav_bal_time = pstr_code_env->code_book_scf_lav_time;
      code_book_scf_lav_bal_freq = pstr_code_env->code_book_scf_lav_freq;
      ptr_huff_tab_lvl_time_l = pstr_code_env->ptr_huff_tab_time_l;
      ptr_huff_tab_bal_time_l = pstr_code_env->ptr_huff_tab_time_l;
      ptr_huff_tab_lvl_freq_l = pstr_code_env->ptr_huff_tab_freq_l;
      ptr_huff_tab_bal_freq_l = pstr_code_env->ptr_huff_tab_freq_l;
  }

  if (coupling == 1 && channel == 1) {
    env_data_tab_comp_factor = 1;
  } else {
    env_data_tab_comp_factor = 0;
  }

  if (pstr_code_env->delta_t_across_frames == 0 || header_active) {
    pstr_code_env->update = 0;
  }

  for (i = 0; i < num_envelopes; i++) {
    if (freq_res[i] == FREQ_RES_HIGH)
      no_of_bands = pstr_code_env->num_scf[FREQ_RES_HIGH];
    else
      no_of_bands = pstr_code_env->num_scf[FREQ_RES_LOW];

    ptr_energy = ptr_sfb_energy;
    current_energy = *ptr_energy;
    delta_f[0] = ixheaac_shr32(current_energy, env_data_tab_comp_factor);
    if (coupling && channel == 1)
      delta_f_bits = pstr_code_env->start_bits_balance;
    else
      delta_f_bits = pstr_code_env->start_bits;

    if (pstr_code_env->update != 0) {
      delta_t[0] = ixheaac_shr32((current_energy - pstr_code_env->sfb_nrg_prev[0]),
                                 env_data_tab_comp_factor);

      err_code = ixheaace_compute_bits(delta_t[0], code_book_scf_lav_lvl_time,
                                       code_book_scf_lav_bal_time, ptr_huff_tab_lvl_time_l,
                                       ptr_huff_tab_bal_time_l, coupling, channel, &delta_t_bits);
      if (err_code) {
        return err_code;
      }
    }

    ixheaace_map_low_res_energy_value(current_energy, pstr_code_env->sfb_nrg_prev, offset, 0,
                                      freq_res[i]);

    band = no_of_bands - 1;
    while (band > 0) {
      if (ptr_energy[band] - ptr_energy[band - 1] > code_book_scf_lav_lvl_freq) {
        ptr_energy[band - 1] = ptr_energy[band] - code_book_scf_lav_lvl_freq;
      }
      band--;
    }

    band = 1;
    while (band < no_of_bands) {
      WORD32 index;
      WORD32 delta_lcl_bits = 0;

      if (ixheaac_sub32_sat(ptr_energy[band - 1], ptr_energy[band]) >
          code_book_scf_lav_lvl_freq) {
        ptr_energy[band] = ptr_energy[band - 1] - code_book_scf_lav_lvl_freq;
      }

      last_nrg = *ptr_energy++;
      current_energy = *ptr_energy;
      delta_f[band] = ixheaac_shr32((current_energy - last_nrg), env_data_tab_comp_factor);

      err_code = ixheaace_compute_bits(
          delta_f[band], code_book_scf_lav_lvl_freq, code_book_scf_lav_bal_freq,
          ptr_huff_tab_lvl_freq_l, ptr_huff_tab_bal_freq_l, coupling, channel, &delta_lcl_bits);
      if (err_code) {
        return err_code;
      }
      delta_f_bits += delta_lcl_bits;

      if (pstr_code_env->update != 0) {
        delta_lcl_bits = 0;
        index = ixheaace_index_low2_high(offset, band, freq_res[i]);
        delta_t[band] = current_energy - pstr_code_env->sfb_nrg_prev[index];
        delta_t[band] = ixheaac_shr32(delta_t[band], env_data_tab_comp_factor);

        ixheaace_map_low_res_energy_value(current_energy, pstr_code_env->sfb_nrg_prev, offset,
                                          band, freq_res[i]);
        if ((delta_t[band] > 31) || (delta_t[band] < -31)) {
          delta_t[band] = 31;
        }

        err_code = ixheaace_compute_bits(
            delta_t[band], code_book_scf_lav_lvl_time, code_book_scf_lav_bal_time,
            ptr_huff_tab_lvl_time_l, ptr_huff_tab_bal_time_l, coupling, channel, &delta_lcl_bits);
        if (err_code) {
          return err_code;
        }
        delta_t_bits += delta_lcl_bits;
      } else {
        ixheaace_map_low_res_energy_value(current_energy, pstr_code_env->sfb_nrg_prev, offset,
                                          band, freq_res[i]);
      }
      band++;
    }

    if (i == 0) {
      use_dt = (pstr_code_env->update != 0 &&
                (delta_f_bits > delta_t_bits * (1 + df_edge_first_ennv)));
    } else if (is_ld_sbr) {
      use_dt = ((pstr_code_env->update != 0) && (delta_f_bits > delta_t_bits));
    } else {
      use_dt = (delta_f_bits > delta_t_bits);
    }

    if (use_dt) {
      ptr_dir_vec[i] = TIME;
      memcpy(ptr_sfb_energy, delta_t, no_of_bands * sizeof(ptr_sfb_energy[0]));
    } else {
      ptr_dir_vec[i] = FREQ;
      memcpy(ptr_sfb_energy, delta_f, no_of_bands * sizeof(ptr_sfb_energy[0]));
    }
    if (0 == i && 1 == usac_indep_flag) {
      ptr_dir_vec[i] = FREQ;
      memcpy(ptr_sfb_energy, delta_f, no_of_bands * sizeof(ptr_sfb_energy[0]));
    }

    ptr_sfb_energy += no_of_bands;
    pstr_code_env->update = 1;
  }
  return err_code;
}
