@/******************************************************************************
@ *
@ * Copyright (C) 2018 The Android Open Source Project
@ *
@ * Licensed under the Apache License, Version 2.0 (the "License");
@ * you may not use this file except in compliance with the License.
@ * You may obtain a copy of the License at:
@ *
@ * http://www.apache.org/licenses/LICENSE-2.0
@ *
@ * Unless required by applicable law or agreed to in writing, software
@ * distributed under the License is distributed on an "AS IS" BASIS,
@ * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
@ * See the License for the specific language governing permissions and
@ * limitations under the License.
@ *
@ *****************************************************************************
@ * Originally developed and contributed by Ittiam Systems Pvt. Ltd, Bangalore
@*/


.text
.p2align 2
.global ixheaacd_mps_mulshift

ixheaacd_mps_mulshift:

    STMFD           sp!, {R4-R12}
    VPUSH           {d8 - d15}
LOOP:
    VLD1.32         {Q0, Q1}, [R0]!     @LOADING values from R0
    VLD1.32         {Q2, Q3}, [R1]!     @LOADING values from R1
    VQDMULL.S32     Q4, D0, D4
    VQDMULL.S32     Q5, D2, D6
    VQDMULL.S32     Q6, D1, D5
    VQDMULL.S32     Q7, D3, D7
    VUZP.32         Q4, Q6
    VUZP.32         Q5, Q7
    VST1.32         {Q6, Q7}, [R2]!     @Storing values to R2
    SUBS            R3, R3, #8
    BGT             LOOP

    VPOP            {d8 - d15}
    LDMFD           sp!, {R4-R12}
    BX              LR

