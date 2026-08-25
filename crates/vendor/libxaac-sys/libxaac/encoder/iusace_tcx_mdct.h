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
VOID iusace_tcx_mdct_main(FLOAT32 *ptr_in, FLOAT32 *ptr_out, WORD32 l, WORD32 m, WORD32 r,
                          iusace_scratch_mem *pstr_scratch);

VOID iusace_tcx_imdct(FLOAT32 *ptr_in, FLOAT32 *ptr_out, WORD32 l, WORD32 m, WORD32 r,
                      iusace_scratch_mem *pstr_scratch);

VOID iusace_tcx_mdct(FLOAT32 *ptr_in, FLOAT32 *ptr_out, WORD32 length,
                     iusace_scratch_mem *pstr_scratch);
