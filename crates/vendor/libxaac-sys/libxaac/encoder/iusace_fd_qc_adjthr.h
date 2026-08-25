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

#define CLIP_SAVE_LO_TO_HI_LONG (CLIP_SAVE_HI_LONG - CLIP_SAVE_LO_LONG)
#define CLIP_SAVE_LO_TO_HI_SHORT (CLIP_SAVE_HI_SHORT - CLIP_SAVE_LO_SHORT)
#define CLIP_SPEND_LO_TO_HI_LONG (CLIP_SPEND_HI_LONG - CLIP_SPEND_LO_LONG)
#define CLIP_SPEND_LO_TO_HI_SHORT (CLIP_SPEND_HI_SHORT - CLIP_SPEND_LO_SHORT)
#define MIN_TO_MAX_SAVE_BITS_LONG (MAX_BITS_SAVE_LONG - MIN_BITS_SAVE_LONG)
#define MIN_TO_MAX_SAVE_BITS_SHORT (MAX_BITS_SAVE_SHORT - MIN_BITS_SAVE_SHORT)
#define MIN_TO_MAX_SPEND_BITS_LONG (MAX_BITS_SPEND_LONG - MIN_BITS_SPEND_LONG)
#define MIN_TO_MAX_SPEND_BITS_SHORT (MAX_BITS_SPEND_SHORT - MIN_BITS_SPEND_SHORT)
#define BITS_SAVE_RATIO_LONG (MIN_TO_MAX_SAVE_BITS_LONG / CLIP_SAVE_LO_TO_HI_LONG)
#define BITS_SAVE_RATIO_SHORT (MIN_TO_MAX_SAVE_BITS_SHORT / CLIP_SAVE_LO_TO_HI_SHORT)
#define BITS_SPEND_RATIO_LONG (MIN_TO_MAX_SPEND_BITS_LONG / CLIP_SPEND_LO_TO_HI_LONG)
#define BITS_SPEND_RATIO_SHORT (MIN_TO_MAX_SPEND_BITS_SHORT / CLIP_SPEND_LO_TO_HI_SHORT)

#define MAX_GAIN_INDEX (128)
#define MIN_GAIN_INDEX (-128)
#define SCF_COUNT_LIMIT_ONE (1)
#define SCF_COUNT_LIMIT_THREE (3)

typedef struct {
  FLOAT32 *sfb_ld_energy;
  FLOAT32 *sfb_lines;
  FLOAT32 sfb_pe[MAX_GROUPED_SFB_TEMP];
  FLOAT32 sfb_const_part[MAX_GROUPED_SFB_TEMP];
  FLOAT32 num_sfb_active_lines[MAX_GROUPED_SFB_TEMP];
  FLOAT32 pe;
  FLOAT32 const_part;
  FLOAT32 num_active_lines;
} ia_qc_pe_chan_data_struct;

typedef struct {
  ia_qc_pe_chan_data_struct pe_ch_data[30];
  FLOAT32 pe;
  FLOAT32 const_part;
  FLOAT32 num_active_lines;
  FLOAT32 offset;
} ia_qc_pe_data_struct;

enum ia_avoid_hole_state { NO_AH = 0, AH_INACTIVE = 1, AH_ACTIVE = 2 };

typedef enum {
  SI_ID_BITS = (3),
  SI_FILL_COUNT_BITS = (4),
  SI_FILL_ESC_COUNT_BITS = (8),
  SI_FILL_EXTENTION_BITS = (4),
  SI_FILL_NIBBLE_BITS = (4),
  SI_SCE_BITS = (4),
  SI_CPE_BITS = (5),
  SI_CPE_MS_MASK_BITS = (2),
  SI_ICS_INFO_BITS_LONG = (1 + 2 + 6),
  SI_ICS_INFO_BITS_SHORT = (1 + 2 + 4 + 7),
  SI_ICS_BITS = (8 + 1 + 1 + 1),
} SI_BITS;

FLOAT32 iusace_bits_to_pe(const FLOAT32 bits);

VOID iusace_adj_thr_init(ia_adj_thr_elem_struct *pstr_adj_thr_state, const FLOAT32 mean_pe,
                         WORD32 ch_bitrate);

IA_ERRORCODE iusace_adj_thr(ia_adj_thr_elem_struct *pstr_adj_thr_elem,
                            ia_psy_mod_out_data_struct *pstr_psy_out, FLOAT32 *ch_bit_dist,
                            ia_qc_out_data_struct *pstr_qc_out, const WORD32 avg_bits,
                            const WORD32 bitres_bits, const WORD32 max_bitres_bits,
                            const WORD32 side_info_bits, FLOAT32 *max_bit_fac,
                            WORD32 num_channels, WORD32 chn, iusace_scratch_mem *pstr_scratch);

VOID iusace_calc_form_fac_per_chan(ia_psy_mod_out_data_struct *pstr_psy_out_chan,
                                   iusace_scratch_mem *pstr_scratch, WORD32 i_ch);

VOID iusace_estimate_scfs_chan(ia_psy_mod_out_data_struct *pstr_psy_out,
                               ia_qc_out_chan_struct *str_qc_out_chan, WORD32 num_channels,
                               WORD32 chn, iusace_scratch_mem *pstr_scratch);

VOID iusace_quantize_lines(const WORD32 gain, const WORD32 num_lines, FLOAT32 *ptr_exp_spectrum,
                           WORD16 *ptr_quant_spectrum, FLOAT32 *ptr_mdct_spec);
