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
#define MAXIMUM_SECTIONS MAXIMUM_GROUPED_SCALE_FACTOR_BAND
#define SECT_ESC_VAL_LONG 31
#define SECT_ESC_VAL_SHORT 7
#define CODE_BCK_BITS 4
#define SECT_BITS_LONG 5
#define SECT_BITS_SHORT 3

typedef enum {
  SI_ID_BITS = (3),
  SI_FILL_COUNT_BITS = (4),
  SI_FILL_ESC_COUNT_BITS = (8),
  SI_FILL_EXTENTION_BITS = (4),
  SI_FILL_NIBBLE_BITS = (4),
  SI_SCE_BITS = (4),
  SI_CPE_BITS = (5),
  SI_CPE_MS_MASK_BITS = (2),
  SI_ICS_INFO_BITS_LONG = (1 + 2 + 1 + 6 + 1),
  SI_ICS_INFO_BITS_SHORT = (1 + 2 + 1 + 4 + 7),
  SI_ICS_BITS = (8 + 1 + 1 + 1),
} SI_BITS;
typedef struct {
  WORD8 code_book;
  WORD8 sfb_start;
  WORD8 sfb_cnt;
  WORD16 section_bits;
} ixheaace_section_info;

typedef struct {
  WORD32 block_type;
  WORD32 total_groups_cnt;
  WORD32 sfb_cnt;
  WORD32 max_sfb_per_grp;
  WORD32 sfb_per_group;
  WORD32 num_of_sections;
  ixheaace_section_info section[MAXIMUM_SECTIONS];
  WORD32 side_info_bits; /* sectioning bits       */
  WORD32 huffman_bits;   /* huffman    coded bits */
  WORD32 scale_fac_bits; /* scalefac   coded bits */
  WORD32 first_scf;      /* first scf to be coded */
} ixheaace_section_data;

VOID ia_enhaacplus_enc_bitcount_init(WORD32 *side_info_tab_long, WORD32 *side_info_tab_short);

IA_ERRORCODE ia_enhaacplus_enc_dyn_bitcount(
    const WORD16 *ptr_quant_spec, const UWORD16 *ptr_max_val_in_sfb, const WORD16 *ptr_scale_fac,
    const WORD32 block_type, const WORD32 sfb_cnt, const WORD32 max_sfb_per_grp,
    const WORD32 sfb_per_grp, const WORD32 *ptr_sfb_offset,
    ixheaace_section_data *pstr_section_data, WORD32 *ptr_side_info_tab_long,
    WORD32 *ptr_side_info_tab_short, ixheaace_huffman_tables *ptr_huffman_tbl,
    WORD32 *ptr_scratch_buf, WORD32 aot, WORD32 *bit_cnt);
