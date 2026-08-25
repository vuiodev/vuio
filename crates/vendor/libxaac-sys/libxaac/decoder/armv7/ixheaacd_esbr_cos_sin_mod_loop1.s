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

    .global ixheaacd_esbr_cos_sin_mod_loop1
    .type ixheaacd_esbr_cos_sin_mod_loop1, %function
ixheaacd_esbr_cos_sin_mod_loop1:

    STMFD           sp!, {r4-r12, r14}
    VPUSH           {D8-D11}
@generating load addresses
    ADD             r4, r0, r1, lsl #3  @psubband1
    SUB             r4, r4, #4
    ADD             r5, r3, r1, lsl #3  @psubband1_t
    SUB             r5, r5, #8
    MOV             r6, r1, ASR #2

LOOP1:
@first part
    vld1.32         {d0} , [r2]!
    vrev64.32       d1, d0
    vld1.32         {d2[0]}, [r0]!
    ADD             r7, r0, #252
    vld1.32         {d2[1]}, [r7]
    vld1.32         {d3[0]}, [r4]
    ADD             r7, r4, #256
    vld1.32         {d3[1]}, [r7]
    SUB             r4, r4, #4

    VMULL.S32       q2, d0, d2          @qsub 2nd
    VMULL.S32       q3, d0, d3          @add 2nd
    VMULL.S32       q4, d1, d2          @add 1st
    VMULL.S32       q5, d1, d3          @qsub 1st

    vadd.I64        q0, q4, q3
    VQSUB.S64       Q1, Q5, Q2

    VSHRN.I64       D0, Q0, #32
    VSHRN.I64       D2, Q1, #32
    VMOV.32         D3, D0
    VST2.32         {D0[0], D2[0]}, [R3]!
    ADD             r7, r3, #248
    VST2.32         {D2[1], D3[1]}, [R7]

@second part
    vld1.32         {d0} , [r2]!
    vrev64.32       d1, d0
    vld1.32         {d2[0]}, [r0]!
    ADD             R7, R0, #252
    vld1.32         {d2[1]}, [r7]
    vld1.32         {d3[0]}, [r4]
    ADD             R7, R4, #256
    vld1.32         {d3[1]}, [r7]
    SUB             r4, r4, #4

    VMULL.S32       q2, d0, d2          @add 2nd
    VMULL.S32       q3, d0, d3          @sub 2nd
    VMULL.S32       q4, d1, d2          @sub 1st
    VMULL.S32       q5, d1, d3          @add 1st

    VADD.I64        Q0, Q5, Q2
    VQSUB.S64       Q1, Q4, Q3

    VSHRN.I64       D0, Q0, #32
    VSHRN.I64       D2, Q1, #32
    VMOV.32         D3, D0
    VST2.32         {D0[0], D2[0]}, [R5]
    ADD             R7, R5, #256
    VST2.32         {D2[1], D3[1]}, [R7]
    SUB             r5, r5, #8
@Third part
    vld1.32         {d0} , [r2]!
    vrev64.32       d1, d0
    vld1.32         {d2[0]}, [r0]!
    ADD             r7, r0, #252
    vld1.32         {d2[1]}, [r7]
    vld1.32         {d3[0]}, [r4]
    ADD             r7, r4, #256
    vld1.32         {d3[1]}, [r7]
    SUB             r4, r4, #4

    VMULL.S32       q2, d0, d2          @qsub 2nd
    VMULL.S32       q3, d0, d3          @add 2nd
    VMULL.S32       q4, d1, d2          @add 1st
    VMULL.S32       q5, d1, d3          @qsub 1st

    vadd.I64        q0, q4, q3
    VQSUB.S64       Q1, Q5, Q2

    VSHRN.I64       D0, Q0, #32
    VSHRN.I64       D2, Q1, #32
    VMOV.32         D3, D0
    VST2.32         {D0[0], D2[0]}, [R3]!
    ADD             r7, r3, #248
    VST2.32         {D2[1], D3[1]}, [R7]

@Fourth part
    vld1.32         {d0} , [r2]!
    vrev64.32       d1, d0
    vld1.32         {d2[0]}, [r0]!
    ADD             R7, R0, #252
    vld1.32         {d2[1]}, [r7]
    vld1.32         {d3[0]}, [r4]
    ADD             R7, R4, #256
    vld1.32         {d3[1]}, [r7]
    SUB             r4, r4, #4

    VMULL.S32       q2, d0, d2          @add 2nd
    VMULL.S32       q3, d0, d3          @sub 2nd
    VMULL.S32       q4, d1, d2          @sub 1st
    VMULL.S32       q5, d1, d3          @add 1st

    VADD.I64        Q0, Q5, Q2
    VQSUB.S64       Q1, Q4, Q3

    VSHRN.I64       D0, Q0, #32
    VSHRN.I64       D2, Q1, #32
    VMOV.32         D3, D0
    VST2.32         {D0[0], D2[0]}, [R5]
    ADD             R7, R5, #256
    SUBS            R6, R6, #1
    VST2.32         {D2[1], D3[1]}, [R7]
    SUB             r5, r5, #8

    BGT             LOOP1
    VPOP            {D8-D11}
    LDMFD           sp!, {r4-r12, r15}


























