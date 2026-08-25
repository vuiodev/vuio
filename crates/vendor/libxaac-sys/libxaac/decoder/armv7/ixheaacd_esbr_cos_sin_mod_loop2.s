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

    .global ixheaacd_esbr_cos_sin_mod_loop2
    .type ixheaacd_esbr_cos_sin_mod_loop2, %function
ixheaacd_esbr_cos_sin_mod_loop2:

    STMFD           sp!, {r4-r12, r14}
    VPUSH           {D8-D15}
    @generating load addresses
    ADD             R3, R0, R2, LSL #3  @psubband1 = &subband[2 * M - 1];
    SUB             R3, R3, #4
    ADD             R10, R0, #256
    ADD             R11, R10, R2, LSL #3
    SUB             R11, R11, #4
    MOV             R8, #-4
    LDR             R6, [R0]
    MOV             R4, R2, ASR #1      @M_2 = ixheaac_shr32(M, 1);
    SUB             R4, R4, #1

    ASR             R6, R6, #1          @*psubband = *psubband >> 1;
    VLD1.32         {D2[0]}, [R3]

    STR             R6, [R0], #4        @psubband++;
    LDR             R7, [R0]
    ASR             R7, R7, #1
    RSB             R6, R7, #0
    STR             R6, [R3], #-4
    VLD1.32         {D3[0]}, [R3]       @  im = *psubband1;

    VLD2.32         {D0[0], D1[0]}, [R1]!
    VDUP.32         D0, D0[0]
    VDUP.32         D1, D1[0]

    VLD1.32         {D2[1]}, [R11]      @re = *psubband12;

    LDR             R6, [R10]
    ASR             R7, R6, #1
    MOV             R9, #0
    QSUB            R7, R9, R7

    STR             R7, [R11], #-4

    LDR             R6, [R10, #4]
    ASR             R6, R6, #1
    STR             R6, [R10], #4

    VLD1.32         {D3[1]}, [R11]

    VMULL.S32       q2, d0, d2          @qsub 2nd
    VMULL.S32       q3, d0, d3          @add 2nd
    VMULL.S32       q4, d1, d2          @add 1st
    VMULL.S32       q5, d1, d3          @qsub 1st

    vadd.I64        q6, q4, q3
    VQSUB.S64       Q7, Q5, Q2
    VQSUB.S64       Q8, Q2, Q5

    VSHRN.I64       D12, Q6, #32
    VSHRN.I64       D14, Q7, #32
    VSHRN.I64       D16, Q8, #32

    VST1.32         {D12[0]}, [R3], R8

    VST1.32         {D14[0]}, [R0]!

    VQNEG.S32       D12, D12


    VST1.32         {D12[1]}, [R10]!

    VST1.32         {D16[1]}, [R11], R8

LOOP1:
    VLD1.32         {D2}, [R0]
    VLD1.32         {D3}, [R10]
    LDR             R5, [R3]            @RE2
    LDR             R6, [R11]           @RE3
    VTRN.32         D2, D3

    VMULL.S32       q2, d0, d2          @qsub 2nd
    VMULL.S32       q3, d0, d3          @add 2nd
    VMULL.S32       q4, d1, d2          @add 1st
    VMULL.S32       q5, d1, d3          @qsub 1st

    vadd.I64        q6, q4, q3
    VQSUB.S64       Q7, Q2, Q5
    VQSUB.S64       Q8, Q5, Q2

    VSHRN.I64       D12, Q6, #32
    VSHRN.I64       D14, Q7, #32
    VSHRN.I64       D16, Q8, #32

    VST1.32         {D12[0]}, [R0]!
    VST1.32         {D14[0]}, [R3], R8
    VQNEG.S32       D12, D12

    VST1.32         {D12[1]}, [R11], R8
    VST1.32         {D16[1]}, [R10]!

    @ second part
    VLD2.32         {D0[0], D1[0]}, [R1]!
    VDUP.32         D0, D0[0]
    VDUP.32         D1, D1[0]

    VMOV            D3, R5, R6
    VLD1.32         {D2[0]}, [R3]
    VLD1.32         {D2[1]}, [R11]

    VMULL.S32       q2, d0, d2          @qsub 2nd
    VMULL.S32       q3, d0, d3          @add 2nd
    VMULL.S32       q4, d1, d2          @add 1st
    VMULL.S32       q5, d1, d3          @qsub 1st

    vadd.I64        q6, q2, q5
    VQSUB.S64       Q7, Q4, Q3
    VQSUB.S64       Q8, Q3, Q4

    VSHRN.I64       D12, Q6, #32
    VSHRN.I64       D14, Q7, #32
    VSHRN.I64       D16, Q8, #32

    VST1.32         {D12[0]}, [R3], R8
    VST1.32         {D14[0]}, [R0]!

    VQNEG.S32       D12, D12

    subs            r4, r4, #1
    VST1.32         {D12[1]}, [R10]!
    VST1.32         {D16[1]}, [R11], R8

    BGT             LOOP1
    VPOP            {D8-D15}
    LDMFD           sp!, {r4-r12, r15}


























