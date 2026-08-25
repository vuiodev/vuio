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
#include "ixheaac_constants.h"
#include "ixheaacd_cnst.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops16.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaac_basic_ops.h"
#include "ixheaacd_bitbuffer.h"
#include "ixheaacd_mps_aac_struct.h"
#include "ixheaacd_mps_res_rom.h"
#include "ixheaacd_mps_res_pulsedata.h"
#include "ixheaacd_mps_res_channelinfo.h"
#include "ixheaacd_mps_res_block.h"
#include "ixheaacd_defines.h"
#include "ixheaacd_mps_res_channel.h"
#include "ixheaac_basic_op.h"
#include "ixheaacd_mps_res_tns.h"
#include "ixheaacd_mps_res.h"
#include "ixheaacd_mps_res_huffman.h"

UWORD32 ixheaacd_res_aac_showbits_32(UWORD8 *p_read_next) {
  UWORD8 *v = p_read_next;
  UWORD32 b = 0;

  _SWAP(v, b);
  return b;
}

static WORD16 ixheaacd_res_c_block_read(
    ia_bit_buf_struct *it_bit_buf,
    ia_mps_dec_residual_channel_info_struct *p_aac_decoder_channel_info, WORD16 global_gain,
    ia_mps_dec_residual_aac_tables_struct *aac_tables_ptr)

{
  FLAG gain_control_data_present;
  WORD16 error_status = AAC_DEC_OK;

  if (p_aac_decoder_channel_info->ics_info.window_sequence == EIGHT_SHORT_SEQUENCE) {
    memset(p_aac_decoder_channel_info->p_scale_factor, 0, MAX_WINDOWS * MAX_SFB_SHORT * 3);
  }

  error_status = ixheaacd_c_block_read_section_data(it_bit_buf, p_aac_decoder_channel_info);

  if (error_status) return error_status;

  ixheaacd_res_c_block_read_scf_data(it_bit_buf, p_aac_decoder_channel_info, global_gain,
                                     aac_tables_ptr);

  error_status = ixheaacd_res_c_pulse_data_read(
      it_bit_buf, &p_aac_decoder_channel_info->pulse_data, aac_tables_ptr);
  if (error_status) return error_status;

  p_aac_decoder_channel_info->tns_data.tns_data_present =
      (FLAG)ixheaacd_read_bits_buf(it_bit_buf, 1);

  error_status = ixheaacd_res_c_tns_read(it_bit_buf, p_aac_decoder_channel_info);
  if (error_status) {
    return error_status;
  }

  gain_control_data_present = ixheaacd_read_bits_buf(it_bit_buf, 1);

  if (gain_control_data_present) {
    return (WORD16)((WORD32)AAC_DEC_UNIMPLEMENTED_GAIN_CONTROL_DATA);
  }

  {
    it_bit_buf->bit_pos = (7 - it_bit_buf->bit_pos);

    error_status = ixheaacd_res_c_block_read_spec_data(it_bit_buf, p_aac_decoder_channel_info,
                                                       aac_tables_ptr);

    it_bit_buf->bit_pos = (7 - it_bit_buf->bit_pos);
  }

  return error_status;
}

WORD16 ixheaacd_res_read_ics(
    ia_bit_buf_struct *it_bit_buf,
    ia_mps_dec_residual_channel_info_struct *p_aac_decoder_channel_info[CHANNELS], WORD32 num_ch,
    ia_mps_dec_residual_aac_tables_struct *aac_tables_ptr, WORD8 tot_sf_bands_ls[2]) {
  WORD16 error_status = AAC_DEC_OK;
  WORD32 ch;

  for (ch = 0; ch < num_ch; ch++) {
    ia_mps_dec_residual_channel_info_struct *p_aac_dec_ch_info = p_aac_decoder_channel_info[ch];
    ia_mps_dec_residual_ics_info_struct *p_ics_info = &p_aac_dec_ch_info->ics_info;

    p_aac_dec_ch_info->global_gain = (WORD16)ixheaacd_read_bits_buf(it_bit_buf, 8);

    if (!p_aac_decoder_channel_info[LEFT]->common_window) {
      error_status = ixheaacd_res_ics_read(it_bit_buf, p_ics_info, tot_sf_bands_ls);
      if (error_status) {
        if (it_bit_buf->cnt_bits < 0)
          error_status = (WORD16)((WORD32)IA_XHEAAC_DEC_EXE_NONFATAL_INSUFFICIENT_INPUT_BYTES);
        return error_status;
      }
    }

    error_status = ixheaacd_res_c_block_read(it_bit_buf, p_aac_dec_ch_info,
                                             p_aac_dec_ch_info->global_gain, aac_tables_ptr);
    if (error_status) {
      if (it_bit_buf->cnt_bits < 0)
        error_status = (WORD16)((WORD32)IA_XHEAAC_DEC_EXE_NONFATAL_INSUFFICIENT_INPUT_BYTES);

      return error_status;
    }
  }

  return error_status;
}

VOID ixheaacd_res_c_pulse_data_apply(ia_mps_dec_residual_pulse_data_struct *pulse_data,
                                     WORD8 *p_pulse_arr,
                                     const WORD16 *p_scale_factor_band_offsets) {
  WORD i, number_pulse;
  WORD32 k;

  memset(p_pulse_arr, 0, sizeof(WORD32) * 256);

  if (pulse_data->pulse_data_present) {
    k = p_scale_factor_band_offsets[pulse_data->pulse_start_band];
    number_pulse = pulse_data->number_pulse;

    for (i = 0; i <= number_pulse; i++) {
      k = add_d(k, pulse_data->pulse_offset[i]);
      p_pulse_arr[k] = pulse_data->pulse_amp[i];
    }
  }
}

WORD16 ixheaacd_res_c_block_read_spec_data(
    ia_bit_buf_struct *it_bit_buf,
    ia_mps_dec_residual_channel_info_struct *p_aac_decoder_channel_info,
    ia_mps_dec_residual_aac_tables_struct *aac_tables_ptr) {
  WORD band, tot_bands, tot_groups = 0;
  WORD group, groupwin, groupoffset;

  WORD index;
  WORD8 *p_code_book, *p_codebook_tmp;
  WORD16 *p_scale_factor;
  WORD32 *p_spectral_coefficient;
  ia_mps_dec_residual_ics_info_struct *p_ics_info = &p_aac_decoder_channel_info->ics_info;
  WORD16 *band_offsets;
  WORD32 maximum_bins_short = ixheaac_shr16_dir_sat(p_ics_info->frame_length, 3);

  WORD32 *p_spec_coeff_out;

  p_code_book = p_aac_decoder_channel_info->p_code_book;
  p_scale_factor = p_aac_decoder_channel_info->p_scale_factor;
  p_spectral_coefficient = p_aac_decoder_channel_info->p_spectral_coefficient;
  tot_groups = p_ics_info->window_groups;
  tot_bands = p_ics_info->max_sf_bands;
  band_offsets = (WORD16 *)ixheaacd_res_get_sfb_offsets(p_ics_info, aac_tables_ptr);

  if (p_aac_decoder_channel_info->ics_info.window_sequence != EIGHT_SHORT_SEQUENCE) {
    WORD8 *p_pul_arr = (WORD8 *)p_aac_decoder_channel_info->p_tns_scratch;
    ixheaacd_res_c_pulse_data_apply(&p_aac_decoder_channel_info->pulse_data, p_pul_arr,
                                    band_offsets);

    p_spec_coeff_out = &p_spectral_coefficient[0];
    for (band = 0; band < tot_bands;) {
      WORD ret_val;
      WORD32 len;
      WORD32 code_no = p_code_book[band];
      WORD start = band;

      for (; band < tot_bands && (p_code_book[band] == code_no); band++)
        ;

      len = band_offsets[band] - band_offsets[start];

      if (code_no > ZERO_HCB && (code_no < NOISE_HCB)) {
        ret_val = ixheaacd_res_c_block_decode_huff_word_all_lb(
            it_bit_buf, code_no, len, aac_tables_ptr, p_spec_coeff_out, p_pul_arr);

        if (ret_val != 0)
          return (WORD16)((WORD32)IA_XHEAAC_DEC_EXE_NONFATAL_EXCEEDS_MAX_HUFFDEC_VAL);
      } else {
        if (p_aac_decoder_channel_info->pulse_data.pulse_data_present)
          ixheaacd_res_inverse_quant_lb(
              p_spec_coeff_out, len,
              (WORD32 *)aac_tables_ptr->res_block_tables_ptr->pow_table_q17, p_pul_arr);
        else
          memset(p_spec_coeff_out, 0, sizeof(WORD32) * len);
      }
      p_pul_arr += len;
      p_spec_coeff_out += len;
    }

    index = 1024 - band_offsets[tot_bands];
    memset(p_spec_coeff_out, 0, sizeof(WORD32) * index);
  } else {
    memset(p_spectral_coefficient, 0, sizeof(WORD32) * 1024);

    groupoffset = 0;

    for (group = 0; group < tot_groups; group++) {
      WORD grp_win = p_ics_info->window_group_length[group];
      p_codebook_tmp = &p_code_book[group * MAX_SFB_SHORT];
      p_spec_coeff_out = &p_spectral_coefficient[groupoffset * maximum_bins_short];

      for (band = 0; band < tot_bands;) {
        WORD code_no = *p_codebook_tmp;
        WORD start = band;
        WORD ret_val;

        for (; band < tot_bands && (*p_codebook_tmp == code_no); band++, p_codebook_tmp++)
          ;

        if (code_no > ZERO_HCB && (code_no < NOISE_HCB)) {
          ret_val = ixheaacd_res_c_block_decode_huff_word_all(
              it_bit_buf, code_no, p_spec_coeff_out, (WORD16 *)band_offsets, start, band, grp_win,
              aac_tables_ptr, maximum_bins_short);

          if (ret_val != 0)
            return (WORD16)((WORD32)IA_XHEAAC_DEC_EXE_NONFATAL_EXCEEDS_MAX_HUFFDEC_VAL);
        }
      }
      groupoffset = (groupoffset + grp_win);
    }
  }

  {
    WORD8 *p_win_grp_len = &p_ics_info->window_group_length[0];
    WORD8 *psfb_width = (WORD8 *)ixheaacd_res_get_sfb_width(p_ics_info, aac_tables_ptr);
    WORD32 *scale_table_ptr;
    if (120 == maximum_bins_short) {
      scale_table_ptr = aac_tables_ptr->res_block_tables_ptr->scale_table_960;
    } else {
      scale_table_ptr = aac_tables_ptr->res_block_tables_ptr->scale_table;
    }
    do {
      groupwin = *p_win_grp_len++;
      do {
        ixheaacd_res_apply_scfs(&p_spectral_coefficient[0], &p_scale_factor[0], tot_bands,
                                (WORD8 *)psfb_width, scale_table_ptr);

        p_spectral_coefficient += maximum_bins_short;
        groupwin--;
      } while (groupwin != 0);

      p_scale_factor += MAX_SFB_SHORT;
      tot_groups--;
    } while (tot_groups != 0);
  }

  return AAC_DEC_OK;
}

WORD16
ixheaacd_res_c_tns_read(ia_bit_buf_struct *it_bit_buf,
                        ia_mps_dec_residual_channel_info_struct *p_aac_decoder_channel_info) {
  WORD window, window_per_frame;
  WORD n_filt_bits, len_bits, order_bits;
  WORD32 next_stop_band_tmp;

  ia_mps_dec_residual_ics_info_struct *p_ics_info = &p_aac_decoder_channel_info->ics_info;
  ia_mps_dec_residual_tns_data *p_tns_data = &p_aac_decoder_channel_info->tns_data;

  if (!p_tns_data->tns_data_present) return AAC_DEC_OK;

  if (p_ics_info->window_sequence != EIGHT_SHORT_SEQUENCE) {
    n_filt_bits = 2;
    len_bits = 6;
    order_bits = 5;
    window_per_frame = 1;
  } else {
    n_filt_bits = 1;
    len_bits = 4;
    order_bits = 3;
    window_per_frame = 8;
  }

  next_stop_band_tmp = p_ics_info->total_sf_bands;

  for (window = 0; window < window_per_frame; window++) {
    WORD n_filt;
    WORD length, coef_res;
    p_tns_data->number_of_filters[window] = n_filt =
        (WORD16)ixheaacd_read_bits_buf(it_bit_buf, n_filt_bits);

    if (n_filt) {
      WORD32 accu;
      WORD index;
      WORD nextstopband;

      coef_res = ixheaacd_read_bits_buf(it_bit_buf, 1);

      nextstopband = next_stop_band_tmp;
      for (index = 0; index < n_filt; index++) {
        WORD order;
        ia_mps_dec_residual_filter_struct *filter = &p_tns_data->filter[window][index];
        length = ixheaacd_read_bits_buf(it_bit_buf, len_bits);

        filter->start_band = nextstopband - length;
        filter->stop_band = nextstopband;

        nextstopband = filter->start_band;

        if (filter->start_band < 0) {
          filter->order = -1;
          return (WORD16)((WORD32)AAC_DEC_TNS_RANGE_ERROR);
        }

        filter->order = order = ixheaacd_read_bits_buf(it_bit_buf, order_bits);
        accu = order - MAX_ORDER_LONG;

        if (accu > 0) return (WORD16)((WORD32)AAC_DEC_TNS_ORDER_ERROR);

        if (order) {
          WORD i;
          WORD32 coef, coef_compress;
          WORD resolution, shift;

          filter->direction = (WORD8)(ixheaacd_read_bits_buf(it_bit_buf, 1) ? -1 : 1);

          coef_compress = ixheaacd_read_bits_buf(it_bit_buf, 1);

          filter->resolution = coef_res;
          resolution = coef_res + 3 - coef_compress;
          shift = 32 - resolution;

          for (i = 0; i < order; i++) {
            coef = ixheaacd_read_bits_buf(it_bit_buf, resolution);
            coef = coef << shift;
            filter->coeff[i] = (WORD8)(coef >> shift);
          }
        }
      }
    }
  }
  return AAC_DEC_OK;
}

WORD32 ixheaacd_res_inv_quant(WORD32 *px_quant, WORD32 *pow_table_q17)

{
  WORD32 q1;
  WORD32 temp;
  WORD32 q_abs;
  WORD16 interp;
  WORD32 shift;

  q_abs = *px_quant;

  if (q_abs > (8191 + 32)) return IA_XHEAAC_DEC_EXE_NONFATAL_EXCEEDS_MAX_HUFFDEC_VAL;

  if (q_abs < 1024)
    shift = 3;
  else
    shift = 6;

  q1 = (q_abs) >> shift;

  interp = q_abs - (q1 << shift);

  temp = pow_table_q17[q1 + 1] - pow_table_q17[q1];

  temp = (WORD32)(temp * (WORD32)interp);

  temp = temp + (pow_table_q17[q1] << shift);

  if (shift == 3)
    temp = temp << 1;
  else
    temp = temp << 2;

  *px_quant = temp;

  return 0;
}
