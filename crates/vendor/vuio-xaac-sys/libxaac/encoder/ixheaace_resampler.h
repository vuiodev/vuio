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
#define LEN_RING_BUF (12)
#define LEN_RING_BUF_SOS_1 (2)
#define LEN_RING_BUF_SOS_2 (10)
#define IIR_NUM_COEFFS (6)
#define IIR_DEN_COEFFS (11)
#define IIR_SOS_STAGES (5)
#define IIR_SOS_COEFFS (3)

typedef struct {
  const FLOAT32 *ptr_coeff_iir_num;
  const FLOAT32 *ptr_coeff_iir_den;
  WORD32 max;
  FLOAT32 ring_buf_1[LEN_RING_BUF];
  FLOAT32 ring_buf_2[LEN_RING_BUF];
} ixheaace_iir_filter;

typedef struct {
  const FLOAT32 *ptr_coeff_iir_num;
  const FLOAT32 *ptr_coeff_iir_den;
  FLOAT32 gain_sos;
  FLOAT32 ring_buf_sos_1[LEN_RING_BUF_SOS_1];
  FLOAT32 ring_buf_sos_2[LEN_RING_BUF_SOS_2];
} ixheaace_iir_sos_filter;

struct ixheaace_iir_params {
  const FLOAT32 coeff_iir_num[IIR_NUM_COEFFS];
  const FLOAT32 coeff_iir_den[IIR_DEN_COEFFS];
  const WORD32 max;
  const WORD32 delay;
};

struct ixheaace_iir_params_sos {
  const FLOAT32 coeff_iir_sos_num[IIR_SOS_STAGES][IIR_SOS_COEFFS];
  const FLOAT32 coeff_iir_sos_den[IIR_SOS_STAGES][IIR_SOS_COEFFS];
  const FLOAT32 gain_sos;
  const WORD32 delay;
};

typedef struct {
  struct ixheaace_iir_params const iir_param_set;
} ixheaace_resampler_table;

typedef struct {
  struct ixheaace_iir_params_sos const iir_param_set_sos;
} ixheaace_resampler_sos_table;

typedef struct {
  ixheaace_iir_filter iir_filter;
  WORD32 ratio;
  WORD32 delay;
  WORD32 pending;
} ixheaace_iir21_resampler;

typedef struct {
  ixheaace_iir_sos_filter iir_filter;
  WORD32 ratio;
  WORD32 delay;
  WORD32 pending;
} ixheaace_iir_sos_resampler;

typedef struct {
  FLOAT32 downsampler_in_buffer[INPUT_LEN_DOWNSAMPLE * IXHEAACE_MAX_CH_IN_BS_ELE * UPSAMPLE_FAC];
  FLOAT32
  downsampler_out_buffer[INPUT_LEN_DOWNSAMPLE * IXHEAACE_MAX_CH_IN_BS_ELE * UPSAMPLE_FAC];
  FLOAT32 scratch_buf1_temp[IIR_SOS_COEFFS];
  FLOAT32 scratch_buf2_temp[IIR_SOS_COEFFS];
  FLOAT32 ring_buf_temp[LEN_RING_BUF];
} ixheaace_resampler_scratch;

WORD32 ixheaace_resampler_scr_size(VOID);

VOID ixheaace_get_input_scratch_buf(VOID *ptr_scr, FLOAT32 **ptr_scratch_buf_inp);

IA_ERRORCODE ia_enhaacplus_enc_init_iir_resampler(ixheaace_iir21_resampler *pstr_resampler,
                                                  WORD32 ratio,
                                                  ixheaace_resampler_table *pstr_resampler_table);

VOID ia_enhaacplus_enc_iir_downsampler(ixheaace_iir21_resampler *pstr_down_sampler,
                                       FLOAT32 *ptr_in_samples, WORD32 num_in_samples,
                                       WORD32 in_stride, FLOAT32 *ptr_out_samples,
                                       WORD32 *num_out_samples, WORD32 out_stride,
                                       FLOAT32 *ptr_ring_buf1, FLOAT32 *ptr_ring_buf2,
                                       ixheaace_resampler_scratch *pstr_resampler_scratch);

VOID ia_enhaacplus_enc_iir_sos_downsampler(ixheaace_iir_sos_resampler *pstr_down_sampler,
                                           FLOAT32 *ptr_in_samples, WORD32 num_in_samples,
                                           WORD32 in_stride, FLOAT32 *ptr_out_samples,
                                           WORD32 *num_out_samples, FLOAT32 *ptr_ring_buf1,
                                           FLOAT32 *ptr_ring_buf2,
                                           ixheaace_resampler_scratch *pstr_resampler_scratch);

VOID ia_enhaacplus_enc_iir_sos_upsampler(ixheaace_iir_sos_resampler *pstr_up_sampler,
                                         FLOAT32 *ptr_in_samples, WORD32 num_in_samples,
                                         WORD32 in_stride, FLOAT32 *ptr_out_samples,
                                         WORD32 *num_out_samples, FLOAT32 *ptr_ring_buf1,
                                         FLOAT32 *ptr_ring_buf2,
                                         ixheaace_resampler_scratch *pstr_resampler_scratch);

WORD32 ia_enhaacplus_enc_compute_resampling_ratio(WORD32 ccfl_idx);

VOID ixheaace_upsampling_inp_buf_generation(FLOAT32 *ptr_inp_buf, FLOAT32 *ptr_temp_buf,
                                            WORD32 num_samples, WORD32 upsamp_fac, WORD32 offset);

IA_ERRORCODE
ia_enhaacplus_enc_init_iir_sos_resampler(ixheaace_iir_sos_resampler *pstr_resampler, WORD32 ratio,
                                         ixheaace_resampler_sos_table *pstr_resampler_table);