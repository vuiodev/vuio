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
typedef struct {
  UWORD32 value;
  UWORD8 length;
} ixheaace_mps_huff_entry;

typedef struct {
  ixheaace_mps_huff_entry entry[2][2];
  ixheaace_mps_huff_entry escape;

} ixheaace_mps_lav1_2d;

typedef struct {
  ixheaace_mps_huff_entry entry[4][4];
  ixheaace_mps_huff_entry escape;

} ixheaace_mps_lav3_2d;

typedef struct {
  ixheaace_mps_huff_entry entry[6][6];
  ixheaace_mps_huff_entry escape;

} ixheaace_mps_lav5_2d;

typedef struct {
  ixheaace_mps_huff_entry entry[7][7];
  ixheaace_mps_huff_entry escape;

} ixheaace_mps_lav6_2d;

typedef struct {
  ixheaace_mps_huff_entry entry[8][8];
  ixheaace_mps_huff_entry escape;

} ixheaace_mps_lav7_2d;

typedef struct {
  ixheaace_mps_huff_entry entry[10][10];
  ixheaace_mps_huff_entry escape;

} ixheaace_mps_lav9_2d;

typedef struct {
  ixheaace_mps_huff_entry entry[13][13];
  ixheaace_mps_huff_entry escape;

} ixheaace_mps_lav12_2d;

typedef struct {
  ixheaace_mps_lav3_2d lav3;
  ixheaace_mps_lav5_2d lav5;
  ixheaace_mps_lav7_2d lav7;
  ixheaace_mps_lav9_2d lav9;

} ixheaace_mps_huff_cld_tab_2d;

typedef struct {
  ixheaace_mps_lav1_2d lav1;
  ixheaace_mps_lav3_2d lav3;
  ixheaace_mps_lav5_2d lav5;
  ixheaace_mps_lav7_2d lav7;

} ixheaace_mps_huff_icc_tab_2d;

typedef struct {
  ixheaace_mps_huff_entry h1_d[2][31];
  ixheaace_mps_huff_cld_tab_2d h2_d[2][2];

} ixheaace_mps_huff_cld_table;

typedef struct {
  ixheaace_mps_huff_entry h1_d[2][8];
  ixheaace_mps_huff_icc_tab_2d h2_d[2][2];

} ixheaace_mps_huff_icc_table;

typedef struct {
  ixheaace_mps_huff_entry cld[31];
  ixheaace_mps_huff_entry icc[8];

} ixheaace_mps_huff_pt0_table;

typedef struct {
  UWORD32 value[31];
  UWORD8 length[31];
} ixheaace_mps_sac_huff_cld_tab;

typedef struct {
  UWORD32 value[8];
  UWORD8 length[8];
} ixheaace_mps_sac_huff_icc_tab;

typedef struct {
  UWORD32 value[26];
  UWORD8 length[26];
} ixheaace_mps_sac_huff_cpc_tab;

typedef struct {
  ixheaace_mps_sac_huff_cld_tab huff_pt0;
  ixheaace_mps_sac_huff_cld_tab huff_diff[2];

} ixheaace_mps_sac_huffman_cld_table;

typedef struct {
  ixheaace_mps_sac_huff_icc_tab huff_pt0;
  ixheaace_mps_sac_huff_icc_tab huff_diff[2];

} ixheaace_mps_sac_huffman_icc_table;

typedef struct {
  ixheaace_mps_sac_huff_cpc_tab huff_pt0;
  ixheaace_mps_sac_huff_cpc_tab huff_diff[2];

} ixheaace_mps_sac_huff_cpc_table;

extern const ixheaace_mps_huff_cld_table ixheaace_mps_212_huff_cld_tab;
extern const ixheaace_mps_huff_icc_table ixheaace_mps_212_huff_icc_tab;
extern const ixheaace_mps_huff_pt0_table ixheaace_mps_212_huff_part_0_tab;
extern const ixheaace_mps_sac_huffman_cld_table ixheaace_mps_515_huff_cld_tab;
extern const ixheaace_mps_sac_huffman_icc_table ixheaace_mps_515_huff_icc_tab;
extern const ixheaace_mps_sac_huff_cpc_table ixheaace_mps_515_huff_cpc_tab;
