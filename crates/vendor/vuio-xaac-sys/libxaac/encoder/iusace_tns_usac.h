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

#pragma once
#define LN (2048)
#define SN (256)
#define LN2 (LN / 2)
#define NSHORT (LN / SN)

#define TNS_MAX_ORDER 31
#define DEF_TNS_GAIN_THRESH 1.41
#define DEF_TNS_COEFF_THRESH 0.1
#define DEF_TNS_RES_OFFSET 3

#ifndef PI
#define PI 3.14159265358979323846
#endif

typedef struct {
  WORD32 order;                        /**< Filter order */
  WORD32 direction;                    /**< Filtering direction */
  WORD32 coef_compress;                /**< Are coeffs compressed? */
  WORD32 length;                       /**< Length, in bands */
  FLOAT64 a_coeffs[TNS_MAX_ORDER + 1]; /**< AR Coefficients */
  FLOAT64 k_coeffs[TNS_MAX_ORDER + 1]; /**< Reflection Coefficients */
  WORD32 index[TNS_MAX_ORDER + 1];     /**< Coefficient indices */
} ia_tns_filter_data;

typedef struct {
  WORD32 n_filt;                    /**< number of filters */
  WORD32 coef_res;                  /**< Coefficient resolution */
  ia_tns_filter_data tns_filter[3]; /**< TNS filters */
  FLOAT64 tns_pred_gain;
  WORD32 tns_active;
} ia_tns_window_data;

typedef struct {
  WORD32 tns_data_present;
  WORD32 tns_min_band_number_long;
  WORD32 tns_min_band_number_short;
  WORD32 tns_max_bands_long;
  WORD32 tns_max_bands_short;
  WORD32 tns_max_order_long;
  WORD32 tns_max_order_short;
  WORD32 lpc_start_band_long;
  WORD32 lpc_start_band_short;
  WORD32 lpc_stop_band_long;
  WORD32 lpc_stop_band_short;
  ia_tns_window_data window_data[NSHORT]; /**< TNS data per window */
  WORD32 *sfb_offset_table_short;         /**< Scalefactor band offset table */
  WORD32 *sfb_offset_table_short_tcx;     /**< Scalefactor band offset table */
  WORD32 *sfb_offset_table_long;          /**< Scalefactor band offset table */
  WORD32 max_sfb_short;                   /**< max_sfb */
  WORD32 max_sfb_long;                    /**< max_sfb */
  FLOAT32 threshold;
  FLOAT32 tns_time_res_short;
  FLOAT32 tns_time_res_long;
  FLOAT64 win_short[8];
  FLOAT64 win_long[16];
  WORD32 block_type;      /**< block type */
  WORD32 number_of_bands; /**< number of bands per window */
  FLOAT64 *spec;          /**< Spectral data array */
} ia_tns_info;

IA_ERRORCODE iusace_tns_init(WORD32 sampling_rate, WORD32 bit_rate, ia_tns_info *pstr_tns_info,
                             WORD32 num_channels);

VOID iusace_tns_encode(ia_tns_info *pstr_tns_info_ch2, ia_tns_info *pstr_tns_info,
                       FLOAT32 *ptr_sfb_energy, WORD32 w, WORD32 i_ch, WORD32 low_pass_line,
                       FLOAT64 *ptr_scratch_tns_filter, WORD32 core_mode,
                       FLOAT64 *ptr_tns_scratch);
