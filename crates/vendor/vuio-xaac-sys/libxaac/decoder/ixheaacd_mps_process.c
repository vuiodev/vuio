/******************************************************************************
 *
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
#include "ixheaacd_memory_standards.h"
#include "ixheaacd_mps_struct_def.h"
#include "ixheaacd_mps_res_rom.h"
#include "ixheaacd_mps_aac_struct.h"
#include "ixheaac_constants.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaacd_bitbuffer.h"
#include "ixheaacd_common_rom.h"
#include "ixheaacd_sbrdecsettings.h"
#include "ixheaacd_sbr_scale.h"
#include "ixheaacd_env_extr_part.h"
#include "ixheaacd_sbr_rom.h"
#include "ixheaacd_hybrid.h"
#include "ixheaacd_ps_dec.h"
#include "ixheaacd_mps_polyphase.h"
#include "ixheaacd_config.h"
#include "ixheaacd_qmf_dec.h"
#include "ixheaacd_mps_dec.h"
#include "ixheaacd_mps_decor.h"
#include "ixheaacd_mps_hybfilter.h"
#include "ixheaacd_mps_mdct_2_qmf.h"
#include "ixheaacd_mps_get_index.h"
#include "ixheaacd_mps_macro_def.h"
#include "ixheaacd_mps_basic_op.h"
#include "ixheaac_error_standards.h"

VOID ixheaacd_mdct_2_qmf(ia_heaac_mps_state_struct *pstr_mps_state) {
  ia_mps_dec_reuse_array_struct *p_array_struct = pstr_mps_state->array_struct;
  WORD32 ch, rfpsf, qb;
  WORD32 qmf_global_offset;
  WORD32 time_slots = pstr_mps_state->time_slots;
  WORD32 time_slots_x4 = (time_slots << 2);
  WORD32 qmf_bands = pstr_mps_state->qmf_bands;
  WORD32 *p_qmf_residual_real_post, *p_qmf_residual_imag_post;
  VOID *scratch = pstr_mps_state->mps_scratch_mem_v;

  if (pstr_mps_state->up_mix_type != 2) {
    WORD32 num_ch = pstr_mps_state->num_ott_boxes + pstr_mps_state->num_ttt_boxes;
    WORD32 rfpsf_max = pstr_mps_state->residual_frames_per_spatial_frame;
    WORD32 upd_qmf = pstr_mps_state->upd_qmf;

    WORD32 *qmf_residual_real_pre = p_array_struct->qmf_residual_real_pre;
    WORD32 *qmf_residual_real_post = p_array_struct->qmf_residual_real_post;

    WORD32 *qmf_residual_imag_pre = p_array_struct->qmf_residual_imag_pre;
    WORD32 *qmf_residual_imag_post = p_array_struct->qmf_residual_imag_post;

    WORD32 *p_res_mdct = p_array_struct->res_mdct;

    for (ch = 0; ch < num_ch; ch++) {
      if (pstr_mps_state->bs_config.bs_residual_present[ch]) {
        WORD32 *res_mdct = p_res_mdct;
        qmf_global_offset = 0;

        p_qmf_residual_real_post = qmf_residual_real_post;
        p_qmf_residual_imag_post = qmf_residual_imag_post;
        for (qb = 0; qb < qmf_bands; qb++) {
          memset(p_qmf_residual_real_post, 0, time_slots_x4);
          memset(p_qmf_residual_imag_post, 0, time_slots_x4);

          p_qmf_residual_real_post += MAX_TIME_SLOTS;
          p_qmf_residual_imag_post += MAX_TIME_SLOTS;
        }

        for (rfpsf = 0; rfpsf < rfpsf_max; rfpsf++) {
          ixheaacd_mdct2qmf_process(upd_qmf, res_mdct, qmf_residual_real_pre,
                                    qmf_residual_real_post, qmf_residual_imag_pre,
                                    qmf_residual_imag_post,
                                    pstr_mps_state->res_block_type[ch][rfpsf], qmf_global_offset,
                                    &(pstr_mps_state->ia_mps_dec_mps_table), scratch, time_slots);
          qmf_global_offset += upd_qmf;
          res_mdct += MDCTCOEFX2;
        }
      }

      qmf_residual_real_pre += QBXTS;
      qmf_residual_imag_pre += QBXTS;

      qmf_residual_real_post += QBXTS;
      qmf_residual_imag_post += QBXTS;

      p_res_mdct += RFX2XMDCTCOEF;
    }
  }

  if (pstr_mps_state->arbitrary_downmix == 2) {
    WORD32 arbdmx_upd_qmf = pstr_mps_state->arbdmx_upd_qmf;
    WORD32 offset = pstr_mps_state->num_ott_boxes + pstr_mps_state->num_ttt_boxes;
    WORD32 in_ch = pstr_mps_state->num_input_channels;
    WORD32 rfpsf_max = pstr_mps_state->arbdmx_frames_per_spatial_frame;

    WORD32 *qmf_residual_real_pre = p_array_struct->qmf_residual_real_pre + offset * QBXTS;
    WORD32 *qmf_residual_imag_pre = p_array_struct->qmf_residual_imag_pre + offset * QBXTS;

    WORD32 *qmf_residual_real_post = p_array_struct->qmf_residual_real_post + offset * QBXTS;
    WORD32 *qmf_residual_imag_post = p_array_struct->qmf_residual_imag_post + offset * QBXTS;

    WORD32 *p_res_mdct = p_array_struct->res_mdct + offset * RFX2XMDCTCOEF;

    for (ch = 0; ch < in_ch; ch++) {
      WORD32 *res_mdct = p_res_mdct;
      qmf_global_offset = 0;

      p_qmf_residual_real_post = qmf_residual_real_post;
      p_qmf_residual_imag_post = qmf_residual_imag_post;
      for (qb = 0; qb < qmf_bands; qb++) {
        memset(p_qmf_residual_real_post, 0, time_slots_x4);
        memset(p_qmf_residual_imag_post, 0, time_slots_x4);

        p_qmf_residual_real_post += MAX_TIME_SLOTS;
        p_qmf_residual_imag_post += MAX_TIME_SLOTS;
      }

      for (rfpsf = 0; rfpsf < rfpsf_max; rfpsf++) {
        ixheaacd_mdct2qmf_process(
            arbdmx_upd_qmf, res_mdct, qmf_residual_real_pre, qmf_residual_real_post,
            qmf_residual_imag_pre, qmf_residual_imag_post,
            pstr_mps_state->res_block_type[offset + ch][rfpsf], qmf_global_offset,
            &(pstr_mps_state->ia_mps_dec_mps_table), scratch, time_slots);
        qmf_global_offset += arbdmx_upd_qmf;
        res_mdct += MDCTCOEFX2;
      }

      qmf_residual_real_pre += QBXTS;
      qmf_residual_imag_pre += QBXTS;

      qmf_residual_imag_post += QBXTS;
      qmf_residual_real_post += QBXTS;

      p_res_mdct += RFX2XMDCTCOEF;
    }
  }
  return;
}

VOID ixheaacd_hybrid_qmf_analysis(ia_heaac_mps_state_struct *pstr_mps_state) {
  WORD32 ch;
  WORD32 in_ch = pstr_mps_state->num_input_channels;
  WORD32 num_ott_boxes = pstr_mps_state->num_ott_boxes;
  WORD32 num_ttt_boxes = pstr_mps_state->num_ttt_boxes;
  WORD32 num_input_channels = in_ch;
  WORD32 qmf_bands = pstr_mps_state->qmf_bands;
  WORD32 time_slots = pstr_mps_state->time_slots;
  WORD32 hybrid_bands = pstr_mps_state->hybrid_bands;
  WORD32 num_parameter_bands = pstr_mps_state->num_parameter_bands;
  SIZE_T *kernels = pstr_mps_state->kernels;
  WORD32 *res_bands = pstr_mps_state->res_bands;
  WORD32 *index = pstr_mps_state->index;

  ia_mps_dec_thyb_filter_state_struct *hyb_filter_state =
      pstr_mps_state->mps_persistent_mem.hyb_filter_state;
  ia_mps_dec_reuse_array_struct *p_array_struct = pstr_mps_state->array_struct;

  ia_mps_dec_hybrid_tables_struct *hybrid_table_ptr =
      pstr_mps_state->ia_mps_dec_mps_table.hybrid_table_ptr;

  WORD32 *p_buf_real = p_array_struct->buf_real;
  WORD32 *p_buf_imag = p_array_struct->buf_imag;

  WORD32 *p_x_real = p_array_struct->x_real;
  WORD32 *p_x_imag = p_array_struct->x_imag;

  for (ch = 0; ch < in_ch; ch++) {
    ixheaacd_apply_ana_hyb_filt_bank_create_x(&hyb_filter_state[ch], p_buf_real, p_buf_imag,
                                              qmf_bands, time_slots, p_x_real, p_x_imag,
                                              hybrid_table_ptr);
    pstr_mps_state->index[ch] = hybrid_bands;

    p_buf_real += TSXHB;
    p_buf_imag += TSXHB;

    p_x_real += TSXHB;
    p_x_imag += TSXHB;
  }

  if ((pstr_mps_state->residual_coding) && (pstr_mps_state->up_mix_type != 2)) {
    WORD32 *qmf_residual_real = p_array_struct->qmf_residual_real_pre;
    WORD32 *qmf_residual_imag = p_array_struct->qmf_residual_imag_pre;

    WORD32 *p_dry_real = p_array_struct->w_dry_real;
    WORD32 *p_dry_imag = p_array_struct->w_dry_imag;

    for (ch = 0; ch < num_ott_boxes; ch++) {
      if (res_bands[ch] > 0) {
        ixheaacd_apply_ana_hyb_filt_bank_merge_res_decor(
            &hyb_filter_state[ch + num_input_channels], qmf_residual_real, qmf_residual_imag,
            qmf_bands, time_slots, p_dry_real, p_dry_imag, hybrid_table_ptr);
      }
      qmf_residual_real += QBXTS;
      qmf_residual_imag += QBXTS;

      p_dry_real += TSXHB;
      p_dry_imag += TSXHB;
    }

    for (ch = num_ott_boxes; ch < num_ott_boxes + num_ttt_boxes; ch++, in_ch++) {
      if (res_bands[ch] > 0) {
        ixheaacd_apply_ana_hyb_filt_bank_create_x_res(
            &hyb_filter_state[ch + num_input_channels], qmf_residual_real, qmf_residual_imag,
            qmf_bands, time_slots, p_x_real, p_x_imag, kernels, res_bands[ch], hybrid_bands,
            num_parameter_bands, &index[in_ch], hybrid_table_ptr);
      } else
        index[in_ch] = 0;

      qmf_residual_real += QBXTS;
      qmf_residual_imag += QBXTS;

      p_x_real += TSXHB;
      p_x_imag += TSXHB;
    }
  }

  in_ch = num_input_channels + num_ttt_boxes;
  if (pstr_mps_state->arbitrary_downmix == 2) {
    WORD32 offset = num_ott_boxes + num_ttt_boxes;

    WORD32 *qmf_residual_real = p_array_struct->qmf_residual_real_pre + offset * QBXTS;
    WORD32 *qmf_residual_imag = p_array_struct->qmf_residual_imag_pre + offset * QBXTS;

    p_x_real = p_array_struct->x_real + in_ch * TSXHB;
    p_x_imag = p_array_struct->x_imag + in_ch * TSXHB;
    for (ch = 0; ch < num_input_channels; ch++, in_ch++) {
      ixheaacd_apply_ana_hyb_filt_bank_create_x_res(
          &hyb_filter_state[offset + ch + num_input_channels], qmf_residual_real,
          qmf_residual_imag, qmf_bands, time_slots, p_x_real, p_x_imag, kernels,
          pstr_mps_state->arbdmx_residual_bands, hybrid_bands, num_parameter_bands, &index[in_ch],
          hybrid_table_ptr);

      qmf_residual_real += QBXTS;
      qmf_residual_imag += QBXTS;

      p_x_real += TSXHB;
      p_x_imag += TSXHB;
    }
  }
}

ATTR_NO_SANITIZE_INTEGER
VOID ixheaacd_merge_res_decor(ia_heaac_mps_state_struct *pstr_mps_state) {
  WORD32 ts, qs, row, res;

  WORD32 temp_1;
  SIZE_T *idx;

  ia_mps_dec_reuse_array_struct *p_array_struct = pstr_mps_state->array_struct;
  ia_mps_dec_auxilary_struct *p_aux_struct = pstr_mps_state->aux_struct;
  WORD32 time_slots = pstr_mps_state->time_slots;
  WORD32 hybrid_bands = pstr_mps_state->hybrid_bands;
  WORD32 num_direct_signals = pstr_mps_state->num_direct_signals;
  WORD32 num_w_channels = pstr_mps_state->num_w_channels;
  WORD32 num_parameter_bands = pstr_mps_state->num_parameter_bands;
  SIZE_T *kernels_ptr = pstr_mps_state->kernels;

  WORD32 *p_buf_real, *p_buf_imag, *p_buf_re, *p_buf_im;
  WORD32 *buf_real_ch4, *buf_imag_ch4;
  WORD32 *buf_real_ch3, *buf_imag_ch3;

  p_buf_real = p_array_struct->buffer_real + TSXHBX5;
  p_buf_imag = p_array_struct->buffer_imag + TSXHBX5;

  for (ts = 0; ts < time_slots; ts++) {
    p_buf_re = p_buf_real;
    p_buf_im = p_buf_imag;

    buf_real_ch4 = p_buf_real - TSXHB;
    buf_imag_ch4 = p_buf_imag - TSXHB;

    buf_real_ch3 = buf_real_ch4 - TSXHB;
    buf_imag_ch3 = buf_imag_ch4 - TSXHB;

    for (qs = 0; qs < hybrid_bands; qs++) {
      if ((kernels_ptr[qs] < ((UWORD32)(p_aux_struct->ttt_config[0][0].stop_band)) &&
           p_aux_struct->ttt_config[0][0].use_ttt_decorr) ||
          (kernels_ptr[qs] >= ((UWORD32)p_aux_struct->ttt_config[1][0].start_band) &&
           p_aux_struct->ttt_config[1][0].use_ttt_decorr)) {
        temp_1 = (WORD32)ONE_BY_SQRT_TWO_Q30;

        *p_buf_re = ixheaacd_mps_mult32_shr_30(*p_buf_re, temp_1);
        *p_buf_re += (*buf_real_ch3 + *buf_real_ch4);

        *p_buf_im = ixheaacd_mps_mult32_shr_30(*p_buf_im, temp_1);
        *p_buf_im += (*buf_imag_ch3 + *buf_imag_ch4);
      }
      p_buf_re++;
      p_buf_im++;

      buf_real_ch4++;
      buf_imag_ch4++;

      buf_real_ch3++;
      buf_imag_ch3++;
    }
    p_buf_real += MAX_HYBRID_BANDS;
    p_buf_imag += MAX_HYBRID_BANDS;
  }

  if (pstr_mps_state->residual_coding) {
    for (row = num_direct_signals; row < num_w_channels; row++) {
      WORD32 resband;
      res = ixheaacd_get_res_idx(pstr_mps_state, row);
      resband = pstr_mps_state->res_bands[res];

      if (resband == 1 && (num_parameter_bands == 20 || num_parameter_bands == 28))
        pstr_mps_state->index[res] = 3;
      else {
        idx = &kernels_ptr[0];
        for (qs = 0; qs < hybrid_bands; qs++) {
          if (*idx++ >= (SIZE_T)resband) {
            pstr_mps_state->index[res] = qs;
            qs = hybrid_bands;
          }
        }
      }
    }
  }
}

VOID ixheaacd_create_w(ia_heaac_mps_state_struct *pstr_mps_state) {
  WORD32 k;
  ia_mps_dec_reuse_array_struct *p_array_struct = pstr_mps_state->array_struct;
  WORD32 num_direct_signals = pstr_mps_state->num_direct_signals;
  WORD32 counter = num_direct_signals + pstr_mps_state->num_decor_signals;
  WORD32 time_slots = pstr_mps_state->time_slots;
  WORD32 offset = num_direct_signals * TSXHB;
  WORD32 *p_buffer_real = p_array_struct->buf_real + offset;
  WORD32 *p_buffer_imag = p_array_struct->buf_imag + offset;

  WORD32 *p_buf_real = p_array_struct->buffer_real + offset;
  WORD32 *p_buf_imag = p_array_struct->buffer_imag + offset;

  for (k = num_direct_signals; k < counter; k++) {
    ixheaacd_decorr_apply(pstr_mps_state, time_slots, p_buffer_real, p_buffer_imag, p_buf_real,
                          p_buf_imag, k);

    p_buffer_real += TSXHB;
    p_buffer_imag += TSXHB;

    p_buf_real += TSXHB;
    p_buf_imag += TSXHB;
  }
  ixheaacd_merge_res_decor(pstr_mps_state);
}

VOID ixheaacd_update_buffers(ia_heaac_mps_state_struct *pstr_mps_state) {
  ia_mps_dec_reuse_array_struct *p_array_struct = pstr_mps_state->array_struct;
  WORD32 *temp_addr = p_array_struct->qmf_residual_real_post;
  p_array_struct->qmf_residual_real_post = p_array_struct->qmf_residual_real_pre;
  p_array_struct->qmf_residual_real_pre = temp_addr;

  temp_addr = p_array_struct->qmf_residual_imag_post;
  p_array_struct->qmf_residual_imag_post = p_array_struct->qmf_residual_imag_pre;
  p_array_struct->qmf_residual_imag_pre = temp_addr;

  p_array_struct->buffer_real = p_array_struct->qmf_residual_real_post;
  p_array_struct->buffer_imag = p_array_struct->qmf_residual_imag_post;

  p_array_struct->m1_param = (ia_mps_dec_m1_param_struct *)p_array_struct->buffer_real;
}
