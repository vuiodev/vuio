/******************************************************************************
 *                                                                            *
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

#include <limits.h>
#include <stddef.h>
#include "ixheaac_type_def.h"
#include "ixheaac_constants.h"
#include "impd_drc_common_enc.h"
#include "impd_drc_uni_drc.h"
#include "impd_drc_tables.h"
#include "impd_drc_api.h"
#include "ixheaace_api.h"
#include "ixheaace_aac_constants.h"
#include "ixheaac_error_standards.h"
#include "ixheaace_error_codes.h"
#include <stdlib.h>

#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops16.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaac_basic_ops.h"

#include "ixheaace_bitbuffer.h"
#include "ixheaace_psy_const.h"
#include "ixheaace_tns.h"
#include "ixheaace_tns_params.h"
#include "ixheaace_rom.h"
#include "ixheaace_common_rom.h"
#include "ixheaace_block_switch.h"
#include "ixheaace_psy_data.h"
#include "ixheaace_interface.h"
#include "ixheaace_bits_count.h"
#include "ixheaace_dynamic_bits.h"
#include "ixheaace_adjust_threshold_data.h"
#include "ixheaace_qc_data.h"
#include "ixheaace_write_bitstream.h"
#include "ixheaace_sbr_header.h"
#include "ixheaace_config.h"

#include "ixheaace_channel_map.h"
#include "ixheaace_write_adts_adif.h"

static VOID PLATFORM_INLINE
ia_enhaacplus_enc_encode_gain_control_data(ixheaace_bit_buf_handle pstr_bit_stream_handle)

{
  ixheaace_write_bits(pstr_bit_stream_handle, 0, 1);
}
static WORD32 ia_enhaacplus_enc_encode_spectral_data(
    WORD32 *pstr_sfb_offset, ixheaace_section_data *pstr_section_data, WORD16 *ptr_quant_spectrum,
    ixheaace_bit_buf_handle pstr_bit_stream_handle, ixheaace_huffman_tables *ptr_huffman_tbl) {
  WORD32 i, sfb;
  WORD32 dbg_val;

  dbg_val = ia_enhaacplus_enc_get_bits_available(pstr_bit_stream_handle);

  for (i = 0; i < pstr_section_data->num_of_sections; i++) {
    for (sfb = pstr_section_data->section[i].sfb_start;
         sfb < pstr_section_data->section[i].sfb_start + pstr_section_data->section[i].sfb_cnt;
         sfb++) {
      ia_enhaacplus_enc_code_values(ptr_quant_spectrum + pstr_sfb_offset[sfb],
                                    pstr_sfb_offset[sfb + 1] - pstr_sfb_offset[sfb],
                                    pstr_section_data->section[i].code_book,
                                    pstr_bit_stream_handle, ptr_huffman_tbl);
    }
  }

  return (ia_enhaacplus_enc_get_bits_available(pstr_bit_stream_handle) - dbg_val);
}

static VOID ia_enhaacplus_enc_encode_global_gain(WORD32 global_gain, WORD32 log_norm,
                                                 WORD32 scalefac,
                                                 ixheaace_bit_buf_handle pstr_bit_stream_handle) {
  ixheaace_write_bits(pstr_bit_stream_handle,
                      global_gain - scalefac + GLOBAL_GAIN_OFFSET - 4 * log_norm, 8);
}

static VOID ia_enhaacplus_enc_encode_ics_info(WORD32 block_type, WORD32 win_shape,
                                              WORD32 grouping_mask,
                                              ixheaace_section_data *pstr_section_data,
                                              ixheaace_bit_buf_handle pstr_bit_stream_handle,
                                              WORD32 aot) {
  WORD32 tmp;

  switch (aot) {
    case AOT_AAC_LD:
      tmp = ((ICS_RESERVED_BIT << 3) | (block_type << 1) | win_shape);
      ixheaace_write_bits(pstr_bit_stream_handle, tmp, 4);
      break;

    case AOT_AAC_LC:
    case AOT_SBR:
    case AOT_PS:
      tmp = ((ICS_RESERVED_BIT << 3) | (block_type << 1) | win_shape);
      ixheaace_write_bits(pstr_bit_stream_handle, tmp, 4);
      break;
  }
  switch (block_type) {
    case LONG_WINDOW:
    case START_WINDOW:
    case STOP_WINDOW:

      switch (aot) {
        case AOT_AAC_LC:
        case AOT_SBR:
        case AOT_PS:
          tmp = pstr_section_data->max_sfb_per_grp << 1;
          ixheaace_write_bits(pstr_bit_stream_handle, tmp, 7);
          break;

        case AOT_AAC_LD:
          tmp = pstr_section_data->max_sfb_per_grp << 1;
          ixheaace_write_bits(pstr_bit_stream_handle, tmp, 7);
          break;
        default:
          ixheaace_write_bits(pstr_bit_stream_handle, pstr_section_data->max_sfb_per_grp, 6);
      }
      break;

    case SHORT_WINDOW:

      // Write grouping bits

      tmp = pstr_section_data->max_sfb_per_grp << (TRANS_FAC - 1) | grouping_mask;
      ixheaace_write_bits(pstr_bit_stream_handle, tmp, TRANS_FAC - 1 + 4);

      break;
  }
}

static WORD32 ia_enhaacplus_enc_encode_section_data(
    ixheaace_section_data *pstr_section_data, ixheaace_bit_buf_handle pstr_bit_stream_handle) {
  WORD32 sect_escape_val = 0, sect_len_bits = 0;
  WORD32 sect_len;
  WORD32 i;
  WORD32 dbg_val = ia_enhaacplus_enc_get_bits_available(pstr_bit_stream_handle);

  switch (pstr_section_data->block_type) {
    case LONG_WINDOW:
    case START_WINDOW:
    case STOP_WINDOW:

      sect_escape_val = SECT_ESC_VAL_LONG;
      sect_len_bits = SECT_BITS_LONG;
      break;

    case SHORT_WINDOW:

      sect_escape_val = SECT_ESC_VAL_SHORT;
      sect_len_bits = SECT_BITS_SHORT;
      break;
  }

  for (i = 0; i < pstr_section_data->num_of_sections; i++) {
    ixheaace_write_bits(pstr_bit_stream_handle, pstr_section_data->section[i].code_book, 4);

    sect_len = pstr_section_data->section[i].sfb_cnt;

    while (sect_len >= sect_escape_val) {
      ixheaace_write_bits(pstr_bit_stream_handle, sect_escape_val, (UWORD8)sect_len_bits);

      sect_len -= sect_escape_val;
    }

    ixheaace_write_bits(pstr_bit_stream_handle, sect_len, (UWORD8)sect_len_bits);
  }

  return (ia_enhaacplus_enc_get_bits_available(pstr_bit_stream_handle) - dbg_val);
}

static VOID ia_enhaacplus_enc_code_scale_factor_delta_lav(WORD32 delta,
                                                            ixheaace_bit_buf_handle ptr_bitstream,
                                                            const UWORD16 *ptr_pltabscf,
                                                            const UWORD32 *ptr_pctabscf) {
  WORD32 code_word, code_length;

  code_word = ptr_pctabscf[delta];
  code_length = ptr_pltabscf[delta];

  ixheaace_write_bits(ptr_bitstream, code_word, (UWORD8)code_length);
}
static WORD32 ia_enhaacplus_enc_encode_scalefactor_data(
    UWORD16 *ptr_max_val_in_sfb, ixheaace_section_data *pstr_section_data, WORD16 *ptr_scalefac,
    ixheaace_bit_buf_handle pstr_bit_stream_handle, ixheaace_huffman_tables *pstr_huffman_tbl) {
  WORD32 i, j, last_val_scf, delta_scf;
  WORD32 dbg_val = ia_enhaacplus_enc_get_bits_available(pstr_bit_stream_handle);
  const UWORD16 *ptr_pltabscf = &pstr_huffman_tbl->huff_ltabscf[CODE_BCK_SCF_LAV];
  const UWORD32 *ptr_pctabscf = &pstr_huffman_tbl->huff_ctabscf[CODE_BCK_SCF_LAV];

  last_val_scf = ptr_scalefac[pstr_section_data->first_scf];

  for (i = 0; i < pstr_section_data->num_of_sections; i++) {
    if (pstr_section_data->section[i].code_book != CODE_BCK_ZERO_NO) {
      for (j = pstr_section_data->section[i].sfb_start;
           j < pstr_section_data->section[i].sfb_start + pstr_section_data->section[i].sfb_cnt;
           j++) {
        if (ptr_max_val_in_sfb[j] == 0) {
          delta_scf = 0;
        } else {
          delta_scf = -(ptr_scalefac[j] - last_val_scf);

          last_val_scf = ptr_scalefac[j];
        }
        ia_enhaacplus_enc_code_scale_factor_delta_lav(delta_scf, pstr_bit_stream_handle,
                                                          ptr_pltabscf, ptr_pctabscf);
      }
    }
  }

  return (ia_enhaacplus_enc_get_bits_available(pstr_bit_stream_handle) - dbg_val);
}

static VOID ia_enhaacplus_enc_encode_ms_info(WORD32 sfb_cnt, WORD32 sfb_grp, WORD32 max_sfb,
                                             WORD32 ms_digest, WORD32 *js_flags,
                                             ixheaace_bit_buf_handle pstr_bit_stream_handle) {
  WORD32 sfb, sfb_offset;
  UWORD32 tmp_var = 0;
  WORD32 *jsflag;
  UWORD8 num_of_bits, remaining = 0;

  switch (ms_digest) {
    case MS_NONE:
      ixheaace_write_bits(pstr_bit_stream_handle, SI_MS_MASK_NONE, 2);
      break;

    case MS_ALL:
      ixheaace_write_bits(pstr_bit_stream_handle, SI_MS_MASK_ALL, 2);
      break;

    case MS_SOME:
      ixheaace_write_bits(pstr_bit_stream_handle, SI_MS_MASK_SOME, 2);

      if (max_sfb > 32) {
        num_of_bits = 32;
        remaining = (UWORD8)(max_sfb - 32);
      } else
        num_of_bits = (UWORD8)max_sfb;

      for (sfb_offset = 0; sfb_offset < sfb_cnt; sfb_offset += sfb_grp) {
        WORD8 flag;
        jsflag = &js_flags[sfb_offset];
        tmp_var = 0;

        for (sfb = num_of_bits - 1; sfb >= 0; sfb--) {
          flag = (WORD8)(*jsflag++);
          tmp_var = ((tmp_var << 1) | flag);
        }

        ixheaace_write_bits(pstr_bit_stream_handle, tmp_var, num_of_bits);

        if (remaining) {
          for (sfb = remaining - 1; sfb >= 0; sfb--) {
            flag = (WORD8)(*jsflag++);
            tmp_var = ((tmp_var << 1) | flag);
          }

          ixheaace_write_bits(pstr_bit_stream_handle, tmp_var, remaining);
        }
      }

      break;
  }
}

static VOID ia_enhaacplus_enc_encode_tns_data(
    ixheaace_temporal_noise_shaping_params pstr_tns_info, WORD32 block_type,
    ixheaace_bit_buf_handle pstr_bit_stream_handle, WORD32 aot) {
  WORD32 i, k;
  WORD32 tns_present;
  WORD32 num_windows;
  WORD32 coef_bits;

  UWORD32 tmp;
  UWORD8 val;

  num_windows = (block_type == 2 ? TRANS_FAC : 1);

  tns_present = 0;

  for (i = 0; i < num_windows; i++) {
    if (pstr_tns_info.tns_active[i] == 1) {
      tns_present = 1;
    }
  }

  if (tns_present == 0) {
    ixheaace_write_bits(pstr_bit_stream_handle, 0, 1);

    if (AOT_AAC_LD == aot) {
      ia_enhaacplus_enc_encode_gain_control_data(pstr_bit_stream_handle);
    }
  } else {
    /* there is data to be written*/

    ixheaace_write_bits(pstr_bit_stream_handle, 1, 1); /* data_present */

    if (aot == AOT_AAC_LD) {
      ia_enhaacplus_enc_encode_gain_control_data(pstr_bit_stream_handle);
    }

    for (i = 0; i < num_windows; i++) {
      ixheaace_write_bits(pstr_bit_stream_handle, pstr_tns_info.tns_active[i],
                          (UWORD8)(block_type == 2 ? 1 : 2));

      if (pstr_tns_info.tns_active[i]) {
        tmp = ((pstr_tns_info.coef_res[i] == 4 ? 1 : 0)
                   << ((block_type == 2 ? 4 : 6) + (block_type == 2 ? 3 : 5)) |
               pstr_tns_info.length[i] << (block_type == 2 ? 3 : 5) | pstr_tns_info.order[i]);

        val = (UWORD8)(1 + (block_type == 2 ? 4 : 6) + (block_type == 2 ? 3 : 5));

        ixheaace_write_bits(pstr_bit_stream_handle, tmp, val);

        if (pstr_tns_info.order[i]) {
          ixheaace_write_bits(pstr_bit_stream_handle, FILTER_DIRECTION, 1);

          if (pstr_tns_info.coef_res[i] == 4) {
            coef_bits = 3;

            for (k = 0; k < pstr_tns_info.order[i]; k++) {
              if (pstr_tns_info.coef[i * TEMPORAL_NOISE_SHAPING_MAX_ORDER_SHORT + k] > 3 ||
                  pstr_tns_info.coef[i * TEMPORAL_NOISE_SHAPING_MAX_ORDER_SHORT + k] < -4) {
                coef_bits = 4;
                break;
              }
            }
          } else {
            coef_bits = 2;

            for (k = 0; k < pstr_tns_info.order[i]; k++) {
              if (pstr_tns_info.coef[i * TEMPORAL_NOISE_SHAPING_MAX_ORDER_SHORT + k] > 1 ||
                  pstr_tns_info.coef[i * TEMPORAL_NOISE_SHAPING_MAX_ORDER_SHORT + k] < -2) {
                coef_bits = 3;
                break;
              }
            }
          }

          ixheaace_write_bits(pstr_bit_stream_handle, -(coef_bits - pstr_tns_info.coef_res[i]),
                              1); /*coef_compres*/

          for (k = 0; k < pstr_tns_info.order[i]; k++) {
            static const WORD32 rmask[] = {0, 1, 3, 7, 15};

            ixheaace_write_bits(
                pstr_bit_stream_handle,
                pstr_tns_info.coef[i * TEMPORAL_NOISE_SHAPING_MAX_ORDER_SHORT + k] &
                    rmask[coef_bits],
                (UWORD8)coef_bits);
          }
        }
      }
    }
  }
}

static VOID PLATFORM_INLINE
ia_enhaacplus_enc_encode_pulse_data(ixheaace_bit_buf_handle pstr_bit_stream_handle) {
  ixheaace_write_bits(pstr_bit_stream_handle, 0, 1);
}

static IA_ERRORCODE ia_enhaacplus_enc_write_ic_stream(
    WORD32 common_window, WORD32 win_shape, WORD32 grouping_mask, WORD32 *pstr_sfb_offset,
    WORD16 *ptr_scf, UWORD16 *ptr_max_val_in_sfb, WORD32 global_gain, WORD16 *ptr_quant_spec,
    ixheaace_section_data *pstr_section_data, ixheaace_bit_buf_handle pstr_bit_stream_handle,
    WORD32 aot, ixheaace_temporal_noise_shaping_params pstr_tns_info,
    ixheaace_aac_tables *pstr_aac_tables) {
  WORD32 log_norm = -1;

  ia_enhaacplus_enc_encode_global_gain(
      global_gain, log_norm, ptr_scf[pstr_section_data->first_scf], pstr_bit_stream_handle);
  if (!common_window) {
    ia_enhaacplus_enc_encode_ics_info(pstr_section_data->block_type, win_shape, grouping_mask,
                                      pstr_section_data, pstr_bit_stream_handle, aot);
  }

  if (ia_enhaacplus_enc_encode_section_data(pstr_section_data, pstr_bit_stream_handle) !=
      pstr_section_data->side_info_bits) {
    return IA_EXHEAACE_EXE_FATAL_INVALID_SIDE_INFO_BITS;
  }

  if (ia_enhaacplus_enc_encode_scalefactor_data(
          ptr_max_val_in_sfb, pstr_section_data, ptr_scf, pstr_bit_stream_handle,
          pstr_aac_tables->pstr_huff_tab) != pstr_section_data->scale_fac_bits) {
    return IA_EXHEAACE_EXE_FATAL_INVALID_SCALE_FACTOR_BITS;
  }

  if (aot == AOT_AAC_LC || aot == AOT_AAC_LD || aot == AOT_SBR || aot == AOT_PS) {
    ia_enhaacplus_enc_encode_pulse_data(pstr_bit_stream_handle);
  }
  ia_enhaacplus_enc_encode_tns_data(pstr_tns_info, pstr_section_data->block_type,
                                    pstr_bit_stream_handle, aot);

  if (aot == AOT_AAC_LC || aot == AOT_SBR || aot == AOT_PS) {
    ia_enhaacplus_enc_encode_gain_control_data(pstr_bit_stream_handle);
  }

  if (ia_enhaacplus_enc_encode_spectral_data(
          pstr_sfb_offset, pstr_section_data, ptr_quant_spec, pstr_bit_stream_handle,
          pstr_aac_tables->pstr_huff_tab) != pstr_section_data->huffman_bits) {
    return IA_EXHEAACE_EXE_FATAL_INVALID_HUFFMAN_BITS;
  }
  return IA_NO_ERROR;
}

static IA_ERRORCODE ia_enhaacplus_enc_write_single_chan_elem(
    WORD32 instance_tag, WORD32 *pstr_sfb_offset, ixheaace_qc_out_channel *pstr_qc_out_ch,
    ixheaace_bit_buf_handle pstr_bit_stream_handle, WORD32 aot,
    ixheaace_temporal_noise_shaping_params pstr_tns_info, ixheaace_aac_tables *pstr_aac_tables) {
  IA_ERRORCODE err_code;

  switch (aot) {
    case AOT_AAC_LC:
    case AOT_SBR:
    case AOT_PS:
      ixheaace_write_bits(pstr_bit_stream_handle, ID_SCE, 3);
      ixheaace_write_bits(pstr_bit_stream_handle, instance_tag, 4);
      break;
    case AOT_AAC_LD:
      ixheaace_write_bits(pstr_bit_stream_handle, instance_tag, 4);
      break;
  }

  err_code = ia_enhaacplus_enc_write_ic_stream(
      0, pstr_qc_out_ch->win_shape, pstr_qc_out_ch->grouping_mask, pstr_sfb_offset,
      pstr_qc_out_ch->scalefactor, pstr_qc_out_ch->max_val_in_sfb, pstr_qc_out_ch->global_gain,
      pstr_qc_out_ch->quant_spec, &(pstr_qc_out_ch->section_data), pstr_bit_stream_handle, aot,
      pstr_tns_info, pstr_aac_tables);

  if (err_code != IA_NO_ERROR) {
    return err_code;
  }

  return IA_NO_ERROR;
}

static IA_ERRORCODE ia_enhaacplus_enc_write_channel_pair_element(
    WORD32 instance_tag, WORD32 ms_digest, WORD32 ms_flags[MAXIMUM_GROUPED_SCALE_FACTOR_BAND],
    WORD32 *pstr_sfb_offset[2], ixheaace_qc_out_channel pstr_qc_out_ch[2],
    ixheaace_bit_buf_handle pstr_bit_stream_handle, WORD32 aot,
    ixheaace_temporal_noise_shaping_params pstr_tns_info[2],
    ixheaace_aac_tables *pstr_aac_tables) {
  IA_ERRORCODE err_code;

  if (aot == AOT_AAC_LC || aot == AOT_SBR || aot == AOT_PS) {
    ixheaace_write_bits(pstr_bit_stream_handle, ID_CPE, 3);
  }

  switch (aot) {
    case AOT_AAC_LD:
      ixheaace_write_bits(pstr_bit_stream_handle, instance_tag, 4);
      ixheaace_write_bits(pstr_bit_stream_handle, 1, 1); /* common window */
      break;

    case AOT_AAC_LC:
    case AOT_SBR:
    case AOT_PS:
      ixheaace_write_bits(pstr_bit_stream_handle, instance_tag, 4);
      ixheaace_write_bits(pstr_bit_stream_handle, 1, 1);
      break;
  }

  ia_enhaacplus_enc_encode_ics_info(pstr_qc_out_ch[0].section_data.block_type,
                                    pstr_qc_out_ch[0].win_shape, pstr_qc_out_ch[0].grouping_mask,
                                    &(pstr_qc_out_ch[0].section_data), pstr_bit_stream_handle,
                                    aot);

  ia_enhaacplus_enc_encode_ms_info(pstr_qc_out_ch[0].section_data.sfb_cnt,
                                   pstr_qc_out_ch[0].section_data.sfb_per_group,
                                   pstr_qc_out_ch[0].section_data.max_sfb_per_grp, ms_digest,
                                   ms_flags, pstr_bit_stream_handle);
  err_code = ia_enhaacplus_enc_write_ic_stream(
      1, pstr_qc_out_ch[0].win_shape, pstr_qc_out_ch[0].grouping_mask, pstr_sfb_offset[0],
      pstr_qc_out_ch[0].scalefactor, pstr_qc_out_ch[0].max_val_in_sfb,
      pstr_qc_out_ch[0].global_gain, pstr_qc_out_ch[0].quant_spec,
      &(pstr_qc_out_ch[0].section_data), pstr_bit_stream_handle, aot, pstr_tns_info[0],
      pstr_aac_tables);

  if (err_code != IA_NO_ERROR) {
    return err_code;
  }

  err_code = ia_enhaacplus_enc_write_ic_stream(
      1, pstr_qc_out_ch[1].win_shape, pstr_qc_out_ch[1].grouping_mask, pstr_sfb_offset[1],
      pstr_qc_out_ch[1].scalefactor, pstr_qc_out_ch[1].max_val_in_sfb,
      pstr_qc_out_ch[1].global_gain, pstr_qc_out_ch[1].quant_spec,
      &(pstr_qc_out_ch[1].section_data), pstr_bit_stream_handle, aot, pstr_tns_info[1],
      pstr_aac_tables);

  if (err_code != IA_NO_ERROR) {
    return err_code;
  }

  return IA_NO_ERROR;
}

static VOID ia_enhaacplus_enc_write_fill_element_LD(
    const UWORD8 *ptr_anc_bytes, WORD32 total_fill_bits,
    ixheaace_bit_buf_handle pstr_bit_stream_handle) {
  WORD32 i, cnt, cnt1, remaining = 0;

  /*
    Write fill Element(s):
    amount of a fill element can be 7+X*8 Bits, X element of [0..270]
  */
  // ID_FIL and the FILL element payload size, ( 3+ 4 bits) are not sent.
  // but they are accounted in the earlier code, so subtracting it here

  total_fill_bits -= 7;
  while (total_fill_bits > 0) {
    cnt = MIN((total_fill_bits >> 3), ((1 << 4) - 1));

    if (ptr_anc_bytes) {
      for (i = 0; i < cnt; i++) {
        ixheaace_write_bits(pstr_bit_stream_handle, *ptr_anc_bytes++, 8);
        total_fill_bits -= 8;
      }
      if (total_fill_bits < 8) {
        ixheaace_write_bits(pstr_bit_stream_handle, *ptr_anc_bytes++, (UWORD8)total_fill_bits);
        ixheaace_write_bits(pstr_bit_stream_handle, 0, (UWORD8)(8 - total_fill_bits));
        total_fill_bits = 0;
      }
    } else {
      cnt1 = cnt >> 2;
      remaining = cnt - (cnt1 << 2);

      for (i = 0; i < cnt1; i++) {
        ixheaace_write_bits(pstr_bit_stream_handle, 0, 32);
        total_fill_bits -= 32;
      }
      if (remaining)
        for (i = 0; i < remaining; i++) {
          ixheaace_write_bits(pstr_bit_stream_handle, 0, 8);
          total_fill_bits -= 8;
        }
    }
  }
}

static VOID ia_enhaacplus_enc_write_fill_element_LC(const UWORD8 *ptr_anc_bytes,
                                                    WORD32 total_fill_bits,
                                                    ixheaace_bit_buf_handle pstr_h_bit_stream) {
  WORD32 i, cnt, esc_count;

  while (total_fill_bits >= (3 + 4)) {
    cnt = MIN((total_fill_bits - (3 + 4)) / 8, ((1 << 4) - 1));

    ixheaace_write_bits(pstr_h_bit_stream, ID_FIL, 3);

    ixheaace_write_bits(pstr_h_bit_stream, cnt, 4);

    total_fill_bits -= (3 + 4);

    if (cnt == (1 << 4) - 1) {
      esc_count = MIN((total_fill_bits / 8) - ((1 << 4) - 1), (1 << 8) - 1);

      ixheaace_write_bits(pstr_h_bit_stream, esc_count, 8);

      total_fill_bits -= 8;

      cnt += esc_count - 1;
    }

    for (i = 0; i < cnt; i++) {
      if (ptr_anc_bytes)
        ixheaace_write_bits(pstr_h_bit_stream, *ptr_anc_bytes++, 8);
      else
        ixheaace_write_bits(pstr_h_bit_stream, 0, 8);

      total_fill_bits -= 8;
    }
  }
}

static IA_ERRORCODE ia_enhaacplus_enc_write_single_channel_element_LFE(
    WORD32 instance_tag, WORD32 *ptr_sfb_offset, ixheaace_qc_out_channel *pstr_qc_out_ch,
    ixheaace_bit_buf_handle pstr_bit_stream_handle, WORD32 aot,
    ixheaace_temporal_noise_shaping_params pstr_tns_info, ixheaace_aac_tables *pstr_aac_tables) {
  IA_ERRORCODE err_code;
  switch (aot) {
    case AOT_AAC_LC:
    case AOT_SBR:
    case AOT_PS:
      ixheaace_write_bits(pstr_bit_stream_handle, ID_LFE, 3);
      ixheaace_write_bits(pstr_bit_stream_handle, instance_tag, 4);
      break;

    case AOT_AAC_LD:
      ixheaace_write_bits(pstr_bit_stream_handle, instance_tag, 4);
      break;
  }

  err_code = ia_enhaacplus_enc_write_ic_stream(
      0, pstr_qc_out_ch->win_shape, pstr_qc_out_ch->grouping_mask, ptr_sfb_offset,
      pstr_qc_out_ch->scalefactor, pstr_qc_out_ch->max_val_in_sfb, pstr_qc_out_ch->global_gain,
      pstr_qc_out_ch->quant_spec, &(pstr_qc_out_ch->section_data), pstr_bit_stream_handle, aot,
      pstr_tns_info, pstr_aac_tables);

  if (err_code != IA_NO_ERROR) {
    return err_code;
  }
  return IA_NO_ERROR;
}

static IA_ERRORCODE ia_enhaacplus_enc_write_single_channel_ind_coupling_element(
    WORD32 *ptr_sfb_offset, ixheaace_qc_out_channel *pstr_qc_out_ch,
    ixheaace_bit_buf_handle pstr_bit_stream_handle, WORD32 aot,
    ixheaace_temporal_noise_shaping_params pstr_tns_info, ixheaace_aac_tables *pstr_aac_tables) {
  IA_ERRORCODE err_code;
  ixheaace_write_bits(pstr_bit_stream_handle, ID_CCE, 3);

  /*Flag indication that this is an independent coupling channel*/
  ixheaace_write_bits(pstr_bit_stream_handle, 1, 1);

  /*number of coupled channel elements*/
  ixheaace_write_bits(pstr_bit_stream_handle, NUM_COUPLED_ELE, 3);

  /*Flag indicating target is CPE*/
  ixheaace_write_bits(pstr_bit_stream_handle, 1, 1);

  /*Instance tag of target CPE*/
  ixheaace_write_bits(pstr_bit_stream_handle, 0, 4);

  ixheaace_write_bits(pstr_bit_stream_handle, 3, 2);

  /*Flag indicating coupling after TNS*/
  ixheaace_write_bits(pstr_bit_stream_handle, 1, 1);

  /*Flag indicating sign of coupling*/
  ixheaace_write_bits(pstr_bit_stream_handle, 0, 1);

  /*Flag indicating Scale of coupling*/
  ixheaace_write_bits(pstr_bit_stream_handle, SCALE_COUPLING_LEVEL0, 2);

  err_code = ia_enhaacplus_enc_write_ic_stream(
      0, pstr_qc_out_ch->win_shape, pstr_qc_out_ch->grouping_mask, ptr_sfb_offset,
      pstr_qc_out_ch->scalefactor, pstr_qc_out_ch->max_val_in_sfb, pstr_qc_out_ch->global_gain,
      pstr_qc_out_ch->quant_spec, &(pstr_qc_out_ch->section_data), pstr_bit_stream_handle, aot,
      pstr_tns_info, pstr_aac_tables);
  if (err_code != IA_NO_ERROR) {
    return err_code;
  }

  ia_enhaacplus_enc_code_scale_factor_delta(-1, pstr_bit_stream_handle,
                                            pstr_aac_tables->pstr_huff_tab);

  return IA_NO_ERROR;
}

IA_ERRORCODE ia_enhaacplus_enc_write_bitstream(
    ixheaace_bit_buf_handle pstr_bit_stream_handle, ixheaace_element_info pstr_element_info,
    ixheaace_qc_out *pstr_qc_out, ixheaace_psy_out *pstr_psy_out, WORD32 *glob_used_bits,
    const UWORD8 *ptr_anc_bytes, ixheaace_aac_tables *pstr_aac_tables, FLAG flag_last_element,
    WORD32 *write_program_config_element, WORD32 i_num_coup_channels, WORD32 i_channels_mask,
    WORD32 i_samp_freq, WORD32 ele_idx, WORD32 aot, WORD32 *total_fill_bits) {
  IA_ERRORCODE err_code;
  WORD32 bit_markup, element_used_bits, frame_bits;

  bit_markup = ia_enhaacplus_enc_get_bits_available(pstr_bit_stream_handle);

  *glob_used_bits = 0;

  {
    WORD32 *ptr_sfb_offset[2];
    ixheaace_temporal_noise_shaping_params tns_info[2];

    element_used_bits = 0;

    if ((*write_program_config_element == 1) && (ele_idx == 0)) {
      WORD32 samp_rate = i_samp_freq, ch_mask = i_channels_mask, n_cc_ch = i_num_coup_channels;

      /*Write Program Config Element*/
      ixheaace_write_bits(pstr_bit_stream_handle, ID_PCE, 3);
      ia_enhaacplus_enc_write_pce(samp_rate, ch_mask, n_cc_ch, pstr_bit_stream_handle);
      *write_program_config_element = 0;
    }

    switch (pstr_element_info.el_type) {
      case ID_SCE: /* single channel */

        ptr_sfb_offset[0] =
            pstr_psy_out->psy_out_ch[pstr_element_info.channel_index[0]]->sfb_offsets;
        tns_info[0] = pstr_psy_out->psy_out_ch[pstr_element_info.channel_index[0]]->tns_info;

        err_code = ia_enhaacplus_enc_write_single_chan_elem(
            pstr_element_info.instance_tag, ptr_sfb_offset[0],
            pstr_qc_out->qc_channel[pstr_element_info.channel_index[0]], pstr_bit_stream_handle,
            aot, tns_info[0], pstr_aac_tables);

        if (err_code != IA_NO_ERROR) {
          return err_code;
        }
        break;

      case ID_CCE: /* single channel independent coupling element*/

        ptr_sfb_offset[0] =
            pstr_psy_out->psy_out_ch[pstr_element_info.channel_index[0]]->sfb_offsets;
        tns_info[0] = pstr_psy_out->psy_out_ch[pstr_element_info.channel_index[0]]->tns_info;

        err_code = ia_enhaacplus_enc_write_single_channel_ind_coupling_element(
            ptr_sfb_offset[0], pstr_qc_out->qc_channel[pstr_element_info.channel_index[0]],
            pstr_bit_stream_handle, aot, tns_info[0], pstr_aac_tables);

        if (err_code != IA_NO_ERROR) {
          return err_code;
        }
        break;

      case ID_LFE: /* single channel */

        ptr_sfb_offset[0] =
            pstr_psy_out->psy_out_ch[pstr_element_info.channel_index[0]]->sfb_offsets;
        tns_info[0] = pstr_psy_out->psy_out_ch[pstr_element_info.channel_index[0]]->tns_info;

        err_code = ia_enhaacplus_enc_write_single_channel_element_LFE(
            pstr_element_info.instance_tag, ptr_sfb_offset[0],
            pstr_qc_out->qc_channel[pstr_element_info.channel_index[0]], pstr_bit_stream_handle,
            aot, tns_info[0], pstr_aac_tables);

        if (err_code != IA_NO_ERROR) {
          return err_code;
        }
        break;

      case ID_CPE: /* channel pair */
      {
        WORD32 ms_digest = pstr_psy_out->psy_out_element.tools_info.ms_digest;
        WORD32 *ms_flags = pstr_psy_out->psy_out_element.tools_info.ms_mask;

        ptr_sfb_offset[0] =
            pstr_psy_out->psy_out_ch[pstr_element_info.channel_index[0]]->sfb_offsets;
        ptr_sfb_offset[1] =
            pstr_psy_out->psy_out_ch[pstr_element_info.channel_index[1]]->sfb_offsets;

        tns_info[0] = pstr_psy_out->psy_out_ch[pstr_element_info.channel_index[0]]->tns_info;
        tns_info[1] = pstr_psy_out->psy_out_ch[pstr_element_info.channel_index[1]]->tns_info;

        err_code = ia_enhaacplus_enc_write_channel_pair_element(
            pstr_element_info.instance_tag, ms_digest, ms_flags, ptr_sfb_offset,
            pstr_qc_out->qc_channel[pstr_element_info.channel_index[0]], pstr_bit_stream_handle,
            aot, tns_info, pstr_aac_tables);
        if (err_code != IA_NO_ERROR) {
          return err_code;
        }
      } break;

      default:
        return IA_EXHEAACE_INIT_FATAL_INVALID_ELEMENT_TYPE;

    } /* switch */

    element_used_bits -= bit_markup;

    bit_markup = ia_enhaacplus_enc_get_bits_available(pstr_bit_stream_handle);

    frame_bits = element_used_bits + bit_markup;
  }

  if (aot == AOT_AAC_LC || aot == AOT_SBR || aot == AOT_PS) {
    ia_enhaacplus_enc_write_fill_element_LC(ptr_anc_bytes, pstr_qc_out->tot_anc_bits_used,
                                            pstr_bit_stream_handle);
  }

  if (flag_last_element) {
    if (aot == AOT_AAC_LC || aot == AOT_SBR || aot == AOT_PS) {
      ia_enhaacplus_enc_write_fill_element_LC(NULL, *total_fill_bits, pstr_bit_stream_handle);

      *total_fill_bits = 0;

      ixheaace_write_bits(pstr_bit_stream_handle, ID_END, 3);
    }

    if (aot == AOT_AAC_LD || aot == AOT_AAC_ELD) {
      {
        WORD32 i, cnt = 0;
        ia_enhaacplus_enc_write_fill_element_LD(ptr_anc_bytes, pstr_qc_out->tot_anc_bits_used,
                                                pstr_bit_stream_handle);

        *total_fill_bits += pstr_qc_out->total_fill_bits;
        *total_fill_bits += pstr_qc_out->align_bits;
        cnt = *total_fill_bits >> 3;
        for (i = 0; i < cnt; i++) {
          ixheaace_write_bits(pstr_bit_stream_handle, 0, 8);
        }
        cnt = *total_fill_bits - (cnt << 3);
        ixheaace_write_bits(pstr_bit_stream_handle, 0, (UWORD8)cnt);
        *total_fill_bits = 0;
      }
    }
    /* byte alignement */

    ixheaace_write_bits(pstr_bit_stream_handle, 0,
                        (UWORD8)((8 - (pstr_bit_stream_handle->cnt_bits % 8)) % 8));
  }

  *glob_used_bits -= bit_markup;

  bit_markup = ia_enhaacplus_enc_get_bits_available(pstr_bit_stream_handle);

  *glob_used_bits += bit_markup;

  frame_bits += *glob_used_bits;

  if (frame_bits != pstr_qc_out->tot_static_bits_used + pstr_qc_out->tot_dyn_bits_used +
                        pstr_qc_out->tot_anc_bits_used + +pstr_qc_out->total_fill_bits +
                        pstr_qc_out->align_bits) {
  }

  return IA_NO_ERROR;
}
