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
#include <stdlib.h>
#include <math.h>
#include <string.h>
#include "impd_type_def.h"
#include "impd_error_standards.h"
#include <string.h>
#include "impd_drc_extr_delta_coded_info.h"
#include "ixheaac_constants.h"
#include "impd_drc_common.h"
#include "impd_drc_struct.h"
#include "impd_drc_interface.h"
#include "impd_memory_standards.h"
#include "impd_drc_peak_limiter.h"
#include "impd_drc_bitbuffer.h"
#include "impd_drc_bitstream_dec_api.h"
#include "impd_drc_gain_dec.h"
#include "impd_drc_filter_bank.h"
#include "impd_drc_multi_band.h"
#include "impd_drc_process_audio.h"
#include "impd_parametric_drc_dec.h"
#include "impd_drc_eq.h"
#include "impd_drc_gain_decoder.h"
#include "impd_drc_selection_process.h"
#include "impd_drc_api_struct_def.h"
#include "impd_drc_peak_limiter.h"

#define PARAMETRIC_DRC_DELAY_MAX_DEFAULT 4096
#define EQ_DELAY_MAX_DEFAULT 256

IA_ERRORCODE impd_drc_mem_api(ia_drc_api_struct *p_obj_drc, WORD32 iCmd,
                              WORD32 iIdx, pVOID pvValue);

IA_ERRORCODE impd_drc_fill_mem_tables(ia_drc_api_struct *p_obj_drc);

WORD32
impd_drc_dec_interface_process(ia_bit_buf_struct *it_bit_buff,
                               ia_drc_interface_struct *pstr_drc_interface,
                               UWORD8 *it_bit_buf, WORD32 num_bit_stream_bits,
                               WORD32 *num_bits_read);

WORD32
impd_drc_dec_interface_add_effect_type(ia_drc_interface_struct *pstr_drc_interface,
                                       WORD32 drc_effect_type, WORD32 target_loudness,
                                       WORD32 loud_norm, WORD32 album_mode, FLOAT32 boost,
                                       FLOAT32 compress
#ifdef LOUDNESS_LEVELING_SUPPORT
                                       ,
                                       WORD32 loudness_leveling_flag
#endif
);

#define BITSTREAM_FILE_FORMAT_SPLIT 1
#define LIM_DEFAULT_THRESHOLD (0.89125094f)

static WORD32 impd_match_downmix(WORD32 downmix_id, WORD32 dec_downmix_id) {
  WORD32 id_match = 0;

  switch (dec_downmix_id) {
    case 0:
      id_match = (downmix_id == 0);
      break;
    case 1:
      id_match = ((downmix_id == 0) || (downmix_id == 0x7F));
      break;
    case 2:
      id_match = (downmix_id == 0x7F);
      break;
    case 3:
      id_match = ((downmix_id != 0) && (downmix_id != 0x7F));
      break;
    case 4:
      id_match = (downmix_id != 0);
      break;
  }
  return id_match;
}

IA_ERRORCODE impd_drc_set_default_config(ia_drc_api_struct *p_obj_drc) {
  memset(p_obj_drc, 0, sizeof(*p_obj_drc));
  p_obj_drc->str_config.bitstream_file_format = 1;
  p_obj_drc->str_config.dec_type = 0;
  p_obj_drc->str_config.sub_band_domain_mode = 0;
  p_obj_drc->str_config.sub_band_count = 0;
  p_obj_drc->str_config.sub_band_down_sampling_factor = 0;
  p_obj_drc->str_config.sampling_rate = 0;
  p_obj_drc->str_config.frame_size = 1024;
  p_obj_drc->str_config.num_ch_in = -1;
  p_obj_drc->str_config.num_ch_out = -1;
  p_obj_drc->str_config.control_parameter_index = -1;
  p_obj_drc->str_config.peak_limiter = 0;
  p_obj_drc->str_config.delay_mode = 0;
  p_obj_drc->str_config.interface_bitstream_present = 1;
  p_obj_drc->str_config.gain_delay_samples = 0;
  p_obj_drc->str_config.absorb_delay_on = 1;
  p_obj_drc->str_config.subband_domain_io_flag = 0;
  p_obj_drc->str_bit_handler.gain_stream_flag = 1;
  p_obj_drc->str_config.constant_delay_on = 0;
  p_obj_drc->str_config.audio_delay_samples = 0;
  p_obj_drc->str_config.effect_type = 0;
  p_obj_drc->str_config.target_loudness = -24;
  p_obj_drc->str_config.loud_norm_flag = 0;
  p_obj_drc->str_config.album_mode = 0;
  p_obj_drc->str_config.boost = 1.0f;
  p_obj_drc->str_config.compress = 1.0f;
  p_obj_drc->str_config.boost_set = 0;
  p_obj_drc->str_config.compress_set = 0;
  memset(&p_obj_drc->str_bit_handler, 0, sizeof(p_obj_drc->str_bit_handler));

  return IA_NO_ERROR;
}

IA_ERRORCODE impd_drc_set_default_bitstream_config(
    ia_drc_config *pstr_drc_config) {
  WORD32 i;

  pstr_drc_config->sample_rate_present = 0;
  pstr_drc_config->sampling_rate = 0;
  pstr_drc_config->dwnmix_instructions_count = 0;
  pstr_drc_config->drc_coefficients_drc_count = 1;
  pstr_drc_config->drc_instructions_uni_drc_count = 4;
  pstr_drc_config->drc_instructions_count_plus = 5;
  pstr_drc_config->drc_description_basic_present = 0;
  pstr_drc_config->drc_coefficients_basic_count = 0;
  pstr_drc_config->drc_instructions_basic_count = 0;
  pstr_drc_config->drc_config_ext_present = 1;
  pstr_drc_config->apply_drc = 0;
  pstr_drc_config->str_drc_config_ext.drc_config_ext_type[0] = 2;
  pstr_drc_config->str_drc_config_ext.ext_bit_size[0] = 345;
  pstr_drc_config->str_drc_config_ext.parametric_drc_present = 0;
  pstr_drc_config->str_drc_config_ext.drc_extension_v1_present = 1;
  pstr_drc_config->str_p_loc_drc_coefficients_uni_drc[0].version = 1;
  pstr_drc_config->str_p_loc_drc_coefficients_uni_drc[0].drc_location = 1;
  pstr_drc_config->str_p_loc_drc_coefficients_uni_drc[0].gain_set_count = 4;
  for (i = 0;
       i <
       pstr_drc_config->str_p_loc_drc_coefficients_uni_drc[0].gain_set_count;
       i++) {
    pstr_drc_config->str_p_loc_drc_coefficients_uni_drc[0]
        .gain_set_params[i]
        .gain_interpolation_type = 1;
    pstr_drc_config->str_p_loc_drc_coefficients_uni_drc[0]
        .gain_set_params[i]
        .band_count = 1;
  }
  pstr_drc_config->str_p_loc_drc_coefficients_uni_drc[0].gain_sequence_count =
      4;
  pstr_drc_config->str_p_loc_drc_coefficients_uni_drc[0]
      .gain_set_params_index_for_gain_sequence[0] = 0;
  pstr_drc_config->str_p_loc_drc_coefficients_uni_drc[0]
      .gain_set_params_index_for_gain_sequence[1] = 1;
  pstr_drc_config->str_p_loc_drc_coefficients_uni_drc[0]
      .gain_set_params_index_for_gain_sequence[2] = 2;
  pstr_drc_config->str_p_loc_drc_coefficients_uni_drc[0]
      .gain_set_params_index_for_gain_sequence[3] = 3;
  pstr_drc_config->str_p_loc_drc_coefficients_uni_drc[0].gain_set_count_plus =
      4;
  pstr_drc_config->str_drc_instruction_str[0].drc_set_id = 1;
  pstr_drc_config->str_drc_instruction_str[0].drc_set_complexity_level = 2;
  pstr_drc_config->str_drc_instruction_str[0].drc_location = 1;
  pstr_drc_config->str_drc_instruction_str[0].dwnmix_id_count = 1;
  pstr_drc_config->str_drc_instruction_str[0].drc_set_effect = 1;
  pstr_drc_config->str_drc_instruction_str[0].gain_set_index[1] = 1;
  pstr_drc_config->str_drc_instruction_str[0]
      .drc_set_target_loudness_value_lower = -63;
  pstr_drc_config->str_drc_instruction_str[0].num_drc_ch_groups = 2;
  pstr_drc_config->str_drc_instruction_str[0]
      .gain_set_index_for_channel_group[1] = 1;
  pstr_drc_config->str_drc_instruction_str[0].band_count_of_ch_group[0] = 1;
  pstr_drc_config->str_drc_instruction_str[0].band_count_of_ch_group[1] = 1;
  pstr_drc_config->str_drc_instruction_str[0]
      .gain_interpolation_type_for_channel_group[0] = 1;
  pstr_drc_config->str_drc_instruction_str[0]
      .gain_interpolation_type_for_channel_group[1] = 1;
  pstr_drc_config->str_drc_instruction_str[0]
      .time_delta_min_for_channel_group[0] = 32;
  pstr_drc_config->str_drc_instruction_str[0]
      .time_delta_min_for_channel_group[1] = 32;
  pstr_drc_config->str_drc_instruction_str[0].channel_group_of_ch[1] = 1;
  pstr_drc_config->str_drc_instruction_str[0].gain_element_count = 2;

  pstr_drc_config->str_drc_instruction_str[1].drc_set_id = 2;
  pstr_drc_config->str_drc_instruction_str[1].drc_set_complexity_level = 2;
  pstr_drc_config->str_drc_instruction_str[1].drc_location = 1;
  pstr_drc_config->str_drc_instruction_str[1].dwnmix_id_count = 1;
  pstr_drc_config->str_drc_instruction_str[1].drc_set_effect = 2;
  pstr_drc_config->str_drc_instruction_str[1].gain_set_index[0] = 1;
  pstr_drc_config->str_drc_instruction_str[1].gain_set_index[1] = 2;
  pstr_drc_config->str_drc_instruction_str[1]
      .drc_set_target_loudness_value_lower = -63;
  pstr_drc_config->str_drc_instruction_str[1].num_drc_ch_groups = 2;
  pstr_drc_config->str_drc_instruction_str[1]
      .gain_set_index_for_channel_group[0] = 1;
  pstr_drc_config->str_drc_instruction_str[1]
      .gain_set_index_for_channel_group[1] = 2;
  pstr_drc_config->str_drc_instruction_str[1].band_count_of_ch_group[0] = 1;
  pstr_drc_config->str_drc_instruction_str[1].band_count_of_ch_group[1] = 1;
  pstr_drc_config->str_drc_instruction_str[1]
      .gain_interpolation_type_for_channel_group[0] = 1;
  pstr_drc_config->str_drc_instruction_str[1]
      .gain_interpolation_type_for_channel_group[1] = 1;
  pstr_drc_config->str_drc_instruction_str[1]
      .time_delta_min_for_channel_group[0] = 32;
  pstr_drc_config->str_drc_instruction_str[1]
      .time_delta_min_for_channel_group[1] = 32;
  pstr_drc_config->str_drc_instruction_str[1].channel_group_of_ch[1] = 1;
  pstr_drc_config->str_drc_instruction_str[1].gain_element_count = 2;

  pstr_drc_config->str_drc_instruction_str[2].drc_set_id = 3;
  pstr_drc_config->str_drc_instruction_str[2].drc_set_complexity_level = 2;
  pstr_drc_config->str_drc_instruction_str[2].drc_location = 1;
  pstr_drc_config->str_drc_instruction_str[2].dwnmix_id_count = 1;
  pstr_drc_config->str_drc_instruction_str[2].drc_set_effect = 4;
  pstr_drc_config->str_drc_instruction_str[2].gain_set_index[0] = 2;
  pstr_drc_config->str_drc_instruction_str[2].gain_set_index[1] = 3;
  pstr_drc_config->str_drc_instruction_str[2]
      .drc_set_target_loudness_value_lower = -63;
  pstr_drc_config->str_drc_instruction_str[2].num_drc_ch_groups = 2;
  pstr_drc_config->str_drc_instruction_str[2]
      .gain_set_index_for_channel_group[0] = 2;
  pstr_drc_config->str_drc_instruction_str[2]
      .gain_set_index_for_channel_group[1] = 3;
  pstr_drc_config->str_drc_instruction_str[2].band_count_of_ch_group[0] = 1;
  pstr_drc_config->str_drc_instruction_str[2].band_count_of_ch_group[1] = 1;
  pstr_drc_config->str_drc_instruction_str[2]
      .gain_interpolation_type_for_channel_group[0] = 1;
  pstr_drc_config->str_drc_instruction_str[2]
      .gain_interpolation_type_for_channel_group[1] = 1;
  pstr_drc_config->str_drc_instruction_str[2]
      .time_delta_min_for_channel_group[0] = 32;
  pstr_drc_config->str_drc_instruction_str[2]
      .time_delta_min_for_channel_group[1] = 32;
  pstr_drc_config->str_drc_instruction_str[2].channel_group_of_ch[1] = 1;
  pstr_drc_config->str_drc_instruction_str[2].gain_element_count = 2;

  pstr_drc_config->str_drc_instruction_str[3].drc_set_id = 4;
  pstr_drc_config->str_drc_instruction_str[3].drc_set_complexity_level = 2;
  pstr_drc_config->str_drc_instruction_str[3].drc_location = 1;
  pstr_drc_config->str_drc_instruction_str[3].dwnmix_id_count = 1;
  pstr_drc_config->str_drc_instruction_str[3].drc_set_effect = 32;
  pstr_drc_config->str_drc_instruction_str[3].gain_set_index[0] = 3;
  pstr_drc_config->str_drc_instruction_str[3].gain_set_index[1] = 0;
  pstr_drc_config->str_drc_instruction_str[3]
      .drc_set_target_loudness_value_lower = -63;
  pstr_drc_config->str_drc_instruction_str[3].num_drc_ch_groups = 2;
  pstr_drc_config->str_drc_instruction_str[3]
      .gain_set_index_for_channel_group[0] = 3;
  pstr_drc_config->str_drc_instruction_str[3]
      .gain_set_index_for_channel_group[1] = 0;
  pstr_drc_config->str_drc_instruction_str[3].band_count_of_ch_group[0] = 1;
  pstr_drc_config->str_drc_instruction_str[3].band_count_of_ch_group[1] = 1;
  pstr_drc_config->str_drc_instruction_str[3]
      .gain_interpolation_type_for_channel_group[0] = 1;
  pstr_drc_config->str_drc_instruction_str[3]
      .gain_interpolation_type_for_channel_group[1] = 1;
  pstr_drc_config->str_drc_instruction_str[3]
      .time_delta_min_for_channel_group[0] = 32;
  pstr_drc_config->str_drc_instruction_str[3]
      .time_delta_min_for_channel_group[1] = 32;
  pstr_drc_config->str_drc_instruction_str[3].channel_group_of_ch[1] = 1;
  pstr_drc_config->str_drc_instruction_str[3].gain_element_count = 2;

  pstr_drc_config->str_drc_instruction_str[4].drc_set_id = -1;
  pstr_drc_config->str_drc_instruction_str[4].dwnmix_id_count = 1;
  pstr_drc_config->channel_layout.base_channel_count = 2;

  return IA_NO_ERROR;
}

IA_ERRORCODE impd_drc_set_struct_pointer(ia_drc_api_struct *p_obj_drc) {
  pUWORD8 persistent_ptr = (pUWORD8)p_obj_drc->p_state->persistent_ptr;

  UWORD64 persistent_size_consumed = 0;
  p_obj_drc->str_payload.pstr_bitstream_dec = (ia_drc_bits_dec_struct *)persistent_ptr;
  persistent_ptr += IXHEAAC_GET_SIZE_ALIGNED(sizeof(ia_drc_bits_dec_struct), BYTE_ALIGN_8);

  p_obj_drc->str_payload.pstr_gain_dec[0] = (ia_drc_gain_dec_struct *)persistent_ptr;
  persistent_ptr += IXHEAAC_GET_SIZE_ALIGNED(sizeof(ia_drc_gain_dec_struct), BYTE_ALIGN_8);

  p_obj_drc->str_payload.pstr_gain_dec[1] = (ia_drc_gain_dec_struct *)persistent_ptr;
  persistent_ptr += IXHEAAC_GET_SIZE_ALIGNED(sizeof(ia_drc_gain_dec_struct), BYTE_ALIGN_8);

  p_obj_drc->str_payload.pstr_loudness_info = (ia_drc_loudness_info_set_struct *)persistent_ptr;
  persistent_ptr +=
      IXHEAAC_GET_SIZE_ALIGNED(sizeof(ia_drc_loudness_info_set_struct), BYTE_ALIGN_8);

  p_obj_drc->str_payload.pstr_drc_gain = (ia_drc_gain_struct *)persistent_ptr;
  persistent_ptr += IXHEAAC_GET_SIZE_ALIGNED(sizeof(ia_drc_gain_struct), BYTE_ALIGN_8);

  p_obj_drc->str_payload.pstr_drc_interface = (ia_drc_interface_struct *)persistent_ptr;
  persistent_ptr += IXHEAAC_GET_SIZE_ALIGNED(sizeof(ia_drc_interface_struct), BYTE_ALIGN_8);

  p_obj_drc->str_payload.pstr_drc_config = (ia_drc_config *)persistent_ptr;
  persistent_ptr += IXHEAAC_GET_SIZE_ALIGNED(sizeof(ia_drc_config), BYTE_ALIGN_8);

  p_obj_drc->str_payload.pstr_selection_proc = (ia_drc_sel_pro_struct *)persistent_ptr;
  persistent_ptr += IXHEAAC_GET_SIZE_ALIGNED(sizeof(ia_drc_sel_pro_struct), BYTE_ALIGN_8);

  p_obj_drc->str_bit_handler.it_bit_buf = (UWORD8 *)persistent_ptr;
  persistent_ptr += IXHEAAC_GET_SIZE_ALIGNED(MAX_DRC_BS_BUF_SIZE, BYTE_ALIGN_8);

  p_obj_drc->str_payload.pstr_drc_sel_proc_params =
      (ia_drc_sel_proc_params_struct *)persistent_ptr;
  persistent_ptr += IXHEAAC_GET_SIZE_ALIGNED(sizeof(ia_drc_sel_proc_params_struct), BYTE_ALIGN_8);

  p_obj_drc->str_payload.pstr_drc_sel_proc_output =
      (ia_drc_sel_proc_output_struct *)persistent_ptr;
  persistent_ptr += IXHEAAC_GET_SIZE_ALIGNED(sizeof(ia_drc_sel_proc_output_struct), BYTE_ALIGN_8);

  p_obj_drc->str_bit_handler.bitstream_drc_config = (UWORD8 *)persistent_ptr;
  persistent_ptr += IXHEAAC_GET_SIZE_ALIGNED(MAX_BS_BUF_SIZE, BYTE_ALIGN_8);

  p_obj_drc->str_bit_handler.bitstream_loudness_info = (UWORD8 *)persistent_ptr;
  persistent_ptr += IXHEAAC_GET_SIZE_ALIGNED(MAX_BS_BUF_SIZE, BYTE_ALIGN_8);

  p_obj_drc->str_bit_handler.bitstream_unidrc_interface = (UWORD8 *)persistent_ptr;
  persistent_ptr += IXHEAAC_GET_SIZE_ALIGNED(MAX_BS_BUF_SIZE, BYTE_ALIGN_8);

  p_obj_drc->str_payload.pstr_peak_limiter = (ia_drc_peak_limiter_struct *)persistent_ptr;
  persistent_ptr += IXHEAAC_GET_SIZE_ALIGNED(sizeof(ia_drc_peak_limiter_struct), BYTE_ALIGN_8);

  p_obj_drc->str_payload.pstr_peak_limiter->buffer = (FLOAT32 *)persistent_ptr;
  persistent_ptr += IXHEAAC_GET_SIZE_ALIGNED(PEAK_LIM_BUF_SIZE, BYTE_ALIGN_8);

  p_obj_drc->str_payload.pstr_qmf_filter = (ia_drc_qmf_filt_struct *)persistent_ptr;
  persistent_ptr += IXHEAAC_GET_SIZE_ALIGNED(sizeof(ia_drc_qmf_filt_struct), BYTE_ALIGN_8);

  p_obj_drc->str_payload.pstr_qmf_filter->ana_buff = (FLOAT64 *)persistent_ptr;
  persistent_ptr += IXHEAAC_GET_SIZE_ALIGNED(ANALY_BUF_SIZE, BYTE_ALIGN_8);

  p_obj_drc->str_payload.pstr_qmf_filter->syn_buff = (FLOAT64 *)persistent_ptr;
  persistent_ptr += IXHEAAC_GET_SIZE_ALIGNED(SYNTH_BUF_SIZE, BYTE_ALIGN_8);

  persistent_size_consumed =
      (UWORD64)(persistent_ptr - (pUWORD8)p_obj_drc->p_state->persistent_ptr);

  if ((UWORD64)p_obj_drc->p_mem_info[IA_MEMTYPE_PERSIST].ui_size <
      persistent_size_consumed)
    return IA_FATAL_ERROR;

  p_obj_drc->p_state->persistent_ptr = (pVOID)persistent_ptr;
  return IA_NO_ERROR;
}

VOID init_qmf_filt_bank(ia_drc_qmf_filt_struct *qmf_filt) {
  WORD32 l, k;

  FLOAT64 gain_ana = 64.0 / QMF_FILT_RESOLUTION;
  FLOAT64 gain_syn = 1.0 / 64.0;
  for (l = 0; l < 2 * QMF_FILT_RESOLUTION; l++) {
    for (k = 0; k < QMF_FILT_RESOLUTION; k++) {
      qmf_filt->syn_tab_real[l][k] =
          gain_syn * cos((0.0245436926) * (k + 0.5) * (2 * l - 255.0));
      qmf_filt->syn_tab_imag[l][k] =
          gain_syn * sin((0.0245436926) * (k + 0.5) * (2 * l - 255.0));
      qmf_filt->ana_tab_real[k][l] =
          gain_ana * cos((0.0245436926) * (k + 0.5) * (2 * l - 1.0));
      qmf_filt->ana_tab_imag[k][l] =
          gain_ana * sin((0.0245436926) * (k + 0.5) * (2 * l - 1.0));
    }
  }
}

IA_ERRORCODE impd_drc_init(ia_drc_api_struct *p_obj_drc) {
  IA_ERRORCODE err_code = IA_NO_ERROR;
  WORD32 i, j;

  pVOID persistent_ptr = p_obj_drc->p_state->persistent_ptr;

  WORD32 decDownmixIdList[NUM_GAIN_DEC_INSTANCES] = {0, 4};

  p_obj_drc->p_state->delay_in_output = 0;
  p_obj_drc->str_payload.pstr_selection_proc->first_frame = 1;

  impd_create_init_bit_buf(&p_obj_drc->str_bit_buf,
                           p_obj_drc->str_bit_handler.it_bit_buf,
                           p_obj_drc->str_bit_handler.num_bytes_bs / 8);

  p_obj_drc->pstr_bit_buf = &p_obj_drc->str_bit_buf;

  err_code = impd_init_drc_bitstream_dec(
      p_obj_drc->str_payload.pstr_bitstream_dec,
      p_obj_drc->str_config.sampling_rate, p_obj_drc->str_config.frame_size,
      p_obj_drc->str_config.delay_mode, -1, 0);
  if (err_code != IA_NO_ERROR) return err_code;

  for (i = 0; i < NUM_GAIN_DEC_INSTANCES; i++) {
    err_code = impd_init_drc_decode(p_obj_drc->str_config.frame_size,
                                    p_obj_drc->str_config.sampling_rate,
                                    p_obj_drc->str_config.gain_delay_samples,
                                    p_obj_drc->str_config.delay_mode,
                                    p_obj_drc->str_config.sub_band_domain_mode,
                                    p_obj_drc->str_payload.pstr_gain_dec[i]);
    if (err_code != IA_NO_ERROR) return err_code;
  }

  err_code = impd_drc_dec_interface_add_effect_type(
      p_obj_drc->str_payload.pstr_drc_interface, p_obj_drc->str_config.effect_type,
      p_obj_drc->str_config.target_loudness, p_obj_drc->str_config.loud_norm_flag,
      p_obj_drc->str_config.album_mode, p_obj_drc->str_config.boost,
      p_obj_drc->str_config.compress
#ifdef LOUDNESS_LEVELING_SUPPORT
      ,
      p_obj_drc->str_config.loudness_leveling_flag
#endif
  );

  if (err_code != IA_NO_ERROR) return err_code;

  err_code = impd_drc_uni_selction_proc_init(
      p_obj_drc->str_payload.pstr_selection_proc, 0,
      p_obj_drc->str_payload.pstr_drc_interface,
      p_obj_drc->str_config.sub_band_domain_mode);
  if (err_code != IA_NO_ERROR) return err_code;

  if (p_obj_drc->str_payload.pstr_drc_interface
          ->loudness_norm_parameter_interface_flag &&
      p_obj_drc->str_payload.pstr_drc_interface->loudness_norm_param_interface
          .peak_limiter) {
    p_obj_drc->str_config.peak_limiter = 1;
  }

  p_obj_drc->str_payload.pstr_loudness_info->loudness_info_album_count = 0;
  p_obj_drc->str_payload.pstr_loudness_info->loudness_info_count = 0;
  p_obj_drc->str_payload.pstr_loudness_info->loudness_info_set_ext_present = 0;
  p_obj_drc->p_state->ui_exe_done = 0;

  err_code = impd_process_drc_bitstream_dec_config(
      p_obj_drc->str_payload.pstr_bitstream_dec, p_obj_drc->pstr_bit_buf,
      p_obj_drc->str_payload.pstr_drc_config,
      &p_obj_drc->str_bit_handler.bitstream_drc_config[0],
      p_obj_drc->str_bit_handler.num_bytes_bs_drc_config);

  if (err_code == 1) {
    memset(p_obj_drc->str_payload.pstr_drc_config, 0, sizeof(ia_drc_config));
    err_code = impd_drc_set_default_bitstream_config(
        p_obj_drc->str_payload.pstr_drc_config);
    p_obj_drc->str_payload.pstr_drc_config->channel_layout.base_channel_count =
        p_obj_drc->str_config.num_ch_in;
  }

  if (err_code != IA_NO_ERROR) return err_code;
  err_code = impd_process_drc_bitstream_dec_loudness_info_set(
      p_obj_drc->pstr_bit_buf, p_obj_drc->str_payload.pstr_loudness_info,
      &p_obj_drc->str_bit_handler.bitstream_loudness_info[0],
      p_obj_drc->str_bit_handler.num_bytes_bs_loudness_info);
  if (err_code != IA_NO_ERROR) return err_code;

  if (p_obj_drc->str_payload.pstr_loudness_info->loudness_info
          ->anchor_loudness_present)
    p_obj_drc->str_payload.pstr_selection_proc->uni_drc_sel_proc_params
        .loudness_measurement_method = METHOD_DEFINITION_ANCHOR_LOUDNESS;

  if (p_obj_drc->str_payload.pstr_loudness_info->loudness_info
          ->expert_loudness_present)
    p_obj_drc->str_payload.pstr_selection_proc->uni_drc_sel_proc_params
        .loudness_measurement_system = USER_MEASUREMENT_SYSTEM_EXPERT_PANEL;

  err_code = impd_drc_uni_sel_proc_process(
      p_obj_drc->str_payload.pstr_selection_proc,
      p_obj_drc->str_payload.pstr_drc_config,
      p_obj_drc->str_payload.pstr_loudness_info,
      p_obj_drc->str_payload.pstr_drc_sel_proc_output);
  if (err_code != IA_NO_ERROR) return err_code;

  for (i = 0; i < NUM_GAIN_DEC_INSTANCES; i++) {
    WORD32 audio_num_chan = 0;
    WORD32 numMatchingDrcSets = 0;
    WORD32 matchingDrcSetIds[3], matchingDownmixIds[3];
    for (j = 0;
         j < p_obj_drc->str_payload.pstr_drc_sel_proc_output->num_sel_drc_sets;
         j++) {
      if (impd_match_downmix(p_obj_drc->str_payload.pstr_drc_sel_proc_output
                                 ->sel_downmix_ids[j],
                             decDownmixIdList[i])) {
        matchingDrcSetIds[numMatchingDrcSets] =
            p_obj_drc->str_payload.pstr_drc_sel_proc_output->sel_drc_set_ids[j];
        matchingDownmixIds[numMatchingDrcSets] =
            p_obj_drc->str_payload.pstr_drc_sel_proc_output->sel_downmix_ids[j];
        numMatchingDrcSets++;
      }
    }
    if (i == 0) {
      if (p_obj_drc->str_config.num_ch_in !=
          p_obj_drc->str_payload.pstr_drc_sel_proc_output->base_channel_count)

        return -1;
      audio_num_chan = p_obj_drc->str_config.num_ch_in;
    } else if (i == 1) {
      p_obj_drc->str_config.num_ch_out =
          p_obj_drc->str_payload.pstr_drc_sel_proc_output->target_channel_count;
      audio_num_chan = p_obj_drc->str_config.num_ch_out;
    }

    err_code = impd_init_drc_decode_post_config(
        audio_num_chan, matchingDrcSetIds, matchingDownmixIds,
        numMatchingDrcSets,
        p_obj_drc->str_payload.pstr_drc_sel_proc_output->sel_eq_set_ids[i]

        ,
        p_obj_drc->str_payload.pstr_gain_dec[i],
        p_obj_drc->str_payload.pstr_drc_config,
        p_obj_drc->str_payload.pstr_loudness_info, &persistent_ptr);
    if (err_code) return err_code;

    impd_get_parametric_drc_delay(
        p_obj_drc->str_payload.pstr_gain_dec[i],
        p_obj_drc->str_payload.pstr_drc_config,
        &p_obj_drc->str_config.parametric_drc_delay_gain_dec_instance,
        &p_obj_drc->str_config.parametric_drc_delay_max);
    impd_get_eq_delay(p_obj_drc->str_payload.pstr_gain_dec[i],
                      p_obj_drc->str_payload.pstr_drc_config,
                      &p_obj_drc->str_config.eq_delay_gain_dec_instance,
                      &p_obj_drc->str_config.eq_delay_max);
    p_obj_drc->str_config.parametric_drc_delay +=
        p_obj_drc->str_config.parametric_drc_delay_gain_dec_instance;
    p_obj_drc->str_config.eq_delay +=
        p_obj_drc->str_config.eq_delay_gain_dec_instance;
  }

  {
    if (p_obj_drc->str_config.parametric_drc_delay_max == -1) {
      p_obj_drc->str_config.parametric_drc_delay_max =
          PARAMETRIC_DRC_DELAY_MAX_DEFAULT;
    }
    if (p_obj_drc->str_config.eq_delay_max == -1) {
      p_obj_drc->str_config.eq_delay_max = EQ_DELAY_MAX_DEFAULT;
    }

    if (!p_obj_drc->str_config.constant_delay_on) {
      p_obj_drc->p_state->delay_in_output +=
          p_obj_drc->str_config.parametric_drc_delay +
          p_obj_drc->str_config.eq_delay +
          p_obj_drc->str_config.audio_delay_samples;
      p_obj_drc->str_config.delay_line_samples =
          p_obj_drc->str_config.audio_delay_samples;

      if (!p_obj_drc->str_config.absorb_delay_on) {
        p_obj_drc->p_state->delay_in_output = 0;
      }
    } else {
      p_obj_drc->p_state->delay_in_output +=
          p_obj_drc->str_config.parametric_drc_delay_max +
          p_obj_drc->str_config.eq_delay_max +
          p_obj_drc->str_config.audio_delay_samples;
      p_obj_drc->str_config.delay_line_samples =
          p_obj_drc->p_state->delay_in_output -
          p_obj_drc->str_config.parametric_drc_delay +
          p_obj_drc->str_config.eq_delay;

      if (!p_obj_drc->str_config.absorb_delay_on) {
        p_obj_drc->p_state->delay_in_output = 0;
      }
    }
  }
  if (p_obj_drc->str_config.dec_type == 1) {
    init_qmf_filt_bank(p_obj_drc->str_payload.pstr_qmf_filter);
  }

  if (p_obj_drc->str_config.peak_limiter) {
    err_code = impd_peak_limiter_init(
        p_obj_drc->str_payload.pstr_peak_limiter, DEFAULT_ATTACK_TIME_MS,
        DEFAULT_RELEASE_TIME_MS, LIM_DEFAULT_THRESHOLD,
        p_obj_drc->str_config.num_ch_out, p_obj_drc->str_config.sampling_rate,
        p_obj_drc->str_payload.pstr_peak_limiter->buffer);
    if (err_code) return (err_code);
  }

  return IA_NO_ERROR;
}
