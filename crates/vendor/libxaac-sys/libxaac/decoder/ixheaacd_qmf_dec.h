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
#ifndef IXHEAACD_QMF_DEC_H
#define IXHEAACD_QMF_DEC_H

typedef struct {
  WORD32 no_channels;
  const WORD16 *analy_win_coeff;
  const WORD16 *p_filter;
  const WORD16 *cos_twiddle;
  const WORD16 *sin_twiddle;
  const WORD16 *alt_sin_twiddle;
  const WORD16 *t_cos;
  const WORD16 *t_sin;

  WORD16 *anal_filter_states;
  WORD16 *filter_states;
  WORD16 num_time_slots;

  WORD16 lsb;
  WORD16 usb;

  WORD16 qmf_filter_state_size;
  WORD16 *core_samples_buffer;
  WORD16 ana_offset;
  WORD16 *filter_pos;
  WORD16 *dummy_0;
  WORD16 ixheaacd_drc_offset;
  WORD16 *filter_pos_syn;
  WORD16 *dummy_1;

  WORD32 *analy_win_coeff_32;
  const WORD32 *p_filter_32;
  const WORD32 *esbr_cos_twiddle;
  const WORD32 *esbr_alt_sin_twiddle;
  const WORD32 *esbr_t_cos;
  WORD32 *anal_filter_states_32;
  WORD32 *state_new_samples_pos_low_32;
  WORD32 *filter_states_32;
  WORD32 *filter_pos_32;
  WORD32 *filter_pos_syn_32;

  WORD16 *fp1_anal;
  WORD16 *fp2_anal;
  WORD16 *filter_2;

  WORD16 *fp1_syn;
  WORD16 *fp2_syn;
  WORD16 sixty4;
  WORD32 *core_samples_buffer_32;
  WORD32 *fp1_anal_32;
  WORD32 *fp2_anal_32;
  WORD32 *filter_2_32;

} ia_sbr_qmf_filter_bank_struct;

VOID ixheaacd_cplx_anal_qmffilt(const WORD16 *time_inp,
                                ia_sbr_scale_fact_struct *sbr_scale_factor, WORD32 **qmf_real,
                                WORD32 **qmf_imag, ia_sbr_qmf_filter_bank_struct *qmf_bank,
                                ia_qmf_dec_tables_struct *qmf_dec_tables_ptr, WORD ch_fac,
                                WORD32 low_pow_flag, WORD audio_object_type);

VOID ixheaacd_cplx_anal_qmffilt_32(const WORD32 *time_inp,
                                   ia_sbr_scale_fact_struct *sbr_scale_factor,
                                   WORD32 **qmf_real, WORD32 **qmf_imag,
                                   ia_sbr_qmf_filter_bank_struct *qmf_bank,
                                   ia_qmf_dec_tables_struct *qmf_dec_tables_ptr,
                                   WORD ch_fac, WORD32 ldsbr_present);

VOID ixheaacd_cplx_synt_qmffilt(
    WORD32 **qmf_real, WORD32 **qmf_im, WORD32 split_slot,
    WORD32 *qmf_real_out[MAX_ENV_COLS], WORD32 *qmf_imag_out[MAX_ENV_COLS],
    ia_sbr_scale_fact_struct *sbr_scale_factor, WORD16 *time_out,
    ia_sbr_qmf_filter_bank_struct *qmf_bank, ia_ps_dec_struct *ptr_ps_dec,
    FLAG active, FLAG low_pow_flag, ia_sbr_tables_struct *sbr_tables_ptr,
    ixheaacd_misc_tables *pstr_common_tables, WORD ch_fac, FLAG drc_on,
    WORD32 drc_sbr_factors[][64], WORD32 audio_object_type);

VOID ixheaacd_esbr_qmfanal32_winadd(WORD32 *inp1, WORD32 *inp2,
                                    WORD32 *tmp_qmf_1, WORD32 *tmp_qmf_2,
                                    WORD32 *out, WORD32 num_band);

VOID ixheaacd_esbr_fwd_modulation(const WORD32 *time_in, WORD32 *r_subband,
                                  WORD32 *i_subband,
                                  ia_sbr_qmf_filter_bank_struct *qmf_bank,
                                  ia_qmf_dec_tables_struct *qmf_dec_tables_ptr);

VOID ixheaacd_esbr_inv_modulation(WORD32 *qmf_real,
                                  ia_sbr_qmf_filter_bank_struct *syn_qmf,
                                  ia_qmf_dec_tables_struct *qmf_dec_tables_ptr,
                                  WORD32 no_synthesis_channels);

VOID ixheaacd_shiftrountine_with_rnd_hq(WORD32 *qmf_real, WORD32 *qmf_imag,
                                        WORD32 *filter_states, WORD32 len,
                                        WORD32 shift);

VOID ixheaacd_esbr_qmfsyn64_winadd(WORD32 *tmp1, WORD32 *tmp2, WORD32 *inp1,
                                   WORD32 *sample_buffer, WORD32 ch_fac);

VOID ixheaacd_esbr_qmfsyn32_winadd(WORD32 *tmp1, WORD32 *tmp2, WORD32 *inp1,
                                   WORD32 *sample_buffer, WORD32 ch_fac);

VOID ixheaacd_sbr_qmfanal32_winadds(WORD16 *fp1, WORD16 *fp2, WORD16 *filter_1,
                                    WORD16 *filter_2, WORD32 *analysis_buffer,
                                    WORD16 *filter_states,
                                    const WORD16 *time_sample_buf,
                                    WORD32 ch_fac);

VOID ixheaacd_sbr_qmfanal32_winadds_eld(WORD16 *fp1, WORD16 *fp2,
                                        WORD16 *filter_1, WORD16 *filter_2,
                                        WORD32 *analysis_buffer,
                                        WORD16 *filter_states,
                                        const WORD16 *time_sample_buf,
                                        WORD32 ch_fac);

VOID ixheaacd_fwd_modulation(const WORD32 *p_time_in1, WORD32 *real_subband,
                             WORD32 *imag_subband,
                             ia_sbr_qmf_filter_bank_struct *qmf_bank,
                             ia_qmf_dec_tables_struct *qmf_dec_tables_ptr,
                             WORD32 ld_mps_flag);
VOID ixheaacd_dct3_32(WORD32 *input, WORD32 *output,
                      const WORD16 *main_twidle_fwd, const WORD16 *post_tbl,
                      const WORD16 *w_16, const WORD32 *p_table);

VOID ixheaacd_dec_DCT2_64_asm(WORD32 *dct_in, WORD32 *ptime_out, WORD32 *w1024,
                              UWORD8 *dig_rev_table2_128, WORD16 *post_fft_tbl,
                              WORD16 *dct23_tw, WORD16 *filter_states);
VOID ixheaacd_cos_sin_mod(WORD32 *subband,
                          ia_sbr_qmf_filter_bank_struct *qmf_bank,
                          WORD16 *p_twiddle, WORD32 *p_dig_rev_tbl);
VOID ixheaacd_shiftrountine(WORD32 *qmf_real, WORD32 *qmf_imag, WORD32 len,
                            WORD32 common_shift);
VOID ixheaacd_shiftrountine_with_rnd(WORD32 *qmf_real, WORD32 *qmf_imag,
                                     WORD16 *filter_states, WORD32 len,
                                     WORD32 shift);

VOID ixheaacd_radix4bfly(const WORD16 *w, WORD32 *x, WORD32 npoints,
                         WORD32 ch_fac);

VOID ixheaacd_postradixcompute4(WORD32 *ptr_y, WORD32 *ptr_x,
                                const WORD32 *p_dig_rev_tbl, WORD32 npoints);

VOID ixheaacd_postradixcompute2(WORD32 *ptr_y, WORD32 *ptr_x,
                                const WORD32 *p_dig_rev_tbl, WORD32 npoints);

VOID ixheaacd_shiftrountine_with_rnd_eld(WORD32 *qmf_real, WORD32 *qmf_imag,
                                         WORD16 *filter_states, WORD32 len,
                                         WORD32 shift);

void ixheaacd_sbr_imdct_using_fft(const WORD32 *ptr_w, WORD32 npoints,
                                  WORD32 *ptr_x, WORD32 *ptr_y,
                                  UWORD8 *bit_rev_1024, UWORD8 *bit_rev_512,
                                  UWORD8 *bit_rev_128, UWORD8 *bit_rev_32);

VOID ixheaacd_esbr_cos_sin_mod_loop1(WORD32 *subband, WORD32 M,
                                     const WORD32 *p_sin_cos,
                                     WORD32 subband_tmp[]);

VOID ixheaacd_esbr_cos_sin_mod_loop2(WORD32 *subband, const WORD32 *p_sin,
                                     WORD32 M);

VOID ixheaacd_esbr_radix4bfly(const WORD32 *p_twiddle, WORD32 subband_tmp[],
                              WORD32 a, WORD32 npoint);

VOID ixheaacd_radix4bfly(const WORD16 *w, WORD32 *x, WORD32 npoints,
                         WORD32 ch_fac);

VOID ixheaacd_postradixcompute4(WORD32 *ptr_y, WORD32 *ptr_x,
                                const WORD32 *p_dig_rev_tbl, WORD32 npoints);

VOID ixheaacd_postradixcompute2(WORD32 *ptr_y, WORD32 *ptr_x,
                                const WORD32 *p_dig_rev_tbl, WORD32 npoints);

VOID ixheaacd_cos_sin_mod_loop1(WORD32 *subband, WORD32 M,
                                const WORD16 *p_sin_cos, WORD32 subband_tmp[]);

VOID ixheaacd_cos_sin_mod_loop2(WORD32 *subband, const WORD16 *p_sin, WORD32 M);

VOID ixheaacd_sbr_qmfsyn64_winadd(WORD16 *tmp1, WORD16 *tmp2, WORD16 *inp1,
                                  WORD16 *sample_buffer, FLAG shift,
                                  WORD32 ch_fac);

VOID ixheaacd_esbr_cos_sin_mod(WORD32 *subband,
                               ia_sbr_qmf_filter_bank_struct *qmf_bank,
                               WORD32 *p_twiddle, WORD32 *p_dig_rev_tbl);

VOID ixheaacd_inv_modulation_lp(WORD32 *qmf_real, WORD16 *filter_states,
                                ia_sbr_qmf_filter_bank_struct *syn_qmf,
                                ia_qmf_dec_tables_struct *qmf_dec_tables_ptr);

VOID ixheaacd_sbr_pre_twiddle(WORD32 *p_xre, WORD32 *p_xim, WORD16 *p_twiddles);

VOID ixheaacd_inv_emodulation(WORD32 *qmf_real,
                              ia_sbr_qmf_filter_bank_struct *syn_qmf,
                              ia_qmf_dec_tables_struct *qmf_dec_tables_ptr);

VOID ixheaacd_pretwdct2(WORD32 *inp, WORD32 *out_fwd);

VOID ixheaacd_sbr_qmfanal32_winadd_eld(WORD16 *inp1, WORD16 *inp2,
                                       const WORD16 *p_qmf1,
                                       const WORD16 *p_qmf2, WORD32 *p_out);

VOID ixheaacd_sbr_qmfanal32_winadd_eld_32(WORD32 *inp1, WORD32 *inp2,
                                          const WORD32 *p_qmf1,
                                          const WORD32 *p_qmf2, WORD32 *p_out);

VOID ixheaacd_sbr_qmfanal32_winadd_eld_mps(WORD32 *inp1, WORD32 *inp2,
                                           const WORD32 *p_qmf1,
                                           const WORD32 *p_qmf2, WORD32 *p_out);

VOID ixheaacd_dct2_32(WORD32 *inp, WORD32 *out,
                      ia_qmf_dec_tables_struct *qmf_dec_tables_ptr,
                      WORD16 *filter_states);

VOID ixheaacd_inv_modulation_lp(WORD32 *qmf_real, WORD16 *filter_states,
                                ia_sbr_qmf_filter_bank_struct *syn_qmf,
                                ia_qmf_dec_tables_struct *qmf_dec_tables_ptr);

VOID ixheaacd_fftposttw(WORD32 *out,
                        ia_qmf_dec_tables_struct *qmf_dec_tables_ptr);

VOID ixheaacd_posttwdct2(WORD32 *inp, WORD16 *out_fwd,
                         ia_qmf_dec_tables_struct *qmf_dec_tables_ptr);

#endif
