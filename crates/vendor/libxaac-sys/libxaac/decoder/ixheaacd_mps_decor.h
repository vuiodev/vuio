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
#ifndef IXHEAACD_MPS_DECOR_H
#define IXHEAACD_MPS_DECOR_H

#define DECOR_ALPHA (0.8f)
#define ONE_MINUS_DECOR_ALPHA (1 - DECOR_ALPHA)
#define DECOR_GAMMA (1.5f)

#define DUCK_ALPHA (26214)
#define DUCK_GAMMA (24576)
#define DUCK_ONEMINUSALPHA (6554)

enum {
  REVERB_BAND_0 = 0,
  REVERB_BAND_1 = 1,
  REVERB_BAND_2 = 2,
  REVERB_BAND_3 = 3
};

enum { DECOR_CONFIG_0 = 0, DECOR_CONFIG_1 = 1, DECOR_CONFIG_2 = 2 };

struct ia_mps_dec_ducker_interface {
  VOID(*apply)
  (ia_mps_dec_ducker_interface *const self, WORD32 const time_slots,
   WORD32 const *input_real, WORD32 const *input_imag, WORD32 *output_real,
   WORD32 *output_imag, ia_mps_dec_mps_tables_struct *ia_mps_dec_mps_table_ptr,
   VOID *scratch);
};

typedef struct {
  WORD32 hybrid_bands;
  WORD32 parameter_bands;
  WORD32 alpha;
  WORD32 one_minus_alpha;
  WORD32 gamma;
  WORD32 abs_thr;
  WORD16 qalpha;
  WORD16 qgamma;
  WORD32 smooth_direct_nrg[MAX_PARAMETER_BANDS];
  WORD32 smooth_reverb_nrg[MAX_PARAMETER_BANDS];
  WORD16 q_smooth_direct_nrg[MAX_PARAMETER_BANDS];
  WORD16 q_smooth_reverb_nrg[MAX_PARAMETER_BANDS];
} ia_mps_dec_duck_instance_struct;

VOID ixheaacd_decorr_apply(ia_heaac_mps_state_struct *pstr_mps_state, WORD32 length,
                           WORD32 *input_real, WORD32 *input_imag,
                           WORD32 *output_real, WORD32 *output_imag,
                           WORD32 index);

WORD32 ixheaacd_decorr_create(
    ia_mps_dec_decorr_dec_handle hDecorrDec, WORD32 subbands, WORD32 seed,
    WORD32 dec_type, WORD32 decorr_config,
    ia_mps_dec_mps_tables_struct *ia_mps_dec_mps_table);

IA_ERRORCODE ixheaacd_mps_decor_init(ia_mps_decor_struct *, WORD32, WORD32, WORD32);

VOID ixheaacd_mps_decor_apply(
    ia_mps_decor_struct *self,
    ia_cmplx_flt_struct in[MAX_TIME_SLOTS][MAX_HYBRID_BANDS_MPS],
    ia_cmplx_flt_struct out[MAX_TIME_SLOTS][MAX_HYBRID_BANDS_MPS],
    WORD32 length, WORD32 res_bands, WORD32 ldmps_present);

#endif /* IXHEAACD_MPS_DECOR_H */
