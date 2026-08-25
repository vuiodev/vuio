/******************************************************************************
 *
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
#include "ixheaacd_mps_struct_def.h"
#include "ixheaacd_mps_res_rom.h"
#include "ixheaacd_mps_aac_struct.h"
#include "ixheaac_constants.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaacd_bitbuffer.h"
#include "ixheaacd_common_rom.h"
#include "ixheaacd_sbrdecsettings.h"
#include "ixheaacd_sbr_scale.h"
#include "ixheaacd_env_extr_part.h"
#include "ixheaacd_sbr_rom.h"
#include "ixheaacd_hybrid.h"
#include "ixheaacd_ps_dec.h"
#include "ixheaacd_mps_polyphase.h"
#include "ixheaacd_config.h"
#include "ixheaacd_qmf_dec.h"
#include "ixheaacd_mps_dec.h"
#include "ixheaacd_mps_macro_def.h"
#include "ixheaacd_mps_apply_common.h"
#include "ixheaacd_mps_basic_op.h"

VOID ixheaacd_mps_apply_m1(ia_heaac_mps_state_struct *pstr_mps_state) {
  ia_mps_dec_reuse_array_struct *p_array_struct = pstr_mps_state->array_struct;
  ia_mps_dec_m1_param_struct *m1_param = p_array_struct->m1_param;
  WORD32 ts, qs, row, col;
  WORD32 temp_1, temp_2;
  WORD32 *rout_real_ptr, *rout_imag_ptr, *rout_kernel_real_ptr, *rout_kernel_imag_ptr;
  WORD32 *v_real, *v_imag, *x_real, *x_imag;
  WORD32 *p_buffer_real, *p_buffer_imag, *p_v_real, *p_v_imag;

  ia_heaac_mps_state_struct *curr_state = pstr_mps_state;
  WORD32 *m1_param_real_prev = curr_state->mps_persistent_mem.m1_param_real_prev;
  WORD32 *m1_param_imag_prev = curr_state->mps_persistent_mem.m1_param_imag_prev;
  WORD32 imag_present = curr_state->m1_param_imag_present;
  WORD32 v_channels = curr_state->num_v_channels;
  WORD32 x_channels = curr_state->num_x_channels;
  WORD32 hybrid_bands = curr_state->hybrid_bands;
  WORD32 num_parameter_bands = curr_state->num_parameter_bands;
  WORD32 *index = curr_state->index;

  WORD32 *p_x_re, *p_x_im;

  WORD32 hyb_bands = hybrid_bands;
  WORD32 time_slots = curr_state->time_slots;
  SIZE_T params[4];

  params[0] = (SIZE_T)(&curr_state->kernels[0]);
  params[1] = time_slots;
  params[2] = num_parameter_bands;
  params[3] = hybrid_bands;

  rout_real_ptr = pstr_mps_state->mps_scratch_mem_v;
  rout_kernel_real_ptr = rout_real_ptr + IXHEAAC_GET_SIZE_ALIGNED_TYPE(
                                             TSXHB, sizeof(*rout_kernel_real_ptr), BYTE_ALIGN_8);
  rout_imag_ptr = rout_kernel_real_ptr +
                  IXHEAAC_GET_SIZE_ALIGNED_TYPE(TSXHB, sizeof(*rout_imag_ptr), BYTE_ALIGN_8);
  rout_kernel_imag_ptr = rout_imag_ptr + IXHEAAC_GET_SIZE_ALIGNED_TYPE(
                                             TSXHB, sizeof(*rout_kernel_imag_ptr), BYTE_ALIGN_8);

  p_buffer_real = p_array_struct->buf_real;
  p_buffer_imag = p_array_struct->buf_imag;

  for (row = 0; row < v_channels; row++) {
    v_real = p_buffer_real;
    v_imag = p_buffer_imag;

    for (ts = 0; ts < time_slots; ts++) {
      memset(v_real, 0, hybrid_bands * sizeof(v_real[0]));
      memset(v_imag, 0, hybrid_bands * sizeof(v_imag[0]));

      v_real += MAX_HYBRID_BANDS;
      v_imag += MAX_HYBRID_BANDS;
    }
    p_buffer_real += TSXHB;
    p_buffer_imag += TSXHB;
  }

  p_buffer_real = p_array_struct->buf_real;
  p_buffer_imag = p_array_struct->buf_imag;

  if (!imag_present) {
    for (row = 0; row < v_channels; row++) {
      WORD32 *p_x_real = p_array_struct->x_real;
      WORD32 *p_x_imag = p_array_struct->x_imag;

      for (col = 0; col < x_channels; col++) {
        if (pstr_mps_state->m1_param_present[row][col]) {
          WORD32 idx = index[col];

          ixheaacd_dec_interp_umx(m1_param->m1_param_real[row][col], rout_real_ptr,
                                  m1_param_real_prev, pstr_mps_state);
          ixheaacd_apply_abs_kernels(rout_real_ptr, rout_kernel_real_ptr, params);

          p_v_real = p_buffer_real;
          p_v_imag = p_buffer_imag;

          p_x_re = p_x_real;
          p_x_im = p_x_imag;

          for (ts = 0; ts < time_slots; ts++) {
            v_real = p_v_real;
            v_imag = p_v_imag;

            x_real = p_x_re;
            x_imag = p_x_im;

            for (qs = 0; qs < idx; qs++) {
              temp_1 = ixheaacd_mps_mult32_shr_15(*x_real, *rout_kernel_real_ptr);
              *v_real = ixheaac_add32_sat(*v_real, temp_1);
              v_real++;

              temp_1 = ixheaacd_mps_mult32_shr_15(*x_imag, *rout_kernel_real_ptr);
              rout_kernel_real_ptr++;
              *v_imag = ixheaac_add32_sat(*v_imag, temp_1);
              v_imag++;

              x_real++;
              x_imag++;
            }
            rout_kernel_real_ptr += hyb_bands - idx;

            p_v_real += MAX_HYBRID_BANDS;
            p_v_imag += MAX_HYBRID_BANDS;

            p_x_re += MAX_HYBRID_BANDS;
            p_x_im += MAX_HYBRID_BANDS;
          }

          m1_param_real_prev += num_parameter_bands;

          p_x_real += TSXHB;
          p_x_imag += TSXHB;
        } else {
          m1_param_real_prev += num_parameter_bands;

          p_x_real += TSXHB;
          p_x_imag += TSXHB;
        }
      }
      p_buffer_real += TSXHB;
      p_buffer_imag += TSXHB;
    }
  } else {
    for (row = 0; row < v_channels; row++) {
      WORD32 *p_x_real = p_array_struct->x_real;
      WORD32 *p_x_imag = p_array_struct->x_imag;

      for (col = 0; col < x_channels; col++) {
        if (pstr_mps_state->m1_param_present[row][col]) {
          WORD32 idx = index[col];

          ixheaacd_dec_interp_umx(m1_param->m1_param_real[row][col], rout_real_ptr,
                                  m1_param_real_prev, pstr_mps_state);
          ixheaacd_dec_interp_umx(m1_param->m1_param_imag[row][col], rout_imag_ptr,
                                  m1_param_imag_prev, pstr_mps_state);
          ixheaacd_apply_abs_kernels(rout_real_ptr, rout_kernel_real_ptr, params);
          ixheaacd_apply_abs_kernels(rout_imag_ptr, rout_kernel_imag_ptr, params);

          p_v_real = p_buffer_real;
          p_v_imag = p_buffer_imag;

          p_x_re = p_x_real;
          p_x_im = p_x_imag;

          for (ts = 0; ts < time_slots; ts++) {
            v_real = p_v_real;
            v_imag = p_v_imag;

            x_real = p_x_re;
            x_imag = p_x_im;
            for (qs = 0; qs < 2; qs++) {
              temp_1 = ixheaacd_mps_mult32_shr_15(*x_real, *rout_kernel_real_ptr);
              temp_2 = ixheaacd_mps_mult32_shr_15(*x_imag, *rout_kernel_imag_ptr);
              temp_1 = ixheaac_add32_sat(temp_1, temp_2);

              *v_real = ixheaac_add32_sat(*v_real, temp_1);
              v_real++;

              temp_1 = ixheaacd_mps_mult32_shr_15(*x_imag, *rout_kernel_real_ptr);
              rout_kernel_real_ptr++;
              temp_2 = ixheaacd_mps_mult32_shr_15(*x_real, *rout_kernel_imag_ptr);
              rout_kernel_imag_ptr++;
              temp_1 = ixheaac_sub32_sat(temp_1, temp_2);

              *v_imag = ixheaac_add32_sat(*v_imag, temp_1);
              v_imag++;

              x_real++;
              x_imag++;
            }
            for (; qs < idx; qs++) {
              temp_1 = ixheaacd_mps_mult32_shr_15(*x_real, *rout_kernel_real_ptr);
              temp_2 = ixheaacd_mps_mult32_shr_15(*x_imag, *rout_kernel_imag_ptr);
              temp_1 = ixheaac_sub32_sat(temp_1, temp_2);

              *v_real = ixheaac_add32_sat(*v_real, temp_1);
              v_real++;

              temp_1 = ixheaacd_mps_mult32_shr_15(*x_imag, *rout_kernel_real_ptr);
              rout_kernel_real_ptr++;
              temp_2 = ixheaacd_mps_mult32_shr_15(*x_real, *rout_kernel_imag_ptr);
              rout_kernel_imag_ptr++;
              temp_1 = ixheaac_add32_sat(temp_1, temp_2);

              *v_imag = ixheaac_add32_sat(*v_imag, temp_1);
              v_imag++;

              x_real++;
              x_imag++;
            }
            rout_kernel_real_ptr += hyb_bands - idx;
            rout_kernel_imag_ptr += hyb_bands - idx;

            p_v_real += MAX_HYBRID_BANDS;
            p_v_imag += MAX_HYBRID_BANDS;

            p_x_re += MAX_HYBRID_BANDS;
            p_x_im += MAX_HYBRID_BANDS;
          }

          m1_param_real_prev += num_parameter_bands;
          m1_param_imag_prev += num_parameter_bands;

          p_x_real += TSXHB;
          p_x_imag += TSXHB;
        } else {
          m1_param_real_prev += num_parameter_bands;
          m1_param_imag_prev += num_parameter_bands;

          p_x_real += TSXHB;
          p_x_imag += TSXHB;
        }
      }
      p_buffer_real += TSXHB;
      p_buffer_imag += TSXHB;
    }
  }
  return;
}
