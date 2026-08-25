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
#ifndef IXHEAACD_ENV_EXTR_PART_H
#define IXHEAACD_ENV_EXTR_PART_H

#define SBR_NOT_INITIALIZED 0
#define UPSAMPLING 1
#define SBR_ACTIVE 2

#define SBR_MONO 1
#define SBR_STEREO 2
#define PS_STEREO 3

#define SBR_RESET 1

typedef struct {
  WORD16 num_sf_bands[2];
  WORD16 num_nf_bands;
  WORD16 num_mf_bands;
  WORD16 sub_band_start;
  WORD16 sub_band_end;
  WORD16 freq_band_tbl_lim[MAX_NUM_LIMITERS + 1];
  WORD16 num_lf_bands;
  WORD16 num_if_bands;
  WORD16 *freq_band_table[2];
  WORD16 freq_band_tbl_lo[MAX_FREQ_COEFFS / 2 + 1];
  WORD16 freq_band_tbl_hi[MAX_FREQ_COEFFS + 1];
  WORD16 freq_band_tbl_noise[MAX_NOISE_COEFFS + 1];
  WORD16 f_master_tbl[MAX_FREQ_COEFFS + 1];

  WORD16 qmf_sb_prev;
} ia_freq_band_data_struct;

typedef struct {
  WORD32 sync_state;
  FLAG err_flag;
  FLAG err_flag_prev;
  WORD16 num_time_slots;
  WORD16 time_step;
  WORD16 core_frame_size;
  WORD32 out_sampling_freq;

  WORD32 channel_mode;
  WORD16 amp_res;

  WORD16 start_freq;
  WORD16 stop_freq;
  WORD16 xover_band;
  WORD16 freq_scale;
  WORD16 alter_scale;
  WORD16 noise_bands;

  WORD16 limiter_bands;
  WORD16 limiter_gains;
  WORD16 interpol_freq;
  WORD16 smoothing_mode;
  ia_freq_band_data_struct *pstr_freq_band_data;

  WORD16 header_extra_1;
  WORD16 header_extra_2;
  WORD16 pre_proc_flag;

  WORD32 status;

  WORD32 sbr_ratio_idx;
  WORD32 upsamp_fac;
  WORD32 is_usf_4;
  WORD32 output_framesize;
  WORD32 usac_independency_flag;
  FLAG pvc_flag;
  FLAG hbe_flag;

  WORD32 esbr_start_up;
  WORD32 esbr_start_up_pvc;
  WORD32 usac_flag;
  UWORD8 pvc_mode;

  FLAG enh_sbr;
  FLAG esbr_hq;
  FLAG enh_sbr_ps;
  FLAG eld_sbr;

} ia_sbr_header_data_struct;

typedef struct {
  WORD16 frame_class;
  WORD16 num_env;
  WORD16 transient_env;
  WORD16 num_noise_env;
  WORD16 border_vec[MAX_ENVELOPES + 1];
  WORD16 freq_res[MAX_ENVELOPES];
  WORD16 noise_border_vec[MAX_NOISE_ENVELOPES + 1];
} ia_frame_info_struct;

#endif
