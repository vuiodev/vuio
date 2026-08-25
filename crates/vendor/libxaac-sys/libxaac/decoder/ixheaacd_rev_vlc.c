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
#include "ixheaacd_sbr_common.h"
#include "ixheaac_type_def.h"

#include "ixheaac_constants.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops16.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaac_basic_ops.h"

#include "ixheaacd_bitbuffer.h"
#include "ixheaacd_error_codes.h"
#include "ixheaacd_defines.h"
#include "ixheaacd_aac_rom.h"
#include "ixheaacd_pulsedata.h"

#include "ixheaacd_pns.h"
#include "ixheaacd_drc_data_struct.h"

#include "ixheaacd_lt_predict.h"
#include "ixheaacd_cnst.h"
#include "ixheaacd_ec_defines.h"
#include "ixheaacd_ec_struct_def.h"
#include "ixheaacd_channelinfo.h"

#include "ixheaacd_drc_dec.h"

#include "ixheaacd_sbrdecoder.h"

#include "ixheaacd_block.h"
#include "ixheaacd_channel.h"

#include "ixheaacd_common_rom.h"
#include "ixheaacd_basic_funcs.h"
#include "ixheaac_basic_op.h"

#include "ixheaacd_aacdec.h"

#include "ixheaacd_sbrdecsettings.h"
#include "ixheaacd_sbr_scale.h"
#include "ixheaacd_env_extr_part.h"
#include "ixheaacd_sbr_rom.h"
#include "ixheaacd_audioobjtypes.h"
#include "ixheaacd_sbrdecoder.h"
#include "ixheaacd_memory_standards.h"
#include "ixheaacd_latmdemux.h"
#include "ixheaacd_mps_polyphase.h"
#include "ixheaacd_config.h"
#include "ixheaacd_hybrid.h"
#include "ixheaacd_ps_dec.h"
#include "ixheaacd_qmf_dec.h"
#include "ixheaacd_mps_macro_def.h"
#include "ixheaacd_mps_struct_def.h"
#include "ixheaacd_mps_res_rom.h"
#include "ixheaacd_mps_aac_struct.h"
#include "ixheaacd_mps_dec.h"
#include "ixheaacd_mps_interface.h"
#include "ixheaacd_struct_def.h"
#include "ixheaacd_cnst.h"
#include "ixheaac_error_standards.h"

#define RVLC_ERROR_ALL_ESCAPE_WORDS_INVALID 0x80000000
#define RVLC_ERROR_FORBIDDEN_CW_DETECTED_FWD 0x08000000
#define RVLC_ERROR_FORBIDDEN_CW_DETECTED_BWD 0x04000000

#define FWD 0
#define BWD 1

#define MAX_LEN_RVLC_CODE_WORD 9
#define MAX_LEN_RVLC_ESCAPE_WORD 20

#define CONCEAL_MAX_INIT 1311
#define CONCEAL_MIN_INIT -1311

#define RVLC_MAX_SFB ((8) * (16))

#define MASK_LEFT 0xFFF000
#define MASK_RIGHT 0xFFF
#define CLR_BIT_10 0x3FF
#define LEFT_OFFSET 12

#define ixheaacd_bitbuf_checkpoint(it_bit_buf, saved_bit_buf) \
  (saved_bit_buf) = (it_bit_buf)
#define ixheaacd_bitbuf_restore(it_bit_buf, saved_bit_buf) \
  (it_bit_buf) = (saved_bit_buf)

static WORD32 ixheaacd_rvlc_decode(WORD16 cw, WORD32 len, WORD32 *found) {
  WORD16 indx = 0;
  *found = 0;
  switch (len) {
    case 1:
      if (cw == 0)
        indx = 0;
      else
        return 3;
      break;
    case 3:
      switch (cw) {
        case 5:
          indx = -1;
          break;
        case 7:
          indx = 1;
          break;
        default:
          return 4;
      }
      break;
    case 4:
      if (cw == 9)
        indx = -2;
      else
        return 5;
      break;
    case 5:
      switch (cw) {
        case 17:
          indx = -3;
          break;
        case 27:
          indx = 2;
          break;
        default:
          return 6;
      }
      break;
    case 6:
      switch (cw) {
        case 33:
          indx = -4;
          break;
        case 51:
          indx = 3;
          break;
        default:
          return 7;
      }
      break;
    case 7:
      switch (cw) {
        case 65:
          indx = -7;
          break;
        case 107:
          indx = 4;
          break;
        case 99:
          indx = 7;
          break;
        default:
          return 8;
      }
      break;
    case 8:
      switch (cw) {
        case 129:
          indx = -5;
          break;
        case 195:
          indx = 5;
          break;
        default:
          return 9;
      }
      break;
    case 9:
      switch (cw) {
        case 257:
          indx = -6;
          break;
        case 427:
          indx = 6;
          break;
        default:
          return -1;
      }
      break;
    default:
      return -1;
  }
  *found = 1;
  return indx;
}

static WORD32 ixheaacd_rvlc_decode_esc(WORD32 cw, WORD32 len, WORD32 *found) {
  WORD16 indx = 0;
  *found = 0;
  switch (len) {
    case 2:
      switch (cw) {
        case 2:
          indx = 0;
          break;
        case 0:
          indx = 1;
          break;
        default:
          return 3;
      }
      break;
    case 3:
      switch (cw) {
        case 6:
          indx = 2;
          break;
        case 2:
          indx = 3;
          break;
        default:
          return 4;
      }
      break;
    case 4:
      if (cw == 14)
        indx = 4;
      else
        return 5;
      break;
    case 5:
      switch (cw) {
        case 31:
          indx = 5;
          break;
        case 15:
          indx = 6;
          break;
        case 13:
          indx = 7;
          break;
        default:
          return 6;
      }
      break;
    case 6:
      switch (cw) {
        case 61:
          indx = 8;
          break;
        case 29:
          indx = 9;
          break;
        case 25:
          indx = 10;
          break;
        case 24:
          indx = 11;
          break;
        default:
          return 7;
      }
      break;
    case 7:
      switch (cw) {
        case 120:
          indx = 12;
          break;
        case 56:
          indx = 13;
          break;
        default:
          return 8;
      }
      break;
    case 8:
      switch (cw) {
        case 242:
          indx = 14;
          break;
        case 114:
          indx = 15;
          break;
        default:
          return 9;
      }
      break;
    case 9:
      switch (cw) {
        case 486:
          indx = 16;
          break;
        case 230:
          indx = 17;
          break;
        default:
          return 10;
      }
      break;
    case 10:
      switch (cw) {
        case 974:
          indx = 18;
          break;
        case 463:
          indx = 19;
          break;
        default:
          return 11;
      }
      break;
    case 11:
      switch (cw) {
        case 1950:
          indx = 20;
          break;
        case 1951:
          indx = 21;
          break;
        case 925:
          indx = 22;
          break;
        default:
          return 12;
      }
      break;
    case 12:
      if (cw == 1848)
        indx = 23;
      else
        return 13;
      break;
    case 13:
      if (cw == 3698)
        indx = 25;
      else
        return 14;
      break;
    case 14:
      if (cw == 7399)
        indx = 24;
      else
        return 15;
      break;
    case 15:
      if (cw == 14797)
        indx = 26;
      else
        return 19;
      break;
    case 19:
      if ((cw >= 236736) && (cw <= 236740))
        indx = 53 - (236740 - cw);
      else
        return 20;
      break;
    case 20:
      if ((cw >= 473482) && (cw <= 473503))
        indx = 48 - (473503 - cw);
      else
        return -1;
      break;
    default:
      return -1;
  }
  *found = 1;
  return indx;
}
static VOID ixheaacd_rvlc_check_intensity_cb(
    ia_rvlc_info_struct *ptr_rvlc,
    ia_aac_dec_channel_info_struct *ptr_aac_dec_channel_info) {
  WORD32 group, band, bnds;

  ptr_rvlc->intensity_used = 0;

  for (group = 0; group < ptr_rvlc->num_wind_grps; group++) {
    for (band = 0; band < ptr_rvlc->max_sfb_transmitted; band++) {
      bnds = 16 * group + band;
      if ((ptr_aac_dec_channel_info->ptr_code_book[bnds] == INTENSITY_HCB) ||
          (ptr_aac_dec_channel_info->ptr_code_book[bnds] == INTENSITY_HCB2)) {
        ptr_rvlc->intensity_used = 1;
        break;
      }
    }
  }
}

VOID ixheaacd_carry_bit_branch_val(UWORD8 carry_bit, UWORD32 tree_node,
                                   UWORD32 *branch_val, UWORD32 *branch_node) {
  if (carry_bit == 0) {
    *branch_node = (tree_node & MASK_LEFT) >> LEFT_OFFSET;
  } else {
    *branch_node = tree_node & MASK_RIGHT;
  }

  *branch_val = *branch_node & CLR_BIT_10;
}

UWORD8 ixheaacd_rvlc_read_bits(ia_bit_buf_struct *it_bit_buff,
                               UWORD16 *ptr_position, UWORD8 read_direction) {
  UWORD32 bit;
  WORD32 read_bit_offset =
      *ptr_position - (it_bit_buff->size - it_bit_buff->cnt_bits);

  if (read_bit_offset) it_bit_buff->cnt_bits -= read_bit_offset;

  it_bit_buff->ptr_read_next =
      it_bit_buff->ptr_bit_buf_base +
      ((it_bit_buff->size - it_bit_buff->cnt_bits) >> 3);
  it_bit_buff->bit_pos = ((it_bit_buff->size - it_bit_buff->cnt_bits) & 7);

  if (read_direction == 0) {
    bit = ixheaacd_aac_read_bit_rev(it_bit_buff);

    *ptr_position += 1;
  } else {
    bit = ixheaacd_aac_read_bit(it_bit_buff);

    *ptr_position -= 1;
  }

  return (bit);
}

static WORD8 ixheaacd_rvlc_decode_escape_word(ia_rvlc_info_struct *ptr_rvlc,
                                              ia_bit_buf_struct *it_bit_buff) {
  WORD32 i;

  UWORD8 carry_bit;

  UWORD16 *ptr_bitstream_index_esc;

  int len = 0;
  int codeword = 0;
  int found = 0;
  int indx;

  ptr_bitstream_index_esc = &(ptr_rvlc->esc_bit_str_idx);

  for (i = MAX_LEN_RVLC_ESCAPE_WORD - 1; i >= 0; i--) {
    carry_bit =
        ixheaacd_rvlc_read_bits(it_bit_buff, ptr_bitstream_index_esc, FWD);

    len++;
    codeword = codeword << 1 | carry_bit;
    indx = ixheaacd_rvlc_decode_esc(codeword, len, &found);

    if (found) {
      ptr_rvlc->rvlc_esc_len -= (MAX_LEN_RVLC_ESCAPE_WORD - i);
      return indx;
    }
  }

  ptr_rvlc->rvlc_err_log |= RVLC_ERROR_ALL_ESCAPE_WORDS_INVALID;

  return -1;
}

static VOID ixheaacd_rvlc_decode_escape(ia_rvlc_info_struct *ptr_rvlc,
                                        WORD16 *ptr_escape,
                                        ia_bit_buf_struct *it_bit_buff) {
  WORD8 esc_word;
  WORD8 esc_cnt = 0;
  WORD16 *ptr_esc_bit_cnt_sum;

  ptr_esc_bit_cnt_sum = &(ptr_rvlc->rvlc_esc_len);

  while (*ptr_esc_bit_cnt_sum > 0) {
    esc_word = ixheaacd_rvlc_decode_escape_word(ptr_rvlc, it_bit_buff);

    if (esc_word >= 0) {
      ptr_escape[esc_cnt] = esc_word;
      esc_cnt++;
    } else {
      ptr_rvlc->rvlc_err_log |= RVLC_ERROR_ALL_ESCAPE_WORDS_INVALID;
      ptr_rvlc->num_esc_words_decoded = esc_cnt;

      return;
    }
  }

  ptr_rvlc->num_esc_words_decoded = esc_cnt;
}

WORD8 ixheaacd_decode_rvlc_code_word(ia_bit_buf_struct *it_bit_buff,
                                     ia_rvlc_info_struct *ptr_rvlc) {
  WORD32 i;

  UWORD8 carry_bit;

  UWORD8 direction = ptr_rvlc->direction;
  UWORD16 *ptr_bit_str_idx_rvl = ptr_rvlc->ptr_rvl_bit_str_idx;

  int len = 0;
  short codeword = 0;
  int found = 0;
  int indx;

  for (i = MAX_LEN_RVLC_CODE_WORD - 1; i >= 0; i--) {
    carry_bit =
        ixheaacd_rvlc_read_bits(it_bit_buff, ptr_bit_str_idx_rvl, direction);

    len++;
    codeword = codeword << 1 | carry_bit;
    indx = ixheaacd_rvlc_decode(codeword, len, &found);
    if (found) {
      indx = indx + 7;
      *ptr_rvlc->ptr_rvl_bit_cnt -= (MAX_LEN_RVLC_CODE_WORD - i);
      return indx;
    }
  }

  return -1;
}

static VOID ixheaacd_rvlc_decode_forward(
    ia_rvlc_info_struct *ptr_rvlc,
    ia_aac_dec_channel_info_struct *ptr_aac_dec_channel_info,
    ia_bit_buf_struct *it_bit_buff) {
  WORD32 band = 0;
  WORD32 group = 0;
  WORD32 bnds = 0;

  WORD16 dpcm;

  ia_bit_buf_struct temp_buf = {0};

  WORD16 factor = ptr_aac_dec_channel_info->global_gain;
  WORD16 position = 0;
  WORD16 noise_energy = ptr_aac_dec_channel_info->global_gain - 90 - 256;

  WORD16 *ptr_scf_fwd = ptr_aac_dec_channel_info->rvlc_scf_fwd_arr;
  WORD16 *ptr_scf_esc = ptr_aac_dec_channel_info->rvlc_scf_esc_arr;
  UWORD8 *ptr_esc_fwd_cnt = &(ptr_rvlc->num_fwd_esc_words_decoded);

  ptr_rvlc->ptr_rvl_bit_cnt = &(ptr_rvlc->rvlc_sf_fwd_len);
  ptr_rvlc->ptr_rvl_bit_str_idx = &(ptr_rvlc->rvl_fwd_bit_str_idx);

  *ptr_esc_fwd_cnt = 0;
  ptr_rvlc->direction = 0;
  ptr_rvlc->noise_used = 0;
  ptr_rvlc->sf_used = 0;
  ptr_rvlc->last_scale_fac = 0;
  ptr_rvlc->last_nrg = 0;
  ptr_rvlc->is_last = 0;

  ixheaacd_rvlc_check_intensity_cb(ptr_rvlc, ptr_aac_dec_channel_info);

  for (group = 0; group < ptr_rvlc->num_wind_grps; group++) {
    for (band = 0; band < ptr_rvlc->max_sfb_transmitted; band++) {
      bnds = 16 * group + band;

      switch (ptr_aac_dec_channel_info->ptr_code_book[bnds]) {
        case ZERO_HCB:
          ptr_scf_fwd[bnds] = 0;
          break;

        case INTENSITY_HCB2:
        case INTENSITY_HCB:

        {
          dpcm = ixheaacd_decode_rvlc_code_word(it_bit_buff, ptr_rvlc);
          if (dpcm < 0) {
            ptr_rvlc->conceal_max = bnds;
            return;
          }
          dpcm -= 7;
        }
          if ((dpcm == -7) || (dpcm == 7)) {
            if (ptr_rvlc->rvlc_esc_len) {
              ptr_rvlc->conceal_max = bnds;
              return;
            } else {
              if (dpcm == -7) {
                dpcm -= *ptr_scf_esc++;
              } else {
                dpcm += *ptr_scf_esc++;
              }
              (*ptr_esc_fwd_cnt)++;
              if (ptr_rvlc->conceal_max_esc == 1311) {
                ptr_rvlc->conceal_max_esc = bnds;
              }
            }
          }
          position += dpcm;
          ptr_scf_fwd[bnds] = position;
          ptr_rvlc->is_last = position;
          break;

        case NOISE_HCB:
          if (ptr_rvlc->noise_used == 0) {
            ptr_rvlc->noise_used = 1;
            ptr_rvlc->first_noise_band = bnds;
            noise_energy += ptr_rvlc->dpcm_noise_nrg;
            ptr_scf_fwd[bnds] = noise_energy;
            ptr_rvlc->last_nrg = noise_energy;
          } else {
            dpcm = ixheaacd_decode_rvlc_code_word(it_bit_buff, ptr_rvlc);
            if (dpcm < 0) {
              ptr_rvlc->conceal_max = bnds;
              return;
            }
            dpcm -= 7;
            if ((dpcm == -7) || (dpcm == 7)) {
              if (ptr_rvlc->rvlc_esc_len) {
                ptr_rvlc->conceal_max = bnds;
                return;
              } else {
                if (dpcm == -7) {
                  dpcm -= *ptr_scf_esc++;
                } else {
                  dpcm += *ptr_scf_esc++;
                }
                (*ptr_esc_fwd_cnt)++;
                if (ptr_rvlc->conceal_max_esc == 1311) {
                  ptr_rvlc->conceal_max_esc = bnds;
                }
              }
            }
            noise_energy += dpcm;
            ptr_scf_fwd[bnds] = noise_energy;
            ptr_rvlc->last_nrg = noise_energy;
          }
          ptr_aac_dec_channel_info->str_pns_info.pns_used[bnds] = 1;
          break;

        default:
          ptr_rvlc->sf_used = 1;
          {
            memcpy(&temp_buf, it_bit_buff, sizeof(ia_bit_buf_struct));

            dpcm = ixheaacd_decode_rvlc_code_word(it_bit_buff, ptr_rvlc);
            if (dpcm < 0) {
              ptr_rvlc->conceal_max = bnds;
              return;
            }
            dpcm -= 7;
          }
          if ((dpcm == -7) || (dpcm == 7)) {
            if (ptr_rvlc->rvlc_esc_len) {
              ptr_rvlc->conceal_max = bnds;
              return;
            } else {
              if (dpcm == -7) {
                dpcm -= *ptr_scf_esc++;
              } else {
                dpcm += *ptr_scf_esc++;
              }
              (*ptr_esc_fwd_cnt)++;
              if (ptr_rvlc->conceal_max_esc == 1311) {
                ptr_rvlc->conceal_max_esc = bnds;
              }
            }
          }
          factor += dpcm;
          ptr_scf_fwd[bnds] = factor;
          ptr_rvlc->last_scale_fac = factor;
          break;
      }
    }
  }

  if (ptr_rvlc->intensity_used) {
    dpcm = ixheaacd_decode_rvlc_code_word(it_bit_buff, ptr_rvlc);
    if (dpcm < 0) {
      ptr_rvlc->conceal_max = bnds;
      return;
    }
    dpcm -= 7;
    if ((dpcm == -7) || (dpcm == 7)) {
      if (ptr_rvlc->rvlc_esc_len) {
        ptr_rvlc->conceal_max = bnds;
        return;
      } else {
        if (dpcm == -7) {
          dpcm -= *ptr_scf_esc++;
        } else {
          dpcm += *ptr_scf_esc++;
        }
        (*ptr_esc_fwd_cnt)++;
        if (ptr_rvlc->conceal_max_esc == 1311) {
          ptr_rvlc->conceal_max_esc = bnds;
        }
      }
    }
    ptr_rvlc->dpcm_is_last_pos = dpcm;
  }
}

static VOID ixheaacd_rvlc_decode_backward(
    ia_rvlc_info_struct *ptr_rvlc,
    ia_aac_dec_channel_info_struct *ptr_aac_dec_channel_info,
    ia_bit_buf_struct *it_bit_buff) {
  WORD16 band, group, dpcm;
  WORD16 bnds = ptr_rvlc->max_sfb_transmitted - 1;

  WORD16 factor = ptr_rvlc->rev_global_gain;
  WORD16 position = ptr_rvlc->dpcm_is_last_pos;
  WORD16 noise_energy =
      ptr_rvlc->rev_global_gain + ptr_rvlc->dpcm_noise_last_pos - 90 - 256;

  WORD16 *ptr_scf_bwd = ptr_aac_dec_channel_info->rvlc_scf_bwd_arr;
  WORD16 *ptr_scf_esc = ptr_aac_dec_channel_info->rvlc_scf_esc_arr;
  UWORD8 *ptr_esc_cnt = &(ptr_rvlc->num_esc_words_decoded);
  UWORD8 *ptr_esc_bwd_cnt = &(ptr_rvlc->num_bwd_esc_words_decoded);

  ptr_rvlc->ptr_rvl_bit_cnt = &(ptr_rvlc->rvlc_sf_bwd_len);
  ptr_rvlc->ptr_rvl_bit_str_idx = &(ptr_rvlc->rvl_bwd_bit_str_idx);

  *ptr_esc_bwd_cnt = 0;
  ptr_rvlc->direction = 1;
  ptr_scf_esc += *ptr_esc_cnt - 1;
  ptr_rvlc->firt_scale_fac = 0;
  ptr_rvlc->first_nrg = 0;
  ptr_rvlc->is_first = 0;

  if (ptr_rvlc->intensity_used) {
    dpcm = ixheaacd_decode_rvlc_code_word(it_bit_buff, ptr_rvlc);
    if (dpcm < 0) {
      ptr_rvlc->dpcm_is_last_pos = 0;
      ptr_rvlc->conceal_min = bnds;
      return;
    }
    dpcm -= 7;
    if ((dpcm == -7) || (dpcm == 7)) {
      if (ptr_rvlc->rvlc_esc_len) {
        ptr_rvlc->conceal_min = bnds;
        return;
      } else {
        if (dpcm == -7) {
          dpcm -= *ptr_scf_esc--;
        } else {
          dpcm += *ptr_scf_esc--;
        }
        (*ptr_esc_bwd_cnt)++;
        if (ptr_rvlc->conceal_min_esc == -1311) {
          ptr_rvlc->conceal_min_esc = bnds;
        }
      }
    }
    ptr_rvlc->dpcm_is_last_pos = dpcm;
  }

  for (group = ptr_rvlc->num_wind_grps - 1; group >= 0; group--) {
    for (band = ptr_rvlc->max_sfb_transmitted - 1; band >= 0; band--) {
      bnds = 16 * group + band;

      switch (ptr_aac_dec_channel_info->ptr_code_book[bnds]) {
        case ZERO_HCB:
          ptr_scf_bwd[bnds] = 0;
          break;

        case INTENSITY_HCB2:
        case INTENSITY_HCB:

          dpcm = ixheaacd_decode_rvlc_code_word(it_bit_buff, ptr_rvlc);
          if (dpcm < 0) {
            ptr_scf_bwd[bnds] = position;

            return;
          }
          dpcm -= 7;
          if ((dpcm == -7) || (dpcm == 7)) {
            if (ptr_rvlc->rvlc_esc_len) {
              ptr_scf_bwd[bnds] = position;

              return;
            } else {
              if (dpcm == -7) {
                dpcm -= *ptr_scf_esc--;
              } else {
                dpcm += *ptr_scf_esc--;
              }
              (*ptr_esc_bwd_cnt)++;
              if (ptr_rvlc->conceal_min_esc == -1311) {
              }
            }
          }
          ptr_scf_bwd[bnds] = position;
          position -= dpcm;
          ptr_rvlc->is_first = position;
          break;

        case NOISE_HCB:
          if (bnds == ptr_rvlc->first_noise_band) {
            ptr_scf_bwd[bnds] = ptr_rvlc->dpcm_noise_nrg +
                                ptr_aac_dec_channel_info->global_gain - 90 -
                                256;
            ptr_rvlc->first_nrg = ptr_scf_bwd[bnds];
          } else {
            dpcm = ixheaacd_decode_rvlc_code_word(it_bit_buff, ptr_rvlc);
            if (dpcm < 0) {
              ptr_scf_bwd[bnds] = noise_energy;
              return;
            }
            dpcm -= 7;
            if ((dpcm == -7) || (dpcm == 7)) {
              if (ptr_rvlc->rvlc_esc_len) {
                ptr_scf_bwd[bnds] = noise_energy;
                return;
              } else {
                if (dpcm == -7) {
                  dpcm -= *ptr_scf_esc--;
                } else {
                  dpcm += *ptr_scf_esc--;
                }
                (*ptr_esc_bwd_cnt)++;
                if (ptr_rvlc->conceal_min_esc == -1311) {
                }
              }
            }
            ptr_scf_bwd[bnds] = noise_energy;
            noise_energy -= dpcm;
            ptr_rvlc->first_nrg = noise_energy;
          }
          break;

        default:
          dpcm = ixheaacd_decode_rvlc_code_word(it_bit_buff, ptr_rvlc);
          if (dpcm < 0) {
            ptr_scf_bwd[bnds] = factor;

            return;
          }
          dpcm -= 7;
          if ((dpcm == -7) || (dpcm == 7)) {
            if (ptr_rvlc->rvlc_esc_len) {
              ptr_scf_bwd[bnds] = factor;

              return;
            } else {
              if (dpcm == -7) {
                dpcm -= *ptr_scf_esc--;
              } else {
                dpcm += *ptr_scf_esc--;
              }
              (*ptr_esc_bwd_cnt)++;
              if (ptr_rvlc->conceal_min_esc == -1311) {
              }
            }
          }
          ptr_scf_bwd[bnds] = factor;
          factor -= dpcm;
          ptr_rvlc->firt_scale_fac = factor;
          break;
      }
    }
  }
}

VOID ixheaacd_rvlc_read(
    ia_bit_buf_struct *it_bit_buff,
    ia_aac_dec_channel_info_struct *ptr_aac_dec_channel_info) {
  ia_rvlc_info_struct *ptr_rvlc = &ptr_aac_dec_channel_info->ptr_rvlc_info;

  WORD32 group, band;

  ptr_rvlc->num_wind_grps =
      ptr_aac_dec_channel_info->str_ics_info.num_window_groups;
  ptr_rvlc->max_sfb_transmitted =
      ptr_aac_dec_channel_info->str_ics_info.max_sfb;
  ptr_rvlc->noise_used = 0;
  ptr_rvlc->dpcm_noise_nrg = 0;
  ptr_rvlc->dpcm_noise_last_pos = 0;
  ptr_rvlc->rvlc_esc_len = -1;
  ptr_rvlc->dpcm_is_last_pos = 0;

  ptr_rvlc->sf_concealment = ixheaacd_read_bits_buf(it_bit_buff, 1);
  ptr_rvlc->rev_global_gain = ixheaacd_read_bits_buf(it_bit_buff, 8);

  if (ptr_aac_dec_channel_info->str_ics_info.window_sequence ==
      EIGHT_SHORT_SEQUENCE) {
    ptr_rvlc->rvlc_sf_len = ixheaacd_read_bits_buf(it_bit_buff, 11);
  } else {
    ptr_rvlc->rvlc_sf_len = ixheaacd_read_bits_buf(it_bit_buff, 9);
  }

  for (group = 0; group < ptr_rvlc->num_wind_grps; group++) {
    for (band = 0; band < ptr_rvlc->max_sfb_transmitted; band++) {
      if (ptr_aac_dec_channel_info->ptr_code_book[16 * group + band] ==
          NOISE_HCB) {
        ptr_rvlc->noise_used = 1;
        break;
      }
    }
  }

  if (ptr_rvlc->noise_used)
    ptr_rvlc->dpcm_noise_nrg = ixheaacd_read_bits_buf(it_bit_buff, 9);

  ptr_rvlc->sf_esc_present = ixheaacd_read_bits_buf(it_bit_buff, 1);

  if (ptr_rvlc->sf_esc_present) {
    ptr_rvlc->rvlc_esc_len = ixheaacd_read_bits_buf(it_bit_buff, 8);
  }

  if (ptr_rvlc->noise_used) {
    ptr_rvlc->dpcm_noise_last_pos = ixheaacd_read_bits_buf(it_bit_buff, 9);
    ptr_rvlc->rvlc_sf_len -= 9;
  }

  ptr_rvlc->rvlc_sf_fwd_len = ptr_rvlc->rvlc_sf_len;
  ptr_rvlc->rvlc_sf_bwd_len = ptr_rvlc->rvlc_sf_len;
}

VOID ixheaacd_hcr_read(ia_bit_buf_struct *it_bit_buff,
                       ia_aac_dec_channel_info_struct *ptr_aac_dec_channel_info,
                       WORD32 ele_type) {
  WORD16 len_reordered_spec_data;
  WORD8 len_longest_code_word;

  ptr_aac_dec_channel_info->reorder_spect_data_len = 0;
  ptr_aac_dec_channel_info->longest_cw_len = 0;

  len_reordered_spec_data = ixheaacd_read_bits_buf(it_bit_buff, 14);
  if (ele_type == ID_CPE) {
    if ((len_reordered_spec_data >= 0) && (len_reordered_spec_data <= 12288)) {
      ptr_aac_dec_channel_info->reorder_spect_data_len =
          len_reordered_spec_data;
    } else {
      if (len_reordered_spec_data > 12288) {
        ptr_aac_dec_channel_info->reorder_spect_data_len = 12288;
      }
    }
  } else if (ele_type == ID_SCE || ele_type == ID_LFE || ele_type == ID_CCE) {
    if ((len_reordered_spec_data >= 0) && (len_reordered_spec_data <= 6144)) {
      ptr_aac_dec_channel_info->reorder_spect_data_len =
          len_reordered_spec_data;
    } else {
      if (len_reordered_spec_data > 6144) {
        ptr_aac_dec_channel_info->reorder_spect_data_len = 6144;
      }
    }
  }

  len_longest_code_word = ixheaacd_read_bits_buf(it_bit_buff, 6);
  if ((len_longest_code_word >= 0) && (len_longest_code_word <= 49)) {
    ptr_aac_dec_channel_info->longest_cw_len = len_longest_code_word;
  } else {
    if (len_longest_code_word > 49) {
      ptr_aac_dec_channel_info->longest_cw_len = 49;
    }
  }
}

static IA_ERRORCODE ixheaacd_rvlc_init(
    ia_rvlc_info_struct *ptr_rvlc,
    ia_aac_dec_channel_info_struct *ptr_aac_dec_channel_info,
    ia_bit_buf_struct *it_bit_buff) {
  WORD16 *ptr_scf_esc = ptr_aac_dec_channel_info->rvlc_scf_esc_arr;
  WORD16 *ptr_scf_fwd = ptr_aac_dec_channel_info->rvlc_scf_fwd_arr;
  WORD16 *ptr_scf_bwd = ptr_aac_dec_channel_info->rvlc_scf_bwd_arr;
  WORD16 *ptr_scale_factor = ptr_aac_dec_channel_info->ptr_scale_factor;
  WORD32 bnds;

  ptr_aac_dec_channel_info->rvlc_intensity_used = 0;

  ptr_rvlc->num_esc_words_decoded = 0;
  ptr_rvlc->num_fwd_esc_words_decoded = 0;
  ptr_rvlc->num_bwd_esc_words_decoded = 0;

  ptr_rvlc->intensity_used = 0;
  ptr_rvlc->rvlc_err_log = 0;

  ptr_rvlc->conceal_max = CONCEAL_MAX_INIT;
  ptr_rvlc->conceal_min = CONCEAL_MIN_INIT;

  ptr_rvlc->conceal_max_esc = CONCEAL_MAX_INIT;
  ptr_rvlc->conceal_min_esc = CONCEAL_MIN_INIT;

  for (bnds = 0; bnds < RVLC_MAX_SFB; bnds++) {
    ptr_scf_fwd[bnds] = 0;
    ptr_scf_bwd[bnds] = 0;
    ptr_scf_esc[bnds] = 0;
    ptr_scale_factor[bnds] = 0;
  }

  ptr_rvlc->rvl_fwd_bit_str_idx = it_bit_buff->size - it_bit_buff->cnt_bits;
  ptr_rvlc->rvl_bwd_bit_str_idx =
      it_bit_buff->size - it_bit_buff->cnt_bits + ptr_rvlc->rvlc_sf_len - 1;

  it_bit_buff->cnt_bits -= ptr_rvlc->rvlc_sf_len;
  it_bit_buff->ptr_read_next =
      it_bit_buff->ptr_bit_buf_base +
      ((it_bit_buff->size - it_bit_buff->cnt_bits) >> 3);
  it_bit_buff->bit_pos = ((it_bit_buff->size - it_bit_buff->cnt_bits) & 7);

  if (ptr_rvlc->sf_esc_present != 0) {
    ptr_rvlc->esc_bit_str_idx = it_bit_buff->size - it_bit_buff->cnt_bits;

    it_bit_buff->cnt_bits -= ptr_rvlc->rvlc_esc_len;
    it_bit_buff->ptr_read_next =
        it_bit_buff->ptr_bit_buf_base +
        ((it_bit_buff->size - it_bit_buff->cnt_bits) >> 3);
    it_bit_buff->bit_pos = ((it_bit_buff->size - it_bit_buff->cnt_bits) & 7);
  }
  if (it_bit_buff->cnt_bits < 0) {
    return IA_XHEAAC_DEC_EXE_NONFATAL_INSUFFICIENT_INPUT_BYTES;
  } else
    return IA_NO_ERROR;
}

VOID ixheaacd_bi_dir_est_scf_prev_frame_reference(
    ia_aac_dec_channel_info_struct *ptr_aac_dec_channel_info,
    ia_aac_dec_overlap_info *ptr_aac_dec_static_channel_info) {
  ia_rvlc_info_struct *ptr_rvlc = &ptr_aac_dec_channel_info->ptr_rvlc_info;
  WORD32 band, bnds, start_band, end_band, group;
  WORD32 conceal_min, conceal_max;
  WORD32 conceal_group_min, conceal_group_max;
  WORD32 max_scf_bands;
  WORD32 common_min;

  if (ptr_aac_dec_channel_info->str_ics_info.window_sequence ==
      EIGHT_SHORT_SEQUENCE) {
    max_scf_bands = 16;
  } else {
    max_scf_bands = 64;
  }

  if (ptr_rvlc->conceal_min == CONCEAL_MIN_INIT) ptr_rvlc->conceal_min = 0;

  if (ptr_rvlc->conceal_max == CONCEAL_MAX_INIT)
    ptr_rvlc->conceal_max =
        (ptr_rvlc->num_wind_grps - 1) * 16 + ptr_rvlc->max_sfb_transmitted - 1;

  conceal_min = ptr_rvlc->conceal_min % max_scf_bands;
  conceal_group_min = ptr_rvlc->conceal_min / max_scf_bands;
  conceal_max = ptr_rvlc->conceal_max % max_scf_bands;
  conceal_group_max = ptr_rvlc->conceal_max / max_scf_bands;

  ptr_aac_dec_channel_info->rvlc_scf_fwd_arr[ptr_rvlc->conceal_max] =
      ptr_aac_dec_channel_info->rvlc_scf_bwd_arr[ptr_rvlc->conceal_max];
  ptr_aac_dec_channel_info->rvlc_scf_bwd_arr[ptr_rvlc->conceal_min] =
      ptr_aac_dec_channel_info->rvlc_scf_fwd_arr[ptr_rvlc->conceal_min];

  start_band = conceal_min;
  if (conceal_group_min == conceal_group_max)
    end_band = conceal_max;
  else
    end_band = ptr_rvlc->max_sfb_transmitted - 1;

  for (group = conceal_group_min; group <= conceal_group_max; group++) {
    for (band = start_band; band <= end_band; band++) {
      bnds = 16 * group + band;
      switch (ptr_aac_dec_channel_info->ptr_code_book[bnds]) {
        case ZERO_HCB:
          ptr_aac_dec_channel_info->ptr_scale_factor[bnds] = 0;
          break;

        case INTENSITY_HCB:
        case INTENSITY_HCB2:
          if ((ptr_aac_dec_static_channel_info->rvlc_prev_cb[bnds] ==
               INTENSITY_HCB) ||
              (ptr_aac_dec_static_channel_info->rvlc_prev_cb[bnds] ==
               INTENSITY_HCB2)) {
            common_min = ixheaac_min32(
                ptr_aac_dec_channel_info->rvlc_scf_fwd_arr[bnds],
                ptr_aac_dec_channel_info->rvlc_scf_bwd_arr[bnds]);
            ptr_aac_dec_channel_info->ptr_scale_factor[bnds] = ixheaac_min32(
                common_min,
                ptr_aac_dec_static_channel_info->rvlc_prev_sf[bnds]);
          } else {
            ptr_aac_dec_channel_info->ptr_scale_factor[bnds] = ixheaac_min32(
                ptr_aac_dec_channel_info->rvlc_scf_fwd_arr[bnds],
                ptr_aac_dec_channel_info->rvlc_scf_bwd_arr[bnds]);
          }
          break;

        case NOISE_HCB:
          if (ptr_aac_dec_static_channel_info->rvlc_prev_cb[bnds] ==
              NOISE_HCB) {
            common_min = ixheaac_min32(
                ptr_aac_dec_channel_info->rvlc_scf_fwd_arr[bnds],
                ptr_aac_dec_channel_info->rvlc_scf_bwd_arr[bnds]);
            ptr_aac_dec_channel_info->ptr_scale_factor[bnds] = ixheaac_min32(
                common_min,
                ptr_aac_dec_static_channel_info->rvlc_prev_sf[bnds]);
          } else {
            ptr_aac_dec_channel_info->ptr_scale_factor[bnds] = ixheaac_min32(
                ptr_aac_dec_channel_info->rvlc_scf_fwd_arr[bnds],
                ptr_aac_dec_channel_info->rvlc_scf_bwd_arr[bnds]);
          }
          break;

        default:
          if ((ptr_aac_dec_static_channel_info->rvlc_prev_cb[bnds] !=
               ZERO_HCB) &&
              (ptr_aac_dec_static_channel_info->rvlc_prev_cb[bnds] !=
               NOISE_HCB) &&
              (ptr_aac_dec_static_channel_info->rvlc_prev_cb[bnds] !=
               INTENSITY_HCB) &&
              (ptr_aac_dec_static_channel_info->rvlc_prev_cb[bnds] !=
               INTENSITY_HCB2)) {
            common_min = ixheaac_min32(
                ptr_aac_dec_channel_info->rvlc_scf_fwd_arr[bnds],
                ptr_aac_dec_channel_info->rvlc_scf_bwd_arr[bnds]);
            ptr_aac_dec_channel_info->ptr_scale_factor[bnds] = ixheaac_min32(
                common_min,
                ptr_aac_dec_static_channel_info->rvlc_prev_sf[bnds]);
          } else {
            ptr_aac_dec_channel_info->ptr_scale_factor[bnds] = ixheaac_min32(
                ptr_aac_dec_channel_info->rvlc_scf_fwd_arr[bnds],
                ptr_aac_dec_channel_info->rvlc_scf_bwd_arr[bnds]);
          }
          break;
      }
    }
    start_band = 0;
    if ((group + 1) == conceal_group_max) end_band = conceal_max;
  }

  if (conceal_group_min == 0)
    end_band = conceal_min;
  else
    end_band = ptr_rvlc->max_sfb_transmitted;
  for (group = 0; group <= conceal_group_min; group++) {
    for (band = 0; band < end_band; band++) {
      bnds = 16 * group + band;
      ptr_aac_dec_channel_info->ptr_scale_factor[bnds] =
          ptr_aac_dec_channel_info->rvlc_scf_fwd_arr[bnds];
    }
    if ((group + 1) == conceal_group_min) end_band = conceal_min;
  }

  start_band = conceal_max + 1;
  for (group = conceal_group_max; group < ptr_rvlc->num_wind_grps; group++) {
    for (band = start_band; band < ptr_rvlc->max_sfb_transmitted; band++) {
      bnds = 16 * group + band;
      ptr_aac_dec_channel_info->ptr_scale_factor[bnds] =
          ptr_aac_dec_channel_info->rvlc_scf_bwd_arr[bnds];
    }
    start_band = 0;
  }
}

static VOID ixheaacd_calc_ref_val_fwd(
    ia_rvlc_info_struct *ptr_rvlc,
    ia_aac_dec_channel_info_struct *ptr_aac_dec_channel_info, WORD32 *ref_fwd,
    WORD32 *ref_nrg_fwd, WORD32 *ref_scf_fwd) {
  WORD32 band, bnds, group, start_band;
  WORD32 id_is, id_nrg, id_scf;
  WORD32 conceal_min, conceal_group_min;
  WORD32 max_scf_bands;

  if (ptr_aac_dec_channel_info->str_ics_info.window_sequence ==
      EIGHT_SHORT_SEQUENCE)
    max_scf_bands = 16;
  else
    max_scf_bands = 64;

  conceal_min = ptr_rvlc->conceal_min % max_scf_bands;
  conceal_group_min = ptr_rvlc->conceal_min / max_scf_bands;

  id_is = id_nrg = id_scf = 1;

  *ref_nrg_fwd = ptr_aac_dec_channel_info->global_gain - 90 - 256;
  *ref_scf_fwd = ptr_aac_dec_channel_info->global_gain;

  start_band = conceal_min - 1;
  for (group = conceal_group_min; group >= 0; group--) {
    for (band = start_band; band >= 0; band--) {
      bnds = 16 * group + band;
      switch (ptr_aac_dec_channel_info->ptr_code_book[bnds]) {
        case ZERO_HCB:
          break;
        case INTENSITY_HCB:
        case INTENSITY_HCB2:
          if (id_is) {
            *ref_fwd = ptr_aac_dec_channel_info->rvlc_scf_fwd_arr[bnds];
            id_is = 0;
          }
          break;
        case NOISE_HCB:
          if (id_nrg) {
            *ref_nrg_fwd = ptr_aac_dec_channel_info->rvlc_scf_fwd_arr[bnds];
            id_nrg = 0;
          }
          break;
        default:
          if (id_scf) {
            *ref_scf_fwd = ptr_aac_dec_channel_info->rvlc_scf_fwd_arr[bnds];
            id_scf = 0;
          }
          break;
      }
    }
    start_band = ptr_rvlc->max_sfb_transmitted - 1;
  }
}

static VOID ixheaacd_calc_ref_val_bwd(
    ia_rvlc_info_struct *ptr_rvlc,
    ia_aac_dec_channel_info_struct *ptr_aac_dec_channel_info, WORD32 *ref_bwd,
    WORD32 *ref_nrg_bwd, WORD32 *ref_scf_bwd) {
  WORD32 band, bnds, group, start_band;
  WORD32 id_is, id_nrg, id_scf;
  WORD32 conceal_max, conceal_group_max;
  WORD32 max_scf_bands;

  if (ptr_aac_dec_channel_info->str_ics_info.window_sequence ==
      EIGHT_SHORT_SEQUENCE)
    max_scf_bands = 16;
  else
    max_scf_bands = 64;

  conceal_max = ptr_rvlc->conceal_max % max_scf_bands;
  conceal_group_max = ptr_rvlc->conceal_max / max_scf_bands;

  id_is = id_nrg = id_scf = 1;

  *ref_bwd = ptr_rvlc->dpcm_is_last_pos;
  *ref_nrg_bwd = ptr_rvlc->rev_global_gain + ptr_rvlc->dpcm_noise_last_pos -
                 90 - 256 + ptr_rvlc->dpcm_noise_nrg;
  *ref_scf_bwd = ptr_rvlc->rev_global_gain;

  start_band = conceal_max + 1;

  for (group = conceal_group_max; group < ptr_rvlc->num_wind_grps; group++) {
    for (band = start_band; band < ptr_rvlc->max_sfb_transmitted; band++) {
      bnds = 16 * group + band;
      switch (ptr_aac_dec_channel_info->ptr_code_book[bnds]) {
        case ZERO_HCB:
          break;
        case INTENSITY_HCB:
        case INTENSITY_HCB2:
          if (id_is) {
            *ref_bwd = ptr_aac_dec_channel_info->rvlc_scf_bwd_arr[bnds];
            id_is = 0;
          }
          break;
        case NOISE_HCB:
          if (id_nrg) {
            *ref_nrg_bwd = ptr_aac_dec_channel_info->rvlc_scf_bwd_arr[bnds];
            id_nrg = 0;
          }
          break;
        default:
          if (id_scf) {
            *ref_scf_bwd = ptr_aac_dec_channel_info->rvlc_scf_bwd_arr[bnds];
            id_scf = 0;
          }
          break;
      }
    }
    start_band = 0;
  }
}

VOID ixheaacd_bi_dir_est_lower_scf_cur_frame(
    ia_aac_dec_channel_info_struct *ptr_aac_dec_channel_info) {
  ia_rvlc_info_struct *ptr_rvlc = &ptr_aac_dec_channel_info->ptr_rvlc_info;
  WORD32 band, bnds, start_band, end_band, group;
  WORD32 conceal_min, conceal_max;
  WORD32 conceal_group_min, conceal_group_max;
  WORD32 max_scf_bands;

  if (ptr_aac_dec_channel_info->str_ics_info.window_sequence ==
      EIGHT_SHORT_SEQUENCE) {
    max_scf_bands = 16;
  } else {
    max_scf_bands = 64;
  }

  if (ptr_rvlc->conceal_min == CONCEAL_MIN_INIT) ptr_rvlc->conceal_min = 0;

  if (ptr_rvlc->conceal_max == CONCEAL_MAX_INIT)
    ptr_rvlc->conceal_max =
        (ptr_rvlc->num_wind_grps - 1) * 16 + ptr_rvlc->max_sfb_transmitted - 1;

  conceal_min = ptr_rvlc->conceal_min % max_scf_bands;
  conceal_group_min = ptr_rvlc->conceal_min / max_scf_bands;
  conceal_max = ptr_rvlc->conceal_max % max_scf_bands;
  conceal_group_max = ptr_rvlc->conceal_max / max_scf_bands;

  if (ptr_rvlc->conceal_min == ptr_rvlc->conceal_max) {
    WORD32 ref_fwd = 0, ref_nrg_fwd = 0, ref_scf_fwd = 0;
    WORD32 ref_bwd = 0, ref_nrg_bwd = 0, ref_scf_bwd = 0;

    bnds = ptr_rvlc->conceal_min;
    ixheaacd_calc_ref_val_fwd(ptr_rvlc, ptr_aac_dec_channel_info, &ref_fwd,
                              &ref_nrg_fwd, &ref_scf_fwd);
    ixheaacd_calc_ref_val_bwd(ptr_rvlc, ptr_aac_dec_channel_info, &ref_bwd,
                              &ref_nrg_bwd, &ref_scf_bwd);

    switch (ptr_aac_dec_channel_info->ptr_code_book[bnds]) {
      case ZERO_HCB:
        break;
      case INTENSITY_HCB:
      case INTENSITY_HCB2:
        if (ref_fwd < ref_bwd)
          ptr_aac_dec_channel_info->ptr_scale_factor[bnds] = ref_fwd;
        else
          ptr_aac_dec_channel_info->ptr_scale_factor[bnds] = ref_bwd;
        break;
      case NOISE_HCB:
        if (ref_nrg_fwd < ref_nrg_bwd)
          ptr_aac_dec_channel_info->ptr_scale_factor[bnds] = ref_nrg_fwd;
        else
          ptr_aac_dec_channel_info->ptr_scale_factor[bnds] = ref_nrg_bwd;
        break;
      default:
        if (ref_scf_fwd < ref_scf_bwd)
          ptr_aac_dec_channel_info->ptr_scale_factor[bnds] = ref_scf_fwd;
        else
          ptr_aac_dec_channel_info->ptr_scale_factor[bnds] = ref_scf_bwd;
        break;
    }
  } else {
    ptr_aac_dec_channel_info->rvlc_scf_fwd_arr[ptr_rvlc->conceal_max] =
        ptr_aac_dec_channel_info->rvlc_scf_bwd_arr[ptr_rvlc->conceal_max];
    ptr_aac_dec_channel_info->rvlc_scf_bwd_arr[ptr_rvlc->conceal_min] =
        ptr_aac_dec_channel_info->rvlc_scf_fwd_arr[ptr_rvlc->conceal_min];

    start_band = conceal_min;
    if (conceal_group_min == conceal_group_max)
      end_band = conceal_max;
    else
      end_band = ptr_rvlc->max_sfb_transmitted - 1;

    for (group = conceal_group_min; group <= conceal_group_max; group++) {
      for (band = start_band; band <= end_band; band++) {
        bnds = 16 * group + band;
        if (ptr_aac_dec_channel_info->rvlc_scf_fwd_arr[bnds] <
            ptr_aac_dec_channel_info->rvlc_scf_bwd_arr[bnds])
          ptr_aac_dec_channel_info->ptr_scale_factor[bnds] =
              ptr_aac_dec_channel_info->rvlc_scf_fwd_arr[bnds];
        else
          ptr_aac_dec_channel_info->ptr_scale_factor[bnds] =
              ptr_aac_dec_channel_info->rvlc_scf_bwd_arr[bnds];
      }
      start_band = 0;
      if ((group + 1) == conceal_group_max) end_band = conceal_max;
    }
  }

  if (conceal_group_min == 0)
    end_band = conceal_min;
  else
    end_band = ptr_rvlc->max_sfb_transmitted;
  for (group = 0; group <= conceal_group_min; group++) {
    for (band = 0; band < end_band; band++) {
      bnds = 16 * group + band;
      ptr_aac_dec_channel_info->ptr_scale_factor[bnds] =
          ptr_aac_dec_channel_info->rvlc_scf_fwd_arr[bnds];
    }
    if ((group + 1) == conceal_group_min) end_band = conceal_min;
  }

  start_band = conceal_max + 1;
  for (group = conceal_group_max; group < ptr_rvlc->num_wind_grps; group++) {
    for (band = start_band; band < ptr_rvlc->max_sfb_transmitted; band++) {
      bnds = 16 * group + band;
      ptr_aac_dec_channel_info->ptr_scale_factor[bnds] =
          ptr_aac_dec_channel_info->rvlc_scf_bwd_arr[bnds];
    }
    start_band = 0;
  }
}

VOID ixheaacd_statistical_estimation(
    ia_aac_dec_channel_info_struct *ptr_aac_dec_channel_info) {
  ia_rvlc_info_struct *ptr_rvlc = &ptr_aac_dec_channel_info->ptr_rvlc_info;
  WORD32 band, bnds, group;
  WORD32 sum_fwd, sum_bwd;
  WORD32 sum_nrg_fwd, sum_nrg_bwd;
  WORD32 sum_scf_fwd, sum_scf_bwd;
  WORD32 use_fwd, use_nrg_fwd, use_scf_fwd;

  sum_fwd = sum_bwd = sum_nrg_fwd = sum_nrg_bwd = sum_scf_fwd = sum_scf_bwd = 0;
  use_fwd = use_nrg_fwd = use_scf_fwd = 0;

  for (group = 0; group < ptr_rvlc->num_wind_grps; group++) {
    for (band = 0; band < ptr_rvlc->max_sfb_transmitted; band++) {
      bnds = 16 * group + band;
      switch (ptr_aac_dec_channel_info->ptr_code_book[bnds]) {
        case ZERO_HCB:
          break;

        case INTENSITY_HCB:
        case INTENSITY_HCB2:
          sum_fwd += ptr_aac_dec_channel_info->rvlc_scf_fwd_arr[bnds];
          sum_bwd += ptr_aac_dec_channel_info->rvlc_scf_bwd_arr[bnds];
          break;

        case NOISE_HCB:
          sum_nrg_fwd += ptr_aac_dec_channel_info->rvlc_scf_fwd_arr[bnds];
          sum_nrg_bwd += ptr_aac_dec_channel_info->rvlc_scf_bwd_arr[bnds];
          break;

        default:
          sum_scf_fwd += ptr_aac_dec_channel_info->rvlc_scf_fwd_arr[bnds];
          sum_scf_bwd += ptr_aac_dec_channel_info->rvlc_scf_bwd_arr[bnds];
          break;
      }
    }
  }

  if (sum_fwd < sum_bwd) use_fwd = 1;

  if (sum_nrg_fwd < sum_nrg_bwd) use_nrg_fwd = 1;

  if (sum_scf_fwd < sum_scf_bwd) use_scf_fwd = 1;

  for (group = 0; group < ptr_rvlc->num_wind_grps; group++) {
    for (band = 0; band < ptr_rvlc->max_sfb_transmitted; band++) {
      bnds = 16 * group + band;
      switch (ptr_aac_dec_channel_info->ptr_code_book[bnds]) {
        case ZERO_HCB:
          break;

        case INTENSITY_HCB:
        case INTENSITY_HCB2:
          if (use_fwd)
            ptr_aac_dec_channel_info->ptr_scale_factor[bnds] =
                ptr_aac_dec_channel_info->rvlc_scf_fwd_arr[bnds];
          else
            ptr_aac_dec_channel_info->ptr_scale_factor[bnds] =
                ptr_aac_dec_channel_info->rvlc_scf_bwd_arr[bnds];
          break;

        case NOISE_HCB:
          if (use_nrg_fwd)
            ptr_aac_dec_channel_info->ptr_scale_factor[bnds] =
                ptr_aac_dec_channel_info->rvlc_scf_fwd_arr[bnds];
          else
            ptr_aac_dec_channel_info->ptr_scale_factor[bnds] =
                ptr_aac_dec_channel_info->rvlc_scf_bwd_arr[bnds];
          break;

        default:
          if (use_scf_fwd)
            ptr_aac_dec_channel_info->ptr_scale_factor[bnds] =
                ptr_aac_dec_channel_info->rvlc_scf_fwd_arr[bnds];
          else
            ptr_aac_dec_channel_info->ptr_scale_factor[bnds] =
                ptr_aac_dec_channel_info->rvlc_scf_bwd_arr[bnds];
          break;
      }
    }
  }
}

VOID ixheaacd_predictive_interpolation(
    ia_aac_dec_channel_info_struct *ptr_aac_dec_channel_info,
    ia_aac_dec_overlap_info *ptr_aac_dec_static_channel_info) {
  ia_rvlc_info_struct *ptr_rvlc = &ptr_aac_dec_channel_info->ptr_rvlc_info;
  WORD32 band, bnds, group;
  WORD32 common_min;

  for (group = 0; group < ptr_rvlc->num_wind_grps; group++) {
    for (band = 0; band < ptr_rvlc->max_sfb_transmitted; band++) {
      bnds = 16 * group + band;
      switch (ptr_aac_dec_channel_info->ptr_code_book[bnds]) {
        case ZERO_HCB:
          ptr_aac_dec_channel_info->ptr_scale_factor[bnds] = 0;
          break;

        case INTENSITY_HCB:
        case INTENSITY_HCB2:
          if ((ptr_aac_dec_static_channel_info->rvlc_prev_cb[bnds] ==
               INTENSITY_HCB) ||
              (ptr_aac_dec_static_channel_info->rvlc_prev_cb[bnds] ==
               INTENSITY_HCB2)) {
            common_min = ixheaac_min32(
                ptr_aac_dec_channel_info->rvlc_scf_fwd_arr[bnds],
                ptr_aac_dec_channel_info->rvlc_scf_bwd_arr[bnds]);
            ptr_aac_dec_channel_info->ptr_scale_factor[bnds] = ixheaac_min32(
                common_min,
                ptr_aac_dec_static_channel_info->rvlc_prev_sf[bnds]);
          } else {
            ptr_aac_dec_channel_info->ptr_scale_factor[bnds] = -110;
          }
          break;

        case NOISE_HCB:
          if (ptr_aac_dec_static_channel_info->rvlc_prev_cb[bnds] ==
              NOISE_HCB) {
            common_min = ixheaac_min32(
                ptr_aac_dec_channel_info->rvlc_scf_fwd_arr[bnds],
                ptr_aac_dec_channel_info->rvlc_scf_bwd_arr[bnds]);
            ptr_aac_dec_channel_info->ptr_scale_factor[bnds] = ixheaac_min32(
                common_min,
                ptr_aac_dec_static_channel_info->rvlc_prev_sf[bnds]);
          } else {
            ptr_aac_dec_channel_info->ptr_scale_factor[bnds] = -110;
          }
          break;

        default:
          if ((ptr_aac_dec_static_channel_info->rvlc_prev_cb[bnds] !=
               ZERO_HCB) &&
              (ptr_aac_dec_static_channel_info->rvlc_prev_cb[bnds] !=
               NOISE_HCB) &&
              (ptr_aac_dec_static_channel_info->rvlc_prev_cb[bnds] !=
               INTENSITY_HCB) &&
              (ptr_aac_dec_static_channel_info->rvlc_prev_cb[bnds] !=
               INTENSITY_HCB2)) {
            common_min = ixheaac_min32(
                ptr_aac_dec_channel_info->rvlc_scf_fwd_arr[bnds],
                ptr_aac_dec_channel_info->rvlc_scf_bwd_arr[bnds]);
            ptr_aac_dec_channel_info->ptr_scale_factor[bnds] = ixheaac_min32(
                common_min,
                ptr_aac_dec_static_channel_info->rvlc_prev_sf[bnds]);
          } else {
            ptr_aac_dec_channel_info->ptr_scale_factor[bnds] = 0;
          }
          break;
      }
    }
  }
}
static VOID ixheaacd_rvlc_final_error_detection(
    ia_aac_dec_channel_info_struct *ptr_aac_dec_channel_info,
    ia_aac_dec_overlap_info *ptr_aac_dec_static_channel_info) {
  ia_rvlc_info_struct *ptr_rvlc = &ptr_aac_dec_channel_info->ptr_rvlc_info;
  UWORD8 err_status_complete = 0;
  UWORD8 err_status_length_fwd = 0;
  UWORD8 err_status_length_bwd = 0;
  UWORD8 err_status_length_escape = 0;
  UWORD8 err_status_first_scf = 0;
  UWORD8 err_status_last_scf = 0;
  UWORD8 err_status_first_nrg = 0;
  UWORD8 err_status_last_nrg = 0;
  UWORD8 err_status_first_is = 0;
  UWORD8 err_status_last_is = 0;
  UWORD8 err_status_forbidden_cw_fwd = 0;
  UWORD8 err_status_forbidden_cw_bwd = 0;
  UWORD8 err_status_num_escapes_fwd = 0;
  UWORD8 err_status_num_escapes_bwd = 0;
  UWORD8 conceal_status = 1;
  UWORD8 current_block_type;

  ptr_aac_dec_channel_info->rvlc_curr_sf_flag = 1;

  if (ptr_rvlc->rvlc_err_log & RVLC_ERROR_FORBIDDEN_CW_DETECTED_FWD)
    err_status_forbidden_cw_fwd = 1;

  if (ptr_rvlc->rvlc_err_log & RVLC_ERROR_FORBIDDEN_CW_DETECTED_BWD)
    err_status_forbidden_cw_bwd = 1;

  if (ptr_rvlc->rvlc_sf_fwd_len) err_status_length_fwd = 1;

  if (ptr_rvlc->rvlc_sf_bwd_len) err_status_length_bwd = 1;

  if (ptr_rvlc->sf_esc_present)
    if (ptr_rvlc->rvlc_esc_len) err_status_length_escape = 1;

  if (ptr_rvlc->sf_used) {
    if (ptr_rvlc->firt_scale_fac != (ptr_aac_dec_channel_info->global_gain))
      err_status_first_scf = 1;

    if (ptr_rvlc->last_scale_fac != (ptr_rvlc->rev_global_gain))
      err_status_last_scf = 1;
  }

  if (ptr_rvlc->noise_used) {
    if (ptr_rvlc->first_nrg != (ptr_aac_dec_channel_info->global_gain +
                                ptr_rvlc->dpcm_noise_nrg - 90 - 256))
      err_status_first_nrg = 1;

    if (ptr_rvlc->last_nrg !=
        (ptr_rvlc->rev_global_gain + ptr_rvlc->dpcm_noise_last_pos - 90 - 256))
      err_status_last_nrg = 1;
  }

  if (ptr_rvlc->intensity_used) {
    if (ptr_rvlc->is_first != 0) err_status_first_is = 1;

    if (ptr_rvlc->is_last != (ptr_rvlc->dpcm_is_last_pos))
      err_status_last_is = 1;
  }

  if ((ptr_rvlc->num_fwd_esc_words_decoded !=
       ptr_rvlc->num_esc_words_decoded) &&
      (ptr_rvlc->conceal_max == CONCEAL_MAX_INIT)) {
    err_status_num_escapes_fwd = 1;
  }

  if ((ptr_rvlc->num_bwd_esc_words_decoded !=
       ptr_rvlc->num_esc_words_decoded) &&
      (ptr_rvlc->conceal_min == CONCEAL_MIN_INIT)) {
    err_status_num_escapes_bwd = 1;
  }

  if (err_status_length_escape ||
      (((ptr_rvlc->conceal_max == CONCEAL_MAX_INIT) &&
        (ptr_rvlc->num_fwd_esc_words_decoded !=
         ptr_rvlc->num_esc_words_decoded) &&
        (err_status_last_scf || err_status_last_nrg || err_status_last_is))

       &&

       ((ptr_rvlc->conceal_min == CONCEAL_MIN_INIT) &&
        (ptr_rvlc->num_bwd_esc_words_decoded !=
         ptr_rvlc->num_esc_words_decoded) &&
        (err_status_first_scf || err_status_first_nrg ||
         err_status_first_is))) ||
      ((ptr_rvlc->conceal_max == CONCEAL_MAX_INIT) &&
       ((ptr_rvlc->rev_global_gain - ptr_rvlc->last_scale_fac) < -15)) ||
      ((ptr_rvlc->conceal_min == CONCEAL_MIN_INIT) &&
       ((ptr_aac_dec_channel_info->global_gain - ptr_rvlc->firt_scale_fac) <
        -15))) {
    if ((ptr_rvlc->conceal_max == CONCEAL_MAX_INIT) ||
        (ptr_rvlc->conceal_min == CONCEAL_MIN_INIT)) {
      ptr_rvlc->conceal_max = 0;
      ptr_rvlc->conceal_min =
          ixheaac_max32(0, (ptr_rvlc->num_wind_grps - 1) * 16 +
                                ptr_rvlc->max_sfb_transmitted - 1);
    } else {
      ptr_rvlc->conceal_max =
          ixheaac_min32(ptr_rvlc->conceal_max, ptr_rvlc->conceal_max_esc);
      ptr_rvlc->conceal_min =
          ixheaac_max32(ptr_rvlc->conceal_min, ptr_rvlc->conceal_min_esc);
    }
  }

  err_status_complete =
      err_status_last_scf || err_status_first_scf || err_status_last_nrg ||
      err_status_first_nrg || err_status_last_is || err_status_first_is ||
      err_status_forbidden_cw_fwd || err_status_forbidden_cw_bwd ||
      err_status_length_fwd || err_status_length_bwd ||
      err_status_length_escape || err_status_num_escapes_fwd ||
      err_status_num_escapes_bwd;

  current_block_type =
      (ptr_aac_dec_channel_info->str_ics_info.window_sequence ==
       EIGHT_SHORT_SEQUENCE)
          ? 0
          : 1;

  if (!err_status_complete) {
    WORD32 band;
    WORD32 group;
    WORD32 bnds;
    WORD32 last_sfb_idx;

    last_sfb_idx = (ptr_rvlc->num_wind_grps > 1) ? 16 : 64;

    for (group = 0; group < ptr_rvlc->num_wind_grps; group++) {
      for (band = 0; band < ptr_rvlc->max_sfb_transmitted; band++) {
        bnds = 16 * group + band;
        ptr_aac_dec_channel_info->ptr_scale_factor[bnds] =
            ptr_aac_dec_static_channel_info->rvlc_prev_sf[bnds] =
                ptr_aac_dec_channel_info->rvlc_scf_fwd_arr[bnds];
      }
    }

    for (group = 0; group < ptr_rvlc->num_wind_grps; group++) {
      for (band = 0; band < ptr_rvlc->max_sfb_transmitted; band++) {
        bnds = 16 * group + band;
        ptr_aac_dec_static_channel_info->rvlc_prev_cb[bnds] =
            ptr_aac_dec_channel_info->ptr_code_book[bnds];
      }
      for (; band < last_sfb_idx; band++) {
        bnds = 16 * group + band;
        ptr_aac_dec_static_channel_info->rvlc_prev_cb[bnds] = ZERO_HCB;
      }
    }
  } else {
    WORD32 band;
    WORD32 group;

    if (((ptr_rvlc->conceal_min != CONCEAL_MIN_INIT) ||
         (ptr_rvlc->conceal_max != CONCEAL_MAX_INIT)) &&
        (ptr_rvlc->conceal_min <= ptr_rvlc->conceal_max) &&
        (ptr_aac_dec_static_channel_info->rvlc_prev_blk_type ==
         current_block_type) &&
        ptr_aac_dec_static_channel_info->rvlc_prev_sf_ok &&
        ptr_rvlc->sf_concealment && conceal_status) {
      ixheaacd_bi_dir_est_scf_prev_frame_reference(
          ptr_aac_dec_channel_info, ptr_aac_dec_static_channel_info);
      conceal_status = 0;
    }

    if ((ptr_rvlc->conceal_min <= ptr_rvlc->conceal_max) &&
        ((ptr_rvlc->conceal_min != CONCEAL_MIN_INIT) ||
         (ptr_rvlc->conceal_max != CONCEAL_MAX_INIT)) &&
        !(ptr_aac_dec_static_channel_info->rvlc_prev_sf_ok &&
          ptr_rvlc->sf_concealment &&
          (ptr_aac_dec_static_channel_info->rvlc_prev_blk_type ==
           current_block_type)) &&
        conceal_status) {
      ixheaacd_bi_dir_est_lower_scf_cur_frame(ptr_aac_dec_channel_info);
      conceal_status = 0;
    }

    if ((ptr_rvlc->conceal_min <= ptr_rvlc->conceal_max) &&
        ((err_status_last_scf && err_status_first_scf) ||
         (err_status_last_nrg && err_status_first_nrg) ||
         (err_status_last_is && err_status_first_is)) &&
        !(err_status_forbidden_cw_fwd || err_status_forbidden_cw_bwd ||
          err_status_length_escape) &&
        conceal_status) {
      ixheaacd_statistical_estimation(ptr_aac_dec_channel_info);
      conceal_status = 0;
    }

    if ((ptr_rvlc->conceal_min <= ptr_rvlc->conceal_max) &&
        ptr_aac_dec_static_channel_info->rvlc_prev_sf_ok &&
        ptr_rvlc->sf_concealment &&
        (ptr_aac_dec_static_channel_info->rvlc_prev_blk_type ==
         current_block_type) &&
        conceal_status) {
      ixheaacd_predictive_interpolation(ptr_aac_dec_channel_info,
                                        ptr_aac_dec_static_channel_info);
      conceal_status = 0;
    }

    if (conceal_status) {
      for (group = 0; group < ptr_rvlc->num_wind_grps; group++) {
        for (band = 0; band < ptr_rvlc->max_sfb_transmitted; band++) {
          ptr_aac_dec_channel_info->ptr_scale_factor[16 * group + band] = 0;
        }
      }
      ptr_aac_dec_channel_info->rvlc_curr_sf_flag = 0;
    }
  }
}

IA_ERRORCODE ixheaacd_rvlc_dec(
    ia_aac_dec_channel_info_struct *ptr_aac_dec_channel_info,
    ia_aac_dec_overlap_info *ptr_aac_dec_static_channel_info,
    ia_bit_buf_struct *it_bit_buff) {
  ia_rvlc_info_struct *ptr_rvlc = &ptr_aac_dec_channel_info->ptr_rvlc_info;
  ia_bit_buf_struct saved_it_bit_buff;
  IA_ERRORCODE error_code = 0;
  error_code =
      ixheaacd_rvlc_init(ptr_rvlc, ptr_aac_dec_channel_info, it_bit_buff);
  if (error_code) return error_code;

  ixheaacd_bitbuf_checkpoint(*it_bit_buff, saved_it_bit_buff);
  if (ptr_rvlc->sf_esc_present)
    ixheaacd_rvlc_decode_escape(
        ptr_rvlc, ptr_aac_dec_channel_info->rvlc_scf_esc_arr, it_bit_buff);

  ixheaacd_rvlc_decode_forward(ptr_rvlc, ptr_aac_dec_channel_info, it_bit_buff);
  ixheaacd_rvlc_decode_backward(ptr_rvlc, ptr_aac_dec_channel_info,
                                it_bit_buff);
  ixheaacd_rvlc_final_error_detection(ptr_aac_dec_channel_info,
                                      ptr_aac_dec_static_channel_info);

  ptr_aac_dec_channel_info->rvlc_intensity_used = ptr_rvlc->intensity_used;
  ptr_aac_dec_channel_info->str_pns_info.pns_active = ptr_rvlc->noise_used;

  ixheaacd_bitbuf_restore(*it_bit_buff, saved_it_bit_buff);
  return error_code;
}
