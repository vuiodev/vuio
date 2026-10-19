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

#define IXHEAACE_MEM_FREE(pv_output)                                                   \
  {                                                                                    \
    WORD32 idx;                                                                        \
    ixheaace_output_config *pstr_output_config = (ixheaace_output_config *)pv_output;  \
    if (pstr_output_config->malloc_count > 0) {                                        \
      for (idx = pstr_output_config->malloc_count - 1; idx >= 0; idx--) {              \
        if (pstr_output_config->arr_alloc_memory[idx]) {                               \
          pstr_output_config->free_xheaace(pstr_output_config->arr_alloc_memory[idx]); \
        }                                                                              \
      }                                                                                \
      pstr_output_config->malloc_count = 0;                                            \
    }                                                                                  \
  }

/*-------------------- structure definitions ------------------------------*/

typedef struct {
  WORD32 sample_rate;              /* mapped input sample rate */
  WORD32 core_sample_rate;         /* core coder processing sample rate */
  WORD32 native_sample_rate;       /* audio file sample rate */
  WORD32 bit_rate;                 /* encoder bit rate in bits/sec */
  WORD32 num_in_channels;          /* number of channels on input (1,2) */
  WORD32 num_out_channels;         /* number of channels on output (1,2) */
  WORD32 band_width;               /* targeted audio bandwidth in Hz */
  WORD32 dual_mono;                /* flag: make 2 SCEs for stereo input files */
  WORD32 use_tns;                  /* flag: use temporal noise shaping */
  WORD32 use_adts;                 /* flag: use ADTS instead of ADIF */
  WORD32 calc_crc;                 /* flag: write CRC checks */
  WORD32 private_bit;              /* private bit of MPEG Header */
  WORD32 copyright_bit;            /* copyright bit of MPEG Header */
  WORD32 original_copy_bit;        /* original bit of MPEG Header */
  WORD32 num_stereo_preprocessing; /* forbid usage of stereo prerpocessing */
  WORD32 inv_quant;                /* improve distortion by inverse quantization */
  WORD32 full_bandwidth;           /* improve distortion by inverse quantization */
  WORD32 flag_framelength_small;   /* indicates frame size. 0 -> 512, 1 -> 480
                                   indicates frame size. 0 -> 1024, 1 -> 960 */
  WORD32 bitreservoir_size;        /* size of bit reservoir (default:0; max 6144)*/
} iaace_config;

typedef struct {
  WORD32 *shared_buffer1;
  WORD32 *shared_buffer_2;
  WORD32 *shared_buffer3;
  WORD8 *shared_buffer5;
} iaace_scratch;

/* pstr_ancillary configuration struct */
typedef struct {
  WORD32 anc_flag;
  WORD32 anc_mode;
  WORD32 anc_rate;
} ixheaace_config_ancillary;

typedef struct {
  iaace_config config;
  ixheaace_element_info element_info;
  ixheaace_psy_out psy_out;
  ixheaace_psy_kernel psy_kernel;
  ixheaace_qc_state qc_kernel;
  ixheaace_qc_out qc_out;
  ixheaace_bitstream_enc_init bse_init;
  ixheaace_stereo_pre_pro_struct str_stereo_pre_pro;
  WORD32 downmix;
  WORD32 downmix_fac;
  WORD32 dual_mono;
  WORD32 bandwidth_90_dB;
  iaace_scratch *pstr_aac_scratch;
} iexheaac_encoder_str;

VOID ia_enhaacplus_enc_aac_init_default_config(iaace_config *config, WORD32 aot);

WORD32 ia_enhaacplus_enc_aac_enc_pers_size(WORD32 num_aac_chan, WORD32 aot);
WORD32 ia_enhaacplus_enc_aac_enc_scr_size(VOID);

VOID ia_enhaacplus_enc_init_aac_tabs(ixheaace_aac_tables *pstr_aac_tabs);

IA_ERRORCODE ia_enhaacplus_enc_aac_enc_open(iexheaac_encoder_str **ppstr_exheaac_encoder,
                                            const iaace_config config,
                                            iaace_scratch *pstr_aac_scratch,
                                            ixheaace_aac_tables *pstr_aac_tabs, WORD32 ele_type,
                                            WORD32 element_instance_tag, WORD32 aot);

IA_ERRORCODE ia_enhaacplus_enc_aac_core_encode(
    iexheaac_encoder_str **pstr_aac_enc, FLOAT32 *ptr_time_signal, UWORD32 time_sn_stride,
    const UWORD8 *ptr_anc_bytes, UWORD8 *num_anc_bytes, UWORD8 *ptr_out_bytes,
    WORD32 *num_out_bytes, ixheaace_aac_tables *pstr_aac_tables, VOID *ptr_bit_stream_handle,
    VOID *ptr_bit_stream, FLAG flag_last_element, WORD32 *write_program_config_element,
    WORD32 i_num_coup_channels, WORD32 i_channels_mask, WORD32 ele_idx, WORD32 *total_fill_bits,
    WORD32 total_channels, WORD32 aot, WORD32 adts_flag, WORD32 num_bs_elements,
    WORD32 *is_quant_spec_zero, WORD32 *is_gain_limited);

VOID ia_enhaacplus_enc_set_shared_bufs(iaace_scratch *scr, WORD32 **shared_buf1,
                                       WORD32 **shared_buf2, WORD32 **shared_buf3,
                                       WORD8 **shared_buf5);
