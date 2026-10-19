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
#include <math.h>
#include "ixheaac_type_def.h"
#include "iusace_cnst.h"
#include "iusace_lpd_rom.h"

static VOID iusace_acelp_ir_vec_corr1(FLOAT32 *ir, FLOAT32 *vec, UWORD8 track, FLOAT32 *sign,
                                      FLOAT32 (*corr_ir)[16], FLOAT32 *corr_out, WORD32 *dn2_pos,
                                      WORD32 num_pluse_pos) {
  WORD16 i, j;
  WORD32 dn;
  WORD32 *dn2;
  FLOAT32 *p0;
  FLOAT32 s;
  dn2 = &dn2_pos[track * 8];
  p0 = corr_ir[track];
  for (i = 0; i < num_pluse_pos; i++) {
    dn = dn2[i];
    s = 0.0F;
    for (j = 0; j < (LEN_SUBFR - dn); j++) {
      s += ir[j] * vec[dn + j];
    }
    corr_out[dn >> 2] = sign[dn] * s + p0[dn >> 2];
  }
}

static VOID iusace_acelp_ir_vec_corr2(FLOAT32 *ir, FLOAT32 *vec, UWORD8 track, FLOAT32 *sign,
                                      FLOAT32 (*corr_ir)[16], FLOAT32 *corr_out) {
  WORD32 i, j;
  FLOAT32 *p0;
  FLOAT32 s;
  p0 = corr_ir[track];
  for (i = 0; i < 16; i++) {
    s = 0.0F;
    for (j = 0; j < LEN_SUBFR - track; j++) {
      s += ir[j] * vec[track + j];
    }
    corr_out[i] = s * sign[track] + p0[i];
    track += 4;
  }
}

static VOID iusace_acelp_get_2p_pos(WORD32 nb_pos_ix, UWORD8 track_p1, UWORD8 track_p2,
                                    FLOAT32 *corr_pulses, FLOAT32 *ener_pulses, WORD32 *pos_p1,
                                    WORD32 *pos_p2, FLOAT32 *dn, WORD32 *dn2, FLOAT32 *corr_p1,
                                    FLOAT32 *corr_p2, FLOAT32 (*corr_p1p2)[256]) {
  WORD32 x, x2, y, x_save = 0, y_save = 0, i, *pos_x;
  FLOAT32 ps0, alp0;
  FLOAT32 ps1, ps2, sq, sqk;
  FLOAT32 alp1, alp2, alpk;
  FLOAT32 *p1, *p2;
  FLOAT32 s;
  pos_x = &dn2[track_p1 << 3];
  ps0 = *corr_pulses;
  alp0 = *ener_pulses;
  sqk = -1.0F;
  alpk = 1.0F;

  for (i = 0; i < nb_pos_ix; i++) {
    x = pos_x[i];
    x2 = x >> 2;

    ps1 = ps0 + dn[x];
    alp1 = alp0 + corr_p1[x2];
    p1 = corr_p2;
    p2 = &corr_p1p2[track_p1][x2 << 4];
    for (y = track_p2; y < LEN_SUBFR; y += 4) {
      ps2 = ps1 + dn[y];
      alp2 = alp1 + (*p1++) + (*p2++);
      sq = ps2 * ps2;
      s = (alpk * sq) - (sqk * alp2);
      if (s > 0.0F) {
        sqk = sq;
        alpk = alp2;
        y_save = y;
        x_save = x;
      }
    }
  }
  *corr_pulses = ps0 + dn[x_save] + dn[y_save];
  *ener_pulses = alpk;
  *pos_p1 = x_save;
  *pos_p2 = y_save;
}

static VOID iusace_acelp_get_1p_pos(UWORD8 track_p1, UWORD8 track_p2, FLOAT32 *corr_pulses,
                                    FLOAT32 *alp, WORD32 *pos_p1, FLOAT32 *dn, FLOAT32 *corr_p1,
                                    FLOAT32 *corr_p2) {
  WORD32 x, x_save = 0;
  FLOAT32 ps0, alp0;
  FLOAT32 ps1, sq, sqk;
  FLOAT32 alp1, alpk;
  FLOAT32 s;

  ps0 = *corr_pulses;
  alp0 = *alp;
  sqk = -1.0F;
  alpk = 1.0F;

  for (x = track_p1; x < LEN_SUBFR; x += 4) {
    ps1 = ps0 + dn[x];
    alp1 = alp0 + corr_p1[x >> 2];
    sq = ps1 * ps1;
    s = (alpk * sq) - (sqk * alp1);
    if (s > 0.0F) {
      sqk = sq;
      alpk = alp1;
      x_save = x;
    }
  }

  if (track_p2 != track_p1) {
    for (x = track_p2; x < LEN_SUBFR; x += 4) {
      ps1 = ps0 + dn[x];
      alp1 = alp0 + corr_p2[x >> 2];
      sq = ps1 * ps1;
      s = (alpk * sq) - (sqk * alp1);
      if (s > 0.0F) {
        sqk = sq;
        alpk = alp1;
        x_save = x;
      }
    }
  }

  *corr_pulses = ps0 + dn[x_save];
  *alp = alpk;
  *pos_p1 = x_save;
}

static WORD32 iusace_acelp_quant_1p_n1bits(WORD32 pos_pulse, WORD32 num_bits_pos) {
  WORD32 mask;
  WORD32 index;
  mask = ((1 << num_bits_pos) - 1);

  index = (pos_pulse & mask);
  if ((pos_pulse & 16) != 0) {
    index += 1 << num_bits_pos;
  }
  return (index);
}

static WORD32 iusace_acelp_quant_2p_2n1bits(WORD32 pos_p1, WORD32 pos_p2, WORD32 num_bits_pos) {
  WORD32 mask;
  WORD32 index;
  mask = ((1 << num_bits_pos) - 1);

  if (((pos_p2 ^ pos_p1) & 16) == 0) {
    if ((pos_p1 - pos_p2) <= 0) {
      index = ((pos_p1 & mask) << num_bits_pos) + (pos_p2 & mask);
    } else {
      index = ((pos_p2 & mask) << num_bits_pos) + (pos_p1 & mask);
    }
    if ((pos_p1 & 16) != 0) {
      index += 1 << (2 * num_bits_pos);
    }
  } else {
    if (((pos_p1 & mask) - (pos_p2 & mask)) <= 0) {
      index = ((pos_p2 & mask) << num_bits_pos) + (pos_p1 & mask);
      if ((pos_p2 & 16) != 0) {
        index += 1 << (2 * num_bits_pos);
      }
    } else {
      index = ((pos_p1 & mask) << num_bits_pos) + (pos_p2 & mask);
      if ((pos_p1 & 16) != 0) {
        index += 1 << (2 * num_bits_pos);
      }
    }
  }
  return (index);
}

static WORD32 iusace_acelp_quant_3p_3n1bits(WORD32 pos_p1, WORD32 pos_p2, WORD32 pos_p3,
                                            WORD32 num_bits_pos) {
  WORD32 nb_pos;
  WORD32 index;
  nb_pos = (1 << (num_bits_pos - 1));

  if (((pos_p1 ^ pos_p2) & nb_pos) == 0) {
    index = iusace_acelp_quant_2p_2n1bits(pos_p1, pos_p2, (num_bits_pos - 1));
    index += (pos_p1 & nb_pos) << num_bits_pos;
    index += iusace_acelp_quant_1p_n1bits(pos_p3, num_bits_pos) << (2 * num_bits_pos);
  } else if (((pos_p1 ^ pos_p3) & nb_pos) == 0) {
    index = iusace_acelp_quant_2p_2n1bits(pos_p1, pos_p3, (num_bits_pos - 1));
    index += (pos_p1 & nb_pos) << num_bits_pos;
    index += iusace_acelp_quant_1p_n1bits(pos_p2, num_bits_pos) << (2 * num_bits_pos);
  } else {
    index = iusace_acelp_quant_2p_2n1bits(pos_p2, pos_p3, (num_bits_pos - 1));
    index += (pos_p2 & nb_pos) << num_bits_pos;
    index += iusace_acelp_quant_1p_n1bits(pos_p1, num_bits_pos) << (2 * num_bits_pos);
  }
  return (index);
}

static WORD32 iusace_acelp_quant_4p_4n1bits(WORD32 pos_p1, WORD32 pos_p2, WORD32 pos_p3,
                                            WORD32 pos_p4, WORD32 num_bits_pos) {
  WORD32 nb_pos;
  WORD32 index;
  nb_pos = (1 << (num_bits_pos - 1));

  if (((pos_p1 ^ pos_p2) & nb_pos) == 0) {
    index = iusace_acelp_quant_2p_2n1bits(pos_p1, pos_p2, (num_bits_pos - 1));
    index += (pos_p1 & nb_pos) << num_bits_pos;
    index += iusace_acelp_quant_2p_2n1bits(pos_p3, pos_p4, num_bits_pos) << (2 * num_bits_pos);
  } else if (((pos_p1 ^ pos_p3) & nb_pos) == 0) {
    index = iusace_acelp_quant_2p_2n1bits(pos_p1, pos_p3, (num_bits_pos - 1));
    index += (pos_p1 & nb_pos) << num_bits_pos;
    index += iusace_acelp_quant_2p_2n1bits(pos_p2, pos_p4, num_bits_pos) << (2 * num_bits_pos);
  } else {
    index = iusace_acelp_quant_2p_2n1bits(pos_p2, pos_p3, (num_bits_pos - 1));
    index += (pos_p2 & nb_pos) << num_bits_pos;
    index += iusace_acelp_quant_2p_2n1bits(pos_p1, pos_p4, num_bits_pos) << (2 * num_bits_pos);
  }
  return (index);
}

static WORD32 iusace_acelp_quant_4p_4nbits(WORD32 *pos_pulses, WORD32 num_bits_pos) {
  WORD32 i, j, k, nb_pos, n_1;
  WORD32 pos_a[4], pos_b[4];
  WORD32 index = 0;
  n_1 = num_bits_pos - 1;
  nb_pos = (1 << n_1);
  i = 0;
  j = 0;
  for (k = 0; k < 4; k++) {
    if ((pos_pulses[k] & nb_pos) == 0) {
      pos_a[i++] = pos_pulses[k];
    } else {
      pos_b[j++] = pos_pulses[k];
    }
  }
  switch (i) {
    case 0:
      index = 1 << ((4 * num_bits_pos) - 3);
      index += iusace_acelp_quant_4p_4n1bits(pos_b[0], pos_b[1], pos_b[2], pos_b[3], n_1);
      break;
    case 1:
      index = iusace_acelp_quant_1p_n1bits(pos_a[0], n_1) << ((3 * n_1) + 1);
      index += iusace_acelp_quant_3p_3n1bits(pos_b[0], pos_b[1], pos_b[2], n_1);
      break;
    case 2:
      index = iusace_acelp_quant_2p_2n1bits(pos_a[0], pos_a[1], n_1) << ((2 * n_1) + 1);
      index += iusace_acelp_quant_2p_2n1bits(pos_b[0], pos_b[1], n_1);
      break;
    case 3:
      index = iusace_acelp_quant_3p_3n1bits(pos_a[0], pos_a[1], pos_a[2], n_1) << num_bits_pos;
      index += iusace_acelp_quant_1p_n1bits(pos_b[0], n_1);
      break;
    case 4:
      index = iusace_acelp_quant_4p_4n1bits(pos_a[0], pos_a[1], pos_a[2], pos_a[3], n_1);
      break;
  }
  index += (i & 3) << ((4 * num_bits_pos) - 2);
  return (index);
}

static WORD32 iusace_acelp_quant_5p_5nbits(WORD32 *pos_pulses, WORD32 num_bits_pos) {
  WORD32 i, j, k, nb_pos, n_1;
  WORD32 pos_a[5], pos_b[5];
  WORD32 index = 0;
  n_1 = num_bits_pos - 1;
  nb_pos = (1 << n_1);
  i = 0;
  j = 0;
  for (k = 0; k < 5; k++) {
    if ((pos_pulses[k] & nb_pos) == 0) {
      pos_a[i++] = pos_pulses[k];
    } else {
      pos_b[j++] = pos_pulses[k];
    }
  }
  switch (i) {
    case 0:
      index = 1 << ((5 * num_bits_pos) - 1);
      index += iusace_acelp_quant_3p_3n1bits(pos_b[0], pos_b[1], pos_b[2], n_1)
               << ((2 * num_bits_pos) + 1);
      index += iusace_acelp_quant_2p_2n1bits(pos_b[3], pos_b[4], num_bits_pos);
      break;
    case 1:
      index = 1 << ((5 * num_bits_pos) - 1);
      index += iusace_acelp_quant_3p_3n1bits(pos_b[0], pos_b[1], pos_b[2], n_1)
               << ((2 * num_bits_pos) + 1);
      index += iusace_acelp_quant_2p_2n1bits(pos_b[3], pos_a[0], num_bits_pos);
      break;
    case 2:
      index = 1 << ((5 * num_bits_pos) - 1);
      index += iusace_acelp_quant_3p_3n1bits(pos_b[0], pos_b[1], pos_b[2], n_1)
               << ((2 * num_bits_pos) + 1);
      index += iusace_acelp_quant_2p_2n1bits(pos_a[0], pos_a[1], num_bits_pos);
      break;
    case 3:
      index = iusace_acelp_quant_3p_3n1bits(pos_a[0], pos_a[1], pos_a[2], n_1)
              << ((2 * num_bits_pos) + 1);
      index += iusace_acelp_quant_2p_2n1bits(pos_b[0], pos_b[1], num_bits_pos);
      break;
    case 4:
      index = iusace_acelp_quant_3p_3n1bits(pos_a[0], pos_a[1], pos_a[2], n_1)
              << ((2 * num_bits_pos) + 1);
      index += iusace_acelp_quant_2p_2n1bits(pos_a[3], pos_b[0], num_bits_pos);
      break;
    case 5:
      index = iusace_acelp_quant_3p_3n1bits(pos_a[0], pos_a[1], pos_a[2], n_1)
              << ((2 * num_bits_pos) + 1);
      index += iusace_acelp_quant_2p_2n1bits(pos_a[3], pos_a[4], num_bits_pos);
      break;
  }
  return (index);
}

static WORD32 iusace_acelp_quant_6p_6n_2bits(WORD32 *pos_pulses, WORD32 num_bits_pos) {
  WORD32 i, j, k, nb_pos, n_1;
  WORD32 pos_a[6], pos_b[6];
  WORD32 index = 0;
  n_1 = num_bits_pos - 1;
  nb_pos = 1 << n_1;
  i = 0;
  j = 0;
  for (k = 0; k < 6; k++) {
    if ((pos_pulses[k] & nb_pos) == 0) {
      pos_a[i++] = pos_pulses[k];
    } else {
      pos_b[j++] = pos_pulses[k];
    }
  }

  switch (i) {
    case 0:
      index = 1 << ((6 * num_bits_pos) - 5);
      index += iusace_acelp_quant_5p_5nbits(pos_b, n_1) << num_bits_pos;
      index += iusace_acelp_quant_1p_n1bits(pos_b[5], n_1);
      break;
    case 1:
      index = 1 << ((6 * num_bits_pos) - 5);
      index += iusace_acelp_quant_5p_5nbits(pos_b, n_1) << num_bits_pos;
      index += iusace_acelp_quant_1p_n1bits(pos_a[0], n_1);
      break;
    case 2:
      index = 1 << ((6 * num_bits_pos) - 5);
      index += iusace_acelp_quant_4p_4nbits(pos_b, n_1) << ((2 * n_1) + 1);
      index += iusace_acelp_quant_2p_2n1bits(pos_a[0], pos_a[1], n_1);
      break;
    case 3:
      index = iusace_acelp_quant_3p_3n1bits(pos_a[0], pos_a[1], pos_a[2], n_1) << ((3 * n_1) + 1);
      index += iusace_acelp_quant_3p_3n1bits(pos_b[0], pos_b[1], pos_b[2], n_1);
      break;
    case 4:
      i = 2;
      index = iusace_acelp_quant_4p_4nbits(pos_a, n_1) << ((2 * n_1) + 1);
      index += iusace_acelp_quant_2p_2n1bits(pos_b[0], pos_b[1], n_1);
      break;
    case 5:
      i = 1;
      index = iusace_acelp_quant_5p_5nbits(pos_a, n_1) << num_bits_pos;
      index += iusace_acelp_quant_1p_n1bits(pos_b[0], n_1);
      break;
    case 6:
      i = 0;
      index = iusace_acelp_quant_5p_5nbits(pos_a, n_1) << num_bits_pos;
      index += iusace_acelp_quant_1p_n1bits(pos_a[5], n_1);
      break;
  }
  index += (i & 3) << ((6 * num_bits_pos) - 4);
  return (index);
}

VOID iusace_acelp_tgt_ir_corr(FLOAT32 *x, FLOAT32 *ir_wsyn, FLOAT32 *corr_out) {
  WORD16 i, j;
  FLOAT32 sum;
  for (i = 0; i < LEN_SUBFR; i++) {
    sum = 0.0F;
    for (j = i; j < LEN_SUBFR; j++) {
      sum += x[j] * ir_wsyn[j - i];
    }
    corr_out[i] = sum;
  }
}

FLOAT32 iusace_acelp_tgt_cb_corr2(FLOAT32 *xn, FLOAT32 *y1, FLOAT32 *corr_out) {
  FLOAT32 gain;
  FLOAT32 t0, t1;
  WORD16 i;
  t0 = xn[0] * y1[0];
  t1 = y1[0] * y1[0];
  for (i = 1; i < LEN_SUBFR; i += 7) {
    t0 += xn[i] * y1[i];
    t1 += y1[i] * y1[i];
    t0 += xn[i + 1] * y1[i + 1];
    t1 += y1[i + 1] * y1[i + 1];
    t0 += xn[i + 2] * y1[i + 2];
    t1 += y1[i + 2] * y1[i + 2];
    t0 += xn[i + 3] * y1[i + 3];
    t1 += y1[i + 3] * y1[i + 3];
    t0 += xn[i + 4] * y1[i + 4];
    t1 += y1[i + 4] * y1[i + 4];
    t0 += xn[i + 5] * y1[i + 5];
    t1 += y1[i + 5] * y1[i + 5];
    t0 += xn[i + 6] * y1[i + 6];
    t1 += y1[i + 6] * y1[i + 6];
  }
  corr_out[0] = t1;
  corr_out[1] = -2.0F * t0 + 0.01F;

  if (t1) {
    gain = t0 / t1;
  } else {
    gain = 1.0F;
  }
  if (gain < 0.0) {
    gain = 0.0;
  } else if (gain > 1.2F) {
    gain = 1.2F;
  }
  return gain;
}

VOID iusace_acelp_tgt_cb_corr1(FLOAT32 *xn, FLOAT32 *y1, FLOAT32 *y2, FLOAT32 *corr_out) {
  WORD32 i;
  FLOAT32 temp1, temp2, temp3;
  temp1 = 0.01F + y2[0] * y2[0];
  temp2 = 0.01F + xn[0] * y2[0];
  temp3 = 0.01F + y1[0] * y2[0];
  temp1 += y2[1] * y2[1];
  temp2 += xn[1] * y2[1];
  temp3 += y1[1] * y2[1];
  temp1 += y2[2] * y2[2];
  temp2 += xn[2] * y2[2];
  temp3 += y1[2] * y2[2];
  temp1 += y2[3] * y2[3];
  temp2 += xn[3] * y2[3];
  temp3 += y1[3] * y2[3];
  for (i = 4; i < LEN_SUBFR; i += 6) {
    temp1 += y2[i] * y2[i];
    temp2 += xn[i] * y2[i];
    temp3 += y1[i] * y2[i];
    temp1 += y2[i + 1] * y2[i + 1];
    temp2 += xn[i + 1] * y2[i + 1];
    temp3 += y1[i + 1] * y2[i + 1];
    temp1 += y2[i + 2] * y2[i + 2];
    temp2 += xn[i + 2] * y2[i + 2];
    temp3 += y1[i + 2] * y2[i + 2];
    temp1 += y2[i + 3] * y2[i + 3];
    temp2 += xn[i + 3] * y2[i + 3];
    temp3 += y1[i + 3] * y2[i + 3];
    temp1 += y2[i + 4] * y2[i + 4];
    temp2 += xn[i + 4] * y2[i + 4];
    temp3 += y1[i + 4] * y2[i + 4];
    temp1 += y2[i + 5] * y2[i + 5];
    temp2 += xn[i + 5] * y2[i + 5];
    temp3 += y1[i + 5] * y2[i + 5];
  }
  corr_out[2] = temp1;
  corr_out[3] = -2.0F * temp2;
  corr_out[4] = 2.0F * temp3;
}

VOID iusace_acelp_cb_target_update(FLOAT32 *x, FLOAT32 *new_x, FLOAT32 *cb_vec, FLOAT32 gain) {
  WORD16 i;
  for (i = 0; i < LEN_SUBFR; i++) {
    new_x[i] = x[i] - gain * cb_vec[i];
  }
}

VOID iusace_acelp_cb_exc(FLOAT32 *corr_input, FLOAT32 *lp_residual, FLOAT32 *ir_wsyn,
                         WORD16 *alg_cb_exc_out, FLOAT32 *filt_cb_exc, WORD32 num_bits_cb,
                         WORD32 *acelp_param_out, FLOAT32 *scratch_acelp_ir_buf) {
  FLOAT32 sign[LEN_SUBFR], vec[LEN_SUBFR];
  FLOAT32 corr_x[16], corr_y[16];
  FLOAT32 *ir_buf = scratch_acelp_ir_buf;
  FLOAT32 corr_ir[4][16];
  FLOAT32 corr_p1p2[4][256];
  FLOAT32 dn2[LEN_SUBFR];
  WORD32 pulse_pos[NPMAXPT * 4] = {0};
  WORD32 codvec[MAX_NUM_PULSES] = {0};
  WORD32 num_pulse_position[10] = {0};
  WORD32 pos_max[4];
  WORD32 dn2_pos[8 * 4];
  UWORD8 ipos[MAX_NUM_PULSES] = {0};
  WORD32 i, j, k, st, pos = 0, index, track, num_pulses = 0, num_iter = 4;
  WORD32 l_index;
  FLOAT32 psk, ps, alpk, alp = 0.0F;
  FLOAT32 val;
  FLOAT32 s, cor;
  FLOAT32 *p0, *p1, *p2, *p3, *psign;
  FLOAT32 *p1_ir_buf, *p2_ir_buf, *p3_ir_buf, *p4_ir_buf, *ir_sign_inv;
  switch (num_bits_cb) {
    case ACELP_NUM_BITS_20:
      num_iter = 4;
      alp = 2.0;
      num_pulses = 4;
      num_pulse_position[0] = 4;
      num_pulse_position[1] = 8;
      break;
    case ACELP_NUM_BITS_28:
      num_iter = 4;
      alp = 1.5;
      num_pulses = 6;
      num_pulse_position[0] = 4;
      num_pulse_position[1] = 8;
      num_pulse_position[2] = 8;
      break;

    case ACELP_NUM_BITS_36:
      num_iter = 4;
      alp = 1.0;
      num_pulses = 8;
      num_pulse_position[0] = 4;
      num_pulse_position[1] = 8;
      num_pulse_position[2] = 8;
      break;
    case ACELP_NUM_BITS_44:
      num_iter = 4;
      alp = 1.0;
      num_pulses = 10;
      num_pulse_position[0] = 4;
      num_pulse_position[1] = 6;
      num_pulse_position[2] = 8;
      num_pulse_position[3] = 8;
      break;
    case ACELP_NUM_BITS_52:
      num_iter = 4;
      alp = 1.0;
      num_pulses = 12;
      num_pulse_position[0] = 4;
      num_pulse_position[1] = 6;
      num_pulse_position[2] = 8;
      num_pulse_position[3] = 8;
      break;
    case ACELP_NUM_BITS_64:
      num_iter = 3;
      alp = 0.8F;
      num_pulses = 16;
      num_pulse_position[0] = 4;
      num_pulse_position[1] = 4;
      num_pulse_position[2] = 6;
      num_pulse_position[3] = 6;
      num_pulse_position[4] = 8;
      num_pulse_position[5] = 8;
      break;
  }

  val = (lp_residual[0] * lp_residual[0]) + 1.0F;
  cor = (corr_input[0] * corr_input[0]) + 1.0F;
  for (i = 1; i < LEN_SUBFR; i += 7) {
    val += (lp_residual[i] * lp_residual[i]);
    cor += (corr_input[i] * corr_input[i]);
    val += (lp_residual[i + 1] * lp_residual[i + 1]);
    cor += (corr_input[i + 1] * corr_input[i + 1]);
    val += (lp_residual[i + 2] * lp_residual[i + 2]);
    cor += (corr_input[i + 2] * corr_input[i + 2]);
    val += (lp_residual[i + 3] * lp_residual[i + 3]);
    cor += (corr_input[i + 3] * corr_input[i + 3]);
    val += (lp_residual[i + 4] * lp_residual[i + 4]);
    cor += (corr_input[i + 4] * corr_input[i + 4]);
    val += (lp_residual[i + 5] * lp_residual[i + 5]);
    cor += (corr_input[i + 5] * corr_input[i + 5]);
    val += (lp_residual[i + 6] * lp_residual[i + 6]);
    cor += (corr_input[i + 6] * corr_input[i + 6]);
  }
  s = (FLOAT32)sqrt(cor / val);
  for (j = 0; j < LEN_SUBFR; j++) {
    cor = (s * lp_residual[j]) + (alp * corr_input[j]);
    if (cor >= 0.0F) {
      sign[j] = 1.0F;
      vec[j] = -1.0F;
      dn2[j] = cor;
    } else {
      sign[j] = -1.0F;
      vec[j] = 1.0F;
      corr_input[j] = -corr_input[j];
      dn2[j] = -cor;
    }
  }
  for (i = 0; i < 4; i++) {
    for (k = 0; k < 8; k++) {
      ps = -1;
      for (j = i; j < LEN_SUBFR; j += 4) {
        if (dn2[j] > ps) {
          ps = dn2[j];
          pos = j;
        }
      }
      dn2[pos] = (FLOAT32)k - 8;
      dn2_pos[i * 8 + k] = pos;
    }
    pos_max[i] = dn2_pos[i * 8];
  }

  memset(ir_buf, 0, LEN_SUBFR * sizeof(FLOAT32));
  memset(ir_buf + (2 * LEN_SUBFR), 0, LEN_SUBFR * sizeof(FLOAT32));
  p1_ir_buf = ir_buf + LEN_SUBFR;
  ir_sign_inv = ir_buf + (3 * LEN_SUBFR);
  memcpy(p1_ir_buf, ir_wsyn, LEN_SUBFR * sizeof(FLOAT32));
  ir_sign_inv[0] = -p1_ir_buf[0];
  ir_sign_inv[1] = -p1_ir_buf[1];
  ir_sign_inv[2] = -p1_ir_buf[2];
  ir_sign_inv[3] = -p1_ir_buf[3];
  for (i = 4; i < LEN_SUBFR; i += 6) {
    ir_sign_inv[i] = -p1_ir_buf[i];
    ir_sign_inv[i + 1] = -p1_ir_buf[i + 1];
    ir_sign_inv[i + 2] = -p1_ir_buf[i + 2];
    ir_sign_inv[i + 3] = -p1_ir_buf[i + 3];
    ir_sign_inv[i + 4] = -p1_ir_buf[i + 4];
    ir_sign_inv[i + 5] = -p1_ir_buf[i + 5];
  }

  p0 = &corr_ir[0][16 - 1];
  p1 = &corr_ir[1][16 - 1];
  p2 = &corr_ir[2][16 - 1];
  p3 = &corr_ir[3][16 - 1];
  p2_ir_buf = p1_ir_buf;
  cor = 0.0F;
  for (i = 0; i < 16; i++) {
    cor += (*p2_ir_buf) * (*p2_ir_buf);
    p2_ir_buf++;
    *p3-- = cor * 0.5F;
    cor += (*p2_ir_buf) * (*p2_ir_buf);
    p2_ir_buf++;
    *p2-- = cor * 0.5F;
    cor += (*p2_ir_buf) * (*p2_ir_buf);
    p2_ir_buf++;
    *p1-- = cor * 0.5F;
    cor += (*p2_ir_buf) * (*p2_ir_buf);
    p2_ir_buf++;
    *p0-- = cor * 0.5F;
  }
  pos = 256 - 1;
  p4_ir_buf = p1_ir_buf + 1;
  for (k = 0; k < 16; k++) {
    p3 = &corr_p1p2[2][pos];
    p2 = &corr_p1p2[1][pos];
    p1 = &corr_p1p2[0][pos];
    if (k == 15) {
      p0 = &corr_p1p2[3][pos - 15];
    } else {
      p0 = &corr_p1p2[3][pos - 16];
    }
    cor = 0.0F;
    p2_ir_buf = p1_ir_buf;
    p3_ir_buf = p4_ir_buf;
    for (i = k + 1; i < 16; i++) {
      cor += (*p2_ir_buf) * (*p3_ir_buf);
      p2_ir_buf++;
      p3_ir_buf++;
      *p3 = cor;
      cor += (*p2_ir_buf) * (*p3_ir_buf);
      p2_ir_buf++;
      p3_ir_buf++;
      *p2 = cor;
      cor += (*p2_ir_buf) * (*p3_ir_buf);
      p2_ir_buf++;
      p3_ir_buf++;
      *p1 = cor;
      cor += (*p2_ir_buf) * (*p3_ir_buf);
      p2_ir_buf++;
      p3_ir_buf++;
      *p0 = cor;
      p3 -= (16 + 1);
      p2 -= (16 + 1);
      p1 -= (16 + 1);
      p0 -= (16 + 1);
    }
    cor += (*p2_ir_buf) * (*p3_ir_buf);
    p2_ir_buf++;
    p3_ir_buf++;
    *p3 = cor;
    cor += (*p2_ir_buf) * (*p3_ir_buf);
    p2_ir_buf++;
    p3_ir_buf++;
    *p2 = cor;
    cor += (*p2_ir_buf) * (*p3_ir_buf);
    p2_ir_buf++;
    p3_ir_buf++;
    *p1 = cor;
    pos -= 16;
    p4_ir_buf += 4;
  }
  pos = 256 - 1;
  p4_ir_buf = p1_ir_buf + 3;
  for (k = 0; k < 16; k++) {
    p3 = &corr_p1p2[3][pos];
    p2 = &corr_p1p2[2][pos - 1];
    p1 = &corr_p1p2[1][pos - 1];
    p0 = &corr_p1p2[0][pos - 1];
    cor = 0.0F;
    p2_ir_buf = p1_ir_buf;
    p3_ir_buf = p4_ir_buf;
    for (i = k + 1; i < 16; i++) {
      cor += (*p2_ir_buf) * (*p3_ir_buf);
      p2_ir_buf++;
      p3_ir_buf++;
      *p3 = cor;
      cor += (*p2_ir_buf) * (*p3_ir_buf);
      p2_ir_buf++;
      p3_ir_buf++;
      *p2 = cor;
      cor += (*p2_ir_buf) * (*p3_ir_buf);
      p2_ir_buf++;
      p3_ir_buf++;
      *p1 = cor;
      cor += (*p2_ir_buf) * (*p3_ir_buf);
      p2_ir_buf++;
      p3_ir_buf++;
      *p0 = cor;
      p3 -= (16 + 1);
      p2 -= (16 + 1);
      p1 -= (16 + 1);
      p0 -= (16 + 1);
    }
    cor += (*p2_ir_buf) * (*p3_ir_buf);
    p2_ir_buf++;
    p3_ir_buf++;
    *p3 = cor;
    pos--;
    p4_ir_buf += 4;
  }

  p0 = &corr_p1p2[0][0];
  for (k = 0; k < 4; k++) {
    for (i = k; i < LEN_SUBFR; i += 4) {
      psign = sign;
      if (psign[i] < 0.0F) {
        psign = vec;
      }
      j = (k + 1) % 4;
      p0[0] = p0[0] * psign[j];
      p0[1] = p0[1] * psign[j + 4];
      p0[2] = p0[2] * psign[j + 8];
      p0[3] = p0[3] * psign[j + 12];
      p0[4] = p0[4] * psign[j + 16];
      p0[5] = p0[5] * psign[j + 20];
      p0[6] = p0[6] * psign[j + 24];
      p0[7] = p0[7] * psign[j + 28];
      p0[8] = p0[8] * psign[j + 32];
      p0[9] = p0[9] * psign[j + 36];
      p0[10] = p0[10] * psign[j + 40];
      p0[11] = p0[11] * psign[j + 44];
      p0[12] = p0[12] * psign[j + 48];
      p0[13] = p0[13] * psign[j + 52];
      p0[14] = p0[14] * psign[j + 56];
      p0[15] = p0[15] * psign[j + 60];
      p0 += 16;
    }
  }
  psk = -1.0;
  alpk = 1.0;
  for (k = 0; k < num_iter; k++) {
    for (i = 0; i < num_pulses - (num_pulses % 3); i += 3) {
      ipos[i] = iusace_acelp_ipos[(k * 4) + i];
      ipos[i + 1] = iusace_acelp_ipos[(k * 4) + i + 1];
      ipos[i + 2] = iusace_acelp_ipos[(k * 4) + i + 2];
    }
    for (; i < num_pulses; i++) {
      ipos[i] = iusace_acelp_ipos[(k * 4) + i];
    }

    if ((num_bits_cb == 20) | (num_bits_cb == 28) | (num_bits_cb == 12) | (num_bits_cb == 16)) {
      pos = 0;
      ps = 0.0F;
      alp = 0.0F;
      memset(vec, 0, LEN_SUBFR * sizeof(FLOAT32));
      if (num_bits_cb == 28) {
        ipos[4] = 0;
        ipos[5] = 1;
      }

      if (num_bits_cb == 16) {
        ipos[0] = 0;
        ipos[1] = 2;
        ipos[2] = 1;
        ipos[3] = 3;
      }
    } else if ((num_bits_cb == 36) | (num_bits_cb == 44)) {
      pos = 2;
      pulse_pos[0] = pos_max[ipos[0]];
      pulse_pos[1] = pos_max[ipos[1]];
      ps = corr_input[pulse_pos[0]] + corr_input[pulse_pos[1]];
      alp = corr_ir[ipos[0]][pulse_pos[0] >> 2] + corr_ir[ipos[1]][pulse_pos[1] >> 2] +
            corr_p1p2[ipos[0]][((pulse_pos[0] >> 2) << 4) + (pulse_pos[1] >> 2)];
      if (sign[pulse_pos[0]] < 0.0) {
        p0 = ir_sign_inv - pulse_pos[0];
      } else {
        p0 = p1_ir_buf - pulse_pos[0];
      }
      if (sign[pulse_pos[1]] < 0.0) {
        p1 = ir_sign_inv - pulse_pos[1];
      } else {
        p1 = p1_ir_buf - pulse_pos[1];
      }
      vec[0] = p0[0] + p1[0];
      vec[1] = p0[1] + p1[1];
      vec[2] = p0[2] + p1[2];
      vec[3] = p0[3] + p1[3];
      for (i = 4; i < LEN_SUBFR; i += 6) {
        vec[i] = p0[i] + p1[i];
        vec[i + 1] = p0[i + 1] + p1[i + 1];
        vec[i + 2] = p0[i + 2] + p1[i + 2];
        vec[i + 3] = p0[i + 3] + p1[i + 3];
        vec[i + 4] = p0[i + 4] + p1[i + 4];
        vec[i + 5] = p0[i + 5] + p1[i + 5];
      }
      if (num_bits_cb == 44) {
        ipos[8] = 0;
        ipos[9] = 1;
      }
    } else {
      pos = 4;
      pulse_pos[0] = pos_max[ipos[0]];
      pulse_pos[1] = pos_max[ipos[1]];
      pulse_pos[2] = pos_max[ipos[2]];
      pulse_pos[3] = pos_max[ipos[3]];
      ps = corr_input[pulse_pos[0]] + corr_input[pulse_pos[1]] + corr_input[pulse_pos[2]] +
           corr_input[pulse_pos[3]];
      p0 = p1_ir_buf - pulse_pos[0];
      if (sign[pulse_pos[0]] < 0.0) {
        p0 = ir_sign_inv - pulse_pos[0];
      }
      p1 = p1_ir_buf - pulse_pos[1];
      if (sign[pulse_pos[1]] < 0.0) {
        p1 = ir_sign_inv - pulse_pos[1];
      }
      p2 = p1_ir_buf - pulse_pos[2];
      if (sign[pulse_pos[2]] < 0.0) {
        p2 = ir_sign_inv - pulse_pos[2];
      }
      p3 = p1_ir_buf - pulse_pos[3];
      if (sign[pulse_pos[3]] < 0.0) {
        p3 = ir_sign_inv - pulse_pos[3];
      }
      vec[0] = p0[0] + p1[0] + p2[0] + p3[0];
      for (i = 1; i < LEN_SUBFR; i += 3) {
        vec[i] = p0[i] + p1[i] + p2[i] + p3[i];
        vec[i + 1] = p0[i + 1] + p1[i + 1] + p2[i + 1] + p3[i + 1];
        vec[i + 2] = p0[i + 2] + p1[i + 2] + p2[i + 2] + p3[i + 2];
      }
      alp = 0.0F;
      alp += vec[0] * vec[0] + vec[1] * vec[1];
      alp += vec[2] * vec[2] + vec[3] * vec[3];
      for (i = 4; i < LEN_SUBFR; i += 6) {
        alp += vec[i] * vec[i];
        alp += vec[i + 1] * vec[i + 1];
        alp += vec[i + 2] * vec[i + 2];
        alp += vec[i + 3] * vec[i + 3];
        alp += vec[i + 4] * vec[i + 4];
        alp += vec[i + 5] * vec[i + 5];
      }
      alp *= 0.5F;
      if (num_bits_cb == 72) {
        ipos[16] = 0;
        ipos[17] = 1;
      }
    }

    for (j = pos, st = 0; j < num_pulses; j += 2, st++) {
      if ((num_pulses - j) >= 2) {
        iusace_acelp_ir_vec_corr1(p1_ir_buf, vec, ipos[j], sign, corr_ir, corr_x, dn2_pos,
                                  num_pulse_position[st]);
        iusace_acelp_ir_vec_corr2(p1_ir_buf, vec, ipos[j + 1], sign, corr_ir, corr_y);

        iusace_acelp_get_2p_pos(num_pulse_position[st], ipos[j], ipos[j + 1], &ps, &alp,
                                &pulse_pos[j], &pulse_pos[j + 1], corr_input, dn2_pos, corr_x,
                                corr_y, corr_p1p2);
      } else {
        iusace_acelp_ir_vec_corr2(p1_ir_buf, vec, ipos[j], sign, corr_ir, corr_x);
        iusace_acelp_ir_vec_corr2(p1_ir_buf, vec, ipos[j + 1], sign, corr_ir, corr_y);
        iusace_acelp_get_1p_pos(ipos[j], ipos[j + 1], &ps, &alp, &pulse_pos[j], corr_input,
                                corr_x, corr_y);
      }
      if (j < (num_pulses - 2)) {
        p0 = p1_ir_buf - pulse_pos[j];
        if (sign[pulse_pos[j]] < 0.0) {
          p0 = ir_sign_inv - pulse_pos[j];
        }
        p1 = p1_ir_buf - pulse_pos[j + 1];
        if (sign[pulse_pos[j + 1]] < 0.0) {
          p1 = ir_sign_inv - pulse_pos[j + 1];
        }
        vec[0] += p0[0] + p1[0];
        vec[1] += p0[1] + p1[1];
        vec[2] += p0[2] + p1[2];
        vec[3] += p0[3] + p1[3];
        for (i = 4; i < LEN_SUBFR; i += 6) {
          vec[i] += p0[i] + p1[i];
          vec[i + 1] += p0[i + 1] + p1[i + 1];
          vec[i + 2] += p0[i + 2] + p1[i + 2];
          vec[i + 3] += p0[i + 3] + p1[i + 3];
          vec[i + 4] += p0[i + 4] + p1[i + 4];
          vec[i + 5] += p0[i + 5] + p1[i + 5];
        }
      }
    }
    ps = ps * ps;
    s = (alpk * ps) - (psk * alp);
    if (s > 0.0F) {
      psk = ps;
      alpk = alp;
      memcpy(codvec, pulse_pos, num_pulses * sizeof(WORD32));
    }
  }

  memset(alg_cb_exc_out, 0, LEN_SUBFR * sizeof(WORD16));
  memset(filt_cb_exc, 0, LEN_SUBFR * sizeof(FLOAT32));
  memset(pulse_pos, 0xffffffff, NPMAXPT * 4 * sizeof(WORD32));
  for (k = 0; k < num_pulses; k++) {
    i = codvec[k];
    val = sign[i];
    index = i / 4;
    track = i % 4;
    if (val > 0) {
      alg_cb_exc_out[i] += 512;
      codvec[k] += (2 * LEN_SUBFR);
    } else {
      alg_cb_exc_out[i] -= 512;
      index += 16;
    }
    i = track * NPMAXPT;
    while (pulse_pos[i] >= 0) {
      i++;
    }
    pulse_pos[i] = index;
    p0 = ir_sign_inv - codvec[k];
    filt_cb_exc[0] += p0[0];
    for (i = 1; i < LEN_SUBFR; i += 3) {
      filt_cb_exc[i] += p0[i];
      filt_cb_exc[i + 1] += p0[i + 1];
      filt_cb_exc[i + 2] += p0[i + 2];
    }
  }

  if (num_bits_cb == ACELP_NUM_BITS_20) {
    for (track = 0; track < 4; track++) {
      k = track * NPMAXPT;
      acelp_param_out[track] = iusace_acelp_quant_1p_n1bits(pulse_pos[k], 4);
    }
  } else if (num_bits_cb == ACELP_NUM_BITS_28) {
    for (track = 0; track < (4 - 2); track++) {
      k = track * NPMAXPT;
      acelp_param_out[track] = iusace_acelp_quant_2p_2n1bits(pulse_pos[k], pulse_pos[k + 1], 4);
    }
    for (track = 2; track < 4; track++) {
      k = track * NPMAXPT;
      acelp_param_out[track] = iusace_acelp_quant_1p_n1bits(pulse_pos[k], 4);
    }
  } else if (num_bits_cb == ACELP_NUM_BITS_36) {
    for (track = 0; track < 4; track++) {
      k = track * NPMAXPT;
      acelp_param_out[track] = iusace_acelp_quant_2p_2n1bits(pulse_pos[k], pulse_pos[k + 1], 4);
    }
  } else if (num_bits_cb == ACELP_NUM_BITS_44) {
    for (track = 0; track < (4 - 2); track++) {
      k = track * NPMAXPT;
      acelp_param_out[track] =
          iusace_acelp_quant_3p_3n1bits(pulse_pos[k], pulse_pos[k + 1], pulse_pos[k + 2], 4);
    }
    for (track = 2; track < 4; track++) {
      k = track * NPMAXPT;
      acelp_param_out[track] = iusace_acelp_quant_2p_2n1bits(pulse_pos[k], pulse_pos[k + 1], 4);
    }
  } else if (num_bits_cb == ACELP_NUM_BITS_52) {
    for (track = 0; track < 4; track++) {
      k = track * NPMAXPT;
      acelp_param_out[track] =
          iusace_acelp_quant_3p_3n1bits(pulse_pos[k], pulse_pos[k + 1], pulse_pos[k + 2], 4);
    }
  } else if (num_bits_cb == ACELP_NUM_BITS_64) {
    for (track = 0; track < 4; track++) {
      k = track * NPMAXPT;
      l_index = iusace_acelp_quant_4p_4nbits(&pulse_pos[k], 4);
      acelp_param_out[track] = ((l_index >> 14) & 3);
      acelp_param_out[track + 4] = (l_index & 0x3FFF);
    }
  } else if (num_bits_cb == ACELP_NUM_BITS_72) {
    for (track = 0; track < (4 - 2); track++) {
      k = track * NPMAXPT;
      l_index = iusace_acelp_quant_5p_5nbits(&pulse_pos[k], 4);
      acelp_param_out[track] = ((l_index >> 10) & 0x03FF);
      acelp_param_out[track + 4] = (l_index & 0x03FF);
    }
    for (track = 2; track < 4; track++) {
      k = track * NPMAXPT;
      l_index = iusace_acelp_quant_4p_4nbits(&pulse_pos[k], 4);
      acelp_param_out[track] = ((l_index >> 14) & 3);
      acelp_param_out[track + 4] = (l_index & 0x3FFF);
    }
  } else if (num_bits_cb == ACELP_NUM_BITS_88) {
    for (track = 0; track < 4; track++) {
      k = track * NPMAXPT;
      l_index = iusace_acelp_quant_6p_6n_2bits(&pulse_pos[k], 4);
      acelp_param_out[track] = ((l_index >> 11) & 0x07FF);
      acelp_param_out[track + 4] = (l_index & 0x07FF);
    }
  }
  return;
}

VOID iusace_acelp_ltpred_cb_exc(FLOAT32 *exc, WORD32 t0, WORD32 t0_frac, WORD32 len_subfrm) {
  WORD32 i, j;
  FLOAT32 s, *x0, *x1, *x2;
  const FLOAT32 *c1, *c2;

  x0 = &exc[-t0];
  t0_frac = -t0_frac;
  if (t0_frac < 0) {
    t0_frac += T_UP_SAMP;
    x0--;
  }
  for (j = 0; j < len_subfrm; j++) {
    x1 = x0++;
    x2 = x1 + 1;
    c1 = &iusace_res_interp_filter1_4[t0_frac];
    c2 = &iusace_res_interp_filter1_4[T_UP_SAMP - t0_frac];
    s = 0.0;
    for (i = 0; i < INTER_LP_FIL_ORDER; i++, c1 += T_UP_SAMP, c2 += T_UP_SAMP) {
      s += (*x1--) * (*c1) + (*x2++) * (*c2);
    }
    exc[j] = s;
  }
}

VOID iusace_acelp_quant_gain(FLOAT32 *code, FLOAT32 *pitch_gain, FLOAT32 *code_gain,
                             FLOAT32 *tgt_cb_corr_data, FLOAT32 mean_energy, WORD32 *qunt_idx) {
  WORD32 i, indice = 0, min_pitch_idx;
  FLOAT32 ener_code, pred_code_gain;
  FLOAT32 dist, dist_min, g_pitch, g_code;
  const FLOAT32 *p1_qua_gain_table, *p2_qua_gain_table;

  p1_qua_gain_table = iusace_acelp_quant_gain_table;
  p2_qua_gain_table = (const FLOAT32 *)(iusace_acelp_quant_gain_table + ACELP_GAIN_TBL_OFFSET);
  min_pitch_idx = 0;
  g_pitch = *pitch_gain;
  for (i = 0; i < ACELP_RANGE_GAIN_PT_IDX_SEARCH; i++, p2_qua_gain_table += 2) {
    if (g_pitch > *p2_qua_gain_table) {
      continue;
    }
  }
  ener_code = 0.01F;

  for (i = 0; i < LEN_SUBFR; i++) {
    ener_code += code[i] * code[i];
  }

  ener_code = (FLOAT32)(10.0 * log10(ener_code / (FLOAT32)LEN_SUBFR));
  pred_code_gain = mean_energy - ener_code;
  pred_code_gain = (FLOAT32)pow(10.0, pred_code_gain / 20.0);

  dist_min = MAX_FLT_VAL;
  p2_qua_gain_table = (const FLOAT32 *)(p1_qua_gain_table + min_pitch_idx * 2);
  for (i = 0; i < ACELP_SEARCH_RANGE_QUANTIZER_IDX; i++) {
    g_pitch = *p2_qua_gain_table++;
    g_code = pred_code_gain * *p2_qua_gain_table++;
    dist = g_pitch * g_pitch * tgt_cb_corr_data[0] + g_pitch * tgt_cb_corr_data[1] +
           g_code * g_code * tgt_cb_corr_data[2] + g_code * tgt_cb_corr_data[3] +
           g_pitch * g_code * tgt_cb_corr_data[4];
    if (dist < dist_min) {
      dist_min = dist;
      indice = i;
    }
  }
  indice += min_pitch_idx;
  *pitch_gain = p1_qua_gain_table[indice * 2];
  *code_gain = p1_qua_gain_table[indice * 2 + 1] * pred_code_gain;
  *qunt_idx = indice;
}
