/******************************************************************************
 *                                                                            *
 * Copyright (C) 2024 The Android Open Source Project
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

#define IXHEAACE_LOUDNESS_NUM_TAPS (3)
#define IXHEAACE_LOUDNESS_PRE_FLT (0)
#define IXHEAACE_LOUDNESS_RLB_FLT (1)
#define IXHEAACE_LOUDNESS_DONT_PASS (-1)
#define IXHEAACE_ABS_GATE (-70)
#define IXHEAACE_MOMENTARY_LOUDNESS_OVERLAP (3)
#define IXHEAACE_SL_OVERLAP (20)
#define BYTE_ALIGN_8 (8)
#define MAX(a, b) ((a) > (b) ? (a) : (b))
#define IXHEAACE_DEFAULT_SHORT_TERM_LOUDENSS (-1000)
#define IXHEAACE_DEFAULT_MOMENTARY_LOUDENSS (-1000)
#define IXHEAACE_SEC_TO_100MS_FACTOR (60 * 10)
#define IXHEAACE_SUM_SQUARE_EPS (1/32768.0f * 1/32768.0f)

typedef struct {
  BOOL passes_abs_gate;
  FLOAT64 short_term_loudness;
  FLOAT64 int_val;
} short_term_frame_t;

typedef struct {
  BOOL passes_abs_gate;
  FLOAT64 momentary_loudness;
  FLOAT64 int_val;
} momentary_frame_t;
typedef struct {
  UWORD32 num_samples_per_ch;
  UWORD32 n_channels;
  UWORD32 length;
  UWORD32 sample_rate;
  UWORD32 pcm_sz;
  FLOAT64 sum_square;
  FLOAT64 prev_four_sum_square[4];
  FLOAT64 w[2][2][4];
  UWORD32 count_fn_call_mmstl;
  UWORD32 mom_loudness_first_time_flag;
  FLOAT64 average_loudness_val;
  FLOAT64 prev_thirty_sum_square[30];
  WORD32 sl_first_time_flag;
  WORD32 local_sl_count;
  UWORD32 short_term_loudness_overlap;
  short_term_frame_t stf_instances[100];
  UWORD32 no_of_stf;
  UWORD32 curr_stf_no;
  UWORD32 loop_curr_stf_no;
  UWORD32 no_of_stf_passing_abs_gate;
  FLOAT64 tot_int_val_stf_passing_abs_gate;
  FLOAT64 temp_stf_instances_loudness[100];
  BOOL get_LRA;
  UWORD32 max_lra_count;
  UWORD32 no_of_mf;

  UWORD32 no_of_mf_passing_abs_gate;
  FLOAT64 tot_int_val_mf_passing_abs_gate;
  UWORD32 no_of_mf_passing_rel_gate;
  FLOAT64 tot_int_val_mf_passing_rel_gate;
  FLOAT64 rel_gate;
  UWORD32 ml_count_fn_call;
  UWORD32 loop_ml_count_fn_call;
  momentary_frame_t mf_instances[1000];
  WORD32 get_intergrated_loudness;
  UWORD32 max_il_buf_size;
  FLOAT64 max_sample_val;
  WORD32 sample_rate_idx;
} ixheaace_loudness_struct;

IA_ERRORCODE ixheaace_loudness_init_params(pVOID loudness_handle,
                                           ixheaace_input_config *pstr_input_config,
                                           ixheaace_output_config *pstr_output_config);

FLOAT64 ixheaace_measure_loudness(pVOID loudness_handle, WORD16 **samples);

FLOAT64 ixheaace_measure_integrated_loudness(pVOID loudness_handle);

WORD32 ixheaace_loudness_info_get_handle_size();

FLOAT32 ixheaace_measure_sample_peak_value(pVOID loudness_handle);