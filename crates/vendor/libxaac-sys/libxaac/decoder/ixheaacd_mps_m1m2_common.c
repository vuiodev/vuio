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
#include "ixheaac_type_def.h"
#include "ixheaacd_mps_struct_def.h"
#include "ixheaacd_error_codes.h"
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
#include "ixheaac_error_standards.h"
#include "ixheaacd_mps_polyphase.h"
#include "ixheaacd_config.h"
#include "ixheaacd_qmf_dec.h"
#include "ixheaacd_mps_dec.h"
#include "ixheaacd_mps_macro_def.h"
#include "ixheaacd_mps_bitdec.h"
#include "ixheaacd_mps_calc_m1m2_tree_config.h"

VOID ixheaacd_buffer_m1(ia_heaac_mps_state_struct *pstr_mps_state) {
  ia_mps_dec_m1_param_struct *m1_param = pstr_mps_state->array_struct->m1_param;
  WORD32 pb, row, col;

  ia_mps_persistent_mem *persistent_mem = &pstr_mps_state->mps_persistent_mem;
  WORD32 *m1_param_real_prev = persistent_mem->m1_param_real_prev;
  WORD32 *m1_param_imag_prev = persistent_mem->m1_param_imag_prev;

  WORD32 *m1_param_real, *m1_param_imag;

  WORD32 *p_m1_param_real, *p_m1_param_re;

  WORD32 num_parameter_bands = pstr_mps_state->num_parameter_bands;
  WORD32 num_v_channels = pstr_mps_state->num_v_channels;
  WORD32 num_x_channels = pstr_mps_state->num_x_channels;
  WORD32 m1_param_imag_present = pstr_mps_state->m1_param_imag_present;

  WORD32 num_parameter_sets_prev;

  pstr_mps_state->num_parameter_sets_prev = pstr_mps_state->num_parameter_sets;
  num_parameter_sets_prev = pstr_mps_state->num_parameter_sets_prev;

  if (m1_param_imag_present) {
    WORD32 *p_m1_param_imag = &m1_param->m1_param_imag[0][0][0][0];
    p_m1_param_real = &m1_param->m1_param_real[0][0][0][0];
    for (row = 0; row < num_v_channels; row++) {
      WORD32 *p_m1_param_im = p_m1_param_imag;
      p_m1_param_re = p_m1_param_real;

      for (col = 0; col < num_x_channels; col++) {
        m1_param_real = p_m1_param_re + (num_parameter_sets_prev - 1) * MAX_PARAMETER_BANDS;
        m1_param_imag = p_m1_param_im + (num_parameter_sets_prev - 1) * MAX_PARAMETER_BANDS;

        for (pb = 0; pb < num_parameter_bands; pb++) {
          *m1_param_real_prev++ = *m1_param_real++;
          *m1_param_imag_prev++ = *m1_param_imag++;
        }
        p_m1_param_re += PBXPS;
        p_m1_param_im += PBXPS;
      }
      p_m1_param_real += MAX_INPUT_CHANNELS_MPS * PBXPS;
      p_m1_param_imag += MAX_INPUT_CHANNELS_MPS * PBXPS;
    }
  } else {
    p_m1_param_real = &m1_param->m1_param_real[0][0][0][0];
    for (row = 0; row < num_v_channels; row++) {
      p_m1_param_re = p_m1_param_real;
      for (col = 0; col < num_x_channels; col++) {
        m1_param_real = p_m1_param_re + (num_parameter_sets_prev - 1) * MAX_PARAMETER_BANDS;
        for (pb = 0; pb < num_parameter_bands; pb++) *m1_param_real_prev++ = *m1_param_real++;

        p_m1_param_re += PBXPS;
      }
      p_m1_param_real += MAX_INPUT_CHANNELS_MPS * PBXPS;
    }
  }
}

VOID ixheaacd_buffer_m2(ia_heaac_mps_state_struct *pstr_mps_state) {
  WORD32 num_direct_signals = pstr_mps_state->num_direct_signals;
  WORD32 pb, row, col, col_counter = num_direct_signals + pstr_mps_state->num_decor_signals;

  ia_mps_persistent_mem *persistent_mem = &pstr_mps_state->mps_persistent_mem;
  ia_mps_dec_m2_param_struct *m2_param = pstr_mps_state->aux_struct->m2_param;

  WORD32 *m2_decor_real_prev = persistent_mem->m2_decor_real_prev;
  WORD32 *m2_decor_imag_prev = persistent_mem->m2_decor_imag_prev;

  WORD32 *m2_resid_real_prev = persistent_mem->m2_resid_real_prev;
  WORD32 *m2_resid_imag_prev = persistent_mem->m2_resid_imag_prev;

  WORD32 *m2_decor_real, *m2_decor_imag, *m2_resid_real, *m2_resid_imag;
  WORD32 idx = -1;

  WORD32 resid_col_counter;

  WORD32 num_parameter_bands = pstr_mps_state->num_parameter_bands;
  WORD32 num_output_channels = pstr_mps_state->num_output_channels;
  WORD32 num_parameter_sets_prev = pstr_mps_state->num_parameter_sets_prev;

  if (pstr_mps_state->residual_coding)
    resid_col_counter = col_counter;
  else
    resid_col_counter = num_direct_signals;

  if (pstr_mps_state->m2_param_imag_present) {
    for (row = 0; row < num_output_channels; row++) {
      for (col = num_direct_signals; col < col_counter; col++) {
        if (pstr_mps_state->m2_param_present[row][col] & 1) {
          idx++;
          m2_decor_real = &m2_param->m2_decor_real[idx][num_parameter_sets_prev - 1][0];

          for (pb = 0; pb < num_parameter_bands; pb++) *m2_decor_real_prev++ = *m2_decor_real++;

          m2_decor_imag = &m2_param->m2_decor_imag[idx][num_parameter_sets_prev - 1][0];

          for (pb = 0; pb < num_parameter_bands; pb++) *m2_decor_imag_prev++ = *m2_decor_imag++;
        }
      }
    }

    idx = -1;
    for (row = 0; row < num_output_channels; row++) {
      for (col = 0; col < resid_col_counter; col++) {
        if (pstr_mps_state->m2_param_present[row][col] & 2) {
          idx++;
          m2_resid_real = &m2_param->m2_resid_real[idx][num_parameter_sets_prev - 1][0];

          for (pb = 0; pb < num_parameter_bands; pb++) *m2_resid_real_prev++ = *m2_resid_real++;
          m2_resid_imag = &m2_param->m2_resid_imag[idx][num_parameter_sets_prev - 1][0];
          for (pb = 0; pb < num_parameter_bands; pb++) *m2_resid_imag_prev++ = *m2_resid_imag++;
        }
      }
    }
  } else {
    for (row = 0; row < num_output_channels; row++) {
      for (col = num_direct_signals; col < col_counter; col++) {
        if (pstr_mps_state->m2_param_present[row][col] & 1) {
          idx++;
          m2_decor_real = &m2_param->m2_decor_real[idx][num_parameter_sets_prev - 1][0];

          for (pb = 0; pb < num_parameter_bands; pb++) *m2_decor_real_prev++ = *m2_decor_real++;
        }
      }
    }

    idx = -1;
    for (row = 0; row < num_output_channels; row++) {
      for (col = 0; col < resid_col_counter; col++) {
        if (pstr_mps_state->m2_param_present[row][col] & 2) {
          idx++;
          m2_resid_real = &m2_param->m2_resid_real[idx][num_parameter_sets_prev - 1][0];

          for (pb = 0; pb < num_parameter_bands; pb++) *m2_resid_real_prev++ = *m2_resid_real++;
        }
      }
    }
  }
}

static VOID ixheaacd_update_alpha(ia_heaac_mps_state_struct *pstr_mps_state) {
  WORD32 alpha;

  WORD32 *arbdmx_alpha_prev = pstr_mps_state->mps_persistent_mem.arbdmx_alpha_prev;
  ia_mps_dec_auxilary_struct *p_aux_struct = pstr_mps_state->aux_struct;
  WORD32 *arbdmx_alpha = p_aux_struct->arbdmx_alpha;

  WORD32 n_ch_in = pstr_mps_state->num_input_channels;
  WORD32 ch;

  for (ch = 0; ch < n_ch_in; ch++) {
    *arbdmx_alpha_prev++ = arbdmx_alpha[ch];

    if (pstr_mps_state->arbitrary_downmix == 2) {
      alpha = arbdmx_alpha[ch];

      if (p_aux_struct->arbdmx_residual_abs[ch]) {
        alpha -= POINT_THREE_THREE_Q15;

        if (alpha < 0) alpha = 0;
      } else {
        alpha += POINT_THREE_THREE_Q15;

        if (alpha > ONE_IN_Q15) alpha = ONE_IN_Q15;
      }
    } else {
      alpha = ONE_IN_Q15;
    }

    arbdmx_alpha[ch] = alpha;
  }
}

VOID ixheaacd_calc_m1m2(ia_heaac_mps_state_struct *pstr_mps_state) {
  WORD32 up_mix_type = pstr_mps_state->up_mix_type;
  WORD32 binaural_quality = pstr_mps_state->binaural_quality;

  if (pstr_mps_state->arbitrary_downmix != 0) {
    ixheaacd_update_alpha(pstr_mps_state);
  }

  switch (pstr_mps_state->tree_config) {
    case TREE_5151: {
      if (up_mix_type == 3) {
        ixheaacd_calc_m1m2_51s1(pstr_mps_state);
      } else {
        ixheaacd_calc_m1m2_5151(pstr_mps_state);
      }
    } break;
    case TREE_5152: {
      if (up_mix_type == 3) {
        ixheaacd_calc_m1m2_51s2(pstr_mps_state);
      } else {
        ixheaacd_calc_m1m2_5152(pstr_mps_state);
      }
    } break;
    case TREE_525:
      if (up_mix_type == 1) {
        ixheaacd_calc_m1m2_emm(pstr_mps_state);
      } else if (up_mix_type == 2) {
        if (binaural_quality == 1) {
          ixheaacd_calc_m1m2_5227(pstr_mps_state);
        }
      } else {
        ixheaacd_calc_m1m2_5251(pstr_mps_state);
      }
      break;
    case TREE_7271:
      if (up_mix_type == 0) {
        ixheaacd_calc_m1m2_7271(pstr_mps_state);
      } else if (up_mix_type == 2) {
        if (binaural_quality == 1) {
          ixheaacd_calc_m1m2_5227(pstr_mps_state);
        }
      }
      break;
    case TREE_7272:
      if (up_mix_type == 0) {
        ixheaacd_calc_m1m2_7272(pstr_mps_state);
      } else if (up_mix_type == 2) {
        if (binaural_quality == 1) {
          ixheaacd_calc_m1m2_5227(pstr_mps_state);
        }
      }
      break;
    case TREE_7571:
      ixheaacd_calc_m1m2_7571(pstr_mps_state);
      break;
    case TREE_7572:
      ixheaacd_calc_m1m2_7572(pstr_mps_state);
      break;
    default:
      break;
  };

  return;
}
