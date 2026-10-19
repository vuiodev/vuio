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
#include <math.h>
#include "ixheaac_type_def.h"
#include "ixheaac_constants.h"
#include "ixheaacd_bitbuffer.h"
#include "ixheaacd_common_rom.h"
#include "ixheaacd_sbrdecsettings.h"
#include "ixheaacd_sbr_scale.h"
#include "ixheaacd_env_extr_part.h"
#include "ixheaacd_sbr_rom.h"
#include "ixheaacd_hybrid.h"
#include "ixheaacd_ps_dec.h"
#include "ixheaacd_config.h"
#include "ixheaacd_qmf_dec.h"
#include "ixheaacd_mps_polyphase.h"
#include "ixheaacd_mps_struct_def.h"
#include "ixheaacd_mps_res_rom.h"
#include "ixheaacd_mps_aac_struct.h"
#include "ixheaacd_mps_dec.h"

#define DIR_DIFF_IN 0
#define DOWNMIX_IN 1

#define LAMDA (4.0f)
#define GES_ALPHA (0.99637864f)
#define GES_BETA (0.9643691f)

extern const WORD32
    ixheaacd_hybrid_band_71_to_processing_band_20_map[MAX_HYBRID_BANDS_MPS];

VOID ixheaacd_mps_env_init(ia_mps_dec_state_struct *self) {
  WORD32 i;
  for (i = 0; i < 3; i++) {
    self->guided_env_shaping.avg_energy_prev[i] = 32768.f * 32768.f;
  }
}

static VOID ixheaacd_mps_est_normalized_envelope(ia_mps_dec_state_struct *self,
                                                 WORD32 inp, WORD32 ch,
                                                 FLOAT32 *env) {
  FLOAT32 slot_energy[MAX_TIME_SLOTS][MAX_PARAMETER_BANDS] = {{0}};
  FLOAT32 pb_energy[MAX_PARAMETER_BANDS] = {0};
  FLOAT32 whitening_weight[MAX_PARAMETER_BANDS];
  WORD32 ii, jj, param_band;

  WORD32 k_start = 10;
  WORD32 k_stop = 18;

  FLOAT32 total_energy = 0, avg_energy = 0;

  WORD32 ch_offset;

  switch (inp) {
    case DIR_DIFF_IN:
      ch_offset = 0;
      for (ii = 0; ii < self->time_slots; ii++) {
        for (jj = 0; jj < self->hyb_band_count_max; jj++) {
          slot_energy[ii]
                     [ixheaacd_hybrid_band_71_to_processing_band_20_map[jj]] +=
              ((self->hyb_dir_out[ch][ii][jj].re +
                self->hyb_diff_out[ch][ii][jj].re) *
               (self->hyb_dir_out[ch][ii][jj].re +
                self->hyb_diff_out[ch][ii][jj].re)) +
              ((self->hyb_dir_out[ch][ii][jj].im +
                self->hyb_diff_out[ch][ii][jj].im) *
               (self->hyb_dir_out[ch][ii][jj].im +
                self->hyb_diff_out[ch][ii][jj].im));
        }
      }
      break;
    case DOWNMIX_IN:
      ch_offset = self->out_ch_count;
      if ((self->pre_mix_req | self->bs_tsd_enable)) {
        for (ii = 0; ii < self->time_slots; ii++) {
          for (jj = 0; jj < self->hyb_band_count_max; jj++) {
            slot_energy
                [ii][ixheaacd_hybrid_band_71_to_processing_band_20_map[jj]] +=
                self->hyb_in[ch][jj][ii].re * self->hyb_in[ch][jj][ii].re +
                self->hyb_in[ch][jj][ii].im * self->hyb_in[ch][jj][ii].im;
          }
        }
      } else {
        for (ii = 0; ii < self->time_slots; ii++) {
          for (jj = 0; jj < self->hyb_band_count_max; jj++) {
            slot_energy
                [ii][ixheaacd_hybrid_band_71_to_processing_band_20_map[jj]] +=
                self->w_dir[ch][ii][jj].re * self->w_dir[ch][ii][jj].re +
                self->w_dir[ch][ii][jj].im * self->w_dir[ch][ii][jj].im;
          }
        }
      }

      break;
    default:
      ch_offset = 0;
      break;
  }

  for (param_band = k_start; param_band <= k_stop; param_band++)
    pb_energy[param_band] =
        self->guided_env_shaping.pb_energy_prev[ch + ch_offset][param_band];

  avg_energy = self->guided_env_shaping.avg_energy_prev[ch + ch_offset];

  for (ii = 0; ii < self->time_slots; ii++) {
    total_energy = 0;
    for (param_band = k_start; param_band <= k_stop; param_band++) {
      pb_energy[param_band] = (1 - GES_ALPHA) * slot_energy[ii][param_band] +
                              GES_ALPHA * pb_energy[param_band];

      total_energy += slot_energy[ii][param_band];
    }
    total_energy /= (k_stop - k_start + 1);

    total_energy =
        (1 - GES_ALPHA) * total_energy +
        GES_ALPHA * self->guided_env_shaping.frame_energy_prev[ch + ch_offset];

    self->guided_env_shaping.frame_energy_prev[ch + ch_offset] = total_energy;

    for (param_band = k_start; param_band <= k_stop; param_band++) {
      whitening_weight[param_band] =
          total_energy / (pb_energy[param_band] + ABS_THR);
    }

    env[ii] = 0;
    for (param_band = k_start; param_band <= k_stop; param_band++) {
      env[ii] += slot_energy[ii][param_band] * whitening_weight[param_band];
    }

    avg_energy = (1 - GES_BETA) * env[ii] + GES_BETA * avg_energy;

    env[ii] = (FLOAT32)sqrt(env[ii] / (avg_energy + ABS_THR));
  }

  for (param_band = k_start; param_band <= k_stop; param_band++)
    self->guided_env_shaping.pb_energy_prev[ch + ch_offset][param_band] =
        pb_energy[param_band];

  self->guided_env_shaping.avg_energy_prev[ch + ch_offset] = avg_energy;
}

VOID ixheaacd_mps_time_env_shaping(ia_mps_dec_state_struct *self) {
  FLOAT32 dir_energy[MAX_TIME_SLOTS];
  FLOAT32 dmx_energy[MAX_TIME_SLOTS];
  WORD32 ch, time_slot, jj;

  WORD32 band_start;
  FLOAT32 gain, ratio;

  FLOAT32 amp_direct = 0;
  FLOAT32 amp_diff = 0;
  FLOAT32 amp_ratio;

  band_start = 6;

  ixheaacd_mps_est_normalized_envelope(self, DOWNMIX_IN, 0, dmx_energy);

  for (ch = 0; ch < self->out_ch_count; ch++) {
    ixheaacd_mps_est_normalized_envelope(self, DIR_DIFF_IN, ch, dir_energy);

    if (self->temp_shape_enable_ch_ges[ch]) {
      for (time_slot = 0; time_slot < self->time_slots; time_slot++) {
        gain = self->env_shape_data[ch][time_slot] * dmx_energy[time_slot] /
               (dir_energy[time_slot] + 1e-9f);

        amp_direct = 0;
        amp_diff = 0;

        for (jj = band_start; jj < self->hyb_band_count_max; jj++) {
          amp_direct += self->hyb_dir_out[ch][time_slot][jj].re *
                            self->hyb_dir_out[ch][time_slot][jj].re +
                        self->hyb_dir_out[ch][time_slot][jj].im *
                            self->hyb_dir_out[ch][time_slot][jj].im;

          amp_diff += self->hyb_diff_out[ch][time_slot][jj].re *
                          self->hyb_diff_out[ch][time_slot][jj].re +
                      self->hyb_diff_out[ch][time_slot][jj].im *
                          self->hyb_diff_out[ch][time_slot][jj].im;
        }

        amp_ratio = (FLOAT32)sqrt(amp_diff / (amp_direct + ABS_THR));

        ratio = min(max((gain + amp_ratio * (gain - 1)), 1 / LAMDA), LAMDA);

        for (jj = band_start; jj < self->hyb_band_count_max; jj++) {
          self->hyb_dir_out[ch][time_slot][jj].re *= ratio;
          self->hyb_dir_out[ch][time_slot][jj].im *= ratio;
        }
      }
    }
  }
}
