/******************************************************************************
 *                                                                            *
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
#ifndef IXHEAACD_STRUCT_DEF_H
#define IXHEAACD_STRUCT_DEF_H

#include <setjmp.h>
#include <stdbool.h>
#include "ixheaacd_peak_limiter_struct_def.h"

#define MAX_DECOR_CONFIG_IDX (2)
#define MAX_TIME_SLOTS (72)
#define NUM_MPS_TABLES (13)

#define MAX_PREROLL_FRAME_OFFSET 4
// max of escapedValue(4, 4, 8) i.e. 2^4 -1 + 2^4 -1 + 2^8 -1;
#define MAX_PREROLL_SIZE 285
#define IA_ENHAACPLUS_DEC_MPS_PAYLOAD_SIZE (1024)

typedef struct {
  WORD8 element_instance_tag;
  WORD32 object_type;
  WORD32 samp_freq_index;
  WORD32 num_front_channel_elements;
  WORD32 num_side_channel_elements;
  WORD32 num_back_channel_elements;
  WORD32 num_lfe_channel_elements;
  WORD32 num_assoc_data_elements;
  WORD32 num_valid_cc_elements;

  WORD8 front_element_is_cpe[16];
  WORD8 front_element_tag_select[16];
  WORD8 side_element_is_cpe[16];
  WORD8 side_element_tag_select[16];
  WORD8 back_element_is_cpe[16];
  WORD8 back_element_tag_select[16];
  WORD8 lfe_element_tag_select[16];

  WORD32 channels;
  WORD32 alignment_bits;

} ia_program_config_struct;

typedef struct ia_enhaacplus_dec_ind_cc {
  WORD8 cc_target_is_cpe[MAX_BS_ELEMENT];
  WORD8 cc_target_tag_select[MAX_BS_ELEMENT];
  WORD8 cc_l[MAX_BS_ELEMENT];
  WORD8 cc_r[MAX_BS_ELEMENT];
  WORD32 cc_gain[2 * MAX_BS_ELEMENT];
  WORD8 elements_coupled[MAX_BS_ELEMENT];
  WORD num_coupled_elements;

} ia_enhaacplus_dec_ind_cc;

typedef struct {
  UWORD32 ui_pcm_wdsz;
  UWORD32 ui_samp_freq;
  UWORD32 ui_n_channels;
  WORD32 i_channel_mask;
  UWORD32 ui_channel_mode;
  UWORD32 ui_sbr_mode;
  WORD32 ui_effect_type;
  WORD32 ui_target_loudness;
  WORD32 ui_loud_norm_flag;

  UWORD32 flag_downmix;
  UWORD32 flag_08khz_out;
  UWORD32 flag_16khz_out;
  UWORD32 flag_to_stereo;
  UWORD32 down_sample_flag;
  UWORD32 header_dec_done;
  UWORD32 ui_mp4_flag;
  UWORD32 ui_disable_sync;
  UWORD32 ui_auto_sbr_upsample;
  WORD32 frame_status;
  UWORD32 ui_max_channels;
  UWORD32 ui_pce_found_in_hdr;
  UWORD32 ui_n_memtabs;

  WORD32 ui_drc_boost;
  WORD32 ui_drc_cut;
  WORD32 ui_drc_target_level;
  WORD32 ui_drc_set;
  WORD32 ui_drc_heavy_comp;

  ia_program_config_struct str_prog_config;
  WORD32 element_type[MAX_BS_ELEMENT + 1];
  WORD32 slot_element[MAX_BS_ELEMENT + 1];

  WORD element_instance_order[MAX_BS_ELEMENT];
  WORD ui_coupling_channel;
  WORD downmix;
  WORD32 loas_present;

  WORD framesize_480;
  WORD ld_decoder;

  WORD eld_sbr_present;

  UWORD32 ui_out_channels;
  WORD32 ui_channel_mask;

  WORD32 ui_dec_type;

  UWORD32 ui_qmf_bands;

  WORD32 ui_flush_cmd;

  ia_drc_config drc_config_struct;
  WORD32 output_level;
  WORD32 i_loud_ref_level;
  UWORD8 dup_stereo_flag;
  WORD32 peak_limiter_off;

  WORD32 mps_present;
  UWORD32 ui_frame_size;
  WORD32 ui_enh_sbr;
  WORD32 ui_hq_esbr;
  WORD32 ui_enh_sbr_ps;
  WORD32 ui_usac_flag;

  WORD32 ui_err_conceal;
  FLAG first_frame;
#ifdef LOUDNESS_LEVELING_SUPPORT
  WORD32 ui_loudness_leveling_flag;
#endif
  UWORD8 ui_drc_mode_cut;
  UWORD8 ui_drc_mode_boost;
} ia_aac_dec_config_struct;

typedef struct ia_aac_dec_state_struct {
  ia_aac_dec_config_struct *p_config;

  AUDIO_OBJECT_TYPE audio_object_type;

  UWORD32 ui_in_bytes;
  UWORD32 ui_out_bytes;
  UWORD32 ui_exec_done;

  WORD16 b_n_raw_data_blk;
  WORD16 s_adts_hdr_present;
  WORD16 s_adif_hdr_present;
  WORD16 num_channel_last;
  UWORD32 sampling_rate;
  UWORD32 extension_samp_rate;
  UWORD32 bit_rate;
  UWORD32 ui_init_done;
  UWORD32 ui_input_over;
  UWORD32 header_dec_done;
  WORD32 frame_counter;
  ia_aac_decoder_struct *pstr_aac_dec_info[MAX_BS_ELEMENT];

  UWORD32 ch_config;
  struct ia_bit_buf_struct str_bit_buf, *pstr_bit_buf;
  ia_aac_dec_sbr_bitstream_struct (*pstr_stream_sbr)[2];
  ia_handle_sbr_dec_inst_struct str_sbr_dec_info[MAX_BS_ELEMENT];
  WORD32 sbr_present_flag;
  WORD32 ps_present;

  ia_bit_buf_struct *ptr_bit_stream;

  VOID *aac_scratch_mem_v;
  VOID *aac_persistent_mem_v;

  VOID *sbr_persistent_mem_v;
  WORD32 *ptr_overlap_buf;
  WORD16 num_of_out_samples;
  WORD32 last_frame_ok;
  WORD32 i_bytes_consumed;

  WORD32 *coup_ch_output;
  ia_enhaacplus_dec_ind_cc ind_cc_info;

  WORD8 protection_absent;
  WORD32 crc_check;
  WORD32 ui_flush_cmd;

  WORD32 frame_len_flag;
  WORD32 depends_on_core_coder;
  WORD32 extension_flag;

  WORD32 bs_format;
  WORD32 bit_count;
  WORD32 sync_status;
  WORD32 extension_flag_3;

  ixheaacd_latm_struct latm_struct_element;
  WORD32 latm_initialized;

  ia_drc_dec_struct str_drc_dec_info;
  ia_drc_dec_struct drc_dummy;
  ia_drc_dec_struct *pstr_drc_dec;
  ixheaac_drc_data_struct *pstr_hdrc_data[MAX_BS_ELEMENT * 3];

  WORD32 prev_channel_mode;
  WORD32 drc_cut_fac;
  WORD32 drc_boost_fac;

  WORD32 first_drc_frame;

  ia_aac_err_config_struct str_err_config;
  WORD32 frame_size;
  WORD32 frame_length;

  WORD32 dwnsmp_signal;
  ia_eld_specific_config_struct eld_specific_config;
  FLAG usac_flag;

  WORD32 num_of_output_ch;
  VOID *ia_audio_specific_config;
  ia_mps_dec_state_struct mps_dec_handle;

  ia_heaac_mps_state_struct heaac_mps_handle;
  UWORD8 mps_buffer[IA_ENHAACPLUS_DEC_MPS_PAYLOAD_SIZE];

  UWORD16 *huffman_code_book_scl;
  UWORD32 *huffman_code_book_scl_index;

  ia_aac_dec_tables_struct *pstr_aac_tables;

  VOID *pstr_dec_data;
  VOID *sbr_persistent_mem_u;
  VOID *sbr_scratch_mem_u;
  UWORD8 *header_ptr;
  WORD32 header_length;
  ia_sbr_header_data_struct str_sbr_config;
  jmp_buf xaac_jmp_buf;
  WORD32 decode_create_done;
  WORD32 ldmps_present;
  WORD32 fatal_err_present;
  WORD8 *pers_mem_ptr;
  UWORD8 preroll_config_present;
  UWORD8 preroll_config_prev[MAX_PREROLL_SIZE];

  UWORD8 qshift_cnt;
  WORD8 qshift_adj[16];
  UWORD32 delay_in_samples;
  UWORD8 peak_lim_init;
  ia_peak_limiter_struct peak_limiter;
  UWORD8 sbr_present;
  UWORD8 slot_pos;
  WORD32 mps_header;
  WORD32 ui_mps_out_bytes;
  WORD32 drc_config_changed;
  WORD32 apply_crossfade;
  WORD32 ec_enable;
  WORD32 first_frame;
} ia_aac_dec_state_struct;

typedef struct ia_exhaacplus_dec_api_struct {
  ia_aac_dec_state_struct *p_state_aac;

  ia_aac_dec_config_struct aac_config;

  ia_mem_info_struct *p_mem_info_aac;

  pVOID *pp_mem_aac;

  ia_aac_dec_tables_struct aac_tables;
  ixheaacd_misc_tables *common_tables;
  ia_sbr_tables_struct str_sbr_tables;

} ia_exhaacplus_dec_api_struct;

WORD32 ixheaacd_aacdec_decodeframe(
    ia_exhaacplus_dec_api_struct *p_obj_exhaacplus_dec,
    ia_aac_dec_scratch_struct *aac_scratch_ptrs, VOID *time_data, FLAG frame_status, WORD *type,
    WORD *ch_idx, WORD init_flag, WORD channel, WORD *element_index_order, WORD skip_full_decode,
    WORD ch_fac, WORD slot_element, WORD max_channels, WORD32 total_channels, WORD32 frame_length,
    WORD32 frame_size, ia_drc_dec_struct *pstr_drc_dec, WORD32 object_type, WORD32 ch_config,
    ia_eld_specific_config_struct eld_specific_config, WORD16 adtsheader,
    ia_drc_dec_struct *drc_dummy, WORD32 ldmps_present, UWORD8 *slot_pos, UWORD8 *mps_buffer,
    WORD32 *mps_header, WORD32 *mps_bytes, WORD32 is_init, WORD32 first_frame);

WORD ixheaacd_get_channel_mask(
    ia_exhaacplus_dec_api_struct *p_obj_exhaacplus_dec);

VOID ixheaacd_allocate_mem_persistent(
    ia_exhaacplus_dec_api_struct *p_obj_exhaacplus_dec,
    ia_aac_dec_state_struct *p_state_enhaacplus_dec, WORD channels,
    WORD *persistent_used_total, WORD *sbr_persistent_start, WORD ps_enable);

WORD32 ixheaacd_dec_mem_api(ia_exhaacplus_dec_api_struct *p_obj_exhaacplus_dec,
                            WORD32 i_cmd, WORD32 i_idx, VOID *pv_value);

VOID ixheaacd_fill_aac_mem_tables(
    ia_exhaacplus_dec_api_struct *p_obj_exhaacplus_dec);

WORD32 ixheaacd_decoder_2_ga_hdr(
    ia_exhaacplus_dec_api_struct *p_obj_exhaacplus_dec);

WORD32 ixheaacd_decoder_flush_api(
    ia_exhaacplus_dec_api_struct *p_obj_exhaacplus_dec);

WORD32 ixheaacd_fill_usac_mem_tables(ia_exhaacplus_dec_api_struct *self);

WORD32 ixheaacd_dec_init(ia_exhaacplus_dec_api_struct *p_obj_exhaacplus_dec);

WORD32 ixheaacd_dec_execute(ia_exhaacplus_dec_api_struct *p_obj_exhaacplus_dec);

WORD32 ixheaacd_dec_table_api(
    ia_exhaacplus_dec_api_struct *p_obj_exhaacplus_dec, WORD32 i_cmd,
    WORD32 i_idx, VOID *pv_value);

IA_ERRORCODE ixheaacd_aac_mps_init(ia_exhaacplus_dec_api_struct *p_obj_mps_dec,
                                   UWORD8 *databuf, WORD32 buffer_size,
                                   WORD32 sample_rate);

WORD32 ixheaacd_peak_limiter_init(ia_peak_limiter_struct *peak_limiter, UWORD32 num_channels,
                                  UWORD32 sample_rate, FLOAT32 *buffer,
                                  UWORD32 *delay_in_samples);

VOID ixheaacd_peak_limiter_process(ia_peak_limiter_struct *peak_limiter, VOID *samples,
                                   UWORD32 frame_len, WORD8 *qshift_adj);

VOID ixheaacd_scale_adjust(VOID *samples, UWORD32 frame_len, WORD8 *qshift_adj,
                           WORD num_channels);
VOID ixheaacd_peak_limiter_process_float(ia_peak_limiter_struct *peak_limiter,
                                         FLOAT32 samples[MAX_NUM_CHANNELS][4096],
                                         UWORD32 frame_len);


#endif /* IXHEAACD_STRUCT_DEF_H */
