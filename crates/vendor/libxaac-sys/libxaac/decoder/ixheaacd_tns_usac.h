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
#ifndef IXHEAACD_TNS_USAC_H
#define IXHEAACD_TNS_USAC_H

#define TNS_MAX_BANDS 49
#define TNS_MAX_ORDER 31
#define TNS_MAX_WIN 8
#define TNS_MAX_FILT 3

typedef struct {
  WORD32 start_band;
  WORD32 stop_band;
  WORD32 order;
  WORD32 direction;
  WORD32 coef_compress;
  WORD16 coef[TNS_MAX_ORDER];

} ia_tns_filter_struct;

typedef struct {
  WORD32 n_filt;
  WORD32 coef_res;
  ia_tns_filter_struct str_filter[TNS_MAX_FILT];
} ia_tns_info_struct;

typedef struct {
  WORD32 n_subblocks;
  ia_tns_info_struct str_tns_info[TNS_MAX_WIN];
} ia_tns_frame_info_struct;

#endif /* IXHEAACD_TNS_USAC_H */
