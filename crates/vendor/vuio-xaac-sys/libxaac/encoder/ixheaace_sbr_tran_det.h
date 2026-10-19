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

#define IXHEAACE_SBR_TRAN_ABS_THR (128000.0f)
#define IXHEAACE_SBR_TRAN_STD_FAC (1.0f)
#define IXHEAACE_SBR_ENERGY_THRESHOLD (65536 * 31)

#define IXHEAACE_TRANSIENT_THRESHOLD (5.0f)
#define IXHEAACE_SMALL_ENERGY (1e-2f)

typedef struct {
  FLOAT32 sbr_transients[3 * QMF_TIME_SLOTS_USAC_4_1];
  FLOAT32 *ptr_transients;
  FLOAT32 sbr_thresholds[IXHEAACE_QMF_CHANNELS];
  FLOAT32 *ptr_thresholds;
  FLOAT32 tran_thr;
  FLOAT32 split_thr;
  WORD32 tran_fc;
  WORD32 buffer_length;
  WORD32 no_cols;
  WORD32 no_rows;
  WORD32 mode;
  FLOAT32 prev_low_band_energy;
  FLOAT32 delta_energy[34];
  FLOAT32 energy[34];
  FLOAT32 coeff[64];
  WORD32 energy_slots[34];
  WORD32 delta_slots[34];
  WORD32 buffer_size;
  WORD32 look_ahead;
  WORD32 time_slots;
  WORD32 start_band;
  WORD32 stop_band;
} ixheaace_str_sbr_trans_detector;

typedef ixheaace_str_sbr_trans_detector *ixheaace_pstr_sbr_trans_detector;

VOID ixheaace_detect_transient(FLOAT32 **ptr_energies,
                               ixheaace_pstr_sbr_trans_detector pstr_sbr_trans_det,
                               WORD32 *ptr_tran_vector, WORD32 time_step,
                               ixheaace_sbr_codec_type sbr_codec);

VOID ixheaace_detect_transient_4_1(FLOAT32 **ptr_energies,
                                   ixheaace_pstr_sbr_trans_detector pstr_sbr_trans_det,
                                   WORD32 *ptr_tran_vector, WORD32 time_step);

VOID ixheaace_detect_transient_eld(FLOAT32 **ptr_energies,
                                   ixheaace_pstr_sbr_trans_detector pstr_sbr_trans_det,
                                   WORD32 *ptr_tran_vector);

VOID ixheaace_create_sbr_transient_detector(
    ixheaace_pstr_sbr_trans_detector pstr_sbr_trans_detector, WORD32 sample_freq,
    WORD32 total_bitrate, WORD32 codec_bitrate, WORD32 tran_thr, WORD32 mode, WORD32 tran_fc,
    WORD32 frame_flag_480, WORD32 is_ld_sbr, WORD32 sbr_ratio_idx,
    ixheaace_sbr_codec_type sbr_codec, WORD32 start_band);

IA_ERRORCODE
ixheaace_frame_splitter(FLOAT32 **ptr_energies,
                        ixheaace_pstr_sbr_trans_detector pstr_sbr_trans_detector,
                        UWORD8 *ptr_freq_band_tab, WORD32 num_scf, WORD32 time_step,
                        WORD32 no_cols, WORD32 *ptr_tran_vector,
                        FLOAT32 *ptr_frame_splitter_scratch, WORD32 is_ld_sbr);
