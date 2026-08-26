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
#include "ixheaacd_sbr_common.h"
#include "ixheaac_type_def.h"

#include "ixheaac_constants.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops16.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaac_basic_ops_arr.h"
#include "ixheaac_basic_ops.h"

#include "ixheaacd_defines.h"
#include "ixheaac_basic_op.h"
#include "ixheaacd_intrinsics.h"
#include "ixheaacd_common_rom.h"
#include "ixheaacd_basic_funcs.h"
#include "ixheaacd_bitbuffer.h"
#include "ixheaacd_sbrdecsettings.h"
#include "ixheaacd_defines.h"
#include "ixheaacd_pns.h"
#include "ixheaacd_aac_rom.h"
#include "ixheaacd_pulsedata.h"

#include "ixheaacd_drc_data_struct.h"
#include "ixheaacd_lt_predict.h"
#include "ixheaacd_cnst.h"
#include "ixheaacd_ec_defines.h"
#include "ixheaacd_ec_struct_def.h"
#include "ixheaacd_channelinfo.h"
#include "ixheaacd_drc_dec.h"
#include "ixheaacd_sbrdecoder.h"
#include "ixheaacd_sbr_scale.h"
#include "ixheaacd_lpp_tran.h"
#include "ixheaacd_env_extr_part.h"
#include "ixheaacd_sbr_rom.h"
#include "ixheaacd_hybrid.h"
#include "ixheaacd_ps_dec.h"
#include "ixheaacd_env_extr.h"

#include "ixheaacd_aac_rom.h"
#include "ixheaacd_ps_dec.h"

#include "ixheaacd_qmf_dec.h"
#include "ixheaacd_env_calc.h"
#include "ixheaac_sbr_const.h"
#include "ixheaacd_pvc_dec.h"
#include "ixheaacd_sbr_dec.h"
#include "ixheaacd_function_selector.h"

VOID ixheaacd_apply_ps(ia_ps_dec_struct *ptr_ps_dec, WORD32 **p_buf_left_real,
                       WORD32 **p_buf_left_imag, WORD32 *p_buf_right_real,
                       WORD32 *p_buf_right_imag,
                       ia_sbr_scale_fact_struct *sbr_scale_factor, WORD16 slot,
                       ia_sbr_tables_struct *sbr_tables_ptr, WORD no_col) {

  WORD16 shiftdelay;

  if (no_col != 30) {
    shiftdelay =
        (WORD16)((slot < (32 - MAX_OV_COLS)) ? 0 : (sbr_scale_factor->lb_scale -
            sbr_scale_factor->ps_scale));
  } else {
  shiftdelay =
      (WORD16)((slot < (no_col - MAX_OV_COLS)) ? 0 : (sbr_scale_factor->lb_scale -
          sbr_scale_factor->ps_scale));
  }

  ixheaacd_hybrid_analysis(p_buf_left_real[HYBRID_FILTER_DELAY],
                           ptr_ps_dec->ptr_hyb_left_re,
                           ptr_ps_dec->ptr_hyb_left_im, &ptr_ps_dec->str_hybrid,
                           shiftdelay, sbr_tables_ptr);

  (*ixheaacd_decorrelation)(ptr_ps_dec, p_buf_left_real[0], p_buf_left_imag[0],
                            p_buf_right_real, p_buf_right_imag,
                            sbr_tables_ptr->ps_tables_ptr);

  (*ixheaacd_apply_rot)(ptr_ps_dec, p_buf_left_real[0], p_buf_left_imag[0],
                        p_buf_right_real, p_buf_right_imag, sbr_tables_ptr,
                        ptr_ps_dec->str_hybrid.ptr_resol);
}

VOID ixheaacd_scale_ps_states(ia_ps_dec_struct *ptr_ps_dec, WORD16 scale) {
  WORD i, m;
  WORD32 delay;

  if (scale > 0) {
    WORD16 scale1 = scale;
    if (scale > 15) scale1 = 15;

    for (m = 0; m < 2; m++)
      ixheaacd_scale_short_vec_left(
          (WORD16 *)&ptr_ps_dec->delay_buf_qmf_ap_re_im[m][3 * 2],
          2 * NUM_OF_QUAD_MIRROR_FILTER_ALL_PASS_CHNLS, scale1);

    delay = 2 * HIGH_DEL * SMALL_DEL_STRT +
            2 * SMALL_DEL *
                (NUM_OF_QUAD_MIRROR_FILTER_ICC_CHNLS -
                 (NUM_OF_QUAD_MIRROR_FILTER_ALL_PASS_CHNLS + SMALL_DEL_STRT));
    ixheaacd_scale_short_vec_left((WORD16 *)ptr_ps_dec->delay_buf_qmf_ld_re_im,
                                  delay, scale1);

    delay = 2 * 16 * DEL_ALL_PASS + 2 * NUM_SER_AP_LINKS * 5 * 16;
    ixheaacd_scale_short_vec_left((WORD16 *)ptr_ps_dec->delay_buf_qmf_sub_re_im,
                                  delay, scale1);

    for (i = 0; i < NUM_SER_AP_LINKS; i++) {
      for (m = 0; m < ptr_ps_dec->delay_sample_ser[i]; m++) {
        ixheaacd_scale_short_vec_left(
            (WORD16 *)&ptr_ps_dec->delay_buf_qmf_ser_re_im[m][i][2 * 3],
            2 * NUM_OF_QUAD_MIRROR_FILTER_ALL_PASS_CHNLS, scale1);
      }
    }

    ixheaacd_scale_int_vec_left(
        ptr_ps_dec->str_hybrid.ptr_qmf_buf_re[0],
        2 * NO_QMF_CHANNELS_IN_HYBRID * ptr_ps_dec->str_hybrid.ptr_qmf_buf,
        scale);

    scale = (scale + scale);

    ixheaacd_scale_int_vec_left(ptr_ps_dec->peak_decay_diff, 3 * NUM_OF_BINS,
                                scale);

  } else {
    if (scale != 0) {
      scale = -scale;
      for (m = 0; m < 2; m++)
        ixheaacd_scale_short_vec_right(
            (WORD16 *)&ptr_ps_dec->delay_buf_qmf_ap_re_im[m][3 * 2],
            2 * NUM_OF_QUAD_MIRROR_FILTER_ALL_PASS_CHNLS, scale);

      delay = 2 * HIGH_DEL * SMALL_DEL_STRT +
              2 * SMALL_DEL *
                  (NUM_OF_QUAD_MIRROR_FILTER_ICC_CHNLS -
                   (NUM_OF_QUAD_MIRROR_FILTER_ALL_PASS_CHNLS + SMALL_DEL_STRT));
      ixheaacd_scale_short_vec_right(
          (WORD16 *)ptr_ps_dec->delay_buf_qmf_ld_re_im, delay, scale);

      delay = 2 * 16 * DEL_ALL_PASS + 2 * NUM_SER_AP_LINKS * 5 * 16;
      ixheaacd_scale_short_vec_right(
          (WORD16 *)ptr_ps_dec->delay_buf_qmf_sub_re_im, delay, scale);

      for (i = 0; i < NUM_SER_AP_LINKS; i++) {
        for (m = 0; m < ptr_ps_dec->delay_sample_ser[i]; m++) {
          ixheaacd_scale_short_vec_right(
              (WORD16 *)&ptr_ps_dec->delay_buf_qmf_ser_re_im[m][i][3 * 2],
              2 * NUM_OF_QUAD_MIRROR_FILTER_ALL_PASS_CHNLS, scale);
        }
      }

      ixheaacd_scale_int_vec_right(
          ptr_ps_dec->str_hybrid.ptr_qmf_buf_re[0],
          2 * NO_QMF_CHANNELS_IN_HYBRID * ptr_ps_dec->str_hybrid.ptr_qmf_buf,
          scale);

      scale = (scale + scale);

      ixheaacd_scale_int_vec_right(ptr_ps_dec->peak_decay_diff, 3 * NUM_OF_BINS,
                                   scale);
    }
  }
}
