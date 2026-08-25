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
#include <stdio.h>
#include <string.h>
#include <math.h>
#include "impd_type_def.h"
#include "impd_drc_bitbuffer.h"
#include "impd_drc_extr_delta_coded_info.h"
#include "impd_drc_common.h"
#include "impd_drc_struct.h"
#include "impd_drc_parser.h"
#include "impd_drc_filter_bank.h"
#include "impd_drc_rom.h"
#ifdef LOUDNESS_LEVELING_SUPPORT
#include "impd_drc_error_codes.h"
#include "ixheaacd_error_codes.h"
#endif
WORD32 impd_parse_loud_eq_instructions(
    ia_bit_buf_struct* it_bit_buff,
    ia_loud_eq_instructions_struct* loud_eq_instructions);

WORD32 impd_parse_eq_coefficients(ia_bit_buf_struct* it_bit_buff,
                                  ia_eq_coeff_struct* str_eq_coeff);

WORD32 impd_parse_eq_instructions(
    ia_bit_buf_struct* it_bit_buff, ia_drc_config* drc_config,
    ia_eq_instructions_struct* str_eq_instructions);
WORD32 impd_dec_initial_gain(ia_bit_buf_struct* it_bit_buff,
                             const WORD32 gain_coding_profile,
                             FLOAT32* initial_gain) {
  WORD32 sign, magn, bit_2_extract;
  switch (gain_coding_profile) {
    case GAIN_CODING_PROFILE_REGULAR:
      sign = impd_read_bits_buf(it_bit_buff, 1);
      if (it_bit_buff->error) return it_bit_buff->error;
      magn = impd_read_bits_buf(it_bit_buff, 8);
      if (it_bit_buff->error) return it_bit_buff->error;
      *initial_gain = magn * 0.125f;
      if (sign) *initial_gain = -*initial_gain;
      break;
    case GAIN_CODING_PROFILE_FADING:
    case GAIN_CODING_PROFILE_CLIPPING:
      bit_2_extract =
          (gain_coding_profile == GAIN_CODING_PROFILE_FADING) ? 10 : 8;
      sign = impd_read_bits_buf(it_bit_buff, 1);
      if (it_bit_buff->error) return it_bit_buff->error;
      if (sign == 0)
        *initial_gain = 0.0f;
      else {
        magn = impd_read_bits_buf(it_bit_buff, bit_2_extract);
        if (it_bit_buff->error) return it_bit_buff->error;
        *initial_gain = -(magn + 1) * 0.125f;
      }
      break;

    case GAIN_CODING_PROFILE_CONSTANT:
      break;
    default:
      return (UNEXPECTED_ERROR);
  }
  return (0);
}

WORD32 impd_dec_gains(ia_bit_buf_struct* it_bit_buff, WORD32 no_nodes,
                      WORD32 gain_coding_profile, ia_node_struct* str_node) {
  WORD32 err = 0, k, e, m;
  WORD32 bit;
  WORD32 num_bits_read;
  WORD32 code;
  WORD32 code_found;
  FLOAT32 drc_gain_delta = 0;
  const ia_delta_gain_code_table_struct* ptr_delta_gain_code_table;
  WORD32 no_delta_gain_entries;

  err = impd_dec_initial_gain(it_bit_buff, gain_coding_profile,
                              &(str_node[0].loc_db_gain));
  if (err) return (err);

  impd_get_delta_gain_code_tbl(gain_coding_profile, &ptr_delta_gain_code_table,
                               &no_delta_gain_entries);
  for (k = 1; k < no_nodes; k++) {
    num_bits_read = 0;
    code = 0;
    code_found = 0;
    e = 0;
    while ((e < no_delta_gain_entries) && (!code_found)) {
      for (m = 0; m < ptr_delta_gain_code_table[e].size - num_bits_read; m++) {
        bit = impd_read_bits_buf(it_bit_buff, 1);
        if (it_bit_buff->error) return it_bit_buff->error;
        code = (code << 1) + bit;
        num_bits_read++;
      }
      while (num_bits_read == ptr_delta_gain_code_table[e].size) {
        if (code == ptr_delta_gain_code_table[e].code) {
          drc_gain_delta = ptr_delta_gain_code_table[e].value;
          code_found = 1;
          break;
        }
        e++;
      }
    }
    if (code_found == 0) {
      return (UNEXPECTED_ERROR);
    }
    str_node[k].loc_db_gain = str_node[k - 1].loc_db_gain + drc_gain_delta;
  }
  return (0);
}

WORD32 impd_dec_slopes(ia_bit_buf_struct* it_bit_buff, WORD32* no_nodes,
                       WORD32 gain_interpolation_type,
                       ia_node_struct* str_node) {
  WORD32 k, e, bit;
  WORD32 code;
  WORD32 code_found;
  FLOAT32 slope_value = 0;
  bool end_marker = 0;
  WORD32 num_bits_read;
  const ia_slope_code_table_struct* ptr_slope_code_table;
  WORD32 no_slope_code_entries;

  ptr_slope_code_table = &(slope_code_tbl_entries_by_size[0]);
  no_slope_code_entries = NUM_SLOPE_TBL_ENTRIES;

  k = 0;
  while (end_marker != 1) {
    k++;
    end_marker = impd_read_bits_buf(it_bit_buff, 1);
    if (it_bit_buff->error) return it_bit_buff->error;
  }
  if (k > NODE_COUNT_MAX) return UNEXPECTED_ERROR;
  *no_nodes = k;

  if (gain_interpolation_type == GAIN_INTERPOLATION_TYPE_SPLINE) {
    for (k = 0; k < *no_nodes; k++) {
      num_bits_read = 0;
      code = 0;
      code_found = 0;
      e = 0;
      while ((e < no_slope_code_entries) && (!code_found)) {
        while (num_bits_read < ptr_slope_code_table[e].size) {
          bit = impd_read_bits_buf(it_bit_buff, 1);
          if (it_bit_buff->error) return it_bit_buff->error;
          code = (code << 1) + bit;
          num_bits_read++;
        }
        while (num_bits_read == ptr_slope_code_table[e].size) {
          if (code == ptr_slope_code_table[e].code) {
            slope_value = ptr_slope_code_table[e].value;
            code_found = 1;
            break;
          }
          e++;
          if (e >= no_slope_code_entries) return UNEXPECTED_ERROR;
        }
      }
      str_node[k].slope = slope_value;
    }
  } else {
    for (k = 0; k < *no_nodes; k++) {
      str_node[k].slope = 0.0f;
    }
  }
  return (0);
}

WORD32 impd_dec_times(ia_bit_buf_struct* it_bit_buff,
                      ia_tables_struct* str_tables, WORD32 num_nodes,
                      WORD32 delta_tmin, WORD32 drc_frame_size,
                      WORD32 full_frame, WORD32 time_offset,
                      ia_node_struct* str_node) {
  WORD32 k, e, m;
  WORD32 bit;
  WORD32 num_bits_read;
  WORD32 code;
  WORD32 code_found = 0;
  WORD32 time_delta = 0;
  WORD32 time_offs = time_offset;
  ia_delta_time_code_table_entry_struct* delta_time_code_table =
      str_tables->delta_time_code_table;
  bool frame_end_flag;
  WORD32 node_time_tmp;
  bool node_res_flag;
  WORD32 exit_cnt;
  if (full_frame == 0) {
    frame_end_flag = impd_read_bits_buf(it_bit_buff, 1);
    if (it_bit_buff->error) return it_bit_buff->error;
  } else {
    frame_end_flag = 1;
  }

  if (frame_end_flag == 1) {
    node_res_flag = 0;
    for (k = 0; k < num_nodes - 1; k++) {
      num_bits_read = 0;
      code = 0;
      code_found = 0;
      exit_cnt = 0;
      e = 1;
      while ((e < N_DELTA_TIME_CODE_TABLE_ENTRIES_MAX) && (!code_found)) {
        exit_cnt++;
        if (exit_cnt > 100000) {
          return -1;
        }
        for (m = 0; m < delta_time_code_table[e].size - num_bits_read; m++) {
          bit = impd_read_bits_buf(it_bit_buff, 1);
          if (it_bit_buff->error) return it_bit_buff->error;
          code = (code << 1) + bit;
          num_bits_read++;
        }
        while (num_bits_read == delta_time_code_table[e].size) {
          if (code == delta_time_code_table[e].code) {
            time_delta = delta_time_code_table[e].value;
            code_found = 1;
            break;
          }
          e++;
        }
      }
      node_time_tmp = time_offs + time_delta * delta_tmin;
      if (node_time_tmp >= (2 * AUDIO_CODEC_FRAME_SIZE_MAX - drc_frame_size))
        return UNEXPECTED_ERROR;
      if (node_time_tmp > drc_frame_size + time_offset) {
        if (node_res_flag == 0) {
          str_node[k].time = drc_frame_size + time_offset;
          node_res_flag = 1;
        }
        str_node[k + 1].time = node_time_tmp;
      } else {
        str_node[k].time = node_time_tmp;
      }
      time_offs = node_time_tmp;
    }
    if (node_res_flag == 0) {
      str_node[k].time = drc_frame_size + time_offset;
    }
  } else {
    for (k = 0; k < num_nodes; k++) {
      num_bits_read = 0;
      code = 0;
      code_found = 0;
      e = 1;
      exit_cnt = 0;
      while ((e < N_DELTA_TIME_CODE_TABLE_ENTRIES_MAX) && (!code_found)) {
        exit_cnt++;
        if (exit_cnt > 100000) {
          return (BITSTREAM_ERROR);
        }
        for (m = 0; m < delta_time_code_table[e].size - num_bits_read; m++) {
          bit = impd_read_bits_buf(it_bit_buff, 1);
          if (it_bit_buff->error) return it_bit_buff->error;
          code = (code << 1) + bit;
          num_bits_read++;
        }
        while (num_bits_read == delta_time_code_table[e].size) {
          if (code == delta_time_code_table[e].code) {
            time_delta = delta_time_code_table[e].value;
            code_found = 1;
            break;
          }
          e++;
        }
      }
      time_offs += time_delta * delta_tmin;
      if (time_offs >= (2 * AUDIO_CODEC_FRAME_SIZE_MAX - drc_frame_size))
        return UNEXPECTED_ERROR;
      str_node[k].time = time_offs;
    }
  }
  return (0);
}

WORD32 impd_drc_uni_gain_read(ia_bit_buf_struct* it_bit_buff,
                              ia_drc_bits_dec_struct* pstr_drc_uni_bs_dec,
                              ia_drc_config* drc_config,
                              ia_drc_gain_struct* pstr_uni_drc_gain) {
  WORD32 err = 0;
  WORD32 seq;
  static WORD32 pkt_loss_frame_cnt = 0;
  ia_spline_nodes_struct* str_spline_nodes = {0};

  {
    WORD32 gain_sequence_count =
        drc_config->str_p_loc_drc_coefficients_uni_drc[0].gain_sequence_count;

    for (seq = 0; seq < gain_sequence_count; seq++) {
      WORD32 index = drc_config->str_p_loc_drc_coefficients_uni_drc[0]
                         .gain_set_params_index_for_gain_sequence[seq];
      ia_gain_set_params_struct str_gain_set_params = {0};
      ia_gain_set_params_struct* gain_set_params = &str_gain_set_params;

      if (index != -1) {
        gain_set_params = &(drc_config->str_p_loc_drc_coefficients_uni_drc
                                ->gain_set_params[index]);
      }
      if (gain_set_params->gain_coding_profile ==
          GAIN_CODING_PROFILE_CONSTANT) {
        str_spline_nodes =
            &(pstr_uni_drc_gain->drc_gain_sequence[seq].str_spline_nodes[0]);
        str_spline_nodes->num_nodes = 1;
        str_spline_nodes->str_node[0].slope = 0.0;
        str_spline_nodes->str_node[0].time =
            (pstr_drc_uni_bs_dec->ia_drc_params_struct).drc_frame_size - 1;
        str_spline_nodes->str_node[0].loc_db_gain = 0.0f;
      } else {
        err = impd_parse_drc_gain_sequence(
            it_bit_buff, pstr_drc_uni_bs_dec, gain_set_params,
            &(pstr_uni_drc_gain->drc_gain_sequence[seq]));
        if (err) return (err);
      }
    }
  }

  if (it_bit_buff->ptr_bit_buf_base == NULL) {
    pkt_loss_frame_cnt++;

    if (pkt_loss_frame_cnt *
            (FLOAT32)pstr_drc_uni_bs_dec->ia_drc_params_struct.drc_frame_size /
            drc_config->sampling_rate >
        MAXPACKETLOSSTIME) {
      drc_config->apply_drc = 0;
    }
  } else {
    pstr_uni_drc_gain->uni_drc_gain_ext_flag =
        impd_read_bits_buf(it_bit_buff, 1);
    if (it_bit_buff->error) return it_bit_buff->error;
    if (pstr_uni_drc_gain->uni_drc_gain_ext_flag == 1) {
      err = impd_parse_uni_drc_gain_ext(it_bit_buff,
                                        &(pstr_uni_drc_gain->uni_drc_gain_ext));
      if (err) return (err);
    }
    pkt_loss_frame_cnt = 0;
    drc_config->apply_drc = 1;
  }

  return (0);
}

WORD32 impd_parse_uni_drc_gain_ext(
    ia_bit_buf_struct* it_bit_buff,
    ia_uni_drc_gain_ext_struct* uni_drc_gain_ext) {
  WORD32 k;
  WORD32 bit_size_len, ext_size_bits, bit_size;

  k = 0;
  uni_drc_gain_ext->uni_drc_gain_ext_type[k] =
      impd_read_bits_buf(it_bit_buff, 4);
  if (it_bit_buff->error) return it_bit_buff->error;
  while (uni_drc_gain_ext->uni_drc_gain_ext_type[k] != UNIDRCGAINEXT_TERM) {
    if (k >= (EXT_COUNT_MAX - 1)) return UNEXPECTED_ERROR;
    bit_size_len = impd_read_bits_buf(it_bit_buff, 3);
    if (it_bit_buff->error) return it_bit_buff->error;
    ext_size_bits = bit_size_len + 4;

    bit_size = impd_read_bits_buf(it_bit_buff, ext_size_bits);
    if (it_bit_buff->error) return it_bit_buff->error;
    uni_drc_gain_ext->ext_bit_size[k] = bit_size + 1;

        impd_skip_bits_buf(it_bit_buff, uni_drc_gain_ext->ext_bit_size[k]);
    if (it_bit_buff->error) return it_bit_buff->error;
    k++;
    uni_drc_gain_ext->uni_drc_gain_ext_type[k] =
        impd_read_bits_buf(it_bit_buff, 4);
    if (it_bit_buff->error) return it_bit_buff->error;
  }

  return (0);
}

WORD32 impd_parse_spline_nodes(ia_bit_buf_struct* it_bit_buff,
                               ia_drc_bits_dec_struct* pstr_drc_uni_bs_dec,
                               ia_gain_set_params_struct* gain_set_params,
                               ia_spline_nodes_struct* str_spline_nodes) {
  WORD32 err = 0;
  WORD32 time_offset;
  if (gain_set_params->time_alignment == 0) {
    time_offset = -1;
  } else {
    if (gain_set_params->time_delt_min_flag) {
      time_offset = -gain_set_params->time_delt_min_val +
                    (gain_set_params->time_delt_min_val - 1) / 2;
    } else {
      time_offset =
          -pstr_drc_uni_bs_dec->ia_drc_params_struct.delta_tmin_default +
          (pstr_drc_uni_bs_dec->ia_drc_params_struct.delta_tmin_default - 1) /
              2;
    }
  }

  if (it_bit_buff->ptr_bit_buf_base == NULL) {
    if ((str_spline_nodes->num_nodes < 1) ||
        (str_spline_nodes->num_nodes > NODE_COUNT_MAX))
      return UNEXPECTED_ERROR;
    FLOAT32 prev_db_gain =
        str_spline_nodes->str_node[str_spline_nodes->num_nodes - 1].loc_db_gain;
    str_spline_nodes->drc_gain_coding_mode = 0;

    str_spline_nodes->num_nodes = 1;

    if (prev_db_gain < 0) {
      str_spline_nodes->str_node[0].loc_db_gain = prev_db_gain;
    } else {
      str_spline_nodes->str_node[0].loc_db_gain = 0.f;
    }

    str_spline_nodes->str_node[0].slope = 0.0;
    str_spline_nodes->str_node[0].time =
        (pstr_drc_uni_bs_dec->ia_drc_params_struct).drc_frame_size +
        time_offset;
  } else {
    str_spline_nodes->drc_gain_coding_mode = impd_read_bits_buf(it_bit_buff, 1);
    if (it_bit_buff->error == PROC_COMPLETE) {
      if ((str_spline_nodes->num_nodes < 1) ||
          (str_spline_nodes->num_nodes > NODE_COUNT_MAX))
        return UNEXPECTED_ERROR;
      str_spline_nodes->drc_gain_coding_mode = 0;
      str_spline_nodes->str_node[0].slope = 0.0;
      str_spline_nodes->str_node[0].time =
          (pstr_drc_uni_bs_dec->ia_drc_params_struct).drc_frame_size +
          time_offset;
      str_spline_nodes->str_node[0].loc_db_gain =
          str_spline_nodes->str_node[str_spline_nodes->num_nodes - 1]
              .loc_db_gain;
      str_spline_nodes->num_nodes = 1;
    } else {
      if (it_bit_buff->error) return (it_bit_buff->error);
    }
    if (str_spline_nodes->drc_gain_coding_mode == 0) {
      str_spline_nodes->num_nodes = 1;

      err = impd_dec_initial_gain(it_bit_buff,
                                  gain_set_params->gain_coding_profile,
                                  &(str_spline_nodes->str_node[0].loc_db_gain));
      if (err) return (err);

      str_spline_nodes->str_node[0].slope = 0.0;
      str_spline_nodes->str_node[0].time =
          (pstr_drc_uni_bs_dec->ia_drc_params_struct).drc_frame_size +
          time_offset;
    } else {
      err = impd_dec_slopes(it_bit_buff, &str_spline_nodes->num_nodes,
                            gain_set_params->gain_interpolation_type,
                            str_spline_nodes->str_node);
      if (err) return (err);
      if (gain_set_params->time_delt_min_flag) {
        err = impd_dec_times(
            it_bit_buff, &gain_set_params->str_tables,
            str_spline_nodes->num_nodes, gain_set_params->time_delt_min_val,
            (pstr_drc_uni_bs_dec->ia_drc_params_struct).drc_frame_size,
            gain_set_params->full_frame, time_offset,
            str_spline_nodes->str_node);
        if (err) return (err);
        err = impd_dec_gains(it_bit_buff, str_spline_nodes->num_nodes,
                             gain_set_params->gain_coding_profile,
                             str_spline_nodes->str_node);
        if (err) return (err);
      } else {
        err = impd_dec_times(
            it_bit_buff, &pstr_drc_uni_bs_dec->tables_default,
            str_spline_nodes->num_nodes,
            (pstr_drc_uni_bs_dec->ia_drc_params_struct).delta_tmin_default,
            (pstr_drc_uni_bs_dec->ia_drc_params_struct).drc_frame_size,
            gain_set_params->full_frame, time_offset,
            str_spline_nodes->str_node);
        if (err) return (err);
        err = impd_dec_gains(it_bit_buff, str_spline_nodes->num_nodes,
                             gain_set_params->gain_coding_profile,
                             str_spline_nodes->str_node);
        if (err) return (err);
      }
    }
  }
  return (0);
}

WORD32 impd_parse_drc_gain_sequence(
    ia_bit_buf_struct* it_bit_buff, ia_drc_bits_dec_struct* pstr_drc_uni_bs_dec,
    ia_gain_set_params_struct* gain_set_params,
    ia_drc_gain_sequence_struct* drc_gain_sequence) {
  WORD32 err = 0, i;
  WORD32 prev_frame_time_buf[NODE_COUNT_MAX],
      cur_frame_time_buf[NODE_COUNT_MAX];
  WORD32 num_nodes_node_reservoir, num_nodes_cur, k, m;

  if (((pstr_drc_uni_bs_dec->ia_drc_params_struct).delay_mode ==
       DELAY_MODE_LOW_DELAY) &&
      (gain_set_params->full_frame == 0)) {
    return (PARAM_ERROR);
  }
  i = 0;
  {
    err = impd_parse_spline_nodes(it_bit_buff, pstr_drc_uni_bs_dec,
                                  gain_set_params,
                                  &(drc_gain_sequence->str_spline_nodes[i]));
    if (err) return (err);

    num_nodes_node_reservoir = 0;
    num_nodes_cur = 0;
    for (k = 0; k < drc_gain_sequence->str_spline_nodes[i].num_nodes; k++) {
      if (drc_gain_sequence->str_spline_nodes[i].str_node[k].time >=
          pstr_drc_uni_bs_dec->ia_drc_params_struct.drc_frame_size) {
        prev_frame_time_buf[num_nodes_node_reservoir] =
            drc_gain_sequence->str_spline_nodes[i].str_node[k].time;
        num_nodes_node_reservoir++;
      } else {
        cur_frame_time_buf[num_nodes_cur] =
            drc_gain_sequence->str_spline_nodes[i].str_node[k].time;
        num_nodes_cur++;
      }
    }
    for (k = 0; k < num_nodes_node_reservoir; k++) {
      WORD32 tmp = prev_frame_time_buf[k] -
                   2 * pstr_drc_uni_bs_dec->ia_drc_params_struct.drc_frame_size;
      if (tmp >= (2 * AUDIO_CODEC_FRAME_SIZE_MAX -
                  pstr_drc_uni_bs_dec->ia_drc_params_struct.drc_frame_size))
        return UNEXPECTED_ERROR;
      drc_gain_sequence->str_spline_nodes[i].str_node[k].time = tmp;
    }
    for (m = 0; m < num_nodes_cur; m++, k++) {
      drc_gain_sequence->str_spline_nodes[i].str_node[k].time =
          cur_frame_time_buf[m];
    }
  }
  return (0);
}
WORD32 impd_parse_drc_ext_v1(ia_bit_buf_struct* it_bit_buff,
                             ia_drc_params_bs_dec_struct* ia_drc_params_struct,
                             ia_drc_config* drc_config,
                             ia_drc_config_ext* str_drc_config_ext) {
  WORD32 dwnmix_instructions_v1_flag;
  WORD32 dwnmix_instructions_v1_count;
  WORD32 drc_coeffs_and_instructions_uni_drc_v1_flag;
  WORD32 drc_coefficients_uni_drc_v1_count;
  WORD32 drc_instructions_uni_drc_v1_count;

  WORD32 i = 0, err = 0;
  const WORD32 version = 1;

  dwnmix_instructions_v1_flag = impd_read_bits_buf(it_bit_buff, 1);
  if (it_bit_buff->error) return it_bit_buff->error;
  if (dwnmix_instructions_v1_flag == 1) {
    dwnmix_instructions_v1_count = impd_read_bits_buf(it_bit_buff, 7);
    if (it_bit_buff->error) return it_bit_buff->error;
    if ((dwnmix_instructions_v1_count + drc_config->dwnmix_instructions_count) >
        DOWNMIX_INSTRUCTION_COUNT_MAX)
      return UNEXPECTED_ERROR;
    for (i = 0; i < dwnmix_instructions_v1_count; i++) {
      err = impd_parse_dwnmix_instructions(
          it_bit_buff, version, ia_drc_params_struct,
          &drc_config->channel_layout,
          &drc_config
               ->dwnmix_instructions[i +
                                     drc_config->dwnmix_instructions_count]);
      if (err) return (err);
    }
    drc_config->dwnmix_instructions_count += dwnmix_instructions_v1_count;
  }

  drc_coeffs_and_instructions_uni_drc_v1_flag =
      impd_read_bits_buf(it_bit_buff, 1);
  if (it_bit_buff->error) return it_bit_buff->error;
  if (drc_coeffs_and_instructions_uni_drc_v1_flag == 1) {
    drc_coefficients_uni_drc_v1_count = impd_read_bits_buf(it_bit_buff, 3);
    if ((drc_coefficients_uni_drc_v1_count +
         drc_config->drc_coefficients_drc_count) > DRC_COEFF_COUNT_MAX) {
      return (UNEXPECTED_ERROR);
    }
    if (it_bit_buff->error) return it_bit_buff->error;
    for (i = 0; i < drc_coefficients_uni_drc_v1_count; i++) {
      err = impd_drc_parse_coeff(
          it_bit_buff, version, ia_drc_params_struct,
          &drc_config->str_p_loc_drc_coefficients_uni_drc
               [i + drc_config->drc_coefficients_drc_count]);
      if (err) return (err);
    }
    drc_config->drc_coefficients_drc_count += drc_coefficients_uni_drc_v1_count;

    drc_instructions_uni_drc_v1_count = impd_read_bits_buf(it_bit_buff, 6);
    if (it_bit_buff->error) return it_bit_buff->error;
    if (drc_config->drc_instructions_uni_drc_count +
            drc_instructions_uni_drc_v1_count >
        DRC_INSTRUCTIONS_COUNT_MAX)
      return (UNEXPECTED_ERROR);

    for (i = 0; i < drc_instructions_uni_drc_v1_count; i++) {
      err = impd_parse_drc_instructions_uni_drc(
          it_bit_buff, version, drc_config,
          &drc_config->str_drc_instruction_str
               [i + drc_config->drc_instructions_uni_drc_count]);
      if (err) return (err);
    }
    drc_config->drc_instructions_uni_drc_count +=
        drc_instructions_uni_drc_v1_count;
  }

  str_drc_config_ext->loud_eq_instructions_flag =
      impd_read_bits_buf(it_bit_buff, 1);
  if (it_bit_buff->error) return it_bit_buff->error;
  if (str_drc_config_ext->loud_eq_instructions_flag == 1) {
    str_drc_config_ext->loud_eq_instructions_count =
        impd_read_bits_buf(it_bit_buff, 4);

    if (it_bit_buff->error) return it_bit_buff->error;
    if (str_drc_config_ext->loud_eq_instructions_count >
        LOUD_EQ_INSTRUCTIONS_COUNT_MAX)
      return UNEXPECTED_ERROR;
    for (i = 0; i < str_drc_config_ext->loud_eq_instructions_count; i++) {
      err = impd_parse_loud_eq_instructions(
          it_bit_buff, &str_drc_config_ext->loud_eq_instructions[i]);
      if (err) return (err);
    }
  } else {
    str_drc_config_ext->loud_eq_instructions_count = 0;
  }

  str_drc_config_ext->eq_flag = impd_read_bits_buf(it_bit_buff, 1);
  if (it_bit_buff->error) return it_bit_buff->error;
  if (str_drc_config_ext->eq_flag == 1) {
    err = impd_parse_eq_coefficients(it_bit_buff,
                                     &str_drc_config_ext->str_eq_coeff);
    if (err) return (err);
    str_drc_config_ext->eq_instructions_count =
        impd_read_bits_buf(it_bit_buff, 4);
    if (str_drc_config_ext->eq_instructions_count > EQ_INSTRUCTIONS_COUNT_MAX)
      return UNEXPECTED_ERROR;
    if (it_bit_buff->error) return it_bit_buff->error;
    for (i = 0; i < str_drc_config_ext->eq_instructions_count; i++) {
      err = impd_parse_eq_instructions(
          it_bit_buff, drc_config, &str_drc_config_ext->str_eq_instructions[i]);
      if (err) return (err);
    }
  }
  return 0;
}

#ifdef LOUDNESS_LEVELING_SUPPORT
IA_ERROR_CODE impd_leveling_instructions(ia_bit_buf_struct* it_bit_buff,
                                         ia_drc_config* pstr_drc_config) {
  WORD32 i;
  IA_ERROR_CODE err = IA_NO_ERROR;
  WORD32 drc_instruction_uni_drc_count = pstr_drc_config->drc_instructions_uni_drc_count;

  for (i = 0; i < drc_instruction_uni_drc_count; ++i) {
    if ((pstr_drc_config->str_drc_instruction_str[i].drc_set_effect & (1 << 11)) != 0) {
      pstr_drc_config->str_drc_instruction_str[i].leveling_present =
          impd_read_bits_buf(it_bit_buff, 1);

      if (pstr_drc_config->str_drc_instruction_str[i].leveling_present) {
        WORD32 ducking_only_drc_set_present = impd_read_bits_buf(it_bit_buff, 1);

        if (ducking_only_drc_set_present) {
          pstr_drc_config->drc_instructions_uni_drc_count++;

          if (pstr_drc_config->drc_instructions_uni_drc_count > DRC_INSTRUCTIONS_COUNT_MAX) {
            return IA_XHEAAC_DEC_INIT_NONFATAL_MAX_INSTRUCTIONS_ERROR;
          }

          err = impd_parse_drc_instructions_uni_drc(
              it_bit_buff, 1, pstr_drc_config,
              &pstr_drc_config
                   ->str_drc_instruction_str[pstr_drc_config->drc_instructions_uni_drc_count -
                                             1]);
          if (err) return err;
        }
      }
    }
  }
  return err;
}
#endif

WORD32 impd_parse_filt_block(ia_bit_buf_struct* it_bit_buff,
                             ia_filt_block_struct* str_filter_block,
                             WORD32 block_count) {
  //    WORD32 err = 0;
  WORD32 k, j, temp;
  ia_filt_ele_struct* str_filter_element;

  for (j = 0; j < block_count; j++) {
    str_filter_block->filter_element_count = impd_read_bits_buf(it_bit_buff, 6);
    if (it_bit_buff->error) return it_bit_buff->error;
    if (str_filter_block->filter_element_count > FILTER_ELEMENT_COUNT_MAX)
      return UNEXPECTED_ERROR;
    str_filter_element = &str_filter_block->str_filter_element[0];
    for (k = 0; k < str_filter_block->filter_element_count; k++) {
      temp = impd_read_bits_buf(it_bit_buff, 7);
      if (it_bit_buff->error) return it_bit_buff->error;

      str_filter_element->filt_ele_idx = (temp & 0x7E) >> 1;
      if (str_filter_element->filt_ele_idx >= FILTER_ELEMENT_COUNT_MAX)
        return (UNEXPECTED_ERROR);
      str_filter_element->filt_ele_gain_flag = temp & 1;
      ;

      if (str_filter_element->filt_ele_gain_flag) {
        WORD32 bs_filter_element_gain;
        bs_filter_element_gain = impd_read_bits_buf(it_bit_buff, 10);
        if (it_bit_buff->error) return it_bit_buff->error;
        str_filter_element->filt_ele_gain =
            bs_filter_element_gain * 0.125f - 96.0f;
      }

      str_filter_element++;
    }
    str_filter_block++;
  }
  return (0);
}

WORD32 impd_parse_unique_td_filt_ele(
    ia_bit_buf_struct* it_bit_buff,
    ia_unique_td_filt_element* unique_td_filt_ele,
    WORD32 td_filter_element_count) {
  WORD32 m, sign, j, temp;
  FLOAT32 tmp;

  for (j = 0; j < td_filter_element_count; j++) {
    unique_td_filt_ele->eq_filter_format = impd_read_bits_buf(it_bit_buff, 1);
    if (it_bit_buff->error) return it_bit_buff->error;

    if (unique_td_filt_ele->eq_filter_format == 0) {
      WORD32 bs_real_zero_radius, bs_generic_zero_radius, bs_generic_zero_angle;
      WORD32 bs_real_pole_radius, bs_cmplx_pole_radius, bs_cmplx_pole_angle;
      WORD32 bs_real_zero_radius_one_count;

      temp = impd_read_bits_buf(it_bit_buff, 23);
      if (it_bit_buff->error) return it_bit_buff->error;

      bs_real_zero_radius_one_count = (temp >> 20) & 7;

      unique_td_filt_ele->bs_real_zero_radius_one_count =
          2 * bs_real_zero_radius_one_count;
      unique_td_filt_ele->real_zero_count = (temp & 0xFC000) >> 14;

      unique_td_filt_ele->generic_zero_count = (temp & 0x3F00) >> 8;

      unique_td_filt_ele->real_pole_count = (temp & 0xF0) >> 4;

      unique_td_filt_ele->cmplx_pole_count = temp & 0xF;

      temp = impd_read_bits_buf(
          it_bit_buff, unique_td_filt_ele->bs_real_zero_radius_one_count);
      if (it_bit_buff->error) return it_bit_buff->error;

      for (m = unique_td_filt_ele->bs_real_zero_radius_one_count - 1; m >= 0;
           m--) {
        unique_td_filt_ele->zero_sign[m] = (temp & 1);
        temp = temp >> 1;
      }

      for (m = 0; m < unique_td_filt_ele->real_zero_count; m++) {
        temp = impd_read_bits_buf(it_bit_buff, 8);
        if (it_bit_buff->error) return it_bit_buff->error;

        bs_real_zero_radius = (temp & 0xFE) >> 1;

        sign = temp & 0x01;

        tmp = 1.0f - zero_pole_radius_tbl[bs_real_zero_radius];

        sign = sign << 1;

        unique_td_filt_ele->real_zero_radius[m] = (1 - sign) * tmp;
      }
      for (m = 0; m < unique_td_filt_ele->generic_zero_count; m++) {
        temp = impd_read_bits_buf(it_bit_buff, 14);
        if (it_bit_buff->error) return it_bit_buff->error;

        bs_generic_zero_radius = (temp & 0x3F80) >> 7;

        unique_td_filt_ele->generic_zero_radius[m] =
            1.0f - zero_pole_radius_tbl[bs_generic_zero_radius];

        bs_generic_zero_angle = (temp & 0x7F);

        unique_td_filt_ele->generic_zero_angle[m] =
            zero_pole_angle_tbl[bs_generic_zero_angle];
      }
      for (m = 0; m < unique_td_filt_ele->real_pole_count; m++) {
        temp = impd_read_bits_buf(it_bit_buff, 8);
        if (it_bit_buff->error) return it_bit_buff->error;

        bs_real_pole_radius = (temp & 0xFE) >> 1;

        sign = temp & 0x01;

        tmp = 1.0f - zero_pole_radius_tbl[bs_real_pole_radius];

        sign = sign << 1;

        unique_td_filt_ele->real_pole_radius[m] = (1 - sign) * tmp;
      }
      for (m = 0; m < unique_td_filt_ele->cmplx_pole_count; m++) {
        temp = impd_read_bits_buf(it_bit_buff, 14);

        if (it_bit_buff->error) return it_bit_buff->error;

        bs_cmplx_pole_radius = (temp & 0x3F80) >> 7;

        unique_td_filt_ele->complex_pole_radius[m] =
            1.0f - zero_pole_radius_tbl[bs_cmplx_pole_radius];

        bs_cmplx_pole_angle = (temp & 0x7F);

        unique_td_filt_ele->complex_pole_angle[m] =
            zero_pole_angle_tbl[bs_cmplx_pole_angle];
      }
    } else {
      temp = impd_read_bits_buf(it_bit_buff, 8);
      if (it_bit_buff->error) return it_bit_buff->error;

      unique_td_filt_ele->fir_filt_order = (temp & 0xFE) >> 1;

      unique_td_filt_ele->fir_symmetry = temp & 0x01;

      for (m = 0; m < unique_td_filt_ele->fir_filt_order / 2 + 1; m++) {
        WORD32 sign, bs_fir_coeff;
        FLOAT32 tmp;

        temp = impd_read_bits_buf(it_bit_buff, 11);
        if (it_bit_buff->error) return it_bit_buff->error;
        sign = (temp >> 10) & 0x01;

        bs_fir_coeff = temp & 0x03FF;

        tmp = (FLOAT32)pow(10.0f, -0.05f * bs_fir_coeff * 0.0625f);

        sign = sign << 1;

        unique_td_filt_ele->fir_coeff[m] = (1 - sign) * tmp;
      }
    }
    unique_td_filt_ele++;
  }
  return (0);
}

WORD32 impd_decode_eq_slope_code(ia_bit_buf_struct* it_bit_buff,
                                 FLOAT32* eq_slope, WORD32 num_eq_nodes) {
  WORD32 bits = 0;
  WORD32 k;

  for (k = 0; k < num_eq_nodes; k++) {
    bits = impd_read_bits_buf(it_bit_buff, 1);
    if (it_bit_buff->error) return it_bit_buff->error;
    if (bits == 0x1) {
      *eq_slope = 0.0f;
    } else {
      bits = impd_read_bits_buf(it_bit_buff, 4);
      if (it_bit_buff->error) return it_bit_buff->error;
      *eq_slope = eq_slope_tbl[bits];
    }
    eq_slope++;
  }
  return (0);
}

WORD32
impd_decode_gain_initial_code(ia_bit_buf_struct* it_bit_buff,
                              FLOAT32* eq_gain_initial) {
  WORD32 bits, bits1;

  bits1 = impd_read_bits_buf(it_bit_buff, 2);
  if (it_bit_buff->error) return it_bit_buff->error;

  switch (bits1) {
    case 0x0:
      bits = impd_read_bits_buf(it_bit_buff, 5);
      if (it_bit_buff->error) return it_bit_buff->error;
      *eq_gain_initial = 0.5f * bits - 8.0f;
      break;
    case 0x1:
    case 0x2:

      bits = impd_read_bits_buf(it_bit_buff, 4);
      if (it_bit_buff->error) return it_bit_buff->error;
      if (bits < 8) {
        *eq_gain_initial = bits1 * bits - bits1 * 16.0f;
      } else {
        *eq_gain_initial = (FLOAT32)bits1 * bits;
      }
      break;

    case 0x3:
      bits = impd_read_bits_buf(it_bit_buff, 3);
      if (it_bit_buff->error) return it_bit_buff->error;
      *eq_gain_initial = 4.0f * bits - 64.0f;
      break;

    default:
      break;
  }
  return (0);
}

WORD32 impd_parse_eq_subband_gain_spline(
    ia_bit_buf_struct* it_bit_buff,
    ia_eq_subband_gain_spline_struct* str_eq_subband_gain_spline,
    WORD32 eq_subband_gains_count) {
  WORD32 err = 0, eq_nodes_cnt, j, k, bits, *eq_freq_delta;
  FLOAT32* peq_gain_delta;

  for (j = 0; j < eq_subband_gains_count; j++) {
    eq_nodes_cnt = impd_read_bits_buf(it_bit_buff, 5);
    if (it_bit_buff->error) return it_bit_buff->error;

    str_eq_subband_gain_spline->num_eq_nodes = eq_nodes_cnt + 2;

    err = impd_decode_eq_slope_code(it_bit_buff,
                                    &(str_eq_subband_gain_spline->eq_slope[0]),
                                    str_eq_subband_gain_spline->num_eq_nodes);
    if (err) return (err);

    eq_freq_delta = &(str_eq_subband_gain_spline->eq_freq_delta[1]);
    for (k = 1; k < str_eq_subband_gain_spline->num_eq_nodes; k++) {
      bits = impd_read_bits_buf(it_bit_buff, 4);
      if (it_bit_buff->error) return it_bit_buff->error;
      *eq_freq_delta = bits + 1;
      eq_freq_delta++;
    }

    err = impd_decode_gain_initial_code(
        it_bit_buff, &(str_eq_subband_gain_spline->eq_gain_initial));
    if (err) return (err);

    peq_gain_delta = &(str_eq_subband_gain_spline->eq_gain_delta[1]);
    for (k = 1; k < str_eq_subband_gain_spline->num_eq_nodes; k++) {
      bits = impd_read_bits_buf(it_bit_buff, 5);
      if (it_bit_buff->error) return it_bit_buff->error;

      *peq_gain_delta = eq_gain_delta_tbl[bits];
      peq_gain_delta++;
    }
    str_eq_subband_gain_spline++;
  }
  return (0);
}

WORD32 impd_parse_eq_subband_gain_vector(
    ia_bit_buf_struct* it_bit_buff, const WORD32 eq_subband_gain_count,
    ia_eq_subband_gain_vector* str_eq_subband_gain_vector,
    WORD32 eq_subband_gains_count) {
  WORD32 m, k, temp;

  for (k = 0; k < eq_subband_gains_count; k++) {
    for (m = 0; m < eq_subband_gain_count; m++) {
      WORD32 sign, bs_eq_subband_gain;
      temp = impd_read_bits_buf(it_bit_buff, 9);
      if (it_bit_buff->error) return it_bit_buff->error;

      sign = (temp >> 8) & 1;
      bs_eq_subband_gain = temp & 0x7F;

      sign = sign << 1;
      str_eq_subband_gain_vector->eq_subband_gain[m] =
          ((1 - sign) * bs_eq_subband_gain) * 0.125f;
    }
    str_eq_subband_gain_vector++;
  }
  return (0);
}

WORD32 impd_parse_eq_coefficients(ia_bit_buf_struct* it_bit_buff,
                                  ia_eq_coeff_struct* str_eq_coeff) {
  WORD32 err = 0;
  WORD32 eq_gain_cnt, mu, nu, temp;
  static const WORD32 subband_gain_len_tbl[7] = {0, 32, 39, 64, 71, 128, 135};

  str_eq_coeff->eq_delay_max_present = impd_read_bits_buf(it_bit_buff, 1);
  if (it_bit_buff->error) return it_bit_buff->error;

  if (str_eq_coeff->eq_delay_max_present) {
    mu = impd_read_bits_buf(it_bit_buff, 5);
    if (it_bit_buff->error) return it_bit_buff->error;
    nu = impd_read_bits_buf(it_bit_buff, 3);
    if (it_bit_buff->error) return it_bit_buff->error;
    str_eq_coeff->eq_delay_max = 16 * mu * (1 << nu);
  }

  str_eq_coeff->unique_filter_block_count = impd_read_bits_buf(it_bit_buff, 6);
  if (it_bit_buff->error) return it_bit_buff->error;

  if (str_eq_coeff->unique_filter_block_count > FILTER_BLOCK_COUNT_MAX) {
    return (UNEXPECTED_ERROR);
  }

  err = impd_parse_filt_block(it_bit_buff, &(str_eq_coeff->str_filter_block[0]),
                              str_eq_coeff->unique_filter_block_count);
  if (err) return (err);

  str_eq_coeff->unique_td_filter_element_count =
      impd_read_bits_buf(it_bit_buff, 6);
  if (str_eq_coeff->unique_td_filter_element_count > FILTER_ELEMENT_COUNT_MAX)
    return (UNEXPECTED_ERROR);
  if (it_bit_buff->error) return it_bit_buff->error;

  err = impd_parse_unique_td_filt_ele(
      it_bit_buff, &(str_eq_coeff->unique_td_filt_ele[0]),
      str_eq_coeff->unique_td_filter_element_count);
  if (err) return (err);

  str_eq_coeff->unique_eq_subband_gains_count =
      impd_read_bits_buf(it_bit_buff, 6);
  if (str_eq_coeff->unique_eq_subband_gains_count >
      UNIQUE_SUBBAND_GAIN_COUNT_MAX)
    return (UNEXPECTED_ERROR);
  if (it_bit_buff->error) return it_bit_buff->error;

  if (str_eq_coeff->unique_eq_subband_gains_count > 0) {
    temp = impd_read_bits_buf(it_bit_buff, 5);
    if (it_bit_buff->error) return it_bit_buff->error;
    str_eq_coeff->eq_subband_gain_representation = (temp >> 4) & 0x01;

    str_eq_coeff->eq_subband_gain_format = temp & 0x0F;
    if ((str_eq_coeff->eq_subband_gain_format > 0) &&
        (str_eq_coeff->eq_subband_gain_format < GAINFORMAT_UNIFORM)) {
      str_eq_coeff->eq_subband_gain_count =
          subband_gain_len_tbl[str_eq_coeff->eq_subband_gain_format];
    } else {
      /* Gain format 0 or any value between 7 to 15 is considered as default
       * case */
      eq_gain_cnt = impd_read_bits_buf(it_bit_buff, 8);

      if (it_bit_buff->error) return it_bit_buff->error;
      str_eq_coeff->eq_subband_gain_count = eq_gain_cnt + 1;

      if (str_eq_coeff->eq_subband_gain_count > EQ_SUBBAND_GAIN_COUNT_MAX)
        return UNEXPECTED_ERROR;

    }

    if (str_eq_coeff->eq_subband_gain_representation == 1) {
      err = impd_parse_eq_subband_gain_spline(
          it_bit_buff, &(str_eq_coeff->str_eq_subband_gain_spline[0]),
          str_eq_coeff->unique_eq_subband_gains_count);
      if (err) return (err);
    } else {
      err = impd_parse_eq_subband_gain_vector(
          it_bit_buff, str_eq_coeff->eq_subband_gain_count,
          &(str_eq_coeff->str_eq_subband_gain_vector[0]),
          str_eq_coeff->unique_eq_subband_gains_count);
      if (err) return (err);
    }
  }

  return (0);
}

WORD32 impd_parser_td_filter_cascade(
    ia_bit_buf_struct* it_bit_buff,
    ia_eq_instructions_struct* str_eq_instructions,
    ia_td_filter_cascade_struct* str_td_filter_cascade) {
  // WORD32 err=0,
  WORD32 i, ii, k;
  WORD32 eq_cascade_gain;
  ia_filter_block_refs_struct* str_filter_block_refs =
      &(str_td_filter_cascade->str_filter_block_refs[0]);

  for (i = 0; i < str_eq_instructions->eq_ch_group_count; i++) {
    str_td_filter_cascade->eq_cascade_gain_present[i] =
        impd_read_bits_buf(it_bit_buff, 1);
    if (it_bit_buff->error) return it_bit_buff->error;
    if (str_td_filter_cascade->eq_cascade_gain_present[i]) {
      eq_cascade_gain = impd_read_bits_buf(it_bit_buff, 10);
      if (it_bit_buff->error) return it_bit_buff->error;
      str_td_filter_cascade->eq_cascade_gain[i] =
          0.125f * eq_cascade_gain - 96.0f;
    } else {
      str_td_filter_cascade->eq_cascade_gain[i] = 0.0f;
    }

    str_filter_block_refs->filter_block_count =
        impd_read_bits_buf(it_bit_buff, 4);
    if (it_bit_buff->error) return it_bit_buff->error;
    if (str_filter_block_refs->filter_block_count > EQ_FILTER_BLOCK_COUNT_MAX) {
      return (UNEXPECTED_ERROR);
    }

    for (ii = 0; ii < str_filter_block_refs->filter_block_count; ii++) {
      str_filter_block_refs->filter_block_index[ii] =
          impd_read_bits_buf(it_bit_buff, 7);
      if (it_bit_buff->error) return it_bit_buff->error;
      if (str_filter_block_refs->filter_block_index[ii] >=
          FILTER_BLOCK_COUNT_MAX)
        return (UNEXPECTED_ERROR);
    }
    str_filter_block_refs++;
  }

  str_td_filter_cascade->eq_phase_alignment_present =
      impd_read_bits_buf(it_bit_buff, 1);
  if (it_bit_buff->error) return it_bit_buff->error;

  if (str_td_filter_cascade->eq_phase_alignment_present) {
    for (i = 0; i < str_eq_instructions->eq_ch_group_count; i++) {
      for (k = i + 1; k < str_eq_instructions->eq_ch_group_count; k++) {
        str_td_filter_cascade->eq_phase_alignment[i][k] =
            impd_read_bits_buf(it_bit_buff, 1);
        if (it_bit_buff->error) return it_bit_buff->error;
      }
    }

  } else {
    for (i = 0; i < str_eq_instructions->eq_ch_group_count; i++) {
      for (k = i + 1; k < str_eq_instructions->eq_ch_group_count; k++)
        str_td_filter_cascade->eq_phase_alignment[i][k] = 1;
    }
  }

  return (0);
}

WORD32 impd_parse_eq_instructions(
    ia_bit_buf_struct* it_bit_buff, ia_drc_config* drc_config,
    ia_eq_instructions_struct* str_eq_instructions) {
  WORD32 i, channel_count, temp;
  WORD32 dmix_id_present, additional_dmix_id_present,
      additional_dmix_id_cnt = 0;
  WORD32 additional_drc_set_id_present, additional_drc_set_id_cnt;

  temp = impd_read_bits_buf(it_bit_buff, 11);
  if (it_bit_buff->error) return it_bit_buff->error;

  str_eq_instructions->eq_set_id = (temp >> 5) & 0x3F;

  if (str_eq_instructions->eq_set_id >= EQ_INSTRUCTIONS_COUNT_MAX)
    return UNEXPECTED_ERROR;

  str_eq_instructions->eq_set_complexity_level = (temp >> 1) & 0x0F;

  dmix_id_present = temp & 0x01;

  if (dmix_id_present) {
    temp = impd_read_bits_buf(it_bit_buff, 9);
    if (it_bit_buff->error) return it_bit_buff->error;

    str_eq_instructions->downmix_id[0] = (temp >> 2) & 0x7F;

    str_eq_instructions->eq_apply_to_downmix = (temp >> 1) & 0x01;

    additional_dmix_id_present = temp & 0x01;

    if (additional_dmix_id_present) {
      additional_dmix_id_cnt = impd_read_bits_buf(it_bit_buff, 7);
      if (it_bit_buff->error) return it_bit_buff->error;

      if (additional_dmix_id_cnt >= DOWNMIX_ID_COUNT_MAX)
        return UNEXPECTED_ERROR;

      for (i = 1; i < additional_dmix_id_cnt + 1; i++) {
        str_eq_instructions->downmix_id[i] = impd_read_bits_buf(it_bit_buff, 7);
        if (it_bit_buff->error) return it_bit_buff->error;
      }
    }
  } else {
    str_eq_instructions->downmix_id[0] = 0;
  }

  str_eq_instructions->dwnmix_id_count = 1 + additional_dmix_id_cnt;

  temp = impd_read_bits_buf(it_bit_buff, 7);
  if (it_bit_buff->error) return it_bit_buff->error;

  str_eq_instructions->drc_set_id[0] = (temp >> 1) & 0x3F;

  additional_drc_set_id_present = temp & 0x01;

  if (additional_drc_set_id_present) {
    additional_drc_set_id_cnt = impd_read_bits_buf(it_bit_buff, 6);
    if (it_bit_buff->error) return it_bit_buff->error;
    if (additional_drc_set_id_cnt >= DRC_SET_ID_COUNT_MAX)
      return UNEXPECTED_ERROR;

    for (i = 1; i < additional_drc_set_id_cnt + 1; i++) {
      str_eq_instructions->drc_set_id[i] = impd_read_bits_buf(it_bit_buff, 6);
      if (it_bit_buff->error) return it_bit_buff->error;
    }
  } else {
    additional_drc_set_id_cnt = 0;
  }
  str_eq_instructions->drc_set_id_count = 1 + additional_drc_set_id_cnt;

  temp = impd_read_bits_buf(it_bit_buff, 17);
  if (it_bit_buff->error) return it_bit_buff->error;

  str_eq_instructions->eq_set_purpose = (temp >> 1) & 0xFFFF;

  str_eq_instructions->depends_on_eq_set_present = temp & 0x01;

  if (str_eq_instructions->depends_on_eq_set_present) {
    str_eq_instructions->depends_on_eq_set = impd_read_bits_buf(it_bit_buff, 6);
    if (it_bit_buff->error) return it_bit_buff->error;
  } else {
    str_eq_instructions->no_independent_eq_use =
        impd_read_bits_buf(it_bit_buff, 1);
    if (it_bit_buff->error) return it_bit_buff->error;
  }

  str_eq_instructions->eq_channel_count = channel_count =
      drc_config->channel_layout.base_channel_count;

  if ((dmix_id_present == 1) &&
      (str_eq_instructions->eq_apply_to_downmix == 1) &&
      (str_eq_instructions->downmix_id[0] != 0) &&
      (str_eq_instructions->downmix_id[0] != ID_FOR_ANY_DOWNMIX) &&
      (str_eq_instructions->dwnmix_id_count == 1)) {
    for (i = 0; i < drc_config->dwnmix_instructions_count; i++) {
      if (str_eq_instructions->downmix_id[0] ==
          drc_config->dwnmix_instructions[i].downmix_id)
        break;
    }
    if (i == drc_config->dwnmix_instructions_count) {
      return UNEXPECTED_ERROR;
    }

    str_eq_instructions->eq_channel_count = channel_count =
        drc_config->dwnmix_instructions[i].target_channel_count;
  } else if ((str_eq_instructions->downmix_id[0] == ID_FOR_ANY_DOWNMIX) ||
             (str_eq_instructions->dwnmix_id_count > 1)) {
    channel_count = 1;
  }

  str_eq_instructions->eq_ch_group_count = 0;

  for (i = 0; i < channel_count; i++) {
    WORD32 tmp = impd_read_bits_buf(it_bit_buff, 7);
    if (it_bit_buff->error) return it_bit_buff->error;
    if (tmp >= EQ_CHANNEL_GROUP_COUNT_MAX) return UNEXPECTED_ERROR;
    str_eq_instructions->eq_ch_group_of_channel[i] = tmp;
  }
  {
    WORD32 total;
    WORD32 groups_used[EQ_CHANNEL_GROUP_COUNT_MAX] = {0};
    for (i = 0; i < channel_count; i++)
      groups_used[str_eq_instructions->eq_ch_group_of_channel[i]] = 1;
    total = 0;
    for (i = 0; i < EQ_CHANNEL_GROUP_COUNT_MAX; i++)
      if (groups_used[i]) total++;
    str_eq_instructions->eq_ch_group_count = total;
  }

  if (str_eq_instructions->eq_ch_group_count > EQ_CHANNEL_GROUP_COUNT_MAX)
    return (UNEXPECTED_ERROR);

  str_eq_instructions->td_filter_cascade_present =
      impd_read_bits_buf(it_bit_buff, 1);
  if (it_bit_buff->error) return it_bit_buff->error;

  if (str_eq_instructions->td_filter_cascade_present) {
    WORD32 err = impd_parser_td_filter_cascade(
        it_bit_buff, str_eq_instructions,
        &(str_eq_instructions->str_td_filter_cascade));
    if (err) return err;
  }

  str_eq_instructions->subband_gains_present =
      impd_read_bits_buf(it_bit_buff, 1);
  if (it_bit_buff->error) return it_bit_buff->error;

  if (str_eq_instructions->subband_gains_present) {
    for (i = 0; i < str_eq_instructions->eq_ch_group_count; i++) {
      WORD32 tmp = impd_read_bits_buf(it_bit_buff, 6);
      if (it_bit_buff->error) return it_bit_buff->error;
      if (tmp >= UNIQUE_SUBBAND_GAIN_COUNT_MAX) return UNEXPECTED_ERROR;
      str_eq_instructions->subband_gains_index[i] = tmp;
    }
  }

  str_eq_instructions->eq_transition_duration_present =
      impd_read_bits_buf(it_bit_buff, 1);
  if (it_bit_buff->error) return it_bit_buff->error;

  if (str_eq_instructions->eq_transition_duration_present) {
    WORD32 bs_eq_transition_duration;
    bs_eq_transition_duration = impd_read_bits_buf(it_bit_buff, 5);
    if (it_bit_buff->error) return it_bit_buff->error;

    str_eq_instructions->eq_transition_duration = (WORD32)(
        0.001f *
        (FLOAT32)pow(2.0f, 2.0f + bs_eq_transition_duration * 0.0625f));
  }
  return (0);
}

WORD32 impd_parse_loud_eq_instructions(
    ia_bit_buf_struct* it_bit_buff,
    ia_loud_eq_instructions_struct* loud_eq_instructions) {
  WORD32 i, bs_loud_eq_scaling, bs_loud_eq_offset, temp;
  WORD32 dmix_id_present, additional_dmix_id_present,
      additional_dmix_id_cnt = 0;
  WORD32 drc_set_id_present, additional_drc_set_id_present,
      additional_drc_set_id_cnt = 0;
  WORD32 eq_set_id_present, additional_eq_set_id_present,
      additional_eq_set_id_cnt = 0;

  temp = impd_read_bits_buf(it_bit_buff, 9);
  if (it_bit_buff->error) return it_bit_buff->error;
  loud_eq_instructions->loud_eq_set_id = (temp >> 5) & 0x0F;

  loud_eq_instructions->drc_location = (temp >> 1) & 0x0F;

  dmix_id_present = temp & 0x01;

  if (dmix_id_present) {
    temp = impd_read_bits_buf(it_bit_buff, 8);
    if (it_bit_buff->error) return it_bit_buff->error;

    loud_eq_instructions->downmix_id[0] = (temp >> 1) & 0x7F;

    additional_dmix_id_present = temp & 0x01;

    if (additional_dmix_id_present) {
      additional_dmix_id_cnt = impd_read_bits_buf(it_bit_buff, 7);
      if (it_bit_buff->error) return it_bit_buff->error;
      if (additional_dmix_id_cnt >= DOWNMIX_ID_COUNT_MAX)
        return UNEXPECTED_ERROR;
      for (i = 1; i < additional_dmix_id_cnt + 1; i++) {
        loud_eq_instructions->downmix_id[i] =
            impd_read_bits_buf(it_bit_buff, 7);
        if (it_bit_buff->error) return it_bit_buff->error;
      }
    }
  } else {
    loud_eq_instructions->downmix_id[0] = 0;
  }

  loud_eq_instructions->dwnmix_id_count = 1 + additional_dmix_id_cnt;

  drc_set_id_present = impd_read_bits_buf(it_bit_buff, 1);
  if (it_bit_buff->error) return it_bit_buff->error;

  if (drc_set_id_present) {
    temp = impd_read_bits_buf(it_bit_buff, 7);
    if (it_bit_buff->error) return it_bit_buff->error;

    loud_eq_instructions->drc_set_id[0] = (temp >> 1) & 0x3F;

    additional_drc_set_id_present = temp & 0x01;

    if (additional_drc_set_id_present) {
      additional_drc_set_id_cnt = impd_read_bits_buf(it_bit_buff, 6);
      if (it_bit_buff->error) return it_bit_buff->error;
      if ((additional_drc_set_id_cnt >= DRC_SET_ID_COUNT_MAX))
        return UNEXPECTED_ERROR;

      for (i = 1; i < additional_drc_set_id_cnt + 1; i++) {
        loud_eq_instructions->drc_set_id[i] =
            impd_read_bits_buf(it_bit_buff, 6);
        if (it_bit_buff->error) return it_bit_buff->error;
      }
    }
  } else {
    loud_eq_instructions->drc_set_id[0] = 0;
  }

  loud_eq_instructions->drc_set_id_count = 1 + additional_drc_set_id_cnt;

  eq_set_id_present = impd_read_bits_buf(it_bit_buff, 1);

  if (it_bit_buff->error) return it_bit_buff->error;

  if (eq_set_id_present) {
    temp = impd_read_bits_buf(it_bit_buff, 7);
    if (it_bit_buff->error) return it_bit_buff->error;

    loud_eq_instructions->eq_set_id[0] = (temp >> 1) & 0x3F;

    additional_eq_set_id_present = temp & 0x01;

    if (additional_eq_set_id_present) {
      additional_eq_set_id_cnt = impd_read_bits_buf(it_bit_buff, 6);
      if (it_bit_buff->error) return it_bit_buff->error;
      if (additional_eq_set_id_cnt >= EQ_SET_ID_COUNT_MAX)
        return UNEXPECTED_ERROR;
      for (i = 0; i < additional_eq_set_id_cnt; i++) {
        loud_eq_instructions->eq_set_id[i + 1] =
            impd_read_bits_buf(it_bit_buff, 6);
        if (it_bit_buff->error) return it_bit_buff->error;
      }
    }
  } else {
    loud_eq_instructions->eq_set_id[0] = 0;
  }
  loud_eq_instructions->eq_set_id_count = 1 + additional_eq_set_id_cnt;

  temp = impd_read_bits_buf(it_bit_buff, 8);
  if (it_bit_buff->error) return it_bit_buff->error;

  /* Parsed but unused */
  loud_eq_instructions->loudness_after_drc = (temp >> 7) & 0x01;

  /* Parsed but unused */
  loud_eq_instructions->loudness_after_eq = (temp >> 6) & 0x01;

  /* Parsed but unused */
  loud_eq_instructions->loud_eq_gain_sequence_count = temp & 0x3F;

  if (loud_eq_instructions->loud_eq_gain_sequence_count >
      LOUD_EQ_GAIN_SEQUENCE_COUNT_MAX)
    return UNEXPECTED_ERROR;

  /* Section under for loop, Parsed but unused */
  for (i = 0; i < loud_eq_instructions->loud_eq_gain_sequence_count; i++) {
    temp = impd_read_bits_buf(it_bit_buff, 7);
    if (it_bit_buff->error) return it_bit_buff->error;

    loud_eq_instructions->gain_seq_idx[i] = (temp >> 1) & 0x3F;

    loud_eq_instructions->drc_characteristic_format_is_cicp[i] = temp & 0x01;

    if (loud_eq_instructions->drc_characteristic_format_is_cicp[i]) {
      loud_eq_instructions->drc_characteristic[i] =
          impd_read_bits_buf(it_bit_buff, 7);
      if (it_bit_buff->error) return it_bit_buff->error;
    } else {
      temp = impd_read_bits_buf(it_bit_buff, 8);
      if (it_bit_buff->error) return it_bit_buff->error;

      loud_eq_instructions->drc_characteristic_left_index[i] =
          (temp >> 4) & 0x0F;

      loud_eq_instructions->drc_characteristic_right_index[i] = temp & 0x0F;
    }

    temp = impd_read_bits_buf(it_bit_buff, 9);
    if (it_bit_buff->error) return it_bit_buff->error;

    loud_eq_instructions->frequency_range_index[i] = (temp >> 3) & 0x3F;

    bs_loud_eq_scaling = temp & 0x07;

    loud_eq_instructions->loud_eq_scaling[i] =
        (FLOAT32)pow(2.0f, -0.5f * bs_loud_eq_scaling);

    bs_loud_eq_offset = impd_read_bits_buf(it_bit_buff, 5);
    if (it_bit_buff->error) return it_bit_buff->error;

    loud_eq_instructions->loud_eq_offset[i] = 1.5f * bs_loud_eq_offset - 16.0f;
  }
  return (0);
}
