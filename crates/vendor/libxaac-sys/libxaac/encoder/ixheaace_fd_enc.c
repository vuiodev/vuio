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
#include "iusace_type_def.h"
#include "iusace_bitbuffer.h"
#include "iusace_cnst.h"
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
#include "ixheaace_adjust_threshold_data.h"
#include "iusace_fd_qc_util.h"
#include "iusace_config.h"
#include "iusace_arith_enc.h"
#include "iusace_fd_quant.h"
#include "iusace_ms.h"
#include "iusace_signal_classifier.h"
#include "iusace_block_switch_const.h"
#include "iusace_block_switch_struct_def.h"
#include "ixheaace_sbr_header.h"
#include "ixheaace_config.h"
#include "ixheaace_asc_write.h"
#include "iusace_main.h"
#include "iusace_write_bitstream.h"
#include "iusace_lpd.h"
#include "ixheaace_cplx_pred.h"
#include "iusace_func_prototypes.h"

IA_ERRORCODE iusace_fd_encode(ia_sfb_params_struct *pstr_sfb_prms, WORD32 usac_independancy_flag,
                              ia_usac_data_struct *pstr_usac_data,
                              ia_usac_encoder_config_struct *pstr_usac_config,
                              ia_bit_buf_struct *pstr_it_bit_buff, WORD32 nr_core_coder_ch,
                              WORD32 chn, WORD32 ele_id, WORD32 *bit_written,
                              WORD32 *is_quant_spec_zero, WORD32 *is_gain_limited) {
  iusace_scratch_mem *pstr_scratch = &pstr_usac_data->str_scratch;
  IA_ERRORCODE err_code = 0;
  WORD32 i_ch, idx = 0;
  WORD32 *ptr_num_fac_bits = pstr_scratch->ptr_num_fac_bits;
  WORD32 tns_data_present[2] = {0};
  WORD32 *ptr_core_mode_next = pstr_usac_data->core_mode_next;
  WORD32 *ptr_core_mode_prev = pstr_usac_data->core_mode_prev;
  *bit_written = 0;
  memset(pstr_scratch->ptr_num_fac_bits, 0,
         MAX_TIME_CHANNELS * sizeof(pstr_scratch->ptr_num_fac_bits[0]));
  for (i_ch = chn; i_ch < chn + nr_core_coder_ch; i_ch++) {
    tns_data_present[idx] = pstr_usac_data->pstr_tns_info[i_ch] != NULL;

    if (tns_data_present[idx]) {
      tns_data_present[idx] = pstr_usac_data->pstr_tns_info[i_ch]->tns_data_present;
    }
    idx++;
  }

  idx = 0;
  for (i_ch = chn; i_ch < chn + nr_core_coder_ch; i_ch++) {
    memset(pstr_scratch->p_reconstructed_time_signal[idx], 0, 4096 * sizeof(FLOAT64));
    err_code = iusace_fd_fac(
        pstr_sfb_prms->grouped_sfb_offset[i_ch], pstr_sfb_prms->max_sfb[i_ch],
        pstr_usac_data->ptr_2frame_time_data[i_ch], pstr_sfb_prms->window_sequence[i_ch],
        pstr_scratch->p_reconstructed_time_signal[idx], pstr_usac_data->td_encoder[i_ch],
        ((pstr_usac_data->td_encoder[i_ch]->prev_mode == 0) && ptr_core_mode_prev[i_ch]) ==
            CORE_MODE_TD,
        ptr_core_mode_next[i_ch] == CORE_MODE_TD, pstr_usac_data->fac_out_stream[i_ch],
        &ptr_num_fac_bits[i_ch], pstr_scratch);
    if (err_code) {
      return err_code;
    }
    idx++;
  }

  err_code = iusace_quantize_spec(pstr_sfb_prms, usac_independancy_flag, nr_core_coder_ch,
                                  pstr_usac_data, pstr_usac_config, chn, ele_id,
                                  is_quant_spec_zero, is_gain_limited);
  if (err_code) return err_code;

  for (i_ch = chn; i_ch < chn + nr_core_coder_ch; i_ch++) {
    pstr_sfb_prms->window_shape[i_ch] =
        pstr_usac_data->str_psy_mod.str_psy_out_data[i_ch].window_shape;
  }

  if (nr_core_coder_ch == 1) {
    iusace_write_bits_buf(pstr_it_bit_buff, tns_data_present[0], 1);
    *bit_written = *bit_written + 1;
  }
  if (nr_core_coder_ch == 2) {
    *bit_written = *bit_written + iusace_write_cpe(pstr_sfb_prms, pstr_it_bit_buff,
                                                   tns_data_present, usac_independancy_flag,
                                                   pstr_usac_config, pstr_usac_data, chn);
  }

  idx = 0;
  for (i_ch = chn; i_ch < chn + nr_core_coder_ch; i_ch++) {
    *bit_written =
        *bit_written + iusace_write_fd_data(pstr_it_bit_buff, pstr_sfb_prms,
                                            ptr_num_fac_bits[i_ch], usac_independancy_flag,
                                            pstr_usac_data, pstr_usac_config, i_ch, ele_id, idx);
    idx++;
  }

  return err_code;
}
