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
#include "impd_drc_common_enc.h"
#include "impd_drc_uni_drc.h"
#include "impd_drc_tables.h"

const FLOAT32 impd_drc_downmix_coeff[16] = {0.0f,  -0.5f, -1.0f, -1.5f,   -2.0f, -2.5f,
                                            -3.0f, -3.5f, -4.0f, -4.5f,   -5.0f, -5.5f,
                                            -6.0f, -7.5f, -9.0f, -1000.0f};

const FLOAT32 impd_drc_downmix_coeff_lfe[16] = {10.0f,  6.0f,   4.5f,   3.0f,    1.5f,   0.0f,
                                                -1.5f,  -3.0f,  -4.5f,  -6.0f,   -10.0f, -15.0f,
                                                -20.0f, -30.0f, -40.0f, -1000.0f};

const FLOAT32 impd_drc_channel_weight[16] = {10.0f,  6.0f,   4.5f,   3.0f,    1.5f,   0.0f,
                                             -1.5f,  -3.0f,  -4.5f,  -6.0f,   -10.0f, -15.0f,
                                             -20.0f, -30.0f, -40.0f, -1000.0f};

const FLOAT32 impd_drc_downmix_coeff_v1[32] = {
    10.00f,  6.00f,   4.50f,   3.00f,   1.50f,   0.00f,   -0.50f,  -1.00f,
    -1.50f,  -2.00f,  -2.50f,  -3.00f,  -3.50f,  -4.00f,  -4.50f,  -5.00f,
    -5.50f,  -6.00f,  -6.50f,  -7.00f,  -7.50f,  -8.00f,  -9.00f,  -10.00f,
    -11.00f, -12.00f, -15.00f, -20.00f, -25.00f, -30.00f, -40.00f, -100000.0f};

const FLOAT32 impd_drc_eq_slope_table[16] = {-32.0f, -24.0f, -18.0f, -12.0f, -7.0f, -4.0f,
                                             -2.0f,  -1.0f,  1.0f,   2.0f,   4.0f,  7.0f,
                                             12.0f,  18.0f,  24.0f,  32.0f};

const FLOAT32 impd_drc_eq_gain_delta_table[32] = {
    -22.0f, -16.0f, -13.0f, -11.0f, -9.0f, -7.0f, -6.0f, -5.0f, -4.0f, -3.0f, -2.5f,
    -2.0f,  -1.5f,  -1.0f,  -0.5f,  0.0f,  0.5f,  1.0f,  1.5f,  2.0f,  2.5f,  3.0f,
    4.0f,   5.0f,   6.0f,   7.0f,   9.0f,  11.0f, 13.0f, 16.0f, 22.0f, 32.0f};

const FLOAT32 impd_drc_zero_pole_radius_table[128] = {
    0.00000000E+00f, 7.57409621E-11f, 7.47451079E-09f, 7.37623509E-08f, 3.37872933E-07f,
    1.05439995E-06f, 2.61370951E-06f, 5.55702854E-06f, 1.05878771E-05f, 1.85806475E-05f,
    3.05868707E-05f, 4.78395414E-05f, 7.17558214E-05f, 1.03938342E-04f, 1.46175269E-04f,
    2.00439375E-04f, 2.68886099E-04f, 3.53850890E-04f, 4.57845890E-04f, 5.83555840E-04f,
    7.33833469E-04f, 9.11694835E-04f, 1.12031354E-03f, 1.36301492E-03f, 1.64327072E-03f,
    1.96469179E-03f, 2.33102194E-03f, 2.74613220E-03f, 3.21401190E-03f, 3.73876374E-03f,
    4.32459544E-03f, 4.97581391E-03f, 5.69681637E-03f, 6.49208482E-03f, 7.36617809E-03f,
    8.32372531E-03f, 9.36941616E-03f, 1.05079999E-02f, 1.17442720E-02f, 1.30830696E-02f,
    1.45292655E-02f, 1.60877611E-02f, 1.77634824E-02f, 1.95613634E-02f, 2.14863531E-02f,
    2.35434026E-02f, 2.57374570E-02f, 2.80734543E-02f, 3.05563174E-02f, 3.31909470E-02f,
    3.59822176E-02f, 3.89349759E-02f, 4.20540236E-02f, 4.53441292E-02f, 4.88100089E-02f,
    5.24563305E-02f, 5.62877022E-02f, 6.03086725E-02f, 6.45237267E-02f, 6.89372867E-02f,
    7.35536888E-02f, 7.83772022E-02f, 8.34120139E-02f, 8.86622295E-02f, 9.41318572E-02f,
    9.98248383E-02f, 1.05744988E-01f, 1.11896060E-01f, 1.18281692E-01f, 1.24905407E-01f,
    1.31770656E-01f, 1.38880774E-01f, 1.46238968E-01f, 1.53848350E-01f, 1.61711931E-01f,
    1.69832602E-01f, 1.78213134E-01f, 1.86856180E-01f, 1.95764288E-01f, 2.04939872E-01f,
    2.14385241E-01f, 2.24102572E-01f, 2.34093949E-01f, 2.44361281E-01f, 2.54906416E-01f,
    2.65731007E-01f, 2.76836663E-01f, 2.88224846E-01f, 2.99896836E-01f, 3.11853856E-01f,
    3.24096978E-01f, 3.36627185E-01f, 3.49445283E-01f, 3.62551987E-01f, 3.75947863E-01f,
    3.89633417E-01f, 4.03608948E-01f, 4.17874694E-01f, 4.32430804E-01f, 4.47277188E-01f,
    4.62413728E-01f, 4.77840215E-01f, 4.93556231E-01f, 5.09561300E-01f, 5.25854886E-01f,
    5.42436182E-01f, 5.59304416E-01f, 5.76458573E-01f, 5.93897760E-01f, 6.11620665E-01f,
    6.29626155E-01f, 6.47912800E-01f, 6.66479111E-01f, 6.85323536E-01f, 7.04444408E-01f,
    7.23839939E-01f, 7.43508339E-01f, 7.63447523E-01f, 7.83655465E-01f, 8.04130018E-01f,
    8.24868977E-01f, 8.45869958E-01f, 8.67130578E-01f, 8.88648331E-01f, 9.10420537E-01f,
    9.32444632E-01f, 9.54717815E-01f, 9.77237225E-01f};

const FLOAT32 impd_drc_zero_pole_angle_table[128] = {
    0.00000000E+00f, 6.90533966E-04f, 7.31595252E-04f, 7.75098170E-04f, 8.21187906E-04f,
    8.70018279E-04f, 9.21752258E-04f, 9.76562500E-04f, 1.03463193E-03f, 1.09615434E-03f,
    1.16133507E-03f, 1.23039165E-03f, 1.30355455E-03f, 1.38106793E-03f, 1.46319050E-03f,
    1.55019634E-03f, 1.64237581E-03f, 1.74003656E-03f, 1.84350452E-03f, 1.95312500E-03f,
    2.06926386E-03f, 2.19230869E-03f, 2.32267015E-03f, 2.46078330E-03f, 2.60710909E-03f,
    2.76213586E-03f, 2.92638101E-03f, 3.10039268E-03f, 3.28475162E-03f, 3.48007312E-03f,
    3.68700903E-03f, 3.90625000E-03f, 4.13852771E-03f, 4.38461738E-03f, 4.64534029E-03f,
    4.92156660E-03f, 5.21421818E-03f, 5.52427173E-03f, 5.85276202E-03f, 6.20078536E-03f,
    6.56950324E-03f, 6.96014624E-03f, 7.37401807E-03f, 7.81250000E-03f, 8.27705542E-03f,
    8.76923475E-03f, 9.29068059E-03f, 9.84313320E-03f, 1.04284364E-02f, 1.10485435E-02f,
    1.17055240E-02f, 1.24015707E-02f, 1.31390065E-02f, 1.39202925E-02f, 1.47480361E-02f,
    1.56250000E-02f, 1.65541108E-02f, 1.75384695E-02f, 1.85813612E-02f, 1.96862664E-02f,
    2.08568727E-02f, 2.20970869E-02f, 2.34110481E-02f, 2.48031414E-02f, 2.62780130E-02f,
    2.78405849E-02f, 2.94960723E-02f, 3.12500000E-02f, 3.31082217E-02f, 3.50769390E-02f,
    3.71627223E-02f, 3.93725328E-02f, 4.17137454E-02f, 4.41941738E-02f, 4.68220962E-02f,
    4.96062829E-02f, 5.25560260E-02f, 5.56811699E-02f, 5.89921445E-02f, 6.25000000E-02f,
    6.62164434E-02f, 7.01538780E-02f, 7.43254447E-02f, 7.87450656E-02f, 8.34274909E-02f,
    8.83883476E-02f, 9.36441923E-02f, 9.92125657E-02f, 1.05112052E-01f, 1.11362340E-01f,
    1.17984289E-01f, 1.25000000E-01f, 1.32432887E-01f, 1.40307756E-01f, 1.48650889E-01f,
    1.57490131E-01f, 1.66854982E-01f, 1.76776695E-01f, 1.87288385E-01f, 1.98425131E-01f,
    2.10224104E-01f, 2.22724680E-01f, 2.35968578E-01f, 2.50000000E-01f, 2.64865774E-01f,
    2.80615512E-01f, 2.97301779E-01f, 3.14980262E-01f, 3.33709964E-01f, 3.53553391E-01f,
    3.74576769E-01f, 3.96850263E-01f, 4.20448208E-01f, 4.45449359E-01f, 4.71937156E-01f,
    5.00000000E-01f, 5.29731547E-01f, 5.61231024E-01f, 5.94603558E-01f, 6.29960525E-01f,
    6.67419927E-01f, 7.07106781E-01f, 7.49153538E-01f, 7.93700526E-01f, 8.40896415E-01f,
    8.90898718E-01f, 9.43874313E-01f, 1.00000000E+00f};

static const ia_drc_delta_gain_code_entry_struct impd_drc_delta_gain_code_table_by_size[25] = {
    {2, 0x003, -0.125f},  {2, 0x002, 0.125f},   {3, 0x001, -0.250f},  {3, 0x002, 0.000f},
    {4, 0x000, -2.000f},  {5, 0x002, -0.500f},  {5, 0x00F, -0.375f},  {5, 0x00E, 1.000f},
    {6, 0x019, -0.625f},  {6, 0x018, 0.250f},   {6, 0x006, 0.375f},   {7, 0x00F, -1.000f},
    {7, 0x034, -0.875f},  {7, 0x036, -0.750f},  {7, 0x037, 0.500f},   {8, 0x01D, 0.625f},
    {9, 0x039, -1.875f},  {9, 0x0D5, -1.125f},  {9, 0x0D7, 0.750f},   {9, 0x0D4, 0.875f},
    {10, 0x070, -1.500f}, {10, 0x1AC, -1.375f}, {10, 0x1AD, -1.250f}, {11, 0x0E2, -1.750f},
    {11, 0x0E3, -1.625f}};

static const ia_drc_delta_gain_code_entry_struct
    impd_drc_delta_gain_code_table_profile_2_by_size[49] = {
        {3, 0x007, -0.125f},  {4, 0x00C, -0.625f},  {4, 0x009, -0.500f},  {4, 0x005, -0.375f},
        {4, 0x003, -0.250f},  {4, 0x001, 0.000f},   {4, 0x00B, 0.125f},   {5, 0x011, -0.875f},
        {5, 0x00E, -0.750f},  {5, 0x005, 0.250f},   {5, 0x004, 0.375f},   {5, 0x008, 0.500f},
        {5, 0x000, 0.625f},   {5, 0x00D, 0.750f},   {5, 0x00F, 0.875f},   {5, 0x010, 1.000f},
        {5, 0x01B, 1.125f},   {6, 0x02B, -1.250f},  {6, 0x028, -1.125f},  {6, 0x002, -1.000f},
        {6, 0x012, 1.250f},   {6, 0x018, 1.375f},   {6, 0x029, 1.500f},   {7, 0x06A, -4.000f},
        {7, 0x054, -1.750f},  {7, 0x068, -1.625f},  {7, 0x026, -1.500f},  {7, 0x006, -1.375f},
        {7, 0x032, 1.625f},   {8, 0x0D2, -2.250f},  {8, 0x0AB, -2.125f},  {8, 0x0AA, -2.000f},
        {8, 0x04F, -1.875f},  {8, 0x04E, 1.750f},   {8, 0x0D7, 1.875f},   {8, 0x00E, 2.000f},
        {9, 0x1AD, -3.625f},  {9, 0x1AC, -3.375f},  {9, 0x1A6, -3.250f},  {9, 0x0CD, -3.125f},
        {9, 0x0CE, -2.750f},  {9, 0x1A7, -2.625f},  {9, 0x01F, -2.500f},  {9, 0x0CC, -2.375f},
        {10, 0x03C, -3.500f}, {10, 0x19E, -3.000f}, {10, 0x19F, -2.875f}, {11, 0x07A, -3.875f},
        {11, 0x07B, -3.750f}};

static const ia_drc_slope_code_table_entry_struct impd_drc_slope_code_table_entry_by_value[15] = {
    {6, 0x018, -3.0518f, 0}, {8, 0x042, -1.2207f, 1}, {7, 0x032, -0.4883f, 2},
    {5, 0x00A, -0.1953f, 3}, {5, 0x009, -0.0781f, 4}, {5, 0x00D, -0.0312f, 5},
    {2, 0x000, -0.005f, 6},  {1, 0x001, 0.0f, 7},     {4, 0x007, 0.005f, 8},
    {5, 0x00B, 0.0312f, 9},  {6, 0x011, 0.0781f, 10}, {9, 0x087, 0.1953f, 11},
    {9, 0x086, 0.4883f, 12}, {7, 0x020, 1.2207f, 13}, {7, 0x033, 3.0518f, 14},
};

static const WORD32 k_num_delta_gain_values_table =
    sizeof(impd_drc_delta_gain_code_table_by_size) /
    sizeof(impd_drc_delta_gain_code_table_by_size[0]);

static const WORD32 k_num_delta_gain_values_table_profile_2 =
    sizeof(impd_drc_delta_gain_code_table_profile_2_by_size) /
    sizeof(impd_drc_delta_gain_code_table_profile_2_by_size[0]);

const ia_drc_slope_code_table_entry_struct *impd_drc_get_slope_code_table_by_value(VOID) {
  return (&(impd_drc_slope_code_table_entry_by_value[0]));
}

VOID impd_drc_get_delta_gain_code_table(
    const WORD32 gain_coding_profile,
    ia_drc_delta_gain_code_entry_struct const **pstr_delta_gain_code_table, WORD32 *num_entries) {
  if (gain_coding_profile != GAIN_CODING_PROFILE_CLIPPING) {
    *pstr_delta_gain_code_table = impd_drc_delta_gain_code_table_by_size;
    *num_entries = k_num_delta_gain_values_table;
  } else {
    *pstr_delta_gain_code_table = impd_drc_delta_gain_code_table_profile_2_by_size;
    *num_entries = k_num_delta_gain_values_table_profile_2;
  }
}

VOID impd_drc_generate_delta_time_code_table(
    const WORD32 num_gain_values_max,
    ia_drc_delta_time_code_table_entry_struct *pstr_delta_time_code_table) {
  LOOPIDX idx;
  WORD32 max_val, temp = 1;

  while ((1 << temp) < 2 * num_gain_values_max) {
    temp++;
  }
  pstr_delta_time_code_table[0].size = -1;
  pstr_delta_time_code_table[0].code = -1;
  pstr_delta_time_code_table[0].value = -1;
  pstr_delta_time_code_table[1].size = 2;
  pstr_delta_time_code_table[1].code = 0x0;
  pstr_delta_time_code_table[1].value = 1;

  for (idx = 0; idx < 4; idx++) {
    pstr_delta_time_code_table[idx + 2].size = 4;
    pstr_delta_time_code_table[idx + 2].code = 0x4 + idx;
    pstr_delta_time_code_table[idx + 2].value = idx + 2;
  }
  for (idx = 0; idx < 8; idx++) {
    pstr_delta_time_code_table[idx + 6].size = 5;
    pstr_delta_time_code_table[idx + 6].code = 0x10 + idx;
    pstr_delta_time_code_table[idx + 6].value = idx + 6;
  }

  max_val = 2 * num_gain_values_max - 14 + 1;
  for (idx = 0; idx < max_val; idx++) {
    pstr_delta_time_code_table[idx + 14].size = 2 + temp;
    pstr_delta_time_code_table[idx + 14].code = (0x3 << temp) + idx;
    pstr_delta_time_code_table[idx + 14].value = idx + 14;
  }
}

FLOAT32 impd_drc_decode_slope_idx_value(const WORD32 slope_code_idx) {
  const ia_drc_slope_code_table_entry_struct *pstr_slope_code_table =
      impd_drc_get_slope_code_table_by_value();

  return pstr_slope_code_table[slope_code_idx].value;
}

FLOAT32 impd_drc_decode_slope_idx_magnitude(const WORD32 slope_code_idx) {
  const ia_drc_slope_code_table_entry_struct *pstr_slope_code_table =
      impd_drc_get_slope_code_table_by_value();

  return (FLOAT32)fabs((FLOAT64)pstr_slope_code_table[slope_code_idx].value);
}
