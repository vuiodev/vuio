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
#ifndef IXHEAACD_MPS_RES_H
#define IXHEAACD_MPS_RES_H

#include "ixheaacd_error_codes.h"

#define AAC_DEC_OK IA_XHEAAC_DEC_API_NONFATAL_NO_ERROR
#define AAC_DEC_INVALID_CODE_BOOK IA_XHEAAC_DEC_EXE_NONFATAL_INVALID_CODE_BOOK
#define AAC_DEC_PREDICTION_NOT_SUPPORTED_IN_LC_AAC \
  IA_XHEAAC_DEC_EXE_NONFATAL_PREDICTION_DATA_PRESENT
#define AAC_DEC_UNIMPLEMENTED_GAIN_CONTROL_DATA \
  IA_XHEAAC_DEC_EXE_NONFATAL_GAIN_CONTROL_DATA_PRESENT
#define AAC_DEC_TNS_RANGE_ERROR IA_XHEAAC_DEC_EXE_FATAL_TNS_RANGE_ERROR
#define AAC_DEC_TNS_ORDER_ERROR IA_XHEAAC_DEC_EXE_NONFATAL_TNS_ORDER_ERROR

#endif /* IXHEAACD_MPS_RES_H */
