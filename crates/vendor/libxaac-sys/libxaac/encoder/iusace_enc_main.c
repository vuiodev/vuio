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
#include <stdio.h>
#include <math.h>

#include "ixheaac_error_standards.h"
#include "iusace_type_def.h"
#include "ixheaace_adjust_threshold_data.h"
#include "iusace_bitbuffer.h"
#include "ixheaace_bitbuffer.h"
#include "ixheaace_mps_common_define.h"

/* DRC */
#include "impd_drc_common_enc.h"
#include "impd_drc_uni_drc.h"
#include "impd_drc_tables.h"
#include "impd_drc_api.h"
#include "impd_drc_uni_drc_eq.h"
#include "impd_drc_uni_drc_filter_bank.h"
#include "impd_drc_gain_enc.h"
#include "impd_drc_struct_def.h"

#include "iusace_cnst.h"
#include "iusace_tns_usac.h"
#include "iusace_psy_mod.h"
#include "iusace_psy_utils.h"
#include "iusace_fd_qc_util.h"
#include "ixheaace_memory_standards.h"
#include "iusace_tns_usac.h"
#include "iusace_config.h"
#include "iusace_arith_enc.h"
#include "iusace_fd_quant.h"
#include "iusace_ms.h"
#include "iusace_block_switch_const.h"
#include "iusace_block_switch_struct_def.h"
#include "iusace_signal_classifier.h"
#include "ixheaace_sbr_header.h"
#include "ixheaace_config.h"
#include "ixheaace_asc_write.h"
#include "iusace_main.h"
#include "iusace_write_bitstream.h"
#include "iusace_windowing.h"
#include "iusace_fd_enc.h"
#include "iusace_fd_qc_adjthr.h"
#include "iusace_config.h"
#include "iusace_tcx_mdct.h"
#include "iusace_func_prototypes.h"
#include "iusace_block_switch.h"
#include "iusace_rom.h"
#include "ixheaace_error_codes.h"

#include "ixheaace_sbr_header.h"
#include "ixheaace_sbr_def.h"
#include "ixheaace_resampler.h"
#include "ixheaace_common_rom.h"

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
#include "ixheaace_sbr_rom.h"
#include "ixheaace_sbr_main.h"
#include "ixheaace_common_rom.h"
#include "ixheaace_sbr_missing_harmonics_det.h"
#include "ixheaace_sbr_inv_filtering_estimation.h"
#include "ixheaace_sbr_noise_floor_est.h"
#include "ixheaace_sbr_ton_corr.h"
#include "iusace_esbr_pvc.h"
#include "iusace_esbr_inter_tes.h"
#include "ixheaace_sbr.h"
#include "ixheaace_sbr_cmondata.h"
#include "ixheaace_sbr_crc.h"
#include "ixheaace_sbr_enc_struct.h"

#include "iusace_esbr_pvc.h"
#include "iusace_esbr_inter_tes.h"

static WORD32 iusace_get_num_elements(WORD32 num_channels) {
  WORD32 num_of_elements = 0;

  switch (num_channels) {
    case 1:
    case 2:
      num_of_elements = 1;
      break;
    default:
      num_of_elements = num_channels;
      break;
  }

  return num_of_elements;
}

static UWORD32 iusace_get_element_type(WORD32 elem_idx, WORD32 num_channels) {
  UWORD32 elem_type = (UWORD32)USAC_ELEMENT_TYPE_INVALID;
  (VOID) elem_idx;

  switch (num_channels) {
    case 1:
      elem_type = USAC_ELEMENT_TYPE_SCE;
      break;
    case 2:
      elem_type = USAC_ELEMENT_TYPE_CPE;
      break;
    default:
      elem_type = USAC_ELEMENT_TYPE_SCE;
      break;
  }

  return elem_type;
}

static VOID iusace_bw_init(ia_usac_encoder_config_struct *ptr_usac_config,
                           ixheaace_audio_specific_config_struct *pstr_asc, WORD32 ele_idx) {
  ptr_usac_config->bw_limit[ele_idx] = 20000;
  (VOID) pstr_asc;
  ptr_usac_config->bw_limit[ele_idx] =
      MIN(ptr_usac_config->bw_limit[ele_idx], ptr_usac_config->core_sample_rate / 2);

  return;
}

VOID iusace_scratch_mem_init(ia_usac_data_struct *usac_data, WORD32 total_ch, WORD32 sr) {
  iusace_scratch_mem *pstr_scratch = &usac_data->str_scratch;
  UWORD8 *temp_ptr = pstr_scratch->ptr_scratch_buf;

  pstr_scratch->ptr_stack_mem = (FLOAT32 *)(temp_ptr);
  temp_ptr += USACE_SCR_STACK;

  pstr_scratch->p_fd_mdct_windowed_long_buf = (FLOAT64 *)(temp_ptr);
  temp_ptr += (2 * FRAME_LEN_LONG) * sizeof(FLOAT64);

  pstr_scratch->p_fd_mdct_windowed_short_buf = (FLOAT64 *)(temp_ptr);
  // Size needed for above pointer is (2 * FRAME_LEN_LONG) * sizeof(FLOAT64)

  temp_ptr = (UWORD8 *)pstr_scratch->p_fd_mdct_windowed_short_buf;

  pstr_scratch->p_tns_filter = (FLOAT64 *)(temp_ptr);
  temp_ptr += (FRAME_LEN_LONG) * sizeof(FLOAT64);

  pstr_scratch->ptr_tns_scratch = (FLOAT64 *)(temp_ptr);
  temp_ptr +=
      (MAX_SHIFT_LEN_LONG + (TNS_MAX_ORDER + 1) * 2) * sizeof(pstr_scratch->ptr_tns_scratch[0]);

  pstr_scratch->p_left_fac_time_data = (FLOAT64 *)(temp_ptr);
  temp_ptr += (2 * FAC_LENGTH + ORDER) * sizeof(FLOAT64);

  pstr_scratch->p_fac_win = (FLOAT64 *)(temp_ptr);
  // Size needed for above pointer is (2 * FAC_LENGTH) * sizeof(FLOAT64)

  temp_ptr = (UWORD8 *)pstr_scratch->p_left_fac_time_data;

  pstr_scratch->p_sort_grouping_scratch = (FLOAT64 *)(temp_ptr);
  // Size needed for above pointer is (LN2) * sizeof(FLOAT64)

  temp_ptr = (UWORD8 *)pstr_scratch->p_sort_grouping_scratch;

  pstr_scratch->p_noise_filling_highest_tone = (FLOAT64 *)(temp_ptr);
  temp_ptr += (LN2) * sizeof(FLOAT64);

  pstr_scratch->p_quant_spectrum_spec_scratch = (FLOAT64 *)(temp_ptr);
  temp_ptr += (2 * FRAME_LEN_LONG) * sizeof(FLOAT64);

  pstr_scratch->p_cmpx_mdct_temp_buf = (FLOAT64 *)(temp_ptr);
  // Size needed for above pointer is (LN2) * sizeof(FLOAT64)

  temp_ptr = (UWORD8 *)pstr_scratch->p_noise_filling_highest_tone;

  for (WORD32 i = 0; i < total_ch; i++) {
    pstr_scratch->p_reconstructed_time_signal[i] = (FLOAT64 *)(temp_ptr);
    temp_ptr += (4 * FRAME_LEN_LONG) * sizeof(FLOAT64);
  }
  pstr_scratch->ptr_next_win_scratch = (WORD32 *)(temp_ptr);
  temp_ptr += (2 * MAX_TIME_CHANNELS) * sizeof(pstr_scratch->ptr_next_win_scratch[0]);

  pstr_scratch->p_fft_p2_y = (FLOAT32 *)(temp_ptr);
  temp_ptr += (2 * FRAME_LEN_LONG) * sizeof(FLOAT32);

  pstr_scratch->p_fft_p3_data_3 = (FLOAT32 *)(temp_ptr);
  temp_ptr += (800) * sizeof(FLOAT32);

  pstr_scratch->p_fft_p3_y = (FLOAT32 *)(temp_ptr);
  temp_ptr += (2 * FRAME_LEN_LONG) * sizeof(FLOAT32);

  pstr_scratch->p_time_signal = (FLOAT32 *)(temp_ptr);
  temp_ptr += (FRAME_LEN_LONG) * sizeof(FLOAT32);

  pstr_scratch->p_complex_fft = (FLOAT32 *)(temp_ptr);
  temp_ptr += (2 * FRAME_LEN_LONG) * sizeof(FLOAT32);

  pstr_scratch->p_tonal_flag = (WORD32 *)(temp_ptr);
  temp_ptr += (FRAME_LEN_LONG / 2) * sizeof(WORD32);

  pstr_scratch->p_pow_spec = (FLOAT32 *)(temp_ptr);
  // Size needed for above pointer is (FRAME_LEN_LONG / 2) * sizeof(FLOAT32)
  temp_ptr = (UWORD8 *)pstr_scratch->p_time_signal;

  pstr_scratch->p_temp_mdct = (FLOAT32 *)(temp_ptr);
  temp_ptr += (1024) * sizeof(FLOAT32);

  pstr_scratch->p_buf_synthesis_tool = (FLOAT32 *)(temp_ptr);
  temp_ptr += (LEN_FRAME_16K + ORDER_LP_FILT_16K) * sizeof(FLOAT32);

  pstr_scratch->p_mdct_spec_float = (FLOAT32 *)(temp_ptr);
  temp_ptr += (FRAME_LEN_LONG) * sizeof(FLOAT32);

  pstr_scratch->p_sq_gain_en = (FLOAT32 *)(temp_ptr);
  temp_ptr += (FRAME_LEN_LONG / 4) * sizeof(FLOAT32);

  pstr_scratch->p_fft_mdct_buf = (FLOAT32 *)(temp_ptr);
  temp_ptr += (4 * FRAME_LEN_LONG) * sizeof(FLOAT32);

  pstr_scratch->p_arith_map_prev_scratch = (WORD32 *)(temp_ptr);
  temp_ptr += (516) * sizeof(WORD32);

  pstr_scratch->p_arith_map_pres_scratch = (WORD32 *)(temp_ptr);
  temp_ptr += (516) * sizeof(WORD32);

  pstr_scratch->p_ol_pitch_buf_tmp = (FLOAT32 *)(temp_ptr);
  temp_ptr += (FRAME_LEN_LONG) * sizeof(FLOAT32);

  pstr_scratch->p_ol_pitch_speech_buf = (FLOAT32 *)(temp_ptr);
  temp_ptr += (FRAME_LEN_LONG + LAG_MAX) * sizeof(FLOAT32);

  pstr_scratch->p_ol_pitch_w_table = (FLOAT32 *)(temp_ptr);
  temp_ptr += (LEN_CORR_R) * sizeof(FLOAT32);

  pstr_scratch->p_ol_pitch_R = (FLOAT32 *)(temp_ptr);
  temp_ptr += (LEN_CORR_R) * sizeof(FLOAT32);

  WORD32 R0_size = (54 + 6 * ((WORD32)(34.f * ((FLOAT32)sr / 2.f) / 12800.f + 0.5f) * 2)) / 2;
  pstr_scratch->p_ol_pitch_R0 = (FLOAT32 *)(temp_ptr);
  temp_ptr += (R0_size) * sizeof(FLOAT32);

  pstr_scratch->p_lpd_frm_enc_scratch = (FLOAT32 *)(temp_ptr);
  temp_ptr += (LEN_FRAME + 1) * sizeof(FLOAT32);

  pstr_scratch->p_wsig_buf = (FLOAT32 *)(temp_ptr + 128 * sizeof(FLOAT32));
  temp_ptr += (128 + FRAME_LEN_LONG) * sizeof(FLOAT32);

  pstr_scratch->p_wsyn_tcx_buf = (FLOAT32 *)(temp_ptr + 128 * sizeof(FLOAT32));
  temp_ptr += (128 + FRAME_LEN_LONG) * sizeof(FLOAT32);

  pstr_scratch->p_synth_tcx_buf = (FLOAT32 *)(temp_ptr + 128 * sizeof(FLOAT32));
  temp_ptr += (128 + FRAME_LEN_LONG) * sizeof(FLOAT32);

  pstr_scratch->p_wsyn_buf = (FLOAT32 *)(temp_ptr + 128 * sizeof(FLOAT32));
  temp_ptr += (128 + FRAME_LEN_LONG) * sizeof(FLOAT32);

  pstr_scratch->p_synth_buf = (FLOAT32 *)(temp_ptr + 128 * sizeof(FLOAT32));
  temp_ptr += (128 + FRAME_LEN_LONG) * sizeof(FLOAT32);

  pstr_scratch->p_temp_wsyn_buf = (FLOAT32 *)temp_ptr;
  temp_ptr += (FRAME_LEN_LONG) * sizeof(FLOAT32);

  pstr_scratch->p_lp_filter_coeff = (FLOAT32 *)(temp_ptr);
  temp_ptr += ((NUM_SUBFR_SUPERFRAME + 1) * (ORDER + 1)) * sizeof(FLOAT32);

  pstr_scratch->p_lp_filter_coeff_q = (FLOAT32 *)(temp_ptr);
  temp_ptr += ((NUM_SUBFR_SUPERFRAME + 1) * (ORDER + 1)) * sizeof(FLOAT32);

  pstr_scratch->p_wsp_prev_buf = (FLOAT32 *)(temp_ptr);
  temp_ptr += ((MAX_PITCH1 / OPL_DECIM) + LEN_FRAME) * sizeof(FLOAT32);

  pstr_scratch->ptr_lpd_scratch = (UWORD8 *)temp_ptr;
  temp_ptr += ((2 * (NUM_SUBFR_SUPERFRAME + 1) * (ORDER + 1)) + (4 * (NUM_FRAMES + 1) * ORDER) +
               (((NUM_FRAMES >> 1) + 1) * ORDER) * 4) *
                  sizeof(FLOAT32) +
              100 * sizeof(WORD32) + 6 * sizeof(ia_usac_lpd_scratch);

  pstr_scratch->p_prm_tcx = (WORD32 *)(temp_ptr);
  temp_ptr += (NUM_TCX80_PRM) * sizeof(WORD32);

  pstr_scratch->p_buf_speech = (FLOAT32 *)(temp_ptr);
  temp_ptr += (2 * LEN_FRAME + ORDER) * sizeof(FLOAT32);

  pstr_scratch->p_buf_res = (FLOAT32 *)(temp_ptr);
  temp_ptr += (2 * LEN_FRAME) * sizeof(FLOAT32);

  pstr_scratch->p_buf_signal = (FLOAT32 *)(temp_ptr);
  temp_ptr += (ORDER + LEN_FRAME) * sizeof(FLOAT32);

  pstr_scratch->p_xn1_tcx = (FLOAT32 *)(temp_ptr);
  temp_ptr += (2 * FAC_LENGTH) * sizeof(FLOAT32);

  pstr_scratch->p_xn_buf_tcx = (FLOAT32 *)(temp_ptr);
  temp_ptr += (128 + FRAME_LEN_LONG + 128) * sizeof(FLOAT32);

  pstr_scratch->p_x_tcx = (FLOAT32 *)(temp_ptr);
  temp_ptr += (FRAME_LEN_LONG) * sizeof(FLOAT32);

  pstr_scratch->p_x_tmp_tcx = (FLOAT32 *)(temp_ptr);
  temp_ptr += (FRAME_LEN_LONG) * sizeof(FLOAT32);

  pstr_scratch->p_en_tcx = (FLOAT32 *)(temp_ptr);
  temp_ptr += (FRAME_LEN_LONG) * sizeof(FLOAT32);

  pstr_scratch->p_alfd_gains_tcx = (FLOAT32 *)(temp_ptr);
  temp_ptr += (FRAME_LEN_LONG / (4 * 8)) * sizeof(FLOAT32);

  pstr_scratch->p_sq_enc_tcx = (FLOAT32 *)(temp_ptr);
  temp_ptr += (FRAME_LEN_LONG) * sizeof(FLOAT32);

  pstr_scratch->p_sq_quant_tcx = (WORD32 *)(temp_ptr);
  temp_ptr += (FRAME_LEN_LONG) * sizeof(WORD32);

  pstr_scratch->p_gain1_tcx = (FLOAT32 *)(temp_ptr);
  temp_ptr += (FRAME_LEN_LONG) * sizeof(FLOAT32);

  pstr_scratch->p_gain2_tcx = (FLOAT32 *)(temp_ptr);
  temp_ptr += (FRAME_LEN_LONG) * sizeof(FLOAT32);

  pstr_scratch->p_facelp_tcx = (FLOAT32 *)(temp_ptr);
  temp_ptr += (FAC_LENGTH) * sizeof(FLOAT32);

  pstr_scratch->p_xn2_tcx = (FLOAT32 *)(temp_ptr);
  temp_ptr += (2 * FAC_LENGTH) * sizeof(FLOAT32);

  pstr_scratch->p_fac_window_tcx = (FLOAT32 *)(temp_ptr);
  temp_ptr += (2 * FAC_LENGTH) * sizeof(FLOAT32);

  pstr_scratch->p_x1_tcx = (FLOAT32 *)(temp_ptr);
  temp_ptr += (FAC_LENGTH) * sizeof(FLOAT32);

  pstr_scratch->p_x2_tcx = (FLOAT32 *)(temp_ptr);
  temp_ptr += (FAC_LENGTH) * sizeof(FLOAT32);

  pstr_scratch->p_y_tcx = (WORD32 *)(temp_ptr);
  temp_ptr += (FAC_LENGTH) * sizeof(WORD32);

  pstr_scratch->p_in_out_tcx = (FLOAT32 *)(temp_ptr);
  temp_ptr += (FRAME_LEN_LONG * 2 * 2) * sizeof(FLOAT32);

  pstr_scratch->p_tcx_input = (FLOAT32 *)(temp_ptr);
  temp_ptr += (FRAME_LEN_LONG) * sizeof(FLOAT32);

  pstr_scratch->ptr_tcx_scratch = (FLOAT32 *)(temp_ptr);
  temp_ptr += 3 * (FRAME_LEN_LONG) * sizeof(pstr_scratch->ptr_tcx_scratch[0]);

  pstr_scratch->p_tcx_output = (FLOAT32 *)(temp_ptr);
  temp_ptr += (FRAME_LEN_LONG) * sizeof(FLOAT32);

  pstr_scratch->p_buf_aut_corr = (FLOAT32 *)(temp_ptr);
  // Size needed for above pointer is (LEN_WIN_PLUS) * sizeof(FLOAT32)
  temp_ptr = (UWORD8 *)pstr_scratch->p_buf_aut_corr;

  pstr_scratch->p_xn2 = (FLOAT32 *)(temp_ptr);
  temp_ptr += (FAC_LENGTH + ORDER) * sizeof(FLOAT32);

  pstr_scratch->p_fac_dec = (FLOAT32 *)(temp_ptr);
  temp_ptr += (2 * FAC_LENGTH) * sizeof(FLOAT32);

  pstr_scratch->p_right_fac_spec = (FLOAT32 *)(temp_ptr);
  temp_ptr += (FAC_LENGTH) * sizeof(FLOAT32);

  pstr_scratch->p_x2 = (FLOAT32 *)(temp_ptr);
  temp_ptr += (FAC_LENGTH) * sizeof(FLOAT32);

  pstr_scratch->p_param = (WORD32 *)(temp_ptr);
  temp_ptr += (FAC_LENGTH + 1) * sizeof(WORD32);

  pstr_scratch->p_x = (FLOAT32 *)(temp_ptr);
  temp_ptr += (FAC_LENGTH) * sizeof(FLOAT32);

  pstr_scratch->p_xn_2 = (FLOAT32 *)(temp_ptr);
  temp_ptr += (2 * FAC_LENGTH + ORDER) * sizeof(FLOAT32);

  pstr_scratch->p_fac_window = (FLOAT32 *)(temp_ptr);
  temp_ptr += (2 * FAC_LENGTH) * sizeof(FLOAT32);

  pstr_scratch->p_fir_sig_buf = (FLOAT32 *)(temp_ptr);
  // Size needed for above pointer is (3 + LEN_FRAME) * sizeof(FLOAT32)
  temp_ptr = (UWORD8 *)pstr_scratch->p_fir_sig_buf;

  pstr_scratch->p_acelp_ir_buf = (FLOAT32 *)(temp_ptr);

  temp_ptr += (4 * LEN_SUBFR) * sizeof(FLOAT32);

  pstr_scratch->ptr_acelp_scratch = (FLOAT32 *)(temp_ptr);
  temp_ptr += ((11 * LEN_SUBFR) + (ORDER + LEN_SUBFR + 8) + 1024) *
              sizeof(pstr_scratch->ptr_acelp_scratch[0]);

  pstr_scratch->p_acelp_exc_buf = (FLOAT32 *)(temp_ptr);
  // Size needed for above pointer is ((3 * LEN_FRAME) + 1 + 41) * sizeof(FLOAT32)

  temp_ptr = (UWORD8 *)pstr_scratch->p_lpd_frm_enc_scratch;

  pstr_scratch->p_fac_bits_word = (WORD16 *)(temp_ptr);
  temp_ptr += (5000) * sizeof(WORD16);

  pstr_scratch->p_left_fac_timedata_flt = (FLOAT32 *)(temp_ptr);
  temp_ptr += (2 * FAC_LENGTH + ORDER) * sizeof(FLOAT32);

  pstr_scratch->p_left_fac_spec = (FLOAT32 *)(temp_ptr);
  temp_ptr += (FAC_LENGTH) * sizeof(FLOAT32);

  pstr_scratch->p_fac_prm = (WORD32 *)(temp_ptr);
  temp_ptr += (FAC_LENGTH + 1) * sizeof(WORD32);

  pstr_scratch->p_acelp_folded_scratch = (FLOAT32 *)(temp_ptr);
  // Size needed for above pointer is (FAC_LENGTH) * sizeof(FLOAT32)

  temp_ptr = (UWORD8 *)pstr_scratch->p_fac_bits_word;

  pstr_scratch->p_exp_spec = (FLOAT32 *)(temp_ptr);
  temp_ptr += (FRAME_LEN_LONG) * sizeof(FLOAT32);

  pstr_scratch->p_adjthr_ptr_exp_spec = (FLOAT32 *)(temp_ptr);
  temp_ptr += (FRAME_LEN_LONG) * sizeof(FLOAT32);

  pstr_scratch->p_adjthr_mdct_spec_float = (FLOAT32 *)(temp_ptr);
  temp_ptr += (FRAME_LEN_LONG) * sizeof(FLOAT32);

  pstr_scratch->p_adjthr_quant_spec_temp = (WORD16 *)(temp_ptr);
  temp_ptr += (FRAME_LEN_LONG) * sizeof(WORD16);

  pstr_scratch->p_degroup_scratch = (WORD32 *)(temp_ptr);
  temp_ptr += (FRAME_LEN_LONG) * sizeof(WORD32);

  /*Newly added*/
  pstr_scratch->ptr_drc_scratch_buf = (UWORD8 *)(temp_ptr);

  pstr_scratch->ptr_num_fac_bits = (WORD32 *)temp_ptr;
  temp_ptr += MAX_TIME_CHANNELS * sizeof(pstr_scratch->ptr_num_fac_bits[0]);
  pstr_scratch->ptr_tns_data_present = (WORD32 *)temp_ptr;
  temp_ptr += MAX_TIME_CHANNELS * sizeof(pstr_scratch->ptr_tns_data_present[0]);

  pstr_scratch->ptr_tmp_lp_res = (FLOAT32 *)temp_ptr;
  temp_ptr += FAC_LENGTH * sizeof(pstr_scratch->ptr_tmp_lp_res[0]);

  for (WORD32 i = 0; i < total_ch; i++) {
    pstr_scratch->ptr_sfb_form_fac[i] = (FLOAT32 *)temp_ptr;
    temp_ptr += (MAX_NUM_GROUPED_SFB) * sizeof(FLOAT32);
  }
  for (WORD32 i = 0; i < total_ch; i++) {
    pstr_scratch->ptr_sfb_num_relevant_lines[i] = (FLOAT32 *)temp_ptr;
    temp_ptr += (MAX_NUM_GROUPED_SFB) * sizeof(FLOAT32);
  }
  for (WORD32 i = 0; i < total_ch; i++) {
    pstr_scratch->ptr_sfb_ld_energy[i] = (FLOAT32 *)temp_ptr;
    temp_ptr += (MAX_NUM_GROUPED_SFB) * sizeof(FLOAT32);
  }
  pstr_scratch->ptr_num_scfs = (WORD32 *)temp_ptr;
  temp_ptr += (MAX_TIME_CHANNELS) * sizeof(pstr_scratch->ptr_num_scfs[0]);

  pstr_scratch->ptr_max_ch_dyn_bits = (WORD32 *)temp_ptr;
  temp_ptr += (MAX_TIME_CHANNELS) * sizeof(pstr_scratch->ptr_max_ch_dyn_bits[0]);
  pstr_scratch->ptr_ch_bit_dist = (FLOAT32 *)temp_ptr;
  temp_ptr += (MAX_TIME_CHANNELS) * sizeof(pstr_scratch->ptr_ch_bit_dist[0]);
  pstr_scratch->ptr_fd_scratch = (UWORD8 *)temp_ptr;
  // Size needed for above pointer is (IXHEAACE_MAX_CH_IN_BS_ELE * MAX_NUM_GROUPED_SFB * 3) *
  // sizeof(WORD32)

  return;
}

WORD32 iusace_limitbitrate(WORD32 core_sample_rate, WORD32 frame_len, WORD32 num_ch,
                           WORD32 bit_rate) {
  WORD32 transport_bits, prev_bit_rate, shift = 0, iter = 0;

  while ((frame_len & ~((1 << (shift + 1)) - 1)) == frame_len &&
         (core_sample_rate & ~((1 << (shift + 1)) - 1)) == core_sample_rate) {
    shift++;
  }

  do {
    prev_bit_rate = bit_rate;
    /* Assume some worst case */
    transport_bits = 208;

    bit_rate =
        MAX(bit_rate, ((((40 * num_ch) + transport_bits) * (core_sample_rate)) / frame_len));
    bit_rate =
        MIN(bit_rate, ((num_ch * 6144) * (core_sample_rate >> shift)) / (frame_len >> shift));

  } while (prev_bit_rate != bit_rate && iter++ < 3);

  return bit_rate;
}

WORD32 iusace_map_sample_rate(WORD32 sample_rate)
{
  WORD32 mapped_sample_rate = sample_rate;

  if ((mapped_sample_rate >= 0) && (mapped_sample_rate < 9391))
  {
    mapped_sample_rate = 8000;
  }
  else if ((mapped_sample_rate >= 9391) && (mapped_sample_rate < 11502))
  {
    mapped_sample_rate = 11025;
  }
  else if ((mapped_sample_rate >= 11502) && (mapped_sample_rate < 13856))
  {
    mapped_sample_rate = 12000;
  }
  else if ((mapped_sample_rate >= 13856) && (mapped_sample_rate < 18783))
  {
    mapped_sample_rate = 16000;
  }
  else if ((mapped_sample_rate >= 18783) && (mapped_sample_rate < 23004))
  {
    mapped_sample_rate = 22050;
  }
  else if ((mapped_sample_rate >= 23004) && (mapped_sample_rate < 27713))
  {
    mapped_sample_rate = 24000;
  }
  else if ((mapped_sample_rate >= 27713) && (mapped_sample_rate < 37566))
  {
    mapped_sample_rate = 32000;
  }
  else if ((mapped_sample_rate >= 37566) && (mapped_sample_rate < 46009))
  {
    mapped_sample_rate = 44100;
  }
  else if ((mapped_sample_rate >= 46009) && (mapped_sample_rate < 55426))
  {
    mapped_sample_rate = 48000;
  }
  else if ((mapped_sample_rate >= 55426) && (mapped_sample_rate < 75132))
  {
    mapped_sample_rate = 64000;
  }
  else if ((mapped_sample_rate >= 75132) && (mapped_sample_rate < 92017))
  {
    mapped_sample_rate = 88200;
  }
  else if (mapped_sample_rate >= 92017)
  {
    mapped_sample_rate = 96000;
  }
  else
  {
    mapped_sample_rate = 48000;
  }
  return mapped_sample_rate;
}

/**
 *  iusace_enc_init
 *
 *  \brief Enocder initialization
 *
 *  \param [in] ptr_usac_config  Pointer to encoder config structure
 *  \param [in] ptr_asc          Pointer to audio specific config strcuture
 *  \param [in] pstr_state       Poniter to encoder handle structure
 *
 *  \return IA_ERRORCODE         Error code
 */
IA_ERRORCODE iusace_enc_init(ia_usac_encoder_config_struct *ptr_usac_config,
                             ixheaace_audio_specific_config_struct *pstr_asc,
                             ia_usac_data_struct *pstr_state) {
  IA_ERRORCODE err_code = IA_NO_ERROR;
  WORD32 i, j, k, idx, i_ch;
  UWORD32 elem_idx = 0;
  ia_usac_data_struct *usac_data = (pstr_state);
  ixheaace_audio_specific_config_struct *p_audio_specific_config = pstr_asc;
  ia_usac_config_struct *pstr_asc_usac_config = &(p_audio_specific_config->str_usac_config);
  WORD32 nbuff = 2048;
  usac_data->usac_independency_flag_count = 0;
  usac_data->usac_independency_flag_interval = 25;
  usac_data->use_acelp_only = ptr_usac_config->use_acelp_only;
  for (j = 0; j < MAX_TIME_CHANNELS; j++) {
    memset(usac_data->overlap_buf[j], 0, nbuff * sizeof(FLOAT64 *));

    usac_data->str_ms_info[j].ms_mask = 0;
    for (i = 0; i < MAX_SHORT_WINDOWS; i++) {
      for (k = 0; k < MAX_SFB_LONG; k++) {
        usac_data->str_ms_info[j].ms_used[i][k] = 0;
      }
    }
  }

  iusace_scratch_mem_init(usac_data, ptr_usac_config->channels,
                          ptr_usac_config->core_sample_rate);

  for (i = 0; i < MAX_TIME_CHANNELS; i++) {
    if (ptr_usac_config->cmplx_pred_flag) {
      usac_data->str_ms_info[i].ms_mask = 3;
    }
    usac_data->ptr_dmx_re_save[i] = &usac_data->arr_dmx_save_float[i][0];
    usac_data->ptr_dmx_im[i] = &usac_data->arr_dmx_im[i][0];
  }

  pstr_asc_usac_config->num_elements = 0;
  pstr_asc_usac_config->usac_cfg_ext_present = 0;
  pstr_asc_usac_config->num_config_extensions = 0;

  if (ptr_usac_config->channels > 0) {
    if (ptr_usac_config->channels < 7) {
      p_audio_specific_config->channel_configuration = ptr_usac_config->channels;
    }
  }
  ia_usac_enc_element_config_struct *pstr_usac_elem_config =
      &(pstr_asc_usac_config->str_usac_element_config[pstr_asc_usac_config->num_elements]);
  pstr_asc_usac_config->usac_element_type[pstr_asc_usac_config->num_elements] = ID_USAC_EXT;
  pstr_usac_elem_config->usac_ext_ele_type = ID_EXT_ELE_AUDIOPREROLL;
  pstr_usac_elem_config->usac_ext_ele_dflt_len_present = 0;
  pstr_usac_elem_config->usac_ext_ele_payload_present = 0;
  pstr_usac_elem_config->usac_ext_ele_cfg_len = 0;
  pstr_asc_usac_config->num_elements++;
  // DRC Config
  if (ptr_usac_config->use_drc_element) {
    ptr_usac_config->str_drc_cfg.str_uni_drc_config.str_channel_layout.base_ch_count =
        ptr_usac_config->channels;

    memset(&usac_data->str_drc_state, 0, sizeof(ia_drc_enc_state));

    err_code = impd_drc_enc_init(&usac_data->str_drc_state, pstr_state->str_scratch.drc_scratch,
                                 &ptr_usac_config->str_drc_cfg);
    if (err_code & IA_FATAL_ERROR) {
      return err_code;
    }
    if (err_code) {
      ptr_usac_config->use_drc_element = 0;
      err_code = IA_NO_ERROR;
    }

    if (ptr_usac_config->use_drc_element) {
      ia_usac_enc_element_config_struct *pstr_usac_elem_config =
          &(pstr_asc_usac_config->str_usac_element_config[pstr_asc_usac_config->num_elements]);
      pstr_asc_usac_config->usac_element_type[pstr_asc_usac_config->num_elements] = ID_USAC_EXT;
      pstr_usac_elem_config->usac_ext_ele_type = ID_EXT_ELE_UNI_DRC;
      pstr_usac_elem_config->usac_ext_ele_dflt_len_present = 0;
      pstr_usac_elem_config->usac_ext_ele_payload_present = 0;
      pstr_usac_elem_config->drc_config_data = usac_data->str_drc_state.bit_buf_base_cfg;
      pstr_usac_elem_config->usac_ext_ele_cfg_len =
          (usac_data->str_drc_state.drc_config_data_size_bit + 7) >> 3;
      pstr_asc_usac_config->num_elements++;
    }
  } else {
    err_code =
        impd_loudness_info_init(&usac_data->str_drc_state, &ptr_usac_config->str_drc_cfg);
    if (err_code) {
      return err_code;
    }
  }

  pstr_asc_usac_config->usac_config_ext_type[pstr_asc_usac_config->num_config_extensions] =
      ID_CONFIG_EXT_STREAM_ID;
  pstr_asc_usac_config->usac_config_ext_len[pstr_asc_usac_config->num_config_extensions] =
      CONFIG_EXT_LEN_STREAM_ID;
  pstr_asc_usac_config->num_config_extensions++;
  pstr_asc_usac_config->stream_identifier = ptr_usac_config->stream_id;

  pstr_asc_usac_config->usac_config_ext_type[pstr_asc_usac_config->num_config_extensions] =
      ID_CONFIG_EXT_LOUDNESS_INFO;
  pstr_asc_usac_config->usac_config_ext_len[pstr_asc_usac_config->num_config_extensions] =
      (usac_data->str_drc_state.drc_config_ext_data_size_bit + 7) >> 3;
  pstr_asc_usac_config->usac_config_ext_buf[pstr_asc_usac_config->num_config_extensions] =
      usac_data->str_drc_state.bit_buf_base_cfg_ext;
  pstr_asc_usac_config->num_config_extensions++;

  if (pstr_asc_usac_config->num_config_extensions) {
    pstr_asc_usac_config->usac_cfg_ext_present = 1;
  }

  p_audio_specific_config->sampling_frequency = ptr_usac_config->native_sample_rate;
  p_audio_specific_config->num_audio_channels = ptr_usac_config->channels;
  elem_idx = pstr_asc_usac_config->num_elements;
  ptr_usac_config->num_ext_elements = elem_idx;
  pstr_asc_usac_config->num_ext_elements = elem_idx;
  i = elem_idx;

  if (ptr_usac_config->channels != 0) {
    ptr_usac_config->num_elements = iusace_get_num_elements(ptr_usac_config->channels);
    pstr_asc_usac_config->num_elements += ptr_usac_config->num_elements;

    for (; i < (WORD32)pstr_asc_usac_config->num_elements; i++) {
      pstr_asc_usac_config->usac_element_type[i] = iusace_get_element_type(
          (i - ptr_usac_config->num_ext_elements), ptr_usac_config->channels);
    }
  }

  WORD32 count = ptr_usac_config->num_elements;
  ptr_usac_config->num_elements = pstr_asc_usac_config->num_elements;
  iusace_qc_create(&usac_data->str_qc_main);

  if (count > 2) {
    WORD32 num_mono = 0, num_stereo = 0, num_lfe = 0;

    for (WORD8 ch_idx = 0; ch_idx < count; ch_idx++) {
      switch (
          pstr_asc_usac_config->usac_element_type[ch_idx + ptr_usac_config->num_ext_elements]) {
        case ID_USAC_SCE:
          num_mono++;
          break;
        case ID_USAC_CPE:
          num_stereo++;
          break;
        case ID_USAC_EXT:
          break;
        default:
          return IA_EXHEAACE_INIT_FATAL_USAC_INVALID_ELEMENT_TYPE;
      }
    }

    WORD32 bitrate_per_stereo = (WORD32)((ptr_usac_config->basic_bitrate - (num_lfe)*8000) /
                                         (num_mono * 0.625 + num_stereo));
    WORD32 bitrate_per_mono = (WORD32)(0.625 * bitrate_per_stereo);

    for (WORD8 ch_idx = 0; ch_idx < count; ch_idx++) {
      switch (
          pstr_asc_usac_config->usac_element_type[ch_idx + ptr_usac_config->num_ext_elements]) {
        case ID_USAC_SCE:
          usac_data->str_qc_main.str_qc_data[ch_idx].ch_bitrate = bitrate_per_mono;
          break;
        case ID_USAC_CPE:
          usac_data->str_qc_main.str_qc_data[ch_idx].ch_bitrate = bitrate_per_stereo;
          break;
        case ID_USAC_EXT:
          break;
        default:
          return IA_EXHEAACE_INIT_FATAL_USAC_INVALID_ELEMENT_TYPE;
      }

      usac_data->str_qc_main.str_qc_data[ch_idx].num_ch = 1;
      if (ID_USAC_CPE ==
          pstr_asc_usac_config->usac_element_type[ch_idx + ptr_usac_config->num_ext_elements]) {
        usac_data->str_qc_main.str_qc_data[ch_idx].num_ch = 2;
      }

      usac_data->str_qc_main.str_qc_data[ch_idx].ch_bitrate =
          MIN(360000 * usac_data->str_qc_main.str_qc_data[ch_idx].num_ch,
              usac_data->str_qc_main.str_qc_data[ch_idx].ch_bitrate);
      usac_data->str_qc_main.str_qc_data[ch_idx].ch_bitrate =
          MAX(8000 * usac_data->str_qc_main.str_qc_data[ch_idx].num_ch,
              usac_data->str_qc_main.str_qc_data[ch_idx].ch_bitrate);

      usac_data->str_qc_main.str_qc_data[ch_idx].ch_bitrate =
          iusace_limitbitrate(ptr_usac_config->core_sample_rate, 512,
                              usac_data->str_qc_main.str_qc_data[ch_idx].num_ch,
                              usac_data->str_qc_main.str_qc_data[ch_idx].ch_bitrate);

      usac_data->str_qc_main.str_qc_data[ch_idx].avg_bits =
          (usac_data->str_qc_main.str_qc_data[ch_idx].ch_bitrate * ptr_usac_config->ccfl) /
          ptr_usac_config->core_sample_rate;
    }
  } else {
    for (WORD8 ch_idx = 0; ch_idx < count; ch_idx++) {
      usac_data->str_qc_main.str_qc_data[ch_idx].num_ch = (WORD8)ptr_usac_config->channels;
      usac_data->str_qc_main.str_qc_data[ch_idx].ch_bitrate = ptr_usac_config->basic_bitrate;
      usac_data->str_qc_main.str_qc_data[ch_idx].avg_bits =
          (usac_data->str_qc_main.str_qc_data[ch_idx].ch_bitrate * ptr_usac_config->ccfl) /
          ptr_usac_config->core_sample_rate;
    }
  }

  for (i_ch = 0;
       i_ch < (WORD32)(ptr_usac_config->num_elements - ptr_usac_config->num_ext_elements);
       i_ch++) {
    iusace_bw_init(ptr_usac_config, p_audio_specific_config, i_ch);

    usac_data->noise_filling[i_ch] = ptr_usac_config->flag_noiseFilling;
  }

  memset(&usac_data->str_psy_mod.str_psy_out_data, 0,
         sizeof(ia_psy_mod_out_data_struct) * MAX_TIME_CHANNELS);

  i_ch = 0;
  for (UWORD32 ch_idx = 0;
       ch_idx < pstr_asc_usac_config->num_elements - ptr_usac_config->num_ext_elements;
       ch_idx++) {
    iusace_psy_mod_init(
        &usac_data->str_psy_mod,
        ((LEN_SUPERFRAME_768 == ptr_usac_config->ccfl) && (0 == ptr_usac_config->sbr_enable)) ?
        (iusace_map_sample_rate((ptr_usac_config->core_sample_rate * 4) / 3)) :
        (ptr_usac_config->core_sample_rate),
        usac_data->str_qc_main.str_qc_data[ch_idx].ch_bitrate, ptr_usac_config->bw_limit[ch_idx],
        usac_data->str_qc_main.str_qc_data[ch_idx].num_ch, i_ch, ch_idx, ptr_usac_config->ccfl);
    i_ch += usac_data->str_qc_main.str_qc_data[ch_idx].num_ch;
  }

  for (; elem_idx < pstr_asc_usac_config->num_elements; elem_idx++) {
    idx = elem_idx - pstr_asc_usac_config->num_ext_elements;
    pstr_asc_usac_config->str_usac_element_config[elem_idx].noise_filling =
        usac_data->noise_filling[idx];
    usac_data->channel_elem_type[idx] = pstr_asc_usac_config->usac_element_type[elem_idx];
  }

  if (ptr_usac_config->use_fill_element) {
    ia_usac_enc_element_config_struct *pstr_usac_elem_config =
        &(pstr_asc_usac_config->str_usac_element_config[pstr_asc_usac_config->num_elements]);
    pstr_asc_usac_config->usac_element_type[pstr_asc_usac_config->num_elements] = ID_USAC_EXT;
    pstr_usac_elem_config->usac_ext_ele_type = ID_EXT_ELE_FILL;
    pstr_usac_elem_config->usac_ext_ele_cfg_len = 0;
    pstr_usac_elem_config->usac_ext_ele_dflt_len_present = 0;
    pstr_usac_elem_config->usac_ext_ele_payload_present = 0;
    pstr_asc_usac_config->num_elements++;
    ptr_usac_config->num_ext_elements++;
    ptr_usac_config->num_elements++;
  }

  if (ptr_usac_config->codec_mode == USAC_SWITCHED) {
    iusace_init_classification(&usac_data->str_sig_class_data);
  }

  i_ch = 0;
  for (UWORD32 ch_idx = 0;
       ch_idx < pstr_asc_usac_config->num_elements - ptr_usac_config->num_ext_elements;
       ch_idx++) {
    for (idx = 0; idx < usac_data->str_qc_main.str_qc_data[ch_idx].num_ch; idx++, i_ch++) {
      iusace_init_block_switching(&usac_data->block_switch_ctrl[i_ch],
                                  usac_data->str_qc_main.str_qc_data[ch_idx].ch_bitrate,
                                  usac_data->str_qc_main.str_qc_data[ch_idx].num_ch);
    }
  }

  pstr_asc_usac_config->str_usac_element_config[elem_idx].stereo_config_index = 0;

  for (i_ch = 0; i_ch < ptr_usac_config->channels; i_ch++) {
    ptr_usac_config->window_sequence[i_ch] = ONLY_LONG_SEQUENCE;
    ptr_usac_config->window_shape_prev[i_ch] = WIN_SEL_0;
  }

  for (i_ch = 0; i_ch < ptr_usac_config->channels; i_ch++) {
    memset(usac_data->td_in_buf[i_ch], 0,
           (FRAME_LEN_LONG + LEN_NEXT_HIGH_RATE) * sizeof(usac_data->td_in_buf[i_ch][0]));
  }

  usac_data->max_bitreservoir_bits = MAX_CHANNEL_BITS * ptr_usac_config->channels;
  usac_data->available_bitreservoir_bits = usac_data->max_bitreservoir_bits;
  usac_data->available_bitreservoir_bits -=
      (ptr_usac_config->bit_rate * ptr_usac_config->ccfl) / ptr_usac_config->core_sample_rate;

  if (usac_data->available_bitreservoir_bits < 0) {
    return IA_EXHEAACE_INIT_FATAL_USAC_BITRES_SIZE_TOO_SMALL;
  }
  i_ch = 0;
  for (UWORD32 ch_idx = 0;
       ch_idx < pstr_asc_usac_config->num_elements - ptr_usac_config->num_ext_elements;
       ch_idx++) {
    for (idx = 0; idx < usac_data->str_qc_main.str_qc_data[ch_idx].num_ch; idx++, i_ch++) {
      usac_data->td_encoder[i_ch]->max_sfb_short =
          usac_data->str_psy_mod.str_psy_short_config[ch_idx].sfb_count;
      if (ptr_usac_config->tns_select == 0) {
        usac_data->pstr_tns_info[i_ch] = NULL;
      } else {
        usac_data->pstr_tns_info[i_ch]->sfb_offset_table_short =
            usac_data->str_psy_mod.str_psy_short_config[ch_idx].sfb_offset;
        usac_data->pstr_tns_info[i_ch]->sfb_offset_table_long =
            usac_data->str_psy_mod.str_psy_long_config[ch_idx].sfb_offset;
        usac_data->pstr_tns_info[i_ch]->max_sfb_short =
            usac_data->str_psy_mod.str_psy_short_config[ch_idx].sfb_count;
        usac_data->pstr_tns_info[i_ch]->max_sfb_long =
            usac_data->str_psy_mod.str_psy_long_config[ch_idx].sfb_count;

        err_code = iusace_tns_init(ptr_usac_config->core_sample_rate,
                            usac_data->str_qc_main.str_qc_data[ch_idx].ch_bitrate /
                                usac_data->str_qc_main.str_qc_data[ch_idx].num_ch,
                            usac_data->pstr_tns_info[i_ch],
                            usac_data->str_qc_main.str_qc_data[ch_idx].num_ch);
        if (err_code) {
          return err_code;
        }
      }
    }
  }

  for (i = 0; i < MAX_TIME_CHANNELS; i++) usac_data->str_quant_info[i].reset = 1;

  if (ptr_usac_config->codec_mode == USAC_SWITCHED ||
      ptr_usac_config->codec_mode == USAC_ONLY_TD) {
    for (i_ch = 0; i_ch < ptr_usac_config->channels; i_ch++) {
      if ((ptr_usac_config->core_sample_rate) < SR_MIN ||
          (ptr_usac_config->core_sample_rate) > SR_MAX) {
        return IA_EXHEAACE_CONFIG_FATAL_USAC_SAMP_FREQ;
      } else {
        usac_data->td_encoder[i_ch]->fscale = ptr_usac_config->core_sample_rate;

        iusace_init_td_data(usac_data->td_encoder[i_ch], ptr_usac_config->ccfl);
      }

      usac_data->td_bitrate[i_ch] = ptr_usac_config->bit_rate;
      usac_data->td_bitrate[i_ch] /= ptr_usac_config->channels;
      iusace_config_acelp_core_mode(usac_data->td_encoder[i_ch],
                                    ptr_usac_config->core_sample_rate,
                                    usac_data->td_bitrate[i_ch]);

      usac_data->acelp_core_mode[i_ch] = (usac_data->td_encoder[i_ch])->acelp_core_mode;
      if (ptr_usac_config->core_sample_rate == 8000) {
        usac_data->use_acelp_only = 1;
      }
    }
  } else {
    usac_data->acelp_core_mode[0] = 0;
  }

  for (UWORD32 ch = 0;
       ch < pstr_asc_usac_config->num_elements - ptr_usac_config->num_ext_elements; ch++) {
    iusace_qc_init(&usac_data->str_qc_main.str_qc_data[ch], MAX_CHANNEL_BITS,
                   ptr_usac_config->core_sample_rate, ptr_usac_config->bw_limit[ch],
                   usac_data->str_qc_main.str_qc_data[ch].num_ch, ptr_usac_config->ccfl);
  }

  return err_code;
}

static WORD32 iexheaax_append_bitstream(ixheaace_bit_buf_handle hdl_bitbuf_write,
                                        ixheaace_bit_buf_handle hdl_bitbuf_read,
                                        WORD32 num_bits) {
  WORD32 idx;
  UWORD32 value;

  if (num_bits > 16) {
    WORD32 cnt, rem;
    cnt = num_bits >> 4;
    rem = num_bits % 16;

    for (idx = 0; idx < cnt; idx++) {
      value = ixheaace_readbits(hdl_bitbuf_read, 16);
      ixheaace_write_bits(hdl_bitbuf_write, value, 16);
    }
    if (rem) {
      value = ixheaace_readbits(hdl_bitbuf_read, (UWORD8)rem);
      ixheaace_write_bits(hdl_bitbuf_write, value, (UWORD8)rem);
    }
  } else {
    value = ixheaace_readbits(hdl_bitbuf_read, (UWORD8)num_bits);
    ixheaace_write_bits(hdl_bitbuf_write, value, (UWORD8)num_bits);
  }

  return num_bits;
}

static IA_ERRORCODE iusace_enc_ext_elemts(UWORD32 usac_ext_ele_type,
                                          ia_usac_encoder_config_struct *pstr_usac_config,
                                          ia_usac_data_struct *pstr_usac_data,
                                          ixheaace_audio_specific_config_struct *pstr_asc,
                                          FLOAT32 **pptr_input, ia_bit_buf_struct *it_bit_buff,
                                          WORD32 *num_bits_written) {
  WORD8 idx = 0;
  LOOPIDX idx_2 = 0;
  WORD32 num_bits_payload = 0;
  WORD32 num_byts_payload = 0;
  ia_usac_config_struct *pstr_asc_usac_config = &(pstr_asc->str_usac_config);
  VOID *pstr_scratch = &pstr_usac_data->str_scratch;
  IA_ERRORCODE err_code = IA_NO_ERROR;

  for (idx = 0; idx < (WORD32)pstr_asc_usac_config->num_elements; idx++) {
    if (ID_USAC_EXT != pstr_asc_usac_config->usac_element_type[idx]) {
      continue;
    }

    ia_usac_enc_element_config_struct *pstr_usac_elem_config =
        &(pstr_asc_usac_config->str_usac_element_config[idx]);

    if (usac_ext_ele_type != pstr_usac_elem_config->usac_ext_ele_type) {
      continue;
    }

    switch (pstr_usac_elem_config->usac_ext_ele_type) {
      case ID_EXT_ELE_UNI_DRC: {
        if (pstr_usac_data->str_drc_state.is_first_drc_process_complete == 0) {
          iusace_reset_bit_buffer(&pstr_usac_data->str_drc_state.str_bit_buf_out);
          err_code = impd_drc_enc(&pstr_usac_data->str_drc_state, pptr_input, 0,
                                  &num_bits_payload, pstr_scratch);
          if (err_code) {
            return err_code;
          }
          pstr_usac_data->str_drc_state.is_first_drc_process_complete = 1;
          num_bits_payload = 0;
        }

        iusace_reset_bit_buffer(&pstr_usac_data->str_drc_state.str_bit_buf_out);
        err_code =
            impd_drc_enc(&pstr_usac_data->str_drc_state, pptr_input,
                         pstr_usac_config->drc_frame_size, &num_bits_payload, pstr_scratch);
        if (err_code) {
          return err_code;
        }
        num_byts_payload = (num_bits_payload + 7) >> 3;
      } break;
      default: {
      } break;
    }

    if (num_byts_payload <= 0) {
      *num_bits_written += iusace_write_bits_buf(it_bit_buff, 0, 1);  // usacExtElementPresent
    } else {
      *num_bits_written += iusace_write_bits_buf(it_bit_buff, 1, 1);  // usacExtElementPresent

      *num_bits_written +=
          iusace_write_bits_buf(it_bit_buff, 0, 1);  // usacExtElementUseDefaultLength

      if (num_byts_payload >= 255) {
        *num_bits_written +=
            iusace_write_bits_buf(it_bit_buff, 255, 8);  // usacExtElementPayloadLength

        UWORD16 value_add = (UWORD16)(num_byts_payload - 255 + 2);
        *num_bits_written += iusace_write_bits_buf(it_bit_buff, value_add, 16);
      } else {
        *num_bits_written += iusace_write_bits_buf(it_bit_buff, num_byts_payload,
                                                   8);  // usacExtElementPayloadLength
      }

      switch (pstr_usac_elem_config->usac_ext_ele_type) {
        case ID_EXT_ELE_UNI_DRC: {
          for (idx_2 = 0; idx_2 < num_byts_payload; idx_2++) {
            *num_bits_written += iusace_write_bits_buf(
                it_bit_buff, pstr_usac_data->str_drc_state.bit_buf_base_out[idx_2], 8);
          }
        } break;
        default: {
        } break;
      }
    }
  }

  return err_code;
}

IA_ERRORCODE ixheaace_usac_encode(FLOAT32 **ptr_input,
                                  ia_usac_encoder_config_struct *ptr_usac_config,
                                  ia_usac_data_struct *pstr_state,
                                  ixheaace_audio_specific_config_struct *pstr_asc,
                                  ia_bit_buf_struct *pstr_it_bit_buff,
                                  ixheaace_pstr_sbr_enc ptr_env_encoder, FLOAT32 **pp_drc_inp,
                                  WORD32 *is_quant_spec_zero, WORD32 *is_gain_limited) {
  IA_ERRORCODE err = IA_NO_ERROR;
  WORD32 i_ch, i, k;
  ia_usac_data_struct *ptr_usac_data = pstr_state;
  iusace_scratch_mem *pstr_scratch = &ptr_usac_data->str_scratch;
  WORD32 bits_written = 0;

  WORD32 *next_window_sequence = pstr_scratch->ptr_next_win_scratch;
  WORD32 *new_win_seq = pstr_scratch->ptr_next_win_scratch + MAX_TIME_CHANNELS;
  memset(next_window_sequence, 0, MAX_TIME_CHANNELS * sizeof(next_window_sequence));
  memset(new_win_seq, 0, MAX_TIME_CHANNELS * sizeof(new_win_seq));
  ia_sfb_params_struct *pstr_sfb_prms = &ptr_usac_config->str_sfb_prms;
  memset(pstr_sfb_prms, 0, sizeof(ia_sfb_params_struct));

  WORD32 *num_window_groups = pstr_sfb_prms->num_window_groups;
  WORD32 average_bits_total;
  WORD32 num_bits;
  WORD32 padding_bits;
  WORD32 *common_win = pstr_sfb_prms->common_win;
  WORD32 usac_independency_flg;
  WORD32 mod[NUM_FRAMES] = {0};
  WORD32 len_frame;
  WORD32 len_lpc0;
  WORD32 len_next_high_rate;
  WORD8 elem_idx, nr_core_coder_channels = 0, chn = 0;
  WORD32 ch_offset = 0;
  WORD32 elem_idx_max = ptr_usac_config->num_elements - ptr_usac_config->num_ext_elements;
  WORD32 td_buffer_offset = (TD_BUFFER_OFFSET * ptr_usac_config->ccfl) / FRAME_LEN_LONG;
  usac_independency_flg = ptr_usac_data->usac_independency_flag;

  len_frame = ptr_usac_config->ccfl;
  len_lpc0 = (LEN_LPC0 * len_frame) / FRAME_LEN_LONG;
  len_next_high_rate = (LEN_NEXT_HIGH_RATE * len_frame) / FRAME_LEN_LONG;

  average_bits_total =
      (ptr_usac_config->bit_rate * ptr_usac_config->ccfl) / ptr_usac_config->core_sample_rate;

  ptr_usac_data->min_bits_needed =
      (long)(ptr_usac_data->available_bitreservoir_bits + 2 * average_bits_total -
             ptr_usac_data->max_bitreservoir_bits);
  if (ptr_usac_data->min_bits_needed < 0) {
    ptr_usac_data->min_bits_needed = 0;
  }

  num_bits = 0;
  if (ptr_usac_config->preroll_flag) {
    if (ptr_usac_config->iframes_interval != ptr_usac_config->num_preroll_frames) {
      iusace_write_bits_buf(pstr_it_bit_buff, usac_independency_flg, 1);
      num_bits++;
    }
  } else {
    iusace_write_bits_buf(pstr_it_bit_buff, usac_independency_flg, 1);
    num_bits++;
  }
  for (elem_idx = 0; elem_idx < elem_idx_max; elem_idx++) {
    switch (ptr_usac_data->channel_elem_type[elem_idx]) {
      case USAC_ELEMENT_TYPE_SCE:
        nr_core_coder_channels = 1;
        break;
      case USAC_ELEMENT_TYPE_CPE:
        nr_core_coder_channels = 2;
        break;
    }

    if (ptr_usac_data->core_mode[0] == CORE_MODE_FD) {
      for (chn = 0, i_ch = ch_offset; chn < nr_core_coder_channels; chn++, i_ch++) {
        iusace_block_switching(&ptr_usac_data->block_switch_ctrl[i_ch], ptr_input[i_ch],
                               ptr_usac_config->ccfl);
      }
    }

    i_ch = ch_offset;
    if (nr_core_coder_channels == 2) {
      iusace_sync_block_switching(&ptr_usac_data->block_switch_ctrl[i_ch],
                                  &ptr_usac_data->block_switch_ctrl[i_ch + 1]);
    }

    for (chn = 0, i_ch = ch_offset; chn < nr_core_coder_channels; chn++, i_ch++) {
      switch (ptr_usac_config->codec_mode) {
        case USAC_SWITCHED:
          if (ptr_usac_data->str_sig_class_data.coding_mode == 2) {
            ptr_usac_data->core_mode_next[i_ch] = CORE_MODE_FD;
          } else {
            ptr_usac_data->core_mode_next[i_ch] = CORE_MODE_TD;
          }
          break;
        case USAC_ONLY_FD:
          ptr_usac_data->core_mode_next[i_ch] = CORE_MODE_FD;
          break;
        case USAC_ONLY_TD:
          ptr_usac_data->core_mode_next[i_ch] = CORE_MODE_TD;
          break;
        default:
          return IA_EXHEAACE_INIT_FATAL_USAC_INVALID_CODEC_MODE;
      }
      if (ptr_usac_data->core_mode[i_ch] == CORE_MODE_TD) {
        for (i = 0; i < ptr_usac_config->ccfl; i++) {
          ptr_usac_data->ptr_2frame_time_data[i_ch][i] = ptr_usac_data->ptr_time_data[i_ch][i];
          ptr_usac_data->ptr_2frame_time_data[i_ch][ptr_usac_config->ccfl + i] =
              ptr_usac_data->ptr_look_ahead_time_data[i_ch][i];
          ptr_usac_data->ptr_time_data[i_ch][i] =
              ptr_usac_data->ptr_look_ahead_time_data[i_ch][i];
          ptr_usac_data->ptr_look_ahead_time_data[i_ch][i] = (FLOAT64)ptr_input[i_ch][i];
        }
      } else {
        for (i = 0; i < ptr_usac_config->ccfl; i++) {
          ptr_usac_data->ptr_2frame_time_data[i_ch][i] = ptr_usac_data->ptr_time_data[i_ch][i];
          ptr_usac_data->ptr_2frame_time_data[i_ch][ptr_usac_config->ccfl + i] =
              ptr_usac_data->ptr_look_ahead_time_data[i_ch][i];
          ptr_usac_data->ptr_time_data[i_ch][i] = ptr_input[i_ch][i];
          ptr_usac_data->ptr_look_ahead_time_data[i_ch][i] = (FLOAT64)ptr_input[i_ch][i];
        }
      }

      for (i = 0; i < len_frame + len_next_high_rate; i++) {
        ptr_usac_data->td_in_buf[i_ch][i] =
            (FLOAT32)(ptr_usac_data->ptr_2frame_time_data[i_ch][i + td_buffer_offset]);
      }
      for (i = 0; i < len_frame + len_next_high_rate + len_lpc0; i++) {
        ptr_usac_data->td_in_prev_buf[i_ch][i] =
            (FLOAT32)(ptr_usac_data->ptr_2frame_time_data[i_ch][i + td_buffer_offset - len_lpc0]);
      }

      if (ptr_usac_data->core_mode[i_ch] == CORE_MODE_FD) {
        ptr_usac_data->window_size_samples[i_ch] = ptr_usac_config->ccfl;
        pstr_sfb_prms->window_sequence[i_ch] = ptr_usac_data->block_switch_ctrl[i_ch].window_seq;
        ptr_usac_config->window_sequence[i_ch] = pstr_sfb_prms->window_sequence[i_ch];
        new_win_seq[i_ch] = ptr_usac_data->block_switch_ctrl[i_ch].next_win_seq;
      }

      err = iusace_sfb_params_init(((LEN_SUPERFRAME_768 == ptr_usac_config->ccfl) &&
                                  (0 == ptr_usac_config->sbr_enable)) ?
                                  (iusace_map_sample_rate((ptr_usac_config->core_sample_rate
                                   * 4) / 3)) :(ptr_usac_config->core_sample_rate),
                                   ptr_usac_config->ccfl, pstr_sfb_prms->sfb_width_table[i_ch],
                                   &pstr_sfb_prms->num_sfb[i_ch],
                                   pstr_sfb_prms->window_sequence[i_ch]);

      if (err) {
        return err;
      }

      pstr_sfb_prms->sfb_offset[i_ch][0] = 0;
      k = 0;
      for (i = 0; i < pstr_sfb_prms->num_sfb[i_ch]; i++) {
        pstr_sfb_prms->sfb_offset[i_ch][i] = k;
        k += pstr_sfb_prms->sfb_width_table[i_ch][i];
      }
      pstr_sfb_prms->sfb_offset[i_ch][i] = k;

      if (ptr_usac_data->core_mode[i_ch] != CORE_MODE_TD) {
        next_window_sequence[i_ch] = new_win_seq[i_ch];
        if (ptr_usac_data->core_mode_next[i_ch] == CORE_MODE_TD) {
          next_window_sequence[i_ch] = EIGHT_SHORT_SEQUENCE;
        }

        if (ptr_usac_data->core_mode[i_ch] == CORE_MODE_TD &&
            ptr_usac_data->core_mode_next[i_ch] != CORE_MODE_TD) {
          next_window_sequence[i_ch] = LONG_STOP_SEQUENCE;
        }

        if (next_window_sequence[i_ch] == EIGHT_SHORT_SEQUENCE) {
          if (pstr_sfb_prms->window_sequence[i_ch] == ONLY_LONG_SEQUENCE) {
            pstr_sfb_prms->window_sequence[i_ch] = LONG_START_SEQUENCE;
          }
          if (pstr_sfb_prms->window_sequence[i_ch] == LONG_STOP_SEQUENCE) {
            pstr_sfb_prms->window_sequence[i_ch] = STOP_START_SEQUENCE;
          }
        }

        if (next_window_sequence[i_ch] == ONLY_LONG_SEQUENCE) {
          if (pstr_sfb_prms->window_sequence[i_ch] == EIGHT_SHORT_SEQUENCE) {
            next_window_sequence[i_ch] = LONG_STOP_SEQUENCE;
          }
        }

        if (pstr_sfb_prms->window_sequence[i_ch] == EIGHT_SHORT_SEQUENCE) {
          num_window_groups[i_ch] = ptr_usac_data->block_switch_ctrl[i_ch].tot_grps_cnt;
          for (i = 0; i < 8; i++) {
            pstr_sfb_prms->window_group_length[i_ch][i] =
                ptr_usac_data->block_switch_ctrl[i_ch].group_len[i];
          }
        } else {
          num_window_groups[i_ch] = 1;
          pstr_sfb_prms->window_group_length[i_ch][0] = 1;
        }

        pstr_sfb_prms->window_shape[i_ch] = ptr_usac_config->window_shape_prev[i_ch];

        err = iusace_fd_mdct(ptr_usac_data, ptr_usac_config, i_ch);

        if (err) {
          return err;
        }

        if (pstr_sfb_prms->window_sequence[i_ch] != EIGHT_SHORT_SEQUENCE) {
          iusace_psy_mod_lb(&ptr_usac_data->str_psy_mod, pstr_sfb_prms,
                            ptr_usac_data->spectral_line_vector[i_ch],
                            ptr_usac_data->pstr_tns_info, ptr_usac_config->tns_select, i_ch, chn,
                            ptr_usac_data->channel_elem_type[elem_idx],
                            pstr_scratch->p_tns_filter, elem_idx, pstr_scratch->ptr_tns_scratch,
                            ptr_usac_config->ccfl);
        } else {
          iusace_psy_mod_sb(&(ptr_usac_data->str_psy_mod), pstr_sfb_prms,
                            ptr_usac_data->spectral_line_vector[i_ch],
                            ptr_usac_data->pstr_tns_info, ptr_usac_config->tns_select, i_ch, chn,
                            ptr_usac_data->channel_elem_type[elem_idx],
                            pstr_scratch->p_tns_filter, elem_idx, pstr_scratch->ptr_tns_scratch,
                            ptr_usac_config->ccfl);
        }

        pstr_sfb_prms->max_sfb[i_ch] =
            ptr_usac_data->str_psy_mod.str_psy_out_data[i_ch].max_sfb_per_grp;
      }
    }
    for (chn = 0, i_ch = ch_offset; chn < nr_core_coder_channels; chn++, i_ch++) {
      if (nr_core_coder_channels == 2) {
        if ((pstr_sfb_prms->window_shape[i_ch] == pstr_sfb_prms->window_shape[i_ch + 1]) &&
            (pstr_sfb_prms->window_sequence[i_ch] == pstr_sfb_prms->window_sequence[i_ch + 1]) &&
            (ptr_usac_data->core_mode[i_ch] == ptr_usac_data->core_mode[i_ch + 1])) {
          common_win[i_ch] = common_win[i_ch + 1] = 1;
        } else {
          common_win[i_ch] = 0;
        }
        chn++;
      } else {
        common_win[i_ch] = 0;
      }
    }
    if (nr_core_coder_channels == 2) {
      if (i_ch == (ch_offset + 1)) {
        if (pstr_sfb_prms->window_sequence[i_ch] != EIGHT_SHORT_SEQUENCE) {
          iusace_calc_ms_band_energy(
              ptr_usac_data->spectral_line_vector[ch_offset],
              ptr_usac_data->spectral_line_vector[ch_offset + 1],
              ptr_usac_data->str_psy_mod.str_psy_long_config[elem_idx].sfb_offset,
              ptr_usac_data->str_psy_mod.str_psy_long_config[elem_idx].sfb_active,
              ptr_usac_data->str_psy_mod.str_psy_data[ch_offset].ptr_sfb_energy_long_ms,
              ptr_usac_data->str_psy_mod.str_psy_data[ch_offset + 1].ptr_sfb_energy_long_ms);
        } else {
          WORD32 frame_len_short = (ptr_usac_config->ccfl * FRAME_LEN_SHORT_128) / FRAME_LEN_LONG;
          for (WORD32 w = 0; w < MAX_SHORT_WINDOWS; w++) {
            WORD32 w_offset = w * frame_len_short;

            iusace_calc_ms_band_energy(
                ptr_usac_data->spectral_line_vector[ch_offset] + w_offset,
                ptr_usac_data->spectral_line_vector[ch_offset + 1] + w_offset,
                ptr_usac_data->str_psy_mod.str_psy_short_config[elem_idx].sfb_offset,
                ptr_usac_data->str_psy_mod.str_psy_short_config[elem_idx].sfb_active,
                ptr_usac_data->str_psy_mod.str_psy_data[ch_offset].ptr_sfb_energy_short_ms[w],
                ptr_usac_data->str_psy_mod.str_psy_data[ch_offset + 1]
                    .ptr_sfb_energy_short_ms[w]);
          }
        }
      }
    }
    if ((nr_core_coder_channels == 2)
            ? ((ptr_usac_data->core_mode[ch_offset] == CORE_MODE_FD) &&
               (ptr_usac_data->core_mode[ch_offset + 1] == CORE_MODE_FD))
            : ((ptr_usac_data->core_mode[ch_offset] == CORE_MODE_FD))) {
      iusace_grouping(pstr_sfb_prms, nr_core_coder_channels, ptr_usac_data, ptr_usac_config,
                      ch_offset, elem_idx);

      if (nr_core_coder_channels == 2) {
        err = iusace_stereo_proc(pstr_sfb_prms, usac_independency_flg, ptr_usac_data,
                                 ptr_usac_config, ch_offset);
        if (err != IA_NO_ERROR) {
          return err;
        }
      }
    }
    if (ptr_usac_config->preroll_flag) {
      if (ptr_usac_config->iframes_interval != ptr_usac_config->num_preroll_frames) {
        iusace_write_bits_buf(pstr_it_bit_buff, 0, 1);  // extension element present
        num_bits++;
      }
    }
    if (ptr_usac_config->use_drc_element) {
      WORD32 num_bits_ext_elem = 0;
      err = iusace_enc_ext_elemts(ID_EXT_ELE_UNI_DRC, ptr_usac_config, pstr_state, pstr_asc,
                                  pp_drc_inp, pstr_it_bit_buff, &num_bits_ext_elem);
      if (err & IA_FATAL_ERROR) {
        return err;
      }
      num_bits += num_bits_ext_elem;
      ptr_usac_data->num_drc_bits = num_bits_ext_elem;
#ifdef DRC_BITRATE_CONSIDERATION
      ptr_usac_data->drc_data_bit_cnt = num_bits_ext_elem;
#endif
    }

    for (chn = 0, i_ch = ch_offset; chn < nr_core_coder_channels; chn++, i_ch++) {
      iusace_write_bits_buf(pstr_it_bit_buff, ptr_usac_data->core_mode[i_ch], 1);
      num_bits++;
    }

    for (chn = 0, i_ch = ch_offset; chn < nr_core_coder_channels; chn++, i_ch++) {
      if (ptr_usac_data->core_mode[i_ch] == CORE_MODE_FD) {
        ptr_usac_data->window_size_samples[i_ch] = ptr_usac_config->ccfl;
        pstr_sfb_prms->window_sequence[i_ch] = ptr_usac_data->block_switch_ctrl[i_ch].window_seq;
        ptr_usac_config->window_sequence[i_ch] = pstr_sfb_prms->window_sequence[i_ch];
        new_win_seq[i_ch] = ptr_usac_data->block_switch_ctrl[i_ch].next_win_seq;
      }

      if (ptr_usac_data->core_mode[i_ch] == CORE_MODE_TD) {
        WORD32 error;

        error = iusace_lpd_frm_enc(ptr_usac_data, mod, usac_independency_flg, len_frame, i_ch,
                                   pstr_it_bit_buff);
        if (error) return error;

        num_bits = pstr_it_bit_buff->cnt_bits;

        if ((ptr_usac_data->core_mode_prev[i_ch] == CORE_MODE_FD) && (mod[0] == 0)) {
          for (i = 0; i < ptr_usac_data->num_td_fac_bits[i_ch]; i++) {
            iusace_write_bits_buf(pstr_it_bit_buff, ptr_usac_data->fac_out_stream[i_ch][i], 1);
            num_bits++;
          }
        }
      } else {
        next_window_sequence[i_ch] = new_win_seq[i_ch];
        if (ptr_usac_data->core_mode_next[i_ch] == CORE_MODE_TD) {
          next_window_sequence[i_ch] = EIGHT_SHORT_SEQUENCE;
        }

        if (ptr_usac_data->core_mode[i_ch] == CORE_MODE_TD &&
            ptr_usac_data->core_mode_next[i_ch] != CORE_MODE_TD) {
          next_window_sequence[i_ch] = LONG_STOP_SEQUENCE;
        }

        if (next_window_sequence[i_ch] == EIGHT_SHORT_SEQUENCE) {
          if (pstr_sfb_prms->window_sequence[i_ch] == ONLY_LONG_SEQUENCE) {
            pstr_sfb_prms->window_sequence[i_ch] = LONG_START_SEQUENCE;
          }
          if (pstr_sfb_prms->window_sequence[i_ch] == LONG_STOP_SEQUENCE) {
            pstr_sfb_prms->window_sequence[i_ch] = STOP_START_SEQUENCE;
          }
        }
        if (next_window_sequence[i_ch] == ONLY_LONG_SEQUENCE) {
          if (pstr_sfb_prms->window_sequence[i_ch] == EIGHT_SHORT_SEQUENCE) {
            next_window_sequence[i_ch] = LONG_STOP_SEQUENCE;
          }
        }
        if (pstr_sfb_prms->window_sequence[i_ch] == EIGHT_SHORT_SEQUENCE) {
          num_window_groups[i_ch] = ptr_usac_data->block_switch_ctrl[i_ch].tot_grps_cnt;
          for (i = 0; i < 8; i++) {
            pstr_sfb_prms->window_group_length[i_ch][i] =
                ptr_usac_data->block_switch_ctrl[i_ch].group_len[i];
          }
        } else {
          num_window_groups[i_ch] = 1;
          pstr_sfb_prms->window_group_length[i_ch][0] = 1;
        }
        pstr_sfb_prms->window_shape[i_ch] = ptr_usac_config->window_shape_prev[i_ch];
      }
    }

    if ((nr_core_coder_channels == 2)
            ? ((ptr_usac_data->core_mode[ch_offset] == CORE_MODE_FD) &&
               (ptr_usac_data->core_mode[ch_offset + 1] == CORE_MODE_FD))
            : ((ptr_usac_data->core_mode[ch_offset] == CORE_MODE_FD))) {
      err = iusace_fd_encode(pstr_sfb_prms, usac_independency_flg, ptr_usac_data, ptr_usac_config,
                             pstr_it_bit_buff, nr_core_coder_channels, ch_offset, elem_idx,
                             &bits_written, is_quant_spec_zero, is_gain_limited);

      if (err) {
        return err;
      }

      num_bits += bits_written;
    }

    for (chn = 0, i_ch = ch_offset; chn < nr_core_coder_channels; chn++, i_ch++) {
      ptr_usac_config->window_shape_prev[i_ch] = pstr_sfb_prms->window_shape[i_ch];
      ptr_usac_config->window_sequence_prev[i_ch] = ptr_usac_config->window_sequence[i_ch];
      ptr_usac_config->window_sequence[i_ch] = next_window_sequence[i_ch];
      ptr_usac_data->core_mode_prev[i_ch] = ptr_usac_data->core_mode[i_ch];
      ptr_usac_data->core_mode[i_ch] = ptr_usac_data->core_mode_next[i_ch];
    }
    ch_offset += nr_core_coder_channels;
  }

  if (1 == ptr_usac_config->sbr_enable) {
    // Append SBR bits
    ixheaace_bit_buf_handle pstr_it_bit_buff_temp =
        &ptr_env_encoder->str_cmon_data.str_sbr_bit_buf;
    WORD32 check_num_bits = ia_enhaacplus_enc_get_bits_available(pstr_it_bit_buff_temp);

    num_bits += iexheaax_append_bitstream((ixheaace_bit_buf_handle)pstr_it_bit_buff,
                                          pstr_it_bit_buff_temp, check_num_bits);
  }

  if (ptr_usac_config->use_fill_element) {
    WORD32 full_elem_num_bits = 0;
    padding_bits = ptr_usac_data->min_bits_needed - num_bits;
    full_elem_num_bits = iusace_write_fill_ele(pstr_it_bit_buff, padding_bits);
    num_bits += full_elem_num_bits;
  }

  ptr_usac_data->available_bitreservoir_bits -= num_bits;

  if (num_bits % 8) {
    ptr_usac_data->available_bitreservoir_bits -= 8 - (num_bits % 8);
  }
  ptr_usac_data->available_bitreservoir_bits += average_bits_total;

  if (ptr_usac_data->available_bitreservoir_bits > ptr_usac_data->max_bitreservoir_bits) {
    ptr_usac_data->available_bitreservoir_bits = ptr_usac_data->max_bitreservoir_bits;
  }

  return err;
}

IA_ERRORCODE iusace_validate_baseline_profile_sample_rate(WORD32 sample_rate) {
  IA_ERRORCODE err_code = IA_NO_ERROR;

  if ((sample_rate != 7350) && (sample_rate != 8000) && (sample_rate != 9600) &&
      (sample_rate != 11025) && (sample_rate != 12000) && (sample_rate != 12800) &&
      (sample_rate != 16000) && (sample_rate != 19200) && (sample_rate != 22050) &&
      (sample_rate != 24000) && (sample_rate != 32000) && (sample_rate != 38400) &&
      (sample_rate != 44100) && (sample_rate != 48000) && (sample_rate != 64000) &&
      (sample_rate != 88200) && (sample_rate != 96000)) {
    err_code = IA_EXHEAACE_CONFIG_FATAL_USAC_BASELINE_INVALID_SAMP_FREQ;
  }

  return err_code;
}