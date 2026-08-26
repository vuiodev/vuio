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
#include "ixheaacd_error_codes.h"
#include "ixheaacd_bitbuffer.h"
#include "ixheaacd_mps_res_rom.h"
#include "ixheaacd_mps_aac_struct.h"
#include "ixheaacd_mps_res_channel.h"
#include "ixheaac_constants.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaacd_cnst.h"
#include "ixheaacd_common_rom.h"
#include "ixheaacd_sbrdecsettings.h"
#include "ixheaacd_sbr_scale.h"
#include "ixheaacd_env_extr_part.h"
#include "ixheaacd_sbr_rom.h"
#include "ixheaacd_hybrid.h"
#include "ixheaacd_ps_dec.h"
#include "ixheaacd_mps_polyphase.h"
#include "ixheaac_error_standards.h"
#include "ixheaacd_config.h"
#include "ixheaacd_qmf_dec.h"
#include "ixheaacd_mps_dec.h"
#include "ixheaacd_mps_interface.h"
#include "ixheaacd_mps_macro_def.h"
#include "ixheaacd_mps_nlc_dec.h"
#include "ixheaacd_mps_bitdec.h"
#include "ixheaacd_mps_res_tns.h"
#include "ixheaacd_mps_mdct_2_qmf.h"
#include "ixheaac_sbr_const.h"

static const WORD32 ixheaacd_freq_res_table[] = {0, 28, 20, 14, 10, 7, 5, 4};

static WORD32 ixheaacd_bound_check(WORD32 var, WORD32 lower_bound, WORD32 upper_bound) {
  var = min(var, upper_bound);
  var = max(var, lower_bound);
  return var;
}

static VOID ixheaacd_mps_check_index_bounds(
    WORD32 output_idx_data[][MAX_PARAMETER_SETS][MAX_PARAMETER_BANDS],
    WORD32 num_parameter_sets, WORD32 start_band, WORD32 stop_band,
    WORD32 param_type, WORD32 xtt_idx) {
  WORD32 i, band;
  for (i = 0; i < num_parameter_sets; i++) {
    for (band = start_band; band < stop_band; band++) {
      if (param_type == CLD) {
        output_idx_data[xtt_idx][i][band] =
          ixheaacd_bound_check(output_idx_data[xtt_idx][i][band], -15, 15);
      } else if (param_type == ICC) {
        output_idx_data[xtt_idx][i][band] =
        ixheaacd_bound_check(output_idx_data[xtt_idx][i][band], 0, 7);
      }
    }
  }
}

static IA_ERRORCODE ixheaacd_parse_extension_config(
    ia_mps_spatial_bs_config_struct *config, WORD32 num_ott_boxes, WORD32 num_ttt_boxes,
    WORD32 num_out_chan, WORD32 bits_available, ia_bit_buf_struct *it_bit_buff,
    ia_mps_dec_bitdec_tables_struct *ixheaacd_mps_dec_bitdec_tables) {
  WORD32 i, ch, idx, tmp, tmp_open, sac_ext_len, bits_read, n_fill_bits, temp;
  WORD32 ba = bits_available;

  config->sac_ext_cnt = 0;

  while (ba >= 8) {
    ba -= 8;
    temp = ixheaacd_read_bits_buf(it_bit_buff, 8);
    config->bs_sac_ext_type[config->sac_ext_cnt] = (temp >> 4) & FOUR_BIT_MASK;
    sac_ext_len = temp & FOUR_BIT_MASK;
    if (sac_ext_len == 15) {
      sac_ext_len += ixheaacd_read_bits_buf(it_bit_buff, 8);
      ba -= 8;
      if (sac_ext_len == 15 + 255) {
        sac_ext_len += ixheaacd_read_bits_buf(it_bit_buff, 16);
        ba -= 16;
      }
    }

    tmp = (WORD32)(((it_bit_buff->ptr_read_next - it_bit_buff->ptr_bit_buf_base + 1) << 3) -
                   (it_bit_buff->bit_pos + 1));

    switch (config->bs_sac_ext_type[config->sac_ext_cnt]) {
      case EXT_TYPE_0:
        config->bs_residual_coding = 1;
        temp = ixheaacd_read_bits_buf(it_bit_buff, 6);
        config->bs_residual_sampling_freq_index = (temp >> 2) & FOUR_BIT_MASK;
        if (config->bs_residual_sampling_freq_index > MAX_RES_SAMP_FREQ_IDX) {
          return IA_FATAL_ERROR;
        }
        config->bs_residual_frames_per_spatial_frame = temp & TWO_BIT_MASK;

        for (i = 0; i < num_ott_boxes + num_ttt_boxes; i++) {
          config->bs_residual_present[i] = ixheaacd_read_bits_buf(it_bit_buff, 1);
          if (config->bs_residual_present[i]) {
            config->bs_residual_bands[i] = ixheaacd_read_bits_buf(it_bit_buff, 5);
            if (config->bs_residual_bands[i] > MAX_PARAMETER_BANDS)
            {
              return IA_FATAL_ERROR;
            }
          }
        }
        break;

      case EXT_TYPE_1:
        config->bs_arbitrary_downmix = 2;

        temp = ixheaacd_read_bits_buf(it_bit_buff, 11);
        config->bs_arbitrary_downmix_residual_sampling_freq_index = (temp >> 7) & FOUR_BIT_MASK;
        if (config->bs_arbitrary_downmix_residual_sampling_freq_index > MAX_RES_SAMP_FREQ_IDX) {
          return IA_FATAL_ERROR;
        }
        config->bs_arbitrary_downmix_residual_frames_per_spatial_frame =
            (temp >> 5) & TWO_BIT_MASK;
        config->bs_arbitrary_downmix_residual_bands = temp & FIVE_BIT_MASK;
        if (config->bs_arbitrary_downmix_residual_bands >=
            ixheaacd_freq_res_table[config->bs_freq_res]) {
          return IA_FATAL_ERROR;
        }

        break;

      case EXT_TYPE_2:
        config->arbitrary_tree = 1;
        config->num_out_chan_at = 0;
        config->num_ott_boxes_at = 0;
        for (ch = 0; ch < num_out_chan; ch++) {
          tmp_open = 1;
          idx = 0;
          while (tmp_open > 0) {
            config->bs_ott_box_present_at[ch][idx] = ixheaacd_read_bits_buf(it_bit_buff, 1);
            if (config->bs_ott_box_present_at[ch][idx]) {
              config->num_ott_boxes_at++;
              tmp_open++;
            } else {
              config->num_out_chan_at++;
              tmp_open--;
            }
            if (config->num_ott_boxes_at >= 56) return IA_FATAL_ERROR;
            if (config->num_out_chan_at > MAX_OUTPUT_CHANNELS_AT_MPS) return IA_FATAL_ERROR;
            idx++;
            if (idx >= MAX_ARBITRARY_TREE_INDEX) return IA_FATAL_ERROR;
          }
        }

        for (i = 0; i < config->num_ott_boxes_at; i++) {
          temp = ixheaacd_read_bits_buf(it_bit_buff, 2);
          config->bs_ott_default_cld_at[i] = (temp >> 1) & ONE_BIT_MASK;
          config->bs_ott_mode_lfe_at[i] = temp & ONE_BIT_MASK;
          if (config->bs_ott_mode_lfe_at[i]) {
            config->bs_ott_bands_at[i] = ixheaacd_read_bits_buf(it_bit_buff, 5);
            if (config->bs_ott_bands_at[i] > MAX_PARAMETER_BANDS) return IA_FATAL_ERROR;
          } else {
            config->bs_ott_bands_at[i] =
                ixheaacd_mps_dec_bitdec_tables->freq_res_table[config->bs_freq_res];
          }
        }

        for (i = 0; i < config->num_out_chan_at; i++) {
          config->bs_output_channel_pos_at[i] = ixheaacd_read_bits_buf(it_bit_buff, 5);
        }

        break;

      default:
        return IA_XHEAAC_MPS_DEC_EXE_NONFATAL_INVALID_EXTENSION_TYPE;
    }

    bits_read = (WORD32)(((it_bit_buff->ptr_read_next - it_bit_buff->ptr_bit_buf_base + 1) << 3) -
                         (it_bit_buff->bit_pos + 1) - tmp);
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
    if (config->sac_ext_cnt >= MAX_NUM_EXT_TYPES) {
      return IA_FATAL_ERROR;
    }
  }

  return IA_NO_ERROR;
}

IA_ERRORCODE ixheaacd_parse_specific_config(ia_heaac_mps_state_struct *pstr_mps_state,
                                            WORD32 sac_header_len) {
  IA_ERRORCODE err_code = IA_NO_ERROR;
  ia_mps_spatial_bs_config_struct *config = &(pstr_mps_state->bs_config);
  ia_heaac_mps_state_struct *curr_state = pstr_mps_state;
  const ia_mps_dec_tree_properties_struct *p_tree_property_table =
      pstr_mps_state->ia_mps_dec_mps_table.bitdec_table_ptr->tree_property_table;
  ia_bit_buf_struct *mps_bit_buf = pstr_mps_state->ptr_mps_bit_buff;

  WORD32 i, hc, hb, num_header_bits, ott_mode_lfe[MAX_NUM_OTT];

  WORD32 tmp = (WORD32)(((mps_bit_buf->ptr_read_next - mps_bit_buf->ptr_bit_buf_base + 1) << 3) -
                        (mps_bit_buf->bit_pos + 1));
  WORD32 bits_available = (sac_header_len << 3);
  WORD32 temp, alignment_bits = 0;

  config->bs_sampling_freq_index = ixheaacd_read_bits_buf(mps_bit_buf, 4);
  if (config->bs_sampling_freq_index == 15) {
    config->bs_sampling_frequency = ixheaacd_read_bits_buf(mps_bit_buf, 24);
  }
  temp = ixheaacd_read_bits_buf(mps_bit_buf, 14);
  config->bs_frame_length = (temp >> 7) & SEVEN_BIT_MASK;
  if (config->bs_frame_length >= (MAX_QMF_BUF_LEN - 1)) {
    return IA_FATAL_ERROR;
  }
  config->bs_freq_res = (temp >> 4) & THREE_BIT_MASK;
  if (config->bs_freq_res == 0) {
    return IA_XHEAAC_MPS_DEC_EXE_FATAL_INVALID_PARAMETER_BANDS;
  }
  config->bs_tree_config = (temp)&FOUR_BIT_MASK;

  if (config->bs_tree_config >= 7) {
    return IA_FATAL_ERROR;
  }

  if (config->bs_tree_config != 15) {
    curr_state->num_ott_boxes = p_tree_property_table[config->bs_tree_config].num_ott_boxes;
    curr_state->num_ttt_boxes = p_tree_property_table[config->bs_tree_config].num_ttt_boxes;
    curr_state->num_input_channels =
        p_tree_property_table[config->bs_tree_config].num_input_channels;
    curr_state->num_output_channels =
        p_tree_property_table[config->bs_tree_config].num_output_channels;
    for (i = 0; i < MAX_NUM_OTT; i++) {
      ott_mode_lfe[i] = p_tree_property_table[config->bs_tree_config].ott_mode_lfe[i];
    }
  }
  temp = ixheaacd_read_bits_buf(mps_bit_buf, 19);
  config->bs_quant_mode = (temp >> 17) & TWO_BIT_MASK;
  config->bs_one_icc = (temp >> 16) & ONE_BIT_MASK;
  config->bs_arbitrary_downmix = (temp >> 15) & ONE_BIT_MASK;
  config->bs_fixed_gain_sur = (temp >> 12) & THREE_BIT_MASK;
  if (config->bs_fixed_gain_sur >= 5) {
    return IA_FATAL_ERROR;
  }
  config->bs_fixed_gain_lfe = (temp >> 9) & THREE_BIT_MASK;

  if (config->bs_fixed_gain_lfe >= 5) return IA_FATAL_ERROR;
  config->bs_fixed_gain_dmx = (temp >> 6) & THREE_BIT_MASK;
  config->bs_matrix_mode = (temp >> 5) & ONE_BIT_MASK;
  config->bs_temp_shape_config = (temp >> 3) & TWO_BIT_MASK;
  if (config->bs_temp_shape_config == 3)
    return IA_FATAL_ERROR;

  config->bs_decorr_config = (temp >> 1) & TWO_BIT_MASK;
  config->bs_3d_audio_mode = (temp)&ONE_BIT_MASK;

  for (i = 0; i < curr_state->num_ott_boxes; i++) {
    if (ott_mode_lfe[i]) {
      config->bs_ott_bands[i] = ixheaacd_read_bits_buf(mps_bit_buf, 5);
      if (config->bs_ott_bands[i] > MAX_PARAMETER_BANDS) return IA_FATAL_ERROR;
    }
  }

  for (i = 0; i < curr_state->num_ttt_boxes; i++) {
    temp = ixheaacd_read_bits_buf(mps_bit_buf, 4);
    config->bs_ttt_dual_mode[i] = (temp >> 3) & ONE_BIT_MASK;
    config->bs_ttt_mode_low[i] = (temp)&THREE_BIT_MASK;
    if (config->bs_ttt_dual_mode[i]) {
      temp = ixheaacd_read_bits_buf(mps_bit_buf, 8);
      config->bs_ttt_mode_high[i] = (temp >> 5) & THREE_BIT_MASK;
      config->bs_ttt_bands_low[i] = (temp)&FIVE_BIT_MASK;
      if (config->bs_ttt_bands_low[i] > MAX_PARAMETER_BANDS) return IA_FATAL_ERROR;
    }
  }

  if (config->bs_temp_shape_config == 2) {
    config->bs_env_quant_mode = ixheaacd_read_bits_buf(mps_bit_buf, 1);
  }

  if (config->bs_3d_audio_mode) {
    config->bs_3d_audio_hrtf_set = ixheaacd_read_bits_buf(mps_bit_buf, 2);
    if (config->bs_3d_audio_hrtf_set == 0) {
      temp = ixheaacd_read_bits_buf(mps_bit_buf, 4);
      config->bs_hrtf_freq_res = (temp >> 1) & THREE_BIT_MASK;
      config->bs_hrtf_num_chan = 5;
      config->bs_hrtf_asymmetric = (temp)&ONE_BIT_MASK;

      config->hrtf_num_band = pstr_mps_state->ia_mps_dec_mps_table.bitdec_table_ptr
                                  ->freq_res_table[config->bs_hrtf_freq_res];

      for (hc = 0; hc < config->bs_hrtf_num_chan; hc++) {
        for (hb = 0; hb < config->hrtf_num_band; hb++) {
          config->bs_hrtf_level_left[hc][hb] = ixheaacd_read_bits_buf(mps_bit_buf, 6);
        }
        for (hb = 0; hb < config->hrtf_num_band; hb++) {
          config->bs_hrtf_level_right[hc][hb] = config->bs_hrtf_asymmetric
                                                    ? ixheaacd_read_bits_buf(mps_bit_buf, 6)
                                                    : config->bs_hrtf_level_left[hc][hb];
        }
        config->bs_hrtf_phase[hc] = ixheaacd_read_bits_buf(mps_bit_buf, 1);
        for (hb = 0; hb < config->hrtf_num_band; hb++) {
          config->bs_hrtf_phase_lr[hc][hb] =
              config->bs_hrtf_phase[hc] ? ixheaacd_read_bits_buf(mps_bit_buf, 6) : 0;
        }
      }
    }
  }

  ixheaacd_byte_align(mps_bit_buf, &alignment_bits);

  num_header_bits =
      (WORD32)(((mps_bit_buf->ptr_read_next - mps_bit_buf->ptr_bit_buf_base + 1) << 3) -
               (mps_bit_buf->bit_pos + 1) - tmp);
  bits_available -= num_header_bits;

  err_code = ixheaacd_parse_extension_config(
      config, curr_state->num_ott_boxes, curr_state->num_ttt_boxes,
      curr_state->num_output_channels, bits_available, mps_bit_buf,
      pstr_mps_state->ia_mps_dec_mps_table.bitdec_table_ptr);
  if (err_code != IA_NO_ERROR) return err_code;

  return IA_NO_ERROR;
}

IA_ERRORCODE ixheaacd_default_specific_config(ia_heaac_mps_state_struct *pstr_mps_state,
                                              WORD32 sampling_freq) {
  ia_mps_spatial_bs_config_struct *config = &(pstr_mps_state->bs_config);
  ia_heaac_mps_state_struct *curr_state = pstr_mps_state;
  const ia_mps_dec_tree_properties_struct *p_tree_property_table =
      pstr_mps_state->ia_mps_dec_mps_table.bitdec_table_ptr->tree_property_table;
  WORD32 i, ott_mode_lfe[MAX_NUM_OTT];

  config->bs_sampling_freq_index = 15;
  for (i = 0; i < 15; i++) {
    if (sampling_freq ==
        pstr_mps_state->ia_mps_dec_mps_table.bitdec_table_ptr->sampling_freq_table[i]) {
      config->bs_sampling_freq_index = i;
    }
  }
  if (config->bs_sampling_freq_index == 15) {
    config->bs_sampling_frequency = sampling_freq;
  }
  config->bs_frame_length = 31;
  config->bs_freq_res = 1;
  config->bs_tree_config = 2;
  if (config->bs_tree_config > 5) return IA_XHEAAC_MPS_DEC_EXE_FATAL_UNSUPPRORTED_TREE_CONFIG;
  if (config->bs_tree_config != 15) {
    curr_state->num_ott_boxes = p_tree_property_table[config->bs_tree_config].num_ott_boxes;
    curr_state->num_ttt_boxes = p_tree_property_table[config->bs_tree_config].num_ttt_boxes;
    curr_state->num_input_channels =
        p_tree_property_table[config->bs_tree_config].num_input_channels;
    curr_state->num_output_channels =
        p_tree_property_table[config->bs_tree_config].num_output_channels;
    memcpy(ott_mode_lfe, p_tree_property_table[config->bs_tree_config].ott_mode_lfe,
           MAX_NUM_OTT * sizeof(ott_mode_lfe[0]));
  }
  config->bs_quant_mode = 0;
  config->bs_one_icc = 0;
  config->bs_arbitrary_downmix = 0;
  config->bs_residual_coding = 0;
  config->bs_smooth_config = 0;
  config->bs_fixed_gain_sur = 2;
  config->bs_fixed_gain_lfe = 1;
  config->bs_fixed_gain_dmx = 0;
  config->bs_matrix_mode = 1;
  config->bs_temp_shape_config = 0;
  config->bs_decorr_config = 0;
  if (config->bs_tree_config == 15) {
    return IA_XHEAAC_MPS_DEC_EXE_FATAL_UNSUPPRORTED_TREE_CONFIG;
  }
  for (i = 0; i < curr_state->num_ott_boxes; i++) {
    if (ott_mode_lfe[i]) {
      config->bs_ott_bands[i] = 28;
    }
  }
  for (i = 0; i < curr_state->num_ttt_boxes; i++) {
    config->bs_ttt_dual_mode[i] = 0;
    config->bs_ttt_mode_low[i] = 1;
    if (config->bs_ttt_dual_mode[i]) {
      config->bs_ttt_mode_high[i] = 1;
      config->bs_ttt_bands_low[i] = 28;
    }
  }
  return IA_NO_ERROR;
}

static VOID ixheaacd_coarse_2_fine(WORD32 *data, WORD32 data_type, WORD32 start_band,
                                   WORD32 num_bands) {
  WORD32 i;

  for (i = start_band; i < start_band + num_bands; i++) {
    data[i] <<= 1;
  }

  if (data_type == CLD) {
    for (i = start_band; i < start_band + num_bands; i++) {
      if (data[i] == -14)
        data[i] = -15;
      else if (data[i] == 14)
        data[i] = 15;
    }
  }
}

static VOID ixheaacd_fine_2_coarse(WORD32 *data, WORD32 start_band, WORD32 num_bands) {
  WORD32 i;

  for (i = start_band; i < start_band + num_bands; i++) {
    data[i] >>= 1;
  }
}

static WORD32 ixheaacd_get_stride_map(
    WORD32 freq_res_stride, WORD32 start_band, WORD32 stop_band, WORD32 *a_strides,
    ia_mps_dec_bitdec_tables_struct *ixheaacd_mps_dec_bitdec_tables) {
  WORD32 i, pb, pb_stride, data_bands, str_offset;

  pb_stride = ixheaacd_mps_dec_bitdec_tables->pb_stride_table[freq_res_stride];
  data_bands = (stop_band - start_band - 1) / pb_stride + 1;

  a_strides[0] = start_band;
  for (pb = 1; pb <= data_bands; pb++) {
    a_strides[pb] = a_strides[pb - 1] + pb_stride;
  }
  str_offset = 0;
  while (a_strides[data_bands] > stop_band) {
    if (str_offset < data_bands) str_offset++;
    for (i = str_offset; i <= data_bands; i++) {
      a_strides[i]--;
    }
  }

  return data_bands;
}

static IA_ERRORCODE ixheaacd_ec_data_dec(ia_heaac_mps_state_struct *pstr_mps_state,
                                         ia_mps_dec_lossless_data_struct *ll_data,
                                         WORD32 data[][MAX_PARAMETER_SETS][MAX_PARAMETER_BANDS],
                                         WORD32 lastdata[][MAX_PARAMETER_BANDS], WORD32 datatype,
                                         WORD32 box_idx, WORD32 param_idx, WORD32 start_band,
                                         WORD32 stop_band) {
  IA_ERRORCODE error_code = IA_NO_ERROR;
  WORD32 i, pb, data_sets, set_idx, bs_data_pair, data_bands, old_quant_coarse_xxx, temp;
  WORD32 a_strides[MAX_PARAMETER_BANDS + 1] = {0};

  ia_mps_dec_spatial_bs_frame_struct *frame = pstr_mps_state->bs_frame;
  ia_bit_buf_struct *mps_bit_buf = pstr_mps_state->ptr_mps_bit_buff;

  data_sets = 0;
  for (i = 0; i < pstr_mps_state->num_parameter_sets; i++) {
    ll_data->bs_xxx_data_mode[param_idx][i] = ixheaacd_read_bits_buf(mps_bit_buf, 2);
    if (ll_data->bs_xxx_data_mode[param_idx][i] == 3) {
      data_sets++;
    }
  }
  set_idx = 0;
  old_quant_coarse_xxx = ll_data->bs_quant_coarse_xxx_prev[param_idx];

  while (set_idx < data_sets) {
    temp = ixheaacd_read_bits_buf(mps_bit_buf, 4);
    bs_data_pair = (temp >> 3) & ONE_BIT_MASK;
    ll_data->bs_quant_coarse_xxx[param_idx][set_idx] = (temp >> 2) & ONE_BIT_MASK;
    ll_data->bs_freq_res_stride_xxx[param_idx][set_idx] = temp & TWO_BIT_MASK;

    if (set_idx == 7 && bs_data_pair == 1) {
      if (pstr_mps_state->ec_flag) {
        bs_data_pair = 0;
      } else {
        return IA_FATAL_ERROR;
      }
    }

    if (ll_data->bs_quant_coarse_xxx[param_idx][set_idx] != old_quant_coarse_xxx) {
      if (old_quant_coarse_xxx) {
        ixheaacd_coarse_2_fine(lastdata[box_idx], datatype, start_band, stop_band - start_band);
      } else {
        ixheaacd_fine_2_coarse(lastdata[box_idx], start_band, stop_band - start_band);
      }
    }

    data_bands = ixheaacd_get_stride_map(ll_data->bs_freq_res_stride_xxx[param_idx][set_idx],
                                         start_band, stop_band, a_strides,
                                         pstr_mps_state->ia_mps_dec_mps_table.bitdec_table_ptr);

    for (pb = 0; pb < data_bands; pb++) {
      lastdata[box_idx][start_band + pb] = lastdata[box_idx][a_strides[pb]];
    }

    error_code = ixheaacd_mps_ecdatapairdec(
        mps_bit_buf, data[box_idx], lastdata[box_idx], datatype, set_idx, start_band, data_bands,
        bs_data_pair, ll_data->bs_quant_coarse_xxx[param_idx][set_idx],
        (!frame->bs_independency_flag || (set_idx > 0)), 0, 1, pstr_mps_state->ec_flag);
    if (error_code != IA_NO_ERROR) return error_code;

    if (datatype == CLD) {
      WORD32 band;
      for (i = 0; i < pstr_mps_state->num_parameter_sets; i++) {
        for (band = start_band; band < stop_band; band++) {
          if (data[box_idx][i][band] > 15 || data[box_idx][i][band] < -15) {
            return IA_FATAL_ERROR;
          }
        }
      }
    } else if (datatype == ICC) {
      WORD32 band;
      for (i = 0; i < pstr_mps_state->num_parameter_sets; i++) {
        for (band = start_band; band < stop_band; band++) {
          if (data[box_idx][i][band] > 7 || data[box_idx][i][band] < 0) {
            return IA_FATAL_ERROR;
          }
        }
      }
    }

    for (pb = 0; pb < data_bands; pb++) {
      for (i = a_strides[pb]; i < a_strides[pb + 1]; i++) {
        lastdata[box_idx][i] = data[box_idx][set_idx + bs_data_pair][start_band + pb];
      }
    }

    old_quant_coarse_xxx = ll_data->bs_quant_coarse_xxx[param_idx][set_idx];

    if (bs_data_pair) {
      ll_data->bs_quant_coarse_xxx[param_idx][set_idx + 1] =
          ll_data->bs_quant_coarse_xxx[param_idx][set_idx];
      ll_data->bs_freq_res_stride_xxx[param_idx][set_idx + 1] =
          ll_data->bs_freq_res_stride_xxx[param_idx][set_idx];
    }
    set_idx += bs_data_pair + 1;
  }
  return error_code;
}

static IA_ERRORCODE ixheaacd_parse_arbitrary_downmix_data(
    ia_heaac_mps_state_struct *pstr_mps_state) {
  ia_mps_dec_spatial_bs_frame_struct *frame = pstr_mps_state->bs_frame;
  IA_ERRORCODE error_code = IA_NO_ERROR;
  WORD32 offset = pstr_mps_state->num_ott_boxes + 4 * pstr_mps_state->num_ttt_boxes;
  WORD32 num_input_channels = pstr_mps_state->num_input_channels;
  WORD32 bitstream_parameter_bands = pstr_mps_state->bitstream_parameter_bands;
  WORD32 ch;

  for (ch = 0; ch < num_input_channels; ch++) {
    error_code = ixheaacd_ec_data_dec(pstr_mps_state, &frame->cld_lossless_data,
                                      frame->cmp_arbdmx_gain_idx, frame->cmp_arbdmx_gain_idx_prev,
                                      CLD, ch, offset + ch, 0, bitstream_parameter_bands);
    if (error_code != IA_NO_ERROR) return error_code;
  }
  return error_code;
}

static WORD32 ixheaacd_decode_icc_diff_code(ia_bit_buf_struct *it_bit_buff) {
  WORD32 value = 0;
  WORD32 count = 0;
  while ((ixheaacd_read_bits_buf(it_bit_buff, 1) == 0) && (count++ < 7)) {
    value++;
  }

  return value;
}

static IA_ERRORCODE ixheaacd_parse_residual_data(ia_heaac_mps_state_struct *pstr_mps_state) {
  WORD32 ich, ch;
  WORD32 rfpsf;
  WORD32 ps;
  WORD32 pb;

  ia_mps_dec_residual_aac_tables_struct *aac_tables_ptr =
      pstr_mps_state->ia_mps_dec_mps_table.aac_tab;
  WORD32 i;

  ia_mps_dec_spatial_bs_frame_struct *frame = pstr_mps_state->bs_frame;
  ia_mps_spatial_bs_config_struct *config = &(pstr_mps_state->bs_config);

  WORD32 num_ott_boxes = pstr_mps_state->num_ott_boxes;
  WORD32 num_parameter_sets = pstr_mps_state->num_parameter_sets;
  WORD32 residual_frames_per_spatial_frame = pstr_mps_state->residual_frames_per_spatial_frame;
  WORD32 upd_qmf = pstr_mps_state->upd_qmf;

  WORD32 loop_counter = num_ott_boxes + pstr_mps_state->num_ttt_boxes;
  WORD32 *p_mdct_res;

  WORD32 *p_res_mdct = pstr_mps_state->array_struct->res_mdct;
  ia_bit_buf_struct *mps_bit_buf = pstr_mps_state->ptr_mps_bit_buff;
  WORD16 error_code = IA_NO_ERROR;

  for (ich = 0; ich < loop_counter; ich++) {
    ch = ich;

    p_mdct_res = p_res_mdct;
    if (config->bs_residual_bands[ch] > 0) {
      if (ch < num_ott_boxes) {
        for (ps = 0; ps < num_parameter_sets; ps++) {
          frame->res_data.bs_icc_diff_present[ch][ps] = ixheaacd_read_bits_buf(mps_bit_buf, 1);
          if (frame->res_data.bs_icc_diff_present[ch][ps]) {
            for (pb = 0; pb < config->bs_residual_bands[ch]; pb++) {
              frame->res_data.bs_icc_diff[ch][ps][pb] =
                  ixheaacd_decode_icc_diff_code(mps_bit_buf);
              frame->ott_icc_diff_idx[ch][ps][pb] = frame->res_data.bs_icc_diff[ch][ps][pb];
            }
          }
        }
      }
      p_mdct_res = p_res_mdct;
      for (rfpsf = 0; rfpsf < residual_frames_per_spatial_frame; rfpsf++) {
        error_code =
            ixheaacd_res_read_ics(mps_bit_buf, pstr_mps_state->p_aac_decoder_channel_info, 1,
                                  aac_tables_ptr, pstr_mps_state->tot_sf_bands_ls);
        if (error_code) {
          if (pstr_mps_state->ec_flag) {
            pstr_mps_state->frame_ok = 0;
          } else
            return error_code;
        }
        if (1 == pstr_mps_state->p_aac_decoder_channel_info[0]->tns_data.tns_data_present)
          ixheaacd_res_ctns_apply(
              pstr_mps_state->p_aac_decoder_channel_info[0],
              pstr_mps_state->p_aac_decoder_channel_info[0]->ics_info.max_sf_bands,
              aac_tables_ptr);
        pstr_mps_state->res_block_type[ch][rfpsf] =
            pstr_mps_state->p_aac_decoder_channel_info[0]->ics_info.window_sequence;
        for (i = 0; i < AAC_FRAME_LENGTH; i++) {
          *p_mdct_res++ =
              (pstr_mps_state->p_aac_decoder_channel_info[0]->p_spectral_coefficient[i]);
        }

        if ((pstr_mps_state->p_aac_decoder_channel_info[0]->ics_info.window_sequence ==
             EIGHT_SHORT_SEQUENCE) &&
            ((upd_qmf == UPD_QMF_18) || (upd_qmf == UPD_QMF_24) || (upd_qmf == UPD_QMF_30))) {
          error_code =
              ixheaacd_res_read_ics(mps_bit_buf, pstr_mps_state->p_aac_decoder_channel_info, 1,
                                    aac_tables_ptr, pstr_mps_state->tot_sf_bands_ls);
          if (error_code) {
            if (pstr_mps_state->ec_flag) {
              pstr_mps_state->frame_ok = 0;
            } else
              return error_code;
          }
          if (1 == pstr_mps_state->p_aac_decoder_channel_info[0]->tns_data.tns_data_present)
            ixheaacd_res_ctns_apply(
                pstr_mps_state->p_aac_decoder_channel_info[0],
                pstr_mps_state->p_aac_decoder_channel_info[0]->ics_info.max_sf_bands,
                aac_tables_ptr);
          for (i = 0; i < AAC_FRAME_LENGTH; i++) {
            *p_mdct_res++ =
                (pstr_mps_state->p_aac_decoder_channel_info[0]->p_spectral_coefficient[i]);
          }
        }
      }
    }

    p_res_mdct += RFX2XMDCTCOEF;
  }
  return IA_NO_ERROR;
}

static IA_ERRORCODE ixheaacd_parse_extension_frame(ia_heaac_mps_state_struct *pstr_mps_state) {
  WORD32 i, fr, gr, offset, ch;
  WORD32 ext_num, sac_ext_type, sac_ext_len, tmp, bits_read, n_fill_bits, temp;
  WORD32 channel_grouping[MAX_INPUT_CHANNELS_MPS];

  ia_mps_dec_spatial_bs_frame_struct *frame = pstr_mps_state->bs_frame;
  ia_mps_spatial_bs_config_struct *p_bs_config = &pstr_mps_state->bs_config;

  ia_mps_dec_residual_aac_tables_struct *aac_tables_ptr =
      pstr_mps_state->ia_mps_dec_mps_table.aac_tab;

  WORD32 arbdmx_upd_qmf = pstr_mps_state->arbdmx_upd_qmf;
  WORD32 num_ott_boxes = pstr_mps_state->num_ott_boxes;
  WORD32 num_input_channels = pstr_mps_state->num_input_channels;
  WORD32 num_ttt_boxes = pstr_mps_state->num_ttt_boxes;
  WORD32 arbdmx_frames_per_spatial_frame = pstr_mps_state->arbdmx_frames_per_spatial_frame;
  WORD32 *p_res_mdct, *p_mdct_res;

  WORD32 sfidx;
  VOID *free_scratch = pstr_mps_state->mps_scratch_mem_v;
  ia_mps_dec_residual_sfband_info_struct *p_sfband_info_tab = &pstr_mps_state->sfband_info_tab;
  ia_bit_buf_struct *mps_bit_buf = pstr_mps_state->ptr_mps_bit_buff;
  IA_ERRORCODE error_code = IA_NO_ERROR;

  for (ch = 0; ch < 2; ch++) {
    pstr_mps_state->p_aac_decoder_channel_info[ch] = free_scratch;
    free_scratch =
        (WORD8 *)free_scratch +
        IXHEAAC_GET_SIZE_ALIGNED(sizeof(ia_mps_dec_residual_channel_info_struct), BYTE_ALIGN_8);
    pstr_mps_state->p_aac_decoder_dynamic_data_init[ch] = free_scratch;
    free_scratch =
        (WORD8 *)free_scratch +
        IXHEAAC_GET_SIZE_ALIGNED(sizeof(ia_mps_dec_residual_dynamic_data_struct), BYTE_ALIGN_8);
    pstr_mps_state->p_aac_decoder_channel_info[ch]->p_scale_factor =
        pstr_mps_state->p_aac_decoder_dynamic_data_init[ch]->a_scale_factor;
    pstr_mps_state->p_aac_decoder_channel_info[ch]->p_code_book =
        pstr_mps_state->p_aac_decoder_dynamic_data_init[ch]->a_code_book;
    pstr_mps_state->p_aac_decoder_channel_info[ch]->p_spectral_coefficient = free_scratch;
    free_scratch = (WORD8 *)free_scratch + IXHEAAC_GET_SIZE_ALIGNED(4096, BYTE_ALIGN_8);
    pstr_mps_state->p_aac_decoder_channel_info[ch]->p_tns_scratch = free_scratch;
    free_scratch = (WORD8 *)free_scratch + IXHEAAC_GET_SIZE_ALIGNED(4096, BYTE_ALIGN_8);
    pstr_mps_state->p_aac_decoder_channel_info[ch]->ics_info.frame_length = AAC_FRAME_LENGTH;
    pstr_mps_state->p_aac_decoder_channel_info[ch]->common_window = 0;
  }
  if (pstr_mps_state->arbitrary_downmix == 2)
    sfidx = p_bs_config->bs_arbitrary_downmix_residual_sampling_freq_index;
  else
    sfidx = p_bs_config->bs_residual_sampling_freq_index;
  {
    WORD16 *psfb_idx[2];
    const WORD8 *psfb_width[2];
    WORD width_idx;
    WORD32 j;

    pstr_mps_state->p_aac_decoder_channel_info[0]->ics_info.sampling_rate_index = sfidx;
    psfb_idx[0] = p_sfband_info_tab->sfb_long_idx;
    psfb_idx[1] = p_sfband_info_tab->sfb_short_idx;
    psfb_width[0] = aac_tables_ptr->scale_factor_bands_long[sfidx];
    psfb_width[1] = aac_tables_ptr->scale_factor_bands_short[sfidx];

    for (j = 1; j >= 0; j--) {
      const WORD8 *ptr_w = psfb_width[j];
      WORD16 *ptr_i = psfb_idx[j];
      width_idx = 0;
      *ptr_i++ = width_idx;
      do {
        width_idx += (*ptr_w++);
        *ptr_i++ = width_idx;
      } while (*ptr_w != -1);

      pstr_mps_state->tot_sf_bands_ls[j] = (WORD8)(ptr_w - psfb_width[j]);
    }

    {
      aac_tables_ptr->sfb_index_long = p_sfband_info_tab->sfb_long_idx;
      aac_tables_ptr->sfb_index_short = p_sfband_info_tab->sfb_short_idx;
      aac_tables_ptr->sfb_index_long_width = (WORD8 *)psfb_width[0];
      aac_tables_ptr->sfb_index_short_width = (WORD8 *)psfb_width[1];
    }
  }

  for (ext_num = 0; ext_num < p_bs_config->sac_ext_cnt; ext_num++) {
    sac_ext_type = p_bs_config->bs_sac_ext_type[ext_num];

    if (sac_ext_type < 12) {
      sac_ext_len = ixheaacd_read_bits_buf(mps_bit_buf, 8);
      if (sac_ext_len == 255) {
        sac_ext_len += ixheaacd_read_bits_buf(mps_bit_buf, 16);
      }

      tmp = (WORD32)(((mps_bit_buf->ptr_read_next - mps_bit_buf->ptr_bit_buf_base + 1) << 3) -
                     (mps_bit_buf->bit_pos + 1));

      switch (sac_ext_type) {
        case EXT_TYPE_0:
          error_code = ixheaacd_parse_residual_data(pstr_mps_state);
          if (error_code) {
            if (pstr_mps_state->ec_flag) {
              pstr_mps_state->frame_ok = 0;
            } else
              return error_code;
          }
          break;

        case EXT_TYPE_1:
          switch (num_input_channels) {
            case IN_CH_1:
              channel_grouping[0] = 1;
              break;
            case IN_CH_2:
              channel_grouping[0] = 2;
              break;
            case IN_CH_6:
              channel_grouping[0] = 2;
              channel_grouping[1] = 2;
              channel_grouping[2] = 2;
              break;
            default:
              return IA_XHEAAC_MPS_DEC_EXE_FATAL_INVALID_INPUT_CHANNEL;
              break;
          }

          offset = num_ott_boxes + num_ttt_boxes;

          p_res_mdct = pstr_mps_state->array_struct->res_mdct + offset * RFX2XMDCTCOEF;

          for (ch = 0, gr = 0; ch < num_input_channels; ch += channel_grouping[gr++]) {
            p_mdct_res = p_res_mdct;

            temp = ixheaacd_read_bits_buf(mps_bit_buf, 2);
            frame->bs_arbitrary_downmix_residual_abs[ch] = (temp >> 1) & ONE_BIT_MASK;
            frame->bs_arbitrary_downmix_residual_alpha_update_set[ch] = temp & ONE_BIT_MASK;

            if (channel_grouping[gr] == 1) {
              for (fr = 0; fr < arbdmx_frames_per_spatial_frame; fr++) {
                error_code =
                    ixheaacd_res_read_ics(mps_bit_buf, pstr_mps_state->p_aac_decoder_channel_info,
                                          1, aac_tables_ptr, pstr_mps_state->tot_sf_bands_ls);
                if (error_code) {
                  if (pstr_mps_state->ec_flag) {
                    pstr_mps_state->frame_ok = 0;
                  } else
                    return error_code;
                }
                if (1 == pstr_mps_state->p_aac_decoder_channel_info[0]->tns_data.tns_data_present)
                  ixheaacd_res_ctns_apply(
                      pstr_mps_state->p_aac_decoder_channel_info[0],
                      pstr_mps_state->p_aac_decoder_channel_info[0]->ics_info.max_sf_bands,
                      aac_tables_ptr);

                pstr_mps_state->res_block_type[offset + ch][fr] =
                    pstr_mps_state->p_aac_decoder_channel_info[0]->ics_info.window_sequence;
                for (i = 0; i < AAC_FRAME_LENGTH; i++) {
                  *p_mdct_res++ =
                      (pstr_mps_state->p_aac_decoder_channel_info[0]->p_spectral_coefficient[i]);
                }

                if ((pstr_mps_state->p_aac_decoder_channel_info[0]->ics_info.window_sequence ==
                     EIGHT_SHORT_SEQUENCE) &&
                    ((arbdmx_upd_qmf == UPD_QMF_18) || (arbdmx_upd_qmf == UPD_QMF_24) ||
                     (arbdmx_upd_qmf == UPD_QMF_30))) {
                  error_code = ixheaacd_res_read_ics(
                      mps_bit_buf, pstr_mps_state->p_aac_decoder_channel_info, 1, aac_tables_ptr,
                      pstr_mps_state->tot_sf_bands_ls);
                  if (error_code) {
                    if (pstr_mps_state->ec_flag) {
                      pstr_mps_state->frame_ok = 0;
                    } else
                      return error_code;
                  }
                  if (1 ==
                      pstr_mps_state->p_aac_decoder_channel_info[0]->tns_data.tns_data_present)
                    ixheaacd_res_ctns_apply(
                        pstr_mps_state->p_aac_decoder_channel_info[0],
                        pstr_mps_state->p_aac_decoder_channel_info[0]->ics_info.max_sf_bands,
                        aac_tables_ptr);
                  for (i = 0; i < AAC_FRAME_LENGTH; i++) {
                    *p_mdct_res++ = (pstr_mps_state->p_aac_decoder_channel_info[0]
                                         ->p_spectral_coefficient[i]);
                  }
                }
              }
              p_res_mdct += RFX2XMDCTCOEF;
            } else {
              frame->bs_arbitrary_downmix_residual_abs[ch + 1] =
                  frame->bs_arbitrary_downmix_residual_abs[ch];
              frame->bs_arbitrary_downmix_residual_alpha_update_set[ch + 1] =
                  frame->bs_arbitrary_downmix_residual_alpha_update_set[ch];

              for (fr = 0; fr < arbdmx_frames_per_spatial_frame; fr++) {
                WORD32 *res_mdct_1 = p_mdct_res + RFX2XMDCTCOEF;
                WORD32 temp, win1, win2;
                temp = ixheaacd_read_bits_buf(mps_bit_buf, 4);
                temp = ixheaacd_read_bits_buf(mps_bit_buf, 1);

                if (temp != 0) {
                  return IA_XHEAAC_MPS_DEC_EXE_FATAL_NONZERO_BIT;
                }

                error_code =
                    ixheaacd_res_read_ics(mps_bit_buf, pstr_mps_state->p_aac_decoder_channel_info,
                                          1, aac_tables_ptr, pstr_mps_state->tot_sf_bands_ls);
                if (error_code) {
                  if (pstr_mps_state->ec_flag) {
                    pstr_mps_state->frame_ok = 0;
                  } else
                    return error_code;
                }

                if (1 == pstr_mps_state->p_aac_decoder_channel_info[0]->tns_data.tns_data_present)
                  ixheaacd_res_ctns_apply(
                      pstr_mps_state->p_aac_decoder_channel_info[0],
                      pstr_mps_state->p_aac_decoder_channel_info[0]->ics_info.max_sf_bands,
                      aac_tables_ptr);
                win1 = pstr_mps_state->p_aac_decoder_channel_info[0]->ics_info.window_sequence;
                pstr_mps_state->res_block_type[offset + ch][fr] =
                    pstr_mps_state->p_aac_decoder_channel_info[0]->ics_info.window_sequence;

                for (i = 0; i < AAC_FRAME_LENGTH; i++) {
                  *p_mdct_res++ =
                      (pstr_mps_state->p_aac_decoder_channel_info[0]->p_spectral_coefficient[i]);
                }

                error_code =
                    ixheaacd_res_read_ics(mps_bit_buf, pstr_mps_state->p_aac_decoder_channel_info,
                                          1, aac_tables_ptr, pstr_mps_state->tot_sf_bands_ls);
                if (error_code) {
                  if (pstr_mps_state->ec_flag) {
                    pstr_mps_state->frame_ok = 0;
                  } else
                    return error_code;
                }

                if (1 == pstr_mps_state->p_aac_decoder_channel_info[0]->tns_data.tns_data_present)
                  ixheaacd_res_ctns_apply(
                      pstr_mps_state->p_aac_decoder_channel_info[0],
                      pstr_mps_state->p_aac_decoder_channel_info[0]->ics_info.max_sf_bands,
                      aac_tables_ptr);
                win2 = pstr_mps_state->p_aac_decoder_channel_info[0]->ics_info.window_sequence;
                for (i = 0; i < AAC_FRAME_LENGTH; i++) {
                  *res_mdct_1++ =
                      (pstr_mps_state->p_aac_decoder_channel_info[0]->p_spectral_coefficient[i]);
                }

                if (win1 != win2) return IA_XHEAAC_MPS_DEC_EXE_FATAL_INVALID_WINDOW_SEQUENCE;

                if ((win1 == EIGHT_SHORT_SEQUENCE) &&
                    ((arbdmx_upd_qmf == UPD_QMF_18) || (arbdmx_upd_qmf == UPD_QMF_24) ||
                     (arbdmx_upd_qmf == UPD_QMF_30))) {
                  temp = ixheaacd_read_bits_buf(mps_bit_buf, 4);
                  temp = ixheaacd_read_bits_buf(mps_bit_buf, 1);

                  if (temp != 0) {
                    return IA_XHEAAC_MPS_DEC_EXE_FATAL_NONZERO_BIT;
                  }

                  error_code = ixheaacd_res_read_ics(
                      mps_bit_buf, pstr_mps_state->p_aac_decoder_channel_info, 1, aac_tables_ptr,
                      pstr_mps_state->tot_sf_bands_ls);
                  if (error_code) {
                    if (pstr_mps_state->ec_flag) {
                      pstr_mps_state->frame_ok = 0;
                    } else
                      return error_code;
                  }

                  if (1 ==
                      pstr_mps_state->p_aac_decoder_channel_info[0]->tns_data.tns_data_present)
                    ixheaacd_res_ctns_apply(
                        pstr_mps_state->p_aac_decoder_channel_info[0],
                        pstr_mps_state->p_aac_decoder_channel_info[0]->ics_info.max_sf_bands,
                        aac_tables_ptr);
                  win1 = pstr_mps_state->p_aac_decoder_channel_info[0]->ics_info.window_sequence;

                  for (i = 0; i < AAC_FRAME_LENGTH; i++) {
                    *p_mdct_res++ = (pstr_mps_state->p_aac_decoder_channel_info[0]
                                         ->p_spectral_coefficient[i]);
                  }

                  error_code = ixheaacd_res_read_ics(
                      mps_bit_buf, pstr_mps_state->p_aac_decoder_channel_info, 1, aac_tables_ptr,
                      pstr_mps_state->tot_sf_bands_ls);
                  if (error_code) {
                    if (pstr_mps_state->ec_flag) {
                      pstr_mps_state->frame_ok = 0;
                    } else
                      return error_code;
                  }

                  if (1 ==
                      pstr_mps_state->p_aac_decoder_channel_info[0]->tns_data.tns_data_present)
                    ixheaacd_res_ctns_apply(
                        pstr_mps_state->p_aac_decoder_channel_info[0],
                        pstr_mps_state->p_aac_decoder_channel_info[0]->ics_info.max_sf_bands,
                        aac_tables_ptr);
                  win2 = pstr_mps_state->p_aac_decoder_channel_info[0]->ics_info.window_sequence;
                  for (i = 0; i < AAC_FRAME_LENGTH; i++) {
                    *res_mdct_1++ = (pstr_mps_state->p_aac_decoder_channel_info[0]
                                         ->p_spectral_coefficient[i]);
                  }

                  if (win1 != win2) return IA_XHEAAC_MPS_DEC_EXE_FATAL_INVALID_WINDOW_SEQUENCE;
                }
              }
              p_res_mdct += RFX2XMDCTCOEF;
            }
          }

          break;

        case EXT_TYPE_2:
          for (i = 0; i < p_bs_config->num_ott_boxes_at; i++) {
            error_code = ixheaacd_ec_data_dec(pstr_mps_state, &frame->cld_lossless_data,
                                              frame->cmp_ott_cld_idx, frame->cmp_ott_cld_idx_prev,
                                              CLD, num_ott_boxes + i, num_ott_boxes + i, 0,
                                              p_bs_config->bs_ott_bands_at[i]);
            if (error_code != IA_NO_ERROR) return error_code;
          }

          break;

        default:
          return IA_XHEAAC_MPS_DEC_EXE_NONFATAL_INVALID_EXTENSION_TYPE;
      }

      bits_read =
          (WORD32)(((mps_bit_buf->ptr_read_next - mps_bit_buf->ptr_bit_buf_base + 1) << 3) -
                   (mps_bit_buf->bit_pos + 1) - tmp);
      n_fill_bits = (sac_ext_len << 3) - bits_read;

      while (n_fill_bits > 7) {
        ixheaacd_read_bits_buf(mps_bit_buf, 8);
        n_fill_bits -= 8;
      }
      if (n_fill_bits > 0) {
        ixheaacd_read_bits_buf(mps_bit_buf, n_fill_bits);
      }
    }
  }
  return IA_NO_ERROR;
}

IA_ERRORCODE ixheaacd_parse_frame(ia_heaac_mps_state_struct *pstr_mps_state) {
  ia_mps_spatial_bs_config_struct *p_bs_config = &pstr_mps_state->bs_config;

  ia_mps_dec_auxilary_struct *p_aux_struct = pstr_mps_state->aux_struct;
  WORD32 i, bs_framing_type, prev_param_slot, data_bands, bs_temp_shape_enable,
      num_temp_shape_chan;
  WORD32 ttt_off, ps, pg, ts, pb, temp;
  WORD32 *bs_env_shape_data = pstr_mps_state->mps_scratch_mem_v;
  WORD32 const *reciprocal_tab = pstr_mps_state->ia_mps_dec_mps_table.m1_m2_table_ptr->reciprocal;
  WORD32 num_parameter_sets;

  ia_mps_dec_spatial_bs_frame_struct *frame = pstr_mps_state->bs_frame;
  ia_mps_dec_bitdec_tables_struct *bitdec_table =
      pstr_mps_state->ia_mps_dec_mps_table.bitdec_table_ptr;
  WORD32 bs_num_output_channels =
      bitdec_table->tree_property_table[pstr_mps_state->tree_config].num_output_channels;

  WORD32 time_slots = pstr_mps_state->time_slots;
  WORD32 bitstream_parameter_bands = pstr_mps_state->bitstream_parameter_bands;
  WORD32 *b_ott_bands = pstr_mps_state->bitstream_ott_bands;
  WORD32 *param_slot = pstr_mps_state->aux_struct->param_slot;

  WORD32 num_ott_boxes = pstr_mps_state->num_ott_boxes;

  WORD32 reciprocal, alignment_bits = 0;
  ia_bit_buf_struct *mps_bit_buf = pstr_mps_state->ptr_mps_bit_buff;
  IA_ERRORCODE error_code = IA_NO_ERROR;

  if (pstr_mps_state->parse_next_bitstream_frame == 0) return IA_NO_ERROR;

  temp = ixheaacd_read_bits_buf(mps_bit_buf, 4);
  bs_framing_type = (temp >> 3) & ONE_BIT_MASK;
  num_parameter_sets = (temp & THREE_BIT_MASK) + 1;
  pstr_mps_state->num_parameter_sets = num_parameter_sets;

  reciprocal = reciprocal_tab[num_parameter_sets - 1];

  prev_param_slot = -1;
  for (i = 0; i < num_parameter_sets; i++) {
    if (bs_framing_type) {
      WORD32 bits_param_slot = 0;
      while ((1 << bits_param_slot) < (time_slots - num_parameter_sets + i - prev_param_slot))
        bits_param_slot++;
      param_slot[i] =
          bits_param_slot
              ? prev_param_slot + 1 + ixheaacd_read_bits_buf(mps_bit_buf, bits_param_slot)
              : prev_param_slot + 1;
      prev_param_slot = param_slot[i];
    } else {
      WORD64 temp = (WORD64)(
          ((WORD64)((time_slots * (i + 1)) + num_parameter_sets - 1) * (WORD64)reciprocal) >> 28);
      param_slot[i] = (WORD32)(temp - 1);
    }
  }
  frame->bs_independency_flag = ixheaacd_read_bits_buf(mps_bit_buf, 1);

  for (i = 0; i < num_ott_boxes; i++) {
    error_code =
        ixheaacd_ec_data_dec(pstr_mps_state, &frame->cld_lossless_data, frame->cmp_ott_cld_idx,
                             frame->cmp_ott_cld_idx_prev, CLD, i, i, 0, b_ott_bands[i]);
    if (error_code != IA_NO_ERROR) return error_code;
  }
  if (pstr_mps_state->one_icc) {
    error_code = ixheaacd_ec_data_dec(pstr_mps_state, &frame->icc_lossless_data,
                                      frame->cmp_ott_icc_idx, frame->cmp_ott_icc_idx_prev, ICC, 0,
                                      0, 0, bitstream_parameter_bands);
    if (error_code != IA_NO_ERROR) return error_code;
  } else {
    for (i = 0; i < num_ott_boxes; i++) {
      if (!pstr_mps_state->ott_mode_lfe[i]) {
        error_code = ixheaacd_ec_data_dec(pstr_mps_state, &frame->icc_lossless_data,
                                          frame->cmp_ott_icc_idx, frame->cmp_ott_icc_idx_prev,
                                          ICC, i, i, 0, b_ott_bands[i]);
        if (error_code != IA_NO_ERROR) return error_code;
      }
    }
  }

  ttt_off = num_ott_boxes;
  for (i = 0; i < pstr_mps_state->num_ttt_boxes; i++) {
    if (p_aux_struct->ttt_config[0][i].mode < 2) {
      error_code = ixheaacd_ec_data_dec(pstr_mps_state, &frame->cpc_lossless_data,
                                        frame->cmp_ttt_cpc_1_idx, frame->cmp_ttt_cpc_1_idx_prev,
                                        CPC, i, ttt_off + 4 * i,
                                        p_aux_struct->ttt_config[0][i].bitstream_start_band,
                                        p_aux_struct->ttt_config[0][i].bitstream_stop_band);
      if (error_code != IA_NO_ERROR) return error_code;

      error_code = ixheaacd_ec_data_dec(pstr_mps_state, &frame->cpc_lossless_data,
                                        frame->cmp_ttt_cpc_2_idx, frame->cmp_ttt_cpc_2_idx_prev,
                                        CPC, i, ttt_off + 4 * i + 1,
                                        p_aux_struct->ttt_config[0][i].bitstream_start_band,
                                        p_aux_struct->ttt_config[0][i].bitstream_stop_band);
      if (error_code != IA_NO_ERROR) return error_code;

      error_code =
          ixheaacd_ec_data_dec(pstr_mps_state, &frame->icc_lossless_data, frame->cmp_ttt_icc_idx,
                               frame->cmp_ttt_icc_idx_prev, ICC, i, ttt_off + 4 * i,
                               p_aux_struct->ttt_config[0][i].bitstream_start_band,
                               p_aux_struct->ttt_config[0][i].bitstream_stop_band);
      if (error_code != IA_NO_ERROR) return error_code;
    } else {
      error_code = ixheaacd_ec_data_dec(pstr_mps_state, &frame->cld_lossless_data,
                                        frame->cmp_ttt_cld_1_idx, frame->cmp_ttt_cld_1_idx_prev,
                                        CLD, i, ttt_off + 4 * i,
                                        p_aux_struct->ttt_config[0][i].bitstream_start_band,
                                        p_aux_struct->ttt_config[0][i].bitstream_stop_band);
      if (error_code != IA_NO_ERROR) return error_code;

      error_code = ixheaacd_ec_data_dec(pstr_mps_state, &frame->cld_lossless_data,
                                        frame->cmp_ttt_cld_2_idx, frame->cmp_ttt_cld_2_idx_prev,
                                        CLD, i, ttt_off + 4 * i + 1,
                                        p_aux_struct->ttt_config[0][i].bitstream_start_band,
                                        p_aux_struct->ttt_config[0][i].bitstream_stop_band);
      if (error_code != IA_NO_ERROR) return error_code;
    }

    if (p_aux_struct->ttt_config[1][i].bitstream_start_band <
        p_aux_struct->ttt_config[1][i].bitstream_stop_band) {
      if (p_aux_struct->ttt_config[1][i].mode < 2) {
        error_code = ixheaacd_ec_data_dec(pstr_mps_state, &frame->cpc_lossless_data,
                                          frame->cmp_ttt_cpc_1_idx, frame->cmp_ttt_cpc_1_idx_prev,
                                          CPC, i, ttt_off + 4 * i + 2,
                                          p_aux_struct->ttt_config[1][i].bitstream_start_band,
                                          p_aux_struct->ttt_config[1][i].bitstream_stop_band);
        if (error_code != IA_NO_ERROR) return error_code;

        error_code = ixheaacd_ec_data_dec(pstr_mps_state, &frame->cpc_lossless_data,
                                          frame->cmp_ttt_cpc_2_idx, frame->cmp_ttt_cpc_2_idx_prev,
                                          CPC, i, ttt_off + 4 * i + 3,
                                          p_aux_struct->ttt_config[1][i].bitstream_start_band,
                                          p_aux_struct->ttt_config[1][i].bitstream_stop_band);
        if (error_code != IA_NO_ERROR) return error_code;

        error_code = ixheaacd_ec_data_dec(pstr_mps_state, &frame->icc_lossless_data,
                                          frame->cmp_ttt_icc_idx, frame->cmp_ttt_icc_idx_prev,
                                          ICC, i, ttt_off + 4 * i + 2,
                                          p_aux_struct->ttt_config[1][i].bitstream_start_band,
                                          p_aux_struct->ttt_config[1][i].bitstream_stop_band);
        if (error_code != IA_NO_ERROR) return error_code;
      } else {
        error_code = ixheaacd_ec_data_dec(pstr_mps_state, &frame->cld_lossless_data,
                                          frame->cmp_ttt_cld_1_idx, frame->cmp_ttt_cld_1_idx_prev,
                                          CLD, i, ttt_off + 4 * i + 2,
                                          p_aux_struct->ttt_config[1][i].bitstream_start_band,
                                          p_aux_struct->ttt_config[1][i].bitstream_stop_band);
        if (error_code != IA_NO_ERROR) return error_code;

        error_code = ixheaacd_ec_data_dec(pstr_mps_state, &frame->cld_lossless_data,
                                          frame->cmp_ttt_cld_2_idx, frame->cmp_ttt_cld_2_idx_prev,
                                          CLD, i, ttt_off + 4 * i + 3,
                                          p_aux_struct->ttt_config[1][i].bitstream_start_band,
                                          p_aux_struct->ttt_config[1][i].bitstream_stop_band);
        if (error_code != IA_NO_ERROR) return error_code;
      }
    }
  }

  frame->bs_smooth_control = 1;

  if (frame->bs_smooth_control) {
    for (ps = 0; ps < num_parameter_sets; ps++) {
      frame->bs_smooth_mode[ps] = ixheaacd_read_bits_buf(mps_bit_buf, 2);
      if (frame->bs_smooth_mode[ps] > 3 || frame->bs_smooth_mode[ps] < 0) {
        return IA_XHEAAC_MPS_DEC_EXE_NONFATAL_INVALID_SMOOTH_MODE;
      }
      if (frame->bs_smooth_mode[ps] >= 2) {
        frame->bs_smooth_time[ps] = ixheaacd_read_bits_buf(mps_bit_buf, 2);
      }
      if (frame->bs_smooth_mode[ps] == 3) {
        frame->bs_freq_res_stride_smg[ps] = ixheaacd_read_bits_buf(mps_bit_buf, 2);
        data_bands = (bitstream_parameter_bands - 1) /
                         bitdec_table->pb_stride_table[frame->bs_freq_res_stride_smg[ps]] +
                     1;
        for (pg = 0; pg < data_bands; pg++) {
          frame->bs_smg_data[ps][pg] = ixheaacd_read_bits_buf(mps_bit_buf, 1);
        }
      }
    }
  }

  for (i = 0; i < bs_num_output_channels; i++) {
    p_aux_struct->temp_shape_enable_channel_stp[i] = 0;
    p_aux_struct->temp_shape_enable_channel_ges[i] = 0;
  }

  if (p_bs_config->bs_temp_shape_config != 0) {
    bs_temp_shape_enable = ixheaacd_read_bits_buf(mps_bit_buf, 1);
    if (bs_temp_shape_enable) {
      num_temp_shape_chan =
          bitdec_table->temp_shape_chan_table[p_bs_config->bs_temp_shape_config - 1]
                                             [p_bs_config->bs_tree_config];
      switch (pstr_mps_state->temp_shape_config) {
        case 1:
          for (i = 0; i < num_temp_shape_chan; i++) {
            p_aux_struct->temp_shape_enable_channel_stp[i] =
                ixheaacd_read_bits_buf(mps_bit_buf, 1);
          }
          break;
        case 2:
          for (i = 0; i < num_temp_shape_chan; i++) {
            p_aux_struct->temp_shape_enable_channel_ges[i] =
                ixheaacd_read_bits_buf(mps_bit_buf, 1);
          }
          for (i = 0; i < num_temp_shape_chan; i++) {
            if (p_aux_struct->temp_shape_enable_channel_ges[i]) {
              WORD32 const *envshape_data =
                  &bitdec_table->envshape_data[pstr_mps_state->env_quant_mode][0];
              ixheaacd_mps_huff_decode(mps_bit_buf, bs_env_shape_data, time_slots);
              for (ts = 0; ts < time_slots; ts++) {
                p_aux_struct->env_shape_data[i][ts] = envshape_data[bs_env_shape_data[ts]];
              }
            }
          }
          break;
        default:
          return IA_XHEAAC_MPS_DEC_EXE_FATAL_INVALID_TEMPORAL_SHAPING_CONFIG;
      }
    }
  }

  if (pstr_mps_state->up_mix_type == 2) {
    for (i = 0; i < bs_num_output_channels; i++) {
      p_aux_struct->temp_shape_enable_channel_stp[i] = 0;
      p_aux_struct->temp_shape_enable_channel_ges[i] = 0;
    }
  }

  if (pstr_mps_state->arbitrary_downmix != 0) {
    ixheaacd_parse_arbitrary_downmix_data(pstr_mps_state);
  }

  ixheaacd_byte_align(mps_bit_buf, &alignment_bits);
  error_code = ixheaacd_parse_extension_frame(pstr_mps_state);
  if (error_code) {
    if (pstr_mps_state->ec_flag) {
      pstr_mps_state->frame_ok = 0;
    }
    else
      return error_code;
  }

  for (i = 0; i < num_ott_boxes; i++) {
    for (ps = 0; ps < num_parameter_sets; ps++) {
      if (!frame->res_data.bs_icc_diff_present[i][ps] || (pstr_mps_state->up_mix_type == 2) ||
          (pstr_mps_state->up_mix_type == 3)) {
        for (pb = 0; pb < bitstream_parameter_bands; pb++) {
          pstr_mps_state->bs_frame->ott_icc_diff_idx[i][ps][pb] = 0;
        }
      }
    }
  }

  pstr_mps_state->parse_next_bitstream_frame = 1;

  return IA_NO_ERROR;
}

static VOID ixheaacd_create_mapping(WORD32 a_map[MAX_PARAMETER_BANDS + 1], WORD32 start_band,
                                    WORD32 stop_band, WORD32 stride, VOID *scratch) {
  WORD32 in_bands, out_bands, bands_achived, bands_diff, incr, k, i;
  WORD32 *v_dk;
  in_bands = stop_band - start_band;
  out_bands = (in_bands - 1) / stride + 1;
  v_dk = scratch;
  if (out_bands < 1) {
    out_bands = 1;
  }

  bands_achived = out_bands * stride;
  bands_diff = in_bands - bands_achived;
  for (i = 0; i < out_bands; i++) {
    v_dk[i] = stride;
  }

  if (bands_diff > 0) {
    incr = -1;
    k = out_bands - 1;
  } else {
    incr = 1;
    k = 0;
  }

  while (bands_diff != 0) {
    v_dk[k] = v_dk[k] - incr;
    k = k + incr;
    bands_diff = bands_diff + incr;
    if (k >= out_bands) {
      if (bands_diff > 0) {
        k = out_bands - 1;
      } else if (bands_diff < 0) {
        k = 0;
      }
    }
  }
  a_map[0] = start_band;
  for (i = 0; i < out_bands; i++) {
    a_map[i + 1] = a_map[i] + v_dk[i];
  }
}

static VOID ixheaacd_map_frequency(WORD32 *p_input, WORD32 *p_output, WORD32 *p_map,
                                   WORD32 data_bands) {
  WORD32 i, j, start_band, stop_band, value;
  WORD32 start_band_0 = p_map[0];

  for (i = 0; i < data_bands; i++) {
    value = p_input[i + start_band_0];

    start_band = p_map[i];
    stop_band = p_map[i + 1];
    for (j = start_band; j < stop_band; j++) {
      p_output[j] = value;
    }
  }
}

static IA_ERRORCODE ixheaacd_deq_coarse(
    WORD32 value, WORD32 param_type, WORD32 *dequant,
    ia_mps_dec_bitdec_tables_struct *ixheaacd_mps_dec_bitdec_tables) {
  switch (param_type) {
    case CLD:
      if (value >= 8 || value < -7) return IA_FATAL_ERROR;
      *dequant = ixheaacd_mps_dec_bitdec_tables->dequant_cld_coarse[value + 7];
      break;

    case ICC:
      if (value >= 8 || value < 0) return IA_FATAL_ERROR;
      *dequant = ixheaacd_mps_dec_bitdec_tables->dequant_icc[value];
      break;

    case CPC:
      if (value >= 16 || value < -10) return IA_FATAL_ERROR;
      *dequant = ixheaacd_mps_dec_bitdec_tables->dequant_cpc_coarse[value + 10];
      break;
    default:
      return IA_XHEAAC_MPS_DEC_EXE_FATAL_INVALID_DEQUANT_PARAM;
  }
  return IA_NO_ERROR;
}

static IA_ERRORCODE ia_mps_dec_deq(
    WORD32 value, WORD32 param_type, WORD32 *dequant,
    ia_mps_dec_bitdec_tables_struct *ixheaacd_mps_dec_bitdec_tables) {
  switch (param_type) {
    case CLD:
      if (value >= 16 || value < -15) return IA_FATAL_ERROR;
      *dequant = ixheaacd_mps_dec_bitdec_tables->dequant_cld[value + 15];
      break;

    case ICC:
      if (value >= 8 || value < 0) return IA_FATAL_ERROR;
      *dequant = ixheaacd_mps_dec_bitdec_tables->dequant_icc[value];
      break;

    case CPC:
      if (value >= 32 || value < -20) return IA_FATAL_ERROR;
      *dequant = ixheaacd_mps_dec_bitdec_tables->dequant_cpc[value + 20];
      break;

    default:
      return IA_XHEAAC_MPS_DEC_EXE_FATAL_INVALID_DEQUANT_PARAM;
  }
  return IA_NO_ERROR;
}

static IA_ERRORCODE ixheaacd_factor_funct(WORD32 ott_vs_tot_db, WORD32 quant_mode,
                                          WORD32 *factor) {
  WORD32 db_diff;
  WORD32 x_linear = 0;

  WORD32 maxfactor = 0;
  WORD32 constfact;

  if (ott_vs_tot_db > 0) return IA_XHEAAC_MPS_DEC_EXE_FATAL_INVALID_MPS_PARAM;
  db_diff = ixheaac_negate32_sat(ott_vs_tot_db);

  switch (quant_mode) {
    case QUANT_MODE_0:
      return (ONE_IN_Q24);
      break;
    case QUANT_MODE_1:
      x_linear = 1024;

      maxfactor = 83886080;
      constfact = 3277;
      break;
    case QUANT_MODE_2:
      x_linear = 1024;

      maxfactor = (ONE_IN_Q27);
      constfact = 4779;
      break;
    default:
      return IA_XHEAAC_MPS_DEC_EXE_FATAL_INVALID_QUANT_MODE;
  }

  if (db_diff > (x_linear << 5)) {
    WORD32 db_diff_fix = db_diff >> 5;
    *factor = ixheaac_add32_sat(
        ixheaac_sat64_32(ixheaac_mult64(ixheaac_sub32_sat(db_diff_fix, x_linear), constfact)),
        ONE_IN_Q24);
  } else {
    *factor = ONE_IN_Q24;
  }

  *factor = min(maxfactor, *factor);
  return IA_NO_ERROR;
}

static VOID ixheaacd_factor_cld(WORD32 *idx, WORD32 ott_vs_tot_db, WORD32 *ott_vs_tot_db_1,
                                WORD32 *ott_vs_tot_db_2, WORD32 quant_mode,
                                ia_mps_dec_bitdec_tables_struct *ixheaacd_mps_dec_bitdec_tables) {
  WORD32 factor = 0;
  WORD32 c1;
  WORD32 c2;
  WORD32 cld_idx;

  ixheaacd_factor_funct(ott_vs_tot_db, quant_mode, &factor);

  cld_idx = ixheaac_mul32_sh(*idx, factor, 23);
  cld_idx = ixheaac_shr32(ixheaac_add32(cld_idx, 1), 1);

  cld_idx = min(cld_idx, 15);
  cld_idx = max(cld_idx, -15);

  *idx = cld_idx;

  c1 = ixheaacd_mps_dec_bitdec_tables->factor_cld_tab_1[*idx + 15];
  c2 = ixheaacd_mps_dec_bitdec_tables->factor_cld_tab_1[15 - *idx];

  *ott_vs_tot_db_1 = ixheaac_add32_sat(c1, ott_vs_tot_db);
  *ott_vs_tot_db_2 = ixheaac_add32_sat(c2, ott_vs_tot_db);
}

static IA_ERRORCODE ixheaacd_map_index_data(
    ia_mps_dec_lossless_data_struct *ll_data,
    WORD32 output_data[][MAX_PARAMETER_SETS][MAX_PARAMETER_BANDS],
    WORD32 output_idx_data[][MAX_PARAMETER_SETS][MAX_PARAMETER_BANDS],
    WORD32 cmp_idx_data[][MAX_PARAMETER_SETS][MAX_PARAMETER_BANDS],
    WORD32 diff_idx_data[][MAX_PARAMETER_SETS][MAX_PARAMETER_BANDS], WORD32 xtt_idx,
    WORD32 idx_prev[MAX_NUM_OTT][MAX_PARAMETER_BANDS], WORD32 param_idx, WORD32 param_type,
    WORD32 start_band, WORD32 stop_band, WORD32 default_value, WORD32 num_parameter_sets,
    WORD32 *param_slot, WORD32 extend_frame, WORD32 quant_mode, WORD32 *ott_vs_tot_db_in,
    WORD32 *ott_vs_tot_db_1, WORD32 *ott_vs_tot_db_2,
    ia_mps_dec_bitdec_tables_struct *ixheaacd_mps_dec_bitdec_tables, VOID *scratch) {
  WORD32 *a_param_slots;
  WORD32 *a_interpolate;

  WORD32 data_sets;
  WORD32 *a_map;
  VOID *free_scratch;

  WORD32 set_idx, i, band, parm_slot;
  WORD32 data_bands, stride;
  WORD32 ps, pb;

  WORD32 i1, i2, x1, xi, x2;
  WORD32 *db_in;
  WORD32 *db_1, *db_2;
  IA_ERRORCODE error_code = IA_NO_ERROR;
  db_in = ott_vs_tot_db_in;
  db_1 = ott_vs_tot_db_1;
  db_2 = ott_vs_tot_db_2;
  a_param_slots = scratch;
  a_interpolate = a_param_slots + IXHEAAC_GET_SIZE_ALIGNED_TYPE(
                                      MAX_PARAMETER_SETS, sizeof(*a_interpolate), BYTE_ALIGN_8);
  a_map = a_interpolate +
          IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_SETS, sizeof(*a_map), BYTE_ALIGN_8);
  free_scratch = a_map + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS_PLUS_1, sizeof(*a_map),
                                                       BYTE_ALIGN_8);

  data_sets = 0;
  for (i = 0; i < num_parameter_sets; i++) {
    if (ll_data->bs_xxx_data_mode[param_idx][i] == 3) {
      a_param_slots[data_sets] = i;
      data_sets++;
    }
  }

  set_idx = 0;

  for (i = 0; i < num_parameter_sets; i++) {
    if (ll_data->bs_xxx_data_mode[param_idx][i] == 0) {
      ll_data->no_cmp_quant_coarse_xxx[param_idx][i] = 0;
      for (band = start_band; band < stop_band; band++) {
        output_idx_data[xtt_idx][i][band] = default_value;
      }
      for (band = start_band; band < stop_band; band++) {
        idx_prev[xtt_idx][band] = output_idx_data[xtt_idx][i][band];
      }
    }

    if (ll_data->bs_xxx_data_mode[param_idx][i] == 1) {
      for (band = start_band; band < stop_band; band++) {
        output_idx_data[xtt_idx][i][band] = idx_prev[xtt_idx][band];
      }
      ll_data->no_cmp_quant_coarse_xxx[param_idx][i] =
          ll_data->bs_quant_coarse_xxx_prev[param_idx];
    }

    if (ll_data->bs_xxx_data_mode[param_idx][i] == 2) {
      for (band = start_band; band < stop_band; band++) {
        output_idx_data[xtt_idx][i][band] = idx_prev[xtt_idx][band];
      }
      a_interpolate[i] = 1;
    } else {
      a_interpolate[i] = 0;
    }

    if (ll_data->bs_xxx_data_mode[param_idx][i] == 3) {
      parm_slot = a_param_slots[set_idx];
      stride = ixheaacd_mps_dec_bitdec_tables
                   ->pb_stride_table[ll_data->bs_freq_res_stride_xxx[param_idx][set_idx]];
      data_bands = (stop_band - start_band - 1) / stride + 1;
      ixheaacd_create_mapping(a_map, start_band, stop_band, stride, free_scratch);
      ixheaacd_map_frequency(&cmp_idx_data[xtt_idx][set_idx][0],
                             &output_idx_data[xtt_idx][parm_slot][0], a_map, data_bands);

      for (band = start_band; band < stop_band; band++) {
        idx_prev[xtt_idx][band] = output_idx_data[xtt_idx][parm_slot][band];
      }

      ll_data->bs_quant_coarse_xxx_prev[param_idx] =
          ll_data->bs_quant_coarse_xxx[param_idx][set_idx];
      ll_data->no_cmp_quant_coarse_xxx[param_idx][i] =
          ll_data->bs_quant_coarse_xxx[param_idx][set_idx];

      set_idx++;
    }

    if (diff_idx_data != NULL) {
      for (band = start_band; band < stop_band; band++) {
        output_idx_data[xtt_idx][i][band] += diff_idx_data[xtt_idx][i][band];
      }
    }
  }

  for (i = 0; i < num_parameter_sets; i++) {
    if (a_interpolate[i] != 1) {
      if (ll_data->no_cmp_quant_coarse_xxx[param_idx][i] == 1) {
        for (band = start_band; band < stop_band; band++) {
          error_code = ixheaacd_deq_coarse(output_idx_data[xtt_idx][i][band], param_type,
                                           &(output_data[xtt_idx][i][band]),
                                           ixheaacd_mps_dec_bitdec_tables);
          if (error_code) {
            return error_code;
          }
        }
      } else {
        for (band = start_band; band < stop_band; band++) {
          error_code =
              ia_mps_dec_deq(output_idx_data[xtt_idx][i][band], param_type,
                             &(output_data[xtt_idx][i][band]), ixheaacd_mps_dec_bitdec_tables);
          if (error_code) {
            return error_code;
          }
        }
      }
    }
  }

  if (quant_mode && (param_type == CLD)) {
    if (db_in == 0 || db_1 == 0 || db_2 == 0)
      return IA_XHEAAC_MPS_DEC_EXE_FATAL_INVALID_MPS_PARAM;

    for (ps = 0; ps < num_parameter_sets; ps++) {
      if (a_interpolate[ps] != 1) {
        if (ll_data->no_cmp_quant_coarse_xxx[param_idx][ps]) {
          ixheaacd_coarse_2_fine(output_idx_data[xtt_idx][ps], param_type, start_band,
                                 stop_band - start_band);
        }
        for (pb = start_band; pb < stop_band; pb++) {
          ll_data->no_cmp_quant_coarse_xxx[param_idx][ps] = 1;
          ixheaacd_factor_cld(&(output_idx_data[xtt_idx][ps][pb]), *db_in++, &(*db_1++),
                              &(*db_2++), quant_mode, ixheaacd_mps_dec_bitdec_tables);
          ia_mps_dec_deq(output_idx_data[xtt_idx][ps][pb], param_type,
                         &(output_data[xtt_idx][ps][pb]), ixheaacd_mps_dec_bitdec_tables);
        }
      }
    }
  }

  i1 = 0;
  x1 = 0;
  i2 = 0;
  for (i = 0; i < num_parameter_sets; i++) {
    if (a_interpolate[i] != 1) {
      i1 = i;
    }
    i2 = i;
    while (a_interpolate[i2] == 1) {
      i2++;
    }
    x1 = param_slot[i1];
    xi = param_slot[i];
    x2 = param_slot[i2];

    if (a_interpolate[i] == 1) {
      if (i2 >= num_parameter_sets) return IA_XHEAAC_MPS_DEC_EXE_FATAL_INVALID_PARAMETER_SETS;
      if (ll_data->no_cmp_quant_coarse_xxx[param_idx][i1]) {
        ixheaacd_coarse_2_fine(output_idx_data[xtt_idx][i1], param_type, start_band,
                               stop_band - start_band);
      }
      if (ll_data->no_cmp_quant_coarse_xxx[param_idx][i2]) {
        ixheaacd_coarse_2_fine(output_idx_data[xtt_idx][i2], param_type, start_band,
                               stop_band - start_band);
      }
      for (band = start_band; band < stop_band; band++) {
        WORD32 yi = 0, y1, y2;
        y1 = output_idx_data[xtt_idx][i1][band];
        y2 = output_idx_data[xtt_idx][i2][band];

        if (x2 != x1) {
          yi = y1 + (xi - x1) * (y2 - y1) / (x2 - x1);
        }
        output_idx_data[xtt_idx][i][band] = yi;
        ia_mps_dec_deq(output_idx_data[xtt_idx][i][band], param_type,
                       &(output_data[xtt_idx][i][band]), ixheaacd_mps_dec_bitdec_tables);
      }
    }
  }

  ixheaacd_mps_check_index_bounds(output_idx_data, num_parameter_sets, start_band,
                                  stop_band, param_type, xtt_idx);

  if (extend_frame) {
    for (band = start_band; band < stop_band; band++) {
      output_data[xtt_idx][num_parameter_sets][band] =
          output_data[xtt_idx][num_parameter_sets - 1][band];
      output_idx_data[xtt_idx][num_parameter_sets][band] =
          output_idx_data[xtt_idx][num_parameter_sets - 1][band];
    }
  }
  return IA_NO_ERROR;
}

static VOID ixheaacd_get_parameters_mapping(
    WORD32 bs_parameter_bands, WORD32 *mapping,
    ia_mps_dec_bitdec_tables_struct *ixheaacd_mps_dec_bitdec_tables) {
  switch (bs_parameter_bands) {
    case PARAMETER_BANDS_4:
      mapping = ixheaacd_mps_dec_bitdec_tables->map_table.mapping_4_to_28;
      break;
    case PARAMETER_BANDS_5:
      mapping = ixheaacd_mps_dec_bitdec_tables->map_table.mapping_5_to_28;
      break;
    case PARAMETER_BANDS_7:
      mapping = ixheaacd_mps_dec_bitdec_tables->map_table.mapping_7_to_28;
      break;
    case PARAMETER_BANDS_10:
      mapping = ixheaacd_mps_dec_bitdec_tables->map_table.mapping_10_to_28;
      break;
    case PARAMETER_BANDS_14:
      mapping = ixheaacd_mps_dec_bitdec_tables->map_table.mapping_14_to_28;
      break;
    case PARAMETER_BANDS_20:
      mapping = ixheaacd_mps_dec_bitdec_tables->map_table.mapping_20_to_28;
      break;
    case PARAMETER_BANDS_28:
      break;
    default:
      break;
  }
  return;
}

static VOID ixheaacd_map_number_of_bands_to_28_bands(
    WORD32 bands, WORD32 bs_parameter_bands, WORD32 *bands28,
    ia_mps_dec_bitdec_tables_struct *ixheaacd_mps_dec_bitdec_tables) {
  WORD32 *mapping = NULL;
  WORD32 pb;

  *bands28 = bands;

  ixheaacd_get_parameters_mapping(bs_parameter_bands, mapping, ixheaacd_mps_dec_bitdec_tables);

  if (mapping != NULL) {
    for (pb = 0; pb < MAX_PARAMETER_BANDS; pb++) {
      if (mapping[pb] == bands) {
        break;
      }
    }
    *bands28 = pb;
  }
  return;
}

static VOID ixheaacd_map_data_to_28_bands(
    WORD32 *data, WORD32 bs_parameter_bands,
    ia_mps_dec_bitdec_tables_struct *ixheaacd_mps_dec_bitdec_tables) {
  WORD32 *mapping = NULL;
  WORD32 pb;

  ixheaacd_get_parameters_mapping(bs_parameter_bands, mapping, ixheaacd_mps_dec_bitdec_tables);

  if (mapping != NULL) {
    for (pb = MAX_PARAMETER_BANDS - 1; pb >= 0; pb--) {
      data[pb] = data[mapping[pb]];
    }
  }
  return;
}

static IA_ERRORCODE ixheaacd_decode_and_map_frame_ott(ia_heaac_mps_state_struct *pstr_mps_state)
{
  IA_ERRORCODE error_code = IA_NO_ERROR;
  ia_mps_dec_spatial_bs_frame_struct *p_cur_bs;
  ia_heaac_mps_state_struct *curr_state = pstr_mps_state;
  ia_mps_dec_auxilary_struct *p_aux_struct = pstr_mps_state->aux_struct;
  ia_mps_dec_bitdec_tables_struct *bitdec_table =
      pstr_mps_state->ia_mps_dec_mps_table.bitdec_table_ptr;
  WORD32 *param_slot = pstr_mps_state->aux_struct->param_slot;

  WORD32 i, num_parameter_sets, ott_idx, band;
  WORD32 num_ott_boxes;
  VOID *free_scratch;

  WORD32 ps, pb;

  WORD32 *tot_db;
  WORD32 *ott_vs_tot_db_fc;
  WORD32 *ott_vs_tot_db_s;
  WORD32 *ott_vs_tot_db_f;
  WORD32 *ott_vs_tot_db_c;
  WORD32 *ott_vs_tot_db_lr;
  WORD32 *ott_vs_tot_db_l;
  WORD32 *ott_vs_tot_db_r;
  WORD32 *tmp1;
  WORD32 *tmp2;

  WORD32 bitstream_parameter_bands = curr_state->bitstream_parameter_bands;
  WORD32 *b_ott_bands = curr_state->bitstream_ott_bands;
  WORD32 *ott_cld_default = curr_state->ott_cld_default;
  WORD32 parameter_sets = curr_state->num_parameter_sets;
  WORD32 extend_frame = curr_state->extend_frame;
  WORD32 quant_mode = curr_state->quant_mode;

  tot_db = pstr_mps_state->mps_scratch_mem_v;
  ott_vs_tot_db_fc =
      tot_db + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PSXPB, sizeof(*ott_vs_tot_db_fc), BYTE_ALIGN_8);
  ott_vs_tot_db_s = ott_vs_tot_db_fc + IXHEAAC_GET_SIZE_ALIGNED_TYPE(
                                           MAX_PSXPB, sizeof(*ott_vs_tot_db_s), BYTE_ALIGN_8);
  ott_vs_tot_db_f = ott_vs_tot_db_s + IXHEAAC_GET_SIZE_ALIGNED_TYPE(
                                          MAX_PSXPB, sizeof(*ott_vs_tot_db_f), BYTE_ALIGN_8);
  ott_vs_tot_db_c = ott_vs_tot_db_f + IXHEAAC_GET_SIZE_ALIGNED_TYPE(
                                          MAX_PSXPB, sizeof(*ott_vs_tot_db_c), BYTE_ALIGN_8);
  ott_vs_tot_db_lr = ott_vs_tot_db_c + IXHEAAC_GET_SIZE_ALIGNED_TYPE(
                                           MAX_PSXPB, sizeof(*ott_vs_tot_db_lr), BYTE_ALIGN_8);
  ott_vs_tot_db_l = ott_vs_tot_db_lr + IXHEAAC_GET_SIZE_ALIGNED_TYPE(
                                           MAX_PSXPB, sizeof(*ott_vs_tot_db_l), BYTE_ALIGN_8);
  ott_vs_tot_db_r = ott_vs_tot_db_l + IXHEAAC_GET_SIZE_ALIGNED_TYPE(
                                          MAX_PSXPB, sizeof(*ott_vs_tot_db_r), BYTE_ALIGN_8);
  tmp1 = ott_vs_tot_db_r + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PSXPB, sizeof(*tmp1), BYTE_ALIGN_8);
  tmp2 = tmp1 + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PSXPB, sizeof(*tmp2), BYTE_ALIGN_8);
  free_scratch = tmp2 + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PSXPB, sizeof(*tmp2), BYTE_ALIGN_8);

  p_cur_bs = pstr_mps_state->bs_frame;
  num_ott_boxes = curr_state->num_ott_boxes;

  pb = MAX_PSXPB;
  for (i = 0; i < pb; i++) tot_db[i] = 0;

  switch (curr_state->tree_config) {
    case TREE_5151:
      i = 0;
      error_code = ixheaacd_map_index_data(
          &p_cur_bs->cld_lossless_data, p_aux_struct->ott_cld, p_cur_bs->ott_cld_idx,
          p_cur_bs->cmp_ott_cld_idx, NULL, i, p_cur_bs->ott_cld_idx_prev, i, CLD, 0,
          b_ott_bands[i], ott_cld_default[i], parameter_sets, param_slot, extend_frame,
          quant_mode, tot_db, ott_vs_tot_db_fc, ott_vs_tot_db_s, bitdec_table, free_scratch);
      if (error_code) return error_code;

      i = 1;
      error_code = ixheaacd_map_index_data(&p_cur_bs->cld_lossless_data, p_aux_struct->ott_cld,
                              p_cur_bs->ott_cld_idx, p_cur_bs->cmp_ott_cld_idx, NULL, i,
                              p_cur_bs->ott_cld_idx_prev, i, CLD, 0, b_ott_bands[i],
                              ott_cld_default[i], parameter_sets, param_slot, extend_frame,
                              quant_mode, ott_vs_tot_db_fc, ott_vs_tot_db_f, ott_vs_tot_db_c,
                              bitdec_table, free_scratch);
      if (error_code) return error_code;

      i = 2;
      error_code = ixheaacd_map_index_data(
          &p_cur_bs->cld_lossless_data, p_aux_struct->ott_cld, p_cur_bs->ott_cld_idx,
          p_cur_bs->cmp_ott_cld_idx, NULL, i, p_cur_bs->ott_cld_idx_prev, i, CLD, 0,
          b_ott_bands[i], ott_cld_default[i], parameter_sets, param_slot, extend_frame,
          quant_mode, ott_vs_tot_db_s, tmp1, tmp2, bitdec_table, free_scratch);
      if (error_code) return error_code;

      i = 3;
      error_code = ixheaacd_map_index_data(
          &p_cur_bs->cld_lossless_data, p_aux_struct->ott_cld, p_cur_bs->ott_cld_idx,
          p_cur_bs->cmp_ott_cld_idx, NULL, i, p_cur_bs->ott_cld_idx_prev, i, CLD, 0,
          b_ott_bands[i], ott_cld_default[i], parameter_sets, param_slot, extend_frame,
          quant_mode, ott_vs_tot_db_f, tmp1, tmp2, bitdec_table, free_scratch);
      if (error_code) return error_code;

      i = 4;
      error_code = ixheaacd_map_index_data(&p_cur_bs->cld_lossless_data, p_aux_struct->ott_cld,
                              p_cur_bs->ott_cld_idx, p_cur_bs->cmp_ott_cld_idx, NULL, i,
                              p_cur_bs->ott_cld_idx_prev, i, CLD, 0, b_ott_bands[i],
                              ott_cld_default[i], parameter_sets, param_slot, extend_frame,
                              quant_mode, tot_db, tmp1, tmp2, bitdec_table, free_scratch);
      if (error_code) return error_code;

      break;

    case TREE_5152:
      i = 0;
      error_code = ixheaacd_map_index_data(
          &p_cur_bs->cld_lossless_data, p_aux_struct->ott_cld, p_cur_bs->ott_cld_idx,
          p_cur_bs->cmp_ott_cld_idx, NULL, i, p_cur_bs->ott_cld_idx_prev, i, CLD, 0,
          b_ott_bands[i], ott_cld_default[i], parameter_sets, param_slot, extend_frame,
          quant_mode, tot_db, ott_vs_tot_db_lr, ott_vs_tot_db_c, bitdec_table, free_scratch);
      if (error_code) return error_code;

      i = 1;
      error_code = ixheaacd_map_index_data(&p_cur_bs->cld_lossless_data, p_aux_struct->ott_cld,
                              p_cur_bs->ott_cld_idx, p_cur_bs->cmp_ott_cld_idx, NULL, i,
                              p_cur_bs->ott_cld_idx_prev, i, CLD, 0, b_ott_bands[i],
                              ott_cld_default[i], parameter_sets, param_slot, extend_frame,
                              quant_mode, ott_vs_tot_db_lr, ott_vs_tot_db_l, ott_vs_tot_db_r,
                              bitdec_table, free_scratch);
      if (error_code) return error_code;

      i = 2;
      error_code = ixheaacd_map_index_data(&p_cur_bs->cld_lossless_data, p_aux_struct->ott_cld,
                              p_cur_bs->ott_cld_idx, p_cur_bs->cmp_ott_cld_idx, NULL, i,
                              p_cur_bs->ott_cld_idx_prev, i, CLD, 0, b_ott_bands[i],
                              ott_cld_default[i], parameter_sets, param_slot, extend_frame,
                              quant_mode, tot_db, tmp1, tmp2, bitdec_table, free_scratch);
      if (error_code) return error_code;

      i = 3;
      error_code = ixheaacd_map_index_data(
          &p_cur_bs->cld_lossless_data, p_aux_struct->ott_cld, p_cur_bs->ott_cld_idx,
          p_cur_bs->cmp_ott_cld_idx, NULL, i, p_cur_bs->ott_cld_idx_prev, i, CLD, 0,
          b_ott_bands[i], ott_cld_default[i], parameter_sets, param_slot, extend_frame,
          quant_mode, ott_vs_tot_db_l, tmp1, tmp2, bitdec_table, free_scratch);
      if (error_code) return error_code;

      i = 4;
      error_code = ixheaacd_map_index_data(
          &p_cur_bs->cld_lossless_data, p_aux_struct->ott_cld, p_cur_bs->ott_cld_idx,
          p_cur_bs->cmp_ott_cld_idx, NULL, i, p_cur_bs->ott_cld_idx_prev, i, CLD, 0,
          b_ott_bands[i], ott_cld_default[i], parameter_sets, param_slot, extend_frame,
          quant_mode, ott_vs_tot_db_r, tmp1, tmp2, bitdec_table, free_scratch);
      if (error_code) return error_code;
      break;

    default:
      for (i = 0; i < num_ott_boxes; i++) {
        error_code = ixheaacd_map_index_data(&p_cur_bs->cld_lossless_data, p_aux_struct->ott_cld,
                                p_cur_bs->ott_cld_idx, p_cur_bs->cmp_ott_cld_idx, NULL, i,
                                p_cur_bs->ott_cld_idx_prev, i, CLD, 0, b_ott_bands[i],
                                ott_cld_default[i], parameter_sets, param_slot, extend_frame,
                                (curr_state->tree_config == TREE_525) ? 0 : quant_mode, NULL,
                                NULL, NULL, bitdec_table, free_scratch);
        if (error_code) return error_code;
      }
      break;
  }

  if (curr_state->one_icc == 1) {
    if (extend_frame == 0) {
      num_parameter_sets = parameter_sets;
    } else {
      num_parameter_sets = parameter_sets + 1;
    }

    for (ott_idx = 1; ott_idx < num_ott_boxes; ott_idx++) {
      if (curr_state->ott_mode_lfe[ott_idx] == 0) {
        for (i = 0; i < num_parameter_sets; i++) {
          for (band = 0; band < bitstream_parameter_bands; band++) {
            p_cur_bs->cmp_ott_icc_idx[ott_idx][i][band] = p_cur_bs->cmp_ott_icc_idx[0][i][band];
          }
        }
      }
    }

    for (ott_idx = 0; ott_idx < num_ott_boxes; ott_idx++) {
      if (curr_state->ott_mode_lfe[ott_idx] == 0) {
        error_code = ixheaacd_map_index_data(&p_cur_bs->icc_lossless_data, p_aux_struct->ott_icc,
                                p_cur_bs->ott_icc_idx, p_cur_bs->cmp_ott_icc_idx,
                                p_cur_bs->ott_icc_diff_idx, ott_idx, p_cur_bs->ott_icc_idx_prev,
                                0, ICC, 0, b_ott_bands[ott_idx], curr_state->icc_default,
                                parameter_sets, param_slot, extend_frame, quant_mode, NULL, NULL,
                                NULL, bitdec_table, free_scratch);
        if (error_code) return error_code;
      }
    }
  } else {
    for (ott_idx = 0; ott_idx < num_ott_boxes; ott_idx++) {
      if (curr_state->ott_mode_lfe[ott_idx] == 0) {
        error_code = ixheaacd_map_index_data(&p_cur_bs->icc_lossless_data, p_aux_struct->ott_icc,
                                p_cur_bs->ott_icc_idx, p_cur_bs->cmp_ott_icc_idx,
                                p_cur_bs->ott_icc_diff_idx, ott_idx, p_cur_bs->ott_icc_idx_prev,
                                ott_idx, ICC, 0, b_ott_bands[ott_idx], curr_state->icc_default,
                                parameter_sets, param_slot, extend_frame, quant_mode, NULL, NULL,
                                NULL, bitdec_table, free_scratch);
        if (error_code) return error_code;
      }
    }
  }

  if (curr_state->up_mix_type == 2) {
    WORD32 num_parameter_sets = parameter_sets;

    if (extend_frame) {
      num_parameter_sets++;
    }

    for (ott_idx = 0; ott_idx < curr_state->num_ott_boxes; ott_idx++) {
      for (ps = 0; ps < num_parameter_sets; ps++) {
        ixheaacd_map_data_to_28_bands(p_aux_struct->ott_cld[ott_idx][ps],
                                      bitstream_parameter_bands, bitdec_table);
        ixheaacd_map_data_to_28_bands(p_aux_struct->ott_icc[ott_idx][ps],
                                      bitstream_parameter_bands, bitdec_table);
      }
    }
  }
  return error_code;
}

static IA_ERRORCODE ixheaacd_decode_and_map_frame_ttt(ia_heaac_mps_state_struct *pstr_mps_state)
{
  IA_ERRORCODE error_code = IA_NO_ERROR;
  ia_mps_dec_spatial_bs_frame_struct *p_cur_bs;
  ia_mps_dec_bitdec_tables_struct *bitdec_table =
      pstr_mps_state->ia_mps_dec_mps_table.bitdec_table_ptr;
  ia_mps_dec_auxilary_struct *p_aux_struct = pstr_mps_state->aux_struct;
  WORD32 *param_slot = pstr_mps_state->aux_struct->param_slot;

  WORD32 num_bands;

  WORD32 i, j, offset;
  WORD32 num_ttt_boxes;
  VOID *free_scratch;

  p_cur_bs = pstr_mps_state->bs_frame;
  num_bands = pstr_mps_state->bitstream_parameter_bands;
  offset = pstr_mps_state->num_ott_boxes;
  num_ttt_boxes = pstr_mps_state->num_ttt_boxes;
  free_scratch = pstr_mps_state->mps_scratch_mem_v;

  for (i = 0; i < num_ttt_boxes; i++) {
    for (j = 0;
         (j < 2) &&
         p_aux_struct->ttt_config[j][i].start_band < p_aux_struct->ttt_config[j][i].stop_band;
         j++) {
      if (p_aux_struct->ttt_config[j][i].mode < 2) {
        error_code = ixheaacd_map_index_data(
            &p_cur_bs->cpc_lossless_data, p_aux_struct->ttt_cpc_1, p_cur_bs->ttt_cpc_1_idx,
            p_cur_bs->cmp_ttt_cpc_1_idx, NULL, i, p_cur_bs->ttt_cpc_1_idx_prev,
            offset + 4 * i + 2 * j, CPC, p_aux_struct->ttt_config[j][i].bitstream_start_band,
            p_aux_struct->ttt_config[j][i].bitstream_stop_band, pstr_mps_state->cpc_default,
            pstr_mps_state->num_parameter_sets, param_slot, pstr_mps_state->extend_frame,
            pstr_mps_state->quant_mode, NULL, NULL, NULL, bitdec_table, free_scratch);
        if (error_code) return error_code;

        error_code = ixheaacd_map_index_data(
            &p_cur_bs->cpc_lossless_data, p_aux_struct->ttt_cpc_2, p_cur_bs->ttt_cpc_2_idx,
            p_cur_bs->cmp_ttt_cpc_2_idx, NULL, i, p_cur_bs->ttt_cpc_2_idx_prev,
            offset + 4 * i + 1 + 2 * j, CPC, p_aux_struct->ttt_config[j][i].bitstream_start_band,
            p_aux_struct->ttt_config[j][i].bitstream_stop_band, pstr_mps_state->cpc_default,
            pstr_mps_state->num_parameter_sets, param_slot, pstr_mps_state->extend_frame,
            pstr_mps_state->quant_mode, NULL, NULL, NULL, bitdec_table, free_scratch);
        if (error_code) return error_code;

        error_code = ixheaacd_map_index_data(
            &p_cur_bs->icc_lossless_data, p_aux_struct->ttt_icc, p_cur_bs->ttt_icc_idx,
            p_cur_bs->cmp_ttt_icc_idx, NULL, i, p_cur_bs->ttt_icc_idx_prev,
            offset + 4 * i + 2 * j, ICC, p_aux_struct->ttt_config[j][i].bitstream_start_band,
            p_aux_struct->ttt_config[j][i].bitstream_stop_band, pstr_mps_state->icc_default,
            pstr_mps_state->num_parameter_sets, param_slot, pstr_mps_state->extend_frame,
            pstr_mps_state->quant_mode, NULL, NULL, NULL, bitdec_table, free_scratch);
        if (error_code) return error_code;
      }

      else {
        error_code = ixheaacd_map_index_data(
            &p_cur_bs->cld_lossless_data, p_aux_struct->ttt_cld_1, p_cur_bs->ttt_cld_1_idx,
            p_cur_bs->cmp_ttt_cld_1_idx, NULL, i, p_cur_bs->ttt_cld_1_idx_prev,
            offset + 4 * i + 2 * j, CLD, p_aux_struct->ttt_config[j][i].bitstream_start_band,
            p_aux_struct->ttt_config[j][i].bitstream_stop_band,
            pstr_mps_state->ttt_cld_1_default[i], pstr_mps_state->num_parameter_sets, param_slot,
            pstr_mps_state->extend_frame, pstr_mps_state->quant_mode, NULL, NULL, NULL,
            bitdec_table, free_scratch);
        if (error_code) return error_code;

        error_code = ixheaacd_map_index_data(
            &p_cur_bs->cld_lossless_data, p_aux_struct->ttt_cld_2, p_cur_bs->ttt_cld_2_idx,
            p_cur_bs->cmp_ttt_cld_2_idx, NULL, i, p_cur_bs->ttt_cld_2_idx_prev,
            offset + 4 * i + 1 + 2 * j, CLD, p_aux_struct->ttt_config[j][i].bitstream_start_band,
            p_aux_struct->ttt_config[j][i].bitstream_stop_band,
            pstr_mps_state->ttt_cld_2_default[i], pstr_mps_state->num_parameter_sets, param_slot,
            pstr_mps_state->extend_frame, pstr_mps_state->quant_mode, NULL, NULL, NULL,
            bitdec_table, free_scratch);
        if (error_code) return error_code;
      }

      if (pstr_mps_state->up_mix_type == 2) {
        WORD32 num_parameter_sets = pstr_mps_state->num_parameter_sets;
        WORD32 ps;

        if (pstr_mps_state->extend_frame) {
          num_parameter_sets++;
        }

        for (ps = 0; ps < num_parameter_sets; ps++) {
          ixheaacd_map_data_to_28_bands(p_aux_struct->ttt_cpc_1[i][ps], num_bands, bitdec_table);
          ixheaacd_map_data_to_28_bands(p_aux_struct->ttt_cpc_2[i][ps], num_bands, bitdec_table);
          ixheaacd_map_data_to_28_bands(p_aux_struct->ttt_cld_1[i][ps], num_bands, bitdec_table);
          ixheaacd_map_data_to_28_bands(p_aux_struct->ttt_cld_2[i][ps], num_bands, bitdec_table);
          ixheaacd_map_data_to_28_bands(p_aux_struct->ttt_icc[i][ps], num_bands, bitdec_table);
        }
      }
    }
  }
  return error_code;
}

static VOID ixheaacd_decode_and_map_frame_smg(ia_heaac_mps_state_struct *pstr_mps_state) {
  ia_mps_dec_smoothing_state_struct *smooth_state =
      pstr_mps_state->mps_persistent_mem.smooth_state;
  ia_mps_dec_bitdec_tables_struct *bitdec_table =
      pstr_mps_state->ia_mps_dec_mps_table.bitdec_table_ptr;
  ia_mps_dec_auxilary_struct *p_aux_struct = pstr_mps_state->aux_struct;
  WORD32 *smg_time = p_aux_struct->smg_time;
  WORD32 ps, pb, pg, pb_stride, data_bands, pb_start, pb_stop;
  WORD32 *a_group_to_band;
  VOID *free_scratch;
  ia_mps_dec_spatial_bs_frame_struct *frame = pstr_mps_state->bs_frame;
  pstr_mps_state->smooth_control = frame->bs_smooth_control;
  a_group_to_band = pstr_mps_state->mps_scratch_mem_v;
  free_scratch =
      a_group_to_band + IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_PARAMETER_BANDS_PLUS_1,
                                                      sizeof(*a_group_to_band), BYTE_ALIGN_8);

  if (pstr_mps_state->smooth_control) {
    for (ps = 0; ps < pstr_mps_state->num_parameter_sets; ps++) {
      switch (frame->bs_smooth_mode[ps]) {
        case SMOOTH_MODE_0:
          smg_time[ps] = 256;
          for (pb = 0; pb < pstr_mps_state->bitstream_parameter_bands; pb++) {
            p_aux_struct->smg_data[ps][pb] = 0;
          }
          break;

        case SMOOTH_MODE_1:
          if (ps > 0)
            smg_time[ps] = smg_time[ps - 1];
          else
            smg_time[ps] = smooth_state->prev_smg_time;

          for (pb = 0; pb < pstr_mps_state->bitstream_parameter_bands; pb++) {
            if (ps > 0)
              p_aux_struct->smg_data[ps][pb] = p_aux_struct->smg_data[ps - 1][pb];
            else
              p_aux_struct->smg_data[ps][pb] = smooth_state->prev_smg_data[pb];
          }
          break;

        case SMOOTH_MODE_2:
          smg_time[ps] = bitdec_table->smg_time_table[frame->bs_smooth_time[ps]];
          for (pb = 0; pb < pstr_mps_state->bitstream_parameter_bands; pb++) {
            p_aux_struct->smg_data[ps][pb] = 1;
          }
          break;

        case SMOOTH_MODE_3:
          smg_time[ps] = bitdec_table->smg_time_table[frame->bs_smooth_time[ps]];
          pb_stride = bitdec_table->pb_stride_table[frame->bs_freq_res_stride_smg[ps]];
          data_bands = (pstr_mps_state->bitstream_parameter_bands - 1) / pb_stride + 1;
          ixheaacd_create_mapping(a_group_to_band, 0, pstr_mps_state->bitstream_parameter_bands,
                                  pb_stride, free_scratch);
          for (pg = 0; pg < data_bands; pg++) {
            pb_start = a_group_to_band[pg];
            pb_stop = a_group_to_band[pg + 1];
            for (pb = pb_start; pb < pb_stop; pb++) {
              p_aux_struct->smg_data[ps][pb] = frame->bs_smg_data[ps][pg];
            }
          }
          break;

        default:
          break;
      }
    }

    smooth_state->prev_smg_time = smg_time[pstr_mps_state->num_parameter_sets - 1];
    for (pb = 0; pb < pstr_mps_state->bitstream_parameter_bands; pb++) {
      smooth_state->prev_smg_data[pb] =
          p_aux_struct->smg_data[pstr_mps_state->num_parameter_sets - 1][pb];
    }

    if (pstr_mps_state->extend_frame) {
      smg_time[pstr_mps_state->num_parameter_sets] =
          smg_time[pstr_mps_state->num_parameter_sets - 1];
      for (pb = 0; pb < pstr_mps_state->bitstream_parameter_bands; pb++) {
        p_aux_struct->smg_data[pstr_mps_state->num_parameter_sets][pb] =
            p_aux_struct->smg_data[pstr_mps_state->num_parameter_sets - 1][pb];
      }
    }

    if (pstr_mps_state->up_mix_type == 2) {
      WORD32 *mapping = NULL;
      ixheaacd_get_parameters_mapping(pstr_mps_state->bitstream_parameter_bands, mapping,
                                      bitdec_table);

      if (mapping != NULL) {
        WORD32 num_parameter_sets = pstr_mps_state->num_parameter_sets;

        if (pstr_mps_state->extend_frame) {
          num_parameter_sets++;
        }

        for (ps = 0; ps < num_parameter_sets; ps++) {
          for (pb = MAX_PARAMETER_BANDS - 1; pb >= 0; pb--) {
            p_aux_struct->smg_data[ps][pb] = p_aux_struct->smg_data[ps][mapping[pb]];
          }
        }
      }
    }
  }
  return;
}

static IA_ERRORCODE ixheaacd_decode_and_map_frame_arbdmx(
  ia_heaac_mps_state_struct *pstr_mps_state) {
  IA_ERRORCODE error_code = IA_NO_ERROR;
  ia_mps_dec_spatial_bs_frame_struct *frame = pstr_mps_state->bs_frame;
  ia_mps_dec_bitdec_tables_struct *bitdec_table =
      pstr_mps_state->ia_mps_dec_mps_table.bitdec_table_ptr;
  ia_mps_dec_auxilary_struct *p_aux_struct = pstr_mps_state->aux_struct;
  WORD32 *param_slot = p_aux_struct->param_slot;
  WORD32 offset = pstr_mps_state->num_ott_boxes + 4 * pstr_mps_state->num_ttt_boxes;
  WORD32 ch;

  VOID *scratch = pstr_mps_state->mps_scratch_mem_v;

  for (ch = 0; ch < pstr_mps_state->num_input_channels; ch++) {
    error_code = ixheaacd_map_index_data(
        &frame->cld_lossless_data, p_aux_struct->arbdmx_gain, frame->arbdmx_gain_idx,
        frame->cmp_arbdmx_gain_idx, NULL, ch, frame->arbdmx_gain_idx_prev, offset + ch, CLD, 0,
        pstr_mps_state->bitstream_parameter_bands, pstr_mps_state->arbdmx_gain_default,
        pstr_mps_state->num_parameter_sets, param_slot, pstr_mps_state->extend_frame, 0, NULL,
        NULL, NULL, bitdec_table, scratch);
    if (error_code) return error_code;

    p_aux_struct->arbdmx_residual_abs[ch] = frame->bs_arbitrary_downmix_residual_abs[ch];
    p_aux_struct->arbdmx_alpha_upd_set[ch] =
        frame->bs_arbitrary_downmix_residual_alpha_update_set[ch];

    if (pstr_mps_state->up_mix_type == 2) {
      WORD32 num_parameter_sets = pstr_mps_state->num_parameter_sets;
      WORD32 ps;

      if (pstr_mps_state->extend_frame) {
        num_parameter_sets++;
      }

      for (ps = 0; ps < num_parameter_sets; ps++) {
        ixheaacd_map_data_to_28_bands(p_aux_struct->arbdmx_gain[ch][ps],
                                      pstr_mps_state->bitstream_parameter_bands, bitdec_table);
      }
    }
  }
  return error_code;
}

static IA_ERRORCODE ixheaacd_decode_and_map_frame_arb_tree(
  ia_heaac_mps_state_struct *pstr_mps_state) {
  IA_ERRORCODE error_code = IA_NO_ERROR;
  ia_mps_dec_spatial_bs_frame_struct *p_cur_bs = pstr_mps_state->bs_frame;
  ia_mps_spatial_bs_config_struct *p_config = &(pstr_mps_state->bs_config);
  ia_mps_dec_auxilary_struct *p_aux_struct = pstr_mps_state->aux_struct;
  WORD32 *param_slot = p_aux_struct->param_slot;
  WORD32 offset = pstr_mps_state->num_ott_boxes;

  VOID *scratch = pstr_mps_state->mps_scratch_mem_v;

  WORD32 i;

  for (i = 0; i < p_config->num_ott_boxes_at; i++) {
    error_code = ixheaacd_map_index_data(&p_cur_bs->cld_lossless_data, p_aux_struct->ott_cld,
                            p_cur_bs->ott_cld_idx, p_cur_bs->cmp_ott_cld_idx, NULL, offset + i,
                            p_cur_bs->ott_cld_idx_prev, offset + i, CLD, 0,
                            p_config->bs_ott_bands_at[i], p_config->bs_ott_default_cld_at[i],
                            pstr_mps_state->num_parameter_sets, param_slot,
                            pstr_mps_state->extend_frame, pstr_mps_state->quant_mode, NULL, NULL,
                            NULL, pstr_mps_state->ia_mps_dec_mps_table.bitdec_table_ptr, scratch);
    if (error_code) return error_code;
  }
  return error_code;
}

IA_ERRORCODE ixheaacd_decode_frame(ia_heaac_mps_state_struct *pstr_mps_state) {
  IA_ERRORCODE error_code = IA_NO_ERROR;
  ia_mps_spatial_bs_config_struct *p_bs_config = &pstr_mps_state->bs_config;
  WORD32 *param_slot = pstr_mps_state->aux_struct->param_slot;

  pstr_mps_state->extend_frame = 0;
  if (param_slot[pstr_mps_state->num_parameter_sets - 1] != pstr_mps_state->time_slots - 1) {
    pstr_mps_state->extend_frame = 1;
  }
  if (pstr_mps_state->extend_frame)
  {
    if (pstr_mps_state->num_parameter_sets == MAX_PARAMETER_SETS) {
      if (pstr_mps_state->ec_flag)
        pstr_mps_state->num_parameter_sets = 1;
      else
        return IA_FATAL_ERROR;
    }
  }

  error_code = ixheaacd_decode_and_map_frame_ott(pstr_mps_state);
  if (error_code)
  {
    if (pstr_mps_state->ec_flag)
    {
      pstr_mps_state->frame_ok = 0;
      for (WORD32 idx = 0; idx < MAX_NUM_OTT; idx++)
      {
        ixheaacd_mps_check_index_bounds(pstr_mps_state->bs_frame->ott_cld_idx,
            pstr_mps_state->num_parameter_sets, 0, pstr_mps_state->bitstream_ott_bands[idx],
            CLD, idx);
        ixheaacd_mps_check_index_bounds(pstr_mps_state->bs_frame->ott_icc_idx,
            pstr_mps_state->num_parameter_sets, 0, pstr_mps_state->bitstream_ott_bands[idx],
            ICC, idx);
      }
    }
    else
      return error_code;
  }

  error_code = ixheaacd_decode_and_map_frame_ttt(pstr_mps_state);
  if (error_code)
  {
    if (pstr_mps_state->ec_flag)
    {
      pstr_mps_state->frame_ok = 0;
      ixheaacd_mps_check_index_bounds(pstr_mps_state->bs_frame->ttt_icc_idx,
          pstr_mps_state->num_parameter_sets, 0, MAX_PARAMETER_BANDS,
          ICC, 0);
    }
    else
      return error_code;
  }

  ixheaacd_decode_and_map_frame_smg(pstr_mps_state);
  if (p_bs_config->arbitrary_tree != 0) {
    error_code = ixheaacd_decode_and_map_frame_arb_tree(pstr_mps_state);
    if (error_code)
    {
      if (pstr_mps_state->ec_flag)
      {
        pstr_mps_state->frame_ok = 0;
        for (WORD32 idx = 0; idx < MAX_NUM_OTT; idx++)
        {
          ixheaacd_mps_check_index_bounds(pstr_mps_state->bs_frame->ott_cld_idx,
              pstr_mps_state->num_parameter_sets, 0, MAX_PARAMETER_BANDS,
              CLD, idx);
        }
      }
      else
        return error_code;
    }
  }

  if (pstr_mps_state->arbitrary_downmix != 0) {
    error_code = ixheaacd_decode_and_map_frame_arbdmx(pstr_mps_state);
    if (error_code)
    {
      if (pstr_mps_state->ec_flag)
      {
        pstr_mps_state->frame_ok = 0;
        for (WORD32 idx = 0; idx < MAX_INPUT_CHANNELS_MPS; idx++)
        {
          ixheaacd_mps_check_index_bounds(pstr_mps_state->bs_frame->arbdmx_gain_idx,
              pstr_mps_state->num_parameter_sets, 0, MAX_PARAMETER_BANDS,
              CLD, idx);
        }
      }
      else
        return error_code;
    }
  }

  if (pstr_mps_state->extend_frame) {
    pstr_mps_state->num_parameter_sets++;
    param_slot[pstr_mps_state->num_parameter_sets - 1] = pstr_mps_state->time_slots - 1;
  }
  return IA_NO_ERROR;
}

IA_ERRORCODE ixheaacd_set_current_state_parameters(ia_heaac_mps_state_struct *pstr_mps_state) {
  WORD32 i;

  ia_mps_spatial_bs_config_struct *config = &(pstr_mps_state->bs_config);
  ia_mps_spatial_bs_config_struct *p_bs_config = &pstr_mps_state->bs_config;
  ia_heaac_mps_state_struct *curr_state = pstr_mps_state;
  ia_mps_dec_bitdec_tables_struct *bitdec_table =
      pstr_mps_state->ia_mps_dec_mps_table.bitdec_table_ptr;
  ia_mps_dec_auxilary_struct *p_aux_struct = pstr_mps_state->aux_struct;
  WORD32 *b_ott_bands = curr_state->bitstream_ott_bands;

  if (config->bs_sampling_freq_index == 15) {
    curr_state->sampling_freq = config->bs_sampling_frequency;
  } else {
    curr_state->sampling_freq = bitdec_table->sampling_freq_table[config->bs_sampling_freq_index];
  }
  curr_state->time_slots = config->bs_frame_length + 1;
  curr_state->frame_length = curr_state->time_slots * curr_state->qmf_bands;
  curr_state->bitstream_parameter_bands = bitdec_table->freq_res_table[config->bs_freq_res];

  curr_state->hybrid_bands = curr_state->qmf_bands - QMF_BANDS_TO_HYBRID + 10;
  curr_state->tp_hyb_band_border = 12;
  if (curr_state->hybrid_bands > 71) {
    return IA_FATAL_ERROR;
  }
  if (curr_state->up_mix_type == 2) {
    curr_state->num_parameter_bands = MAX_PARAMETER_BANDS;
  } else {
    curr_state->num_parameter_bands = curr_state->bitstream_parameter_bands;
  }

  switch (curr_state->num_parameter_bands) {
    case PARAMETER_BANDS_4:
      for (i = 0; i < curr_state->hybrid_bands; i++) {
        curr_state->kernels[i] = bitdec_table->kernel_table.kernels_4_to_71[i];
      }
      break;
    case PARAMETER_BANDS_5:
      for (i = 0; i < curr_state->hybrid_bands; i++) {
        curr_state->kernels[i] = bitdec_table->kernel_table.kernels_5_to_71[i];
      }
      break;
    case PARAMETER_BANDS_7:
      for (i = 0; i < curr_state->hybrid_bands; i++) {
        curr_state->kernels[i] = bitdec_table->kernel_table.kernels_7_to_71[i];
      }
      break;
    case PARAMETER_BANDS_10:
      for (i = 0; i < curr_state->hybrid_bands; i++) {
        curr_state->kernels[i] = bitdec_table->kernel_table.kernels_10_to_71[i];
      }
      break;
    case PARAMETER_BANDS_14:
      for (i = 0; i < curr_state->hybrid_bands; i++) {
        curr_state->kernels[i] = bitdec_table->kernel_table.kernels_14_to_71[i];
      }
      break;
    case PARAMETER_BANDS_20:
      for (i = 0; i < curr_state->hybrid_bands; i++) {
        curr_state->kernels[i] = bitdec_table->kernel_table.kernels_20_to_71[i];
      }
      break;
    case PARAMETER_BANDS_28:
      for (i = 0; i < curr_state->hybrid_bands; i++) {
        curr_state->kernels[i] = bitdec_table->kernel_table.kernels_28_to_71[i];
      }
      break;
    default:
      return IA_XHEAAC_MPS_DEC_EXE_FATAL_INVALID_PARAMETER_BANDS;
  };

  curr_state->tree_config = config->bs_tree_config;

  switch (curr_state->tree_config) {
    case TREE_5151:
    case TREE_5152:
    case TREE_525:
      config->ui_channel_mask = FIVE_POINT_ONE_CHANNEL_MASK;
      break;
    case TREE_7271:
    case TREE_7571:
      config->ui_channel_mask = SEVEN_POINT_ONE_CHANNEL_MASK1;
      break;
    case TREE_7272:
    case TREE_7572:
      config->ui_channel_mask = SEVEN_POINT_ONE_CHANNEL_MASK2;
      break;
    default:
      return IA_XHEAAC_MPS_DEC_EXE_FATAL_UNSUPPRORTED_TREE_CONFIG;
  }

  curr_state->num_ott_boxes =
      bitdec_table->tree_property_table[curr_state->tree_config].num_ott_boxes;
  curr_state->num_ttt_boxes =
      bitdec_table->tree_property_table[curr_state->tree_config].num_ttt_boxes;
  curr_state->num_input_channels =
      bitdec_table->tree_property_table[curr_state->tree_config].num_input_channels;
  curr_state->num_output_channels =
      bitdec_table->tree_property_table[curr_state->tree_config].num_output_channels;
  curr_state->quant_mode = config->bs_quant_mode;
  curr_state->one_icc = config->bs_one_icc;
  curr_state->arbitrary_downmix = config->bs_arbitrary_downmix;
  curr_state->residual_coding = config->bs_residual_coding;
  curr_state->smooth_config = config->bs_smooth_config;
  curr_state->mtx_inversion = config->bs_matrix_mode;
  curr_state->temp_shape_config = config->bs_temp_shape_config;
  curr_state->decorr_config = config->bs_decorr_config;
  curr_state->env_quant_mode = config->bs_env_quant_mode;
  curr_state->lfe_gain = bitdec_table->lfe_gain_table[config->bs_fixed_gain_lfe];
  curr_state->surround_gain = bitdec_table->surround_gain_table[config->bs_fixed_gain_sur];
  curr_state->clip_protect_gain = bitdec_table->clip_gain_table[config->bs_fixed_gain_dmx];

  if (curr_state->up_mix_type == 2) {
    curr_state->num_output_channels = 2;
    curr_state->decorr_config = 0;
  }

  if (curr_state->up_mix_type == 3) {
    curr_state->num_output_channels = 2;
  }

  if (p_bs_config->arbitrary_tree == 1)
    curr_state->num_output_channels_at = p_bs_config->num_out_chan_at;
  else
    curr_state->num_output_channels_at = curr_state->num_output_channels;

  p_bs_config->ui_out_channels = curr_state->num_output_channels_at;

  curr_state->_3d_stereo_inversion = config->bs_3d_audio_mode;

  if (curr_state->mtx_inversion == 1 || curr_state->_3d_stereo_inversion == 1)
    curr_state->m1_param_imag_present = 1;

  for (i = 0; i < curr_state->num_ott_boxes; i++) {
    if (bitdec_table->tree_property_table[curr_state->tree_config].ott_mode_lfe[i]) {
      b_ott_bands[i] = config->bs_ott_bands[i];
      curr_state->ott_mode_lfe[i] = 1;
    } else {
      b_ott_bands[i] = curr_state->bitstream_parameter_bands;
      curr_state->ott_mode_lfe[i] = 0;
    }

    if (curr_state->up_mix_type == 2) {
      ixheaacd_map_number_of_bands_to_28_bands(b_ott_bands[i],
                                               curr_state->bitstream_parameter_bands,
                                               &p_aux_struct->num_ott_bands[i], bitdec_table);
    } else {
      p_aux_struct->num_ott_bands[i] = b_ott_bands[i];
    }
  }
  for (i = 0; i < curr_state->num_ttt_boxes; i++) {
    p_aux_struct->ttt_config[0][i].mode = config->bs_ttt_mode_low[i];
    p_aux_struct->ttt_config[1][i].mode = config->bs_ttt_mode_high[i];
    p_aux_struct->ttt_config[0][i].bitstream_start_band = 0;
    p_aux_struct->ttt_config[1][i].bitstream_stop_band = curr_state->bitstream_parameter_bands;

    if (config->bs_ttt_dual_mode[i]) {
      p_aux_struct->ttt_config[0][i].bitstream_stop_band = config->bs_ttt_bands_low[i];
      p_aux_struct->ttt_config[1][i].bitstream_start_band = config->bs_ttt_bands_low[i];
    } else {
      p_aux_struct->ttt_config[0][i].bitstream_stop_band = curr_state->bitstream_parameter_bands;
      p_aux_struct->ttt_config[1][i].bitstream_start_band = curr_state->bitstream_parameter_bands;
    }

    if (curr_state->up_mix_type == 2) {
      ixheaacd_map_number_of_bands_to_28_bands(
          p_aux_struct->ttt_config[0][i].bitstream_start_band,
          curr_state->bitstream_parameter_bands, &p_aux_struct->ttt_config[0][i].start_band,
          bitdec_table);

      ixheaacd_map_number_of_bands_to_28_bands(p_aux_struct->ttt_config[0][i].bitstream_stop_band,
                                               curr_state->bitstream_parameter_bands,
                                               &p_aux_struct->ttt_config[0][i].stop_band,
                                               bitdec_table);

      ixheaacd_map_number_of_bands_to_28_bands(
          p_aux_struct->ttt_config[1][i].bitstream_start_band,
          curr_state->bitstream_parameter_bands, &p_aux_struct->ttt_config[1][i].start_band,
          bitdec_table);

      ixheaacd_map_number_of_bands_to_28_bands(p_aux_struct->ttt_config[1][i].bitstream_stop_band,
                                               curr_state->bitstream_parameter_bands,
                                               &p_aux_struct->ttt_config[1][i].stop_band,
                                               bitdec_table);
    } else {
      p_aux_struct->ttt_config[0][i].start_band =
          p_aux_struct->ttt_config[0][i].bitstream_start_band;
      p_aux_struct->ttt_config[0][i].stop_band =
          p_aux_struct->ttt_config[0][i].bitstream_stop_band;
      p_aux_struct->ttt_config[1][i].start_band =
          p_aux_struct->ttt_config[1][i].bitstream_start_band;
      p_aux_struct->ttt_config[1][i].stop_band =
          p_aux_struct->ttt_config[1][i].bitstream_stop_band;
    }
  }
  curr_state->residual_coding = config->bs_residual_coding;
  curr_state->num_residual_signals = 0;
  if (curr_state->residual_coding) {
    for (i = 0; i < curr_state->num_ttt_boxes + curr_state->num_ott_boxes; i++) {
      if (config->bs_residual_present[i]) {
        curr_state->res_bands[i] = config->bs_residual_bands[i];
        curr_state->num_residual_signals++;
      } else {
        curr_state->res_bands[i] = 0;
      }

      if (curr_state->up_mix_type == 2 || curr_state->up_mix_type == 3) {
        curr_state->res_bands[i] = 0;
      }
    }
  }

  curr_state->residual_frames_per_spatial_frame =
      p_bs_config->bs_residual_frames_per_spatial_frame + 1;
  if (curr_state->residual_frames_per_spatial_frame > 0) {
    WORD32 const *reciprocal_tab =
        pstr_mps_state->ia_mps_dec_mps_table.m1_m2_table_ptr->reciprocal;

    WORD64 temp =
        (WORD64)(((WORD64)(p_bs_config->bs_frame_length + 1) *
                  (WORD64)reciprocal_tab[p_bs_config->bs_residual_frames_per_spatial_frame]) >>
                 28);
    curr_state->upd_qmf = (WORD32)temp;
    if (curr_state->upd_qmf != UPD_QMF_15 && curr_state->upd_qmf != UPD_QMF_16 &&
        curr_state->upd_qmf != UPD_QMF_32 && curr_state->upd_qmf != UPD_QMF_18 &&
        curr_state->upd_qmf != UPD_QMF_30 && curr_state->upd_qmf != UPD_QMF_24)
      return IA_XHEAAC_MPS_DEC_EXE_NONFATAL_INVALID_QMF_UPDATE;
  }

  curr_state->arbdmx_residual_bands = config->bs_arbitrary_downmix_residual_bands;
  curr_state->arbdmx_frames_per_spatial_frame =
      config->bs_arbitrary_downmix_residual_frames_per_spatial_frame + 1;
  if (curr_state->arbdmx_frames_per_spatial_frame > 0) {
    curr_state->arbdmx_upd_qmf =
        curr_state->time_slots / curr_state->arbdmx_frames_per_spatial_frame;
    if (curr_state->arbdmx_upd_qmf != UPD_QMF_15 && curr_state->arbdmx_upd_qmf != UPD_QMF_16 &&
        curr_state->arbdmx_upd_qmf != UPD_QMF_32 && curr_state->arbdmx_upd_qmf != UPD_QMF_18 &&
        curr_state->arbdmx_upd_qmf != UPD_QMF_30 && curr_state->arbdmx_upd_qmf != UPD_QMF_24)
      return IA_XHEAAC_MPS_DEC_EXE_NONFATAL_INVALID_QMF_UPDATE;

    if ((curr_state->arbdmx_upd_qmf * 1.5f) > (curr_state->upd_qmf * 2))
      return IA_XHEAAC_MPS_DEC_EXE_NONFATAL_INVALID_QMF_UPDATE;
  }

  curr_state->cpc_default = 10;
  curr_state->ttt_cld_1_default[0] = 15;
  curr_state->ttt_cld_2_default[0] = 0;
  curr_state->icc_default = 0;
  curr_state->arbdmx_gain_default = 0;

  if (curr_state->_3d_stereo_inversion) {
    if (config->bs_3d_audio_hrtf_set == 0) {
      return IA_XHEAAC_MPS_DEC_EXE_FATAL_INVALID_HRTF_SET;
    } else {
      return IA_XHEAAC_MPS_DEC_EXE_FATAL_INVALID_HRTF_SET;
    }
  }

  switch (curr_state->tree_config) {
    case TREE_5151:
      curr_state->num_direct_signals = 1;
      curr_state->num_decor_signals = 4;

      if (curr_state->up_mix_type == 2) {
        curr_state->num_decor_signals = 1;
      }

      if (curr_state->up_mix_type == 3) {
        curr_state->num_decor_signals = 3;
      }

      curr_state->num_x_channels = 1;
      if (curr_state->arbitrary_downmix == 2) {
        curr_state->num_x_channels += 1;
      }
      curr_state->num_v_channels = curr_state->num_direct_signals + curr_state->num_decor_signals;
      curr_state->num_w_channels = curr_state->num_v_channels;
      curr_state->w_start_residual_idx = 0;
      curr_state->ott_cld_default[0] = 15;
      curr_state->ott_cld_default[1] = 15;
      curr_state->ott_cld_default[2] = 0;
      curr_state->ott_cld_default[3] = 0;
      curr_state->ott_cld_default[4] = 15;
      break;
    case TREE_5152:
      curr_state->num_direct_signals = 1;
      curr_state->num_decor_signals = 4;

      if (curr_state->up_mix_type == 2) {
        curr_state->num_decor_signals = 1;
      }

      if (curr_state->up_mix_type == 3) {
        curr_state->num_decor_signals = 2;
      }

      curr_state->num_x_channels = 1;
      if (curr_state->arbitrary_downmix == 2) {
        curr_state->num_x_channels += 1;
      }
      curr_state->num_v_channels = curr_state->num_direct_signals + curr_state->num_decor_signals;
      curr_state->num_w_channels = curr_state->num_v_channels;
      curr_state->w_start_residual_idx = 0;
      curr_state->ott_cld_default[0] = 15;
      curr_state->ott_cld_default[1] = 0;
      curr_state->ott_cld_default[2] = 15;
      curr_state->ott_cld_default[3] = 15;
      curr_state->ott_cld_default[4] = 15;
      break;
    case TREE_525:
      curr_state->num_direct_signals = 3;

      for (i = 0; i < 2; i++) {
        switch (p_aux_struct->ttt_config[i][0].mode) {
          case TTT_MODE_0:
            p_aux_struct->ttt_config[i][0].use_ttt_decorr = 1;
            curr_state->num_decor_signals = 3;
            break;
          case TTT_MODE_1:
          case TTT_MODE_2:
          case TTT_MODE_3:
          case TTT_MODE_4:
          case TTT_MODE_5:
            p_aux_struct->ttt_config[i][0].use_ttt_decorr = 0;
            curr_state->num_decor_signals = 2;
            break;
          default:
            if (p_bs_config->bs_ttt_mode_low[0] <= 1)
              return IA_XHEAAC_MPS_DEC_EXE_FATAL_INVALID_TTT_MODE;
            break;
        }
      }

      if (curr_state->residual_coding == 1) {
        curr_state->num_x_channels = 3;
      } else {
        curr_state->num_x_channels = 2;
      }

      if (curr_state->arbitrary_downmix == 2) {
        curr_state->num_x_channels = 5;
      }

      if (curr_state->up_mix_type == 2) {
        curr_state->num_direct_signals = 2;
        curr_state->num_decor_signals = 0;
        curr_state->num_x_channels = 2;

        if (curr_state->arbitrary_downmix == 2) {
          curr_state->num_direct_signals = 4;
          curr_state->num_x_channels = 5;
        }
      }

      curr_state->num_v_channels = curr_state->num_direct_signals + curr_state->num_decor_signals;
      curr_state->num_w_channels = curr_state->num_v_channels;
      curr_state->w_start_residual_idx = 1;
      curr_state->ott_cld_default[0] = 15;
      curr_state->ott_cld_default[1] = 15;
      curr_state->ott_cld_default[2] = 15;
      break;
    case TREE_7271:
    case TREE_7272:
      curr_state->num_direct_signals = 3;

      for (i = 0; i < 2; i++) {
        switch (p_aux_struct->ttt_config[i][0].mode) {
          case TTT_MODE_0:
            p_aux_struct->ttt_config[i][0].use_ttt_decorr = 1;
            curr_state->num_decor_signals = 5;
            break;
          case TTT_MODE_1:
          case TTT_MODE_2:
          case TTT_MODE_3:
          case TTT_MODE_4:
          case TTT_MODE_5:
            p_aux_struct->ttt_config[i][0].use_ttt_decorr = 0;
            curr_state->num_decor_signals = 5;
            break;
          default:
            if (p_bs_config->bs_ttt_mode_low[0] <= 1)
              return IA_XHEAAC_MPS_DEC_EXE_FATAL_INVALID_TTT_MODE;
            break;
        }
      }

      if (curr_state->residual_coding == 1) {
        curr_state->num_x_channels = 3;
      } else {
        curr_state->num_x_channels = 2;
      }

      if (curr_state->arbitrary_downmix == 2) {
        curr_state->num_x_channels = 5;
      }

      if (curr_state->up_mix_type == 2) {
        curr_state->num_direct_signals = 2;
        curr_state->num_decor_signals = 0;
        curr_state->num_x_channels = 2;

        if (curr_state->arbitrary_downmix == 2) {
          curr_state->num_direct_signals = 4;
          curr_state->num_x_channels = 5;
        }
      }

      curr_state->num_v_channels = curr_state->num_direct_signals + curr_state->num_decor_signals;
      curr_state->num_w_channels = curr_state->num_v_channels;
      curr_state->w_start_residual_idx = 1;
      curr_state->ott_cld_default[0] = 15;
      curr_state->ott_cld_default[1] = 15;
      curr_state->ott_cld_default[2] = 15;
      curr_state->ott_cld_default[3] = 15;
      curr_state->ott_cld_default[4] = 15;
      break;
    case TREE_7571:
    case TREE_7572:
      curr_state->num_direct_signals = 6;
      curr_state->num_decor_signals = 2;
      curr_state->num_x_channels = 6;
      curr_state->num_v_channels = curr_state->num_direct_signals + curr_state->num_decor_signals;
      curr_state->num_w_channels = curr_state->num_v_channels;
      curr_state->w_start_residual_idx = 0;
      curr_state->ott_cld_default[0] = 15;
      curr_state->ott_cld_default[1] = 15;
      break;
    default:
      return IA_XHEAAC_MPS_DEC_EXE_FATAL_UNSUPPRORTED_TREE_CONFIG;
      break;
  }
  return IA_NO_ERROR;
}

VOID ixheaacd_get_dequant_tables(
    WORD32 **cld, WORD32 **icc, WORD32 **cpc,
    ia_mps_dec_bitdec_tables_struct *ixheaacd_mps_dec_bitdec_tables) {
  *cld = ixheaacd_mps_dec_bitdec_tables->dequant_cld;
  *icc = ixheaacd_mps_dec_bitdec_tables->dequant_icc;
  *cpc = ixheaacd_mps_dec_bitdec_tables->dequant_cpc;
}

WORD32 ixheaacd_quantize_cld(WORD32 v,
                             ia_mps_dec_bitdec_tables_struct *ixheaacd_mps_dec_bitdec_tables) {
  WORD32 i = 1;
  WORD32 temp_1;
  WORD32 vmin = ixheaacd_mps_dec_bitdec_tables->dequant_cld[0];
  WORD32 dmin = abs(v - vmin);

  do {
    temp_1 = abs(v - ixheaacd_mps_dec_bitdec_tables->dequant_cld[i]);
    if (temp_1 < dmin) {
      dmin = temp_1;
      vmin = ixheaacd_mps_dec_bitdec_tables->dequant_cld[i];
    }
  } while (ixheaacd_mps_dec_bitdec_tables->dequant_cld[i++] < ONE_FORTYNINE_Q15);
  return vmin;
}

WORD32 ixheaacd_quantize_icc(WORD32 v,
                             ia_mps_dec_bitdec_tables_struct *ixheaacd_mps_dec_bitdec_tables) {
  WORD32 i = 1;
  WORD32 temp_1;
  WORD32 vmin = ixheaacd_mps_dec_bitdec_tables->dequant_icc[0];
  WORD32 dmin = abs(v - vmin);

  do {
    temp_1 = abs(v - ixheaacd_mps_dec_bitdec_tables->dequant_icc[i]);
    if (temp_1 < dmin) {
      dmin = temp_1;
      vmin = ixheaacd_mps_dec_bitdec_tables->dequant_icc[i];
    }
  } while (ixheaacd_mps_dec_bitdec_tables->dequant_icc[i++] > MINUS_POINT_NINE_EIGHT_Q15);

  return vmin;
}
