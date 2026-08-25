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
#ifndef IMPD_PARAMETRIC_DRC_DEC_H
#define IMPD_PARAMETRIC_DRC_DEC_H

#ifdef __cplusplus
extern "C" {
#endif

typedef struct ia_2nd_order_filt_coeff_struct_t {
  FLOAT32 b0, b1, b2;
  FLOAT32 a1, a2;
} ia_2nd_order_filt_coeff_struct;

typedef struct ia_2nd_order_filt_state_struct_t {
  FLOAT32 z1, z2;
} ia_2nd_order_filt_state_struct;

typedef struct ia_parametric_drc_type_ff_params_struct_t {
  WORD32 audio_num_chan;
  WORD32 frame_size;
  WORD32 sub_band_domain_mode;
  WORD32 sub_band_count;
  WORD32 level_estim_integration_time;
  WORD32 level_estim_frame_index;
  WORD32 level_estim_frame_count;
  FLOAT32 level[PARAM_DRC_TYPE_FF_LEVEL_ESTIM_FRAME_COUNT_MAX];
  WORD32 start_up_phase;
  FLOAT32 level_estim_ch_weight[MAX_CHANNEL_COUNT];
  WORD32 level_estim_k_weighting_type;
  ia_2nd_order_filt_coeff_struct pre_filt_coeff;
  ia_2nd_order_filt_coeff_struct rlb_filt_coeff;
  ia_2nd_order_filt_state_struct pre_filt_state[MAX_CHANNEL_COUNT];
  ia_2nd_order_filt_state_struct rlb_filt_state[MAX_CHANNEL_COUNT];
  FLOAT32 weighting_filt[AUDIO_CODEC_SUBBAND_COUNT_MAX];
  WORD32 sub_band_compensation_type;
  ia_2nd_order_filt_coeff_struct filt_coeff_subband;
  ia_2nd_order_filt_state_struct filt_state_subband_real[MAX_CHANNEL_COUNT];
  ia_2nd_order_filt_state_struct filt_state_subband_imag[MAX_CHANNEL_COUNT];
  FLOAT32 ref_level_parametric_drc;

  WORD32 node_count;
  WORD32 node_level[PARAM_DRC_TYPE_FF_NODE_COUNT_MAX];
  WORD32 node_gain[PARAM_DRC_TYPE_FF_NODE_COUNT_MAX];

  FLOAT32 gain_smooth_attack_alpha_slow;
  FLOAT32 gain_smooth_rel_alpha_slow;
  FLOAT32 gain_smooth_attack_alpha_fast;
  FLOAT32 gain_smooth_rel_alpha_fast;
  WORD32 gain_smooth_attack_threshold;
  WORD32 gain_smooth_rel_threshold;
  WORD32 gain_smooth_hold_off_count;
  FLOAT32 db_level_smooth;
  FLOAT32 db_gain_smooth;
  WORD32 hold_counter;

} ia_parametric_drc_type_ff_params_struct;

typedef struct ia_parametric_drc_type_lim_params_struct_t {
  WORD32 audio_num_chan;
  WORD32 frame_size;
  FLOAT32 level_estim_ch_weight[MAX_CHANNEL_COUNT];

  UWORD32 attack;
  FLOAT32 attack_constant;
  FLOAT32 release_constant;
  FLOAT32 attack_ms;
  FLOAT32 release_ms;
  FLOAT32 threshold;
  UWORD32 channels;
  UWORD32 sampling_rate;
  FLOAT32 cor;
  FLOAT32* max_buf;
  FLOAT32* max_buf_slow;
  UWORD32 max_buf_idx;
  UWORD32 max_buf_slow_idx;
  UWORD32 sec_len;
  UWORD32 num_max_buf_sec;
  UWORD32 max_buf_sec_idx;
  UWORD32 max_buf_sec_ctr;
  FLOAT64 smooth_state_0;

} ia_parametric_drc_type_lim_params_struct;

typedef struct ia_parametric_drc_instance_params_struct_t {
  WORD32 disable_paramteric_drc;
  WORD32 parametric_drc_type;
  ia_spline_nodes_struct str_spline_nodes;
  ia_parametric_drc_type_ff_params_struct str_parametric_drc_type_ff_params;
  ia_parametric_drc_type_lim_params_struct str_parametric_drc_type_lim_params;
} ia_parametric_drc_instance_params_struct;

typedef struct ia_parametric_drc_params_struct_t {
  WORD32 sampling_rate;
  WORD32 audio_num_chan;
  WORD32 sub_band_domain_mode;
  WORD32 sub_band_count;

  WORD32 num_nodes;
  WORD32 drc_frame_size;
  WORD32 parametric_drc_frame_size;
  WORD32 parametric_drc_look_ahead_samples_max;
  WORD32 reset_parametric_drc;

  WORD32 parametric_drc_instance_count;
  WORD32 parametric_drc_idx[PARAM_DRC_INSTANCE_COUNT_MAX];
  WORD32 gain_set_index[PARAM_DRC_INSTANCE_COUNT_MAX];
  WORD32 dwnmix_id_from_drc_instructions[PARAM_DRC_INSTANCE_COUNT_MAX];
  WORD32 channel_map[PARAM_DRC_INSTANCE_COUNT_MAX][MAX_CHANNEL_COUNT];

  ia_parametric_drc_instance_params_struct
      str_parametric_drc_instance_params[PARAM_DRC_INSTANCE_COUNT_MAX];

} ia_parametric_drc_params_struct;

WORD32 impd_init_parametric_drc(
    WORD32 drc_frame_size, WORD32 sampling_rate, WORD32 sub_band_domain_mode,
    ia_parametric_drc_params_struct* pstr_parametric_drc_params);

WORD32 impd_init_parametric_drc_after_config(
    ia_drc_config* pstr_drc_config,
    ia_drc_loudness_info_set_struct* pstr_loudness_info,
    ia_parametric_drc_params_struct* pstr_parametric_drc_params,
    pVOID* mem_ptr);

WORD32
impd_init_lvl_est_filt_time(WORD32 level_estim_k_weighting_type,
                            WORD32 sampling_rate,
                            ia_2nd_order_filt_coeff_struct* pre_filt_coeff,
                            ia_2nd_order_filt_coeff_struct* rlb_filt_coeff);

WORD32
impd_init_lvl_est_filt_subband(
    WORD32 level_estim_k_weighting_type, WORD32 sampling_rate,
    WORD32 sub_band_domain_mode, WORD32 sub_band_count,
    WORD32 sub_band_compensation_type, FLOAT32* weighting_filt,
    ia_2nd_order_filt_coeff_struct* filt_coeff_subband);

WORD32
impd_parametric_ffwd_type_drc_reset(ia_parametric_drc_type_ff_params_struct*
                                        pstr_parametric_ffwd_type_drc_params);

WORD32
impd_parametric_lim_type_drc_reset(WORD32 instance_idx,
                                   ia_parametric_drc_type_lim_params_struct*
                                       pstr_parametric_lim_type_drc_params);

WORD32
impd_parametric_drc_instance_process(
    FLOAT32* audio_in_out_buf[], FLOAT32* audio_real_buff[],
    FLOAT32* audio_imag_buff[],
    ia_parametric_drc_params_struct* pstr_parametric_drc_params,
    ia_parametric_drc_instance_params_struct*
        pstr_parametric_drc_instance_params);

WORD32
impd_parametric_ffwd_type_drc_process(FLOAT32* audio_in_out_buf[],
                                      FLOAT32* audio_real_buff[],
                                      FLOAT32* audio_imag_buff[],
                                      WORD32 nodeIdx,
                                      ia_parametric_drc_type_ff_params_struct*
                                          pstr_parametric_ffwd_type_drc_params,
                                      ia_spline_nodes_struct* str_spline_nodes);

VOID impd_parametric_lim_type_drc_process(
    FLOAT32* audio_in_out_buf[], FLOAT32 loudness_normalization_gain_db,
    ia_parametric_drc_type_lim_params_struct*
        pstr_parametric_lim_type_drc_params,
    FLOAT32* lpcm_gains);

#ifdef __cplusplus
}
#endif
#endif
