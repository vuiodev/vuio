/******************************************************************************
 *
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
#ifndef IXHEAACD_MPS_BITDEC_H
#define IXHEAACD_MPS_BITDEC_H

enum { EXT_TYPE_0 = 0, EXT_TYPE_1 = 1, EXT_TYPE_2 = 2 };

enum { QUANT_MODE_0 = 0, QUANT_MODE_1 = 1, QUANT_MODE_2 = 2 };

enum { IN_CH_1 = 1, IN_CH_2 = 2, IN_CH_6 = 6 };

enum {
  TREE_5151 = 0,
  TREE_5152 = 1,
  TREE_525 = 2,
  TREE_7271 = 3,
  TREE_7272 = 4,
  TREE_7571 = 5,
  TREE_7572 = 6
};

enum {
  PARAMETER_BANDS_4 = 4,
  PARAMETER_BANDS_5 = 5,
  PARAMETER_BANDS_7 = 7,
  PARAMETER_BANDS_10 = 10,
  PARAMETER_BANDS_14 = 14,
  PARAMETER_BANDS_20 = 20,
  PARAMETER_BANDS_28 = 28,
  PARAMETER_BANDS_40 = 40
};

enum { SMOOTH_MODE_0 = 0, SMOOTH_MODE_1 = 1, SMOOTH_MODE_2 = 2, SMOOTH_MODE_3 = 3 };

enum {
  TTT_MODE_0 = 0,
  TTT_MODE_1 = 1,
  TTT_MODE_2 = 2,
  TTT_MODE_3 = 3,
  TTT_MODE_4 = 4,
  TTT_MODE_5 = 5
};

IA_ERRORCODE ixheaacd_parse_specific_config(ia_heaac_mps_state_struct *pstr_mps_state,
                                            WORD32 sac_header_len);

IA_ERRORCODE ixheaacd_default_specific_config(ia_heaac_mps_state_struct *pstr_mps_state,
                                              WORD32 sampling_freq);

IA_ERRORCODE ixheaacd_decode_frame(ia_heaac_mps_state_struct *pstr_mps_state);

IA_ERRORCODE ixheaacd_set_current_state_parameters(ia_heaac_mps_state_struct *pstr_mps_state);

VOID ixheaacd_get_dequant_tables(WORD32 **cld, WORD32 **icc, WORD32 **cpc,
                                 ia_mps_dec_bitdec_tables_struct *ixheaacd_mps_dec_bitdec_tables);

WORD32 ixheaacd_quantize_icc(WORD32 v,
                             ia_mps_dec_bitdec_tables_struct *ixheaacd_mps_dec_bitdec_tables);

WORD32 ixheaacd_quantize_cld(WORD32 v,
                             ia_mps_dec_bitdec_tables_struct *ixheaacd_mps_dec_bitdec_tables);

#endif /* IXHEAACD_MPS_BITDEC_H */
