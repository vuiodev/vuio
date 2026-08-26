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
#include "ixheaac_basic_ops.h"

#include "ixheaac_basic_op.h"
#include "ixheaacd_intrinsics.h"
#include "ixheaacd_common_rom.h"
#include "ixheaacd_basic_funcs.h"

#include "ixheaacd_bitbuffer.h"
#include "ixheaacd_sbrdecsettings.h"
#include "ixheaacd_sbr_scale.h"
#include "ixheaacd_lpp_tran.h"
#include "ixheaacd_env_extr_part.h"
#include "ixheaacd_sbr_rom.h"
#include "ixheaacd_hybrid.h"

#include "ixheaacd_ps_dec.h"

#include "ixheaacd_env_extr.h"

#include "ixheaacd_dsp_fft32x32s.h"

#include "ixheaacd_function_selector.h"

static VOID ixheaacd_filt_2_ch(const WORD32 *ptr_qmf, WORD32 *ptr_hybrid,
                               ia_sbr_tables_struct *ptr_sbr_tables) {
  WORD32 cum0, cum1, cum00, cum11;
  WORD16 *p2_6 = ptr_sbr_tables->ps_tables_ptr->p2_6;

  cum0 = ptr_qmf[HYBRID_FILTER_DELAY] >> 1;
  cum00 = ptr_qmf[HYBRID_FILTER_DELAY + 16] >> 1;
  cum1 = 0L;
  cum11 = 0L;

  {
    cum1 = ixheaac_add32_sat(cum1, ixheaac_mult32x16in32(ptr_qmf[1], *p2_6));
    cum11 =
        ixheaac_add32_sat(cum11, ixheaac_mult32x16in32(ptr_qmf[17], *p2_6++));

    cum1 = ixheaac_add32_sat(cum1, ixheaac_mult32x16in32(ptr_qmf[3], *p2_6));
    cum11 =
        ixheaac_add32_sat(cum11, ixheaac_mult32x16in32(ptr_qmf[19], *p2_6++));

    cum1 = ixheaac_add32_sat(cum1, ixheaac_mult32x16in32(ptr_qmf[5], *p2_6));
    cum11 =
        ixheaac_add32_sat(cum11, ixheaac_mult32x16in32(ptr_qmf[21], *p2_6++));

    cum1 = ixheaac_add32_sat(cum1, ixheaac_mult32x16in32(ptr_qmf[7], *p2_6));
    cum11 =
        ixheaac_add32_sat(cum11, ixheaac_mult32x16in32(ptr_qmf[23], *p2_6++));

    cum1 = ixheaac_add32_sat(cum1, ixheaac_mult32x16in32(ptr_qmf[9], *p2_6));
    cum11 =
        ixheaac_add32_sat(cum11, ixheaac_mult32x16in32(ptr_qmf[25], *p2_6++));

    cum1 = ixheaac_add32_sat(cum1, ixheaac_mult32x16in32(ptr_qmf[11], *p2_6));
    cum11 =
        ixheaac_add32_sat(cum11, ixheaac_mult32x16in32(ptr_qmf[27], *p2_6++));
  }
  cum1 = ixheaac_shl32(cum1, 1);
  cum11 = ixheaac_shl32(cum11, 1);

  ptr_hybrid[0] = ixheaac_add32_sat(cum0, cum1);
  ptr_hybrid[1] = ixheaac_sub32_sat(cum0, cum1);

  ptr_hybrid[16] = ixheaac_add32_sat(cum00, cum11);
  ptr_hybrid[17] = ixheaac_sub32_sat(cum00, cum11);
}

static VOID ixheaacd_filt_8_ch(const WORD32 *ptr_qmf_real,
                               const WORD32 *ptr_qmf_imag, WORD32 *ptr_hyb_real,
                               WORD32 *ptr_hyb_imag,
                               ia_sbr_tables_struct *ptr_sbr_tables) {
  const WORD16 tcos = 0x7642;
  const WORD16 tsin = 0x30fc;
  const WORD16 tcom = 0x5a82;
  WORD32 real, imag;
  WORD32 cum[16];
  const WORD16 *p8_13 = ptr_sbr_tables->ps_tables_ptr->p8_13;
  const WORD16 *p8_13_8 = ptr_sbr_tables->ps_tables_ptr->p8_13 + 8;

  real = ixheaac_shl32(
      ixheaac_add32_sat(ixheaac_mult32x16in32(ptr_qmf_real[0], *p8_13),
                         ixheaac_mult32x16in32(ptr_qmf_real[8], *p8_13_8)),
      1);
  imag = ixheaac_shl32(
      ixheaac_add32_sat(ixheaac_mult32x16in32(ptr_qmf_imag[0], *p8_13++),
                         ixheaac_mult32x16in32(ptr_qmf_imag[8], *p8_13_8++)),
      1);

  cum[12] = ixheaac_shl32(
      ixheaac_mult32x16in32(ixheaac_add32_sat(imag, real), tcom), 1);
  cum[13] = ixheaac_shl32(
      ixheaac_mult32x16in32(ixheaac_sub32_sat(imag, real), tcom), 1);

  real = ixheaac_shl32(
      ixheaac_add32_sat(ixheaac_mult32x16in32(ptr_qmf_real[1], *p8_13),
                         ixheaac_mult32x16in32(ptr_qmf_real[9], *p8_13_8)),
      1);
  imag = ixheaac_shl32(
      ixheaac_add32_sat(ixheaac_mult32x16in32(ptr_qmf_imag[1], *p8_13++),
                         ixheaac_mult32x16in32(ptr_qmf_imag[9], *p8_13_8++)),
      1);

  cum[10] =
      ixheaac_shl32(ixheaac_add32_sat(ixheaac_mult32x16in32(imag, tcos),
                                        ixheaac_mult32x16in32(real, tsin)),
                     1);
  cum[11] =
      ixheaac_shl32(ixheaac_sub32_sat(ixheaac_mult32x16in32(imag, tsin),
                                        ixheaac_mult32x16in32(real, tcos)),
                     1);
  cum[9] = ixheaac_shl32(
      ixheaac_mult32x16in32(
          ixheaac_sub32_sat(ptr_qmf_real[2], ptr_qmf_real[10]), *p8_13_8++),
      1);
  cum[8] = ixheaac_shl32(
      ixheaac_mult32x16in32(
          ixheaac_sub32_sat(ptr_qmf_imag[2], ptr_qmf_imag[10]), *p8_13++),
      1);

  real = ixheaac_shl32(
      ixheaac_add32_sat(ixheaac_mult32x16in32(ptr_qmf_real[3], *p8_13),
                         ixheaac_mult32x16in32(ptr_qmf_real[11], *p8_13_8)),
      1);
  imag = ixheaac_shl32(
      ixheaac_add32_sat(ixheaac_mult32x16in32(ptr_qmf_imag[3], *p8_13++),
                         ixheaac_mult32x16in32(ptr_qmf_imag[11], *p8_13_8++)),
      1);

  cum[6] =
      ixheaac_shl32(ixheaac_sub32_sat(ixheaac_mult32x16in32(imag, tcos),
                                        ixheaac_mult32x16in32(real, tsin)),
                     1);
  cum[7] = ixheaac_shl32(ixheaac_negate32_sat(ixheaac_add32_sat(
                              ixheaac_mult32x16in32(imag, tsin),
                              ixheaac_mult32x16in32(real, tcos))),
                          1);

  real = ixheaac_shl32(
      ixheaac_add32_sat(ixheaac_mult32x16in32(ptr_qmf_real[4], *p8_13),
                         ixheaac_mult32x16in32(ptr_qmf_real[12], *p8_13_8)),
      1);
  imag = ixheaac_shl32(
      ixheaac_add32_sat(ixheaac_mult32x16in32(ptr_qmf_imag[4], *p8_13++),
                         ixheaac_mult32x16in32(ptr_qmf_imag[12], *p8_13_8++)),
      1);

  cum[4] = ixheaac_shl32(
      ixheaac_mult32x16in32(ixheaac_sub32_sat(imag, real), tcom), 1);
  cum[5] = ixheaac_shl32(
      ixheaac_mult32x16in32(
          ixheaac_negate32_sat(ixheaac_add32_sat(imag, real)), tcom),
      1);

  real = ixheaac_shl32(ixheaac_mult32x16in32(ptr_qmf_real[5], *p8_13), 1);
  imag = ixheaac_shl32(ixheaac_mult32x16in32(ptr_qmf_imag[5], *p8_13++), 1);

  cum[2] =
      ixheaac_shl32(ixheaac_sub32_sat(ixheaac_mult32x16in32(real, tcos),
                                        ixheaac_mult32x16in32(imag, tsin)),
                     1);
  cum[3] =
      ixheaac_shl32(ixheaac_add32_sat(ixheaac_mult32x16in32(real, tsin),
                                        ixheaac_mult32x16in32(imag, tcos)),
                     1);

  cum[0] = ixheaac_shl32(
      ixheaac_mult32x16in32(ptr_qmf_real[HYBRID_FILTER_DELAY], *p8_13), 1);
  cum[1] = ixheaac_shl32(
      ixheaac_mult32x16in32(ptr_qmf_imag[HYBRID_FILTER_DELAY], *p8_13++), 1);

  real = ixheaac_shl32(ixheaac_mult32x16in32(ptr_qmf_real[7], *p8_13), 1);
  imag = ixheaac_shl32(ixheaac_mult32x16in32(ptr_qmf_imag[7], *p8_13++), 1);

  cum[14] =
      ixheaac_shl32(ixheaac_add32_sat(ixheaac_mult32x16in32(imag, tsin),
                                        ixheaac_mult32x16in32(real, tcos)),
                     1);
  cum[15] =
      ixheaac_shl32(ixheaac_sub32_sat(ixheaac_mult32x16in32(imag, tcos),
                                        ixheaac_mult32x16in32(real, tsin)),
                     1);

  (*ixheaacd_inv_dit_fft_8pt)(cum, ptr_hyb_real, ptr_hyb_imag);
}

VOID ixheaacd_hybrid_analysis(const WORD32 *ptr_qmf_real, WORD32 *ptr_hyb_real,
                              WORD32 *ptr_hyb_imag,
                              ia_hybrid_struct *ptr_hybrid, WORD16 scale,
                              ia_sbr_tables_struct *ptr_sbr_tables)

{
  WORD band, j;
  WORD chn_offset = 0;
  WORD32 *ptr_re, *ptr_im;
  WORD32 *ptr_temp_real, *ptr_temp_imag;

  for (band = 0; band < NO_QMF_CHANNELS_IN_HYBRID; band++) {
    ptr_re = ptr_hybrid->ptr_qmf_buf_re[band];
    ptr_im = ptr_hybrid->ptr_qmf_buf_im[band];

    ptr_temp_real = &ptr_hybrid->ptr_work_re[0];
    ptr_temp_imag = &ptr_hybrid->ptr_work_im[0];

    *ptr_temp_real = *ptr_re;
    *ptr_temp_imag = *ptr_im;

    ptr_temp_real++;
    ptr_re++;
    ptr_temp_imag++;
    ptr_im++;

    for (j = ptr_hybrid->ptr_qmf_buf - 2; j >= 0; j--) {
      *ptr_temp_real++ = *ptr_re;
      *(ptr_re - 1) = *ptr_re;
      ptr_re++;
      *ptr_temp_imag++ = *ptr_im;
      *(ptr_im - 1) = *ptr_im;
      ptr_im++;
    }

    {
      WORD32 temp_re = ptr_qmf_real[band];
      WORD32 temp_im = ptr_qmf_real[band + 0x40];

      if (scale < 0) {
        temp_re = ixheaac_shl32(temp_re, -scale);
        temp_im = ixheaac_shl32(temp_im, -scale);
      } else {
        temp_re = ixheaac_shr32(temp_re, scale);
        temp_im = ixheaac_shr32(temp_im, scale);
      }
      *ptr_temp_real = temp_re;
      *--ptr_re = temp_re;

      *ptr_temp_imag = temp_im;
      *--ptr_im = temp_im;
    }

    switch (ptr_hybrid->ptr_resol[band]) {
      case NO_HYBRID_CHANNELS_LOW:

        ixheaacd_filt_2_ch(ptr_hybrid->ptr_work_re, &ptr_hyb_real[chn_offset],
                           ptr_sbr_tables);

        chn_offset += 2;

        break;
      case NO_HYBRID_CHANNELS_HIGH:

        ixheaacd_filt_8_ch(ptr_hybrid->ptr_work_re, ptr_hybrid->ptr_work_im,
                           &ptr_hyb_real[chn_offset], &ptr_hyb_imag[chn_offset],
                           ptr_sbr_tables);

        chn_offset += 6;
    }
  }
}
