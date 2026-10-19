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
#include <string.h>
#include "ixheaac_type_def.h"

#include "ixheaacd_sbr_common.h"

#include "ixheaac_constants.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops16.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaac_basic_ops.h"
#include "ixheaacd_bitbuffer.h"

#include "ixheaac_basic_op.h"
#include "ixheaacd_intrinsics.h"

#include "ixheaacd_defines.h"

#include "ixheaacd_aac_rom.h"

#include "ixheaacd_definitions.h"

#include "ixheaacd_error_codes.h"

#include "ixheaacd_pulsedata.h"

#include "ixheaacd_pns.h"
#include "ixheaacd_drc_data_struct.h"

#include "ixheaacd_lt_predict.h"
#include "ixheaacd_cnst.h"
#include "ixheaacd_ec_defines.h"
#include "ixheaacd_ec_struct_def.h"

#include "ixheaacd_channelinfo.h"
#include "ixheaacd_drc_dec.h"

#include "ixheaacd_sbrdecoder.h"
#include "ixheaacd_block.h"
#include "ixheaacd_channel.h"

#include "ixheaacd_sbr_payload.h"
#include "ixheaacd_common_rom.h"
#include "ixheaacd_sbrdecsettings.h"
#include "ixheaacd_sbr_scale.h"
#include "ixheaacd_env_extr_part.h"
#include "ixheaacd_sbr_rom.h"

#include "ixheaacd_lpp_tran.h"
#include "ixheaacd_hybrid.h"
#include "ixheaacd_ps_dec.h"

#include "ixheaacd_env_extr.h"
#include "ixheaacd_adts.h"
#include "ixheaacd_audioobjtypes.h"
#include "ixheaacd_memory_standards.h"

#include "ixheaacd_latmdemux.h"

#include "ixheaacd_aacdec.h"
#include "ixheaacd_mps_polyphase.h"
#include "ixheaacd_config.h"
#include "ixheaacd_qmf_dec.h"
#include "ixheaacd_mps_macro_def.h"
#include "ixheaacd_mps_struct_def.h"
#include "ixheaacd_mps_res_rom.h"
#include "ixheaacd_mps_aac_struct.h"
#include "ixheaacd_mps_dec.h"

#include "ixheaacd_struct_def.h"

#include "ixheaacd_multichannel.h"

#define ALIGN_SIZE64(x) ((((x) + 7) >> 3) << 3)

WORD32 ixheaacd_set_aac_persistent_buffers(VOID *aac_persistent_mem_v,
                                           WORD32 num_channel) {
  WORD32 persistent_used;

  struct ia_aac_persistent_struct *aac_persistent_mem =
      (struct ia_aac_persistent_struct *)aac_persistent_mem_v;

  persistent_used =
      IXHEAAC_GET_SIZE_ALIGNED(sizeof(struct ia_aac_persistent_struct), BYTE_ALIGN_8);

  memset(aac_persistent_mem, 0,
         IXHEAAC_GET_SIZE_ALIGNED(sizeof(struct ia_aac_persistent_struct), BYTE_ALIGN_8));
  aac_persistent_mem->overlap_buffer =
      (WORD32 *)((WORD8 *)aac_persistent_mem_v + persistent_used);

  memset(
      (WORD32 *)((WORD8 *)aac_persistent_mem_v + persistent_used), 0,
      IXHEAAC_GET_SIZE_ALIGNED(
          4 * 512 * num_channel * sizeof(aac_persistent_mem->overlap_buffer[0]), BYTE_ALIGN_8));

  persistent_used += IXHEAAC_GET_SIZE_ALIGNED(
      4 * 512 * num_channel * sizeof(aac_persistent_mem->overlap_buffer[0]), BYTE_ALIGN_8);

  aac_persistent_mem->sbr_payload_buffer =
      (WORD8 *)((WORD8 *)aac_persistent_mem_v + persistent_used);

  memset((WORD16 *)((WORD8 *)aac_persistent_mem_v + persistent_used), 0,
         num_channel *
             IXHEAAC_GET_SIZE_ALIGNED(
                 MAXSBRBYTES * sizeof(aac_persistent_mem->sbr_payload_buffer[0]), BYTE_ALIGN_8));

  persistent_used +=
      num_channel *
      IXHEAAC_GET_SIZE_ALIGNED(MAXSBRBYTES * sizeof(aac_persistent_mem->sbr_payload_buffer[0]),
                               BYTE_ALIGN_8);

  aac_persistent_mem->prev_sbr_payload_buffer =
      (WORD8 *)((WORD8 *)aac_persistent_mem_v + persistent_used);

  memset((WORD8 *)aac_persistent_mem->prev_sbr_payload_buffer, 0,
         num_channel * IXHEAAC_GET_SIZE_ALIGNED(
                           MAXSBRBYTES * sizeof(*(aac_persistent_mem->prev_sbr_payload_buffer)),
                           BYTE_ALIGN_8));

  persistent_used +=
      num_channel *
      IXHEAAC_GET_SIZE_ALIGNED(
          MAXSBRBYTES * sizeof(*(aac_persistent_mem->prev_sbr_payload_buffer)), BYTE_ALIGN_8);

  {
    WORD32 i;

    for (i = 0; i < num_channel; i++) {
      aac_persistent_mem->ltp_buf[i] =
          (WORD16 *)((WORD8 *)aac_persistent_mem_v + persistent_used);

      memset((WORD16 *)((WORD8 *)aac_persistent_mem_v + persistent_used), 0,
             IXHEAAC_GET_SIZE_ALIGNED(ltp_buffer_size * sizeof(aac_persistent_mem->ltp_buf[i][0]),
                                      BYTE_ALIGN_8));

      persistent_used += IXHEAAC_GET_SIZE_ALIGNED(
          ltp_buffer_size * sizeof(aac_persistent_mem->ltp_buf[i][0]), BYTE_ALIGN_8);

      aac_persistent_mem->ptr_aac_dec_static_channel_info[i] =
          (ia_aac_dec_channel_info *)((WORD8 *)aac_persistent_mem_v + persistent_used);
      persistent_used += IXHEAAC_GET_SIZE_ALIGNED(sizeof(ia_aac_dec_channel_info), BYTE_ALIGN_8);

      aac_persistent_mem->ptr_aac_dec_static_channel_info[i]
          ->overlap_add_data.win_shape = 0;
      aac_persistent_mem->ptr_aac_dec_static_channel_info[i]
          ->overlap_add_data.win_seq = 0;

      aac_persistent_mem->ptr_aac_dec_static_channel_info[i]
          ->overlap_add_data.ptr_overlap_buf =
          &aac_persistent_mem->overlap_buffer[i * OVERLAP_BUFFER_SIZE];
    }
  }

  return persistent_used;
}

VOID ixheaacd_huff_tables_create(ia_aac_dec_tables_struct *ptr_aac_tables) {
  ptr_aac_tables->code_book[0] = 0;
  ptr_aac_tables->code_book[1] =
      (UWORD16 *)ptr_aac_tables->pstr_huffmann_tables->input_table_cb1;
  ptr_aac_tables->code_book[2] =
      (UWORD16 *)ptr_aac_tables->pstr_huffmann_tables->input_table_cb2;
  ptr_aac_tables->code_book[3] =
      (UWORD16 *)ptr_aac_tables->pstr_huffmann_tables->input_table_cb3;
  ptr_aac_tables->code_book[4] =
      (UWORD16 *)ptr_aac_tables->pstr_huffmann_tables->input_table_cb4;
  ptr_aac_tables->code_book[5] =
      (UWORD16 *)ptr_aac_tables->pstr_huffmann_tables->input_table_cb5;
  ptr_aac_tables->code_book[6] =
      (UWORD16 *)ptr_aac_tables->pstr_huffmann_tables->input_table_cb6;
  ptr_aac_tables->code_book[7] =
      (UWORD16 *)ptr_aac_tables->pstr_huffmann_tables->input_table_cb7;
  ptr_aac_tables->code_book[8] =
      (UWORD16 *)ptr_aac_tables->pstr_huffmann_tables->input_table_cb8;
  ptr_aac_tables->code_book[9] =
      (UWORD16 *)ptr_aac_tables->pstr_huffmann_tables->input_table_cb9;
  ptr_aac_tables->code_book[10] =
      (UWORD16 *)ptr_aac_tables->pstr_huffmann_tables->input_table_cb10;
  ptr_aac_tables->code_book[11] =
      (UWORD16 *)ptr_aac_tables->pstr_huffmann_tables->input_table_cb10;
  ptr_aac_tables->code_book[12] =
      (UWORD16 *)ptr_aac_tables->pstr_huffmann_tables->input_table_cb10;

  ptr_aac_tables->index_table[1] =
      (UWORD32 *)ptr_aac_tables->pstr_huffmann_tables->idx_table_hf1;
  ptr_aac_tables->index_table[2] =
      (UWORD32 *)ptr_aac_tables->pstr_huffmann_tables->idx_table_hf2;
  ptr_aac_tables->index_table[3] =
      (UWORD32 *)ptr_aac_tables->pstr_huffmann_tables->idx_table_hf3;
  ptr_aac_tables->index_table[4] =
      (UWORD32 *)ptr_aac_tables->pstr_huffmann_tables->idx_table_hf4;
  ptr_aac_tables->index_table[5] =
      (UWORD32 *)ptr_aac_tables->pstr_huffmann_tables->idx_table_hf5;
  ptr_aac_tables->index_table[6] =
      (UWORD32 *)ptr_aac_tables->pstr_huffmann_tables->idx_table_hf6;
  ptr_aac_tables->index_table[7] =
      (UWORD32 *)ptr_aac_tables->pstr_huffmann_tables->idx_table_hf7;
  ptr_aac_tables->index_table[8] =
      (UWORD32 *)ptr_aac_tables->pstr_huffmann_tables->idx_table_hf8;
  ptr_aac_tables->index_table[9] =
      (UWORD32 *)ptr_aac_tables->pstr_huffmann_tables->idx_table_hf9;
  ptr_aac_tables->index_table[10] =
      (UWORD32 *)ptr_aac_tables->pstr_huffmann_tables->idx_table_hf10;
  ptr_aac_tables->index_table[11] =
      (UWORD32 *)ptr_aac_tables->pstr_huffmann_tables->idx_table_hf10;
  ptr_aac_tables->index_table[12] =
      (UWORD32 *)ptr_aac_tables->pstr_huffmann_tables->idx_table_hf10;

  ptr_aac_tables->scale_factor_bands_short[0] =
      ptr_aac_tables->pstr_huffmann_tables->ixheaacd_sfb_96_128;
  ptr_aac_tables->scale_factor_bands_short[1] =
      ptr_aac_tables->pstr_huffmann_tables->ixheaacd_sfb_96_128;
  ptr_aac_tables->scale_factor_bands_short[2] =
      ptr_aac_tables->pstr_huffmann_tables->ixheaacd_sfb_96_128;
  ptr_aac_tables->scale_factor_bands_short[3] =
      ptr_aac_tables->pstr_huffmann_tables->ixheaacd_sfb_48_128;
  ptr_aac_tables->scale_factor_bands_short[4] =
      ptr_aac_tables->pstr_huffmann_tables->ixheaacd_sfb_48_128;
  ptr_aac_tables->scale_factor_bands_short[5] =
      ptr_aac_tables->pstr_huffmann_tables->ixheaacd_sfb_48_128;
  ptr_aac_tables->scale_factor_bands_short[6] =
      ptr_aac_tables->pstr_huffmann_tables->ixheaacd_sfb_24_128;
  ptr_aac_tables->scale_factor_bands_short[7] =
      ptr_aac_tables->pstr_huffmann_tables->ixheaacd_sfb_24_128;
  ptr_aac_tables->scale_factor_bands_short[8] =
      ptr_aac_tables->pstr_huffmann_tables->ixheaacd_sfb_16_128;
  ptr_aac_tables->scale_factor_bands_short[9] =
      ptr_aac_tables->pstr_huffmann_tables->ixheaacd_sfb_16_128;
  ptr_aac_tables->scale_factor_bands_short[10] =
      ptr_aac_tables->pstr_huffmann_tables->ixheaacd_sfb_16_128;
  ptr_aac_tables->scale_factor_bands_short[11] =
      ptr_aac_tables->pstr_huffmann_tables->ixheaacd_sfb_8_128;

  ptr_aac_tables->scale_factor_bands_short[12] =
      ptr_aac_tables->pstr_huffmann_tables->ixheaacd_sfb_96_120;
  ptr_aac_tables->scale_factor_bands_short[13] =
      ptr_aac_tables->pstr_huffmann_tables->ixheaacd_sfb_96_120;
  ptr_aac_tables->scale_factor_bands_short[14] =
      ptr_aac_tables->pstr_huffmann_tables->ixheaacd_sfb_96_120;
  ptr_aac_tables->scale_factor_bands_short[15] =
      ptr_aac_tables->pstr_huffmann_tables->ixheaacd_sfb_48_120;
  ptr_aac_tables->scale_factor_bands_short[16] =
      ptr_aac_tables->pstr_huffmann_tables->ixheaacd_sfb_48_120;
  ptr_aac_tables->scale_factor_bands_short[17] =
      ptr_aac_tables->pstr_huffmann_tables->ixheaacd_sfb_48_120;
  ptr_aac_tables->scale_factor_bands_short[18] =
      ptr_aac_tables->pstr_huffmann_tables->ixheaacd_sfb_24_120;
  ptr_aac_tables->scale_factor_bands_short[19] =
      ptr_aac_tables->pstr_huffmann_tables->ixheaacd_sfb_24_120;
  ptr_aac_tables->scale_factor_bands_short[20] =
      ptr_aac_tables->pstr_huffmann_tables->ixheaacd_sfb_16_120;
  ptr_aac_tables->scale_factor_bands_short[21] =
      ptr_aac_tables->pstr_huffmann_tables->ixheaacd_sfb_16_120;
  ptr_aac_tables->scale_factor_bands_short[22] =
      ptr_aac_tables->pstr_huffmann_tables->ixheaacd_sfb_16_120;
  ptr_aac_tables->scale_factor_bands_short[23] =
      ptr_aac_tables->pstr_huffmann_tables->ixheaacd_sfb_8_120;

  ptr_aac_tables->scale_factor_bands_long[0] =
      ptr_aac_tables->pstr_huffmann_tables->ixheaacd_sfb_96_1024;
  ptr_aac_tables->scale_factor_bands_long[1] =
      ptr_aac_tables->pstr_huffmann_tables->ixheaacd_sfb_96_1024;
  ptr_aac_tables->scale_factor_bands_long[2] =
      ptr_aac_tables->pstr_huffmann_tables->ixheaacd_sfb_64_1024;
  ptr_aac_tables->scale_factor_bands_long[3] =
      ptr_aac_tables->pstr_huffmann_tables->ixheaacd_sfb_48_1024;
  ptr_aac_tables->scale_factor_bands_long[4] =
      ptr_aac_tables->pstr_huffmann_tables->ixheaacd_sfb_48_1024;
  ptr_aac_tables->scale_factor_bands_long[5] =
      ptr_aac_tables->pstr_huffmann_tables->ixheaacd_sfb_32_1024;
  ptr_aac_tables->scale_factor_bands_long[6] =
      ptr_aac_tables->pstr_huffmann_tables->ixheaacd_sfb_24_1024;
  ptr_aac_tables->scale_factor_bands_long[7] =
      ptr_aac_tables->pstr_huffmann_tables->ixheaacd_sfb_24_1024;
  ptr_aac_tables->scale_factor_bands_long[8] =
      ptr_aac_tables->pstr_huffmann_tables->ixheaacd_sfb_16_1024;
  ptr_aac_tables->scale_factor_bands_long[9] =
      ptr_aac_tables->pstr_huffmann_tables->ixheaacd_sfb_16_1024;
  ptr_aac_tables->scale_factor_bands_long[10] =
      ptr_aac_tables->pstr_huffmann_tables->ixheaacd_sfb_16_1024;
  ptr_aac_tables->scale_factor_bands_long[11] =
      ptr_aac_tables->pstr_huffmann_tables->ixheaacd_sfb_8_1024;

  ptr_aac_tables->scale_factor_bands_long[12] =
      ptr_aac_tables->pstr_huffmann_tables->ixheaacd_sfb_96_960;
  ptr_aac_tables->scale_factor_bands_long[13] =
      ptr_aac_tables->pstr_huffmann_tables->ixheaacd_sfb_96_960;
  ptr_aac_tables->scale_factor_bands_long[14] =
      ptr_aac_tables->pstr_huffmann_tables->ixheaacd_sfb_64_960;
  ptr_aac_tables->scale_factor_bands_long[15] =
      ptr_aac_tables->pstr_huffmann_tables->ixheaacd_sfb_48_960;
  ptr_aac_tables->scale_factor_bands_long[16] =
      ptr_aac_tables->pstr_huffmann_tables->ixheaacd_sfb_48_960;
  ptr_aac_tables->scale_factor_bands_long[17] =
      ptr_aac_tables->pstr_huffmann_tables->ixheaacd_sfb_48_960;
  ptr_aac_tables->scale_factor_bands_long[18] =
      ptr_aac_tables->pstr_huffmann_tables->ixheaacd_sfb_24_960;
  ptr_aac_tables->scale_factor_bands_long[19] =
      ptr_aac_tables->pstr_huffmann_tables->ixheaacd_sfb_24_960;
  ptr_aac_tables->scale_factor_bands_long[20] =
      ptr_aac_tables->pstr_huffmann_tables->ixheaacd_sfb_16_960;
  ptr_aac_tables->scale_factor_bands_long[21] =
      ptr_aac_tables->pstr_huffmann_tables->ixheaacd_sfb_16_960;
  ptr_aac_tables->scale_factor_bands_long[22] =
      ptr_aac_tables->pstr_huffmann_tables->ixheaacd_sfb_16_960;
  ptr_aac_tables->scale_factor_bands_long[23] =
      ptr_aac_tables->pstr_huffmann_tables->ixheaacd_sfb_8_960;

  ptr_aac_tables->scale_fac_bands_512[0] =
      ptr_aac_tables->pstr_huffmann_tables->ixheaacd_sfb_48_512;
  ptr_aac_tables->scale_fac_bands_512[1] =
      ptr_aac_tables->pstr_huffmann_tables->ixheaacd_sfb_48_512;
  ptr_aac_tables->scale_fac_bands_512[2] =
      ptr_aac_tables->pstr_huffmann_tables->ixheaacd_sfb_48_512;
  ptr_aac_tables->scale_fac_bands_512[3] =
      ptr_aac_tables->pstr_huffmann_tables->ixheaacd_sfb_48_512;
  ptr_aac_tables->scale_fac_bands_512[4] =
      ptr_aac_tables->pstr_huffmann_tables->ixheaacd_sfb_48_512;
  ptr_aac_tables->scale_fac_bands_512[5] =
      ptr_aac_tables->pstr_huffmann_tables->ixheaacd_sfb_32_512;
  ptr_aac_tables->scale_fac_bands_512[6] =
      ptr_aac_tables->pstr_huffmann_tables->ixheaacd_sfb_24_512;
  ptr_aac_tables->scale_fac_bands_512[7] =
      ptr_aac_tables->pstr_huffmann_tables->ixheaacd_sfb_24_512;
  ptr_aac_tables->scale_fac_bands_512[8] =
      ptr_aac_tables->pstr_huffmann_tables->ixheaacd_sfb_24_512;
  ptr_aac_tables->scale_fac_bands_512[9] =
      ptr_aac_tables->pstr_huffmann_tables->ixheaacd_sfb_24_512;
  ptr_aac_tables->scale_fac_bands_512[10] =
      ptr_aac_tables->pstr_huffmann_tables->ixheaacd_sfb_24_512;
  ptr_aac_tables->scale_fac_bands_512[11] =
      ptr_aac_tables->pstr_huffmann_tables->ixheaacd_sfb_24_512;
  ptr_aac_tables->scale_fac_bands_512[12] =
      ptr_aac_tables->pstr_huffmann_tables->ixheaacd_sfb_24_512;
  ptr_aac_tables->scale_fac_bands_512[13] =
      ptr_aac_tables->pstr_huffmann_tables->ixheaacd_sfb_24_512;
  ptr_aac_tables->scale_fac_bands_512[14] =
      ptr_aac_tables->pstr_huffmann_tables->ixheaacd_sfb_24_512;
  ptr_aac_tables->scale_fac_bands_512[15] =
      ptr_aac_tables->pstr_huffmann_tables->ixheaacd_sfb_24_512;

  ptr_aac_tables->scale_fac_bands_480[0] =
      ptr_aac_tables->pstr_huffmann_tables->ixheaacd_sfb_48_480;
  ptr_aac_tables->scale_fac_bands_480[1] =
      ptr_aac_tables->pstr_huffmann_tables->ixheaacd_sfb_48_480;
  ptr_aac_tables->scale_fac_bands_480[2] =
      ptr_aac_tables->pstr_huffmann_tables->ixheaacd_sfb_48_480;
  ptr_aac_tables->scale_fac_bands_480[3] =
      ptr_aac_tables->pstr_huffmann_tables->ixheaacd_sfb_48_480;
  ptr_aac_tables->scale_fac_bands_480[4] =
      ptr_aac_tables->pstr_huffmann_tables->ixheaacd_sfb_48_480;
  ptr_aac_tables->scale_fac_bands_480[5] =
      ptr_aac_tables->pstr_huffmann_tables->ixheaacd_sfb_32_480;
  ptr_aac_tables->scale_fac_bands_480[6] =
      ptr_aac_tables->pstr_huffmann_tables->ixheaacd_sfb_24_480;
  ptr_aac_tables->scale_fac_bands_480[7] =
      ptr_aac_tables->pstr_huffmann_tables->ixheaacd_sfb_24_480;
  ptr_aac_tables->scale_fac_bands_480[8] =
      ptr_aac_tables->pstr_huffmann_tables->ixheaacd_sfb_24_480;
  ptr_aac_tables->scale_fac_bands_480[9] =
      ptr_aac_tables->pstr_huffmann_tables->ixheaacd_sfb_24_480;
  ptr_aac_tables->scale_fac_bands_480[10] =
      ptr_aac_tables->pstr_huffmann_tables->ixheaacd_sfb_24_480;
  ptr_aac_tables->scale_fac_bands_480[11] =
      ptr_aac_tables->pstr_huffmann_tables->ixheaacd_sfb_24_480;
  ptr_aac_tables->scale_fac_bands_480[12] =
      ptr_aac_tables->pstr_huffmann_tables->ixheaacd_sfb_24_480;
  ptr_aac_tables->scale_fac_bands_480[13] =
      ptr_aac_tables->pstr_huffmann_tables->ixheaacd_sfb_24_480;
  ptr_aac_tables->scale_fac_bands_480[14] =
      ptr_aac_tables->pstr_huffmann_tables->ixheaacd_sfb_24_480;
  ptr_aac_tables->scale_fac_bands_480[15] =
      ptr_aac_tables->pstr_huffmann_tables->ixheaacd_sfb_24_480;
}

ia_aac_decoder_struct *ixheaacd_aac_decoder_init(
    ia_aac_dec_state_struct *p_state_enhaacplus_dec,
    ia_aac_dec_sbr_bitstream_struct *ptr_sbr_bitstream, WORD channels,
    VOID *aac_persistent_mem_v, WORD32 frame_length) {
  WORD i, ch;
  struct ia_aac_persistent_struct *aac_persistent_mem;
  aac_persistent_mem = (struct ia_aac_persistent_struct *)aac_persistent_mem_v;

  aac_persistent_mem->str_aac_decoder.pstr_sbr_bitstream = ptr_sbr_bitstream;

  for (ch = 0; ch < channels; ch++) {
    ia_aac_decoder_struct *aac_dec_handle =
        &aac_persistent_mem->str_aac_decoder;
    aac_dec_handle->pstr_aac_dec_overlap_info[ch] =
        &aac_persistent_mem->str_aac_dec_overlap_info[ch];
    aac_dec_handle->pstr_pns_rand_vec_data =
        &aac_persistent_mem->str_pns_rand_vec_data;

    aac_dec_handle->pstr_aac_dec_overlap_info[ch]->window_shape = 0;
    aac_dec_handle->pstr_aac_dec_overlap_info[ch]->window_sequence = 0;
    if (p_state_enhaacplus_dec->audio_object_type == AOT_ER_AAC_ELD)
      aac_dec_handle->pstr_aac_dec_overlap_info[ch]->ptr_overlap_buf =
          &aac_persistent_mem->overlap_buffer[ch * 4 * OVERLAP_BUFFER_SIZE];
    else
      aac_dec_handle->pstr_aac_dec_overlap_info[ch]->ptr_overlap_buf =
          &aac_persistent_mem->overlap_buffer[ch * OVERLAP_BUFFER_SIZE];

    {
      WORD32 *ptr_overlap_buf =
          aac_dec_handle->pstr_aac_dec_overlap_info[ch]->ptr_overlap_buf;
      memset(ptr_overlap_buf, 0,
             IXHEAAC_GET_SIZE_ALIGNED(sizeof(ptr_overlap_buf[0]) * 4 * 512, BYTE_ALIGN_8));
    }
    aac_persistent_mem->str_aac_decoder.ptr_aac_dec_static_channel_info[ch] =
        aac_persistent_mem->ptr_aac_dec_static_channel_info[ch];
    aac_persistent_mem->str_aac_decoder.ptr_aac_dec_static_channel_info[ch]
        ->ltp_buf = aac_persistent_mem->ltp_buf[ch];
  }

  for (i = 0; i < 1; i++) {
    ia_aac_dec_sbr_bitstream_struct *ptr_sbr_bitstream =
        &aac_persistent_mem->str_aac_decoder.pstr_sbr_bitstream[i];

    ptr_sbr_bitstream->no_elements = 0;
    ptr_sbr_bitstream->str_sbr_ele[0].ptr_sbr_data =
        &aac_persistent_mem->sbr_payload_buffer[ALIGN_SIZE64(MAXSBRBYTES) * i];
    ptr_sbr_bitstream->str_sbr_ele[0].ptr_prev_sbr_data =
        &aac_persistent_mem->prev_sbr_payload_buffer[ALIGN_SIZE64(MAXSBRBYTES) * i];
    ptr_sbr_bitstream->str_sbr_ele[0].sbr_ele_id = ID_SCE;
    ptr_sbr_bitstream->str_sbr_ele[0].size_payload = 0;
    ptr_sbr_bitstream->str_sbr_ele[0].prev_size_payload = 0;
    ptr_sbr_bitstream->str_sbr_ele[0].prev_sbr_ele_id = ID_SCE;
    ptr_sbr_bitstream->str_sbr_ele[0].size_payload_old = 0;
  }

  {
    ia_aac_dec_tables_struct *ptr_aac_tables =
        aac_persistent_mem->str_aac_decoder.pstr_aac_tables;
    ia_aac_dec_huffman_tables_struct *pstr_huffmann_tables =
        ptr_aac_tables->pstr_huffmann_tables;

    WORD num_entries = sizeof(pstr_huffmann_tables->str_sample_rate_info) /
                       sizeof(ia_sampling_rate_info_struct);

    WORD32 sampling_rate = p_state_enhaacplus_dec->sampling_rate;
    WORD sfidx;

    i = 0;
    while ((i < num_entries) &&
           sampling_rate != ((pstr_huffmann_tables->str_sample_rate_info[i]
                                  .sampling_frequency))) {
      i++;
    }
    if (i == 12) {
      i = i - 1;
    }

    if (i == num_entries) {
      return NULL;
    }
    sfidx = i;

    if (frame_length == 1024 || frame_length == 960) {
      WORD16 *psfb_table_idx[2];
      const WORD8 *psfb_width[2];
      WORD width_idx;
      WORD32 j;

      if (frame_length == 960)
        sfidx += 12;

      psfb_table_idx[0] = ptr_aac_tables->sfb_long_table;
      psfb_table_idx[1] = ptr_aac_tables->sfb_short_table;

      psfb_width[0] = ptr_aac_tables->scale_factor_bands_long[sfidx];
      psfb_width[1] = ptr_aac_tables->scale_factor_bands_short[sfidx];

      for (j = 1; j >= 0; j--) {
        const WORD8 *ptr_width = psfb_width[j];
        WORD16 *ptable_idx = psfb_table_idx[j];
        width_idx = 0;
        *ptable_idx++ = width_idx;
        do {
          width_idx += (*ptr_width++);
          *ptable_idx++ = width_idx;
        } while (*ptr_width != -1);

        aac_persistent_mem->str_aac_decoder.num_swb_window[j] =
            (WORD8)(ptr_width - psfb_width[j]);
      }

      {
        ptr_aac_tables->str_aac_sfb_info[0].sfb_index =
            ptr_aac_tables->sfb_long_table;
        ptr_aac_tables->str_aac_sfb_info[1].sfb_index =
            ptr_aac_tables->sfb_long_table;
        ptr_aac_tables->str_aac_sfb_info[3].sfb_index =
            ptr_aac_tables->sfb_long_table;

        ptr_aac_tables->str_aac_sfb_info[2].sfb_index =
            ptr_aac_tables->sfb_short_table;

        ptr_aac_tables->str_aac_sfb_info[0].sfb_width = (WORD8 *)psfb_width[0];
        ptr_aac_tables->str_aac_sfb_info[1].sfb_width = (WORD8 *)psfb_width[0];
        ptr_aac_tables->str_aac_sfb_info[3].sfb_width = (WORD8 *)psfb_width[0];

        ptr_aac_tables->str_aac_sfb_info[2].sfb_width = (WORD8 *)psfb_width[1];
      }
    } else {
      WORD16 *ptr_sfb_idx[2];
      const WORD8 *ptr_sfb_width[2] = {0};
      WORD width_idx;
      WORD32 j;

      ptr_sfb_idx[0] = ptr_aac_tables->sfb_long_table;
      ptr_sfb_idx[1] = ptr_aac_tables->sfb_short_table;

      if (frame_length == 512)
        ptr_sfb_width[0] = ptr_aac_tables->scale_fac_bands_512[i];
      else
        ptr_sfb_width[0] = ptr_aac_tables->scale_fac_bands_480[i];

      for (j = 0; j >= 0; j--) {
        const WORD8 *ptr_width = ptr_sfb_width[j];
        WORD16 *ptr_idx = ptr_sfb_idx[j];
        width_idx = 0;
        *ptr_idx++ = width_idx;
        do {
          width_idx += (*ptr_width++);
          *ptr_idx++ = width_idx;
        } while (*ptr_width != -1);

        aac_persistent_mem->str_aac_decoder.num_swb_window[j] =
            (WORD8)(ptr_width - ptr_sfb_width[j]);
      }

      {
        ptr_aac_tables->str_aac_sfb_info[0].sfb_index =
            ptr_aac_tables->sfb_long_table;
        ptr_aac_tables->str_aac_sfb_info[1].sfb_index =
            ptr_aac_tables->sfb_long_table;
        ptr_aac_tables->str_aac_sfb_info[3].sfb_index =
            ptr_aac_tables->sfb_long_table;
        ptr_aac_tables->str_aac_sfb_info[2].sfb_index =
            ptr_aac_tables->sfb_short_table;

        ptr_aac_tables->str_aac_sfb_info[0].sfb_width =
            (WORD8 *)ptr_sfb_width[0];
        ptr_aac_tables->str_aac_sfb_info[1].sfb_width =
            (WORD8 *)ptr_sfb_width[0];
        ptr_aac_tables->str_aac_sfb_info[3].sfb_width =
            (WORD8 *)ptr_sfb_width[0];
        ptr_aac_tables->str_aac_sfb_info[2].sfb_width =
            (WORD8 *)ptr_sfb_width[1];
      }
    }
    {
      ia_aac_decoder_struct *aac_dec_handle =
          &aac_persistent_mem->str_aac_decoder;
      aac_dec_handle->sampling_rate_index = (WORD16)i;
      aac_dec_handle->sampling_rate = sampling_rate;
      aac_dec_handle->channels = 1;
      aac_dec_handle->block_number = 0;
      aac_dec_handle->samples_per_frame = frame_length;
    }
  }

  return &(aac_persistent_mem->str_aac_decoder);
}
