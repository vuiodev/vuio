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
#ifndef IMPD_DRC_API_STRUCT_DEF_H
#define IMPD_DRC_API_STRUCT_DEF_H

/****************************************************************************/
/*                          structure definitions                           */
/****************************************************************************/
/* DRC Configuration */
typedef struct ia_drc_config_struct {
  WORD32 bitstream_file_format;
  WORD32 dec_type;
  WORD32 sub_band_domain_mode;
  WORD32 num_ch_in;
  WORD32 num_ch_out;
  WORD32 sampling_rate;
  WORD32 control_parameter_index;
  WORD32 delay_mode;
  WORD32 absorb_delay_on;
  WORD32 gain_delay_samples;
  WORD32 subband_domain_io_flag;
  WORD32 frame_size;
  WORD32 sub_band_down_sampling_factor;
  WORD32 sub_band_count;
  WORD32 peak_limiter;
  WORD32 interface_bitstream_present;
  WORD32 pcm_size;
  WORD32 parametric_drc_delay_gain_dec_instance;
  WORD32 parametric_drc_delay;
  WORD32 parametric_drc_delay_max;
  WORD32 eq_delay_gain_dec_instance;
  WORD32 eq_delay;
  WORD32 eq_delay_max;
  WORD32 delay_line_samples;
  WORD32 constant_delay_on;
  WORD32 audio_delay_samples;

  WORD32 effect_type;
  WORD32 target_loudness;
  WORD32 loud_norm_flag;
  WORD32 album_mode;
  FLOAT32 boost;
  FLOAT32 compress;
  UWORD8 boost_set;
  UWORD8 compress_set;
  WORD32 apply_crossfade;
  WORD32 is_config_changed;
  WORD32 ln_dbgain_prev;
#ifdef LOUDNESS_LEVELING_SUPPORT
  WORD32 loudness_leveling_flag;
#endif
} ia_drc_config_struct;

/* DRC bitsteam handler */
typedef struct bits_handler {
  UWORD8 *bitstream_drc_config;
  UWORD8 *bitstream_loudness_info;
  UWORD8 *bitstream_unidrc_interface;
  UWORD8 *it_bit_buf;
  WORD32 num_bytes_bs_drc_config;
  WORD32 num_bytes_bs_loudness_info;
  WORD32 num_bits_read_bs_unidrc_interface;
  WORD32 num_bytes_bs_unidrc_interface;
  WORD32 num_bits_read_bs;
  WORD32 num_bytes_read_bs;
  WORD32 num_bytes_bs;
  WORD32 num_bytes_offset_bs;
  WORD32 num_total_bytes;
  WORD32 num_bits_offset_bs;
  WORD32 byte_index_bs;
  WORD32 num_byts_cur;
  WORD32 num_byts_cur_ic;
  WORD32 num_byts_cur_il;
  WORD32 num_byts_cur_in;
  WORD32 cpy_over;
  WORD32 cpy_over_ic;
  WORD32 cpy_over_il;
  WORD32 cpy_over_in;
  WORD32 gain_stream_flag;
} ia_drc_bits_handler_struct;

typedef struct {
  ia_drc_bits_dec_struct *pstr_bitstream_dec;
  ia_drc_gain_dec_struct *pstr_gain_dec[2];
  ia_drc_sel_pro_struct *pstr_selection_proc;
  ia_drc_config *pstr_drc_config;
  ia_drc_loudness_info_set_struct *pstr_loudness_info;
  ia_drc_gain_struct *pstr_drc_gain;
  ia_drc_interface_struct *pstr_drc_interface;

  ia_drc_peak_limiter_struct *pstr_peak_limiter;
  ia_drc_qmf_filt_struct *pstr_qmf_filter;
  ia_drc_sel_proc_params_struct *pstr_drc_sel_proc_params;
  ia_drc_sel_proc_output_struct *pstr_drc_sel_proc_output;

} ia_drc_payload_struct;

typedef struct ia_drc_state_struct {
  UWORD32 ui_out_bytes;
  UWORD32 ui_in_bytes;
  UWORD32 ui_ir_bytes;
  UWORD32 total_num_out_samples;
  UWORD32 frame_no;
  UWORD32 out_size;
  UWORD32 ui_init_done;
  UWORD32 ui_exe_done;
  UWORD32 ui_ir_used;
  WORD32 delay_in_output;
  WORD32 delay_adjust_samples;
  pVOID persistent_ptr;
} ia_drc_state_struct;

typedef struct IA_PSM_API_Struct {
  ia_drc_state_struct *p_state;
  ia_drc_config_struct str_config;
  ia_drc_payload_struct str_payload;
  ia_drc_bits_handler_struct str_bit_handler;
  ia_mem_info_struct *p_mem_info;
  pVOID *pp_mem;
  struct ia_bit_buf_struct str_bit_buf, *pstr_bit_buf;
  WORD32 frame_count;
} ia_drc_api_struct;

#endif
