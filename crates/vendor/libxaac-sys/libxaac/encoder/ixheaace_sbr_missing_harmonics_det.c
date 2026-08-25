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
#include <float.h>
#include "ixheaac_type_def.h"
#include "ixheaac_constants.h"
#include "ixheaac_error_standards.h"
#include "ixheaace_aac_constants.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops16.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaac_basic_ops.h"

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
#include "ixheaace_common_utils.h"

static VOID ia_enhaacplus_enc_diff(FLOAT32 *ptr_tonal_orig, FLOAT32 *ptr_diff_map_2_scfb,
                                   const UWORD8 *ptr_freq_band_tab, WORD32 n_scfb,
                                   WORD8 *ptr_idx_vx) {
  WORD32 i, ll, lu, k;
  FLOAT32 max_val_orig, max_val_sbr;
  for (i = 0; i < n_scfb; i++) {
    ll = ptr_freq_band_tab[i];
    lu = ptr_freq_band_tab[i + 1];
    max_val_orig = max_val_sbr = 0;

    for (k = ll; k < lu; k++) {
      if (ptr_tonal_orig[k] > max_val_orig) {
        max_val_orig = ptr_tonal_orig[k];
      }
      if (ptr_tonal_orig[ptr_idx_vx[k]] > max_val_sbr) {
        max_val_sbr = ptr_tonal_orig[ptr_idx_vx[k]];
      }
    }

    ptr_diff_map_2_scfb[i] = (max_val_sbr > 1) ? max_val_orig / max_val_sbr : max_val_orig;
  }
}

static VOID ia_enhaacplus_enc_calculate_flatness_measure(FLOAT32 *ptr_quota_buf,
                                                         WORD8 *ptr_idx_vx, FLOAT32 *ptr_sfm_orig,
                                                         FLOAT32 *ptr_sfm_sbr,
                                                         const UWORD8 *ptr_freq_band_tab,
                                                         WORD32 num_sfb) {
  WORD32 i, j;

  FLOAT32 am_org, am_transp, gm_org, gm_transp, sfm_org, sfm_transp;

  i = 0;
  while (i < num_sfb) {
    WORD32 band_low = ptr_freq_band_tab[i];
    WORD32 band_high = ptr_freq_band_tab[i + 1];

    ptr_sfm_orig[i] = 1.0f;
    ptr_sfm_sbr[i] = 1.0f;

    if (band_high - band_low > 1) {
      am_org = 0;
      am_transp = 0;
      gm_org = 1.0f;
      gm_transp = 1.0f;

      for (j = band_low; j < band_high; j++) {
        sfm_org = ptr_quota_buf[j];
        sfm_transp = ptr_quota_buf[ptr_idx_vx[j]];

        am_org += sfm_org;
        gm_org += sfm_org;
        am_transp += sfm_transp;
        gm_transp += sfm_transp;
      }

      am_org /= band_high - band_low;
      am_transp /= band_high - band_low;
      gm_org = (FLOAT32)pow(gm_org, 1.0f / (band_high - band_low));
      gm_transp = (FLOAT32)pow(gm_transp, 1.0f / (band_high - band_low));

      if (am_org) {
        ptr_sfm_orig[i] = gm_org / am_org;
      }

      if (am_transp) {
        ptr_sfm_sbr[i] = gm_transp / am_transp;
      }
    }
    i++;
  }
}

static VOID ia_enhaacplus_enc_calculate_detector_input(
    FLOAT32 **ptr_quota_buf, WORD8 *ptr_idx_vx, FLOAT32 **ptr_tonal_diff, FLOAT32 **ptr_sfm_orig,
    FLOAT32 **ptr_sfm_sbr, const UWORD8 *ptr_freq_band_tab, WORD32 num_sfb,
    WORD32 no_est_per_frame, WORD32 move, WORD32 no_qmf_bands) {
  WORD32 est;

  for (est = 0; est < move; est++) {
    memcpy(ptr_tonal_diff[est], ptr_tonal_diff[est + no_est_per_frame],
           no_qmf_bands * sizeof(ptr_tonal_diff[est][0]));

    memcpy(ptr_sfm_orig[est], ptr_sfm_orig[est + no_est_per_frame],
           no_qmf_bands * sizeof(ptr_sfm_orig[est][0]));

    memcpy(ptr_sfm_sbr[est], ptr_sfm_sbr[est + no_est_per_frame],
           no_qmf_bands * sizeof(ptr_sfm_sbr[est][0]));
  }

  for (est = 0; est < no_est_per_frame; est++) {
    ia_enhaacplus_enc_diff(ptr_quota_buf[est + move], ptr_tonal_diff[est + move],
                           ptr_freq_band_tab, num_sfb, ptr_idx_vx);

    ia_enhaacplus_enc_calculate_flatness_measure(
        ptr_quota_buf[est + move], ptr_idx_vx, ptr_sfm_orig[est + move], ptr_sfm_sbr[est + move],
        ptr_freq_band_tab, num_sfb);
  }
}

static WORD32 ia_enhaacplus_enc_isDetectionOfNewToneAllowed(
    const ixheaace_str_frame_info_sbr *ptr_frame_info, WORD32 prev_transient_frame,
    WORD32 prev_transient_pos, WORD32 prev_trans_flag, WORD32 trans_pos_offset,
    WORD32 transient_flag, WORD32 transient_pos,
    ixheaace_pstr_sbr_missing_harmonics_detector pstr_sbr_missing_harmonics_detector) {
  WORD32 transient_frame, new_detection_allowed;

  transient_frame = 0;

  if (transient_flag) {
    if (transient_pos + trans_pos_offset < ptr_frame_info->borders[ptr_frame_info->n_envelopes]) {
      transient_frame = 1;
    }
  } else {
    if (prev_trans_flag && !prev_transient_frame) {
      transient_frame = 1;
    }
  }

  new_detection_allowed = 0;

  if (transient_frame) {
    new_detection_allowed = 1;
  } else {
    if (prev_transient_frame &&
        ixheaac_abs32(ptr_frame_info->borders[0] -
                      (prev_transient_pos + trans_pos_offset -
                       pstr_sbr_missing_harmonics_detector->time_slots)) < DELTA_TIME) {
      new_detection_allowed = 1;
    }
  }

  pstr_sbr_missing_harmonics_detector->prev_trans_flag = transient_flag;
  pstr_sbr_missing_harmonics_detector->prev_trans_frame = transient_frame;
  pstr_sbr_missing_harmonics_detector->prev_trans_pos = transient_pos;

  return (new_detection_allowed);
}

static VOID ia_enhaacplus_enc_transient_cleanup(FLOAT32 **ptr_quota_buf, WORD32 num_sfb,
                                                UWORD8 **ptr_detection_vectors,
                                                const UWORD8 *ptr_freq_band_tab,
                                                ixheaace_str_guide_vectors ptr_guide_vectors,
                                                WORD32 start, WORD32 stop) {
  WORD32 sfb, band, low_band, up_band, est;

  UWORD8 ptr_harm_vec[MAXIMUM_FREQ_COEFFS];

  memset(ptr_harm_vec, 0, sizeof(ptr_harm_vec));

  for (est = start; est < stop; est++) {
    for (sfb = 0; sfb < num_sfb - 1; sfb++) {
      ptr_harm_vec[sfb] = ptr_harm_vec[sfb] || ptr_detection_vectors[est][sfb];
    }
  }

  for (sfb = 0; sfb < num_sfb - 1; sfb++) {
    if (ptr_harm_vec[sfb] && ptr_harm_vec[sfb + 1]) {
      FLOAT32 max_val_1, max_val_2;
      WORD32 maxPos1, maxPos2;

      low_band = ptr_freq_band_tab[sfb];
      up_band = ptr_freq_band_tab[sfb + 1];
      maxPos1 = low_band;

      max_val_1 = ptr_quota_buf[start][low_band];
      for (band = low_band; band < up_band; band++) {
        if (ptr_quota_buf[start][band] > max_val_1) {
          max_val_1 = ptr_quota_buf[start][band];
          maxPos1 = band;
        }
      }

      for (est = start + 1; est < stop; est++) {
        for (band = low_band; band < up_band; band++) {
          if (ptr_quota_buf[est][band] > max_val_1) {
            max_val_1 = ptr_quota_buf[est][band];
            maxPos1 = band;
          }
        }
      }

      low_band = ptr_freq_band_tab[sfb + 1];
      up_band = ptr_freq_band_tab[sfb + 2];

      maxPos2 = low_band;
      max_val_2 = ptr_quota_buf[start][low_band];

      for (band = low_band; band < up_band; band++) {
        if (ptr_quota_buf[start][band] > max_val_2) {
          max_val_2 = ptr_quota_buf[start][band];
          maxPos2 = band;
        }
      }

      for (est = start + 1; est < stop; est++) {
        for (band = low_band; band < up_band; band++) {
          if (ptr_quota_buf[est][band] > max_val_2) {
            max_val_2 = ptr_quota_buf[est][band];
            maxPos2 = band;
          }
        }
      }

      if (maxPos2 - maxPos1 < 2) {
        if (max_val_1 > max_val_2) {
          ptr_guide_vectors.ptr_guide_vector_detected[sfb + 1] = 0;
          ptr_guide_vectors.ptr_guide_vec_orig[sfb + 1] = 0;
          ptr_guide_vectors.ptr_guide_vec_diff[sfb + 1] = 0;

          for (est = start; est < stop; est++) {
            ptr_detection_vectors[est][sfb + 1] = 0;
          }
        } else {
          ptr_guide_vectors.ptr_guide_vector_detected[sfb] = 0;
          ptr_guide_vectors.ptr_guide_vec_diff[sfb] = 0;
          ptr_guide_vectors.ptr_guide_vec_orig[sfb] = 0;

          for (est = start; est < stop; est++) {
            ptr_detection_vectors[est][sfb] = 0;
          }
        }
      }
    }
  }
}

static VOID ia_enhaacplus_enc_detection(FLOAT32 *ptr_quota_buf, FLOAT32 *ptr_diff_vec_scfb,
                                        WORD32 num_sfb, UWORD8 *ptr_harm_vec,
                                        const UWORD8 *ptr_freq_band_tab, FLOAT32 *ptr_sfm_orig,
                                        FLOAT32 *ptr_sfm_sbr,
                                        ixheaace_str_guide_vectors *ptr_guide_vectors,
                                        ixheaace_str_guide_vectors *ptr_new_guide_vectors) {
  WORD32 i, j;
  WORD32 lower_band, upper_band;
  FLOAT32 thr, thr_org;

  for (i = 0; i < num_sfb; i++) {
    if (ptr_guide_vectors->ptr_guide_vec_diff[i]) {
      if (SBR_DECAY_GUIDE_DIFF * ptr_guide_vectors->ptr_guide_vec_diff[i] > SBR_THR_DIFF_GUIDE) {
        thr = SBR_DECAY_GUIDE_DIFF * ptr_guide_vectors->ptr_guide_vec_diff[i];
      } else {
        thr = SBR_THR_DIFF_GUIDE;
      }
      if (thr > SBR_THR_DIFF) {
        thr = SBR_THR_DIFF;
      }
    } else {
      thr = SBR_THR_DIFF;
    }

    if (ptr_diff_vec_scfb[i] > thr) {
      ptr_harm_vec[i] = 1;
      ptr_new_guide_vectors->ptr_guide_vec_diff[i] = ptr_diff_vec_scfb[i];
    } else {
      if (ptr_guide_vectors->ptr_guide_vec_diff[i]) {
        ptr_guide_vectors->ptr_guide_vec_orig[i] = SBR_THR_TONE_GUIDE;
      }
    }
  }

  for (i = 0; i < num_sfb; i++) {
    lower_band = ptr_freq_band_tab[i];
    upper_band = ptr_freq_band_tab[i + 1];

    if (ptr_guide_vectors->ptr_guide_vec_orig[i] * SBR_DECAY_GUIDE_ORIG > SBR_THR_TONE_GUIDE) {
      thr_org = ptr_guide_vectors->ptr_guide_vec_orig[i] * SBR_DECAY_GUIDE_ORIG;
    } else {
      thr_org = SBR_THR_TONE_GUIDE;
    }
    if (thr_org > SBR_THR_TONE) {
      thr_org = SBR_THR_TONE;
    }
    if (ptr_guide_vectors->ptr_guide_vec_orig[i]) {
      for (j = lower_band; j < upper_band; j++) {
        if (ptr_quota_buf[j] > thr_org) {
          ptr_harm_vec[i] = 1;
          ptr_new_guide_vectors->ptr_guide_vec_orig[i] = ptr_quota_buf[j];
        }
      }
    }
  }

  thr_org = SBR_THR_TONE;

  for (i = 0; i < num_sfb; i++) {
    lower_band = ptr_freq_band_tab[i];
    upper_band = ptr_freq_band_tab[i + 1];

    if (upper_band - lower_band > 1) {
      for (j = lower_band; j < upper_band; j++) {
        if (ptr_quota_buf[j] > thr_org && ptr_sfm_sbr[i] > SBR_THR_SFM_SBR &&
            ptr_sfm_orig[i] > SBR_THR_SFM_ORG) {
          ptr_harm_vec[i] = 1;
          ptr_new_guide_vectors->ptr_guide_vec_orig[i] = ptr_quota_buf[j];
        }
      }
    } else {
      if (1 < (num_sfb - 1)) {
        lower_band = ptr_freq_band_tab[i];

        if (i > 0) {
          if (ptr_quota_buf[lower_band] > SBR_THR_TONE &&
              (ptr_diff_vec_scfb[i - 1] < SBR_INV_THR_TONE ||
               ptr_diff_vec_scfb[i + 1] < SBR_INV_THR_TONE)) {
            ptr_harm_vec[i] = 1;
            ptr_new_guide_vectors->ptr_guide_vec_orig[i] = ptr_quota_buf[lower_band];
          }
        } else {
          if (ptr_quota_buf[lower_band] > SBR_THR_TONE &&
              ptr_diff_vec_scfb[i + 1] < SBR_INV_THR_TONE) {
            ptr_harm_vec[i] = 1;
            ptr_new_guide_vectors->ptr_guide_vec_orig[i] = ptr_quota_buf[lower_band];
          }
        }
      }
    }
  }
}

static VOID ia_enhaacplus_enc_detection_with_prediction(
    FLOAT32 **ptr_quota_buf, FLOAT32 **ptr_diff_vec_sfb, WORD32 num_sfb,
    const UWORD8 *ptr_freq_band_tab, FLOAT32 **ptr_sfm_orig, FLOAT32 **ptr_sfm_sbr,
    UWORD8 **ptr_detection_vectors, UWORD8 *ptr_prev_frame_sfb_harm,
    ixheaace_str_guide_vectors *ptr_guide_vectors, WORD32 no_est_per_frame, WORD32 tot_no_est,
    WORD32 new_detection_allowed, UWORD8 *ptr_add_harmonics_sfbs) {
  WORD32 est = 0, i;
  WORD32 start;

  memset(ptr_add_harmonics_sfbs, 0, num_sfb * sizeof(ptr_add_harmonics_sfbs[0]));

  if (new_detection_allowed) {
    if (tot_no_est > 1) {
      start = no_est_per_frame;
      memcpy(ptr_guide_vectors[no_est_per_frame].ptr_guide_vec_diff,
             ptr_guide_vectors[0].ptr_guide_vec_diff,
             num_sfb * sizeof(ptr_guide_vectors[no_est_per_frame].ptr_guide_vec_diff[0]));

      memcpy(ptr_guide_vectors[no_est_per_frame].ptr_guide_vec_orig,
             ptr_guide_vectors[0].ptr_guide_vec_orig,
             num_sfb * sizeof(ptr_guide_vectors[no_est_per_frame].ptr_guide_vec_orig[0]));

      memset(
          ptr_guide_vectors[no_est_per_frame - 1].ptr_guide_vector_detected, 0,
          num_sfb * sizeof(ptr_guide_vectors[no_est_per_frame - 1].ptr_guide_vector_detected[0]));
    } else {
      start = 0;
    }
  } else {
    start = 0;
  }

  for (est = start; est < tot_no_est; est++) {
    if (est > 0) {
      memcpy(ptr_guide_vectors[est].ptr_guide_vector_detected, ptr_detection_vectors[est - 1],
             num_sfb * sizeof(ptr_guide_vectors[est].ptr_guide_vector_detected[0]));
    }

    memset(ptr_detection_vectors[est], 0, num_sfb * sizeof(ptr_detection_vectors[est][0]));

    if (est < tot_no_est - 1) {
      memset(ptr_guide_vectors[est + 1].ptr_guide_vec_diff, 0,
             num_sfb * sizeof(ptr_guide_vectors[est + 1].ptr_guide_vec_diff[0]));

      memset(ptr_guide_vectors[est + 1].ptr_guide_vec_orig, 0,
             num_sfb * sizeof(ptr_guide_vectors[est + 1].ptr_guide_vec_orig[0]));

      memset(ptr_guide_vectors[est + 1].ptr_guide_vector_detected, 0,
             num_sfb * sizeof(ptr_guide_vectors[est + 1].ptr_guide_vector_detected[0]));

      ia_enhaacplus_enc_detection(ptr_quota_buf[est], ptr_diff_vec_sfb[est], num_sfb,
                                  ptr_detection_vectors[est], ptr_freq_band_tab,
                                  ptr_sfm_orig[est], ptr_sfm_sbr[est], &(ptr_guide_vectors[est]),
                                  &(ptr_guide_vectors[est + 1]));
    } else {
      memset(ptr_guide_vectors[est].ptr_guide_vec_diff, 0,
             num_sfb * sizeof(ptr_guide_vectors[est].ptr_guide_vec_diff[0]));

      memset(ptr_guide_vectors[est].ptr_guide_vec_orig, 0,
             num_sfb * sizeof(ptr_guide_vectors[est].ptr_guide_vec_orig[0]));

      memset(ptr_guide_vectors[est].ptr_guide_vector_detected, 0,
             num_sfb * sizeof(ptr_guide_vectors[est].ptr_guide_vector_detected[0]));

      ia_enhaacplus_enc_detection(ptr_quota_buf[est], ptr_diff_vec_sfb[est], num_sfb,
                                  ptr_detection_vectors[est], ptr_freq_band_tab,
                                  ptr_sfm_orig[est], ptr_sfm_sbr[est], &(ptr_guide_vectors[est]),
                                  &(ptr_guide_vectors[est]));
    }
  }

  if (new_detection_allowed) {
    if (tot_no_est > 1) {
      ia_enhaacplus_enc_transient_cleanup(ptr_quota_buf, num_sfb, ptr_detection_vectors,
                                          ptr_freq_band_tab, ptr_guide_vectors[no_est_per_frame],
                                          start, tot_no_est);
    } else {
      ia_enhaacplus_enc_transient_cleanup(ptr_quota_buf, num_sfb, ptr_detection_vectors,
                                          ptr_freq_band_tab, ptr_guide_vectors[0], start,
                                          tot_no_est);
    }
  }

  for (i = 0; i < num_sfb; i++) {
    for (est = start; est < tot_no_est; est++) {
      ptr_add_harmonics_sfbs[i] = ptr_add_harmonics_sfbs[i] || ptr_detection_vectors[est][i];
    }
  }

  if (!new_detection_allowed) {
    for (i = 0; i < num_sfb; i++) {
      if (ptr_add_harmonics_sfbs[i] - ptr_prev_frame_sfb_harm[i] > 0) {
        ptr_add_harmonics_sfbs[i] = 0;
      }
    }
  }
}

static VOID ia_enhaacplus_enc_calculate_comp_vector(
    UWORD8 *ptr_add_harmonics_sfbs, FLOAT32 **ptr_tonality, WORD8 *ptr_env_compensation,
    WORD32 num_sfb, const UWORD8 *ptr_freq_band_tab, FLOAT32 **ptr_diff, WORD32 tot_no_est,
    WORD8 *ptr_prev_env_compensation, WORD32 new_detection_allowed) {
  WORD32 i, j, l;

  memset(ptr_env_compensation, 0, num_sfb * sizeof(ptr_env_compensation[0]));

  FLOAT32 max_val;
  WORD32 lower_band, upper_band;
  WORD32 max_pos_band, max_pos_est;
  WORD8 comp_val;

  for (i = 0; i < num_sfb; i++) {
    if (ptr_add_harmonics_sfbs[i]) {
      lower_band = ptr_freq_band_tab[i];
      upper_band = ptr_freq_band_tab[i + 1];

      max_pos_band = 0;
      max_pos_est = 0;
      max_val = 0;

      for (j = 0; j < tot_no_est; j++) {
        for (l = lower_band; l < upper_band; l++) {
          if (ptr_tonality[j][l] > max_val) {
            max_val = ptr_tonality[j][l];
            max_pos_band = l;
            max_pos_est = j;
          }
        }
      }

      if (max_pos_band == lower_band && i) {
        comp_val =
            (WORD8)(fabs(SBR_INV_LOG_2 * log(ptr_diff[max_pos_est][i - 1] + SBR_EPS)) + 0.5f);
        if (comp_val > SBR_MAX_COMP) {
          comp_val = SBR_MAX_COMP;
        }

        if (!ptr_add_harmonics_sfbs[i - 1]) {
          if (ptr_tonality[max_pos_est][max_pos_band - 1] >
              SBR_TONALITY_QUOTA * ptr_tonality[max_pos_est][max_pos_band]) {
            ptr_env_compensation[i - 1] = -1 * comp_val;
          }
        }
      }

      if (max_pos_band == (upper_band - 1) && (i + 1) < num_sfb) {
        comp_val =
            (WORD8)(fabs(SBR_INV_LOG_2 * log(ptr_diff[max_pos_est][i + 1] + SBR_EPS)) + 0.5f);
        if (comp_val > SBR_MAX_COMP) {
          comp_val = SBR_MAX_COMP;
        }

        if (!ptr_add_harmonics_sfbs[i + 1]) {
          if (ptr_tonality[max_pos_est][max_pos_band + 1] >
              SBR_TONALITY_QUOTA * ptr_tonality[max_pos_est][max_pos_band]) {
            ptr_env_compensation[i + 1] = comp_val;
          }
        }
      }

      if (i && i < (num_sfb - 1)) {
        comp_val =
            (WORD8)(fabs(SBR_INV_LOG_2 * log(ptr_diff[max_pos_est][i - 1] + SBR_EPS)) + 0.5f);
        if (comp_val > SBR_MAX_COMP) {
          comp_val = SBR_MAX_COMP;
        }

        if (ixheaace_div32((FLOAT32)1.0f, ptr_diff[max_pos_est][i - 1]) >
            (SBR_DIFF_QUOTA * ptr_diff[max_pos_est][i])) {
          ptr_env_compensation[i - 1] = -1 * comp_val;
        }

        comp_val =
            (WORD8)(fabs(SBR_INV_LOG_2 * log(ptr_diff[max_pos_est][i + 1] + SBR_EPS)) + 0.5f);
        if (comp_val > SBR_MAX_COMP) {
          comp_val = SBR_MAX_COMP;
        }

        if (ixheaace_div32((FLOAT32)1.0f, ptr_diff[max_pos_est][i + 1]) >
            (SBR_DIFF_QUOTA * ptr_diff[max_pos_est][i])) {
          ptr_env_compensation[i + 1] = comp_val;
        }
      }
    }
  }

  if (!new_detection_allowed) {
    for (i = 0; i < num_sfb; i++) {
      if (ptr_env_compensation[i] != 0 && ptr_prev_env_compensation[i] == 0) {
        ptr_env_compensation[i] = 0;
      }
    }
  }
}

VOID ixheaace_sbr_missing_harmonics_detector_qmf(
    ixheaace_pstr_sbr_missing_harmonics_detector pstr_sbr_mhd_et, FLOAT32 **ptr_quota_buf,
    WORD8 *ptr_idx_vx, const ixheaace_str_frame_info_sbr *ptr_frame_info,
    const WORD32 *ptr_tran_info, WORD32 *ptr_add_harmonics_flag, UWORD8 *ptr_add_harmonics_sfbs,
    const UWORD8 *ptr_freq_band_tab, WORD32 num_sfb, WORD8 *ptr_env_compensation) {
  WORD32 i;
  WORD32 transient_flag = ptr_tran_info[1];
  WORD32 transient_pos = ptr_tran_info[0];
  WORD32 new_detection_allowed;

  UWORD8 **ptr_detection_vectors = pstr_sbr_mhd_et->ptr_detection_vectors;
  WORD32 move = pstr_sbr_mhd_et->move;
  WORD32 no_est_per_frame = pstr_sbr_mhd_et->no_est_per_frame;
  WORD32 tot_no_est = pstr_sbr_mhd_et->tot_no_est;
  WORD32 prev_trans_flag = pstr_sbr_mhd_et->prev_trans_flag;
  WORD32 prev_transient_frame = pstr_sbr_mhd_et->prev_trans_frame;
  WORD32 trans_pos_offset = pstr_sbr_mhd_et->trans_pos_offset;
  WORD32 prev_transient_pos = pstr_sbr_mhd_et->prev_trans_pos;
  ixheaace_str_guide_vectors *ptr_guide_vectors = pstr_sbr_mhd_et->guide_vectors;

  FLOAT32 **ptr_sfm_sbr = pstr_sbr_mhd_et->ptr_sfm_sbr;
  FLOAT32 **ptr_sfm_orig = pstr_sbr_mhd_et->ptr_sfm_orig;
  FLOAT32 **ptr_tonal_diff = pstr_sbr_mhd_et->ptr_tonal_diff;

  WORD32 no_qmf_bands = ptr_freq_band_tab[num_sfb] - ptr_freq_band_tab[0];

  new_detection_allowed = ia_enhaacplus_enc_isDetectionOfNewToneAllowed(
      ptr_frame_info, prev_transient_frame, prev_transient_pos, prev_trans_flag, trans_pos_offset,
      transient_flag, transient_pos, pstr_sbr_mhd_et);

  ia_enhaacplus_enc_calculate_detector_input(ptr_quota_buf, ptr_idx_vx, ptr_tonal_diff,
                                             ptr_sfm_orig, ptr_sfm_sbr, ptr_freq_band_tab,
                                             num_sfb, no_est_per_frame, move, no_qmf_bands);

  ia_enhaacplus_enc_detection_with_prediction(
      ptr_quota_buf, ptr_tonal_diff, num_sfb, ptr_freq_band_tab, ptr_sfm_orig, ptr_sfm_sbr,
      ptr_detection_vectors, pstr_sbr_mhd_et->ptr_guide_scfb, ptr_guide_vectors, no_est_per_frame,
      tot_no_est, new_detection_allowed, ptr_add_harmonics_sfbs);

  ia_enhaacplus_enc_calculate_comp_vector(
      ptr_add_harmonics_sfbs, ptr_quota_buf, ptr_env_compensation, num_sfb, ptr_freq_band_tab,
      ptr_tonal_diff, tot_no_est, pstr_sbr_mhd_et->sbr_prev_env_compensation,
      new_detection_allowed);

  *ptr_add_harmonics_flag = 0;

  for (i = 0; i < num_sfb; i++) {
    if (ptr_add_harmonics_sfbs[i]) {
      *ptr_add_harmonics_flag = 1;
      break;
    }
  }

  memcpy(pstr_sbr_mhd_et->sbr_prev_env_compensation, ptr_env_compensation,
         num_sfb * sizeof(pstr_sbr_mhd_et->sbr_prev_env_compensation[0]));

  memcpy(pstr_sbr_mhd_et->ptr_guide_scfb, ptr_add_harmonics_sfbs,
         num_sfb * sizeof(pstr_sbr_mhd_et->ptr_guide_scfb[0]));

  memcpy(ptr_guide_vectors[0].ptr_guide_vector_detected, ptr_add_harmonics_sfbs,
         num_sfb * sizeof(ptr_guide_vectors[0].ptr_guide_vector_detected[0]));

  if (tot_no_est > no_est_per_frame) {
    memcpy(ptr_guide_vectors[0].ptr_guide_vec_diff,
           ptr_guide_vectors[no_est_per_frame].ptr_guide_vec_diff,
           num_sfb * sizeof(ptr_guide_vectors[0].ptr_guide_vec_diff[0]));
    memcpy(ptr_guide_vectors[0].ptr_guide_vec_orig,
           ptr_guide_vectors[no_est_per_frame].ptr_guide_vec_orig,
           num_sfb * sizeof(ptr_guide_vectors[0].ptr_guide_vec_orig[0]));
  } else {
    memcpy(ptr_guide_vectors[0].ptr_guide_vec_diff,
           ptr_guide_vectors[no_est_per_frame - 1].ptr_guide_vec_diff,
           num_sfb * sizeof(ptr_guide_vectors[0].ptr_guide_vec_diff[0]));

    memcpy(ptr_guide_vectors[0].ptr_guide_vec_orig,
           ptr_guide_vectors[no_est_per_frame - 1].ptr_guide_vec_orig,
           num_sfb * sizeof(ptr_guide_vectors[0].ptr_guide_vec_orig[0]));
  }

  for (i = 0; i < num_sfb; i++) {
#ifdef _WIN32
#pragma warning(suppress : 6385)
#endif
    if ((ptr_guide_vectors[0].ptr_guide_vec_diff[i] ||
         ptr_guide_vectors[0].ptr_guide_vec_orig[i]) &&
        !ptr_add_harmonics_sfbs[i]) {
      ptr_guide_vectors[0].ptr_guide_vec_diff[i] = 0;
      ptr_guide_vectors[0].ptr_guide_vec_orig[i] = 0;
    }
  }
}

VOID ixheaace_create_sbr_missing_harmonics_detector(
    WORD32 ch, ixheaace_pstr_sbr_missing_harmonics_detector pstr_sbr_mhdet, WORD32 sample_freq,
    WORD32 num_sfb, WORD32 qmf_num_ch, WORD32 tot_no_est, WORD32 move, WORD32 no_est_per_frame,
    WORD32 *ptr_common_buffer) {
  WORD32 i;
  WORD32 *ptr_fix;
  FLOAT32 *ptr_mem;
  ixheaace_pstr_sbr_missing_harmonics_detector pstr_sbr_detector_handle = pstr_sbr_mhdet;

  memset(pstr_sbr_detector_handle, 0, sizeof(ixheaace_str_sbr_missing_har_detector));

  pstr_sbr_detector_handle->trans_pos_offset = 4;
  pstr_sbr_detector_handle->time_slots = 16;

  pstr_sbr_detector_handle->qmf_num_ch = qmf_num_ch;
  pstr_sbr_detector_handle->sample_freq = sample_freq;
  pstr_sbr_detector_handle->num_scf = num_sfb;

  pstr_sbr_detector_handle->tot_no_est = tot_no_est;
  pstr_sbr_detector_handle->move = move;
  pstr_sbr_detector_handle->no_est_per_frame = no_est_per_frame;

  ptr_fix = &ptr_common_buffer[ch * 5 * NO_OF_ESTIMATES * MAXIMUM_FREQ_COEFFS];
  ptr_mem = (FLOAT32 *)ptr_fix;
  for (i = 0; i < tot_no_est; i++) {
    pstr_sbr_detector_handle->ptr_tonal_diff[i] = ptr_mem;
    ptr_mem += MAXIMUM_FREQ_COEFFS;

    memset(pstr_sbr_detector_handle->ptr_tonal_diff[i], 0,
           sizeof(pstr_sbr_detector_handle->ptr_tonal_diff[0][0]) * MAXIMUM_FREQ_COEFFS);

    pstr_sbr_detector_handle->ptr_sfm_orig[i] = ptr_mem;
    ptr_mem += MAXIMUM_FREQ_COEFFS;

    memset(pstr_sbr_detector_handle->ptr_sfm_orig[i], 0,
           sizeof(pstr_sbr_detector_handle->ptr_sfm_orig[0][0]) * MAXIMUM_FREQ_COEFFS);

    pstr_sbr_detector_handle->ptr_sfm_sbr[i] = ptr_mem;
    ptr_mem += MAXIMUM_FREQ_COEFFS;

    memset(pstr_sbr_detector_handle->ptr_sfm_sbr[i], 0,
           sizeof(pstr_sbr_detector_handle->ptr_sfm_sbr[0][0]) * MAXIMUM_FREQ_COEFFS);

    pstr_sbr_detector_handle->guide_vectors[i].ptr_guide_vec_diff = ptr_mem;
    ptr_mem += MAXIMUM_FREQ_COEFFS;

    memset(pstr_sbr_detector_handle->guide_vectors[i].ptr_guide_vec_diff, 0,
           sizeof(pstr_sbr_detector_handle->guide_vectors[i].ptr_guide_vec_diff[0]) *
               MAXIMUM_FREQ_COEFFS);

    pstr_sbr_detector_handle->guide_vectors[i].ptr_guide_vec_orig = ptr_mem;
    ptr_mem += MAXIMUM_FREQ_COEFFS;

    memset(pstr_sbr_detector_handle->guide_vectors[i].ptr_guide_vec_orig, 0,
           sizeof(pstr_sbr_detector_handle->guide_vectors[i].ptr_guide_vec_orig[0]) *
               MAXIMUM_FREQ_COEFFS);

    pstr_sbr_detector_handle->ptr_detection_vectors[i] =
        &(pstr_sbr_detector_handle->sbr_detection_vectors[i * MAXIMUM_FREQ_COEFFS]);

    memset(pstr_sbr_detector_handle->ptr_detection_vectors[i], 0,
           sizeof(pstr_sbr_detector_handle->ptr_detection_vectors[0][0]) * MAXIMUM_FREQ_COEFFS);

    pstr_sbr_detector_handle->guide_vectors[i].ptr_guide_vector_detected =
        &(pstr_sbr_detector_handle->sbr_guide_vector_detected[i * MAXIMUM_FREQ_COEFFS]);

    memset(pstr_sbr_detector_handle->guide_vectors[i].ptr_guide_vector_detected, 0,
           sizeof(pstr_sbr_detector_handle->guide_vectors[i].ptr_guide_vector_detected[0]) *
               MAXIMUM_FREQ_COEFFS);
  }

  pstr_sbr_detector_handle->ptr_prev_env_compensation =
      &(pstr_sbr_detector_handle->sbr_prev_env_compensation[0]);

  memset(pstr_sbr_detector_handle->ptr_prev_env_compensation, 0,
         sizeof(pstr_sbr_detector_handle->ptr_prev_env_compensation[0]) * MAXIMUM_FREQ_COEFFS);

  pstr_sbr_detector_handle->ptr_guide_scfb = &(pstr_sbr_detector_handle->sbr_guide_scfb[0]);

  memset(pstr_sbr_detector_handle->ptr_guide_scfb, 0,
         sizeof(pstr_sbr_detector_handle->ptr_guide_scfb[0]) * MAXIMUM_FREQ_COEFFS);

  pstr_sbr_detector_handle->prev_trans_flag = 0;
  pstr_sbr_detector_handle->prev_trans_frame = 0;
  pstr_sbr_detector_handle->prev_trans_pos = 0;
}
