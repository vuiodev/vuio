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
#include <stdio.h>
#include <string.h>

#include <math.h>

#include "ixheaacd_sbr_common.h"
#include "ixheaac_type_def.h"

#include "ixheaac_constants.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops16.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaac_basic_ops.h"
#include "ixheaacd_bitbuffer.h"

#include "ixheaac_basic_op.h"
#include "ixheaacd_intrinsics.h"
#include "ixheaac_sbr_const.h"

#include "ixheaacd_defines.h"

#include "ixheaacd_aac_rom.h"

#include "ixheaacd_definitions.h"

#include "ixheaacd_error_codes.h"

#include "ixheaacd_pulsedata.h"

#include "ixheaacd_pns.h"
#include "ixheaacd_drc_data_struct.h"

#include "ixheaacd_lt_predict.h"
#include "ixheaacd_cnst.h"
#include "ixheaacd_ec_defines.h"
#include "ixheaacd_ec_struct_def.h"
#include "ixheaacd_channelinfo.h"
#include "ixheaacd_drc_dec.h"
#include "ixheaacd_sbrdecoder.h"
#include "ixheaacd_block.h"
#include "ixheaacd_channel.h"

#include "ixheaacd_sbr_payload.h"
#include "ixheaacd_common_rom.h"

#include "ixheaacd_sbrdecsettings.h"
#include "ixheaacd_sbr_scale.h"
#include "ixheaacd_env_extr_part.h"
#include "ixheaacd_sbr_rom.h"
#include "ixheaacd_stereo.h"
#include "ixheaacd_lpp_tran.h"
#include "ixheaacd_hybrid.h"
#include "ixheaacd_ps_dec.h"
#include "ixheaacd_env_extr.h"
#include "ixheaacd_adts.h"
#
#include "ixheaacd_audioobjtypes.h"
#include "ixheaacd_memory_standards.h"
#include "ixheaacd_latmdemux.h"
#include "ixheaacd_aacdec.h"
#include "ixheaacd_mps_polyphase.h"
#include "ixheaacd_config.h"
#include "ixheaacd_qmf_dec.h"
#include "ixheaacd_env_calc.h"

#include "ixheaacd_pvc_dec.h"
#include "ixheaacd_sbr_dec.h"
#include "ixheaacd_mps_macro_def.h"
#include "ixheaacd_mps_struct_def.h"
#include "ixheaacd_mps_res_rom.h"
#include "ixheaacd_mps_aac_struct.h"
#include "ixheaacd_mps_dec.h"
#include "ixheaacd_struct_def.h"

#include "ixheaacd_mps_process.h"

#include "ixheaacd_ld_mps_dec.h"
#include "ixheaacd_interface.h"
#include "ixheaacd_info.h"
#include "ixheaacd_tns_usac.h"
#include "ixheaacd_acelp_info.h"
#include "ixheaacd_main.h"
#include "ixheaacd_struct.h"
#include "ixheaacd_create.h"

WORD32 ixheaacd_ld_qmf_analysis(ia_mps_dec_state_struct *self,
                                WORD16 *output_buf) {
  WORD32 *p_arr_qmf_buf_real[MAX_ENV_COLS + MAX_ENV_COLS];
  WORD32 **p_arr_qmf_buf_imag = &p_arr_qmf_buf_real[MAX_ENV_COLS];
  WORD32 temp_arr[64 * 64];
  WORD32 *ptr = &temp_arr[0];
  float in_gain_n_q8_fac = self->input_gain * 0.00390625f;

  int i;
  WORD32 ts;

  for (ts = 2; ts < self->time_slots + 2; ts++) {
    p_arr_qmf_buf_real[ts] = ptr;
    ptr += NO_SYNTHESIS_CHANNELS;

    p_arr_qmf_buf_imag[ts] = ptr;
    ptr += NO_SYNTHESIS_CHANNELS;
  }

  self->str_mps_qmf_bank.num_time_slots = self->time_slots;
  self->str_mps_qmf_bank.no_channels = self->qmf_band_count;

  ixheaacd_cplx_anal_qmffilt_32((WORD32 *)output_buf, self->str_sbr_scale_fact,
                                &p_arr_qmf_buf_real[0], &p_arr_qmf_buf_imag[0],
                                &self->str_mps_qmf_bank,
                                self->qmf_dec_tables_ptr, 2, 0);

  for (ts = 0; ts < self->time_slots; ts++) {
    for (i = 0; i < self->qmf_band_count; i++) {
      self->qmf_in[0][ts][i].re =
          (p_arr_qmf_buf_real[ts + 2][i] * in_gain_n_q8_fac);
      self->qmf_in[0][ts][i].im =
          (p_arr_qmf_buf_imag[ts + 2][i] * in_gain_n_q8_fac);
    }
  }

  return 0;
}

WORD32 ixheaacd_ld_mps_apply(ia_exhaacplus_dec_api_struct *p_obj_exhaacplus_dec,
                             WORD16 *output_buf) {
  ia_aac_dec_state_struct *aac_handle = p_obj_exhaacplus_dec->p_state_aac;
  ia_mps_dec_state_struct *mps_handle = &(aac_handle->mps_dec_handle);
  WORD32 err = 0;
  WORD32 ts, i;
  ia_dec_data_struct *pstr_dec_data =
      (ia_dec_data_struct *)aac_handle->pstr_dec_data;
  ia_usac_data_struct *pstr_usac_data = &(pstr_dec_data->str_usac_data);

  mps_handle->hyb_band_count[0] = mps_handle->hyb_band_count_max;
  mps_handle->hyb_band_count[1] = mps_handle->hyb_band_count_max;

  err = ixheaacd_mps_frame_decode(mps_handle);
  if (err != IA_NO_ERROR) return err;

  if (aac_handle->mps_dec_handle.num_parameter_sets_prev <= 0) return IA_FATAL_ERROR;

  if (mps_handle->ldmps_config.bs_tree_config == 7) {
    ixheaacd_pre_and_mix_matrix_calculation(&(aac_handle->mps_dec_handle));
    ixheaacd_mps_pre_matrix_mix_matrix_smoothing(&(aac_handle->mps_dec_handle));
  }
  mps_handle->output_buffer = pstr_usac_data->time_sample_vector;
  if (mps_handle->ldmps_config.no_ldsbr_present)
    ixheaacd_ld_qmf_analysis(mps_handle, output_buf);
  else {
    for (ts = 0; ts < mps_handle->time_slots; ts++) {
      FLOAT32 *qmf_re = p_obj_exhaacplus_dec->p_state_aac->str_sbr_dec_info[0]
                          ->pstr_sbr_channel[0]
                          ->str_sbr_dec.mps_qmf_buf_real[ts + 2];
      FLOAT32 *qmf_im = p_obj_exhaacplus_dec->p_state_aac->str_sbr_dec_info[0]
                          ->pstr_sbr_channel[0]
                          ->str_sbr_dec.mps_qmf_buf_imag[ts + 2];
      for (i = 0; i < mps_handle->qmf_band_count; i++) {
        mps_handle->qmf_in[0][ts][i].re = qmf_re[i] * mps_handle->input_gain;
        mps_handle->qmf_in[0][ts][i].im = qmf_im[i] * mps_handle->input_gain;
      }
    }
  }
  ixheaacd_mps_qmf_hyb_analysis(mps_handle);

  mps_handle->bs_high_rate_mode = 1;

  ixheaacd_mps_apply_pre_matrix(mps_handle);

  ixheaacd_mps_create_w(mps_handle);

  if ((!(mps_handle->res_bands | mps_handle->pre_mix_req)) &&
      (mps_handle->config->bs_phase_coding == 0)) {
    ixheaacd_mps_apply_mix_matrix_type1(mps_handle);

  } else if (mps_handle->pre_mix_req) {
    ixheaacd_mps_apply_mix_matrix_type2(mps_handle);

  } else {
    ixheaacd_mps_apply_mix_matrix_type3(mps_handle);
  }

  if (mps_handle->ldmps_config.bs_temp_shape_config == 2) {
    ixheaacd_mps_time_env_shaping(mps_handle);
  }

  err = ixheaacd_mps_temp_process(mps_handle);
  mps_handle->pre_mix_req = 0;
  if (err)
    return err;

  return err;
}
