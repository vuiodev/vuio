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
#ifndef IXHEAACD_SBR_DEC_H
#define IXHEAACD_SBR_DEC_H

#include <setjmp.h>

typedef struct {
  FLOAT32 real[64][128];
  FLOAT32 imag[64][128];
} ia_dft_hbe_anal_coeff;

typedef struct {
  WORD32 x_over_qmf[MAX_NUM_PATCHES];
  WORD32 max_stretch;
  WORD32 core_frame_length;
  WORD32 hbe_qmf_in_len;
  WORD32 hbe_qmf_out_len;
  WORD32 no_bins;
  WORD32 start_band;
  WORD32 end_band;
  WORD32 upsamp_4_flag;
  WORD32 synth_buf_offset;

  FLOAT32 *ptr_input_buf;

  FLOAT32 **qmf_in_buf;
  FLOAT32 **qmf_out_buf;

  WORD32 k_start;
  WORD32 synth_size;
  FLOAT32 synth_buf[1280];
  FLOAT32 analy_buf[640];
  FLOAT32 *synth_wind_coeff;
  FLOAT32 *analy_wind_coeff;

  FLOAT32 *synth_cos_tab;
  FLOAT32 *analy_cos_sin_tab;

  FLOAT32 norm_qmf_in_buf[46][128];
  VOID (*ixheaacd_real_synth_fft)(FLOAT32 *inp, FLOAT32 *out, WORD32 n_points);

  VOID (*ixheaacd_cmplx_anal_fft)(FLOAT32 *inp, FLOAT32 *out, WORD32 n_points);

  WORD32 esbr_hq;
  WORD32 in_hop_size;
  WORD32 fft_size[2];

  FLOAT32 *anal_window;
  FLOAT32 *synth_window;

  FLOAT32 *ptr_spectrum;
  FLOAT32 *ptr_spectrum_tx;
  FLOAT32 *mag;
  FLOAT32 *phase;
  FLOAT32 *ptr_output_buf;
  WORD32 ana_fft_size[2];
  WORD32 syn_fft_size[2];
  WORD32 out_hop_size;
  WORD32 analy_size;
  WORD32 x_over_bin[MAX_STRETCH][2];
  WORD32 a_start;

  FLOAT32 spectrum_buf[1536];
  FLOAT32 spectrum_transposed_buf[1536];
  FLOAT32 mag_buf[1536];
  FLOAT32 phase_buf[1536];
  FLOAT32 output_buf[4096];
  FLOAT32 fd_win_buf[3][3][1536];

  FLOAT32 analysis_window_buf[1024];
  FLOAT32 synthesis_window_buf[1024];

  WORD32 oversampling_flag;
  ia_dft_hbe_anal_coeff str_dft_hbe_anal_coeff;
  VOID (*ixheaacd_hbe_anal_fft)(FLOAT32 *inp, FLOAT32 *scratch, WORD32 len, WORD32 sign);
  VOID (*ixheaacd_hbe_synth_ifft)(FLOAT32 *inp, FLOAT32 *scratch, WORD32 len, WORD32 sign);
  FLOAT32 *syn_cos_sin_tab;
  FLOAT32 *ana_cos_sin_tab;

} ia_esbr_hbe_txposer_struct;

typedef struct {
  WORD32 *ptr_sbr_overlap_buf;
  WORD32 **drc_factors_sbr;
  ia_sbr_qmf_filter_bank_struct str_codec_qmf_bank;
  ia_sbr_qmf_filter_bank_struct str_synthesis_qmf_bank;
  ia_sbr_calc_env_struct str_sbr_calc_env;
  ia_sbr_hf_generator_struct str_hf_generator;

  ia_sbr_scale_fact_struct str_sbr_scale_fact;

  WORD32 max_samp_val;
  WORD32 band_count;
  ia_esbr_hbe_txposer_struct *p_hbe_txposer;

  FLOAT32 core_sample_buf[2624];
  WORD16 core_sample_buf_sbr[2624];
  FLOAT32 ph_vocod_qmf_real[TIMESLOT_BUFFER_SIZE][NO_QMF_SYNTH_CHANNELS];
  FLOAT32 ph_vocod_qmf_imag[TIMESLOT_BUFFER_SIZE][NO_QMF_SYNTH_CHANNELS];
  FLOAT32 sbr_qmf_out_real[TIMESLOT_BUFFER_SIZE][NO_QMF_SYNTH_CHANNELS];
  FLOAT32 sbr_qmf_out_imag[TIMESLOT_BUFFER_SIZE][NO_QMF_SYNTH_CHANNELS];
  FLOAT32 qmf_buf_real[TIMESLOT_BUFFER_SIZE + 2 * 32][NO_QMF_SYNTH_CHANNELS];
  FLOAT32 qmf_buf_imag[TIMESLOT_BUFFER_SIZE + 2 * 32][NO_QMF_SYNTH_CHANNELS];

  FLOAT32 mps_qmf_buf_real[TIMESLOT_BUFFER_SIZE][NO_QMF_SYNTH_CHANNELS];
  FLOAT32 mps_qmf_buf_imag[TIMESLOT_BUFFER_SIZE][NO_QMF_SYNTH_CHANNELS];
  FLOAT32 mps_sbr_qmf_buf_real[TIMESLOT_BUFFER_SIZE][NO_QMF_SYNTH_CHANNELS];
  FLOAT32 mps_sbr_qmf_buf_imag[TIMESLOT_BUFFER_SIZE][NO_QMF_SYNTH_CHANNELS];

  WORD32 sbr_scratch_local[256];
  FLOAT32 scratch_buff[320];

  FLOAT32 qmf_energy_buf[64][32];
  FLOAT32 pvc_qmf_enrg_arr[16 * 32];

  FLOAT32 **pp_qmf_buf_real;
  FLOAT32 **pp_qmf_buf_imag;

  WORD32 *p_arr_qmf_buf_real[MAX_ENV_COLS];
  WORD32 *p_arr_qmf_buf_imag[MAX_ENV_COLS];

  FLOAT32 *time_sample_buf;
} ia_sbr_dec_struct;

typedef struct {
  ia_sbr_prev_frame_data_struct *pstr_prev_frame_data;
  ia_sbr_dec_struct str_sbr_dec;
  WORD32 output_frame_size;
  WORD32 sync_state;
} ia_sbr_channel_struct;

struct ia_sbr_dec_inst_struct {
  ia_ps_dec_struct *pstr_ps_stereo_dec;
  ia_ps_dec_config_struct str_ps_config_prev;
  FLAG ps_present;
  ia_sbr_channel_struct *pstr_sbr_channel[MAXNRSBRCHANNELS];
  ia_sbr_header_data_struct *pstr_sbr_header[MAXNRSBRCHANNELS];
  ia_freq_band_data_struct *pstr_freq_band_data[MAXNRSBRCHANNELS];
  ia_sbr_tables_struct *pstr_sbr_tables;
  ixheaacd_misc_tables *pstr_common_tables;
  ia_pvc_data_struct *ptr_pvc_data_str;
  VOID *hbe_txposer_buffers;
  FLOAT32 *time_sample_buf[MAXNRSBRCHANNELS];
  VOID *scratch_mem_v;
  VOID *frame_buffer[2];
  ia_sbr_header_data_struct str_sbr_dflt_header;
  FLAG stereo_config_idx;
  FLAG usac_independency_flag;
  FLAG pvc_flag;
  FLAG hbe_flag;
  FLAG sbr_mode;
  FLAG prev_sbr_mode;
  FLAG inter_tes_flag;
  FLAG aot_usac_flag;
  WORD32 band_count[MAXNRSBRCHANNELS];
  jmp_buf *xaac_jmp_buf;
  WORD8 *ptr_mps_data;
  WORD32 left_mps_bits;
  WORD32 mps_bits_pos;
  FLAG esbr_hq;
  FLAG enh_sbr;
  FLAG enh_sbr_ps;
  FLAG eld_sbr;
  WORD32 num_delay_frames;
  FLAG sbr_parse_err_flag;
  FLAG frame_ok;
  FLAG ec_flag;
  FLAG first_frame;
  FLAG prev_usac_independency_flag;
  FLAG sbr_parse_complete;
};

typedef struct ia_sbr_pers_struct {
  WORD16 *sbr_qmf_analy_states;

  WORD32 *sbr_qmf_analy_states_32;

  WORD16 *sbr_qmf_synth_states;

  WORD32 *sbr_qmf_synth_states_32;

  WORD32 **sbr_lpc_filter_states_real[MAXNRSBRCHANNELS];

  WORD32 **sbr_lpc_filter_states_imag[MAXNRSBRCHANNELS];

  WORD32 *ptr_sbr_overlap_buf[MAXNRSBRCHANNELS];

  struct ia_sbr_dec_inst_struct str_sbr_dec_inst;

  ia_transposer_settings_struct str_sbr_tran_settings;

  WORD16 *sbr_smooth_gain_buf[MAXNRSBRCHANNELS];

  WORD16 *sbr_smooth_noise_buf[MAXNRSBRCHANNELS];

  ia_sbr_prev_frame_data_struct *pstr_prev_frame_data[MAXNRSBRCHANNELS];

} ia_sbr_pers_struct;

WORD32 ixheaacd_sbr_dec(
    ia_sbr_dec_struct *ptr_sbr_dec, WORD16 *ptr_time_data,
    ia_sbr_header_data_struct *ptr_header_data, ia_sbr_frame_info_data_struct *ptr_frame_data,
    ia_sbr_prev_frame_data_struct *ptr_frame_data_prev, ia_ps_dec_struct *ptr_ps_dec,
    ia_sbr_qmf_filter_bank_struct *ptr_qmf_synth_bank_r, ia_sbr_scale_fact_struct *ptr_sbr_sf_r,
    FLAG apply_processing, FLAG low_pow_flag, WORD32 *ptr_work_buf_core,
    ia_sbr_tables_struct *sbr_tables_ptr, ixheaacd_misc_tables *pstr_common_tables, WORD ch_fac,
    ia_pvc_data_struct *ptr_pvc_data_str, FLAG drc_on, WORD32 drc_sbr_factors[][64],
    WORD32 audio_object_type, WORD32 ldmps_present, VOID *self, WORD32 heaac_mps_present,
    WORD32 ec_flag);

WORD16 ixheaacd_create_sbrdec(ixheaacd_misc_tables *pstr_common_table,
                              ia_sbr_channel_struct *ptr_sbr_channel,
                              ia_sbr_header_data_struct *ptr_header_data,
                              WORD16 chan, FLAG down_sample_flag,
                              VOID *sbr_persistent_mem_v, WORD ps_enable,
                              WORD audio_object_type, WORD32 ldmps_present,
                              WORD32 ldsbr_present);

#define MAX_NUM_QMF_BANDS_ESBR 128

WORD32 ixheaacd_sbr_dec_from_mps(FLOAT32 *p_mps_qmf_output, VOID *p_sbr_dec, VOID *p_sbr_frame,
                                 VOID *p_sbr_header, WORD32 ec_flag);

WORD32 ixheaacd_qmf_hbe_apply(ia_esbr_hbe_txposer_struct *h_hbe_txposer,
                              FLOAT32 qmf_buf_real[][64],
                              FLOAT32 qmf_buf_imag[][64], WORD32 num_columns,
                              FLOAT32 pv_qmf_buf_real[][64],
                              FLOAT32 pv_qmf_buf_imag[][64],
                              WORD32 pitch_in_bins,
                              ia_sbr_header_data_struct *ptr_header_data);

VOID ixheaacd_hbe_apply_cfftn(FLOAT32 re[], FLOAT32 *scratch, WORD32 n_pass, WORD32 i_sign);

VOID ixheaacd_hbe_apply_cfftn_gen(FLOAT32 re[], FLOAT32 *scratch, WORD32 n_pass,
                                  WORD32 i_sign);

VOID ixheaacd_hbe_apply_fft_288(FLOAT32 *inp, FLOAT32 *scratch, WORD32 len, WORD32 i_sign);

VOID ixheaacd_hbe_apply_ifft_224(FLOAT32 *inp, FLOAT32 *scratch, WORD32 len, WORD32 i_sign);

VOID ixheaacd_hbe_apply_fft_3(FLOAT32 *inp, FLOAT32 *op, WORD32 i_sign);

VOID ixheaacd_hbe_apply_ifft_7(FLOAT32 *inp, FLOAT32 *op);

VOID ixheaacd_hbe_apply_ifft_336(FLOAT32 *inp, FLOAT32 *ptr_scratch, WORD32 len,
                                 WORD32 i_sign);

WORD32 ixheaacd_dft_hbe_apply(ia_esbr_hbe_txposer_struct *ptr_hbe_txposer,
                              FLOAT32 qmf_buf_real[][64],
                              FLOAT32 qmf_buf_imag[][64], WORD32 num_columns,
                              FLOAT32 pv_qmf_buf_real[][64],
                              FLOAT32 pv_qmf_buf_imag[][64],
                              WORD32 pitch_in_bins,
                              FLOAT32 *dft_hbe_scratch_buf);

WORD32 ixheaacd_sbr_env_calc(ia_sbr_frame_info_data_struct *frame_data, FLOAT32 input_real[][64],
                             FLOAT32 input_imag[][64], FLOAT32 input_real1[][64],
                             FLOAT32 input_imag1[][64], WORD32 x_over_qmf[MAX_NUM_PATCHES],
                             FLOAT32 *scratch_buff, FLOAT32 *env_out, WORD32 ldmps_present,
                             WORD32 ec_flag);

WORD32 ixheaacd_generate_hf(FLOAT32 ptr_src_buf_real[][64], FLOAT32 ptr_src_buf_imag[][64],
                            FLOAT32 ptr_ph_vocod_buf_real[][64],
                            FLOAT32 ptr_ph_vocod_buf_imag[][64], FLOAT32 ptr_dst_buf_real[][64],
                            FLOAT32 ptr_dst_buf_imag[][64],
                            ia_sbr_frame_info_data_struct *ptr_frame_data,
                            ia_sbr_header_data_struct *ptr_header_data, WORD32 audio_object_type,
                            WORD32 time_slots, WORD32 ec_flag);

VOID ixheaacd_clr_subsamples(WORD32 *ptr_qmf_buf, WORD32 num, WORD32 size);

VOID ixheaacd_rescale_x_overlap(
    ia_sbr_dec_struct *ptr_sbr_dec, ia_sbr_header_data_struct *ptr_header_data,
    ia_sbr_frame_info_data_struct *ptr_frame_data,
    ia_sbr_prev_frame_data_struct *ptr_frame_data_prev,
    WORD32 **pp_overlap_buffer_real, WORD32 **pp_overlap_buffer_imag,
    FLAG low_pow_flag);

WORD32 ixheaacd_qmf_hbe_data_reinit(ia_esbr_hbe_txposer_struct *ptr_hbe_transposer_str,
                                    WORD16 *ptr_freq_band_tbl[2], WORD16 *ptr_num_sf_bands,
                                    WORD32 upsamp_4_flag);

WORD32 ixheaacd_dft_hbe_data_reinit(ia_esbr_hbe_txposer_struct *ptr_hbe_txposer,
                                    WORD16 *p_freq_band_tab[2], WORD16 *p_num_sfb);

WORD32 ixheaacd_sbr_read_pvc_sce(ia_sbr_frame_info_data_struct *ptr_frame_data,
                                 ia_bit_buf_struct *it_bit_buff,
                                 WORD32 hbe_flag,
                                 ia_pvc_data_struct *ptr_pvc_data,
                                 ia_sbr_tables_struct *sbr_tables_ptr,
                                 ia_sbr_header_data_struct *ptr_header_data);

WORD32 ixheaacd_esbr_dec(ia_sbr_dec_struct *ptr_sbr_dec,
                         ia_sbr_header_data_struct *ptr_header_data,
                         ia_sbr_frame_info_data_struct *ptr_frame_data,
                         FLAG apply_processing, FLAG low_pow_flag,
                         ia_sbr_tables_struct *sbr_tables_ptr, WORD32 ch_fac);

VOID ixheaacd_hbe_repl_spec(WORD32 x_over_qmf[MAX_NUM_PATCHES],
                            FLOAT32 qmf_buf_real[][64],
                            FLOAT32 qmf_buf_imag[][64], WORD32 no_bins,
                            WORD32 max_stretch);

#endif
