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
#ifndef IMPD_DRC_GAIN_DECODER2_H
#define IMPD_DRC_GAIN_DECODER2_H

#ifndef M_PI
#define M_PI (3.14159265358979323846)
#endif

typedef struct ia_drc_gain_dec_struct {
  WORD32 audio_num_chan;
  ia_drc_params_struct ia_drc_params_struct;
  ia_drc_gain_buffers_struct drc_gain_buffers;
  ia_audio_band_buffer_struct audio_band_buffer;
  ia_overlap_params_struct str_overlap_params;
  ia_filter_banks_struct ia_filter_banks_struct;
  ia_audio_in_out_buf audio_in_out_buf;
  ia_parametric_drc_params_struct parametricdrc_params;
  shape_filter_block shape_filter_block[CHANNEL_GROUP_COUNT_MAX];
  ia_eq_set_struct* eq_set;
} ia_drc_gain_dec_struct;

WORD32 impd_init_drc_decode(WORD32 frame_size, WORD32 sample_rate,
                            WORD32 gain_delay_samples, WORD32 delay_mode,
                            WORD32 sub_band_domain_mode,
                            ia_drc_gain_dec_struct* p_drc_gain_dec_structs);

WORD32 impd_init_drc_decode_post_config(
    WORD32 audio_num_chan, WORD32* drc_set_id_processed,
    WORD32* downmix_id_processed, WORD32 num_sets_processed,
    WORD32 eq_set_id_processed,

    ia_drc_gain_dec_struct* p_drc_gain_dec_structs,
    ia_drc_config* pstr_drc_config,
    ia_drc_loudness_info_set_struct* pstr_loudness_info, pVOID* mem_ptr

    );

WORD32 impd_drc_process_time_domain(
    ia_drc_gain_dec_struct* p_drc_gain_dec_structs,
    ia_drc_config* pstr_drc_config, ia_drc_gain_struct* pstr_drc_gain,
    FLOAT32* audio_in_out_buf[], FLOAT32 loudness_normalization_gain_db,
    FLOAT32 boost, FLOAT32 compress, WORD32 drc_characteristic);

WORD32 impd_drc_process_freq_domain(
    ia_drc_gain_dec_struct* p_drc_gain_dec_structs,
    ia_drc_config* pstr_drc_config, ia_drc_gain_struct* pstr_drc_gain,
    FLOAT32* audio_real_buff[], FLOAT32* audio_imag_buff[],
    FLOAT32 loudness_normalization_gain_db, FLOAT32 boost, FLOAT32 compress,
    WORD32 drc_characteristic);

VOID impd_get_parametric_drc_delay(
    ia_drc_gain_dec_struct* p_drc_gain_dec_structs,
    ia_drc_config* pstr_drc_config, WORD32* parametric_drc_delay,
    WORD32* parametric_drc_delay_max);

VOID impd_get_eq_delay(ia_drc_gain_dec_struct* p_drc_gain_dec_structs,
                       ia_drc_config* pstr_drc_config, WORD32* eq_delay,
                       WORD32* eq_delay_max);

WORD32
impd_get_drc_gain(ia_drc_gain_dec_struct* p_drc_gain_dec_structs,
                  ia_drc_config* pstr_drc_config,
                  ia_drc_gain_struct* pstr_drc_gain, FLOAT32 compress,
                  FLOAT32 boost, WORD32 characteristic_index,
                  FLOAT32 loudness_normalization_gain_db, WORD32 sub_drc_index,
                  ia_drc_gain_buffers_struct* drc_gain_buffers);

#endif
