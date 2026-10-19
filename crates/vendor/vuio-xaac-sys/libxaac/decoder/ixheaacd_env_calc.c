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
#include "ixheaacd_sbr_common.h"
#include "ixheaac_type_def.h"

#include "ixheaac_constants.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops16.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaac_basic_ops.h"

#include "ixheaacd_intrinsics.h"
#include "ixheaacd_bitbuffer.h"
#include "ixheaacd_sbrdecsettings.h"
#include "ixheaacd_defines.h"

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
#include "ixheaacd_sbr_scale.h"
#include "ixheaacd_lpp_tran.h"

#include "ixheaacd_env_extr_part.h"
#include "ixheaacd_sbr_rom.h"
#include "ixheaacd_hybrid.h"
#include "ixheaacd_ps_dec.h"
#include "ixheaacd_env_extr.h"

#include "ixheaac_sbr_const.h"
#include "ixheaacd_common_rom.h"
#include "ixheaacd_freq_sca.h"

#include "ixheaacd_basic_funcs.h"
#include "ixheaacd_env_extr.h"

#include "ixheaacd_env_calc.h"
#include "ixheaac_basic_op.h"

#include "ixheaacd_qmf_dec.h"

#include "ixheaacd_pvc_dec.h"
#include "ixheaacd_sbr_dec.h"
#include "ixheaacd_function_selector.h"

#include "ixheaacd_audioobjtypes.h"

#define FACTOR 0x010b0000 * 2

static VOID ixheaacd_alias_reduction(WORD16 *deg_patched, WORD16 *nrg_gain,
                                     WORD16 *nrg_est, WORD8 *alias_red_buf,
                                     WORD32 num_sub_bands,
                                     ixheaacd_misc_tables *pstr_common_tables) {
  WORD32 group, grouping, i, num_groups, k;
  WORD16 f_group_vec[MAX_FREQ_COEFFS], *ptr_f_group_vec;

  grouping = 0;
  i = 0;

  for (k = 0; k < num_sub_bands - 1; k++) {
    if ((deg_patched[k + 1] != 0) && alias_red_buf[k]) {
      if (grouping == 0) {
        f_group_vec[i] = k;
        grouping = 1;
        i++;
      } else {
        if ((f_group_vec[i - 1] + 3) == k) {
          f_group_vec[i] = (k + 1);
          grouping = 0;
          i++;
        }
      }
    } else {
      if (grouping) {
        grouping = 0;
        f_group_vec[i] = k;

        if (alias_red_buf[k]) f_group_vec[i] = k + 1;

        i++;
      }
    }
  }

  if (grouping) {
    f_group_vec[i] = num_sub_bands;
    i++;
  }
  num_groups = (i >> 1);

  ptr_f_group_vec = f_group_vec;

  for (group = num_groups; group != 0; group--) {
    WORD16 nrg_amp_mant;
    WORD16 nrg_amp_exp;
    WORD16 nrgMod_m;
    WORD16 nrgMod_e;
    WORD16 grp_gain_mant;
    WORD16 grp_gain_exp;
    WORD16 compensation_m;
    WORD16 compensation_e;
    WORD32 nrg_mod_mant;
    WORD32 nrg_mod_exp;

    WORD32 start_grp = *ptr_f_group_vec++;
    WORD32 stop_grp = *ptr_f_group_vec++;

    ixheaacd_avggain_calc(nrg_est, nrg_gain, start_grp, stop_grp, &nrg_amp_mant,
                          &nrg_amp_exp, &grp_gain_mant, &grp_gain_exp,
                          pstr_common_tables, 1);

    nrg_mod_mant = 0;
    nrg_mod_exp = 0;
    {
      WORD16 *ptr_nrg_gain_mant = &nrg_gain[2 * start_grp];

      for (k = start_grp; k < stop_grp; k++) {
        WORD32 tmp_mant, tmp_gain_mant, gain_m;
        WORD32 tmp_e, tmp_gain_exp;
        WORD16 one_minus_alpha, alpha = deg_patched[k];

        if (k < (num_sub_bands - 1)) {
          alpha = ixheaac_max16(deg_patched[k + 1], alpha);
        }
        gain_m = (alpha * grp_gain_mant);
        one_minus_alpha = 0x7fff - alpha;

        tmp_gain_mant = *ptr_nrg_gain_mant;
        tmp_gain_exp = *(ptr_nrg_gain_mant + 1);

        {
          WORD32 exp_diff;

          tmp_gain_mant = (one_minus_alpha * tmp_gain_mant) >> 15;

          exp_diff = (grp_gain_exp - tmp_gain_exp);

          if (exp_diff >= 0) {
            tmp_gain_exp = grp_gain_exp;
            tmp_gain_mant = ixheaac_shr32(tmp_gain_mant, exp_diff);

            tmp_gain_mant = (gain_m >> 15) + tmp_gain_mant;

          } else {
            tmp_gain_mant =
                (ixheaac_shr32(gain_m, (15 - exp_diff))) + tmp_gain_mant;
          }
        }
        *ptr_nrg_gain_mant++ = tmp_gain_mant;
        *ptr_nrg_gain_mant++ = tmp_gain_exp;

        tmp_mant = (tmp_gain_mant * (nrg_est[2 * k])) >> 16;
        tmp_e = (tmp_gain_exp + (nrg_est[2 * k + 1]) + 1);

        {
          WORD32 exp_diff;
          exp_diff = tmp_e - nrg_mod_exp;
          if (exp_diff >= 0) {
            nrg_mod_mant = tmp_mant + (ixheaac_shr32(nrg_mod_mant, exp_diff));
            nrg_mod_exp = tmp_e;
          } else {
            exp_diff = -exp_diff;
            nrg_mod_mant = (ixheaac_shr32(tmp_mant, exp_diff)) + nrg_mod_mant;
          }
        }
      }
    }

    {
      WORD32 norm_val;
      norm_val = 16 - ixheaac_pnorm32(nrg_mod_mant);
      if (norm_val > 0) {
        nrg_mod_mant >>= norm_val;
        nrg_mod_exp += norm_val;
      }
    }

    nrgMod_m = (WORD16)nrg_mod_mant;
    nrgMod_e = (WORD16)nrg_mod_exp;

    compensation_e = ixheaacd_fix_mant_div(nrg_amp_mant, nrgMod_m,
                                           &compensation_m, pstr_common_tables);
    compensation_e += nrg_amp_exp - nrgMod_e + 1 + 1;

    {
      WORD16 *ptr_nrg_gain_mant = &nrg_gain[2 * start_grp];

      for (k = stop_grp - start_grp; k != 0; k--) {
        WORD16 temp1, temp2;
        temp1 = *ptr_nrg_gain_mant;
        temp2 = *(ptr_nrg_gain_mant + 1);
        temp1 = (temp1 * compensation_m) >> 16;
        temp2 = (temp2 + compensation_e);
        *ptr_nrg_gain_mant++ = temp1;
        *ptr_nrg_gain_mant++ = temp2;
      }
    }
  }
}

VOID ixheaacd_noiselimiting(ia_freq_band_data_struct *pstr_freq_band_data,
                            WORD32 skip_bands, WORD16 *ptr_enrg_orig,
                            WORD16 *nrg_est, WORD16 *nrg_gain,
                            WORD16 *noise_level_mant, WORD16 *nrg_sine,
                            WORD16 *ptr_limit_gain_table, FLAG noise_absc_flag,
                            ixheaacd_misc_tables *pstr_common_tables) {
  WORD32 c, k;
  WORD32 temp_val;
  WORD16 limit_gain_mant = *ptr_limit_gain_table++;
  WORD16 limit_gain_exp = *ptr_limit_gain_table;
  for (c = 0; c < pstr_freq_band_data->num_lf_bands; c++) {
    WORD16 max_gain_mant;
    WORD16 sum_orig_mant, sum_orig_exp;
    WORD16 max_gain_exp;
    WORD32 max_temp;
    WORD32 start_band = 0;
    WORD32 stop_band = 0;

    if ((pstr_freq_band_data->freq_band_tbl_lim[c] > skip_bands)) {
      start_band = (pstr_freq_band_data->freq_band_tbl_lim[c] - skip_bands);
    }

    if ((pstr_freq_band_data->freq_band_tbl_lim[c + 1] > skip_bands)) {
      stop_band = (pstr_freq_band_data->freq_band_tbl_lim[c + 1] - skip_bands);
    }

    if ((start_band < stop_band)) {
      ixheaacd_avggain_calc(ptr_enrg_orig, nrg_est, start_band, stop_band,
                            &sum_orig_mant, &sum_orig_exp, &max_gain_mant,
                            &max_gain_exp, pstr_common_tables, 0);

      max_temp = ixheaac_mult16x16in32_shl(max_gain_mant, limit_gain_mant);
      max_gain_exp = (max_gain_exp + limit_gain_exp);

      temp_val = ixheaac_norm32(max_temp);

      max_gain_exp = (WORD16)(max_gain_exp - temp_val);
      max_gain_mant = (WORD16)((max_temp << temp_val) >> 16);

      if ((max_gain_exp >= MAX_GAIN_EXP)) {
        max_gain_mant = 0x3000;
        max_gain_exp = MAX_GAIN_EXP;
      }

      {
        WORD16 *ptr_nrg_gain = &nrg_gain[2 * start_band];
        WORD16 *p_noise_level = &noise_level_mant[2 * start_band];

        for (k = stop_band - start_band; k != 0; k--) {
          WORD16 noise_amp_mant;
          WORD16 noise_amp_exp;

          WORD16 t_gain_mant = *(ptr_nrg_gain);
          WORD16 t_gain_exp = *(ptr_nrg_gain + 1);

          if (((t_gain_exp > max_gain_exp)) ||
              ((t_gain_exp == max_gain_exp) && (t_gain_mant > max_gain_mant))) {
            noise_amp_exp =
                ixheaacd_fix_mant_div(max_gain_mant, t_gain_mant,
                                      &noise_amp_mant, pstr_common_tables);
            noise_amp_exp += (max_gain_exp - t_gain_exp) + 1;

            *p_noise_level = ixheaac_extract16h(ixheaac_shl32_dir_sat_limit(
                ixheaac_mult16x16in32_shl(*p_noise_level, noise_amp_mant),
                noise_amp_exp));

            *ptr_nrg_gain = max_gain_mant;
            *(ptr_nrg_gain + 1) = max_gain_exp;
          }
          ptr_nrg_gain += 2;
          p_noise_level += 2;
        }
      }

      {
        WORD16 boost_gain_mant;
        WORD16 boost_gain_exp;
        WORD16 accu_m;
        WORD16 accu_e;
        WORD32 accu_m_t;
        WORD32 accu_e_t;
        WORD16 *ptr_nrg_gain = &nrg_gain[2 * start_band];
        WORD16 *ptr_enrg_est_buf = &nrg_est[2 * start_band];
        WORD16 *p_noise_level = &noise_level_mant[2 * start_band];
        WORD16 *p_nrg_sine = &nrg_sine[2 * start_band];

        accu_m_t = 0;
        accu_e_t = 0;

        for (k = stop_band - start_band; k != 0; k--) {
          WORD32 tmp_mant, tmp_e;

          tmp_mant = *ptr_nrg_gain++;
          tmp_e = *ptr_nrg_gain++;
          tmp_mant = (tmp_mant * (*ptr_enrg_est_buf++));
          tmp_e = (tmp_e + (*ptr_enrg_est_buf++));
          tmp_mant = tmp_mant >> 15;
          {
            WORD32 exp_diff;
            exp_diff = tmp_e - accu_e_t;
            if (exp_diff >= 0) {
              accu_m_t = tmp_mant + ixheaac_shr32(accu_m_t, exp_diff);
              accu_e_t = tmp_e;
            } else {
              exp_diff = -exp_diff;
              accu_m_t = ixheaac_shr32(tmp_mant, exp_diff) + accu_m_t;
            }
          }

          if (p_nrg_sine[0] != 0) {
            WORD32 exp_diff = p_nrg_sine[1] - accu_e_t;
            if (exp_diff >= 0) {
              accu_m_t = p_nrg_sine[0] + ixheaac_shr32(accu_m_t, exp_diff);
              accu_e_t = p_nrg_sine[1];
            } else {
              exp_diff = -exp_diff;
              accu_m_t = accu_m_t + ixheaac_shr32(p_nrg_sine[0], exp_diff);
            }

          } else {
            if (noise_absc_flag == 0) {
              WORD32 exp_diff = p_noise_level[1] - accu_e_t;
              if (exp_diff >= 0) {
                accu_m_t =
                    p_noise_level[0] + ixheaac_shr32(accu_m_t, exp_diff);
                accu_e_t = p_noise_level[1];
              } else {
                exp_diff = -exp_diff;
                accu_m_t =
                    accu_m_t + ixheaac_shr32(p_noise_level[0], exp_diff);
              }
            }
          }
          p_noise_level += 2;
          p_nrg_sine += 2;
        }

        {
          WORD32 norm_val;
          norm_val = 16 - ixheaac_norm32(accu_m_t);
          if (norm_val > 0) {
            accu_m_t >>= norm_val;
            accu_e_t += norm_val;
          }
        }

        accu_m = (WORD16)accu_m_t;
        accu_e = (WORD16)accu_e_t;

        boost_gain_exp = ixheaacd_fix_mant_div(
            sum_orig_mant, accu_m, &boost_gain_mant, pstr_common_tables);

        boost_gain_exp += (sum_orig_exp - accu_e) + 1;

        if ((boost_gain_exp > 2) ||
            ((boost_gain_exp == 2) && (boost_gain_mant > 0x5061))) {
          boost_gain_mant = 0x5061;
          boost_gain_exp = 2;
        }

        ptr_nrg_gain = &nrg_gain[2 * start_band];
        p_noise_level = &noise_level_mant[2 * start_band];
        p_nrg_sine = &nrg_sine[2 * start_band];

        for (k = stop_band - start_band; k != 0; k--) {
          WORD16 temp1, temp2, temp3;

          temp1 = *ptr_nrg_gain;
          temp2 = *p_nrg_sine;
          temp3 = *p_noise_level;

          temp1 = ixheaac_mult16_shl(temp1, boost_gain_mant);
          temp2 = ixheaac_mult16_shl(temp2, boost_gain_mant);
          temp3 = ixheaac_mult16_shl(temp3, boost_gain_mant);
          *ptr_nrg_gain++ = temp1;
          *p_nrg_sine++ = temp2;
          *p_noise_level++ = temp3;

          temp1 = *ptr_nrg_gain;
          temp2 = *p_nrg_sine;
          temp3 = *p_noise_level;

          temp1 = (temp1 + boost_gain_exp);
          temp2 = (temp2 + boost_gain_exp);
          temp3 = (temp3 + boost_gain_exp);
          *ptr_nrg_gain++ = (temp1);
          *p_nrg_sine++ = (temp2);
          *p_noise_level++ = (temp3);
        }
      }
    }
  }
}

VOID ixheaacd_conv_ergtoamplitudelp_dec(WORD32 bands, WORD16 noise_e,
                                        WORD16 *nrg_sine, WORD16 *nrg_gain,
                                        WORD16 *noise_level_mant,
                                        WORD16 *sqrt_table) {
  WORD32 k;
  for (k = 0; k < bands; k++) {
    WORD32 shift;
    ixheaacd_fix_mant_exp_sqrt(&nrg_sine[2 * k], sqrt_table);
    ixheaacd_fix_mant_exp_sqrt(&nrg_gain[2 * k], sqrt_table);
    ixheaacd_fix_mant_exp_sqrt(&noise_level_mant[2 * k], sqrt_table);

    shift = (noise_e - noise_level_mant[2 * k + 1]);

    shift = (shift - 4);
    if (shift > 0)
      noise_level_mant[2 * k] = (noise_level_mant[2 * k] >> shift);
    else
      noise_level_mant[2 * k] = (noise_level_mant[2 * k] << -shift);

    shift = (nrg_sine[2 * k + 1] - noise_e);
    if (shift > 0)
      nrg_sine[2 * k] = ixheaac_shl16_sat(nrg_sine[2 * k], (WORD16)shift);
    else
      nrg_sine[2 * k] = ixheaac_shr16(nrg_sine[2 * k], (WORD16)-shift);
  }
}

VOID ixheaacd_conv_ergtoamplitude_dec(WORD32 bands, WORD16 noise_e,
                                      WORD16 *nrg_sine, WORD16 *nrg_gain,
                                      WORD16 *noise_level_mant,
                                      WORD16 *sqrt_table) {
  WORD32 k;
  for (k = 0; k < bands; k++) {
    WORD32 shift;

    ixheaacd_fix_mant_exp_sqrt(&nrg_sine[2 * k], sqrt_table);
    ixheaacd_fix_mant_exp_sqrt(&nrg_gain[2 * k], sqrt_table);
    ixheaacd_fix_mant_exp_sqrt(&noise_level_mant[2 * k], sqrt_table);

    shift = (noise_e - noise_level_mant[2 * k + 1]);

    shift = (shift - 4);
    if (shift > 0) {
      if (shift > 31) {
        shift = 31;
      }
      noise_level_mant[2 * k] = (noise_level_mant[2 * k] >> shift);
    } else {
      if (shift < -31) {
        shift = -31;
      }
      noise_level_mant[2 * k] = (noise_level_mant[2 * k] << -shift);
    }
  }
}

static PLATFORM_INLINE VOID ixheaacd_adapt_noise_gain_calc(
    ia_sbr_calc_env_struct *ptr_sbr_calc_env, WORD32 noise_e,
    WORD32 num_sub_bands, WORD32 skip_bands, WORD16 *nrg_gain,
    WORD16 *noise_level_mant, WORD16 *nrg_sine, WORD32 start_pos,
    WORD32 end_pos, WORD32 input_e, WORD32 adj_e, WORD32 final_e,
    WORD32 sub_band_start, WORD32 lb_scale, FLAG noise_absc_flag,
    WORD32 smooth_length, WORD32 **anal_buf_real_mant,
    WORD32 **anal_buf_imag_mant, WORD32 low_pow_flag,
    ia_sbr_tables_struct *ptr_sbr_tables, WORD16 max_cols) {
  WORD32 l, k;
  WORD32 scale_change;
  WORD32 bands = num_sub_bands - skip_bands;
  WORD16 *ptr_filt_buf;
  WORD16 *ptr_filt_buf_noise;
  WORD16 *ptr_gain = &nrg_gain[0];

  if (ptr_sbr_calc_env->start_up) {
    WORD16 *ptr_noise = noise_level_mant;
    ptr_sbr_calc_env->start_up = 0;

    ptr_sbr_calc_env->filt_buf_noise_e = noise_e;
    ptr_filt_buf = &ptr_sbr_calc_env->filt_buf_me[skip_bands * 2];
    ptr_filt_buf_noise = &ptr_sbr_calc_env->filt_buf_noise_m[skip_bands];

    for (k = bands; k != 0; k--) {
      WORD16 temp1 = *ptr_gain++;
      WORD16 temp2 = *ptr_gain++;
      WORD16 temp3 = *ptr_noise;
      ptr_noise += 2;

      *ptr_filt_buf++ = temp1;
      *ptr_filt_buf++ = temp2;
      *ptr_filt_buf_noise++ = temp3;
    }
  } else {
    ixheaacd_equalize_filt_buff_exp(
        &ptr_sbr_calc_env->filt_buf_me[2 * skip_bands], nrg_gain, bands);
  }

  for (l = start_pos; l < end_pos; l++) {

    if (max_cols != 30) {
      if ((l < MAX_COLS)) {
        scale_change = (adj_e - input_e);
      } else {
        scale_change = (final_e - input_e);

        if (((l == MAX_COLS)) && ((start_pos < MAX_COLS))) {
          WORD32 diff = final_e - noise_e;
          noise_e = final_e;
          ixheaacd_noise_level_rescaling(noise_level_mant, diff, bands, 2);
        }
      }
    } else {
      if ((l < max_cols)) {
        scale_change = (adj_e - input_e);
      } else {
        scale_change = (final_e - input_e);

        if (((l == max_cols)) && ((start_pos < max_cols))) {
          WORD32 diff = final_e - noise_e;
          noise_e = final_e;
          ixheaacd_noise_level_rescaling(noise_level_mant, diff, bands, 2);
        }
      }
    }

    ixheaacd_noise_level_rescaling(ptr_sbr_calc_env->filt_buf_noise_m,
                                   ptr_sbr_calc_env->filt_buf_noise_e - noise_e,
                                   num_sub_bands, 1);

    ptr_sbr_calc_env->filt_buf_noise_e = noise_e;

    {
      WORD32 *anal_buf_real_m_l;
      anal_buf_real_m_l = anal_buf_real_mant[l];

      if (low_pow_flag) {
        WORD32 index = ptr_sbr_calc_env->ph_index;
        WORD32 harm_index = ptr_sbr_calc_env->harm_index;
        WORD32 freq_inv_flag = (sub_band_start & 1);
        WORD32 *ptr_real_buf = &anal_buf_real_m_l[sub_band_start];

        const WORD32 *ptr_rand_ph = &ptr_sbr_tables->sbr_rand_ph[index + 1];

        ptr_sbr_calc_env->ph_index =
            (WORD16)((index + num_sub_bands) & (SBR_NF_NO_RANDOM_VAL - 1));
        ptr_sbr_calc_env->harm_index = (WORD16)(((harm_index + 1)) & 3);

        if (!(harm_index & 0x1)) {
          (*ixheaacd_harm_idx_zerotwolp)(
              ptr_real_buf, nrg_gain, scale_change, nrg_sine, ptr_rand_ph,
              noise_level_mant, num_sub_bands, noise_absc_flag, harm_index);
        } else {
          WORD32 noise = (noise_e - 16) - lb_scale;

          freq_inv_flag = (!freq_inv_flag);
          freq_inv_flag = (freq_inv_flag << 1) - 1;

          if (harm_index == 3) freq_inv_flag = -freq_inv_flag;

          ixheaacd_harm_idx_onethreelp(ptr_real_buf, nrg_gain, scale_change,
                                       nrg_sine, ptr_rand_ph, noise_level_mant,
                                       num_sub_bands, noise_absc_flag,
                                       freq_inv_flag, noise, sub_band_start);
        }

      } else {
        WORD16 smooth_ratio;
        WORD32 *anal_buf_imag_m_l;
        anal_buf_imag_m_l = anal_buf_imag_mant[l];

        if (((l - start_pos) < smooth_length)) {
          smooth_ratio = ptr_sbr_tables->env_calc_tables_ptr
                             ->sbr_smooth_filter[(l - start_pos)];
        } else {
          smooth_ratio = 0;
        }

        ixheaacd_adj_timeslot(
            &anal_buf_real_m_l[sub_band_start],
            &anal_buf_imag_m_l[sub_band_start],
            &ptr_sbr_calc_env->filt_buf_me[2 * skip_bands],
            &ptr_sbr_calc_env->filt_buf_noise_m[skip_bands], nrg_gain,
            noise_level_mant, nrg_sine, (WORD16)(noise_e - 16),
            &ptr_sbr_calc_env->harm_index, (WORD16)sub_band_start,
            (WORD16)(bands), (WORD16)scale_change, smooth_ratio,
            noise_absc_flag, &ptr_sbr_calc_env->ph_index, ptr_sbr_tables);
      }
    }
  }

  ixheaacd_filt_buf_update(ptr_sbr_calc_env->filt_buf_me + 2 * skip_bands,
                           ptr_sbr_calc_env->filt_buf_noise_m + skip_bands,
                           nrg_gain, noise_level_mant, bands);
}

VOID ixheaacd_calc_subband_gains(ia_freq_band_data_struct *pstr_freq_band_data,
                                 ia_sbr_frame_info_data_struct *ptr_frame_data,
                                 WORD32 freq_res, WORD16 *ptr_noise_floor,
                                 WORD32 num_sf_bands, WORD32 mvalue, WORD32 env,
                                 WORD8 *sine_mapped_matrix,
                                 WORD8 *alias_red_buf, WORD16 *ptr_enrg_orig,
                                 WORD16 *nrg_sine, WORD16 *nrg_est,
                                 WORD16 *nrg_gain, WORD16 *noise_level_mant,
                                 FLAG noise_absc_flag,
                                 ixheaacd_misc_tables *pstr_common_tables) {
  WORD16 *ptr_freq_band_tbl = pstr_freq_band_data->freq_band_table[freq_res];
  WORD32 ui_noise = pstr_freq_band_data->freq_band_tbl_noise[1];
  WORD32 nb_idx = 0;
  WORD16 tmp_noise_mant;
  WORD16 tmp_noise_exp;
  WORD8 *ptr_sine_mapped = sine_mapped_matrix;
  WORD32 sub_band_start = pstr_freq_band_data->sub_band_start;
  WORD32 skip_bands = (ptr_frame_data->max_qmf_subband_aac - sub_band_start);
  WORD8 *ptr_sine_mapped_1 = &sine_mapped_matrix[skip_bands];
  WORD32 k, c = 0, j;

  WORD16 *ptr_env_sf_arr = &ptr_frame_data->int_env_sf_arr[mvalue];
  WORD8 *ptr_alias_red_buf =
      &alias_red_buf[ptr_freq_band_tbl[0] - sub_band_start];

  tmp_noise_mant = (WORD16)(ptr_noise_floor[nb_idx] & MASK_M);
  tmp_noise_exp =
      (WORD16)(ptr_noise_floor[nb_idx] & MASK_FOR_EXP) - NOISE_EXP_OFFSET;

  for (j = 0; j < num_sf_bands; j++) {
    WORD8 sine_present_flag;
    WORD16 tmp_nrg_ref_exp, tmp_nrg_ref_mant;
    WORD16 li = *ptr_freq_band_tbl++;
    WORD16 ui = *ptr_freq_band_tbl;
    WORD16 env_sf_val = *ptr_env_sf_arr++;

    tmp_nrg_ref_exp =
        (WORD16)((env_sf_val & (WORD16)MASK_FOR_EXP) - NRG_EXP_OFFSET);
    tmp_nrg_ref_mant = (WORD16)(env_sf_val & MASK_M);

    sine_present_flag = 0;
    for (k = li; k < ui; k++) {
      if ((env >= *ptr_sine_mapped++)) sine_present_flag = 1;
    }
    for (k = li; k < ui; k++) {
      *ptr_alias_red_buf++ = !sine_present_flag;

      if ((k >= ui_noise)) {
        nb_idx++;
        ui_noise = pstr_freq_band_data->freq_band_tbl_noise[nb_idx + 1];
        tmp_noise_mant = (WORD16)(ptr_noise_floor[nb_idx] & MASK_M);
        tmp_noise_exp =
            (WORD16)(ptr_noise_floor[nb_idx] & MASK_FOR_EXP) - NOISE_EXP_OFFSET;
      }

      if ((k >= ptr_frame_data->max_qmf_subband_aac)) {
        ptr_enrg_orig[2 * c] = tmp_nrg_ref_mant;
        ptr_enrg_orig[2 * c + 1] = tmp_nrg_ref_exp;
        nrg_sine[2 * c] = 0;
        nrg_sine[2 * c + 1] = 0;

        ixheaacd_subbandgain_calc(
            tmp_nrg_ref_mant, tmp_noise_mant, nrg_est[2 * c],
            nrg_est[2 * c + 1], tmp_noise_exp, tmp_nrg_ref_exp,
            sine_present_flag,
            (env >= ptr_sine_mapped_1[c]) ? (FLAG)1 : (FLAG)0, noise_absc_flag,
            &nrg_gain[2 * c], &noise_level_mant[2 * c], &nrg_sine[2 * c],
            pstr_common_tables);
        c++;
      }
    }
  }
}

#define ALIGN_SIZE64(x) ((((x) + 7) >> 3) << 3)

IA_ERRORCODE ixheaacd_calc_sbrenvelope(
    ia_sbr_scale_fact_struct *ptr_sbr_scale_fac,
    ia_sbr_calc_env_struct *ptr_sbr_calc_env,
    ia_sbr_header_data_struct *ptr_header_data,
    ia_sbr_frame_info_data_struct *ptr_frame_data,
    ia_sbr_prev_frame_data_struct *ptr_frame_data_prev,
    WORD32 **anal_buf_real_mant, WORD32 **anal_buf_imag_mant,
    WORD16 *deg_patched, FLAG low_pow_flag,
    ia_sbr_tables_struct *ptr_sbr_tables,
    ixheaacd_misc_tables *pstr_common_tables, WORD32 *ptr_qmf_matrix,
    WORD32 audio_object_type) {
  WORD32 i, j, m;
  WORD32 noise_floor_idx;
  WORD32 start_pos, end_pos;
  WORD32 freq_res;
  WORD32 num_env = ptr_frame_data->str_frame_info_details.num_env;
  WORD16 *ptr_border_vec = ptr_frame_data->str_frame_info_details.border_vec;
  IA_ERRORCODE err_code = IA_NO_ERROR;
  WORD16 *ptr_noise_floor;
  ia_freq_band_data_struct *pstr_freq_band_data =
      ptr_header_data->pstr_freq_band_data;

  FLAG noise_absc_flag;
  WORD32 smooth_length;

  const WORD16 *num_sf_bands = pstr_freq_band_data->num_sf_bands;
  const WORD32 num_nf_bands = pstr_freq_band_data->num_nf_bands;

  WORD32 sub_band_start = pstr_freq_band_data->sub_band_start;
  WORD32 sub_band_end = pstr_freq_band_data->sub_band_end;

  WORD16 num_timeslots = ptr_header_data->num_time_slots;
  WORD16 max_cols = ptr_header_data->num_time_slots * 2;

  WORD32 num_sub_bands;
  WORD32 skip_bands;
  WORD32 bands;

  WORD num_cols;
  WORD32 first_start;

  WORD16 *ptr_sbr_lim_gain;
  WORD32 max_sfb_nrg_exp;

  WORD16 *ptr_enrg_orig;

  WORD32 input_e;
  WORD32 ov_adj_e;
  WORD32 adj_e;
  WORD32 output_e;
  WORD32 final_e;
  WORD16 noise_e;
  WORD16 lb_scale;

  WORD16 nrg_est[2 * MAX_FREQ_COEFFS];

  WORD16 nrg_gain[2 * MAX_FREQ_COEFFS];
  WORD16 noise_level_mant[2 * MAX_FREQ_COEFFS];
  WORD16 nrg_sine[2 * MAX_FREQ_COEFFS];

  WORD8 sine_mapped_matrix[MAX_FREQ_COEFFS];
  WORD8 alias_red_buf[64];

  ptr_noise_floor = ptr_frame_data->int_noise_floor;

  ptr_enrg_orig =
      (WORD16 *)((WORD8 *)ptr_frame_data +
                 ALIGN_SIZE64(sizeof(ia_sbr_frame_info_data_struct)));

  num_env = ptr_frame_data->str_frame_info_details.num_env;
  ptr_border_vec = ptr_frame_data->str_frame_info_details.border_vec;
  num_sub_bands = (sub_band_end - sub_band_start);
  skip_bands = (ptr_frame_data->max_qmf_subband_aac - sub_band_start);

  ixheaacd_map_sineflags(
      pstr_freq_band_data->freq_band_table[HIGH],
      pstr_freq_band_data->num_sf_bands[HIGH], ptr_frame_data->add_harmonics,
      ptr_sbr_calc_env->harm_flags_prev,
      ptr_frame_data->str_frame_info_details.transient_env, sine_mapped_matrix);

  adj_e = 0;
  {
    WORD16 max_noise;
    WORD32 first_band;

    if (ptr_frame_data_prev->max_qmf_subband_aac >
        ptr_frame_data->max_qmf_subband_aac)
      first_band = (ptr_frame_data_prev->max_qmf_subband_aac - sub_band_start);
    else
      first_band = (ptr_frame_data->max_qmf_subband_aac - sub_band_start);

    max_noise = 0;
    for (i = first_band; i < num_sub_bands; i++) {
      if (ptr_sbr_calc_env->filt_buf_noise_m[i] > max_noise) {
        max_noise = ptr_sbr_calc_env->filt_buf_noise_m[i];
      }
    }
    adj_e = ((ptr_sbr_calc_env->filt_buf_noise_e - ixheaac_norm32(max_noise)) -
             16);
  }

  final_e = 0;
  {
    WORD16 *ptr_env_sf_buf = ptr_frame_data->int_env_sf_arr;
    for (i = 0; i < num_env; i++) {
      WORD32 temp_val;

      max_sfb_nrg_exp = NRG_EXP_OFFSET - SHORT_BITS;

      freq_res = ptr_frame_data->str_frame_info_details.freq_res[i];

      for (j = 0; j < num_sf_bands[freq_res]; j++) {
        temp_val = ((*ptr_env_sf_buf++ & MASK_FOR_EXP));

        if ((temp_val > max_sfb_nrg_exp)) {
          max_sfb_nrg_exp = temp_val;
        }
      }

      max_sfb_nrg_exp = (max_sfb_nrg_exp - NRG_EXP_OFFSET);

      temp_val = ((max_sfb_nrg_exp + 13) >> 1);

      if (num_timeslots != 15) {
        if ((ptr_border_vec[i] < SBR_TIME_SLOTS)) {
          if ((temp_val > adj_e)) {
            adj_e = (WORD16)temp_val;
          }
        }

        if ((ptr_border_vec[i + 1] > SBR_TIME_SLOTS)) {
          if ((temp_val > final_e)) {
            final_e = (WORD16)temp_val;
          }
        }
      } else {
        if ((ptr_border_vec[i] < num_timeslots)) {
          if ((temp_val > adj_e)) {
            adj_e = (WORD16)temp_val;
          }
        }

        if ((ptr_border_vec[i + 1] > num_timeslots)) {
          if ((temp_val > final_e)) {
            final_e = (WORD16)temp_val;
          }
        }
      }
    }
  }

  m = 0;
  noise_floor_idx = 0;

  for (i = 0; i < num_env; i++) {
    if (audio_object_type == AOT_ER_AAC_ELD ||
        audio_object_type == AOT_ER_AAC_LD) {
      start_pos = ptr_border_vec[i];
      end_pos = ptr_border_vec[i + 1];
    } else {
      start_pos = SBR_TIME_STEP * ptr_border_vec[i];
      end_pos = SBR_TIME_STEP * ptr_border_vec[i + 1];
    }
    if ((start_pos >= MAX_ENV_COLS) || (end_pos > MAX_ENV_COLS))
      return IA_FATAL_ERROR;
    freq_res = ptr_frame_data->str_frame_info_details.freq_res[i];

    if (noise_floor_idx >= MAX_NOISE_ENVELOPES) return IA_FATAL_ERROR;

    if (ptr_border_vec[i] ==
        ptr_frame_data->str_frame_info_details
            .noise_border_vec[noise_floor_idx + 1]) {
      ptr_noise_floor += num_nf_bands;
      noise_floor_idx++;
    }

    if ((i == ptr_frame_data->str_frame_info_details.transient_env) ||
        (i == ptr_sbr_calc_env->tansient_env_prev)) {
      noise_absc_flag = 1;
      smooth_length = 0;
    } else {
      noise_absc_flag = 0;
      smooth_length = ((1 - ptr_header_data->smoothing_mode) << 2);
    }

    input_e = 15 - ptr_sbr_scale_fac->hb_scale;

    if (ptr_header_data->interpol_freq) {
      (*ixheaacd_enery_calc_per_subband)(
          start_pos, end_pos, ptr_frame_data->max_qmf_subband_aac, sub_band_end,
          input_e, nrg_est, low_pow_flag, ptr_sbr_tables, ptr_qmf_matrix);
    } else {
      ixheaacd_enery_calc_persfb(
          anal_buf_real_mant, anal_buf_imag_mant, num_sf_bands[freq_res],
          pstr_freq_band_data->freq_band_table[freq_res], start_pos, end_pos,
          ptr_frame_data->max_qmf_subband_aac, input_e, nrg_est, low_pow_flag,
          ptr_sbr_tables);
    }

    if (pstr_freq_band_data->freq_band_table[freq_res][0] < pstr_freq_band_data->sub_band_start) {
      pstr_freq_band_data->sub_band_start = pstr_freq_band_data->freq_band_table[freq_res][0];
      return IA_FATAL_ERROR;
    }

    ixheaacd_calc_subband_gains(
        pstr_freq_band_data, ptr_frame_data, freq_res, ptr_noise_floor,
        num_sf_bands[freq_res], m, i, sine_mapped_matrix, alias_red_buf,
        ptr_enrg_orig, nrg_sine, nrg_est, nrg_gain, noise_level_mant,
        noise_absc_flag, pstr_common_tables);

    m += num_sf_bands[freq_res];

    ptr_sbr_lim_gain =
        &ptr_sbr_tables->env_calc_tables_ptr
             ->sbr_lim_gains_m[2 * ptr_header_data->limiter_gains];
    ixheaacd_noiselimiting(pstr_freq_band_data, skip_bands, ptr_enrg_orig,
                           nrg_est, nrg_gain, noise_level_mant, nrg_sine,
                           ptr_sbr_lim_gain, noise_absc_flag,
                           pstr_common_tables);

    if (low_pow_flag) {
      ixheaacd_alias_reduction(deg_patched + sub_band_start, nrg_gain, nrg_est,
                               alias_red_buf, num_sub_bands,
                               pstr_common_tables);
    }

    if (max_cols != 30) {
      if ((start_pos < MAX_COLS)) {
        noise_e = adj_e;
      } else {
        noise_e = final_e;
      }
    } else {
      if ((start_pos < max_cols)) {
        noise_e = adj_e;
      } else {
        noise_e = final_e;
      }
    }

    bands = num_sub_bands - skip_bands;

    if (low_pow_flag) {
      (*ixheaacd_conv_ergtoamplitudelp)(
          bands, noise_e, nrg_sine, nrg_gain, noise_level_mant,
          (WORD16 *)pstr_common_tables->sqrt_table);
    } else

    {
      (*ixheaacd_conv_ergtoamplitude)(bands, noise_e, nrg_sine, nrg_gain,
                                      noise_level_mant,
                                      (WORD16 *)pstr_common_tables->sqrt_table);
    }

    lb_scale = ixheaac_sub16(15, ptr_sbr_scale_fac->lb_scale);

    ixheaacd_adapt_noise_gain_calc(
        ptr_sbr_calc_env, noise_e, num_sub_bands, skip_bands, nrg_gain,
        noise_level_mant, nrg_sine, start_pos, end_pos, input_e, adj_e, final_e,
        ptr_frame_data->max_qmf_subband_aac, lb_scale, noise_absc_flag,
        smooth_length, anal_buf_real_mant, anal_buf_imag_mant, low_pow_flag,
        ptr_sbr_tables, max_cols);
  }

  first_start = ptr_border_vec[0] * SBR_TIME_STEP;
  {
    WORD32 ov_reserve, reserve;

    ov_reserve = reserve = 0;

    if (audio_object_type != AOT_ER_AAC_ELD) {
      if (ptr_header_data->channel_mode == PS_STEREO) {
        ov_reserve = (*ixheaacd_ixheaacd_expsubbandsamples)(
            anal_buf_real_mant, anal_buf_imag_mant,
            ptr_frame_data->max_qmf_subband_aac, sub_band_end, 0, first_start,
            low_pow_flag);

        if (max_cols != 30) {
          reserve = (*ixheaacd_ixheaacd_expsubbandsamples)(
            anal_buf_real_mant, anal_buf_imag_mant,
            ptr_frame_data->max_qmf_subband_aac, sub_band_end, first_start,
            MAX_COLS, low_pow_flag);
        } else {
          reserve = (*ixheaacd_ixheaacd_expsubbandsamples)(
            anal_buf_real_mant, anal_buf_imag_mant,
            ptr_frame_data->max_qmf_subband_aac, sub_band_end, first_start,
            max_cols, low_pow_flag);
        }
      }
    }

    output_e = 0;

    ov_adj_e = 15 - ptr_sbr_scale_fac->ov_hb_scale;

    if (((ov_adj_e - ov_reserve) > (adj_e - reserve)))
      output_e = (ov_adj_e - ov_reserve);
    else
      output_e = (adj_e - reserve);

    (*ixheaacd_adjust_scale)(anal_buf_real_mant, anal_buf_imag_mant,
                             ptr_frame_data->max_qmf_subband_aac, sub_band_end,
                             0, first_start, (ov_adj_e - output_e),
                             low_pow_flag);

    num_cols = (ptr_header_data->num_time_slots * ptr_header_data->time_step);

    (*ixheaacd_adjust_scale)(anal_buf_real_mant, anal_buf_imag_mant,
                             ptr_frame_data->max_qmf_subband_aac, sub_band_end,
                             first_start, num_cols, (adj_e - output_e),
                             low_pow_flag);
  }

  ptr_sbr_scale_fac->hb_scale = (WORD16)(15 - output_e);

  ptr_sbr_scale_fac->ov_hb_scale = (WORD16)(15 - final_e);

  if (ptr_frame_data->str_frame_info_details.transient_env == num_env) {
    ptr_sbr_calc_env->tansient_env_prev = 0;
  } else {
    ptr_sbr_calc_env->tansient_env_prev = -1;
  }
  return err_code;
}

VOID ixheaacd_equalize_filt_buff_exp(WORD16 *ptr_filt_buf, WORD16 *nrg_gain,
                                     WORD32 subbands) {
  WORD32 band;
  WORD32 diff;
  WORD32 gain_m, gain_e;
  WORD32 filt_buf_mant, filt_buf_exp;

  for (band = subbands - 1; band >= 0; band--) {
    filt_buf_exp = *(ptr_filt_buf + 1);
    gain_e = *(nrg_gain + 1);
    filt_buf_mant = *ptr_filt_buf;
    gain_m = *nrg_gain;
    diff = (gain_e - filt_buf_exp);

    if (diff >= 0) {
      *(ptr_filt_buf + 1) = (WORD16)(gain_e);

      *ptr_filt_buf = (WORD16)(*ptr_filt_buf >> diff);
    } else {
      WORD32 reserve;
      reserve = (ixheaac_norm32(filt_buf_mant) - 16);

      if ((diff + reserve) >= 0) {
        *ptr_filt_buf = (WORD16)(filt_buf_mant << -diff);
        *(ptr_filt_buf + 1) = (WORD16)(filt_buf_exp + diff);
      } else {
        WORD32 shift;

        *ptr_filt_buf = (WORD16)(filt_buf_mant << reserve);

        *(ptr_filt_buf + 1) = (WORD16)(filt_buf_exp - reserve);

        shift = -(reserve + diff);

        *nrg_gain = (WORD16)(gain_m >> shift);
        *(nrg_gain + 1) = (WORD16)(*(nrg_gain + 1) + shift);
      }
    }
    nrg_gain += 2;
    ptr_filt_buf += 2;
  }
}

VOID ixheaacd_filt_buf_update(WORD16 *ptr_filt_buf,
                                                     WORD16 *ptr_filt_buf_noise,
                                                     WORD16 *nrg_gain,
                                                     WORD16 *noise_level_mant,
                                                     WORD32 num_sub_bands) {
  WORD32 k;
  WORD32 temp1, temp2;

  for (k = num_sub_bands - 1; k >= 0; k--) {
    temp1 = *nrg_gain;
    nrg_gain += 2;
    temp2 = *noise_level_mant;
    noise_level_mant += 2;

    *ptr_filt_buf = temp1;
    ptr_filt_buf += 2;
    *ptr_filt_buf_noise++ = temp2;
  }
}

VOID ixheaacd_noise_level_rescaling(WORD16 *noise_level_mant, WORD32 diff,
                                    WORD32 num_sub_bands,
                                    WORD32 ixheaacd_drc_offset) {
  WORD32 k;

  if (diff > 0) {
    for (k = num_sub_bands - 1; k >= 0; k--) {
      *noise_level_mant = *noise_level_mant >> diff;
      noise_level_mant += ixheaacd_drc_offset;
    }
  } else if (diff < 0) {
    diff = -diff;
    for (k = num_sub_bands - 1; k >= 0; k--) {
      *noise_level_mant = *noise_level_mant << diff;
      noise_level_mant += ixheaacd_drc_offset;
    }
  }
}

VOID ixheaacd_adjust_scale_dec(WORD32 **re, WORD32 **im, WORD32 sub_band_start,
                               WORD32 sub_band_end, WORD32 start_pos,
                               WORD32 next_pos, WORD32 shift,
                               FLAG low_pow_flag) {
  WORD32 k, l;

  if (shift != 0) {
    WORD32 num_sub_bands = (sub_band_end - sub_band_start);

    shift = ixheaac_min32(shift, 31);
    shift = ixheaac_max32(shift, -31);

    if (low_pow_flag) {
      if (shift > 0) {
        for (l = start_pos; l < next_pos; l++) {
          WORD32 *ptr = re[l] + sub_band_start;
          for (k = num_sub_bands - 1; k >= 0; k--) {
            *ptr = (*ptr << shift);
            ptr++;
          }
        }
      } else {
        shift = -shift;
        for (l = start_pos; l < next_pos; l++) {
          WORD32 *ptr = re[l] + sub_band_start;
          for (k = num_sub_bands - 1; k >= 0; k--) {
            *ptr = (*ptr >> shift);
            ptr++;
          }
        }
      }
    } else {
      if (shift > 0) {
        for (l = start_pos; l < next_pos; l++) {
          WORD32 *ptr = re[l] + sub_band_start;
          WORD32 *pti = im[l] + sub_band_start;
          for (k = num_sub_bands; k > 0; k--) {
            *ptr = (*ptr << shift);
            *pti = (*pti << shift);
            pti++;
            ptr++;
          }
        }
      } else {
        shift = -shift;
        for (l = start_pos; l < next_pos; l++) {
          WORD32 *ptr = re[l] + sub_band_start;
          WORD32 *pti = im[l] + sub_band_start;
          for (k = num_sub_bands; k > 0; k--) {
            *ptr = (*ptr >> shift);
            *pti = (*pti >> shift);
            ptr++;
            pti++;
          }
        }
      }
    }
  }
}

WORD16 ixheaacd_expsubbandsamples_dec(WORD32 **re, WORD32 **im,
                                      WORD32 sub_band_start,
                                      WORD32 sub_band_end, WORD32 start_pos,
                                      WORD32 next_pos, FLAG low_pow_flag) {
  WORD32 l, k;
  WORD16 max_shift;

  WORD32 value;
  WORD32 max_abs;
  WORD32 num_sub_bands;

  WORD32 *ptr_real;
  WORD32 *ptr_imag;

  max_abs = 1;
  num_sub_bands = (sub_band_end - sub_band_start);

  if (low_pow_flag) {
    for (l = start_pos; l < next_pos; l++) {
      WORD32 temp_real;
      ptr_real = re[l] + sub_band_start;
      temp_real = *ptr_real++;
      for (k = num_sub_bands; k > 0; k--) {
        value = ixheaac_abs32_nrm(temp_real);
        max_abs |= value;
        temp_real = *ptr_real++;
      }
    }
    max_shift = ixheaac_pnorm32(max_abs);
  } else {
    for (l = start_pos; l < next_pos; l++) {
      ptr_real = re[l] + sub_band_start;
      ptr_imag = im[l] + sub_band_start;

      for (k = num_sub_bands; k > 0; k--) {
        WORD32 temp_real = *ptr_real++;
        WORD32 tempIm = *ptr_imag++;

        temp_real = ixheaac_abs32_nrm(temp_real);
        max_abs |= temp_real;
        tempIm = ixheaac_abs32_nrm(tempIm);
        max_abs |= tempIm;
      }
    }
    max_shift = ixheaac_pnorm32(max_abs);
  }

  return max_shift;
}

#define SHIFT_BEFORE_SQUARE 4

VOID ixheaacd_enery_calc_per_subband_dec(WORD32 start_pos, WORD32 next_pos,
                                         WORD32 sub_band_start,
                                         WORD32 sub_band_end, WORD32 frame_exp,
                                         WORD16 *nrg_est, FLAG low_pow_flag,
                                         ia_sbr_tables_struct *ptr_sbr_tables,
                                         WORD32 *ptr_qmf_matrix) {
  WORD16 temp;
  WORD16 inv_width;
  WORD16 sum_m;
  WORD32 accu;
  WORD32 k, l;
  WORD32 pre_shift_val;
  WORD32 shift;
  WORD32 *p_real;
  WORD32 max_shift_gap = SHIFT_BEFORE_SQUARE;
  WORD32 extra_shift = 0;
  WORD32 num_cols = next_pos - start_pos;

  if (low_pow_flag) {
    max_shift_gap -= 1;
    p_real = ptr_qmf_matrix + sub_band_start + (start_pos << 6);
    extra_shift++;
  } else {
    p_real = ptr_qmf_matrix + sub_band_start + (start_pos << 7);
    num_cols = num_cols << 1;
  }
  inv_width = ptr_sbr_tables->env_calc_tables_ptr
                  ->sbr_inv_int_table[(next_pos - start_pos)];
  frame_exp = (frame_exp << 1);

  {
    WORD32 *ptr;
    for (k = sub_band_start; k < sub_band_end; k++) {
      WORD32 max_val = 1;

      ptr = p_real;

      for (l = num_cols; l != 0; l -= 2) {
        WORD32 value = ixheaac_abs32_nrm(*ptr);
        ptr += 64;
        max_val = ixheaac_max32(value, max_val);
        value = ixheaac_abs32_nrm(*ptr);
        ptr += 64;
        max_val = ixheaac_max32(value, max_val);
      }
      pre_shift_val = (ixheaac_pnorm32(max_val) - max_shift_gap);

      accu = 0L;
      shift = 16 - pre_shift_val;
      ptr = p_real;

      if (shift > 0)
        for (l = num_cols; l != 0; l -= 2) {
          temp = (WORD16)((*(ptr) >> shift));
          ptr += 64;
          accu += (temp * temp);
          temp = (WORD16)((*(ptr) >> shift));
          ptr += 64;
          accu += (temp * temp);
        }
      else
        for (l = num_cols; l != 0; l -= 2) {
          temp = (WORD16)((*(ptr) << (-shift)));
          ptr += 64;
          accu += (temp * temp);
          temp = (WORD16)((*(ptr) << (-shift)));
          ptr += 64;
          accu += (temp * temp);
        }

      if (accu != 0L) {
        shift = -(ixheaac_pnorm32(accu));
        sum_m = (WORD16)(ixheaac_shr32_dir_sat_limit(accu, (16 + shift)));
        *nrg_est++ = ixheaac_mult16_shl_sat(sum_m, inv_width);
        shift = (shift - (pre_shift_val << 1));
        shift += extra_shift;
        *nrg_est++ = (WORD16)(frame_exp + shift + 1);
      } else {
        *nrg_est++ = 0;
        *nrg_est++ = 0;
      }

      p_real++;
    }
  }
}

VOID ixheaacd_enery_calc_persfb(WORD32 **anal_buf_real, WORD32 **anal_buf_imag,
                                WORD32 num_sf_bands, WORD16 *freq_band_table,
                                WORD32 start_pos, WORD32 next_pos,
                                WORD32 max_qmf_subband_aac, WORD32 frame_exp,
                                WORD16 *nrg_est, FLAG low_pow_flag,
                                ia_sbr_tables_struct *ptr_sbr_tables) {
  WORD16 inv_width;
  WORD32 pre_shift_val;
  WORD32 shift;
  WORD32 sum_e;
  WORD16 sum_m;

  WORD32 j, k, l;
  WORD32 li, ui;
  WORD32 accu_line;
  WORD32 accumulate;
  WORD32 extra_shift = 10;

  inv_width = ptr_sbr_tables->env_calc_tables_ptr
                  ->sbr_inv_int_table[(next_pos - start_pos)];

  frame_exp = (frame_exp << 1);

  if (low_pow_flag) extra_shift++;

  for (j = 0; j < num_sf_bands; j++) {
    li = freq_band_table[j];

    if ((li >= max_qmf_subband_aac)) {
      ui = freq_band_table[j + 1];

      pre_shift_val = (*ixheaacd_ixheaacd_expsubbandsamples)(
          anal_buf_real, anal_buf_imag, li, ui, start_pos, next_pos,
          low_pow_flag);

      pre_shift_val = (pre_shift_val - SHIFT_BEFORE_SQUARE);

      accumulate = 0;

      for (k = li; k < ui; k++) {
        WORD32 pre_shift1 = (16 - pre_shift_val);
        accu_line = 0L;
        pre_shift1 = min(pre_shift1, 31);
        {
          WORD32 *ptr = &anal_buf_real[start_pos][k];
          WORD32 inc = !low_pow_flag;
          for (l = (next_pos - start_pos) << inc; l != 0; l--) {
            WORD16 temp;
            temp = ixheaac_extract16l(ixheaac_shr32_dir(*ptr, pre_shift1));
            ptr += 64;
            accu_line = ixheaac_mac16x16in32_sat(accu_line, temp, temp);
          }
        }
        accumulate =
            ixheaac_add32_sat(accumulate, ixheaac_shr32(accu_line, 9));
      }

      shift = ixheaac_pnorm32(accumulate);

      sum_m = ixheaac_extract16l(
          ixheaac_shr32_dir_sat_limit(accumulate, (16 - shift)));

      if (sum_m == 0) {
        sum_e = 0;
      } else {
        sum_m = ixheaac_mult16_shl_sat(sum_m, inv_width);

        sum_m = ixheaac_mult16_shl_sat(
            sum_m,
            ptr_sbr_tables->env_calc_tables_ptr->sbr_inv_int_table[ui - li]);

        sum_e = ((frame_exp + extra_shift) - shift);

        sum_e = (sum_e - (pre_shift_val << 1));
      }

      for (k = li; k < ui; k++) {
        *nrg_est++ = sum_m;
        *nrg_est++ = (WORD16)sum_e;
      }
    }
  }
}

VOID ixheaacd_subbandgain_calc(WORD16 e_orig_mant_matrix, WORD16 tmp_noise_mant,
                               WORD16 nrg_est_mant, WORD16 nrg_est_exp,
                               WORD16 tmp_noise_exp, WORD16 nrg_ref_exp,
                               FLAG sine_present_flag, FLAG sine_mapped_matrix,
                               FLAG noise_absc_flag, WORD16 *ptr_nrg_gain_mant,
                               WORD16 *ptr_noise_floor_mant,
                               WORD16 *ptr_nrg_sine_m,
                               ixheaacd_misc_tables *pstr_common_tables) {
  WORD16 var1_mant;
  WORD16 var1_exp;
  WORD16 var2_mant;
  WORD16 var2_exp;
  WORD16 var3_mant;
  WORD16 var3_exp;
  WORD32 temp;

  if (nrg_est_mant == 0) {
    nrg_est_mant = 0x4000;
    nrg_est_exp = 1;
  }

  var1_mant = ixheaac_mult16_shl_sat(e_orig_mant_matrix, tmp_noise_mant);
  var1_exp = (nrg_ref_exp + tmp_noise_exp);

  {
    WORD32 accu, exp_diff;

    exp_diff = tmp_noise_exp - 1;

    if (exp_diff >= 0) {
      accu = tmp_noise_mant + ixheaac_shr32(0x4000, exp_diff);
      var2_exp = tmp_noise_exp;
    } else {
      exp_diff = -exp_diff;
      accu = ixheaac_shr32((WORD32)tmp_noise_mant, exp_diff) + 0x4000;
      var2_exp = 1;
    }
    if (ixheaac_abs32(accu) >= 0x8000) {
      accu = accu >> 1;
      var2_exp++;
    }
    var2_mant = (WORD16)(accu);
  }

  temp = ixheaacd_fix_mant_div(var1_mant, var2_mant, ptr_noise_floor_mant,
                               pstr_common_tables);
  *(ptr_noise_floor_mant + 1) = temp + (var1_exp - var2_exp) + 1;

  if (sine_present_flag || !noise_absc_flag) {
    var3_mant = ixheaac_mult16_shl_sat(var2_mant, nrg_est_mant);
    var3_exp = (var2_exp + nrg_est_exp);
  } else {
    var3_mant = nrg_est_mant;
    var3_exp = nrg_est_exp;
  }

  if (sine_present_flag == 0) {
    var1_mant = e_orig_mant_matrix;
    var1_exp = nrg_ref_exp;
  }

  temp = ixheaacd_fix_mant_div(var1_mant, var3_mant, ptr_nrg_gain_mant,
                               pstr_common_tables);
  *(ptr_nrg_gain_mant + 1) = temp + (var1_exp - var3_exp) + 1;

  if (sine_present_flag && sine_mapped_matrix) {
    temp = ixheaacd_fix_mant_div(e_orig_mant_matrix, var2_mant, ptr_nrg_sine_m,
                                 pstr_common_tables);
    *(ptr_nrg_sine_m + 1) = temp + (nrg_ref_exp - var2_exp) + 1;
  }
}

VOID ixheaacd_avggain_calc(WORD16 *ptr_enrg_orig, WORD16 *nrg_est,
                           WORD32 sub_band_start, WORD32 sub_band_end,
                           WORD16 *ptr_enrg_orig_mant, WORD16 *ptr_sum_ref_exp,
                           WORD16 *ptr_avg_gain_mant, WORD16 *ptr_avg_gain_exp,
                           ixheaacd_misc_tables *pstr_common_tables,
                           WORD32 flag) {
  WORD16 sum_orig_mant;
  WORD16 sum_orig_exp;
  WORD16 sum_est_mant;
  WORD16 sum_est_exp;

  WORD32 accu_sum_orig_mant;
  WORD32 accu_sum_orig_exp;
  WORD32 accu_sum_est_mant;
  WORD32 accu_sum_est_exp;

  WORD32 k, temp;
  WORD16 *ptr_enrg_orig_buf;
  WORD16 *ptr_enrg_est_buf;

  {
    accu_sum_orig_mant = 0;
    accu_sum_orig_exp = 0;

    accu_sum_est_mant = 0;
    accu_sum_est_exp = 0;
  }

  ptr_enrg_orig_buf = &ptr_enrg_orig[sub_band_start << 1];
  ptr_enrg_est_buf = &nrg_est[sub_band_start << 1];

  for (k = sub_band_end - sub_band_start; k != 0; k--) {
    WORD16 tmp_mant, tmp_e;
    WORD16 tmp2_m, tmp2_e;

    tmp_mant = *ptr_enrg_orig_buf++;
    tmp_e = *ptr_enrg_orig_buf++;
    tmp2_m = *ptr_enrg_est_buf++;
    tmp2_e = *ptr_enrg_est_buf++;
    {
      WORD32 exp_diff;
      exp_diff = tmp_e - accu_sum_orig_exp;
      if (exp_diff >= 0) {
        accu_sum_orig_mant =
            tmp_mant + ixheaac_shr32(accu_sum_orig_mant, exp_diff);
        accu_sum_orig_exp = tmp_e;
      } else {
        exp_diff = -exp_diff;
        accu_sum_orig_mant =
            ixheaac_shr32(tmp_mant, exp_diff) + accu_sum_orig_mant;
      }
    }
    if (flag) {
      tmp_mant = (tmp_mant * tmp2_m) >> 16;
      tmp_e = (tmp_e + tmp2_e + 1);

    } else {
      tmp_mant = tmp2_m;
      tmp_e = tmp2_e;
    }

    {
      WORD32 exp_diff;
      exp_diff = tmp_e - accu_sum_est_exp;
      if (exp_diff >= 0) {
        accu_sum_est_mant =
            tmp_mant + ixheaac_shr32(accu_sum_est_mant, exp_diff);
        accu_sum_est_exp = tmp_e;
      } else {
        exp_diff = -exp_diff;
        accu_sum_est_mant =
            ixheaac_shr32(tmp_mant, exp_diff) + accu_sum_est_mant;
      }
    }
  }
  {
    WORD32 norm_val;
    norm_val = 16 - ixheaac_pnorm32(accu_sum_orig_mant);
    if (norm_val > 0) {
      accu_sum_orig_mant >>= norm_val;
      accu_sum_orig_exp += norm_val;
    }
    norm_val = 16 - ixheaac_pnorm32(accu_sum_est_mant);
    if (norm_val > 0) {
      accu_sum_est_mant >>= norm_val;
      accu_sum_est_exp += norm_val;
    }
  }

  if (!flag) {
    sum_orig_mant = (WORD16)accu_sum_orig_mant;
    sum_orig_exp = (WORD16)accu_sum_orig_exp;
    sum_est_mant = (WORD16)accu_sum_est_mant;
    sum_est_exp = (WORD16)accu_sum_est_exp;
  } else {
    sum_est_mant = (WORD16)accu_sum_orig_mant;
    sum_est_exp = (WORD16)accu_sum_orig_exp;
    sum_orig_mant = (WORD16)accu_sum_est_mant;
    sum_orig_exp = (WORD16)accu_sum_est_exp;
  }

  {
    temp = ixheaacd_fix_mant_div(sum_orig_mant, sum_est_mant, ptr_avg_gain_mant,
                                 pstr_common_tables);
    *ptr_avg_gain_exp = temp + (sum_orig_exp - sum_est_exp) + 1;
    *ptr_enrg_orig_mant = sum_orig_mant;
    *ptr_sum_ref_exp = sum_orig_exp;
  }
}

VOID ixheaacd_harm_idx_zerotwolp_dec(WORD32 *ptr_real_buf, WORD16 *ptr_gain_buf,
                                     WORD32 scale_change,
                                     WORD16 *ptr_sine_level_buf,
                                     const WORD32 *ptr_rand_ph,
                                     WORD16 *noise_level_mant,
                                     WORD32 num_sub_bands, FLAG noise_absc_flag,
                                     WORD32 harm_index) {
  WORD32 shift, k;
  WORD32 signal_real;
  WORD32 sine_level;

  scale_change = scale_change - 1;
  if (!noise_absc_flag) {
    for (k = 0; k < num_sub_bands; k++) {
      signal_real = ixheaac_mult32x16in32(*ptr_real_buf, *ptr_gain_buf++);
      shift = (*ptr_gain_buf++ - scale_change);

      if (shift > 0)
        signal_real = (signal_real << shift);
      else
        signal_real = (signal_real >> -(shift));

      sine_level = (ptr_sine_level_buf[2 * k] << 16);

      if (sine_level == 0) {
        *ptr_real_buf++ = ixheaac_mac16x16in32_shl_sat(
            signal_real, ixheaac_extract16h(ptr_rand_ph[k]),
            noise_level_mant[2 * k]);
      } else if (harm_index == 0)
        *ptr_real_buf++ = ixheaac_add32_sat(signal_real, sine_level);
      else
        *ptr_real_buf++ = ixheaac_sub32_sat(signal_real, sine_level);
    }
  } else {
    for (k = 0; k < num_sub_bands; k++) {
      signal_real = ixheaac_mult32x16in32(*ptr_real_buf, *ptr_gain_buf++);
      shift = (*ptr_gain_buf++ - scale_change);

      if (shift > 0)
        signal_real = (signal_real << shift);
      else
        signal_real = (signal_real >> -(shift));

      sine_level = (ptr_sine_level_buf[2 * k] << 16);

      if (harm_index == 0)
        *ptr_real_buf++ = ixheaac_add32_sat(signal_real, sine_level);
      else
        *ptr_real_buf++ = ixheaac_sub32_sat(signal_real, sine_level);
    }
  }
}

VOID ixheaacd_harm_idx_onethreelp(
    WORD32 *ptr_real_buf, WORD16 *ptr_gain_buf, WORD32 scale_change,
    WORD16 *ptr_sine_level_buf, const WORD32 *ptr_rand_ph,
    WORD16 *noise_level_mant, WORD32 num_sub_bands, FLAG noise_absc_flag,
    WORD32 freq_inv_flag, WORD32 noise_e, WORD32 sub_band_start) {
  WORD32 shift, k = 0;
  WORD32 signal_real, temp_mult, temp_mult2;
  WORD16 sine_level, sine_level_prev, sine_level_next;
  WORD32 tone_count = 0;
  WORD16 tmp;

  scale_change = scale_change - 1;

  signal_real = ixheaac_mult32x16in32(*ptr_real_buf, *ptr_gain_buf++);
  shift = (*ptr_gain_buf++ - scale_change);

  if (shift > 0)
    signal_real = (signal_real << shift);
  else
    signal_real = (signal_real >> -(shift));

  sine_level = ((ptr_sine_level_buf[2 * 0]));

  if (num_sub_bands > 1) {
    sine_level_next = ((ptr_sine_level_buf[2 * 1]));
  } else {
    sine_level_next = 0;
  }

  if (ptr_sine_level_buf[2 * 0] != 0) {
    tone_count++;
  } else {
    if (!noise_absc_flag) {
      signal_real = ixheaac_mac16x16in32_shl_sat(
          signal_real, ixheaac_extract16h(ptr_rand_ph[k]), *noise_level_mant);
    }
  }

  noise_level_mant += 2;
  temp_mult2 = ixheaac_mult32x16in32(FACTOR, sine_level_next);
  temp_mult = ixheaac_mult32x16in32(FACTOR, sine_level);
  tmp = noise_e;

  if (tmp > 0) {
    temp_mult = ixheaac_shl32(temp_mult, tmp);
  } else {
    temp_mult = ixheaac_shr32(temp_mult, -tmp);
  }

  if (freq_inv_flag < 0) {
    *(ptr_real_buf - 1) = ixheaac_add32_sat(*(ptr_real_buf - 1), temp_mult);
    signal_real = ixheaac_sub32_sat(signal_real, temp_mult2);
  } else {
    *(ptr_real_buf - 1) = ixheaac_sub32_sat(*(ptr_real_buf - 1), temp_mult);
    signal_real = ixheaac_add32_sat(signal_real, temp_mult2);
  }
  *ptr_real_buf++ = signal_real;

  num_sub_bands = num_sub_bands - 1;
  for (k = 1; k < num_sub_bands; k++) {
    WORD16 gain_m = *ptr_gain_buf++;
    WORD16 gain_e = *ptr_gain_buf++;
    WORD32 q_real = *ptr_real_buf;

    signal_real = ixheaac_mult32x16in32(q_real, gain_m);

    if ((shift = (gain_e - scale_change)) >= 0)
      signal_real = (signal_real << shift);
    else
      signal_real = (signal_real >> -(shift));

    sine_level_prev = sine_level;
    sine_level = sine_level_next;
    if (sine_level != 0) {
      tone_count++;
    }
    sine_level_next = (ptr_sine_level_buf[2 * (k + 1)]);

    if ((!noise_absc_flag) && (sine_level == 0)) {
      signal_real = ixheaac_mac16x16in32_shl_sat(
          signal_real, ixheaac_extract16h(ptr_rand_ph[k]), *noise_level_mant);
    }
    noise_level_mant += 2;

    if (tone_count <= 16) {
      WORD32 temp_mult;
      WORD32 add_sine = ixheaac_mult32x16in32(
          FACTOR, ixheaac_sub16(sine_level_prev, sine_level_next));
      temp_mult = add_sine * freq_inv_flag;
      signal_real = ixheaac_add32_sat(signal_real, temp_mult);
    }
    *ptr_real_buf++ = signal_real;
    freq_inv_flag = -(freq_inv_flag);
  }

  freq_inv_flag = (freq_inv_flag + 1) >> 1;

  if (num_sub_bands > 0) {
    WORD32 temp_mult_sine;
    signal_real = ixheaac_mult32x16in32(*ptr_real_buf, *ptr_gain_buf++);
    shift = (*ptr_gain_buf - scale_change);

    if (shift > 0)
      signal_real = (signal_real << shift);
    else
      signal_real = (signal_real >> -(shift));

    temp_mult_sine = ixheaac_mult32x16in32(FACTOR, sine_level);
    sine_level = sine_level_next;

    if (sine_level != 0) {
      tone_count++;
    } else {
      if (!noise_absc_flag) {
        signal_real = ixheaac_mac16x16in32_shl_sat(
            signal_real, ixheaac_extract16h(ptr_rand_ph[k]),
            *noise_level_mant);
      }
    }

    if (tone_count <= 16) {
      temp_mult2 = ixheaac_mult32x16in32(FACTOR, sine_level);

      if (freq_inv_flag) {
        *ptr_real_buf++ = ixheaac_add32_sat(signal_real, temp_mult_sine);

        if ((k + sub_band_start) < 62) {
          *ptr_real_buf = ixheaac_sub32_sat(*ptr_real_buf, temp_mult2);
        }
      } else {
        *ptr_real_buf++ = ixheaac_sub32_sat(signal_real, temp_mult_sine);

        if ((k + sub_band_start) < 62) {
          *ptr_real_buf = ixheaac_add32_sat(*ptr_real_buf, temp_mult2);
        }
      }
    } else {
      *ptr_real_buf = signal_real;
    }
  }
}

VOID ixheaacd_harm_idx_zerotwo(FLAG noise_absc_flag, WORD16 num_sub_bands,
                               WORD32 *ptr_real_buf, WORD32 *ptr_imag,
                               WORD16 *smoothed_gain, WORD16 *smoothed_noise,
                               WORD32 factor, WORD16 *ptr_gain_buf,
                               WORD16 scale_change, const WORD32 *ptr_rand_ph,
                               WORD16 *ptr_sine_level_buf, WORD16 noise_e,
                               WORD32 harm_index) {
  WORD32 k;
  WORD32 signal_real, sig_imag;
  WORD32 shift;
  WORD32 sine_level;
  ptr_gain_buf++;

  for (k = 0; k < num_sub_bands; k++) {
    signal_real = ixheaac_mult32x16in32(*ptr_real_buf, smoothed_gain[0]);
    sig_imag = ixheaac_mult32x16in32(*ptr_imag, smoothed_gain[0]);

    shift = ixheaac_sub16(*ptr_gain_buf, scale_change);
    ptr_gain_buf += 2;

    if (shift > 0) {
      signal_real = ixheaac_shl32(signal_real, shift);
      sig_imag = ixheaac_shl32(sig_imag, shift);
    } else {
      shift = -shift;
      signal_real = ixheaac_shr32(signal_real, shift);
      sig_imag = ixheaac_shr32(sig_imag, shift);
    }

    ptr_rand_ph++;

    if (*ptr_sine_level_buf != 0) {
      WORD32 tmp = ixheaac_sub16(ptr_sine_level_buf[1], noise_e);

      if (tmp > 0)
        sine_level = ixheaac_shl32(ptr_sine_level_buf[0], tmp);
      else
        sine_level = ixheaac_shr32(ptr_sine_level_buf[0], tmp);

      if (harm_index == 0)
        *ptr_real_buf = ixheaac_add32_sat(signal_real, sine_level);
      else
        *ptr_real_buf = ixheaac_sub32_sat(signal_real, sine_level);

      *ptr_imag = sig_imag;
    } else {
      if (!noise_absc_flag) {
        WORD32 random = *ptr_rand_ph;
        WORD16 noise = smoothed_noise[0];

        *ptr_real_buf = ixheaac_mac16x16in32_shl_sat(
            signal_real, ixheaac_extract16h(random), noise);
        *ptr_imag = ixheaac_mac16x16in32_shl_sat(
            sig_imag, ixheaac_extract16l(random), noise);
      } else {
        *ptr_real_buf = signal_real;
        *ptr_imag = sig_imag;
      }
    }

    smoothed_noise += factor;
    smoothed_gain += 2;
    ptr_sine_level_buf += 2;
    ptr_real_buf++;
    ptr_imag++;
  }
}

VOID ixheaacd_harm_idx_onethree(FLAG noise_absc_flag, WORD16 num_sub_bands,
                                WORD32 *ptr_real_buf, WORD32 *ptr_imag,
                                WORD16 *smoothed_gain, WORD16 *smoothed_noise,
                                WORD32 factor, WORD16 *ptr_gain_buf,
                                WORD16 scale_change, const WORD32 *ptr_rand_ph,
                                WORD16 *ptr_sine_level_buf, WORD16 noise_e,
                                WORD32 freq_inv_flag, WORD32 harm_index) {
  WORD32 k;
  WORD32 signal_real, sig_imag;
  WORD32 shift;
  WORD32 sine_level;

  ptr_gain_buf++;

  if (harm_index == 1) freq_inv_flag = !freq_inv_flag;

  for (k = 0; k < num_sub_bands; k++) {
    signal_real = ixheaac_mult32x16in32(*ptr_real_buf, smoothed_gain[0]);
    sig_imag = ixheaac_mult32x16in32(*ptr_imag, smoothed_gain[0]);

    shift = ixheaac_sub16(*ptr_gain_buf, scale_change);
    ptr_gain_buf += 2;

    if (shift > 0) {
      signal_real = ixheaac_shl32(signal_real, shift);
      sig_imag = ixheaac_shl32(sig_imag, shift);
    } else {
      shift = -shift;
      signal_real = ixheaac_shr32(signal_real, shift);
      sig_imag = ixheaac_shr32(sig_imag, shift);
    }

    ptr_rand_ph++;

    if (*ptr_sine_level_buf != 0) {
      WORD32 tmp = ixheaac_sub16(ptr_sine_level_buf[1], noise_e);

      if (tmp > 0)
        sine_level = ixheaac_shl32(ptr_sine_level_buf[0], tmp);
      else
        sine_level = ixheaac_shr32(ptr_sine_level_buf[0], -tmp);

      *ptr_real_buf = signal_real;

      if (freq_inv_flag) {
        *ptr_imag = ixheaac_add32_sat(sig_imag, sine_level);
      } else {
        *ptr_imag = ixheaac_sub32_sat(sig_imag, sine_level);
      }

    } else {
      if (!noise_absc_flag) {
        WORD32 random = *ptr_rand_ph;
        WORD16 noise = smoothed_noise[0];

        *ptr_real_buf = ixheaac_mac16x16in32_shl_sat(
            signal_real, ixheaac_extract16h(random), noise);
        *ptr_imag = ixheaac_mac16x16in32_shl_sat(
            sig_imag, ixheaac_extract16l(random), noise);
      } else {
        *ptr_real_buf = signal_real;
        *ptr_imag = sig_imag;
      }
    }

    freq_inv_flag = (!freq_inv_flag);
    smoothed_gain += 2;
    smoothed_noise += factor;
    ptr_sine_level_buf += 2;
    ptr_real_buf++;
    ptr_imag++;
  }
}