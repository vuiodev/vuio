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
#include <stdlib.h>
#include <string.h>
#include <math.h>
#include <float.h>
#include "ixheaac_type_def.h"
#include "ixheaac_error_standards.h"
#include "ixheaace_error_codes.h"
#include "ixheaace_api.h"
#include "ixheaace_loudness_measurement.h"
#include "iusace_cnst.h"
#include "ixheaac_constants.h"

FLOAT64 a_coeff_pre_flt[12][3] = {
    {0, -1.84460946989011, 0.85584332293064},  /* 96000Hz sample_rate*/
    {0, -1.83091998796233, 0.84414226108785},  /* 88200Hz sample_rate*/
    {0, -1.76738637827624, 0.79175893605869},  /* 64000Hz sample_rate*/
    {0, -1.69065929318241, 0.73248077421585},  /* 48000Hz sample_rate*/
    {0, -1.66365511325602, 0.71259542807323},  /* 44100Hz sample_rate*/
    {0, -1.53904509625064, 0.62696685598156},  /* 32000Hz sample_rate*/
    {0, -1.39023460519282, 0.53683848126040},  /* 24000Hz sample_rate*/
    {0, -1.33830533606613, 0.50824455891360},  /* 22050Hz sample_rate*/
    {0, -1.10153376910699, 0.39491236874986},  /* 16000Hz sample_rate*/
    {0, -0.82398044060334, 0.29429059828526},  /* 12000Hz sample_rate*/
    {0, -0.73075690963163, 0.26764083061798},  /* 11025Hz sample_rate*/
    {0, -0.29338078241492, 0.18687510604541}}; /* 8000Hz sample_rate*/

FLOAT64 b_coeff_pre_flt[12][3] = {
    {1.55971422897580, -2.92674157825108, 1.37826120231582},  /* 96000Hz sample_rate*/
    {1.55751537557965, -2.90562707992635, 1.36133397747221},  /* 88200Hz sample_rate*/
    {1.54734277602520, -2.80819560855113, 1.28522539030837},  /* 64000Hz sample_rate*/
    {1.53512485958697, -2.69169618940638, 1.19839281085285},  /* 48000Hz sample_rate*/
    {1.53084123005034, -2.65097999515473, 1.16907907992158},  /* 44100Hz sample_rate*/
    {1.51117789956876, -2.46488941336014, 1.04163327352229},  /* 32000Hz sample_rate*/
    {1.48790022096228, -2.24620546814114, 0.90490912324644},  /* 24000Hz sample_rate*/
    {1.47982535097775, -2.17072861285683, 0.86084248472655},  /* 22050Hz sample_rate*/
    {1.44329522349136, -1.83157538126046, 0.68165875741197},  /* 16000Hz sample_rate*/
    {1.40101638596118, -1.44343141964020, 0.51272519136094},  /* 12000Hz sample_rate*/
    {1.38693639705635, -1.31515305817747, 0.46510058210747},  /* 11025Hz sample_rate*/
    {1.32162356892998, -0.72625549131569, 0.29812624601620}}; /* 8000Hz sample_rate*/

FLOAT64 a_coeff_RLB_flt[12][3] = {
    {0, -1.99501754472472, 0.99502375904092},  /* 96000Hz sample_rate*/
    {0, -1.99457751545034, 0.99458487587805},  /* 88200Hz sample_rate*/
    {0, -1.99253095794890, 0.99254492277827},  /* 64000Hz sample_rate*/
    {0, -1.99004745483398, 0.99007225036621},  /* 48000Hz sample_rate*/
    {0, -1.98916967362979, 0.98919903578704},  /* 44100Hz sample_rate*/
    {0, -1.98508966898868, 0.98514532066955},  /* 32000Hz sample_rate*/
    {0, -1.98014412622893, 0.98024281785928},  /* 24000Hz sample_rate*/
    {0, -1.97839760259012, 0.97851441950325},  /* 22050Hz sample_rate*/
    {0, -1.97028952800443, 0.97051049053584},  /* 16000Hz sample_rate*/
    {0, -1.96048317995201, 0.96087407552357},  /* 12000Hz sample_rate*/
    {0, -1.95712192483092, 0.95758214578578},  /* 11025Hz sample_rate*/
    {0, -1.94101334282922, 0.94188430416850}}; /* 8000Hz sample_rate*/

FLOAT64 b_coeff_RLB_flt[12][3] = {
    {1.00247575433736, -2.00497008989074, 1.00247575433736},  /* 96000Hz sample_rate*/
    {1.00225631275593, -2.00453006061636, 1.00225631275593},  /* 88200Hz sample_rate*/
    {1.00123633620603, -2.00248350311492, 1.00123633620603},  /* 64000Hz sample_rate*/
    {1.0, -2.0, 1.0},                                         /* 48000Hz sample_rate*/
    {0.99956006454251, -1.9991201290850, 0.99956006454251},   /* 44100Hz sample_rate*/
    {0.99751647782627, -1.9950329556525, 0.99751647782627},   /* 32000Hz sample_rate*/
    {0.99508528374654, -1.99009667139495, 0.99508528374654},  /* 24000Hz sample_rate*/
    {0.99422108456853, -1.98835014775614, 0.99422108456853},  /* 22050Hz sample_rate*/
    {0.99021912008482, -1.98024207317045, 0.99021912008482},  /* 16000Hz sample_rate*/
    {0.98540091257869, -1.97043572511803, 0.98540091257869},  /* 12000Hz sample_rate*/
    {0.98375494770979, -1.96707446999694, 0.98375494770979},  /* 11025Hz sample_rate*/
    {0.97590602690115, -1.95096588799524, 0.97590602690115}}; /* 8000Hz sample_rate*/

static WORD32 ixheaace_map_sample_rate(WORD32 sample_rate,
                                       ixheaace_loudness_struct *pstr_loudness_hdl) {
  WORD32 mapped_sample_rate = sample_rate;

  if ((mapped_sample_rate >= 0) && (mapped_sample_rate < 9391)) {
    mapped_sample_rate = 8000;
    pstr_loudness_hdl->sample_rate_idx = 11;
  } else if ((mapped_sample_rate >= 9391) && (mapped_sample_rate < 11502)) {
    mapped_sample_rate = 11025;
    pstr_loudness_hdl->sample_rate_idx = 10;
  } else if ((mapped_sample_rate >= 11502) && (mapped_sample_rate < 13856)) {
    mapped_sample_rate = 12000;
    pstr_loudness_hdl->sample_rate_idx = 9;
  } else if ((mapped_sample_rate >= 13856) && (mapped_sample_rate < 18783)) {
    mapped_sample_rate = 16000;
    pstr_loudness_hdl->sample_rate_idx = 8;
  } else if ((mapped_sample_rate >= 18783) && (mapped_sample_rate < 23004)) {
    mapped_sample_rate = 22050;
    pstr_loudness_hdl->sample_rate_idx = 7;
  } else if ((mapped_sample_rate >= 23004) && (mapped_sample_rate < 27713)) {
    mapped_sample_rate = 24000;
    pstr_loudness_hdl->sample_rate_idx = 6;
  } else if ((mapped_sample_rate >= 27713) && (mapped_sample_rate < 37566)) {
    mapped_sample_rate = 32000;
    pstr_loudness_hdl->sample_rate_idx = 5;
  } else if ((mapped_sample_rate >= 37566) && (mapped_sample_rate < 46009)) {
    mapped_sample_rate = 44100;
    pstr_loudness_hdl->sample_rate_idx = 4;
  } else if ((mapped_sample_rate >= 46009) && (mapped_sample_rate < 55426)) {
    mapped_sample_rate = 48000;
    pstr_loudness_hdl->sample_rate_idx = 3;
  } else if ((mapped_sample_rate >= 55426) && (mapped_sample_rate < 75132)) {
    mapped_sample_rate = 64000;
    pstr_loudness_hdl->sample_rate_idx = 2;
  } else if ((mapped_sample_rate >= 75132) && (mapped_sample_rate < 92017)) {
    mapped_sample_rate = 88200;
    pstr_loudness_hdl->sample_rate_idx = 1;
  } else if (mapped_sample_rate >= 92017) {
    mapped_sample_rate = 96000;
    pstr_loudness_hdl->sample_rate_idx = 0;
  } else {
    mapped_sample_rate = 48000;
    pstr_loudness_hdl->sample_rate_idx = 3;
  }
  return mapped_sample_rate;
}

WORD32 ixheaace_loudness_info_get_handle_size() {
  return IXHEAAC_GET_SIZE_ALIGNED(sizeof(ixheaace_loudness_struct), BYTE_ALIGN_8);
}
IA_ERRORCODE ixheaace_loudness_init_params(pVOID loudness_handle,
                                           ixheaace_input_config *pstr_input_config,
                                           ixheaace_output_config *pstr_output_config) {
  UWORD32 count = 0;
  UWORD8 temp_count = 0;
  IA_ERRORCODE err_code = IA_NO_ERROR;
  ixheaace_loudness_struct *pstr_loudness_hdl = (ixheaace_loudness_struct *)loudness_handle;
  memset(pstr_loudness_hdl, 0, sizeof(ixheaace_loudness_struct));
  pstr_loudness_hdl->sample_rate =
      ixheaace_map_sample_rate(pstr_input_config->i_samp_freq, pstr_loudness_hdl);

  pstr_output_config->samp_freq = pstr_loudness_hdl->sample_rate;
  pstr_loudness_hdl->length = pstr_input_config->aac_config.length;
  pstr_loudness_hdl->pcm_sz = pstr_input_config->ui_pcm_wd_sz;
  if (pstr_loudness_hdl->pcm_sz != 16) {
    return (IA_EXHEAACE_CONFIG_FATAL_PCM_WDSZ);
  }
  pstr_loudness_hdl->n_channels = pstr_input_config->i_channels;
  if (pstr_loudness_hdl->n_channels > 2 || pstr_loudness_hdl->n_channels < 1) {
    return (IA_EXHEAACE_CONFIG_FATAL_NUM_CHANNELS);
  }
  pstr_loudness_hdl->num_samples_per_ch = (pstr_loudness_hdl->sample_rate / 10);
  pstr_loudness_hdl->sum_square = 0;
  pstr_loudness_hdl->mom_loudness_first_time_flag = 1;

  pstr_loudness_hdl->count_fn_call_mmstl = 0;
  pstr_loudness_hdl->sl_first_time_flag = 1;
  pstr_loudness_hdl->local_sl_count = 0;
  pstr_loudness_hdl->short_term_loudness_overlap = IXHEAACE_SL_OVERLAP;

  pstr_loudness_hdl->no_of_mf = IXHEAACE_SEC_TO_100MS_FACTOR;
  pstr_loudness_hdl->no_of_mf -= IXHEAACE_MOMENTARY_LOUDNESS_OVERLAP;
  pstr_loudness_hdl->no_of_stf =
      (((pstr_loudness_hdl->no_of_mf + IXHEAACE_MOMENTARY_LOUDNESS_OVERLAP) - 30) /
       (30 - pstr_loudness_hdl->short_term_loudness_overlap)) +
      1;

  pstr_loudness_hdl->tot_int_val_stf_passing_abs_gate = 0;
  pstr_loudness_hdl->curr_stf_no = 0;
  pstr_loudness_hdl->loop_curr_stf_no = 0;
  pstr_loudness_hdl->no_of_stf_passing_abs_gate = 0;
  pstr_loudness_hdl->max_lra_count = pstr_loudness_hdl->no_of_stf;
  pstr_loudness_hdl->loop_ml_count_fn_call = 0;
  pstr_loudness_hdl->ml_count_fn_call = 0;
  pstr_loudness_hdl->max_il_buf_size = pstr_loudness_hdl->no_of_mf;
  pstr_loudness_hdl->get_intergrated_loudness = 1;
  pstr_loudness_hdl->max_sample_val = FLT_EPSILON;

  for (count = 0; count < pstr_loudness_hdl->n_channels; count++) {
    for (temp_count = 0; temp_count < IXHEAACE_LOUDNESS_NUM_TAPS; temp_count++) {
      pstr_loudness_hdl->w[0][count][temp_count] = 0;
      pstr_loudness_hdl->w[1][count][temp_count] = 0;
    }
  }

  for (count = 0; count < 4; count++) {
    pstr_loudness_hdl->prev_four_sum_square[count] = 0;
  }

  for (count = 0; count < 30; count++) {
    pstr_loudness_hdl->prev_thirty_sum_square[count] = 0;
  }

  count = 0;
  while (count < (pstr_loudness_hdl->no_of_stf)) {
    pstr_loudness_hdl->temp_stf_instances_loudness[count] = 0;
    count++;
  }

  count = 0;
  while (count < (pstr_loudness_hdl->no_of_stf)) {
    pstr_loudness_hdl->stf_instances[count].short_term_loudness =
        IXHEAACE_DEFAULT_SHORT_TERM_LOUDENSS;
    pstr_loudness_hdl->stf_instances[count].int_val = 0;
    pstr_loudness_hdl->stf_instances[count].passes_abs_gate = FALSE;
    count++;
  }

  count = 0;
  while (count < pstr_loudness_hdl->no_of_mf) {
    pstr_loudness_hdl->mf_instances[count].momentary_loudness =
        IXHEAACE_DEFAULT_MOMENTARY_LOUDENSS;
    pstr_loudness_hdl->mf_instances[count].int_val = 0.0;
    pstr_loudness_hdl->mf_instances[count].passes_abs_gate = FALSE;
    count++;
  }
  return err_code;
}

static FLOAT64 ixheaace_loudness_gen_flt(FLOAT64 *a, FLOAT64 *b, FLOAT64 *w, FLOAT64 input) {
  FLOAT64 output = 0;
  UWORD8 count;

  for (count = 0; count <= IXHEAACE_LOUDNESS_NUM_TAPS - 2; count++) {
    w[count] = w[count + 1];
  }
  w[IXHEAACE_LOUDNESS_NUM_TAPS - 1] = 0;

  for (count = 1; count <= IXHEAACE_LOUDNESS_NUM_TAPS - 1; count++) {
    w[IXHEAACE_LOUDNESS_NUM_TAPS - 1] += a[count] * w[IXHEAACE_LOUDNESS_NUM_TAPS - count - 1];
  }
  w[IXHEAACE_LOUDNESS_NUM_TAPS - 1] = input - w[IXHEAACE_LOUDNESS_NUM_TAPS - 1];

  for (count = 0; count <= IXHEAACE_LOUDNESS_NUM_TAPS - 1; count++) {
    output += b[count] * w[IXHEAACE_LOUDNESS_NUM_TAPS - count - 1];
  }

  return output;
}

static FLOAT64 ixheaace_loudness_k_flt(FLOAT64 input, ixheaace_loudness_struct *pstr_loudness_hdl,
                                       UWORD8 channel_no) {
  FLOAT64 temp;
  temp = ixheaace_loudness_gen_flt(a_coeff_pre_flt[pstr_loudness_hdl->sample_rate_idx],
                                   b_coeff_pre_flt[pstr_loudness_hdl->sample_rate_idx],
                                   pstr_loudness_hdl->w[IXHEAACE_LOUDNESS_PRE_FLT][channel_no],
                                   input);

  temp = ixheaace_loudness_gen_flt(a_coeff_RLB_flt[pstr_loudness_hdl->sample_rate_idx],
                                   b_coeff_RLB_flt[pstr_loudness_hdl->sample_rate_idx],
                                   pstr_loudness_hdl->w[IXHEAACE_LOUDNESS_RLB_FLT][channel_no],
                                   temp);

  return temp;
}

static VOID ixheaace_measure_sum_square(WORD16 **input,
                                        ixheaace_loudness_struct *pstr_loudness_hdl) {
  FLOAT64 tot_one_channel = 0;
  FLOAT64 sum_square = 0;
  UWORD32 count = 0;
  FLOAT64 temp = 0;
  UWORD8 channel_no = 0;
  FLOAT64 input_sample;
  for (channel_no = 0; channel_no < pstr_loudness_hdl->n_channels; channel_no++) {
    tot_one_channel = 0;
    for (count = 0; count < pstr_loudness_hdl->num_samples_per_ch; count++) {
      input_sample = (FLOAT64)input[channel_no][count] / 32768.0;
      pstr_loudness_hdl->max_sample_val =
          MAX(fabs(input_sample), pstr_loudness_hdl->max_sample_val);
      temp = ixheaace_loudness_k_flt(input_sample, pstr_loudness_hdl, channel_no);
      tot_one_channel = tot_one_channel + (temp * temp);
    }
    sum_square += tot_one_channel;
  }
  pstr_loudness_hdl->sum_square = sum_square;
}

static FLOAT64 ixheaace_measure_momentary_loudness(ixheaace_loudness_struct *pstr_loudness_hdl) {
  FLOAT64 sum = 0;
  FLOAT64 momentary_loudness;
  UWORD32 count = 0;
  FLOAT64 old_ml_val, db_old_ml_val;
  {
    for (count = 0; count <= 2; count++) {
      pstr_loudness_hdl->prev_four_sum_square[count] =
          pstr_loudness_hdl->prev_four_sum_square[count + 1];
      sum += pstr_loudness_hdl->prev_four_sum_square[count];
    }

    pstr_loudness_hdl->prev_four_sum_square[3] = pstr_loudness_hdl->sum_square;
    sum += pstr_loudness_hdl->prev_four_sum_square[3];

    if ((pstr_loudness_hdl->mom_loudness_first_time_flag == 1) &&
        (pstr_loudness_hdl->count_fn_call_mmstl <= 2)) {
      momentary_loudness = IXHEAACE_LOUDNESS_DONT_PASS;
    } else {
      pstr_loudness_hdl->mom_loudness_first_time_flag = 0;
      momentary_loudness =
          -0.691 + 10 * log10(sum / ((FLOAT64)(4 * pstr_loudness_hdl->num_samples_per_ch)));

      if (pstr_loudness_hdl->get_intergrated_loudness == 1) {
        old_ml_val =
            pstr_loudness_hdl->mf_instances[pstr_loudness_hdl->loop_ml_count_fn_call].int_val;
        pstr_loudness_hdl->mf_instances[pstr_loudness_hdl->loop_ml_count_fn_call].int_val =
            sum / ((FLOAT64)(4 * pstr_loudness_hdl->num_samples_per_ch));
        db_old_ml_val = pstr_loudness_hdl->mf_instances[pstr_loudness_hdl->loop_ml_count_fn_call]
                            .momentary_loudness;
        pstr_loudness_hdl->mf_instances[pstr_loudness_hdl->loop_ml_count_fn_call]
            .momentary_loudness = momentary_loudness;
        if (pstr_loudness_hdl->mf_instances[pstr_loudness_hdl->loop_ml_count_fn_call]
                .momentary_loudness >= IXHEAACE_ABS_GATE) {
          if (db_old_ml_val < IXHEAACE_ABS_GATE) {
            pstr_loudness_hdl->no_of_mf_passing_abs_gate++;
            old_ml_val = 0;
          }

          pstr_loudness_hdl->tot_int_val_mf_passing_abs_gate =
              pstr_loudness_hdl->tot_int_val_mf_passing_abs_gate +
              (pstr_loudness_hdl->mf_instances[pstr_loudness_hdl->loop_ml_count_fn_call].int_val -
               old_ml_val);
        } else {
          if (db_old_ml_val >= IXHEAACE_ABS_GATE) {
            pstr_loudness_hdl->no_of_mf_passing_abs_gate--;
            pstr_loudness_hdl->tot_int_val_mf_passing_abs_gate =
                pstr_loudness_hdl->tot_int_val_mf_passing_abs_gate - old_ml_val;
          }
        }

        pstr_loudness_hdl->loop_ml_count_fn_call++;
        if (pstr_loudness_hdl->ml_count_fn_call < pstr_loudness_hdl->max_il_buf_size)
          pstr_loudness_hdl->ml_count_fn_call++;

        pstr_loudness_hdl->loop_ml_count_fn_call =
            pstr_loudness_hdl->loop_ml_count_fn_call % pstr_loudness_hdl->max_il_buf_size;
      }
    }
  }
  return (momentary_loudness);
}

FLOAT64 ixheaace_measure_integrated_loudness(pVOID loudness_handle) {
  UWORD32 count = 0;
  FLOAT64 avg = 0;
  FLOAT64 loudness = 0;
  ixheaace_loudness_struct *pstr_loudness_hdl = (ixheaace_loudness_struct *)loudness_handle;
  pstr_loudness_hdl->no_of_mf_passing_rel_gate = 0;
  pstr_loudness_hdl->tot_int_val_mf_passing_rel_gate = 0;

  if (pstr_loudness_hdl->no_of_mf_passing_abs_gate) {
    avg = (pstr_loudness_hdl->tot_int_val_mf_passing_abs_gate /
           pstr_loudness_hdl->no_of_mf_passing_abs_gate);
  } else {
    avg = IXHEAACE_SUM_SQUARE_EPS / pstr_loudness_hdl->num_samples_per_ch;
  }
  pstr_loudness_hdl->rel_gate = -0.691 + 10 * log10(avg) - 10;

  while (count < pstr_loudness_hdl->ml_count_fn_call) {
    if (pstr_loudness_hdl->mf_instances[count].momentary_loudness >=
        pstr_loudness_hdl->rel_gate) {
      pstr_loudness_hdl->no_of_mf_passing_rel_gate++;
      pstr_loudness_hdl->tot_int_val_mf_passing_rel_gate +=
          pstr_loudness_hdl->mf_instances[count].int_val;
    }
    count++;
  }

  if (pstr_loudness_hdl->no_of_mf_passing_rel_gate) {
    loudness = -0.691 + 10 * log10((pstr_loudness_hdl->tot_int_val_mf_passing_rel_gate /
                                    (FLOAT64)pstr_loudness_hdl->no_of_mf_passing_rel_gate));
  } else {
    loudness =
        -0.691 + 10 * log10(IXHEAACE_SUM_SQUARE_EPS / pstr_loudness_hdl->num_samples_per_ch);
  }

  return loudness;
}

FLOAT64 ixheaace_measure_loudness(pVOID loudness_handle, WORD16 **samples) {
  FLOAT64 loudness_value;
  ixheaace_loudness_struct *pstr_loudness_hdl = (ixheaace_loudness_struct *)loudness_handle;
  ixheaace_measure_sum_square(samples, pstr_loudness_hdl);
  loudness_value = ixheaace_measure_momentary_loudness(pstr_loudness_hdl);
  pstr_loudness_hdl->count_fn_call_mmstl++;
  return loudness_value;
}

FLOAT32 ixheaace_measure_sample_peak_value(pVOID loudness_handle) {
  FLOAT32 sample_peak_value;
  ixheaace_loudness_struct *pstr_loudness_hdl = (ixheaace_loudness_struct *)loudness_handle;
  sample_peak_value = (FLOAT32)(20 * log10(pstr_loudness_hdl->max_sample_val));
  return sample_peak_value;
}