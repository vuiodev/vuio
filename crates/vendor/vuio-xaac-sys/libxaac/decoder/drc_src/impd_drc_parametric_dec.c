/******************************************************************************
 *
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
#include <stdlib.h>
#include <stdio.h>
#include <math.h>
#include <string.h>
#include "impd_type_def.h"
#include "impd_drc_extr_delta_coded_info.h"
#include "ixheaac_constants.h"
#include "impd_drc_common.h"
#include "impd_drc_struct.h"
#include "impd_parametric_drc_dec.h"
#include "impd_drc_filter_bank.h"
#include "impd_drc_rom.h"

#define PI 3.14159265f

#ifndef max
#define max(a, b) (((a) > (b)) ? (a) : (b))
#endif
#ifndef min
#define min(a, b) (((a) < (b)) ? (a) : (b))
#endif

WORD32 impd_init_parametric_drc(
    WORD32 drc_frame_size, WORD32 sampling_rate, WORD32 sub_band_domain_mode,
    ia_parametric_drc_params_struct* p_parametricdrc_params) {
  static const WORD32 sub_band_count_tbl[4] = {0, 64, 71, 256};
  p_parametricdrc_params->drc_frame_size = drc_frame_size;
  p_parametricdrc_params->sampling_rate = sampling_rate;
  p_parametricdrc_params->sub_band_domain_mode = sub_band_domain_mode;

  p_parametricdrc_params->sub_band_count =
      sub_band_count_tbl[sub_band_domain_mode];

  return 0;
}

WORD32 impd_init_parametric_drc_feed_fwd(
    ia_drc_config* pstr_drc_config, WORD32 instance_idx,
    WORD32 ch_count_from_dwnmix_id,
    ia_parametric_drc_params_struct* p_parametricdrc_params) {
  WORD32 err = 0, i = 0;

  WORD32 parametric_drc_idx =
      p_parametricdrc_params->parametric_drc_idx[instance_idx];
  WORD32 gain_set_index = p_parametricdrc_params->gain_set_index[instance_idx];
  WORD32* channel_map = p_parametricdrc_params->channel_map[instance_idx];

  ia_drc_coeff_parametric_drc_struct* hDrcCoefficientsParametricDrcBs =
      &(pstr_drc_config->str_drc_config_ext.str_drc_coeff_param_drc);
  ia_parametric_drc_type_feed_forward_struct* hParametricDrcTypeFeedForwardBs =
      &(pstr_drc_config->str_drc_config_ext
            .str_parametric_drc_instructions[parametric_drc_idx]
            .str_parametric_drc_type_feed_forward);
  ia_parametric_drc_type_ff_params_struct*
      pstr_parametric_ffwd_type_drc_params =
          &(p_parametricdrc_params
                ->str_parametric_drc_instance_params[instance_idx]
                .str_parametric_drc_type_ff_params);

  /* level estimation */
  pstr_parametric_ffwd_type_drc_params->frame_size =
      p_parametricdrc_params->parametric_drc_frame_size;
  pstr_parametric_ffwd_type_drc_params->sub_band_domain_mode =
      p_parametricdrc_params->sub_band_domain_mode;
  pstr_parametric_ffwd_type_drc_params->sub_band_count =
      p_parametricdrc_params->sub_band_count;
  pstr_parametric_ffwd_type_drc_params->sub_band_compensation_type = 0;

  if (pstr_parametric_ffwd_type_drc_params->sub_band_domain_mode ==
      SUBBAND_DOMAIN_MODE_QMF64) {
    if (p_parametricdrc_params->sampling_rate == 48000) {
      pstr_parametric_ffwd_type_drc_params->sub_band_compensation_type = 1;
    } else {
      /* support of other sampling rates than 48000 might be missing */
      return UNEXPECTED_ERROR;
    }
  }

  pstr_parametric_ffwd_type_drc_params->audio_num_chan =
      p_parametricdrc_params->audio_num_chan;
  pstr_parametric_ffwd_type_drc_params->level_estim_k_weighting_type =
      hParametricDrcTypeFeedForwardBs->level_estim_k_weighting_type;
  pstr_parametric_ffwd_type_drc_params->level_estim_integration_time =
      hParametricDrcTypeFeedForwardBs->level_estim_integration_time;
  pstr_parametric_ffwd_type_drc_params->level_estim_frame_index = 0;
  pstr_parametric_ffwd_type_drc_params->level_estim_frame_count =
      hParametricDrcTypeFeedForwardBs->level_estim_integration_time /
      pstr_parametric_ffwd_type_drc_params->frame_size;

  memset(pstr_parametric_ffwd_type_drc_params->level, 0,
         PARAM_DRC_TYPE_FF_LEVEL_ESTIM_FRAME_COUNT_MAX * sizeof(FLOAT32));

  if (ch_count_from_dwnmix_id != 0) {
    memcpy(pstr_parametric_ffwd_type_drc_params->level_estim_ch_weight,
           hDrcCoefficientsParametricDrcBs
               ->str_parametric_drc_gain_set_params[gain_set_index]
               .level_estim_ch_weight,
           ch_count_from_dwnmix_id * sizeof(FLOAT32));
  } else {
    for (i = 0; i < pstr_parametric_ffwd_type_drc_params->audio_num_chan; i++) {
      pstr_parametric_ffwd_type_drc_params->level_estim_ch_weight[i] =
          (FLOAT32)channel_map[i];
    }
  }

  if (pstr_parametric_ffwd_type_drc_params->sub_band_domain_mode ==
      SUBBAND_DOMAIN_MODE_OFF) {
    err = impd_init_lvl_est_filt_time(
        pstr_parametric_ffwd_type_drc_params->level_estim_k_weighting_type,
        p_parametricdrc_params->sampling_rate,
        &pstr_parametric_ffwd_type_drc_params->pre_filt_coeff,
        &pstr_parametric_ffwd_type_drc_params->rlb_filt_coeff);

    if (err) return (err);
  } else {
    err = impd_init_lvl_est_filt_subband(
        pstr_parametric_ffwd_type_drc_params->level_estim_k_weighting_type,
        p_parametricdrc_params->sampling_rate,
        p_parametricdrc_params->sub_band_domain_mode,
        p_parametricdrc_params->sub_band_count,
        pstr_parametric_ffwd_type_drc_params->sub_band_compensation_type,
        pstr_parametric_ffwd_type_drc_params->weighting_filt,
        &pstr_parametric_ffwd_type_drc_params->filt_coeff_subband);

    if (err) return (err);
  }

  pstr_parametric_ffwd_type_drc_params->node_count =
      hParametricDrcTypeFeedForwardBs->node_count;

  memcpy(pstr_parametric_ffwd_type_drc_params->node_level,
         hParametricDrcTypeFeedForwardBs->node_level,
         pstr_parametric_ffwd_type_drc_params->node_count * sizeof(WORD32));
  memcpy(pstr_parametric_ffwd_type_drc_params->node_gain,
         hParametricDrcTypeFeedForwardBs->node_gain,
         pstr_parametric_ffwd_type_drc_params->node_count * sizeof(WORD32));

  pstr_parametric_ffwd_type_drc_params->ref_level_parametric_drc =
      hDrcCoefficientsParametricDrcBs
          ->str_parametric_drc_gain_set_params[gain_set_index]
          .drc_input_loudness;

  {
    WORD32 gain_smooth_attack_time_fast =
        hParametricDrcTypeFeedForwardBs->gain_smooth_attack_time_fast;
    WORD32 gain_smooth_release_time_fast =
        hParametricDrcTypeFeedForwardBs->gain_smooth_release_time_fast;
    WORD32 gain_smooth_attack_time_slow =
        hParametricDrcTypeFeedForwardBs->gain_smooth_attack_time_slow;
    WORD32 gain_smooth_release_time_slow =
        hParametricDrcTypeFeedForwardBs->gain_smooth_release_time_slow;
    WORD32 gain_smooth_hold_off =
        hParametricDrcTypeFeedForwardBs->gain_smooth_hold_off;
    WORD32 sampling_rate = p_parametricdrc_params->sampling_rate;
    WORD32 parametric_drc_frame_size =
        p_parametricdrc_params->parametric_drc_frame_size;

    pstr_parametric_ffwd_type_drc_params->gain_smooth_attack_alpha_fast =
        1 -
        (FLOAT32)exp(-1.0 * parametric_drc_frame_size /
                     (gain_smooth_attack_time_fast * sampling_rate * 0.001));
    pstr_parametric_ffwd_type_drc_params->gain_smooth_rel_alpha_fast =
        1 -
        (FLOAT32)exp(-1.0 * parametric_drc_frame_size /
                     (gain_smooth_release_time_fast * sampling_rate * 0.001));
    pstr_parametric_ffwd_type_drc_params->gain_smooth_attack_alpha_slow =
        1 -
        (FLOAT32)exp(-1.0 * parametric_drc_frame_size /
                     (gain_smooth_attack_time_slow * sampling_rate * 0.001));
    pstr_parametric_ffwd_type_drc_params->gain_smooth_rel_alpha_slow =
        1 -
        (FLOAT32)exp(-1.0 * parametric_drc_frame_size /
                     (gain_smooth_release_time_slow * sampling_rate * 0.001));
    pstr_parametric_ffwd_type_drc_params->gain_smooth_hold_off_count =
        gain_smooth_hold_off * 256 * sampling_rate /
        (parametric_drc_frame_size * 48000);
    pstr_parametric_ffwd_type_drc_params->gain_smooth_attack_threshold =
        hParametricDrcTypeFeedForwardBs->gain_smooth_attack_threshold;
    pstr_parametric_ffwd_type_drc_params->gain_smooth_rel_threshold =
        hParametricDrcTypeFeedForwardBs->gain_smooth_rel_threshold;
  }

  err =
      impd_parametric_ffwd_type_drc_reset(pstr_parametric_ffwd_type_drc_params);

  if (err) return (err);

  return 0;
}

VOID impd_init_parametric_drc_lim(
    ia_drc_config* pstr_drc_config, WORD32 instance_idx,
    WORD32 ch_count_from_dwnmix_id,
    ia_parametric_drc_params_struct* p_parametricdrc_params, pVOID* mem_ptr) {
  WORD32 i = 0;
  UWORD32 j;
  UWORD32 attack, sec_len;

  WORD32 parametric_drc_idx =
      p_parametricdrc_params->parametric_drc_idx[instance_idx];
  WORD32 gain_set_index = p_parametricdrc_params->gain_set_index[instance_idx];
  WORD32* channel_map = p_parametricdrc_params->channel_map[instance_idx];

  ia_drc_coeff_parametric_drc_struct* hDrcCoefficientsParametricDrcBs =
      &(pstr_drc_config->str_drc_config_ext.str_drc_coeff_param_drc);
  ia_parametric_drc_lim_struct* hParametricDrcTypeLimBs =
      &(pstr_drc_config->str_drc_config_ext
            .str_parametric_drc_instructions[parametric_drc_idx]
            .parametric_drc_lim);
  ia_parametric_drc_type_lim_params_struct*
      pstr_parametric_lim_type_drc_params =
          &(p_parametricdrc_params
                ->str_parametric_drc_instance_params[instance_idx]
                .str_parametric_drc_type_lim_params);

  pstr_parametric_lim_type_drc_params->frame_size =
      p_parametricdrc_params->drc_frame_size;
  pstr_parametric_lim_type_drc_params->audio_num_chan =
      p_parametricdrc_params->audio_num_chan;

  if (ch_count_from_dwnmix_id != 0) {
    memcpy(pstr_parametric_lim_type_drc_params->level_estim_ch_weight,
           hDrcCoefficientsParametricDrcBs
               ->str_parametric_drc_gain_set_params[gain_set_index]
               .level_estim_ch_weight,
           ch_count_from_dwnmix_id * sizeof(FLOAT32));
  } else {
    for (i = 0; i < pstr_parametric_lim_type_drc_params->audio_num_chan; i++) {
      pstr_parametric_lim_type_drc_params->level_estim_ch_weight[i] =
          (FLOAT32)channel_map[i];
    }
  }

  attack = (UWORD32)(hParametricDrcTypeLimBs->parametric_lim_attack *
                     p_parametricdrc_params->sampling_rate / 1000);

  sec_len = (UWORD32)sqrt(attack + 1);

  pstr_parametric_lim_type_drc_params->sec_len = sec_len;
  pstr_parametric_lim_type_drc_params->num_max_buf_sec = (attack + 1) / sec_len;
  if (pstr_parametric_lim_type_drc_params->num_max_buf_sec * sec_len <
      (attack + 1))
    pstr_parametric_lim_type_drc_params->num_max_buf_sec++;

  pstr_parametric_lim_type_drc_params->max_buf = (FLOAT32*)(*mem_ptr);
  *mem_ptr = (pVOID)((SIZE_T)(*mem_ptr) +
                     IXHEAAC_GET_SIZE_ALIGNED(
                         pstr_parametric_lim_type_drc_params->num_max_buf_sec * sec_len *
                             sizeof(pstr_parametric_lim_type_drc_params->max_buf[0]),
                         BYTE_ALIGN_8));

  pstr_parametric_lim_type_drc_params->attack_ms =
      (FLOAT32)hParametricDrcTypeLimBs->parametric_lim_attack;
  pstr_parametric_lim_type_drc_params->release_ms =
      (FLOAT32)hParametricDrcTypeLimBs->parametric_lim_release;
  pstr_parametric_lim_type_drc_params->attack = attack;
  pstr_parametric_lim_type_drc_params->attack_constant =
      (FLOAT32)pow(0.1, 1.0 / (attack + 1));
  pstr_parametric_lim_type_drc_params->release_constant = (FLOAT32)pow(
      0.1, 1.0 / (hParametricDrcTypeLimBs->parametric_lim_release *
                      p_parametricdrc_params->sampling_rate / 1000 +
                  1));
  pstr_parametric_lim_type_drc_params->threshold = (FLOAT32)pow(
      10.0f, 0.05f * hParametricDrcTypeLimBs->parametric_lim_threshold);
  pstr_parametric_lim_type_drc_params->channels =
      pstr_parametric_lim_type_drc_params->audio_num_chan;
  pstr_parametric_lim_type_drc_params->sampling_rate =
      p_parametricdrc_params->sampling_rate;
  pstr_parametric_lim_type_drc_params->cor = 1.0f;
  pstr_parametric_lim_type_drc_params->smooth_state_0 = 1.0;

  for (j = 0; j < pstr_parametric_lim_type_drc_params->num_max_buf_sec *
                      pstr_parametric_lim_type_drc_params->sec_len;
       j++) {
    pstr_parametric_lim_type_drc_params->max_buf[j] = 0.f;
  }
}

WORD32 impd_init_parametric_drcInstance(
    ia_drc_config* pstr_drc_config, WORD32 instance_idx,
    WORD32 ch_count_from_dwnmix_id,
    ia_parametric_drc_params_struct* p_parametricdrc_params, pVOID* mem_ptr) {
  WORD32 err = 0;

  WORD32 parametric_drc_idx =
      p_parametricdrc_params->parametric_drc_idx[instance_idx];
  ia_parametric_drc_instructions_struct* hParametricDrcInstructions =
      &(pstr_drc_config->str_drc_config_ext
            .str_parametric_drc_instructions[parametric_drc_idx]);

  p_parametricdrc_params->str_parametric_drc_instance_params[instance_idx]
      .disable_paramteric_drc =
      hParametricDrcInstructions->disable_paramteric_drc;
  p_parametricdrc_params->str_parametric_drc_instance_params[instance_idx]
      .parametric_drc_type = hParametricDrcInstructions->parametric_drc_type;
  p_parametricdrc_params->str_parametric_drc_instance_params[instance_idx]
      .str_spline_nodes.num_nodes = p_parametricdrc_params->num_nodes;

  if (p_parametricdrc_params->str_parametric_drc_instance_params[instance_idx]
          .disable_paramteric_drc == 0) {
    if (p_parametricdrc_params->str_parametric_drc_instance_params[instance_idx]
            .parametric_drc_type == PARAM_DRC_TYPE_FF) {
      err = impd_init_parametric_drc_feed_fwd(pstr_drc_config, instance_idx,
                                              ch_count_from_dwnmix_id,
                                              p_parametricdrc_params);

      if (err) return (err);

    } else if (p_parametricdrc_params
                   ->str_parametric_drc_instance_params[instance_idx]
                   .parametric_drc_type == PARAM_DRC_TYPE_LIM) {
      p_parametricdrc_params->str_parametric_drc_instance_params[instance_idx]
          .str_spline_nodes.num_nodes = p_parametricdrc_params->drc_frame_size;

      impd_init_parametric_drc_lim(pstr_drc_config, instance_idx,
                                   ch_count_from_dwnmix_id,
                                   p_parametricdrc_params, mem_ptr);

    } else {
      return (UNEXPECTED_ERROR);
    }
  }

  return 0;
}

WORD32 impd_init_parametric_drc_after_config(
    ia_drc_config* pstr_drc_config,
    ia_drc_loudness_info_set_struct* pstr_loudness_info,
    ia_parametric_drc_params_struct* p_parametricdrc_params, pVOID* mem_ptr) {
  WORD32 err = 0, instance_idx = 0, gain_set_index = 0,
         side_chain_config_type = 0, downmix_id = 0,
         ch_count_from_dwnmix_id = 0, L = 0;

  p_parametricdrc_params->parametric_drc_frame_size =
      pstr_drc_config->str_drc_config_ext.str_drc_coeff_param_drc
          .parametric_drc_frame_size;
  p_parametricdrc_params->reset_parametric_drc =
      pstr_drc_config->str_drc_config_ext.str_drc_coeff_param_drc
          .reset_parametric_drc;
  p_parametricdrc_params->num_nodes =
      p_parametricdrc_params->drc_frame_size /
      p_parametricdrc_params->parametric_drc_frame_size;

  switch (p_parametricdrc_params->sub_band_domain_mode) {
    case SUBBAND_DOMAIN_MODE_QMF64:
      L = AUDIO_CODEC_SUBBAND_DOWNSAMPLING_FACTOR_QMF64;
      break;
    case SUBBAND_DOMAIN_MODE_QMF71:
      L = AUDIO_CODEC_SUBBAND_DOWNSAMPLING_FACTOR_QMF71;
      break;
    case SUBBAND_DOMAIN_MODE_STFT256:
      L = AUDIO_CODEC_SUBBAND_DOWNSAMPLING_FACTOR_STFT256;
      break;
    case SUBBAND_DOMAIN_MODE_OFF:
    default:
      L = 0;
      break;
  }

  if (p_parametricdrc_params->sub_band_domain_mode != SUBBAND_DOMAIN_MODE_OFF &&
      p_parametricdrc_params->parametric_drc_frame_size != L) {
    return (EXTERNAL_ERROR);
  }

  for (instance_idx = 0;
       instance_idx < p_parametricdrc_params->parametric_drc_instance_count;
       instance_idx++) {
    gain_set_index = p_parametricdrc_params->gain_set_index[instance_idx];
    side_chain_config_type =
        pstr_drc_config->str_drc_config_ext.str_drc_coeff_param_drc
            .str_parametric_drc_gain_set_params[gain_set_index]
            .side_chain_config_type;
    downmix_id = pstr_drc_config->str_drc_config_ext.str_drc_coeff_param_drc
                     .str_parametric_drc_gain_set_params[gain_set_index]
                     .downmix_id;

    if (side_chain_config_type == 1 &&
        downmix_id ==
            p_parametricdrc_params
                ->dwnmix_id_from_drc_instructions[instance_idx]) {
      ch_count_from_dwnmix_id =
          pstr_drc_config->str_drc_config_ext.str_drc_coeff_param_drc
              .str_parametric_drc_gain_set_params[gain_set_index]
              .ch_count_from_dwnmix_id;
    } else {
      ch_count_from_dwnmix_id = 0;
    }

    if (pstr_drc_config->str_drc_config_ext.str_drc_coeff_param_drc
            .str_parametric_drc_gain_set_params[gain_set_index]
            .drc_input_loudness_present == 0) {
      WORD32 n = 0, m = 0, drcInputLoudnessFound = 0;
      FLOAT32 drc_input_loudness = 0.f;

      for (n = 0; n < pstr_loudness_info->loudness_info_count; n++) {
        ia_loudness_info_struct* loudness_info =
            &pstr_loudness_info->loudness_info[n];
        if (p_parametricdrc_params
                ->dwnmix_id_from_drc_instructions[instance_idx] ==
            loudness_info->downmix_id) {
          if (0 == loudness_info->drc_set_id) {
            for (m = 0; m < loudness_info->measurement_count; m++) {
              if (loudness_info->loudness_measure[m].method_def ==
                  METHOD_DEFINITION_PROGRAM_LOUDNESS) {
                drc_input_loudness =
                    loudness_info->loudness_measure[m].method_val;
                drcInputLoudnessFound = 1;
                break;
              }
            }
            if (drcInputLoudnessFound == 0) {
              for (m = 0; m < loudness_info->measurement_count; m++) {
                if (loudness_info->loudness_measure[m].method_def ==
                    METHOD_DEFINITION_ANCHOR_LOUDNESS) {
                  drc_input_loudness =
                      loudness_info->loudness_measure[m].method_val;
                  drcInputLoudnessFound = 1;
                  break;
                }
              }
            }
          }
        }
      }
      if (drcInputLoudnessFound == 0) {
        for (n = 0; n < pstr_loudness_info->loudness_info_count; n++) {
          ia_loudness_info_struct* loudness_info =
              &pstr_loudness_info->loudness_info[n];
          if (0 == loudness_info->downmix_id) {
            if (0 == loudness_info->drc_set_id) {
              for (m = 0; m < loudness_info->measurement_count; m++) {
                if (loudness_info->loudness_measure[m].method_def ==
                    METHOD_DEFINITION_PROGRAM_LOUDNESS) {
                  drc_input_loudness =
                      loudness_info->loudness_measure[m].method_val;
                  drcInputLoudnessFound = 1;
                  break;
                }
              }
              if (drcInputLoudnessFound == 0) {
                for (m = 0; m < loudness_info->measurement_count; m++) {
                  if (loudness_info->loudness_measure[m].method_def ==
                      METHOD_DEFINITION_ANCHOR_LOUDNESS) {
                    drc_input_loudness =
                        loudness_info->loudness_measure[m].method_val;
                    drcInputLoudnessFound = 1;
                    break;
                  }
                }
              }
            }
          }
        }
      }
      if (drcInputLoudnessFound == 0) {
        return (UNEXPECTED_ERROR);
      } else {
        pstr_drc_config->str_drc_config_ext.str_drc_coeff_param_drc
            .str_parametric_drc_gain_set_params[gain_set_index]
            .drc_input_loudness = drc_input_loudness;
      }
    }

    err = impd_init_parametric_drcInstance(pstr_drc_config, instance_idx,
                                           ch_count_from_dwnmix_id,
                                           p_parametricdrc_params, mem_ptr);
    if (err) return (err);
  }

  return 0;
}

WORD32 impd_init_lvl_est_filt_time(
    WORD32 level_estim_k_weighting_type, WORD32 sampling_rate,
    ia_2nd_order_filt_coeff_struct* pre_filt_coeff,
    ia_2nd_order_filt_coeff_struct* rlb_filt_coeff) {
  WORD32 i;
  const FLOAT32* ptr_samp_tbl;

  switch (sampling_rate) {
    case 96000:
      i = 0;
      break;
    case 88200:
      i = 1;
      break;
    case 64000:
      i = 2;
      break;
    case 48000:
      i = 3;
      break;
    case 44100:
      i = 4;
      break;
    case 32000:
      i = 5;
      break;
    case 24000:
      i = 6;
      break;
    case 22050:
      i = 7;
      break;
    case 16000:
      i = 8;
      break;
    case 12000:
      i = 9;
      break;
    case 11025:
      i = 10;
      break;
    case 8000:
      i = 11;
      break;
    case 7350:
      i = 12;
      break;
    default:
      i = 3;
      break;
  }

  ptr_samp_tbl = samp_rate_tbl[i];

  if (level_estim_k_weighting_type == 2) {
    pre_filt_coeff->b0 = ptr_samp_tbl[0];
    pre_filt_coeff->b1 = ptr_samp_tbl[1];
    pre_filt_coeff->b2 = ptr_samp_tbl[2];
    pre_filt_coeff->a1 = ptr_samp_tbl[3];
    pre_filt_coeff->a2 = ptr_samp_tbl[4];
  }

  if (level_estim_k_weighting_type == 1 || level_estim_k_weighting_type == 2) {
    rlb_filt_coeff->b0 = ptr_samp_tbl[5];
    rlb_filt_coeff->b1 = ptr_samp_tbl[6];
    rlb_filt_coeff->b2 = ptr_samp_tbl[7];
    rlb_filt_coeff->a1 = ptr_samp_tbl[8];
    rlb_filt_coeff->a2 = ptr_samp_tbl[9];
  }

  return 0;
}

WORD32 impd_init_lvl_est_filt_subband(
    WORD32 level_estim_k_weighting_type, WORD32 sampling_rate,
    WORD32 sub_band_domain_mode, WORD32 sub_band_count,
    WORD32 sub_band_compensation_type, FLOAT32* weighting_filt,
    ia_2nd_order_filt_coeff_struct* filt_coeff_subband) {
  FLOAT32 w0, alpha, sinw0, cosw0;
  FLOAT32 b0, b1, b2, a0, a1, a2;
  FLOAT32 num_real, num_imag, den_real, den_imag;
  const FLOAT32* f_bands_nrm;
  WORD32 b;
  WORD32 i;
  const FLOAT32* ptr_samp_tbl;

  switch (sampling_rate) {
    case 96000:
      i = 0;
      break;
    case 88200:
      i = 1;
      break;
    case 64000:
      i = 2;
      break;
    case 48000:
      i = 3;
      break;
    case 44100:
      i = 4;
      break;
    case 32000:
      i = 5;
      break;
    case 24000:
      i = 6;
      break;
    case 22050:
      i = 7;
      break;
    case 16000:
      i = 8;
      break;
    case 12000:
      i = 9;
      break;
    case 11025:
      i = 10;
      break;
    case 8000:
      i = 11;
      break;
    case 7350:
      i = 12;
      break;
    default:
      i = 3;
      break;
  }

  ptr_samp_tbl = samp_rate_tbl[i];

  switch (sub_band_domain_mode) {
    case SUBBAND_DOMAIN_MODE_QMF64:
      f_bands_nrm = f_bands_nrm_QMF64;
      break;
    case SUBBAND_DOMAIN_MODE_QMF71:
      f_bands_nrm = f_bands_nrm_QMF71;
      break;
    case SUBBAND_DOMAIN_MODE_STFT256:
      f_bands_nrm = f_bands_nrm_STFT256;
      break;
    default:
      return UNEXPECTED_ERROR;
      break;
  }

  for (b = 0; b < sub_band_count; b++) {
    weighting_filt[b] = 1.f;
  }

  if (level_estim_k_weighting_type == 2) {
    b0 = ptr_samp_tbl[0];
    b1 = ptr_samp_tbl[1];
    b2 = ptr_samp_tbl[2];
    a1 = ptr_samp_tbl[3];
    a2 = ptr_samp_tbl[4];
    a0 = 1.f;

    for (b = 0; b < sub_band_count; b++) {
      num_real = b0 + b1 * (FLOAT32)cos(PI * f_bands_nrm[b]) +
                 b2 * (FLOAT32)cos(PI * 2 * f_bands_nrm[b]);
      num_imag = -b1 * (FLOAT32)sin(PI * f_bands_nrm[b]) -
                 b2 * (FLOAT32)sin(PI * 2 * f_bands_nrm[b]);
      den_real = a0 + a1 * (FLOAT32)cos(PI * f_bands_nrm[b]) +
                 a2 * (FLOAT32)cos(PI * 2 * f_bands_nrm[b]);
      den_imag = -a1 * (FLOAT32)sin(PI * f_bands_nrm[b]) -
                 a2 * (FLOAT32)sin(PI * 2 * f_bands_nrm[b]);

      weighting_filt[b] *=
          (FLOAT32)(sqrt((num_real * num_real + num_imag * num_imag) /
                         (den_real * den_real + den_imag * den_imag)));
    }
  }

  if (level_estim_k_weighting_type == 1 || level_estim_k_weighting_type == 2) {
    b0 = ptr_samp_tbl[5];
    b1 = ptr_samp_tbl[6];
    b2 = ptr_samp_tbl[7];
    a1 = ptr_samp_tbl[8];
    a2 = ptr_samp_tbl[9];
    a0 = 1.f;

    for (b = 0; b < sub_band_count; b++) {
      if (!(sub_band_compensation_type == 1 && b == 0)) {
        num_real = (FLOAT32)(b0 + b1 * cos(PI * f_bands_nrm[b]) +
                             b2 * cos(PI * 2 * f_bands_nrm[b]));
        num_imag = (FLOAT32)(-b1 * sin(PI * f_bands_nrm[b]) -
                             b2 * sin(PI * 2 * f_bands_nrm[b]));
        den_real = (FLOAT32)(a0 + a1 * cos(PI * f_bands_nrm[b]) +
                             a2 * cos(PI * 2 * f_bands_nrm[b]));
        den_imag = (FLOAT32)(-a1 * sin(PI * f_bands_nrm[b]) -
                             a2 * sin(PI * 2 * f_bands_nrm[b]));

        weighting_filt[b] *=
            (FLOAT32)(sqrt((num_real * num_real + num_imag * num_imag) /
                           (den_real * den_real + den_imag * den_imag)));
      }
    }

    if (sub_band_compensation_type == 1) {
      w0 = 2.0f * PI * 38.0f / (FLOAT32)sampling_rate *
           AUDIO_CODEC_SUBBAND_DOWNSAMPLING_FACTOR_QMF64;
      sinw0 = (FLOAT32)sin(w0);
      cosw0 = (FLOAT32)cos(w0);
      alpha = sinw0;

      b0 = (1 + cosw0) / 2;
      b1 = -(1 + cosw0);
      b2 = (1 + cosw0) / 2;
      a0 = 1 + alpha;
      a1 = -2 * cosw0;
      a2 = 1 - alpha;

      filt_coeff_subband->b0 = b0 / a0;
      filt_coeff_subband->b1 = b1 / a0;
      filt_coeff_subband->b2 = b2 / a0;
      filt_coeff_subband->a1 = a1 / a0;
      filt_coeff_subband->a2 = a2 / a0;
    }
  }

  return 0;
}

WORD32 impd_parametric_ffwd_type_drc_reset(
    ia_parametric_drc_type_ff_params_struct*
        pstr_parametric_ffwd_type_drc_params) {
  WORD32 i = 0;

  pstr_parametric_ffwd_type_drc_params->level_estim_frame_index = 0;
  pstr_parametric_ffwd_type_drc_params->start_up_phase = 1;
  for (i = 0; i < PARAM_DRC_TYPE_FF_LEVEL_ESTIM_FRAME_COUNT_MAX; i++) {
    pstr_parametric_ffwd_type_drc_params->level[i] = 0.f;
  }

  for (i = 0; i < MAX_CHANNEL_COUNT; i++) {
    pstr_parametric_ffwd_type_drc_params->pre_filt_state[i].z1 = 0.f;
    pstr_parametric_ffwd_type_drc_params->pre_filt_state[i].z2 = 0.f;
    pstr_parametric_ffwd_type_drc_params->rlb_filt_state[i].z1 = 0.f;
    pstr_parametric_ffwd_type_drc_params->rlb_filt_state[i].z2 = 0.f;
    pstr_parametric_ffwd_type_drc_params->filt_state_subband_real[i].z1 = 0.f;
    pstr_parametric_ffwd_type_drc_params->filt_state_subband_real[i].z2 = 0.f;
    pstr_parametric_ffwd_type_drc_params->filt_state_subband_imag[i].z1 = 0.f;
    pstr_parametric_ffwd_type_drc_params->filt_state_subband_imag[i].z2 = 0.f;
  }

  pstr_parametric_ffwd_type_drc_params->db_level_smooth = -135.f;
  pstr_parametric_ffwd_type_drc_params->db_gain_smooth = 0.f;
  pstr_parametric_ffwd_type_drc_params->hold_counter = 0;

  return 0;
}

WORD32 impd_parametric_drc_instance_process(
    FLOAT32* audio_in_out_buf[], FLOAT32* audio_real_buff[],
    FLOAT32* audio_imag_buff[],
    ia_parametric_drc_params_struct* p_parametricdrc_params,
    ia_parametric_drc_instance_params_struct*
        pstr_parametric_drc_instance_params) {
  WORD32 err = 0, i = 0;

  if (pstr_parametric_drc_instance_params->disable_paramteric_drc) {
    for (i = 0; i < p_parametricdrc_params->num_nodes; i++) {
      pstr_parametric_drc_instance_params->str_spline_nodes.str_node[i]
          .loc_db_gain = 0.f;
      pstr_parametric_drc_instance_params->str_spline_nodes.str_node[i].slope =
          0.f;
      pstr_parametric_drc_instance_params->str_spline_nodes.str_node[i].time =
          (i + 1) * p_parametricdrc_params->parametric_drc_frame_size - 1;
    }

  } else {
    if (pstr_parametric_drc_instance_params->parametric_drc_type ==
        PARAM_DRC_TYPE_FF) {
      ia_parametric_drc_type_ff_params_struct*
          pstr_parametric_ffwd_type_drc_params =
              &(pstr_parametric_drc_instance_params
                    ->str_parametric_drc_type_ff_params);
      for (i = 0; i < p_parametricdrc_params->num_nodes; i++) {
        err = impd_parametric_ffwd_type_drc_process(
            audio_in_out_buf, audio_real_buff, audio_imag_buff, i,
            pstr_parametric_ffwd_type_drc_params,
            &pstr_parametric_drc_instance_params->str_spline_nodes);
        if (err) return (err);
      }

    } else if (pstr_parametric_drc_instance_params->parametric_drc_type ==
               PARAM_DRC_TYPE_LIM) {
      return (UNEXPECTED_ERROR);

    } else {
      return (UNEXPECTED_ERROR);
    }
  }

  return 0;
}

VOID iir_second_order_filter(ia_2nd_order_filt_coeff_struct* coeff,
                             ia_2nd_order_filt_state_struct* state,
                             WORD32 frame_len, FLOAT32* input,
                             FLOAT32* output) {
  FLOAT32 z2 = state->z2;
  FLOAT32 z1 = state->z1;
  FLOAT32 z0;
  WORD32 i;

  for (i = 0; i < frame_len; i++) {
    z0 = input[i] - coeff->a1 * z1 - coeff->a2 * z2;
    output[i] = coeff->b0 * z0 + coeff->b1 * z1 + coeff->b2 * z2;
    z2 = z1;
    z1 = z0;
  }
  state->z1 = z1;
  state->z2 = z2;
}
WORD32 impd_parametric_ffwd_type_drc_process(
    FLOAT32* audio_in_out_buf[], FLOAT32* audio_real_buff[],
    FLOAT32* audio_imag_buff[], WORD32 nodeIdx,
    ia_parametric_drc_type_ff_params_struct*
        pstr_parametric_ffwd_type_drc_params,
    ia_spline_nodes_struct* str_spline_nodes) {
  WORD32 c, t, b, n, i, offset;
  FLOAT32 x, y, channelLevel, level, levelDb, loc_db_gain, levelDelta, alpha;

  WORD32 frame_size = pstr_parametric_ffwd_type_drc_params->frame_size;
  WORD32 sub_band_count = pstr_parametric_ffwd_type_drc_params->sub_band_count;
  FLOAT32* level_estim_ch_weight =
      pstr_parametric_ffwd_type_drc_params->level_estim_ch_weight;
  WORD32 level_estim_k_weighting_type =
      pstr_parametric_ffwd_type_drc_params->level_estim_k_weighting_type;

  ia_2nd_order_filt_coeff_struct preC =
      pstr_parametric_ffwd_type_drc_params->pre_filt_coeff;
  ia_2nd_order_filt_coeff_struct rlbC =
      pstr_parametric_ffwd_type_drc_params->rlb_filt_coeff;
  ia_2nd_order_filt_state_struct* preS =
      pstr_parametric_ffwd_type_drc_params->pre_filt_state;
  ia_2nd_order_filt_state_struct* rlbS =
      pstr_parametric_ffwd_type_drc_params->rlb_filt_state;

  ia_2nd_order_filt_coeff_struct rlbC_sb =
      pstr_parametric_ffwd_type_drc_params->filt_coeff_subband;
  ia_2nd_order_filt_state_struct* rlbS_sbReal =
      pstr_parametric_ffwd_type_drc_params->filt_state_subband_real;
  ia_2nd_order_filt_state_struct* rlbS_sbImag =
      pstr_parametric_ffwd_type_drc_params->filt_state_subband_imag;
  FLOAT32* weighting_filt =
      pstr_parametric_ffwd_type_drc_params->weighting_filt;
  WORD32 sub_band_compensation_type =
      pstr_parametric_ffwd_type_drc_params->sub_band_compensation_type;

  if (audio_in_out_buf != NULL) {
    level = 0;
    offset = nodeIdx * pstr_parametric_ffwd_type_drc_params->frame_size;
    for (c = 0; c < pstr_parametric_ffwd_type_drc_params->audio_num_chan; c++) {
      channelLevel = 0.f;

      if (!level_estim_ch_weight[c]) continue;

      if (level_estim_k_weighting_type == 0) {
        for (t = 0; t < frame_size; t++) {
          x = audio_in_out_buf[c][offset + t];

          channelLevel += x * x;
        }

      } else if (level_estim_k_weighting_type == 1) {
        for (t = 0; t < frame_size; t++) {
          x = audio_in_out_buf[c][offset + t];

          iir_second_order_filter(&rlbC, &rlbS[c], 1, &x, &x);

          channelLevel += x * x;
        }

      } else if (level_estim_k_weighting_type == 2) {
        for (t = 0; t < frame_size; t++) {
          x = audio_in_out_buf[c][offset + t];

          iir_second_order_filter(&preC, &preS[c], 1, &x, &x);

          iir_second_order_filter(&rlbC, &rlbS[c], 1, &x, &x);

          channelLevel += x * x;
        }

      } else {
        return (UNEXPECTED_ERROR);
      }

      level += level_estim_ch_weight[c] * channelLevel;
    }

  } else {
    level = 0;
    offset = nodeIdx * pstr_parametric_ffwd_type_drc_params->sub_band_count;
    for (c = 0; c < pstr_parametric_ffwd_type_drc_params->audio_num_chan; c++) {
      channelLevel = 0.f;

      if (!level_estim_ch_weight[c]) continue;

      if (level_estim_k_weighting_type == 0) {
        for (b = 0; b < sub_band_count; b++) {
          x = audio_real_buff[c][offset + b];
          y = audio_imag_buff[c][offset + b];

          channelLevel += x * x + y * y;
        }

      } else if (level_estim_k_weighting_type == 1 ||
                 level_estim_k_weighting_type == 2) {
        for (b = 0; b < sub_band_count; b++) {
          x = audio_real_buff[c][offset + b] * weighting_filt[b];
          y = audio_imag_buff[c][offset + b] * weighting_filt[b];

          if (b == 0 && sub_band_compensation_type == 1) {
            iir_second_order_filter(&rlbC_sb, &rlbS_sbReal[c], 1, &x, &x);

            iir_second_order_filter(&rlbC_sb, &rlbS_sbImag[c], 1, &y, &y);
          }

          channelLevel += x * x + y * y;
        }

      } else {
        return (UNEXPECTED_ERROR);
      }

      level += level_estim_ch_weight[c] * channelLevel;
    }

    level /= sub_band_count;
  }
  pstr_parametric_ffwd_type_drc_params
      ->level[pstr_parametric_ffwd_type_drc_params->level_estim_frame_index] =
      level;
  pstr_parametric_ffwd_type_drc_params->level_estim_frame_index++;

  level = 0.f;
  if (pstr_parametric_ffwd_type_drc_params->start_up_phase) {
    for (i = 0;
         i < pstr_parametric_ffwd_type_drc_params->level_estim_frame_index;
         i++) {
      level += pstr_parametric_ffwd_type_drc_params->level[i];
    }
    level /= pstr_parametric_ffwd_type_drc_params->level_estim_frame_index *
             pstr_parametric_ffwd_type_drc_params->frame_size;
  } else {
    for (i = 0;
         i < pstr_parametric_ffwd_type_drc_params->level_estim_frame_count;
         i++) {
      level += pstr_parametric_ffwd_type_drc_params->level[i];
    }
    level /= pstr_parametric_ffwd_type_drc_params->level_estim_integration_time;
  }
  if (pstr_parametric_ffwd_type_drc_params->level_estim_frame_index ==
      pstr_parametric_ffwd_type_drc_params->level_estim_frame_count) {
    pstr_parametric_ffwd_type_drc_params->level_estim_frame_index = 0;
    pstr_parametric_ffwd_type_drc_params->start_up_phase = 0;
  }

  if (level < 1e-10f) level = 1e-10f;
  if (level_estim_k_weighting_type == 2) {
    levelDb = -0.691f + 10 * (FLOAT32)log10(level) + 3;
  } else {
    levelDb = 10 * (FLOAT32)log10(level) + 3;
  }
  levelDb -= pstr_parametric_ffwd_type_drc_params->ref_level_parametric_drc;

  for (n = 0; n < pstr_parametric_ffwd_type_drc_params->node_count; n++) {
    if (levelDb <=
        (FLOAT32)pstr_parametric_ffwd_type_drc_params->node_level[n]) {
      break;
    }
  }
  if (n == 0) {
    loc_db_gain = (FLOAT32)pstr_parametric_ffwd_type_drc_params->node_gain[n];
  } else if (n == pstr_parametric_ffwd_type_drc_params->node_count) {
    loc_db_gain =
        (FLOAT32)pstr_parametric_ffwd_type_drc_params->node_gain[n - 1] -
        levelDb +
        (FLOAT32)pstr_parametric_ffwd_type_drc_params->node_level[n - 1];
  } else {
    loc_db_gain =
        (FLOAT32)pstr_parametric_ffwd_type_drc_params->node_gain[n] +
        (levelDb -
         (FLOAT32)pstr_parametric_ffwd_type_drc_params->node_level[n]) /
            (FLOAT32)(pstr_parametric_ffwd_type_drc_params->node_level[n - 1] -
                      pstr_parametric_ffwd_type_drc_params->node_level[n]) *
            (FLOAT32)(pstr_parametric_ffwd_type_drc_params->node_gain[n - 1] -
                      pstr_parametric_ffwd_type_drc_params->node_gain[n]);
  }

  levelDelta = levelDb - pstr_parametric_ffwd_type_drc_params->db_level_smooth;
  if (loc_db_gain < pstr_parametric_ffwd_type_drc_params->db_gain_smooth) {
    if (levelDelta >
        pstr_parametric_ffwd_type_drc_params->gain_smooth_attack_threshold) {
      alpha =
          pstr_parametric_ffwd_type_drc_params->gain_smooth_attack_alpha_fast;
    } else {
      alpha =
          pstr_parametric_ffwd_type_drc_params->gain_smooth_attack_alpha_slow;
    }
  } else {
    if (levelDelta <
        -pstr_parametric_ffwd_type_drc_params->gain_smooth_rel_threshold) {
      alpha = pstr_parametric_ffwd_type_drc_params->gain_smooth_rel_alpha_fast;
    } else {
      alpha = pstr_parametric_ffwd_type_drc_params->gain_smooth_rel_alpha_slow;
    }
  }
  if (loc_db_gain < pstr_parametric_ffwd_type_drc_params->db_gain_smooth ||
      pstr_parametric_ffwd_type_drc_params->hold_counter == 0) {
    pstr_parametric_ffwd_type_drc_params->db_level_smooth =
        (1 - alpha) * pstr_parametric_ffwd_type_drc_params->db_level_smooth +
        alpha * levelDb;
    pstr_parametric_ffwd_type_drc_params->db_gain_smooth =
        (1 - alpha) * pstr_parametric_ffwd_type_drc_params->db_gain_smooth +
        alpha * loc_db_gain;
  }
  if (pstr_parametric_ffwd_type_drc_params->hold_counter) {
    pstr_parametric_ffwd_type_drc_params->hold_counter -= 1;
  }
  if (loc_db_gain < pstr_parametric_ffwd_type_drc_params->db_gain_smooth) {
    pstr_parametric_ffwd_type_drc_params->hold_counter =
        pstr_parametric_ffwd_type_drc_params->gain_smooth_hold_off_count;
  }

  str_spline_nodes->str_node[nodeIdx].loc_db_gain =
      pstr_parametric_ffwd_type_drc_params->db_gain_smooth;
  str_spline_nodes->str_node[nodeIdx].slope = 0.f;
  str_spline_nodes->str_node[nodeIdx].time =
      pstr_parametric_ffwd_type_drc_params->frame_size + offset - 1;

  return 0;
}

VOID impd_parametric_lim_type_drc_process(
    FLOAT32* samples[], FLOAT32 loudness_normalization_gain_db,
    ia_parametric_drc_type_lim_params_struct*
        pstr_parametric_lim_type_drc_params,
    FLOAT32* lpcm_gains) {
  WORD32 i, j;
  FLOAT32 tmp, gain;
  //  FLOAT32 min_gain = 1;
  FLOAT32 maximum, sectionMaximum;
  FLOAT32 loudness_normalization_gain =
      (FLOAT32)pow(10.0f, 0.05f * loudness_normalization_gain_db);
  FLOAT32* level_estim_ch_weight =
      pstr_parametric_lim_type_drc_params->level_estim_ch_weight;
  WORD32 num_channels = pstr_parametric_lim_type_drc_params->channels;
  WORD32 attack_time_samples = pstr_parametric_lim_type_drc_params->attack;
  FLOAT32 attack_constant =
      pstr_parametric_lim_type_drc_params->attack_constant;
  FLOAT32 release_constant =
      pstr_parametric_lim_type_drc_params->release_constant;
  FLOAT32 limit_threshold = pstr_parametric_lim_type_drc_params->threshold;
  FLOAT32* max_buf = pstr_parametric_lim_type_drc_params->max_buf;
  FLOAT32 gain_modified = pstr_parametric_lim_type_drc_params->cor;
  FLOAT64 pre_smoothed_gain =
      pstr_parametric_lim_type_drc_params->smooth_state_0;

  for (i = 0; i < pstr_parametric_lim_type_drc_params->frame_size; i++) {
    tmp = 0.0f;
    for (j = 0; j < num_channels; j++) {
      if (!level_estim_ch_weight[j]) continue;
      tmp =
          max(tmp, (FLOAT32)fabs(loudness_normalization_gain *
                                 (level_estim_ch_weight[j]) * (samples[j][i])));
    }

    for (j = attack_time_samples; j > 0; j--) {
      max_buf[j] = max_buf[j - 1];
    }
    max_buf[0] = tmp;
    sectionMaximum = tmp;
    for (j = 1; j < (attack_time_samples + 1); j++) {
      if (max_buf[j] > sectionMaximum) sectionMaximum = max_buf[j];
    }
    maximum = sectionMaximum;

    if (maximum > limit_threshold) {
      gain = limit_threshold / maximum;
    } else {
      gain = 1;
    }

    if (gain < pre_smoothed_gain) {
      gain_modified =
          min(gain_modified,
              (gain - 0.1f * (FLOAT32)pre_smoothed_gain) * 1.11111111f);
    } else {
      gain_modified = gain;
    }

    if (gain_modified < pre_smoothed_gain) {
      pre_smoothed_gain =
          attack_constant * (pre_smoothed_gain - gain_modified) + gain_modified;
      pre_smoothed_gain = max(pre_smoothed_gain, gain);
    } else {
      pre_smoothed_gain =
          release_constant * (pre_smoothed_gain - gain_modified) +
          gain_modified;
    }

    gain = (FLOAT32)pre_smoothed_gain;

    lpcm_gains[i] = gain;
  }

  pstr_parametric_lim_type_drc_params->cor = gain_modified;
  pstr_parametric_lim_type_drc_params->smooth_state_0 = pre_smoothed_gain;
  return;
}
