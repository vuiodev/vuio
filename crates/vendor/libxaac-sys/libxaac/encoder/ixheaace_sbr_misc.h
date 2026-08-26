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
/* Sorting routines */
VOID ixheaace_shellsort_int(WORD32 *ptr_in, WORD32 n);

VOID ixheaace_add_left(WORD32 *ptr_vector, WORD32 *ptr_length_vector, WORD32 value);

VOID ixheaace_add_right(WORD32 *ptr_vector, WORD32 *ptr_length_vector, WORD32 value);

VOID ixheaace_add_vec_left(WORD32 *ptr_dst, WORD32 *ptr_length_dst, WORD32 *ptr_src,
                           WORD32 length_src);
