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

#include <math.h>
#include <stdlib.h>
#include <string.h>

#include "ixheaac_type_def.h"
#include "ixheaac_constants.h"
#include "ixheaace_aac_constants.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops16.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaac_basic_ops.h"

#include "ixheaace_psy_const.h"
#include "ixheaace_tns.h"
#include "ixheaace_tns_params.h"
#include "ixheaace_rom.h"
#include "ixheaace_common_rom.h"
#include "ixheaace_bitbuffer.h"

#include "ixheaace_block_switch.h"
#include "ixheaace_psy_data.h"
#include "ixheaace_interface.h"
#include "ixheaace_adjust_threshold_data.h"
#include "ixheaace_dynamic_bits.h"
#include "ixheaace_qc_data.h"
#include "ixheaac_error_standards.h"
#include "ixheaace_stereo_preproc.h"
#include "ixheaace_common_utils.h"

IA_ERRORCODE iaace_init_stereo_pre_processing(ixheaace_stereo_pre_pro_pstr pstr_stereo_pre_pro,
                                        WORD32 no_channels, WORD32 bit_rate, WORD32 sample_rate,
                                        FLOAT32 used_scf_ratio) {
  FLOAT32 bpf = bit_rate * 1024.0f / sample_rate;
  FLOAT32 tmp;

  memset(pstr_stereo_pre_pro, 0, sizeof(ixheaace_stereo_pre_pro_struct));

  if (no_channels == 2) {
    (pstr_stereo_pre_pro)->stereo_attenuation_flag = 1;

    (pstr_stereo_pre_pro)->norm_pe_fac = 230.0f * used_scf_ratio / bpf;
    (pstr_stereo_pre_pro)->impact_factor =
        MIN(1, 400000.0f / (FLOAT32)(bit_rate - sample_rate * sample_rate / 72000.0f));
    (pstr_stereo_pre_pro)->inc_stereo_attenuation = 22050.0f / sample_rate * 400.0f / bpf;
    (pstr_stereo_pre_pro)->dec_stereo_attenuation = 22050.0f / sample_rate * 200.0f / bpf;

    (pstr_stereo_pre_pro)->const_attenuation = 0.0f;
    (pstr_stereo_pre_pro)->stereo_attenuation_max = 12.0f;

    /* energy ratio thresholds (dB) */
    (pstr_stereo_pre_pro)->sm_min = 0.0f;
    (pstr_stereo_pre_pro)->sm_max = 15.0f;
    (pstr_stereo_pre_pro)->lr_min = 10.0f;
    (pstr_stereo_pre_pro)->lr_max = 30.0f;

    /* pe thresholds */
    (pstr_stereo_pre_pro)->pe_crit = 1200.0f;
    (pstr_stereo_pre_pro)->pe_min = 700.0f;
    (pstr_stereo_pre_pro)->pe_impact_max = 100.0f;

    /* init start values */
    (pstr_stereo_pre_pro)->average_freq_energy_l = 0.0f;
    (pstr_stereo_pre_pro)->average_freq_energy_r = 0.0f;
    (pstr_stereo_pre_pro)->average_freq_energy_s = 0.0f;
    (pstr_stereo_pre_pro)->average_freq_energy_m = 0.0f;
    (pstr_stereo_pre_pro)->smoothed_pe_sum_sum = 7000.0f; /* typical start value */
    (pstr_stereo_pre_pro)->avg_s_to_m = -10.0f;           /* typical start value */
    (pstr_stereo_pre_pro)->last_l_to_r = 0.0f;
    (pstr_stereo_pre_pro)->last_nrg_lr = 0.0f;

    tmp = 1.0f - (bpf / 2600.0f);

    tmp = MAX(tmp, 0.0f);

    (pstr_stereo_pre_pro)->stereo_attenuation =
        tmp * (pstr_stereo_pre_pro)->stereo_attenuation_max;
  }

  return IA_NO_ERROR;
}

VOID iaace_apply_stereo_preproc(ixheaace_stereo_pre_pro_pstr pstr_stereo_pre_pro,
                                WORD32 num_channels, ixheaace_element_info *pstr_elem_info,
                                FLOAT32 *ptr_time_data, WORD32 granule_len) {
  FLOAT32 sm_ratio, s_to_m;
  FLOAT32 lr_ratio, l_to_r, delta_l_to_r, delta_nrg;
  FLOAT32 en_impact, pe_impact, pe_norm;
  FLOAT32 att, att_aimed;
  FLOAT32 max_inc, max_dec, swift_factor;
  FLOAT32 delta = 0.1f;
  FLOAT32 fac = pstr_stereo_pre_pro->stereo_attenuation_fac;
  FLOAT32 m_part, upper, div;
  FLOAT32 l_fac, r_fac;
  FLOAT32 L, R;
  WORD32 i;

  if (!pstr_stereo_pre_pro->stereo_attenuation_flag) {
    return;
  }

  m_part = 2.0f * pstr_stereo_pre_pro->average_freq_energy_m * (1.0f - fac * fac);

  upper = pstr_stereo_pre_pro->average_freq_energy_l * (1.0f + fac) +
          pstr_stereo_pre_pro->average_freq_energy_r * (1.0f - fac) - m_part;
  div = pstr_stereo_pre_pro->average_freq_energy_r * (1.0f + fac) +
        pstr_stereo_pre_pro->average_freq_energy_l * (1.0f - fac) - m_part;

  if ((div == 0.0f) || (upper == 0.0f)) {
    l_to_r = pstr_stereo_pre_pro->lr_max;
  } else {
    lr_ratio = (FLOAT32)fabs(upper / div);
    l_to_r = (FLOAT32)fabs(10.0f * log10(lr_ratio));
  }

  /* Calculate delta energy to previous frame */
  delta_nrg = (pstr_stereo_pre_pro->average_freq_energy_l +
               pstr_stereo_pre_pro->average_freq_energy_r + 1.0f) /
              (pstr_stereo_pre_pro->last_nrg_lr + 1.0f);

  delta_nrg = (FLOAT32)(fabs(10.0f * log10(delta_nrg)));

  /* Smooth S/M over time */
  sm_ratio = (pstr_stereo_pre_pro->average_freq_energy_s + 1.0f) /
             (pstr_stereo_pre_pro->average_freq_energy_m + 1.0f);

  s_to_m = (FLOAT32)(10.0f * log10(sm_ratio));

  pstr_stereo_pre_pro->avg_s_to_m =
      delta * s_to_m + (1 - delta) * pstr_stereo_pre_pro->avg_s_to_m;

  en_impact = 1.0f;

  if (pstr_stereo_pre_pro->avg_s_to_m > pstr_stereo_pre_pro->sm_min) {
    if (pstr_stereo_pre_pro->avg_s_to_m > pstr_stereo_pre_pro->sm_max) {
      en_impact = 0.0f;
    } else {
      en_impact = (pstr_stereo_pre_pro->sm_max - pstr_stereo_pre_pro->avg_s_to_m) /
                  (pstr_stereo_pre_pro->sm_max - pstr_stereo_pre_pro->sm_min);
    }
  }

  if (l_to_r > pstr_stereo_pre_pro->lr_min) {
    if (l_to_r > pstr_stereo_pre_pro->lr_max) {
      en_impact = 0.0f;
    } else {
      en_impact *= (pstr_stereo_pre_pro->lr_max - l_to_r) /
                   (pstr_stereo_pre_pro->lr_max - pstr_stereo_pre_pro->lr_min);
    }
  }

  pe_impact = 0.0f;

  pe_norm = pstr_stereo_pre_pro->smoothed_pe_sum_sum * pstr_stereo_pre_pro->norm_pe_fac;

  if (pe_norm > pstr_stereo_pre_pro->pe_min) {
    pe_impact = ((pe_norm - pstr_stereo_pre_pro->pe_min) /
                 (pstr_stereo_pre_pro->pe_crit - pstr_stereo_pre_pro->pe_min));
  }

  pe_impact = MIN(pe_impact, pstr_stereo_pre_pro->pe_impact_max);

  att_aimed = MIN((en_impact * pe_impact * pstr_stereo_pre_pro->impact_factor),
                  pstr_stereo_pre_pro->stereo_attenuation_max);

  /* Only accept changes if they are large enough */
  if ((fabs(att_aimed - pstr_stereo_pre_pro->stereo_attenuation) < 1.0f) && (att_aimed != 0.0f)) {
    att_aimed = pstr_stereo_pre_pro->stereo_attenuation;
  }

  att = att_aimed;

  swift_factor = (6.0f + pstr_stereo_pre_pro->stereo_attenuation) / (10.0f + l_to_r) *
                 MAX(1.0f, 0.2f * delta_nrg);

  delta_l_to_r = MAX(3.0f, l_to_r - pstr_stereo_pre_pro->last_l_to_r);

  max_dec = MIN((delta_l_to_r * delta_l_to_r / 9.0f * swift_factor), 5.0f);

  max_dec *= pstr_stereo_pre_pro->dec_stereo_attenuation;

  max_dec = MIN(max_dec, pstr_stereo_pre_pro->stereo_attenuation * 0.8f);

  delta_l_to_r = MAX(3.0f, pstr_stereo_pre_pro->last_l_to_r - l_to_r);

  max_inc = MIN((delta_l_to_r * delta_l_to_r / 9.0f * swift_factor), 5.0f);

  max_inc *= pstr_stereo_pre_pro->inc_stereo_attenuation;

  att = MIN(att, pstr_stereo_pre_pro->stereo_attenuation + max_inc);

  att = MAX(att, pstr_stereo_pre_pro->stereo_attenuation - max_dec);

  pstr_stereo_pre_pro->stereo_attenuation = (pstr_stereo_pre_pro->const_attenuation == 0)
                                                ? att
                                                : pstr_stereo_pre_pro->const_attenuation;

  /* Perform attenuation of side channel */
  pstr_stereo_pre_pro->stereo_attenuation_fac =
      (FLOAT32)pow(10.0f, 0.05f * (-pstr_stereo_pre_pro->stereo_attenuation));

  l_fac = 0.5f * (1.0f + pstr_stereo_pre_pro->stereo_attenuation_fac);
  r_fac = 0.5f * (1.0f - pstr_stereo_pre_pro->stereo_attenuation_fac);

  for (i = 0; i < granule_len; i++) {
    L = l_fac * ptr_time_data[num_channels * i + pstr_elem_info->channel_index[0]] +
        r_fac * ptr_time_data[num_channels * i + pstr_elem_info->channel_index[1]];
    R = r_fac * ptr_time_data[num_channels * i + pstr_elem_info->channel_index[0]] +
        l_fac * ptr_time_data[num_channels * i + pstr_elem_info->channel_index[1]];

    ptr_time_data[num_channels * i + pstr_elem_info->channel_index[0]] = L;
    ptr_time_data[num_channels * i + pstr_elem_info->channel_index[1]] = R;
  }

  pstr_stereo_pre_pro->last_l_to_r = l_to_r;

  pstr_stereo_pre_pro->last_nrg_lr =
      pstr_stereo_pre_pro->average_freq_energy_l + pstr_stereo_pre_pro->average_freq_energy_r;
}

VOID iaace_update_stereo_pre_process(ixheaace_psy_out_channel **pstr_psy_out,
                                     ixheaace_qc_out_element *pstr_qc_out,
                                     ixheaace_stereo_pre_pro_pstr pstr_stereo_pre_pro,
                                     FLOAT32 weight_pe_fac) {
  if (pstr_stereo_pre_pro->stereo_attenuation_flag) {
    FLOAT32 delta = 0.1f;

    pstr_stereo_pre_pro->average_freq_energy_l = pstr_psy_out[0]->sfb_sum_lr_energy;
    pstr_stereo_pre_pro->average_freq_energy_r = pstr_psy_out[1]->sfb_sum_lr_energy;
    pstr_stereo_pre_pro->average_freq_energy_m = pstr_psy_out[0]->sfb_sum_ms_energy;
    pstr_stereo_pre_pro->average_freq_energy_s = pstr_psy_out[1]->sfb_sum_ms_energy;

    pstr_stereo_pre_pro->smoothed_pe_sum_sum =
        delta * pstr_qc_out->pe * weight_pe_fac +
        (1 - delta) * pstr_stereo_pre_pro->smoothed_pe_sum_sum;
  }
}
