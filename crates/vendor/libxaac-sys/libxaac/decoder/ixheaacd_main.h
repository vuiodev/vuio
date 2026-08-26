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
#ifndef IXHEAACD_MAIN_H
#define IXHEAACD_MAIN_H

VOID ixheaacd_imdct_flt(FLOAT32 in_data[], FLOAT32 out_data[], WORD32 len);

VOID usac_tw_imdct(FLOAT32 in_data[], FLOAT32 out_data[], WORD32 len);

WORD32 ixheaacd_window_calc(FLOAT32 window[], WORD32 len, WORD32 wfun_select);

VOID calc_window_ratio(FLOAT32 window[], WORD32 len, WORD32 prev_len,
                       WORD32 wfun_select, WORD32 prev_wfun_select);

WORD32 ixheaacd_tw_window_calc(FLOAT32 window[], WORD32 len,
                               WORD32 wfun_select);

typedef struct ia_usac_lpd_decoder {
  WORD32 mode_prev;
  float synth_prev[MAX_PITCH + SYNTH_DELAY_LMAX];
  float xcitation_prev[MAX_PITCH + INTER_LP_FIL_ORDER + 1];
  int pitch_prev[NUM_SUBFR_SUPERFRAME_BY2 - 1];
  float gain_prev[NUM_SUBFR_SUPERFRAME_BY2 - 1];

  float lp_flt_coeff_a_prev[2 * (ORDER + 1)];

  FLOAT32 exc_prev[1 + (2 * FAC_LENGTH)];

  FLOAT32 bpf_prev[FILTER_DELAY + LEN_SUBFR];

  WORD32 ilspold[ORDER];

  FLOAT32 fac_gain;
  FLOAT32 fac_fd_data[FAC_LENGTH / 4];

  FLOAT32 lsf_prev[ORDER];
  FLOAT32 lspold[ORDER];
  WORD32 lsfold_first[ORDER];

  FLOAT32 gain_threshold;

  WORD32 fscale;

  FLOAT32 fd_synth_buf[3 * LEN_FRAME + 1 + ORDER];
  FLOAT32 *fd_synth;
  WORD32 bpf_active_prev;
  WORD32 last_tcx_pitch;
  FLOAT32 synth_prev_ec[ORDER];
} ia_usac_lpd_decoder, *ia_usac_lpd_decoder_handle;

typedef struct ia_usac_data_main_struct {
  FLOAT32 time_sample_vector[MAX_NUM_CHANNELS][4096];
  FLOAT32 time_sample_vector_prev[MAX_NUM_CHANNELS][4096];
  WORD32 input_data_ptr[MAX_NUM_CHANNELS][4096];
  WORD32 overlap_data_ptr[MAX_NUM_CHANNELS][4096];
  WORD32 output_data_ptr[MAX_NUM_CHANNELS][4096];

  WORD32 window_shape[MAX_NUM_CHANNELS];
  WORD32 window_shape_prev[MAX_NUM_CHANNELS];
  WORD32 window_sequence[MAX_NUM_CHANNELS];
  WORD32 window_sequence_last[MAX_NUM_CHANNELS];

  WORD32 output_samples;
  WORD32 sbr_ratio_idx;
  WORD32 usac_independency_flg;

  WORD32 sampling_rate_idx;
  WORD32 audio_object_type;

  WORD32 down_samp_sbr;
  WORD32 sbr_mode;

  WORD32 tw_mdct[MAX_ELEMENTS];
  WORD32 mps_pseudo_lr[MAX_ELEMENTS];
  WORD32 td_frame_prev[MAX_NUM_CHANNELS];

  FLOAT32 warp_sum[MAX_NUM_CHANNELS][2];
  FLOAT32 warp_cont_mem[MAX_NUM_CHANNELS][3 * 1024];
  FLOAT32 prev_sample_pos[MAX_NUM_CHANNELS][3 * 1024];
  FLOAT32 prev_tw_trans_len[MAX_NUM_CHANNELS][2];
  WORD32 prev_tw_start_stop[MAX_NUM_CHANNELS][2];
  FLOAT32 prev_warped_time_sample_vector[MAX_NUM_CHANNELS][3 * 1024];

  FLOAT32 lpc_prev[MAX_NUM_CHANNELS][ORDER + 1];
  FLOAT32 acelp_in[MAX_NUM_CHANNELS][1 + (2 * FAC_LENGTH)];

  WORD32 alpha_q_re[MAX_SHORT_WINDOWS][SFB_NUM_MAX];
  WORD32 alpha_q_im[MAX_SHORT_WINDOWS][SFB_NUM_MAX];
  UWORD8 cplx_pred_used[MAX_SHORT_WINDOWS][SFB_NUM_MAX];

  WORD32 alpha_q_re_prev[SFB_NUM_MAX];
  WORD32 alpha_q_im_prev[SFB_NUM_MAX];
  WORD32 dmx_re_prev[BLOCK_LEN_LONG];

  VOID *sbr_scratch_mem_base;

  WORD32 *coef_fix[MAX_NUM_CHANNELS];
  FLOAT32 *coef[MAX_NUM_CHANNELS];
  UWORD8 *ms_used[MAX_NUM_CHANNELS];
  WORD32 *coef_save[MAX_NUM_CHANNELS];

  WORD16 *factors[MAX_NUM_CHANNELS];
  UWORD8 *group_dis[MAX_NUM_CHANNELS];

  WORD32 tw_data_present[MAX_NUM_CHANNELS];
  WORD32 *tw_ratio[MAX_NUM_CHANNELS];
  ia_tns_frame_info_struct *pstr_tns[MAX_NUM_CHANNELS];

  ia_usac_lpd_decoder_handle str_tddec[MAX_NUM_CHANNELS];

  WORD32 arith_prev_n[MAX_NUM_CHANNELS];
  WORD8 c_prev[MAX_NUM_CHANNELS][1024 / 2 + 4];
  WORD8 c[MAX_NUM_CHANNELS][1024 / 2 + 4];

  WORD32 noise_filling_config[MAX_NUM_ELEMENTS];
  UWORD32 seed_value[MAX_NUM_CHANNELS];
  WORD32 present_chan;

  WORD32 fac_data_present[MAX_NUM_CHANNELS];
  WORD32 fac_data[MAX_NUM_CHANNELS][FAC_LENGTH + 1];

  ia_sfb_info_struct *pstr_sfb_info[MAX_NUM_CHANNELS];
  ia_sfb_info_struct str_only_long_info;
  ia_sfb_info_struct str_eight_short_info;
  ia_sfb_info_struct *pstr_usac_winmap[NUM_WIN_SEQ];
  WORD16 sfb_width_short[(1 << LEN_MAX_SFBS)];

  WORD32 ccfl;
  WORD32 len_subfrm;
  WORD32 num_subfrm;

  ia_handle_sbr_dec_inst_struct pstr_esbr_dec;
  ia_aac_dec_sbr_bitstream_struct esbr_bit_str[2];

  WORD32 x_ac_dec[1024];
  WORD32 scratch_buffer[1024];

  FLOAT32 synth_buf[1883];
  FLOAT32 exc_buf[1453];
  FLOAT32 lp_flt_coff[290];
  WORD32 pitch[25];
  FLOAT32 pitch_gain[25];

  UWORD16 *huffman_code_book_scl;
  UWORD32 *huffman_code_book_scl_index;

  WORD32 *tns_coeff3_32;

  WORD32 *tns_coeff4_32;

  WORD32 (*tns_max_bands_tbl_usac)[16][2];

  WORD16 sfb_width_long[(1 << LEN_MAX_SFBL)];
  WORD32 usac_flag;

  WORD32 arr_coef_fix[MAX_NUM_CHANNELS][(LN2 + LN2 / 8)];
  FLOAT32 arr_coef[MAX_NUM_CHANNELS][(LN2 + LN2 / 8)];
  WORD32 arr_coef_save[MAX_NUM_CHANNELS][(LN2 + LN2 / 8)];
  WORD16 arr_factors[MAX_NUM_CHANNELS][MAXBANDS];
  UWORD8 arr_group_dis[MAX_NUM_CHANNELS][NSHORT];
  WORD32 arr_tw_ratio[MAX_NUM_CHANNELS][NUM_TW_NODES];
  UWORD8 arr_ms_used[MAX_NUM_CHANNELS][MAXBANDS];
  ia_usac_lpd_decoder arr_str_tddec[MAX_NUM_CHANNELS];
  ia_tns_frame_info_struct arr_str_tns[MAX_NUM_CHANNELS];

  WORD32 enh_sbr;
  WORD32 esbr_hq;
  WORD32 enh_sbr_ps;
  WORD32 drc_config_changed;
  WORD32 core_mode;
  WORD32 frame_ok;
  WORD32 sbr_parse_err_flag;
  WORD32 last_frame_ok;
  WORD32 ec_flag;
  WORD32 first_frame;
  WORD32 sbr_parse_complete;
  UWORD8 max_sfb[2];
  WORD32 num_ch_out;
  WORD16 spec_scale[MAX_NUM_CHANNELS][128];
  ia_ec_state_str str_error_concealment[MAX_NUM_CHANNELS];
  ia_td_frame_data_struct *pstr_td_frame;
  WORD32 sampling_rate;
  WORD32 td_frame_prev_ec[MAX_NUM_CHANNELS];
  FLOAT32 lsp_coeff[5][ORDER];
  FLOAT32 lsf_adaptive_mean_cand[ORDER];
  FLOAT32 lsf_adaptive_mean[ORDER];
  FLOAT32 lpc4_lsf[ORDER];
  WORD32 bpf_control_info;
  WORD32 first_lpd_flag;
  WORD32 short_fac_flag;
  WORD32 core_mode_last;
  FLOAT32 stability_factor_old;
  WORD32 num_lost_lpd_frames[MAX_NUM_CHANNELS];
  WORD32 pitch_lag_old;
  WORD32 pitch_lag_frac_old;
  WORD32 pitch_lag;
  WORD32 pitch_lag_frac;
  WORD16 seed_ace;
  ia_ec_state_str *pstr_ec_state;
  FLOAT32 past_pitch_gain;
  FLOAT32 past_gain_code;
  FLOAT32 past_gain_tcx[MAX_NUM_CHANNELS];
  WORD32 tcx_spec_coeffs[MAX_NUM_CHANNELS][1280];
  FLOAT32 lspold_ec[ORDER];
  FLOAT32 lp_flt_coff_a_ec[ORDER + 1];
  ia_td_frame_data_struct td_frame_data_prev[MAX_NUM_CHANNELS];
  WORD32 last_shiftp;
} ia_usac_data_struct;

IA_ERRORCODE ixheaacd_tns_apply(ia_usac_data_struct *usac_data, WORD32 *spec,
                                WORD32 nbands,
                                ia_sfb_info_struct *pstr_sfb_info,
                                ia_tns_frame_info_struct *pstr_tns);

WORD32 ixheaacd_calc_max_spectral_line_dec(WORD32 *ptr_tmp, WORD32 size);

WORD32 ixheaacd_calc_max_spectral_line_armv7(WORD32 *ptr_tmp, WORD32 size);

WORD32 ixheaacd_calc_max_spectral_line_armv8(WORD32 *ptr_tmp, WORD32 size);

WORD32 ixheaacd_tw_buff_update(ia_usac_data_struct *usac_data, WORD32 i,
                               ia_usac_lpd_decoder_handle st);

VOID ixheaacd_fix2flt_data(ia_usac_data_struct *usac_data,
                           ia_usac_lpd_decoder_handle st, WORD32 k);

VOID ixheaacd_td_frm_dec(ia_usac_data_struct *usac_data, WORD32 k, WORD32 mod0);

WORD32 ixheaacd_tw_frame_dec(ia_usac_data_struct *usac_data, WORD32 i_ch,
                             FLOAT32 sample_pos[], FLOAT32 tw_trans_len[],
                             WORD32 tw_start_stop[]);

WORD32 ixheaacd_fd_frm_dec(ia_usac_data_struct *usac_data, WORD32 i_ch);

VOID ixheaacd_acelp_mdct(WORD32 *ptr_in, WORD32 *ptr_out, WORD32 *preshift, WORD32 length,
                         WORD32 *ptr_scratch);

VOID ixheaacd_acelp_mdct_main(ia_usac_data_struct *usac_data, WORD32 *x, WORD32 *y, WORD32 l,
                              WORD32 m, WORD32 *preshift);

VOID ixheaacd_fr_alias_cnx_fix(WORD32 *x_in, WORD32 len_subfr, WORD32 lfac, WORD32 *iaq,
                               WORD32 *izir, WORD32 *ifacdec, WORD8 *qshift1, WORD8 qshift2,
                               WORD8 qshift3, WORD32 *preshift, WORD32 *ptr_scratch);

VOID ixheaacd_fwd_alias_cancel_tool(ia_usac_data_struct *usac_data,
                                    ia_td_frame_data_struct *pstr_td_frame_data,
                                    WORD32 fac_length, FLOAT32 *iaq, WORD32 gain);

WORD32 ixheaacd_lpd_bpf_fix(ia_usac_data_struct *usac_data, WORD32 is_short,
                            FLOAT32 out_buffer[],
                            ia_usac_lpd_decoder_handle st);

VOID ixheaacd_reset_acelp_data_fix(ia_usac_data_struct *usac_data,
                                   ia_usac_lpd_decoder_handle st,
                                   WORD32 *ptr_ola_buff, WORD32 last_was_short,
                                   WORD32 tw_mdct);

FLOAT32 ixheaacd_randomsign(UWORD32 *seed);
#endif
