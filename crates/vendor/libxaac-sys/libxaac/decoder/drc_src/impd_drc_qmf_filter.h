/******************************************************************************
 *
 * Copyright (C) 2015 The Android Open Source Project
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
#ifndef IMPD_DRC_QMF_FILTER_H
#define IMPD_DRC_QMF_FILTER_H

#define QMF_NUM_FILT_BANDS 64
#define QMF_FILT_RESOLUTION 64

typedef struct ia_drc_qmf_filt_struct {
  FLOAT64* ana_buff;
  FLOAT64* syn_buff;
  FLOAT64 ana_tab_real[QMF_NUM_FILT_BANDS][2 * QMF_NUM_FILT_BANDS];
  FLOAT64 ana_tab_imag[QMF_NUM_FILT_BANDS][2 * QMF_NUM_FILT_BANDS];
  FLOAT64 syn_tab_real[2 * QMF_NUM_FILT_BANDS][QMF_NUM_FILT_BANDS];
  FLOAT64 syn_tab_imag[2 * QMF_NUM_FILT_BANDS][QMF_NUM_FILT_BANDS];

} ia_drc_qmf_filt_struct;

#endif
