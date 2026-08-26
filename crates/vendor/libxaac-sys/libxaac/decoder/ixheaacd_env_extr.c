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

#include "ixheaac_basic_op.h"
#include "ixheaacd_intrinsics.h"
#include "ixheaacd_common_rom.h"
#include "ixheaacd_basic_funcs.h"
#include "ixheaacd_bitbuffer.h"
#include "ixheaacd_sbr_common.h"
#include "ixheaacd_drc_data_struct.h"
#include "ixheaacd_drc_dec.h"
#include "ixheaacd_sbrdecoder.h"
#include "ixheaacd_sbrdecsettings.h"
#include "ixheaacd_sbr_scale.h"
#include "ixheaacd_lpp_tran.h"
#include "ixheaacd_env_extr_part.h"
#include "ixheaacd_sbr_rom.h"
#include "ixheaacd_hybrid.h"
#include "ixheaacd_ps_dec.h"
#include "ixheaacd_env_extr.h"

#include "ixheaac_sbr_const.h"
#include "ixheaacd_intrinsics.h"

#include "ixheaacd_pvc_dec.h"

#include "ixheaacd_ps_bitdec.h"

#include "ixheaacd_audioobjtypes.h"


static const int ixheaacd_ld_env_table_512[LD_ENV_TBL_512][LD_ENV_TBL_SIZE] = {
    {2, 4, -1, 0},  {2, 5, -1, 0},  {3, 2, 6, 1},   {3, 3, 7, 1},
    {3, 4, 8, 1},   {3, 5, 9, 1},   {3, 6, 10, 1},  {3, 7, 11, 1},
    {3, 8, 12, 1},  {3, 9, 13, 1},  {3, 10, 14, 1}, {2, 11, -1, 1},
    {2, 12, -1, 1}, {2, 13, -1, 1}, {2, 14, -1, 1}, {2, 15, -1, 1},
};

static const int ixheaacd_ld_env_table_480[LD_ENV_TBL_480][LD_ENV_TBL_SIZE] = {
    {2, 4, -1, 0},  {2, 5, -1, 0},  {3, 2, 6, 1},   {3, 3, 7, 1},
    {3, 4, 8, 1},   {3, 5, 9, 1},   {3, 6, 10, 1},  {3, 7, 11, 1},
    {3, 8, 12, 1},  {3, 9, 13, 1},  {2, 10, -1, 1}, {2, 11, -1, 1},
    {2, 12, -1, 1}, {2, 13, -1, 1}, {2, 14, -1, 1},
};

static const int ixheaacd_ld_env_table_time_slot[LD_ENV_TIME_SLOT] = {
    8, 5, 0, 0, 0, 0, 0};

WORD32 ixheaacd_cnt_leading_ones(WORD32 a) {
  WORD32 count = 0;

  while (a) {
    if (a & 0x80000000)
      count++;
    else
      break;
    a = a << 1;
  }
  return count;
}
VOID ixheaacd_huffman_decode(WORD32 it_bit_buff, WORD16 *h_index, WORD16 *len,
                             const UWORD16 *input_table,
                             const UWORD32 *idx_table) {
  UWORD32 temp = 0;
  UWORD32 temp1 = 0;
  WORD32 found = 0;
  UWORD32 mask = 0x80000000;

  WORD32 clo;
  WORD32 MAX_LEN;
  WORD32 ixheaacd_drc_offset = 0;
  WORD32 length;
  UWORD32 cwrd;
  WORD32 len_end;

  MAX_LEN = input_table[0];
  mask = mask - (1 << (31 - MAX_LEN));
  mask = mask << 1;
  temp = (UWORD32)(it_bit_buff & mask);

  len_end = input_table[0];
  clo = ixheaacd_cnt_leading_ones(temp);
  do {
    ixheaacd_drc_offset = (idx_table[clo] >> 20) & 0xff;
    length = input_table[ixheaacd_drc_offset + 1] & 0x1f;
    cwrd = idx_table[clo] & 0xfffff;
    temp1 = temp >> (32 - length);
    if (temp1 <= cwrd) {
      ixheaacd_drc_offset = ixheaacd_drc_offset - (cwrd - temp1);
      found = 1;
    } else {
      len_end = len_end + ((idx_table[clo] >> 28) & 0xf);
      clo = len_end;
    }
  } while (!found);
  *h_index = input_table[ixheaacd_drc_offset + 1] >> 5;
  *len = length;
}

static WORD32 ixheaacd_read_esbr_pvc_envelope(ia_pvc_data_struct *ptr_pvc_data,
                                              ia_bit_buf_struct *it_bit_buff,
                                              WORD32 indepFlag) {
  WORD32 i, j, k;
  WORD32 fixed_length = 0, num_grid_info = 0, grid_info;
  UWORD8 div_mode, ns_mode;
  UWORD16 pvc_id[PVC_NUM_TIME_SLOTS + 1];
  UWORD8 num_length;
  UWORD8 length;
  UWORD8 reuse_pvc_id;
  WORD32 sum_length = 0;
  WORD32 length_bits = 4;
  UWORD8 pvc_id_bits = PVC_ID_BITS;
  IA_ERRORCODE err = IA_NO_ERROR;

  div_mode = (UWORD8)ixheaacd_read_bits_buf(it_bit_buff, PVC_DIV_MODE_BITS);
  ns_mode = (UWORD8)ixheaacd_read_bits_buf(it_bit_buff, PVC_NS_MODE_BITS);

  if (ptr_pvc_data->pvc_mode == 3) {
    pvc_id_bits = 0;
  }

  if (div_mode <= 3) {
    num_length = div_mode;
    if (indepFlag) {
      reuse_pvc_id = 0;
    } else {
      reuse_pvc_id =
          (UWORD8)ixheaacd_read_bits_buf(it_bit_buff, PVC_REUSE_PVC_ID_BITS);
    }
    if (reuse_pvc_id == 1) {
      pvc_id[0] = ptr_pvc_data->prev_pvc_id;
    } else {
      pvc_id[0] = (UWORD16)ixheaacd_read_bits_buf(it_bit_buff, pvc_id_bits);
    }

    k = 1;
    if (num_length) {
      sum_length = 0;
      for (i = 0; i < num_length; i++) {
        if (sum_length >= 13) {
          length_bits = 1;
        } else if (sum_length >= 11) {
          length_bits = 2;
        } else if (sum_length >= 7) {
          length_bits = 3;
        } else {
          length_bits = 4;
        }
        length = (UWORD8)ixheaacd_read_bits_buf(it_bit_buff, length_bits);
        length += 1;
        sum_length += length;
        if ((k + length - 1) > PVC_NUM_TIME_SLOTS) {
          return -1;
        }
        for (j = 1; j < length; j++, k++) {
          pvc_id[k] = pvc_id[k - 1];
        }
        pvc_id[k++] = (UWORD16)ixheaacd_read_bits_buf(it_bit_buff, pvc_id_bits);
      }
    }

    for (; k < 16; k++) {
      pvc_id[k] = pvc_id[k - 1];
    }

  } else {
    switch (div_mode) {
      case 4:
        num_grid_info = 2;
        fixed_length = 8;
        break;
      case 5:
        num_grid_info = 4;
        fixed_length = 4;
        break;
      case 6:
        num_grid_info = 8;
        fixed_length = 2;
        break;
      case 7:
        num_grid_info = 16;
        fixed_length = 1;
        break;
      default:;
    }
    if (indepFlag) {
      grid_info = 1;
    } else {
      grid_info = ixheaacd_read_bits_buf(it_bit_buff, PVC_GRID_INFO_BITS);
    }
    if (grid_info) {
      pvc_id[0] = (UWORD16)ixheaacd_read_bits_buf(it_bit_buff, pvc_id_bits);
    } else {
      pvc_id[0] = ptr_pvc_data->prev_pvc_id;
    }
    for (j = 1, k = 1; j < fixed_length; j++, k++) {
      pvc_id[k] = pvc_id[k - 1];
    }

    for (i = 1; i < num_grid_info; i++) {
      grid_info = ixheaacd_read_bits_buf(it_bit_buff, PVC_GRID_INFO_BITS);
      if (grid_info == 1) {
        pvc_id[k++] = (UWORD16)ixheaacd_read_bits_buf(it_bit_buff, pvc_id_bits);
      } else {
        pvc_id[k] = pvc_id[k - 1];
        k++;
      }
      for (j = 1; j < fixed_length; j++, k++) {
        pvc_id[k] = pvc_id[k - 1];
      }
    }
  }
  ptr_pvc_data->div_mode = div_mode;
  ptr_pvc_data->ns_mode = ns_mode;
  for (i = 0; i < PVC_NUM_TIME_SLOTS; i++) {
    ptr_pvc_data->pvc_id[i] = pvc_id[i];
  }
  return err;
}

static VOID ixheaacd_pvc_env_dtdf_data(
    ia_sbr_frame_info_data_struct *ptr_frame_data,
    ia_bit_buf_struct *it_bit_buff) {
  WORD32 i;
  WORD32 usac_independency_flag = ptr_frame_data->usac_independency_flag;
  WORD32 bs_num_noise = ptr_frame_data->str_frame_info_details.num_noise_env;

  if (usac_independency_flag) {
    ptr_frame_data->del_cod_dir_noise_arr[0] = 0;
  } else {
    ptr_frame_data->del_cod_dir_noise_arr[0] =
        ixheaacd_read_bits_buf(it_bit_buff, ESBR_DOMAIN_BITS);
  }

  for (i = 1; i < bs_num_noise; i++) {
    ptr_frame_data->del_cod_dir_noise_arr[i] =
        ixheaacd_read_bits_buf(it_bit_buff, ESBR_DOMAIN_BITS);
  }
}

static VOID ixheaacd_read_sbr_addi_data(
    ia_sbr_frame_info_data_struct *ptr_frame_data,
    ia_sbr_header_data_struct *ptr_header_data,
    ia_bit_buf_struct *it_bit_buff) {
  WORD32 i;

  WORD32 flag = ixheaacd_read_bits_buf(it_bit_buff, 1);

  ptr_frame_data->sin_start_for_cur_top =
      ptr_frame_data->sin_start_for_next_top;
  ptr_frame_data->sin_len_for_cur_top = ptr_frame_data->sin_len_for_next_top;
  ptr_frame_data->sin_start_for_next_top = 0;
  ptr_frame_data->sin_len_for_next_top = 0;

  if (flag) {
    for (i = 0; i < ptr_header_data->pstr_freq_band_data->num_sf_bands[HIGH];
         i++) {
      ptr_frame_data->add_harmonics[i] = ixheaacd_read_bits_buf(it_bit_buff, 1);
    }
    if (ptr_frame_data->pvc_mode != 0) {
      ptr_frame_data->sine_position = ESC_SIN_POS;

      ptr_frame_data->bs_sin_pos_present =
          ixheaacd_read_bits_buf(it_bit_buff, 1);

      if (ptr_frame_data->bs_sin_pos_present == 1) {
        ptr_frame_data->sine_position = ixheaacd_read_bits_buf(it_bit_buff, 5);
      }
      if (ptr_frame_data->var_len > 0) {
        if (ptr_frame_data->sine_position > 16) {
          if (ptr_frame_data->sine_position == 31) {
            ptr_frame_data->sin_start_for_next_top = 0;
            ptr_frame_data->sin_len_for_next_top = ptr_frame_data->var_len;
          } else {
            if ((ptr_frame_data->var_len + 16) ==
                ptr_frame_data->sine_position) {
              ptr_frame_data->sin_start_for_next_top = 0;
              ptr_frame_data->sin_len_for_next_top = ptr_frame_data->var_len;
            } else {
              ptr_frame_data->sin_start_for_next_top =
                  ptr_frame_data->sine_position - 16;
              ptr_frame_data->sin_len_for_next_top = ptr_frame_data->var_len;
            }
          }
        } else {
          ptr_frame_data->sin_start_for_next_top = 0;
          ptr_frame_data->sin_len_for_next_top = ptr_frame_data->var_len;
        }
      } else {
        ptr_frame_data->sin_start_for_next_top = 0;
        ptr_frame_data->sin_len_for_next_top = 0;
      }
    }
  }
  return;
}

WORD32 ixheaacd_ssc_huff_dec(ia_huffman_data_type t_huff,
                             ia_bit_buf_struct *it_bit_buff) {
  WORD32 index;
  WORD32 value, bit;
  WORD16 cw;
  index = 0;

  while (index >= 0) {
    cw = t_huff[index];

    bit = ixheaacd_read_bits_buf(it_bit_buff, 1);

    if (bit) {
      WORD sign = (cw & 0x0080);
      if (sign) {
        index = (cw | 0xffffff80);
      } else {
        index = (cw & 0x007f);
      }
    } else {
      index = (cw >> 8);
    }
  }

  value = (index + 64);

  return (value);
}

WORD32 ixheaacd_sbr_read_header_data(
    ia_sbr_header_data_struct *pstr_sbr_header, ia_bit_buf_struct *it_bit_buff,
    FLAG stereo_flag, ia_sbr_header_data_struct *pstr_sbr_dflt_header) {
  ia_sbr_header_data_struct prev_header_info;
  prev_header_info.start_freq = 0;
  prev_header_info.noise_bands = 0;
  FLAG header_extra_1 = 0, header_extra_2 = 0;
  WORD32 tmp;
  WORD32 usac_independency_flag = pstr_sbr_header->usac_independency_flag;
  WORD32 use_dflt_hdr = 0;
  WORD32 header_present = 1;
  WORD32 usac_flag = pstr_sbr_header->usac_flag;

  if (!usac_flag) {
    memcpy(&prev_header_info, pstr_sbr_header,
           sizeof(ia_sbr_header_data_struct));

    tmp = ixheaacd_read_bits_buf(
        it_bit_buff, SBR_AMPLITUDE_RESOLUTION_BITS + SBR_BEGIN_SAMP_FREQ_BITS +
                         SBR_END_SAMP_FREQ_BITS + SBR_CROSS_OVER_BND_BITS);

    pstr_sbr_header->amp_res = (WORD16)(
        (tmp & 0x0800) >> (SBR_BEGIN_SAMP_FREQ_BITS + SBR_END_SAMP_FREQ_BITS +
                           SBR_CROSS_OVER_BND_BITS));

    pstr_sbr_header->start_freq = (WORD16)(
        (tmp & 0x0780) >> (SBR_END_SAMP_FREQ_BITS + SBR_CROSS_OVER_BND_BITS));

    pstr_sbr_header->stop_freq =
        (WORD16)((tmp & 0x078) >> (SBR_CROSS_OVER_BND_BITS));

    pstr_sbr_header->xover_band = (WORD16)((tmp & 0x07));

    tmp = ixheaacd_read_bits_buf(
        it_bit_buff,
        SBR_HDR_RESERV_BITS + SBR_HDR_EXTR_1_BITS + SBR_HDR_EXTR_2_BITS);
    header_extra_1 = (FLAG)((tmp & 0x02) >> (SBR_HDR_EXTR_2_BITS));
    header_extra_2 = (FLAG)((tmp & 0x01));
    if (stereo_flag) {
      pstr_sbr_header->channel_mode = SBR_STEREO;
    } else {
      pstr_sbr_header->channel_mode = SBR_MONO;
    }
  } else {
    WORD32 info_present = 0;
    if (pstr_sbr_header->sync_state == SBR_ACTIVE) {
      memcpy(&prev_header_info, pstr_sbr_header,
             sizeof(ia_sbr_header_data_struct));
    }
    if (usac_independency_flag) {
      header_present = 1;
      info_present = 1;
    } else {
      info_present = ixheaacd_read_bits_buf(it_bit_buff, 1);
      if (info_present) {
        header_present = ixheaacd_read_bits_buf(it_bit_buff, 1);
      } else {
        header_present = 0;
      }
    }

    if (info_present) {
      tmp = ixheaacd_read_bits_buf(it_bit_buff, SBR_AMPLITUDE_RESOLUTION_BITS +
                                                    ESBR_CROSS_OVER_BND_BITS +
                                                    ESBR_PRE_FLAT_BITS);
      pstr_sbr_header->amp_res = (WORD16)(
          (tmp & 0x0020) >> (ESBR_CROSS_OVER_BND_BITS + ESBR_PRE_FLAT_BITS));
      pstr_sbr_header->xover_band =
          (WORD16)((tmp & 0x001E) >> (ESBR_PRE_FLAT_BITS));
      pstr_sbr_header->pre_proc_flag = (WORD16)((tmp & 0x001));
      if (pstr_sbr_header->pvc_flag) {
        pstr_sbr_header->pvc_mode =
            ixheaacd_read_bits_buf(it_bit_buff, ESBR_PVC_MODE_BITS);
      } else {
        pstr_sbr_header->pvc_mode = 0;
      }
    }

    if (header_present) {
      use_dflt_hdr = ixheaacd_read_bits_buf(it_bit_buff, 1);
      if (use_dflt_hdr) {
        pstr_sbr_header->start_freq = pstr_sbr_dflt_header->start_freq;
        pstr_sbr_header->stop_freq = pstr_sbr_dflt_header->stop_freq;
        pstr_sbr_header->header_extra_1 = pstr_sbr_dflt_header->header_extra_1;
        pstr_sbr_header->header_extra_2 = pstr_sbr_dflt_header->header_extra_2;
        pstr_sbr_header->freq_scale = pstr_sbr_dflt_header->freq_scale;
        pstr_sbr_header->alter_scale = pstr_sbr_dflt_header->alter_scale;
        pstr_sbr_header->noise_bands = pstr_sbr_dflt_header->noise_bands;
        pstr_sbr_header->limiter_bands = pstr_sbr_dflt_header->limiter_bands;
        pstr_sbr_header->limiter_gains = pstr_sbr_dflt_header->limiter_gains;
        pstr_sbr_header->interpol_freq = pstr_sbr_dflt_header->interpol_freq;
        pstr_sbr_header->smoothing_mode = pstr_sbr_dflt_header->smoothing_mode;
      } else {
        tmp = ixheaacd_read_bits_buf(
            it_bit_buff, SBR_BEGIN_SAMP_FREQ_BITS + SBR_END_SAMP_FREQ_BITS +
                             SBR_HDR_EXTR_1_BITS + SBR_HDR_EXTR_2_BITS);
        pstr_sbr_header->start_freq =
            (tmp & 0x03C0) >> (SBR_END_SAMP_FREQ_BITS + SBR_HDR_EXTR_1_BITS +
                               SBR_HDR_EXTR_2_BITS);
        pstr_sbr_header->stop_freq =
            (tmp & 0x003C) >> (SBR_HDR_EXTR_1_BITS + SBR_HDR_EXTR_2_BITS);
        pstr_sbr_header->header_extra_1 =
            (tmp & 0x0002) >> (SBR_HDR_EXTR_2_BITS);
        pstr_sbr_header->header_extra_2 = (tmp & 0x0001);
        header_extra_1 = pstr_sbr_header->header_extra_1;
        header_extra_2 = pstr_sbr_header->header_extra_2;
      }
    }
  }

  if (!use_dflt_hdr && header_present) {
    if (header_extra_1) {
      tmp = ixheaacd_read_bits_buf(
          it_bit_buff,
          SBR_SAMP_FREQ_LVL_BITS + SBR_CHANGE_LVL_BITS + SBR_NOISE_BND_BITS);
      pstr_sbr_header->freq_scale =
          (WORD16)((tmp & 0x018) >> (SBR_CHANGE_LVL_BITS + SBR_NOISE_BND_BITS));
      pstr_sbr_header->alter_scale =
          (WORD16)((tmp & 0x04) >> (SBR_NOISE_BND_BITS));
      pstr_sbr_header->noise_bands = (WORD16)((tmp & 0x03));
    } else {
      pstr_sbr_header->freq_scale = SBR_SAMP_FEQ_LVL_DEF;
      pstr_sbr_header->alter_scale = SBR_CHANGE_LVL_DEF;
      pstr_sbr_header->noise_bands = SBR_NOISE_BND_DEF;
    }

    if (header_extra_2) {
      tmp = ixheaacd_read_bits_buf(
          it_bit_buff, SBR_BND_LIMIT_BITS + SBR_GAIN_LIMIT_BITS +
                           SBR_INTERPOL_SAMP_FREQ_BITS + SBR_SMOOTH_LEN_BITS);
      pstr_sbr_header->limiter_bands = (WORD16)(
          (tmp & 0x030) >> (SBR_GAIN_LIMIT_BITS + SBR_INTERPOL_SAMP_FREQ_BITS +
                            SBR_SMOOTH_LEN_BITS));
      pstr_sbr_header->limiter_gains = (WORD16)(
          (tmp & 0x0c) >> (SBR_INTERPOL_SAMP_FREQ_BITS + SBR_SMOOTH_LEN_BITS));
      pstr_sbr_header->interpol_freq =
          (WORD16)((tmp & 0x02) >> (SBR_SMOOTH_LEN_BITS));
      pstr_sbr_header->smoothing_mode = (WORD16)((tmp & 0x01));
    } else {
      pstr_sbr_header->limiter_bands = SBR_BND_LIMIT_DEF;
      pstr_sbr_header->limiter_gains = SBR_GAIN_LIMIT_DEF;
      pstr_sbr_header->interpol_freq = SBR_INTERPOL_SAMP_FEQ_DEF;
      pstr_sbr_header->smoothing_mode = SBR_SMOOTH_LEN_DEF;
    }
  }

  if ((pstr_sbr_header->sync_state != SBR_ACTIVE) ||
      (prev_header_info.start_freq != pstr_sbr_header->start_freq) ||
      (prev_header_info.stop_freq != pstr_sbr_header->stop_freq) ||
      (prev_header_info.xover_band != pstr_sbr_header->xover_band) ||
      (prev_header_info.freq_scale != pstr_sbr_header->freq_scale) ||
      (prev_header_info.alter_scale != pstr_sbr_header->alter_scale) ||
      (prev_header_info.noise_bands != pstr_sbr_header->noise_bands)) {
    return SBR_RESET;
  }

  return 0;
}

static VOID ixheaacd_sbr_sin_coding_data(
    ia_sbr_header_data_struct *ptr_header_data,
    ia_sbr_frame_info_data_struct *ptr_frame_data,
    ia_bit_buf_struct *it_bit_buff) {
  FLAG *p_add_harmonic = ptr_frame_data->add_harmonics;
  WORD32 i;

  i = ptr_header_data->pstr_freq_band_data->num_sf_bands[HIGH];
  do {
    *p_add_harmonic++ =
        (FLAG)ixheaacd_read_bits_buf(it_bit_buff, SBR_ADD_SINE_FLAG_BITS);
    i--;
  } while (i != 0);

  return;
}

static WORD16 ixheaacd_validate_frame_info(
    ia_frame_info_struct *pstr_frame_info, WORD16 num_time_slots,
    WORD audio_object_type) {
  WORD32 i, j;

  WORD32 start_pos, end_pos, transient_env, start_pos_noise, end_pos_noise,
      num_env_sf, num_noise_env;

  num_env_sf = pstr_frame_info->num_env;
  num_noise_env = pstr_frame_info->num_noise_env;

  if ((num_env_sf < 1) || (num_env_sf > MAX_ENVELOPES)) return 0;

  if (num_noise_env > MAX_NOISE_ENVELOPES) return 0;

  start_pos = pstr_frame_info->border_vec[0];
  end_pos = pstr_frame_info->border_vec[num_env_sf];
  transient_env = pstr_frame_info->transient_env;

  if (transient_env > num_env_sf) return 0;

  start_pos_noise = pstr_frame_info->noise_border_vec[0];
  end_pos_noise = pstr_frame_info->noise_border_vec[num_noise_env];

  if ((start_pos < 0) || (start_pos >= end_pos)) return 0;

  if (start_pos > SBR_OV_SLOTS) return 0;
  if (audio_object_type != AOT_ER_AAC_ELD &&
      audio_object_type != AOT_ER_AAC_LD) {
    if (num_time_slots != 15) {
      if (end_pos < SBR_TIME_SLOTS) return 0;
    } else {
      if (end_pos < num_time_slots) return 0;
    }
  } else {
    if (end_pos < num_time_slots) return 0;
  }

  if (num_time_slots != 15) {
    if (end_pos > add_d(SBR_TIME_SLOTS, SBR_OV_SLOTS)) return 0;
  } else {
    if (end_pos > add_d(num_time_slots, SBR_OV_SLOTS)) return 0;
  }

  for (i = 0; i < num_env_sf; i++) {
    if (pstr_frame_info->border_vec[i] > pstr_frame_info->border_vec[i + 1])
      return 0;
  }

  if ((num_env_sf == 1) && (num_noise_env > 1)) return 0;

  if ((start_pos != start_pos_noise) || (end_pos != end_pos_noise)) return 0;

  for (i = 0; i < num_noise_env; i++) {
    start_pos_noise = pstr_frame_info->noise_border_vec[i];

    for (j = 0; j < num_env_sf; j++) {
      if (pstr_frame_info->border_vec[j] == start_pos_noise) break;
    }
    if (j == num_env_sf) return 0;
  }

  return 1;
}

static WORD16 ixheaacd_read_enh_sbr_data(
    ia_sbr_header_data_struct *ptr_header_data,
    ia_bit_buf_struct *it_bit_buff,
    VOID *p_frame_data,
    WORD32 ele_id) {
  WORD32 tmp = 0;
  WORD16 num_bits_read = 0;
  tmp = ixheaacd_read_bits_buf(it_bit_buff, ESBR_PRE_FLAT_BITS);
  ptr_header_data->pre_proc_flag = tmp;
  num_bits_read += ESBR_PRE_FLAT_BITS;

  if (ele_id == SBR_ID_SCE) {
    ia_sbr_frame_info_data_struct *ptr_frame_data =
        (ia_sbr_frame_info_data_struct *)p_frame_data;

    tmp = ixheaacd_read_bits_buf(it_bit_buff, ESBR_PATCHING_MODE_BITS);
    ptr_frame_data->sbr_patching_mode = tmp;
    num_bits_read += ESBR_PATCHING_MODE_BITS;

    if (tmp == 0) {
      tmp = ixheaacd_read_bits_buf(it_bit_buff, ESBR_OVERSAMPLING_FLAG_BITS);
      ptr_frame_data->over_sampling_flag = tmp;
      num_bits_read += ESBR_OVERSAMPLING_FLAG_BITS;

      tmp = ixheaacd_read_bits_buf(it_bit_buff, ESBR_PITCHIN_FLAG_BITS);
      num_bits_read += ESBR_PITCHIN_FLAG_BITS;

      if (tmp) {
        tmp =
           ixheaacd_read_bits_buf(it_bit_buff, ESBR_PITCHIN_BINS_BITS);
        ptr_frame_data->pitch_in_bins = tmp;
        num_bits_read += ESBR_PITCHIN_BINS_BITS;
      } else {
        ptr_frame_data->pitch_in_bins = 0;
      }
    } else {
      ptr_frame_data->over_sampling_flag = 0;
      ptr_frame_data->pitch_in_bins = 0;
    }
  } else if (ele_id == SBR_ID_CPE) {
    ia_sbr_frame_info_data_struct **ptr_frame_data =
        (ia_sbr_frame_info_data_struct **)p_frame_data;
    if (ptr_frame_data[0]->coupling_mode) {
      ptr_frame_data[0]->sbr_patching_mode =
          ptr_frame_data[1]->sbr_patching_mode =
          ixheaacd_read_bits_buf(it_bit_buff, ESBR_PATCHING_MODE_BITS);
      num_bits_read += ESBR_PATCHING_MODE_BITS;

      if (ptr_frame_data[0]->sbr_patching_mode == 0) {
        ptr_frame_data[0]->over_sampling_flag =
            ptr_frame_data[1]->over_sampling_flag =
            ixheaacd_read_bits_buf(it_bit_buff, ESBR_OVERSAMPLING_FLAG_BITS);
        num_bits_read += ESBR_OVERSAMPLING_FLAG_BITS;
        num_bits_read += ESBR_PITCHIN_FLAG_BITS;
        if (ixheaacd_read_bits_buf(it_bit_buff, ESBR_PITCHIN_FLAG_BITS)) {
          ptr_frame_data[0]->pitch_in_bins =
          ptr_frame_data[1]->pitch_in_bins =
              ixheaacd_read_bits_buf(it_bit_buff, ESBR_PITCHIN_BINS_BITS);
          num_bits_read += ESBR_PITCHIN_BINS_BITS;
        } else {
          ptr_frame_data[0]->pitch_in_bins = 0;
          ptr_frame_data[1]->pitch_in_bins = 0;
        }
      } else {
        ptr_frame_data[0]->over_sampling_flag = 0;
        ptr_frame_data[0]->pitch_in_bins = 0;

        ptr_frame_data[1]->over_sampling_flag = 0;
        ptr_frame_data[1]->pitch_in_bins = 0;
      }
    } else {
      ptr_frame_data[0]->sbr_patching_mode =
          ixheaacd_read_bits_buf(it_bit_buff, ESBR_PATCHING_MODE_BITS);
      num_bits_read += ESBR_PATCHING_MODE_BITS;

      if (ptr_frame_data[0]->sbr_patching_mode == 0) {
        ptr_frame_data[0]->over_sampling_flag =
            ixheaacd_read_bits_buf(it_bit_buff, ESBR_OVERSAMPLING_FLAG_BITS);
        num_bits_read += ESBR_OVERSAMPLING_FLAG_BITS;
        num_bits_read += ESBR_PITCHIN_FLAG_BITS;
        if (ixheaacd_read_bits_buf(it_bit_buff, ESBR_PITCHIN_FLAG_BITS)) {
          ptr_frame_data[0]->pitch_in_bins =
              ixheaacd_read_bits_buf(it_bit_buff, ESBR_PITCHIN_BINS_BITS);
          num_bits_read += ESBR_PITCHIN_BINS_BITS;
        } else {
          ptr_frame_data[0]->pitch_in_bins = 0;
        }
      } else {
        ptr_frame_data[0]->over_sampling_flag = 0;
        ptr_frame_data[0]->pitch_in_bins = 0;
      }

      ptr_frame_data[1]->sbr_patching_mode =
          ixheaacd_read_bits_buf(it_bit_buff, ESBR_PATCHING_MODE_BITS);
      num_bits_read += ESBR_PATCHING_MODE_BITS;

      if (ptr_frame_data[1]->sbr_patching_mode == 0) {
        ptr_frame_data[1]->over_sampling_flag =
            ixheaacd_read_bits_buf(it_bit_buff, ESBR_OVERSAMPLING_FLAG_BITS);
        num_bits_read += ESBR_OVERSAMPLING_FLAG_BITS;
        num_bits_read += ESBR_PITCHIN_FLAG_BITS;
        if (ixheaacd_read_bits_buf(it_bit_buff, ESBR_PITCHIN_FLAG_BITS)) {
          ptr_frame_data[1]->pitch_in_bins =
              ixheaacd_read_bits_buf(it_bit_buff, ESBR_PITCHIN_BINS_BITS);
          num_bits_read += ESBR_PITCHIN_BINS_BITS;
        } else {
          ptr_frame_data[1]->pitch_in_bins = 0;
        }
      } else {
        ptr_frame_data[1]->over_sampling_flag =
            ptr_frame_data[1]->pitch_in_bins = 0;
      }
    }
  }
  if (num_bits_read < 6) {
    ixheaacd_read_bits_buf(it_bit_buff, 6 - num_bits_read);
    num_bits_read = 6;
  }
  return num_bits_read;
}

static IA_ERRORCODE ixheaacd_read_extn_data(ia_sbr_header_data_struct *ptr_header_data,
                                            ia_ps_dec_struct *ptr_ps_dec,
                                            ia_bit_buf_struct *it_bit_buff,
                                            ia_ps_tables_struct *ps_tables_ptr,
                                            VOID *p_frame_data, WORD32 ele_id) {
  WORD i;
  WORD extended_data;
  WORD no_bits_left;

  extended_data = ixheaacd_read_bits_buf(it_bit_buff, SBR_ENLARGED_DATA_BITS);

  if (extended_data) {
    WORD cnt;
    FLAG ps_read;

    ps_read = 0;

    cnt = ixheaacd_read_bits_buf(it_bit_buff, SBR_CONT_SIZE_BITS);

    if (cnt == ((1 << SBR_CONT_SIZE_BITS) - 1)) {
      cnt = (cnt + ixheaacd_read_bits_buf(it_bit_buff, SBR_CONT_ESC_CNT_BITS));
    }

    no_bits_left = (cnt << 3);

    ptr_header_data->hbe_flag = !ptr_header_data->usac_flag;
    ptr_header_data->sbr_ratio_idx = SBR_UPSAMPLE_IDX_2_1;

    while (no_bits_left > 7) {
      WORD extension_id = ixheaacd_read_bits_buf(it_bit_buff, SBR_CONT_ID_BITS);

      if (extension_id == EXTENSION_ID_ENHSBR_CODING && !ptr_header_data->enh_sbr)
      {
        extension_id = -1;
      }
      no_bits_left = (no_bits_left - SBR_CONT_ID_BITS);

      switch (extension_id) {
        case EXTENSION_ID_PS_CODING:

          if (ptr_ps_dec == NULL) {
            return 0;
          }

          if (!(ptr_ps_dec->force_mono || ps_read)) {
            IA_ERRORCODE ret_val = ixheaacd_read_ps_data(ptr_ps_dec, it_bit_buff,
                                                         (WORD16)no_bits_left, ps_tables_ptr);
            if (ret_val == IA_FATAL_ERROR) {
              return ret_val;
            } else {
              no_bits_left = no_bits_left - ret_val;
            }

            if (no_bits_left < 0) return 0;
            ptr_header_data->channel_mode = PS_STEREO;
            ps_read = 1;
            break;
          }
        case EXTENSION_ID_ENHSBR_CODING: {
          no_bits_left =
              (no_bits_left - ixheaacd_read_enh_sbr_data(ptr_header_data, it_bit_buff,
                  p_frame_data, ele_id));

          ptr_header_data->hbe_flag = 1;
          ptr_header_data->sbr_ratio_idx = SBR_UPSAMPLE_IDX_2_1;
          break;
        }
        default:
          cnt = (no_bits_left >> 3);
          for (i = cnt - 1; i >= 0; i--) ixheaacd_read_bits_buf(it_bit_buff, 8);
          no_bits_left = (no_bits_left - (cnt << 3));
          break;
      }
    }

    if (no_bits_left < 0) return 0;
    ixheaacd_read_bits_buf(it_bit_buff, no_bits_left);
  }
  return 0;
}

WORD32 ixheaacd_sbr_read_pvc_sce(ia_sbr_frame_info_data_struct *ptr_frame_data,
                                 ia_bit_buf_struct *it_bit_buff,
                                 WORD32 hbe_flag,
                                 ia_pvc_data_struct *ptr_pvc_data,
                                 ia_sbr_tables_struct *ptr_sbr_tables,
                                 ia_sbr_header_data_struct *ptr_header_data) {
  WORD32 i;
  WORD32 err_code = 0;
  ia_env_extr_tables_struct *env_extr_tables_ptr =
      ptr_sbr_tables->env_extr_tables_ptr;
  WORD32 usac_independency_flag = ptr_frame_data->usac_independency_flag;

  if (hbe_flag) {
    ptr_frame_data->sbr_patching_mode =
        ixheaacd_read_bits_buf(it_bit_buff, ESBR_PATCHING_MODE_BITS);

    if (ptr_frame_data->sbr_patching_mode == 0) {
      ptr_frame_data->over_sampling_flag =
          ixheaacd_read_bits_buf(it_bit_buff, ESBR_OVERSAMPLING_FLAG_BITS);
      if (ixheaacd_read_bits_buf(it_bit_buff, ESBR_PITCHIN_FLAG_BITS))
        ptr_frame_data->pitch_in_bins =
            ixheaacd_read_bits_buf(it_bit_buff, ESBR_PITCHIN_BINS_BITS);
      else
        ptr_frame_data->pitch_in_bins = 0;
    } else {
      ptr_frame_data->over_sampling_flag = ptr_frame_data->pitch_in_bins = 0;
    }
  }

  err_code = ixheaacd_pvc_time_freq_grid_info(it_bit_buff, ptr_frame_data);
  if (err_code) return err_code;

  ptr_pvc_data->prev_sbr_mode = PVC_SBR;

  ixheaacd_pvc_env_dtdf_data(ptr_frame_data, it_bit_buff);

  for (i = 0; i < ptr_header_data->pstr_freq_band_data->num_nf_bands; i++) {
    ptr_frame_data->sbr_invf_mode_prev[i] = ptr_frame_data->sbr_invf_mode[i];
    ptr_frame_data->sbr_invf_mode[i] =
        ixheaacd_read_bits_buf(it_bit_buff, ESBR_INVF_MODE_BITS);
  }

  ptr_pvc_data->pvc_mode = ptr_header_data->pvc_mode;

  err_code = ixheaacd_read_esbr_pvc_envelope(ptr_pvc_data, it_bit_buff,
                                             usac_independency_flag);
  if (err_code) return err_code;

  ixheaacd_read_sbr_noise_floor_data(ptr_header_data, ptr_frame_data,
                                     it_bit_buff, env_extr_tables_ptr);

  memset(ptr_frame_data->add_harmonics, 0,
         ptr_header_data->pstr_freq_band_data->num_sf_bands[HIGH] *
             sizeof(WORD32));
  ptr_frame_data->pvc_mode = ptr_header_data->pvc_mode;

  ixheaacd_read_sbr_addi_data(ptr_frame_data, ptr_header_data, it_bit_buff);

  ptr_frame_data->coupling_mode = COUPLING_OFF;

  return err_code;
}

IA_ERRORCODE ixheaacd_sbr_read_sce(ia_sbr_header_data_struct *ptr_header_data,
                                   ia_sbr_frame_info_data_struct *ptr_frame_data,
                                   ia_ps_dec_struct *ptr_ps_dec, ia_bit_buf_struct *it_bit_buff,
                                   ia_sbr_tables_struct *ptr_sbr_tables, WORD audio_object_type,
                                   WORD32 ec_flag) {
  WORD32 bit;
  WORD32 i;
  WORD32 hbe_flag = ptr_header_data->hbe_flag;
  WORD32 num_if_bands = ptr_header_data->pstr_freq_band_data->num_if_bands;
  WORD32 usac_flag = ptr_header_data->usac_flag;
  ia_env_extr_tables_struct *env_extr_tables_ptr =
      ptr_sbr_tables->env_extr_tables_ptr;
  IA_ERRORCODE err = IA_NO_ERROR;

  ptr_frame_data->coupling_mode = COUPLING_OFF;

  if (!usac_flag) {
    bit = ixheaacd_read_bits_buf(it_bit_buff, 1);

    if (bit) ixheaacd_read_bits_buf(it_bit_buff, SBR_SCE_RESERV_BITS);
    if (audio_object_type == AOT_ER_AAC_ELD ||
        audio_object_type == AOT_ER_AAC_LD) {
      if (ptr_frame_data->eld_sbr_flag == 1) {
        err = ixheaacd_extract_frame_info_ld(it_bit_buff, ptr_frame_data);
        if (err) return err;
      }
    } else {
      if (!ixheaacd_sbr_time_freq_grid_info(it_bit_buff, ptr_frame_data,
                                            env_extr_tables_ptr,
                                            ptr_header_data->num_time_slots))

        return 0;
    }
    if (!ixheaacd_validate_frame_info(&ptr_frame_data->str_frame_info_details,
                                      ptr_header_data->num_time_slots,
                                      audio_object_type))
      return 0;

  } else {
    if (hbe_flag) {
      ptr_frame_data->sbr_patching_mode =
          ixheaacd_read_bits_buf(it_bit_buff, ESBR_PATCHING_MODE_BITS);
      if (ptr_frame_data->sbr_patching_mode == 0) {
        ptr_frame_data->over_sampling_flag =
            ixheaacd_read_bits_buf(it_bit_buff, ESBR_OVERSAMPLING_FLAG_BITS);
        if (ixheaacd_read_bits_buf(it_bit_buff, ESBR_PITCHIN_FLAG_BITS))
          ptr_frame_data->pitch_in_bins =
              ixheaacd_read_bits_buf(it_bit_buff, ESBR_PITCHIN_BINS_BITS);
        else
          ptr_frame_data->pitch_in_bins = 0;
      } else {
        ptr_frame_data->over_sampling_flag = ptr_frame_data->pitch_in_bins = 0;
      }
    }
    ptr_frame_data->num_time_slots = ptr_header_data->num_time_slots;
    if (!ixheaacd_sbr_time_freq_grid_info(it_bit_buff, ptr_frame_data,
                                          env_extr_tables_ptr,
                                          ptr_header_data->num_time_slots))
      return 0;

    if (!ixheaacd_validate_frame_info(&ptr_frame_data->str_frame_info_details,
                                      ptr_header_data->num_time_slots,
                                      audio_object_type))
      return 0;

    ptr_frame_data->prev_sbr_mode = ORIG_SBR;
  }

  ixheaacd_sbr_env_dtdf_data(ptr_frame_data, it_bit_buff,
                             ptr_header_data->usac_flag);

  if (ptr_frame_data->del_cod_dir_arr[0] == DTDF_DIR_FREQ) {
    ptr_header_data->err_flag = 0;
  }

  for (i = 0; i < num_if_bands; i++) {
    ptr_frame_data->sbr_invf_mode_prev[i] = ptr_frame_data->sbr_invf_mode[i];
    ptr_frame_data->sbr_invf_mode[i] =
        ixheaacd_read_bits_buf(it_bit_buff, SBR_INVERSE_FILT_MODE_BITS);
  }

  if (!ixheaacd_read_sbr_env_data(ptr_header_data, ptr_frame_data, it_bit_buff,
                                  env_extr_tables_ptr, audio_object_type))
    return 0;

  ixheaacd_read_sbr_noise_floor_data(ptr_header_data, ptr_frame_data,
                                     it_bit_buff, env_extr_tables_ptr);

  if (usac_flag) {
    memset(
        ptr_frame_data->add_harmonics, 0,
        ptr_header_data->pstr_freq_band_data->num_sf_bands[1] * sizeof(WORD32));
    ptr_frame_data->coupling_mode = COUPLING_OFF;
  }

  bit = (FLAG)ixheaacd_read_bits_buf(it_bit_buff, 1);
  if (bit) {
    ixheaacd_sbr_sin_coding_data(ptr_header_data, ptr_frame_data, it_bit_buff);
  } else {
    memset(ptr_frame_data->add_harmonics, 0, sizeof(FLAG) * MAX_FREQ_COEFFS);
  }

  if (!usac_flag) {
    IA_ERRORCODE err =
        ixheaacd_read_extn_data(ptr_header_data, ptr_ps_dec, it_bit_buff,
                                ptr_sbr_tables->ps_tables_ptr, ptr_frame_data, SBR_ID_SCE);
    if (err == IA_FATAL_ERROR) {
      if (ec_flag)
        return 0;
      else
        return err;
    }
  }

  return 1;
}

IA_ERRORCODE ixheaacd_sbr_read_cpe(
    ia_sbr_header_data_struct *ptr_header_data,
    ia_sbr_frame_info_data_struct **ptr_frame_data,
    ia_bit_buf_struct *it_bit_buff, ia_sbr_tables_struct *ptr_sbr_tables,
    WORD audio_object_type) {
  WORD32 i, k, bit, num_ch = 2;
  WORD32 num_if_bands = ptr_header_data->pstr_freq_band_data->num_if_bands;
  WORD32 hbe_flag = ptr_header_data->hbe_flag;
  WORD32 usac_flag = ptr_header_data->usac_flag;
  IA_ERRORCODE err = IA_NO_ERROR;
  ia_env_extr_tables_struct *env_extr_tables_ptr =
      ptr_sbr_tables->env_extr_tables_ptr;
  bit = ixheaacd_read_bits_buf(it_bit_buff, 1);

  if (usac_flag) {
    if (bit) {
      if (hbe_flag) {
        ptr_frame_data[0]->sbr_patching_mode =
            ptr_frame_data[1]->sbr_patching_mode =
                ixheaacd_read_bits_buf(it_bit_buff, ESBR_PATCHING_MODE_BITS);
        if (ptr_frame_data[0]->sbr_patching_mode == 0) {
          ptr_frame_data[0]->over_sampling_flag =
              ptr_frame_data[1]->over_sampling_flag = ixheaacd_read_bits_buf(
                  it_bit_buff, ESBR_OVERSAMPLING_FLAG_BITS);
          if (ixheaacd_read_bits_buf(it_bit_buff, ESBR_PITCHIN_FLAG_BITS))
            ptr_frame_data[0]->pitch_in_bins =
                ptr_frame_data[1]->pitch_in_bins =
                    ixheaacd_read_bits_buf(it_bit_buff, ESBR_PITCHIN_BINS_BITS);
          else
            ptr_frame_data[0]->pitch_in_bins =
                ptr_frame_data[1]->pitch_in_bins = 0;
        } else {
          ptr_frame_data[0]->over_sampling_flag = 0;
          ptr_frame_data[1]->over_sampling_flag = 0;
          ptr_frame_data[0]->pitch_in_bins = 0;
          ptr_frame_data[1]->pitch_in_bins = 0;
        }
      }
      ptr_frame_data[0]->coupling_mode = COUPLING_LEVEL;
      ptr_frame_data[1]->coupling_mode = COUPLING_BAL;
    } else {
      if (hbe_flag) {
        ptr_frame_data[0]->sbr_patching_mode =
            ixheaacd_read_bits_buf(it_bit_buff, ESBR_PATCHING_MODE_BITS);
        if (ptr_frame_data[0]->sbr_patching_mode == 0) {
          ptr_frame_data[0]->over_sampling_flag =
              ixheaacd_read_bits_buf(it_bit_buff, ESBR_OVERSAMPLING_FLAG_BITS);
          if (ixheaacd_read_bits_buf(it_bit_buff, ESBR_PITCHIN_FLAG_BITS))
            ptr_frame_data[0]->pitch_in_bins =
                ixheaacd_read_bits_buf(it_bit_buff, ESBR_PITCHIN_BINS_BITS);
          else
            ptr_frame_data[0]->pitch_in_bins = 0;
        } else {
          ptr_frame_data[0]->over_sampling_flag = 0;
          ptr_frame_data[0]->pitch_in_bins = 0;
        }
        ptr_frame_data[1]->sbr_patching_mode =
            ixheaacd_read_bits_buf(it_bit_buff, ESBR_PATCHING_MODE_BITS);
        if (ptr_frame_data[1]->sbr_patching_mode == 0) {
          ptr_frame_data[1]->over_sampling_flag =
              ixheaacd_read_bits_buf(it_bit_buff, ESBR_OVERSAMPLING_FLAG_BITS);
          if (ixheaacd_read_bits_buf(it_bit_buff, ESBR_PITCHIN_FLAG_BITS))
            ptr_frame_data[1]->pitch_in_bins =
                ixheaacd_read_bits_buf(it_bit_buff, ESBR_PITCHIN_BINS_BITS);
          else
            ptr_frame_data[1]->pitch_in_bins = 0;
        } else {
          ptr_frame_data[1]->over_sampling_flag =
              ptr_frame_data[1]->pitch_in_bins = 0;
        }
      }

      ptr_frame_data[0]->coupling_mode = COUPLING_OFF;
      ptr_frame_data[1]->coupling_mode = COUPLING_OFF;
    }
  } else {
    if (bit) {
      ixheaacd_read_bits_buf(it_bit_buff,
                             SBR_SCE_RESERV_BITS + SBR_SCE_RESERV_BITS);
    }
    if ((audio_object_type != AOT_ER_AAC_ELD) &&
        (ptr_header_data->channel_mode != SBR_STEREO)) {
      ptr_header_data->sync_state = UPSAMPLING;
      return 0;
    }

    bit = ixheaacd_read_bits_buf(it_bit_buff, SBR_COUPLNG_MODE_BITS);

    if (bit) {
      ptr_frame_data[0]->coupling_mode = COUPLING_LEVEL;
      ptr_frame_data[1]->coupling_mode = COUPLING_BAL;
    } else {
      ptr_frame_data[0]->coupling_mode = COUPLING_OFF;
      ptr_frame_data[1]->coupling_mode = COUPLING_OFF;
    }
  }

  for (i = 0; i < num_ch; i++) {
    ptr_frame_data[i]->num_time_slots = ptr_header_data->num_time_slots;
    if (audio_object_type == AOT_ER_AAC_ELD ||
        audio_object_type == AOT_ER_AAC_LD) {
      if (ptr_frame_data[i]->eld_sbr_flag == 1) {
        err = ixheaacd_extract_frame_info_ld(it_bit_buff, ptr_frame_data[i]);
        if (err) return err;
      }
    } else {
      if (!ixheaacd_sbr_time_freq_grid_info(it_bit_buff, ptr_frame_data[i],
                                            env_extr_tables_ptr,
                                            ptr_header_data->num_time_slots))
        return 0;
    }

    if (!ixheaacd_validate_frame_info(
            &ptr_frame_data[i]->str_frame_info_details,
            ptr_header_data->num_time_slots, audio_object_type))
      return 0;

    if (ptr_frame_data[0]->coupling_mode) {
      memcpy(&ptr_frame_data[1]->str_frame_info_details,
             &ptr_frame_data[0]->str_frame_info_details,
             sizeof(ia_frame_info_struct));
      if (audio_object_type == AOT_ER_AAC_ELD ||
          audio_object_type == AOT_ER_AAC_LD) {
        ptr_frame_data[1]->amp_res = ptr_frame_data[0]->amp_res;
      }
      num_ch = 1;
    }
  }

  if (ptr_frame_data[0]->coupling_mode && usac_flag) {
    ixheaacd_sbr_env_dtdf_data(ptr_frame_data[0], it_bit_buff,
                               ptr_header_data->usac_flag);
    ixheaacd_sbr_env_dtdf_data(ptr_frame_data[1], it_bit_buff,
                               ptr_header_data->usac_flag);

    for (i = 0; i < ptr_header_data->pstr_freq_band_data->num_if_bands; i++) {
      ptr_frame_data[0]->sbr_invf_mode_prev[i] =
          ptr_frame_data[0]->sbr_invf_mode[i];
      ptr_frame_data[1]->sbr_invf_mode_prev[i] =
          ptr_frame_data[1]->sbr_invf_mode[i];

      ptr_frame_data[0]->sbr_invf_mode[i] =
          ixheaacd_read_bits_buf(it_bit_buff, ESBR_INVF_MODE_BITS);
      ptr_frame_data[1]->sbr_invf_mode[i] = ptr_frame_data[0]->sbr_invf_mode[i];
    }

    if (!ixheaacd_read_sbr_env_data(ptr_header_data, ptr_frame_data[0],
                                    it_bit_buff, env_extr_tables_ptr,
                                    audio_object_type)) {
      return 0;
    }
    ixheaacd_read_sbr_noise_floor_data(ptr_header_data, ptr_frame_data[0],
                                       it_bit_buff, env_extr_tables_ptr);

    if (!ixheaacd_read_sbr_env_data(ptr_header_data, ptr_frame_data[1],
                                    it_bit_buff, env_extr_tables_ptr,
                                    audio_object_type)) {
      return 0;
    }
    ixheaacd_read_sbr_noise_floor_data(ptr_header_data, ptr_frame_data[1],
                                       it_bit_buff, env_extr_tables_ptr);

    memset(
        ptr_frame_data[0]->add_harmonics, 0,
        ptr_header_data->pstr_freq_band_data->num_sf_bands[1] * sizeof(WORD32));
    memset(
        ptr_frame_data[1]->add_harmonics, 0,
        ptr_header_data->pstr_freq_band_data->num_sf_bands[1] * sizeof(WORD32));

  } else {
    ixheaacd_sbr_env_dtdf_data(ptr_frame_data[0], it_bit_buff,
                               ptr_header_data->usac_flag);
    ixheaacd_sbr_env_dtdf_data(ptr_frame_data[1], it_bit_buff,
                               ptr_header_data->usac_flag);

    if ((ptr_frame_data[0]->del_cod_dir_arr[0] == DTDF_DIR_FREQ) &&
        (ptr_frame_data[1]->del_cod_dir_arr[0] == DTDF_DIR_FREQ)) {
      ptr_header_data->err_flag = 0;
    }

    for (k = 0; k < num_ch; k++) {
      for (i = 0; i < num_if_bands; i++) {
        ptr_frame_data[k]->sbr_invf_mode_prev[i] =
            ptr_frame_data[k]->sbr_invf_mode[i];
        ptr_frame_data[k]->sbr_invf_mode[i] = ixheaacd_read_bits_buf(
            it_bit_buff, SBR_INVERSE_FILT_MODE_BITS);
      }
    }

    if (ptr_frame_data[0]->coupling_mode) {
      memcpy(ptr_frame_data[1]->sbr_invf_mode_prev, ptr_frame_data[1]->sbr_invf_mode,
             sizeof(ptr_frame_data[1]->sbr_invf_mode_prev[0]) * num_if_bands);
      memcpy(ptr_frame_data[1]->sbr_invf_mode, ptr_frame_data[0]->sbr_invf_mode,
             sizeof(WORD32) * num_if_bands);

      if (!ixheaacd_read_sbr_env_data(ptr_header_data, ptr_frame_data[0],
                                      it_bit_buff, env_extr_tables_ptr,
                                      audio_object_type)) {
        return 0;
      }

      ixheaacd_read_sbr_noise_floor_data(ptr_header_data, ptr_frame_data[0],
                                         it_bit_buff, env_extr_tables_ptr);

      if (!ixheaacd_read_sbr_env_data(ptr_header_data, ptr_frame_data[1],
                                      it_bit_buff, env_extr_tables_ptr,
                                      audio_object_type)) {
        return 0;
      }
    } else {
      if (!ixheaacd_read_sbr_env_data(ptr_header_data, ptr_frame_data[0],
                                      it_bit_buff, env_extr_tables_ptr,
                                      audio_object_type))
        return 0;

      if (!ixheaacd_read_sbr_env_data(ptr_header_data, ptr_frame_data[1],
                                      it_bit_buff, env_extr_tables_ptr,
                                      audio_object_type))
        return 0;

      ixheaacd_read_sbr_noise_floor_data(ptr_header_data, ptr_frame_data[0],
                                         it_bit_buff, env_extr_tables_ptr);
    }
    ixheaacd_read_sbr_noise_floor_data(ptr_header_data, ptr_frame_data[1],
                                       it_bit_buff, env_extr_tables_ptr);
  }

  bit = (FLAG)ixheaacd_read_bits_buf(it_bit_buff, 1);
  if (bit) {
    ixheaacd_sbr_sin_coding_data(ptr_header_data, ptr_frame_data[0],
                                 it_bit_buff);
  } else {
    memset(ptr_frame_data[0]->add_harmonics, 0, sizeof(FLAG) * MAX_FREQ_COEFFS);
  }

  bit = (FLAG)ixheaacd_read_bits_buf(it_bit_buff, 1);
  if (bit) {
    ixheaacd_sbr_sin_coding_data(ptr_header_data, ptr_frame_data[1],
                                 it_bit_buff);
  } else {
    memset(ptr_frame_data[1]->add_harmonics, 0, sizeof(FLAG) * MAX_FREQ_COEFFS);
  }

  if (!usac_flag) {
    IA_ERRORCODE err =
        ixheaacd_read_extn_data(ptr_header_data, NULL, it_bit_buff, ptr_sbr_tables->ps_tables_ptr,
                                (VOID *)ptr_frame_data, SBR_ID_CPE);
    if (err == IA_FATAL_ERROR) {
      return err;
    }
  }
  return 1;
}

VOID ixheaacd_sbr_env_dtdf_data(ia_sbr_frame_info_data_struct *ptr_frame_data,
                                ia_bit_buf_struct *it_bit_buff,
                                WORD32 usac_flag) {
  WORD32 i;
  WORD32 num_env = ptr_frame_data->str_frame_info_details.num_env;
  WORD32 num_noise_env = ptr_frame_data->str_frame_info_details.num_noise_env;
  WORD16 *p_coding_dir_vec = ptr_frame_data->del_cod_dir_arr;
  WORD16 *p_coding_dir_noise_vec = ptr_frame_data->del_cod_dir_noise_arr;
  WORD32 usac_independency_flag = ptr_frame_data->usac_independency_flag;

  if (usac_flag) {
    if (usac_independency_flag) {
      *p_coding_dir_vec = 0;
      p_coding_dir_vec++;
    } else {
      *p_coding_dir_vec =
          (WORD16)ixheaacd_read_bits_buf(it_bit_buff, SBR_DEL_COD_DIR_BITS);
      p_coding_dir_vec++;
    }
    for (i = num_env - 1; i >= 1; i--) {
      *p_coding_dir_vec++ =
          (WORD16)ixheaacd_read_bits_buf(it_bit_buff, SBR_DEL_COD_DIR_BITS);
    }
    if (usac_independency_flag) {
      *p_coding_dir_noise_vec = 0;
      p_coding_dir_noise_vec++;
    } else {
      *p_coding_dir_noise_vec =
          (WORD16)ixheaacd_read_bits_buf(it_bit_buff, SBR_DEL_COD_DIR_BITS);
      p_coding_dir_noise_vec++;
    }
    for (i = num_noise_env - 1; i >= 1; i--) {
      *p_coding_dir_noise_vec++ =
          (WORD16)ixheaacd_read_bits_buf(it_bit_buff, SBR_DEL_COD_DIR_BITS);
    }
  } else {
    for (i = num_env - 1; i >= 0; i--) {
      *p_coding_dir_vec++ =
          (WORD16)ixheaacd_read_bits_buf(it_bit_buff, SBR_DEL_COD_DIR_BITS);
    }

    for (i = num_noise_env - 1; i >= 0; i--) {
      *p_coding_dir_noise_vec++ =
          (WORD16)ixheaacd_read_bits_buf(it_bit_buff, SBR_DEL_COD_DIR_BITS);
    }
  }
}

VOID ixheaacd_read_env_data(ia_sbr_frame_info_data_struct *ptr_frame_data,
                            ia_bit_buf_struct *it_bit_buff,
                            ia_huffman_data_type hcb_t,
                            ia_huffman_data_type hcb_f, WORD32 *idx_t,
                            WORD32 *idx_f, WORD16 *no_band, WORD32 num_env,
                            WORD32 env_data_tbl_comp_factor, WORD32 start_bits,
                            WORD32 start_bits_balance, WORD32 num_noise_env,
                            WORD32 lav, WORD32 usac_flag) {
  WORD32 j, i, ixheaacd_drc_offset = 0,
               coupling_mode = ptr_frame_data->coupling_mode, delta, bits,
               shift;
  WORD16 *p_coding_dir_vec, *p_sbr_sf;
  WORD16 index, length;
  WORD32 readword;
  FLOAT32 *p_sbr_sf_float;

  if (num_noise_env) {
    p_coding_dir_vec = ptr_frame_data->del_cod_dir_noise_arr;
    p_sbr_sf = ptr_frame_data->int_noise_floor;
    p_sbr_sf_float = ptr_frame_data->flt_noise_floor;
  } else {
    p_coding_dir_vec = ptr_frame_data->del_cod_dir_arr;
    p_sbr_sf = ptr_frame_data->int_env_sf_arr;
    p_sbr_sf_float = ptr_frame_data->flt_env_sf_arr;
  }

  if (coupling_mode == COUPLING_BAL) {
    bits = start_bits_balance;
    shift = env_data_tbl_comp_factor;

  } else {
    bits = start_bits;
    shift = 0;
  }

  for (j = 0; j < num_env; j++) {
    ia_huffman_data_type h;
    const WORD32 *idx_tab;
    WORD32 dtdf_dir_flag = p_coding_dir_vec[j];

    if (dtdf_dir_flag == DTDF_DIR_FREQ) {
      p_sbr_sf[ixheaacd_drc_offset] =
          (WORD16)(ixheaacd_read_bits_buf(it_bit_buff, bits) << shift);
      p_sbr_sf_float[ixheaacd_drc_offset] = p_sbr_sf[ixheaacd_drc_offset];
      h = hcb_f;
      idx_tab = idx_f;
    } else {
      h = hcb_t;
      idx_tab = idx_t;
    }

    for (i = (1 - dtdf_dir_flag); i < no_band[j]; i++) {
      if (it_bit_buff->cnt_bits < 20) {
        readword = ixheaacd_show_bits_buf(it_bit_buff, it_bit_buff->cnt_bits);
        readword = readword << (32 - it_bit_buff->cnt_bits);
      } else {
        readword = ixheaacd_show_bits_buf(it_bit_buff, 20);
        readword = readword << 12;
      }
      ixheaacd_huffman_decode(readword, &index, &length, (const UWORD16 *)h,
                              (const UWORD32 *)idx_tab);
      delta = index - lav;
      ixheaacd_read_bits_buf(it_bit_buff, length);
      p_sbr_sf[ixheaacd_drc_offset + i] =
          (WORD16)(delta << env_data_tbl_comp_factor);
      p_sbr_sf_float[ixheaacd_drc_offset + i] =
          p_sbr_sf[ixheaacd_drc_offset + i];
    }
    if (usac_flag && (num_noise_env == 0)) {
      ptr_frame_data->inter_temp_shape_mode[j] = 0;
      if (ptr_frame_data->inter_tes_flag) {
        WORD32 flag = ixheaacd_read_bits_buf(it_bit_buff, 1);
        if (flag) {
          ptr_frame_data->inter_temp_shape_mode[j] =
              ixheaacd_read_bits_buf(it_bit_buff, 2);
        }
      }
    }
    ixheaacd_drc_offset += (no_band[j]);
  }
}

VOID ixheaacd_read_sbr_noise_floor_data(
    ia_sbr_header_data_struct *ptr_header_data,
    ia_sbr_frame_info_data_struct *ptr_frame_data,
    ia_bit_buf_struct *it_bit_buff,
    ia_env_extr_tables_struct *env_extr_tables_ptr) {
  WORD32 i;
  WORD32 coupling_mode;
  WORD16 num_noise_bands[MAX_NOISE_ENVELOPES];
  ia_huffman_data_type hcb_noise_env;
  ia_huffman_data_type hcb_noise;
  WORD32 *idx_noise_env;
  WORD32 *idx_noise;
  WORD32 lav;
  WORD32 env_data_tbl_comp_factor;

  WORD32 start_bits;
  WORD32 start_bits_balance;
  WORD32 num_noise_env = ptr_frame_data->str_frame_info_details.num_noise_env;

  for (i = 0; i < num_noise_env; i++)
    num_noise_bands[i] = ptr_header_data->pstr_freq_band_data->num_nf_bands;

  start_bits = SBR_BEGIN_NOISE_BITS_AMPLITUDE_RESOLUTION_3_0;
  start_bits_balance = SBR_BEGIN_NOISE_BITS_BALNCE_AMPLITUDE_RESOLUTION_3_0;

  coupling_mode = ptr_frame_data->coupling_mode;

  if (coupling_mode == COUPLING_BAL) {
    lav = 12;
    hcb_noise = (ia_huffman_data_type)&env_extr_tables_ptr
                    ->ixheaacd_t_huffman_noise_bal_3_0db_inp_table;
    idx_noise =
        env_extr_tables_ptr->ixheaacd_t_huffman_noise_bal_3_0db_idx_table;
    hcb_noise_env = (ia_huffman_data_type)&env_extr_tables_ptr
                        ->ixheaacd_f_huffman_env_bal_3_0db_inp_table;
    idx_noise_env =
        env_extr_tables_ptr->ixheaacd_f_huffman_env_bal_3_0db_idx_table;
    env_data_tbl_comp_factor = 1;
  } else {
    lav = 31;
    hcb_noise = (ia_huffman_data_type)&env_extr_tables_ptr
                    ->ixheaacd_t_huffman_noise_3_0db_inp_table;
    idx_noise = env_extr_tables_ptr->ixheaacd_t_huffman_noise_3_0db_idx_table;
    hcb_noise_env = (ia_huffman_data_type)&env_extr_tables_ptr
                        ->ixheaacd_f_huffman_env_3_0db_inp_table;
    idx_noise_env = env_extr_tables_ptr->ixheaacd_f_huffman_env_3_0db_idx_table;
    env_data_tbl_comp_factor = 0;
  }

  ixheaacd_read_env_data(ptr_frame_data, it_bit_buff, hcb_noise, hcb_noise_env,
                         idx_noise, idx_noise_env, &num_noise_bands[0],
                         num_noise_env, env_data_tbl_comp_factor, start_bits,
                         start_bits_balance, 1, lav,
                         ptr_header_data->usac_flag);
}

WORD16 ixheaacd_read_sbr_env_data(
    ia_sbr_header_data_struct *ptr_header_data,
    ia_sbr_frame_info_data_struct *ptr_frame_data,
    ia_bit_buf_struct *it_bit_buff,
    ia_env_extr_tables_struct *env_extr_tables_ptr, WORD audio_object_type) {
  WORD32 coupling_mode = ptr_frame_data->coupling_mode;
  WORD32 *idx_t, *idx_f;
  WORD32 lav;
  WORD32 i;
  WORD16 no_band[MAX_ENVELOPES];
  WORD32 amp_res, num_env, env_data_tbl_comp_factor, start_bits,
      start_bits_balance;
  WORD16 *p_freq_res = ptr_frame_data->str_frame_info_details.freq_res;
  WORD16 *p_num_sf_bands = ptr_header_data->pstr_freq_band_data->num_sf_bands;
  ia_huffman_data_type hcb_t, hcb_f;

  amp_res = ptr_header_data->amp_res;
  num_env = ptr_frame_data->str_frame_info_details.num_env;

  ptr_frame_data->num_env_sfac = 0;

  if ((ptr_frame_data->str_frame_info_details.frame_class == FIXFIX) &&
      (num_env == 1)) {
    if (audio_object_type != AOT_ER_AAC_ELD &&
        audio_object_type != AOT_ER_AAC_LD) {
      amp_res = SBR_AMPLITUDE_RESOLUTION_1_5;
    } else {
      amp_res = ptr_frame_data->amp_res;
    }
  }
  ptr_frame_data->amp_res = amp_res;

  if (amp_res == SBR_AMPLITUDE_RESOLUTION_3_0) {
    start_bits = SBR_BEGIN_ENVN_BITS_AMPLITUDE_RESOLUTION_3_0;
    start_bits_balance = SBR_BEGIN_ENVN_BITS_BALNCE_AMPLITUDE_RESOLUTION_3_0;
  } else {
    start_bits = SBR_BEGIN_ENVN_BITS_AMPLITUDE_RESOLUTION_1_5;
    start_bits_balance = SBR_BEGIN_ENVN_BITS_BALNCE_AMPLITUDE_RESOLUTION_1_5;
  }

  for (i = 0; i < num_env; i++) {
    no_band[i] = p_num_sf_bands[*p_freq_res++];
    ptr_frame_data->num_env_sfac =
        ixheaac_add16(ptr_frame_data->num_env_sfac, no_band[i]);
  }

  if (ptr_frame_data->num_env_sfac > MAX_NUM_ENVELOPE_VALUES) return 0;

  if (coupling_mode == COUPLING_BAL) {
    env_data_tbl_comp_factor = 1;

    if (amp_res == SBR_AMPLITUDE_RESOLUTION_1_5) {
      lav = 24;
      hcb_t = (ia_huffman_data_type)&env_extr_tables_ptr
                  ->ixheaacd_t_huffman_env_bal_1_5db_inp_table;
      idx_t = env_extr_tables_ptr->ixheaacd_t_huffman_env_bal_1_5db_idx_table;
      hcb_f = (ia_huffman_data_type)&env_extr_tables_ptr
                  ->ixheaacd_f_huffman_env_bal_1_5db_inp_table;
      idx_f = env_extr_tables_ptr->ixheaacd_f_huffman_env_bal_1_5db_idx_table;
    } else {
      lav = 12;
      hcb_t = (ia_huffman_data_type)&env_extr_tables_ptr
                  ->ixheaacd_t_huffman_env_bal_3_0db_inp_table;
      idx_t = env_extr_tables_ptr->ixheaacd_t_huffman_env_bal_3_0db_idx_table;
      hcb_f = (ia_huffman_data_type)&env_extr_tables_ptr
                  ->ixheaacd_f_huffman_env_bal_3_0db_inp_table;
      idx_f = env_extr_tables_ptr->ixheaacd_f_huffman_env_bal_3_0db_idx_table;
    }
  } else {
    env_data_tbl_comp_factor = 0;

    if (amp_res == SBR_AMPLITUDE_RESOLUTION_1_5) {
      lav = 60;
      hcb_t = (ia_huffman_data_type)&env_extr_tables_ptr
                  ->ixheaacd_t_huffman_env_1_5db_inp_table;
      idx_t = env_extr_tables_ptr->ixheaacd_t_huffman_env_1_5db_idx_table;
      hcb_f = (ia_huffman_data_type)&env_extr_tables_ptr
                  ->ixheaacd_f_huffman_env_1_5db_inp_table;
      idx_f = env_extr_tables_ptr->ixheaacd_f_huffman_env_1_5db_idx_table;
    } else {
      lav = 31;
      hcb_t = (ia_huffman_data_type)&env_extr_tables_ptr
                  ->ixheaacd_t_huffman_env_3_0db_inp_table;
      idx_t = env_extr_tables_ptr->ixheaacd_t_huffman_env_3_0db_idx_table;
      hcb_f = (ia_huffman_data_type)&env_extr_tables_ptr
                  ->ixheaacd_f_huffman_env_3_0db_inp_table;
      idx_f = env_extr_tables_ptr->ixheaacd_f_huffman_env_3_0db_idx_table;
    }
  }

  ixheaacd_read_env_data(ptr_frame_data, it_bit_buff, hcb_t, hcb_f, idx_t,
                         idx_f, &no_band[0], num_env, env_data_tbl_comp_factor,
                         start_bits, start_bits_balance, 0, lav,
                         ptr_header_data->usac_flag);

  return 1;
}

IA_ERRORCODE ixheaacd_extract_frame_info_ld(
    ia_bit_buf_struct *it_bit_buff,
    ia_sbr_frame_info_data_struct *h_frame_data) {
  int abs_bord_lead = 0, num_rel_lead = 0, bs_num_env = 0,
      frame_class, temp, env, k, abs_bord_trail = 0, middle_bord = 0,
      bs_num_noise, transient_env_temp = 0, bs_transient_position = 0;

  IA_ERRORCODE err = IA_NO_ERROR;
  WORD16 time_border[MAX_ENVELOPES + 1];
  WORD16 time_border_noise[2 + 1];
  WORD16 f[MAX_ENVELOPES + 1];
  int rel_bord_lead[7] = {0};

  ia_frame_info_struct *v_frame_info = &h_frame_data->str_frame_info_details;

  int numTimeSlots = h_frame_data->num_time_slots;

  v_frame_info->frame_class = frame_class =
      ixheaacd_read_bits_buf(it_bit_buff, SBRLD_CLA_BITS);

  switch (frame_class) {
    case FIXFIX:
      temp = ixheaacd_read_bits_buf(it_bit_buff, SBR_ENV_BITS);
      bs_num_env = 1 << temp;

      if (bs_num_env == 1)
        h_frame_data->amp_res =
            ixheaacd_read_bits_buf(it_bit_buff, SBR_AMPLITUDE_RESOLUTION_BITS);

      f[0] = ixheaacd_read_bits_buf(it_bit_buff, SBR_RES_BITS);

      for (env = 1; env < bs_num_env; env++) f[env] = f[0];
      break;
    case LD_TRAN:
      bs_transient_position =
          ixheaacd_read_bits_buf(it_bit_buff, SBR_TRAN_BITS);
      v_frame_info->frame_class = 0;
      if ((numTimeSlots != 16) && (bs_transient_position >= LD_ENV_TBL_480)) {
        return -1;
      }
      bs_num_env = (numTimeSlots == 16)
                       ? ixheaacd_ld_env_table_512[bs_transient_position]
                                                  [SBR_ENVT_NUMENV]
                       : ixheaacd_ld_env_table_480[bs_transient_position]
                                                  [SBR_ENVT_NUMENV];
      for (env = 0; env < bs_num_env; env++)
        f[env] = ixheaacd_read_bits_buf(it_bit_buff, SBR_RES_BITS);
      break;
  }

  switch (frame_class) {
    case FIXFIX:
      abs_bord_lead = 0;
      abs_bord_trail = numTimeSlots;
      num_rel_lead = bs_num_env - 1;

      for (k = 0; k < num_rel_lead; k++) {
        rel_bord_lead[k] = ixheaacd_ld_env_table_time_slot[num_rel_lead - 1];
      }

      time_border[0] = abs_bord_lead;
      time_border[bs_num_env] = abs_bord_trail;
      for (env = 1; env <= num_rel_lead; env++) {
        time_border[env] = abs_bord_lead;
        for (k = 0; k <= env - 1; k++) time_border[env] += rel_bord_lead[k];
      }
      break;

    case LD_TRAN:
      time_border[0] = 0;
      time_border[bs_num_env] = numTimeSlots;
      for (k = 1; k < bs_num_env; k++)
        time_border[k] =
            (numTimeSlots == 16)
                ? ixheaacd_ld_env_table_512[bs_transient_position][k]
                : ixheaacd_ld_env_table_480[bs_transient_position][k];
      break;

    default:
      time_border[0] = 0;

      break;
  };

  switch (frame_class) {
    case FIXFIX:
      middle_bord = bs_num_env / 2;
      break;
    case LD_TRAN:
      middle_bord = 1;
      break;
  };

  time_border_noise[0] = time_border[0];
  if (bs_num_env > 1) {
    time_border_noise[1] = time_border[middle_bord];
    time_border_noise[2] = time_border[bs_num_env];
    bs_num_noise = 2;
  } else {
    time_border_noise[1] = time_border[bs_num_env];
    bs_num_noise = 1;
  }

  switch (frame_class) {
    case FIXFIX:
      transient_env_temp = -1;
      break;
    case LD_TRAN:
      transient_env_temp =
          (numTimeSlots == 16)
              ? ixheaacd_ld_env_table_512[bs_transient_position]
                                         [SBR_ENVT_TRANIDX]
              : ixheaacd_ld_env_table_480[bs_transient_position]
                                         [SBR_ENVT_TRANIDX];
      break;
  };

  v_frame_info->num_env = bs_num_env;
  memcpy(v_frame_info->border_vec, time_border,
         (bs_num_env + 1) * sizeof(WORD16));
  memcpy(v_frame_info->freq_res, f, bs_num_env * sizeof(WORD16));
  v_frame_info->transient_env = transient_env_temp;
  v_frame_info->num_noise_env = bs_num_noise;
  memcpy(v_frame_info->noise_border_vec, time_border_noise,
         (bs_num_noise + 1) * sizeof(WORD16));

  return err;
}

WORD32 ixheaacd_pvc_time_freq_grid_info(
    ia_bit_buf_struct *it_bit_buff,
    ia_sbr_frame_info_data_struct *ptr_frame_data) {
  WORD32 bs_num_env = 0, bs_num_noise = 0;
  WORD32 time_border[MAX_ENVELOPES + 1];
  WORD32 time_border_noise[2 + 1];
  WORD32 pvc_time_border[MAX_ENVELOPES + 1];
  WORD32 pvc_time_border_noise[2 + 1];
  WORD32 bs_freq_res[MAX_ENVELOPES + 1];
  WORD32 var_len;
  ia_frame_info_struct *p_frame_info = &ptr_frame_data->str_frame_info_details;
  ia_frame_info_struct *pvc_frame_info = &ptr_frame_data->str_pvc_frame_info;
  WORD32 i;
  WORD32 prev_sbr_mode = ptr_frame_data->prev_sbr_mode;

  WORD32 tmp;
  WORD32 bs_noise_pos;
  bs_noise_pos = ixheaacd_read_bits_buf(it_bit_buff, 4);

  tmp = ixheaacd_read_bits_buf(it_bit_buff, 1);
  if (tmp == 0) {
    ptr_frame_data->var_len = 0;
  } else {
    tmp = ixheaacd_read_bits_buf(it_bit_buff, 2);
    ptr_frame_data->var_len = tmp + 1;
    if (ptr_frame_data->var_len > 3) {
      return -1;
    }
  }
  var_len = ptr_frame_data->var_len;

  if (p_frame_info->num_env > 0) {
    time_border[0] = p_frame_info->border_vec[p_frame_info->num_env] - 16;
  } else {
    time_border[0] = 0;
  }
  if (time_border[0] < 0) return -1;
  pvc_time_border[0] = 0;
  bs_freq_res[0] = 0;

  if (ptr_frame_data->prev_sbr_mode == 0) {
    pvc_time_border[0] = time_border[0];
  }

  if (bs_noise_pos == 0) {
    time_border[1] = 16 + var_len;
    pvc_time_border[1] = 16;
    bs_num_noise = 1;
    bs_num_env = 1;
  } else {
    time_border[1] = bs_noise_pos;
    pvc_time_border[1] = bs_noise_pos;
    time_border[2] = 16 + var_len;
    pvc_time_border[2] = 16;
    bs_freq_res[1] = 0;
    bs_num_noise = 2;
    bs_num_env = 2;
  }

  for (i = 0; i < 3; i++) {
    time_border_noise[i] = time_border[i];
    pvc_time_border_noise[i] = pvc_time_border[i];
  }

  if (prev_sbr_mode == ORIG_SBR) {
    pvc_time_border[0] = time_border[0];
    pvc_time_border_noise[0] = time_border[0];
  }

  pvc_frame_info->num_env = bs_num_env;
  for (i = 0; i < (bs_num_env + 1); i++) {
    pvc_frame_info->border_vec[i] = pvc_time_border[i];
  }
  for (i = 0; i < (bs_num_env); i++) {
    pvc_frame_info->freq_res[i] = bs_freq_res[i];
  }
  pvc_frame_info->transient_env = -1;
  pvc_frame_info->num_noise_env = bs_num_noise;
  for (i = 0; i < (bs_num_noise + 1); i++) {
    pvc_frame_info->noise_border_vec[i] = pvc_time_border_noise[i];
  }
  p_frame_info->num_env = bs_num_env;
  for (i = 0; i < (bs_num_env + 1); i++) {
    p_frame_info->border_vec[i] = time_border[i];
  }
  for (i = 0; i < (bs_num_env); i++) {
    p_frame_info->freq_res[i] = bs_freq_res[i];
  }
  p_frame_info->transient_env = -1;
  p_frame_info->num_noise_env = bs_num_noise;
  for (i = 0; i < (bs_num_noise + 1); i++) {
    p_frame_info->noise_border_vec[i] = time_border_noise[i];
  }
  return 0;
}

WORD16 ixheaacd_sbr_time_freq_grid_info(
    ia_bit_buf_struct *it_bit_buff,
    ia_sbr_frame_info_data_struct *ptr_frame_data,
    ia_env_extr_tables_struct *env_extr_tables_ptr, WORD16 number_of_time_slots) {
  WORD32 i, k, bs_num_rel = 0;
  WORD32 bs_pointer_bits = 0, bs_num_env = 0, border, bs_pointer,
         bs_var_bord = 0, temp = 0;
  WORD32 freq_res_0 = 0, frame_class;
  WORD32 abs_bord_lead, abs_bord_trail, num_rel_trail, num_rel_lead;
  static const WORD32 pointer_bits_array[7] = {1, 2, 2, 3, 3, 3, 3};
  ia_frame_info_struct *p_fixfix_tab;
  ia_frame_info_struct *p_frame_info = &ptr_frame_data->str_frame_info_details;

  frame_class = ixheaacd_read_bits_buf(it_bit_buff, SBR_FRAME_CLASS_BITS);
  p_frame_info->frame_class = frame_class;

  switch (frame_class) {
    case FIXFIX:
      temp =
          ixheaacd_read_bits_buf(it_bit_buff, SBR_ENV_BITS + SBR_FRQ_RES_BITS);
      bs_num_env = (temp & 0x6) >> SBR_FRQ_RES_BITS;

      if (number_of_time_slots != 15) {
        p_fixfix_tab = &env_extr_tables_ptr->sbr_frame_info1_2_4_16[bs_num_env];
      } else {
        if (bs_num_env > 2) return 0;
        p_fixfix_tab = &env_extr_tables_ptr->sbr_frame_info1_2_4_16[bs_num_env + 4];
      }

      memcpy(p_frame_info, p_fixfix_tab, sizeof(ia_frame_info_struct));
      bs_num_env = (1 << bs_num_env);
      freq_res_0 = temp & 0x1;

      if (!freq_res_0) {
        memset(&p_frame_info->freq_res[0], 0, sizeof(WORD16) * bs_num_env);
      }
      break;
    case FIXVAR:
      bs_var_bord =
          ixheaacd_read_bits_buf(it_bit_buff, SBR_VAR_BORD_BITS + SBR_NUM_BITS);
      bs_num_rel = bs_var_bord & 3;
      bs_var_bord = bs_var_bord >> SBR_NUM_BITS;
      bs_num_env = bs_num_rel + 1;
      p_frame_info->border_vec[0] = 0;

      if (number_of_time_slots != 15) {
        border = bs_var_bord + SBR_TIME_SLOTS;
      } else {
        border = bs_var_bord + number_of_time_slots;
      }

      p_frame_info->border_vec[bs_num_env] = border;
      for (k = bs_num_rel; k > 0; k--) {
        temp = ixheaacd_read_bits_buf(it_bit_buff, SBR_REL_BITS);
        border = border - ((temp << 1) + 2);
        if (border < 0) border = 0;
        p_frame_info->border_vec[k] = border;
      }

      bs_pointer_bits = pointer_bits_array[bs_num_rel];
      bs_pointer = ixheaacd_read_bits_buf(it_bit_buff, bs_pointer_bits);

      if ((bs_pointer - (bs_num_rel + 1)) > 0) return 0;

      for (k = bs_num_rel; k >= 0; k--) {
        p_frame_info->freq_res[k] =
            ixheaacd_read_bits_buf(it_bit_buff, SBR_FRQ_RES_BITS);
      }
      if (bs_pointer) {
        p_frame_info->transient_env = bs_num_env + 1 - bs_pointer;
      } else {
        p_frame_info->transient_env = -1;
      }
      if ((bs_pointer == 0) || (bs_pointer == 1))
        p_frame_info->noise_border_vec[1] =
            p_frame_info->border_vec[bs_num_rel];
      else
        p_frame_info->noise_border_vec[1] =
            p_frame_info->border_vec[p_frame_info->transient_env];

      break;

    case VARFIX:
      bs_var_bord =
          ixheaacd_read_bits_buf(it_bit_buff, SBR_VAR_BORD_BITS + SBR_NUM_BITS);
      bs_num_rel = bs_var_bord & 3;
      bs_var_bord = bs_var_bord >> SBR_NUM_BITS;
      bs_num_env = bs_num_rel + 1;

      border = bs_var_bord;
      p_frame_info->border_vec[0] = border;
      for (k = 1; k <= bs_num_rel; k++) {
        temp = ixheaacd_read_bits_buf(it_bit_buff, SBR_REL_BITS);
        border = border + ((temp << 1) + 2);

        if (number_of_time_slots != 15) {
          if (border > SBR_TIME_SLOTS) border = SBR_TIME_SLOTS;
        } else {
          if (border > number_of_time_slots) border = number_of_time_slots;
        }

        p_frame_info->border_vec[k] = border;
      }

      if (number_of_time_slots != 15) {
        p_frame_info->border_vec[k] = SBR_TIME_SLOTS;
      } else {
        p_frame_info->border_vec[k] = number_of_time_slots;
      }

      bs_pointer_bits = pointer_bits_array[bs_num_rel];

      bs_pointer = ixheaacd_read_bits_buf(it_bit_buff, bs_pointer_bits);

      if ((bs_pointer - (bs_num_rel + 1)) > 0) return 0;

      if (bs_pointer == 0 || (bs_pointer - 1) == 0) {
        p_frame_info->transient_env = -1;
      } else {
        p_frame_info->transient_env = bs_pointer - 1;
      }

      for (k = 0; k <= bs_num_rel; k++) {
        p_frame_info->freq_res[k] =
            ixheaacd_read_bits_buf(it_bit_buff, SBR_FRQ_RES_BITS);
      }

      switch (bs_pointer) {
        case 0:
          p_frame_info->noise_border_vec[1] = p_frame_info->border_vec[1];
          break;
        case 1:
          p_frame_info->noise_border_vec[1] =
              p_frame_info->border_vec[bs_num_rel];
          break;
        default:
          p_frame_info->noise_border_vec[1] =
              p_frame_info->border_vec[(WORD32)p_frame_info->transient_env];
          break;
      }

      break;

    case VARVAR:
      abs_bord_lead = ixheaacd_read_bits_buf(
          it_bit_buff, 2 * SBR_VAR_BORD_BITS + 2 * SBR_NUM_BITS);

      if (number_of_time_slots != 15) {
        abs_bord_trail =
          (((abs_bord_lead & 0x30) >> (2 * SBR_NUM_BITS)) + SBR_TIME_SLOTS);
      } else {
        abs_bord_trail =
          (((abs_bord_lead & 0x30) >> (2 * SBR_NUM_BITS)) + number_of_time_slots);
      }

      num_rel_trail = ((abs_bord_lead & 0xc) >> SBR_NUM_BITS);
      num_rel_lead = (abs_bord_lead & 0x3);
      abs_bord_lead = abs_bord_lead >> (SBR_VAR_BORD_BITS + 2 * SBR_NUM_BITS);
      bs_num_env = ((num_rel_trail + num_rel_lead) + 1);
      border = abs_bord_lead;
      p_frame_info->border_vec[0] = border;

      for (k = 1; k <= num_rel_trail; k++) {
        temp = ixheaacd_read_bits_buf(it_bit_buff, SBR_REL_BITS);
        border = border + ((temp << 1) + 2);
        p_frame_info->border_vec[k] = border;
      }

      border = abs_bord_trail;
      i = bs_num_env;

      p_frame_info->border_vec[i] = border;

      for (k = 0; k < num_rel_lead; k++) {
        temp = ixheaacd_read_bits_buf(it_bit_buff, SBR_REL_BITS);
        border = border - ((temp << 1) + 2);
        i--;
        p_frame_info->border_vec[i] = border;
      }
      bs_pointer_bits = pointer_bits_array[num_rel_trail + num_rel_lead];

      bs_pointer = ixheaacd_read_bits_buf(it_bit_buff, bs_pointer_bits);
      if ((bs_pointer - ((num_rel_trail + num_rel_lead) + 1)) > 0) return 0;

      if (bs_pointer) {
        p_frame_info->transient_env = bs_num_env + 1 - bs_pointer;
      } else {
        p_frame_info->transient_env = -1;
      }

      for (k = 0; k < bs_num_env; k++) {
        p_frame_info->freq_res[k] =
            ixheaacd_read_bits_buf(it_bit_buff, SBR_FRQ_RES_BITS);
      }
      p_frame_info->noise_border_vec[0] = abs_bord_lead;
      if (bs_num_env == 1) {
        p_frame_info->noise_border_vec[1] = abs_bord_trail;
      } else {
        if (bs_pointer == 0 || (bs_pointer - 1) == 0)
          p_frame_info->noise_border_vec[1] =
              p_frame_info->border_vec[bs_num_env - 1];
        else
          p_frame_info->noise_border_vec[1] =
              p_frame_info->border_vec[(WORD32)p_frame_info->transient_env];

        p_frame_info->noise_border_vec[2] = abs_bord_trail;
      }
      break;
  }
  p_frame_info->num_env = bs_num_env;

  if (bs_num_env == 1)
    p_frame_info->num_noise_env = 1;
  else
    p_frame_info->num_noise_env = 2;

  if (frame_class == VARFIX || frame_class == FIXVAR) {
    p_frame_info->noise_border_vec[0] = p_frame_info->border_vec[0];
    p_frame_info->noise_border_vec[p_frame_info->num_noise_env] =
        p_frame_info->border_vec[bs_num_env];
  }
  return 1;
}