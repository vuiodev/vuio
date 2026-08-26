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

#include <stdlib.h>
#include <math.h>

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
#include "ixheaace_psy_const.h"
#include "ixheaace_tns.h"
#include "ixheaace_tns_params.h"
#include "ixheaace_rom.h"
#include "ixheaace_common_rom.h"
#include "ixheaace_bitbuffer.h"

#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops16.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaac_basic_ops.h"

#include "ixheaace_block_switch.h"
#include "ixheaace_psy_utils_spreading.h"
#include "ixheaace_psy_utils.h"
#include "ixheaace_calc_ms_band_energy.h"
#include "ixheaace_tns.h"
#include "ixheaace_adjust_threshold_data.h"
#include "ixheaace_dynamic_bits.h"
#include "ixheaace_qc_data.h"
#include "ixheaace_psy_data.h"
#include "ixheaace_ms_stereo.h"
#include "ixheaace_interface.h"

#include "ixheaace_write_bitstream.h"
#include "ixheaace_psy_configuration.h"
#include "ixheaace_psy_mod.h"
#include "ixheaace_stereo_preproc.h"
#include "ixheaace_enc_main.h"
#include "ixheaace_group_data.h"

#include "ixheaace_tns_func.h"

#include "ixheaace_tns_params.h"
#include "ixheaace_common_utils.h"
#include "ixheaace_fft.h"

static WORD32 ia_enhaacplus_enc_block_type_to_window_shape_lc[] = {KBD_WINDOW, SINE_WINDOW,
                                                                   SINE_WINDOW, KBD_WINDOW};

static WORD32 ia_enhaacplus_enc_block_type_to_window_shape_ld[] = {SINE_WINDOW, LD_WINDOW,
                                                                   LD_WINDOW, SINE_WINDOW};

WORD32 ia_enhaacplus_enc_psy_new(ixheaace_psy_kernel *pstr_h_psy, WORD32 num_chan,
                                 WORD32 *ptr_shared_buffer_2, WORD32 long_frame_len)

{
  WORD32 i;
  for (i = 0; i < num_chan; i++) {
    pstr_h_psy->psy_data[i]->ptr_spec_coeffs =
        (FLOAT32 *)(&ptr_shared_buffer_2[i * long_frame_len]);

    memset(pstr_h_psy->psy_data[i]->ptr_spec_coeffs, 0,
           long_frame_len * sizeof(*pstr_h_psy->psy_data[i]->ptr_spec_coeffs));
  }

  pstr_h_psy->p_scratch_tns_float = (FLOAT32 *)(&ptr_shared_buffer_2[2 * long_frame_len]);
  if (long_frame_len == FRAME_LEN_960) {
    pstr_h_psy->p_scratch_tns_float = pstr_h_psy->p_scratch_tns_float + 128;
  }

  memset(pstr_h_psy->p_scratch_tns_float, 0,
         long_frame_len * sizeof(*pstr_h_psy->p_scratch_tns_float));

  return IA_NO_ERROR;
}

IA_ERRORCODE ia_enhaacplus_enc_psy_main_init(ixheaace_psy_kernel *pstr_h_psy, WORD32 sample_rate,
                                             WORD32 bit_rate, WORD32 channels, WORD32 tns_mask,
                                             WORD32 bandwidth, WORD32 aot,
                                             ixheaace_aac_tables *pstr_aac_tables,
                                             WORD32 frame_len_long)

{
  WORD32 ch;
  IA_ERRORCODE err;

  err = ia_enhaacplus_enc_init_psy_configuration(bit_rate / channels, sample_rate, bandwidth, aot,
                                                 &(pstr_h_psy->psy_conf_long), pstr_aac_tables,
                                                 frame_len_long);
  if (err != IA_NO_ERROR) {
    return err;
  }

  if (!err) {
    err = ia_enhaacplus_enc_init_tns_configuration(
        bit_rate, sample_rate, channels, &pstr_h_psy->psy_conf_long.str_tns_conf,
        &(pstr_h_psy->psy_conf_long), (WORD32)(tns_mask & 1), pstr_aac_tables->pstr_tns_tab,
        frame_len_long, aot);

    if (err != IA_NO_ERROR) {
      return err;
    }
  }
  if (aot == AOT_AAC_LC || aot == AOT_SBR || aot == AOT_PS) {
    if (!err) {
      err = ia_enhaacplus_enc_init_psy_configuration_short(
          bit_rate / channels, sample_rate, bandwidth, aot, &pstr_h_psy->psy_conf_short,
          pstr_aac_tables, frame_len_long / BLK_SWITCH_WIN);
      if (err != IA_NO_ERROR) {
        return err;
      }
    }

    if (!err) {
      err = ia_enhaacplus_enc_init_tns_configuration_short(
          bit_rate, sample_rate, channels, &pstr_h_psy->psy_conf_short.str_tns_conf,
          &(pstr_h_psy->psy_conf_short), (WORD32)(tns_mask & 1), pstr_aac_tables->pstr_tns_tab,
          frame_len_long / BLK_SWITCH_WIN, aot);
      if (err != IA_NO_ERROR) {
        return err;
      }
    }
  }

  if (!err) {
    /* pstr_h_psy->psy_data[] */
    for (ch = 0; ch < channels; ch++) {
      switch (aot) {
        case AOT_AAC_LC:
        case AOT_SBR:
        case AOT_PS:
          iaace_init_block_switching(&pstr_h_psy->psy_data[ch]->blk_switch_cntrl, bit_rate,
                                     channels);
          break;

        case AOT_AAC_LD:
        case AOT_AAC_ELD:
          pstr_h_psy->psy_data[ch]->blk_switch_cntrl.win_seq = LONG_WINDOW;
          pstr_h_psy->psy_data[ch]->blk_switch_cntrl.nxt_win_seq = LONG_WINDOW;
          break;
      }
      ia_enhaacplus_enc_init_pre_echo_control(pstr_h_psy->psy_data[ch]->sfb_threshold_nm1_float,
                                              pstr_h_psy->psy_conf_long.sfb_cnt,
                                              pstr_h_psy->psy_conf_long.sfb_threshold_quiet);
    }
  }

  return IA_NO_ERROR;
}

static VOID ia_enhaacplus_enc_advance_psy_long_ms(
    ixheaace_psy_data **psy_data,
    ixheaace_psy_configuration_long *pstr_psy_conf_long) {
  ia_enhaacplus_enc_calc_band_energy_ms(
      psy_data[0]->ptr_spec_coeffs, psy_data[1]->ptr_spec_coeffs, pstr_psy_conf_long->sfb_offsets,
      pstr_psy_conf_long->sfb_active, pstr_psy_conf_long->sfb_cnt,
      psy_data[0]->sfb_energy_ms.long_nrg, &psy_data[0]->sfb_energy_sum_ms.long_nrg,
      psy_data[1]->sfb_energy_ms.long_nrg, &psy_data[1]->sfb_energy_sum_ms.long_nrg);
}

static VOID ia_enhaacplus_enc_advance_psy_short_ms(
    ixheaace_psy_data **psy_data,
    ixheaace_psy_configuration_short *pstr_psy_conf_short, WORD32 ccfl) {
  WORD32 w;
  WORD32 frame_len_short = FRAME_LEN_SHORT_128;

  if (ccfl == FRAME_LEN_960) {
    frame_len_short = FRAME_LEN_SHORT_120;
  }
  for (w = 0; w < TRANS_FAC; w++) {
    WORD32 w_offset = w * frame_len_short;

    ia_enhaacplus_enc_calc_band_energy_ms(
        psy_data[0]->ptr_spec_coeffs + w_offset, psy_data[1]->ptr_spec_coeffs + w_offset,
        pstr_psy_conf_short->sfb_offsets, pstr_psy_conf_short->sfb_active,
        pstr_psy_conf_short->sfb_cnt, psy_data[0]->sfb_energy_ms.short_nrg[w],
        &psy_data[0]->sfb_energy_sum_ms.short_nrg[w], psy_data[1]->sfb_energy_ms.short_nrg[w],
        &psy_data[1]->sfb_energy_sum_ms.short_nrg[w]);
  }
}

static IA_ERRORCODE ia_enhaacplus_enc_advance_psy_short(
    ixheaace_psy_data *pstr_psy_data, ixheaace_temporal_noise_shaping_data *pstr_tns_data,
    ixheaace_psy_configuration_short *pstr_psy_conf_short,
    ixheaace_psy_out_channel *pstr_psy_out_ch, FLOAT32 *ptr_tns_scratch,
    const ixheaace_temporal_noise_shaping_data *ptr_tns_data2, const WORD32 ch, WORD32 aot,
    FLOAT32 *ptr_shared_buffer1, ixheaace_aac_tables *pstr_aac_tables, WORD32 frame_len_long) {
  IA_ERRORCODE error_code = IA_NO_ERROR;
  WORD32 w;
  FLOAT32 energy_shift = 0.25f;
  FLOAT32 clip_energy = pstr_psy_conf_short->clip_energy * energy_shift;

  for (w = 0; w < TRANS_FAC; w++) {
    WORD32 w_offset = w * frame_len_long;
    WORD32 i, offset;
    FLOAT32 *ptr_mdct =
        &pstr_psy_data->ptr_spec_coeffs[pstr_psy_conf_short->lowpass_line + w_offset];

    /* Low pass */
    offset = frame_len_long - pstr_psy_conf_short->lowpass_line;
    memset(ptr_mdct, 0, sizeof(FLOAT32) * offset);

    /* Calc sfb-bandwise MDCT-energies for left and right channel */
    ia_enhaacplus_enc_calc_band_energy(
        pstr_psy_data->ptr_spec_coeffs + w_offset, pstr_psy_conf_short->sfb_offsets,
        pstr_psy_conf_short->sfb_active, pstr_psy_data->sfb_energy.short_nrg[w],
        pstr_psy_conf_short->sfb_cnt, &pstr_psy_data->sfb_energy_sum.short_nrg[w]);

    /* TNS Detect*/
    error_code = ia_enhaacplus_enc_tns_detect(
        pstr_tns_data, pstr_psy_conf_short->str_tns_conf, ptr_tns_scratch,
        pstr_psy_conf_short->sfb_offsets, pstr_psy_data->ptr_spec_coeffs + w_offset, w,
        pstr_psy_data->blk_switch_cntrl.win_seq, aot, pstr_psy_data->sfb_energy.short_nrg[w],
        ptr_shared_buffer1, frame_len_long);

    if (error_code != IA_NO_ERROR) {
      return error_code;
    }

    /*  TNS Sync */
    if (ch == 1) {
      ia_enhaacplus_enc_tns_sync(pstr_tns_data, ptr_tns_data2, pstr_psy_conf_short->str_tns_conf,
                                 w, (WORD32)pstr_psy_data->blk_switch_cntrl.win_seq);
    }

    /* TNS Encode */
    ia_enhaacplus_enc_tns_encode(
        &pstr_psy_out_ch->tns_info, pstr_tns_data, pstr_psy_conf_short->sfb_cnt,
        pstr_psy_conf_short->str_tns_conf, pstr_psy_conf_short->lowpass_line,
        pstr_psy_data->ptr_spec_coeffs + w_offset, w, pstr_psy_data->blk_switch_cntrl.win_seq,
        pstr_aac_tables->pstr_tns_tab);

    for (i = 0; i < pstr_psy_conf_short->sfb_cnt; i++) {
      pstr_psy_data->sfb_threshold.short_nrg[w][i] =
          pstr_psy_data->sfb_energy.short_nrg[w][i] * pstr_psy_conf_short->ratio_float;

      pstr_psy_data->sfb_threshold.short_nrg[w][i] =
          MIN(pstr_psy_data->sfb_threshold.short_nrg[w][i], clip_energy);
    }

    /* Calc sfb-bandwise MDCT-energies for left and right channel again, if TNS has modified the
     * spectrum */
    if (pstr_psy_out_ch->tns_info.tns_active[w]) {
      ia_enhaacplus_enc_calc_band_energy(
          pstr_psy_data->ptr_spec_coeffs + w_offset, pstr_psy_conf_short->sfb_offsets,
          pstr_psy_conf_short->sfb_active, pstr_psy_data->sfb_energy.short_nrg[w],
          pstr_psy_conf_short->sfb_cnt, &pstr_psy_data->sfb_energy_sum.short_nrg[w]);
    }

    /* Spreading */
    ia_enhaacplus_enc_spreading_max(
        pstr_psy_conf_short->sfb_cnt, pstr_psy_conf_short->sfb_mask_low_factor,
        pstr_psy_conf_short->sfb_mask_high_factor, pstr_psy_data->sfb_threshold.short_nrg[w]);

    for (i = 0; i < pstr_psy_conf_short->sfb_cnt; i++) {
      pstr_psy_data->sfb_threshold.short_nrg[w][i] =
          MAX(pstr_psy_data->sfb_threshold.short_nrg[w][i],
              (pstr_psy_conf_short->sfb_threshold_quiet[i] * energy_shift));
    }

    /* Pre-echo Control */
    ia_enhaacplus_enc_pre_echo_control(pstr_psy_data->sfb_threshold_nm1_float,
                                       pstr_psy_conf_short->sfb_cnt,
                                       pstr_psy_conf_short->min_remaining_threshold_factor,
                                       pstr_psy_data->sfb_threshold.short_nrg[w]);

    /* Apply TNS mult table on CB thresholds */
    if (pstr_psy_out_ch->tns_info.tns_active[w]) {
      ia_enhaacplus_enc_apply_tns_mult_table_to_ratios(
          pstr_psy_conf_short->str_tns_conf.tns_ratio_patch_lowest_cb,
          pstr_psy_conf_short->str_tns_conf.tns_start_band,
          pstr_psy_data->sfb_threshold.short_nrg[w]);
    }

    /* Spreaded energy for avoid hole detection */
    for (i = 0; i < pstr_psy_conf_short->sfb_cnt; i++) {
      pstr_psy_data->sfb_sreaded_energy.short_nrg[w][i] =
          pstr_psy_data->sfb_energy.short_nrg[w][i];
    }

    ia_enhaacplus_enc_spreading_max(pstr_psy_conf_short->sfb_cnt,
                                    pstr_psy_conf_short->sfb_mask_low_factor_spread_nrg,
                                    pstr_psy_conf_short->sfb_mask_high_factor_spread_nrg,
                                    pstr_psy_data->sfb_sreaded_energy.short_nrg[w]);
  }
  return error_code;
}

static IA_ERRORCODE ia_enhaacplus_enc_advance_psy_long(
    ixheaace_psy_data *pstr_psy_data, ixheaace_temporal_noise_shaping_data *pstr_tns_data,
    ixheaace_psy_configuration_long *pstr_psy_conf_long,
    ixheaace_psy_out_channel *pstr_psy_out_ch, FLOAT32 *ptr_scratch_tns,
    const ixheaace_temporal_noise_shaping_data *pstr_tns_data2, const WORD32 ch, WORD32 aot,
    FLOAT32 *ptr_shared_buffer1, ixheaace_aac_tables *pstr_aac_tables, WORD32 frame_len_long) {
  WORD32 i;
  IA_ERRORCODE error_code = IA_NO_ERROR;
  FLOAT32 energy_shift = 0.25f;
  FLOAT32 clip_energy = pstr_psy_conf_long->clip_energy / 4;
  FLOAT32 *ptr_sfb_energy_long = pstr_psy_data->sfb_energy.long_nrg;
  FLOAT32 *ptr_sfb_spreaded_energy = pstr_psy_data->sfb_sreaded_energy.long_nrg;

  /* Low pass */
  memset(&pstr_psy_data->ptr_spec_coeffs[pstr_psy_conf_long->lowpass_line], 0,
         sizeof(*pstr_psy_data->ptr_spec_coeffs) *
             (frame_len_long - pstr_psy_conf_long->lowpass_line));

  /* Calculate scale_factor_band - bandwise MDCT-energies for left and right channels */
  ia_enhaacplus_enc_calc_band_energy(
      pstr_psy_data->ptr_spec_coeffs, pstr_psy_conf_long->sfb_offsets,
      pstr_psy_conf_long->sfb_active, pstr_psy_data->sfb_energy.long_nrg,
      pstr_psy_conf_long->sfb_cnt, &pstr_psy_data->sfb_energy_sum.long_nrg);

  /* TNS Detect */
  error_code = ia_enhaacplus_enc_tns_detect(
      pstr_tns_data, pstr_psy_conf_long->str_tns_conf, ptr_scratch_tns,
      pstr_psy_conf_long->sfb_offsets, pstr_psy_data->ptr_spec_coeffs, 0,
      pstr_psy_data->blk_switch_cntrl.win_seq, aot, pstr_psy_data->sfb_energy.long_nrg,
      ptr_shared_buffer1, frame_len_long);

  if (error_code != IA_NO_ERROR) {
    return error_code;
  }

  /* TNS Sync */
  if (ch == 1) {
    ia_enhaacplus_enc_tns_sync(pstr_tns_data, pstr_tns_data2, pstr_psy_conf_long->str_tns_conf, 0,
                               (WORD32)pstr_psy_data->blk_switch_cntrl.win_seq);
  }

  /* TNS Encode */
  ia_enhaacplus_enc_tns_encode(&pstr_psy_out_ch->tns_info, pstr_tns_data,
                               pstr_psy_conf_long->sfb_cnt, pstr_psy_conf_long->str_tns_conf,
                               pstr_psy_conf_long->lowpass_line, pstr_psy_data->ptr_spec_coeffs,
                               0, (WORD32)pstr_psy_data->blk_switch_cntrl.win_seq,
                               pstr_aac_tables->pstr_tns_tab);

  if (aot == AOT_AAC_ELD) {
    for (i = 0; i < pstr_psy_conf_long->sfb_active; i++) {
      pstr_psy_data->sfb_threshold.long_nrg[i] =
          pstr_psy_data->sfb_energy.long_nrg[i] * pstr_psy_conf_long->ratio_float;

      pstr_psy_data->sfb_threshold.long_nrg[i] =
          MIN(pstr_psy_data->sfb_threshold.long_nrg[i], clip_energy);
    }
  } else {
    for (i = 0; i < pstr_psy_conf_long->sfb_cnt; i++) {
      pstr_psy_data->sfb_threshold.long_nrg[i] =
          pstr_psy_data->sfb_energy.long_nrg[i] * pstr_psy_conf_long->ratio_float;

      pstr_psy_data->sfb_threshold.long_nrg[i] =
          MIN(pstr_psy_data->sfb_threshold.long_nrg[i], clip_energy);
    }
  }

  /* Calculate scale factor band - bandwise MDCT-energies for left and right channel again, if TNS
   * has modified the spectrum */
  if (pstr_psy_out_ch->tns_info.tns_active[0] == 1) {
    ia_enhaacplus_enc_calc_band_energy(
        pstr_psy_data->ptr_spec_coeffs, pstr_psy_conf_long->sfb_offsets,
        pstr_psy_conf_long->sfb_active, pstr_psy_data->sfb_energy.long_nrg,
        pstr_psy_conf_long->sfb_cnt, &pstr_psy_data->sfb_energy_sum.long_nrg);
  }

  /* Spreading */
  if (aot == AOT_AAC_ELD) {
    ia_enhaacplus_enc_spreading_max(
        pstr_psy_conf_long->sfb_active, pstr_psy_conf_long->sfb_mask_low_factor,
        pstr_psy_conf_long->sfb_mask_high_factor, pstr_psy_data->sfb_threshold.long_nrg);
  } else {
    ia_enhaacplus_enc_spreading_max(
        pstr_psy_conf_long->sfb_cnt, pstr_psy_conf_long->sfb_mask_low_factor,
        pstr_psy_conf_long->sfb_mask_high_factor, pstr_psy_data->sfb_threshold.long_nrg);
  }

  /* Threshold in quiet */
  if (aot == AOT_AAC_ELD) {
    for (i = 0; i < pstr_psy_conf_long->sfb_active; i++) {
      pstr_psy_data->sfb_threshold.long_nrg[i] =
          MAX(pstr_psy_data->sfb_threshold.long_nrg[i],
              (pstr_psy_conf_long->sfb_threshold_quiet[i] * energy_shift));
    }
  } else {
    for (i = 0; i < pstr_psy_conf_long->sfb_cnt; i++) {
      pstr_psy_data->sfb_threshold.long_nrg[i] =
          MAX(pstr_psy_data->sfb_threshold.long_nrg[i],
              (pstr_psy_conf_long->sfb_threshold_quiet[i] * energy_shift));
    }
  }

  /* Pre-echo control */
  if (pstr_psy_data->blk_switch_cntrl.win_seq == STOP_WINDOW) {
    /* Prevent pre-echo control from comparing stop thresholds with short thresholds */
    for (i = 0; i < pstr_psy_conf_long->sfb_cnt; i++) {
      pstr_psy_data->sfb_threshold_nm1_float[i] = 1.0e20f;
    }
  }

  ia_enhaacplus_enc_pre_echo_control(
      pstr_psy_data->sfb_threshold_nm1_float, pstr_psy_conf_long->sfb_cnt,
      pstr_psy_conf_long->min_remaining_threshold_factor, pstr_psy_data->sfb_threshold.long_nrg);

  if (pstr_psy_data->blk_switch_cntrl.win_seq == START_WINDOW) {
    /* Prevent pre-echo control in next frame from comparing start thresholds with short
     * thresholds */
    for (i = 0; i < pstr_psy_conf_long->sfb_cnt; i++) {
      pstr_psy_data->sfb_threshold_nm1_float[i] = 1.0e20f;
    }
  }

  /* Apply TNS mult table on CB thresholds */
  if (pstr_psy_out_ch->tns_info.tns_active[0]) {
    ia_enhaacplus_enc_apply_tns_mult_table_to_ratios(
        pstr_psy_conf_long->str_tns_conf.tns_ratio_patch_lowest_cb,
        pstr_psy_conf_long->str_tns_conf.tns_start_band, pstr_psy_data->sfb_threshold.long_nrg);
  }

  /* Spreaded energy for avoid hole detection */

  memcpy(&ptr_sfb_spreaded_energy[0], &ptr_sfb_energy_long[0],
         sizeof(ptr_sfb_spreaded_energy[0]) * pstr_psy_conf_long->sfb_cnt);

  if (aot == AOT_AAC_ELD) {
    ia_enhaacplus_enc_spreading_max(pstr_psy_conf_long->sfb_active,
                                    pstr_psy_conf_long->sfb_mask_low_factor_spread_nrg,
                                    pstr_psy_conf_long->sfb_mask_high_factor_spread_nrg,
                                    pstr_psy_data->sfb_sreaded_energy.long_nrg);
  } else {
    ia_enhaacplus_enc_spreading_max(pstr_psy_conf_long->sfb_cnt,
                                    pstr_psy_conf_long->sfb_mask_low_factor_spread_nrg,
                                    pstr_psy_conf_long->sfb_mask_high_factor_spread_nrg,
                                    pstr_psy_data->sfb_sreaded_energy.long_nrg);
  }

  return error_code;
}

IA_ERRORCODE ia_enhaacplus_enc_psy_main(
    WORD32 time_sn_stride, ixheaace_element_info *pstr_elem_info, const FLOAT32 *ptr_time_signal,
    WORD32 aot, ixheaace_psy_data **psy_data,
    ixheaace_temporal_noise_shaping_data **tns_data,
    ixheaace_psy_configuration_long *pstr_psy_conf_long,
    ixheaace_psy_configuration_short *pstr_psy_conf_short,
    ixheaace_psy_out_channel **psy_out_ch,
    ixheaace_psy_out_element *pstr_psy_out_element, FLOAT32 *ptr_scratch_tns,
    FLOAT32 *ptr_shared_buffer1, WORD8 *ptr_shared_buffer5, ixheaace_aac_tables *pstr_aac_tables,
    WORD32 frame_len_long)

{
  IA_ERRORCODE error_code = IA_NO_ERROR;
  WORD32 grouped_sfb_offset[IXHEAACE_MAX_CH_IN_BS_ELE][MAXIMUM_GROUPED_SCALE_FACTOR_BAND +
                                                       1]; /* plus one for last dummy offset ! */
  FLOAT32 grouped_sfb_min_snr[IXHEAACE_MAX_CH_IN_BS_ELE][MAXIMUM_GROUPED_SCALE_FACTOR_BAND];
  WORD32 max_sfb_per_grp[IXHEAACE_MAX_CH_IN_BS_ELE] = {0};
  WORD32 ch, sfb, line;
  WORD32 num_channels = pstr_elem_info->n_channels_in_el;

  if (aot == AOT_AAC_LC || aot == AOT_SBR || aot == AOT_PS) {
    if (pstr_elem_info->el_type != ID_LFE) {
      for (ch = 0; ch < num_channels; ch++) {
        iaace_block_switching(&psy_data[ch]->blk_switch_cntrl,
                              ptr_time_signal + pstr_elem_info->channel_index[ch], frame_len_long,
                              num_channels);
      }
    } else {
      for (ch = 0; ch < num_channels; ch++) {
        psy_data[ch]->blk_switch_cntrl.win_seq = LONG_WINDOW;
      }
    }

    /* synch left and right block type */
    if (num_channels == NUM_CHANS_MONO) {
      iaace_sync_block_switching(&psy_data[0]->blk_switch_cntrl, NULL, num_channels);
    }
    else {
      iaace_sync_block_switching(&psy_data[0]->blk_switch_cntrl, &psy_data[1]->blk_switch_cntrl,
                                 num_channels);
    }
  } else if (aot == AOT_AAC_LD || aot == AOT_AAC_ELD) {
    for (ch = 0; ch < num_channels; ch++) {
      psy_data[ch]->blk_switch_cntrl.win_seq_ld = LONG_WINDOW;
      psy_data[ch]->blk_switch_cntrl.next_win_seq_ld = LONG_WINDOW;
      psy_data[ch]->blk_switch_cntrl.win_seq = LONG_WINDOW;
      psy_data[ch]->blk_switch_cntrl.nxt_win_seq = LONG_WINDOW;
      psy_data[ch]->blk_switch_cntrl.total_groups_cnt = 1;
    }
  }
  /* transform */
  for (ch = 0; ch < num_channels; ch++) {
    if (aot == AOT_AAC_LC || aot == AOT_SBR || aot == AOT_PS) {
      ixheaace_transform_real_lc_ld(
          psy_data[ch]->ptr_mdct_delay_buf, ptr_time_signal + pstr_elem_info->channel_index[ch],
          time_sn_stride, psy_data[ch]->ptr_spec_coeffs, psy_data[ch]->blk_switch_cntrl.win_seq,
          frame_len_long, ptr_shared_buffer5);
    } else if (aot == AOT_AAC_LD) {
      if (frame_len_long == FRAME_LEN_480) {
        ia_enhaacplus_enc_transform_real(
            psy_data[ch]->ptr_mdct_delay_buf, ptr_time_signal + pstr_elem_info->channel_index[ch],
            time_sn_stride, psy_data[ch]->ptr_spec_coeffs, pstr_aac_tables->pstr_mdct_tab,
            ptr_scratch_tns, ptr_shared_buffer5, frame_len_long);
      } else {
        ixheaace_transform_real_lc_ld(
            psy_data[ch]->ptr_mdct_delay_buf, ptr_time_signal + pstr_elem_info->channel_index[ch],
            time_sn_stride, psy_data[ch]->ptr_spec_coeffs, psy_data[ch]->blk_switch_cntrl.win_seq,
            frame_len_long, ptr_shared_buffer5);
      }
    } else if (aot == AOT_AAC_ELD) {
      if (frame_len_long == FRAME_LEN_480) {
        ia_enhaacplus_enc_transform_real(
            psy_data[ch]->ptr_mdct_delay_buf, ptr_time_signal + pstr_elem_info->channel_index[ch],
            time_sn_stride, psy_data[ch]->ptr_spec_coeffs, pstr_aac_tables->pstr_mdct_tab,
            ptr_scratch_tns, ptr_shared_buffer5, frame_len_long);
      } else {
        ia_enhaacplus_enc_transform_real_eld(
            psy_data[ch]->ptr_mdct_delay_buf, ptr_time_signal + pstr_elem_info->channel_index[ch],
            time_sn_stride, psy_data[ch]->ptr_spec_coeffs, ptr_shared_buffer5, frame_len_long);
      }
    }
  }

  for (ch = 0; ch < num_channels; ch++) {
    if (psy_data[ch]->blk_switch_cntrl.win_seq != SHORT_WINDOW) {
      error_code = ia_enhaacplus_enc_advance_psy_long(
          psy_data[ch], tns_data[ch], pstr_psy_conf_long, psy_out_ch[ch], ptr_scratch_tns,
          tns_data[1 - ch], ch, aot, ptr_shared_buffer1, pstr_aac_tables, frame_len_long);

      if (error_code != IA_NO_ERROR) {
        return error_code;
      }

      for (sfb = pstr_psy_conf_long->sfb_cnt - 1; sfb >= 0; sfb--) {
        for (line = pstr_psy_conf_long->sfb_offsets[sfb + 1] - 1;
             line >= pstr_psy_conf_long->sfb_offsets[sfb]; line--) {
          if (psy_data[ch]->ptr_spec_coeffs[line] != 0) {
            break;
          }
        }
        if (line >= pstr_psy_conf_long->sfb_offsets[sfb]) {
          break;
        }
      }

      max_sfb_per_grp[ch] = sfb + 1;

      /* Calculate bandwise energies for mid and side channels - only if 2 channels exist */
      if (ch == 1) {
        ia_enhaacplus_enc_advance_psy_long_ms(psy_data, pstr_psy_conf_long);
      }
    } else {
      error_code = ia_enhaacplus_enc_advance_psy_short(
          psy_data[ch], tns_data[ch], pstr_psy_conf_short, psy_out_ch[ch], ptr_scratch_tns,
          tns_data[1 - ch], ch, aot, ptr_shared_buffer1, pstr_aac_tables, frame_len_long / 8);

      if (error_code != IA_NO_ERROR) {
        return error_code;
      }

      /* Calculate bandwise energies for mid and side channels - only if 2 channels exist */
      if (ch == 1) {
        ia_enhaacplus_enc_advance_psy_short_ms(psy_data, pstr_psy_conf_short, frame_len_long);
      }
    }
  }

  /* Group short data (max_sfb for short blocks is determined here) */
  if (aot == AOT_AAC_LC || aot == AOT_SBR || aot == AOT_PS) {
    for (ch = 0; ch < num_channels; ch++) {
      if (psy_data[ch]->blk_switch_cntrl.win_seq == SHORT_WINDOW) {
        iaace_group_short_data(psy_data[ch]->ptr_spec_coeffs, ptr_scratch_tns,
                               &psy_data[ch]->sfb_threshold, &psy_data[ch]->sfb_energy,
                               &psy_data[ch]->sfb_energy_ms, &psy_data[ch]->sfb_sreaded_energy,
                               pstr_psy_conf_short->sfb_cnt, pstr_psy_conf_short->sfb_offsets,
                               pstr_psy_conf_short->sfb_min_snr, grouped_sfb_offset[ch],
                               &max_sfb_per_grp[ch], grouped_sfb_min_snr[ch],
                               psy_data[ch]->blk_switch_cntrl.total_groups_cnt,
                               psy_data[ch]->blk_switch_cntrl.group_len, frame_len_long);
      }
    }
  }

#if (IXHEAACE_MAX_CH_IN_BS_ELE > 1)
  /* Stereo Processing */
  if (num_channels == 2) {
    pstr_psy_out_element->tools_info.ms_digest = MS_NONE;

    max_sfb_per_grp[0] = max_sfb_per_grp[1] = MAX(max_sfb_per_grp[0], max_sfb_per_grp[1]);

    if (psy_data[0]->blk_switch_cntrl.win_seq != SHORT_WINDOW) {
      iaace_ms_apply(
          psy_data, psy_data[0]->ptr_spec_coeffs, psy_data[1]->ptr_spec_coeffs,
          &pstr_psy_out_element->tools_info.ms_digest, pstr_psy_out_element->tools_info.ms_mask,
          pstr_psy_conf_long->sfb_cnt, pstr_psy_conf_long->sfb_cnt, max_sfb_per_grp[0],
          pstr_psy_conf_long->sfb_offsets, &pstr_psy_out_element->weight_ms_lr_pe_ratio);
    } else {
      iaace_ms_apply(psy_data, psy_data[0]->ptr_spec_coeffs, psy_data[1]->ptr_spec_coeffs,
                     &pstr_psy_out_element->tools_info.ms_digest,
                     pstr_psy_out_element->tools_info.ms_mask,
                     psy_data[0]->blk_switch_cntrl.total_groups_cnt *
                     pstr_psy_conf_short->sfb_cnt,
                     pstr_psy_conf_short->sfb_cnt, max_sfb_per_grp[0], grouped_sfb_offset[0],
                     &pstr_psy_out_element->weight_ms_lr_pe_ratio);
    }
  }
#endif

  /* Build output */
  if (aot == AOT_AAC_LC || aot == AOT_SBR || aot == AOT_PS) {
    for (ch = 0; ch < num_channels; ch++) {
      if (psy_data[ch]->blk_switch_cntrl.win_seq != SHORT_WINDOW) {
        ia_enhaacplus_enc_build_interface(
            psy_data[ch]->ptr_spec_coeffs, &psy_data[ch]->sfb_threshold,
            &psy_data[ch]->sfb_energy,
            &psy_data[ch]->sfb_sreaded_energy, psy_data[ch]->sfb_energy_sum,
            psy_data[ch]->sfb_energy_sum_ms, psy_data[ch]->blk_switch_cntrl.win_seq,
            ia_enhaacplus_enc_block_type_to_window_shape_lc[psy_data[ch]
                                                                ->blk_switch_cntrl.win_seq],
            pstr_psy_conf_long->sfb_cnt, pstr_psy_conf_long->sfb_offsets, max_sfb_per_grp[ch],
            pstr_psy_conf_long->sfb_min_snr, psy_data[ch]->blk_switch_cntrl.total_groups_cnt,
            psy_data[ch]->blk_switch_cntrl.group_len, psy_out_ch[ch]);
      } else {
        ia_enhaacplus_enc_build_interface(
            psy_data[ch]->ptr_spec_coeffs, &psy_data[ch]->sfb_threshold,
            &psy_data[ch]->sfb_energy,
            &psy_data[ch]->sfb_sreaded_energy, psy_data[ch]->sfb_energy_sum,
            psy_data[ch]->sfb_energy_sum_ms, SHORT_WINDOW, SINE_WINDOW,
            psy_data[ch]->blk_switch_cntrl.total_groups_cnt * pstr_psy_conf_short->sfb_cnt,
            grouped_sfb_offset[ch], max_sfb_per_grp[ch], grouped_sfb_min_snr[ch],
            psy_data[ch]->blk_switch_cntrl.total_groups_cnt,
            psy_data[ch]->blk_switch_cntrl.group_len, psy_out_ch[ch]);
      }
    }
  } else if (aot == AOT_AAC_LD || aot == AOT_AAC_ELD) {
    for (ch = 0; ch < num_channels; ch++) {
      ia_enhaacplus_enc_build_interface(
          psy_data[ch]->ptr_spec_coeffs, &psy_data[ch]->sfb_threshold, &psy_data[ch]->sfb_energy,
          &psy_data[ch]->sfb_sreaded_energy, psy_data[ch]->sfb_energy_sum,
          psy_data[ch]->sfb_energy_sum_ms, LONG_WINDOW,
          ia_enhaacplus_enc_block_type_to_window_shape_ld[psy_data[ch]
                                                              ->blk_switch_cntrl.win_seq_ld],
          ((aot == AOT_AAC_ELD) ? pstr_psy_conf_long->sfb_active : pstr_psy_conf_long->sfb_cnt),
          pstr_psy_conf_long->sfb_offsets, max_sfb_per_grp[ch], pstr_psy_conf_long->sfb_min_snr,
          psy_data[ch]->blk_switch_cntrl.total_groups_cnt,
          psy_data[ch]->blk_switch_cntrl.group_len, psy_out_ch[ch]);
    }
  }
  return error_code;
}
