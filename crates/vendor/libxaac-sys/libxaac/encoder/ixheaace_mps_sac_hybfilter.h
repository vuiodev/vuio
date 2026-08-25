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
struct ixheaace_mps_hyb_filter_state {
  FLOAT32 buffer_lf_real[MAX_QMF_BANDS_TO_HYBRID][BUFFER_LEN_LF];
  FLOAT32 buffer_lf_imag[MAX_QMF_BANDS_TO_HYBRID][BUFFER_LEN_LF];
  FLOAT32 buffer_hf_real[NUM_QMF_BANDS][BUFFER_LEN_HF];
  FLOAT32 buffer_hf_imag[NUM_QMF_BANDS][BUFFER_LEN_HF];
};

typedef struct ixheaace_mps_hyb_filter_state ixheaace_mps_hyb_filter_state,
    *ixheaace_mps_pstr_hyb_filter_state;

VOID ixheaace_mps_515_apply_ana_hyb_filterbank(ixheaace_mps_pstr_hyb_filter_state hyb_state,
                                               FLOAT32 *m_qmf_real, FLOAT32 *m_qmf_imag,
                                               WORD32 nr_samples, FLOAT32 *m_hybrid_real,
                                               FLOAT32 *m_hybrid_imag);

VOID ixheaace_mps_515_apply_syn_hyb_filterbank(FLOAT32 *m_hybrid_real, FLOAT32 *m_hybrid_imag,
                                               WORD32 nr_samples, FLOAT32 *m_qmf_real,
                                               FLOAT32 *m_qmf_imag);
