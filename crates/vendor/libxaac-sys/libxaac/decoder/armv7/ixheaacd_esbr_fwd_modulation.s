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
 .extern ixheaacd_esbr_cos_sin_mod
.hidden ixheaacd_esbr_cos_sin_mod
 .global ixheaacd_esbr_fwd_modulation
 .type ixheaacd_esbr_fwd_modulation, %function
ixheaacd_esbr_fwd_modulation:

    STMFD           sp!, {r4-r12, lr}
    VPUSH           {D8 - D15}
    LDR             R4, [R3]
    ADD             R5, R0, R4, LSL #3
    MOV             R6, R1
    MOV             R7, R2

LOOP1:
    SUB             R5, R5, #32
    VLD1.32         {D0, D1, D2, D3}, [R0]!
    VLD1.32         {D4, D5, D6, D7}, [R5]
    VSHR.S32        Q0, Q0, #4
    VSHR.S32        Q1, Q1, #4
    VSHR.S32        Q2, Q2, #4
    VSHR.S32        Q3, Q3, #4

    vswp            d4, d7
    vswp            d5, d6

    vrev64.32       q2, q2
    vrev64.32       q3, q3

    VQSUB.S32       Q4, Q0, Q2
    VQSUB.S32       Q5, Q1, Q3

    VADD.S32        Q6, Q0, Q2
    VADD.S32        Q7, Q1, Q3

    SUBS            R4, R4, #8
    VST1.32         {D8, D9, D10, D11}, [R6]!
    VST1.32         {D12, D13, D14, D15}, [R7]!

    BGT             LOOP1
    STMFD           sp!, {r0-r3, lr}
    LDR             R4, [SP, #124]
    MOV             R0, R1
    MOV             R1, R3
    MOVW            R5, #0x41FC
    ADD             R2, R4, R5
    ADD             R3, R4, #0xB8

    BL              ixheaacd_esbr_cos_sin_mod

    LDMFD           sp!, {r0-r3, r14}

    LDR             R0, [R3, #0x5C]
    LDRSH           R4, [R3, #0x2C]
    LDRSH           R5, [R3, #0x2A]

    SUB             R4, R4, R5

LOOP2:
    VLD2.32         {D0, D1}, [R0]!
    VLD1.32         {D2}, [R1]
    VLD1.32         {D3}, [R2]

    VMULL.S32       q2, d0, d2
    VMULL.S32       q3, d0, d3
    VMULL.S32       q4, d1, d2
    VMULL.S32       q5, d1, d3

    VADD.I64        Q0, Q2, Q5
    VQSUB.S64       Q1, Q3, Q4

    VSHRN.I64       D0, Q0, #31
    VSHRN.I64       D2, Q1, #31

    SUBS            R4, R4, #2
    VST1.32         {D0}, [R1]!
    VST1.32         {D2}, [R2]!

    BGT             LOOP2

    VPOP            {D8-D15}
    LDMFD           sp!, {r4-r12, r15}








