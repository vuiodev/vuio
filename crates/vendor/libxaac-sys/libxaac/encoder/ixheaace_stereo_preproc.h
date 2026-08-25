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

typedef struct {
  FLOAT32 norm_pe_fac; /* factor to normalize input PE, depends on bitrate and bandwidth */
  FLOAT32 inc_stereo_attenuation; /* att. increment parameter */
  FLOAT32 dec_stereo_attenuation; /* att. decrement parameter */
  FLOAT32 average_freq_energy_l;  /* energy left */
  FLOAT32 average_freq_energy_r;  /* energy right */
  FLOAT32 average_freq_energy_m;  /* energy mid */
  FLOAT32 average_freq_energy_s;  /* energy side */
  FLOAT32 smoothed_pe_sum_sum;    /* time-smoothed PE */
  FLOAT32 avg_s_to_m;             /* time-smoothed energy ratio S/M [dB] */
  FLOAT32 last_l_to_r;            /* previous frame energy ratio L/R [dB] */
  FLOAT32 last_nrg_lr;            /* previous frame energy L+R */
  FLOAT32 impact_factor;          /* bitrate dependent parameter */
  FLOAT32 stereo_attenuation;     /* the actual attenuation of this frame */
  FLOAT32 stereo_attenuation_fac; /* the actual attenuation factor of this frame */
  /* tuning parameters that are not varied from frame to frame but initialized at init */
  WORD32 stereo_attenuation_flag; /* flag to indicate usage */
  FLOAT32 const_attenuation;      /* if not zero, a constant att. will be applied [dB]*/
  FLOAT32 stereo_attenuation_max; /* the max. attenuation [dB]*/
  FLOAT32 lr_min;                 /* tuning parameter [dB] */
  FLOAT32 lr_max;                 /* tuning parameter [dB] */
  FLOAT32 sm_min;                 /* tuning parameter [dB] */
  FLOAT32 sm_max;                 /* tuning parameter [dB] */
  FLOAT32 pe_min;                 /* tuning parameter */
  FLOAT32 pe_crit;                /* tuning parameter */
  FLOAT32 pe_impact_max;          /* tuning parameter */
} ixheaace_stereo_pre_pro_struct;

typedef ixheaace_stereo_pre_pro_struct *ixheaace_stereo_pre_pro_pstr;

IA_ERRORCODE iaace_init_stereo_pre_processing(ixheaace_stereo_pre_pro_pstr pstr_stereo_pre_pro,
                                        WORD32 no_channels, WORD32 bit_rate, WORD32 sample_rate,
                                        FLOAT32 used_scf_ratio);

VOID iaace_apply_stereo_preproc(ixheaace_stereo_pre_pro_pstr pstr_stereo_pre_pro,
                                WORD32 num_channels, ixheaace_element_info *pstr_elem_info,
                                FLOAT32 *ptr_time_data, WORD32 granule_len);

VOID iaace_update_stereo_pre_process(ixheaace_psy_out_channel **pstr_psy_out,
                                     ixheaace_qc_out_element *pstr_qc_out,
                                     ixheaace_stereo_pre_pro_pstr pstr_stereo_pre_pro,
                                     FLOAT32 weight_pe_fac);
