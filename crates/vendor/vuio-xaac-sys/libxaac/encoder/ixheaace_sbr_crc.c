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
#include "ixheaace_aac_constants.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops16.h"
#include "ixheaac_basic_ops40.h"

#include "ixheaace_bitbuffer.h"
#include "ixheaace_sbr_def.h"
#include "ixheaace_sbr_cmondata.h"
#include "ixheaace_sbr_crc.h"
#include "ixheaace_common_utils.h"

static VOID ixheaace_crc_advance(UWORD16 crc_poly, UWORD16 crc_mask, UWORD16 *ptr_crc,
                                 UWORD32 b_value, WORD32 b_bits) {
  WORD32 i = b_bits - 1;

  while (i >= 0) {
    UWORD16 flag = (*ptr_crc) & crc_mask ? 1 : 0;

    flag ^= (b_value & (1 << i) ? 1 : 0);

    (*ptr_crc) <<= 1;

    if (flag) {
      (*ptr_crc) ^= crc_poly;
    }
    i--;
  }
}

VOID ixheaace_init_sbr_bitstream(ixheaace_pstr_common_data pstr_cmon_data,
                                 UWORD8 *ptr_memory_base, WORD32 memory_size, WORD32 crc_active,
                                 ixheaace_sbr_codec_type sbr_codec) {
  ixheaace_reset_bitbuf(&pstr_cmon_data->str_sbr_bit_buf, ptr_memory_base, memory_size);

  pstr_cmon_data->str_tmp_write_bit_buf = pstr_cmon_data->str_sbr_bit_buf;
  if (HEAAC_SBR == sbr_codec) {
    ixheaace_write_bits(&pstr_cmon_data->str_sbr_bit_buf, 0, SI_FILL_EXTENTION_BITS);

    if (crc_active) {
      ixheaace_write_bits(&pstr_cmon_data->str_sbr_bit_buf, 0, SI_CRC_BITS_SBR);
    }
  }
}

VOID ixheaace_assemble_sbr_bitstream(ixheaace_pstr_common_data pstr_cmon_data,
                                     ixheaace_sbr_codec_type sbr_codec) {
  UWORD16 crc_reg = CRCINIT_SBR;
  WORD32 num_crc_bits, i;
  WORD32 sbr_load = 0;

  sbr_load = pstr_cmon_data->sbr_hdr_bits + pstr_cmon_data->sbr_data_bits;

  if (HEAAC_SBR == sbr_codec) {
    sbr_load += SI_FILL_EXTENTION_BITS;
  }

  if (pstr_cmon_data->sbr_crc_len) {
    sbr_load += SI_CRC_BITS_SBR;
  }

  if (USAC_SBR != sbr_codec) {
    pstr_cmon_data->sbr_fill_bits = (8 - (sbr_load) % 8) % 8;

    ixheaace_write_bits(&pstr_cmon_data->str_sbr_bit_buf, 0,
                        (UWORD8)pstr_cmon_data->sbr_fill_bits);
  }
  if (pstr_cmon_data->sbr_crc_len) {
    ixheaace_bit_buf tmp_crc_buf = pstr_cmon_data->str_sbr_bit_buf;

    ixheaace_readbits(&tmp_crc_buf, SI_FILL_EXTENTION_BITS);

    ixheaace_readbits(&tmp_crc_buf, SI_CRC_BITS_SBR);

    num_crc_bits = pstr_cmon_data->sbr_hdr_bits + pstr_cmon_data->sbr_data_bits +
                   pstr_cmon_data->sbr_fill_bits;

    i = 0;
    while (i < num_crc_bits) {
      UWORD32 bit;

      bit = ixheaace_readbits(&tmp_crc_buf, 1);

      ixheaace_crc_advance(CRC_POLYNOMIAL_SBR, CRC_MASK_SBR, &crc_reg, bit, 1);

      i++;
    }

    crc_reg &= (CRC_RANGE_SBR);
  }

  if (pstr_cmon_data->sbr_crc_len) {
    ixheaace_write_bits(&pstr_cmon_data->str_tmp_write_bit_buf, SI_FIL_CRC_SBR,
                        SI_FILL_EXTENTION_BITS);

    ixheaace_write_bits(&pstr_cmon_data->str_tmp_write_bit_buf, crc_reg, SI_CRC_BITS_SBR);
  } else {
    if (HEAAC_SBR == sbr_codec) {
      ixheaace_write_bits(&pstr_cmon_data->str_tmp_write_bit_buf, SI_FIL_SBR,
                          SI_FILL_EXTENTION_BITS);
    }
  }
}
