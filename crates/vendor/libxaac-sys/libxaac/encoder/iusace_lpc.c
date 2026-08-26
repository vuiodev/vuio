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
#include "iusace_bitbuffer.h"
#include "iusace_cnst.h"
#include "iusace_tns_usac.h"
#include "iusace_psy_mod.h"
#include "iusace_block_switch_const.h"
#include "iusace_rom.h"

static FLOAT32 iusace_lpc_eval_chebyshev_polyn(FLOAT32 x, FLOAT32 *coefs, WORD32 order) {
  WORD32 i;
  FLOAT32 b0, b1, b2, x2;
  x2 = 2.0f * x;
  b2 = 1.0f;
  b1 = x2 + coefs[1];
  for (i = 2; i < order; i++) {
    b0 = x2 * b1 - b2 + coefs[i];
    b2 = b1;
    b1 = b0;
  }
  return (x * b1 - b2 + 0.5f * coefs[order]);
}

VOID iusace_lpc_2_lsp_conversion(FLOAT32 *lpc, FLOAT32 *lsp, FLOAT32 *prev_lsp) {
  FLOAT32 sum_polyn[(ORDER_BY_2) + 1], diff_polyn[(ORDER_BY_2) + 1];
  FLOAT32 *p1_lpc, *p2_lpc, *p_sum_polyn, *p_diff_polyn;
  WORD32 i, j = 0, num_found_freeq = 0, is_first_polyn = 0;
  FLOAT32 x_low, y_low, x_high, y_high, x_mid, y_mid, x_lin_interp;

  p_sum_polyn = sum_polyn;
  p_diff_polyn = diff_polyn;
  *p_sum_polyn++ = 1.0f;
  *p_diff_polyn++ = 1.0f;
  sum_polyn[0] = 1.0f;
  diff_polyn[0] = 1.0f;
  p1_lpc = lpc + 1;
  p2_lpc = lpc + ORDER;
  for (i = 0; i <= ORDER_BY_2 - 1; i++) {
    *p_sum_polyn = *p1_lpc + *p2_lpc - *(p_sum_polyn - 1);
    p_sum_polyn++;
    *p_diff_polyn = *p1_lpc++ - *p2_lpc-- + *(p_diff_polyn - 1);
    p_diff_polyn++;
  }
  p_sum_polyn = sum_polyn;
  x_low = iusace_chebyshev_polyn_grid[0];
  y_low = iusace_lpc_eval_chebyshev_polyn(x_low, p_sum_polyn, ORDER_BY_2);

  while ((num_found_freeq < ORDER) && (j < CHEBYSHEV_NUM_POINTS)) {
    j++;
    x_high = x_low;
    y_high = y_low;
    x_low = iusace_chebyshev_polyn_grid[j];
    y_low = iusace_lpc_eval_chebyshev_polyn(x_low, p_sum_polyn, ORDER_BY_2);

    if (y_low * y_high <= 0.0) /* if sign change new root exists */
    {
      j--;
      for (i = 0; i < CHEBYSHEV_NUM_ITER; i++) {
        x_mid = 0.5f * (x_low + x_high);
        y_mid = iusace_lpc_eval_chebyshev_polyn(x_mid, p_sum_polyn, ORDER_BY_2);
        if (y_low * y_mid <= 0.0) {
          y_high = y_mid;
          x_high = x_mid;
        } else {
          y_low = y_mid;
          x_low = x_mid;
        }
      }

      /* linear interpolation for evaluating the root */
      x_lin_interp = x_low - y_low * (x_high - x_low) / (y_high - y_low);

      lsp[num_found_freeq] = x_lin_interp;
      num_found_freeq++;

      is_first_polyn = 1 - is_first_polyn;
      p_sum_polyn = is_first_polyn ? diff_polyn : sum_polyn;

      x_low = x_lin_interp;
      y_low = iusace_lpc_eval_chebyshev_polyn(x_low, p_sum_polyn, ORDER_BY_2);
    }
  }

  /* Check if ORDER roots found */
  /* if not use the LSPs from previous frame */
  if (num_found_freeq < ORDER) {
    for (i = 0; i < ORDER; i++) lsp[i] = prev_lsp[i];
  }
}

static VOID iusace_compute_coeff_poly_f(FLOAT32 *lsp, FLOAT32 *poly1, FLOAT32 *poly2) {
  FLOAT32 b1, b2;
  FLOAT32 *ptr_lsp;
  WORD32 i, j;

  ptr_lsp = lsp;
  poly1[0] = poly2[0] = 1.0f;

  for (i = 1; i <= ORDER_BY_2; i++) {
    b1 = -2.0f * (*ptr_lsp++);
    b2 = -2.0f * (*ptr_lsp++);
    poly1[i] = (b1 * poly1[i - 1]) + (2.0f * poly1[i - 2]);
    poly2[i] = (b2 * poly2[i - 1]) + (2.0f * poly2[i - 2]);
    for (j = i - 1; j > 0; j--) {
      poly1[j] += (b1 * poly1[j - 1]) + poly1[j - 2];
      poly2[j] += (b2 * poly2[j - 1]) + poly2[j - 2];
    }
  }
}

VOID iusace_lsp_to_lp_conversion(FLOAT32 *lsp, FLOAT32 *lp_flt_coff_a) {
  WORD32 i;
  FLOAT32 *ppoly_f1, *ppoly_f2;
  FLOAT32 *plp_flt_coff_a_bott, *plp_flt_coff_a_top;
  FLOAT32 poly1[ORDER_BY_2 + 2], poly2[ORDER_BY_2 + 2];

  poly1[0] = 0.0f;
  poly2[0] = 0.0f;

  iusace_compute_coeff_poly_f(lsp, &poly1[1], &poly2[1]);

  ppoly_f1 = poly1 + ORDER_BY_2 + 1;
  ppoly_f2 = poly2 + ORDER_BY_2 + 1;

  for (i = 0; i < ORDER_BY_2; i++) {
    ppoly_f1[0] += ppoly_f1[-1];
    ppoly_f2[0] -= ppoly_f2[-1];
    ppoly_f1--;
    ppoly_f2--;
  }

  plp_flt_coff_a_bott = lp_flt_coff_a;
  *plp_flt_coff_a_bott++ = 1.0f;
  plp_flt_coff_a_top = lp_flt_coff_a + ORDER;
  ppoly_f1 = poly1 + 2;
  ppoly_f2 = poly2 + 2;
  for (i = 0; i < ORDER_BY_2; i++) {
    *plp_flt_coff_a_bott++ = 0.5f * (*ppoly_f1 + *ppoly_f2);
    *plp_flt_coff_a_top-- = 0.5f * (*ppoly_f1++ - *ppoly_f2++);
  }
}

VOID iusace_levinson_durbin_algo(FLOAT32 *auto_corr_input, FLOAT32 *lpc) {
  WORD32 i, j;
  FLOAT32 lpc_val, sum, sigma;
  FLOAT32 reflection_coeffs[LEV_DUR_MAX_ORDER];

  lpc[0] = 1.0f;

  reflection_coeffs[0] = -auto_corr_input[1] / auto_corr_input[0];
  lpc[1] = reflection_coeffs[0];
  sigma = auto_corr_input[0] + auto_corr_input[1] * reflection_coeffs[0];

  for (i = 2; i <= ORDER; i++) {
    sum = 0.0f;
    for (j = 0; j < i; j++) sum += auto_corr_input[i - j] * lpc[j];
    reflection_coeffs[i - 1] = -sum / sigma;

    sigma = sigma * (1.0f - reflection_coeffs[i - 1] * reflection_coeffs[i - 1]);

    if (sigma <= 1.0E-09f) {
      for (j = i; j <= ORDER; j++) {
        reflection_coeffs[j - 1] = 0.0f;
        lpc[j] = 0.0f;
      }
      break;
    }

    for (j = 1; j <= (i / 2); j++) {
      lpc_val = lpc[j] + reflection_coeffs[i - 1] * lpc[i - j];
      lpc[i - j] += reflection_coeffs[i - 1] * lpc[j];
      lpc[j] = lpc_val;
    }

    lpc[i] = reflection_coeffs[i - 1];
  }
}

VOID iusace_get_weighted_lpc(FLOAT32 *lpc, FLOAT32 *weighted_lpc) {
  WORD32 i;
  for (i = 0; i <= ORDER; i++) {
    weighted_lpc[i] = iusace_gamma_table[i] * lpc[i];
  }
}

VOID iusace_lsp_2_lsf_conversion(FLOAT32 *lsp, FLOAT32 *lsf) {
  WORD32 i;
  for (i = 0; i < ORDER; i++) {
    lsf[i] = (FLOAT32)(acos(lsp[i]) * LSP_2_LSF_SCALE);
  }
}

VOID iusace_lsf_2_lsp_conversion(FLOAT32 *lsf, FLOAT32 *lsp) {
  WORD32 i;
  for (i = 0; i < ORDER; i++) lsp[i] = (FLOAT32)cos((FLOAT64)lsf[i] * (FLOAT64)PI_BY_6400);
}
