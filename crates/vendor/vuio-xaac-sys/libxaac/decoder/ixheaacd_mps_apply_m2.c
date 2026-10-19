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
#include "ixheaacd_mps_get_index.h"

VOID ixheaacd_apply_m2(ia_heaac_mps_state_struct *pstr_mps_state) {
  WORD32 ts, qs, row, col;
  ia_heaac_mps_state_struct *curr_state = pstr_mps_state;
  ia_mps_persistent_mem *persistent_mem = &curr_state->mps_persistent_mem;
  ia_mps_dec_reuse_array_struct *p_array_struct = pstr_mps_state->array_struct;
  ia_mps_dec_m2_param_struct *p_m2_param = pstr_mps_state->aux_struct->m2_param;

  WORD32 num_direct_signals = curr_state->num_direct_signals;
  WORD32 temp_1, loop_counter, col_counter = num_direct_signals + curr_state->num_decor_signals;

  WORD32 *rout_ptr, *rout_kernel_ptr;

  WORD32 *hyb_output_real_dry, *hyb_output_imag_dry, *hyb_output_real_wet, *hyb_output_imag_wet;

  WORD32 *p_hyb_out_dry_real, *p_hyb_out_dry_imag, *p_hyb_out_dry_re, *p_hyb_out_dry_im;

  WORD32 *w_wet_real, *w_wet_imag, *w_dry_real, *w_dry_imag;

  WORD32 *m2_decor_real_prev = persistent_mem->m2_decor_real_prev;
  WORD32 *m2_decor_imag_prev = persistent_mem->m2_decor_imag_prev;

  WORD32 *p_buffer_real, *p_buffer_imag, *p_buffer_re, *p_buffer_im;
  WORD32 *p_buf_real, *p_buf_imag, *p_buf_re, *p_buf_im;

  WORD32 *m2_resid_real_prev = persistent_mem->m2_resid_real_prev;
  WORD32 *m2_resid_imag_prev = persistent_mem->m2_resid_imag_prev;

  WORD32 idx = 0;
  WORD32 w_wet_offset = num_direct_signals * TSXHB;

  WORD32 num_output_channels = curr_state->num_output_channels;
  WORD32 time_slots = curr_state->time_slots;
  WORD32 hybrid_bands = curr_state->hybrid_bands;
  WORD32 m2_param_imag_present = curr_state->m2_param_imag_present;
  WORD32 num_parameter_bands = curr_state->num_parameter_bands;
  WORD32 up_mix_type = curr_state->up_mix_type;
  WORD32 residual_coding = curr_state->residual_coding;
  WORD32 *index_ptr = curr_state->index;

  SIZE_T params[4];

  params[0] = (SIZE_T)(&curr_state->kernels[0]);
  params[1] = time_slots;
  params[2] = num_parameter_bands;
  params[3] = hybrid_bands;

  rout_ptr = pstr_mps_state->mps_scratch_mem_v;
  rout_kernel_ptr =
      rout_ptr + IXHEAAC_GET_SIZE_ALIGNED_TYPE(TSXHB, sizeof(*rout_kernel_ptr), BYTE_ALIGN_8);

  p_hyb_out_dry_real = p_array_struct->hyb_output_real_dry;
  p_hyb_out_dry_imag = p_array_struct->hyb_output_imag_dry;

  for (row = 0; row < num_output_channels; row++) {
    hyb_output_real_dry = p_hyb_out_dry_real;
    hyb_output_imag_dry = p_hyb_out_dry_imag;

    for (ts = 0; ts < time_slots; ts++) {
      memset(hyb_output_real_dry, 0, (hybrid_bands) * sizeof(hyb_output_real_dry[0]));
      memset(hyb_output_imag_dry, 0, (hybrid_bands) * sizeof(hyb_output_imag_dry[0]));

      hyb_output_real_dry += MAX_HYBRID_BANDS;
      hyb_output_imag_dry += MAX_HYBRID_BANDS;
    }

    p_hyb_out_dry_real += TSXHB;
    p_hyb_out_dry_imag += TSXHB;
  }

  if (residual_coding)
    loop_counter = col_counter;
  else
    loop_counter = num_direct_signals;

  idx = 0;

  p_hyb_out_dry_real = p_array_struct->hyb_output_real_dry;
  p_hyb_out_dry_imag = p_array_struct->hyb_output_imag_dry;

  for (row = 0; row < num_output_channels; row++) {
    p_buffer_real = p_array_struct->buf_real;
    p_buffer_imag = p_array_struct->buf_imag;

    for (col = 0; col < num_direct_signals; col++) {
      p_buffer_re = p_buffer_real;
      p_buffer_im = p_buffer_imag;

      if (curr_state->m2_param_present[row][col] & 2) {
        ixheaacd_dec_interp_umx(p_m2_param->m2_resid_real[idx++], rout_ptr, m2_resid_real_prev,
                                pstr_mps_state);
        ixheaacd_apply_abs_kernels(rout_ptr, rout_kernel_ptr, params);

        p_hyb_out_dry_re = p_hyb_out_dry_real;
        p_hyb_out_dry_im = p_hyb_out_dry_imag;

        for (ts = 0; ts < time_slots; ts++) {
          hyb_output_real_dry = p_hyb_out_dry_re;
          hyb_output_imag_dry = p_hyb_out_dry_im;

          w_dry_real = p_buffer_re;
          w_dry_imag = p_buffer_im;

          for (qs = 0; qs < hybrid_bands; qs++) {
            temp_1 = ixheaacd_mps_mult32_shr_15(*w_dry_real, *rout_kernel_ptr);
            w_dry_real++;
            *hyb_output_real_dry = ixheaac_add32_sat(*hyb_output_real_dry, temp_1);
            hyb_output_real_dry++;

            temp_1 = ixheaacd_mps_mult32_shr_15(*w_dry_imag, *rout_kernel_ptr);
            w_dry_imag++;
            rout_kernel_ptr++;
            *hyb_output_imag_dry = ixheaac_add32_sat(*hyb_output_imag_dry, temp_1);
            hyb_output_imag_dry++;
          }
          p_buffer_re += MAX_HYBRID_BANDS;
          p_buffer_im += MAX_HYBRID_BANDS;

          p_hyb_out_dry_re += MAX_HYBRID_BANDS;
          p_hyb_out_dry_im += MAX_HYBRID_BANDS;
        }
        m2_resid_real_prev += num_parameter_bands;
      }
      p_buffer_real += TSXHB;
      p_buffer_imag += TSXHB;
    }

    for (; col < loop_counter; col++) {
      WORD32 index;
      WORD32 res = ixheaacd_get_res_idx(pstr_mps_state, col);
      index = index_ptr[res];

      if (curr_state->m2_param_present[row][col] & 2) {
        WORD32 *p_dry_real = p_array_struct->w_dry_real + res * TSXHB;
        WORD32 *p_dry_imag = p_array_struct->w_dry_imag + res * TSXHB;

        ixheaacd_dec_interp_umx(p_m2_param->m2_resid_real[idx++], rout_ptr, m2_resid_real_prev,
                                pstr_mps_state);
        ixheaacd_apply_abs_kernels(rout_ptr, rout_kernel_ptr, params);

        p_hyb_out_dry_re = p_hyb_out_dry_real;
        p_hyb_out_dry_im = p_hyb_out_dry_imag;

        for (ts = 0; ts < time_slots; ts++) {
          hyb_output_real_dry = p_hyb_out_dry_re;
          hyb_output_imag_dry = p_hyb_out_dry_im;

          w_dry_real = p_dry_real;
          w_dry_imag = p_dry_imag;

          for (qs = 0; qs < index; qs++) {
            temp_1 = ixheaacd_mps_mult32_shr_15(*w_dry_real, *rout_kernel_ptr);
            w_dry_real++;
            *hyb_output_real_dry = ixheaac_add32_sat(*hyb_output_real_dry, temp_1);
            hyb_output_real_dry++;

            temp_1 = ixheaacd_mps_mult32_shr_15(*w_dry_imag, *rout_kernel_ptr);
            w_dry_imag++;
            rout_kernel_ptr++;
            *hyb_output_imag_dry = ixheaac_add32_sat(*hyb_output_imag_dry, temp_1);
            hyb_output_imag_dry++;
          }
          rout_kernel_ptr += hybrid_bands - index;

          p_hyb_out_dry_re += MAX_HYBRID_BANDS;
          p_hyb_out_dry_im += MAX_HYBRID_BANDS;

          p_dry_real += MAX_HYBRID_BANDS;
          p_dry_imag += MAX_HYBRID_BANDS;
        }
        m2_resid_real_prev += num_parameter_bands;
      }
    }

    p_hyb_out_dry_real += TSXHB;
    p_hyb_out_dry_imag += TSXHB;
  }

  if (up_mix_type == 2) {
    if (m2_param_imag_present) {
      if (residual_coding)
        loop_counter = col_counter;
      else
        loop_counter = num_direct_signals;

      idx = 0;

      p_hyb_out_dry_real = p_array_struct->hyb_output_real_dry;
      p_hyb_out_dry_imag = p_array_struct->hyb_output_imag_dry;

      for (row = 0; row < num_output_channels; row++) {
        p_buffer_real = p_array_struct->buf_real;
        p_buffer_imag = p_array_struct->buf_imag;

        for (col = 0; col < num_direct_signals; col++) {
          p_buffer_re = p_buffer_real;
          p_buffer_im = p_buffer_imag;

          if (curr_state->m2_param_present[row][col] & 2) {
            ixheaacd_dec_interp_umx(p_m2_param->m2_resid_imag[idx++], rout_ptr,
                                    m2_resid_imag_prev, pstr_mps_state);
            ixheaacd_apply_abs_kernels(rout_ptr, rout_kernel_ptr, params);

            p_hyb_out_dry_re = p_hyb_out_dry_real;
            p_hyb_out_dry_im = p_hyb_out_dry_imag;

            for (ts = 0; ts < time_slots; ts++) {
              hyb_output_real_dry = p_hyb_out_dry_re;
              hyb_output_imag_dry = p_hyb_out_dry_im;

              w_dry_real = p_buffer_re;
              w_dry_imag = p_buffer_im;

              for (qs = 0; qs < 2; qs++) {
                temp_1 = ixheaacd_mps_mult32_shr_15(*w_dry_imag, *rout_kernel_ptr);
                w_dry_imag++;
                *hyb_output_real_dry = ixheaac_add32_sat(*hyb_output_real_dry, temp_1);
                hyb_output_real_dry++;

                temp_1 = ixheaacd_mps_mult32_shr_15(*w_dry_real, *rout_kernel_ptr);
                w_dry_real++;
                rout_kernel_ptr++;
                *hyb_output_imag_dry = ixheaac_sub32_sat(*hyb_output_imag_dry, temp_1);
                hyb_output_imag_dry++;
              }

              for (; qs < hybrid_bands; qs++) {
                temp_1 = ixheaacd_mps_mult32_shr_15(*w_dry_imag, *rout_kernel_ptr);
                w_dry_imag++;
                *hyb_output_real_dry = ixheaac_sub32_sat(*hyb_output_real_dry, temp_1);
                hyb_output_real_dry++;

                temp_1 = ixheaacd_mps_mult32_shr_15(*w_dry_real, *rout_kernel_ptr);
                w_dry_real++;
                rout_kernel_ptr++;
                *hyb_output_imag_dry = ixheaac_add32_sat(*hyb_output_imag_dry, temp_1);
                hyb_output_imag_dry++;
              }
              p_buffer_re += MAX_HYBRID_BANDS;
              p_buffer_im += MAX_HYBRID_BANDS;

              p_hyb_out_dry_re += MAX_HYBRID_BANDS;
              p_hyb_out_dry_im += MAX_HYBRID_BANDS;
            }
            m2_resid_imag_prev += num_parameter_bands;
          }
          p_buffer_real += TSXHB;
          p_buffer_imag += TSXHB;
        }

        for (; col < loop_counter; col++) {
          WORD32 index;
          WORD32 res = ixheaacd_get_res_idx(pstr_mps_state, col);
          index = index_ptr[res];

          if (curr_state->m2_param_present[row][col] & 2) {
            WORD32 *p_dry_real = p_array_struct->w_dry_real + res * TSXHB;
            WORD32 *p_dry_imag = p_array_struct->w_dry_imag + res * TSXHB;
            ixheaacd_dec_interp_umx(p_m2_param->m2_resid_imag[idx++], rout_ptr,
                                    m2_resid_imag_prev, pstr_mps_state);
            ixheaacd_apply_abs_kernels(rout_ptr, rout_kernel_ptr, params);

            p_hyb_out_dry_re = p_hyb_out_dry_real;
            p_hyb_out_dry_im = p_hyb_out_dry_imag;

            for (ts = 0; ts < time_slots; ts++) {
              hyb_output_real_dry = p_hyb_out_dry_re;
              hyb_output_imag_dry = p_hyb_out_dry_im;

              w_dry_real = p_dry_real;
              w_dry_imag = p_dry_imag;

              for (qs = 0; qs < 2; qs++) {
                temp_1 = ixheaacd_mps_mult32_shr_15(*w_dry_imag, *rout_kernel_ptr);
                w_dry_imag++;
                *hyb_output_real_dry = ixheaac_add32_sat(*hyb_output_real_dry, temp_1);
                hyb_output_real_dry++;

                temp_1 = ixheaacd_mps_mult32_shr_15(*w_dry_real, *rout_kernel_ptr);
                w_dry_real++;
                rout_kernel_ptr++;
                *hyb_output_imag_dry = ixheaac_sub32_sat(*hyb_output_imag_dry, temp_1);
                hyb_output_imag_dry++;
              }

              for (; qs < index; qs++) {
                temp_1 = ixheaacd_mps_mult32_shr_15(*w_dry_imag, *rout_kernel_ptr);
                w_dry_imag++;
                *hyb_output_real_dry = ixheaac_sub32_sat(*hyb_output_real_dry, temp_1);
                hyb_output_real_dry++;

                temp_1 = ixheaacd_mps_mult32_shr_15(*w_dry_real, *rout_kernel_ptr);
                w_dry_real++;
                rout_kernel_ptr++;
                *hyb_output_imag_dry = ixheaac_add32_sat(*hyb_output_imag_dry, temp_1);
                hyb_output_imag_dry++;
              }
              rout_kernel_ptr += hybrid_bands - index;

              p_hyb_out_dry_re += MAX_HYBRID_BANDS;
              p_hyb_out_dry_im += MAX_HYBRID_BANDS;

              p_dry_real += MAX_HYBRID_BANDS;
              p_dry_imag += MAX_HYBRID_BANDS;
            }
            m2_resid_imag_prev += num_parameter_bands;
          }
        }
        p_hyb_out_dry_real += TSXHB;
        p_hyb_out_dry_imag += TSXHB;
      }
    }
  }
  p_buffer_real = p_array_struct->buf_real;
  p_buffer_imag = p_array_struct->buf_imag;

  for (row = 0; row < num_output_channels; row++) {
    hyb_output_real_wet = p_buffer_real;
    hyb_output_imag_wet = p_buffer_imag;

    for (ts = 0; ts < time_slots; ts++) {
      memset(hyb_output_real_wet, 0, (hybrid_bands) * sizeof(*hyb_output_real_wet));
      memset(hyb_output_imag_wet, 0, (hybrid_bands) * sizeof(*hyb_output_imag_wet));

      hyb_output_real_wet += MAX_HYBRID_BANDS;
      hyb_output_imag_wet += MAX_HYBRID_BANDS;
    }
    p_buffer_real += TSXHB;
    p_buffer_imag += TSXHB;
  }
  idx = 0;

  p_buffer_real = p_array_struct->buf_real;
  p_buffer_imag = p_array_struct->buf_imag;

  for (row = 0; row < num_output_channels; row++) {
    p_buf_real = p_array_struct->buffer_real + w_wet_offset;
    p_buf_imag = p_array_struct->buffer_imag + w_wet_offset;
    for (col = num_direct_signals; col < col_counter; col++) {
      if (curr_state->m2_param_present[row][col] & 1) {
        ixheaacd_dec_interp_umx(p_m2_param->m2_decor_real[idx++], rout_ptr, m2_decor_real_prev,
                                pstr_mps_state);

        ixheaacd_apply_abs_kernels(rout_ptr, rout_kernel_ptr, params);
        p_buffer_re = p_buffer_real;
        p_buffer_im = p_buffer_imag;

        p_buf_re = p_buf_real;
        p_buf_im = p_buf_imag;
        for (ts = 0; ts < time_slots; ts++) {
          hyb_output_real_wet = p_buffer_re;
          hyb_output_imag_wet = p_buffer_im;

          w_wet_real = p_buf_re;
          w_wet_imag = p_buf_im;

          for (qs = 0; qs < hybrid_bands; qs++) {
            temp_1 = ixheaacd_mps_mult32_shr_15(*w_wet_real, *rout_kernel_ptr);
            w_wet_real++;
            *hyb_output_real_wet = ixheaac_add32_sat(*hyb_output_real_wet, temp_1);
            hyb_output_real_wet++;

            temp_1 = ixheaacd_mps_mult32_shr_15(*w_wet_imag, *rout_kernel_ptr);
            w_wet_imag++;
            rout_kernel_ptr++;
            *hyb_output_imag_wet = ixheaac_add32_sat(*hyb_output_imag_wet, temp_1);
            hyb_output_imag_wet++;
          }
          p_buffer_re += MAX_HYBRID_BANDS;
          p_buffer_im += MAX_HYBRID_BANDS;

          p_buf_re += MAX_HYBRID_BANDS;
          p_buf_im += MAX_HYBRID_BANDS;
        }
        m2_decor_real_prev += num_parameter_bands;
      }
      p_buf_real += TSXHB;
      p_buf_imag += TSXHB;
    }
    p_buffer_real += TSXHB;
    p_buffer_imag += TSXHB;
  }

  if (up_mix_type == 2) {
    if (m2_param_imag_present) {
      idx = 0;

      p_buffer_real = p_array_struct->buf_real;
      p_buffer_imag = p_array_struct->buf_imag;

      for (row = 0; row < num_output_channels; row++) {
        m2_decor_imag_prev += num_parameter_bands * num_direct_signals;
        p_buf_real = p_array_struct->buffer_real + w_wet_offset;
        p_buf_imag = p_array_struct->buffer_imag + w_wet_offset;
        for (col = num_direct_signals; col < col_counter; col++) {
          if (curr_state->m2_param_present[row][col] & 1) {
            ixheaacd_dec_interp_umx(p_m2_param->m2_decor_imag[idx++], rout_ptr,
                                    m2_decor_imag_prev, pstr_mps_state);
            ixheaacd_apply_abs_kernels(rout_ptr, rout_kernel_ptr, params);

            p_buffer_re = p_buffer_real;
            p_buffer_im = p_buffer_imag;

            p_buf_re = p_buf_real;
            p_buf_im = p_buf_imag;
            for (ts = 0; ts < time_slots; ts++) {
              hyb_output_real_wet = p_buffer_re;
              hyb_output_imag_wet = p_buffer_im;

              w_wet_real = p_buf_re;
              w_wet_imag = p_buf_im;

              for (qs = 0; qs < 2; qs++) {
                temp_1 = ixheaacd_mps_mult32_shr_15(*w_wet_imag, *rout_kernel_ptr);
                w_wet_imag++;
                *hyb_output_real_wet = ixheaac_add32_sat(*hyb_output_real_wet, temp_1);
                hyb_output_real_wet++;

                temp_1 = ixheaacd_mps_mult32_shr_15(*w_wet_real, *rout_kernel_ptr);
                w_wet_real++;
                rout_kernel_ptr++;
                *hyb_output_imag_wet = ixheaac_sub32_sat(*hyb_output_imag_wet, temp_1);
                hyb_output_imag_wet++;
              }

              for (; qs < hybrid_bands; qs++) {
                temp_1 = ixheaacd_mps_mult32_shr_15(*w_wet_imag, *rout_kernel_ptr);
                w_wet_imag++;
                *hyb_output_real_wet = ixheaac_sub32_sat(*hyb_output_real_wet, temp_1);
                hyb_output_real_wet++;

                temp_1 = ixheaacd_mps_mult32_shr_15(*w_wet_real, *rout_kernel_ptr);
                w_wet_real++;
                rout_kernel_ptr++;
                *hyb_output_imag_wet = ixheaac_add32_sat(*hyb_output_imag_wet, temp_1);
                hyb_output_imag_wet++;
              }
              p_buffer_re += MAX_HYBRID_BANDS;
              p_buffer_im += MAX_HYBRID_BANDS;

              p_buf_re += MAX_HYBRID_BANDS;
              p_buf_im += MAX_HYBRID_BANDS;
            }
            m2_decor_imag_prev += num_parameter_bands;
          }
          p_buf_real += TSXHB;
          p_buf_imag += TSXHB;
        }
        p_buffer_real += TSXHB;
        p_buffer_imag += TSXHB;
      }
    }
  }
  return;
}
