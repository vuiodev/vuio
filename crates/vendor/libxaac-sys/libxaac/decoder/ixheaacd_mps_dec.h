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
#ifndef IXHEAACD_MPS_DEC_H
#define IXHEAACD_MPS_DEC_H

#include "stddef.h"

#define ABS_THR (1e-9f * 32768 * 32768)

#define MAX_NUM_QMF_BANDS_MPS (128)
#define MAX_NUM_QMF_BANDS_MPS_NEW (64)

#define MAX_PARAMETER_SETS_MPS (9)
#define BUFFER_LEN_HF_MPS ((QMF_HYBRID_FILT_ORDER - 1) / 2 + MAX_TIME_SLOTS)

#define DECORR_FILTER_ORDER_BAND_0 (20)
#define DECORR_FILTER_ORDER_BAND_1 (15)
#define DECORR_FILTER_ORDER_BAND_2 (6)
#define DECORR_FILTER_ORDER_BAND_3 (3)

#define MAX_DECORR_FILTER_ORDER (DECORR_FILTER_ORDER_BAND_0)

#define DECORR_FILT_0_ORD (10)
#define DECORR_FILT_1_ORD (8)
#define DECORR_FILT_2_ORD (3)
#define DECORR_FILT_3_ORD (2)

#define NO_RES_BANDS -1

#define MAX_HYBRID_BANDS_MPS (MAX_NUM_QMF_BANDS_MPS_NEW - 3 + 10)
#define MAX_TIME_SLOTS (72)

#define QMF_HYBRID_FILT_ORDER (13)
#define BUFFER_LEN_LF_MPS (QMF_HYBRID_FILT_ORDER - 1 + MAX_TIME_SLOTS)
#define MAX_NO_TIME_SLOTS_DELAY (14)

#define MAXNRSBRCHANNELS 2
#define abs(x) (x < 0) ? -x : x

#define PC_FILTERLENGTH (11)
#define PC_FILTERDELAY ((PC_FILTERLENGTH - 1) / 2)
#define ABS_THR_FIX (35184)

#ifndef MAX_NUM_QMF_BANDS
#define MAX_NUM_QMF_BANDS (64)
#endif

#define MAX_HYBRID_BANDS (MAX_NUM_QMF_BANDS - 3 + 10)
#define MAX_INPUT_CHANNELS_MPS (6)

#define MAX_RESIDUAL_CHANNELS_MPS (10)
#define MAX_RESIDUAL_FRAMES (4)

#define MAX_OUTPUT_CHANNELS_MPS (8)
#define MAX_NUM_PARAMS (MAX_NUM_OTT + 4 * MAX_NUM_TTT + MAX_INPUT_CHANNELS_MPS)

#define MAX_PARAMETER_SETS (8)

#define MAX_M1_OUTPUT (8)
#define MAX_M2_INPUT (8)
#define MAX_M2_OUTPUT (8)

#define PROTO_LEN (13)
#define BUFFER_LEN_LF (PROTO_LEN - 1 + MAX_TIME_SLOTS)
#define BUFFER_LEN_HF ((PROTO_LEN - 1) / 2 + MAX_TIME_SLOTS)

#define MAX_NO_DECORR_CHANNELS (5)

#define MAX_OUTPUT_CHANNELS_AT_MPS (8)

#define QMF_FILTER_STATE_SYN_SIZE_MPS (576)

#define QMF_FILTER_STATE_ANA_SIZE_MPS (640)

#define MAX_NUM_POAT max(MAX_NUM_PARAMS, MAX_NUM_OTT + MAX_OUTPUT_CHANNELS_AT)

#define BP_SIZE 25

#define MPS_SCRATCH_MEM_SIZE (194048)

typedef struct {
  FLOAT32 re;
  FLOAT32 im;

} ia_cmplx_flt_struct;

typedef struct {
  WORD32 re;
  WORD32 im;
} ia_cmplx_w32_struct;

typedef struct {
  FLOAT32 *re;
  FLOAT32 *im;
} ia_cmplx_fltp_struct;

typedef struct ia_mps_decor_filt_struct {
  WORD32 state_len;
  WORD32 num_len;
  WORD32 den_len;

  ia_cmplx_flt_struct state[MAX_DECORR_FILTER_ORDER + 1];
  const FLOAT32 *num;
  const FLOAT32 *den;

} ia_mps_decor_filt_struct;

typedef struct ixheaacd_mps_decor_energy_adjust_filt_struct {
  WORD32 num_bins;
  FLOAT32 smooth_in_energy[MAX_PARAMETER_BANDS];
  FLOAT32 smooth_out_energy[MAX_PARAMETER_BANDS];

} ixheaacd_mps_decor_energy_adjust_filt_struct;

typedef struct ia_mps_decor_struct {
  WORD32 num_bins;
  ia_mps_decor_filt_struct filter[MAX_HYBRID_BANDS_MPS];
  ixheaacd_mps_decor_energy_adjust_filt_struct decor_nrg_smooth;

  WORD32 delay_sample_count[MAX_HYBRID_BANDS_MPS];

  ia_cmplx_flt_struct
      decor_delay_buffer[71][MAX_TIME_SLOTS + MAX_NO_TIME_SLOTS_DELAY];

} ia_mps_decor_struct;

typedef struct ia_mps_hybrid_filt_struct {
  ia_cmplx_flt_struct hf_buffer[MAX_NUM_QMF_BANDS_MPS][BUFFER_LEN_HF_MPS];
  ia_cmplx_flt_struct lf_buffer[QMF_BANDS_TO_HYBRID][BUFFER_LEN_LF_MPS];
} ia_mps_hybrid_filt_struct;

typedef struct ia_mps_data_struct {
  WORD32 bs_xxx_data_mode[MAX_PARAMETER_SETS_MPS];
  WORD32 quant_coarse_xxx_flag[MAX_PARAMETER_SETS_MPS];
  WORD32 bs_freq_res_stride_xxx[MAX_PARAMETER_SETS_MPS];
  WORD8 bs_quant_coarse_xxx[MAX_PARAMETER_SETS_MPS];
  WORD8 bs_quant_coarse_xxx_prev;
} ia_mps_data_struct;

typedef struct ia_mps_bs_frame {
  WORD8 independency_flag;

  WORD32 cld_idx[MAX_PARAMETER_SETS_MPS][MAX_PARAMETER_BANDS];
  WORD32 icc_idx[MAX_PARAMETER_SETS_MPS][MAX_PARAMETER_BANDS];

  WORD32 cld_idx_pre[MAX_PARAMETER_BANDS];
  WORD32 icc_idx_pre[MAX_PARAMETER_BANDS];

  WORD32 cmp_cld_idx[MAX_PARAMETER_SETS_MPS][MAX_PARAMETER_BANDS];
  WORD32 cmp_icc_idx[MAX_PARAMETER_SETS_MPS][MAX_PARAMETER_BANDS];

  WORD32 cmp_cld_idx_prev[MAX_PARAMETER_BANDS];
  WORD32 cmp_icc_idx_prev[MAX_PARAMETER_BANDS];

  ia_mps_data_struct cld_data;
  ia_mps_data_struct icc_data;
  ia_mps_data_struct ipd_data;

  WORD32 ipd_idx_data[MAX_PARAMETER_SETS_MPS][MAX_PARAMETER_BANDS];
  WORD32 ipd_idx_data_prev[MAX_PARAMETER_BANDS];
  WORD32 ipd_idx[MAX_PARAMETER_SETS_MPS][MAX_PARAMETER_BANDS];
  WORD32 ipd_idx_prev[MAX_PARAMETER_BANDS];

  WORD32 bs_smooth_mode[MAX_PARAMETER_SETS_MPS];
  WORD32 bs_smooth_time[MAX_PARAMETER_SETS_MPS];
  WORD32 bs_freq_res_stride_smg[MAX_PARAMETER_SETS_MPS];
  WORD32 bs_smg_data[MAX_PARAMETER_SETS_MPS][MAX_PARAMETER_BANDS];

} ia_mps_bs_frame;

typedef struct ia_mps_smoothing_struct {
  WORD32 prev_smg_time;
  FLOAT32 inv_prev_smg_time;
  WORD32 prev_smg_data[MAX_PARAMETER_BANDS];
} ia_mps_smoothing_struct;

typedef struct ia_mps_env_reshape_struct {
  FLOAT32 pb_energy_prev[3][MAX_PARAMETER_BANDS];
  FLOAT32 avg_energy_prev[3];
  FLOAT32 frame_energy_prev[3];
} ia_mps_env_reshape_struct;

typedef struct ia_mps_stp_struct {
  FLOAT32 nrg_dir;
  FLOAT32 nrg_diff[2];
  FLOAT32 nrg_dir_prev;
  FLOAT32 nrg_diff_prev[2];
  FLOAT32 tp_scale_last[2];
  WORD32 init_flag;
  WORD32 update_old_ener;
} ia_mps_stp_struct;

typedef struct ia_mps_opd_smooth_struct {
  WORD32 smooth_l_phase[MAX_PARAMETER_BANDS];
  WORD32 smooth_r_phase[MAX_PARAMETER_BANDS];
} ia_mps_opd_smooth_struct;

typedef struct ia_mps_dec_state_struct {
  WORD32 in_ch_count;
  WORD32 out_ch_count;

  FLOAT32 input_gain;
  WORD32 dir_sig_count;

  WORD32 decor_sig_count;

  WORD32 time_slots;
  WORD32 pre_mix_req;
  WORD32 temp_shape_enable_ch_stp[2];
  WORD32 temp_shape_enable_ch_ges[2];

  FLOAT32 env_shape_data[2][MAX_TIME_SLOTS];

  WORD8 parse_nxt_frame;
  WORD32 band_count[MAX_M_INPUT];
  WORD32 synth_count;
  WORD32 qmf_band_count;
  WORD32 hyb_band_count[MAX_M_INPUT];
  WORD32 hyb_band_count_max;
  const WORD32 *hyb_band_to_processing_band_table;

  WORD32 res_ch_count;

  WORD32 res_bands;
  WORD32 max_res_bands;
  WORD32 bs_param_bands;

  WORD32 ext_frame_flag;
  WORD32 num_parameter_sets;
  WORD32 num_parameter_sets_prev;
  WORD32 param_slots[MAX_PARAMETER_SETS_MPS];
  WORD32 param_slot_diff[MAX_PARAMETER_SETS_MPS];
  FLOAT32 inv_param_slot_diff[MAX_PARAMETER_SETS_MPS];
  WORD32 inv_param_slot_diff_Q30[MAX_PARAMETER_SETS_MPS];

  WORD32 frame_length;
  WORD32 residual_coding;
  WORD32 bs_residual_present;

  WORD32 bs_residual_bands;

  ia_usac_dec_mps_config_struct *config;
  ia_usac_dec_mps_config_struct ldmps_config;
  ia_mps_bs_frame bs_frame;

  WORD32 smoothing_time[MAX_PARAMETER_SETS_MPS];
  FLOAT32 inv_smoothing_time[MAX_PARAMETER_SETS_MPS];
  WORD32 smoothing_data[MAX_PARAMETER_SETS_MPS][MAX_PARAMETER_BANDS];

  WORD32 bs_tsd_enable;
  WORD32 bs_tsd_sep_data[MAX_TIME_SLOTS];
  WORD32 bs_tsd_tr_phase_data[MAX_TIME_SLOTS];
  WORD32 tsd_num_tr_slots;
  WORD32 tsd_codeword_len;

  FLOAT32 cld_data[MAX_PARAMETER_SETS_MPS][MAX_PARAMETER_BANDS];
  FLOAT32 icc_data[MAX_PARAMETER_SETS_MPS][MAX_PARAMETER_BANDS];
  FLOAT32 ipd_data[MAX_PARAMETER_SETS_MPS][MAX_PARAMETER_BANDS];

  WORD32 bs_phase_mode;
  WORD32 opd_smoothing_mode;
  WORD32 num_bands_ipd;

  FLOAT32 phase_l[MAX_PARAMETER_SETS_MPS][MAX_PARAMETER_BANDS];
  FLOAT32 phase_r[MAX_PARAMETER_SETS_MPS][MAX_PARAMETER_BANDS];
  FLOAT32 phase_l_prev[MAX_PARAMETER_BANDS];
  FLOAT32 phase_r_prev[MAX_PARAMETER_BANDS];

  FLOAT32 m1_param_re[MAX_PARAMETER_SETS_MPS][MAX_PARAMETER_BANDS][MAX_M_OUTPUT]
                     [MAX_M_INPUT];
  FLOAT32 m1_param_im[MAX_PARAMETER_SETS_MPS][MAX_PARAMETER_BANDS][MAX_M_OUTPUT]
                     [MAX_M_INPUT];
  FLOAT32 m2_decor_re[MAX_PARAMETER_SETS_MPS][MAX_PARAMETER_BANDS][MAX_M_OUTPUT]
                     [MAX_M_INPUT];
  FLOAT32 m2_decor_im[MAX_PARAMETER_SETS_MPS][MAX_PARAMETER_BANDS][MAX_M_OUTPUT]
                     [MAX_M_INPUT];
  FLOAT32 m2_resid_re[MAX_PARAMETER_SETS_MPS][MAX_PARAMETER_BANDS][MAX_M_OUTPUT]
                     [MAX_M_INPUT];
  FLOAT32 m2_resid_im[MAX_PARAMETER_SETS_MPS][MAX_PARAMETER_BANDS][MAX_M_OUTPUT]
                     [MAX_M_INPUT];

  FLOAT32 m1_param_re_prev[MAX_PARAMETER_BANDS][MAX_M_OUTPUT][MAX_M_INPUT];
  FLOAT32 m1_param_im_prev[MAX_PARAMETER_BANDS][MAX_M_OUTPUT][MAX_M_INPUT];
  FLOAT32 m2_decor_re_prev[MAX_PARAMETER_BANDS][MAX_M_OUTPUT][MAX_M_INPUT];
  FLOAT32 m2_decor_im_prev[MAX_PARAMETER_BANDS][MAX_M_OUTPUT][MAX_M_INPUT];
  FLOAT32 m2_resid_re_prev[MAX_PARAMETER_BANDS][MAX_M_OUTPUT][MAX_M_INPUT];
  FLOAT32 m2_resid_im_prev[MAX_PARAMETER_BANDS][MAX_M_OUTPUT][MAX_M_INPUT];

  ia_cmplx_flt_struct qmf_in[2][MAX_NUM_QMF_BANDS_MPS_NEW][MAX_TIME_SLOTS];
  ia_cmplx_flt_struct hyb_in[2][MAX_HYBRID_BANDS_MPS][MAX_TIME_SLOTS];
  ia_cmplx_flt_struct hyb_res[MAX_HYBRID_BANDS_MPS][MAX_TIME_SLOTS];
  ia_cmplx_flt_struct v[MAX_M_OUTPUT][MAX_TIME_SLOTS][MAX_HYBRID_BANDS_MPS];
  ia_cmplx_flt_struct w_diff[MAX_M_INPUT][MAX_TIME_SLOTS][MAX_HYBRID_BANDS_MPS];
  ia_cmplx_flt_struct w_dir[MAX_M_INPUT][MAX_TIME_SLOTS][MAX_HYBRID_BANDS_MPS];
  ia_cmplx_flt_struct hyb_dir_out[2][MAX_TIME_SLOTS][MAX_HYBRID_BANDS_MPS];
  ia_cmplx_flt_struct hyb_diff_out[2][MAX_TIME_SLOTS][MAX_HYBRID_BANDS_MPS];

  ia_cmplx_flt_struct qmf_out_dir[2][MAX_TIME_SLOTS][MAX_NUM_QMF_BANDS_MPS];
  ia_cmplx_flt_struct scratch[MAX_TIME_SLOTS][MAX_HYBRID_BANDS_MPS];

  FLOAT32 (*output_buffer)[4096];

  ia_mps_hybrid_filt_struct hyb_filt_state[2];
  FLOAT32 qmf_filt_state[2][POLY_PHASE_SYNTH_SIZE];

  ia_mps_decor_struct mps_decor;

  ia_mps_smoothing_struct smoothing_filt_state;

  ia_mps_env_reshape_struct guided_env_shaping;

  WORD32 bs_high_rate_mode;

  FLOAT32 tmp_buf[84 * MAX_NUM_QMF_BANDS_SAC];

  FLOAT32 r_out_re_in_m1[MAX_TIME_SLOTS][MAX_PARAMETER_BANDS][MAX_M_OUTPUT]
                        [MAX_M_INPUT];
  FLOAT32 r_out_im_in_m1[MAX_TIME_SLOTS][MAX_PARAMETER_BANDS][MAX_M_OUTPUT]
                        [MAX_M_INPUT];
  WORD32 r_out_re_scratch_m1[MAX_TIME_SLOTS][MAX_PARAMETER_BANDS][MAX_M_OUTPUT]
                            [MAX_M_INPUT];
  WORD32 r_out_im_scratch_m1[MAX_TIME_SLOTS][MAX_PARAMETER_BANDS][MAX_M_OUTPUT]
                            [MAX_M_INPUT];

  FLOAT32 r_out_re_in_m2[MAX_TIME_SLOTS][MAX_PARAMETER_BANDS][MAX_M_OUTPUT]
                        [MAX_M_INPUT];
  FLOAT32 r_out_im_in_m2[MAX_TIME_SLOTS][MAX_PARAMETER_BANDS][MAX_M_OUTPUT]
                        [MAX_M_INPUT];
  FLOAT32 r_out_diff_re_in_m2[MAX_TIME_SLOTS][MAX_PARAMETER_BANDS][MAX_M_OUTPUT]
                             [MAX_M_INPUT];
  FLOAT32 r_out_diff_im_in_m2[MAX_TIME_SLOTS][MAX_PARAMETER_BANDS][MAX_M_OUTPUT]
                             [MAX_M_INPUT];

  WORD32 r_out_re_fix_in_m2[MAX_TIME_SLOTS][MAX_PARAMETER_BANDS][MAX_M_OUTPUT]
                           [MAX_M_INPUT];
  WORD32 r_out_im_fix_in_m2[MAX_TIME_SLOTS][MAX_PARAMETER_BANDS][MAX_M_OUTPUT]
                           [MAX_M_INPUT];
  WORD32 r_diff_out_re_fix_in_m2[MAX_TIME_SLOTS][MAX_PARAMETER_BANDS]
                                [MAX_M_OUTPUT][MAX_M_INPUT];
  WORD32 r_diff_out_im_fix_in_m2[MAX_TIME_SLOTS][MAX_PARAMETER_BANDS]
                                [MAX_M_OUTPUT][MAX_M_INPUT];

  FLOAT32 r_out_ph_re_in_m2[MAX_TIME_SLOTS][MAX_PARAMETER_BANDS][2];
  FLOAT32 r_out_ph_im_in_m2[MAX_TIME_SLOTS][MAX_PARAMETER_BANDS][2];

  ia_mps_stp_struct subband_var;
  ia_mps_opd_smooth_struct opd_smooth;
  WORD32 resolution;
  VOID *p_sbr_dec[MAXNRSBRCHANNELS];
  VOID *p_sbr_frame[MAXNRSBRCHANNELS];
  VOID *p_sbr_header[MAXNRSBRCHANNELS];

  WORD32 object_type;
  WORD32 mps_init_done;
  ia_sbr_qmf_filter_bank_struct str_mps_qmf_bank;
  ia_qmf_dec_tables_struct *qmf_dec_tables_ptr;
  ia_sbr_tables_struct *sbr_tables_ptr;
  ia_sbr_scale_fact_struct *str_sbr_scale_fact;
  WORD8 ec_flag;
  WORD8 frame_ok;
} ia_mps_dec_state_struct;

VOID ixheaacd_mps_init_pre_and_post_matrix(ia_mps_dec_state_struct *self);
VOID ixheaacd_pre_and_mix_matrix_calculation(ia_mps_dec_state_struct *self);
VOID ixheaacd_mps_apply_pre_matrix(ia_mps_dec_state_struct *self);
VOID ixheaacd_mps_apply_mix_matrix(ia_mps_dec_state_struct *self);
VOID ixheaacd_mps_apply_mix_matrix_type1(ia_mps_dec_state_struct *self);
VOID ixheaacd_mps_apply_mix_matrix_type2(ia_mps_dec_state_struct *self);
VOID ixheaacd_mps_apply_mix_matrix_type3(ia_mps_dec_state_struct *self);

VOID ixheaacd_samples_sat(WORD8 *outbuffer, WORD32 num_samples_out,
                          WORD32 pcmsize, FLOAT32 (*out_samples)[4096],
                          WORD32 *out_bytes, WORD32 num_channel_out);

VOID ixheaacd_samples_sat_mc(WORD8* outbuffer, WORD32 num_samples_out,
                             FLOAT32(*out_samples)[4096], WORD32* out_bytes,
                             WORD32 num_channel_out, WORD32 ch_fac);

IA_ERRORCODE ixheaacd_mps_frame_decode(ia_mps_dec_state_struct *self);

WORD32 ixheaacd_mps_header_decode(ia_mps_dec_state_struct *self);

VOID ixheaacd_mps_env_init(ia_mps_dec_state_struct *self);
VOID ixheaacd_mps_time_env_shaping(ia_mps_dec_state_struct *self);

VOID ixheaacd_mps_pre_matrix_mix_matrix_smoothing(
    ia_mps_dec_state_struct *self);
VOID ixheaacd_mps_smoothing_opd(ia_mps_dec_state_struct *self);
WORD32 ixheaacd_mps_temp_process(ia_mps_dec_state_struct *self);

VOID ixheaacd_mps_par2umx_ps(ia_mps_dec_state_struct *self,
                             ia_mps_bs_frame *curr_bit_stream, FLOAT32 *h_real,
                             WORD32 param_set_idx);
VOID ixheaacd_mps_par2umx_ps_ipd_opd(ia_mps_dec_state_struct *self,
                                     ia_mps_bs_frame *curr_bit_stream,
                                     FLOAT32 *h_real, WORD32 param_set_idx);

VOID ixheaacd_mps_par2umx_pred(ia_mps_dec_state_struct *self,
                               ia_mps_bs_frame *curr_bit_stream, FLOAT32 *h_imag,
                               FLOAT32 *h_real, WORD32 param_set_idx,
                               WORD32 res_bands);

VOID ixheaacd_mps_upmix_interp(
    FLOAT32 m_matrix[MAX_PARAMETER_SETS_MPS][MAX_PARAMETER_BANDS][MAX_M_OUTPUT]
                   [MAX_M_INPUT],
    FLOAT32 r_matrix[MAX_TIME_SLOTS][MAX_PARAMETER_BANDS][MAX_M_OUTPUT]
                   [MAX_M_INPUT],
    FLOAT32 m_matrix_prev[MAX_PARAMETER_BANDS][MAX_M_OUTPUT][MAX_M_INPUT],
    WORD32 num_rows, WORD32 num_cols, ia_mps_dec_state_struct *self, WORD32 bs_high_rate_mode);

VOID ixheaacd_mps_upmix_interp_type1(
    FLOAT32 m_matrix[MAX_PARAMETER_SETS_MPS][MAX_PARAMETER_BANDS][MAX_M_OUTPUT]
                    [MAX_M_INPUT],
    FLOAT32 r_matrix[MAX_TIME_SLOTS][MAX_PARAMETER_BANDS][MAX_M_OUTPUT]
                    [MAX_M_INPUT],
    FLOAT32 m_matrix_prev[MAX_PARAMETER_BANDS][MAX_M_OUTPUT][MAX_M_INPUT],
    WORD32 num_rows, WORD32 num_cols, ia_mps_dec_state_struct *self,
    WORD32 bs_high_rate_mode);

VOID ixheaacd_mps_upmix_interp_type2(
    FLOAT32 m_matrix[MAX_PARAMETER_SETS_MPS][MAX_PARAMETER_BANDS][MAX_M_OUTPUT]
                    [MAX_M_INPUT],
    FLOAT32 r_matrix[MAX_TIME_SLOTS][MAX_PARAMETER_BANDS][MAX_M_OUTPUT]
                    [MAX_M_INPUT],
    FLOAT32 m_matrix_prev[MAX_PARAMETER_BANDS][MAX_M_OUTPUT][MAX_M_INPUT],
    WORD32 num_rows, ia_mps_dec_state_struct *self, WORD32 col);

VOID ixheaacd_mps_phase_interpolation(
    FLOAT32 pl[MAX_PARAMETER_SETS_MPS][MAX_PARAMETER_BANDS],
    FLOAT32 pr[MAX_PARAMETER_SETS_MPS][MAX_PARAMETER_BANDS],
    FLOAT32 pl_prev[MAX_PARAMETER_BANDS], FLOAT32 pr_prev[MAX_PARAMETER_BANDS],
    FLOAT32 r_re[MAX_TIME_SLOTS][MAX_PARAMETER_BANDS][2],
    FLOAT32 r_im[MAX_TIME_SLOTS][MAX_PARAMETER_BANDS][2],
    ia_mps_dec_state_struct *self);

VOID ixheaacd_mps_complex_fft(FLOAT32 *xr, FLOAT32 *xi, WORD32 nlength);

typedef struct {
  WORD32 state_length;
  WORD32 num_length;
  WORD32 den_length;
  WORD32 complex;

  WORD32 *state_real;
  WORD32 *state_imag;

  WORD32 *numerator_real;
  WORD32 *numerator_imag;

  WORD32 *denominator_real;
  WORD32 *denominator_imag;
} ia_mps_dec_decorr_filter_instance_struct;

typedef struct ia_mps_dec_ducker_interface ia_mps_dec_ducker_interface;

typedef struct {
  WORD32 decorr_seed;
  WORD32 numbins;

  ia_mps_dec_decorr_filter_instance_struct *filter[MAX_HYBRID_BANDS];

  ia_mps_dec_ducker_interface *ducker;

  WORD32 no_sample_delay[MAX_HYBRID_BANDS];
  WORD32 **delay_buffer_real;
  WORD32 **delay_buffer_imag;
} ia_mps_dec_decorr_dec_struct, *ia_mps_dec_decorr_dec_handle;

typedef struct {
  WORD32 buffer_lf_real[QMF_BANDS_TO_HYBRID][BUFFER_LEN_LF];
  WORD32 buffer_lf_imag[QMF_BANDS_TO_HYBRID][BUFFER_LEN_LF];
  WORD32 qmf_lf_real[QMF_BANDS_TO_HYBRID][BUFFER_LEN_LF];
  WORD32 qmf_lf_imag[QMF_BANDS_TO_HYBRID][BUFFER_LEN_LF];
  WORD32 buffer_hf_real[MAX_NUM_QMF_BANDS][BUFFER_LEN_HF];
  WORD32 buffer_hf_imag[MAX_NUM_QMF_BANDS][BUFFER_LEN_HF];
} ia_mps_dec_thyb_filter_state_struct;

typedef struct {
  WORD32 re;
  WORD32 im;
} complex;

typedef struct {
  UWORD32 ui_pcm_wdsz;
  UWORD32 ui_samp_freq;
  UWORD32 ui_in_channels;
  UWORD32 ui_out_channels;
  WORD32 ui_channel_mask;

  WORD32 frame_ok;
  UWORD32 ui_bs_is_buried;
  WORD32 ui_dec_type;
  WORD32 ui_upmix_type;
  WORD32 ui_binaural_quality;
  WORD32 ui_hrtf_model;
  UWORD32 ui_qmf_bands;

  WORD32 bs_frame_length;
  WORD32 bs_sampling_freq_index;
  WORD32 bs_sampling_frequency;
  WORD32 bs_freq_res;
  WORD32 bs_tree_config;
  WORD32 bs_quant_mode;
  WORD32 bs_one_icc;
  WORD32 bs_arbitrary_downmix;
  WORD32 bs_residual_coding;
  WORD32 bs_smooth_config;
  WORD32 bs_fixed_gain_sur;
  WORD32 bs_fixed_gain_lfe;
  WORD32 bs_fixed_gain_dmx;
  WORD32 bs_matrix_mode;
  WORD32 bs_temp_shape_config;

  WORD32 bs_decorr_config;

  WORD32 bs_3d_audio_mode;
  WORD32 bs_3d_audio_hrtf_set;
  WORD32 bs_hrtf_freq_res;
  WORD32 hrtf_num_band;
  WORD32 bs_hrtf_num_chan;
  WORD32 bs_hrtf_asymmetric;
  WORD32 bs_hrtf_level_left[MAX_OUTPUT_CHANNELS_MPS][MAX_PARAMETER_BANDS];
  WORD32 bs_hrtf_level_right[MAX_OUTPUT_CHANNELS_MPS][MAX_PARAMETER_BANDS];
  WORD32 bs_hrtf_phase[MAX_OUTPUT_CHANNELS_MPS];
  WORD32 bs_hrtf_phase_lr[MAX_OUTPUT_CHANNELS_MPS][MAX_PARAMETER_BANDS];

  WORD32 bs_ott_bands[MAX_NUM_OTT];
  WORD32 bs_ttt_dual_mode[MAX_NUM_TTT];
  WORD32 bs_ttt_mode_low[MAX_NUM_TTT];
  WORD32 bs_ttt_mode_high[MAX_NUM_TTT];
  WORD32 bs_ttt_bands_low[MAX_NUM_TTT];

  WORD32 bs_sac_ext_type[MAX_NUM_EXT_TYPES];
  WORD32 sac_ext_cnt;

  WORD32 bs_residual_present[MAX_RESIDUAL_CHANNELS_MPS];
  WORD32 bs_residual_sampling_freq_index;
  WORD32 bs_residual_frames_per_spatial_frame;

  WORD32 bs_residual_bands[MAX_RESIDUAL_CHANNELS_MPS];

  WORD32 bs_arbitrary_downmix_residual_sampling_freq_index;
  WORD32 bs_arbitrary_downmix_residual_frames_per_spatial_frame;
  WORD32 bs_arbitrary_downmix_residual_bands;

  WORD32 bs_env_quant_mode;

  WORD32 arbitrary_tree;
  WORD32 num_out_chan_at;
  WORD32 num_ott_boxes_at;
  WORD32 bs_output_channel_pos_at[MAX_OUTPUT_CHANNELS_AT_MPS];
  WORD32 bs_ott_box_present_at[MAX_OUTPUT_CHANNELS_AT_MPS]
                              [MAX_ARBITRARY_TREE_INDEX];
  WORD32 bs_ott_default_cld_at[MAX_OUTPUT_CHANNELS_AT_MPS *
                               MAX_ARBITRARY_TREE_INDEX];
  WORD32
  bs_ott_mode_lfe_at[MAX_OUTPUT_CHANNELS_AT_MPS * MAX_ARBITRARY_TREE_INDEX];
  WORD32 bs_ott_bands_at[MAX_OUTPUT_CHANNELS_AT_MPS * MAX_ARBITRARY_TREE_INDEX];
} ia_mps_spatial_bs_config_struct;

typedef struct {
  WORD32
  bs_xxx_data_mode[MAX_NUM_POAT][MAX_PARAMETER_SETS];
  WORD32
  bs_quant_coarse_xxx[MAX_NUM_POAT][MAX_PARAMETER_SETS];
  WORD32
  bs_freq_res_stride_xxx[MAX_NUM_POAT][MAX_PARAMETER_SETS];

  WORD32 bs_quant_coarse_xxx_prev[MAX_NUM_POAT];
  WORD32
  no_cmp_quant_coarse_xxx[MAX_NUM_POAT][MAX_PARAMETER_SETS];

} ia_mps_dec_lossless_data_struct;

typedef struct {
  WORD32 bs_icc_diff_present[MAX_RESIDUAL_CHANNELS_MPS][MAX_PARAMETER_SETS];
  WORD32
  bs_icc_diff[MAX_RESIDUAL_CHANNELS_MPS][MAX_PARAMETER_SETS]
             [MAX_PARAMETER_BANDS];

} RESIDUAL_FRAME_DATA;

typedef struct {
  WORD32 bs_independency_flag;
  WORD32
  ott_cld_idx[MAX_NUM_OTT + MAX_OUTPUT_CHANNELS_AT][MAX_PARAMETER_SETS]
             [MAX_PARAMETER_BANDS];
  WORD32 ott_icc_idx[MAX_NUM_OTT][MAX_PARAMETER_SETS][MAX_PARAMETER_BANDS];
  WORD32 ttt_cpc_1_idx[MAX_NUM_TTT][MAX_PARAMETER_SETS][MAX_PARAMETER_BANDS];
  WORD32 ttt_cpc_2_idx[MAX_NUM_TTT][MAX_PARAMETER_SETS][MAX_PARAMETER_BANDS];
  WORD32 ttt_cld_1_idx[MAX_NUM_TTT][MAX_PARAMETER_SETS][MAX_PARAMETER_BANDS];
  WORD32 ttt_cld_2_idx[MAX_NUM_TTT][MAX_PARAMETER_SETS][MAX_PARAMETER_BANDS];
  WORD32 ttt_icc_idx[MAX_NUM_TTT][MAX_PARAMETER_SETS][MAX_PARAMETER_BANDS];

  WORD32 ott_cld_idx_prev[MAX_NUM_OTT + MAX_OUTPUT_CHANNELS_AT][MAX_PARAMETER_BANDS];
  WORD32 ott_icc_idx_prev[MAX_NUM_OTT][MAX_PARAMETER_BANDS];
  WORD32 ttt_cpc_1_idx_prev[MAX_NUM_TTT][MAX_PARAMETER_BANDS];
  WORD32 ttt_cpc_2_idx_prev[MAX_NUM_TTT][MAX_PARAMETER_BANDS];
  WORD32 ttt_cld_1_idx_prev[MAX_NUM_TTT][MAX_PARAMETER_BANDS];
  WORD32 ttt_cld_2_idx_prev[MAX_NUM_TTT][MAX_PARAMETER_BANDS];
  WORD32 ttt_icc_idx_prev[MAX_NUM_TTT][MAX_PARAMETER_BANDS];

  WORD32
  cmp_ott_cld_idx[MAX_NUM_OTT + MAX_OUTPUT_CHANNELS_AT][MAX_PARAMETER_SETS]
                 [MAX_PARAMETER_BANDS];
  WORD32 cmp_ott_icc_idx[MAX_NUM_OTT][MAX_PARAMETER_SETS][MAX_PARAMETER_BANDS];
  WORD32 ott_icc_diff_idx[MAX_NUM_OTT][MAX_PARAMETER_SETS][MAX_PARAMETER_BANDS];
  WORD32 cmp_ttt_cpc_1_idx[MAX_NUM_TTT][MAX_PARAMETER_SETS]
                          [MAX_PARAMETER_BANDS];
  WORD32 cmp_ttt_cpc_2_idx[MAX_NUM_TTT][MAX_PARAMETER_SETS]
                          [MAX_PARAMETER_BANDS];
  WORD32 cmp_ttt_cld_1_idx[MAX_NUM_TTT][MAX_PARAMETER_SETS]
                          [MAX_PARAMETER_BANDS];
  WORD32 cmp_ttt_cld_2_idx[MAX_NUM_TTT][MAX_PARAMETER_SETS]
                          [MAX_PARAMETER_BANDS];
  WORD32 cmp_ttt_icc_idx[MAX_NUM_TTT][MAX_PARAMETER_SETS][MAX_PARAMETER_BANDS];
  WORD32 cmp_ott_cld_idx_prev[MAX_NUM_OTT + MAX_OUTPUT_CHANNELS_AT]
                             [MAX_PARAMETER_BANDS];
  WORD32 cmp_ott_icc_idx_prev[MAX_NUM_OTT][MAX_PARAMETER_BANDS];
  WORD32 cmp_ttt_cpc_1_idx_prev[MAX_NUM_TTT][MAX_PARAMETER_BANDS];
  WORD32 cmp_ttt_cpc_2_idx_prev[MAX_NUM_TTT][MAX_PARAMETER_BANDS];
  WORD32 cmp_ttt_cld_1_idx_prev[MAX_NUM_TTT][MAX_PARAMETER_BANDS];
  WORD32 cmp_ttt_cld_2_idx_prev[MAX_NUM_TTT][MAX_PARAMETER_BANDS];
  WORD32 cmp_ttt_icc_idx_prev[MAX_NUM_TTT][MAX_PARAMETER_BANDS];

  ia_mps_dec_lossless_data_struct cld_lossless_data;
  ia_mps_dec_lossless_data_struct icc_lossless_data;
  ia_mps_dec_lossless_data_struct cpc_lossless_data;

  WORD32 bs_smooth_control;
  WORD32 bs_smooth_mode[MAX_PARAMETER_SETS];
  WORD32 bs_smooth_time[MAX_PARAMETER_SETS];
  WORD32 bs_freq_res_stride_smg[MAX_PARAMETER_SETS];
  WORD32 bs_smg_data[MAX_PARAMETER_SETS][MAX_PARAMETER_BANDS];

  RESIDUAL_FRAME_DATA res_data;

  WORD32
  arbdmx_gain_idx[MAX_INPUT_CHANNELS_MPS][MAX_PARAMETER_SETS]
                 [MAX_PARAMETER_BANDS];
  WORD32 arbdmx_gain_idx_prev[MAX_INPUT_CHANNELS_MPS][MAX_PARAMETER_BANDS];
  WORD32
  cmp_arbdmx_gain_idx[MAX_INPUT_CHANNELS_MPS][MAX_PARAMETER_SETS]
                     [MAX_PARAMETER_BANDS];
  WORD32 cmp_arbdmx_gain_idx_prev[MAX_INPUT_CHANNELS_MPS][MAX_PARAMETER_BANDS];
  WORD32 bs_arbitrary_downmix_residual_abs[MAX_INPUT_CHANNELS_MPS];
  WORD32 bs_arbitrary_downmix_residual_alpha_update_set[MAX_INPUT_CHANNELS_MPS];

} ia_mps_dec_spatial_bs_frame_struct;

typedef struct {
  WORD32 spec_prev_real[MAX_NUM_QMF_BANDS * 8];
  WORD32 spec_prev_imag[MAX_NUM_QMF_BANDS * 8];
  WORD32 p_cross_real[MAX_NUM_QMF_BANDS * 8];
  WORD32 p_cross_imag[MAX_NUM_QMF_BANDS * 8];

  WORD32 p_sum[MAX_NUM_QMF_BANDS * 8];
  WORD32 p_sum_prev[MAX_NUM_QMF_BANDS * 8];

  WORD32 buf_real[MAX_NUM_QMF_BANDS][6];
  WORD32 buf_imag[MAX_NUM_QMF_BANDS][6];

  WORD32 win_buf_real[MAX_NUM_QMF_BANDS][16];
  WORD32 win_buf_imag[MAX_NUM_QMF_BANDS][16];
} ia_mps_dec_tonality_state_struct;

typedef struct {
  WORD32 prev_smg_time;
  WORD32 prev_smg_data[MAX_PARAMETER_BANDS];
} ia_mps_dec_smoothing_state_struct;

typedef struct {
  WORD32
  part_nrg_prev[2 * MAX_OUTPUT_CHANNELS_MPS + MAX_INPUT_CHANNELS_MPS]
               [MAX_PARAMETER_BANDS];
  WORD32 norm_nrg_prev[2 * MAX_OUTPUT_CHANNELS_MPS + MAX_INPUT_CHANNELS_MPS];
  WORD32 frame_nrg_prev[2 * MAX_OUTPUT_CHANNELS_MPS + MAX_INPUT_CHANNELS_MPS];

  WORD16
  q_part_nrg_prev[2 * MAX_OUTPUT_CHANNELS_MPS + MAX_INPUT_CHANNELS_MPS]
                 [MAX_PARAMETER_BANDS];
  WORD16 q_norm_nrg_prev[2 * MAX_OUTPUT_CHANNELS_MPS + MAX_INPUT_CHANNELS_MPS];
  WORD16 q_frame_nrg_prev[2 * MAX_OUTPUT_CHANNELS_MPS + MAX_INPUT_CHANNELS_MPS];

} ia_mps_dec_reshape_bb_env_state_struct;

typedef struct {
  WORD32 use_ttt_decorr;
  WORD32 mode;
  WORD32 start_band;
  WORD32 stop_band;
  WORD32 bitstream_start_band;
  WORD32 bitstream_stop_band;
} ia_mps_dec_ttt_config_struct;

typedef struct {
  WORD32 excitation[3][MAX_PARAMETER_BANDS];
  WORD32 filter_coeff;
  WORD16 q_excitation[3][MAX_PARAMETER_BANDS];
} ia_mps_dec_blind_decoder_struct;

typedef struct {
  WORD32 run_dry_ener[MAX_INPUT_CHANNELS_MPS];
  WORD32 run_wet_ener[MAX_OUTPUT_CHANNELS_MPS];
  WORD32 old_dry_ener[MAX_INPUT_CHANNELS_MPS];
  WORD32 old_wet_ener[MAX_OUTPUT_CHANNELS_MPS];

  WORD16 q_run_dry_ener[MAX_INPUT_CHANNELS_MPS];
  WORD16 q_run_wet_ener[MAX_OUTPUT_CHANNELS_MPS];
  WORD16 q_old_dry_ener[MAX_INPUT_CHANNELS_MPS];
  WORD16 q_old_wet_ener[MAX_OUTPUT_CHANNELS_MPS];
  WORD32 update_old_ener;

  WORD32 prev_tp_scale[MAX_OUTPUT_CHANNELS_MPS];
  WORD16 q_prev_tp_scale[MAX_OUTPUT_CHANNELS_MPS];
} ia_mps_dec_subband_tp_params_struct;

typedef struct ia_mps_persistent_mem {
  WORD32 *prev_gain_at;
  WORD32 *arbdmx_alpha_prev;
  WORD32 *m1_param_real_prev;
  WORD32 *m1_param_imag_prev;
  WORD32 *m2_decor_real_prev;
  WORD32 *m2_decor_imag_prev;
  WORD32 *m2_resid_real_prev;
  WORD32 *m2_resid_imag_prev;
  WORD32 *qmf_input_delay_real;
  WORD32 *qmf_input_delay_imag;
  WORD32 *ana_qmf_states_buffer;
  WORD32 *syn_qmf_states_buffer;
  VOID *decorr_ptr;

  ia_mps_dec_thyb_filter_state_struct *hyb_filter_state;
  ia_mps_dec_tonality_state_struct *ton_state;
  ia_mps_dec_smoothing_state_struct *smooth_state;
  ia_mps_dec_reshape_bb_env_state_struct *reshape_bb_env_state;
  ia_mps_dec_subband_tp_params_struct *sub_band_params;
  ia_mps_dec_blind_decoder_struct *blind_decoder;
  ia_mps_dec_spatial_bs_frame_struct *p_bs_frame;
} ia_mps_persistent_mem;

typedef struct {
  ia_mps_dec_qmf_tables_struct *qmf_table_ptr;
  ia_mps_dec_common_tables_struct *common_table_ptr;
  ia_mps_dec_hybrid_tables_struct *hybrid_table_ptr;
  ia_mps_dec_m1_m2_tables_struct *m1_m2_table_ptr;
  ia_mps_dec_decorr_tables_struct *decor_table_ptr;
  ia_mps_dec_tp_process_tables_struct *tp_process_table_ptr;
  ia_mps_dec_mdct2qmf_table_struct *mdct2qmf_table_ptr;
  ia_mps_dec_tonality_tables_struct *tonality_table_ptr;
  ia_mps_dec_bitdec_tables_struct *bitdec_table_ptr;
  ia_mps_dec_blind_tables_struct *blind_table_ptr;
  ia_mps_dec_mdct2qmf_tables_struct *mdct2qmfcos_table_ptr;
  ia_mps_dec_mdct2qmf_cos_table_struct *mdct2qmfcos_tab_ptr;
  VOID *aac_tab;
  ia_mps_dec_wf_ptr_table_struct *wf_tab_ptr;

} ia_mps_dec_mps_tables_struct;

typedef struct {
  VOID(*syn_filter_bank)
  (ia_mps_dec_qmf_syn_filter_bank *syn, WORD32 *sr, WORD32 *si,
   WORD32 *time_sig, WORD32 channel, WORD32 resolution, WORD32 nr_samples,
   ia_mps_dec_qmf_tables_struct *qmf_table_ptr);
} ia_mps_dec_synthesis_interface, *ia_mps_dec_synthesis_interface_handle;

typedef struct {
  WORD32
  m1_param_real[MAX_M1_OUTPUT][MAX_INPUT_CHANNELS_MPS][MAX_PARAMETER_SETS]
               [MAX_PARAMETER_BANDS];
  WORD32
  m1_param_imag[MAX_M1_OUTPUT][MAX_INPUT_CHANNELS_MPS][MAX_PARAMETER_SETS]
               [MAX_PARAMETER_BANDS];
} ia_mps_dec_m1_param_struct;

typedef struct {
  WORD32 *qmf_residual_real;
  WORD32 *qmf_residual_imag;
  WORD32 *qmf_residual_real_pre, *qmf_residual_real_post;
  WORD32 *qmf_residual_imag_pre, *qmf_residual_imag_post;
  WORD32 *res_mdct;
  WORD32 *time_out;
  WORD32 *x_real;
  WORD32 *x_imag;
  WORD32 *hyb_output_real_dry;
  WORD32 *hyb_output_imag_dry;
  WORD32 *env_dmx_0;
  WORD32 *env_dmx_1;
  WORD32 *m_qmf_real;
  WORD32 *m_qmf_imag;
  WORD32 *w_dry_real;
  WORD32 *w_dry_imag;
  WORD32 *buf_real;
  WORD32 *buf_imag;
  WORD32 *buffer_real;
  WORD32 *buffer_imag;
  ia_mps_dec_m1_param_struct *m1_param;
} ia_mps_dec_reuse_array_struct;

typedef struct {
  WORD32 m2_decor_real[15][MAX_PARAMETER_SETS]
                      [MAX_PARAMETER_BANDS];
  WORD32 m2_decor_imag[15][MAX_PARAMETER_SETS]
                      [MAX_PARAMETER_BANDS];

  WORD32 m2_resid_real[19][MAX_PARAMETER_SETS]
                      [MAX_PARAMETER_BANDS];
  WORD32 m2_resid_imag[19][MAX_PARAMETER_SETS]
                      [MAX_PARAMETER_BANDS];
} ia_mps_dec_m2_param_struct;

typedef struct {
  ia_mps_dec_m2_param_struct *m2_param;

  WORD32 temp_shape_enable_channel_stp[MAX_OUTPUT_CHANNELS_MPS];
  WORD32 temp_shape_enable_channel_ges[MAX_OUTPUT_CHANNELS_MPS];

  WORD32 env_shape_data[MAX_OUTPUT_CHANNELS_MPS][MAX_TIME_SLOTS];

  WORD32 num_ott_bands[MAX_NUM_OTT];

  ia_mps_dec_ttt_config_struct ttt_config[2][MAX_NUM_TTT];
  WORD32 param_slot[MAX_PARAMETER_SETS];

  WORD32 smg_time[MAX_PARAMETER_SETS];
  WORD32 smg_data[MAX_PARAMETER_SETS][MAX_PARAMETER_BANDS];

  WORD32
  ott_cld[MAX_NUM_OTT + MAX_OUTPUT_CHANNELS_AT][MAX_PARAMETER_SETS]
         [MAX_PARAMETER_BANDS];
  WORD32 ott_icc[MAX_NUM_OTT][MAX_PARAMETER_SETS]
                [MAX_PARAMETER_BANDS];

  WORD32 ttt_cpc_1[MAX_NUM_TTT][MAX_PARAMETER_SETS]
                  [MAX_PARAMETER_BANDS];
  WORD32 ttt_cpc_2[MAX_NUM_TTT][MAX_PARAMETER_SETS]
                  [MAX_PARAMETER_BANDS];
  WORD32 ttt_cld_1[MAX_NUM_TTT][MAX_PARAMETER_SETS]
                  [MAX_PARAMETER_BANDS];
  WORD32 ttt_cld_2[MAX_NUM_TTT][MAX_PARAMETER_SETS]
                  [MAX_PARAMETER_BANDS];
  WORD32 ttt_icc[MAX_NUM_TTT][MAX_PARAMETER_SETS]
                [MAX_PARAMETER_BANDS];

  WORD32
  arbdmx_gain[MAX_INPUT_CHANNELS_MPS][MAX_PARAMETER_SETS]
             [MAX_PARAMETER_BANDS];

  WORD32 arbdmx_residual_abs[MAX_INPUT_CHANNELS_MPS];
  WORD32 arbdmx_alpha_upd_set[MAX_INPUT_CHANNELS_MPS];
  WORD32 arbdmx_alpha[MAX_INPUT_CHANNELS_MPS];
} ia_mps_dec_auxilary_struct;

typedef struct ia_heaac_mps_state_struct {
  WORD32 sac_time_align_flag;
  WORD32 sac_time_align;
  WORD32 sampling_freq;

  WORD32 tree_config;
  WORD32 num_input_channels;
  WORD32 num_output_channels;
  WORD32 num_ott_boxes;
  WORD32 num_ttt_boxes;

  WORD32 num_output_channels_at;

  WORD32 quant_mode;
  WORD32 one_icc;
  WORD32 arbitrary_downmix;
  WORD32 residual_coding;
  WORD32 smooth_config;
  WORD32 temp_shape_config;
  WORD32 decorr_config;
  WORD32 mtx_inversion;
  WORD32 _3d_stereo_inversion;
  WORD32 env_quant_mode;

  WORD32 clip_protect_gain;
  WORD32 surround_gain;
  WORD32 lfe_gain;
  WORD32 cpc_default;
  WORD32 icc_default;
  WORD32 arbdmx_gain_default;

  WORD32 num_direct_signals;
  WORD32 num_residual_signals;
  WORD32 num_decor_signals;
  WORD32 num_v_channels;
  WORD32 num_w_channels;
  WORD32 w_start_residual_idx;
  WORD32 num_x_channels;

  WORD32 time_slots;
  WORD32 cur_time_slot;
  WORD32 frame_length;
  WORD32 dec_type;
  WORD32 up_mix_type;
  WORD32 binaural_quality;
  WORD32 hrtf_model;

  WORD32 tp_hyb_band_border;

  WORD32 parse_next_bitstream_frame;

  WORD32 qmf_bands;
  WORD32 hybrid_bands;

  WORD32 residual_frames_per_spatial_frame;
  WORD32 upd_qmf;

  WORD32 arbdmx_residual_bands;
  WORD32 arbdmx_frames_per_spatial_frame;
  WORD32 arbdmx_upd_qmf;

  WORD32 bitstream_parameter_bands;
  WORD32 num_parameter_bands;

  WORD32 extend_frame;
  WORD32 num_parameter_sets;
  WORD32 num_parameter_sets_prev;

  WORD32 smooth_control;

  WORD32 i_bytes_consumed_mps;
  WORD32 bytes_remaining;
  WORD32 ui_mps_in_bytes;
  WORD32 is_sbr_present;

  WORD32 bits_per_sample;
  WORD32 qmf_input_delay_index;

  WORD32 m1_param_imag_present;
  WORD32 m2_param_imag_present;

  WORD32 m1_param_present[MAX_M1_OUTPUT][MAX_INPUT_CHANNELS_MPS];
  WORD32 m2_param_present[MAX_M2_OUTPUT][MAX_M2_INPUT];

  WORD32 index[MAX_RESIDUAL_CHANNELS_MPS];

  WORD32 ott_cld_default[MAX_NUM_OTT];
  WORD32 ttt_cld_1_default[MAX_NUM_TTT];
  WORD32 ttt_cld_2_default[MAX_NUM_TTT];

  SIZE_T kernels[MAX_HYBRID_BANDS];

  WORD32 res_bands[MAX_RESIDUAL_CHANNELS_MPS];
  WORD32 ott_mode_lfe[MAX_NUM_OTT];
  WORD32 bitstream_ott_bands[MAX_NUM_OTT];

  WORD32 scaling_enable;

  WORD32 is_buried_flag;

  ia_mps_dec_residual_sfband_info_struct sfband_info_tab;
  WORD16 *pcm_out_buf;

  WORD32 res_block_type[MAX_RESIDUAL_CHANNELS_MPS][MAX_RESIDUAL_FRAMES];

  ia_mps_spatial_bs_config_struct bs_config;
  ia_mps_dec_decorr_dec_handle ap_decor[MAX_NO_DECORR_CHANNELS];
  ia_mps_dec_qmf_ana_filter_bank qmf_bank[6];
  ia_mps_dec_qmf_syn_filter_bank syn_qmf_bank;
  struct ia_bit_buf_struct mps_bit_buf, *ptr_mps_bit_buff;

  ia_mps_dec_spatial_bs_frame_struct *bs_frame;
  ia_mps_dec_reuse_array_struct *array_struct;
  ia_mps_dec_auxilary_struct *aux_struct;
  VOID *mps_scratch_mem_v;
  ia_mps_persistent_mem mps_persistent_mem;
  VOID *mps_persistent_mem_v;

  ia_mps_dec_synthesis_interface *syn;
  ia_mps_dec_residual_channel_info_struct *p_aac_decoder_channel_info[2];
  ia_mps_dec_residual_dynamic_data_struct *p_aac_decoder_dynamic_data_init[2];
  WORD8 tot_sf_bands_ls[2];

  ia_mps_dec_mps_tables_struct ia_mps_dec_mps_table;
  ia_mps_dec_residual_aac_tables_struct aac_table;
  ia_mps_dec_mdct2qmf_cos_table_struct ia_mps_dec_mdct2qmfcos_table;
  ia_mps_dec_wf_ptr_table_struct wf_tab;
  WORD32 is_first;
  WORD32 mps_decode;
  UWORD8 temp_buf[1024];
  WORD32 heaac_mps_present;
  WORD32 mps_with_sbr;
  WORD32 mps_init_done;
  WORD32 ec_flag;
  WORD32 frame_ok;
  WORD32 first_frame;

} ia_heaac_mps_state_struct;

WORD32 ixheaacd_mps_persistent_buffer_sizes();

WORD32 ixheaacd_getsize_mps_persistent();

VOID ixheaacd_set_mps_persistent_buffers(ia_heaac_mps_state_struct *pstr_mps_state,
                                         WORD32 *persistent_used,
                                         WORD32 num_channel,
                                         VOID *persistent_mem);

WORD32 ixheaacd_scratch_buffer_sizes();

VOID ixheaacd_set_scratch_buffers(ia_heaac_mps_state_struct *pstr_mps_state,
                                  VOID *scratch_mem);

VOID ixheaacd_calc_ana_filt_bank(ia_heaac_mps_state_struct *pstr_mps_state,
                                 WORD16 *time_in, WORD32 *p_qmf_real,
                                 WORD32 *p_qmf_imag, WORD32 channel);

IA_ERRORCODE
ixheaacd_syn_filt_bank_init(ia_mps_dec_synthesis_interface_handle self,
                            WORD32 resolution);
#endif /* IXHEAACD_MPS_DEC_H */
