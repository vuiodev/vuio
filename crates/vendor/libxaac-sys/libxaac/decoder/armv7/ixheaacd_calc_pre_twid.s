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

    .global ixheaacd_calc_pre_twid_armv7
ixheaacd_calc_pre_twid_armv7:

    STMFD           sp!, {r4-r12, r14}
    VPUSH           {D8-D15}
    LDR             R4, [SP, #104]
    LDR             R5, [SP, #108]
    ADD             R6, R0, R3, LSL #3
    SUB             R6, R6, #12
    MOV             R7, #-16

LOOP1:
    VLD1.32         {D0, D1}, [R4]!
    VLD1.32         {D2, D3}, [R5]!
    VLD2.32         {D4, D5}, [R0]!
    VLD2.32         {D6, D7}, [R0]!
    VLD2.32         {D8, D9}, [R6], R7
    VLD2.32         {D10, D11}, [R6], R7

    VREV64.32       D8, D8
    VREV64.32       D9, D10
    VNEG.S32        D5, D4
    VNEG.S32        D7, D6

    VMULL.S32       Q6, D0, D5
    VMULL.S32       Q7, D2, D8
    VMULL.S32       Q8, D0, D8
    VMULL.S32       Q9, D2, D4
    VMULL.S32       Q10, D1, D7
    VMULL.S32       Q11, D9, D3
    VMULL.S32       Q12, D1, D9
    VMULL.S32       Q13, D3, D6


    VSHRN.S64       D12, Q6, #32
    VSHRN.S64       D14, Q7, #32
    VSHRN.S64       D16, Q8, #32
    VSHRN.S64       D18, Q9, #32
    VSHRN.S64       D20, Q10, #32
    VSHRN.S64       D22, Q11, #32
    VSHRN.S64       D24, Q12, #32
    VSHRN.S64       D26, Q13, #32

    VSUB.I32        D0, D12, D14
    VSUB.I32        D2, D16, D18
    VSUB.I32        D1, D20, D22
    VSUB.I32        D3, D24, D26

    SUBS            R3, R3, #4
    VST1.32         {D0, D1}, [R1]!
    VST1.32         {D2, D3}, [R2]!

    BGT             LOOP1
    VPOP            {D8-D15}
    LDMFD           sp!, {r4-r12, r15}




























