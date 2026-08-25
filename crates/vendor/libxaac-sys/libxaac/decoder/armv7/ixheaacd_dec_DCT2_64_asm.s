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
        .extern ixheaacd_radix4bfly
.hidden ixheaacd_radix4bfly
        .extern ixheaacd_postradixcompute2
.hidden ixheaacd_postradixcompute2
        .extern ixheaacd_sbr_imdct_using_fft
.hidden ixheaacd_sbr_imdct_using_fft

        .global ixheaacd_dec_DCT2_64_asm
        .type ixheaacd_dec_DCT2_64_asm, %function

ixheaacd_dec_DCT2_64_asm:

    STMFD           sp!, {r0-r3, r4-r12, r14}
    ADD             R2, R1, #252
    MOV             R3, #32
    MOV             R4, #-4

    ADD             R2, R2, #4


FOR_LOOP:

    VLD2.32         {Q0, Q1}, [R0]!
    SUBS            R3, R3, #4

    VST1.32         {Q0}, [R1]!
    SUB             R2, R2, #16

    VREV64.32       Q1, Q1
    VSWP            D2, D3
    VST1.32         {Q1}, [R2]
    BGT             FOR_LOOP

    LDR             r0, [sp, #8]
    MOV             r1, #32
    LDR             r2, [sp, #4]
    LDR             r3, [sp]

    LDR             r4, [sp, #12]
    STR             r4, [sp, #-4]!
    STR             r4, [sp, #-4]!
    STR             r4, [sp, #-4]!
    STR             r4, [sp, #-4]!

    BL              ixheaacd_sbr_imdct_using_fft

    ADD             sp, sp, #16

    LDR             r0, [sp]


    LDR             r2, [sp, #56]

    VPUSH           {D8 - D15}
    ADD             R5, R0, #252
    VLD1.32         D0, [R0]
    ADD             R3, R2, #2
    VSHL.S32        D0, D0, #1
    VST1.32         D0, [R0]!
    SUB             R5, R5, #28


    VLD2.32         {Q0, Q1}, [R0]!

    VLD2.32         {Q2, Q3}, [R5]!
    VREV64.32       Q2, Q2
    VSWP            D4, D5
    MOV             R10, #-8
    VREV64.32       Q3, Q3
    ADD             R4, R2, #30
    VSWP            D6, D7
    SUB             R4, R4, #6
    VLD1.16         D8, [R3]!
    VSUB.I32        Q11, Q3, Q1
    VLD1.16         D10, [R4], R10
    VADD.I32        Q10, Q3, Q1
    VREV64.16       D10, D10
    VSUB.I32        Q9, Q0, Q2
    VUZP.16         D20, D21
    VADD.I32        Q8, Q0, Q2
    VUZP.16         D18, D19
    VMULL.U16       Q15, D20, D8
    VMLSL.U16       Q15, D18, D10
    VMULL.U16       Q14, D18, D8
    VMLAL.U16       Q14, D20, D10
    SUB             R11, R0, #32
    VSHR.S32        Q15, Q15, #16
    VSHR.U32        Q14, Q14, #16
    SUB             R12, R5, #32
    VMLAL.S16       Q15, D21, D8
    VMLSL.S16       Q15, D19, D10

    VLD2.32         {Q0, Q1}, [R0]!
    SUB             R5, R5, #64
    VMLAL.S16       Q14, D19, D8
    VLD2.32         {Q2, Q3}, [R5]!
    VMLAL.S16       Q14, D21, D10
    VREV64.32       Q2, Q2
    VSHL.S32        Q15, Q15, #1
    VSWP            D4, D5
    VSHL.S32        Q14, Q14, #1

    VREV64.32       Q3, Q3
    VADD.I32        Q13, Q8, Q15
    VSWP            D6, D7
    VADD.I32        Q12, Q11, Q14


    VLD1.16         D8, [R3]!
    VSUB.I32        Q7, Q14, Q11
    VLD1.16         D10, [R4], R10

    VSUB.I32        Q6, Q8, Q15
    VREV64.32       Q7, Q7
    VREV64.32       Q6, Q6
    VSWP            D14, D15
    VSWP            D12, D13
    VREV64.16       D10, D10


    VSUB.I32        Q11, Q3, Q1
    VSWP            Q13, Q12
    VADD.I32        Q10, Q3, Q1
    VST2.32         {Q12, Q13}, [R11]!
    VSUB.I32        Q9, Q0, Q2

    VADD.I32        Q8, Q0, Q2
    VST2.32         {Q6, Q7}, [R12]
    SUB             R11, R0, #32

    VUZP.16         D20, D21
    SUB             R12, R5, #32

    VUZP.16         D18, D19
    SUB             R5, R5, #64

    VMULL.U16       Q15, D20, D8
    VLD2.32         {Q0, Q1}, [R0]!
    VMLSL.U16       Q15, D18, D10
    VLD2.32         {Q2, Q3}, [R5]!
    VMULL.U16       Q14, D18, D8
    VREV64.32       Q2, Q2
    VMLAL.U16       Q14, D20, D10
    VSWP            D4, D5
    VSHR.S32        Q15, Q15, #16
    VREV64.32       Q3, Q3
    VMLAL.S16       Q15, D21, D8
    VMLSL.S16       Q15, D19, D10
    VSWP            D6, D7
    VSHR.U32        Q14, Q14, #16
    VMLAL.S16       Q14, D19, D8
    VLD1.16         D8, [R3]!
    VMLAL.S16       Q14, D21, D10
    VSHL.S32        Q15, Q15, #1
    VLD1.16         D10, [R4], R10

    VSHL.S32        Q14, Q14, #1
    VREV64.16       D10, D10
    VADD.I32        Q13, Q8, Q15
    VADD.I32        Q12, Q11, Q14


    VSUB.I32        Q7, Q14, Q11
    VSUB.I32        Q6, Q8, Q15
    VREV64.32       Q7, Q7
    VSUB.I32        Q11, Q3, Q1
    VREV64.32       Q6, Q6
    VADD.I32        Q10, Q3, Q1
    VSWP            D14, D15
    VSUB.I32        Q9, Q0, Q2
    VSWP            D12, D13
    VADD.I32        Q8, Q0, Q2
    VSWP            Q13, Q12


    VUZP.16         D20, D21
    VUZP.16         D18, D19
    VMULL.U16       Q15, D20, D8
    VMLSL.U16       Q15, D18, D10
    VST2.32         {Q12, Q13}, [R11]!
    VMULL.U16       Q14, D18, D8
    VMLAL.U16       Q14, D20, D10
    VST2.32         {Q6, Q7}, [R12]

    SUB             R11, R0, #32
    VLD2.32         {Q0, Q1}, [R0]!
    SUB             R12, R5, #32
    SUB             R5, R5, #64
    VSHR.S32        Q15, Q15, #16
    VLD2.32         {Q2, Q3}, [R5]!
    VMLAL.S16       Q15, D21, D8
    VREV64.32       Q2, Q2
    VMLSL.S16       Q15, D19, D10
    VSWP            D4, D5
    VSHR.U32        Q14, Q14, #16

    VREV64.32       Q3, Q3
    VMLAL.S16       Q14, D19, D8
    VSWP            D6, D7
    VMLAL.S16       Q14, D21, D10
    VSHL.S32        Q15, Q15, #1

    VLD1.16         D8, [R3]!
    VSHL.S32        Q14, Q14, #1

    VADD.I32        Q13, Q8, Q15

    VLD1.16         D10, [R4], R10

    VADD.I32        Q12, Q11, Q14


    VREV64.16       D10, D10
    VSUB.I32        Q7, Q14, Q11


    VSUB.I32        Q6, Q8, Q15
    VREV64.32       Q7, Q7
    VREV64.32       Q6, Q6
    VSWP            D14, D15
    VSWP            D12, D13

    VSWP            Q13, Q12
    VSUB.I32        Q11, Q3, Q1
    VST2.32         {Q12, Q13}, [R11]!
    VADD.I32        Q10, Q3, Q1

    VST2.32         {Q6, Q7}, [R12]

    SUB             R11, R0, #32

    VSUB.I32        Q9, Q0, Q2
    VADD.I32        Q8, Q0, Q2
    VUZP.16         D20, D21
    SUB             R12, R5, #32
    VUZP.16         D18, D19
    SUB             R5, R5, #64

    VMULL.U16       Q15, D20, D8
    VMLSL.U16       Q15, D18, D10
    VMULL.U16       Q14, D18, D8
    VMLAL.U16       Q14, D20, D10
    VSHR.S32        Q15, Q15, #16
    VMLAL.S16       Q15, D21, D8
    VMLSL.S16       Q15, D19, D10

    VSHR.U32        Q14, Q14, #16
    VMLAL.S16       Q14, D19, D8
    VMLAL.S16       Q14, D21, D10

    VSHL.S32        Q15, Q15, #1
    VSHL.S32        Q14, Q14, #1
    VADD.I32        Q13, Q8, Q15
    VADD.I32        Q12, Q11, Q14

    VSUB.I32        Q7, Q14, Q11
    VSUB.I32        Q6, Q8, Q15
    VREV64.32       Q7, Q7
    VREV64.32       Q6, Q6
    VSWP            D14, D15
    VSWP            D12, D13


    VSWP            Q13, Q12
    VST2.32         {Q12, Q13}, [R11]!

    VST2.32         {Q6, Q7}, [R12]
    VPOP            {D8 - D15}












    LDMFD           sp!, {r0-r2, r3}
    LDR             R1, [SP, #48]
    LDR             R2, [SP, #44]
    ADD             R3, R1, #126
    VLD1.32         D0[0], [R0, :32]!
    SUB             R4, R1, #2

    ADD             R5, R1, #130
    VLD1.32         D1[0], [R0, :32]!
    ADD             R7, R2, #4

    MOV             R6, #0x8000
    VDUP.32         Q15, R6

    VADD.I32        D2, D0, D1
    VSHR.S32        D2, D2, #1
    VSHL.S32        D2, D2, #4
    VADD.I32        Q2, Q1, Q15
    VSHR.S32        Q2, Q2, #16
    VSUB.I32        D6, D0, D1
    VST1.16         D4[0], [R1]!
    MOV             R8, #28

    MOV             R9, #-2
    VLD2.32         {Q0, Q1}, [R0]!
    SUB             R4, R4, #6

    SUB             R3, R3, #6
    VLD2.16         {D4, D5}, [R7]!

    VUZP.16         D0, D1

    VUZP.16         D2, D3
    VMULL.U16       Q14, D0, D4

    VMLSL.U16       Q14, D2, D5
    VSHR.S32        Q14, Q14, #16
    VMLAL.S16       Q14, D1, D4
    VMLSL.S16       Q14, D3, D5


    VMULL.U16       Q13, D0, D5
    VMLAL.U16       Q13, D2, D4
    VSHR.U32        Q13, Q13, #16
    VMLAL.S16       Q13, D1, D5
    VMLAL.S16       Q13, D3, D4

    VSHL.S32        Q12, Q14, #4
    VLD2.32         {Q0, Q1}, [R0]!
    VADD.I32        Q12, Q12, Q15

    VSHR.S32        Q12, Q12, #16
    VUZP.16         D24, D25


    VSHL.S32        Q11, Q13, #4
    VLD2.16         {D4, D5}, [R7]!
    VADD.I32        Q11, Q11, Q15

    VSHR.S32        Q11, Q11, #16
    VUZP.16         D22, D23


    VQNEG.S16       D20, D22

    VUZP.16         D0, D1
    VUZP.16         D2, D3
    SUB             R8, R8, #8

LOOP_2:

    VMULL.U16       Q14, D0, D4
    VST1.16         D24, [R1]!
    VMLSL.U16       Q14, D2, D5

    VREV64.16       D24, D24
    VMULL.U16       Q13, D0, D5

    VMLAL.U16       Q13, D2, D4
    VST1.16         D24, [R4]
    SUB             R4, R4, #8

    VREV64.16       D22, D22
    VSHR.S32        Q14, Q14, #16
    VSHR.U32        Q13, Q13, #16
    VST1.16         D22, [R3]
    VMLAL.S16       Q14, D1, D4
    VMLSL.S16       Q14, D3, D5
    VST1.16         D20, [R5]!
    VMLAL.S16       Q13, D1, D5
    VMLAL.S16       Q13, D3, D4
    VSHL.S32        Q12, Q14, #4

    SUB             R3, R3, #8
    VLD2.32         {Q0, Q1}, [R0]!
    VSHL.S32        Q11, Q13, #4

    VADD.I32        Q12, Q12, Q15
    VLD2.16         {D4, D5}, [R7]!

    VADD.I32        Q11, Q11, Q15
    VUZP.16         D0, D1
    VSHR.S32        Q12, Q12, #16
    VUZP.16         D24, D25


    VSHR.S32        Q11, Q11, #16
    VUZP.16         D22, D23
    SUBS            R8, R8, #4

    VUZP.16         D2, D3
    VQNEG.S16       D20, D22

    BGT             LOOP_2


    VMULL.U16       Q14, D0, D4
    VST1.16         D24, [R1]!
    VMLSL.U16       Q14, D2, D5
    VREV64.16       D24, D24
    VMULL.U16       Q13, D0, D5
    VMLAL.U16       Q13, D2, D4
    VST1.16         D24, [R4]
    VSHR.S32        Q14, Q14, #16
    SUB             R4, R4, #8
    VST1.16         D20, [R5]!

    VMLAL.S16       Q14, D1, D4
    VMLSL.S16       Q14, D3, D5
    VREV64.16       D22, D22
    VSHR.U32        Q13, Q13, #16
    VST1.16         D22, [R3]
    SUB             R3, R3, #8
    VMLAL.S16       Q13, D1, D5
    VSHL.S32        Q12, Q14, #4
    VMLAL.S16       Q13, D3, D4
    VADD.I32        Q12, Q12, Q15

    VSHL.S32        Q11, Q13, #4

    VSHR.S32        Q12, Q12, #16
    VUZP.16         D24, D25
    VADD.I32        Q11, Q11, Q15

    VST1.16         D24, [R1]!
    VSHR.S32        Q11, Q11, #16
    VREV64.16       D24, D24
    VUZP.16         D22, D23

    VST1.16         D24, [R4]
    VQNEG.S16       D20, D22

    SUB             R4, R4, #2
    VREV64.16       D22, D22
    VST1.16         D22, [R3]
    SUB             R3, R3, #2

    VST1.16         D20, [R5]!
    VLD2.32         {Q0, Q1}, [R0]!

    VLD2.16         {Q2}, [R7]
    ADD             R7, R7, #12
    VUZP.16         D0, D1
    VUZP.16         D2, D3

    VMULL.U16       Q14, D0, D4
    VMLSL.U16       Q14, D2, D5
    VSHR.S32        Q14, Q14, #16
    VMLAL.S16       Q14, D1, D4
    VMLSL.S16       Q14, D3, D5

    VMULL.U16       Q13, D0, D5
    VMLAL.U16       Q13, D2, D4
    VSHR.U32        Q13, Q13, #16
    VMLAL.S16       Q13, D1, D5
    VMLAL.S16       Q13, D3, D4

    VSHL.S32        Q12, Q14, #4
    VADD.I32        Q12, Q12, Q15
    VSHR.S32        Q12, Q12, #16
    VUZP.16         D24, D25

    VSHL.S32        Q11, Q13, #4
    VADD.I32        Q11, Q11, Q15
    VSHR.S32        Q11, Q11, #16
    VUZP.16         D22, D23

    VQNEG.S16       D20, D22

    VST1.16         D24[0], [R1]!
    VST1.16         D24[1], [R1]!
    VST1.16         D24[2], [R1]!

    VST1.16         D24[0], [R4], R9
    VST1.16         D24[1], [R4], R9
    VST1.16         D24[2], [R4], R9

    VST1.16         D22[0], [R3], R9
    VST1.16         D22[1], [R3], R9
    VST1.16         D22[2], [R3], R9

    VST1.16         D20[0], [R5]!
    VST1.16         D20[1], [R5]!
    VST1.16         D20[2], [R5]!
    VUZP.16         D6, D7
    VLD1.16         D0, [R7]!
    VMULL.U16       Q1, D0, D6
    VSHR.S32        Q1, Q1, #16
    VMLAL.S16       Q1, D0, D7
    VSHL.S32        Q1, Q1, #4

    VADD.I32        Q1, Q1, Q15
    VSHR.S32        Q1, Q1, #16

    VST1.16         D2[0], [R1]!
    VST1.16         D2[0], [R4], R9

    LDMFD           sp!, {r4-r12, r15}


