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

#include <string.h>

#include "ixheaac_type_def.h"
#include "ixheaac_constants.h"
#include "impd_drc_common_enc.h"
#include "impd_drc_uni_drc.h"
#include "impd_drc_tables.h"
#include "impd_drc_api.h"
#include "ixheaace_api.h"
#include "ixheaace_aac_constants.h"
#include "ixheaace_error_codes.h"
#include "ixheaac_error_standards.h"
#include <stdlib.h>
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
#include "ixheaace_psy_const.h"
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

static WORD32 ia_enhaacplus_enc_calc_frame_len(WORD32 bit_rate, WORD32 sample_rate,
                                               FRAME_LEN_RESULT_MODE mode,
                                               WORD32 long_frame_len) {
  WORD32 result;

  result = ((long_frame_len) >> 3) * (bit_rate);

  switch (mode) {
    case FRAME_LEN_BYTES_MODULO:
      result %= sample_rate;
      break;
    case FRAME_LEN_BYTES_INT:
      result /= sample_rate;
      break;
    default:
      break;
  }

  return result;
}

static WORD32 ia_enhaacplus_enc_frame_padding(WORD32 bit_rate, WORD32 sample_rate,
                                              WORD32 *ptr_padding_rest, WORD32 frame_len_long) {
  WORD32 padding_on;
  WORD32 difference;

  padding_on = 0;

  difference = ia_enhaacplus_enc_calc_frame_len(bit_rate, sample_rate, FRAME_LEN_BYTES_MODULO,
                                                frame_len_long);

  *ptr_padding_rest -= difference;

  if (*ptr_padding_rest <= 0) {
    padding_on = 1;

    *ptr_padding_rest += sample_rate;
  }

  return padding_on;
}

IA_ERRORCODE ia_enhaacplus_enc_qc_out_new(ixheaace_qc_out *pstr_qc_out, WORD32 num_channels,
                                          WORD32 *ptr_shared_buffer1, WORD32 *ptr_shared_buffer3,
                                          WORD32 long_frame_len)

{
  WORD32 i;

  for (i = 0; i < num_channels; i++) {
    pstr_qc_out->qc_channel[i]->quant_spec = &((WORD16 *)ptr_shared_buffer1)[i * long_frame_len];

    memset(pstr_qc_out->qc_channel[i]->quant_spec, 0,
           sizeof(*pstr_qc_out->qc_channel[i]->quant_spec) * long_frame_len);

    pstr_qc_out->qc_channel[i]->max_val_in_sfb =
        &((UWORD16 *)&ptr_shared_buffer3[(long_frame_len + long_frame_len / 2) +
                                         IXHEAACE_MAX_CH_IN_BS_ELE *
                                             MAXIMUM_GROUPED_SCALE_FACTOR_BAND /
                                             2])[i * MAXIMUM_GROUPED_SCALE_FACTOR_BAND];
    memset(
        pstr_qc_out->qc_channel[i]->max_val_in_sfb, 0,
        sizeof(*pstr_qc_out->qc_channel[i]->max_val_in_sfb) * MAXIMUM_GROUPED_SCALE_FACTOR_BAND);

    pstr_qc_out->qc_channel[i]->scalefactor = &((WORD16 *)&ptr_shared_buffer3[(
        long_frame_len + long_frame_len / 2)])[i * MAXIMUM_GROUPED_SCALE_FACTOR_BAND];

    memset(pstr_qc_out->qc_channel[i]->scalefactor, 0,
           sizeof(*pstr_qc_out->qc_channel[i]->scalefactor) * MAXIMUM_GROUPED_SCALE_FACTOR_BAND);
  }

  if (pstr_qc_out == NULL) {
    return IA_EXHEAACE_INIT_FATAL_AAC_INIT_FAILED;
  }
  return IA_NO_ERROR;
}

IA_ERRORCODE ia_enhaacplus_enc_qc_new(ixheaace_qc_state *pstr_qc_state,
                                      WORD32 *ptr_shared_buffer_2, WORD32 long_frame_len
) {
  memset(pstr_qc_state, 0, sizeof(ixheaace_qc_state));
  pstr_qc_state->qc_scr.shared_buffer_2 =
      (ptr_shared_buffer_2 + long_frame_len * IXHEAACE_MAX_CH_IN_BS_ELE + 16);

  return IA_NO_ERROR;
}

IA_ERRORCODE ia_enhaacplus_enc_qc_init(ixheaace_qc_state *pstr_qc_state, WORD32 aot,
                                       ixheaace_qc_init *pstr_init, FLAG flag_framelength_small) {
  IA_ERRORCODE error = IA_NO_ERROR;
  pstr_qc_state->num_channels = pstr_init->pstr_element_info->n_channels_in_el;
  pstr_qc_state->max_bits_tot = pstr_init->max_bits;
  switch (aot) {
    case AOT_AAC_LC:
    case AOT_SBR:
    case AOT_PS:
      pstr_qc_state->bit_res_tot = pstr_init->bit_res - pstr_init->average_bits;
      break;

    case AOT_AAC_LD:
    case AOT_AAC_ELD:
      if (pstr_init->bit_res) {
        pstr_qc_state->bit_res_tot = pstr_init->bit_res - pstr_init->average_bits;
      } else {
        pstr_qc_state->bit_res_tot = 0;
      }
      break;
  }
  pstr_qc_state->average_bits_tot = pstr_init->average_bits;
  pstr_qc_state->max_bit_fac = pstr_init->max_bit_fac;
  pstr_qc_state->padding.padding_rest = pstr_init->padding.padding_rest;

  pstr_qc_state->quality_level = pstr_init->inv_quant;
  if (aot == AOT_AAC_LC || aot == AOT_SBR || aot == AOT_PS) {
    pstr_qc_state->glob_stat_bits = 3; /* for ID_END */
  }
  error = ia_enhaacplus_enc_init_element_bits(
      &pstr_qc_state->element_bits, *pstr_init->pstr_element_info, pstr_init->bitrate,
      pstr_init->average_bits, aot, pstr_qc_state->glob_stat_bits, pstr_init->bit_res,
      flag_framelength_small);

  if (error != IA_NO_ERROR) {
    return error;
  }
  iaace_adj_thr_init(&pstr_qc_state->str_adj_thr, pstr_init->mean_pe,
                     pstr_qc_state->element_bits.ch_bitrate, aot);

  ia_enhaacplus_enc_bitcount_init((WORD32 *)pstr_qc_state->side_info_tab_long,
                                  (WORD32 *)pstr_qc_state->side_info_tab_short);

  return IA_NO_ERROR;
}

VOID ia_enhaacplus_enc_update_bit_reservoir(ixheaace_qc_state *pstr_qc_kernel,
                                            ixheaace_qc_out *pstr_qc_out)

{
  ixheaace_element_bits *pstr_el_bits;

  pstr_qc_kernel->bit_res_tot = 0;

  pstr_el_bits = &pstr_qc_kernel->element_bits;

  if (pstr_el_bits->average_bits > 0) {
    /* constant bitrate */
    pstr_el_bits->bit_res_level +=
        pstr_el_bits->average_bits -
        (pstr_qc_out->qc_element.static_bits_used + pstr_qc_out->qc_element.dyn_bits_used +
         pstr_qc_out->qc_element.anc_bits_used + pstr_qc_out->qc_element.fill_bits);

    pstr_qc_kernel->bit_res_tot += pstr_el_bits->bit_res_level;
  } else {
    /* variable bitrate */
    pstr_el_bits->bit_res_level = pstr_el_bits->max_bits;
    pstr_qc_kernel->bit_res_tot = pstr_qc_kernel->max_bits_tot;
  }
}

IA_ERRORCODE ia_enhaacplus_enc_finalize_bit_consumption(ixheaace_qc_state *pstr_qc_kernel,
                                                        ixheaace_qc_out *pstr_qc_out,
                                                        WORD32 flag_last_element, WORD32 cnt_bits,
                                                        WORD32 *tot_fill_bits,
                                                        iexheaac_encoder_str **pstr_aac_enc,
                                                        WORD32 num_bs_elements, WORD32 aot) {
  WORD32 n_full_fill_elem, diff_bits;
  WORD32 total_fill_bits = 0;

  const WORD32 max_fill_elem_bits = 7 + 270 * 8;
  WORD32 tfb_flag = 0;
  WORD32 tfb_flag1 = 0;
  WORD32 tfb_flag2 = 0;

  pstr_qc_out->tot_static_bits_used = (flag_last_element ? pstr_qc_kernel->glob_stat_bits : 0);

  pstr_qc_out->tot_dyn_bits_used = 0;
  pstr_qc_out->tot_anc_bits_used = 0;
  pstr_qc_out->total_fill_bits = 0;
  pstr_qc_out->tot_static_bits_used += pstr_qc_out->qc_element.static_bits_used;
  pstr_qc_out->tot_dyn_bits_used += pstr_qc_out->qc_element.dyn_bits_used;
  pstr_qc_out->tot_anc_bits_used += pstr_qc_out->qc_element.anc_bits_used;
  pstr_qc_out->total_fill_bits += pstr_qc_out->qc_element.fill_bits;

  /* Accumulate total fill bits */
  *tot_fill_bits += pstr_qc_out->qc_element.fill_bits;
  if (flag_last_element) {
    WORD32 i, j, temp_resv;
    WORD32 bit_resv_spc[(MAXIMUM_BS_ELE << 1) + 1];
    WORD32 bit_resv_spc_sort[(MAXIMUM_BS_ELE << 1) + 1] = {0, 1,  2,  3,  4,  5,  6,  7, 8,
                                                           9, 10, 11, 12, 13, 14, 15, 16};

    total_fill_bits = *tot_fill_bits;

    /* Distribute fill bits among all channel elements for next frame */
    if (total_fill_bits > 0) {
      /* Generate array of vacancies in bit reservoirs */
      for (i = 0, temp_resv = 0; i < num_bs_elements; i++, temp_resv++) {
        bit_resv_spc[temp_resv] = (pstr_aac_enc[i]->qc_kernel.element_bits.max_bit_res_bits -
                                   pstr_aac_enc[i]->qc_kernel.element_bits.bit_res_level);

        /* CPE gets double the weight of SCE, so split CPE reservoir into two */
        if (pstr_aac_enc[i]->qc_kernel.num_channels == 2) {
          bit_resv_spc[temp_resv + 1] = bit_resv_spc[temp_resv] >> 1;
          bit_resv_spc[temp_resv] -= bit_resv_spc[temp_resv + 1];
          temp_resv++;
        }
      }

      /* Sort bit_resv_spc[] in descending order of levels and
      store the order in bit_resv_spc_sort[] */
      for (i = (temp_resv - 1); i > 0; i--) {
        for (j = 0; j < i; j++) {
          if (bit_resv_spc[bit_resv_spc_sort[j]] < bit_resv_spc[bit_resv_spc_sort[j + 1]]) {
            WORD32 tmp_var = bit_resv_spc_sort[j];
            bit_resv_spc_sort[j] = bit_resv_spc_sort[j + 1];
            bit_resv_spc_sort[j + 1] = tmp_var;
          }
        }
      }

      /* One dummy full reservoir at the end to help in bit distribution */
      bit_resv_spc[temp_resv] = 0;
      bit_resv_spc_sort[temp_resv] = temp_resv;

      /* Distribute fill bits among reservoirs in the order of bit_resv_spc_sort[]:
      - Bring up [0] to the level of [1]
      - Next bring up [0] and [1] to the level of [2]...and so on */
      for (i = 1; ((i < (temp_resv + 1)) && (total_fill_bits > 0)); i++) {
        if (((bit_resv_spc[bit_resv_spc_sort[0]] - bit_resv_spc[bit_resv_spc_sort[i]]) * i) <=
            total_fill_bits) {
          total_fill_bits -=
              ((bit_resv_spc[bit_resv_spc_sort[0]] - bit_resv_spc[bit_resv_spc_sort[i]]) * i);
          for (j = 0; j < i; j++) {
            bit_resv_spc[bit_resv_spc_sort[j]] = bit_resv_spc[bit_resv_spc_sort[i]];
          }
        } else {
          WORD32 div_bs_ele;

          div_bs_ele = (WORD32)(total_fill_bits / i);
          total_fill_bits -= (div_bs_ele * i);

          for (j = 0; j < i; j++) {
            bit_resv_spc[bit_resv_spc_sort[j]] -= div_bs_ele;
          }

          for (j = 0; ((j < i) && (total_fill_bits > 0)); j++) {
            bit_resv_spc[bit_resv_spc_sort[j]]--;
            total_fill_bits--;
          }
        }
      }

      /* Supply additional bits added for coding next frame */
      for (i = 0, temp_resv = 0; i < num_bs_elements; i++, temp_resv++) {
        WORD32 add_bits;

        add_bits = (pstr_aac_enc[i]->qc_kernel.element_bits.max_bit_res_bits -
                    pstr_aac_enc[i]->qc_kernel.element_bits.bit_res_level) -
                   bit_resv_spc[temp_resv];

        /* Because CPE reservoir has been split into two */
        if (pstr_aac_enc[i]->qc_kernel.num_channels == 2) {
          temp_resv++;
          add_bits -= bit_resv_spc[temp_resv];
        }

        /* These will be in addition to the avg. bitrate for the next frame */
        pstr_aac_enc[i]->qc_kernel.element_bits.carry_bits = add_bits;
      }

      /* Update remaining fill bits */
      *tot_fill_bits = total_fill_bits;
    }

    n_full_fill_elem = (total_fill_bits - 1) / max_fill_elem_bits;

    if (n_full_fill_elem) {
      total_fill_bits -= n_full_fill_elem * max_fill_elem_bits;
    }

    if (total_fill_bits > 0) {
      /* minimum Fillelement contains 7 (TAG + byte cnt) bits */
      total_fill_bits = MAX(7, total_fill_bits);

      /* fill element size equals n*8 + 7 */
      total_fill_bits += ((8 - (total_fill_bits - 7) % 8) % 8);

      switch (total_fill_bits) {
        case 7:
          tfb_flag2 = 1;
          break;

        case 15:
          tfb_flag1 = 1;
          break;

        default:
          tfb_flag = 1;
          break;
      }
    }

    total_fill_bits += n_full_fill_elem * max_fill_elem_bits;

    pstr_qc_out->align_bits =
        7 - (cnt_bits + pstr_qc_out->tot_dyn_bits_used + pstr_qc_out->tot_static_bits_used +
             pstr_qc_out->tot_anc_bits_used + +total_fill_bits - 1) %
                8;
    if (((pstr_qc_out->align_bits + total_fill_bits - *tot_fill_bits) == 8) &&
        (total_fill_bits > 8)) {
      total_fill_bits -= 8;
    }

    diff_bits = (pstr_qc_out->align_bits + total_fill_bits) - *tot_fill_bits;

    if (diff_bits) {
      if (diff_bits < 0) {
        return IA_EXHEAACE_EXE_FATAL_INVALID_BIT_CONSUMPTION;
      } else {
        {
          if (cnt_bits + pstr_qc_out->tot_static_bits_used + pstr_qc_out->tot_dyn_bits_used +
                  pstr_qc_out->tot_anc_bits_used + total_fill_bits >
              12288) {
            if ((diff_bits > 8) && (total_fill_bits > 8)) {
              if (tfb_flag || tfb_flag1) {
                total_fill_bits -= 8;
              }
              if (tfb_flag2) {
                total_fill_bits -= 7;
              }
            }
          } else {
            if (pstr_qc_kernel->element_bits.bit_res_level - diff_bits > 0) {
              pstr_qc_kernel->element_bits.bit_res_level -= diff_bits;
              pstr_qc_kernel->bit_res_tot = pstr_qc_kernel->element_bits.bit_res_level;
            } else {
              if ((diff_bits > 8) && (total_fill_bits > 8) && (tfb_flag)) {
                total_fill_bits -= 8;
              } else if ((diff_bits > 8) && (total_fill_bits > 8) && (tfb_flag1)) {
                total_fill_bits -= 8;
              } else if ((diff_bits > 8) && (total_fill_bits > 8) && (tfb_flag2)) {
                total_fill_bits -= 7;
              }
            }
          }
        }
      }
    }
    switch (aot) {
      case AOT_AAC_LC:
      case AOT_SBR:
      case AOT_PS:
        *tot_fill_bits = total_fill_bits;
        break;

      case AOT_AAC_LD:
      case AOT_AAC_ELD:
        pstr_qc_out->total_fill_bits = total_fill_bits;
        *tot_fill_bits = 0;
        break;
    }
  }  // if flag_last_element
  else {
    pstr_qc_out->align_bits = 0;
  }

  if ((pstr_qc_out->tot_dyn_bits_used + pstr_qc_out->tot_static_bits_used +
       pstr_qc_out->tot_anc_bits_used + pstr_qc_out->total_fill_bits + pstr_qc_out->align_bits) >
      pstr_qc_kernel->max_bits_tot) {
  }

  return IA_NO_ERROR;
}

VOID ia_enhaacplus_enc_adjust_bitrate(ixheaace_qc_state *pstr_qc_state, WORD32 bit_rate,
                                      WORD32 sample_rate, WORD32 flag_last_element,
                                      WORD32 frame_len_long)

{
  WORD32 padding_on;
  WORD32 frame_len;
  WORD32 code_bits;
  WORD32 code_bits_last;

  padding_on = ia_enhaacplus_enc_frame_padding(
      bit_rate, sample_rate, &pstr_qc_state->padding.padding_rest, frame_len_long);

  frame_len = padding_on + ia_enhaacplus_enc_calc_frame_len(bit_rate, sample_rate,
                                                            FRAME_LEN_BYTES_INT, frame_len_long);

  frame_len <<= 3;

  if (flag_last_element) {
    code_bits_last = pstr_qc_state->average_bits_tot - pstr_qc_state->glob_stat_bits;

    code_bits = frame_len - pstr_qc_state->glob_stat_bits;
  } else {
    code_bits_last = pstr_qc_state->average_bits_tot;

    code_bits = frame_len;
  }

  /* calculate bits for every channel element */
  if (code_bits != code_bits_last) {
    WORD32 total_bits = 0;

    pstr_qc_state->element_bits.average_bits =
        (WORD32)(pstr_qc_state->element_bits.relative_bits * code_bits);

    total_bits += pstr_qc_state->element_bits.average_bits;

    pstr_qc_state->element_bits.average_bits += code_bits - total_bits;
  }

  pstr_qc_state->average_bits_tot = frame_len;

  /* Bits carried over from previous frame due to distribution of fill bits */
  pstr_qc_state->element_bits.average_bits += pstr_qc_state->element_bits.carry_bits;
  pstr_qc_state->average_bits_tot += pstr_qc_state->element_bits.carry_bits;

  /* Flush for current frame */
  pstr_qc_state->element_bits.carry_bits = 0;
}

WORD32 ia_enhaacplus_aac_limitbitrate(WORD32 core_sampling_rate, WORD32 frame_length,
                                      WORD32 num_channels, WORD32 bit_rate) {
  WORD32 prev_bit_rate, shift = 0, iter = 0;
  WORD32 max_ch_bits = MAXIMUM_CHANNEL_BITS_1024;

  while ((frame_length & ~((1 << (shift + 1)) - 1)) == frame_length &&
         (core_sampling_rate & ~((1 << (shift + 1)) - 1)) == core_sampling_rate) {
    shift++;
  }

  max_ch_bits = MAXIMUM_CHANNEL_BITS_1024 * frame_length / MAX_FRAME_LEN;

  do {
    prev_bit_rate = bit_rate;

    bit_rate = MAX(bit_rate, ((((40 * num_channels) + TRANSPORT_BITS) * (core_sampling_rate)) /
                              frame_length));
    bit_rate = MIN(bit_rate, ((num_channels * max_ch_bits) * (core_sampling_rate >> shift)) /
                                 (frame_length >> shift));

  } while (prev_bit_rate != bit_rate && iter++ < 3);

  return bit_rate;
}
