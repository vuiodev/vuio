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

#include "ixheaac_type_def.h"
#include "ixheaac_constants.h"
#include "impd_drc_common_enc.h"
#include "impd_drc_uni_drc.h"
#include "impd_drc_tables.h"
#include "impd_drc_api.h"
#include "ixheaace_api.h"
#include "ixheaace_aac_constants.h"
#include "ixheaace_error_codes.h"
#include "ixheaac_error_standards.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops16.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaac_basic_ops.h"

#include "ixheaace_sbr_header.h"
#include "ixheaace_sbr_def.h"
#include "ixheaace_resampler.h"
#include "ixheaace_sbr_rom.h"
#include "ixheaace_common_rom.h"
#include "ixheaace_sbr_hbe.h"
#include "ixheaace_sbr_qmf_enc.h"
#include "ixheaace_sbr_tran_det.h"
#include "ixheaace_sbr_frame_info_gen.h"
#include "ixheaace_sbr_env_est.h"
#include "ixheaace_sbr_code_envelope.h"
#include "ixheaace_psy_const.h"
#include "ixheaace_tns.h"
#include "ixheaace_tns_params.h"
#include "ixheaace_rom.h"
#include "ixheaace_common_rom.h"
#include "ixheaace_bitbuffer.h"

#include "ixheaace_sbr_main.h"
#include "ixheaace_common_rom.h"
#include "ixheaace_sbr_missing_harmonics_det.h"
#include "ixheaace_sbr_inv_filtering_estimation.h"
#include "ixheaace_sbr_noise_floor_est.h"
#include "ixheaace_sbr_ton_corr.h"
#include "iusace_esbr_pvc.h"
#include "iusace_esbr_inter_tes.h"
#include "ixheaace_sbr.h"

#include "ixheaace_sbr_freq_scaling.h"

#include "ixheaace_bitbuffer.h"

#include "ixheaace_sbr_hybrid.h"
#include "ixheaace_sbr_ps_enc.h"
#include "ixheaace_sbr_cmondata.h"
#include "ixheaace_sbr_crc.h"
#include "ixheaace_sbr_enc_struct.h"

VOID ixheaace_set_usac_sbr_params(ixheaace_pstr_sbr_enc pstr_env_enc, WORD32 usac_indep_flag,
                                  WORD32 sbr_pre_proc, WORD32 sbr_pvc_active, WORD32 sbr_pvc_mode,
                                  WORD32 inter_tes_active, WORD32 sbr_harmonic,
                                  WORD32 sbr_patching_mode) {
  WORD32 ch;
  pstr_env_enc->str_sbr_bs.usac_indep_flag = usac_indep_flag;
  pstr_env_enc->str_sbr_hdr.sbr_pre_proc = sbr_pre_proc;
  pstr_env_enc->str_sbr_hdr.sbr_pvc_active = sbr_pvc_active;
  if (pstr_env_enc->str_sbr_cfg.num_ch == 2) {
    pstr_env_enc->str_sbr_hdr.sbr_pvc_mode = 0;
  } else {
    pstr_env_enc->str_sbr_hdr.sbr_pvc_mode = sbr_pvc_mode;
  }
  pstr_env_enc->str_sbr_hdr.sbr_inter_tes_active = inter_tes_active;
  pstr_env_enc->str_sbr_hdr.sbr_harmonic = sbr_harmonic;
  for (ch = 0; ch < pstr_env_enc->str_sbr_cfg.num_ch; ch++) {
    pstr_env_enc->pstr_env_channel[ch]->enc_env_data.sbr_inter_tes = inter_tes_active;
    pstr_env_enc->pstr_env_channel[ch]->enc_env_data.sbr_patching_mode = sbr_patching_mode;
  }
}
FLOAT32 *ixheaace_get_hbe_resample_buffer(ixheaace_pstr_sbr_enc pstr_env_enc) {
  return pstr_env_enc->ptr_hbe_resample_buf;
}

static FLAG ia_enhaacplus_enc_get_sbr_tuning_table_idx(
    UWORD32 bitrate, UWORD32 num_ch, UWORD32 sample_rate, ixheaace_str_qmf_tabs *pstr_qmf_tab,
    UWORD32 *ptr_closest_br, WORD32 *ptr_idx_sr, WORD32 *ptr_idx_ch, WORD32 *ptr_idx_entry,
    ixheaace_sbr_tuning_tables sbr_tune_table[10][2][10]) {
  WORD32 found = 0;
  WORD32 i_sr, br_closest_lower_idx_sr = -1, br_closest_upper_idx_sr = -1;
  WORD32 br_closest_lower_idx_ch = -1, br_closest_upper_idx_sch = -1;
  UWORD32 i_ch;
  WORD32 i_entry, br_closest_lower_idx_entry = -1, br_closest_upper_idx_entry = -1;
  UWORD32 br_closest_upper = 0, br_closest_lower = IXHEAACE_DISTANCE_CEIL_VALUE;
  const UWORD32 *ptr_sample_rate_supported = pstr_qmf_tab->supported_sample_rate;
  WORD32 check_size = sizeof(sbr_tune_table[i_sr][i_ch]) / sizeof(sbr_tune_table[i_sr][i_ch][0]);
  for (i_sr = 0; i_sr < 9; i_sr++) {
    for (i_ch = 0; i_ch < 2; i_ch++) {
      found = 1;
      if ((num_ch - 1) == (i_ch) && (sample_rate == ptr_sample_rate_supported[i_sr])) {
        for (i_entry = 0; i_entry < check_size - 1; i_entry++) {
          if ((bitrate >= sbr_tune_table[i_sr][i_ch][i_entry].bitrate_from) &&
              (bitrate < sbr_tune_table[i_sr][i_ch][i_entry].bitrate_to)) {
            *ptr_idx_sr = i_sr;
            *ptr_idx_ch = i_ch;
            *ptr_idx_entry = i_entry;
            return IXHEAACE_TABLE_IDX_FOUND;
          } else {
            if ((sbr_tune_table[i_sr][i_ch][i_entry].bitrate_from > bitrate) &&
                (sbr_tune_table[i_sr][i_ch][i_entry].bitrate_from < br_closest_lower)) {
              br_closest_lower = sbr_tune_table[i_sr][i_ch][i_entry].bitrate_from;
              br_closest_lower_idx_sr = i_sr;
              br_closest_lower_idx_ch = i_ch;
              br_closest_lower_idx_entry = i_entry;
            }
            if ((sbr_tune_table[i_sr][i_ch][i_entry].bitrate_to <= bitrate) &&
                (sbr_tune_table[i_sr][i_ch][i_entry].bitrate_to > br_closest_upper)) {
              br_closest_upper = sbr_tune_table[i_sr][i_ch][i_entry].bitrate_to - 1;
              br_closest_upper_idx_sr = i_sr;
              br_closest_upper_idx_sch = i_ch;
              br_closest_upper_idx_entry = i_entry;
            }
          }

          if (sbr_tune_table[i_sr][i_ch][i_entry + 1].bitrate_from == 0) {
            *ptr_idx_sr = i_sr;
            *ptr_idx_ch = i_ch;
            *ptr_idx_entry = i_entry;
            break;
          }
        }
      }
    }
  }

  if (br_closest_upper_idx_entry >= 0) {
    return IXHEAACE_TABLE_IDX_FOUND;
  }

  if (ptr_closest_br != NULL) {
    if (found) {
      WORD32 distance_upper = IXHEAACE_DISTANCE_CEIL_VALUE,
             distance_lower = IXHEAACE_DISTANCE_CEIL_VALUE;
      if (br_closest_lower_idx_entry >= 0) {
        distance_lower = sbr_tune_table[br_closest_lower_idx_sr][br_closest_lower_idx_ch]
                                       [br_closest_lower_idx_entry]
                                           .bitrate_from -
                         bitrate;
      }
      if (br_closest_upper_idx_entry >= 0) {
        distance_upper =
            bitrate - sbr_tune_table[br_closest_upper_idx_sr][br_closest_upper_idx_sch]
                                    [br_closest_lower_idx_entry]
                                        .bitrate_to;
      }

      *ptr_closest_br = (distance_upper < distance_lower) ? br_closest_upper : br_closest_lower;
    } else {
      *ptr_closest_br = 0;
    }
  }

  return IXHEAACE_TABLE_IDX_NOT_FOUND;
}

static IA_ERRORCODE ixheaace_create_env_channel(
    WORD32 ch, ixheaace_pstr_sbr_config_data pstr_sbr_cfg,
    ixheaace_pstr_sbr_hdr_data pstr_sbr_hdr, ixheaace_pstr_enc_channel pstr_env,
    ixheaace_pstr_sbr_cfg params, WORD32 *ptr_common_buf, WORD32 *ptr_common_buffer2,
    FLOAT32 *ptr_sbr_env_r_buf, FLOAT32 *ptr_sbr_env_i_buf, ixheaace_str_sbr_tabs *pstr_sbr_tab) {
  WORD32 e = 1;
  WORD32 tran_fc = 0;
  IA_ERRORCODE err_code = IA_NO_ERROR;

  WORD32 start_index;
  WORD32 noise_groups[2] = {3, 3};

  e = ixheaac_shl32(1, params->e);

  if (params->use_low_freq_res == 1) {
    pstr_env->enc_env_data.freq_res_fix = FREQ_RES_LOW;
  } else {
    pstr_env->enc_env_data.freq_res_fix = FREQ_RES_HIGH;
  }

  pstr_env->enc_env_data.sbr_xpos_mode = (ixheaace_sbr_xpos_mode)params->sbr_xpos_mode;
  pstr_env->enc_env_data.sbr_xpos_ctrl = params->sbr_xpos_ctrl;

  if (params->is_ld_sbr) {
    pstr_env->str_sbr_qmf.num_time_slots = 16;
    pstr_env->str_sbr_qmf.rate = 1;
    if (params->frame_flag_480 == 1) {
      pstr_env->str_sbr_qmf.num_time_slots = 15;
    }
  } else {
    pstr_env->str_sbr_qmf.num_time_slots = 16;
    pstr_env->str_sbr_qmf.rate = 2;
    if (params->frame_flag_960 == 1) {
      pstr_env->str_sbr_qmf.num_time_slots = 15;
    }
    if (pstr_sbr_cfg->sbr_codec == USAC_SBR) {
      if (USAC_SBR_RATIO_INDEX_4_1 == pstr_sbr_cfg->sbr_ratio_idx) {
        pstr_env->str_sbr_qmf.rate = 4;
      }
    }
  }

  ixheaace_create_qmf_bank(&pstr_env->str_sbr_qmf, pstr_sbr_tab, params->is_ld_sbr);

  start_index = 576;

  err_code = ixheaace_create_ton_corr_param_extr(
      ch, &pstr_env->str_ton_corr, pstr_sbr_cfg->sample_freq, 64, params->sbr_xpos_ctrl,
      pstr_sbr_cfg->ptr_freq_band_tab[LOW_RES][0], pstr_sbr_cfg->ptr_v_k_master,
      pstr_sbr_cfg->num_master, params->ana_max_level, pstr_sbr_cfg->ptr_freq_band_tab,
      pstr_sbr_cfg->num_scf, pstr_sbr_hdr->sbr_noise_bands, params->use_speech_config,
      ptr_common_buf, pstr_sbr_tab->ptr_qmf_tab, params->is_ld_sbr);
  if (err_code) {
    return err_code;
  }

  pstr_env->enc_env_data.noise_band_count =
      pstr_env->str_ton_corr.sbr_noise_floor_est.num_of_noise_bands;

  noise_groups[0] = pstr_env->enc_env_data.noise_band_count;
  noise_groups[1] = pstr_env->enc_env_data.noise_band_count;

  pstr_env->enc_env_data.sbr_invf_mode = (ixheaace_invf_mode)params->sbr_invf_mode;

  if (pstr_env->enc_env_data.sbr_invf_mode == IXHEAACE_INVF_SWITCHED) {
    pstr_env->enc_env_data.sbr_invf_mode = IXHEAACE_INVF_MID_LEVEL;
    pstr_env->str_ton_corr.switch_inverse_filt = TRUE;
  } else {
    pstr_env->str_ton_corr.switch_inverse_filt = FALSE;
  }

  tran_fc = params->tran_fc;

  if (tran_fc == 0) {
    tran_fc = ixheaac_min32(5000, ixheaace_get_sbr_start_freq_raw(pstr_sbr_hdr->sbr_start_freq,
                                                                  64, pstr_sbr_cfg->sample_freq));
  }

  tran_fc = (tran_fc * 4 * 64 / pstr_sbr_cfg->sample_freq + 1) >> 1;
  if (params->sbr_codec == USAC_SBR) {
    pstr_env->str_sbr_extract_env.sbr_ratio_idx = params->sbr_ratio_idx;
  }
  err_code = ixheaace_create_extract_sbr_envelope(
      ch, &pstr_env->str_sbr_extract_env, start_index, ptr_common_buffer2, ptr_sbr_env_r_buf,
      ptr_sbr_env_i_buf, params->frame_flag_480, params->sbr_codec);
  if (err_code) {
    return err_code;
  }

  ixheaace_create_sbr_code_envelope(&pstr_env->str_sbr_code_env, pstr_sbr_cfg->num_scf,
                                    params->delta_t_across_frames, params->df_edge_1st_env,
                                    params->df_edge_incr);

  ixheaace_create_sbr_code_envelope(&pstr_env->str_sbr_code_noise_floor, noise_groups,
                                    params->delta_t_across_frames, 0, 0);

  pstr_env->sbr_amp_res_init = pstr_sbr_hdr->sbr_amp_res;

  err_code = ixheaace_init_sbr_huffman_tabs(
      &pstr_env->enc_env_data, &pstr_env->str_sbr_code_env, &pstr_env->str_sbr_code_noise_floor,
      pstr_env->sbr_amp_res_init, pstr_sbr_tab->ptr_sbr_huff_tab);
  if (err_code) {
    return err_code;
  }

  ixheaace_create_frame_info_generator(&pstr_env->str_sbr_env_frame, params->spread, e,
                                       params->stat, pstr_env->enc_env_data.freq_res_fix,
                                       params->use_low_freq_res);

  ixheaace_create_sbr_transient_detector(
      &pstr_env->str_sbr_trans_detector, pstr_sbr_cfg->sample_freq,
      params->codec_settings.standard_bitrate * params->codec_settings.num_channels,
      params->codec_settings.bit_rate, params->tran_thr, params->tran_det_mode, tran_fc,
      params->frame_flag_480, params->is_ld_sbr, params->sbr_ratio_idx, params->sbr_codec,
      pstr_sbr_cfg->ptr_freq_band_tab[0][0]);

  pstr_sbr_cfg->xpos_control_switch = params->sbr_xpos_ctrl;
  pstr_env->enc_env_data.no_harmonics = pstr_sbr_cfg->num_scf[HI];
  pstr_env->enc_env_data.synthetic_coding = pstr_sbr_cfg->detect_missing_harmonics;
  pstr_env->enc_env_data.add_harmonic_flag = 0;
  ixheaace_init_esbr_inter_tes(&pstr_env->str_inter_tes_enc, params->sbr_ratio_idx);
  pstr_env->enc_env_data.ptr_sbr_inter_tes_shape = pstr_env->str_inter_tes_enc.bs_tes_shape;
  pstr_env->enc_env_data.ptr_sbr_inter_tes_shape_mode =
      pstr_env->str_inter_tes_enc.bs_tes_shape_mode;

  if (params->sbr_codec == USAC_SBR) {
    pstr_env->enc_env_data.harmonic_sbr = pstr_sbr_hdr->sbr_harmonic;
    if (1 == pstr_env->enc_env_data.harmonic_sbr) {
      WORD32 persist_mem_used = 0, bd;
      WORD32 upsamp_4_flag, num_aac_samples, num_out_samples;
      switch (pstr_sbr_cfg->sbr_ratio_idx) {
        case USAC_SBR_RATIO_INDEX_2_1:
          upsamp_4_flag = 0;
          num_aac_samples = 1024;
          num_out_samples = 2048;
          break;
        case USAC_SBR_RATIO_INDEX_4_1:
          upsamp_4_flag = 1;
          num_aac_samples = 1024;
          num_out_samples = 4096;
          break;
        case USAC_SBR_RATIO_INDEX_8_3:
          upsamp_4_flag = 0;
          num_aac_samples = 768;
          num_out_samples = 2048;
          break;
        default:
          upsamp_4_flag = 0;
          num_aac_samples = 1024;
          num_out_samples = 2048;
          break;
      }

      ixheaace_esbr_hbe_data_init(
          pstr_env->pstr_hbe_enc->pstr_hbe_txposer, num_aac_samples, upsamp_4_flag,
          num_out_samples, pstr_env->pstr_hbe_enc->ptr_hbe_txposer_buffers, &persist_mem_used);

      ixheaace_esbr_qmf_init(&(pstr_env->pstr_hbe_enc->str_codec_qmf_bank),
                             pstr_sbr_cfg->sbr_ratio_idx, num_out_samples);

      for (bd = 0; bd < (IXHEAACE_MAX_FREQ_COEFFS / 2 + 1); bd++) {
        pstr_env->pstr_hbe_enc->pstr_hbe_txposer->freq_band_tbl_lo[bd] =
            pstr_sbr_cfg->sbr_freq_band_tab_lo[bd];
        pstr_env->pstr_hbe_enc->pstr_hbe_txposer->freq_band_tbl_hi[bd] =
            pstr_sbr_cfg->sbr_freq_band_tab_hi[bd];
      }

      for (; bd < (IXHEAACE_MAX_FREQ_COEFFS + 1); bd++) {
        pstr_env->pstr_hbe_enc->pstr_hbe_txposer->freq_band_tbl_hi[bd] =
            pstr_sbr_cfg->sbr_freq_band_tab_hi[bd];
      }

      pstr_env->pstr_hbe_enc->pstr_hbe_txposer->ptr_freq_band_tab[LO] =
          pstr_env->pstr_hbe_enc->pstr_hbe_txposer->freq_band_tbl_lo;
      pstr_env->pstr_hbe_enc->pstr_hbe_txposer->ptr_freq_band_tab[HI] =
          pstr_env->pstr_hbe_enc->pstr_hbe_txposer->freq_band_tbl_hi;
      pstr_env->pstr_hbe_enc->pstr_hbe_txposer->num_sf_bands[0] =
          (WORD16)pstr_sbr_cfg->num_scf[0];
      pstr_env->pstr_hbe_enc->pstr_hbe_txposer->num_sf_bands[1] =
          (WORD16)pstr_sbr_cfg->num_scf[1];
      pstr_env->pstr_hbe_enc->pstr_hbe_txposer->upsamp_4_flag = upsamp_4_flag;
      err_code = ixheaace_dft_hbe_data_reinit(pstr_env->pstr_hbe_enc->pstr_hbe_txposer);
      if (err_code) {
        return err_code;
      }
      err_code = ixheaace_qmf_hbe_data_reinit(pstr_env->pstr_hbe_enc->pstr_hbe_txposer);
      if (err_code) {
        return err_code;
      }
    }
  } else

  {
    pstr_env->enc_env_data.harmonic_sbr = 0;
  }

  return err_code;
}

UWORD32
ixheaace_is_sbr_setting_available(UWORD32 bitrate, UWORD32 num_output_channels,
                                  UWORD32 sample_rate_input, UWORD32 *ptr_core_sr,
                                  ixheaace_str_qmf_tabs *pstr_qmf_tab, WORD32 aot) {
  FLAG table_found = IXHEAACE_TABLE_IDX_NOT_FOUND;
  WORD32 idx_sr;
  WORD32 idx_ch;
  WORD32 idx_entry;

  switch (num_output_channels) {
    case MONO:
      if (sample_rate_input < 16000 || sample_rate_input > 48000) {
        return 0;
      }
    case STEREO:
      if (sample_rate_input < 32000 || sample_rate_input > 48000) {
        return 0;
      }
  }

  *ptr_core_sr = sample_rate_input / 2;

  table_found = ia_enhaacplus_enc_get_sbr_tuning_table_idx(
      bitrate, num_output_channels, *ptr_core_sr, pstr_qmf_tab, NULL, &idx_sr, &idx_ch,
      &idx_entry,
      ((AOT_AAC_ELD == aot) ? pstr_qmf_tab->sbr_tuning_table_ld
                            : pstr_qmf_tab->sbr_tuning_table_lc));

  return (table_found == IXHEAACE_TABLE_IDX_NOT_FOUND) ? 0 : 1;
}

UWORD32 ixheaace_sbr_limit_bitrate(UWORD32 bit_rate, UWORD32 num_ch, UWORD32 core_sample_rate,
                                   ixheaace_str_qmf_tabs *pstr_qmf_tab, WORD32 aot) {
  UWORD32 new_bit_rate;
  WORD32 index;
  WORD32 idx_sr;
  WORD32 idx_ch;
  WORD32 idx_entry;

  index = ia_enhaacplus_enc_get_sbr_tuning_table_idx(
      bit_rate, num_ch, core_sample_rate, pstr_qmf_tab, &new_bit_rate, &idx_sr, &idx_ch,
      &idx_entry,
      ((AOT_AAC_ELD == aot) ? pstr_qmf_tab->sbr_tuning_table_ld
                            : pstr_qmf_tab->sbr_tuning_table_lc));
  if (index != IXHEAACE_TABLE_IDX_NOT_FOUND) {
    new_bit_rate = bit_rate;
  }

  return new_bit_rate;
}

VOID ixheaace_adjust_sbr_settings(const ixheaace_pstr_sbr_cfg pstr_config, UWORD32 bit_rate,
                                  UWORD32 num_ch, UWORD32 fs_core, UWORD32 trans_fac,
                                  UWORD32 std_br, ixheaace_str_qmf_tabs *pstr_qmf_tab,
                                  WORD32 aot, WORD32 is_esbr_4_1) {
  FLAG table_found = IXHEAACE_TABLE_IDX_NOT_FOUND;
  WORD32 idx_sr = 0;
  WORD32 idx_ch = 0;
  WORD32 idx_entry = 0;
  /* set the codec settings */
  pstr_config->codec_settings.bit_rate = bit_rate;
  pstr_config->codec_settings.num_channels = num_ch;
  pstr_config->codec_settings.sample_freq = fs_core;
  pstr_config->codec_settings.trans_fac = trans_fac;
  pstr_config->codec_settings.standard_bitrate = std_br;

  if (bit_rate <= 20000) {
    pstr_config->parametric_coding = 0;
    pstr_config->use_speech_config = 1;
  }

  table_found = ia_enhaacplus_enc_get_sbr_tuning_table_idx(
      bit_rate, num_ch, fs_core, pstr_qmf_tab, NULL, &idx_sr, &idx_ch, &idx_entry,
      ((AOT_AAC_ELD == aot) ? pstr_qmf_tab->sbr_tuning_table_ld
                            : pstr_qmf_tab->sbr_tuning_table_lc));

  if (table_found == IXHEAACE_TABLE_IDX_NOT_FOUND) {
    if (aot == AOT_USAC) {
      if (num_ch == 1) {
        if (bit_rate >= 30000) {
          pstr_config->start_freq = 7;
          pstr_config->stop_freq = 9;
        } else {
          pstr_config->start_freq = 5;
          pstr_config->stop_freq = 7;
        }
      } else {
        pstr_config->start_freq = 12;
        pstr_config->stop_freq = 9;
      }
    }
  } else {
    switch (aot) {
      case AOT_AAC_ELD: {
        pstr_config->start_freq =
            pstr_qmf_tab->sbr_tuning_table_ld[idx_sr][idx_ch][idx_entry].freq_band.start_freq;
        pstr_config->stop_freq =
            pstr_qmf_tab->sbr_tuning_table_ld[idx_sr][idx_ch][idx_entry].freq_band.stop_freq;

        pstr_config->sbr_noise_bands =
            pstr_qmf_tab->sbr_tuning_table_ld[idx_sr][idx_ch][idx_entry].noise.num_noise_bands;

        pstr_config->noise_floor_offset =
            pstr_qmf_tab->sbr_tuning_table_ld[idx_sr][idx_ch][idx_entry].noise.noise_floor_offset;

        pstr_config->ana_max_level =
            pstr_qmf_tab->sbr_tuning_table_ld[idx_sr][idx_ch][idx_entry].noise.noise_max_level;
        pstr_config->stereo_mode =
            pstr_qmf_tab->sbr_tuning_table_ld[idx_sr][idx_ch][idx_entry].stereo_mode;
        pstr_config->freq_scale =
            pstr_qmf_tab->sbr_tuning_table_ld[idx_sr][idx_ch][idx_entry].freq_scale;
        break;
      }
      default: {
        pstr_config->start_freq =
            pstr_qmf_tab->sbr_tuning_table_lc[idx_sr][idx_ch][idx_entry].freq_band.start_freq;
        pstr_config->stop_freq =
            pstr_qmf_tab->sbr_tuning_table_lc[idx_sr][idx_ch][idx_entry].freq_band.stop_freq;

        pstr_config->sbr_noise_bands =
            pstr_qmf_tab->sbr_tuning_table_lc[idx_sr][idx_ch][idx_entry].noise.num_noise_bands;

        pstr_config->noise_floor_offset =
            pstr_qmf_tab->sbr_tuning_table_lc[idx_sr][idx_ch][idx_entry].noise.noise_floor_offset;

        pstr_config->ana_max_level =
            pstr_qmf_tab->sbr_tuning_table_lc[idx_sr][idx_ch][idx_entry].noise.noise_max_level;
        pstr_config->stereo_mode =
            pstr_qmf_tab->sbr_tuning_table_lc[idx_sr][idx_ch][idx_entry].stereo_mode;
        pstr_config->freq_scale =
            pstr_qmf_tab->sbr_tuning_table_lc[idx_sr][idx_ch][idx_entry].freq_scale;
        break;
      }
    }

    if (pstr_config->sbr_codec == ELD_SBR) {
      pstr_config->send_header_data_time = -1;
      if ((num_ch == NUM_CHANS_MONO) && (bit_rate <= 22000)) {
        pstr_config->use_low_freq_res = 1;
      }
      if ((num_ch == NUM_CHANS_STEREO) && (bit_rate <= 48000)) {
        pstr_config->use_low_freq_res = 1;
      }
    }
    else {
      if ((num_ch == NUM_CHANS_MONO) && (bit_rate <= 18000)) {
        pstr_config->use_low_freq_res = 1;
      }
      if ((num_ch == NUM_CHANS_STEREO) && (bit_rate <= 28000)) {
        pstr_config->use_low_freq_res = 1;
      }
    }
    if (bit_rate <= 20000) {
      pstr_config->parametric_coding = 0;
      pstr_config->use_speech_config = 1;
    }

    if (pstr_config->use_ps) {
      pstr_config->ps_mode = ixheaace_get_ps_mode(bit_rate);
    }
  }

  if (is_esbr_4_1) {
    pstr_config->start_freq = 10;
    pstr_config->stop_freq = 11;
  }
}

VOID ixheaace_initialize_sbr_defaults(ixheaace_pstr_sbr_cfg pstr_config) {
  pstr_config->send_header_data_time = 500;
  pstr_config->crc_sbr = 0;
  pstr_config->tran_thr = 13000;
  pstr_config->detect_missing_harmonics = 1;
  pstr_config->parametric_coding = 1;
  pstr_config->use_speech_config = 0;

  pstr_config->sbr_data_extra = 0;
  pstr_config->amp_res = IXHEAACE_SBR_AMP_RES_3_0;
  pstr_config->tran_fc = 0;
  pstr_config->tran_det_mode = 1;
  pstr_config->spread = 1;
  pstr_config->stat = 0;
  pstr_config->e = 1;
  pstr_config->delta_t_across_frames = 1;
  pstr_config->df_edge_1st_env = 0.3f;
  pstr_config->df_edge_incr = 0.3f;

  pstr_config->sbr_invf_mode = IXHEAACE_INVF_SWITCHED;
  pstr_config->sbr_xpos_mode = IXHEAACE_XPOS_LC;
  pstr_config->sbr_xpos_ctrl = SBR_XPOS_CTRL_DEFAULT;
  pstr_config->sbr_xpos_lvl = 0;

  pstr_config->use_ps = 0;
  pstr_config->ps_mode = -1;

  pstr_config->stereo_mode = IXHEAACE_SBR_MODE_SWITCH_LRC;
  pstr_config->ana_max_level = 6;
  pstr_config->noise_floor_offset = 0;
  pstr_config->start_freq = 5;
  pstr_config->stop_freq = 9;

  pstr_config->freq_scale = SBR_FREQ_SCALE_DEFAULT;
  pstr_config->alter_scale = SBR_ALTER_SCALE_DEFAULT;
  pstr_config->sbr_noise_bands = SBR_NOISE_BANDS_DEFAULT;

  pstr_config->sbr_limiter_bands = SBR_LIMITER_BANDS_DEFAULT;
  pstr_config->sbr_limiter_gains = SBR_LIMITER_GAINS_DEFAULT;
  pstr_config->sbr_interpol_freq = SBR_INTERPOL_FREQ_DEFAULT;
  pstr_config->sbr_smoothing_length = SBR_SMOOTHING_LENGTH_DEFAULT;
  pstr_config->is_ld_sbr = 0;
  pstr_config->is_esbr = 0;
  pstr_config->frame_flag_960 = 0;
  pstr_config->frame_flag_480 = 0;
  pstr_config->hq_esbr = 0;
  pstr_config->sbr_pvc_active = 0;
  pstr_config->sbr_harmonic = 0;
  pstr_config->sbr_ratio_idx = 0;  // NO_SBR
  pstr_config->use_low_freq_res = 0;
}

static IA_ERRORCODE ia_enhaacplus_enc_update_freq_band_tab(
    ixheaace_pstr_sbr_config_data pstr_sbr_cfg, ixheaace_pstr_sbr_hdr_data pstr_sbr_hdr,
    WORD32 num_qmf_ch) {
  IA_ERRORCODE err_code = IA_NO_ERROR;
  WORD32 k0, k2;
  WORD32 samp_freq = pstr_sbr_cfg->sample_freq;
  if ((pstr_sbr_cfg->sbr_codec == USAC_SBR) &&
      (pstr_sbr_cfg->sbr_ratio_idx == USAC_SBR_RATIO_INDEX_4_1)) {
    samp_freq = samp_freq / 2;
  }
  err_code = ixheaace_find_start_and_stop_band(
      samp_freq, num_qmf_ch, pstr_sbr_hdr->sbr_start_freq, pstr_sbr_hdr->sbr_stop_freq,
      pstr_sbr_hdr->sample_rate_mode, &k0, &k2, pstr_sbr_cfg->sbr_ratio_idx,
      pstr_sbr_cfg->sbr_codec);
  if (err_code) {
    return err_code;
  }

  err_code = ixheaace_update_freq_scale(
      pstr_sbr_cfg->ptr_v_k_master, &pstr_sbr_cfg->num_master, k0, k2, pstr_sbr_hdr->freq_scale,
      pstr_sbr_hdr->alter_scale, pstr_sbr_hdr->sample_rate_mode);
  if (err_code) {
    return err_code;
  }

  pstr_sbr_hdr->sbr_xover_band = 0;

  ixheaace_update_high_res(pstr_sbr_cfg->ptr_freq_band_tab[HI], &pstr_sbr_cfg->num_scf[HI],
                           pstr_sbr_cfg->ptr_v_k_master, pstr_sbr_cfg->num_master,
                           &pstr_sbr_hdr->sbr_xover_band, pstr_sbr_hdr->sample_rate_mode,
                           num_qmf_ch);

  ixheaace_update_low_res(pstr_sbr_cfg->ptr_freq_band_tab[LO], &pstr_sbr_cfg->num_scf[LO],
                          pstr_sbr_cfg->ptr_freq_band_tab[HI], pstr_sbr_cfg->num_scf[HI]);

  pstr_sbr_cfg->xover_freq =
      (pstr_sbr_cfg->ptr_freq_band_tab[LOW_RES][0] * pstr_sbr_cfg->sample_freq / num_qmf_ch +
       1) >>
      1;

  return err_code;
}

VOID ia_enhaacplus_enc_init_sbr_tabs(ixheaace_str_sbr_tabs *pstr_sbr_tabs) {
  pstr_sbr_tabs->ptr_ps_tab = (ixheaace_str_ps_tab *)&ia_enhaacplus_enc_ps_tab;
  pstr_sbr_tabs->ptr_qmf_tab = (ixheaace_str_qmf_tabs *)&ixheaace_qmf_tab;
  pstr_sbr_tabs->ptr_sbr_huff_tab = (ixheaace_str_sbr_huff_tabs *)&ixheaace_sbr_huff_tab;
  pstr_sbr_tabs->ptr_resamp_tab =
      (ixheaace_resampler_table *)&ixheaace_resamp_2_to_1_iir_filt_params;
  pstr_sbr_tabs->ptr_esbr_sfb_tab = (ixheaace_str_esbr_sfb_bin_tabs *)&ia_esbr_sfb_bin_tabs;
}

IA_ERRORCODE
ixheaace_env_encode_frame(ixheaace_pstr_sbr_enc pstr_env_encoder, FLOAT32 *ptr_samples,
                          FLOAT32 *ptr_core_buffer, UWORD32 time_sn_stride,
                          UWORD8 *ptr_num_anc_bytes, UWORD8 *ptr_anc_data,
                          ixheaace_str_sbr_tabs *pstr_sbr_tab,
                          ixheaace_comm_tables *pstr_common_tab, UWORD8 *ptr_mps_data,
                          WORD32 mps_bits, WORD32 flag_fl_small, WORD32 *usac_stat_bits) {
  IA_ERRORCODE err_code = IA_NO_ERROR;

  if (pstr_env_encoder != NULL) {
    ixheaace_pstr_sbr_bitstream_data pstr_sbr_bs = &pstr_env_encoder->str_sbr_bs;

    pstr_sbr_bs->header_active = 0;
    if ((pstr_env_encoder->str_sbr_cfg.is_ld_sbr) &&
        (pstr_sbr_bs->count_send_header_data == pstr_sbr_bs->nr_send_header_data - 1)) {
      pstr_sbr_bs->header_active = 1;
    }

    if (pstr_sbr_bs->count_send_header_data == 0) {
      pstr_sbr_bs->header_active = 1;
    }

    if (pstr_sbr_bs->nr_send_header_data == 0) {
      pstr_sbr_bs->count_send_header_data = 1;
    } else {
      if (pstr_env_encoder->str_sbr_cfg.is_ld_sbr) {
        if (pstr_sbr_bs->count_send_header_data >= 0) {
          pstr_sbr_bs->count_send_header_data++;

          pstr_sbr_bs->count_send_header_data %= pstr_sbr_bs->nr_send_header_data;
        }
      } else {
        pstr_sbr_bs->count_send_header_data++;

        pstr_sbr_bs->count_send_header_data %= pstr_sbr_bs->nr_send_header_data;
      }
    }

    ixheaace_init_sbr_bitstream(
        &pstr_env_encoder->str_cmon_data, (UWORD8 *)pstr_env_encoder->sbr_payload,
        sizeof(pstr_env_encoder->sbr_payload), pstr_env_encoder->str_sbr_bs.crc_active,
        pstr_env_encoder->str_sbr_cfg.sbr_codec);

    err_code = ixheaace_extract_sbr_envelope(ptr_samples, ptr_core_buffer, time_sn_stride,
                                             pstr_env_encoder, pstr_sbr_tab, pstr_common_tab,
                                             flag_fl_small);
    if (err_code) {
      return err_code;
    }

    if (mps_bits) {
      WORD32 num_bytes;
      if (pstr_env_encoder->str_sbr_cfg.sbr_codec == ELD_SBR) {
        ixheaace_write_bits(&pstr_env_encoder->str_cmon_data.str_sbr_bit_buf,
                            IXHEAACE_MPS_EXT_LDSAC_DATA, 4);
        ixheaace_write_bits(&pstr_env_encoder->str_cmon_data.str_sbr_bit_buf, *ptr_mps_data++, 4);
      }
      num_bytes = mps_bits >> 3;
      for (WORD32 k = 0; k < num_bytes; k++) {
        ixheaace_write_bits(&pstr_env_encoder->str_cmon_data.str_sbr_bit_buf, *ptr_mps_data++, 8);
      }
      if (mps_bits & 0x7) {
        ixheaace_write_bits(&pstr_env_encoder->str_cmon_data.str_sbr_bit_buf,
                            (*ptr_mps_data++) >> (8 - (mps_bits & 0x7)), mps_bits & 0x7);
      }
    }

    ixheaace_assemble_sbr_bitstream(&pstr_env_encoder->str_cmon_data,
                                    pstr_env_encoder->str_sbr_cfg.sbr_codec);

    pstr_env_encoder->sbr_payload_size =
        ((ia_enhaacplus_enc_get_bits_available(&pstr_env_encoder->str_cmon_data.str_sbr_bit_buf) +
          7) /
         8);

    if (pstr_env_encoder->sbr_payload_size > IXHEAACE_MAX_PAYLOAD_SIZE) {
      pstr_env_encoder->sbr_payload_size = 0;
    }

    if (ptr_anc_data) {
      *ptr_num_anc_bytes = (UWORD8)pstr_env_encoder->sbr_payload_size;
      memcpy(ptr_anc_data, pstr_env_encoder->sbr_payload, pstr_env_encoder->sbr_payload_size);
    }
    if (usac_stat_bits) {
      *usac_stat_bits = pstr_env_encoder->str_cmon_data.sbr_hdr_bits +
                        pstr_env_encoder->str_cmon_data.sbr_data_bits;
    }
  }
  return err_code;
}

WORD32 ixheaace_sbr_enc_scr_size(VOID) {
  return IXHEAAC_GET_SIZE_ALIGNED(sizeof(ixheaace_str_sbr_enc_scratch), BYTE_ALIGN_8);
}

VOID ia_enhaacplus_enc_get_scratch_bufs(VOID *ptr_scr, FLOAT32 **ptr_shared_buf1,
                                        FLOAT32 **ptr_shared_buf2) {
  ixheaace_str_sbr_enc_scratch *ptr_sbr_enc_scr = (ixheaace_str_sbr_enc_scratch *)ptr_scr;

  *ptr_shared_buf1 = ptr_sbr_enc_scr->sbr_env_r_buf;
  *ptr_shared_buf2 = ptr_sbr_enc_scr->sbr_env_i_buf;
}

VOID ia_enhaacplus_enc_get_shared_bufs(VOID *ptr_scr, WORD32 **ptr_shared_buf1,
                                       WORD32 **ptr_shared_buf2, WORD32 **ptr_shared_buf3,
                                       WORD8 **ptr_shared_buf4, WORD32 aacenc_blocksize) {
  ixheaace_str_sbr_enc_scratch *ptr_sbr_enc_scr = (ixheaace_str_sbr_enc_scratch *)ptr_scr;

  *ptr_shared_buf1 = (WORD32 *)ptr_sbr_enc_scr->ps_buf3;
  *ptr_shared_buf2 = (WORD32 *)ptr_sbr_enc_scr->sbr_env_r_buf;
  *ptr_shared_buf3 = (WORD32 *)ptr_sbr_enc_scr->sbr_env_i_buf;
  *ptr_shared_buf4 =
      (WORD8 *)&ptr_sbr_enc_scr
          ->sbr_env_i_buf[IXHEAACE_QMF_TIME_SLOTS * IXHEAACE_QMF_CHANNELS + aacenc_blocksize];
}

VOID ixheaace_sbr_set_scratch_ptr(ixheaace_pstr_sbr_enc pstr_env_enc, VOID *ptr_scr) {
  pstr_env_enc->ptr_sbr_enc_scr = ptr_scr;
}

WORD32 ixheaace_sbr_enc_pers_size(WORD32 num_ch, WORD32 use_ps, WORD32 harmonic_sbr) {
  WORD32 num_bytes;
  num_bytes = IXHEAAC_GET_SIZE_ALIGNED(sizeof(struct ixheaace_str_sbr_enc), BYTE_ALIGN_8);
  num_bytes += (IXHEAAC_GET_SIZE_ALIGNED(sizeof(struct ixheaace_str_enc_channel), BYTE_ALIGN_8) *
                num_ch);
  num_bytes += IXHEAAC_GET_SIZE_ALIGNED(sizeof(ixheaace_pvc_enc), BYTE_ALIGN_8);
  num_bytes += (IXHEAAC_GET_SIZE_ALIGNED(2 * sizeof(FLOAT32) * QMF_FILTER_LENGTH, BYTE_ALIGN_8) *
               num_ch);
  if (1 == harmonic_sbr) {
    num_bytes += (IXHEAAC_GET_SIZE_ALIGNED(sizeof(ixheaace_str_hbe_enc), BYTE_ALIGN_8) * num_ch);
    num_bytes += (IXHEAAC_GET_SIZE_ALIGNED(sizeof(ixheaace_str_esbr_hbe_txposer), BYTE_ALIGN_8) *
                  num_ch);
    num_bytes += (IXHEAAC_GET_SIZE_ALIGNED(IXHEAACE_MAX_HBE_PERSISTENT_SIZE, BYTE_ALIGN_8) *
                  num_ch);
    num_bytes += IXHEAAC_GET_SIZE_ALIGNED(ESBR_RESAMP_SAMPLES * sizeof(FLOAT32), BYTE_ALIGN_8);
  }
  num_bytes += (IXHEAAC_GET_SIZE_ALIGNED(sizeof(FLOAT32) * 5 * NO_OF_ESTIMATES *
                MAXIMUM_FREQ_COEFFS, BYTE_ALIGN_8) * num_ch);

  num_bytes += (IXHEAAC_GET_SIZE_ALIGNED(sizeof(FLOAT32) * MAX_QMF_TIME_SLOTS *
                IXHEAACE_QMF_CHANNELS, BYTE_ALIGN_8) * num_ch);

  if (use_ps) {
    num_bytes += IXHEAAC_GET_SIZE_ALIGNED(sizeof(struct ixheaace_str_enc_channel), BYTE_ALIGN_8);
    num_bytes += IXHEAAC_GET_SIZE_ALIGNED(sizeof(struct ixheaace_ps_enc), BYTE_ALIGN_8);
    num_bytes += IXHEAAC_GET_SIZE_ALIGNED(sizeof(FLOAT32) * QMF_FILTER_LENGTH, BYTE_ALIGN_8);

    num_bytes += IXHEAAC_GET_SIZE_ALIGNED(sizeof(WORD32) * 5 * NO_OF_ESTIMATES *
                 MAXIMUM_FREQ_COEFFS, BYTE_ALIGN_8);

    /*shared between spectral_band_replication_envYBuffer_fix and IIC IDD PS data buffers*/
    num_bytes += IXHEAAC_GET_SIZE_ALIGNED(sizeof(WORD32) * IXHEAACE_QMF_TIME_SLOTS *
                 IXHEAACE_QMF_CHANNELS, BYTE_ALIGN_8);
  }
  num_bytes += IXHEAAC_GET_SIZE_ALIGNED(sizeof(ixheaace_str_sbr_qmf_filter_bank), BYTE_ALIGN_8);
  return num_bytes;
}

VOID ia_enhaacplus_enc_sbr_set_persist_buf(WORD8 *ptr_base, WORD32 num_ch, WORD32 use_ps,
                                           WORD32 harmonic_sbr) {
  struct ixheaace_str_sbr_enc *pstr_env_enc;
  WORD8 *ptr_curr_mem = ptr_base +
    IXHEAAC_GET_SIZE_ALIGNED(sizeof(struct ixheaace_str_sbr_enc), BYTE_ALIGN_8);
  WORD32 i;

  pstr_env_enc = (struct ixheaace_str_sbr_enc *)ptr_base;

  for (i = 0; i < num_ch; i++) {
    pstr_env_enc->pstr_env_channel[i] = (struct ixheaace_str_enc_channel *)(ptr_curr_mem);
    ptr_curr_mem = ptr_curr_mem +
      IXHEAAC_GET_SIZE_ALIGNED(sizeof(struct ixheaace_str_enc_channel), BYTE_ALIGN_8);
  }

  for (i = 0; i < num_ch; i++) {
    pstr_env_enc->pstr_env_channel[i]->str_sbr_qmf.ptr_sbr_qmf_states_ana =
        (FLOAT32 *)ptr_curr_mem;
    ptr_curr_mem += IXHEAAC_GET_SIZE_ALIGNED(
        sizeof(pstr_env_enc->pstr_env_channel[i]->str_sbr_qmf.ptr_sbr_qmf_states_ana[0]) *
        QMF_FILTER_LENGTH, BYTE_ALIGN_8);
  }
  if (!use_ps) {
    pstr_env_enc->ptr_common_buffer1 = (WORD32 *)ptr_curr_mem;
    ptr_curr_mem += IXHEAAC_GET_SIZE_ALIGNED(sizeof(pstr_env_enc->ptr_common_buffer1[0]) *
                    num_ch * 5 * NO_OF_ESTIMATES * MAXIMUM_FREQ_COEFFS, BYTE_ALIGN_8);

    pstr_env_enc->ptr_common_buffer2 = (WORD32 *)ptr_curr_mem;
    ptr_curr_mem += IXHEAAC_GET_SIZE_ALIGNED(sizeof(pstr_env_enc->ptr_common_buffer2[0]) *
                    num_ch * MAX_QMF_TIME_SLOTS * IXHEAACE_QMF_CHANNELS, BYTE_ALIGN_8);
  } else {
    pstr_env_enc->ptr_common_buffer1 = (WORD32 *)ptr_curr_mem;
    ptr_curr_mem += IXHEAAC_GET_SIZE_ALIGNED(2 * sizeof(pstr_env_enc->ptr_common_buffer1[0]) *
                    5 * NO_OF_ESTIMATES * MAXIMUM_FREQ_COEFFS, BYTE_ALIGN_8);

    pstr_env_enc->ptr_common_buffer2 = (WORD32 *)ptr_curr_mem;
    ptr_curr_mem += IXHEAAC_GET_SIZE_ALIGNED(2 * sizeof(pstr_env_enc->ptr_common_buffer2[0]) *
                    IXHEAACE_QMF_TIME_SLOTS * IXHEAACE_QMF_CHANNELS, BYTE_ALIGN_8);
  }
  // PVC encoder
  pstr_env_enc->pstr_pvc_enc = (ixheaace_pvc_enc *)ptr_curr_mem;
  ptr_curr_mem = ptr_curr_mem + IXHEAAC_GET_SIZE_ALIGNED(sizeof(ixheaace_pvc_enc), BYTE_ALIGN_8);
  // Harmonic SBR
  if (1 == harmonic_sbr) {
    for (i = 0; i < num_ch; i++) {
      pstr_env_enc->pstr_env_channel[i]->pstr_hbe_enc = (ixheaace_str_hbe_enc *)ptr_curr_mem;
      ptr_curr_mem = ptr_curr_mem +
        IXHEAAC_GET_SIZE_ALIGNED(sizeof(ixheaace_str_hbe_enc), BYTE_ALIGN_8);
      pstr_env_enc->pstr_env_channel[i]->pstr_hbe_enc->pstr_hbe_txposer =
          (ixheaace_str_esbr_hbe_txposer *)ptr_curr_mem;
      ptr_curr_mem = ptr_curr_mem +
        IXHEAAC_GET_SIZE_ALIGNED(sizeof(ixheaace_str_esbr_hbe_txposer), BYTE_ALIGN_8);
      pstr_env_enc->pstr_env_channel[i]->pstr_hbe_enc->ptr_hbe_txposer_buffers =
          (VOID *)ptr_curr_mem;
      ptr_curr_mem = ptr_curr_mem + IXHEAACE_MAX_HBE_PERSISTENT_SIZE;
    }
    pstr_env_enc->ptr_hbe_resample_buf = (FLOAT32 *)ptr_curr_mem;
    ptr_curr_mem = ptr_curr_mem + IXHEAAC_GET_SIZE_ALIGNED(
      (ESBR_RESAMP_SAMPLES * sizeof(pstr_env_enc->ptr_hbe_resample_buf[0])), BYTE_ALIGN_8);
  }
  if (use_ps) {
    pstr_env_enc->pstr_env_channel[1] = (struct ixheaace_str_enc_channel *)(ptr_curr_mem);
    ptr_curr_mem = ptr_curr_mem +
      IXHEAAC_GET_SIZE_ALIGNED(sizeof(struct ixheaace_str_enc_channel), BYTE_ALIGN_8);
    memset(pstr_env_enc->pstr_env_channel[1], 0, sizeof(struct ixheaace_str_enc_channel));

    pstr_env_enc->pstr_env_channel[1]->str_sbr_qmf.ptr_sbr_qmf_states_ana =
        (FLOAT32 *)ptr_curr_mem;
    ptr_curr_mem += IXHEAAC_GET_SIZE_ALIGNED(
      sizeof(pstr_env_enc->pstr_env_channel[1]->str_sbr_qmf.ptr_sbr_qmf_states_ana[0]) *
        QMF_FILTER_LENGTH, BYTE_ALIGN_8);
    memset(pstr_env_enc->pstr_env_channel[1]->str_sbr_qmf.ptr_sbr_qmf_states_ana, 0,
           sizeof(pstr_env_enc->pstr_env_channel[1]->str_sbr_qmf.ptr_sbr_qmf_states_ana[0]) *
               QMF_FILTER_LENGTH);

    pstr_env_enc->pstr_ps_enc = (struct ixheaace_ps_enc *)(ptr_curr_mem);
    ptr_curr_mem = ptr_curr_mem +
      IXHEAAC_GET_SIZE_ALIGNED(sizeof(struct ixheaace_ps_enc), BYTE_ALIGN_8);
    memset(pstr_env_enc->pstr_ps_enc, 0, sizeof(struct ixheaace_ps_enc));
  }
  pstr_env_enc->pstr_synthesis_qmf_bank = (ixheaace_str_sbr_qmf_filter_bank *)(ptr_curr_mem);
  memset(pstr_env_enc->pstr_synthesis_qmf_bank, 0, sizeof(ixheaace_str_sbr_qmf_filter_bank));

  pstr_env_enc->pstr_synthesis_qmf_bank->ptr_sbr_qmf_states_ana = NULL;
}

IA_ERRORCODE
ixheaace_env_open(ixheaace_pstr_sbr_enc *pstr_env_encoder, ixheaace_pstr_sbr_cfg params,
                  WORD32 *ptr_core_bw, WORD8 *ptr_sbr_scratch,
                  ixheaace_str_sbr_tabs *pstr_sbr_tab,
                  ixheaace_pstr_sbr_hdr_data *pstr_sbr_config) {
  ixheaace_pstr_sbr_enc pstr_env_enc;
  WORD32 ch;
  IA_ERRORCODE err_code = IA_NO_ERROR;

  pstr_env_enc = *pstr_env_encoder;

  memset(pstr_env_enc, 0, sizeof(struct ixheaace_str_sbr_enc));

  *pstr_sbr_config = &pstr_env_enc->str_sbr_hdr;

  ixheaace_sbr_set_scratch_ptr(pstr_env_enc, ptr_sbr_scratch);

  ia_enhaacplus_enc_sbr_set_persist_buf((WORD8 *)pstr_env_enc,
                                        params->codec_settings.num_channels, params->use_ps,
                                        params->sbr_harmonic);

  if ((params->codec_settings.num_channels < 1) ||
      (params->codec_settings.num_channels > IXHEAACE_MAX_CH_IN_BS_ELE)) {
    return IA_EXHEAACE_INIT_FATAL_SBR_INVALID_NUM_CHANNELS;
  }

  pstr_env_enc->str_sbr_cfg.ptr_freq_band_tab[LO] =
      pstr_env_enc->str_sbr_cfg.sbr_freq_band_tab_lo;

  memset(
      pstr_env_enc->str_sbr_cfg.ptr_freq_band_tab[LO], 0,
      sizeof(pstr_env_enc->str_sbr_cfg.ptr_freq_band_tab[LO][0]) * (MAXIMUM_FREQ_COEFFS / 2 + 1));

  pstr_env_enc->str_sbr_cfg.ptr_freq_band_tab[HI] =
      pstr_env_enc->str_sbr_cfg.sbr_freq_band_tab_hi;

  memset(pstr_env_enc->str_sbr_cfg.ptr_freq_band_tab[HI], 0,
         sizeof(pstr_env_enc->str_sbr_cfg.ptr_freq_band_tab[HI][0]) * (MAXIMUM_FREQ_COEFFS + 1));

  pstr_env_enc->str_sbr_cfg.ptr_v_k_master = pstr_env_enc->str_sbr_cfg.sbr_v_k_master;

  memset(pstr_env_enc->str_sbr_cfg.ptr_v_k_master, 0,
         sizeof(pstr_env_enc->str_sbr_cfg.ptr_v_k_master[0]) * (MAXIMUM_FREQ_COEFFS + 1));

  ia_enhaacplus_enc_create_bitbuffer(&pstr_env_enc->str_cmon_data.str_sbr_bit_buf,
                                     (UWORD8 *)pstr_env_enc->sbr_payload,
                                     sizeof(pstr_env_enc->sbr_payload));

  pstr_env_enc->str_cmon_data.prev_bit_buf_read_offset = 0;
  pstr_env_enc->str_cmon_data.prev_bit_buf_write_offset = 0;

  ia_enhaacplus_enc_create_bitbuffer(&pstr_env_enc->str_cmon_data.str_sbr_bit_buf_prev,
                                     (UWORD8 *)pstr_env_enc->sbr_payload_prev,
                                     sizeof(pstr_env_enc->sbr_payload));

  pstr_env_enc->str_cmon_data.str_sbr_bit_buf_prev.ptr_read_next =
      pstr_env_enc->str_cmon_data.str_sbr_bit_buf_prev.ptr_bit_buf_base +
      pstr_env_enc->str_cmon_data.prev_bit_buf_read_offset;
  pstr_env_enc->str_cmon_data.str_sbr_bit_buf_prev.ptr_write_next =
      pstr_env_enc->str_cmon_data.str_sbr_bit_buf_prev.ptr_bit_buf_base +
      pstr_env_enc->str_cmon_data.prev_bit_buf_write_offset;

  pstr_env_enc->str_sbr_cfg.num_ch = params->codec_settings.num_channels;
  pstr_env_enc->str_sbr_cfg.is_ld_sbr = params->is_ld_sbr;
  pstr_env_enc->str_sbr_cfg.sbr_codec = params->sbr_codec;
  pstr_env_enc->str_sbr_cfg.is_esbr = params->is_esbr;
  if (pstr_env_enc->str_sbr_cfg.sbr_codec == USAC_SBR) {
    pstr_env_enc->str_sbr_cfg.sbr_ratio_idx = params->sbr_ratio_idx;
  }
  pstr_env_enc->str_sbr_cfg.stereo_mode =
      (params->codec_settings.num_channels == 2) ? params->stereo_mode : IXHEAACE_SBR_MODE_MONO;

  if (params->codec_settings.sample_freq <= 24000) {
    pstr_env_enc->str_sbr_hdr.sample_rate_mode = IXHEAACE_DUAL_RATE;
    if (params->sbr_codec == USAC_SBR) {
      pstr_env_enc->str_sbr_cfg.sample_freq = 2 * params->codec_settings.sample_freq;
      if (USAC_SBR_RATIO_INDEX_4_1 == params->sbr_ratio_idx) {
        pstr_env_enc->str_sbr_cfg.sample_freq = 4 * params->codec_settings.sample_freq;
        pstr_env_enc->str_sbr_hdr.sample_rate_mode = IXHEAACE_QUAD_RATE;
      }
    } else {
      pstr_env_enc->str_sbr_cfg.sample_freq = 2 * params->codec_settings.sample_freq;
    }
  } else {
    pstr_env_enc->str_sbr_hdr.sample_rate_mode = IXHEAACE_SINGLE_RATE;
    pstr_env_enc->str_sbr_cfg.sample_freq = params->codec_settings.sample_freq;
  }
  if (params->is_ld_sbr) {
    pstr_env_enc->str_sbr_bs.count_send_header_data = -1;
  } else {
    pstr_env_enc->str_sbr_bs.count_send_header_data = 0;
  }
  if (params->send_header_data_time > 0) {
    pstr_env_enc->str_sbr_bs.nr_send_header_data = (WORD32)(
        params->send_header_data_time * 0.001 * pstr_env_enc->str_sbr_cfg.sample_freq / 2048);

    pstr_env_enc->str_sbr_bs.nr_send_header_data =
        ixheaac_max32(pstr_env_enc->str_sbr_bs.nr_send_header_data, 1);
  } else {
    pstr_env_enc->str_sbr_bs.nr_send_header_data = 0;
  }

  pstr_env_enc->str_sbr_hdr.sbr_data_extra = params->sbr_data_extra;
  pstr_env_enc->str_sbr_bs.crc_active = params->crc_sbr;
  pstr_env_enc->str_sbr_bs.header_active = 0;
  pstr_env_enc->str_sbr_hdr.sbr_start_freq = params->start_freq;
  pstr_env_enc->str_sbr_hdr.sbr_stop_freq = params->stop_freq;
  pstr_env_enc->str_sbr_hdr.sbr_xover_band = 0;

  if (params->sbr_xpos_ctrl != SBR_XPOS_CTRL_DEFAULT) {
    pstr_env_enc->str_sbr_hdr.sbr_data_extra = 1;
  }

  pstr_env_enc->str_sbr_hdr.protocol_version = SI_SBR_PROTOCOL_VERSION_ID;

  pstr_env_enc->str_sbr_hdr.sbr_amp_res = (ixheaace_amp_res)params->amp_res;

  pstr_env_enc->str_sbr_hdr.freq_scale = params->freq_scale;
  pstr_env_enc->str_sbr_hdr.alter_scale = params->alter_scale;
  pstr_env_enc->str_sbr_hdr.sbr_noise_bands = params->sbr_noise_bands;
  pstr_env_enc->str_sbr_hdr.header_extra_1 = 0;

  if ((params->freq_scale != SBR_FREQ_SCALE_DEFAULT) ||
      (params->alter_scale != SBR_ALTER_SCALE_DEFAULT) ||
      (params->sbr_noise_bands != SBR_NOISE_BANDS_DEFAULT)) {
    pstr_env_enc->str_sbr_hdr.header_extra_1 = 1;
  }

  pstr_env_enc->str_sbr_hdr.sbr_limiter_bands = params->sbr_limiter_bands;
  pstr_env_enc->str_sbr_hdr.sbr_limiter_gains = params->sbr_limiter_gains;
  pstr_env_enc->str_sbr_hdr.sbr_interpol_freq = params->sbr_interpol_freq;
  pstr_env_enc->str_sbr_hdr.sbr_smoothing_length = params->sbr_smoothing_length;
  pstr_env_enc->str_sbr_hdr.header_extra_2 = 0;

  if ((params->sbr_limiter_bands != SBR_LIMITER_BANDS_DEFAULT) ||
      (params->sbr_limiter_gains != SBR_LIMITER_GAINS_DEFAULT) ||
      (params->sbr_interpol_freq != SBR_INTERPOL_FREQ_DEFAULT) ||
      (params->sbr_smoothing_length != SBR_SMOOTHING_LENGTH_DEFAULT)) {
    pstr_env_enc->str_sbr_hdr.header_extra_2 = 1;
  }
  pstr_env_enc->str_sbr_hdr.sbr_harmonic = params->sbr_harmonic;
  pstr_env_enc->str_sbr_hdr.sbr_pvc_active = params->sbr_pvc_active;
  pstr_env_enc->str_sbr_hdr.hq_esbr = params->hq_esbr;

  pstr_env_enc->str_sbr_cfg.detect_missing_harmonics = params->detect_missing_harmonics;
  pstr_env_enc->str_sbr_cfg.use_parametric_coding = params->parametric_coding;

  err_code = ia_enhaacplus_enc_update_freq_band_tab(
      &pstr_env_enc->str_sbr_cfg, &pstr_env_enc->str_sbr_hdr, IXHEAACE_QMF_CHANNELS);
  if (err_code) {
    return err_code;
  }
  ch = 0;
  while (ch < pstr_env_enc->str_sbr_cfg.num_ch) {
    FLOAT32 *ptr_sbr_env_r_buf = &pstr_env_enc->ptr_sbr_enc_scr->sbr_env_r_buf[0];
    FLOAT32 *ptr_sbr_env_i_buf = &pstr_env_enc->ptr_sbr_enc_scr->sbr_env_i_buf[0];
    if (!params->use_ps) {
      ptr_sbr_env_r_buf = &pstr_env_enc->ptr_sbr_enc_scr->sbr_env_r_buffer[0];
      ptr_sbr_env_i_buf = &pstr_env_enc->ptr_sbr_enc_scr->sbr_env_i_buffer[0];
    }

    err_code = ixheaace_create_env_channel(
        ch, &pstr_env_enc->str_sbr_cfg, &pstr_env_enc->str_sbr_hdr,
        pstr_env_enc->pstr_env_channel[ch], params, pstr_env_enc->ptr_common_buffer1,
        pstr_env_enc->ptr_common_buffer2, ptr_sbr_env_r_buf, ptr_sbr_env_i_buf, pstr_sbr_tab);
    if (err_code) {
      return err_code;
    }
    ch++;
  }

  if (params->use_ps) {
    err_code = ixheaace_create_env_channel(
        1, &pstr_env_enc->str_sbr_cfg, &pstr_env_enc->str_sbr_hdr,
        pstr_env_enc->pstr_env_channel[1], params, pstr_env_enc->ptr_common_buffer1,
        pstr_env_enc->ptr_common_buffer2, pstr_env_enc->ptr_sbr_enc_scr->sbr_env_r_buf,
        pstr_env_enc->ptr_sbr_enc_scr->sbr_env_i_buf, pstr_sbr_tab);
    if (err_code) {
      return err_code;
    }

    ixheaace_create_synthesis_qmf_bank(pstr_env_enc->pstr_synthesis_qmf_bank,
                                       pstr_env_enc->ptr_common_buffer1, pstr_sbr_tab);

    err_code = ixheaace_create_ps_enc(pstr_env_enc->pstr_ps_enc, params->ps_mode,
                                      (FLOAT32 *)pstr_env_enc->ptr_common_buffer1,
                                      (FLOAT32 *)pstr_env_enc->ptr_common_buffer2,
                                      (FLOAT32 *)pstr_env_enc->ptr_sbr_enc_scr->ps_buf3);
    if (err_code) {
      return err_code;
    }
  }

  pstr_env_enc->str_cmon_data.sbr_num_channels = pstr_env_enc->str_sbr_cfg.num_ch;
  if (USAC_SBR == params->sbr_codec) {
    ixheaace_pvc_enc_init(pstr_env_enc->pstr_pvc_enc, params->sbr_pvc_rate);
  }
  *pstr_env_encoder = pstr_env_enc;
  *ptr_core_bw = pstr_env_enc->str_sbr_cfg.xover_freq;

  return err_code;
}
