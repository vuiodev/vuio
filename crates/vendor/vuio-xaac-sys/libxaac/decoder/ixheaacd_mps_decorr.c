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
#include <string.h>
#include "ixheaac_type_def.h"
#include "ixheaacd_bitbuffer.h"
#include "ixheaacd_common_rom.h"
#include "ixheaacd_sbrdecsettings.h"
#include "ixheaacd_sbr_scale.h"
#include "ixheaacd_env_extr_part.h"
#include "ixheaacd_sbr_rom.h"
#include "ixheaacd_hybrid.h"
#include "ixheaacd_ps_dec.h"
#include "ixheaacd_config.h"
#include "ixheaacd_qmf_dec.h"
#include "ixheaacd_audioobjtypes.h"
#include "ixheaacd_mps_polyphase.h"
#include "ixheaacd_mps_struct_def.h"
#include "ixheaacd_mps_res_rom.h"
#include "ixheaacd_mps_aac_struct.h"
#include "ixheaac_constants.h"
#include "ixheaacd_mps_dec.h"
#include "ixheaacd_mps_decor.h"
#include "ixheaacd_mps_hybfilter.h"
#include "ixheaac_error_standards.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaac_basic_ops40.h"
#include "ixheaacd_mps_macro_def.h"
#include "ixheaacd_mps_basic_op.h"

static const WORD32 ixheaacd_decorr_delay[] = {11, 10, 5, 2};
static const WORD32 ixheaacd_decorr_delay_ldmps[] = {8, 7, 2, 1};

static const WORD32 ixheaacd_qmf_split_freq_0[] = {3, 15, 24, 65};
static const WORD32 ixheaacd_qmf_split_freq_1[] = {3, 50, 65, 65};
static const WORD32 ixheaacd_qmf_split_freq_2[] = {0, 15, 65, 65};


static const WORD32 ixheaacd_qmf_split_freq_0_ldmps[] = {0, 15, 24, 65};
static const WORD32 ixheaacd_qmf_split_freq_1_ldmps[] = {0, 50, 65, 65};
static const WORD32 ixheaacd_qmf_split_freq_2_ldmps[] = {0, 15, 65, 65};

extern const WORD32 ixheaacd_mps_gain_set_indx[29];

static const FLOAT32
    ixheaacd_lattice_coeff_0_filt_den_coeff[DECORR_FILT_0_ORD + 1] = {
        1.000000f, -0.314818f, -0.256828f, -0.173641f, -0.115077f, 0.000599f,
        0.033343f, 0.122672f,  -0.356362f, 0.128058f,  0.089800f};
static const FLOAT32
    ixheaacd_lattice_coeff_0_filt_num_coeff[DECORR_FILT_0_ORD + 1] = {
        0.089800f,  0.128058f,  -0.356362f, 0.122672f,  0.033343f, 0.000599f,
        -0.115077f, -0.173641f, -0.256828f, -0.314818f, 1.000000f};

static const FLOAT32
    ixheaacd_lattice_coeff_1_filt_den_coeff[DECORR_FILT_1_ORD + 1] = {
        1.000000f, -0.287137f, -0.088940f, 0.123204f, -0.126111f,
        0.064218f, 0.045768f,  -0.016264f, -0.122100f};
static const FLOAT32
    ixheaacd_lattice_coeff_1_filt_num_coeff[DECORR_FILT_1_ORD + 1] = {
        -0.122100f, -0.016264f, 0.045768f,  0.064218f, -0.126111f,
        0.123204f,  -0.088940f, -0.287137f, 1.000000f};

static const FLOAT32
    ixheaacd_lattice_coeff_2_filt_den_coeff[DECORR_FILT_2_ORD + 1] = {
        1.000000f, 0.129403f, -0.032633f, 0.035700f};
static const FLOAT32
    ixheaacd_lattice_coeff_2_filt_num_coeff[DECORR_FILT_2_ORD + 1] = {
        0.035700f, -0.032633f, 0.129403f, 1.000000f};

static const FLOAT32
    ixheaacd_lattice_coeff_3_filt_den_coeff[DECORR_FILT_3_ORD + 1] = {
        1.000000f, 0.034742f, -0.013000f};
static const FLOAT32
    ixheaacd_lattice_coeff_3_filt_num_coeff[DECORR_FILT_3_ORD + 1] = {
        -0.013000f, 0.034742f, 1.000000f};

static const FLOAT32
    ixheaacd_lattice_coeff_1_filt_num_ldmps[DECORR_FILTER_ORDER_BAND_1 + 1] = {
        (0.3355999887f),  (0.0024894588f),  (-0.1572290659f), (0.2807503343f),
        (-0.1942857355f), (0.3840600252f),  (-0.4084388912f), (-0.1750483066f),
        (0.5559588671f),  (-0.4935829639f), (0.0567415841f),  (-0.0658148378f),
        (0.3378961682f),  (0.2284426540f),  (-0.7025330663f), (1.0000000000f)};

static const FLOAT32
    ixheaacd_lattice_coeff_1_filt_den_ldmps[DECORR_FILTER_ORDER_BAND_1 + 1] = {
        (1.0000000000f),  (-0.7025330663f), (0.2284426540f),  (0.3378961682f),
        (-0.0658148378f), (0.0567415841f),  (-0.4935829639f), (0.5559588671f),
        (-0.1750483066f), (-0.4084388912f), (0.3840600252f),  (-0.1942857355f),
        (0.2807503343f),  (-0.1572290659f), (0.0024894588f),  (0.3355999887f)};

static const FLOAT32
    ixheaacd_lattice_coeff_2_filt_num_ldmps[DECORR_FILTER_ORDER_BAND_2 + 1] = {
        (-0.4623999894f), (0.2341193259f), (0.5163637400f), (-0.0253488291f),
        (-0.2871030867f), (0.0153170601f), (1.0000000000f)};

static const FLOAT32
    ixheaacd_lattice_coeff_2_filt_den_ldmps[DECORR_FILTER_ORDER_BAND_2 + 1] = {
        (1.0000000000f), (0.0153170601f), (-0.2871030867f), (-0.0253488291f),
        (0.5163637400f), (0.2341193259f), (-0.4623999894f)

};

static const FLOAT32
    ixheaacd_lattice_coeff_3_filt_num_ldmps[DECORR_FILTER_ORDER_BAND_3 + 1] = {
        (0.2468000054f), (0.0207958221f), (-0.3898491263f), (1.0000000000f)};

static const FLOAT32
    ixheaacd_lattice_coeff_3_filt_den_ldmps[DECORR_FILTER_ORDER_BAND_3 + 1] = {
        (1.0000000000f), (-0.3898491263f), (0.0207958221f), (0.2468000054f)};

extern WORD32
    ixheaacd_hybrid_band_71_to_processing_band_28_map[MAX_HYBRID_BANDS_MPS];
extern WORD32
    ixheaacd_hybrid_band_64_to_processing_band_23_map[MAX_HYBRID_BANDS_MPS];

static const WORD32 ixheaacd_hybrid_to_qmf_map[MAX_HYBRID_BANDS_MPS] = {
    0,  0,  0,  0,  0,  0,  1,  1,  2,  2,  3,  4,  5,  6,  7,  8,  9,  10,
    11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28,
    29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46,
    47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63};

static const WORD32 ixheaacd_hybrid_to_qmf_map_ldmps[MAX_HYBRID_BANDS_MPS] = {
    0,  1,  2,  3,  4,  5,  6,  7,  8,  9,  10, 11, 12, 13, 14, 15, 16, 17,
    18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35,
    36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53,
    54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 70};

static VOID ixheaacd_mps_decor_filt_init(ia_mps_decor_filt_struct *self,
                                         WORD32 reverb_band,
                                         WORD32 object_type) {
  if (object_type == AOT_ER_AAC_ELD || object_type == AOT_ER_AAC_LD) {
    switch (reverb_band) {
      case 0:
        self->num_len = self->den_len = DECORR_FILTER_ORDER_BAND_0 + 1;
        self->num = NULL;
        self->den = NULL;

        break;
      case 1:
        self->num_len = self->den_len = DECORR_FILTER_ORDER_BAND_1 + 1;
        self->num = ixheaacd_lattice_coeff_1_filt_num_ldmps;
        self->den = ixheaacd_lattice_coeff_1_filt_den_ldmps;

        break;
      case 2:
        self->num_len = self->den_len = DECORR_FILTER_ORDER_BAND_2 + 1;
        self->num = ixheaacd_lattice_coeff_2_filt_num_ldmps;
        self->den = ixheaacd_lattice_coeff_2_filt_den_ldmps;
        break;
      case 3:
        self->num_len = self->den_len = DECORR_FILTER_ORDER_BAND_3 + 1;
        self->num = ixheaacd_lattice_coeff_3_filt_num_ldmps;
        self->den = ixheaacd_lattice_coeff_3_filt_den_ldmps;
        break;
    }
  } else {
    switch (reverb_band) {
      case 0:
        self->num_len = self->den_len = DECORR_FILT_0_ORD + 1;
        self->num = ixheaacd_lattice_coeff_0_filt_num_coeff;
        self->den = ixheaacd_lattice_coeff_0_filt_den_coeff;

        break;
      case 1:
        self->num_len = self->den_len = DECORR_FILT_1_ORD + 1;
        self->num = ixheaacd_lattice_coeff_1_filt_num_coeff;
        self->den = ixheaacd_lattice_coeff_1_filt_den_coeff;

        break;
      case 2:
        self->num_len = self->den_len = DECORR_FILT_2_ORD + 1;
        self->num = ixheaacd_lattice_coeff_2_filt_num_coeff;
        self->den = ixheaacd_lattice_coeff_2_filt_den_coeff;
        break;
      case 3:
        self->num_len = self->den_len = DECORR_FILT_3_ORD + 1;
        self->num = ixheaacd_lattice_coeff_3_filt_num_coeff;
        self->den = ixheaacd_lattice_coeff_3_filt_den_coeff;
        break;
    }
  }

  self->state_len = self->num_len;
  memset(self->state, 0,
         sizeof(ia_cmplx_flt_struct) * (MAX_DECORR_FILTER_ORDER + 1));

  return;
}

static VOID ixheaacd_mps_allpass_apply(ia_mps_decor_filt_struct *self,
                                       ia_cmplx_flt_struct *input, WORD32 len,
                                       ia_cmplx_flt_struct *output) {
  WORD32 i, j;

  for (i = 0; i < len; i++) {
    output[i].re = self->state[0].re + input[i].re * self->num[0];
    output[i].im = self->state[0].im + input[i].im * self->num[0];

    for (j = 1; j < self->num_len; j++) {
      self->state[j - 1].re = self->state[j].re + self->num[j] * input[i].re -
                              self->den[j] * output[i].re;
      self->state[j - 1].im = self->state[j].im + self->num[j] * input[i].im -
                              self->den[j] * output[i].im;
    }
  }
}

static VOID ixheaacd_mps_decor_energy_adjustment(
    ixheaacd_mps_decor_energy_adjust_filt_struct *handle,
    ia_cmplx_flt_struct in[MAX_TIME_SLOTS][MAX_HYBRID_BANDS_MPS],
    ia_cmplx_flt_struct out[MAX_TIME_SLOTS][MAX_HYBRID_BANDS_MPS],
    WORD32 time_slots, WORD32 res_bands, WORD32 ldmps_present) {
  ixheaacd_mps_decor_energy_adjust_filt_struct *self =
      (ixheaacd_mps_decor_energy_adjust_filt_struct *)handle;
  FLOAT32 in_energy[MAX_PARAMETER_BANDS] = {0};
  FLOAT32 out_energy[MAX_PARAMETER_BANDS] = {0};
  FLOAT32 gain[MAX_PARAMETER_BANDS];
  WORD32 i, j, k, loop_counter;
  WORD32 *ptr_hybrid_band;

  if (ldmps_present == 1)
    ptr_hybrid_band = ixheaacd_hybrid_band_64_to_processing_band_23_map;
  else
    ptr_hybrid_band = ixheaacd_hybrid_band_71_to_processing_band_28_map;

  WORD32 start_param_band = 0, start_bin = 0;

  if (res_bands != NO_RES_BANDS) {
    start_bin = ixheaacd_mps_gain_set_indx[res_bands];
    start_param_band = res_bands;
  }

  for (i = 0; i < time_slots; i++) {
    memset(in_energy, 0, sizeof(FLOAT32) * MAX_PARAMETER_BANDS);
    memset(out_energy, 0, sizeof(FLOAT32) * MAX_PARAMETER_BANDS);

    for (j = start_bin; j < self->num_bins; j++) {
      k = ptr_hybrid_band[j];

      in_energy[k] += in[i][j].re * in[i][j].re + in[i][j].im * in[i][j].im;
      out_energy[k] +=
          out[i][j].re * out[i][j].re + out[i][j].im * out[i][j].im;
    }

    loop_counter = MAX_PARAMETER_BANDS;

    for (k = start_param_band; k < loop_counter; k++) {
      self->smooth_in_energy[k] = self->smooth_in_energy[k] * DECOR_ALPHA +
                                  in_energy[k] * ONE_MINUS_DECOR_ALPHA;
      self->smooth_out_energy[k] = self->smooth_out_energy[k] * DECOR_ALPHA +
                                   out_energy[k] * ONE_MINUS_DECOR_ALPHA;

      gain[k] = 1.0f;

      if (self->smooth_out_energy[k] >
          self->smooth_in_energy[k] * DECOR_GAMMA) {
        gain[k] = (FLOAT32)sqrt(self->smooth_in_energy[k] * DECOR_GAMMA /
                                (self->smooth_out_energy[k] + ABS_THR));
      }

      if (self->smooth_in_energy[k] >
          self->smooth_out_energy[k] * DECOR_GAMMA) {
        gain[k] =
            min(2.0f, (FLOAT32)sqrt(self->smooth_in_energy[k] /
                                    (DECOR_GAMMA * self->smooth_out_energy[k] +
                                     ABS_THR)));
      }
    }

    for (j = start_bin; j < self->num_bins; j++) {
      k = ptr_hybrid_band[j];

      out[i][j].re *= gain[k];
      out[i][j].im *= gain[k];
    }
  }
}

IA_ERRORCODE ixheaacd_mps_decor_init(ia_mps_decor_struct *self,
                                     WORD32 subbands, WORD32 decor_config,
                                     WORD32 object_type) {
  WORD32 i, reverb_band;
  const WORD32 *splitfreq;
  const WORD32 *ptr_ixheaacd_hybrid_to_qmf_map;
  const WORD32 *ptr_decorr_delay;
  if (object_type == AOT_ER_AAC_ELD || object_type == AOT_ER_AAC_LD) {
    ptr_ixheaacd_hybrid_to_qmf_map = ixheaacd_hybrid_to_qmf_map_ldmps;
    ptr_decorr_delay = ixheaacd_decorr_delay_ldmps;
    switch (decor_config) {
      case 0:
        splitfreq = ixheaacd_qmf_split_freq_0_ldmps;
        break;
      case 1:
        splitfreq = ixheaacd_qmf_split_freq_1_ldmps;
        break;
      case 2:
        splitfreq = ixheaacd_qmf_split_freq_2_ldmps;
        break;
      default:
        return IA_FATAL_ERROR;
    }
  } else {
    ptr_ixheaacd_hybrid_to_qmf_map = ixheaacd_hybrid_to_qmf_map;
    ptr_decorr_delay = ixheaacd_decorr_delay;
    switch (decor_config) {
      case 0:
        splitfreq = ixheaacd_qmf_split_freq_0;
        break;
      case 1:
        splitfreq = ixheaacd_qmf_split_freq_1;
        break;
      case 2:
        splitfreq = ixheaacd_qmf_split_freq_2;
        break;
      default:
        return IA_FATAL_ERROR;
    }
  }

  self->num_bins = subbands;
  if (self->num_bins > MAX_HYBRID_BANDS_MPS) return IA_FATAL_ERROR;

  for (i = 0; i < self->num_bins; i++) {
    reverb_band = 0;
    while ((reverb_band < 3) &&
           (ptr_ixheaacd_hybrid_to_qmf_map[i] >= (splitfreq[reverb_band] - 1)))
      reverb_band++;

    self->delay_sample_count[i] = ptr_decorr_delay[reverb_band];
    ixheaacd_mps_decor_filt_init(&self->filter[i], reverb_band, object_type);
  }

  self->decor_nrg_smooth.num_bins = self->num_bins;

  return IA_NO_ERROR;
}

VOID ixheaacd_mps_decor_apply(
    ia_mps_decor_struct *self,
    ia_cmplx_flt_struct in[MAX_TIME_SLOTS][MAX_HYBRID_BANDS_MPS],
    ia_cmplx_flt_struct out[MAX_TIME_SLOTS][MAX_HYBRID_BANDS_MPS],
    WORD32 length, WORD32 res_bands, WORD32 ldmps_present) {
  WORD32 idx, sb_sample, index = 0;

  ia_cmplx_flt_struct scratch[MAX_TIME_SLOTS];

  if (res_bands != NO_RES_BANDS) index = ixheaacd_mps_gain_set_indx[res_bands];

  for (idx = index; idx < self->num_bins; idx++) {
    for (sb_sample = 0; sb_sample < length; sb_sample++) {
      self->decor_delay_buffer[idx][self->delay_sample_count[idx] + sb_sample]
          .re = in[sb_sample][idx].re;
      self->decor_delay_buffer[idx][self->delay_sample_count[idx] + sb_sample]
          .im = in[sb_sample][idx].im;
    }
    ixheaacd_mps_allpass_apply(&self->filter[idx],
                               self->decor_delay_buffer[idx], length, scratch);

    for (sb_sample = 0; sb_sample < length; sb_sample++) {
      out[sb_sample][idx].re = scratch[sb_sample].re;
      out[sb_sample][idx].im = scratch[sb_sample].im;
    }

    for (sb_sample = 0; sb_sample < self->delay_sample_count[idx];
         sb_sample++) {
      self->decor_delay_buffer[idx][sb_sample].re =
          self->decor_delay_buffer[idx][length + sb_sample].re;
      self->decor_delay_buffer[idx][sb_sample].im =
          self->decor_delay_buffer[idx][length + sb_sample].im;
    }
  }

  ixheaacd_mps_decor_energy_adjustment(&self->decor_nrg_smooth, in, out, length,
                                       res_bands,
                                       ldmps_present);
}

static VOID ixheaacd_convert_lattice_coefs_complex(WORD32 const order,
                                                   WORD32 const *const rfc_real,
                                                   WORD32 const *const rfc_imag,
                                                   WORD32 *const apar_real,
                                                   WORD32 *const apar_imag) {
  WORD32 i, j;
  WORD32 tmp_real[MAX_DECORR_FILTER_ORDER + 1];
  WORD32 tmp_imag[MAX_DECORR_FILTER_ORDER + 1];
  WORD64 temp;

  apar_real[0] = 32768;
  apar_imag[0] = 0;

  for (i = 0; i < order; i++) {
    apar_real[i + 1] = rfc_real[i];
    apar_imag[i + 1] = rfc_imag[i];
    for (j = 0; j < i; j++) {
      temp = (WORD64)((WORD64)rfc_real[i] * (WORD64)tmp_real[i - j - 1] +
                      (WORD64)rfc_imag[i] * (WORD64)tmp_imag[i - j - 1]);
      temp >>= 15;
      apar_real[j + 1] = ixheaac_add32(tmp_real[j], (WORD32)temp);

      temp = (WORD64)((WORD64)rfc_real[i] * (WORD64)tmp_imag[i - j - 1] +
                      (WORD64)rfc_imag[i] * (WORD64)tmp_real[i - j - 1]);
      temp >>= 15;
      apar_imag[j + 1] = ixheaac_sub32(tmp_imag[j], (WORD32)temp);
    }
    for (j = 0; j <= i; j++) {
      tmp_real[j] = apar_real[j + 1];
      tmp_imag[j] = apar_imag[j + 1];
    }
  }
}

static IA_ERRORCODE ixheaacd_decorr_filt_create(
    ia_mps_dec_decorr_filter_instance_struct *self, WORD32 const decorr_seed,
    WORD32 const qmf_band, WORD32 const reverb_band, WORD32 const dec_type,
    ia_mps_dec_mps_tables_struct *ia_mps_dec_mps_table) {
  IA_ERRORCODE error_code = IA_NO_ERROR;
  WORD32 i;
  const WORD32 *lattice_coeff = NULL;
  WORD32 lattice_coeff_real[MAX_DECORR_FILTER_ORDER];
  WORD32 lattice_coeff_imag[MAX_DECORR_FILTER_ORDER];
  WORD32 temp_1;

  if (self == NULL) {
    error_code = IA_FATAL_ERROR;
  }

  if (error_code == IA_NO_ERROR) {
    switch (reverb_band) {
      case REVERB_BAND_0:
        self->num_length = self->den_length = DECORR_FILTER_ORDER_BAND_0 + 1;
        lattice_coeff = &(ia_mps_dec_mps_table->decor_table_ptr
                              ->lattice_coeff_0[decorr_seed][0]);
        break;
      case REVERB_BAND_1:
        self->num_length = self->den_length = DECORR_FILTER_ORDER_BAND_1 + 1;
        lattice_coeff = &(ia_mps_dec_mps_table->decor_table_ptr
                              ->lattice_coeff_1[decorr_seed][0]);
        break;
      case REVERB_BAND_2:
        self->num_length = self->den_length = DECORR_FILTER_ORDER_BAND_2 + 1;
        lattice_coeff = &(ia_mps_dec_mps_table->decor_table_ptr
                              ->lattice_coeff_2[decorr_seed][0]);
        break;
      case REVERB_BAND_3:
        self->num_length = self->den_length = DECORR_FILTER_ORDER_BAND_3 + 1;
        lattice_coeff = &(ia_mps_dec_mps_table->decor_table_ptr
                              ->lattice_coeff_3[decorr_seed][0]);
        break;
      default:
        return IA_FATAL_ERROR;
    }
    self->state_length = (self->num_length > self->den_length)
                             ? self->num_length
                             : self->den_length;
  }

  if (error_code == IA_NO_ERROR) {
    const WORD32 *cos_tab =
        ia_mps_dec_mps_table->hybrid_table_ptr->cosine_array;
    const WORD32 *sin_tab = ia_mps_dec_mps_table->hybrid_table_ptr->sine_array;

    if (dec_type == 1) {
      for (i = 0; i < self->num_length - 1; i++) {
        temp_1 = (qmf_band * ia_mps_dec_mps_table->decor_table_ptr
                                 ->lattice_delta_phi[decorr_seed][i]) >>
                 1;
        lattice_coeff_real[i] = ixheaacd_mps_mult32_shr_15(
            ixheaacd_mps_cos(temp_1, cos_tab), lattice_coeff[i]);
        lattice_coeff_imag[i] = ixheaacd_mps_mult32_shr_15(
            ixheaacd_mps_sin(temp_1, sin_tab), lattice_coeff[i]);
      }

      ixheaacd_convert_lattice_coefs_complex(
          self->num_length - 1, lattice_coeff_real, lattice_coeff_imag,
          self->denominator_real, self->denominator_imag);
      for (i = 0; i < self->num_length; i++) {
        self->numerator_real[i] =
            self->denominator_real[self->num_length - 1 - i];
        self->numerator_imag[i] =
            -self->denominator_imag[self->num_length - 1 - i];
      }

      self->complex = 1;
    } else {
      switch (reverb_band) {
        case REVERB_BAND_0:
          self->denominator_real = &(ia_mps_dec_mps_table->decor_table_ptr
                                         ->den_coef_0[decorr_seed][0]);
          break;
        case REVERB_BAND_1:
          self->denominator_real = &(ia_mps_dec_mps_table->decor_table_ptr
                                         ->den_coef_1[decorr_seed][0]);
          break;
        case REVERB_BAND_2:
          self->denominator_real = &(ia_mps_dec_mps_table->decor_table_ptr
                                         ->den_coef_2[decorr_seed][0]);
          break;
        case REVERB_BAND_3:
          self->denominator_real = &(ia_mps_dec_mps_table->decor_table_ptr
                                         ->den_coef_3[decorr_seed][0]);
          break;
        default:
          return IA_FATAL_ERROR;
      }

      for (i = 0; i < self->num_length; i++) {
        self->numerator_real[i] =
            self->denominator_real[self->num_length - 1 - i];
      }
      self->complex = 0;
    }
  }
  return error_code;
}

static VOID ixheaacd_decorr_filt_apply(
    ia_mps_dec_decorr_filter_instance_struct *const self, WORD32 const length,
    WORD32 const *const input_real, WORD32 const *const input_imag,
    WORD32 *const p_output_real, WORD32 *const p_output_imag) {
  WORD32 temp_1, temp_2, temp3, temp4;
  WORD32 temp5, temp6, temp7, temp8;
  WORD32 *state_real, *state_imag;
  WORD32 *numerator_real, *denominator_real;
  WORD32 *output_real = p_output_real;
  WORD32 *output_imag = p_output_imag;

  WORD32 common_part;
  WORD32 i;
  WORD32 j;

  common_part = self->num_length;
  state_real = self->state_real;
  state_imag = self->state_imag;
  numerator_real = self->numerator_real;
  denominator_real = self->denominator_real;

  {
    for (i = 0; i < length; i++) {
      {
        temp5 = input_real[i];
        temp6 = input_imag[i];

        temp_1 = ixheaacd_mps_mult32_shr_14(temp5, numerator_real[0]);
        temp_2 = ixheaacd_mps_mult32_shr_14(temp6, numerator_real[0]);

        *output_real = ixheaac_add32_sat(temp_1, state_real[0]);
        *output_imag = ixheaac_add32_sat(temp_2, state_imag[0]);

        temp7 = *output_real;
        temp8 = *output_imag;

        output_real += MAX_HYBRID_BANDS;
        output_imag += MAX_HYBRID_BANDS;
        for (j = 1; j < common_part; j++) {
          temp_1 = ixheaacd_mps_mult32x16_shr_16(temp5, numerator_real[j]);
          temp3 = ixheaacd_mps_mult32x16_shr_16(temp6, numerator_real[j]);
          temp_2 = ixheaacd_mps_mult32x16_shr_16(temp7, denominator_real[j]);
          temp4 = ixheaacd_mps_mult32x16_shr_16(temp8, denominator_real[j]);
          temp_1 -= temp_2;

          state_real[j - 1] = ixheaac_add32_sat(state_real[j], (temp_1 << 2));
          temp3 -= temp4;

          state_imag[j - 1] = ixheaac_add32_sat(state_imag[j], (temp3 << 2));
        }
      }
    }
  }
}

static VOID ixheaacd_ducker_apply_71(
    ia_mps_dec_ducker_interface *const face, WORD32 const time_slots,
    WORD32 const *input_real, WORD32 const *input_imag, WORD32 *output_real,
    WORD32 *output_imag, ia_mps_dec_mps_tables_struct *ia_mps_dec_mps_table_ptr,
    VOID *scratch) {
  ia_mps_dec_duck_instance_struct *self =
      (ia_mps_dec_duck_instance_struct *)&face[1];
  WORD32 *duck_gain;
  WORD32 gain;
  WORD16 qgain;
  WORD64 direct_nrg[28];
  WORD64 reverb_nrg[28];
  WORD16 *q_duck_gain;
  WORD32 ts;
  WORD32 qs;
  WORD32 pb;
  WORD16 qtemp1, qtemp2, qtemp3;
  WORD32 temp_1, temp_2, temp3;
  const WORD32 *p_input_real;
  const WORD32 *p_input_imag;
  const WORD32 *hybrid_2_param_28 =
      ia_mps_dec_mps_table_ptr->m1_m2_table_ptr->hybrid_2_param_28;
  const WORD32 *sqrt_tab = ia_mps_dec_mps_table_ptr->common_table_ptr->sqrt_tab;
  WORD32 *smooth_direct_nrg = self->smooth_direct_nrg;
  WORD16 *q_smooth_direct_nrg = self->q_smooth_direct_nrg;

  WORD32 *smooth_reverb_nrg = self->smooth_reverb_nrg;
  WORD16 *q_smooth_reverb_nrg = self->q_smooth_reverb_nrg;

  WORD32 parameter_bands = self->parameter_bands;

  WORD32 *p_output_real, *p_output_imag;

  WORD32 num_bands_2 = self->hybrid_bands;
  WORD32 v1, v2, v3, v4;
  WORD16 one_by_5 = ONE_BY_FIVE_Q16;

  duck_gain = scratch;
  q_duck_gain = (WORD16 *)scratch + IXHEAAC_GET_SIZE_ALIGNED_TYPE(
                                        PARAMETER_BANDSX2, sizeof(*q_duck_gain), BYTE_ALIGN_8);

  p_input_real = input_real;
  p_input_imag = input_imag;

  p_output_real = output_real;
  p_output_imag = output_imag;

  for (ts = 0; ts < time_slots; ts++) {
    memset(direct_nrg, 0, sizeof(direct_nrg));
    memset(reverb_nrg, 0, sizeof(reverb_nrg));

    for (qs = 0; qs < 55; qs++) {
      v1 = p_input_real[qs];
      v2 = p_input_imag[qs];
      v3 = p_output_real[qs];
      v4 = p_output_imag[qs];

      pb = hybrid_2_param_28[qs];
      direct_nrg[pb] = ixheaac_add64_sat(direct_nrg[pb], ixheaac_mult32x32in64(v1, v1));
      direct_nrg[pb] = ixheaac_add64_sat(direct_nrg[pb], ixheaac_mult32x32in64(v2, v2));
      reverb_nrg[pb] = ixheaac_add64_sat(reverb_nrg[pb], ixheaac_mult32x32in64(v3, v3));
      reverb_nrg[pb] = ixheaac_add64_sat(reverb_nrg[pb], ixheaac_mult32x32in64(v4, v4));
    }

    for (; qs < num_bands_2; qs++) {
      v1 = p_input_real[qs];
      v2 = p_input_imag[qs];
      v3 = p_output_real[qs];
      v4 = p_output_imag[qs];

      direct_nrg[27] = ixheaac_add64_sat(direct_nrg[27], ixheaac_mult32x32in64(v1, v1));
      direct_nrg[27] = ixheaac_add64_sat(direct_nrg[27], ixheaac_mult32x32in64(v2, v2));
      reverb_nrg[27] = ixheaac_add64_sat(reverb_nrg[27], ixheaac_mult32x32in64(v3, v3));
      reverb_nrg[27] = ixheaac_add64_sat(reverb_nrg[27], ixheaac_mult32x32in64(v4, v4));
    }

    for (pb = 0; pb < parameter_bands; pb++) {
      WORD16 qtemp, qtemp_1;
      temp_1 = ixheaacd_mps_narrow(direct_nrg[pb], &qtemp);

      temp_2 = smooth_direct_nrg[pb] << 2;
      temp3 =
          ixheaacd_mps_add32(temp_2, temp_1, &(q_smooth_direct_nrg[pb]), qtemp);
      smooth_direct_nrg[pb] = ixheaacd_mps_mult32x16_shr_16(temp3, one_by_5);

      temp_1 = ixheaacd_mps_narrow(reverb_nrg[pb], &qtemp);
      temp_2 = smooth_reverb_nrg[pb] << 2;

      temp3 =
          ixheaacd_mps_add32(temp_2, temp_1, &(q_smooth_reverb_nrg[pb]), qtemp);
      smooth_reverb_nrg[pb] = ixheaacd_mps_mult32x16_shr_16(temp3, one_by_5);

      qtemp1 = q_smooth_reverb_nrg[pb] - 1;
      temp_1 = (smooth_reverb_nrg[pb] >> 2) * 3;
      qtemp = q_smooth_direct_nrg[pb];
      temp3 = smooth_direct_nrg[pb];

      if (ixheaacd_mps_comp(temp3, temp_1, &qtemp, qtemp1)) {
        temp_2 = ixheaacd_mps_div_32(temp3, temp_1, &qtemp2);
        qtemp2 = qtemp2 + qtemp - qtemp1;
        if (temp_1 == 0) {
          qtemp2 = qtemp;
        }
        temp3 = (qtemp2) > 28 ? MAX_32 : 4 << qtemp2;

        if (temp_2 > temp3) {
          *duck_gain = (ONE_IN_Q15 - 1);
          *q_duck_gain++ = 14;
        } else {
          *duck_gain = ixheaacd_mps_sqrt(temp_2, &qtemp2, sqrt_tab);
          *q_duck_gain++ = qtemp2;
        }
        duck_gain++;
        continue;
      }

      *duck_gain = ONE_IN_Q14 - 1;

      qtemp = q_smooth_direct_nrg[pb] - 1;
      temp_1 = (smooth_direct_nrg[pb] >> 2) * 3;

      qtemp_1 = q_smooth_reverb_nrg[pb];
      temp_2 = smooth_reverb_nrg[pb];
      if (ixheaacd_mps_comp(temp_2, temp_1, &(qtemp_1), qtemp)) {
        temp3 = ixheaacd_mps_div_32(temp_1, temp_2, &qtemp3);
        qtemp3 = qtemp3 + qtemp - qtemp_1;

        *duck_gain = ixheaacd_mps_sqrt(temp3, &qtemp3, sqrt_tab);
        *q_duck_gain = qtemp3;
      }

      duck_gain++;
      q_duck_gain++;
    }
    duck_gain -= parameter_bands;
    q_duck_gain -= parameter_bands;

    for (qs = 0; qs < 55; qs++) {
      pb = hybrid_2_param_28[qs];
      gain = duck_gain[pb];
      if (gain == 16383) {
        continue;
      }
      qgain = q_duck_gain[pb];
      p_output_real[qs] =
          ixheaacd_mps_mult32_shr_n(p_output_real[qs], gain, qgain);
      p_output_imag[qs] =
          ixheaacd_mps_mult32_shr_n(p_output_imag[qs], gain, qgain);
    }

    gain = duck_gain[27];

    if (gain != 16383) {
      qgain = q_duck_gain[27];
      for (; qs < num_bands_2; qs++) {
        p_output_real[qs] =
            ixheaacd_mps_mult32_shr_n(p_output_real[qs], gain, qgain);
        p_output_imag[qs] =
            ixheaacd_mps_mult32_shr_n(p_output_imag[qs], gain, qgain);
      }
    }

    p_input_real += MAX_HYBRID_BANDS;
    p_input_imag += MAX_HYBRID_BANDS;

    p_output_real += MAX_HYBRID_BANDS;
    p_output_imag += MAX_HYBRID_BANDS;
  }
}

static VOID ixheaacd_ducker_apply(
    ia_mps_dec_ducker_interface *const face, WORD32 const time_slots,
    WORD32 const *input_real, WORD32 const *input_imag, WORD32 *output_real,
    WORD32 *output_imag, ia_mps_dec_mps_tables_struct *ia_mps_dec_mps_table_ptr,
    VOID *scratch) {
  ia_mps_dec_duck_instance_struct *self =
      (ia_mps_dec_duck_instance_struct *)&face[1];
  WORD32 *duck_gain;
  WORD32 gain;
  WORD16 qgain;
  WORD64 direct_nrg[28];
  WORD64 reverb_nrg[28];
  WORD16 *q_duck_gain;
  WORD32 ts;
  WORD32 qs;
  WORD32 pb;
  WORD16 qtemp1, qtemp2, qtemp3;
  WORD32 temp_1, temp_2, temp3;
  const WORD32 *p_input_real;
  const WORD32 *p_input_imag;
  const WORD32 *hybrid_2_param_28 =
      ia_mps_dec_mps_table_ptr->m1_m2_table_ptr->hybrid_2_param_28;
  const WORD32 *sqrt_tab = ia_mps_dec_mps_table_ptr->common_table_ptr->sqrt_tab;
  WORD32 *smooth_direct_nrg = self->smooth_direct_nrg;
  WORD16 *q_smooth_direct_nrg = self->q_smooth_direct_nrg;

  WORD32 *smooth_reverb_nrg = self->smooth_reverb_nrg;
  WORD16 *q_smooth_reverb_nrg = self->q_smooth_reverb_nrg;

  WORD32 parameter_bands = self->parameter_bands;

  WORD32 *p_output_real, *p_output_imag;

  WORD32 num_bands_2 = self->hybrid_bands;
  WORD32 v1, v2, v3, v4;
  WORD16 one_by_5 = ONE_BY_FIVE_Q16;

  duck_gain = scratch;
  q_duck_gain = (WORD16 *)scratch + IXHEAAC_GET_SIZE_ALIGNED_TYPE(
                                        PARAMETER_BANDSX2, sizeof(*q_duck_gain), BYTE_ALIGN_8);

  p_input_real = input_real;
  p_input_imag = input_imag;

  p_output_real = output_real;
  p_output_imag = output_imag;

  for (ts = 0; ts < time_slots; ts++) {
    memset(direct_nrg, 0, sizeof(direct_nrg));
    memset(reverb_nrg, 0, sizeof(reverb_nrg));

    for (qs = 0; qs < num_bands_2; qs++) {
      v1 = p_input_real[qs];
      v2 = p_input_imag[qs];
      v3 = p_output_real[qs];
      v4 = p_output_imag[qs];

      pb = hybrid_2_param_28[qs];
      direct_nrg[pb] = ixheaac_add64_sat(direct_nrg[pb], ixheaac_mult32x32in64(v1, v1));
      direct_nrg[pb] = ixheaac_add64_sat(direct_nrg[pb], ixheaac_mult32x32in64(v2, v2));
      reverb_nrg[pb] = ixheaac_add64_sat(reverb_nrg[pb], ixheaac_mult32x32in64(v3, v3));
      reverb_nrg[pb] = ixheaac_add64_sat(reverb_nrg[pb], ixheaac_mult32x32in64(v4, v4));
    }

    for (pb = 0; pb < parameter_bands; pb++) {
      WORD16 qtemp, qtemp_1;
      temp_1 = ixheaacd_mps_narrow(direct_nrg[pb], &qtemp);
      temp_2 = smooth_direct_nrg[pb] << 2;
      temp3 =
          ixheaacd_mps_add32(temp_2, temp_1, &(q_smooth_direct_nrg[pb]), qtemp);
      smooth_direct_nrg[pb] = ixheaacd_mps_mult32x16_shr_16(temp3, one_by_5);

      temp_1 = ixheaacd_mps_narrow(reverb_nrg[pb], &qtemp);
      temp_2 = smooth_reverb_nrg[pb] << 2;

      temp3 =
          ixheaacd_mps_add32(temp_2, temp_1, &(q_smooth_reverb_nrg[pb]), qtemp);
      smooth_reverb_nrg[pb] = ixheaacd_mps_mult32x16_shr_16(temp3, one_by_5);

      qtemp1 = q_smooth_reverb_nrg[pb] - 1;
      temp_1 = (smooth_reverb_nrg[pb] >> 2) * 3;
      qtemp = q_smooth_direct_nrg[pb];
      temp3 = smooth_direct_nrg[pb];

      if (ixheaacd_mps_comp(temp3, temp_1, &qtemp, qtemp1)) {
        temp_2 = ixheaacd_mps_div_32(temp3, temp_1, &qtemp2);
        qtemp2 = qtemp2 + qtemp - qtemp1;
        if (temp_1 == 0) {
          qtemp2 = qtemp;
        }
        temp3 = qtemp2 > 28 ? MAX_32 : 4 << qtemp2;

        if (temp_2 > temp3) {
          *duck_gain = 32767;
          *q_duck_gain++ = 14;
        } else {
          *duck_gain = ixheaacd_mps_sqrt(temp_2, &qtemp2, sqrt_tab);
          *q_duck_gain++ = qtemp2;
        }
        duck_gain++;
        continue;
      }

      *duck_gain = 16383;

      qtemp = q_smooth_direct_nrg[pb] - 1;
      temp_1 = (smooth_direct_nrg[pb] >> 2) * 3;

      qtemp_1 = q_smooth_reverb_nrg[pb];
      temp_2 = smooth_reverb_nrg[pb];
      if (ixheaacd_mps_comp(temp_2, temp_1, &(qtemp_1), qtemp)) {
        temp3 = ixheaacd_mps_div_32(temp_1, temp_2, &qtemp3);
        qtemp3 = qtemp3 + qtemp - qtemp_1;

        *duck_gain = ixheaacd_mps_sqrt(temp3, &qtemp3, sqrt_tab);
        *q_duck_gain = qtemp3;
      }

      duck_gain++;
      q_duck_gain++;
    }

    duck_gain -= parameter_bands;
    q_duck_gain -= parameter_bands;

    for (qs = 0; qs < num_bands_2; qs++) {
      pb = hybrid_2_param_28[qs];
      gain = duck_gain[pb];
      if (gain == 16383) {
        continue;
      }
      qgain = q_duck_gain[pb];
      p_output_real[qs] =
          ixheaacd_mps_mult32_shr_n(p_output_real[qs], gain, qgain);
      p_output_imag[qs] =
          ixheaacd_mps_mult32_shr_n(p_output_imag[qs], gain, qgain);
    }

    p_input_real += MAX_HYBRID_BANDS;
    p_input_imag += MAX_HYBRID_BANDS;

    p_output_real += MAX_HYBRID_BANDS;
    p_output_imag += MAX_HYBRID_BANDS;
  }
}

static IA_ERRORCODE ixheaacd_ducker_create(
    ia_mps_dec_ducker_interface *const face, WORD32 const hybrid_bands) {
  ia_mps_dec_duck_instance_struct *self = NULL;
  IA_ERRORCODE error_code = IA_NO_ERROR;
  WORD32 i;

  if (face == NULL) {
    error_code = IA_FATAL_ERROR;
  }

  if (error_code == IA_NO_ERROR) {
    self = (ia_mps_dec_duck_instance_struct *)&face[1];

    self->hybrid_bands = hybrid_bands;
    self->parameter_bands = MAX_PARAMETER_BANDS;

    self->alpha = DUCK_ALPHA;
    self->one_minus_alpha = DUCK_ONEMINUSALPHA;
    self->gamma = DUCK_GAMMA;
    self->abs_thr = ABS_THR_FIX;
    self->hybrid_bands = hybrid_bands;
    self->parameter_bands = MAX_PARAMETER_BANDS;

    self->qalpha = 15;
    self->qgamma = 14;

    if (hybrid_bands == 71)
      face->apply = ixheaacd_ducker_apply_71;
    else
      face->apply = ixheaacd_ducker_apply;

    for (i = 0; i < MAX_PARAMETER_BANDS; i++) {
      self->q_smooth_direct_nrg[i] = 31;
      self->q_smooth_reverb_nrg[i] = 31;
    }
  }

  return error_code;
}

IA_ERRORCODE ixheaacd_decorr_create(
    ia_mps_dec_decorr_dec_handle self, WORD32 subbands, WORD32 seed,
    WORD32 dec_type, WORD32 decorr_config,
    ia_mps_dec_mps_tables_struct *ia_mps_dec_mps_table_ptr) {
  IA_ERRORCODE error_code = IA_NO_ERROR;
  WORD32 i, reverb_band;

  const WORD32 *rev_split_freq;

  switch (decorr_config) {
    case DECOR_CONFIG_0:
      rev_split_freq =
          ia_mps_dec_mps_table_ptr->decor_table_ptr->rev_table.rev_split_freq_0;
      break;
    case DECOR_CONFIG_1:
      rev_split_freq =
          ia_mps_dec_mps_table_ptr->decor_table_ptr->rev_table.rev_split_freq_1;
      break;
    case DECOR_CONFIG_2:
      rev_split_freq =
          ia_mps_dec_mps_table_ptr->decor_table_ptr->rev_table.rev_split_freq_2;
      break;
    default:
      return IA_FATAL_ERROR;
      break;
  }

  if (error_code == IA_NO_ERROR) {
    self->decorr_seed = seed;
    self->numbins = subbands;

    for (i = 0; i < self->numbins; i++) {
      reverb_band = 0;
      while ((reverb_band < 3) &&
             (ixheaacd_get_qmf_sb(
                  i, ia_mps_dec_mps_table_ptr->mdct2qmf_table_ptr) >=
              (rev_split_freq[reverb_band] - 1)))
        reverb_band++;

      {
        self->no_sample_delay[i] =
            ia_mps_dec_mps_table_ptr->decor_table_ptr->rev_table
                .rev_delay[reverb_band][self->decorr_seed];

        error_code = ixheaacd_decorr_filt_create(
            self->filter[i], self->decorr_seed,
            ixheaacd_get_qmf_sb(i,
                                ia_mps_dec_mps_table_ptr->mdct2qmf_table_ptr),
            reverb_band, dec_type, ia_mps_dec_mps_table_ptr);
      }
    }

    if (error_code == IA_NO_ERROR) {
      error_code = ixheaacd_ducker_create(self->ducker, self->numbins);
    }
  }
  return (error_code);
}

VOID ixheaacd_decorr_apply(ia_heaac_mps_state_struct *pstr_mps_state, WORD32 length,
                           WORD32 *input_real, WORD32 *input_imag,
                           WORD32 *output_real, WORD32 *output_imag,
                           WORD32 index) {
  WORD32 l = index - pstr_mps_state->num_direct_signals;
  ia_mps_dec_decorr_dec_handle decorr_ptr = pstr_mps_state->ap_decor[l];
  WORD32 idx, sb_sample;

  WORD32 *p_input_real, *p_input_re, *p_input_imag, *p_input_im;
  WORD32 *p_output_real, *p_output_imag, *p_output_re, *p_output_im;
  WORD32 *delay_buffer_real, *delay_buffer_imag;
  WORD32 length1;
  VOID *free_scratch;

  free_scratch = (WORD32 *)pstr_mps_state->mps_scratch_mem_v +
                 IXHEAAC_GET_SIZE_ALIGNED_TYPE(MAX_TIMESLOTSX2, sizeof(WORD32), BYTE_ALIGN_8);

  if (decorr_ptr != NULL) {
    p_input_real = input_real;
    p_input_imag = input_imag;

    p_output_real = output_real;
    p_output_imag = output_imag;
    for (idx = 0; idx < decorr_ptr->numbins; idx++) {
      p_input_re = p_input_real;
      p_input_im = p_input_imag;

      p_output_re = p_output_real;
      p_output_im = p_output_imag;

      length1 = length - decorr_ptr->no_sample_delay[idx];
      delay_buffer_real =
          &decorr_ptr->delay_buffer_real[idx][decorr_ptr->no_sample_delay[idx]];
      delay_buffer_imag =
          &decorr_ptr->delay_buffer_imag[idx][decorr_ptr->no_sample_delay[idx]];
      for (sb_sample = 0; sb_sample < length1; sb_sample++) {
        delay_buffer_real[sb_sample] = *p_input_re;
        *delay_buffer_imag++ = *p_input_im;
        p_input_re += MAX_HYBRID_BANDS;
        p_input_im += MAX_HYBRID_BANDS;
      }
      {
        ixheaacd_decorr_filt_apply(
            decorr_ptr->filter[idx], length, decorr_ptr->delay_buffer_real[idx],
            decorr_ptr->delay_buffer_imag[idx], p_output_re++, p_output_im++);
      }

      length1 = decorr_ptr->no_sample_delay[idx];
      delay_buffer_real = &decorr_ptr->delay_buffer_real[idx][0];
      delay_buffer_imag = &decorr_ptr->delay_buffer_imag[idx][0];
      for (sb_sample = 0; sb_sample < length1; sb_sample++) {
        delay_buffer_real[sb_sample] = *p_input_re;
        p_input_re += MAX_HYBRID_BANDS;
        *delay_buffer_imag++ = *p_input_im;
        p_input_im += MAX_HYBRID_BANDS;
      }

      p_input_real++;
      p_input_imag++;

      p_output_real++;
      p_output_imag++;
    }
    decorr_ptr->ducker->apply(decorr_ptr->ducker, length, input_real,
                              input_imag, output_real, output_imag,
                              &(pstr_mps_state->ia_mps_dec_mps_table), free_scratch);
  }
}
