/******************************************************************************
 *
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
#include <stdio.h>
#include <stdlib.h>
#include <math.h>
#include <string.h>

#include "impd_type_def.h"
#include "impd_drc_extr_delta_coded_info.h"
#include "impd_drc_common.h"
#include "impd_drc_struct.h"
#include "impd_drc_filter_bank.h"
#include "impd_drc_multi_band.h"
#include "impd_drc_gain_dec.h"
#include "impd_drc_process_audio.h"

VOID impd_apply_gains_and_add(
    ia_drc_instructions_struct* pstr_drc_instruction_arr,
    const WORD32 drc_instructions_index,
    ia_drc_params_struct* ia_drc_params_struct,
    ia_gain_buffer_struct* pstr_gain_buf,
    shape_filter_block shape_filter_block[], FLOAT32* deinterleaved_audio[],

    FLOAT32* channel_audio[], WORD32 impd_apply_gains) {
  WORD32 c, b, g, i;
  WORD32 offset = 0, signal_index = 0;
  WORD32 gain_index_for_group[CHANNEL_GROUP_COUNT_MAX];
  WORD32 signal_index_for_channel[MAX_CHANNEL_COUNT];
  FLOAT32* lpcm_gains;
  FLOAT32 sum;
  FLOAT32 drc_gain_last = 0, gain_thr;
  WORD32 i_end, i_start;
  ia_drc_instructions_struct* str_drc_instruction_str =
      &(pstr_drc_instruction_arr[drc_instructions_index]);

  if (drc_instructions_index >= 0) {
    str_drc_instruction_str =
        &(pstr_drc_instruction_arr[drc_instructions_index]);
    {
      if (str_drc_instruction_str->drc_set_id > 0) {
        if (ia_drc_params_struct->delay_mode == DELAY_MODE_LOW_DELAY) {
          offset = ia_drc_params_struct->drc_frame_size;
        }
        gain_index_for_group[0] = 0;
        for (g = 0; g < str_drc_instruction_str->num_drc_ch_groups - 1; g++) {
          gain_index_for_group[g + 1] =
              gain_index_for_group[g] +
              str_drc_instruction_str->band_count_of_ch_group[g];
        }
        signal_index_for_channel[0] = 0;
        for (c = 0; c < str_drc_instruction_str->audio_num_chan - 1; c++) {
          if (str_drc_instruction_str->channel_group_of_ch[c] >= 0) {
            signal_index_for_channel[c + 1] =
                signal_index_for_channel[c] +
                str_drc_instruction_str->band_count_of_ch_group
                    [str_drc_instruction_str->channel_group_of_ch[c]];
          } else {
            signal_index_for_channel[c + 1] = signal_index_for_channel[c] + 1;
          }
        }

        for (g = 0; g < str_drc_instruction_str->num_drc_ch_groups; g++) {
          for (b = 0; b < str_drc_instruction_str->band_count_of_ch_group[g];
               b++) {
            if (str_drc_instruction_str->ch_group_parametric_drc_flag[g] == 0) {
              lpcm_gains =
                  pstr_gain_buf->buf_interpolation[gain_index_for_group[g] + b]
                      .lpcm_gains +
                  MAX_SIGNAL_DELAY - ia_drc_params_struct->gain_delay_samples -
                  ia_drc_params_struct->audio_delay_samples + offset;
            } else {
              lpcm_gains =
                  pstr_gain_buf->buf_interpolation[gain_index_for_group[g] + b]
                      .lpcm_gains +
                  MAX_SIGNAL_DELAY +
                  str_drc_instruction_str
                      ->parametric_drc_look_ahead_samples[g] -
                  ia_drc_params_struct->audio_delay_samples;
            }
            i_end = 0;
            i_start = 0;
            while (i_end < ia_drc_params_struct->drc_frame_size) {
              if (shape_filter_block[g].shape_flter_block_flag) {
                drc_gain_last = shape_filter_block[g].drc_gain_last;
                gain_thr = 0.0001f * drc_gain_last;
                while ((i_end < ia_drc_params_struct->drc_frame_size) &&
                       (fabs(lpcm_gains[i_end] - drc_gain_last) <= gain_thr))
                  i_end++;
              } else {
                i_end = ia_drc_params_struct->drc_frame_size;
              }

              for (c = 0; c < str_drc_instruction_str->audio_num_chan; c++)

              {
                if (g == str_drc_instruction_str->channel_group_of_ch[c]) {
                  signal_index = signal_index_for_channel[c] + b;

                  if (impd_apply_gains == 1) {
                    impd_shape_filt_block_time_process(
                        &shape_filter_block[g], &lpcm_gains[0], signal_index,
                        &deinterleaved_audio[signal_index][0], i_start, i_end);

                  } else {
                    for (i = i_start; i < i_end; i++) {
                      deinterleaved_audio[signal_index][i] = lpcm_gains[i];
                    }
                  }
                }
              }
              if ((i_end < ia_drc_params_struct->drc_frame_size) &&
                  (shape_filter_block[g].shape_flter_block_flag)) {
                impd_shape_filt_block_adapt(lpcm_gains[i_end],
                                            &shape_filter_block[g]);
              }
              if ((i_end == i_start) &&
                  (drc_gain_last == shape_filter_block[g].drc_gain_last))
                break;
              i_start = i_end;
            }
          }
        }
      }
    }
  }

  signal_index = 0;

  if (str_drc_instruction_str->drc_set_id > 0) {
    for (c = 0; c < str_drc_instruction_str->audio_num_chan; c++)

    {
      g = str_drc_instruction_str->channel_group_of_ch[c];
      if (g >= 0) {
        for (i = 0; i < ia_drc_params_struct->drc_frame_size; i++) {
          sum = 0.0f;
          for (b = 0; b < str_drc_instruction_str->band_count_of_ch_group[g];
               b++) {
            sum += deinterleaved_audio[signal_index + b][i];
          }

          channel_audio[c][i] = sum;
        }
        signal_index += str_drc_instruction_str->band_count_of_ch_group[g];
      } else {
        for (i = 0; i < ia_drc_params_struct->drc_frame_size; i++) {
          channel_audio[c][i] = deinterleaved_audio[signal_index][i];
        }
        signal_index++;
      }
    }
  } else {
    for (c = 0; c < str_drc_instruction_str->audio_num_chan; c++)

    {
      for (i = 0; i < ia_drc_params_struct->drc_frame_size; i++) {
        channel_audio[c][i] = deinterleaved_audio[c][i];
      }
    }
  }

  return;
}

WORD32
impd_filter_banks_process(ia_drc_instructions_struct* pstr_drc_instruction_arr,
                          const WORD32 drc_instructions_index,
                          ia_drc_params_struct* ia_drc_params_struct,
                          FLOAT32* audio_io_buf[],
                          ia_audio_band_buffer_struct* audio_band_buffer,
                          ia_filter_banks_struct* ia_filter_banks_struct,
                          const WORD32 passThru) {
  WORD32 c, g, e, i, num_bands;
  // WORD32 err = 0;
  FLOAT32* audio_in;
  FLOAT32** audio_out;
  ia_drc_filter_bank_struct* str_drc_filter_bank;
  ia_drc_instructions_struct* str_drc_instruction_str;
  WORD32 drc_frame_size = ia_drc_params_struct->drc_frame_size;

  if (drc_instructions_index >= 0) {
    str_drc_instruction_str =
        &(pstr_drc_instruction_arr[drc_instructions_index]);
  } else {
    return -1;
  }

  e = 0;

  for (c = 0; c < str_drc_instruction_str->audio_num_chan; c++)

  {
    str_drc_filter_bank = NULL;

    audio_in = audio_io_buf[c];

    audio_out = &(audio_band_buffer->non_interleaved_audio[e]);
    if ((passThru == 0) && (drc_instructions_index >= 0)) {
      if (str_drc_instruction_str->drc_set_id < 0) {
        num_bands = 1;
      } else {
        g = str_drc_instruction_str->channel_group_of_ch[c];
        if (g == -1) {
          num_bands = 1;
          // if (ia_filter_banks_struct->str_drc_filter_bank != NULL)
          //{
          str_drc_filter_bank =
              &(ia_filter_banks_struct->str_drc_filter_bank
                    [str_drc_instruction_str->num_drc_ch_groups]);
          //}
        } else {
          num_bands = str_drc_instruction_str->band_count_of_ch_group[g];
          // if (ia_filter_banks_struct->str_drc_filter_bank != NULL)
          //{
          str_drc_filter_bank =
              &(ia_filter_banks_struct->str_drc_filter_bank[g]);
          //}
        }
        // if (ia_filter_banks_struct->str_drc_filter_bank != NULL)
        //{
        // if (&str_drc_filter_bank->str_all_pass_cascade != NULL)
        //{
        impd_all_pass_cascade_process(
            &str_drc_filter_bank->str_all_pass_cascade, c, drc_frame_size,
            audio_in);
        //}
        //}
      }
    } else {
      num_bands = 1;
    }
    switch (num_bands) {
      case 1:
        for (i = 0; i < drc_frame_size; i++) {
          audio_out[0][i] = audio_in[i];
        }
        e++;
        break;
      case 2:
        impd_two_band_filter_process(&str_drc_filter_bank->str_two_band_bank, c,
                                     drc_frame_size, audio_in, audio_out);
        e += 2;
        break;
      case 3:
        impd_three_band_filter_process(
            &str_drc_filter_bank->str_three_band_bank, c, drc_frame_size,
            audio_in, audio_out);
        e += 3;
        break;
      case 4:
        impd_four_band_filter_process(&str_drc_filter_bank->str_four_band_bank,
                                      c, drc_frame_size, audio_in, audio_out);
        e += 4;
        break;
      default:
        return (PARAM_ERROR);
        break;
    }
  }

  return (0);
}

VOID impd_store_audio_io_buffer_time(
    FLOAT32* audio_in_out_buf[], ia_audio_in_out_buf* audio_io_buf_internal) {
  WORD32 i, j;

  if (audio_io_buf_internal->audio_delay_samples) {
    for (i = 0; i < audio_io_buf_internal->audio_num_chan; i++) {
      for (j = 0; j < audio_io_buf_internal->frame_size; j++) {
        audio_io_buf_internal->audio_io_buffer_delayed
            [i][audio_io_buf_internal->audio_delay_samples + j] =
            audio_in_out_buf[i][j];
      }
    }
  } else {
    audio_io_buf_internal->audio_io_buffer_delayed = audio_in_out_buf;
    audio_io_buf_internal->audio_in_out_buf = audio_in_out_buf;
  }

  return;
}

VOID impd_retrieve_audio_io_buffer_time(
    FLOAT32* audio_in_out_buf[], ia_audio_in_out_buf* audio_io_buf_internal) {
  WORD32 i, j;

  if (audio_io_buf_internal->audio_delay_samples) {
    for (i = 0; i < audio_io_buf_internal->audio_num_chan; i++) {
      for (j = 0; j < audio_io_buf_internal->frame_size; j++) {
        audio_in_out_buf[i][j] =
            audio_io_buf_internal->audio_io_buffer_delayed[i][j];
      }
    }
  }

  return;
}

VOID impd_advance_audio_io_buffer_time(
    ia_audio_in_out_buf* audio_io_buf_internal) {
  WORD32 i;
  if (audio_io_buf_internal->audio_delay_samples) {
    for (i = 0; i < audio_io_buf_internal->audio_num_chan; i++) {
      memmove(
          audio_io_buf_internal->audio_io_buffer_delayed[i],
          &audio_io_buf_internal
               ->audio_io_buffer_delayed[i][audio_io_buf_internal->frame_size],
          sizeof(FLOAT32) * audio_io_buf_internal->audio_delay_samples);
    }
  }

  return;
}
