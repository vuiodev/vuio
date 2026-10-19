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

#include <math.h>
#include <stdlib.h>
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

#include "ixheaac_sbr_const.h"
#include "ixheaacd_env_extr.h"
#include "ixheaacd_freq_sca.h"
#include "ixheaacd_intrinsics.h"

const WORD32 ixheaacd_samp_rate_table[12] = {92017, 75132, 55426, 46009,
                                             37566, 27713, 23004, 18783,
                                             13856, 11502, 9391,  16428320};

const WORD32 ixheaacd_v_offset_40[16] = {
    3 + 1, 2 + 1, 2 + 1, 2 + 1, 2 + 1, 2 + 1, 2 + 1, 2 + 1,
    2 + 1, 2 + 1, 2 + 1, 2 + 1, 2 + 1, 2 + 1, 1 + 1, 0};

static WORD32 ixheaacd_int_div(WORD32 num, WORD32 den) {
  if (den != 0) {
    WORD32 result = 0;
    WORD32 temp = 0;
    while (den <= num) {
      temp = 0;
      while (num >= (den << (temp + 1))) {
        temp++;
      }
      result = result + (1 << temp);
      num = num - (den * (1 << temp));
    }
    return result;
  } else {
    return 0;
  }
}

VOID ixheaacd_aac_shellsort(WORD16 *in, WORD32 n) {
  WORD32 i, j;
  WORD32 inc;
  WORD32 v, w;

  inc = 1;

  do {
    inc = (((inc << 1) + inc) + 1);
  } while (inc <= n);

  do {
    inc = (ixheaacd_int_div(inc, 3));
    for (i = inc; i < n; i++) {
      v = in[i];
      j = i;

      while ((w = in[(j - inc)]) > v) {
        in[j] = w;
        j = (j - inc);

        if (j < inc) break;
      }
      in[j] = v;
    }

  } while (inc > 1);
}

WORD32
ixheaacd_calc_start_band(WORD32 fs_mapped, const WORD32 start_freq,
                         FLOAT32 upsamp_fac) {
  WORD32 k0_min;

  if (upsamp_fac == 4) {
    if (fs_mapped < 32000) {
      k0_min = (WORD32)(((FLOAT32)(3000 * 2 * 32) / fs_mapped) + 0.5);
    } else {
      if (fs_mapped < 64000) {
        k0_min = (WORD32)(((FLOAT32)(4000 * 2 * 32) / fs_mapped) + 0.5);
      } else {
        k0_min = (WORD32)(((FLOAT32)(5000 * 2 * 32) / fs_mapped) + 0.5);
      }
    }
  } else {
    if (fs_mapped < 32000) {
      k0_min = (WORD32)(((FLOAT32)(3000 * 2 * 64) / fs_mapped) + 0.5);
    } else {
      if (fs_mapped < 64000) {
        k0_min = (WORD32)(((FLOAT32)(4000 * 2 * 64) / fs_mapped) + 0.5);
      } else {
        k0_min = (WORD32)(((FLOAT32)(5000 * 2 * 64) / fs_mapped) + 0.5);
      }
    }
  }

  switch (fs_mapped) {
    case 16000: {
      WORD32 v_offset[] = {-8, -7, -6, -5, -4, -3, -2, -1,
                           0,  1,  2,  3,  4,  5,  6,  7};
      return (k0_min + v_offset[start_freq]);
    } break;
    case 22050: {
      WORD32 v_offset[] = {-5, -4, -3, -2, -1, 0, 1,  2,
                           3,  4,  5,  6,  7,  9, 11, 13};
      return (k0_min + v_offset[start_freq]);
    } break;
    case 24000: {
      WORD32 v_offset[] = {-5, -3, -2, -1, 0, 1,  2,  3,
                           4,  5,  6,  7,  9, 11, 13, 16};
      return (k0_min + v_offset[start_freq]);
    } break;
    case 32000: {
      WORD32 v_offset[] = {-6, -4, -2, -1, 0, 1,  2,  3,
                           4,  5,  6,  7,  9, 11, 13, 16};
      return (k0_min + v_offset[start_freq]);
    } break;
    case 40000: {
      WORD32 v_offset[] = {-1, 0, 1, 2,  3,  4,  5,  6,
                           7,  8, 9, 11, 13, 15, 17, 19};
      return (k0_min + v_offset[start_freq]);
    } break;
    case 44100:
    case 48000:
    case 64000: {
      WORD32 v_offset[] = {-4, -2, -1, 0, 1,  2,  3,  4,
                           5,  6,  7,  9, 11, 13, 16, 20};
      return (k0_min + v_offset[start_freq]);
    } break;
    case 88200:
    case 96000: {
      WORD32 v_offset[] = {-2, -1, 0, 1,  2,  3,  4,  5,
                           6,  7,  9, 11, 13, 16, 20, 24};
      return (k0_min + v_offset[start_freq]);
    } break;

    default: {
      WORD32 v_offset[] = {0, 1,  2,  3,  4,  5,  6,  7,
                           9, 11, 13, 16, 20, 24, 28, 33};
      return (k0_min + v_offset[start_freq]);
    }
  }
}

WORD32
ixheaacd_calc_stop_band(WORD32 fs, const WORD32 stop_freq, FLOAT32 upsamp_fac) {
  WORD32 result, i;
  WORD16 arr_stop_freq[14];
  WORD32 k1_min;
  WORD16 arr_diff_stop_freq[13];

  if (upsamp_fac == 4) {
    fs = fs / 2;
    if (fs < 32000) {
      k1_min = (WORD32)(((FLOAT32)(6000 * 2 * 32) / fs) + 0.5);
    } else {
      if (fs < 64000) {
        k1_min = (WORD32)(((FLOAT32)(8000 * 2 * 32) / fs) + 0.5);
      } else {
        k1_min = (WORD32)(((FLOAT32)(10000 * 2 * 32) / fs) + 0.5);
      }
    }
  } else {
    if (fs < 32000) {
      k1_min = (WORD32)(((FLOAT32)(6000 * 2 * 64) / fs) + 0.5);
    } else {
      if (fs < 64000) {
        k1_min = (WORD32)(((FLOAT32)(8000 * 2 * 64) / fs) + 0.5);
      } else {
        k1_min = (WORD32)(((FLOAT32)(10000 * 2 * 64) / fs) + 0.5);
      }
    }
  }

  /*Calculate stop frequency vector*/
  for (i = 0; i <= 13; i++) {
    arr_stop_freq[i] = (WORD32)(k1_min * pow(64.0 / k1_min, i / 13.0) + 0.5);
  }

  /*Ensure increasing bandwidth */
  for (i = 0; i <= 12; i++) {
    arr_diff_stop_freq[i] = arr_stop_freq[i + 1] - arr_stop_freq[i];
  }

  ixheaacd_aac_shellsort(&arr_diff_stop_freq[0],
                         13); /*Sort bandwidth changes */

  result = k1_min;
  for (i = 0; i < stop_freq; i++) {
    result = ixheaac_add32_sat(result, arr_diff_stop_freq[i]);
  }

  return (result);
}
IA_ERRORCODE ixheaacd_calc_k0_k2_bands(const WORD32 samp_freq,
                                       const WORD32 start_freq,
                                       const WORD32 stop_freq,
                                       FLOAT32 upsamp_fac, WORD16 *ptr_k0,
                                       WORD16 *ptr_k2) {
  IA_ERRORCODE err_code = IA_NO_ERROR;

  WORD32 fs_mapped = 0;
  WORD32 fs = samp_freq;

  if (upsamp_fac == 4) {
    fs = fs / 2;
  }

  if (fs >= 0 && fs < 18783) {
    fs_mapped = 16000;
  } else if (fs >= 18783 && fs < 23004) {
    fs_mapped = 22050;
  } else if (fs >= 23004 && fs < 27713) {
    fs_mapped = 24000;
  } else if (fs >= 27713 && fs < 35777) {
    fs_mapped = 32000;
  } else if (fs >= 35777 && fs < 42000) {
    fs_mapped = 40000;
  } else if (fs >= 42000 && fs < 46009) {
    fs_mapped = 44100;
  } else if (fs >= 46009 && fs < 55426) {
    fs_mapped = 48000;
  } else if (fs >= 55426 && fs < 75132) {
    fs_mapped = 64000;
  } else if (fs >= 75132 && fs < 92017) {
    fs_mapped = 88200;
  } else if (fs >= 92017) {
    fs_mapped = 96000;
  } else {
    return -1;
  }

  /* Update start_freq struct */
  *ptr_k0 = ixheaacd_calc_start_band(fs_mapped, start_freq, upsamp_fac);

  /*Update stop_freq struct */
  if (stop_freq < 14) {
    *ptr_k2 = ixheaacd_calc_stop_band(samp_freq, stop_freq, upsamp_fac);
  } else if (stop_freq == 14) {
    *ptr_k2 = 2 * (*ptr_k0);
  } else {
    *ptr_k2 = 3 * (*ptr_k0);
  }

  /* limit to Nyqvist */
  if (*ptr_k2 > 64) {
    *ptr_k2 = 64;
  }
  return err_code;
}

IA_ERRORCODE ixheaacd_calc_master_frq_bnd_tbl(
    ia_freq_band_data_struct *pstr_freq_band_data,
    ia_sbr_header_data_struct *ptr_header_data,
    ixheaacd_misc_tables *pstr_common_tables) {
  WORD32 k;
  WORD32 fs = ptr_header_data->out_sampling_freq;
  WORD16 bands;
  WORD16 k0 = 0, k2 = 0, k1;
  WORD32 k2_achived;
  WORD32 k2_diff;
  WORD32 incr;
  WORD32 dk;
  WORD16 vec_dk[MAX_OCTAVE + MAX_SECOND_REGION];
  WORD16 *vec_dk0 = &vec_dk[0];
  WORD16 *vec_dk1 = &vec_dk[MAX_OCTAVE];
  WORD16 upsamp_fac = ptr_header_data->upsamp_fac;
  WORD16 *f_master_tbl = pstr_freq_band_data->f_master_tbl;
  WORD16 num_mf_bands;
  IA_ERRORCODE err_code = IA_NO_ERROR;

  k1 = 0;
  incr = 0;
  dk = 0;

  err_code = ixheaacd_calc_k0_k2_bands(fs, ptr_header_data->start_freq,
                                       ptr_header_data->stop_freq, upsamp_fac,
                                       &k0, &k2);
  if (err_code) return err_code;

  if (k2 > NO_SYNTHESIS_CHANNELS) {
    k2 = NO_SYNTHESIS_CHANNELS;
  }
  if (upsamp_fac == 4) {
    if ((sub_d(k2, k0) > MAX_FREQ_COEFFS) || (k2 <= k0)) {
      return -1;
    }
    if ((2 * fs == 44100) && (sub_d(k2, k0) > MAX_FREQ_COEFFS)) {
      return -1;
    }
    if ((2 * fs >= 48000) && (sub_d(k2, k0) > MAX_FREQ_COEFFS)) {
      return -1;
    }
  } else {
    if ((sub_d(k2, k0) > MAX_FREQ_COEFFS_SBR) || (k2 <= k0)) {
      return -1;
    }
    if ((fs == 44100) && (sub_d(k2, k0) > MAX_FREQ_COEFFS_FS44100)) {
      return -1;
    }
    if ((fs >= 48000) && (sub_d(k2, k0) > MAX_FREQ_COEFFS_FS48000)) {
      return -1;
    }
  }

  if (ptr_header_data->freq_scale == 0) {
    WORD16 num_bands;
    if (ptr_header_data->alter_scale == 0) {
      dk = 1;
      num_bands = (WORD16)(k2 - k0);
      num_bands = num_bands - (num_bands & 0x1);
    } else {
      dk = 2;
      num_bands = (WORD16)((k2 - k0) + 2) >> 2;
      num_bands = num_bands << 1;
    }
    if (num_bands < 1) {
      return -1;
    }
    k2_achived = k0 + (num_bands << (dk - 1));

    k2_diff = k2 - k2_achived;

    for (k = 0; k < num_bands; k++) {
      vec_dk[k] = dk;
    }

    if (k2_diff < 0) {
      incr = 1;
      k = 0;
    }
    if (k2_diff > 0) {
      incr = -1;
      k = sub_d(num_bands, 1);
    }
    while (k2_diff != 0) {
      vec_dk[k] = vec_dk[k] - incr;
      k = (WORD16)(k + incr);
      k2_diff = k2_diff + incr;
    }
    f_master_tbl[0] = k0;
    for (k = 1; k <= num_bands; k++)
      f_master_tbl[k] = f_master_tbl[k - 1] + vec_dk[k - 1];
    num_mf_bands = num_bands;
  } else {
    WORD32 num_bands0;
    WORD32 num_bands1;

    switch (ptr_header_data->freq_scale) {
      case 1:
        bands = 12;
        break;
      case 2:
        bands = 10;
        break;
      case 3:
        bands = 8;
        break;
      default:
        bands = 8;
    };

    if ((upsamp_fac == 4) && (k0 < bands)) {
      bands = ((WORD32)(k0 - (k0 & 1)));
    }

    if ((WORD32)(10000 * k2) > (WORD32)(22449 * k0)) {
      k1 = k0 << 1;

      num_bands0 = bands;

      num_bands1 = pstr_common_tables->log_dual_is_table[k2] -
                   pstr_common_tables->log_dual_is_table[k1];
      num_bands1 = bands * num_bands1;

      if (ptr_header_data->alter_scale) {
        num_bands1 = (WORD32)(((WORD64)num_bands1 * (0x6276)) >> 15);
      }
      num_bands1 = num_bands1 + 0x1000;

      num_bands1 = num_bands1 >> 13;
      num_bands1 = num_bands1 << 1;

      if (num_bands0 < 1) {
        return -1;
      }

      if (num_bands1 < 1) {
        return -1;
      }

      ixheaacd_calc_bands(vec_dk0, k0, k1, (WORD16)num_bands0);

      ixheaacd_aac_shellsort(vec_dk0, num_bands0);

      f_master_tbl[0] = k0;

      for (k = 1; k <= num_bands0; k++)
        f_master_tbl[k] = f_master_tbl[k - 1] + vec_dk0[k - 1];

      ixheaacd_calc_bands(vec_dk1, k1, k2, (WORD16)num_bands1);
      ixheaacd_aac_shellsort(vec_dk1, num_bands1);

      if (vec_dk1[0] < vec_dk0[num_bands0 - 1]) {
        WORD16 change = vec_dk0[num_bands0 - 1] - vec_dk1[0];
        WORD16 temp = vec_dk1[num_bands1 - 1] - vec_dk1[0];
        temp = temp >> 1;
        if (change > temp) {
          change = temp;
        }
        vec_dk1[0] = vec_dk1[0] + change;
        vec_dk1[num_bands1 - 1] = vec_dk1[num_bands1 - 1] - change;
        ixheaacd_aac_shellsort(vec_dk1, num_bands1);
      }

      f_master_tbl[num_bands0] = k1;
      for (k = 1; k <= num_bands1; k++)
        f_master_tbl[num_bands0 + k] =
            f_master_tbl[num_bands0 + k - 1] + vec_dk1[k - 1];
      num_mf_bands = add_d(num_bands0, num_bands1);
    } else {
      k1 = k2;

      num_bands0 = pstr_common_tables->log_dual_is_table[k1] -
                   pstr_common_tables->log_dual_is_table[k0];

      num_bands0 = bands * num_bands0;

      num_bands0 = num_bands0 + 0x1000;

      num_bands0 = num_bands0 >> 13;
      num_bands0 = num_bands0 << 1;

      if (num_bands0 < 1) {
        return -1;
      }
      ixheaacd_calc_bands(vec_dk0, k0, k1, (WORD16)num_bands0);
      ixheaacd_aac_shellsort(vec_dk0, num_bands0);

      if (vec_dk0[0] == 0) {
        return -1;
      }

      f_master_tbl[0] = k0;
      for (k = 1; k <= num_bands0; k++)
        f_master_tbl[k] = f_master_tbl[k - 1] + vec_dk0[k - 1];

      num_mf_bands = num_bands0;
    }
  }
  if (num_mf_bands < 1) {
    return -1;
  }
  pstr_freq_band_data->num_mf_bands = num_mf_bands;

  if (upsamp_fac == 4) {
    for (k = 1; k < num_mf_bands; k++) {
      if (!(f_master_tbl[k] - f_master_tbl[k - 1] <= k0 - 2)) {
        return -1;
      }
    }
  }

  return 0;
}

static WORD16 ixheaacd_calc_freq_ratio(WORD16 k_start, WORD16 k_stop,
                                       WORD16 num_bands) {
  WORD32 bandfactor;
  WORD32 step;
  WORD32 direction;
  WORD32 start;
  WORD32 stop;
  WORD32 temp;
  WORD32 j, i;

  bandfactor = 0x3f000000L;
  step = 0x20000000L;
  direction = 1;
  start = ixheaac_shl32(ixheaac_deposit16l_in32(k_start), INT_BITS - 8);
  stop = ixheaac_shl32(ixheaac_deposit16l_in32(k_stop), INT_BITS - 8);

  i = 0;

  do {
    i = i + 1;
    temp = stop;

    for (j = 0; j < num_bands; j++)
      temp = ixheaac_mult16x16in32_shl(ixheaac_extract16h(temp),
                                        ixheaac_extract16h(bandfactor));

    if (temp < start) {
      if (direction == 0) step = ixheaac_shr32(step, 1);
      direction = 1;
      bandfactor = ixheaac_add32_sat(bandfactor, step);
    } else {
      if (direction == 1) step = ixheaac_shr32(step, 1);
      direction = 0;
      bandfactor = ixheaac_sub32_sat(bandfactor, step);
    }

    if (i > 100) {
      step = 0;
    }
  } while (step > 0);

  return ixheaac_extract16h(bandfactor);
}

VOID ixheaacd_calc_bands(WORD16 *diff, WORD16 start, WORD16 stop,
                         WORD16 num_bands) {
  WORD32 i;
  WORD32 previous;
  WORD32 current;
  WORD32 temp, exact;
  WORD16 bandfactor = ixheaacd_calc_freq_ratio(start, stop, num_bands);

  previous = stop;
  exact = ixheaac_shl32_sat(ixheaac_deposit16l_in32(stop), INT_BITS - 8);

  for (i = num_bands - 1; i >= 0; i--) {
    exact = ixheaac_mult16x16in32(ixheaac_extract16h(exact), bandfactor);

    temp = ixheaac_add32_sat(exact, 0x00400000);
    exact = exact << 1;

    current = ixheaac_extract16l(ixheaac_shr32(temp, (INT_BITS - 9)));

    diff[i] = sub_d(previous, current);
    previous = current;
  }
}

static VOID ixheaacd_derive_hi_lo_freq_bnd_tbls(
    ia_freq_band_data_struct *pstr_freq_band_data,
    ia_sbr_header_data_struct *ptr_header_data) {
  WORD16 k;
  WORD16 xover_band = ptr_header_data->xover_band;
  WORD16 *f_master_tbl = pstr_freq_band_data->f_master_tbl + xover_band;
  WORD16 *f_low_tbl = pstr_freq_band_data->freq_band_table[LOW];
  WORD16 *f_high_tbl = pstr_freq_band_data->freq_band_table[HIGH];
  WORD16 num_mf_bands = pstr_freq_band_data->num_mf_bands;
  WORD16 num_lf_bands, num_hf_bands;
  num_hf_bands = num_mf_bands - xover_band;
  k = 0;
  *f_low_tbl = *f_high_tbl = *f_master_tbl;
  f_low_tbl++;
  f_high_tbl++;
  f_master_tbl++;
  k++;
  if ((num_hf_bands & 1)) {
    *f_low_tbl = *f_high_tbl = *f_master_tbl;
    f_high_tbl++;
    f_master_tbl++;
    f_low_tbl++;
    k++;
  }
  for (; k <= num_hf_bands; k++) {
    *f_high_tbl = *f_master_tbl;
    f_high_tbl++;
    f_master_tbl++;
    k++;

    *f_low_tbl = *f_high_tbl = *f_master_tbl;
    f_high_tbl++;
    f_master_tbl++;
    f_low_tbl++;
  }
  num_lf_bands = ((num_hf_bands + 1) >> 1);

  pstr_freq_band_data->num_sf_bands[LOW] = num_lf_bands;
  pstr_freq_band_data->num_sf_bands[HIGH] = num_hf_bands;
}

WORD32 ixheaacd_derive_noise_freq_bnd_tbl(
    ia_sbr_header_data_struct *ptr_header_data,
    ixheaacd_misc_tables *pstr_common_tables,
    ia_freq_band_data_struct *pstr_freq_band_data) {
  WORD16 k2, kx;
  WORD32 temp;
  WORD32 num_lf_bands = pstr_freq_band_data->num_sf_bands[LOW];
  WORD32 num_hf_bands = pstr_freq_band_data->num_sf_bands[HIGH];
  k2 = pstr_freq_band_data->freq_band_table[HIGH][num_hf_bands];
  kx = pstr_freq_band_data->freq_band_table[HIGH][0];

  if (ptr_header_data->noise_bands == 0) {
    temp = 1;
  } else {
    temp = pstr_common_tables->log_dual_is_table[k2] -
           pstr_common_tables->log_dual_is_table[kx];
    temp = temp * ptr_header_data->noise_bands;
    temp = temp + 0x800;
    temp = temp >> 12;
    if (temp == 0) {
      temp = 1;
    }
  }
  if (temp > MAX_NOISE_COEFFS) {
    return -1;
  }
  pstr_freq_band_data->num_nf_bands = temp;
  pstr_freq_band_data->num_if_bands = pstr_freq_band_data->num_nf_bands;
  {
    WORD16 i_k, k;
    WORD16 num, den;
    WORD16 *f_noise_tbl = pstr_freq_band_data->freq_band_tbl_noise;
    WORD16 *f_low_tbl = pstr_freq_band_data->freq_band_table[LOW];
    WORD32 num_nf_bands = pstr_freq_band_data->num_nf_bands;

    num = num_lf_bands;
    den = num_nf_bands;

    k = 0;
    *f_noise_tbl = f_low_tbl[0];
    f_noise_tbl++;
    k++;
    i_k = 0;

    for (; k <= num_nf_bands; k++) {
      i_k = i_k + (WORD16)ixheaacd_int_div(num, den);
      *f_noise_tbl = f_low_tbl[i_k];
      num = num_lf_bands - i_k;
      den = den - 1;
      f_noise_tbl++;
    }
  }
  return 0;
}

WORD32 ixheaacd_calc_frq_bnd_tbls(ia_sbr_header_data_struct *ptr_header_data,
                                  ixheaacd_misc_tables *pstr_common_tables) {
  WORD32 err;
  WORD16 num_lf_bands, lsb, usb;
  ia_freq_band_data_struct *pstr_freq_band_data =
      ptr_header_data->pstr_freq_band_data;

  err = ixheaacd_calc_master_frq_bnd_tbl(pstr_freq_band_data, ptr_header_data,
                                         pstr_common_tables);

  if (err ||
      (ptr_header_data->xover_band > pstr_freq_band_data->num_mf_bands)) {
    return -1;
  }

  ixheaacd_derive_hi_lo_freq_bnd_tbls(pstr_freq_band_data, ptr_header_data);

  num_lf_bands = pstr_freq_band_data->num_sf_bands[LOW];

  if ((num_lf_bands <= 0) ||
      (num_lf_bands > ixheaac_shr16(MAX_FREQ_COEFFS, 1))) {
    return -1;
  }

  lsb = pstr_freq_band_data->freq_band_table[LOW][0];
  usb = pstr_freq_band_data->freq_band_table[LOW][num_lf_bands];

  pstr_freq_band_data->sub_band_start = lsb;

  ptr_header_data->status = 1;

  if ((lsb > ((ptr_header_data->sbr_ratio_idx == SBR_UPSAMPLE_IDX_4_1) ? 16 : 32)) ||
      (lsb >= usb)) {
    return -1;
  }

  if (ixheaacd_derive_noise_freq_bnd_tbl(ptr_header_data, pstr_common_tables,
                                         pstr_freq_band_data)) {
    return -1;
  }

  pstr_freq_band_data->sub_band_start = lsb;
  pstr_freq_band_data->sub_band_end = usb;

  return 0;
}
