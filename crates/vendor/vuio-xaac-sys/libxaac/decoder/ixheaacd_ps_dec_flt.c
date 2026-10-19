/******************************************************************************
 *                                                                            *
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
#include <string.h>
#include <math.h>
#include "ixheaacd_sbr_common.h"
#include "ixheaac_type_def.h"

#include "ixheaac_constants.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops16.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaac_basic_ops_arr.h"
#include "ixheaac_basic_ops.h"

#include "ixheaacd_defines.h"
#include "ixheaac_basic_op.h"
#include "ixheaacd_intrinsics.h"
#include "ixheaacd_common_rom.h"
#include "ixheaacd_basic_funcs.h"
#include "ixheaacd_bitbuffer.h"
#include "ixheaacd_sbrdecsettings.h"
#include "ixheaacd_defines.h"

#include "ixheaacd_pns.h"

#include "ixheaacd_aac_rom.h"
#include "ixheaacd_pulsedata.h"

#include "ixheaacd_drc_data_struct.h"
#include "ixheaacd_lt_predict.h"
#include "ixheaacd_cnst.h"
#include "ixheaacd_ec_defines.h"
#include "ixheaacd_ec_struct_def.h"
#include "ixheaacd_channelinfo.h"
#include "ixheaacd_drc_dec.h"

#include "ixheaacd_sbrdecoder.h"
#include "ixheaacd_sbr_scale.h"
#include "ixheaacd_lpp_tran.h"
#include "ixheaacd_env_extr_part.h"
#include "ixheaacd_sbr_rom.h"

#include "ixheaacd_hybrid.h"
#include "ixheaacd_ps_dec.h"

#include "ixheaacd_env_extr.h"

#include "ixheaacd_ps_dec.h"

#include "ixheaacd_qmf_dec.h"
#include "ixheaacd_env_calc.h"
#include "ixheaac_sbr_const.h"

#include "ixheaacd_pvc_dec.h"
#include "ixheaacd_sbr_dec.h"
#include "ixheaacd_function_selector.h"

VOID ixheaacd_k_chan_filt(const FLOAT32 *ptr_qmf_re, const FLOAT32 *ptr_qmf_im,
                          FLOAT32 (*ptr_tmp_hyb_re)[MAX_NUM_QMF_CHANNELS],
                          FLOAT32 (*ptr_tmp_hyb_im)[MAX_NUM_QMF_CHANNELS], WORD32 nSamples,
                          WORD32 k, WORD32 bCplx, const FLOAT32 *p,
                          FLOAT32 *cos_sin_mod_tbl) {
  WORD32 i, n, q;
  FLOAT32 real, imag;
  FLOAT32 cos_val, sin_val;
  FLOAT32 *p_real_imag = cos_sin_mod_tbl;

  if (bCplx) {
    for (i = 0; i < nSamples; i++) {
      for (q = 0; q < k; q++) {
        real = 0;
        imag = 0;
        for (n = 0; n < 13; n++) {
          cos_val = *p_real_imag++;
          sin_val = *p_real_imag++;
          real += p[n] * (ptr_qmf_re[n + i] * cos_val - ptr_qmf_im[n + i] * sin_val);
          imag += p[n] * (ptr_qmf_im[n + i] * cos_val + ptr_qmf_re[n + i] * sin_val);
        }
        ptr_tmp_hyb_re[i][q] = real;
        ptr_tmp_hyb_im[i][q] = imag;
      }
      p_real_imag -= (13 * k * 2);
    }
  } else {
    for (i = 0; i < nSamples; i++) {
      for (q = 0; q < k; q++) {
        real = 0;
        imag = 0;
        for (n = 0; n < 13; n++) {
          cos_val = *p_real_imag++;
          real += p[n] * (ptr_qmf_re[n + i] * cos_val);
          imag += p[n] * (ptr_qmf_im[n + i] * cos_val);
        }
        ptr_tmp_hyb_re[i][q] = real;
        ptr_tmp_hyb_im[i][q] = imag;
      }
      p_real_imag -= (13 * k);
    }
  }
}

VOID ixheaacd_hyb_anal(const FLOAT32 **ptr_qmf_re, const FLOAT32 **ptr_qmf_im,
                       ia_ps_dec_struct *ptr_ps_dec, ia_ps_tables_struct *ptr_ps_tables,
                       WORD32 use_34_st_bands) {
  WORD32 k, n, band;
  WORD32 band_res;
  WORD32 frame_size;
  WORD32 ch_offset = 0;
  FLOAT32(*ptr_tmp_hyb_re)[MAX_NUM_COLUMNS];
  FLOAT32(*ptr_tmp_hyb_im)[MAX_NUM_COLUMNS];
  ia_hybrid_flt_struct *ptr_hybrid;
  if (!use_34_st_bands) {
    ptr_tmp_hyb_re = ptr_ps_dec->hyb_left_re;
    ptr_tmp_hyb_im = ptr_ps_dec->hyb_left_im;
    ptr_hybrid = &ptr_ps_dec->str_flt_hybrid20;
  } else {
    ptr_tmp_hyb_re = NULL;
    ptr_tmp_hyb_im = ptr_ps_dec->hyb_left_im;
    ptr_hybrid = &ptr_ps_dec->str_flt_hybrid34;
  }
  frame_size = ptr_hybrid->frame_size;
  for (band = 0; band < ptr_hybrid->num_qmf_bands; band++) {
    band_res = ptr_hybrid->ptr_resol[band];

    memcpy(ptr_hybrid->ptr_work_re, ptr_hybrid->ptr_qmf_buf_re[band],
           (HYBRID_FILTER_LENGTH - 1) * sizeof(ptr_hybrid->ptr_work_re[0]));
    memcpy(ptr_hybrid->ptr_work_im, ptr_hybrid->ptr_qmf_buf_im[band],
           (HYBRID_FILTER_LENGTH - 1) * sizeof(ptr_hybrid->ptr_work_im[0]));

    for (n = 0; n < frame_size; n++) {
      ptr_hybrid->ptr_work_re[(HYBRID_FILTER_LENGTH - 1) + n] =
          ptr_qmf_re[n + HYBRID_FILTER_DELAY][band];
      ptr_hybrid->ptr_work_im[(HYBRID_FILTER_LENGTH - 1) + n] =
          ptr_qmf_im[n + HYBRID_FILTER_DELAY][band];
    }

    memcpy(ptr_hybrid->ptr_qmf_buf_re[band], ptr_hybrid->ptr_work_re + frame_size,
           (HYBRID_FILTER_LENGTH - 1) * sizeof(ptr_hybrid->ptr_qmf_buf_re[band][0]));
    memcpy(ptr_hybrid->ptr_qmf_buf_im[band], ptr_hybrid->ptr_work_im + frame_size,
           (HYBRID_FILTER_LENGTH - 1) * sizeof(ptr_hybrid->ptr_qmf_buf_im[band][0]));

    if (ptr_tmp_hyb_re) {
      switch (band_res) {
        case NO_HYBRID_CHANNELS_2:
          ixheaacd_k_chan_filt(
              ptr_hybrid->ptr_work_re, ptr_hybrid->ptr_work_im, ptr_hybrid->ptr_temp_re,
              ptr_hybrid->ptr_temp_im, frame_size, NO_HYBRID_CHANNELS_2, REAL,
              ptr_ps_tables->p2_13_20,
              &ptr_ps_tables->cos_mod_2channel[0][0]);
          break;
        case NO_HYBRID_CHANNELS_4:
          ixheaacd_k_chan_filt(
              ptr_hybrid->ptr_work_re, ptr_hybrid->ptr_work_im, ptr_hybrid->ptr_temp_re,
              ptr_hybrid->ptr_temp_im, frame_size, NO_HYBRID_CHANNELS_4, CPLX,
              ptr_ps_tables->p4_13_34,
              &ptr_ps_tables->cos_sin_mod_4channel[0][0]);
          break;
        case NO_HYBRID_CHANNELS_8:
          ixheaacd_k_chan_filt(
              ptr_hybrid->ptr_work_re, ptr_hybrid->ptr_work_im, ptr_hybrid->ptr_temp_re,
              ptr_hybrid->ptr_temp_im, frame_size, NO_HYBRID_CHANNELS_8, CPLX,
              use_34_st_bands ? ptr_ps_tables->p8_13_34 : ptr_ps_tables->p8_13_20,
              &ptr_ps_tables->cos_sin_mod_8channel[0][0]);
          break;
        case NO_HYBRID_CHANNELS_12:
          ixheaacd_k_chan_filt(ptr_hybrid->ptr_work_re, ptr_hybrid->ptr_work_im,
                               ptr_hybrid->ptr_temp_re, ptr_hybrid->ptr_temp_im, frame_size,
                               NO_HYBRID_CHANNELS_12, CPLX, ptr_ps_tables->p12_13_34,
                               &ptr_ps_tables->cos_sin_mod_12channel[0][0]);
          break;
        default:
          break;
      }

      for (n = 0; n < frame_size; n++) {
        for (k = 0; k < (WORD32)band_res; k++) {
          ptr_tmp_hyb_re[n][ch_offset + k] = ptr_hybrid->ptr_temp_re[n][k];
          ptr_tmp_hyb_im[n][ch_offset + k] = ptr_hybrid->ptr_temp_im[n][k];
        }
      }
      ch_offset += band_res;
    }
  }
}

VOID ixheaacd_hyb_synth(
    FLOAT32 (*ptr_tmp_hyb_re)[MAX_NUM_COLUMNS],
    FLOAT32 (*ptr_tmp_hyb_im)[MAX_NUM_COLUMNS],
    FLOAT32 **ptr_qmf_re,
    FLOAT32 **ptr_qmf_im,
    ia_hybrid_flt_struct *ptr_hybrid) {
  WORD32 k, n, band;
  WORD16 band_res;
  WORD32 frame_size = ptr_hybrid->frame_size;
  WORD32 ch_offset = 0;

  for (band = 0; band < ptr_hybrid->num_qmf_bands; band++) {
    band_res = ptr_hybrid->ptr_resol[band];

    for (n = 0; n < frame_size; n++) {
      ptr_qmf_re[n][band] = ptr_qmf_im[n][band] = 0;

      for (k = 0; k < (WORD32)band_res; k++) {
        ptr_qmf_re[n][band] += ptr_tmp_hyb_re[n][ch_offset + k];
        ptr_qmf_im[n][band] += ptr_tmp_hyb_im[n][ch_offset + k];
      }
    }
    ch_offset += band_res;
  }
}

VOID ixheaacd_map_34_float_to_20(FLOAT32 *ptr_index) {
  ptr_index[0] = (2 * ptr_index[0] + ptr_index[1]) / 3.0f;
  ptr_index[1] = (ptr_index[1] + 2 * ptr_index[2]) / 3.0f;
  ptr_index[2] = (2 * ptr_index[3] + ptr_index[4]) / 3.0f;
  ptr_index[3] = (ptr_index[4] + 2 * ptr_index[5]) / 3.0f;
  ptr_index[4] = (ptr_index[6] + ptr_index[7]) / 2.0f;
  ptr_index[5] = (ptr_index[8] + ptr_index[9]) / 2.0f;
  ptr_index[6] = ptr_index[10];
  ptr_index[7] = ptr_index[11];
  ptr_index[8] = (ptr_index[12] + ptr_index[13]) / 2.0f;
  ptr_index[9] = (ptr_index[14] + ptr_index[15]) / 2.0f;
  ptr_index[10] = ptr_index[16];
  ptr_index[11] = ptr_index[17];
  ptr_index[12] = ptr_index[18];
  ptr_index[13] = ptr_index[19];
  ptr_index[14] = (ptr_index[20] + ptr_index[21]) / 2.0f;
  ptr_index[15] = (ptr_index[22] + ptr_index[23]) / 2.0f;
  ptr_index[16] = (ptr_index[24] + ptr_index[25]) / 2.0f;
  ptr_index[17] = (ptr_index[26] + ptr_index[27]) / 2.0f;
  ptr_index[18] = (ptr_index[28] + ptr_index[29] + ptr_index[30] + ptr_index[31]) / 4.0f;
  ptr_index[19] = (ptr_index[32] + ptr_index[33]) / 2.0f;
}

VOID ixheaacd_map_20_float_to_34(FLOAT32 *ptr_index) {
  FLOAT32 arr_temp[NUM_HI_RES_BINS];
  WORD32 i;

  arr_temp[0] = ptr_index[0];
  arr_temp[1] = (ptr_index[0] + ptr_index[1]) / 2.0f;
  arr_temp[2] = ptr_index[1];
  arr_temp[3] = ptr_index[2];
  arr_temp[4] = (ptr_index[2] + ptr_index[3]) / 2.0f;
  arr_temp[5] = ptr_index[3];
  arr_temp[6] = ptr_index[4];
  arr_temp[7] = ptr_index[4];
  arr_temp[8] = ptr_index[5];
  arr_temp[9] = ptr_index[5];
  arr_temp[10] = ptr_index[6];
  arr_temp[11] = ptr_index[7];
  arr_temp[12] = ptr_index[8];
  arr_temp[13] = ptr_index[8];
  arr_temp[14] = ptr_index[9];
  arr_temp[15] = ptr_index[9];
  arr_temp[16] = ptr_index[10];
  arr_temp[17] = ptr_index[11];
  arr_temp[18] = ptr_index[12];
  arr_temp[19] = ptr_index[13];
  arr_temp[20] = ptr_index[14];
  arr_temp[21] = ptr_index[14];
  arr_temp[22] = ptr_index[15];
  arr_temp[23] = ptr_index[15];
  arr_temp[24] = ptr_index[16];
  arr_temp[25] = ptr_index[16];
  arr_temp[26] = ptr_index[17];
  arr_temp[27] = ptr_index[17];
  arr_temp[28] = ptr_index[18];
  arr_temp[29] = ptr_index[18];
  arr_temp[30] = ptr_index[18];
  arr_temp[31] = ptr_index[18];
  arr_temp[32] = ptr_index[19];
  arr_temp[33] = ptr_index[19];

  for (i = 0; i < 34; i++) {
    ptr_index[i] = arr_temp[i];
  }
}

WORD32 ixheaacd_create_ps_esbr_dec(ia_ps_dec_struct *ptr_ps_dec_struct,
                                   ia_ps_tables_struct *ptr_ps_tables, UWORD32 noQmfChans,
                                   UWORD32 num_sub_samples, WORD32 ps_mode) {
  UWORD32 i;
  ia_ps_dec_struct *ptr_ps_dec = ptr_ps_dec_struct;

  ptr_ps_dec = ptr_ps_dec_struct;

  ptr_ps_dec->num_sub_samples = num_sub_samples;
  ptr_ps_dec->num_chans = noQmfChans;
  ptr_ps_dec->ps_mode = ps_mode;

  ptr_ps_dec->ps_data_present = 0;
  ptr_ps_dec->enable_iid = 0;
  ptr_ps_dec->iid_mode = 0;
  ptr_ps_dec->enable_icc = 0;
  ptr_ps_dec->icc_mode = 0;
  ptr_ps_dec->enable_ext = 0;

  ptr_ps_dec->use_pca_rot_flg = 0;
  ptr_ps_dec->freq_res_ipd = 0;
  ptr_ps_dec->use_34_st_bands = 0;
  ptr_ps_dec->use_34_st_bands_prev = 0;

  ptr_ps_dec->str_flt_hybrid20.frame_size = ptr_ps_dec->num_sub_samples;
  ptr_ps_dec->str_flt_hybrid20.num_qmf_bands = NUM_QMF_BANDS_IN_HYBRID20;
  ptr_ps_dec->str_flt_hybrid20.ptr_resol = (WORD16 *)&ptr_ps_tables->band_res_hyb20[0];
  ptr_ps_dec->str_flt_hybrid20.ptr_work_re = &ptr_ps_dec->hyb_work_re_20[0];
  ptr_ps_dec->str_flt_hybrid20.ptr_work_im = &ptr_ps_dec->hyb_work_im_20[0];
  ptr_ps_dec->str_flt_hybrid20.ptr_qmf_buf_re = ptr_ps_dec->hyb_qmf_buf_re_20;
  ptr_ps_dec->str_flt_hybrid20.ptr_qmf_buf_im = ptr_ps_dec->hyb_qmf_buf_im_20;
  ptr_ps_dec->str_flt_hybrid20.ptr_temp_re = ptr_ps_dec->hyb_temp_re_20;
  ptr_ps_dec->str_flt_hybrid20.ptr_temp_im = ptr_ps_dec->hyb_temp_im_20;

  ptr_ps_dec->str_flt_hybrid34.frame_size = ptr_ps_dec->num_sub_samples;
  ptr_ps_dec->str_flt_hybrid34.num_qmf_bands = NUM_QMF_BANDS_IN_HYBRID34;
  ptr_ps_dec->str_flt_hybrid34.ptr_resol = (WORD16 *)&ptr_ps_tables->band_res_hyb34[0];
  ptr_ps_dec->str_flt_hybrid34.ptr_work_re = &ptr_ps_dec->hyb_work_re_34[0];
  ptr_ps_dec->str_flt_hybrid34.ptr_work_im = &ptr_ps_dec->hyb_work_im_34[0];
  ptr_ps_dec->str_flt_hybrid34.ptr_qmf_buf_re = ptr_ps_dec->hyb_qmf_buf_re_34;
  ptr_ps_dec->str_flt_hybrid34.ptr_qmf_buf_im = ptr_ps_dec->hyb_qmf_buf_im_34;
  ptr_ps_dec->str_flt_hybrid34.ptr_temp_re = ptr_ps_dec->hyb_temp_re_34;
  ptr_ps_dec->str_flt_hybrid34.ptr_temp_im = ptr_ps_dec->hyb_temp_im_34;

  ptr_ps_dec->delay_buf_idx = 0;

  for (i = 0; i < NUM_OF_QUAD_MIRROR_FILTER_CHNLS; i++) {
    ptr_ps_dec->delay_qmf_delay_buf_idx[i] = 0;
    ptr_ps_dec->delay_qmf_delay_num_samp[i] = ptr_ps_tables->qmf_delay_idx_tbl[i];
  }

  for (i = 0; i < NUM_HI_RES_BINS; i++) {
    ptr_ps_dec->h11_re_prev[i] = 1.0f;
    ptr_ps_dec->h12_re_prev[i] = 1.0f;
  }

  memset(ptr_ps_dec->h11_im_prev, 0, sizeof(ptr_ps_dec->h11_im_prev));
  memset(ptr_ps_dec->h12_im_prev, 0, sizeof(ptr_ps_dec->h12_im_prev));
  memset(ptr_ps_dec->h21_re_prev, 0, sizeof(ptr_ps_dec->h21_re_prev));
  memset(ptr_ps_dec->h22_re_prev, 0, sizeof(ptr_ps_dec->h22_re_prev));
  memset(ptr_ps_dec->h21_im_prev, 0, sizeof(ptr_ps_dec->h21_im_prev));
  memset(ptr_ps_dec->h22_im_prev, 0, sizeof(ptr_ps_dec->h22_im_prev));

  memset(ptr_ps_dec->ipd_idx_map_1, 0, sizeof(ptr_ps_dec->ipd_idx_map_1));
  memset(ptr_ps_dec->opd_idx_map_1, 0, sizeof(ptr_ps_dec->opd_idx_map_1));
  memset(ptr_ps_dec->ipd_idx_map_2, 0, sizeof(ptr_ps_dec->ipd_idx_map_2));
  memset(ptr_ps_dec->opd_idx_map_2, 0, sizeof(ptr_ps_dec->opd_idx_map_2));

  for (i = 0; i < NUM_HI_RES_BINS; i++) {
    ptr_ps_dec->peak_decay_fast_bin[i] = 0.0f;
    ptr_ps_dec->prev_nrg_bin[i] = 0.0f;
    ptr_ps_dec->prev_peak_diff_bin[i] = 0.0f;
  }

  memset(ptr_ps_dec->qmf_delay_buf_re, 0, sizeof(ptr_ps_dec->qmf_delay_buf_re));
  memset(ptr_ps_dec->qmf_delay_buf_im, 0, sizeof(ptr_ps_dec->qmf_delay_buf_im));
  memset(ptr_ps_dec->sub_qmf_delay_buf_re, 0, sizeof(ptr_ps_dec->sub_qmf_delay_buf_re));
  memset(ptr_ps_dec->sub_qmf_delay_buf_im, 0, sizeof(ptr_ps_dec->sub_qmf_delay_buf_im));

  for (i = 0; i < NUM_SER_AP_LINKS; i++) {
    memset(&ptr_ps_dec->ser_qmf_delay_buf_re[i][0][0], 0,
           sizeof(ptr_ps_dec->ser_qmf_delay_buf_re[i]));
    memset(&ptr_ps_dec->ser_qmf_delay_buf_im[i][0][0], 0,
           sizeof(ptr_ps_dec->ser_qmf_delay_buf_im[i]));
    memset(&ptr_ps_dec->ser_sub_qmf_dealy_buf_re[i][0][0], 0,
           sizeof(ptr_ps_dec->ser_sub_qmf_dealy_buf_re[i]));
    memset(&ptr_ps_dec->ser_sub_qmf_dealy_buf_im[i][0][0], 0,
           sizeof(ptr_ps_dec->ser_sub_qmf_dealy_buf_im[i]));
  }

  return 0;
}

VOID ixheaacd_esbr_apply_ps(ia_ps_dec_struct *ptr_ps_dec,
                            FLOAT32 **pp_qmf_buf_re_left,
                            FLOAT32 **pp_qmf_buf_im_left,
                            FLOAT32 **pp_qmf_buf_re_right,
                            FLOAT32 **pp_qmf_buf_im_right,
                            WORD32 usb, ia_ps_tables_struct *ptr_ps_tables,
                            WORD32 num_time_slot) {
  WORD32 sb;
  WORD32 i, k;

  WORD32 max_num_column;

  if (num_time_slot == 15)
    max_num_column = MAX_NUM_COLUMNS_960;
  else
    max_num_column = MAX_NUM_COLUMNS;

  if (ptr_ps_dec->use_34_st_bands) {
    ptr_ps_dec->ptr_group_borders = (WORD32 *)&ptr_ps_tables->group_borders_34_tbl[0];
    ptr_ps_dec->ptr_bins_group_map = (WORD32 *)&ptr_ps_tables->bin_group_map_34[0];
    ptr_ps_dec->ptr_hybrid = &ptr_ps_dec->str_flt_hybrid34;
    ptr_ps_dec->num_groups = NUM_IID_GROUPS_HI_RES;
    ptr_ps_dec->num_sub_qmf_groups = SUBQMF_GROUPS_HI_RES;
    ptr_ps_dec->num_bins = NUM_HI_RES_BINS;
    ptr_ps_dec->first_delay_gr = SUBQMF_GROUPS_HI_RES;
  } else {
    ptr_ps_dec->ptr_group_borders = (WORD32 *)&ptr_ps_tables->group_borders_20_tbl[0];
    ptr_ps_dec->ptr_bins_group_map = (WORD32 *)&ptr_ps_tables->bin_group_map_20[0];
    ptr_ps_dec->ptr_hybrid = &ptr_ps_dec->str_flt_hybrid20;
    ptr_ps_dec->num_groups = NUM_IID_GROUPS;
    ptr_ps_dec->num_sub_qmf_groups = SUBQMF_GROUPS;
    ptr_ps_dec->num_bins = NUM_MID_RES_BINS;
    ptr_ps_dec->first_delay_gr = SUBQMF_GROUPS;
  }

  for (sb = usb; sb < ptr_ps_dec->num_chans; sb++) {
    for (i = 0; i < NUM_SER_AP_LINKS; i++) {
      for (k = 0; k < ptr_ps_dec->delay_sample_ser[i]; k++) {
        ptr_ps_dec->ser_qmf_delay_buf_re[i][k][sb] = 0;
        ptr_ps_dec->ser_qmf_delay_buf_im[i][k][sb] = 0;
      }
    }
    for (k = 0; k < HIGH_DEL; k++) {
      ptr_ps_dec->qmf_delay_buf_re[k][sb] = 0;
      ptr_ps_dec->qmf_delay_buf_im[k][sb] = 0;
    }
  }
  ixheaacd_hyb_anal((const FLOAT32 **)pp_qmf_buf_re_left,
                    (const FLOAT32 **)pp_qmf_buf_im_left,
                    ptr_ps_dec, ptr_ps_tables, 0);
  ixheaacd_hyb_anal((const FLOAT32 **)pp_qmf_buf_re_left,
                    (const FLOAT32 **)pp_qmf_buf_im_left,
                    ptr_ps_dec, ptr_ps_tables, 1);

  if (!ptr_ps_dec->use_34_st_bands) {
    WORD32 k;
    for (k = 0; k < (WORD32)ptr_ps_dec->num_sub_samples; k++) {
      ptr_ps_dec->hyb_left_re[k][3] += ptr_ps_dec->hyb_left_re[k][4];
      ptr_ps_dec->hyb_left_im[k][3] += ptr_ps_dec->hyb_left_im[k][4];
      ptr_ps_dec->hyb_left_re[k][4] = 0.;
      ptr_ps_dec->hyb_left_im[k][4] = 0.;

      ptr_ps_dec->hyb_left_re[k][2] += ptr_ps_dec->hyb_left_re[k][5];
      ptr_ps_dec->hyb_left_im[k][2] += ptr_ps_dec->hyb_left_im[k][5];
      ptr_ps_dec->hyb_left_re[k][5] = 0.;
      ptr_ps_dec->hyb_left_im[k][5] = 0.;
    }
  }

  if (ptr_ps_dec->ps_mode & 0x0080) {
    WORD32 i, j;
    for (i = 0; i < max_num_column; i++) {
      for (j = 0; j < NUM_OF_QUAD_MIRROR_FILTER_CHNLS; j++) {
        pp_qmf_buf_im_right[i][j] = pp_qmf_buf_im_left[i][j];
        pp_qmf_buf_re_right[i][j] = pp_qmf_buf_re_left[i][j];
      }
    }
    for (i = 0; i < max_num_column; i++) {
      for (j = 0; j < NUM_SUB_QMF_CHANNELS_HI_RES; j++) {
        ptr_ps_dec->hyb_right_re[i][j] = ptr_ps_dec->hyb_left_re[i][j];
        ptr_ps_dec->hyb_right_im[i][j] = ptr_ps_dec->hyb_left_im[i][j];
      }
    }
  } else {
    if (ptr_ps_dec->ps_mode & 0x0002) {
      WORD32 i, j;
      for (i = 0; i < max_num_column; i++) {
        for (j = 0; j < NUM_OF_QUAD_MIRROR_FILTER_CHNLS; j++) {
          pp_qmf_buf_im_right[i][j] = 0.;
          pp_qmf_buf_re_right[i][j] = 0.;
        }
      }
      for (i = 0; i < max_num_column; i++) {
        for (j = 0; j < NUM_SUB_QMF_CHANNELS_HI_RES; j++) {
          ptr_ps_dec->hyb_right_re[i][j] = 0.;
          ptr_ps_dec->hyb_right_im[i][j] = 0.;
        }
      }
    } else {
      ixheaacd_esbr_ps_de_correlate(ptr_ps_dec, pp_qmf_buf_re_left, pp_qmf_buf_im_left,
                                    pp_qmf_buf_re_right, pp_qmf_buf_im_right,
                                    ptr_ps_tables);
    }

    if (!(ptr_ps_dec->ps_mode & 0x0040)) {
      ixheaacd_esbr_ps_apply_rotation(ptr_ps_dec, pp_qmf_buf_re_left, pp_qmf_buf_im_left,
                                      pp_qmf_buf_re_right, pp_qmf_buf_im_right,
                                      ptr_ps_tables);
    }
  }

  ixheaacd_hyb_synth(ptr_ps_dec->hyb_left_re, ptr_ps_dec->hyb_left_im,
                     pp_qmf_buf_re_left, pp_qmf_buf_im_left, ptr_ps_dec->ptr_hybrid);

  ixheaacd_hyb_synth(ptr_ps_dec->hyb_right_re, ptr_ps_dec->hyb_right_im,
                     pp_qmf_buf_re_right, pp_qmf_buf_im_right, ptr_ps_dec->ptr_hybrid);

  ptr_ps_dec->use_34_st_bands_prev = ptr_ps_dec->use_34_st_bands;

}

VOID ixheaacd_esbr_ps_de_correlate(
    ia_ps_dec_struct *ptr_ps_dec,
    FLOAT32 **pp_qmf_buf_re_left,
    FLOAT32 **pp_qmf_buf_im_left,
    FLOAT32 **pp_qmf_buf_re_right,
    FLOAT32 **pp_qmf_buf_im_right,
    ia_ps_tables_struct *ptr_ps_tables) {
  WORD32 sb, maxsb, gr, k;
  WORD32 m;
  WORD32 l_delay = 0;
  WORD32 l_ser_delay_arr[NUM_SER_AP_LINKS] = {0};
  FLOAT32 re_left;
  FLOAT32 im_left;
  FLOAT32 peak_diff, nrg, trans_ratio;

  FLOAT32(*pp_hyb_left_re)[MAX_NUM_COLUMNS];
  FLOAT32(*pp_hyb_left_im)[MAX_NUM_COLUMNS];
  FLOAT32(*pp_hyb_right_re)[MAX_NUM_COLUMNS];
  FLOAT32(*pp_hyb_right_im)[MAX_NUM_COLUMNS];

  FLOAT32(*ppp_ser_sub_qmf_dealy_buf_re)[5][NUM_OF_QUAD_MIRROR_FILTER_CHNLS];
  FLOAT32(*ppp_ser_sub_qmf_dealy_buf_im)[5][NUM_OF_QUAD_MIRROR_FILTER_CHNLS];
  FLOAT32(*pp_sub_qmf_delay_buf_re)[NUM_OF_QUAD_MIRROR_FILTER_CHNLS];
  FLOAT32(*pp_sub_qmf_delay_buf_im)[NUM_OF_QUAD_MIRROR_FILTER_CHNLS];

  FLOAT32 *pp_frac_delay_phase_fac_re;
  FLOAT32 *pp_frac_delay_phase_fac_im;
  FLOAT32(*pp_frac_delay_phase_fac_ser_re)[NUM_SER_AP_LINKS];
  FLOAT32(*pp_frac_delay_phase_fac_ser_im)[NUM_SER_AP_LINKS];

  WORD32 *p_delay_qmf_delay_num_samp = NULL;
  WORD32 *p_delay_qmf_delay_buf_idx = NULL;

  FLOAT32 pow_arr[32][NUM_HI_RES_BINS];
  FLOAT32 trans_ratio_arr[32][NUM_HI_RES_BINS];
  WORD32 bin;
  FLOAT32 decay_cutoff;

  pp_hyb_left_re = ptr_ps_dec->hyb_left_re;
  pp_hyb_left_im = ptr_ps_dec->hyb_left_im;
  pp_hyb_right_re = ptr_ps_dec->hyb_right_re;
  pp_hyb_right_im = ptr_ps_dec->hyb_right_im;

  ppp_ser_sub_qmf_dealy_buf_re = ptr_ps_dec->ser_sub_qmf_dealy_buf_re;
  ppp_ser_sub_qmf_dealy_buf_im = ptr_ps_dec->ser_sub_qmf_dealy_buf_im;

  pp_sub_qmf_delay_buf_re = ptr_ps_dec->sub_qmf_delay_buf_re;
  pp_sub_qmf_delay_buf_im = ptr_ps_dec->sub_qmf_delay_buf_im;

  if (ptr_ps_dec->use_34_st_bands != ptr_ps_dec->use_34_st_bands_prev) {
    if (ptr_ps_dec->use_34_st_bands) {
      WORD32 i;
      for (i = 0; i < NUM_SER_AP_LINKS; i++) {
        memset(&ptr_ps_dec->ser_qmf_delay_buf_re[i][0][0], 0,
               sizeof(ptr_ps_dec->ser_qmf_delay_buf_re[i]));
        memset(&ptr_ps_dec->ser_qmf_delay_buf_im[i][0][0], 0,
               sizeof(ptr_ps_dec->ser_qmf_delay_buf_im[i]));
        memset(&ptr_ps_dec->ser_sub_qmf_dealy_buf_re[i][0][0], 0,
               sizeof(ptr_ps_dec->ser_sub_qmf_dealy_buf_re[i]));
        memset(&ptr_ps_dec->ser_sub_qmf_dealy_buf_im[i][0][0], 0,
               sizeof(ptr_ps_dec->ser_sub_qmf_dealy_buf_im[i]));
      }
      return;
    } else {
      WORD32 i;
      for (i = 0; i < NUM_SER_AP_LINKS; i++) {
        memset(&ptr_ps_dec->ser_qmf_delay_buf_re[i][0][0], 0,
               sizeof(ptr_ps_dec->ser_qmf_delay_buf_re[i]));
        memset(&ptr_ps_dec->ser_qmf_delay_buf_im[i][0][0], 0,
               sizeof(ptr_ps_dec->ser_qmf_delay_buf_im[i]));
        memset(&ptr_ps_dec->ser_sub_qmf_dealy_buf_re[i][0][0], 0,
               sizeof(ptr_ps_dec->ser_sub_qmf_dealy_buf_re[i]));
        memset(&ptr_ps_dec->ser_sub_qmf_dealy_buf_im[i][0][0], 0,
               sizeof(ptr_ps_dec->ser_sub_qmf_dealy_buf_im[i]));
      }

      return;
    }
  }

  if (ptr_ps_dec->use_34_st_bands) {
    pp_frac_delay_phase_fac_re = ptr_ps_tables->frac_delay_phase_fac_qmf_sub_re_34;
    pp_frac_delay_phase_fac_im = ptr_ps_tables->frac_delay_phase_fac_qmf_sub_im_34;
    pp_frac_delay_phase_fac_ser_re = ptr_ps_tables->frac_delay_phase_fac_ser_qmf_sub_re_34;
    pp_frac_delay_phase_fac_ser_im = ptr_ps_tables->frac_delay_phase_fac_ser_qmf_sub_im_34;
  } else {
    pp_frac_delay_phase_fac_re = ptr_ps_tables->frac_delay_phase_fac_qmf_sub_re_20;
    pp_frac_delay_phase_fac_im = ptr_ps_tables->frac_delay_phase_fac_qmf_sub_im_20;
    pp_frac_delay_phase_fac_ser_re = ptr_ps_tables->frac_delay_phase_fac_ser_qmf_sub_re_20;
    pp_frac_delay_phase_fac_ser_im = ptr_ps_tables->frac_delay_phase_fac_ser_qmf_sub_im_20;
  }

  for (k = 0; k < 32; k++) {
    for (bin = 0; bin < NUM_HI_RES_BINS; bin++) {
      pow_arr[k][bin] = 0;
    }
  }

  for (gr = 0; gr < ptr_ps_dec->num_sub_qmf_groups; gr++) {
    bin = (~NEGATE_IPD_MASK) & ptr_ps_dec->ptr_bins_group_map[gr];
    maxsb = ptr_ps_dec->ptr_group_borders[gr] + 1;
    for (sb = ptr_ps_dec->ptr_group_borders[gr]; sb < maxsb; sb++) {
      for (k = ptr_ps_dec->border_position[0];
           k < ptr_ps_dec->border_position[ptr_ps_dec->num_env]; k++) {
        im_left = pp_hyb_left_re[k][sb];
        re_left = pp_hyb_left_im[k][sb];
        pow_arr[k][bin] += im_left * im_left + re_left * re_left;
      }
    }
  }
  for (; gr < ptr_ps_dec->num_groups; gr++) {
    bin = (~NEGATE_IPD_MASK) & ptr_ps_dec->ptr_bins_group_map[gr];
    maxsb = ptr_ps_dec->ptr_group_borders[gr + 1];
    for (sb = ptr_ps_dec->ptr_group_borders[gr]; sb < maxsb; sb++) {
      for (k = ptr_ps_dec->border_position[0];
           k < ptr_ps_dec->border_position[ptr_ps_dec->num_env]; k++) {
        im_left = pp_qmf_buf_re_left[k][sb];
        re_left = pp_qmf_buf_im_left[k][sb];
        pow_arr[k][bin] += im_left * im_left + re_left * re_left;
      }
    }
  }

  for (bin = 0; bin < ptr_ps_dec->num_bins; bin++) {
    for (k = ptr_ps_dec->border_position[0];
         k < ptr_ps_dec->border_position[ptr_ps_dec->num_env]; k++) {
      FLOAT32 q = 1.5f;

      ptr_ps_dec->peak_decay_fast_bin[bin] *= PEAK_DECAY_FACTOR_FAST;
      if (ptr_ps_dec->peak_decay_fast_bin[bin] < pow_arr[k][bin])
        ptr_ps_dec->peak_decay_fast_bin[bin] = pow_arr[k][bin];

      peak_diff = ptr_ps_dec->prev_peak_diff_bin[bin];
      peak_diff += INIT_FILT_COEFF * (ptr_ps_dec->peak_decay_fast_bin[bin] -
                                      pow_arr[k][bin] -
                                      ptr_ps_dec->prev_peak_diff_bin[bin]);
      ptr_ps_dec->prev_peak_diff_bin[bin] = peak_diff;

      nrg = ptr_ps_dec->prev_nrg_bin[bin];
      nrg += INIT_FILT_COEFF * (pow_arr[k][bin] - ptr_ps_dec->prev_nrg_bin[bin]);
      ptr_ps_dec->prev_nrg_bin[bin] = nrg;
      if (q * peak_diff <= nrg) {
        trans_ratio_arr[k][bin] = 1.0f;
      } else {
        trans_ratio_arr[k][bin] = nrg / (q * peak_diff);
      }
    }
  }

  if (ptr_ps_dec->use_34_st_bands) {
    decay_cutoff = DECAY_CUTOFF_HI_RES;
  } else {
    decay_cutoff = DECAY_CUTOFF;
  }

  for (gr = 0; gr < ptr_ps_dec->num_sub_qmf_groups; gr++) {
    maxsb = ptr_ps_dec->ptr_group_borders[gr] + 1;

    for (sb = ptr_ps_dec->ptr_group_borders[gr]; sb < maxsb; sb++) {
      FLOAT32 decay_scale_factor;

      decay_scale_factor = 1.0f;

      decay_scale_factor = max(decay_scale_factor, 0.0f);

      l_delay = ptr_ps_dec->delay_buf_idx;
      for (k = 0; k < NUM_SER_AP_LINKS; k++)
        l_ser_delay_arr[k] = ptr_ps_dec->delay_buf_idx_ser[k];

      for (k = ptr_ps_dec->border_position[0];
           k < ptr_ps_dec->border_position[ptr_ps_dec->num_env]; k++) {
        FLOAT32 real, imag, real0, imag0, r_r0, i_r0;

        im_left = pp_hyb_left_re[k][sb];
        re_left = pp_hyb_left_im[k][sb];

        {
          real0 = pp_sub_qmf_delay_buf_re[l_delay][sb];
          imag0 = pp_sub_qmf_delay_buf_im[l_delay][sb];
          pp_sub_qmf_delay_buf_re[l_delay][sb] = im_left;
          pp_sub_qmf_delay_buf_im[l_delay][sb] = re_left;

          real = real0 * pp_frac_delay_phase_fac_re[sb] - imag0 *
                 pp_frac_delay_phase_fac_im[sb];
          imag = real0 * pp_frac_delay_phase_fac_im[sb] + imag0 *
                 pp_frac_delay_phase_fac_re[sb];

          r_r0 = real;
          i_r0 = imag;
          for (m = 0; m < NUM_SER_AP_LINKS; m++) {
            real0 = ppp_ser_sub_qmf_dealy_buf_re[m][l_ser_delay_arr[m]][sb];
            imag0 = ppp_ser_sub_qmf_dealy_buf_im[m][l_ser_delay_arr[m]][sb];
            real = real0 * pp_frac_delay_phase_fac_ser_re[sb][m] -
                   imag0 * pp_frac_delay_phase_fac_ser_im[sb][m];
            imag = real0 * pp_frac_delay_phase_fac_ser_im[sb][m] +
                   imag0 * pp_frac_delay_phase_fac_ser_re[sb][m];

            real += -decay_scale_factor * ptr_ps_tables->all_pass_link_decay_ser[m] * r_r0;
            imag += -decay_scale_factor * ptr_ps_tables->all_pass_link_decay_ser[m] * i_r0;
            ppp_ser_sub_qmf_dealy_buf_re[m][l_ser_delay_arr[m]][sb] =
                r_r0 + decay_scale_factor * ptr_ps_tables->all_pass_link_decay_ser[m] * real;
            ppp_ser_sub_qmf_dealy_buf_im[m][l_ser_delay_arr[m]][sb] =
                i_r0 + decay_scale_factor * ptr_ps_tables->all_pass_link_decay_ser[m] * imag;
            r_r0 = real;
            i_r0 = imag;
          }
        }

        bin = (~NEGATE_IPD_MASK) & ptr_ps_dec->ptr_bins_group_map[gr];
        trans_ratio = trans_ratio_arr[k][bin];

        pp_hyb_right_re[k][sb] = trans_ratio * r_r0;
        pp_hyb_right_im[k][sb] = trans_ratio * i_r0;

        if (++l_delay >= DEL_ALL_PASS) l_delay = 0;

        for (m = 0; m < NUM_SER_AP_LINKS; m++) {
          if (++l_ser_delay_arr[m] >= ptr_ps_dec->delay_sample_ser[m]) {
            l_ser_delay_arr[m] = 0;
          }
        }
      }
    }
  }
  {
    ppp_ser_sub_qmf_dealy_buf_re = ptr_ps_dec->ser_qmf_delay_buf_re;
    ppp_ser_sub_qmf_dealy_buf_im = ptr_ps_dec->ser_qmf_delay_buf_im;

    pp_sub_qmf_delay_buf_re = ptr_ps_dec->qmf_delay_buf_re;
    pp_sub_qmf_delay_buf_im = ptr_ps_dec->qmf_delay_buf_im;

    pp_frac_delay_phase_fac_re = ptr_ps_tables->qmf_fract_delay_phase_factor_re;
    pp_frac_delay_phase_fac_im = ptr_ps_tables->qmf_fract_delay_phase_factor_im;

    pp_frac_delay_phase_fac_ser_re = ptr_ps_tables->qmf_ser_fract_delay_phase_factor_re;
    pp_frac_delay_phase_fac_ser_im = ptr_ps_tables->qmf_ser_fract_delay_phase_factor_im;

    p_delay_qmf_delay_buf_idx = ptr_ps_dec->delay_qmf_delay_buf_idx;
    p_delay_qmf_delay_num_samp = ptr_ps_dec->delay_qmf_delay_num_samp;
  }
  for (; gr < ptr_ps_dec->num_groups; gr++) {
    maxsb = ptr_ps_dec->ptr_group_borders[gr + 1];

    for (sb = ptr_ps_dec->ptr_group_borders[gr]; sb < maxsb; sb++) {
      FLOAT32 decay_scale_factor;
      if (sb <= decay_cutoff)
        decay_scale_factor = 1.0f;
      else
        decay_scale_factor = 1.0f + decay_cutoff * DECAY_SLOPE - DECAY_SLOPE * sb;

      decay_scale_factor = max(decay_scale_factor, 0.0f);

      l_delay = ptr_ps_dec->delay_buf_idx;
      for (k = 0; k < NUM_SER_AP_LINKS; k++)
        l_ser_delay_arr[k] = ptr_ps_dec->delay_buf_idx_ser[k];

      for (k = ptr_ps_dec->border_position[0];
           k < ptr_ps_dec->border_position[ptr_ps_dec->num_env]; k++) {
        FLOAT32 real, imag, real0, imag0, r_r0, i_r0;

        im_left = pp_qmf_buf_re_left[k][sb];
        re_left = pp_qmf_buf_im_left[k][sb];

        if (gr >= ptr_ps_dec->first_delay_gr && sb >= NUM_OF_ALL_PASS_CHNLS) {
          real = pp_sub_qmf_delay_buf_re[p_delay_qmf_delay_buf_idx[sb]][sb];
          imag = pp_sub_qmf_delay_buf_im[p_delay_qmf_delay_buf_idx[sb]][sb];
          r_r0 = real;
          i_r0 = imag;
          pp_sub_qmf_delay_buf_re[p_delay_qmf_delay_buf_idx[sb]][sb] = im_left;
          pp_sub_qmf_delay_buf_im[p_delay_qmf_delay_buf_idx[sb]][sb] = re_left;
        } else {
          real0 = pp_sub_qmf_delay_buf_re[l_delay][sb];
          imag0 = pp_sub_qmf_delay_buf_im[l_delay][sb];
          pp_sub_qmf_delay_buf_re[l_delay][sb] = im_left;
          pp_sub_qmf_delay_buf_im[l_delay][sb] = re_left;

          real = real0 * pp_frac_delay_phase_fac_re[sb] - imag0 *
                 pp_frac_delay_phase_fac_im[sb];
          imag = real0 * pp_frac_delay_phase_fac_im[sb] + imag0 *
                 pp_frac_delay_phase_fac_re[sb];

          r_r0 = real;
          i_r0 = imag;
          for (m = 0; m < NUM_SER_AP_LINKS; m++) {
            real0 = ppp_ser_sub_qmf_dealy_buf_re[m][l_ser_delay_arr[m]][sb];
            imag0 = ppp_ser_sub_qmf_dealy_buf_im[m][l_ser_delay_arr[m]][sb];
            real = real0 * pp_frac_delay_phase_fac_ser_re[sb][m] -
                   imag0 * pp_frac_delay_phase_fac_ser_im[sb][m];
            imag = real0 * pp_frac_delay_phase_fac_ser_im[sb][m] +
                   imag0 * pp_frac_delay_phase_fac_ser_re[sb][m];

            real += -decay_scale_factor * ptr_ps_tables->all_pass_link_decay_ser[m] * r_r0;
            imag += -decay_scale_factor * ptr_ps_tables->all_pass_link_decay_ser[m] * i_r0;
            ppp_ser_sub_qmf_dealy_buf_re[m][l_ser_delay_arr[m]][sb] =
                r_r0 + decay_scale_factor * ptr_ps_tables->all_pass_link_decay_ser[m] * real;
            ppp_ser_sub_qmf_dealy_buf_im[m][l_ser_delay_arr[m]][sb] =
                i_r0 + decay_scale_factor * ptr_ps_tables->all_pass_link_decay_ser[m] * imag;
            r_r0 = real;
            i_r0 = imag;
          }
        }

        bin = (~NEGATE_IPD_MASK) & ptr_ps_dec->ptr_bins_group_map[gr];
        trans_ratio = trans_ratio_arr[k][bin];

        pp_qmf_buf_re_right[k][sb] = trans_ratio * r_r0;
        pp_qmf_buf_im_right[k][sb] = trans_ratio * i_r0;

        if (++l_delay >= DEL_ALL_PASS) l_delay = 0;

        if (gr >= ptr_ps_dec->first_delay_gr && sb >= NUM_OF_ALL_PASS_CHNLS) {
          if (++p_delay_qmf_delay_buf_idx[sb] >= p_delay_qmf_delay_num_samp[sb]) {
            p_delay_qmf_delay_buf_idx[sb] = 0;
          }
        }

        for (m = 0; m < NUM_SER_AP_LINKS; m++) {
          if (++l_ser_delay_arr[m] >= ptr_ps_dec->delay_sample_ser[m]) {
            l_ser_delay_arr[m] = 0;
          }
        }
      }
    }
  }

  ptr_ps_dec->delay_buf_idx = l_delay;
  for (m = 0; m < NUM_SER_AP_LINKS; m++) {
    ptr_ps_dec->delay_buf_idx_ser[m] = l_ser_delay_arr[m];
  }
}

VOID ixheaacd_esbr_ps_apply_rotation(
    ia_ps_dec_struct *ptr_ps_dec,
    FLOAT32 **pp_qmf_buf_re_left,
    FLOAT32 **pp_qmf_buf_im_left,
    FLOAT32 **pp_qmf_buf_re_right,
    FLOAT32 **pp_qmf_buf_im_right,
    ia_ps_tables_struct *ptr_ps_tables) {
  WORD32 i;
  WORD32 group;
  WORD32 bin = 0;
  WORD32 subband, max_subband;
  WORD32 env;
  FLOAT32(*p_hyb_left_re)[MAX_NUM_COLUMNS];
  FLOAT32(*p_hyb_left_im)[MAX_NUM_COLUMNS];
  FLOAT32(*p_hyb_rigth_re)[MAX_NUM_COLUMNS];
  FLOAT32(*p_hyb_rigth_im)[MAX_NUM_COLUMNS];
  FLOAT32 scale_fac_l, scale_fac_r;
  FLOAT32 alpha, beta;
  FLOAT32 ipd, opd;
  FLOAT32 ipd1, opd1;
  FLOAT32 ipd2, opd2;

  FLOAT32 h11r, h12r, h21r, h22r;
  FLOAT32 h11i, h12i, h21i, h22i;
  FLOAT32 H11r, H12r, H21r, H22r;
  FLOAT32 H11i, H12i, H21i, H22i;
  FLOAT32 deltaH11r, deltaH12r, deltaH21r, deltaH22r;
  FLOAT32 deltaH11i, deltaH12i, deltaH21i, deltaH22i;
  FLOAT32 l_left_re, l_left_im;
  FLOAT32 l_right_re, l_right_im;

  WORD32 L;
  FLOAT32 *ptr_scale_factors;
  const WORD32 *ptr_quantized_iids;
  WORD32 num_iid_steps;

  if (ptr_ps_dec->iid_quant) {
    num_iid_steps = NUM_IID_STEPS_FINE;
    ptr_scale_factors = ptr_ps_tables->scale_factors_fine_flt;
    ptr_quantized_iids = ptr_ps_tables->quantized_iids_fine;
  } else {
    num_iid_steps = NUM_IID_STEPS;
    ptr_scale_factors = ptr_ps_tables->scale_factors_flt;
    ptr_quantized_iids = ptr_ps_tables->quantized_iids;
  }

  if (ptr_ps_dec->use_34_st_bands != ptr_ps_dec->use_34_st_bands_prev) {
    if (ptr_ps_dec->use_34_st_bands) {
      ixheaacd_map_20_float_to_34(ptr_ps_dec->h11_re_prev);
      ixheaacd_map_20_float_to_34(ptr_ps_dec->h12_re_prev);
      ixheaacd_map_20_float_to_34(ptr_ps_dec->h21_re_prev);
      ixheaacd_map_20_float_to_34(ptr_ps_dec->h22_re_prev);

      ixheaacd_map_20_float_to_34(ptr_ps_dec->h11_im_prev);
      ixheaacd_map_20_float_to_34(ptr_ps_dec->h12_im_prev);
      ixheaacd_map_20_float_to_34(ptr_ps_dec->h21_im_prev);
      ixheaacd_map_20_float_to_34(ptr_ps_dec->h22_im_prev);

      memset(ptr_ps_dec->ipd_idx_map_1, 0, sizeof(ptr_ps_dec->ipd_idx_map_1));
      memset(ptr_ps_dec->opd_idx_map_1, 0, sizeof(ptr_ps_dec->opd_idx_map_1));
      memset(ptr_ps_dec->ipd_idx_map_2, 0, sizeof(ptr_ps_dec->ipd_idx_map_2));
      memset(ptr_ps_dec->opd_idx_map_2, 0, sizeof(ptr_ps_dec->opd_idx_map_2));
    } else {
      ixheaacd_map_34_float_to_20(ptr_ps_dec->h11_re_prev);
      ixheaacd_map_34_float_to_20(ptr_ps_dec->h12_re_prev);
      ixheaacd_map_34_float_to_20(ptr_ps_dec->h21_re_prev);
      ixheaacd_map_34_float_to_20(ptr_ps_dec->h22_re_prev);

      ixheaacd_map_34_float_to_20(ptr_ps_dec->h11_im_prev);
      ixheaacd_map_34_float_to_20(ptr_ps_dec->h12_im_prev);
      ixheaacd_map_34_float_to_20(ptr_ps_dec->h21_im_prev);
      ixheaacd_map_34_float_to_20(ptr_ps_dec->h22_im_prev);

      memset(ptr_ps_dec->ipd_idx_map_1, 0, sizeof(ptr_ps_dec->ipd_idx_map_1));
      memset(ptr_ps_dec->opd_idx_map_1, 0, sizeof(ptr_ps_dec->opd_idx_map_1));
      memset(ptr_ps_dec->ipd_idx_map_2, 0, sizeof(ptr_ps_dec->ipd_idx_map_2));
      memset(ptr_ps_dec->opd_idx_map_2, 0, sizeof(ptr_ps_dec->opd_idx_map_2));
    }
  }

  for (env = 0; env < ptr_ps_dec->num_env; env++) {
    for (bin = 0; bin < ptr_ps_dec->num_bins; bin++) {
      if (!ptr_ps_dec->use_pca_rot_flg) {
        scale_fac_r = ptr_scale_factors[num_iid_steps +
                      ptr_ps_dec->iid_par_table[env][bin]];
        scale_fac_l = ptr_scale_factors[num_iid_steps -
                      ptr_ps_dec->iid_par_table[env][bin]];

        alpha = ptr_ps_tables->alphas[ptr_ps_dec->icc_par_table[env][bin]];

        beta = alpha * (scale_fac_r - scale_fac_l) / PSC_SQRT2F;

        h11r = (FLOAT32)(scale_fac_l * cos(beta + alpha));
        h12r = (FLOAT32)(scale_fac_r * cos(beta - alpha));
        h21r = (FLOAT32)(scale_fac_l * sin(beta + alpha));
        h22r = (FLOAT32)(scale_fac_r * sin(beta - alpha));
      } else {
        FLOAT32 c, rho, mu, alpha, gamma;
        WORD32 i;

        i = ptr_ps_dec->iid_par_table[env][bin];
        c = (FLOAT32)pow(
            10.0,
            ((i) ? (((i > 0) ? 1 : -1) * ptr_quantized_iids[((i > 0) ? i : -i) - 1]) : 0.) /
                20.0);
        rho = ptr_ps_tables->quantized_rhos[ptr_ps_dec->icc_par_table[env][bin]];
        rho = max(rho, 0.05f);

        if (rho == 0.0f && c == 1.) {
          alpha = (FLOAT32)PI / 4.0f;
        } else {
          if (rho <= 0.05f) {
            rho = 0.05f;
          }
          alpha = 0.5f * (FLOAT32)atan((2.0f * c * rho) / (c * c - 1.0f));

          if (alpha < 0.) {
            alpha += (FLOAT32)PI / 2.0f;
          }
        }
        mu = c + 1.0f / c;
        mu = 1 + (4.0f * rho * rho - 4.0f) / (mu * mu);
        gamma = (FLOAT32)atan(sqrt((1.0f - sqrt(mu)) / (1.0f + sqrt(mu))));

        h11r = (FLOAT32)(sqrt(2.) * cos(alpha) * cos(gamma));
        h12r = (FLOAT32)(sqrt(2.) * sin(alpha) * cos(gamma));
        h21r = (FLOAT32)(sqrt(2.) * -sin(alpha) * sin(gamma));
        h22r = (FLOAT32)(sqrt(2.) * cos(alpha) * sin(gamma));
      }

      if (bin >= ptr_ps_tables->ipd_bins_tbl[ptr_ps_dec->freq_res_ipd]) {
        h11i = h12i = h21i = h22i = 0.0f;
      } else {
        ipd = (IPD_SCALE_FACTOR * 2.0f) * ptr_ps_dec->ipd_idx_map[env][bin];
        opd = (OPD_SCALE_FACTOR * 2.0f) * ptr_ps_dec->opd_idx_map[env][bin];
        ipd1 = (IPD_SCALE_FACTOR * 2.0f) * ptr_ps_dec->ipd_idx_map_1[bin];
        opd1 = (OPD_SCALE_FACTOR * 2.0f) * ptr_ps_dec->opd_idx_map_1[bin];
        ipd2 = (IPD_SCALE_FACTOR * 2.0f) * ptr_ps_dec->ipd_idx_map_2[bin];
        opd2 = (OPD_SCALE_FACTOR * 2.0f) * ptr_ps_dec->opd_idx_map_2[bin];

        l_left_re = (FLOAT32)cos(ipd);
        l_left_im = (FLOAT32)sin(ipd);
        l_right_re = (FLOAT32)cos(opd);
        l_right_im = (FLOAT32)sin(opd);

        l_left_re += PHASE_SMOOTH_HIST1 * (FLOAT32)cos(ipd1);
        l_left_im += PHASE_SMOOTH_HIST1 * (FLOAT32)sin(ipd1);
        l_right_re += PHASE_SMOOTH_HIST1 * (FLOAT32)cos(opd1);
        l_right_im += PHASE_SMOOTH_HIST1 * (FLOAT32)sin(opd1);

        l_left_re += PHASE_SMOOTH_HIST2 * (FLOAT32)cos(ipd2);
        l_left_im += PHASE_SMOOTH_HIST2 * (FLOAT32)sin(ipd2);
        l_right_re += PHASE_SMOOTH_HIST2 * (FLOAT32)cos(opd2);
        l_right_im += PHASE_SMOOTH_HIST2 * (FLOAT32)sin(opd2);

        ipd = (FLOAT32)atan2(l_left_im, l_left_re);
        opd = (FLOAT32)atan2(l_right_im, l_right_re);

        l_left_re = (FLOAT32)cos(opd);
        l_left_im = (FLOAT32)sin(opd);
        opd -= ipd;
        l_right_re = (FLOAT32)cos(opd);
        l_right_im = (FLOAT32)sin(opd);

        h11i = h11r * l_left_im;
        h12i = h12r * l_right_im;
        h21i = h21r * l_left_im;
        h22i = h22r * l_right_im;

        h11r *= l_left_re;
        h12r *= l_right_re;
        h21r *= l_left_re;
        h22r *= l_right_re;
      }

      ptr_ps_dec->h11_re_vec[bin] = h11r;
      ptr_ps_dec->h12_re_vec[bin] = h12r;
      ptr_ps_dec->h21_re_vec[bin] = h21r;
      ptr_ps_dec->h22_re_vec[bin] = h22r;
      ptr_ps_dec->h11_im_vec[bin] = h11i;
      ptr_ps_dec->h12_im_vec[bin] = h12i;
      ptr_ps_dec->h21_im_vec[bin] = h21i;
      ptr_ps_dec->h22_im_vec[bin] = h22i;

    }

    p_hyb_left_re = ptr_ps_dec->hyb_left_re;
    p_hyb_left_im = ptr_ps_dec->hyb_left_im;
    p_hyb_rigth_re = ptr_ps_dec->hyb_right_re;
    p_hyb_rigth_im = ptr_ps_dec->hyb_right_im;

    for (group = 0; group < ptr_ps_dec->num_sub_qmf_groups; group++) {
      bin = (~NEGATE_IPD_MASK) & ptr_ps_dec->ptr_bins_group_map[group];

      max_subband = ptr_ps_dec->ptr_group_borders[group] + 1;

      L = ptr_ps_dec->border_position[env + 1] - ptr_ps_dec->border_position[env];

      H11r = ptr_ps_dec->h11_re_prev[bin];
      H12r = ptr_ps_dec->h12_re_prev[bin];
      H21r = ptr_ps_dec->h21_re_prev[bin];
      H22r = ptr_ps_dec->h22_re_prev[bin];
      if ((NEGATE_IPD_MASK & ptr_ps_dec->ptr_bins_group_map[group]) != 0) {
        H11i = -ptr_ps_dec->h11_im_prev[bin];
        H12i = -ptr_ps_dec->h12_im_prev[bin];
        H21i = -ptr_ps_dec->h21_im_prev[bin];
        H22i = -ptr_ps_dec->h22_im_prev[bin];
      } else {
        H11i = ptr_ps_dec->h11_im_prev[bin];
        H12i = ptr_ps_dec->h12_im_prev[bin];
        H21i = ptr_ps_dec->h21_im_prev[bin];
        H22i = ptr_ps_dec->h22_im_prev[bin];
      }

      h11r = ptr_ps_dec->h11_re_vec[bin];
      h12r = ptr_ps_dec->h12_re_vec[bin];
      h21r = ptr_ps_dec->h21_re_vec[bin];
      h22r = ptr_ps_dec->h22_re_vec[bin];
      if ((NEGATE_IPD_MASK & ptr_ps_dec->ptr_bins_group_map[group]) != 0) {
        h11i = -ptr_ps_dec->h11_im_vec[bin];
        h12i = -ptr_ps_dec->h12_im_vec[bin];
        h21i = -ptr_ps_dec->h21_im_vec[bin];
        h22i = -ptr_ps_dec->h22_im_vec[bin];
      } else {
        h11i = ptr_ps_dec->h11_im_vec[bin];
        h12i = ptr_ps_dec->h12_im_vec[bin];
        h21i = ptr_ps_dec->h21_im_vec[bin];
        h22i = ptr_ps_dec->h22_im_vec[bin];
      }

      deltaH11r = (h11r - H11r) / L;
      deltaH12r = (h12r - H12r) / L;
      deltaH21r = (h21r - H21r) / L;
      deltaH22r = (h22r - H22r) / L;

      deltaH11i = (h11i - H11i) / L;
      deltaH12i = (h12i - H12i) / L;
      deltaH21i = (h21i - H21i) / L;
      deltaH22i = (h22i - H22i) / L;

      for (i = ptr_ps_dec->border_position[env]; i < ptr_ps_dec->border_position[env + 1];
           i++) {
        H11r += deltaH11r;
        H12r += deltaH12r;
        H21r += deltaH21r;
        H22r += deltaH22r;

        H11i += deltaH11i;
        H12i += deltaH12i;
        H21i += deltaH21i;
        H22i += deltaH22i;

        for (subband = ptr_ps_dec->ptr_group_borders[group]; subband < max_subband;
             subband++) {
          l_left_re = H11r * p_hyb_left_re[i][subband] - H11i * p_hyb_left_im[i][subband] +
                      H21r * p_hyb_rigth_re[i][subband] - H21i * p_hyb_rigth_im[i][subband];

          l_left_im = H11i * p_hyb_left_re[i][subband] + H11r * p_hyb_left_im[i][subband] +
                      H21i * p_hyb_rigth_re[i][subband] + H21r * p_hyb_rigth_im[i][subband];

          l_right_re = H12r * p_hyb_left_re[i][subband] - H12i * p_hyb_left_im[i][subband] +
                       H22r * p_hyb_rigth_re[i][subband] - H22i * p_hyb_rigth_im[i][subband];

          l_right_im = H12i * p_hyb_left_re[i][subband] + H12r * p_hyb_left_im[i][subband] +
                       H22i * p_hyb_rigth_re[i][subband] + H22r * p_hyb_rigth_im[i][subband];

          p_hyb_left_re[i][subband] = l_left_re;
          p_hyb_left_im[i][subband] = l_left_im;
          p_hyb_rigth_re[i][subband] = l_right_re;
          p_hyb_rigth_im[i][subband] = l_right_im;
        }
      }
    }

    for (; group < ptr_ps_dec->num_groups; group++) {
      bin = (~NEGATE_IPD_MASK) & ptr_ps_dec->ptr_bins_group_map[group];

      max_subband = ptr_ps_dec->ptr_group_borders[group + 1];

      L = ptr_ps_dec->border_position[env + 1] - ptr_ps_dec->border_position[env];

      H11r = ptr_ps_dec->h11_re_prev[bin];
      H12r = ptr_ps_dec->h12_re_prev[bin];
      H21r = ptr_ps_dec->h21_re_prev[bin];
      H22r = ptr_ps_dec->h22_re_prev[bin];
      if ((NEGATE_IPD_MASK & ptr_ps_dec->ptr_bins_group_map[group]) != 0) {
        H11i = -ptr_ps_dec->h11_im_prev[bin];
        H12i = -ptr_ps_dec->h12_im_prev[bin];
        H21i = -ptr_ps_dec->h21_im_prev[bin];
        H22i = -ptr_ps_dec->h22_im_prev[bin];
      } else {
        H11i = ptr_ps_dec->h11_im_prev[bin];
        H12i = ptr_ps_dec->h12_im_prev[bin];
        H21i = ptr_ps_dec->h21_im_prev[bin];
        H22i = ptr_ps_dec->h22_im_prev[bin];
      }

      h11r = ptr_ps_dec->h11_re_vec[bin];
      h12r = ptr_ps_dec->h12_re_vec[bin];
      h21r = ptr_ps_dec->h21_re_vec[bin];
      h22r = ptr_ps_dec->h22_re_vec[bin];
      if ((NEGATE_IPD_MASK & ptr_ps_dec->ptr_bins_group_map[group]) != 0) {
        h11i = -ptr_ps_dec->h11_im_vec[bin];
        h12i = -ptr_ps_dec->h12_im_vec[bin];
        h21i = -ptr_ps_dec->h21_im_vec[bin];
        h22i = -ptr_ps_dec->h22_im_vec[bin];
      } else {
        h11i = ptr_ps_dec->h11_im_vec[bin];
        h12i = ptr_ps_dec->h12_im_vec[bin];
        h21i = ptr_ps_dec->h21_im_vec[bin];
        h22i = ptr_ps_dec->h22_im_vec[bin];
      }

      deltaH11r = (h11r - H11r) / L;
      deltaH12r = (h12r - H12r) / L;
      deltaH21r = (h21r - H21r) / L;
      deltaH22r = (h22r - H22r) / L;

      deltaH11i = (h11i - H11i) / L;
      deltaH12i = (h12i - H12i) / L;
      deltaH21i = (h21i - H21i) / L;
      deltaH22i = (h22i - H22i) / L;

      for (i = ptr_ps_dec->border_position[env]; i < ptr_ps_dec->border_position[env + 1];
           i++) {
        H11r += deltaH11r;
        H12r += deltaH12r;
        H21r += deltaH21r;
        H22r += deltaH22r;

        H11i += deltaH11i;
        H12i += deltaH12i;
        H21i += deltaH21i;
        H22i += deltaH22i;

        for (subband = ptr_ps_dec->ptr_group_borders[group]; subband < max_subband;
             subband++) {
          l_left_re = H11r * pp_qmf_buf_re_left[i][subband] - H11i *
                      pp_qmf_buf_im_left[i][subband] +
                      H21r * pp_qmf_buf_re_right[i][subband] - H21i *
                      pp_qmf_buf_im_right[i][subband];

          l_left_im = H11i * pp_qmf_buf_re_left[i][subband] + H11r *
                      pp_qmf_buf_im_left[i][subband] +
                      H21i * pp_qmf_buf_re_right[i][subband] + H21r *
                      pp_qmf_buf_im_right[i][subband];

          l_right_re = H12r * pp_qmf_buf_re_left[i][subband] - H12i *
                       pp_qmf_buf_im_left[i][subband] +
                       H22r * pp_qmf_buf_re_right[i][subband] - H22i *
                       pp_qmf_buf_im_right[i][subband];

          l_right_im = H12i * pp_qmf_buf_re_left[i][subband] + H12r *
                       pp_qmf_buf_im_left[i][subband] +
                       H22i * pp_qmf_buf_re_right[i][subband] + H22r *
                       pp_qmf_buf_im_right[i][subband];

          pp_qmf_buf_re_left[i][subband] = l_left_re;
          pp_qmf_buf_im_left[i][subband] = l_left_im;
          pp_qmf_buf_re_right[i][subband] = l_right_re;
          pp_qmf_buf_im_right[i][subband] = l_right_im;
        }
      }
    }
    for (bin = 0; bin < ptr_ps_dec->num_bins; bin++) {
      ptr_ps_dec->h11_re_prev[bin] = ptr_ps_dec->h11_re_vec[bin];
      ptr_ps_dec->h12_re_prev[bin] = ptr_ps_dec->h12_re_vec[bin];
      ptr_ps_dec->h21_re_prev[bin] = ptr_ps_dec->h21_re_vec[bin];
      ptr_ps_dec->h22_re_prev[bin] = ptr_ps_dec->h22_re_vec[bin];

      ptr_ps_dec->h11_im_prev[bin] = ptr_ps_dec->h11_im_vec[bin];
      ptr_ps_dec->h12_im_prev[bin] = ptr_ps_dec->h12_im_vec[bin];
      ptr_ps_dec->h21_im_prev[bin] = ptr_ps_dec->h21_im_vec[bin];
      ptr_ps_dec->h22_im_prev[bin] = ptr_ps_dec->h22_im_vec[bin];
    }

    for (bin = 0; bin < ptr_ps_tables->ipd_bins_tbl[ptr_ps_dec->freq_res_ipd]; bin++) {
      ptr_ps_dec->ipd_idx_map_2[bin] = ptr_ps_dec->ipd_idx_map_1[bin];
      ptr_ps_dec->opd_idx_map_2[bin] = ptr_ps_dec->opd_idx_map_1[bin];
      ptr_ps_dec->ipd_idx_map_1[bin] = ptr_ps_dec->ipd_idx_map[env][bin];
      ptr_ps_dec->opd_idx_map_1[bin] = ptr_ps_dec->opd_idx_map[env][bin];
    }
  }
}
