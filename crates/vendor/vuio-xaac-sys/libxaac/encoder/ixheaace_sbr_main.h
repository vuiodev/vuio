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

#pragma once

#define IXHEAACE_MAX_PAYLOAD_SIZE (256)

enum { MONO = 1, STEREO = 2 };

typedef struct {
  WORD32 bit_rate;
  WORD32 num_channels;
  WORD32 sample_freq;
  WORD32 trans_fac;
  WORD32 standard_bitrate;
} ixheaace_str_codec_param;

typedef struct ixheaace_str_sbr_cfg {
  ixheaace_str_codec_param codec_settings;
  WORD32 send_header_data_time;
  WORD32 crc_sbr;
  WORD32 detect_missing_harmonics;
  WORD32 parametric_coding;

  WORD32 tran_thr;
  WORD32 noise_floor_offset;
  UWORD32 use_speech_config;

  WORD32 sbr_data_extra;
  WORD32 amp_res;
  WORD32 ana_max_level;
  WORD32 tran_fc;
  WORD32 tran_det_mode;
  WORD32 spread;
  WORD32 stat;
  WORD32 e;
  ixheaace_sbr_stereo_mode stereo_mode;
  WORD32 delta_t_across_frames;
  FLOAT32 df_edge_1st_env;
  FLOAT32 df_edge_incr;

  WORD32 sbr_invf_mode;
  WORD32 sbr_xpos_mode;
  WORD32 sbr_xpos_ctrl;
  WORD32 sbr_xpos_lvl;
  WORD32 start_freq;
  WORD32 stop_freq;

  WORD32 use_ps;
  WORD32 ps_mode;

  WORD32 freq_scale;
  WORD32 alter_scale;
  WORD32 sbr_noise_bands;

  WORD32 sbr_limiter_bands;
  WORD32 sbr_limiter_gains;
  WORD32 sbr_interpol_freq;
  WORD32 sbr_smoothing_length;
  WORD32 frame_flag_480;
  WORD32 frame_flag_960;
  WORD32 is_ld_sbr;
  WORD32 is_esbr;
  WORD32 sbr_pvc_rate;
  WORD32 sbr_ratio_idx;
  WORD32 sbr_pvc_active;
  WORD32 sbr_harmonic;
  WORD32 hq_esbr;
  ixheaace_sbr_codec_type sbr_codec;
  WORD32 use_low_freq_res;
} ixheaace_str_sbr_cfg, *ixheaace_pstr_sbr_cfg;

typedef struct ixheaace_str_sbr_enc *ixheaace_pstr_sbr_enc;

UWORD32
ixheaace_is_sbr_setting_available(UWORD32 bitrate, UWORD32 num_output_channels,
                                  UWORD32 sample_rate_input, UWORD32 *sample_rate_core,
                                  ixheaace_str_qmf_tabs *ptr_qmf_tab, WORD32 aot);

UWORD32 ixheaace_sbr_limit_bitrate(UWORD32 bit_rate, UWORD32 num_channels,
                                   UWORD32 core_sample_rate, ixheaace_str_qmf_tabs *ptr_qmf_tab,
                                   WORD32 aot);

VOID ixheaace_adjust_sbr_settings(const ixheaace_pstr_sbr_cfg pstr_config, UWORD32 bit_rate,
                                  UWORD32 num_channels, UWORD32 fs_core, UWORD32 trans_fac,
                                  UWORD32 standard_bitrate, ixheaace_str_qmf_tabs *ptr_qmf_tab,
                                  WORD32 aot, WORD32 is_esbr_4_1);

VOID ixheaace_initialize_sbr_defaults(ixheaace_pstr_sbr_cfg pstr_config);

IA_ERRORCODE
ixheaace_env_open(ixheaace_pstr_sbr_enc *pstr_env_encoder, ixheaace_pstr_sbr_cfg params,
                  WORD32 *core_bandwidth, WORD8 *spectral_band_replication_scratch_ptr,
                  ixheaace_str_sbr_tabs *pstr_sbr_tab, ixheaace_pstr_sbr_hdr_data *sbr_config);

VOID ixheaace_env_close(ixheaace_pstr_sbr_enc pstr_env_encoder);

WORD32
ixheaace_sbr_get_stop_freq_raw(ixheaace_pstr_sbr_enc ptr_env);

IA_ERRORCODE
ixheaace_env_encode_frame(ixheaace_pstr_sbr_enc ptr_env_encoder, FLOAT32 *samples,
                          FLOAT32 *core_buffer, UWORD32 time_sn_stride, UWORD8 *num_anc_bytes,
                          UWORD8 *anc_data, ixheaace_str_sbr_tabs *pstr_sbr_tab,
                          ixheaace_comm_tables *common_tab, UWORD8 *mps_data, WORD32 mps_bits,
                          WORD32 flag_framelength_small, WORD32 *usac_stat_bits);

VOID ixheaace_sbr_set_scratch_ptr(ixheaace_pstr_sbr_enc h_env_enc, VOID *scr);
WORD32 ixheaace_sbr_enc_pers_size(WORD32 num_chan, WORD32 use_ps, WORD32 harmonic_sbr);
WORD32 ixheaace_sbr_enc_scr_size(VOID);

VOID ixheaace_set_usac_sbr_params(ixheaace_pstr_sbr_enc h_env_enc, WORD32 usac_indep_flag,
                                  WORD32 sbr_pre_proc, WORD32 sbr_pvc_active, WORD32 sbr_pvc_mode,
                                  WORD32 inter_tes_active, WORD32 sbr_harmonic,
                                  WORD32 sbr_patching_mode);

FLOAT32 *ixheaace_get_hbe_resample_buffer(ixheaace_pstr_sbr_enc h_env_enc);

IA_ERRORCODE ixheaace_extract_sbr_envelope(FLOAT32 *ptr_in_time, FLOAT32 *ptr_core_buf,
                                           UWORD32 time_sn_stride,
                                           ixheaace_pstr_sbr_enc pstr_env_enc,
                                           ixheaace_str_sbr_tabs *ptr_sbr_tab,
                                           ixheaace_comm_tables *pstr_com_tab,
                                           WORD32 flag_framelength_small);
VOID ia_enhaacplus_enc_init_sbr_tabs(ixheaace_str_sbr_tabs *pstr_sbr_tabs);
VOID ia_enhaacplus_enc_get_shared_bufs(VOID *scr, WORD32 **shared_buf1, WORD32 **shared_buf2,
                                       WORD32 **shared_buf3, WORD8 **shared_buf4,
                                       WORD32 aacenc_blocksize);
VOID ia_enhaacplus_enc_get_scratch_bufs(VOID *scr, FLOAT32 **shared_buf1_ring,
                                        FLOAT32 **shared_buf2_ring);