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

#define IXHEAACE_MAXIMUM_OCTAVE (29)
#define IXHEAACE_MAXIMUM_SECOND_REGION (50)

IA_ERRORCODE
ixheaace_update_freq_scale(UWORD8 *ptr_k_master, WORD32 *ptr_num_bands, const WORD32 k0,
                           const WORD32 k2, const WORD32 freq_scale, const WORD32 alter_scale,
                           ixheaace_sr_mode sbr_rate);

VOID ixheaace_update_high_res(UWORD8 *ptr_hires, WORD32 *ptr_num_hires, UWORD8 *ptr_k_master,
                              WORD32 num_master, WORD32 *ptr_xover_band,
                              ixheaace_sr_mode dr_or_sr, WORD32 num_qmf_ch);

VOID ixheaace_update_low_res(UWORD8 *ptr_lores, WORD32 *ptr_num_lores, UWORD8 *ptr_hires,
                             WORD32 ptr_num_hires);

IA_ERRORCODE
ixheaace_find_start_and_stop_band(const WORD32 sampling_freq, const WORD32 num_channels,
                                  const WORD32 start_freq, const WORD32 stop_freq,
                                  const ixheaace_sr_mode sample_rate_mode, WORD32 *ptr_k0,
                                  WORD32 *ptr_k2, WORD32 sbr_ratio_idx,
                                  ixheaace_sbr_codec_type sbr_codec);

WORD32 ixheaace_get_sbr_start_freq_raw(WORD32 start_freq, WORD32 qmf_bands, WORD32 fs);
