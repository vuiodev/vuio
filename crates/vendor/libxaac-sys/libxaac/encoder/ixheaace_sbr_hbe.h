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

#define IXHEAACE_SBR_HF_ADJ_OFFSET (2)
#define IXHEAACE_ESBR_HBE_DELAY_OFFSET (32)
#define IXHEAACE_TIMESLOT_BUFFER_SIZE (78)
#define IXHEAACE_MAX_NUM_PATCHES (6)
#define IXHEAACE_MAX_STRETCH (4)
#define IXHEAACE_MAX_NUM_LIMITERS (12)
#define IXHEAACE_MAX_FREQ_COEFFS (56)
#define IXHEAACE_MAX_NOISE_COEFFS (5)

#define IXHEAACE_HBE_OPER_WIN_LEN (13)
#define IXHEAACE_NUM_QMF_SYNTH_CHANNELS (64)
#define IXHEAACE_TWICE_QMF_SYNTH_CH_NUM (128)
#define IXHEAACE_HBE_ZERO_BAND_IDX (6)
#define IXHEAACE_HBE_OPER_BLK_LEN_2 (10)
#define IXHEAACE_HBE_OPER_BLK_LEN_3 (8)
#define IXHEAACE_HBE_OPER_BLK_LEN_4 (6)
#define IXHEAACE_FD_OVERSAMPLING_FAC (1.5f)

#define IXHEAACE_MAX_NO_COLS_VALUE (64)
#define IXHEAACE_MAX_FRAME_SIZE (1024)
#define IXHEAACE_MAX_NUM_SAMPLES (4096)
#define IXHEAACE_MAX_QMF_X_INBUF_SIZE (IXHEAACE_MAX_NO_COLS_VALUE)
#define IXHEAACE_MAX_QMF_X_OUTBUF_SIZE (2 * IXHEAACE_MAX_QMF_X_INBUF_SIZE)

#define IXHEAACE_MAX_QMF_X_IN_REAL_BUF \
  (IXHEAACE_NUM_QMF_SYNTH_CHANNELS * IXHEAACE_MAX_QMF_X_INBUF_SIZE)
#define IXHEAACE_MAX_QMF_X_IN_IMAG_BUF \
  (IXHEAACE_NUM_QMF_SYNTH_CHANNELS * IXHEAACE_MAX_QMF_X_INBUF_SIZE)

#define IXHEAACE_MAX_QMF_X_OUT_REAL_BUF \
  (IXHEAACE_NUM_QMF_SYNTH_CHANNELS * IXHEAACE_MAX_QMF_X_OUTBUF_SIZE)
#define IXHEAACE_MAX_QMF_X_OUT_IMAG_BUF \
  (IXHEAACE_NUM_QMF_SYNTH_CHANNELS * IXHEAACE_MAX_QMF_X_OUTBUF_SIZE)

#define IXHEAACE_X_INBUF_SIZE (IXHEAACE_MAX_FRAME_SIZE + IXHEAACE_NUM_QMF_SYNTH_CHANNELS)
#define IXHEAACE_X_OUTBUF_SIZE (IXHEAACE_X_INBUF_SIZE * 2)

#define IXHEAACE_MAX_HBE_PERSISTENT_SIZE                                                         \
  (IXHEAACE_MAX_QMF_X_INBUF_SIZE * sizeof(FLOAT32 *) +                                           \
   IXHEAACE_MAX_QMF_X_OUTBUF_SIZE * sizeof(FLOAT32 *) +                                          \
   IXHEAACE_MAX_QMF_X_IN_REAL_BUF * sizeof(FLOAT32) +                                            \
   IXHEAACE_MAX_QMF_X_IN_IMAG_BUF * sizeof(FLOAT32) +                                            \
   IXHEAACE_MAX_QMF_X_OUT_REAL_BUF * sizeof(FLOAT32) +                                           \
   IXHEAACE_MAX_QMF_X_OUT_IMAG_BUF * sizeof(FLOAT32) + IXHEAACE_X_INBUF_SIZE * sizeof(FLOAT32) + \
   IXHEAACE_X_OUTBUF_SIZE * sizeof(FLOAT32))

#define IXHEAACE_LOW (0)
#define IXHEAACE_HIGH (1)

#define IXHEAACE_SBR_CONST_PMIN 1.0f

#define ixheaace_cbrt_calc(a) (pow(a, -0.333333f))

#define IXHEAACE_QMF_FILTER_STATE_ANA_SIZE 320

typedef struct {
  FLOAT32 real[64][128];
  FLOAT32 imag[64][128];
} ixheaace_str_dft_hbe_anal_coeff;

typedef struct {
  WORD32 x_over_qmf[IXHEAACE_MAX_NUM_PATCHES];
  WORD32 max_stretch;
  WORD32 core_frame_length;
  WORD32 hbe_qmf_in_len;
  WORD32 hbe_qmf_out_len;
  WORD32 no_bins;
  WORD32 start_band;
  WORD32 end_band;
  WORD32 upsamp_4_flag;
  WORD32 synth_buf_offset;

  WORD16 num_sf_bands[2];
  WORD16 *ptr_freq_band_tab[2];
  WORD16 freq_band_tbl_lo[IXHEAACE_MAX_FREQ_COEFFS / 2 + 1];
  WORD16 freq_band_tbl_hi[IXHEAACE_MAX_FREQ_COEFFS + 1];

  FLOAT32 *ptr_input_buf;

  FLOAT32 qmf_in_buf[IXHEAACE_TWICE_QMF_SYNTH_CH_NUM][IXHEAACE_TWICE_QMF_SYNTH_CH_NUM];
  FLOAT32 qmf_out_buf[IXHEAACE_TWICE_QMF_SYNTH_CH_NUM][IXHEAACE_TWICE_QMF_SYNTH_CH_NUM];

  WORD32 k_start;
  WORD32 synth_size;
  FLOAT32 synth_buf[1280];
  FLOAT32 analy_buf[640];
  FLOAT32 *ptr_syn_win_coeff;
  FLOAT32 *ptr_ana_win_coeff;

  FLOAT32 *ptr_syn_cos_tab;
  FLOAT32 *ptr_analy_cos_sin_tab;

  VOID (*ixheaace_real_synth_fft)(FLOAT32 *ptr_inp, FLOAT32 *ptr_out, WORD32 n_points);
  FLOAT32 norm_qmf_in_buf[128][128];

  VOID (*ixheaace_cmplx_anal_fft)(FLOAT32 *ptr_inp, FLOAT32 *ptr_out, WORD32 n_points);

  WORD32 esbr_hq;
  WORD32 in_hop_size;
  WORD32 fft_size[2];

  FLOAT32 *ptr_ana_win; /* Phase Vocoder Analysis Window for FFT */
  FLOAT32 *ptr_syn_win; /* Phase Vocoder Synthesis Window for OLA */

  FLOAT32 *ptr_spectrum;    /* FFT values in cartesian space */
  FLOAT32 *ptr_spectrum_tx; /* Transposed spectrum */
  FLOAT32 *ptr_mag;         /* FFT magnitudes */
  FLOAT32 *ptr_phase;       /* FFT angles */
  FLOAT32 *ptr_output_buf;
  WORD32 ana_fft_size[2]; /* Analysis FFT length */
  WORD32 syn_fft_size[2]; /* Synthesis FFT length */
  WORD32 out_hop_size;
  WORD32 analy_size;
  WORD32 x_over_bin[IXHEAACE_MAX_STRETCH][2];
  WORD32 a_start;

  FLOAT32 spectrum_buf[1536];            /* FFT values in cartesian space */
  FLOAT32 spectrum_transposed_buf[1536]; /* Transposed spectrum */
  FLOAT32 mag_buf[1536];                 /* FFT magnitudes */
  FLOAT32 phase_buf[1536];               /* FFT angles */
  FLOAT32 output_buf[IXHEAACE_MAX_NUM_SAMPLES];
  FLOAT32 fd_win_buf[3][3][1536];

  FLOAT32 analysis_window_buf[1024];
  FLOAT32 synthesis_window_buf[1024];

  WORD32 oversampling_flag;
  ixheaace_str_dft_hbe_anal_coeff str_dft_hbe_anal_coeff;
  VOID (*ixheaace_hbe_anal_fft)(FLOAT32 *ptr_inp, FLOAT32 *ptr_scratch, WORD32 len, WORD32 sign);
  VOID(*ixheaace_hbe_synth_ifft)
  (FLOAT32 *ptr_inp, FLOAT32 *ptr_scratch, WORD32 len, WORD32 sign);
  FLOAT32 *ptr_syn_cos_sin_tab;
  FLOAT32 *ptr_ana_cos_sin_tab;
} ixheaace_str_esbr_hbe_txposer;

typedef struct {
  WORD16 w_16[2 * 12];
  FLOAT32 dig_rev_tab_4_16[2];

  FLOAT32 esbr_qmf_c[1280];
  FLOAT32 esbr_qmf_c_24[480];
  FLOAT32 esbr_w_16[2 * 12];

  FLOAT32 esbr_sin_cos_twiddle_l64[64];
  FLOAT32 esbr_alt_sin_twiddle_l64[32];

  FLOAT32 esbr_sin_cos_twiddle_l32[32];
  FLOAT32 esbr_alt_sin_twiddle_l32[16];
  FLOAT32 esbr_t_cos_sin_l32[32 + 32];

  FLOAT32 esbr_sin_cos_twiddle_l24[24];
  FLOAT32 esbr_alt_sin_twiddle_l24[12];
  FLOAT32 esbr_t_cos_sin_l24[24 + 24];

  FLOAT32 esbr_sin_cos_twiddle_l16[16];
  FLOAT32 esbr_alt_sin_twiddle_l16[8];
  FLOAT32 esbr_t_cos_sin_l16[16 + 16];
} ixheaace_str_qmf_dec_tabs_struct;

typedef struct {
  WORD32 no_channels;
  WORD16 num_time_slots;

  WORD16 lsb;
  WORD16 usb;

  const FLOAT32 *ptr_ana_win_coeff_32;
  const FLOAT32 *ptr_esbr_cos_twiddle;
  const FLOAT32 *ptr_esbr_alt_sin_twiddle;
  const FLOAT32 *ptr_esbr_t_cos;
  FLOAT32 anal_filter_states_32[IXHEAACE_QMF_FILTER_STATE_ANA_SIZE];
  FLOAT32 *ptr_state_new_samples_pos_low_32;
  const FLOAT32 *ptr_filter_pos_32;
  ixheaace_str_qmf_dec_tabs_struct *pstr_qmf_dec_tabs;
} ia_sbr_qmf_filter_bank_struct;

typedef struct {
  ixheaace_str_esbr_hbe_txposer *pstr_hbe_txposer;
  VOID *ptr_hbe_txposer_buffers;
  FLOAT32 ph_vocod_qmf_real[IXHEAACE_TIMESLOT_BUFFER_SIZE][IXHEAACE_NUM_QMF_SYNTH_CHANNELS];
  FLOAT32 ph_vocod_qmf_imag[IXHEAACE_TIMESLOT_BUFFER_SIZE][IXHEAACE_NUM_QMF_SYNTH_CHANNELS];
  FLOAT32 sbr_qmf_out_real[IXHEAACE_TIMESLOT_BUFFER_SIZE][IXHEAACE_NUM_QMF_SYNTH_CHANNELS];
  FLOAT32 sbr_qmf_out_imag[IXHEAACE_TIMESLOT_BUFFER_SIZE][IXHEAACE_NUM_QMF_SYNTH_CHANNELS];
  FLOAT32 qmf_buf_real[IXHEAACE_TIMESLOT_BUFFER_SIZE + 2 * 32][IXHEAACE_NUM_QMF_SYNTH_CHANNELS];
  FLOAT32 qmf_buf_imag[IXHEAACE_TIMESLOT_BUFFER_SIZE + 2 * 32][IXHEAACE_NUM_QMF_SYNTH_CHANNELS];

  ia_sbr_qmf_filter_bank_struct str_codec_qmf_bank;
} ixheaace_str_hbe_enc;

extern const ixheaace_str_qmf_dec_tabs_struct ixheaace_str_aac_qmf_tabs;

VOID iusace_complex_fft_p2(FLOAT32 *ptr_x, WORD32 nlength, FLOAT32 *ptr_scratch_fft_p2_y);
VOID iusace_complex_fft_p3_no_scratch(FLOAT32 *ptr_data, WORD32 nlength);

VOID ixheaace_esbr_hbe_data_init(ixheaace_str_esbr_hbe_txposer *pstr_esbr_hbe_txposer,
                                 const WORD32 num_aac_samples, WORD32 samp_fac_4_flag,
                                 const WORD32 num_out_samples, VOID *ptr_persistent_hbe_mem,
                                 WORD32 *ptr_total_persistant);

IA_ERRORCODE ixheaace_dft_hbe_data_reinit(ixheaace_str_esbr_hbe_txposer *pstr_hbe_txposer);

IA_ERRORCODE ixheaace_qmf_hbe_data_reinit(ixheaace_str_esbr_hbe_txposer *pstr_hbe_txposer);

IA_ERRORCODE ixheaace_dft_hbe_apply(ixheaace_str_esbr_hbe_txposer *pstr_hbe_txposer,
                                    FLOAT32 qmf_buf_real[][64], FLOAT32 qmf_buf_imag[][64],
                                    WORD32 num_columns, FLOAT32 pv_qmf_buf_real[][64],
                                    FLOAT32 pv_qmf_buf_imag[][64], WORD32 pitch_in_bins,
                                    FLOAT32 *ptr_dft_hbe_scratch_buf);

IA_ERRORCODE ixheaace_qmf_hbe_apply(ixheaace_str_esbr_hbe_txposer *pstr_hbe_txposer,
                                    FLOAT32 qmf_buf_real[][64], FLOAT32 qmf_buf_imag[][64],
                                    WORD32 num_columns, FLOAT32 pv_qmf_buf_real[][64],
                                    FLOAT32 pv_qmf_buf_imag[][64], WORD32 pitch_in_bins);

IA_ERRORCODE ixheaace_hbe_post_anal_process(ixheaace_str_esbr_hbe_txposer *pstr_hbe_txposer,
                                            WORD32 pitch_in_bins, WORD32 sbr_upsamp_4_flg);

VOID ixheaace_hbe_repl_spec(WORD32 x_over_qmf[IXHEAACE_MAX_NUM_PATCHES],
                            FLOAT32 qmf_buf_real[][64], FLOAT32 qmf_buf_imag[][64],
                            WORD32 no_bins, WORD32 max_stretch);

VOID ixheaace_esbr_qmf_init(ia_sbr_qmf_filter_bank_struct *pstr_codec_qmf_bank,
                            WORD32 sbr_ratio_idx, WORD32 output_frame_size);

VOID ixheaace_esbr_analysis_filt_block(
    ia_sbr_qmf_filter_bank_struct *pstr_codec_qmf_bank,
    ixheaace_str_qmf_dec_tabs_struct *pstr_qmf_dec_tabs, FLOAT32 *ptr_core_coder_samples,
    FLOAT32 qmf_buf_real[IXHEAACE_TIMESLOT_BUFFER_SIZE + 2 * 32][IXHEAACE_NUM_QMF_SYNTH_CHANNELS],
    FLOAT32 qmf_buf_imag[IXHEAACE_TIMESLOT_BUFFER_SIZE + 2 * 32][IXHEAACE_NUM_QMF_SYNTH_CHANNELS],
    WORD32 op_delay);
