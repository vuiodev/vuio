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
#include "ixheaac_constants.h"
#include "ixheaac_error_standards.h"

#include "ixheaacd_bitbuffer.h"
#include "ixheaacd_config.h"

#include <assert.h>

#ifndef sign
#define sign(a) (((a) > 0) ? 1 : ((a) < 0) ? -1 : 0)
#endif

typedef struct {
  WORD32 num_input_chan;
  WORD32 num_output_chan;
  WORD32 num_ott_boxes;
  WORD32 num_ttt_boxes;
  WORD32 ott_mode_lfe[MAX_NUM_OTT];
} ia_ld_mps_dec_tree_properties_struct;

static WORD32 ixheaacd_freq_res_table[] = {0, 23, 15, 12, 9, 7, 5, 4};

static WORD32 ixheaacd_hrtf_freq_res_table[][8] = {{0, 28, 20, 14, 10, 7, 5, 4},
                                          {0, 13, 13, 8, 7, 4, 3, 3}};

static ia_ld_mps_dec_tree_properties_struct ixheaacd_tree_property_table[] = {
    {1, 6, 5, 0, {0, 0, 0, 0, 1}}, {1, 6, 5, 0, {0, 0, 1, 0, 0}}, {2, 6, 3, 1, {1, 0, 0, 0, 0}},
    {2, 8, 5, 1, {1, 0, 0, 0, 0}}, {2, 8, 5, 1, {1, 0, 0, 0, 0}}, {6, 8, 2, 0, {0, 0, 0, 0, 0}},
    {6, 8, 2, 0, {0, 0, 0, 0, 0}}, {1, 2, 1, 0, {0, 0, 0, 0, 0}}};

static IA_ERRORCODE ixheaacd_ld_spatial_extension_config(
    ia_bit_buf_struct *it_bit_buff, ia_usac_dec_mps_config_struct *config,
    WORD32 bits_available) {
  WORD32 j, ch, idx, tmp, tmp_open, sac_ext_len, bits_read, n_fill_bits;
  UWORD32 i;
  WORD32 ba = bits_available;

  config->sac_ext_cnt = 0;

  tmp = it_bit_buff->cnt_bits;

  while (ba >= 8) {
    if (config->sac_ext_cnt >= MAX_NUM_EXT_TYPES) return IA_FATAL_ERROR;

    config->bs_sac_ext_type[config->sac_ext_cnt] =
        ixheaacd_read_bits_buf(it_bit_buff, 4);
    ba -= 4;

    sac_ext_len = ixheaacd_read_bits_buf(it_bit_buff, 4);
    ba -= 4;

    if ((ba >= 6) && (sac_ext_len > 0)) {
      if (sac_ext_len == 15) {
        sac_ext_len += ixheaacd_read_bits_buf(it_bit_buff, 8);
        ba -= 8;
        if (sac_ext_len == 15 + 255) {
          sac_ext_len += ixheaacd_read_bits_buf(it_bit_buff, 16);
          ba -= 16;
        }
      }

      switch (config->bs_sac_ext_type[config->sac_ext_cnt]) {
        case 0:
          config->bs_residual_coding = 1;

          config->bs_residual_sampling_freq_index =
              ixheaacd_read_bits_buf(it_bit_buff, 4);
          if (config->bs_residual_sampling_freq_index > MAX_RES_SAMP_FREQ_IDX) {
            return IA_FATAL_ERROR;
          }
          config->bs_residual_frames_per_spatial_frame =
              ixheaacd_read_bits_buf(it_bit_buff, 2);

          if ((config->num_ott_boxes + config->num_ttt_boxes) >
              MAX_RESIDUAL_CHANNELS)
            return IA_FATAL_ERROR;
          for (j = 0; j < config->num_ott_boxes + config->num_ttt_boxes; j++) {
            config->bs_residual_present[j] =
                ixheaacd_read_bits_buf(it_bit_buff, 1);
            if (config->bs_residual_present[j]) {
              config->bs_residual_bands_ld_mps[j] =
                  ixheaacd_read_bits_buf(it_bit_buff, 5);
              if (config->bs_residual_bands_ld_mps[j] > MAX_PARAMETER_BANDS)
              {
                return IA_FATAL_ERROR;
              }
            }
          }
          break;

        case 1:
          config->bs_arbitrary_downmix = 2;

          config->bs_arbitrary_downmix_residual_sampling_freq_index =
              ixheaacd_read_bits_buf(it_bit_buff, 4);
          if (config->bs_arbitrary_downmix_residual_sampling_freq_index > MAX_RES_SAMP_FREQ_IDX) {
            return IA_FATAL_ERROR;
          }
          config->bs_arbitrary_downmix_residual_frames_per_spatial_frame =
              ixheaacd_read_bits_buf(it_bit_buff, 2);
          config->bs_arbitrary_downmix_residual_bands =
              ixheaacd_read_bits_buf(it_bit_buff, 5);
          if (config->bs_arbitrary_downmix_residual_bands >=
              ixheaacd_freq_res_table[config->bs_freq_res]) {
            return IA_FATAL_ERROR;
          }

          break;

        case 2:
          config->num_out_chan_AT = 0;
          config->num_ott_boxes_AT = 0;
          if (config->num_output_channels > MAX_OUTPUT_CHANNELS)
            return IA_FATAL_ERROR;
          for (ch = 0; ch < config->num_output_channels; ch++) {
            tmp_open = 1;
            idx = 0;
            while ((tmp_open > 0) && (idx < MAX_ARBITRARY_TREE_INDEX)) {
              config->bs_ott_box_present_AT[ch][idx] =
                  ixheaacd_read_bits_buf(it_bit_buff, 1);
              if (config->bs_ott_box_present_AT[ch][idx]) {
                config->num_ott_boxes_AT++;
                tmp_open++;
              } else {
                config->num_out_chan_AT++;
                tmp_open--;
              }
              idx++;
            }
          }

          for (i = 0; i < config->num_ott_boxes_AT; i++) {
            config->bs_ott_default_cld_AT[i] =
                ixheaacd_read_bits_buf(it_bit_buff, 1);
            config->bs_ott_mode_lfe_AT[i] =
                ixheaacd_read_bits_buf(it_bit_buff, 1);
            if (config->bs_ott_mode_lfe_AT[i]) {
              config->bs_ott_bands_AT[i] =
                  ixheaacd_read_bits_buf(it_bit_buff, 5);
            } else {
              config->bs_ott_bands_AT[i] = ixheaacd_freq_res_table[config->bs_freq_res];
            }
          }

          for (i = 0; i < config->num_out_chan_AT; i++) {
            config->bs_output_channel_pos_AT[i] =
                ixheaacd_read_bits_buf(it_bit_buff, 5);
          }

          break;

        default:;
      }
    }

    bits_read = tmp - it_bit_buff->cnt_bits;
    n_fill_bits = 8 * sac_ext_len - bits_read;

    while (n_fill_bits > 7) {
      ixheaacd_read_bits_buf(it_bit_buff, 8);
      n_fill_bits -= 8;
    }
    if (n_fill_bits > 0) {
      ixheaacd_read_bits_buf(it_bit_buff, n_fill_bits);
    }

    ba -= 8 * sac_ext_len;
    config->sac_ext_cnt++;
  }
  return IA_NO_ERROR;
}

IA_ERRORCODE ixheaacd_ld_spatial_specific_config(
    ia_usac_dec_mps_config_struct *config, ia_bit_buf_struct *it_bit_buff) {
  WORD32 i, num_header_bits;
  UWORD32 hc, hb;
  WORD32 sac_header_len;
  WORD32 bits_available;
  WORD32 tmp = it_bit_buff->cnt_bits;
  WORD32 err = 0;

  sac_header_len = tmp;

  bits_available = sac_header_len;
  config->bs_sampling_freq_index = ixheaacd_read_bits_buf(it_bit_buff, 4);
  if (config->bs_sampling_freq_index == 15) {
    config->bs_fampling_frequency = ixheaacd_read_bits_buf(it_bit_buff, 24);
  }

  config->bs_frame_length = ixheaacd_read_bits_buf(it_bit_buff, 5);
  config->bs_freq_res = ixheaacd_read_bits_buf(it_bit_buff, 3);
  config->bs_tree_config = ixheaacd_read_bits_buf(it_bit_buff, 4);

  if (config->bs_tree_config > 7) return IA_FATAL_ERROR;

  if (config->bs_tree_config != 15) {
    config->num_ott_boxes =
        ixheaacd_tree_property_table[config->bs_tree_config].num_ott_boxes;
    config->num_ttt_boxes =
        ixheaacd_tree_property_table[config->bs_tree_config].num_ttt_boxes;
    config->num_input_channels =
        ixheaacd_tree_property_table[config->bs_tree_config].num_input_chan;
    config->num_output_channels =
        ixheaacd_tree_property_table[config->bs_tree_config].num_output_chan;
    for (i = 0; i < MAX_NUM_OTT; i++) {
      config->ott_mode_lfe[i] =
          ixheaacd_tree_property_table[config->bs_tree_config].ott_mode_lfe[i];
    }
  }
  config->bs_quant_mode = ixheaacd_read_bits_buf(it_bit_buff, 2);
  if (config->bs_tree_config != 7) {
    config->bs_one_icc = ixheaacd_read_bits_buf(it_bit_buff, 1);
  }
  config->bs_arbitrary_downmix = ixheaacd_read_bits_buf(it_bit_buff, 1);
  if (config->bs_tree_config != 7) {
    config->bs_fixed_gain_sur = ixheaacd_read_bits_buf(it_bit_buff, 3);
    config->bs_fixed_gain_LFE = ixheaacd_read_bits_buf(it_bit_buff, 3);
  }
  config->bs_fixed_gain_dmx = ixheaacd_read_bits_buf(it_bit_buff, 3);
  if (config->bs_tree_config != 7) {
    config->bs_matrix_mode = ixheaacd_read_bits_buf(it_bit_buff, 1);
  }
  config->bs_temp_shape_config = ixheaacd_read_bits_buf(it_bit_buff, 2);
  config->bs_decorr_config = ixheaacd_read_bits_buf(it_bit_buff, 2);
  if (config->bs_tree_config != 7) {
    config->bs_3D_audio_mode = ixheaacd_read_bits_buf(it_bit_buff, 1);
  } else {
    config->bs_3D_audio_mode = 0;
  }

  // ott_config
  for (i = 0; i < config->num_ott_boxes; i++) {
    if (config->ott_mode_lfe[i]) {
      config->bs_ott_bands[i] = ixheaacd_read_bits_buf(it_bit_buff, 5);
    } else {
      config->bs_ott_bands[i] = ixheaacd_freq_res_table[config->bs_freq_res];
    }
  }

  // ttt_config
  for (i = 0; i < config->num_ttt_boxes; i++) {
    config->bs_ttt_dual_mode[i] = ixheaacd_read_bits_buf(it_bit_buff, 1);
    config->bs_ttt_mode_low[i] = ixheaacd_read_bits_buf(it_bit_buff, 3);
    if (config->bs_ttt_dual_mode[i]) {
      config->bs_ttt_mode_high[i] = ixheaacd_read_bits_buf(it_bit_buff, 3);
      config->bs_ttt_bands_low[i] = ixheaacd_read_bits_buf(it_bit_buff, 5);
      config->bs_ttt_bands_high[i] = ixheaacd_freq_res_table[config->bs_freq_res];
    } else {
      config->bs_ttt_bands_low[i] = ixheaacd_freq_res_table[config->bs_freq_res];
    }
  }

  if (config->bs_temp_shape_config == 2) {
    config->bs_env_quant_mode = ixheaacd_read_bits_buf(it_bit_buff, 1);
  }

  if (config->bs_3D_audio_mode) {
    config->bs_3D_audio_HRTF_set = ixheaacd_read_bits_buf(it_bit_buff, 2);
    // param_HRTF_set
    if (config->bs_3D_audio_HRTF_set == 0) {
      config->bs_HRTF_freq_res = ixheaacd_read_bits_buf(it_bit_buff, 3);
      config->bs_HRTF_num_chan = 5;
      config->bs_HRTF_asymmetric = ixheaacd_read_bits_buf(it_bit_buff, 1);

      config->HRTF_num_band = ixheaacd_hrtf_freq_res_table[0][config->bs_HRTF_freq_res];
      config->HRTF_num_phase = ixheaacd_hrtf_freq_res_table[1][config->bs_HRTF_freq_res];

      for (hc = 0; hc < config->bs_HRTF_num_chan; hc++) {
        for (hb = 0; hb < config->HRTF_num_band; hb++) {
          config->bs_HRTF_level_left[hc][hb] =
              ixheaacd_read_bits_buf(it_bit_buff, 6);
        }
        for (hb = 0; hb < config->HRTF_num_band; hb++) {
          config->bs_HRTF_level_right[hc][hb] =
              config->bs_HRTF_asymmetric
                  ? ixheaacd_read_bits_buf(it_bit_buff, 6)
                  : config->bs_HRTF_level_left[hc][hb];
        }
        config->bs_HRTF_phase[hc] = ixheaacd_read_bits_buf(it_bit_buff, 1);
        for (hb = 0; hb < config->HRTF_num_phase; hb++) {
          config->bs_HRTF_phase_LR[hc][hb] =
              config->bs_HRTF_phase[hc] ? ixheaacd_read_bits_buf(it_bit_buff, 6)
                                        : 0;
        }
        config->bs_HRTF_icc[hc] = ixheaacd_read_bits_buf(it_bit_buff, 1);
        if (config->bs_HRTF_icc[hc]) {
          for (hb = 0; hb < config->HRTF_num_band; hb++)
            config->bs_HRTF_icc_LR[hc][hb] =
                ixheaacd_read_bits_buf(it_bit_buff, 3);
        }
      }
    }
  }

  // byte_align
  i = (it_bit_buff->cnt_bits & 0x7);
  ixheaacd_read_bits_buf(it_bit_buff, i);

  num_header_bits = tmp - (it_bit_buff->cnt_bits);
  bits_available -= num_header_bits;

  err =
      ixheaacd_ld_spatial_extension_config(it_bit_buff, config, bits_available);
  return err;
}
