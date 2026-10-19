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

#include "ixheaac_constants.h"

#include "ixheaac_basic_ops32.h"
#include "ixheaacd_windows.h"

static PLATFORM_INLINE WORD32 ixheaacd_mult32_sh1(WORD32 a, WORD32 b) {
  WORD32 result;
  WORD64 temp_result;

  temp_result = (WORD64)a * (WORD64)b;
  result = (WORD32)(temp_result >> 31);

  return (result);
}

VOID ixheaacd_memset(FLOAT32 *x, WORD32 n) {
  memset(x, 0, n * sizeof(FLOAT32));
  return;
}

VOID ixheaacd_mem_cpy(const FLOAT32 x[], FLOAT32 y[], WORD32 n) {
  memcpy(y, x, n * sizeof(FLOAT32));
  return;
}

VOID ixheaacd_vec_cnst_mul(FLOAT32 a, FLOAT32 x[], FLOAT32 z[], WORD32 n) {
  WORD32 i;
  for (i = 0; i < n; i++) {
    z[i] = (FLOAT32)a * x[i];
  }
  return;
}

VOID ixheaacd_combine_fac(WORD32 *src1, WORD32 *src2, WORD32 *dest, WORD32 len,
                          WORD8 output_q, WORD8 fac_q) {
  WORD32 i;
  if (fac_q > output_q) {
    for (i = 0; i < len; i++) {
      *dest = ixheaac_add32_sat(*src1, ((*src2) >> (fac_q - output_q)));
      dest++;
      src1++;
      src2++;
    }
  } else {
    for (i = 0; i < len; i++) {
      *dest = ixheaac_add32_sat(
          *src1, ixheaac_shl32_sat((*src2), (output_q - fac_q)));
      dest++;
      src1++;
      src2++;
    }
  }
}

WORD8 ixheaacd_windowing_long1(WORD32 *src1, WORD32 *src2,
                               const WORD32 *win_fwd, const WORD32 *win_rev,
                               WORD32 *dest, WORD32 vlen, WORD8 shift1,
                               WORD8 shift2) {
  WORD32 i;
  WORD32 *rsrc2 = src2 + vlen - 1;

  if (shift1 > shift2) {
    for (i = 0; i < vlen / 2; i++) {
      *dest = ixheaac_add32_sat(
          ((ixheaacd_mult32_sh1(*src1, *win_fwd)) >> (shift1 - shift2)),
          ixheaacd_mult32_sh1(*src2, *win_rev));
      *(dest + (vlen - (2 * i)) - 1) = ixheaac_add32_sat(
          ((ixheaacd_mult32_sh1(ixheaac_negate32_sat(*src1), *win_rev)) >>
           (shift1 - shift2)),
          ixheaacd_mult32_sh1(*rsrc2, *win_fwd));

      src1++;
      src2++;
      win_fwd++;
      win_rev--;
      rsrc2--;
      dest++;
    }
    return (shift2);
  } else {
    for (i = 0; i < vlen / 2; i++) {
      *dest = ixheaac_add32_sat(
          ixheaacd_mult32_sh1(*src1, *win_fwd),
          ((ixheaacd_mult32_sh1(*src2, *win_rev)) >> (shift2 - shift1)));

      *(dest + (vlen - (2 * i)) - 1) = ixheaac_add32_sat(
          ixheaacd_mult32_sh1(ixheaac_negate32_sat(*src1), *win_rev),
          ((ixheaacd_mult32_sh1(*rsrc2, *win_fwd)) >> (shift2 - shift1)));
      src1++;
      src2++;
      win_fwd++;
      win_rev--;
      rsrc2--;
      dest++;
    }
    return (shift1);
  }
}

WORD8 ixheaacd_windowing_long2(WORD32 *src1, const WORD32 *win_fwd,
                               WORD32 *fac_data_out, WORD32 *over_lap,
                               WORD32 *p_out_buffer,
                               offset_lengths *ixheaacd_drc_offset,
                               WORD8 shiftp, WORD8 shift_olap, WORD8 fac_q) {
  WORD32 i;
  WORD32 *dest = p_out_buffer;

  win_fwd += ixheaacd_drc_offset->lfac;

  if (shiftp > fac_q) {
    if (shift_olap > fac_q) {
      for (i = 0;
           i < ixheaacd_drc_offset->n_flat_ls + ixheaacd_drc_offset->lfac;
           i++) {
        dest[i] = over_lap[i] >> (shift_olap - fac_q);
      }

      for (i = ixheaacd_drc_offset->n_flat_ls + ixheaacd_drc_offset->lfac;
           i < ixheaacd_drc_offset->n_flat_ls + ixheaacd_drc_offset->n_trans_ls;
           i++) {
        dest[i] = ixheaac_add32_sat(
            (ixheaacd_mult32_sh1(
                 ixheaac_negate32_sat(src1[ixheaacd_drc_offset->n_long / 2 +
                                            ixheaacd_drc_offset->n_flat_ls +
                                            ixheaacd_drc_offset->lfac - i - 1]),
                 *win_fwd) >>
             (shiftp - fac_q)),
            (*fac_data_out));
        win_fwd++;
        fac_data_out++;
      }

      for (;
           i < ixheaacd_drc_offset->n_flat_ls + (ixheaacd_drc_offset->lfac * 3);
           i++) {
        dest[i] = ixheaac_add32_sat(
            (ixheaac_negate32_sat(src1[ixheaacd_drc_offset->n_long / 2 +
                                        ixheaacd_drc_offset->n_flat_ls +
                                        ixheaacd_drc_offset->lfac - i - 1]) >>
             (shiftp - fac_q)),
            (*fac_data_out));
        fac_data_out++;
      }

      for (; i < ixheaacd_drc_offset->n_long; i++) {
        dest[i] =
            ixheaac_negate32_sat(src1[ixheaacd_drc_offset->n_long / 2 +
                                       ixheaacd_drc_offset->n_flat_ls +
                                       ixheaacd_drc_offset->lfac - i - 1]) >>
            (shiftp - fac_q);
      }
      return (fac_q);
    } else {
      memcpy(dest, over_lap, sizeof(WORD32) * (ixheaacd_drc_offset->n_flat_ls +
                                               ixheaacd_drc_offset->lfac));

      for (i = ixheaacd_drc_offset->n_flat_ls + ixheaacd_drc_offset->lfac;
           i < ixheaacd_drc_offset->n_flat_ls + ixheaacd_drc_offset->n_trans_ls;
           i++) {
        dest[i] = ixheaac_add32_sat(
            (ixheaacd_mult32_sh1(
                 ixheaac_negate32_sat(src1[ixheaacd_drc_offset->n_long / 2 +
                                            ixheaacd_drc_offset->n_flat_ls +
                                            ixheaacd_drc_offset->lfac - i - 1]),
                 *win_fwd) >>
             (shiftp - shift_olap)),
            (*fac_data_out) >> (fac_q - shift_olap));
        win_fwd++;
        fac_data_out++;
      }

      for (;
           i < ixheaacd_drc_offset->n_flat_ls + (ixheaacd_drc_offset->lfac * 3);
           i++) {
        dest[i] = ixheaac_add32_sat(
            (ixheaac_negate32_sat(src1[ixheaacd_drc_offset->n_long / 2 +
                                        ixheaacd_drc_offset->n_flat_ls +
                                        ixheaacd_drc_offset->lfac - i - 1]) >>
             (shiftp - shift_olap)),
            (*fac_data_out) >> (fac_q - shift_olap));
        fac_data_out++;
      }

      for (; i < ixheaacd_drc_offset->n_long; i++) {
        dest[i] =
            ixheaac_negate32_sat(src1[ixheaacd_drc_offset->n_long / 2 +
                                       ixheaacd_drc_offset->n_flat_ls +
                                       ixheaacd_drc_offset->lfac - i - 1]) >>
            (shiftp - shift_olap);
      }
      return (shift_olap);
    }
  } else {
    if (shift_olap > shiftp) {
      for (i = 0;
           i < ixheaacd_drc_offset->n_flat_ls + ixheaacd_drc_offset->lfac;
           i++) {
        dest[i] = over_lap[i] >> (shift_olap - shiftp);
      }

      for (i = ixheaacd_drc_offset->n_flat_ls + ixheaacd_drc_offset->lfac;
           i < ixheaacd_drc_offset->n_flat_ls + ixheaacd_drc_offset->n_trans_ls;
           i++) {
        dest[i] = ixheaac_add32_sat(
            ixheaacd_mult32_sh1(
                ixheaac_negate32_sat(src1[ixheaacd_drc_offset->n_long / 2 +
                                           ixheaacd_drc_offset->n_flat_ls +
                                           ixheaacd_drc_offset->lfac - i - 1]),
                *win_fwd),
            (*fac_data_out) >> (fac_q - shiftp));
        win_fwd++;
        fac_data_out++;
      }

      for (;
           i < ixheaacd_drc_offset->n_flat_ls + (ixheaacd_drc_offset->lfac * 3);
           i++) {
        dest[i] = ixheaac_add32_sat(
            ixheaac_negate32_sat(src1[ixheaacd_drc_offset->n_long / 2 +
                                       ixheaacd_drc_offset->n_flat_ls +
                                       ixheaacd_drc_offset->lfac - i - 1]),
            (*fac_data_out) >> (fac_q - shiftp));
        fac_data_out++;
      }

      for (; i < ixheaacd_drc_offset->n_long; i++) {
        dest[i] =
            ixheaac_negate32_sat(src1[ixheaacd_drc_offset->n_long / 2 +
                                       ixheaacd_drc_offset->n_flat_ls +
                                       ixheaacd_drc_offset->lfac - i - 1]);
      }
      return (shiftp);
    } else {
      memcpy(dest, over_lap, sizeof(WORD32) * (ixheaacd_drc_offset->n_flat_ls +
                                               ixheaacd_drc_offset->lfac));

      for (i = ixheaacd_drc_offset->n_flat_ls + ixheaacd_drc_offset->lfac;
           i < ixheaacd_drc_offset->n_flat_ls + ixheaacd_drc_offset->n_trans_ls;
           i++) {
        dest[i] = ixheaac_add32_sat(
            (ixheaacd_mult32_sh1(
                 ixheaac_negate32_sat(src1[ixheaacd_drc_offset->n_long / 2 +
                                            ixheaacd_drc_offset->n_flat_ls +
                                            ixheaacd_drc_offset->lfac - i - 1]),
                 *win_fwd) >>
             (shiftp - shift_olap)),
            (*fac_data_out) >> (fac_q - shift_olap));
        win_fwd++;
        fac_data_out++;
      }

      for (;
           i < ixheaacd_drc_offset->n_flat_ls + (ixheaacd_drc_offset->lfac * 3);
           i++) {
        dest[i] = ixheaac_add32_sat(
            (ixheaac_negate32_sat(src1[ixheaacd_drc_offset->n_long / 2 +
                                        ixheaacd_drc_offset->n_flat_ls +
                                        ixheaacd_drc_offset->lfac - i - 1]) >>
             (shiftp - shift_olap)),
            (*fac_data_out) >> (fac_q - shift_olap));
        fac_data_out++;
      }

      for (; i < ixheaacd_drc_offset->n_long; i++) {
        dest[i] =
            ixheaac_negate32_sat(src1[ixheaacd_drc_offset->n_long / 2 +
                                       ixheaacd_drc_offset->n_flat_ls +
                                       ixheaacd_drc_offset->lfac - i - 1]) >>
            (shiftp - shift_olap);
      }
      return (shift_olap);
    }
  }
}

WORD8 ixheaacd_windowing_long3(WORD32 *src1, const WORD32 *win_fwd,
                               WORD32 *over_lap, WORD32 *p_out_buffer,
                               const WORD32 *win_rev,
                               offset_lengths *ixheaacd_drc_offset,
                               WORD8 shiftp, WORD8 shift_olap) {
  WORD32 i;
  WORD32 *dest = p_out_buffer;

  if (shiftp > shift_olap) {
    memcpy(dest, over_lap, sizeof(FLOAT32) * ixheaacd_drc_offset->n_flat_ls);

    for (i = ixheaacd_drc_offset->n_flat_ls;
         i < ixheaacd_drc_offset->n_long / 2; i++) {
      dest[i] = ixheaac_add32_sat(
          (ixheaacd_mult32_sh1(src1[i], *win_fwd) >> (shiftp - shift_olap)),
          ixheaacd_mult32_sh1(over_lap[i], *win_rev));
      win_fwd++;
      win_rev--;
    }

    for (i = ixheaacd_drc_offset->n_long / 2;
         i < ixheaacd_drc_offset->n_flat_ls + ixheaacd_drc_offset->n_trans_ls;
         i++) {
      dest[i] = ixheaac_add32_sat(
          (ixheaacd_mult32_sh1(
               ixheaac_negate32_sat(src1[ixheaacd_drc_offset->n_long - i - 1]),
               *win_fwd) >>
           (shiftp - shift_olap)),
          ixheaacd_mult32_sh1(over_lap[i], *win_rev));
      win_fwd++;
      win_rev--;
    }

    for (; i < ixheaacd_drc_offset->n_long; i++) {
      dest[i] =
          ixheaac_negate32_sat(src1[ixheaacd_drc_offset->n_long - i - 1]) >>
          (shiftp - shift_olap);
    }

    return (shift_olap);
  } else {
    for (i = 0; i < ixheaacd_drc_offset->n_flat_ls; i++) {
      dest[i] = over_lap[i] >> (shift_olap - shiftp);
    }

    for (i = ixheaacd_drc_offset->n_flat_ls;
         i < ixheaacd_drc_offset->n_long / 2; i++) {
      dest[i] = ixheaac_add32_sat(
          ixheaacd_mult32_sh1(src1[i], *win_fwd),
          ixheaacd_mult32_sh1(over_lap[i], *win_rev) >> (shift_olap - shiftp));
      win_fwd++;
      win_rev--;
    }

    for (i = ixheaacd_drc_offset->n_long / 2;
         i < ixheaacd_drc_offset->n_flat_ls + ixheaacd_drc_offset->n_trans_ls;
         i++) {
      dest[i] = ixheaac_add32_sat(
          ixheaacd_mult32_sh1(
              ixheaac_negate32_sat(src1[ixheaacd_drc_offset->n_long - i - 1]),
              *win_fwd),
          ixheaacd_mult32_sh1(over_lap[i], *win_rev) >> (shift_olap - shiftp));
      win_fwd++;
      win_rev--;
    }

    for (; i < ixheaacd_drc_offset->n_long; i++) {
      dest[i] =
          ixheaac_negate32_sat(src1[ixheaacd_drc_offset->n_long - i - 1]);
    }

    return (shiftp);
  }
}

VOID ixheaacd_windowing_short1(WORD32 *src1, WORD32 *src2, WORD32 *fp,
                               offset_lengths *ixheaacd_drc_offset,
                               WORD8 shiftp, WORD8 shift_olap) {
  WORD32 i;
  WORD32 *dest = fp;

  if (shift_olap > shiftp) {
    if (ixheaacd_drc_offset->n_short > ixheaacd_drc_offset->lfac) {
      for (i = 0; i < ixheaacd_drc_offset->lfac; i++) {
        dest[i] = dest[i] >> (shift_olap - shiftp);
      }
      for (i = ixheaacd_drc_offset->lfac; i < ixheaacd_drc_offset->n_short;
           i++) {
        dest[i] = ixheaacd_mult32_sh1(
            ixheaac_negate32_sat(src1[ixheaacd_drc_offset->n_short - i - 1]),
            src2[i]);
      }

      for (; i < ixheaacd_drc_offset->n_flat_ls + ixheaacd_drc_offset->lfac;
           i++) {
        dest[i] = 0;
      }
    } else {
      for (i = 0; i < ixheaacd_drc_offset->lfac; i++) {
        dest[i] = dest[i] >> (shift_olap - shiftp);
      }
      for (i = ixheaacd_drc_offset->lfac;
           i < ixheaacd_drc_offset->n_flat_ls + ixheaacd_drc_offset->lfac;
           i++) {
        dest[i] = 0;
      }
    }
  } else {
    if (ixheaacd_drc_offset->n_short > ixheaacd_drc_offset->lfac) {
      for (i = ixheaacd_drc_offset->lfac; i < ixheaacd_drc_offset->n_short;
           i++) {
        dest[i] =
            ixheaacd_mult32_sh1(ixheaac_negate32_sat(
                                    src1[ixheaacd_drc_offset->n_short - i - 1]),
                                src2[i]) >>
            (shiftp - shift_olap);
      }

      for (; i < ixheaacd_drc_offset->n_flat_ls + ixheaacd_drc_offset->lfac;
           i++) {
        dest[i] = 0;
      }
    } else {
      for (i = ixheaacd_drc_offset->lfac;
           i < ixheaacd_drc_offset->n_flat_ls + ixheaacd_drc_offset->lfac;
           i++) {
        dest[i] = 0;
      }
    }
  }
}

VOID ixheaacd_windowing_short2(WORD32 *src1, WORD32 *win_fwd, WORD32 *fp,
                               offset_lengths *ixheaacd_drc_offset,
                               WORD8 shiftp, WORD8 shift_olap) {
  WORD32 i;

  WORD32 *win_rev = win_fwd + ixheaacd_drc_offset->n_short - 1;

  if (shift_olap > shiftp) {
    for (i = 0; i < ixheaacd_drc_offset->n_short / 2; i++) {
      fp[i] = ixheaac_add32_sat(
          ixheaacd_mult32_sh1(src1[i], *win_fwd),
          (ixheaacd_mult32_sh1(fp[i], *win_rev) >> (shift_olap - shiftp)));

      fp[ixheaacd_drc_offset->n_short - i - 1] = ixheaac_add32_sat(
          ixheaacd_mult32_sh1(ixheaac_negate32_sat(src1[i]), *win_rev),
          (ixheaacd_mult32_sh1(fp[ixheaacd_drc_offset->n_short - i - 1],
                               *win_fwd) >>
           (shift_olap - shiftp)));
      win_fwd++;
      win_rev--;
    }

    for (i = ixheaacd_drc_offset->n_short;
         i < ixheaacd_drc_offset->n_flat_ls + ixheaacd_drc_offset->n_short;
         i++) {
      fp[i] = 0;
    }
  } else {
    for (i = 0; i < ixheaacd_drc_offset->n_short / 2; i++) {
      fp[i] = ixheaac_add32_sat(
          (ixheaacd_mult32_sh1(src1[i], *win_fwd) >> (shiftp - shift_olap)),
          ixheaacd_mult32_sh1(fp[i], *win_rev));

      fp[ixheaacd_drc_offset->n_short - i - 1] = ixheaac_add32_sat(
          (ixheaacd_mult32_sh1(ixheaac_negate32_sat(src1[i]), *win_rev) >>
           (shiftp - shift_olap)),
          ixheaacd_mult32_sh1(fp[ixheaacd_drc_offset->n_short - i - 1],
                              *win_fwd));

      win_fwd++;
      win_rev--;
    }

    for (i = ixheaacd_drc_offset->n_short;
         i < ixheaacd_drc_offset->n_flat_ls + ixheaacd_drc_offset->n_short;
         i++) {
      fp[i] = 0;
    }
  }
}

WORD8 ixheaacd_windowing_short3(WORD32 *src1, WORD32 *win_rev, WORD32 *fp,
                                WORD32 n_short, WORD8 shiftp,
                                WORD8 shift_olap) {
  WORD32 i;
  const WORD32 *win_fwd = win_rev - n_short + 1;
  if (shift_olap > shiftp) {
    for (i = 0; i < n_short / 2; i++) {
      fp[i] = ixheaac_add32_sat(
          ixheaacd_mult32_sh1(ixheaac_negate32_sat(src1[n_short / 2 - i - 1]),
                              *win_rev),
          (fp[i] >> (shift_olap - shiftp)));

      fp[n_short - i - 1] = ixheaac_add32_sat(
          ixheaacd_mult32_sh1(ixheaac_negate32_sat(src1[n_short / 2 - i - 1]),
                              *win_fwd),
          (fp[n_short - i - 1] >> (shift_olap - shiftp)));
      win_rev--;
      win_fwd++;
    }
    return (shiftp);
  } else {
    for (i = 0; i < n_short / 2; i++) {
      fp[i] = ixheaac_add32_sat(
          (ixheaacd_mult32_sh1(ixheaac_negate32_sat(src1[n_short / 2 - i - 1]),
                               *win_rev) >>
           (shiftp - shift_olap)),
          fp[i]);

      fp[n_short - i - 1] = ixheaac_add32_sat(
          (ixheaacd_mult32_sh1(ixheaac_negate32_sat(src1[n_short / 2 - i - 1]),
                               *win_fwd) >>
           (shiftp - shift_olap)),
          fp[n_short - i - 1]);

      win_rev--;
      win_fwd++;
    }
    return (shift_olap);
  }
}

WORD8 ixheaacd_windowing_short4(WORD32 *src1, WORD32 *win_fwd, WORD32 *fp,
                                WORD32 *win_fwd1, WORD32 n_short, WORD32 flag,
                                WORD8 shiftp, WORD8 shift_olap,
                                WORD8 output_q) {
  WORD32 i;
  const WORD32 *win_rev = win_fwd + n_short - 1;
  const WORD32 *win_rev1 = win_fwd1 - n_short + 1;
  if (shift_olap > output_q) {
    for (i = 0; i < n_short / 2; i++) {
      fp[i] = ixheaac_add32_sat(
          ixheaacd_mult32_sh1(src1[n_short / 2 + i], *win_fwd) >>
              (shiftp - output_q),
          fp[i]);

      fp[n_short - i - 1] = ixheaac_add32_sat(
          ixheaacd_mult32_sh1(ixheaac_negate32_sat(src1[n_short / 2 + i]),
                              *win_rev) >>
              (shiftp - output_q),
          fp[n_short - i - 1]);

      win_fwd++;
      win_rev--;
    }
    if (flag == 1) {
      for (; i < n_short; i++) {
        fp[i + n_short / 2] = ixheaac_add32_sat(
            ixheaacd_mult32_sh1(ixheaac_negate32_sat(src1[n_short - i - 1]),
                                *win_fwd1) >>
                (shiftp - output_q),
            (fp[i + n_short / 2] >> (shift_olap - output_q)));

        fp[3 * n_short - n_short / 2 - i - 1] = ixheaac_add32_sat(
            ixheaacd_mult32_sh1(ixheaac_negate32_sat(src1[n_short - i - 1]),
                                *win_rev1) >>
                (shiftp - output_q),
            (fp[3 * n_short - n_short / 2 - i - 1] >> (shift_olap - output_q)));

        win_fwd1--;
        win_rev1++;
      }
    } else {
      for (; i < n_short; i++) {
        fp[i + n_short / 2] = ixheaac_add32_sat(
            ixheaac_negate32_sat(src1[n_short - i - 1]) >> (shiftp - output_q),
            fp[i + n_short / 2] >> (shift_olap - output_q));
        fp[3 * n_short - n_short / 2 - i - 1] = ixheaac_add32_sat(
            ixheaac_negate32_sat(src1[n_short - i - 1]) >> (shiftp - output_q),
            fp[3 * n_short - n_short / 2 - i - 1] >> (shift_olap - output_q));
      }
    }
    return (output_q);
  } else {
    for (i = 0; i < n_short / 2; i++) {
      fp[i] = ixheaac_add32_sat(
          ixheaacd_mult32_sh1(src1[n_short / 2 + i], *win_fwd) >>
              (shiftp - shift_olap),
          fp[i] >> (output_q - shift_olap));

      fp[n_short - i - 1] = ixheaac_add32_sat(
          ixheaacd_mult32_sh1(ixheaac_negate32_sat(src1[n_short / 2 + i]),
                              *win_rev) >>
              (shiftp - shift_olap),
          fp[n_short - i - 1]);

      win_fwd++;
      win_rev--;
    }
    if (flag == 1) {
      for (; i < n_short; i++) {
        fp[i + n_short / 2] = ixheaac_add32_sat(
            ixheaacd_mult32_sh1(ixheaac_negate32_sat(src1[n_short - i - 1]),
                                *win_fwd1) >>
                (shiftp - shift_olap),
            fp[i + n_short / 2]);

        fp[3 * n_short - n_short / 2 - i - 1] = ixheaac_add32_sat(
            ixheaacd_mult32_sh1(ixheaac_negate32_sat(src1[n_short - i - 1]),
                                *win_rev1) >>
                (shiftp - shift_olap),
            fp[3 * n_short - n_short / 2 - i - 1]);

        win_fwd1--;
        win_rev1++;
      }
    } else {
      for (; i < n_short; i++) {
        fp[i + n_short / 2] =
            ixheaac_add32_sat(ixheaac_negate32_sat(src1[n_short - i - 1]) >>
                                   (shiftp - shift_olap),
                               fp[i + n_short / 2]);
        fp[3 * n_short - n_short / 2 - i - 1] =
            ixheaac_add32_sat(ixheaac_negate32_sat(src1[n_short - i - 1]) >>
                                   (shiftp - shift_olap),
                               fp[3 * n_short - n_short / 2 - i - 1]);
      }
    }
    return (shift_olap);
  }
}

VOID ixheaacd_scale_down(WORD32 *dest, WORD32 *src, WORD32 len, WORD8 shift1,
                         WORD8 shift2) {
  WORD32 i;
  if (shift1 > shift2) {
    for (i = 0; i < len; i++) {
      *dest = *src >> (shift1 - shift2);
      src++;
      dest++;
    }
  } else {
    for (i = 0; i < len; i++) {
      *dest = ixheaac_shl32_sat((*src), (shift2 - shift1));
      src++;
      dest++;
    }
  }
}

VOID ixheaacd_scale_down_adj(WORD32 *dest, WORD32 *src, WORD32 len,
                             WORD8 shift1, WORD8 shift2) {
  WORD32 i;
  if (shift1 > shift2) {
    for (i = 0; i < len; i++) {
      *dest = ixheaac_add32_sat((*src >> (shift1 - shift2)), ADJ_SCALE);
      src++;
      dest++;
    }
  } else {
    for (i = 0; i < len; i++) {
      *dest = ixheaac_add32_sat(ixheaac_shl32_sat((*src), (shift2 - shift1)),
                                 ADJ_SCALE);
      src++;
      dest++;
    }
  }
}
