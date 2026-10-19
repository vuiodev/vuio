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

#include <string.h>
#include <stdlib.h>
#include <math.h>
#include "ixheaac_type_def.h"
#include "ixheaac_error_standards.h"
#include "ixheaace_error_codes.h"
#include "ixheaace_mps_common_fix.h"
#include "ixheaace_mps_defines.h"
#include "ixheaace_mps_common_define.h"
#include "ixheaace_bitbuffer.h"
#include "ixheaace_mps_nlc_enc.h"
#include "ixheaace_mps_huff_tab.h"
#include "ixheaace_mps_param_extract.h"
#include "ixheaace_mps_sac_nlc_enc.h"
#include "ixheaace_mps_bitstream.h"
#include "ixheaace_mps_struct_def.h"
#include "ixheaace_mps_sac_polyphase.h"
#include "ixheaace_mps_sac_nlc_enc.h"
#include "ixheaace_mps_sac_hybfilter.h"
#include "ixheaace_mps_spatial_bitstream.h"
#include "ixheaace_mps_tree.h"
#include "ixheaace_mps_rom.h"
#include "ixheaace_common_utils.h"
#include "ixheaac_constants.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaac_basic_ops.h"

static UWORD8 ixheaace_mps_212_sym_check(WORD16 data[2], const WORD16 lav,
                                         WORD16 *const p_sym_bits) {
  UWORD8 sym_bits = 0;
  UWORD8 num_sbits = 0;
  WORD16 sum_val = data[0] + data[1];
  WORD16 diff_val = data[0] - data[1];

  if (sum_val != 0) {
    WORD8 sum_neg = (sum_val < 0) ? 1 : 0;
    if (sum_neg) {
      sum_val = -sum_val;
      diff_val = -diff_val;
    }
    sym_bits = (sym_bits << 1) | sum_neg;
    num_sbits++;
  }

  if (diff_val != 0) {
    WORD8 diff_neg = (diff_val < 0) ? 1 : 0;
    if (diff_neg) {
      diff_val = -diff_val;
    }
    sym_bits = (sym_bits << 1) | diff_neg;
    num_sbits++;
  }
  *p_sym_bits = sym_bits;

  if (!(sum_val % 2)) {
    data[0] = (sum_val / 2);
    data[1] = (diff_val / 2);
  } else {
    data[0] = (lav - sum_val / 2);
    data[1] = (lav - diff_val / 2);
  }

  return num_sbits;
}

static WORD16 ixheaace_mps_212_calc_pcm_bits(const WORD16 num_val, const WORD16 n_levels) {
  WORD16 num_complete_chunks = 0, rest_chunk_size = 0;
  WORD16 max_grp_len = 0, bits_pcm = 0;
  WORD32 chunk_levels, lvl;

  switch (n_levels) {
    case 7:
      max_grp_len = 6;
      break;
    case 3:
    case 6:
      max_grp_len = 5;
      break;
    case 13:
    case 19:
    case 51:
      max_grp_len = 4;
      break;
    case 25:
      max_grp_len = 3;
      break;
    case 11:
      max_grp_len = 2;
      break;
    default:
      max_grp_len = 1;
  }

  num_complete_chunks = num_val / max_grp_len;
  rest_chunk_size = num_val % max_grp_len;

  chunk_levels = 1;
  for (lvl = 1; lvl <= max_grp_len; lvl++) {
    chunk_levels *= n_levels;
  }
  bits_pcm = (WORD16)(round(log(chunk_levels) / log(2)) * num_complete_chunks);
  bits_pcm += (WORD16)(round(log(chunk_levels) / log(2)) * rest_chunk_size);

  return bits_pcm;
}

static VOID ixheaace_mps_212_apply_pcm_coding(ixheaace_bit_buf_handle pstr_bit_buf,
                                              const WORD16 *const in_data_1,
                                              const WORD16 *const in_data_2, const WORD16 offset,
                                              const WORD16 num_val, const WORD16 n_levels) {
  WORD16 val = 0, lvl = 0, idx = 0;
  WORD16 max_grp_len = 0, grp_len = 0, next_val = 0;
  WORD32 grp_val = 0, chunk_levels = 0;
  UWORD8 pcm_block_size[7] = {0};

  switch (n_levels) {
    case 7:
      max_grp_len = 6;
      break;
    case 3:
    case 6:
    case 9:
      max_grp_len = 5;
      break;
    case 13:
    case 19:
    case 51:
      max_grp_len = 4;
      break;
    case 5:
    case 25:
      max_grp_len = 3;
      break;
    case 11:
      max_grp_len = 2;
      break;
    default:
      max_grp_len = 1;
  }

  chunk_levels = 1;
  for (lvl = 1; lvl <= max_grp_len; lvl++) {
    chunk_levels *= n_levels;
    pcm_block_size[lvl] = (UWORD8)round(log(chunk_levels) / log(2));
  }

  for (val = 0; val < num_val; val += max_grp_len) {
    grp_len = MIN(max_grp_len, num_val - val);
    grp_val = 0;
    for (lvl = 0; lvl < grp_len; lvl++) {
      idx = val + lvl;
      next_val = (in_data_2 == NULL)
                     ? in_data_1[idx]
                     : (in_data_1 == NULL)
                           ? in_data_2[idx]
                           : ((idx & 01) ? in_data_2[idx >> 1] : in_data_1[idx >> 1]);
      grp_val = grp_val * n_levels + next_val + offset;
    }
    ixheaace_write_bits(pstr_bit_buf, grp_val, pcm_block_size[grp_len]);
  }
}

static IA_ERRORCODE ixheaace_mps_212_huff_enc_1_d(ixheaace_bit_buf_handle pstr_bit_buf,
                                                  const WORD32 data_type, const WORD32 dim_1,
                                                  WORD16 *const in_data, const WORD16 num_bands,
                                                  const WORD16 part_0_flag,
                                                  UWORD16 *huff_code_bits) {
  WORD32 band, offset = 0;
  UWORD16 huff_bits = 0;
  ixheaace_mps_huff_entry part0;
  const ixheaace_mps_huff_entry *p_huff_tab = NULL;

  *huff_code_bits = 0;
  if (data_type == IXHEAACE_MPS_SAC_DATA_TYPE_ICC) {
    p_huff_tab = ixheaace_mps_212_huff_icc_tab.h1_d[dim_1];
  } else if (data_type == IXHEAACE_MPS_SAC_DATA_TYPE_CLD) {
    p_huff_tab = ixheaace_mps_212_huff_cld_tab.h1_d[dim_1];
  } else {
    return IA_EXHEAACE_EXE_FATAL_MPS_INVALID_HUFF_DATA_TYPE;
  }

  if (part_0_flag) {
    if (data_type == IXHEAACE_MPS_SAC_DATA_TYPE_ICC) {
      part0 = ixheaace_mps_212_huff_part_0_tab.icc[in_data[0]];
    } else if (data_type == IXHEAACE_MPS_SAC_DATA_TYPE_CLD) {
      part0 = ixheaace_mps_212_huff_part_0_tab.cld[in_data[0]];
    } else {
      return IA_EXHEAACE_EXE_FATAL_MPS_INVALID_HUFF_DATA_TYPE;
    }
    huff_bits += ixheaace_write_bits(pstr_bit_buf, part0.value, part0.length);
    offset = 1;
  }

  for (band = offset; band < num_bands; band++) {
    WORD32 id_sign = 0;
    WORD32 id = in_data[band];

    if (id != 0) {
      id_sign = 0;
      if (id < 0) {
        id = -id;
        id_sign = 1;
      }
    }

    huff_bits += ixheaace_write_bits(pstr_bit_buf, p_huff_tab[id].value, p_huff_tab[id].length);

    if (id != 0) {
      huff_bits += ixheaace_write_bits(pstr_bit_buf, id_sign, 1);
    }
  }

  *huff_code_bits = huff_bits;
  return IA_NO_ERROR;
}

static VOID ixheaace_mps_212_get_huff_entry(const WORD32 lav, const WORD32 data_type,
                                            const WORD32 band, WORD16 tab_idx_2_d[2],
                                            WORD16 in_data[][2],
                                            ixheaace_mps_huff_entry *const pstr_huff_entry,
                                            ixheaace_mps_huff_entry *const pstr_huff_escape) {
  const ixheaace_mps_huff_cld_tab_2d *pstr_huff_cld_tab_2d =
      &ixheaace_mps_212_huff_cld_tab.h2_d[tab_idx_2_d[0]][tab_idx_2_d[1]];
  const ixheaace_mps_huff_icc_tab_2d *pstr_huff_icc_tab_2d =
      &ixheaace_mps_212_huff_icc_tab.h2_d[tab_idx_2_d[0]][tab_idx_2_d[1]];

  switch (lav) {
    case 9: {
      *pstr_huff_entry = pstr_huff_cld_tab_2d->lav9.entry[in_data[band][0]][in_data[band][1]];
      *pstr_huff_escape = pstr_huff_cld_tab_2d->lav9.escape;
    } break;
    case 7: {
      switch (data_type) {
        case IXHEAACE_MPS_SAC_DATA_TYPE_CLD:
          *pstr_huff_entry = pstr_huff_cld_tab_2d->lav7.entry[in_data[band][0]][in_data[band][1]];
          *pstr_huff_escape = pstr_huff_cld_tab_2d->lav7.escape;
          break;
        case IXHEAACE_MPS_SAC_DATA_TYPE_ICC:
          *pstr_huff_entry = pstr_huff_icc_tab_2d->lav7.entry[in_data[band][0]][in_data[band][1]];
          *pstr_huff_escape = pstr_huff_icc_tab_2d->lav7.escape;
          break;
      }
    } break;
    case 5: {
      switch (data_type) {
        case IXHEAACE_MPS_SAC_DATA_TYPE_CLD:
          *pstr_huff_entry = pstr_huff_cld_tab_2d->lav5.entry[in_data[band][0]][in_data[band][1]];
          *pstr_huff_escape = pstr_huff_cld_tab_2d->lav5.escape;
          break;
        case IXHEAACE_MPS_SAC_DATA_TYPE_ICC:
          *pstr_huff_entry = pstr_huff_icc_tab_2d->lav5.entry[in_data[band][0]][in_data[band][1]];
          *pstr_huff_escape = pstr_huff_icc_tab_2d->lav5.escape;
          break;
      }
    } break;
    case 3: {
      switch (data_type) {
        case IXHEAACE_MPS_SAC_DATA_TYPE_CLD:
          *pstr_huff_entry = pstr_huff_cld_tab_2d->lav3.entry[in_data[band][0]][in_data[band][1]];
          *pstr_huff_escape = pstr_huff_cld_tab_2d->lav3.escape;
          break;
        case IXHEAACE_MPS_SAC_DATA_TYPE_ICC:
          *pstr_huff_entry = pstr_huff_icc_tab_2d->lav3.entry[in_data[band][0]][in_data[band][1]];
          *pstr_huff_escape = pstr_huff_icc_tab_2d->lav3.escape;
          break;
      }
    } break;
    case 1: {
      *pstr_huff_entry = pstr_huff_icc_tab_2d->lav1.entry[in_data[band][0]][in_data[band][1]];
      *pstr_huff_escape = pstr_huff_icc_tab_2d->lav1.escape;
    } break;
  }
}

static IA_ERRORCODE ixheaace_mps_212_huff_enc_2_d(ixheaace_bit_buf_handle pstr_bit_buf,
                                                  const WORD32 data_type, WORD16 tab_idx_2_d[2],
                                                  WORD16 lav_idx, WORD16 in_data[][2],
                                                  WORD16 num_bands, WORD16 stride,
                                                  WORD16 *p_0_data[2], UWORD16 *huff_bits) {
  WORD16 band = 0, lav = 0, sym_bits = 0, esc_idx = 0;
  UWORD8 num_sbits = 0;
  WORD16 esc_data[2][IXHEAACE_MPS_SAC_MAX_FREQ_BANDS] = {{0}};

  *huff_bits = 0;
  const ixheaace_mps_huff_entry *pstr_huff_entry = NULL;
  if (data_type == IXHEAACE_MPS_SAC_DATA_TYPE_ICC) {
    lav = 2 * lav_idx + 1;
    pstr_huff_entry = ixheaace_mps_212_huff_part_0_tab.icc;
  } else if (data_type == IXHEAACE_MPS_SAC_DATA_TYPE_CLD) {
    lav = 2 * lav_idx + 3;
    pstr_huff_entry = ixheaace_mps_212_huff_part_0_tab.cld;
  } else {
    return IA_EXHEAACE_EXE_FATAL_MPS_INVALID_HUFF_DATA_TYPE;
  }

  if (p_0_data[0] != NULL) {
    ixheaace_mps_huff_entry entry = pstr_huff_entry[*p_0_data[0]];
    *huff_bits += ixheaace_write_bits(pstr_bit_buf, entry.value, entry.length);
  }
  if (p_0_data[1] != NULL) {
    ixheaace_mps_huff_entry entry = pstr_huff_entry[*p_0_data[1]];
    *huff_bits += ixheaace_write_bits(pstr_bit_buf, entry.value, entry.length);
  }

  for (band = 0; band < num_bands; band += stride) {
    ixheaace_mps_huff_entry entry = {0};
    ixheaace_mps_huff_entry escape = {0};

    esc_data[0][esc_idx] = in_data[band][0] + lav;
    esc_data[1][esc_idx] = in_data[band][1] + lav;

    num_sbits = ixheaace_mps_212_sym_check(in_data[band], lav, &sym_bits);

    ixheaace_mps_212_get_huff_entry(lav, data_type, band, tab_idx_2_d, in_data, &entry, &escape);

    *huff_bits += ixheaace_write_bits(pstr_bit_buf, entry.value, entry.length);

    if ((entry.value == escape.value) && (entry.length == escape.length)) {
      esc_idx++;
    } else {
      *huff_bits += ixheaace_write_bits(pstr_bit_buf, sym_bits, num_sbits);
    }
  }
  if (esc_idx > 0) {
    *huff_bits += ixheaace_mps_212_calc_pcm_bits(2 * esc_idx, (2 * lav + 1));
    ixheaace_mps_212_apply_pcm_coding(pstr_bit_buf, esc_data[0], esc_data[1], 0, 2 * esc_idx,
                                      (2 * lav + 1));
  }

  return IA_NO_ERROR;
}

static WORD8 ixheaace_mps_212_get_next_lav_step(const WORD32 lav, const WORD32 data_type) {
  WORD8 lav_step = 0;
  if (data_type == IXHEAACE_MPS_SAC_DATA_TYPE_ICC) {
    lav_step = (lav > 7) ? -1 : lav_step_icc[lav];
  } else if (data_type == IXHEAACE_MPS_SAC_DATA_TYPE_CLD) {
    lav_step = (lav > 9) ? -1 : lav_step_cld[lav];
  } else {
    lav_step = 0;
  }

  return lav_step;
}

static IA_ERRORCODE ixheaace_mps_212_calc_huff_bits(
    WORD16 *in_data_1, WORD16 *in_data_2, const WORD32 data_type, const WORD32 diff_type_1,
    const WORD32 diff_type_2, const WORD16 num_bands, WORD16 *const lav_idx, WORD16 *bit_count,
    WORD16 *const huff_dim, WORD16 *const huff_pair_type) {
  IA_ERRORCODE error;
  WORD16 band = 0;
  WORD16 bit_count_1_d = 0;
  WORD16 bit_count_2_d_freq = 0;
  WORD16 bit_count_min = 0;
  WORD16 num_band_short_data_1 = 0;
  WORD16 num_band_short_data_2 = 0;
  UWORD16 huff_bits;
  WORD16 tab_idx_2_d[2][2] = {{0}};
  WORD16 tab_idx_1_d[2] = {0};
  WORD16 df_rest_flag[2] = {0};
  WORD16 part_0_flag[2] = {0};
  WORD16 lav_fp[2] = {0};
  WORD16 pair_vec[IXHEAACE_MPS_SAC_MAX_FREQ_BANDS][2] = {{0}};
  WORD16 *part_0_data_1[2] = {NULL};
  WORD16 *part_0_data_2[2] = {NULL};
  WORD16 *in_short_data_1 = NULL;
  WORD16 *in_short_data_2 = NULL;

  bit_count_1_d = 1;
  bit_count_2_d_freq = 1;
  num_band_short_data_1 = num_bands;
  num_band_short_data_2 = num_bands;

  tab_idx_1_d[0] = (diff_type_1 == IXHEAACE_MPS_SAC_DIFF_FREQ) ? 0 : 1;
  tab_idx_1_d[1] = (diff_type_2 == IXHEAACE_MPS_SAC_DIFF_FREQ) ? 0 : 1;

  part_0_flag[0] = (diff_type_1 == IXHEAACE_MPS_SAC_DIFF_FREQ);
  part_0_flag[1] = (diff_type_2 == IXHEAACE_MPS_SAC_DIFF_FREQ);

  if (in_data_1 != NULL) {
    in_short_data_1 = in_data_1 + (diff_type_1 == IXHEAACE_MPS_SAC_DIFF_TIME ? 2 : 0);
    error = ixheaace_mps_212_huff_enc_1_d(NULL, data_type, tab_idx_1_d[0], in_short_data_1,
                                          num_band_short_data_1, part_0_flag[0], &huff_bits);
    if (error) {
      return error;
    }
    bit_count_1_d += huff_bits;
  }
  if (in_data_2 != NULL) {
    in_short_data_2 = in_data_2 + (diff_type_2 == IXHEAACE_MPS_SAC_DIFF_TIME ? 2 : 0);
    error = ixheaace_mps_212_huff_enc_1_d(NULL, data_type, tab_idx_1_d[1], in_short_data_2,
                                          num_band_short_data_2, part_0_flag[1], &huff_bits);
    if (error) {
      return error;
    }
    bit_count_1_d += huff_bits;
  }

  bit_count_min = bit_count_1_d;
  *huff_dim = IXHEAACE_MPS_SAC_HUFF_1D;
  lav_idx[0] = lav_idx[1] = -1;

  if (in_data_1 != NULL) {
    if (diff_type_1 == IXHEAACE_MPS_SAC_DIFF_FREQ) {
      part_0_data_1[0] = &in_data_1[0];
      part_0_data_1[1] = NULL;

      num_band_short_data_1 -= 1;
      in_short_data_1 += 1;
    }

    df_rest_flag[0] = num_band_short_data_1 % 2;

    if (df_rest_flag[0]) {
      num_band_short_data_1 -= 1;
    }

    for (band = 0; band < num_band_short_data_1 - 1; band += 2) {
      pair_vec[band][0] = in_short_data_1[band];
      pair_vec[band][1] = in_short_data_1[band + 1];

      lav_fp[0] = (WORD16)MAX(lav_fp[0], abs(pair_vec[band][0]));
      lav_fp[0] = (WORD16)MAX(lav_fp[0], abs(pair_vec[band][1]));
    }

    tab_idx_2_d[0][0] = (diff_type_1 == IXHEAACE_MPS_SAC_DIFF_TIME) ? 1 : 0;
    tab_idx_2_d[0][1] = 0;

    tab_idx_1_d[0] = (diff_type_1 == IXHEAACE_MPS_SAC_DIFF_FREQ) ? 0 : 1;

    lav_fp[0] = ixheaace_mps_212_get_next_lav_step(lav_fp[0], data_type);

    if (lav_fp[0] != -1) {
      bit_count_2_d_freq += lav_huff_len[lav_fp[0]];
    }
  }

  if (in_data_2 != NULL) {
    if (diff_type_2 == IXHEAACE_MPS_SAC_DIFF_FREQ) {
      part_0_data_2[0] = NULL;
      part_0_data_2[1] = &in_data_2[0];

      num_band_short_data_2 -= 1;
      in_short_data_2 += 1;
    }

    df_rest_flag[1] = num_band_short_data_2 % 2;

    if (df_rest_flag[1]) {
      num_band_short_data_2 -= 1;
    }

    for (band = 0; band < num_band_short_data_2 - 1; band += 2) {
      pair_vec[band + 1][0] = in_short_data_2[band];
      pair_vec[band + 1][1] = in_short_data_2[band + 1];

      lav_fp[1] = (WORD16)MAX(lav_fp[1], abs(pair_vec[band + 1][0]));
      lav_fp[1] = (WORD16)MAX(lav_fp[1], abs(pair_vec[band + 1][1]));
    }

    tab_idx_2_d[1][0] = (diff_type_2 == IXHEAACE_MPS_SAC_DIFF_TIME) ? 1 : 0;
    tab_idx_2_d[1][1] = 0;

    tab_idx_1_d[1] = (diff_type_2 == IXHEAACE_MPS_SAC_DIFF_FREQ) ? 0 : 1;

    lav_fp[1] = ixheaace_mps_212_get_next_lav_step(lav_fp[1], data_type);

    if (lav_fp[1] != -1) bit_count_2_d_freq += lav_huff_len[lav_fp[1]];
  }

  if ((lav_fp[0] != -1) && (lav_fp[1] != -1)) {
    if (in_data_1 != NULL) {
      error = ixheaace_mps_212_huff_enc_2_d(NULL, data_type, tab_idx_2_d[0], lav_fp[0], pair_vec,
                                            num_band_short_data_1, 2, part_0_data_1, &huff_bits);
      if (error) {
        return error;
      }

      bit_count_2_d_freq += huff_bits;
    }
    if (in_data_2 != NULL) {
      error =
          ixheaace_mps_212_huff_enc_2_d(NULL, data_type, tab_idx_2_d[1], lav_fp[1], pair_vec + 1,
                                        num_band_short_data_2, 2, part_0_data_2, &huff_bits);
      if (error) {
        return error;
      }
      bit_count_2_d_freq += huff_bits;
    }
    if (in_data_1 != NULL) {
      if (df_rest_flag[0]) {
        error = ixheaace_mps_212_huff_enc_1_d(NULL, data_type, tab_idx_1_d[0],
                                              in_short_data_1 + num_band_short_data_1, 1, 0,
                                              &huff_bits);
        if (error) {
          return error;
        }
        bit_count_2_d_freq += huff_bits;
      }
    }
    if (in_data_2 != NULL) {
      if (df_rest_flag[1]) {
        error = ixheaace_mps_212_huff_enc_1_d(NULL, data_type, tab_idx_1_d[1],
                                              in_short_data_2 + num_band_short_data_2, 1, 0,
                                              &huff_bits);
        if (error) {
          return error;
        }
        bit_count_2_d_freq += huff_bits;
      }
    }

    if (bit_count_2_d_freq < bit_count_min) {
      bit_count_min = bit_count_2_d_freq;
      *huff_dim = IXHEAACE_MPS_SAC_HUFF_2D;
      *huff_pair_type = IXHEAACE_MPS_SAC_FREQ_PAIR;
      lav_idx[0] = lav_fp[0];
      lav_idx[1] = lav_fp[1];
    }
  }

  *bit_count = bit_count_min;
  return IA_NO_ERROR;
}

static IA_ERRORCODE ixheaace_mps_212_apply_huff_coding(
    ixheaace_bit_buf_handle pstr_bit_buf, WORD16 *const in_data_1, WORD16 *const in_data_2,
    const WORD32 data_type, const WORD32 diff_type_1, const WORD32 diff_type_2,
    const WORD16 bands, const WORD16 *const lav_idx, WORD16 huff_dim, WORD16 huff_pair_type) {
  IA_ERRORCODE error;
  WORD16 band = 0;
  WORD16 num_band_short_data_1 = bands;
  WORD16 num_band_short_data_2 = bands;
  UWORD16 huff_bits;

  WORD16 df_rest_flag[2] = {0};
  WORD16 part_0_flag[2] = {0};
  WORD16 tab_idx_1_d[2] = {0};
  WORD16 tab_idx_2_d[2][2] = {{0}};
  WORD16 pair_vec[IXHEAACE_MPS_SAC_MAX_FREQ_BANDS][2] = {{0}};

  WORD16 *part_0_data_1[2] = {NULL};
  WORD16 *part_0_data_2[2] = {NULL};
  WORD16 *in_short_data_1 = NULL;
  WORD16 *in_short_data_2 = NULL;

  ixheaace_write_bits(pstr_bit_buf, huff_dim, 1);
  if (huff_dim == IXHEAACE_MPS_SAC_HUFF_1D) {
    part_0_flag[0] = (diff_type_1 == IXHEAACE_MPS_SAC_DIFF_FREQ);
    part_0_flag[1] = (diff_type_2 == IXHEAACE_MPS_SAC_DIFF_FREQ);

    tab_idx_1_d[0] = (diff_type_1 == IXHEAACE_MPS_SAC_DIFF_FREQ) ? 0 : 1;
    tab_idx_1_d[1] = (diff_type_2 == IXHEAACE_MPS_SAC_DIFF_FREQ) ? 0 : 1;

    if (in_data_1 != NULL) {
      in_short_data_1 = in_data_1 + (diff_type_1 == IXHEAACE_MPS_SAC_DIFF_TIME ? 2 : 0);
      error =
          ixheaace_mps_212_huff_enc_1_d(pstr_bit_buf, data_type, tab_idx_1_d[0], in_short_data_1,
                                        num_band_short_data_1, part_0_flag[0], &huff_bits);
      if (error) {
        return error;
      }
    }
    if (in_data_2 != NULL) {
      in_short_data_2 = in_data_2 + (diff_type_2 == IXHEAACE_MPS_SAC_DIFF_TIME ? 2 : 0);
      error =
          ixheaace_mps_212_huff_enc_1_d(pstr_bit_buf, data_type, tab_idx_1_d[1], in_short_data_2,
                                        num_band_short_data_2, part_0_flag[1], &huff_bits);
      if (error) {
        return error;
      }
    }
  } else {
    if (huff_pair_type == IXHEAACE_MPS_SAC_FREQ_PAIR) {
      if (in_data_1 != NULL) {
        in_short_data_1 = in_data_1 + (diff_type_1 == IXHEAACE_MPS_SAC_DIFF_TIME ? 2 : 0);
        if (diff_type_1 == IXHEAACE_MPS_SAC_DIFF_FREQ) {
          part_0_data_1[0] = &in_data_1[0];
          part_0_data_1[1] = NULL;

          num_band_short_data_1 -= 1;
          in_short_data_1 += 1;
        }

        df_rest_flag[0] = num_band_short_data_1 % 2;

        if (df_rest_flag[0]) num_band_short_data_1 -= 1;

        for (band = 0; band < num_band_short_data_1 - 1; band += 2) {
          pair_vec[band][0] = in_short_data_1[band];
          pair_vec[band][1] = in_short_data_1[band + 1];
        }

        tab_idx_2_d[0][0] = (diff_type_1 == IXHEAACE_MPS_SAC_DIFF_TIME) ? 1 : 0;
        tab_idx_2_d[0][1] = 0;

        tab_idx_1_d[0] = (diff_type_1 == IXHEAACE_MPS_SAC_DIFF_FREQ) ? 0 : 1;
      }
      if (in_data_2 != NULL) {
        in_short_data_2 = in_data_2 + (diff_type_2 == IXHEAACE_MPS_SAC_DIFF_TIME ? 2 : 0);
        if (diff_type_2 == IXHEAACE_MPS_SAC_DIFF_FREQ) {
          part_0_data_2[0] = NULL;
          part_0_data_2[1] = &in_data_2[0];

          num_band_short_data_2 -= 1;
          in_short_data_2 += 1;
        }

        df_rest_flag[1] = num_band_short_data_2 % 2;

        if (df_rest_flag[1]) num_band_short_data_2 -= 1;

        for (band = 0; band < num_band_short_data_2 - 1; band += 2) {
          pair_vec[band + 1][0] = in_short_data_2[band];
          pair_vec[band + 1][1] = in_short_data_2[band + 1];
        }

        tab_idx_2_d[1][0] = (diff_type_2 == IXHEAACE_MPS_SAC_DIFF_TIME) ? 1 : 0;
        tab_idx_2_d[1][1] = 0;

        tab_idx_1_d[1] = (diff_type_2 == IXHEAACE_MPS_SAC_DIFF_FREQ) ? 0 : 1;
      }
      if (in_data_1 != NULL) {
        ixheaace_write_bits(pstr_bit_buf, lav_huff_val[lav_idx[0]], lav_huff_len[lav_idx[0]]);
        error = ixheaace_mps_212_huff_enc_2_d(pstr_bit_buf, data_type, tab_idx_2_d[0], lav_idx[0],
                                              pair_vec, num_band_short_data_1, 2, part_0_data_1,
                                              &huff_bits);
        if (error) {
          return error;
        }
        if (df_rest_flag[0]) {
          error = ixheaace_mps_212_huff_enc_1_d(pstr_bit_buf, data_type, tab_idx_1_d[0],
                                                in_short_data_1 + num_band_short_data_1, 1, 0,
                                                &huff_bits);
          if (error) {
            return error;
          }
        }
      }
      if (in_data_2 != NULL) {
        ixheaace_write_bits(pstr_bit_buf, lav_huff_val[lav_idx[1]], lav_huff_len[lav_idx[1]]);
        error = ixheaace_mps_212_huff_enc_2_d(pstr_bit_buf, data_type, tab_idx_2_d[1], lav_idx[1],
                                              pair_vec + 1, num_band_short_data_2, 2,
                                              part_0_data_2, &huff_bits);
        if (error) {
          return error;
        }
        if (df_rest_flag[1]) {
          error = ixheaace_mps_212_huff_enc_1_d(pstr_bit_buf, data_type, tab_idx_1_d[1],
                                                in_short_data_2 + num_band_short_data_2, 1, 0,
                                                &huff_bits);
          if (error) {
            return error;
          }
        }
      }
    } else {
      if (in_data_1 == NULL || in_data_2 == NULL || in_short_data_1 == NULL ||
          in_short_data_2 == NULL) {
        return IA_EXHEAACE_EXE_FATAL_MPS_NULL_DATA_HANDLE;
      }

      if ((diff_type_1 == IXHEAACE_MPS_SAC_DIFF_FREQ) ||
          (diff_type_2 == IXHEAACE_MPS_SAC_DIFF_FREQ)) {
        part_0_data_1[0] = &in_data_1[0];
        part_0_data_1[1] = &in_data_2[0];

        in_short_data_1 += 1;
        in_short_data_2 += 1;

        num_band_short_data_1 -= 1;
      }

      for (band = 0; band < num_band_short_data_1; band++) {
        pair_vec[band][0] = in_short_data_1[band];
        pair_vec[band][1] = in_short_data_2[band];
      }
      tab_idx_2_d[0][0] = ((diff_type_1 == IXHEAACE_MPS_SAC_DIFF_TIME) ||
                           (diff_type_2 == IXHEAACE_MPS_SAC_DIFF_TIME))
                              ? 1
                              : 0;
      tab_idx_2_d[0][1] = 1;
      ixheaace_write_bits(pstr_bit_buf, lav_huff_val[lav_idx[0]], lav_huff_len[lav_idx[0]]);
      error = ixheaace_mps_212_huff_enc_2_d(pstr_bit_buf, data_type, tab_idx_2_d[0], lav_idx[0],
                                            pair_vec, num_band_short_data_1, 1, part_0_data_1,
                                            &huff_bits);
      if (error) {
        return error;
      }
    }
  }
  return IA_NO_ERROR;
}

static WORD32 ixheaace_mps_515_split_lsb(WORD32 *in_data, WORD32 offset, WORD32 num_lsb,
                                         WORD32 num_val, WORD32 *out_data_lsb,
                                         WORD32 *out_data_msb) {
  WORD32 i = 0, val = 0, lsb = 0, msb = 0;

  UWORD32 mask = (1 << num_lsb) - 1;
  WORD32 no_lsb_bits = 0;

  for (i = 0; i < num_val; i++) {
    val = in_data[i] + offset;

    lsb = val & mask;
    msb = val >> num_lsb;

    if (out_data_lsb != NULL) out_data_lsb[i] = lsb;
    if (out_data_msb != NULL) out_data_msb[i] = msb;

    no_lsb_bits += num_lsb;
  }

  return no_lsb_bits;
}

static VOID ixheaace_mps_515_apply_lsb_coding(ixheaace_bit_buf_handle pstr_bit_buf,
                                              WORD32 *in_data_lsb, UWORD8 num_lsb,
                                              WORD32 num_val) {
  WORD32 i = 0;

  for (i = 0; i < num_val; i++) {
    ixheaace_write_bits(pstr_bit_buf, in_data_lsb[i], num_lsb);
  }
}

static VOID ixheaace_mps_515_calc_diff_freq(WORD32 *in_data, WORD32 *out_data, WORD32 num_val) {
  WORD32 i = 0;

  out_data[0] = in_data[0];

  for (i = 1; i < num_val; i++) {
    out_data[i] = in_data[i] - in_data[i - 1];
  }
}

static VOID ixheaace_mps_515_calc_diff_time(WORD32 *in_data, WORD32 *prev_data, WORD32 *out_data,
                                            WORD32 direction, WORD32 num_val) {
  WORD32 i = 0;

  out_data[-1] = (direction == IXHEAACE_MPS_SAC_DIRECTION_BACKWARDS) ? in_data[0] : prev_data[0];

  for (i = 0; i < num_val; i++) {
    out_data[i] = in_data[i] - prev_data[i];
  }
}

static WORD32 ixheaace_mps_515_calc_pcm_bits(WORD32 num_val, WORD32 n_levels) {
  WORD32 num_complete_chunks = 0, rest_chunk_size = 0;
  WORD32 max_grp_len = 0, bits_pcm = 0;

  FLOAT32 num_ld_levels = 0.f;

  switch (n_levels) {
    case 3:
      max_grp_len = 5;
      break;
    case 7:
      max_grp_len = 6;
      break;
    case 11:
      max_grp_len = 2;
      break;
    case 13:
      max_grp_len = 4;
      break;
    case 19:
      max_grp_len = 4;
      break;
    case 25:
      max_grp_len = 3;
      break;
    case 51:
      max_grp_len = 4;
      break;

    case 4:
    case 8:
    case 15:
    case 26:
    case 31:
      max_grp_len = 1;
      break;
    default:
      return IA_EXHEAACE_EXE_FATAL_MPS_INVALID_LEVELS;
  }

  num_ld_levels = (FLOAT32)(log((FLOAT32)n_levels) / log(2.f));

  num_complete_chunks = num_val / max_grp_len;
  rest_chunk_size = num_val % max_grp_len;

  bits_pcm = ((WORD32)ceil((FLOAT32)(max_grp_len)*num_ld_levels)) * num_complete_chunks;
  bits_pcm += (WORD32)ceil((FLOAT32)(rest_chunk_size)*num_ld_levels);

  return bits_pcm;
}

static VOID ixheaace_mps_515_apply_pcm_coding(ixheaace_bit_buf_handle pstr_bit_buf,
                                              WORD32 *in_data_1, WORD32 *in_data_2, WORD32 offset,
                                              WORD32 num_val, WORD32 n_levels) {
  WORD32 i = 0, j = 0, idx = 0;
  WORD32 max_grp_len = 1, grp_len = 0, next_val = 0, grp_val = 0;

  FLOAT32 num_ld_levels = 0.f;

  UWORD8 pcm_block_size[7] = {0};
  switch (n_levels) {
    case 4:
      num_ld_levels = 2.00000000f;
      break;
    case 8:
      num_ld_levels = 3.00000000f;
      break;
    case 15:
      num_ld_levels = 3.90689060f;
      break;
    case 26:
      num_ld_levels = 4.70043972f;
      break;
    case 31:
      num_ld_levels = 4.95419645f;
      break;
    case 51:
      max_grp_len = 4;
      num_ld_levels = 5.67242534f;
      break;
  }

  for (i = 1; i <= max_grp_len; i++) {
    pcm_block_size[i] = (UWORD8)ceil((FLOAT32)(i)*num_ld_levels);
  }

  for (i = 0; i < num_val; i += max_grp_len) {
    grp_len = MIN(max_grp_len, num_val - i);
    grp_val = 0;
    for (j = 0; j < grp_len; j++) {
      idx = i + j;
      next_val = (in_data_2 == NULL)
                     ? in_data_1[idx]
                     : (in_data_1 == NULL)
                           ? in_data_2[idx]
                           : ((idx & 01) ? in_data_2[idx >> 1] : in_data_1[idx >> 1]);
      grp_val = grp_val * n_levels + next_val + offset;
    }

    ixheaace_write_bits(pstr_bit_buf, grp_val, pcm_block_size[grp_len]);
  }
}

static WORD32 ixheaace_mps_515_count_huff_cld(
    const ixheaace_mps_sac_huff_cld_tab *pstr_huff_cld_tab_pt0,
    const ixheaace_mps_sac_huff_cld_tab *pstr_huff_cld_tab_diff, WORD32 *in_data, WORD32 num_val,
    WORD32 p0_flag) {
  WORD32 i = 0, id = 0;
  WORD32 huff_bits = 0;
  WORD32 offset = 0;
  if (p0_flag) {
    huff_bits += pstr_huff_cld_tab_pt0->length[in_data[0]];
    offset = 1;
  }

  for (i = offset; i < num_val; i++) {
    id = in_data[i];

    if (id != 0) {
      if (id < 0) {
        id = -id;
      }
      huff_bits += 1;
    }

    huff_bits += pstr_huff_cld_tab_diff->length[id];
  }

  return huff_bits;
}

static WORD32 ixheaace_mps_515_count_huff_icc(
    const ixheaace_mps_sac_huff_icc_tab *pstr_huff_icc_tab_pt0,
    const ixheaace_mps_sac_huff_icc_tab *pstr_huff_icc_tab_diff, WORD32 *in_data, WORD32 num_val,
    WORD32 p0_flag) {
  WORD32 i = 0, id = 0;
  WORD32 huff_bits = 0;
  WORD32 offset = 0;

  if (p0_flag) {
    huff_bits += pstr_huff_icc_tab_pt0->length[in_data[0]];
    offset = 1;
  }

  for (i = offset; i < num_val; i++) {
    id = in_data[i];

    if (id != 0) {
      if (id < 0) {
        id = -id;
      }
      huff_bits += 1;
    }

    huff_bits += pstr_huff_icc_tab_diff->length[id];
  }

  return huff_bits;
}

static WORD32 ixheaace_mps_515_count_huff_cpc(
    const ixheaace_mps_sac_huff_cpc_tab *pstr_huff_cpc_tab_pt0,
    const ixheaace_mps_sac_huff_cpc_tab *pstr_huff_cpc_tab_diff, WORD32 *in_data, WORD32 num_val,
    WORD32 p0_flag) {
  WORD32 i = 0, id = 0;
  WORD32 huff_bits = 0;
  WORD32 offset = 0;

  if (p0_flag) {
    huff_bits += pstr_huff_cpc_tab_pt0->length[in_data[0]];
    offset = 1;
  }

  for (i = offset; i < num_val; i++) {
    id = in_data[i];

    if (id != 0) {
      if (id < 0) {
        id = -id;
      }
      huff_bits += 1;
    }

    huff_bits += pstr_huff_cpc_tab_diff->length[id];
  }

  return huff_bits;
}

static VOID ixheaace_mps_515_huff_enc_cld(
    ixheaace_bit_buf_handle pstr_bit_buf,
    const ixheaace_mps_sac_huff_cld_tab *pstr_huff_cld_tab_pt0,
    const ixheaace_mps_sac_huff_cld_tab *pstr_huff_cld_tab_diff, WORD32 *in_data, WORD32 num_val,
    WORD32 p0_flag) {
  WORD32 i = 0, id = 0, id_sign = 0;
  WORD32 offset = 0;

  if (p0_flag) {
    ixheaace_write_bits(pstr_bit_buf, pstr_huff_cld_tab_pt0->value[in_data[0]],
                        pstr_huff_cld_tab_pt0->length[in_data[0]]);
    offset = 1;
  }

  for (i = offset; i < num_val; i++) {
    id = in_data[i];

    if (id != 0) {
      id_sign = 0;
      if (id < 0) {
        id = -id;
        id_sign = 1;
      }
    }

    ixheaace_write_bits(pstr_bit_buf, pstr_huff_cld_tab_diff->value[id],
                        pstr_huff_cld_tab_diff->length[id]);

    if (id != 0) {
      ixheaace_write_bits(pstr_bit_buf, id_sign, 1);
    }
  }
}

static VOID ixheaace_mps_515_huff_enc_icc(
    ixheaace_bit_buf_handle pstr_bit_buf,
    const ixheaace_mps_sac_huff_icc_tab *pstr_huff_icc_tab_pt0,
    const ixheaace_mps_sac_huff_icc_tab *pstr_huff_icc_tab_diff, WORD32 *in_data, WORD32 num_val,
    WORD32 p0_flag) {
  WORD32 i = 0, id = 0, id_sign = 0;
  WORD32 offset = 0;

  if (p0_flag) {
    ixheaace_write_bits(pstr_bit_buf, pstr_huff_icc_tab_pt0->value[in_data[0]],
                        pstr_huff_icc_tab_pt0->length[in_data[0]]);
    offset = 1;
  }

  for (i = offset; i < num_val; i++) {
    id = in_data[i];

    if (id != 0) {
      id_sign = 0;
      if (id < 0) {
        id = -id;
        id_sign = 1;
      }
    }

    ixheaace_write_bits(pstr_bit_buf, pstr_huff_icc_tab_diff->value[id],
                        pstr_huff_icc_tab_diff->length[id]);

    if (id != 0) {
      ixheaace_write_bits(pstr_bit_buf, id_sign, 1);
    }
  }
}

static VOID ixheaace_mps_515_huff_enc_cpc(
    ixheaace_bit_buf_handle pstr_bit_buf,
    const ixheaace_mps_sac_huff_cpc_tab *pstr_huff_cpc_tab_pt0,
    const ixheaace_mps_sac_huff_cpc_tab *pstr_huff_cpc_tab_diff, WORD32 *in_data, WORD32 num_val,
    WORD32 p0_flag) {
  WORD32 i = 0, id = 0, id_sign = 0;
  WORD32 offset = 0;

  if (p0_flag) {
    ixheaace_write_bits(pstr_bit_buf, pstr_huff_cpc_tab_pt0->value[in_data[0]],
                        pstr_huff_cpc_tab_pt0->length[in_data[0]]);
    offset = 1;
  }

  for (i = offset; i < num_val; i++) {
    id = in_data[i];

    if (id != 0) {
      id_sign = 0;
      if (id < 0) {
        id = -id;
        id_sign = 1;
      }
    }

    ixheaace_write_bits(pstr_bit_buf, pstr_huff_cpc_tab_diff->value[id],
                        pstr_huff_cpc_tab_diff->length[id]);

    if (id != 0) {
      ixheaace_write_bits(pstr_bit_buf, id_sign, 1);
    }
  }
}

static WORD32 ixheaace_mps_515_calc_huff_bits(WORD32 *in_data_1, WORD32 *in_data_2,
                                              WORD32 data_type, WORD32 diff_type_1,
                                              WORD32 diff_type_2, WORD32 num_val) {
  WORD32 p0_flag[2];

  WORD32 offset_1 = (diff_type_1 == IXHEAACE_MPS_SAC_DIFF_TIME) ? 1 : 0;
  WORD32 offset_2 = (diff_type_2 == IXHEAACE_MPS_SAC_DIFF_TIME) ? 1 : 0;

  WORD32 bit_count_huff = 0;

  WORD32 num_val_1_int = 0;
  WORD32 num_val_2_int = 0;

  WORD32 *in_data_1_int = in_data_1 + offset_1;
  WORD32 *in_data_2_int = in_data_2 + offset_2;

  bit_count_huff = 1;

  num_val_1_int = num_val;
  num_val_2_int = num_val;

  p0_flag[0] = (diff_type_1 == IXHEAACE_MPS_SAC_DIFF_FREQ);
  p0_flag[1] = (diff_type_2 == IXHEAACE_MPS_SAC_DIFF_FREQ);

  switch (data_type) {
    case IXHEAACE_MPS_SAC_DATA_TYPE_CLD:
      if (in_data_1 != NULL)
        bit_count_huff +=
            ixheaace_mps_515_count_huff_cld(&ixheaace_mps_515_huff_cld_tab.huff_pt0,
                                            &ixheaace_mps_515_huff_cld_tab.huff_diff[diff_type_1],
                                            in_data_1_int, num_val_1_int, p0_flag[0]);
      if (in_data_2 != NULL)
        bit_count_huff +=
            ixheaace_mps_515_count_huff_cld(&ixheaace_mps_515_huff_cld_tab.huff_pt0,
                                            &ixheaace_mps_515_huff_cld_tab.huff_diff[diff_type_2],
                                            in_data_2_int, num_val_2_int, p0_flag[1]);
      break;

    case IXHEAACE_MPS_SAC_DATA_TYPE_ICC:
      if (in_data_1 != NULL)
        bit_count_huff +=
            ixheaace_mps_515_count_huff_icc(&ixheaace_mps_515_huff_icc_tab.huff_pt0,
                                            &ixheaace_mps_515_huff_icc_tab.huff_diff[diff_type_1],
                                            in_data_1_int, num_val_1_int, p0_flag[0]);
      if (in_data_2 != NULL)
        bit_count_huff +=
            ixheaace_mps_515_count_huff_icc(&ixheaace_mps_515_huff_icc_tab.huff_pt0,
                                            &ixheaace_mps_515_huff_icc_tab.huff_diff[diff_type_2],
                                            in_data_2_int, num_val_2_int, p0_flag[1]);
      break;

    case IXHEAACE_MPS_SAC_DATA_TYPE_CPC:
      if (in_data_1 != NULL)
        bit_count_huff +=
            ixheaace_mps_515_count_huff_cpc(&ixheaace_mps_515_huff_cpc_tab.huff_pt0,
                                            &ixheaace_mps_515_huff_cpc_tab.huff_diff[diff_type_1],
                                            in_data_1_int, num_val_1_int, p0_flag[0]);
      if (in_data_2 != NULL)
        bit_count_huff +=
            ixheaace_mps_515_count_huff_cpc(&ixheaace_mps_515_huff_cpc_tab.huff_pt0,
                                            &ixheaace_mps_515_huff_cpc_tab.huff_diff[diff_type_2],
                                            in_data_2_int, num_val_2_int, p0_flag[1]);
      break;

    default:
      break;
  }

  return bit_count_huff;
}

static VOID ixheaace_mps_515_apply_huff_coding(ixheaace_bit_buf_handle pstr_bit_buf,
                                               WORD32 *in_data_1, WORD32 *in_data_2,
                                               WORD32 data_type, WORD32 diff_type_1,
                                               WORD32 diff_type_2, WORD32 num_val) {
  WORD32 p0_flag[2];

  WORD32 num_val_1_int = num_val;
  WORD32 num_val_2_int = num_val;

  WORD32 *in_data_1_int = in_data_1;
  WORD32 *in_data_2_int = in_data_2;

  if (diff_type_1 == IXHEAACE_MPS_SAC_DIFF_TIME) in_data_1_int += 1;
  if (diff_type_2 == IXHEAACE_MPS_SAC_DIFF_TIME) in_data_2_int += 1;

  ixheaace_write_bits(pstr_bit_buf, IXHEAACE_MPS_SAC_HUFF_1D, 1);

  p0_flag[0] = (diff_type_1 == IXHEAACE_MPS_SAC_DIFF_FREQ);
  p0_flag[1] = (diff_type_2 == IXHEAACE_MPS_SAC_DIFF_FREQ);

  switch (data_type) {
    case IXHEAACE_MPS_SAC_DATA_TYPE_CLD:
      if (in_data_1 != NULL)
        ixheaace_mps_515_huff_enc_cld(pstr_bit_buf, &ixheaace_mps_515_huff_cld_tab.huff_pt0,
                                      &ixheaace_mps_515_huff_cld_tab.huff_diff[diff_type_1],
                                      in_data_1_int, num_val_1_int, p0_flag[0]);
      if (in_data_2 != NULL)
        ixheaace_mps_515_huff_enc_cld(pstr_bit_buf, &ixheaace_mps_515_huff_cld_tab.huff_pt0,
                                      &ixheaace_mps_515_huff_cld_tab.huff_diff[diff_type_2],
                                      in_data_2_int, num_val_2_int, p0_flag[1]);
      break;

    case IXHEAACE_MPS_SAC_DATA_TYPE_ICC:
      if (in_data_1 != NULL)
        ixheaace_mps_515_huff_enc_icc(pstr_bit_buf, &ixheaace_mps_515_huff_icc_tab.huff_pt0,
                                      &ixheaace_mps_515_huff_icc_tab.huff_diff[diff_type_1],
                                      in_data_1_int, num_val_1_int, p0_flag[0]);
      if (in_data_2 != NULL)
        ixheaace_mps_515_huff_enc_icc(pstr_bit_buf, &ixheaace_mps_515_huff_icc_tab.huff_pt0,
                                      &ixheaace_mps_515_huff_icc_tab.huff_diff[diff_type_2],
                                      in_data_2_int, num_val_2_int, p0_flag[1]);
      break;

    case IXHEAACE_MPS_SAC_DATA_TYPE_CPC:
      if (in_data_1 != NULL)
        ixheaace_mps_515_huff_enc_cpc(pstr_bit_buf, &ixheaace_mps_515_huff_cpc_tab.huff_pt0,
                                      &ixheaace_mps_515_huff_cpc_tab.huff_diff[diff_type_1],
                                      in_data_1_int, num_val_1_int, p0_flag[0]);
      if (in_data_2 != NULL)
        ixheaace_mps_515_huff_enc_cpc(pstr_bit_buf, &ixheaace_mps_515_huff_cpc_tab.huff_pt0,
                                      &ixheaace_mps_515_huff_cpc_tab.huff_diff[diff_type_2],
                                      in_data_2_int, num_val_2_int, p0_flag[1]);
      break;

    default:
      break;
  }
}

IA_ERRORCODE ixheaace_mps_212_ec_data_pair_enc(
    ixheaace_bit_buf_handle pstr_bit_buf, WORD16 pp_in_data[][IXHEAACE_MPS_SAC_MAX_FREQ_BANDS],
    WORD16 p_in_data[IXHEAACE_MPS_SAC_MAX_FREQ_BANDS], const WORD32 data_type,
    const WORD32 set_idx, const WORD32 start_band, const WORD16 data_bands,
    const WORD32 coarse_flag, const WORD32 independency_flag) {
  IA_ERRORCODE error;
  WORD16 band = 0;
  WORD16 quant_levels = 0, quant_offset = 0, num_pcm_val = 0;
  WORD16 pcm_coding_flag = 0;
  WORD16 min_bits_all = 0;
  WORD16 min_found = 0;
  WORD16 huff_dim_df_df = 0;
  WORD16 huff_pair_type_df_df = 0;
  WORD16 huff_dim_df_dt = 0;
  WORD16 huff_pair_type_df_dt = 0;
  WORD16 huff_dim_dtbw_df = 0;
  WORD16 huff_pair_type_dtbw_df = 0;
  WORD16 huff_dim_dt_dt = 0;
  WORD16 huff_pair_type_dt_dt = 0;
  WORD16 num_pcm_bits = -1;
  WORD16 min_bits_df_df = -1;
  WORD16 min_bits_df_dt = -1;
  WORD16 min_bits_dtbw_df = -1;
  WORD16 min_bits_dt_dt = -1;
  WORD16 allow_diff_time_back_flag = !independency_flag || (set_idx > 0);

  WORD16 quant_data_msb[2][IXHEAACE_MPS_SAC_MAX_FREQ_BANDS] = {{0}};
  WORD16 quant_data_hist_msb[IXHEAACE_MPS_SAC_MAX_FREQ_BANDS] = {0};
  WORD16 data_diff_freq[2][IXHEAACE_MPS_SAC_MAX_FREQ_BANDS] = {{0}};
  WORD16 data_diff_time[2][IXHEAACE_MPS_SAC_MAX_FREQ_BANDS + 2];
  WORD16 lav_df_df[2] = {-1, -1};
  WORD16 lav_df_dt[2] = {-1, -1};
  WORD16 lav_dtbw_df[2] = {-1, -1};
  WORD16 lav_dt_dt[2] = {-1, -1};
  WORD16 *p_quant_data_msb[2] = {NULL, NULL};
  WORD16 *p_quant_data_hist_msb = NULL;

  if (data_bands <= 0) {
    return IA_EXHEAACE_EXE_NONFATAL_MPS_INVALID_DATA_BANDS;
  }

  if (data_type == IXHEAACE_MPS_SAC_DATA_TYPE_ICC) {
    if (coarse_flag) {
      quant_levels = 4;
      quant_offset = 0;
    } else {
      quant_levels = 8;
      quant_offset = 0;
    }
  } else if (data_type == IXHEAACE_MPS_SAC_DATA_TYPE_CLD) {
    if (coarse_flag) {
      quant_levels = 15;
      quant_offset = 7;
    } else {
      quant_levels = 31;
      quant_offset = 15;
    }
  } else {
    return IA_EXHEAACE_EXE_FATAL_MPS_INVALID_HUFF_DATA_TYPE;
  }
  if (quant_offset != 0) {
    for (band = 0; band < data_bands; band++) {
      quant_data_msb[0][band] = pp_in_data[set_idx][start_band + band] + quant_offset;
      quant_data_msb[1][band] = pp_in_data[set_idx + 1][start_band + band] + quant_offset;
    }
    p_quant_data_msb[0] = quant_data_msb[0];
    p_quant_data_msb[1] = quant_data_msb[1];
  } else {
    p_quant_data_msb[0] = pp_in_data[set_idx] + start_band;
    p_quant_data_msb[1] = pp_in_data[set_idx + 1] + start_band;
  }

  if (allow_diff_time_back_flag) {
    if (quant_offset != 0) {
      for (band = 0; band < data_bands; band++) {
        quant_data_hist_msb[band] = p_in_data[start_band + band] + quant_offset;
      }
      p_quant_data_hist_msb = quant_data_hist_msb;
    } else {
      p_quant_data_hist_msb = p_in_data + start_band;
    }
  }

  data_diff_freq[0][0] = p_quant_data_msb[0][0];
  data_diff_freq[1][0] = p_quant_data_msb[1][0];

  for (band = 1; band < data_bands; band++) {
    data_diff_freq[0][band] = p_quant_data_msb[0][band] - p_quant_data_msb[0][band - 1];
    data_diff_freq[1][band] = p_quant_data_msb[1][band] - p_quant_data_msb[1][band - 1];
  }

  if (allow_diff_time_back_flag) {
    data_diff_time[0][0] = p_quant_data_msb[0][0];
    data_diff_time[0][1] = p_quant_data_hist_msb[0];

    for (band = 0; band < data_bands; band++) {
      data_diff_time[0][band + 2] = p_quant_data_msb[0][band] - p_quant_data_hist_msb[band];
    }
  }

  data_diff_time[1][0] = p_quant_data_msb[1][0];
  data_diff_time[1][1] = p_quant_data_msb[0][0];

  for (band = 0; band < data_bands; band++) {
    data_diff_time[1][band + 2] = p_quant_data_msb[1][band] - p_quant_data_msb[0][band];
  }

  num_pcm_bits = ixheaace_mps_212_calc_pcm_bits((WORD16)(2 * data_bands), quant_levels);
  num_pcm_val = (WORD16)(2 * data_bands);

  min_bits_all = num_pcm_bits;

  error = ixheaace_mps_212_calc_huff_bits(data_diff_freq[0], data_diff_freq[1], data_type,
                                          IXHEAACE_MPS_SAC_DIFF_FREQ, IXHEAACE_MPS_SAC_DIFF_FREQ,
                                          data_bands, lav_df_df, &min_bits_df_df, &huff_dim_df_df,
                                          &huff_pair_type_df_df);
  if (error) {
    return error;
  }

  min_bits_df_df += 2;

  if (min_bits_df_df < min_bits_all) {
    min_bits_all = min_bits_df_df;
  }
  error = ixheaace_mps_212_calc_huff_bits(data_diff_freq[0], data_diff_time[1], data_type,
                                          IXHEAACE_MPS_SAC_DIFF_FREQ, IXHEAACE_MPS_SAC_DIFF_TIME,
                                          data_bands, lav_df_dt, &min_bits_df_dt, &huff_dim_df_dt,
                                          &huff_pair_type_df_dt);
  if (error) {
    return error;
  }

  min_bits_df_dt += 2;

  if (min_bits_df_dt < min_bits_all) {
    min_bits_all = min_bits_df_dt;
  }

  if (allow_diff_time_back_flag) {
    error = ixheaace_mps_212_calc_huff_bits(
        data_diff_time[0], data_diff_freq[1], data_type, IXHEAACE_MPS_SAC_DIFF_TIME,
        IXHEAACE_MPS_SAC_DIFF_FREQ, data_bands, lav_dtbw_df, &min_bits_dtbw_df, &huff_dim_dtbw_df,
        &huff_pair_type_dtbw_df);
    if (error) {
      return error;
    }

    min_bits_dtbw_df += 2;

    if (min_bits_dtbw_df < min_bits_all) {
      min_bits_all = min_bits_dtbw_df;
    }

    error = ixheaace_mps_212_calc_huff_bits(
        data_diff_time[0], data_diff_time[1], data_type, IXHEAACE_MPS_SAC_DIFF_TIME,
        IXHEAACE_MPS_SAC_DIFF_TIME, data_bands, lav_dt_dt, &min_bits_dt_dt, &huff_dim_dt_dt,
        &huff_pair_type_dt_dt);
    if (error) {
      return error;
    }

    min_bits_dt_dt += 2;

    if (min_bits_dt_dt < min_bits_all) {
      min_bits_all = min_bits_dt_dt;
    }
  }

  pcm_coding_flag = (min_bits_all == num_pcm_bits);

  ixheaace_write_bits(pstr_bit_buf, pcm_coding_flag, 1);

  if (pcm_coding_flag) {
    ixheaace_mps_212_apply_pcm_coding(pstr_bit_buf, pp_in_data[set_idx] + start_band,
                                      pp_in_data[set_idx + 1] + start_band, quant_offset,
                                      num_pcm_val, quant_levels);
  } else {
    min_found = 0;

    if (min_bits_all == min_bits_df_df) {
      ixheaace_write_bits(pstr_bit_buf, IXHEAACE_MPS_SAC_DIFF_FREQ, 1);
      ixheaace_write_bits(pstr_bit_buf, IXHEAACE_MPS_SAC_DIFF_FREQ, 1);

      error = ixheaace_mps_212_apply_huff_coding(
          pstr_bit_buf, data_diff_freq[0], data_diff_freq[1], data_type,
          IXHEAACE_MPS_SAC_DIFF_FREQ, IXHEAACE_MPS_SAC_DIFF_FREQ, data_bands, lav_df_df,
          huff_dim_df_df, huff_pair_type_df_df);
      if (error) {
        return error;
      }
      min_found = 1;
    }

    if (!min_found && (min_bits_all == min_bits_df_dt)) {
      ixheaace_write_bits(pstr_bit_buf, IXHEAACE_MPS_SAC_DIFF_FREQ, 1);
      ixheaace_write_bits(pstr_bit_buf, IXHEAACE_MPS_SAC_DIFF_TIME, 1);

      error = ixheaace_mps_212_apply_huff_coding(
          pstr_bit_buf, data_diff_freq[0], data_diff_time[1], data_type,
          IXHEAACE_MPS_SAC_DIFF_FREQ, IXHEAACE_MPS_SAC_DIFF_TIME, data_bands, lav_df_dt,
          huff_dim_df_dt, huff_pair_type_df_dt);
      if (error) {
        return error;
      }
      min_found = 1;
    }

    if (allow_diff_time_back_flag) {
      if (!min_found && (min_bits_all == min_bits_dtbw_df)) {
        ixheaace_write_bits(pstr_bit_buf, IXHEAACE_MPS_SAC_DIFF_TIME, 1);
        ixheaace_write_bits(pstr_bit_buf, IXHEAACE_MPS_SAC_DIFF_FREQ, 1);

        error = ixheaace_mps_212_apply_huff_coding(
            pstr_bit_buf, data_diff_time[0], data_diff_freq[1], data_type,
            IXHEAACE_MPS_SAC_DIFF_TIME, IXHEAACE_MPS_SAC_DIFF_FREQ, data_bands, lav_dtbw_df,
            huff_dim_dtbw_df, huff_pair_type_dtbw_df);
        if (error) {
          return error;
        }
        min_found = 1;
      }
      if (!min_found && (min_bits_all == min_bits_dt_dt)) {
        ixheaace_write_bits(pstr_bit_buf, IXHEAACE_MPS_SAC_DIFF_TIME, 1);
        ixheaace_write_bits(pstr_bit_buf, IXHEAACE_MPS_SAC_DIFF_TIME, 1);

        error = ixheaace_mps_212_apply_huff_coding(
            pstr_bit_buf, data_diff_time[0], data_diff_time[1], data_type,
            IXHEAACE_MPS_SAC_DIFF_TIME, IXHEAACE_MPS_SAC_DIFF_TIME, data_bands, lav_dt_dt,
            huff_dim_dt_dt, huff_pair_type_dt_dt);
        if (error) {
          return error;
        }
      }
    }
  }

  return IA_NO_ERROR;
}

IA_ERRORCODE ixheaace_mps_212_ec_data_single_enc(
    ixheaace_bit_buf_handle pstr_bit_buf, WORD16 pp_in_data[][IXHEAACE_MPS_SAC_MAX_FREQ_BANDS],
    WORD16 p_in_data[IXHEAACE_MPS_SAC_MAX_FREQ_BANDS], const WORD32 data_type,
    const WORD32 set_idx, const WORD32 start_band, const WORD16 data_bands,
    const WORD32 coarse_flag, const WORD32 independency_flag) {
  IA_ERRORCODE error;
  WORD16 band = 0;
  WORD16 quant_levels = 0, quant_offset = 0, num_pcm_val = 0;
  WORD16 pcm_coding_flag = 0;
  WORD16 min_bits_all = 0;
  WORD16 min_found = 0;
  WORD16 huff_dim_df = 0;
  WORD16 huff_pair_type_df = 0;
  WORD16 huff_dim_dt = 0;
  WORD16 huff_pair_type_dt = 0;
  WORD16 allow_diff_time_back_flag = !independency_flag || (set_idx > 0);
  WORD16 num_pcm_bits = -1;
  WORD16 min_bits_df = -1;
  WORD16 min_bits_dt = -1;

  WORD16 quant_data_msb[IXHEAACE_MPS_SAC_MAX_FREQ_BANDS] = {0};
  WORD16 quant_data_hist_msb[IXHEAACE_MPS_SAC_MAX_FREQ_BANDS] = {0};
  WORD16 data_diff_freq[IXHEAACE_MPS_SAC_MAX_FREQ_BANDS] = {0};
  WORD16 data_diff_time[IXHEAACE_MPS_SAC_MAX_FREQ_BANDS + 2] = {0};
  WORD16 *p_quant_data_msb;
  WORD16 *p_quant_data_hist_msb = NULL;
  WORD16 lav_df[2] = {-1, -1};
  WORD16 lav_dt[2] = {-1, -1};

  if (data_type == IXHEAACE_MPS_SAC_DATA_TYPE_ICC) {
    if (coarse_flag) {
      quant_levels = 4;
      quant_offset = 0;
    } else {
      quant_levels = 8;
      quant_offset = 0;
    }
  } else if (data_type == IXHEAACE_MPS_SAC_DATA_TYPE_CLD) {
    if (coarse_flag) {
      quant_levels = 15;
      quant_offset = 7;
    } else {
      quant_levels = 31;
      quant_offset = 15;
    }
  } else {
    return IA_EXHEAACE_EXE_FATAL_MPS_INVALID_HUFF_DATA_TYPE;
  }
  if (quant_offset != 0) {
    for (band = 0; band < data_bands; band++) {
      quant_data_msb[band] = pp_in_data[set_idx][start_band + band] + quant_offset;
    }
    p_quant_data_msb = quant_data_msb;
  } else {
    p_quant_data_msb = pp_in_data[set_idx] + start_band;
  }

  if (allow_diff_time_back_flag) {
    if (quant_offset != 0) {
      for (band = 0; band < data_bands; band++) {
        quant_data_hist_msb[band] = p_in_data[start_band + band] + quant_offset;
      }
      p_quant_data_hist_msb = quant_data_hist_msb;
    } else {
      p_quant_data_hist_msb = p_in_data + start_band;
    }
  }

  data_diff_freq[0] = p_quant_data_msb[0];

  for (band = 1; band < data_bands; band++) {
    data_diff_freq[band] = p_quant_data_msb[band] - p_quant_data_msb[band - 1];
  }

  if (allow_diff_time_back_flag) {
    data_diff_time[0] = p_quant_data_msb[0];
    data_diff_time[1] = p_quant_data_hist_msb[0];

    for (band = 0; band < data_bands; band++) {
      data_diff_time[band + 2] = p_quant_data_msb[band] - p_quant_data_hist_msb[band];
    }
  }

  num_pcm_bits = ixheaace_mps_212_calc_pcm_bits(data_bands, quant_levels);
  num_pcm_val = data_bands;
  min_bits_all = num_pcm_bits;

  error = ixheaace_mps_212_calc_huff_bits(
      data_diff_freq, NULL, data_type, IXHEAACE_MPS_SAC_DIFF_FREQ, IXHEAACE_MPS_SAC_DIFF_FREQ,
      data_bands, lav_df, &min_bits_df, &huff_dim_df, &huff_pair_type_df);
  if (error) {
    return error;
  }

  if (allow_diff_time_back_flag) min_bits_df += 1;

  if (min_bits_df < min_bits_all) {
    min_bits_all = min_bits_df;
  }
  if (allow_diff_time_back_flag) {
    error = ixheaace_mps_212_calc_huff_bits(
        data_diff_time, NULL, data_type, IXHEAACE_MPS_SAC_DIFF_TIME, IXHEAACE_MPS_SAC_DIFF_TIME,
        data_bands, lav_dt, &min_bits_dt, &huff_dim_dt, &huff_pair_type_dt);
    if (error) {
      return error;
    }

    min_bits_dt += 1;

    if (min_bits_dt < min_bits_all) {
      min_bits_all = min_bits_dt;
    }
  }
  pcm_coding_flag = (min_bits_all == num_pcm_bits);

  ixheaace_write_bits(pstr_bit_buf, pcm_coding_flag, 1);

  if (pcm_coding_flag) {
    ixheaace_mps_212_apply_pcm_coding(pstr_bit_buf, pp_in_data[set_idx] + start_band, NULL,
                                      quant_offset, num_pcm_val, quant_levels);
  } else {
    min_found = 0;
    if (min_bits_all == min_bits_df) {
      if (allow_diff_time_back_flag) {
        ixheaace_write_bits(pstr_bit_buf, IXHEAACE_MPS_SAC_DIFF_FREQ, 1);
      }

      error = ixheaace_mps_212_apply_huff_coding(
          pstr_bit_buf, data_diff_freq, NULL, data_type, IXHEAACE_MPS_SAC_DIFF_FREQ,
          IXHEAACE_MPS_SAC_DIFF_FREQ, data_bands, lav_df, huff_dim_df, huff_pair_type_df);
      if (error) {
        return error;
      }
      min_found = 1;
    }

    if (allow_diff_time_back_flag) {
      if (!min_found && (min_bits_all == min_bits_dt)) {
        ixheaace_write_bits(pstr_bit_buf, IXHEAACE_MPS_SAC_DIFF_TIME, 1);

        error = ixheaace_mps_212_apply_huff_coding(
            pstr_bit_buf, data_diff_time, NULL, data_type, IXHEAACE_MPS_SAC_DIFF_TIME,
            IXHEAACE_MPS_SAC_DIFF_TIME, data_bands, lav_dt, huff_dim_dt, huff_pair_type_dt);
        if (error) {
          return error;
        }
      }
    }
  }

  return IA_NO_ERROR;
}

VOID ixheaace_mps_515_ec_data_pair_enc(ixheaace_bit_buf_handle pstr_bit_buf,
                                         WORD32 pp_in_data[][MAXBANDS],
                                         WORD32 p_in_data[MAXBANDS], WORD32 data_type,
                                         WORD32 set_idx, WORD32 start_band, WORD32 data_bands,
                                         WORD32 pair_flag, WORD32 coarse_flag,
                                         WORD32 independency_flag) {
  WORD32 dummy = 0;
  WORD32 quant_levels = 0, quant_offset = 0, num_pcm_val = 0;

  WORD32 split_lsb_flag = 0;
  WORD32 pcm_coding_flag = 0;

  WORD32 min_bits_df_df = -1;
  WORD32 min_bits_df_dt = -1;
  WORD32 min_bits_dtbw_df = -1;
  WORD32 min_bits_dtfw_df = -1;

  WORD32 min_bits_dt_dt = -1;
  WORD32 allow_diff_time_back_flag = !independency_flag || (set_idx > 0);

  WORD32 num_lsb_bits[2] = {0, 0};
  WORD32 num_pcm_bits = 0;

  WORD32 a_data_hist[MAXBANDS] = {0};
  WORD32 aa_data_pair[2][MAXBANDS] = {{0}};

  WORD32 quant_data_lsb[2][MAXBANDS] = {{0}};
  WORD32 quant_data_msb[2][MAXBANDS] = {{0}};

  WORD32 quant_data_hist_lsb[MAXBANDS] = {0};
  WORD32 quant_data_hist_msb[MAXBANDS] = {0};

  WORD32 data_diff_freq[2][MAXBANDS] = {{0}};
  WORD32 data_diff_time_bw[2][MAXBANDS + 1] = {{0}};
  WORD32 data_diff_time_fw[MAXBANDS + 1] = {0};

  WORD32 *p_data_pcm[2] = {NULL};
  WORD32 *p_data_diff_freq[2] = {NULL};
  WORD32 *p_data_diff_time_bw[2] = {NULL};
  WORD32 *p_data_diff_time_fw = NULL;

  WORD32 min_bits_all = 0;
  WORD32 min_found = 0;

  switch (data_type) {
    case IXHEAACE_MPS_SAC_DATA_TYPE_CLD:
      if (coarse_flag) {
        split_lsb_flag = 0;
        quant_levels = 15;
        quant_offset = 7;
      } else {
        split_lsb_flag = 0;
        quant_levels = 31;
        quant_offset = 15;
      }

      break;

    case IXHEAACE_MPS_SAC_DATA_TYPE_ICC:
      if (coarse_flag) {
        split_lsb_flag = 0;
        quant_levels = 4;
        quant_offset = 0;
      } else {
        split_lsb_flag = 0;
        quant_levels = 8;
        quant_offset = 0;
      }

      break;

    case IXHEAACE_MPS_SAC_DATA_TYPE_CPC:
      if (coarse_flag) {
        split_lsb_flag = 0;
        quant_levels = 26;
        quant_offset = 0;
      } else {
        split_lsb_flag = 1;
        quant_levels = 51;
        quant_offset = 0;
      }

      break;

    default:
      return;
  }

  memcpy(a_data_hist, p_in_data + start_band, sizeof(WORD32) * data_bands);

  memcpy(aa_data_pair[0], pp_in_data[set_idx] + start_band, sizeof(int) * data_bands);
  p_data_pcm[0] = aa_data_pair[0];
  if (pair_flag) {
    memcpy(aa_data_pair[1], pp_in_data[set_idx + 1] + start_band, sizeof(int) * data_bands);
    p_data_pcm[1] = aa_data_pair[1];
  }

  num_lsb_bits[0] =
      ixheaace_mps_515_split_lsb(aa_data_pair[0], quant_offset, split_lsb_flag ? 1 : 0,
                                 data_bands, quant_data_lsb[0], quant_data_msb[0]);

  if (pair_flag) {
    num_lsb_bits[1] =
        ixheaace_mps_515_split_lsb(aa_data_pair[1], quant_offset, split_lsb_flag ? 1 : 0,
                                   data_bands, quant_data_lsb[1], quant_data_msb[1]);
  }
  (VOID) num_lsb_bits;
  if (allow_diff_time_back_flag) {
    dummy = ixheaace_mps_515_split_lsb(a_data_hist, quant_offset, split_lsb_flag ? 1 : 0,
                                       data_bands, quant_data_hist_lsb, quant_data_hist_msb);
    (VOID) dummy;
  }

  ixheaace_mps_515_calc_diff_freq(quant_data_msb[0], data_diff_freq[0], data_bands);
  p_data_diff_freq[0] = data_diff_freq[0];

  if (pair_flag) {
    ixheaace_mps_515_calc_diff_freq(quant_data_msb[1], data_diff_freq[1], data_bands);
    p_data_diff_freq[1] = data_diff_freq[1];
  }

  if (allow_diff_time_back_flag) {
    ixheaace_mps_515_calc_diff_time(quant_data_msb[0], quant_data_hist_msb,
                                    data_diff_time_bw[0] + 1,
                                    IXHEAACE_MPS_SAC_DIRECTION_BACKWARDS, data_bands);
    p_data_diff_time_bw[0] = data_diff_time_bw[0];
  }

  if (pair_flag) {
    ixheaace_mps_515_calc_diff_time(quant_data_msb[1], quant_data_msb[0],
                                    data_diff_time_bw[1] + 1,
                                    IXHEAACE_MPS_SAC_DIRECTION_BACKWARDS, data_bands);
    p_data_diff_time_bw[1] = data_diff_time_bw[1];

    ixheaace_mps_515_calc_diff_time(quant_data_msb[1], quant_data_msb[0], data_diff_time_fw + 1,
                                    IXHEAACE_MPS_SAC_DIRECTION_FORWARDS, data_bands);
    p_data_diff_time_fw = data_diff_time_fw;
  }

  if (pair_flag) {
    num_pcm_bits = ixheaace_mps_515_calc_pcm_bits(2 * data_bands, quant_levels);
    num_pcm_val = 2 * data_bands;
  } else {
    num_pcm_bits = ixheaace_mps_515_calc_pcm_bits(data_bands, quant_levels);
    num_pcm_val = data_bands;
  }

  min_bits_all = num_pcm_bits;

  if ((p_data_diff_freq[0] != NULL) || (p_data_diff_freq[1] != NULL)) {
    min_bits_df_df = ixheaace_mps_515_calc_huff_bits(p_data_diff_freq[0], p_data_diff_freq[1],
                                                     data_type, IXHEAACE_MPS_SAC_DIFF_FREQ,
                                                     IXHEAACE_MPS_SAC_DIFF_FREQ, data_bands);

    if (pair_flag || allow_diff_time_back_flag) min_bits_df_df += 1;
    if (pair_flag) min_bits_df_df += 1;

    if (min_bits_df_df < min_bits_all) {
      min_bits_all = min_bits_df_df;
    }
  }

  if ((p_data_diff_freq[0] != NULL) || (p_data_diff_time_bw[1] != NULL)) {
    min_bits_df_dt = ixheaace_mps_515_calc_huff_bits(p_data_diff_freq[0], p_data_diff_time_bw[1],
                                                     data_type, IXHEAACE_MPS_SAC_DIFF_FREQ,
                                                     IXHEAACE_MPS_SAC_DIFF_TIME, data_bands);

    if (pair_flag || allow_diff_time_back_flag) min_bits_df_dt += 1;
    if (pair_flag) min_bits_df_dt += 1;

    if (min_bits_df_dt < min_bits_all) {
      min_bits_all = min_bits_df_dt;
    }
  }

  if ((p_data_diff_time_fw != NULL) || (p_data_diff_freq[1] != NULL)) {
    min_bits_dtfw_df = ixheaace_mps_515_calc_huff_bits(p_data_diff_time_fw, p_data_diff_freq[1],
                                                       data_type, IXHEAACE_MPS_SAC_DIFF_TIME,
                                                       IXHEAACE_MPS_SAC_DIFF_FREQ, data_bands);

    if (pair_flag || allow_diff_time_back_flag) min_bits_dtfw_df += 1;
    if (pair_flag && allow_diff_time_back_flag) min_bits_dtfw_df += 1;

    if (pair_flag && allow_diff_time_back_flag) {
      min_bits_dtfw_df += 1;
    }

    if (min_bits_dtfw_df < min_bits_all) {
      min_bits_all = min_bits_dtfw_df;
    }
  }

  if (allow_diff_time_back_flag) {
    if ((p_data_diff_time_bw[0] != NULL) || (p_data_diff_freq[1] != NULL)) {
      min_bits_dtbw_df = ixheaace_mps_515_calc_huff_bits(
          p_data_diff_time_bw[0], p_data_diff_freq[1], data_type, IXHEAACE_MPS_SAC_DIFF_TIME,
          IXHEAACE_MPS_SAC_DIFF_FREQ, data_bands);

      min_bits_dtbw_df += 1;
      if (pair_flag) {
        min_bits_dtbw_df += 2;
      }

      if (min_bits_dtbw_df < min_bits_all) {
        min_bits_all = min_bits_dtbw_df;
      }
    }

    if ((p_data_diff_time_bw[0] != NULL) || (p_data_diff_time_bw[1] != NULL)) {
      min_bits_dt_dt = ixheaace_mps_515_calc_huff_bits(
          p_data_diff_time_bw[0], p_data_diff_time_bw[1], data_type, IXHEAACE_MPS_SAC_DIFF_TIME,
          IXHEAACE_MPS_SAC_DIFF_TIME, data_bands);

      min_bits_dt_dt += 1;
      if (pair_flag) min_bits_dt_dt += 1;

      if (min_bits_dt_dt < min_bits_all) {
        min_bits_all = min_bits_dt_dt;
      }
    }
  }

  pcm_coding_flag = (min_bits_all == num_pcm_bits);

  ixheaace_write_bits(pstr_bit_buf, pcm_coding_flag, 1);

  if (pcm_coding_flag) {
    if (data_bands >= PBC_MIN_BANDS) {
      ixheaace_write_bits(pstr_bit_buf, 0, 1);
    }

    ixheaace_mps_515_apply_pcm_coding(pstr_bit_buf, p_data_pcm[0], p_data_pcm[1], quant_offset,
                                      num_pcm_val, quant_levels);
  } else {
    min_found = 0;

    if (min_bits_all == min_bits_df_df) {
      if (pair_flag || allow_diff_time_back_flag) {
        ixheaace_write_bits(pstr_bit_buf, IXHEAACE_MPS_SAC_DIFF_FREQ, 1);
      }

      if (pair_flag) {
        ixheaace_write_bits(pstr_bit_buf, IXHEAACE_MPS_SAC_DIFF_FREQ, 1);
      }

      ixheaace_mps_515_apply_huff_coding(pstr_bit_buf, p_data_diff_freq[0], p_data_diff_freq[1],
                                         data_type, IXHEAACE_MPS_SAC_DIFF_FREQ,
                                         IXHEAACE_MPS_SAC_DIFF_FREQ, data_bands);

      min_found = 1;
    }

    if (!min_found && (min_bits_all == min_bits_df_dt)) {
      if (pair_flag || allow_diff_time_back_flag) {
        ixheaace_write_bits(pstr_bit_buf, IXHEAACE_MPS_SAC_DIFF_FREQ, 1);
      }

      if (pair_flag) {
        ixheaace_write_bits(pstr_bit_buf, IXHEAACE_MPS_SAC_DIFF_TIME, 1);
      }

      ixheaace_mps_515_apply_huff_coding(
          pstr_bit_buf, p_data_diff_freq[0], p_data_diff_time_bw[1], data_type,
          IXHEAACE_MPS_SAC_DIFF_FREQ, IXHEAACE_MPS_SAC_DIFF_TIME, data_bands);

      min_found = 1;
    }

    if (!min_found && (min_bits_all == min_bits_dtfw_df)) {
      if (pair_flag || allow_diff_time_back_flag) {
        ixheaace_write_bits(pstr_bit_buf, IXHEAACE_MPS_SAC_DIFF_TIME, 1);
      }

      if (pair_flag && allow_diff_time_back_flag) {
        ixheaace_write_bits(pstr_bit_buf, IXHEAACE_MPS_SAC_DIFF_FREQ, 1);
      }

      ixheaace_mps_515_apply_huff_coding(pstr_bit_buf, p_data_diff_time_fw, p_data_diff_freq[1],
                                         data_type, IXHEAACE_MPS_SAC_DIFF_TIME,
                                         IXHEAACE_MPS_SAC_DIFF_FREQ, data_bands);

      if (pair_flag && allow_diff_time_back_flag) {
        ixheaace_write_bits(pstr_bit_buf, IXHEAACE_MPS_SAC_DIRECTION_FORWARDS, 1);
      }

      min_found = 1;
    }

    if (allow_diff_time_back_flag) {
      if (!min_found && (min_bits_all == min_bits_dtbw_df)) {
        ixheaace_write_bits(pstr_bit_buf, IXHEAACE_MPS_SAC_DIFF_TIME, 1);

        if (pair_flag) {
          ixheaace_write_bits(pstr_bit_buf, IXHEAACE_MPS_SAC_DIFF_FREQ, 1);
        }

        ixheaace_mps_515_apply_huff_coding(
            pstr_bit_buf, p_data_diff_time_bw[0], p_data_diff_freq[1], data_type,
            IXHEAACE_MPS_SAC_DIFF_TIME, IXHEAACE_MPS_SAC_DIFF_FREQ, data_bands);

        if (pair_flag) {
          ixheaace_write_bits(pstr_bit_buf, IXHEAACE_MPS_SAC_DIRECTION_BACKWARDS, 1);
        }

        min_found = 1;
      }

      if (!min_found && (min_bits_all == min_bits_dt_dt)) {
        ixheaace_write_bits(pstr_bit_buf, IXHEAACE_MPS_SAC_DIFF_TIME, 1);

        if (pair_flag) {
          ixheaace_write_bits(pstr_bit_buf, IXHEAACE_MPS_SAC_DIFF_TIME, 1);
        }

        ixheaace_mps_515_apply_huff_coding(
            pstr_bit_buf, p_data_diff_time_bw[0], p_data_diff_time_bw[1], data_type,
            IXHEAACE_MPS_SAC_DIFF_TIME, IXHEAACE_MPS_SAC_DIFF_TIME, data_bands);
      }
    }

    if (split_lsb_flag) {
      ixheaace_mps_515_apply_lsb_coding(pstr_bit_buf, quant_data_lsb[0], 1, data_bands);

      if (pair_flag) {
        ixheaace_mps_515_apply_lsb_coding(pstr_bit_buf, quant_data_lsb[1], 1, data_bands);
      }
    }
  }

  return;
}
