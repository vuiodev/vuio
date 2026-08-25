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
  IAAC_ELDEXT_TERM = 0x0,         /* Termination tag */
  IAAC_ELDEXT_SAOC = 0x1,         /* SAOC config */
  IAAC_ELDEXT_LDSAC = 0x2,        /* LD MPEG Surround config */
  IAAC_ELDEXT_DOWNSCALEINFO = 0x3 /* ELD sample rate adaptation */
                                  /* reserved */
} IAAC_ASC_ELD_EXT_TYPE;

typedef struct {
  // GA specific parameters
  WORD32 depends_on_core_coder;
  WORD32 core_coder_delay;
  WORD32 extension_flag;

  // common to LD and ELD
  WORD32 frame_length_flag;
  WORD32 aac_sec_data_resilience_flag;
  WORD32 aac_sf_data_resilience_flag;
  WORD32 aac_spec_data_resilience_flag;

  // ELD specific parameters
  WORD32 ld_sbr_present_flag;
  WORD32 ld_sbr_sample_rate;
  WORD32 ld_sbr_crc_flag;
  WORD32 eld_ext_type[16];
  WORD32 eld_ext_len[16];
  ixheaace_pstr_sbr_hdr_data sbr_config;
  WORD32 num_sac_cfg_bits;
  UWORD8 sac_cfg_data[1024];
} ia_aace_config_struct;
