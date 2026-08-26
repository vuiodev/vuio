#include <stdlib.h>
#include <stdio.h>

#include "ixheaac_type_def.h"
#include "ixheaacd_interface.h"
#include "ixheaac_constants.h"
#include "ixheaac_basic_ops32.h"
#include "ixheaacd_function_selector.h"

extern const WORD32 ixheaacd_twiddle_table_fft_32x32[514];
extern const WORD8 ixheaacd_mps_dig_rev[16];

VOID ixheaacd_complex_fft_p2_armv7(WORD32 *xr, WORD32 *xi, WORD32 nlength,
                                   WORD32 fft_mode, WORD32 *preshift) {
  WORD32 i, n_stages;
  WORD32 not_power_4;
  WORD32 npts, shift;
  WORD32 dig_rev_shift;
  WORD32 ptr_x[1024];
  WORD32 y[1024];
  WORD32 npoints = nlength;
  WORD32 n = 0;
  WORD32 *ptr_y = y;
  dig_rev_shift = ixheaac_norm32(npoints) + 1 - 16;
  n_stages = 30 - ixheaac_norm32(npoints);  // log2(npoints), if npoints=2^m
  not_power_4 = n_stages & 1;

  n_stages = n_stages >> 1;

  npts = npoints;  // CALCULATION OF GUARD BITS
  while (npts >> 1) {
    n++;
    npts = npts >> 1;
  }

  if (n % 2 == 0)
    shift = ((n + 4)) / 2;
  else
    shift = ((n + 3) / 2);

  for (i = 0; i < nlength; i++) {
    ptr_x[2 * i] = (xr[i] / (1 << (shift)));
    ptr_x[2 * i + 1] = (xi[i] / (1 << (shift)));
  }

  if (fft_mode == -1) {
    ixheaacd_complex_fft_p2_asm(ixheaacd_twiddle_table_fft_32x32, nlength,
                                ptr_x, ptr_y);
    if (not_power_4) shift += 1;
  }

  else {
    ixheaacd_complex_ifft_p2_asm(ixheaacd_twiddle_table_fft_32x32, nlength,
                                 ptr_x, ptr_y);
    if (not_power_4) shift += 1;
  }

  for (i = 0; i < nlength; i++) {
    xr[i] = y[2 * i];
    xi[i] = y[2 * i + 1];
  }

  *preshift = shift - *preshift;
  return;
}

VOID ixheaacd_mps_complex_fft_64_armv7(WORD32 *ptr_x, WORD32 *fin_re,
                                       WORD32 *fin_im, WORD32 nlength) {
  WORD32 i, n_stages;
  WORD32 y[128];
  WORD32 npoints = nlength;
  WORD32 *ptr_y = y;
  const WORD32 *ptr_w;
  n_stages = 30 - ixheaac_norm32(npoints);  // log2(npoints), if npoints=2^m

  n_stages = n_stages >> 1;

  ptr_w = ixheaacd_twiddle_table_fft_32x32;  // 32 BIT TWIDDLE TABLE

  ixheaacd_mps_complex_fft_64_asm(ptr_w, nlength, ptr_x, ptr_y,
                                  ixheaacd_mps_dig_rev);

  for (i = 0; i < 2 * nlength; i += 2) {
    fin_re[i] = y[i];
    fin_im[i] = y[i + 1];
  }

  return;
}
