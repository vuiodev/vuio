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
#define NB_SPHERE 32
#define LEN_ABS_LEADER 37
#define LEN_SIGN_LEADER 226

VOID iusace_find_nearest_neighbor(FLOAT32 *bk, WORD32 *ck);
VOID iusace_apply_voronoi_ext(WORD32 *x, WORD32 *n, WORD32 *idx, WORD32 *k);
VOID iusace_alg_vec_quant(FLOAT32 *ptr_input, WORD32 *ptr_out, WORD32 *ptr_lpc_idx);

extern const WORD32 iusace_pow2_table[8];
extern const WORD32 iusace_factorial_table[8];
extern const WORD32 iusace_iso_code_index_table[LEN_ABS_LEADER];
extern const UWORD8 iusace_iso_code_data_table[LEN_SIGN_LEADER];
extern const UWORD32 iusace_signed_leader_is[LEN_SIGN_LEADER];
extern const WORD32 iusace_da_nq[], iusace_da_pos[], iusace_da_num_bits[];
extern const UWORD32 iusace_da_id[];
extern const FLOAT32 iusace_wlsf_factor_table[4];
extern const FLOAT32 iusace_dico_lsf_abs_8b_flt[16 * 256];
