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
#define BS_MAX_NUM_OUT_CHANNELS (255)
#define MINIMUM_BITRATE 8000

typedef struct {
  FLOAT64 *p_fd_mdct_windowed_long_buf;
  FLOAT64 *p_fd_mdct_windowed_short_buf;
  FLOAT32 *p_fft_mdct_buf;
  FLOAT64 *p_sort_grouping_scratch;
  WORD32 *p_degroup_scratch;
  WORD32 *p_arith_map_prev_scratch;
  WORD32 *p_arith_map_pres_scratch;
  FLOAT64 *p_noise_filling_highest_tone;
  FLOAT32 *p_lpd_frm_enc_scratch;
  FLOAT64 *p_quant_spectrum_spec_scratch;
  UWORD8 *ptr_scratch_buf;
  FLOAT32 *p_synth_tcx_buf;
  FLOAT32 *p_synth_buf;
  FLOAT32 *p_wsig_buf;
  FLOAT32 *p_wsyn_buf;
  FLOAT32 *p_wsyn_tcx_buf;
  FLOAT32 *p_temp_wsyn_buf;
  FLOAT32 *p_buf_aut_corr;
  FLOAT32 *p_buf_synthesis_tool;
  FLOAT32 *p_buf_speech;
  FLOAT32 *p_buf_res;
  FLOAT32 *p_buf_signal;
  FLOAT32 *p_lp_filter_coeff;
  FLOAT32 *p_lp_filter_coeff_q;
  WORD32 *p_prm_tcx;
  FLOAT32 *p_wsp_prev_buf;
  FLOAT32 *p_xn2;
  FLOAT32 *p_fac_dec;
  FLOAT32 *p_right_fac_spec;
  FLOAT32 *p_x2;
  WORD32 *p_param;
  FLOAT32 *p_x;
  FLOAT32 *p_xn_2;
  FLOAT32 *p_fac_window;
  FLOAT32 *p_temp_mdct;
  WORD16 *p_fac_bits_word;
  FLOAT64 *p_left_fac_time_data;
  FLOAT32 *p_left_fac_timedata_flt;
  FLOAT32 *p_left_fac_spec;
  FLOAT64 *p_fac_win;
  WORD32 *p_fac_prm;
  FLOAT32 *p_acelp_folded_scratch;
  FLOAT32 *p_xn1_tcx;
  FLOAT32 *p_xn_buf_tcx;
  FLOAT32 *p_x_tcx;
  FLOAT32 *p_x_tmp_tcx;
  FLOAT32 *p_en_tcx;
  FLOAT32 *p_alfd_gains_tcx;
  FLOAT32 *p_sq_enc_tcx;
  WORD32 *p_sq_quant_tcx;
  FLOAT32 *p_gain1_tcx;
  FLOAT32 *p_gain2_tcx;
  FLOAT32 *p_facelp_tcx;
  FLOAT32 *p_xn2_tcx;
  FLOAT32 *p_fac_window_tcx;
  FLOAT32 *p_x1_tcx;
  FLOAT32 *p_x2_tcx;
  WORD32 *p_y_tcx;
  FLOAT32 *p_in_out_tcx;
  FLOAT32 *p_time_signal;
  FLOAT32 *p_complex_fft;
  WORD32 *p_tonal_flag;
  FLOAT32 *p_pow_spec;
  FLOAT64 *p_tns_scratch;
  FLOAT64 *p_tns_filter;
  FLOAT32 *p_exp_spec;
  FLOAT32 *p_mdct_spec_float;
  FLOAT32 *p_fir_sig_buf;
  FLOAT32 *p_sq_gain_en;
  FLOAT32 *p_acelp_ir_buf;
  FLOAT32 *p_acelp_exc_buf;
  FLOAT32 *p_adjthr_ptr_exp_spec;
  FLOAT32 *p_adjthr_mdct_spec_float;
  WORD16 *p_adjthr_quant_spec_temp;
  FLOAT64 *p_cmpx_mdct_temp_buf;
  FLOAT32 *p_fft_p2_y;
  FLOAT32 *p_fft_p3_data_3;
  FLOAT32 *p_fft_p3_y;
  FLOAT32 *p_tcx_input;
  FLOAT32 *p_tcx_output;
  FLOAT64 *p_reconstructed_time_signal[MAX_TIME_CHANNELS];
  FLOAT32 *p_ol_pitch_buf_tmp;
  FLOAT32 *p_ol_pitch_speech_buf;
  FLOAT32 *p_ol_pitch_w_table;
  FLOAT32 *p_ol_pitch_R;
  FLOAT32 *p_ol_pitch_R0;
  WORD32 *ptr_num_fac_bits;
  WORD32 *ptr_tns_data_present;
  FLOAT32 *ptr_tmp_lp_res;

  FLOAT32 *ptr_sfb_form_fac[MAX_TIME_CHANNELS];
  FLOAT32 *ptr_sfb_num_relevant_lines[MAX_TIME_CHANNELS];
  FLOAT32 *ptr_sfb_ld_energy[MAX_TIME_CHANNELS];
  WORD32 *ptr_num_scfs;
  WORD32 *ptr_max_ch_dyn_bits;
  FLOAT32 *ptr_ch_bit_dist;
  pUWORD8 ptr_fd_scratch;
  pUWORD8 ptr_lpd_scratch;
  FLOAT32 *ptr_tcx_scratch;
  FLOAT64 *ptr_tns_scratch;
  WORD32 *ptr_next_win_scratch;
  FLOAT32 *ptr_acelp_scratch;
  FLOAT32 mixed_rad_fft[2 * LEN_SUPERFRAME];
  pVOID drc_scratch;
  VOID *ptr_drc_scratch_buf;
  VOID *ptr_stack_mem;
} iusace_scratch_mem;

#define USAC_MAX_ELEMENTS (32)
#define USAC_MAX_CONFIG_EXTENSIONS (16)

#define ID_USAC_SCE 0
#define ID_USAC_CPE 1
#define ID_USAC_EXT 3

#define AOT_SBR (5)
#define AOT_USAC (42)

#define ID_EXT_ELE_FILL 0
#define ID_EXT_ELE_UNI_DRC 4
#define ID_EXT_ELE_AUDIOPREROLL (3)

#define ID_CONFIG_EXT_FILL 0
#define ID_CONFIG_EXT_DOWNMIX (1)
#define ID_CONFIG_EXT_LOUDNESS_INFO (2)
#define ID_CONFIG_EXT_STREAM_ID (7)
#define CONFIG_EXT_LEN_STREAM_ID (2)
#define NUM_COEFF (1024)

typedef enum {

  USAC_ELEMENT_TYPE_INVALID = -1,
  USAC_ELEMENT_TYPE_SCE = 0,
  USAC_ELEMENT_TYPE_CPE = 1,
  USAC_ELEMENT_TYPE_EXT = 3

} ia_usac_ele_type;

typedef struct {
  UWORD32 harmonic_sbr;
  UWORD32 bs_inter_tes;
  UWORD32 bs_pvc;
  UWORD32 dflt_start_freq;
  UWORD32 dflt_stop_freq;
  UWORD32 dflt_header_extra1;
  UWORD32 dflt_header_extra2;
  UWORD32 dflt_freq_scale;
  UWORD32 dflt_alter_scale;
  UWORD32 dflt_noise_bands;
  UWORD32 dflt_limiter_bands;
  UWORD32 dflt_limiter_gains;
  UWORD32 dflt_interpol_freq;
  UWORD32 dflt_smoothing_mode;
} ia_usac_enc_sbr_config_struct;

typedef struct {
  WORD32 bs_tree_config;
  WORD32 bs_freq_res;
  WORD32 bs_fixed_gain_dmx;
  WORD32 bs_temp_shape_config;
  WORD32 bs_decorr_config;
  WORD32 bs_residual_coding;
  WORD32 bs_residual_bands;
  WORD32 bs_low_rate_mode;
  WORD32 bs_phase_coding;
  WORD32 bs_quant_coarse_xxx;
  WORD32 bs_ott_bands_phase;
  WORD32 bs_ott_bands_phase_present;
  WORD32 bs_pseudo_lr;
  WORD32 bs_env_quant_mode;
  WORD32 bs_high_rate_mode;
} ia_usac_enc_mps_config_struct;

typedef struct {
  UWORD32 usac_ext_ele_type;
  UWORD32 usac_ext_ele_cfg_len;
  UWORD32 usac_ext_ele_dflt_len_present;
  UWORD32 usac_ext_ele_dflt_len;
  UWORD32 usac_ext_ele_payload_present;
  UWORD32 stereo_config_index;
  UWORD32 tw_mdct;
  UWORD32 noise_filling;
  UWORD8 usac_ext_ele_cfg_payload[6144 / 8];
  ia_usac_enc_sbr_config_struct str_usac_sbr_config;
  ia_usac_enc_mps_config_struct str_usac_mps212_config;
  UWORD8 *drc_config_data;
} ia_usac_enc_element_config_struct;

typedef struct {
  UWORD32 num_elements;
  UWORD32 num_ext_elements;
  UWORD32 usac_element_type[USAC_MAX_ELEMENTS];
  UWORD32 usac_cfg_ext_present;
  UWORD32 num_config_extensions;
  UWORD32 usac_config_ext_type[USAC_MAX_CONFIG_EXTENSIONS];
  UWORD32 usac_config_ext_len[USAC_MAX_CONFIG_EXTENSIONS];
  UWORD8 *usac_config_ext_buf[USAC_MAX_CONFIG_EXTENSIONS];
  UWORD8 usac_cfg_ext_info_buf[USAC_MAX_CONFIG_EXTENSIONS][6144 / 8];
  WORD32 num_out_channels;
  WORD32 num_signal_grp;
  WORD32 output_channel_pos[BS_MAX_NUM_OUT_CHANNELS];
  WORD32 ccfl;
  ia_usac_enc_element_config_struct str_usac_element_config[USAC_MAX_ELEMENTS];
  UWORD16 stream_identifier;
} ia_usac_config_struct;

typedef struct {
  WORD32 aac_allow_scalefacs;
  WORD32 aac_scale_facs;
  WORD32 bit_rate;
  WORD32 basic_bitrate;
  WORD32 bw_limit[USAC_MAX_ELEMENTS];
  WORD32 ccfl;
  WORD32 ccfl_idx;
  WORD32 channels;
  WORD32 codec_mode;
  WORD32 flag_noiseFilling;
  WORD32 iframes_interval;
  UWORD32 num_elements;
  UWORD32 num_ext_elements;

  WORD32 sample_rate;
  WORD32 native_sample_rate;
  WORD32 core_sample_rate;

  WORD32 tns_select;
  WORD32 ui_pcm_wd_sz;
  WORD32 use_fill_element;
  WORD32 window_shape_prev[MAX_TIME_CHANNELS];
  WORD32 window_shape_prev_copy[MAX_TIME_CHANNELS];
  WORD32 window_sequence[MAX_TIME_CHANNELS];
  WORD32 window_sequence_prev[MAX_TIME_CHANNELS];
  WORD32 window_sequence_prev_copy[MAX_TIME_CHANNELS];
  WORD32 cmplx_pred_flag;
  WORD32 wshape_flag;
  WORD32 delay_total;
  WORD32 in_frame_length;
  // eSBR Parameters
  WORD32 sbr_enable;
  WORD32 sbr_ratio_idx;
  WORD32 up_sample_ratio;
  WORD32 sbr_pvc_active;
  WORD32 sbr_harmonic;
  WORD32 hq_esbr;
  WORD32 sbr_inter_tes_active;
  // MPS Parameters
  WORD32 usac212enable;
  ia_sfb_params_struct str_sfb_prms;
  // DRC Params
  FLAG use_drc_element;
  WORD32 drc_frame_size;
  ia_drc_input_config str_drc_cfg;
  WORD32 use_acelp_only;
  WORD32 random_access_interval;
  WORD32 preroll_flag;
  WORD32 num_preroll_frames;
  WORD32 preroll_idx;
  WORD32 is_ipf;
  WORD32 preroll_frame;
  WORD32 is_first_frame;
  UWORD16 stream_id;
  FLAG use_delay_adjustment;
  FLAG is_loudness_configured;
} ia_usac_encoder_config_struct;

typedef struct {
  WORD32 mode;
  WORD32 num_bits;
  FLOAT32 lpc_coeffs_quant[2 * (ORDER + 1)];
  FLOAT32 lpc_coeffs[2 * (ORDER + 1)];
  FLOAT32 synth[ORDER + 128];
  FLOAT32 wsynth[1 + 128];
  FLOAT32 acelp_exc[2 * LEN_FRAME];
  WORD32 avq_params[FAC_LENGTH];
  FLOAT32 tcx_mem[128];
  FLOAT32 tcx_quant[1 + (2 * 128)];
  FLOAT32 tcx_fac;
  FLOAT32 mem_wsyn;
} ia_usac_lpd_state_struct;

typedef struct {
  WORD32 len_frame;
  WORD32 len_subfrm;
  WORD32 num_subfrm;
  WORD16 acelp_core_mode;
  WORD32 fscale;
  FLOAT32 mem_lp_decim2[3];
  WORD32 decim_frac;
  FLOAT32 mem_sig_in[4];
  FLOAT32 mem_preemph;
  FLOAT32 old_speech_pe[L_OLD_SPEECH_HIGH_RATE + LEN_LPC0];
  FLOAT32 weighted_sig[128];
  ia_usac_lpd_state_struct lpd_state;
  FLOAT32 prev_wsp[MAX_PITCH / OPL_DECIM];
  FLOAT32 prev_exc[MAX_PITCH + LEN_INTERPOL];
  FLOAT32 prev_wsyn_mem;
  FLOAT32 prev_wsp_mem;
  FLOAT32 prev_xnq_mem;
  WORD32 prev_ovlp_size;
  FLOAT32 isf_old[ORDER];
  FLOAT32 isp_old[ORDER];
  FLOAT32 isp_old_q[ORDER];
  FLOAT32 mem_wsp;
  FLOAT32 ada_w;
  FLOAT32 ol_gain;
  WORD16 ol_wght_flg;
  WORD32 prev_ol_lags[5];
  WORD32 prev_pitch_med;
  FLOAT32 prev_hp_wsp[LEN_SUPERFRAME / OPL_DECIM + (MAX_PITCH / OPL_DECIM)];
  FLOAT32 hp_ol_ltp_mem[3 * 2 + 1];
  const FLOAT32 *lp_analysis_window;
  FLOAT32 xn_buffer[128];
  WORD32 c_prev[(NUM_COEFF / 2) + 4];
  WORD32 c_pres[(NUM_COEFF / 2) + 4];
  WORD32 arith_reset_flag;
  WORD16 prev_mode;
  WORD32 num_bits_per_supfrm;
  FLOAT32 fd_synth[2 * LEN_FRAME + 1 + ORDER];
  FLOAT32 fd_orig[2 * LEN_FRAME + 1 + ORDER];
  WORD32 low_pass_line;
  WORD32 last_was_short;
  WORD32 next_is_short;
  FLOAT32 gain_tcx;
  WORD32 max_sfb_short;
} ia_usac_td_encoder_struct;
