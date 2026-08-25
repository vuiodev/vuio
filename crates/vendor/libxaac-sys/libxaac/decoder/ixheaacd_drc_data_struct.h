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
#ifndef IXHEAACD_DRC_DATA_STRUCT_H

#define IXHEAACD_DRC_DATA_STRUCT_H

#define MAX_DRC_BANDS 16

#define MAX_AUDIO_CHANNELS 8

#define SBR_QMF_SUB_SAMPLES 64
#define SBR_QMF_SUB_BANDS 64

typedef enum {
  UNKNOWN_PAYLOAD = 0,
  MPEG_DRC_EXT_DATA = 1,
  DVB_DRC_ANC_DATA = 2

} AACDEC_DRC_PAYLOAD_TYPE;

#define DVB_ANC_DATA_SYNC_BYTE (0xBC)
typedef struct {
  WORD32 prog_ref_level;
  WORD16 n_mdct_bands[MAX_DRC_BANDS];
  WORD16 drc_fac[MAX_DRC_BANDS];
  WORD16 drc_fac_dvb[MAX_DRC_BANDS];
  WORD8 drc_exp;
  UWORD8 short_block;
  UWORD8 drc_interp_scheme;
  UWORD8 n_drc_bands;
  UWORD8 new_prog_ref_level;
  UWORD8 new_drc_fac;
  UWORD8 prev_interp_scheme;
  WORD32 drc_factors_sbr[SBR_QMF_SUB_SAMPLES][SBR_QMF_SUB_BANDS];
  WORD32 drc_factors_sbr_lat[SBR_QMF_SUB_SAMPLES][SBR_QMF_SUB_BANDS];
} ixheaac_drc_data_struct;

typedef struct {
  UWORD8 b_channel_on[MAX_AUDIO_CHANNELS];
  UWORD8 prog_ref_level_present;
  UWORD8 prog_ref_level;
  UWORD8 drc_num_bands;
  UWORD8 drc_band_top[MAX_DRC_BANDS];
  WORD8 dyn_rng_dlbl[MAX_DRC_BANDS];
  WORD8 dyn_rng_dlbl_dvb[MAX_DRC_BANDS];
  WORD8 max_dyn_rng_dlbl;
  UWORD8 drc_interpolation_scheme;
  WORD8 drc_data_type;
} ixheaac_drc_bs_data_struct;

typedef struct {
  ixheaac_drc_bs_data_struct str_drc_bs_data[MAX_BS_ELEMENT];
  ixheaac_drc_data_struct str_drc_channel_data[MAX_BS_ELEMENT];
  WORD16 drc_ref_level;
  WORD16 drc_def_level;
  UWORD8 drc_channel_next_index[MAX_BS_ELEMENT];
  UWORD8 sbr_allowed;
  UWORD8 sbr_found;
  UWORD8 drc_element_found;
  UWORD8 max_audio_channels;
  UWORD8 length_history;
  UWORD8 num_drc_elements;
  WORD32 state;
  WORD32 target_ref_level;
  WORD32 prog_ref_level;
  WORD32 cut_factor;
  WORD32 boost_factor;
  FLAG drc_dig_norm;
  FLAG drc_on;
  FLAG dvb_anc_data_present;
  WORD32 dvb_anc_data_pos;
  WORD32 pres_mode;
  WORD32 heavy_mode;
} ia_drc_dec_struct;

#endif
