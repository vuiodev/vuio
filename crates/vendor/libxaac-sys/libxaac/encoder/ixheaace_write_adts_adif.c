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

#include "ixheaac_type_def.h"
#include "ixheaac_constants.h"
#include "ixheaace_psy_const.h"
#include "ixheaace_tns.h"
#include "ixheaace_tns_params.h"
#include "ixheaace_rom.h"
#include "ixheaace_common_rom.h"
#include "ixheaace_bitbuffer.h"
#include "ixheaace_aac_constants.h"
#include "ixheaace_write_adts_adif.h"

#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops16.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaac_basic_ops.h"
#include "ixheaace_common_utils.h"

static VOID ia_enhaacplus_enc_putbit(ixheaace_bitstream_params *pstr_bitstream, UWORD32 data,
                                       WORD32 num_bit) {
  WORD32 num, max_num, curr_num;
  WORD32 num_used, idx;
  unsigned long bits;
  WORD32 current_bitstream_bit;
  UWORD8 *bitstream_data;

  if (num_bit == 0) return;

  current_bitstream_bit = pstr_bitstream->current_bit;
  bitstream_data = pstr_bitstream->data;

  /*
      Functionality of Writing bits to the bitstream is split into 3 stages.
  */

  /*
      Stage #1: Write remainder bits to the partially filled byte
  */

  num = 0;
  num_used = current_bitstream_bit & 7;
  max_num = BYTE_NUMBIT - num_used;
  curr_num = MIN(num_bit, max_num);
  bits = data >> (num_bit - curr_num);

  idx = (current_bitstream_bit >> 3);

  if (num_used == 0) bitstream_data[idx] = 0;

  bitstream_data[idx] |= (bits & ((1 << curr_num) - 1)) << (max_num - curr_num);
  current_bitstream_bit += curr_num;
  num += curr_num;

  /*
      Stage #2:
      At this point, (num + num_used), will be a multiple of 8
      Now the bytes can be written directly to bitstream_data[], as long
      as (num + 8) < num_bit
  */

  while ((num + 8) < num_bit) {
    num += 8;
    current_bitstream_bit += 8;
    bits = data >> (num_bit - num);
    bitstream_data[++idx] = (UWORD8)bits;
  }

  /*
      Stage #3: Write remainder bits from the data
  */
  if (num < num_bit) {
    curr_num = num_bit - num;
    num_used = current_bitstream_bit & 7;
    max_num = BYTE_NUMBIT - num_used;
    idx = current_bitstream_bit >> 3;
    if (num_used == 0) bitstream_data[idx] = 0;
    bitstream_data[idx] |= (data & ((1 << curr_num) - 1)) << (max_num - curr_num);
    current_bitstream_bit += curr_num;
  }

  pstr_bitstream->current_bit = current_bitstream_bit;
  pstr_bitstream->num_bit = current_bitstream_bit;
}

static WORD16 ia_enhaacplus_enc_get_sample_rate_index(WORD32 sample_rate) {
  if (92017 <= sample_rate) {
    return 0;
  }
  if (75132 <= sample_rate) {
    return 1;
  }
  if (55426 <= sample_rate) {
    return 2;
  }
  if (46009 <= sample_rate) {
    return 3;
  }
  if (37566 <= sample_rate) {
    return 4;
  }
  if (27713 <= sample_rate) {
    return 5;
  }
  if (23004 <= sample_rate) {
    return 6;
  }
  if (18783 <= sample_rate) {
    return 7;
  }
  if (13856 <= sample_rate) {
    return 8;
  }
  if (11502 <= sample_rate) {
    return 9;
  }
  if (9391 <= sample_rate) {
    return 10;
  }
  return 11;
}

WORD32 ia_enhaacplus_enc_write_pce(WORD32 samp_rate, WORD32 ch_mask, WORD32 num_core_coder_chans,
                                   ixheaace_bit_buf_handle pstr_bit_stream_handle) {
  UWORD32 object_type = 0;
  UWORD8 buffer[200];
  ixheaace_bitstream_params bitstream_temp;
  ixheaace_bitstream_params *pstr_bitstream = &bitstream_temp;
  WORD32 write_flag = 1;
  WORD32 start_bits = pstr_bit_stream_handle->cnt_bits, end_bits;
  pstr_bitstream->data = buffer;
  pstr_bitstream->num_bit = 8;
  pstr_bitstream->current_bit = 0;

  object_type = 01;

  if (write_flag) {
    WORD32 i;
    WORD32 num_front_chan_ele, num_side_chan_ele, num_back_chan_ele, num_lfe_chan_ele;

    /* element instance tag can be anything... writing 0 */
    ixheaace_write_bits(pstr_bit_stream_handle, 0x00, 4);

    /* object type --> LC / LTP / any other */
    ixheaace_write_bits(pstr_bit_stream_handle, object_type, 2);

    /* sampling freq index */
    ixheaace_write_bits(pstr_bit_stream_handle,
                        ia_enhaacplus_enc_get_sample_rate_index(samp_rate), 4);

    /*  num_front_channel_elements --> only this present for mono / stereo */
    num_front_chan_ele = 0;

    if (ch_mask & 0x3) {
      num_front_chan_ele++; /*Front Left and Right Present*/
    }
    if (ch_mask & 0x4) {
      num_front_chan_ele++; /*Front Center Present*/
    }
    ixheaace_write_bits(pstr_bit_stream_handle, num_front_chan_ele, 4);

    /*	num_side_channel_elements 4 */
    num_side_chan_ele = 0;

    if (ch_mask & 0xC0) {
      num_side_chan_ele++; /*Back Left and Right Present*/
    }
    ixheaace_write_bits(pstr_bit_stream_handle, num_side_chan_ele, 4);

    /*	num_back_channel_elements 4  */
    num_back_chan_ele = 0;

    if (ch_mask & 0x30) {
      num_back_chan_ele++; /*Back Left and Right of center Present*/
    }
    ixheaace_write_bits(pstr_bit_stream_handle, num_back_chan_ele, 4);

    /*	num_lfe_channel_elements 2  */
    num_lfe_chan_ele = 0;

    if (ch_mask & 0x8) {
      num_lfe_chan_ele++; /*LFE channel Present*/
    }
    ixheaace_write_bits(pstr_bit_stream_handle, num_lfe_chan_ele, 2);

    /*	num_assoc_data_elements 3  */
    ixheaace_write_bits(pstr_bit_stream_handle, 0x00, 3);

    /*	num_valid_cc_elements 4 */
    ixheaace_write_bits(pstr_bit_stream_handle, num_core_coder_chans, 4);

    /* mono mix down is zero */
    ixheaace_write_bits(pstr_bit_stream_handle, 0x00, 1);

    /* mono_mixdown_element_number 4 if mono_mixdown_present == 1 */

    /* stereo_mixdown_present is zero */
    ixheaace_write_bits(pstr_bit_stream_handle, 0x00, 1);

    /* stereo_mixdown_element_number 4 if stereo_mixdown_present == 1 */

    /*	matrix_mixdown_idx_present is zero */
    ixheaace_write_bits(pstr_bit_stream_handle, 0x00, 1);

    {
      /* stereo --> 1 mono --> 0 */
      if (ch_mask & 0x4) {
        ixheaace_write_bits(pstr_bit_stream_handle, 0x00, 1);
        /* element tag select */
        ixheaace_write_bits(pstr_bit_stream_handle, 0x00, 4);
      }
      if (ch_mask & 0x3) {
        if ((ch_mask & 0x3) == 0x3) {
          /* stereo channel */
          ixheaace_write_bits(pstr_bit_stream_handle, 0x01, 1);
        } else {
          /* mono channel */
          ixheaace_write_bits(pstr_bit_stream_handle, 0x00, 1);
        }

        /* element tag select */
        ixheaace_write_bits(pstr_bit_stream_handle, 0x00, 4);
      }
    }

    for (i = 0; i < num_side_chan_ele; i++) {
      if ((ch_mask & 0xC0) == 0xC0) {
        /* stereo channel */
        ixheaace_write_bits(pstr_bit_stream_handle, 0x01, 1);
      } else {
        /* mono channel */
        ixheaace_write_bits(pstr_bit_stream_handle, 0x00, 1);
      }

      /* element tag select */
      ixheaace_write_bits(pstr_bit_stream_handle, 0x02, 4);
    }

    for (i = 0; i < num_back_chan_ele; i++) {
      if ((ch_mask & 0x30) == 0x30) {
        /* stereo channel */
        ixheaace_write_bits(pstr_bit_stream_handle, 0x01, 1);
      } else {
        /* mono channel */
        ixheaace_write_bits(pstr_bit_stream_handle, 0x00, 1);
      }

      /* element tag select */
      ixheaace_write_bits(pstr_bit_stream_handle, 0x01, 4);
    }

    for (i = 0; i < num_lfe_chan_ele; i++) {
      /* element tag select */
      ixheaace_write_bits(pstr_bit_stream_handle, 0x00, 4);
    }

    /* loop for independent coupling elements */
    for (i = 0; i < num_core_coder_chans; i++) {
      /* It is independent coupling channel */
      ixheaace_write_bits(pstr_bit_stream_handle, 0x01, 1);

      /* element tag select */
      ixheaace_write_bits(pstr_bit_stream_handle, num_core_coder_chans - i - 1, 4);
    }

    /* byte align the stream */
    ixheaace_write_bits(pstr_bit_stream_handle, 0,
                        (UWORD8)((8 - (pstr_bit_stream_handle->cnt_bits % 8)) % 8));
    /* comment field types --> do not quite know what this is */
    ixheaace_write_bits(pstr_bit_stream_handle, 0x00, 8);
  }

  end_bits = pstr_bit_stream_handle->cnt_bits;

  return (end_bits - start_bits);
}

WORD32 ia_enhaacplus_enc_write_ADTS_header(pUWORD8 buffer, WORD32 bytes_used, WORD32 samp_rate,
                                           WORD32 num_ch) {
  WORD32 bits = 56;
  UWORD32 object_type = 0;
  ixheaace_bitstream_params bitstream_temp;
  ixheaace_bitstream_params *pstr_bitstream = &bitstream_temp;
  WORD32 write_flag = 1;
  pstr_bitstream->data = buffer;
  pstr_bitstream->num_bit = 8;

  pstr_bitstream->current_bit = 0;
  ;
  object_type = 01;

  if (write_flag) {
    /* Fixed ADTS header */
    ia_enhaacplus_enc_putbit(pstr_bitstream, 0xFFFF, 12); /* 12 bit Syncword */
    ia_enhaacplus_enc_putbit(pstr_bitstream, 1 /*aacStateStruct->aacConfigSturct.mpegVersion*/,
                             1);                    /* ID == 0 for MPEG4 AAC, 1 for MPEG2 AAC */
    ia_enhaacplus_enc_putbit(pstr_bitstream, 0, 2); /* layer == 0 */
    ia_enhaacplus_enc_putbit(pstr_bitstream, 1, 1); /* protection absent */
    ia_enhaacplus_enc_putbit(pstr_bitstream, object_type, 2); /* profile */
    ia_enhaacplus_enc_putbit(
        pstr_bitstream,
        ia_enhaacplus_enc_get_sample_rate_index(samp_rate) /*aacStateStruct->sampleRateIdx*/,
        4);                                         /* sampling rate */
    ia_enhaacplus_enc_putbit(pstr_bitstream, 0, 1); /* private bit */
    ia_enhaacplus_enc_putbit(pstr_bitstream, num_ch /*aacStateStruct->aacConfigSturct.ch*/,
                             3);                    /* ch. aacConfigSturct (must be > 0) */
                                                    /* simply using num_channels only works for
                                                    6 channels or less, else a channel
     configuration should be written */
    ia_enhaacplus_enc_putbit(pstr_bitstream, 0, 1); /* original/copy */
    ia_enhaacplus_enc_putbit(pstr_bitstream, 0, 1); /* home */

    /* Variable ADTS header */
    ia_enhaacplus_enc_putbit(pstr_bitstream, 0, 1); /* copyr. id. bit */
    ia_enhaacplus_enc_putbit(pstr_bitstream, 0, 1); /* copyr. id. start */
    ia_enhaacplus_enc_putbit(pstr_bitstream, /*aacStateStruct->*/ bytes_used + 7, 13);
    ia_enhaacplus_enc_putbit(pstr_bitstream, 0x7FF, 11); /* buffer fullness (0x7FF for VBR) */

    ia_enhaacplus_enc_putbit(pstr_bitstream, 0, 2); /* raw data blocks (0+1=1) */
  }

  /*
   * MPEG2 says byte_aligment() here, but ADTS always is multiple of 8 bits
   * MPEG4 has no byte_alignment() here
   */
  return bits / 8;
}
