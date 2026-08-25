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
#include "ixheaac_type_def.h"
#include "ixheaac_constants.h"
#include "ixheaacd_bitbuffer.h"
#include "ixheaacd_common_rom.h"
#include "ixheaacd_sbrdecsettings.h"
#include "ixheaacd_sbr_scale.h"
#include "ixheaacd_env_extr_part.h"
#include "ixheaacd_sbr_rom.h"
#include "ixheaacd_config.h"
#include "ixheaacd_hybrid.h"
#include "ixheaacd_ps_dec.h"
#include "ixheaacd_qmf_dec.h"
#include "ixheaacd_mps_polyphase.h"
#include "ixheaacd_mps_struct_def.h"
#include "ixheaacd_mps_res_rom.h"
#include "ixheaacd_mps_aac_struct.h"
#include "ixheaacd_mps_dec.h"
#include "ixheaacd_mps_process.h"
#include "ixheaacd_lpp_tran.h"
#include "ixheaacd_env_extr.h"
#include "ixheaacd_env_calc.h"
#include "ixheaac_sbr_const.h"
#include "ixheaacd_pvc_dec.h"
#include "ixheaacd_sbr_dec.h"
#include "ixheaacd_audioobjtypes.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaacd_mps_bitdec.h"
#include "ixheaacd_mps_macro_def.h"
#include "ixheaacd_mps_get_index.h"
#include "ixheaacd_mps_basic_op.h"
#include "ixheaacd_mps_tp_process.h"
#include "ixheaacd_error_codes.h"
#define HP_SIZE (9)

#define STP_LPF_COEFF1 (0.950f)
#define STP_LPF_COEFF2 (0.450f)
#define STP_UPDATE_ENERGY_RATE (32)
#define STP_SCALE_LIMIT (2.82f)
#define STP_DAMP (0.1f)

static const FLOAT32 ixheaacd_bp[BP_SIZE] = {
  0.0000f, 0.0005f, 0.0092f, 0.0587f, 0.2580f, 0.7392f, 0.9791f,
  0.9993f, 1.0000f, 1.0000f, 1.0000f, 1.0000f, 0.9999f, 0.9984f,
  0.9908f, 0.9639f, 0.8952f, 0.7711f, 0.6127f, 0.4609f, 0.3391f,
  0.2493f, 0.1848f, 0.1387f, 0.1053f};

static const FLOAT32 ixheaacd_gf[BP_SIZE] = {
  0.f,     0.f,     0.f,     0.f,     0.f,
  0.f,     1e-008f,   8.1e-007f,   3.61e-006f,  8.41e-006f,
  1.6e-005f,   2.704e-005f, 3.969e-005f, 5.625e-005f, 7.396e-005f,
  9.801e-005f, 0.00012321f, 0.00015625f, 0.00019881f, 0.00024964f,
  0.00032041f, 0.00041209f, 0.00053824f, 0.00070756f, 0.00094249f};

extern const WORD32 ixheaacd_mps_gain_set_indx[29];

static VOID ixheaacd_mps_temp_process_scale_calc(ia_mps_dec_state_struct* self,
                                                 WORD32 ts, FLOAT32* scale) {
  FLOAT32 dir_energy;
  FLOAT32 diff_energy[2];
  FLOAT32 temp;

  WORD32 ch, n;
  WORD32 left_ch = 0, right_ch = 1;

  if (self->subband_var.init_flag == 0) {
    for (ch = 0; ch < 2; ch++) {
      self->subband_var.tp_scale_last[ch] = 1.0f;
      self->subband_var.nrg_diff_prev[ch] = 32768 * 32768;
    }

    self->subband_var.nrg_dir_prev = 32768 * 32768;
    self->subband_var.init_flag = 1;
  }

  if (self->subband_var.update_old_ener == STP_UPDATE_ENERGY_RATE) {
    self->subband_var.update_old_ener = 1;
    self->subband_var.nrg_dir_prev = self->subband_var.nrg_dir;
    for (ch = 0; ch < self->out_ch_count; ch++)
      self->subband_var.nrg_diff_prev[ch] = self->subband_var.nrg_diff[ch];
  } else
    self->subband_var.update_old_ener++;

  dir_energy = 0;

  for (n = 6; n < BP_SIZE; n++) {
    FLOAT32 dir_left_re = self->hyb_dir_out[left_ch][ts][n + 7].re;
    FLOAT32 dir_right_re = self->hyb_dir_out[right_ch][ts][n + 7].re;
    FLOAT32 dir_left_im = self->hyb_dir_out[left_ch][ts][n + 7].im;
    FLOAT32 dir_right_im = self->hyb_dir_out[right_ch][ts][n + 7].im;

    temp = ((dir_left_re + dir_right_re) * (dir_left_re + dir_right_re)) +
           ((dir_left_im + dir_right_im) * (dir_left_im + dir_right_im));
    dir_energy += temp * ixheaacd_bp[n] * ixheaacd_bp[n] * ixheaacd_gf[n] *
                  ixheaacd_gf[n];
  }

  self->subband_var.nrg_dir =
      (FLOAT32)(STP_LPF_COEFF1 * self->subband_var.nrg_dir +
                (1.0 - STP_LPF_COEFF1) * dir_energy);

  dir_energy /= (self->subband_var.nrg_dir_prev + ABS_THR);

  for (ch = 0; ch < self->out_ch_count; ch++) {
    diff_energy[ch] = 0;
    for (n = 6; n < BP_SIZE; n++) {
      FLOAT32 diff_re = self->hyb_diff_out[ch][ts][n + 7].re;
      FLOAT32 diff_im = self->hyb_diff_out[ch][ts][n + 7].im;

      temp = (diff_re * diff_re) + (diff_im * diff_im);
      diff_energy[ch] += temp * ixheaacd_bp[n] * ixheaacd_bp[n] *
                         ixheaacd_gf[n] * ixheaacd_gf[n];
    }

    self->subband_var.nrg_diff[ch] =
        (FLOAT32)(STP_LPF_COEFF1 * self->subband_var.nrg_diff[ch] +
                  (1.0 - STP_LPF_COEFF1) * diff_energy[ch]);
    diff_energy[ch] /= (self->subband_var.nrg_diff_prev[ch] + ABS_THR);
  }

  scale[left_ch] = (FLOAT32)sqrt((dir_energy) / (diff_energy[left_ch] + 1e-9));
  scale[right_ch] =
      (FLOAT32)sqrt((dir_energy) / (diff_energy[right_ch] + 1e-9));

  for (ch = 0; ch < self->out_ch_count; ch++) {
    scale[ch] = STP_DAMP + (1 - STP_DAMP) * scale[ch];
  }

  for (ch = 0; ch < self->out_ch_count; ch++) {
    scale[ch] =
        min(max(scale[ch], (FLOAT32)(1.0 / STP_SCALE_LIMIT)), STP_SCALE_LIMIT);
  }

  for (ch = 0; ch < self->out_ch_count; ch++) {
    scale[ch] =
        (FLOAT32)(STP_LPF_COEFF2 * scale[ch] +
                  (1.0 - STP_LPF_COEFF2) * self->subband_var.tp_scale_last[ch]);
    self->subband_var.tp_scale_last[ch] = scale[ch];
  }
}

static VOID ixheaacd_mps_subbandtp(ia_mps_dec_state_struct* self, WORD32 ts) {
  FLOAT32 scale[2];
  WORD32 ch, n;
  WORD32 no_scaling;
  FLOAT32 temp;
  const WORD32 ixheaacd_hybrid_to_qmf_map[] = {0, 0, 0, 0, 0, 0, 1, 1, 2, 2};
  const WORD32 ixheaacd_hybrid_to_qmf_map_ldmps[] = {0, 1, 2};
  const WORD32* ptr_ixheaacd_hybrid_to_qmf_map;
  WORD32 loop_counter = 0;

  if (self->ldmps_config.ldmps_present_flag) {
    ptr_ixheaacd_hybrid_to_qmf_map = ixheaacd_hybrid_to_qmf_map_ldmps;
    loop_counter = 3;
  } else {
    ptr_ixheaacd_hybrid_to_qmf_map = ixheaacd_hybrid_to_qmf_map;
    loop_counter = 10;
  }

  ixheaacd_mps_temp_process_scale_calc(self, ts, scale);

  for (ch = 0; ch < self->out_ch_count; ch++) {
    no_scaling = 1;

    if ((self->config->bs_temp_shape_config == 1) ||
        (self->config->bs_temp_shape_config == 2))
      no_scaling = !self->temp_shape_enable_ch_stp[ch];

    if (no_scaling == 1) {
      for (n = 0; n < self->hyb_band_count_max; n++) {
        self->hyb_dir_out[ch][ts][n].re += self->hyb_diff_out[ch][ts][n].re;
        self->hyb_dir_out[ch][ts][n].im += self->hyb_diff_out[ch][ts][n].im;
      }
    } else {
      if (self->ldmps_config.ldmps_present_flag) {
        for (n = 0; n < 3; n++) {
          temp = (FLOAT32)(scale[ch] *
                           ixheaacd_bp[ptr_ixheaacd_hybrid_to_qmf_map[n]]);
          self->hyb_dir_out[ch][ts][n].re +=
              (self->hyb_diff_out[ch][ts][n].re * temp);
          self->hyb_dir_out[ch][ts][n].im +=
              (self->hyb_diff_out[ch][ts][n].im * temp);
        }
      } else {
        for (n = 0; n < loop_counter; n++) {
          temp = (FLOAT32)(scale[ch] *
                           ixheaacd_bp[ptr_ixheaacd_hybrid_to_qmf_map[n]]);
          self->hyb_dir_out[ch][ts][n].re +=
              (self->hyb_diff_out[ch][ts][n].re * temp);
          self->hyb_dir_out[ch][ts][n].im +=
              (self->hyb_diff_out[ch][ts][n].im * temp);
        }
      }
      for (n = 7; n < HP_SIZE - 3 + 10; n++) {
        temp = (FLOAT32)(scale[ch] * ixheaacd_bp[n + 3 - 10]);
        self->hyb_dir_out[ch][ts][n].re +=
            (self->hyb_diff_out[ch][ts][n].re * temp);
        self->hyb_dir_out[ch][ts][n].im +=
            (self->hyb_diff_out[ch][ts][n].im * temp);
      }
      for (; n < self->hyb_band_count_max; n++) {
        temp = (FLOAT32)(scale[ch]);
        self->hyb_dir_out[ch][ts][n].re +=
            (self->hyb_diff_out[ch][ts][n].re * temp);
        self->hyb_dir_out[ch][ts][n].im +=
            (self->hyb_diff_out[ch][ts][n].im * temp);
      }
    }
  }
}

WORD32 ixheaacd_mps_temp_process(ia_mps_dec_state_struct* self) {
  WORD32 ch, ts, hyb;
  WORD32 err = 0;
  ia_sbr_frame_info_data_struct* ptr_frame_data =
      (ia_sbr_frame_info_data_struct*)self->p_sbr_frame[0];
  if (self->res_bands != 28) {
    if (self->config->bs_temp_shape_config == 1) {
      WORD32 dif_s = ((self->res_bands == 0)
                          ? 0
                          : ixheaacd_mps_gain_set_indx[self->res_bands]);
      for (ch = 0; ch < self->out_ch_count; ch++) {
        for (ts = 0; ts < self->time_slots; ts++) {
          for (hyb = dif_s; hyb < HYBRID_BAND_BORDER; hyb++) {
            self->hyb_dir_out[ch][ts][hyb].re +=
                self->hyb_diff_out[ch][ts][hyb].re;
            self->hyb_dir_out[ch][ts][hyb].im +=
                self->hyb_diff_out[ch][ts][hyb].im;
            self->hyb_diff_out[ch][ts][hyb].re = 0;
            self->hyb_diff_out[ch][ts][hyb].im = 0;
          }
        }
      }

      for (ts = 0; ts < self->time_slots; ts++)
        ixheaacd_mps_subbandtp(self, ts);

    } else {
      WORD32 dif_s = ((self->res_bands == 0)
                          ? 0
                          : ixheaacd_mps_gain_set_indx[self->res_bands]);
      for (ch = 0; ch < self->out_ch_count; ch++) {
        for (ts = 0; ts < self->time_slots; ts++) {
          for (hyb = dif_s; hyb < self->hyb_band_count_max; hyb++) {
            self->hyb_dir_out[ch][ts][hyb].re +=
                self->hyb_diff_out[ch][ts][hyb].re;
            self->hyb_dir_out[ch][ts][hyb].im +=
                self->hyb_diff_out[ch][ts][hyb].im;
          }
        }
      }
    }
  }

  ixheaacd_mps_qmf_hyb_synthesis(self);

  if (self->ldmps_config.ldmps_present_flag != 1) {
    for (ch = 0; ch < self->out_ch_count; ch++) {
      err =
          ixheaacd_sbr_dec_from_mps(&self->qmf_out_dir[ch][0][0].re, self->p_sbr_dec[ch],
                                    self->p_sbr_frame[ch], self->p_sbr_header[ch], self->ec_flag);
      if (err) return err;
    }
  }

  if (self->object_type == AOT_ER_AAC_ELD || self->object_type == AOT_ER_AAC_LD)
    self->synth_count = self->hyb_band_count[0];
  else
  {
    if (ptr_frame_data->mps_sbr_flag) {
      self->synth_count =
        ptr_frame_data->pstr_sbr_header->pstr_freq_band_data->sub_band_end;
    }
    else {
      self->synth_count = self->band_count[0];
    }
  }

  ixheaacd_mps_synt_calc(self);
  return err;
}

static VOID ixheaacd_subband_tp(ia_heaac_mps_state_struct *pstr_mps_state, WORD32 ts) {
  ia_mps_dec_tp_process_tables_struct *tp_process_table_ptr =
      pstr_mps_state->ia_mps_dec_mps_table.tp_process_table_ptr;
  const WORD32 *sqrt_tab =
      pstr_mps_state->ia_mps_dec_mps_table.common_table_ptr->sqrt_tab;
  ia_mps_dec_subband_tp_params_struct *sub_band_tp =
      pstr_mps_state->mps_persistent_mem.sub_band_params;
  ia_mps_dec_reuse_array_struct *p_array_struct = pstr_mps_state->array_struct;

  WORD32 temp_1, temp_2;
  WORD16 qtemp1, qtemp2;
  WORD32 *qmf_output_real_dry;
  WORD32 *qmf_output_imag_dry;
  WORD32 *qmf_output_real_wet;
  WORD32 *qmf_output_imag_wet;

  WORD32 *dmx_real;
  WORD32 *dmx_imag;
  WORD32 *dry_ener;
  WORD32 *wet_ener;
  WORD16 *q_dry_ener;
  WORD16 *q_wet_ener;

  WORD32 *p_buffer_real, *p_buffer_imag, *p_buffer_re, *p_buffer_im;
  WORD32 *p_buf_real, *p_buf_imag, *p_buf_re, *p_buf_im;
  WORD32 *scale;
  WORD16 *q_scale;
  WORD32 damp, one_minus_damp;
  WORD32 temp;

  WORD32 *prev_tp_scale = sub_band_tp->prev_tp_scale;

  WORD32 *old_wet_ener = sub_band_tp->old_wet_ener;
  WORD16 *q_old_wet_ener = sub_band_tp->q_old_wet_ener;

  WORD32 *run_wet_ener = sub_band_tp->run_wet_ener;
  WORD16 *q_run_wet_ener = sub_band_tp->q_run_wet_ener;

  WORD32 *old_dry_ener = sub_band_tp->old_dry_ener;
  WORD16 *q_old_dry_ener = sub_band_tp->q_old_dry_ener;

  WORD32 *run_dry_ener = sub_band_tp->run_dry_ener;
  WORD16 *q_run_dry_ener = sub_band_tp->q_run_dry_ener;

  WORD32 *hyb_output_real_dry, *hyb_output_imag_dry;

  WORD32 *p_hyb_out_dry_real, *p_hyb_out_dry_imag;

  WORD32 ch, n, no_scaling, i, k = 0, offset;
  WORD32 i_lf = 0, i_rf = 0, i_c = 0, i_lfe = 0, i_ls = 0, i_rs = 0, i_al = 0, i_ar = 0;

  WORD32 loop_counter = 0;

  WORD32 num_input_channels = pstr_mps_state->num_input_channels;
  WORD32 num_output_channels = pstr_mps_state->num_output_channels;
  WORD32 hybrid_bands = pstr_mps_state->hybrid_bands;

  WORD32 tree_config = pstr_mps_state->tree_config;
  WORD64 dmx_real64 = 0;
  WORD64 dmx_imag64 = 0;

  dry_ener = pstr_mps_state->mps_scratch_mem_v;
  q_dry_ener = (WORD16 *)pstr_mps_state->mps_scratch_mem_v +
               IXHEAAC_GET_SIZE_ALIGNED_TYPE(INPUT_CHX2, sizeof(*q_dry_ener), BYTE_ALIGN_8);

  wet_ener =
      dry_ener + IXHEAAC_GET_SIZE_ALIGNED_TYPE(INPUT_CHX1_5, sizeof(*wet_ener), BYTE_ALIGN_8);
  q_wet_ener = q_dry_ener +
               IXHEAAC_GET_SIZE_ALIGNED_TYPE(IN_CH_2XOUT_CH, sizeof(*q_wet_ener), BYTE_ALIGN_8);

  scale = wet_ener + IXHEAAC_GET_SIZE_ALIGNED_TYPE(OUTPUT_CHX1_5, sizeof(*scale), BYTE_ALIGN_8);
  q_scale =
      q_wet_ener + IXHEAAC_GET_SIZE_ALIGNED_TYPE(OUTPUT_CHX3, sizeof(*q_scale), BYTE_ALIGN_8);

  dmx_real =
      scale + IXHEAAC_GET_SIZE_ALIGNED_TYPE(OUTPUT_CHX1_5, sizeof(*dmx_real), BYTE_ALIGN_8);
  dmx_imag =
      dmx_real + IXHEAAC_GET_SIZE_ALIGNED_TYPE(IN_CHXBP_SIZE, sizeof(*dmx_imag), BYTE_ALIGN_8);

  qmf_output_real_dry = dmx_imag + IXHEAAC_GET_SIZE_ALIGNED_TYPE(
                                       IN_CHXBP_SIZE, sizeof(*qmf_output_real_dry), BYTE_ALIGN_8);

  qmf_output_imag_dry =
      qmf_output_real_dry +
      IXHEAAC_GET_SIZE_ALIGNED_TYPE(OUT_CHXQB, sizeof(*qmf_output_imag_dry), BYTE_ALIGN_8);

  qmf_output_real_wet =
      qmf_output_imag_dry +
      IXHEAAC_GET_SIZE_ALIGNED_TYPE(OUT_CHXQB, sizeof(*qmf_output_real_wet), BYTE_ALIGN_8);

  qmf_output_imag_wet =
      qmf_output_real_wet +
      IXHEAAC_GET_SIZE_ALIGNED_TYPE(OUT_CHXQB, sizeof(*qmf_output_imag_wet), BYTE_ALIGN_8);

  if (sub_band_tp->update_old_ener == STP_UPDATE_ENERGY_RATE) {
    sub_band_tp->update_old_ener = 1;
    for (ch = 0; ch < num_input_channels; ch++) {
      old_dry_ener[ch] = run_dry_ener[ch];
      q_old_dry_ener[ch] = q_run_dry_ener[ch];
    }
    for (ch = 0; ch < num_output_channels; ch++) {
      old_wet_ener[ch] = run_wet_ener[ch];
      q_old_wet_ener[ch] = q_run_wet_ener[ch];
    }
  } else
    sub_band_tp->update_old_ener++;

  for (ch = 0; ch < MAX_OUTPUT_CHANNELS_MPS; ch++) {
    scale[ch] = ONE_IN_Q15;
    q_scale[ch] = 15;
  }

  switch (tree_config) {
    case TREE_5151:
      i_lf = 0;
      i_rf = 1;
      i_c = 2;
      i_lfe = 3;
      i_ls = 4;
      i_rs = 5;
      loop_counter = 6;
      break;
    case TREE_5152:
      i_lf = 0;
      i_rf = 2;
      i_c = 4;
      i_lfe = 5;
      i_ls = 1;
      i_rs = 3;
      loop_counter = 5;
      break;
    case TREE_525:
      i_lf = 0;
      i_rf = 2;
      i_c = 4;
      i_lfe = 5;
      i_ls = 1;
      i_rs = 3;
      loop_counter = 4;
      break;
    case TREE_7271:
    case TREE_7272:
    case TREE_7572:
      i_lf = 0;
      i_rf = 3;
      i_c = 6;
      i_lfe = 7;
      i_ls = 2;
      i_rs = 5;
      i_al = 1;
      i_ar = 4;
      loop_counter = 6;
      break;
    case TREE_7571:
      i_lf = 0;
      i_rf = 3;
      i_c = 6;
      i_lfe = 7;
      i_ls = 2;
      i_rs = 5;
      i_al = 1;
      i_ar = 4;
      loop_counter = 5;
      break;
    default:
      break;
  }

  offset = ts * MAX_HYBRID_BANDS;
  p_buffer_real = p_array_struct->buf_real + offset + HYBRID_BAND_BORDER;
  p_buffer_imag = p_array_struct->buf_imag + offset + HYBRID_BAND_BORDER;

  for (ch = 0; ch < num_output_channels; ch++) {
    p_buffer_re = p_buffer_real;
    p_buffer_im = p_buffer_imag;

    for (i = QMF_OUT_START_IDX; i < BP_SIZE; i++) {
      *qmf_output_real_wet++ = *p_buffer_re++;
      *qmf_output_imag_wet++ = *p_buffer_im++;
    }
    p_buffer_real += TSXHB;
    p_buffer_imag += TSXHB;
  }
  i = QMF_OUT_OFFSET * num_output_channels;
  qmf_output_real_wet -= i;
  qmf_output_imag_wet -= i;

  p_buffer_re = qmf_output_real_dry;
  p_buffer_im = qmf_output_imag_dry;

  hyb_output_real_dry =
      p_array_struct->hyb_output_real_dry + ts * MAX_HYBRID_BANDS + 6;
  hyb_output_imag_dry =
      p_array_struct->hyb_output_imag_dry + ts * MAX_HYBRID_BANDS + 6;

  for (ch = 0; ch < loop_counter; ch++) {
    *p_buffer_re++ = ixheaac_add32_sat(hyb_output_real_dry[0], hyb_output_real_dry[1]);
    *p_buffer_im++ = ixheaac_add32_sat(hyb_output_imag_dry[0], hyb_output_imag_dry[1]);

    hyb_output_real_dry += TSXHB;
    hyb_output_imag_dry += TSXHB;
  }

  hyb_output_real_dry =
      p_array_struct->hyb_output_real_dry + ts * MAX_HYBRID_BANDS + 8;
  hyb_output_imag_dry =
      p_array_struct->hyb_output_imag_dry + ts * MAX_HYBRID_BANDS + 8;

  for (ch = 0; ch < loop_counter; ch++) {
    *p_buffer_re++ = ixheaac_add32_sat(hyb_output_real_dry[0], hyb_output_real_dry[1]);
    *p_buffer_im++ = ixheaac_add32_sat(hyb_output_imag_dry[0], hyb_output_imag_dry[1]);

    hyb_output_real_dry += TSXHB;
    hyb_output_imag_dry += TSXHB;
  }

  p_hyb_out_dry_real =
      p_array_struct->hyb_output_real_dry + ts * MAX_HYBRID_BANDS + 10;
  p_hyb_out_dry_imag =
      p_array_struct->hyb_output_imag_dry + ts * MAX_HYBRID_BANDS + 10;

  for (i = 3; i < BP_SIZE; i++) {
    hyb_output_real_dry = p_hyb_out_dry_real;
    hyb_output_imag_dry = p_hyb_out_dry_imag;

    for (ch = 0; ch < loop_counter; ch++) {
      *p_buffer_re++ = *hyb_output_real_dry;
      *p_buffer_im++ = *hyb_output_imag_dry;

      hyb_output_real_dry += TSXHB;
      hyb_output_imag_dry += TSXHB;
    }
    p_hyb_out_dry_real++;
    p_hyb_out_dry_imag++;
  }

  for (n = 1; n < BP_SIZE; n++) {
    switch (tree_config) {
      case TREE_5151:
        dmx_real64 =
            ixheaac_mult32x32in64(*qmf_output_real_dry++, tp_process_table_ptr->bpxgf[n]);
        dmx_real64 = ixheaac_mac32x32in64(dmx_real64, *qmf_output_real_dry++,
                                          tp_process_table_ptr->bpxgf[n]);
        dmx_real64 = ixheaac_mac32x32in64(dmx_real64, *qmf_output_real_dry++,
                                          tp_process_table_ptr->bpxgf[n]);	    
        qmf_output_real_dry++;
        dmx_real64 = ixheaac_mac32x32in64(dmx_real64, *qmf_output_real_dry++,
                                          tp_process_table_ptr->bpxgf[n]);
        dmx_real64 = ixheaac_mac32x32in64(dmx_real64, *qmf_output_real_dry++,
                                          tp_process_table_ptr->bpxgf[n]);
        *dmx_real = (WORD32)(dmx_real64 >> 30);
        dmx_real++;
        dmx_real++;

        break;
      case TREE_5152:
        dmx_real64 =
            ixheaac_mult32x32in64(*qmf_output_real_dry++, tp_process_table_ptr->bpxgf[n]);
        dmx_real64 = ixheaac_mac32x32in64(dmx_real64, *qmf_output_real_dry++,
                                          tp_process_table_ptr->bpxgf[n]);
        dmx_real64 = ixheaac_mac32x32in64(dmx_real64, *qmf_output_real_dry++,
                                          tp_process_table_ptr->bpxgf[n]);
        dmx_real64 = ixheaac_mac32x32in64(dmx_real64, *qmf_output_real_dry++,
                                          tp_process_table_ptr->bpxgf[n]);
        dmx_real64 = ixheaac_mac32x32in64(dmx_real64, *qmf_output_real_dry++,
                                          tp_process_table_ptr->bpxgf[n]);
        *dmx_real = (WORD32)(dmx_real64 >> 30);
        dmx_real++;
        dmx_real++;

        break;
      case TREE_525:
        dmx_real64 =
            ixheaac_mult32x32in64(*qmf_output_real_dry++, tp_process_table_ptr->bpxgf[n]);
        dmx_real64 = ixheaac_mac32x32in64(dmx_real64, *qmf_output_real_dry++,
                                          tp_process_table_ptr->bpxgf[n]);
        *dmx_real = (WORD32)(dmx_real64 >> 30);
        dmx_real++;

        dmx_real64 =
            ixheaac_mult32x32in64(*qmf_output_real_dry++, tp_process_table_ptr->bpxgf[n]);
        dmx_real64 = ixheaac_mac32x32in64(dmx_real64, *qmf_output_real_dry++,
                                          tp_process_table_ptr->bpxgf[n]);
        *dmx_real = (WORD32)(dmx_real64 >> 30);
        dmx_real++;

        break;
      case TREE_7271:
      case TREE_7272:
        dmx_real64 =
            ixheaac_mult32x32in64(*qmf_output_real_dry++, tp_process_table_ptr->bpxgf[n]);
        dmx_real64 = ixheaac_mac32x32in64(dmx_real64, *qmf_output_real_dry++,
                                          tp_process_table_ptr->bpxgf[n]);
        dmx_real64 = ixheaac_mac32x32in64(dmx_real64, *qmf_output_real_dry++,
                                          tp_process_table_ptr->bpxgf[n]);
        *dmx_real = (WORD32)(dmx_real64 >> 30);
        dmx_real++;

        dmx_real64 =
            ixheaac_mult32x32in64(*qmf_output_real_dry++, tp_process_table_ptr->bpxgf[n]);
        dmx_real64 = ixheaac_mac32x32in64(dmx_real64, *qmf_output_real_dry++,
                                          tp_process_table_ptr->bpxgf[n]);
        dmx_real64 = ixheaac_mac32x32in64(dmx_real64, *qmf_output_real_dry++,
                                          tp_process_table_ptr->bpxgf[n]);
        *dmx_real = (WORD32)(dmx_real64 >> 30);
        dmx_real++;

        break;
      case TREE_7571:

        dmx_real64 =
            ixheaac_mult32x32in64(*qmf_output_real_dry++, tp_process_table_ptr->bpxgf[n]);
        dmx_real64 = ixheaac_mac32x32in64(dmx_real64, *qmf_output_real_dry++,
                                          tp_process_table_ptr->bpxgf[n]);
        qmf_output_real_dry++;
        *dmx_real = (WORD32)(dmx_real64 >> 30);
		
        dmx_real++;

        dmx_real64 =
            ixheaac_mult32x32in64(*qmf_output_real_dry++, tp_process_table_ptr->bpxgf[n]);
        dmx_real64 = ixheaac_mac32x32in64(dmx_real64, *qmf_output_real_dry++,
                                          tp_process_table_ptr->bpxgf[n]);
        *dmx_real = (WORD32)(dmx_real64 >> 30);
        dmx_real++;

        break;
      case TREE_7572:
        qmf_output_real_dry++;
        dmx_real64 =
            ixheaac_mult32x32in64(*qmf_output_real_dry++, tp_process_table_ptr->bpxgf[n]);
        dmx_real64 = ixheaac_mac32x32in64(dmx_real64, *qmf_output_real_dry++,
                                          tp_process_table_ptr->bpxgf[n]);
        qmf_output_real_dry++;
        *dmx_real = (WORD32)(dmx_real64 >> 30);
        dmx_real++;

        dmx_real64 =
            ixheaac_mult32x32in64(*qmf_output_real_dry++, tp_process_table_ptr->bpxgf[n]);
        dmx_real64 = ixheaac_mac32x32in64(dmx_real64, *qmf_output_real_dry++,
                                          tp_process_table_ptr->bpxgf[n]);
        *dmx_real = (WORD32)(dmx_real64 >> 30);
        dmx_real++;

        break;
      default:
        break;
    }
  }
  dmx_real -= DMX_OFFSET;

  for (n = 1; n < BP_SIZE; n++) {
    switch (tree_config) {
      case TREE_5151:
        dmx_imag64 =
            ixheaac_mult32x32in64(*qmf_output_imag_dry++, tp_process_table_ptr->bpxgf[n]);
        dmx_imag64 = ixheaac_mac32x32in64(dmx_imag64, *qmf_output_imag_dry++,
                                          tp_process_table_ptr->bpxgf[n]);
        dmx_imag64 = ixheaac_mac32x32in64(dmx_imag64, *qmf_output_imag_dry++,
                                          tp_process_table_ptr->bpxgf[n]);	    
        qmf_output_imag_dry++;
        dmx_imag64 = ixheaac_mac32x32in64(dmx_imag64, *qmf_output_imag_dry++,
                                          tp_process_table_ptr->bpxgf[n]);
        dmx_imag64 = ixheaac_mac32x32in64(dmx_imag64, *qmf_output_imag_dry++,
                                          tp_process_table_ptr->bpxgf[n]);
        *dmx_imag = (WORD32)(dmx_imag64 >> 30);
		dmx_imag++;
		dmx_imag++;

        break;
      case TREE_5152:

        dmx_imag64 =
            ixheaac_mult32x32in64(*qmf_output_imag_dry++, tp_process_table_ptr->bpxgf[n]);
        dmx_imag64 = ixheaac_mac32x32in64(dmx_imag64, *qmf_output_imag_dry++,
                                          tp_process_table_ptr->bpxgf[n]);
        dmx_imag64 = ixheaac_mac32x32in64(dmx_imag64, *qmf_output_imag_dry++,
                                          tp_process_table_ptr->bpxgf[n]);
        dmx_imag64 = ixheaac_mac32x32in64(dmx_imag64, *qmf_output_imag_dry++,
                                          tp_process_table_ptr->bpxgf[n]);
        dmx_imag64 = ixheaac_mac32x32in64(dmx_imag64, *qmf_output_imag_dry++,
                                          tp_process_table_ptr->bpxgf[n]);
        *dmx_imag = (WORD32)(dmx_imag64 >> 30);
        dmx_imag++;
        dmx_imag++;
		
        break;
      case TREE_525:
        dmx_imag64 =
            ixheaac_mult32x32in64(*qmf_output_imag_dry++, tp_process_table_ptr->bpxgf[n]);
        dmx_imag64 = ixheaac_mac32x32in64(dmx_imag64, *qmf_output_imag_dry++,
                                          tp_process_table_ptr->bpxgf[n]);
        *dmx_imag = (WORD32)(dmx_imag64 >> 30);
        dmx_imag++;

        dmx_imag64 =
            ixheaac_mult32x32in64(*qmf_output_imag_dry++, tp_process_table_ptr->bpxgf[n]);
        dmx_imag64 = ixheaac_mac32x32in64(dmx_imag64, *qmf_output_imag_dry++,
                                          tp_process_table_ptr->bpxgf[n]);
        *dmx_imag = (WORD32)(dmx_imag64 >> 30);
        dmx_imag++;
        break;
      case TREE_7271:
      case TREE_7272:
        dmx_imag64 =
            ixheaac_mult32x32in64(*qmf_output_imag_dry++, tp_process_table_ptr->bpxgf[n]);
        dmx_imag64 = ixheaac_mac32x32in64(dmx_imag64, *qmf_output_imag_dry++,
                                          tp_process_table_ptr->bpxgf[n]);
        dmx_imag64 = ixheaac_mac32x32in64(dmx_imag64, *qmf_output_imag_dry++,
                                          tp_process_table_ptr->bpxgf[n]);
        *dmx_imag = (WORD32)(dmx_imag64 >> 30);
        dmx_imag++;

        dmx_imag64 =
            ixheaac_mult32x32in64(*qmf_output_imag_dry++, tp_process_table_ptr->bpxgf[n]);
        dmx_imag64 = ixheaac_mac32x32in64(dmx_imag64, *qmf_output_imag_dry++,
                                          tp_process_table_ptr->bpxgf[n]);
        dmx_imag64 = ixheaac_mac32x32in64(dmx_imag64, *qmf_output_imag_dry++,
                                          tp_process_table_ptr->bpxgf[n]);
        *dmx_imag = (WORD32)(dmx_imag64 >> 30);
        dmx_imag++;

        break;
      case TREE_7571:
        dmx_imag64 =
            ixheaac_mult32x32in64(*qmf_output_imag_dry++, tp_process_table_ptr->bpxgf[n]);
        dmx_imag64 = ixheaac_mac32x32in64(dmx_imag64, *qmf_output_imag_dry++,
                                          tp_process_table_ptr->bpxgf[n]);
        qmf_output_imag_dry++;
        *dmx_imag = (WORD32)(dmx_imag64 >> 30);
		
        dmx_imag++;

        dmx_imag64 =
            ixheaac_mult32x32in64(*qmf_output_imag_dry++, tp_process_table_ptr->bpxgf[n]);
        dmx_imag64 = ixheaac_mac32x32in64(dmx_imag64, *qmf_output_imag_dry++,
                                          tp_process_table_ptr->bpxgf[n]);
        *dmx_imag = (WORD32)(dmx_imag64 >> 30);
        dmx_imag++;


        break;
      case TREE_7572:
        qmf_output_imag_dry++;
        dmx_imag64 =
            ixheaac_mult32x32in64(*qmf_output_imag_dry++, tp_process_table_ptr->bpxgf[n]);
        dmx_imag64 = ixheaac_mac32x32in64(dmx_imag64, *qmf_output_imag_dry++,
                                          tp_process_table_ptr->bpxgf[n]);
        qmf_output_imag_dry++;
        *dmx_imag = (WORD32)(dmx_imag64 >> 30);
        dmx_imag++;

        dmx_imag64 =
            ixheaac_mult32x32in64(*qmf_output_imag_dry++, tp_process_table_ptr->bpxgf[n]);
        dmx_imag64 = ixheaac_mac32x32in64(dmx_imag64, *qmf_output_imag_dry++,
                                          tp_process_table_ptr->bpxgf[n]);
        *dmx_imag = (WORD32)(dmx_imag64 >> 30);
        dmx_imag++;

        break;
      default:
        break;
    }
  }
  dmx_imag -= DMX_OFFSET;

  for (ch = 0; ch < min(2, num_input_channels); ch++) {
    dry_ener[ch] = 0;
    q_dry_ener[ch] = 15;

    for (n = 1; n < BP_SIZE; n++) {
      qtemp1 = 10;
      temp_1 = ixheaacd_mps_mult32(*dmx_real, *dmx_real, &qtemp1, qtemp1);
      dmx_real += 2;
      dry_ener[ch] =
          ixheaacd_mps_add32(dry_ener[ch], temp_1, &q_dry_ener[ch], qtemp1);

      qtemp1 = 10;
      temp_1 = ixheaacd_mps_mult32(*dmx_imag, *dmx_imag, &qtemp1, qtemp1);
      dmx_imag += 2;
      dry_ener[ch] =
          ixheaacd_mps_add32(dry_ener[ch], temp_1, &q_dry_ener[ch], qtemp1);
    }
    dmx_real -= DMX_OFFSET_MINUS_ONE;
    dmx_imag -= DMX_OFFSET_MINUS_ONE;

    temp_1 = ixheaacd_mps_mult32_shr_15(run_dry_ener[ch], STP_LPF_COEFF1_FIX);

    temp_2 = ONE_IN_Q15 - STP_LPF_COEFF1_FIX;
    temp_2 = ixheaacd_mps_mult32_shr_15(temp_2, dry_ener[ch]);

    run_dry_ener[ch] = ixheaacd_mps_add32(temp_1, temp_2, &(q_run_dry_ener[ch]),
                                          q_dry_ener[ch]);

    qtemp1 = q_old_dry_ener[ch];
    temp_1 = ixheaacd_mps_add32(old_dry_ener[ch], ABS_THR_FIX, &qtemp1, 15);
    ;

    dry_ener[ch] = ixheaacd_mps_div_32(dry_ener[ch], temp_1, &qtemp2);
    q_dry_ener[ch] = qtemp2 + q_dry_ener[ch] - qtemp1;
  }

  for (ch = 0; ch < num_output_channels; ch++) {
    if (ch == i_lfe) continue;
    if ((tree_config >= TREE_525) && (ch == i_c)) continue;
    if ((tree_config == TREE_7571) && ((ch == i_ls) || (ch == i_rs))) continue;
    if ((tree_config == TREE_7572) && ((ch == i_lf) || (ch == i_rf))) continue;

    wet_ener[ch] = 0;
    q_wet_ener[ch] = 15;

    wet_ener[ch] = 0;
    q_wet_ener[ch] = 15;
    for (n = FIVE; n < BP_SIZE; n++) {
      qtemp1 = 10;
      temp_1 = ixheaacd_mps_mult32(*qmf_output_real_wet, *qmf_output_real_wet,
                                   &qtemp1, qtemp1);
      qmf_output_real_wet++;

      qtemp2 = 10;
      temp_2 = ixheaacd_mps_mult32(*qmf_output_imag_wet, *qmf_output_imag_wet,
                                   &qtemp2, qtemp2);
      qmf_output_imag_wet++;

      temp_1 = ixheaacd_mps_add32(temp_1, temp_2, &qtemp1, qtemp2);

      temp_1 = ixheaacd_mps_mult32(temp_1, tp_process_table_ptr->bp2xgf2[n],
                                   &qtemp1, 57);

      wet_ener[ch] =
          ixheaacd_mps_add32(wet_ener[ch], temp_1, &q_wet_ener[ch], qtemp1);
    }
    temp_1 = ixheaacd_mps_mult32_shr_15(run_wet_ener[ch], STP_LPF_COEFF1_FIX);

    temp_2 = ONE_IN_Q15 - STP_LPF_COEFF1_FIX;

    temp_2 = ixheaacd_mps_mult32_shr_15(temp_2, wet_ener[ch]);

    run_wet_ener[ch] =
        ixheaacd_mps_add32(temp_1, temp_2, &q_run_wet_ener[ch], q_wet_ener[ch]);

    qtemp1 = q_old_wet_ener[ch];
    temp_1 = ixheaacd_mps_add32(old_wet_ener[ch], ABS_THR_FIX, &qtemp1, 15);

    wet_ener[ch] = ixheaacd_mps_div_32(wet_ener[ch], temp_1, &qtemp2);
    q_wet_ener[ch] = qtemp2 + q_wet_ener[ch] - qtemp1;
  }

  damp = POINT_ONE_Q15;
  one_minus_damp = POINT_NINE_Q15;
  switch (tree_config) {
    case TREE_5151:
    case TREE_5152:
      if (wet_ener[i_lf] != 0) {
        scale[i_lf] = ixheaacd_mps_div_32(dry_ener[0], wet_ener[i_lf], &qtemp2);
        q_scale[i_lf] = qtemp2 + q_dry_ener[0] - q_wet_ener[i_lf];
      } else {
        temp_1 = ixheaac_norm32(dry_ener[0]);
        scale[i_lf] = dry_ener[0] << temp_1;
        q_scale[i_lf] = q_dry_ener[0] + temp_1 - 30;
      }
      scale[i_lf] = ixheaacd_mps_sqrt(scale[i_lf], &(q_scale[i_lf]), sqrt_tab);

      if (wet_ener[i_rf] != 0) {
        scale[i_rf] = ixheaacd_mps_div_32(dry_ener[0], wet_ener[i_rf], &qtemp2);
        q_scale[i_rf] = qtemp2 + q_dry_ener[0] - q_wet_ener[i_rf];
      } else {
        temp_1 = ixheaac_norm32(dry_ener[0]);
        scale[i_rf] = dry_ener[0] << temp_1;
        q_scale[i_rf] = q_dry_ener[0] + temp_1 - 30;
      }
      scale[i_rf] = ixheaacd_mps_sqrt(scale[i_rf], &(q_scale[i_rf]), sqrt_tab);

      if (wet_ener[i_c] != 0) {
        scale[i_c] = ixheaacd_mps_div_32(dry_ener[0], wet_ener[i_c], &qtemp2);
        q_scale[i_c] = qtemp2 + q_dry_ener[0] - q_wet_ener[i_c];
      } else {
        temp_1 = ixheaac_norm32(dry_ener[0]);
        scale[i_c] = dry_ener[0] << temp_1;
        q_scale[i_c] = q_dry_ener[0] + temp_1 - 30;
      }
      scale[i_c] = ixheaacd_mps_sqrt(scale[i_c], &(q_scale[i_c]), sqrt_tab);

      if (wet_ener[i_ls] != 0) {
        scale[i_ls] = ixheaacd_mps_div_32(dry_ener[0], wet_ener[i_ls], &qtemp2);
        q_scale[i_ls] = qtemp2 + q_dry_ener[0] - q_wet_ener[i_ls];
      } else {
        temp_1 = ixheaac_norm32(dry_ener[0]);
        scale[i_ls] = dry_ener[0] << temp_1;
        q_scale[i_ls] = q_dry_ener[0] + temp_1 - 30;
      }
      scale[i_ls] = ixheaacd_mps_sqrt(scale[i_ls], &(q_scale[i_ls]), sqrt_tab);

      if (wet_ener[i_rs] != 0) {
        scale[i_rs] = ixheaacd_mps_div_32(dry_ener[0], wet_ener[i_rs], &qtemp2);
        q_scale[i_rs] = qtemp2 + q_dry_ener[0] - q_wet_ener[i_rs];
      } else {
        temp_1 = ixheaac_norm32(dry_ener[0]);
        scale[i_rs] = dry_ener[0] << temp_1;
        q_scale[i_rs] = q_dry_ener[0] + temp_1 - 30;
      }
      scale[i_rs] = ixheaacd_mps_sqrt(scale[i_rs], &(q_scale[i_rs]), sqrt_tab);

      for (ch = 0; ch < 6; ch++) {
        if (ch == 3 && tree_config == 0) continue;
        temp_1 = ixheaacd_mps_mult32_shr_15(scale[ch], one_minus_damp);
        scale[ch] = ixheaacd_mps_add32(temp_1, damp, &(q_scale[ch]), 15);
        scale[ch] = ixheaacd_mps_convert_to_qn(scale[ch], q_scale[ch], 15);
        if (scale[ch] > STP_SCALE_LIMIT_FIX) scale[ch] = STP_SCALE_LIMIT_FIX;
        if (scale[ch] < ONE_BY_STP_SCALE_LIMIT)
          scale[ch] = ONE_BY_STP_SCALE_LIMIT;
      }

      break;
    case TREE_525:
      if (wet_ener[i_lf] != 0) {
        scale[i_lf] = ixheaacd_mps_div_32(dry_ener[0], wet_ener[i_lf], &qtemp2);
        q_scale[i_lf] = qtemp2 + q_dry_ener[0] - q_wet_ener[i_lf];
      } else {
        temp_1 = ixheaac_norm32(dry_ener[0]);
        scale[i_lf] = dry_ener[0] << temp_1;
        q_scale[i_lf] = q_dry_ener[0] + temp_1 - 30;
      }
      scale[i_lf] = ixheaacd_mps_sqrt(scale[i_lf], &(q_scale[i_lf]), sqrt_tab);

      if (wet_ener[i_rf] != 0) {
        scale[i_rf] = ixheaacd_mps_div_32(dry_ener[1], wet_ener[i_rf], &qtemp2);
        q_scale[i_rf] = qtemp2 + q_dry_ener[1] - q_wet_ener[i_rf];
      } else {
        temp_1 = ixheaac_norm32(dry_ener[1]);
        scale[i_rf] = dry_ener[1] << temp_1;
        q_scale[i_rf] = q_dry_ener[1] + temp_1 - 30;
      }
      scale[i_rf] = ixheaacd_mps_sqrt(scale[i_rf], &(q_scale[i_rf]), sqrt_tab);

      if (wet_ener[i_ls] != 0) {
        scale[i_ls] = ixheaacd_mps_div_32(dry_ener[0], wet_ener[i_ls], &qtemp2);
        q_scale[i_ls] = qtemp2 + q_dry_ener[0] - q_wet_ener[i_ls];
      } else {
        temp_1 = ixheaac_norm32(dry_ener[0]);
        scale[i_ls] = dry_ener[0] << temp_1;
        q_scale[i_ls] = q_dry_ener[0] + temp_1 - 30;
      }
      scale[i_ls] = ixheaacd_mps_sqrt(scale[i_ls], &(q_scale[i_ls]), sqrt_tab);

      if (wet_ener[i_rs] != 0) {
        scale[i_rs] = ixheaacd_mps_div_32(dry_ener[1], wet_ener[i_rs], &qtemp2);
        q_scale[i_rs] = qtemp2 + q_dry_ener[1] - q_wet_ener[i_rs];
      } else {
        temp_1 = ixheaac_norm32(dry_ener[1]);
        scale[i_rs] = dry_ener[1] << temp_1;
        q_scale[i_rs] = q_dry_ener[1] + temp_1 - 30;
      }
      scale[i_rs] = ixheaacd_mps_sqrt(scale[i_rs], &(q_scale[i_rs]), sqrt_tab);

      for (ch = 0; ch < 4; ch++) {
        temp_1 = ixheaacd_mps_mult32_shr_15(scale[ch], one_minus_damp);
        scale[ch] = ixheaacd_mps_add32(temp_1, damp, &(q_scale[ch]), 15);
        scale[ch] = ixheaacd_mps_convert_to_qn(scale[ch], q_scale[ch], 15);
        if (scale[ch] > STP_SCALE_LIMIT_FIX) scale[ch] = STP_SCALE_LIMIT_FIX;
        if (scale[ch] < ONE_BY_STP_SCALE_LIMIT)
          scale[ch] = ONE_BY_STP_SCALE_LIMIT;
      }
      break;
    case TREE_7271:
    case TREE_7272:
      if (wet_ener[i_lf] != 0) {
        scale[i_lf] = ixheaacd_mps_div_32(dry_ener[0], wet_ener[i_lf], &qtemp2);
        q_scale[i_lf] = qtemp2 + q_dry_ener[0] - q_wet_ener[i_lf];
      } else {
        temp_1 = ixheaac_norm32(dry_ener[0]);
        scale[i_lf] = dry_ener[0] << temp_1;
        q_scale[i_lf] = q_dry_ener[0] + temp_1 - 30;
      }
      scale[i_lf] = ixheaacd_mps_sqrt(scale[i_lf], &(q_scale[i_lf]), sqrt_tab);

      if (wet_ener[i_rf] != 0) {
        scale[i_rf] = ixheaacd_mps_div_32(dry_ener[1], wet_ener[i_rf], &qtemp2);
        q_scale[i_rf] = qtemp2 + q_dry_ener[1] - q_wet_ener[i_rf];
      } else {
        temp_1 = ixheaac_norm32(dry_ener[1]);
        scale[i_rf] = dry_ener[1] << temp_1;
        q_scale[i_rf] = q_dry_ener[1] + temp_1 - 30;
      }
      scale[i_rf] = ixheaacd_mps_sqrt(scale[i_rf], &(q_scale[i_rf]), sqrt_tab);

      if (wet_ener[i_ls] != 0) {
        scale[i_ls] = ixheaacd_mps_div_32(dry_ener[0], wet_ener[i_ls], &qtemp2);
        q_scale[i_ls] = qtemp2 + q_dry_ener[0] - q_wet_ener[i_ls];
      } else {
        temp_1 = ixheaac_norm32(dry_ener[0]);
        scale[i_ls] = dry_ener[0] << temp_1;
        q_scale[i_ls] = q_dry_ener[0] + temp_1 - 30;
      }
      scale[i_ls] = ixheaacd_mps_sqrt(scale[i_ls], &(q_scale[i_ls]), sqrt_tab);

      if (wet_ener[i_rs] != 0) {
        scale[i_rs] = ixheaacd_mps_div_32(dry_ener[1], wet_ener[i_rs], &qtemp2);
        q_scale[i_rs] = qtemp2 + q_dry_ener[1] - q_wet_ener[i_rs];
      } else {
        temp_1 = ixheaac_norm32(dry_ener[1]);
        scale[i_rs] = dry_ener[1] << temp_1;
        q_scale[i_rs] = q_dry_ener[1] + temp_1 - 30;
      }
      scale[i_rs] = ixheaacd_mps_sqrt(scale[i_rs], &(q_scale[i_rs]), sqrt_tab);

      if (wet_ener[i_al] != 0) {
        scale[i_al] = ixheaacd_mps_div_32(dry_ener[0], wet_ener[i_al], &qtemp2);
        q_scale[i_al] = qtemp2 + q_dry_ener[0] - q_wet_ener[i_al];
      } else {
        temp_1 = ixheaac_norm32(dry_ener[0]);
        scale[i_al] = dry_ener[0] << temp_1;
        q_scale[i_al] = q_dry_ener[0] + temp_1 - 30;
      }
      scale[i_al] = ixheaacd_mps_sqrt(scale[i_al], &(q_scale[i_al]), sqrt_tab);

      if (wet_ener[i_ar] != 0) {
        scale[i_ar] = ixheaacd_mps_div_32(dry_ener[1], wet_ener[i_ar], &qtemp2);
        q_scale[i_ar] = qtemp2 + q_dry_ener[1] - q_wet_ener[i_ar];
      } else {
        temp_1 = ixheaac_norm32(dry_ener[1]);
        scale[i_ar] = dry_ener[1] << temp_1;
        q_scale[i_ar] = q_dry_ener[1] + temp_1 - 30;
      }
      scale[i_ar] = ixheaacd_mps_sqrt(scale[i_ar], &(q_scale[i_ar]), sqrt_tab);

      for (ch = 0; ch < 6; ch++) {
        temp_1 = ixheaacd_mps_mult32_shr_15(scale[ch], one_minus_damp);
        scale[ch] = ixheaacd_mps_add32(temp_1, damp, &(q_scale[ch]), 15);
        scale[ch] = ixheaacd_mps_convert_to_qn(scale[ch], q_scale[ch], 15);
        if (scale[ch] > STP_SCALE_LIMIT_FIX) scale[ch] = STP_SCALE_LIMIT_FIX;
        if (scale[ch] < ONE_BY_STP_SCALE_LIMIT)
          scale[ch] = ONE_BY_STP_SCALE_LIMIT;
      }

      break;
    case TREE_7571:
      if (wet_ener[i_lf] != 0) {
        scale[i_lf] = ixheaacd_mps_div_32(dry_ener[0], wet_ener[i_lf], &qtemp2);
        q_scale[i_lf] = qtemp2 + q_dry_ener[0] - q_wet_ener[i_lf];
      } else {
        temp_1 = ixheaac_norm32(dry_ener[0]);
        scale[i_lf] = dry_ener[0] << temp_1;
        q_scale[i_lf] = q_dry_ener[0] + temp_1 - 30;
      }
      scale[i_lf] = ixheaacd_mps_sqrt(scale[i_lf], &(q_scale[i_lf]), sqrt_tab);

      if (wet_ener[i_rf] != 0) {
        scale[i_rf] = ixheaacd_mps_div_32(dry_ener[1], wet_ener[i_rf], &qtemp2);
        q_scale[i_rf] = qtemp2 + q_dry_ener[1] - q_wet_ener[i_rf];
      } else {
        temp_1 = ixheaac_norm32(dry_ener[1]);
        scale[i_rf] = dry_ener[1] << temp_1;
        q_scale[i_rf] = q_dry_ener[1] + temp_1 - 30;
      }
      scale[i_rf] = ixheaacd_mps_sqrt(scale[i_rf], &(q_scale[i_rf]), sqrt_tab);

      if (wet_ener[i_al] != 0) {
        scale[i_al] = ixheaacd_mps_div_32(dry_ener[0], wet_ener[i_al], &qtemp2);
        q_scale[i_al] = qtemp2 + q_dry_ener[0] - q_wet_ener[i_al];
      } else {
        temp_1 = ixheaac_norm32(dry_ener[0]);
        scale[i_al] = dry_ener[0] << temp_1;
        q_scale[i_al] = q_dry_ener[0] + temp_1 - 30;
      }
      scale[i_al] = ixheaacd_mps_sqrt(scale[i_al], &(q_scale[i_al]), sqrt_tab);

      if (wet_ener[i_ar] != 0) {
        scale[i_ar] = ixheaacd_mps_div_32(dry_ener[1], wet_ener[i_ar], &qtemp2);
        q_scale[i_ar] = qtemp2 + q_dry_ener[1] - q_wet_ener[i_ar];
      } else {
        temp_1 = ixheaac_norm32(dry_ener[1]);
        scale[i_ar] = dry_ener[1] << temp_1;
        q_scale[i_ar] = q_dry_ener[1] + temp_1 - 30;
      }
      scale[i_ar] = ixheaacd_mps_sqrt(scale[i_ar], &(q_scale[i_ar]), sqrt_tab);
      for (ch = 0; ch < FIVE; ch++) {
        if (ch == 2) continue;
        temp_1 = ixheaacd_mps_mult32_shr_15(scale[ch], one_minus_damp);
        scale[ch] = ixheaacd_mps_add32(temp_1, damp, &(q_scale[ch]), 15);
        scale[ch] = ixheaacd_mps_convert_to_qn(scale[ch], q_scale[ch], 15);
        if (scale[ch] > STP_SCALE_LIMIT_FIX) scale[ch] = STP_SCALE_LIMIT_FIX;
        if (scale[ch] < ONE_BY_STP_SCALE_LIMIT)
          scale[ch] = ONE_BY_STP_SCALE_LIMIT;
      }

      break;
    case TREE_7572:
      if (wet_ener[i_ls] != 0) {
        scale[i_ls] = ixheaacd_mps_div_32(dry_ener[0], wet_ener[i_ls], &qtemp2);
        q_scale[i_ls] = qtemp2 + q_dry_ener[0] - q_wet_ener[i_ls];
      } else {
        temp_1 = ixheaac_norm32(dry_ener[0]);
        scale[i_ls] = dry_ener[0] << temp_1;
        q_scale[i_ls] = q_dry_ener[0] + temp_1 - 30;
      }
      scale[i_ls] = ixheaacd_mps_sqrt(scale[i_ls], &(q_scale[i_ls]), sqrt_tab);

      if (wet_ener[i_rs] != 0) {
        scale[i_rs] = ixheaacd_mps_div_32(dry_ener[1], wet_ener[i_rs], &qtemp2);
        q_scale[i_rs] = qtemp2 + q_dry_ener[1] - q_wet_ener[i_rs];
      } else {
        temp_1 = ixheaac_norm32(dry_ener[1]);
        scale[i_rs] = dry_ener[1] << temp_1;
        q_scale[i_rs] = q_dry_ener[1] + temp_1 - 30;
      }
      scale[i_rs] = ixheaacd_mps_sqrt(scale[i_rs], &(q_scale[i_rs]), sqrt_tab);

      if (wet_ener[i_al] != 0) {
        scale[i_al] = ixheaacd_mps_div_32(dry_ener[0], wet_ener[i_al], &qtemp2);
        q_scale[i_al] = qtemp2 + q_dry_ener[0] - q_wet_ener[i_al];
      } else {
        temp_1 = ixheaac_norm32(dry_ener[0]);
        scale[i_al] = dry_ener[0] << temp_1;
        q_scale[i_al] = q_dry_ener[0] + temp_1 - 30;
      }
      scale[i_al] = ixheaacd_mps_sqrt(scale[i_al], &(q_scale[i_al]), sqrt_tab);

      if (wet_ener[i_ar] != 0) {
        scale[i_ar] = ixheaacd_mps_div_32(dry_ener[1], wet_ener[i_ar], &qtemp2);
        q_scale[i_ar] = qtemp2 + q_dry_ener[1] - q_wet_ener[i_ar];
      } else {
        temp_1 = ixheaac_norm32(dry_ener[1]);
        scale[i_ar] = dry_ener[1] << temp_1;
        q_scale[i_ar] = q_dry_ener[1] + temp_1 - 30;
      }
      scale[i_ar] = ixheaacd_mps_sqrt(scale[i_ar], &(q_scale[i_ar]), sqrt_tab);
      for (ch = 0; ch < 6; ch++) {
        if (ch == 3 || ch == 0) continue;
        temp_1 = ixheaacd_mps_mult32_shr_15(scale[ch], one_minus_damp);
        scale[ch] = ixheaacd_mps_add32(temp_1, damp, &(q_scale[ch]), 15);
        scale[ch] = ixheaacd_mps_convert_to_qn(scale[ch], q_scale[ch], 15);
        if (scale[ch] > STP_SCALE_LIMIT_FIX) scale[ch] = STP_SCALE_LIMIT_FIX;
        if (scale[ch] < ONE_BY_STP_SCALE_LIMIT)
          scale[ch] = ONE_BY_STP_SCALE_LIMIT;
      }

      break;
    default:
      break;
  }

  for (ch = 0; ch < num_output_channels; ch++) {
    temp_1 = ixheaacd_mps_mult32_shr_15(STP_LPF_COEFF2_FIX, scale[ch]);
    temp_2 =
        ixheaacd_mps_mult32_shr_15(ONE_MINUS_STP_LPF_COEFF2, prev_tp_scale[ch]);
    scale[ch] = ixheaac_add32_sat(temp_1, temp_2);
    prev_tp_scale[ch] = scale[ch];
  }

  offset = ts * MAX_HYBRID_BANDS;
  p_buffer_real = p_array_struct->buf_real + offset + HYBRID_BAND_BORDER;
  p_buffer_imag = p_array_struct->buf_imag + offset + HYBRID_BAND_BORDER;

  p_buf_real = p_array_struct->buffer_real + offset + FIVE;
  p_buf_imag = p_array_struct->buffer_imag + offset + FIVE;

  p_hyb_out_dry_real = p_array_struct->hyb_output_real_dry +
                       ts * MAX_HYBRID_BANDS + HYBRID_BAND_BORDER;
  p_hyb_out_dry_imag = p_array_struct->hyb_output_imag_dry +
                       ts * MAX_HYBRID_BANDS + HYBRID_BAND_BORDER;

  for (ch = 0; ch < num_output_channels; ch++) {
    no_scaling = 1;

    ixheaacd_get_ch_idx(pstr_mps_state, ch, &i);
    if (i != -1) {
      no_scaling = !pstr_mps_state->aux_struct->temp_shape_enable_channel_stp[i];
    }
    p_buffer_re = p_buffer_real;
    p_buffer_im = p_buffer_imag;

    p_buf_re = p_buf_real;
    p_buf_im = p_buf_imag;

    hyb_output_real_dry = p_hyb_out_dry_real;
    hyb_output_imag_dry = p_hyb_out_dry_imag;

    if (no_scaling == 1) {
      for (n = HYBRID_BAND_BORDER; n < (HP_SIZE + QMF_TO_HYB_OFFSET); n++) {
        *p_buf_re++ = ixheaac_add32_sat(*hyb_output_real_dry++, *p_buffer_re++);

        *p_buf_im++ = ixheaac_add32_sat(*hyb_output_imag_dry++, *p_buffer_im++);
      }

      for (; n < hybrid_bands; n++, k++) {
        temp = (no_scaling ? ONE_IN_Q15 : scale[ch]);

        *p_buf_re++ = ixheaac_add32_sat(*hyb_output_real_dry++, *p_buffer_re++);

        *p_buf_im++ = ixheaac_add32_sat(*hyb_output_imag_dry++, *p_buffer_im++);
      }
    } else {
      for (n = HYBRID_BAND_BORDER; n < (HP_SIZE + QMF_TO_HYB_OFFSET); n++) {
        temp = ixheaacd_mps_mult32_shr_30(
            scale[ch], tp_process_table_ptr->bp[n - QMF_TO_HYB_OFFSET]);

        *p_buf_re++ = ixheaac_add32_sat(*hyb_output_real_dry++, 
		              ixheaacd_mps_mult32_shr_15(temp, *p_buffer_re));
        p_buffer_re++;

        *p_buf_im++ = ixheaac_add32_sat(*hyb_output_imag_dry++, 
                      ixheaacd_mps_mult32_shr_15(temp, *p_buffer_im));
        p_buffer_im++;
      }

      for (; n < hybrid_bands; n++, k++) {
        temp = (no_scaling ? ONE_IN_Q15 : scale[ch]);

        *p_buf_re++ = ixheaac_add32_sat(*hyb_output_real_dry++, 
                      ixheaacd_mps_mult32_shr_15(temp, *p_buffer_re));
        p_buffer_re++;

        *p_buf_im++ = ixheaac_add32_sat(*hyb_output_imag_dry++, 
                      ixheaacd_mps_mult32_shr_15(temp, *p_buffer_im));
        p_buffer_im++;
      }
    }

    p_buffer_real += TSXHB;
    p_buffer_imag += TSXHB;

    p_buf_real += TSXHB;
    p_buf_imag += TSXHB;

    p_hyb_out_dry_real += TSXHB;
    p_hyb_out_dry_imag += TSXHB;
  }

  return;
}

VOID ixheaacd_tp_process(ia_heaac_mps_state_struct *pstr_mps_state) {
  WORD32 ch, ts, hyb, n;
  WORD32 temp, temp_1, temp_2;
  ia_mps_dec_synthesis_interface *syn = pstr_mps_state->syn;
  WORD32 *hyb_output_real_wet, *hyb_output_imag_wet;
  WORD32 *hyb_output_real_dry, *hyb_output_imag_dry;

  WORD32 *p_buffer_real, *p_buffer_imag, *p_buffer_re, *p_buffer_im;
  WORD32 *p_buf_real, *p_buf_imag, *p_buf_re, *p_buf_im;
  WORD32 *buf_real, *buf_imag;

  WORD32 num_output_channels = pstr_mps_state->num_output_channels;
  WORD32 time_slots = pstr_mps_state->time_slots;
  WORD32 qmf_bands = pstr_mps_state->qmf_bands;
  WORD32 num_output_channels_at = pstr_mps_state->num_output_channels_at;
  WORD32 tree_config = pstr_mps_state->tree_config;
  WORD32 up_mix_type = pstr_mps_state->up_mix_type;
  WORD32 tp_hyb_band_border = pstr_mps_state->tp_hyb_band_border;

  ia_mps_dec_reuse_array_struct *p_array_struct = pstr_mps_state->array_struct;
  WORD32 *p_hyb_out_dry_real = p_array_struct->hyb_output_real_dry;
  WORD32 *p_hyb_out_dry_imag = p_array_struct->hyb_output_imag_dry;
  WORD32 *p_hyb_out_dry_re, *p_hyb_out_dry_im;

  WORD32 *p_time_out;

  p_buffer_real = p_array_struct->buf_real;
  p_buffer_imag = p_array_struct->buf_imag;

  p_buf_real = p_array_struct->buffer_real;
  p_buf_imag = p_array_struct->buffer_imag;

  if (!pstr_mps_state->scaling_enable) {
    for (ch = 0; ch < num_output_channels; ch++) {
      p_buffer_re = p_buffer_real;
      p_buffer_im = p_buffer_imag;

      p_buf_re = p_buf_real;
      p_buf_im = p_buf_imag;

      p_hyb_out_dry_re = p_hyb_out_dry_real;
      p_hyb_out_dry_im = p_hyb_out_dry_imag;

      for (ts = 0; ts < time_slots; ts++) {
        hyb_output_real_wet = p_buffer_re;
        hyb_output_imag_wet = p_buffer_im;
        hyb_output_real_dry = p_hyb_out_dry_re;
        hyb_output_imag_dry = p_hyb_out_dry_im;

        buf_real = p_buf_re;
        buf_imag = p_buf_im;

        temp_1 = ixheaac_add32_sat(*hyb_output_real_dry++, *hyb_output_real_wet++);
        temp_2 = ixheaac_add32_sat(*hyb_output_imag_dry++, *hyb_output_imag_wet++);
        for (n = 1; n < 6; n++) {
          temp_1 = ixheaac_add32_sat(temp_1, *hyb_output_real_dry++);
          temp_1 = ixheaac_add32_sat(temp_1, *hyb_output_real_wet++);
          temp_2 = ixheaac_add32_sat(temp_2, *hyb_output_imag_dry++);
          temp_2 = ixheaac_add32_sat(temp_2, *hyb_output_imag_wet++);
        }

        *buf_real++ = temp_1;
        *buf_imag++ = temp_2;

        temp = ixheaac_add32_sat(*hyb_output_real_dry++, *hyb_output_real_wet++);
        *buf_real = ixheaac_add32_sat(temp, *hyb_output_real_dry++);
        *buf_real = ixheaac_add32_sat(*buf_real, *hyb_output_real_wet++);
        temp = ixheaac_add32_sat(*hyb_output_imag_dry++, *hyb_output_imag_wet++);
        *buf_imag = ixheaac_add32_sat(temp, *hyb_output_imag_dry++);
        *buf_imag = ixheaac_add32_sat(*buf_imag, *hyb_output_imag_wet++);

        buf_real++;
        buf_imag++;

        temp = ixheaac_add32_sat(*hyb_output_real_dry++, *hyb_output_real_wet++);
        *buf_real = ixheaac_add32_sat(temp, *hyb_output_real_dry++);
        *buf_real = ixheaac_add32_sat(*buf_real, *hyb_output_real_wet++);
        temp = ixheaac_add32_sat(*hyb_output_imag_dry++, *hyb_output_imag_wet++);
        *buf_imag = ixheaac_add32_sat(temp, *hyb_output_imag_dry++);
        *buf_imag = ixheaac_add32_sat(*buf_imag, *hyb_output_imag_wet++);

        buf_real++;
        buf_imag++;

        for (n = 0; n < qmf_bands; n++) {
          *buf_real++ = ixheaac_add32_sat(*hyb_output_real_dry++, *hyb_output_real_wet++);
          *buf_imag++ = ixheaac_add32_sat(*hyb_output_imag_dry++, *hyb_output_imag_wet++);
        }

        p_buffer_re += MAX_HYBRID_BANDS;
        p_buffer_im += MAX_HYBRID_BANDS;

        p_buf_re += MAX_HYBRID_BANDS;
        p_buf_im += MAX_HYBRID_BANDS;

        p_hyb_out_dry_re += MAX_HYBRID_BANDS;
        p_hyb_out_dry_im += MAX_HYBRID_BANDS;
      }
      p_buffer_real += TSXHB;
      p_buffer_imag += TSXHB;

      p_buf_real += TSXHB;
      p_buf_imag += TSXHB;

      p_hyb_out_dry_real += TSXHB;
      p_hyb_out_dry_imag += TSXHB;
    }
  } else {
    for (ch = 0; ch < num_output_channels; ch++) {
      p_buffer_re = p_buffer_real;
      p_buffer_im = p_buffer_imag;

      p_buf_re = p_buf_real;
      p_buf_im = p_buf_imag;

      p_hyb_out_dry_re = p_hyb_out_dry_real;
      p_hyb_out_dry_im = p_hyb_out_dry_imag;

      for (ts = 0; ts < time_slots; ts++) {
        hyb_output_real_wet = p_buffer_re;
        hyb_output_imag_wet = p_buffer_im;
        hyb_output_real_dry = p_hyb_out_dry_re;
        hyb_output_imag_dry = p_hyb_out_dry_im;

        buf_real = p_buf_re;
        buf_imag = p_buf_im;

        temp_1 = ixheaac_add32_sat(*hyb_output_real_dry++, *hyb_output_real_wet++);
        temp_2 = ixheaac_add32_sat(*hyb_output_imag_dry++, *hyb_output_imag_wet++);
        for (n = 1; n < 6; n++) {
          temp_1 = ixheaac_add32_sat(temp_1, *hyb_output_real_dry++);
          temp_1 = ixheaac_add32_sat(temp_1, *hyb_output_real_wet++);
          temp_2 = ixheaac_add32_sat(temp_2, *hyb_output_imag_dry++);
          temp_2 = ixheaac_add32_sat(temp_2, *hyb_output_imag_wet++);
        }

        *buf_real++ = temp_1;
        *buf_imag++ = temp_2;

        *buf_real = ixheaac_add32_sat(*hyb_output_real_dry++, *hyb_output_real_wet++);
        *buf_real = ixheaac_add32_sat(*buf_real, *hyb_output_real_dry++);
        *buf_real = ixheaac_add32_sat(*buf_real, *hyb_output_real_wet++);

        *buf_imag = ixheaac_add32_sat(*hyb_output_imag_dry++, *hyb_output_imag_wet++);
        *buf_imag = ixheaac_add32_sat(*buf_imag, *hyb_output_imag_dry++);
        *buf_imag = ixheaac_add32_sat(*buf_imag, *hyb_output_imag_wet++);

        buf_real++;
        buf_imag++;

        *buf_real = ixheaac_add32_sat(*hyb_output_real_dry++, *hyb_output_real_wet++);
        *buf_real = ixheaac_add32_sat(*buf_real, *hyb_output_real_dry++);
        *buf_real = ixheaac_add32_sat(*buf_real, *hyb_output_real_wet++);
        *buf_imag = ixheaac_add32_sat(*hyb_output_imag_dry++, *hyb_output_imag_wet++);
        *buf_imag = ixheaac_add32_sat(*buf_imag, *hyb_output_imag_dry++);
        *buf_imag = ixheaac_add32_sat(*buf_imag, *hyb_output_imag_wet++);

        buf_real++;
        buf_imag++;

        for (hyb = 3; hyb < tp_hyb_band_border - QMF_TO_HYB_OFFSET; hyb++) {
          *buf_real++ = ixheaac_add32_sat(*hyb_output_real_dry++, *hyb_output_real_wet++);
          *buf_imag++ = ixheaac_add32_sat(*hyb_output_imag_dry++, *hyb_output_imag_wet++);
        }
        p_buffer_re += MAX_HYBRID_BANDS;
        p_buffer_im += MAX_HYBRID_BANDS;

        p_buf_re += MAX_HYBRID_BANDS;
        p_buf_im += MAX_HYBRID_BANDS;

        p_hyb_out_dry_re += MAX_HYBRID_BANDS;
        p_hyb_out_dry_im += MAX_HYBRID_BANDS;
      }
      p_buffer_real += TSXHB;
      p_buffer_imag += TSXHB;

      p_buf_real += TSXHB;
      p_buf_imag += TSXHB;

      p_hyb_out_dry_real += TSXHB;
      p_hyb_out_dry_imag += TSXHB;
    }

    for (ts = 0; ts < time_slots; ts++) {
      ixheaacd_subband_tp(pstr_mps_state, ts);
    }
  }

  if ((!pstr_mps_state->bs_config.arbitrary_tree) &&
      ((up_mix_type != 2) && (up_mix_type != 3))) {
    WORD32 *time_out_5xxx =
        pstr_mps_state->ia_mps_dec_mps_table.tp_process_table_ptr->time_out_idx_5xxx;
    WORD32 *time_out_7xxx =
        pstr_mps_state->ia_mps_dec_mps_table.tp_process_table_ptr->time_out_idx_7xxx;

    p_buf_real = p_array_struct->buffer_real;
    p_buf_imag = p_array_struct->buffer_imag;

    for (ch = 0; ch < num_output_channels_at; ch++) {
      WORD32 tempch = 0;
      switch (tree_config) {
        case TREE_5151:
          tempch = ch;
          break;
        case TREE_5152:
        case TREE_525:
          tempch = time_out_5xxx[ch];
          break;
        case TREE_7271:
        case TREE_7272:
        case TREE_7571:
        case TREE_7572:
          tempch = time_out_7xxx[ch];
          break;
        default:
          break;
      }
      p_time_out = p_array_struct->time_out + tempch * QBXTS;
      syn->syn_filter_bank(&pstr_mps_state->syn_qmf_bank, p_buf_real, p_buf_imag,
                           p_time_out, ch, qmf_bands, time_slots,
                           pstr_mps_state->ia_mps_dec_mps_table.qmf_table_ptr);

      p_buf_real += TSXHB;
      p_buf_imag += TSXHB;
    }
  } else {
    p_time_out = p_array_struct->time_out;
    for (ch = 0; ch < num_output_channels_at; ch++) {
      syn->syn_filter_bank(&pstr_mps_state->syn_qmf_bank, p_buf_real, p_buf_imag,
                           p_time_out, ch, qmf_bands, time_slots,
                           pstr_mps_state->ia_mps_dec_mps_table.qmf_table_ptr);

      p_buf_real += TSXHB;
      p_buf_imag += TSXHB;
      p_time_out += QBXTS;
    }
  }

  return;
}
