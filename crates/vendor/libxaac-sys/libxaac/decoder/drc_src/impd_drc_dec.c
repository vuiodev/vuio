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
#include "impd_type_def.h"
#include "impd_drc_extr_delta_coded_info.h"
#include "impd_drc_common.h"
#include "impd_drc_struct.h"
#include "impd_parametric_drc_dec.h"
#include "impd_drc_gain_dec.h"
#include "impd_drc_filter_bank.h"
#include "impd_drc_multi_band.h"

WORD32 impd_init_drc_params(WORD32 frame_size, WORD32 sample_rate,
                            WORD32 gain_delay_samples, WORD32 delay_mode,
                            WORD32 sub_band_domain_mode,
                            ia_drc_params_struct* ia_drc_params_struct) {
  WORD32 k;
  if (frame_size < 1 || frame_size > AUDIO_CODEC_FRAME_SIZE_MAX) {
    return -1;
  }

  if (sample_rate < 1000) {
    return -1;
  }

  ia_drc_params_struct->drc_frame_size = frame_size;

  if (ia_drc_params_struct->drc_frame_size < 0.001f * sample_rate) {
    return -1;
  }

  ia_drc_params_struct->sample_rate = sample_rate;

  ia_drc_params_struct->delta_tmin_default = impd_get_delta_tmin(sample_rate);

  if (ia_drc_params_struct->delta_tmin_default >
      ia_drc_params_struct->drc_frame_size) {
    return -1;
  }

  if ((delay_mode != DELAY_MODE_REGULAR_DELAY) &&
      (delay_mode != DELAY_MODE_LOW_DELAY)) {
    return -1;
  }

  ia_drc_params_struct->delay_mode = delay_mode;

  ia_drc_params_struct->drc_set_counter = 0;
  ia_drc_params_struct->multiband_sel_drc_idx = -1;

  for (k = 0; k < SEL_DRC_COUNT; k++) {
    ia_drc_params_struct->sel_drc_array[k].drc_instructions_index = -1;
    ia_drc_params_struct->sel_drc_array[k].dwnmix_instructions_index = -1;
    ia_drc_params_struct->sel_drc_array[k].drc_coeff_idx = -1;
  }

  if ((gain_delay_samples > MAX_SIGNAL_DELAY) || (gain_delay_samples < 0)) {
    return -1;
  } else {
    ia_drc_params_struct->gain_delay_samples = gain_delay_samples;
  }

  switch (sub_band_domain_mode) {
    case SUBBAND_DOMAIN_MODE_OFF:
    case SUBBAND_DOMAIN_MODE_QMF64:
    case SUBBAND_DOMAIN_MODE_QMF71:
    case SUBBAND_DOMAIN_MODE_STFT256:
      ia_drc_params_struct->sub_band_domain_mode = sub_band_domain_mode;
      break;
    default:
      return -1;
      break;
  }

  ia_drc_params_struct->parametric_drc_delay = 0;
  ia_drc_params_struct->eq_delay = 0;

  return 0;
}

WORD32 impd_select_drc_coefficients(
    ia_drc_config* drc_config, ia_uni_drc_coeffs_struct** drc_coefficients_drc,
    WORD32* drc_coefficients_selected) {
  WORD32 i;
  WORD32 cof1 = -1;
  WORD32 cof0 = -1;
  for (i = 0; i < drc_config->drc_coefficients_drc_count; i++) {
    if (drc_config->str_p_loc_drc_coefficients_uni_drc[i].drc_location == 1) {
      if (drc_config->str_p_loc_drc_coefficients_uni_drc[i].version == 0) {
        cof0 = i;
        *drc_coefficients_selected = cof0;
      } else {
        cof1 = i;
        *drc_coefficients_selected = cof1;
      }
    }
  }

  if (cof1 >= 0) {
    *drc_coefficients_drc =
        &(drc_config->str_p_loc_drc_coefficients_uni_drc[cof1]);
  } else if (cof0 >= 0) {
    *drc_coefficients_drc =
        &(drc_config->str_p_loc_drc_coefficients_uni_drc[cof0]);
  } else {
    *drc_coefficients_drc = NULL;
  }

  return 0;
}

WORD32 impd_init_selected_drc_set(
    ia_drc_config* drc_config, ia_drc_params_struct* ia_drc_params_struct,
    ia_parametric_drc_params_struct* p_parametric_drc_params,
    WORD32 audio_num_chan, WORD32 drc_set_id_selected,
    WORD32 downmix_id_selected, ia_filter_banks_struct* ia_filter_banks_struct,
    ia_overlap_params_struct* pstr_overlap_params

    ,
    shape_filter_block* shape_filter_block) {
  WORD32 g, n, c, err = 0;
  WORD32 channel_count = 0;
  WORD32 i;

  ia_drc_instructions_struct* drc_instructions_uni_drc = NULL;
  ia_uni_drc_coeffs_struct* drc_coefficients_uni_drc = NULL;
  WORD32 selected_drc_is_multiband = 0;
  WORD32 drc_instructions_selected = -1;
  WORD32 downmix_instructions_selected = -1;
  WORD32 drc_coefficients_selected = -1;
  p_parametric_drc_params->parametric_drc_instance_count = 0;

  if (drc_config->drc_coefficients_drc_count &&
      drc_config->str_p_loc_drc_coefficients_uni_drc->drc_frame_size_present) {
    if (drc_config->str_p_loc_drc_coefficients_uni_drc->drc_frame_size !=
        ia_drc_params_struct->drc_frame_size) {
      return -1;
    }
  }

  for (n = 0; n < drc_config->drc_instructions_count_plus; n++) {
    if (drc_config->str_drc_instruction_str[n].drc_set_id ==
        drc_set_id_selected)
      break;
  }
  if (n == drc_config->drc_instructions_count_plus) {
    return -1;
  }
  drc_instructions_selected = n;
  drc_instructions_uni_drc =
      &(drc_config->str_drc_instruction_str[drc_instructions_selected]);

  if (downmix_id_selected == ID_FOR_BASE_LAYOUT) {
    channel_count = drc_config->channel_layout.base_channel_count;
  } else if (downmix_id_selected == ID_FOR_ANY_DOWNMIX) {
    channel_count = audio_num_chan;
  } else {
    for (n = 0; n < drc_config->dwnmix_instructions_count; n++) {
      if (drc_config->dwnmix_instructions[n].downmix_id == downmix_id_selected)
        break;
    }
    if (n == drc_config->dwnmix_instructions_count) {
      return (UNEXPECTED_ERROR);
    }
    channel_count = drc_config->dwnmix_instructions[n].target_channel_count;

    downmix_instructions_selected = n;
  }
  drc_instructions_uni_drc->audio_num_chan = channel_count;

  if (drc_instructions_uni_drc->drc_set_id <= 0) {
    drc_coefficients_selected = 0;
  } else {
    err = impd_select_drc_coefficients(drc_config, &drc_coefficients_uni_drc,
                                       &drc_coefficients_selected);
    if (err) return err;
  }

  ia_drc_params_struct->sel_drc_array[ia_drc_params_struct->drc_set_counter]
      .drc_instructions_index = drc_instructions_selected;
  ia_drc_params_struct->sel_drc_array[ia_drc_params_struct->drc_set_counter]
      .dwnmix_instructions_index = downmix_instructions_selected;
  ia_drc_params_struct->sel_drc_array[ia_drc_params_struct->drc_set_counter]
      .drc_coeff_idx = drc_coefficients_selected;

  if ((drc_instructions_uni_drc->downmix_id[0] == ID_FOR_ANY_DOWNMIX) ||
      (drc_instructions_uni_drc->dwnmix_id_count > 1)) {
    WORD32 idx = drc_instructions_uni_drc->gain_set_index[0];
    for (c = 0; c < drc_instructions_uni_drc->audio_num_chan; c++) {
      drc_instructions_uni_drc->channel_group_of_ch[c] = (idx >= 0) ? 0 : -1;
    }
  }

  for (g = 0; g < drc_instructions_uni_drc->num_drc_ch_groups; g++) {
    drc_instructions_uni_drc->num_chan_per_ch_group[g] = 0;
    for (c = 0; c < drc_instructions_uni_drc->audio_num_chan; c++) {
      if (drc_instructions_uni_drc->channel_group_of_ch[c] == g) {
        drc_instructions_uni_drc->num_chan_per_ch_group[g]++;
      }
    }
  }

  if (drc_instructions_uni_drc->drc_set_effect &
      (EFFECT_BIT_DUCK_OTHER | EFFECT_BIT_DUCK_SELF)) {
    drc_instructions_uni_drc->multiband_audio_sig_count =
        drc_instructions_uni_drc->audio_num_chan;
  } else {
    drc_instructions_uni_drc->multiband_audio_sig_count = 0;
    for (c = 0; c < drc_instructions_uni_drc->audio_num_chan; c++) {
      g = drc_instructions_uni_drc->channel_group_of_ch[c];
      if (g < 0) {
        drc_instructions_uni_drc->multiband_audio_sig_count++;
      } else {
        drc_instructions_uni_drc->multiband_audio_sig_count +=
            drc_instructions_uni_drc->band_count_of_ch_group[g];
      }
    }
  }

  for (g = 0; g < drc_instructions_uni_drc->num_drc_ch_groups; g++) {
    if (g == 0) {
      drc_instructions_uni_drc->parametric_drc_look_ahead_samples_max = 0;
    }
    if (drc_instructions_uni_drc->ch_group_parametric_drc_flag[g] == 0) {
      if (drc_instructions_uni_drc->band_count_of_ch_group[g] > 1) {
        if (ia_drc_params_struct->multiband_sel_drc_idx != -1) {
          return (UNEXPECTED_ERROR);
        }
        selected_drc_is_multiband = 1;
      }
    } else {
      WORD32 gain_set_index =
          drc_instructions_uni_drc->gain_set_idx_of_ch_group_parametric_drc[g];
      WORD32 parametric_drc_id =
          drc_config->str_drc_config_ext.str_drc_coeff_param_drc
              .str_parametric_drc_gain_set_params[gain_set_index]
              .parametric_drc_id;
      WORD32 parametric_drc_look_ahead_samples = 0;
      ia_parametric_drc_instructions_struct* str_parametric_drc_instructions;

      for (i = 0;
           i < drc_config->str_drc_config_ext.parametric_drc_instructions_count;
           i++) {
        if (parametric_drc_id ==
            drc_config->str_drc_config_ext.str_parametric_drc_instructions[i]
                .parametric_drc_id)
          break;
      }
      if (i ==
          drc_config->str_drc_config_ext.parametric_drc_instructions_count) {
        return (UNEXPECTED_ERROR);
      }
      str_parametric_drc_instructions =
          &drc_config->str_drc_config_ext.str_parametric_drc_instructions[i];

      p_parametric_drc_params->parametric_drc_idx
          [p_parametric_drc_params->parametric_drc_instance_count] = i;
      p_parametric_drc_params->gain_set_index
          [p_parametric_drc_params->parametric_drc_instance_count] =
          gain_set_index;
      if (drc_instructions_uni_drc->drc_apply_to_dwnmix == 0) {
        p_parametric_drc_params->dwnmix_id_from_drc_instructions
            [p_parametric_drc_params->parametric_drc_instance_count] =
            ID_FOR_BASE_LAYOUT;
      } else {
        if (drc_instructions_uni_drc->dwnmix_id_count > 1) {
          p_parametric_drc_params->dwnmix_id_from_drc_instructions
              [p_parametric_drc_params->parametric_drc_instance_count] =
              ID_FOR_ANY_DOWNMIX;
        } else {
          p_parametric_drc_params->dwnmix_id_from_drc_instructions
              [p_parametric_drc_params->parametric_drc_instance_count] =
              drc_instructions_uni_drc->downmix_id[0];
        }
      }
      p_parametric_drc_params->audio_num_chan = channel_count;
      for (i = 0; i < p_parametric_drc_params->audio_num_chan; i++) {
        if (drc_instructions_uni_drc->channel_group_of_ch[i] == g) {
          p_parametric_drc_params->channel_map
              [p_parametric_drc_params->parametric_drc_instance_count][i] = 1;
        } else {
          p_parametric_drc_params->channel_map
              [p_parametric_drc_params->parametric_drc_instance_count][i] = 0;
        }
      }
      drc_instructions_uni_drc->parametric_drc_look_ahead_samples[g] = 0;
      if (str_parametric_drc_instructions->parametric_drc_look_ahead_flag) {
        parametric_drc_look_ahead_samples = (WORD32)(
            (FLOAT32)
                str_parametric_drc_instructions->parametric_drc_look_ahead *
            (FLOAT32)p_parametric_drc_params->sampling_rate * 0.001f);
      } else {
        if (str_parametric_drc_instructions->parametric_drc_type ==
            PARAM_DRC_TYPE_FF) {
          parametric_drc_look_ahead_samples = (WORD32)(
              10.0f * (FLOAT32)p_parametric_drc_params->sampling_rate * 0.001f);
        } else if (str_parametric_drc_instructions->parametric_drc_type ==
                   PARAM_DRC_TYPE_LIM) {
          parametric_drc_look_ahead_samples = (WORD32)(
              5.0f * (FLOAT32)p_parametric_drc_params->sampling_rate * 0.001f);
        } else {
          return (UNEXPECTED_ERROR);
        }
      }
      drc_instructions_uni_drc->parametric_drc_look_ahead_samples[g] =
          parametric_drc_look_ahead_samples;
      if (drc_instructions_uni_drc->parametric_drc_look_ahead_samples_max <
          drc_instructions_uni_drc->parametric_drc_look_ahead_samples[g]) {
        drc_instructions_uni_drc->parametric_drc_look_ahead_samples_max =
            drc_instructions_uni_drc->parametric_drc_look_ahead_samples[g];
      }
      p_parametric_drc_params->parametric_drc_instance_count += 1;
      selected_drc_is_multiband = 0;
    }
  }
  ia_drc_params_struct->parametric_drc_delay +=
      drc_instructions_uni_drc->parametric_drc_look_ahead_samples_max;

  if (selected_drc_is_multiband == 1) {
    ia_drc_params_struct->multiband_sel_drc_idx =
        ia_drc_params_struct->drc_set_counter;
    err = impd_init_all_filter_banks(
        drc_coefficients_uni_drc,
        &(drc_config->str_drc_instruction_str[drc_instructions_selected]),
        ia_filter_banks_struct);
    if (err) return (err);

    impd_init_overlap_weight(
        drc_coefficients_uni_drc,
        &(drc_config->str_drc_instruction_str[drc_instructions_selected]),
        ia_drc_params_struct->sub_band_domain_mode, pstr_overlap_params);

  } else {
    ia_gain_modifiers_struct* gain_modifiers =
        drc_config->str_drc_instruction_str->str_gain_modifiers_of_ch_group;
    for (g = 0; g < drc_instructions_uni_drc->num_drc_ch_groups; g++) {
      if (gain_modifiers[g].shape_filter_flag == 1) {
        impd_shape_filt_block_init(
            &drc_coefficients_uni_drc->str_shape_filter_block_params
                 [gain_modifiers[g].shape_filter_idx],
            &shape_filter_block[g]);
      } else {
        shape_filter_block[g].shape_flter_block_flag = 0;
      }
    }
  }

  ia_drc_params_struct->drc_set_counter++;

  return (0);
}
