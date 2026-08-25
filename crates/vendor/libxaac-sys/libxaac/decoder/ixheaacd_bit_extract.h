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
#ifndef IXHEAACD_BIT_EXTRACT_H
#define IXHEAACD_BIT_EXTRACT_H

typedef struct {
  WORD32 core_mode[2];
  WORD32 common_tw;
  WORD32 common_window;
  WORD32 tns_data_present[2];
  WORD32 tns_active;
  WORD32 common_tns;
  WORD32 tns_on_lr;
  WORD32 tns_present_both;
  WORD32 common_max_sfb;
  UWORD8 max_sfb[2];
  WORD32 max_sfb_ste;
  WORD32 pred_dir;
  WORD32 complex_coef;
  WORD32 use_prev_frame;
  UWORD8 ms_mask_present[2];

} ia_usac_tmp_core_coder_struct;

WORD32 ixheaacd_ics_info(ia_usac_data_struct *usac_data, WORD32 widx,
                         UWORD8 *max_sfb, ia_bit_buf_struct *it_bit_buff,
                         WORD32 window_sequence_last);

VOID ixheaacd_calc_grp_offset(ia_sfb_info_struct *pstr_sfb_info, UWORD8 *group);

VOID ixheaacd_read_tns_u(ia_sfb_info_struct *pstr_sfb_info,
                         ia_tns_frame_info_struct *pstr_tns_frame_info,
                         ia_bit_buf_struct *it_bit_buff);

WORD32 ixheaacd_core_coder_data(WORD32 id, ia_usac_data_struct *usac_data,
                                WORD32 elem_idx, WORD32 chan_offset,
                                ia_bit_buf_struct *it_bit_buff,
                                WORD32 nr_core_coder_channels);

WORD32 ixheaacd_lpd_channel_stream(ia_usac_data_struct *usac_data,
                                   ia_td_frame_data_struct *pstr_td_frame_data,
                                   ia_bit_buf_struct *it_bit_buff,
                                   FLOAT32 *synth);

VOID ixheaacd_acelp_decoding(WORD32 k, ia_usac_data_struct *usac_data,
                             ia_td_frame_data_struct *pstr_td_frame_data,
                             ia_bit_buf_struct *it_bit_buff, WORD32 chan);

VOID ixheaacd_tcx_coding(ia_usac_data_struct *usac_data, WORD32 *quant,
                                 WORD32 k, WORD32 first_tcx_flag,
                                 ia_td_frame_data_struct *pstr_td_frame_data,
                                 ia_bit_buf_struct *it_bit_buff);

WORD32 ixheaacd_win_seq_select(WORD32 window_sequence_curr,
                               WORD32 window_sequence_last);

WORD32 ixheaacd_fd_channel_stream(
    ia_usac_data_struct *usac_data,
    ia_usac_tmp_core_coder_struct *pstr_core_coder, UWORD8 *max_sfb,
    WORD32 window_sequence_last, WORD32 chn, WORD32 noise_filling_config,
    WORD32 ch, ia_bit_buf_struct *it_bit_buff);

VOID ixheaacd_read_fac_data(WORD32 lfac, WORD32 *fac_data,
                            ia_bit_buf_struct *it_bit_buff);

#endif /* IXHEAACD_BIT_EXTRACT_H */
