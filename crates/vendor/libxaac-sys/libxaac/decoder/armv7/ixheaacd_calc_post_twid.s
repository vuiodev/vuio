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

    .global ixheaacd_calc_post_twid_armv7
ixheaacd_calc_post_twid_armv7:

    STMFD           sp!, {r4-r12, r14}
    VPUSH           {D8-D15}
    LDR             R4, [SP, #104]
    LDR             R5, [SP, #108]
    ADD             R6, R0, R3, LSL #3
    SUB             R6, R6, #4
    MOV             R7, #-8
    MOV             R8, #8

LOOP1:
    VLD1.32         {D0, D1}, [R4]!
    VLD1.32         {D2, D3}, [R5]!
    VLD1.32         {D4, D5}, [R1]!
    VLD1.32         {D6, D7}, [R2]!

    VMULL.S32       Q4, D4, D0
    VMULL.S32       Q5, D6, D2
    VMULL.S32       Q6, D6, D0
    VMULL.S32       Q7, D4, D2
    VMULL.S32       Q8, D5, D1
    VMULL.S32       Q9, D7, D3
    VMULL.S32       Q10, D7, D1
    VMULL.S32       Q11, D5, D3

    VSHRN.S64       D6, Q4, #32
    VSHRN.S64       D8, Q5, #32
    VSHRN.S64       D10, Q6, #32
    VSHRN.S64       D12, Q7, #32
    VSHRN.S64       D7, Q8, #32
    VSHRN.S64       D9, Q9, #32
    VSHRN.S64       D11, Q10, #32
    VSHRN.S64       D13, Q11, #32

    VSUB.I32        D0, D6, D8
    VADD.I32        D1, D10, D12
    VSUB.I32        D2, D7, D9
    VADD.I32        D3, D11, D13

    VNEG.S32        Q0, Q0
    VNEG.S32        Q1, Q1
    SUBS            R3, R3, #4

    VST1.32         {D0[0]}, [R0], R8
    VST1.32         {D1[0]}, [R6], R7
    VST1.32         {D0[1]}, [R0], R8
    VST1.32         {D1[1]}, [R6], R7

    VST1.32         {D2[0]}, [R0], R8
    VST1.32         {D3[0]}, [R6], R7
    VST1.32         {D2[1]}, [R0], R8
    VST1.32         {D3[1]}, [R6], R7
    BGT             LOOP1
    VPOP            {D8-D15}
    LDMFD           sp!, {r4-r12, r15}




























