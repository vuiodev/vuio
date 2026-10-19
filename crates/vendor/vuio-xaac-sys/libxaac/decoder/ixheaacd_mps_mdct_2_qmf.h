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
#ifndef IXHEAACD_MPS_MDCT_2_QMF_H
#define IXHEAACD_MPS_MDCT_2_QMF_H

#define AAC_FRAME_LENGTH (1024)
#define AAC_SHORT_FRAME_LENGTH (128)

#define MDCT_LENGTH_LO (AAC_FRAME_LENGTH)
#define MDCT_LENGTH_HI (AAC_FRAME_LENGTH << 1)
#define MDCT_LENGTH_SF (3 * AAC_SHORT_FRAME_LENGTH)

#define UPD_QMF_15 (15)
#define UPD_QMF_16 (16)
#define UPD_QMF_18 (18)
#define UPD_QMF_24 (24)
#define UPD_QMF_30 (30)
#define UPD_QMF_32 (32)

#define TSX2_4 (4)
#define TSX2_6 (6)
#define TSX2_8 (8)
#define TSX2_30 (30)
#define TSX2_32 (32)
#define TSX2_36 (36)
#define TSX2_48 (48)
#define TSX2_60 (60)
#define TSX2_64 (64)
#define TS_2 (2)
#define TS_4 (4)
#define TS_MINUS_ONE_4 (3)
#define TS_MINUS_ONE_15 (14)
#define TS_MINUS_ONE_16 (15)
#define TS_MINUS_ONE_18 (17)
#define TS_MINUS_ONE_24 (23)
#define TS_MINUS_ONE_30 (29)
#define TS_MINUS_ONE_32 (31)
#define ZERO (0)

IA_ERRORCODE ixheaacd_mdct2qmf_create(ia_heaac_mps_state_struct *pstr_mps_state);

VOID ixheaacd_mdct2qmf_process(WORD32 upd_qmf, WORD32 *const mdct_in, WORD32 *qmf_real_pre,
                               WORD32 *qmf_real_post, WORD32 *qmf_imag_pre, WORD32 *qmf_imag_post,
                               WORD32 const window_type, WORD32 qmf_global_offset,
                               ia_mps_dec_mps_tables_struct *ia_mps_dec_mps_table_ptr,
                               VOID *scratch, WORD32 time_slots);

#endif /* IXHEAACD_MPS_MDCT_2_QMF_H */
