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
#ifndef IXHEAACD_MPS_NLC_DEC_H
#define IXHEAACD_MPS_NLC_DEC_H

#define CLD (0)
#define ICC (1)
#define IPD (2)
#define CPC (3)

#define BACKWARDS (0)
#define FORWARDS (1)

#define DIFF_FREQ (0)
#define DIFF_TIME (1)

#define HUFF_1D (0)
#define HUFF_2D (1)

#define FREQ_PAIR (0)
#define TIME_PAIR (1)

#define PAIR_SHIFT 4
#define PAIR_MASK 0xf

#define PCM_PLT 0x2

#define MAXPARAM MAX_NUM_PARAMS
#define MAXBANDS MAX_PARAMETER_BANDS

#define ia_huff_node_struct const WORD32(*)[][2]

WORD32 ixheaacd_mps_ecdatapairdec(ia_bit_buf_struct *it_bit_buf, WORD32 aa_out_data[][MAXBANDS],
                                  WORD32 a_history[MAXBANDS], WORD32 data_type, WORD32 set_idx,
                                  WORD32 start_band, WORD32 data_bands, WORD32 pair_flag,
                                  WORD32 coarse_flag, WORD32 independency_flag, WORD32 ldmps_flag,
                                  WORD32 heaac_mps_present, WORD32 ec_flag);

VOID ixheaacd_mps_huff_decode(ia_bit_buf_struct *it_bit_buf, WORD32 *out_data,
                              WORD32 num_val);

enum { LAV_3 = 3, LAV_5 = 5, LAV_6 = 6, LAV_7 = 7, LAV_9 = 9, LAV_12 = 12 };

#endif /* IXHEAACD_MPS_NLC_DEC_H */
