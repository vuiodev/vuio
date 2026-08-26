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
#include <math.h>
#include "ixheaacd_sbr_common.h"
#include "ixheaac_type_def.h"

#include "ixheaac_constants.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops16.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaac_basic_ops.h"
#include "ixheaacd_defines.h"

#include "ixheaacd_intrinsics.h"
#include "ixheaac_sbr_const.h"
#include "ixheaac_basic_op.h"
#include "ixheaacd_defines.h"
#include "ixheaacd_bitbuffer.h"
#include "ixheaacd_pns.h"

#include "ixheaacd_aac_rom.h"
#include "ixheaacd_pulsedata.h"

#include "ixheaacd_drc_data_struct.h"
#include "ixheaacd_lt_predict.h"
#include "ixheaacd_cnst.h"
#include "ixheaacd_ec_defines.h"
#include "ixheaacd_ec_struct_def.h"
#include "ixheaacd_channelinfo.h"
#include "ixheaacd_drc_dec.h"

#include "ixheaacd_sbrdecoder.h"

#include "ixheaacd_sbrdecsettings.h"
#include "ixheaacd_sbr_scale.h"
#include "ixheaacd_lpp_tran.h"
#include "ixheaacd_env_extr_part.h"
#include "ixheaacd_sbr_rom.h"
#include "ixheaacd_hybrid.h"
#include "ixheaacd_ps_dec.h"
#include "ixheaacd_ps_bitdec.h"
#include "ixheaacd_env_extr.h"
#include "ixheaacd_common_rom.h"
#include "ixheaacd_freq_sca.h"

#include "ixheaacd_qmf_dec.h"

#include "ixheaacd_env_calc.h"

#include "ixheaacd_pvc_dec.h"
#include "ixheaacd_sbr_dec.h"
#include "ixheaacd_env_dec.h"
#include "ixheaacd_basic_funcs.h"
#include "ixheaacd_sbr_crc.h"
#include "ixheaacd_function_selector.h"

#include "ixheaacd_audioobjtypes.h"
#include "ixheaacd_error_codes.h"

#define ALIGN_SIZE64(x) ((((x) + 7) >> 3) << 3)

static const FLOAT32 ixheaacd_new_bw_table[4][4] = {
    {0.00f, 0.60f, 0.90f, 0.98f},
    {0.60f, 0.75f, 0.90f, 0.98f},
    {0.00f, 0.75f, 0.90f, 0.98f},
    {0.00f, 0.75f, 0.90f, 0.98f}};
static const WORD32 ixheaacd_inew_bw_table[4][4] = {
    {0x00000000, 0x4ccccccd, 0x73333333, 0x7d70a3d7},
    {0x4ccccccd, 0x60000000, 0x73333333, 0x7d70a3d7},
    {0x00000000, 0x60000000, 0x73333333, 0x7d70a3d7},
    {0x00000000, 0x60000000, 0x73333333, 0x7d70a3d7}};

VOID ixheaacd_reset_sbrenvelope_calc(ia_sbr_calc_env_struct *h_cal_env) {
  h_cal_env->ph_index = 0;
  h_cal_env->filt_buf_noise_e = 0;
  h_cal_env->start_up = 1;
}

VOID ixheaacd_derive_lim_band_tbl(
    ia_sbr_header_data_struct *ptr_header_data,
    const ia_patch_param_struct *p_str_patch_param, WORD16 num_patches,
    ixheaacd_misc_tables *pstr_common_tables) {
  WORD32 i, k, k_1;
  WORD32 nr_lim, patch_border_k, patch_border_k_1, temp_nr_lim;

  WORD16 lim_table[MAX_FREQ_COEFFS / 2 + MAX_NUM_PATCHES + 1];
  WORD16 patch_borders[MAX_NUM_PATCHES + 1];
  WORD16 kx, k2;
  WORD16 temp, lim_bands, num_octaves;

  WORD16 *f_lim_tbl = ptr_header_data->pstr_freq_band_data->freq_band_tbl_lim;
  WORD16 *num_lf_bands = &ptr_header_data->pstr_freq_band_data->num_lf_bands;
  WORD16 *f_low_tbl =
      ptr_header_data->pstr_freq_band_data->freq_band_table[LOW];
  WORD16 num_low_bnd = ptr_header_data->pstr_freq_band_data->num_sf_bands[LOW];
  WORD16 limiter_bands = ptr_header_data->limiter_bands;

  WORD16 sub_band_start = f_low_tbl[0];
  WORD16 sub_band_end = f_low_tbl[num_low_bnd];
  const WORD16 limbnd_per_oct[4] = {(WORD16)0x2000, (WORD16)0x2666, (WORD16)0x4000,
                                    (WORD16)0x6000};

  if (limiter_bands == 0) {
    f_lim_tbl[0] = 0;
    f_lim_tbl[1] = sub_band_end - sub_band_start;
    nr_lim = 1;
  } else {
    for (k = 0; k < num_patches; k++) {
      patch_borders[k] = p_str_patch_param[k].guard_start_band - sub_band_start;
    }
    patch_borders[k] = sub_band_end - sub_band_start;

    for (k = 0; k <= num_low_bnd; k++) {
      lim_table[k] = f_low_tbl[k] - sub_band_start;
    }
    for (k = 1; k < num_patches; k++) {
      lim_table[num_low_bnd + k] = patch_borders[k];
    }

    temp_nr_lim = nr_lim = (num_low_bnd + num_patches) - 1;
    ixheaacd_aac_shellsort(lim_table, (temp_nr_lim + 1));

    k = 1;
    k_1 = 0;

    lim_bands = limbnd_per_oct[limiter_bands];

    while ((k - temp_nr_lim) <= 0) {
      k2 = lim_table[k] + sub_band_start;
      kx = lim_table[k_1] + sub_band_start;

      num_octaves = pstr_common_tables->log_dual_is_table[k2];
      num_octaves -= pstr_common_tables->log_dual_is_table[kx];

      temp = (WORD16)(((WORD32)lim_bands * (WORD32)num_octaves) >> 15);

      if (temp < 0x01f6) {
        if (lim_table[k_1] == lim_table[k]) {
          lim_table[k] = sub_band_end;
          nr_lim = nr_lim - 1;
          k = (k + 1);
          continue;
        }
        patch_border_k_1 = patch_border_k = 0;

        for (i = 0; i <= num_patches; i++) {
          if (lim_table[k] == patch_borders[i]) {
            patch_border_k = 1;
          }
          if (lim_table[k_1] == patch_borders[i]) {
            patch_border_k_1 = 1;
          }
        }
        if (!patch_border_k) {
          lim_table[k] = sub_band_end;
          nr_lim = nr_lim - 1;
          k = (k + 1);
          continue;
        }

        if (!patch_border_k_1) {
          lim_table[k_1] = sub_band_end;
          nr_lim = nr_lim - 1;
        }
      }
      k_1 = k;
      k = (k + 1);
    }
    ixheaacd_aac_shellsort(lim_table, (temp_nr_lim + 1));

    memcpy(f_lim_tbl, lim_table, sizeof(WORD16) * (nr_lim + 1));
  }
  *num_lf_bands = nr_lim;

  return;
}

VOID ixheaacd_lean_sbrconcealment(
    ia_sbr_header_data_struct *ptr_header_data,
    ia_sbr_frame_info_data_struct *ptr_sbr_data,
    ia_sbr_prev_frame_data_struct *ptr_prev_data) {
  WORD32 target;
  WORD32 step;
  WORD32 i;

  WORD16 cur_start_pos;
  WORD16 cur_stop_pos;

  ptr_sbr_data->amp_res = ptr_prev_data->amp_res;
  ptr_sbr_data->coupling_mode = ptr_prev_data->coupling_mode;
  ptr_sbr_data->max_qmf_subband_aac = ptr_prev_data->max_qmf_subband_aac;

  memcpy(ptr_sbr_data->sbr_invf_mode, ptr_prev_data->sbr_invf_mode,
         sizeof(WORD32) * MAX_INVF_BANDS);

  ptr_sbr_data->str_frame_info_details.num_env = 1;

  cur_start_pos = ptr_prev_data->end_position - ptr_header_data->num_time_slots;
  cur_stop_pos = ptr_header_data->num_time_slots;

  ptr_sbr_data->str_frame_info_details.border_vec[0] = cur_start_pos;
  ptr_sbr_data->str_frame_info_details.border_vec[1] = cur_stop_pos;

  ptr_sbr_data->str_frame_info_details.noise_border_vec[0] = cur_start_pos;
  ptr_sbr_data->str_frame_info_details.noise_border_vec[1] = cur_stop_pos;
  ;

  ptr_sbr_data->str_frame_info_details.freq_res[0] = 1;
  ptr_sbr_data->str_frame_info_details.transient_env = -1;
  ptr_sbr_data->str_frame_info_details.num_noise_env = 1;

  ptr_sbr_data->num_env_sfac =
      ptr_header_data->pstr_freq_band_data->num_sf_bands[1];

  ptr_sbr_data->del_cod_dir_arr[0] = DTDF_DIR_TIME;

  if (ptr_sbr_data->coupling_mode == COUPLING_BAL) {
    target = SBR_ENERGY_PAN_OFFSET;
  } else {
    target = 0;
  }

  step = 1;

  if (ptr_header_data->amp_res - SBR_AMPLITUDE_RESOLUTION_1_5 == 0) {
    target = (target << 1);
    step = (step << 1);
  }

  for (i = 0; i < ptr_sbr_data->num_env_sfac; i++) {
    if (ptr_prev_data->sfb_nrg_prev[i] > target)
      ptr_sbr_data->int_env_sf_arr[i] = -(step);
    else
      ptr_sbr_data->int_env_sf_arr[i] = step;
  }

  ptr_sbr_data->del_cod_dir_noise_arr[0] = DTDF_DIR_TIME;

  memset(ptr_sbr_data->int_noise_floor, 0,
         sizeof(ptr_sbr_data->int_noise_floor));

  memset(ptr_sbr_data->add_harmonics, 0, sizeof(FLAG) * MAX_FREQ_COEFFS);
}

static WORD16 ixheaacd_find_closest_entry(WORD32 goal_sb, WORD16 *f_master_tbl,
                                          WORD16 num_mf_bands,
                                          WORD16 direction) {
  WORD32 index;

  if (goal_sb <= f_master_tbl[0]) return f_master_tbl[0];

  if (goal_sb >= f_master_tbl[num_mf_bands]) return f_master_tbl[num_mf_bands];

  if (direction) {
    index = 0;
    while (f_master_tbl[index] < goal_sb) {
      index++;
    }
  } else {
    index = num_mf_bands;
    while (f_master_tbl[index] > goal_sb) {
      index--;
    }
  }

  return f_master_tbl[index];
}

WORD32 ixheaacd_reset_hf_generator(ia_sbr_hf_generator_struct *ptr_hf_gen_str,
                                   ia_sbr_header_data_struct *ptr_header_data,
                                   WORD audio_object_type) {
  WORD32 patch, sb;
  WORD32 temp;
  WORD16 *ptr_noise_freq_tbl;
  WORD32 num_nf_bands;

  ia_transposer_settings_struct *pstr_transposer_settings =
      ptr_hf_gen_str->pstr_settings;
  ia_patch_param_struct *p_str_patch_param =
      pstr_transposer_settings->str_patch_param;

  WORD32 sub_band_start = ptr_header_data->pstr_freq_band_data->sub_band_start;
  WORD16 *f_master_tbl = ptr_header_data->pstr_freq_band_data->f_master_tbl;
  WORD16 num_mf_bands = ptr_header_data->pstr_freq_band_data->num_mf_bands;
  WORD16 usb = ptr_header_data->pstr_freq_band_data->sub_band_end;

  WORD32 src_start_band;
  WORD32 patch_stride;
  WORD32 num_bands_in_patch;

  WORD32 lsb = f_master_tbl[0];
  WORD16 xover_offset = sub_band_start - lsb;

  WORD16 goal_sb, flag_break_1 = 0;
  WORD32 fs = ptr_header_data->out_sampling_freq;

  if (lsb < (SHIFT_START_SB + 4)) {
    return (1);
  }
  switch (fs) {
    case 16000:
    case 22050:
    case 24000:
    case 32000:
      goal_sb = 64;
      break;
    case 44100:
      goal_sb = 46;
      break;
    case 48000:
      goal_sb = 43;
      break;
    case 64000:
      goal_sb = 32;
      break;
    case 88200:
      goal_sb = 23;
      break;
    case 96000:
      goal_sb = 21;
      break;
    default:
      return (0);
  }

  goal_sb = ixheaacd_find_closest_entry(goal_sb, f_master_tbl, num_mf_bands, 1);
  if (audio_object_type != AOT_ER_AAC_ELD &&
      audio_object_type != AOT_ER_AAC_LD) {
    if (ixheaac_abs16_sat((WORD16)(goal_sb - usb)) < 4) {
      goal_sb = usb;
    }
  }

  src_start_band = SHIFT_START_SB + xover_offset;
  sb = (lsb + xover_offset);

  patch = 0;

  if ((goal_sb < sb) && (lsb > src_start_band)) {
    return -1;
  }

  while (((sb - usb) < 0) && (patch < MAX_NUM_PATCHES)) {
    ia_patch_param_struct *ptr_loc_patch_param = &p_str_patch_param[patch];
    WORD16 abs_sb, flag_break = 0;
    ptr_loc_patch_param->guard_start_band = sb;
    sb = (sb + GUARDBANDS);
    ptr_loc_patch_param->dst_start_band = sb;

    num_bands_in_patch = (goal_sb - sb);
    if ((num_bands_in_patch <= 0) &&
        ((num_bands_in_patch - (lsb - src_start_band)) < 0)) {
      flag_break = 1;
    }
    if ((num_bands_in_patch - (lsb - src_start_band)) >= 0) {
      patch_stride = sb - src_start_band;
      patch_stride = (WORD16)(patch_stride & ~1);
      num_bands_in_patch = (lsb - (sb - patch_stride));
      num_bands_in_patch = ixheaacd_find_closest_entry(
          sb + num_bands_in_patch, f_master_tbl, num_mf_bands, 0);
      num_bands_in_patch -= sb;
    }

    patch_stride = ((num_bands_in_patch + sb) - lsb);
    patch_stride = (WORD16)((patch_stride + 1) & ~1);

    if (num_bands_in_patch > 0) {
      ptr_loc_patch_param->src_start_band = (sb - patch_stride);
      ptr_loc_patch_param->dst_end_band = patch_stride;
      ptr_loc_patch_param->num_bands_in_patch = num_bands_in_patch;
      ptr_loc_patch_param->src_end_band =
          (ptr_loc_patch_param->src_start_band + num_bands_in_patch);

      sb = (sb + ptr_loc_patch_param->num_bands_in_patch);
      patch++;
    }

    src_start_band = SHIFT_START_SB;
    abs_sb = ixheaac_abs16_sat((WORD16)((sb - goal_sb))) - 3;

    if (num_bands_in_patch <= 0 && flag_break_1 == 1) {
      break;
    }

    if (abs_sb < 0) {
      goal_sb = usb;
    } else {
      if (flag_break == 1) break;
    }

    if (num_bands_in_patch <= 0) {
      flag_break_1 = 1;
    } else {
      flag_break_1 = 0;
    }
  }

  patch--;

  if ((patch > 0) && (p_str_patch_param[patch].num_bands_in_patch < 3)) {
    patch--;
    sb = p_str_patch_param[patch].dst_start_band +
         p_str_patch_param[patch].num_bands_in_patch;
  }

  if (patch >= MAX_NUM_PATCHES) {
    return -1;
  }

  pstr_transposer_settings->num_patches = patch + 1;

  temp = 0;

  for (patch = 0; patch < pstr_transposer_settings->num_patches; patch++) {
    sb = ixheaac_min32(sb, p_str_patch_param[patch].src_start_band);
    temp = ixheaac_max32(temp, p_str_patch_param[patch].src_end_band);
  }

  if (sb > temp) return IA_FATAL_ERROR;

  pstr_transposer_settings->start_patch = sb;
  pstr_transposer_settings->stop_patch = temp;

  ptr_noise_freq_tbl =
      ptr_header_data->pstr_freq_band_data->freq_band_tbl_noise;
  num_nf_bands = ptr_header_data->pstr_freq_band_data->num_nf_bands;

  memcpy(&pstr_transposer_settings->bw_borders[0], &ptr_noise_freq_tbl[1],
         sizeof(WORD16) * num_nf_bands);

  memset(ptr_hf_gen_str->bw_array_prev, 0, sizeof(WORD32) * MAX_NUM_PATCHES);

  return 0;
}
VOID ixheaacd_rescale_x_overlap(
    ia_sbr_dec_struct *ptr_sbr_dec, ia_sbr_header_data_struct *ptr_header_data,
    ia_sbr_frame_info_data_struct *ptr_frame_data,
    ia_sbr_prev_frame_data_struct *ptr_frame_data_prev,
    WORD32 **pp_overlap_buffer_real, WORD32 **pp_overlap_buffer_imag,
    FLAG low_pow_flag) {
  WORD32 k, l;
  WORD32 start_band, end_band;
  WORD32 target_lsb, target_usb;
  WORD32 source_scale, target_scale, delta_scale, reserve;

  WORD32 old_lsb = ptr_frame_data_prev->max_qmf_subband_aac;
  WORD32 start_slot =
      (ptr_header_data->time_step *
       (ptr_frame_data_prev->end_position - ptr_header_data->num_time_slots));
  WORD32 new_lsb = ptr_frame_data->max_qmf_subband_aac;

  ptr_sbr_dec->str_codec_qmf_bank.usb = new_lsb;
  ptr_sbr_dec->str_synthesis_qmf_bank.lsb = new_lsb;

  start_band = ixheaac_min32(old_lsb, new_lsb);
  end_band = ixheaac_max32(old_lsb, new_lsb);

  if (new_lsb != old_lsb && old_lsb > 0) {
    for (l = start_slot; l < 6; l++) {
      for (k = old_lsb; k < new_lsb; k++) {
        pp_overlap_buffer_real[l][k] = 0L;

        if (!low_pow_flag) {
          pp_overlap_buffer_imag[l][k] = 0L;
        }
      }
    }

    if (new_lsb > old_lsb) {
      source_scale = ptr_sbr_dec->str_sbr_scale_fact.ov_hb_scale;
      target_scale = ptr_sbr_dec->str_sbr_scale_fact.ov_lb_scale;
      target_lsb = 0;
      target_usb = old_lsb;
    } else {
      source_scale = ptr_sbr_dec->str_sbr_scale_fact.ov_lb_scale;
      target_scale = ptr_sbr_dec->str_sbr_scale_fact.ov_hb_scale;
      target_lsb = old_lsb;
      target_usb = ptr_sbr_dec->str_synthesis_qmf_bank.usb;
    }

    reserve = (*ixheaacd_ixheaacd_expsubbandsamples)(
        pp_overlap_buffer_real, pp_overlap_buffer_imag, start_band, end_band, 0,
        start_slot, low_pow_flag);

    (*ixheaacd_adjust_scale)(pp_overlap_buffer_real, pp_overlap_buffer_imag,
                             start_band, end_band, 0, start_slot, reserve,
                             low_pow_flag);

    source_scale += reserve;

    delta_scale = (target_scale - source_scale);

    if (delta_scale > 0) {
      delta_scale = -(delta_scale);
      start_band = target_lsb;
      end_band = target_usb;

      if (new_lsb > old_lsb) {
        ptr_sbr_dec->str_sbr_scale_fact.ov_lb_scale = source_scale;
      } else {
        ptr_sbr_dec->str_sbr_scale_fact.ov_hb_scale = source_scale;
      }
    }

    (*ixheaacd_adjust_scale)(pp_overlap_buffer_real, pp_overlap_buffer_imag,
                             start_band, end_band, 0, start_slot, delta_scale,
                             low_pow_flag);
  }
}

VOID ixheaacd_map_sineflags(WORD16 *freq_band_table, WORD16 num_sf_bands,
                            FLAG *add_harmonics, WORD8 *harm_flags_prev,
                            WORD16 transient_env, WORD8 *sine_mapped)

{
  WORD32 qmfband2, li, ui, i;
  WORD32 low_subband_sec;
  WORD32 oldflags;

  low_subband_sec = (freq_band_table[0] << 1);

  memset(sine_mapped, MAX_ENVELOPES, sizeof(WORD8) * MAX_FREQ_COEFFS);

  for (i = (num_sf_bands - 1); i >= 0; i--) {
    oldflags = *harm_flags_prev;
    *harm_flags_prev++ = add_harmonics[i];

    if (add_harmonics[i]) {
      li = freq_band_table[i];

      ui = freq_band_table[i + 1];

      qmfband2 = ((ui + li) - low_subband_sec) >> 1;

      if (oldflags)
        sine_mapped[qmfband2] = 0;
      else
        sine_mapped[qmfband2] = (WORD8)transient_env;
    }
  }
}

VOID ixheaacd_map_34_params_to_20(WORD16 *params) {
  params[0] = ixheaacd_divideby3(params[0] + params[0] + params[1]);
  params[1] = ixheaacd_divideby3(params[1] + params[2] + params[2]);
  params[2] = ixheaacd_divideby3(params[3] + params[3] + params[4]);
  params[3] = ixheaacd_divideby3(params[4] + params[5] + params[5]);
  params[4] = ixheaacd_divideby2(params[6] + params[7]);
  params[5] = ixheaacd_divideby2(params[8] + params[9]);
  params[6] = params[10];
  params[7] = params[11];
  params[8] = ixheaacd_divideby2(params[12] + params[13]);
  params[9] = ixheaacd_divideby2(params[14] + params[15]);
  params[10] = params[16];
  params[11] = params[17];
  params[12] = params[18];
  params[13] = params[19];
  params[14] = ixheaacd_divideby2(params[20] + params[21]);
  params[15] = ixheaacd_divideby2(params[22] + params[23]);
  params[16] = ixheaacd_divideby2(params[24] + params[25]);
  params[17] = ixheaacd_divideby2(params[26] + params[27]);
  params[18] = ixheaacd_divideby2(
      ixheaacd_divideby2(params[28] + params[29] + params[30] + params[31]));
  params[19] = ixheaacd_divideby2(params[32] + params[33]);
}

extern const WORD16 ixheaacd_num_bands[3];

IA_ERRORCODE ixheaacd_read_ps_data(ia_ps_dec_struct *ptr_ps_dec, ia_bit_buf_struct *it_bit_buff,
                                   WORD16 num_bits_left, ia_ps_tables_struct *ps_tables_ptr)
{
  WORD b, e, temp;
  const WORD16 num_env_tab[4] = {0, 1, 2, 4};
  WORD cnt_bits;
  ia_huffman_data_type huffman_table, huffman_df_table, huffman_dt_table;
  FLAG enable_ps_header;

  if (!ptr_ps_dec) {
    return 0;
  }

  cnt_bits = it_bit_buff->cnt_bits;

  enable_ps_header = ixheaacd_read_bits_buf(it_bit_buff, 1);

  if (enable_ps_header) {
    ptr_ps_dec->enable_iid = ixheaacd_read_bits_buf(it_bit_buff, 1);
    if (ptr_ps_dec->enable_iid) {
      ptr_ps_dec->iid_mode = ixheaacd_read_bits_buf(it_bit_buff, 3);
    }

    if (ptr_ps_dec->iid_mode > 2) {
      ptr_ps_dec->iid_quant = 1;
      ptr_ps_dec->iid_mode -= 3;
    } else {
      ptr_ps_dec->iid_quant = 0;
    }

    ptr_ps_dec->enable_icc = ixheaacd_read_bits_buf(it_bit_buff, 1);
    if (ptr_ps_dec->enable_icc) {
      ptr_ps_dec->icc_mode = ixheaacd_read_bits_buf(it_bit_buff, 3);
    }

    ptr_ps_dec->enable_ext = ixheaacd_read_bits_buf(it_bit_buff, 1);

    if (ptr_ps_dec->icc_mode > 2) {
      ptr_ps_dec->icc_mode -= 3;
      ptr_ps_dec->use_pca_rot_flg = 1;
    } else {
      ptr_ps_dec->use_pca_rot_flg = 0;
    }
    ptr_ps_dec->freq_res_ipd = ptr_ps_dec->iid_mode;
    if (ptr_ps_dec->freq_res_ipd > 2) {
      return IA_FATAL_ERROR;
    }
  }

  ptr_ps_dec->use_34_st_bands = 0;
  ptr_ps_dec->use_pca_rot_flg = 0;

  if ((ptr_ps_dec->enable_iid && ptr_ps_dec->iid_mode > 2) ||
      (ptr_ps_dec->enable_icc && ptr_ps_dec->icc_mode > 2)) {
    ptr_ps_dec->ps_data_present = 0;

    num_bits_left -= (cnt_bits - it_bit_buff->cnt_bits);

    while (num_bits_left > 8) {
      ixheaacd_read_bits_buf(it_bit_buff, 8);
      num_bits_left -= 8;
    }
    if (num_bits_left >= 0) {
      ixheaacd_read_bits_buf(it_bit_buff, num_bits_left);
    }

    return (cnt_bits - it_bit_buff->cnt_bits);
  }

  ptr_ps_dec->frame_class = (FLAG)ixheaacd_read_bits_buf(it_bit_buff, 1);

  temp = ixheaacd_read_bits_buf(it_bit_buff, 2);

  if (ptr_ps_dec->frame_class == 0) {
    ptr_ps_dec->num_env = num_env_tab[temp];
  } else {
    ptr_ps_dec->num_env = (((1 + temp) << 8) >> 8);

    for (e = 1; e < ptr_ps_dec->num_env + 1; e++) {
      ptr_ps_dec->border_position[e] =
          (((ixheaacd_read_bits_buf(it_bit_buff, 5) + 1) << 8) >> 8);
    }
  }

  if (ptr_ps_dec->enable_iid) {
    if (ptr_ps_dec->iid_quant) {
      huffman_df_table = (ia_huffman_data_type)&ps_tables_ptr->huff_iid_df_fine;
      huffman_dt_table = (ia_huffman_data_type)&ps_tables_ptr->huff_iid_dt_fine;
    } else {
      huffman_df_table = (ia_huffman_data_type)&ps_tables_ptr->huff_iid_df;
      huffman_dt_table = (ia_huffman_data_type)&ps_tables_ptr->huff_iid_dt;
    }

    for (e = 0; e < ptr_ps_dec->num_env; e++) {
      ptr_ps_dec->iid_dt[e] = (FLAG)ixheaacd_read_bits_buf(it_bit_buff, 1);

      if (ptr_ps_dec->iid_dt[e]) {
        huffman_table = huffman_dt_table;
      } else {
        huffman_table = huffman_df_table;
      }

      for (b = 0; b < ixheaacd_num_bands[ptr_ps_dec->iid_mode]; b++) {
        ptr_ps_dec->iid_par_table[e][b] =
            ixheaacd_ssc_huff_dec(huffman_table, it_bit_buff);
      }
    }
  }

  if (ptr_ps_dec->enable_icc) {
    huffman_df_table = (ia_huffman_data_type)&ps_tables_ptr->huff_icc_df;
    huffman_dt_table = (ia_huffman_data_type)&ps_tables_ptr->huff_icc_dt;

    for (e = 0; e < ptr_ps_dec->num_env; e++) {
      ptr_ps_dec->icc_dt[e] = ixheaacd_read_bits_buf(it_bit_buff, 1);

      if (ptr_ps_dec->icc_dt[e]) {
        huffman_table = huffman_dt_table;
      } else {
        huffman_table = huffman_df_table;
      }

      for (b = 0; b < ixheaacd_num_bands[ptr_ps_dec->icc_mode]; b++) {
        ptr_ps_dec->icc_par_table[e][b] = ixheaacd_ssc_huff_dec(huffman_table, it_bit_buff);
      }
    }
  }

  if (ptr_ps_dec->enable_ext) {
    WORD32 cnt;
    if (it_bit_buff->cnt_bits < 4)
      cnt = ixheaacd_read_bits_buf(it_bit_buff, it_bit_buff->cnt_bits);
    else
      cnt = ixheaacd_read_bits_buf(it_bit_buff, 4);

    if (cnt == 15) {
      cnt += ixheaacd_read_bits_buf(it_bit_buff, 8);
    }
    while (cnt--) {
      ixheaacd_read_bits_buf(it_bit_buff, 8);
    }
  }

  ptr_ps_dec->ps_data_present = 1;

  return (cnt_bits - it_bit_buff->cnt_bits);
}

VOID ixheaacd_invfilt_level_emphasis(ia_sbr_hf_generator_struct *ptr_hf_gen_str,
                                     WORD32 num_if_bands, WORD32 *inv_filt_mode,
                                     WORD32 *inv_filt_mode_prev,
                                     WORD32 *bw_array) {
  WORD32 i;
  WORD32 accu;
  WORD16 w1, w2;

  for (i = 0; i < num_if_bands; i++) {
    bw_array[i] =
        ixheaacd_inew_bw_table[inv_filt_mode_prev[i]][inv_filt_mode[i]];

    if (bw_array[i] < ptr_hf_gen_str->bw_array_prev[i]) {
      w1 = 0x6000;
      w2 = 0x2000;
    } else {
      w1 = 0x7400;
      w2 = 0x0c00;
    }
    accu = ixheaac_add32(
        ixheaac_mult32x16in32_shl(bw_array[i], w1),
        ixheaac_mult32x16in32_shl(ptr_hf_gen_str->bw_array_prev[i], w2));

    if (accu < 0x02000000) {
      accu = 0;
    }

    if (accu >= 0x7f800000) {
      accu = 0x7f800000;
    }
    bw_array[i] = accu;
  }
}

typedef struct {
  FLOAT32 phi_0_1_real;
  FLOAT32 phi_0_1_imag;
  FLOAT32 phi_0_2_real;
  FLOAT32 phi_0_2_imag;
  FLOAT32 phi_1_1;
  FLOAT32 phi_1_2_real;
  FLOAT32 phi_1_2_imag;
  FLOAT32 phi_2_2;
  FLOAT32 det;
} ia_auto_corr_ele_struct;

static VOID ixheaacd_esbr_calc_co_variance(
    ia_auto_corr_ele_struct *pstr_auto_corr, FLOAT32 vec_x_real[][64],
    FLOAT32 vec_x_imag[][64], WORD32 bd, WORD32 len) {
  WORD32 j, jminus1, jminus2;

  memset(pstr_auto_corr, 0, sizeof(ia_auto_corr_ele_struct));

  for (j = 0; j < len; j++) {
    jminus1 = j - 1;
    jminus2 = jminus1 - 1;

    pstr_auto_corr->phi_0_1_real +=
        vec_x_real[j][bd] * vec_x_real[jminus1][bd] +
        vec_x_imag[j][bd] * vec_x_imag[jminus1][bd];

    pstr_auto_corr->phi_0_1_imag +=
        vec_x_imag[j][bd] * vec_x_real[jminus1][bd] -
        vec_x_real[j][bd] * vec_x_imag[jminus1][bd];

    pstr_auto_corr->phi_0_2_real +=
        vec_x_real[j][bd] * vec_x_real[jminus2][bd] +
        vec_x_imag[j][bd] * vec_x_imag[jminus2][bd];

    pstr_auto_corr->phi_0_2_imag +=
        vec_x_imag[j][bd] * vec_x_real[jminus2][bd] -
        vec_x_real[j][bd] * vec_x_imag[jminus2][bd];

    pstr_auto_corr->phi_1_1 +=
        vec_x_real[jminus1][bd] * vec_x_real[jminus1][bd] +
        vec_x_imag[jminus1][bd] * vec_x_imag[jminus1][bd];

    pstr_auto_corr->phi_1_2_real +=
        vec_x_real[jminus1][bd] * vec_x_real[jminus2][bd] +
        vec_x_imag[jminus1][bd] * vec_x_imag[jminus2][bd];

    pstr_auto_corr->phi_1_2_imag +=
        vec_x_imag[jminus1][bd] * vec_x_real[jminus2][bd] -
        vec_x_real[jminus1][bd] * vec_x_imag[jminus2][bd];

    pstr_auto_corr->phi_2_2 +=
        vec_x_real[jminus2][bd] * vec_x_real[jminus2][bd] +
        vec_x_imag[jminus2][bd] * vec_x_imag[jminus2][bd];
  }

  pstr_auto_corr->det =
      pstr_auto_corr->phi_1_1 * pstr_auto_corr->phi_2_2 -
      (pstr_auto_corr->phi_1_2_real * pstr_auto_corr->phi_1_2_real +
       pstr_auto_corr->phi_1_2_imag * pstr_auto_corr->phi_1_2_imag) *
          SBR_HF_RELAXATION_PARAM;
}

static void ixheaacd_esbr_chirp_fac_calc(WORD32 *inv_filt_mode,
                                         WORD32 *inv_filt_mode_prev,
                                         WORD32 num_if_bands, FLOAT32 *bw_array,
                                         FLOAT32 *bw_array_prev) {
  WORD32 i;

  for (i = 0; i < num_if_bands; i++) {
    bw_array[i] =
        ixheaacd_new_bw_table[inv_filt_mode_prev[i]][inv_filt_mode[i]];

    if (bw_array[i] < bw_array_prev[i])
      bw_array[i] = 0.75000f * bw_array[i] + 0.25000f * bw_array_prev[i];
    else
      bw_array[i] = 0.90625f * bw_array[i] + 0.09375f * bw_array_prev[i];

    if (bw_array[i] < 0.015625) bw_array[i] = 0;
  }
}

static void ixheaacd_gausssolve(WORD32 n, FLOAT32 a[][MAXDEG + 1], FLOAT32 b[],
                                FLOAT32 y[]) {
  WORD32 i, j, k, imax;
  FLOAT32 v;

  for (i = 0; i < n; i++) {
    imax = i;
    for (k = i + 1; k < n; k++) {
      if (fabs(a[k][i]) > fabs(a[imax][i])) {
        imax = k;
      }
    }
    if (imax != i) {
      v = b[imax];
      b[imax] = b[i];
      b[i] = v;
      for (j = i; j < n; j++) {
        v = a[imax][j];
        a[imax][j] = a[i][j];
        a[i][j] = v;
      }
    }

    v = a[i][i];

    b[i] /= v;
    for (j = i; j < n; j++) {
      a[i][j] /= v;
    }

    for (k = i + 1; k < n; k++) {
      v = a[k][i];
      b[k] -= v * b[i];
      for (j = i + 1; j < n; j++) {
        a[k][j] -= v * a[i][j];
      }
    }
  }

  for (i = n - 1; i >= 0; i--) {
    y[i] = b[i];
    for (j = i + 1; j < n; j++) {
      y[i] -= a[i][j] * y[j];
    }
  }
}

void ixheaacd_polyfit(WORD32 n, FLOAT32 y[], FLOAT32 p[]) {
  WORD32 i, j, k;
  FLOAT32 a[MAXDEG + 1][MAXDEG + 1];
  FLOAT32 b[MAXDEG + 1];
  FLOAT32 v[2 * MAXDEG + 1];

  for (i = 0; i <= MAXDEG; i++) {
    b[i] = 0.0f;
    for (j = 0; j <= MAXDEG; j++) {
      a[i][j] = 0.0f;
    }
  }

  for (k = 0; k < n; k++) {
    v[0] = 1.0;
    for (i = 1; i <= 2 * MAXDEG; i++) {
      v[i] = k * v[i - 1];
    }

    for (i = 0; i <= MAXDEG; i++) {
      b[i] += v[MAXDEG - i] * y[k];
      for (j = 0; j <= MAXDEG; j++) {
        a[i][j] += v[2 * MAXDEG - i - j];
      }
    }
  }

  ixheaacd_gausssolve(MAXDEG + 1, a, b, p);
}

VOID ixheaacd_pre_processing(FLOAT32 ptr_src_buf_real[][64],
                             FLOAT32 ptr_src_buf_imag[][64],
                             FLOAT32 gain_vector[], WORD32 num_bands,
                             WORD32 start_sample, WORD32 end_sample) {
  WORD32 k, i;
  FLOAT32 poly_coeff[4];
  FLOAT32 mean_enrg = 0;
  FLOAT32 low_env_slope[64];
  FLOAT32 low_env[64] = {0};
  FLOAT32 a0;
  FLOAT32 a1;
  FLOAT32 a2;
  FLOAT32 a3;

  if (num_bands != 0 && end_sample != start_sample) {
    for (k = 0; k < num_bands; k++) {
      FLOAT32 temp = 0;
      for (i = start_sample; i < end_sample; i++) {
        temp += ptr_src_buf_real[i][k] * ptr_src_buf_real[i][k] +
                ptr_src_buf_imag[i][k] * ptr_src_buf_imag[i][k];
      }
      temp /= (end_sample - start_sample);
      low_env[k] = (FLOAT32)(10 * log10(temp + 1));
      mean_enrg = mean_enrg + low_env[k];
    }
    mean_enrg /= num_bands;
  }

  ixheaacd_polyfit(num_bands, low_env, poly_coeff);

  a0 = poly_coeff[0];
  a1 = poly_coeff[1];
  a2 = poly_coeff[2];
  a3 = poly_coeff[3];
  for (k = 0; k < num_bands; k++) {
    FLOAT32 x_low_l = (FLOAT32)k;
    FLOAT32 low_env_slope_l = a3;
    low_env_slope_l = low_env_slope_l + a2 * x_low_l;

    x_low_l = x_low_l * x_low_l;
    low_env_slope_l = low_env_slope_l + a1 * x_low_l;

    x_low_l = x_low_l * (FLOAT32)k;
    low_env_slope_l = low_env_slope_l + a0 * x_low_l;

    low_env_slope[k] = low_env_slope_l;
  }

  for (i = 0; i < num_bands; i++) {
    gain_vector[i] = (FLOAT32)pow(10, (mean_enrg - low_env_slope[i]) / 20.0f);
  }
}

WORD32 ixheaacd_generate_hf(FLOAT32 ptr_src_buf_real[][64], FLOAT32 ptr_src_buf_imag[][64],
                            FLOAT32 ptr_ph_vocod_buf_real[][64],
                            FLOAT32 ptr_ph_vocod_buf_imag[][64], FLOAT32 ptr_dst_buf_real[][64],
                            FLOAT32 ptr_dst_buf_imag[][64],
                            ia_sbr_frame_info_data_struct *ptr_frame_data,
                            ia_sbr_header_data_struct *ptr_header_data, WORD32 ldmps_present,
                            WORD32 time_slots, WORD32 ec_flag) {
  WORD32 bw_index, i, k, k2, patch = 0;
  WORD32 co_var_len;
  WORD32 start_sample, end_sample, goal_sb;
  WORD32 sb, source_start_band, patch_stride, num_bands_in_patch;
  WORD32 hbe_flag = ptr_header_data->hbe_flag;
  FLOAT32 a0r, a0i, a1r, a1i;
  FLOAT32 bw_array[MAX_NUM_PATCHES] = {0};

  ia_auto_corr_ele_struct str_auto_corr;

  WORD16 *ptr_invf_band_tbl =
      &ptr_header_data->pstr_freq_band_data
           ->freq_band_tbl_noise[1];  // offest 1 used as base address of
                                      // ptr_invf_band_tbl
  WORD32 num_if_bands = ptr_header_data->pstr_freq_band_data->num_nf_bands;
  WORD32 sub_band_start = ptr_header_data->pstr_freq_band_data->sub_band_start;
  WORD16 *f_master_tbl = ptr_header_data->pstr_freq_band_data->f_master_tbl;
  WORD32 num_mf_bands = ptr_header_data->pstr_freq_band_data->num_mf_bands;
  WORD32 *inv_filt_mode = ptr_frame_data->sbr_invf_mode;
  WORD32 *inv_filt_mode_prev = ptr_frame_data->sbr_invf_mode_prev;
  WORD32 sbr_patching_mode = ptr_frame_data->sbr_patching_mode;
  ia_frame_info_struct *p_frame_info = &ptr_frame_data->str_frame_info_details;
  WORD32 pre_proc_flag = ptr_header_data->pre_proc_flag;
  WORD32 is_usf_4 = ptr_header_data->is_usf_4;
  WORD32 fs = ptr_header_data->out_sampling_freq;
  WORD32 cov_count;
  WORD32 lsb = f_master_tbl[0];
  WORD32 usb = f_master_tbl[num_mf_bands];
  WORD32 xover_offset = sub_band_start - f_master_tbl[0];

  FLOAT32 bw = 0.0f;
  FLOAT32 fac = 0.0f;

  FLOAT32 gain;
  FLOAT32 gain_vector[64];

  WORD32 slope_length = 0;
  WORD32 first_slot_offset = p_frame_info->border_vec[0];
  WORD32 end_slot_offs = 0;

  FLOAT32 *bw_array_prev = ptr_frame_data->bw_array_prev;

  end_slot_offs = p_frame_info->border_vec[p_frame_info->num_env] - 16;

  if (ldmps_present == 1)
    end_slot_offs =
        p_frame_info->border_vec[p_frame_info->num_env] - time_slots;

  if (is_usf_4) {
    start_sample = first_slot_offset * 4;
    end_sample = 64 + end_slot_offs * 4;
    co_var_len = 76;
  } else {
    start_sample = first_slot_offset * 2;
    end_sample = 32 + end_slot_offs * 2;
    co_var_len = 38;
  }

  if (ldmps_present == 1) {
    start_sample = 0;
    end_sample = time_slots;
    co_var_len = time_slots;
  }

  if (pre_proc_flag) {
    ixheaacd_pre_processing(ptr_src_buf_real, ptr_src_buf_imag, gain_vector,
                            f_master_tbl[0], start_sample, end_sample);
  }

  ixheaacd_esbr_chirp_fac_calc(inv_filt_mode, inv_filt_mode_prev, num_if_bands,
                               bw_array, bw_array_prev);

  for (i = start_sample; i < end_sample; i++) {
    memset(ptr_dst_buf_real[i] + usb, 0, (64 - usb) * sizeof(FLOAT32));
    memset(ptr_dst_buf_imag[i] + usb, 0, (64 - usb) * sizeof(FLOAT32));
  }

  if (sbr_patching_mode || !hbe_flag) {
    WORD32 flag_break = 0;
    FLOAT32 alpha_real[64][2] = {{0}}, alpha_imag[64][2] = {{0}};
    if (ptr_frame_data->mps_sbr_flag) {
      cov_count = (f_master_tbl[0] < ptr_frame_data->cov_count)
                      ? f_master_tbl[0]
                      : ptr_frame_data->cov_count;
    } else {
      cov_count = f_master_tbl[0];
    }

    for (k = 1; k < cov_count; k++) {
      ixheaacd_esbr_calc_co_variance(&str_auto_corr, &ptr_src_buf_real[0],
                                     &ptr_src_buf_imag[0], k, co_var_len);
      if (str_auto_corr.det == 0.0f) {
        alpha_real[k][1] = alpha_imag[k][1] = 0;
      } else {
        fac = 1.0f / str_auto_corr.det;
        alpha_real[k][1] =
            (str_auto_corr.phi_0_1_real * str_auto_corr.phi_1_2_real -
             str_auto_corr.phi_0_1_imag * str_auto_corr.phi_1_2_imag -
             str_auto_corr.phi_0_2_real * str_auto_corr.phi_1_1) *
            fac;
        alpha_imag[k][1] =
            (str_auto_corr.phi_0_1_imag * str_auto_corr.phi_1_2_real +
             str_auto_corr.phi_0_1_real * str_auto_corr.phi_1_2_imag -
             str_auto_corr.phi_0_2_imag * str_auto_corr.phi_1_1) *
            fac;
      }

      if (str_auto_corr.phi_1_1 == 0) {
        alpha_real[k][0] = alpha_imag[k][0] = 0;
      } else {
        fac = 1.0f / str_auto_corr.phi_1_1;
        alpha_real[k][0] = -(str_auto_corr.phi_0_1_real +
                             alpha_real[k][1] * str_auto_corr.phi_1_2_real +
                             alpha_imag[k][1] * str_auto_corr.phi_1_2_imag) *
                           fac;
        alpha_imag[k][0] = -(str_auto_corr.phi_0_1_imag +
                             alpha_imag[k][1] * str_auto_corr.phi_1_2_real -
                             alpha_real[k][1] * str_auto_corr.phi_1_2_imag) *
                           fac;
      }

      if ((alpha_real[k][0] * alpha_real[k][0] +
               alpha_imag[k][0] * alpha_imag[k][0] >=
           16.0f) ||
          (alpha_real[k][1] * alpha_real[k][1] +
               alpha_imag[k][1] * alpha_imag[k][1] >=
           16.0f)) {
        alpha_real[k][0] = 0.0f;
        alpha_imag[k][0] = 0.0f;
        alpha_real[k][1] = 0.0f;
        alpha_imag[k][1] = 0.0f;
      }
    }

    goal_sb = (WORD32)(2.048e6f / fs + 0.5f);
    {
      WORD32 index;
      if (goal_sb < f_master_tbl[num_mf_bands]) {
        for (index = 0; (f_master_tbl[index] < goal_sb); index++)
          ;
        goal_sb = f_master_tbl[index];
      } else {
        goal_sb = f_master_tbl[num_mf_bands];
      }
    }

    source_start_band = xover_offset + 1;
    sb = lsb + xover_offset;

    patch = 0;
    while (sb < usb) {
      if (MAX_NUM_PATCHES <= patch) {
        if (ec_flag) {
          break;
        } else {
          return -1;
        }
      }
      ptr_frame_data->patch_param.start_subband[patch] = sb;
      num_bands_in_patch = goal_sb - sb;

      if (num_bands_in_patch >= lsb - source_start_band) {
        patch_stride = sb - source_start_band;
        patch_stride = patch_stride & ~1;
        num_bands_in_patch = lsb - (sb - patch_stride);
        num_bands_in_patch =
            ixheaacd_find_closest_entry(sb + num_bands_in_patch, f_master_tbl,
                                        (WORD16)(num_mf_bands), 0) -
            (WORD32)(sb);
      }

      patch_stride = num_bands_in_patch + sb - lsb;
      patch_stride = (patch_stride + 1) & ~1;

      source_start_band = 1;

      if (goal_sb - (sb + num_bands_in_patch) < 3) {
        goal_sb = usb;
      }

      if ((num_bands_in_patch < 3) && (patch > 0) &&
          (sb + num_bands_in_patch == usb)) {
        for (i = start_sample + slope_length; i < end_sample + slope_length;
             i++) {
          for (k2 = sb; k2 < sb + num_bands_in_patch; k2++) {
            ptr_dst_buf_real[i][k2] = 0.0f;
            ptr_dst_buf_imag[i][k2] = 0.0f;
          }
        }

        break;
      }

      if (num_bands_in_patch < 0 && flag_break == 1) {
        break;
      }

      if (num_bands_in_patch < 0) {
        flag_break = 1;
        continue;
      } else {
        flag_break = 0;
      }

      for (k2 = sb; k2 < sb + num_bands_in_patch; k2++) {
        k = k2 - patch_stride;
        bw_index = 0;
        while (k2 >= ptr_invf_band_tbl[bw_index]) {
          bw_index++;
          if (bw_index >= MAX_NOISE_COEFFS) {
            if (ec_flag) {
              bw_index = MAX_NOISE_COEFFS - 1;
              break;
            } else
              return -1;
          }
        }

        if (bw_index >= MAX_NUM_PATCHES) {
          if (ec_flag)
            bw_index = MAX_NUM_PATCHES - 1;
          else
            return -1;
        }
        bw = bw_array[bw_index];

        a0r = bw * alpha_real[k][0];
        a0i = bw * alpha_imag[k][0];
        bw *= bw;
        a1r = bw * alpha_real[k][1];
        a1i = bw * alpha_imag[k][1];

        if (pre_proc_flag) {
          gain = gain_vector[k];
        } else {
          gain = 1.0f;
        }

        for (i = start_sample + slope_length; i < end_sample + slope_length;
             i++) {
          ptr_dst_buf_real[i][k2] = ptr_src_buf_real[i][k] * gain;

          ptr_dst_buf_imag[i][k2] = ptr_src_buf_imag[i][k] * gain;

          if (bw > 0.0f) {
            ptr_dst_buf_real[i][k2] += (a0r * ptr_src_buf_real[i - 1][k] -
                                        a0i * ptr_src_buf_imag[i - 1][k] +
                                        a1r * ptr_src_buf_real[i - 2][k] -
                                        a1i * ptr_src_buf_imag[i - 2][k]) *
                                       gain;
            ptr_dst_buf_imag[i][k2] += (a0i * ptr_src_buf_real[i - 1][k] +
                                        a0r * ptr_src_buf_imag[i - 1][k] +
                                        a1i * ptr_src_buf_real[i - 2][k] +
                                        a1r * ptr_src_buf_imag[i - 2][k]) *
                                       gain;
          }
        }
      }
      sb += num_bands_in_patch;
      patch++;
    }
  }

  if (NULL != ptr_ph_vocod_buf_real && NULL != ptr_ph_vocod_buf_imag) {
    if (hbe_flag && !sbr_patching_mode) {
      FLOAT32 alpha_real[2], alpha_imag[2];

      bw_index = 0, patch = 1;

      for (k2 = sub_band_start; k2 < f_master_tbl[num_mf_bands]; k2++) {
        ixheaacd_esbr_calc_co_variance(&str_auto_corr, &ptr_ph_vocod_buf_real[0],
                                       &ptr_ph_vocod_buf_imag[0], k2, co_var_len);

        if (str_auto_corr.det == 0.0f) {
          alpha_real[1] = alpha_imag[1] = 0;
        } else {
          fac = 1.0f / str_auto_corr.det;
          alpha_real[1] = (str_auto_corr.phi_0_1_real * str_auto_corr.phi_1_2_real -
                           str_auto_corr.phi_0_1_imag * str_auto_corr.phi_1_2_imag -
                           str_auto_corr.phi_0_2_real * str_auto_corr.phi_1_1) *
                          fac;
          alpha_imag[1] = (str_auto_corr.phi_0_1_imag * str_auto_corr.phi_1_2_real +
                           str_auto_corr.phi_0_1_real * str_auto_corr.phi_1_2_imag -
                           str_auto_corr.phi_0_2_imag * str_auto_corr.phi_1_1) *
                          fac;
        }

        if (str_auto_corr.phi_1_1 == 0) {
          alpha_real[0] = alpha_imag[0] = 0;
        } else {
          fac = 1.0f / str_auto_corr.phi_1_1;
          alpha_real[0] =
              -(str_auto_corr.phi_0_1_real + alpha_real[1] * str_auto_corr.phi_1_2_real +
                alpha_imag[1] * str_auto_corr.phi_1_2_imag) *
              fac;
          alpha_imag[0] =
              -(str_auto_corr.phi_0_1_imag + alpha_imag[1] * str_auto_corr.phi_1_2_real -
                alpha_real[1] * str_auto_corr.phi_1_2_imag) *
              fac;
        }

        if (alpha_real[0] * alpha_real[0] + alpha_imag[0] * alpha_imag[0] >= 16.0f ||
            alpha_real[1] * alpha_real[1] + alpha_imag[1] * alpha_imag[1] >= 16.0f) {
          alpha_real[0] = 0.0f;
          alpha_imag[0] = 0.0f;
          alpha_real[1] = 0.0f;
          alpha_imag[1] = 0.0f;
        }

        while (k2 >= ptr_invf_band_tbl[bw_index]) {
          bw_index++;
          if (bw_index >= MAX_NOISE_COEFFS) {
            if (ec_flag) {
              bw_index = MAX_NOISE_COEFFS - 1;
              break;
            } else
              return -1;
          }
        }

        if (bw_index >= MAX_NUM_PATCHES) {
          if (ec_flag)
            bw_index = MAX_NUM_PATCHES - 1;
          else
            return -1;
        }
        bw = bw_array[bw_index];

        a0r = bw * alpha_real[0];
        a0i = bw * alpha_imag[0];
        bw *= bw;
        a1r = bw * alpha_real[1];
        a1i = bw * alpha_imag[1];

        if (bw > 0.0f) {
          for (i = start_sample; i < end_sample; i++) {
            FLOAT32 real1, imag1, real2, imag2, realTarget, imag_target;

            realTarget = ptr_ph_vocod_buf_real[i][k2];
            imag_target = ptr_ph_vocod_buf_imag[i][k2];
            real1 = ptr_ph_vocod_buf_real[i - 1][k2];
            imag1 = ptr_ph_vocod_buf_imag[i - 1][k2];
            real2 = ptr_ph_vocod_buf_real[i - 2][k2];
            imag2 = ptr_ph_vocod_buf_imag[i - 2][k2];
            realTarget += ((a0r * real1 - a0i * imag1) + (a1r * real2 - a1i * imag2));
            imag_target += ((a0i * real1 + a0r * imag1) + (a1i * real2 + a1r * imag2));

            ptr_dst_buf_real[i][k2] = realTarget;
            ptr_dst_buf_imag[i][k2] = imag_target;
          }
        } else {
          for (i = start_sample; i < end_sample; i++) {
            ptr_dst_buf_real[i][k2] = ptr_ph_vocod_buf_real[i][k2];
            ptr_dst_buf_imag[i][k2] = ptr_ph_vocod_buf_imag[i][k2];
          }
        }
      }
    }
  }
  if ((MAX_NUM_PATCHES + 1) <= patch) {
    if (ec_flag) {
      patch = MAX_NUM_PATCHES;
    } else {
      return -1;
    }
  }
  ptr_frame_data->patch_param.num_patches = patch;
  for (i = 0; i < num_if_bands; i++) {
    bw_array_prev[i] = bw_array[i];
  }
  return 0;
}
