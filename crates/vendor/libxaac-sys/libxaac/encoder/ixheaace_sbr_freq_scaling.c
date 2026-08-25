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

#include <math.h>
#include "ixheaac_type_def.h"
#include "ixheaac_constants.h"
#include "ixheaace_error_codes.h"
#include "ixheaac_error_standards.h"
#include "ixheaace_aac_constants.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops16.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaac_basic_ops.h"

#include "ixheaace_common_rom.h"
#include "ixheaace_sbr_header.h"
#include "ixheaace_sbr_def.h"
#include "ixheaace_sbr_freq_scaling.h"
#include "ixheaace_sbr_misc.h"
#include "ixheaace_resampler.h"
#include "ixheaace_sbr_rom.h"

static WORD32 ixheaace_get_start_freq_4_1(WORD32 fs, WORD32 start_freq) {
  WORD32 minimum_k0;
  const WORD32 *ptr_start_offset;

  switch (fs) {
    case 16000:
      minimum_k0 = 12;
      break;
    case 22050:
      minimum_k0 = 9;
      break;
    case 24000:
      minimum_k0 = 8;
      break;
    case 32000:
      minimum_k0 = 8;
      break;
    case 44100:
      minimum_k0 = 6;
      break;
    case 48000:
      minimum_k0 = 5;
      break;
    default:
      minimum_k0 = 5; /* illegal fs */
  }

  switch (fs) {
    case 16000: {
      ptr_start_offset = &ixheaace_start_freq_16k_4_1[0];
    } break;

    case 22050: {
      ptr_start_offset = &ixheaace_start_freq_22k_4_1[0];
    } break;

    case 24000: {
      ptr_start_offset = &ixheaace_start_freq_24k_4_1[0];
    } break;

    case 32000: {
      ptr_start_offset = &ixheaace_start_freq_32k_4_1[0];
    } break;

    case 44100:
    case 48000:
    case 64000: {
      ptr_start_offset = &ixheaace_start_freq_48k_4_1[0];
    } break;

    case 88200:
    case 96000: {
      ptr_start_offset = &ixheaace_start_freq_96k_4_1[0];
    } break;

    default: {
      ptr_start_offset = &ixheaace_start_freq_dflt_4_1[0];
    }
  }
  return (minimum_k0 + ptr_start_offset[start_freq]);
}

static WORD32 ixheaace_get_stop_freq_4_1(WORD32 fs, WORD32 stop_freq) {
  WORD32 result, i;
  WORD32 *v_stop_freq = 0;
  WORD32 k1_min;
  WORD32 v_dstop[13];

  /* counting previous operations */
  switch (fs) {
    case 16000:
      k1_min = 24;
      v_stop_freq = (WORD32 *)&ixheaace_stop_freq_16k_4_1[0];
      break;
    case 22050:
      k1_min = 17;
      v_stop_freq = (WORD32 *)&ixheaace_stop_freq_22k_4_1[0];
      break;
    case 24000:
      k1_min = 16;
      v_stop_freq = (WORD32 *)&ixheaace_stop_freq_24k_4_1[0];
      break;
    case 32000:
      k1_min = 16;
      v_stop_freq = (WORD32 *)&ixheaace_stop_freq_32k_4_1[0];
      break;

    case 44100:
      k1_min = 12;
      v_stop_freq = (WORD32 *)&ixheaace_stop_freq_44k_4_1[0];
      break;

    case 48000:
      k1_min = 11;
      v_stop_freq = (WORD32 *)&ixheaace_stop_freq_48k_4_1[0];
      break;

    default:
      v_stop_freq = (WORD32 *)&ixheaace_stop_freq_32k_4_1[0];
      k1_min = 11; /* illegal fs  */
  }

  for (i = 0; i <= 12; i++) {
    v_dstop[i] = v_stop_freq[i + 1] - v_stop_freq[i];
  }

  ixheaace_shellsort_int(v_dstop, 13);

  result = k1_min;

  for (i = 0; i < stop_freq; i++) {
    result = result + v_dstop[i];
  }

  return result;
}

static WORD32 ixheaace_get_start_freq(WORD32 fs, WORD32 start_freq) {
  WORD32 minimum_k0;

  switch (fs) {
    case 16000:
      minimum_k0 = 24;
      break;
    case 22050:
      minimum_k0 = 17;
      break;
    case 24000:
      minimum_k0 = 16;
      break;
    case 32000:
      minimum_k0 = 16;
      break;
    case 44100:
      minimum_k0 = 12;
      break;
    case 48000:
      minimum_k0 = 11;
      break;
    case 64000:
      minimum_k0 = 10;
      break;
    case 88200:
      minimum_k0 = 7;
      break;
    case 96000:
      minimum_k0 = 7;
      break;
    default:
      minimum_k0 = 11; /* illegal fs */
  }

  switch (fs) {
    case 16000: {
      return (minimum_k0 + vector_offset_16k[start_freq]);
    } break;

    case 22050: {
      return (minimum_k0 + vector_offset_22k[start_freq]);
    } break;

    case 24000: {
      return (minimum_k0 + vector_offset_24k[start_freq]);
    } break;

    case 32000: {
      return (minimum_k0 + vector_offset_32k[start_freq]);
    } break;

    case 44100:
    case 48000:
    case 64000: {
      return (minimum_k0 + vector_offset_44_48_64[start_freq]);
    } break;

    case 88200:
    case 96000: {
      return (minimum_k0 + vector_offset_88_96[start_freq]);
    } break;

    default: {
      return (minimum_k0 + vector_offset_def[start_freq]);
    }
  }
}

static WORD32 ixheaace_get_stop_freq(WORD32 fs, WORD32 stop_freq) {
  WORD32 result, i;
  WORD32 *v_stop_freq = 0;
  WORD32 k1_min;
  WORD32 v_dstop[13];

  switch (fs) {
    case 16000:
      k1_min = ixheaace_stop_freq_16k[0];
      v_stop_freq = (WORD32 *)&ixheaace_stop_freq_16k[0];
      break;
    case 22050:
      k1_min = ixheaace_stop_freq_22k[0];
      v_stop_freq = (WORD32 *)&ixheaace_stop_freq_22k[0];
      break;
    case 24000:
      k1_min = ixheaace_stop_freq_24k[0];
      v_stop_freq = (WORD32 *)&ixheaace_stop_freq_24k[0];
      break;
    case 32000:
      k1_min = 32;

      v_stop_freq = (WORD32 *)vector_stop_freq_32;
      break;

    case 44100:
      k1_min = 23;

      v_stop_freq = (WORD32 *)vector_stop_freq_44;
      break;

    case 48000:
      k1_min = 21;

      v_stop_freq = (WORD32 *)vector_stop_freq_48;
      break;

    default:
      v_stop_freq = (WORD32 *)vector_stop_freq_32;
      k1_min = 21; /* illegal fs  */
  }

  for (i = 0; i <= 12; i++) {
    v_dstop[i] = v_stop_freq[i + 1] - v_stop_freq[i];
  }

  ixheaace_shellsort_int(v_dstop, 13);

  result = k1_min;

  for (i = 0; i < stop_freq; i++) {
    result = result + v_dstop[i];
  }

  return result;
}

static WORD32 ixheaace_get_usac_stop_freq(WORD32 fs, WORD32 stop_freq)
{
  WORD32 result, i;
  WORD32 *v_stop_freq = 0;
  WORD32 k1_min;
  WORD32 v_dstop[13];

  switch (fs)
  {
  case 16000:
    k1_min = ixheaace_usac_stop_freq_16k[0];
    v_stop_freq = (WORD32 *)&ixheaace_usac_stop_freq_16k[0];
    break;
  case 22050:
    k1_min = ixheaace_usac_stop_freq_22k[0];
    v_stop_freq = (WORD32 *)&ixheaace_usac_stop_freq_22k[0];
    break;
  case 24000:
    k1_min = ixheaace_usac_stop_freq_24k[0];
    v_stop_freq = (WORD32 *)&ixheaace_usac_stop_freq_24k[0];
    break;
  case 32000:
    k1_min = 32;
    v_stop_freq = (WORD32 *)vector_stop_freq_32;
    break;

  case 44100:
    k1_min = 23;
    v_stop_freq = (WORD32 *)vector_stop_freq_44;
    break;

  case 48000:
    k1_min = 21;
    v_stop_freq = (WORD32 *)vector_stop_freq_48;
    break;

  default:
    v_stop_freq = (WORD32 *)vector_stop_freq_32;
    k1_min = 21; /* illegal fs  */
  }

  for (i = 0; i <= 12; i++)
  {
    v_dstop[i] = v_stop_freq[i + 1] - v_stop_freq[i];
  }

  ixheaace_shellsort_int(v_dstop, 13);

  result = k1_min;

  for (i = 0; i < stop_freq; i++)
  {
    result = result + v_dstop[i];
  }

  return result;
}

WORD32
ixheaace_get_sbr_start_freq_raw(WORD32 start_freq, WORD32 qmf_bands, WORD32 fs) {
  WORD32 result;

  if (start_freq < 0 || start_freq > 15) {
    return -1;
  }

  result = ixheaace_get_start_freq(fs, start_freq);

  result = (result * fs / qmf_bands + 1) >> 1;

  return result;
}

static WORD32 ixheaace_number_of_bands(WORD32 b_p_o, WORD32 start, WORD32 stop,
                                       FLOAT32 warp_fac) {
  WORD32 result = 0;
  result = (WORD32)(b_p_o * log((FLOAT32)(stop) / start) / (2.0 * log(2.0) * warp_fac) + 0.5);
  result <<= 1;
  return result;
}

static VOID ixheaace_calc_bands(WORD32 *ptr_diff, WORD32 start, WORD32 stop, WORD32 num_bands) {
  WORD32 i;
  WORD32 previous;
  WORD32 current;
  previous = start;
  for (i = 1; i <= num_bands; i++) {
    current = (WORD32)((start * pow((FLOAT32)stop / start, (FLOAT32)i / num_bands)) + 0.5f);
    ptr_diff[i - 1] = current - previous;
    previous = current;
  }
}

static VOID ixheaace_modify_bands(WORD32 max_band_previous, WORD32 *ptr_diff, WORD32 length) {
  WORD32 change = max_band_previous - ptr_diff[0];

  if (change > (ptr_diff[length - 1] - ptr_diff[0]) / 2) {
    change = (ptr_diff[length - 1] - ptr_diff[0]) / 2;
  }

  ptr_diff[0] += change;

  ptr_diff[length - 1] -= change;

  ixheaace_shellsort_int(ptr_diff, length);
}

static VOID ixheaace_cum_sum(WORD32 start_value, WORD32 *ptr_diff, WORD32 length,
                             UWORD8 *ptr_start_adress) {
  WORD32 i;

  ptr_start_adress[0] = (UWORD8)start_value;

  for (i = 1; i <= length; i++) {
    ptr_start_adress[i] = ptr_start_adress[i - 1] + (UWORD8)ptr_diff[i - 1];
  }
}

IA_ERRORCODE
ixheaace_find_start_and_stop_band(const WORD32 sampling_freq, const WORD32 num_channels,
                                  const WORD32 start_freq, const WORD32 stop_freq,
                                  const ixheaace_sr_mode sample_rate_mode, WORD32 *ptr_k0,
                                  WORD32 *ptr_k2, WORD32 sbr_ratio_idx,
                                  ixheaace_sbr_codec_type sbr_codec) {
  switch (sbr_codec) {
    case USAC_SBR: {
      if (sbr_ratio_idx == USAC_SBR_RATIO_INDEX_4_1) {
        *ptr_k0 = ixheaace_get_start_freq_4_1(sampling_freq, start_freq);
      } else {
        *ptr_k0 = ixheaace_get_start_freq(sampling_freq, start_freq);
      }
      break;
    }
    default: {
      *ptr_k0 = ixheaace_get_start_freq(sampling_freq, start_freq);
      break;
    }
  }
  if ((sample_rate_mode == 1) && (sampling_freq * num_channels < 2 * *ptr_k0 * sampling_freq)) {
    return IA_EXHEAACE_INIT_FATAL_SBR_INVALID_SAMPLERATE_MODE;
  }

  if (stop_freq < 14) {
    switch (sbr_codec) {
      case USAC_SBR: {
        if (USAC_SBR_RATIO_INDEX_4_1 == sbr_ratio_idx) {
          *ptr_k2 = ixheaace_get_stop_freq_4_1(sampling_freq, stop_freq);
        } else {
          *ptr_k2 = ixheaace_get_usac_stop_freq(sampling_freq, stop_freq);
        }
        break;
      }
      default: {
        *ptr_k2 = ixheaace_get_stop_freq(sampling_freq, stop_freq);
        break;
      }
    }
  }

  else {
    *ptr_k2 = (stop_freq == 14 ? 2 * *ptr_k0 : 3 * *ptr_k0);
  }

  if (*ptr_k2 > num_channels) {
    *ptr_k2 = num_channels;
  }
  if (sbr_codec == USAC_SBR) {
    if (sbr_ratio_idx == USAC_SBR_RATIO_INDEX_4_1) {
      if (((*ptr_k2 - *ptr_k0) > MAXIMUM_FREQ_COEFFS_USAC) || (*ptr_k2 <= *ptr_k0)) {
        return IA_EXHEAACE_INIT_FATAL_SBR_INVALID_FREQ_COEFFS;
      }
      if ((2 * sampling_freq == 44100) && ((*ptr_k2 - *ptr_k0) > MAXIMUM_FREQ_COEFFS_USAC)) {
        return IA_EXHEAACE_INIT_FATAL_SBR_INVALID_FREQ_COEFFS;
      }
      if ((2 * sampling_freq >= 48000) && ((*ptr_k2 - *ptr_k0) > MAXIMUM_FREQ_COEFFS_USAC)) {
        return IA_EXHEAACE_INIT_FATAL_SBR_INVALID_FREQ_COEFFS;
      }
    } else {
      if (sampling_freq <= 32000) {
        if ((*ptr_k2 - *ptr_k0) > MAXIMUM_FREQ_COEFFS_LE32KHZ) {
          return IA_EXHEAACE_INIT_FATAL_SBR_INVALID_FREQ_COEFFS;
        }
      } else if (sampling_freq == 44100) {
        if ((*ptr_k2 - *ptr_k0) > MAXIMUM_FREQ_COEFFS_EQ44KHZ) {
          return IA_EXHEAACE_INIT_FATAL_SBR_INVALID_FREQ_COEFFS;
        }
      } else if (sampling_freq >= 48000) {
        if ((*ptr_k2 - *ptr_k0) > MAXIMUM_FREQ_COEFFS_GE48KHZ) {
          return IA_EXHEAACE_INIT_FATAL_SBR_INVALID_FREQ_COEFFS;
        }
      } else {
        return IA_EXHEAACE_INIT_FATAL_SBR_INVALID_FREQ_COEFFS;
      }
    }
  } else {
    if (sampling_freq <= 32000) {
      if ((*ptr_k2 - *ptr_k0) > MAXIMUM_FREQ_COEFFS_LE32KHZ) {
        return IA_EXHEAACE_INIT_FATAL_SBR_INVALID_FREQ_COEFFS;
      }
    } else if (sampling_freq == 44100) {
      if ((*ptr_k2 - *ptr_k0) > MAXIMUM_FREQ_COEFFS_EQ44KHZ) {
        return IA_EXHEAACE_INIT_FATAL_SBR_INVALID_FREQ_COEFFS;
      }
    } else if (sampling_freq >= 48000) {
      if ((*ptr_k2 - *ptr_k0) > MAXIMUM_FREQ_COEFFS_GE48KHZ) {
        return IA_EXHEAACE_INIT_FATAL_SBR_INVALID_FREQ_COEFFS;
      }
    } else {
      return IA_EXHEAACE_INIT_FATAL_SBR_INVALID_FREQ_COEFFS;
    }
  }

  if ((*ptr_k2 - *ptr_k0) < 0) {
    return IA_EXHEAACE_INIT_FATAL_SBR_INVALID_FREQ_COEFFS;
  }

  return IA_NO_ERROR;
}

IA_ERRORCODE
ixheaace_update_freq_scale(UWORD8 *ptr_k_master, WORD32 *ptr_num_bands, const WORD32 k0,
                           const WORD32 k2, const WORD32 freq_scale, const WORD32 alter_scale,
                           ixheaace_sr_mode sbr_rate)

{
  IA_ERRORCODE err_code = IA_NO_ERROR;
  WORD32 b_p_o = 0;
  WORD32 dk = 0;

  FLOAT32 warp;
  WORD32 k1 = 0, i;
  WORD32 num_bands0;
  WORD32 num_bands1;
  WORD32 diff_tot[IXHEAACE_MAXIMUM_OCTAVE + IXHEAACE_MAXIMUM_SECOND_REGION] = {0};
  WORD32 *diff0 = diff_tot;
  WORD32 *diff1 = diff_tot + IXHEAACE_MAXIMUM_OCTAVE;
  WORD32 k2_achived;
  WORD32 k2_diff;
  WORD32 incr = 0;

  switch (freq_scale) {
    case 1:
      b_p_o = 12;
      break;
    case 2:
      b_p_o = 10;
      break;
    case 3:
      b_p_o = 8;
      break;
  }

  if (freq_scale > 0) {
    if (alter_scale == 0) {
      warp = 1.0f;
    } else {
      warp = 1.3f;
    }

    if (IXHEAACE_QUAD_RATE == sbr_rate) {
      if (k0 < b_p_o) {
        b_p_o = (k0 >> 1) * 2;
      }
    }

    if (4 * k2 >= 9 * k0) {
      k1 = 2 * k0;
      num_bands0 = ixheaace_number_of_bands(b_p_o, k0, k1, 1.0f);
      num_bands1 = ixheaace_number_of_bands(b_p_o, k1, k2, warp);

      ixheaace_calc_bands(diff0, k0, k1, num_bands0);
      ixheaace_shellsort_int(diff0, num_bands0);

      if (diff0[0] == 0) {
        return IA_EXHEAACE_INIT_FATAL_SBR_INVALID_NUM_BANDS;
      }

      ixheaace_cum_sum(k0, diff0, num_bands0, ptr_k_master);

      ixheaace_calc_bands(diff1, k1, k2, num_bands1);
      ixheaace_shellsort_int(diff1, num_bands1);

      if (diff0[num_bands0 - 1] > diff1[0]) {
        ixheaace_modify_bands(diff0[num_bands0 - 1], diff1, num_bands1);
      }

      ixheaace_cum_sum(k1, diff1, num_bands1, &ptr_k_master[num_bands0]);
      *ptr_num_bands = num_bands0 + num_bands1;
    } else {
      k1 = k2;
      num_bands0 = ixheaace_number_of_bands(b_p_o, k0, k1, 1.0f);

      ixheaace_calc_bands(diff0, k0, k1, num_bands0);
      ixheaace_shellsort_int(diff0, num_bands0);

      if (diff0[0] == 0) {
        return IA_EXHEAACE_INIT_FATAL_SBR_INVALID_NUM_BANDS;
      }

      ixheaace_cum_sum(k0, diff0, num_bands0, ptr_k_master);

      *ptr_num_bands = num_bands0;
    }
  } else {
    if (alter_scale == 0) {
      dk = 1;
      num_bands0 = 2 * ((k2 - k0) / 2);
    } else {
      dk = 2;
      num_bands0 = 2 * (((k2 - k0) / dk + 1) / 2);
    }

    k2_achived = k0 + num_bands0 * dk;
    k2_diff = k2 - k2_achived;

    for (i = 0; i < num_bands0; i++) {
      diff_tot[i] = dk;
    }

    if (k2_diff < 0) {
      incr = 1;
      i = 0;
    }

    if (k2_diff > 0) {
      incr = -1;

      i = num_bands0 - 1;
    }

    while (k2_diff != 0) {
      if (i < 0) break;
      diff_tot[i] = diff_tot[i] - incr;

      i = i + incr;

      k2_diff = k2_diff + incr;
    }

    ixheaace_cum_sum(k0, diff_tot, num_bands0, ptr_k_master);

    *ptr_num_bands = num_bands0;
  }

  if (*ptr_num_bands < 1) {
    return IA_EXHEAACE_INIT_FATAL_SBR_INVALID_NUM_BANDS;
  }

  if (sbr_rate == IXHEAACE_QUAD_RATE) {
    for (i = 1; i < *ptr_num_bands; i++) {
      if (!(ptr_k_master[i] - ptr_k_master[i - 1] <= k0 - 2)) {
        return IA_EXHEAACE_INIT_FATAL_SBR_INVALID_NUM_BANDS;
      }
    }
  }

  return err_code;
}

VOID ixheaace_update_high_res(UWORD8 *ptr_hires, WORD32 *ptr_num_hires, UWORD8 *ptr_k_master,
                              WORD32 num_master, WORD32 *ptr_xover_band,
                              ixheaace_sr_mode dr_or_sr, WORD32 num_qmf_ch) {
  WORD32 i;
  WORD32 divider;
  WORD32 max1, max2;

  divider = (dr_or_sr == IXHEAACE_DUAL_RATE) ? 2 : 1;
  if (dr_or_sr == IXHEAACE_QUAD_RATE) {
    divider = 4;
  }

  if ((ptr_k_master[*ptr_xover_band] > (num_qmf_ch / divider)) ||
      (*ptr_xover_band > num_master)) {
    max1 = 0;
    max2 = num_master;

    while ((ptr_k_master[max1 + 1] < (num_qmf_ch / divider)) && ((max1 + 1) < max2)) {
      max1++;
    }

    *ptr_xover_band = max1;
  }

  *ptr_num_hires = num_master - *ptr_xover_band;

  for (i = *ptr_xover_band; i <= num_master; i++) {
    ptr_hires[i - *ptr_xover_band] = ptr_k_master[i];
  }
}

VOID ixheaace_update_low_res(UWORD8 *ptr_lores, WORD32 *ptr_num_lores, UWORD8 *ptr_hires,
                             WORD32 ptr_num_hires) {
  WORD32 i;

  if (ptr_num_hires % 2 == 0) {
    *ptr_num_lores = ptr_num_hires / 2;

    for (i = 0; i <= *ptr_num_lores; i++) {
      ptr_lores[i] = ptr_hires[i * 2];
    }
  } else {
    *ptr_num_lores = (ptr_num_hires + 1) / 2;

    ptr_lores[0] = ptr_hires[0];

    for (i = 1; i <= *ptr_num_lores; i++) {
      ptr_lores[i] = ptr_hires[i * 2 - 1];
    }
  }
}
