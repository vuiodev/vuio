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
#include "ixheaace_error_codes.h"
#include "ixheaac_error_standards.h"
#include "ixheaace_mps_common_fix.h"
#include "ixheaace_mps_defines.h"
#include "ixheaace_mps_common_define.h"
#include "ixheaace_bitbuffer.h"

#include "ixheaace_mps_struct_def.h"
#include "ixheaace_mps_sac_polyphase.h"
#include "ixheaace_mps_sac_hybfilter.h"
#include "ixheaace_mps_spatial_bitstream.h"
#include "ixheaace_mps_bitstream.h"
#include "ixheaace_mps_param_extract.h"
#include "ixheaace_mps_tree.h"
#include "ixheaace_mps_rom.h"
#include "ixheaace_mps_sac_hybfilter.h"

static IA_ERRORCODE ixheaace_mps_cfftn_process(FLOAT32 *ptr_real, FLOAT32 *ptr_imag,
                                               WORD32 n_total, WORD32 n_pass, WORD32 n_span,
                                               WORD32 i_sign) {
  IA_ERRORCODE error = IA_NO_ERROR;
  WORD32 repeat = 0;
  FLOAT64 sine_60 = SIN_60;
  FLOAT64 cos_72 = COS_72;
  FLOAT64 sin_72 = SIN_72;
  FLOAT64 pi2 = M_PI;
  WORD32 ii, mfactor, kspan, ispan, inc;
  WORD32 j, jc, jf, jj, k, k1, k2, k3 = 0, k4, kk, kt, nn, ns, nt;

  FLOAT64 radf;
  FLOAT64 c1, c2 = 0.0, c3 = 0.0, cd;
  FLOAT64 s1, s2 = 0.0, s3 = 0.0, sd;
  FLOAT32 temp_real, temp_imag, temp_real_1, temp_imag_1, temp_real_2, temp_imag_2, temp_real_3,
      temp_imag_3, temp_real_4, temp_imag_4, aj, bj, aa, bb;

  FLOAT32 temp_real_array[MAX_FACTORS] = {0};
  FLOAT32 temp_imag_array[MAX_FACTORS] = {0};
  FLOAT64 cos_array[MAX_FACTORS] = {0};
  FLOAT64 sin_array[MAX_FACTORS] = {0};

  WORD32 perm[MAX_PERM] = {0};
  WORD32 factor[NFACTOR] = {0};
  if (n_pass < 2) {
    return IA_EXHEAACE_EXE_FATAL_MPS_CFFT_PROCESS;
  }

  ptr_real--;
  ptr_imag--;
  inc = i_sign;
  if (i_sign < 0) {
    sin_72 = -sin_72;
    sine_60 = -sine_60;
    pi2 = -pi2;
    inc = -inc;
  }
  ii = 0;
  jf = 0;
  mfactor = 0;
  j = 3;

  nt = inc * n_total;
  ns = inc * n_span;
  kspan = ns;
  nn = nt - inc;
  jc = ns / n_pass;
  radf = pi2 * (FLOAT64)jc;
  pi2 *= 2.0;
  k = n_pass;

  while (k % 16 == 0) {
    mfactor++;
    factor[mfactor - 1] = 4;
    k /= 16;
  }

  jj = 9;
  do {
    while (k % jj == 0) {
      mfactor++;
      factor[mfactor - 1] = j;
      k /= jj;
    }
    j += 2;
    jj = j * j;
  } while (jj <= k);

  if (k > 4) {
    if (k - (k / 4 << 2) == 0) {
      mfactor++;
      factor[mfactor - 1] = 2;
      k /= 4;
    }

    kt = mfactor;
    j = 2;

    do {
      if (k % j == 0) {
        mfactor++;
        factor[mfactor - 1] = j;
        k /= j;
      }
      j = ((j + 1) / 2 << 1) + 1;
    } while (j <= k);
  } else {
    kt = mfactor;
    factor[mfactor] = k;
    if (k != 1) {
      mfactor++;
    }
  }

  if (kt) {
    j = kt;
    do {
      mfactor++;
      factor[mfactor - 1] = factor[j - 1];
      j--;
    } while (j);
  }

  if (mfactor > NFACTOR) {
    return IA_EXHEAACE_EXE_FATAL_MPS_CFFT_PROCESS;
  }

  do {
    sd = radf / (FLOAT64)kspan;
    cd = sin(sd);
    cd = 2.0 * cd * cd;
    sd = sin(sd + sd);
    kk = 1;
    ii++;
    switch (factor[ii - 1]) {
      case 4:
        ispan = kspan;
        (VOID) ispan;
        kspan /= 4;

        do {
          c1 = 1.0;
          s1 = 0.0;
          do {
            do {
              k1 = kk + kspan;
              k2 = k1 + kspan;
              k3 = k2 + kspan;

              temp_real_1 = ptr_real[kk] + ptr_real[k2];
              temp_real_4 = ptr_real[kk] - ptr_real[k2];
              temp_real_2 = ptr_real[k1] + ptr_real[k3];
              temp_real_3 = ptr_real[k1] - ptr_real[k3];
              temp_imag_1 = ptr_imag[kk] + ptr_imag[k2];
              temp_imag_4 = ptr_imag[kk] - ptr_imag[k2];
              temp_imag_2 = ptr_imag[k1] + ptr_imag[k3];
              temp_imag_3 = ptr_imag[k1] - ptr_imag[k3];

              ptr_real[kk] = temp_real_1 + temp_real_2;
              ptr_imag[kk] = temp_imag_1 + temp_imag_2;

              temp_real_2 = temp_real_1 - temp_real_2;
              temp_imag_2 = temp_imag_1 - temp_imag_2;

              if (i_sign >= 0) {
                temp_real_1 = temp_real_4 - temp_imag_3;
                temp_imag_1 = temp_imag_4 + temp_real_3;
                temp_real_4 += temp_imag_3;
                temp_imag_4 -= temp_real_3;
              } else {
                temp_real_1 = temp_real_4 + temp_imag_3;
                temp_imag_1 = temp_imag_4 - temp_real_3;
                temp_real_4 -= temp_imag_3;
                temp_imag_4 += temp_real_3;
              }

              if (s1 == 0.0) {
                ptr_real[k1] = temp_real_1;
                ptr_real[k2] = temp_real_2;
                ptr_real[k3] = temp_real_4;
                ptr_imag[k1] = temp_imag_1;
                ptr_imag[k2] = temp_imag_2;
                ptr_imag[k3] = temp_imag_4;
              } else {
                ptr_real[k1] = (FLOAT32)(temp_real_1 * c1 - temp_imag_1 * s1);
                ptr_real[k2] = (FLOAT32)(temp_real_2 * c2 - temp_imag_2 * s2);
                ptr_real[k3] = (FLOAT32)(temp_real_4 * c3 - temp_imag_4 * s3);

                ptr_imag[k1] = (FLOAT32)(temp_real_1 * s1 + temp_imag_1 * c1);
                ptr_imag[k2] = (FLOAT32)(temp_real_2 * s2 + temp_imag_2 * c2);
                ptr_imag[k3] = (FLOAT32)(temp_real_4 * s3 + temp_imag_4 * c3);
              }

              kk = k3 + kspan;
            } while (kk <= nt);

            c2 = c1 - (cd * c1 + sd * s1);
            s1 = sd * c1 - cd * s1 + s1;

            c1 = 2.0 - (c2 * c2 + s1 * s1);
            s1 *= c1;
            c1 *= c2;

            c2 = c1 * c1 - s1 * s1;
            s2 = 2.0 * c1 * s1;

            c3 = c2 * c1 - s2 * s1;
            s3 = c2 * s1 + s2 * c1;

            kk = kk - nt + jc;
          } while (kk <= kspan);
          kk = kk - kspan + inc;

        } while (kk <= jc);
        if (kspan == jc) {
          repeat = 1;
          break;
        }
        break;
      case 2:
        kspan /= 2;
        k1 = kspan + 2;
        do {
          do {
            k2 = kk + kspan;
            temp_real = ptr_real[k2];
            temp_imag = ptr_imag[k2];
            ptr_real[k2] = ptr_real[kk] - temp_real;
            ptr_imag[k2] = ptr_imag[kk] - temp_imag;
            ptr_real[kk] += temp_real;
            ptr_imag[kk] += temp_imag;
            kk = k2 + kspan;
          } while (kk <= nn);

          kk -= nn;
        } while (kk <= jc);

        if (kk > kspan) {
          repeat = 1;
          break;
        }

        do {
          c1 = 1.0 - cd;
          s1 = sd;
          do {
            do {
              do {
                k2 = kk + kspan;
                temp_real = ptr_real[kk] - ptr_real[k2];
                temp_imag = ptr_imag[kk] - ptr_imag[k2];

                ptr_real[kk] += ptr_real[k2];
                ptr_imag[kk] += ptr_imag[k2];

                ptr_real[k2] = (FLOAT32)(c1 * temp_real - s1 * temp_imag);
                ptr_imag[k2] = (FLOAT32)(s1 * temp_real + c1 * temp_imag);

                kk = k2 + kspan;
              } while (kk < nt);

              k2 = kk - nt;
              c1 = -c1;
              kk = k1 - k2;
            } while (kk > k2);

            temp_real = (FLOAT32)(c1 - (cd * c1 + sd * s1));
            s1 = sd * c1 - cd * s1 + s1;
            c1 = 2.0 - (temp_real * temp_real + s1 * s1);

            s1 *= c1;
            c1 *= temp_real;
            kk += jc;
          } while (kk < k2);

          k1 += inc + inc;
          kk = (k1 - kspan) / 2 + jc;
        } while (kk <= jc + jc);
        break;
      default:
        k = factor[ii - 1];
        ispan = kspan;
        if (k != 0) kspan /= k;

        switch (k) {
          case 5:
            c2 = cos_72 * cos_72 - sin_72 * sin_72;
            s2 = 2.0 * cos_72 * sin_72;
            do {
              do {
                k1 = kk + kspan;
                k2 = k1 + kspan;
                k3 = k2 + kspan;
                k4 = k3 + kspan;

                temp_real_1 = ptr_real[k1] + ptr_real[k4];
                temp_real_4 = ptr_real[k1] - ptr_real[k4];
                temp_imag_1 = ptr_imag[k1] + ptr_imag[k4];
                temp_imag_4 = ptr_imag[k1] - ptr_imag[k4];
                temp_real_2 = ptr_real[k2] + ptr_real[k3];
                temp_real_3 = ptr_real[k2] - ptr_real[k3];
                temp_imag_2 = ptr_imag[k2] + ptr_imag[k3];
                temp_imag_3 = ptr_imag[k2] - ptr_imag[k3];

                aa = ptr_real[kk];
                bb = ptr_imag[kk];
                ptr_real[kk] = aa + temp_real_1 + temp_real_2;
                ptr_imag[kk] = bb + temp_imag_1 + temp_imag_2;

                temp_real = (FLOAT32)(temp_real_1 * cos_72 + temp_real_2 * c2 + aa);
                temp_imag = (FLOAT32)(temp_imag_1 * cos_72 + temp_imag_2 * c2 + bb);

                aj = (FLOAT32)(temp_real_4 * sin_72 + temp_real_3 * s2);
                bj = (FLOAT32)(temp_imag_4 * sin_72 + temp_imag_3 * s2);

                ptr_real[k1] = temp_real - bj;
                ptr_real[k4] = temp_real + bj;
                ptr_imag[k1] = temp_imag + aj;
                ptr_imag[k4] = temp_imag - aj;

                temp_real = (FLOAT32)(temp_real_1 * c2 + temp_real_2 * cos_72 + aa);
                temp_imag = (FLOAT32)(temp_imag_1 * c2 + temp_imag_2 * cos_72 + bb);

                aj = (FLOAT32)(temp_real_4 * s2 - temp_real_3 * sin_72);
                bj = (FLOAT32)(temp_imag_4 * s2 - temp_imag_3 * sin_72);

                ptr_real[k2] = temp_real - bj;
                ptr_real[k3] = temp_real + bj;
                ptr_imag[k2] = temp_imag + aj;
                ptr_imag[k3] = temp_imag - aj;

                kk = k4 + kspan;
              } while (kk < nn);
              kk -= nn;
            } while (kk <= kspan);
            break;
          case 3:
            do {
              do {
                k1 = kk + kspan;
                k2 = k1 + kspan;

                temp_real = ptr_real[kk];
                temp_imag = ptr_imag[kk];

                aj = ptr_real[k1] + ptr_real[k2];
                bj = ptr_imag[k1] + ptr_imag[k2];

                ptr_real[kk] = temp_real + aj;
                ptr_imag[kk] = temp_imag + bj;

                temp_real -= 0.5f * aj;
                temp_imag -= 0.5f * bj;

                aj = (FLOAT32)((ptr_real[k1] - ptr_real[k2]) * sine_60);
                bj = (FLOAT32)((ptr_imag[k1] - ptr_imag[k2]) * sine_60);

                ptr_real[k1] = temp_real - bj;
                ptr_real[k2] = temp_real + bj;
                ptr_imag[k1] = temp_imag + aj;
                ptr_imag[k2] = temp_imag - aj;

                kk = k2 + kspan;
              } while (kk < nn);
              kk -= nn;
            } while (kk <= kspan);
            break;

          default:
            if (k != jf) {
              jf = k;
              s1 = pi2 / (FLOAT64)k;

              c1 = cos(s1);
              s1 = sin(s1);

              if (jf > MAX_FACTORS) {
                return IA_EXHEAACE_EXE_FATAL_MPS_CFFT_PROCESS;
              }

              cos_array[jf - 1] = 1.0;
              sin_array[jf - 1] = 0.0;
              j = 1;
              do {
                cos_array[j - 1] = cos_array[k - 1] * c1 + sin_array[k - 1] * s1;
                sin_array[j - 1] = cos_array[k - 1] * s1 - sin_array[k - 1] * c1;

                k--;
                cos_array[k - 1] = cos_array[j - 1];
                sin_array[k - 1] = -sin_array[j - 1];

                j++;
              } while (j < k);
            }
            do {
              do {
                k1 = kk;
                k2 = kk + ispan;

                temp_real = aa = ptr_real[kk];
                temp_imag = bb = ptr_imag[kk];

                j = 1;
                k1 += kspan;
                do {
                  k2 -= kspan;
                  j++;

                  temp_real_array[j - 1] = ptr_real[k1] + ptr_real[k2];
                  temp_real += temp_real_array[j - 1];
                  temp_imag_array[j - 1] = ptr_imag[k1] + ptr_imag[k2];
                  temp_imag += temp_imag_array[j - 1];
                  j++;

                  temp_real_array[j - 1] = ptr_real[k1] - ptr_real[k2];
                  temp_imag_array[j - 1] = ptr_imag[k1] - ptr_imag[k2];
                  k1 += kspan;

                } while (k1 < k2);

                ptr_real[kk] = temp_real;
                ptr_imag[kk] = temp_imag;
                k1 = kk;
                k2 = kk + ispan;
                j = 1;

                do {
                  k1 += kspan;
                  k2 -= kspan;

                  jj = j;
                  temp_real = aa;
                  temp_imag = bb;
                  aj = 0.0;
                  bj = 0.0;
                  k = 1;

                  do {
                    k++;
                    temp_real += (FLOAT32)(temp_real_array[k - 1] * cos_array[jj - 1]);
                    temp_imag += (FLOAT32)(temp_imag_array[k - 1] * cos_array[jj - 1]);

                    k++;
                    aj += (FLOAT32)(temp_real_array[k - 1] * sin_array[jj - 1]);
                    bj += (FLOAT32)(temp_imag_array[k - 1] * sin_array[jj - 1]);

                    jj += j;
                    if (jj > jf) {
                      jj -= jf;
                    }
                  } while (k < jf);

                  k = jf - j;
                  ptr_real[k1] = temp_real - bj;
                  ptr_imag[k1] = temp_imag + aj;
                  ptr_real[k2] = temp_real + bj;
                  ptr_imag[k2] = temp_imag - aj;

                  j++;
                } while (j < k);
                kk += ispan;

              } while (kk <= nn);
              kk -= nn;

            } while (kk <= kspan);
            break;
        }

        if (ii == mfactor) {
          repeat = 1;
          break;
        }

        kk = jc + 1;

        do {
          c2 = 1.0 - cd;
          s1 = sd;

          do {
            c1 = c2;
            s2 = s1;
            kk += kspan;
            do {
              do {
                temp_real = ptr_real[kk];
                ptr_real[kk] = (FLOAT32)(c2 * temp_real - s2 * ptr_imag[kk]);
                ptr_imag[kk] = (FLOAT32)(s2 * temp_real + c2 * ptr_imag[kk]);
                kk += ispan;

              } while (kk <= nt);

              temp_real = (FLOAT32)(s1 * s2);
              s2 = s1 * c2 + c1 * s2;
              c2 = c1 * c2 - temp_real;
              kk = kk - nt + kspan;

            } while (kk <= ispan);

            c2 = c1 - (cd * c1 + sd * s1);
            s1 += sd * c1 - cd * s1;
            c1 = 2.0 - (c2 * c2 + s1 * s1);

            s1 *= c1;
            c2 *= c1;

            kk = kk - ispan + jc;
          } while (kk <= kspan);

          kk = kk - kspan + jc + inc;

        } while (kk <= jc + jc);
        break;
    }
  } while (repeat == 0);

  perm[0] = ns;

  if (kt) {
    k = kt + kt + 1;
    if (mfactor < k) {
      k--;
    }

    j = 1;
    perm[k] = jc;

    do {
      perm[j] = perm[j - 1] / factor[j - 1];
      perm[k - 1] = perm[k] * factor[j - 1];

      j++;
      k--;
    } while (j < k);

    k3 = perm[k];
    kspan = perm[1];
    kk = jc + 1;
    k2 = kspan + 1;

    j = 1;

    if (n_pass == n_total) {
      do {
        do {
          temp_real = ptr_real[kk];
          ptr_real[kk] = ptr_real[k2];
          ptr_real[k2] = temp_real;
          temp_imag = ptr_imag[kk];
          ptr_imag[kk] = ptr_imag[k2];
          ptr_imag[k2] = temp_imag;

          kk += inc;
          k2 += kspan;

        } while (k2 < ns);
        do {
          do {
            k2 -= perm[j - 1];
            j++;
            k2 = perm[j] + k2;

          } while (k2 > perm[j - 1]);

          j = 1;
          do {
            if (kk < k2) {
              repeat = 1;
              break;
            } else {
              repeat = 0;
            }

            kk += inc;
            k2 += kspan;

          } while (k2 < ns);
          if (repeat) {
            break;
          }
        } while (kk < ns);
      } while (repeat);
    } else {
      do {
        do {
          do {
            k = kk + jc;
            do {
              temp_real = ptr_real[kk];
              ptr_real[kk] = ptr_real[k2];
              ptr_real[k2] = temp_real;
              temp_imag = ptr_imag[kk];
              ptr_imag[kk] = ptr_imag[k2];
              ptr_imag[k2] = temp_imag;

              kk += inc;
              k2 += inc;

            } while (kk < k);
            kk += ns - jc;
            k2 += ns - jc;

          } while (kk < nt);

          k2 = k2 - nt + kspan;
          kk = kk - nt + jc;

        } while (k2 < ns);

        do {
          do {
            k2 -= perm[j - 1];
            j++;
            k2 = perm[j] + k2;
          } while (k2 > perm[j - 1]);

          j = 1;
          do {
            if (kk >= k2) {
              repeat = 0;
            } else {
              repeat = 1;
              break;
            }

            kk += jc;
            k2 += kspan;

          } while (k2 < ns);
          if (repeat) {
            break;
          }
        } while (kk < ns);
      } while (repeat);
    }
    jc = k3;
  }

  if ((kt << 1) + 1 >= mfactor) {
    return IA_NO_ERROR;
  }

  ispan = perm[kt];
  j = mfactor - kt;
  factor[j] = 1;

  do {
    factor[j - 1] *= factor[j];
    j--;
  } while (j != kt);

  kt++;
  nn = factor[kt - 1] - 1;

  if (nn > MAX_PERM) {
    return IA_EXHEAACE_EXE_FATAL_MPS_CFFT_PROCESS;
  }

  for (j = jj = 0; j <= nn; j++) {
    k = kt + 1;
    k2 = factor[kt - 1];
    kk = factor[k - 1];

    for (jj += kk; jj >= k2;) {
      jj -= k2;
      k2 = kk;
      k++;
      kk = factor[k - 1];
      jj += kk;
    }

    perm[j - 1] = jj;
  }

  for (j = 0;;) {
    do {
      j++;
      kk = perm[j - 1];

    } while (kk < 0);

    if (kk == j) {
      perm[j - 1] = -j;
      if (j == nn) {
        break;
      }
    } else {
      do {
        k = kk;
        kk = perm[k - 1];
        perm[k - 1] = -kk;
      } while (kk != j);

      k3 = kk;
    }
  }
  for (;;) {
    j = k3 + 1;
    nt -= ispan;
    ii = nt - inc + 1;
    if (nt < 0) {
      break;
    }
    do {
      do {
        j--;
      } while (perm[j - 1] < 0);

      jj = jc;

      do {
        kspan = jj;
        if (jj > MAX_FACTORS * inc) {
          kspan = MAX_FACTORS * inc;
        }

        jj -= kspan;
        k = perm[j - 1];
        kk = jc * k + ii + jj;
        k1 = kk + kspan;
        k2 = 0;

        do {
          k2++;
          temp_real_array[k2 - 1] = ptr_real[k1];
          temp_imag_array[k2 - 1] = ptr_imag[k1];
          k1 -= inc;
        } while (k1 != kk);

        do {
          k1 = kk + kspan;
          k2 = k1 - jc * (k + perm[k - 1]);
          k = -perm[k - 1];

          do {
            ptr_real[k1] = ptr_real[k2];
            ptr_imag[k1] = ptr_imag[k2];

            k1 -= inc;
            k2 -= inc;

          } while (k1 != kk);
          kk = k2;

        } while (k != j);

        k1 = kk + kspan;
        k2 = 0;

        do {
          k2++;
          ptr_real[k1] = temp_real_array[k2 - 1];
          ptr_imag[k1] = temp_imag_array[k2 - 1];
          k1 -= inc;

        } while (k1 != kk);
      } while (jj);
    } while (j != 1);
  }
  (VOID) ispan;
  return error;
}

static IA_ERRORCODE ixheaace_mps_eight_channel_filtering(const FLOAT32 *ptr_qmf_real,
                                                         const FLOAT32 *ptr_qmf_imag,
                                                         FLOAT32 *ptr_hybrid_real,
                                                         FLOAT32 *ptr_hybrid_imag) {
  IA_ERRORCODE error = IA_NO_ERROR;
  WORD32 idx;
  FLOAT32 ptr_real, ptr_imag;
  FLOAT32 cum[16];

  cum[0] = p8_13[HYBRID_FILTER_DELAY] * ptr_qmf_real[HYBRID_FILTER_DELAY];
  cum[1] = p8_13[HYBRID_FILTER_DELAY] * ptr_qmf_imag[HYBRID_FILTER_DELAY];

  ptr_real = p8_13[5] * ptr_qmf_real[5];
  ptr_imag = p8_13[5] * ptr_qmf_imag[5];
  cum[2] = ptr_real * COS_22_5 - ptr_imag * SIN_22_5;
  cum[3] = ptr_real * COS_67_5 + ptr_imag * SIN_67_5;

  ptr_real = p8_13[4] * ptr_qmf_real[4] + p8_13[12] * ptr_qmf_real[12];
  ptr_imag = p8_13[4] * ptr_qmf_imag[4] + p8_13[12] * ptr_qmf_imag[12];
  cum[4] = (ptr_imag - ptr_real) * INV_SQRT_2;
  cum[5] = -(ptr_imag + ptr_real) * INV_SQRT_2;

  ptr_real = p8_13[3] * ptr_qmf_real[3] + p8_13[11] * ptr_qmf_real[11];
  ptr_imag = p8_13[3] * ptr_qmf_imag[3] + p8_13[11] * ptr_qmf_imag[11];
  cum[6] = ptr_imag * SIN_67_5 - ptr_real * COS_67_5;
  cum[7] = -(ptr_imag * SIN_22_5 + ptr_real * COS_22_5);
  cum[9] = -(p8_13[2] * ptr_qmf_real[2] + p8_13[10] * ptr_qmf_real[10]);
  cum[8] = p8_13[2] * ptr_qmf_imag[2] + p8_13[10] * ptr_qmf_imag[10];

  ptr_real = p8_13[1] * ptr_qmf_real[1] + p8_13[9] * ptr_qmf_real[9];
  ptr_imag = p8_13[1] * ptr_qmf_imag[1] + p8_13[9] * ptr_qmf_imag[9];
  cum[10] = ptr_imag * SIN_67_5 + ptr_real * COS_67_5;
  cum[11] = ptr_imag * SIN_22_5 - ptr_real * COS_22_5;

  ptr_real = p8_13[0] * ptr_qmf_real[0] + p8_13[8] * ptr_qmf_real[8];
  ptr_imag = p8_13[0] * ptr_qmf_imag[0] + p8_13[8] * ptr_qmf_imag[8];
  cum[12] = (ptr_imag + ptr_real) * INV_SQRT_2;
  cum[13] = (ptr_imag - ptr_real) * INV_SQRT_2;

  ptr_real = p8_13[7] * ptr_qmf_real[7];
  ptr_imag = p8_13[7] * ptr_qmf_imag[7];
  cum[14] = ptr_imag * SIN_22_5 + ptr_real * COS_22_5;
  cum[15] = ptr_imag * SIN_67_5 - ptr_real * COS_67_5;

  error = ixheaace_mps_cfftn_process(&cum[0], &cum[1], 8, 8, 8, 2);
  if (error) {
    return error;
  }
  for (idx = 0; idx < 8; idx++) {
    ptr_hybrid_real[idx] = cum[2 * idx];
    ptr_hybrid_imag[idx] = cum[2 * idx + 1];
  }
  return error;
}

static VOID ixheaace_mps_two_channel_filtering(const FLOAT32 *p_qmf, FLOAT32 *ptr_hybrid) {
  WORD32 n;
  FLOAT32 cum0, cum1;

  cum0 = 0.5f * p_qmf[HYBRID_FILTER_DELAY];

  cum1 = 0;

  for (n = 0; n < 6; n++) {
    cum1 += p2_6[n] * p_qmf[2 * n + 1];
  }

  ptr_hybrid[0] = cum0 + cum1;
  ptr_hybrid[1] = cum0 - cum1;
}

VOID ixheaace_mps_515_apply_ana_hyb_filterbank(
    ixheaace_mps_pstr_hyb_filter_state pstr_hyb_filter_state, FLOAT32 *ptr_qmf_real,
    FLOAT32 *ptr_qmf_imag, WORD32 nr_samples, FLOAT32 *ptr_hybrid_real,
    FLOAT32 *ptr_hybrid_imag) {
  WORD32 nr_samples_shift_lf;
  WORD32 nr_qmf_bands_lf;
  WORD32 k, n;
  WORD32 time_slot, ch_offset;

  FLOAT32 m_temp_output_real[MAX_HYBRID_ONLY_BANDS_PER_QMF] = {0};
  FLOAT32 m_temp_output_imag[MAX_HYBRID_ONLY_BANDS_PER_QMF] = {0};

  nr_samples_shift_lf = BUFFER_LEN_LF - nr_samples;

  nr_qmf_bands_lf = NUM_QMF_BANDS_TO_LF;
  for (k = 0; k < nr_qmf_bands_lf; k++) {
    for (n = 0; n < nr_samples_shift_lf; n++) {
      pstr_hyb_filter_state->buffer_lf_real[k][n] =
          pstr_hyb_filter_state->buffer_lf_real[k][n + nr_samples];
      pstr_hyb_filter_state->buffer_lf_imag[k][n] =
          pstr_hyb_filter_state->buffer_lf_imag[k][n + nr_samples];
    }
  }

  for (k = 0; k < nr_qmf_bands_lf; k++) {
    for (n = 0; n < nr_samples; n++) {
      pstr_hyb_filter_state->buffer_lf_real[k][n + nr_samples_shift_lf] =
          ptr_qmf_real[n * NUM_QMF_BANDS + k];
      pstr_hyb_filter_state->buffer_lf_imag[k][n + nr_samples_shift_lf] =
          ptr_qmf_imag[n * NUM_QMF_BANDS + k];
    }
  }
  for (k = 0; k < NUM_QMF_BANDS - nr_qmf_bands_lf; k++) {
    for (n = 0; n < BUFFER_LEN_HF; n++) {
      ptr_hybrid_real[n * MAX_HYBRID_BANDS + k + 10] =
          pstr_hyb_filter_state->buffer_hf_real[k][n];
      ptr_hybrid_imag[n * MAX_HYBRID_BANDS + k + 10] =
          pstr_hyb_filter_state->buffer_hf_imag[k][n];

      pstr_hyb_filter_state->buffer_hf_real[k][n] =
          ptr_qmf_real[(nr_samples - BUFFER_LEN_HF + n) * NUM_QMF_BANDS + k + nr_qmf_bands_lf];
      pstr_hyb_filter_state->buffer_hf_imag[k][n] =
          ptr_qmf_imag[(nr_samples - BUFFER_LEN_HF + n) * NUM_QMF_BANDS + k + nr_qmf_bands_lf];
    }
  }

  for (k = 0; k < NUM_QMF_BANDS - nr_qmf_bands_lf; k++) {
    for (n = BUFFER_LEN_HF; n < nr_samples; n++) {
      ptr_hybrid_real[n * MAX_HYBRID_BANDS + k + 10] =
          ptr_qmf_real[(n - BUFFER_LEN_HF) * NUM_QMF_BANDS + k + nr_qmf_bands_lf];
      ptr_hybrid_imag[n * MAX_HYBRID_BANDS + k + 10] =
          ptr_qmf_imag[(n - BUFFER_LEN_HF) * NUM_QMF_BANDS + k + nr_qmf_bands_lf];
    }
  }

  for (time_slot = 0; time_slot < nr_samples; time_slot++) {
    ch_offset = 0;

    ixheaace_mps_eight_channel_filtering(
        (const FLOAT32 *)&(pstr_hyb_filter_state->buffer_lf_real[0][nr_samples_shift_lf + 1 -
                                                                    PROTO_LEN + time_slot]),
        (const FLOAT32 *)&(pstr_hyb_filter_state->buffer_lf_imag[0][nr_samples_shift_lf + 1 -
                                                                    PROTO_LEN + time_slot]),
        m_temp_output_real, m_temp_output_imag);

    ptr_hybrid_real[time_slot * MAX_HYBRID_BANDS + ch_offset + 0] = m_temp_output_real[6];
    ptr_hybrid_imag[time_slot * MAX_HYBRID_BANDS + ch_offset + 0] = m_temp_output_imag[6];
    ptr_hybrid_real[time_slot * MAX_HYBRID_BANDS + ch_offset + 1] = m_temp_output_real[7];
    ptr_hybrid_imag[time_slot * MAX_HYBRID_BANDS + ch_offset + 1] = m_temp_output_imag[7];
    ptr_hybrid_real[time_slot * MAX_HYBRID_BANDS + ch_offset + 2] = m_temp_output_real[0];
    ptr_hybrid_imag[time_slot * MAX_HYBRID_BANDS + ch_offset + 2] = m_temp_output_imag[0];
    ptr_hybrid_real[time_slot * MAX_HYBRID_BANDS + ch_offset + 3] = m_temp_output_real[1];
    ptr_hybrid_imag[time_slot * MAX_HYBRID_BANDS + ch_offset + 3] = m_temp_output_imag[1];
    ptr_hybrid_real[time_slot * MAX_HYBRID_BANDS + ch_offset + 4] =
        m_temp_output_real[2] + m_temp_output_real[5];
    ptr_hybrid_imag[time_slot * MAX_HYBRID_BANDS + ch_offset + 4] =
        m_temp_output_imag[2] + m_temp_output_imag[5];
    ptr_hybrid_real[time_slot * MAX_HYBRID_BANDS + ch_offset + 5] =
        m_temp_output_real[3] + m_temp_output_real[4];
    ptr_hybrid_imag[time_slot * MAX_HYBRID_BANDS + ch_offset + 5] =
        m_temp_output_imag[3] + m_temp_output_imag[4];

    ch_offset += 6;

    ixheaace_mps_two_channel_filtering(
        (const FLOAT32 *)&(pstr_hyb_filter_state->buffer_lf_real[1][nr_samples_shift_lf + 1 -
                                                                    PROTO_LEN + time_slot]),
        m_temp_output_real);
    ixheaace_mps_two_channel_filtering(
        (const FLOAT32 *)&(pstr_hyb_filter_state->buffer_lf_imag[1][nr_samples_shift_lf + 1 -
                                                                    PROTO_LEN + time_slot]),
        m_temp_output_imag);
    ptr_hybrid_real[time_slot * MAX_HYBRID_BANDS + ch_offset] = m_temp_output_real[1];
    ptr_hybrid_imag[time_slot * MAX_HYBRID_BANDS + ch_offset] = m_temp_output_imag[1];
    ptr_hybrid_real[time_slot * MAX_HYBRID_BANDS + ch_offset + 1] = m_temp_output_real[0];
    ptr_hybrid_imag[time_slot * MAX_HYBRID_BANDS + ch_offset + 1] = m_temp_output_imag[0];

    ch_offset += 2;

    ixheaace_mps_two_channel_filtering(
        (const FLOAT32 *)&(pstr_hyb_filter_state->buffer_lf_real[2][nr_samples_shift_lf + 1 -
                                                                    PROTO_LEN + time_slot]),
        m_temp_output_real);
    ixheaace_mps_two_channel_filtering(
        (const FLOAT32 *)&(pstr_hyb_filter_state->buffer_lf_imag[2][nr_samples_shift_lf + 1 -
                                                                    PROTO_LEN + time_slot]),
        m_temp_output_imag);

    ptr_hybrid_real[time_slot * MAX_HYBRID_BANDS + ch_offset] = m_temp_output_real[0];
    ptr_hybrid_imag[time_slot * MAX_HYBRID_BANDS + ch_offset] = m_temp_output_imag[0];
    ptr_hybrid_real[time_slot * MAX_HYBRID_BANDS + ch_offset + 1] = m_temp_output_real[1];
    ptr_hybrid_imag[time_slot * MAX_HYBRID_BANDS + ch_offset + 1] = m_temp_output_imag[1];
  }
}

VOID ixheaace_mps_515_apply_syn_hyb_filterbank(FLOAT32 *ptr_hybrid_real, FLOAT32 *ptr_hybrid_imag,
                                               WORD32 nr_samples, FLOAT32 *ptr_qmf_real,
                                               FLOAT32 *ptr_qmf_imag) {
  WORD32 k, n;

  for (n = 0; n < nr_samples; n++) {
    ptr_qmf_real[n * NUM_QMF_BANDS] = ptr_hybrid_real[n * MAX_HYBRID_BANDS];
    ptr_qmf_imag[n * NUM_QMF_BANDS] = ptr_hybrid_imag[n * MAX_HYBRID_BANDS];

    for (k = 1; k < 6; k++) {
      ptr_qmf_real[n * NUM_QMF_BANDS] += ptr_hybrid_real[n * MAX_HYBRID_BANDS + k];
      ptr_qmf_imag[n * NUM_QMF_BANDS] += ptr_hybrid_imag[n * MAX_HYBRID_BANDS + k];
    }

    ptr_qmf_real[n * NUM_QMF_BANDS + 1] =
        ptr_hybrid_real[n * MAX_HYBRID_BANDS + 6] + ptr_hybrid_real[n * MAX_HYBRID_BANDS + 7];
    ptr_qmf_imag[n * NUM_QMF_BANDS + 1] =
        ptr_hybrid_imag[n * MAX_HYBRID_BANDS + 6] + ptr_hybrid_imag[n * MAX_HYBRID_BANDS + 7];

    ptr_qmf_real[n * NUM_QMF_BANDS + 2] =
        ptr_hybrid_real[n * MAX_HYBRID_BANDS + 8] + ptr_hybrid_real[n * MAX_HYBRID_BANDS + 9];
    ptr_qmf_imag[n * NUM_QMF_BANDS + 2] =
        ptr_hybrid_imag[n * MAX_HYBRID_BANDS + 8] + ptr_hybrid_imag[n * MAX_HYBRID_BANDS + 9];

    for (k = 0; k < 61; k++) {
      ptr_qmf_real[n * NUM_QMF_BANDS + k + 3] = ptr_hybrid_real[n * MAX_HYBRID_BANDS + k + 10];
      ptr_qmf_imag[n * NUM_QMF_BANDS + k + 3] = ptr_hybrid_imag[n * MAX_HYBRID_BANDS + k + 10];
    }
  }
}
