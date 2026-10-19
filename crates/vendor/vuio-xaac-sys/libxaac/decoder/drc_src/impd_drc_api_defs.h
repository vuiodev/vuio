/******************************************************************************
 *
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

#ifndef IMPD_API_DEFS_H
#define IMPD_API_DEFS_H

/*****************************************************************************/
/* Constant hash defines                                                     */
/*****************************************************************************/
/* A constant to let API copy small strings to buffers outside */
#define IA_API_STR_LEN 30
#define IA_APIVERSION_MAJOR 1
#define IA_APIVERSION_MINOR 10

/* last compatible version */
/* sometimes a new API version is just for a bugfix, or a added feature  in */
/* this case it is better to use a newer version even though a library  was */
/* made for an older version, library API can then be upgraded to newer API */
/* version after checking for compatibility or by adding features           */
#define IA_LASTCOMP_APIVERSION_MAJOR 1
#define IA_LASTCOMP_APIVERSION_MINOR 10

#define IA_STR(str) #str
#define IA_MAKE_VERSION_STR(maj, min) IA_STR(maj) "." IA_STR(min)
#define IA_APIVERSION \
  IA_MAKE_VERSION_STR(IA_APIVERSION_MAJOR, IA_APIVERSION_MINOR)

#define IA_LAST_COMP_APIVERSION                     \
  IA_MAKE_VERSION_STR(IA_LASTCOMP_APIVERSION_MAJOR, \
                      IA_LASTCOMP_APIVERSION_MINOR)

#endif
