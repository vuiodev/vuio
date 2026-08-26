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
#include <string.h>

#include "ixheaacd_sbr_common.h"
#include "ixheaac_type_def.h"

#include "ixheaac_constants.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops16.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaac_basic_ops.h"

#include "ixheaacd_intrinsics.h"
#include "ixheaacd_common_rom.h"
#include "ixheaacd_bitbuffer.h"
#include "ixheaacd_sbrdecsettings.h"
#include "ixheaacd_sbr_scale.h"
#include "ixheaacd_lpp_tran.h"
#include "ixheaacd_env_extr_part.h"
#include "ixheaacd_sbr_rom.h"
#include "ixheaacd_hybrid.h"
#include "ixheaacd_ps_dec.h"
#include "ixheaacd_env_extr.h"
#include "ixheaacd_qmf_dec.h"

#include "ixheaac_basic_op.h"
#include "ixheaacd_env_calc.h"

#include "ixheaacd_interface.h"

#include "ixheaacd_function_selector.h"
#include "ixheaacd_audioobjtypes.h"

#define DCT3_LEN (32)
#define DCT2_LEN (64)

#define LP_SHIFT_VAL 7
#define HQ_SHIFT_64 4
#define RADIXSHIFT 1
#define ROUNDING_SPECTRA 1
#define HQ_SHIFT_VAL 4

extern const WORD32 ixheaacd_ldmps_polyphase_filter_coeff_fix[1280];

VOID ixheaacd_dct2_64(WORD32 *x, WORD32 *X,
                      ia_qmf_dec_tables_struct *qmf_dec_tables_ptr,
                      WORD16 *filter_states) {
  ixheaacd_pretwdct2(x, X);

  ixheaacd_sbr_imdct_using_fft(qmf_dec_tables_ptr->w1024, 32, X, x,
                               qmf_dec_tables_ptr->dig_rev_table2_128,
                               qmf_dec_tables_ptr->dig_rev_table2_128,
                               qmf_dec_tables_ptr->dig_rev_table2_128,
                               qmf_dec_tables_ptr->dig_rev_table2_128);

  ixheaacd_fftposttw(x, qmf_dec_tables_ptr);

  ixheaacd_posttwdct2(x, filter_states, qmf_dec_tables_ptr);

  return;
}

VOID ixheaacd_cplx_anal_qmffilt(const WORD16 *time_sample_buf,
                                ia_sbr_scale_fact_struct *sbr_scale_factor,
                                WORD32 **qmf_real, WORD32 **qmf_imag,
                                ia_sbr_qmf_filter_bank_struct *qmf_bank,
                                ia_qmf_dec_tables_struct *qmf_dec_tables_ptr,
                                WORD32 ch_fac, WORD32 low_pow_flag,
                                WORD audio_object_type) {
  WORD32 i, k;
  WORD32 num_time_slots = qmf_bank->num_time_slots;

  WORD32 analysis_buffer[4 * NO_ANALYSIS_CHANNELS];
  WORD16 *filter_states = qmf_bank->core_samples_buffer;

  WORD16 *fp1, *fp2, *tmp;

  WORD16 *filter_1;
  WORD16 *filter_2;
  WORD16 *filt_ptr;
  if (audio_object_type != AOT_ER_AAC_ELD &&
      audio_object_type != AOT_ER_AAC_LD) {
    qmf_bank->filter_pos +=
        (qmf_dec_tables_ptr->qmf_c - qmf_bank->analy_win_coeff);
    qmf_bank->analy_win_coeff = qmf_dec_tables_ptr->qmf_c;
  } else {
    qmf_bank->filter_pos +=
        (qmf_dec_tables_ptr->qmf_c_eld3 - qmf_bank->analy_win_coeff);
    qmf_bank->analy_win_coeff = qmf_dec_tables_ptr->qmf_c_eld3;
  }

  filter_1 = qmf_bank->filter_pos;

  if (audio_object_type != AOT_ER_AAC_ELD &&
      audio_object_type != AOT_ER_AAC_LD) {
    filter_2 = filter_1 + 64;
  } else {
    filter_2 = filter_1 + 32;
  }

  sbr_scale_factor->st_lb_scale = 0;
  sbr_scale_factor->lb_scale = -10;
  if (!low_pow_flag) {
    if (audio_object_type != AOT_ER_AAC_ELD &&
        audio_object_type != AOT_ER_AAC_LD) {
      sbr_scale_factor->lb_scale = -8;
    } else {
      sbr_scale_factor->lb_scale = -9;
    }
    qmf_bank->cos_twiddle =
        (WORD16 *)qmf_dec_tables_ptr->sbr_sin_cos_twiddle_l32;
    qmf_bank->alt_sin_twiddle =
        (WORD16 *)qmf_dec_tables_ptr->sbr_alt_sin_twiddle_l32;
    if (audio_object_type != AOT_ER_AAC_ELD &&
        audio_object_type != AOT_ER_AAC_LD) {
      qmf_bank->t_cos = (WORD16 *)qmf_dec_tables_ptr->sbr_t_cos_sin_l32;
    } else {
      qmf_bank->t_cos =
          (WORD16 *)qmf_dec_tables_ptr->ixheaacd_sbr_t_cos_sin_l32_eld;
    }
  }

  fp1 = qmf_bank->anal_filter_states;
  fp2 = qmf_bank->anal_filter_states + NO_ANALYSIS_CHANNELS;

  if (audio_object_type == AOT_ER_AAC_ELD ||
      audio_object_type == AOT_ER_AAC_LD) {
    filter_2 = qmf_bank->filter_2;
    fp1 = qmf_bank->fp1_anal;
    fp2 = qmf_bank->fp2_anal;
  }

  for (i = 0; i < num_time_slots; i++) {
    for (k = 0; k < NO_ANALYSIS_CHANNELS; k++)
      filter_states[NO_ANALYSIS_CHANNELS - 1 - k] = time_sample_buf[ch_fac * k];

    if (audio_object_type != AOT_ER_AAC_ELD &&
        audio_object_type != AOT_ER_AAC_LD) {
      ixheaacd_sbr_qmfanal32_winadds(fp1, fp2, filter_1, filter_2,
                                     analysis_buffer, filter_states,
                                     time_sample_buf, ch_fac);
    } else {
      ixheaacd_sbr_qmfanal32_winadds_eld(fp1, fp2, filter_1, filter_2,
                                         analysis_buffer, filter_states,
                                         time_sample_buf, ch_fac);
    }

    time_sample_buf += NO_ANALYSIS_CHANNELS * ch_fac;

    filter_states -= NO_ANALYSIS_CHANNELS;
    if (filter_states < qmf_bank->anal_filter_states) {
      filter_states = qmf_bank->anal_filter_states + 288;
    }

    tmp = fp1;
    fp1 = fp2;
    fp2 = tmp;
    if (audio_object_type != AOT_ER_AAC_ELD &&
        audio_object_type != AOT_ER_AAC_LD) {
      filter_1 += 64;
      filter_2 += 64;
    } else {
      filter_1 += 32;
      filter_2 += 32;
    }

    filt_ptr = filter_1;
    filter_1 = filter_2;
    filter_2 = filt_ptr;
    if (audio_object_type != AOT_ER_AAC_ELD &&
        audio_object_type != AOT_ER_AAC_LD) {
      if (filter_2 > (qmf_bank->analy_win_coeff + 640)) {
        filter_1 = (WORD16 *)qmf_bank->analy_win_coeff;
        filter_2 = (WORD16 *)qmf_bank->analy_win_coeff + 64;
      }
    } else {
      if (filter_2 > (qmf_bank->analy_win_coeff + 320)) {
        filter_1 = (WORD16 *)qmf_bank->analy_win_coeff;
        filter_2 = (WORD16 *)qmf_bank->analy_win_coeff + 32;
      }
    }

    if (!low_pow_flag) {
      ixheaacd_fwd_modulation(analysis_buffer, qmf_real[i], qmf_imag[i],
                              qmf_bank, qmf_dec_tables_ptr, 0);
    } else {
      ixheaacd_dct3_32(
          (WORD32 *)analysis_buffer, qmf_real[i], qmf_dec_tables_ptr->dct23_tw,
          qmf_dec_tables_ptr->post_fft_tbl, qmf_dec_tables_ptr->w_16,
          qmf_dec_tables_ptr->dig_rev_table4_16);
    }
  }

  qmf_bank->filter_pos = filter_1;
  qmf_bank->core_samples_buffer = filter_states;

  if (audio_object_type == AOT_ER_AAC_ELD || audio_object_type == AOT_ER_AAC_LD)

  {
    qmf_bank->fp1_anal = fp1;
    qmf_bank->fp2_anal = fp2;
    qmf_bank->filter_2 = filter_2;
  }
}

VOID ixheaacd_cplx_anal_qmffilt_32(const WORD32 *time_sample_buf,
                                   ia_sbr_scale_fact_struct *sbr_scale_factor,
                                   WORD32 **qmf_real, WORD32 **qmf_imag,
                                   ia_sbr_qmf_filter_bank_struct *qmf_bank,
                                   ia_qmf_dec_tables_struct *qmf_dec_tables_ptr,
                                   WORD32 ch_fac, WORD32 ldsbr_present) {
  WORD32 i, k;
  WORD32 num_time_slots = qmf_bank->num_time_slots;

  WORD32 analysis_buffer[4 * NO_ANALYSIS_CHANNELS];
  WORD32 *filter_states = qmf_bank->core_samples_buffer_32;

  WORD32 *fp1, *fp2, *tmp;

  WORD32 *filter_1;
  WORD32 *filter_2;
  WORD32 *filt_ptr;
  WORD32 start_slot = 2;

  if (ldsbr_present) {
    qmf_bank->filter_pos_32 +=
        (qmf_dec_tables_ptr->qmf_c_ldsbr_mps - qmf_bank->analy_win_coeff_32);
    qmf_bank->analy_win_coeff_32 = qmf_dec_tables_ptr->qmf_c_ldsbr_mps;
  } else {
    qmf_bank->filter_pos_32 += (ixheaacd_ldmps_polyphase_filter_coeff_fix -
                                qmf_bank->analy_win_coeff_32);
    qmf_bank->analy_win_coeff_32 =
        (WORD32 *)ixheaacd_ldmps_polyphase_filter_coeff_fix;
  }

  filter_1 = qmf_bank->filter_pos_32;
  filter_2 = filter_1 + qmf_bank->no_channels;

  sbr_scale_factor->st_lb_scale = 0;
  sbr_scale_factor->lb_scale = -10;

  sbr_scale_factor->lb_scale = -9;
  if (qmf_bank->no_channels != 64) {
    qmf_bank->cos_twiddle =
        (WORD16 *)qmf_dec_tables_ptr->sbr_sin_cos_twiddle_l32;
    qmf_bank->alt_sin_twiddle =
        (WORD16 *)qmf_dec_tables_ptr->sbr_alt_sin_twiddle_l32;
  } else {
    qmf_bank->cos_twiddle =
        (WORD16 *)qmf_dec_tables_ptr->sbr_sin_cos_twiddle_l64;
    qmf_bank->alt_sin_twiddle =
        (WORD16 *)qmf_dec_tables_ptr->sbr_alt_sin_twiddle_l64;
  }
  qmf_bank->t_cos =
      (WORD16 *)qmf_dec_tables_ptr->ixheaacd_sbr_t_cos_sin_l32_eld;

  fp1 = qmf_bank->anal_filter_states_32;
  fp2 = qmf_bank->anal_filter_states_32 + qmf_bank->no_channels;

  filter_2 = qmf_bank->filter_2_32;
  fp1 = qmf_bank->fp1_anal_32;
  fp2 = qmf_bank->fp2_anal_32;

  for (i = start_slot; i < num_time_slots + start_slot; i++) {
    for (k = 0; k < qmf_bank->no_channels; k++)
      filter_states[qmf_bank->no_channels - 1 - k] =
          time_sample_buf[ch_fac * k];

    if (ldsbr_present) {
      ixheaacd_sbr_qmfanal32_winadd_eld_32(fp1, fp2, filter_1, filter_2,
                                           analysis_buffer);
    } else {
      ixheaacd_sbr_qmfanal32_winadd_eld_mps(fp1, fp2, filter_1, filter_2,
                                            analysis_buffer);
    }

    time_sample_buf += qmf_bank->no_channels * ch_fac;

    filter_states -= qmf_bank->no_channels;

    if (filter_states < qmf_bank->anal_filter_states_32) {
      filter_states = qmf_bank->anal_filter_states_32 +
                      ((qmf_bank->no_channels * 10) - qmf_bank->no_channels);
    }

    tmp = fp1;
    fp1 = fp2;
    fp2 = tmp;

    filter_1 += qmf_bank->no_channels;
    filter_2 += qmf_bank->no_channels;

    filt_ptr = filter_1;
    filter_1 = filter_2;
    filter_2 = filt_ptr;

    if (filter_2 >
        (qmf_bank->analy_win_coeff_32 + (qmf_bank->no_channels * 10))) {
      filter_1 = (WORD32 *)qmf_bank->analy_win_coeff_32;
      filter_2 = (WORD32 *)qmf_bank->analy_win_coeff_32 + qmf_bank->no_channels;
    }

    ixheaacd_fwd_modulation(analysis_buffer, qmf_real[i], qmf_imag[i], qmf_bank,
                            qmf_dec_tables_ptr, 1);
  }

  qmf_bank->filter_pos_32 = filter_1;
  qmf_bank->core_samples_buffer_32 = filter_states;

  qmf_bank->fp1_anal_32 = fp1;
  qmf_bank->fp2_anal_32 = fp2;
  qmf_bank->filter_2_32 = filter_2;
}

VOID ixheaacd_inv_modulation_lp(WORD32 *qmf_real, WORD16 *filter_states,
                                ia_sbr_qmf_filter_bank_struct *syn_qmf,
                                ia_qmf_dec_tables_struct *qmf_dec_tables_ptr) {
  WORD32 L = syn_qmf->no_channels;
  const WORD32 M = (L >> 1);
  WORD32 *dct_in = qmf_real;
  WORD32 time_out[2 * NO_SYNTHESIS_CHANNELS];

  WORD32 ui_rem = ((WORD32)(&time_out[0]) % 8);
  WORD32 *ptime_out = (pVOID)((WORD8 *)&time_out[0] + 8 - ui_rem);

  if (L == 64)
    ixheaacd_dec_DCT2_64_asm(dct_in, ptime_out, qmf_dec_tables_ptr->w1024,
                             qmf_dec_tables_ptr->dig_rev_table2_128,
                             qmf_dec_tables_ptr->post_fft_tbl,
                             qmf_dec_tables_ptr->dct23_tw, filter_states + M);
  else
    ixheaacd_dct2_32(dct_in, time_out, qmf_dec_tables_ptr, filter_states);

  filter_states[3 * M] = 0;
}

VOID ixheaacd_inv_emodulation(WORD32 *qmf_real,
                              ia_sbr_qmf_filter_bank_struct *syn_qmf,
                              ia_qmf_dec_tables_struct *qmf_dec_tables_ptr) {
  ixheaacd_cos_sin_mod(qmf_real, syn_qmf, (WORD16 *)qmf_dec_tables_ptr->w1024,
                       (WORD32 *)qmf_dec_tables_ptr->dig_rev_table2_128);
}

VOID ixheaacd_esbr_cos_sin_mod(WORD32 *subband,
                               ia_sbr_qmf_filter_bank_struct *qmf_bank,
                               WORD32 *p_twiddle, WORD32 *p_dig_rev_tbl) {
  WORD32 z;
  WORD32 temp[128];
  WORD32 scaleshift = 0;

  WORD32 M_2;
  WORD32 M = ixheaac_shr32(qmf_bank->no_channels, 1);

  const WORD32 *p_sin;
  const WORD32 *p_sin_cos;

  WORD32 subband_tmp[128];

  p_sin_cos = qmf_bank->esbr_cos_twiddle;
  ixheaacd_esbr_cos_sin_mod_loop1(subband, M, p_sin_cos, subband_tmp);

  M_2 = ixheaac_shr32(M, 1);
  if (M == 32) {
    ixheaacd_esbr_radix4bfly(p_twiddle, subband_tmp, 1, 8);
    ixheaacd_esbr_radix4bfly(p_twiddle + 48, subband_tmp, 4, 2);
    ixheaacd_postradixcompute2(subband, subband_tmp, p_dig_rev_tbl, 32);

    ixheaacd_esbr_radix4bfly(p_twiddle, &subband_tmp[64], 1, 8);
    ixheaacd_esbr_radix4bfly(p_twiddle + 48, &subband_tmp[64], 4, 2);
    ixheaacd_postradixcompute2(&subband[64], &subband_tmp[64], p_dig_rev_tbl,
                               32);

  }

  else if (M == 16) {
    ixheaacd_esbr_radix4bfly(p_twiddle, subband_tmp, 1, 4);
    ixheaacd_postradixcompute4(subband, subband_tmp, p_dig_rev_tbl, 16);

    ixheaacd_esbr_radix4bfly(p_twiddle, &subband_tmp[64], 1, 4);
    ixheaacd_postradixcompute4(&subband[64], &subband_tmp[64], p_dig_rev_tbl,
                               16);

  }

  else if (M == 12) {
    for (z = 0; z < (qmf_bank->no_channels >> 1); z++) {
      temp[z] = subband_tmp[2 * z];
      temp[12 + z] = subband_tmp[2 * z + 1];
    }

    ixheaacd_complex_fft_p3(temp, &temp[12], 12, -1, &scaleshift);

    for (z = 0; z < (qmf_bank->no_channels >> 1); z++) {
      subband[2 * z] = temp[z];
      subband[2 * z + 1] = temp[z + 12];
    }
    scaleshift = 0;
    for (z = 0; z < (qmf_bank->no_channels >> 1); z++) {
      temp[z] = subband_tmp[64 + 2 * z];
      temp[12 + z] = subband_tmp[64 + 2 * z + 1];
    }

    ixheaacd_complex_fft_p3(temp, &temp[12], 12, -1, &scaleshift);

    for (z = 0; z < (qmf_bank->no_channels >> 1); z++) {
      subband[64 + 2 * z] = temp[z];
      subband[64 + 2 * z + 1] = temp[z + 12];
    }

  }

  else {
    for (z = 0; z < (qmf_bank->no_channels >> 1); z++) {
      temp[z] = subband_tmp[2 * z];
      temp[8 + z] = subband_tmp[2 * z + 1];
    }

    (*ixheaacd_complex_fft_p2)(temp, &temp[8], 8, -1, &scaleshift);

    for (z = 0; z < (qmf_bank->no_channels >> 1); z++) {
      subband[2 * z] = temp[z] << scaleshift;
      subband[2 * z + 1] = temp[z + 8] << scaleshift;
    }
    scaleshift = 0;
    for (z = 0; z < (qmf_bank->no_channels >> 1); z++) {
      temp[z] = subband_tmp[64 + 2 * z];
      temp[8 + z] = subband_tmp[64 + 2 * z + 1];
    }

    (*ixheaacd_complex_fft_p2)(temp, &temp[8], 8, -1, &scaleshift);

    for (z = 0; z < (qmf_bank->no_channels >> 1); z++) {
      subband[64 + 2 * z] = temp[z] << scaleshift;
      subband[64 + 2 * z + 1] = temp[8 + z] << scaleshift;
    }
  }
  p_sin = qmf_bank->esbr_alt_sin_twiddle;
  ixheaacd_esbr_cos_sin_mod_loop2(subband, p_sin, M);
}

VOID ixheaacd_esbr_qmfsyn32_winadd(WORD32 *tmp1, WORD32 *tmp2, WORD32 *inp1,
                                   WORD32 *sample_buffer, WORD32 ch_fac) {
  WORD32 k;

  for (k = 0; k < 32; k++) {
    WORD64 syn_out = 0;

    syn_out =
        ixheaac_add64(syn_out, ixheaac_mult64(tmp1[0 + k], inp1[2 * (k + 0)]));
    syn_out =
        ixheaac_add64(syn_out, ixheaac_mult64(tmp1[128 + k], inp1[2 * (k + 64)]));
    syn_out =
        ixheaac_add64(syn_out, ixheaac_mult64(tmp1[256 + k], inp1[2 * (k + 128)]));
    syn_out =
        ixheaac_add64(syn_out, ixheaac_mult64(tmp1[384 + k], inp1[2 * (k + 192)]));
    syn_out =
        ixheaac_add64(syn_out, ixheaac_mult64(tmp1[512 + k], inp1[2 * (k + 256)]));

    syn_out =
        ixheaac_add64(syn_out, ixheaac_mult64(tmp2[64 + k], inp1[2 * (k + 32)]));
    syn_out =
        ixheaac_add64(syn_out, ixheaac_mult64(tmp2[192 + k], inp1[2 * (k + 96)]));
    syn_out =
        ixheaac_add64(syn_out, ixheaac_mult64(tmp2[320 + k], inp1[2 * (k + 160)]));
    syn_out =
        ixheaac_add64(syn_out, ixheaac_mult64(tmp2[448 + k], inp1[2 * (k + 224)]));
    syn_out =
        ixheaac_add64(syn_out, ixheaac_mult64(tmp2[576 + k], inp1[2 * (k + 288)]));

    sample_buffer[ch_fac * k] = (WORD32)(syn_out >> 31);
  }
}
