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
#ifndef IXHEAACD_SBR_SCALE_H
#define IXHEAACD_SBR_SCALE_H

typedef struct {
  WORD16 lb_scale;
  WORD16 st_lb_scale;
  WORD16 ov_lb_scale;
  WORD16 hb_scale;
  WORD16 ov_hb_scale;
  WORD16 st_syn_scale;
  WORD16 ps_scale;
} ia_sbr_scale_fact_struct;

#endif
