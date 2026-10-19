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

#include <string.h>
#include "ixheaac_type_def.h"
#include "ixheaac_constants.h"
#include "ixheaace_sbr_def.h"
#include "ixheaace_resampler.h"
#include "iusace_cnst.h"

static VOID ia_enhaacplus_enc_downsample_iir_filter(
    ixheaace_iir21_resampler *pstr_down_sampler, FLOAT32 *ptr_in_samples,
    FLOAT32 *ptr_out_samples, WORD32 in_stride, WORD32 num_in_samples, WORD32 *num_out_samples,
    WORD32 out_stride, FLOAT32 *ptr_ring_buf1, FLOAT32 *ptr_ring_buf2) {
  WORD32 i;
  ixheaace_iir_filter *pstr_iir_filter = &(pstr_down_sampler->iir_filter);
  FLOAT32 *ptr_iir_ring1, *ptr_iir_ring2;
  FLOAT32 coeff_temp_a, coeff_temp_b;
  const FLOAT32 *ptr_coeff_filt = (FLOAT32 *)pstr_iir_filter->ptr_coeff_iir_num;
  FLOAT32 iir_out = 0;
  FLOAT32 temp1 = 0.f, temp2 = 0.f;

  *num_out_samples = 0;
  ptr_iir_ring1 = ptr_ring_buf1;
  ptr_iir_ring2 = ptr_ring_buf2;

  for (i = 0; i < num_in_samples; i++) {
    temp1 = ptr_in_samples[i * in_stride] / 2;
    ptr_iir_ring1[i] = temp1;
    coeff_temp_b = *ptr_coeff_filt;
    temp1 = coeff_temp_b * (temp1 + ptr_iir_ring1[i - 10]);
    coeff_temp_a = *(ptr_coeff_filt + 16);
    iir_out = (ptr_iir_ring2[i - 10] * coeff_temp_a);

    coeff_temp_b = *(ptr_coeff_filt + 1);
    temp1 = (coeff_temp_b * (ptr_iir_ring1[i - 1] + ptr_iir_ring1[i - 9])) + temp1;
    coeff_temp_a = *(ptr_coeff_filt + 7);
    temp2 = (ptr_iir_ring2[i - 1] * coeff_temp_a) + iir_out;
    coeff_temp_a = *(ptr_coeff_filt + 15);
    iir_out = (ptr_iir_ring2[i - 9] * coeff_temp_a) + temp2;

    coeff_temp_b = *(ptr_coeff_filt + 2);
    temp1 = (coeff_temp_b * (ptr_iir_ring1[i - 2] + ptr_iir_ring1[i - 8])) + temp1;
    coeff_temp_a = *(ptr_coeff_filt + 8);
    temp2 = (ptr_iir_ring2[i - 2] * coeff_temp_a) + iir_out;
    coeff_temp_a = *(ptr_coeff_filt + 14);
    iir_out = (ptr_iir_ring2[i - 8] * coeff_temp_a) + temp2;

    coeff_temp_b = *(ptr_coeff_filt + 3);
    temp1 = (coeff_temp_b * (ptr_iir_ring1[i - 3] + ptr_iir_ring1[i - 7])) + temp1;
    coeff_temp_a = *(ptr_coeff_filt + 9);
    temp2 = (ptr_iir_ring2[i - 3] * coeff_temp_a) + iir_out;
    coeff_temp_a = *(ptr_coeff_filt + 13);
    iir_out = (ptr_iir_ring2[i - 7] * coeff_temp_a) + temp2;

    coeff_temp_b = *(ptr_coeff_filt + 4);
    temp1 = (coeff_temp_b * (ptr_iir_ring1[i - 4] + ptr_iir_ring1[i - 6])) + temp1;
    coeff_temp_a = *(ptr_coeff_filt + 10);
    temp2 = (ptr_iir_ring2[i - 4] * coeff_temp_a) + iir_out;
    coeff_temp_a = *(ptr_coeff_filt + 12);
    iir_out = (ptr_iir_ring2[i - 6] * coeff_temp_a) + temp2;

    coeff_temp_b = *(ptr_coeff_filt + 5);
    temp1 = (coeff_temp_b * ptr_iir_ring1[i - 5]) + temp1;
    coeff_temp_a = *(ptr_coeff_filt + 11);
    iir_out = (ptr_iir_ring2[i - 5] * coeff_temp_a) + iir_out;
    iir_out = temp1 - iir_out;

    ptr_iir_ring2[i] = iir_out;
    iir_out = (iir_out * (4681.0f / 32767.0f) * pstr_iir_filter->max);

    pstr_down_sampler->pending++;

    if (pstr_down_sampler->pending == pstr_down_sampler->ratio) {
      ptr_out_samples[(*num_out_samples) * out_stride] = iir_out;
      (*num_out_samples)++;

      pstr_down_sampler->pending = 0;
    }
  }
}

static VOID ia_enhaacplus_enc_copy_ring_buffers(FLOAT32 *ptr_iir_ring1, FLOAT32 *ptr_iir_ring2,
                                                FLOAT32 *ptr_ring_buf1, FLOAT32 *ptr_ring_buf2) {
  WORD32 i;
  FLOAT32 temp1, temp2, temp3;
  for (i = (LEN_RING_BUF / 2) - 1; i >= 0; i--) {
    temp1 = *ptr_iir_ring1++;
    temp2 = *ptr_iir_ring2++;
    temp3 = *ptr_iir_ring1++;

    *ptr_ring_buf1++ = temp1;
    temp1 = *ptr_iir_ring2++;
    *ptr_ring_buf2++ = temp2;
    *ptr_ring_buf1++ = temp3;
    *ptr_ring_buf2++ = temp1;
  }
}

static VOID ia_enhaacplus_enc_copy_ring_buffers_sos(FLOAT32 *ptr_iir_ring1,
                                                    FLOAT32 *ptr_iir_ring2,
                                                    FLOAT32 *ptr_ring_buf1,
                                                    FLOAT32 *ptr_ring_buf2, WORD32 len1,
                                                    WORD32 len2) {
  memcpy(ptr_ring_buf1, ptr_iir_ring1, len1 * sizeof(*ptr_ring_buf1));
  memcpy(ptr_ring_buf2, ptr_iir_ring2, len2 * sizeof(*ptr_ring_buf2));
}

static VOID ia_enhaacplus_enc_update_ring_buffer_sos(FLOAT32 *ptr_ring_buf, FLOAT32 *ptr_samples,
                                                     WORD32 len, WORD32 in_stride,
                                                     WORD32 coeff_idx) {
  ptr_ring_buf[2 * coeff_idx] = ptr_samples[len - 2 * in_stride];
  ptr_ring_buf[2 * coeff_idx + 1] = ptr_samples[len - in_stride];
}

VOID ixheaace_get_input_scratch_buf(VOID *ptr_scr, FLOAT32 **ptr_scratch_buf_inp) {
  ixheaace_resampler_scratch *pstr_resampler_scr = (ixheaace_resampler_scratch *)ptr_scr;

  *ptr_scratch_buf_inp = pstr_resampler_scr->downsampler_in_buffer;
}

WORD32 ixheaace_resampler_scr_size(VOID) {
  return IXHEAAC_GET_SIZE_ALIGNED(sizeof(ixheaace_resampler_scratch), BYTE_ALIGN_8);
}

static VOID ia_enhaacplus_enc_iir_sos_filter(ixheaace_iir_sos_resampler *pstr_down_sampler,
                                             FLOAT32 *ptr_in_samples, WORD32 in_stride,
                                             FLOAT32 *ptr_out_samples, WORD32 num_in_samples,
                                             FLOAT32 *ptr_ring_buf1, FLOAT32 *ptr_ring_buf2,
                                             WORD32 coeff_idx) {
  WORD32 i;
  ixheaace_iir_sos_filter *pstr_iir_filter = &(pstr_down_sampler->iir_filter);
  const FLOAT32 *ptr_coeff_den =
      (pstr_iir_filter->ptr_coeff_iir_den + (coeff_idx * IIR_SOS_COEFFS));
  const FLOAT32 *ptr_coeff_num =
      (pstr_iir_filter->ptr_coeff_iir_num + (coeff_idx * IIR_SOS_COEFFS));
  FLOAT32 iir_out = 0.f;
  FLOAT32 temp1 = 0.f, temp2 = 0.f;

  for (i = 0; i < num_in_samples; i++) {
    ptr_ring_buf1[2] = ptr_in_samples[i * in_stride];

    temp1 = ptr_coeff_num[0] * ptr_in_samples[i * in_stride] +
            ptr_coeff_num[1] * ptr_ring_buf1[1] + ptr_coeff_num[2] * ptr_ring_buf1[0];
    temp2 = ptr_coeff_den[1] * ptr_ring_buf2[1] + ptr_coeff_den[2] * ptr_ring_buf2[0];

    iir_out = temp1 - temp2;
    ptr_ring_buf2[2] = iir_out;

    ptr_out_samples[i * in_stride] = iir_out;

    // Shift ring buffers
    ptr_ring_buf1[0] = ptr_ring_buf1[1];
    ptr_ring_buf1[1] = ptr_ring_buf1[2];

    ptr_ring_buf2[0] = ptr_ring_buf2[1];
    ptr_ring_buf2[1] = ptr_ring_buf2[2];
  }
}

VOID ia_enhaacplus_enc_iir_downsampler(ixheaace_iir21_resampler *pstr_down_sampler,
                                       FLOAT32 *ptr_in_samples, WORD32 num_in_samples,
                                       WORD32 in_stride, FLOAT32 *ptr_out_samples,
                                       WORD32 *num_out_samples, WORD32 out_stride,
                                       FLOAT32 *ptr_ring_buf1, FLOAT32 *ptr_ring_buf2,
                                       ixheaace_resampler_scratch *pstr_resampler_scratch) {
  ixheaace_iir_filter *pstr_iir_filter = &(pstr_down_sampler->iir_filter);
  WORD32 k;
  FLOAT32 *ptr_iir_ring1, *ptr_iir_ring2, *ptr_out_temp;

  ptr_iir_ring2 = pstr_iir_filter->ring_buf_2;
  ptr_iir_ring1 = pstr_iir_filter->ring_buf_1;
  ptr_out_temp = pstr_resampler_scratch->downsampler_out_buffer;

  ia_enhaacplus_enc_copy_ring_buffers(ptr_iir_ring1, ptr_iir_ring2, ptr_ring_buf1, ptr_ring_buf2);

  ia_enhaacplus_enc_downsample_iir_filter(
      pstr_down_sampler, ptr_in_samples, ptr_out_temp, in_stride, num_in_samples, num_out_samples,
      out_stride, &ptr_ring_buf1[LEN_RING_BUF - 1], &ptr_ring_buf2[LEN_RING_BUF - 1]);

  for (k = 0; k < *num_out_samples; k++) {
    ptr_out_samples[k * in_stride] = ptr_out_temp[k * out_stride];
  }

  ia_enhaacplus_enc_copy_ring_buffers(&ptr_ring_buf1[num_in_samples],
                                      &ptr_ring_buf2[num_in_samples], ptr_iir_ring1,
                                      ptr_iir_ring2);
}

VOID ia_enhaacplus_enc_iir_sos_downsampler(ixheaace_iir_sos_resampler *pstr_down_sampler,
                                           FLOAT32 *ptr_in_samples, WORD32 num_in_samples,
                                           WORD32 in_stride, FLOAT32 *ptr_out_samples,
                                           WORD32 *num_out_samples, FLOAT32 *ptr_ring_buf1,
                                           FLOAT32 *ptr_ring_buf2,
                                           ixheaace_resampler_scratch *pstr_resampler_scratch) {
  ixheaace_iir_sos_filter *pstr_iir_filter = &(pstr_down_sampler->iir_filter);
  FLOAT32 *ptr_iir_ring1, *ptr_iir_ring2;
  FLOAT32 *ptr_out_stage1, *ptr_out_stage2, *ptr_out_stage3, *ptr_out_stage4, *ptr_out_stage5,
      *ptr_out_final;
  WORD32 p = 0, idx = 0, offset1 = 0, offset2 = 0;
  FLOAT32 *ptr_temp_buf1, *ptr_temp_buf2, *ptr_temp_ring_buf;
  WORD32 upper_lim = num_in_samples * in_stride;

  ptr_iir_ring1 = pstr_iir_filter->ring_buf_sos_1;
  ptr_iir_ring2 = pstr_iir_filter->ring_buf_sos_2;

  ia_enhaacplus_enc_copy_ring_buffers_sos(ptr_iir_ring1, ptr_iir_ring2, ptr_ring_buf1,
                                          ptr_ring_buf2, LEN_RING_BUF_SOS_1, LEN_RING_BUF_SOS_2);

  ptr_temp_buf1 = pstr_resampler_scratch->scratch_buf1_temp;
  ptr_temp_buf2 = pstr_resampler_scratch->scratch_buf2_temp;
  ptr_temp_ring_buf = pstr_resampler_scratch->ring_buf_temp;

  ia_enhaacplus_enc_copy_ring_buffers_sos(ptr_ring_buf1, ptr_ring_buf2, ptr_temp_buf1,
                                          ptr_temp_buf2, LEN_RING_BUF_SOS_1, LEN_RING_BUF_SOS_1);

  ptr_out_stage1 = pstr_resampler_scratch->downsampler_out_buffer;

  ia_enhaacplus_enc_update_ring_buffer_sos(ptr_temp_ring_buf, ptr_in_samples, upper_lim,
                                           in_stride, idx);

  // Stage 1
  ia_enhaacplus_enc_iir_sos_filter(pstr_down_sampler, ptr_in_samples, in_stride, ptr_out_stage1,
                                   num_in_samples, ptr_temp_buf1, ptr_temp_buf2, idx);

  offset1 = LEN_RING_BUF_SOS_1 * idx;
  offset2 = LEN_RING_BUF_SOS_1 * (idx + 1);

  ia_enhaacplus_enc_copy_ring_buffers_sos(ptr_ring_buf2 + offset1, ptr_ring_buf2 + offset2,
                                          ptr_temp_buf1, ptr_temp_buf2, LEN_RING_BUF_SOS_1,
                                          LEN_RING_BUF_SOS_1);

  idx++;

  ia_enhaacplus_enc_update_ring_buffer_sos(ptr_temp_ring_buf, ptr_out_stage1, upper_lim,
                                           in_stride, idx);

  ptr_out_stage2 = pstr_resampler_scratch->downsampler_in_buffer;

  // Stage 2
  ia_enhaacplus_enc_iir_sos_filter(pstr_down_sampler, ptr_out_stage1, in_stride, ptr_out_stage2,
                                   num_in_samples, ptr_temp_buf1, ptr_temp_buf2, idx);

  offset1 = LEN_RING_BUF_SOS_1 * idx;
  offset2 = LEN_RING_BUF_SOS_1 * (idx + 1);

  ia_enhaacplus_enc_copy_ring_buffers_sos(ptr_ring_buf2 + offset1, ptr_ring_buf2 + offset2,
                                          ptr_temp_buf1, ptr_temp_buf2, LEN_RING_BUF_SOS_1,
                                          LEN_RING_BUF_SOS_1);

  idx++;

  ia_enhaacplus_enc_update_ring_buffer_sos(ptr_temp_ring_buf, ptr_out_stage2, upper_lim,
                                           in_stride, idx);

  ptr_out_stage3 = pstr_resampler_scratch->downsampler_out_buffer;

  // Stage 3
  ia_enhaacplus_enc_iir_sos_filter(pstr_down_sampler, ptr_out_stage2, in_stride, ptr_out_stage3,
                                   num_in_samples, ptr_temp_buf1, ptr_temp_buf2, idx);

  offset1 = LEN_RING_BUF_SOS_1 * idx;
  offset2 = LEN_RING_BUF_SOS_1 * (idx + 1);

  ia_enhaacplus_enc_copy_ring_buffers_sos(ptr_ring_buf2 + offset1, ptr_ring_buf2 + offset2,
                                          ptr_temp_buf1, ptr_temp_buf2, LEN_RING_BUF_SOS_1,
                                          LEN_RING_BUF_SOS_1);

  idx++;

  ia_enhaacplus_enc_update_ring_buffer_sos(ptr_temp_ring_buf, ptr_out_stage3, upper_lim,
                                           in_stride, idx);

  ptr_out_stage4 = pstr_resampler_scratch->downsampler_in_buffer;

  // Stage 4
  ia_enhaacplus_enc_iir_sos_filter(pstr_down_sampler, ptr_out_stage3, in_stride, ptr_out_stage4,
                                   num_in_samples, ptr_temp_buf1, ptr_temp_buf2, idx);

  offset1 = LEN_RING_BUF_SOS_1 * idx;
  offset2 = LEN_RING_BUF_SOS_1 * (idx + 1);

  ia_enhaacplus_enc_copy_ring_buffers_sos(ptr_ring_buf2 + offset1, ptr_ring_buf2 + offset2,
                                          ptr_temp_buf1, ptr_temp_buf2, LEN_RING_BUF_SOS_1,
                                          LEN_RING_BUF_SOS_1);

  idx++;

  ia_enhaacplus_enc_update_ring_buffer_sos(ptr_temp_ring_buf, ptr_out_stage4, upper_lim,
                                           in_stride, idx);

  ptr_out_stage5 = pstr_resampler_scratch->downsampler_out_buffer;

  // Stage 5

  ia_enhaacplus_enc_iir_sos_filter(pstr_down_sampler, ptr_out_stage4, in_stride, ptr_out_stage5,
                                   num_in_samples, ptr_temp_buf1, ptr_temp_buf2, idx);

  idx++;

  ia_enhaacplus_enc_update_ring_buffer_sos(ptr_temp_ring_buf, ptr_out_stage5, upper_lim,
                                           in_stride, idx);

  ptr_out_final = pstr_resampler_scratch->downsampler_in_buffer;

  // Multiply by gain and perform downsamplng
  *num_out_samples = 0;
  for (p = 0; p < num_in_samples * in_stride; p += in_stride) {
    ptr_out_final[p] = ptr_out_stage5[p] * pstr_down_sampler->iir_filter.gain_sos;

    pstr_down_sampler->pending++;

    if (pstr_down_sampler->pending == pstr_down_sampler->ratio) {
      ptr_out_samples[(*num_out_samples) * in_stride] = ptr_out_final[p];

      (*num_out_samples)++;

      pstr_down_sampler->pending = 0;
    }
  }

  // Update ring buffers
  ia_enhaacplus_enc_copy_ring_buffers_sos(ptr_temp_ring_buf,
                                          ptr_temp_ring_buf + LEN_RING_BUF_SOS_1, ptr_ring_buf1,
                                          ptr_ring_buf2, LEN_RING_BUF_SOS_1, LEN_RING_BUF_SOS_2);

  ia_enhaacplus_enc_copy_ring_buffers_sos(ptr_ring_buf1, ptr_ring_buf2, ptr_iir_ring1,
                                          ptr_iir_ring2, LEN_RING_BUF_SOS_1, LEN_RING_BUF_SOS_2);
}

VOID ia_enhaacplus_enc_iir_sos_upsampler(ixheaace_iir_sos_resampler *pstr_up_sampler,
                                         FLOAT32 *ptr_in_samples, WORD32 num_in_samples,
                                         WORD32 in_stride, FLOAT32 *ptr_out_samples,
                                         WORD32 *num_out_samples, FLOAT32 *ptr_ring_buf1,
                                         FLOAT32 *ptr_ring_buf2,
                                         ixheaace_resampler_scratch *pstr_resampler_scratch) {
  ixheaace_iir_sos_filter *pstr_iir_filter = &(pstr_up_sampler->iir_filter);
  FLOAT32 *ptr_iir_ring1, *ptr_iir_ring2;
  FLOAT32 *ptr_out_stage1, *ptr_out_stage2, *ptr_out_stage3, *ptr_out_stage4, *ptr_out_stage5;
  FLOAT32 out_val;
  FLOAT32 *ptr_temp_buf1, *ptr_temp_buf2, *ptr_temp_ring_buf;
  WORD32 p = 0, idx = 0, offset1 = 0, offset2 = 0;
  WORD32 upsample_fac = pstr_up_sampler->ratio;
  WORD32 upper_lim = num_in_samples * in_stride * upsample_fac;

  ptr_iir_ring2 = pstr_iir_filter->ring_buf_sos_2;
  ptr_iir_ring1 = pstr_iir_filter->ring_buf_sos_1;

  ia_enhaacplus_enc_copy_ring_buffers_sos(ptr_iir_ring1, ptr_iir_ring2, ptr_ring_buf1,
                                          ptr_ring_buf2, LEN_RING_BUF_SOS_1, LEN_RING_BUF_SOS_2);

  ptr_temp_buf1 = pstr_resampler_scratch->scratch_buf1_temp;
  ptr_temp_buf2 = pstr_resampler_scratch->scratch_buf2_temp;
  ptr_temp_ring_buf = pstr_resampler_scratch->ring_buf_temp;

  ia_enhaacplus_enc_copy_ring_buffers_sos(ptr_ring_buf1, ptr_ring_buf2, ptr_temp_buf1,
                                          ptr_temp_buf2, LEN_RING_BUF_SOS_1, LEN_RING_BUF_SOS_1);

  ptr_out_stage1 = pstr_resampler_scratch->downsampler_out_buffer;

  ia_enhaacplus_enc_update_ring_buffer_sos(ptr_temp_ring_buf, ptr_in_samples, upper_lim,
                                           in_stride, idx);

  // Stage 1
  ia_enhaacplus_enc_iir_sos_filter(pstr_up_sampler, ptr_in_samples, in_stride, ptr_out_stage1,
                                   num_in_samples * upsample_fac, ptr_temp_buf1, ptr_temp_buf2,
                                   idx);

  offset1 = LEN_RING_BUF_SOS_1 * idx;
  offset2 = LEN_RING_BUF_SOS_1 * (idx + 1);

  ia_enhaacplus_enc_copy_ring_buffers_sos(ptr_ring_buf2 + offset1, ptr_ring_buf2 + offset2,
                                          ptr_temp_buf1, ptr_temp_buf2, LEN_RING_BUF_SOS_1,
                                          LEN_RING_BUF_SOS_1);

  idx++;

  ia_enhaacplus_enc_update_ring_buffer_sos(ptr_temp_ring_buf, ptr_out_stage1, upper_lim,
                                           in_stride, idx);

  ptr_out_stage2 = pstr_resampler_scratch->downsampler_in_buffer;

  // Stage 2
  ia_enhaacplus_enc_iir_sos_filter(pstr_up_sampler, ptr_out_stage1, in_stride, ptr_out_stage2,
                                   num_in_samples * upsample_fac, ptr_temp_buf1, ptr_temp_buf2,
                                   idx);

  offset1 = LEN_RING_BUF_SOS_1 * idx;
  offset2 = LEN_RING_BUF_SOS_1 * (idx + 1);

  ia_enhaacplus_enc_copy_ring_buffers_sos(ptr_ring_buf2 + offset1, ptr_ring_buf2 + offset2,
                                          ptr_temp_buf1, ptr_temp_buf2, LEN_RING_BUF_SOS_1,
                                          LEN_RING_BUF_SOS_1);

  idx++;

  ia_enhaacplus_enc_update_ring_buffer_sos(ptr_temp_ring_buf, ptr_out_stage2, upper_lim,
                                           in_stride, idx);

  ptr_out_stage3 = pstr_resampler_scratch->downsampler_out_buffer;

  // Stage 3
  ia_enhaacplus_enc_iir_sos_filter(pstr_up_sampler, ptr_out_stage2, in_stride, ptr_out_stage3,
                                   num_in_samples * upsample_fac, ptr_temp_buf1, ptr_temp_buf2,
                                   idx);

  offset1 = LEN_RING_BUF_SOS_1 * idx;
  offset2 = LEN_RING_BUF_SOS_1 * (idx + 1);

  ia_enhaacplus_enc_copy_ring_buffers_sos(ptr_ring_buf2 + offset1, ptr_ring_buf2 + offset2,
                                          ptr_temp_buf1, ptr_temp_buf2, LEN_RING_BUF_SOS_1,
                                          LEN_RING_BUF_SOS_1);

  idx++;

  ia_enhaacplus_enc_update_ring_buffer_sos(ptr_temp_ring_buf, ptr_out_stage3, upper_lim,
                                           in_stride, idx);

  ptr_out_stage4 = pstr_resampler_scratch->downsampler_in_buffer;

  // Stage 4
  ia_enhaacplus_enc_iir_sos_filter(pstr_up_sampler, ptr_out_stage3, in_stride, ptr_out_stage4,
                                   num_in_samples * upsample_fac, ptr_temp_buf1, ptr_temp_buf2,
                                   idx);

  offset1 = LEN_RING_BUF_SOS_1 * idx;
  offset2 = LEN_RING_BUF_SOS_1 * (idx + 1);

  ia_enhaacplus_enc_copy_ring_buffers_sos(ptr_ring_buf2 + offset1, ptr_ring_buf2 + offset2,
                                          ptr_temp_buf1, ptr_temp_buf2, LEN_RING_BUF_SOS_1,
                                          LEN_RING_BUF_SOS_1);

  idx++;

  ia_enhaacplus_enc_update_ring_buffer_sos(ptr_temp_ring_buf, ptr_out_stage4, upper_lim,
                                           in_stride, idx);

  ptr_out_stage5 = pstr_resampler_scratch->downsampler_out_buffer;

  // Stage 5
  ia_enhaacplus_enc_iir_sos_filter(pstr_up_sampler, ptr_out_stage4, in_stride, ptr_out_stage5,
                                   num_in_samples * upsample_fac, ptr_temp_buf1, ptr_temp_buf2,
                                   idx);

  idx++;

  ia_enhaacplus_enc_update_ring_buffer_sos(ptr_temp_ring_buf, ptr_out_stage5, upper_lim,
                                           in_stride, idx);

  // Multiply by gain and perform downsamplng
  *num_out_samples = 0;
  for (p = 0; p < num_in_samples * in_stride * upsample_fac; p += in_stride) {
    out_val = ptr_out_stage5[p] * pstr_up_sampler->iir_filter.gain_sos;
    ptr_out_samples[p] = out_val;
    (*num_out_samples)++;
  }

  ia_enhaacplus_enc_copy_ring_buffers_sos(ptr_temp_ring_buf,
                                          ptr_temp_ring_buf + LEN_RING_BUF_SOS_1, ptr_ring_buf1,
                                          ptr_ring_buf2, LEN_RING_BUF_SOS_1, LEN_RING_BUF_SOS_2);

  ia_enhaacplus_enc_copy_ring_buffers_sos(ptr_ring_buf1, ptr_ring_buf2, ptr_iir_ring1,
                                          ptr_iir_ring2, LEN_RING_BUF_SOS_1, LEN_RING_BUF_SOS_2);
}

WORD32 ia_enhaacplus_enc_compute_resampling_ratio(WORD32 ccfl_idx) {
  WORD32 resamp_ratio;

  if (ccfl_idx == SBR_4_1) {
    resamp_ratio = 2;
  } else if (ccfl_idx == SBR_8_3) {
    resamp_ratio = 4;
  } else {
    resamp_ratio = 1;
  }

  return resamp_ratio;
}

VOID ixheaace_upsampling_inp_buf_generation(FLOAT32 *ptr_inp_buf, FLOAT32 *ptr_temp_buf,
                                            WORD32 num_samples, WORD32 upsamp_fac,
                                            WORD32 offset) {
  WORD32 idx, m = 0;
  FLOAT32 *ptr_in_samples;

  memset(ptr_temp_buf, 0,
         (num_samples * IXHEAACE_MAX_CH_IN_BS_ELE * upsamp_fac * sizeof(*ptr_temp_buf)));

  ptr_in_samples = ptr_inp_buf + offset;

  // Perform actual upsampling (repeat samples)
  for (idx = 0; idx < num_samples; idx++) {
    ptr_temp_buf[m++] = ptr_in_samples[idx * IXHEAACE_MAX_CH_IN_BS_ELE];
    ptr_temp_buf[m++] = ptr_in_samples[idx * IXHEAACE_MAX_CH_IN_BS_ELE + 1];
    ptr_temp_buf[m++] =
        ptr_in_samples[idx *
                       IXHEAACE_MAX_CH_IN_BS_ELE];  // 1st channel sample repeated for upsampling
    ptr_temp_buf[m++] = ptr_in_samples[idx * IXHEAACE_MAX_CH_IN_BS_ELE +
                                       1];  // 2nd channel sample repeated for upsampling
    ptr_temp_buf[m++] =
        ptr_in_samples[idx *
                       IXHEAACE_MAX_CH_IN_BS_ELE];  // 1st channel sample repeated for upsampling
    ptr_temp_buf[m++] = ptr_in_samples[idx * IXHEAACE_MAX_CH_IN_BS_ELE +
                                       1];  // 2nd channel sample repeated for upsampling
  }
}