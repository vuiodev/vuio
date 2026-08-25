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
/****************************************************************************/
/*                          structure definitions                           */
/****************************************************************************/
/* enhaacplus_enc configuration */
typedef struct {
  WORD32 i_channels;
  WORD32 i_native_channels;
  WORD32 i_n_memtabs;
  WORD32 sample_rate;
  WORD32 native_sample_rate;
  WORD32 i_channels_mode;
  WORD32 aot;
  WORD32 i_channels_mask;
  WORD32 i_num_coupling_chan;
  WORD32 element_type;
  WORD32 element_slot;
  WORD32 num_bs_elements;
  WORD32 element_instance_tag;
  /* Add config params here */
  WORD32 aac_classic;
  WORD32 use_parametric_stereo;
  WORD32 chmode_nchannels;
  WORD32 chmode;
  WORD32 firstframe;
  WORD32 adts_flag;
  WORD32 esbr_flag;
  WORD32 init_success;
  WORD32 silence_marker;
  WORD32 frame_count;
  FLAG write_program_config_element;
  iaace_config aac_config;
  ixheaace_config_ancillary pstr_ancillary;
  WORD32 mps_tree_config;
  WORD32 use_mps;
  WORD32 eldsbr_found;
  WORD32 ccfl_idx;
  UWORD32 ui_pcm_wd_sz;
  WORD32 frame_length;
  ia_usac_encoder_config_struct usac_config;
} ixheaace_config_struct;

typedef struct ixheaace_state_struct {
  // The first AACENC_BLOCKSIZE*2 elements are the same as that of the encoder i/p buffer.
  // The usage of input buffer as scratch is avoided here
  FLOAT32 *inp_delay;
  FLOAT32 *time_signal_mps;
  FLOAT32 *time_signal;
  UWORD8 *mps_bs;
  ixheaace_config_struct *pstr_config[MAXIMUM_BS_ELE];
  WORD32 aot;
  WORD32 mps_enable;
  WORD32 mps_tree_config;
  WORD32 i_out_bytes;
  UWORD32 ui_in_bytes;
  UWORD32 ui_input_over;
  UWORD32 ui_init_done;
  /* other state structure variables */
  WORD32 downsample[MAXIMUM_BS_ELE];
  WORD32 buffer_offset;
  iexheaac_encoder_str *aac_enc_pers_mem[MAXIMUM_BS_ELE];
  VOID *temp_buff_aac;
  ixheaace_bit_buf bit_stream;
  ixheaace_bit_buf_handle pstr_bit_stream_handle;
  struct ixheaace_str_sbr_enc *spectral_band_replication_enc_pers_mem[MAXIMUM_BS_ELE];
  VOID *temp_buff_sbr;
  VOID *ptr_temp_buff_resamp;
  ixheaace_iir21_resampler down_sampler[MAXIMUM_BS_ELE][IXHEAACE_MAX_CH_IN_BS_ELE];
  ixheaace_iir_sos_resampler down_samp_sos[MAXIMUM_BS_ELE][IXHEAACE_MAX_CH_IN_BS_ELE];
  ixheaace_iir_sos_resampler up_sampler[MAXIMUM_BS_ELE][IXHEAACE_MAX_CH_IN_BS_ELE];
  ixheaace_iir21_resampler hbe_down_sampler[MAXIMUM_BS_ELE][IXHEAACE_MAX_CH_IN_BS_ELE];
  ixheaace_iir_sos_resampler hbe_down_samp_sos[MAXIMUM_BS_ELE][IXHEAACE_MAX_CH_IN_BS_ELE];
  ixheaace_iir_sos_resampler hbe_up_sampler[MAXIMUM_BS_ELE][IXHEAACE_MAX_CH_IN_BS_ELE];
  UWORD8 num_anc_data_bytes[MAXIMUM_BS_ELE][IXHEAACE_MAX_CH_IN_BS_ELE];
  UWORD8 anc_data_bytes[MAXIMUM_BS_ELE][IXHEAACE_MAX_PAYLOAD_SIZE];
  WORD32 total_fill_bits;
  int *scratch_addr;
  char flag_error;
  FLOAT32 **ptr_in_buf;
  FLOAT32 **pp_drc_in_buf;
  FLOAT32 *mps_scratch;
  ixheaace_audio_specific_config_struct audio_specific_config;
  ia_usac_data_struct str_usac_enc_data;
  ia_bit_buf_struct str_bit_buf;
  ixheaace_mps_212_memory_struct *mps_pers_mem;
  ixheaace_mps_515_memory_struct *mps_515_pers_mem;
  WORD32 is_quant_spec_zero;
  WORD32 is_gain_limited;
  WORD32 i_out_bits;
  VOID *loudness_handle;
} ixheaace_state_struct;

typedef struct ixheaace_api_struct {
  /* pointer to the state structure */
  ixheaace_state_struct *pstr_state;
  ixheaace_config_struct config[MAXIMUM_BS_ELE];
  /* the mem tables */
  ixheaace_mem_info_struct *pstr_mem_info;
  /* the mem pointers */
  pVOID *pp_mem;
  /* the table structs */
  ixheaace_aac_tables pstr_aac_tabs;
  ixheaace_comm_tables common_tabs;
  ixheaace_str_sbr_tabs spectral_band_replication_tabs;
  WORD32 usac_en;
  VOID *pstr_mps_212_enc;
  VOID *pstr_mps_515_enc;
} ixheaace_api_struct;
