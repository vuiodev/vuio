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
#include "ixheaace_error_codes.h"

#include "ixheaace_sbr_header.h"
#include "ixheaace_sbr_def.h"
#include "ixheaace_resampler.h"
#include "ixheaace_sbr_rom.h"
#include "ixheaace_common_rom.h"
#include "ixheaace_sbr_hbe.h"
#include "ixheaace_sbr_qmf_enc.h"
#include "ixheaace_sbr_tran_det.h"
#include "ixheaace_sbr_frame_info_gen.h"
#include "ixheaace_sbr_env_est.h"
#include "ixheaace_sbr_code_envelope.h"
#include "ixheaace_sbr_main.h"
#include "ixheaace_sbr_missing_harmonics_det.h"
#include "ixheaace_sbr_inv_filtering_estimation.h"
#include "ixheaace_sbr_noise_floor_est.h"
#include "ixheaace_sbr_ton_corr.h"
#include "iusace_esbr_pvc.h"
#include "iusace_esbr_inter_tes.h"
#include "ixheaace_sbr.h"

#include "ixheaace_sbr_misc.h"

VOID ixheaace_ton_corr_param_extr(ixheaace_pstr_sbr_ton_corr_est pstr_ton_corr,
                                  ixheaace_invf_mode *pstr_inf_vec, FLOAT32 *ptr_noise_lvls,
                                  WORD32 *ptr_missing_harmonic_flag,
                                  UWORD8 *ptr_missing_harmonic_index, WORD8 *ptr_env_compensation,
                                  const ixheaace_str_frame_info_sbr *pstr_frame_info,
                                  WORD32 *ptr_trans_info, UWORD8 *ptr_freq_band_tab,
                                  WORD32 ptr_num_scf, ixheaace_sbr_xpos_mode xpos_type,
                                  WORD8 *ptr_sbr_scratch, WORD32 is_ld_sbr) {
  (VOID) ptr_sbr_scratch;
  WORD32 transient_flag = ptr_trans_info[1];
  WORD32 transient_pos = ptr_trans_info[0];
  WORD32 transient_frame_invf_est;
  ixheaace_invf_mode *pstr_inf_vec_lcl;
  WORD32 transient_frame = 0;

  if (pstr_ton_corr->trans_nxt_frame) {
    transient_frame = 1;
    pstr_ton_corr->trans_nxt_frame = 0;

    if ((transient_flag) && (transient_pos + pstr_ton_corr->trans_pos_offset >=
                             pstr_frame_info->borders[pstr_frame_info->n_envelopes])) {
      pstr_ton_corr->trans_nxt_frame = 1;
    }
  } else {
    if (transient_flag) {
      if (transient_pos + pstr_ton_corr->trans_pos_offset <
          pstr_frame_info->borders[pstr_frame_info->n_envelopes]) {
        transient_frame = 1;
        pstr_ton_corr->trans_nxt_frame = 0;
      } else {
        pstr_ton_corr->trans_nxt_frame = 1;
      }
    }
  }
  transient_frame_invf_est = is_ld_sbr ? transient_frame : pstr_ton_corr->trans_nxt_frame;
  if (pstr_ton_corr->switch_inverse_filt) {
    ixheaace_qmf_inverse_filtering_detector(
        &pstr_ton_corr->sbr_noise_inv_filt, pstr_ton_corr->ptr_quota_mtx,
        pstr_ton_corr->energy_vec, pstr_ton_corr->idx_vx,
        pstr_ton_corr->frame_start_index_invf_est,
        pstr_ton_corr->est_cnt_per_frame + pstr_ton_corr->frame_start_index_invf_est,
        transient_frame_invf_est, pstr_inf_vec, is_ld_sbr);
  }

  if (xpos_type == IXHEAACE_XPOS_LC) {
    ixheaace_sbr_missing_harmonics_detector_qmf(
        &pstr_ton_corr->sbr_missing_har_detector, pstr_ton_corr->ptr_quota_mtx,
        pstr_ton_corr->idx_vx, pstr_frame_info, ptr_trans_info, ptr_missing_harmonic_flag,
        ptr_missing_harmonic_index, ptr_freq_band_tab, ptr_num_scf, ptr_env_compensation);
  } else {
    *ptr_missing_harmonic_flag = 0;
    memset(ptr_missing_harmonic_index, 0, ptr_num_scf * sizeof(ptr_missing_harmonic_index[0]));
  }

  pstr_inf_vec_lcl = pstr_ton_corr->sbr_noise_inv_filt.prev_invf_mode;

  ixheaace_sbr_noise_floor_estimate_qmf(
      &pstr_ton_corr->sbr_noise_floor_est, pstr_frame_info, ptr_noise_lvls,
      pstr_ton_corr->ptr_quota_mtx, pstr_ton_corr->idx_vx, *ptr_missing_harmonic_flag,
      pstr_ton_corr->frame_start_index, transient_frame, pstr_inf_vec_lcl, is_ld_sbr);

  memcpy(pstr_ton_corr->sbr_noise_inv_filt.prev_invf_mode, pstr_inf_vec,
         pstr_ton_corr->sbr_noise_inv_filt.no_detector_bands * sizeof(pstr_inf_vec[0]));
}

static WORD32 ia_enhaacplus_enc_find_closest_entry(WORD32 goal_sb, UWORD8 *ptr_vk_master,
                                                   WORD32 num_master, WORD32 direction) {
  WORD32 index;

  if (goal_sb <= ptr_vk_master[0]) {
    return ptr_vk_master[0];
  }

  if (goal_sb >= ptr_vk_master[num_master]) {
    return ptr_vk_master[num_master];
  }

  if (direction) {
    index = 0;

    while (ptr_vk_master[index] < goal_sb) {
      index++;
    }
  } else {
    index = num_master;
    while (ptr_vk_master[index] > goal_sb) {
      index--;
    }
  }

  return ptr_vk_master[index];
}

static IA_ERRORCODE ia_enhaacplus_enc_reset_patch(ixheaace_pstr_sbr_ton_corr_est pstr_ton_corr,
                                                  WORD32 xpos_ctrl, WORD32 high_band_start_sb,
                                                  UWORD8 *ptr_vk_master, WORD32 num_master,
                                                  WORD32 fs, WORD32 num_channels) {
  WORD32 patch, k, i;
  WORD32 target_stop_band;

  ixheaace_patch_param *ptr_patch_param = pstr_ton_corr->str_patch_param;

  WORD32 sb_guard = pstr_ton_corr->guard;
  WORD32 source_start_band;
  WORD32 patch_distance;
  WORD32 num_bands_in_patch;

  WORD32 lsb = ptr_vk_master[0];
  WORD32 usb = ptr_vk_master[num_master];
  WORD32 xover_offset = high_band_start_sb - ptr_vk_master[0];

  WORD32 goal_sb;

  if (xpos_ctrl == 1) {
    lsb += xover_offset;
    xover_offset = 0;
  }

  goal_sb = (WORD32)(2 * num_channels * 16000.0f / fs + 0.5f);
  goal_sb = ia_enhaacplus_enc_find_closest_entry(goal_sb, ptr_vk_master, num_master, 1);

  source_start_band = pstr_ton_corr->shift_start_sb + xover_offset;

  target_stop_band = lsb + xover_offset;

  patch = 0;

  while (target_stop_band < usb) {
    if (patch >= IXHEAACE_MAXIMUM_NUM_PATCHES) {
      return IA_EXHEAACE_EXE_NONFATAL_ESBR_INVALID_NUM_PATCH;
    }

    ptr_patch_param[patch].guard_start_band = target_stop_band;

    target_stop_band += sb_guard;

    ptr_patch_param[patch].target_start_band = target_stop_band;

    num_bands_in_patch = goal_sb - target_stop_band;

    if (num_bands_in_patch >= lsb - source_start_band) {
      patch_distance = target_stop_band - source_start_band;

      patch_distance = patch_distance & ~1;

      num_bands_in_patch = lsb - (target_stop_band - patch_distance);

      num_bands_in_patch =
          ia_enhaacplus_enc_find_closest_entry(target_stop_band + num_bands_in_patch,
                                               ptr_vk_master, num_master, 0) -
          target_stop_band;
    }

    patch_distance = num_bands_in_patch + target_stop_band - lsb;

    patch_distance = (patch_distance + 1) & ~1;

    if (num_bands_in_patch <= 0) {
      patch--;
    } else {
      ptr_patch_param[patch].source_start_band = target_stop_band - patch_distance;

      ptr_patch_param[patch].target_band_offs = patch_distance;
      ptr_patch_param[patch].num_bands_in_patch = num_bands_in_patch;

      ptr_patch_param[patch].source_stop_band =
          ptr_patch_param[patch].source_start_band + num_bands_in_patch;

      target_stop_band += ptr_patch_param[patch].num_bands_in_patch;
    }

    source_start_band = pstr_ton_corr->shift_start_sb;

    if (ixheaac_abs32(target_stop_band - goal_sb) < 3) {
      goal_sb = usb;
    }
    patch++;
  }

  patch--;

  if (ptr_patch_param[patch].num_bands_in_patch < 3 && patch > 0) {
    patch--;
  }

  pstr_ton_corr->no_of_patches = patch + 1;

  for (k = 0; k < pstr_ton_corr->str_patch_param[0].guard_start_band; k++) {
    pstr_ton_corr->idx_vx[k] = (WORD8)k;
  }

  for (i = 0; i < pstr_ton_corr->no_of_patches; i++) {
    WORD32 source_start = pstr_ton_corr->str_patch_param[i].source_start_band;
    WORD32 target_start = pstr_ton_corr->str_patch_param[i].target_start_band;
    WORD32 number_of_bands = pstr_ton_corr->str_patch_param[i].num_bands_in_patch;
    WORD32 start_guard_band = pstr_ton_corr->str_patch_param[i].guard_start_band;

    for (k = 0; k < (target_start - start_guard_band); k++) {
      pstr_ton_corr->idx_vx[start_guard_band + k] = -1;
    }

    for (k = 0; k < number_of_bands; k++) {
      pstr_ton_corr->idx_vx[target_start + k] = (WORD8)(source_start + k);
    }
  }

  return IA_NO_ERROR;
}

IA_ERRORCODE
ixheaace_create_ton_corr_param_extr(WORD32 ch, ixheaace_pstr_sbr_ton_corr_est pstr_ton_corr,
                                    WORD32 fs, WORD32 num_qmf_ch, WORD32 xpos_ctrl,
                                    WORD32 high_band_start_sb, UWORD8 *ptr_vk_master,
                                    WORD32 num_master, WORD32 ana_max_level,
                                    UWORD8 *ptr_freq_band_tab[2], WORD32 *ptr_num_scf,
                                    WORD32 noise_groups, UWORD32 use_speech_config,
                                    WORD32 *ptr_common_buffer,
                                    ixheaace_str_qmf_tabs *pstr_qmf_tab, WORD32 is_ld_sbr) {
  WORD32 i, loop_cnt;
  IA_ERRORCODE err_code = IA_NO_ERROR;

  memset(pstr_ton_corr, 0, sizeof(ixheaace_str_sbr_ton_corr_est));

  pstr_ton_corr->est_cnt_per_frame = 2;
  /* The code is modified to handle 480 frame size  */
  if (0 == is_ld_sbr) {
    pstr_ton_corr->est_cnt = NO_OF_ESTIMATES;
    pstr_ton_corr->move = 2;
    pstr_ton_corr->start_index_matrix = 2;
  } else {
    pstr_ton_corr->est_cnt = NO_OF_ESTIMATES_ELD;
    pstr_ton_corr->move = 1;
    pstr_ton_corr->start_index_matrix = 1;
  }

  pstr_ton_corr->frame_start_index_invf_est = 0;
  pstr_ton_corr->trans_pos_offset = 4;
  pstr_ton_corr->frame_start_index = 0;
  pstr_ton_corr->prev_trans_flag = 0;
  pstr_ton_corr->trans_nxt_frame = 0;
  pstr_ton_corr->num_qmf_ch = num_qmf_ch;
  pstr_ton_corr->guard = 0;
  pstr_ton_corr->shift_start_sb = 1;

  loop_cnt = is_ld_sbr ? NO_OF_ESTIMATES : pstr_ton_corr->est_cnt;

  for (i = 0; i < loop_cnt; i++) {
    pstr_ton_corr->ptr_quota_mtx[i] = &(pstr_ton_corr->sbr_quota_mtx[i * num_qmf_ch]);
    memset(pstr_ton_corr->ptr_quota_mtx[i], 0,
           sizeof(pstr_ton_corr->ptr_quota_mtx[0][0]) * num_qmf_ch);
  }

  err_code = ia_enhaacplus_enc_reset_patch(pstr_ton_corr, xpos_ctrl, high_band_start_sb,
                                           ptr_vk_master, num_master, fs, num_qmf_ch);
  if (err_code) {
    return err_code;
  }

  err_code = ixheaace_create_sbr_noise_floor_estimate(
      &pstr_ton_corr->sbr_noise_floor_est, ana_max_level, ptr_freq_band_tab[LO], ptr_num_scf[LO],
      noise_groups, use_speech_config, pstr_qmf_tab);
  if (err_code) {
    return err_code;
  }

  ixheaace_create_inv_filt_detector(
      &pstr_ton_corr->sbr_noise_inv_filt, pstr_ton_corr->sbr_noise_floor_est.s_freq_qmf_band_tbl,
      pstr_ton_corr->sbr_noise_floor_est.num_of_noise_bands, use_speech_config, pstr_qmf_tab);

  ixheaace_create_sbr_missing_harmonics_detector(
      ch, &pstr_ton_corr->sbr_missing_har_detector, fs, ptr_num_scf[HI], num_qmf_ch,
      pstr_ton_corr->est_cnt, pstr_ton_corr->move, pstr_ton_corr->est_cnt_per_frame,
      ptr_common_buffer);

  return err_code;
}
