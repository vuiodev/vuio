@/******************************************************************************
@ *
@ * Copyright (C) 2018 The Android Open Source Project
@ *
@ * Licensed under the Apache License, Version 2.0 (the "License");
@ * you may not use this file except in compliance with the License.
@ * You may obtain a copy of the License at:
@ *
@ * http:@www.apache.org/licenses/LICENSE-2.0
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

    .global ixheaacd_mps_synt_post_fft_twiddle_armv7
ixheaacd_mps_synt_post_fft_twiddle_armv7:

    STMFD           sp!, {r4-r12, r14}
    VPUSH           {D8-D15}
    LDR             R4, [SP, #104]
    LDR             R5, [SP, #108]

    ADD             R6, R5, R0, LSL #3
    LSL             R7, R0, #1
    MOV             R8, #-16
    ADD             R6, R6, R8
LOOP1:
    VLD1.32         {D0, D1}, [R1]!
    VLD1.32         {D2, D3}, [R2]!
    VLD1.32         {D4, D5}, [R3]!
    VLD1.32         {D6, D7}, [R4]!

    VMULL.S32       Q4, D0, D4
    VMULL.S32       Q5, D2, D6
    VMULL.S32       Q6, D1, D5
    VMULL.S32       Q7, D3, D7

    VSHRN.S64       D8, Q4, #31
    VSHRN.S64       D10, Q5, #31
    VSHRN.S64       D12, Q6, #31
    VSHRN.S64       D14, Q7, #31

    VQADD.S32       D1, D8, D10
    VQADD.S32       D0, D12, D14

    VREV64.32       D1, D1
    VREV64.32       D0, D0


    SUBS            R7, R7, #4
    VST1.32         {D0, D1}, [R6], R8

    BGT             LOOP1
    VPOP            {D8-D15}
    LDMFD           sp!, {r4-r12, r15}
