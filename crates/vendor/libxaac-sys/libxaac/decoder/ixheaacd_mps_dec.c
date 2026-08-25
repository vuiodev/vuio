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
#include <math.h>
#include <string.h>
#include "ixheaac_type_def.h"
#include "ixheaac_constants.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaacd_bitbuffer.h"
#include "ixheaacd_defines.h"
#include "ixheaac_sbr_const.h"
#include "ixheaacd_memory_standards.h"
#include "ixheaacd_sbrdecsettings.h"
#include "ixheaacd_env_extr_part.h"
#include "ixheaacd_aac_rom.h"
#include "ixheaacd_common_rom.h"
#include "ixheaacd_sbr_rom.h"
#include "ixheaacd_pulsedata.h"
#include "ixheaacd_pns.h"
#include "ixheaacd_sbr_common.h"
#include "ixheaacd_drc_data_struct.h"
#include "ixheaacd_cnst.h"
#include "ixheaacd_ec_defines.h"
#include "ixheaacd_ec_struct_def.h"
#include "ixheaacd_error_codes.h"
#include "ixheaacd_channelinfo.h"
#include "ixheaacd_sbrdecoder.h"
#include "ixheaacd_audioobjtypes.h"
#include "ixheaacd_latmdemux.h"
#include "ixheaacd_aacdec.h"
#include "ixheaacd_hybrid.h"
#include "ixheaacd_sbr_scale.h"
#include "ixheaacd_ps_dec.h"
#include "ixheaacd_lpp_tran.h"
#include "ixheaacd_env_extr.h"
#include "ixheaacd_qmf_dec.h"
#include "ixheaacd_env_calc.h"
#include "ixheaacd_pvc_dec.h"
#include "ixheaacd_sbr_dec.h"
#include "ixheaacd_mps_polyphase.h"
#include "ixheaacd_config.h"
#include "ixheaacd_mps_macro_def.h"
#include "ixheaacd_mps_struct_def.h"
#include "ixheaacd_mps_res_rom.h"
#include "ixheaacd_mps_aac_struct.h"
#include "ixheaacd_mps_dec.h"
#include "ixheaacd_mps_interface.h"
#include "ixheaacd_struct_def.h"
#include "ixheaacd_mps_process.h"
#include "ixheaacd_mps_bitdec.h"
#include "ixheaacd_mps_smoothing.h"
#include "ixheaacd_mps_tp_process.h"
#include "ixheaacd_mps_reshape_bb_env.h"
#include "ixheaacd_mps_blind.h"
#include "ixheaacd_mps_m1m2.h"
#include "ixheaacd_mps_basic_op.h"
#include "ixheaacd_mps_decor.h"
#include "ixheaacd_mps_hybfilter.h"
#include "ixheaacd_mps_nlc_dec.h"
#include "ixheaacd_mps_huff_tab.h"

extern const ia_huff_pt0_nodes_struct ixheaacd_huff_part0_nodes;
extern const ia_huff_ipd_nodes_struct ixheaacd_huff_ipd_nodes;
extern const ia_huff_lav_nodes_struct ixheaacd_huff_lav_idx_nodes;
extern const ia_huff_pt0_nodes_struct ixheaacd_huff_pilot_nodes;
extern const ia_huff_cld_nodes_struct ixheaacd_huff_cld_nodes;
extern const ia_huff_icc_nodes_struct ixheaacd_huff_icc_nodes;
extern const ia_huff_cpc_nodes_struct ixheaacd_huff_cpc_nodes;
extern const ia_huff_res_nodes_struct ixheaacd_huff_reshape_nodes;

WORD32 ixheaacd_mps_create(ia_mps_dec_state_struct* self, WORD32 bs_frame_len,
                           WORD32 residual_coding,
                           ia_usac_dec_mps_config_struct* mps212_config) {
  WORD32 num_ch;
  WORD32 err_code = 0;

  ia_mps_bs_frame bs_frame;

  self->num_parameter_sets = 1;
  self->qmf_band_count = 64;

  self->res_ch_count = 0;

  if (mps212_config) {
    self->config = mps212_config;
    self->frame_length = bs_frame_len;
    self->in_ch_count = 1;
    self->out_ch_count = 2;
    self->residual_coding = residual_coding;
    if (self->residual_coding) {
      self->bs_residual_present = 1;
      self->bs_residual_bands = mps212_config->bs_residual_bands;
      if (self->config->bs_phase_coding) {
        self->config->bs_phase_coding = 2;
      }
    }
  }

  err_code = ixheaacd_mps_header_decode(self);

  if (err_code != IA_NO_ERROR) {
    self->mps_init_done = 0;
    return err_code;
  }

  if ((self->residual_coding) && (self->res_bands > 0)) self->res_ch_count++;

  ixheaacd_mps_env_init(self);

  self->resolution = self->qmf_band_count;

  for (num_ch = 0; num_ch < self->out_ch_count; num_ch++) {
    ixheaacd_mps_synt_init(self->qmf_filt_state[num_ch]);
  }

  ixheaacd_mps_qmf_hybrid_analysis_init(&self->hyb_filt_state[0]);

  if ((self->residual_coding) && (self->res_bands > 0))
    ixheaacd_mps_qmf_hybrid_analysis_init(&self->hyb_filt_state[1]);

  err_code = ixheaacd_mps_decor_init(&(self->mps_decor), self->hyb_band_count_max,
                                     self->config->bs_decorr_config,
                                     self->object_type);

  if (err_code != IA_NO_ERROR) {
    self->mps_init_done = 0;
    return err_code;
  }

  ixheaacd_mps_init_pre_and_post_matrix(self);

  self->parse_nxt_frame = 1;

  bs_frame = self->bs_frame;
  memset(bs_frame.cld_idx_pre, 0, MAX_PARAMETER_BANDS * sizeof(WORD32));
  memset(bs_frame.icc_idx_pre, 0, MAX_PARAMETER_BANDS * sizeof(WORD32));
  memset(bs_frame.cmp_cld_idx_prev, 0, MAX_PARAMETER_BANDS * sizeof(WORD32));
  memset(bs_frame.cmp_icc_idx_prev, 0, MAX_PARAMETER_BANDS * sizeof(WORD32));

  self->subband_var.init_flag = 0;
  self->subband_var.update_old_ener = 0;
  self->subband_var.nrg_dir = 0;
  memset(self->subband_var.nrg_diff, 0, 2 * sizeof(FLOAT32));

  memset(self->opd_smooth.smooth_l_phase, 0,
         MAX_PARAMETER_BANDS * sizeof(WORD32));
  memset(self->opd_smooth.smooth_r_phase, 0,
         MAX_PARAMETER_BANDS * sizeof(WORD32));
  self->mps_init_done = 1;

  return 0;
}

static const FLOAT32 ixheaacd_tsd_mul_re[] = {
    1.0f,  0.707106781186548f,  0.0f, -0.707106781186548f,
    -1.0f, -0.707106781186548f, 0.0f, 0.707106781186548f};

static const FLOAT32 ixheaacd_tsd_mul_im[] = {
    0.0f, 0.707106781186548f,  1.0f,  0.707106781186548f,
    0.0f, -0.707106781186548f, -1.0f, -0.707106781186548f};

VOID ixheaacd_mps_qmf_hyb_analysis(ia_mps_dec_state_struct* self) {
  if (self->object_type == AOT_ER_AAC_ELD ||
      self->object_type == AOT_ER_AAC_LD) {
    WORD32 k, n;

    for (n = 0; n < self->time_slots; n++) {
      for (k = 0; k < self->qmf_band_count; k++) {
        self->hyb_in[0][k][n].re = self->qmf_in[0][n][k].re;
        self->hyb_in[0][k][n].im = self->qmf_in[0][n][k].im;
      }
    }
  } else {
      ixheaacd_mps_qmf_hybrid_analysis(&self->hyb_filt_state[0], self->qmf_in[0],
                                   self->qmf_band_count, self->time_slots,
                                   self->hyb_in[0]);
  }

  if ((self->residual_coding) && (self->res_bands > 0)) {
    ixheaacd_mps_qmf_hybrid_analysis(&self->hyb_filt_state[self->in_ch_count],
                                     self->qmf_in[1], self->band_count[1],
                                     self->time_slots, self->hyb_res);
  }
}

VOID ixheaacd_mps_qmf_hyb_synthesis(ia_mps_dec_state_struct* self) {
  WORD32 ch;

  if (self->object_type == AOT_ER_AAC_ELD ||
      self->object_type == AOT_ER_AAC_LD) {
    WORD32 k, n;
    for (ch = 0; ch < self->out_ch_count; ch++) {
      for (n = 0; n < self->time_slots; n++) {
        for (k = 0; k < self->qmf_band_count; k++) {
          self->qmf_out_dir[ch][n][k].re = self->hyb_dir_out[ch][n][k].re;
          self->qmf_out_dir[ch][n][k].im = self->hyb_dir_out[ch][n][k].im;
        }
      }
    }
  } else {
      for (ch = 0; ch < self->out_ch_count; ch++) {
          ixheaacd_mps_qmf_hybrid_synthesis(self->hyb_dir_out[ch],
                                      self->qmf_band_count, self->time_slots,
                                      self->qmf_out_dir[ch]);
      }
  }
}

VOID ixheaacd_mps_decor(ia_mps_dec_state_struct* self) {
  WORD32 k, sb_sample, idx;

  ia_cmplx_flt_struct(*scratch)[MAX_HYBRID_BANDS_MPS];

  ia_cmplx_flt_struct coeff;
  WORD32 band_start = 7;

  scratch = self->scratch;

  for (k = self->dir_sig_count; k < self->dir_sig_count + self->decor_sig_count;
       k++) {
    if (self->bs_tsd_enable) {
      for (sb_sample = 0; sb_sample < self->time_slots; sb_sample++) {
        if (self->bs_tsd_sep_data[sb_sample]) {
          for (idx = band_start; idx < self->mps_decor.num_bins; idx++) {
            scratch[sb_sample][idx].re = self->v[k][sb_sample][idx].re;
            scratch[sb_sample][idx].im = self->v[k][sb_sample][idx].im;
            self->v[k][sb_sample][idx].re = 0.0f;
            self->v[k][sb_sample][idx].im = 0.0f;
          }
        }
      }
    }

    ixheaacd_mps_decor_apply(&self->mps_decor, self->v[k], self->w_diff[k],
                             self->time_slots, NO_RES_BANDS,
                             self->ldmps_config.ldmps_present_flag);

    if (self->bs_tsd_enable) {
      for (sb_sample = 0; sb_sample < self->time_slots; sb_sample++) {
        if (self->bs_tsd_sep_data[sb_sample]) {
          coeff.re = ixheaacd_tsd_mul_re[self->bs_tsd_tr_phase_data[sb_sample]];
          coeff.im = ixheaacd_tsd_mul_im[self->bs_tsd_tr_phase_data[sb_sample]];

          for (idx = band_start; idx < self->mps_decor.num_bins; idx++) {
            self->w_diff[k][sb_sample][idx].re +=
                coeff.re * scratch[sb_sample][idx].re -
                coeff.im * scratch[sb_sample][idx].im;
            self->w_diff[k][sb_sample][idx].im +=
                coeff.im * scratch[sb_sample][idx].re +
                coeff.re * scratch[sb_sample][idx].im;
          }
        }
      }
    }
  }
}

VOID ixheaacd_mps_mix_res_decor(ia_mps_dec_state_struct* self) {
  WORD32 ts, qs, row, indx;

  for (ts = 0; ts < self->time_slots; ts++) {
    for (qs = 0; qs < self->hyb_band_count_max; qs++) {
      indx = self->hyb_band_to_processing_band_table[qs];

      for (row = 0; row < self->dir_sig_count; row++) {
        self->w_dir[row][ts][qs].re = self->v[row][ts][qs].re;
        self->w_dir[row][ts][qs].im = self->v[row][ts][qs].im;
      }

      for (row = self->dir_sig_count;
           row < (self->dir_sig_count + self->decor_sig_count); row++) {
        if (indx < self->res_bands) {
          self->w_dir[row][ts][qs].re = self->hyb_res[qs][ts].re;
          self->w_dir[row][ts][qs].im = self->hyb_res[qs][ts].im;
        } else {
          self->w_dir[row][ts][qs].re = 0.0f;
          self->w_dir[row][ts][qs].im = 0.0f;
        }
      }

      for (row = 0; row < self->dir_sig_count; row++) {
        self->w_diff[row][ts][qs].re = 0.0f;
        self->w_diff[row][ts][qs].im = 0.0f;
      }

      for (row = self->dir_sig_count;
           row < (self->dir_sig_count + self->decor_sig_count); row++) {
        if (indx < self->res_bands) {
          self->w_diff[row][ts][qs].re = 0.0f;
          self->w_diff[row][ts][qs].im = 0.0f;
        }
      }
    }
  }
}

VOID ixheaacd_mps_mix_res_decor_residual_band(ia_mps_dec_state_struct* self) {
  WORD32 ts, qs, indx;
  for (qs = 0; qs < self->hyb_band_count_max; qs++) {
    indx = self->hyb_band_to_processing_band_table[qs];
    if (indx >= self->res_bands) {
      if (qs < self->hyb_band_count[1]) {
        for (ts = 0; ts < self->time_slots; ts++) {
          self->w_dir[1][ts][qs].re = 0.0f;
          self->w_dir[1][ts][qs].im = 0.0f;
        }
      }
    } else {
      for (ts = 0; ts < self->time_slots; ts++) {
        self->w_diff[1][ts][qs].re = 0.0f;
        self->w_diff[1][ts][qs].im = 0.0f;
      }
    }
  }
}

VOID ixheaacd_mps_create_w(ia_mps_dec_state_struct* self) {
  ixheaacd_mps_decor(self);
  ixheaacd_mps_mix_res_decor(self);
}

VOID ixheaacd_mps_qmf_hyb_analysis_no_pre_mix(ia_mps_dec_state_struct* self) {
  ixheaacd_mps_qmf_hybrid_analysis_no_pre_mix(
      &self->hyb_filt_state[0], self->qmf_in[0], self->band_count[0],
      self->time_slots, self->w_dir[0]);

  if (self->res_bands) {
    ixheaacd_mps_qmf_hybrid_analysis_no_pre_mix(
        &self->hyb_filt_state[1], self->qmf_in[1], self->band_count[1],
        self->time_slots, self->w_dir[1]);

    if (self->res_bands != 28) {
      ixheaacd_mps_decor_apply(&self->mps_decor, self->w_dir[0],
                               self->w_diff[1], self->time_slots,
                               self->res_bands,
                               self->ldmps_config.ldmps_present_flag);

      ixheaacd_mps_mix_res_decor_residual_band(self);
    }
  } else {
    ixheaacd_mps_decor_apply(&self->mps_decor, self->w_dir[0], self->w_diff[1],
                             self->time_slots, NO_RES_BANDS,
                             self->ldmps_config.ldmps_present_flag);
  }
}

WORD32 ixheaacd_mps_apply(ia_mps_dec_state_struct* self,
                          FLOAT32** input_buffer[4],
                          FLOAT32 (*output_buffer)[4096]) {
  WORD32 ch, ts, qs;
  WORD32 time_slots = self->time_slots;
  WORD32 in_ch_count = self->in_ch_count + self->res_ch_count;
  WORD32 err = 0;
  self->hyb_band_count[0] = self->band_count[0] - QMF_BANDS_TO_HYBRID + 10;
  self->hyb_band_count[1] = self->band_count[1] - QMF_BANDS_TO_HYBRID + 10;
  self->hyb_band_count_max =
      max(self->hyb_band_count[0], self->hyb_band_count[1]);
  self->mps_decor.decor_nrg_smooth.num_bins = self->hyb_band_count_max;
  self->mps_decor.num_bins = self->hyb_band_count_max;
  self->output_buffer = output_buffer;

  err = ixheaacd_mps_frame_decode(self);

  if (err != IA_NO_ERROR) return err;

  ixheaacd_pre_and_mix_matrix_calculation(self);

  ixheaacd_mps_pre_matrix_mix_matrix_smoothing(self);

  for (ch = 0; ch < in_ch_count; ch++) {
    for (ts = 0; ts < time_slots; ts++) {
      for (qs = 0; qs < self->band_count[ch]; qs++) {
        self->qmf_in[ch][qs][ts].re =
            self->input_gain * input_buffer[2 * ch][ts][qs];
        self->qmf_in[ch][qs][ts].im =
            self->input_gain * input_buffer[2 * ch + 1][ts][qs];
      }
    }
  }

  if (!(self->pre_mix_req | self->bs_tsd_enable)) {
    ixheaacd_mps_qmf_hyb_analysis_no_pre_mix(self);
  } else {
    ixheaacd_mps_qmf_hyb_analysis(self);

    ixheaacd_mps_apply_pre_matrix(self);

    ixheaacd_mps_create_w(self);
  }

  if ((!(self->res_bands | self->pre_mix_req)) &&
      (self->config->bs_phase_coding == 0)) {
    ixheaacd_mps_apply_mix_matrix_type1(self);

  } else if (self->pre_mix_req) {
    ixheaacd_mps_apply_mix_matrix_type2(self);

  } else {
    ixheaacd_mps_apply_mix_matrix_type3(self);
  }

  if (self->config->bs_temp_shape_config == 2) {
    ixheaacd_mps_time_env_shaping(self);
  }

  err = ixheaacd_mps_temp_process(self);
  if (err) return err;

  self->parse_nxt_frame = 1;
  self->pre_mix_req = 0;
  return 0;
}

static VOID ixheaacd_mps_pcm_decode(ia_bit_buf_struct *it_bit_buff,
                                    WORD32* out_data_1, WORD32* out_data_2,
                                    WORD32 ixheaacd_drc_offset, WORD32 num_val,
                                    WORD32 num_levels) {
  WORD32 i = 0, j = 0, idx = 0;
  WORD32 max_grp_len = 0, grp_len = 0, next_val = 0, grp_val = 0;
  UWORD32 data = 0;

  FLOAT32 ld_nlev = 0.f;

  WORD32 pcm_chunk_size[7] = {0};

  switch (num_levels) {
    case 3:
      max_grp_len = 5;
      break;
    case 7:
      max_grp_len = 6;
      break;
    case 11:
      max_grp_len = 2;
      break;
    case 13:
      max_grp_len = 4;
      break;
    case 19:
      max_grp_len = 4;
      break;
    case 25:
      max_grp_len = 3;
      break;
    case 51:
      max_grp_len = 4;
      break;
    case 4:
    case 8:
    case 15:
    case 16:
    case 26:
    case 31:
      max_grp_len = 1;
      break;
    default:
      return;
  }

  ld_nlev = (FLOAT32)(log((FLOAT32)num_levels) / log(2.f));

  for (i = 1; i <= max_grp_len; i++) {
    pcm_chunk_size[i] = (WORD32)ceil((FLOAT32)(i)*ld_nlev);
  }

  for (i = 0; i < num_val; i += max_grp_len) {
    grp_len = min(max_grp_len, num_val - i);
    data = ixheaacd_read_bits_buf(it_bit_buff, pcm_chunk_size[grp_len]);

    grp_val = data;

    for (j = 0; j < grp_len; j++) {
      idx = i + (grp_len - j - 1);
      next_val = grp_val % num_levels;

      if (out_data_2 == NULL) {
        out_data_1[idx] = next_val - ixheaacd_drc_offset;
      } else if (out_data_1 == NULL) {
        out_data_2[idx] = next_val - ixheaacd_drc_offset;
      } else {
        if (idx % 2) {
          out_data_2[idx / 2] = next_val - ixheaacd_drc_offset;
        } else {
          out_data_1[idx / 2] = next_val - ixheaacd_drc_offset;
        }
      }

      grp_val = (grp_val - next_val) / num_levels;
    }
  }

  return;
}

static VOID ixheaacd_mps_huff_read(ia_bit_buf_struct *it_bit_buff,
                                   const WORD32 (*node_tab)[][2],
                                   WORD32* out_data) {
  WORD32 node = 0;
  UWORD32 next_bit = 0;

  do {
    next_bit = ixheaacd_read_bits_buf(it_bit_buff, 1);
    node = (*node_tab)[node][next_bit];
  } while (node > 0);

  *out_data = node;

  return;
}

static VOID ixheaacd_mps_huff_read_2d(ia_bit_buf_struct *it_bit_buff,
                                      const WORD32 (*node_tab)[][2],
                                      WORD32 out_data[2], WORD32* escape)

{
  WORD32 huff_2d_8bit = 0;
  WORD32 node = 0;

  ixheaacd_mps_huff_read(it_bit_buff, node_tab, &node);
  *escape = (node == 0);

  if (*escape) {
    out_data[0] = 0;
    out_data[1] = 1;
  } else {
    huff_2d_8bit = -(node + 1);
    out_data[0] = huff_2d_8bit >> 4;
    out_data[1] = huff_2d_8bit & 0xf;
  }

  return;
}

static VOID ixheaacd_mps_sym_restore(ia_bit_buf_struct *it_bit_buff,
                                     WORD32 lav, WORD32 data[2]) {
  WORD32 tmp = 0;
  UWORD32 sym_bit = 0;

  WORD32 sum_val = data[0] + data[1];
  WORD32 diff_val = data[0] - data[1];

  if (sum_val > lav) {
    data[0] = -sum_val + (2 * lav + 1);
    data[1] = -diff_val;
  } else {
    data[0] = sum_val;
    data[1] = diff_val;
  }

  if (data[0] + data[1] != 0) {
    sym_bit = ixheaacd_read_bits_buf(it_bit_buff, 1);
    if (sym_bit) {
      data[0] = -data[0];
      data[1] = -data[1];
    }
  }

  if (data[0] - data[1] != 0) {
    sym_bit = ixheaacd_read_bits_buf(it_bit_buff, 1);
    if (sym_bit) {
      tmp = data[0];
      data[0] = data[1];
      data[1] = tmp;
    }
  }

  return;
}

static VOID ixheaacd_mps_sym_restoreipd(ia_bit_buf_struct *it_bit_buff,
                                        WORD32 lav, WORD32 data[2]) {
  WORD32 tmp = 0;
  UWORD32 sym_bit = 0;

  WORD32 sum_val = data[0] + data[1];
  WORD32 diff_val = data[0] - data[1];

  if (sum_val > lav) {
    data[0] = -sum_val + (2 * lav + 1);
    data[1] = -diff_val;
  } else {
    data[0] = sum_val;
    data[1] = diff_val;
  }

  if (data[0] - data[1] != 0) {
    sym_bit = ixheaacd_read_bits_buf(it_bit_buff, 1);
    if (sym_bit) {
      tmp = data[0];
      data[0] = data[1];
      data[1] = tmp;
    }
  }

  return;
}

static VOID ixheaacd_mps_huff_dec_pilot(ia_bit_buf_struct *it_bit_buff,
                                        const WORD32 (*node_tab)[][2],
                                        WORD32* pilot_data) {
  WORD32 node = 0;

  ixheaacd_mps_huff_read(it_bit_buff, node_tab, &node);
  *pilot_data = -(node + 1);

  return;
}

static VOID ixheaacd_mps_huff_dec_cld_1d(
    ia_bit_buf_struct *it_bit_buff,
    const ia_huff_cld_node_1d_struct* huff_nodes, WORD32* out_data,
    WORD32 num_val, WORD32 p0_flag) {
  WORD32 i = 0, node = 0, ixheaacd_drc_offset = 0;
  WORD32 od = 0, od_sign = 0;
  UWORD32 data = 0;

  if (p0_flag) {
    ixheaacd_mps_huff_read(it_bit_buff,
                           (ia_huff_node_struct)&ixheaacd_huff_part0_nodes.cld,
                           &node);
    out_data[0] = -(node + 1);
    ixheaacd_drc_offset = 1;
  }

  for (i = ixheaacd_drc_offset; i < num_val; i++) {
    ixheaacd_mps_huff_read(it_bit_buff,
                           (ia_huff_node_struct)&huff_nodes->node_tab, &node);
    od = -(node + 1);

    if (od != 0) {
      data = ixheaacd_read_bits_buf(it_bit_buff, 1);
      od_sign = data;

      if (od_sign) od = -od;
    }

    out_data[i] = od;
  }

  return;
}

static VOID ixheaacd_mps_huff_dec_ipd_1d(
    ia_bit_buf_struct *it_bit_buff,
    const ia_huff_ipd_node_1d_struct* huff_nodes, WORD32* out_data,
    WORD32 num_val, WORD32 p0_flag) {
  WORD32 i = 0, node = 0, ixheaacd_drc_offset = 0;
  WORD32 od = 0;

  if (p0_flag) {
    ixheaacd_mps_huff_read(
        it_bit_buff, (ia_huff_node_struct)&ixheaacd_huff_ipd_nodes.hp0.node_tab,
        &node);
    out_data[0] = -(node + 1);
    ixheaacd_drc_offset = 1;
  }

  for (i = ixheaacd_drc_offset; i < num_val; i++) {
    ixheaacd_mps_huff_read(it_bit_buff,
                           (ia_huff_node_struct)&huff_nodes->node_tab, &node);
    od = -(node + 1);
    out_data[i] = od;
  }

  return;
}

static VOID ixheaacd_mps_huff_dec_icc_1d(
    ia_bit_buf_struct *it_bit_buff,
    const ia_huff_icc_node_1d_struct* huff_nodes, WORD32* out_data,
    WORD32 num_val, WORD32 p0_flag) {
  WORD32 i = 0, node = 0, ixheaacd_drc_offset = 0;
  WORD32 od = 0, od_sign = 0;
  UWORD32 data = 0;

  if (p0_flag) {
    ixheaacd_mps_huff_read(it_bit_buff,
                           (ia_huff_node_struct)&ixheaacd_huff_part0_nodes.icc,
                           &node);
    out_data[0] = -(node + 1);
    ixheaacd_drc_offset = 1;
  }

  for (i = ixheaacd_drc_offset; i < num_val; i++) {
    ixheaacd_mps_huff_read(it_bit_buff,
                           (ia_huff_node_struct)&huff_nodes->node_tab, &node);
    od = -(node + 1);

    if (od != 0) {
      data = ixheaacd_read_bits_buf(it_bit_buff, 1);
      od_sign = data;

      if (od_sign) od = -od;
    }

    out_data[i] = od;
  }

  return;
}

static VOID ia_mps_dec_huff_dec_cpc_1d(
    const ia_huff_cpc_node_1d_struct *huff_nodes, WORD32 *out_data,
    WORD32 num_val, WORD32 p0_flag, ia_bit_buf_struct *h_bit_buf) {
  WORD32 i = 0, node = 0, offset = 0;
  WORD32 od = 0, od_sign = 0;
  WORD32 data = 0;

  if (p0_flag) {
    ixheaacd_mps_huff_read(
        h_bit_buf, (ia_huff_node_struct) & (ixheaacd_huff_part0_nodes.cpc),
        &node);
    out_data[0] = -(node + 1);
    offset = 1;
  }

  for (i = offset; i < num_val; i++) {
    ixheaacd_mps_huff_read(h_bit_buf,
                           (ia_huff_node_struct)&huff_nodes->node_tab, &node);
    od = -(node + 1);

    if (od != 0) {
      data = ixheaacd_read_bits_buf(h_bit_buf, 1);
      od_sign = data;

      if (od_sign) od = -od;
    }

    out_data[i] = od;
  }
}

static VOID ixheaacd_mps_huff_dec_cld_2d(
    ia_bit_buf_struct *it_bit_buff,
    const ia_huff_cld_node_2d_struct* huff_nodes, WORD32 out_data[][2],
    WORD32 num_val, WORD32 ch_fac, WORD32* p0_data[2]) {
  WORD32 i = 0, lav = 0, escape = 0, esc_contrl = 0;
  WORD32 node = 0;
  UWORD32 data = 0;

  WORD32 esc_data[MAXBANDS][2] = {{0}};
  WORD32 esc_idx[MAXBANDS] = {0};

  ixheaacd_mps_huff_read(
      it_bit_buff, (ia_huff_node_struct)&ixheaacd_huff_lav_idx_nodes.node_tab,
      &node);
  data = -(node + 1);

  lav = 2 * data + 3;

  if (p0_data[0] != NULL) {
    ixheaacd_mps_huff_read(it_bit_buff,
                           (ia_huff_node_struct)&ixheaacd_huff_part0_nodes.cld,
                           &node);
    *p0_data[0] = -(node + 1);
  }
  if (p0_data[1] != NULL) {
    ixheaacd_mps_huff_read(it_bit_buff,
                           (ia_huff_node_struct)&ixheaacd_huff_part0_nodes.cld,
                           &node);
    *p0_data[1] = -(node + 1);
  }

  for (i = 0; i < num_val; i += ch_fac) {
    switch (lav) {
      case 3:
        ixheaacd_mps_huff_read_2d(it_bit_buff,
                                  (ia_huff_node_struct)&huff_nodes->lav3,
                                  out_data[i], &escape);
        break;
      case 5:
        ixheaacd_mps_huff_read_2d(it_bit_buff,
                                  (ia_huff_node_struct)&huff_nodes->lav5,
                                  out_data[i], &escape);
        break;
      case 7:
        ixheaacd_mps_huff_read_2d(it_bit_buff,
                                  (ia_huff_node_struct)&huff_nodes->lav7,
                                  out_data[i], &escape);
        break;
      case 9:
        ixheaacd_mps_huff_read_2d(it_bit_buff,
                                  (ia_huff_node_struct)&huff_nodes->lav9,
                                  out_data[i], &escape);
        break;
      default:
        break;
    }

    if (escape) {
      esc_idx[esc_contrl++] = i;
    } else {
      ixheaacd_mps_sym_restore(it_bit_buff, lav, out_data[i]);
    }
  }

  if (esc_contrl > 0) {
    ixheaacd_mps_pcm_decode(it_bit_buff, esc_data[0], esc_data[1], 0,
                            2 * esc_contrl, (2 * lav + 1));

    for (i = 0; i < esc_contrl; i++) {
      out_data[esc_idx[i]][0] = esc_data[0][i] - lav;
      out_data[esc_idx[i]][1] = esc_data[1][i] - lav;
    }
  }

  return;
}

static VOID ixheaacd_mps_huff_dec_icc_2d(
    ia_bit_buf_struct *it_bit_buff,
    const ia_huff_icc_node_2d_struct* huff_nodes, WORD32 out_data[][2],
    WORD32 num_val, WORD32 ch_fac, WORD32* p0_data[2]) {
  WORD32 i = 0, lav = 0, escape = 0, esc_contrl = 0;
  WORD32 node = 0;
  UWORD32 data = 0;

  WORD32 esc_data[2][MAXBANDS] = {{0}};
  WORD32 esc_idx[MAXBANDS] = {0};

  ixheaacd_mps_huff_read(
      it_bit_buff, (ia_huff_node_struct)&ixheaacd_huff_lav_idx_nodes.node_tab,
      &node);
  data = -(node + 1);

  lav = 2 * data + 1;

  if (p0_data[0] != NULL) {
    ixheaacd_mps_huff_read(it_bit_buff,
                           (ia_huff_node_struct)&ixheaacd_huff_part0_nodes.icc,
                           &node);
    *p0_data[0] = -(node + 1);
  }
  if (p0_data[1] != NULL) {
    ixheaacd_mps_huff_read(it_bit_buff,
                           (ia_huff_node_struct)&ixheaacd_huff_part0_nodes.icc,
                           &node);
    *p0_data[1] = -(node + 1);
  }

  for (i = 0; i < num_val; i += ch_fac) {
    switch (lav) {
      case 1:
        ixheaacd_mps_huff_read_2d(it_bit_buff,
                                  (ia_huff_node_struct)&huff_nodes->lav1,
                                  out_data[i], &escape);
        break;
      case 3:
        ixheaacd_mps_huff_read_2d(it_bit_buff,
                                  (ia_huff_node_struct)&huff_nodes->lav3,
                                  out_data[i], &escape);
        break;
      case 5:
        ixheaacd_mps_huff_read_2d(it_bit_buff,
                                  (ia_huff_node_struct)&huff_nodes->lav5,
                                  out_data[i], &escape);
        break;
      case 7:
        ixheaacd_mps_huff_read_2d(it_bit_buff,
                                  (ia_huff_node_struct)&huff_nodes->lav7,
                                  out_data[i], &escape);
        break;
    }

    if (escape) {
      esc_idx[esc_contrl++] = i;
    } else {
      ixheaacd_mps_sym_restore(it_bit_buff, lav, out_data[i]);
    }
  }

  if (esc_contrl > 0) {
    ixheaacd_mps_pcm_decode(it_bit_buff, esc_data[0], esc_data[1], 0,
                            2 * esc_contrl, (2 * lav + 1));

    for (i = 0; i < esc_contrl; i++) {
      out_data[esc_idx[i]][0] = esc_data[0][i] - lav;
      out_data[esc_idx[i]][1] = esc_data[1][i] - lav;
    }
  }

  return;
}

static VOID ixheaacd_mps_huff_dec_ipd_2d(
    ia_bit_buf_struct *it_bit_buff,
    const ia_huff_ipd_node_2d_struct* huff_nodes, WORD32 out_data[][2],
    WORD32 num_val, WORD32 ch_fac, WORD32* p0_data[2]) {
  WORD32 i = 0, lav = 0, escape = 0, esc_contrl = 0;
  WORD32 node = 0;
  UWORD32 data = 0;

  WORD32 esc_data[2][MAXBANDS] = {{0}};
  WORD32 esc_idx[MAXBANDS] = {0};

  ixheaacd_mps_huff_read(
      it_bit_buff, (ia_huff_node_struct)&ixheaacd_huff_lav_idx_nodes.node_tab,
      &node);

  data = -(node + 1);
  if (data == 0)
    data = 3;
  else
    data--;

  lav = 2 * data + 1;

  if (p0_data[0] != NULL) {
    ixheaacd_mps_huff_read(
        it_bit_buff, (ia_huff_node_struct)&ixheaacd_huff_ipd_nodes.hp0.node_tab,
        &node);
    *p0_data[0] = -(node + 1);
  }
  if (p0_data[1] != NULL) {
    ixheaacd_mps_huff_read(
        it_bit_buff, (ia_huff_node_struct)&ixheaacd_huff_ipd_nodes.hp0.node_tab,
        &node);
    *p0_data[1] = -(node + 1);
  }

  for (i = 0; i < num_val; i += ch_fac) {
    switch (lav) {
      case 1:
        ixheaacd_mps_huff_read_2d(it_bit_buff,
                                  (ia_huff_node_struct)&huff_nodes->lav1,
                                  out_data[i], &escape);
        break;
      case 3:
        ixheaacd_mps_huff_read_2d(it_bit_buff,
                                  (ia_huff_node_struct)&huff_nodes->lav3,
                                  out_data[i], &escape);
        break;
      case 5:
        ixheaacd_mps_huff_read_2d(it_bit_buff,
                                  (ia_huff_node_struct)&huff_nodes->lav5,
                                  out_data[i], &escape);
        break;
      case 7:
        ixheaacd_mps_huff_read_2d(it_bit_buff,
                                  (ia_huff_node_struct)&huff_nodes->lav7,
                                  out_data[i], &escape);
        break;
    }

    if (escape) {
      esc_idx[esc_contrl++] = i;
    } else {
      ixheaacd_mps_sym_restoreipd(it_bit_buff, lav, out_data[i]);
    }
  }

  if (esc_contrl > 0) {
    ixheaacd_mps_pcm_decode(it_bit_buff, esc_data[0], esc_data[1], 0,
                            2 * esc_contrl, (2 * lav + 1));

    for (i = 0; i < esc_contrl; i++) {
      out_data[esc_idx[i]][0] = esc_data[0][i] - lav;
      out_data[esc_idx[i]][1] = esc_data[1][i] - lav;
    }
  }

  return;
}

static VOID ia_mps_dec_huff_dec_cpc_2d(
    const ia_mps_dec_huff_cpc_nod_2d *huff_nodes, WORD32 out_data[][2],
    WORD32 num_val, WORD32 stride, WORD32 *p0_data[2],
    ia_bit_buf_struct *h_bit_buf) {
  WORD32 i = 0, lav = 0, escape = 0, esc_cntr = 0;
  WORD32 node = 0;
  WORD32 data = 0;

  WORD32 esc_data[2][MAXBANDS] = {{0}};
  WORD32 esc_idx[MAXBANDS] = {0};

  ixheaacd_mps_huff_read(
      h_bit_buf, (ia_huff_node_struct) & (ixheaacd_huff_lav_idx_nodes.node_tab),
      &node);
  data = -(node + 1);

  lav = 3 * data + 3;

  if (p0_data[0] != NULL) {
    ixheaacd_mps_huff_read(
        h_bit_buf, (ia_huff_node_struct) & (ixheaacd_huff_part0_nodes.cpc),
        &node);
    *p0_data[0] = -(node + 1);
  }
  if (p0_data[1] != NULL) {
    ixheaacd_mps_huff_read(
        h_bit_buf, (ia_huff_node_struct) & (ixheaacd_huff_part0_nodes.cpc),
        &node);
    *p0_data[1] = -(node + 1);
  }

  for (i = 0; i < num_val; i += stride) {
    switch (lav) {
      case LAV_3:
        ixheaacd_mps_huff_read_2d(h_bit_buf,
                                  (ia_huff_node_struct)&huff_nodes->lav3,
                                  out_data[i], &escape);
        break;
      case LAV_6:
        ixheaacd_mps_huff_read_2d(h_bit_buf,
                                  (ia_huff_node_struct)&huff_nodes->lav6,
                                  out_data[i], &escape);
        break;
      case LAV_9:
        ixheaacd_mps_huff_read_2d(h_bit_buf,
                                  (ia_huff_node_struct)&huff_nodes->lav9,
                                  out_data[i], &escape);
        break;
      case LAV_12:
        ixheaacd_mps_huff_read_2d(h_bit_buf,
                                  (ia_huff_node_struct)&huff_nodes->lav12,
                                  out_data[i], &escape);
        break;
      default:
        break;
    }

    if (escape) {
      esc_idx[esc_cntr++] = i;
    } else {
      ixheaacd_mps_sym_restore(h_bit_buf, lav, out_data[i]);
    }
  }

  if (esc_cntr > 0) {
    ixheaacd_mps_pcm_decode(h_bit_buf, esc_data[0], esc_data[1], 0,
                            (esc_cntr << 1), ((lav << 1) + 1));

    for (i = 0; i < esc_cntr; i++) {
      out_data[esc_idx[i]][0] = esc_data[0][i] - lav;
      out_data[esc_idx[i]][1] = esc_data[1][i] - lav;
    }
  }
  return;
}

static VOID ixheaacd_huff_decode(ia_bit_buf_struct *it_bit_buff, WORD32 *out_data_1,
                                 WORD32 *out_data_2, WORD32 data_type, WORD32 diff_type_1,
                                 WORD32 diff_type_2, WORD32 pilot_coding_flag, WORD32 *pilot_data,
                                 WORD32 num_val, WORD32 *cdg_scheme, WORD32 ld_mps_flag) {
  WORD32 diff_type;

  WORD32 i = 0;
  UWORD32 data = 0;

  WORD32 pair_vec[MAXBANDS][2];

  WORD32* p0_data_1[2] = {NULL, NULL};
  WORD32* p0_data_2[2] = {NULL, NULL};

  WORD32 p0_flag[2];

  WORD32 num_val_1_int = num_val;
  WORD32 num_val_2_int = num_val;

  WORD32* out_data_1_int = out_data_1;
  WORD32* out_data_2_int = out_data_2;

  WORD32 df_rest_flag_1 = 0;
  WORD32 df_rest_flag_2 = 0;

  WORD32 huff_yy_1;
  WORD32 huff_yy_2;
  WORD32 huff_yy;
  if (pilot_coding_flag) {
    switch (data_type) {
      case CLD:
        if (out_data_1 != NULL) {
          ixheaacd_mps_huff_dec_pilot(
              it_bit_buff, (ia_huff_node_struct)&ixheaacd_huff_pilot_nodes.cld,
              pilot_data);
        }
        break;

      case ICC:
        if (out_data_1 != NULL) {
          ixheaacd_mps_huff_dec_pilot(
              it_bit_buff, (ia_huff_node_struct)&ixheaacd_huff_pilot_nodes.icc,
              pilot_data);
        }
        break;

      case CPC:
        if (out_data_1 != NULL) {
          ixheaacd_mps_huff_dec_pilot(
              it_bit_buff,
              (ia_huff_node_struct) & (ixheaacd_huff_pilot_nodes.cpc),
              pilot_data);
        }
        break;

      default:
        break;
    }
  }

  data = ixheaacd_read_bits_buf(it_bit_buff, 1);
  *cdg_scheme = data << PAIR_SHIFT;

  if (*cdg_scheme >> PAIR_SHIFT == HUFF_2D) {
    if ((out_data_1 != NULL) && (out_data_2 != NULL) && (ld_mps_flag != 1)) {
      data = ixheaacd_read_bits_buf(it_bit_buff, 1);
      *cdg_scheme |= data;
    } else {
      *cdg_scheme |= FREQ_PAIR;
    }
  }

  if (pilot_coding_flag) {
    huff_yy_1 = PCM_PLT;
    huff_yy_2 = PCM_PLT;
  } else {
    huff_yy_1 = diff_type_1;
    huff_yy_2 = diff_type_2;
  }

  switch (*cdg_scheme >> PAIR_SHIFT) {
    case HUFF_1D:

      p0_flag[0] = (diff_type_1 == DIFF_FREQ) && !pilot_coding_flag;
      p0_flag[1] = (diff_type_2 == DIFF_FREQ) && !pilot_coding_flag;

      switch (data_type) {
        case CLD:
          if (out_data_1 != NULL) {
            ixheaacd_mps_huff_dec_cld_1d(
                it_bit_buff, &ixheaacd_huff_cld_nodes.h_1_dim[huff_yy_1],
                out_data_1, num_val_1_int, p0_flag[0]);
          }
          if (out_data_2 != NULL) {
            ixheaacd_mps_huff_dec_cld_1d(
                it_bit_buff, &ixheaacd_huff_cld_nodes.h_1_dim[huff_yy_2],
                out_data_2, num_val_2_int, p0_flag[1]);
          }

          break;

        case ICC:
          if (out_data_1 != NULL) {
            ixheaacd_mps_huff_dec_icc_1d(
                it_bit_buff, &ixheaacd_huff_icc_nodes.h_1_dim[huff_yy_1],
                out_data_1, num_val_1_int, p0_flag[0]);
          }
          if (out_data_2 != NULL) {
            ixheaacd_mps_huff_dec_icc_1d(
                it_bit_buff, &ixheaacd_huff_icc_nodes.h_1_dim[huff_yy_2],
                out_data_2, num_val_2_int, p0_flag[1]);
          }

          break;

        case IPD:
          if (out_data_1 != NULL) {
            ixheaacd_mps_huff_dec_ipd_1d(
                it_bit_buff, &ixheaacd_huff_ipd_nodes.h_1_dim[huff_yy_1],
                out_data_1, num_val_1_int, p0_flag[0]);
          }
          if (out_data_2 != NULL) {
            ixheaacd_mps_huff_dec_ipd_1d(
                it_bit_buff, &ixheaacd_huff_ipd_nodes.h_1_dim[huff_yy_2],
                out_data_2, num_val_2_int, p0_flag[1]);
          }

          break;
        case CPC:
          if (out_data_1 != NULL) {
            ia_mps_dec_huff_dec_cpc_1d(
                &(ixheaacd_huff_cpc_nodes.h_1_dim[huff_yy_1]), out_data_1,
                num_val_1_int, p0_flag[0], it_bit_buff);
          }
          if (out_data_2 != NULL) {
            ia_mps_dec_huff_dec_cpc_1d(
                &(ixheaacd_huff_cpc_nodes.h_1_dim[huff_yy_2]), out_data_2,
                num_val_2_int, p0_flag[1], it_bit_buff);
          }

          break;
        default:
          break;
      }

      break;

    case HUFF_2D:

      switch (*cdg_scheme & PAIR_MASK) {
        case FREQ_PAIR:

          if (out_data_1 != NULL) {
            if (!pilot_coding_flag && diff_type_1 == DIFF_FREQ) {
              p0_data_1[0] = &out_data_1[0];
              p0_data_1[1] = NULL;

              num_val_1_int -= 1;
              out_data_1_int += 1;
            }
            df_rest_flag_1 = num_val_1_int % 2;
            if (df_rest_flag_1) num_val_1_int -= 1;
          }
          if (out_data_2 != NULL) {
            if (!pilot_coding_flag && diff_type_2 == DIFF_FREQ) {
              p0_data_2[0] = NULL;
              p0_data_2[1] = &out_data_2[0];

              num_val_2_int -= 1;
              out_data_2_int += 1;
            }
            df_rest_flag_2 = num_val_2_int % 2;
            if (df_rest_flag_2) num_val_2_int -= 1;
          }

          switch (data_type) {
            case CLD:

              if (out_data_1 != NULL) {
                ixheaacd_mps_huff_dec_cld_2d(
                    it_bit_buff,
                    &ixheaacd_huff_cld_nodes.h_2_dim[huff_yy_1][FREQ_PAIR],
                    pair_vec, num_val_1_int, 2, p0_data_1);
                if (df_rest_flag_1) {
                  ixheaacd_mps_huff_dec_cld_1d(
                      it_bit_buff, &ixheaacd_huff_cld_nodes.h_1_dim[huff_yy_1],
                      out_data_1_int + num_val_1_int, 1, 0);
                }
              }
              if (out_data_2 != NULL) {
                ixheaacd_mps_huff_dec_cld_2d(
                    it_bit_buff,
                    &ixheaacd_huff_cld_nodes.h_2_dim[huff_yy_2][FREQ_PAIR],
                    pair_vec + 1, num_val_2_int, 2, p0_data_2);
                if (df_rest_flag_2) {
                  ixheaacd_mps_huff_dec_cld_1d(
                      it_bit_buff, &ixheaacd_huff_cld_nodes.h_1_dim[huff_yy_2],
                      out_data_2_int + num_val_2_int, 1, 0);
                }
              }
              break;

            case ICC:
              if (out_data_1 != NULL) {
                ixheaacd_mps_huff_dec_icc_2d(
                    it_bit_buff,
                    &ixheaacd_huff_icc_nodes.h_2_dim[huff_yy_1][FREQ_PAIR],
                    pair_vec, num_val_1_int, 2, p0_data_1);
                if (df_rest_flag_1) {
                  ixheaacd_mps_huff_dec_icc_1d(
                      it_bit_buff, &ixheaacd_huff_icc_nodes.h_1_dim[huff_yy_1],
                      out_data_1_int + num_val_1_int, 1, 0);
                }
              }
              if (out_data_2 != NULL) {
                ixheaacd_mps_huff_dec_icc_2d(
                    it_bit_buff,
                    &ixheaacd_huff_icc_nodes.h_2_dim[huff_yy_2][FREQ_PAIR],
                    pair_vec + 1, num_val_2_int, 2, p0_data_2);
                if (df_rest_flag_2) {
                  ixheaacd_mps_huff_dec_icc_1d(
                      it_bit_buff, &ixheaacd_huff_icc_nodes.h_1_dim[huff_yy_2],
                      out_data_2_int + num_val_2_int, 1, 0);
                }
              }
              break;

            case IPD:
              if (out_data_1 != NULL) {
                ixheaacd_mps_huff_dec_ipd_2d(
                    it_bit_buff,
                    &ixheaacd_huff_ipd_nodes.h_2_dim[huff_yy_1][FREQ_PAIR],
                    pair_vec, num_val_1_int, 2, p0_data_1);
                if (df_rest_flag_1) {
                  ixheaacd_mps_huff_dec_ipd_1d(
                      it_bit_buff, &ixheaacd_huff_ipd_nodes.h_1_dim[huff_yy_1],
                      out_data_1_int + num_val_1_int, 1, 0);
                }
              }
              if (out_data_2 != NULL) {
                ixheaacd_mps_huff_dec_ipd_2d(
                    it_bit_buff,
                    &ixheaacd_huff_ipd_nodes.h_2_dim[huff_yy_2][FREQ_PAIR],
                    pair_vec + 1, num_val_2_int, 2, p0_data_2);
                if (df_rest_flag_2) {
                  ixheaacd_mps_huff_dec_ipd_1d(
                      it_bit_buff, &ixheaacd_huff_ipd_nodes.h_1_dim[huff_yy_2],
                      out_data_2_int + num_val_2_int, 1, 0);
                }
              }
              break;
            case CPC:
              if (out_data_1 != NULL) {
                ia_mps_dec_huff_dec_cpc_2d(
                    &(ixheaacd_huff_cpc_nodes.h_2_dim[huff_yy_1][FREQ_PAIR]), pair_vec,
                    num_val_1_int, 2, p0_data_1, it_bit_buff);
                if (df_rest_flag_1) {
                  ia_mps_dec_huff_dec_cpc_1d(&(ixheaacd_huff_cpc_nodes.h_1_dim[huff_yy_1]),
                      out_data_1_int + num_val_1_int, 1, 0, it_bit_buff);
                }
              }
              if (out_data_2 != NULL) {
                ia_mps_dec_huff_dec_cpc_2d(
                    &(ixheaacd_huff_cpc_nodes.h_2_dim[huff_yy_2][FREQ_PAIR]), pair_vec + 1,
                    num_val_2_int, 2, p0_data_2, it_bit_buff);

                if (df_rest_flag_2) {
                  ia_mps_dec_huff_dec_cpc_1d(
                      &(ixheaacd_huff_cpc_nodes.h_1_dim[huff_yy_2]),
                      out_data_2_int + num_val_2_int, 1, 0, it_bit_buff);
                }
              }
              break;
            default:
              break;
          }

          if (out_data_1 != NULL) {
            for (i = 0; i < num_val_1_int - 1; i += 2) {
              out_data_1_int[i] = pair_vec[i][0];
              out_data_1_int[i + 1] = pair_vec[i][1];
            }
          }
          if (out_data_2 != NULL) {
            for (i = 0; i < num_val_2_int - 1; i += 2) {
              out_data_2_int[i] = pair_vec[i + 1][0];
              out_data_2_int[i + 1] = pair_vec[i + 1][1];
            }
          }

          break;

        case TIME_PAIR:

          if (!pilot_coding_flag &&
              ((diff_type_1 == DIFF_FREQ) || (diff_type_2 == DIFF_FREQ))) {
            p0_data_1[0] = &out_data_1[0];
            p0_data_1[1] = &out_data_2[0];

            out_data_1_int += 1;
            out_data_2_int += 1;

            num_val_1_int -= 1;
          }

          if ((diff_type_1 == DIFF_TIME) || (diff_type_2 == DIFF_TIME)) {
            diff_type = DIFF_TIME;
          } else {
            diff_type = DIFF_FREQ;
          }
          if (pilot_coding_flag) {
            huff_yy = PCM_PLT;
          } else {
            huff_yy = diff_type;
          }

          switch (data_type) {
            case CLD:
              ixheaacd_mps_huff_dec_cld_2d(
                  it_bit_buff,
                  &ixheaacd_huff_cld_nodes.h_2_dim[huff_yy][TIME_PAIR],
                  pair_vec, num_val_1_int, 1, p0_data_1);
              break;

            case ICC:
              ixheaacd_mps_huff_dec_icc_2d(
                  it_bit_buff,
                  &ixheaacd_huff_icc_nodes.h_2_dim[huff_yy][TIME_PAIR],
                  pair_vec, num_val_1_int, 1, p0_data_1);
              break;

            case IPD:
              ixheaacd_mps_huff_dec_ipd_2d(
                  it_bit_buff,
                  &ixheaacd_huff_ipd_nodes.h_2_dim[huff_yy][TIME_PAIR],
                  pair_vec, num_val_1_int, 1, p0_data_1);
              break;
            case CPC:
              ia_mps_dec_huff_dec_cpc_2d(&(ixheaacd_huff_cpc_nodes.h_2_dim[huff_yy][TIME_PAIR]),
                  pair_vec, num_val_1_int, 1, p0_data_1, it_bit_buff);
              break;
            default:
              break;
          }

          for (i = 0; i < num_val_1_int; i++) {
            out_data_1_int[i] = pair_vec[i][0];
            out_data_2_int[i] = pair_vec[i][1];
          }

          break;

        default:
          break;
      }

      break;

    default:
      break;
  }

  return;
}

static VOID ixheaacd_diff_freq_decode(WORD32* diff_data, WORD32* out_data,
                                      WORD32 num_val) {
  WORD32 i = 0;

  out_data[0] = diff_data[0];

  for (i = 1; i < num_val; i++) {
    out_data[i] = out_data[i - 1] + diff_data[i];
  }
}

static VOID ixheaacd_mps_diff_time_dec_bwd(WORD32* prev_data, WORD32* diff_data,
                                           WORD32* out_data,
                                           WORD32 mixed_diff_type,
                                           WORD32 num_val) {
  WORD32 i = 0;

  if (mixed_diff_type) {
    out_data[0] = diff_data[0];
    for (i = 1; i < num_val; i++) {
      out_data[i] = prev_data[i] + diff_data[i];
    }
  } else {
    for (i = 0; i < num_val; i++) {
      out_data[i] = prev_data[i] + diff_data[i];
    }
  }
}

static VOID ixheaacd_mps_diff_time_dec_fwd(WORD32* prev_data, WORD32* diff_data,
                                           WORD32* out_data,
                                           WORD32 mixed_diff_type,
                                           WORD32 num_val) {
  WORD32 i = 0;

  if (mixed_diff_type) {
    out_data[0] = diff_data[0];
    for (i = 1; i < num_val; i++) {
      out_data[i] = prev_data[i] - diff_data[i];
    }
  } else {
    for (i = 0; i < num_val; i++) {
      out_data[i] = prev_data[i] - diff_data[i];
    }
  }
}

static VOID ixheaacd_attach_lsb(ia_bit_buf_struct *it_bit_buff,
                                WORD32* in_data_msb, WORD32 ixheaacd_drc_offset,
                                WORD32 num_lsb, WORD32 num_val,
                                WORD32* out_data) {
  WORD32 i = 0, lsb = 0, msb = 0;
  UWORD32 data = 0;

  for (i = 0; i < num_val; i++) {
    msb = in_data_msb[i];

    if (num_lsb > 0) {
      data = ixheaacd_read_bits_buf(it_bit_buff, num_lsb);
      lsb = data;

      out_data[i] = ((msb << num_lsb) | lsb) - ixheaacd_drc_offset;
    } else
      out_data[i] = msb - ixheaacd_drc_offset;
  }

  return;
}

WORD32 ixheaacd_mps_ecdatapairdec(ia_bit_buf_struct *it_bit_buff, WORD32 outdata[][MAXBANDS],
                                  WORD32 history[MAXBANDS], WORD32 data_type, WORD32 set_idx,
                                  WORD32 start_band, WORD32 data_bands, WORD32 pair_flag,
                                  WORD32 coarse_flag, WORD32 diff_time_back_flag,
                                  WORD32 ld_mps_flag, WORD32 heaac_mps_present, WORD32 ec_flag) {
  WORD32 attach_lsb_flag = 0;
  WORD32 pcm_coding_flag = 0;
  WORD32 pilot_coding_flag = 0;
  WORD32 pilot_data[2] = {0, 0};
  WORD32 mixed_time_pair = 0, pcm_val = 0;
  WORD32 quant_levels = 0, quant_offset = 0;
  UWORD32 data = 0;
  WORD32 band_start = 0;

  WORD32 data_pair[2][MAXBANDS] = {{0}};
  WORD32 data_diff[2][MAXBANDS] = {{0}};

  WORD32 msb_state[MAXBANDS] = {0};

  WORD32* data_array[2] = {NULL, NULL};

  WORD32 diff_type[2] = {DIFF_FREQ, DIFF_FREQ};
  WORD32 cdg_scheme = HUFF_1D;
  WORD32 direction = BACKWARDS;

  if (heaac_mps_present == 1) {
    band_start = start_band;
  }

  switch (data_type) {
    case CLD:
      if (coarse_flag) {
        attach_lsb_flag = 0;
        quant_levels = 15;
        quant_offset = 7;
      } else {
        attach_lsb_flag = 0;
        quant_levels = 31;
        quant_offset = 15;
      }

      break;

    case ICC:
      if (coarse_flag) {
        attach_lsb_flag = 0;
        quant_levels = 4;
        quant_offset = 0;
      } else {
        attach_lsb_flag = 0;
        quant_levels = 8;
        quant_offset = 0;
      }

      break;

    case IPD:
      if (coarse_flag) {
        attach_lsb_flag = 0;
        quant_levels = 8;
        quant_offset = 0;
      } else {
        attach_lsb_flag = 1;
        quant_levels = 16;
        quant_offset = 0;
      }
      break;

    case CPC:
      if (coarse_flag) {
        attach_lsb_flag = 0;
        quant_levels = 26;
        quant_offset = 10;
      } else {
        attach_lsb_flag = 1;
        quant_levels = 51;
        quant_offset = 20;
      }
      break;

    default:
      break;
  }

  data = ixheaacd_read_bits_buf(it_bit_buff, 1);
  pcm_coding_flag = data;

  pilot_coding_flag = 0;

  if (heaac_mps_present == 1) {
    if (pcm_coding_flag && data_bands > 4) {
      data = ixheaacd_read_bits_buf(it_bit_buff, 1);
      pilot_coding_flag = data;
    }
  }

  if (pcm_coding_flag && !pilot_coding_flag) {
    if (pair_flag) {
      data_array[0] = data_pair[0];
      data_array[1] = data_pair[1];
      pcm_val = 2 * data_bands;
    } else {
      data_array[0] = data_pair[0];
      data_array[1] = NULL;
      pcm_val = data_bands;
    }

    ixheaacd_mps_pcm_decode(it_bit_buff, data_array[0], data_array[1],
                            quant_offset, pcm_val, quant_levels);

  } else {
    if (pair_flag) {
      data_array[0] = data_diff[0];
      data_array[1] = data_diff[1];
    } else {
      data_array[0] = data_diff[0];
      data_array[1] = NULL;
    }

    diff_type[0] = DIFF_FREQ;
    diff_type[1] = DIFF_FREQ;

    direction = BACKWARDS;

    if (!pilot_coding_flag) {
      if (pair_flag || diff_time_back_flag) {
        data = ixheaacd_read_bits_buf(it_bit_buff, 1);
        diff_type[0] = data;
      }

      if (pair_flag && ((diff_type[0] == DIFF_FREQ) || diff_time_back_flag)) {
        data = ixheaacd_read_bits_buf(it_bit_buff, 1);
        diff_type[1] = data;
      }
    }

    if (data_bands <= 0) {
      if (!ec_flag)
        return -1;
      else
        longjmp(*(it_bit_buff->xaac_jmp_buf), IA_FATAL_ERROR);
    }

    ixheaacd_huff_decode(it_bit_buff, data_array[0], data_array[1], data_type, diff_type[0],
                         diff_type[1], pilot_coding_flag, pilot_data, data_bands, &cdg_scheme,
                         ld_mps_flag);

    if (pilot_coding_flag && heaac_mps_present == 1) {
      WORD32 i;
      for (i = 0; i < data_bands; i++) {
        data_pair[0][i] = data_diff[0][i] + pilot_data[0];
      }

      if (pair_flag) {
        for (i = 0; i < data_bands; i++) {
          data_pair[1][i] = data_diff[1][i] + pilot_data[0];
        }
      }
    } else {
      if ((diff_type[0] == DIFF_TIME) || (diff_type[1] == DIFF_TIME)) {
        if (pair_flag) {
          if ((diff_type[0] == DIFF_TIME) && !diff_time_back_flag) {
            direction = FORWARDS;
          } else if (diff_type[1] == DIFF_TIME) {
            direction = BACKWARDS;
          } else {
            data = ixheaacd_read_bits_buf(it_bit_buff, 1);
            direction = data;
          }
        } else {
          direction = BACKWARDS;
        }
      }

      mixed_time_pair = (diff_type[0] != diff_type[1]) &&
                        ((cdg_scheme & PAIR_MASK) == TIME_PAIR);

      if (direction == BACKWARDS) {
        if (diff_type[0] == DIFF_FREQ) {
          ixheaacd_diff_freq_decode(data_diff[0], data_pair[0], data_bands);
        } else {
          WORD32 i;
          for (i = 0; i < data_bands; i++) {
            msb_state[i] = history[i + band_start] + quant_offset;
            if (attach_lsb_flag) {
              msb_state[i] >>= 1;
            }
          }
          ixheaacd_mps_diff_time_dec_bwd(msb_state, data_diff[0], data_pair[0],
                                         mixed_time_pair, data_bands);
        }
        if (diff_type[1] == DIFF_FREQ) {
          ixheaacd_diff_freq_decode(data_diff[1], data_pair[1], data_bands);
        } else {
          ixheaacd_mps_diff_time_dec_bwd(data_pair[0], data_diff[1], data_pair[1],
                                         mixed_time_pair, data_bands);
        }
      } else {
        ixheaacd_diff_freq_decode(data_diff[1], data_pair[1], data_bands);

        if (diff_type[0] == DIFF_FREQ) {
          ixheaacd_diff_freq_decode(data_diff[0], data_pair[0], data_bands);
        } else {
          ixheaacd_mps_diff_time_dec_fwd(data_pair[1], data_diff[0], data_pair[0],
                                         mixed_time_pair, data_bands);
        }
      }
    }
    ixheaacd_attach_lsb(it_bit_buff, data_pair[0], quant_offset,
                        attach_lsb_flag ? 1 : 0, data_bands, data_pair[0]);
    if (pair_flag) {
      ixheaacd_attach_lsb(it_bit_buff, data_pair[1], quant_offset,
                          attach_lsb_flag ? 1 : 0, data_bands, data_pair[1]);
    }
  }

  memcpy(outdata[set_idx] + band_start, data_pair[0],
         sizeof(WORD32) * data_bands);
  if (pair_flag) {
    memcpy(outdata[set_idx + 1] + band_start, data_pair[1],
           sizeof(WORD32) * data_bands);
  }

  return IA_NO_ERROR;
}

VOID ixheaacd_mps_huff_decode(ia_bit_buf_struct *it_bit_buff,
                              WORD32* out_data, WORD32 num_val) {
  WORD32 val_rcvd = 0, dummy = 0, i = 0, val = 0, len = 0;
  WORD32 rl_data[2] = {0};

  while (val_rcvd < num_val) {
    ixheaacd_mps_huff_read_2d(it_bit_buff,
                              (ia_huff_node_struct)&ixheaacd_huff_reshape_nodes,
                              rl_data, &dummy);
    val = rl_data[0];
    len = rl_data[1] + 1;
    for (i = val_rcvd; i < val_rcvd + len; i++) {
      out_data[i] = val;
    }
    val_rcvd += len;
  }

  return;
}

VOID ixheaacd_update_out_buffer(ia_heaac_mps_state_struct *pstr_mps_state,
                                WORD16 *out_buf) {
  WORD32 ch, sam;
  WORD32 num_output_channels_at = pstr_mps_state->num_output_channels_at;
  WORD32 frame_length = pstr_mps_state->frame_length;

  WORD32 *p_time_out = pstr_mps_state->array_struct->time_out;

  for (ch = 0; ch < num_output_channels_at; ch++) {
    WORD32 *time_out = p_time_out;
    for (sam = 0; sam < frame_length; sam++) {
      out_buf[sam * num_output_channels_at + ch] = (*time_out++) >> 16;
    }
    p_time_out += QBXTS;
  }

  return;
}

VOID ixheaacd_update_time_out_buffer(ia_heaac_mps_state_struct *pstr_mps_state) {
  WORD32 n;
  WORD32 up_mix_type = pstr_mps_state->up_mix_type;
  WORD32 tree_config = pstr_mps_state->tree_config;
  WORD32 lfe_gain = pstr_mps_state->lfe_gain;
  WORD32 surround_gain = pstr_mps_state->surround_gain;

  if (!pstr_mps_state->bs_config.arbitrary_tree && up_mix_type != 2 && up_mix_type != 3) {
    WORD32 frame_length = pstr_mps_state->frame_length;

    WORD32 *time_out_3 = pstr_mps_state->array_struct->time_out + QBXTSX3;
    WORD32 *time_out_4 = time_out_3 + QBXTS;
    WORD32 *time_out_5 = time_out_4 + QBXTS;
    WORD32 *time_out_6 = time_out_5 + QBXTS;
    WORD32 *time_out_7 = time_out_6 + QBXTS;

    for (n = 0; n < frame_length; n++) {
      *time_out_3 = ixheaacd_mps_mult32_shr_15(*time_out_3, lfe_gain);
      time_out_3++;
      *time_out_4 = ixheaacd_mps_mult32_shr_15(*time_out_4, surround_gain);
      time_out_4++;
      *time_out_5 = ixheaacd_mps_mult32_shr_15(*time_out_5, surround_gain);
      time_out_5++;
    }

    if (tree_config == 4 || tree_config == 6) {
      for (n = 0; n < frame_length; n++) {
        *time_out_6 = ixheaacd_mps_mult32_shr_15(*time_out_6, surround_gain);
        time_out_6++;
        *time_out_7 = ixheaacd_mps_mult32_shr_15(*time_out_7, surround_gain);
        time_out_7++;
      }
    }
  }
}

static IA_ERRORCODE ixheaacd_apply_frame(ia_heaac_mps_state_struct *pstr_mps_state,
                                         WORD32 in_time_slots,
                                         WORD32 *m_qmf_real, WORD32 *m_qmf_imag,
                                         WORD16 *out_buf) {
  IA_ERRORCODE err_code = IA_NO_ERROR;
  WORD32 ch, ts, qs;
  WORD32 *pbuf_real, *pbuf_imag, *pbuf_re, *pbuf_im;
  WORD32 *buf_real, *buf_imag;
  WORD32 *qmf_input_delay_real_2 =
      pstr_mps_state->mps_persistent_mem.qmf_input_delay_real;
  WORD32 *qmf_input_delay_imag_2 =
      pstr_mps_state->mps_persistent_mem.qmf_input_delay_imag;
  WORD32 num_input_channels = pstr_mps_state->num_input_channels;
  WORD32 qmf_bands = pstr_mps_state->qmf_bands;
  WORD32 cur_time_slot = pstr_mps_state->cur_time_slot;
  WORD32 time_slots = pstr_mps_state->time_slots;
  WORD32 qmf_input_delay_index = pstr_mps_state->qmf_input_delay_index;
  WORD32 up_mix_type = pstr_mps_state->up_mix_type;
  WORD32 residual_coding = pstr_mps_state->residual_coding;
  WORD32 arbitrary_downmix = pstr_mps_state->arbitrary_downmix;

  WORD32 *qmf_input_delay_real_1, *qmf_input_delay_imag_1;
  WORD32 *qmf_input_delay_real, *qmf_input_delay_imag;

  WORD32 *p_qmf_real = m_qmf_real;
  WORD32 *p_qmf_imag = m_qmf_imag;
  WORD32 *p_qmf_re, *p_qmf_im, *qmf_real, *qmf_imag;

  if (cur_time_slot + in_time_slots > time_slots) {
    if (pstr_mps_state->ec_flag)
      cur_time_slot = time_slots - in_time_slots;
    else
      return IA_FATAL_ERROR;
  }
  if (time_slots % HOP_SLOTS != 0) {
    if (pstr_mps_state->ec_flag)
      time_slots = time_slots - (time_slots % HOP_SLOTS);
    else
      return IA_XHEAAC_MPS_DEC_EXE_FATAL_INVALID_TIMESLOTS;
  }

  pbuf_real = pstr_mps_state->array_struct->buf_real;
  pbuf_imag = pstr_mps_state->array_struct->buf_imag;

  if (up_mix_type == 1) {
    for (ch = 0; ch < num_input_channels; ch++) {
      pbuf_re = pbuf_real;
      pbuf_im = pbuf_imag;

      p_qmf_re = p_qmf_real;
      p_qmf_im = p_qmf_imag;

      for (ts = 0; ts < in_time_slots; ts++) {
        buf_real = pbuf_re + (cur_time_slot + ts);
        buf_imag = pbuf_im + (cur_time_slot + ts);

        qmf_real = p_qmf_re;
        qmf_imag = p_qmf_im;

        for (qs = 0; qs < qmf_bands; qs++) {
          *buf_real++ = *qmf_real++;
          *buf_imag++ = *qmf_imag++;
        }
        pbuf_re += MAX_HYBRID_BANDS;
        pbuf_im += MAX_HYBRID_BANDS;

        p_qmf_re += MAX_NUM_QMF_BANDS;
        p_qmf_im += MAX_NUM_QMF_BANDS;
      }
      pbuf_real += TSXHB;
      pbuf_imag += TSXHB;

      p_qmf_real += QBXTS;
      p_qmf_imag += QBXTS;
    }
  } else {
    for (ch = 0; ch < num_input_channels; ch++) {
      WORD32 offset = ch * PCXQB;
      qmf_input_delay_index = pstr_mps_state->qmf_input_delay_index;
      qmf_input_delay_real_1 = qmf_input_delay_real_2 + offset;
      qmf_input_delay_imag_1 = qmf_input_delay_imag_2 + offset;

      pbuf_re = pbuf_real + cur_time_slot * MAX_HYBRID_BANDS;
      pbuf_im = pbuf_imag + cur_time_slot * MAX_HYBRID_BANDS;

      p_qmf_re = p_qmf_real;
      p_qmf_im = p_qmf_imag;

      for (ts = 0; ts < in_time_slots; ts++) {
        WORD32 off_set = qmf_input_delay_index * MAX_NUM_QMF_BANDS;
        qmf_input_delay_real = qmf_input_delay_real_1 + off_set;
        qmf_input_delay_imag = qmf_input_delay_imag_1 + off_set;

        buf_real = pbuf_re;
        buf_imag = pbuf_im;

        qmf_real = p_qmf_re;
        qmf_imag = p_qmf_im;

        for (qs = 0; qs < qmf_bands; qs++) {
          {
            *buf_real++ = *qmf_input_delay_real;
            *buf_imag++ = *qmf_input_delay_imag;

            *qmf_input_delay_real++ = *qmf_real++;
            *qmf_input_delay_imag++ = *qmf_imag++;
          }
        }

        qmf_input_delay_index++;

        if (qmf_input_delay_index == PC_FILTERDELAY) {
          qmf_input_delay_index = 0;
        }
        pbuf_re += MAX_HYBRID_BANDS;
        pbuf_im += MAX_HYBRID_BANDS;

        p_qmf_re += MAX_NUM_QMF_BANDS;
        p_qmf_im += MAX_NUM_QMF_BANDS;
      }
      pbuf_real += TSXHB;
      pbuf_imag += TSXHB;

      p_qmf_real += QBXTS;
      p_qmf_imag += QBXTS;
    }
    pstr_mps_state->qmf_input_delay_index = qmf_input_delay_index;
  }

  pstr_mps_state->cur_time_slot += in_time_slots;
  cur_time_slot = pstr_mps_state->cur_time_slot;

  if (pstr_mps_state->cur_time_slot < time_slots) {
    if (pstr_mps_state->ec_flag) {
      pstr_mps_state->cur_time_slot = time_slots;
    } else
      return IA_FATAL_ERROR;
  }

  pstr_mps_state->cur_time_slot = 0;

  err_code = ixheaacd_decode_frame(pstr_mps_state);
  if (err_code != IA_NO_ERROR) return err_code;

  ixheaacd_mdct_2_qmf(pstr_mps_state);

  ixheaacd_hybrid_qmf_analysis(pstr_mps_state);

  if (residual_coding || (arbitrary_downmix == 2)) {
    ixheaacd_update_buffers(pstr_mps_state);
  }

  if (up_mix_type == 1) {
    ixheaacd_apply_blind(pstr_mps_state);
  }

  ixheaacd_calc_m1m2(pstr_mps_state);

  ixheaacd_smooth_m1m2(pstr_mps_state);

  ixheaacd_mps_apply_m1(pstr_mps_state);

  ixheaacd_buffer_m1(pstr_mps_state);

  if (up_mix_type != 2) {
    if (pstr_mps_state->temp_shape_config == 2) {
      ixheaacd_pre_reshape_bb_env(pstr_mps_state);
    }
  }

  ixheaacd_create_w(pstr_mps_state);

  ixheaacd_apply_m2(pstr_mps_state);

  ixheaacd_buffer_m2(pstr_mps_state);

  if (up_mix_type != 2) {
    if (pstr_mps_state->temp_shape_config == 2) {
      ixheaacd_reshape_bb_env(pstr_mps_state);
    }
  }

  ixheaacd_tp_process(pstr_mps_state);

  ixheaacd_update_time_out_buffer(pstr_mps_state);

  ixheaacd_update_out_buffer(pstr_mps_state, out_buf);

  pstr_mps_state->parse_next_bitstream_frame = 1;

  return IA_NO_ERROR;
}

IA_ERRORCODE ixheaacd_heaac_mps_apply(ia_exhaacplus_dec_api_struct *self,
                                      WORD16 *output_buf, UWORD8 *mps_buffer,
                                      WORD32 mps_bytes) {
  ia_heaac_mps_state_struct *pstr_mps_state =
      &self->p_state_aac->heaac_mps_handle;
  IA_ERRORCODE error_code = IA_NO_ERROR;
  WORD32 n_channels, n_time_slots, qmf_bands, channel;
  ia_heaac_mps_state_struct *curr_state = pstr_mps_state;

  WORD32 *p_qmf_real = pstr_mps_state->array_struct->m_qmf_real;
  WORD32 *p_qmf_imag = pstr_mps_state->array_struct->m_qmf_imag;
  WORD32 buffer_size = mps_bytes;

  if (self->p_state_aac->heaac_mps_handle.is_first == 1) {
    self->p_state_aac->heaac_mps_handle.is_first = 1;
    if (pstr_mps_state->bytes_remaining != 0) {
      buffer_size = mps_bytes + pstr_mps_state->bytes_remaining;
      if (buffer_size > 1024) {
        return IA_FATAL_ERROR;
      }
      for (WORD32 ii = 0; ii < mps_bytes; ii++) {
        pstr_mps_state->temp_buf[ii + pstr_mps_state->bytes_remaining] =
            mps_buffer[ii];
      }

      pstr_mps_state->ptr_mps_bit_buff = ixheaacd_create_bit_buf(
          &pstr_mps_state->mps_bit_buf, (UWORD8 *)pstr_mps_state->temp_buf,
          buffer_size);
      pstr_mps_state->ptr_mps_bit_buff->xaac_jmp_buf =
          &self->p_state_aac->xaac_jmp_buf;

      pstr_mps_state->ptr_mps_bit_buff->cnt_bits += (8 * buffer_size);
    } else {
      memcpy(pstr_mps_state->temp_buf, mps_buffer, mps_bytes);
      buffer_size = mps_bytes;
      pstr_mps_state->ptr_mps_bit_buff = ixheaacd_create_bit_buf(
          &pstr_mps_state->mps_bit_buf, (UWORD8 *)mps_buffer, buffer_size);
      pstr_mps_state->ptr_mps_bit_buff->cnt_bits += (8 * buffer_size);

      pstr_mps_state->ptr_mps_bit_buff->xaac_jmp_buf =
          &self->p_state_aac->xaac_jmp_buf;
    }
  }

  if (curr_state->num_input_channels > 2 && pstr_mps_state->mps_with_sbr == 1) {
    if (pstr_mps_state->ec_flag) {
      curr_state->num_input_channels = 2;
      pstr_mps_state->frame_ok = 0;
    } else {
      return IA_FATAL_ERROR;
    }
  }

  n_channels = curr_state->num_input_channels;
  n_time_slots = curr_state->time_slots;
  qmf_bands = curr_state->qmf_bands;

  if (pstr_mps_state->mps_decode == 1) {
    if (pstr_mps_state->mps_with_sbr) {
      for (channel = 0; channel < n_channels; channel++) {
        WORD32 kk = 0;
        if (self->aac_config.ui_enh_sbr)
        {
          for (WORD32 ii = 0; ii < n_time_slots; ii++) {
            FLOAT32 *qmf_re = self->p_state_aac->str_sbr_dec_info[0]
                ->pstr_sbr_channel[channel]->str_sbr_dec.pp_qmf_buf_real[ii];
            FLOAT32 *qmf_im = self->p_state_aac->str_sbr_dec_info[0]
                ->pstr_sbr_channel[channel]->str_sbr_dec.pp_qmf_buf_imag[ii];

            for (WORD32 jj = 0; jj < qmf_bands; jj++) {
              p_qmf_real[kk] = ixheaacd_mps_mult32_shr_15(
                  curr_state->clip_protect_gain, (WORD32)(qmf_re[jj] * 1024));
              p_qmf_imag[kk++] = ixheaacd_mps_mult32_shr_15(
                  curr_state->clip_protect_gain, (WORD32)(qmf_im[jj] * 1024));
            }
          }
        }
        else
        {
          for (WORD32 ii = 0; ii < n_time_slots; ii++) {
            WORD32 *qmf_re = self->p_state_aac->str_sbr_dec_info[0]
                ->pstr_sbr_channel[channel]->str_sbr_dec.p_arr_qmf_buf_real[ii];
            WORD32 *qmf_im = self->p_state_aac->str_sbr_dec_info[0]
                ->pstr_sbr_channel[channel]->str_sbr_dec.p_arr_qmf_buf_imag[ii];

            for (WORD32 jj = 0; jj < qmf_bands; jj++) {
              p_qmf_real[kk] = ixheaacd_mps_mult32_shr_15(
                  curr_state->clip_protect_gain, qmf_re[jj] * 256);
              p_qmf_imag[kk++] = ixheaacd_mps_mult32_shr_15(
                  curr_state->clip_protect_gain, qmf_im[jj] * 256);
            }
          }
        }
        p_qmf_real += QBXTS;
        p_qmf_imag += QBXTS;
      }
    } else {
      for (channel = 0; channel < n_channels; channel++) {
        ixheaacd_calc_ana_filt_bank(pstr_mps_state, output_buf, p_qmf_real,
                                    p_qmf_imag, channel);

        p_qmf_real += QBXTS;
        p_qmf_imag += QBXTS;
      }
    }
    if (!pstr_mps_state->ec_flag && pstr_mps_state->frame_ok) {
      error_code = ixheaacd_parse_frame(pstr_mps_state);
      if (error_code != IA_NO_ERROR) return error_code;
    }

    if (!pstr_mps_state->first_frame || !pstr_mps_state->ec_flag) {
      error_code = ixheaacd_apply_frame(pstr_mps_state, n_time_slots,
                                        pstr_mps_state->array_struct->m_qmf_real,
                                        pstr_mps_state->array_struct->m_qmf_imag, output_buf);
      if (error_code != IA_NO_ERROR) return error_code;
    }
    if (error_code == 0 && pstr_mps_state->ec_flag && pstr_mps_state->frame_ok) {
      error_code = ixheaacd_parse_frame(pstr_mps_state);
      if (error_code != IA_NO_ERROR) {
        pstr_mps_state->frame_ok = 0;
      }
    }

    pstr_mps_state->i_bytes_consumed_mps =
        (WORD32)(pstr_mps_state->ptr_mps_bit_buff->ptr_read_next -
                 pstr_mps_state->ptr_mps_bit_buff->ptr_bit_buf_base);

    pstr_mps_state->bytes_remaining =
        buffer_size - pstr_mps_state->i_bytes_consumed_mps;
    if (pstr_mps_state->bytes_remaining < 0)
    {
      if (pstr_mps_state->ec_flag)
      {
        pstr_mps_state->bytes_remaining = 0;
        pstr_mps_state->frame_ok = 0;
      }
      else
      {
        return IA_FATAL_ERROR;
      }
    }

    if (pstr_mps_state->bytes_remaining != 0) {
      for (WORD32 ii = 0; ii < pstr_mps_state->bytes_remaining; ii++) {
        pstr_mps_state->temp_buf[ii] =
            pstr_mps_state->temp_buf[ii + pstr_mps_state->i_bytes_consumed_mps];
      }
    }
  }
  self->p_state_aac->heaac_mps_handle.is_first = 1;
  self->p_state_aac->heaac_mps_handle.first_frame = 0;

  return error_code;
}
