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

#pragma once

typedef struct {
  WORD32 window_size_samples[MAX_TIME_CHANNELS];
  WORD32 usac_independency_flag_interval;
  WORD32 usac_independency_flag_count;
  WORD32 usac_independency_flag;
  WORD32 frame_count;
  WORD32 core_mode[MAX_TIME_CHANNELS];
  WORD32 core_mode_prev[MAX_TIME_CHANNELS];
  WORD32 core_mode_prev_copy[MAX_TIME_CHANNELS];
  WORD32 core_mode_next[MAX_TIME_CHANNELS];
  WORD32 core_mode_copy[MAX_TIME_CHANNELS];
  ia_block_switch_ctrl block_switch_ctrl[MAX_TIME_CHANNELS];
  ia_classification_struct str_sig_class_data;
  FLOAT32 td_in_buf[MAX_TIME_CHANNELS][LEN_SUPERFRAME + LEN_NEXT_HIGH_RATE];
  FLOAT32 td_in_prev_buf[MAX_TIME_CHANNELS][LEN_SUPERFRAME + LEN_NEXT_HIGH_RATE + LEN_LPC0];
  FLOAT32 speech_buf[LEN_TOTAL_HIGH_RATE + LEN_LPC0];
  FLOAT32 synth_buf[ORDER + LEN_SUPERFRAME];
  WORD32 param_buf[(NUM_FRAMES * MAX_NUM_TCX_PRM_PER_DIV) + NUM_LPC_PRM];
  ia_usac_td_encoder_struct *td_encoder[MAX_TIME_CHANNELS];
  WORD32 total_nbbits[MAX_TIME_CHANNELS];
  WORD32 FD_nbbits_fac[MAX_TIME_CHANNELS];
  WORD32 num_td_fac_bits[MAX_TIME_CHANNELS];
  WORD32 td_bitrate[MAX_TIME_CHANNELS];
  WORD32 acelp_core_mode[MAX_TIME_CHANNELS];
  WORD32 max_bitreservoir_bits;
  WORD32 available_bitreservoir_bits;
  ia_drc_enc_state str_drc_state;
  WORD32 num_sbr_bits;
  ia_ms_info_struct str_ms_info[MAX_TIME_CHANNELS];
  WORD32 pred_coef_re[MAX_TIME_CHANNELS][MAX_SHORT_WINDOWS][MAX_SFB_LONG],
      pred_coef_im[MAX_TIME_CHANNELS][MAX_SHORT_WINDOWS][MAX_SFB_LONG];
  WORD32 pred_coef_re_prev[MAX_TIME_CHANNELS][MAX_SFB_LONG],
      pred_coef_im_prev[MAX_TIME_CHANNELS][MAX_SFB_LONG];
  /* Temporary buffers for bitstream writing function when computing static bits */
  WORD32 temp_pred_coef_re_prev[MAX_TIME_CHANNELS][MAX_SFB_LONG],
      temp_pred_coef_im_prev[MAX_TIME_CHANNELS][MAX_SFB_LONG];
  WORD32 pred_dir_idx[MAX_TIME_CHANNELS];
  WORD32 cplx_pred_all[MAX_TIME_CHANNELS];
  WORD32 cplx_pred_used[MAX_TIME_CHANNELS][MAX_SHORT_WINDOWS][MAX_SFB_LONG];
  WORD32 delta_code_time[MAX_TIME_CHANNELS];
  WORD32 complex_coef[MAX_TIME_CHANNELS];
  FLOAT64 *ptr_dmx_re_save[MAX_TIME_CHANNELS]; /*For saving previous frame MDCT down-mix */
  FLOAT64 *ptr_dmx_im[MAX_TIME_CHANNELS];
  FLOAT64 arr_dmx_im[MAX_TIME_CHANNELS][(FRAME_LEN_LONG + FRAME_LEN_LONG / 8)];
  FLOAT64 arr_dmx_save_float[MAX_TIME_CHANNELS][(FRAME_LEN_LONG + FRAME_LEN_LONG / 8)];
  FLOAT64 left_chan_save[MAX_TIME_CHANNELS][(FRAME_LEN_LONG + FRAME_LEN_LONG / 8)];
  FLOAT64 right_chan_save[MAX_TIME_CHANNELS][(FRAME_LEN_LONG + FRAME_LEN_LONG / 8)];

  ia_tns_info *pstr_tns_info[MAX_TIME_CHANNELS];
  WORD32 common_window[MAX_TIME_CHANNELS];
  WORD32 noise_offset[MAX_TIME_CHANNELS];
  WORD32 noise_level[MAX_TIME_CHANNELS];

  ia_usac_quant_info_struct str_quant_info[MAX_TIME_CHANNELS];
  WORD32 noise_filling[MAX_TIME_CHANNELS];
  ia_psy_mod_struct str_psy_mod;
  ia_qc_main_struct str_qc_main;
  FLOAT64 *ptr_time_data[MAX_TIME_CHANNELS];
  FLOAT64 *ptr_look_ahead_time_data[MAX_TIME_CHANNELS];
  FLOAT64 *spectral_line_vector[MAX_TIME_CHANNELS];
  // Pre-/post- twiddle portions of MDCT use two times ccfl of this buffer, hence size of second
  // argument is 2 * pstr_config->ccfl
  FLOAT64 mdst_spectrum[MAX_TIME_CHANNELS][2 * FRAME_LEN_LONG];
  FLOAT64 *ptr_2frame_time_data[MAX_TIME_CHANNELS];
  WORD16 td_serial_out[MAX_TIME_CHANNELS][NBITS_MAX];
  WORD16 fac_out_stream[MAX_TIME_CHANNELS][NBITS_MAX];
  FLOAT64 overlap_buf[MAX_TIME_CHANNELS][2 * FRAME_LEN_LONG];
  WORD32 channel_elem_type[MAX_TIME_CHANNELS];
  WORD32 channel_elem_idx[MAX_TIME_CHANNELS];
  WORD32 num_ext_elements;
  WORD32 ext_type[MAX_EXTENSION_PAYLOADS];
  UWORD8 ext_elem_config_payload[MAX_EXTENSION_PAYLOADS][MAX_EXTENSION_PAYLOAD_LEN];
  UWORD32 ext_elem_config_len[MAX_EXTENSION_PAYLOADS];
  iusace_scratch_mem str_scratch;
  WORD32 min_bits_needed;
  WORD32 num_drc_bits;
  WORD32 use_acelp_only;
  WORD32 prev_out_bytes[MAX_PREROLL_FRAMES];
  UWORD8 prev_out_data[MAX_PREROLL_FRAMES][MAX_OUTPUT_BYTES_PER_CH * IXHEAACE_MAX_CH_IN_BS_ELE];
  UWORD32 stereo_config_index;
} ia_usac_data_struct;

typedef struct {
  ia_usac_lpd_state_struct lpd_state[6];
  ia_usac_lpd_state_struct flpd_state[6];

} ia_usac_lpd_scratch;

IA_ERRORCODE iusace_enc_init(ia_usac_encoder_config_struct *ptr_usac_config,
                             ixheaace_audio_specific_config_struct *pstr_asc,
                             ia_usac_data_struct *pstr_state);

IA_ERRORCODE iusace_quantize_spec(ia_sfb_params_struct *pstr_sfb_prms,
                                  WORD32 usac_independancy_flag, WORD32 num_chans,
                                  ia_usac_data_struct *ptr_usac_data,
                                  ia_usac_encoder_config_struct *ptr_usac_config, WORD32 chn,
                                  WORD32 ele_id, WORD32 *is_quant_spec_zero,
                                  WORD32 *is_gain_limited);

IA_ERRORCODE iusace_grouping(ia_sfb_params_struct *pstr_sfb_prms, WORD32 num_chans,
                             ia_usac_data_struct *ptr_usac_data,
                             ia_usac_encoder_config_struct *ptr_usac_config, WORD32 chn,
                             WORD32 ele_id);

IA_ERRORCODE iusace_stereo_proc(ia_sfb_params_struct *pstr_sfb_prms,
                                WORD32 usac_independancy_flag, ia_usac_data_struct *ptr_usac_data,
                                ia_usac_encoder_config_struct *ptr_usac_config, WORD32 chn);

VOID iusace_classification(ia_classification_struct *pstr_sig_class,
                           iusace_scratch_mem *pstr_scratch, WORD32 ccfl);
