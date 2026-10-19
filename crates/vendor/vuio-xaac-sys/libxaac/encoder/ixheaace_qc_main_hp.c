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

#include "ixheaac_type_def.h"
#include "ixheaac_constants.h"
#include "ixheaace_aac_constants.h"

#include "ixheaac_error_standards.h"
#include "ixheaace_error_codes.h"
#include "ixheaace_psy_const.h"
#include "ixheaace_tns.h"
#include "ixheaace_tns_params.h"
#include "ixheaace_rom.h"
#include "ixheaace_common_rom.h"
#include "ixheaace_quant.h"
#include "ixheaace_block_switch.h"
#include "ixheaace_bitbuffer.h"

#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops16.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaac_basic_ops.h"

#include "ixheaace_tns.h"
#include "ixheaace_psy_data.h"
#include "ixheaace_interface.h"
#include "ixheaace_adjust_threshold_data.h"

#include "ixheaace_dynamic_bits.h"
#include "ixheaace_qc_data.h"
#include "ixheaace_adjust_threshold.h"

#include "ixheaace_sf_estimation.h"

#include "ixheaace_static_bits.h"

#include "ixheaace_bits_count.h"

#include "ixheaace_channel_map.h"
#include "ixheaace_write_bitstream.h"
#include "ixheaace_psy_configuration.h"
#include "ixheaace_psy_mod.h"
#include "ixheaace_tns_params.h"
#include "ixheaace_stereo_preproc.h"
#include "ixheaace_enc_main.h"
#include "ixheaace_qc_util.h"
#include "ixheaace_common_utils.h"

IA_ERRORCODE ia_enhaacplus_enc_qc_main(
    ixheaace_qc_state *pstr_qc_state, WORD32 num_channels, ixheaace_element_bits *pstr_el_bits,
    ixheaace_psy_out_channel **psy_out_ch,
    ixheaace_psy_out_element *pstr_psy_out_element,
    ixheaace_qc_out_channel **pstr_qc_out_ch,
    ixheaace_qc_out_element *pstr_qc_out_element, WORD32 ancillary_data_bytes,
    ixheaace_aac_tables *pstr_aac_tables, WORD32 adts_flag, WORD32 aot, WORD32 stat_bits_flag,
    WORD32 flag_last_element, WORD32 frame_len_long, WORD8 *ptr_scratch,
    WORD32 *is_quant_spec_zero, WORD32 *is_gain_limited) {
  IA_ERRORCODE err_code;
  WORD32 ch;
  WORD32 i = 0;
  WORD32 k = 0;
  WORD32 j = 0;
  WORD32 iterations = 0;
  WORD32 constraints_fulfilled;
  WORD32 ch_dyn_bits;
  WORD32 max_ch_dyn_bits[IXHEAACE_MAX_CH_IN_BS_ELE];
  FLOAT32 ch_bit_dist[IXHEAACE_MAX_CH_IN_BS_ELE];
  ixheaace_qc_stack *ptr_stack = (ixheaace_qc_stack *)ptr_scratch;
  ptr_scratch += sizeof(ixheaace_qc_stack);

  ia_adj_thr_elem_struct *pstr_adj_thr_elem = &pstr_qc_state->str_adj_thr.str_adj_thr_ele;
  WORD32 gain;

  if (pstr_el_bits->bit_res_level < 0) {
    return IA_EXHEAACE_EXE_FATAL_INVALID_BIT_RES_LEVEL;
  }

  if (pstr_el_bits->bit_res_level > pstr_el_bits->max_bit_res_bits) {
    return IA_EXHEAACE_EXE_FATAL_INVALID_BIT_RES_LEVEL;
  }
  pstr_qc_out_element->static_bits_used =
      ia_enhaacplus_enc_count_static_bitdemand(psy_out_ch, pstr_psy_out_element, num_channels,
                                               aot, adts_flag, stat_bits_flag, flag_last_element);

  if (ancillary_data_bytes) {
    pstr_qc_out_element->anc_bits_used =
        7 + 8 * (ancillary_data_bytes + (ancillary_data_bytes >= 15));
  } else {
    pstr_qc_out_element->anc_bits_used = 0;
  }

  for (ch = 0; ch < num_channels; ch++) {
    iaace_calc_form_fac_per_chan(ptr_stack->sfb_form_fac[ch],
                                 ptr_stack->sfb_num_relevant_lines[ch], psy_out_ch[ch],
                                 ptr_stack->sfb_ld_energy[ch]);
  }

  iaace_adjust_threshold(
      &pstr_qc_state->str_adj_thr, pstr_adj_thr_elem, psy_out_ch, ch_bit_dist,
      pstr_qc_out_element,
      pstr_el_bits->average_bits - pstr_qc_out_element->static_bits_used -
          pstr_qc_out_element->anc_bits_used,
      pstr_el_bits->bit_res_level, pstr_el_bits->max_bit_res_bits,
      pstr_qc_out_element->static_bits_used + pstr_qc_out_element->anc_bits_used,
      &pstr_qc_state->max_bit_fac, ptr_stack->sfb_num_relevant_lines[0],
      ptr_stack->sfb_ld_energy[0], num_channels, 0, aot, ptr_scratch);

  iaace_estimate_scfs_chan(psy_out_ch, pstr_qc_out_ch, ptr_stack->sfb_form_fac,
                           ptr_stack->sfb_num_relevant_lines, num_channels, 0, frame_len_long);

  for (ch = 0; ch < num_channels; ch++) {
    max_ch_dyn_bits[ch] =
        (pstr_el_bits->average_bits + pstr_el_bits->bit_res_level - 7 -
         pstr_qc_out_element->static_bits_used - pstr_qc_out_element->anc_bits_used);

    max_ch_dyn_bits[ch] = (WORD32)floor(ch_bit_dist[ch] * (FLOAT32)(max_ch_dyn_bits[ch]));
  }

  pstr_qc_out_element->dyn_bits_used = 0;

  for (ch = 0; ch < num_channels; ch++) {
    /* now loop until bitstream constraints (ch_dyn_bits < maxChDynBits)
       are fulfilled */
    WORD32 spec_idx, sfb_offs, sfb;
    iterations = 0;
    gain = 0;
    for (spec_idx = 0; spec_idx < frame_len_long; spec_idx++) {
      ptr_stack->exp_spec[spec_idx] = (FLOAT32)psy_out_ch[ch]->ptr_spec_coeffs[spec_idx];
      ptr_stack->mdct_spec_float[spec_idx] = (FLOAT32)psy_out_ch[ch]->ptr_spec_coeffs[spec_idx];
    }
    do {
      WORD32 max_val;
      constraints_fulfilled = 1;
      WORD32 quant_spec_is_zero = 1;
      if (iterations > 0) {
        for (sfb_offs = 0; sfb_offs < psy_out_ch[ch]->sfb_count;
             sfb_offs += psy_out_ch[ch]->sfb_per_group) {
          for (sfb = 0; sfb < psy_out_ch[ch]->max_sfb_per_grp; sfb++) {
            WORD32 scalefactor = pstr_qc_out_ch[ch]->scalefactor[sfb + sfb_offs];
            gain = MAX(gain, pstr_qc_out_ch[ch]->global_gain - scalefactor);
            iaace_quantize_lines(
                pstr_qc_out_ch[ch]->global_gain - scalefactor,
                psy_out_ch[ch]->sfb_offsets[sfb_offs + sfb + 1] -
                    psy_out_ch[ch]->sfb_offsets[sfb_offs + sfb],
                ptr_stack->exp_spec + psy_out_ch[ch]->sfb_offsets[sfb_offs + sfb],
                pstr_qc_out_ch[ch]->quant_spec + psy_out_ch[ch]->sfb_offsets[sfb_offs + sfb],
                ptr_stack->mdct_spec_float + psy_out_ch[ch]->sfb_offsets[sfb_offs + sfb]);
          }
        }
      }

      max_val = iaace_calc_max_val_in_sfb(
          psy_out_ch[ch]->sfb_count, psy_out_ch[ch]->max_sfb_per_grp,
          psy_out_ch[ch]->sfb_per_group,
          psy_out_ch[ch]->sfb_offsets, pstr_qc_out_ch[ch]->quant_spec,
          pstr_qc_out_ch[ch]->max_val_in_sfb);

      if (max_val > MAXIMUM_QUANT) {
        constraints_fulfilled = 0;
      }

      for (k = 0; ((k < psy_out_ch[ch]->sfb_count) && (quant_spec_is_zero));
           k += psy_out_ch[ch]->sfb_per_group) {
        for (i = 0; ((i < psy_out_ch[ch]->max_sfb_per_grp) && (quant_spec_is_zero)); i++) {
          for (j = psy_out_ch[ch]->sfb_offsets[i+k]; j < psy_out_ch[ch]->sfb_offsets[i+k+1]; j++)
          {
            if (pstr_qc_out_ch[ch]->quant_spec[j] != 0) {
              quant_spec_is_zero = 0;
              break;
            }
          }
        }
      }
      err_code = ia_enhaacplus_enc_dyn_bitcount(
          pstr_qc_out_ch[ch]->quant_spec, pstr_qc_out_ch[ch]->max_val_in_sfb,
          pstr_qc_out_ch[ch]->scalefactor, psy_out_ch[ch]->window_sequence,
          psy_out_ch[ch]->sfb_count, psy_out_ch[ch]->max_sfb_per_grp,
          psy_out_ch[ch]->sfb_per_group,
          psy_out_ch[ch]->sfb_offsets, &pstr_qc_out_ch[ch]->section_data,
          pstr_qc_state->side_info_tab_long, pstr_qc_state->side_info_tab_short,
          pstr_aac_tables->pstr_huff_tab, pstr_qc_state->qc_scr.shared_buffer_2, aot,
          &ch_dyn_bits);

      if (err_code != IA_NO_ERROR) {
        return err_code;
      }

      if (ch_dyn_bits >= max_ch_dyn_bits[ch]) {
        constraints_fulfilled = 0;
      }

      if (quant_spec_is_zero == 1) {
        constraints_fulfilled = 1;
        /*Bit consuption is exceding bit reservoir, there is no scope left for bit consumption
          reduction, as spectrum is zero. Hence breaking the quantization loop. */
        if (iterations > 0) {
          *is_quant_spec_zero = 1;
          ch_dyn_bits = max_ch_dyn_bits[ch];
        }
      }
      if ((gain == MAX_GAIN_INDEX) && (constraints_fulfilled == 0)) {
        /* Bit consuption is exceding bit reservoir, there is no scope left for bit consumption
           reduction, as gain has reached the maximum value. Hence breaking the quantization
           loop. */
        constraints_fulfilled = 1;
        *is_gain_limited = 1;
        ch_dyn_bits = max_ch_dyn_bits[ch];
      }
      if (!constraints_fulfilled) {
        pstr_qc_out_ch[ch]->global_gain++;
      }
      iterations++;

    } while (!constraints_fulfilled);

    pstr_qc_out_element->dyn_bits_used += ch_dyn_bits;

    pstr_qc_out_ch[ch]->grouping_mask = psy_out_ch[ch]->grouping_mask;
    pstr_qc_out_ch[ch]->win_shape = psy_out_ch[ch]->window_shape;
  }

  pstr_adj_thr_elem->dyn_bits_last = pstr_qc_out_element->dyn_bits_used;
  {
    WORD32 bit_res_space = pstr_el_bits->max_bit_res_bits - pstr_el_bits->bit_res_level;
    WORD32 delta_bit_res = pstr_el_bits->average_bits - (pstr_qc_out_element->static_bits_used +
                                                         pstr_qc_out_element->dyn_bits_used +
                                                         pstr_qc_out_element->anc_bits_used);

    pstr_qc_out_element->fill_bits = MAX(0, (delta_bit_res - bit_res_space));
  }

  return IA_NO_ERROR;
}

WORD32 iaace_calc_max_val_in_sfb(WORD32 sfb_count, WORD32 max_sfb_per_grp, WORD32 ptr_sfb_per_grp,
                                 WORD32 *ptr_sfb_offset, WORD16 *ptr_quant_spec,
                                 UWORD16 *ptr_max_value) {
  WORD32 sfb;
  WORD32 max = 0;
  WORD32 sfb_offs;

  for (sfb_offs = 0; sfb_offs < sfb_count; sfb_offs += ptr_sfb_per_grp) {
    for (sfb = 0; sfb < max_sfb_per_grp; sfb++) {
      WORD32 line;
      WORD32 local_max = 0;
      for (line = ptr_sfb_offset[sfb + sfb_offs]; line < ptr_sfb_offset[sfb + sfb_offs + 1];
           line++) {
        if (abs(ptr_quant_spec[line]) > local_max) {
          local_max = abs(ptr_quant_spec[line]);
        }
      }
      ptr_max_value[sfb_offs + sfb] = (UWORD16)local_max;
      if (local_max > max) {
        max = local_max;
      }
    }
  }

  return max;
}
