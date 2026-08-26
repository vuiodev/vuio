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
#ifndef IXHEAACD_PS_DEC_H
#define IXHEAACD_PS_DEC_H

#define MAX_NUM_COLUMNS 32

#define MAX_NUM_COLUMNS_960 30
#define NUM_OF_QUAD_MIRROR_FILTER_CHNLS 64
#define NUM_OF_ALL_PASS_CHNLS 23
#define NUM_OF_DEL_CHNLS \
  (NUM_OF_QUAD_MIRROR_FILTER_CHNLS - NUM_OF_ALL_PASS_CHNLS)
#define DEL_ALL_PASS 2
#define SMALL_DEL_STRT 12
#define SMALL_DEL 1
#define HIGH_DEL 14
#define NUM_OF_QUAD_MIRROR_FILTER_ALL_PASS_CHNLS \
  (NUM_OF_ALL_PASS_CHNLS - NO_QMF_CHANNELS_IN_HYBRID)
#define NUM_OF_QUAD_MIRROR_FILTER_ICC_CHNLS \
  (NUM_OF_QUAD_MIRROR_FILTER_ALL_PASS_CHNLS + NUM_OF_DEL_CHNLS)
#define PEAK_DECAYING_FACT 0x620a

#define MAXIM_NUM_OF_PS_ENVLOPS 5
#define PSC_SQRT05F (0x5a82)
#define NUM_OF_BINS (20)
#define NUM_BANDS_FINE (34)

#define NUM_SUB_SAMPLES_960 30
#define CORE_CODEC_FRAME_SIZE 1024
#define NUM_SUB_SAMPLES (CORE_CODEC_FRAME_SIZE / 32)

#define NRG_INT_COEFF 0.75f
#define INIT_FILT_COEFF (1.0f - NRG_INT_COEFF)

#define NEGATE_IPD_MASK (0x00001000)

#define NUM_IPD_STEPS (8)
#define NUM_OPD_STEPS (8)

#define NUM_HI_RES_BINS (34)
#define NUM_MID_RES_BINS (20)

#define NUM_HI_RES_IPD_BINS (17)

#define NUM_MID_RES_IPD_BINS (11)

#define NUM_LOW_RES_IPD_BINS (5)

#ifndef _M_PI_
#define _M_PI_ (3.141592653589793238462643383279)
#endif
#define PSC_SQRT2  (1.41421356237309504880)
#define PSC_PIF ((FLOAT32)_M_PI_)
#define PSC_SQRT2F ((FLOAT32)PSC_SQRT2)

#define IPD_HALF_RANGE (PSC_PIF)
#define IPD_SCALE_FACTOR (IPD_HALF_RANGE / NUM_IPD_STEPS)
#define OPD_HALF_RANGE (PSC_PIF)
#define OPD_SCALE_FACTOR (OPD_HALF_RANGE / NUM_OPD_STEPS)
#define NEGATE_IPD_MASK (0x00001000)

#define DECAY_CUTOFF 3
#define DECAY_CUTOFF_HI_RES 5
#define DECAY_SLOPE 0.05f

#define PHASE_SMOOTH_HIST1 (0.5f)
#define PHASE_SMOOTH_HIST2 (0.25f)

#define NUM_QMF_BANDS_IN_HYBRID20 3
#define NUM_QMF_BANDS_IN_HYBRID34 5

#define MAX_NUM_QMF_CHANNELS_IN_HYBRID (NUM_QMF_BANDS_IN_HYBRID34)

#define PEAK_DECAY_FACTOR_FAST (0.765928338364649f)


typedef WORD16((*REVERB_BUFFERS_RI)[NUM_SER_AP_LINKS])[32 * 2];
typedef WORD16((REVERB_BUFFERS_CH_RI[5])[NUM_SER_AP_LINKS])[16 * 2];

typedef struct {
  WORD16 (*delay_buf_qmf_ap_re_im)[32 * 2];

  WORD16 (*delay_buf_qmf_ld_re_im)[SMALL_DEL_STRT * 2];

  WORD16 (*delay_buf_qmf_sd_re_im)[2 * 32];

  WORD16 delay_buf_idx_ser[NUM_SER_AP_LINKS];
  WORD16 delay_sample_ser[NUM_SER_AP_LINKS];

  REVERB_BUFFERS_RI delay_buf_qmf_ser_re_im;
  WORD16 delay_buf_idx;
  WORD16 delay_buf_idx_long;

  WORD32 *peak_decay_diff;
  WORD32 *energy_prev;
  WORD32 *peak_decay_diff_prev;

  WORD32 *ptr_hyb_left_re;
  WORD32 *ptr_hyb_left_im;
  WORD32 *ptr_hyb_right_re;
  WORD32 *ptr_hyb_right_im;

  WORD16 delay_buf_qmf_sub_re_im[DEL_ALL_PASS][16 * 2];
  REVERB_BUFFERS_CH_RI delay_buf_qmf_sub_ser_re_im;

  WORD16 h11_h12_vec[2 * 24];
  WORD16 h21_h22_vec[2 * 24];

  WORD16 H11_H12[2 * 24];
  WORD16 H21_H22[2 * 24];

  WORD16 delta_h11_h12[2 * 24];
  WORD16 delta_h21_h22[2 * 24];

  FLAG force_mono;

  WORD16 delay_buffer_scale;
  WORD16 usb;

  WORD16 iid_par_prev[NUM_BANDS_FINE];
  WORD16 icc_par_prev[NUM_BANDS_FINE];

  FLAG ps_data_present;

  FLAG enable_iid;
  FLAG enable_icc;

  FLAG enable_ext;

  WORD16 iid_mode;
  WORD16 icc_mode;
  FLAG iid_quant;

  FLAG frame_class;
  WORD16 num_env;
  WORD16 border_position[MAXIM_NUM_OF_PS_ENVLOPS + 2];

  FLAG iid_dt[MAXIM_NUM_OF_PS_ENVLOPS];
  FLAG icc_dt[MAXIM_NUM_OF_PS_ENVLOPS];

  WORD16 iid_par_table[MAXIM_NUM_OF_PS_ENVLOPS + 2][NUM_BANDS_FINE];
  WORD16 icc_par_table[MAXIM_NUM_OF_PS_ENVLOPS + 2][NUM_BANDS_FINE];

  ia_hybrid_struct str_hybrid;
  FLOAT32 hyb_left_re[CORE_CODEC_FRAME_SIZE / MAX_NUM_COLUMNS][MAX_NUM_COLUMNS];
  FLOAT32 hyb_left_im[CORE_CODEC_FRAME_SIZE / MAX_NUM_COLUMNS][MAX_NUM_COLUMNS];
  FLOAT32 hyb_right_re[CORE_CODEC_FRAME_SIZE / MAX_NUM_COLUMNS][MAX_NUM_COLUMNS];
  FLOAT32 hyb_right_im[CORE_CODEC_FRAME_SIZE / MAX_NUM_COLUMNS][MAX_NUM_COLUMNS];

  FLOAT32 h11_re_vec[NUM_HI_RES_BINS];
  FLOAT32 h11_im_vec[NUM_HI_RES_BINS];
  FLOAT32 h12_re_vec[NUM_HI_RES_BINS];
  FLOAT32 h12_im_vec[NUM_HI_RES_BINS];
  FLOAT32 h21_re_vec[NUM_HI_RES_BINS];
  FLOAT32 h21_im_vec[NUM_HI_RES_BINS];
  FLOAT32 h22_re_vec[NUM_HI_RES_BINS];
  FLOAT32 h22_im_vec[NUM_HI_RES_BINS];

  FLOAT32 h11_re_prev[NUM_HI_RES_BINS];
  FLOAT32 h11_im_prev[NUM_HI_RES_BINS];
  FLOAT32 h12_re_prev[NUM_HI_RES_BINS];
  FLOAT32 h12_im_prev[NUM_HI_RES_BINS];
  FLOAT32 h21_re_prev[NUM_HI_RES_BINS];
  FLOAT32 h21_im_prev[NUM_HI_RES_BINS];
  FLOAT32 h22_re_prev[NUM_HI_RES_BINS];
  FLOAT32 h22_im_prev[NUM_HI_RES_BINS];

  FLOAT32 qmf_delay_buf_re[HIGH_DEL][NUM_OF_QUAD_MIRROR_FILTER_CHNLS];
  FLOAT32 qmf_delay_buf_im[HIGH_DEL][NUM_OF_QUAD_MIRROR_FILTER_CHNLS];
  FLOAT32 sub_qmf_delay_buf_re[HIGH_DEL][NUM_OF_QUAD_MIRROR_FILTER_CHNLS];
  FLOAT32 sub_qmf_delay_buf_im[HIGH_DEL][NUM_OF_QUAD_MIRROR_FILTER_CHNLS];
  FLOAT32 ser_qmf_delay_buf_re[NUM_SER_AP_LINKS][5][NUM_OF_QUAD_MIRROR_FILTER_CHNLS];
  FLOAT32 ser_qmf_delay_buf_im[NUM_SER_AP_LINKS][5][NUM_OF_QUAD_MIRROR_FILTER_CHNLS];

  ia_hybrid_flt_struct *ptr_hybrid;
  ia_hybrid_flt_struct str_flt_hybrid20;
  ia_hybrid_flt_struct str_flt_hybrid34;
  WORD32 use_34_st_bands;
  WORD32 use_34_st_bands_prev;
  WORD32 ps_mode;

  WORD32 *ptr_group_borders;
  WORD32 num_groups;
  WORD32 num_sub_qmf_groups;
  WORD32 num_bins;
  WORD32 first_delay_gr;
  WORD32 *ptr_bins_group_map;
  WORD32 num_sub_samples;
  WORD32 num_chans;
  WORD32 use_pca_rot_flg;
  WORD32 freq_res_ipd;
  WORD32 delay_qmf_delay_buf_idx[NUM_OF_QUAD_MIRROR_FILTER_CHNLS];
  WORD32 delay_qmf_delay_num_samp[NUM_OF_QUAD_MIRROR_FILTER_CHNLS];
  FLOAT32 peak_decay_fast_bin[NUM_HI_RES_BINS];
  FLOAT32 prev_nrg_bin[NUM_HI_RES_BINS];
  FLOAT32 prev_peak_diff_bin[NUM_HI_RES_BINS];
  WORD32 ipd_idx_map_1[NUM_HI_RES_IPD_BINS];
  WORD32 opd_idx_map_1[NUM_HI_RES_IPD_BINS];
  WORD32 ipd_idx_map_2[NUM_HI_RES_IPD_BINS];
  WORD32 opd_idx_map_2[NUM_HI_RES_IPD_BINS];

  WORD32 ipd_idx_map[MAXIM_NUM_OF_PS_ENVLOPS][NUM_HI_RES_IPD_BINS];
  WORD32 opd_idx_map[MAXIM_NUM_OF_PS_ENVLOPS][NUM_HI_RES_IPD_BINS];

  FLOAT32 ser_sub_qmf_dealy_buf_re[NUM_SER_AP_LINKS][5][NUM_OF_QUAD_MIRROR_FILTER_CHNLS];
  FLOAT32 ser_sub_qmf_dealy_buf_im[NUM_SER_AP_LINKS][5][NUM_OF_QUAD_MIRROR_FILTER_CHNLS];

  FLOAT32 hyb_work_re_20[NUM_SUB_SAMPLES + HYBRID_FILTER_LENGTH - 1];
  FLOAT32 hyb_work_im_20[NUM_SUB_SAMPLES + HYBRID_FILTER_LENGTH - 1];
  FLOAT32 hyb_qmf_buf_re_20[MAX_NUM_QMF_CHANNELS_IN_HYBRID][HYBRID_FILTER_LENGTH - 1];
  FLOAT32 hyb_qmf_buf_im_20[MAX_NUM_QMF_CHANNELS_IN_HYBRID][HYBRID_FILTER_LENGTH - 1];
  FLOAT32 hyb_temp_re_20[NUM_SUB_SAMPLES][NUM_OF_QUAD_MIRROR_FILTER_CHNLS];
  FLOAT32 hyb_temp_im_20[NUM_SUB_SAMPLES][NUM_OF_QUAD_MIRROR_FILTER_CHNLS];

  FLOAT32 hyb_work_re_34[NUM_SUB_SAMPLES + HYBRID_FILTER_LENGTH - 1];
  FLOAT32 hyb_work_im_34[NUM_SUB_SAMPLES + HYBRID_FILTER_LENGTH - 1];
  FLOAT32 hyb_qmf_buf_re_34[MAX_NUM_QMF_CHANNELS_IN_HYBRID][HYBRID_FILTER_LENGTH - 1];
  FLOAT32 hyb_qmf_buf_im_34[MAX_NUM_QMF_CHANNELS_IN_HYBRID][HYBRID_FILTER_LENGTH - 1];
  FLOAT32 hyb_temp_re_34[NUM_SUB_SAMPLES][NUM_OF_QUAD_MIRROR_FILTER_CHNLS];
  FLOAT32 hyb_temp_im_34[NUM_SUB_SAMPLES][NUM_OF_QUAD_MIRROR_FILTER_CHNLS];

  FLOAT32 **pp_qmf_buf_real[2];
  FLOAT32 **pp_qmf_buf_imag[2];
  FLOAT32 *time_sample_buf[2];

} ia_ps_dec_struct;

typedef struct {
  FLAG enable_iid;
  FLAG enable_icc;
  WORD16 iid_mode;
  WORD16 icc_mode;
  FLAG frame_class;
  WORD32 freq_res_ipd;
  WORD16 border_position[MAXIM_NUM_OF_PS_ENVLOPS + 2];
  FLAG iid_dt[MAXIM_NUM_OF_PS_ENVLOPS];
  FLAG icc_dt[MAXIM_NUM_OF_PS_ENVLOPS];
  WORD16 iid_par_table[MAXIM_NUM_OF_PS_ENVLOPS + 2][NUM_BANDS_FINE];
  WORD16 icc_par_table[MAXIM_NUM_OF_PS_ENVLOPS + 2][NUM_BANDS_FINE];
} ia_ps_dec_config_struct;

VOID ixheaacd_create_psdec(ia_ps_dec_struct *ptr_ps_dec,
                           VOID *sbr_persistent_mem, WORD32 *ptr_overlap_buf,
                           WORD32 frame_size);

VOID ixheaacd_decorr_filter1_dec(ia_ps_dec_struct *ptr_ps_dec,
                                 ia_ps_tables_struct *ps_tables_ptr,
                                 WORD16 *transient_ratio);

VOID ixheaacd_decorr_filter1_armv7(ia_ps_dec_struct *ptr_ps_dec,
                                   ia_ps_tables_struct *ps_tables_ptr,
                                   WORD16 *transient_ratio);

VOID ixheaacd_decorr_filter2_dec(
    ia_ps_dec_struct *ptr_ps_dec, WORD32 *p_buf_left_real,
    WORD32 *p_buf_left_imag, WORD32 *p_buf_right_real, WORD32 *p_buf_right_imag,
    ia_ps_tables_struct *ps_tables_ptr, WORD16 *transient_ratio);

VOID ixheaacd_decorr_filter2_armv7(
    ia_ps_dec_struct *ptr_ps_dec, WORD32 *p_buf_left_real,
    WORD32 *p_buf_left_imag, WORD32 *p_buf_right_real, WORD32 *p_buf_right_imag,
    ia_ps_tables_struct *ps_tables_ptr, WORD16 *transient_ratio);

WORD32 ixheaacd_divide16_pos_dec(WORD32 op1, WORD32 op2);

WORD32 ixheaacd_divide16_pos_armv7(WORD32 op1, WORD32 op2);

VOID ixheaacd_decorrelation_dec(ia_ps_dec_struct *ptr_ps_dec,
                                WORD32 *p_buf_left_real,
                                WORD32 *p_buf_left_imag,
                                WORD32 *p_buf_right_real,
                                WORD32 *p_buf_right_imag,
                                ia_ps_tables_struct *ps_tables_ptr);

VOID ixheaacd_decorrelation_armv7(ia_ps_dec_struct *ptr_ps_dec,
                                  WORD32 *p_buf_left_real,
                                  WORD32 *p_buf_left_imag,
                                  WORD32 *p_buf_right_real,
                                  WORD32 *p_buf_right_imag,
                                  ia_ps_tables_struct *ps_tables_ptr);

VOID ixheaacd_init_ps_scale(ia_ps_dec_struct *ptr_ps_dec,
                            ia_sbr_scale_fact_struct *sbr_scale_factor);

VOID ixheaacd_init_rot_env(ia_ps_dec_struct *ptr_ps_dec, WORD16 env, WORD16 usb,
                           ia_sbr_tables_struct *sbr_tables_ptr,
                           const WORD16 *cos_sin_lookup_tab);

VOID ixheaacd_apply_ps(ia_ps_dec_struct *ptr_ps_dec, WORD32 **real_buf_left,
                       WORD32 **imag_buf_left, WORD32 *real_buf_right,
                       WORD32 *imag_buf_right,
                       ia_sbr_scale_fact_struct *sbr_scale_factor, WORD16 slot,
                       ia_sbr_tables_struct *sbr_tables_ptr, WORD no_col);

VOID ixheaacd_apply_rot_dec(ia_ps_dec_struct *ptr_ps_dec, WORD32 *p_qmf_left_re,
                            WORD32 *p_qmf_left_im, WORD32 *p_qmf_right_re,
                            WORD32 *p_qmf_right_im,
                            ia_sbr_tables_struct *sbr_tables_ptr,
                            const WORD16 *ptr_res);

VOID ixheaacd_apply_rot_armv7(ia_ps_dec_struct *ptr_ps_dec,
                              WORD32 *qmf_left_real, WORD32 *qmf_left_imag,
                              WORD32 *qmf_right_real, WORD32 *qmf_right_imag,
                              ia_sbr_tables_struct *sbr_tables_ptr,
                              const WORD16 *ptr_resol);

VOID ixheaacd_scale_ps_states(ia_ps_dec_struct *ptr_ps_dec, WORD16 scale);

VOID
ixheaacd_esbr_apply_ps(ia_ps_dec_struct * ptr_ps_dec,
                       FLOAT32 **pp_qmf_buf_re_left,
                       FLOAT32 **pp_qmf_buf_im_left,
                       FLOAT32 **pp_qmf_buf_re_right,
                       FLOAT32 **pp_qmf_buf_im_right,
                       WORD32 usb, ia_ps_tables_struct *ptr_ps_tables,
                       WORD32 num_time_slot);

VOID
ixheaacd_esbr_ps_de_correlate(ia_ps_dec_struct *ptr_ps_dec,
                              FLOAT32 **pp_qmf_buf_re_left,
                              FLOAT32 **pp_qmf_buf_im_left,
                              FLOAT32 **pp_qmf_buf_re_right,
                              FLOAT32 **pp_qmf_buf_im_right,
                              ia_ps_tables_struct *ptr_ps_tables);

VOID
ixheaacd_esbr_ps_apply_rotation(ia_ps_dec_struct *ptr_ps_dec,
                                FLOAT32 **pp_qmf_buf_re_left,
                                FLOAT32 **pp_qmf_buf_im_left,
                                FLOAT32 **pp_qmf_buf_re_right,
                                FLOAT32 **pp_qmf_buf_im_right,
                                ia_ps_tables_struct *ptr_ps_tables);

WORD32
ixheaacd_create_hyb_filterbank_esbr_ps(ia_hybrid_flt_struct *pHybrid,
                                       WORD32 frameSize,
                                       WORD32 noBands);

WORD32 ixheaacd_create_ps_esbr_dec(ia_ps_dec_struct *ptr_ps_dec_struct,
                                   ia_ps_tables_struct *ptr_ps_tables,
                                   UWORD32 noQmfChans,
                                   UWORD32 num_sub_samples,
                                   WORD32 ps_mode);

VOID ResetPsDec(ia_ps_dec_struct * ptr_ps_dec);
VOID ResetPsDeCor(ia_ps_dec_struct * ptr_ps_dec);

extern WORD16 ixheaacd_divideby2(WORD32 op);
extern WORD16 ixheaacd_divideby3(WORD32 op);

#endif
