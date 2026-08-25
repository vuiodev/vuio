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
#include "ixheaac_type_def.h"
#include "ixheaacd_bitbuffer.h"
#include "ixheaacd_common_rom.h"
#include "ixheaacd_sbrdecsettings.h"
#include "ixheaacd_sbr_scale.h"
#include "ixheaacd_env_extr_part.h"
#include "ixheaacd_sbr_rom.h"
#include "ixheaacd_hybrid.h"
#include "ixheaacd_ps_dec.h"
#include "ixheaacd_config.h"
#include "ixheaacd_qmf_dec.h"
#include "ixheaacd_mps_polyphase.h"
#include "ixheaacd_mps_struct_def.h"
#include "ixheaacd_mps_res_rom.h"
#include "ixheaacd_mps_aac_struct.h"
#include "ixheaac_constants.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaacd_mps_dec.h"
#include "ixheaac_error_standards.h"
#include "ixheaacd_error_codes.h"
#include "ixheaacd_mps_macro_def.h"
#include "ixheaacd_mps_smoothing.h"
#include "ixheaacd_mps_tonality.h"
#ifndef MULT
#define MULT(a, b) (a * b)
#endif
#define ONE_BY_128_IN_Q30 (8388608)
#define ONE_IN_Q30 (1073741824)
#define PI_IN_Q27 (421657440)
#define FIFTY_X_PI_BY_180_Q27 (117127067)
#define TWENTY_FIVE_X_PI_BY_180_Q27 (58563533)
#define Q28_VALUE (1 << 28)
#define Q28_FLOAT_VAL ((FLOAT32)(1 << 28))
#define ONE_BY_Q28_FLOAT_VAL (1.0f / Q28_FLOAT_VAL)

VOID ixheaacd_mps_pre_matrix_mix_matrix_smoothing(
    ia_mps_dec_state_struct *self) {
  WORD32 smooth_band;
  FLOAT32 delta, one_minus_delta;
  WORD32 ps = 0, pb, row, col;
  WORD32 res_bands = 0;
  WORD32 *p_smoothing_data;

  if (self->residual_coding) res_bands = self->max_res_bands;

  p_smoothing_data = &self->smoothing_data[ps][res_bands];

  delta = self->param_slot_diff[ps] * self->inv_smoothing_time[ps];
  one_minus_delta = 1.0f - delta;

  for (pb = res_bands; pb < self->bs_param_bands; pb++) {
    smooth_band = *p_smoothing_data++;
    if (smooth_band) {
      for (row = 0; row < MAX_M_OUTPUT; row++) {
        for (col = 0; col < MAX_M_INPUT; col++) {
          self->m1_param_re[ps][pb][row][col] =
              (MULT(delta, self->m1_param_re[ps][pb][row][col]) +
               MULT(one_minus_delta, self->m1_param_re_prev[pb][row][col]));
          self->m1_param_im[ps][pb][row][col] =
              (MULT(delta, self->m1_param_im[ps][pb][row][col]) +
               MULT(one_minus_delta, self->m1_param_im_prev[pb][row][col]));
          self->m2_decor_re[ps][pb][row][col] =
              (MULT(delta, self->m2_decor_re[ps][pb][row][col]) +
               MULT(one_minus_delta, self->m2_decor_re_prev[pb][row][col]));
          self->m2_decor_im[ps][pb][row][col] =
              (MULT(delta, self->m2_decor_im[ps][pb][row][col]) +
               MULT(one_minus_delta, self->m2_decor_im_prev[pb][row][col]));
          self->m2_resid_re[ps][pb][row][col] =
              (MULT(delta, self->m2_resid_re[ps][pb][row][col]) +
               MULT(one_minus_delta, self->m2_resid_re_prev[pb][row][col]));
          self->m2_resid_im[ps][pb][row][col] =
              (MULT(delta, self->m2_resid_im[ps][pb][row][col]) +
               MULT(one_minus_delta, self->m2_resid_im_prev[pb][row][col]));
        }
      }
      self->pre_mix_req++;
    }
  }

  for (ps = 1; ps < self->num_parameter_sets; ps++) {
    delta = self->param_slot_diff[ps] * self->inv_smoothing_time[ps];
    one_minus_delta = 1.0f - delta;

    p_smoothing_data = &self->smoothing_data[ps][res_bands];

    for (pb = res_bands; pb < self->bs_param_bands; pb++) {
      smooth_band = *p_smoothing_data++;
      if (smooth_band) {
        for (row = 0; row < MAX_M_OUTPUT; row++) {
          for (col = 0; col < MAX_M_INPUT; col++) {
            self->m1_param_re[ps][pb][row][col] =
                (MULT(delta, self->m1_param_re[ps][pb][row][col]) +
                 MULT(one_minus_delta,
                      self->m1_param_re[ps - 1][pb][row][col]));
            self->m1_param_im[ps][pb][row][col] =
                (MULT(delta, self->m1_param_im[ps][pb][row][col]) +
                 MULT(one_minus_delta,
                      self->m1_param_im[ps - 1][pb][row][col]));
            self->m2_resid_re[ps][pb][row][col] =
                (MULT(delta, self->m2_resid_re[ps][pb][row][col]) +
                 MULT(one_minus_delta,
                      self->m2_resid_re[ps - 1][pb][row][col]));
            self->m2_decor_re[ps][pb][row][col] =
                (MULT(delta, self->m2_decor_re[ps][pb][row][col]) +
                 MULT(one_minus_delta,
                      self->m2_decor_re[ps - 1][pb][row][col]));
            self->m2_decor_im[ps][pb][row][col] =
                (MULT(delta, self->m2_decor_im[ps][pb][row][col]) +
                 MULT(one_minus_delta,
                      self->m2_decor_im[ps - 1][pb][row][col]));
            self->m2_resid_im[ps][pb][row][col] =
                (MULT(delta, self->m2_resid_im[ps][pb][row][col]) +
                 MULT(one_minus_delta,
                      self->m2_resid_im[ps - 1][pb][row][col]));
          }
        }
        self->pre_mix_req++;
      }
    }
  }
}

VOID ixheaacd_mps_smoothing_opd(ia_mps_dec_state_struct *self) {
  WORD32 ps, pb;
  WORD32 delta, one_minus_delta;

  if (self->opd_smoothing_mode == 0) {
    for (pb = 0; pb < self->bs_param_bands; pb++) {
      self->opd_smooth.smooth_l_phase[pb] =
          ((WORD32)(self->phase_l[self->num_parameter_sets - 1][pb] *
                    Q28_VALUE)) >>
          1;
      self->opd_smooth.smooth_r_phase[pb] =
          ((WORD32)(self->phase_r[self->num_parameter_sets - 1][pb] *
                    Q28_VALUE)) >>
          1;
    }
    return;
  }
  for (ps = 0; ps < self->num_parameter_sets; ps++) {
    WORD32 thr = self->bs_frame.ipd_data.bs_quant_coarse_xxx[ps]
                  ? FIFTY_X_PI_BY_180_Q27
                  : TWENTY_FIVE_X_PI_BY_180_Q27;

    delta = self->param_slot_diff[ps] * ONE_BY_128_IN_Q30;
    one_minus_delta = ONE_IN_Q30 - delta;

    for (pb = 0; pb < self->bs_param_bands; pb++) {
      WORD32 ltemp, rtemp, tmp;
      ltemp = ((WORD32)(self->phase_l[ps][pb] * Q28_FLOAT_VAL)) >> 1;
      rtemp = ((WORD32)(self->phase_r[ps][pb] * Q28_FLOAT_VAL)) >> 1;

      while (ltemp > self->opd_smooth.smooth_l_phase[pb] + PI_IN_Q27)
        ltemp -= 2 * PI_IN_Q27;
      while (ltemp < self->opd_smooth.smooth_l_phase[pb] - PI_IN_Q27)
        ltemp += 2 * PI_IN_Q27;
      while (rtemp > self->opd_smooth.smooth_r_phase[pb] + PI_IN_Q27)
        rtemp -= 2 * PI_IN_Q27;
      while (rtemp < self->opd_smooth.smooth_r_phase[pb] - PI_IN_Q27)
        rtemp += 2 * PI_IN_Q27;

      self->opd_smooth.smooth_l_phase[pb] =
          (ixheaac_mult32_shl(delta, ltemp) +
           ixheaac_mult32_shl(one_minus_delta,
                               self->opd_smooth.smooth_l_phase[pb]))
          << 1;
      self->opd_smooth.smooth_r_phase[pb] =
          (ixheaac_mult32_shl(delta, rtemp) +
           ixheaac_mult32_shl(one_minus_delta,
                               self->opd_smooth.smooth_r_phase[pb]))
          << 1;

      tmp = (ltemp - rtemp) - (self->opd_smooth.smooth_l_phase[pb] -
                               self->opd_smooth.smooth_r_phase[pb]);
      while (tmp > PI_IN_Q27) tmp -= 2 * PI_IN_Q27;
      while (tmp < -PI_IN_Q27) tmp += 2 * PI_IN_Q27;

      if (ixheaac_abs32(tmp) > thr) {
        self->opd_smooth.smooth_l_phase[pb] = ltemp;
        self->opd_smooth.smooth_r_phase[pb] = rtemp;
      }

      while (self->opd_smooth.smooth_l_phase[pb] > 2 * PI_IN_Q27)
        self->opd_smooth.smooth_l_phase[pb] -= 2 * PI_IN_Q27;
      while (self->opd_smooth.smooth_l_phase[pb] < 0)
        self->opd_smooth.smooth_l_phase[pb] += 2 * PI_IN_Q27;
      while (self->opd_smooth.smooth_r_phase[pb] > 2 * PI_IN_Q27)
        self->opd_smooth.smooth_r_phase[pb] -= 2 * PI_IN_Q27;
      while (self->opd_smooth.smooth_r_phase[pb] < 0)
        self->opd_smooth.smooth_r_phase[pb] += 2 * PI_IN_Q27;

      self->phase_l[ps][pb] =
          (self->opd_smooth.smooth_l_phase[pb] << 1) * ONE_BY_Q28_FLOAT_VAL;
      self->phase_r[ps][pb] =
          (self->opd_smooth.smooth_r_phase[pb] << 1) * ONE_BY_Q28_FLOAT_VAL;
    }
  }
}

static VOID ixheaacd_calc_filter_coeff(
    ia_heaac_mps_state_struct *pstr_mps_state, WORD32 ps, WORD32 *delta) {
  WORD32 d_slots;
  WORD32 *param_slot = pstr_mps_state->aux_struct->param_slot;
  WORD32 *smg_time = pstr_mps_state->aux_struct->smg_time;

  if (ps == 0)
    d_slots = param_slot[ps] + 1;
  else
    d_slots = param_slot[ps] - param_slot[ps - 1];

  if (pstr_mps_state->smooth_control) {
    switch (smg_time[ps]) {
      case SMG_TIME_64:
        *delta = d_slots << 9;
        break;
      case SMG_TIME_128:
        *delta = d_slots << 8;
        break;
      case SMG_TIME_256:
        *delta = d_slots << 7;
        break;
      case SMG_TIME_512:
        *delta = d_slots << 6;
        break;
      default:
        break;
    }
  } else {
    *delta = d_slots << 7;
  }

  return;
}

VOID ixheaacd_smooth_m1m2(ia_heaac_mps_state_struct *pstr_mps_state) {
  ia_heaac_mps_state_struct *curr_state = pstr_mps_state;
  ia_mps_persistent_mem *persistent_mem = &curr_state->mps_persistent_mem;
  ia_mps_dec_auxilary_struct *p_aux_struct = pstr_mps_state->aux_struct;
  ia_mps_dec_m2_param_struct *m2_param = p_aux_struct->m2_param;
  ia_mps_dec_m1_param_struct *m1_param = pstr_mps_state->array_struct->m1_param;
  WORD32 *m1_param_real_prev = persistent_mem->m1_param_real_prev;
  WORD32 *m2_decor_real_prev = persistent_mem->m2_decor_real_prev;
  WORD32 *m2_resid_real_prev = persistent_mem->m2_resid_real_prev;

  WORD32 num_parameter_bands = curr_state->num_parameter_bands;
  WORD32 num_direct_signals = curr_state->num_direct_signals;
  WORD32 num_decor_signals = curr_state->num_decor_signals;
  WORD32 m1_param_imag_present = curr_state->m1_param_imag_present;
  WORD32 m2_param_imag_present = curr_state->m2_param_imag_present;
  WORD32 col_counter = num_direct_signals + num_decor_signals;
  WORD32 num_parameter_sets = curr_state->num_parameter_sets;
  WORD32 num_output_channels = curr_state->num_output_channels;
  WORD32 num_v_channels = curr_state->num_v_channels;
  WORD32 num_x_channels = curr_state->num_x_channels;
  WORD32 smooth_control = curr_state->smooth_control;
  WORD32 smooth_config = curr_state->smooth_config;
  WORD32 resid_col_counter;
  WORD32 smooth_band_arr[MAX_PARAMETER_SETS][MAX_PARAMETER_BANDS];

  WORD32 *delta, *one_minus_delta, *delta_ptr, *one_minus_delta_ptr;
  WORD32 *param_r, *param_i, *param_prev_r, *param_prev_i;

  WORD32 *ton;
  WORD32 i, ps, pb, row, col;
  WORD32 res_bands = 0;
  WORD32 idx = 0;

  WORD32 *m2_decor_imag_prev = persistent_mem->m2_decor_imag_prev;
  WORD32 *m2_resid_imag_prev = persistent_mem->m2_resid_imag_prev;
  WORD32 *m1_param_imag_prev = persistent_mem->m1_param_imag_prev;

  ton = pstr_mps_state->mps_scratch_mem_v;
  delta = delta_ptr =
      ton + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS, sizeof(*delta), BYTE_ALIGN_8);
  one_minus_delta = one_minus_delta_ptr =
      delta +
      IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_SETS, sizeof(*one_minus_delta), BYTE_ALIGN_8);

  param_r = curr_state->res_bands;
  if (curr_state->residual_coding) {
    for (i = 0; i < MAX_RESIDUAL_CHANNELS_MPS; i++) {
      if (param_r[i] > res_bands) {
        res_bands = param_r[i];
      }
    }
  }

  if (curr_state->arbitrary_downmix == 2) {
    if (res_bands < curr_state->arbdmx_residual_bands) {
      res_bands = curr_state->arbdmx_residual_bands;
    }
  }

  if (smooth_config) {
    ixheaacd_measure_tonality(pstr_mps_state, ton);
  }

  for (ps = 0; ps < num_parameter_sets; ps++) {
    ixheaacd_calc_filter_coeff(pstr_mps_state, ps, delta);
    *one_minus_delta++ = (1 << 15) - *delta++;
  }

  if (smooth_control) {
    for (ps = 0; ps < num_parameter_sets; ps++) {
      if (ps < 8) {
        for (pb = 0; pb < num_parameter_bands; pb++) {
          smooth_band_arr[ps][pb] = pstr_mps_state->aux_struct->smg_data[ps][pb];
        }
      }
    }
  } else if (smooth_config) {
    for (ps = 0; ps < num_parameter_sets; ps++) {
      for (pb = 0; pb < num_parameter_bands; pb++) {
        smooth_band_arr[ps][pb] = (ton[pb] > POINT_EIGHT_Q15);
      }
    }
  }

  if (!(smooth_control == 0 && smooth_config == 0)) {
    if (m1_param_imag_present) {
      WORD32 *ptr_r1 = &m1_param->m1_param_real[0][0][0][0];
      WORD32 *ptr_i1 = &m1_param->m1_param_imag[0][0][0][0];
      for (row = 0; row < num_v_channels; row++) {
        WORD32 *ptr_r2 = ptr_r1;
        WORD32 *ptr_i2 = ptr_i1;
        for (col = 0; col < num_x_channels; col++) {
          param_r = ptr_r2;
          param_i = ptr_i2;
          m1_param_real_prev += res_bands;
          m1_param_imag_prev += res_bands;

          for (pb = res_bands; pb < num_parameter_bands; pb++) {
            if (smooth_band_arr[0][pb]) {
              WORD64 acc;

              acc = (WORD64)((WORD64)param_r[pb] * (WORD64)(*delta_ptr) +
                             (WORD64)(*m1_param_real_prev) *
                                 (WORD64)(*one_minus_delta_ptr));

              acc >>= 15;

              param_r[pb] = (WORD32)acc;

              acc = (WORD64)((WORD64)param_i[pb] * (WORD64)(*delta_ptr) +
                             (WORD64)(*m1_param_imag_prev) *
                                 (WORD64)(*one_minus_delta_ptr));

              acc >>= 15;

              param_i[pb] = (WORD32)acc;
            }
            m1_param_real_prev++;
            m1_param_imag_prev++;
          }
          param_r += MAX_PARAMETER_BANDS;
          param_i += MAX_PARAMETER_BANDS;

          for (ps = 1; ps < num_parameter_sets; ps++) {
            WORD32 del = delta_ptr[ps];
            WORD32 one_minus_del = one_minus_delta_ptr[ps];

            param_prev_r = param_r - MAX_PARAMETER_BANDS;
            param_prev_i = param_i - MAX_PARAMETER_BANDS;

            for (pb = res_bands; pb < num_parameter_bands; pb++) {
              if (smooth_band_arr[ps][pb]) {
                WORD64 acc;

                acc = (WORD64)((WORD64)param_r[pb] * (WORD64)(del) +
                               (WORD64)param_prev_r[pb] *
                                   (WORD64)(one_minus_del));

                acc >>= 15;

                param_r[pb] = (WORD32)acc;

                acc = (WORD64)((WORD64)param_i[pb] * (WORD64)(del) +
                               (WORD64)param_prev_i[pb] *
                                   (WORD64)(one_minus_del));

                acc >>= 15;

                param_i[pb] = (WORD32)acc;
              }
            }
            param_r += MAX_PARAMETER_BANDS;
            param_i += MAX_PARAMETER_BANDS;
          }
          ptr_r2 += PBXPS;
          ptr_i2 += PBXPS;
        }
        ptr_r1 += INCHXPBXPS;
        ptr_i1 += INCHXPBXPS;
      }
    } else {
      WORD32 *ptr1 = (WORD32 *)m1_param;

      for (row = 0; row < num_v_channels; row++) {
        WORD32 *ptr2 = ptr1;

        for (col = 0; col < num_x_channels; col++) {
          WORD32 *param_r = ptr2;

          WORD32 del = delta_ptr[0];
          WORD32 one_minus_del = one_minus_delta_ptr[0];

          m1_param_real_prev += res_bands;

          for (pb = res_bands; pb < num_parameter_bands; pb++) {
            if (smooth_band_arr[0][pb]) {
              WORD64 acc;

              acc = (WORD64)((WORD64)(param_r[pb]) * (WORD64)(del)) +
                    (WORD64)((WORD64)(*m1_param_real_prev) *
                             (WORD64)(one_minus_del));

              param_r[pb] = (WORD32)(acc >> 15);
            }
            m1_param_real_prev++;
          }
          param_r += MAX_PARAMETER_BANDS;

          for (ps = 1; ps < num_parameter_sets; ps++) {
            WORD32 del = delta_ptr[ps];
            WORD32 one_minus_del = one_minus_delta_ptr[ps];

            param_prev_r = param_r - MAX_PARAMETER_BANDS;

            for (pb = res_bands; pb < num_parameter_bands; pb++) {
              if (smooth_band_arr[ps][pb]) {
                WORD64 acc;

                acc = (WORD64)((WORD64)(param_r[pb]) * (WORD64)del) +
                      (WORD64)((WORD64)(param_prev_r[pb]) *
                               (WORD64)one_minus_del);

                param_r[pb] = (WORD32)(acc >> 15);
              }
            }
            param_r += MAX_PARAMETER_BANDS;
          }
          ptr2 += PBXPS;
        }
        ptr1 += INCHXPBXPS;
      }
    }

    if (curr_state->residual_coding)
      resid_col_counter = col_counter;
    else
      resid_col_counter = num_direct_signals;

    idx = 0;
    if (m2_param_imag_present) {
      WORD32 *ptr_r1 = &m2_param->m2_resid_real[0][0][0];
      WORD32 *ptr_i1 = &m2_param->m2_resid_imag[0][0][0];
      for (row = 0; row < num_output_channels; row++) {
        for (col = 0; col < resid_col_counter; col++) {
          if (curr_state->m2_param_present[row][col] & 2) {
            WORD32 del = *delta_ptr;
            WORD32 one_minus_del = *one_minus_delta_ptr;

            param_r = ptr_r1;
            param_i = ptr_i1;

            m2_resid_real_prev += res_bands;
            m2_resid_imag_prev += res_bands;

            for (pb = res_bands; pb < num_parameter_bands; pb++) {
              if (smooth_band_arr[0][pb]) {
                WORD64 acc;
                acc = (WORD64)((WORD64)(param_r[pb]) * (WORD64)(del) +
                               (WORD64)(*m2_resid_real_prev) *
                                   (WORD64)(one_minus_del));

                acc >>= 15;
                param_r[pb] = (WORD32)acc;

                acc = (WORD64)((WORD64)(param_i[pb]) * (WORD64)(del) +
                               (WORD64)(*m2_resid_imag_prev) *
                                   (WORD64)(one_minus_del));

                acc >>= 15;
                param_i[pb] = (WORD32)acc;
              }

              m2_resid_real_prev++;
              m2_resid_imag_prev++;
            }

            param_r += MAX_PARAMETER_BANDS;
            param_i += MAX_PARAMETER_BANDS;

            for (ps = 1; ps < num_parameter_sets; ps++) {
              WORD32 del = delta_ptr[ps];
              WORD32 one_minus_del = one_minus_delta_ptr[ps];

              param_prev_r = param_r - MAX_PARAMETER_BANDS;
              param_prev_i = param_i - MAX_PARAMETER_BANDS;
              for (pb = res_bands; pb < num_parameter_bands; pb++) {
                if (smooth_band_arr[ps][pb]) {
                  WORD64 acc;
                  acc = (WORD64)((WORD64)(param_r[pb]) * (WORD64)(del) +
                                 (WORD64)(param_prev_r[pb]) *
                                     (WORD64)(one_minus_del));

                  acc >>= 15;
                  param_r[pb] = (WORD32)acc;

                  acc = (WORD64)((WORD64)(param_i[pb]) * (WORD64)(del) +
                                 (WORD64)(param_prev_i[pb]) *
                                     (WORD64)(one_minus_del));

                  acc >>= 15;
                  param_i[pb] = (WORD32)acc;
                }
              }
              param_r += MAX_PARAMETER_BANDS;
              param_i += MAX_PARAMETER_BANDS;
            }
            idx++;
            ptr_r1 += PBXPS;
            ptr_i1 += PBXPS;
          }
        }
      }

      idx = 0;

      ptr_r1 = &m2_param->m2_resid_real[0][0][0];
      ptr_i1 = &m2_param->m2_resid_imag[0][0][0];
      for (row = 0; row < num_output_channels; row++) {
        for (col = num_direct_signals; col < col_counter; col++) {
          if (curr_state->m2_param_present[row][col] & 1) {
            WORD32 del = *delta_ptr;
            WORD32 one_minus_del = *one_minus_delta_ptr;
            m2_decor_real_prev += res_bands;
            m2_decor_imag_prev += res_bands;

            param_r = ptr_r1;
            param_i = ptr_i1;

            for (pb = res_bands; pb < num_parameter_bands; pb++) {
              if (smooth_band_arr[0][pb]) {
                WORD64 acc;
                acc = (WORD64)((WORD64)(param_r[pb]) * (WORD64)del +
                               (WORD64)(*m2_decor_real_prev) *
                                   (WORD64)one_minus_del);
                acc >>= 15;
                param_r[pb] = (WORD32)acc;

                acc = (WORD64)((WORD64)(param_i[pb]) * (WORD64)del +
                               (WORD64)(*m2_decor_imag_prev) *
                                   (WORD64)one_minus_del);
                acc >>= 15;
                param_i[pb] = (WORD32)acc;
              }
              m2_decor_real_prev++;
              m2_decor_imag_prev++;
            }

            param_r += MAX_PARAMETER_BANDS;
            param_i += MAX_PARAMETER_BANDS;

            for (ps = 1; ps < num_parameter_sets; ps++) {
              WORD32 del = delta_ptr[ps];
              WORD32 one_minus_del = one_minus_delta_ptr[ps];
              param_prev_r = param_r - MAX_PARAMETER_BANDS;
              param_prev_i = param_i - MAX_PARAMETER_BANDS;
              for (pb = res_bands; pb < num_parameter_bands; pb++) {
                if (smooth_band_arr[ps][pb]) {
                  WORD64 acc;

                  acc = (WORD64)((WORD64)(param_r[pb]) * (WORD64)del +
                                 (WORD64)(param_prev_r[pb]) *
                                     (WORD64)one_minus_del);
                  acc >>= 15;
                  param_r[pb] = (WORD32)acc;

                  acc = (WORD64)((WORD64)(param_i[pb]) * (WORD64)del +
                                 (WORD64)(param_prev_i[pb]) *
                                     (WORD64)one_minus_del);
                  acc >>= 15;
                  param_i[pb] = (WORD32)acc;
                }
              }
              param_r += MAX_PARAMETER_BANDS;
              param_i += MAX_PARAMETER_BANDS;
            }

            idx++;
            ptr_r1 += PBXPS;
            ptr_i1 += PBXPS;
          }
        }
      }
    } else {
      WORD32 *ptr1 = &m2_param->m2_resid_real[0][0][0];

      for (row = 0; row < num_output_channels; row++) {
        for (col = 0; col < resid_col_counter; col++) {
          if (curr_state->m2_param_present[row][col] & 2) {
            WORD32 *ptr2 = ptr1;
            WORD32 del = *delta_ptr;
            WORD32 one_minus_del = *one_minus_delta_ptr;
            m2_resid_real_prev += res_bands;

            for (pb = res_bands; pb < num_parameter_bands; pb++) {
              if (smooth_band_arr[0][pb]) {
                WORD64 acc;

                acc = (WORD64)((WORD64)(ptr2[pb]) * (WORD64)(del) +
                               (WORD64)(*m2_resid_real_prev) *
                                   (WORD64)(one_minus_del));

                acc >>= 15;
                ptr2[pb] = (WORD32)acc;
              }

              m2_resid_real_prev++;
            }

            ptr2 += MAX_PARAMETER_BANDS;

            for (ps = 1; ps < num_parameter_sets; ps++) {
              WORD32 del = delta_ptr[ps];
              WORD32 one_minus_del = one_minus_delta_ptr[ps];

              param_prev_r = ptr2 - MAX_PARAMETER_BANDS;

              for (pb = res_bands; pb < num_parameter_bands; pb++) {
                if (smooth_band_arr[ps][pb]) {
                  WORD64 acc;

                  acc = (WORD64)((WORD64)(ptr2[pb]) * (WORD64)(del) +
                                 (WORD64)(*param_prev_r) *
                                     (WORD64)(one_minus_del));

                  acc >>= 15;
                  ptr2[pb] = (WORD32)acc;
                }

                param_prev_r++;
              }
              ptr2 += MAX_PARAMETER_BANDS;
            }
            idx++;
            ptr1 += PBXPS;
          }
        }
      }
      idx = 0;
      ptr1 = &m2_param->m2_decor_real[0][0][0];

      for (row = 0; row < num_output_channels; row++) {
        for (col = num_direct_signals; col < col_counter; col++) {
          if (curr_state->m2_param_present[row][col] & 1) {
            WORD32 *ptr2 = ptr1;
            m2_decor_real_prev += res_bands;

            param_r = &m2_param->m2_decor_real[idx][0][res_bands];
            for (pb = res_bands; pb < num_parameter_bands; pb++) {
              if (smooth_band_arr[0][pb]) {
                WORD64 acc;
                acc = (WORD64)((WORD64)(ptr2[pb]) * (WORD64)*delta_ptr +
                               (WORD64)(*m2_decor_real_prev) *
                                   (WORD64)*one_minus_delta_ptr);
                acc >>= 15;
                ptr2[pb] = (WORD32)acc;
              }
              m2_decor_real_prev++;
            }
            ptr2 += MAX_PARAMETER_BANDS;

            for (ps = 1; ps < num_parameter_sets; ps++) {
              WORD32 del = delta_ptr[ps];
              WORD32 one_minus_del = one_minus_delta_ptr[ps];

              param_prev_r = ptr2 - MAX_PARAMETER_BANDS;
              for (pb = res_bands; pb < num_parameter_bands; pb++) {
                if (smooth_band_arr[ps][pb]) {
                  WORD64 acc;

                  acc =
                      (WORD64)((WORD64)(ptr2[pb]) * (WORD64)del +
                               (WORD64)(*param_prev_r) * (WORD64)one_minus_del);
                  acc >>= 15;
                  ptr2[pb] = (WORD32)acc;
                }

                param_prev_r++;
              }

              ptr2 += MAX_PARAMETER_BANDS;
            }

            idx++;

            ptr1 += PBXPS;
          }
        }
      }
    }
  }
  return;
}
