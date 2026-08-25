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

#include <math.h>
#include <stdlib.h>
#include "ixheaac_type_def.h"
#include "ixheaac_constants.h"
#include "ixheaace_aac_constants.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops16.h"
#include "ixheaac_basic_ops40.h"

#include "ixheaace_sbr_header.h"
#include "ixheaace_sbr_def.h"
#include "ixheaace_resampler.h"
#include "ixheaace_sbr_rom.h"
#include "ixheaace_common_rom.h"
#include "ixheaace_bitbuffer.h"
#include "ixheaace_sbr_hbe.h"
#include "ixheaace_sbr_qmf_enc.h"
#include "ixheaace_sbr_tran_det.h"
#include "ixheaace_sbr_frame_info_gen.h"
#include "ixheaace_sbr_env_est.h"
#include "ixheaace_sbr_code_envelope.h"
#include "ixheaace_sbr_main.h"
#include "ixheaace_sbr_missing_harmonics_det.h"
#include "ixheaace_sbr_inv_filtering_estimation.h"
#include "ixheaace_sbr_noise_floor_est.h"

#include "ixheaace_sbr_ton_corr.h"
#include "iusace_esbr_pvc.h"
#include "iusace_esbr_inter_tes.h"
#include "ixheaace_sbr.h"
#include "ixheaace_sbr_cmondata.h"
#include "iusace_esbr_pvc.h"

#include "ixheaace_sbr_hybrid.h"
#include "ixheaace_sbr_ps_enc.h"
#include "ixheaace_sbr_ps_bitenc.h"
#include "ixheaace_sbr_write_bitstream.h"
#include "ixheaac_error_standards.h"
#include "ixheaace_error_codes.h"
#include "ixheaace_common_utils.h"

static WORD32 ixheaace_get_esbr_ext_data_size(ixheaace_str_esbr_bs_data *pstr_esbr_bs_data) {
  WORD32 num_bits = 1;
  if (1 == pstr_esbr_bs_data->sbr_num_chan) {
    num_bits += 1;
    if (pstr_esbr_bs_data->sbr_patching_mode[0] == 0) {
      num_bits += 2;
      if (pstr_esbr_bs_data->sbr_pitchin_flags[0] == 1) {
        num_bits += 7;
      }
    }
  } else if (2 == pstr_esbr_bs_data->sbr_num_chan) {
    if (pstr_esbr_bs_data->sbr_coupling) {
      num_bits += 1;
      if (pstr_esbr_bs_data->sbr_patching_mode[0] == 0) {
        num_bits += 2;
        if (pstr_esbr_bs_data->sbr_pitchin_flags[0] == 1) {
          num_bits += 7;
        }
      }
    } else {
      num_bits += 1;
      if (pstr_esbr_bs_data->sbr_patching_mode[0] == 0) {
        num_bits += 2;
        if (pstr_esbr_bs_data->sbr_pitchin_flags[0] == 1) {
          num_bits += 7;
        }
      }
      num_bits += 1;
      if (pstr_esbr_bs_data->sbr_patching_mode[1] == 0) {
        num_bits += 2;
        if (pstr_esbr_bs_data->sbr_pitchin_flags[1] == 1) {
          num_bits += 7;
        }
      }
    }
  } else {
    num_bits = 0;
  }
  if (num_bits != 0 && num_bits < 6) {
    num_bits = 6;
  }
  return num_bits;
}
static WORD32 iusace_encode_pvc_envelope(ixheaace_bit_buf_handle pstr_bs_handle,
                                         ixheaace_pvc_bs_info *pstr_pvc_bs_data,
                                         WORD32 usac_indep_flag) {
  WORD32 payload_cnt_bits = 0;
  payload_cnt_bits += ixheaace_write_bits(pstr_bs_handle, pstr_pvc_bs_data->div_mode,
                                          IXHEAACE_ESBR_PVC_DIV_MODE_BITS);
  payload_cnt_bits += ixheaace_write_bits(pstr_bs_handle, pstr_pvc_bs_data->ns_mode,
                                          IXHEAACE_ESBR_PVC_NS_MODE_BITS);

  if (0 == pstr_pvc_bs_data->div_mode) {
    if (1 == usac_indep_flag) {
      payload_cnt_bits += ixheaace_write_bits(pstr_bs_handle, pstr_pvc_bs_data->pvc_id_bs[0],
                                              IXHEAACE_ESBR_PVC_ID_BITS);
    } else {
      if (1 == pstr_pvc_bs_data->grid_info[0]) {
        payload_cnt_bits += ixheaace_write_bits(pstr_bs_handle, 0, IXHEAACE_ESBR_PVC_REUSE_BITS);
        payload_cnt_bits += ixheaace_write_bits(pstr_bs_handle, pstr_pvc_bs_data->pvc_id_bs[0],
                                                IXHEAACE_ESBR_PVC_ID_BITS);
      } else {
        payload_cnt_bits += ixheaace_write_bits(pstr_bs_handle, 1, IXHEAACE_ESBR_PVC_REUSE_BITS);
      }
    }
  } else if (pstr_pvc_bs_data->div_mode <= 3) {
    /* Do nothing */
  } else {
    WORD32 gi, is_grid_info;
    for (gi = 0; gi < pstr_pvc_bs_data->num_grid_info; gi++) {
      if (gi == 0 && 1 == usac_indep_flag) {
        is_grid_info = 1;
      } else {
        is_grid_info = pstr_pvc_bs_data->grid_info[gi];
        payload_cnt_bits +=
            ixheaace_write_bits(pstr_bs_handle, is_grid_info, IXHEAACE_ESBR_PVC_GRID_INFO_BITS);
      }
      if (is_grid_info) {
        payload_cnt_bits += ixheaace_write_bits(pstr_bs_handle, pstr_pvc_bs_data->pvc_id_bs[gi],
                                                IXHEAACE_ESBR_PVC_ID_BITS);
      }
    }
  }
  return payload_cnt_bits;
}
static WORD32 ia_enhaacplus_enc_ceil_ln2(WORD32 x) {
  WORD32 tmp = -1;

  while (ixheaac_shl32(1, ++tmp) < x)
    ;

  return (tmp);
}

static WORD32 ixheaace_encode_sbr_grid(ixheaace_pstr_sbr_env_data pstr_sbr_env_info,
                                       ixheaace_bit_buf_handle pstr_bs_handle,
                                       ixheaace_sbr_codec_type sbr_codec) {
  WORD32 payload_cnt_bits = 0;
  WORD32 i, tmp_var;

  if (ELD_SBR != sbr_codec) {
    if (HEAAC_SBR == sbr_codec ||
        (USAC_SBR == sbr_codec && pstr_sbr_env_info->sbr_pvc_mode == 0)) {
      payload_cnt_bits += ixheaace_write_bits(
          pstr_bs_handle, pstr_sbr_env_info->pstr_sbr_bs_grid->frame_type, SBR_CLA_BITS);

      switch (pstr_sbr_env_info->pstr_sbr_bs_grid->frame_type) {
        case IXHEAACE_FIXFIX:

          tmp_var = ia_enhaacplus_enc_ceil_ln2(pstr_sbr_env_info->pstr_sbr_bs_grid->bs_num_env);

          payload_cnt_bits += ixheaace_write_bits(pstr_bs_handle, tmp_var, SBR_ENV_BITS);

          payload_cnt_bits +=
              ixheaace_write_bits(pstr_bs_handle, pstr_sbr_env_info->freq_res_fix, SBR_RES_BITS);
          break;

        case IXHEAACE_FIXVAR:
        case IXHEAACE_VARFIX:

          if (pstr_sbr_env_info->pstr_sbr_bs_grid->frame_type == IXHEAACE_FIXVAR) {
            tmp_var = pstr_sbr_env_info->pstr_sbr_bs_grid->bs_abs_bord - 16;
          } else {
            tmp_var = pstr_sbr_env_info->pstr_sbr_bs_grid->bs_abs_bord;
          }

          payload_cnt_bits += ixheaace_write_bits(pstr_bs_handle, tmp_var, SBR_ABS_BITS);

          payload_cnt_bits += ixheaace_write_bits(
              pstr_bs_handle, pstr_sbr_env_info->pstr_sbr_bs_grid->n, SBR_NUM_BITS);

          for (i = 0; i < pstr_sbr_env_info->pstr_sbr_bs_grid->n; i++) {
            tmp_var = (pstr_sbr_env_info->pstr_sbr_bs_grid->bs_rel_bord[i] - 2) >> 1;

            payload_cnt_bits += ixheaace_write_bits(pstr_bs_handle, tmp_var, SBR_REL_BITS);
          }

          tmp_var = ia_enhaacplus_enc_ceil_ln2(pstr_sbr_env_info->pstr_sbr_bs_grid->n + 2);

          payload_cnt_bits += ixheaace_write_bits(
              pstr_bs_handle, pstr_sbr_env_info->pstr_sbr_bs_grid->p, (UWORD8)tmp_var);

          for (i = 0; i < pstr_sbr_env_info->pstr_sbr_bs_grid->n + 1; i++) {
            payload_cnt_bits += ixheaace_write_bits(
                pstr_bs_handle, pstr_sbr_env_info->pstr_sbr_bs_grid->v_f[i], SBR_RES_BITS);
          }
          break;

        case IXHEAACE_VARVAR:

          tmp_var = pstr_sbr_env_info->pstr_sbr_bs_grid->bs_abs_bord_0;

          payload_cnt_bits += ixheaace_write_bits(pstr_bs_handle, tmp_var, SBR_ABS_BITS);
          tmp_var = pstr_sbr_env_info->pstr_sbr_bs_grid->bs_abs_bord_1 - 16;

          payload_cnt_bits += ixheaace_write_bits(pstr_bs_handle, tmp_var, SBR_ABS_BITS);

          payload_cnt_bits += ixheaace_write_bits(
              pstr_bs_handle, pstr_sbr_env_info->pstr_sbr_bs_grid->bs_num_rel_0, SBR_NUM_BITS);

          payload_cnt_bits += ixheaace_write_bits(
              pstr_bs_handle, pstr_sbr_env_info->pstr_sbr_bs_grid->bs_num_rel_1, SBR_NUM_BITS);

          for (i = 0; i < pstr_sbr_env_info->pstr_sbr_bs_grid->bs_num_rel_0; i++) {
            tmp_var = (pstr_sbr_env_info->pstr_sbr_bs_grid->bs_rel_bord_0[i] - 2) >> 1;

            payload_cnt_bits += ixheaace_write_bits(pstr_bs_handle, tmp_var, SBR_REL_BITS);
          }

          for (i = 0; i < pstr_sbr_env_info->pstr_sbr_bs_grid->bs_num_rel_1; i++) {
            tmp_var = (pstr_sbr_env_info->pstr_sbr_bs_grid->bs_rel_bord_1[i] - 2) >> 1;

            payload_cnt_bits += ixheaace_write_bits(pstr_bs_handle, tmp_var, SBR_REL_BITS);
          }

          tmp_var =
              ia_enhaacplus_enc_ceil_ln2(pstr_sbr_env_info->pstr_sbr_bs_grid->bs_num_rel_0 +
                                         pstr_sbr_env_info->pstr_sbr_bs_grid->bs_num_rel_1 + 2);

          payload_cnt_bits += ixheaace_write_bits(
              pstr_bs_handle, pstr_sbr_env_info->pstr_sbr_bs_grid->p, (UWORD8)tmp_var);

          tmp_var = pstr_sbr_env_info->pstr_sbr_bs_grid->bs_num_rel_0 +
                    pstr_sbr_env_info->pstr_sbr_bs_grid->bs_num_rel_1 + 1;

          for (i = 0; i < tmp_var; i++) {
            payload_cnt_bits += ixheaace_write_bits(
                pstr_bs_handle, pstr_sbr_env_info->pstr_sbr_bs_grid->v_f_lr[i], SBR_RES_BITS);
          }
          break;
        default:
          break;
      }
    } else {
      // If PVC mode is non-zero, bit stream parameters are updated here
      if (pstr_sbr_env_info->no_of_envelopes > 1) {
        payload_cnt_bits += ixheaace_write_bits(pstr_bs_handle, 8, SBR_PVC_NOISE_POSITION_BITS);
      } else {
        payload_cnt_bits += ixheaace_write_bits(pstr_bs_handle, 0, SBR_PVC_NOISE_POSITION_BITS);
      }
      payload_cnt_bits += ixheaace_write_bits(pstr_bs_handle, 0, SBR_PVC_VAR_LEN_HF_BITS);
    }
  } else {
    payload_cnt_bits += ixheaace_write_bits(
        pstr_bs_handle, pstr_sbr_env_info->pstr_sbr_bs_grid->frame_type, LDSBR_CLA_BITS);

    switch (pstr_sbr_env_info->pstr_sbr_bs_grid->frame_type) {
      case IXHEAACE_FIXFIX:
        tmp_var = ia_enhaacplus_enc_ceil_ln2(pstr_sbr_env_info->pstr_sbr_bs_grid->bs_num_env);
        payload_cnt_bits += ixheaace_write_bits(pstr_bs_handle, tmp_var, SBR_ENV_BITS);
        if (pstr_sbr_env_info->pstr_sbr_bs_grid->bs_num_env == 1) {
          payload_cnt_bits += ixheaace_write_bits(
              pstr_bs_handle, pstr_sbr_env_info->curr_sbr_amp_res, SI_SBR_AMP_RES_BITS);
        }
        payload_cnt_bits += ixheaace_write_bits(
            pstr_bs_handle, pstr_sbr_env_info->pstr_sbr_bs_grid->v_f[0], SBR_RES_BITS);
        break;

      case IXHEAACE_LD_TRAN:
        payload_cnt_bits += ixheaace_write_bits(
            pstr_bs_handle, pstr_sbr_env_info->pstr_sbr_bs_grid->bs_transient_position,
            IXHEAACE_SBR_TRAN_BITS);
        for (i = 0; i < pstr_sbr_env_info->pstr_sbr_bs_grid->bs_num_env; i++) {
          payload_cnt_bits += ixheaace_write_bits(
              pstr_bs_handle, pstr_sbr_env_info->pstr_sbr_bs_grid->v_f[i], SBR_RES_BITS);
        }
        break;
      default:
        break;
    }
  }

  return payload_cnt_bits;
}

static WORD32 ixheaace_encode_sbr_dtdf(ixheaace_pstr_sbr_env_data pstr_sbr_env_info,
                                       ixheaace_bit_buf_handle pstr_bs_handle,
                                       ixheaace_sbr_codec_type sbr_codec, WORD32 usac_indep_flag,
                                       WORD32 sbr_pvc_mode) {
  WORD32 i, payload_cnt_bits = 0, num_of_noise_env;

  num_of_noise_env = (pstr_sbr_env_info->no_of_envelopes > 1) ? 2 : 1;

  if (USAC_SBR != sbr_codec) {
    for (i = 0; i < pstr_sbr_env_info->no_of_envelopes; ++i) {
      payload_cnt_bits +=
          ixheaace_write_bits(pstr_bs_handle, pstr_sbr_env_info->domain_vec[i], SBR_DIR_BITS);
    }
  }

  else {
    if (sbr_pvc_mode == 0) {
      WORD32 start_env = 0;
      if (1 == usac_indep_flag) {
        start_env = 1;
      }
      for (i = start_env; i < pstr_sbr_env_info->no_of_envelopes; ++i) {
        payload_cnt_bits +=
            ixheaace_write_bits(pstr_bs_handle, pstr_sbr_env_info->domain_vec[i], SBR_DIR_BITS);
      }
    } else {
      /* Do nothing */
    }
  }
  if (USAC_SBR != sbr_codec) {
    for (i = 0; i < num_of_noise_env; ++i) {
      payload_cnt_bits += ixheaace_write_bits(
          pstr_bs_handle, pstr_sbr_env_info->domain_vec_noise[i], SBR_DIR_BITS);
    }
  } else {
    WORD32 start_env = 0;
    if (1 == usac_indep_flag) {
      start_env = 1;
    }

    for (i = start_env; i < num_of_noise_env; ++i) {
      payload_cnt_bits += ixheaace_write_bits(
          pstr_bs_handle, pstr_sbr_env_info->domain_vec_noise[i], SBR_DIR_BITS);
    }
  }
  return payload_cnt_bits;
}

static WORD32 ixheaace_write_noise_lvl_data(ixheaace_pstr_sbr_env_data pstr_sbr_env_info,
                                            ixheaace_bit_buf_handle pstr_bs_handle,
                                            WORD32 coupling) {
  WORD32 j, i, payload_cnt_bits = 0;
  WORD32 n_noise_envelopes = ((pstr_sbr_env_info->no_of_envelopes > 1) ? 2 : 1);

  for (i = 0; i < n_noise_envelopes; i++) {
    switch (pstr_sbr_env_info->domain_vec_noise[i]) {
      case FREQ:

        if (coupling && pstr_sbr_env_info->balance) {
          payload_cnt_bits += ixheaace_write_bits(
              pstr_bs_handle,
              pstr_sbr_env_info->noise_level[i * pstr_sbr_env_info->noise_band_count],
              (UWORD8)pstr_sbr_env_info->si_sbr_start_noise_bits_balance);
        } else {
          payload_cnt_bits += ixheaace_write_bits(
              pstr_bs_handle,
              pstr_sbr_env_info->noise_level[i * pstr_sbr_env_info->noise_band_count],
              (UWORD8)pstr_sbr_env_info->si_sbr_start_noise_bits);
        }

        for (j = 1 + i * pstr_sbr_env_info->noise_band_count;
             j < (pstr_sbr_env_info->noise_band_count * (1 + i)); j++) {
          if (coupling) {
            if (pstr_sbr_env_info->balance) {
              payload_cnt_bits += ixheaace_write_bits(
                  pstr_bs_handle,
                  pstr_sbr_env_info
                      ->ptr_huff_tab_noise_bal_freq_c[pstr_sbr_env_info->noise_level[j] +
                                                      CODE_BCK_SCF_LAV_BALANCE11],
                  pstr_sbr_env_info
                      ->ptr_huff_tab_noise_bal_freq_l[pstr_sbr_env_info->noise_level[j] +
                                                      CODE_BCK_SCF_LAV_BALANCE11]);
            } else {
              payload_cnt_bits += ixheaace_write_bits(
                  pstr_bs_handle,
                  pstr_sbr_env_info
                      ->ptr_huff_tab_noise_lvl_freq_c[pstr_sbr_env_info->noise_level[j] +
                                                      CODE_BCK_SCF_LAV11],
                  pstr_sbr_env_info
                      ->ptr_huff_tab_noise_lvl_freq_l[pstr_sbr_env_info->noise_level[j] +
                                                      CODE_BCK_SCF_LAV11]);
            }
          } else {
            payload_cnt_bits += ixheaace_write_bits(
                pstr_bs_handle,
                pstr_sbr_env_info->ptr_huff_tab_noise_freq_c[pstr_sbr_env_info->noise_level[j] +
                                                             CODE_BCK_SCF_LAV11],
                pstr_sbr_env_info->ptr_huff_tab_noise_freq_l[pstr_sbr_env_info->noise_level[j] +
                                                             CODE_BCK_SCF_LAV11]);
          }
        }
        break;

      case TIME:
        for (j = i * pstr_sbr_env_info->noise_band_count;
             j < (pstr_sbr_env_info->noise_band_count * (1 + i)); j++) {
          if (coupling) {
            if (pstr_sbr_env_info->balance) {
              payload_cnt_bits += ixheaace_write_bits(
                  pstr_bs_handle,
                  pstr_sbr_env_info
                      ->ptr_huff_tab_noise_bal_time_c[pstr_sbr_env_info->noise_level[j] +
                                                      CODE_BCK_SCF_LAV_BALANCE11],
                  pstr_sbr_env_info
                      ->ptr_huff_tab_noise_bal_time_l[pstr_sbr_env_info->noise_level[j] +
                                                      CODE_BCK_SCF_LAV_BALANCE11]);
            } else {
              payload_cnt_bits += ixheaace_write_bits(
                  pstr_bs_handle,
                  pstr_sbr_env_info
                      ->ptr_huff_tab_noise_lvl_time_c[pstr_sbr_env_info->noise_level[j] +
                                                      CODE_BCK_SCF_LAV11],
                  pstr_sbr_env_info
                      ->ptr_huff_tab_noise_lvl_time_l[pstr_sbr_env_info->noise_level[j] +
                                                      CODE_BCK_SCF_LAV11]);
            }
          } else {
            payload_cnt_bits += ixheaace_write_bits(
                pstr_bs_handle,
                pstr_sbr_env_info
                    ->ptr_huff_tab_noise_lvl_time_c[pstr_sbr_env_info->noise_level[j] +
                                                    CODE_BCK_SCF_LAV11],
                pstr_sbr_env_info
                    ->ptr_huff_tab_noise_lvl_time_l[pstr_sbr_env_info->noise_level[j] +
                                                    CODE_BCK_SCF_LAV11]);
          }
        }
        break;
    }
  }
  return payload_cnt_bits;
}

static IA_ERRORCODE ixheaace_write_env_data(ixheaace_pstr_sbr_env_data pstr_sbr_env_info,
                                            ixheaace_bit_buf_handle pstr_bs_handle,
                                            WORD32 coupling, ixheaace_sbr_codec_type sbr_codec,
                                            WORD32 *ptr_payload_cnt_bits) {
  WORD32 j, i, delta;

  *ptr_payload_cnt_bits = 0;

  for (j = 0; j < pstr_sbr_env_info->no_of_envelopes; j++) {
    if (pstr_sbr_env_info->domain_vec[j] == FREQ) {
      if (coupling && pstr_sbr_env_info->balance) {
        *ptr_payload_cnt_bits +=
            ixheaace_write_bits(pstr_bs_handle, pstr_sbr_env_info->ienvelope[j][0],
                                (UWORD8)pstr_sbr_env_info->si_sbr_start_env_bits_balance);
      } else {
        *ptr_payload_cnt_bits +=
            ixheaace_write_bits(pstr_bs_handle, pstr_sbr_env_info->ienvelope[j][0],
                                (UWORD8)pstr_sbr_env_info->si_sbr_start_env_bits);
      }
    }

    for (i = 1 - pstr_sbr_env_info->domain_vec[j]; i < pstr_sbr_env_info->no_scf_bands[j]; i++) {
      delta = pstr_sbr_env_info->ienvelope[j][i];

      if (coupling && pstr_sbr_env_info->balance) {
        if (abs(delta) > pstr_sbr_env_info->code_book_scf_lav_balance) {
          return IA_EXHEAACE_EXE_FATAL_SBR_INVALID_CODEBOOK;
        }
      } else {
        if (abs(delta) > pstr_sbr_env_info->code_book_scf_lav) {
          return IA_EXHEAACE_EXE_FATAL_SBR_INVALID_CODEBOOK;
        }
      }

      if (coupling) {
        if (pstr_sbr_env_info->balance) {
          if (pstr_sbr_env_info->domain_vec[j]) {
            *ptr_payload_cnt_bits += ixheaace_write_bits(
                pstr_bs_handle,
                pstr_sbr_env_info
                    ->ptr_huff_tab_bal_time_c[delta +
                                              pstr_sbr_env_info->code_book_scf_lav_balance],
                pstr_sbr_env_info
                    ->ptr_huff_tab_bal_time_l[delta +
                                              pstr_sbr_env_info->code_book_scf_lav_balance]);
          } else {
            *ptr_payload_cnt_bits += ixheaace_write_bits(
                pstr_bs_handle,
                pstr_sbr_env_info
                    ->ptr_huff_tab_bal_freq_c[delta +
                                              pstr_sbr_env_info->code_book_scf_lav_balance],
                pstr_sbr_env_info
                    ->ptr_huff_tab_bal_freq_l[delta +
                                              pstr_sbr_env_info->code_book_scf_lav_balance]);
          }
        } else {
          if (pstr_sbr_env_info->domain_vec[j]) {
            *ptr_payload_cnt_bits += ixheaace_write_bits(
                pstr_bs_handle,
                pstr_sbr_env_info
                    ->ptr_huff_tab_lvl_time_c[delta + pstr_sbr_env_info->code_book_scf_lav],
                pstr_sbr_env_info
                    ->ptr_huff_tab_lvl_time_l[delta + pstr_sbr_env_info->code_book_scf_lav]);
          } else {
            *ptr_payload_cnt_bits += ixheaace_write_bits(
                pstr_bs_handle,
                pstr_sbr_env_info
                    ->ptr_huff_tab_lvl_freq_c[delta + pstr_sbr_env_info->code_book_scf_lav],
                pstr_sbr_env_info
                    ->ptr_huff_tab_lvl_freq_l[delta + pstr_sbr_env_info->code_book_scf_lav]);
          }
        }
      } else {
        if (pstr_sbr_env_info->domain_vec[j]) {
          *ptr_payload_cnt_bits += ixheaace_write_bits(
              pstr_bs_handle,
              pstr_sbr_env_info
                  ->ptr_huff_tab_time_c[delta + pstr_sbr_env_info->code_book_scf_lav],
              pstr_sbr_env_info
                  ->ptr_huff_tab_time_l[delta + pstr_sbr_env_info->code_book_scf_lav]);
        } else {
          *ptr_payload_cnt_bits += ixheaace_write_bits(
              pstr_bs_handle,
              pstr_sbr_env_info
                  ->ptr_huff_tab_freq_c[delta + pstr_sbr_env_info->code_book_scf_lav],
              pstr_sbr_env_info
                  ->ptr_huff_tab_freq_l[delta + pstr_sbr_env_info->code_book_scf_lav]);
        }
      }
    }
    if (USAC_SBR == sbr_codec) {
      if (1 == pstr_sbr_env_info->sbr_inter_tes) {
        *ptr_payload_cnt_bits +=
            ixheaace_write_bits(pstr_bs_handle, pstr_sbr_env_info->ptr_sbr_inter_tes_shape[j],
                                IXHEAACE_SBR_TES_SHAPE_BITS);
        if (1 == pstr_sbr_env_info->ptr_sbr_inter_tes_shape[j]) {
          *ptr_payload_cnt_bits += ixheaace_write_bits(
              pstr_bs_handle, pstr_sbr_env_info->ptr_sbr_inter_tes_shape_mode[j],
              IXHEAACE_SBR_TES_SHAPE_MODE_BITS);
        }
      }
    }
  }

  return IA_NO_ERROR;
}

static WORD32 ixheaace_write_synthetic_coding_data(ixheaace_pstr_sbr_env_data pstr_sbr_env_info,
                                                   ixheaace_bit_buf_handle pstr_bs_handle,
                                                   ixheaace_sbr_codec_type sbr_codec,
                                                   WORD32 sbr_pvc_mode)

{
  WORD32 i;
  WORD32 payload_cnt_bits = 0;

  payload_cnt_bits +=
      ixheaace_write_bits(pstr_bs_handle, pstr_sbr_env_info->add_harmonic_flag, 1);

  if (pstr_sbr_env_info->add_harmonic_flag) {
    for (i = 0; i < pstr_sbr_env_info->no_harmonics; i++) {
      payload_cnt_bits +=
          ixheaace_write_bits(pstr_bs_handle, pstr_sbr_env_info->add_harmonic[i], 1);
    }
    if (USAC_SBR == sbr_codec && 0 != sbr_pvc_mode) {
      if (pstr_sbr_env_info->sbr_sinusoidal_pos_flag) {
        payload_cnt_bits += ixheaace_write_bits(pstr_bs_handle, 1, 1);
        payload_cnt_bits += ixheaace_write_bits(pstr_bs_handle, 31, 5);
      } else {
        payload_cnt_bits += ixheaace_write_bits(pstr_bs_handle, 0, 1);
      }
    }
  }
  return payload_cnt_bits;
}

static IA_ERRORCODE ixheaace_encode_sbr_single_channel_element(
    ixheaace_pstr_sbr_env_data pstr_sbr_env_info, ixheaace_bit_buf_handle pstr_bs_handle,
    ixheaace_sbr_codec_type sbr_codec, WORD32 *ptr_num_bits)

{
  WORD32 payload_cnt_bits = 0;
  IA_ERRORCODE err_code = IA_NO_ERROR;

  if (sbr_codec != USAC_SBR) {
    payload_cnt_bits += ixheaace_write_bits(pstr_bs_handle, 0, 1);
  } else {
    if (pstr_sbr_env_info->harmonic_sbr) {
      // USAC Harmonic SBR data
      payload_cnt_bits +=
          ixheaace_write_bits(pstr_bs_handle, pstr_sbr_env_info->sbr_patching_mode, 1);
      if (0 == pstr_sbr_env_info->sbr_patching_mode) {
        payload_cnt_bits +=
            ixheaace_write_bits(pstr_bs_handle, pstr_sbr_env_info->sbr_oversampling_flag, 1);
        payload_cnt_bits +=
            ixheaace_write_bits(pstr_bs_handle, pstr_sbr_env_info->sbr_pitchin_bins_flag, 1);
        if (0 != pstr_sbr_env_info->sbr_pitchin_bins_flag) {
          payload_cnt_bits +=
              ixheaace_write_bits(pstr_bs_handle, pstr_sbr_env_info->sbr_pitchin_bins, 7);
        }
      }
    }
  }
  payload_cnt_bits += ixheaace_encode_sbr_grid(pstr_sbr_env_info, pstr_bs_handle, sbr_codec);
  if (sbr_codec == USAC_SBR) {
    payload_cnt_bits += ixheaace_encode_sbr_dtdf(pstr_sbr_env_info, pstr_bs_handle, sbr_codec,
                                                 pstr_sbr_env_info->usac_indep_flag,
                                                 pstr_sbr_env_info->sbr_pvc_mode);
  } else {
    payload_cnt_bits +=
        ixheaace_encode_sbr_dtdf(pstr_sbr_env_info, pstr_bs_handle, sbr_codec, 0, 0);
  }

  {
    WORD32 i;
    for (i = 0; i < pstr_sbr_env_info->noise_band_count; i++) {
      payload_cnt_bits += ixheaace_write_bits(
          pstr_bs_handle, pstr_sbr_env_info->sbr_invf_mode_vec[i], SI_SBR_INVF_MODE_BITS);
    }
  }
  if (sbr_codec != USAC_SBR) {
    WORD32 env_data_len;
    err_code =
        ixheaace_write_env_data(pstr_sbr_env_info, pstr_bs_handle, 0, sbr_codec, &env_data_len);
    if (err_code) {
      *ptr_num_bits = payload_cnt_bits;
      return err_code;
    }
    payload_cnt_bits += env_data_len;
  } else {
    if (0 == pstr_sbr_env_info->sbr_pvc_mode) {
      WORD32 env_data_len;
      err_code =
          ixheaace_write_env_data(pstr_sbr_env_info, pstr_bs_handle, 0, sbr_codec, &env_data_len);
      if (err_code) {
        *ptr_num_bits = payload_cnt_bits;
        return err_code;
      }
      payload_cnt_bits += env_data_len;
    } else {
      // PVC envelope goes here
      payload_cnt_bits += iusace_encode_pvc_envelope(pstr_bs_handle, &pstr_sbr_env_info->pvc_info,
                                                     pstr_sbr_env_info->usac_indep_flag);
    }
  }
  payload_cnt_bits += ixheaace_write_noise_lvl_data(pstr_sbr_env_info, pstr_bs_handle, 0);

  if (USAC_SBR == sbr_codec) {
    payload_cnt_bits += ixheaace_write_synthetic_coding_data(
        pstr_sbr_env_info, pstr_bs_handle, sbr_codec, pstr_sbr_env_info->sbr_pvc_mode);
  } else {
    payload_cnt_bits +=
        ixheaace_write_synthetic_coding_data(pstr_sbr_env_info, pstr_bs_handle, sbr_codec, 0);
  }

  *ptr_num_bits = payload_cnt_bits;
  return err_code;
}

static IA_ERRORCODE ixheaace_encode_sbr_channel_pair_element(
    ixheaace_pstr_sbr_env_data pstr_sbr_env_data_left,
    ixheaace_pstr_sbr_env_data pstr_sbr_env_data_right, ixheaace_bit_buf_handle pstr_bs_handle,
    WORD32 coupling, ixheaace_sbr_codec_type sbr_codec, WORD32 *ptr_num_bits) {
  IA_ERRORCODE err_code = IA_NO_ERROR;
  WORD32 payload_cnt_bits = 0;
  WORD32 env_data_len;
  WORD32 i = 0;

  if (USAC_SBR != sbr_codec) {
    payload_cnt_bits += ixheaace_write_bits(pstr_bs_handle, 0, 1); /* no reserved bits */
  }

  payload_cnt_bits += ixheaace_write_bits(pstr_bs_handle, coupling, SI_SBR_COUPLING_BITS);

  if (coupling) {
    if (sbr_codec == USAC_SBR && pstr_sbr_env_data_left->harmonic_sbr) {
      // USAC Harmonic SBR data
      payload_cnt_bits +=
          ixheaace_write_bits(pstr_bs_handle, pstr_sbr_env_data_left->sbr_patching_mode, 1);
      if (0 == pstr_sbr_env_data_left->sbr_patching_mode) {
        payload_cnt_bits +=
            ixheaace_write_bits(pstr_bs_handle, pstr_sbr_env_data_left->sbr_oversampling_flag, 1);
        payload_cnt_bits +=
            ixheaace_write_bits(pstr_bs_handle, pstr_sbr_env_data_left->sbr_pitchin_bins_flag, 1);
        if (0 != pstr_sbr_env_data_left->sbr_pitchin_bins_flag) {
          payload_cnt_bits +=
              ixheaace_write_bits(pstr_bs_handle, pstr_sbr_env_data_left->sbr_pitchin_bins, 7);
        }
      }
    }
    payload_cnt_bits +=
        ixheaace_encode_sbr_grid(pstr_sbr_env_data_left, pstr_bs_handle, sbr_codec);

    payload_cnt_bits +=
        ixheaace_encode_sbr_dtdf(pstr_sbr_env_data_left, pstr_bs_handle, sbr_codec,
                                 pstr_sbr_env_data_left->usac_indep_flag, 0);

    payload_cnt_bits +=
        ixheaace_encode_sbr_dtdf(pstr_sbr_env_data_right, pstr_bs_handle, sbr_codec,
                                 pstr_sbr_env_data_left->usac_indep_flag, 0);

    for (i = 0; i < pstr_sbr_env_data_left->noise_band_count; i++) {
      payload_cnt_bits += ixheaace_write_bits(
          pstr_bs_handle, pstr_sbr_env_data_left->sbr_invf_mode_vec[i], SI_SBR_INVF_MODE_BITS);
    }

    err_code = ixheaace_write_env_data(pstr_sbr_env_data_left, pstr_bs_handle, 1, sbr_codec,
                                       &env_data_len);
    if (err_code) {
      *ptr_num_bits = payload_cnt_bits;
      return err_code;
    }

    payload_cnt_bits += env_data_len;

    payload_cnt_bits += ixheaace_write_noise_lvl_data(pstr_sbr_env_data_left, pstr_bs_handle, 1);

    err_code = ixheaace_write_env_data(pstr_sbr_env_data_right, pstr_bs_handle, 1, sbr_codec,
                                       &env_data_len);
    if (err_code) {
      *ptr_num_bits = payload_cnt_bits;
      return err_code;
    }
    payload_cnt_bits += env_data_len;

    payload_cnt_bits += ixheaace_write_noise_lvl_data(pstr_sbr_env_data_right, pstr_bs_handle, 1);

    payload_cnt_bits += ixheaace_write_synthetic_coding_data(pstr_sbr_env_data_left,
                                                             pstr_bs_handle, sbr_codec, 0);

    payload_cnt_bits += ixheaace_write_synthetic_coding_data(pstr_sbr_env_data_right,
                                                             pstr_bs_handle, sbr_codec, 0);
  } else {
    if (sbr_codec == USAC_SBR && pstr_sbr_env_data_left->harmonic_sbr) {
      // USAC Harmonic SBR data
      payload_cnt_bits +=
          ixheaace_write_bits(pstr_bs_handle, pstr_sbr_env_data_left->sbr_patching_mode, 1);
      if (0 == pstr_sbr_env_data_left->sbr_patching_mode) {
        payload_cnt_bits +=
            ixheaace_write_bits(pstr_bs_handle, pstr_sbr_env_data_left->sbr_oversampling_flag, 1);
        payload_cnt_bits +=
            ixheaace_write_bits(pstr_bs_handle, pstr_sbr_env_data_left->sbr_pitchin_bins_flag, 1);
        if (0 != pstr_sbr_env_data_left->sbr_pitchin_bins_flag) {
          payload_cnt_bits +=
              ixheaace_write_bits(pstr_bs_handle, pstr_sbr_env_data_left->sbr_pitchin_bins, 7);
        }
      }

      payload_cnt_bits +=
          ixheaace_write_bits(pstr_bs_handle, pstr_sbr_env_data_right->sbr_patching_mode, 1);
      if (0 == pstr_sbr_env_data_right->sbr_patching_mode) {
        payload_cnt_bits += ixheaace_write_bits(
            pstr_bs_handle, pstr_sbr_env_data_right->sbr_oversampling_flag, 1);
        payload_cnt_bits += ixheaace_write_bits(
            pstr_bs_handle, pstr_sbr_env_data_right->sbr_pitchin_bins_flag, 1);
        if (0 != pstr_sbr_env_data_right->sbr_pitchin_bins_flag) {
          payload_cnt_bits +=
              ixheaace_write_bits(pstr_bs_handle, pstr_sbr_env_data_right->sbr_pitchin_bins, 7);
        }
      }
    }
    payload_cnt_bits +=
        ixheaace_encode_sbr_grid(pstr_sbr_env_data_left, pstr_bs_handle, sbr_codec);

    payload_cnt_bits +=
        ixheaace_encode_sbr_grid(pstr_sbr_env_data_right, pstr_bs_handle, sbr_codec);

    payload_cnt_bits +=
        ixheaace_encode_sbr_dtdf(pstr_sbr_env_data_left, pstr_bs_handle, sbr_codec,
                                 pstr_sbr_env_data_left->usac_indep_flag, 0);

    payload_cnt_bits +=
        ixheaace_encode_sbr_dtdf(pstr_sbr_env_data_right, pstr_bs_handle, sbr_codec,
                                 pstr_sbr_env_data_left->usac_indep_flag, 0);

    for (i = 0; i < pstr_sbr_env_data_left->noise_band_count; i++) {
      payload_cnt_bits += ixheaace_write_bits(
          pstr_bs_handle, pstr_sbr_env_data_left->sbr_invf_mode_vec[i], SI_SBR_INVF_MODE_BITS);
    }

    for (i = 0; i < pstr_sbr_env_data_right->noise_band_count; i++) {
      payload_cnt_bits += ixheaace_write_bits(
          pstr_bs_handle, pstr_sbr_env_data_right->sbr_invf_mode_vec[i], SI_SBR_INVF_MODE_BITS);
    }

    err_code = ixheaace_write_env_data(pstr_sbr_env_data_left, pstr_bs_handle, 0, sbr_codec,
                                       &env_data_len);
    if (err_code) {
      *ptr_num_bits = payload_cnt_bits;
      return err_code;
    }
    payload_cnt_bits += env_data_len;

    err_code = ixheaace_write_env_data(pstr_sbr_env_data_right, pstr_bs_handle, 0, sbr_codec,
                                       &env_data_len);
    if (err_code) {
      *ptr_num_bits = payload_cnt_bits;
      return err_code;
    }
    payload_cnt_bits += env_data_len;

    payload_cnt_bits += ixheaace_write_noise_lvl_data(pstr_sbr_env_data_left, pstr_bs_handle, 0);

    payload_cnt_bits += ixheaace_write_noise_lvl_data(pstr_sbr_env_data_right, pstr_bs_handle, 0);

    payload_cnt_bits += ixheaace_write_synthetic_coding_data(pstr_sbr_env_data_left,
                                                             pstr_bs_handle, sbr_codec, 0);

    payload_cnt_bits += ixheaace_write_synthetic_coding_data(pstr_sbr_env_data_right,
                                                             pstr_bs_handle, sbr_codec, 0);
  }

  *ptr_num_bits = payload_cnt_bits;
  return err_code;
}

static WORD32 iexhaace_esbr_write_bs(ixheaace_str_esbr_bs_data *pstr_esbr_bs_data,
                                     ixheaace_bit_buf_handle pstr_bs_handle) {
  WORD32 num_bits = 0;
  num_bits +=
      ixheaace_write_bits(pstr_bs_handle, EXTENSION_ID_ESBR_CODING, SI_SBR_EXTENSION_ID_BITS);
  num_bits += ixheaace_write_bits(pstr_bs_handle, pstr_esbr_bs_data->sbr_preprocessing, 1);
  if (1 == pstr_esbr_bs_data->sbr_num_chan) {
    num_bits += ixheaace_write_bits(pstr_bs_handle, pstr_esbr_bs_data->sbr_patching_mode[0], 1);
    if (pstr_esbr_bs_data->sbr_patching_mode[0] == 0) {
      num_bits +=
          ixheaace_write_bits(pstr_bs_handle, pstr_esbr_bs_data->sbr_oversampling_flag[0], 1);
      num_bits += ixheaace_write_bits(pstr_bs_handle, pstr_esbr_bs_data->sbr_pitchin_flags[0], 1);
      if (pstr_esbr_bs_data->sbr_pitchin_flags[0] == 1) {
        num_bits +=
            ixheaace_write_bits(pstr_bs_handle, pstr_esbr_bs_data->sbr_pitchin_bins[0], 7);
      }
    }
  } else if (2 == pstr_esbr_bs_data->sbr_num_chan) {
    if (pstr_esbr_bs_data->sbr_coupling) {
      num_bits += ixheaace_write_bits(pstr_bs_handle, pstr_esbr_bs_data->sbr_patching_mode[0], 1);
      if (pstr_esbr_bs_data->sbr_patching_mode[0] == 0) {
        num_bits +=
            ixheaace_write_bits(pstr_bs_handle, pstr_esbr_bs_data->sbr_oversampling_flag[0], 1);
        num_bits +=
            ixheaace_write_bits(pstr_bs_handle, pstr_esbr_bs_data->sbr_pitchin_flags[0], 1);
        if (pstr_esbr_bs_data->sbr_pitchin_flags[0] == 1) {
          num_bits +=
              ixheaace_write_bits(pstr_bs_handle, pstr_esbr_bs_data->sbr_pitchin_bins[0], 7);
        }
      }
    } else {
      num_bits += ixheaace_write_bits(pstr_bs_handle, pstr_esbr_bs_data->sbr_patching_mode[0], 1);
      if (pstr_esbr_bs_data->sbr_patching_mode[0] == 0) {
        num_bits +=
            ixheaace_write_bits(pstr_bs_handle, pstr_esbr_bs_data->sbr_oversampling_flag[0], 1);
        num_bits +=
            ixheaace_write_bits(pstr_bs_handle, pstr_esbr_bs_data->sbr_pitchin_flags[0], 1);
        if (pstr_esbr_bs_data->sbr_pitchin_flags[0] == 1) {
          num_bits +=
              ixheaace_write_bits(pstr_bs_handle, pstr_esbr_bs_data->sbr_pitchin_bins[0], 7);
        } else {
          pstr_esbr_bs_data->sbr_patching_mode[0] = pstr_esbr_bs_data->sbr_patching_mode[0];
        }
      }
      num_bits += ixheaace_write_bits(pstr_bs_handle, pstr_esbr_bs_data->sbr_patching_mode[1], 1);
      if (pstr_esbr_bs_data->sbr_patching_mode[1] == 0) {
        num_bits +=
            ixheaace_write_bits(pstr_bs_handle, pstr_esbr_bs_data->sbr_oversampling_flag[1], 1);
        num_bits +=
            ixheaace_write_bits(pstr_bs_handle, pstr_esbr_bs_data->sbr_pitchin_flags[1], 1);
        if (pstr_esbr_bs_data->sbr_pitchin_flags[1] == 1) {
          num_bits +=
              ixheaace_write_bits(pstr_bs_handle, pstr_esbr_bs_data->sbr_pitchin_bins[0], 7);
        }
      }
    }
  }
  if (num_bits < 8) {
    num_bits += ixheaace_write_bits(pstr_bs_handle, 0, (UWORD8)(8 - num_bits));
  }

  return num_bits;
}

static WORD32 ixheaace_get_sbr_extended_data_size(struct ixheaace_ps_enc *pstr_ps_handle,
                                                  WORD32 is_hdr_active,
                                                  ixheaace_str_sbr_tabs *pstr_sbr_tab,
                                                  WORD32 is_esbr,
                                                  ixheaace_str_esbr_bs_data *pstr_esbr_data) {
  WORD32 ext_data_bits = 0;

  if (pstr_ps_handle) {
    ext_data_bits +=
        ixheaace_enc_write_ps_data(pstr_ps_handle, is_hdr_active, pstr_sbr_tab->ptr_ps_tab);
  }
  if (is_esbr) {
    ext_data_bits += ixheaace_get_esbr_ext_data_size(pstr_esbr_data);
  }

  if (ext_data_bits != 0) {
    ext_data_bits += SI_SBR_EXTENSION_ID_BITS;
  }

  return (ext_data_bits + 7) >> 3;
}

static VOID ixheaace_encode_extended_data(struct ixheaace_ps_enc *pstr_ps_handle,
                                          WORD32 is_hdr_active,
                                          ixheaace_bit_buf_handle pstr_bs_prev,
                                          WORD32 *ptr_sbr_hdr_bits,
                                          ixheaace_bit_buf_handle pstr_bs_handle,
                                          WORD32 *ptr_payload_bits,
                                          ixheaace_str_sbr_tabs *pstr_sbr_tab, WORD32 is_esbr,
                                          ixheaace_str_esbr_bs_data *pstr_esbr_data) {
  WORD32 ext_data_size;
  WORD32 payload_bits_in = *ptr_payload_bits;
  WORD32 payload_cnt_bits = 0;

  ext_data_size = ixheaace_get_sbr_extended_data_size(pstr_ps_handle, is_hdr_active, pstr_sbr_tab,
                                                      is_esbr, pstr_esbr_data);

  if (ext_data_size != 0) {
    if (pstr_ps_handle && ixheaace_append_ps_bitstream(pstr_ps_handle, NULL_PTR, 0)) {
      ixheaace_bit_buf bitbuf_tmp;
      UWORD8 tmp[IXHEAACE_MAX_PAYLOAD_SIZE];
      WORD32 max_ext_size = (1 << SI_SBR_EXTENSION_SIZE_BITS) - 1;
      WORD32 num_bits;

      num_bits = ia_enhaacplus_enc_get_bits_available(&pstr_ps_handle->ps_bit_buf);
      num_bits += SI_SBR_EXTENSION_ID_BITS;
      if (is_esbr) {
        num_bits += ixheaace_get_esbr_ext_data_size(pstr_esbr_data);
        num_bits += SI_SBR_EXTENSION_ID_BITS;
      }
      ext_data_size = (num_bits + 7) >> 3;
      if (ia_enhaacplus_enc_get_bits_available(pstr_bs_prev) == 0) {
        pstr_ps_handle->hdr_bits_prev_frame = *ptr_sbr_hdr_bits;
        ia_enhaacplus_enc_copy_bitbuf(pstr_bs_handle, pstr_bs_prev);
      } else {
        WORD32 tmp_bits;
        ia_enhaacplus_enc_create_bitbuffer(&bitbuf_tmp, tmp, sizeof(tmp));
        tmp_bits = *ptr_sbr_hdr_bits;
        *ptr_sbr_hdr_bits = pstr_ps_handle->hdr_bits_prev_frame;
        pstr_ps_handle->hdr_bits_prev_frame = tmp_bits;
        ixheaace_copy_bitbuf_to_and_fro(pstr_bs_prev, pstr_bs_handle);
      }
      ixheaace_write_bits(pstr_bs_handle, 1, SI_SBR_EXTENDED_DATA_BITS);

      if (ext_data_size < max_ext_size) {
        ixheaace_write_bits(pstr_bs_handle, ext_data_size, SI_SBR_EXTENSION_SIZE_BITS);
      } else {
        ixheaace_write_bits(pstr_bs_handle, max_ext_size, SI_SBR_EXTENSION_SIZE_BITS);
        ixheaace_write_bits(pstr_bs_handle, ext_data_size - max_ext_size,
                            SI_SBR_EXTENSION_ESC_COUNT_BITS);
      }
      WORD32 start_bits = pstr_bs_handle->cnt_bits;
      *ptr_payload_bits =
          ixheaace_append_ps_bitstream(pstr_ps_handle, pstr_bs_handle, ptr_sbr_hdr_bits);

      if (is_esbr) {
        *ptr_payload_bits += iexhaace_esbr_write_bs(pstr_esbr_data, pstr_bs_handle);
      }

      WORD32 fill_bits = (ext_data_size << 3) - (pstr_bs_handle->cnt_bits - start_bits);
      ixheaace_write_bits(pstr_bs_handle, 0, (UWORD8)fill_bits);
      *ptr_payload_bits = *ptr_payload_bits + fill_bits;
    } else {
      if (is_esbr) {
        WORD32 max_ext_size = (1 << SI_SBR_EXTENSION_SIZE_BITS) - 1;
        WORD32 num_bits;
        num_bits = ixheaace_get_esbr_ext_data_size(pstr_esbr_data);
        ext_data_size = (num_bits + SI_SBR_EXTENSION_ID_BITS + 7) >> 3;
        ixheaace_write_bits(pstr_bs_handle, 1, SI_SBR_EXTENDED_DATA_BITS);
        if (ext_data_size < max_ext_size) {
          ixheaace_write_bits(pstr_bs_handle, ext_data_size, SI_SBR_EXTENSION_SIZE_BITS);
        } else {
          ixheaace_write_bits(pstr_bs_handle, max_ext_size, SI_SBR_EXTENSION_SIZE_BITS);
          ixheaace_write_bits(pstr_bs_handle, ext_data_size - max_ext_size,
                              SI_SBR_EXTENSION_ESC_COUNT_BITS);
        }
        WORD32 start_bits = pstr_bs_handle->cnt_bits;
        *ptr_payload_bits += iexhaace_esbr_write_bs(pstr_esbr_data, pstr_bs_handle);
        UWORD8 fill_bits =
            (UWORD8)((ext_data_size << 3) - (pstr_bs_handle->cnt_bits - start_bits));
        ixheaace_write_bits(pstr_bs_handle, 0, fill_bits);
        *ptr_payload_bits = *ptr_payload_bits + fill_bits;
      } else {
        WORD32 max_ext_size = (1 << SI_SBR_EXTENSION_SIZE_BITS) - 1;
        payload_cnt_bits += ixheaace_write_bits(pstr_bs_handle, 1, SI_SBR_EXTENDED_DATA_BITS);

        if (ext_data_size < max_ext_size) {
          payload_cnt_bits +=
              ixheaace_write_bits(pstr_bs_handle, ext_data_size, SI_SBR_EXTENSION_SIZE_BITS);
        } else {
          payload_cnt_bits +=
              ixheaace_write_bits(pstr_bs_handle, max_ext_size, SI_SBR_EXTENSION_SIZE_BITS);

          payload_cnt_bits += ixheaace_write_bits(pstr_bs_handle, ext_data_size - max_ext_size,
                                                  SI_SBR_EXTENSION_ESC_COUNT_BITS);
        }

        *ptr_payload_bits = payload_cnt_bits + payload_bits_in;
      }
    }
  } else {
    payload_cnt_bits += ixheaace_write_bits(pstr_bs_handle, 0, SI_SBR_EXTENDED_DATA_BITS);

    *ptr_payload_bits = payload_cnt_bits + payload_bits_in;
  }
}

static IA_ERRORCODE ixheaace_encode_sbr_data(
    ixheaace_pstr_sbr_env_data pstr_sbr_env_data_left,
    ixheaace_pstr_sbr_env_data pstr_sbr_env_data_right, ixheaace_pstr_common_data pstr_cmon_data,
    ixheaace_sbr_element_type sbr_ele_type, struct ixheaace_ps_enc *pstr_ps_handle,
    WORD32 is_hdr_active, WORD32 coupling, ixheaace_str_sbr_tabs *pstr_sbr_tab,
    ixheaace_sbr_codec_type sbr_codec, WORD32 is_esbr, ixheaace_str_esbr_bs_data *pstr_esbr_data,
    WORD32 *ptr_num_bits) {
  WORD32 payload_cnt_bits = 0;
  IA_ERRORCODE err_code = IA_NO_ERROR;

  switch (sbr_ele_type) {
    case IXHEAACE_SBR_ID_SCE:

      err_code = ixheaace_encode_sbr_single_channel_element(
          pstr_sbr_env_data_left, &pstr_cmon_data->str_sbr_bit_buf, sbr_codec, &payload_cnt_bits);
      if (err_code) {
        return err_code;
      }
      if (USAC_SBR != sbr_codec) {
        ixheaace_encode_extended_data(
            pstr_ps_handle, is_hdr_active, &pstr_cmon_data->str_sbr_bit_buf_prev,
            &pstr_cmon_data->sbr_hdr_bits, &pstr_cmon_data->str_sbr_bit_buf, &payload_cnt_bits,
            pstr_sbr_tab, is_esbr, pstr_esbr_data);
      }
      break;
    case IXHEAACE_SBR_ID_CPE:

      err_code = ixheaace_encode_sbr_channel_pair_element(
          pstr_sbr_env_data_left, pstr_sbr_env_data_right, &pstr_cmon_data->str_sbr_bit_buf,
          coupling, sbr_codec, &payload_cnt_bits);
      if (err_code) {
        return err_code;
      }
      if (USAC_SBR != sbr_codec) {
        ixheaace_encode_extended_data(NULL_PTR, 0, NULL_PTR, 0, &pstr_cmon_data->str_sbr_bit_buf,
                                      &payload_cnt_bits, pstr_sbr_tab, is_esbr, pstr_esbr_data);
      }
      break;
  }

  pstr_cmon_data->sbr_data_bits = payload_cnt_bits;
  *ptr_num_bits = payload_cnt_bits;

  pstr_cmon_data->prev_bit_buf_read_offset =
      (WORD32)(pstr_cmon_data->str_sbr_bit_buf_prev.ptr_read_next -
               pstr_cmon_data->str_sbr_bit_buf_prev.ptr_bit_buf_base);
  pstr_cmon_data->prev_bit_buf_write_offset =
      (WORD32)(pstr_cmon_data->str_sbr_bit_buf_prev.ptr_write_next -
               pstr_cmon_data->str_sbr_bit_buf_prev.ptr_bit_buf_base);

  return err_code;
}

static WORD32 ixheaace_encode_sbr_header_data(ixheaace_pstr_sbr_hdr_data pstr_sbr_hdr,
                                              ixheaace_bit_buf_handle pstr_bs_handle)

{
  WORD32 payload_cnt_bits = 0;

  if (pstr_sbr_hdr != NULL_PTR) {
    payload_cnt_bits +=
        ixheaace_write_bits(pstr_bs_handle, pstr_sbr_hdr->sbr_amp_res, SI_SBR_AMP_RES_BITS);

    payload_cnt_bits +=
        ixheaace_write_bits(pstr_bs_handle, pstr_sbr_hdr->sbr_start_freq, SI_SBR_START_FREQ_BITS);

    payload_cnt_bits +=
        ixheaace_write_bits(pstr_bs_handle, pstr_sbr_hdr->sbr_stop_freq, SI_SBR_STOP_FREQ_BITS);

    payload_cnt_bits +=
        ixheaace_write_bits(pstr_bs_handle, pstr_sbr_hdr->sbr_xover_band, SI_SBR_XOVER_BAND_BITS);

    payload_cnt_bits += ixheaace_write_bits(pstr_bs_handle, 0, SI_SBR_RESERVED_BITS);

    payload_cnt_bits += ixheaace_write_bits(pstr_bs_handle, pstr_sbr_hdr->header_extra_1,
                                            SI_SBR_HEADER_EXTRA_1_BITS);

    payload_cnt_bits += ixheaace_write_bits(pstr_bs_handle, pstr_sbr_hdr->header_extra_2,
                                            SI_SBR_HEADER_EXTRA_2_BITS);

    if (pstr_sbr_hdr->header_extra_1) {
      payload_cnt_bits +=
          ixheaace_write_bits(pstr_bs_handle, pstr_sbr_hdr->freq_scale, SI_SBR_FREQ_SCALE_BITS);

      payload_cnt_bits +=
          ixheaace_write_bits(pstr_bs_handle, pstr_sbr_hdr->alter_scale, SI_SBR_ALTER_SCALE_BITS);

      payload_cnt_bits += ixheaace_write_bits(pstr_bs_handle, pstr_sbr_hdr->sbr_noise_bands,
                                              SI_SBR_NOISE_BANDS_BITS);
    }

    if (pstr_sbr_hdr->header_extra_2) {
      payload_cnt_bits += ixheaace_write_bits(pstr_bs_handle, pstr_sbr_hdr->sbr_limiter_bands,
                                              SI_SBR_LIMITER_BANDS_BITS);
      payload_cnt_bits += ixheaace_write_bits(pstr_bs_handle, pstr_sbr_hdr->sbr_limiter_gains,
                                              SI_SBR_LIMITER_GAINS_BITS);

      payload_cnt_bits += ixheaace_write_bits(pstr_bs_handle, pstr_sbr_hdr->sbr_interpol_freq,
                                              SI_SBR_INTERPOL_FREQ_BITS);

      payload_cnt_bits += ixheaace_write_bits(pstr_bs_handle, pstr_sbr_hdr->sbr_smoothing_length,
                                              SI_SBR_SMOOTHING_LENGTH_BITS);
    }
  }

  return payload_cnt_bits;
}

static WORD32 ia_enhaacplus_enc_encode_sbr_header(ixheaace_pstr_sbr_hdr_data pstr_sbr_hdr,
                                                  ixheaace_pstr_sbr_bitstream_data pstr_sbr_bs,
                                                  ixheaace_pstr_common_data pstr_cmon_data) {
  WORD32 payload_cnt_bits = 0;

  if (pstr_sbr_bs->crc_active) {
    pstr_cmon_data->sbr_crc_len = 1;
  } else {
    pstr_cmon_data->sbr_crc_len = 0;
  }

  if (pstr_sbr_bs->header_active) {
    payload_cnt_bits += ixheaace_write_bits(&pstr_cmon_data->str_sbr_bit_buf, 1, 1);

    payload_cnt_bits +=
        ixheaace_encode_sbr_header_data(pstr_sbr_hdr, &pstr_cmon_data->str_sbr_bit_buf);
  } else {
    payload_cnt_bits += ixheaace_write_bits(&pstr_cmon_data->str_sbr_bit_buf, 0, 1);
  }

  pstr_cmon_data->sbr_hdr_bits = payload_cnt_bits;

  return payload_cnt_bits;
}
static WORD32 iusace_encode_sbr_header_data(ixheaace_pstr_sbr_hdr_data pstr_sbr_hdr,
                                            ixheaace_bit_buf_handle pstr_bs_handle) {
  WORD32 payload_cnt_bits = 0;

  if (pstr_sbr_hdr != NULL_PTR) {
    payload_cnt_bits +=
        ixheaace_write_bits(pstr_bs_handle, pstr_sbr_hdr->sbr_start_freq, SI_SBR_START_FREQ_BITS);

    payload_cnt_bits +=
        ixheaace_write_bits(pstr_bs_handle, pstr_sbr_hdr->sbr_stop_freq, SI_SBR_STOP_FREQ_BITS);

    payload_cnt_bits += ixheaace_write_bits(pstr_bs_handle, pstr_sbr_hdr->header_extra_1,
                                            SI_SBR_HEADER_EXTRA_1_BITS);

    payload_cnt_bits += ixheaace_write_bits(pstr_bs_handle, pstr_sbr_hdr->header_extra_2,
                                            SI_SBR_HEADER_EXTRA_2_BITS);

    if (pstr_sbr_hdr->header_extra_1) {
      payload_cnt_bits +=
          ixheaace_write_bits(pstr_bs_handle, pstr_sbr_hdr->freq_scale, SI_SBR_FREQ_SCALE_BITS);

      payload_cnt_bits +=
          ixheaace_write_bits(pstr_bs_handle, pstr_sbr_hdr->alter_scale, SI_SBR_ALTER_SCALE_BITS);

      payload_cnt_bits += ixheaace_write_bits(pstr_bs_handle, pstr_sbr_hdr->sbr_noise_bands,
                                              SI_SBR_NOISE_BANDS_BITS);
    }

    if (pstr_sbr_hdr->header_extra_2) {
      payload_cnt_bits += ixheaace_write_bits(pstr_bs_handle, pstr_sbr_hdr->sbr_limiter_bands,
                                              SI_SBR_LIMITER_BANDS_BITS);

      payload_cnt_bits += ixheaace_write_bits(pstr_bs_handle, pstr_sbr_hdr->sbr_limiter_gains,
                                              SI_SBR_LIMITER_GAINS_BITS);

      payload_cnt_bits += ixheaace_write_bits(pstr_bs_handle, pstr_sbr_hdr->sbr_interpol_freq,
                                              SI_SBR_INTERPOL_FREQ_BITS);

      payload_cnt_bits += ixheaace_write_bits(pstr_bs_handle, pstr_sbr_hdr->sbr_smoothing_length,
                                              SI_SBR_SMOOTHING_LENGTH_BITS);
    }
  }

  return payload_cnt_bits;
}

static WORD32 ia_usac_enc_encode_sbr_header(ixheaace_pstr_sbr_hdr_data pstr_sbr_hdr,
                                            ixheaace_pstr_sbr_bitstream_data pstr_sbr_bs,
                                            ixheaace_pstr_common_data pstr_cmon_data) {
  WORD32 payload_cnt_bits = 0;
  WORD32 sbr_info_flag = 0;
  WORD32 sbr_hdr_flag = 0;
  if (pstr_sbr_bs->usac_indep_flag) {
    sbr_hdr_flag = 1;
    sbr_info_flag = 1;
  } else {
    if (pstr_sbr_bs->header_active) {
      sbr_info_flag = 1;
      payload_cnt_bits += ixheaace_write_bits(&pstr_cmon_data->str_sbr_bit_buf, 1, 1);
      sbr_hdr_flag = 1;
      payload_cnt_bits += ixheaace_write_bits(&pstr_cmon_data->str_sbr_bit_buf, 1, 1);
    } else {
      payload_cnt_bits += ixheaace_write_bits(&pstr_cmon_data->str_sbr_bit_buf, 0, 1);
    }
  }

  if (1 == sbr_info_flag) {
    payload_cnt_bits +=
        ixheaace_write_bits(&pstr_cmon_data->str_sbr_bit_buf, pstr_sbr_hdr->sbr_amp_res, 1);

    payload_cnt_bits +=
        ixheaace_write_bits(&pstr_cmon_data->str_sbr_bit_buf, pstr_sbr_hdr->sbr_xover_band, 4);

    payload_cnt_bits +=
        ixheaace_write_bits(&pstr_cmon_data->str_sbr_bit_buf, pstr_sbr_hdr->sbr_pre_proc, 1);
    if (pstr_sbr_hdr->sbr_pvc_active) {
      payload_cnt_bits +=
          ixheaace_write_bits(&pstr_cmon_data->str_sbr_bit_buf, pstr_sbr_hdr->sbr_pvc_mode, 2);
    }
  }

  if (1 == sbr_hdr_flag) {
    WORD32 sbr_def_hdr = 0;
    // SBR default header
    payload_cnt_bits += ixheaace_write_bits(&pstr_cmon_data->str_sbr_bit_buf, sbr_def_hdr, 1);
    if (0 == sbr_def_hdr) {
      payload_cnt_bits +=
          iusace_encode_sbr_header_data(pstr_sbr_hdr, &pstr_cmon_data->str_sbr_bit_buf);
    }
  }
  pstr_cmon_data->sbr_hdr_bits = payload_cnt_bits;

  return payload_cnt_bits;
}
IA_ERRORCODE
ixheaace_write_env_single_channel_element(
    ixheaace_pstr_sbr_hdr_data pstr_sbr_hdr, ixheaace_pstr_sbr_bitstream_data pstr_sbr_bs,
    ixheaace_pstr_sbr_env_data pstr_sbr_env_info, struct ixheaace_ps_enc *pstr_ps_handle,
    ixheaace_pstr_common_data pstr_cmon_data, ixheaace_str_sbr_tabs *pstr_sbr_tab,
    ixheaace_sbr_codec_type sbr_codec, WORD32 is_esbr, ixheaace_str_esbr_bs_data *pstr_esbr_data,
    WORD32 *ptr_num_bits) {
  WORD32 payload_cnt_bits = 0;
  WORD32 num_sbr_data_bits = 0;
  IA_ERRORCODE err_code = IA_NO_ERROR;

  pstr_cmon_data->sbr_hdr_bits = 0;
  pstr_cmon_data->sbr_data_bits = 0;
  pstr_cmon_data->sbr_crc_len = 0;

  if (pstr_sbr_env_info != NULL_PTR) {
    if (USAC_SBR == sbr_codec) {
      payload_cnt_bits +=
          ia_usac_enc_encode_sbr_header(pstr_sbr_hdr, pstr_sbr_bs, pstr_cmon_data);
    } else {
      /* write header */
      payload_cnt_bits +=
          ia_enhaacplus_enc_encode_sbr_header(pstr_sbr_hdr, pstr_sbr_bs, pstr_cmon_data);
    }
    /* write data */
    err_code =
        ixheaace_encode_sbr_data(pstr_sbr_env_info, NULL_PTR, pstr_cmon_data, IXHEAACE_SBR_ID_SCE,
                                 pstr_ps_handle, pstr_sbr_bs->header_active, 0, pstr_sbr_tab,
                                 sbr_codec, is_esbr, pstr_esbr_data, &num_sbr_data_bits);
    if (err_code) {
      return err_code;
    }
    payload_cnt_bits += num_sbr_data_bits;
  }

  *ptr_num_bits = payload_cnt_bits;
  return err_code;
}

IA_ERRORCODE
ixheaace_write_env_channel_pair_element(
    ixheaace_pstr_sbr_hdr_data pstr_sbr_hdr, ixheaace_pstr_sbr_bitstream_data pstr_sbr_bs,
    ixheaace_pstr_sbr_env_data pstr_sbr_env_data_left,
    ixheaace_pstr_sbr_env_data pstr_sbr_env_data_right, ixheaace_pstr_common_data pstr_cmon_data,
    ixheaace_str_sbr_tabs *pstr_sbr_tab, ixheaace_sbr_codec_type sbr_codec, WORD32 is_esbr,
    ixheaace_str_esbr_bs_data *pstr_esbr_data, WORD32 *ptr_num_bits)

{
  WORD32 payload_cnt_bits = 0;
  WORD32 num_sbr_data_bits = 0;
  IA_ERRORCODE err_code = IA_NO_ERROR;

  pstr_cmon_data->sbr_hdr_bits = 0;
  pstr_cmon_data->sbr_data_bits = 0;
  pstr_cmon_data->sbr_crc_len = 0;

  /* write pure SBR data */
  if ((pstr_sbr_env_data_left != NULL_PTR) && (pstr_sbr_env_data_right != NULL_PTR)) {
    if (USAC_SBR == sbr_codec) {
      payload_cnt_bits +=
          ia_usac_enc_encode_sbr_header(pstr_sbr_hdr, pstr_sbr_bs, pstr_cmon_data);
    } else {
      /* write header */
      payload_cnt_bits +=
          ia_enhaacplus_enc_encode_sbr_header(pstr_sbr_hdr, pstr_sbr_bs, pstr_cmon_data);
    }

    /* write data */
    err_code = ixheaace_encode_sbr_data(pstr_sbr_env_data_left, pstr_sbr_env_data_right,
                                        pstr_cmon_data, IXHEAACE_SBR_ID_CPE, NULL_PTR, 0,
                                        pstr_sbr_hdr->coupling, pstr_sbr_tab, sbr_codec, is_esbr,
                                        pstr_esbr_data, &num_sbr_data_bits);
    if (err_code) {
      return err_code;
    }
    payload_cnt_bits += num_sbr_data_bits;
  }

  *ptr_num_bits = payload_cnt_bits;
  return err_code;
}

IA_ERRORCODE
ixheaace_count_sbr_channel_pair_element(
    ixheaace_pstr_sbr_hdr_data pstr_sbr_hdr, ixheaace_pstr_sbr_bitstream_data pstr_sbr_bs,
    ixheaace_pstr_sbr_env_data pstr_sbr_env_data_left,
    ixheaace_pstr_sbr_env_data pstr_sbr_env_data_right, ixheaace_pstr_common_data pstr_cmon_data,
    ixheaace_str_sbr_tabs *pstr_sbr_tab, ixheaace_sbr_codec_type sbr_codec, WORD32 is_esbr,
    ixheaace_str_esbr_bs_data *pstr_esbr_data, WORD32 *ptr_num_bits)

{
  IA_ERRORCODE err_code = IA_NO_ERROR;
  ixheaace_bit_buf bit_buf_tmp = pstr_cmon_data->str_sbr_bit_buf;

  err_code = ixheaace_write_env_channel_pair_element(
      pstr_sbr_hdr, pstr_sbr_bs, pstr_sbr_env_data_left, pstr_sbr_env_data_right, pstr_cmon_data,
      pstr_sbr_tab, sbr_codec, is_esbr, pstr_esbr_data, ptr_num_bits);

  pstr_cmon_data->str_sbr_bit_buf = bit_buf_tmp;

  return err_code;
}

VOID ixheaace_map_low_res_energy_value(WORD32 curr_val, WORD32 *ptr_prev_data, WORD32 offset,
                                       WORD32 index, ixheaace_freq_res res) {
  if (res == FREQ_RES_LOW) {
    if (offset >= 0) {
      if (index < offset) {
        ptr_prev_data[index] = curr_val;
      } else {
        ptr_prev_data[2 * index - offset] = curr_val;
        ptr_prev_data[2 * index + 1 - offset] = curr_val;
      }
    } else {
      offset = -offset;

      if (index < offset) {
        ptr_prev_data[3 * index] = curr_val;
        ptr_prev_data[3 * index + 1] = curr_val;
        ptr_prev_data[3 * index + 2] = curr_val;
      } else {
        ptr_prev_data[2 * index + offset] = curr_val;
        ptr_prev_data[2 * index + 1 + offset] = curr_val;
      }
    }
  } else {
    ptr_prev_data[index] = curr_val;
  }
}

IA_ERRORCODE
ixheaace_compute_bits(WORD32 delta, WORD32 code_book_scf_lav_lvl,
                      WORD32 code_book_scf_lav_balance, const UWORD8 *ptr_huff_tbl_lvl,
                      const UWORD8 *ptr_huff_tbl_bal, WORD32 coupling, WORD32 ch,
                      WORD32 *ptr_delta_bits) {
  WORD32 index;
  *ptr_delta_bits = 0;

  if (coupling) {
    if (ch == 1) {
      index = (delta < 0) ? ixheaac_max32(delta, -code_book_scf_lav_balance)
                          : ixheaac_min32(delta, code_book_scf_lav_balance);

      if (index != delta) {
        return IA_EXHEAACE_EXE_FATAL_SBR_INVALID_BS;
      }

      *ptr_delta_bits = ptr_huff_tbl_bal[index + code_book_scf_lav_balance];
    } else {
      index = (delta < 0) ? ixheaac_max32(delta, -code_book_scf_lav_lvl)
                          : ixheaac_min32(delta, code_book_scf_lav_lvl);

      if (index != delta) {
        return IA_EXHEAACE_EXE_FATAL_SBR_INVALID_BS;
      }

      *ptr_delta_bits = ptr_huff_tbl_lvl[index + code_book_scf_lav_lvl];
    }
  } else {
    index = (delta < 0) ? ixheaac_max32(delta, -code_book_scf_lav_lvl)
                        : ixheaac_min32(delta, code_book_scf_lav_lvl);

    *ptr_delta_bits = ptr_huff_tbl_lvl[index + code_book_scf_lav_lvl];
  }

  return IA_NO_ERROR;
}
