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
#include "ixheaacd_mps_res_rom.h"
#include "ixheaacd_mps_aac_struct.h"
#include "ixheaac_constants.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaacd_bitbuffer.h"
#include "ixheaacd_error_codes.h"
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
#include "ixheaacd_mps_bitdec.h"
#include "ixheaacd_mps_macro_def.h"
#include "ixheaacd_mps_basic_op.h"
#include "ixheaacd_mps_reshape_bb_env.h"
#include "ixheaac_error_standards.h"

#define ALIGN_SIZE64(x) ((((x) + 7) >> 3) << 3)

VOID ixheaacd_init_bb_env(ia_heaac_mps_state_struct *pstr_mps_state) {
  WORD32 k, j;
  ia_mps_dec_reshape_bb_env_state_struct *reshape_bb_env_state =
      pstr_mps_state->mps_persistent_mem.reshape_bb_env_state;

  for (k = 0; k < 2 * MAX_OUTPUT_CHANNELS_MPS + MAX_INPUT_CHANNELS_MPS; k++) {
    reshape_bb_env_state->norm_nrg_prev[k] = ONE_IN_Q30;
    reshape_bb_env_state->frame_nrg_prev[k] = 0;
    reshape_bb_env_state->q_frame_nrg_prev[k] = 30;
    reshape_bb_env_state->q_norm_nrg_prev[k] = 30;
    for (j = 0; j < MAX_PARAMETER_BANDS; j++) {
      reshape_bb_env_state->part_nrg_prev[k][j] = 0;
      reshape_bb_env_state->q_part_nrg_prev[k][j] = 30;
    }
  }
}

static VOID ixheaacd_extract_bb_env(ia_heaac_mps_state_struct *pstr_mps_state, WORD32 inp,
                                            WORD32 ch, WORD32 *env, VOID *scratch, WORD32 flag) {
  ia_mps_dec_reshape_bb_env_state_struct *reshape_bb_env_state =
      pstr_mps_state->mps_persistent_mem.reshape_bb_env_state;
  WORD64 *slot_nrg_fix, *slot_nrg;
  WORD16 *q_slot_nrg_fix, *q_slot_nrg;
  WORD32 *part_nrg_fix;
  WORD16 *q_part_nrg_fix;

  WORD32 *p_buffer_real, *p_buffer_imag, *p_buffer_re, *p_buffer_im;
  WORD32 ts, qs, pb;

  WORD32 start_p = 10;
  WORD32 end_p = 18;
  WORD32 env_fix_l;
  WORD16 q_env_fix_l;

  WORD16 alpha_fix = ALPHA_Q15;
  WORD16 beta_fix = BETA_Q15;

  WORD16 one_min_alpha_fix = ONE_MINUS_ALPHA_Q16;
  WORD16 one_min_beta_fix = ONE_MINUS_BETA_Q16;
  WORD16 one_by_nine = ONE_BY_NINE_Q16;
  WORD32 frame_nrg_fix = 0;
  WORD32 *norm_nrg_fix;
  WORD16 q_frame_nrg_fix = 0;
  WORD16 *q_norm_nrg_fix;
  WORD32 temp_1, temp4;
  WORD16 qtemp1, q_env;

  WORD32 prev_ch_offs;
  WORD32 cnt = min(42, pstr_mps_state->hybrid_bands);
  WORD32 time_slots = pstr_mps_state->time_slots;
  const WORD32 *sqrt_tab = pstr_mps_state->ia_mps_dec_mps_table.common_table_ptr->sqrt_tab;
  WORD32 *hyb_output_real_dry, *n_slot_nrg;
  WORD32 *hyb_output_imag_dry;

  const WORD32 *bb_env_kernels =
      pstr_mps_state->ia_mps_dec_mps_table.bitdec_table_ptr->kernel_table.bb_env_kernels;

  q_slot_nrg_fix = (WORD16 *)scratch;
  n_slot_nrg =
      (WORD32 *)((WORD8 *)scratch + IXHEAAC_GET_SIZE_ALIGNED(RESHAPE_OFFSET_1, BYTE_ALIGN_8));
  slot_nrg_fix =
      (WORD64 *)((WORD8 *)scratch + IXHEAAC_GET_SIZE_ALIGNED(RESHAPE_OFFSET_2, BYTE_ALIGN_8));
  switch (inp) {
    WORD32 frame_nrg_prev;
    WORD16 q_frame_nrg_prev;
    WORD32 *p_hyb_out_dry_real, *p_hyb_out_dry_imag;

    case INP_DRY_WET:
      frame_nrg_prev = reshape_bb_env_state->frame_nrg_prev[ch];
      q_frame_nrg_prev = reshape_bb_env_state->q_frame_nrg_prev[ch];

      part_nrg_fix = &reshape_bb_env_state->part_nrg_prev[ch][0];
      q_part_nrg_fix = &reshape_bb_env_state->q_part_nrg_prev[ch][0];

      norm_nrg_fix = &reshape_bb_env_state->norm_nrg_prev[ch];
      q_norm_nrg_fix = &reshape_bb_env_state->q_norm_nrg_prev[ch];

      p_buffer_real = pstr_mps_state->array_struct->buf_real + ch * TSXHB + 12;
      p_buffer_imag = pstr_mps_state->array_struct->buf_imag + ch * TSXHB + 12;

      p_hyb_out_dry_real = pstr_mps_state->array_struct->hyb_output_real_dry + ch * TSXHB + 12;
      p_hyb_out_dry_imag = pstr_mps_state->array_struct->hyb_output_imag_dry + ch * TSXHB + 12;

      for (ts = 0; ts < time_slots; ts++) {
        WORD32 prev_idx = 10;

        slot_nrg = slot_nrg_fix + 4;
        for (pb = 14; pb <= end_p; pb++) *slot_nrg++ = 0;

        slot_nrg = slot_nrg_fix;

        p_buffer_re = p_buffer_real;
        p_buffer_im = p_buffer_imag;

        hyb_output_real_dry = p_hyb_out_dry_real;
        hyb_output_imag_dry = p_hyb_out_dry_imag;

        for (qs = 12; qs < 16; qs++) {
          temp_1 = ixheaac_add32_sat(*hyb_output_real_dry, *p_buffer_re);
          temp4 = ixheaac_add32_sat(*hyb_output_imag_dry, *p_buffer_im);

          *slot_nrg++ = ixheaac_add64_sat(ixheaac_mult32x32in64(temp_1, temp_1), 
		                ixheaac_mult32x32in64(temp4, temp4));

          p_buffer_re++;
          p_buffer_im++;
          hyb_output_real_dry++;
          hyb_output_imag_dry++;
        }
        prev_idx = 14;
        for (; qs < 30; qs++) {
          WORD32 idx = bb_env_kernels[qs];
          if (prev_idx != idx) {
            slot_nrg++;
            prev_idx = idx;
          }
          temp_1 = ixheaac_add32_sat(*hyb_output_real_dry, *p_buffer_re);
          temp4 = ixheaac_add32_sat(*hyb_output_imag_dry, *p_buffer_im);

		  *slot_nrg = ixheaac_add64_sat(*slot_nrg, ixheaac_mult32x32in64(temp_1, temp_1));
		  *slot_nrg = ixheaac_add64_sat(*slot_nrg, ixheaac_mult32x32in64(temp4, temp4));

          p_buffer_re++;
          p_buffer_im++;
          hyb_output_real_dry++;
          hyb_output_imag_dry++;
        }
        slot_nrg++;
        for (; qs < cnt; qs++) {
          temp_1 = ixheaac_add32_sat(*hyb_output_real_dry, *p_buffer_re);
          temp4 = ixheaac_add32_sat(*hyb_output_imag_dry, *p_buffer_im);

		  *slot_nrg = ixheaac_add64_sat(*slot_nrg, ixheaac_mult32x32in64(temp_1, temp_1));
		  *slot_nrg = ixheaac_add64_sat(*slot_nrg, ixheaac_mult32x32in64(temp4, temp4));

          p_buffer_re++;
          p_buffer_im++;
          hyb_output_real_dry++;
          hyb_output_imag_dry++;
        }

        slot_nrg = slot_nrg_fix;
        q_slot_nrg = q_slot_nrg_fix;

        frame_nrg_fix = 0;
        q_frame_nrg_fix = 30;
        for (pb = start_p; pb <= end_p; pb++) {
          *n_slot_nrg = ixheaacd_mps_narrow(*slot_nrg, q_slot_nrg);
          slot_nrg++;
          temp_1 = ixheaac_mult32x16in32(*n_slot_nrg, one_min_alpha_fix);
          temp4 = ixheaac_mult32x16in32((part_nrg_fix[pb]) << 1, alpha_fix);
          part_nrg_fix[pb] =
              ixheaacd_mps_reshape_add32(temp4, temp_1, &q_part_nrg_fix[pb], *q_slot_nrg);

          frame_nrg_fix = ixheaacd_mps_reshape_add32(frame_nrg_fix, *n_slot_nrg++,
                                                     &q_frame_nrg_fix, *q_slot_nrg++);
        }

        frame_nrg_fix = ixheaac_mult32x16in32(frame_nrg_fix, one_by_nine);

        temp_1 = ixheaac_mult32x16in32(frame_nrg_fix, one_min_alpha_fix);
        temp4 = ixheaac_mult32x16in32((frame_nrg_prev) << 1, alpha_fix);
        frame_nrg_fix =
            ixheaacd_mps_reshape_add32(temp_1, temp4, &q_frame_nrg_fix, q_frame_nrg_prev);

        frame_nrg_prev = frame_nrg_fix;
        q_frame_nrg_prev = q_frame_nrg_fix;

        env_fix_l = 0;
        q_env_fix_l = 30;
        q_slot_nrg = q_slot_nrg_fix;

        n_slot_nrg -= PB_OFFSET;
        for (pb = start_p; pb <= end_p; pb++) {
          temp_1 = ixheaacd_mps_div_32(*n_slot_nrg++, part_nrg_fix[pb], &qtemp1);
          qtemp1 = *q_slot_nrg++ + qtemp1 - q_part_nrg_fix[pb];
          env_fix_l = ixheaacd_mps_reshape_add32(env_fix_l, temp_1, &q_env_fix_l, qtemp1);
        }
        n_slot_nrg -= PB_OFFSET;

        env_fix_l =
            ixheaacd_mps_mult32x32(env_fix_l, frame_nrg_fix, &q_env_fix_l, q_frame_nrg_fix);

        temp_1 = ixheaac_mult32x16in32(env_fix_l, one_min_beta_fix);
        temp4 = ixheaac_mult32x16in32((*norm_nrg_fix) << 1, beta_fix);
        *norm_nrg_fix = ixheaacd_mps_reshape_add32(temp4, temp_1, q_norm_nrg_fix, q_env_fix_l);

        if (flag) {
          temp_1 = ixheaacd_mps_div_32(env_fix_l, *norm_nrg_fix, &qtemp1);
          q_env = q_env_fix_l + qtemp1 - *q_norm_nrg_fix;
          env[ts] = ixheaacd_mps_sqrt(temp_1, &(q_env), sqrt_tab);
          env[ts] = ixheaacd_mps_convert_to_qn(env[ts], q_env, 15);
        }

        p_buffer_real += MAX_HYBRID_BANDS;
        p_buffer_imag += MAX_HYBRID_BANDS;

        p_hyb_out_dry_real += MAX_HYBRID_BANDS;
        p_hyb_out_dry_imag += MAX_HYBRID_BANDS;
      }
      reshape_bb_env_state->frame_nrg_prev[ch] = frame_nrg_prev;
      reshape_bb_env_state->q_frame_nrg_prev[ch] = q_frame_nrg_prev;

      break;
    case INP_DMX:
      prev_ch_offs = ch + pstr_mps_state->num_output_channels;

      frame_nrg_prev = reshape_bb_env_state->frame_nrg_prev[prev_ch_offs];
      q_frame_nrg_prev = reshape_bb_env_state->q_frame_nrg_prev[prev_ch_offs];

      part_nrg_fix = &reshape_bb_env_state->part_nrg_prev[prev_ch_offs][0];
      q_part_nrg_fix = &reshape_bb_env_state->q_part_nrg_prev[prev_ch_offs][0];

      norm_nrg_fix = &reshape_bb_env_state->norm_nrg_prev[prev_ch_offs];
      q_norm_nrg_fix = &reshape_bb_env_state->q_norm_nrg_prev[prev_ch_offs];

      p_buffer_real = pstr_mps_state->array_struct->x_real + ch * TSXHB + 12;
      p_buffer_imag = pstr_mps_state->array_struct->x_imag + ch * TSXHB + 12;
      for (ts = 0; ts < time_slots; ts++) {
        WORD32 prev_idx;

        slot_nrg = slot_nrg_fix + 4;
        for (pb = 14; pb <= end_p; pb++) *slot_nrg++ = 0;

        slot_nrg = slot_nrg_fix;

        hyb_output_real_dry = p_buffer_real;
        hyb_output_imag_dry = p_buffer_imag;

        for (qs = 12; qs < 16; qs++) {
          *slot_nrg++ = ixheaac_add64_sat(ixheaac_mult32x32in64(*hyb_output_real_dry, *hyb_output_real_dry), 
		                ixheaac_mult32x32in64(*hyb_output_imag_dry, *hyb_output_imag_dry));

          hyb_output_real_dry++;
          hyb_output_imag_dry++;
        }
        prev_idx = 14;
        for (; qs < 30; qs++) {
          WORD32 idx = bb_env_kernels[qs];
          if (prev_idx != idx) {
            slot_nrg++;
            prev_idx = idx;
          }

		  *slot_nrg = ixheaac_add64_sat(*slot_nrg, ixheaac_mult32x32in64(*hyb_output_real_dry, *hyb_output_real_dry));
		  *slot_nrg = ixheaac_add64_sat(*slot_nrg, ixheaac_mult32x32in64(*hyb_output_imag_dry, *hyb_output_imag_dry));

          hyb_output_real_dry++;
          hyb_output_imag_dry++;
        }
        slot_nrg++;
        for (; qs < cnt; qs++) {
		  *slot_nrg = ixheaac_add64_sat(*slot_nrg, ixheaac_mult32x32in64(*hyb_output_real_dry, *hyb_output_real_dry));
		  *slot_nrg = ixheaac_add64_sat(*slot_nrg, ixheaac_mult32x32in64(*hyb_output_imag_dry, *hyb_output_imag_dry));

          hyb_output_real_dry++;
          hyb_output_imag_dry++;
        }

        slot_nrg = slot_nrg_fix;
        q_slot_nrg = q_slot_nrg_fix;

        frame_nrg_fix = 0;
        q_frame_nrg_fix = 30;
        for (pb = start_p; pb <= end_p; pb++) {
          *n_slot_nrg = ixheaacd_mps_narrow(*slot_nrg, q_slot_nrg);
          slot_nrg++;
          temp_1 = ixheaac_mult32x16in32(*n_slot_nrg, one_min_alpha_fix);
          temp4 = ixheaac_mult32x16in32((part_nrg_fix[pb]) << 1, alpha_fix);
          part_nrg_fix[pb] =
              ixheaacd_mps_reshape_add32(temp4, temp_1, &q_part_nrg_fix[pb], *q_slot_nrg);
          frame_nrg_fix = ixheaacd_mps_reshape_add32(frame_nrg_fix, *n_slot_nrg++,
                                                     &q_frame_nrg_fix, *q_slot_nrg++);
        }

        frame_nrg_fix = ixheaac_mult32x16in32(frame_nrg_fix, one_by_nine);

        temp_1 = ixheaac_mult32x16in32(frame_nrg_fix, one_min_alpha_fix);
        temp4 = ixheaac_mult32x16in32((frame_nrg_prev) << 1, alpha_fix);
        frame_nrg_fix =
            ixheaacd_mps_reshape_add32(temp_1, temp4, &q_frame_nrg_fix, q_frame_nrg_prev);

        frame_nrg_prev = frame_nrg_fix;
        q_frame_nrg_prev = q_frame_nrg_fix;

        env_fix_l = 0;
        q_env_fix_l = 30;

        q_slot_nrg = q_slot_nrg_fix;
        n_slot_nrg -= PB_OFFSET;
        for (pb = start_p; pb <= end_p; pb++) {
          temp_1 = ixheaacd_mps_div_32(*n_slot_nrg++, part_nrg_fix[pb], &qtemp1);
          qtemp1 = *q_slot_nrg++ + qtemp1 - q_part_nrg_fix[pb];
          env_fix_l = ixheaacd_mps_reshape_add32(env_fix_l, temp_1, &q_env_fix_l, qtemp1);
        }
        n_slot_nrg -= PB_OFFSET;

        env_fix_l =
            ixheaacd_mps_mult32x32(env_fix_l, frame_nrg_fix, &q_env_fix_l, q_frame_nrg_fix);

        temp_1 = ixheaac_mult32x16in32(env_fix_l, one_min_beta_fix);
        temp4 = ixheaac_mult32x16in32((*norm_nrg_fix) << 1, beta_fix);
        *norm_nrg_fix = ixheaacd_mps_reshape_add32(temp4, temp_1, q_norm_nrg_fix, q_env_fix_l);

        temp_1 = ixheaacd_mps_div_32(env_fix_l, *norm_nrg_fix, &qtemp1);
        q_env = q_env_fix_l + qtemp1 - *q_norm_nrg_fix;
        env[ts] = ixheaacd_mps_sqrt(temp_1, &(q_env), sqrt_tab);
        env[ts] = ixheaacd_mps_convert_to_qn(env[ts], q_env, 15);

        p_buffer_real += MAX_HYBRID_BANDS;
        p_buffer_imag += MAX_HYBRID_BANDS;
      }
      reshape_bb_env_state->frame_nrg_prev[prev_ch_offs] = frame_nrg_prev;
      reshape_bb_env_state->q_frame_nrg_prev[prev_ch_offs] = q_frame_nrg_prev;

      break;
    default:
      break;
  }
  return;
}

VOID ixheaacd_reshape_bb_env(ia_heaac_mps_state_struct *pstr_mps_state) {
  WORD32 *env_dry;
  WORD32 *env_dmx_0, *env_dmx_1;

  WORD32 *p_buffer_real, *p_buffer_imag, *p_buffer_re, *p_buffer_im;
  WORD32 *hyb_output_real_wet, *hyb_output_imag_wet;

  WORD32 temp_1, temp_2;
  WORD16 qtemp1, qtemp2;
  WORD32 tmp, dry_fac, slot_amp_dry, slot_amp_wet;
  WORD16 q_dry_fac, q_slot_amp_dry, q_slot_amp_wet;

  WORD32 slot_amp_ratio;
  WORD16 q_slot_amp_ratio;
  WORD32 ch, ch2, ts, qs;
  WORD32 *hyb_output_real_dry, *hyb_out_dry_real;
  WORD32 *hyb_output_imag_dry, *hyb_out_dry_imag;
  WORD64 *inter;

  VOID *free_scratch;
  const WORD32 *sqrt_tab = pstr_mps_state->ia_mps_dec_mps_table.common_table_ptr->sqrt_tab;
  ia_mps_dec_auxilary_struct *p_aux_struct = pstr_mps_state->aux_struct;
  WORD32 *temp_shape_enable_channel_ges = p_aux_struct->temp_shape_enable_channel_ges;

  WORD32 start_hsb;
  WORD32 time_slots = pstr_mps_state->time_slots;
  WORD32 num_output_channels = pstr_mps_state->num_output_channels;
  WORD32 tree_config = pstr_mps_state->tree_config;
  WORD32 hybrid_bands = pstr_mps_state->hybrid_bands;

  const WORD32 *ch_idx = &pstr_mps_state->ia_mps_dec_mps_table.m1_m2_table_ptr->idx_table
                              .row_2_channel_ges[tree_config][0];
  WORD64 acc, acc2;
  start_hsb = 6;

  free_scratch = pstr_mps_state->mps_scratch_mem_v;
  env_dry = free_scratch;
  env_dmx_0 = pstr_mps_state->array_struct->env_dmx_0;
  env_dmx_1 = pstr_mps_state->array_struct->env_dmx_1;
  inter = (WORD64 *)((WORD8 *)free_scratch +
                     IXHEAAC_GET_SIZE_ALIGNED(MAX_TIME_SLOTSX12, BYTE_ALIGN_8));
  free_scratch =
      inter + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_TIME_SLOTS, sizeof(*inter), BYTE_ALIGN_8);

  p_buffer_real = pstr_mps_state->array_struct->buf_real + start_hsb;
  p_buffer_imag = pstr_mps_state->array_struct->buf_imag + start_hsb;

  for (ch = 0; ch < num_output_channels; ch++) {
    ch2 = ch_idx[ch];

    if (ch2 == -1) continue;

    p_buffer_re = p_buffer_real;
    p_buffer_im = p_buffer_imag;

    ixheaacd_extract_bb_env(pstr_mps_state, INP_DRY_WET, ch, env_dry, free_scratch,
                                         temp_shape_enable_channel_ges[ch2]);

    if (temp_shape_enable_channel_ges[ch2]) {
      WORD32 *env = &p_aux_struct->env_shape_data[ch2][0];
      switch (tree_config) {
        case TREE_5151:
        case TREE_5152:
          for (ts = 0; ts < time_slots; ts++) {
            inter[ts] = (WORD64)((WORD64)*env++ * (WORD64)env_dmx_0[ts]);
          }
          break;

        case TREE_525:
        case TREE_7271:
        case TREE_7272:

          switch (ch2) {
            case 0:
            case 3:
            case 5:

              for (ts = 0; ts < time_slots; ts++) {
                inter[ts] = (WORD64)((WORD64)*env++ * (WORD64)env_dmx_0[ts]);
              }
              break;
            case 1:
            case 4:
            case 6:

              for (ts = 0; ts < time_slots; ts++) {
                inter[ts] = (WORD64)((WORD64)*env++ * (WORD64)env_dmx_1[ts]);
              }
              break;
            case 2:

              for (ts = 0; ts < time_slots; ts++) {
                temp_2 = ((WORD64)env_dmx_0[ts] + (WORD64)env_dmx_1[ts]) >> 1;
                inter[ts] = (WORD64)((WORD64)*env++ * (WORD64)temp_2);
              }
              break;
            default:
              break;
          }
          break;

        case TREE_7571:
        case TREE_7572:
          switch (ch2) {
            case 0:
            case 2:
              for (ts = 0; ts < time_slots; ts++) {
                inter[ts] = (WORD64)((WORD64)*env++ * (WORD64)env_dmx_0[ts]);
              }

              break;
            case 1:
            case 3:
              for (ts = 0; ts < time_slots; ts++) {
                inter[ts] = (WORD64)((WORD64)*env++ * (WORD64)env_dmx_1[ts]);
              }
              break;
            default:
              break;
          }
        default:
          break;
      }

      hyb_out_dry_real =
          pstr_mps_state->array_struct->hyb_output_real_dry + ch * TSXHB + start_hsb;
      hyb_out_dry_imag =
          pstr_mps_state->array_struct->hyb_output_imag_dry + ch * TSXHB + start_hsb;

      for (ts = 0; ts < time_slots; ts++) {
        tmp = ixheaacd_mps_narrow(inter[ts], &qtemp1);

        if (env_dry[ts] == 0) {
          q_dry_fac = 0;
          dry_fac = MAX_32;
        } else {
          dry_fac = ixheaacd_mps_div_32(tmp, env_dry[ts], &q_dry_fac);
          q_dry_fac += qtemp1 - 5;
        }

        hyb_output_real_wet = p_buffer_re;
        hyb_output_imag_wet = p_buffer_im;

        hyb_output_real_dry = hyb_out_dry_real;
        hyb_output_imag_dry = hyb_out_dry_imag;
        acc = 0;
        acc2 = 0;

        for (qs = start_hsb; qs < hybrid_bands; qs++) {
          acc = ixheaac_add64_sat(acc, ixheaac_mult32x32in64(*hyb_output_real_dry, *hyb_output_real_dry));
          hyb_output_real_dry++;
          acc = ixheaac_add64_sat(acc, ixheaac_mult32x32in64(*hyb_output_imag_dry, *hyb_output_imag_dry));
          hyb_output_imag_dry++;

          acc2 = ixheaac_add64_sat(acc2, ixheaac_mult32x32in64(*hyb_output_real_wet, *hyb_output_real_wet));
          hyb_output_real_wet++;
          acc2 = ixheaac_add64_sat(acc2, ixheaac_mult32x32in64(*hyb_output_imag_wet, *hyb_output_imag_wet));
          hyb_output_imag_wet++;
        }
        slot_amp_dry = ixheaacd_mps_narrow(acc, &q_slot_amp_dry);
        slot_amp_wet = ixheaacd_mps_narrow(acc2, &q_slot_amp_wet);

        qtemp1 = q_slot_amp_dry;

        temp_1 = ixheaacd_mps_add32(slot_amp_dry, ABS_THR_FIX, &qtemp1, 15);
        temp_2 = ixheaacd_mps_div_32(slot_amp_wet, temp_1, &qtemp2);
        q_slot_amp_ratio = qtemp2 + q_slot_amp_wet - qtemp1;
        slot_amp_ratio = ixheaacd_mps_sqrt(temp_2, &q_slot_amp_ratio, sqrt_tab);

        temp_1 = ixheaacd_mps_convert_to_qn(dry_fac, q_dry_fac, 15);
        temp_1 -= ONE_IN_Q15;
        temp_1 = ixheaacd_mps_mult32_shr_16(temp_1, slot_amp_ratio);
        q_slot_amp_ratio -= 1;

        temp_1 = ixheaacd_mps_add32(temp_1, dry_fac, &q_slot_amp_ratio, q_dry_fac);

        temp_1 = ixheaacd_mps_convert_to_qn(temp_1, q_slot_amp_ratio, 15);
        temp_1 = max(ONE_IN_Q13, temp_1);
        dry_fac = min(FOUR_IN_Q15, temp_1);

        hyb_output_real_dry = hyb_out_dry_real;
        hyb_output_imag_dry = hyb_out_dry_imag;

        for (qs = start_hsb; qs < hybrid_bands; qs++) {
          *hyb_output_real_dry = ixheaacd_mps_mult32_shr_15(*hyb_output_real_dry, dry_fac);
          hyb_output_real_dry++;
          *hyb_output_imag_dry = ixheaacd_mps_mult32_shr_15(*hyb_output_imag_dry, dry_fac);
          hyb_output_imag_dry++;
        }
        p_buffer_re += MAX_HYBRID_BANDS;
        p_buffer_im += MAX_HYBRID_BANDS;
        hyb_out_dry_real += MAX_HYBRID_BANDS;
        hyb_out_dry_imag += MAX_HYBRID_BANDS;
      }
    }
    p_buffer_real += TSXHB;
    p_buffer_imag += TSXHB;
  }
  return;
}

VOID ixheaacd_pre_reshape_bb_env(ia_heaac_mps_state_struct *pstr_mps_state) {
  WORD32 *env_dmx_0, *env_dmx_1;

  VOID *free_scratch;

  WORD32 tree_config = pstr_mps_state->tree_config;

  free_scratch = pstr_mps_state->mps_scratch_mem_v;
  env_dmx_0 = pstr_mps_state->array_struct->env_dmx_0;
  env_dmx_1 = pstr_mps_state->array_struct->env_dmx_1;

  switch (tree_config) {
    case TREE_7572:
      ixheaacd_extract_bb_env(pstr_mps_state, INP_DMX, 0 + 4, env_dmx_0, free_scratch, 0);
      ixheaacd_extract_bb_env(pstr_mps_state, INP_DMX, 1 + 4, env_dmx_1, free_scratch, 0);
      break;
    default:
      ixheaacd_extract_bb_env(pstr_mps_state, INP_DMX, 0, env_dmx_0, free_scratch, 0);
      if (min(pstr_mps_state->num_input_channels, 2) == 2) {
        ixheaacd_extract_bb_env(pstr_mps_state, INP_DMX, 1, env_dmx_1, free_scratch, 0);
      }
  }
  return;
}
