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
typedef enum {
  ID_SCE = 0, /*Single Channel Element*/
  ID_CPE,     /*Channel Pair Element*/
  ID_CCE,     /*Coupling channel Element*/
  ID_LFE,     /*Low frequency Effect Channel*/
  ID_DSE,
  ID_PCE, /*Program Config. Element*/
  ID_FIL, /*Fill element*/
  ID_END  /*End of block*/
} ixheaace_element_type;

typedef enum {
  FRONT_CENTER = 0,
  FRONT_LEFT_RIGHT,
  BACK_LEFT_RIGHT,
  REAR_CENTER,
  LFE_CHANNEL,
  COUPLING_CH,
} ELEMENT_NAME;

typedef enum { FRAME_LEN_BYTES_MODULO = 1, FRAME_LEN_BYTES_INT = 2 } FRAME_LEN_RESULT_MODE;

typedef struct {
  ixheaace_element_type el_type;
  WORD32 instance_tag;
  WORD32 n_channels_in_el;
  WORD32 channel_index[IXHEAACE_MAX_CH_IN_BS_ELE];
} ixheaace_element_info;

typedef struct {
  WORD32 padding_rest;
} ixheaace_padding;

/* Quantizing & coding stage */

typedef struct {
  ixheaace_element_info *pstr_element_info;
  WORD32 max_bits;     /* maximum number of bits in reservoir  */
  WORD32 average_bits; /* average number of bits we should use */
  WORD32 bit_res;
  FLOAT32 mean_pe;
  WORD32 ch_bitrate;
  WORD32 inv_quant;
  FLOAT32 max_bit_fac;
  WORD32 bitrate;
  ixheaace_padding padding;
} ixheaace_qc_init;

#define RED_EXP_VAL 0.25f
#define INV_RED_EXP_VAL (1.0f / RED_EXP_VAL)
#define MIN_SNR_LIMIT 0.8f

#define MAX_SCF_DELTA 60

#define LOG2_1 1.442695041f
#define C1_SF -69.33295f /* -16/3*log(MAX_QUANT+0.5-logCon)/log(2) */
#define C2_SF 5.77078f   /* 4/log(2) */

#define PE_C1 3.0f       /* log(8.0)/log(2) */
#define PE_C2 1.3219281f /* log(2.5)/log(2) */
#define PE_C3 0.5593573f /* 1-C2/C1 */

#define TRANSPORT_BITS (208)

#define MAX_GAIN_INDEX_AAC (128)
#define MIN_GAIN_INDEX_AAC (-128)
#define SCF_COUNT_LIMIT_AAC (1)

typedef struct {
  WORD16 *quant_spec;
  UWORD16 *max_val_in_sfb;
  WORD16 *scalefactor;
  WORD32 global_gain;
  WORD32 grouping_mask;
  ixheaace_section_data section_data;
  WORD32 win_shape;
} ixheaace_qc_out_channel;

typedef struct {
  WORD32 static_bits_used; /* for verification purposes */
  WORD32 dyn_bits_used;    /* for verification purposes */
  FLOAT32 pe;
  WORD32 anc_bits_used;
  WORD32 fill_bits;
} ixheaace_qc_out_element;

typedef struct {
  ixheaace_qc_out_channel *qc_channel[IXHEAACE_MAX_CH_IN_BS_ELE];
  ixheaace_qc_out_element qc_element;
  WORD32 tot_static_bits_used; /* for verification purposes */
  WORD32 tot_dyn_bits_used;    /* for verification purposes */
  WORD32 tot_anc_bits_used;    /* for verification purposes */
  WORD32 total_fill_bits;
  WORD32 align_bits;
  WORD32 bit_res_tot;
  WORD32 average_bits_tot;
} ixheaace_qc_out;

typedef struct {
  WORD32 ch_bitrate;
  WORD32 average_bits; /* brutto -> look pstr_ancillary.h */
  WORD32 max_bits;
  WORD32 bit_res_level;
  WORD32 max_bit_res_bits;
  WORD32 relative_bits; /* Bits relative to total Bits*/
  WORD32 carry_bits;    /* Bits carried over from prev. frame */
} ixheaace_element_bits;

typedef struct {
  WORD32 *shared_buffer_2;
} ixheaace_qc_scratch;

typedef struct {
  /* this is basically struct ixheaace_qc_init */
  WORD32 average_bits_tot;
  WORD32 max_bits_tot;
  WORD32 glob_stat_bits;
  WORD32 num_channels;
  WORD32 bit_res_tot;
  WORD32 quality_level;
  ixheaace_padding padding;
  ixheaace_element_bits element_bits;
  FLOAT32 max_bit_fac;
  ia_adj_thr_state_struct str_adj_thr;
  WORD32 side_info_tab_long[MAXIMUM_SCALE_FACTOR_BAND_LONG + 1];
  WORD32 side_info_tab_short[MAXIMUM_SCALE_FACTOR_BAND_SHORT + 1];
  ixheaace_qc_scratch qc_scr;
} ixheaace_qc_state;

typedef struct {
  FLOAT32 exp_spec[FRAME_LEN_1024];
  FLOAT32 mdct_spec_float[FRAME_LEN_1024];
  FLOAT32 sfb_form_fac[IXHEAACE_MAX_CH_IN_BS_ELE][MAXIMUM_GROUPED_SCALE_FACTOR_BAND];
  FLOAT32 sfb_num_relevant_lines[IXHEAACE_MAX_CH_IN_BS_ELE][MAXIMUM_GROUPED_SCALE_FACTOR_BAND];
  FLOAT32 sfb_ld_energy[IXHEAACE_MAX_CH_IN_BS_ELE][MAXIMUM_GROUPED_SCALE_FACTOR_BAND];
} ixheaace_qc_stack;
