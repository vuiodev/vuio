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
   .global ixheaacd_calc_max_spectral_line_armv7

ixheaacd_calc_max_spectral_line_armv7:

    STMFD           sp!, {R4-R12, R14}
    MOV             R4, R1, LSR #3
    MOV             R6, R4, LSL #3
    VMOV.S32        D6, #0x00000000
    VMOV.S32        D7, #0x00000000

LOOP_1:
    VLD1.32         {D0, D1}, [R0]!

    VLD1.32         {D2, D3}, [R0]!
    VABS.S32        Q0, Q0


    VABS.S32        Q1, Q1
    SUBS            R4, R4, #1

    VORR            Q3, Q0, Q3

    VORR            Q3, Q1, Q3
    BGT             LOOP_1

    SUBS            R7, R1, R6

    VMOV.32         R4, D6[0]
    VMOV.32         R1, D6[1]
    VMOV.32         R2, D7[0]
    ORR             R4, R4, R1
    VMOV.32         R3, D7[1]
    ORR             R4, R4, R2


    ORR             R4, R4, R3
    BEQ             END_FUNC
LOOP_2:

    LDR             R2, [R0], #4
    MOVS            R2, R2
    RSBMI           R2, R2, #0
    ORR             R4, R4, R2
    SUBS            R7, R7, #1
    BGT             LOOP_2

END_FUNC:

    MOVS            R0, R4
    MVNMI           R0, R0
    CLZ             R0, R0
    SUB             R0, R0, #1

    LDMFD           sp!, {R4-R12, R15}





