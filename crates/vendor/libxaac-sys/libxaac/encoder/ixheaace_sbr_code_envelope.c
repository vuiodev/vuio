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
#include "ixheaace_error_codes.h"
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

IA_ERRORCODE
ixheaace_init_sbr_huffman_tabs(ixheaace_pstr_sbr_env_data pstr_sbr_env,
                               ixheaace_pstr_sbr_code_envelope pstr_code_env,
                               ixheaace_pstr_sbr_code_envelope pstr_noise,
                               ixheaace_amp_res amp_res,
                               ixheaace_str_sbr_huff_tabs *pstr_sbr_huff_tabs) {
  pstr_sbr_env->init_sbr_amp_res = amp_res;

  switch (amp_res) {
    case IXHEAACE_SBR_AMP_RES_3_0:

      pstr_sbr_env->ptr_huff_tab_lvl_time_c = pstr_sbr_huff_tabs->v_huff_env_lvl_c11t;
      pstr_sbr_env->ptr_huff_tab_lvl_time_l = pstr_sbr_huff_tabs->v_huff_env_lvl_l11t;
      pstr_sbr_env->ptr_huff_tab_bal_time_c = pstr_sbr_huff_tabs->book_sbr_env_bal_c11t;
      pstr_sbr_env->ptr_huff_tab_bal_time_l = pstr_sbr_huff_tabs->book_sbr_env_bal_l11t;

      pstr_sbr_env->ptr_huff_tab_lvl_freq_c = pstr_sbr_huff_tabs->v_huff_env_lvl_c11f;
      pstr_sbr_env->ptr_huff_tab_lvl_freq_l = pstr_sbr_huff_tabs->v_huff_env_lvl_l11f;
      pstr_sbr_env->ptr_huff_tab_bal_freq_c = pstr_sbr_huff_tabs->book_sbr_env_bal_c11f;
      pstr_sbr_env->ptr_huff_tab_bal_freq_l = pstr_sbr_huff_tabs->book_sbr_env_bal_l11f;

      pstr_sbr_env->ptr_huff_tab_time_c = pstr_sbr_huff_tabs->v_huff_env_lvl_c11t;
      pstr_sbr_env->ptr_huff_tab_time_l = pstr_sbr_huff_tabs->v_huff_env_lvl_l11t;
      pstr_sbr_env->ptr_huff_tab_freq_c = pstr_sbr_huff_tabs->v_huff_env_lvl_c11f;
      pstr_sbr_env->ptr_huff_tab_freq_l = pstr_sbr_huff_tabs->v_huff_env_lvl_l11f;

      pstr_sbr_env->code_book_scf_lav_balance = CODE_BCK_SCF_LAV_BALANCE11;
      pstr_sbr_env->code_book_scf_lav = CODE_BCK_SCF_LAV11;

      pstr_sbr_env->si_sbr_start_env_bits = SI_SBR_START_ENV_BITS_AMP_RES_3_0;
      pstr_sbr_env->si_sbr_start_env_bits_balance = SI_SBR_START_ENV_BITS_BALANCE_AMP_RES_3_0;
      break;

    case IXHEAACE_SBR_AMP_RES_1_5:

      pstr_sbr_env->ptr_huff_tab_lvl_time_c = pstr_sbr_huff_tabs->v_huff_env_lvl_c10t;
      pstr_sbr_env->ptr_huff_tab_lvl_time_l = pstr_sbr_huff_tabs->v_huff_env_lvl_l10t;
      pstr_sbr_env->ptr_huff_tab_bal_time_c = pstr_sbr_huff_tabs->book_sbr_env_bal_c10t;
      pstr_sbr_env->ptr_huff_tab_bal_time_l = pstr_sbr_huff_tabs->book_sbr_env_bal_l10t;

      pstr_sbr_env->ptr_huff_tab_lvl_freq_c = pstr_sbr_huff_tabs->v_huff_env_lvl_c10f;
      pstr_sbr_env->ptr_huff_tab_lvl_freq_l = pstr_sbr_huff_tabs->v_huff_env_lvl_l10f;
      pstr_sbr_env->ptr_huff_tab_bal_freq_c = pstr_sbr_huff_tabs->book_sbr_env_bal_c10f;
      pstr_sbr_env->ptr_huff_tab_bal_freq_l = pstr_sbr_huff_tabs->book_sbr_env_bal_l10f;

      pstr_sbr_env->ptr_huff_tab_time_c = pstr_sbr_huff_tabs->v_huff_env_lvl_c10t;
      pstr_sbr_env->ptr_huff_tab_time_l = pstr_sbr_huff_tabs->v_huff_env_lvl_l10t;
      pstr_sbr_env->ptr_huff_tab_freq_c = pstr_sbr_huff_tabs->v_huff_env_lvl_c10f;
      pstr_sbr_env->ptr_huff_tab_freq_l = pstr_sbr_huff_tabs->v_huff_env_lvl_l10f;

      pstr_sbr_env->code_book_scf_lav_balance = CODE_BCK_SCF_LAV_BALANCE10;
      pstr_sbr_env->code_book_scf_lav = CODE_BCK_SCF_LAV10;

      pstr_sbr_env->si_sbr_start_env_bits = SI_SBR_START_ENV_BITS_AMP_RES_1_5;
      pstr_sbr_env->si_sbr_start_env_bits_balance = SI_SBR_START_ENV_BITS_BALANCE_AMP_RES_1_5;
      break;

    default:
      return IA_EXHEAACE_EXE_FATAL_SBR_INVALID_AMP_RES;
      break;
  }

  pstr_sbr_env->ptr_huff_tab_noise_lvl_time_c = pstr_sbr_huff_tabs->v_huff_noise_lvl_c11t;
  pstr_sbr_env->ptr_huff_tab_noise_lvl_time_l = pstr_sbr_huff_tabs->v_huff_noise_lvl_l11t;
  pstr_sbr_env->ptr_huff_tab_noise_bal_time_c = pstr_sbr_huff_tabs->book_sbr_noise_bal_c11t;
  pstr_sbr_env->ptr_huff_tab_noise_bal_time_l = pstr_sbr_huff_tabs->book_sbr_noise_bal_l11t;

  pstr_sbr_env->ptr_huff_tab_noise_lvl_freq_c = pstr_sbr_huff_tabs->v_huff_env_lvl_c11f;
  pstr_sbr_env->ptr_huff_tab_noise_lvl_freq_l = pstr_sbr_huff_tabs->v_huff_env_lvl_l11f;
  pstr_sbr_env->ptr_huff_tab_noise_bal_freq_c = pstr_sbr_huff_tabs->book_sbr_env_bal_c11f;
  pstr_sbr_env->ptr_huff_tab_noise_bal_freq_l = pstr_sbr_huff_tabs->book_sbr_env_bal_l11f;

  pstr_sbr_env->ptr_huff_tab_noise_time_c = pstr_sbr_huff_tabs->v_huff_noise_lvl_c11t;
  pstr_sbr_env->ptr_huff_tab_noise_time_l = pstr_sbr_huff_tabs->v_huff_noise_lvl_l11t;
  pstr_sbr_env->ptr_huff_tab_noise_freq_c = pstr_sbr_huff_tabs->v_huff_env_lvl_c11f;
  pstr_sbr_env->ptr_huff_tab_noise_freq_l = pstr_sbr_huff_tabs->v_huff_env_lvl_l11f;

  pstr_sbr_env->si_sbr_start_noise_bits = SI_SBR_START_NOISE_BITS_AMP_RES_3_0;
  pstr_sbr_env->si_sbr_start_noise_bits_balance = SI_SBR_START_NOISE_BITS_BALANCE_AMP_RES_3_0;

  pstr_code_env->code_book_scf_lav_bal_time = pstr_sbr_env->code_book_scf_lav_balance;
  pstr_code_env->code_book_scf_lav_bal_freq = pstr_sbr_env->code_book_scf_lav_balance;
  pstr_code_env->code_book_scf_lav_lvl_time = pstr_sbr_env->code_book_scf_lav;
  pstr_code_env->code_book_scf_lav_lvl_freq = pstr_sbr_env->code_book_scf_lav;
  pstr_code_env->code_book_scf_lav_time = pstr_sbr_env->code_book_scf_lav;
  pstr_code_env->code_book_scf_lav_freq = pstr_sbr_env->code_book_scf_lav;

  pstr_code_env->ptr_huff_tab_lvl_time_l = pstr_sbr_env->ptr_huff_tab_lvl_time_l;
  pstr_code_env->ptr_huff_tab_bal_time_l = pstr_sbr_env->ptr_huff_tab_bal_time_l;
  pstr_code_env->ptr_huff_tab_time_l = pstr_sbr_env->ptr_huff_tab_time_l;
  pstr_code_env->ptr_huff_tab_lvl_freq_l = pstr_sbr_env->ptr_huff_tab_lvl_freq_l;
  pstr_code_env->ptr_huff_tab_bal_freq_l = pstr_sbr_env->ptr_huff_tab_bal_freq_l;
  pstr_code_env->ptr_huff_tab_freq_l = pstr_sbr_env->ptr_huff_tab_freq_l;

  pstr_code_env->code_book_scf_lav_freq = pstr_sbr_env->code_book_scf_lav;
  pstr_code_env->code_book_scf_lav_time = pstr_sbr_env->code_book_scf_lav;

  pstr_code_env->start_bits = pstr_sbr_env->si_sbr_start_env_bits;
  pstr_code_env->start_bits_balance = pstr_sbr_env->si_sbr_start_env_bits_balance;

  pstr_noise->code_book_scf_lav_bal_time = CODE_BCK_SCF_LAV_BALANCE11;
  pstr_noise->code_book_scf_lav_bal_freq = CODE_BCK_SCF_LAV_BALANCE11;
  pstr_noise->code_book_scf_lav_lvl_time = CODE_BCK_SCF_LAV11;
  pstr_noise->code_book_scf_lav_lvl_freq = CODE_BCK_SCF_LAV11;
  pstr_noise->code_book_scf_lav_time = CODE_BCK_SCF_LAV11;
  pstr_noise->code_book_scf_lav_freq = CODE_BCK_SCF_LAV11;

  pstr_noise->ptr_huff_tab_lvl_time_l = pstr_sbr_env->ptr_huff_tab_noise_lvl_time_l;
  pstr_noise->ptr_huff_tab_bal_time_l = pstr_sbr_env->ptr_huff_tab_noise_bal_time_l;
  pstr_noise->ptr_huff_tab_time_l = pstr_sbr_env->ptr_huff_tab_noise_time_l;
  pstr_noise->ptr_huff_tab_lvl_freq_l = pstr_sbr_env->ptr_huff_tab_noise_lvl_freq_l;
  pstr_noise->ptr_huff_tab_bal_freq_l = pstr_sbr_env->ptr_huff_tab_noise_bal_freq_l;
  pstr_noise->ptr_huff_tab_freq_l = pstr_sbr_env->ptr_huff_tab_noise_freq_l;

  pstr_noise->start_bits = pstr_sbr_env->si_sbr_start_noise_bits;
  pstr_noise->start_bits_balance = pstr_sbr_env->si_sbr_start_noise_bits_balance;

  pstr_code_env->update = 0;
  pstr_noise->update = 0;

  return IA_NO_ERROR;
}

VOID ixheaace_create_sbr_code_envelope(ixheaace_pstr_sbr_code_envelope pstr_code_env,
                                       WORD32 *num_sfb, WORD32 delta_t_across_frames,
                                       FLOAT32 df_edge_first_env, FLOAT32 df_edge_incr) {
  memset(pstr_code_env, 0, sizeof(ixheaace_str_sbr_code_envelope));

  pstr_code_env->delta_t_across_frames = delta_t_across_frames;
  pstr_code_env->df_edge_1st_env = df_edge_first_env;
  pstr_code_env->df_edge_incr = df_edge_incr;
  pstr_code_env->df_edge_incr_fac = 0;
  pstr_code_env->update = 0;
  pstr_code_env->num_scf[FREQ_RES_LOW] = num_sfb[FREQ_RES_LOW];
  pstr_code_env->num_scf[FREQ_RES_HIGH] = num_sfb[FREQ_RES_HIGH];

  pstr_code_env->offset =
      2 * pstr_code_env->num_scf[FREQ_RES_LOW] - pstr_code_env->num_scf[FREQ_RES_HIGH];
}
