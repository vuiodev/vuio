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
#ifndef IXHEAACD_MPS_HUFF_TAB_H
#define IXHEAACD_MPS_HUFF_TAB_H

typedef struct { WORD32 node_tab[39][2]; } ia_huff_res_nodes_struct;

typedef struct { WORD32 node_tab[30][2]; } ia_huff_cld_node_1d_struct;

typedef struct { WORD32 node_tab[7][2]; } ia_huff_icc_node_1d_struct;

typedef struct { WORD32 node_tab[50][2]; } ia_huff_cpc_node_1d_struct;

typedef struct {
  WORD32 lav3[15][2];
  WORD32 lav5[35][2];
  WORD32 lav7[63][2];
  WORD32 lav9[99][2];
} ia_huff_cld_node_2d_struct;

typedef struct {
  WORD32 lav1[3][2];
  WORD32 lav3[15][2];
  WORD32 lav5[35][2];
  WORD32 lav7[63][2];
} ia_huff_icc_node_2d_struct;

typedef struct {
  WORD32 lav3[15][2];
  WORD32 lav6[48][2];
  WORD32 lav9[99][2];
  WORD32 lav12[168][2];
} ia_mps_dec_huff_cpc_nod_2d;

typedef struct {
  ia_huff_cld_node_1d_struct h_1_dim[3];
  ia_huff_cld_node_2d_struct h_2_dim[3][2];

} ia_huff_cld_nodes_struct;

typedef struct {
  ia_huff_icc_node_1d_struct h_1_dim[3];
  ia_huff_icc_node_2d_struct h_2_dim[3][2];

} ia_huff_icc_nodes_struct;

typedef struct {
  ia_huff_cpc_node_1d_struct h_1_dim[3];
  ia_mps_dec_huff_cpc_nod_2d h_2_dim[3][2];

} ia_huff_cpc_nodes_struct;

typedef struct {
  WORD32 cld[30][2];
  WORD32 icc[7][2];
  WORD32 cpc[25][2];

} ia_huff_pt0_nodes_struct;

typedef struct { WORD32 node_tab[3][2]; } ia_huff_lav_nodes_struct;

typedef struct { WORD32 node_tab[7][2]; } ia_huff_ipd_node_1d_struct;

typedef struct {
  WORD32 lav1[3][2];
  WORD32 lav3[15][2];
  WORD32 lav5[35][2];
  WORD32 lav7[63][2];
} ia_huff_ipd_node_2d_struct;

typedef struct {
  ia_huff_ipd_node_1d_struct hp0;
  ia_huff_ipd_node_1d_struct h_1_dim[3];
  ia_huff_ipd_node_2d_struct h_2_dim[3][2];

} ia_huff_ipd_nodes_struct;

#endif /* IXHEAACD_MPS_HUFF_TAB_H */
