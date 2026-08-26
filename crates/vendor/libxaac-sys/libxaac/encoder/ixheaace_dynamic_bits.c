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
#include <stdlib.h>
#include <string.h>
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

#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops16.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaac_basic_ops.h"

#include "ixheaace_psy_const.h"
#include "ixheaace_tns.h"
#include "ixheaace_tns_params.h"
#include "ixheaace_rom.h"
#include "ixheaace_common_rom.h"
#include "ixheaace_bitbuffer.h"
#include "ixheaace_bits_count.h"

#include "ixheaace_dynamic_bits.h"
#include "ixheaace_common_utils.h"

static WORD32 ia_enhaacplus_enc_calc_side_info_bits(WORD32 sfb_cnt, WORD32 block_type) {
  WORD32 seg_len_bits = (block_type == SHORT_WINDOW ? 3 : 5);
  WORD32 escape_val = (block_type == SHORT_WINDOW ? 7 : 31);
  WORD32 side_info_bits, tmp;

  side_info_bits = CODE_BCK_BITS;
  tmp = sfb_cnt;

  while (tmp >= 0) {
    side_info_bits += seg_len_bits;
    tmp -= escape_val;
  }

  return (side_info_bits);
}

static VOID ia_enhaacplus_enc_build_bit_look_up(
    const WORD16 *ptr_quant_spec, const WORD32 max_sfb, const WORD32 *ptr_sfb_offset,
    const UWORD16 *sfb_max,
    WORD32 bit_look_up[MAXIMUM_SCALE_FACTOR_BAND_LONG][CODE_BCK_ESC_NDX + 1],
    ixheaace_section_info *pstr_section_info, ixheaace_huffman_tables *pstr_huffman_tbl,
    WORD32 aot) {
  WORD8 i;

  for (i = 0; i < max_sfb; i++) {
    WORD32 sfb_width, max_val;

    pstr_section_info[i].sfb_cnt = 1;
    pstr_section_info[i].sfb_start = i;

    switch (aot) {
      case AOT_AAC_LC:
      case AOT_SBR:
      case AOT_PS:
        pstr_section_info[i].section_bits = INVALID_BITCOUNT_LC;
        break;

      case AOT_AAC_LD:
      case AOT_AAC_ELD:
        pstr_section_info[i].section_bits = INVALID_BITCOUNT_LD;
        break;
    }

    pstr_section_info[i].code_book = -1;

    sfb_width = ptr_sfb_offset[i + 1] - ptr_sfb_offset[i];

    max_val = sfb_max[i];

    ia_enhaacplus_enc_bitcount(ptr_quant_spec + ptr_sfb_offset[i], sfb_width, max_val,
                               bit_look_up[i], pstr_huffman_tbl, aot);
  }
}

static WORD32 ia_enhaacplus_enc_find_best_book(const WORD32 *ptr_bit_cnt, WORD8 *ptr_codebook,
                                               WORD32 aot) {
  WORD32 min_bits = 0, temp1, temp2;
  WORD8 temp_book = CODE_BCK_ESC_NDX;
  WORD8 j;

  switch (aot) {
    case AOT_AAC_LC:
    case AOT_SBR:
    case AOT_PS:
      min_bits = INVALID_BITCOUNT_LC;
      break;

    case AOT_AAC_LD:
    case AOT_AAC_ELD:
      min_bits = INVALID_BITCOUNT_LD;
      break;
  }

  for (j = 0; j <= CODE_BCK_ESC_NDX; j += 2) {
    temp1 = *ptr_bit_cnt++;
    temp2 = *ptr_bit_cnt++;
    if (temp1 < min_bits) {
      min_bits = temp1;
      temp_book = j;
    }
    if (temp2 < min_bits) {
      min_bits = temp2;
      temp_book = j + 1;
    }
  }

  *ptr_codebook = temp_book;
  return min_bits;
}

static WORD32 ia_enhaacplus_enc_find_min_merge_bits(const WORD32 *ptr_bit_cnt1,
                                                    const WORD32 *ptr_bit_cnt2, WORD32 aot) {
  WORD32 min_bits = 0, j, temp1, temp2, temp3, temp4;

  switch (aot) {
    case AOT_AAC_LC:
    case AOT_SBR:
    case AOT_PS:
      min_bits = INVALID_BITCOUNT_LC;
      break;

    case AOT_AAC_LD:
    case AOT_AAC_ELD:
      min_bits = INVALID_BITCOUNT_LD;
      break;
  }

  for (j = CODE_BCK_ESC_NDX; j >= 0; j -= 2) {
    temp1 = *ptr_bit_cnt1++;
    temp2 = *ptr_bit_cnt2++;
    temp3 = *ptr_bit_cnt1++;

    temp1 = temp1 + temp2;

    min_bits = MIN(temp1, min_bits);
    temp4 = *ptr_bit_cnt2++;
    temp1 = temp3 + temp4;

    min_bits = MIN(temp1, min_bits);
  }

  return min_bits;
}

static VOID ia_enhaacplus_enc_merge_bit_look_up(WORD32 *ptr_bit_cnt1, const WORD32 *ptr_bit_cnt2,
                                                WORD32 aot) {
  WORD32 j, temp1, temp2, temp3, temp4;
  WORD32 invalid_bitcnt = 0;

  switch (aot) {
    case AOT_AAC_LC:
    case AOT_SBR:
    case AOT_PS:
      invalid_bitcnt = INVALID_BITCOUNT_LC;
      break;

    case AOT_AAC_LD:
    case AOT_AAC_ELD:
      invalid_bitcnt = INVALID_BITCOUNT_LD;
      break;
  }
  for (j = 0; j <= CODE_BCK_ESC_NDX; j += 2) {
    temp1 = ptr_bit_cnt1[j];
    temp2 = ptr_bit_cnt2[j];
    temp3 = ptr_bit_cnt1[j + 1];

    ptr_bit_cnt1[j] = MIN(temp1 + temp2, invalid_bitcnt);
    temp4 = ptr_bit_cnt2[j + 1];
    ptr_bit_cnt1[j + 1] = MIN(temp3 + temp4, invalid_bitcnt);
  }
}

static WORD32 ia_enhaacplus_enc_find_max_merge(
    const WORD32 merge_gain_look_up[MAXIMUM_SCALE_FACTOR_BAND_LONG],
    const ixheaace_section_info *section, const WORD32 max_sfb, WORD32 *max_ndx) {
  WORD32 i, max_merge_gain = 0;

  for (i = 0; i + section[i].sfb_cnt < max_sfb; i += section[i].sfb_cnt) {
    if (merge_gain_look_up[i] > max_merge_gain) {
      max_merge_gain = merge_gain_look_up[i];
      *max_ndx = i;
    }
  }

  return (max_merge_gain);
}

static WORD32 ia_enhaacplus_enc_calc_merge_gain(
    const ixheaace_section_info *pstr_section_info,
    WORD32 bit_look_up[MAXIMUM_SCALE_FACTOR_BAND_LONG][CODE_BCK_ESC_NDX + 1],
    const WORD32 *pstr_side_info_tab, const WORD32 idx1, const WORD32 idx2, WORD32 aot) {
  WORD32 split_bits;
  WORD32 merge_bits;
  WORD32 merge_gain;

  split_bits = pstr_section_info[idx1].section_bits + pstr_section_info[idx2].section_bits;

  merge_bits =
      pstr_side_info_tab[pstr_section_info[idx1].sfb_cnt + pstr_section_info[idx2].sfb_cnt] +
      ia_enhaacplus_enc_find_min_merge_bits(bit_look_up[idx1], bit_look_up[idx2], aot);

  merge_gain = split_bits - merge_bits;

  return merge_gain;
}

static VOID ia_enhaacplus_enc_gm_stage0(
    ixheaace_section_info *pstr_section,
    WORD32 bit_look_up[MAXIMUM_SCALE_FACTOR_BAND_LONG][CODE_BCK_ESC_NDX + 1],
    const WORD32 max_sfb, WORD32 aot) {
  WORD32 i = 0;
  WORD32 invalid_bitcnt = 0;

  switch (aot) {
    case AOT_AAC_LC:
    case AOT_SBR:
    case AOT_PS:
      invalid_bitcnt = INVALID_BITCOUNT_LC;
      break;

    case AOT_AAC_LD:
    case AOT_AAC_ELD:

      invalid_bitcnt = INVALID_BITCOUNT_LD;
      break;
  }
  while (i < max_sfb) {
    if (pstr_section[i].section_bits == invalid_bitcnt) {
      pstr_section[i].section_bits = (WORD16)ia_enhaacplus_enc_find_best_book(
          bit_look_up[i], &(pstr_section[i].code_book), aot);
    }
    i++;
  }
}

static VOID ia_enhaacplus_enc_gm_stage1(
    ixheaace_section_info *pstr_section_info,
    WORD32 bit_look_up[MAXIMUM_SCALE_FACTOR_BAND_LONG][CODE_BCK_ESC_NDX + 1],
    const WORD32 max_sfb, const WORD32 *ptr_side_info_tab, WORD32 aot) {
  WORD32 merge_start = 0, merge_end;

  do {
    for (merge_end = merge_start + 1; merge_end < max_sfb; merge_end++) {
      if (pstr_section_info[merge_start].code_book != pstr_section_info[merge_end].code_book) {
        break;
      }

      pstr_section_info[merge_start].sfb_cnt++;

      pstr_section_info[merge_start].section_bits += pstr_section_info[merge_end].section_bits;

      ia_enhaacplus_enc_merge_bit_look_up(bit_look_up[merge_start], bit_look_up[merge_end], aot);
    }

    pstr_section_info[merge_start].section_bits +=
        (WORD16)ptr_side_info_tab[pstr_section_info[merge_start].sfb_cnt];

    pstr_section_info[merge_end - 1].sfb_start = pstr_section_info[merge_start].sfb_start;

    merge_start = merge_end;

  } while (merge_start < max_sfb);
}

static VOID ia_enhaacplus_enc_gm_stage2(
    ixheaace_section_info *pstr_section_info,
    WORD32 merge_gain_look_up[MAXIMUM_SCALE_FACTOR_BAND_LONG],
    WORD32 bit_look_up[MAXIMUM_SCALE_FACTOR_BAND_LONG][CODE_BCK_ESC_NDX + 1],
    const WORD32 max_sfb, const WORD32 *ptr_side_info_tab, WORD32 aot) {
  WORD32 i;

  for (i = 0; i + pstr_section_info[i].sfb_cnt < max_sfb; i += pstr_section_info[i].sfb_cnt) {
    merge_gain_look_up[i] =
        ia_enhaacplus_enc_calc_merge_gain(pstr_section_info, bit_look_up, ptr_side_info_tab, i,
                                          i + pstr_section_info[i].sfb_cnt, aot);
  }

  while (TRUE) {
    WORD32 max_merge_gain = 0, max_idx = 0, max_idx_next = 0, max_idx_last = 0;

    max_merge_gain = ia_enhaacplus_enc_find_max_merge(merge_gain_look_up, pstr_section_info,
                                                      max_sfb, &max_idx);

    if (max_merge_gain <= 0) {
      break;
    }

    max_idx_next = max_idx + pstr_section_info[max_idx].sfb_cnt;

    pstr_section_info[max_idx].sfb_cnt += pstr_section_info[max_idx_next].sfb_cnt;

    pstr_section_info[max_idx].section_bits +=
        pstr_section_info[max_idx_next].section_bits - (WORD16)max_merge_gain;

    ia_enhaacplus_enc_merge_bit_look_up(bit_look_up[max_idx], bit_look_up[max_idx_next], aot);

    if (max_idx != 0) {
      max_idx_last = pstr_section_info[max_idx - 1].sfb_start;

      merge_gain_look_up[max_idx_last] = ia_enhaacplus_enc_calc_merge_gain(
          pstr_section_info, bit_look_up, ptr_side_info_tab, max_idx_last, max_idx, aot);
    }

    max_idx_next = max_idx + pstr_section_info[max_idx].sfb_cnt;

    pstr_section_info[max_idx_next - 1].sfb_start = pstr_section_info[max_idx].sfb_start;

    if (max_idx_next < max_sfb) {
      merge_gain_look_up[max_idx] = ia_enhaacplus_enc_calc_merge_gain(
          pstr_section_info, bit_look_up, ptr_side_info_tab, max_idx, max_idx_next, aot);
    }
  }
}

static IA_ERRORCODE ia_enhaacplus_enc_noiseless_counter(
    ixheaace_section_data *pstr_section_data,
    WORD32 merge_gain_look_up[MAXIMUM_SCALE_FACTOR_BAND_LONG],
    WORD32 bit_look_up[MAXIMUM_SCALE_FACTOR_BAND_LONG][CODE_BCK_ESC_NDX + 1],
    const WORD16 *ptr_quant_spec, const UWORD16 *ptr_max_val_in_sfb, const WORD32 *ptr_sfb_offset,
    const WORD32 block_type, WORD32 *ptr_side_info_tab_long, WORD32 *ptr_side_info_tab_short,
    ixheaace_huffman_tables *pstr_huffman_tbl, WORD32 aot) {
  WORD32 grp_idx;
  WORD32 *ptr_side_info_tab = 0;
  ixheaace_section_info *pstr_section_info;
  WORD32 i;

  for (i = 0; i < MAXIMUM_SCALE_FACTOR_BAND_LONG; i++) {
    memset(bit_look_up[i], 0, (CODE_BCK_ESC_NDX + 1) * sizeof(bit_look_up[0][0]));
  }

  /* counting previous operations */
  switch (block_type) {
    case LONG_WINDOW:
    case START_WINDOW:
    case STOP_WINDOW:

      ptr_side_info_tab = ptr_side_info_tab_long;
      break;
    case SHORT_WINDOW:

      ptr_side_info_tab = ptr_side_info_tab_short;
      break;
    default:
      return IA_EXHEAACE_EXE_FATAL_INVALID_BLOCK_TYPE;
  }

  pstr_section_data->num_of_sections = 0;
  pstr_section_data->huffman_bits = 0;
  pstr_section_data->side_info_bits = 0;

  if (pstr_section_data->max_sfb_per_grp == 0) {
    return IA_NO_ERROR;
  }

  for (grp_idx = 0; grp_idx < pstr_section_data->sfb_cnt;
       grp_idx += pstr_section_data->sfb_per_group) {
    pstr_section_info = pstr_section_data->section + pstr_section_data->num_of_sections;

    ia_enhaacplus_enc_build_bit_look_up(ptr_quant_spec, pstr_section_data->max_sfb_per_grp,
                                        ptr_sfb_offset + grp_idx, ptr_max_val_in_sfb + grp_idx,
                                        bit_look_up, pstr_section_info, pstr_huffman_tbl, aot);

    ia_enhaacplus_enc_gm_stage0(pstr_section_info, bit_look_up,
                                pstr_section_data->max_sfb_per_grp, aot);

    ia_enhaacplus_enc_gm_stage1(pstr_section_info, bit_look_up,
                                pstr_section_data->max_sfb_per_grp, ptr_side_info_tab, aot);

    ia_enhaacplus_enc_gm_stage2(pstr_section_info, merge_gain_look_up, bit_look_up,
                                pstr_section_data->max_sfb_per_grp, ptr_side_info_tab, aot);

    for (i = 0; i < pstr_section_data->max_sfb_per_grp; i += pstr_section_info[i].sfb_cnt) {
      ia_enhaacplus_enc_find_best_book(bit_look_up[i], &(pstr_section_info[i].code_book), aot);

      pstr_section_info[i].sfb_start += (WORD8)grp_idx;

      pstr_section_data->huffman_bits +=
          pstr_section_info[i].section_bits - ptr_side_info_tab[pstr_section_info[i].sfb_cnt];

      pstr_section_data->side_info_bits += ptr_side_info_tab[pstr_section_info[i].sfb_cnt];

      pstr_section_data->section[pstr_section_data->num_of_sections++] = pstr_section_info[i];
    }
  }
  return IA_NO_ERROR;
}

static WORD32 ia_enhaacplus_enc_bit_count_scalefactor_delta(
    WORD32 delta, ixheaace_huffman_tables *pstr_huffman_tbl) {
  return pstr_huffman_tbl->huff_ltabscf[delta + CODE_BCK_SCF_LAV];
}

static IA_ERRORCODE ia_enhaacplus_enc_scf_count(const WORD16 *ptr_scale_fac,
                                                const UWORD16 *ptr_max_val_in_sfb,
                                                ixheaace_section_data *pstr_section_data,
                                                ixheaace_huffman_tables *pstr_huffman_tbl) {
  WORD32 sect_idx1 = 0;
  WORD32 sfb_idx1 = 0;
  WORD32 scf_idx = 0;
  WORD32 sect_idx2 = 0;
  WORD32 sfb_idx2 = 0;

  WORD32 last_val_scf = 0;
  WORD32 delta_scf = 0;
  WORD32 found = 0;
  WORD32 scf_skip_counter = 0;

  pstr_section_data->scale_fac_bits = 0;

  if (ptr_scale_fac == 0) {
    return IA_EXHEAACE_EXE_FATAL_INVALID_SCALE_FACTOR_GAIN;
  }
  last_val_scf = 0;
  pstr_section_data->first_scf = 0;

  for (sect_idx1 = 0; sect_idx1 < pstr_section_data->num_of_sections; sect_idx1++) {
    if (pstr_section_data->section[sect_idx1].code_book != CODE_BCK_ZERO_NO) {
      pstr_section_data->first_scf = pstr_section_data->section[sect_idx1].sfb_start;

      last_val_scf = ptr_scale_fac[pstr_section_data->first_scf];
      break;
    }
  }

  for (sect_idx1 = 0; sect_idx1 < pstr_section_data->num_of_sections; sect_idx1++) {
    if ((pstr_section_data->section[sect_idx1].code_book != CODE_BCK_ZERO_NO) &&
        (pstr_section_data->section[sect_idx1].code_book != CODE_BCK_PNS_NO)) {
      for (sfb_idx1 = pstr_section_data->section[sect_idx1].sfb_start;
           sfb_idx1 < pstr_section_data->section[sect_idx1].sfb_start +
                          pstr_section_data->section[sect_idx1].sfb_cnt;
           sfb_idx1++) {
        if (ptr_max_val_in_sfb[sfb_idx1] == 0) {
          found = 0;

          if (scf_skip_counter == 0) {
            if (sfb_idx1 == (pstr_section_data->section[sect_idx1].sfb_start +
                             pstr_section_data->section[sect_idx1].sfb_cnt - 1)) {
              found = 0;
            } else {
              for (scf_idx = (sfb_idx1 + 1);
                   scf_idx < pstr_section_data->section[sect_idx1].sfb_start +
                                 pstr_section_data->section[sect_idx1].sfb_cnt;
                   scf_idx++) {
                if (ptr_max_val_in_sfb[scf_idx] != 0) {
                  found = 1;

                  if ((abs32(ptr_scale_fac[scf_idx] - last_val_scf)) < CODE_BCK_SCF_LAV) {
                    delta_scf = 0;
                  } else {
                    delta_scf = -(ptr_scale_fac[sfb_idx1] - last_val_scf);

                    last_val_scf = ptr_scale_fac[sfb_idx1];
                    scf_skip_counter = 0;
                  }
                  break;
                }
                /* count scalefactor skip */
                scf_skip_counter = scf_skip_counter + 1;
              }
            }

            /* search for the next ptr_max_val_in_sfb[] != 0 in all other sections */
            for (sect_idx2 = (sect_idx1 + 1);
                 (sect_idx2 < pstr_section_data->num_of_sections) && (found == 0); sect_idx2++) {
              if ((pstr_section_data->section[sect_idx2].code_book != CODE_BCK_ZERO_NO) &&
                  (pstr_section_data->section[sect_idx2].code_book != CODE_BCK_PNS_NO)) {
                for (sfb_idx2 = pstr_section_data->section[sect_idx2].sfb_start;
                     sfb_idx2 < pstr_section_data->section[sect_idx2].sfb_start +
                                    pstr_section_data->section[sect_idx2].sfb_cnt;
                     sfb_idx2++) {
                  if (ptr_max_val_in_sfb[sfb_idx2] != 0) {
                    found = 1;

                    if ((abs32(ptr_scale_fac[sfb_idx2] - last_val_scf)) < CODE_BCK_SCF_LAV) {
                      delta_scf = 0;
                    } else {
                      delta_scf = -(ptr_scale_fac[sfb_idx1] - last_val_scf);

                      last_val_scf = ptr_scale_fac[sfb_idx1];
                      scf_skip_counter = 0;
                    }
                    break;
                  }

                  scf_skip_counter = scf_skip_counter + 1;
                }
              }
            }

            if (found == 0) {
              delta_scf = 0;
              scf_skip_counter = 0;
            }
          } else {
            delta_scf = 0;

            scf_skip_counter = scf_skip_counter - 1;
          }
        } else {
          delta_scf = -(ptr_scale_fac[sfb_idx1] - last_val_scf);

          last_val_scf = ptr_scale_fac[sfb_idx1];
        }

        pstr_section_data->scale_fac_bits +=
            ia_enhaacplus_enc_bit_count_scalefactor_delta(delta_scf, pstr_huffman_tbl);
      }
    }
  }
  return IA_NO_ERROR;
}

IA_ERRORCODE ia_enhaacplus_enc_dyn_bitcount(
    const WORD16 *ptr_quant_spec, const UWORD16 *ptr_max_val_in_sfb, const WORD16 *ptr_scale_fac,
    const WORD32 block_type, const WORD32 sfb_cnt, const WORD32 max_sfb_per_grp,
    const WORD32 sfb_per_grp, const WORD32 *ptr_sfb_offset,
    ixheaace_section_data *pstr_section_data, WORD32 *ptr_side_info_tab_long,
    WORD32 *ptr_side_info_tab_short, ixheaace_huffman_tables *ptr_huffman_tbl,
    WORD32 *ptr_scratch_buf, WORD32 aot, WORD32 *bit_cnt) {
  IA_ERRORCODE err_code;
  WORD32(*ptr_bit_look_up)
  [CODE_BCK_ESC_NDX + 1] = (WORD32(*)[CODE_BCK_ESC_NDX + 1]) ptr_scratch_buf;
  WORD32 *ptr_merge_gain_look_up =
      ptr_scratch_buf + MAXIMUM_SCALE_FACTOR_BAND_LONG * (CODE_BCK_ESC_NDX + 1);
  *bit_cnt = 0;
  pstr_section_data->block_type = block_type;
  pstr_section_data->sfb_cnt = sfb_cnt;
  pstr_section_data->sfb_per_group = sfb_per_grp;

  pstr_section_data->total_groups_cnt = sfb_cnt / sfb_per_grp;

  pstr_section_data->max_sfb_per_grp = max_sfb_per_grp;

  err_code = ia_enhaacplus_enc_noiseless_counter(
      pstr_section_data, ptr_merge_gain_look_up, ptr_bit_look_up, ptr_quant_spec,
      ptr_max_val_in_sfb, ptr_sfb_offset, block_type, ptr_side_info_tab_long,
      ptr_side_info_tab_short, ptr_huffman_tbl, aot);

  if (err_code != IA_NO_ERROR) {
    return err_code;
  }

  err_code = ia_enhaacplus_enc_scf_count(ptr_scale_fac, ptr_max_val_in_sfb, pstr_section_data,
                                         ptr_huffman_tbl);

  if (err_code != IA_NO_ERROR) {
    return err_code;
  }

  *bit_cnt = (pstr_section_data->huffman_bits + pstr_section_data->side_info_bits +
              pstr_section_data->scale_fac_bits);
  return IA_NO_ERROR;
}

VOID ia_enhaacplus_enc_bitcount_init(WORD32 *side_info_tab_long, WORD32 *side_info_tab_short) {
  WORD32 i;

  /* side_info_tab_long[] */
  for (i = 0; i <= MAXIMUM_SCALE_FACTOR_BAND_LONG; i++) {
    side_info_tab_long[i] = ia_enhaacplus_enc_calc_side_info_bits(i, LONG_WINDOW);
  }

  /* side_info_tab_short[] */
  for (i = 0; i <= MAXIMUM_SCALE_FACTOR_BAND_SHORT; i++) {
    side_info_tab_short[i] = ia_enhaacplus_enc_calc_side_info_bits(i, SHORT_WINDOW);
  }
}
